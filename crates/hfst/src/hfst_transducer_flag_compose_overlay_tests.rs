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
fn one_sided_flags_become_only_the_opposite_overlay() {
    let left_flag = "@U.LEFT.VALUE@";
    let mut left = fixture(Some(left_flag), &[]);
    let mut right = fixture(None, &[]);
    let sizes = (size(&left), size(&right));

    let overlay = left
        .prepare_flag_diacritics_for_compose(&mut right)
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
        .prepare_flag_diacritics_for_compose(&mut right)
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
        .prepare_flag_diacritics_for_compose(&mut right)
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
        .prepare_flag_diacritics_for_compose(&mut right)
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
fn foma_backend_accepts_virtual_overlay() {
    use crate::backend_foma::FomaTransducer;

    let mut left =
        HfstTransducer::<FomaTransducer>::new_symbol("shared").expect("valid Foma left fixture");
    let right =
        HfstTransducer::<FomaTransducer>::new_symbol("shared").expect("valid Foma right fixture");
    let overlay = FlagDiacriticComposeOverlay::default();

    left.compose_with_config_and_flag_overlay(
        &right,
        true,
        &EngineConfig::default(),
        Some(&overlay),
    )
    .expect("Foma must accept the virtual-overlay compose path");

    assert!(left.is_cyclic().is_ok(), "composed Foma graph is queryable");
}
