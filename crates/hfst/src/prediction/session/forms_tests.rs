use super::Session;
use crate::error::ErrorKind;
use crate::prediction::model::{
    Ambiguity, Backoff, Model, ModelBuilder, ModelSettings, generator_accepts,
};
use crate::prediction::realise::Script;
use crate::prediction::units::{AnalysisTokenizer, UnitGrammar, UnitKey};
use crate::prediction::walk::fixtures_test::*;
use crate::prediction::walk::{Suggest, Suggestion};
use crate::transducer::{Transducer, WeightedTables};

/// guoli twice, each time before girji; girji three times; girjat once.
const TEXT: &[(&str, &str)] = &[
    ("guoli", "guolli+N+Sg+Nom"),
    ("girji", "girji+N+Sg+Nom"),
    ("Guoli", "guolli+N+Sg+Nom"),
    ("girji", "girji+N+Sg+Nom"),
    ("girji", "girji+N+Sg+Nom"),
    ("girjat", "girji+N+Pl+Nom"),
];

fn model_bytes(generator: &Transducer, grammar: &UnitGrammar) -> Vec<u8> {
    let tokenizer = AnalysisTokenizer::new(generator.get_alphabet().get_symbol_table());
    let settings = ModelSettings {
        order: 2,
        backoff: Backoff::StupidBackoff,
        ambiguity: Ambiguity::FirstReading,
        face_size: 0,
        size_limit: usize::MAX,
    };
    let mut builder =
        ModelBuilder::new(grammar, &tokenizer, generator_accepts(generator), settings).unwrap();
    let text: String = TEXT
        .iter()
        .map(|(surface, reading)| format!("{surface}\t{reading}\t0.000000\n\n"))
        .collect();
    builder.add_text(&text).unwrap();
    builder.write().unwrap()
}

fn offer(forms: bool, units: bool) -> Suggest {
    Suggest {
        limit: Some(3),
        forms,
        units,
        ..Suggest::default()
    }
}

fn shown(
    session: &mut Session<'_, WeightedTables>,
    model: &Model<'_>,
    settings: Suggest,
) -> Vec<String> {
    session
        .suggestions(model, settings)
        .unwrap()
        .iter()
        .map(|suggestion| match suggestion {
            Suggestion::Unit(candidate) => candidate.key.as_str().to_owned(),
            Suggestion::Word(word) => format!("[{}]", word.surface()),
            Suggestion::Form(seen) => format!("<{}>", seen.form),
        })
        .collect()
}

fn keys(keys: &[&str]) -> Vec<UnitKey> {
    keys.iter().map(|&key| UnitKey::from(key)).collect()
}

// [spec:hfst:sem:prediction.seen-forms/test]
// [spec:hfst:sem:prediction.session/test]
#[test]
fn a_picked_form_completes_and_sets_the_context() {
    let generator = lexc(NOUNS);
    let grammar = grammar(NOUNS_GRAMMAR);
    let bytes = model_bytes(&generator, &grammar);
    let model = Model::from_bytes(&bytes, &grammar).unwrap();
    let mut session = Session::new(walker(&generator, &grammar), 1);
    assert_eq!(
        shown(&mut session, &model, offer(true, true)),
        ["<girji>", "<guoli>", "<girjat>"]
    );
    session.pick_form(&model, "Guoli").unwrap();
    assert!(session.is_complete());
    assert_eq!(session.picked_form().unwrap().form, "guoli");
    assert!(shown(&mut session, &model, offer(true, true)).is_empty());
    assert_eq!(session.preview(&model).as_deref(), Some("guoli"));
    assert_eq!(session.commit().unwrap(), "guoli");
    assert_eq!(session.context(), keys(&["+N+Sg+Nom"]));
    assert_eq!(session.before(), ["guoli"]);
    assert!(!session.is_complete());
}

