//! Port of 'libhfst/src/HfstSymbolDefs.{h,cc}' — symbols, symbol pairs, and
//! sets of symbols.
//!
//! The container typedefs shared with 'HfstDataTypes.h' ('StringPair',
//! 'StringVector', 'StringPairVector', 'StringPairSet', 'HfstTwoLevelPath',
//! 'HfstTwoLevelPaths') are owned by ['crate::hfst_data_types'] and re-exported
//! here under their 'HfstSymbolDefs' spec ids.

use std::collections::{BTreeMap, BTreeSet};

use crate::hfst_flag_diacritics::FdOperation;

// [spec:hfst:def:hfst-symbol-defs.hfst.string]
pub type String = std::string::String;

// [spec:hfst:def:hfst-symbol-defs.hfst.string-set]
pub type StringSet = BTreeSet<String>;

// [spec:hfst:def:hfst-symbol-defs.hfst.string-vector]
pub use crate::hfst_data_types::StringVector;

// [spec:hfst:def:hfst-symbol-defs.hfst.string-pair]
pub use crate::hfst_data_types::StringPair;

// [spec:hfst:def:hfst-symbol-defs.hfst.string-pair-vector]
pub use crate::hfst_data_types::StringPairVector;

// [spec:hfst:def:hfst-symbol-defs.hfst.string-pair-set]
pub use crate::hfst_data_types::StringPairSet;

// [spec:hfst:def:hfst-symbol-defs.hfst.hfst-symbol-substitutions]
pub type HfstSymbolSubstitutions = BTreeMap<String, String>;

// [spec:hfst:def:hfst-symbol-defs.hfst.hfst-symbol-pair-substitutions]
pub type HfstSymbolPairSubstitutions = BTreeMap<StringPair, StringPair>;

// [spec:hfst:def:hfst-symbol-defs.hfst.hfst-two-level-path]
pub use crate::hfst_data_types::HfstTwoLevelPath;
// [spec:hfst:def:hfst-symbol-defs.hfst.hfst-two-level-paths]
pub use crate::hfst_data_types::HfstTwoLevelPaths;

// For internal use
// [spec:hfst:def:hfst-symbol-defs.hfst.number-pair]
pub type NumberPair = (u32, u32);
// [spec:hfst:def:hfst-symbol-defs.hfst.number-pair-vector]
pub type NumberPairVector = Vec<NumberPair>;
// [spec:hfst:def:hfst-symbol-defs.hfst.number-pair-set]
pub type NumberPairSet = BTreeSet<NumberPair>;
// [spec:hfst:def:hfst-symbol-defs.hfst.string-number-map]
pub type StringNumberMap = BTreeMap<String, u32>;
// [spec:hfst:def:hfst-symbol-defs.hfst.number-number-map]
pub type NumberNumberMap = BTreeMap<u32, u32>;

// Macros that can be used instead of hfst::internal_epsilon etc.
pub const INTERNAL_EPSILON: &str = "@_EPSILON_SYMBOL_@";
pub const INTERNAL_UNKNOWN: &str = "@_UNKNOWN_SYMBOL_@";
pub const INTERNAL_IDENTITY: &str = "@_IDENTITY_SYMBOL_@";
pub const INTERNAL_DEFAULT: &str = "@_DEFAULT_SYMBOL_@";

/* The internal representations */
pub const internal_epsilon: &str = "@_EPSILON_SYMBOL_@";
pub const internal_unknown: &str = "@_UNKNOWN_SYMBOL_@";
pub const internal_identity: &str = "@_IDENTITY_SYMBOL_@";
pub const internal_default: &str = "@_DEFAULT_SYMBOL_@";

// [spec:hfst:def:hfst-symbol-defs.hfst.is-epsilon-fn]
// [spec:hfst:sem:hfst-symbol-defs.hfst.is-epsilon-fn]
//
// The C++ 'const char *' overloads collapse onto these '&str' functions: both
// 'std::string == internal_epsilon' and 'std::string(str) == internal_epsilon'
// are the same equality test.
pub fn is_epsilon(str: &str) -> bool {
    str == internal_epsilon
}

