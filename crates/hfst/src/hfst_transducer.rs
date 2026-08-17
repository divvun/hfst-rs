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

use crate::backend::{AlgebraBackend, Backend};
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
use crate::transducer::{Transducer, UnweightedTables, WeightedTables};
use crate::tropical_weight_transducer::TropicalWeightTransducer;

#[path = "hfst_transducer_flag_ops.rs"]
mod flag_ops;
pub(crate) use flag_ops::{decode_flag, encode_flag};
use flag_ops::{decode_flag_diacritics, encode_flag_diacritics, has_flags, rename_flag_diacritics};

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

/// The flag-diacritic self-loops that `-F` composition must expose virtually.
///
/// Preparing an overlay inserts these symbols into the corresponding operand's
/// alphabet, but deliberately does not insert transitions.  The OpenFst layer
/// maps the symbols to labels and presents a unit-weight self-loop only when
/// its composition matcher asks for one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FlagDiacriticComposeOverlay {
    /// Flags logically inserted as self-loops at every state of the left FST.
    pub left_self_loops: StringSet,
    /// Flags logically inserted as self-loops at every state of the right FST.
    pub right_self_loops: StringSet,
    /// Whether `_1` flags must precede `_2` flags between regular left-output
    /// symbols, matching HFST's two-state illegal-flag-path restriction.
    pub enforce_left_before_right: bool,
}

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

impl<B: Backend> HfstTransducer<B> {
    // -------------------------------------------------------------------------
    // ----- Constructors -----
    // -------------------------------------------------------------------------

    /// Wrap an already-built backend in fresh facade metadata. Crate-visible
    /// for the stream readers and the OL conversion smugglers, which build the
    /// backend first.
    pub(crate) fn wrap(fst: B) -> Self {
        HfstTransducer {
            name: String::new(),
            props: BTreeMap::new(),
            anonymous: false,
            is_trie: true,
            fst,
        }
    }

    /// \brief Create an empty transducer.
    ///
    /// Covers both C++ constructors 'HfstTransducer()' (the UNSPECIFIED
    /// placeholder — a facade without a backend is no longer representable)
    /// and 'HfstTransducer(ImplementationType type)' (empty transducer of a
    /// type; the type is the parameter 'B' now).
    pub fn new() -> Self {
        Self::wrap(B::empty())
    }

    /// Convert the backend to a weighted optimized-lookup transducer — the
    /// pmatch archive writer's per-member conversion ([`Backend::to_hfst_ol`];
    /// facade metadata is not carried over, the caller re-applies name and
    /// properties on the wrapped result).
    pub fn to_hfst_ol(
        &self,
        weighted: bool,
        options: &str,
        harmonizer: Option<&crate::transducer::Transducer>,
    ) -> crate::error::Result<crate::transducer::Transducer> {
        self.fst.to_hfst_ol(weighted, options, harmonizer)
    }

    /// \brief Create a deep copy of transducer 'another'.
    ///
    /// 'HfstTransducer(const HfstTransducer &another)'.
    pub fn new_copy(another: &HfstTransducer<B>) -> crate::error::Result<Self> {
        let mut props = BTreeMap::new();
        for (k, v) in &another.props {
            if k.as_str() != "type" {
                props.insert(k.clone(), v.clone());
            }
        }
        // NOTE: like C++, 'name' stays "" even though 'props' may carry a copied
        // "name" entry.
        Ok(HfstTransducer {
            name: String::new(),
            props,
            anonymous: another.anonymous,
            is_trie: another.is_trie,
            fst: another.fst.copy()?,
        })
    }

    /// \brief Create an HFST transducer equivalent to HFST basic transducer
    /// 'net'.
    ///
    /// 'HfstTransducer(const hfst::implementations::HfstBasicTransducer &net, type)'.
    pub fn new_from_basic(net: &HfstBasicTransducer) -> crate::error::Result<Self> {
        Ok(HfstTransducer {
            name: net.name.clone(), // C++: name = net.name; (after the switch)
            props: BTreeMap::new(),
            anonymous: false,
            is_trie: false,
            fst: B::from_basic(net)?,
        })
    }

    /// [`Self::new_from_basic`] for a caller that is finished with `net`, so a
    /// backend can release the graph as it converts it.
    pub fn new_from_basic_owned(net: HfstBasicTransducer) -> crate::error::Result<Self> {
        Ok(HfstTransducer {
            name: net.name.clone(),
            props: BTreeMap::new(),
            anonymous: false,
            is_trie: false,
            fst: B::from_basic_owned(net)?,
        })
    }

    // -------------------------------------------------------------------------
    // ----- Assignment -----
    // -------------------------------------------------------------------------

    /// 'HfstTransducer &assign(const HfstTransducer &another)' -> 'operator='.
    pub fn assign(
        &mut self,
        another: &HfstTransducer<B>,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.operator_assign(another)
    }

    /// \brief Assign this transducer a new value equivalent to 'another'.
    ///
    /// 'HfstTransducer &operator=(const HfstTransducer &another)'. The C++
    /// type-mismatch check is compile-time now (both sides are the same 'B').
    pub fn operator_assign(
        &mut self,
        another: &HfstTransducer<B>,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        // Check for self-assignment.
        if std::ptr::eq(
            another as *const HfstTransducer<B>,
            self as *const HfstTransducer<B>,
        ) {
            return Ok(self);
        }

        // set some features
        self.anonymous = another.anonymous;
        self.is_trie = another.is_trie;
        let nm = another.get_name();
        self.set_name(&nm);

        // Set new transducer (the old backend is freed by the assignment).
        self.fst = another.fst.copy()?;
        Ok(self)
    }

    // -------------------------------------------------------------------------
    // ----- Accessors -----
    // -------------------------------------------------------------------------

