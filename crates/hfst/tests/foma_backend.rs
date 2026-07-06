//! Integration tests for the native foma backend (`--features foma`).
//!
//! The whole file is gated on the `foma` feature: with it off there is no
//! `FomaTransducer`, no `AnyTransducer::Foma` arm, and no foma stream I/O, so
//! nothing here should compile into the test binary.
//!
//! These exercise the backend through hfst's PUBLIC surface only (an
//! integration test crate cannot name the `foma` crate directly — it is a
//! regular, not dev, dependency of `hfst`). So a "foma-constructed" net is
//! built via the `AlgebraBackend` constructors, which are thin wrappers over
//! the very foma primitives the task calls out: `define_transducer_symbol_pair`
//! is `fsm_cross_product(fsm_symbol, fsm_symbol)`, `disjunct` is `fsm_union`,
//! etc. (see `backend_foma.rs`).
#![cfg(feature = "foma")]

use std::collections::BTreeSet;

use hfst::backend::{AlgebraBackend, Backend, LookupBackend};
use hfst::backend_foma::FomaTransducer;
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_data_types::{HfstTwoLevelPath, HfstTwoLevelPaths, Symbol};
use hfst::hfst_extract_strings::{ExtractStringsCb, RetVal};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_transducer::{AnyTransducer, HfstTransducer};
use hfst::transducer::{Transducer, WeightedTables};
use hfst_openfst::StdVectorFst;

/// The tropical/log/OL symbol coding lives in process-global statics behind
/// their own mutexes; cargo runs every `#[test]` as a parallel thread in ONE
/// process, so tests touching the OpenFst family serialize through this lock to
/// restore the one-at-a-time-per-process model (mirrors test_streams.rs).
static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

const EPSILON: &str = "@_EPSILON_SYMBOL_@";

fn sym(s: &str) -> Symbol {
    Symbol::from(s)
}

// ---------------------------------------------------------------------------
// HfstBasicTransducer builders (the common parity source).
// ---------------------------------------------------------------------------

/// A transducer mapping the char sequence `inp` to `outp` (per-column aligned;
/// equal char counts required). `inp == outp` yields an acceptor.
fn basic_pair(inp: &str, outp: &str) -> HfstBasicTransducer {
    let ic: Vec<String> = inp.chars().map(|c| c.to_string()).collect();
    let oc: Vec<String> = outp.chars().map(|c| c.to_string()).collect();
    assert_eq!(ic.len(), oc.len(), "basic_pair needs aligned columns");
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    for i in 0..ic.len() {
        let tr = HfstBasicTransition::new_symbols(
            (i + 1) as u32,
            sym(&ic[i]),
            sym(&oc[i]),
            0.0,
            net.coder_mut(),
        );
        net.add_transition(i as u32, &tr, true);
    }
    net.set_final_weight(ic.len() as u32, &0.0);
    net
}

fn basic_acceptor(word: &str) -> HfstBasicTransducer {
    basic_pair(word, word)
}

/// `{a,b,c}*` as a one-state acceptor with a self-loop per symbol.
fn basic_sigma_star(symbols: &[&str]) -> HfstBasicTransducer {
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    for s in symbols {
        let tr = HfstBasicTransition::new_symbols(0, sym(s), sym(s), 0.0, net.coder_mut());
        net.add_transition(0, &tr, true);
    }
    net.set_final_weight(0, &0.0);
    net
}

fn foma_of(net: &HfstBasicTransducer) -> FomaTransducer {
    FomaTransducer::from_basic(net).expect("foma from_basic")
}

fn tropical_of(net: &HfstBasicTransducer) -> StdVectorFst {
    <StdVectorFst as Backend>::from_basic(net).expect("tropical from_basic")
}

/// State count of a backend transducer, read off its interchange form so foma
/// and openfst are compared on the same footing (FomaTransducer does not
/// implement `number_of_states`).
fn state_count<B: Backend>(b: &B) -> usize {
    let basic = b.to_basic().expect("to_basic");
    (basic.get_max_state() + 1) as usize
}

// ---------------------------------------------------------------------------
// Backend-agnostic accepted-relation extractor (cross-backend equivalence).
// ---------------------------------------------------------------------------

/// Collects each complete path as a canonical `(input, output)` string pair by
/// concatenating non-epsilon input/output symbols. This normalizes foma's
/// whole-word single-pair paths and openfst's per-column paths to the same
/// shape, so the recognized *relation* can be compared across backends.
struct PairCollector {
    pairs: BTreeSet<(String, String)>,
    cap: usize,
}

