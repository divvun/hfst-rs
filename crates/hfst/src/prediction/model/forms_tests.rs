use super::fixture::{build_text, grammar, keys, lookup_forms, settings};
use super::{Ambiguity, Backoff, Model, ModelSettings};

/// Five tokens, a token with no reading, then two more. `Nuna` and `nuna`
/// are one form, so the forms are nuna three times, oqarpoq twice, and
/// sinippoq and nunap once each: seven in all.
const SAID: &[(&str, &[&str])] = &[
    ("Nuna", &["nuna+N+Abs+Sg"]),
    ("oqarpoq", &["oqar+V+Ind+3Sg"]),
    ("nuna", &["nuna+N+Abs+Sg"]),
    ("sinippoq", &["sini+V+Ind+3Sg"]),
    ("nunap", &["nuna+N+Rel+Sg"]),
    (".", &[]),
    ("nuna", &["nuna+N+Abs+Sg"]),
    ("oqarpoq", &["oqar+V+Ind+3Sg"]),
];

fn said(settings: ModelSettings) -> Vec<u8> {
    build_text(settings, &lookup_forms(SAID))
}

fn before(forms: &[&str]) -> Vec<String> {
    forms.iter().map(|&form| form.to_owned()).collect()
}

// [spec:hfst:req:prediction-model.word-forms/test]
// [spec:hfst:def:prediction-model.file+1/test]
#[test]
fn forms_are_counted_folded_alone_and_in_runs() {
    let bytes = said(settings(3, Backoff::StupidBackoff));
    let model = Model::from_bytes(&bytes, &grammar()).unwrap();
    let count = |run: &[&str]| model.form_count(run);
    assert_eq!(count(&[]), 7.0);
    assert_eq!(count(&["nuna"]), 3.0);
    assert_eq!(count(&["NUNA"]), 3.0);
    assert_eq!(count(&["oqarpoq"]), 2.0);
    assert_eq!(count(&["."]), 0.0);
    assert_eq!(count(&["nuna", "oqarpoq"]), 2.0);
    assert_eq!(count(&["Nuna", "oqarpoq", "nuna"]), 1.0);
    assert_eq!(count(&["nuna", "sinippoq", "nunap"]), 1.0);
    assert_eq!(count(&["nuna", "oqarpoq", "nuna", "sinippoq"]), 0.0);
    assert_eq!(model.form_units("Nuna"), Some(keys(&["nuna", "+N+Abs+Sg"])));
    assert_eq!(
        model.form_units("nunap"),
        Some(keys(&["nuna", "+N+Rel+Sg"]))
    );
    assert_eq!(model.form_units("nunat"), None);
}

// [spec:hfst:req:prediction-model.word-forms/test]
#[test]
fn a_token_with_no_reading_breaks_form_runs() {
    let bytes = said(settings(3, Backoff::StupidBackoff));
    let model = Model::from_bytes(&bytes, &grammar()).unwrap();
    assert_eq!(model.form_count(&["nunap", "nuna"]), 0.0);
    assert_eq!(model.form_count(&["sinippoq", "nunap", "nuna"]), 0.0);
    assert_eq!(model.form_count(&["nunap", ".", "nuna"]), 0.0);
    assert_eq!(model.form_count(&["sinippoq", "nunap"]), 1.0);
}

/// `oqarpoq` five times: twice read as 3Sg alone, three times as 3Pl
/// first and 3Sg second.
const AMBIGUOUS: &[(&str, &[&str])] = &[
    ("oqarpoq", &["oqar+V+Ind+3Sg"]),
    ("oqarpoq", &["oqar+V+Ind+3Sg"]),
    ("oqarpoq", &["oqar+V+Ind+3Pl", "oqar+V+Ind+3Sg"]),
    ("oqarpoq", &["oqar+V+Ind+3Pl", "oqar+V+Ind+3Sg"]),
    ("oqarpoq", &["oqar+V+Ind+3Pl", "oqar+V+Ind+3Sg"]),
];