    /// \brief The implementation type of the transducer ('type' in C++) —
    /// now a constant of the backend type.
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-type-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-type-fn]
    pub fn get_type(&self) -> ImplementationType {
        B::TYPE
    }

    /// \brief Rename the transducer.
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.set-name-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.set-name-fn]
    pub fn set_name(&mut self, name: &str) {
        self.set_property("name", name);
    }

    /// \brief Get the name of the transducer.
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-name-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-name-fn]
    pub fn get_name(&self) -> String {
        self.get_property("name")
    }

    /// \brief Set arbitrary string property 'property' to 'name'.
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.set-property-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.set-property-fn]
    pub fn set_property(&mut self, property: &str, name: &str) {
        HfstTokenizer::check_utf8_correctness(name);
        self.props.insert(property.to_string(), name.to_string());
        if property == "name" {
            self.name = name.to_string();
        }
    }

    /// \brief Get arbitrary string property 'property'.
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-property-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-property-fn]
    pub fn get_property(&self, property: &str) -> String {
        match self.props.get(property) {
            Some(v) => v.clone(),
            None => String::new(),
        }
    }

    /// \brief Get all properties from the transducer.
    pub fn get_properties(&self) -> &BTreeMap<String, String> {
        &self.props
    }

    // -------------------------------------------------------------------------
    // ----- Conversion functions (typed; the runtime 'convert(ty)' is gone) -----
    // -------------------------------------------------------------------------

    /// For internal use: create an 'HfstBasicTransducer' equivalent to '*this'.
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-basic-transducer-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-basic-transducer-fn]
    pub fn get_basic_transducer(&self) -> crate::error::Result<HfstBasicTransducer> {
        self.fst.to_basic()
    }

    /// The typed conversion to the interchange transducer
    /// ([dec:hfst:monomorphic-backends]); cross-backend conversion is
    /// 'HfstTransducer::<Target>::from_basic(&t.to_basic()?)'.
    pub fn to_basic(&self) -> crate::error::Result<HfstBasicTransducer> {
        self.fst.to_basic()
    }

    /// Return a copy with every transition labelled `symbol` (on either the
    /// input or output side) removed, surviving states renumbered. Converts to a
    /// basic transducer, applies [`HfstBasicTransducer::kill_paths`], and
    /// converts back to this transducer's type. Lifted from hfst-kill-paths.
    pub fn kill_paths(&self, symbol: &str) -> HfstTransducer<B> {
        let killed = self
            .get_basic_transducer()
            .expect("get_basic_transducer on a valid transducer cannot fail")
            .kill_paths(symbol);
        HfstTransducer::from_basic_transducer(&killed)
    }

    /// For internal use: create an 'HfstBasicTransducer' equivalent to '*this'
    /// and delete the backend implementation.
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.convert-to-basic-transducer-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.convert-to-basic-transducer-fn]
    pub fn convert_to_basic_transducer(&mut self) -> crate::error::Result<HfstBasicTransducer> {
        let net = self.fst.to_basic()?;
        // The C++ 'delete's the backend here and leaves a null pointer until
        // 'convert_to_hfst_transducer' restores it; an empty backend stands in
        // for the null (a facade without a backend is not representable).
        self.fst = B::empty();
        Ok(net)
    }

    /// For internal use: build a backend equivalent to 't', delete 't', and
    /// store it as this transducer's implementation.
    pub fn convert_to_hfst_transducer(
        &mut self,
        t: HfstBasicTransducer,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.name = t.name.clone();
        self.fst = B::from_basic_owned(t)?;
        Ok(self)
    }
    // -------------------------------------------------------------------------
    // ----- Alphabet and harmonization (backend-agnostic surface) -----
    // -------------------------------------------------------------------------

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.insert-to-alphabet-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.insert-to-alphabet-fn]
    pub fn insert_to_alphabet_string(&mut self, symbol: &str) -> crate::error::Result<()> {
        HfstTokenizer::check_utf8_correctness(symbol);

        if symbol.is_empty() {
            crate::bail!(EmptyString, "insert_to_alphabet");
        }

        // The C++ per-type dispatch (OL inserts directly; everything else
        // round-trips through the basic transducer) is 'Backend::insert_to_alphabet'.
        self.fst.insert_to_alphabet(symbol)
    }

    pub fn insert_to_alphabet_string_set(
        &mut self,
        symbols: &StringSet,
    ) -> crate::error::Result<()> {
        for symbol in symbols.iter() {
            HfstTokenizer::check_utf8_correctness(symbol);
            if symbol.is_empty() {
                crate::bail!(EmptyString, "insert_to_alphabet");
            }
        }

        self.fst.add_symbols_to_alphabet(symbols)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.remove-from-alphabet-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.remove-from-alphabet-fn]
    pub fn remove_from_alphabet_string(&mut self, symbol: &str) -> crate::error::Result<()> {
        HfstTokenizer::check_utf8_correctness(symbol);

        if symbol.is_empty() {
            crate::bail!(EmptyString, "remove_from_alphabet");
        }

        self.fst.remove_from_alphabet(symbol)
    }

    pub fn remove_from_alphabet_string_set(
        &mut self,
        symbols: &StringSet,
    ) -> crate::error::Result<()> {
        for symbol in symbols.iter() {
            self.remove_from_alphabet_string(symbol)?;
        }
        Ok(())
    }

    pub fn prune_alphabet(&mut self, force: bool) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.fst.prune_alphabet(force)?;
        Ok(self)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-alphabet-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-alphabet-fn]
    pub fn get_alphabet(&self) -> crate::error::Result<StringSet> {
        Ok(self.fst.get_alphabet())
    }

    /*
      Only harmonize number-to-symbol-encodings.
      \a another is not modifed, but a modifed copy of it is returned.
    */
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.harmonize-symbol-encodings-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.harmonize-symbol-encodings-fn]
    pub fn harmonize_symbol_encodings(&mut self, another: &HfstTransducer<B>) -> HfstTransducer<B> {
        let another_basic = HfstBasicTransducer::from_hfst_transducer(another);
        let this_basic = HfstBasicTransducer::from_hfst_transducer(&*self);
        *self = HfstTransducer::from_basic_transducer(&this_basic);
        HfstTransducer::from_basic_transducer(&another_basic)
    }

    // test function
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.print-alphabet-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.print-alphabet-fn]
    pub fn print_alphabet(&self) {
        self.fst.print_alphabet();
    }

    // -------------------------------------------------------------------------
    // ----- Missing symbols / diacritics -----
    // -------------------------------------------------------------------------

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.insert-missing-diacritics-to-alphabet-from-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.insert-missing-diacritics-to-alphabet-from-fn]
    pub fn insert_missing_diacritics_to_alphabet_from(
        &mut self,
        another: &HfstTransducer<B>,
    ) -> crate::error::Result<StringSet> {
        let this_alphabet: StringSet = self.get_alphabet()?;
        let another_alphabet: StringSet = another.get_alphabet()?;
        let mut missing_flags: StringSet = StringSet::new();

        for it in another_alphabet.iter() {
            if !this_alphabet.contains(it) && FdOperation::is_diacritic(it) {
                missing_flags.insert(it.clone());
            }
        }
        self.insert_to_alphabet_set(&missing_flags)?;
        Ok(missing_flags)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.insert-missing-symbols-to-alphabet-from-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.insert-missing-symbols-to-alphabet-from-fn]
    pub fn insert_missing_symbols_to_alphabet_from(
        &mut self,
        another: &HfstTransducer<B>,
        only_special_symbols: bool,
    ) -> crate::error::Result<()> {
        let this_alphabet: StringSet = self.get_alphabet()?;
        let another_alphabet: StringSet = another.get_alphabet()?;
        let mut missing_symbols: StringSet = StringSet::new();

        for it in another_alphabet.iter() {
            if !this_alphabet.contains(it) {
                if !only_special_symbols {
                    missing_symbols.insert(it.clone());
                } else {
                    if is_special_symbol(it) {
                        missing_symbols.insert(it.clone());
                    }
                }
            }
        }
        self.insert_to_alphabet_set(&missing_symbols)?;
        Ok(())
    }

    /*
       Check for missing flag diacritics (FG), i.e. FGs that are present in the
       alphabet of \a another but not in the alphabet of this transducer and insert
       them to \a missing_flags. \a return_on_first_miss defines whether function
       returns after first missing FG is found and inserted to \a missing_flags.
       @ retval Whether any missing FGs where found.
    */
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.check-for-missing-flags-in-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.check-for-missing-flags-in-fn]
    pub fn check_for_missing_flags_in_into(
        &self,
        another: &HfstTransducer<B>,
        missing_flags: &mut StringSet,
        return_on_first_miss: bool,
    ) -> bool {
        let mut retval = false;
        let this_alphabet: StringSet = self
            .get_alphabet()
            .expect("get_alphabet on a valid transducer cannot fail");
        let another_alphabet: StringSet = another
            .get_alphabet()
            .expect("get_alphabet on a valid transducer cannot fail");

        for it in another_alphabet.iter() {
            if FdOperation::is_diacritic(it) && (!this_alphabet.contains(it)) {
                missing_flags.insert(it.clone());
                retval = true;
                if return_on_first_miss {
                    return retval;
                }
            }
        }
        retval
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.insert-freely-missing-flags-from-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.insert-freely-missing-flags-from-fn]
    pub fn insert_freely_missing_flags_from(&mut self, another: &HfstTransducer<B>) {
        let mut missing_flags: StringSet = StringSet::new();
        if self.check_for_missing_flags_in_into(
            another,
            &mut missing_flags,
            false, /* do not return on first miss */
        ) {
            let mut basic: HfstBasicTransducer = HfstBasicTransducer::from_transducer(self);

            // Every state gains a free self-loop per missing flag, so the graph
            // grows by 'states x flags' transitions — on a Giella speller that is
            // hundreds of millions. Intern each flag's symbol number and alphabet
            // entry once instead of once per (state, flag), and give each state
            // room for exactly its new loops, so the transition vectors never
            // double past the size they end at.
            let loops: Vec<(u32, u32)> = missing_flags
                .iter()
                .map(|flag| {
                    let tr = HfstBasicTransition::new_symbols(
                        0,
                        flag.clone(),
                        flag.clone(),
                        0.0,
                        basic.coder_mut(),
                    );
                    basic.add_symbol_to_alphabet(flag);
                    (tr.get_input_number(), tr.get_output_number())
                })
                .collect();

            for s in 0..=basic.get_max_state() {
                let transitions = &mut basic.state_vector[s as usize];
                transitions.reserve_exact(loops.len());
                for (input, output) in &loops {
                    transitions.push(HfstBasicTransition::new_numbers(
                        s, *input, *output, 0.0, false,
                    ));
                }
            }

            *self = HfstTransducer::from_basic_owned(basic);
        }
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.has-flag-diacritics-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.has-flag-diacritics-fn]
    pub fn has_flag_diacritics(&self) -> bool {
        has_flags(self)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.twosided-flag-diacritics-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.twosided-flag-diacritics-fn]
    pub fn twosided_flag_diacritics(&mut self) -> crate::error::Result<()> {
        let basic_fst: HfstBasicTransducer = HfstBasicTransducer::from_transducer(self);
        let mut basic_fst_copy: HfstBasicTransducer = HfstBasicTransducer::new();
        let _ = basic_fst_copy.add_state(basic_fst.get_max_state());

        for (s, states) in basic_fst.state_vector.iter().enumerate() {
            let s = s as HfstState;
            for transition in states.iter() {
                let istr = transition.get_input_symbol(basic_fst.coder());
                let ostr = transition.get_output_symbol(basic_fst.coder());
                let istr_is_flag = FdOperation::is_diacritic(&istr);
                let ostr_is_flag = FdOperation::is_diacritic(&ostr);

                let extra_transition_needed = (istr_is_flag || ostr_is_flag) && (istr != ostr);

                if extra_transition_needed {
                    let new_state: HfstState = basic_fst_copy.add_state_new();

                    // flag:foo -> flag:flag 0:foo, foo:flag -> foo:0 flag:flag
                    // flag1:flag2 -> flag1:flag1 flag2:flag2

                    let mut input = istr.clone();
                    let mut out = if istr_is_flag {
                        istr.clone()
                    } else {
                        Symbol::new_static(crate::hfst_symbol_defs::internal_epsilon)
                    };

                    let tr = HfstBasicTransition::new_symbols(
                        new_state,
                        input,
                        out,
                        0.0, /*?*/
                        basic_fst_copy.coder_mut(),
                    );
                    basic_fst_copy.add_transition(s, &tr, true);

                    input = if ostr_is_flag {
                        ostr.clone()
                    } else {
                        Symbol::new_static(crate::hfst_symbol_defs::internal_epsilon)
                    };
                    out = ostr.clone();

                    let tr = HfstBasicTransition::new_symbols(
                        transition.get_target_state(),
                        input,
                        out,
                        transition.get_weight(), /*?*/
                        basic_fst_copy.coder_mut(),
                    );
                    basic_fst_copy.add_transition(new_state, &tr, true);
                } else {
                    let tr = HfstBasicTransition::new_symbols(
                        transition.get_target_state(),
                        istr.clone(),
                        ostr.clone(),
                        transition.get_weight(),
                        basic_fst_copy.coder_mut(),
                    );
                    basic_fst_copy.add_transition(s, &tr, true);
                }
            }

            if basic_fst.is_final_state(s) {
                basic_fst_copy.set_final_weight(
                    s,
                    &basic_fst
                        .get_final_weight(s)
                        .expect("state was confirmed final via is_final_state"),
                );
            }
        }
        *self = HfstTransducer::new_from_basic(&basic_fst_copy)?;
        Ok(())
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.check-for-missing-flags-in-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.check-for-missing-flags-in-fn]
    pub fn check_for_missing_flags_in(&self, another: &HfstTransducer<B>) -> bool {
        let mut unused_missing_flags: StringSet = StringSet::new(); /* An obligatory argument that is not used. */
        self.check_for_missing_flags_in_into(
            another,
            &mut unused_missing_flags,
            true, /* return on first miss */
        )
    }

    // -------------------------------------------------------------------------
    // ----- Queries -----
    // -------------------------------------------------------------------------

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-cyclic-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-cyclic-fn]
    pub fn is_cyclic(&self) -> crate::error::Result<bool> {
        Ok(self.fst.is_cyclic())
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.number-of-states-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.number-of-states-fn]
    pub fn number_of_states(&self) -> u32 {
        self.fst.number_of_states()
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.number-of-arcs-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.number-of-arcs-fn]
    pub fn number_of_arcs(&self) -> u32 {
        self.fst.number_of_arcs()
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.has-weights-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.has-weights-fn]
    pub fn has_weights(&self) -> bool {
        self.fst.has_weights()
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-infinitely-ambiguous-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-infinitely-ambiguous-fn]
    pub fn is_infinitely_ambiguous(&self) -> crate::error::Result<bool> {
        self.fst.is_infinitely_ambiguous()
    }

    // -------------------------------------------------------------------------
    // ----- Path extraction -----
    // -------------------------------------------------------------------------

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-paths-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-paths-fn]
    pub fn extract_paths_cb(
        &self,
        callback: &mut dyn ExtractStringsCb,
        cycles: i32,
    ) -> crate::error::Result<()> {
        self.fst.extract_paths_cb(callback, cycles);
        Ok(())
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-paths-fd-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-paths-fd-fn]
    pub fn extract_paths_fd_cb(
        &self,
        callback: &mut dyn ExtractStringsCb,
        cycles: i32,
        filter_fd: bool,
    ) -> crate::error::Result<()> {
        self.fst.extract_paths_fd_cb(callback, cycles, filter_fd);
        Ok(())
    }

    pub fn extract_paths(
        &self,
        results: &mut HfstTwoLevelPaths,
        max_num: i32,
        cycles: i32,
    ) -> crate::error::Result<()> {
        if self.is_cyclic()? && max_num < 1 && cycles < 0 {
            crate::bail!(TransducerIsCyclic, "HfstTransducer::extract_paths");
        }

        let mut cb = ExtractStringsCb_::new(results, max_num);
        self.extract_paths_cb(&mut cb, cycles)?;
        Ok(())
    }

    pub fn extract_paths_fd(
        &self,
        results: &mut HfstTwoLevelPaths,
        max_num: i32,
        cycles: i32,
        filter_fd: bool,
    ) -> crate::error::Result<()> {
        if self.is_cyclic()? && max_num < 1 && cycles < 0 {
            crate::bail!(TransducerIsCyclic, "HfstTransducer::extract_paths_fd");
        }

        let mut cb = ExtractStringsCb_::new(results, max_num);
        self.extract_paths_fd_cb(&mut cb, cycles, filter_fd)?;
        Ok(())
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-shortest-paths-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-shortest-paths-fn]
    pub fn extract_shortest_paths(
        &self,
        results: &mut HfstTwoLevelPaths,
    ) -> crate::error::Result<()> {
        // The C++ converted a copy to TROPICAL_OPENFST_TYPE before n_best; the
        // conversion is typed now ([dec:hfst:monomorphic-backends]).
        let mut t: HfstTransducer<StdVectorFst> = HfstTransducer::wrap(
            <StdVectorFst as Backend>::from_basic(&self.fst.to_basic()?)?,
        );
        t.n_best(1)?;
        t.extract_paths(results, -1, -1)?;
        Ok(())
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-random-paths-fd-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-random-paths-fd-fn]
    pub fn extract_random_paths_fd(
        &self,
        results: &mut HfstTwoLevelPaths,
        max_num: i32,
        filter_fd: bool,
    ) -> crate::error::Result<()> {
        // The C++ converted a copy to TROPICAL_OPENFST_TYPE (the only backend
        // with a fd-filtered random extraction); the conversion is typed now.
        let copy: StdVectorFst = <StdVectorFst as Backend>::from_basic(&self.fst.to_basic()?)?;
        TropicalWeightTransducer::extract_random_paths_fd(&copy, results, max_num, filter_fd);
        Ok(())
    }
    // -------------------------------------------------------------------------
    // ----- AT&T / prolog I/O, tokenizer creation (HfstTransducer.cc ~5823-6410)
    // -------------------------------------------------------------------------
    // 'HfstBasicTransducer net(*this)' is the conversion constructor
    // 'HfstBasicTransducer(const HfstTransducer&)' — ported as the assoc-fn
    // 'HfstBasicTransducer::new_from_hfst_transducer(&self)'.

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.write-in-att-format-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.write-in-att-format-fn]
    pub fn write_in_att_format_filename(
        &self,
        filename: &str,
        print_weights: bool,
    ) -> crate::error::Result<()> {
        let file = match std::fs::File::create(filename) {
            Ok(f) => f,
            Err(_) => {
                let message = filename.to_string();
                crate::bail!(StreamCannotBeWritten, message);
            }
        };
        let mut ofile = std::io::BufWriter::new(file);
        self.write_in_att_format_file(&mut ofile, print_weights)
            .and_then(|()| std::io::Write::flush(&mut ofile))
            .map_err(|_| crate::err!(StreamCannotBeWritten, filename))?;
        Ok(())
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.write-in-att-format-number-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.write-in-att-format-number-fn]
    pub fn write_in_att_format_number(
        &self,
        ofile: &mut dyn std::io::Write,
        print_weights: bool,
    ) -> std::io::Result<()> {
        let net = HfstBasicTransducer::new_from_hfst_transducer(self);
        net.write_in_att_format_number_file(ofile, print_weights)
    }

    pub fn write_in_att_format_file(
        &self,
        ofile: &mut dyn std::io::Write,
        print_weights: bool,
    ) -> std::io::Result<()> {
        // Implemented only for internal transducer format.
        let net = HfstBasicTransducer::new_from_hfst_transducer(self);
        net.write_in_att_format_file(ofile, print_weights)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.write-in-prolog-format-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.write-in-prolog-format-fn]
    pub fn write_in_prolog_format(
        &mut self,
        file: &mut dyn std::io::Write,
        name: &str,
        write_weights: bool,
    ) -> crate::error::Result<()> {
        let fsm = HfstBasicTransducer::new_from_hfst_transducer(self);
        fsm.write_in_prolog_format_file(file, name, write_weights)
    }

    /// 'HfstTransducer &read_in_att_format(const std::string &filename, type,
    ///  const std::string &epsilon_symbol, bool warn_negs)'. The target type is
    ///  the type parameter now.
    pub fn read_in_att_format_filename(
        filename: &str,
        epsilon_symbol: &str,
        warn_negs: bool,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let ifile = match std::fs::File::open(filename) {
            Ok(f) => f,
            Err(_) => {
                // [spec:hfst:def:hfst-transducer.hfst.message-fn]
                // [spec:hfst:sem:hfst-transducer.hfst.message-fn]
                crate::bail!(StreamNotReadable, filename);
            }
        };
        HfstTokenizer::check_utf8_correctness(epsilon_symbol);

        let mut reader = std::io::BufReader::new(ifile);
        Self::read_in_att_format_file(&mut reader, epsilon_symbol, warn_negs)
    }

    /// 'HfstTransducer &read_in_att_format(FILE *ifile, type,
    ///  const std::string &epsilon_symbol, bool warn_negs)'.
    pub fn read_in_att_format_file(
        ifile: &mut dyn std::io::BufRead,
        epsilon_symbol: &str,
        warn_negs: bool,
    ) -> crate::error::Result<HfstTransducer<B>> {
        HfstTokenizer::check_utf8_correctness(epsilon_symbol);

        let mut linecount: u32 = 0;
        let net = HfstBasicTransducer::read_in_att_format_file(
            ifile,
            epsilon_symbol,
            &mut linecount,
            warn_negs,
        )?;
        // C++ 'new HfstTransducer(net, type)' returned a heap pointer the caller
        // owned; the owned value is the idiomatic equivalent.
        let _ = linecount;
        HfstTransducer::new_from_basic(&net)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.universal-pair-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.universal-pair-fn]
    pub fn universal_pair() -> HfstTransducer<B> {
        let mut bt = HfstBasicTransducer::new();
        let tr = HfstBasicTransition::new_symbols(
            1,
            Symbol::new_static("@_IDENTITY_SYMBOL_@"),
            Symbol::new_static("@_IDENTITY_SYMBOL_@"),
            0.0,
            bt.coder_mut(),
        );
        bt.add_transition(0, &tr, true);
        let tr = HfstBasicTransition::new_symbols(
            1,
            Symbol::new_static("@_UNKNOWN_SYMBOL_@"),
            Symbol::new_static("@_UNKNOWN_SYMBOL_@"),
            0.0,
            bt.coder_mut(),
        );
        bt.add_transition(0, &tr, true);
        let tr = HfstBasicTransition::new_symbols(
            1,
            Symbol::new_static("@_UNKNOWN_SYMBOL_@"),
            Symbol::new_static("@_EPSILON_SYMBOL_@"),
            0.0,
            bt.coder_mut(),
        );
        bt.add_transition(0, &tr, true);
        let tr = HfstBasicTransition::new_symbols(
            1,
            Symbol::new_static("@_EPSILON_SYMBOL_@"),
            Symbol::new_static("@_UNKNOWN_SYMBOL_@"),
            0.0,
            bt.coder_mut(),
        );
        bt.add_transition(0, &tr, true);
        bt.set_final_weight(1, &0.0);

        HfstTransducer::new_from_basic_transducer(&bt)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.identity-pair-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.identity-pair-fn]
    pub fn identity_pair() -> HfstTransducer<B> {
        let mut bt = HfstBasicTransducer::new();
        let tr = HfstBasicTransition::new_symbols(
            1,
            Symbol::new_static("@_IDENTITY_SYMBOL_@"),
            Symbol::new_static("@_IDENTITY_SYMBOL_@"),
            0.0,
            bt.coder_mut(),
        );
        bt.add_transition(0, &tr, true);
        bt.set_final_weight(1, &0.0);

        HfstTransducer::new_from_basic_transducer(&bt)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.create-tokenizer-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.create-tokenizer-fn]
    pub fn create_tokenizer(&mut self) -> HfstTokenizer {
        let mut tok = HfstTokenizer::new();

        // (the SFST 'get_symbol_pairs' branch is compiled out with the backend)
        let mut t = HfstBasicTransducer::new_from_hfst_transducer(self);
        t.prune_alphabet(true);
        let alpha = t.get_alphabet();
        for it in alpha.iter() {
            if it.len() > 1 {
                tok.add_multichar_symbol(it);
            }
        }

        tok
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.read-lexc-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.read-lexc-fn]
    // The C++ 'type' parameter is the backend type parameter 'B' now
    // ([dec:hfst:monomorphic-backends]); its availability check was pure
    // capability gating and is a static fact of the instantiation.
    pub fn read_lexc(filename: &str, verbose: bool) -> crate::error::Result<HfstTransducer<B>>
    where
        B: AlgebraBackend,
    {
        Ok(HfstTransducer::read_lexc_ptr(filename, verbose)?
            .expect("read_lexc: lexc compilation produced no transducer"))
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.read-lexc-ptr-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.read-lexc-ptr-fn]
    pub fn read_lexc_ptr(
        filename: &str,
        verbose: bool,
    ) -> crate::error::Result<Option<HfstTransducer<B>>>
    where
        B: AlgebraBackend,
    {
        // The C++ 'compiler.parse(filename.c_str())' reads the file via the
        // Flex/Bison lexer; the ported LexcCompiler walks an AST built from
        // source text instead, so read the file here and feed 'compile'.
        // (The C++ 'new HfstTransducer()' placeholder that it then leaks was a
        // raw-pointer artifact and is gone with the owned return.)
        let mut compiler = crate::lexc::LexcCompiler::<B>::new();
        compiler.set_verbosity(verbose as u32);
        let source = std::fs::read_to_string(filename)
            .map_err(|_| crate::err!(StreamNotReadable, filename))?;
        Ok(compiler.compile(&source))
    }

    // ----- integration shims (copy-constructor aliases) -----

    pub fn new_from(another: &HfstTransducer<B>) -> Self {
        HfstTransducer::new_copy(another).expect("copying an existing transducer cannot fail")
    }
    pub fn new_from_transducer(another: &HfstTransducer<B>) -> Self {
        HfstTransducer::new_copy(another).expect("copying an existing transducer cannot fail")
    }
    pub fn from_basic(net: &HfstBasicTransducer) -> Self {
        HfstTransducer::new_from_basic(net)
            .expect("converting a basic transducer to an available backend type cannot fail")
    }
    pub fn from_basic_owned(net: HfstBasicTransducer) -> Self {
        HfstTransducer::new_from_basic_owned(net)
            .expect("converting a basic transducer to an available backend type cannot fail")
    }
    pub fn from_basic_transducer(net: &HfstBasicTransducer) -> Self {
        HfstTransducer::new_from_basic(net)
            .expect("converting a basic transducer to an available backend type cannot fail")
    }
    pub fn new_from_basic_transducer(net: &HfstBasicTransducer) -> Self {
        HfstTransducer::new_from_basic(net)
            .expect("converting a basic transducer to an available backend type cannot fail")
    }

    // ----- integration shims (alphabet / substitute overload-name aliases) -----

    pub fn insert_to_alphabet_symbol<S: AsRef<str>>(
        &mut self,
        symbol: S,
    ) -> crate::error::Result<()> {
        self.insert_to_alphabet_string(symbol.as_ref())
    }
    pub fn insert_to_alphabet<S: AsRef<str>>(&mut self, symbol: S) -> crate::error::Result<()> {
        self.insert_to_alphabet_string(symbol.as_ref())
    }
    pub fn insert_to_alphabet_set(&mut self, symbols: &StringSet) -> crate::error::Result<()> {
        self.insert_to_alphabet_string_set(symbols)
    }
    pub fn remove_from_alphabet_symbol<S: AsRef<str>>(
        &mut self,
        symbol: S,
    ) -> crate::error::Result<()> {
        self.remove_from_alphabet_string(symbol.as_ref())
    }
    pub fn remove_from_alphabet<S: AsRef<str>>(&mut self, symbol: S) -> crate::error::Result<()> {
        self.remove_from_alphabet_string(symbol.as_ref())
    }
    pub fn remove_from_alphabet_set(&mut self, symbols: &StringSet) -> crate::error::Result<()> {
        self.remove_from_alphabet_string_set(symbols)
    }
}

impl<B: Backend> Default for HfstTransducer<B> {
    fn default() -> Self {
        Self::new()
    }
}

// ===== integration shims: Clone (C++ copy ctor) =====
impl<B: Backend> Clone for HfstTransducer<B> {
    fn clone(&self) -> Self {
        HfstTransducer::new_copy(self).expect("cloning a valid transducer cannot fail")
    }
}

// -----------------------------------------------------------------------------
// The mutable FST algebra (tropical instantiation only).
// -----------------------------------------------------------------------------

impl<B: AlgebraBackend> HfstTransducer<B> {
    /// The typed algebra->OL conversion of [dec:hfst:monomorphic-backends]
    /// (the C++ 'convert(HFST_OLW_TYPE)' / 'convert(HFST_OL_TYPE)' pair).
    /// Both build weighted-shaped tables in memory, exactly as the C++ did
    /// even for HFST_OL_TYPE output; 'weighted' only sets the header flag,
    /// i.e. the stream type the result serializes under. 'options' is the
    /// C++ convert's options string ("quick" skips the hard table packing).
    /// The facade metadata survives, as it did through the C++ convert.
    pub fn to_ol(
        &self,
        weighted: bool,
        options: &str,
    ) -> crate::error::Result<HfstTransducer<Transducer<WeightedTables>>> {
        let net = self.get_basic_transducer()?;
        let ol = crate::convert_transducer_format::ConversionFunctions::
            hfst_basic_transducer_to_hfst_ol(&net, weighted, options, None)?;
        let mut t = HfstTransducer::wrap(ol);
        t.name = self.name.clone();
        t.props = self.props.clone();
        t.anonymous = self.anonymous;
        t.is_trie = self.is_trie;
        Ok(t)
    }

    /// Convert to a native foma transducer. The runtime-type analogue of the
    /// FOMA_TYPE arm of C++ `HfstTransducer::convert`: go through the basic
    /// transducer (`hfst_basic_transducer_to_foma`) and wrap the result. Used by
    /// the CLI to write a compiled (algebra-backend) transducer to a foma stream.
    #[cfg(feature = "foma")]
    pub fn to_foma(
        &self,
    ) -> crate::error::Result<HfstTransducer<crate::backend_foma::FomaTransducer>> {
        let net = self.get_basic_transducer()?;
        let foma =
            <crate::backend_foma::FomaTransducer as crate::backend::Backend>::from_basic(&net)?;
        let mut t = HfstTransducer::wrap(foma);
        t.name = self.name.clone();
        t.props = self.props.clone();
        t.anonymous = self.anonymous;
        t.is_trie = self.is_trie;
        Ok(t)
    }

    // -------------------------------------------------------------------------
    // ----- Construction constructors (define_transducer_* arms) -----
    // -------------------------------------------------------------------------

    /// 'HfstTransducer(const std::string &utf8_str, const HfstTokenizer&, type)'.
    pub fn new_tokenized(
        utf8_str: &str,
        multichar_symbol_tokenizer: &HfstTokenizer,
    ) -> crate::error::Result<Self> {
        if utf8_str.is_empty() {
            crate::bail!(
                EmptyString,
                "HfstTransducer(const std::string&, const HfstTokenizer&, ImplementationType)"
            );
        }
        let spv = multichar_symbol_tokenizer.tokenize(utf8_str, false);
        Ok(Self::wrap(B::define_transducer_spv(&spv)))
    }