impl ExtractStringsCb for PairCollector {
    fn operator_call(&mut self, path: &mut HfstTwoLevelPath, is_final: bool) -> RetVal {
        if is_final {
            let mut i = String::new();
            let mut o = String::new();
            for (a, b) in path.second.iter() {
                if a.as_str() != EPSILON {
                    i.push_str(a.as_str());
                }
                if b.as_str() != EPSILON {
                    o.push_str(b.as_str());
                }
            }
            self.pairs.insert((i, o));
        }
        RetVal::new(self.pairs.len() < self.cap, true)
    }
}

/// The accepted `(input, output)` string-pair set of an (acyclic) backend
/// transducer.
fn accepted_pairs<B: Backend>(b: &B) -> BTreeSet<(String, String)> {
    let mut cb = PairCollector {
        pairs: BTreeSet::new(),
        cap: 4096,
    };
    b.extract_paths_cb(&mut cb, -1);
    cb.pairs
}

fn expect_pairs(items: &[(&str, &str)]) -> BTreeSet<(String, String)> {
    items
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
}

// ---------------------------------------------------------------------------
// snapshot (round-trip structural fingerprint).
// ---------------------------------------------------------------------------

type Snapshot = (
    usize,
    BTreeSet<u32>,
    BTreeSet<String>,
    BTreeSet<(u32, String, String, u32)>,
);

fn snapshot(net: &HfstBasicTransducer) -> Snapshot {
    let coder = net.coder();
    let n_states = (net.get_max_state() + 1) as usize;
    let mut finals = BTreeSet::new();
    let mut arcs = BTreeSet::new();
    for (s, transitions) in net.states_and_transitions().iter().enumerate() {
        let s = s as u32;
        if net.is_final_state(s) {
            finals.insert(s);
        }
        for tr in transitions.iter() {
            arcs.insert((
                s,
                tr.get_input_symbol(coder).to_string(),
                tr.get_output_symbol(coder).to_string(),
                tr.get_target_state(),
            ));
        }
    }
    let alphabet = net
        .get_alphabet()
        .iter()
        .map(|s| s.to_string())
        .collect::<BTreeSet<String>>();
    (n_states, finals, alphabet, arcs)
}

// ---------------------------------------------------------------------------
// Test 1: .foma round-trip through the real HfstInputStream.
// ---------------------------------------------------------------------------

/// Build the HFST framing `HfstOutputStream::operator<<` would prepend for a
/// FOMA_TYPE payload (the deferred `FomaOutputStream` makes the real stream
/// panic, so the header is assembled here byte-for-byte the way the C++/facade
/// writer does). Feeding this to `HfstInputStream` routes to the FOMA_TYPE read
/// arm instead of the raw-gzip `FileIsInGzFormat` bail in `guess_fst_type`.
fn hfst_frame_foma(payload: &[u8]) -> Vec<u8> {
    let mut content: Vec<u8> = Vec::new();
    for (k, v) in [("version", "3.3"), ("type", "FOMA")] {
        content.extend_from_slice(k.as_bytes());
        content.push(0);
        content.extend_from_slice(v.as_bytes());
        content.push(0);
    }
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(b"HFST");
    out.push(0);
    let len = content.len() as u16; // reader reconstructs little-endian: low then high
    out.push((len & 0xff) as u8);
    out.push((len >> 8) as u8);
    out.push(0);
    out.extend_from_slice(&content);
    out.extend_from_slice(payload);
    out
}