// [spec:hfst:def:hfst-symbol-defs.hfst.is-unknown-fn]
// [spec:hfst:sem:hfst-symbol-defs.hfst.is-unknown-fn]
pub fn is_unknown(str: &str) -> bool {
    str == internal_unknown
}

// [spec:hfst:def:hfst-symbol-defs.hfst.is-identity-fn]
// [spec:hfst:sem:hfst-symbol-defs.hfst.is-identity-fn]
pub fn is_identity(str: &str) -> bool {
    str == internal_identity
}

// [spec:hfst:def:hfst-symbol-defs.hfst.is-default-fn]
// [spec:hfst:sem:hfst-symbol-defs.hfst.is-default-fn]
pub fn is_default(str: &str) -> bool {
    str == internal_default
}

pub mod symbols {
    use super::{
        FdOperation, HfstTwoLevelPath, HfstTwoLevelPaths, String, StringPairSet, StringPairVector,
        StringSet, StringVector,
    };

    // [spec:hfst:def:hfst-symbol-defs.hfst.symbols.collect-unknown-sets-fn]
    // [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.collect-unknown-sets-fn]
    pub fn collect_unknown_sets(
        s1: &StringSet,
        unknown1: &mut StringSet,
        s2: &StringSet,
        unknown2: &mut StringSet,
    ) {
        for it1 in s1.iter() {
            let sym1 = it1.clone();
            if !s2.contains(&sym1) {
                unknown2.insert(sym1);
            }
        }
        for it2 in s2.iter() {
            let sym2 = it2.clone();
            if !s1.contains(&sym2) {
                unknown1.insert(sym2);
            }
        }
    }

    // [spec:hfst:def:hfst-symbol-defs.hfst.symbols.std.string-to-string-fn]
    // [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.std.string-to-string-fn]
    //
    // 'to_string(const StringVector &, bool spaces=false)'. The default
    // argument is dropped (Rust has none); callers pass 'spaces' explicitly.
    pub fn to_string_string_vector(sv: &StringVector, spaces: bool) -> String {
        let mut result = String::new();
        for (i, s) in sv.iter().enumerate() {
            if spaces && i != 0 {
                result.push_str(" ");
            }
            result.push_str(s);
        }
        result
    }

    // [spec:hfst:def:hfst-symbol-defs.hfst.symbols.to-string-fn]
    // [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.to-string-fn]
    //
    // 'to_string(const StringPairVector &, bool spaces)'.
    pub fn to_string_string_pair_vector(spv: &StringPairVector, spaces: bool) -> String {
        let mut result = String::new();
        for (i, it) in spv.iter().enumerate() {
            if spaces && i != 0 {
                result.push_str(" ");
            }
            result.push_str(&it.0);
            if it.0 != it.1 {
                result.push_str(":");
                result.push_str(&it.1);
            }
        }
        result
    }

    // [spec:hfst:def:hfst-symbol-defs.hfst.symbols.to-string-pair-set-fn]
    // [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.to-string-pair-set-fn]
    pub fn to_string_pair_set(ss: &StringSet) -> StringPairSet {
        let mut result = StringPairSet::new();
        for it in ss.iter() {
            result.insert((it.clone(), it.clone()));
        }
        result
    }

    // [spec:hfst:def:hfst-symbol-defs.hfst.symbols.to-string-vector-fn]
    // [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.to-string-vector-fn]
    //
    // 'to_string_vector(const StringPairVector &, bool input_side)'.
    pub fn to_string_vector_from_string_pair_vector(
        spv: &StringPairVector,
        input_side: bool,
    ) -> StringVector {
        let mut result = StringVector::new();
        for it in spv.iter() {
            if input_side {
                result.push(it.0.clone());
            } else {
                result.push(it.1.clone());
            }
        }
        result
    }

