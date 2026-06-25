use hfst::log_weight_transducer::LogWeightTransducer as LWT;

fn main() {
    let eps = LWT::create_epsilon_transducer();
    assert!(LWT::number_of_states(&eps) >= 1);

    // a:b
    let ab = LWT::define_transducer_symbol_pair("a", "b");
    assert!(LWT::number_of_states(&ab) >= 1);

    // copy preserves state count
    let c = LWT::copy(&ab);
    assert_eq!(LWT::number_of_states(&c), LWT::number_of_states(&ab));

    // the OpenFST-algorithm wrappers run end to end over the log semiring
    let det = LWT::determinize(&ab);
    assert!(LWT::number_of_states(&det) >= 1);
    let minz = LWT::minimize(&ab);
    assert!(LWT::number_of_states(&minz) >= 1);
    let nx = LWT::remove_epsilons(&ab);
    assert!(LWT::number_of_states(&nx) >= 1);

    println!(
        "log OK: eps={} ab={} det={} min={} rmeps={}",
        LWT::number_of_states(&eps),
        LWT::number_of_states(&ab),
        LWT::number_of_states(&det),
        LWT::number_of_states(&minz),
        LWT::number_of_states(&nx)
    );
}