// [spec:hfst:sem:foma-backend.stream-io/test]
// [spec:hfst:sem:foma-backend.to-basic-fn/test]
// [spec:hfst:sem:foma-backend.from-basic-fn/test]
#[test]
fn foma_stream_round_trip_through_hfst_input_stream() {
    // A genuine foma-constructed net: (a:b | c:d), built from
    // fsm_cross_product(fsm_symbol,fsm_symbol) unioned via fsm_union.
    let ab = FomaTransducer::define_transducer_symbol_pair("a", "b");
    let cd = FomaTransducer::define_transducer_symbol_pair("c", "d");
    let original = ab.disjunct(&cd);
    let basic1 = original.to_basic().expect("to_basic original");

    // Backend::write -> native gzip-compressed .foma image.
    let mut payload: Vec<u8> = Vec::new();
    original
        .write(&mut payload, false)
        .expect("Backend::write foma payload");
    assert_eq!(
        &payload[0..2],
        &[0x1f, 0x8b],
        "foma payload is the gzip-compressed native image"
    );

    // Frame as an HFST stream and read back through the real HfstInputStream.
    let bytes = hfst_frame_foma(&payload);
    let path = std::env::temp_dir().join(format!(
        "hfst_foma_roundtrip_{}_{}.hfst",
        std::process::id(),
        line!()
    ));
    std::fs::write(&path, &bytes).expect("write temp .hfst");

    let mut instream = HfstInputStream::new_filename(path.to_str().unwrap())
        .expect("HfstInputStream over framed foma bytes");
    let any = instream.read().expect("read foma transducer from stream");
    instream.close();
    let _ = std::fs::remove_file(&path);

    let basic2 = match any {
        AnyTransducer::Foma(t) => t.to_basic().expect("to_basic round-tripped"),
        other => panic!(
            "stream yielded the wrong variant, expected Foma, got type {:?}",
            other.get_type()
        ),
    };

    // The recognized relation, alphabet, states, finals and arcs survive the
    // Backend::write -> HfstInputStream::read -> to_basic round trip unchanged.
    assert_eq!(
        snapshot(&basic1),
        snapshot(&basic2),
        "foma stream round trip must preserve to_basic exactly"
    );

    // And the recognized relation is what we built.
    let read_back = FomaTransducer::from_basic(&basic2).expect("from_basic read-back");
    assert_eq!(
        accepted_pairs(&read_back),
        expect_pairs(&[("a", "b"), ("c", "d")]),
        "round-tripped net recognizes {{a:b, c:d}}"
    );
}

// [spec:hfst:sem:foma-backend.stream-io/test]
// Regression: a MULTI-transducer FOMA stream (each transducer its own
// [HFST header][gzip image], as twolc emits one per rule) must read back every
// transducer, not just the first. The read arm once slurped the whole tail and
// parsed a single gzip member, so downstream a 46-rule twolc phonology came back
// as 1 rule. The fix reads exactly one gzip member and ungets the leftover.
#[test]
fn foma_stream_reads_every_transducer_in_a_multi_stream() {
    let pairs = [("a", "b"), ("c", "d"), ("e", "f")];
    let mut bytes: Vec<u8> = Vec::new();
    for (i, o) in pairs {
        let t = FomaTransducer::define_transducer_symbol_pair(i, o);
        let mut payload: Vec<u8> = Vec::new();
        t.write(&mut payload, false)
            .expect("Backend::write foma payload");
        bytes.extend_from_slice(&hfst_frame_foma(&payload));
    }

    let path = std::env::temp_dir().join(format!(
        "hfst_foma_multi_{}_{}.hfst",
        std::process::id(),
        line!()
    ));
    std::fs::write(&path, &bytes).expect("write temp multi .hfst");

    let mut instream = HfstInputStream::new_filename(path.to_str().unwrap())
        .expect("HfstInputStream over multi framed foma bytes");
    let mut got: Vec<std::collections::BTreeSet<(String, String)>> = Vec::new();
    while !instream.is_eof() {
        let any = instream
            .read()
            .expect("read foma transducer from multi stream");
        let basic = match any {
            AnyTransducer::Foma(t) => t.to_basic().expect("to_basic"),
            other => panic!("expected Foma, got {:?}", other.get_type()),
        };
        let t = FomaTransducer::from_basic(&basic).expect("from_basic");
        got.push(accepted_pairs(&t));
    }
    instream.close();
    let _ = std::fs::remove_file(&path);

    assert_eq!(got.len(), 3, "all three transducers must be read back");
    for ((i, o), relation) in pairs.iter().zip(got.iter()) {
        assert_eq!(
            relation,
            &expect_pairs(&[(i, o)]),
            "transducer {i}:{o} round-trips"
        );
    }
}

// ---------------------------------------------------------------------------
// Test 2: algebra parity vs the tropical openfst backend.
// ---------------------------------------------------------------------------

/// Accepted `(input, output)` pairs of a FACADE transducer, via the facade's
/// own `extract_paths`.
fn facade_pairs<B: AlgebraBackend>(t: &HfstTransducer<B>) -> BTreeSet<(String, String)> {
    let mut results = HfstTwoLevelPaths::new();
    t.extract_paths(&mut results, -1, -1)
        .expect("extract_paths on an acyclic result");
    results
        .iter()
        .map(|p| {
            let mut i = String::new();
            let mut o = String::new();
            for (a, b) in p.second.iter() {
                if a.as_str() != EPSILON {
                    i.push_str(a.as_str());
                }
                if b.as_str() != EPSILON {
                    o.push_str(b.as_str());
                }
            }
            (i, o)
        })
        .collect()
}