    /// 'HfstTransducer(const std::string &upper, const std::string &lower,
    ///  const HfstTokenizer&, type)'.
    pub fn new_tokenized_pair(
        upper_utf8_str: &str,
        lower_utf8_str: &str,
        multichar_symbol_tokenizer: &HfstTokenizer,
    ) -> crate::error::Result<Self> {
        if upper_utf8_str.is_empty() || lower_utf8_str.is_empty() {
            // NOTE: the C++ message is missing its closing paren; preserved.
            crate::bail!(
                EmptyString,
                "HfstTransducer(const std::string&, const std::string&, const HfstTokenizer&, ImplementationType"
            );
        }
        let spv = multichar_symbol_tokenizer.tokenize_pair(upper_utf8_str, lower_utf8_str, false);
        Ok(Self::wrap(B::define_transducer_spv(&spv)))
    }

    /// 'HfstTransducer(const StringPairSet &sps, type, bool cyclic=false)'.
    pub fn new_string_pair_set(sps: &StringPairSet, cyclic: bool) -> crate::error::Result<Self> {
        for sp in sps {
            if sp.0.is_empty() || sp.1.is_empty() {
                crate::bail!(
                    EmptyString,
                    "HfstTransducer(const StringPairSet&, ImplementationType, bool)"
                );
            }
        }
        let mut t = Self::wrap(B::define_transducer_sps(sps, cyclic));
        t.is_trie = false;
        Ok(t)
    }

    /// 'HfstTransducer(const StringPairVector &spv, type)'.
    pub fn new_string_pair_vector(spv: &StringPairVector) -> crate::error::Result<Self> {
        for it in spv {
            if it.0.is_empty() || it.1.is_empty() {
                crate::bail!(
                    EmptyString,
                    "HfstTransducer(const StringPairVector&, ImplementationType)"
                );
            }
        }
        let mut t = Self::wrap(B::define_transducer_spv(spv));
        t.is_trie = false;
        Ok(t)
    }

    /// 'HfstTransducer(const StringVector &sv, type)'.
    ///
    /// C++ builds 'spv' then does '*this = HfstTransducer(spv, type)' on an
    /// uninitialized placeholder; the placeholder is a real empty transducer
    /// now, and 'operator_assign' reproduces the observable result
    /// ('props["name"] == ""', the copied backend).
    pub fn new_string_vector(sv: &StringVector) -> crate::error::Result<Self> {
        let mut this = Self::new();
        this.is_trie = false;
        let mut spv = StringPairVector::new();
        for it in sv {
            spv.push((it.clone(), it.clone()));
        }
        // *this = HfstTransducer(spv, type);
        let tmp = Self::new_string_pair_vector(&spv)?;
        this.operator_assign(&tmp)?;
        Ok(this)
    }

    /// 'HfstTransducer(const std::vector<StringPairSet> &spsv, type)'.
    pub fn new_string_pair_set_vector(spsv: &[StringPairSet]) -> crate::error::Result<Self> {
        for it in spsv {
            for pair in it {
                if pair.0.is_empty() || pair.1.is_empty() {
                    crate::bail!(
                        EmptyString,
                        "HfstTransducer(const std::vector<StringPairSet>&, ImplementationType)"
                    );
                }
            }
        }
        let mut t = Self::wrap(B::define_transducer_spsv(spsv));
        t.is_trie = false;
        Ok(t)
    }

    /// \brief Create '[symbol:symbol]'.
    ///
    /// 'HfstTransducer(const std::string &symbol, type)'.
    pub fn new_symbol(symbol: &str) -> crate::error::Result<Self> {
        HfstTokenizer::check_utf8_correctness(symbol);
        if symbol.is_empty() {
            crate::bail!(
                EmptyString,
                "HfstTransducer(const std::string&, ImplementationType)"
            );
        }
        let mut t = Self::wrap(B::define_transducer_symbol(symbol));
        t.is_trie = false;
        Ok(t)
    }

    /// \brief Create '[isymbol:osymbol]'.
    ///
    /// 'HfstTransducer(const std::string &isymbol, const std::string &osymbol, type)'.
    pub fn new_symbol_pair(isymbol: &str, osymbol: &str) -> crate::error::Result<Self> {
        HfstTokenizer::check_utf8_correctness(isymbol);
        HfstTokenizer::check_utf8_correctness(osymbol);
        if isymbol.is_empty() || osymbol.is_empty() {
            crate::bail!(
                EmptyString,
                "HfstTransducer(const std::string&, const std::string&,  ImplementationType)"
            );
        }
        let mut t = Self::wrap(B::define_transducer_symbol_pair(isymbol, osymbol));
        t.is_trie = false;
        Ok(t)
    }
    // -------------------------------------------------------------------------
    // ----- Harmonization -----
    // -------------------------------------------------------------------------

