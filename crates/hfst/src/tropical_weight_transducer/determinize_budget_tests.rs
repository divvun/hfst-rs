use super::determinize::{AdaptiveDeterminize, DeterminizeBudget};
use super::*;

fn budget(states: usize, subset_elements: usize, trs: usize) -> DeterminizeBudget {
    DeterminizeBudget {
        states,
        subset_elements,
        trs,
    }
}

fn fanout() -> StdVectorFst {
    let mut input = StdVectorFst::new();
    let start = input.add_state();
    input.set_start(start).unwrap();
    for weight in [0.0, 1.0, 2.0, 3.0] {
        let target = input.add_state();
        input.set_final(target, TropicalWeight::one()).unwrap();
        input
            .add_tr(
                start,
                StdTransition::new(1, 1, TropicalWeight::new(weight), target),
            )
            .unwrap();
    }
    input
}

/// The shape that blows determinization up, in miniature: a sparse chain
/// unioned with a dense hub that accepts the whole alphabet at every step, both
/// live from the start state. Every determinized state pairs one chain state
/// with the hub and so inherits the hub's out-degree — transitions multiply
/// while the state count barely moves, which is exactly what a state budget and
/// a subset budget cannot see. It is the Giella speller's lexicon-unioned-with-
/// error-model at a scale that fits in a unit test.
///
/// Transition weights are uniform on purpose: weight encoding folds the weight
/// into the label, so branches that differ in weight never share a determinized
/// state and the inflation this fixture exists to produce would not happen. The
/// differing final weights keep it a weighted machine.
fn sparse_chain_union_dense_hub(chain_len: u32, alphabet: u32) -> StdVectorFst {
    const STEP: u32 = 1;
    let mut input = StdVectorFst::new();
    let start = input.add_state();
    input.set_start(start).unwrap();

    let mut previous = start;
    for _ in 0..chain_len {
        let next = input.add_state();
        input
            .add_tr(previous, StdTransition::new(STEP, STEP, 0.0, next))
            .unwrap();
        previous = next;
    }
    input.set_final(previous, TropicalWeight::one()).unwrap();

    let hub = input.add_state();
    input.set_final(hub, TropicalWeight::new(1.0)).unwrap();
    for label in 1..=alphabet {
        input
            .add_tr(start, StdTransition::new(label, label, 0.0, hub))
            .unwrap();
        input
            .add_tr(hub, StdTransition::new(label, label, 0.0, hub))
            .unwrap();
    }
    input
}

#[test]
fn subset_limit_preserves_input() {
    let mut input = fanout();
    let original = input.clone();
    let mut output = StdVectorFst::new();
    let outcome = TropicalWeightTransducer::determinize_adaptive(
        &mut input,
        false,
        budget(100, 4, usize::MAX),
        "test",
        &mut output,
        true,
    );

    assert!(matches!(outcome, AdaptiveDeterminize::SubsetLimit));
    assert_eq!(input, original);
    assert_eq!(output.num_states(), 0);
}

#[test]
fn reverse_fallback_preserves_language() {
    let input = fanout();
    let minimized = TropicalWeightTransducer::minimize_with_reverse_fallback(
        input.clone(),
        false,
        true,
        Some(budget(100, 4, usize::MAX)),
    );
    assert!(TropicalWeightTransducer::are_equivalent(
        &input, &minimized, false
    ));
}

// The weight-encoding strategy is the last one tried, so an unbounded one made
// every other budget decorative: overruns funnelled straight into it.
// [spec:hfst:req:determinize-envelope.bounded-strategies/test]
#[test]
fn encoded_weight_strategy_honours_the_budget() {
    let mut input = fanout();
    let original = input.clone();
    let mut output = StdVectorFst::new();
    let outcome = TropicalWeightTransducer::determinize_adaptive(
        &mut input,
        true,
        budget(1, 4, usize::MAX),
        "test",
        &mut output,
        true,
    );

    assert!(matches!(outcome, AdaptiveDeterminize::SubsetLimit));
    assert_eq!(
        input, original,
        "an aborted strategy must decode the machine back to its input form"
    );
}

