//! Port of the facade 'libhfst/src/HfstTransducer.{h,cc}' (+ 'HfstApply.cc'),
//! monomorphized per [dec:hfst:monomorphic-backends].
//!
//! The C++ 'HfstTransducer' was a tagged union: the field 'type'
//! ('ImplementationType') selected the active member of a raw-pointer union,
//! and every facade operation dispatched on it at runtime (the 'apply*'
//! functor family of HfstApply.cc). Here the backend is a type parameter:
//! 'HfstTransducer<B: Backend>' owns its backend directly, the former
//! per-backend closure pairs live as ['crate::backend::Backend'] /
//! ['crate::backend::AlgebraBackend'] trait methods, and each facade method is
//! a thin monomorphic wrapper ('self.fst = self.fst.method(args)'). The
//! 'apply'/'apply_bool'/'apply_n'/'apply_string_string'/'apply_binary'
//! combinators are gone; only 'apply_another''s harmonization preamble
//! survives, as the generic ['HfstTransducer::harmonize_for_binary_op'].
//!
//! Capability mismatches that the C++ reported at runtime
//! ('FunctionNotImplementedException' / 'TransducerHasWrongTypeException',
//! e.g. calling the FST algebra on an optimized-lookup backend) are now
//! compile-time impossibilities: those methods only exist on
//! 'HfstTransducer<B: AlgebraBackend>' instantiations, and the lookup surface
//! only on the two optimized-lookup instantiations.
//!
//! The ONLY runtime type decision left is at the stream boundary, where file
//! bytes carry the type as data: ['AnyTransducer'] (the one runtime sum) is
//! produced by 'HfstInputStream' readers and consumed by 'HfstOutputStream'.
//! 'ImplementationType' survives only as that stream header tag and as the
//! CLI '--format' value.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use hfst_openfst::StdVectorFst;

use crate::backend::{AlgebraBackend, Backend, FlagDiacriticOperation};
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_data_types::ImplementationType;
use crate::hfst_data_types::ImplementationType::FOMA_TYPE;
use crate::hfst_data_types::ImplementationType::SFST_TYPE;
use crate::hfst_data_types::ImplementationType::TROPICAL_OPENFST_TYPE;
use crate::hfst_data_types::ImplementationType::XFSM_TYPE;
use crate::hfst_data_types::implementations::HfstState;
use crate::hfst_data_types::{
    HfstOneLevelPaths, HfstTwoLevelPath, HfstTwoLevelPaths, PushType, StringPair, StringPairSet,
    StringPairVector, StringVector, Symbol,
};
use crate::hfst_extract_strings::{ExtractStringsCb, RetVal};
use crate::hfst_flag_diacritics::FdOperation;
use crate::hfst_symbol_defs::{
    HfstSymbolPairSubstitutions, HfstSymbolSubstitutions, StringSet, internal_epsilon,
    internal_identity, internal_unknown, is_epsilon, is_unknown,
};
use crate::hfst_tokenizer::HfstTokenizer;
use crate::lookup_state::LookupState;
use crate::transducer::{Transducer, UnweightedTables, WeightedTables};
use crate::tropical_weight_transducer::TropicalWeightTransducer;

mod alphabet;
mod binary_ops;
mod compose_intersect;
#[cfg(test)]
mod compose_intersect_tests;
mod construction;
mod conversion;
mod extraction;
mod flag_diacritics;
mod flag_ops;
mod intersect;
mod io;
mod substitution;
mod subtract;
mod unary_ops;
pub use binary_ops::{
    ShuffleCoding, substitute_one_sided_identity, substitute_unknown_identity_pairs,
};
pub use conversion::{AnyTransducer, FromAnyTransducer};
pub use flag_diacritics::get_flag_path_restriction;
pub(crate) use flag_ops::{decode_flag, encode_flag};
use flag_ops::{
    decode_flag_diacritics, encode_flag_diacritics, rename_flag_diacritics,
    substitute_input_flag_with_epsilon, substitute_one_sided_flags,
    substitute_output_flag_with_epsilon,
};
pub use io::write_to;