    /*
       Harmonize this transducer with a copy of another.
       another is not modifed, but a modified copy of it is returned.
       Flag diacritics from the alphabet of this transducer are inserted
       to the alphabet of the copy of another, so that they are excluded
       from harmonization.
       (The C++ returned NULL for foma inputs, harmonizing them not at all.
       Every backend is harmonized here, through the interchange graph. The
       Option shape is kept — callers still handle the None case.)
    */
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.harmonize-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.harmonize-fn]
    pub fn harmonize_copy(
        &mut self,
        another: &HfstTransducer<B>,
    ) -> crate::error::Result<Option<HfstTransducer<B>>> {
        if self.anonymous && another.anonymous {
            crate::bail!(Fatal, "harmonize_copy with anonymous transducers");
        }

        // (The C++ pre-inserted flag diacritics for foma inputs only. The
        // harmonization below runs on the interchange graph, which treats
        // flags alike whatever backend the operands came from.)

        // The C++ copied 'another' before converting, because its conversion
        // consumed the source. 'get_basic_transducer' builds a fresh graph from
        // a shared reference, so the copy is a second full transducer nobody
        // reads — on a flag-harmonized operand that is gigabytes.
        let another_basic = another.get_basic_transducer()?;
        self.harmonize_onto(another_basic).map(Some)
    }

    /// [`Self::harmonize_copy`] for a caller that is finished with `another`,
    /// which is then released before the harmonized copy is built rather than
    /// standing beside it.
    pub fn harmonize_copy_owned(
        &mut self,
        another: HfstTransducer<B>,
    ) -> crate::error::Result<Option<HfstTransducer<B>>> {
        if self.anonymous && another.anonymous {
            crate::bail!(Fatal, "harmonize_copy with anonymous transducers");
        }

        let another_basic = another.get_basic_transducer()?;
        drop(another);
        self.harmonize_onto(another_basic).map(Some)
    }

    /// Harmonize this transducer against `another_basic` in place and return
    /// the matching harmonized copy of it.
    fn harmonize_onto(
        &mut self,
        mut another_basic: HfstBasicTransducer,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let mut this_basic = self.convert_to_basic_transducer()?;

        this_basic.harmonize(&mut another_basic);

        // The two graphs carry independent symbol codings; reindex both
        // onto one shared coder so that, after each is converted back to an
        // OpenFst transducer, identical symbols carry identical labels (the
        // per-graph-coder replacement for the former process-global
        // numbering on which the subsequent binary op relies). Intern every
        // symbol of BOTH graphs (coder + full alphabet) into the shared
        // coder FIRST, so even alphabet-only symbols agree before either
        // graph adopts the coding.
        let mut canonical = crate::hfst_tropical_transducer_transition_data::SymbolCoder::new();
        this_basic.intern_into(&mut canonical);
        another_basic.intern_into(&mut canonical);
        this_basic.reindex_into(&mut canonical);
        another_basic.reindex_into(&mut canonical);

        self.convert_to_hfst_transducer(this_basic)?;
        Ok(HfstTransducer::from_basic_owned(another_basic))
    }

    /*  Harmonize symbol-to-number encodings and expand unknown and
    identity symbols. */
    pub fn harmonize(
        &mut self,
        another: &mut HfstTransducer<B>,
        force: bool,
    ) -> crate::error::Result<()> {
        if self.anonymous && another.anonymous {
            return Ok(());
        }

        // Prevent flag diacritics from being harmonized by inserting them to
        // the alphabet.
        let this_alphabet = self.get_alphabet()?;
        let another_alphabet = another.get_alphabet()?;

        for it in another_alphabet.iter() {
            if FdOperation::is_diacritic(it) && !this_alphabet.contains(it) {
                self.insert_to_alphabet_string(it)?;
            }
        }

        for it in this_alphabet.iter() {
            if FdOperation::is_diacritic(it) && !another_alphabet.contains(it) {
                another.insert_to_alphabet_string(it)?;
            }
        }

        let _ = force;

        let mut this_basic = self.convert_to_basic_transducer()?;
        let mut another_basic = another.convert_to_basic_transducer()?;

        this_basic.harmonize(&mut another_basic);

        // Reindex both graphs onto one shared symbol coding so that, after
        // each is converted back to an OpenFst transducer, identical symbols
        // carry identical labels for the subsequent binary op (the
        // per-graph-coder replacement for the former process-global numbering).
        // Intern both graphs' symbols (coder + alphabet) into the shared
        // coder first so alphabet-only symbols agree too.
        let mut canonical = crate::hfst_tropical_transducer_transition_data::SymbolCoder::new();
        this_basic.intern_into(&mut canonical);
        another_basic.intern_into(&mut canonical);
        this_basic.reindex_into(&mut canonical);
        another_basic.reindex_into(&mut canonical);

        self.convert_to_hfst_transducer(this_basic)?;
        another.convert_to_hfst_transducer(another_basic)?;
        Ok(())
    }

    /// The harmonization preamble of the former 'apply(..., HfstTransducer&,
    /// bool harmonize)' binary functor (HfstApply.cc) — the only part of the
    /// 'apply*' family that survives monomorphization. Every binary op calls
    /// this before its backend trait call.
    // [spec:hfst:def:hfst-apply.another-fn]
    // [spec:hfst:sem:hfst-apply.another-fn]
    fn harmonize_for_binary_op(
        &mut self,
        another_tr: &HfstTransducer<B>,
        harmonize: bool,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let mut another = HfstTransducer::new_copy(another_tr)?;

        // prevent harmonization, if needed
        if !harmonize {
            self.insert_missing_symbols_to_alphabet_from(&another, false)?;
            another.insert_missing_symbols_to_alphabet_from(self, false)?;
        }

        // special symbols are never harmonized
        self.insert_missing_symbols_to_alphabet_from(&another, true)?;
        another.insert_missing_symbols_to_alphabet_from(self, true)?;
        // 'harmonize_copy' returns None for foma (use our own copy of 'another').
        let another: HfstTransducer<B> = match self.harmonize_copy(&another)? {
            Some(h) => h,
            None => HfstTransducer::new_copy(&another)?,
        };
        Ok(another)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.harmonize-flag-diacritics-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.harmonize-flag-diacritics-fn]
    pub fn harmonize_flag_diacritics(
        &mut self,
        another: &mut HfstTransducer<B>,
        insert_renamed_flags: bool,
    ) -> crate::error::Result<()> {
        let this_has_flag_diacritics = has_flags(self);
        let another_has_flag_diacritics = has_flags(another);

        if this_has_flag_diacritics && another_has_flag_diacritics {
            rename_flag_diacritics(self, "_1");
            rename_flag_diacritics(another, "_2");

            if insert_renamed_flags {
                self.insert_freely_missing_flags_from(another);
                another.insert_freely_missing_flags_from(self);
                self.remove_illegal_flag_paths()?;
            }
        } else if this_has_flag_diacritics && insert_renamed_flags {
            another.insert_freely_missing_flags_from(self);
        } else if another_has_flag_diacritics && insert_renamed_flags {
            self.insert_freely_missing_flags_from(another);
        }
        Ok(())
    }

    /// Prepare the operands for flag-diacritic-aware composition without
    /// materializing the `states * missing_flags` self-loops.
    ///
    /// This performs the same flag renaming as [`Self::harmonize_flag_diacritics`]
    /// when both operands originally contain flags.  It then harmonizes only
    /// the missing flag symbols into the opposite alphabets and returns those
    /// exact differences for the lazy OpenFst overlay.
    pub fn prepare_flag_diacritics_for_compose(
        &mut self,
        another: &mut HfstTransducer<B>,
    ) -> crate::error::Result<FlagDiacriticComposeOverlay> {
        let left_had_flags = has_flags(self);
        let right_had_flags = has_flags(another);

        if left_had_flags && right_had_flags {
            rename_flag_diacritics(self, "_1");
            rename_flag_diacritics(another, "_2");
        }

        // Do this in order: after right-side flags are inserted into the left
        // alphabet, the reverse difference still consists exactly of the
        // original left-side flags because the post-rename sets are disjoint.
        let left_self_loops = self.insert_missing_diacritics_to_alphabet_from(another)?;
        let right_self_loops = another.insert_missing_diacritics_to_alphabet_from(self)?;
        let enforce_left_before_right = left_had_flags
            && right_had_flags
            && !left_self_loops.is_empty()
            && !right_self_loops.is_empty();

        Ok(FlagDiacriticComposeOverlay {
            left_self_loops,
            right_self_loops,
            enforce_left_before_right,
        })
    }

    // -------------------------------------------------------------------------
    // ----- compare, queries (HfstTransducer.cc ~1681-2663) -----
    // -------------------------------------------------------------------------

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.compare-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.compare-fn]
    pub fn compare(
        &self,
        another: &HfstTransducer<B>,
        harmonize: bool,
    ) -> crate::error::Result<bool> {
        let mut one_copy = HfstTransducer::new_from(self);
        let mut another_copy = HfstTransducer::new_from(another);

        /* prevent harmonization, if needed */
        if !harmonize {
            one_copy.insert_missing_symbols_to_alphabet_from(&another_copy, false)?;
            another_copy.insert_missing_symbols_to_alphabet_from(&one_copy, false)?;
        }
        /* always prevent harmonizing special symbols */
        one_copy.insert_missing_symbols_to_alphabet_from(&another_copy, true)?;
        another_copy.insert_missing_symbols_to_alphabet_from(&one_copy, true)?;

        another_copy = one_copy
            .harmonize_copy(&another_copy)?
            .expect("harmonize_copy returns Some for tropical types");

        one_copy.determinize()?;
        another_copy.determinize()?;

        // No caller configures equivalence-checking, so the former global
        // 'encode_weights' is read at its C++ default (false) here.
        Ok(one_copy.fst.are_equivalent(&another_copy.fst, false))
    }

    pub fn compare_default(&self, another: &HfstTransducer<B>) -> crate::error::Result<bool> {
        self.compare(another, true)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-automaton-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-automaton-fn]
    pub fn is_automaton(&self) -> crate::error::Result<bool> {
        Ok(self.fst.is_automaton())
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-initial-input-symbols-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-initial-input-symbols-fn]
    pub fn get_initial_input_symbols(&self) -> StringSet {
        self.fst.get_initial_input_symbols()
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-first-input-symbols-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-first-input-symbols-fn]
    pub fn get_first_input_symbols(&self) -> crate::error::Result<StringSet> {
        Ok(self.fst.get_first_input_symbols())
    }

    // -------------------------------------------------------------------------
    // ----- Flag elimination -----
    // -------------------------------------------------------------------------

    pub fn eliminate_flags(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        let basic = crate::hfst_basic_transducer::HfstBasicTransducer::new_from_transducer(self);
        let flags = basic.get_flags();
        let filter = get_flag_filter(self, &flags, "")?;

        if let Some(filter) = filter {
            let mut filter_copy = HfstTransducer::new_from(&filter);
            {
                let self_copy = HfstTransducer::new_from(self);
                let filter_deref = HfstTransducer::new_from(&filter);
                // Compose the symbol-level flag-constraint filter with flags
                // encoded as ordinary symbols (see eliminate_flag for why).
                let cfg = EngineConfig {
                    xerox_composition: true,
                    ..EngineConfig::default()
                };
                filter_copy.compose_with_config(&self_copy, true, &cfg)?;
                filter_copy.compose_with_config(&filter_deref, true, &cfg)?;
            }
            flag_purge(&mut filter_copy, "")?;
            *self = filter_copy;
        } else {
            flag_purge(self, "")?;
        }

        self.optimize()
    }

    pub fn eliminate_flag(&mut self, flag: &str) -> crate::error::Result<&mut HfstTransducer<B>> {
        let basic = crate::hfst_basic_transducer::HfstBasicTransducer::new_from_transducer(self);
        let flags = basic.get_flags();
        let feature_found = flags
            .iter()
            .any(|it| crate::hfst_flag_diacritics::FdOperation::get_feature(it) == flag);
        if !feature_found {
            if !flag.contains('.') {
                crate::bail!(
                    Hfst,
                    format!(
                        "HfstTransducer::eliminate_flag: flag feature does not occur in the transducer: {}",
                        flag
                    )
                );
            } else {
                crate::bail!(
                    Hfst,
                    format!(
                        "HfstTransducer::eliminate_flag: only the flag feature must be given, no value or operator: {}",
                        flag
                    )
                );
            }
        }

        let filter = get_flag_filter(self, &flags, flag)?;
        if let Some(filter) = filter {
            let mut filter_copy = HfstTransducer::new_from(&filter);
            {
                let self_copy = HfstTransducer::new_from(self);
                let filter_deref = HfstTransducer::new_from(&filter);
                // The filter is a symbol-level constraint (built over escaped
                // flags so the flag features are ordinary symbols); apply it
                // with flag diacritics encoded as ordinary symbols in the
                // composition. Otherwise flag harmonization drops any path that
                // carries a flag of some OTHER feature, since the filter's
                // '?' (identity) will not match a foreign flag once that flag
                // is added to the filter's alphabet without an explicit arc.
                let cfg = EngineConfig {
                    xerox_composition: true,
                    ..EngineConfig::default()
                };
                filter_copy.compose_with_config(&self_copy, true, &cfg)?;
                filter_copy.compose_with_config(&filter_deref, true, &cfg)?;
            }
            flag_purge(&mut filter_copy, flag)?;
            *self = filter_copy;
        } else {
            flag_purge(self, flag)?;
        }

        self.optimize()
    }

    // -------------------------------------------------------------------------
    // ----- Epsilon removal, determinization, minimization -----
    // -------------------------------------------------------------------------

    pub fn remove_epsilons(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false;
        self.fst = self.fst.remove_epsilons();
        Ok(self)
    }

    pub fn determinize(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.determinize_with_config(&EngineConfig::default())
    }

    /// 'determinize', reading 'encode_weights' (the only engine-policy flag this op
    /// consults) from the supplied config. The tropical backend encodes weights iff
    /// 'config.encode_weights'.
    pub fn determinize_with_config(
        &mut self,
        config: &EngineConfig,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false;
        let fst = std::mem::replace(&mut self.fst, B::empty());
        self.fst = fst.determinize(config.encode_weights);
        Ok(self)
    }

    pub fn minimize(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.minimize_with_config(&EngineConfig::default())
    }

    /// 'minimize', reading 'encode_weights' from the supplied config (see
    /// 'determinize_with_config').
    pub fn minimize_with_config(
        &mut self,
        config: &EngineConfig,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false;
        let fst = std::mem::replace(&mut self.fst, B::empty());
        self.fst = fst.minimize(config.encode_weights);
        Ok(self)
    }

    pub fn optimize(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.optimize_with_config(&EngineConfig::default())
    }

    pub fn optimize_with_config(
        &mut self,
        config: &EngineConfig,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        if config.minimization {
            self.minimize_with_config(config)
        } else {
            self.determinize_with_config(config)
        }
    }

    // -------------------------------------------------------------------------
    // ----- Repeat functions -----
    // -------------------------------------------------------------------------

    pub fn repeat_star(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false;
        self.fst = self.fst.repeat_star();
        Ok(self)
    }

    pub fn repeat_plus(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false;
        self.fst = self.fst.repeat_plus();
        Ok(self)
    }

    pub fn repeat_n(&mut self, n: u32) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false; // This could be done so that is_trie is preserved
        self.fst = self.fst.repeat_n(n);
        Ok(self)
    }

    pub fn repeat_n_plus(&mut self, n: u32) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false; // This could be done so that is_trie is preserved
        let mut a = HfstTransducer::new_from(self);
        let b = HfstTransducer::new_from(a.repeat_star()?);
        self.repeat_n(n)?.concatenate(&b, true)
    }

    pub fn repeat_n_minus(&mut self, n: u32) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false; // This could be done so that is_trie is preserved
        self.fst = self.fst.repeat_le_n(n);
        Ok(self)
    }

    pub fn repeat_n_to_k(
        &mut self,
        n: u32,
        k: u32,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false; // This could be done so that is_trie is preserved
        let mut a = HfstTransducer::new_from(self);
        let b = HfstTransducer::new_from(a.repeat_n_minus(k - n)?);
        self.repeat_n(n)?.concatenate(&b, true)
    }

    // -------------------------------------------------------------------------
    // ----- Unary operators -----
    // -------------------------------------------------------------------------

    pub fn optionalize(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false; // This could be done so that is_trie is preserved
        self.fst = self.fst.optionalize();
        Ok(self)
    }

    pub fn invert(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false; // This could be done so that is_trie is preserved
        self.fst = self.fst.invert();
        Ok(self)
    }

    pub fn reverse(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false; // This could be done so that is_trie is preserved
        self.fst = self.fst.reverse();
        Ok(self)
    }

    pub fn input_project(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false; // This could be done so that is_trie is preserved
        self.fst = self.fst.extract_input_language();
        Ok(self)
    }

    pub fn output_project(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false; // This could be done so that is_trie is preserved
        self.fst = self.fst.extract_output_language();
        Ok(self)
    }

    /// `[? | flag1 | ... | flagN]` — the single-symbol identity universe with
    /// `other`'s flag diacritics inserted as ORDINARY symbols (so subtract
    /// harmonization cannot erase them). This is the building block of every
    /// flag-correct complement (hfst/hfst#349): starring it and subtracting
    /// gives `~A` / `negate`, using it unstarred and subtracting gives the term
    /// complement `\A = [? - A]`. Both must treat flags as plain symbols to
    /// match the Xerox transcript, so both share this constructor.
    pub fn identity_with_flags_of(
        other: &HfstTransducer<B>,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let mut universe = HfstTransducer::new_from_symbol("@_IDENTITY_SYMBOL_@")?;
        // diacritics will not be harmonized in subtract
        let flags = universe.insert_missing_diacritics_to_alphabet_from(other)?;
        for flag in flags.iter() {
            let tr = HfstTransducer::new_from_symbol(flag)?;
            universe.disjunct(&tr, true)?;
        }
        Ok(universe)
    }

    pub fn negate(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false; // This could be done so that is_trie is preserved

        if !self.is_automaton()? {
            crate::bail!(TransducerIsNotAutomaton);
        }

        let mut idstar = HfstTransducer::identity_with_flags_of(self)?;
        idstar.repeat_star()?;
        idstar.minimize()?;
        idstar.subtract(self, true)?;
        *self = idstar;
        Ok(self)
    }
    // -------------------------------------------------------------------------
    // ----- Longest / random / n-best paths -----
    // -------------------------------------------------------------------------

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.longest-path-size-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.longest-path-size-fn]
    pub fn longest_path_size(&self, obey_flags: bool) -> crate::error::Result<i32> {
        if self.is_cyclic()? {
            crate::bail!(TransducerIsCyclic);
        }

        if !obey_flags {
            let net = HfstBasicTransducer::new_from_transducer(self);
            return Ok(net.longest_path_size());
        }

        let mut results = HfstTwoLevelPaths::new();
        let paths_found = self.extract_longest_paths(&mut results, true /* obey flags */)?;
        if !paths_found {
            return Ok(-1);
        }
        // else, there is at least one path
        Ok(results
            .iter()
            .next()
            .expect("paths_found is true, so results has at least one entry")
            .second
            .len() as i32)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-longest-paths-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-longest-paths-fn]
    pub fn extract_longest_paths(
        &self,
        results: &mut HfstTwoLevelPaths,
        obey_flags: bool, /*,show_flags: bool*/
    ) -> crate::error::Result<bool> {
        if self.is_cyclic()? {
            crate::bail!(TransducerIsCyclic);
        }

        let net = HfstBasicTransducer::new_from_transducer(self);
        let path_lengths = net.path_sizes();
        if path_lengths.is_empty() {
            return Ok(false);
        }

        let flags = net.get_flags();

        // go through each length of accepted paths in descending order
        for path_length in path_lengths.iter().copied() {
            // create a transducer [ any any ... any any ] where the number of
            // transitions that accept any symbol (including flags) is equal to
            // current length of accepted paths
            let match_length = match_any_n_times(path_length, &flags);

            let mut xre = crate::xre::XreCompiler::<B>::new();
            let mut length_tr: HfstTransducer<B> = xre
                .compile(match_length.as_str())
                .expect("match_any_n_times builds a well-formed xre");

            // filter out the paths of current length and extract them
            length_tr.compose(self, true)?;
            length_tr.optimize()?;
            if obey_flags {
                length_tr.extract_paths_fd(results, -1, -1, true)?;
            } else {
                length_tr.extract_paths(results, -1, -1)?;
            }

            // if paths were found
            if !results.is_empty() {
                return Ok(true);
            }
        } // lengths of accepted paths gone through

        // no paths found
        Ok(false)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-random-paths-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-random-paths-fn]
    pub fn extract_random_paths(
        &self,
        results: &mut HfstTwoLevelPaths,
        max_num: i32,
    ) -> crate::error::Result<()> {
        // (The C++ round-tripped SFST and foma through TROPICAL_OPENFST_TYPE
        // to borrow its implementation. Each backend answers for itself here;
        // foma's unweighted reading is documented on its impl.)
        self.fst.extract_random_paths(results, max_num);
        Ok(())
    }

    pub fn n_best(&mut self, n: u32) -> crate::error::Result<&mut HfstTransducer<B>> {
        // (Same C++ round-trip through TROPICAL_OPENFST_TYPE as
        // extract_random_paths; each backend answers for itself here.)
        self.fst = self.fst.n_best(n);
        Ok(self)
    }

    // -------------------------------------------------------------------------
    // ----- Insert freely -----
    // -------------------------------------------------------------------------

    pub fn insert_freely_pair(
        &mut self,
        symbol_pair: &StringPair,
        harmonize: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        HfstTokenizer::check_utf8_correctness(&symbol_pair.0);
        HfstTokenizer::check_utf8_correctness(&symbol_pair.1);

        if symbol_pair.0.is_empty() || symbol_pair.1.is_empty() {
            crate::bail!(EmptyString, "insert_freely(const StringPair&)");
        }

        let tr = HfstTransducer::new_from_symbol_pair(&symbol_pair.0, &symbol_pair.1)?;
        self.insert_freely(&tr, harmonize)
    }

    pub fn insert_freely(
        &mut self,
        tr: &HfstTransducer<B>,
        harmonize: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        /* In this function, this transducer must always be harmonized
        according to tr, not the other way round. */
        // foma or no harmonization -> use our own copy of tr.
        let tr_harmonized: HfstTransducer<B> = match if harmonize {
            self.harmonize_copy(tr)?
        } else {
            None
        } {
            Some(h) => h,
            None => HfstTransducer::new_copy(tr)?,
        };

        let mut net = self.fst.to_basic()?;
        let substituting_net = tr_harmonized.fst.to_basic()?;

        net.insert_freely_graph(&substituting_net)?;
        self.fst = B::from_basic(&net)?;
        Ok(self)
    }

    pub fn insert_freely_transducer(
        &mut self,
        tr: &HfstTransducer<B>,
        harmonize: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.insert_freely(tr, harmonize)
    }

    // -------------------------------------------------------------------------
    // ----- Substitution functions -----
    // -------------------------------------------------------------------------

    pub fn substitute_with_func(
        &mut self,
        func: impl Fn(&StringPair, &mut StringPairSet) -> bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        let mut net = self.convert_to_basic_transducer()?;
        net.substitute_with_func(func)?;
        self.convert_to_hfst_transducer(net)
    }

    pub fn substitute_string(
        &mut self,
        old_symbol: &str,
        new_symbol: &str,
        input_side: bool,
        output_side: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        // empty strings are not accepted
        if old_symbol.is_empty() || new_symbol.is_empty() {
            crate::bail!(
                EmptyString,
                "substitute(const std::string&, const std::string&, bool, bool)"
            );
        }

        // if there are implementations available, use them: the per-backend
        // both-sides fast path (dead code for tropical — 'if (false && ...)') is
        // 'AlgebraBackend::substitute_symbol_fast'.
        if input_side
            && output_side
            && let Some(tmp) = self.fst.substitute_symbol_fast(old_symbol, new_symbol)
        {
            self.fst = tmp;
            return Ok(self);
        }

        // use the default HfstBasicTransducer function
        let mut net = self.convert_to_basic_transducer()?;
        net.substitute_symbol(
            &Symbol::new(old_symbol),
            &Symbol::new(new_symbol),
            input_side,
            output_side,
        )?;
        self.convert_to_hfst_transducer(net)
    }

    pub fn substitute_pair_with_pair(
        &mut self,
        old_symbol_pair: &StringPair,
        new_symbol_pair: &StringPair,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        // empty strings are not accepted
        if old_symbol_pair.0.is_empty()
            || old_symbol_pair.1.is_empty()
            || new_symbol_pair.0.is_empty()
            || new_symbol_pair.1.is_empty()
        {
            crate::bail!(
                EmptyString,
                "substitute(const StringPair&, const StringPair&)"
            );
        }

        let mut net = self.convert_to_basic_transducer()?;
        net.substitute_symbol_pair(old_symbol_pair, new_symbol_pair)?;
        self.convert_to_hfst_transducer(net)?;
        Ok(self)
    }

    pub fn substitute_pair_with_pair_set(
        &mut self,
        old_symbol_pair: &StringPair,
        new_symbol_pair_set: &StringPairSet,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        if old_symbol_pair.0.is_empty() || old_symbol_pair.1.is_empty() {
            crate::bail!(
                EmptyString,
                "substitute(const StringPair&, const StringPairSet&"
            );
        }

        let mut net = self.convert_to_basic_transducer()?;
        net.substitute_symbol_pair_with_set(old_symbol_pair, new_symbol_pair_set)?;
        self.convert_to_hfst_transducer(net)
    }

    pub fn substitute_symbol(
        &mut self,
        old_symbol: &str,
        new_symbol: &str,
        input_side: bool,
        output_side: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.substitute_string(old_symbol, new_symbol, input_side, output_side)
    }

    pub fn substitute_symbol_pair(
        &mut self,
        old_symbol_pair: &StringPair,
        new_symbol_pair: &StringPair,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.substitute_pair_with_pair(old_symbol_pair, new_symbol_pair)
    }

    pub fn substitute_symbol_pair_with_set(
        &mut self,
        old_symbol_pair: &StringPair,
        new_symbol_pair_set: &StringPairSet,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.substitute_pair_with_pair_set(old_symbol_pair, new_symbol_pair_set)
    }

    pub fn substitute_symbol_pair_with_transducer(
        &mut self,
        symbol_pair: &StringPair,
        transducer: &mut HfstTransducer<B>,
        harmonize: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.substitute_pair_with_transducer(symbol_pair, transducer, harmonize)
    }

    pub fn substitute_symbols(
        &mut self,
        substitutions: &HfstSymbolSubstitutions,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.substitute_symbol_substitutions(substitutions)
    }

    pub fn substitute_symbol_substitutions(
        &mut self,
        substitutions: &HfstSymbolSubstitutions,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        let mut net = self.convert_to_basic_transducer()?;

        net.substitute_symbols(substitutions);

        self.convert_to_hfst_transducer(net)
    }

    pub fn substitute_symbol_pairs(
        &mut self,
        substitutions: &HfstSymbolPairSubstitutions,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.substitute_symbol_pair_substitutions(substitutions)
    }

    pub fn substitute_symbol_pair_substitutions(
        &mut self,
        substitutions: &HfstSymbolPairSubstitutions,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        let mut net = self.convert_to_basic_transducer()?;
        net.substitute_symbol_pairs(substitutions);
        self.convert_to_hfst_transducer(net)
    }

    pub fn substitute_pair_with_transducer(
        &mut self,
        symbol_pair: &StringPair,
        transducer: &mut HfstTransducer<B>,
        harmonize: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        if symbol_pair.0.is_empty() || symbol_pair.1.is_empty() {
            crate::bail!(
                EmptyString,
                "substitute(const StringPair&, HfstTransducer&)"
            );
        }

        let mut pair_transducer =
            HfstTransducer::new_from_symbol_pair(&symbol_pair.0, &symbol_pair.1)?;
        if !harmonize {
            self.insert_missing_symbols_to_alphabet_from(&pair_transducer, false)?;
            pair_transducer.insert_missing_symbols_to_alphabet_from(self, false)?;
        }
        self.insert_missing_symbols_to_alphabet_from(&pair_transducer, true)?;
        pair_transducer.insert_missing_symbols_to_alphabet_from(self, true)?;

        self.harmonize(&mut pair_transducer, false)?;

        if !harmonize {
            self.insert_missing_symbols_to_alphabet_from(transducer, false)?;
            transducer.insert_missing_symbols_to_alphabet_from(self, false)?;
        }
        self.insert_missing_symbols_to_alphabet_from(transducer, true)?;
        transducer.insert_missing_symbols_to_alphabet_from(self, true)?;

        self.harmonize(transducer, false)?;

        self.fst = self
            .fst
            .substitute_string_transducer(symbol_pair.clone(), &transducer.fst);
        Ok(self)
    }

    pub fn substitute<S1: AsRef<str>, S2: AsRef<str>>(
        &mut self,
        old_symbol: S1,
        new_symbol: S2,
        input_side: bool,
        output_side: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.substitute_string(
            old_symbol.as_ref(),
            new_symbol.as_ref(),
            input_side,
            output_side,
        )
    }

    pub fn substitute_substitutions(
        &mut self,
        substitutions: &HfstSymbolSubstitutions,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.substitute_symbol_substitutions(substitutions)
    }

    // -------------------------------------------------------------------------
    // ----- Weight handling -----
    // -------------------------------------------------------------------------

    pub fn set_final_weights(
        &mut self,
        weight: f32,
        increment: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.fst = self.fst.set_final_weights(weight, increment);
        Ok(self)
    }

    pub fn push_labels(
        &mut self,
        push_type: PushType,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        let to_initial_state = push_type == PushType::TO_INITIAL_STATE;
        self.fst = self.fst.push_labels(to_initial_state);
        Ok(self)
    }

    /// Realign a transducer by pushing its labels to the start on both sides:
    /// invert, push labels to the initial state, invert back, and push again.
    /// Lifted verbatim from hfst-realign (the boundary-symbol variant is dead /
    /// commented out in the C++; this is the only realignment it performs).
    pub fn realign(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.invert()?;
        self.push_labels(PushType::TO_INITIAL_STATE)?;
        self.invert()?;
        self.push_labels(PushType::TO_INITIAL_STATE)
    }

    pub fn push_weights(
        &mut self,
        push_type: PushType,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        let to_initial_state = push_type == PushType::TO_INITIAL_STATE;
        self.fst = self.fst.push_weights(to_initial_state);
        Ok(self)
    }

    pub fn transform_weights(
        &mut self,
        func: fn(f32) -> f32,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.fst = self.fst.transform_weights(func);
        Ok(self)
    }
    // -------------------------------------------------------------------------
    // ----- Binary operators (HfstTransducer.cc ~4173-5423) -----
    // -------------------------------------------------------------------------

    pub fn merge(
        &mut self,
        another: &HfstTransducer<B>,
        args: &crate::xre::XreConstructorArguments<B>,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        let mut this_basic = HfstBasicTransducer::from_transducer(self);
        // [spec:hfst:def:hfst-transducer.hfst.another-basic-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.another-basic-fn]
        let mut another_basic = HfstBasicTransducer::from_transducer(another);
        let mut markers_added: BTreeSet<Symbol> = BTreeSet::new();
        let result = HfstBasicTransducer::merge(
            &mut this_basic,
            &mut another_basic,
            &args.list_definitions,
            &mut markers_added,
        )?;
        let mut initial_merge = HfstTransducer::from_basic(&result);
        initial_merge.optimize()?;

        // filter non-optimal paths
        // [ ? | #V ?:? ]* %#V:V ?:0 [ ? | #V ?:? | %#V:V ?:0 ]*
        // [spec:hfst:def:hfst-transducer.hfst.xre-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.xre-fn]
        let mut xre = crate::xre::XreCompiler::new_with_args(args);
        xre.set_verbosity(false);

        for it in &markers_added {
            let marker = it.clone();
            let symbol = (it.as_bytes()[1] as char).to_string(); // @X@ -> X
            let worsener_string = format!(
                "[ ? | \"{m}\" ?:? ]* \"{m}\":{s} ?:0 [ ? | \"{m}\" ?:? | \"{m}\":{s} ?:0 ]* ;",
                m = marker,
                s = symbol
            );

            let mut worsener: HfstTransducer<B> = xre
                .compile(&worsener_string)
                .expect("the merge worsener xre is well-formed");
            worsener.optimize()?;
            // [spec:hfst:def:hfst-transducer.hfst.cp-fn]
            // [spec:hfst:sem:hfst-transducer.hfst.cp-fn]
            let mut cp = initial_merge.clone();
            cp.compose(&worsener, true)?.output_project()?.optimize()?;

            initial_merge.subtract(&cp, true)?.optimize()?;
            initial_merge.substitute_symbol(&marker, internal_epsilon, true, true)?;

            // [spec:hfst:def:hfst-transducer.hfst.fsm-fn]
            // [spec:hfst:sem:hfst-transducer.hfst.fsm-fn]
            let fsm = HfstBasicTransducer::from_transducer(&initial_merge);
            let symbols = fsm.symbols_used();
            if !symbols.contains(symbol.as_str()) {
                initial_merge.remove_from_alphabet(&symbol)?;
            }
        }

        *self = initial_merge;
        Ok(self)
    }

    /// Apply a set of label substitutions by composition — the `--compose` path
    /// of hfst-substitute. `substitutions` is the disjunction of the from:to
    /// symbol pairs to apply. Builds `(substitutions ∪ (identity − input(
    /// substitutions)))*` — the substitutions plus a pass-through identity for
    /// every symbol they do not rewrite — then composes it onto the right of
    /// `self`, minimises, and composes the inverse onto the left. Lifted verbatim
    /// from hfst-substitute's perform_delayed.
    pub fn substitute_by_composition(
        &mut self,
        substitutions: &HfstTransducer<B>,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        let mut subs = substitutions.clone();
        let mut sigma_minus_subs = HfstTransducer::new_symbol_pair(
            crate::hfst_symbol_defs::internal_identity,
            crate::hfst_symbol_defs::internal_identity,
        )?;
        let mut subs_in = substitutions.clone();
        subs_in.input_project()?;
        sigma_minus_subs.subtract(&subs_in, true)?;
        subs.disjunct(&sigma_minus_subs, true)?;
        subs.repeat_star()?;
        // Compose on the right, minimise, then compose the inverse on the left
        // (C++: trans = substitution_trans->compose(trans)).
        self.compose(&subs, true)?;
        self.minimize()?;
        subs.invert()?;
        subs.compose(&*self, true)?;
        *self = subs;
        self.minimize()?;
        Ok(self)
    }

    pub fn compose(
        &mut self,
        another: &HfstTransducer<B>,
        harmonize: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.compose_with_config(another, harmonize, &EngineConfig::default())
    }

    /// 'compose', reading the engine-policy flags it consults
    /// ('flag_is_epsilon_in_composition', 'unknown_symbols_in_use',
    /// 'xerox_composition') from the supplied config.
    pub fn compose_with_config(
        &mut self,
        another: &HfstTransducer<B>,
        harmonize: bool,
        config: &EngineConfig,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.compose_with_config_and_flag_overlay(another, harmonize, config, None)
    }

    /// Compose with an optional lazy flag-diacritic self-loop overlay.
    ///
    /// The overlay must have been produced by
    /// [`Self::prepare_flag_diacritics_for_compose`] for these operands.  It is
    /// resolved to backend labels only after ordinary symbol harmonization has
    /// established the operands' shared canonical coding. Virtual overlays are
    /// rejected for backends without overlay support and for the flag-as-epsilon
    /// and Xerox composition modes, which require the eager flag path.
    pub fn compose_with_config_and_flag_overlay(
        &mut self,
        another: &HfstTransducer<B>,
        harmonize: bool,
        config: &EngineConfig,
        flag_overlay: Option<&FlagDiacriticComposeOverlay>,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        if flag_overlay.is_some() {
            if !B::SUPPORTS_FLAG_OVERLAY {
                crate::bail!(
                    Hfst,
                    "this backend does not support virtual flag composition"
                );
            }
            if config.flag_is_epsilon_in_composition {
                crate::bail!(
                    Hfst,
                    "virtual flag composition cannot be combined with flag-is-epsilon composition"
                );
            }
            if config.xerox_composition {
                crate::bail!(
                    Hfst,
                    "virtual flag composition cannot be combined with xerox composition"
                );
            }
        }

        self.is_trie = false;

        let mut another_copy: HfstTransducer<B> = another.clone();

        /* If we want flag diacritcs to be handled in the same way as epsilons
        in composition, we substitute output flags of first transducer with
        epsilons and input flags of second transducer with epsilons. */
        if config.flag_is_epsilon_in_composition {
            // The C++ caught a throw from these substitutions and rethrew it as
            // FlagDiacriticsAreNotIdentities; the ported substitute returns the
            // error instead, so remap it the same way.
            if self
                .substitute_with_func(substitute_output_flag_with_epsilon)
                .is_err()
                || another_copy
                    .substitute_with_func(substitute_input_flag_with_epsilon)
                    .is_err()
            {
                crate::bail!(FlagDiacriticsAreNotIdentities);
            }
        }

        // (The XFSM-only 'insert_missing_diacritics_to_alphabet_from' arm is
        // compiled out with the xfsm backend.)
        if config.xerox_composition {
            encode_flag_diacritics(self);
            encode_flag_diacritics(&mut another_copy);
        }

        /* Prevent harmonization (i.e. matching unknown symbols), if requested. */
        if !harmonize {
            self.insert_missing_symbols_to_alphabet_from(&another_copy, false)?;
            another_copy.insert_missing_symbols_to_alphabet_from(self, false)?;
        }

        /* Special symbols are never harmonized. */
        self.insert_missing_symbols_to_alphabet_from(&another_copy, true)?;
        another_copy.insert_missing_symbols_to_alphabet_from(self, true)?;

        // Harmonize (FOMA and XFSM took care of this by default; both are
        // compiled out).
        another_copy = self
            .harmonize_copy_owned(another_copy)?
            .expect("harmonize_copy returns Some for tropical types");

        /* Take care of unknown and identity symbols being handled right in
        composition. */
        if config.unknown_symbols_in_use {
            self.substitute_symbol("@_IDENTITY_SYMBOL_@", "@_UNKNOWN_SYMBOL_@", false, true)?;
            another_copy.substitute_symbol(
                "@_IDENTITY_SYMBOL_@",
                "@_UNKNOWN_SYMBOL_@",
                true,
                false,
            )?;
        }

        // (The HFST_OL/HFST_OLW arm threw HfstTransducerTypeMismatch — compose
        // simply does not exist on the lookup instantiations now.)
        let left = std::mem::replace(&mut self.fst, B::empty());
        self.fst = left.try_compose_owned(
            another_copy.fst,
            flag_overlay,
            config.compose_memory_limit_bytes,
        )?;

        // Revert changes made before composition
        if config.xerox_composition {
            decode_flag_diacritics(self);
        }

        if config.flag_is_epsilon_in_composition {
            self.substitute_with_func(substitute_one_sided_flags)?;
        }

        if config.unknown_symbols_in_use {
            self.substitute_with_func(substitute_single_identity_with_the_other_symbol)?;
        }

        Ok(self)
    }

    pub(crate) fn remove_illegal_flag_paths(
        &mut self,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        let alphabet = self.get_alphabet()?;
        let mut _1_flags: StringSet = StringSet::new();
        let mut _2_flags: StringSet = StringSet::new();

        // Gather _1 and _2 flag diacritics.
        for it in &alphabet {
            if !FdOperation::is_diacritic(it) {
                continue;
            }

            if it.find("_1.").is_some() {
                _1_flags.insert(it.clone());
            }

            if it.find("_2.").is_some() {
                _2_flags.insert(it.clone());
            }
        }

        // if there aren't both _1 and _2 flag diaciritcs, there can be no
        // illegal paths.
        if _1_flags.is_empty() || _2_flags.is_empty() {
            return Ok(self);
        }

        // Rename @...@ flags to $...$ flags and compile restriction.
        let mut subst: HfstSymbolSubstitutions = HfstSymbolSubstitutions::new();
        let mut back_subst: HfstSymbolSubstitutions = HfstSymbolSubstitutions::new();

        for _1_flag in &_1_flags {
            let at_flag = _1_flag.clone();
            // Replace the leading and trailing '@' (both ASCII) with '$'.
            let dollar_flag = Symbol::from(format!("${}$", &at_flag[1..at_flag.len() - 1]));

            subst.insert(at_flag.clone(), dollar_flag.clone());
            back_subst.insert(dollar_flag, at_flag);
        }

        for _2_flag in &_2_flags {
            let at_flag = _2_flag.clone();
            // Replace the leading and trailing '@' (both ASCII) with '$'.
            let dollar_flag = Symbol::from(format!("${}$", &at_flag[1..at_flag.len() - 1]));

            subst.insert(at_flag.clone(), dollar_flag.clone());
            back_subst.insert(dollar_flag, at_flag);
        }

        self.substitute_symbols(&subst)?;

        let mut restriction = get_flag_path_restriction(&_1_flags, &_2_flags);

        // Apply restrictions.
        self.compose(&restriction, true)?;
        let _ = &mut restriction;

        // Rename $...$ flags back to @...@ flags.
        self.substitute_symbols(&back_subst)?;

        Ok(self)
    }

    pub fn lenient_composition(
        &mut self,
        another: &HfstTransducer<B>,
        _harmonize: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        let mut retval = self.clone();
        // true is a dummy variable, false means do not encode epsilons
        retval
            .compose(another, true)?
            .optimize()?
            .priority_union(self)?
            .optimize()?;

        *self = retval;
        Ok(self)
    }

    pub fn cross_product(
        &mut self,
        another: &HfstTransducer<B>,
        _harmonize: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        let mut automata1 = self.clone();
        // [spec:hfst:def:hfst-transducer.hfst.automata2-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.automata2-fn]
        let mut automata2 = another.clone();

        // Check if both input transducers are automata
        // [spec:hfst:def:hfst-transducer.hfst.t1-proj-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.t1-proj-fn]
        let mut t1_proj = automata1.clone();
        t1_proj.input_project()?;
        // [spec:hfst:def:hfst-transducer.hfst.t2-proj-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.t2-proj-fn]
        let mut t2_proj = automata2.clone();
        t2_proj.input_project()?;

        if !t1_proj.compare(&automata1, true)? || !t2_proj.compare(&automata2, true)? {
            crate::bail!(TransducersAreNotAutomata, "HfstTransducer::cross_product");
        }

        // Put MARK all over lower part of automata1 and upper part of automata2,
        // and then compose them. Also, there should be created padding after
        // strings, on both sides
        automata1.insert_to_alphabet("@_MARK_@")?;
        automata2.insert_to_alphabet("@_MARK_@")?;

        let mut tok = HfstTokenizer::new();
        tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
        tok.add_multichar_symbol("@_UNKNOWN_SYMBOL_@");
        tok.add_multichar_symbol("@_MARK_@");

        // EpsilonToMark and MarkToEpsilon are paddings (if strings are not the
        // same size)
        let mut unknown_to_mark =
            HfstTransducer::from_strings("@_UNKNOWN_SYMBOL_@", "@_MARK_@", &tok)?;
        let mut epsilon_to_mark =
            HfstTransducer::from_strings("@_EPSILON_SYMBOL_@", "@_MARK_@", &tok)?;

        // [spec:hfst:def:hfst-transducer.hfst.mark-to-unknown-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.mark-to-unknown-fn]
        let mut mark_to_unknown = unknown_to_mark.clone();
        mark_to_unknown.invert()?;
        // [spec:hfst:def:hfst-transducer.hfst.mark-to-epsilon-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.mark-to-epsilon-fn]
        let mut mark_to_epsilon = epsilon_to_mark.clone();
        mark_to_epsilon.invert()?;

        unknown_to_mark.repeat_star()?.minimize()?; // minimization is safe
        epsilon_to_mark.repeat_star()?.minimize()?; // minimization is safe
        mark_to_unknown.repeat_star()?.minimize()?; // minimization is safe
        mark_to_epsilon.repeat_star()?.minimize()?; // minimization is safe

        // [spec:hfst:def:hfst-transducer.hfst.a1-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.a1-fn]
        let mut a1 = automata1.clone();
        a1.compose(&unknown_to_mark, true)?
            .optimize()?
            .concatenate(&epsilon_to_mark, true)?
            .optimize()?;

        // [spec:hfst:def:hfst-transducer.hfst.b1-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.b1-fn]
        let mut b1 = mark_to_unknown.clone();
        b1.compose(&automata2, true)?
            .optimize()?
            .concatenate(&mark_to_epsilon, true)?
            .optimize()?;

        // [spec:hfst:def:hfst-transducer.hfst.retval-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.retval-fn]
        let mut retval = a1.clone();
        retval.compose(&b1, true)?.optimize()?;

        // Expand ?:? transitions to ?:?|?
        let mut id_or_unk: StringPairSet = StringPairSet::new();
        id_or_unk.insert((
            Symbol::new_static("@_UNKNOWN_SYMBOL_@"),
            Symbol::new_static("@_UNKNOWN_SYMBOL_@"),
        ));
        id_or_unk.insert((
            Symbol::new_static("@_IDENTITY_SYMBOL_@"),
            Symbol::new_static("@_IDENTITY_SYMBOL_@"),
        ));
        retval.substitute_symbol_pair_with_set(
            &(
                Symbol::new_static("@_UNKNOWN_SYMBOL_@"),
                Symbol::new_static("@_UNKNOWN_SYMBOL_@"),
            ),
            &id_or_unk,
        )?;

        retval.remove_from_alphabet("@_MARK_@")?;

        *self = retval;
        Ok(self)
    }

    pub fn shuffle(
        &mut self,
        another: &HfstTransducer<B>,
        _b: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        // We use HfstBasicTransducers for efficiency
        let mut this_basic = HfstBasicTransducer::from_transducer(self);
        let mut another_basic = HfstBasicTransducer::from_transducer(another);

        // Expand (unknowns and) identities
        this_basic.harmonize(&mut another_basic);

        // Find out the original alphabets of both transducers
        let mut this_alphabet: StringSet = this_basic.get_alphabet().clone();
        let mut another_alphabet: StringSet = another_basic.get_alphabet().clone();

        // Op-local state replacing the former process-global shuffle flags.
        let shuffle_failed = std::cell::Cell::new(false);
        let coding_case = std::cell::Cell::new(ShuffleCoding::ENCODE_FIRST_SHUFFLE_ARGUMENT);

        // Encode first transducer, i.e. prefix each symbol with "@1"
        coding_case.set(ShuffleCoding::ENCODE_FIRST_SHUFFLE_ARGUMENT);
        this_basic.substitute_with_func(|sp, sps| {
            code_symbols_for_shuffle_impl(sp, sps, &coding_case, &shuffle_failed)
        })?;
        // also remember to remove the unprefixed symbols from the alphabet
        this_basic.remove_symbols_from_alphabet(&this_alphabet);

        // Encode second transducer, i.e. prefix each symbol with "@2"
        coding_case.set(ShuffleCoding::ENCODE_SECOND_SHUFFLE_ARGUMENT);
        another_basic.substitute_with_func(|sp, sps| {
            code_symbols_for_shuffle_impl(sp, sps, &coding_case, &shuffle_failed)
        })?;
        // also remember to remove the unprefixed symbols from the alphabet
        another_basic.remove_symbols_from_alphabet(&another_alphabet);

        // See if shuffle failed, i.e. either transducer is not an automaton
        if shuffle_failed.get() {
            shuffle_failed.set(false);
            crate::bail!(
                TransducersAreNotAutomata,
                "HfstTransducer::shuffle(const HfstTransducer&)"
            );
        }

        // The new alphabets of transducers where each symbol is prefixed
        // with "@1" or "@2"
        this_alphabet = this_basic.get_alphabet().clone();
        another_alphabet = another_basic.get_alphabet().clone();

        // Transform alphabets of transducers into string pair sets for function
        // insert_freely
        let mut this_alphabet_pairset: StringPairSet = StringPairSet::new();
        for it in &this_alphabet {
            this_alphabet_pairset.insert((it.clone(), it.clone()));
        }
        let mut another_alphabet_pairset: StringPairSet = StringPairSet::new();
        for it in &another_alphabet {
            another_alphabet_pairset.insert((it.clone(), it.clone()));
        }

        // Freely insert any number of any symbol in the first transducer
        // to the second transducer and vice versa
        this_basic.insert_freely_set(&another_alphabet_pairset, 0.0)?;
        another_basic.insert_freely_set(&this_alphabet_pairset, 0.0)?;

        // We use HfstTransducers for intersection
        let mut this1: HfstTransducer<B> = HfstTransducer::from_basic(&this_basic);
        let another1: HfstTransducer<B> = HfstTransducer::from_basic(&another_basic);

        this1.intersect(&another1, true)?;
        this1.optimize()?;

        // We use HfstBasicTransducers again
        // [spec:hfst:def:hfst-transducer.hfst.this1-basic-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.this1-basic-fn]
        let mut this1_basic = HfstBasicTransducer::from_transducer(&this1);

        // Decode the shuffled transducer, i.e. remove the prefixes
        // "@1" and "@2" from symbols
        coding_case.set(ShuffleCoding::DECODE_AFTER_SHUFFLE);
        this1_basic.substitute_with_func(|sp, sps| {
            code_symbols_for_shuffle_impl(sp, sps, &coding_case, &shuffle_failed)
        })?;
        // also remember to remove the prefixed symbols from the alphabet
        this1_basic.remove_symbols_from_alphabet(&this_alphabet);
        this1_basic.remove_symbols_from_alphabet(&another_alphabet);

        // Convert once again to HfstTransducer
        let this_finally = HfstTransducer::from_basic(&this1_basic);
        *self = this_finally;

        Ok(self)
    }

    // ---------------------- Shuffle functions end --------------------

    // Q .P. R = Q | [~[Q .u] .o. R ]
    // .u is input project
    pub fn priority_union(
        &mut self,
        another: &HfstTransducer<B>,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        let t1 = self.clone();
        // [spec:hfst:def:hfst-transducer.hfst.t2-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.t2-fn]
        let t2 = another.clone();

        // [spec:hfst:def:hfst-transducer.hfst.t1upper-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.t1upper-fn]
        let mut t1upper = t1.clone();
        t1upper.input_project()?.optimize()?;

        // DIVERGENCE from upstream C++ (hfst#341 investigation): when Q carries
        // flag diacritics, input_project keeps each flag as a LITERAL arc, so the
        // subsequent negate() treats the flag as an ordinary symbol. The flagless
        // string that Q actually accepts (flags obeyed) then falls OUTSIDE t1upper,
        // lands INSIDE the complement, and R's lower-priority mapping LEAKS through
        // — Q .P. R yields the string twice (Q's weight and R's weight) instead of
        // just Q's. Upstream shares this bug. We fix it by resolving flag diacritics
        // on the input projection FIRST: eliminate_flags rewrites t1upper into the
        // flagless automaton whose language is exactly the strings Q accepts with
        // flags obeyed — precisely the universe the complement must be taken over.
        if t1upper.has_flag_diacritics() {
            t1upper.eliminate_flags()?;
            t1upper.optimize()?;
        }

        // [spec:hfst:def:hfst-transducer.hfst.complement-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.complement-fn]
        let mut complement = t1upper.clone();
        complement.negate()?.prune_alphabet(false)?;

        complement.compose(&t2, true)?.optimize()?;

        let mut retval = t1.clone();
        retval.disjunct(&complement, true)?.optimize()?;

        *self = retval;
        Ok(self)
    }

    pub fn compose_intersect(
        &mut self,
        v: &HfstTransducerVector<B>,
        invert: bool,
        _b: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        // (The C++ converted foma inputs to TROPICAL_OPENFST_TYPE first. This
        // runs over the generic backend, so no conversion is needed.)

        // The intersection of an empty set of rules is the empty language,
        // which makes the result empty.
        if v.is_empty() {
            *self = HfstTransducer::new();
            return Ok(self);
        }

        let first = &v[0];

        // If rule transducers contain word boundaries, add word boundaries to
        // the lexicon unless the lexicon already contains them.
        let rule_alphabet = first.get_alphabet()?;

        if rule_alphabet.contains("@#@") {
            let lexicon_alphabet = self.get_alphabet()?;
            let mut tokenizer = HfstTokenizer::new();
            tokenizer.add_multichar_symbol("@#@");
            tokenizer.add_multichar_symbol(internal_epsilon);
            let mut wb = HfstTransducer::from_strings(internal_epsilon, "@#@", &tokenizer)?;
            // [spec:hfst:def:hfst-transducer.hfst.wb-copy-fn]
            // [spec:hfst:sem:hfst-transducer.hfst.wb-copy-fn]
            let wb_copy = wb.clone();

            // Add the word boundary symbol to the alphabet so harmonization
            // won't touch it.
            let mut basic_this = HfstBasicTransducer::from_transducer(self);
            basic_this.add_symbol_to_alphabet(&Symbol::new_static("@#@"));
            *self = HfstTransducer::from_basic(&basic_this);

            wb.concatenate(self, true)?
                .concatenate(&wb_copy, true)?
                .optimize()?;
            *self = wb;
            let _ = lexicon_alphabet;
        }

        let mut rule_1 = v[0].clone();

        // foma / no harmonization -> use our own copy.
        let mut harmonized_lexicon: HfstTransducer<B> =
            rule_1.harmonize_copy(self)?.unwrap_or_else(|| self.clone());

        if invert {
            harmonized_lexicon.invert()?;
            harmonized_lexicon.substitute_symbol_pair(
                &(
                    Symbol::new_static("@#@"),
                    Symbol::new_static(internal_epsilon),
                ),
                &(
                    Symbol::new_static(internal_epsilon),
                    Symbol::new_static("@#@"),
                ),
            )?;
        }

        harmonized_lexicon.substitute_symbol(
            internal_identity,
            "||_IDENTITY_SYMBOL_||",
            true,
            true,
        )?;
        harmonized_lexicon.substitute_symbol(
            internal_unknown,
            "||_UNKNOWN_SYMBOL_||",
            true,
            true,
        )?;

        if v.len() == 1 {
            let mut rule_fst = v[0].clone();

            if invert {
                rule_fst.invert()?;
                rule_fst.substitute_symbol_pair(
                    &(
                        Symbol::new_static(internal_epsilon),
                        Symbol::new_static("@#@"),
                    ),
                    &(
                        Symbol::new_static("@#@"),
                        Symbol::new_static(internal_epsilon),
                    ),
                )?;
            }

            // In case there is only onw rule, compose with that.
            // [spec:hfst:def:hfst-transducer.hfst.rule-fn]
            // [spec:hfst:sem:hfst-transducer.hfst.rule-fn]
            // implementations::ComposeIntersectRule rule(rule_fst);
            //
            // The lexicon and rule basic transducers each carry their own symbol
            // coding; reindex both onto one shared `canonical` coder ONCE so their
            // symbol numbers can be combined directly in the lazy product (the
            // per-graph-coder replacement for the former process-global numbering).
            let mut canonical = crate::hfst_tropical_transducer_transition_data::SymbolCoder::new();
            let mut rule_basic = HfstBasicTransducer::from_transducer(&rule_fst);
            let mut lexicon_basic = HfstBasicTransducer::from_transducer(&harmonized_lexicon);
            lexicon_basic.intern_into(&mut canonical);
            rule_basic.intern_into(&mut canonical);
            lexicon_basic.reindex_into(&mut canonical);
            rule_basic.reindex_into(&mut canonical);

            let mut rule =
                crate::compose_intersect_rule_pair::ComposeIntersectRuleComponent::Rule(Box::new(
                    crate::compose_intersect_rule::ComposeIntersectRule::new_from_transducer(
                        &rule_basic,
                    ),
                ));

            // Create a ComposeIntersectLexicon from *harmonized_lexicon.
            let mut lexicon =
                crate::compose_intersect_lexicon::ComposeIntersectLexicon::new_from_transducer(
                    &lexicon_basic,
                );

            let mut res: HfstBasicTransducer = lexicon.compose_with_rules(&mut rule)?;

            // The composition inputs (the lexicon copy inside 'lexicon', the
            // rule copy inside 'rule', and the interned basics) contribute
            // nothing to the output past this point — free them before the
            // basic→backend conversion below so they don't inflate its peak.
            drop(lexicon);
            drop(rule);
            drop(rule_basic);
            drop(lexicon_basic);

            res.prune_alphabet(true);
            *self = HfstTransducer::from_basic(&res);
        } else {
            // In case there are many rules, build a ComposeIntersectRulePair
            // recursively and compose with that.
            let mut first_rule_fst = v[0].clone();

            if invert {
                first_rule_fst.invert()?;
                first_rule_fst.substitute_symbol_pair(
                    &(
                        Symbol::new_static(internal_epsilon),
                        Symbol::new_static("@#@"),
                    ),
                    &(
                        Symbol::new_static("@#@"),
                        Symbol::new_static(internal_epsilon),
                    ),
                )?;
            }

            let mut second_rule_fst = v[1].clone();

            if invert {
                second_rule_fst.invert()?;
                second_rule_fst.substitute_symbol_pair(
                    &(
                        Symbol::new_static(internal_epsilon),
                        Symbol::new_static("@#@"),
                    ),
                    &(
                        Symbol::new_static("@#@"),
                        Symbol::new_static(internal_epsilon),
                    ),
                )?;
            }

            // std::vector<implementations::ComposeIntersectRule *> rule_vector;
            // (declared but unused in the C++; omitted)
            //
            // ComposeIntersectRule * first_rule = new ComposeIntersectRule(first_rule_fst);
            // ComposeIntersectRule * second_rule = new ComposeIntersectRule(second_rule_fst);
            // ComposeIntersectRulePair * rules =
            //     new ComposeIntersectRulePair(first_rule, second_rule);
            //
            // Reindex the lexicon and every rule basic transducer onto one shared
            // `canonical` coder ONCE so their symbol numbers can be combined
            // directly in the lazy product (the per-graph-coder replacement for the
            // former process-global numbering). Build every basic transducer first,
            // intern them ALL into the shared coder, then reindex each — so even
            // alphabet-only symbols agree across all of them.
            let mut lexicon_basic = HfstBasicTransducer::from_transducer(&harmonized_lexicon);
            let mut first_rule_basic = HfstBasicTransducer::from_transducer(&first_rule_fst);
            let mut second_rule_basic = HfstBasicTransducer::from_transducer(&second_rule_fst);
            let mut extra_rule_basics: Vec<HfstBasicTransducer> = Vec::new();
            for it in &v[2..] {
                let mut rule_fst = it.clone();

                if invert {
                    rule_fst.invert()?;
                    rule_fst.substitute_symbol_pair(
                        &(
                            Symbol::new_static(internal_epsilon),
                            Symbol::new_static("@#@"),
                        ),
                        &(
                            Symbol::new_static("@#@"),
                            Symbol::new_static(internal_epsilon),
                        ),
                    )?;
                }
                extra_rule_basics.push(HfstBasicTransducer::from_transducer(&rule_fst));
            }

            let mut canonical = crate::hfst_tropical_transducer_transition_data::SymbolCoder::new();
            lexicon_basic.intern_into(&mut canonical);
            first_rule_basic.intern_into(&mut canonical);
            second_rule_basic.intern_into(&mut canonical);
            for rb in extra_rule_basics.iter() {
                rb.intern_into(&mut canonical);
            }
            lexicon_basic.reindex_into(&mut canonical);
            first_rule_basic.reindex_into(&mut canonical);
            second_rule_basic.reindex_into(&mut canonical);
            for rb in extra_rule_basics.iter_mut() {
                rb.reindex_into(&mut canonical);
            }

            use crate::compose_intersect_rule_pair::{
                ComposeIntersectRuleComponent, ComposeIntersectRulePair,
            };
            let first_rule = ComposeIntersectRuleComponent::Rule(Box::new(
                crate::compose_intersect_rule::ComposeIntersectRule::new_from_transducer(
                    &first_rule_basic,
                ),
            ));
            let second_rule = ComposeIntersectRuleComponent::Rule(Box::new(
                crate::compose_intersect_rule::ComposeIntersectRule::new_from_transducer(
                    &second_rule_basic,
                ),
            ));
            let mut rules = ComposeIntersectRuleComponent::Pair(Box::new(
                ComposeIntersectRulePair::new(first_rule, second_rule),
            ));

            for rule_basic in extra_rule_basics.iter() {
                // rules = new ComposeIntersectRulePair(
                //     new ComposeIntersectRule(rule_fst), rules);
                let new_rule = ComposeIntersectRuleComponent::Rule(Box::new(
                    crate::compose_intersect_rule::ComposeIntersectRule::new_from_transducer(
                        rule_basic,
                    ),
                ));
                rules = ComposeIntersectRuleComponent::Pair(Box::new(
                    ComposeIntersectRulePair::new(new_rule, rules),
                ));
            }

            // Create a ComposeIntersectLexicon from *harmonized_lexicon.
            let mut lexicon =
                crate::compose_intersect_lexicon::ComposeIntersectLexicon::new_from_transducer(
                    &lexicon_basic,
                );
            let mut res: HfstBasicTransducer = lexicon.compose_with_rules(&mut rules)?;

            // 'delete rules;' in the C++ — and more: the lexicon copy inside
            // 'lexicon', every rule copy inside 'rules', and the interned
            // basics contribute nothing to the output past this point. Free
            // them before the basic→backend conversion below so they don't
            // inflate its peak.
            drop(lexicon);
            drop(rules);
            drop(first_rule_basic);
            drop(second_rule_basic);
            drop(extra_rule_basics);
            drop(lexicon_basic);

            res.prune_alphabet(true);
            *self = HfstTransducer::from_basic(&res);

            if invert {
                self.invert()?;
            }
        }

        drop(harmonized_lexicon);

        self.substitute_symbol("||_IDENTITY_SYMBOL_||", internal_identity, true, true)?;
        self.substitute_symbol("||_UNKNOWN_SYMBOL_||", internal_unknown, true, true)?;

        Ok(self)
    }

    pub fn concatenate(
        &mut self,
        another: &HfstTransducer<B>,
        harmonize: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false; // This could be done so that is_trie is preserved
        let another = self.harmonize_for_binary_op(another, harmonize)?;
        self.fst = self.fst.concatenate(&another.fst);
        Ok(self)
    }

    pub fn disjunct_spv(
        &mut self,
        spv: &StringPairVector,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        // The tropical backend mutates in place via the trait impl.
        self.fst.disjunct_spv(spv);
        Ok(self)
    }

    pub fn disjunct(
        &mut self,
        another: &HfstTransducer<B>,
        harmonize: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false;
        let another = self.harmonize_for_binary_op(another, harmonize)?;
        self.fst = self.fst.disjunct(&another.fst);
        Ok(self)
    }

    pub fn intersect(
        &mut self,
        another: &HfstTransducer<B>,
        harmonize: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false; // This could be done so that is_trie is preserved
        let another = self.harmonize_for_binary_op(another, harmonize)?;
        self.fst = self.fst.intersect(&another.fst);
        Ok(self)
    }

    pub fn subtract(
        &mut self,
        another: &HfstTransducer<B>,
        harmonize: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false; // This could be done so that is_trie is preserved
        let another = self.harmonize_for_binary_op(another, harmonize)?;
        self.fst = self.fst.subtract(&another.fst);
        Ok(self)
    }

    // ----- integration shims (constructor-name aliases; the 'ty' parameter is
    // the type parameter now) -----

    pub fn from_symbol(symbol: &str) -> crate::error::Result<Self> {
        HfstTransducer::new_symbol(symbol)
    }
    pub fn new_from_symbol(symbol: &str) -> crate::error::Result<Self> {
        HfstTransducer::new_symbol(symbol)
    }
    pub fn from_isymbol_osymbol(isymbol: &str, osymbol: &str) -> crate::error::Result<Self> {
        HfstTransducer::new_symbol_pair(isymbol, osymbol)
    }
    pub fn new_from_symbol_pair(isymbol: &str, osymbol: &str) -> crate::error::Result<Self> {
        HfstTransducer::new_symbol_pair(isymbol, osymbol)
    }
    pub fn from_strings(
        isymbol: &str,
        osymbol: &str,
        tokenizer: &HfstTokenizer,
    ) -> crate::error::Result<Self> {
        HfstTransducer::new_tokenized_pair(isymbol, osymbol, tokenizer)
    }
    pub fn new_string_tokenizer_type(
        utf8_str: &str,
        tokenizer: &HfstTokenizer,
    ) -> crate::error::Result<Self> {
        HfstTransducer::new_tokenized(utf8_str, tokenizer)
    }
    pub fn new_string_string_tokenizer_type(
        upper: &str,
        lower: &str,
        tokenizer: &HfstTokenizer,
    ) -> crate::error::Result<Self> {
        HfstTransducer::new_tokenized_pair(upper, lower, tokenizer)
    }
    pub fn from_string_pair_set(sps: &StringPairSet, cyclic: bool) -> crate::error::Result<Self> {
        HfstTransducer::new_string_pair_set(sps, cyclic)
    }
}

