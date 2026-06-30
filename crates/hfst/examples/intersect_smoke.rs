use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;

fn arc(g: &mut HfstBasicTransducer, from: u32, to: u32, i: &str, o: &str) {
    let tr = HfstBasicTransition::new_symbols(to, i.to_string(), o.to_string(), 0.0, g.coder_mut());
    g.add_transition(from, &tr, true);
}

fn main() {
    // g1 = [a:a]
    let mut g1 = HfstBasicTransducer::new();
    arc(&mut g1, 0, 1, "a", "a");
    g1.set_final_weight(1, &0.0);

    // g2 = [a:a | b:b]
    let mut g2 = HfstBasicTransducer::new();
    arc(&mut g2, 0, 1, "a", "a");
    arc(&mut g2, 0, 1, "b", "b");
    g2.set_final_weight(1, &0.0);

    let result = HfstBasicTransducer::intersect(&mut g1, &mut g2);

    // intersection keeps only a:a (the common transition)
    let t = result.transitions(0);
    assert_eq!(t.len(), 1, "expected one transition, got {}", t.len());
    assert_eq!(t[0].get_input_symbol(result.coder()), "a");
    assert_eq!(t[0].get_output_symbol(result.coder()), "a");
    assert!(result.is_final_state(t[0].get_target_state()));
    println!("intersect OK (only a:a kept)");
}
