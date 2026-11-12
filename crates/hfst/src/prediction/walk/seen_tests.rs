use std::collections::HashMap;

use super::fixtures_test::*;
use super::words_tests::{STEMS, STEMS_GRAMMAR};
use super::{CandidateOrder, Ranking, SeenForm, SeenForms, Suggest, Suggestion};
use crate::prediction::units::UnitKey;
use crate::transducer::Weight;

/// Scores units by their keys and offers fixed forms, each with its score,
/// those that start with the letters typed.
struct Fixed {
    units: HashMap<&'static str, f64>,
    forms: Vec<(&'static str, f64)>,
}

impl CandidateOrder for Fixed {
    fn scores(&self, _context: &[UnitKey], candidates: &[(UnitKey, Weight)]) -> Vec<Option<f64>> {
        candidates
            .iter()
            .map(|(key, _)| self.units.get(key.as_str()).copied())
            .collect()
    }

    fn shares(&self, context: &[UnitKey], candidates: &[(UnitKey, Weight)]) -> Vec<Option<f64>> {
        self.scores(context, candidates)
    }

    fn seen_forms<'s>(
        &'s self,
        _before: &[String],
        typed: &str,
    ) -> Option<Box<dyn SeenForms + 's>> {
        let forms = self
            .forms
            .iter()
            .filter(|(form, _)| form.starts_with(typed))
            .map(|&(form, score)| {
                let seen = SeenForm {
                    form: form.to_owned(),
                    units: vec![UnitKey::from("x")],
                };
                (seen, Some(score))
            })
            .collect();
        Some(Box::new(Offered(forms)))
    }
}

struct Offered(Vec<(SeenForm, Option<f64>)>);

impl SeenForms for Offered {
    fn best(&self, limit: Option<usize>) -> Vec<(SeenForm, Option<f64>)> {
        let mut best = self.0.clone();
        best.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        best.truncate(limit.unwrap_or(best.len()));
        best
    }

    fn find(&self, surface: &str) -> Option<(SeenForm, Option<f64>)> {
        self.0
            .iter()
            .find(|(seen, _)| seen.form == surface)
            .cloned()
    }
}

/// nanoq scores 0.8 and its first ending 0.6, so [nanoq] scores 0.48 and
/// the stem keeps 0.32; naja scores 0.5, [naja] 0.3 and the stem keeps 0.2.
/// nalu is unseen. The forms nanoq, naja and najat score 0.1, 0.35 and 0.25.
fn fixed() -> Fixed {
    Fixed {
        units: HashMap::from([("nanoq", 0.8), ("naja", 0.5), ("+N+Abs+Sg", 0.6)]),
        forms: vec![("nanoq", 0.1), ("naja", 0.35), ("najat", 0.25)],
    }
}

fn suggested(typed: &str, settings: Suggest, picked: &[&str]) -> Vec<String> {
    let generator = lexc(STEMS);
    let grammar = grammar(STEMS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    let word = match picked {
        [] => walker.start(),
        [stem, rest @ ..] => word_after(&walker, stem, rest),
    };
    let next = if typed.is_empty() {
        walker.next(&word)
    } else {
        walker.entry(&word, typed, None).unwrap()
    };
    let order = fixed();
    let ranking = Ranking {
        order: &order,
        context: &[],
        before: &[],
        typed,
    };
    walker
        .suggest(&word, &next.candidates, &ranking, settings)
        .iter()
        .map(|suggestion| match suggestion {
            Suggestion::Unit(candidate) => candidate.key.as_str().to_owned(),
            Suggestion::Word(word) => format!("[{}]", word.surface()),
            Suggestion::Form(seen) => format!("<{}>", seen.form),
        })
        .collect()
}

fn offer(forms: bool, units: bool, unit_weight: f64) -> Suggest {
    Suggest {
        whole_words: 1,
        limit: Some(5),
        forms,
        units,
        unit_weight,
    }
}

// [spec:hfst:sem:prediction-model.form-score/test]
// [spec:hfst:sem:prediction.seen-forms/test]
#[test]
fn a_form_built_from_units_is_offered_once() {
    // [nanoq] takes 0.48 over the form's 0.1, the form naja keeps 0.35
    // over [naja]'s 0.3, and neither complete word is offered again.
    assert_eq!(
        suggested("na", offer(true, true, 1.0), &[]),
        ["<nanoq>", "<naja>", "nanoq", "<najat>", "naja"]
    );
}

// [spec:hfst:sem:prediction-model.form-score/test]
#[test]
fn the_unit_weight_scales_units_beside_forms() {
    // At 0.5, [nanoq] scores 0.24, under najat's 0.25, and the stems 0.16
    // and 0.1.
    assert_eq!(
        suggested("na", offer(true, true, 0.5), &[]),
        ["<naja>", "<najat>", "<nanoq>", "nanoq", "naja"]
    );
    // At 0, every unit scores 0: below every form, then by key.
    assert_eq!(
        suggested("na", offer(true, true, 0.0), &[]),
        ["<naja>", "<najat>", "<nanoq>", "naja", "nanoq"]
    );
}

// [spec:hfst:sem:prediction.seen-forms/test]
#[test]
fn units_off_offers_forms_and_forms_off_none() {
    assert_eq!(
        suggested("na", offer(true, false, 1.0), &[]),
        ["<naja>", "<najat>", "<nanoq>"]
    );
    assert_eq!(
        suggested("na", offer(false, true, 1.0), &[]),
        ["[nanoq]", "nanoq", "[naja]", "naja", "nalu"]
    );
    assert!(suggested("na", offer(false, false, 1.0), &[]).is_empty());
}

// [spec:hfst:sem:prediction.seen-forms/test]
#[test]
fn forms_are_offered_only_before_the_first_unit() {
    let after_stem = suggested("", offer(true, true, 1.0), &["nanoq"]);
    assert!(
        after_stem.iter().all(|shown| !shown.starts_with('<')),
        "{after_stem:?}"
    );
    assert_eq!(after_stem[0], "+N+Abs+Sg");
    assert_eq!(
        suggested("", offer(true, true, 1.0), &[]),
        ["<naja>", "<najat>", "<nanoq>"]
    );
}
