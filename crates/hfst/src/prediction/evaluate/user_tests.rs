use super::fixture::*;
use super::report::{Row, Tally};
use super::{Faces, Harness, RunSettings, Variant};
use crate::prediction::model::Model;
use crate::prediction::walk::WalkLimits;
use crate::transducer::Transducer;

/// The text the hand-counted models are built from.
fn training() -> String {
    lookup(&[
        ("nuna", &["nuna+N+Abs+Sg"]),
        ("nuna", &["nuna+N+Abs+Sg"]),
        ("illut", &["illu+N+Abs+Pl"]),
        ("nanoqqarpoq", &["nanoq+QAR+Der/nv+V+Ind+3Sg"]),
    ])
}

/// `nanoq` four times, `nanoqqarpoq` twice and `nanoqqarput` once.
fn nanoq_training() -> String {
    let noun: (&str, &[&str]) = ("nanoq", &["nanoq+N+Abs+Sg"]);
    let singular: (&str, &[&str]) = ("nanoqqarpoq", &["nanoq+QAR+Der/nv+V+Ind+3Sg"]);
    let plural: (&str, &[&str]) = ("nanoqqarput", &["nanoq+QAR+Der/nv+V+Ind+3Pl"]);
    lookup(&[noun, noun, noun, noun, singular, singular, plural])
}

/// Score `scored` as one text with a model of `trained`.
fn score(
    generator: &Transducer,
    settings: RunSettings,
    variant: &Variant,
    trained: &str,
    scored: &str,
) -> Row {
    let grammar = grammar();
    let bytes = model_of(generator, &grammar, variant.model, trained);
    let model = Model::from_bytes(&bytes, &grammar).unwrap();
    let harness = Harness::new(generator, &grammar, settings).unwrap();
    let corpus = harness.corpus(scored).unwrap();
    let mut row = Row::new(&variant.name, true);
    harness
        .scorer()
        .score(&corpus, 0..corpus.tokens().len(), &model, variant, &mut row)
        .unwrap();
    row
}

fn tally(words: usize, characters: usize, taps: usize, in_full: usize) -> Tally {
    Tally {
        words,
        characters,
        taps,
        typed_in_full: in_full,
        ..Tally::default()
    }
}

/// With N = 1 and the model of [`training`], counted by hand:
///
/// - `nunat`: `n` (1), pick `nuna` (1); the model's first ending writes
///   `nuna`, so type `t` (1), which leaves `+N+Abs+Pl` first; pick it (1).
///   4 taps for 5 characters.
/// - `Illu`: `i` (1), pick `illu` (1); the first ending writes `illut`, and
///   the generator has written all four letters, so no letter is left to
///   type. Typed in full: 4 taps.
/// - `.` has no characters, and breaks the context.
/// - `nanoqqarpoq`: `n` (1) shows `nuna`, `a` (1) shows `nanoq`, pick it
///   (1), pick `+QAR+Der/nv` (1), pick `+V+Ind+3Sg` (1). 5 taps for 11.
/// - `xyz` has no reading: 3 taps.
///
/// 16 taps for 23 characters.
// [spec:hfst:def:prediction-evaluation.keystrokes/test]
#[test]
fn the_measure_counts_taps_by_hand() {
    let generator = generator();
    let variant = variant("order 2", model_settings(2, 0), 1, None);
    let scored = lookup(&[
        ("nunat", &["nuna+N+Abs+Pl"]),
        ("Illu", &["illu+N+Abs+Sg"]),
        (".", &[]),
        ("nanoqqarpoq", &["nanoq+QAR+Der/nv+V+Ind+3Sg"]),
        ("xyz", &[]),
    ]);
    let row = score(
        &generator,
        RunSettings::default(),
        &variant,
        &training(),
        &scored,
    );
    assert_eq!(
        row.overall,
        Tally {
            units: 7,
            first: 4,
            shown: 4,
            ..tally(4, 23, 16, 2)
        }
    );
    assert_eq!(row.overall.keystrokes_saved(), Some(1.0 - 16.0 / 23.0));
    assert_eq!(
        row.by_units[&2],
        Tally {
            units: 4,
            first: 2,
            shown: 2,
            ..tally(2, 9, 8, 1)
        }
    );
    assert_eq!(
        row.by_units[&3],
        Tally {
            units: 3,
            first: 2,
            shown: 2,
            ..tally(1, 11, 5, 0)
        }
    );
    assert_eq!(row.by_units[&0], tally(1, 3, 3, 1));
}

