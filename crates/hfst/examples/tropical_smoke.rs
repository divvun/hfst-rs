use hfst::tropical_weight_transducer::TropicalWeightTransducer as TWT;

fn main() {
    let eps = TWT::create_epsilon_transducer();
    assert!(TWT::number_of_states(&eps) >= 1);

    // a:b
    let ab = TWT::define_transducer_symbol_pair("a", "b");
    assert!(TWT::number_of_states(&ab) >= 1);

    // copy preserves state count
    let c = TWT::copy(&ab);
    assert_eq!(TWT::number_of_states(&c), TWT::number_of_states(&ab));

    // the OpenFST-algorithm wrappers run end to end
    let det = TWT::determinize(ab.clone(), false);
    assert!(TWT::number_of_states(&det) >= 1);
    let minz = TWT::minimize(ab.clone(), false);
    assert!(TWT::number_of_states(&minz) >= 1);
    let nb = TWT::n_best(&ab, 1);
    assert!(TWT::number_of_states(&nb) >= 1);

    println!(
        "tropical OK: eps={} ab={} det={} min={} nbest={}",
        TWT::number_of_states(&eps),
        TWT::number_of_states(&ab),
        TWT::number_of_states(&det),
        TWT::number_of_states(&minz),
        TWT::number_of_states(&nb)
    );
}
