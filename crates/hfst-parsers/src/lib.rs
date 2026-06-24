//! `hfst-parsers` — AST→transducer evaluators over `nfst`.
//!
//! `nfst` (sibling repo) replaces the Flex/Bison front-end: text → typed AST.
//! This crate ports the `*Compiler`/`*_utils` *semantics* (the transducer-
//! building logic) faithfully, but restructured to walk nfst's ASTs instead of
//! mirroring inline bison actions — same behavior, different input form.
//! Targets: XRE, LEXC, PMATCH, TWOLC, XFST evaluators + AT&T format I/O.
