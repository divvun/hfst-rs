use std::sync::{Mutex, MutexGuard};

use hfst::backend::AlgebraBackend;
#[cfg(feature = "foma")]
use hfst::backend_foma::FomaTransducer;
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_data_types::{HfstTwoLevelPaths, Symbol};
use hfst::hfst_transducer::HfstTransducer;
use hfst_openfst::StdVectorFst;

const EPSILON: &str = "@_EPSILON_SYMBOL_@";
const IDENTITY: &str = "@_IDENTITY_SYMBOL_@";
const UNKNOWN: &str = "@_UNKNOWN_SYMBOL_@";
const LEFT_FLAG: &str = "@U.LEFT.VALUE@";
const RIGHT_FLAG: &str = "@P.RIGHT.VALUE@";

static SYMBOL_TABLE_LOCK: Mutex<()> = Mutex::new(());

fn serialized() -> MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn path(arcs: &[(&str, &str, f32)]) -> HfstBasicTransducer {
    let mut basic = HfstBasicTransducer::new();
    let mut source = 0;
    for &(input, output, weight) in arcs {
        let target = basic.add_state_new();
        let transition = HfstBasicTransition::new_symbols(
            target,
            Symbol::new(input),
            Symbol::new(output),
            weight,
            basic.coder_mut(),
        );
        basic.add_transition(source, &transition, true);
        source = target;
    }
    basic.set_final_weight(source, &0.0);
    basic
}

fn weighted_parallel_fixture() -> (HfstBasicTransducer, HfstBasicTransducer) {
    let mut left = HfstBasicTransducer::new();
    let left_body = left.add_state_new();
    let flag = HfstBasicTransition::new_symbols(
        left_body,
        Symbol::new(LEFT_FLAG),
        Symbol::new(LEFT_FLAG),
        0.0,
        left.coder_mut(),
    );
    left.add_transition(0, &flag, true);
    for weight in [1.0, 2.0] {
        let left_final = left.add_state_new();
        let transition = HfstBasicTransition::new_symbols(
            left_final,
            Symbol::new_static("a"),
            Symbol::new_static("b"),
            weight,
            left.coder_mut(),
        );
        left.add_transition(left_body, &transition, true);
        left.set_final_weight(left_final, &0.0);
    }

    let mut right = HfstBasicTransducer::new();
    for weight in [3.0, 4.0] {
        let right_final = right.add_state_new();
        let transition = HfstBasicTransition::new_symbols(
            right_final,
            Symbol::new_static("a"),
            Symbol::new_static("b"),
            weight,
            right.coder_mut(),
        );
        right.add_transition(0, &transition, true);
        right.set_final_weight(right_final, &0.0);
    }
    (left, right)
}

fn eager_and_virtual<B: AlgebraBackend>(
    left: &HfstBasicTransducer,
    right: &HfstBasicTransducer,
) -> (HfstTransducer<B>, HfstTransducer<B>) {
    let mut eager_left = HfstTransducer::<B>::new_from_basic(left).expect("valid left fixture");
    let mut eager_right = HfstTransducer::<B>::new_from_basic(right).expect("valid right fixture");
    let mut virtual_left = eager_left.clone();
    let mut virtual_right = eager_right.clone();

    eager_left
        .harmonize_flag_diacritics(&mut eager_right, true)
        .expect("eager flag harmonization");
    eager_left
        .intersect(&eager_right, true)
        .expect("eager intersection");

    let before = (
        virtual_left.number_of_states(),
        virtual_left.number_of_arcs(),
        virtual_right.number_of_states(),
        virtual_right.number_of_arcs(),
    );
    let overlay = virtual_left
        .prepare_flag_diacritics_for_operation(&mut virtual_right)
        .expect("virtual flag harmonization");
    assert_eq!(
        before,
        (
            virtual_left.number_of_states(),
            virtual_left.number_of_arcs(),
            virtual_right.number_of_states(),
            virtual_right.number_of_arcs(),
        ),
        "virtual preparation inserted physical transitions"
    );
    virtual_left
        .intersect_with_flag_overlay(&virtual_right, true, Some(&overlay))
        .expect("virtual intersection");

    assert!(
        virtual_left
            .compare(&eager_left, true)
            .expect("compare eager and virtual intersections"),
        "virtual intersection differs from eager harmonization"
    );
    (virtual_left, eager_left)
}

