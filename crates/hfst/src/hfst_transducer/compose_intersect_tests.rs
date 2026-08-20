use super::*;

fn pair(input: &str, output: &str) -> HfstTransducer<StdVectorFst> {
    HfstTransducer::from_strings(input, output, &HfstTokenizer::new()).unwrap()
}

#[test]
fn lookahead_matches_legacy_product() {
    let lexicon = pair("ab", "xy");
    let rule = pair("xy", "pq");
    let rules = vec![rule];

    let mut lookahead = lexicon.clone();
    lookahead
        .compose_intersect_with_config(&rules, false, true, &EngineConfig::default())
        .unwrap();

    let mut legacy = lexicon;
    let legacy_config = EngineConfig {
        xerox_composition: true,
        ..EngineConfig::default()
    };
    legacy
        .compose_intersect_with_config(&rules, false, true, &legacy_config)
        .unwrap();

    assert!(lookahead.compare(&legacy, true).unwrap());
}

#[test]
fn special_modes_keep_legacy_path() {
    let lexicon = pair("a", "x");
    let rule = pair("x", "b");

    assert!(
        compose_intersect::try_lookahead(&lexicon, &rule, 2, false, &EngineConfig::default())
            .unwrap()
            .is_none()
    );
    assert!(
        compose_intersect::try_lookahead(&lexicon, &rule, 1, true, &EngineConfig::default())
            .unwrap()
            .is_none()
    );

    let special = EngineConfig {
        flag_is_epsilon_in_composition: true,
        ..EngineConfig::default()
    };
    assert!(
        compose_intersect::try_lookahead(&lexicon, &rule, 1, false, &special)
            .unwrap()
            .is_none()
    );
}