fn fac_foma(net: &HfstBasicTransducer) -> HfstTransducer<FomaTransducer> {
    HfstTransducer::from_basic(net)
}

fn fac_trop(net: &HfstBasicTransducer) -> HfstTransducer<StdVectorFst> {
    HfstTransducer::from_basic(net)
}

/// Assert foma and openfst recognize the same relation after the binary op, and
/// that it equals `expected`. The op runs through the facade so both sides get
/// the same symbol harmonization (raw backend binary ops do not harmonize; the
/// tropical backend's local symbol tables would otherwise collide across
/// disjoint alphabets). Both operands are built from the SAME
/// HfstBasicTransducer per side — the recommended parity harness.
fn assert_binary_parity(
    lhs: &HfstBasicTransducer,
    rhs: &HfstBasicTransducer,
    fop: impl Fn(&mut HfstTransducer<FomaTransducer>, &HfstTransducer<FomaTransducer>),
    top: impl Fn(&mut HfstTransducer<StdVectorFst>, &HfstTransducer<StdVectorFst>),
    expected: &[(&str, &str)],
    label: &str,
) {
    let mut f = fac_foma(lhs);
    fop(&mut f, &fac_foma(rhs));
    let mut t = fac_trop(lhs);
    top(&mut t, &fac_trop(rhs));

    let fp = facade_pairs(&f);
    let tp = facade_pairs(&t);
    let ep = expect_pairs(expected);
    assert_eq!(fp, ep, "{label}: foma relation");
    assert_eq!(tp, ep, "{label}: openfst relation");
    assert_eq!(fp, tp, "{label}: foma/openfst parity");
}

// [spec:hfst:sem:foma-backend.algebra-impl/test]
#[test]
fn algebra_parity_union_intersect_compose_subtract_concat() {
    let _g = serialized();

    let a = basic_acceptor("a");
    let b = basic_acceptor("b");

    // union {a} ∪ {b} = {a, b}
    assert_binary_parity(
        &a,
        &b,
        |x, y| {
            x.disjunct(y, true).unwrap();
        },
        |x, y| {
            x.disjunct(y, true).unwrap();
        },
        &[("a", "a"), ("b", "b")],
        "union {a,b}",
    );

    // intersect {a,b,c}* ∩ {b} = {b}
    let star = basic_sigma_star(&["a", "b", "c"]);
    assert_binary_parity(
        &star,
        &b,
        |x, y| {
            x.intersect(y, true).unwrap();
        },
        |x, y| {
            x.intersect(y, true).unwrap();
        },
        &[("b", "b")],
        "intersect {a,b,c}* with {b}",
    );

    // compose a:b ∘ b:c = a:c
    let ab = basic_pair("a", "b");
    let bc = basic_pair("b", "c");
    assert_binary_parity(
        &ab,
        &bc,
        |x, y| {
            x.compose(y, true).unwrap();
        },
        |x, y| {
            x.compose(y, true).unwrap();
        },
        &[("a", "c")],
        "compose a:b ∘ b:c",
    );

    // subtract {a,b} - {b} = {a}
    let a_or_b = {
        let mut n = basic_acceptor("a");
        let tr = HfstBasicTransition::new_symbols(1, sym("b"), sym("b"), 0.0, n.coder_mut());
        n.add_transition(0, &tr, true);
        n // 0 -a-> 1(final), 0 -b-> 1(final): accepts {a, b}
    };
    assert_binary_parity(
        &a_or_b,
        &b,
        |x, y| {
            x.subtract(y, true).unwrap();
        },
        |x, y| {
            x.subtract(y, true).unwrap();
        },
        &[("a", "a")],
        "subtract {a,b} - {b}",
    );

    // concatenate {a} · {b} = {ab}
    assert_binary_parity(
        &a,
        &b,
        |x, y| {
            x.concatenate(y, true).unwrap();
        },
        |x, y| {
            x.concatenate(y, true).unwrap();
        },
        &[("ab", "ab")],
        "concatenate a·b",
    );
}

