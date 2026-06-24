use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;

fn main() {
    // t1 = [a:a]
    let mut t1 = HfstBasicTransducer::new();
    t1.add_transition(
        0,
        &HfstBasicTransition::new_symbols(1, "a".to_string(), "a".to_string(), 0.0),
        true,
    );
    t1.set_final_weight(1, &0.0);

    // t2 = [?:?]  (identity)
    let mut t2 = HfstBasicTransducer::new();
    t2.add_transition(
        0,
        &HfstBasicTransition::new_symbols(
            1,
            "@_IDENTITY_SYMBOL_@".to_string(),
            "@_IDENTITY_SYMBOL_@".to_string(),
            0.0,
        ),
        true,
    );
    t2.set_final_weight(1, &0.0);

    t1.harmonize(&mut t2);

    // t2's identity transition is expanded to also carry a:a (the symbol from t1).
    let found = t2
        .transitions(0)
        .iter()
        .any(|tr| tr.get_input_symbol() == "a" && tr.get_output_symbol() == "a");
    assert!(found, "identity not expanded with 'a'");
    println!(
        "harmonize OK (t2 state0 now has {} transitions)",
        t2.transitions(0).len()
    );
}