// [spec:hfst:sem:prediction.seen-forms/test]
// [spec:hfst:sem:prediction-model.form-score/test]
#[test]
fn no_letters_offer_the_likeliest_next_forms() {
    let generator = lexc(NOUNS);
    let grammar = grammar(NOUNS_GRAMMAR);
    let bytes = model_bytes(&generator, &grammar);
    let model = Model::from_bytes(&bytes, &grammar).unwrap();
    let mut session = Session::new(walker(&generator, &grammar), 1);
    session.pick_form(&model, "guoli").unwrap();
    session.commit().unwrap();
    // girji followed guoli both times; guoli and girjat back off.
    assert_eq!(
        shown(&mut session, &model, offer(true, true)),
        ["<girji>", "<guoli>", "<girjat>"]
    );
    // Typed letters keep the forms that start with them, capitals folded,
    // ranked beside the stem girji, which follows +N+Sg+Nom three times in
    // five. Entry matches stems by their letters as typed.
    session.type_letters("GI").unwrap();
    assert_eq!(
        shown(&mut session, &model, offer(true, true)),
        ["<girji>", "<girjat>"]
    );
    session.backspace();
    session.backspace();
    session.type_letters("gi").unwrap();
    assert_eq!(
        shown(&mut session, &model, offer(true, true)),
        ["<girji>", "girji", "<girjat>"]
    );
    session.pick(&UnitKey::from("girji")).unwrap();
    session.pick(&UnitKey::from("+N+Pl+Nom")).unwrap();
    assert_eq!(session.commit().unwrap(), "girjat");
    assert_eq!(session.before(), ["girjat"]);
    assert_eq!(session.context(), keys(&["+N+Pl+Nom"]));
}

// [spec:hfst:sem:prediction.seen-forms/test]
#[test]
fn units_off_offers_forms_and_forms_off_none() {
    let generator = lexc(NOUNS);
    let grammar = grammar(NOUNS_GRAMMAR);
    let bytes = model_bytes(&generator, &grammar);
    let model = Model::from_bytes(&bytes, &grammar).unwrap();
    let mut session = Session::new(walker(&generator, &grammar), 1);
    session.type_letters("g").unwrap();
    assert_eq!(
        shown(&mut session, &model, offer(true, false)),
        ["<girji>", "<guoli>", "<girjat>"]
    );
    assert_eq!(
        shown(&mut session, &model, offer(false, true)),
        ["girji", "guolli", "gáhkku"]
    );
}

// [spec:hfst:sem:prediction.seen-forms/test]
// [spec:hfst:sem:prediction.session/test]
#[test]
fn offered_forms_are_picked_and_backspace_removes_them() {
    let generator = lexc(NOUNS);
    let grammar = grammar(NOUNS_GRAMMAR);
    let bytes = model_bytes(&generator, &grammar);
    let model = Model::from_bytes(&bytes, &grammar).unwrap();
    let mut session = Session::new(walker(&generator, &grammar), 1);
    session.type_letters("gu").unwrap();
    for refused in ["girji", "guolat", "gu"] {
        let error = session.pick_form(&model, refused).expect_err(refused);
        assert_eq!(error.kind, ErrorKind::FaultyStringInput, "{refused}");
    }
    session.pick_form(&model, "guoli").unwrap();
    assert_eq!(session.typed(), "");
    assert!(session.type_letters("g").is_err());
    assert!(session.pick(&UnitKey::from("guolli")).is_err());
    assert!(session.pick_form(&model, "guoli").is_err());
    assert!(session.backspace());
    assert!(session.picked_form().is_none());
    assert!(!session.backspace());
    session.type_letters("gi").unwrap();
    session.pick(&UnitKey::from("girji")).unwrap();
    assert!(session.pick_form(&model, "girji").is_err());
}

// [spec:hfst:req:prediction.script-edges/test]
// [spec:hfst:sem:prediction.seen-forms/test]
#[test]
fn a_picked_form_passes_the_output_transliterator() {
    let generator = lexc(NOUNS);
    let grammar = grammar(NOUNS_GRAMMAR);
    let bytes = model_bytes(&generator, &grammar);
    let model = Model::from_bytes(&bytes, &grammar).unwrap();
    let lower = regex("[ G:g | I:i | U:u ]*");
    let upper = regex("[ g:G | i:I | r:R | j:J | a:A | t:T | u:U | o:O | l:L ]*");
    let script = Script {
        input: Some(&lower),
        output: Some(&upper),
    };
    let mut session = Session::new(walker(&generator, &grammar), 1).with_script(script);
    session.type_letters("GI").unwrap();
    assert_eq!(
        shown(&mut session, &model, offer(true, false)),
        ["<girji>", "<girjat>"]
    );
    session.pick_form(&model, "girjat").unwrap();
    assert_eq!(session.commit().unwrap(), "GIRJAT");
    assert_eq!(session.before(), ["girjat"]);
}
