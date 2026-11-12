use std::ops::Range;

use crate::error::Result;
pub use crate::prediction::model::is_character;
use crate::prediction::units::{AnalysisTokenizer, Unit, UnitGrammar, UnitKey};

/// One token of analysed text in hfst-lookup's format.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token<'t> {
    pub surface: &'t str,
    /// In the order hfst-lookup wrote them; empty when it found none.
    pub readings: Vec<&'t str>,
    /// The bytes of the token's lines in the text, with the blank line that
    /// ends them.
    pub span: Range<usize>,
}

/// What the simulated user builds a word from: the reading the model
/// builder counts first for its token, cut into units.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub reading: String,
    pub units: Vec<Unit>,
    pub keys: Vec<UnitKey>,
}

/// Analysed text, in text order, with each token's target.
#[derive(Clone, Debug)]
pub struct Corpus<'t> {
    text: &'t str,
    tokens: Vec<Token<'t>>,
    targets: Vec<Option<Target>>,
}

/// One of the k parts of a corpus, and the parts its model is built from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fold {
    /// The tokens scored, by index.
    pub scored: Range<usize>,
    /// The tokens before and after the scored part. Each range is a text of
    /// its own, so no run of units the model counts spans the part left out.
    pub built_from: Vec<Range<usize>>,
}

impl<'t> Corpus<'t> {
    /// Read hfst-lookup output and find each token's target: its first
    /// reading that `accepts` takes and the grammar cuts into units, as the
    /// model builder counts a token's first reading.
    pub fn read(
        text: &'t str,
        grammar: &UnitGrammar,
        tokenizer: &AnalysisTokenizer,
        accepts: &dyn Fn(&str) -> bool,
    ) -> Result<Corpus<'t>> {
        let tokens = read_tokens(text)?;
        let targets = tokens
            .iter()
            .map(|token| {
                token.readings.iter().find_map(|&reading| {
                    if !accepts(reading) {
                        return None;
                    }
                    let symbols = tokenizer.tokenize(reading).ok()?;
                    let units = grammar.segment(&symbols).ok()?;
                    Some(Target {
                        reading: reading.to_owned(),
                        keys: units.iter().map(|unit| grammar.key(unit)).collect(),
                        units,
                    })
                })
            })
            .collect();
        Ok(Corpus {
            text,
            tokens,
            targets,
        })
    }

    pub fn tokens(&self) -> &[Token<'t>] {
        &self.tokens
    }

    /// None for a token with no reading the model can count.
    pub fn target(&self, token: usize) -> Option<&Target> {
        self.targets.get(token).and_then(Option::as_ref)
    }

    /// The text of a run of tokens, as the model builder reads it.
    pub fn text_of(&self, tokens: Range<usize>) -> &'t str {
        if tokens.is_empty() {
            return "";
        }
        let (Some(first), Some(last)) = (
            self.tokens.get(tokens.start),
            self.tokens.get(tokens.end - 1),
        ) else {
            return "";
        };
        &self.text[first.span.start..last.span.end]
    }

    /// The corpus cut into k contiguous parts of nearly equal numbers of
    /// tokens. Each part is scored by a model built from the others.
    // [spec:hfst:req:prediction-evaluation.held-out]
    pub fn folds(&self, k: usize) -> Result<Vec<Fold>> {
        let n = self.tokens.len();
        if k < 2 {
            crate::bail!(
                Hfst,
                format!(
                    "prediction evaluation: {k} parts; a model must be built from at least one part it does not score"
                )
            );
        }
        if n < k {
            crate::bail!(
                Hfst,
                format!("prediction evaluation: {n} tokens cannot be split into {k} parts")
            );
        }
        let bound = |part: usize| part * n / k;
        Ok((0..k)
            .map(|part| Fold {
                scored: bound(part)..bound(part + 1),
                built_from: [0..bound(part), bound(part + 1)..n]
                    .into_iter()
                    .filter(|range| !range.is_empty())
                    .collect(),
            })
            .collect())
    }
}

/// Split hfst-lookup output into tokens: lines of a surface form, a reading
/// and a weight separated by tabs, with a blank line after each token. A
/// token hfst-lookup could not analyse has the one reading `surface+?`,
/// which is left out.
fn read_tokens(text: &str) -> Result<Vec<Token<'_>>> {
    let mut tokens = Vec::new();
    let mut open: Option<Token<'_>> = None;
    let mut at = 0;
    for (number, line) in text.split_inclusive('\n').enumerate() {
        let start = at;
        at += line.len();
        let line = line.trim_end_matches(['\n', '\r']);
        if line.is_empty() {
            if let Some(mut token) = open.take() {
                token.span.end = at;
                tokens.push(token);
            }
            continue;
        }
        let mut fields = line.split('\t');
        let (Some(surface), Some(reading), Some(_), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            crate::bail!(
                FaultyStringInput,
                format!(
                    "analysed text, line {}: not a surface form, a reading and a weight separated by tabs",
                    number + 1
                )
            );
        };
        let token = open.get_or_insert(Token {
            surface,
            readings: Vec::new(),
            span: start..at,
        });
        if token.surface != surface {
            crate::bail!(
                FaultyStringInput,
                format!(
                    "analysed text, line {}: {surface:?} follows {:?} with no blank line between",
                    number + 1,
                    token.surface
                )
            );
        }
        token.span.end = at;
        if reading.strip_suffix("+?") != Some(surface) {
            token.readings.push(reading);
        }
    }
    tokens.extend(open);
    Ok(tokens)
}

/// The characters of a word that the measure counts.
// [spec:hfst:def:prediction-evaluation.keystrokes]
pub fn characters(word: &str) -> usize {
    word.chars().filter(|&c| is_character(c)).count()
}
