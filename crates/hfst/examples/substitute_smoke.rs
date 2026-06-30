use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;

fn main() {
    // a:b --> substitute input a with x  => x:b
    let mut g = HfstBasicTransducer::new();
    let tr =
        HfstBasicTransition::new_symbols(1, "a".to_string(), "b".to_string(), 0.0, g.coder_mut());
    g.add_transition(0, &tr, true);
    g.substitute_symbol(&"a".to_string(), &"x".to_string(), true, false);
    let t = g.transitions(0);
    assert_eq!(t[0].get_input_symbol(g.coder()), "x");
    assert_eq!(t[0].get_output_symbol(g.coder()), "b");
    println!("substitute_symbol OK");

    // pair substitution: x:b -> c:d
    g.substitute_pair(
        &("x".to_string(), "b".to_string()),
        &("c".to_string(), "d".to_string()),
    );
    let t = g.transitions(0);
    // the first new pair both replaces and is appended (bug preserved) -> two arcs
    assert!(
        t.iter()
            .any(|tr| tr.get_input_symbol(g.coder()) == "c"
                && tr.get_output_symbol(g.coder()) == "d")
    );
    println!("substitute_pair OK ({} transition(s) at state 0)", t.len());

    // weight<->marker encoding round-trip
    let m = HfstBasicTransducer::weight2marker(0.5);
    let mut w = 0.0f32;
    assert!(HfstBasicTransducer::marker2weight(&m, &mut w));
    assert!((w - 0.5).abs() < 1e-6);
    println!("weight2marker/marker2weight OK ({m} -> {w})");

    // substitute a:b with a copy of another graph (p:q)
    let mut sub = HfstBasicTransducer::new();
    let tr =
        HfstBasicTransition::new_symbols(1, "p".to_string(), "q".to_string(), 0.0, sub.coder_mut());
    sub.add_transition(0, &tr, true);
    sub.set_final_weight(1, &0.0);
    let mut host = HfstBasicTransducer::new();
    let tr = HfstBasicTransition::new_symbols(
        1,
        "a".to_string(),
        "b".to_string(),
        0.0,
        host.coder_mut(),
    );
    host.add_transition(0, &tr, true);
    host.set_final_weight(1, &0.0);
    host.substitute_pair_with_graph(&("a".to_string(), "b".to_string()), &sub);
    // p:q now appears somewhere in the host's expanded graph
    let found = (0..=host.get_max_state()).any(|s| {
        host.transitions(s).iter().any(|tr| {
            tr.get_input_symbol(host.coder()) == "p" && tr.get_output_symbol(host.coder()) == "q"
        })
    });
    assert!(found, "substituting graph not inserted");
    println!(
        "substitute_pair_with_graph OK (max_state={})",
        host.get_max_state()
    );
}
