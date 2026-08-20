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
use hfst::backend_thfst::ThfstTransducer;
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_data_types::{HfstTwoLevelPath, HfstTwoLevelPaths, Symbol};
use hfst::hfst_extract_strings::{ExtractStringsCb, RetVal};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_symbol_defs::StringSet;
use hfst::hfst_tokenizer::HfstTokenizer;
use hfst::hfst_transducer::{AnyTransducer, HfstTransducer, HfstTransducerPair};
use hfst::hfst_xerox_rules as xr;
use hfst::transducer::{Transducer, WeightedTables};
use hfst::xfst_compiler::XfstCompiler;
use hfst_openfst::StdVectorFst;

/// The tropical/OL symbol coding lives in process-global statics behind
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

/// State count of a backend transducer, read off its interchange form — an
/// independent witness against the backend's own `number_of_states`.
fn state_count<B: Backend>(b: &B) -> usize {
    let basic = b.to_basic().expect("to_basic");
    (basic.get_max_state() + 1) as usize
}

/// Arc count of a backend transducer, read off its interchange form — the
/// counterpart witness against `number_of_arcs`.
fn arc_count<B: Backend>(b: &B) -> usize {
    let basic = b.to_basic().expect("to_basic");
    basic
        .states_and_transitions()
        .iter()
        .map(|trs| trs.len())
        .sum()
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
        other @ (AnyTransducer::Tropical(_)
        | AnyTransducer::OlW(_)
        | AnyTransducer::OlU(_)
        | AnyTransducer::Thfst(_)) => panic!(
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
            other @ (AnyTransducer::Tropical(_)
            | AnyTransducer::OlW(_)
            | AnyTransducer::OlU(_)
            | AnyTransducer::Thfst(_)) => panic!("expected Foma, got {:?}", other.get_type()),
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
// Generic-path parity: harmonization and compose_intersect.
//
// Four arms of `hfst_transducer.rs` once dropped their C++ counterparts on the
// grounds that foma "is compiled out": harmonize_copy's no-harmonization
// branch, its flag-diacritic pre-insertion, extract_random_paths/n_best, and
// compose_intersect. Each is now served by the generic backend path, which is
// only correct if foma's interchange round trip carries everything those arms
// depend on — the unknown/identity specials and the flag diacritics. The cases
// below are the ones where a lossy round trip would silently change the answer
// rather than fail: disjoint alphabets (so harmonization has real work), the
// two specials (whose meaning is defined by what the alphabet does NOT list),
// and flags (which must survive harmonization unexpanded).
// ---------------------------------------------------------------------------

const UNKNOWN: &str = "@_UNKNOWN_SYMBOL_@";
const IDENTITY: &str = "@_IDENTITY_SYMBOL_@";

/// Build a net from an explicit arc list `(from, isym, osym, to)` and final
/// states — the shapes below branch and self-loop, which `basic_pair` cannot
/// express.
fn basic_arcs(arcs: &[(u32, &str, &str, u32)], finals: &[u32]) -> HfstBasicTransducer {
    let mut n = HfstBasicTransducer::new();
    n.add_state(0);
    for (from, i, o, to) in arcs {
        let tr = HfstBasicTransition::new_symbols(*to, sym(i), sym(o), 0.0, n.coder_mut());
        n.add_transition(*from, &tr, true);
    }
    for f in finals {
        n.set_final_weight(*f, &0.0);
    }
    n
}

/// The `assert_binary_parity` shape for `compose_intersect`, which takes a rule
/// VECTOR rather than a second operand: `lexicon ∘ (⋂ rules)` must come out the
/// same relation on both backends, and equal `expected`.
fn assert_compose_intersect_parity(
    lexicon: &HfstBasicTransducer,
    rules: &[HfstBasicTransducer],
    expected: &[(&str, &str)],
    label: &str,
) {
    let mut f = fac_foma(lexicon);
    let frules: Vec<HfstTransducer<FomaTransducer>> = rules.iter().map(fac_foma).collect();
    f.compose_intersect(&frules, false, true)
        .expect("foma compose_intersect");

    let mut t = fac_trop(lexicon);
    let trules: Vec<HfstTransducer<StdVectorFst>> = rules.iter().map(fac_trop).collect();
    t.compose_intersect(&trules, false, true)
        .expect("openfst compose_intersect");

    let fp = facade_pairs(&f);
    let tp = facade_pairs(&t);
    let ep = expect_pairs(expected);
    assert_eq!(fp, ep, "{label}: foma relation");
    assert_eq!(tp, ep, "{label}: openfst relation");
    assert_eq!(fp, tp, "{label}: foma/openfst parity");
}

// [spec:hfst:sem:foma-backend.algebra-impl/test]
#[test]
fn harmonization_parity_across_disjoint_alphabets_and_specials() {
    let _g = serialized();

    // Baseline: disjoint alphabets, no specials. Harmonization must leave both
    // relations alone rather than let one operand's coding bleed into the other.
    assert_binary_parity(
        &basic_pair("ab", "ab"),
        &basic_pair("cd", "cd"),
        |x, y| {
            x.disjunct(y, true).expect("foma disjunct");
        },
        |x, y| {
            x.disjunct(y, true).expect("openfst disjunct");
        },
        &[("ab", "ab"), ("cd", "cd")],
        "union of disjoint alphabets",
    );

    // `?` on the left, `@` on the right, alphabets {a} and {c}. Harmonization
    // expands each special against the symbols the OTHER operand contributes:
    // the left `?:?` gains `?:c` and `c:?` (the `c:c` case folds into the plain
    // `a c` path), while the right `@:@` gains the identity `a:a`. A round trip
    // that dropped either special would leave 2 paths here.
    assert_binary_parity(
        &basic_arcs(&[(0, "a", "a", 1), (1, UNKNOWN, UNKNOWN, 2)], &[2]),
        &basic_arcs(&[(0, "c", "c", 1), (1, IDENTITY, IDENTITY, 2)], &[2]),
        |x, y| {
            x.disjunct(y, true).expect("foma disjunct");
        },
        |x, y| {
            x.disjunct(y, true).expect("openfst disjunct");
        },
        &[
            ("a@_UNKNOWN_SYMBOL_@", "a@_UNKNOWN_SYMBOL_@"),
            ("a@_UNKNOWN_SYMBOL_@", "ac"),
            ("ac", "a@_UNKNOWN_SYMBOL_@"),
            ("c@_IDENTITY_SYMBOL_@", "c@_IDENTITY_SYMBOL_@"),
            ("ca", "ca"),
        ],
        "union with unknown and identity",
    );

    // A NON-identity unknown (`?:b`): the input side expands, the output side
    // stays pinned to `b`, so the expansion has to be asymmetric.
    assert_binary_parity(
        &basic_arcs(&[(0, "a", "a", 1), (1, UNKNOWN, "b", 2)], &[2]),
        &basic_arcs(&[(0, "c", "c", 1), (1, IDENTITY, IDENTITY, 2)], &[2]),
        |x, y| {
            x.disjunct(y, true).expect("foma disjunct");
        },
        |x, y| {
            x.disjunct(y, true).expect("openfst disjunct");
        },
        &[
            ("a@_UNKNOWN_SYMBOL_@", "ab"),
            ("ac", "ab"),
            ("c@_IDENTITY_SYMBOL_@", "c@_IDENTITY_SYMBOL_@"),
            ("ca", "ca"),
            ("cb", "cb"),
        ],
        "union with a non-identity unknown",
    );
}

// [spec:hfst:sem:foma-backend.algebra-impl/test]
#[test]
fn harmonization_parity_with_flag_carrying_operands() {
    let _g = serialized();

    // A flag is NOT an ordinary symbol: harmonization pre-inserts it into the
    // other operand's alphabet precisely so no `?` expansion ever produces it.
    // Three paths, the flag surviving verbatim on its own.
    assert_binary_parity(
        &basic_arcs(&[(0, "@U.F.A@", "@U.F.A@", 1), (1, "a", "a", 2)], &[2]),
        &basic_arcs(&[(0, "b", "b", 1), (0, "c", "c", 1)], &[1]),
        |x, y| {
            x.disjunct(y, true).expect("foma disjunct");
        },
        |x, y| {
            x.disjunct(y, true).expect("openfst disjunct");
        },
        &[("@U.F.A@a", "@U.F.A@a"), ("b", "b"), ("c", "c")],
        "union with a flag-carrying operand",
    );

    // Flags on both sides, from different features, over disjoint alphabets:
    // neither feature may leak into the other operand's paths.
    assert_binary_parity(
        &basic_arcs(
            &[
                (0, "@P.F.X@", "@P.F.X@", 1),
                (1, "a", "a", 2),
                (2, "@R.F.X@", "@R.F.X@", 3),
            ],
            &[3],
        ),
        &basic_arcs(&[(0, "@U.G.Y@", "@U.G.Y@", 1), (1, "z", "z", 2)], &[2]),
        |x, y| {
            x.disjunct(y, true).expect("foma disjunct");
        },
        |x, y| {
            x.disjunct(y, true).expect("openfst disjunct");
        },
        &[
            ("@P.F.X@a@R.F.X@", "@P.F.X@a@R.F.X@"),
            ("@U.G.Y@z", "@U.G.Y@z"),
        ],
        "union with flags on both operands",
    );
}

// [spec:hfst:sem:foma-backend.algebra-impl/test]
#[test]
fn compose_intersect_parity_vs_tropical() {
    let _g = serialized();

    // Two rules whose INTERSECTION is what the lexicon composes with:
    // {a,b}² ∘ ({aa,ab} ∩ {ab,bb}) = {ab}. Composing with the rules in sequence
    // would give the same answer here; losing one gives {aa,ab} or {ab,bb}.
    let lexicon = basic_arcs(
        &[
            (0, "a", "a", 1),
            (0, "b", "b", 1),
            (1, "a", "a", 2),
            (1, "b", "b", 2),
        ],
        &[2],
    );
    assert_compose_intersect_parity(
        &lexicon,
        &[
            basic_arcs(
                &[(0, "a", "a", 1), (1, "a", "a", 2), (1, "b", "b", 2)],
                &[2],
            ),
            basic_arcs(
                &[(0, "a", "a", 1), (0, "b", "b", 1), (1, "b", "b", 2)],
                &[2],
            ),
        ],
        &[("ab", "ab")],
        "compose_intersect of two acceptor rules",
    );

    // A transducing rule in the usual xerox shape: rewrite `a`, stay identity on
    // everything the rule does not name (`?` / `@`). The specials have to survive
    // the harmonization compose_intersect does internally, or `b` falls off the
    // end of the rule and the result is empty.
    let ab = basic_pair("ab", "ab");
    assert_compose_intersect_parity(
        &ab,
        &[basic_arcs(
            &[
                (0, "a", "A", 0),
                (0, UNKNOWN, UNKNOWN, 0),
                (0, IDENTITY, IDENTITY, 0),
            ],
            &[0],
        )],
        &[("ab", "Ab")],
        "compose_intersect with an unknown-carrying rule",
    );

    // The rule's literal unknown label is protected from harmonization while
    // composing, then restored in the result rather than leaking the private
    // placeholder used by the fast path.
    let unknown_output = basic_pair("a", "a");
    assert_compose_intersect_parity(
        &unknown_output,
        &[basic_arcs(&[(0, "a", UNKNOWN, 1)], &[1])],
        &[("a", UNKNOWN)],
        "compose_intersect preserves a literal unknown output",
    );

    // A rule whose alphabet carries the word boundary `@#@` takes the branch
    // that wraps the lexicon in boundaries before composing.
    assert_compose_intersect_parity(
        &ab,
        &[basic_arcs(
            &[
                (0, "@#@", "@#@", 1),
                (1, "a", "A", 1),
                (1, IDENTITY, IDENTITY, 1),
                (1, "@#@", "@#@", 2),
            ],
            &[2],
        )],
        &[("ab", "@#@Ab@#@")],
        "compose_intersect with a word-boundary rule",
    );
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

/// `substitute_string_transducer` must actually substitute on foma.
///
/// It used to return `self.clone()` — a silent no-op — because foma had no
/// primitive matching an exact `upper:lower` arc, only `fsm_substitute_label`
/// on a single symbol. So xfst `substitute`, regex definition expansion, twolc,
/// pmatch and hfst-substitute all reported success and handed back the original
/// net under `-f foma`. Fixed by adding `fsm_substitute_pair` to the foma crate
/// (0.4.3) rather than by the foma -> basic -> foma round-trip C++ HFST uses.
///
/// Asserted against the tropical backend so the two cannot drift apart.
#[test]
fn foma_substitutes_a_pair_with_transducer_like_tropical() {
    let _g = serialized();
    // "ab" with the a:a arc replaced by the relation x:y.
    let base = basic_pair("ab", "ab");
    let repl = basic_pair("x", "y");

    let mut foma = foma_of(&base);
    foma = foma.substitute_string_transducer((sym("a"), sym("a")), &foma_of(&repl));

    let mut tropical = tropical_of(&base);
    tropical = tropical.substitute_string_transducer((sym("a"), sym("a")), &tropical_of(&repl));

    // Splicing x:y in place of the a:a arc adds states; an unchanged net is
    // the no-op signature this test exists to catch.
    assert_ne!(
        state_count(&foma),
        state_count(&foma_of(&base)),
        "foma substitution was a no-op: the net came back unchanged"
    );
    assert_eq!(
        state_count(&foma),
        state_count(&tropical),
        "foma and tropical disagree after substituting a pair"
    );
}

// ---------------------------------------------------------------------------
// Test 5: path extraction parity vs the tropical openfst backend.
// ---------------------------------------------------------------------------

/// A transducer over explicit symbol vectors, so paths can carry multichar
/// symbols (epsilons, flag diacritics) that `basic_pair`'s per-char split
/// cannot express.
fn basic_symbols(inp: &[&str], outp: &[&str]) -> HfstBasicTransducer {
    assert_eq!(inp.len(), outp.len(), "basic_symbols needs aligned columns");
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    for i in 0..inp.len() {
        let tr = HfstBasicTransition::new_symbols(
            (i + 1) as u32,
            sym(inp[i]),
            sym(outp[i]),
            0.0,
            net.coder_mut(),
        );
        net.add_transition(i as u32, &tr, true);
    }
    net.set_final_weight(inp.len() as u32, &0.0);
    net
}

/// Every callback invocation of a path extraction, as `(is_final, columns)`.
/// This is the whole observable contract: the per-symbol column vector, not
/// just the concatenated words.
type Trace = Vec<(bool, Vec<(String, String)>)>;

struct TraceCb {
    trace: Trace,
    cap: usize,
}

impl ExtractStringsCb for TraceCb {
    fn operator_call(&mut self, path: &mut HfstTwoLevelPath, is_final: bool) -> RetVal {
        self.trace.push((
            is_final,
            path.second
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect(),
        ));
        RetVal::new(self.trace.len() < self.cap, true)
    }
}

/// The extraction trace, sorted so backends that visit arcs in a different
/// order still compare equal (multiplicity preserved).
fn trace_of<B: Backend>(b: &B, cycles: i32, filter_fd: Option<bool>) -> Trace {
    let mut cb = TraceCb {
        trace: Vec::new(),
        cap: 4096,
    };
    match filter_fd {
        None => b.extract_paths_cb(&mut cb, cycles),
        Some(f) => b.extract_paths_fd_cb(&mut cb, cycles, f),
    }
    cb.trace.sort();
    cb.trace
}

/// foma used to hand the callback ONE `StringPair` holding the whole input word
/// and the whole output word, so `HfstTwoLevelPath::second.len()` was always 1.
/// That made `print longest-string-size` report 1 for every net and broke
/// `fst2strings --xfst=print-pairs` / `--xfst=print-space`, which read the
/// per-symbol columns.
// [spec:hfst:sem:foma-backend.lookup-impl/test]
#[test]
fn extract_paths_columns_match_tropical() {
    let _g = serialized();

    let cases: [(&str, HfstBasicTransducer, usize); 3] = [
        (
            "acceptor abc",
            basic_symbols(&["a", "b", "c"], &["a", "b", "c"]),
            3,
        ),
        (
            "relation a:b c:d e",
            basic_symbols(&["a", "c", "e"], &["b", "d", "e"]),
            3,
        ),
        (
            "epsilon output",
            basic_symbols(&["x", "y"], &["x", EPSILON]),
            2,
        ),
    ];

    for (name, net, columns) in cases {
        let foma = trace_of(&foma_of(&net), -1, None);
        let tropical = trace_of(&tropical_of(&net), -1, None);
        assert_eq!(foma, tropical, "extract_paths trace parity for {name}");

        // The defect's signature: a single whole-word pair per path.
        let longest = foma
            .iter()
            .filter(|(is_final, _)| *is_final)
            .map(|(_, cols)| cols.len())
            .max()
            .expect("every case has a final path");
        assert_eq!(longest, columns, "{name}: one column per symbol");
    }
}

/// A final start state must still report the empty path, as the openfst
/// backends do (`regex [a|0]` reaches this).
// [spec:hfst:sem:foma-backend.lookup-impl/test]
#[test]
fn extract_paths_reports_the_empty_path_like_tropical() {
    let _g = serialized();
    let mut net = basic_symbols(&["a"], &["a"]);
    net.set_final_weight(0, &0.0);

    assert_eq!(
        trace_of(&foma_of(&net), -1, None),
        trace_of(&tropical_of(&net), -1, None),
        "empty-path reporting parity"
    );
}

/// `cycles` bounds the traversal per state, rather than being approximated by
/// a cap on the number of paths produced.
// [spec:hfst:sem:foma-backend.lookup-impl/test]
#[test]
fn extract_paths_cycle_bound_matches_tropical() {
    let _g = serialized();
    let net = basic_sigma_star(&["a", "b"]);

    for cycles in [0, 1, 2] {
        assert_eq!(
            trace_of(&foma_of(&net), cycles, None),
            trace_of(&tropical_of(&net), cycles, None),
            "cycles={cycles} traversal parity on a cyclic net"
        );
    }
}

/// `extract_paths_fd_cb` used to ignore `filter_fd` entirely, so flag
/// diacritics never appeared in an extracted path on a foma transducer —
/// `fst2strings --xfst=print-flags` printed nothing where the openfst backends
/// printed the flags.
// [spec:hfst:sem:foma-backend.lookup-impl/test]
#[test]
fn extract_paths_fd_honours_filter_fd_like_tropical() {
    let _g = serialized();
    let net = basic_symbols(&["@U.N.SG@", "a", "b"], &["@U.N.SG@", "a", "b"]);

    let shown = trace_of(&foma_of(&net), -1, Some(false));
    let hidden = trace_of(&foma_of(&net), -1, Some(true));

    assert_eq!(
        shown,
        trace_of(&tropical_of(&net), -1, Some(false)),
        "filter_fd=false (print flags) parity"
    );
    assert_eq!(
        hidden,
        trace_of(&tropical_of(&net), -1, Some(true)),
        "filter_fd=true (filter flags) parity"
    );
    assert_ne!(shown, hidden, "filter_fd made no difference");

    let flagged = |t: &Trace| {
        t.iter()
            .any(|(_, cols)| cols.iter().any(|(a, _)| a == "@U.N.SG@"))
    };
    assert!(flagged(&shown), "filter_fd=false must show the flag");
    assert!(!flagged(&hidden), "filter_fd=true must hide the flag");
}

// ---------------------------------------------------------------------------
// Test 6: infinite ambiguity is about input-epsilon cycles, not cyclicity.
// ---------------------------------------------------------------------------

/// foma used to answer whole-net `is_cyclic()`, which reports every cyclic net
/// as infinitely ambiguous — `a*` is cyclic but reads one input symbol per arc,
/// so it is finitely ambiguous. Only a cycle that consumes no input (an input
/// epsilon or a flag diacritic) is.
// [spec:hfst:sem:foma-backend.lookup-impl/test]
#[test]
fn is_infinitely_ambiguous_needs_an_input_epsilon_cycle() {
    let _g = serialized();

    // a* — cyclic, finitely ambiguous.
    let consuming = basic_sigma_star(&["a"]);
    // (0:a)* — an input-epsilon cycle, infinitely ambiguous.
    let mut epsilon_loop = HfstBasicTransducer::new();
    epsilon_loop.add_state(0);
    let tr =
        HfstBasicTransition::new_symbols(0, sym(EPSILON), sym("a"), 0.0, epsilon_loop.coder_mut());
    epsilon_loop.add_transition(0, &tr, true);
    epsilon_loop.set_final_weight(0, &0.0);

    for (name, net, want) in [("a*", consuming, false), ("(0:a)*", epsilon_loop, true)] {
        let foma = foma_of(&net);
        assert!(foma.is_cyclic(), "{name} is cyclic either way");
        assert_eq!(
            foma.is_infinitely_ambiguous().expect("foma ambiguity"),
            want,
            "foma is_infinitely_ambiguous({name})"
        );
        assert_eq!(
            foma.is_infinitely_ambiguous().expect("foma ambiguity"),
            tropical_of(&net)
                .is_infinitely_ambiguous()
                .expect("tropical ambiguity"),
            "is_infinitely_ambiguous parity for {name}"
        );
    }
}

/// The lookup-time question is about the input, not the whole net: the answer
/// used to be whole-net cyclicity, so every input got the same answer.
// [spec:hfst:sem:foma-backend.lookup-impl/test]
#[test]
fn is_lookup_infinitely_ambiguous_depends_on_the_input() {
    let _g = serialized();

    // 0 -a:a-> 1 (final, with a 0:x self-loop); 0 -b:b-> 2 (final).
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    for (target, i, o) in [(1u32, "a", "a"), (2, "b", "b")] {
        let tr = HfstBasicTransition::new_symbols(target, sym(i), sym(o), 0.0, net.coder_mut());
        net.add_transition(0, &tr, true);
    }
    let loop_tr = HfstBasicTransition::new_symbols(1, sym(EPSILON), sym("x"), 0.0, net.coder_mut());
    net.add_transition(1, &loop_tr, true);
    net.set_final_weight(1, &0.0);
    net.set_final_weight(2, &0.0);

    let mut foma = foma_of(&net);
    let mut ol: Transducer<WeightedTables> =
        <Transducer<WeightedTables> as Backend>::from_basic(&net).expect("OL from_basic");

    for (input, want) in [("a", true), ("b", false)] {
        let sv = vec![sym(input)];
        assert_eq!(
            foma.is_lookup_infinitely_ambiguous_strvec(&sv),
            want,
            "foma is_lookup_infinitely_ambiguous({input})"
        );
        assert_eq!(
            foma.is_lookup_infinitely_ambiguous_strvec(&sv),
            ol.is_lookup_infinitely_ambiguous_strvec(&sv),
            "foma/OL lookup-ambiguity parity for {input}"
        );
        assert_eq!(
            foma.is_lookup_infinitely_ambiguous_str(input),
            want,
            "foma is_lookup_infinitely_ambiguous_str({input})"
        );
    }
}

// ---------------------------------------------------------------------------
// State/arc counts. `Backend::number_of_states` / `number_of_arcs` used to be
// defaulted to 0, and only the tropical backend overrode them — so `hfst xfst
// -f foma` printed "0 states, 0 arcs" for every net it built, a stub value the
// caller printed as fact. The C++ FomaTransducer::number_of_states/_arcs do
// exist (FomaTransducer.cc), so this was a port gap, not a foma limitation.
// ---------------------------------------------------------------------------

/// Both counts, from the backend itself and from its interchange form.
fn counts<B: Backend>(b: &B) -> ((u32, u32), (usize, usize)) {
    (
        (b.number_of_states(), b.number_of_arcs()),
        (state_count(b), arc_count(b)),
    )
}

fn count_cases() -> Vec<(&'static str, HfstBasicTransducer)> {
    vec![
        ("cat", basic_acceptor("cat")),
        ("a:b", basic_pair("a", "b")),
        ("abc:xyz", basic_pair("abc", "xyz")),
        ("{a,b,c}*", basic_sigma_star(&["a", "b", "c"])),
    ]
}

#[test]
fn foma_matches_tropical_state_and_arc_counts() {
    let _g = serialized();

    for (name, net) in count_cases() {
        let foma = foma_of(&net);
        let tropical = tropical_of(&net);
        assert_eq!(
            (foma.number_of_states(), foma.number_of_arcs()),
            (tropical.number_of_states(), tropical.number_of_arcs()),
            "foma/tropical count parity for {name}"
        );
        assert!(
            foma.number_of_states() > 0 && foma.number_of_arcs() > 0,
            "{name} is a non-trivial net, so neither foma count may be 0"
        );
    }
}

#[test]
fn foma_counts_agree_with_its_own_graph() {
    let _g = serialized();

    for (name, net) in count_cases() {
        let (reported, witness) = counts(&foma_of(&net));
        assert_eq!(
            reported,
            (witness.0 as u32, witness.1 as u32),
            "foma counts disagree with its interchange graph for {name}"
        );
    }
}

#[test]
fn optimized_lookup_and_thfst_report_real_counts() {
    let _g = serialized();

    for (name, net) in count_cases() {
        let ol: Transducer<WeightedTables> =
            <Transducer<WeightedTables> as Backend>::from_basic(&net).expect("OL from_basic");
        let (reported, witness) = counts(&ol);
        assert_eq!(
            reported,
            (witness.0 as u32, witness.1 as u32),
            "OL counts disagree with its interchange graph for {name}"
        );
        assert!(
            reported.0 > 0 && reported.1 > 0,
            "{name} is a non-trivial net, so neither OL count may be 0"
        );

        // THFST is the same engine under a different stream identity, so it must
        // report the same counts rather than fall back to a stub.
        let thfst = ThfstTransducer::from_ol(ol);
        assert_eq!(
            (thfst.number_of_states(), thfst.number_of_arcs()),
            reported,
            "THFST count parity with its inner OL engine for {name}"
        );
    }
}

/// The observable defect: the net-size line `hfst xfst -f foma` prints after
/// every command reads these counts straight off the backend.
#[test]
fn xfst_net_size_under_foma_is_nonzero() {
    let _g = serialized();

    let script = "regex [a:b | c:d | e:f];\n";

    let mut foma_c = XfstCompiler::<FomaTransducer>::new_with_impl();
    foma_c.parse(script);
    let foma_top = *foma_c.get_stack().last().expect("foma stack non-empty");
    let foma_size = (
        foma_c.net(foma_top).number_of_states(),
        foma_c.net(foma_top).number_of_arcs(),
    );

    let mut trop_c = XfstCompiler::<StdVectorFst>::new_with_impl();
    trop_c.parse(script);
    let trop_top = *trop_c.get_stack().last().expect("tropical stack non-empty");
    let trop_size = (
        trop_c.net(trop_top).number_of_states(),
        trop_c.net(trop_top).number_of_arcs(),
    );

    assert_eq!(foma_size, (2, 3), "three alternations over two states");
    assert_eq!(foma_size, trop_size, "xfst net size is backend-independent");
}

// ---------------------------------------------------------------------------
// has_weights. Same silent-stub shape as the counts: the trait defaulted it to
// false and only tropical overrode it, so every OL/OLW and THFST transducer
// answered false regardless of what it carried.
// ---------------------------------------------------------------------------

/// `a:b` carrying `arc_w` on its single arc and `final_w` on its final state.
fn basic_weighted(arc_w: f32, final_w: f32) -> HfstBasicTransducer {
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    let tr = HfstBasicTransition::new_symbols(1, sym("a"), sym("b"), arc_w, net.coder_mut());
    net.add_transition(0, &tr, true);
    net.set_final_weight(1, &final_w);
    net
}

/// The three cases that separate "carries a weight" from "is weighted-shaped".
fn weight_cases() -> Vec<(&'static str, HfstBasicTransducer, bool)> {
    vec![
        ("all-zero", basic_weighted(0.0, 0.0), false),
        ("weighted arc", basic_weighted(0.5, 0.0), true),
        ("weighted final", basic_weighted(0.0, 0.5), true),
    ]
}

#[test]
fn has_weights_reports_carried_weights_not_table_shape() {
    let _g = serialized();

    for (name, net, want) in weight_cases() {
        let ol: Transducer<WeightedTables> =
            <Transducer<WeightedTables> as Backend>::from_basic(&net).expect("OL from_basic");

        // Conversions always build weighted-SHAPED tables, so the header flag
        // `stream_type` reads is true even for the all-zero net. `has_weights`
        // deliberately answers the other question, and tropical is the reference
        // for what that question means.
        assert!(
            ol.is_weighted(),
            "{name}: conversions produce weighted-shaped tables"
        );
        assert_eq!(ol.has_weights(), want, "OL has_weights({name})");
        assert_eq!(
            ol.has_weights(),
            tropical_of(&net).has_weights(),
            "OL/tropical has_weights parity for {name}"
        );
    }
}

#[test]
fn foma_and_thfst_report_weights_honestly() {
    let _g = serialized();

    for (name, net, want) in weight_cases() {
        assert!(
            !foma_of(&net).has_weights(),
            "{name}: foma nets have no weight field to carry a weight in"
        );

        let ol: Transducer<WeightedTables> =
            <Transducer<WeightedTables> as Backend>::from_basic(&net).expect("OL from_basic");
        let thfst = ThfstTransducer::from_ol(ol);
        assert_eq!(thfst.has_weights(), want, "THFST has_weights({name})");
    }
}

// ---------------------------------------------------------------------------
// get_initial_input_symbols vs get_first_input_symbols.
//
// The has_weights shape again, one layer subtler: foma answered BOTH from one
// helper that read the start state's out-arcs, so the wrong answer was never
// empty and still varied plausibly with the net — nothing an assertion about
// shape alone can catch. The two are DIFFERENT walks in the contract tropical
// sets, so the test has to be that they DISAGREE where the contract says they
// must, and that each separately agrees with tropical on the same net.
// ---------------------------------------------------------------------------

fn syms(items: &[&str]) -> StringSet {
    items.iter().copied().map(sym).collect()
}

// [spec:hfst:sem:foma-backend.algebra-impl/test]
#[test]
fn initial_and_first_input_symbols_are_different_walks() {
    let _g = serialized();

    // `abc`: initial is the one symbol a path can start with; first is every
    // symbol in the net. Answering both from the start state's out-arcs gives
    // {a} twice — non-empty, correct for `initial`, and wrong for `first`.
    let abc = basic_acceptor("abc");
    let f = foma_of(&abc);
    let t = tropical_of(&abc);

    assert_eq!(f.get_initial_input_symbols(), syms(&["a"]));
    assert_eq!(f.get_first_input_symbols(), syms(&["a", "b", "c"]));
    assert_ne!(
        f.get_initial_input_symbols(),
        f.get_first_input_symbols(),
        "the two walks must disagree on a net longer than one symbol"
    );
    assert_eq!(
        f.get_initial_input_symbols(),
        t.get_initial_input_symbols(),
        "initial-symbol parity with tropical"
    );
    assert_eq!(
        f.get_first_input_symbols(),
        t.get_first_input_symbols(),
        "first-symbol parity with tropical"
    );
}

// [spec:hfst:sem:foma-backend.algebra-impl/test]
#[test]
fn initial_input_symbols_descend_through_epsilon_and_flags() {
    let _g = serialized();

    // Reading the start state's out-arcs literally answers `@_EPSILON_SYMBOL_@`
    // / `@U.F.A@` here — a symbol no path can begin with. `@_UNKNOWN_@` is the
    // control: it is a reserved sigma number too, but it is not epsilon and not
    // a flag, so both walks report it as the real symbol it is.
    let eps = basic_arcs(
        &[(0, EPSILON, EPSILON, 1), (1, "a", "a", 2), (2, "b", "b", 3)],
        &[3],
    );
    let flag = basic_arcs(
        &[
            (0, "@U.F.A@", "@U.F.A@", 1),
            (1, "a", "a", 2),
            (2, "b", "b", 3),
        ],
        &[3],
    );
    let unk = basic_arcs(&[(0, UNKNOWN, IDENTITY, 1), (1, "b", "b", 2)], &[2]);
    // Two branches off the start state, one of them behind an epsilon: both
    // first symbols are initial, and the walk must not stop at the first branch.
    let branch = basic_arcs(
        &[
            (0, "a", "a", 1),
            (1, "b", "b", 2),
            (0, EPSILON, EPSILON, 3),
            (3, "c", "c", 4),
        ],
        &[2, 4],
    );

    let cases: [(&str, &HfstBasicTransducer, &[&str], &[&str]); 4] = [
        ("epsilon prefix", &eps, &["a"], &["a", "b"]),
        ("flag prefix", &flag, &["a"], &["a", "b"]),
        ("unknown arc", &unk, &[UNKNOWN], &[UNKNOWN, "b"]),
        ("epsilon branch", &branch, &["a", "c"], &["a", "b", "c"]),
    ];

    for (name, net, initial, first) in cases {
        let f = foma_of(net);
        let t = tropical_of(net);
        assert_eq!(
            f.get_initial_input_symbols(),
            syms(initial),
            "{name}: foma initial"
        );
        assert_eq!(
            f.get_first_input_symbols(),
            syms(first),
            "{name}: foma first"
        );
        assert_eq!(
            f.get_initial_input_symbols(),
            t.get_initial_input_symbols(),
            "{name}: initial parity with tropical"
        );
        assert_eq!(
            f.get_first_input_symbols(),
            t.get_first_input_symbols(),
            "{name}: first parity with tropical"
        );
    }

    // An empty net has no start state to walk from; both answer the empty set
    // rather than panicking.
    let empty = HfstBasicTransducer::new();
    assert!(foma_of(&empty).get_initial_input_symbols().is_empty());
    assert!(foma_of(&empty).get_first_input_symbols().is_empty());
}

// ---------------------------------------------------------------------------
// Replace-rule marker hygiene (the live consumer of the alphabet edits).
// ---------------------------------------------------------------------------

/// `hfst_xerox_rules` compiles a conditioned replace rule by bracketing the
/// centre with temporary markers (`@LM@`, `@RM@`, `@LM2@`, `@RM2@`, `@1@`, ...),
/// declaring them with `insert_to_alphabet_set` so `?` stops covering them, and
/// stripping them again with `remove_from_alphabet_symbol` / `_set` once the
/// composition is done. Both halves are alphabet edits, so both were silent on
/// the foma backend while `from_basic` rebuilt the sigma from arcs alone: the
/// declarations never landed and the strip had nothing to strip.
///
/// Now that they land, the strip has to be real — a marker left in the sigma is
/// a symbol `?` no longer matches, which changes what the rule accepts.
// ab -> x || ab _ a  (test1 of test_xerox_rules.rs, on the foma backend)
#[test]
fn replace_rule_leaves_no_markers_in_the_alphabet() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    type B = FomaTransducer;

    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol(EPSILON);

    let mapping_pair: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("ab", &tok)?,
        HfstTransducer::<B>::new_tokenized("x", &tok)?,
    );
    let context: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("ab", &tok)?,
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
    );

    let rule = xr::Rule::new_mapping_context_repl_type(
        &vec![mapping_pair],
        &vec![context],
        xr::ReplaceType::REPL_UP,
    )?;
    let replace_tr = xr::replace_rule(&rule, false)?;

    let alphabet = replace_tr.get_alphabet()?;
    let leftovers: BTreeSet<String> = alphabet
        .iter()
        .map(|s| s.to_string())
        .filter(|s| s.starts_with('@') && s.ends_with('@') && s != EPSILON)
        .collect();
    assert!(
        leftovers.is_empty(),
        "foma replace-rule compile left temporary markers in the alphabet: {leftovers:?}"
    );
    assert_eq!(
        alphabet
            .iter()
            .map(|s| s.to_string())
            .filter(|s| !s.starts_with('@'))
            .collect::<BTreeSet<String>>(),
        BTreeSet::from(["a".to_string(), "b".to_string(), "x".to_string()]),
        "foma replace-rule compile lost or gained an ordinary alphabet symbol"
    );

    // The markers being gone is only worth asserting if the rule they built is
    // the right one: `abababa` has exactly one non-optional upward replacement.
    let input = HfstTransducer::<B>::new_tokenized("abababa", &tok)?;
    let expected = HfstTransducer::<B>::new_tokenized_pair(
        "abababa",
        "abx@_EPSILON_SYMBOL_@x@_EPSILON_SYMBOL_@a",
        &tok,
    )?;
    let mut got = input.clone();
    got.compose(&replace_tr, true)?.minimize()?;
    assert!(
        got.compare(&expected, true)?,
        "foma replace-rule compile produced the wrong relation"
    );
    Ok(())
}
