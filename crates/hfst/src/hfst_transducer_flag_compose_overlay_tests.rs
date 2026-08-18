//! Focused virtual flag-overlay preparation and validation tests.

use super::*;

fn symbol_set(symbols: &[&str]) -> StringSet {
    symbols.iter().map(Symbol::new).collect()
}

fn fixture(arc_flag: Option<&str>, alphabet_only_flags: &[&str]) -> HfstTransducer<StdVectorFst> {
    let mut basic = HfstBasicTransducer::new();
    if let Some(flag) = arc_flag {
        let target = basic.add_state_new();
        let flag = Symbol::new(flag);
        let tr =
            HfstBasicTransition::new_symbols(target, flag.clone(), flag, 0.0, basic.coder_mut());
        basic.add_transition(0, &tr, true);
        basic.set_final_weight(target, &0.0);
    } else {
        let target = basic.add_state_new();
        let tr = HfstBasicTransition::new_symbols(
            target,
            Symbol::new_static("ordinary"),
            Symbol::new_static("ordinary"),
            0.0,
            basic.coder_mut(),
        );
        basic.add_transition(0, &tr, true);
        basic.set_final_weight(target, &0.0);
    }
    for flag in alphabet_only_flags {
        basic.add_symbol_to_alphabet(&Symbol::new(flag));
    }
    HfstTransducer::new_from_basic(&basic).expect("valid tropical fixture")
}

fn size(fst: &HfstTransducer<StdVectorFst>) -> (u32, u32) {
    (fst.number_of_states(), fst.number_of_arcs())
}

#[test]
// [spec:hfst:req:virtual-flag-algebra.materialized-reference/test]
// [spec:hfst:req:virtual-flag-algebra.backend-core/test]
fn one_sided_flags_become_only_the_opposite_overlay() {
    let left_flag = "@U.LEFT.VALUE@";
    let mut left = fixture(Some(left_flag), &[]);
    let mut right = fixture(None, &[]);
    let sizes = (size(&left), size(&right));

    let overlay = left
        .prepare_flag_diacritics_for_operation(&mut right)
        .expect("one-sided left overlay preparation");
    assert!(overlay.left_self_loops.is_empty());
    assert_eq!(overlay.right_self_loops, symbol_set(&[left_flag]));
    assert!(!overlay.enforce_left_before_right);
    assert_eq!((size(&left), size(&right)), sizes);
    assert!(right.get_alphabet().unwrap().contains(left_flag));

    let right_flag = "@P.RIGHT.VALUE@";
    let mut left = fixture(None, &[]);
    let mut right = fixture(Some(right_flag), &[]);
    let sizes = (size(&left), size(&right));

    let overlay = left
        .prepare_flag_diacritics_for_operation(&mut right)
        .expect("one-sided right overlay preparation");
    assert_eq!(overlay.left_self_loops, symbol_set(&[right_flag]));
    assert!(overlay.right_self_loops.is_empty());
    assert!(!overlay.enforce_left_before_right);
    assert_eq!((size(&left), size(&right)), sizes);
    assert!(left.get_alphabet().unwrap().contains(right_flag));
}

#[test]
fn both_sides_rename_to_disjoint_overlays() {
    let mut left = fixture(Some("@U.FEATURE.LEFT@"), &[]);
    let mut right = fixture(Some("@P.FEATURE.RIGHT@"), &[]);
    let sizes = (size(&left), size(&right));

    let overlay = left
        .prepare_flag_diacritics_for_operation(&mut right)
        .expect("two-sided overlay preparation");
    let renamed_left = "@U.FEATURE_1.LEFT@";
    let renamed_right = "@P.FEATURE_2.RIGHT@";
    assert_eq!(overlay.left_self_loops, symbol_set(&[renamed_right]));
    assert_eq!(overlay.right_self_loops, symbol_set(&[renamed_left]));
    assert!(
        overlay
            .left_self_loops
            .is_disjoint(&overlay.right_self_loops)
    );
    assert!(overlay.enforce_left_before_right);
    assert_eq!((size(&left), size(&right)), sizes);

    for alphabet in [left.get_alphabet().unwrap(), right.get_alphabet().unwrap()] {
        assert!(alphabet.contains(renamed_left));
        assert!(alphabet.contains(renamed_right));
        assert!(!alphabet.contains("@U.FEATURE.LEFT@"));
        assert!(!alphabet.contains("@P.FEATURE.RIGHT@"));
    }
}

