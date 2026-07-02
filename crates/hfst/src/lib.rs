//! 'hfst' — the literal 1:1 port of 'libhfst/src'.
//!
//! Per the Wave-2 plan, the whole of 'libhfst/src' is ported into this single
//! crate (one Rust module per C++ file), because the C++ sources form
//! cross-file cycles that no layered crate split can express but intra-crate
//! modules can. The dormant 'hfst-types'/'-core'/'-ol' crates remain as Wave-4
//! redistribution targets; 'hfst-openfst' (the rustfst adapter) is the real
//! downstream backend dependency.
//!
//! Faithfulness over idiom: C++ identifiers are kept verbatim (hence the
//! crate-wide naming lints below), bugs are preserved, and 'unsafe'/raw
//! pointers mirror the C++ where it uses them.

#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
// Wave-4 idiom: every unsafe op must sit in an explicit, justified `unsafe {}`
// block — even inside an `unsafe fn`. The only remaining unsafe is the flagged
// raw-pointer islands (search `SAFETY-ISLAND`).
#![deny(unsafe_op_in_unsafe_fn)]

pub mod alphabet;
pub mod compose_intersect_fst;
pub mod compose_intersect_lexicon;
pub mod compose_intersect_rule;
pub mod compose_intersect_rule_pair;
pub mod compose_intersect_utilities;
pub mod convert;
pub mod convert_log_weight_transducer;
pub mod convert_ol_transducer;
pub mod convert_transducer_format;
pub mod convert_tropical_weight_transducer;
pub mod error;
pub mod expand_equivalences;
pub mod generate_model_forms;
pub mod guessify_fst;
pub mod harmonize_unknown_and_identity_symbols;
pub mod hfst_basic_transducer;
pub mod hfst_basic_transition;
pub mod hfst_data_types;
pub mod hfst_epsilon_handler;
pub mod hfst_extract_strings;
pub mod hfst_flag_diacritics;
pub mod hfst_input_stream;
pub mod hfst_lookup_flag_diacritics;
pub mod hfst_ol_transducer;
pub mod hfst_output_stream;
pub mod hfst_print_dot;
pub mod hfst_print_pckimmo;
pub mod hfst_rules;
pub mod hfst_string_conversions;
pub mod hfst_strings2_fst_tokenizer;
pub mod hfst_symbol_defs;
pub mod hfst_tokenizer;
pub mod hfst_transducer;
pub mod hfst_transition;
pub mod hfst_tropical_transducer_transition_data;
pub mod hfst_xerox_rules;
pub mod io_utils;
pub mod lexc;
pub mod log_weight_transducer;
pub mod ospell;
pub mod pmatch;
pub mod pmatch_compiler;
pub mod pmatch_tokenize;
pub mod string_manipulation;
pub mod string_utils;
pub mod transducer;
pub mod tropical_weight_transducer;
pub mod twolc;
pub mod xfst_compiler;
pub mod xre;
