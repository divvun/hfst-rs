use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_data_types::HfstTwoLevelPaths;
use std::collections::BTreeSet;

fn main() {
    // a:b / 0.5, final state weight 0.3
    let mut g = HfstBasicTransducer::new();
    g.add_transition(
        0,
        &HfstBasicTransition::new_symbols(1, "a".to_string(), "b".to_string(), 0.5),
        true,
    );
    g.set_final_weight(1, &0.3);

    let path = vec!["a".to_string()];
    let mut results: HfstTwoLevelPaths = BTreeSet::new();
    g.lookup(&path, &mut results, None, None, -1, false);

    assert_eq!(results.len(), 1, "expected exactly one result path");
    let r = results.iter().next().unwrap();
    // collected weight = transition 0.5 + final 0.3
    assert!((r.first - 0.8).abs() < 1e-6, "weight was {}", r.first);
    assert_eq!(r.second, vec![("a".to_string(), "b".to_string())]);
    println!("lookup OK (weight={}, path a:b)", r.first);

    // looking up "x" (not in the transducer) yields nothing
    let mut none: HfstTwoLevelPaths = BTreeSet::new();
    g.lookup(&vec!["x".to_string()], &mut none, None, None, -1, false);
    assert!(none.is_empty());
    println!("lookup of unknown input OK (no results)");
}
