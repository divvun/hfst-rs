//! A unit grammar: how the analysis side of one language's generator divides
//! into units, the pieces a user picks while building a word
//! (`docs/spec/prediction-units.md`).

mod grammar;
mod pattern;
mod segment;
mod tokenizer;

#[cfg(test)]
mod grammar_tests;
#[cfg(test)]
mod segment_tests;

pub use grammar::{Filter, UnitGrammar};
pub use tokenizer::AnalysisTokenizer;

use crate::hfst_data_types::Symbol;
use crate::hfst_flag_diacritics::FdOperation;
use crate::hfst_symbol_defs::{is_default, is_epsilon, is_identity, is_unknown};

/// What a symbol on a generator's analysis side is to a unit grammar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SymbolClass {
    /// A symbol of one character the grammar does not list under `tags`.
    Letter,
    Flag,
    /// Any other symbol.
    Tag,
    /// HFST's own epsilon, unknown, identity and default symbols, and the
    /// empty string. No analysis spells them, so they are none of the
    /// above: the walk follows epsilon without reading a symbol.
    Reserved,
}

// [spec:hfst:def:prediction-units.grammar+1]
/// Classify one symbol of a generator's analysis side as if no grammar
/// listed `tags`. A letter is one character, not one byte, so `ŋ` is a
/// letter. A grammar can make a one-character symbol a tag, as North Sámi's
/// does with the `#` that closes a compound join, so a walk or a model
/// builder must classify with [`UnitGrammar::classify`] instead.
pub fn classify_symbol(symbol: &str) -> SymbolClass {
    if symbol.is_empty()
        || is_epsilon(symbol)
        || is_unknown(symbol)
        || is_identity(symbol)
        || is_default(symbol)
    {
        return SymbolClass::Reserved;
    }
    let mut chars = symbol.chars();
    if chars.next().is_some() && chars.next().is_none() {
        SymbolClass::Letter
    } else if FdOperation::is_diacritic(symbol) {
        SymbolClass::Flag
    } else {
        SymbolClass::Tag
    }
}

/// The kinds of unit (`prediction.unit`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum UnitKind {
    Stem,
    Prefix,
    /// A derivation or a compound join.
    Boundary,
    Ending,
    Enclitic,
    Filter,
}

/// One choice a user makes while building a word. A stem holds letters, a
/// filter holds its flag diacritics, and every other unit holds tags. Flag
/// diacritics the generator reads on the way to a unit are applied, not
/// kept in it: an analysis never spells them, so a unit that kept them
/// would have a key no model counted.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Unit {
    pub kind: UnitKind,
    pub symbols: Vec<Symbol>,
}

/// A unit's symbols with the grammar's hidden tags left out, joined into one
/// string. Two units with the same key are one choice.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnitKey(Symbol);

impl UnitKey {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<&str> for UnitKey {
    fn from(key: &str) -> Self {
        UnitKey(Symbol::new(key))
    }
}

/// What a keyboard shows for a unit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Label {
    Text(String),
    /// An ending with no entry in the labels table is shown as the complete
    /// word it makes, which only the generator can write.
    CompleteWord,
}

#[cfg(test)]
mod tests {
    use super::*;

    // [spec:hfst:def:prediction-units.grammar+1/test]
    #[test]
    fn one_character_symbols_are_letters() {
        for symbol in ["a", "ŋ", "â", "#", "+", "@"] {
            assert_eq!(classify_symbol(symbol), SymbolClass::Letter, "{symbol}");
        }
    }

    // [spec:hfst:def:prediction-units.grammar+1/test]
    #[test]
    fn flags_are_flags_and_the_rest_are_tags() {
        for flag in ["@U.person.NI@", "@D.person@", "@P.LEXNAME.#@", "@C.Num@"] {
            assert_eq!(classify_symbol(flag), SymbolClass::Flag, "{flag}");
        }
        for tag in ["+N", "+Der/vn", "TA+", "PV/e+", "@Q.no.flag@", "^T"] {
            assert_eq!(classify_symbol(tag), SymbolClass::Tag, "{tag}");
        }
    }

    #[test]
    fn hfst_internal_symbols_are_reserved() {
        for symbol in [
            "",
            "@_EPSILON_SYMBOL_@",
            "@_UNKNOWN_SYMBOL_@",
            "@_IDENTITY_SYMBOL_@",
            "@_DEFAULT_SYMBOL_@",
        ] {
            assert_eq!(classify_symbol(symbol), SymbolClass::Reserved, "{symbol:?}");
        }
    }

    #[test]
    fn unit_keys_order_and_compare_by_their_text() {
        let noun = UnitKey::from("+N+Sg+Nom");
        let verb = UnitKey::from("+V+Ind+Prs+Sg3");
        assert!(noun < verb);
        assert_eq!(noun, UnitKey::from("+N+Sg+Nom"));
        assert_eq!(verb.as_str(), "+V+Ind+Prs+Sg3");
    }
}
