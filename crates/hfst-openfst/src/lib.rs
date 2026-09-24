//! 'hfst-openfst' — the OpenFST-compatible weighted-FST backend, served by
//! ['rustfst'] (the 'necessary-nu/rustfst' fork, a git submodule at 'rustfst/').
//!
//! This crate is a **thin adapter**: it re-exports rustfst's 'VectorFst', the
//! tropical semiring and 'SymbolTable' for the HFST facade's
//! 'TropicalWeightTransducer', and adds the few algorithms rustfst lacks. The
//! backend calls rustfst's own algorithms directly. It is NOT a
//! reimplementation — porting OpenFST 1:1 was ~30K LOC and rejected in favour
//! of rustfst.
//!
//! Fidelity: rustfst is OpenFST-compatible (binary format, pynini-validated).
//! Divergences from OpenFST are tolerated unless a ported HFST test proves one;
//! the fix then lands in the rustfst fork (upstreamable), with the in-tree
//! 'openfst/' clone as the behavioural reference.
//!
//! Note: rustfst names transitions 'Tr'; at its API boundary we use its names,
//! while HFST-side code uses 'transition'.

pub use rustfst;
// The rustfst prelude brings the Fst/MutableFst/ExpandedFst traits, the
// algorithms, the semirings and 'Tr' into scope for downstream callers.
pub use rustfst::prelude;

pub mod algorithms;
pub mod compose_storage;
pub mod flag_overlay_compose;

/// 'fst::TropicalWeight' — the tropical semiring (min, +) over 'f32'.
pub type TropicalWeight = rustfst::semirings::TropicalWeight;

/// 'fst::StdArc' — a tropical-weighted transition (rustfst 'Tr').
pub type StdTransition = rustfst::Tr<TropicalWeight>;

/// 'fst::StdVectorFst' — the mutable tropical-weighted vector FST that HFST's
/// 'TropicalWeightTransducer' is built on.
pub type StdVectorFst = rustfst::fst_impls::VectorFst<TropicalWeight>;

pub use rustfst::SymbolTable;
