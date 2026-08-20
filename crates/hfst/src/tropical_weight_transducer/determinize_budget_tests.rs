use super::determinize::AdaptiveDeterminize;
use super::*;

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

#[test]
fn subset_limit_preserves_input() {
    let mut input = fanout();
    let original = input.clone();
    let mut output = StdVectorFst::new();
    let outcome = TropicalWeightTransducer::determinize_adaptive(
        &mut input,
        false,
        (100, 4),
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
        Some((100, 4)),
    );
    assert!(TropicalWeightTransducer::are_equivalent(
        &input, &minimized, false
    ));
}
