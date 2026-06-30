//! Port of the facade 'libhfst/src/HfstTransducer.{h,cc}' (+ 'HfstApply.cc').
//!
//! 'HfstTransducer' is a tagged union: the field 'type_'
//! (['crate::hfst_data_types::ImplementationType']) selects which member of the
//! real Rust ['union TransducerImplementation'] is active. Only the OpenFST
//! (tropical/log) and 'hfst_ol' backends are compiled in this port; the
//! SFST/FOMA/XFSM/My* members of the C++ union are '#if''d out and not emitted.
//!
//! Union access is 'unsafe' and is always guarded by a 'match'/'if' on 'type_'
//! exactly as the C++ 'switch' does. C++ owning 'new X(..)' becomes
//! 'Box::into_raw(Box::new(..))'; 'delete' / 'delete_transducer' becomes
//! 'drop(Box::from_raw(p))' / 'Backend::delete_transducer(*Box::from_raw(p))'.
//! The static C++ "interfaces" map to the backend unit-structs:
//!   * 'tropical_ofst_interface.X(..)' -> ['TropicalWeightTransducer']'::X(..)'
//!   * 'log_ofst_interface.X(..)'      -> ['LogWeightTransducer']'::X(..)'
//!   * 'hfst_ol_interface.X(..)'       -> 'HfstOlTransducer::X(..)' (NOT YET
//!     PORTED -> 'unimplemented! ("deferred: HfstOlTransducer")').
//!
//! This module is the *skeleton* (the facade contract): struct + union, every
//! constructor + 'Drop', 'operator='/'assign', the basic accessors,
//! 'is_safe_conversion', every 'apply()' overload, the 'convert' family +
//! 'get_basic_transducer' + 'convert_to_*', the implementation-type availability
//! predicates, and the facade type aliases. The algebraic / method-group
//! operations (compose, disjunct, repeat_*, harmonize_, substitute,
//! insert_missing_symbols_to_alphabet_from, ...) are added by separate body
//! modules; 'apply_another' below already references the harmonization helpers
//! they provide.
//!
//! C++ 'apply' is overloaded purely on its function-pointer + trailing
//! parameters; Rust has no overloading, so the four overloads become four
//! distinct names:
//!   * 'apply'               <- trailing 'bool dummy'            (unary ops)
//!   * 'apply_n'             <- trailing 'unsigned int n'        (repeat_n, ...)
//!   * 'apply_string_string' <- trailing 'String, String'       (substitute, ...)
//!   * 'apply_another'       <- trailing 'HfstTransducer&, bool' (binary ops)
//! Backend ops are '(&Fst) -> Fst' (owned return); callers in the body modules
//! adapt them into the '*mut -> *mut' 'fn' pointers these expect via small
//! non-capturing closures that read the input by borrow and return a freshly
//! 'Box::into_raw''d result (apply deletes the old input afterwards).

#![allow(dead_code)]

use std::collections::BTreeMap;

use hfst_openfst::StdVectorFst;

use crate::HFST_THROW;
use crate::HFST_THROW_MESSAGE;
use crate::convert_transducer_format::ConversionFunctions;
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_data_types::ImplementationType;
use crate::hfst_data_types::ImplementationType::ERROR_TYPE;
use crate::hfst_data_types::ImplementationType::FOMA_TYPE;
use crate::hfst_data_types::ImplementationType::HFST_OL_TYPE;
use crate::hfst_data_types::ImplementationType::HFST_OLW_TYPE;
use crate::hfst_data_types::ImplementationType::HFST2_TYPE;
use crate::hfst_data_types::ImplementationType::LOG_OPENFST_TYPE;
use crate::hfst_data_types::ImplementationType::SFST_TYPE;
use crate::hfst_data_types::ImplementationType::TROPICAL_OPENFST_TYPE;
use crate::hfst_data_types::ImplementationType::UNSPECIFIED_TYPE;
use crate::hfst_data_types::ImplementationType::XFSM_TYPE;
use crate::hfst_data_types::StringPairSet;
use crate::hfst_data_types::StringPairVector;
use crate::hfst_data_types::StringVector;
use crate::hfst_exception_defs::EmptyStringException;
use crate::hfst_exception_defs::FunctionNotImplementedException;
use crate::hfst_exception_defs::HfstFatalException;
use crate::hfst_exception_defs::ImplementationTypeNotAvailableException;
use crate::hfst_exception_defs::SpecifiedTypeRequiredException;
use crate::hfst_exception_defs::StreamNotReadableException;
use crate::hfst_exception_defs::TransducerHasWrongTypeException;
use crate::hfst_exception_defs::TransducerTypeMismatchException;
use crate::hfst_tokenizer::HfstTokenizer;
use crate::log_weight_transducer::LogFst;
use crate::log_weight_transducer::LogWeightTransducer;
use crate::transducer::Transducer;
use crate::tropical_weight_transducer::TropicalWeightTransducer;
// integration: types referenced by the body modules but absent from the skeleton imports
use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_data_types::StringPair;
use crate::hfst_data_types::implementations::HfstState;
use crate::hfst_flag_diacritics::FdOperation;
use crate::hfst_symbol_defs::HfstSymbolPairSubstitutions;
use crate::hfst_symbol_defs::HfstSymbolSubstitutions;
use crate::hfst_symbol_defs::StringSet;

// Suppress the unused-import lint for variants only used by '#if''d-out
// backends / not-yet-reached body code paths.
const _: ImplementationType = HFST2_TYPE;

// -----------------------------------------------------------------------------
// Facade type aliases (the 'HfstTransducer'-dependent typedefs deferred out of
// 'HfstDataTypes.h' until the facade type exists).
// -----------------------------------------------------------------------------

/// 'typedef std::vector<HfstTransducer> HfstTransducerVector;'
// [spec:hfst:def:hfst-data-types.hfst.hfst-transducer-vector]
pub type HfstTransducerVector = Vec<HfstTransducer>;

/// 'typedef std::pair<HfstTransducer,HfstTransducer> HfstTransducerPair;'
// [spec:hfst:def:hfst-data-types.hfst.hfst-transducer-pair]
pub type HfstTransducerPair = (HfstTransducer, HfstTransducer);

/// 'typedef std::vector<HfstTransducerPair> HfstTransducerPairVector;'
// [spec:hfst:def:hfst-data-types.hfst.hfst-transducer-pair-vector]
pub type HfstTransducerPairVector = Vec<HfstTransducerPair>;

// -----------------------------------------------------------------------------
// The backend union.
// -----------------------------------------------------------------------------

/// The backend implementation, owned. The C++ 'union' of raw backend pointers
/// is modelled as a safe Rust enum; the active variant must agree with
/// 'HfstTransducer::type_' (which still carries the OL/OLW and
/// UNSPECIFIED/ERROR distinctions the variant alone cannot). The SFST / FOMA /
/// XFSM / My* members of the C++ union are '#if''d out and not present.
/// 'None' models the null/uninitialised backend ('UNSPECIFIED_TYPE').
// [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.transducer-implementation]
pub enum TransducerImplementation {
    Tropical(Box<StdVectorFst>),
    Log(Box<LogFst>),
    HfstOl(Box<Transducer>),
    None,
}

impl TransducerImplementation {
    /// Borrow the tropical backend. Panics if the active variant disagrees with
    /// the caller's 'type_' dispatch (the safe analogue of reading the wrong
    /// C++ union field, which was undefined behaviour).
    #[inline]
    pub(crate) fn as_tropical(&self) -> &StdVectorFst {
        match self {
            TransducerImplementation::Tropical(b) => b,
            _ => panic!("TransducerImplementation: active variant is not Tropical"),
        }
    }
    #[inline]
    pub(crate) fn as_tropical_mut(&mut self) -> &mut StdVectorFst {
        match self {
            TransducerImplementation::Tropical(b) => b,
            _ => panic!("TransducerImplementation: active variant is not Tropical"),
        }
    }
    #[inline]
    pub(crate) fn as_log(&self) -> &LogFst {
        match self {
            TransducerImplementation::Log(b) => b,
            _ => panic!("TransducerImplementation: active variant is not Log"),
        }
    }
    #[inline]
    pub(crate) fn as_log_mut(&mut self) -> &mut LogFst {
        match self {
            TransducerImplementation::Log(b) => b,
            _ => panic!("TransducerImplementation: active variant is not Log"),
        }
    }
    #[inline]
    pub(crate) fn as_hfst_ol(&self) -> &Transducer {
        match self {
            TransducerImplementation::HfstOl(b) => b,
            _ => panic!("TransducerImplementation: active variant is not HfstOl"),
        }
    }
    #[inline]
    pub(crate) fn as_hfst_ol_mut(&mut self) -> &mut Transducer {
        match self {
            TransducerImplementation::HfstOl(b) => b,
            _ => panic!("TransducerImplementation: active variant is not HfstOl"),
        }
    }
    // IDIOM-STAGE-2: the OL lookup methods take '&mut self' (they mutate internal
    // lookup state), but HfstTransducer exposes lookup on '&self' — as C++ does on
    // a const transducer, mutating the OL backend through a const-cast on the union
    // pointer. Until the OL lookup path no longer needs '&mut', this const-cast is
    // reached through a raw pointer.
    #[inline]
    pub(crate) fn as_hfst_ol_ptr(&self) -> *mut Transducer {
        match self {
            TransducerImplementation::HfstOl(b) => &**b as *const Transducer as *mut Transducer,
            _ => panic!("TransducerImplementation: active variant is not HfstOl"),
        }
    }
}

// -----------------------------------------------------------------------------
// The facade transducer.
// -----------------------------------------------------------------------------

/// \brief A synchronous finite-state transducer.
// [spec:hfst:def:hfst-transducer.hfst.hfst-transducer]
pub struct HfstTransducer {
    /// The backend implementation type of the transducer ('type' in C++).
    pub(crate) type_: ImplementationType,
    /// currently not used
    pub(crate) anonymous: bool,
    /// currently not used
    pub(crate) is_trie: bool,
    /// The name of the transducer.
    pub(crate) name: String,
    /// rest of fst metadata ('std::map<std::string,std::string>').
    pub(crate) props: BTreeMap<String, String>,
    /// The backend implementation.
    pub(crate) implementation: TransducerImplementation,
}

impl HfstTransducer {
    // -------------------------------------------------------------------------
    // ----- Constructors -----
    // -------------------------------------------------------------------------

    /// \brief Create an uninitialized transducer (use with care).
    ///
    /// 'HfstTransducer()'. C++ leaves 'implementation' uninitialized; we seed it
    /// with a null backend pointer ('type_ == UNSPECIFIED_TYPE' means it is
    /// never read).
    pub fn new() -> Self {
        HfstTransducer {
            type_: UNSPECIFIED_TYPE,
            anonymous: false,
            is_trie: true,
            name: String::new(),
            props: BTreeMap::new(),
            implementation: TransducerImplementation::None,
        }
    }

    /// \brief Create an empty transducer of type 'type_'.
    ///
    /// 'HfstTransducer(ImplementationType type)'.
    pub fn new_type(type_: ImplementationType) -> Self {
        if !Self::is_implementation_type_available(type_) {
            std::panic::panic_any(ImplementationTypeNotAvailableException::new(
                "ImplementationTypeNotAvailableException".to_string(),
                file!().to_string(),
                line!() as usize,
                type_,
            ));
        }
        // SFST_TYPE / FOMA_TYPE / XFSM_TYPE arms are #if'd out.
        let implementation = match type_ {
            TROPICAL_OPENFST_TYPE => TransducerImplementation::Tropical(Box::new(
                TropicalWeightTransducer::create_empty_transducer(),
            )),
            LOG_OPENFST_TYPE => TransducerImplementation::Log(Box::new(
                LogWeightTransducer::create_empty_transducer(),
            )),
            HFST_OL_TYPE | HFST_OLW_TYPE => TransducerImplementation::HfstOl(
                // implementation.hfst_ol =
                //   hfst_ol_interface.create_empty_transducer(type == HFST_OLW_TYPE);
                Box::new(
                    crate::hfst_ol_transducer::HfstOlTransducer::create_empty_transducer(
                        type_ == HFST_OLW_TYPE,
                    ),
                ),
            ),
            ERROR_TYPE => HFST_THROW!(SpecifiedTypeRequiredException),
            _ => HFST_THROW!(FunctionNotImplementedException),
        };
        HfstTransducer {
            type_,
            anonymous: false,
            is_trie: true,
            name: String::new(),
            props: BTreeMap::new(),
            implementation,
        }
    }

    /// 'HfstTransducer(const std::string &utf8_str, const HfstTokenizer&, type)'.
    pub fn new_tokenized(
        utf8_str: &str,
        multichar_symbol_tokenizer: &HfstTokenizer,
        type_: ImplementationType,
    ) -> Self {
        if !Self::is_implementation_type_available(type_) {
            std::panic::panic_any(ImplementationTypeNotAvailableException::new(
                "ImplementationTypeNotAvailableException".to_string(),
                file!().to_string(),
                line!() as usize,
                type_,
            ));
        }
        if utf8_str.is_empty() {
            HFST_THROW_MESSAGE!(
                EmptyStringException,
                "HfstTransducer(const std::string&, const HfstTokenizer&, ImplementationType)"
            );
        }
        let spv = multichar_symbol_tokenizer.tokenize(utf8_str, false);
        let implementation = match type_ {
            TROPICAL_OPENFST_TYPE => TransducerImplementation::Tropical(Box::new(
                TropicalWeightTransducer::define_transducer_spv(&spv),
            )),
            LOG_OPENFST_TYPE => TransducerImplementation::Log(Box::new(
                LogWeightTransducer::define_transducer_spv(&spv),
            )),
            ERROR_TYPE => HFST_THROW!(SpecifiedTypeRequiredException),
            _ => HFST_THROW!(FunctionNotImplementedException),
        };
        HfstTransducer {
            type_,
            anonymous: false,
            is_trie: true,
            name: String::new(),
            props: BTreeMap::new(),
            implementation,
        }
    }

    /// 'HfstTransducer(const std::string &upper, const std::string &lower,
    ///  const HfstTokenizer&, type)'.
    pub fn new_tokenized_pair(
        upper_utf8_str: &str,
        lower_utf8_str: &str,
        multichar_symbol_tokenizer: &HfstTokenizer,
        type_: ImplementationType,
    ) -> Self {
        if !Self::is_implementation_type_available(type_) {
            std::panic::panic_any(ImplementationTypeNotAvailableException::new(
                "ImplementationTypeNotAvailableException".to_string(),
                file!().to_string(),
                line!() as usize,
                type_,
            ));
        }
        if upper_utf8_str.is_empty() || lower_utf8_str.is_empty() {
            // NOTE: the C++ message is missing its closing paren; preserved.
            HFST_THROW_MESSAGE!(
                EmptyStringException,
                "HfstTransducer(const std::string&, const std::string&, const HfstTokenizer&, ImplementationType"
            );
        }
        let spv = multichar_symbol_tokenizer.tokenize_pair(upper_utf8_str, lower_utf8_str, false);
        let implementation = match type_ {
            TROPICAL_OPENFST_TYPE => TransducerImplementation::Tropical(Box::new(
                TropicalWeightTransducer::define_transducer_spv(&spv),
            )),
            LOG_OPENFST_TYPE => TransducerImplementation::Log(Box::new(
                LogWeightTransducer::define_transducer_spv(&spv),
            )),
            ERROR_TYPE => HFST_THROW!(SpecifiedTypeRequiredException),
            // C++ default here throws ImplementationTypeNotAvailableException.
            _ => std::panic::panic_any(ImplementationTypeNotAvailableException::new(
                "ImplementationTypeNotAvailableException".to_string(),
                file!().to_string(),
                line!() as usize,
                type_,
            )),
        };
        HfstTransducer {
            type_,
            anonymous: false,
            is_trie: true,
            name: String::new(),
            props: BTreeMap::new(),
            implementation,
        }
    }

    /// 'HfstTransducer(const StringPairSet &sps, type, bool cyclic=false)'.
    pub fn new_string_pair_set(
        sps: &StringPairSet,
        type_: ImplementationType,
        cyclic: bool,
    ) -> Self {
        if !Self::is_implementation_type_available(type_) {
            std::panic::panic_any(ImplementationTypeNotAvailableException::new(
                "ImplementationTypeNotAvailableException".to_string(),
                file!().to_string(),
                line!() as usize,
                type_,
            ));
        }
        for sp in sps {
            if sp.0.is_empty() || sp.1.is_empty() {
                HFST_THROW_MESSAGE!(
                    EmptyStringException,
                    "HfstTransducer(const StringPairSet&, ImplementationType, bool)"
                );
            }
        }
        let implementation = match type_ {
            TROPICAL_OPENFST_TYPE => TransducerImplementation::Tropical(Box::new(
                TropicalWeightTransducer::define_transducer_sps(sps, cyclic),
            )),
            LOG_OPENFST_TYPE => TransducerImplementation::Log(Box::new(
                LogWeightTransducer::define_transducer_sps(sps, cyclic),
            )),
            ERROR_TYPE => HFST_THROW!(SpecifiedTypeRequiredException),
            _ => HFST_THROW!(FunctionNotImplementedException),
        };
        HfstTransducer {
            type_,
            anonymous: false,
            is_trie: false,
            name: String::new(),
            props: BTreeMap::new(),
            implementation,
        }
    }

    /// 'HfstTransducer(const StringPairVector &spv, type)'.
    pub fn new_string_pair_vector(spv: &StringPairVector, type_: ImplementationType) -> Self {
        if !Self::is_implementation_type_available(type_) {
            std::panic::panic_any(ImplementationTypeNotAvailableException::new(
                "ImplementationTypeNotAvailableException".to_string(),
                file!().to_string(),
                line!() as usize,
                type_,
            ));
        }
        for it in spv {
            if it.0.is_empty() || it.1.is_empty() {
                HFST_THROW_MESSAGE!(
                    EmptyStringException,
                    "HfstTransducer(const StringPairVector&, ImplementationType)"
                );
            }
        }
        let implementation = match type_ {
            TROPICAL_OPENFST_TYPE => TransducerImplementation::Tropical(Box::new(
                TropicalWeightTransducer::define_transducer_spv(spv),
            )),
            LOG_OPENFST_TYPE => TransducerImplementation::Log(Box::new(
                LogWeightTransducer::define_transducer_spv(spv),
            )),
            ERROR_TYPE => HFST_THROW!(SpecifiedTypeRequiredException),
            _ => HFST_THROW!(FunctionNotImplementedException),
        };
        HfstTransducer {
            type_,
            anonymous: false,
            is_trie: false,
            name: String::new(),
            props: BTreeMap::new(),
            implementation,
        }
    }

    /// 'HfstTransducer(const StringVector &sv, type)'.
    ///
    /// C++ builds 'spv' then does '*this = HfstTransducer(spv, type)'. The C++
    /// object's 'implementation' is uninitialized at that point and 'type ==
    /// type', so its 'operator=' deletes that garbage (UB with no defined
    /// effect). We seed the placeholder with 'type_ == UNSPECIFIED_TYPE' so
    /// 'operator=''s delete switch is a safe no-op; the observable result
    /// ('props["name"] == ""', the copied backend, and the final 'type_') is
    /// identical.
    pub fn new_string_vector(sv: &StringVector, type_: ImplementationType) -> Self {
        let mut this = HfstTransducer {
            type_: UNSPECIFIED_TYPE,
            anonymous: false,
            is_trie: false,
            name: String::new(),
            props: BTreeMap::new(),
            implementation: TransducerImplementation::None,
        };
        let mut spv = StringPairVector::new();
        for it in sv {
            spv.push((it.clone(), it.clone()));
        }
        // *this = HfstTransducer(spv, type);
        let tmp = HfstTransducer::new_string_pair_vector(&spv, type_);
        this.operator_assign(&tmp);
        this
    }

    /// 'HfstTransducer(const std::vector<StringPairSet> &spsv, type)'.
    pub fn new_string_pair_set_vector(spsv: &[StringPairSet], type_: ImplementationType) -> Self {
        if !Self::is_implementation_type_available(type_) {
            std::panic::panic_any(ImplementationTypeNotAvailableException::new(
                "ImplementationTypeNotAvailableException".to_string(),
                file!().to_string(),
                line!() as usize,
                type_,
            ));
        }
        for it in spsv {
            for pair in it {
                if pair.0.is_empty() || pair.1.is_empty() {
                    HFST_THROW_MESSAGE!(
                        EmptyStringException,
                        "HfstTransducer(const std::vector<StringPairSet>&, ImplementationType)"
                    );
                }
            }
        }
        let implementation = match type_ {
            TROPICAL_OPENFST_TYPE => TransducerImplementation::Tropical(Box::new(
                TropicalWeightTransducer::define_transducer_spsv(spsv),
            )),
            LOG_OPENFST_TYPE => TransducerImplementation::Log(Box::new(
                LogWeightTransducer::define_transducer_spsv(spsv),
            )),
            ERROR_TYPE => HFST_THROW!(SpecifiedTypeRequiredException),
            _ => HFST_THROW!(FunctionNotImplementedException),
        };
        HfstTransducer {
            type_,
            anonymous: false,
            is_trie: false,
            name: String::new(),
            props: BTreeMap::new(),
            implementation,
        }
    }

    /// \brief Create a deep copy of transducer 'another'.
    ///
    /// 'HfstTransducer(const HfstTransducer &another)'.
    pub fn new_copy(another: &HfstTransducer) -> Self {
        let type_ = another.type_;
        if !Self::is_implementation_type_available(type_) {
            std::panic::panic_any(ImplementationTypeNotAvailableException::new(
                "ImplementationTypeNotAvailableException".to_string(),
                file!().to_string(),
                line!() as usize,
                type_,
            ));
        }
        let mut props = BTreeMap::new();
        for (k, v) in &another.props {
            if k.as_str() != "type" {
                props.insert(k.clone(), v.clone());
            }
        }
        let implementation = match type_ {
            TROPICAL_OPENFST_TYPE => TransducerImplementation::Tropical(Box::new(
                TropicalWeightTransducer::copy(another.implementation.as_tropical()),
            )),
            LOG_OPENFST_TYPE => TransducerImplementation::Log(Box::new(LogWeightTransducer::copy(
                another.implementation.as_log(),
            ))),
            HFST_OL_TYPE => TransducerImplementation::HfstOl(Box::new(Transducer::copy(
                another.implementation.as_hfst_ol(),
                false,
            ))),
            HFST_OLW_TYPE => TransducerImplementation::HfstOl(Box::new(Transducer::copy(
                another.implementation.as_hfst_ol(),
                true,
            ))),
            ERROR_TYPE => HFST_THROW!(TransducerHasWrongTypeException),
            _ => HFST_THROW!(FunctionNotImplementedException),
        };
        // NOTE: like C++, 'name' stays "" even though 'props' may carry a copied
        // "name" entry.
        HfstTransducer {
            type_,
            anonymous: another.anonymous,
            is_trie: another.is_trie,
            name: String::new(),
            props,
            implementation,
        }
    }

