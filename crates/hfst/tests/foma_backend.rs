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
//!
//! The query tests (lookup, path extraction, ambiguity, counts, weights, input
//! symbols and the alphabet) are in foma_backend_queries.rs.
#![cfg(feature = "foma")]

mod foma_backend_common;

use std::collections::BTreeSet;

use foma_backend_common::{
    EPSILON, IDENTITY, UNKNOWN, arc_count, basic_acceptor, basic_arcs, basic_pair,
    basic_sigma_star, foma_of, serialized, state_count, sym, tropical_of,
};
use hfst::backend::{AlgebraBackend, Backend};
use hfst::backend_foma::FomaTransducer;
use hfst::convert_transducer_format::ConversionFunctions;
use hfst::guessify_fst::{GuessDirection, affix_guessify};
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_data_types::{HfstTwoLevelPath, HfstTwoLevelPaths};
use hfst::hfst_extract_strings::{ExtractStringsCb, RetVal};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_tokenizer::HfstTokenizer;
use hfst::hfst_transducer::{AnyTransducer, HfstTransducer, HfstTransducerPair};
use hfst::hfst_xerox_rules as xr;
use hfst_openfst::StdVectorFst;

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

/// Build the HFST framing `HfstOutputStream::write` would prepend for a
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
fn foma_stream_round_trip_through_hfst_input_stream() -> hfst::error::Result<()> {
    // A genuine foma-constructed net: (a:b | c:d), built from
    // fsm_cross_product(fsm_symbol,fsm_symbol) unioned via fsm_union.
    let ab = FomaTransducer::define_transducer_symbol_pair("a", "b");
    let cd = FomaTransducer::define_transducer_symbol_pair("c", "d");
    let original = ab.disjunct(&cd)?;
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
    Ok(())
}

// [spec:hfst:sem:foma-backend.stream-io/test]
// Regression: a MULTI-transducer FOMA stream (each transducer its own
// [HFST header][gzip image], as twolc emits one per rule) must read back every
// transducer, not just the first. The read arm once slurped the whole tail and
// parsed a single gzip member, so downstream a 46-rule twolc phonology came back
// as 1 rule. The fix reads exactly one gzip member and ungets the leftover.
#[test]
fn foma_stream_reads_every_transducer_in_multi_stream() {
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
    HfstTransducer::new_from_basic(net)
        .expect("converting a basic transducer to an available backend type cannot fail")
}