// -----------------------------------------------------------------------------
// Facade type aliases (the 'HfstTransducer'-dependent typedefs deferred out of
// 'HfstDataTypes.h' until the facade type exists). Generic over the backend.
// -----------------------------------------------------------------------------

/// 'typedef std::vector<HfstTransducer> HfstTransducerVector;'
// [spec:hfst:def:hfst-data-types.hfst.hfst-transducer-vector]
pub type HfstTransducerVector<B> = Vec<HfstTransducer<B>>;

/// 'typedef std::pair<HfstTransducer,HfstTransducer> HfstTransducerPair;'
// [spec:hfst:def:hfst-data-types.hfst.hfst-transducer-pair]
pub type HfstTransducerPair<B> = (HfstTransducer<B>, HfstTransducer<B>);

/// 'typedef std::vector<HfstTransducerPair> HfstTransducerPairVector;'
// [spec:hfst:def:hfst-data-types.hfst.hfst-transducer-pair-vector]
pub type HfstTransducerPairVector<B> = Vec<HfstTransducerPair<B>>;

/// The flag-diacritic self-loops that an `-F` algebra operation must expose
/// virtually.
///
/// Preparing an overlay inserts these symbols into the corresponding operand's
/// alphabet, but deliberately does not insert transitions. The selected
/// backend presents a unit-weight self-loop only when its operation engine asks
/// for that symbol at that state.
// [spec:hfst:req:virtual-flag-algebra.backend-core]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FlagDiacriticOverlay {
    /// Flags logically inserted as self-loops at every state of the left FST.
    pub left_self_loops: StringSet,
    /// Flags logically inserted as self-loops at every state of the right FST.
    pub right_self_loops: StringSet,
    /// Whether `_1` flags must precede `_2` flags between regular left-output
    /// symbols, matching HFST's two-state illegal-flag-path restriction.
    pub enforce_left_before_right: bool,
}

/// Backwards-compatible name for the overlay accepted by composition APIs.
pub type FlagDiacriticComposeOverlay = FlagDiacriticOverlay;

// -----------------------------------------------------------------------------
// Static predicates (formerly static member functions of the facade; free
// functions now so callers need no backend type parameter).
// -----------------------------------------------------------------------------

/// Whether the conversion requested can be done without losing information.
// [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-safe-conversion-fn]
// [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-safe-conversion-fn]
// [spec:hfst:def:hfst-apply.hfst.hfst-transducer.is-safe-conversion-fn]
// [spec:hfst:sem:hfst-apply.hfst.hfst-transducer.is-safe-conversion-fn]
pub fn is_safe_conversion(original: ImplementationType, converted: ImplementationType) -> bool {
    if original == converted {
        return true;
    }
    if original == TROPICAL_OPENFST_TYPE {
        if converted == SFST_TYPE {
            return false;
        }
        if converted == FOMA_TYPE {
            return false;
        }
        if converted == XFSM_TYPE {
            return false;
        }
    }
    true
}

/// Whether HFST is linked to the transducer library needed by 'ty'.
///
/// ERROR_TYPE or UNSPECIFIED_TYPE return true (handled separately by callers).
// [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-implementation-type-available-fn]
// [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-implementation-type-available-fn]
pub fn is_implementation_type_available(ty: ImplementationType) -> bool {
    // #if !HAVE_FOMA (the `foma` Cargo feature is the HAVE_FOMA switch)
    #[cfg(feature = "foma")]
    if ty == FOMA_TYPE {
        return true;
    }
    #[cfg(not(feature = "foma"))]
    if ty == FOMA_TYPE {
        return false;
    }
    // #if !HAVE_SFST
    if ty == SFST_TYPE {
        return false;
    }
    // HAVE_OPENFST and HAVE_OPENFST_LOG: no checks emitted.
    // #if !HAVE_XFSM
    if ty == XFSM_TYPE {
        return false;
    }
    let _ = ty;
    true
}

