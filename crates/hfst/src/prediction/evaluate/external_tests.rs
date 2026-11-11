use super::fixture::*;
use super::report::Tally;
use super::{Harness, Recording, RunSettings};
use crate::error::ErrorKind;

/// `nunat`: the start shows `nuna` and `nanoq`; pick `nuna` (1), then
/// nothing is shown, so type `t` (1). `illu`: the start shows only
/// `illut`, which `illu` does not start with, and nothing is recorded
/// after it, so all four letters are typed. 6 taps for 9 characters.
const RECORDED: &str = "nunat\t\tnuna\tnanoq\n\
                        nunat\tn\tnun\tnunat\n\
                        illu\t\tillut\n";

// [spec:hfst:req:prediction-evaluation.external/test]
// [spec:hfst:def:prediction-evaluation.keystrokes/test]
#[test]
fn a_recording_is_scored_by_hand() {
    let recording = Recording::parse("speller", RECORDED).unwrap();
    assert_eq!(recording.suggestions("nunat", ""), ["nuna", "nanoq"]);
    assert!(recording.suggestions("nunat", "nuna").is_empty());
    assert_eq!(recording.type_word("nunat").taps, 2);
    assert_eq!(recording.type_word("illu").taps, 4);
    assert_eq!(recording.type_word("nanoq").taps, 5);
}

/// The longest suggestion the word starts with is picked, and one that is
/// not longer than what is typed is passed over.
// [spec:hfst:req:prediction-evaluation.external/test]
#[test]
fn the_longest_fitting_suggestion_is_picked() {
    let recording = Recording::parse(
        "morphs",
        "nunaqarpoq\t\tnu\tnunaqar\tnuna\tnunaqarpunga\n\
         nunaqarpoq\tnunaqar\tnunaqar\tnunaqarpoq\n",
    )
    .unwrap();
    assert_eq!(recording.type_word("nunaqarpoq").taps, 2);
    let stuck = Recording::parse("stuck", "nuna\tnu\tnu\tn\n").unwrap();
    assert_eq!(stuck.type_word("nuna").taps, 4);
}

// [spec:hfst:req:prediction-evaluation.external/test]
#[test]
fn a_malformed_recording_is_refused() {
    let refused = Recording::parse("r", "nuna\n").unwrap_err();
    assert_eq!(refused.kind, ErrorKind::FaultyStringInput);
    assert!(refused.to_string().contains("line 1"), "{refused}");
    let refused = Recording::parse("r", "nuna\tx\tnuna\n").unwrap_err();
    assert!(refused.to_string().contains("does not start"), "{refused}");
    let twice = "nuna\t\tnuna\n\nnuna\t\tnuna\n";
    assert!(Recording::parse("r", twice).is_ok());
    let refused = Recording::parse("r", "nuna\t\tnuna\nnuna\t\tnu\n").unwrap_err();
    assert!(refused.to_string().contains("line 2"), "{refused}");
}

/// A recording is scored on every word of the text, beside the variants,
/// and grouped as they are by the units of each word's target.
// [spec:hfst:req:prediction-evaluation.external/test]
// [spec:hfst:req:prediction-evaluation.report/test]
#[test]
fn a_recording_is_reported_beside_the_variants() {
    let generator = generator();
    let grammar = grammar();
    let settings = RunSettings {
        folds: 2,
        ..RunSettings::default()
    };
    let harness = Harness::new(&generator, &grammar, settings).unwrap();
    let text = lookup(&[
        ("nunat", &["nuna+N+Abs+Pl"]),
        ("illu", &["illu+N+Abs+Sg"]),
        (".", &[]),
        ("xyz", &[]),
    ]);
    let corpus = harness.corpus(&text).unwrap();
    let recording = Recording::parse("speller", RECORDED).unwrap();
    let variant = variant("order 2", model_settings(2, 0), 3, None);
    let report = harness.run(&corpus, &[variant], &[recording]).unwrap();
    let names: Vec<&str> = report.rows.iter().map(|row| row.name.as_str()).collect();
    assert_eq!(names, ["order 2", "speller"]);
    let recorded = &report.rows[1];
    assert!(!recorded.ranks_units);
    assert_eq!(
        recorded.overall,
        Tally {
            words: 3,
            characters: 12,
            taps: 9,
            ..Tally::default()
        }
    );
    assert_eq!(recorded.by_units[&2].taps, 6);
    assert_eq!(recorded.by_units[&0].taps, 3);
    assert_eq!(report.rows[0].overall.words, 3);
    assert_eq!(report.rows[0].overall.characters, 12);
}