/// `nunap` is read `+N+Erg+Sg`, which the model never saw, but the first
/// ending, `+N+Rel+Sg`, writes the same word: `n`, pick `nuna`, pick it.
// [spec:hfst:def:prediction-evaluation.keystrokes/test]
#[test]
fn the_complete_word_is_picked_whatever_its_reading() {
    let generator = generator();
    let variant = variant("order 2", model_settings(2, 0), 1, None);
    let trained = lookup(&[("nunap", &["nuna+N+Rel+Sg"])]);
    let scored = lookup(&[("nunap", &["nuna+N+Erg+Sg"])]);
    let row = score(
        &generator,
        RunSettings::default(),
        &variant,
        &trained,
        &scored,
    );
    assert_eq!(
        row.overall,
        Tally {
            units: 2,
            first: 1,
            shown: 1,
            ..tally(1, 5, 3, 0)
        }
    );
}

/// The verb face holds `+QAR+Der/nv`, `+V+Ind+3Sg` and `+V+Ind+3Pl`, most
/// frequent first. For `nanoqqarput`, with one key and two long presses on
/// a face: `n` (1), pick `nanoq` (1); `+N+Abs+Sg` is the one suggestion, so
/// switch to the verb face and press `+QAR+Der/nv` (2); after it the word
/// is a verb, whose face is shown, and `+V+Ind+3Pl` is behind a long press
/// (2).
// [spec:hfst:def:prediction-evaluation.keystrokes/test]
#[test]
fn faces_cost_taps_long_presses_and_switches() {
    let generator = generator();
    let trained = nanoq_training();
    let scored = lookup(&[("nanoqqarput", &["nanoq+QAR+Der/nv+V+Ind+3Pl"])]);
    let faces = Faces {
        keys: 1,
        long_press: 2,
    };
    let with_faces = variant("faces", model_settings(2, 3), 1, Some(faces));
    let row = score(
        &generator,
        RunSettings::default(),
        &with_faces,
        &trained,
        &scored,
    );
    assert_eq!(
        row.overall,
        Tally {
            units: 3,
            first: 1,
            shown: 1,
            ..tally(1, 11, 6, 0)
        }
    );
}

/// With N = 1: `n` (1), pick `nanoq` (1). `+N+Abs+Sg` is first, and the
/// generator has written `nanoq`, so `q` (1) leaves the suggestions whose
/// preview starts `nanoqq`; pick `+QAR+Der/nv` (1). The generator has
/// written `qa` on one path and `qar` on another, so letters are typed
/// from the `r`: `r` (1) and `p` (1) leave `+V+Ind+3Sg` first, and `u` (1)
/// leaves `+V+Ind+3Pl`; pick it (1). With N = 2 every unit is shown.
// [spec:hfst:def:prediction-evaluation.keystrokes/test]
#[test]
fn letters_after_a_unit_filter_previews() {
    let generator = generator();
    let trained = nanoq_training();
    let scored = lookup(&[("nanoqqarput", &["nanoq+QAR+Der/nv+V+Ind+3Pl"])]);
    let one = variant("N 1", model_settings(2, 0), 1, None);
    let row = score(&generator, RunSettings::default(), &one, &trained, &scored);
    assert_eq!(row.overall.taps, 8);
    let two = variant("N 2", model_settings(2, 0), 2, None);
    let row = score(&generator, RunSettings::default(), &two, &trained, &scored);
    assert_eq!(row.overall.taps, 4);
}

// [spec:hfst:def:prediction-evaluation.keystrokes/test]
#[test]
fn unreachable_words_are_typed_in_full() {
    let generator = generator();
    let variant = variant("order 2", model_settings(2, 0), 3, None);
    let scored = lookup(&[("nanoqqarpoq", &["nanoq+QAR+Der/nv+V+Ind+3Sg"])]);
    let cramped = RunSettings {
        limits: WalkLimits {
            max_unit_symbols: 48,
            max_configurations: 3,
        },
        ..RunSettings::default()
    };
    let row = score(&generator, cramped, &variant, &training(), &scored);
    assert_eq!(row.overall.taps, 11);
    assert_eq!(row.overall.typed_in_full, 1);
    assert_eq!(row.overall.truncated, 1);
    let roomy = score(
        &generator,
        RunSettings::default(),
        &variant,
        &training(),
        &scored,
    );
    assert_eq!(roomy.overall.typed_in_full, 0);
    assert_eq!(roomy.overall.truncated, 0);
}