    /// \brief Create an HFST transducer equivalent to HFST basic transducer
    /// 'net', of type 'type_'.
    ///
    /// 'HfstTransducer(const hfst::implementations::HfstBasicTransducer &net, type)'.
    pub fn new_from_basic(net: &HfstBasicTransducer, type_: ImplementationType) -> Self {
        if !Self::is_lean_implementation_type_available(type_) {
            std::panic::panic_any(ImplementationTypeNotAvailableException::new(
                "ImplementationTypeNotAvailableException".to_string(),
                file!().to_string(),
                line!() as usize,
                type_,
            ));
        }
        // SFST_TYPE / FOMA_TYPE / XFSM_TYPE arms are #if'd out.
        let implementation = match type_ {
            TROPICAL_OPENFST_TYPE => TransducerImplementation::Tropical(Box::new(
                ConversionFunctions::hfst_basic_transducer_to_tropical_ofst(net),
            )),
            LOG_OPENFST_TYPE => TransducerImplementation::Log(Box::new(
                ConversionFunctions::hfst_basic_transducer_to_log_ofst(net),
            )),
            HFST_OL_TYPE => TransducerImplementation::HfstOl(Box::new(
                ConversionFunctions::hfst_basic_transducer_to_hfst_ol(net, false, "", None),
            )),
            HFST_OLW_TYPE => TransducerImplementation::HfstOl(Box::new(
                ConversionFunctions::hfst_basic_transducer_to_hfst_ol(net, true, "", None),
            )),
            ERROR_TYPE => HFST_THROW!(TransducerHasWrongTypeException),
            _ => HFST_THROW!(FunctionNotImplementedException),
        };
        HfstTransducer {
            type_,
            anonymous: false,
            is_trie: false,
            name: net.name.clone(), // C++: name = net.name; (after the switch)
            props: BTreeMap::new(),
            implementation,
        }
    }

    /// \brief Read a (binary) transducer from an HFST input stream.
    ///
    /// 'HfstTransducer(HfstInputStream &in)'.
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.hfst-transducer-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.hfst-transducer-fn]
    pub fn new_from_stream(instream: &mut crate::hfst_input_stream::HfstInputStream) -> Self {
        let type_ = instream.get_type();
        if !Self::is_lean_implementation_type_available(type_) {
            std::panic::panic_any(ImplementationTypeNotAvailableException::new(
                "ImplementationTypeNotAvailableException".to_string(),
                file!().to_string(),
                line!() as usize,
                type_,
            ));
        }
        // C++ leaves 'implementation' to be filled by 'in.read_transducer(*this)';
        // seed it with a null pointer first (overwritten by the read).
        let mut t = HfstTransducer {
            type_,
            anonymous: false,
            is_trie: false,
            name: String::new(),
            props: BTreeMap::new(),
            implementation: TransducerImplementation::None,
        };
        instream.read_transducer(&mut t);
        t
    }

    /// \brief Create '[symbol:symbol]' of type 'type_'.
    ///
    /// 'HfstTransducer(const std::string &symbol, type)'.
    pub fn new_symbol(symbol: &str, type_: ImplementationType) -> Self {
        if !Self::is_implementation_type_available(type_) {
            std::panic::panic_any(ImplementationTypeNotAvailableException::new(
                "ImplementationTypeNotAvailableException".to_string(),
                file!().to_string(),
                line!() as usize,
                type_,
            ));
        }
        HfstTokenizer::check_utf8_correctness(symbol);
        if symbol.is_empty() {
            HFST_THROW_MESSAGE!(
                EmptyStringException,
                "HfstTransducer(const std::string&, ImplementationType)"
            );
        }
        let implementation = match type_ {
            TROPICAL_OPENFST_TYPE => TransducerImplementation::Tropical(Box::new(
                TropicalWeightTransducer::define_transducer_symbol(symbol),
            )),
            LOG_OPENFST_TYPE => TransducerImplementation::Log(Box::new(
                LogWeightTransducer::define_transducer_symbol(symbol),
            )),
            ERROR_TYPE => HFST_THROW!(TransducerHasWrongTypeException),
            _ => HFST_THROW!(FunctionNotImplementedException),
        };
        HfstTransducer {
            type_,
            anonymous: false,
            is_trie: false,
            name: String::new(),
            props: BTreeMap::new(),
            implementation,
        }
    }

    /// \brief Create '[isymbol:osymbol]' of type 'type_'.
    ///
    /// 'HfstTransducer(const std::string &isymbol, const std::string &osymbol, type)'.
    pub fn new_symbol_pair(isymbol: &str, osymbol: &str, type_: ImplementationType) -> Self {
        if !Self::is_implementation_type_available(type_) {
            std::panic::panic_any(ImplementationTypeNotAvailableException::new(
                "ImplementationTypeNotAvailableException".to_string(),
                file!().to_string(),
                line!() as usize,
                type_,
            ));
        }
        HfstTokenizer::check_utf8_correctness(isymbol);
        HfstTokenizer::check_utf8_correctness(osymbol);
        if isymbol.is_empty() || osymbol.is_empty() {
            HFST_THROW_MESSAGE!(
                EmptyStringException,
                "HfstTransducer(const std::string&, const std::string&,  ImplementationType)"
            );
        }
        let implementation = match type_ {
            TROPICAL_OPENFST_TYPE => TransducerImplementation::Tropical(Box::new(
                TropicalWeightTransducer::define_transducer_symbol_pair(isymbol, osymbol),
            )),
            LOG_OPENFST_TYPE => TransducerImplementation::Log(Box::new(
                LogWeightTransducer::define_transducer_symbol_pair(isymbol, osymbol),
            )),
            ERROR_TYPE => HFST_THROW!(TransducerHasWrongTypeException),
            _ => HFST_THROW!(FunctionNotImplementedException),
        };
        HfstTransducer {
            type_,
            anonymous: false,
            is_trie: false,
            name: String::new(),
            props: BTreeMap::new(),
            implementation,
        }
    }

    // -------------------------------------------------------------------------
    // ----- Assignment -----
    // -------------------------------------------------------------------------

    /// 'HfstTransducer &assign(const HfstTransducer &another)' -> 'operator='.
    pub fn assign(&mut self, another: &HfstTransducer) -> &mut HfstTransducer {
        self.operator_assign(another)
    }

    /// \brief Assign this transducer a new value equivalent to 'another'.
    ///
    /// 'HfstTransducer &operator=(const HfstTransducer &another)'.
    pub fn operator_assign(&mut self, another: &HfstTransducer) -> &mut HfstTransducer {
        // #if HAVE_XFSM: XFSM_TYPE -> FunctionNotImplemented (XFSM #if'd out).

        // Check for self-assignment.
        if std::ptr::eq(
            another as *const HfstTransducer,
            self as *const HfstTransducer,
        ) {
            return self;
        }

        if self.type_ != UNSPECIFIED_TYPE && self.type_ != another.type_ {
            HFST_THROW_MESSAGE!(TransducerTypeMismatchException, "HfstTransducer::operator=");
        }

        // set some features
        self.anonymous = another.anonymous;
        self.is_trie = another.is_trie;
        let nm = another.get_name();
        self.set_name(&nm);

        // Delete old transducer. (FOMA / SFST arms #if'd out.) The enum's owned
        // backend is freed automatically when 'self.implementation' is
        // reassigned below; only the C++ ERROR guard remains.
        match self.type_ {
            TROPICAL_OPENFST_TYPE
            | LOG_OPENFST_TYPE
            | HFST_OL_TYPE
            | HFST_OLW_TYPE
            | UNSPECIFIED_TYPE => {}
            // case ERROR_TYPE: default: -> TransducerHasWrongTypeException
            _ => HFST_THROW!(TransducerHasWrongTypeException),
        }

        // Set new transducer.
        let another_1 = another;
        self.type_ = another.type_;
        match self.type_ {
            TROPICAL_OPENFST_TYPE => {
                self.implementation = TransducerImplementation::Tropical(Box::new(
                    TropicalWeightTransducer::copy(another_1.implementation.as_tropical()),
                ));
            }
            LOG_OPENFST_TYPE => {
                self.implementation = TransducerImplementation::Log(Box::new(
                    LogWeightTransducer::copy(another_1.implementation.as_log()),
                ));
            }
            HFST_OL_TYPE => {
                self.implementation = TransducerImplementation::HfstOl(Box::new(Transducer::copy(
                    another_1.implementation.as_hfst_ol(),
                    false,
                )));
            }
            HFST_OLW_TYPE => {
                self.implementation = TransducerImplementation::HfstOl(Box::new(Transducer::copy(
                    another_1.implementation.as_hfst_ol(),
                    true,
                )));
            }
            // default: (void)1;  (implementation left unchanged)
            _ => {
                let _ = 1;
            }
        }
        self
    }

    // -------------------------------------------------------------------------
    // ----- Accessors -----
    // -------------------------------------------------------------------------

    /// \brief The implementation type of the transducer.
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-type-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-type-fn]
    pub fn get_type(&self) -> ImplementationType {
        self.type_
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
    // ----- Conversion safety / availability -----
    // -------------------------------------------------------------------------

    /// Whether the conversion requested can be done without losing information.
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-safe-conversion-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-safe-conversion-fn]
    // [spec:hfst:def:hfst-apply.hfst.hfst-transducer.is-safe-conversion-fn]
    // [spec:hfst:sem:hfst-apply.hfst.hfst-transducer.is-safe-conversion-fn]
    pub fn is_safe_conversion(original: ImplementationType, converted: ImplementationType) -> bool {
        if original == converted {
            return true;
        }
        if original == TROPICAL_OPENFST_TYPE && converted == LOG_OPENFST_TYPE {
            return false;
        }
        if original == LOG_OPENFST_TYPE && converted == TROPICAL_OPENFST_TYPE {
            return false;
        }
        if original == TROPICAL_OPENFST_TYPE || original == LOG_OPENFST_TYPE {
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

    /// Whether HFST is linked to the transducer library needed by 'type_'.
    ///
    /// ERROR_TYPE or UNSPECIFIED_TYPE return true (handled separately by callers).
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-implementation-type-available-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-implementation-type-available-fn]
    pub fn is_implementation_type_available(type_: ImplementationType) -> bool {
        // #if !HAVE_FOMA
        if type_ == FOMA_TYPE {
            return false;
        }
        // #if !HAVE_SFST
        if type_ == SFST_TYPE {
            return false;
        }
        // HAVE_OPENFST and HAVE_OPENFST_LOG: no checks emitted.
        // #if !HAVE_XFSM
        if type_ == XFSM_TYPE {
            return false;
        }
        let _ = type_;
        true
    }

    /// Whether HFST offers at least reading, writing, and conversion for 'type_'.
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-lean-implementation-type-available-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-lean-implementation-type-available-fn]
    pub fn is_lean_implementation_type_available(type_: ImplementationType) -> bool {
        // #if !HAVE_FOMA
        if type_ == FOMA_TYPE {
            return false;
        }
        // #if !HAVE_SFST && !HAVE_LEAN_SFST
        if type_ == SFST_TYPE {
            return false;
        }
        // HAVE_OPENFST / HAVE_OPENFST_LOG: no checks emitted.
        // #if !HAVE_XFSM
        if type_ == XFSM_TYPE {
            return false;
        }
        let _ = type_;
        true
    }

    // -------------------------------------------------------------------------
    // ----- Conversion functions -----
    // -------------------------------------------------------------------------

    /// For internal use: create an 'HfstBasicTransducer' equivalent to '*this'.
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-basic-transducer-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-basic-transducer-fn]
    pub fn get_basic_transducer(&self) -> HfstBasicTransducer {
        // SFST arm #if'd out.
        if self.type_ == TROPICAL_OPENFST_TYPE {
            return ConversionFunctions::tropical_ofst_to_hfst_basic_transducer(
                self.implementation.as_tropical(),
                true,
            );
        }
        if self.type_ == LOG_OPENFST_TYPE {
            return ConversionFunctions::log_ofst_to_hfst_basic_transducer(
                self.implementation.as_log(),
                true,
            );
        }
        // FOMA arm #if'd out.
        if self.type_ == ERROR_TYPE {
            HFST_THROW!(TransducerHasWrongTypeException);
        }
        HFST_THROW!(FunctionNotImplementedException)
    }

    /// Return a copy with every transition labelled `symbol` (on either the
    /// input or output side) removed, surviving states renumbered. Converts to a
    /// basic transducer, applies [`HfstBasicTransducer::kill_paths`], and
    /// converts back to this transducer's type. Lifted from hfst-kill-paths.
    pub fn kill_paths(&self, symbol: &str) -> HfstTransducer {
        let killed = self.get_basic_transducer().kill_paths(symbol);
        HfstTransducer::from_basic_transducer(&killed, self.get_type())
    }

    /// For internal use: create an 'HfstBasicTransducer' equivalent to '*this'
    /// and delete the backend implementation.
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.convert-to-basic-transducer-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.convert-to-basic-transducer-fn]
    pub fn convert_to_basic_transducer(&mut self) -> HfstBasicTransducer {
        // SFST arm #if'd out.
        if self.type_ == TROPICAL_OPENFST_TYPE {
            let net = ConversionFunctions::tropical_ofst_to_hfst_basic_transducer(
                self.implementation.as_tropical(),
                true,
            );
            // 'delete' the old backend: moving the enum to None drops the owned Box.
            self.implementation = TransducerImplementation::None;
            return net;
        }
        if self.type_ == LOG_OPENFST_TYPE {
            let net = ConversionFunctions::log_ofst_to_hfst_basic_transducer(
                self.implementation.as_log(),
                true,
            );
            self.implementation = TransducerImplementation::None;
            return net;
        }
        // FOMA arm #if'd out.
        if self.type_ == ERROR_TYPE {
            HFST_THROW!(TransducerHasWrongTypeException);
        }
        HFST_THROW!(FunctionNotImplementedException)
    }

    /// For internal use: build a backend of 'self.type_' equivalent to 't',
    /// delete 't', and store it as this transducer's implementation.
    pub fn convert_to_hfst_transducer(&mut self, t: HfstBasicTransducer) -> &mut HfstTransducer {
        // SFST arm #if'd out.
        if self.type_ == TROPICAL_OPENFST_TYPE {
            self.implementation = TransducerImplementation::Tropical(Box::new(
                ConversionFunctions::hfst_basic_transducer_to_tropical_ofst(&t),
            ));
            self.name = t.name.clone();
            return self;
        }
        if self.type_ == LOG_OPENFST_TYPE {
            self.implementation = TransducerImplementation::Log(Box::new(
                ConversionFunctions::hfst_basic_transducer_to_log_ofst(&t),
            ));
            self.name = t.name.clone();
            return self;
        }
        // FOMA arm #if'd out.
        if self.type_ == ERROR_TYPE {
            HFST_THROW!(TransducerHasWrongTypeException);
        }
        HFST_THROW!(FunctionNotImplementedException)
    }

    /// For internal use: create a new transducer equivalent to 't' in format
    /// 'type_'. (Static 'convert'.)
    pub fn convert_static(t: &HfstTransducer, type_: ImplementationType) -> HfstTransducer {
        if type_ == ERROR_TYPE {
            HFST_THROW_MESSAGE!(SpecifiedTypeRequiredException, "HfstTransducer::convert");
        }
        if type_ == t.type_ {
            return HfstTransducer::new_copy(t);
        }
        if !Self::is_lean_implementation_type_available(type_) {
            std::panic::panic_any(ImplementationTypeNotAvailableException::new(
                "HfstTransducer::convert".to_string(),
                file!().to_string(),
                line!() as usize,
                type_,
            ));
        }
        // [spec:hfst:def:hfst-transducer.hfst.net-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.net-fn]
        // C++: HfstBasicTransducer net(t);
        //      HfstTransducer *retval = new HfstTransducer(net, type);
        //      return *retval;
        // The HfstBasicTransducer(const HfstTransducer&) ctor is the facade's
        // get_basic_transducer (full type-dispatch incl. HFST_OL, heap-allocated
        // like the C++ stack temporary); new_from_basic is HfstTransducer(net, type).
        let net = t.get_basic_transducer();
        HfstTransducer::new_from_basic(&net, type_)
    }

    /// \brief Convert the transducer into an equivalent transducer in format
    /// 'type_'. (Member 'convert'.)
    pub fn convert(&mut self, type_: ImplementationType, options: String) -> &mut HfstTransducer {
        if !Self::is_lean_implementation_type_available(self.type_) {
            HFST_THROW_MESSAGE!(
                HfstFatalException,
                "HfstTransducer::convert: the original type of the transducer is not available!"
            );
        }
        if type_ == ERROR_TYPE {
            HFST_THROW_MESSAGE!(SpecifiedTypeRequiredException, "HfstTransducer::convert");
        }
        if type_ == self.type_ {
            return self;
        }
        if !Self::is_lean_implementation_type_available(type_) {
            std::panic::panic_any(ImplementationTypeNotAvailableException::new(
                "HfstTransducer::convert".to_string(),
                file!().to_string(),
                line!() as usize,
                type_,
            ));
        }

        // FOMA / XFSM arms #if'd out.
        let internal: HfstBasicTransducer = match self.type_ {
            SFST_TYPE => {
                // SFST #if'd out.
                unimplemented!("deferred: SfstTransducer")
            }
            TROPICAL_OPENFST_TYPE => {
                let net = ConversionFunctions::tropical_ofst_to_hfst_basic_transducer(
                    self.implementation.as_tropical(),
                    true,
                );
                self.implementation = TransducerImplementation::None;
                net
            }
            LOG_OPENFST_TYPE => {
                let net = ConversionFunctions::log_ofst_to_hfst_basic_transducer(
                    self.implementation.as_log(),
                    true,
                );
                self.implementation = TransducerImplementation::None;
                net
            }
            HFST_OL_TYPE | HFST_OLW_TYPE => {
                let net = ConversionFunctions::hfst_ol_to_hfst_basic_transducer(
                    self.implementation.as_hfst_ol(),
                );
                self.implementation = TransducerImplementation::None;
                net
            }
            // case ERROR_TYPE: default: throw.
            _ => HFST_THROW!(TransducerHasWrongTypeException),
        };

        self.type_ = type_;
        // SFST / FOMA / XFSM arms #if'd out.
        match self.type_ {
            TROPICAL_OPENFST_TYPE => {
                self.implementation = TransducerImplementation::Tropical(Box::new(
                    ConversionFunctions::hfst_basic_transducer_to_tropical_ofst(&internal),
                ));
            }
            LOG_OPENFST_TYPE => {
                self.implementation = TransducerImplementation::Log(Box::new(
                    ConversionFunctions::hfst_basic_transducer_to_log_ofst(&internal),
                ));
            }
            HFST_OL_TYPE | HFST_OLW_TYPE => {
                self.implementation = TransducerImplementation::HfstOl(Box::new(
                    ConversionFunctions::hfst_basic_transducer_to_hfst_ol(
                        &internal,
                        self.type_ == HFST_OLW_TYPE,
                        &options,
                        None,
                    ),
                ));
            }
            _ => HFST_THROW!(TransducerHasWrongTypeException),
        }
        self
    }

    // -------------------------------------------------------------------------
    // ----- apply() (HfstApply.cc) -----
    // -------------------------------------------------------------------------

    /// 'apply(... , bool dummy)' — unary backend ops. The backend functs borrow
    /// the current backend and return a fresh one; assigning the new enum value
    /// frees the old backend automatically (the C++ 'delete_transducer').
    pub fn apply(
        &mut self,
        tropical_ofst_funct: fn(&StdVectorFst) -> StdVectorFst,
        log_ofst_funct: fn(&LogFst) -> LogFst,
        foo: bool,
    ) -> &mut HfstTransducer {
        let _ = foo;
        // SFST / FOMA / XFSM arms #if'd out.
        match self.type_ {
            TROPICAL_OPENFST_TYPE => {
                let temp = tropical_ofst_funct(self.implementation.as_tropical());
                self.implementation = TransducerImplementation::Tropical(Box::new(temp));
            }
            LOG_OPENFST_TYPE => {
                let temp = log_ofst_funct(self.implementation.as_log());
                self.implementation = TransducerImplementation::Log(Box::new(temp));
            }
            // case ERROR_TYPE: default: -> TransducerHasWrongTypeException
            _ => HFST_THROW!(TransducerHasWrongTypeException),
        }
        self
    }

    /// 'apply' threading a single 'bool' through to the backend functor (used to
    /// pass an engine-policy flag such as 'encode_weights' that the C++ read from a
    /// file-static global). Mirrors 'apply_n'.
    pub fn apply_bool(
        &mut self,
        tropical_ofst_funct: fn(&StdVectorFst, bool) -> StdVectorFst,
        log_ofst_funct: fn(&LogFst, bool) -> LogFst,
        b: bool,
    ) -> &mut HfstTransducer {
        match self.type_ {
            TROPICAL_OPENFST_TYPE => {
                let temp = tropical_ofst_funct(self.implementation.as_tropical(), b);
                self.implementation = TransducerImplementation::Tropical(Box::new(temp));
            }
            LOG_OPENFST_TYPE => {
                let temp = log_ofst_funct(self.implementation.as_log(), b);
                self.implementation = TransducerImplementation::Log(Box::new(temp));
            }
            _ => HFST_THROW!(TransducerHasWrongTypeException),
        }
        self
    }

    /// 'apply(... , unsigned int n)'.
    pub fn apply_n(
        &mut self,
        tropical_ofst_funct: fn(&StdVectorFst, u32) -> StdVectorFst,
        log_ofst_funct: fn(&LogFst, u32) -> LogFst,
        n: u32,
    ) -> &mut HfstTransducer {
        match self.type_ {
            TROPICAL_OPENFST_TYPE => {
                let temp = tropical_ofst_funct(self.implementation.as_tropical(), n);
                self.implementation = TransducerImplementation::Tropical(Box::new(temp));
            }
            LOG_OPENFST_TYPE => {
                let temp = log_ofst_funct(self.implementation.as_log(), n);
                self.implementation = TransducerImplementation::Log(Box::new(temp));
            }
            _ => HFST_THROW!(TransducerHasWrongTypeException),
        }
        self
    }

    /// 'apply(... , String s1, String s2)'.
    pub fn apply_string_string(
        &mut self,
        tropical_ofst_funct: fn(&StdVectorFst, String, String) -> StdVectorFst,
        log_ofst_funct: fn(&LogFst, String, String) -> LogFst,
        s1: String,
        s2: String,
    ) -> &mut HfstTransducer {
        match self.type_ {
            TROPICAL_OPENFST_TYPE => {
                let temp = tropical_ofst_funct(self.implementation.as_tropical(), s1, s2);
                self.implementation = TransducerImplementation::Tropical(Box::new(temp));
            }
            LOG_OPENFST_TYPE => {
                let temp = log_ofst_funct(self.implementation.as_log(), s1, s2);
                self.implementation = TransducerImplementation::Log(Box::new(temp));
            }
            _ => HFST_THROW!(TransducerHasWrongTypeException),
        }
        self
    }

    /// 'apply(... , HfstTransducer &another_tr, bool harmonize)' — binary ops.
    ///
    /// References the harmonization helpers 'insert_missing_symbols_to_alphabet_from'
    /// and 'harmonize_' provided by the method-group body modules.
    pub fn apply_another(
        &mut self,
        tropical_ofst_funct: fn(&StdVectorFst, &StdVectorFst) -> StdVectorFst,
        log_ofst_funct: fn(&LogFst, &LogFst) -> LogFst,
        another_tr: &HfstTransducer,
        harmonize: bool,
    ) -> &mut HfstTransducer {
        if self.type_ != another_tr.type_ {
            HFST_THROW!(TransducerTypeMismatchException);
        }

        // [spec:hfst:def:hfst-apply.another-fn]
        // [spec:hfst:sem:hfst-apply.another-fn]
        let mut another = HfstTransducer::new_copy(another_tr);

        // prevent harmonization, if needed
        if !harmonize {
            self.insert_missing_symbols_to_alphabet_from(&another, false);
            another.insert_missing_symbols_to_alphabet_from(self, false);
        }

        // special symbols are never harmonized
        self.insert_missing_symbols_to_alphabet_from(&another, true);
        another.insert_missing_symbols_to_alphabet_from(self, true);
        // 'harmonize_' returns None for foma (use our own copy of 'another').
        let another_: HfstTransducer = self
            .harmonize_(&another)
            .unwrap_or_else(|| HfstTransducer::new_copy(&another));

        // SFST / FOMA / XFSM arms #if'd out.
        match self.type_ {
            TROPICAL_OPENFST_TYPE => {
                let temp = tropical_ofst_funct(
                    self.implementation.as_tropical(),
                    another_.implementation.as_tropical(),
                );
                self.implementation = TransducerImplementation::Tropical(Box::new(temp));
            }
            LOG_OPENFST_TYPE => {
                let temp = log_ofst_funct(
                    self.implementation.as_log(),
                    another_.implementation.as_log(),
                );
                self.implementation = TransducerImplementation::Log(Box::new(temp));
            }
            _ => HFST_THROW!(TransducerHasWrongTypeException),
        }

        self
    }
}

