//! Two-input-stream tools: the binary_ops family (one operation applied
//! pairwise across two archives) and its close relatives.
//!
//! Contains one child module per tool:
//! - `binary_tool`
//! - `check_alpha`
//! - `compare`
//! - `compose`
//! - `concatenate`
//! - `conjunct`
//! - `disjunct`
//! - `priority_disjunct`
//! - `shuffle`
//! - `subtract`

pub mod binary_tool;
pub mod check_alpha;
pub mod compare;
pub mod compose;
pub mod concatenate;
pub mod conjunct;
pub mod disjunct;
pub mod priority_disjunct;
pub mod shuffle;
pub mod subtract;
