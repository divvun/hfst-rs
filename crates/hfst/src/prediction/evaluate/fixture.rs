//! A small Greenlandic-like generator, its grammar, and analysed text written
//! as hfst-lookup writes it, for the harness's tests.

use super::{Faces, Variant};
use crate::prediction::model::{
    Ambiguity, Backoff, ModelBuilder, ModelSettings, generator_accepts,
};
use crate::prediction::units::{AnalysisTokenizer, UnitGrammar};
use crate::prediction::walk::fixtures_test;
use crate::transducer::Transducer;

/// Three nouns, four noun endings, two of which write the same letter, a
/// derivation to a verb with two verb endings, and an enclitic. Nothing
/// changes where morphemes meet, so a word's surface is its morphemes'
/// surfaces in a row.
pub(super) const WORDS: &str = r#"
Multichar_Symbols
+N +V +Abs +Rel +Erg +Sg +Pl +Ind +3Sg +3Pl +QAR +Der/nv +LI

LEXICON Root
Nouns ;

LEXICON Nouns
nuna:nuna N ;
nanoq:nanoq N ;
illu:illu N ;

LEXICON N
+N+Abs+Sg:0 End ;
+N+Abs+Pl:t End ;
+N+Rel+Sg:p End ;
+N+Erg+Sg:p End ;
+QAR+Der/nv:qar V ;

LEXICON V
+V+Ind+3Sg:poq End ;
+V+Ind+3Pl:put End ;

LEXICON End
# ;
+LI:li # ;
"#;

pub(super) const WORDS_GRAMMAR: &str = r#"
boundary = ["+Der/*"]
enclitic = ["+LI"]

[[class]]
name = "noun"
patterns = ["+N"]

[[class]]
name = "verb"
patterns = ["+Der/nv", "+V"]
"#;

pub(super) fn generator() -> Transducer {
    fixtures_test::lexc(WORDS)
}

pub(super) fn grammar() -> UnitGrammar {
    fixtures_test::grammar(WORDS_GRAMMAR)
}

/// hfst-lookup output for tokens given as a surface form and its readings.
/// A token with no readings is written as hfst-lookup writes one it cannot
/// analyse.
pub(super) fn lookup(tokens: &[(&str, &[&str])]) -> String {
    tokens
        .iter()
        .map(|(surface, readings)| {
            let lines: Vec<String> = match readings {
                [] => vec![format!("{surface}\t{surface}+?\tinf")],
                _ => readings
                    .iter()
                    .map(|reading| format!("{surface}\t{reading}\t0.000000"))
                    .collect(),
            };
            lines.join("\n") + "\n\n"
        })
        .collect()
}

pub(super) fn model_settings(order: usize, face_size: usize) -> ModelSettings {
    ModelSettings {
        order,
        backoff: Backoff::StupidBackoff,
        ambiguity: Ambiguity::FirstReading,
        face_size,
        size_limit: usize::MAX,
    }
}

pub(super) fn variant(
    name: &str,
    model: ModelSettings,
    shown: usize,
    faces: Option<Faces>,
) -> Variant {
    Variant {
        faces,
        ..Variant::units(name, model, shown)
    }
}

/// A model of `text` alone, built over the fixture generator.
pub(super) fn model_of(
    generator: &Transducer,
    grammar: &UnitGrammar,
    settings: ModelSettings,
    text: &str,
) -> Vec<u8> {
    let tokenizer = AnalysisTokenizer::new(generator.get_alphabet().get_symbol_table());
    let mut builder =
        ModelBuilder::new(grammar, &tokenizer, generator_accepts(generator), settings)
            .expect("the settings are valid");
    builder.add_text(text).expect("the text reads");
    builder.write().expect("the model writes")
}
