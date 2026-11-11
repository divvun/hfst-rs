use std::collections::BTreeMap;

use super::fixture::*;
use super::report::Typed;
use super::{Harness, Report, Row, RunSettings, Tally};

fn tally(words: usize, characters: usize, taps: usize, in_full: usize) -> Tally {
    Tally {
        words,
        characters,
        taps,
        typed_in_full: in_full,
        ..Tally::default()
    }
}

fn report() -> Report {
    let ranked = Row {
        name: "order 2, N 3".to_owned(),
        ranks_units: true,
        overall: Tally {
            units: 4,
            first: 1,
            shown: 2,
            ..tally(3, 12, 9, 1)
        },
        by_units: BTreeMap::from([
            (0, tally(1, 3, 3, 1)),
            (
                2,
                Tally {
                    units: 4,
                    first: 1,
                    shown: 2,
                    ..tally(2, 9, 6, 0)
                },
            ),
        ]),
    };
    let recorded = Row {
        name: "a speller that completes morphs".to_owned(),
        ranks_units: false,
        overall: tally(3, 12, 6, 0),
        by_units: BTreeMap::from([(0, tally(1, 3, 3, 0)), (2, tally(2, 9, 3, 0))]),
    };
    Report {
        rows: vec![ranked, recorded],
    }
}

// [spec:hfst:req:prediction-evaluation.report/test]
#[test]
fn rows_are_set_side_by_side() {
    let expected = [
        "                              order 2, N 3                      a speller that completes morphs",
        "units        words    chars     saved   first   shown    full     saved   first   shown    full",
        "all              3       12     25.0%   25.0%   50.0%       1     50.0%       -       -       0",
        "2                2        9     33.3%   25.0%   50.0%       0     66.7%       -       -       0",
        "no reading       1        3      0.0%       -       -       1      0.0%       -       -       0",
    ];
    let table = report().table();
    assert_eq!(table.lines().collect::<Vec<_>>(), expected, "\n{table}");
}

// [spec:hfst:req:prediction-evaluation.report/test]
#[test]
fn words_are_counted_overall_and_by_units() {
    let mut row = Row::new("v", true);
    let picked = Typed {
        taps: 3,
        looked_for: vec![(true, true), (false, true)],
        ..Typed::default()
    };
    row.add(2, 5, &picked);
    row.add(2, 4, &Typed::in_full(4));
    row.add(0, 3, &Typed::in_full(3));
    assert_eq!(
        row.overall,
        Tally {
            units: 2,
            first: 1,
            shown: 2,
            ..tally(3, 12, 10, 2)
        }
    );
    assert_eq!(row.by_units[&2].words, 2);
    assert_eq!(row.by_units[&0], tally(1, 3, 3, 1));
    assert_eq!(row.overall.first_rate(), Some(0.5));
    assert_eq!(row.overall.shown_rate(), Some(1.0));
    assert_eq!(Tally::default().keystrokes_saved(), None);
    assert_eq!(Tally::default().first_rate(), None);
}

/// Every variant is scored on every fold, so each row covers the same
/// words.
// [spec:hfst:req:prediction-evaluation.report/test]
// [spec:hfst:req:prediction-evaluation.held-out/test]
#[test]
fn every_variant_scores_the_same_words() {
    let generator = generator();
    let grammar = grammar();
    let settings = RunSettings {
        folds: 2,
        ..RunSettings::default()
    };
    let harness = Harness::new(&generator, &grammar, settings).unwrap();
    let word: (&str, &[&str]) = ("nunat", &["nuna+N+Abs+Pl"]);
    let other: (&str, &[&str]) = ("illu", &["illu+N+Abs+Sg"]);
    let text = lookup(&[word, other, word, other, word, other]);
    let corpus = harness.corpus(&text).unwrap();
    let variants = [
        variant("order 1, N 1", model_settings(1, 0), 1, None),
        variant("order 2, N 1", model_settings(2, 0), 1, None),
        variant("order 2, N 3", model_settings(2, 0), 3, None),
    ];
    let report = harness.run(&corpus, &variants, &[]).unwrap();
    assert_eq!(report.rows.len(), 3);
    for row in &report.rows {
        assert_eq!(row.overall.words, 6, "{}", row.name);
        assert_eq!(row.overall.characters, 27, "{}", row.name);
        assert_eq!(row.overall.units, 12, "{}", row.name);
        assert!(row.overall.taps < 27, "{}: {:?}", row.name, row.overall);
    }
    assert!(report.rows[2].overall.taps <= report.rows[1].overall.taps);
    let table = report.table();
    let names = table.lines().next().unwrap();
    assert!(
        names.contains("order 1, N 1") && names.contains("order 2, N 3"),
        "{table}"
    );
}
