//! A small grammar and alphabet, and hfst-lookup output written from
//! analyses, for the model's tests.

use std::sync::LazyLock;

use super::{Ambiguity, Backoff, BuildReport, ModelBuilder, ModelSettings};
use crate::error::Result;
use crate::prediction::units::{AnalysisTokenizer, UnitGrammar, UnitKey};

pub(crate) const GRAMMAR: &str = r#"
boundary = ["+Der/*"]
prefix = ["TA+"]
enclitic = ["+LI"]
hidden = ["+Gram/*"]

[[class]]
name = "noun"
patterns = ["+Der/vn", "+N"]

[[class]]
name = "verb"
patterns = ["+Der/nv", "+V"]
"#;

const TAGS: &[&str] = &[
    "+Der/vn", "+Der/nv", "+NIQ", "+LI", "+N", "+V", "+Ind", "+Abs", "+Rel", "+Sg", "+Pl", "+3Sg",
    "+3Pl", "+Gram/IV", "TA+",
];

pub(crate) fn grammar() -> UnitGrammar {
    UnitGrammar::from_toml(GRAMMAR.as_bytes()).expect("the test grammar reads")
}

pub(crate) fn tokenizer() -> AnalysisTokenizer {
    let mut alphabet: Vec<String> = TAGS.iter().map(|tag| (*tag).to_owned()).collect();
    alphabet.extend(('a'..='z').map(String::from));
    AnalysisTokenizer::new(&alphabet)
}

pub(crate) fn settings(order: usize, backoff: Backoff) -> ModelSettings {
    ModelSettings {
        order,
        backoff,
        ambiguity: Ambiguity::FirstReading,
        face_size: 3,
        size_limit: usize::MAX,
    }
}

/// hfst-lookup output for tokens given as their readings. A token with no
/// readings is written as hfst-lookup writes one it cannot analyse.
pub(crate) fn lookup(tokens: &[&[&str]]) -> String {
    let mut text = String::new();
    for (number, readings) in tokens.iter().enumerate() {
        let surface = format!("w{number}");
        if readings.is_empty() {
            text.push_str(&format!("{surface}\t{surface}+?\tinf\n"));
        }
        for reading in *readings {
            text.push_str(&format!("{surface}\t{reading}\t0.000000\n"));
        }
        text.push('\n');
    }
    text
}

/// hfst-lookup output for tokens given as a surface form and its readings.
/// A token with no readings is written as hfst-lookup writes one it cannot
/// analyse.
pub(crate) fn lookup_forms(tokens: &[(&str, &[&str])]) -> String {
    let mut text = String::new();
    for (surface, readings) in tokens {
        if readings.is_empty() {
            text.push_str(&format!("{surface}\t{surface}+?\tinf\n"));
        }
        for reading in *readings {
            text.push_str(&format!("{surface}\t{reading}\t0.000000\n"));
        }
        text.push('\n');
    }
    text
}

/// A model of `text`, with every reading accepted.
pub(crate) fn build_text(settings: ModelSettings, text: &str) -> Vec<u8> {
    let mut builder = builder(settings, |_| true).expect("the settings are valid");
    builder.add_text(text).expect("the text reads");
    builder.write().expect("the model writes")
}

static GRAMMAR_READ: LazyLock<UnitGrammar> = LazyLock::new(grammar);
static TOKENIZER: LazyLock<AnalysisTokenizer> = LazyLock::new(tokenizer);

pub(crate) fn builder(
    settings: ModelSettings,
    accepts: impl Fn(&str) -> bool + 'static,
) -> Result<ModelBuilder<'static>> {
    ModelBuilder::new(&GRAMMAR_READ, &TOKENIZER, Box::new(accepts), settings)
}

/// A model of `tokens`, with every reading accepted.
pub(crate) fn build(settings: ModelSettings, tokens: &[&[&str]]) -> (Vec<u8>, BuildReport) {
    build_accepting(settings, tokens, |_| true)
}

pub(crate) fn build_accepting(
    settings: ModelSettings,
    tokens: &[&[&str]],
    accepts: impl Fn(&str) -> bool + 'static,
) -> (Vec<u8>, BuildReport) {
    let mut builder = builder(settings, accepts).expect("the settings are valid");
    let report = builder.add_text(&lookup(tokens)).expect("the text reads");
    (builder.write().expect("the model writes"), report)
}

pub(crate) fn keys(keys: &[&str]) -> Vec<UnitKey> {
    keys.iter().map(|&key| UnitKey::from(key)).collect()
}

/// Stems and endings for hand-counted tests: the units run nuna, +N+Abs+Sg,
/// oqar, +V+Ind+3Sg, nuna, +N+Abs+Sg, sini, +V+Ind+3Sg, nuna, +N+Rel+Sg.
pub(crate) const STORY: &[&[&str]] = &[
    &["nuna+N+Abs+Sg"],
    &["oqar+V+Ind+3Sg"],
    &["nuna+N+Abs+Sg"],
    &["sini+V+Ind+3Sg"],
    &["nuna+N+Rel+Sg"],
];
