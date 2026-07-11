// Regression tests for HfstTransducer::priority_union (Q .P. R).
//
// priority_union realizes  Q .P. R = Q | [ ~[Q.u] .o. R ] : every input Q
// maps stays; for inputs Q does NOT cover, R's mapping is taken instead.
//
// Two concerns are pinned here:
//
//  1. The (already-fixed) hfst#341 behaviour: a weighted higher-priority Q map
//     wins over R's lower-priority map for the SAME input, while R's
//     input-disjoint maps survive.
//
//  2. The flag-diacritic LEAK (shared with upstream C++, fixed deliberately in
//     our port — see the DIVERGENCE comment in priority_union). When Q carries
//     a flag diacritic on an input word, input_project() keeps the flag as a
//     literal arc and negate() then treats it as an ordinary symbol, so the
//     FLAGLESS string Q actually accepts falls outside ~[Q.u], lands inside the
//     complement, and R's lower-priority mapping for that same flagless string
//     LEAKS through — the string ends up mapped TWICE (Q's weight and R's
//     weight). Our priority_union resolves the flags on the input projection
//     (eliminate_flags) before the complement, so the leak cannot occur.
//
// All transducers are built directly as HfstBasicTransducers so weights and
// flag arcs are placed exactly; only the in-scope tropical backend is used.

use std::collections::BTreeSet;

use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_data_types::HfstTwoLevelPaths;
use hfst::hfst_transducer::HfstTransducer;
use hfst_openfst::StdVectorFst;

// The tropical transition-data symbol coding lives in process-global statics
// guarded by Mutexes that race under cargo's parallel #[test] threads (see the
// long note in test_flag_diacritics.rs). Serialize through one lock.
static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

// Build a linear transducer for one input:output symbol-pair sequence with the
// whole path weight placed on the final state. `pairs` is a slice of
// (input_symbol, output_symbol); a flag diacritic is just an input==output
// symbol like "@U.F.T@".
fn linear(pairs: &[(&str, &str)], weight: f32) -> HfstTransducer<StdVectorFst> {
    let mut t = HfstBasicTransducer::new();
    let mut state = 0u32;
    for (i, o) in pairs {
        let next = t.add_state_new();
        let tr =
            HfstBasicTransition::new_symbols(next, (*i).into(), (*o).into(), 0.0, t.coder_mut());
        t.add_transition(state, &tr, true);
        state = next;
    }
    t.set_final_weight(state, &weight);
    HfstTransducer::<StdVectorFst>::new_from_basic(&t)
        .expect("linear: basic -> tropical conversion")
}

// Disjunct a list of linear transducers into one.
fn union_of(parts: Vec<HfstTransducer<StdVectorFst>>) -> HfstTransducer<StdVectorFst> {
    let mut it = parts.into_iter();
    let mut acc = it.next().expect("union_of: at least one part");
    for p in it {
        acc.disjunct(&p, true).expect("union_of: disjunct");
    }
    acc.optimize().expect("union_of: optimize");
    acc
}

// Extract every (istring, ostring, weight) triple. Flags are filtered out of the
// surface strings (filter_fd = true) so the flagless language is what we compare.
fn triples(t: &HfstTransducer<StdVectorFst>) -> Vec<(String, String, f32)> {
    let mut results: HfstTwoLevelPaths = BTreeSet::new();
    t.extract_paths_fd(&mut results, -1, -1, true)
        .expect("extract_paths_fd");
    let mut out = Vec::new();
    for path in results.iter() {
        let mut istring = String::new();
        let mut ostring = String::new();
        for (i, o) in path.second.iter() {
            // Epsilon symbols carry no surface material; skip them so the
            // compared strings are the flagless surface language.
            if i != hfst::hfst_symbol_defs::INTERNAL_EPSILON {
                istring.push_str(i);
            }
            if o != hfst::hfst_symbol_defs::INTERNAL_EPSILON {
                ostring.push_str(o);
            }
        }
        out.push((istring, ostring, path.first));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.total_cmp(&b.2)));
    out
}

// (a) hfst#341 fixed behaviour: weighted Q {a:x/3} .P. R {a:x/15, b:y/15}
// must yield exactly {a:x/3, b:y/15} — Q's higher-priority weight wins for the
// shared input "a"; R's input-disjoint "b" survives.
#[test]
fn priority_union_weighted_341() {
    let _g = serialized();

    let mut q = linear(&[("a", "x")], 3.0);
    let r = union_of(vec![
        linear(&[("a", "x")], 15.0),
        linear(&[("b", "y")], 15.0),
    ]);

    q.priority_union(&r).expect("priority_union");

    let got = triples(&q);
    assert_eq!(
        got,
        vec![
            ("a".to_string(), "x".to_string(), 3.0),
            ("b".to_string(), "y".to_string(), 15.0),
        ],
        "Q's weight must win for shared input a; R's disjoint input b must survive"
    );
}