// -----------------------------------------------------------------------------
// Tropical-only operations (the C++ converted other types to
// TROPICAL_OPENFST_TYPE first; cross-type callers now do the typed conversion
// themselves).
// -----------------------------------------------------------------------------

impl HfstTransducer<StdVectorFst> {
    pub fn prune(&mut self) -> crate::error::Result<&mut HfstTransducer<StdVectorFst>> {
        let temp = TropicalWeightTransducer::prune(&self.fst);
        self.fst = temp;
        Ok(self)
    }
}

// -----------------------------------------------------------------------------
// The lookup surface — only on the two optimized-lookup instantiations.
// -----------------------------------------------------------------------------

macro_rules! ol_lookup_facade {
    ($tables:ty) => {
        impl HfstTransducer<Transducer<$tables>> {
            // The OL lookup methods take '&mut self': looking up an input that
            // contains a symbol outside the transducer's alphabet grows that
            // alphabet (initialize_input), so a lookup genuinely mutates the
            // backend. The C++ exposed these as const and mutated through a
            // const-cast; '&mut self' states the mutation honestly instead.
            pub fn lookup_string_vector(
                &mut self,
                s: &StringVector,
                limit: isize,
                time_cutoff: f64,
            ) -> crate::error::Result<HfstOneLevelPaths> {
                self.lookup_fd_string_vector(s, limit, time_cutoff)
            }

            pub fn lookup_string(
                &mut self,
                s: &str,
                limit: isize,
                time_cutoff: f64,
            ) -> crate::error::Result<HfstOneLevelPaths> {
                self.lookup_fd_string(s, limit, time_cutoff)
            }

            /// Whether `s` tokenizes into symbols this transducer already has.
            /// Unlike the lookup methods, this cannot grow the alphabet — see
            /// [`Transducer::can_tokenize`] for why the distinction is
            /// observable.
            pub fn can_tokenize(&self, s: &str) -> bool {
                self.fst.can_tokenize(s)
            }

            // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.lookup-pairs-fn]
            // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.lookup-pairs-fn]
            pub fn lookup_pairs(
                &mut self,
                s: &str,
                limit: isize,
                time_cutoff: f64,
            ) -> HfstTwoLevelPaths {
                self.fst.lookup_fd_pairs_str(s, limit, time_cutoff)
            }

            // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.lookup-fd-fn]
            // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.lookup-fd-fn]
            pub fn lookup_fd_string_vector(
                &mut self,
                s: &StringVector,
                limit: isize,
                time_cutoff: f64,
            ) -> crate::error::Result<HfstOneLevelPaths> {
                Ok(self.fst.lookup_fd_strvec(s, limit, time_cutoff))
            }

            pub fn lookup_fd_string(
                &mut self,
                s: &str,
                limit: isize,
                time_cutoff: f64,
            ) -> crate::error::Result<HfstOneLevelPaths> {
                Ok(self.fst.lookup_fd_str(s, limit, time_cutoff))
            }

            // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.lookup-fn]
            // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.lookup-fn]
            pub fn lookup_tokenizer(
                &mut self,
                tok: &HfstTokenizer,
                s: &str,
                limit: isize,
                time_cutoff: f64,
            ) -> crate::error::Result<HfstOneLevelPaths> {
                let sv: StringVector = tok.tokenize_one_level(s, false);
                self.lookup_string_vector(&sv, limit, time_cutoff)
            }

            // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-lookup-infinitely-ambiguous-fn]
            // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-lookup-infinitely-ambiguous-fn]
            pub fn is_lookup_infinitely_ambiguous_string_vector(
                &mut self,
                s: &StringVector,
            ) -> bool {
                self.fst.is_lookup_infinitely_ambiguous_strvec(s)
            }

            pub fn is_lookup_infinitely_ambiguous_string(&mut self, s: &str) -> bool {
                self.fst.is_lookup_infinitely_ambiguous_str(s)
            }
        }
    };
}