// -----------------------------------------------------------------------------
// Destructor.
// -----------------------------------------------------------------------------

impl Drop for HfstTransducer {
    /// '~HfstTransducer()'. Throwing (panicking) for UNSPECIFIED/ERROR mirrors
    /// the C++ destructor, which 'HFST_THROW's for those.
    fn drop(&mut self) {
        if !Self::is_lean_implementation_type_available(self.type_) {
            std::panic::panic_any(ImplementationTypeNotAvailableException::new(
                "ImplementationTypeNotAvailableException".to_string(),
                file!().to_string(),
                line!() as usize,
                self.type_,
            ));
        }
        // SFST / FOMA / XFSM arms #if'd out. The enum's owned backend is freed
        // automatically when this struct's fields drop after this method; only
        // the C++ destructor's type guards remain.
        match self.type_ {
            TROPICAL_OPENFST_TYPE | LOG_OPENFST_TYPE | HFST_OL_TYPE | HFST_OLW_TYPE => {}
            // C++ 'operator=' (the assignment path that reaches Drop when a
            // default-constructed transducer is replaced) lists
            // 'case UNSPECIFIED_TYPE: break;' -- deleting a never-assigned
            // transducer is a no-op. The C++ *destructor* lacks that case and
            // would throw, but in faithful code an UNSPECIFIED transducer is
            // always reassigned (operator=), never scope-dropped; Rust's Drop
            // serves the operator= role here, so it mirrors the no-op (the
            // implementation pointer is null, so nothing leaks).
            UNSPECIFIED_TYPE => {}
            ERROR_TYPE => HFST_THROW!(TransducerHasWrongTypeException),
            // default -> FunctionNotImplementedException
            _ => HFST_THROW!(FunctionNotImplementedException),
        }
    }
}

// ===== alphabet_harmonize (workflow body) =====
// ===== alphabet_harmonize (flattened body) =====

impl HfstTransducer {
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-profile-seconds-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-profile-seconds-fn]
    pub fn get_profile_seconds(type_: ImplementationType) -> f32 {
        if type_ == ImplementationType::TROPICAL_OPENFST_TYPE {
            return TropicalWeightTransducer::get_profile_seconds();
        }
        0.0
    }

    // -----------------------------------------------------------------------
    //
    //                   Alphabet and harmonization
    //
    // -----------------------------------------------------------------------

    // used only for SFST_TYPE
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-symbol-pairs-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-symbol-pairs-fn]
    pub fn get_symbol_pairs(&mut self) -> StringPairSet {
        crate::HFST_THROW_MESSAGE!(FunctionNotImplementedException, "get_symbol_pairs")
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.insert-to-alphabet-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.insert-to-alphabet-fn]
    pub fn insert_to_alphabet_string(&mut self, symbol: &str) {
        HfstTokenizer::check_utf8_correctness(symbol);

        if symbol.is_empty() {
            crate::HFST_THROW_MESSAGE!(EmptyStringException, "insert_to_alphabet");
        }

        if self.type_ == ImplementationType::HFST_OL_TYPE
            || self.type_ == ImplementationType::HFST_OLW_TYPE
        {
            self.implementation
                .as_hfst_ol_mut()
                .include_symbol_in_alphabet(symbol);
            return;
        }
        if self.type_ != ImplementationType::XFSM_TYPE {
            let mut net = self.convert_to_basic_transducer();
            net.add_symbol_to_alphabet(&symbol.to_string());
            self.convert_to_hfst_transducer(net);
        } else {
            std::panic::panic_any(ImplementationTypeNotAvailableException::new(
                "ImplementationTypeNotAvailableException".to_string(),
                file!().to_string(),
                line!() as usize,
                ImplementationType::XFSM_TYPE,
            ));
        }
    }

    pub fn insert_to_alphabet_string_set(&mut self, symbols: &StringSet) {
        for symbol in symbols.iter() {
            HfstTokenizer::check_utf8_correctness(symbol);
            if symbol.is_empty() {
                crate::HFST_THROW_MESSAGE!(EmptyStringException, "insert_to_alphabet");
            }
        }

        if self.type_ != ImplementationType::XFSM_TYPE {
            let mut net = self.convert_to_basic_transducer();
            net.add_symbols_to_alphabet_set(symbols);
            self.convert_to_hfst_transducer(net);
        } else {
            std::panic::panic_any(ImplementationTypeNotAvailableException::new(
                "ImplementationTypeNotAvailableException".to_string(),
                file!().to_string(),
                line!() as usize,
                ImplementationType::XFSM_TYPE,
            ));
        }
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.remove-from-alphabet-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.remove-from-alphabet-fn]
    pub fn remove_from_alphabet_string(&mut self, symbol: &str) {
        HfstTokenizer::check_utf8_correctness(symbol);

        if symbol.is_empty() {
            crate::HFST_THROW_MESSAGE!(EmptyStringException, "remove_from_alphabet");
        }

        let mut net = self.convert_to_basic_transducer();
        net.remove_symbol_from_alphabet(&symbol.to_string());
        self.convert_to_hfst_transducer(net);
    }

    pub fn remove_from_alphabet_string_set(&mut self, symbols: &StringSet) {
        for symbol in symbols.iter() {
            self.remove_from_alphabet_string(symbol);
        }
    }

    /* Implemented for XFSM_TYPE, as conversion between HfstBasicFormat and
     * XFSM_TYPE is slow. */
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.remove-symbols-from-alphabet-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.remove-symbols-from-alphabet-fn]
    pub fn remove_symbols_from_alphabet(&mut self, symbols: &StringSet) {
        if self.type_ != ImplementationType::XFSM_TYPE {
            crate::HFST_THROW_MESSAGE!(
                FunctionNotImplementedException,
                "remove_symbols_from_alphabet"
            );
        }
        let _ = symbols;
    }

    pub fn prune_alphabet(&mut self, force: bool) -> &mut HfstTransducer {
        let mut net = self.convert_to_basic_transducer();
        net.prune_alphabet(force);
        self.convert_to_hfst_transducer(net)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-initial-input-symbols-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-initial-input-symbols-fn]
    pub fn get_initial_input_symbols(&self) -> StringSet {
        match self.type_ {
            ImplementationType::TROPICAL_OPENFST_TYPE => {
                TropicalWeightTransducer::get_initial_input_symbols(
                    self.implementation.as_tropical(),
                )
            }
            _ => {
                crate::HFST_THROW_MESSAGE!(
                    FunctionNotImplementedException,
                    "get_first_input_symbols"
                )
            }
        }
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-first-input-symbols-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-first-input-symbols-fn]
    pub fn get_first_input_symbols(&self) -> StringSet {
        match self.type_ {
            ImplementationType::TROPICAL_OPENFST_TYPE => {
                TropicalWeightTransducer::get_first_input_symbols(self.implementation.as_tropical())
            }
            ImplementationType::LOG_OPENFST_TYPE => {
                crate::HFST_THROW_MESSAGE!(
                    FunctionNotImplementedException,
                    "get_first_input_symbols"
                )
            }
            ImplementationType::ERROR_TYPE => crate::HFST_THROW!(TransducerHasWrongTypeException),
            ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => {
                crate::HFST_THROW_MESSAGE!(
                    FunctionNotImplementedException,
                    "get_first_input_symbols"
                )
            }
            _ => {
                crate::HFST_THROW_MESSAGE!(
                    FunctionNotImplementedException,
                    "get_first_input_symbols"
                )
            }
        }
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-alphabet-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-alphabet-fn]
    pub fn get_alphabet(&self) -> StringSet {
        match self.type_ {
            ImplementationType::TROPICAL_OPENFST_TYPE => {
                TropicalWeightTransducer::get_alphabet(self.implementation.as_tropical())
            }
            ImplementationType::LOG_OPENFST_TYPE => {
                LogWeightTransducer::get_alphabet(self.implementation.as_log())
            }
            ImplementationType::ERROR_TYPE => crate::HFST_THROW!(TransducerHasWrongTypeException),
            ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => {
                crate::hfst_ol_transducer::HfstOlTransducer::get_alphabet(
                    self.implementation.as_hfst_ol(),
                )
            }
            _ => crate::HFST_THROW_MESSAGE!(FunctionNotImplementedException, "get_alphabet"),
        }
    }

    /*
      Only harmonize number-to-symbol-encodings.
      \a another is not modifed, but a modifed copy of it is returned.
    */
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.harmonize-symbol-encodings-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.harmonize-symbol-encodings-fn]
    pub fn harmonize_symbol_encodings(&mut self, another: &HfstTransducer) -> HfstTransducer {
        let another_basic = HfstBasicTransducer::from_hfst_transducer(another);
        let this_basic = HfstBasicTransducer::from_hfst_transducer(&*self);
        *self = HfstTransducer::from_basic_transducer(&this_basic, self.get_type());
        HfstTransducer::from_basic_transducer(&another_basic, another.get_type())
    }

    /*
       Harmonize this transducer with a copy of another.
       another is not modifed, but a modified copy of it is returned.
       Flag diacritics from the alphabet of this transducer are inserted
       to the alphabet of the copy of another, so that they are excluded
       from harmonization.
       If foma is used as implementation type, no harmonization is carried out,
       as foma's functions take care of harmonization. Then NULL is returned.
    */
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.harmonize-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.harmonize-fn]
    #[allow(unreachable_code)]
    pub fn harmonize_(&mut self, another: &HfstTransducer) -> Option<HfstTransducer> {
        if self.type_ != another.type_ {
            crate::HFST_THROW!(TransducerTypeMismatchException);
        }

        if self.anonymous && another.anonymous {
            crate::HFST_THROW_MESSAGE!(HfstFatalException, "harmonize_ with anonymous transducers");
        }

        let mut another_copy = another.clone();

        // Prevent flag diacritics from being harmonized by inserting them to
        // the alphabet. FIX?: remove them at the end?
        if self.get_type() == ImplementationType::FOMA_TYPE {
            let this_alphabet = self.get_alphabet();
            let another_alphabet = another_copy.get_alphabet();
            let mut add_to_this = StringSet::new();
            let mut add_to_another = StringSet::new();

            for it in another_alphabet.iter() {
                if FdOperation::is_diacritic(it) && !this_alphabet.contains(it) {
                    add_to_this.insert(it.clone());
                }
            }

            self.insert_to_alphabet_string_set(&add_to_this);

            for it in this_alphabet.iter() {
                if FdOperation::is_diacritic(it) && !another_alphabet.contains(it) {
                    add_to_another.insert(it.clone());
                }
            }
            another_copy.insert_to_alphabet_string_set(&add_to_another);
        }

        match self.type_ {
            ImplementationType::SFST_TYPE
            | ImplementationType::TROPICAL_OPENFST_TYPE
            | ImplementationType::LOG_OPENFST_TYPE => {
                let mut another_basic = another_copy.get_basic_transducer();
                let mut this_basic = self.convert_to_basic_transducer();

                this_basic.harmonize(&mut another_basic);

                // The two graphs carry independent symbol codings; reindex both
                // onto one shared coder so that, after each is converted back to an
                // OpenFst transducer, identical symbols carry identical labels (the
                // per-graph-coder replacement for the former process-global
                // numbering on which the subsequent binary op relies). Intern every
                // symbol of BOTH graphs (coder + full alphabet) into the shared
                // coder FIRST, so even alphabet-only symbols agree before either
                // graph adopts the coding.
                let mut canonical =
                    crate::hfst_tropical_transducer_transition_data::SymbolCoder::new();
                this_basic.intern_into(&mut canonical);
                another_basic.intern_into(&mut canonical);
                this_basic.reindex_into(&mut canonical);
                another_basic.reindex_into(&mut canonical);

                self.convert_to_hfst_transducer(this_basic);
                let another_harmonized =
                    HfstTransducer::from_basic_transducer(&another_basic, self.type_);

                return Some(another_harmonized);
            }
            ImplementationType::ERROR_TYPE => crate::HFST_THROW!(TransducerHasWrongTypeException),
            _ => crate::HFST_THROW!(TransducerHasWrongTypeException),
        }
        None // make compiler happy
    }

    /*  Harmonize symbol-to-number encodings and expand unknown and
    identity symbols.

    In the case of foma transducers, does nothing because foma's own functions
    take care of harmonizing. If harmonization is needed,
    FomaTransducer::harmonize can be used instead. */
    pub fn harmonize(&mut self, another: &mut HfstTransducer, force: bool) {
        if self.type_ != another.type_ {
            crate::HFST_THROW!(TransducerTypeMismatchException);
        }

        if self.anonymous && another.anonymous {
            return;
        }

        // Prevent flag diacritics from being harmonized by inserting them to
        // the alphabet.
        let this_alphabet = self.get_alphabet();
        let another_alphabet = another.get_alphabet();

        for it in another_alphabet.iter() {
            if FdOperation::is_diacritic(it) && !this_alphabet.contains(it) {
                self.insert_to_alphabet_string(it);
            }
        }

        for it in this_alphabet.iter() {
            if FdOperation::is_diacritic(it) && !another_alphabet.contains(it) {
                another.insert_to_alphabet_string(it);
            }
        }

        let _ = force;

        match self.type_ {
            ImplementationType::SFST_TYPE
            | ImplementationType::TROPICAL_OPENFST_TYPE
            | ImplementationType::LOG_OPENFST_TYPE => {
                let mut this_basic = self.convert_to_basic_transducer();
                let mut another_basic = another.convert_to_basic_transducer();

                this_basic.harmonize(&mut another_basic);

                // Reindex both graphs onto one shared symbol coding so that, after
                // each is converted back to an OpenFst transducer, identical symbols
                // carry identical labels for the subsequent binary op (the
                // per-graph-coder replacement for the former process-global numbering).
                // Intern both graphs' symbols (coder + alphabet) into the shared
                // coder first so alphabet-only symbols agree too.
                let mut canonical =
                    crate::hfst_tropical_transducer_transition_data::SymbolCoder::new();
                this_basic.intern_into(&mut canonical);
                another_basic.intern_into(&mut canonical);
                this_basic.reindex_into(&mut canonical);
                another_basic.reindex_into(&mut canonical);

                self.convert_to_hfst_transducer(this_basic);
                another.convert_to_hfst_transducer(another_basic);
            }
            ImplementationType::ERROR_TYPE => crate::HFST_THROW!(TransducerHasWrongTypeException),
            _ => crate::HFST_THROW!(TransducerHasWrongTypeException),
        }
    }

    // test function
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.print-alphabet-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.print-alphabet-fn]
    pub fn print_alphabet(&self) {
        if self.type_ == ImplementationType::TROPICAL_OPENFST_TYPE {
            TropicalWeightTransducer::print_alphabet(self.implementation.as_tropical());
        }
    }
}

// ===== lookup_predicates (workflow body) =====
// ===== lookup_predicates (flattened body) =====
use crate::hfst_data_types::HfstOneLevelPaths;
use crate::hfst_data_types::HfstTwoLevelPaths;

impl HfstTransducer {
    pub fn lookup_string_vector(
        &self,
        s: &StringVector,
        limit: isize,
        time_cutoff: f64,
    ) -> HfstOneLevelPaths {
        self.lookup_fd_string_vector(s, limit, time_cutoff)
    }

    pub fn lookup_string(&self, s: &str, limit: isize, time_cutoff: f64) -> HfstOneLevelPaths {
        self.lookup_fd_string(s, limit, time_cutoff)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.lookup-pairs-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.lookup-pairs-fn]
    pub fn lookup_pairs(&self, s: &str, limit: isize, time_cutoff: f64) -> HfstTwoLevelPaths {
        match self.type_ {
            ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => unsafe {
                (*self.implementation.as_hfst_ol_ptr()).lookup_fd_pairs_str(s, limit, time_cutoff)
            },
            _ => crate::HFST_THROW!(FunctionNotImplementedException),
        }
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.lookup-fd-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.lookup-fd-fn]
    pub fn lookup_fd_string_vector(
        &self,
        s: &StringVector,
        limit: isize,
        time_cutoff: f64,
    ) -> HfstOneLevelPaths {
        match self.type_ {
            ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => unsafe {
                (*self.implementation.as_hfst_ol_ptr()).lookup_fd_strvec(s, limit, time_cutoff)
            },
            ImplementationType::ERROR_TYPE => crate::HFST_THROW!(TransducerHasWrongTypeException),
            _ => crate::HFST_THROW!(FunctionNotImplementedException),
        }
    }

    pub fn lookup_fd_string(&self, s: &str, limit: isize, time_cutoff: f64) -> HfstOneLevelPaths {
        match self.type_ {
            ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => unsafe {
                (*self.implementation.as_hfst_ol_ptr()).lookup_fd_str(s, limit, time_cutoff)
            },
            ImplementationType::ERROR_TYPE => crate::HFST_THROW!(TransducerHasWrongTypeException),
            _ => crate::HFST_THROW!(FunctionNotImplementedException),
        }
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.lookup-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.lookup-fn]
    pub fn lookup_tokenizer(
        &self,
        tok: &HfstTokenizer,
        s: &str,
        limit: isize,
        time_cutoff: f64,
    ) -> HfstOneLevelPaths {
        let sv: StringVector = tok.tokenize_one_level(s, false);
        self.lookup_string_vector(&sv, limit, time_cutoff)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.lookdown-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.lookdown-fn]
    pub fn lookdown_string_vector(&self, s: &StringVector, limit: isize) -> HfstOneLevelPaths {
        let _ = s;
        let _ = limit;
        crate::HFST_THROW!(FunctionNotImplementedException)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.lookdown-fd-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.lookdown-fd-fn]
    pub fn lookdown_fd_string_vector(
        &self,
        s: &mut StringVector,
        limit: isize,
    ) -> HfstOneLevelPaths {
        let _ = s;
        let _ = limit;
        crate::HFST_THROW!(FunctionNotImplementedException)
    }

    pub fn lookdown_string(&self, s: &str, limit: isize) -> HfstOneLevelPaths {
        let _ = s;
        let _ = limit;
        crate::HFST_THROW!(FunctionNotImplementedException)
    }

    pub fn lookdown_fd_string(&self, s: &str, limit: isize) -> HfstOneLevelPaths {
        let _ = s;
        let _ = limit;
        crate::HFST_THROW!(FunctionNotImplementedException)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-lookup-infinitely-ambiguous-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-lookup-infinitely-ambiguous-fn]
    pub fn is_lookup_infinitely_ambiguous_string_vector(&self, s: &StringVector) -> bool {
        match self.type_ {
            /* TODO: Convert into HFST_OL(W)_TYPE, if needed. */
            ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => unsafe {
                (*self.implementation.as_hfst_ol_ptr()).is_lookup_infinitely_ambiguous_strvec(s)
            },
            _ => {
                let _ = s;
                crate::HFST_THROW!(FunctionNotImplementedException)
            }
        }
    }

    pub fn is_lookup_infinitely_ambiguous_string(&self, s: &str) -> bool {
        match self.type_ {
            /* TODO: Convert into HFST_OL(W)_TYPE, if needed. */
            ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => unsafe {
                (*self.implementation.as_hfst_ol_ptr()).is_lookup_infinitely_ambiguous_str(s)
            },
            _ => {
                let _ = s;
                crate::HFST_THROW!(FunctionNotImplementedException)
            }
        }
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-lookdown-infinitely-ambiguous-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-lookdown-infinitely-ambiguous-fn]
    pub fn is_lookdown_infinitely_ambiguous(&self, s: &StringVector) -> bool {
        let _ = s;
        crate::HFST_THROW!(FunctionNotImplementedException)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-infinitely-ambiguous-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-infinitely-ambiguous-fn]
    pub fn is_infinitely_ambiguous(&self) -> bool {
        match self.type_ {
            ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => unsafe {
                (*self.implementation.as_hfst_ol_ptr()).is_infinitely_ambiguous()
            },
            ImplementationType::ERROR_TYPE => crate::HFST_THROW!(TransducerHasWrongTypeException),
            _ => {
                // hfst::implementations::HfstBasicTransducer net(*this);
                // return net.is_infinitely_ambiguous();
                let net = self.get_basic_transducer();
                net.is_infinitely_ambiguous()
            }
        }
    }
}

// ===== queries_unary_ops (workflow body) =====
// ===== queries_unary_ops (flattened body) =====
// -----------------------------------------------------------------------
//
//              compare, queries, epsilon removal, determinization,
//              minimization, repeats and unary operators
//              (HfstTransducer.cc ~1681-2663)
//
// -----------------------------------------------------------------------

