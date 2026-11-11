use super::fixture::*;
use super::{Harness, RunSettings};
use crate::prediction::model::Model;
use crate::prediction::units::UnitKey;

/// Three parts of two tokens, each with a stem no other part has.
fn three_parts() -> String {
    lookup(&[
        ("nuna", &["nuna+N+Abs+Sg"]),
        ("nunat", &["nuna+N+Abs+Pl"]),
        ("nanoq", &["nanoq+N+Abs+Sg"]),
        ("nanoqp", &["nanoq+N+Rel+Sg"]),
        ("illu", &["illu+N+Abs+Sg"]),
        ("illut", &["illu+N+Abs+Pl"]),
    ])
}

// [spec:hfst:req:prediction-evaluation.held-out/test]
#[test]
fn folds_never_score_their_own_build_text() {
    let generator = generator();
    let grammar = grammar();
    let settings = RunSettings {
        folds: 3,
        ..RunSettings::default()
    };
    let harness = Harness::new(&generator, &grammar, settings).unwrap();
    let text = three_parts();
    let corpus = harness.corpus(&text).unwrap();
    let stems = ["nuna", "nanoq", "illu"];
    let folds = corpus.folds(settings.folds).unwrap();
    assert_eq!(folds.len(), 3);
    for (part, fold) in folds.iter().enumerate() {
        let built_from: Vec<usize> = fold
            .built_from
            .iter()
            .flat_map(|range| range.clone())
            .collect();
        assert!(
            fold.scored
                .clone()
                .all(|token| !built_from.contains(&token))
        );
        assert_eq!(built_from.len() + fold.scored.len(), corpus.tokens().len());
        let built_text: String = fold
            .built_from
            .iter()
            .map(|range| corpus.text_of(range.clone()))
            .collect();
        for token in &corpus.tokens()[fold.scored.clone()] {
            assert!(!built_text.contains(&format!("{}\t", token.surface)));
        }
        let bytes = harness
            .build_model(&corpus, fold, model_settings(2, 0))
            .unwrap();
        let model = Model::from_bytes(&bytes, &grammar).unwrap();
        for (other, stem) in stems.iter().enumerate() {
            let count = model.count(&[UnitKey::from(*stem)]);
            if other == part {
                assert_eq!(count, 0.0, "fold {part} counts {stem}, which it scores");
            } else {
                assert_eq!(count, 2.0, "fold {part} leaves out {stem}");
            }
        }
    }
}

/// The parts on either side of the scored one are texts of their own: no
/// run of units spans the part left out.
// [spec:hfst:req:prediction-evaluation.held-out/test]
#[test]
fn no_run_spans_the_part_left_out() {
    let generator = generator();
    let grammar = grammar();
    let settings = RunSettings {
        folds: 3,
        ..RunSettings::default()
    };
    let harness = Harness::new(&generator, &grammar, settings).unwrap();
    let text = three_parts();
    let corpus = harness.corpus(&text).unwrap();
    let middle = &corpus.folds(3).unwrap()[1];
    let bytes = harness
        .build_model(&corpus, middle, model_settings(2, 0))
        .unwrap();
    let model = Model::from_bytes(&bytes, &grammar).unwrap();
    let run = |keys: &[&str]| {
        model.count(
            &keys
                .iter()
                .map(|&key| UnitKey::from(key))
                .collect::<Vec<_>>(),
        )
    };
    assert_eq!(run(&["+N+Abs+Pl", "illu"]), 0.0);
    assert_eq!(run(&["+N+Abs+Pl", "nanoq"]), 0.0);
    assert_eq!(run(&["+N+Abs+Sg", "nuna"]), 1.0);
    assert_eq!(run(&["+N+Abs+Sg", "illu"]), 1.0);
}

// [spec:hfst:req:prediction-evaluation.held-out/test]
#[test]
fn k_is_ten_when_not_given() {
    assert_eq!(RunSettings::default().folds, 10);
    let generator = generator();
    let grammar = grammar();
    let harness = Harness::new(&generator, &grammar, RunSettings::default()).unwrap();
    let text = three_parts();
    let corpus = harness.corpus(&text).unwrap();
    let refused = harness
        .run(&corpus, &[variant("v", model_settings(2, 0), 3, None)], &[])
        .unwrap_err();
    assert!(
        refused
            .to_string()
            .contains("6 tokens cannot be split into 10 parts"),
        "{refused}"
    );
}