ol_lookup_facade!(WeightedTables);
ol_lookup_facade!(UnweightedTables);

// -----------------------------------------------------------------------------
// THFST <-> OLW cheap conversions — O(1) table MOVES that transfer the inner
// weighted optimized-lookup engine and preserve the facade metadata (the inner
// 'fst' is 'pub(crate)', so these must live in the hfst crate).
// -----------------------------------------------------------------------------

impl HfstTransducer<Transducer<WeightedTables>> {
    /// Re-tag this weighted optimized-lookup transducer as THFST — an O(1)
    /// table move, not a round-trip through the basic transducer; the facade
    /// metadata survives.
    // [spec:hfst:def:thfst-backend.olw-moves]
    // [spec:hfst:sem:thfst-backend.olw-moves]
    pub fn into_thfst(self) -> HfstTransducer<crate::backend_thfst::ThfstTransducer> {
        rewrap_facade(self, crate::backend_thfst::ThfstTransducer::from_ol)
    }
}

impl HfstTransducer<crate::backend_thfst::ThfstTransducer> {
    /// Re-tag this THFST transducer as weighted optimized-lookup — the inverse
    /// O(1) table move; the facade metadata survives.
    pub fn into_olw(self) -> HfstTransducer<Transducer<WeightedTables>> {
        rewrap_facade(self, |b| b.into_ol())
    }
}