impl HfstTransducer {
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.compare-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.compare-fn]
    pub fn compare(&self, another: &HfstTransducer, harmonize: bool) -> bool {
        if self.type_ != another.type_ {
            std::panic::panic_any(
                crate::hfst_exception_defs::TransducerTypeMismatchException::new(
                    "TransducerTypeMismatchException".to_string(),
                    file!().to_string(),
                    line!() as usize,
                ),
            );
        }

        let mut one_copy = HfstTransducer::new_from(self);
        let mut another_copy = HfstTransducer::new_from(another);

        /* prevent harmonization, if needed */
        if !harmonize {
            one_copy.insert_missing_symbols_to_alphabet_from(&another_copy, false);
            another_copy.insert_missing_symbols_to_alphabet_from(&one_copy, false);
        }
        /* always prevent harmonizing special symbols */
        one_copy.insert_missing_symbols_to_alphabet_from(&another_copy, true);
        another_copy.insert_missing_symbols_to_alphabet_from(&one_copy, true);

        if self.type_ != ImplementationType::FOMA_TYPE
            && self.type_ != ImplementationType::XFSM_TYPE
        {
            another_copy = one_copy.harmonize_(&another_copy).unwrap();
        }

        one_copy.determinize();
        another_copy.determinize();

        match one_copy.type_ {
            ImplementationType::TROPICAL_OPENFST_TYPE => {
                crate::tropical_weight_transducer::TropicalWeightTransducer::are_equivalent(
                    one_copy.implementation.as_tropical(),
                    another_copy.implementation.as_tropical(),
                    // No caller configures equivalence-checking, so the former global
                    // 'encode_weights' is read at its C++ default (false) here.
                    false,
                )
            }
            ImplementationType::LOG_OPENFST_TYPE => {
                crate::log_weight_transducer::LogWeightTransducer::are_equivalent(
                    one_copy.implementation.as_log(),
                    another_copy.implementation.as_log(),
                )
            }
            ImplementationType::ERROR_TYPE => std::panic::panic_any(
                crate::hfst_exception_defs::TransducerHasWrongTypeException::new(
                    "TransducerHasWrongTypeException".to_string(),
                    file!().to_string(),
                    line!() as usize,
                ),
            ),
            _ => std::panic::panic_any(
                crate::hfst_exception_defs::FunctionNotImplementedException::new(
                    "FunctionNotImplementedException".to_string(),
                    file!().to_string(),
                    line!() as usize,
                ),
            ),
        }
    }

    pub fn compare_default(&self, another: &HfstTransducer) -> bool {
        self.compare(another, true)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-automaton-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-automaton-fn]
    pub fn is_automaton(&self) -> bool {
        match self.type_ {
            ImplementationType::TROPICAL_OPENFST_TYPE => {
                crate::tropical_weight_transducer::TropicalWeightTransducer::is_automaton(
                    self.implementation.as_tropical(),
                )
            }
            ImplementationType::LOG_OPENFST_TYPE => {
                crate::log_weight_transducer::LogWeightTransducer::is_automaton(
                    self.implementation.as_log(),
                )
            }
            ImplementationType::ERROR_TYPE => std::panic::panic_any(
                crate::hfst_exception_defs::TransducerHasWrongTypeException::new(
                    "TransducerHasWrongTypeException".to_string(),
                    file!().to_string(),
                    line!() as usize,
                ),
            ),
            _ => std::panic::panic_any(
                crate::hfst_exception_defs::FunctionNotImplementedException::new(
                    "FunctionNotImplementedException".to_string(),
                    file!().to_string(),
                    line!() as usize,
                ),
            ),
        }
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-cyclic-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-cyclic-fn]
    pub fn is_cyclic(&self) -> bool {
        match self.type_ {
            ImplementationType::TROPICAL_OPENFST_TYPE => {
                crate::tropical_weight_transducer::TropicalWeightTransducer::is_cyclic(
                    self.implementation.as_tropical(),
                )
            }
            ImplementationType::LOG_OPENFST_TYPE => {
                crate::log_weight_transducer::LogWeightTransducer::is_cyclic(
                    self.implementation.as_log(),
                )
            }
            ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => {
                crate::hfst_ol_transducer::HfstOlTransducer::is_cyclic(
                    self.implementation.as_hfst_ol(),
                )
            }
            ImplementationType::ERROR_TYPE => std::panic::panic_any(
                crate::hfst_exception_defs::TransducerHasWrongTypeException::new(
                    "TransducerHasWrongTypeException".to_string(),
                    file!().to_string(),
                    line!() as usize,
                ),
            ),
            _ => std::panic::panic_any(
                crate::hfst_exception_defs::FunctionNotImplementedException::new(
                    "FunctionNotImplementedException".to_string(),
                    file!().to_string(),
                    line!() as usize,
                ),
            ),
        }
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.number-of-states-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.number-of-states-fn]
    pub fn number_of_states(&self) -> u32 {
        if self.type_ == ImplementationType::TROPICAL_OPENFST_TYPE {
            return {
                crate::tropical_weight_transducer::TropicalWeightTransducer::number_of_states(
                    self.implementation.as_tropical(),
                )
            };
        }
        0
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.number-of-arcs-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.number-of-arcs-fn]
    pub fn number_of_arcs(&self) -> u32 {
        if self.type_ == ImplementationType::TROPICAL_OPENFST_TYPE {
            return {
                crate::tropical_weight_transducer::TropicalWeightTransducer::number_of_arcs(
                    self.implementation.as_tropical(),
                )
            };
        }
        0
    }

    // -----------------------------------------------------------------------
    //
    //              Epsilon removal, determinization, minimization
    //
    // -----------------------------------------------------------------------

    pub fn eliminate_flags(&mut self) -> &mut HfstTransducer {
        let basic = crate::hfst_basic_transducer::HfstBasicTransducer::new_from_transducer(self);
        let flags = basic.get_flags();
        let filter = get_flag_filter(self, &flags, "");

        if let Some(filter) = filter {
            let mut filter_copy = HfstTransducer::new_from(&filter);
            {
                let self_copy = HfstTransducer::new_from(self);
                let filter_deref = HfstTransducer::new_from(&filter);
                filter_copy
                    .compose(&self_copy, true)
                    .compose(&filter_deref, true);
            }
            flag_purge(&mut filter_copy, "");
            *self = filter_copy;
        } else {
            flag_purge(self, "");
        }

        self.optimize()
    }

    pub fn eliminate_flag(&mut self, flag: &str) -> &mut HfstTransducer {
        let basic = crate::hfst_basic_transducer::HfstBasicTransducer::new_from_transducer(self);
        let flags = basic.get_flags();
        let mut feature_found = false;
        for it in flags.iter() {
            if crate::hfst_flag_diacritics::FdOperation::get_feature(it) == flag {
                feature_found = true;
                break;
            }
        }
        if !feature_found {
            if !flag.contains('.') {
                std::panic::panic_any(crate::hfst_exception_defs::HfstException::new(
                    format!(
                        "HfstTransducer::eliminate_flag: flag feature does not occur in the transducer: {}",
                        flag
                    ),
                    file!().to_string(),
                    line!() as usize,
                ));
            } else {
                std::panic::panic_any(crate::hfst_exception_defs::HfstException::new(
                    format!(
                        "HfstTransducer::eliminate_flag: only the flag feature must be given, no value or operator: {}",
                        flag
                    ),
                    file!().to_string(),
                    line!() as usize,
                ));
            }
        }

        let filter = get_flag_filter(self, &flags, flag);
        if let Some(filter) = filter {
            let mut filter_copy = HfstTransducer::new_from(&filter);
            {
                let self_copy = HfstTransducer::new_from(self);
                let filter_deref = HfstTransducer::new_from(&filter);
                filter_copy
                    .compose(&self_copy, true)
                    .compose(&filter_deref, true);
            }
            flag_purge(&mut filter_copy, flag);
            *self = filter_copy;
        } else {
            flag_purge(self, flag);
        }

        self.optimize()
    }

    pub fn remove_epsilons(&mut self) -> &mut HfstTransducer {
        self.is_trie = false;
        self.apply(
            |t: &StdVectorFst| -> StdVectorFst {
                crate::tropical_weight_transducer::TropicalWeightTransducer::remove_epsilons(t)
            },
            |t: &crate::log_weight_transducer::LogFst| -> crate::log_weight_transducer::LogFst {
                crate::log_weight_transducer::LogWeightTransducer::remove_epsilons(t)
            },
            false,
        )
    }

    pub fn prune(&mut self) -> &mut HfstTransducer {
        // slow for xfsm type...
        self.convert(ImplementationType::TROPICAL_OPENFST_TYPE, "".to_string());
        let temp = crate::tropical_weight_transducer::TropicalWeightTransducer::prune(
            self.implementation.as_tropical(),
        );
        self.implementation = TransducerImplementation::Tropical(Box::new(temp));
        self
    }

    pub fn determinize(&mut self) -> &mut HfstTransducer {
        self.determinize_with_config(&EngineConfig::default())
    }

    /// 'determinize', reading 'encode_weights' (the only engine-policy flag this op
    /// consults) from the supplied config. The tropical backend encodes weights iff
    /// 'config.encode_weights'; the log backend never did, so it ignores it.
    pub fn determinize_with_config(&mut self, config: &EngineConfig) -> &mut HfstTransducer {
        self.is_trie = false;
        self.apply_bool(
            |t: &StdVectorFst, ew: bool| -> StdVectorFst {
                crate::tropical_weight_transducer::TropicalWeightTransducer::determinize(t, ew)
            },
            |t: &crate::log_weight_transducer::LogFst,
             _ew: bool|
             -> crate::log_weight_transducer::LogFst {
                crate::log_weight_transducer::LogWeightTransducer::determinize(t)
            },
            config.encode_weights,
        )
    }

    pub fn minimize(&mut self) -> &mut HfstTransducer {
        self.minimize_with_config(&EngineConfig::default())
    }

    /// 'minimize', reading 'encode_weights' from the supplied config (see
    /// 'determinize_with_config').
    pub fn minimize_with_config(&mut self, config: &EngineConfig) -> &mut HfstTransducer {
        self.is_trie = false;
        self.apply_bool(
            |t: &StdVectorFst, ew: bool| -> StdVectorFst {
                crate::tropical_weight_transducer::TropicalWeightTransducer::minimize(t, ew)
            },
            |t: &crate::log_weight_transducer::LogFst,
             _ew: bool|
             -> crate::log_weight_transducer::LogFst {
                crate::log_weight_transducer::LogWeightTransducer::minimize(t)
            },
            config.encode_weights,
        )
    }

    pub fn optimize(&mut self) -> &mut HfstTransducer {
        self.optimize_with_config(&EngineConfig::default())
    }

    pub fn optimize_with_config(&mut self, config: &EngineConfig) -> &mut HfstTransducer {
        if config.minimization {
            self.minimize_with_config(config)
        } else {
            self.determinize_with_config(config)
        }
    }

    // -----------------------------------------------------------------------
    //
    //                        Repeat functions
    //
    // -----------------------------------------------------------------------

    pub fn repeat_star(&mut self) -> &mut HfstTransducer {
        self.is_trie = false;
        self.apply(
            |t: &StdVectorFst| -> StdVectorFst {
                crate::tropical_weight_transducer::TropicalWeightTransducer::repeat_star(t)
            },
            |t: &crate::log_weight_transducer::LogFst| -> crate::log_weight_transducer::LogFst {
                crate::log_weight_transducer::LogWeightTransducer::repeat_star(t)
            },
            false,
        )
    }

    pub fn repeat_plus(&mut self) -> &mut HfstTransducer {
        self.is_trie = false;
        self.apply(
            |t: &StdVectorFst| -> StdVectorFst {
                crate::tropical_weight_transducer::TropicalWeightTransducer::repeat_plus(t)
            },
            |t: &crate::log_weight_transducer::LogFst| -> crate::log_weight_transducer::LogFst {
                crate::log_weight_transducer::LogWeightTransducer::repeat_plus(t)
            },
            false,
        )
    }

    pub fn repeat_n(&mut self, n: u32) -> &mut HfstTransducer {
        self.is_trie = false; // This could be done so that is_trie is preserved
        self.apply_n(
            |t: &StdVectorFst, n: u32| -> StdVectorFst {
                crate::tropical_weight_transducer::TropicalWeightTransducer::repeat_n(t, n)
            },
            |t: &crate::log_weight_transducer::LogFst,
             n: u32|
             -> crate::log_weight_transducer::LogFst {
                crate::log_weight_transducer::LogWeightTransducer::repeat_n(t, n)
            },
            n,
        )
    }

    pub fn repeat_n_plus(&mut self, n: u32) -> &mut HfstTransducer {
        self.is_trie = false; // This could be done so that is_trie is preserved
        let mut a = HfstTransducer::new_from(self);
        let b = HfstTransducer::new_from(a.repeat_star());
        self.repeat_n(n).concatenate(&b, true)
    }

    pub fn repeat_n_minus(&mut self, n: u32) -> &mut HfstTransducer {
        self.is_trie = false; // This could be done so that is_trie is preserved
        self.apply_n(
            |t: &StdVectorFst, n: u32| -> StdVectorFst {
                crate::tropical_weight_transducer::TropicalWeightTransducer::repeat_le_n(t, n)
            },
            |t: &crate::log_weight_transducer::LogFst,
             n: u32|
             -> crate::log_weight_transducer::LogFst {
                crate::log_weight_transducer::LogWeightTransducer::repeat_le_n(t, n)
            },
            n,
        )
    }

    pub fn repeat_n_to_k(&mut self, n: u32, k: u32) -> &mut HfstTransducer {
        self.is_trie = false; // This could be done so that is_trie is preserved
        let mut a = HfstTransducer::new_from(self);
        let b = HfstTransducer::new_from(a.repeat_n_minus(k - n));
        self.repeat_n(n).concatenate(&b, true)
    }

    // -----------------------------------------------------------------------
    //
    //                      Unary operators
    //
    // -----------------------------------------------------------------------

    pub fn optionalize(&mut self) -> &mut HfstTransducer {
        self.is_trie = false; // This could be done so that is_trie is preserved
        self.apply(
            |t: &StdVectorFst| -> StdVectorFst {
                crate::tropical_weight_transducer::TropicalWeightTransducer::optionalize(t)
            },
            |t: &crate::log_weight_transducer::LogFst| -> crate::log_weight_transducer::LogFst {
                crate::log_weight_transducer::LogWeightTransducer::optionalize(t)
            },
            false,
        )
    }

    pub fn invert(&mut self) -> &mut HfstTransducer {
        self.is_trie = false; // This could be done so that is_trie is preserved
        self.apply(
            |t: &StdVectorFst| -> StdVectorFst {
                crate::tropical_weight_transducer::TropicalWeightTransducer::invert(t)
            },
            |t: &crate::log_weight_transducer::LogFst| -> crate::log_weight_transducer::LogFst {
                crate::log_weight_transducer::LogWeightTransducer::invert(t)
            },
            false,
        )
    }

    pub fn reverse(&mut self) -> &mut HfstTransducer {
        self.is_trie = false; // This could be done so that is_trie is preserved
        self.apply(
            |t: &StdVectorFst| -> StdVectorFst {
                crate::tropical_weight_transducer::TropicalWeightTransducer::reverse(t)
            },
            |t: &crate::log_weight_transducer::LogFst| -> crate::log_weight_transducer::LogFst {
                crate::log_weight_transducer::LogWeightTransducer::reverse(t)
            },
            false,
        )
    }

    pub fn input_project(&mut self) -> &mut HfstTransducer {
        self.is_trie = false; // This could be done so that is_trie is preserved
        self.apply(
            |t: &StdVectorFst| -> StdVectorFst {
                crate::tropical_weight_transducer::TropicalWeightTransducer::extract_input_language(
                    t,
                )
            },
            |t: &crate::log_weight_transducer::LogFst| -> crate::log_weight_transducer::LogFst {
                crate::log_weight_transducer::LogWeightTransducer::extract_input_language(t)
            },
            false,
        )
    }

    pub fn output_project(&mut self) -> &mut HfstTransducer {
        self.is_trie = false; // This could be done so that is_trie is preserved
        self.apply(
            |t: &StdVectorFst| -> StdVectorFst { crate::tropical_weight_transducer::TropicalWeightTransducer::extract_output_language(
                        t,
                    ) },
            |t: &crate::log_weight_transducer::LogFst| -> crate::log_weight_transducer::LogFst { crate::log_weight_transducer::LogWeightTransducer::extract_output_language(
                        t,
                    ) },
            false,
        )
    }

    pub fn negate(&mut self) -> &mut HfstTransducer {
        self.is_trie = false; // This could be done so that is_trie is preserved

        if !self.is_automaton() {
            std::panic::panic_any(
                crate::hfst_exception_defs::TransducerIsNotAutomatonException::new(
                    "TransducerIsNotAutomatonException".to_string(),
                    file!().to_string(),
                    line!() as usize,
                ),
            );
        }

        let mut idstar = HfstTransducer::new_from_symbol("@_IDENTITY_SYMBOL_@", self.type_);
        // diacritics will not be harmonized in subtract
        let flags = idstar.insert_missing_diacritics_to_alphabet_from(self);
        for flag in flags.iter() {
            let tr = HfstTransducer::new_from_symbol(flag, self.type_);
            idstar.disjunct(&tr, true);
        }
        idstar.repeat_star();
        idstar.minimize();
        idstar.subtract(self, true);
        *self = idstar;
        self
    }
}

// if (required): return ~[(?* FAIL_FLAGS) ~$SUCCEED_FLAGS SELF ?*]
// if (! required): return ~[?* FAIL_FLAGS ~$SUCCEED_FLAGS SELF ?*]
// [spec:hfst:def:hfst-transducer.hfst.new-filter-fn]
// [spec:hfst:sem:hfst-transducer.hfst.new-filter-fn]
fn new_filter(
    fail_flags: &HfstTransducer,
    succeed_flags: &HfstTransducer,
    self_: &HfstTransducer,
    required: bool,
) -> HfstTransducer {
    let type_ = fail_flags.get_type();
    let mut comp = crate::xre::XreCompiler::new(type_);
    comp.set_expand_definitions(true);
    comp.define_transducer("Fail", fail_flags);
    comp.define_transducer("Succeed", succeed_flags);
    comp.define_transducer("Self", self_);
    let mut result: HfstTransducer = if required {
        comp.compile("~[(?* Fail) ~$Succeed Self ?*]")
    } else {
        comp.compile("~[?* Fail ~$Succeed Self ?*]")
    }
    .unwrap();

    // Should the xre compiler do this?
    result.remove_from_alphabet("Fail");
    result.remove_from_alphabet("Succeed");
    result.remove_from_alphabet("Self");

    result
}

// Substitute each symbol '_@FLAG@' with '@FLAG@'
// [spec:hfst:def:hfst-transducer.hfst.substitute-escaped-flags-fn]
// [spec:hfst:sem:hfst-transducer.hfst.substitute-escaped-flags-fn]
fn substitute_escaped_flags(filter: &mut HfstTransducer) {
    let alpha = filter.get_alphabet();
    for it in alpha.iter() {
        if it.len() > 1 {
            let bytes = it.as_bytes();
            if bytes[0] == b'_' && bytes[1] == b'@' {
                let mut s = it.clone();
                s.remove(0);
                filter.substitute_symbol(it, &s, true, true);
            }
        }
    }
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
fn get_flag_filter(
    transducer: &HfstTransducer,
    flags: &crate::hfst_symbol_defs::StringSet,
    flag: &str,
) -> Option<HfstTransducer> {
    let type_ = transducer.get_type();
    let mut flag_found = false;
    let mut filter: Option<HfstTransducer> = None;

    for f in flags.iter() {
        let self_ = HfstTransducer::new_from_symbol(&format!("_{}", f), type_); // escape flags
        let mut succeed_flags = HfstTransducer::new_type(type_);
        let mut fail_flags = HfstTransducer::new_type(type_);

        let op = crate::hfst_flag_diacritics::FdOperation::get_operator(f).as_bytes()[0];
        if (flag.is_empty() || crate::hfst_flag_diacritics::FdOperation::get_feature(f) == flag)
            && (op == b'U' || op == b'R' || op == b'D')
        // Equal flag?
        {
            for flag2 in flags.iter() {
                let fstatus = is_valid_flag_combination(f, flag2);

                if fstatus == 1 {
                    fail_flags.disjunct(
                        &HfstTransducer::new_from_symbol(&format!("_{}", flag2), type_),
                        true,
                    );
                    flag_found = true;
                } else if fstatus == 2 {
                    succeed_flags.disjunct(
                        &HfstTransducer::new_from_symbol(&format!("_{}", flag2), type_),
                        true,
                    );
                    flag_found = true;
                } else {
                }
            }
        }

        if flag_found {
            let newfilter = new_filter(
                &fail_flags,
                &succeed_flags,
                &self_,
                crate::hfst_flag_diacritics::FdOperation::get_operator(f).as_bytes()[0] == b'R',
            );

            // intersect filter with newfilter
            match filter.as_mut() {
                None => filter = Some(newfilter),
                Some(filt) => {
                    filt.intersect(&newfilter, true);
                }
            }
        }
        flag_found = false;
    }

    if let Some(filt) = filter.as_mut() {
        substitute_escaped_flags(filt); // unescape the flags
        filt.optimize();
    }

    filter
}

// Replace arcs in \a transducer that use flag \a flag with epsilon arcs
// and remove \a flag from alphabet of \a transducer. If \a flag is the empty
// string, replace/remove all flags.
// [spec:hfst:def:hfst-transducer.hfst.flag-purge-fn]
// [spec:hfst:sem:hfst-transducer.hfst.flag-purge-fn]
fn flag_purge(transducer: &mut HfstTransducer, flag: &str) {
    let type_ = transducer.get_type();
    // slow for xfsm_transducer..
    let mut net =
        crate::hfst_basic_transducer::HfstBasicTransducer::new_from_transducer(transducer);
    net.flag_purge(flag);
    *transducer = HfstTransducer::new_from_basic(&net, type_);
}

// ===== extract_nbest (workflow body) =====
// ===== extract_nbest (flattened body) =====
use crate::hfst_data_types::HfstTwoLevelPath;
use crate::hfst_exception_defs::TransducerIsCyclicException;
use crate::hfst_extract_strings::ExtractStringsCb;
use crate::hfst_extract_strings::RetVal;

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
    fn operator_call(&mut self, path: &mut HfstTwoLevelPath, final_: bool) -> RetVal {
        if final_ {
            self.paths.insert(path.clone());
        }

        RetVal::new(
            (self.max_num < 1) || (self.paths.len() as i32) < self.max_num,
            true,
        )
    }
}

