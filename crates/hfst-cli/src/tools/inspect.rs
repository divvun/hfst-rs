//! Report and metadata tools: the ones that describe, name, or slice an
//! archive rather than transform the transducers in it.
//!
//! Contains one child module per tool:
//! - `dump_alphabets`
//! - `edit_metadata`
//! - `head`
//! - `info`
//! - `name`
//! - `split`
//! - `strip_header`
//! - `tail`
//! - `traverse`

pub mod dump_alphabets;
pub mod edit_metadata;
pub mod head;
pub mod info;
pub mod name;
pub mod split;
pub mod strip_header;
pub mod tail;
pub mod traverse;