    // [spec:hfst:def:hfst-symbol-defs.hfst.symbols.string-vector-to-string-vector-fn]
    // [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.string-vector-to-string-vector-fn]
    //
    // 'to_string_vector(const HfstTwoLevelPath & path)'.
    pub fn to_string_vector_from_two_level_path(path: &HfstTwoLevelPath) -> StringVector {
        let mut result = StringVector::new();
        let spv = path.second.clone();
        for it in spv.iter() {
            result.push(it.0.clone());
        }
        result
    }

    // [spec:hfst:def:hfst-symbol-defs.hfst.symbols.longest-path-length-fn]
    // [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.longest-path-length-fn]
    //
    // 'longest_path_length(const HfstTwoLevelPaths &, bool equally_long=false)'.
    pub fn longest_path_length(paths: &HfstTwoLevelPaths, equally_long: bool) -> i32 {
        if paths.len() == 0 {
            return -1;
        }
        if equally_long {
            return paths.iter().next().unwrap().second.len() as i32;
        }

        let mut max_path_length: u32 = 0;

        for it in paths.iter() {
            let length = it.second.len() as u32;
            max_path_length = if length > max_path_length {
                length
            } else {
                max_path_length
            };
        }
        max_path_length as i32
    }

    // [spec:hfst:def:hfst-symbol-defs.hfst.symbols.get-longest-paths-fn]
    // [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.get-longest-paths-fn]
    pub fn get_longest_paths(paths: &HfstTwoLevelPaths) -> HfstTwoLevelPaths {
        let mut result = HfstTwoLevelPaths::new();
        let mut max_path_length: u32 = 0;

        for it in paths.iter() {
            let length = it.second.len() as u32;
            max_path_length = if length > max_path_length {
                length
            } else {
                max_path_length
            };
        }

        for it in paths.iter() {
            let length = it.second.len() as u32;
            if length == max_path_length {
                result.insert(it.clone());
            }
        }

        result
    }

    // [spec:hfst:def:hfst-symbol-defs.hfst.symbols.hfst.hfst-two-level-paths-remove-flags-fn]
    // [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.hfst.hfst-two-level-paths-remove-flags-fn]
    //
    // 'remove_flags(const HfstTwoLevelPaths &)'.
    pub fn remove_flags_two_level_paths(paths: &HfstTwoLevelPaths) -> HfstTwoLevelPaths {
        let mut result = HfstTwoLevelPaths::new();

        for it in paths.iter() {
            result.insert(HfstTwoLevelPath {
                first: it.first,
                second: remove_flags_string_pair_vector(&it.second),
            });
        }
        result
    }

    // [spec:hfst:def:hfst-symbol-defs.hfst.symbols.hfst.hfst-two-level-path-remove-flags-fn]
    // [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.hfst.hfst-two-level-path-remove-flags-fn]
    //
    // 'remove_flags(const HfstTwoLevelPath &)'.
    pub fn remove_flags_two_level_path(path: &HfstTwoLevelPath) -> HfstTwoLevelPath {
        let spv = path.second.clone();
        let spv = remove_flags_string_pair_vector(&spv);
        HfstTwoLevelPath {
            first: path.first,
            second: spv,
        }
    }

    // [spec:hfst:def:hfst-symbol-defs.hfst.symbols.string-vector-remove-flags-fn]
    // [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.string-vector-remove-flags-fn]
    //
    // 'remove_flags(const StringVector &)'.
    pub fn remove_flags_string_vector(v: &StringVector) -> StringVector {
        let mut v_wo_flags = StringVector::new();
        for it in v.iter() {
            if !FdOperation::is_diacritic(it) {
                v_wo_flags.push(it.clone());
            }
        }
        v_wo_flags
    }

    // [spec:hfst:def:hfst-symbol-defs.hfst.symbols.remove-flags-fn]
    // [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.remove-flags-fn]
    //
    // 'remove_flags(const StringPairVector &)'.
    pub fn remove_flags_string_pair_vector(v: &StringPairVector) -> StringPairVector {
        let mut v_wo_flags = StringPairVector::new();
        for it in v.iter() {
            if !FdOperation::is_diacritic(&it.0) && !FdOperation::is_diacritic(&it.1) {
                v_wo_flags.push(it.clone());
            }
        }
        v_wo_flags
    }
}
