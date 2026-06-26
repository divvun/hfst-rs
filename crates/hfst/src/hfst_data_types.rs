//! Port of 'libhfst/src/HfstDataTypes.{h,cc}' — datatypes needed by the HFST API.
//!
//! 1:1 translation. 'std::pair's that are used as ordered-set elements and carry
//! a leading 'float' ('HfstOneLevelPath', 'HfstTwoLevelPath') cannot be plain
//! Rust tuples (an 'f32' tuple is not 'Ord'), so they are modelled as newtype
//! structs that keep the C++ '.first'/'.second' field names and impl 'Ord' via
//! 'f32::total_cmp' followed by the vector comparison — mirroring
//! 'std::pair::operator<'.

use std::cmp::Ordering;
use std::collections::BTreeSet;

// The HfstTransducer-dependent typedefs from HfstDataTypes.h
// ('HfstTransducerVector', 'HfstTransducerPair', 'HfstTransducerPairVector')
// are deferred until 'HfstTransducer' is ported (facade layer), since Rust
// cannot reference an as-yet-undefined type the way a C++ forward declaration
// can.

/// \brief The type of an HfstTransducer.
// [spec:hfst:def:hfst-data-types.hfst.implementation-type]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum ImplementationType {
    SFST_TYPE,
    TROPICAL_OPENFST_TYPE,
    LOG_OPENFST_TYPE,
    FOMA_TYPE,
    XFSM_TYPE,
    HFST_OL_TYPE,
    HFST_OLW_TYPE,
    HFST2_TYPE,
    UNSPECIFIED_TYPE,
    ERROR_TYPE,
}

/// \brief The type of a push operation. @see HfstTransducer::push_weights
// [spec:hfst:def:hfst-data-types.hfst.push-type]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum PushType {
    TO_INITIAL_STATE,
    TO_FINAL_STATE,
}

// [spec:hfst:def:hfst-data-types.hfst.string-pair]
pub type StringPair = (String, String);
// [spec:hfst:def:hfst-data-types.hfst.string-pair-set]
pub type StringPairSet = BTreeSet<StringPair>;
// [spec:hfst:def:hfst-data-types.hfst.string-vector]
pub type StringVector = Vec<String>;
// [spec:hfst:def:hfst-data-types.hfst.string-pair-vector]
pub type StringPairVector = Vec<StringPair>;

/// \brief A path of one level of transitions with collected weight.
///
/// 'typedef std::pair<float, StringVector> HfstOneLevelPath'.
// [spec:hfst:def:hfst-data-types.hfst.hfst-one-level-path]
#[derive(Clone, Debug)]
pub struct HfstOneLevelPath {
    pub first: f32,
    pub second: StringVector,
}