impl HfstTransducer {
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-path-transducers-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-path-transducers-fn]
    pub fn extract_path_transducers(&mut self) -> Vec<HfstTransducer> {
        if self.type_ != ImplementationType::SFST_TYPE {
            crate::HFST_THROW!(FunctionNotImplementedException);
        }

        let hfst_paths: Vec<HfstTransducer> = Vec::new();
        // #if HAVE_SFST block elided (SFST backend is compiled out).
        hfst_paths
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-paths-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-paths-fn]
    pub fn extract_paths_cb(&self, callback: &mut dyn ExtractStringsCb, cycles: i32) {
        match self.type_ {
            ImplementationType::LOG_OPENFST_TYPE => {
                LogWeightTransducer::extract_paths(
                    self.implementation.as_log(),
                    callback,
                    cycles,
                    None,
                    false,
                );
            }
            ImplementationType::TROPICAL_OPENFST_TYPE => {
                TropicalWeightTransducer::extract_paths(
                    self.implementation.as_tropical(),
                    callback,
                    cycles,
                    None,
                    false,
                );
            }
            /* Add here your implementation. */
            ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => {
                crate::hfst_ol_transducer::HfstOlTransducer::extract_paths(
                    self.implementation.as_hfst_ol(),
                    callback,
                    cycles,
                    std::ptr::null(),
                    false,
                );
            }
            ImplementationType::ERROR_TYPE => {
                crate::HFST_THROW!(TransducerHasWrongTypeException);
            }
            _ => {
                crate::HFST_THROW!(FunctionNotImplementedException);
            }
        }
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-paths-fd-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-paths-fd-fn]
    pub fn extract_paths_fd_cb(
        &self,
        callback: &mut dyn ExtractStringsCb,
        cycles: i32,
        filter_fd: bool,
    ) {
        match self.type_ {
            ImplementationType::LOG_OPENFST_TYPE => {
                let t_log_ofst =
                    LogWeightTransducer::get_flag_diacritics(self.implementation.as_log());
                LogWeightTransducer::extract_paths(
                    self.implementation.as_log(),
                    callback,
                    cycles,
                    Some(&t_log_ofst),
                    filter_fd,
                );
            }
            ImplementationType::TROPICAL_OPENFST_TYPE => {
                let t_tropical_ofst = TropicalWeightTransducer::get_flag_diacritics(
                    self.implementation.as_tropical(),
                );
                TropicalWeightTransducer::extract_paths(
                    self.implementation.as_tropical(),
                    callback,
                    cycles,
                    Some(&t_tropical_ofst),
                    filter_fd,
                );
            }
            /* Add here your implementation. */
            ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => {
                let t_hfst_ol = crate::hfst_ol_transducer::HfstOlTransducer::get_flag_diacritics(
                    self.implementation.as_hfst_ol(),
                );
                crate::hfst_ol_transducer::HfstOlTransducer::extract_paths(
                    self.implementation.as_hfst_ol(),
                    callback,
                    cycles,
                    t_hfst_ol as *const _,
                    filter_fd,
                );
                // don't delete t_hfst_ol, it's not a copy of the FdTable but the
                // real thing
            }
            ImplementationType::ERROR_TYPE => {
                crate::HFST_THROW!(TransducerHasWrongTypeException);
            }
            _ => {
                crate::HFST_THROW!(FunctionNotImplementedException);
            }
        }
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.longest-path-size-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.longest-path-size-fn]
    pub fn longest_path_size(&self, obey_flags: bool) -> i32 {
        if self.is_cyclic() {
            crate::HFST_THROW!(TransducerIsCyclicException);
        }

        if !obey_flags {
            let net = HfstBasicTransducer::new_from_transducer(self);
            return net.longest_path_size();
        }

        let mut results = HfstTwoLevelPaths::new();
        let paths_found = self.extract_longest_paths(&mut results, true /* obey flags */);
        if !paths_found {
            return -1;
        }
        // else, there is at least one path
        results.iter().next().unwrap().second.len() as i32
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-longest-paths-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-longest-paths-fn]
    pub fn extract_longest_paths(
        &self,
        results: &mut HfstTwoLevelPaths,
        obey_flags: bool, /*,show_flags: bool*/
    ) -> bool {
        if self.is_cyclic() {
            crate::HFST_THROW!(TransducerIsCyclicException);
        }

        let net = HfstBasicTransducer::new_from_transducer(self);
        let path_lengths = net.path_sizes();
        if path_lengths.len() == 0 {
            return false;
        }

        let flags = net.get_flags();

        // go through each length of accepted paths in descending order
        for path_length in path_lengths.iter().copied() {
            // create a transducer [ any any ... any any ] where the number of
            // transitions that accept any symbol (including flags) is equal to
            // current length of accepted paths
            let match_length = match_any_n_times(path_length, &flags);

            let mut xre = crate::xre::XreCompiler::new(self.get_type());
            let mut length_tr = xre.compile(match_length.as_str()).unwrap();

            // filter out the paths of current length and extract them
            length_tr.compose(self, true);
            length_tr.optimize();
            if obey_flags {
                length_tr.extract_paths_fd(results, -1, -1, true);
            } else {
                length_tr.extract_paths(results, -1, -1);
            }

            // if paths were found
            if results.len() > 0 {
                return true;
            }
        } // lengths of accepted paths gone through

        // no paths found
        false
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-shortest-paths-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-shortest-paths-fn]
    pub fn extract_shortest_paths(&self, results: &mut HfstTwoLevelPaths) {
        let mut t = HfstTransducer::new_from_transducer(self);
        t.convert(ImplementationType::TROPICAL_OPENFST_TYPE, String::new());
        t.n_best(1);
        t.extract_paths(results, -1, -1);
    }

    pub fn extract_paths(&self, results: &mut HfstTwoLevelPaths, max_num: i32, cycles: i32) {
        if self.is_cyclic() && max_num < 1 && cycles < 0 {
            crate::HFST_THROW_MESSAGE!(
                TransducerIsCyclicException,
                "HfstTransducer::extract_paths"
            );
        }

        let mut cb = ExtractStringsCb_::new(results, max_num);
        self.extract_paths_cb(&mut cb, cycles);
    }

    pub fn extract_paths_fd(
        &self,
        results: &mut HfstTwoLevelPaths,
        max_num: i32,
        cycles: i32,
        filter_fd: bool,
    ) {
        if self.is_cyclic() && max_num < 1 && cycles < 0 {
            crate::HFST_THROW_MESSAGE!(
                TransducerIsCyclicException,
                "HfstTransducer::extract_paths_fd"
            );
        }

        let mut cb = ExtractStringsCb_::new(results, max_num);
        self.extract_paths_fd_cb(&mut cb, cycles, filter_fd);
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-random-paths-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-random-paths-fn]
    pub fn extract_random_paths(&self, results: &mut HfstTwoLevelPaths, max_num: i32) {
        match self.type_ {
            ImplementationType::TROPICAL_OPENFST_TYPE => {
                TropicalWeightTransducer::extract_random_paths(
                    self.implementation.as_tropical(),
                    results,
                    max_num,
                );
            }
            ImplementationType::LOG_OPENFST_TYPE => {
                LogWeightTransducer::extract_random_paths(
                    self.implementation.as_log(),
                    results,
                    max_num,
                );
            }
            ImplementationType::SFST_TYPE => {
                let mut copy = HfstTransducer::new_from_transducer(self);
                copy.convert(ImplementationType::TROPICAL_OPENFST_TYPE, String::new());
                TropicalWeightTransducer::extract_random_paths(
                    copy.implementation.as_tropical(),
                    results,
                    max_num,
                );
            }
            ImplementationType::FOMA_TYPE => {
                let mut copy = HfstTransducer::new_from_transducer(self);
                copy.convert(ImplementationType::TROPICAL_OPENFST_TYPE, String::new());
                TropicalWeightTransducer::extract_random_paths(
                    copy.implementation.as_tropical(),
                    results,
                    max_num,
                );
            }
            /* Add here your implementation. */
            ImplementationType::ERROR_TYPE => {
                crate::HFST_THROW!(TransducerHasWrongTypeException);
            }
            _ => {
                crate::HFST_THROW!(FunctionNotImplementedException);
            }
        }
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.extract-random-paths-fd-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.extract-random-paths-fd-fn]
    pub fn extract_random_paths_fd(
        &self,
        results: &mut HfstTwoLevelPaths,
        max_num: i32,
        filter_fd: bool,
    ) {
        let mut copy = HfstTransducer::new_from_transducer(self);
        copy.convert(ImplementationType::TROPICAL_OPENFST_TYPE, String::new());
        TropicalWeightTransducer::extract_random_paths_fd(
            copy.implementation.as_tropical(),
            results,
            max_num,
            filter_fd,
        );
    }

    pub fn n_best(&mut self, n: u32) -> &mut HfstTransducer {
        if !HfstTransducer::is_implementation_type_available(
            ImplementationType::TROPICAL_OPENFST_TYPE,
        ) {
            let _ = n;
            std::panic::panic_any(ImplementationTypeNotAvailableException::new(
                "HfstTransducer::n_best implemented only for TROPICAL_OPENFST_TYPE".to_string(),
                file!().to_string(),
                line!() as usize,
                self.type_,
            ));
        }

        let original_type: ImplementationType = self.type_;
        if (original_type == ImplementationType::SFST_TYPE)
            || (original_type == ImplementationType::FOMA_TYPE)
        {
            self.convert(ImplementationType::TROPICAL_OPENFST_TYPE, String::new());
        }

        match self.type_ {
            ImplementationType::TROPICAL_OPENFST_TYPE => {
                let temp = TropicalWeightTransducer::n_best(
                    self.implementation.as_tropical(),
                    n as i32 as u32,
                );
                self.implementation = TransducerImplementation::Tropical(Box::new(temp));
            }
            ImplementationType::LOG_OPENFST_TYPE => {
                let temp =
                    LogWeightTransducer::n_best(self.implementation.as_log(), n as i32 as u32);
                self.implementation = TransducerImplementation::Log(Box::new(temp));
            }
            ImplementationType::ERROR_TYPE => {
                crate::HFST_THROW!(TransducerHasWrongTypeException);
            }
            _ => {
                crate::HFST_THROW!(FunctionNotImplementedException);
            }
        }
        self.convert(original_type, String::new());
        self
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
}

// ===== flags_substitute (workflow body) =====
// ===== flags_substitute (flattened body) =====
// -----------------------------------------------------------------------
//
//                Alphabet handling (missing diacritics / symbols)
//
// -----------------------------------------------------------------------

impl HfstTransducer {
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.insert-missing-diacritics-to-alphabet-from-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.insert-missing-diacritics-to-alphabet-from-fn]
    pub fn insert_missing_diacritics_to_alphabet_from(
        &mut self,
        another: &HfstTransducer,
    ) -> StringSet {
        let this_alphabet: StringSet = self.get_alphabet();
        let another_alphabet: StringSet = another.get_alphabet();
        let mut missing_flags: StringSet = StringSet::new();

        for it in another_alphabet.iter() {
            if this_alphabet.get(it).is_none() {
                if FdOperation::is_diacritic(it) {
                    missing_flags.insert(it.clone());
                }
            }
        }
        self.insert_to_alphabet_set(&missing_flags);
        missing_flags
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.insert-missing-symbols-to-alphabet-from-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.insert-missing-symbols-to-alphabet-from-fn]
    pub fn insert_missing_symbols_to_alphabet_from(
        &mut self,
        another: &HfstTransducer,
        only_special_symbols: bool,
    ) {
        let this_alphabet: StringSet = self.get_alphabet();
        let another_alphabet: StringSet = another.get_alphabet();
        let mut missing_symbols: StringSet = StringSet::new();

        for it in another_alphabet.iter() {
            if this_alphabet.get(it).is_none() {
                if !only_special_symbols {
                    missing_symbols.insert(it.clone());
                } else {
                    if HfstTransducer::is_special_symbol(it) {
                        missing_symbols.insert(it.clone());
                    }
                }
            }
        }
        self.insert_to_alphabet_set(&missing_symbols);
    }

    // -----------------------------------------------------------------------
    //
    //                        Flag diacritics
    //
    // -----------------------------------------------------------------------

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
        another: &HfstTransducer,
        missing_flags: &mut StringSet,
        return_on_first_miss: bool,
    ) -> bool {
        let mut retval = false;
        let this_alphabet: StringSet = self.get_alphabet();
        let another_alphabet: StringSet = another.get_alphabet();

        for it in another_alphabet.iter() {
            if FdOperation::is_diacritic(it) && (this_alphabet.get(it).is_none()) {
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
    pub fn insert_freely_missing_flags_from(&mut self, another: &HfstTransducer) {
        let mut missing_flags: StringSet = StringSet::new();
        if self.check_for_missing_flags_in_into(
            another,
            &mut missing_flags,
            false, /* do not return on first miss */
        ) {
            let mut basic: HfstBasicTransducer = HfstBasicTransducer::from_transducer(self);

            let mut s: u32 = 0;
            while s <= (basic.get_max_state() as u32) {
                for missing_flag in missing_flags.iter() {
                    let tr = HfstBasicTransition::new_symbols(
                        s,
                        missing_flag.clone(),
                        missing_flag.clone(),
                        0.0,
                        basic.coder_mut(),
                    );
                    basic.add_transition(s, &tr, true);
                }
                s += 1;
            }

            *self = HfstTransducer::new_from_basic(&basic, self.type_);
        }
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.has-flag-diacritics-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.has-flag-diacritics-fn]
    pub fn has_flag_diacritics(&self) -> bool {
        has_flags(self)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.twosided-flag-diacritics-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.twosided-flag-diacritics-fn]
    pub fn twosided_flag_diacritics(&mut self) {
        let basic_fst: HfstBasicTransducer = HfstBasicTransducer::from_transducer(self);
        let mut basic_fst_copy: HfstBasicTransducer = HfstBasicTransducer::new();
        let _ = basic_fst_copy.add_state(basic_fst.get_max_state());

        let mut s: HfstState = 0;

        for states in basic_fst.state_vector.iter() {
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

                    let mut in_: String = istr.clone();
                    let mut out: String = if istr_is_flag {
                        istr.clone()
                    } else {
                        crate::hfst_symbol_defs::internal_epsilon.to_string()
                    };

                    let tr = HfstBasicTransition::new_symbols(
                        new_state,
                        in_,
                        out,
                        0.0, /*?*/
                        basic_fst_copy.coder_mut(),
                    );
                    basic_fst_copy.add_transition(s, &tr, true);

                    in_ = if ostr_is_flag {
                        ostr.clone()
                    } else {
                        crate::hfst_symbol_defs::internal_epsilon.to_string()
                    };
                    out = ostr.clone();

                    let tr = HfstBasicTransition::new_symbols(
                        transition.get_target_state(),
                        in_,
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
                basic_fst_copy.set_final_weight(s, &basic_fst.get_final_weight(s));
            }

            s += 1;
        }
        *self = HfstTransducer::new_from_basic(&basic_fst_copy, self.get_type());
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.harmonize-flag-diacritics-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.harmonize-flag-diacritics-fn]
    pub fn harmonize_flag_diacritics(
        &mut self,
        another: &mut HfstTransducer,
        insert_renamed_flags: bool,
    ) {
        if self.type_ != another.type_ {
            crate::HFST_THROW!(TransducerTypeMismatchException);
        }

        let this_has_flag_diacritics = has_flags(self);
        let another_has_flag_diacritics = has_flags(another);

        if this_has_flag_diacritics && another_has_flag_diacritics {
            rename_flag_diacritics(self, "_1");
            rename_flag_diacritics(another, "_2");

            if insert_renamed_flags {
                self.insert_freely_missing_flags_from(another);
                another.insert_freely_missing_flags_from(self);
                self.remove_illegal_flag_paths();
            }
        } else if this_has_flag_diacritics && insert_renamed_flags {
            another.insert_freely_missing_flags_from(self);
        } else if another_has_flag_diacritics && insert_renamed_flags {
            self.insert_freely_missing_flags_from(another);
        }
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.check-for-missing-flags-in-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.check-for-missing-flags-in-fn]
    pub fn check_for_missing_flags_in(&self, another: &HfstTransducer) -> bool {
        let mut foo: StringSet = StringSet::new(); /* An obligatory argument that is not used. */
        self.check_for_missing_flags_in_into(
            another, &mut foo, true, /* return on first miss */
        )
    }

    // -----------------------------------------------------------------------
    //
    //                        Insert freely
    //
    // -----------------------------------------------------------------------

    pub fn insert_freely_pair(
        &mut self,
        symbol_pair: &StringPair,
        harmonize: bool,
    ) -> &mut HfstTransducer {
        HfstTokenizer::check_utf8_correctness(&symbol_pair.0);
        HfstTokenizer::check_utf8_correctness(&symbol_pair.1);

        if symbol_pair.0.is_empty() || symbol_pair.1.is_empty() {
            crate::HFST_THROW_MESSAGE!(EmptyStringException, "insert_freely(const StringPair&)");
        }

        let tr =
            HfstTransducer::new_from_symbol_pair(&symbol_pair.0, &symbol_pair.1, self.get_type());
        self.insert_freely(&tr, harmonize)
    }

    pub fn insert_freely(&mut self, tr: &HfstTransducer, harmonize: bool) -> &mut HfstTransducer {
        if self.type_ != tr.type_ {
            crate::HFST_THROW_MESSAGE!(
                TransducerTypeMismatchException,
                "HfstTransducer::insert_freely"
            );
        }

        // Segfaults in xfst command line tool...
        // (XFSM_TYPE branch is #if'd out: HAVE_XFSM is not defined.)

        /* In this function, this transducer must always be harmonized
        according to tr, not the other way round. */
        // foma or no harmonization -> use our own copy of tr.
        let tr_harmonized: HfstTransducer = if harmonize { self.harmonize_(tr) } else { None }
            .unwrap_or_else(|| HfstTransducer::new_copy(tr));

        match self.type_ {
            ImplementationType::TROPICAL_OPENFST_TYPE => {
                let mut net = ConversionFunctions::tropical_ofst_to_hfst_basic_transducer(
                    self.implementation.as_tropical(),
                    true,
                );
                let substituting_net = ConversionFunctions::tropical_ofst_to_hfst_basic_transducer(
                    tr_harmonized.implementation.as_tropical(),
                    true,
                );

                net.insert_freely_graph(&substituting_net);
                self.implementation = TransducerImplementation::Tropical(Box::new(
                    ConversionFunctions::hfst_basic_transducer_to_tropical_ofst(&net),
                ));
                return self;
            }
            ImplementationType::LOG_OPENFST_TYPE => {
                let mut net = ConversionFunctions::log_ofst_to_hfst_basic_transducer(
                    self.implementation.as_log(),
                    true,
                );
                let substituting_net = ConversionFunctions::log_ofst_to_hfst_basic_transducer(
                    tr_harmonized.implementation.as_log(),
                    true,
                );

                net.insert_freely_graph(&substituting_net);
                self.implementation = TransducerImplementation::Log(Box::new(
                    ConversionFunctions::hfst_basic_transducer_to_log_ofst(&net),
                ));
                return self;
            }
            /* Add here your implementation. */
            ImplementationType::ERROR_TYPE => {
                crate::HFST_THROW!(TransducerHasWrongTypeException);
            }
            _ => {
                crate::HFST_THROW!(FunctionNotImplementedException);
            }
        }
    }

    // -----------------------------------------------------------------------
    //
    //                        Substitution functions
    //
    // -----------------------------------------------------------------------

    pub fn substitute_with_func(
        &mut self,
        func: impl Fn(&StringPair, &mut StringPairSet) -> bool,
    ) -> &mut HfstTransducer {
        // (XFSM_TYPE branch is #if'd out: HAVE_XFSM is not defined.)
        let mut net = self.convert_to_basic_transducer();
        net.substitute_with_func(func);
        self.convert_to_hfst_transducer(net)
    }

    pub fn substitute_string(
        &mut self,
        old_symbol: &str,
        new_symbol: &str,
        input_side: bool,
        output_side: bool,
    ) -> &mut HfstTransducer {
        // (XFSM_TYPE branch is #if'd out: HAVE_XFSM is not defined.)
        // empty strings are not accepted
        if old_symbol.is_empty() || new_symbol.is_empty() {
            crate::HFST_THROW_MESSAGE!(
                EmptyStringException,
                "substitute(const std::string&, const std::string&, bool, bool)"
            );
        }

        // if there are implementations available, use them

        // (SFST branch is #if'd out: HAVE_SFST is not defined.)
        // do not use until substituted symbols are correctly erased from the
        // alphabet
        // (tropical fast path is dead code: 'if false && ...'.)
        if false
            && (self.type_ == ImplementationType::TROPICAL_OPENFST_TYPE
                && input_side
                && output_side)
        {
            {
                let tmp = TropicalWeightTransducer::substitute_symbol(
                    self.implementation.as_tropical(),
                    old_symbol.to_string(),
                    new_symbol.to_string(),
                );
                self.implementation = TransducerImplementation::Tropical(Box::new(tmp));
            }
            return self;
        }
        if self.type_ == ImplementationType::LOG_OPENFST_TYPE && input_side && output_side {
            {
                let tmp = LogWeightTransducer::substitute_symbol(
                    self.implementation.as_log(),
                    old_symbol.to_string(),
                    new_symbol.to_string(),
                );
                self.implementation = TransducerImplementation::Log(Box::new(tmp));
            }
            return self;
        }

        // use the default HfstBasicTransducer function
        let mut net = self.convert_to_basic_transducer();
        net.substitute_symbol(
            &old_symbol.to_string(),
            &new_symbol.to_string(),
            input_side,
            output_side,
        );
        self.convert_to_hfst_transducer(net)
    }

    pub fn substitute_pair_with_pair(
        &mut self,
        old_symbol_pair: &StringPair,
        new_symbol_pair: &StringPair,
    ) -> &mut HfstTransducer {
        // (XFSM_TYPE branch is #if'd out: HAVE_XFSM is not defined.)
        // empty strings are not accepted
        if old_symbol_pair.0.is_empty()
            || old_symbol_pair.1.is_empty()
            || new_symbol_pair.0.is_empty()
            || new_symbol_pair.1.is_empty()
        {
            crate::HFST_THROW_MESSAGE!(
                EmptyStringException,
                "substitute(const StringPair&, const StringPair&)"
            );
        }

        let mut net = self.convert_to_basic_transducer();
        net.substitute_symbol_pair(old_symbol_pair, new_symbol_pair);
        self.convert_to_hfst_transducer(net);
        self
    }

    pub fn substitute_pair_with_pair_set(
        &mut self,
        old_symbol_pair: &StringPair,
        new_symbol_pair_set: &StringPairSet,
    ) -> &mut HfstTransducer {
        // (XFSM_TYPE branch is #if'd out: HAVE_XFSM is not defined.)
        if old_symbol_pair.0.is_empty() || old_symbol_pair.1.is_empty() {
            crate::HFST_THROW_MESSAGE!(
                EmptyStringException,
                "substitute(const StringPair&, const StringPairSet&"
            );
        }

        let mut net = self.convert_to_basic_transducer();
        net.substitute_symbol_pair_with_set(old_symbol_pair, new_symbol_pair_set);
        self.convert_to_hfst_transducer(net)
    }

    pub fn substitute_symbol(
        &mut self,
        old_symbol: &str,
        new_symbol: &str,
        input_side: bool,
        output_side: bool,
    ) -> &mut HfstTransducer {
        self.substitute_string(old_symbol, new_symbol, input_side, output_side)
    }

    pub fn substitute_symbol_pair(
        &mut self,
        old_symbol_pair: &StringPair,
        new_symbol_pair: &StringPair,
    ) -> &mut HfstTransducer {
        self.substitute_pair_with_pair(old_symbol_pair, new_symbol_pair)
    }

    pub fn substitute_symbol_pair_with_set(
        &mut self,
        old_symbol_pair: &StringPair,
        new_symbol_pair_set: &StringPairSet,
    ) -> &mut HfstTransducer {
        self.substitute_pair_with_pair_set(old_symbol_pair, new_symbol_pair_set)
    }

    pub fn substitute_symbol_pair_with_transducer(
        &mut self,
        symbol_pair: &StringPair,
        transducer: &mut HfstTransducer,
        harmonize: bool,
    ) -> &mut HfstTransducer {
        self.substitute_pair_with_transducer(symbol_pair, transducer, harmonize)
    }

    pub fn substitute_symbols(
        &mut self,
        substitutions: &HfstSymbolSubstitutions,
    ) -> &mut HfstTransducer {
        self.substitute_symbol_substitutions(substitutions)
    }

    pub fn substitute_symbol_substitutions(
        &mut self,
        substitutions: &HfstSymbolSubstitutions,
    ) -> &mut HfstTransducer {
        // (XFSM_TYPE branch is #if'd out: HAVE_XFSM is not defined.)
        let mut net = self.convert_to_basic_transducer();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            net.substitute_symbols(substitutions);
        }));
        if let Err(e) = result {
            if e.downcast_ref::<FunctionNotImplementedException>()
                .is_some()
            {
                for substitution in substitutions.iter() {
                    net.substitute_symbol(substitution.0, substitution.1, true, true);
                }
            } else {
                std::panic::resume_unwind(e);
            }
        }

        self.convert_to_hfst_transducer(net)
    }

    pub fn substitute_symbol_pairs(
        &mut self,
        substitutions: &HfstSymbolPairSubstitutions,
    ) -> &mut HfstTransducer {
        self.substitute_symbol_pair_substitutions(substitutions)
    }

    pub fn substitute_symbol_pair_substitutions(
        &mut self,
        substitutions: &HfstSymbolPairSubstitutions,
    ) -> &mut HfstTransducer {
        // (XFSM_TYPE branch is #if'd out: HAVE_XFSM is not defined.)
        let mut net = self.convert_to_basic_transducer();
        net.substitute_symbol_pairs(substitutions);
        self.convert_to_hfst_transducer(net)
    }

    pub fn substitute_pair_with_transducer(
        &mut self,
        symbol_pair: &StringPair,
        transducer: &mut HfstTransducer,
        harmonize: bool,
    ) -> &mut HfstTransducer {
        // (XFSM_TYPE branch is #if'd out: HAVE_XFSM is not defined.)
        if self.type_ != transducer.type_ {
            crate::HFST_THROW_MESSAGE!(
                TransducerTypeMismatchException,
                "HfstTransducer::substitute"
            );
        }

        if symbol_pair.0.is_empty() || symbol_pair.1.is_empty() {
            crate::HFST_THROW_MESSAGE!(
                EmptyStringException,
                "substitute(const StringPair&, HfstTransducer&)"
            );
        }

        // (SFST conversion fast path is #if'd out: HAVE_SFST is not defined.)

        let mut pair_transducer =
            HfstTransducer::new_from_symbol_pair(&symbol_pair.0, &symbol_pair.1, self.type_);
        if !harmonize {
            self.insert_missing_symbols_to_alphabet_from(&pair_transducer, false);
            pair_transducer.insert_missing_symbols_to_alphabet_from(self, false);
        }
        self.insert_missing_symbols_to_alphabet_from(&pair_transducer, true);
        pair_transducer.insert_missing_symbols_to_alphabet_from(self, true);

        self.harmonize(&mut pair_transducer, false);

        if !harmonize {
            self.insert_missing_symbols_to_alphabet_from(transducer, false);
            transducer.insert_missing_symbols_to_alphabet_from(self, false);
        }
        self.insert_missing_symbols_to_alphabet_from(transducer, true);
        transducer.insert_missing_symbols_to_alphabet_from(self, true);

        self.harmonize(transducer, false);

        // (FOMA branch is #if'd out: HAVE_FOMA is not defined.)
        if self.type_ == ImplementationType::TROPICAL_OPENFST_TYPE {
            {
                let result = TropicalWeightTransducer::substitute_string_transducer(
                    self.implementation.as_tropical(),
                    symbol_pair.clone(),
                    transducer.implementation.as_tropical(),
                );
                self.implementation = TransducerImplementation::Tropical(Box::new(result));
            }
            return self;
        }
        if self.type_ == ImplementationType::LOG_OPENFST_TYPE {
            {
                let result = LogWeightTransducer::substitute_string_transducer(
                    self.implementation.as_log(),
                    symbol_pair.clone(),
                    transducer.implementation.as_log(),
                );
                self.implementation = TransducerImplementation::Log(Box::new(result));
            }
            return self;
        }
        if self.type_ == ImplementationType::ERROR_TYPE {
            crate::HFST_THROW!(TransducerHasWrongTypeException);
        }

        crate::HFST_THROW!(FunctionNotImplementedException)
    }

    // -----------------------------------------------------------------------
    //
    //                        Weight handling
    //
    // -----------------------------------------------------------------------

    pub fn set_final_weights(&mut self, weight: f32, increment: bool) -> &mut HfstTransducer {
        if self.type_ == ImplementationType::TROPICAL_OPENFST_TYPE {
            {
                self.implementation = TransducerImplementation::Tropical(Box::new(
                    TropicalWeightTransducer::set_final_weights(
                        self.implementation.as_tropical(),
                        weight,
                        increment,
                    ),
                ));
            }
            return self;
        }
        if self.type_ == ImplementationType::LOG_OPENFST_TYPE {
            {
                self.implementation = TransducerImplementation::Log(Box::new(
                    LogWeightTransducer::set_final_weights(self.implementation.as_log(), weight),
                ));
            }
            return self;
        }
        let _ = weight;
        self
    }
}

