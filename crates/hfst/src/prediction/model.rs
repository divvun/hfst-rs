//! A prediction model: how often each unit, and each run of units, occurs in
//! a language's analysed text, written to one file and read in place
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
//! | 8 | 4 | format version, 1 |
//! | 12 | 32 | the SHA-256 hash of the unit grammar the model was built with |
//! | 44 | 4 | order, the length of the longest run counted, at least 1 |
//! | 48 | 4 | backoff: 0 drop-oldest, 1 stupid-backoff |
//! | 52 | 4 | ambiguity: 0 first reading, 1 equal shares |
//! | 56 | 8 | the sum of all unit counts, an `f64` |
//! | 64 | 8 × (order + 3) | the table directory |
//!
//! Each directory entry is a table's offset and its number of records, two
//! `u32`s. The entries are, in order: the string table, whose records are
//! bytes; the unit table; the class table; the face table; and one run table
//! for each length from 2 to the order.
//!
//! - The string table holds the UTF-8 of every unit key and class name, with
//!   nothing between them.
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
//! - A record of the run table for length n is n unit ids, `u32`s, then the
//!   run's count, an `f32`: 4n + 4 bytes. The records are sorted by their ids
//!   in order, which is the order of their keys.
//!
//! A count is how many times the unit or run was seen. A token counted at an
//! equal share of one over its readings adds a fraction, so counts are
//! `f32`.

mod build;
mod format;
mod input;
mod read;
mod write;

#[cfg(test)]
mod build_tests;
#[cfg(test)]
pub(super) mod fixture;
#[cfg(test)]
mod input_tests;
#[cfg(test)]
mod read_tests;
#[cfg(test)]
mod score_tests;
#[cfg(test)]
mod write_tests;

pub use build::{BuildReport, ModelBuilder};
pub use input::generator_accepts;
pub use read::Model;

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