fn one_sided<B: AlgebraBackend>() {
    let left = path(&[(LEFT_FLAG, LEFT_FLAG, 0.0), ("a", "a", 0.0)]);
    let right = path(&[("a", "a", 0.0)]);
    let (actual, _) = eager_and_virtual::<B>(&left, &right);
    assert!(actual.number_of_arcs() >= 2);
}

fn two_sided<B: AlgebraBackend>() {
    let left = path(&[(LEFT_FLAG, LEFT_FLAG, 0.0), ("a", "a", 0.0)]);
    let right = path(&[(RIGHT_FLAG, RIGHT_FLAG, 0.0), ("a", "a", 0.0)]);
    let (actual, _) = eager_and_virtual::<B>(&left, &right);
    assert!(actual.number_of_arcs() >= 3);
}

fn epsilon_order<B: AlgebraBackend>() {
    let left = path(&[
        ("x", EPSILON, 0.0),
        (LEFT_FLAG, LEFT_FLAG, 0.0),
        ("a", "a", 0.0),
    ]);
    let right = path(&[
        (RIGHT_FLAG, RIGHT_FLAG, 0.0),
        ("x", EPSILON, 0.0),
        ("a", "a", 0.0),
    ]);
    let (actual, _) = eager_and_virtual::<B>(&left, &right);
    let mut paths = HfstTwoLevelPaths::new();
    actual
        .extract_paths(&mut paths, -1, -1)
        .expect("extract finite intersection paths");
    assert!(
        paths.is_empty(),
        "x:epsilon incorrectly reset two-sided flag ordering"
    );
}

fn alphabet_and_wildcards<B: AlgebraBackend>() {
    let left = path(&[(LEFT_FLAG, LEFT_FLAG, 0.0), ("a", "a", 0.0)]);
    let mut right = path(&[("a", "a", 0.0)]);
    for symbol in [IDENTITY, UNKNOWN, RIGHT_FLAG] {
        right.add_symbol_to_alphabet(&Symbol::new(symbol));
    }
    let identity = HfstBasicTransition::new_symbols(
        0,
        Symbol::new(IDENTITY),
        Symbol::new(IDENTITY),
        0.0,
        right.coder_mut(),
    );
    right.add_transition(0, &identity, true);

    let (actual, eager) = eager_and_virtual::<B>(&left, &right);
    assert_eq!(actual.number_of_arcs(), eager.number_of_arcs());
}

// [spec:hfst:req:virtual-flag-algebra.intersection/test]
#[test]
fn tropical_intersection_matrix() {
    let _guard = serialized();
    one_sided::<StdVectorFst>();
    two_sided::<StdVectorFst>();
    epsilon_order::<StdVectorFst>();
    alphabet_and_wildcards::<StdVectorFst>();

    let (left, right) = weighted_parallel_fixture();
    let (actual, eager) = eager_and_virtual::<StdVectorFst>(&left, &right);
    assert_eq!(actual.number_of_arcs(), eager.number_of_arcs());
    assert_eq!(
        actual.number_of_arcs(),
        5,
        "one virtual flag arc plus four weighted parallel matches must survive"
    );
}

#[cfg(feature = "foma")]
// [spec:hfst:req:virtual-flag-algebra.intersection/test]
#[test]
fn foma_intersection_matrix() {
    let _guard = serialized();
    one_sided::<FomaTransducer>();
    two_sided::<FomaTransducer>();
    epsilon_order::<FomaTransducer>();
    alphabet_and_wildcards::<FomaTransducer>();
}