/// Whether HFST offers at least reading, writing, and conversion for 'ty'.
// [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-lean-implementation-type-available-fn]
// [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-lean-implementation-type-available-fn]
pub fn is_lean_implementation_type_available(ty: ImplementationType) -> bool {
    // #if !HAVE_FOMA (the `foma` Cargo feature is the HAVE_FOMA switch)
    #[cfg(feature = "foma")]
    if ty == FOMA_TYPE {
        return true;
    }
    #[cfg(not(feature = "foma"))]
    if ty == FOMA_TYPE {
        return false;
    }
    // #if !HAVE_SFST && !HAVE_LEAN_SFST
    if ty == SFST_TYPE {
        return false;
    }
    // HAVE_OPENFST / HAVE_OPENFST_LOG: no checks emitted.
    // #if !HAVE_XFSM
    if ty == XFSM_TYPE {
        return false;
    }
    let _ = ty;
    true
}

// [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-profile-seconds-fn]
// [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-profile-seconds-fn]
pub fn get_profile_seconds(ty: ImplementationType) -> f32 {
    if ty == ImplementationType::TROPICAL_OPENFST_TYPE {
        return TropicalWeightTransducer::get_profile_seconds();
    }
    0.0
}

// [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-special-symbol-fn]
// [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-special-symbol-fn]
pub fn is_special_symbol(symbol: &str) -> bool {
    let bytes = symbol.as_bytes();
    if bytes.len() < 4 {
        return false;
    }
    if bytes[0] == b'@'
        && bytes[bytes.len() - 1] == b'@'
        && bytes[1] == b'_'
        && bytes[bytes.len() - 2] == b'_'
    {
        return true;
    }
    false
}

// Deleted C++-only arms (the backends are compiled out and the methods were
// unconditionally 'FunctionNotImplemented' for every backend in this build;
// under [dec:hfst:monomorphic-backends] such capability mismatches are
// compile-time absences rather than runtime throws):
//   - get_symbol_pairs (SFST-only)
//     [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-symbol-pairs-fn]
//     [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-symbol-pairs-fn]
//   - remove_symbols_from_alphabet (XFSM-only)
//     [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.remove-symbols-from-alphabet-fn]
//     [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.remove-symbols-from-alphabet-fn]
//   - extract_path_transducers (SFST-only)
//     [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-path-transducers-fn]
//     [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-path-transducers-fn]
//   - write_xfsm_transducer_in_prolog_format (XFSM-only)
//     [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.write-xfsm-transducer-in-prolog-format-fn]
//     [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.write-xfsm-transducer-in-prolog-format-fn]
//   - prolog_file_to_xfsm_transducer (XFSM-only)
//     [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.prolog-file-to-xfsm-transducer-fn]
//     [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.prolog-file-to-xfsm-transducer-fn]

// -----------------------------------------------------------------------------
// The facade transducer.
// -----------------------------------------------------------------------------

/// \brief A synchronous finite-state transducer.
///
/// The backend is the type parameter ([dec:hfst:monomorphic-backends]); the
/// C++ 'type' field + 'TransducerImplementation' union are gone.
// [spec:hfst:def:hfst-transducer.hfst.hfst-transducer]
pub struct HfstTransducer<B: Backend> {
    /// The name of the transducer.
    pub(crate) name: String,
    /// rest of fst metadata ('std::map<std::string,std::string>').
    pub(crate) props: BTreeMap<String, String>,
    /// currently not used
    pub(crate) anonymous: bool,
    /// currently not used
    pub(crate) is_trie: bool,
    /// The backend implementation (owned; was 'ty' + the union).
    pub(crate) fst: B,
}

// ===== integration shims: HfstTransducer.cc engine-policy config =====
// The C++ file-static engine-policy flags (HfstTransducer.cc:84-97) are no longer
// process-global atomics. They live in an owned 'EngineConfig' threaded into the
// operations that read them; a caller that configures nothing uses
// 'EngineConfig::default()' (the C++ initial values), so behavior is unchanged.
// XFST and the CLI tools own an 'EngineConfig' and thread it into their op calls.
//
// 'minimize_even_if_already_minimal', 'minimization_algorithm' and 'harmonize_smaller'
// have no functional consumer in the ported (rustfst-backed) scope — the rustfst
// 'Minimize' / 'harmonize_copy' do not branch on them — so they are carried as inert
// config fields (faithful to the C++ public API and to their already-vestigial
// state) rather than wired into a backend.

