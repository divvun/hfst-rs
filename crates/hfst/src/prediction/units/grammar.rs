use std::collections::BTreeMap;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::pattern::{Pattern, any_matches};
use super::{Label, SymbolClass, Unit, UnitKey, UnitKind, classify_symbol};
use crate::error::Result;
use crate::hfst_data_types::Symbol;
use crate::hfst_flag_diacritics::FdOperation;

/// A filter: a unit that adds nothing to the analysis but applies its flag
/// diacritics to the word in progress.
// [spec:hfst:sem:prediction-units.filter]
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filter {
    pub name: String,
    pub label: String,
    /// Flag diacritics, written as the generator writes them.
    pub flags: Vec<String>,
}

impl Filter {
    /// The unit that picks this filter. Its symbols are the filter's flag
    /// diacritics, which the walk applies in their order.
    pub fn unit(&self) -> Unit {
        Unit {
            kind: UnitKind::Filter,
            symbols: self.flags.iter().map(Symbol::new).collect(),
        }
    }

    fn is_picked_by(&self, unit: &Unit) -> bool {
        unit.kind == UnitKind::Filter
            && self.flags.len() == unit.symbols.len()
            && self.flags.iter().zip(&unit.symbols).all(|(f, s)| f == s)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WordClass {
    name: String,
    patterns: Vec<Pattern>,
}

/// The grammar file's keys, as `prediction-units.format` lists them.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GrammarFile {
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    boundary: Vec<Pattern>,
    #[serde(default)]
    prefix: Vec<Pattern>,
    #[serde(default)]
    enclitic: Vec<Pattern>,
    #[serde(default)]
    hidden: Vec<Pattern>,
    #[serde(default)]
    filter: Vec<Filter>,
    #[serde(default)]
    labels: BTreeMap<String, String>,
    #[serde(default)]
    class: Vec<WordClass>,
}

/// One language's unit grammar, read from its TOML file.
// [spec:hfst:def:prediction-units.grammar+1]
#[derive(Clone, Debug)]
pub struct UnitGrammar {
    file: GrammarFile,
    hash: [u8; 32],
}

fn grammar_error(cause: impl std::fmt::Display) -> crate::error::Error {
    crate::err!(Hfst, format!("unit grammar: {cause}"))
}

impl UnitGrammar {
    /// Read a grammar. A key the format does not list, a missing field, a
    /// `tags` entry that is not one character and a filter flag that is not
    /// a flag diacritic are refused here; what needs the generator's alphabet
    /// is refused by [`UnitGrammar::validate`].
    // [spec:hfst:syn:prediction-units.format+1]
    pub fn from_toml(bytes: &[u8]) -> Result<UnitGrammar> {
        let text = std::str::from_utf8(bytes).map_err(grammar_error)?;
        let file: GrammarFile = toml::from_str(text).map_err(grammar_error)?;
        if let Some(tag) = file.tags.iter().find(|tag| tag.chars().count() != 1) {
            return Err(grammar_error(format!(
                "tags lists {tag:?}, which is not one character"
            )));
        }
        for filter in &file.filter {
            if let Some(flag) = filter
                .flags
                .iter()
                .find(|flag| !FdOperation::is_diacritic(flag))
            {
                return Err(grammar_error(format!(
                    "filter {:?} lists {flag:?}, which is not a flag diacritic",
                    filter.name
                )));
            }
        }
        Ok(UnitGrammar {
            file,
            hash: Sha256::digest(bytes).into(),
        })
    }

    /// Check the grammar against its generator's analysis-side alphabet.
    /// Every `tags` entry and every filter flag must be in it, and every
    /// pattern must match some tag in it.
    // [spec:hfst:syn:prediction-units.format+1]
    pub fn validate<S: AsRef<str>>(&self, analysis_alphabet: &[S]) -> Result<()> {
        let in_alphabet = |wanted: &str| analysis_alphabet.iter().any(|s| s.as_ref() == wanted);
        if let Some(tag) = self.file.tags.iter().find(|tag| !in_alphabet(tag)) {
            return Err(grammar_error(format!(
                "tags lists {tag:?}, which the generator lacks"
            )));
        }
        let tags: Vec<&str> = analysis_alphabet
            .iter()
            .map(AsRef::as_ref)
            .filter(|symbol| self.classify(symbol) == SymbolClass::Tag)
            .collect();
        for (key, pattern) in self.patterns() {
            if !tags.iter().any(|tag| pattern.matches(tag)) {
                return Err(grammar_error(format!(
                    "{key} pattern {:?} matches no tag in the generator's alphabet",
                    pattern.as_str()
                )));
            }
        }
        for filter in &self.file.filter {
            for flag in &filter.flags {
                if !in_alphabet(flag) {
                    return Err(grammar_error(format!(
                        "filter {:?} sets {flag}, which the generator lacks",
                        filter.name
                    )));
                }
            }
        }
        Ok(())
    }