// [spec:hfst:req:prediction-model.word-forms/test]
#[test]
fn a_form_keeps_its_most_frequent_reading() {
    let mut equal = settings(2, Backoff::StupidBackoff);
    equal.ambiguity = Ambiguity::EqualShares;
    for (settings, ending, runs) in [
        (settings(2, Backoff::StupidBackoff), "+V+Ind+3Pl", 4.0),
        (equal, "+V+Ind+3Sg", 4.0),
    ] {
        let bytes = build_text(settings, &lookup_forms(AMBIGUOUS));
        let model = Model::from_bytes(&bytes, &grammar()).unwrap();
        assert_eq!(model.form_count(&["oqarpoq"]), 5.0);
        assert_eq!(model.form_count(&["oqarpoq", "oqarpoq"]), runs);
        assert_eq!(model.form_units("oqarpoq"), Some(keys(&["oqar", ending])));
    }
}

// [spec:hfst:req:prediction-model.word-forms/test]
// [spec:hfst:syn:prediction-model.format/test]
#[test]
fn forms_starting_with_letters_are_found_in_place() {
    let bytes = said(settings(2, Backoff::StupidBackoff));
    let mut buffer = vec![0xAB; 3];
    buffer.extend_from_slice(&bytes);
    let model = Model::from_bytes(&buffer[3..], &grammar()).unwrap();
    let starting = |letters: &str| -> Vec<(String, f64)> {
        model
            .forms_starting(letters)
            .map(|(form, count)| (form.to_owned(), count))
            .collect()
    };
    let pair = |form: &str, count: f64| (form.to_owned(), count);
    assert_eq!(starting("nu"), [pair("nuna", 3.0), pair("nunap", 1.0)]);
    assert_eq!(starting("NUNAP"), [pair("nunap", 1.0)]);
    assert_eq!(starting("o"), [pair("oqarpoq", 2.0)]);
    assert!(starting("x").is_empty());
    assert!(starting("nunapi").is_empty());
    let all: Vec<String> = starting("").into_iter().map(|(form, _)| form).collect();
    assert_eq!(all, ["nuna", "nunap", "oqarpoq", "sinippoq"]);
}

/// The forms' scores after a context, best first, by the form.
fn scored(
    backoff: Backoff,
    context: &[&str],
    typed: &str,
    limit: Option<usize>,
) -> Vec<(String, Option<f64>)> {
    let bytes = said(settings(3, backoff));
    let model = Model::from_bytes(&bytes, &grammar()).unwrap();
    let scores = model.form_scores(&before(context));
    let offered = scores.offered(typed);
    scores
        .best(&offered, limit)
        .into_iter()
        .map(|(id, score)| (scores.text(id).unwrap().to_owned(), score))
        .collect()
}

fn form(text: &str, score: Option<f64>) -> (String, Option<f64>) {
    (text.to_owned(), score)
}

// [spec:hfst:sem:prediction-model.form-score/test]
#[test]
fn stupid_backoff_scores_forms_after_the_forms_before() {
    // nuna oqarpoq was seen twice, once with a form after it, nuna. The
    // other forms were never seen after it or after oqarpoq alone, so they
    // back off twice, to 0.4 × 0.4 times their share of the seven forms.
    let all = scored(Backoff::StupidBackoff, &["Nuna", "oqarpoq"], "", None);
    assert_eq!(
        all,
        [
            form("nuna", Some(1.0)),
            form("oqarpoq", Some(0.4 * 0.4 * 2.0 / 7.0)),
            form("nunap", Some(0.4 * 0.4 / 7.0)),
            form("sinippoq", Some(0.4 * 0.4 / 7.0)),
        ]
    );
    let after = scored(Backoff::StupidBackoff, &["sinippoq"], "nu", None);
    assert_eq!(
        after,
        [
            form("nunap", Some(1.0)),
            form("nuna", Some(0.4 * 3.0 / 7.0))
        ]
    );
    let unknown = scored(Backoff::StupidBackoff, &["zzz", "nunap"], "o", None);
    assert_eq!(unknown, [form("oqarpoq", Some(0.4 * 0.4 * 2.0 / 7.0))]);
}

