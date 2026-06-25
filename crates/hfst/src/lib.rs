//! `hfst` — the literal 1:1 port of `libhfst/src`.
//!
//! Per the Wave-2 plan, the whole of `libhfst/src` is ported into this single
//! crate (one Rust module per C++ file), because the C++ sources form
//! cross-file cycles that no layered crate split can express but intra-crate
//! modules can. The dormant `hfst-types`/`-core`/`-ol` crates remain as Wave-4
//! redistribution targets; `hfst-openfst` (the rustfst adapter) is the real
//! downstream backend dependency.
//!
//! Faithfulness over idiom: C++ identifiers are kept verbatim (hence the
//! crate-wide naming lints below), bugs are preserved, and `unsafe`/raw
//! pointers mirror the C++ where it uses them.

#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]

pub mod convert;
pub mod convert_ol_transducer;
pub mod convert_transducer_format;
pub mod format_specifiers;
pub mod harmonize_unknown_and_identity_symbols;
pub mod hfst_basic_transducer;
pub mod hfst_basic_transition;
pub mod hfst_data_types;
pub mod hfst_epsilon_handler;
pub mod hfst_exception_defs;
pub mod hfst_flag_diacritics;
pub mod hfst_lookup_flag_diacritics;
pub mod hfst_string_conversions;
pub mod hfst_strings2_fst_tokenizer;
pub mod hfst_symbol_defs;
pub mod hfst_tokenizer;
pub mod hfst_transition;
pub mod hfst_tropical_transducer_transition_data;
pub mod ospell;
pub mod pmatch;
pub mod string_utils;
pub mod transducer;
