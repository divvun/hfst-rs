use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_symbol_defs::StringPairVector;

fn p(s: &str) -> StringPairVector {
    s.chars().map(|c| (c.to_string(), c.to_string())).collect()
}

fn main() {
    // Build a trie by disjuncting "cat" and "car"; they share the "ca" prefix.
    let mut lex = HfstBasicTransducer::new();
    lex.disjunct_path(&p("cat"), 0.3);
    lex.disjunct_path(&p("car"), 0.5);

    // 0 -c-> 1 -a-> 2 { -t-> 3(final 0.3), -r-> 4(final 0.5) }  => max_state 4
    assert_eq!(lex.get_max_state(), 4);
    assert!(lex.is_final_state(3));
    assert!(lex.is_final_state(4));
    assert!((lex.get_final_weight(3) - 0.3).abs() < 1e-6);
    assert!((lex.get_final_weight(4) - 0.5).abs() < 1e-6);
    // state 2 branches to two transitions (t and r)
    assert_eq!(lex.transitions(2).len(), 2);
    println!("disjunct trie OK (max_state={})", lex.get_max_state());
}
