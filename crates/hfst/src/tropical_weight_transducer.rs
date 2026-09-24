//! Port of 'libhfst/src/implementations/TropicalWeightTransducer.{h,cc}' — the
//! OpenFST tropical-weight backend bridge between the HFST API and rustfst's
//! 'VectorFst<TropicalWeight>' (= ['StdVectorFst']).
//!
//! In C++ 'TropicalWeightTransducer' is a class of (almost entirely) STATIC
//! methods — a stateless operations wrapper over 'fst::StdVectorFst'. It is
//! modelled here as a unit struct ['TropicalWeightTransducer'] with an 'impl'
//! block of associated functions. The two stream helper classes
//! (['TropicalWeightInputStream'] / ['TropicalWeightOutputStream']) become their
//! own structs, and the 'StdArcLessThan' comparator becomes a small struct.
//!
//! 'using namespace fst;' in the C++ header is mapped onto the
//! 'hfst-openfst' adapter ('StdVectorFst', 'StdTransition' (= 'fst::StdArc'),
//! 'TropicalWeight', 'SymbolTable', 'StateId') and rustfst's algorithms, whose
//! errors propagate as ['crate::error::Error'].
//!
//! Ownership mapping for the C++ 'StdVectorFst*' signatures:
//! - factory / unary-op methods that the C++ 'new's a result and returns
//!   'StdVectorFst*' -> return owned 'StdVectorFst'.
//! - methods that take a 'StdVectorFst*' and read it -> '&StdVectorFst'.
//! - methods that take a 'StdVectorFst*' and mutate it in place (state/arc
//!   builders, 'add_to_weights', symbol-table setters, ...) -> '&mut StdVectorFst'.
//! - 'delete_transducer(StdVectorFst*)' -> dropping the owned 'StdVectorFst'.
//! - the per-state accessors 'set_final_weight', 'get_final_weight', 'is_final'
//!   and 'get_initial_state' -> rustfst's own 'set_final', 'final_weight',
//!   'is_final' and 'start'.
//! - 'represent_empty_transducer_as_having_one_state' -> nothing: the C++
//!   reassigned only its local pointer, so the caller's transducer was never
//!   changed.
//!
//! The C++ 'int64' typedef is 'i64' here.
// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.delete-transducer-fn]
// [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.delete-transducer-fn]
// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-final-weight-fn]
// [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-final-weight-fn]
// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-final-weight-fn]
// [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-final-weight-fn]
// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.is-final-fn]
// [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.is-final-fn]
// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-initial-state-fn]
// [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-initial-state-fn]
// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.represent-empty-transducer-as-having-one-state-fn]
// [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.represent-empty-transducer-as-having-one-state-fn]

#![allow(non_snake_case)]
#![allow(dead_code)] // many ported ops are only reached once the facade lands

use std::collections::{BTreeMap, BTreeSet};

use hfst_openfst::algorithms;
use hfst_openfst::prelude::*;
use hfst_openfst::rustfst::algorithms::encode::{
    EncodeTable, EncodeType, decode, encode, encode_into,
};
use hfst_openfst::rustfst::algorithms::rm_epsilon::rm_epsilon;
use hfst_openfst::{StdTransition, StdVectorFst, SymbolTable, TropicalWeight};

use crate::hfst_data_types::{
    HfstTwoLevelPaths, StringPair, StringPairSet, StringPairVector, StringVector,
};
use crate::hfst_extract_strings::ExtractStringsCb;
use crate::hfst_flag_diacritics::FdTable;
use crate::hfst_symbol_defs::{
    NumberNumberMap, NumberPair, NumberPairSet, NumberPairVector, internal_epsilon,
    internal_identity, internal_unknown,
};

// [spec:hfst:def:tropical-weight-transducer.int64]

// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.state-id]
pub type StateId = u32;

/// 'typedef std::set<std::string> StringSet' (used by the alphabet helpers).
pub type StringSet = BTreeSet<crate::hfst_data_types::Symbol>;

// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.std-arc-vector]
pub type StdArcVector = Vec<StdTransition>;

// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.std-arc-less-than]
pub struct StdArcLessThan;

#[allow(dead_code)]
impl StdArcLessThan {
    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.std-arc-less-than.operator-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.std-arc-less-than.operator-fn]
    // Standard StdArc strict ordering (ilabel, then olabel, then weight, then
    // target state). The C++ declares this comparator but never defines or uses
    // it; a faithful total order keeps it correct rather than panicking.
    pub fn operator_call(&self, arc1: &StdTransition, arc2: &StdTransition) -> bool {
        if arc1.ilabel != arc2.ilabel {
            return arc1.ilabel < arc2.ilabel;
        }
        if arc1.olabel != arc2.olabel {
            return arc1.olabel < arc2.olabel;
        }
        if arc1.weight.value() != arc2.weight.value() {
            return arc1.weight.value() < arc2.weight.value();
        }
        arc1.nextstate < arc2.nextstate
    }
}

// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream]
pub struct TropicalWeightInputStream<'a> {
    filename: String,
    /// C++ holds an 'std::ifstream i_stream' plus an 'std::istream &input_stream'
    /// reference that aliases either 'i_stream' or 'std::cin'. Modelled here as
    /// the one owned buffered reader.
    input_stream: Box<dyn std::io::BufRead + 'a>,
}

// (no Default: TropicalWeightInputStream owns its reader and cannot be
// constructed without one; the no-source ctors are deferred.)

// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-output-stream]
pub struct TropicalWeightOutputStream {
    filename: String,
    /// C++ holds 'std::ofstream o_stream' + 'std::ostream &output_stream' that
    /// aliases either it or 'std::cout'. Modelled as a single owned writer.
    output_stream: Box<dyn std::io::Write>,
    hfst_format: bool,
}

/* Maps state numbers in AT&T text format to state ids used by OpenFst. */
// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.state-map]
// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.state-map]
// [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.state-map]
pub type StateMap = BTreeMap<i32, StateId>;

// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer]
pub struct TropicalWeightTransducer;

/// Map a failed rustfst algorithm to an error naming the operation.
fn openfst_error<E: std::fmt::Display>(operation: &str) -> impl FnOnce(E) -> crate::error::Error {
    move |error| crate::err!(Hfst, format!("OpenFst {operation}: {error}"))
}

/// Re-attach 'source''s symbol tables to 'target', which rustfst built fresh
/// and may have left without them (an empty composition carries none). HFST
/// reads the table back as the symbol-number map, so every result must carry
/// it.
fn carry_symbol_tables(source: &StdVectorFst, target: &mut StdVectorFst) {
    if let Some(symbols) = source.input_symbols() {
        target.set_input_symbols(std::sync::Arc::clone(symbols));
    }
    if let Some(symbols) = source.output_symbols() {
        target.set_output_symbols(std::sync::Arc::clone(symbols));
    }
}

mod alphabet;
mod compose;
mod construction;
mod determinize;
mod intersect;
mod io;
mod operations;
mod path_extraction;
mod substitute;
mod subtract;

#[cfg(test)]
mod compose_owned_tests;

#[cfg(test)]
mod flag_encode_tests;

#[cfg(test)]
mod determinize_budget_tests;