impl PartialEq for HfstOneLevelPath {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for HfstOneLevelPath {}
impl PartialOrd for HfstOneLevelPath {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for HfstOneLevelPath {
    fn cmp(&self, other: &Self) -> Ordering {
        self.first
            .total_cmp(&other.first)
            .then_with(|| self.second.cmp(&other.second))
    }
}

// [spec:hfst:def:hfst-data-types.hfst.hfst-one-level-paths]
pub type HfstOneLevelPaths = BTreeSet<HfstOneLevelPath>;

/// \brief A path of two levels of transitions with collected weight.
///
/// 'typedef std::pair<float, StringPairVector> HfstTwoLevelPath'.
// [spec:hfst:def:hfst-data-types.hfst.hfst-two-level-path]
#[derive(Clone, Debug)]
pub struct HfstTwoLevelPath {
    pub first: f32,
    pub second: StringPairVector,
}

impl PartialEq for HfstTwoLevelPath {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for HfstTwoLevelPath {}
impl PartialOrd for HfstTwoLevelPath {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for HfstTwoLevelPath {
    fn cmp(&self, other: &Self) -> Ordering {
        self.first
            .total_cmp(&other.first)
            .then_with(|| self.second.cmp(&other.second))
    }
}

// [spec:hfst:def:hfst-data-types.hfst.hfst-two-level-paths]
pub type HfstTwoLevelPaths = BTreeSet<HfstTwoLevelPath>;

pub mod implementations {
    // [spec:hfst:def:hfst-data-types.hfst.implementations.hfst-state]
    pub type HfstState = u32;
}

// [spec:hfst:def:hfst-data-types.hfst.implementation-type-to-string-fn]
// [spec:hfst:sem:hfst-data-types.hfst.implementation-type-to-string-fn]
pub fn implementation_type_to_string(type_: ImplementationType) -> &'static str {
    match type_ {
        ImplementationType::SFST_TYPE => "SFST_TYPE",
        ImplementationType::TROPICAL_OPENFST_TYPE => "TROPICAL_OPENFST_TYPE",
        ImplementationType::LOG_OPENFST_TYPE => "LOG_OPENFST_TYPE",
        ImplementationType::FOMA_TYPE => "FOMA_TYPE",
        ImplementationType::XFSM_TYPE => "XFSM_TYPE",
        ImplementationType::HFST_OL_TYPE => "HFST_OL_TYPE",
        ImplementationType::HFST_OLW_TYPE => "HFST_OLW_TYPE",
        ImplementationType::HFST2_TYPE => "HFST2_TYPE",
        ImplementationType::UNSPECIFIED_TYPE => "UNSPECIFIED_TYPE",
        ImplementationType::ERROR_TYPE => "ERROR_TYPE",
    }
}

// [spec:hfst:def:hfst-data-types.hfst.implementation-type-to-format-fn]
// [spec:hfst:sem:hfst-data-types.hfst.implementation-type-to-format-fn]
pub fn implementation_type_to_format(type_: ImplementationType) -> &'static str {
    match type_ {
        ImplementationType::SFST_TYPE => "sfst",
        ImplementationType::TROPICAL_OPENFST_TYPE => "openfst-tropical",
        ImplementationType::LOG_OPENFST_TYPE => "openfst-log",
        ImplementationType::FOMA_TYPE => "foma",
        ImplementationType::XFSM_TYPE => "xfsm",
        ImplementationType::HFST_OL_TYPE => "hfst-optimized-lookup-unweighted",
        ImplementationType::HFST_OLW_TYPE => "hfst-optimized-lookup-weighted",
        ImplementationType::HFST2_TYPE => "hfst2",
        ImplementationType::UNSPECIFIED_TYPE => "unspecified-type",
        ImplementationType::ERROR_TYPE => "error-type",
    }
}

// [spec:hfst:def:hfst-data-types.hfst.size-t-to-int-fn]
// [spec:hfst:sem:hfst-data-types.hfst.size-t-to-int-fn]
pub fn size_t_to_int(value: usize) -> i32 {
    if value > i32::MAX as usize {
        panic!("data is larger than INT_MAX");
    }
    value as i32
}

// [spec:hfst:def:hfst-data-types.hfst.size-t-to-uint-fn]
// [spec:hfst:sem:hfst-data-types.hfst.size-t-to-uint-fn]
pub fn size_t_to_uint(value: usize) -> u32 {
    if value > u32::MAX as usize {
        panic!("data is larger than UINT_MAX");
    }
    value as u32
}

// [spec:hfst:def:hfst-data-types.hfst.size-t-to-ushort-fn]
// [spec:hfst:sem:hfst-data-types.hfst.size-t-to-ushort-fn]
pub fn size_t_to_ushort(value: usize) -> u16 {
    if value > u16::MAX as usize {
        panic!("data is larger than USHRT_MAX");
    }
    value as u16
}

// [spec:hfst:def:hfst-data-types.hfst.double-to-float-fn]
// [spec:hfst:sem:hfst-data-types.hfst.double-to-float-fn]
pub fn double_to_float(value: f64) -> f32 {
    if value > f32::MAX as f64 {
        panic!("data is larger than FLT_MAX");
    }
    value as f32
}

// [spec:hfst:def:hfst-data-types.hfst.hfst-fopen-fn]
// [spec:hfst:sem:hfst-data-types.hfst.hfst-fopen-fn]
//
// Thin portability wrapper around the platform's file-open call. On MSVC the
// C++ uses 'fopen_s'; on all other platforms it is 'fopen'. Mirrors the raw
// 'FILE *' C surface, so the signature is over raw C pointers and the call is
// 'unsafe'.
pub unsafe fn hfst_fopen(
    filename: *const libc::c_char,
    mode: *const libc::c_char,
) -> *mut libc::FILE {
    unsafe { libc::fopen(filename, mode) }
}