fn fac_trop(net: &HfstBasicTransducer) -> HfstTransducer<StdVectorFst> {
    HfstTransducer::new_from_basic(net)
        .expect("converting a basic transducer to an available backend type cannot fail")
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

    // subtract {a:x, a:y} - {a:y} = {a:x}. A difference of RELATIONS: both
    // operands agree on the input side, so a subtraction that only looked at
    // languages would answer the empty net. foma's complement is single-tape
    // and cannot answer this alone (see 'pair_complement' in backend_foma/algebra.rs).
    let ax_or_ay = {
        let mut n = basic_pair("a", "x");
        let tr = HfstBasicTransition::new_symbols(1, sym("a"), sym("y"), 0.0, n.coder_mut());
        n.add_transition(0, &tr, true);
        n // 0 -a:x-> 1(final), 0 -a:y-> 1(final)
    };
    assert_binary_parity(
        &ax_or_ay,
        &basic_pair("a", "y"),
        |x, y| {
            x.subtract(y, true).unwrap();
        },
        |x, y| {
            x.subtract(y, true).unwrap();
        },
        &[("a", "x")],
        "subtract {a:x,a:y} - {a:y}",
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
fn algebra_parity_determinize_minimize_nondeterministic_union() -> hfst::error::Result<()> {
    let _g = serialized();

    // A nondeterministic {a} | {a}: two parallel a-arcs 0 -> 1 (both final).
    let mut nd = HfstBasicTransducer::new();
    nd.add_state(0);
    for _ in 0..2 {
        let tr = HfstBasicTransition::new_symbols(1, sym("a"), sym("a"), 0.0, nd.coder_mut());
        nd.add_transition(0, &tr, true);
    }
    nd.set_final_weight(1, &0.0);

    let f = foma_of(&nd).determinize(false)?.minimize(false)?;
    let t = tropical_of(&nd).determinize(false)?.minimize(false)?;

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
    Ok(())
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
fn boolean_minimize_does_not_blow_up_vs_weighted() -> hfst::error::Result<()> {
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

    let f = foma_of(&net).determinize(false)?.minimize(false)?;
    let t = tropical_of(&net).determinize(false)?.minimize(false)?;

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
    Ok(())
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
// Replace-rule marker hygiene (the live consumer of the alphabet edits).
// ---------------------------------------------------------------------------

/// `hfst_xerox_rules` compiles a conditioned replace rule by bracketing the
/// centre with temporary markers (`@LM@`, `@RM@`, `@LM2@`, `@RM2@`, `@1@`, ...),
/// declaring them with `insert_to_alphabet_string_set` so `?` stops covering
/// them, and stripping them again with `remove_from_alphabet_string` / `_set` once the
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
    // The three special strings belong to every alphabet; a leftover is a
    // marker the rule compiler minted and failed to strip.
    let leftovers: BTreeSet<String> = alphabet
        .iter()
        .map(|s| s.to_string())
        .filter(|s| {
            s.starts_with('@') && s.ends_with('@') && ![EPSILON, UNKNOWN, IDENTITY].contains(&&**s)
        })
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

/// A reserved symbol pair is one arc on foma, not two.
///
/// `fsm_symbol` reads IDENTITY as foma's `?`, so crossing the two sides gave
/// `? .x. ?` — an UNKNOWN:UNKNOWN arc beside the intended IDENTITY:IDENTITY
/// one. Both sides then expanded independently as the alphabet grew, and
/// `expand-equivalences` over three words returned 1637 strings where the
/// tropical backend returned 5.
#[test]
fn a_reserved_symbol_pair_is_one_arc() -> Result<(), hfst::error::Error> {
    let id2id = FomaTransducer::define_transducer_symbol_pair(IDENTITY, IDENTITY);
    assert_eq!(
        arc_count(&id2id),
        1,
        "the identity pair must be a single arc: {:?}",
        id2id.to_basic()?.states_and_transitions()
    );

    // The ordinary path still goes through the cross product.
    let a2b = FomaTransducer::define_transducer_symbol_pair("a", "b");
    assert_eq!(arc_count(&a2b), 1, "an ordinary pair is a single arc");
    Ok(())
}

/// The affix guesser accepts the same language on foma as on tropical.
// [spec:hfst:sem:hfst-affix-guessify.process-stream-fn/test]
#[test]
fn affix_guesser_language_matches_tropical() -> Result<(), hfst::error::Error> {
    let _guard = serialized();

    let words = ["cat", "cats", "dog"];
    for direction in [GuessDirection::GuessSuffix, GuessDirection::GuessPrefix] {
        let mut f = fac_foma(&basic_acceptor(words[0]));
        let mut t = fac_trop(&basic_acceptor(words[0]));
        for w in &words[1..] {
            f.disjunct(&fac_foma(&basic_acceptor(w)), true)?;
            t.disjunct(&fac_trop(&basic_acceptor(w)), true)?;
        }
        f.minimize()?;
        t.minimize()?;

        let foma_guesser = affix_guessify(&f, direction, 1.0)?;
        // foma is unweighted, so only the LANGUAGE can be compared: routing the
        // tropical guesser through the interchange graph into foma is what
        // drops the affix-length ranking weights without touching the relation.
        let tropical_guesser: HfstTransducer<FomaTransducer> = HfstTransducer::new_from_basic(
            &ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&affix_guessify(
                &t, direction, 1.0,
            )?)?,
        )?;
        assert!(
            foma_guesser.compare(&tropical_guesser, true)?,
            "affix guesser diverges between foma and tropical"
        );
    }
    Ok(())
}
