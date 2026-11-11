//! The byte layout of a model file, shared by the writer and the reader. The
//! layout is described in the module documentation of
//! [`crate::prediction::model`].

use super::{Ambiguity, Backoff};

pub(super) const MAGIC: &[u8; 8] = b"HFSTPRED";
pub(super) const VERSION: u32 = 1;

pub(super) const VERSION_AT: usize = 8;
pub(super) const HASH_AT: usize = 12;
pub(super) const ORDER_AT: usize = 44;
pub(super) const BACKOFF_AT: usize = 48;
pub(super) const AMBIGUITY_AT: usize = 52;
pub(super) const TOTAL_AT: usize = 56;
/// Where the table directory starts, and the length of everything before it.
pub(super) const DIRECTORY_AT: usize = 64;
/// One directory entry: a table's offset and its number of records.
pub(super) const DIRECTORY_ENTRY: usize = 8;
/// The string table, the unit table, the class table and the face table
/// come before the run tables in the directory.
pub(super) const FIXED_TABLES: usize = 4;

/// Key offset, key length, count, class.
pub(super) const UNIT_RECORD: usize = 16;
/// Name offset, name length, face start, face length.
pub(super) const CLASS_RECORD: usize = 16;
pub(super) const FACE_RECORD: usize = 4;
/// The class of a unit no class pattern matches.
pub(super) const NO_CLASS: u32 = u32::MAX;

/// A run of `length` units: its unit ids, then its count.
pub(super) fn run_record(length: usize) -> usize {
    4 * length + 4
}

/// The bytes before the first table of a model of this order.
pub(super) fn header_len(order: usize) -> usize {
    DIRECTORY_AT + DIRECTORY_ENTRY * (FIXED_TABLES + order - 1)
}

pub(super) fn backoff_code(backoff: Backoff) -> u32 {
    match backoff {
        Backoff::DropOldest => 0,
        Backoff::StupidBackoff => 1,
    }
}

pub(super) fn backoff_from_code(code: u32) -> Option<Backoff> {
    match code {
        0 => Some(Backoff::DropOldest),
        1 => Some(Backoff::StupidBackoff),
        _ => None,
    }
}

pub(super) fn ambiguity_code(ambiguity: Ambiguity) -> u32 {
    match ambiguity {
        Ambiguity::FirstReading => 0,
        Ambiguity::EqualShares => 1,
    }
}

pub(super) fn ambiguity_from_code(code: u32) -> Option<Ambiguity> {
    match code {
        0 => Some(Ambiguity::FirstReading),
        1 => Some(Ambiguity::EqualShares),
        _ => None,
    }
}

/// The caller has checked that four bytes follow `at`.
pub(super) fn le_u32(bytes: &[u8], at: usize) -> u32 {
    let mut word = [0; 4];
    word.copy_from_slice(&bytes[at..at + 4]);
    u32::from_le_bytes(word)
}

/// The caller has checked that eight bytes follow `at`.
pub(super) fn le_f64(bytes: &[u8], at: usize) -> f64 {
    let mut word = [0; 8];
    word.copy_from_slice(&bytes[at..at + 8]);
    f64::from_le_bytes(word)
}

pub(super) fn hex(hash: &[u8]) -> String {
    hash.iter().map(|byte| format!("{byte:02x}")).collect()
}