// [spec:hfst:sem:foma-backend.algebra-impl/test]
#[test]
fn algebra_parity_determinize_minimize_nondeterministic_union() {
    let _g = serialized();

    // A nondeterministic {a} | {a}: two parallel a-arcs 0 -> 1 (both final).
    let mut nd = HfstBasicTransducer::new();
    nd.add_state(0);
    for _ in 0..2 {
        let tr = HfstBasicTransition::new_symbols(1, sym("a"), sym("a"), 0.0, nd.coder_mut());
        nd.add_transition(0, &tr, true);
    }
    nd.set_final_weight(1, &0.0);

    let f = foma_of(&nd).determinize(false).minimize(false);
    let t = tropical_of(&nd).determinize(false).minimize(false);

    // Both collapse the duplicate path to the single relation {a:a}.
    assert_eq!(accepted_pairs(&f), expect_pairs(&[("a", "a")]));
    assert_eq!(accepted_pairs(&t), expect_pairs(&[("a", "a")]));
    assert_eq!(accepted_pairs(&f), accepted_pairs(&t), "det/min parity");

    // The minimal acceptor of {a} is 2 states (start + final) in both backends.
    assert!(
        state_count(&f) <= state_count(&t),
        "foma minimal ({}) must be <= openfst minimal ({})",
        state_count(&f),
        state_count(&t)
    );
    assert_eq!(state_count(&f), 2, "minimal {{a}} is 2 states in foma");
}

// ---------------------------------------------------------------------------
// Test 3: boolean-determinize/minimize non-blowup vs weighted (tropical).
// ---------------------------------------------------------------------------

// The sma-tokeniser blowup this backend exists to fix: plan/main.styx:97,104
// record that hfst's tropical minimize/determinize (encode_weights=false, the
// pmatch default) runs WEIGHTED subset construction — it tracks residual
// weights and so cannot merge states that are language-equivalent but
// weight-divergent, exploding the sma pmatch archive to ~538MB. foma
// determinizes/minimizes UNWEIGHTED automata (boolean subset construction),
// which merges those states freely.
//
// This unit test reproduces the mechanism in miniature: a "reconvergent
// diamond" whose two forks (on inputs `p` and `q`) reach the SAME pair of NFA
// states {1,2} but with different accumulated weights (the `q`->2 arc costs 5,
// every other arc 0), and states 1/2 then share the tail language {t, u}.
// Unweighted (foma), boolean subset construction sees ONE subset {1,2} for both
// forks and boolean minimize fuses everything -> 3 states. Weighted (tropical,
// encode_weights=false — the pmatch default), the residual on the `u`-branch
// (0 via p, 5 via q) cannot be pushed away because state 2 has two in-arcs of
// different weight, so determinize keeps {1,2}@0 and {1,2}@5 as distinct states
// and minimize cannot merge them -> more states. The full pmatch-archive repro
// needs the sma pmscript data and is out of scope for a unit test (see the
// #[ignore]d stub below).
#[test]
fn boolean_minimize_does_not_blow_up_vs_weighted() {
    let _g = serialized();

    // 0 -p(0)-> 1, 0 -p(0)-> 2, 0 -q(0)-> 1, 0 -q(5)-> 2
    // 1 -t(0)-> 3(final), 2 -u(0)-> 4(final)   -> language {pt, pu, qt, qu}
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    for (sym_str, tgt, w) in [
        ("p", 1u32, 0.0f32),
        ("p", 2, 0.0),
        ("q", 1, 0.0),
        ("q", 2, 5.0),
    ] {
        let tr =
            HfstBasicTransition::new_symbols(tgt, sym(sym_str), sym(sym_str), w, net.coder_mut());
        net.add_transition(0, &tr, true);
    }
    let tr = HfstBasicTransition::new_symbols(3, sym("t"), sym("t"), 0.0, net.coder_mut());
    net.add_transition(1, &tr, true);
    let tr = HfstBasicTransition::new_symbols(4, sym("u"), sym("u"), 0.0, net.coder_mut());
    net.add_transition(2, &tr, true);
    net.set_final_weight(3, &0.0);
    net.set_final_weight(4, &0.0);

    let f = foma_of(&net).determinize(false).minimize(false);
    let t = tropical_of(&net).determinize(false).minimize(false);

    // Both recognize the same language (foma just drops the weights).
    assert_eq!(
        accepted_pairs(&f),
        expect_pairs(&[("pt", "pt"), ("pu", "pu"), ("qt", "qt"), ("qu", "qu")]),
        "foma recognizes {{pt,pu,qt,qu}}"
    );
    assert_eq!(accepted_pairs(&f), accepted_pairs(&t), "language parity");

    let fs = state_count(&f);
    let ts = state_count(&t);
    eprintln!("boolean-vs-weighted minimize: foma={fs} states, openfst={ts} states");

    // The invariant the backend guarantees: foma never has MORE states than the
    // weighted openfst result.
    assert!(
        fs <= ts,
        "foma minimal ({fs}) must be <= openfst minimal ({ts})"
    );
    // And here, where the tropical result is inflated by weight diversity, foma
    // is strictly smaller — the boolean-vs-weighted merge difference.
    assert!(
        fs < ts,
        "weight-divergent branches: foma ({fs}) must be strictly smaller than openfst ({ts})"
    );
}

