use super::fixture::*;
use super::report::{Row, Tally};
use super::{Harness, RunSettings, Variant};
use crate::prediction::model::Model;
use crate::transducer::Transducer;

/// nuna twice, illut once and nanoqqarpoq once: four forms, and nine units.
fn training() -> String {
    lookup(&[
        ("nuna", &["nuna+N+Abs+Sg"]),
        ("nuna", &["nuna+N+Abs+Sg"]),
        ("illut", &["illu+N+Abs+Pl"]),
        ("nanoqqarpoq", &["nanoq+QAR+Der/nv+V+Ind+3Sg"]),
    ])
}

fn score(generator: &Transducer, variant: &Variant, trained: &str, scored: &str) -> Row {
    let grammar = grammar();
    let bytes = model_of(generator, &grammar, variant.model, trained);
    let model = Model::from_bytes(&bytes, &grammar).unwrap();
    let harness = Harness::new(generator, &grammar, RunSettings::default()).unwrap();
    let corpus = harness.corpus(scored).unwrap();
    let mut rows = [Row::new(&variant.name, true)];
    let all = 0..corpus.tokens().len();
    harness
        .scorer()
        .score(&corpus, all.clone(), all, &[(&model, variant)], &mut rows)
        .unwrap();
    let [row] = rows;
    row
}

fn forms_variant(order: usize, shown: usize, units: bool) -> Variant {
    Variant {
        forms: true,
        units,
        ..variant("forms", model_settings(order, 0), shown, None)
    }
}

/// Word forms alone, N = 1, order 1, counted by hand:
///
/// - `nuna`: with no letter typed, nuna, the most frequent form, is shown
///   and picked: 1 tap for 4 characters, judged first and shown at that
///   look.
/// - `Illut`: nuna is shown; type `I` (1), which leaves illut; pick it (1).
///   Judged after the first letter.
/// - `nanoq`: nuna is shown, then after `n` nuna, after `na`
///   nanoqqarpoq; the word was never counted, so it is typed in full: 5.
/// - `xyz` has no reading, so no form, and is typed in full: 3.
///
/// 11 taps for 17 characters.
// [spec:hfst:def:prediction-evaluation.keystrokes/test]
// [spec:hfst:sem:prediction.seen-forms/test]
#[test]
fn completing_counted_forms_alone_counts_taps_by_hand() {
    let generator = generator();
    let scored = lookup(&[
        ("nuna", &["nuna+N+Abs+Sg"]),
        ("Illut", &["illu+N+Abs+Pl"]),
        ("nanoq", &["nanoq+N+Abs+Sg"]),
        ("xyz", &[]),
    ]);
    let row = score(
        &generator,
        &forms_variant(1, 1, false),
        &training(),
        &scored,
    );
    let tally = |words, characters, taps, typed_in_full, first, shown| Tally {
        words,
        characters,
        taps,
        typed_in_full,
        truncated: 0,
        units: words,
        first,
        shown,
    };
    assert_eq!(row.overall, tally(4, 17, 11, 2, 2, 2));
    assert_eq!(row.by_units[&2], tally(3, 14, 8, 1, 2, 2));
    assert_eq!(row.by_units[&0], tally(1, 3, 3, 1, 0, 0));
}

/// Forms alone, N = 1, order 2, after a training text in which nuna comes
/// before illut twice, broken by tokens with no reading:
///
/// - `nuna`, with nothing before: nanoqqarpoq is shown with no letter and
///   after `n`, where the word is judged; after `nu` nuna is, and is
///   picked: 3 taps.
/// - `illut`, after nuna: illut is the likeliest form, picked with no
///   letter typed: 1 tap.
/// - `.` has no reading and breaks the context.
/// - `illut`, with nothing before: nanoqqarpoq is shown; after `i` illut
///   is, and is picked: 2 taps.
///
/// 6 taps for 14 characters; two words were first and shown when judged.
// [spec:hfst:def:prediction-evaluation.keystrokes/test]
// [spec:hfst:sem:prediction-model.form-score/test]
#[test]
fn the_forms_before_rank_the_next_word() {
    let generator = generator();
    let nuna: (&str, &[&str]) = ("nuna", &["nuna+N+Abs+Sg"]);
    let illut: (&str, &[&str]) = ("illut", &["illu+N+Abs+Pl"]);
    let nanoq: (&str, &[&str]) = ("nanoqqarpoq", &["nanoq+QAR+Der/nv+V+Ind+3Sg"]);
    let stop: (&str, &[&str]) = (".", &[]);
    let trained = lookup(&[nuna, illut, stop, nuna, illut, stop, nanoq, nanoq, nanoq]);
    let scored = lookup(&[nuna, illut, stop, illut]);
    let row = score(&generator, &forms_variant(2, 1, false), &trained, &scored);
    assert_eq!((row.overall.characters, row.overall.taps), (14, 6));
    assert_eq!((row.overall.first, row.overall.shown), (2, 2));
}

/// Forms and units, N = 2, order 1, unit weight 1, counted by hand:
///
/// - `nuna`: with no letter, nuna (2/4) and illut (1/4, before
///   nanoqqarpoq by its letters) are shown; pick nuna: 1 tap, judged at
///   that look.
/// - `nanoq`, a form never counted: with no letter, nuna and illut; after
///   `n` the forms nuna (1/2) and nanoqqarpoq (1/4) outrank the stems nuna
///   (2/9) and nanoq (1/9); after `na`, nanoqqarpoq and the stem nanoq are
///   shown, and the stem is picked (1), then its first ending, which
///   writes the word (1): 4 taps. The stem is judged after `n`, not shown;
///   the ending at its first look, first.
// [spec:hfst:def:prediction-evaluation.keystrokes/test]
// [spec:hfst:sem:prediction.seen-forms/test]
#[test]
fn forms_and_units_count_taps_by_hand() {
    let generator = generator();
    let scored = lookup(&[("nuna", &["nuna+N+Abs+Sg"]), ("nanoq", &["nanoq+N+Abs+Sg"])]);
    let row = score(&generator, &forms_variant(1, 2, true), &training(), &scored);
    assert_eq!(
        row.overall,
        Tally {
            words: 2,
            characters: 9,
            taps: 5,
            typed_in_full: 0,
            truncated: 0,
            units: 3,
            first: 2,
            shown: 2,
        }
    );
}

/// Without forms, the same words are typed as units alone, and the unit
/// weight changes nothing: each costs `n`, its stem, which the two shown
/// hold, and its ending.
// [spec:hfst:def:prediction-evaluation.keystrokes/test]
#[test]
fn forms_off_types_by_units_alone() {
    let generator = generator();
    let scored = lookup(&[("nuna", &["nuna+N+Abs+Sg"]), ("nanoq", &["nanoq+N+Abs+Sg"])]);
    let variant = Variant {
        unit_weight: 0.0,
        ..variant("units", model_settings(1, 0), 2, None)
    };
    let row = score(&generator, &variant, &training(), &scored);
    assert_eq!((row.overall.characters, row.overall.taps), (9, 6));
}
