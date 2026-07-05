use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;

const EPS: &str = "@_EPSILON_SYMBOL_@";

fn main() {
    // epsilon self-loop with negative weight
    let mut g = HfstBasicTransducer::new();
    let tr = HfstBasicTransition::new_symbols(0, EPS.into(), EPS.into(), -0.5, g.coder_mut());
    g.add_transition(0, &tr, true);
    g.set_final_weight(0, &0.0);
    assert!(g.has_negative_epsilon_cycles());
    assert!(g.is_infinitely_ambiguous());
    println!("negative epsilon cycle detected OK");

    // acyclic a:b -- neither
    let mut g2 = HfstBasicTransducer::new();
    let tr = HfstBasicTransition::new_symbols(1, "a".into(), "b".into(), 0.0, g2.coder_mut());
    g2.add_transition(0, &tr, true);
    g2.set_final_weight(1, &0.0);
    assert!(!g2.has_negative_epsilon_cycles());
    assert!(!g2.is_infinitely_ambiguous());
    println!("acyclic graph OK");
}