// [spec:hfst:sem:foma-backend.algebra-impl/test]
// The real repro of the plan/main.styx:97,104 blowup: compile the lang-sma
// tokeniser-disamb pmscript to a pmatch archive under both backends and compare
// archive sizes / TOP state+arc counts. It needs the sma pmscript data (the
// `lang-sma` pmatch sources), which is not vendored into this repo, so it is
// documented here rather than run.
//
//   1. Obtain the lang-sma tokeniser-disamb pmscript (the sma pmatch sources).
//   2. `hfst pmatch2fst` it with the tropical/openfst backend (weighted
//      minimize, encode_weights=false) -> ~538MB, TOP ~553k states / ~22.7M
//      arcs (plan/main.styx:104).
//   3. Route the same build's optimize()/minimize() chain through the foma
//      backend (boolean subset construction) and confirm the archive stays
//      compact — that is what this backend is for.
#[test]
#[ignore = "needs the lang-sma pmscript data (out of scope for a unit test); see plan/main.styx:97,104"]
fn sma_tokeniser_pmatch_archive_blowup_repro() {
    unimplemented!("documentation stub: see the comment above for the manual repro steps");
}

// ---------------------------------------------------------------------------
// Test 4: lookup parity vs the optimized-lookup (openfst-family) backend.
// ---------------------------------------------------------------------------

/// The set of output words `lookup(input)` yields, as concatenated strings.
fn lookup_outputs(paths: &hfst::hfst_data_types::HfstOneLevelPaths) -> BTreeSet<String> {
    paths
        .iter()
        .map(|p| p.second.iter().map(|s| s.as_str()).collect::<String>())
        .collect()
}

// [spec:hfst:sem:foma-backend.lookup-impl/test]
#[test]
fn lookup_parity_vs_optimized_lookup() {
    let _g = serialized();

    // cat:dog and the nondeterministic a:b | a:c.
    let cat_dog = basic_pair("cat", "dog");
    let mut a_bc = basic_pair("a", "b");
    let tr = HfstBasicTransition::new_symbols(2, sym("a"), sym("c"), 0.0, a_bc.coder_mut());
    a_bc.add_transition(0, &tr, true);
    a_bc.set_final_weight(2, &0.0); // 0 -a:b-> 1(final), 0 -a:c-> 2(final)

    for (net, input, expected) in [(&cat_dog, "cat", vec!["dog"]), (&a_bc, "a", vec!["b", "c"])] {
        let mut foma = foma_of(net);
        // The openfst-family lookup path is the optimized-lookup backend, which
        // is how hfst looks up an openfst transducer (Backend::from_basic builds
        // the weighted-shaped OL tables).
        let mut ol: Transducer<WeightedTables> =
            <Transducer<WeightedTables> as Backend>::from_basic(net).expect("OL from_basic");

        let foma_out = lookup_outputs(&foma.lookup_fd_str(input, -1, 0.0));
        let ol_out = lookup_outputs(&ol.lookup_fd_str(input, -1, 0.0));
        let want: BTreeSet<String> = expected.iter().map(|s| s.to_string()).collect();

        assert_eq!(foma_out, want, "foma lookup({input})");
        assert_eq!(ol_out, want, "OL lookup({input})");
        assert_eq!(foma_out, ol_out, "foma/OL lookup parity for {input}");
    }

    // Unknown input yields the empty set in both backends.
    let mut foma = foma_of(&cat_dog);
    let mut ol: Transducer<WeightedTables> =
        <Transducer<WeightedTables> as Backend>::from_basic(&cat_dog).expect("OL from_basic");
    assert!(
        foma.lookup_fd_str("zzz", -1, 0.0).is_empty(),
        "foma lookup of unknown input is empty"
    );
    assert!(
        ol.lookup_fd_str("zzz", -1, 0.0).is_empty(),
        "OL lookup of unknown input is empty"
    );
}
