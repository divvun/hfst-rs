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

/// A transducer symbol (re-export; see ['crate::hfst_data_types::Symbol']).
pub use crate::hfst_data_types::Symbol;

// [spec:hfst:def:hfst-symbol-defs.hfst.string-set]
pub type StringSet = BTreeSet<Symbol>;

// [spec:hfst:def:hfst-symbol-defs.hfst.string-vector]
pub use crate::hfst_data_types::StringVector;

// [spec:hfst:def:hfst-symbol-defs.hfst.string-pair]
pub use crate::hfst_data_types::StringPair;

// [spec:hfst:def:hfst-symbol-defs.hfst.string-pair-vector]
pub use crate::hfst_data_types::StringPairVector;

// [spec:hfst:def:hfst-symbol-defs.hfst.string-pair-set]
pub use crate::hfst_data_types::StringPairSet;

// [spec:hfst:def:hfst-symbol-defs.hfst.hfst-symbol-substitutions]
pub type HfstSymbolSubstitutions = BTreeMap<Symbol, Symbol>;

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
pub type StringNumberMap = BTreeMap<Symbol, u32>;
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

// [spec:hfst:def:hfst-substitute.label-to-stringpair-fn]
// [spec:hfst:sem:hfst-substitute.label-to-stringpair-fn]
// [spec:hfst:def:hfst-insert-freely.label-to-stringpair-fn]
// [spec:hfst:sem:hfst-insert-freely.label-to-stringpair-fn]
/// Parse a transducer arc label `in:out` into its (input, output) pair,
/// honoring backslash-escaped colons (`\:` is a literal colon, not a separator)
/// and mapping the `@0@` epsilon marker to the internal epsilon symbol. Returns
/// `None` when the label has no genuine interior separator.
///
/// This is the 1:1 port of the byte-identical `label_to_stringpair()` carried by
/// both `tools/src/hfst-substitute.cc` and `tools/src/hfst-insert-freely.cc`.
/// NOTE: like the C++, a label whose only colon is an escaped colon at index 1
/// (e.g. `\:x`) loops forever — the C++ `if (colon > label+1)` has no `else`, so
/// `colon` is never advanced. The behaviour is preserved verbatim.
pub fn label_to_stringpair(label: &str) -> Option<StringPair> {
    let bytes = label.as_bytes();
    let len = bytes.len();
    // Byte index of the candidate separating colon (`strchr(label, ':')`);
    // `None` models the C `NULL`.
    let find_colon_from = |start: usize| -> Option<usize> {
        bytes[start..]
            .iter()
            .position(|&b| b == b':')
            .map(|i| start + i)
    };
    let mut colon: Option<usize> = find_colon_from(0);
    while let Some(c) = colon {
        if c == 0 {
            // colon == label
            colon = find_colon_from(c + 1);
        } else if c == len - 1 {
            // colon == endstr - 1
            colon = None;
        } else if bytes[c - 1] == b'\\' {
            if c > 1 {
                // colon > label + 1
                if bytes[c - 2] == b'\\' {
                    break;
                } else {
                    colon = find_colon_from(c + 1);
                }
            }
            // (When c == 1 the C code leaves 'colon' unchanged; preserved here.)
        } else {
            break;
        }
    }
    let (mut first, mut second): (Symbol, Symbol) = match colon {
        // (label < colon) && (colon < endstr): a real, interior separator.
        Some(c) if c > 0 && c < len => (label[0..c].into(), label[c + 1..len].into()),
        _ => return None,
    };
    if first == "@0@" {
        first = Symbol::new_static(internal_epsilon);
    }
    if second == "@0@" {
        second = Symbol::new_static(internal_epsilon);
    }
    Some((first, second))
}

pub mod symbols {
    use super::{
        FdOperation, HfstTwoLevelPath, HfstTwoLevelPaths, String, StringPairSet, StringPairVector,
        StringSet, StringVector,
    };

    // [spec:hfst:def:hfst-symbol-defs.hfst.symbols.collect-unknown-sets-fn]
    // [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.collect-unknown-sets-fn]
    // Returns (unknown1, unknown2): the symbols of s2 missing from s1 and vice
    // versa.
    pub fn collect_unknown_sets(s1: &StringSet, s2: &StringSet) -> (StringSet, StringSet) {
        let mut unknown1 = StringSet::new();
        let mut unknown2 = StringSet::new();
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
        (unknown1, unknown2)
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

    // [spec:hfst:def:hfst-symbol-defs.hfst.symbols.string-pair-set-to-string-pair-set-fn]
    // [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.string-pair-set-to-string-pair-set-fn]
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
            return paths
                .iter()
                .next()
                .expect("paths is non-empty; the empty case returned above")
                .second
                .len() as i32;
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

    // [spec:hfst:def:hfst-symbol-defs.hfst.symbols.hfst.hfst-two-level-paths-get-longest-paths-fn]
    // [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.hfst.hfst-two-level-paths-get-longest-paths-fn]
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

    // [spec:hfst:def:hfst-symbol-defs.hfst.symbols.string-pair-vector-remove-flags-fn]
    // [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.string-pair-vector-remove-flags-fn]
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
