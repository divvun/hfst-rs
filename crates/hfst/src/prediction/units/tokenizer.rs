use std::collections::HashMap;

use super::{SymbolClass, classify_symbol};
use crate::error::Result;
use crate::hfst_data_types::Symbol;

/// Splits an analysis string into the symbols of a generator's analysis
/// side, taking the longest symbol at each place. Flag diacritics and
/// reserved symbols are left out of the alphabet, since no analysis spells
/// them.
#[derive(Clone, Debug)]
pub struct AnalysisTokenizer {
    /// A trie over the alphabet's symbols. Node 0 is the root.
    nodes: Vec<TrieNode>,
}

#[derive(Clone, Debug, Default)]
struct TrieNode {
    children: HashMap<char, usize>,
    symbol: Option<Symbol>,
}

impl AnalysisTokenizer {
    pub fn new<S: AsRef<str>>(analysis_alphabet: &[S]) -> AnalysisTokenizer {
        let mut nodes = vec![TrieNode::default()];
        for symbol in analysis_alphabet.iter().map(AsRef::as_ref) {
            match classify_symbol(symbol) {
                SymbolClass::Letter | SymbolClass::Tag => {}
                SymbolClass::Flag | SymbolClass::Reserved => continue,
            }
            let mut node = 0;
            for c in symbol.chars() {
                let next = nodes.len();
                node = *nodes[node].children.entry(c).or_insert(next);
                if node == next {
                    nodes.push(TrieNode::default());
                }
            }
            nodes[node].symbol = Some(Symbol::new(symbol));
        }
        AnalysisTokenizer { nodes }
    }

    /// The error names the character, counted from 0, where no symbol of the
    /// alphabet starts.
    pub fn tokenize(&self, analysis: &str) -> Result<Vec<Symbol>> {
        let mut symbols = Vec::new();
        let mut rest = analysis;
        let mut position = 0;
        while !rest.is_empty() {
            let Some((symbol, length)) = self.longest_symbol(rest) else {
                crate::bail!(
                    SymbolNotFound,
                    format!(
                        "analysis {analysis:?}: no symbol of the generator's alphabet starts at character {position}, {rest:?}"
                    )
                );
            };
            symbols.push(symbol.clone());
            position += rest[..length].chars().count();
            rest = &rest[length..];
        }
        Ok(symbols)
    }

    /// The length is in bytes.
    fn longest_symbol(&self, text: &str) -> Option<(&Symbol, usize)> {
        let mut node = 0;
        let mut longest = None;
        for (at, c) in text.char_indices() {
            let Some(&next) = self.nodes[node].children.get(&c) else {
                break;
            };
            node = next;
            if let Some(symbol) = &self.nodes[node].symbol {
                longest = Some((symbol, at + c.len_utf8()));
            }
        }
        longest
    }
}