// ===== weights_binary_ops (workflow body) =====
// ===== weights_binary_ops (flattened body) =====
// ===========================================================================
// area: weights_binary_ops — 'HfstTransducer.cc' weight handling (push_labels,
// push_weights, transform_weights, has_weights) and the binary operators
// (merge, compose, remove_illegal_flag_paths, lenient_composition,
// cross_product, shuffle, priority_union, compose_intersect, concatenate,
// disjunct (both overloads), disjunct_as_tries, intersect, subtract), plus the
// file-scope free helpers they use.
//
// 1:1 port of 'libhfst/src/HfstTransducer.cc' lines ~4173-5423. The union is
// matched on 'self.type_' exactly as the C++ 'switch'/'if (this->type == ...)'.
// SFST/FOMA/XFSM dispatch arms are '#if''d out of the union, but the *guards*
// that reference those types are kept verbatim (they are dead in this build).
// ===========================================================================

use std::collections::BTreeSet;

use crate::hfst_data_types::PushType;
use crate::hfst_exception_defs::*;
use crate::hfst_symbol_defs::internal_epsilon;
use crate::hfst_symbol_defs::internal_identity;
use crate::hfst_symbol_defs::internal_unknown;
use crate::hfst_symbol_defs::is_epsilon;
use crate::hfst_symbol_defs::is_unknown;

// -----------------------------------------------------------------------
//
//                        Binary operators — free helpers
//
// -----------------------------------------------------------------------

// [spec:hfst:def:hfst-transducer.hfst.substitute-single-identity-with-the-other-symbol-fn]
// [spec:hfst:sem:hfst-transducer.hfst.substitute-single-identity-with-the-other-symbol-fn]
pub fn substitute_single_identity_with_the_other_symbol(
    sp: &StringPair,
    sps: &mut StringPairSet,
) -> bool {
    let mut isymbol: String = sp.0.clone();
    let mut osymbol: String = sp.1.clone();

    if isymbol == "@_IDENTITY_SYMBOL_@" && (osymbol != "@_IDENTITY_SYMBOL_@") {
        isymbol = String::from("@_UNKNOWN_SYMBOL_@");
        sps.insert((isymbol, osymbol));
        true
    } else if osymbol == "@_IDENTITY_SYMBOL_@" && (isymbol != "@_IDENTITY_SYMBOL_@") {
        osymbol = String::from("@_UNKNOWN_SYMBOL_@");
        sps.insert((isymbol, osymbol));
        true
    } else {
        false
    }
}

// [spec:hfst:def:hfst-transducer.hfst.substitute-unknown-identity-pairs-fn]
// [spec:hfst:sem:hfst-transducer.hfst.substitute-unknown-identity-pairs-fn]
pub fn substitute_unknown_identity_pairs(sp: &StringPair, sps: &mut StringPairSet) -> bool {
    let mut isymbol: String = sp.0.clone();
    let mut osymbol: String = sp.1.clone();

    if isymbol == "@_UNKNOWN_SYMBOL_@" && osymbol == "@_IDENTITY_SYMBOL_@" {
        isymbol = String::from("@_IDENTITY_SYMBOL_@");
        osymbol = String::from("@_IDENTITY_SYMBOL_@");
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
pub fn get_flag_path_restriction(
    _1_flags: &StringSet,
    _2_flags: &StringSet,
    type_: ImplementationType,
) -> HfstTransducer {
    // Two state fst with borh states final.
    let mut basic_restriction = HfstBasicTransducer::new();
    basic_restriction.add_state_new();
    let start_state: HfstState = 0;
    let seen_2_state: HfstState = 1;

    basic_restriction.set_final_weight(start_state, &0.0);
    basic_restriction.set_final_weight(seen_2_state, &0.0);

    let tr = HfstBasicTransition::new_symbols(
        start_state,
        internal_identity.to_string(),
        internal_identity.to_string(),
        0.0,
        basic_restriction.coder_mut(),
    );
    basic_restriction.add_transition(start_state, &tr, true);

    let tr = HfstBasicTransition::new_symbols(
        start_state,
        internal_identity.to_string(),
        internal_identity.to_string(),
        0.0,
        basic_restriction.coder_mut(),
    );
    basic_restriction.add_transition(seen_2_state, &tr, true);

    // All _1_flags are allowed as long as no _2_flags with no
    // intervening symbols were observed.
    for dollar_flag in _1_flags {
        let mut dollar_flag = dollar_flag.clone();
        unsafe {
            let b = dollar_flag.as_bytes_mut();
            let n = b.len();
            b[0] = b'$';
            b[n - 1] = b'$';
        }

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
        let mut dollar_flag = dollar_flag.clone();
        unsafe {
            let b = dollar_flag.as_bytes_mut();
            let n = b.len();
            b[0] = b'$';
            b[n - 1] = b'$';
        }

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

    let restriction = HfstTransducer::from_basic(&basic_restriction, type_);

    restriction
}

//
// -------------------- Shuffle functions --------------------
//

// Possible cases for function code_symbols_for_shuffle.
// [spec:hfst:def:hfst-transducer.hfst.shuffle-coding]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
            let symbol_escaped = format!("@1{}", sp.0);
            let new_sp: StringPair = (symbol_escaped.clone(), symbol_escaped);
            sps.insert(new_sp);
        }
        // substitute each symbol bar in the second argument transducer
        // with a symbol @2bar
        ShuffleCoding::ENCODE_SECOND_SHUFFLE_ARGUMENT => {
            let symbol_escaped = format!("@2{}", sp.0);
            let new_sp: StringPair = (symbol_escaped.clone(), symbol_escaped);
            sps.insert(new_sp);
        }
        // substitute each symbol @1foo or @2bar in the shuffled transducer
        // with the original foo or bar.
        ShuffleCoding::DECODE_AFTER_SHUFFLE => {
            let symbol_unescaped = sp.0[2..].to_string();
            let new_sp: StringPair = (symbol_unescaped.clone(), symbol_unescaped);
            sps.insert(new_sp);
        }
    }

    true
}

// -----------------------------------------------------------------------
//
//                        Weight handling + Binary operators
//
// -----------------------------------------------------------------------

#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
impl HfstTransducer {
    pub fn push_labels(&mut self, push_type: PushType) -> &mut HfstTransducer {
        let to_initial_state = push_type == PushType::TO_INITIAL_STATE;
        if self.type_ == ImplementationType::TROPICAL_OPENFST_TYPE {
            let tmp = TropicalWeightTransducer::push_labels(
                self.implementation.as_tropical(),
                to_initial_state,
            );
            self.implementation = TransducerImplementation::Tropical(Box::new(tmp));
            return self;
        }
        if self.type_ == ImplementationType::LOG_OPENFST_TYPE {
            let tmp =
                LogWeightTransducer::push_labels(self.implementation.as_log(), to_initial_state);
            self.implementation = TransducerImplementation::Log(Box::new(tmp));
            return self;
        }
        let _ = push_type;
        self
    }

    /// Realign a transducer by pushing its labels to the start on both sides:
    /// invert, push labels to the initial state, invert back, and push again.
    /// Lifted verbatim from hfst-realign (the boundary-symbol variant is dead /
    /// commented out in the C++; this is the only realignment it performs).
    pub fn realign(&mut self) -> &mut HfstTransducer {
        self.invert();
        self.push_labels(PushType::TO_INITIAL_STATE);
        self.invert();
        self.push_labels(PushType::TO_INITIAL_STATE)
    }

    pub fn push_weights(&mut self, push_type: PushType) -> &mut HfstTransducer {
        let to_initial_state = push_type == PushType::TO_INITIAL_STATE;
        if self.type_ == ImplementationType::TROPICAL_OPENFST_TYPE {
            let tmp = TropicalWeightTransducer::push_weights(
                self.implementation.as_tropical(),
                to_initial_state,
            );
            self.implementation = TransducerImplementation::Tropical(Box::new(tmp));
            return self;
        }
        if self.type_ == ImplementationType::LOG_OPENFST_TYPE {
            let tmp =
                LogWeightTransducer::push_weights(self.implementation.as_log(), to_initial_state);
            self.implementation = TransducerImplementation::Log(Box::new(tmp));
            return self;
        }
        let _ = push_type;
        self
    }

    pub fn transform_weights(&mut self, func: fn(f32) -> f32) -> &mut HfstTransducer {
        if self.type_ == ImplementationType::TROPICAL_OPENFST_TYPE {
            // NOTE: as in the C++ facade, the old transducer is NOT deleted here.
            self.implementation = TransducerImplementation::Tropical(Box::new(
                TropicalWeightTransducer::transform_weights(
                    self.implementation.as_tropical(),
                    func,
                ),
            ));
            return self;
        }
        if self.type_ == ImplementationType::LOG_OPENFST_TYPE {
            self.implementation = TransducerImplementation::Log(Box::new(
                LogWeightTransducer::transform_weights(self.implementation.as_log(), func),
            ));
            return self;
        }
        let _ = func;
        self
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.has-weights-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.has-weights-fn]
    pub fn has_weights(&self) -> bool {
        if self.type_ == ImplementationType::TROPICAL_OPENFST_TYPE {
            return TropicalWeightTransducer::has_weights(self.implementation.as_tropical());
        }
        if self.type_ == ImplementationType::LOG_OPENFST_TYPE {
            crate::HFST_THROW!(FunctionNotImplementedException);
        }
        false
    }

    pub fn merge(
        &mut self,
        another: &HfstTransducer,
        args: &crate::xre::XreConstructorArguments,
    ) -> &mut HfstTransducer {
        // #if HAVE_XFSM: if (this->type == XFSM_TYPE) throw FunctionNotImplemented
        let mut this_basic = HfstBasicTransducer::from_transducer(self);
        // [spec:hfst:def:hfst-transducer.hfst.another-basic-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.another-basic-fn]
        let mut another_basic = HfstBasicTransducer::from_transducer(another);
        let mut markers_added: BTreeSet<String> = BTreeSet::new();
        let result = HfstBasicTransducer::merge(
            &mut this_basic,
            &mut another_basic,
            &args.list_definitions,
            &mut markers_added,
        );
        let mut initial_merge = HfstTransducer::from_basic(&result, self.get_type());
        initial_merge.optimize();

        // filter non-optimal paths
        // [ ? | #V ?:? ]* %#V:V ?:0 [ ? | #V ?:? | %#V:V ?:0 ]*
        // [spec:hfst:def:hfst-transducer.hfst.xre-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.xre-fn]
        let mut xre_ = crate::xre::XreCompiler::new(args);
        xre_.set_verbosity(false);

        for it in &markers_added {
            let marker = it.clone();
            let symbol = (it.as_bytes()[1] as char).to_string(); // @X@ -> X
            let worsener_string = format!(
                "[ ? | \"{m}\" ?:? ]* \"{m}\":{s} ?:0 [ ? | \"{m}\" ?:? | \"{m}\":{s} ?:0 ]* ;",
                m = marker,
                s = symbol
            );

            let mut worsener = xre_.compile(&worsener_string).unwrap();
            worsener.optimize();
            // [spec:hfst:def:hfst-transducer.hfst.cp-fn]
            // [spec:hfst:sem:hfst-transducer.hfst.cp-fn]
            let mut cp = initial_merge.clone();
            cp.compose(&worsener, true).output_project().optimize();

            initial_merge.subtract(&cp, true).optimize();
            initial_merge.substitute_symbol(&marker, internal_epsilon, true, true);

            // [spec:hfst:def:hfst-transducer.hfst.fsm-fn]
            // [spec:hfst:sem:hfst-transducer.hfst.fsm-fn]
            let fsm = HfstBasicTransducer::from_transducer(&initial_merge);
            let symbols = fsm.symbols_used();
            if !symbols.contains(&symbol) {
                initial_merge.remove_from_alphabet(&symbol);
            }
        }

        *self = initial_merge;
        self
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
        substitutions: &HfstTransducer,
    ) -> &mut HfstTransducer {
        let mut subs = substitutions.clone();
        let mut sigma_minus_subs = HfstTransducer::new_symbol_pair(
            crate::hfst_symbol_defs::internal_identity,
            crate::hfst_symbol_defs::internal_identity,
            self.type_,
        );
        let mut subs_in = substitutions.clone();
        subs_in.input_project();
        sigma_minus_subs.subtract(&subs_in, true);
        subs.disjunct(&sigma_minus_subs, true);
        subs.repeat_star();
        // Compose on the right, minimise, then compose the inverse on the left
        // (C++: trans = substitution_trans->compose(trans)).
        self.compose(&subs, true);
        self.minimize();
        subs.invert();
        subs.compose(&*self, true);
        *self = subs;
        self.minimize();
        self
    }

    pub fn compose(&mut self, another: &HfstTransducer, harmonize: bool) -> &mut HfstTransducer {
        self.compose_with_config(another, harmonize, &EngineConfig::default())
    }

