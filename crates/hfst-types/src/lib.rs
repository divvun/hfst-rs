//! `hfst-types` — foundational data types, exceptions, and symbol definitions.
//!
//! Layer 0 of the HFST C++ -> Rust port. Wave 2 targets: `HfstDataTypes`,
//! `HfstExceptionDefs`, `HfstSymbolDefs`, string utilities. Freezes the shared
//! vocabulary every other crate reuses. Ported items carry their C++
//! `[spec:hfst:...]` ids.
//!
//! Naming convention: the FST "arc" concept is named `transition` throughout.