// -----------------------------------------------------------------------------
// Flag-elimination helpers (file-scope free functions in the C++).
// -----------------------------------------------------------------------------

// if (required): return ~[(?* FAIL_FLAGS) ~$SUCCEED_FLAGS SELF ?*]
// if (! required): return ~[?* FAIL_FLAGS ~$SUCCEED_FLAGS SELF ?*]
// [spec:hfst:def:hfst-transducer.hfst.new-filter-fn]
// [spec:hfst:sem:hfst-transducer.hfst.new-filter-fn]
fn new_filter<B: AlgebraBackend>(
    fail_flags: &HfstTransducer<B>,
    succeed_flags: &HfstTransducer<B>,
    this: &HfstTransducer<B>,
    required: bool,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut comp = crate::xre::XreCompiler::<B>::new();
    comp.set_expand_definitions(true);
    comp.define_transducer("Fail", fail_flags);
    comp.define_transducer("Succeed", succeed_flags);
    comp.define_transducer("Self", this);
    let mut result: HfstTransducer<B> = if required {
        comp.compile("~[(?* Fail) ~$Succeed Self ?*]")
    } else {
        comp.compile("~[?* Fail ~$Succeed Self ?*]")
    }
    .expect("the flag-filter xre is well-formed");

    // Should the xre compiler do this?
    result.remove_from_alphabet("Fail")?;
    result.remove_from_alphabet("Succeed")?;
    result.remove_from_alphabet("Self")?;

    Ok(result)
}

// Substitute each symbol '_@FLAG@' with '@FLAG@'
// [spec:hfst:def:hfst-transducer.hfst.substitute-escaped-flags-fn]
// [spec:hfst:sem:hfst-transducer.hfst.substitute-escaped-flags-fn]
fn substitute_escaped_flags<B: AlgebraBackend>(
    filter: &mut HfstTransducer<B>,
) -> crate::error::Result<()> {
    let alpha = filter.get_alphabet()?;
    for it in alpha.iter() {
        if it.len() > 1 {
            let bytes = it.as_bytes();
            if bytes[0] == b'_' && bytes[1] == b'@' {
                // 'std::string::erase(0)' drops the leading '_'; rebuild the
                // SmolStr from the remaining bytes instead of mutating in place.
                let s = Symbol::new(&it[1..]);
                filter.substitute_symbol(it, &s, true, true)?;
            }
        }
    }
    Ok(())
}

const FLAG_UNIFY: i32 = 1;
const FLAG_CLEAR: i32 = 2;
const FLAG_DISALLOW: i32 = 4;
const FLAG_NEGATIVE: i32 = 8;
const FLAG_POSITIVE: i32 = 16;
const FLAG_REQUIRE: i32 = 32;
#[allow(dead_code)]
const FLAG_EQUAL: i32 = 64;

const FLAG_FAIL: i32 = 1;
const FLAG_SUCCEED: i32 = 2;
const FLAG_NONE: i32 = 3;

// [spec:hfst:def:hfst-transducer.hfst.flag-build-fn]
// [spec:hfst:sem:hfst-transducer.hfst.flag-build-fn]
fn flag_build(
    ftype: i32,
    fname: &str,
    fvalue: &str,
    fftype: i32,
    ffname: &str,
    ffvalue: &str,
) -> i32 {
    if fname != ffname {
        return FLAG_NONE;
    }

    let mut selfnull = false; /* If current flag has no value, e.g. @R.A@ */
    if fvalue.is_empty() {
        selfnull = true;
    }

    let eq: i32 = if fvalue == ffvalue {
        0
    } else if fvalue < ffvalue {
        -1
    } else {
        1
    };

    /* U flags */
    if (ftype == FLAG_UNIFY) && (fftype == FLAG_POSITIVE) && (eq == 0) {
        return FLAG_SUCCEED;
    }
    if (ftype == FLAG_UNIFY) && (fftype == FLAG_CLEAR) {
        return FLAG_SUCCEED;
    }
    if (ftype == FLAG_UNIFY) && (fftype == FLAG_UNIFY) && (eq != 0) {
        return FLAG_FAIL;
    }
    if (ftype == FLAG_UNIFY) && (fftype == FLAG_POSITIVE) && (eq != 0) {
        return FLAG_FAIL;
    }
    if (ftype == FLAG_UNIFY) && (fftype == FLAG_NEGATIVE) && (eq == 0) {
        return FLAG_FAIL;
    }

    /* R flag with value = 0 */
    if (ftype == FLAG_REQUIRE) && (fftype == FLAG_UNIFY) && selfnull {
        return FLAG_SUCCEED;
    }
    if (ftype == FLAG_REQUIRE) && (fftype == FLAG_POSITIVE) && selfnull {
        return FLAG_SUCCEED;
    }
    if (ftype == FLAG_REQUIRE) && (fftype == FLAG_NEGATIVE) && selfnull {
        return FLAG_SUCCEED;
    }
    if (ftype == FLAG_REQUIRE) && (fftype == FLAG_CLEAR) && selfnull {
        return FLAG_FAIL;
    }

    /* R flag with value */
    if (ftype == FLAG_REQUIRE) && (fftype == FLAG_POSITIVE) && (eq == 0) && !selfnull {
        return FLAG_SUCCEED;
    }
    if (ftype == FLAG_REQUIRE) && (fftype == FLAG_UNIFY) && (eq == 0) && !selfnull {
        return FLAG_SUCCEED;
    }
    if (ftype == FLAG_REQUIRE) && (fftype == FLAG_POSITIVE) && (eq != 0) && !selfnull {
        return FLAG_FAIL;
    }
    if (ftype == FLAG_REQUIRE) && (fftype == FLAG_UNIFY) && (eq != 0) && !selfnull {
        return FLAG_FAIL;
    }
    if (ftype == FLAG_REQUIRE) && (fftype == FLAG_NEGATIVE) && !selfnull {
        return FLAG_FAIL;
    }
    if (ftype == FLAG_REQUIRE) && (fftype == FLAG_CLEAR) && !selfnull {
        return FLAG_FAIL;
    }

    /* D flag with value = 0 */
    if (ftype == FLAG_DISALLOW) && (fftype == FLAG_CLEAR) && selfnull {
        return FLAG_SUCCEED;
    }
    if (ftype == FLAG_DISALLOW) && (fftype == FLAG_POSITIVE) && selfnull {
        return FLAG_FAIL;
    }
    if (ftype == FLAG_DISALLOW) && (fftype == FLAG_UNIFY) && selfnull {
        return FLAG_FAIL;
    }
    if (ftype == FLAG_DISALLOW) && (fftype == FLAG_NEGATIVE) && selfnull {
        return FLAG_FAIL;
    }

    /* D flag with value */
    if (ftype == FLAG_DISALLOW) && (fftype == FLAG_POSITIVE) && (eq != 0) && !selfnull {
        return FLAG_SUCCEED;
    }
    if (ftype == FLAG_DISALLOW) && (fftype == FLAG_CLEAR) && !selfnull {
        return FLAG_SUCCEED;
    }
    if (ftype == FLAG_DISALLOW) && (fftype == FLAG_NEGATIVE) && (eq == 0) && !selfnull {
        return FLAG_SUCCEED;
    }
    if (ftype == FLAG_DISALLOW) && (fftype == FLAG_POSITIVE) && (eq == 0) && !selfnull {
        return FLAG_FAIL;
    }
    if (ftype == FLAG_DISALLOW) && (fftype == FLAG_UNIFY) && (eq == 0) && !selfnull {
        return FLAG_FAIL;
    }
    if (ftype == FLAG_DISALLOW) && (fftype == FLAG_NEGATIVE) && (eq != 0) && !selfnull {
        return FLAG_FAIL;
    }

    FLAG_NONE
}

// [spec:hfst:def:hfst-transducer.hfst.hfst-operator-to-char-fn]
// [spec:hfst:sem:hfst-transducer.hfst.hfst-operator-to-char-fn]
fn hfst_operator_to_char(op: &str) -> i32 {
    let c = op.as_bytes()[0];
    if c == b'U' {
        return FLAG_UNIFY;
    }
    if c == b'C' {
        return FLAG_CLEAR;
    }
    if c == b'D' {
        return FLAG_DISALLOW;
    }
    if c == b'N' {
        return FLAG_NEGATIVE;
    }
    if c == b'P' {
        return FLAG_POSITIVE;
    }
    if c == b'R' {
        return FLAG_REQUIRE;
    }
    std::panic::panic_any("invalid operator");
}

// [spec:hfst:def:hfst-transducer.hfst.is-valid-flag-combination-fn]
// [spec:hfst:sem:hfst-transducer.hfst.is-valid-flag-combination-fn]
fn is_valid_flag_combination(flag1: &str, flag2: &str) -> i32 {
    let operator1 = hfst_operator_to_char(&crate::hfst_flag_diacritics::FdOperation::get_operator(
        flag1,
    ));
    let feature1 = crate::hfst_flag_diacritics::FdOperation::get_feature(flag1);
    let value1 = crate::hfst_flag_diacritics::FdOperation::get_value(flag1);

    let operator2 = hfst_operator_to_char(&crate::hfst_flag_diacritics::FdOperation::get_operator(
        flag2,
    ));
    let feature2 = crate::hfst_flag_diacritics::FdOperation::get_feature(flag2);
    let value2 = crate::hfst_flag_diacritics::FdOperation::get_value(flag2);

    flag_build(operator1, &feature1, &value1, operator2, &feature2, &value2)
}

/* @brief Get flag filter for transducer \a transducer. */
// [spec:hfst:def:hfst-transducer.hfst.get-flag-filter-fn]
// [spec:hfst:sem:hfst-transducer.hfst.get-flag-filter-fn]
fn get_flag_filter<B: AlgebraBackend>(
    transducer: &HfstTransducer<B>,
    flags: &crate::hfst_symbol_defs::StringSet,
    flag: &str,
) -> crate::error::Result<Option<HfstTransducer<B>>> {
    let _ = transducer;
    let mut flag_found = false;
    let mut filter: Option<HfstTransducer<B>> = None;

    for f in flags.iter() {
        let this = HfstTransducer::new_from_symbol(&format!("_{}", f))?; // escape flags
        let mut succeed_flags = HfstTransducer::new();
        let mut fail_flags = HfstTransducer::new();

        let op = crate::hfst_flag_diacritics::FdOperation::get_operator(f).as_bytes()[0];
        if (flag.is_empty() || crate::hfst_flag_diacritics::FdOperation::get_feature(f) == flag)
            && (op == b'U' || op == b'R' || op == b'D')
        // Equal flag?
        {
            for flag2 in flags.iter() {
                let fstatus = is_valid_flag_combination(f, flag2);

                if fstatus == 1 {
                    fail_flags.disjunct(
                        &HfstTransducer::new_from_symbol(&format!("_{}", flag2))?,
                        true,
                    )?;
                    flag_found = true;
                } else if fstatus == 2 {
                    succeed_flags.disjunct(
                        &HfstTransducer::new_from_symbol(&format!("_{}", flag2))?,
                        true,
                    )?;
                    flag_found = true;
                }
            }
        }

        if flag_found {
            let newfilter = new_filter(
                &fail_flags,
                &succeed_flags,
                &this,
                crate::hfst_flag_diacritics::FdOperation::get_operator(f).as_bytes()[0] == b'R',
            )?;

            // intersect filter with newfilter
            match filter.as_mut() {
                None => filter = Some(newfilter),
                Some(filt) => {
                    filt.intersect(&newfilter, true)?;
                }
            }
        }
        flag_found = false;
    }

    if let Some(filt) = filter.as_mut() {
        substitute_escaped_flags(filt)?; // unescape the flags
        filt.optimize()?;
    }

    Ok(filter)
}

// Replace arcs in \a transducer that use flag \a flag with epsilon arcs
// and remove \a flag from alphabet of \a transducer. If \a flag is the empty
// string, replace/remove all flags.
// [spec:hfst:def:hfst-transducer.hfst.flag-purge-fn]
// [spec:hfst:sem:hfst-transducer.hfst.flag-purge-fn]
fn flag_purge<B: Backend>(
    transducer: &mut HfstTransducer<B>,
    flag: &str,
) -> crate::error::Result<()> {
    let mut net =
        crate::hfst_basic_transducer::HfstBasicTransducer::new_from_transducer(transducer);
    net.flag_purge(flag);
    *transducer = HfstTransducer::new_from_basic(&net)?;
    Ok(())
}

// -----------------------------------------------------------------------------
// extract_nbest helpers.
// -----------------------------------------------------------------------------

// [spec:hfst:def:hfst-transducer.hfst.match-any-n-times-fn]
// [spec:hfst:sem:hfst-transducer.hfst.match-any-n-times-fn]
fn match_any_n_times(n: u32, flags: &crate::hfst_symbol_defs::StringSet) -> String {
    let mut match_any = String::from(" [ ? ");
    for flag in flags.iter() {
        match_any = match_any + "| \"" + flag + "\" ";
    }
    match_any += " ] ";

    let mut match_length = String::from("[");
    for _i in 0..n {
        match_length += &match_any;
    }
    match_length += "]";
    match_length
}

// [spec:hfst:def:hfst-transducer.hfst.extract-strings-cb]
struct ExtractStringsCb_<'a> {
    paths: &'a mut HfstTwoLevelPaths,
    max_num: i32,
}

impl<'a> ExtractStringsCb_<'a> {
    // [spec:hfst:def:hfst-transducer.hfst.extract-strings-cb.extract-strings-cb-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.extract-strings-cb.extract-strings-cb-fn]
    fn new(p: &'a mut HfstTwoLevelPaths, max: i32) -> Self {
        ExtractStringsCb_ {
            paths: p,
            max_num: max,
        }
    }
}

impl<'a> ExtractStringsCb for ExtractStringsCb_<'a> {
    // [spec:hfst:def:hfst-transducer.hfst.extract-strings-cb.operator-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.extract-strings-cb.operator-fn]
    fn operator_call(&mut self, path: &mut HfstTwoLevelPath, is_final: bool) -> RetVal {
        if is_final {
            self.paths.insert(path.clone());
        }

        RetVal::new(
            (self.max_num < 1) || (self.paths.len() as i32) < self.max_num,
            true,
        )
    }
}
// -----------------------------------------------------------------------------
// Binary operators — free helpers.
// -----------------------------------------------------------------------------

// [spec:hfst:def:hfst-transducer.hfst.substitute-single-identity-with-the-other-symbol-fn]
// [spec:hfst:sem:hfst-transducer.hfst.substitute-single-identity-with-the-other-symbol-fn]
pub fn substitute_single_identity_with_the_other_symbol(
    sp: &StringPair,
    sps: &mut StringPairSet,
) -> bool {
    let mut isymbol: Symbol = sp.0.clone();
    let mut osymbol: Symbol = sp.1.clone();

    if isymbol == "@_IDENTITY_SYMBOL_@" && (osymbol != "@_IDENTITY_SYMBOL_@") {
        isymbol = Symbol::new_static("@_UNKNOWN_SYMBOL_@");
        sps.insert((isymbol, osymbol));
        true
    } else if osymbol == "@_IDENTITY_SYMBOL_@" && (isymbol != "@_IDENTITY_SYMBOL_@") {
        osymbol = Symbol::new_static("@_UNKNOWN_SYMBOL_@");
        sps.insert((isymbol, osymbol));
        true
    } else {
        false
    }
}

// [spec:hfst:def:hfst-transducer.hfst.substitute-unknown-identity-pairs-fn]
// [spec:hfst:sem:hfst-transducer.hfst.substitute-unknown-identity-pairs-fn]
pub fn substitute_unknown_identity_pairs(sp: &StringPair, sps: &mut StringPairSet) -> bool {
    let mut isymbol: Symbol = sp.0.clone();
    let mut osymbol: Symbol = sp.1.clone();

    if isymbol == "@_UNKNOWN_SYMBOL_@" && osymbol == "@_IDENTITY_SYMBOL_@" {
        isymbol = Symbol::new_static("@_IDENTITY_SYMBOL_@");
        osymbol = Symbol::new_static("@_IDENTITY_SYMBOL_@");
        sps.insert((isymbol, osymbol));
        return true;
    }
    false
}