    /// 'compose', reading the engine-policy flags it consults
    /// ('flag_is_epsilon_in_composition', 'unknown_symbols_in_use',
    /// 'xerox_composition') from the supplied config.
    pub fn compose_with_config(
        &mut self,
        another: &HfstTransducer,
        harmonize: bool,
        config: &EngineConfig,
    ) -> &mut HfstTransducer {
        self.is_trie = false;

        if self.type_ != another.type_ {
            crate::HFST_THROW!(TransducerTypeMismatchException);
        }

        let mut another_copy: HfstTransducer = another.clone();

        /* If we want flag diacritcs to be handled in the same way as epsilons
        in composition, we substitute output flags of first transducer with
        epsilons and input flags of second transducer with epsilons. */
        if config.flag_is_epsilon_in_composition && self.type_ != ImplementationType::XFSM_TYPE {
            let __prev_hook = std::panic::take_hook();
            std::panic::set_hook(Box::new(|_| {}));
            let __res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.substitute_with_func(substitute_output_flag_with_epsilon);
                another_copy.substitute_with_func(substitute_input_flag_with_epsilon);
            }));
            std::panic::set_hook(__prev_hook);
            if __res.is_err() {
                crate::HFST_THROW!(FlagDiacriticsAreNotIdentitiesException);
            }
        }

        // Variables possibly needed next.
        let mut diacritics_added_from_another_to_this: StringSet = StringSet::new();
        let mut diacritics_added_from_this_to_another: StringSet = StringSet::new();

        if config.xerox_composition {
            if self.type_ != ImplementationType::XFSM_TYPE {
                encode_flag_diacritics(self);
                encode_flag_diacritics(&mut another_copy);
            }
        } else if self.type_ == ImplementationType::XFSM_TYPE {
            diacritics_added_from_another_to_this =
                self.insert_missing_diacritics_to_alphabet_from(&another_copy);
            diacritics_added_from_this_to_another =
                another_copy.insert_missing_diacritics_to_alphabet_from(self);
        }

        /* Prevent harmonization (i.e. matching unknown symbols), if requested. */
        if !harmonize {
            self.insert_missing_symbols_to_alphabet_from(&another_copy, false);
            another_copy.insert_missing_symbols_to_alphabet_from(self, false);
        }

        /* Special symbols are never harmonized. */
        self.insert_missing_symbols_to_alphabet_from(&another_copy, true);
        another_copy.insert_missing_symbols_to_alphabet_from(self, true);

        // Harmonize, FOMA and XFSM take care of this by default.
        if self.type_ != ImplementationType::FOMA_TYPE
            && self.type_ != ImplementationType::XFSM_TYPE
        {
            another_copy = self.harmonize_(&another_copy).unwrap();
        }

        /* Take care of unknown and identity symbols being handled right in
        composition, FOMA and XFSM take care of this by default. */
        if (self.type_ != ImplementationType::FOMA_TYPE
            && self.type_ != ImplementationType::XFSM_TYPE)
            && config.unknown_symbols_in_use
        {
            self.substitute_symbol("@_IDENTITY_SYMBOL_@", "@_UNKNOWN_SYMBOL_@", false, true);
            another_copy.substitute_symbol(
                "@_IDENTITY_SYMBOL_@",
                "@_UNKNOWN_SYMBOL_@",
                true,
                false,
            );
        }

        match self.type_ {
            ImplementationType::TROPICAL_OPENFST_TYPE => {
                let tropical_ofst_temp = TropicalWeightTransducer::compose(
                    self.implementation.as_tropical(),
                    another_copy.implementation.as_tropical(),
                );
                self.implementation =
                    TransducerImplementation::Tropical(Box::new(tropical_ofst_temp));
            }
            ImplementationType::LOG_OPENFST_TYPE => {
                let log_ofst_temp = LogWeightTransducer::compose(
                    self.implementation.as_log(),
                    another_copy.implementation.as_log(),
                );
                self.implementation = TransducerImplementation::Log(Box::new(log_ofst_temp));
            }
            ImplementationType::HFST_OL_TYPE | ImplementationType::HFST_OLW_TYPE => {
                // This is the exception the tool wants to hear
                crate::HFST_THROW!(HfstTransducerTypeMismatchException);
            }
            ImplementationType::ERROR_TYPE => {
                crate::HFST_THROW!(TransducerHasWrongTypeException);
            }
            _ => {
                crate::HFST_THROW!(FunctionNotImplementedException);
            }
        }

        // Revert changes made before composition
        if config.xerox_composition {
            if self.type_ != ImplementationType::XFSM_TYPE {
                decode_flag_diacritics(self);
                decode_flag_diacritics(&mut another_copy);
            }
        } else if self.type_ == ImplementationType::XFSM_TYPE {
            self.remove_symbols_from_alphabet(&diacritics_added_from_another_to_this);
            another_copy.remove_symbols_from_alphabet(&diacritics_added_from_this_to_another);
        }

        if config.flag_is_epsilon_in_composition && self.type_ != ImplementationType::XFSM_TYPE {
            self.substitute_with_func(substitute_one_sided_flags);
        }

        if (self.type_ != ImplementationType::FOMA_TYPE
            && self.type_ != ImplementationType::XFSM_TYPE)
            && config.unknown_symbols_in_use
        {
            self.substitute_with_func(substitute_single_identity_with_the_other_symbol);
            another_copy.substitute_with_func(substitute_unknown_identity_pairs);
        }

        self
    }

    pub(crate) fn remove_illegal_flag_paths(&mut self) -> &mut HfstTransducer {
        let alphabet = self.get_alphabet();
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
            return self;
        }

        // Rename @...@ flags to $...$ flags and compile restriction.
        let mut subst: HfstSymbolSubstitutions = HfstSymbolSubstitutions::new();
        let mut back_subst: HfstSymbolSubstitutions = HfstSymbolSubstitutions::new();

        for _1_flag in &_1_flags {
            let at_flag = _1_flag.clone();
            // Replace the leading and trailing '@' (both ASCII) with '$'.
            let dollar_flag = format!("${}$", &at_flag[1..at_flag.len() - 1]);

            subst.insert(at_flag.clone(), dollar_flag.clone());
            back_subst.insert(dollar_flag, at_flag);
        }

        for _2_flag in &_2_flags {
            let at_flag = _2_flag.clone();
            // Replace the leading and trailing '@' (both ASCII) with '$'.
            let dollar_flag = format!("${}$", &at_flag[1..at_flag.len() - 1]);

            subst.insert(at_flag.clone(), dollar_flag.clone());
            back_subst.insert(dollar_flag, at_flag);
        }

        self.substitute_symbols(&subst);

        let mut restriction = get_flag_path_restriction(&_1_flags, &_2_flags, self.type_);

        // Apply restrictions.
        self.compose(&restriction, true);
        let _ = &mut restriction;

        // Rename $...$ flags back to @...@ flags.
        self.substitute_symbols(&back_subst);

        self
    }

    pub fn lenient_composition(
        &mut self,
        another: &HfstTransducer,
        _harmonize: bool,
    ) -> &mut HfstTransducer {
        // #if HAVE_XFSM: if (this->type == XFSM_TYPE) throw FunctionNotImplemented
        if self.type_ != another.type_ {
            crate::HFST_THROW_MESSAGE!(
                HfstTransducerTypeMismatchException,
                "HfstTransducer::lenient_composition"
            );
        }

        let mut retval = self.clone();
        // true is a dummy variable, false means do not encode epsilons
        retval
            .compose(another, true)
            .optimize()
            .priority_union(self)
            .optimize();

        *self = retval;
        self
    }

    pub fn cross_product(
        &mut self,
        another: &HfstTransducer,
        _harmonize: bool,
    ) -> &mut HfstTransducer {
        // #if HAVE_XFSM: if (this->type == XFSM_TYPE) throw FunctionNotImplemented
        if self.type_ != another.type_ {
            crate::HFST_THROW_MESSAGE!(
                HfstTransducerTypeMismatchException,
                "HfstTransducer::cross_product"
            );
        }

        let mut automata1 = self.clone();
        // [spec:hfst:def:hfst-transducer.hfst.automata2-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.automata2-fn]
        let mut automata2 = another.clone();

        // Check if both input transducers are automata
        // [spec:hfst:def:hfst-transducer.hfst.t1-proj-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.t1-proj-fn]
        let mut t1_proj = automata1.clone();
        t1_proj.input_project();
        // [spec:hfst:def:hfst-transducer.hfst.t2-proj-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.t2-proj-fn]
        let mut t2_proj = automata2.clone();
        t2_proj.input_project();

        if !t1_proj.compare(&automata1, true) || !t2_proj.compare(&automata2, true) {
            crate::HFST_THROW_MESSAGE!(
                TransducersAreNotAutomataException,
                "HfstTransducer::cross_product"
            );
        }

        // Put MARK all over lower part of automata1 and upper part of automata2,
        // and then compose them. Also, there should be created padding after
        // strings, on both sides
        automata1.insert_to_alphabet("@_MARK_@");
        automata2.insert_to_alphabet("@_MARK_@");

        let mut tok = HfstTokenizer::new();
        tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
        tok.add_multichar_symbol("@_UNKNOWN_SYMBOL_@");
        tok.add_multichar_symbol("@_MARK_@");

        // EpsilonToMark and MarkToEpsilon are paddings (if strings are not the
        // same size)
        let mut unknown_to_mark =
            HfstTransducer::from_strings("@_UNKNOWN_SYMBOL_@", "@_MARK_@", &tok, self.type_);
        let mut epsilon_to_mark =
            HfstTransducer::from_strings("@_EPSILON_SYMBOL_@", "@_MARK_@", &tok, self.type_);

        // [spec:hfst:def:hfst-transducer.hfst.mark-to-unknown-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.mark-to-unknown-fn]
        let mut mark_to_unknown = unknown_to_mark.clone();
        mark_to_unknown.invert();
        // [spec:hfst:def:hfst-transducer.hfst.mark-to-epsilon-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.mark-to-epsilon-fn]
        let mut mark_to_epsilon = epsilon_to_mark.clone();
        mark_to_epsilon.invert();

        unknown_to_mark.repeat_star().minimize(); // minimization is safe
        epsilon_to_mark.repeat_star().minimize(); // minimization is safe
        mark_to_unknown.repeat_star().minimize(); // minimization is safe
        mark_to_epsilon.repeat_star().minimize(); // minimization is safe

        // [spec:hfst:def:hfst-transducer.hfst.a1-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.a1-fn]
        let mut a1 = automata1.clone();
        a1.compose(&unknown_to_mark, true)
            .optimize()
            .concatenate(&epsilon_to_mark, true)
            .optimize();

        // [spec:hfst:def:hfst-transducer.hfst.b1-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.b1-fn]
        let mut b1 = mark_to_unknown.clone();
        b1.compose(&automata2, true)
            .optimize()
            .concatenate(&mark_to_epsilon, true)
            .optimize();

        // [spec:hfst:def:hfst-transducer.hfst.retval-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.retval-fn]
        let mut retval = a1.clone();
        retval.compose(&b1, true).optimize();

        // Expand ?:? transitions to ?:?|?
        let mut id_or_unk: StringPairSet = StringPairSet::new();
        id_or_unk.insert((
            "@_UNKNOWN_SYMBOL_@".to_string(),
            "@_UNKNOWN_SYMBOL_@".to_string(),
        ));
        id_or_unk.insert((
            "@_IDENTITY_SYMBOL_@".to_string(),
            "@_IDENTITY_SYMBOL_@".to_string(),
        ));
        retval.substitute_symbol_pair_with_set(
            &(
                "@_UNKNOWN_SYMBOL_@".to_string(),
                "@_UNKNOWN_SYMBOL_@".to_string(),
            ),
            &id_or_unk,
        );

        retval.remove_from_alphabet("@_MARK_@");

        *self = retval;
        self
    }

    pub fn shuffle(&mut self, another: &HfstTransducer, _b: bool) -> &mut HfstTransducer {
        // #if HAVE_XFSM: if (this->type == XFSM_TYPE) throw FunctionNotImplemented
        if self.type_ != another.type_ {
            crate::HFST_THROW_MESSAGE!(
                TransducerTypeMismatchException,
                "HfstTransducer::shuffle(const HfstTransducer&)"
            );
        }

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
        });
        // also remember to remove the unprefixed symbols from the alphabet
        this_basic.remove_symbols_from_alphabet(&this_alphabet);

        // Encode second transducer, i.e. prefix each symbol with "@2"
        coding_case.set(ShuffleCoding::ENCODE_SECOND_SHUFFLE_ARGUMENT);
        another_basic.substitute_with_func(|sp, sps| {
            code_symbols_for_shuffle_impl(sp, sps, &coding_case, &shuffle_failed)
        });
        // also remember to remove the unprefixed symbols from the alphabet
        another_basic.remove_symbols_from_alphabet(&another_alphabet);

        // See if shuffle failed, i.e. either transducer is not an automaton
        if shuffle_failed.get() {
            shuffle_failed.set(false);
            crate::HFST_THROW_MESSAGE!(
                TransducersAreNotAutomataException,
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
        this_basic.insert_freely_set(&another_alphabet_pairset, 0.0);
        another_basic.insert_freely_set(&this_alphabet_pairset, 0.0);

        // We use HfstTransducers for intersection
        let mut this1 = HfstTransducer::from_basic(&this_basic, self.get_type());
        let another1 = HfstTransducer::from_basic(&another_basic, another.get_type());

        this1.intersect(&another1, true);
        this1.optimize();

        // We use HfstBasicTransducers again
        // [spec:hfst:def:hfst-transducer.hfst.this1-basic-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.this1-basic-fn]
        let mut this1_basic = HfstBasicTransducer::from_transducer(&this1);

        // Decode the shuffled transducer, i.e. remove the prefixes
        // "@1" and "@2" from symbols
        coding_case.set(ShuffleCoding::DECODE_AFTER_SHUFFLE);
        this1_basic.substitute_with_func(|sp, sps| {
            code_symbols_for_shuffle_impl(sp, sps, &coding_case, &shuffle_failed)
        });
        // also remember to remove the prefixed symbols from the alphabet
        this1_basic.remove_symbols_from_alphabet(&this_alphabet);
        this1_basic.remove_symbols_from_alphabet(&another_alphabet);

        // Convert once again to HfstTransducer
        let this_finally = HfstTransducer::from_basic(&this1_basic, self.get_type());
        *self = this_finally;

        self
    }

    // ---------------------- Shuffle functions end --------------------

    // Q .P. R = Q | [~[Q .u] .o. R ]
    // .u is input project
    pub fn priority_union(&mut self, another: &HfstTransducer) -> &mut HfstTransducer {
        // #if HAVE_XFSM: if (this->type == XFSM_TYPE) throw FunctionNotImplemented
        if self.type_ != another.type_ {
            crate::HFST_THROW_MESSAGE!(
                HfstTransducerTypeMismatchException,
                "HfstTransducer::priority_union"
            );
        }
        let t1 = self.clone();
        // [spec:hfst:def:hfst-transducer.hfst.t2-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.t2-fn]
        let t2 = another.clone();

        // [spec:hfst:def:hfst-transducer.hfst.t1upper-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.t1upper-fn]
        let mut t1upper = t1.clone();
        t1upper.input_project().optimize();

        // [spec:hfst:def:hfst-transducer.hfst.complement-fn]
        // [spec:hfst:sem:hfst-transducer.hfst.complement-fn]
        let mut complement = t1upper.clone();
        complement.negate().prune_alphabet(false);

        complement.compose(&t2, true).optimize();

        let mut retval = t1.clone();
        retval.disjunct(&complement, true).optimize();

        *self = retval;
        self
    }

    #[allow(unused_variables, unused_mut, unreachable_code)]
    #[allow(unused_variables, unused_mut, unreachable_code)]
    pub fn compose_intersect(
        &mut self,
        v: &HfstTransducerVector,
        invert: bool,
        _b: bool,
    ) -> &mut HfstTransducer {
        // #if HAVE_XFSM: if (this->type == XFSM_TYPE) throw FunctionNotImplemented
        // Foma transducers don't harmonize porperly. If the input is foma
        // transducers, convert to openfst type.
        let mut convert_to_openfst = false;
        if self.get_type() == ImplementationType::FOMA_TYPE {
            convert_to_openfst = true;
            self.convert(ImplementationType::TROPICAL_OPENFST_TYPE, String::new());
        }

        // The intersection of an empty set of rules is the empty language,
        // which makes the result empty.
        if v.is_empty() {
            *self = HfstTransducer::from_type(self.type_);
        }

        let first = &v[0];

        // If rule transducers contain word boundaries, add word boundaries to
        // the lexicon unless the lexicon already contains them.
        let rule_alphabet = first.get_alphabet();

        if rule_alphabet.contains("@#@") {
            let lexicon_alphabet = self.get_alphabet();
            let mut tokenizer = HfstTokenizer::new();
            tokenizer.add_multichar_symbol("@#@");
            tokenizer.add_multichar_symbol(internal_epsilon);
            let mut wb =
                HfstTransducer::from_strings(internal_epsilon, "@#@", &tokenizer, self.type_);
            // [spec:hfst:def:hfst-transducer.hfst.wb-copy-fn]
            // [spec:hfst:sem:hfst-transducer.hfst.wb-copy-fn]
            let wb_copy = wb.clone();

            // Add the word boundary symbol to the alphabet so harmonization
            // won't touch it.
            let mut basic_this = HfstBasicTransducer::from_transducer(self);
            basic_this.add_symbol_to_alphabet(&"@#@".to_string());
            *self = HfstTransducer::from_basic(&basic_this, self.get_type());

            wb.concatenate(self, true)
                .concatenate(&wb_copy, true)
                .optimize();
            *self = wb;
            let _ = lexicon_alphabet;
        }

        let mut rule_1 = v[0].clone();

        if convert_to_openfst {
            rule_1.convert(ImplementationType::TROPICAL_OPENFST_TYPE, String::new());
        }

        // foma / no harmonization -> use our own copy.
        let mut harmonized_lexicon: HfstTransducer =
            rule_1.harmonize_(self).unwrap_or_else(|| self.clone());

        if invert {
            harmonized_lexicon.invert();
            harmonized_lexicon.substitute_symbol_pair(
                &("@#@".to_string(), internal_epsilon.to_string()),
                &(internal_epsilon.to_string(), "@#@".to_string()),
            );
        }

        harmonized_lexicon.substitute_symbol(
            internal_identity,
            "||_IDENTITY_SYMBOL_||",
            true,
            true,
        );
        harmonized_lexicon.substitute_symbol(internal_unknown, "||_UNKNOWN_SYMBOL_||", true, true);

        if v.len() == 1 {
            let mut rule_fst = v[0].clone();
            if convert_to_openfst {
                rule_fst.convert(ImplementationType::TROPICAL_OPENFST_TYPE, String::new());
            }

            if invert {
                rule_fst.invert();
                rule_fst.substitute_symbol_pair(
                    &(internal_epsilon.to_string(), "@#@".to_string()),
                    &("@#@".to_string(), internal_epsilon.to_string()),
                );
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

            let mut rule = crate::compose_intersect_rule::ComposeIntersectRule::new_from_transducer(
                &rule_basic,
            );

            // Create a ComposeIntersectLexicon from *harmonized_lexicon.
            let mut lexicon =
                crate::compose_intersect_lexicon::ComposeIntersectLexicon::new_from_transducer(
                    &lexicon_basic,
                );

            let mut res: HfstBasicTransducer = lexicon.compose_with_rules(&mut rule);

            res.prune_alphabet(true);
            *self = HfstTransducer::from_basic(&res, self.type_);
        } else {
            // In case there are many rules, build a ComposeIntersectRulePair
            // recursively and compose with that.
            let mut first_rule_fst = v[0].clone();
            if convert_to_openfst {
                first_rule_fst.convert(ImplementationType::TROPICAL_OPENFST_TYPE, String::new());
            }

            if invert {
                first_rule_fst.invert();
                first_rule_fst.substitute_symbol_pair(
                    &(internal_epsilon.to_string(), "@#@".to_string()),
                    &("@#@".to_string(), internal_epsilon.to_string()),
                );
            }

            let mut second_rule_fst = v[1].clone();
            if convert_to_openfst {
                second_rule_fst.convert(ImplementationType::TROPICAL_OPENFST_TYPE, String::new());
            }

            if invert {
                second_rule_fst.invert();
                second_rule_fst.substitute_symbol_pair(
                    &(internal_epsilon.to_string(), "@#@".to_string()),
                    &("@#@".to_string(), internal_epsilon.to_string()),
                );
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
                if convert_to_openfst {
                    rule_fst.convert(ImplementationType::TROPICAL_OPENFST_TYPE, String::new());
                }

                if invert {
                    rule_fst.invert();
                    rule_fst.substitute_symbol_pair(
                        &(internal_epsilon.to_string(), "@#@".to_string()),
                        &("@#@".to_string(), internal_epsilon.to_string()),
                    );
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

            let first_rule: Box<
                dyn crate::compose_intersect_rule_pair::ComposeIntersectRuleObject,
            > = Box::new(
                crate::compose_intersect_rule::ComposeIntersectRule::new_from_transducer(
                    &first_rule_basic,
                ),
            );
            let second_rule: Box<
                dyn crate::compose_intersect_rule_pair::ComposeIntersectRuleObject,
            > = Box::new(
                crate::compose_intersect_rule::ComposeIntersectRule::new_from_transducer(
                    &second_rule_basic,
                ),
            );
            let mut rules: Box<dyn crate::compose_intersect_rule_pair::ComposeIntersectRuleObject> =
                Box::new(
                    crate::compose_intersect_rule_pair::ComposeIntersectRulePair::new(
                        first_rule,
                        second_rule,
                    ),
                );

            for rule_basic in extra_rule_basics.iter() {
                // rules = new ComposeIntersectRulePair(
                //     new ComposeIntersectRule(rule_fst), rules);
                let new_rule: Box<
                    dyn crate::compose_intersect_rule_pair::ComposeIntersectRuleObject,
                > = Box::new(
                    crate::compose_intersect_rule::ComposeIntersectRule::new_from_transducer(
                        rule_basic,
                    ),
                );
                rules = Box::new(
                    crate::compose_intersect_rule_pair::ComposeIntersectRulePair::new(
                        new_rule, rules,
                    ),
                );
            }

            // Create a ComposeIntersectLexicon from *harmonized_lexicon.
            let mut lexicon =
                crate::compose_intersect_lexicon::ComposeIntersectLexicon::new_from_transducer(
                    &lexicon_basic,
                );
            let mut res: HfstBasicTransducer = lexicon.compose_with_rules(&mut *rules);

            res.prune_alphabet(true);
            *self = HfstTransducer::from_basic(&res, self.type_);

            if invert {
                self.invert();
            }

            // delete rules; -> the owning 'rules' Box (and the recursively nested
            // pairs/rules it owns) is dropped at the end of this scope.
        }

        drop(harmonized_lexicon);

        self.substitute_symbol("||_IDENTITY_SYMBOL_||", internal_identity, true, true);
        self.substitute_symbol("||_UNKNOWN_SYMBOL_||", internal_unknown, true, true);

        if convert_to_openfst {
            self.convert(ImplementationType::FOMA_TYPE, String::new());
        }

        self
    }

    pub fn concatenate(
        &mut self,
        another: &HfstTransducer,
        harmonize: bool,
    ) -> &mut HfstTransducer {
        self.is_trie = false; // This could be done so that is_trie is preserved
        self.apply_binary(
            |t1, t2| TropicalWeightTransducer::concatenate(t1, t2),
            |t1, t2| LogWeightTransducer::concatenate(t1, t2),
            another,
            harmonize,
        )
    }

    pub fn disjunct_spv(&mut self, spv: &StringPairVector) -> &mut HfstTransducer {
        match self.type_ {
            ImplementationType::TROPICAL_OPENFST_TYPE => {
                TropicalWeightTransducer::disjunct_spv(self.implementation.as_tropical_mut(), spv);
            }
            ImplementationType::LOG_OPENFST_TYPE => {
                crate::HFST_THROW!(FunctionNotImplementedException);
            }
            ImplementationType::FOMA_TYPE => {
                crate::HFST_THROW!(FunctionNotImplementedException);
            }
            // Add here your implementation.
            _ => {
                assert!(false);
            }
        }
        self
    }

    // TODO...
    pub(crate) fn disjunct_as_tries(
        &mut self,
        another: &mut HfstTransducer,
        type_: ImplementationType,
    ) -> &mut HfstTransducer {
        self.convert(type_, String::new());
        if type_ != another.type_ {
            let mut __tmp = another.clone();
            __tmp.convert(type_, String::new());
            *another = __tmp;
        }

        match self.type_ {
            ImplementationType::SFST_TYPE => {
                crate::HFST_THROW!(FunctionNotImplementedException);
            }
            ImplementationType::TROPICAL_OPENFST_TYPE => {
                crate::HFST_THROW!(FunctionNotImplementedException);
            }
            ImplementationType::LOG_OPENFST_TYPE => {
                crate::HFST_THROW!(FunctionNotImplementedException);
            }
            ImplementationType::FOMA_TYPE => {
                crate::HFST_THROW!(FunctionNotImplementedException);
            }
            _ => {
                assert!(false);
            }
        }
        self
    }

    pub fn disjunct(&mut self, another: &HfstTransducer, harmonize: bool) -> &mut HfstTransducer {
        self.is_trie = false;
        self.apply_binary(
            |t1, t2| TropicalWeightTransducer::disjunct(t1, t2),
            |t1, t2| LogWeightTransducer::disjunct(t1, t2),
            another,
            harmonize,
        )
    }

    pub fn intersect(&mut self, another: &HfstTransducer, harmonize: bool) -> &mut HfstTransducer {
        self.is_trie = false; // This could be done so that is_trie is preserved
        self.apply_binary(
            |t1, t2| TropicalWeightTransducer::intersect(t1, t2),
            |t1, t2| LogWeightTransducer::intersect(t1, t2),
            another,
            harmonize,
        )
    }

    pub fn subtract(&mut self, another: &HfstTransducer, harmonize: bool) -> &mut HfstTransducer {
        self.is_trie = false; // This could be done so that is_trie is preserved
        self.apply_binary(
            |t1, t2| TropicalWeightTransducer::subtract(t1, t2),
            |t1, t2| LogWeightTransducer::subtract(t1, t2),
            another,
            harmonize,
        )
    }
}

// ===== io_misc (workflow body) =====
// ===== io_misc (flattened body) =====
use crate::hfst_exception_defs::StreamCannotBeWrittenException;

// -----------------------------------------------------------------------
//   AT&T / xfsm / prolog I/O, tokenizer creation, lexc and misc factories
//   (HfstTransducer.cc lines ~5823-6410).
//
// 'HfstBasicTransducer net(*this)' is the conversion constructor
// 'HfstBasicTransducer(const HfstTransducer&)' — ported here as the assoc-fn
// 'HfstBasicTransducer::new_from_hfst_transducer(&self)' (the
// 'hfst_transducer_to_hfst_basic_transducer' type-dispatch). The
// 'HfstTransducer(const HfstBasicTransducer&, ImplementationType)' ctor is
// provided by the skeleton as 'HfstTransducer::new_from_basic_transducer'.
//
// All '#if HAVE_XFSM' / '#if HAVE_SFST' backend blocks are compiled out (the
// reduced union only carries tropical_ofst/log_ofst/hfst_ol), so the XFSM
// guards collapse to their fall-through throws and the SFST_TYPE branch of
// 'create_tokenizer' stays as a plain runtime check.
impl HfstTransducer {
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.write-in-att-format-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.write-in-att-format-fn]
    pub fn write_in_att_format_filename(&self, filename: &str, print_weights: bool) {
        let file = match std::fs::File::create(filename) {
            Ok(f) => f,
            Err(_) => {
                let message = filename.to_string();
                crate::HFST_THROW_MESSAGE!(StreamCannotBeWrittenException, message);
            }
        };
        let mut ofile = std::io::BufWriter::new(file);
        self.write_in_att_format_file(&mut ofile, print_weights);
        let _ = std::io::Write::flush(&mut ofile);
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.write-in-att-format-number-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.write-in-att-format-number-fn]
    pub fn write_in_att_format_number(&self, ofile: &mut dyn std::io::Write, print_weights: bool) {
        let net = HfstBasicTransducer::new_from_hfst_transducer(self);
        net.write_in_att_format_number_file(ofile, print_weights);
    }

    pub fn write_in_att_format_file(&self, ofile: &mut dyn std::io::Write, print_weights: bool) {
        // Implemented only for internal transducer format.
        let net = HfstBasicTransducer::new_from_hfst_transducer(self);
        net.write_in_att_format_file(ofile, print_weights);
    }

    /* Implemented only for XFSM_TYPE. */
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.write-xfsm-transducer-in-prolog-format-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.write-xfsm-transducer-in-prolog-format-fn]
    pub fn write_xfsm_transducer_in_prolog_format(&self, filename: &str) {
        if self.type_ != ImplementationType::XFSM_TYPE {
            crate::HFST_THROW!(FunctionNotImplementedException);
        }
        let _ = filename;
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.write-in-prolog-format-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.write-in-prolog-format-fn]
    pub fn write_in_prolog_format(
        &mut self,
        file: &mut dyn std::io::Write,
        name: &str,
        write_weights: bool,
    ) {
        /* For big transducers, converting from xfsm is slow. */
        if self.type_ == ImplementationType::XFSM_TYPE {
            crate::HFST_THROW!(FunctionNotImplementedException);
        }
        let fsm = HfstBasicTransducer::new_from_hfst_transducer(self);
        fsm.write_in_prolog_format_file(file, name, write_weights);
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.prolog-file-to-xfsm-transducer-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.prolog-file-to-xfsm-transducer-fn]
    pub fn prolog_file_to_xfsm_transducer(filename: &str) -> HfstTransducer {
        let _ = filename;
        crate::HFST_THROW!(FunctionNotImplementedException)
    }

    /// 'HfstTransducer &read_in_att_format(const std::string &filename, type,
    ///  const std::string &epsilon_symbol, bool warn_negs)'.
    pub fn read_in_att_format_filename<'a>(
        filename: &str,
        type_: ImplementationType,
        epsilon_symbol: &str,
        warn_negs: bool,
    ) -> &'a mut HfstTransducer {
        if type_ == XFSM_TYPE {
            HFST_THROW!(FunctionNotImplementedException);
        }
        let ifile = match std::fs::File::open(filename) {
            Ok(f) => f,
            Err(_) => {
                // [spec:hfst:def:hfst-transducer.hfst.message-fn]
                // [spec:hfst:sem:hfst-transducer.hfst.message-fn]
                HFST_THROW_MESSAGE!(StreamNotReadableException, filename);
            }
        };
        HfstTokenizer::check_utf8_correctness(epsilon_symbol);

        let mut reader = std::io::BufReader::new(ifile);
        Self::read_in_att_format_file(&mut reader, type_, epsilon_symbol, warn_negs)
    }

    /// 'HfstTransducer &read_in_att_format(FILE *ifile, type,
    ///  const std::string &epsilon_symbol, bool warn_negs)'.
    pub fn read_in_att_format_file<'a>(
        ifile: &mut dyn std::io::BufRead,
        type_: ImplementationType,
        epsilon_symbol: &str,
        warn_negs: bool,
    ) -> &'a mut HfstTransducer {
        if type_ == XFSM_TYPE {
            HFST_THROW!(FunctionNotImplementedException);
        }
        if !Self::is_implementation_type_available(type_) {
            std::panic::panic_any(ImplementationTypeNotAvailableException::new(
                "ImplementationTypeNotAvailableException".to_string(),
                file!().to_string(),
                line!() as usize,
                type_,
            ));
        }
        HfstTokenizer::check_utf8_correctness(epsilon_symbol);

        let mut foo: u32 = 0;
        let net = HfstBasicTransducer::read_in_att_format_file(
            ifile,
            epsilon_symbol,
            &mut foo,
            warn_negs,
        );
        // C++ 'new HfstTransducer(net, type)' returned by reference; 'Box::leak'
        // mirrors the heap allocation the caller takes ownership of / deletes.
        let _ = foo;
        Box::leak(Box::new(HfstTransducer::new_from_basic(&net, type_)))
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.universal-pair-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.universal-pair-fn]
    pub fn universal_pair(type_: ImplementationType) -> HfstTransducer {
        let mut bt = HfstBasicTransducer::new();
        let tr = HfstBasicTransition::new_symbols(
            1,
            "@_IDENTITY_SYMBOL_@".to_string(),
            "@_IDENTITY_SYMBOL_@".to_string(),
            0.0,
            bt.coder_mut(),
        );
        bt.add_transition(0, &tr, true);
        let tr = HfstBasicTransition::new_symbols(
            1,
            "@_UNKNOWN_SYMBOL_@".to_string(),
            "@_UNKNOWN_SYMBOL_@".to_string(),
            0.0,
            bt.coder_mut(),
        );
        bt.add_transition(0, &tr, true);
        let tr = HfstBasicTransition::new_symbols(
            1,
            "@_UNKNOWN_SYMBOL_@".to_string(),
            "@_EPSILON_SYMBOL_@".to_string(),
            0.0,
            bt.coder_mut(),
        );
        bt.add_transition(0, &tr, true);
        let tr = HfstBasicTransition::new_symbols(
            1,
            "@_EPSILON_SYMBOL_@".to_string(),
            "@_UNKNOWN_SYMBOL_@".to_string(),
            0.0,
            bt.coder_mut(),
        );
        bt.add_transition(0, &tr, true);
        bt.set_final_weight(1, &0.0);

        let Retval = HfstTransducer::new_from_basic_transducer(&bt, type_);

        Retval
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.identity-pair-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.identity-pair-fn]
    pub fn identity_pair(type_: ImplementationType) -> HfstTransducer {
        let mut bt = HfstBasicTransducer::new();
        let tr = HfstBasicTransition::new_symbols(
            1,
            "@_IDENTITY_SYMBOL_@".to_string(),
            "@_IDENTITY_SYMBOL_@".to_string(),
            0.0,
            bt.coder_mut(),
        );
        bt.add_transition(0, &tr, true);
        bt.set_final_weight(1, &0.0);

        let Retval = HfstTransducer::new_from_basic_transducer(&bt, type_);

        Retval
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.create-tokenizer-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.create-tokenizer-fn]
    pub fn create_tokenizer(&mut self) -> HfstTokenizer {
        let mut tok = HfstTokenizer::new();

        if self.type_ == ImplementationType::SFST_TYPE {
            let sps = self.get_symbol_pairs();
            for sp in sps.iter() {
                if sp.0.len() > 1 {
                    tok.add_multichar_symbol(&sp.0);
                }
                if sp.1.len() > 1 {
                    tok.add_multichar_symbol(&sp.1);
                }
            }
        } else {
            let mut t = HfstBasicTransducer::new_from_hfst_transducer(self);
            t.prune_alphabet(true);
            let alpha = t.get_alphabet();
            for it in alpha.iter() {
                if it.len() > 1 {
                    tok.add_multichar_symbol(it);
                }
            }
        }

        tok
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.read-lexc-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.read-lexc-fn]
    pub fn read_lexc(filename: &str, type_: ImplementationType, verbose: bool) -> HfstTransducer {
        HfstTransducer::read_lexc_ptr(filename, type_, verbose)
            .expect("read_lexc: lexc compilation produced no transducer")
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.read-lexc-ptr-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.read-lexc-ptr-fn]
    pub fn read_lexc_ptr(
        filename: &str,
        type_: ImplementationType,
        verbose: bool,
    ) -> Option<HfstTransducer> {
        if type_ == ImplementationType::XFSM_TYPE {
            HFST_THROW!(FunctionNotImplementedException);
        }

        if !HfstTransducer::is_implementation_type_available(type_) {
            std::panic::panic_any(ImplementationTypeNotAvailableException::new(
                "ImplementationTypeNotAvailableException".to_string(),
                file!().to_string(),
                line!() as usize,
                type_,
            ));
        }

        match type_ {
            ImplementationType::FOMA_TYPE
            | ImplementationType::SFST_TYPE
            | ImplementationType::TROPICAL_OPENFST_TYPE
            | ImplementationType::LOG_OPENFST_TYPE => {
                // The C++ 'compiler.parse(filename.c_str())' reads the file via the
                // Flex/Bison lexer; the ported LexcCompiler walks an AST built from
                // source text instead, so read the file here and feed 'compile'.
                // (The C++ 'new HfstTransducer()' placeholder that it then leaks was a
                // raw-pointer artifact and is gone with the owned return.)
                let mut compiler = crate::lexc::LexcCompiler::new(type_);
                compiler.set_verbosity(verbose as u32);
                let source = std::fs::read_to_string(filename).unwrap();
                compiler.compile(&source)
            }
            ImplementationType::ERROR_TYPE => {
                HFST_THROW!(TransducerHasWrongTypeException);
            }
            _ => {
                HFST_THROW!(TransducerHasWrongTypeException);
            }
        }
    }
}