// [spec:hfst:sem:prediction-model.form-score/test]
// [spec:hfst:req:prediction-model.rank-only/test]
#[test]
fn drop_oldest_scores_forms_after_one_context() {
    let all = scored(Backoff::DropOldest, &["nuna", "oqarpoq"], "", None);
    assert_eq!(
        all,
        [
            form("nuna", Some(1.0)),
            form("nunap", None),
            form("oqarpoq", None),
            form("sinippoq", None),
        ]
    );
    let s = scored(Backoff::DropOldest, &["nuna", "oqarpoq"], "s", None);
    assert_eq!(s, [form("sinippoq", Some(1.0 / 7.0))]);
    let nu = scored(Backoff::DropOldest, &["oqarpoq"], "nu", Some(1));
    assert_eq!(nu, [form("nuna", Some(1.0))]);
}

// [spec:hfst:sem:prediction-model.form-score/test]
// [spec:hfst:sem:prediction.seen-forms/test]
#[test]
fn no_letters_offer_the_likeliest_next_forms() {
    assert_eq!(
        scored(Backoff::StupidBackoff, &[], "", Some(2)),
        [
            form("nuna", Some(3.0 / 7.0)),
            form("oqarpoq", Some(2.0 / 7.0))
        ]
    );
    let after_nuna = scored(Backoff::StupidBackoff, &["nuna"], "", Some(2));
    assert_eq!(
        after_nuna,
        [
            form("oqarpoq", Some(2.0 / 3.0)),
            form("sinippoq", Some(1.0 / 3.0))
        ]
    );
    assert!(scored(Backoff::StupidBackoff, &["nuna"], "", Some(0)).is_empty());
}

// [spec:hfst:req:prediction-model.budget/test]
// [spec:hfst:req:prediction-model.word-forms/test]
#[test]
fn size_limit_drops_form_runs_with_unit_runs() {
    let mut sized = settings(2, Backoff::StupidBackoff);
    let full = said(sized).len();
    // Of the runs of two, three runs of units and three runs of forms were
    // counted once. The units go first, then sinippoq nunap, whose ids come
    // last.
    sized.size_limit = full - 4 * 12;
    let bytes = said(sized);
    assert_eq!(bytes.len(), sized.size_limit);
    let model = Model::from_bytes(&bytes, &grammar()).unwrap();
    assert_eq!(model.count(&keys(&["nuna", "+N+Rel+Sg"])), 0.0);
    assert_eq!(model.count(&keys(&["oqar", "+V+Ind+3Sg"])), 2.0);
    assert_eq!(model.form_count(&["sinippoq", "nunap"]), 0.0);
    assert_eq!(model.form_count(&["nuna", "sinippoq"]), 1.0);
    assert_eq!(model.form_count(&["nuna", "oqarpoq"]), 2.0);
    sized.size_limit = full - 11 * 12;
    let floor = said(sized);
    let model = Model::from_bytes(&floor, &grammar()).unwrap();
    assert_eq!(model.form_count(&["nuna", "oqarpoq"]), 0.0);
    assert_eq!(model.form_count(&["nuna"]), 3.0);
    assert_eq!(
        model.form_units("nunap"),
        Some(keys(&["nuna", "+N+Rel+Sg"]))
    );
    sized.size_limit -= 1;
    let error = super::fixture::builder(sized, |_| true)
        .and_then(|mut builder| {
            builder.add_text(&lookup_forms(SAID))?;
            builder.write()
        })
        .unwrap_err();
    assert!(error.to_string().contains("1 bytes more"), "{error}");
}