#[test]
fn alphabet_only_flags_form_overlay() {
    let mut left = fixture(None, &["@U.ALPHA.LEFT@"]);
    let mut right = fixture(None, &["@R.BETA.RIGHT@"]);
    let sizes = (size(&left), size(&right));

    let overlay = left
        .prepare_flag_diacritics_for_operation(&mut right)
        .expect("alphabet-only overlay preparation");
    let renamed_left = "@U.ALPHA_1.LEFT@";
    let renamed_right = "@R.BETA_2.RIGHT@";
    assert_eq!(overlay.left_self_loops, symbol_set(&[renamed_right]));
    assert_eq!(overlay.right_self_loops, symbol_set(&[renamed_left]));
    assert!(overlay.enforce_left_before_right);
    assert_eq!((size(&left), size(&right)), sizes);

    for alphabet in [left.get_alphabet().unwrap(), right.get_alphabet().unwrap()] {
        assert!(alphabet.contains(renamed_left));
        assert!(alphabet.contains(renamed_right));
    }
}

#[test]
fn special_modes_reject_overlay_before_mutation() {
    for (config, expected_message) in [
        (
            EngineConfig {
                flag_is_epsilon_in_composition: true,
                ..EngineConfig::default()
            },
            "flag-is-epsilon",
        ),
        (
            EngineConfig {
                xerox_composition: true,
                ..EngineConfig::default()
            },
            "xerox composition",
        ),
    ] {
        let mut left = fixture(Some("@U.LEFT.VALUE@"), &[]);
        let mut right = fixture(None, &[]);
        let overlay = left
            .prepare_flag_diacritics_for_compose(&mut right)
            .expect("overlay preparation");
        left.is_trie = true;
        let before = left.fst.clone();

        let error = left
            .compose_with_config_and_flag_overlay(&right, true, &config, Some(&overlay))
            .err()
            .expect("incompatible overlay configuration must fail");

        assert!(error.to_string().contains(expected_message), "{error}");
        assert_eq!(left.fst, before, "validation mutated the left graph");
        assert!(left.is_trie, "validation mutated facade metadata");
    }
}

#[cfg(feature = "foma")]
#[test]
// [spec:hfst:req:foma-transducer.hfst.implementations.foma-transducer.resource-controlled-compose/test]
// [spec:hfst:req:virtual-flag-algebra.backend-core/test]
fn foma_backend_accepts_virtual_overlay() {
    use crate::backend_foma::FomaTransducer;

    let path = |flag: &str| {
        let mut basic = HfstBasicTransducer::new();
        let flag_state = basic.add_state_new();
        let flag = Symbol::new(flag);
        let flag_transition = HfstBasicTransition::new_symbols(
            flag_state,
            flag.clone(),
            flag,
            0.0,
            basic.coder_mut(),
        );
        basic.add_transition(0, &flag_transition, true);

        let final_state = basic.add_state_new();
        let ordinary = Symbol::new_static("shared");
        let ordinary_transition = HfstBasicTransition::new_symbols(
            final_state,
            ordinary.clone(),
            ordinary,
            0.0,
            basic.coder_mut(),
        );
        basic.add_transition(flag_state, &ordinary_transition, true);
        basic.set_final_weight(final_state, &0.0);
        HfstTransducer::<FomaTransducer>::new_from_basic(&basic).expect("valid Foma flag path")
    };

    let mut virtual_left = path("@U.LEFT.VALUE@");
    let mut virtual_right = path("@P.RIGHT.VALUE@");
    let mut eager_left = virtual_left.clone();
    let mut eager_right = virtual_right.clone();

    eager_left
        .harmonize_flag_diacritics(&mut eager_right, true)
        .expect("eager two-sided Foma harmonization");
    eager_left
        .compose(&eager_right, true)
        .expect("eager two-sided Foma composition");

    let sizes = (
        virtual_left.number_of_arcs(),
        virtual_right.number_of_arcs(),
    );
    let overlay = virtual_left
        .prepare_flag_diacritics_for_compose(&mut virtual_right)
        .expect("virtual two-sided Foma harmonization");
    assert!(overlay.enforce_left_before_right);
    assert_eq!(
        (
            virtual_left.number_of_arcs(),
            virtual_right.number_of_arcs(),
        ),
        sizes,
        "overlay preparation inserted physical flag loops"
    );

    virtual_left
        .compose_with_config_and_flag_overlay(
            &virtual_right,
            true,
            &EngineConfig::default(),
            Some(&overlay),
        )
        .expect("Foma must accept the two-sided virtual-overlay path");

    assert!(
        virtual_left
            .compare(&eager_left, true)
            .expect("compare eager and virtual Foma results"),
        "two-sided virtual Foma composition differs from eager harmonization"
    );
}
