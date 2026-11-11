use std::collections::HashMap;

use super::corpus::is_character;
use super::report::Typed;
use crate::error::Result;

/// Suggestions recorded from another system: for each word and each of its
/// prefixes, what that system shows, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recording {
    pub name: String,
    /// By word, then by prefix.
    suggestions: HashMap<String, HashMap<String, Vec<String>>>,
}

impl Recording {
    /// Read a recording: one line for each word and prefix, the word, a tab,
    /// the prefix and then each suggestion after a tab. The prefix is empty
    /// on the line for a word's start. A prefix with no line has no
    /// suggestions. Two lines for one word and prefix must agree.
    // [spec:hfst:req:prediction-evaluation.external]
    pub fn parse(name: &str, text: &str) -> Result<Recording> {
        let mut suggestions: HashMap<String, HashMap<String, Vec<String>>> = HashMap::new();
        for (number, line) in text.lines().enumerate() {
            let line = line.strip_suffix('\r').unwrap_or(line);
            if line.is_empty() {
                continue;
            }
            let refuse = |cause: &str| {
                crate::err!(
                    FaultyStringInput,
                    format!("recording {name:?}, line {}: {cause}", number + 1)
                )
            };
            let mut fields = line.split('\t');
            let (Some(word), Some(prefix)) = (fields.next(), fields.next()) else {
                return Err(refuse("no tab after the word"));
            };
            if !word.starts_with(prefix) {
                return Err(refuse(&format!("{prefix:?} does not start {word:?}")));
            }
            let shown: Vec<String> = fields.map(str::to_owned).collect();
            let prefixes = suggestions.entry(word.to_owned()).or_default();
            match prefixes.get(prefix) {
                Some(known) if *known != shown => {
                    return Err(refuse(&format!(
                        "a second line for {word:?} after {prefix:?}, with other suggestions"
                    )));
                }
                Some(_) => {}
                None => {
                    prefixes.insert(prefix.to_owned(), shown);
                }
            }
        }
        Ok(Recording {
            name: name.to_owned(),
            suggestions,
        })
    }

    /// What the system shows for `word` after `prefix`, in order.
    pub fn suggestions(&self, word: &str, prefix: &str) -> &[String] {
        self.suggestions
            .get(word)
            .and_then(|prefixes| prefixes.get(prefix))
            .map_or(&[], Vec::as_slice)
    }

    /// The simulated user picks the longest suggestion the word starts
    /// with and that is longer than the prefix, which becomes the prefix, or
    /// types the word's next letter. The word is committed, for no tap, once
    /// the prefix is the whole word.
    // [spec:hfst:req:prediction-evaluation.external]
    // [spec:hfst:def:prediction-evaluation.keystrokes]
    pub(super) fn type_word(&self, word: &str) -> Typed {
        let letters: Vec<char> = word.chars().collect();
        let mut typed = 0;
        let mut taps = 0;
        while typed < letters.len() {
            let prefix: String = letters[..typed].iter().collect();
            let picked = self
                .suggestions(word, &prefix)
                .iter()
                .filter(|shown| word.starts_with(shown.as_str()))
                .map(|shown| shown.chars().count())
                .filter(|&length| length > typed)
                .max();
            match picked {
                Some(length) => {
                    taps += 1;
                    typed = length;
                }
                None => {
                    taps += usize::from(is_character(letters[typed]));
                    typed += 1;
                }
            }
        }
        Typed {
            taps,
            ..Typed::default()
        }
    }
}
