//! A prediction model: how often each unit, and each run of units, occurs in
//! a language's analysed text, and how often each word form and each run of
//! forms does, written to one file and read in place
//! (`docs/spec/prediction-model.md`).
//!
//! # The file
//!
//! Integers are little-endian. Every offset counts bytes from the start of
//! the file.
//!
//! | offset | bytes | field |
//! |---|---|---|
//! | 0 | 8 | `HFSTPRED` |
//! | 8 | 4 | format version, 2 |
//! | 12 | 32 | the SHA-256 hash of the unit grammar the model was built with |
//! | 44 | 4 | order, the length of the longest run counted, at least 1 |
//! | 48 | 4 | backoff: 0 drop-oldest, 1 stupid-backoff |
//! | 52 | 4 | ambiguity: 0 first reading, 1 equal shares |
//! | 56 | 8 | the sum of all unit counts, an `f64` |
//! | 64 | 8 | the sum of all form counts, an `f64` |
//! | 72 | 8 × (2 × order + 4) | the table directory |
//!
//! Each directory entry is a table's offset and its number of records, two
//! `u32`s. The entries are, in order: the string table, whose records are
//! bytes; the unit table; the class table; the face table; the form table;
//! the form-unit table; one run table of units for each length from 2 to the
//! order; and one run table of forms for each length from 2 to the order.
//!
//! - The string table holds the UTF-8 of every unit key, class name and word
//!   form, with nothing between them.
//! - A unit record is 16 bytes: the key's offset in the string table and its
//!   length, both `u32`, its count, an `f32`, and its class, a `u32` index
//!   into the class table or `0xFFFFFFFF` for none. The records are sorted by
//!   the bytes of their keys. A unit's id is the index of its record.
//! - A class record is 16 bytes: the name's offset in the string table and
//!   its length, then the index of its face's first entry in the face table
//!   and the face's length, all `u32`. The records are sorted by the bytes of
//!   their names.
//! - A face entry is a unit id, a `u32`. A class's entries are consecutive,
//!   most frequent first.
//! - A form record is 20 bytes: the form's offset in the string table and its
//!   length, both `u32`, its count, an `f32`, then the index of its first
//!   entry in the form-unit table and the number of its entries, both `u32`.
//!   A form is a token's surface form with capitals folded ([`fold`]). The
//!   records are sorted by the bytes of their forms, so the forms that start
//!   with given letters are consecutive. A form's id is the index of its
//!   record.
//! - A form-unit entry is a unit id, a `u32`. A form's entries are
//!   consecutive: the units of the reading it was counted with most often.
//! - A record of a run table of length n is n unit ids, or n form ids, as
//!   `u32`s, then the run's count, an `f32`: 4n + 4 bytes. The records are
//!   sorted by their ids in order, which is the order of their keys or forms.
//!
//! A count is how many times the unit, form or run was seen. A token counted
//! at an equal share of one over its readings adds a fraction to its units,
//! so counts are `f32`.

use icu::properties::CodePointMapData;
use icu::properties::props::{GeneralCategory, GeneralCategoryGroup};

mod build;
mod format;
mod forms;
mod input;
mod read;
mod write;

#[cfg(test)]
mod build_tests;
#[cfg(test)]
pub(super) mod fixture;
#[cfg(test)]
mod forms_tests;
#[cfg(test)]
mod input_tests;
#[cfg(test)]
mod read_tests;
#[cfg(test)]
mod score_tests;
#[cfg(test)]
mod write_tests;

pub use build::{BuildReport, ModelBuilder};
pub use forms::{FormScores, Offered};
pub use input::generator_accepts;
pub use read::Model;

/// True for a character the measure counts: neither a space nor
/// punctuation. Typing one costs a tap, and a counted form is offered as a
/// word only when it holds one, so that punctuation, which the model counts
/// for the context, never takes the place of a word.
// [spec:hfst:def:prediction-evaluation.keystrokes]
// [spec:hfst:sem:prediction.seen-forms]
pub fn is_character(c: char) -> bool {
    !c.is_whitespace()
        && !GeneralCategoryGroup::Punctuation
            .contains(CodePointMapData::<GeneralCategory>::new().get(c))
}

/// Capitals folded, as a keyboard's shift state folds them: each character
/// by its lower-case mapping, the mapping the harness's typing compares
/// letters by. Word forms are counted and matched folded.
// [spec:hfst:req:prediction-model.word-forms]
pub fn fold(text: &str) -> String {
    text.chars().flat_map(char::to_lowercase).collect()
}

/// How a model's builder counts and how its scores back off to shorter
/// contexts (`prediction-model.order`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModelSettings {
    /// The length of the longest run of units counted. At least 1.
    pub order: usize,
    pub backoff: Backoff,
    pub ambiguity: Ambiguity,
    /// How many unit keys each class's face holds (`prediction-model.faces`).
    pub face_size: usize,
    /// The most bytes the written model may take (`prediction-model.budget`).
    /// `usize::MAX` sets no limit.
    pub size_limit: usize,
}

// [spec:hfst:sem:prediction-model.order]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backoff {
    /// Score by the counts after the longest context after which some
    /// candidate was seen.
    DropOldest,
    /// Score by the count after the context over the context's count, or 0.4
    /// times the score after a context one unit shorter.
    StupidBackoff,
}

/// How a token with more than one reading is counted
/// (`prediction-model.input`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ambiguity {
    /// Only the token's first reading the model can count.
    FirstReading,
    /// Every reading the model can count, each at an equal share of one.
    EqualShares,
}