// (b) Right-side-only compound survives when Q carries flags. Q maps a
// flag-input "lea" to LEA; R additionally maps "biila" to BIILA (an input Q
// does not cover). "biila" -> BIILA must survive in the result.
#[test]
fn priority_union_right_side_only_survives_with_flags() {
    let _g = serialized();

    let mut q = linear(
        &[("@U.F.T@", "@U.F.T@"), ("l", "L"), ("e", "E"), ("a", "A")],
        3.0,
    );
    let r = union_of(vec![
        linear(&[("l", "L"), ("e", "E"), ("a", "A")], 15.0),
        linear(
            &[("b", "B"), ("i", "I"), ("i", "I"), ("l", "L"), ("a", "A")],
            0.0,
        ),
    ]);

    q.priority_union(&r).expect("priority_union");

    let got = triples(&q);
    assert!(
        got.iter().any(|(i, o, _)| i == "biila" && o == "BIILA"),
        "right-side-only compound biila->BIILA must survive priority_union; got {got:?}"
    );
}

// (c) THE LEAK. Q maps flag-input "lea" (@U.F.T@ l e a) to LEA with weight 3;
// R maps flagless "lea" to LEA with weight 15. After flags are filtered, both Q
// and R accept the SAME flagless input "lea". priority_union must keep exactly
// ONE lea mapping, carrying Q's weight (3) — never a second copy with R's
// weight (15). Before the fix, input_project kept the flag literal, negate let
// the flagless "lea" leak into the complement, and R's 15-weight copy came
// through too.
#[test]
fn priority_union_flag_input_does_not_leak() {
    let _g = serialized();

    let mut q = linear(
        &[("@U.F.T@", "@U.F.T@"), ("l", "L"), ("e", "E"), ("a", "A")],
        3.0,
    );
    let r = linear(&[("l", "L"), ("e", "E"), ("a", "A")], 15.0);

    q.priority_union(&r).expect("priority_union");

    let lea: Vec<_> = triples(&q)
        .into_iter()
        .filter(|(i, o, _)| i == "lea" && o == "LEA")
        .collect();

    assert_eq!(
        lea.len(),
        1,
        "flagless lea must map exactly once (no R leak); got {lea:?}"
    );
    assert_eq!(
        lea[0].2, 3.0,
        "the surviving lea mapping must carry Q's higher-priority weight 3, not R's 15; got {lea:?}"
    );
}

// (d) lenient_composition smoke with flags. lenient_composition(self, R) =
// [self .o. R] .P. self : the composition takes precedence where it is defined,
// falling back to self elsewhere (priority_union with self as the priority arg).
//
// `self` carries a flag diacritic on its INPUT side (output epsilon) and maps
// the flagless surface input "lea" -> "LEA" with weight 3. R further rewrites
// "LEA" -> "XYZ" with weight 5, so the composition self .o. R IS defined on
// input "lea" (output "XYZ", weight 8). lenient_composition must therefore let
// the composed mapping take precedence (priority arg is `self`, so composition
// -defined inputs keep the composed output). Because the flag lives on self's
// input, priority_union's flag resolution applies here too: the flagless "lea"
// must yield exactly ONE mapping (no leaked fallback duplicate from the flag
// mishandling). We assert the composed output wins and appears exactly once.
#[test]
fn lenient_composition_smoke_with_flags() {
    let _g = serialized();

    // self: @U.F.T@:eps l:L e:E a:A  -> flagless input "lea" maps to "LEA", w3
    let mut lhs = linear(
        &[
            ("@U.F.T@", "@_EPSILON_SYMBOL_@"),
            ("l", "L"),
            ("e", "E"),
            ("a", "A"),
        ],
        3.0,
    );
    // R: L:X E:Y A:Z  -> rewrites "LEA" -> "XYZ", w5
    let r = linear(&[("L", "X"), ("E", "Y"), ("A", "Z")], 5.0);

    lhs.lenient_composition(&r, true)
        .expect("lenient_composition");

    let lea: Vec<_> = triples(&lhs)
        .into_iter()
        .filter(|(i, _, _)| i == "lea")
        .collect();

    // self .o. R is defined on "lea" (LEA matches R's input), so the composed
    // output XYZ must win over self's fallback LEA, and the flag on self's input
    // must produce exactly one flagless "lea" entry (no leaked fallback).
    assert_eq!(
        lea.len(),
        1,
        "flag-input lea must yield exactly one mapping (composition wins, no fallback duplicate, no leak); got {lea:?}"
    );
    assert_eq!(
        lea[0].1, "XYZ",
        "where the composition is defined its output XYZ must take precedence; got {lea:?}"
    );
    assert_eq!(
        lea[0].2, 8.0,
        "the composed mapping must carry the summed weight 3+5=8; got {lea:?}"
    );
}
