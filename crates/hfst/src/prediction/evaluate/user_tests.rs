use super::fixture::*;
use super::report::{Row, Tally};
use super::user::{Eyes, Slot, UnitTypist, View};
use super::walks::{Building, Walks};
use super::{Faces, Harness, RunSettings, Variant};
use crate::prediction::model::Model;
use crate::prediction::units::UnitKey;
use crate::prediction::walk::{WalkLimits, Walker};
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
    let mut rows = [Row::new(&variant.name, true)];
    let all = 0..corpus.tokens().len();
    harness
        .scorer()
        .score(&corpus, all.clone(), all, &[(&model, variant)], &mut rows)
        .unwrap();
    let [row] = rows;
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

/// What one look after `nanoq` shows with the filter `filter` and N =
/// `shown`, under a model of [`nanoq_training`], which ranks `+N+Abs+Sg`
/// first and `+QAR+Der/nv` second: the keys shown, the keys whose previews
/// were read, and the keys ruled out without one.
fn look_after_nanoq(shown: usize, filter: &str) -> [Vec<String>; 3] {
    let generator = generator();
    let grammar = grammar();
    let settings = model_settings(2, 0);
    let bytes = model_of(&generator, &grammar, settings, &nanoq_training());
    let model = Model::from_bytes(&bytes, &grammar).unwrap();
    let variant = variant("v", settings, shown, None);
    let walker = || Walker::new(&generator, &grammar, WalkLimits::default()).unwrap();
    let eyes = Eyes::new(walker(), &model, &variant);
    let mut walks = Walks::new(walker());
    let mut building = Building::new(&walker());
    for letter in "nanoq".chars() {
        building.type_letter(&walker(), letter).unwrap();
    }
    let entered = walks.offer(building.steps(), building.word()).unwrap();
    let stem = entered
        .candidates
        .iter()
        .find(|candidate| candidate.key.as_str() == "nanoq")
        .unwrap();
    building.pick(stem);
    let next = walks.offer(building.steps(), building.word()).unwrap();
    let mut slot = Slot {
        filter: Some(filter.to_owned()),
        ..Slot::default()
    };
    let view = View {
        word: building.word(),
        steps: building.steps(),
        ranking: &[UnitKey::from("nanoq")],
    };
    let looked = eyes
        .shown(&view, &next.candidates, &mut slot, &mut walks)
        .unwrap();
    let sorted = |keys: Vec<&UnitKey>| {
        let mut keys: Vec<String> = keys.iter().map(|key| key.as_str().to_owned()).collect();
        keys.sort();
        keys
    };
    [
        sorted(looked.iter().map(|candidate| &candidate.key).collect()),
        sorted(slot.previews.keys().collect()),
        sorted(slot.ruled_out.iter().collect()),
    ]
}

/// Previews are read in the model's order and none after the Nth that
/// fits. A suggestion whose paths have written something the filter does
/// not start with, as `nanoqt` and `nanoqp` against `nanoqq`, is ruled out
/// without its preview.
// [spec:hfst:def:prediction-evaluation.keystrokes/test]
#[test]
fn previews_are_read_only_up_to_the_cut() {
    let none: Vec<String> = Vec::new();
    let [shown, walked, ruled_out] = look_after_nanoq(1, "nanoq");
    assert_eq!(
        (shown, walked),
        (vec!["+N+Abs+Sg".to_owned()], vec!["+N+Abs+Sg".to_owned()])
    );
    assert_eq!(ruled_out, none);
    let [shown, walked, ruled_out] = look_after_nanoq(1, "nanoqq");
    assert_eq!(shown, ["+QAR+Der/nv"]);
    assert_eq!(walked, ["+N+Abs+Sg", "+QAR+Der/nv"]);
    assert_eq!(ruled_out, none);
    let [shown, walked, ruled_out] = look_after_nanoq(2, "nanoqq");
    assert_eq!(shown, ["+QAR+Der/nv"]);
    assert_eq!(walked, ["+N+Abs+Sg", "+QAR+Der/nv"]);
    assert_eq!(ruled_out, ["+N+Abs+Pl", "+N+Erg+Sg", "+N+Rel+Sg"]);
}

/// Variants typing the same word share its walks. With N = 2 the user
/// reaches only places the user with N = 1 has walked, so the second walks
/// nothing where alone it walks four, and types the word as it would
/// alone.
// [spec:hfst:def:prediction-evaluation.keystrokes/test]
#[test]
fn variants_typing_a_word_share_its_walks() {
    let generator = generator();
    let grammar = grammar();
    let settings = model_settings(2, 0);
    let bytes = model_of(&generator, &grammar, settings, &nanoq_training());
    let model = Model::from_bytes(&bytes, &grammar).unwrap();
    let harness = Harness::new(&generator, &grammar, RunSettings::default()).unwrap();
    let text = lookup(&[("nanoqqarput", &["nanoq+QAR+Der/nv+V+Ind+3Pl"])]);
    let corpus = harness.corpus(&text).unwrap();
    let target = corpus.target(0).unwrap();
    let limits = WalkLimits::default();
    let type_with = |shown: usize, walks: &mut Walks<'_, _>| {
        let variant = variant("v", settings, shown, None);
        let mut typist = UnitTypist::new(&generator, &grammar, limits, &model, &variant).unwrap();
        typist.type_word("nanoqqarput", target, &[], walks).unwrap()
    };
    let walker = || Walker::new(&generator, &grammar, limits).unwrap();
    let mut shared = Walks::new(walker());
    assert_eq!(type_with(1, &mut shared).taps, 8);
    let walked = shared.walked();
    let typed = type_with(2, &mut shared);
    assert_eq!(shared.walked(), walked);
    assert_eq!(typed.taps, 4);
    let mut alone = Walks::new(walker());
    assert_eq!(type_with(2, &mut alone), typed);
    assert_eq!(alone.walked(), 4);
}
