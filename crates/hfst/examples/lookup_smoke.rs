use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_data_types::{HfstTwoLevelPaths, Symbol};
use std::collections::BTreeSet;

fn main() {
    // a:b / 0.5, final state weight 0.3
    let mut g = HfstBasicTransducer::new();
    let tr = HfstBasicTransition::new_symbols(1, "a".into(), "b".into(), 0.5, g.coder_mut());
    g.add_transition(0, &tr, true);
    g.set_final_weight(1, &0.3);

    let path: Vec<Symbol> = vec!["a".into()];
    let mut results: HfstTwoLevelPaths = BTreeSet::new();
    g.lookup(&path, &mut results, None, None, -1, false);

    assert_eq!(results.len(), 1, "expected exactly one result path");
    let r = results.iter().next().unwrap();
    // collected weight = transition 0.5 + final 0.3
    assert!((r.first - 0.8).abs() < 1e-6, "weight was {}", r.first);
    assert_eq!(
        r.second,
        vec![(Symbol::new_static("a"), Symbol::new_static("b"))]
    );
    println!("lookup OK (weight={}, path a:b)", r.first);

    // looking up "x" (not in the transducer) yields nothing
    let mut none: HfstTwoLevelPaths = BTreeSet::new();
    g.lookup(&vec!["x".into()], &mut none, None, None, -1, false);
    assert!(none.is_empty());
    println!("lookup of unknown input OK (no results)");
}
