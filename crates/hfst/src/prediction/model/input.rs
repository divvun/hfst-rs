use std::cell::RefCell;
use std::collections::HashMap;

use crate::error::Result;
use crate::lookup_state::LookupState;
use crate::transducer::{Transducer, TransducerTablesInterface};

/// One token's block of hfst-lookup output.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Token<'t> {
    pub surface: &'t str,
    /// The token's readings in the order hfst-lookup wrote them. Empty when
    /// hfst-lookup found no analysis.
    pub analyses: Vec<&'t str>,
}

/// Split hfst-lookup output into tokens. A line holds a surface form, a
/// reading and a weight, separated by tabs, and a blank line ends a token.
/// hfst-lookup writes a token it cannot analyse as one reading, the surface
/// form followed by `+?`, with an infinite weight; that token has no
/// readings. A block whose lines disagree on the surface form is refused,
/// since a missing blank line would otherwise join two tokens into one.
// [spec:hfst:req:prediction-model.input]
pub(super) fn tokens(text: &str) -> Result<Vec<Token<'_>>> {
    let mut tokens = Vec::new();
    let mut block: Option<Token<'_>> = None;
    for (index, line) in text.lines().enumerate() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            tokens.extend(block.take());
            continue;
        }
        let (surface, analysis) = reading(index + 1, line)?;
        let token = block.get_or_insert(Token {
            surface,
            analyses: Vec::new(),
        });
        if token.surface != surface {
            crate::bail!(
                FaultyStringInput,
                format!(
                    "analysed text, line {}: surface form {surface:?} in the block of {:?}; a blank line must end each token",
                    index + 1,
                    token.surface
                )
            );
        }
        if analysis.strip_suffix("+?") != Some(surface) {
            token.analyses.push(analysis);
        }
    }
    tokens.extend(block);
    Ok(tokens)
}

fn reading(number: usize, line: &str) -> Result<(&str, &str)> {
    let fields: Vec<&str> = line.split('\t').collect();
    let [surface, analysis, weight] = fields[..] else {
        crate::bail!(
            FaultyStringInput,
            format!(
                "analysed text, line {number}: {} tab-separated fields, not a surface form, a reading and a weight",
                fields.len()
            )
        );
    };
    if weight.parse::<f32>().is_err() {
        crate::bail!(
            FaultyStringInput,
            format!("analysed text, line {number}: weight {weight:?} is not a number")
        );
    }
    Ok((surface, analysis))
}

/// The most answers [`generator_accepts`] keeps. Analysed text repeats its
/// readings, and a lookup on a large generator can take milliseconds.
const ANSWERS_KEPT: usize = 1 << 20;

/// Whether `generator` generates something from an analysis, for
/// [`super::ModelBuilder::new`].
// [spec:hfst:req:prediction-model.input]
pub fn generator_accepts<'g, T>(generator: &'g Transducer<T>) -> Box<dyn Fn(&str) -> bool + 'g>
where
    T: TransducerTablesInterface + 'g,
{
    let state = RefCell::new(LookupState::new(generator));
    let answers: RefCell<HashMap<String, bool>> = RefCell::new(HashMap::new());
    Box::new(move |analysis| {
        if let Some(&accepted) = answers.borrow().get(analysis) {
            return accepted;
        }
        let accepted = !state.borrow_mut().lookup_fd(analysis, 1, 0.0).is_empty();
        let mut answers = answers.borrow_mut();
        if answers.len() < ANSWERS_KEPT {
            answers.insert(analysis.to_owned(), accepted);
        }
        accepted
    })
}