// Composition with this transducer restricts _1_flags ($X.Y_1.Z$) so
// they can't succeed _2_flags ($X.Y_2.Z$) immediately. Used for
// filtering illegal combinations of flag diacritics after binary
// operations.
// [spec:hfst:def:hfst-transducer.hfst.get-flag-path-restriction-fn]
// [spec:hfst:sem:hfst-transducer.hfst.get-flag-path-restriction-fn]
pub fn get_flag_path_restriction<B: Backend>(
    _1_flags: &StringSet,
    _2_flags: &StringSet,
) -> HfstTransducer<B> {
    // Two state fst with borh states final.
    let mut basic_restriction = HfstBasicTransducer::new();
    basic_restriction.add_state_new();
    let start_state: HfstState = 0;
    let seen_2_state: HfstState = 1;

    basic_restriction.set_final_weight(start_state, &0.0);
    basic_restriction.set_final_weight(seen_2_state, &0.0);

    let tr = HfstBasicTransition::new_symbols(
        start_state,
        Symbol::new_static(internal_identity),
        Symbol::new_static(internal_identity),
        0.0,
        basic_restriction.coder_mut(),
    );
    basic_restriction.add_transition(start_state, &tr, true);

    let tr = HfstBasicTransition::new_symbols(
        start_state,
        Symbol::new_static(internal_identity),
        Symbol::new_static(internal_identity),
        0.0,
        basic_restriction.coder_mut(),
    );
    basic_restriction.add_transition(seen_2_state, &tr, true);

    // All _1_flags are allowed as long as no _2_flags with no
    // intervening symbols were observed.
    for dollar_flag in _1_flags {
        let inner = dollar_flag
            .strip_prefix('@')
            .and_then(|s| s.strip_suffix('@'))
            .expect("flag diacritic is @-delimited");
        let dollar_flag = Symbol::from(format!("${inner}$"));

        let tr = HfstBasicTransition::new_symbols(
            start_state,
            dollar_flag.clone(),
            dollar_flag,
            0.0,
            basic_restriction.coder_mut(),
        );
        basic_restriction.add_transition(start_state, &tr, true);
    }

    // If _2_flags are observed, _1_flags are illegal before an
    // intervening regular symbol is seen.
    for dollar_flag in _2_flags {
        let inner = dollar_flag
            .strip_prefix('@')
            .and_then(|s| s.strip_suffix('@'))
            .expect("flag diacritic is @-delimited");
        let dollar_flag = Symbol::from(format!("${inner}$"));

        let tr = HfstBasicTransition::new_symbols(
            seen_2_state,
            dollar_flag.clone(),
            dollar_flag.clone(),
            0.0,
            basic_restriction.coder_mut(),
        );
        basic_restriction.add_transition(start_state, &tr, true);

        let tr = HfstBasicTransition::new_symbols(
            seen_2_state,
            dollar_flag.clone(),
            dollar_flag,
            0.0,
            basic_restriction.coder_mut(),
        );
        basic_restriction.add_transition(seen_2_state, &tr, true);
    }

    HfstTransducer::from_basic(&basic_restriction)
}

//
// -------------------- Shuffle functions --------------------
//

// Possible cases for function code_symbols_for_shuffle.
// [spec:hfst:def:hfst-transducer.hfst.shuffle-coding]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(non_camel_case_types)]
pub enum ShuffleCoding {
    ENCODE_FIRST_SHUFFLE_ARGUMENT,
    ENCODE_SECOND_SHUFFLE_ARGUMENT,
    DECODE_AFTER_SHUFFLE,
}

// A function that is given as a parameter to substitute function
// during the shuffle operation. The purpose of this function is (1)
// to encode symbols in the two argument transducers so that no symbol
// is present at both transducers or (2) to decode the symbols
// in the shuffled transducer back to the original ones.
//
// The 'coding_case'/'shuffle_failed' state was process-global in the C++
// (file-static); it is now passed in as op-local Cells owned by 'shuffle'.
// [spec:hfst:def:hfst-transducer.hfst.code-symbols-for-shuffle-fn]
// [spec:hfst:sem:hfst-transducer.hfst.code-symbols-for-shuffle-fn]
fn code_symbols_for_shuffle_impl(
    sp: &StringPair,
    sps: &mut StringPairSet,
    coding_case: &std::cell::Cell<ShuffleCoding>,
    shuffle_failed: &std::cell::Cell<bool>,
) -> bool {
    // not automaton, shuffle fails
    if sp.0 != sp.1 {
        shuffle_failed.set(true);
        return false;
    }
    // special symbols are not coded, except identities
    if is_epsilon(&sp.0) || is_unknown(&sp.0) {
        return false;
    }
    let case = coding_case.get();
    match case {
        // substitute each symbol foo in the first argument transducer
        // with a symbol @1foo
        ShuffleCoding::ENCODE_FIRST_SHUFFLE_ARGUMENT => {
            let symbol_escaped = Symbol::from(format!("@1{}", sp.0));
            let new_sp: StringPair = (symbol_escaped.clone(), symbol_escaped);
            sps.insert(new_sp);
        }
        // substitute each symbol bar in the second argument transducer
        // with a symbol @2bar
        ShuffleCoding::ENCODE_SECOND_SHUFFLE_ARGUMENT => {
            let symbol_escaped = Symbol::from(format!("@2{}", sp.0));
            let new_sp: StringPair = (symbol_escaped.clone(), symbol_escaped);
            sps.insert(new_sp);
        }
        // substitute each symbol @1foo or @2bar in the shuffled transducer
        // with the original foo or bar.
        ShuffleCoding::DECODE_AFTER_SHUFFLE => {
            let symbol_unescaped = Symbol::new(&sp.0[2..]);
            let new_sp: StringPair = (symbol_unescaped.clone(), symbol_unescaped);
            sps.insert(new_sp);
        }
    }

    true
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
    /// Exact configured allowance for OpenFst tropical compose working memory.
    /// That backend partitions it among budget-aware compose structures; it is
    /// not an exact RSS ceiling. Other backends do not honor it. `None` is
    /// unbounded.
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

impl EngineConfig {
    pub fn new() -> Self {
        Self::default()
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

// C++ file-static substitution callbacks passed to substitute_with_func; deferred
// port (signature fn(&StringPair, &mut StringPairSet) -> bool).
// [spec:hfst:def:hfst-transducer.hfst.substitute-one-sided-flags-fn]
// [spec:hfst:sem:hfst-transducer.hfst.substitute-one-sided-flags-fn]
fn substitute_one_sided_flags(sp: &StringPair, sps: &mut StringPairSet) -> bool {
    if FdOperation::is_diacritic(&sp.0) && (sp.1 == crate::hfst_symbol_defs::internal_epsilon) {
        let new_pair: StringPair = (sp.0.clone(), sp.0.clone());
        sps.insert(new_pair);
        return true;
    }
    if FdOperation::is_diacritic(&sp.1) && (sp.0 == crate::hfst_symbol_defs::internal_epsilon) {
        let new_pair: StringPair = (sp.1.clone(), sp.1.clone());
        sps.insert(new_pair);
        return true;
    }
    false
}
// [spec:hfst:def:hfst-transducer.hfst.substitute-input-flag-with-epsilon-fn]
// [spec:hfst:sem:hfst-transducer.hfst.substitute-input-flag-with-epsilon-fn]
fn substitute_input_flag_with_epsilon(sp: &StringPair, sps: &mut StringPairSet) -> bool {
    if FdOperation::is_diacritic(&sp.0) {
        let new_pair: StringPair = (
            Symbol::new_static(crate::hfst_symbol_defs::internal_epsilon),
            sp.1.clone(),
        );
        sps.insert(new_pair);
        return true;
    }
    false
}
// [spec:hfst:def:hfst-transducer.hfst.substitute-output-flag-with-epsilon-fn]
// [spec:hfst:sem:hfst-transducer.hfst.substitute-output-flag-with-epsilon-fn]
fn substitute_output_flag_with_epsilon(sp: &StringPair, sps: &mut StringPairSet) -> bool {
    if FdOperation::is_diacritic(&sp.1) {
        let new_pair: StringPair = (
            sp.0.clone(),
            Symbol::new_static(crate::hfst_symbol_defs::internal_epsilon),
        );
        sps.insert(new_pair);
        return true;
    }
    false
}
// ===== integration shims: HfstBasicTransducer<-facade ctors, method + free-fn aliases =====
impl HfstBasicTransducer {
    /// 'HfstBasicTransducer(const HfstTransducer&)' — convert a facade transducer
    /// to the interchange basic transducer. The C++ ctor goes through
    /// 'ConversionFunctions::hfst_transducer_to_hfst_basic_transducer', NOT
    /// 'HfstTransducer::get_basic_transducer' — the former also handles the
    /// HFST_OL/HFST_OLW backends and propagates the transducer name.
    pub fn from_transducer<B: Backend>(t: &HfstTransducer<B>) -> HfstBasicTransducer {
        HfstBasicTransducer::try_from_transducer(t)
            .expect("hfst_transducer_to_hfst_basic_transducer on a valid transducer cannot fail")
    }
    /// The same conversion with the error surfaced instead of panicking, for
    /// callers (the CLI tools) that report it and exit.
    pub fn try_from_transducer<B: Backend>(
        t: &HfstTransducer<B>,
    ) -> crate::error::Result<HfstBasicTransducer> {
        crate::convert_transducer_format::ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(t)
    }
    pub fn new_from_transducer<B: Backend>(t: &HfstTransducer<B>) -> HfstBasicTransducer {
        HfstBasicTransducer::from_transducer(t)
    }
    pub fn new_from_hfst_transducer<B: Backend>(t: &HfstTransducer<B>) -> HfstBasicTransducer {
        HfstBasicTransducer::from_transducer(t)
    }
    pub fn from_hfst_transducer<B: Backend>(t: &HfstTransducer<B>) -> HfstBasicTransducer {
        HfstBasicTransducer::from_transducer(t)
    }
}

// C++ 'operator<<(std::ostream &out, const HfstTransducer &t)' (HfstTransducer.cc:6419)
// — write the transducer in AT&T format. Implemented only for the internal
// (basic) transducer format: convert to a HfstBasicTransducer and write it.
pub fn write_to<W: std::io::Write, B: Backend>(out: &mut W, t: &HfstTransducer<B>) {
    let net = HfstBasicTransducer::from_transducer(t);
    // C++ writes weights for every type except SFST/FOMA (both out of scope here).
    let write_weights = t.get_type() != ImplementationType::SFST_TYPE
        && t.get_type() != ImplementationType::FOMA_TYPE;
    net.write_in_att_format_os(out, write_weights);
}

// -----------------------------------------------------------------------------
// The one runtime sum ([dec:hfst:monomorphic-backends]).
// -----------------------------------------------------------------------------

/// The single runtime type sum, produced ONLY by the stream readers
/// ('HfstInputStream::read') — the point where file bytes (whose type is data,
/// not code) enter the program. It replaces the C++ union port
/// 'TransducerImplementation' at the stream boundary; everywhere else the
/// backend is the type parameter of ['HfstTransducer'].
// [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.transducer-implementation]
pub enum AnyTransducer {
    Tropical(HfstTransducer<StdVectorFst>),
    OlW(HfstTransducer<Transducer<WeightedTables>>),
    OlU(HfstTransducer<Transducer<UnweightedTables>>),
    #[cfg(feature = "foma")]
    Foma(HfstTransducer<crate::backend_foma::FomaTransducer>),
    Thfst(HfstTransducer<crate::backend_thfst::ThfstTransducer>),
}

/// Delegate an expression over every variant (each arm monomorphizes
/// separately).
macro_rules! any_delegate {
    ($any:expr, $t:ident => $body:expr) => {
        match $any {
            AnyTransducer::Tropical($t) => $body,
            AnyTransducer::OlW($t) => $body,
            AnyTransducer::OlU($t) => $body,
            #[cfg(feature = "foma")]
            AnyTransducer::Foma($t) => $body,
            AnyTransducer::Thfst($t) => $body,
        }
    };
}

impl AnyTransducer {
    /// The stream/serialization tag: 'Backend::TYPE', except that the OL
    /// backends carry the logical OL/OLW distinction in the payload header
    /// (interim invariant: in-memory OL tables are always weighted-shaped).
    pub fn get_type(&self) -> ImplementationType {
        any_delegate!(self, t => t.fst.stream_type())
    }

    pub fn get_name(&self) -> String {
        any_delegate!(self, t => t.get_name())
    }

    pub fn set_name(&mut self, name: &str) {
        any_delegate!(self, t => t.set_name(name))
    }

    pub fn get_property(&self, property: &str) -> String {
        any_delegate!(self, t => t.get_property(property))
    }

    pub fn set_property(&mut self, property: &str, name: &str) {
        any_delegate!(self, t => t.set_property(property, name))
    }

    pub fn get_properties(&self) -> &BTreeMap<String, String> {
        any_delegate!(self, t => t.get_properties())
    }

    /// The typed conversion to the interchange transducer.
    pub fn to_basic(&self) -> crate::error::Result<HfstBasicTransducer> {
        any_delegate!(self, t => t.to_basic())
    }

    /// Write this transducer to an HFST output stream ('operator<<'); the
    /// stream's per-type logic collapses onto this single dispatch.
    pub fn write(
        &mut self,
        out: &mut crate::hfst_output_stream::HfstOutputStream,
    ) -> crate::error::Result<()> {
        any_delegate!(self, t => { out.write(t)?; Ok(()) })
    }

    /// Typed extraction from the stream sum — the C++ pattern
    /// 'HfstTransducer t(instream); t.convert(format);' of the compilers'
    /// '@bin' file loads. The matching variant moves out unchanged; any other
    /// variant converts through the interchange transducer (a typed
    /// 'convert', [dec:hfst:monomorphic-backends]), preserving the facade
    /// metadata as the C++ convert did.
    pub fn into_typed<B: FromAnyTransducer>(self) -> crate::error::Result<HfstTransducer<B>> {
        B::from_any(self)
    }
}

/// The per-backend arm of ['AnyTransducer::into_typed']: each backend takes
/// its own variant by move and converts the rest via the interchange
/// transducer.
pub trait FromAnyTransducer: Backend {
    fn from_any(any: AnyTransducer) -> crate::error::Result<HfstTransducer<Self>>;
}

/// Re-tag a facade around a transformed backend, preserving the metadata
/// (name, properties, anonymous, is_trie). The O(1)-move analogue of
/// ['any_into_backend_via_basic']: the backend is transferred by 'f', not
/// rebuilt through the interchange transducer.
/// [spec:hfst:def:thfst-backend.olw-moves]
/// [spec:hfst:sem:thfst-backend.olw-moves]
fn rewrap_facade<A: Backend, B: Backend>(
    src: HfstTransducer<A>,
    f: impl FnOnce(A) -> B,
) -> HfstTransducer<B> {
    HfstTransducer {
        name: src.name,
        props: src.props,
        anonymous: src.anonymous,
        is_trie: src.is_trie,
        fst: f(src.fst),
    }
}

/// The convert-through-basic arm of ['AnyTransducer::into_typed'].
fn any_into_backend_via_basic<B: Backend>(
    any: AnyTransducer,
) -> crate::error::Result<HfstTransducer<B>> {
    let net = any.to_basic()?;
    let mut t: HfstTransducer<B> = HfstTransducer::wrap(B::from_basic(&net)?);
    // The C++ convert replaced only the implementation; the facade metadata
    // survives.
    any_delegate!(&any, s => {
        t.name = s.name.clone();
        t.props = s.props.clone();
        t.anonymous = s.anonymous;
        t.is_trie = s.is_trie;
    });
    Ok(t)
}

impl FromAnyTransducer for StdVectorFst {
    fn from_any(any: AnyTransducer) -> crate::error::Result<HfstTransducer<Self>> {
        match any {
            AnyTransducer::Tropical(t) => Ok(t),
            other @ AnyTransducer::OlW(_) | other @ AnyTransducer::OlU(_) => {
                any_into_backend_via_basic(other)
            }
            #[cfg(feature = "foma")]
            other @ AnyTransducer::Foma(_) => any_into_backend_via_basic(other),
            other @ AnyTransducer::Thfst(_) => any_into_backend_via_basic(other),
        }
    }
}

impl FromAnyTransducer for Transducer<WeightedTables> {
    fn from_any(any: AnyTransducer) -> crate::error::Result<HfstTransducer<Self>> {
        match any {
            AnyTransducer::OlW(t) => Ok(t),
            // THFST <-> OLW is an O(1) table MOVE, not a round-trip through the
            // basic transducer: recover the inner engine and rewrap the facade
            // metadata unchanged.
            // [spec:hfst:def:thfst-backend.olw-moves]
            // [spec:hfst:sem:thfst-backend.olw-moves]
            AnyTransducer::Thfst(t) => Ok(rewrap_facade(t, |b| b.into_ol())),
            other @ AnyTransducer::Tropical(_) | other @ AnyTransducer::OlU(_) => {
                any_into_backend_via_basic(other)
            }
            #[cfg(feature = "foma")]
            other @ AnyTransducer::Foma(_) => any_into_backend_via_basic(other),
        }
    }
}

impl FromAnyTransducer for Transducer<UnweightedTables> {
    fn from_any(any: AnyTransducer) -> crate::error::Result<HfstTransducer<Self>> {
        match any {
            AnyTransducer::OlU(t) => Ok(t),
            // Any other source would need 'from_basic' into unweighted-shaped
            // tables, which the interim invariant of
            // [dec:hfst:monomorphic-backends] rules out (conversions always
            // build weighted-shaped tables); 'from_basic' reports that.
            other @ AnyTransducer::Tropical(_) | other @ AnyTransducer::OlW(_) => {
                any_into_backend_via_basic(other)
            }
            #[cfg(feature = "foma")]
            other @ AnyTransducer::Foma(_) => any_into_backend_via_basic(other),
            other @ AnyTransducer::Thfst(_) => any_into_backend_via_basic(other),
        }
    }
}

#[cfg(feature = "foma")]
impl FromAnyTransducer for crate::backend_foma::FomaTransducer {
    fn from_any(any: AnyTransducer) -> crate::error::Result<HfstTransducer<Self>> {
        match any {
            AnyTransducer::Foma(t) => Ok(t),
            other @ AnyTransducer::Tropical(_)
            | other @ AnyTransducer::OlW(_)
            | other @ AnyTransducer::OlU(_)
            | other @ AnyTransducer::Thfst(_) => any_into_backend_via_basic(other),
        }
    }
}

impl FromAnyTransducer for crate::backend_thfst::ThfstTransducer {
    fn from_any(any: AnyTransducer) -> crate::error::Result<HfstTransducer<Self>> {
        match any {
            AnyTransducer::Thfst(t) => Ok(t),
            // THFST <-> OLW is an O(1) table MOVE: transfer the inner engine
            // and rewrap the facade metadata unchanged.
            // [spec:hfst:sem:thfst-backend.olw-moves]
            AnyTransducer::OlW(t) => Ok(rewrap_facade(
                t,
                crate::backend_thfst::ThfstTransducer::from_ol,
            )),
            other @ AnyTransducer::Tropical(_) | other @ AnyTransducer::OlU(_) => {
                any_into_backend_via_basic(other)
            }
            #[cfg(feature = "foma")]
            other @ AnyTransducer::Foma(_) => any_into_backend_via_basic(other),
        }
    }
}

#[cfg(test)]
#[path = "hfst_transducer_flag_compose_overlay_tests.rs"]
mod flag_compose_overlay_tests;

#[cfg(test)]
#[path = "hfst_transducer_flag_encode_tests.rs"]
mod flag_encode_tests;
