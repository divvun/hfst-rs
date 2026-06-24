use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;

fn main() {
    // a:b --> substitute input a with x  => x:b
    let mut g = HfstBasicTransducer::new();
    g.add_transition(
        0,
        &HfstBasicTransition::new_symbols(1, "a".to_string(), "b".to_string(), 0.0),
        true,
    );
    g.substitute_symbol(&"a".to_string(), &"x".to_string(), true, false);
    let t = g.transitions(0);
    assert_eq!(t[0].get_input_symbol(), "x");
    assert_eq!(t[0].get_output_symbol(), "b");
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
            .any(|tr| tr.get_input_symbol() == "c" && tr.get_output_symbol() == "d")
    );
    println!("substitute_pair OK ({} transition(s) at state 0)", t.len());
}