/// Owned engine-policy configuration: the former file-static flags of
/// HfstTransducer.cc, defaulting to the C++ initial values.
#[derive(Clone, Copy, Debug)]
pub struct EngineConfig {
    // [spec:hfst:def:hfst-transducer.hfst.set-minimization-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.set-minimization-fn]
    // [spec:hfst:def:hfst-transducer.hfst.get-minimization-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.get-minimization-fn]
    pub minimization: bool,
    // [spec:hfst:def:hfst-transducer.hfst.set-minimize-even-if-already-minimal-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.set-minimize-even-if-already-minimal-fn]
    // [spec:hfst:def:hfst-transducer.hfst.get-minimize-even-if-already-minimal-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.get-minimize-even-if-already-minimal-fn]
    pub minimize_even_if_already_minimal: bool,
    // [spec:hfst:def:hfst-transducer.hfst.set-unknown-symbols-in-use-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.set-unknown-symbols-in-use-fn]
    // [spec:hfst:def:hfst-transducer.hfst.get-unknown-symbols-in-use-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.get-unknown-symbols-in-use-fn]
    pub unknown_symbols_in_use: bool,
    // [spec:hfst:def:hfst-transducer.hfst.set-flag-is-epsilon-in-composition-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.set-flag-is-epsilon-in-composition-fn]
    // [spec:hfst:def:hfst-transducer.hfst.get-flag-is-epsilon-in-composition-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.get-flag-is-epsilon-in-composition-fn]
    pub flag_is_epsilon_in_composition: bool,
    /// Exact configured allowance for budget-aware compose working memory.
    /// The OpenFst tropical and Foma backends partition it among their scalable
    /// compose structures; it is not an exact RSS ceiling. `None` is unbounded.
    pub compose_memory_limit_bytes: Option<u64>,
    // [spec:hfst:def:hfst-transducer.hfst.set-encode-weights-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.set-encode-weights-fn]
    // [spec:hfst:def:hfst-transducer.hfst.get-encode-weights-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.get-encode-weights-fn]
    pub encode_weights: bool,
    // [spec:hfst:def:hfst-transducer.hfst.set-minimization-algorithm-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.set-minimization-algorithm-fn]
    // [spec:hfst:def:hfst-transducer.hfst.get-minimization-algorithm-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.get-minimization-algorithm-fn]
    // [spec:hfst:def:hfst-transducer.hfst.minimization-algorithm-get-minimization-algorithm-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.minimization-algorithm-get-minimization-algorithm-fn]
    pub minimization_algorithm: MinimizationAlgorithm,
    // [spec:hfst:def:hfst-transducer.hfst.set-harmonize-smaller-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.set-harmonize-smaller-fn]
    // [spec:hfst:def:hfst-transducer.hfst.get-harmonize-smaller-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.get-harmonize-smaller-fn]
    pub harmonize_smaller: bool,
    // [spec:hfst:def:hfst-transducer.hfst.set-xerox-composition-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.set-xerox-composition-fn]
    // [spec:hfst:def:hfst-transducer.hfst.get-xerox-composition-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.get-xerox-composition-fn]
    pub xerox_composition: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        EngineConfig {
            minimization: true,
            minimize_even_if_already_minimal: false,
            unknown_symbols_in_use: true,
            flag_is_epsilon_in_composition: false,
            compose_memory_limit_bytes: None,
            encode_weights: false,
            minimization_algorithm: MinimizationAlgorithm::HOPCROFT,
            harmonize_smaller: true,
            xerox_composition: false,
        }
    }
}

// C++ 'enum MinimizationAlgorithm { HOPCROFT, BRZOZOWSKI }' (HfstTransducer.h:130).
// [spec:hfst:def:hfst-transducer.hfst.minimization-algorithm]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(non_camel_case_types)]
pub enum MinimizationAlgorithm {
    HOPCROFT,
    BRZOZOWSKI,
}

#[cfg(test)]
mod flag_compose_overlay_tests;

#[cfg(test)]
mod flag_encode_tests;