// ===== integration shims: Clone (C++ copy ctor) + constructor-name aliases =====
// The body modules were translated against synonym constructor/copy names; these
// thin forwarders bridge them to the skeleton's canonical 'new_*' constructors.
impl Clone for HfstTransducer {
    fn clone(&self) -> Self {
        HfstTransducer::new_copy(self)
    }
}

impl HfstTransducer {
    pub fn new_from(another: &HfstTransducer) -> Self {
        HfstTransducer::new_copy(another)
    }
    pub fn new_from_transducer(another: &HfstTransducer) -> Self {
        HfstTransducer::new_copy(another)
    }
    pub fn from_type(type_: ImplementationType) -> Self {
        HfstTransducer::new_type(type_)
    }
    pub fn from_symbol(symbol: &str, type_: ImplementationType) -> Self {
        HfstTransducer::new_symbol(symbol, type_)
    }
    pub fn new_from_symbol(symbol: &str, type_: ImplementationType) -> Self {
        HfstTransducer::new_symbol(symbol, type_)
    }
    pub fn from_isymbol_osymbol(isymbol: &str, osymbol: &str, type_: ImplementationType) -> Self {
        HfstTransducer::new_symbol_pair(isymbol, osymbol, type_)
    }
    pub fn new_from_symbol_pair(isymbol: &str, osymbol: &str, type_: ImplementationType) -> Self {
        HfstTransducer::new_symbol_pair(isymbol, osymbol, type_)
    }
    pub fn from_strings(
        isymbol: &str,
        osymbol: &str,
        tokenizer: &HfstTokenizer,
        type_: ImplementationType,
    ) -> Self {
        HfstTransducer::new_tokenized_pair(isymbol, osymbol, tokenizer, type_)
    }
    pub fn new_string_tokenizer_type(
        utf8_str: &str,
        tokenizer: &HfstTokenizer,
        type_: ImplementationType,
    ) -> Self {
        HfstTransducer::new_tokenized(utf8_str, tokenizer, type_)
    }
    pub fn new_string_string_tokenizer_type(
        upper: &str,
        lower: &str,
        tokenizer: &HfstTokenizer,
        type_: ImplementationType,
    ) -> Self {
        HfstTransducer::new_tokenized_pair(upper, lower, tokenizer, type_)
    }
    pub fn from_basic(net: &HfstBasicTransducer, type_: ImplementationType) -> Self {
        HfstTransducer::new_from_basic(net, type_)
    }
    pub fn from_basic_transducer(net: &HfstBasicTransducer, type_: ImplementationType) -> Self {
        HfstTransducer::new_from_basic(net, type_)
    }
    pub fn new_from_basic_transducer(net: &HfstBasicTransducer, type_: ImplementationType) -> Self {
        HfstTransducer::new_from_basic(net, type_)
    }
    pub fn from_string_pair_set(
        sps: &StringPairSet,
        type_: ImplementationType,
        cyclic: bool,
    ) -> Self {
        HfstTransducer::new_string_pair_set(sps, type_, cyclic)
    }
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
// 'Minimize' / 'harmonize_' do not branch on them — so they are carried as inert
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
            crate::hfst_symbol_defs::internal_epsilon.to_string(),
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
            crate::hfst_symbol_defs::internal_epsilon.to_string(),
        );
        sps.insert(new_pair);
        return true;
    }
    false
}

// ===== integration shims: alphabet / substitute overload-name aliases =====
impl HfstTransducer {
    pub fn insert_to_alphabet_symbol<S: AsRef<str>>(&mut self, symbol: S) {
        self.insert_to_alphabet_string(symbol.as_ref());
    }
    pub fn insert_to_alphabet<S: AsRef<str>>(&mut self, symbol: S) {
        self.insert_to_alphabet_string(symbol.as_ref());
    }
    pub fn insert_to_alphabet_set(&mut self, symbols: &StringSet) {
        self.insert_to_alphabet_string_set(symbols);
    }
    pub fn remove_from_alphabet_symbol<S: AsRef<str>>(&mut self, symbol: S) {
        self.remove_from_alphabet_string(symbol.as_ref());
    }
    pub fn remove_from_alphabet<S: AsRef<str>>(&mut self, symbol: S) {
        self.remove_from_alphabet_string(symbol.as_ref());
    }
    pub fn remove_from_alphabet_set(&mut self, symbols: &StringSet) {
        self.remove_from_alphabet_string_set(symbols);
    }
    pub fn substitute<A: AsRef<str>, B: AsRef<str>>(
        &mut self,
        old_symbol: A,
        new_symbol: B,
        input_side: bool,
        output_side: bool,
    ) -> &mut HfstTransducer {
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
    ) -> &mut HfstTransducer {
        self.substitute_symbol_substitutions(substitutions)
    }
}

// ===== integration shims: HfstBasicTransducer<-facade ctors, method + free-fn aliases =====
impl HfstBasicTransducer {
    /// 'HfstBasicTransducer(const HfstTransducer&)' — convert a facade transducer
    /// to the interchange basic transducer.
    pub fn from_transducer(t: &HfstTransducer) -> HfstBasicTransducer {
        t.get_basic_transducer()
    }
    pub fn new_from_transducer(t: &HfstTransducer) -> HfstBasicTransducer {
        HfstBasicTransducer::from_transducer(t)
    }
    pub fn new_from_hfst_transducer(t: &HfstTransducer) -> HfstBasicTransducer {
        HfstBasicTransducer::from_transducer(t)
    }
    pub fn from_hfst_transducer(t: &HfstTransducer) -> HfstBasicTransducer {
        HfstBasicTransducer::from_transducer(t)
    }
}

impl HfstTransducer {
    pub fn insert_freely_transducer(
        &mut self,
        tr: &HfstTransducer,
        harmonize: bool,
    ) -> &mut HfstTransducer {
        self.insert_freely(tr, harmonize)
    }
    pub fn apply_binary(
        &mut self,
        tropical_ofst_funct: fn(&StdVectorFst, &StdVectorFst) -> StdVectorFst,
        log_ofst_funct: fn(&LogFst, &LogFst) -> LogFst,
        another_tr: &HfstTransducer,
        harmonize: bool,
    ) -> &mut HfstTransducer {
        self.apply_another(tropical_ofst_funct, log_ofst_funct, another_tr, harmonize)
    }
}

// C++ file-static flag-diacritic helpers. 'has_flags' is read-only; the others
// mutate the transducer in place (C++ 'fst = HfstTransducer(...)'), so they take
// &mut HfstTransducer (callers pass &mut self / &mut another).

// [spec:hfst:def:hfst-transducer.hfst.encode-flag-fn]
// [spec:hfst:sem:hfst-transducer.hfst.encode-flag-fn]
fn encode_flag(flag_diacritic: &str) -> String {
    let mut retval: Vec<u8> = flag_diacritic.as_bytes().to_vec();
    let last = retval.len() - 1;
    retval[0] = b'%';
    retval[last] = b'%';
    String::from_utf8(retval).unwrap()
}

// [spec:hfst:def:hfst-transducer.hfst.decode-flag-fn]
// [spec:hfst:sem:hfst-transducer.hfst.decode-flag-fn]
fn decode_flag(flag_diacritic: &str) -> String {
    let bytes = flag_diacritic.as_bytes();
    if bytes[0] != b'%' || bytes[bytes.len() - 1] != b'%' {
        return flag_diacritic.to_string();
    }
    let mut retval: Vec<u8> = bytes.to_vec();
    let last = retval.len() - 1;
    retval[0] = b'@';
    retval[last] = b'@';
    String::from_utf8(retval).unwrap()
}

// [spec:hfst:def:hfst-transducer.hfst.add-suffix-to-feature-name-fn]
// [spec:hfst:sem:hfst-transducer.hfst.add-suffix-to-feature-name-fn]
fn add_suffix_to_feature_name(flag_diacritic: &str, suffix: &str) -> String {
    "@".to_string()
        + &FdOperation::get_operator(flag_diacritic)
        + "."
        + &FdOperation::get_feature(flag_diacritic)
        + suffix
        + &(if FdOperation::has_value(flag_diacritic) {
            ".".to_string() + &FdOperation::get_value(flag_diacritic)
        } else {
            String::new()
        })
        + "@"
}

// [spec:hfst:def:hfst-transducer.hfst.has-flags-fn]
// [spec:hfst:sem:hfst-transducer.hfst.has-flags-fn]
fn has_flags(fst: &HfstTransducer) -> bool {
    let alphabet = fst.get_alphabet();
    for it in alphabet.iter() {
        if FdOperation::is_diacritic(it) {
            return true;
        }
    }
    false
}

// Return true if the flag in flag_diacritic ends in suffix and false
// otherwise. E.g. if flag_diacritic = "@D.NeedNoun_1.ON@ and suffix =
// "_1", return true.
// [spec:hfst:def:hfst-transducer.hfst.is-flag-suffix-fn]
// [spec:hfst:sem:hfst-transducer.hfst.is-flag-suffix-fn]
#[allow(dead_code)]
fn is_flag_suffix(suffix: &str, flag_diacritic: &str) -> bool {
    let flag_end_pos = match flag_diacritic.rfind('.') {
        None => return false,
        Some(pos) => pos,
    };

    if flag_end_pos < suffix.len() {
        return false;
    }

    if flag_diacritic[flag_end_pos - suffix.len()..flag_end_pos] != *suffix {
        return false;
    }

    true
}

// [spec:hfst:def:hfst-transducer.hfst.rename-flag-diacritics-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rename-flag-diacritics-fn]
fn rename_flag_diacritics(fst: &mut HfstTransducer, suffix: &str) {
    let basic_fst = HfstBasicTransducer::from_transducer(fst);
    let mut basic_fst_copy = HfstBasicTransducer::new();
    let _ = basic_fst_copy.add_state(basic_fst.get_max_state());

    let mut s: HfstState = 0;

    for states in basic_fst.state_vector.iter() {
        for transition in states.iter() {
            let input_symbol = transition.get_input_symbol(basic_fst.coder());
            let output_symbol = transition.get_output_symbol(basic_fst.coder());
            let isym = if FdOperation::is_diacritic(&input_symbol) {
                add_suffix_to_feature_name(&input_symbol, suffix)
            } else {
                input_symbol
            };
            let osym = if FdOperation::is_diacritic(&output_symbol) {
                add_suffix_to_feature_name(&output_symbol, suffix)
            } else {
                output_symbol
            };
            let tr = HfstBasicTransition::new_symbols(
                transition.get_target_state(),
                isym,
                osym,
                transition.get_weight(),
                basic_fst_copy.coder_mut(),
            );
            basic_fst_copy.add_transition(s, &tr, true);
        }

        if basic_fst.is_final_state(s) {
            basic_fst_copy.set_final_weight(s, &basic_fst.get_final_weight(s));
        }

        s += 1;
    }
    *fst = HfstTransducer::new_from_basic(&basic_fst_copy, fst.get_type());
}

// [spec:hfst:def:hfst-transducer.hfst.encode-flag-diacritics-fn]
// [spec:hfst:sem:hfst-transducer.hfst.encode-flag-diacritics-fn]
fn encode_flag_diacritics(fst: &mut HfstTransducer) {
    let basic_fst = HfstBasicTransducer::from_transducer(fst);
    let mut basic_fst_copy = HfstBasicTransducer::new();
    let _ = basic_fst_copy.add_state(basic_fst.get_max_state());

    let mut s: HfstState = 0;

    for states in basic_fst.state_vector.iter() {
        for transition in states.iter() {
            let input_symbol = transition.get_input_symbol(basic_fst.coder());
            let output_symbol = transition.get_output_symbol(basic_fst.coder());
            let isym = if FdOperation::is_diacritic(&input_symbol) {
                encode_flag(&input_symbol)
            } else {
                input_symbol
            };
            let osym = if FdOperation::is_diacritic(&output_symbol) {
                encode_flag(&output_symbol)
            } else {
                output_symbol
            };
            let tr = HfstBasicTransition::new_symbols(
                transition.get_target_state(),
                isym,
                osym,
                transition.get_weight(),
                basic_fst_copy.coder_mut(),
            );
            basic_fst_copy.add_transition(s, &tr, true);
        }

        if basic_fst.is_final_state(s) {
            basic_fst_copy.set_final_weight(s, &basic_fst.get_final_weight(s));
        }

        s += 1;
    }

    // copy alphabet, encode all flags
    let alpha = basic_fst.get_alphabet().clone();
    for it in alpha.iter() {
        if it.len() > 4 {
            let bytes = it.as_bytes();
            if (bytes[0] == b'%') && (bytes[it.len() - 1] == b'%') {
                let mut str_bytes = bytes.to_vec();
                let last = str_bytes.len() - 1;
                str_bytes[0] = b'@';
                str_bytes[last] = b'@';
                let str_ = String::from_utf8(str_bytes).unwrap();
                if FdOperation::is_diacritic(&str_) {
                    let msg = "error: reserved symbol '".to_string() + &str_ + "' detected";
                    std::panic::panic_any(msg);
                }
            }
        }
        let mut symbol: String = it.clone();
        if FdOperation::is_diacritic(&symbol) {
            symbol = encode_flag(&symbol);
        }
        basic_fst_copy.add_symbol_to_alphabet(&symbol);
    }

    *fst = HfstTransducer::new_from_basic(&basic_fst_copy, fst.get_type());
}

// [spec:hfst:def:hfst-transducer.hfst.decode-flag-diacritics-fn]
// [spec:hfst:sem:hfst-transducer.hfst.decode-flag-diacritics-fn]
fn decode_flag_diacritics(fst: &mut HfstTransducer) {
    let basic_fst = HfstBasicTransducer::from_transducer(fst);
    let mut basic_fst_copy = HfstBasicTransducer::new();
    let _ = basic_fst_copy.add_state(basic_fst.get_max_state());

    let mut s: HfstState = 0;

    for states in basic_fst.state_vector.iter() {
        for transition in states.iter() {
            let input_symbol = transition.get_input_symbol(basic_fst.coder());
            let output_symbol = transition.get_output_symbol(basic_fst.coder());

            let mut istr = decode_flag(&input_symbol);
            if !FdOperation::is_diacritic(&istr) {
                istr = input_symbol;
            }

            let mut ostr = decode_flag(&output_symbol);
            if !FdOperation::is_diacritic(&ostr) {
                ostr = output_symbol;
            }

            let tr = HfstBasicTransition::new_symbols(
                transition.get_target_state(),
                istr,
                ostr,
                transition.get_weight(),
                basic_fst_copy.coder_mut(),
            );
            basic_fst_copy.add_transition(s, &tr, true);
        }

        if basic_fst.is_final_state(s) {
            basic_fst_copy.set_final_weight(s, &basic_fst.get_final_weight(s));
        }

        s += 1;
    }

    // copy alphabet, decode all flags
    let alpha = basic_fst.get_alphabet().clone();
    for it in alpha.iter() {
        let mut symbol: String = decode_flag(it);
        if !FdOperation::is_diacritic(&symbol) {
            symbol = it.clone();
        }
        basic_fst_copy.add_symbol_to_alphabet(&symbol);
    }

    *fst = HfstTransducer::new_from_basic(&basic_fst_copy, fst.get_type());
}

// C++ 'operator<<(std::ostream &out, const HfstTransducer &t)' (HfstTransducer.cc:6419)
// — write the transducer in AT&T format. Implemented only for the internal
// (basic) transducer format: convert to a HfstBasicTransducer and write it.
pub fn operator_shl_os(out: &mut dyn std::io::Write, t: &HfstTransducer) {
    // (XFSM_TYPE branch is #if'd out.)
    let net = HfstBasicTransducer::from_transducer(t);
    // C++ writes weights for every type except SFST/FOMA (both out of scope here).
    let write_weights = t.get_type() != ImplementationType::SFST_TYPE
        && t.get_type() != ImplementationType::FOMA_TYPE;
    net.write_in_att_format_os(out, write_weights);
}
