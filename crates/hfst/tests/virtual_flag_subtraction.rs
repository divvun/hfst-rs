use std::sync::{Mutex, MutexGuard};

use hfst::backend::AlgebraBackend;
#[cfg(feature = "foma")]
use hfst::backend_foma::FomaTransducer;
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_data_types::Symbol;
use hfst::hfst_transducer::HfstTransducer;
use hfst_openfst::StdVectorFst;

const EPSILON: &str = "@_EPSILON_SYMBOL_@";
const FLAG: &str = "@P.FEATURE.VALUE@";
const ALPHABET_FLAG: &str = "@U.ALPHABET.VALUE@";
const CLEAR_FLAG: &str = "@C.Num@";

static SYMBOL_TABLE_LOCK: Mutex<()> = Mutex::new(());

type ArcSpec<'a> = (&'a str, &'a str, f32);
type BranchSpec<'a> = (&'a [ArcSpec<'a>], f32);

fn serialized() -> MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn paths(branches: &[BranchSpec<'_>]) -> HfstBasicTransducer {
    let mut basic = HfstBasicTransducer::new();
    for &(arcs, final_weight) in branches {
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
        basic.set_final_weight(source, &final_weight);
    }
    basic
}

fn assert_eager_virtual_parity<B: AlgebraBackend>(
    left: &HfstBasicTransducer,
    right: &HfstBasicTransducer,
) {
    let eager_left = HfstTransducer::<B>::new_from_basic(left).expect("valid left fixture");
    let eager_right = HfstTransducer::<B>::new_from_basic(right).expect("valid right fixture");
    assert_eager_virtual_transducers(eager_left, eager_right);
}

fn assert_eager_virtual_transducers<B: AlgebraBackend>(
    mut eager_left: HfstTransducer<B>,
    mut eager_right: HfstTransducer<B>,
) {
    let mut virtual_left = eager_left.clone();
    let mut virtual_right = eager_right.clone();

    eager_left
        .harmonize_flag_diacritics(&mut eager_right, true)
        .expect("eager flag harmonization");
    eager_left
        .subtract(&eager_right, true)
        .expect("eager subtraction");

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
        .subtract_with_flag_overlay(&virtual_right, true, Some(&overlay))
        .expect("virtual subtraction");

    assert!(
        virtual_left
            .compare(&eager_left, true)
            .expect("compare eager and virtual subtraction"),
        "virtual subtraction differs from eager harmonization"
    );
}

fn matrix<B: AlgebraBackend>() {
    let left = paths(&[
        (&[(FLAG, FLAG, 0.0), ("a", "a", 1.0)], 2.0),
        (&[(FLAG, FLAG, 0.0), ("b", "b", 3.0)], 4.0),
    ]);
    let right = paths(&[(&[(FLAG, FLAG, 0.0), ("a", "a", 0.0)], 0.0)]);
    assert_eager_virtual_parity::<B>(&left, &right);

    // Clear flags have no value component. The two-sided ordering pass must
    // classify the renamed `@C.Num_1@` form just like a valued `_1` flag.
    let left = paths(&[(&[(CLEAR_FLAG, CLEAR_FLAG, 0.0)], 0.0)]);
    let right = paths(&[(&[(FLAG, FLAG, 0.0), ("a", "a", 0.0)], 0.0)]);
    assert_eager_virtual_parity::<B>(&left, &right);

    // True output epsilon must not reset the left-before-right ordering state.
    let left = paths(&[(
        &[("x", EPSILON, 0.0), (FLAG, FLAG, 0.0), ("a", "a", 0.0)],
        0.0,
    )]);
    let right = paths(&[(
        &[(FLAG, FLAG, 0.0), ("x", EPSILON, 0.0), ("a", "a", 0.0)],
        0.0,
    )]);
    assert_eager_virtual_parity::<B>(&left, &right);

    // A flag declared only in the right alphabet still causes an eager loop
    // in the left operand and therefore belongs to subtraction's complement
    // alphabet.
    let left = paths(&[
        (&[(FLAG, FLAG, 0.0), ("a", "a", 0.0)], 0.0),
        (&[(FLAG, FLAG, 0.0), ("b", "b", 0.0)], 0.0),
    ]);
    let right = paths(&[(&[(FLAG, FLAG, 0.0), ("a", "a", 0.0)], 0.0)]);
    let left = HfstTransducer::<B>::new_from_basic(&left).expect("valid alphabet left fixture");
    let mut right =
        HfstTransducer::<B>::new_from_basic(&right).expect("valid alphabet right fixture");
    right
        .insert_to_alphabet(ALPHABET_FLAG)
        .expect("insert alphabet-only right flag");
    assert_eager_virtual_transducers(left, right);
}

// [spec:hfst:req:virtual-flag-algebra.subtraction/test]
#[test]
fn tropical_subtraction_matches_eager_harmonization() {
    let _guard = serialized();
    matrix::<StdVectorFst>();
}

#[cfg(feature = "foma")]
// [spec:hfst:req:virtual-flag-algebra.subtraction/test]
#[test]
fn foma_subtraction_matches_eager_harmonization() {
    let _guard = serialized();
    matrix::<FomaTransducer>();
}