    /// Every pattern, with the key that holds it.
    fn patterns(&self) -> impl Iterator<Item = (String, &Pattern)> {
        let lists = [
            ("boundary", &self.file.boundary),
            ("prefix", &self.file.prefix),
            ("enclitic", &self.file.enclitic),
            ("hidden", &self.file.hidden),
        ];
        let keyed = lists
            .into_iter()
            .flat_map(|(key, patterns)| patterns.iter().map(move |p| (key.to_owned(), p)));
        let classes = self.file.class.iter().flat_map(|class| {
            let key = format!("class {:?}", class.name);
            class.patterns.iter().map(move |p| (key.clone(), p))
        });
        keyed.chain(classes)
    }

    /// Classify one symbol of the generator's analysis side as
    /// [`classify_symbol`] does, except that a symbol the grammar lists under
    /// `tags` is a tag.
    // [spec:hfst:def:prediction-units.grammar+1]
    pub fn classify(&self, symbol: &str) -> SymbolClass {
        let class = classify_symbol(symbol);
        if class == SymbolClass::Letter && self.file.tags.iter().any(|tag| tag == symbol) {
            SymbolClass::Tag
        } else {
            class
        }
    }

    /// The SHA-256 of the bytes the grammar was read from.
    // [spec:hfst:def:prediction-units.grammar+1]
    pub fn hash(&self) -> [u8; 32] {
        self.hash
    }

    // [spec:hfst:sem:prediction-units.boundary+1]
    pub fn is_boundary(&self, tag: &str) -> bool {
        any_matches(&self.file.boundary, tag)
    }

    // [spec:hfst:sem:prediction-units.prefix]
    pub fn is_prefix(&self, tag: &str) -> bool {
        any_matches(&self.file.prefix, tag)
    }

    // [spec:hfst:sem:prediction-units.enclitic]
    pub fn is_enclitic(&self, tag: &str) -> bool {
        any_matches(&self.file.enclitic, tag)
    }

    // [spec:hfst:sem:prediction-units.hidden]
    pub fn is_hidden(&self, tag: &str) -> bool {
        any_matches(&self.file.hidden, tag)
    }

    /// The grammar's filters, in the file's order.
    pub fn filters(&self) -> &[Filter] {
        &self.file.filter
    }

    /// A tag the grammar hides is left out; letters, flags and other tags
    /// stay, in order.
    // [spec:hfst:sem:prediction-units.hidden]
    pub fn key(&self, unit: &Unit) -> UnitKey {
        let key: String = unit
            .symbols
            .iter()
            .filter(|symbol| !(self.classify(symbol) == SymbolClass::Tag && self.is_hidden(symbol)))
            .map(Symbol::as_str)
            .collect();
        UnitKey(Symbol::new(key))
    }

    /// The name of the first class, in the grammar's order, with a pattern
    /// that matches one of the unit's tags. Hidden tags count: they stay in
    /// the unit and the spec leaves them out of only its key and label.
    // [spec:hfst:sem:prediction-units.classes]
    pub fn class_of(&self, unit: &Unit) -> Option<&str> {
        let tags = unit
            .symbols
            .iter()
            .filter(|symbol| self.classify(symbol) == SymbolClass::Tag);
        self.file
            .class
            .iter()
            .find(|class| tags.clone().any(|tag| any_matches(&class.patterns, tag)))
            .map(|class| class.name.as_str())
    }

    /// The class of a word in progress: that of its latest unit that has one.
    // [spec:hfst:sem:prediction-units.classes]
    pub fn word_class(&self, units: &[Unit]) -> Option<&str> {
        units.iter().rev().find_map(|unit| self.class_of(unit))
    }

    // [spec:hfst:sem:prediction-units.labels]
    pub fn label(&self, unit: &Unit) -> Label {
        if let Some(filter) = self.file.filter.iter().find(|f| f.is_picked_by(unit)) {
            return Label::Text(filter.label.clone());
        }
        let key = self.key(unit);
        if let Some(label) = self.file.labels.get(key.as_str()) {
            return Label::Text(label.clone());
        }
        match unit.kind {
            UnitKind::Stem => Label::Text(
                unit.symbols
                    .iter()
                    .filter(|symbol| self.classify(symbol) == SymbolClass::Letter)
                    .map(Symbol::as_str)
                    .collect(),
            ),
            UnitKind::Ending => Label::CompleteWord,
            UnitKind::Prefix | UnitKind::Boundary | UnitKind::Enclitic | UnitKind::Filter => {
                Label::Text(key.as_str().to_owned())
            }
        }
    }
}
