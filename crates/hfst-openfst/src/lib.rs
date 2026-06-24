//! `hfst-openfst` — the OpenFST-compatible weighted-FST backend, served by
//! [`rustfst`] (the `necessary-nu/rustfst` fork, a git submodule at `rustfst/`).
//!
//! This crate is a **thin adapter**: it re-exports / wraps rustfst's
//! `VectorFst`, the tropical/log semirings, `SymbolTable`, and algorithms so the
//! HFST facade's `Tropical/LogWeightTransducer` wrappers can call them in
//! HFST-shaped terms. It is NOT a reimplementation — porting OpenFST 1:1 was
//! ~30K LOC and rejected in favour of rustfst.
//!
//! Fidelity: rustfst is OpenFST-compatible (binary format, pynini-validated).
//! Divergences from OpenFST are tolerated unless a ported HFST test proves one;
//! the fix then lands in the rustfst fork (upstreamable), with the in-tree
//! `openfst/` clone as the behavioural reference. Known gaps to add as needed:
//! `difference`, `intersect`, `prune`, `equivalent`, `eps_normalize`.
//!
//! Note: rustfst names transitions `Tr`; at its API boundary we use its names,
//! while HFST-side code uses `transition`.

pub use rustfst;
