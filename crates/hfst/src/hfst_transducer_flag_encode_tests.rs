//! Focused flag-diacritic encode/decode tests.

use super::*;
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_data_types::Symbol;
use hfst_openfst::StdVectorFst;
use hfst_openfst::prelude::*;

// Snapshot the tropical backend's alphabet as ordered (label, symbol) pairs.
// The in-place encode/decode is a divergence precisely because it keeps
// symbols at their original labels, so comparing this exact map (not just a
// string set) is what proves the round-trip is the *identity* on the
// symbol table.
fn symbol_pairs(t: &HfstTransducer<StdVectorFst>) -> Vec<(u32, String)> {
    let st = t
        .fst
        .input_symbols()
        .expect("tropical transducer always carries a symbol table");
    let mut v: Vec<(u32, String)> = st.iter().map(|(l, s)| (l, s.to_string())).collect();
    v.sort_by_key(|(l, _)| *l);
    v
}

// 0 -@U.F.FOO@-> 1 -a-> 2 -@P.F.BAR@-> 3 (final), plus an ordinary 'b' arc.
fn build_flagged() -> HfstTransducer<StdVectorFst> {
    let mut t = HfstBasicTransducer::new();
    let s1 = t.add_state_new();
    let s2 = t.add_state_new();
    let s3 = t.add_state_new();
    t.set_final_weight(s3, &0.0);

    let fd1 = Symbol::new_static("@U.F.FOO@");
    let fd2 = Symbol::new_static("@P.F.BAR@");

    let tr = HfstBasicTransition::new_symbols(s1, fd1.clone(), fd1.clone(), 0.0, t.coder_mut());
    t.add_transition(0, &tr, true);
    let tr = HfstBasicTransition::new_symbols(s2, "a".into(), "a".into(), 0.0, t.coder_mut());
    t.add_transition(s1, &tr, true);
    let tr = HfstBasicTransition::new_symbols(s2, "b".into(), "b".into(), 0.0, t.coder_mut());
    t.add_transition(s1, &tr, true);
    let tr = HfstBasicTransition::new_symbols(s3, fd2.clone(), fd2.clone(), 0.0, t.coder_mut());
    t.add_transition(s2, &tr, true);

    HfstTransducer::<StdVectorFst>::new_from_basic(&t)
        .expect("building a tropical transducer from a basic one cannot fail")
}

#[test]
fn flag_encode_decode_round_trip_restores_exactly() {
    let original = build_flagged();
    let before = symbol_pairs(&original);
    // The flags start in @...@ form.
    assert!(
        before.iter().any(|(_, s)| s == "@U.F.FOO@"),
        "expected @-wrapped flag before encode: {before:?}"
    );

    let mut t = original.clone();
    encode_flag_diacritics(&mut t);

    // After encode every flag is %-escaped at its ORIGINAL label; ordinary
    // symbols and epsilon are untouched.
    let encoded = symbol_pairs(&t);
    assert_eq!(
        before.len(),
        encoded.len(),
        "encode must not add or drop symbols"
    );
    for ((lb, sb), (le, se)) in before.iter().zip(encoded.iter()) {
        assert_eq!(lb, le, "labels must be preserved by the in-place rename");
        if sb == "@U.F.FOO@" {
            assert_eq!(se, "%U.F.FOO%");
        } else if sb == "@P.F.BAR@" {
            assert_eq!(se, "%P.F.BAR%");
        } else {
            assert_eq!(sb, se, "non-flag symbol {sb} must be unchanged");
        }
    }

    decode_flag_diacritics(&mut t);
    let after = symbol_pairs(&t);
    // Full equality: the symbol table (labels AND strings) is byte-for-byte
    // the original, and the automaton compares equal.
    assert_eq!(before, after, "decode must be the exact inverse of encode");
    assert!(
        original
            .compare_default(&t)
            .expect("compare on tropical transducers cannot fail"),
        "the round-tripped transducer must be equivalent to the original"
    );
}

#[test]
fn encoded_transducer_is_accepted_by_compose() {
    // Exercise the whole xerox-composition path (encode both operands ->
    // product -> decode) the way compose_with_config does: composing a
    // flagged acceptor with itself under xerox_composition must succeed and
    // stay equivalent to the flag-free compose.
    let t = build_flagged();
    let cfg = EngineConfig {
        xerox_composition: true,
        ..EngineConfig::default()
    };

    let mut xerox = t.clone();
    xerox
        .compose_with_config(&t, true, &cfg)
        .expect("xerox composition of a flagged acceptor must succeed");

    let mut plain = t.clone();
    plain
        .compose(&t, true)
        .expect("plain composition of a flagged acceptor must succeed");

    assert!(
        xerox
            .compare_default(&plain)
            .expect("compare on tropical transducers cannot fail"),
        "xerox-composition must agree with the plain compose for identity flags"
    );
}

#[test]
#[should_panic(expected = "reserved symbol")]
fn reserved_symbol_collision_panics() {
    // An alphabet symbol already wrapped in %...% whose @-unescaped form is a
    // flag diacritic collides with an encoded flag: encode must panic.
    let mut t = HfstBasicTransducer::new();
    let s1 = t.add_state_new();
    t.set_final_weight(s1, &0.0);
    let reserved = Symbol::new_static("%U.RESERVED.X%");
    let tr = HfstBasicTransition::new_symbols(
        s1,
        reserved.clone(),
        reserved.clone(),
        0.0,
        t.coder_mut(),
    );
    t.add_transition(0, &tr, true);
    let mut fst = HfstTransducer::<StdVectorFst>::new_from_basic(&t)
        .expect("building a tropical transducer from a basic one cannot fail");
    encode_flag_diacritics(&mut fst);
}

#[test]
#[should_panic(expected = "reserved symbol")]
// [spec:hfst:req:virtual-flag-algebra.special-compose/test]
fn virtual_xerox_preserves_collision_error() {
    let mut left = build_flagged();
    let mut basic = HfstBasicTransducer::new();
    let final_state = basic.add_state_new();
    basic.set_final_weight(final_state, &0.0);
    let reserved = Symbol::new_static("%U.F.FOO%");
    let transition = HfstBasicTransition::new_symbols(
        final_state,
        reserved.clone(),
        reserved,
        0.0,
        basic.coder_mut(),
    );
    basic.add_transition(0, &transition, true);
    let mut right = HfstTransducer::<StdVectorFst>::new_from_basic(&basic)
        .expect("valid reserved-symbol fixture");
    let overlay = left
        .prepare_flag_diacritics_for_compose(&mut right)
        .expect("virtual flag preparation");
    let config = EngineConfig {
        xerox_composition: true,
        ..EngineConfig::default()
    };
    let _ = left.compose_with_config_and_flag_overlay(&right, true, &config, Some(&overlay));
}
