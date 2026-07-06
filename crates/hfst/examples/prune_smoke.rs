// Exercise HfstTransducer::prune end-to-end. prune() converts to tropical and
// runs fst::Prune(fst, One), now ported into the rustfst fork: with threshold
// One the only surviving paths are those whose weight equals the shortest path.

use hfst::hfst_data_types::HfstTwoLevelPaths;
use hfst::hfst_transducer::HfstTransducer;
use hfst_openfst::StdVectorFst;
use std::collections::BTreeSet;

fn main() -> hfst::error::Result<()> {
    // Two paths: "a" (total weight 1.0, the best) and "b" (total weight 5.0).
    let att = "0\t1\ta\ta\t1\n1\t0\n0\t2\tb\tb\t5\n2\t0\n";
    let path = std::env::temp_dir().join("hfst_prune_smoke.att");
    let path = path.to_str().unwrap().to_string();
    std::fs::write(&path, att).unwrap();

    let mut t = HfstTransducer::<StdVectorFst>::read_in_att_format_filename(
        &path,
        "@_EPSILON_SYMBOL_@",
        false,
    )
    .expect("written two-path AT&T file reads back as a valid transducer");

    // Before pruning: both "a" and "b" are present.
    let mut before: HfstTwoLevelPaths = BTreeSet::new();
    t.extract_paths(&mut before, -1, -1)?;
    let before_inputs: BTreeSet<String> = before
        .iter()
        .map(|p| p.second.iter().map(|(i, _)| i.as_str()).collect::<String>())
        .collect();
    assert_eq!(
        before_inputs,
        BTreeSet::from(["a".to_string(), "b".to_string()]),
        "both paths present before prune, got {before_inputs:?}"
    );

    t.prune()?;

    // After pruning with threshold One only the best path ("a", weight 1.0) survives.
    let mut after: HfstTwoLevelPaths = BTreeSet::new();
    t.extract_paths(&mut after, -1, -1)?;
    let after_inputs: BTreeSet<String> = after
        .iter()
        .map(|p| p.second.iter().map(|(i, _)| i.as_str()).collect::<String>())
        .collect();
    assert_eq!(
        after_inputs,
        BTreeSet::from(["a".to_string()]),
        "only the best path survives prune, got {after_inputs:?}"
    );

    // And it kept its weight (1.0).
    let best = after.iter().next().unwrap();
    assert!(
        (best.first - 1.0).abs() < 1e-6,
        "surviving path keeps weight 1.0, got {}",
        best.first
    );

    let _ = std::fs::remove_file(&path);
    println!("prune OK (b pruned, a kept with weight 1.0)");
    Ok(())
}