// A state budget and a subset budget both see this determinization as cheap;
// only counting the transitions it writes catches it.
// [spec:hfst:req:determinize-envelope.transition-axis/test]
#[test]
fn transition_budget_catches_what_the_other_axes_miss() {
    let input = sparse_chain_union_dense_hub(64, 64);
    let mut encoded = input.clone();
    let mut output = StdVectorFst::new();
    let unconstrained_axes = budget(usize::MAX, usize::MAX, usize::MAX);
    let outcome = TropicalWeightTransducer::determinize_adaptive(
        &mut encoded,
        true,
        unconstrained_axes,
        "test",
        &mut output,
        true,
    );
    let AdaptiveDeterminize::Determinized(_) = outcome else {
        panic!("with every axis unconstrained this input determinizes")
    };
    let determinized_trs = TropicalWeightTransducer::number_of_arcs(&output);
    let input_trs = TropicalWeightTransducer::number_of_arcs(&input);
    assert!(
        determinized_trs > 4 * input_trs,
        "the fixture must actually inflate: {input_trs} in, {determinized_trs} out"
    );
    assert!(
        output.num_states() < 4 * input.num_states(),
        "and must do it without tripping a state budget"
    );

    let mut bounded_input = input.clone();
    let mut bounded_output = StdVectorFst::new();
    let outcome = TropicalWeightTransducer::determinize_adaptive(
        &mut bounded_input,
        true,
        budget(usize::MAX, usize::MAX, input_trs as usize),
        "test",
        &mut bounded_output,
        true,
    );
    assert!(
        matches!(outcome, AdaptiveDeterminize::SubsetLimit),
        "the transition axis must stop it"
    );
    assert_eq!(bounded_input, input);
}

// The union that motivated the envelope denotes the same relation whether or
// not minimization is allowed to finish — the whole point of being free to
// abandon it.
// [spec:hfst:req:determinize-envelope.relation-preserved/test]
#[test]
fn transition_budget_preserves_the_relation() {
    let input = sparse_chain_union_dense_hub(24, 16);
    let input_trs = TropicalWeightTransducer::number_of_arcs(&input) as usize;

    let unbounded = TropicalWeightTransducer::minimize_with_reverse_fallback(
        input.clone(),
        true,
        false,
        Some(budget(usize::MAX, usize::MAX, usize::MAX)),
    );
    let stopped = TropicalWeightTransducer::minimize_with_reverse_fallback(
        input.clone(),
        true,
        false,
        Some(budget(usize::MAX, usize::MAX, input_trs)),
    );

    assert!(TropicalWeightTransducer::are_equivalent(
        &input, &unbounded, false
    ));
    assert!(TropicalWeightTransducer::are_equivalent(
        &input, &stopped, false
    ));
    assert!(
        TropicalWeightTransducer::number_of_arcs(&stopped)
            < TropicalWeightTransducer::number_of_arcs(&unbounded),
        "stopping early is what makes this worth doing"
    );
}

// Widening or adding an axis must never perturb a compilation that already fit.
// [spec:hfst:req:determinize-envelope.transition-axis/test]
#[test]
fn a_generous_transition_budget_is_byte_identical() {
    let input = sparse_chain_union_dense_hub(24, 16);
    let unbounded = TropicalWeightTransducer::minimize_with_reverse_fallback(
        input.clone(),
        false,
        true,
        Some(budget(usize::MAX, usize::MAX, usize::MAX)),
    );
    let bounded = TropicalWeightTransducer::minimize_with_reverse_fallback(
        input,
        false,
        true,
        Some(budget(usize::MAX, usize::MAX, 1 << 30)),
    );
    assert_eq!(unbounded, bounded);
}
