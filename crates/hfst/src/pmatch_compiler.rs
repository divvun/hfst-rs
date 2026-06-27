//! 'pmatch_compiler' — 1:1 port of 'libhfst/src/parsers/pmatch_utils.{h,cc}',
//! the PMATCH compiler (the 'PmatchObject' lazy-evaluation AST + every
//! 'evaluate()' + every free function in 'namespace hfst::pmatch').
//!
//! The runtime matcher ('pmatch.cc') is ported separately in ['crate::pmatch']
//! and is NOT part of this module.
//!
//! Faithfulness over idiom: C++ identifiers are kept verbatim (Rust casing),
//! bugs are preserved, and 'unsafe'/raw pointers mirror the C++ 'PmatchObject*'
//! hierarchy and 'hfst::pmatch' namespace globals. The ONE sanctioned
//! structural deviation is that the bison tree construction is replaced by a
//! walk over the 'nfst-pmatch' parse-only AST (see ['build_object'],
//! ['build_statement'], ['PmatchCompiler']).

#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(clippy::too_many_arguments)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_data_types::StringPairSet;
use crate::hfst_data_types::double_to_float;
use crate::hfst_data_types::{ImplementationType, StringPair, StringVector};
use crate::hfst_symbol_defs::StringSet;
use crate::hfst_symbol_defs::{
    internal_default, internal_epsilon, internal_identity, internal_unknown,
};
use crate::hfst_tokenizer::HfstTokenizer;
use crate::hfst_transducer::HfstTransducer;
use crate::hfst_transducer::{HfstTransducerPair, HfstTransducerPairVector};
use crate::hfst_xerox_rules::{ReplaceArrow, ReplaceType};
use crate::hfst_xerox_rules::{
    Rule, create_mapping_for_mark_up_replace, replace_left_rule, replace_left_rule_vector,
    replace_leftmost_longest_match_rule, replace_leftmost_longest_match_rule_vector,
    replace_leftmost_shortest_match_rule, replace_leftmost_shortest_match_rule_vector,
    replace_rightmost_longest_match_rule, replace_rightmost_longest_match_rule_vector,
    replace_rightmost_shortest_match_rule, replace_rightmost_shortest_match_rule_vector,
    replace_rule, replace_rule_vector, restriction,
};
use crate::pmatch::PmatchAlphabet;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::Write as _;
use std::os::raw::c_char;

// ---------------------------------------------------------------------------
// Primitive typedefs
// ---------------------------------------------------------------------------

/// Mirror of C 'clock_t' (used for the verbose-mode compilation timers).
pub type clock_t = i64;

/// Mirror of C 'CLOCKS_PER_SEC' for ['clock'].
pub const CLOCKS_PER_SEC: clock_t = 1_000_000;

// [spec:hfst:def:pmatch-utils.hfst.pmatch.word-vec-float]
pub type WordVecFloat = f32;

// [spec:hfst:def:pmatch-utils.hfst.pmatch.transducer-pointer-pair]
pub type TransducerPointerPair = (*mut HfstTransducer, *mut HfstTransducer);

// [spec:hfst:def:pmatch-utils.hfst.pmatch.mapping-pair-vector]
pub type MappingPairVector = Vec<*mut dyn PmatchObjectPairBase>;

/// Mirror of C 'clock()' — processor time in ['CLOCKS_PER_SEC'] ticks. The
/// skeleton uses wall-clock microseconds, which is only consulted in verbose
/// timing output.
pub fn clock() -> clock_t {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_micros() as clock_t,
        Err(_) => 0,
    }
}

// ---------------------------------------------------------------------------
// Special symbol constants
// ---------------------------------------------------------------------------

pub const RC_ENTRY_SYMBOL: &str = "@PMATCH_RC_ENTRY@";

pub const RC_EXIT_SYMBOL: &str = "@PMATCH_RC_EXIT@";

pub const LC_ENTRY_SYMBOL: &str = "@PMATCH_LC_ENTRY@";

pub const LC_EXIT_SYMBOL: &str = "@PMATCH_LC_EXIT@";

pub const NRC_ENTRY_SYMBOL: &str = "@PMATCH_NRC_ENTRY@";

pub const NRC_EXIT_SYMBOL: &str = "@PMATCH_NRC_EXIT@";

pub const NLC_ENTRY_SYMBOL: &str = "@PMATCH_NLC_ENTRY@";

pub const NLC_EXIT_SYMBOL: &str = "@PMATCH_NLC_EXIT@";

pub const PASSTHROUGH_SYMBOL: &str = "@PMATCH_PASSTHROUGH@";

pub const BOUNDARY_SYMBOL: &str = "@BOUNDARY@";

pub const ENTRY_SYMBOL: &str = "@PMATCH_ENTRY@";

pub const EXIT_SYMBOL: &str = "@PMATCH_EXIT@";

// ---------------------------------------------------------------------------
// Static latin-1 character class tables (used by PmatchUtilityTransducers)
// ---------------------------------------------------------------------------

// It is assumed that latin1_upper and latin1_lower have the same length!
pub static latin1_upper: &[&str] = &[
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S",
    "T", "U", "V", "W", "X", "Y", "Z", "À", "Á", "Â", "Ã", "Ä", "Å", "Æ", "Ç", "È", "É", "Ê", "Ë",
    "Ì", "Í", "Î", "Ï", "Ð", "Ñ", "Ò", "Ó", "Ô", "Õ", "Ö", "Ø", "Ù", "Ú", "Û", "Ü", "Ý", "Þ", "ẞ",
];

pub static latin1_lower: &[&str] = &[
    "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r", "s",
    "t", "u", "v", "w", "x", "y", "z", "à", "á", "â", "ã", "ä", "å", "æ", "ç", "è", "é", "ê", "ë",
    "ì", "í", "î", "ï", "ð", "ñ", "ò", "ó", "ô", "õ", "ö", "ø", "ù", "ú", "û", "ü", "ý", "þ", "ß",
];

pub static combining_accents: &[&str] = &[
    // Combining accents: grave, acute, circumflex, tilde, overline,
    // diaresis, charon, cedilla
    "\u{0300}", "\u{0301}", "\u{0302}", "\u{0303}", "\u{0305}", "\u{0308}", "\u{030C}", "\u{0327}",
    // Small solidus and large combining solidus
    "\u{0337}", "\u{0338}",
];

pub static latin1_punct: &[&str] = &[
    "!", "\"", "#", "$", "%", "&", "'", "(", ")", "*", "+", ",", "-", ".", "/", ":", ";", "<", "=",
    ">", "?", "@", "[", "\\", "]", "^", "_", "{", "|", "}", "~", "`", "´", "¡", "«", "»", "¿",
];

pub static latin1_whitespace: &[&str] = &[
    " ", "\n", "\t", // Non-breaking space, CR
    "\u{00A0}", "\r", // punctuation space, thin space, line separator, par separator
    "\u{2008}", "\u{2009}", "\u{2028}", "\u{2029}",
];

// ---------------------------------------------------------------------------
// Enums (argument tags for the operation node structs)
// ---------------------------------------------------------------------------

// These are used as arguments for casing functions
// [spec:hfst:def:pmatch-utils.hfst.pmatch.side]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Both,
    Upper,
    Lower,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-op]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PmatchUnaryOp {
    AddDelimiters,
    Optionalize,
    RepeatStar,
    RepeatPlus,
    Reverse,
    Invert,
    InputProject,
    OutputProject,
    Complement,
    Containment,
    ContainmentOnce,
    ContainmentOptional,
    TermComplement,
    Cap,
    OptCap,
    ToLower,
    ToUpper,
    OptToLower,
    OptToUpper,
    AnyCase,
    CapUpper,
    OptCapUpper,
    ToLowerUpper,
    ToUpperUpper,
    OptToLowerUpper,
    OptToUpperUpper,
    AnyCaseUpper,
    CapLower,
    OptCapLower,
    ToLowerLower,
    ToUpperLower,
    OptToLowerLower,
    OptToUpperLower,
    AnyCaseLower,
    MakeSigma,
    MakeList,
    MakeExcList,
    LC,
    NLC,
    RC,
    NRC,
    Explode,
    Implode,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-op]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PmatchBinaryOp {
    Concatenate,
    Compose,
    CrossProduct,
    LenientCompose,
    Disjunct,
    Intersect,
    Subtract,
    UpperSubtract,
    LowerSubtract,
    UpperPriorityUnion,
    LowerPriorityUnion,
    Shuffle,
    Before,
    After,
    InsertFreely,
    IgnoreInternally,
    Merge,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-ternary-op]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PmatchTernaryOp {
    Substitute,
    Uncompose,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-numeric-op]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PmatchNumericOp {
    RepeatN,
    RepeatNPlus,
    RepeatNMinus,
    RepeatNToK,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-predefined]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PmatchPredefined {
    Alpha,
    UppercaseAlpha,
    LowercaseAlpha,
    Numeral,
    Punctuation,
    Whitespace,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-builtin]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PmatchBuiltin {
    Interpolate,
}

// ---------------------------------------------------------------------------
// WordVector
// ---------------------------------------------------------------------------

// [spec:hfst:def:pmatch-utils.hfst.pmatch.word-vector]
#[derive(Clone, Default)]
pub struct WordVector {
    pub word: String,
    pub vector: Vec<WordVecFloat>,
    pub norm: WordVecFloat,
}

// ---------------------------------------------------------------------------
// PmatchObject trait (the abstract base of the lazy-evaluation AST)
// ---------------------------------------------------------------------------

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object]
//
// The C++ 'struct PmatchObject' has base fields 'name'/'weight'/'line_defined'/
// 'my_timer'/'cache' carried (verbatim) on every node struct below; the trait
// exposes them through accessor methods (get_name/set_name/.../get_cache/
// set_cache) implemented on each node struct.
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.pmatch-object-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.pmatch-object-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch-object.pmatch-object-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.pmatch-object-fn]
pub trait PmatchObject {
    // --- base field accessors (required) -----------------------------------
    fn get_name(&self) -> &str {
        ""
    }
    fn set_name(&mut self, name: String) {}
    fn get_weight(&self) -> f64 {
        0.0
    }
    fn set_weight(&mut self, weight: f64) {}
    fn get_line_defined(&self) -> i32 {
        0
    }
    fn set_line_defined(&mut self, line_defined: i32) {}
    fn get_my_timer(&self) -> clock_t {
        0
    }
    fn set_my_timer(&mut self, my_timer: clock_t) {}
    fn get_cache(&self) -> *mut HfstTransducer {
        std::ptr::null_mut()
    }
    fn set_cache(&mut self, cache: *mut HfstTransducer) {}

    // --- shared (non-virtual in C++) timing/cache helpers ------------------
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.start-timing-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.start-timing-fn]
    fn start_timing(&mut self) {
        unsafe {
            if verbose && self.get_name() != "" {
                self.set_my_timer(clock());
                named_object_evaluation_stack_depth += 1;
                write_compilation_stack_indentation_to_err();
                eprint!("Compiling {}...\n", self.get_name());
            }
        }
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.report-time-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.report-time-fn]
    fn report_time(&self, extra_info: String) {
        unsafe {
            if verbose && self.get_name() != "" {
                let duration = (clock() - self.get_my_timer()) as f64 / CLOCKS_PER_SEC as f64;
                write_compilation_stack_indentation_to_err();
                eprintln!(
                    "{} compiled in {} seconds{}",
                    self.get_name(),
                    duration,
                    extra_info
                );
                named_object_evaluation_stack_depth -= 1;
            }
        }
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.report-cache-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.report-cache-fn]
    fn report_cache(&self, extra_info: String) {
        unsafe {
            if verbose && self.get_name() != "TOP" {
                named_object_evaluation_stack_depth += 1;
                write_compilation_stack_indentation_to_err();
                eprintln!("{} fetched from cache{}", self.get_name(), extra_info);
                named_object_evaluation_stack_depth -= 1;
            }
        }
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.should-use-cache-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.should-use-cache-fn]
    fn should_use_cache(&self) -> bool {
        self.get_name() != "" && call_stack().len() == 0
    }

    // --- virtual graph-walk / query methods (base defaults) ----------------
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.is-unweighted-disjunction-of-strings-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.is-unweighted-disjunction-of-strings-fn]
    fn is_unweighted_disjunction_of_strings(&mut self) -> bool {
        false
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.collect-strings-into-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.collect-strings-into-fn]
    fn collect_strings_into(&mut self, strings: &mut StringVector) {}
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.collect-initial-symbols-into-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.collect-initial-symbols-into-fn]
    fn collect_initial_symbols_into(
        &mut self,
        allowed: &mut StringSet,
        disallowed: &mut StringSet,
    ) {
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.get-real-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.get-real-initial-symbols-fn]
    fn get_real_initial_symbols(&mut self) -> StringSet {
        StringSet::new()
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.get-real-initial-symbols-from-right-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.get-real-initial-symbols-from-right-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-object.get-real-initial-symbols-from-right-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.get-real-initial-symbols-from-right-fn]
    fn get_real_initial_symbols_from_right(&mut self) -> StringSet {
        StringSet::new()
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.is-left-concatenation-with-context-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.is-left-concatenation-with-context-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-object.is-left-concatenation-with-context-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.is-left-concatenation-with-context-fn]
    fn is_left_concatenation_with_context(&mut self) -> bool {
        false
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.is-context-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.is-context-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-object.is-context-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.is-context-fn]
    fn is_context(&mut self) -> bool {
        false
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.is-delimiter-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.is-delimiter-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-object.is-delimiter-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.is-delimiter-fn]
    fn is_delimiter(&mut self) -> bool {
        false
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.get-initial-symbols-from-unary-root-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.get-initial-symbols-from-unary-root-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-object.get-initial-symbols-from-unary-root-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.get-initial-symbols-from-unary-root-fn]
    fn get_initial_symbols_from_unary_root(&mut self) -> StringSet {
        StringSet::new()
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.get-initial-rc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.get-initial-rc-initial-symbols-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-object.get-initial-rc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.get-initial-rc-initial-symbols-fn]
    fn get_initial_RC_initial_symbols(&mut self) -> StringSet {
        StringSet::new()
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.get-initial-nrc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.get-initial-nrc-initial-symbols-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-object.get-initial-nrc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.get-initial-nrc-initial-symbols-fn]
    fn get_initial_NRC_initial_symbols(&mut self) -> StringSet {
        StringSet::new()
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.expand-ins-arcs-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.expand-ins-arcs-fn]
    fn expand_Ins_arcs(&mut self, ss: &mut StringSet) {}
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.evaluate-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.evaluate-fn]
    unsafe fn evaluate(&mut self) -> *mut HfstTransducer;
    /// The C++ overload 'evaluate(std::vector<PmatchObject*> args)' (base
    /// default).
    unsafe fn evaluate_args(&mut self, args: Vec<*mut dyn PmatchObject>) -> *mut HfstTransducer {
        self.evaluate()
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.evaluate-as-arg-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.evaluate-as-arg-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-object.evaluate-as-arg-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.evaluate-as-arg-fn]
    unsafe fn evaluate_as_arg(&mut self) -> *mut dyn PmatchObject {
        panic!("evaluate_as_arg called on a PmatchObject that does not support it")
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.as-string-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.as-string-fn]
    fn as_string(&mut self) -> String {
        String::new()
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.as-string-pair-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.as-string-pair-fn]
    fn as_string_pair(&mut self) -> StringPair {
        (String::new(), String::new())
    }
}

// ---------------------------------------------------------------------------
// PmatchObject node structs
// ---------------------------------------------------------------------------

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-symbol]
pub struct PmatchSymbol {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub my_timer: clock_t,
    pub cache: *mut HfstTransducer,
    // This handles argumentless function calls and definition invocations,
    // which are the same thing under the hood.
    pub sym: String,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string]
pub struct PmatchString {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub my_timer: clock_t,
    pub cache: *mut HfstTransducer,
    pub string: String,
    pub multichar: bool,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-question-mark]
pub struct PmatchQuestionMark {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub my_timer: clock_t,
    pub cache: *mut HfstTransducer,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-numeric-operation]
pub struct PmatchNumericOperation {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub my_timer: clock_t,
    pub cache: *mut HfstTransducer,
    pub op: PmatchNumericOp,
    pub root: *mut dyn PmatchObject,
    pub values: Vec<i32>,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation]
pub struct PmatchUnaryOperation {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub my_timer: clock_t,
    pub cache: *mut HfstTransducer,
    pub op: PmatchUnaryOp,
    pub root: *mut dyn PmatchObject,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation]
pub struct PmatchBinaryOperation {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub my_timer: clock_t,
    pub cache: *mut HfstTransducer,
    pub op: PmatchBinaryOp,
    pub left: *mut dyn PmatchObject,
    pub right: *mut dyn PmatchObject,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-ternary-operation]
pub struct PmatchTernaryOperation {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub my_timer: clock_t,
    pub cache: *mut HfstTransducer,
    pub op: PmatchTernaryOp,
    pub left: *mut dyn PmatchObject,
    pub middle: *mut dyn PmatchObject,
    pub right: *mut dyn PmatchObject,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-transducer-container]
pub struct PmatchTransducerContainer {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub my_timer: clock_t,
    pub cache: *mut HfstTransducer,
    pub t: *mut HfstTransducer,
}

impl PmatchTransducerContainer {
    pub fn new(t: *mut HfstTransducer) -> *mut PmatchTransducerContainer {
        Box::into_raw(Box::new(PmatchTransducerContainer {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            my_timer: 0,
            cache: std::ptr::null_mut(),
            t,
        }))
    }
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-function]
pub struct PmatchFunction {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub my_timer: clock_t,
    pub cache: *mut HfstTransducer,
    pub args: Vec<String>,
    pub root: *mut dyn PmatchObject,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-funcall]
pub struct PmatchFuncall {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub my_timer: clock_t,
    pub cache: *mut HfstTransducer,
    pub args: *mut Vec<*mut dyn PmatchObject>,
    pub fun: *mut PmatchFunction,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-builtin-function]
pub struct PmatchBuiltinFunction {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub my_timer: clock_t,
    pub cache: *mut HfstTransducer,
    pub args: *mut Vec<*mut dyn PmatchObject>,
    pub type_: PmatchBuiltin,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-epsilon-arc]
pub struct PmatchEpsilonArc {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub my_timer: clock_t,
    pub cache: *mut HfstTransducer,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-empty]
pub struct PmatchEmpty {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub my_timer: clock_t,
    pub cache: *mut HfstTransducer,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-acceptor]
pub struct PmatchAcceptor {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub my_timer: clock_t,
    pub cache: *mut HfstTransducer,
    pub set: PmatchPredefined,
}

// ---------------------------------------------------------------------------
// PmatchObjectPair hierarchy (NOT PmatchObject subclasses; carries the virtual
// evaluate_pair()). The 'PmatchObjectPairBase' trait preserves the C++ virtual
// dispatch between 'PmatchObjectPair' and 'PmatchMarkupContainer'.
// ---------------------------------------------------------------------------

pub trait PmatchObjectPairBase {
    fn get_left(&self) -> *mut dyn PmatchObject {
        std::ptr::null_mut::<PmatchSymbol>() as *mut dyn PmatchObject
    }
    fn set_left(&mut self, l: *mut dyn PmatchObject) {}
    fn get_right(&self) -> *mut dyn PmatchObject {
        std::ptr::null_mut::<PmatchSymbol>() as *mut dyn PmatchObject
    }
    fn set_right(&mut self, r: *mut dyn PmatchObject) {}
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object-pair.evaluate-pair-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object-pair.evaluate-pair-fn]
    unsafe fn evaluate_pair(&mut self) -> TransducerPointerPair {
        panic!("evaluate_pair called on a PmatchObject that is not a pair")
    }
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object-pair]
pub struct PmatchObjectPair {
    pub left: *mut dyn PmatchObject,
    pub right: *mut dyn PmatchObject,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-markup-container]
pub struct PmatchMarkupContainer {
    pub left: *mut dyn PmatchObject,
    pub right: *mut dyn PmatchObject,
    pub left_of_arrow: *mut dyn PmatchObject,
}

// ---------------------------------------------------------------------------
// Replace-rule / restriction container nodes (PmatchObject subclasses)
// ---------------------------------------------------------------------------

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-restriction-container]
pub struct PmatchRestrictionContainer {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub my_timer: clock_t,
    pub cache: *mut HfstTransducer,
    pub left: *mut dyn PmatchObject,
    pub contexts: *mut MappingPairVector,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-mapping-pairs-container]
pub struct PmatchMappingPairsContainer {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub my_timer: clock_t,
    pub cache: *mut HfstTransducer,
    pub arrow: ReplaceArrow,
    pub mapping_pairs: MappingPairVector,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-contexts-container]
pub struct PmatchContextsContainer {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub my_timer: clock_t,
    pub cache: *mut HfstTransducer,
    pub type_: ReplaceType,
    pub context_pairs: MappingPairVector,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-replace-rule-container]
pub struct PmatchReplaceRuleContainer {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub my_timer: clock_t,
    pub cache: *mut HfstTransducer,
    pub arrow: ReplaceArrow,
    pub type_: ReplaceType,
    pub mapping: MappingPairVector,
    pub context: MappingPairVector,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-parallel-rules-container]
pub struct PmatchParallelRulesContainer {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub my_timer: clock_t,
    pub cache: *mut HfstTransducer,
    pub arrow: ReplaceArrow,
    pub rules: Vec<*mut PmatchReplaceRuleContainer>,
}

// ---------------------------------------------------------------------------
// PmatchUtilityTransducers (cached character-class acceptors)
// ---------------------------------------------------------------------------

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers]
pub struct PmatchUtilityTransducers {
    // Character class acceptors
    pub latin1_acceptor: *const HfstTransducer,
    pub latin1_alpha_acceptor: *const HfstTransducer,
    pub latin1_lowercase_acceptor: *const HfstTransducer,
    pub latin1_uppercase_acceptor: *const HfstTransducer,
    pub combining_accent_acceptor: *const HfstTransducer,
    pub latin1_numeral_acceptor: *const HfstTransducer,
    pub latin1_punct_acceptor: *const HfstTransducer,
    pub latin1_whitespace_acceptor: *const HfstTransducer,
    pub capify: *const HfstTransducer,
    pub lowerfy: *const HfstTransducer,
}

// ---------------------------------------------------------------------------
// Global state (mirroring the 'hfst::pmatch' namespace globals).
//
// Collection globals are 'static mut' plus an accessor returning a
// '&'static mut' (built through 'addr_of_mut!', which avoids the edition-2024
// 'static_mut_refs' hard error). Scalar/pointer globals are read/written
// directly inside 'unsafe' blocks, exactly like the C++ namespace globals.
// ---------------------------------------------------------------------------

pub static mut data: *mut c_char = std::ptr::null_mut();

pub static mut startptr: *mut c_char = std::ptr::null_mut();

pub static mut len: usize = 0;

pub static mut format: ImplementationType = ImplementationType::TROPICAL_OPENFST_TYPE;

pub static mut verbose: bool = false;

pub static mut flatten: bool = false;

pub static mut include_cosine_distances: bool = false;

pub static mut timer: clock_t = 0;

pub static mut minimization_guard_count: i32 = 0;

pub static mut named_object_evaluation_stack_depth: i32 = 0;

pub static mut need_delimiters: bool = false;

pub static mut vector_similarity_projection_factor: WordVecFloat = 0.0;

pub static mut utils: *mut PmatchUtilityTransducers = std::ptr::null_mut();

static mut DEFINITIONS: BTreeMap<String, *mut dyn PmatchObject> = BTreeMap::new();

static mut VARIABLES: BTreeMap<String, String> = BTreeMap::new();

static mut CALL_STACK: Vec<BTreeMap<String, *mut dyn PmatchObject>> = Vec::new();

static mut EVAL_STACK: Vec<String> = Vec::new();

static mut DEF_INSED_EXPRESSIONS: BTreeMap<String, *mut dyn PmatchObject> = BTreeMap::new();

static mut INSERTED_NAMES: BTreeSet<String> = BTreeSet::new();

static mut UNCOMPOSED: BTreeSet<String> = BTreeSet::new();

static mut UNSATISFIED_INSERTIONS: BTreeSet<String> = BTreeSet::new();

static mut USED_DEFINITIONS: BTreeSet<String> = BTreeSet::new();

static mut FUNCTION_NAMES: BTreeSet<String> = BTreeSet::new();

static mut CAPTURE_NAMES: BTreeSet<String> = BTreeSet::new();

static mut WORD_VECTORS: Vec<WordVector> = Vec::new();

static mut NAMED_TRANSDUCERS: BTreeMap<String, HfstTransducer> = BTreeMap::new();

static mut INCLUDEDIR: String = String::new();

// Maps a compiled Lst() symbol (@L....@) to the pmatch source line where the
// Lst() was defined.
static mut LST_LINE_MAP: BTreeMap<String, i32> = BTreeMap::new();

// De-duplicate overlap warnings: the same Lst() can get checked multiple times
// (e.g. from both sides of a disjunction).
static mut LST_OVERLAP_WARNED: BTreeSet<String> = BTreeSet::new();

pub fn definitions() -> &'static mut BTreeMap<String, *mut dyn PmatchObject> {
    unsafe { &mut *std::ptr::addr_of_mut!(DEFINITIONS) }
}

pub fn variables() -> &'static mut BTreeMap<String, String> {
    unsafe { &mut *std::ptr::addr_of_mut!(VARIABLES) }
}

pub fn call_stack() -> &'static mut Vec<BTreeMap<String, *mut dyn PmatchObject>> {
    unsafe { &mut *std::ptr::addr_of_mut!(CALL_STACK) }
}

pub fn eval_stack() -> &'static mut Vec<String> {
    unsafe { &mut *std::ptr::addr_of_mut!(EVAL_STACK) }
}

pub fn def_insed_expressions() -> &'static mut BTreeMap<String, *mut dyn PmatchObject> {
    unsafe { &mut *std::ptr::addr_of_mut!(DEF_INSED_EXPRESSIONS) }
}

pub fn inserted_names() -> &'static mut BTreeSet<String> {
    unsafe { &mut *std::ptr::addr_of_mut!(INSERTED_NAMES) }
}

pub fn uncomposed() -> &'static mut BTreeSet<String> {
    unsafe { &mut *std::ptr::addr_of_mut!(UNCOMPOSED) }
}

pub fn unsatisfied_insertions() -> &'static mut BTreeSet<String> {
    unsafe { &mut *std::ptr::addr_of_mut!(UNSATISFIED_INSERTIONS) }
}

pub fn used_definitions() -> &'static mut BTreeSet<String> {
    unsafe { &mut *std::ptr::addr_of_mut!(USED_DEFINITIONS) }
}

pub fn function_names() -> &'static mut BTreeSet<String> {
    unsafe { &mut *std::ptr::addr_of_mut!(FUNCTION_NAMES) }
}

pub fn capture_names() -> &'static mut BTreeSet<String> {
    unsafe { &mut *std::ptr::addr_of_mut!(CAPTURE_NAMES) }
}

pub fn word_vectors() -> &'static mut Vec<WordVector> {
    unsafe { &mut *std::ptr::addr_of_mut!(WORD_VECTORS) }
}

pub fn named_transducers() -> &'static mut BTreeMap<String, HfstTransducer> {
    unsafe { &mut *std::ptr::addr_of_mut!(NAMED_TRANSDUCERS) }
}

pub fn includedir() -> &'static mut String {
    unsafe { &mut *std::ptr::addr_of_mut!(INCLUDEDIR) }
}

pub fn lst_line_map() -> &'static mut BTreeMap<String, i32> {
    unsafe { &mut *std::ptr::addr_of_mut!(LST_LINE_MAP) }
}

pub fn lst_overlap_warned() -> &'static mut BTreeSet<String> {
    unsafe { &mut *std::ptr::addr_of_mut!(LST_OVERLAP_WARNED) }
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.add-to-pmatch-symbols-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.add-to-pmatch-symbols-fn]
pub fn add_to_pmatch_symbols(symbols: StringSet) {
    // Declared in pmatch_utils.h but never defined in pmatch_utils.cc.
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.getinput-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.getinput-fn]
pub unsafe fn getinput(buf: *mut c_char, maxlen: i32) -> i32 {
    // Flex/bison input callback; superseded by the nfst-pmatch parser.
    0
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.get-rc-transition-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-rc-transition-fn]
pub unsafe fn get_RC_transition(s: *const c_char) -> String {
    // Declared in pmatch_utils.h but never defined in pmatch_utils.cc.
    String::new()
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.get-lc-transition-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-lc-transition-fn]
pub unsafe fn get_LC_transition(s: *const c_char) -> String {
    // Declared in pmatch_utils.h but never defined in pmatch_utils.cc.
    String::new()
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.acceptor-from-cstr-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.acceptor-from-cstr-fn]
pub fn acceptor_from_cstr(strings: &[&str], type_: ImplementationType) -> *mut HfstTransducer {
    unsafe {
        let retval: *mut HfstTransducer = Box::into_raw(Box::new(HfstTransducer::new_type(type_)));
        let mut i = 0;
        while i < array_len(strings) {
            let tmp = HfstTransducer::new_symbol(strings[i], type_);
            (*retval).disjunct(&tmp, true);
            i += 1;
        }
        (*retval).minimize();
        retval
    }
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.array-len-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.array-len-fn]
pub fn array_len(strings: &[&str]) -> usize {
    strings.len()
}

/// Facade mirroring the C++ 'hfst::pmatch::compile': construct the
/// 'PmatchObject' definitions + TOP from a pmatch source string and return the
/// evaluated transducers ('map<string, HfstTransducer*>' in C++).
pub struct PmatchCompiler {
    pub type_: ImplementationType,
    pub verbose: bool,
    pub flatten: bool,
    pub include_cosine_distances: bool,
    pub includedir: String,
}

// ===== body: utility-transducers =====
#[allow(unsafe_op_in_unsafe_fn)]
impl PmatchUtilityTransducers {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.pmatch-utility-transducers-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.pmatch-utility-transducers-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.pmatch-utility-transducers-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.pmatch-utility-transducers-fn]
    pub unsafe fn new() -> PmatchUtilityTransducers {
        let mut retval = PmatchUtilityTransducers {
            latin1_acceptor: std::ptr::null(),
            latin1_alpha_acceptor: std::ptr::null(),
            latin1_lowercase_acceptor: std::ptr::null(),
            latin1_uppercase_acceptor: std::ptr::null(),
            combining_accent_acceptor: std::ptr::null(),
            latin1_numeral_acceptor: std::ptr::null(),
            latin1_punct_acceptor: std::ptr::null(),
            latin1_whitespace_acceptor: std::ptr::null(),
            capify: std::ptr::null(),
            lowerfy: std::ptr::null(),
        };
        retval.latin1_acceptor = PmatchUtilityTransducers::make_latin1_acceptor(
            ImplementationType::TROPICAL_OPENFST_TYPE,
        );
        retval.latin1_alpha_acceptor = PmatchUtilityTransducers::make_latin1_alpha_acceptor(
            ImplementationType::TROPICAL_OPENFST_TYPE,
        );
        retval.latin1_lowercase_acceptor = PmatchUtilityTransducers::make_latin1_lowercase_acceptor(
            ImplementationType::TROPICAL_OPENFST_TYPE,
        );
        retval.latin1_uppercase_acceptor = PmatchUtilityTransducers::make_latin1_uppercase_acceptor(
            ImplementationType::TROPICAL_OPENFST_TYPE,
        );
        retval.combining_accent_acceptor = PmatchUtilityTransducers::make_combining_accent_acceptor(
            ImplementationType::TROPICAL_OPENFST_TYPE,
        );
        retval.latin1_numeral_acceptor = PmatchUtilityTransducers::make_latin1_numeral_acceptor(
            ImplementationType::TROPICAL_OPENFST_TYPE,
        );
        retval.latin1_punct_acceptor = PmatchUtilityTransducers::make_latin1_punct_acceptor(
            ImplementationType::TROPICAL_OPENFST_TYPE,
        );
        retval.latin1_whitespace_acceptor =
            PmatchUtilityTransducers::make_latin1_whitespace_acceptor(
                ImplementationType::TROPICAL_OPENFST_TYPE,
            );
        retval.lowerfy = retval.make_lowerfy(ImplementationType::TROPICAL_OPENFST_TYPE);
        retval.capify = retval.make_capify(ImplementationType::TROPICAL_OPENFST_TYPE);
        retval
    }

    pub unsafe fn drop_in_place(&mut self) {
        drop(Box::from_raw(self.latin1_acceptor as *mut HfstTransducer));
        drop(Box::from_raw(
            self.latin1_alpha_acceptor as *mut HfstTransducer,
        ));
        drop(Box::from_raw(
            self.latin1_lowercase_acceptor as *mut HfstTransducer,
        ));
        drop(Box::from_raw(
            self.latin1_uppercase_acceptor as *mut HfstTransducer,
        ));
        drop(Box::from_raw(
            self.combining_accent_acceptor as *mut HfstTransducer,
        ));
        drop(Box::from_raw(
            self.latin1_numeral_acceptor as *mut HfstTransducer,
        ));
        drop(Box::from_raw(
            self.latin1_punct_acceptor as *mut HfstTransducer,
        ));
        drop(Box::from_raw(
            self.latin1_whitespace_acceptor as *mut HfstTransducer,
        ));
        drop(Box::from_raw(self.lowerfy as *mut HfstTransducer));
        drop(Box::from_raw(self.capify as *mut HfstTransducer));
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-acceptor-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-acceptor-fn]
    pub unsafe fn make_latin1_acceptor(type_: ImplementationType) -> *const HfstTransducer {
        let retval: *mut HfstTransducer = PmatchUtilityTransducers::make_latin1_alpha_acceptor(
            ImplementationType::TROPICAL_OPENFST_TYPE,
        ) as *mut HfstTransducer;
        let mut tmp: *mut HfstTransducer = PmatchUtilityTransducers::make_latin1_numeral_acceptor(
            ImplementationType::TROPICAL_OPENFST_TYPE,
        ) as *mut HfstTransducer;
        (*retval).disjunct(&*tmp, true);
        drop(Box::from_raw(tmp));
        tmp = PmatchUtilityTransducers::make_latin1_punct_acceptor(
            ImplementationType::TROPICAL_OPENFST_TYPE,
        ) as *mut HfstTransducer;
        (*retval).disjunct(&*tmp, true);
        drop(Box::from_raw(tmp));
        tmp = PmatchUtilityTransducers::make_latin1_whitespace_acceptor(
            ImplementationType::TROPICAL_OPENFST_TYPE,
        ) as *mut HfstTransducer;
        (*retval).disjunct(&*tmp, true);
        drop(Box::from_raw(tmp));
        (*retval).minimize();
        retval as *const HfstTransducer
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-alpha-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-alpha-acceptor-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-alpha-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-alpha-acceptor-fn]
    pub unsafe fn make_latin1_alpha_acceptor(type_: ImplementationType) -> *const HfstTransducer {
        let retval: *mut HfstTransducer = PmatchUtilityTransducers::make_latin1_lowercase_acceptor(
            ImplementationType::TROPICAL_OPENFST_TYPE,
        ) as *mut HfstTransducer;
        let tmp: *mut HfstTransducer = PmatchUtilityTransducers::make_latin1_uppercase_acceptor(
            ImplementationType::TROPICAL_OPENFST_TYPE,
        ) as *mut HfstTransducer;
        (*retval).disjunct(&*tmp, true);
        drop(Box::from_raw(tmp));
        (*retval).minimize();
        retval as *const HfstTransducer
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-lowercase-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-lowercase-acceptor-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-lowercase-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-lowercase-acceptor-fn]
    pub unsafe fn make_latin1_lowercase_acceptor(
        type_: ImplementationType,
    ) -> *const HfstTransducer {
        let retval: *mut HfstTransducer = acceptor_from_cstr(latin1_lower, type_);
        let tmp: *mut HfstTransducer = PmatchUtilityTransducers::make_combining_accent_acceptor(
            ImplementationType::TROPICAL_OPENFST_TYPE,
        ) as *mut HfstTransducer;
        (*retval).disjunct(&*tmp, true);
        drop(Box::from_raw(tmp));
        (*retval).minimize();
        retval as *const HfstTransducer
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-uppercase-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-uppercase-acceptor-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-uppercase-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-uppercase-acceptor-fn]
    pub unsafe fn make_latin1_uppercase_acceptor(
        type_: ImplementationType,
    ) -> *const HfstTransducer {
        let retval: *mut HfstTransducer = acceptor_from_cstr(latin1_upper, type_);
        let tmp: *mut HfstTransducer = PmatchUtilityTransducers::make_combining_accent_acceptor(
            ImplementationType::TROPICAL_OPENFST_TYPE,
        ) as *mut HfstTransducer;
        (*retval).disjunct(&*tmp, true);
        drop(Box::from_raw(tmp));
        (*retval).minimize();
        retval as *const HfstTransducer
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-combining-accent-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-combining-accent-acceptor-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-combining-accent-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-combining-accent-acceptor-fn]
    pub unsafe fn make_combining_accent_acceptor(
        type_: ImplementationType,
    ) -> *const HfstTransducer {
        acceptor_from_cstr(combining_accents, type_) as *const HfstTransducer
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-numeral-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-numeral-acceptor-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-numeral-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-numeral-acceptor-fn]
    pub unsafe fn make_latin1_numeral_acceptor(type_: ImplementationType) -> *const HfstTransducer {
        let retval: *mut HfstTransducer = Box::into_raw(Box::new(HfstTransducer::new_type(type_)));
        let num: String = "0123456789".to_string();
        for it in num.chars() {
            (*retval).disjunct(&HfstTransducer::new_symbol(&it.to_string(), type_), true);
        }
        // retval->minimize(); ?
        retval as *const HfstTransducer
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-punct-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-punct-acceptor-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-punct-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-punct-acceptor-fn]
    pub unsafe fn make_latin1_punct_acceptor(type_: ImplementationType) -> *const HfstTransducer {
        acceptor_from_cstr(latin1_punct, type_) as *const HfstTransducer
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-whitespace-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-whitespace-acceptor-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-whitespace-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-whitespace-acceptor-fn]
    pub unsafe fn make_latin1_whitespace_acceptor(
        type_: ImplementationType,
    ) -> *const HfstTransducer {
        acceptor_from_cstr(latin1_whitespace, type_) as *const HfstTransducer
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-capify-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-capify-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-capify-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-capify-fn]
    pub unsafe fn make_capify(&mut self, type_: ImplementationType) -> *const HfstTransducer {
        let retval: *mut HfstTransducer = Box::into_raw(Box::new(HfstTransducer::new_type(type_)));
        let tok: HfstTokenizer = HfstTokenizer::new();
        let mut i: usize = 0;
        while i < array_len(latin1_upper) {
            (*retval).disjunct(
                &HfstTransducer::new_tokenized_pair(latin1_lower[i], latin1_upper[i], &tok, type_),
                true,
            );
            i += 1;
        }
        let mut accents: HfstTransducer =
            HfstTransducer::new_copy(&*self.combining_accent_acceptor);
        accents.optionalize();
        (*retval).concatenate(&accents, true);
        (*retval).minimize();
        retval as *const HfstTransducer
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-lowerfy-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-lowerfy-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-lowerfy-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-lowerfy-fn]
    pub unsafe fn make_lowerfy(&mut self, type_: ImplementationType) -> *const HfstTransducer {
        let retval: *mut HfstTransducer = Box::into_raw(Box::new(HfstTransducer::new_type(type_)));
        let tok: HfstTokenizer = HfstTokenizer::new();
        let mut i: usize = 0;
        while i < array_len(latin1_upper) {
            (*retval).disjunct(
                &HfstTransducer::new_tokenized_pair(latin1_upper[i], latin1_lower[i], &tok, type_),
                true,
            );
            i += 1;
        }
        let mut accents: HfstTransducer =
            HfstTransducer::new_copy(&*self.combining_accent_acceptor);
        accents.optionalize();
        (*retval).concatenate(&accents, true);
        (*retval).minimize();
        retval as *const HfstTransducer
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.get-lowercase-acceptor-from-transducer-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.get-lowercase-acceptor-from-transducer-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.get-lowercase-acceptor-from-transducer-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.get-lowercase-acceptor-from-transducer-fn]
    pub fn get_lowercase_acceptor_from_transducer(
        &mut self,
        t: &mut HfstTransducer,
    ) -> HfstTransducer {
        let mut lowercase: HfstTransducer = HfstTransducer::new_type(t.get_type());
        let ss: StringSet = t.get_alphabet();
        for it in ss.iter() {
            let us: Vec<char> = it.chars().collect();
            if us.len() == 1 {
                if icu::properties::CodePointSetData::new::<icu::properties::props::Lowercase>()
                    .contains(us[0])
                {
                    lowercase.disjunct(&HfstTransducer::new_symbol(it, t.get_type()), true);
                }
            }
        }
        lowercase
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.get-uppercase-acceptor-from-transducer-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.get-uppercase-acceptor-from-transducer-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.get-uppercase-acceptor-from-transducer-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.get-uppercase-acceptor-from-transducer-fn]
    pub fn get_uppercase_acceptor_from_transducer(
        &mut self,
        t: &mut HfstTransducer,
    ) -> HfstTransducer {
        let mut uppercase: HfstTransducer = HfstTransducer::new_type(t.get_type());
        let ss: StringSet = t.get_alphabet();
        for it in ss.iter() {
            let us: Vec<char> = it.chars().collect();
            if us.len() == 1 {
                if icu::properties::CodePointSetData::new::<icu::properties::props::Uppercase>()
                    .contains(us[0])
                {
                    uppercase.disjunct(&HfstTransducer::new_symbol(it, t.get_type()), true);
                }
            }
        }
        uppercase
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.lowercaser-from-transducer-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.lowercaser-from-transducer-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.lowercaser-from-transducer-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.lowercaser-from-transducer-fn]
    pub fn lowercaser_from_transducer(&mut self, t: &mut HfstTransducer) -> HfstTransducer {
        let mut lowercase: HfstTransducer = HfstTransducer::new_type(t.get_type());
        let ss: StringSet = t.get_alphabet();
        let mut uppercases_seen: StringSet = StringSet::new();
        for it in ss.iter() {
            let us: Vec<char> = it.chars().collect();
            if us.len() == 1 {
                let this_unichar: char = us[0];
                if icu::properties::CodePointSetData::new::<icu::properties::props::Alphabetic>()
                    .contains(this_unichar)
                {
                    let upper: String = icu::casemap::CaseMapper::new()
                        .uppercase_to_string(it, &icu::locale::LanguageIdentifier::UNKNOWN)
                        .into_owned();
                    if uppercases_seen.contains(&upper) {
                        continue;
                    }
                    uppercases_seen.insert(upper.clone());
                    let lower: String = icu::casemap::CaseMapper::new()
                        .lowercase_to_string(it, &icu::locale::LanguageIdentifier::UNKNOWN)
                        .into_owned();
                    lowercase.disjunct(
                        &HfstTransducer::new_symbol_pair(&upper, &lower, t.get_type()),
                        true,
                    );
                }
            }
        }
        lowercase
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.uppercaser-from-transducer-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.uppercaser-from-transducer-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.uppercaser-from-transducer-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.uppercaser-from-transducer-fn]
    pub fn uppercaser_from_transducer(&mut self, t: &mut HfstTransducer) -> HfstTransducer {
        let mut uppercase: HfstTransducer = HfstTransducer::new_type(t.get_type());
        let ss: StringSet = t.get_alphabet();
        let mut uppercases_seen: StringSet = StringSet::new();
        for it in ss.iter() {
            let us: Vec<char> = it.chars().collect();
            if us.len() == 1 {
                let this_unichar: char = us[0];
                if icu::properties::CodePointSetData::new::<icu::properties::props::Alphabetic>()
                    .contains(this_unichar)
                {
                    let upper: String = icu::casemap::CaseMapper::new()
                        .uppercase_to_string(it, &icu::locale::LanguageIdentifier::UNKNOWN)
                        .into_owned();
                    if uppercases_seen.contains(&upper) {
                        continue;
                    }
                    uppercases_seen.insert(upper.clone());
                    let lower: String = icu::casemap::CaseMapper::new()
                        .lowercase_to_string(it, &icu::locale::LanguageIdentifier::UNKNOWN)
                        .into_owned();
                    uppercase.disjunct(
                        &HfstTransducer::new_symbol_pair(&lower, &upper, t.get_type()),
                        true,
                    );
                }
            }
        }
        uppercase
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.cap-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.cap-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.cap-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.cap-fn]
    pub unsafe fn cap(
        &mut self,
        t: &mut HfstTransducer,
        side: Side,
        optional: bool,
    ) -> *mut HfstTransducer {
        let orig_xerox_composition_value: bool = crate::hfst_transducer::get_xerox_composition();
        // This is to match flags in t with ?'s in "anything"
        crate::hfst_transducer::set_xerox_composition(true);

        let mut retval: *mut HfstTransducer = std::ptr::null_mut();
        let mut cap: HfstTransducer = self.uppercaser_from_transducer(t);
        let mut decap: HfstTransducer = HfstTransducer::new_copy(&cap);
        decap.invert();
        let mut anything: HfstTransducer = HfstTransducer::identity_pair(t.get_type());
        let mut anything_but_whitespace_star: HfstTransducer = HfstTransducer::new_copy(&anything);
        anything_but_whitespace_star.subtract(&*self.latin1_whitespace_acceptor, true);
        anything_but_whitespace_star.repeat_star();
        if optional == false {
            // don't let lowercased first letters through
            anything.subtract(&self.get_lowercase_acceptor_from_transducer(t), true);
        }
        // As in the regexp
        // [[[["A":"a" [[\" "]* (" " "A":"a")]* ] .o. [{ab ad}:{ef eh}].u]] .o.
        //   [{ab ad}:{ef eh}] ] .o. [[{ab ad}:{ef eh}].l] .o.
        //   ["e":"E" [[\" "]+ (" " "e":"E")]*]
        if side == Side::Lower {
            retval = Box::into_raw(Box::new(HfstTransducer::new_copy(t)));
            cap.disjunct(&anything, true);
            // Cap is the first letter to either capitalize or accept if it's not a
            // lowercase letter
            let mut continuation: HfstTransducer =
                HfstTransducer::new_copy(&anything_but_whitespace_star);
            // continuation is the rest of the first word
            let mut more_caps: HfstTransducer =
                HfstTransducer::new_copy(&*self.latin1_whitespace_acceptor);
            // more_caps is more words to capitalize
            more_caps.concatenate(&cap, true);
            more_caps.optionalize();
            continuation.concatenate(&more_caps, true);
            continuation.repeat_star();
            cap.concatenate(&continuation, true);
            (*retval).compose(&cap, true);
        } else if side == Side::Upper {
            decap.disjunct(&anything, true);
            let mut continuation: HfstTransducer =
                HfstTransducer::new_copy(&anything_but_whitespace_star);
            let mut more_decaps: HfstTransducer =
                HfstTransducer::new_copy(&*self.latin1_whitespace_acceptor);
            more_decaps.concatenate(&decap, true);
            more_decaps.optionalize();
            continuation.concatenate(&more_decaps, true);
            continuation.repeat_star();
            retval = Box::into_raw(Box::new(HfstTransducer::new_copy(&decap)));
            (*retval).concatenate(&continuation, true);
            (*retval).compose(t, true);
        } else {
            // both
            decap.disjunct(&anything, true);
            let mut continuation: HfstTransducer =
                HfstTransducer::new_copy(&anything_but_whitespace_star);
            let mut more_decaps: HfstTransducer =
                HfstTransducer::new_copy(&*self.latin1_whitespace_acceptor);
            more_decaps.concatenate(&decap, true);
            more_decaps.optionalize();
            continuation.concatenate(&more_decaps, true);
            continuation.repeat_star();
            retval = Box::into_raw(Box::new(HfstTransducer::new_copy(&decap)));
            (*retval).concatenate(&continuation, true);
            (*retval).compose(t, true);
            let mut continuation2: HfstTransducer =
                HfstTransducer::new_copy(&anything_but_whitespace_star);
            let mut more_caps: HfstTransducer =
                HfstTransducer::new_copy(&*self.latin1_whitespace_acceptor);
            cap.disjunct(&anything, true);
            more_caps.concatenate(&cap, true);
            more_caps.optionalize();
            continuation2.concatenate(&more_caps, true);
            continuation2.repeat_star();
            cap.concatenate(&continuation2, true);
            (*retval).compose(&cap, true);
            (*retval).output_project();
        }
        (*retval).minimize();
        crate::hfst_transducer::set_xerox_composition(orig_xerox_composition_value);
        retval
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.tolower-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.tolower-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.tolower-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.tolower-fn]
    pub unsafe fn tolower(
        &mut self,
        t: &mut HfstTransducer,
        side: Side,
        optional: bool,
    ) -> *mut HfstTransducer {
        let orig_xerox_composition_value: bool = crate::hfst_transducer::get_xerox_composition();
        // This is to match flags in t with ?'s in "anything"
        crate::hfst_transducer::set_xerox_composition(true);

        let mut anything: HfstTransducer =
            HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity, format);
        if optional == false {
            anything.subtract(&self.get_uppercase_acceptor_from_transducer(t), true);
        }
        let mut retval: *mut HfstTransducer = std::ptr::null_mut();
        if side == Side::Lower {
            let mut lowercase: HfstTransducer = self.lowercaser_from_transducer(t);
            lowercase.disjunct(&anything, true);
            lowercase.repeat_star();
            retval = Box::into_raw(Box::new(HfstTransducer::new_copy(t)));
            (*retval).compose(&lowercase, true);
        } else if side == Side::Upper {
            retval = Box::into_raw(Box::new(self.uppercaser_from_transducer(t)));
            (*retval).disjunct(&anything, true);
            (*retval).repeat_star();
            (*retval).compose(t, true);
        } else {
            // both
            retval = Box::into_raw(Box::new(self.uppercaser_from_transducer(t)));
            (*retval).disjunct(&anything, true);
            (*retval).repeat_star();
            (*retval).compose(t, true);
            let mut lowercase: HfstTransducer = self.lowercaser_from_transducer(t);
            lowercase.disjunct(&anything, true);
            lowercase.repeat_star();
            (*retval).compose(&lowercase, true);
        }
        (*retval).minimize();
        crate::hfst_transducer::set_xerox_composition(orig_xerox_composition_value);
        retval
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.toupper-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.toupper-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.toupper-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.toupper-fn]
    pub unsafe fn toupper(
        &mut self,
        t: &mut HfstTransducer,
        side: Side,
        optional: bool,
    ) -> *mut HfstTransducer {
        let orig_xerox_composition_value: bool = crate::hfst_transducer::get_xerox_composition();
        // This is to match flags in t with ?'s in "anything"
        crate::hfst_transducer::set_xerox_composition(true);

        let mut anything: HfstTransducer =
            HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity, format);
        if optional == false {
            anything.subtract(&self.get_lowercase_acceptor_from_transducer(t), true);
        }
        let mut retval: *mut HfstTransducer = std::ptr::null_mut();
        if side == Side::Lower {
            let mut uppercase: HfstTransducer = self.uppercaser_from_transducer(t);
            uppercase.disjunct(&anything, true);
            uppercase.repeat_star();
            retval = Box::into_raw(Box::new(HfstTransducer::new_copy(t)));
            (*retval).compose(&uppercase, true);
        } else if side == Side::Upper {
            retval = Box::into_raw(Box::new(self.lowercaser_from_transducer(t)));
            (*retval).disjunct(&anything, true);
            (*retval).repeat_star();
            (*retval).compose(t, true);
        } else {
            // both
            retval = Box::into_raw(Box::new(self.lowercaser_from_transducer(t)));
            (*retval).disjunct(&anything, true);
            (*retval).repeat_star();
            (*retval).compose(t, true);
            let mut uppercase: HfstTransducer = self.uppercaser_from_transducer(t);
            uppercase.disjunct(&anything, true);
            uppercase.repeat_star();
            (*retval).compose(&uppercase, true);
        }
        (*retval).minimize();
        crate::hfst_transducer::set_xerox_composition(orig_xerox_composition_value);
        retval
    }
}

// ===== body: string-symbol-marker-helpers =====
// Helper for faithfully constructing a freshly-'new'ed PmatchObject node's base
// fields, mirroring the C++ 'PmatchObject::PmatchObject()' default constructor
// (name="", weight=0.0, line_defined=pmatchlineno, my_timer uninitialised,
// cache=NULL). There is no lexer line counter in this port, so line_defined=0.

// [spec:hfst:def:pmatch-utils.should-colourise-fn]
// [spec:hfst:sem:pmatch-utils.should-colourise-fn]
pub fn should_colourise() -> bool {
    unsafe { if libc::isatty(1) != 0 { true } else { false } }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.warn-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.warn-fn]
pub fn warn(warning: String) {
    if should_colourise() {
        eprint!("\u{1b}[01m");
    }
    eprint!("hfst-pmatch: ");
    if should_colourise() {
        eprint!("\u{1b}[33m");
    }
    eprint!("Warning: ");
    if should_colourise() {
        eprint!("\u{1b}[0m");
    }
    eprint!("{}", warning);
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.get-utils-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-utils-fn]
pub fn get_utils() -> *mut PmatchUtilityTransducers {
    unsafe {
        if utils.is_null() {
            utils = Box::into_raw(Box::new(PmatchUtilityTransducers::new()));
        }
        utils
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.zero-minimization-guard-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.zero-minimization-guard-fn]
pub fn zero_minimization_guard() {
    unsafe {
        minimization_guard_count = 0;
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-minimization-guard-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-minimization-guard-fn]
pub fn make_minimization_guard() -> *mut PmatchTransducerContainer {
    unsafe {
        let mut guard = String::new();
        if minimization_guard_count == 0 {
            guard.push_str(internal_epsilon);
        } else {
            let mgc = minimization_guard_count;
            guard.push_str(&format!("@PMATCH_GUARD_{}@", mgc));
        }
        minimization_guard_count += 1;
        epsilon_to_symbol_container(guard)
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.symbol-in-global-context-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.symbol-in-global-context-fn]
pub fn symbol_in_global_context(sym: &mut String) -> bool {
    definitions().get(sym).is_some()
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.symbol-in-local-context-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.symbol-in-local-context-fn]
pub fn symbol_in_local_context(sym: &mut String) -> bool {
    if call_stack().len() == 0 {
        return false;
    }
    call_stack().last().unwrap().get(sym).is_some()
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.symbol-from-global-context-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.symbol-from-global-context-fn]
pub fn symbol_from_global_context(sym: &mut String) -> *mut dyn PmatchObject {
    if symbol_in_global_context(sym) {
        *definitions().get(sym).unwrap()
    } else {
        let p: *const dyn PmatchObject = std::ptr::null::<PmatchSymbol>();
        p as *mut dyn PmatchObject
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.symbol-from-local-context-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.symbol-from-local-context-fn]
pub fn symbol_from_local_context(sym: &mut String) -> *mut dyn PmatchObject {
    if symbol_in_local_context(sym) {
        *call_stack().last().unwrap().get(sym).unwrap()
    } else {
        let p: *const dyn PmatchObject = std::ptr::null::<PmatchSymbol>();
        p as *mut dyn PmatchObject
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.string-set-has-meta-arc-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.string-set-has-meta-arc-fn]
pub fn string_set_has_meta_arc(ss: &mut StringSet) -> bool {
    ss.iter().filter(|s| s.as_str() == internal_unknown).count() == 1
        || ss
            .iter()
            .filter(|s| s.as_str() == internal_identity)
            .count()
            == 1
        || ss.iter().filter(|s| s.as_str() == internal_default).count() == 1
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.is-special-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.is-special-fn]
pub fn is_special(symbol: &String) -> bool {
    if symbol.len() < 3 {
        return false;
    }
    symbol.find('@') == Some(0) && symbol.rfind('@') == Some(symbol.len() - 1)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.get-n-to-k-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-n-to-k-fn]
pub unsafe fn get_n_to_k(s: *const c_char) -> *mut i32 {
    let rv = libc::malloc(std::mem::size_of::<i32>() * 2) as *mut i32;
    let mut endptr: *mut c_char = std::ptr::null_mut();
    let mut finalptr: *mut c_char = std::ptr::null_mut();
    if *(s.add(1)) == b'{' as c_char {
        *rv.add(0) = libc::strtol(s.add(2), &mut endptr, 10) as i32;
        *rv.add(1) = libc::strtol(endptr.add(1), &mut finalptr, 10) as i32;
        assert!(*finalptr == b'}' as c_char);
    } else {
        *rv.add(0) = libc::strtol(s.add(1), &mut endptr, 10) as i32;
        *rv.add(1) = libc::strtol(endptr.add(1), &mut finalptr, 10) as i32;
        assert!(*finalptr == b'\0' as c_char);
    }
    rv
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.strip-percents-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.strip-percents-fn]
pub unsafe fn strip_percents(s: *const c_char) -> *mut c_char {
    let mut c = s;
    let stripped = libc::calloc(std::mem::size_of::<c_char>(), libc::strlen(s) + 1) as *mut c_char;
    let mut i: usize = 0;
    while *c != b'\0' as c_char {
        if *c == b'%' as c_char {
            if *(c.add(1)) == b'\0' as c_char {
                break;
            } else {
                *stripped.add(i) = *(c.add(1));
                i += 1;
                c = c.add(2);
            }
        } else {
            *stripped.add(i) = *c;
            i += 1;
            c = c.add(1);
        }
    }
    *stripped.add(i) = b'\0' as c_char;
    stripped
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.add-percents-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.add-percents-fn]
pub unsafe fn add_percents(s: *const c_char) -> *mut c_char {
    let ns = libc::malloc(std::mem::size_of::<c_char>() * libc::strlen(s) * 2 + 1) as *mut c_char;
    let mut p = ns;
    let mut s = s;
    while *s != b'\0' as c_char {
        let ch = *s;
        if (ch == b'@' as c_char)
            || (ch == b'-' as c_char)
            || (ch == b' ' as c_char)
            || (ch == b'|' as c_char)
            || (ch == b'!' as c_char)
            || (ch == b':' as c_char)
            || (ch == b';' as c_char)
            || (ch == b'0' as c_char)
            || (ch == b'\\' as c_char)
            || (ch == b'&' as c_char)
            || (ch == b'?' as c_char)
            || (ch == b'$' as c_char)
            || (ch == b'+' as c_char)
            || (ch == b'*' as c_char)
            || (ch == b'/' as c_char)
            || (ch == b'/' as c_char)
            || (ch == b'_' as c_char)
            || (ch == b'(' as c_char)
            || (ch == b')' as c_char)
            || (ch == b'{' as c_char)
            || (ch == b'}' as c_char)
            || (ch == b'[' as c_char)
            || (ch == b']' as c_char)
        {
            *p = b'%' as c_char;
            p = p.add(1);
        }
        *p = *s;
        p = p.add(1);
        s = s.add(1);
    }
    *p = b'\0' as c_char;
    ns
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.strip-newline-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.strip-newline-fn]
pub unsafe fn strip_newline(s: *mut c_char) -> *mut c_char {
    let mut pos: usize = 0;
    while *s.add(pos) != b'\0' as c_char {
        if *s.add(pos) == b'\n' as c_char || *s.add(pos) == b'\r' as c_char {
            *s.add(pos) = b'\0' as c_char;
        }
        pos += 1;
    }
    s
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.get-ins-transition-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-ins-transition-fn]
pub unsafe fn get_Ins_transition(s: *const c_char) -> String {
    let s_str = std::ffi::CStr::from_ptr(s).to_string_lossy();
    format!("@I.{}@", s_str)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.add-pmatch-delimiters-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.add-pmatch-delimiters-fn]
pub unsafe fn add_pmatch_delimiters(regex: *mut HfstTransducer) -> *mut HfstTransducer {
    let delimited_regex = Box::into_raw(Box::new(HfstTransducer::new_symbol_pair(
        internal_epsilon,
        ENTRY_SYMBOL,
        (*regex).get_type(),
    )));
    (*delimited_regex).concatenate(&*regex, true);
    let exit = HfstTransducer::new_symbol_pair(internal_epsilon, EXIT_SYMBOL, (*regex).get_type());
    (*delimited_regex).concatenate(&exit, true);
    drop(Box::from_raw(regex));
    delimited_regex
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-end-tag-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-end-tag-fn]
pub fn make_end_tag(tag: String) -> *mut PmatchTransducerContainer {
    epsilon_to_symbol_container(format!("@PMATCH_ENDTAG_{}@", tag))
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-capture-tag-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-capture-tag-fn]
pub fn make_capture_tag(tag: String) -> *mut PmatchTransducerContainer {
    epsilon_to_symbol_container(format!("@PMATCH_CAPTURE_{}@", tag))
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-captured-tag-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-captured-tag-fn]
pub fn make_captured_tag(tag: String) -> *mut PmatchTransducerContainer {
    epsilon_to_symbol_container(format!("@PMATCH_CAPTURED_{}@", tag))
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-with-tag-entry-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-with-tag-entry-fn]
pub fn make_with_tag_entry(key: String, value: String) -> *mut dyn PmatchObject {
    let obj = PmatchString {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        my_timer: 0,
        cache: std::ptr::null_mut(),
        string: format!("@P.PMATCH_GLOBAL_{}.{}@", key, value),
        multichar: false,
    };
    Box::into_raw(Box::new(obj)) as *mut dyn PmatchObject
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-with-tag-exit-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-with-tag-exit-fn]
pub fn make_with_tag_exit(key: String) -> *mut dyn PmatchObject {
    let obj = PmatchString {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        my_timer: 0,
        cache: std::ptr::null_mut(),
        string: format!("@C.PMATCH_GLOBAL_{}@", key),
        multichar: false,
    };
    Box::into_raw(Box::new(obj)) as *mut dyn PmatchObject
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-counter-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-counter-fn]
pub fn make_counter(name: String) -> *mut PmatchTransducerContainer {
    epsilon_to_symbol_container(format!("@PMATCH_COUNTER_{}@", name))
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.get-non-special-alphabet-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-non-special-alphabet-fn]
pub unsafe fn get_non_special_alphabet(t: *mut HfstTransducer) -> StringSet {
    let mut retval: StringSet = StringSet::new();
    let alphabet = (*t).get_alphabet();
    for it in alphabet.iter() {
        if PmatchAlphabet::is_printable(it) {
            retval.insert(it.clone());
        }
    }
    retval
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-list-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-list-fn]
pub unsafe fn make_list(t: *mut HfstTransducer, f: ImplementationType) -> *mut HfstTransducer {
    let mut transition = String::from("@L.");
    let alphabet = get_non_special_alphabet(t);
    for it in alphabet.iter() {
        transition.push_str(it);
        transition.push_str("_");
    }
    transition.push_str("@");
    Box::into_raw(Box::new(HfstTransducer::new_symbol(&transition, f)))
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-exc-list-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-exc-list-fn]
pub unsafe fn make_exc_list(t: *mut HfstTransducer, f: ImplementationType) -> *mut HfstTransducer {
    let mut transition = String::from("@X.");
    let alphabet = get_non_special_alphabet(t);
    for it in alphabet.iter() {
        transition.push_str(it);
        transition.push_str("_");
    }
    transition.push_str("@");
    Box::into_raw(Box::new(HfstTransducer::new_symbol(&transition, f)))
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-sigma-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-sigma-fn]
pub unsafe fn make_sigma(t: *mut HfstTransducer) -> *mut HfstTransducer {
    let retval = Box::into_raw(Box::new(HfstTransducer::new_type(format)));
    let alphabet = get_non_special_alphabet(t);
    for it in alphabet.iter() {
        (*retval).disjunct(&HfstTransducer::new_symbol(it, format), true);
    }
    retval
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.epsilon-to-symbol-container-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.epsilon-to-symbol-container-fn]
pub fn epsilon_to_symbol_container(s: String) -> *mut PmatchTransducerContainer {
    unsafe {
        let tmp = Box::into_raw(Box::new(HfstTransducer::new_symbol_pair(
            internal_epsilon,
            &s,
            format,
        )));
        let container = PmatchTransducerContainer {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            my_timer: 0,
            cache: std::ptr::null_mut(),
            t: tmp,
        };
        Box::into_raw(Box::new(container))
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-rc-entry-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-rc-entry-fn]
pub fn make_rc_entry() -> *mut PmatchTransducerContainer {
    epsilon_to_symbol_container(RC_ENTRY_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-lc-entry-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-lc-entry-fn]
pub fn make_lc_entry() -> *mut PmatchTransducerContainer {
    epsilon_to_symbol_container(LC_ENTRY_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-nrc-entry-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-nrc-entry-fn]
pub fn make_nrc_entry() -> *mut PmatchTransducerContainer {
    epsilon_to_symbol_container(NRC_ENTRY_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-nlc-entry-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-nlc-entry-fn]
pub fn make_nlc_entry() -> *mut PmatchTransducerContainer {
    epsilon_to_symbol_container(NLC_ENTRY_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-rc-exit-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-rc-exit-fn]
pub fn make_rc_exit() -> *mut PmatchTransducerContainer {
    epsilon_to_symbol_container(RC_EXIT_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-lc-exit-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-lc-exit-fn]
pub fn make_lc_exit() -> *mut PmatchTransducerContainer {
    epsilon_to_symbol_container(LC_EXIT_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-nrc-exit-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-nrc-exit-fn]
pub fn make_nrc_exit() -> *mut PmatchTransducerContainer {
    epsilon_to_symbol_container(NRC_EXIT_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-nlc-exit-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-nlc-exit-fn]
pub fn make_nlc_exit() -> *mut PmatchTransducerContainer {
    epsilon_to_symbol_container(NLC_EXIT_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-passthrough-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-passthrough-fn]
pub fn make_passthrough() -> *mut PmatchTransducerContainer {
    epsilon_to_symbol_container(PASSTHROUGH_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.get-delimited-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-delimited-fn]
pub unsafe fn get_delimited_lr(
    s: *const c_char,
    delim_left: c_char,
    delim_right: c_char,
) -> *mut c_char {
    let qstart = libc::strchr(s, delim_left as i32).add(1) as *const c_char;
    let qend = libc::strrchr(s, delim_right as i32) as *const c_char;
    let qpart = libc::strdup(qstart);
    *(qpart.add((qend as usize).wrapping_sub(qstart as usize))) = b'\0' as c_char;
    qpart
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.get-delimited-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-delimited-fn]
pub unsafe fn get_delimited(s: *const c_char, delim: c_char) -> *mut c_char {
    get_delimited_lr(s, delim, delim)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.get-escaped-delimited-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-escaped-delimited-fn]
pub unsafe fn get_escaped_delimited_lr(
    s: *const c_char,
    delim_left: c_char,
    delim_right: c_char,
) -> *mut c_char {
    unescape_delimited(get_delimited_lr(s, delim_left, delim_right), delim_right)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.get-escaped-delimited-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-escaped-delimited-fn]
pub unsafe fn get_escaped_delimited(s: *const c_char, delim: c_char) -> *mut c_char {
    unescape_delimited(get_delimited_lr(s, delim, delim), delim)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.unescape-delimited-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.unescape-delimited-fn]
pub unsafe fn unescape_delimited(s: *mut c_char, delim: c_char) -> *mut c_char {
    let mut read = s;
    let mut write = s;
    while *read != b'\0' as c_char {
        if *read == b'\\' as c_char
            && (*(read.add(1)) == delim || *(read.add(1)) == b'\\' as c_char)
        {
            *write = *(read.add(1));
            read = read.add(2);
            write = write.add(1);
        } else {
            *write = *read;
            read = read.add(1);
            write = write.add(1);
        }
    }
    *write = b'\0' as c_char;
    s
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.parse-quoted-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.parse-quoted-fn]
pub unsafe fn parse_quoted(s: *const c_char) -> *mut c_char {
    let quoted = get_delimited(s, b'"' as c_char);
    // Mysteriously, when the quoted string is 24 + n * 16 bytes in length, an
    // extra byte is needed for rv.
    let rv =
        libc::malloc(std::mem::size_of::<c_char>() * (libc::strlen(quoted) + 1)) as *mut c_char;
    let mut p = quoted;
    let mut r = rv;
    while *p != b'\0' as c_char {
        if *p != b'\\' as c_char {
            *r = *p;
            r = r.add(1);
            p = p.add(1);
        } else if *p == b'\\' as c_char {
            match *(p.add(1)) as u8 {
                b'0' | b'1' | b'2' | b'3' | b'4' | b'5' | b'6' | b'7' => {
                    eprint!(
                        "*** PMATCH unimplemented: parse octal escape in {}",
                        std::ffi::CStr::from_ptr(p).to_string_lossy()
                    );
                    *r = b'\0' as c_char;
                    p = p.add(5);
                }
                b'a' => {
                    *r = 0x07 as c_char;
                    r = r.add(1);
                    p = p.add(2);
                }
                b'b' => {
                    *r = 0x08 as c_char;
                    r = r.add(1);
                    p = p.add(2);
                }
                b'f' => {
                    *r = 0x0c as c_char;
                    r = r.add(1);
                    p = p.add(2);
                }
                b'n' => {
                    *r = b'\n' as c_char;
                    r = r.add(1);
                    p = p.add(2);
                }
                b'r' => {
                    *r = b'\r' as c_char;
                    r = r.add(1);
                    p = p.add(2);
                }
                b't' => {
                    *r = b'\t' as c_char;
                    r = r.add(1);
                    p = p.add(2);
                }
                b'u' => {
                    if libc::strlen(p) < 6 {
                        // Can't be a valid escape sequence
                        *r = *p;
                        r = r.add(1);
                        *r = *(p.add(1));
                        r = r.add(1);
                        p = p.add(2);
                    } else {
                        let mut buf: [c_char; 5] = [0; 5];
                        libc::memcpy(
                            buf.as_mut_ptr() as *mut libc::c_void,
                            p.add(2) as *const libc::c_void,
                            4,
                        );
                        buf[4] = b'\0' as c_char;
                        let codepoint = libc::strtol(buf.as_ptr(), std::ptr::null_mut(), 16) as u32;
                        let utf8_char = codepoint_to_utf8(codepoint);
                        let cs = std::ffi::CString::new(utf8_char.clone()).unwrap();
                        libc::strcpy(r, cs.as_ptr());
                        r = r.add(utf8_char.len() + 1);
                        p = p.add(6);
                    }
                }
                b'U' => {
                    if libc::strlen(p) < 10 {
                        // Can't be a valid escape sequence
                        *r = *p;
                        r = r.add(1);
                        *r = *(p.add(1));
                        r = r.add(1);
                        p = p.add(2);
                    } else {
                        let mut buf: [c_char; 9] = [0; 9];
                        libc::memcpy(
                            buf.as_mut_ptr() as *mut libc::c_void,
                            p.add(2) as *const libc::c_void,
                            8,
                        );
                        buf[8] = b'\0' as c_char;
                        let codepoint = libc::strtol(buf.as_ptr(), std::ptr::null_mut(), 16) as u32;
                        let utf8_char = codepoint_to_utf8(codepoint);
                        let cs = std::ffi::CString::new(utf8_char.clone()).unwrap();
                        libc::strcpy(r, cs.as_ptr());
                        r = r.add(utf8_char.len() + 1);
                        p = p.add(10);
                    }
                }
                b'v' => {
                    *r = 0x0b as c_char;
                    r = r.add(1);
                    p = p.add(2);
                }
                b'x' => {
                    let mut endp: *mut c_char = std::ptr::null_mut();
                    let i = libc::strtol(p.add(2), &mut endp, 16) as i32;
                    if 0 < i && i <= 127 {
                        *r = i as c_char;
                    } else {
                        eprint!("*** PMATCH unimplemented: parse \\x{}\n", i);
                        *r = b'\0' as c_char;
                    }
                    r = r.add(1);
                    assert!(endp != p);
                    p = endp;
                }
                b'\0' => {
                    eprint!("End of line after \\ escape\n");
                    *r = b'\0' as c_char;
                    r = r.add(1);
                    p = p.add(1);
                }
                _ => {
                    *r = *(p.add(1));
                    r = r.add(1);
                    p = p.add(2);
                }
            }
        }
    }
    *r = b'\0' as c_char;
    libc::free(quoted as *mut libc::c_void);
    rv
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.next-utf8-to-codepoint-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.next-utf8-to-codepoint-fn]
pub unsafe fn next_utf8_to_codepoint(c: *mut *mut u8) -> u32 {
    let mut codepoint: u32 = 0;
    let mut bytes_in_char: i32 = 0;
    if **c <= 127 {
        bytes_in_char = 1;
        codepoint = (**c & 127) as u32;
    } else if (**c & (128 + 64)) == (128 + 64) {
        bytes_in_char = 2;
        codepoint = (**c & 31) as u32;
    } else if (**c & (128 + 64 + 32)) == (128 + 64 + 32) {
        bytes_in_char = 3;
        codepoint = (**c & 15) as u32;
    } else if (**c & (128 + 64 + 32 + 16)) == (128 + 64 + 32 + 16) {
        bytes_in_char = 4;
        codepoint = (**c & 7) as u32;
    } else {
        return 0;
    }
    let mut i = 1;
    while i < bytes_in_char {
        codepoint = (codepoint << 6) | ((*(*c).add(i as usize) & 63) as u64) as u32;
        i += 1;
    }
    *c = (*c).add(bytes_in_char as usize);
    codepoint
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.codepoint-to-utf8-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.codepoint-to-utf8-fn]
pub fn codepoint_to_utf8(codepoint: u32) -> String {
    let mut buf: [u8; 5] = [0; 5];
    let mut u_parse_err = false;
    // The following is adapted from an answer at
    // http://stackoverflow.com/questions/4607413/c-library-to-convert-unicode-code-points-to-utf8
    // My understanding of the magic numbers:
    // 0x80 = 128 = 2^7
    // 64 = 2^6, 192 = 2^6 + 2^7
    // 0x800 = 2048 = 2^11
    // 0x1000 = 2^16 etc.
    if codepoint < 0x80 {
        buf[0] = codepoint as u8;
        buf[1] = b'\0';
    } else if codepoint < 0x800 {
        buf[0] = (192 + codepoint / 64) as u8;
        buf[1] = (128 + codepoint % 64) as u8;
        buf[2] = b'\0';
    } else if codepoint.wrapping_sub(0xd800u32) < 0x800 {
        u_parse_err = true;
    } else if codepoint < 0x10000 {
        buf[0] = (224 + codepoint / 4096) as u8;
        buf[1] = (128 + codepoint / 64 % 64) as u8;
        buf[2] = (128 + codepoint % 64) as u8;
        buf[3] = b'\0';
    } else if codepoint < 0x110000 {
        buf[0] = (240 + codepoint / 262144) as u8;
        buf[1] = (128 + codepoint / 4096 % 64) as u8;
        buf[2] = (128 + codepoint / 64 % 64) as u8;
        buf[3] = (128 + codepoint % 64) as u8;
        buf[4] = b'\0';
    } else {
        u_parse_err = true;
    }
    if u_parse_err {
        "".to_string()
    } else {
        // std::string(buf) constructs up to the first NUL byte.
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        unsafe { String::from_utf8_unchecked(buf[..end].to_vec()) }
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.parse-range-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.parse-range-fn]
pub unsafe fn parse_range(s: *const c_char) -> *mut PmatchTransducerContainer {
    let quoted = get_delimited(s, b'"' as c_char);
    let orig_quoted = quoted;
    let mut quoted_var = quoted;
    let c: *mut *mut c_char = &mut quoted_var;
    let retval = Box::into_raw(Box::new(HfstTransducer::new_type(format)));
    while **c != b'\0' as c_char {
        let mut codepoint1: u32 = 0;
        let mut codepoint2: u32 = 0;
        if libc::strlen(*c) >= 6
            && **c == b'\\' as c_char
            && (*((*c).add(1)) == b'u' as c_char || *((*c).add(1)) == b'U' as c_char)
        {
            // an escape sequence
            let mut buf: [c_char; 9] = [0; 9];
            if *((*c).add(1)) == b'u' as c_char {
                libc::memcpy(
                    buf.as_mut_ptr() as *mut libc::c_void,
                    (*c).add(2) as *const libc::c_void,
                    4,
                );
                buf[4] = b'\0' as c_char;
                *c = (*c).add(6);
            } else {
                libc::memcpy(
                    buf.as_mut_ptr() as *mut libc::c_void,
                    (*c).add(2) as *const libc::c_void,
                    8,
                );
                buf[8] = b'\0' as c_char;
                *c = (*c).add(10);
            }
            codepoint1 = libc::strtol(buf.as_ptr(), std::ptr::null_mut(), 16) as u32;
        } else {
            codepoint1 = next_utf8_to_codepoint(c as *mut *mut u8);
        }
        if **c != b'-' as c_char {
            let mut errstring = String::from("Could not parse range expression: ");
            errstring.push_str(&std::ffi::CStr::from_ptr(s).to_string_lossy());
            let cs = std::ffi::CString::new(errstring).unwrap();
            pmatcherror(&cs.to_string_lossy());
        }
        *c = (*c).add(1);
        if libc::strlen(*c) >= 6
            && **c == b'\\' as c_char
            && (*((*c).add(1)) == b'u' as c_char || *((*c).add(1)) == b'U' as c_char)
        {
            let mut buf: [c_char; 9] = [0; 9];
            if *((*c).add(1)) == b'u' as c_char {
                libc::memcpy(
                    buf.as_mut_ptr() as *mut libc::c_void,
                    (*c).add(2) as *const libc::c_void,
                    4,
                );
                buf[4] = b'\0' as c_char;
                *c = (*c).add(6);
            } else {
                libc::memcpy(
                    buf.as_mut_ptr() as *mut libc::c_void,
                    (*c).add(2) as *const libc::c_void,
                    8,
                );
                buf[8] = b'\0' as c_char;
                *c = (*c).add(10);
            }
            codepoint2 = libc::strtol(buf.as_ptr(), std::ptr::null_mut(), 16) as u32;
        } else {
            codepoint2 = next_utf8_to_codepoint(c as *mut *mut u8);
        }
        if codepoint1 == 0 || codepoint2 == 0 {
            let mut errstring = String::from("Malformed character in range expression: ");
            errstring.push_str(&std::ffi::CStr::from_ptr(s).to_string_lossy());
            let cs = std::ffi::CString::new(errstring).unwrap();
            pmatcherror(&cs.to_string_lossy());
        }
        if codepoint2 < codepoint1 {
            let mut errstring = String::from("Range expression goes from higher to lower: ");
            errstring.push_str(&std::ffi::CStr::from_ptr(s).to_string_lossy());
            let cs = std::ffi::CString::new(errstring).unwrap();
            pmatcherror(&cs.to_string_lossy());
        }
        while codepoint1 <= codepoint2 {
            (*retval).disjunct(
                &HfstTransducer::new_symbol(&codepoint_to_utf8(codepoint1), format),
                true,
            );
            codepoint1 += 1;
        }
    }
    libc::free(orig_quoted as *mut libc::c_void);
    let container = PmatchTransducerContainer {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        my_timer: 0,
        cache: std::ptr::null_mut(),
        t: retval,
    };
    Box::into_raw(Box::new(container))
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.get-weight-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-weight-fn]
pub unsafe fn get_weight(s: *const c_char) -> f64 {
    let mut rv: f64 = -3.1415;
    let mut weightstart = s;
    while (*weightstart != b'\0' as c_char)
        && ((*weightstart == b' ' as c_char)
            || (*weightstart == b'\t' as c_char)
            || (*weightstart == b';' as c_char))
    {
        weightstart = weightstart.add(1);
    }
    let mut endp: *mut c_char = std::ptr::null_mut();
    rv = libc::strtod(weightstart, &mut endp);
    assert!(endp != weightstart as *mut c_char);
    rv
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.get-size-info-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-size-info-fn]
pub unsafe fn get_size_info(net: *mut HfstTransducer) -> String {
    let tmp = HfstBasicTransducer::from_transducer(&*net);
    let mut states: usize = 0;
    let mut arcs: usize = 0;
    for state_it in tmp.states_and_transitions().iter() {
        states += 1;
        for _tr_it in state_it.iter() {
            arcs += 1;
        }
    }
    format!("{} states and {} arcs", states, arcs)
}

// ===== body: unary-eval =====
// [spec:hfst:def:pmatch-utils.hfst.pmatch-unary-operation.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-unary-operation.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-unary-operation.evaluate-fn]
impl PmatchObject for PmatchUnaryOperation {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn set_name(&mut self, name: String) {
        self.name = name;
    }
    fn get_weight(&self) -> f64 {
        self.weight
    }
    fn set_weight(&mut self, weight: f64) {
        self.weight = weight;
    }
    fn get_line_defined(&self) -> i32 {
        self.line_defined
    }
    fn set_line_defined(&mut self, line_defined: i32) {
        self.line_defined = line_defined;
    }
    fn get_my_timer(&self) -> clock_t {
        self.my_timer
    }
    fn set_my_timer(&mut self, my_timer: clock_t) {
        self.my_timer = my_timer;
    }
    fn get_cache(&self) -> *mut HfstTransducer {
        self.cache
    }
    fn set_cache(&mut self, cache: *mut HfstTransducer) {
        self.cache = cache;
    }

    unsafe fn evaluate(&mut self) -> *mut HfstTransducer {
        if !self.cache.is_null() {
            self.report_cache(String::new());
            return Box::into_raw(Box::new(HfstTransducer::new_copy(&*self.cache)));
        }
        let mut retval: *mut HfstTransducer = std::ptr::null_mut();
        self.start_timing();

        // Special optimization cases
        if self.op == PmatchUnaryOp::Implode {
            let mut strings: StringVector = StringVector::new();
            (*self.root).collect_strings_into(&mut strings);
            let mut whole_string = String::new();
            for it in strings.iter() {
                whole_string += it;
            }
            if whole_string.len() > 0 {
                retval = Box::into_raw(Box::new(HfstTransducer::new_symbol(&whole_string, format)));
            } else {
                retval = Box::into_raw(Box::new(HfstTransducer::new_type(format)));
            }
            (*retval).set_final_weights(crate::hfst_data_types::double_to_float(self.weight), true);
            if self.cache.is_null() && self.should_use_cache() == true {
                self.cache = retval;
                self.report_time(" with ".to_string() + &get_size_info(self.cache));
                return Box::into_raw(Box::new(HfstTransducer::new_copy(&*self.cache)));
            }
            self.report_time(String::new());
            return retval;
        } else if self.op == PmatchUnaryOp::Explode {
            let mut strings: StringVector = StringVector::new();
            (*self.root).collect_strings_into(&mut strings);
            let mut whole_string = String::new();
            for it in strings.iter() {
                whole_string += it;
            }
            let tok = crate::hfst_tokenizer::HfstTokenizer::new();
            if whole_string.len() > 0 {
                retval = Box::into_raw(Box::new(HfstTransducer::new_tokenized(
                    &whole_string,
                    &tok,
                    format,
                )));
            } else {
                retval = Box::into_raw(Box::new(HfstTransducer::new_type(format)));
            }
            (*retval).set_final_weights(crate::hfst_data_types::double_to_float(self.weight), true);
            if self.cache.is_null() && self.should_use_cache() == true {
                self.cache = retval;
                return Box::into_raw(Box::new(HfstTransducer::new_copy(&*self.cache)));
            }
            self.report_time(String::new());
            return retval;
        }

        if self.name != "" {
            eval_stack().push(self.name.clone());
        }
        retval = (*self.root).evaluate();
        if self.op == PmatchUnaryOp::AddDelimiters {
            retval = add_pmatch_delimiters(retval);
        } else if self.op == PmatchUnaryOp::Optionalize {
            (*retval).optionalize();
        } else if self.op == PmatchUnaryOp::RepeatStar {
            (*retval).repeat_star();
        } else if self.op == PmatchUnaryOp::RepeatPlus {
            (*retval).repeat_plus();
        } else if self.op == PmatchUnaryOp::Reverse {
            (*retval).reverse();
        } else if self.op == PmatchUnaryOp::Invert {
            (*retval).invert();
        } else if self.op == PmatchUnaryOp::InputProject {
            (*retval).input_project();
        } else if self.op == PmatchUnaryOp::OutputProject {
            (*retval).output_project();
        } else if self.op == PmatchUnaryOp::Complement {
            // Defined here only for automata, so can project to input
            let complement = Box::into_raw(Box::new(HfstTransducer::new_symbol(
                crate::hfst_symbol_defs::internal_identity,
                format,
            )));
            (*complement).repeat_star();
            (*complement).subtract(&*retval, true);
            drop(Box::from_raw(retval));
            retval = complement;
        } else if self.op == PmatchUnaryOp::Containment {
            let mut any =
                HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity, format);
            any.repeat_star();
            let left = Box::into_raw(Box::new(HfstTransducer::new_copy(&any)));
            (*left).concatenate(&*retval, true);
            (*left).concatenate(&any, true);
            drop(Box::from_raw(retval));
            retval = left;
        } else if self.op == PmatchUnaryOp::ContainmentOnce {
            let mut xre_comp = crate::xre::XreCompiler::new(format);
            let new_retval = Box::into_raw(Box::new(xre_comp.contains_once(&*retval)));
            drop(Box::from_raw(retval));
            retval = new_retval;
        } else if self.op == PmatchUnaryOp::ContainmentOptional {
            let mut xre_comp = crate::xre::XreCompiler::new(format);
            let new_retval = Box::into_raw(Box::new(xre_comp.contains_once_optional(&*retval)));
            drop(Box::from_raw(retval));
            retval = new_retval;
        } else if self.op == PmatchUnaryOp::TermComplement {
            let any = Box::into_raw(Box::new(HfstTransducer::new_symbol(
                crate::hfst_symbol_defs::internal_identity,
                format,
            )));
            let alphabet: StringSet = get_non_special_alphabet(retval);
            for it in alphabet.iter() {
                let symbol = HfstTransducer::new_symbol(it, format);
                (*any).subtract(&symbol, true);
            }
            drop(Box::from_raw(retval));
            retval = any;
        } else if self.op == PmatchUnaryOp::Cap {
            let tmp = (*get_utils()).cap(&mut *retval, Side::Both, false);
            drop(Box::from_raw(retval));
            retval = tmp;
        } else if self.op == PmatchUnaryOp::OptCap {
            let tmp = (*get_utils()).cap(&mut *retval, Side::Both, true);
            drop(Box::from_raw(retval));
            retval = tmp;
        } else if self.op == PmatchUnaryOp::ToLower {
            let tmp = (*get_utils()).tolower(&mut *retval, Side::Both, false);
            drop(Box::from_raw(retval));
            retval = tmp;
        } else if self.op == PmatchUnaryOp::ToUpper {
            let tmp = (*get_utils()).toupper(&mut *retval, Side::Both, false);
            drop(Box::from_raw(retval));
            retval = tmp;
        } else if self.op == PmatchUnaryOp::OptToLower {
            let tmp = (*get_utils()).tolower(&mut *retval, Side::Both, true);
            (*tmp).disjunct(&*retval, true);
            drop(Box::from_raw(retval));
            retval = tmp;
        } else if self.op == PmatchUnaryOp::OptToUpper {
            let tmp = (*get_utils()).toupper(&mut *retval, Side::Both, true);
            drop(Box::from_raw(retval));
            retval = tmp;
        } else if self.op == PmatchUnaryOp::AnyCase {
            let toupper = (*get_utils()).toupper(&mut *retval, Side::Both, true);
            let tolower = (*get_utils()).tolower(&mut *retval, Side::Both, true);
            (*retval).disjunct(&*toupper, true);
            (*retval).disjunct(&*tolower, true);
            drop(Box::from_raw(toupper));
            drop(Box::from_raw(tolower));
        } else if self.op == PmatchUnaryOp::CapUpper {
            let tmp = (*get_utils()).cap(&mut *retval, Side::Upper, false);
            drop(Box::from_raw(retval));
            retval = tmp;
        } else if self.op == PmatchUnaryOp::OptCapUpper {
            let tmp = (*get_utils()).cap(&mut *retval, Side::Upper, true);
            drop(Box::from_raw(retval));
            retval = tmp;
        } else if self.op == PmatchUnaryOp::ToLowerUpper {
            let tmp = (*get_utils()).tolower(&mut *retval, Side::Upper, false);
            drop(Box::from_raw(retval));
            retval = tmp;
        } else if self.op == PmatchUnaryOp::ToUpperUpper {
            let tmp = (*get_utils()).toupper(&mut *retval, Side::Upper, false);
            drop(Box::from_raw(retval));
            retval = tmp;
        } else if self.op == PmatchUnaryOp::OptToLowerUpper {
            let tmp = (*get_utils()).tolower(&mut *retval, Side::Upper, true);
            (*tmp).disjunct(&*retval, true);
            drop(Box::from_raw(retval));
            retval = tmp;
        } else if self.op == PmatchUnaryOp::OptToUpperUpper {
            let tmp = (*get_utils()).toupper(&mut *retval, Side::Upper, true);
            drop(Box::from_raw(retval));
            retval = tmp;
        } else if self.op == PmatchUnaryOp::AnyCaseUpper {
            let toupper = (*get_utils()).toupper(&mut *retval, Side::Upper, true);
            let tolower = (*get_utils()).tolower(&mut *retval, Side::Upper, true);
            (*retval).disjunct(&*toupper, true);
            (*retval).disjunct(&*tolower, true);
            drop(Box::from_raw(toupper));
            drop(Box::from_raw(tolower));
        } else if self.op == PmatchUnaryOp::CapLower {
            let tmp = (*get_utils()).cap(&mut *retval, Side::Lower, false);
            drop(Box::from_raw(retval));
            retval = tmp;
        } else if self.op == PmatchUnaryOp::OptCapLower {
            let tmp = (*get_utils()).cap(&mut *retval, Side::Lower, true);
            drop(Box::from_raw(retval));
            retval = tmp;
        } else if self.op == PmatchUnaryOp::ToLowerLower {
            let tmp = (*get_utils()).tolower(&mut *retval, Side::Lower, false);
            drop(Box::from_raw(retval));
            retval = tmp;
        } else if self.op == PmatchUnaryOp::ToUpperLower {
            let tmp = (*get_utils()).toupper(&mut *retval, Side::Lower, false);
            drop(Box::from_raw(retval));
            retval = tmp;
        } else if self.op == PmatchUnaryOp::OptToLowerLower {
            let tmp = (*get_utils()).tolower(&mut *retval, Side::Lower, true);
            (*tmp).disjunct(&*retval, true);
            drop(Box::from_raw(retval));
            retval = tmp;
        } else if self.op == PmatchUnaryOp::OptToUpperLower {
            let tmp = (*get_utils()).toupper(&mut *retval, Side::Lower, true);
            drop(Box::from_raw(retval));
            retval = tmp;
        } else if self.op == PmatchUnaryOp::AnyCaseLower {
            let toupper = (*get_utils()).toupper(&mut *retval, Side::Lower, true);
            let tolower = (*get_utils()).tolower(&mut *retval, Side::Lower, true);
            (*retval).disjunct(&*toupper, true);
            (*retval).disjunct(&*tolower, true);
            drop(Box::from_raw(toupper));
            drop(Box::from_raw(tolower));
        } else if self.op == PmatchUnaryOp::MakeSigma {
            let tmp = make_sigma(retval);
            drop(Box::from_raw(retval));
            retval = tmp;
        } else if self.op == PmatchUnaryOp::MakeList {
            let tmp = make_list(retval, format);
            register_lst_line_numbers_from_transducer(tmp, self.line_defined);
            drop(Box::from_raw(retval));
            retval = tmp;
        } else if self.op == PmatchUnaryOp::MakeExcList {
            let tmp = make_exc_list(retval, format);
            drop(Box::from_raw(retval));
            retval = tmp;
        } else if self.op == PmatchUnaryOp::LC {
            if !transducer_has_context_symbol(retval) {
                (*retval).reverse();
                let tmp = Box::into_raw(Box::new(HfstTransducer::new_symbol_pair(
                    crate::hfst_symbol_defs::internal_epsilon,
                    LC_ENTRY_SYMBOL,
                    format,
                )));
                (*tmp).concatenate(&*retval, true);
                let lc_exit = HfstTransducer::new_symbol_pair(
                    crate::hfst_symbol_defs::internal_epsilon,
                    LC_EXIT_SYMBOL,
                    format,
                );
                (*tmp).concatenate(&lc_exit, true);
                drop(Box::from_raw(retval));
                retval = tmp;
            } else if verbose {
                write_compilation_stack_indentation_to_err();
                eprint!(
                    "** Warning: ignoring nested context condition when compiling {}\n",
                    eval_stack().last().unwrap()
                );
            }
        } else if self.op == PmatchUnaryOp::NLC {
            if !transducer_has_context_symbol(retval) {
                (*retval).reverse();
                let tmp = make_minimization_guard();
                let head = (*tmp).evaluate();
                drop(Box::from_raw(tmp));
                let passthrough = HfstTransducer::new_symbol(PASSTHROUGH_SYMBOL, format);
                let mut nlc_entry = HfstTransducer::new_symbol_pair(
                    crate::hfst_symbol_defs::internal_epsilon,
                    NLC_ENTRY_SYMBOL,
                    format,
                );
                let nlc_exit = HfstTransducer::new_symbol_pair(
                    crate::hfst_symbol_defs::internal_epsilon,
                    NLC_EXIT_SYMBOL,
                    format,
                );
                nlc_entry.concatenate(&*retval, true);
                nlc_entry.concatenate(&nlc_exit, true);
                nlc_entry.disjunct(&passthrough, true);
                (*head).concatenate(&nlc_entry, true);
                drop(Box::from_raw(retval));
                retval = head;
            } else if verbose {
                write_compilation_stack_indentation_to_err();
                eprint!(
                    "** Warning: ignoring nested context condition when compiling {}\n",
                    eval_stack().last().unwrap()
                );
            }
        } else if self.op == PmatchUnaryOp::RC {
            if !transducer_has_context_symbol(retval) {
                let tmp = Box::into_raw(Box::new(HfstTransducer::new_symbol_pair(
                    crate::hfst_symbol_defs::internal_epsilon,
                    RC_ENTRY_SYMBOL,
                    format,
                )));
                (*tmp).concatenate(&*retval, true);
                let rc_exit = HfstTransducer::new_symbol_pair(
                    crate::hfst_symbol_defs::internal_epsilon,
                    RC_EXIT_SYMBOL,
                    format,
                );
                (*tmp).concatenate(&rc_exit, true);
                drop(Box::from_raw(retval));
                retval = tmp;
            } else if verbose {
                write_compilation_stack_indentation_to_err();
                eprint!(
                    "** Warning: ignoring nested context condition when compiling {}\n",
                    eval_stack().last().unwrap()
                );
            }
        } else if self.op == PmatchUnaryOp::NRC {
            if !transducer_has_context_symbol(retval) {
                let tmp = make_minimization_guard();
                let head = (*tmp).evaluate();
                drop(Box::from_raw(tmp));
                let passthrough = HfstTransducer::new_symbol(PASSTHROUGH_SYMBOL, format);
                let mut nrc_entry = HfstTransducer::new_symbol_pair(
                    crate::hfst_symbol_defs::internal_epsilon,
                    NRC_ENTRY_SYMBOL,
                    format,
                );
                let nrc_exit = HfstTransducer::new_symbol_pair(
                    crate::hfst_symbol_defs::internal_epsilon,
                    NRC_EXIT_SYMBOL,
                    format,
                );
                nrc_entry.concatenate(&*retval, true);
                nrc_entry.concatenate(&nrc_exit, true);
                nrc_entry.disjunct(&passthrough, true);
                (*head).concatenate(&nrc_entry, true);
                drop(Box::from_raw(retval));
                retval = head;
            } else if verbose {
                write_compilation_stack_indentation_to_err();
                eprint!(
                    "** Warning: ignoring nested context condition when compiling {}\n",
                    eval_stack().last().unwrap()
                );
            }
        }
        (*retval).set_final_weights(crate::hfst_data_types::double_to_float(self.weight), true);

        if self.name != "" {
            eval_stack().pop();
        }
        if self.cache.is_null() && self.should_use_cache() == true {
            self.cache = retval;
            (*self.cache).minimize();
            self.report_time(" with ".to_string() + &get_size_info(self.cache));
            return Box::into_raw(Box::new(HfstTransducer::new_copy(&*self.cache)));
        }
        self.report_time(String::new());
        return retval;
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-unary-operation.get-initial-symbols-from-unary-root-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-unary-operation.get-initial-symbols-from-unary-root-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation.get-initial-symbols-from-unary-root-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-unary-operation.get-initial-symbols-from-unary-root-fn]
    fn get_initial_symbols_from_unary_root(&mut self) -> StringSet {
        unsafe { PmatchObject_get_real_initial_symbols(&mut *self.root) }
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-unary-operation.is-context-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-unary-operation.is-context-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation.is-context-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-unary-operation.is-context-fn]
    fn is_context(&mut self) -> bool {
        self.op == PmatchUnaryOp::LC
            || self.op == PmatchUnaryOp::NLC
            || self.op == PmatchUnaryOp::RC
            || self.op == PmatchUnaryOp::NRC
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-unary-operation.is-delimiter-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-unary-operation.is-delimiter-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation.is-delimiter-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-unary-operation.is-delimiter-fn]
    fn is_delimiter(&mut self) -> bool {
        self.op == PmatchUnaryOp::AddDelimiters
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-unary-operation.get-initial-rc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-unary-operation.get-initial-rc-initial-symbols-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation.get-initial-rc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-unary-operation.get-initial-rc-initial-symbols-fn]
    fn get_initial_RC_initial_symbols(&mut self) -> StringSet {
        unsafe {
            if self.op == PmatchUnaryOp::RC {
                let tmp: *mut HfstTransducer = (*self.root).evaluate();
                let retval: StringSet = (*tmp).get_initial_input_symbols();
                drop(Box::from_raw(tmp));
                return retval;
            }
            if self.op == PmatchUnaryOp::AddDelimiters {
                return (*self.root).get_initial_RC_initial_symbols();
            }
            StringSet::new()
        }
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-unary-operation.get-initial-nrc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-unary-operation.get-initial-nrc-initial-symbols-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation.get-initial-nrc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-unary-operation.get-initial-nrc-initial-symbols-fn]
    fn get_initial_NRC_initial_symbols(&mut self) -> StringSet {
        unsafe {
            if self.op == PmatchUnaryOp::NRC {
                let tmp: *mut HfstTransducer = (*self.root).evaluate();
                let retval: StringSet = (*tmp).get_initial_input_symbols();
                drop(Box::from_raw(tmp));
                return retval;
            }
            if self.op == PmatchUnaryOp::AddDelimiters {
                return (*self.root).get_initial_NRC_initial_symbols();
            }
            StringSet::new()
        }
    }
}

// ===== body: binary-ternary-numeric-eval =====
// [spec:hfst:def:pmatch-utils.hfst.pmatch-numeric-operation.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-numeric-operation.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-numeric-operation.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-numeric-operation.evaluate-fn]
impl PmatchObject for PmatchNumericOperation {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn set_name(&mut self, name: String) {
        self.name = name;
    }
    fn get_weight(&self) -> f64 {
        self.weight
    }
    fn set_weight(&mut self, weight: f64) {
        self.weight = weight;
    }
    fn get_line_defined(&self) -> i32 {
        self.line_defined
    }
    fn set_line_defined(&mut self, line_defined: i32) {
        self.line_defined = line_defined;
    }
    fn get_my_timer(&self) -> clock_t {
        self.my_timer
    }
    fn set_my_timer(&mut self, my_timer: clock_t) {
        self.my_timer = my_timer;
    }
    fn get_cache(&self) -> *mut HfstTransducer {
        self.cache
    }
    fn set_cache(&mut self, cache: *mut HfstTransducer) {
        self.cache = cache;
    }

    unsafe fn evaluate(&mut self) -> *mut HfstTransducer {
        unsafe {
            if !self.cache.is_null() {
                self.report_cache(String::new());
                return Box::into_raw(Box::new(HfstTransducer::new_copy(&*self.cache)));
            }
            let tmp: *mut HfstTransducer;
            self.start_timing();
            if self.name != "" {
                eval_stack().push(self.name.clone());
            }
            tmp = (*self.root).evaluate();
            if self.op == PmatchNumericOp::RepeatN {
                (*tmp).repeat_n(self.values[0] as u32);
            } else if self.op == PmatchNumericOp::RepeatNPlus {
                (*tmp).repeat_n_plus(self.values[0] as u32);
            } else if self.op == PmatchNumericOp::RepeatNMinus {
                (*tmp).repeat_n_minus(self.values[0] as u32);
            } else if self.op == PmatchNumericOp::RepeatNToK {
                (*tmp).repeat_n_to_k(self.values[0] as u32, self.values[1] as u32);
            }
            (*tmp).set_final_weights(crate::hfst_data_types::double_to_float(self.weight), true);
            if self.name != "" {
                eval_stack().pop();
            }
            if self.cache.is_null() && self.should_use_cache() == true {
                self.cache = tmp;
                (*self.cache).minimize();
                self.report_time(String::new());
                return Box::into_raw(Box::new(HfstTransducer::new_copy(&*self.cache)));
            }
            self.report_time(String::new());
            return tmp;
        }
    }
}
// Pass a map from Lst() symbol to line number. Emits one warning per Lst()
// symbol (so line numbers stay unambiguous).
// [spec:hfst:def:pmatch-utils.hfst.fix-list-overlap-fn]
// [spec:hfst:sem:pmatch-utils.hfst.fix-list-overlap-fn]
pub unsafe fn fix_list_overlap(
    lhs: *mut HfstTransducer,
    rhs: *mut HfstTransducer,
    list_set: &StringSet,
    literal_set: &StringSet,
    lst_line_map: &BTreeMap<String, i32>,
) {
    unsafe {
        for sym in list_set.iter() {
            if !sym.starts_with("@L.") {
                continue;
            }

            let mut overlapping_chars: Vec<String> = Vec::new();
            let mut retained_chars: Vec<String> = Vec::new();
            let mut lst_line: i32 = -1;
            if let Some(line) = lst_line_map.get(sym) {
                lst_line = *line;
            }

            // Parse the list content: @L.a_b_c@ -> a, b, c
            let mut start: usize = 3; // Skip @L.
            let mut end: Option<usize> = sym[start..].find('_').map(|i| i + start);
            while let Some(e) = end {
                let sub = sym[start..e].to_string();
                if literal_set.contains(&sub) {
                    overlapping_chars.push(sub);
                } else {
                    retained_chars.push(sub);
                }
                start = e + 1;
                end = sym[start..].find('_').map(|i| i + start);
            }
            // Check last element (before final @)
            if start < sym.len() {
                let last = sym[start..sym.len() - 1].to_string();
                if literal_set.contains(&last) {
                    overlapping_chars.push(last);
                }
            }

            if overlapping_chars.is_empty() {
                continue;
            }

            // Ensure each Lst() only triggers a single warning per compilation.
            let warn_key: String;
            if lst_line >= 0 {
                let mut wk = lst_line.to_string();
                wk.push('\t');
                wk.push_str(sym);
                warn_key = wk;
            } else {
                warn_key = sym.clone();
            }
            if lst_overlap_warned().contains(&warn_key) {
                continue;
            }
            lst_overlap_warned().insert(warn_key);
            let mut newlist = String::from("@L.");
            let mut first = true;
            for s in retained_chars.iter() {
                newlist.push_str(s);
                newlist.push('_');
                first = false;
            }
            newlist.push('@');
            let newsym: StringPair = (newlist.clone(), newlist.clone());
            let mut newpairs: StringPairSet = StringPairSet::new();
            newpairs.insert(newsym);
            if verbose {
                eprint!(
                    "Automatically optimising:Removing the following symbols from Lst() (line {}):",
                    if lst_line >= 0 {
                        lst_line.to_string()
                    } else {
                        "?".to_string()
                    }
                );
            }
            for i in 0..overlapping_chars.len() {
                let overlapsym: StringPair =
                    (overlapping_chars[i].clone(), overlapping_chars[i].clone());
                if verbose {
                    eprint!("\n  '{}' (", overlapping_chars[i]);
                    print_unicode_codepoints(&mut std::io::stderr(), &overlapping_chars[i]);
                    eprint!(")");
                }
                newpairs.insert(overlapsym);
            }
            if verbose {
                eprintln!();
                eprintln!(
                    "Replacing all {} instances with new list: {} and abovementioned disjunction ",
                    sym, newlist
                );
            }
            let oldsym: StringPair = (sym.clone(), sym.clone());
            (*lhs).substitute_pair_with_pair_set(&oldsym, &newpairs);
            (*rhs).substitute_pair_with_pair_set(&oldsym, &newpairs);
        }
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.evaluate-fn]
impl PmatchObject for PmatchBinaryOperation {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn set_name(&mut self, name: String) {
        self.name = name;
    }
    fn get_weight(&self) -> f64 {
        self.weight
    }
    fn set_weight(&mut self, weight: f64) {
        self.weight = weight;
    }
    fn get_line_defined(&self) -> i32 {
        self.line_defined
    }
    fn set_line_defined(&mut self, line_defined: i32) {
        self.line_defined = line_defined;
    }
    fn get_my_timer(&self) -> clock_t {
        self.my_timer
    }
    fn set_my_timer(&mut self, my_timer: clock_t) {
        self.my_timer = my_timer;
    }
    fn get_cache(&self) -> *mut HfstTransducer {
        self.cache
    }
    fn set_cache(&mut self, cache: *mut HfstTransducer) {
        self.cache = cache;
    }

    unsafe fn evaluate(&mut self) -> *mut HfstTransducer {
        unsafe {
            if !self.cache.is_null() {
                self.report_cache(String::new());
                return Box::into_raw(Box::new(HfstTransducer::new_copy(&*self.cache)));
            }
            self.start_timing();
            let mut retval: *mut HfstTransducer = std::ptr::null_mut();

            // Special optimization cases
            if self.op == PmatchBinaryOp::Disjunct {
                if (*self.left).is_unweighted_disjunction_of_strings()
                    && (*self.right).is_unweighted_disjunction_of_strings()
                {
                    let mut strings: StringVector = StringVector::new();
                    (*self.left).collect_strings_into(&mut strings);
                    (*self.right).collect_strings_into(&mut strings);
                    let tok = crate::hfst_tokenizer::HfstTokenizer::new();
                    retval = Box::into_raw(Box::new(HfstTransducer::new_type(format)));
                    for it in strings.iter() {
                        let spv = tok.tokenize(it, false); // XXX
                        (*retval).disjunct_spv(&spv);
                    }
                    (*retval).set_final_weights(
                        crate::hfst_data_types::double_to_float(self.weight),
                        true,
                    );
                    if self.cache.is_null() && self.should_use_cache() == true {
                        self.cache = retval;
                        // No minimization because we did it the clever way!
                        self.report_time(format!(" with {}", get_size_info(self.cache)));
                        return Box::into_raw(Box::new(HfstTransducer::new_copy(&*self.cache)));
                    }
                    self.report_time(String::new());
                    return retval;
                }
            }

            if self.name != "" {
                eval_stack().push(self.name.clone());
            }
            // General cases
            let mut lhs: *mut HfstTransducer = (*self.left).evaluate();
            let mut rhs: *mut HfstTransducer = (*self.right).evaluate();
            match self.op {
                PmatchBinaryOp::Concatenate => {
                    (*lhs).concatenate(&*rhs, true);
                }
                PmatchBinaryOp::Compose => {
                    (*lhs).compose(&*rhs, true);
                }
                PmatchBinaryOp::CrossProduct => {
                    (*lhs).cross_product(&*rhs, true);
                }
                PmatchBinaryOp::LenientCompose => {
                    (*lhs).lenient_composition(&*rhs, true);
                }
                PmatchBinaryOp::Disjunct => {
                    let lhs_syms = (*lhs).get_alphabet();
                    let rhs_syms = (*rhs).get_alphabet();
                    fix_list_overlap(lhs, rhs, &lhs_syms, &rhs_syms, lst_line_map());
                    fix_list_overlap(rhs, lhs, &rhs_syms, &lhs_syms, lst_line_map());
                    (*lhs).disjunct(&*rhs, true);
                }
                PmatchBinaryOp::Intersect => {
                    (*lhs).intersect(&*rhs, true);
                }
                PmatchBinaryOp::Subtract => {
                    if verbose {
                        warn_on_nonsubtractable_symbols(lhs);
                        warn_on_nonsubtractable_symbols(rhs);
                    }
                    (*lhs).subtract(&*rhs, true);
                }
                PmatchBinaryOp::UpperSubtract => {
                    pmatcherror("Upper subtraction not implemented.");
                    return lhs;
                }
                PmatchBinaryOp::LowerSubtract => {
                    pmatcherror("Lower subtraction not implemented.");
                    return lhs;
                }
                PmatchBinaryOp::UpperPriorityUnion => {
                    (*lhs).priority_union(&*rhs);
                }
                PmatchBinaryOp::LowerPriorityUnion => {
                    (*lhs).invert();
                    (*rhs).invert();
                    (*lhs).priority_union(&*rhs);
                    (*lhs).invert();
                }
                PmatchBinaryOp::Shuffle => {
                    let __prev_hook = std::panic::take_hook();
                    std::panic::set_hook(Box::new(|_| {}));
                    let __res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                        (*lhs).shuffle(&*rhs, true);
                    }));
                    std::panic::set_hook(__prev_hook);
                    if let Err(e) = __res {
                        if e.downcast_ref::<crate::hfst_exception_defs::TransducersAreNotAutomataException>()
                            .is_some()
                        {
                            pmatchwarning(
                                "tried to shuffle with non-automaton transducers,\n    shuffling with their input projection instead.",
                            );
                            (*lhs).input_project();
                            (*rhs).input_project();
                            (*lhs).shuffle(&*rhs, true);
                        } else {
                            std::panic::resume_unwind(e);
                        }
                    }
                }
                PmatchBinaryOp::Before => {
                    let tmp: *mut HfstTransducer =
                        Box::into_raw(Box::new(crate::hfst_xerox_rules::before(&*lhs, &*rhs)));
                    let _ = Box::from_raw(lhs);
                    lhs = tmp;
                }
                PmatchBinaryOp::After => {
                    let tmp: *mut HfstTransducer =
                        Box::into_raw(Box::new(crate::hfst_xerox_rules::after(&*lhs, &*rhs)));
                    let _ = Box::from_raw(lhs);
                    lhs = tmp;
                }
                PmatchBinaryOp::InsertFreely => {
                    (*lhs).insert_freely(&*rhs, false);
                }
                PmatchBinaryOp::IgnoreInternally => {
                    let right_part: *mut HfstTransducer =
                        Box::into_raw(Box::new(HfstTransducer::new_copy(&*lhs)));
                    let middle_part: *mut HfstTransducer =
                        Box::into_raw(Box::new(HfstTransducer::new_copy(&*lhs)));
                    (*middle_part).disjunct(&*rhs, true);
                    (*middle_part).repeat_star();
                    (*lhs).concatenate(&*middle_part, true);
                    (*lhs).concatenate(&*right_part, true);
                    let _ = Box::from_raw(middle_part);
                    let _ = Box::from_raw(right_part);
                }
                PmatchBinaryOp::Merge => {
                    let tmp: *mut HfstTransducer;
                    let fmt = format;
                    let __prev_hook = std::panic::take_hook();
                    std::panic::set_hook(Box::new(|_| {}));
                    let __res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                        // hfst::xre::merge_first_to_second(lhs, rhs)
                        let args = crate::xre::XreConstructorArguments::new(
                            BTreeMap::new(),
                            BTreeMap::new(),
                            BTreeMap::new(),
                            BTreeMap::new(),
                            fmt,
                        );
                        (*lhs).optimize();
                        (*rhs).merge(&*lhs, &args);
                        rhs
                    }));
                    std::panic::set_hook(__prev_hook);
                    match __res {
                        Ok(t) => {
                            tmp = t;
                        }
                        Err(e) => {
                            if e.downcast_ref::<crate::hfst_exception_defs::TransducersAreNotAutomataException>()
                                .is_some()
                            {
                                pmatcherror(
                                    "Error: transducers must be automata in merge operation.",
                                );
                                tmp = std::ptr::null_mut();
                            } else {
                                std::panic::resume_unwind(e);
                            }
                        }
                    }
                    let _ = Box::from_raw(lhs);
                    // NB: mirrors the C++ aliasing (lhs now points at rhs); the
                    // unconditional 'delete rhs' below frees it and the later use
                    // is the original's behaviour.
                    lhs = tmp;
                }
            }
            let _ = Box::from_raw(rhs);
            (*lhs).set_final_weights(crate::hfst_data_types::double_to_float(self.weight), true);
            if self.name != "" {
                eval_stack().pop();
            }
            retval = lhs;
            if self.cache.is_null() && self.should_use_cache() == true {
                self.cache = retval;
                (*self.cache).minimize();
                self.report_time(format!(" with {}", get_size_info(self.cache)));
                return Box::into_raw(Box::new(HfstTransducer::new_copy(&*self.cache)));
            }
            self.report_time(String::new());
            return retval;
        }
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.get-real-initial-symbols-from-right-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.get-real-initial-symbols-from-right-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.get-real-initial-symbols-from-right-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.get-real-initial-symbols-from-right-fn]
    fn get_real_initial_symbols_from_right(&mut self) -> StringSet {
        unsafe { PmatchObject_get_real_initial_symbols(&mut *self.right) }
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.is-left-concatenation-with-context-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.is-left-concatenation-with-context-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.is-left-concatenation-with-context-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.is-left-concatenation-with-context-fn]
    fn is_left_concatenation_with_context(&mut self) -> bool {
        unsafe { self.op == PmatchBinaryOp::Concatenate && (*self.left).is_context() }
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.get-initial-rc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.get-initial-rc-initial-symbols-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.get-initial-rc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.get-initial-rc-initial-symbols-fn]
    fn get_initial_RC_initial_symbols(&mut self) -> StringSet {
        unsafe {
            let mut retval: StringSet = StringSet::new();
            if self.op == PmatchBinaryOp::Concatenate {
                let left_ss: StringSet = (*self.left).get_initial_RC_initial_symbols();
                let mut right_ss: StringSet = StringSet::new();
                if (*self.right).is_context() || (*self.right).is_delimiter() {
                    right_ss = (*self.right).get_initial_NRC_initial_symbols();
                }
                for it in left_ss.iter() {
                    retval.insert(it.clone());
                }
                for it in right_ss.iter() {
                    retval.insert(it.clone());
                }
                return retval;
            }
            retval
        }
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.get-initial-nrc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.get-initial-nrc-initial-symbols-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.get-initial-nrc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.get-initial-nrc-initial-symbols-fn]
    fn get_initial_NRC_initial_symbols(&mut self) -> StringSet {
        unsafe {
            let mut retval: StringSet = StringSet::new();
            if self.op == PmatchBinaryOp::Concatenate {
                let left_ss: StringSet = (*self.left).get_initial_NRC_initial_symbols();
                let mut right_ss: StringSet = StringSet::new();
                if (*self.right).is_context() || (*self.right).is_delimiter() {
                    right_ss = (*self.right).get_initial_NRC_initial_symbols();
                }
                for it in left_ss.iter() {
                    retval.insert(it.clone());
                }
                for it in right_ss.iter() {
                    retval.insert(it.clone());
                }
                return retval;
            }
            retval
        }
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.collect-strings-into-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.collect-strings-into-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.collect-strings-into-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.collect-strings-into-fn]
    fn collect_strings_into(&mut self, strings: &mut StringVector) {
        unsafe {
            (*self.left).collect_strings_into(strings);
            (*self.right).collect_strings_into(strings);
        }
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.as-string-pair-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.as-string-pair-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.as-string-pair-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.as-string-pair-fn]
    fn as_string_pair(&mut self) -> StringPair {
        unsafe {
            if self.op == PmatchBinaryOp::CrossProduct {
                let left_string: String = (*self.left).as_string();
                let right_string: String = (*self.right).as_string();
                return (left_string, right_string);
            }
            (String::new(), String::new())
        }
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.is-unweighted-disjunction-of-strings-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.is-unweighted-disjunction-of-strings-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.is-unweighted-disjunction-of-strings-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.is-unweighted-disjunction-of-strings-fn]
    fn is_unweighted_disjunction_of_strings(&mut self) -> bool {
        unsafe {
            self.weight == 0.0
                && self.op == PmatchBinaryOp::Disjunct
                && (*self.left).is_unweighted_disjunction_of_strings()
                && (*self.right).is_unweighted_disjunction_of_strings()
        }
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-ternary-operation.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-ternary-operation.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-ternary-operation.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-ternary-operation.evaluate-fn]
impl PmatchObject for PmatchTernaryOperation {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn set_name(&mut self, name: String) {
        self.name = name;
    }
    fn get_weight(&self) -> f64 {
        self.weight
    }
    fn set_weight(&mut self, weight: f64) {
        self.weight = weight;
    }
    fn get_line_defined(&self) -> i32 {
        self.line_defined
    }
    fn set_line_defined(&mut self, line_defined: i32) {
        self.line_defined = line_defined;
    }
    fn get_my_timer(&self) -> clock_t {
        self.my_timer
    }
    fn set_my_timer(&mut self, my_timer: clock_t) {
        self.my_timer = my_timer;
    }
    fn get_cache(&self) -> *mut HfstTransducer {
        self.cache
    }
    fn set_cache(&mut self, cache: *mut HfstTransducer) {
        self.cache = cache;
    }

    unsafe fn evaluate(&mut self) -> *mut HfstTransducer {
        unsafe {
            if !self.cache.is_null() {
                self.report_cache(String::new());
                return Box::into_raw(Box::new(HfstTransducer::new_copy(&*self.cache)));
            }
            self.start_timing();
            if self.name != "" {
                eval_stack().push(self.name.clone());
            }
            let mut retval: *mut HfstTransducer = std::ptr::null_mut();
            if self.op == PmatchTernaryOp::Substitute {
                retval = (*self.left).evaluate();
                let middle_pair: StringPair = (*self.middle).as_string_pair();
                let right_pair: StringPair = (*self.right).as_string_pair();
                if right_pair.0 != "" || right_pair.1 != "" {
                    (*retval).substitute_pair_with_pair(&middle_pair, &right_pair);
                } else {
                    let tmp: *mut HfstTransducer = (*self.right).evaluate();
                    (*retval).substitute_pair_with_transducer(&middle_pair, &mut *tmp, true);
                    let _ = Box::from_raw(tmp);
                }
            } else if self.op == PmatchTernaryOp::Uncompose {
                retval = (*self.left).evaluate();
                let unc_left: *mut HfstTransducer = (*self.middle).evaluate();
                let unc_right: *mut HfstTransducer = (*self.right).evaluate();
            }
            (*retval).set_final_weights(crate::hfst_data_types::double_to_float(self.weight), true);
            if self.cache.is_null() && self.should_use_cache() == true {
                self.cache = retval;
                (*self.cache).minimize();
                self.report_time(String::new());
                return Box::into_raw(Box::new(HfstTransducer::new_copy(&*self.cache)));
            }
            self.report_time(String::new());
            if self.name != "" {
                eval_stack().pop();
            }
            return retval;
        }
    }
}
// [spec:hfst:def:pmatch-utils.hfst.transducer-has-context-symbol-fn]
// [spec:hfst:sem:pmatch-utils.hfst.transducer-has-context-symbol-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.transducer-has-context-symbol-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.transducer-has-context-symbol-fn]
pub unsafe fn transducer_has_context_symbol(t: *mut HfstTransducer) -> bool {
    unsafe {
        let ss: StringSet = (*t).get_alphabet();
        ss.contains(LC_ENTRY_SYMBOL)
            || ss.contains(NLC_ENTRY_SYMBOL)
            || ss.contains(RC_ENTRY_SYMBOL)
            || ss.contains(NRC_ENTRY_SYMBOL)
    }
}
// [spec:hfst:def:pmatch-utils.hfst.warn-on-nonsubtractable-symbols-fn]
// [spec:hfst:sem:pmatch-utils.hfst.warn-on-nonsubtractable-symbols-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.warn-on-nonsubtractable-symbols-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.warn-on-nonsubtractable-symbols-fn]
pub unsafe fn warn_on_nonsubtractable_symbols(t: *mut HfstTransducer) {
    unsafe {
        let alphabet: StringSet = (*t).get_alphabet();
        for it in alphabet.iter() {
            if it.len() < 3 {
                continue;
            } else if it.starts_with("@PMATCH") || it.starts_with("@I") || it.starts_with("@L") {
                write_compilation_stack_indentation_to_err();
                eprintln!("Warning: subtracting with nonsubtractable symbol {}", it);
            }
        }
    }
}

// ===== body: replace-restriction-containers =====
// [spec:hfst:def:pmatch-utils.hfst.pmatch-parallel-rules-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-parallel-rules-container.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-parallel-rules-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-parallel-rules-container.evaluate-fn]
impl PmatchObject for PmatchParallelRulesContainer {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn set_name(&mut self, name: String) {
        self.name = name;
    }
    fn get_weight(&self) -> f64 {
        self.weight
    }
    fn set_weight(&mut self, weight: f64) {
        self.weight = weight;
    }
    fn get_line_defined(&self) -> i32 {
        self.line_defined
    }
    fn set_line_defined(&mut self, line_defined: i32) {
        self.line_defined = line_defined;
    }
    fn get_my_timer(&self) -> clock_t {
        self.my_timer
    }
    fn set_my_timer(&mut self, my_timer: clock_t) {
        self.my_timer = my_timer;
    }
    fn get_cache(&self) -> *mut HfstTransducer {
        self.cache
    }
    fn set_cache(&mut self, cache: *mut HfstTransducer) {
        self.cache = cache;
    }

    unsafe fn evaluate(&mut self) -> *mut HfstTransducer {
        if !self.cache.is_null() {
            self.report_cache(String::new());
            return Box::into_raw(Box::new(HfstTransducer::new_copy(&*self.cache)));
        }
        self.start_timing();
        let mut retval: *mut HfstTransducer = std::ptr::null_mut();
        match self.arrow {
            ReplaceArrow::E_REPLACE_RIGHT => {
                retval = Box::into_raw(Box::new(replace_rule_vector(&self.make_mappings(), false)));
            }
            ReplaceArrow::E_OPTIONAL_REPLACE_RIGHT => {
                retval = Box::into_raw(Box::new(replace_rule_vector(&self.make_mappings(), true)));
            }
            ReplaceArrow::E_REPLACE_LEFT => {
                retval = Box::into_raw(Box::new(replace_left_rule_vector(
                    &self.make_mappings(),
                    false,
                )));
            }
            ReplaceArrow::E_OPTIONAL_REPLACE_LEFT => {
                retval = Box::into_raw(Box::new(replace_left_rule_vector(
                    &self.make_mappings(),
                    true,
                )));
            }
            ReplaceArrow::E_RTL_LONGEST_MATCH => {
                retval = Box::into_raw(Box::new(replace_rightmost_longest_match_rule_vector(
                    &self.make_mappings(),
                )));
            }
            ReplaceArrow::E_RTL_SHORTEST_MATCH => {
                retval = Box::into_raw(Box::new(replace_rightmost_shortest_match_rule_vector(
                    &self.make_mappings(),
                )));
            }
            ReplaceArrow::E_LTR_LONGEST_MATCH => {
                retval = Box::into_raw(Box::new(replace_leftmost_longest_match_rule_vector(
                    &self.make_mappings(),
                )));
            }
            ReplaceArrow::E_LTR_SHORTEST_MATCH => {
                retval = Box::into_raw(Box::new(replace_leftmost_shortest_match_rule_vector(
                    &self.make_mappings(),
                )));
            }
            ReplaceArrow::E_REPLACE_RIGHT_MARKUP => {
                pmatcherror("Unrecognized arrow type");
                return std::ptr::null_mut();
            }
        }
        (*retval).set_final_weights(double_to_float(self.weight), true);
        self.report_time(String::new());
        if self.cache.is_null() && self.should_use_cache() == true {
            self.cache = retval;
            (*self.cache).minimize();
            return Box::into_raw(Box::new(HfstTransducer::new_copy(&*self.cache)));
        }
        retval
    }
}
impl PmatchParallelRulesContainer {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-parallel-rules-container.make-mappings-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-parallel-rules-container.make-mappings-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-parallel-rules-container.make-mappings-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-parallel-rules-container.make-mappings-fn]
    pub unsafe fn make_mappings(&mut self) -> Vec<Rule> {
        let mut retval: Vec<Rule> = Vec::new();
        for it in self.rules.iter() {
            retval.push((**it).make_mapping());
        }

        retval
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-replace-rule-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-replace-rule-container.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-replace-rule-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-replace-rule-container.evaluate-fn]
impl PmatchObject for PmatchReplaceRuleContainer {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn set_name(&mut self, name: String) {
        self.name = name;
    }
    fn get_weight(&self) -> f64 {
        self.weight
    }
    fn set_weight(&mut self, weight: f64) {
        self.weight = weight;
    }
    fn get_line_defined(&self) -> i32 {
        self.line_defined
    }
    fn set_line_defined(&mut self, line_defined: i32) {
        self.line_defined = line_defined;
    }
    fn get_my_timer(&self) -> clock_t {
        self.my_timer
    }
    fn set_my_timer(&mut self, my_timer: clock_t) {
        self.my_timer = my_timer;
    }
    fn get_cache(&self) -> *mut HfstTransducer {
        self.cache
    }
    fn set_cache(&mut self, cache: *mut HfstTransducer) {
        self.cache = cache;
    }

    unsafe fn evaluate(&mut self) -> *mut HfstTransducer {
        if !self.cache.is_null() {
            self.report_cache(String::new());
            return Box::into_raw(Box::new(HfstTransducer::new_copy(&*self.cache)));
        }
        self.start_timing();
        let mut retval: *mut HfstTransducer = std::ptr::null_mut();
        match self.arrow {
            ReplaceArrow::E_REPLACE_RIGHT => {
                retval = Box::into_raw(Box::new(replace_rule(&self.make_mapping(), false)));
            }
            ReplaceArrow::E_OPTIONAL_REPLACE_RIGHT => {
                retval = Box::into_raw(Box::new(replace_rule(&self.make_mapping(), true)));
            }
            ReplaceArrow::E_REPLACE_LEFT => {
                retval = Box::into_raw(Box::new(replace_left_rule(&self.make_mapping(), false)));
            }
            ReplaceArrow::E_OPTIONAL_REPLACE_LEFT => {
                retval = Box::into_raw(Box::new(replace_left_rule(&self.make_mapping(), true)));
            }
            ReplaceArrow::E_RTL_LONGEST_MATCH => {
                retval = Box::into_raw(Box::new(replace_rightmost_longest_match_rule(
                    &self.make_mapping(),
                )));
            }
            ReplaceArrow::E_RTL_SHORTEST_MATCH => {
                retval = Box::into_raw(Box::new(replace_rightmost_shortest_match_rule(
                    &self.make_mapping(),
                )));
            }
            ReplaceArrow::E_LTR_LONGEST_MATCH => {
                retval = Box::into_raw(Box::new(replace_leftmost_longest_match_rule(
                    &self.make_mapping(),
                )));
            }
            ReplaceArrow::E_LTR_SHORTEST_MATCH => {
                retval = Box::into_raw(Box::new(replace_leftmost_shortest_match_rule(
                    &self.make_mapping(),
                )));
            }
            ReplaceArrow::E_REPLACE_RIGHT_MARKUP => {
                pmatcherror("Unrecognized arrow");
                return std::ptr::null_mut();
            }
        }
        (*retval).set_final_weights(double_to_float(self.weight), true);
        self.report_time(String::new());
        if self.cache.is_null() && self.should_use_cache() == true {
            self.cache = retval;
            (*self.cache).minimize();
            return Box::into_raw(Box::new(HfstTransducer::new_copy(&*self.cache)));
        }
        retval
    }
}
impl PmatchReplaceRuleContainer {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-replace-rule-container.make-mapping-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-replace-rule-container.make-mapping-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-replace-rule-container.make-mapping-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-replace-rule-container.make-mapping-fn]
    pub unsafe fn make_mapping(&mut self) -> Rule {
        let mut pair_vector: HfstTransducerPairVector = Vec::new();
        for it in self.mapping.iter() {
            let pp: TransducerPointerPair = (**it).evaluate_pair();
            let p: HfstTransducerPair = (
                HfstTransducer::new_copy(&*pp.0),
                HfstTransducer::new_copy(&*pp.1),
            );
            drop(Box::from_raw(pp.0));
            drop(Box::from_raw(pp.1));
            pair_vector.push(p);
        }
        if self.context.len() == 0 {
            return Rule::new_mapping(&pair_vector);
        }
        let mut context_vector: HfstTransducerPairVector = Vec::new();
        for it in self.context.iter() {
            let pp: TransducerPointerPair = (**it).evaluate_pair();
            let p: HfstTransducerPair = (
                HfstTransducer::new_copy(&*pp.0),
                HfstTransducer::new_copy(&*pp.1),
            );
            drop(Box::from_raw(pp.0));
            drop(Box::from_raw(pp.1));
            context_vector.push(p);
        }
        Rule::new_mapping_context_repl_type(&pair_vector, &context_vector, self.type_)
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-restriction-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-restriction-container.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-restriction-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-restriction-container.evaluate-fn]
impl PmatchObject for PmatchRestrictionContainer {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn set_name(&mut self, name: String) {
        self.name = name;
    }
    fn get_weight(&self) -> f64 {
        self.weight
    }
    fn set_weight(&mut self, weight: f64) {
        self.weight = weight;
    }
    fn get_line_defined(&self) -> i32 {
        self.line_defined
    }
    fn set_line_defined(&mut self, line_defined: i32) {
        self.line_defined = line_defined;
    }
    fn get_my_timer(&self) -> clock_t {
        self.my_timer
    }
    fn set_my_timer(&mut self, my_timer: clock_t) {
        self.my_timer = my_timer;
    }
    fn get_cache(&self) -> *mut HfstTransducer {
        self.cache
    }
    fn set_cache(&mut self, cache: *mut HfstTransducer) {
        self.cache = cache;
    }

    unsafe fn evaluate(&mut self) -> *mut HfstTransducer {
        if !self.cache.is_null() {
            self.report_cache(String::new());
            return Box::into_raw(Box::new(HfstTransducer::new_copy(&*self.cache)));
        }
        self.start_timing();
        let mut retval: *mut HfstTransducer = std::ptr::null_mut();
        let mut pair_vector: HfstTransducerPairVector = Vec::new();
        for it in (*self.contexts).iter() {
            let pp: TransducerPointerPair = (**it).evaluate_pair();
            let p: HfstTransducerPair = (
                HfstTransducer::new_copy(&*pp.0),
                HfstTransducer::new_copy(&*pp.1),
            );
            drop(Box::from_raw(pp.0));
            drop(Box::from_raw(pp.1));
            pair_vector.push(p);
        }
        let l: *mut HfstTransducer = (*self.left).evaluate();
        retval = Box::into_raw(Box::new(restriction(&*l, &pair_vector)));
        drop(Box::from_raw(l));
        (*retval).set_final_weights(double_to_float(self.weight), true);
        self.report_time(String::new());
        if self.cache.is_null() && self.should_use_cache() == true {
            self.cache = retval;
            (*self.cache).minimize();
            return Box::into_raw(Box::new(HfstTransducer::new_copy(&*self.cache)));
        }
        retval
    }
}
impl PmatchObjectPairBase for PmatchObjectPair {
    fn get_left(&self) -> *mut dyn PmatchObject {
        self.left
    }
    fn set_left(&mut self, l: *mut dyn PmatchObject) {
        self.left = l;
    }
    fn get_right(&self) -> *mut dyn PmatchObject {
        self.right
    }
    fn set_right(&mut self, r: *mut dyn PmatchObject) {
        self.right = r;
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object-pair.evaluate-pair-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object-pair.evaluate-pair-fn]
    unsafe fn evaluate_pair(&mut self) -> TransducerPointerPair {
        let mut retval: TransducerPointerPair;
        let first = (*self.left).evaluate();
        let second = (*self.right).evaluate();
        retval = (first, second);
        retval
    }
}
impl PmatchObjectPairBase for PmatchMarkupContainer {
    fn get_left(&self) -> *mut dyn PmatchObject {
        self.left
    }
    fn set_left(&mut self, l: *mut dyn PmatchObject) {
        self.left = l;
    }
    fn get_right(&self) -> *mut dyn PmatchObject {
        self.right
    }
    fn set_right(&mut self, r: *mut dyn PmatchObject) {
        self.right = r;
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-markup-container.evaluate-pair-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-markup-container.evaluate-pair-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-markup-container.evaluate-pair-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-markup-container.evaluate-pair-fn]
    unsafe fn evaluate_pair(&mut self) -> TransducerPointerPair {
        let mut retval: TransducerPointerPair;
        let loa: *mut HfstTransducer = (*self.left_of_arrow).evaluate();
        let lom: *mut HfstTransducer = (*self.left).evaluate();
        let rom: *mut HfstTransducer = (*self.right).evaluate();
        let tmpMappingPair: HfstTransducerPair = (
            HfstTransducer::new_copy(&*loa),
            HfstTransducer::new_type(format),
        );
        let marks: HfstTransducerPair = (
            HfstTransducer::new_copy(&*lom),
            HfstTransducer::new_copy(&*rom),
        );
        let MappingPair: HfstTransducerPair =
            create_mapping_for_mark_up_replace(&tmpMappingPair, &marks);
        drop(Box::from_raw(loa));
        drop(Box::from_raw(lom));
        drop(Box::from_raw(rom));
        retval = (
            Box::into_raw(Box::new(HfstTransducer::new_copy(&MappingPair.0))),
            Box::into_raw(Box::new(HfstTransducer::new_copy(&MappingPair.1))),
        );
        retval
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-mapping-pairs-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-mapping-pairs-container.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-mapping-pairs-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-mapping-pairs-container.evaluate-fn]
impl PmatchObject for PmatchMappingPairsContainer {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn set_name(&mut self, name: String) {
        self.name = name;
    }
    fn get_weight(&self) -> f64 {
        self.weight
    }
    fn set_weight(&mut self, weight: f64) {
        self.weight = weight;
    }
    fn get_line_defined(&self) -> i32 {
        self.line_defined
    }
    fn set_line_defined(&mut self, line_defined: i32) {
        self.line_defined = line_defined;
    }
    fn get_my_timer(&self) -> clock_t {
        self.my_timer
    }
    fn set_my_timer(&mut self, my_timer: clock_t) {
        self.my_timer = my_timer;
    }
    fn get_cache(&self) -> *mut HfstTransducer {
        self.cache
    }
    fn set_cache(&mut self, cache: *mut HfstTransducer) {
        self.cache = cache;
    }

    unsafe fn evaluate(&mut self) -> *mut HfstTransducer {
        pmatcherror("Should never happen\n");
        std::ptr::null_mut()
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-contexts-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-contexts-container.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-contexts-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-contexts-container.evaluate-fn]
impl PmatchObject for PmatchContextsContainer {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn set_name(&mut self, name: String) {
        self.name = name;
    }
    fn get_weight(&self) -> f64 {
        self.weight
    }
    fn set_weight(&mut self, weight: f64) {
        self.weight = weight;
    }
    fn get_line_defined(&self) -> i32 {
        self.line_defined
    }
    fn set_line_defined(&mut self, line_defined: i32) {
        self.line_defined = line_defined;
    }
    fn get_my_timer(&self) -> clock_t {
        self.my_timer
    }
    fn set_my_timer(&mut self, my_timer: clock_t) {
        self.my_timer = my_timer;
    }
    fn get_cache(&self) -> *mut HfstTransducer {
        self.cache
    }
    fn set_cache(&mut self, cache: *mut HfstTransducer) {
        self.cache = cache;
    }

    unsafe fn evaluate(&mut self) -> *mut HfstTransducer {
        pmatcherror("Should never happen\n");
        std::ptr::null_mut()
    }
}

// ===== body: atom-symbol-function-eval =====
// [spec:hfst:def:pmatch-utils.hfst.pmatch-object.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.evaluate-fn]
// The C++ overload 'PmatchObject::evaluate(std::vector<PmatchObject*> args)'
// (the base default for the trait 'evaluate_args'); this is the shared
// weight/cache handling. ('pmatchlineno' does not exist in the AST-walk port,
// so the diagnostic uses 'line_defined'.)
pub unsafe fn PmatchObject_evaluate_args(
    this: &mut dyn PmatchObject,
    args: Vec<*mut dyn PmatchObject>,
) -> *mut HfstTransducer {
    unsafe {
        if args.len() == 0 {
            if this.should_use_cache() {
                if this.get_cache().is_null() {
                    this.start_timing();
                    let c = this.evaluate();
                    this.set_cache(c);
                    this.report_time(String::new());
                }
                return Box::into_raw(Box::new(HfstTransducer::new_copy(&*this.get_cache())));
            } else {
                this.start_timing();
                let retval = this.evaluate();
                (*retval).minimize();
                this.report_time(String::new());
                return retval;
            }
        } else {
            let errstring = format!(
                "Object {} on line {} has no argument handling",
                this.get_name(),
                this.get_line_defined()
            );
            panic!("{}", errstring);
        }
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-object.expand-ins-arcs-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.expand-ins-arcs-fn]
pub unsafe fn PmatchObject_expand_Ins_arcs(this: &mut dyn PmatchObject, ss: &mut StringSet) {
    unsafe {
        let mut did_no_expansions = false;
        let mut expansions_done: StringSet = StringSet::new();
        let mut expanded_symbols: StringSet = StringSet::new();
        let this_name = this.get_name().to_string();
        if this_name.len() != 0 {
            let mut this_name_insed = String::from("@I.");
            this_name_insed.push_str(&this_name);
            this_name_insed.push_str("@");
            expansions_done.insert(this_name_insed);
        }
        while !did_no_expansions {
            did_no_expansions = true;
            for it in ss.iter() {
                if it.find("@I.") == Some(0) && it.rfind('@') == Some(it.len() - 1) {
                    // it's an Ins
                    if expansions_done.get(it).is_none() {
                        let ins_name = it[3..it.len() - 1].to_string();
                        did_no_expansions = false;
                        expansions_done.insert(it.clone());
                        if definitions().contains_key(&ins_name) {
                            let mut allowed: StringSet = StringSet::new();
                            let mut disallowed: StringSet = StringSet::new();
                            if def_insed_expressions().contains_key(&ins_name) {
                                (*def_insed_expressions()[&ins_name])
                                    .collect_initial_symbols_into(&mut allowed, &mut disallowed);
                            } else {
                                (*definitions()[&ins_name])
                                    .collect_initial_symbols_into(&mut allowed, &mut disallowed);
                            }
                            if allowed.len() != 0 {
                                for s in allowed.iter() {
                                    expanded_symbols.insert(s.clone());
                                }
                            } else {
                                expanded_symbols.insert(internal_identity.to_string());
                            }
                        }
                    }
                }
            }
        }
        for it in expansions_done.iter() {
            ss.remove(it);
        }
        for s in expanded_symbols.iter() {
            ss.insert(s.clone());
        }
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-object.get-real-initial-symbols-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.get-real-initial-symbols-fn]
pub unsafe fn PmatchObject_get_real_initial_symbols(this: &mut dyn PmatchObject) -> StringSet {
    unsafe {
        if this.is_left_concatenation_with_context() {
            return this.get_real_initial_symbols_from_right();
        }
        if this.is_delimiter() {
            return this.get_initial_symbols_from_unary_root();
        }
        let tmp: *mut HfstTransducer = this.evaluate();
        let retval: StringSet = (*tmp).get_initial_input_symbols();
        drop(Box::from_raw(tmp));
        retval
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-object.collect-initial-symbols-into-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.collect-initial-symbols-into-fn]
pub unsafe fn PmatchObject_collect_initial_symbols_into(
    this: &mut dyn PmatchObject,
    allowed_initial_symbols: &mut StringSet,
    disallowed_initial_symbols: &mut StringSet,
) {
    unsafe {
        // One or neither of allowed_initial_symbols and disallowed_initial_symbols
        // will have some symbols inserted to it.

        let mut allowed: StringSet = this.get_real_initial_symbols();
        let mut required: StringSet = this.get_initial_RC_initial_symbols();
        let mut disallowed: StringSet = this.get_initial_NRC_initial_symbols();

        // The first input symbols collected in this way may include:
        // - insertion arcs
        // - unknown, identity, default
        // - symbols after flag diacritics
        // - symbols from contexts
        PmatchObject_expand_Ins_arcs(this, &mut allowed);
        PmatchObject_expand_Ins_arcs(this, &mut required);
        PmatchObject_expand_Ins_arcs(this, &mut disallowed);

        if allowed.len() == 0 {
            // Probably something went wrong, we'll just not make no judgement
            return;
        }

        if string_set_has_meta_arc(&mut allowed) {
            if required.len() != 0 && !string_set_has_meta_arc(&mut required) {
                // RC sets a constraint
                for it in required.iter() {
                    if disallowed.get(it).is_none() {
                        allowed_initial_symbols.insert(it.clone());
                    }
                }
                return;
            } else {
                // Anything goes except what is disallowed
                if disallowed.len() == 0 || string_set_has_meta_arc(&mut disallowed) {
                    return;
                } else {
                    for s in disallowed.iter() {
                        disallowed_initial_symbols.insert(s.clone());
                    }
                    return;
                }
            }
        }

        // Now we can assume that "allowed" is nonempty and non-meta.

        if required.len() == 0 || string_set_has_meta_arc(&mut required) {
            // RC poses no constraint
            for it in allowed.iter() {
                if disallowed.get(it).is_none() {
                    allowed_initial_symbols.insert(it.clone());
                }
            }
            return;
        }

        // Now we can assume that there is a genuine RC constraint.

        for it in required.iter() {
            if allowed.get(it).is_some() && disallowed.get(it).is_none() {
                allowed_initial_symbols.insert(it.clone());
            }
        }
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-symbol.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-symbol.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-symbol.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-symbol.evaluate-fn]
impl PmatchObject for PmatchSymbol {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn set_name(&mut self, name: String) {
        self.name = name;
    }
    fn get_weight(&self) -> f64 {
        self.weight
    }
    fn set_weight(&mut self, weight: f64) {
        self.weight = weight;
    }
    fn get_line_defined(&self) -> i32 {
        self.line_defined
    }
    fn set_line_defined(&mut self, line_defined: i32) {
        self.line_defined = line_defined;
    }
    fn get_my_timer(&self) -> clock_t {
        self.my_timer
    }
    fn set_my_timer(&mut self, my_timer: clock_t) {
        self.my_timer = my_timer;
    }
    fn get_cache(&self) -> *mut HfstTransducer {
        self.cache
    }
    fn set_cache(&mut self, cache: *mut HfstTransducer) {
        self.cache = cache;
    }

    unsafe fn evaluate(&mut self) -> *mut HfstTransducer {
        unsafe {
            if self.name != "" {
                eval_stack().push(self.name.clone());
            }
            self.start_timing();
            let mut retval: *mut HfstTransducer = std::ptr::null_mut();
            if symbol_in_local_context(&mut self.sym) {
                retval = (*symbol_from_local_context(&mut self.sym)).evaluate();
            } else if symbol_in_global_context(&mut self.sym) {
                if flatten && def_insed_expressions().contains_key(&self.sym) {
                    retval = (*def_insed_expressions()[&self.sym]).evaluate();
                } else {
                    retval = (*symbol_from_global_context(&mut self.sym)).evaluate();
                }
                used_definitions().insert(self.sym.clone());
            } else {
                if verbose {
                    eprint!(
                        "Warning: interpreting undefined symbol \"{}\" as label on line {}\n",
                        self.sym, self.line_defined
                    );
                }
                retval = Box::into_raw(Box::new(HfstTransducer::new_symbol(&self.sym, format)));
            }
            (*retval).set_final_weights(double_to_float(self.weight), true);
            (*retval).minimize();
            self.report_time(String::new());
            if self.name != "" {
                eval_stack().pop();
            }
            retval
        }
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-symbol.evaluate-as-arg-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-symbol.evaluate-as-arg-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-symbol.evaluate-as-arg-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-symbol.evaluate-as-arg-fn]
    unsafe fn evaluate_as_arg(&mut self) -> *mut dyn PmatchObject {
        if symbol_in_local_context(&mut self.sym) {
            return (*symbol_from_local_context(&mut self.sym)).evaluate_as_arg();
        } else if symbol_in_global_context(&mut self.sym) {
            used_definitions().insert(self.sym.clone());
            if flatten && def_insed_expressions().contains_key(&self.sym) {
                return (*def_insed_expressions()[&self.sym]).evaluate_as_arg();
            } else {
                return (*symbol_from_global_context(&mut self.sym)).evaluate_as_arg();
            }
        } else {
            if verbose {
                eprint!(
                    "Warning: interpreting undefined symbol \"{}\" as label on line {}\n",
                    self.sym, self.line_defined
                );
            }
            return Box::into_raw(Box::new(PmatchString {
                name: String::new(),
                weight: 0.0,
                line_defined: 0,
                my_timer: 0,
                cache: std::ptr::null_mut(),
                string: self.sym.clone(),
                multichar: false,
            })) as *mut dyn PmatchObject;
        }
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-symbol.collect-strings-into-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-symbol.collect-strings-into-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-symbol.collect-strings-into-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-symbol.collect-strings-into-fn]
    fn collect_strings_into(&mut self, strings: &mut StringVector) {
        unsafe {
            if symbol_in_local_context(&mut self.sym) {
                (*symbol_from_local_context(&mut self.sym)).collect_strings_into(strings);
            } else if symbol_in_global_context(&mut self.sym) {
                (*symbol_from_global_context(&mut self.sym)).collect_strings_into(strings);
                used_definitions().insert(self.sym.clone());
            } else {
                strings.push(self.sym.clone());
            }
        }
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-string.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-string.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-string.evaluate-fn]
impl PmatchObject for PmatchString {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn set_name(&mut self, name: String) {
        self.name = name;
    }
    fn get_weight(&self) -> f64 {
        self.weight
    }
    fn set_weight(&mut self, weight: f64) {
        self.weight = weight;
    }
    fn get_line_defined(&self) -> i32 {
        self.line_defined
    }
    fn set_line_defined(&mut self, line_defined: i32) {
        self.line_defined = line_defined;
    }
    fn get_my_timer(&self) -> clock_t {
        self.my_timer
    }
    fn set_my_timer(&mut self, my_timer: clock_t) {
        self.my_timer = my_timer;
    }
    fn get_cache(&self) -> *mut HfstTransducer {
        self.cache
    }
    fn set_cache(&mut self, cache: *mut HfstTransducer) {
        self.cache = cache;
    }

    unsafe fn evaluate(&mut self) -> *mut HfstTransducer {
        unsafe {
            if !self.cache.is_null() {
                self.report_cache(String::new());
                return Box::into_raw(Box::new(HfstTransducer::new_copy(&*self.cache)));
            }
            self.start_timing();
            let tmp: *mut HfstTransducer;
            if self.multichar {
                let tok = HfstTokenizer::new();
                tmp = Box::into_raw(Box::new(HfstTransducer::new_tokenized(
                    &self.string,
                    &tok,
                    format,
                )));
            } else {
                tmp = Box::into_raw(Box::new(HfstTransducer::new_symbol(&self.string, format)));
            }
            (*tmp).set_final_weights(double_to_float(self.weight), true);
            if self.cache.is_null() && self.should_use_cache() == true {
                self.cache = tmp;
                (*self.cache).minimize();
                self.report_time(String::new());
                return Box::into_raw(Box::new(HfstTransducer::new_copy(&*self.cache)));
            }
            self.report_time(String::new());
            return tmp;
        }
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-string.collect-strings-into-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-string.collect-strings-into-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string.collect-strings-into-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-string.collect-strings-into-fn]
    fn collect_strings_into(&mut self, strings: &mut StringVector) {
        strings.push(self.string.clone());
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-string.evaluate-as-arg-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-string.evaluate-as-arg-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string.evaluate-as-arg-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-string.evaluate-as-arg-fn]
    unsafe fn evaluate_as_arg(&mut self) -> *mut dyn PmatchObject {
        Box::into_raw(Box::new(PmatchString {
            name: self.name.clone(),
            weight: self.weight,
            line_defined: self.line_defined,
            my_timer: self.my_timer,
            cache: self.cache,
            string: self.string.clone(),
            multichar: self.multichar,
        })) as *mut dyn PmatchObject
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-question-mark.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-question-mark.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-question-mark.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-question-mark.evaluate-fn]
impl PmatchObject for PmatchQuestionMark {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn set_name(&mut self, name: String) {
        self.name = name;
    }
    fn get_weight(&self) -> f64 {
        self.weight
    }
    fn set_weight(&mut self, weight: f64) {
        self.weight = weight;
    }
    fn get_line_defined(&self) -> i32 {
        self.line_defined
    }
    fn set_line_defined(&mut self, line_defined: i32) {
        self.line_defined = line_defined;
    }
    fn get_my_timer(&self) -> clock_t {
        self.my_timer
    }
    fn set_my_timer(&mut self, my_timer: clock_t) {
        self.my_timer = my_timer;
    }
    fn get_cache(&self) -> *mut HfstTransducer {
        self.cache
    }
    fn set_cache(&mut self, cache: *mut HfstTransducer) {
        self.cache = cache;
    }

    unsafe fn evaluate(&mut self) -> *mut HfstTransducer {
        unsafe {
            self.start_timing();
            let mut retval: *mut HfstTransducer = std::ptr::null_mut();
            retval = Box::into_raw(Box::new(HfstTransducer::new_symbol(
                internal_identity,
                format,
            )));
            (*retval).set_final_weights(double_to_float(self.weight), true);
            self.report_time(String::new());
            retval
        }
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-acceptor.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-acceptor.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-acceptor.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-acceptor.evaluate-fn]
impl PmatchObject for PmatchAcceptor {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn set_name(&mut self, name: String) {
        self.name = name;
    }
    fn get_weight(&self) -> f64 {
        self.weight
    }
    fn set_weight(&mut self, weight: f64) {
        self.weight = weight;
    }
    fn get_line_defined(&self) -> i32 {
        self.line_defined
    }
    fn set_line_defined(&mut self, line_defined: i32) {
        self.line_defined = line_defined;
    }
    fn get_my_timer(&self) -> clock_t {
        self.my_timer
    }
    fn set_my_timer(&mut self, my_timer: clock_t) {
        self.my_timer = my_timer;
    }
    fn get_cache(&self) -> *mut HfstTransducer {
        self.cache
    }
    fn set_cache(&mut self, cache: *mut HfstTransducer) {
        self.cache = cache;
    }

    unsafe fn evaluate(&mut self) -> *mut HfstTransducer {
        unsafe {
            self.start_timing();
            let mut retval: *mut HfstTransducer = std::ptr::null_mut();
            match self.set {
                PmatchPredefined::Alpha => {
                    if variables()
                        .entry("unicode-character-classes".to_string())
                        .or_default()
                        .as_str()
                        == "on"
                    {
                        retval = Box::into_raw(Box::new(HfstTransducer::new_symbol(
                            "@UNICODE_ALPHA@",
                            format,
                        )));
                    } else {
                        retval = Box::into_raw(Box::new(HfstTransducer::new_copy(
                            &*(*get_utils()).latin1_alpha_acceptor,
                        )));
                    }
                }
                PmatchPredefined::UppercaseAlpha => {
                    if variables()
                        .entry("unicode-character-classes".to_string())
                        .or_default()
                        .as_str()
                        == "on"
                    {
                        retval = Box::into_raw(Box::new(HfstTransducer::new_symbol(
                            "@UNICODE_UPPERALPHA@",
                            format,
                        )));
                    } else {
                        retval = Box::into_raw(Box::new(HfstTransducer::new_copy(
                            &*(*get_utils()).latin1_uppercase_acceptor,
                        )));
                    }
                }
                PmatchPredefined::LowercaseAlpha => {
                    if variables()
                        .entry("unicode-character-classes".to_string())
                        .or_default()
                        .as_str()
                        == "on"
                    {
                        retval = Box::into_raw(Box::new(HfstTransducer::new_symbol(
                            "@UNICODE_LOWERALPHA@",
                            format,
                        )));
                    } else {
                        retval = Box::into_raw(Box::new(HfstTransducer::new_copy(
                            &*(*get_utils()).latin1_lowercase_acceptor,
                        )));
                    }
                }
                PmatchPredefined::Numeral => {
                    retval = Box::into_raw(Box::new(HfstTransducer::new_copy(
                        &*(*get_utils()).latin1_numeral_acceptor,
                    )));
                }
                PmatchPredefined::Punctuation => {
                    retval = Box::into_raw(Box::new(HfstTransducer::new_copy(
                        &*(*get_utils()).latin1_punct_acceptor,
                    )));
                }
                PmatchPredefined::Whitespace => {
                    if variables()
                        .entry("unicode-character-classes".to_string())
                        .or_default()
                        .as_str()
                        == "on"
                    {
                        retval = Box::into_raw(Box::new(HfstTransducer::new_symbol(
                            "@UNICODE_WHITESPACE@",
                            format,
                        )));
                    } else {
                        retval = Box::into_raw(Box::new(HfstTransducer::new_copy(
                            &*(*get_utils()).latin1_whitespace_acceptor,
                        )));
                    }
                }
            }
            (*retval).set_final_weights(double_to_float(self.weight), true);
            self.report_time(String::new());
            retval
        }
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-empty.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-empty.evaluate-fn]
impl PmatchObject for PmatchEmpty {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn set_name(&mut self, name: String) {
        self.name = name;
    }
    fn get_weight(&self) -> f64 {
        self.weight
    }
    fn set_weight(&mut self, weight: f64) {
        self.weight = weight;
    }
    fn get_line_defined(&self) -> i32 {
        self.line_defined
    }
    fn set_line_defined(&mut self, line_defined: i32) {
        self.line_defined = line_defined;
    }
    fn get_my_timer(&self) -> clock_t {
        self.my_timer
    }
    fn set_my_timer(&mut self, my_timer: clock_t) {
        self.my_timer = my_timer;
    }
    fn get_cache(&self) -> *mut HfstTransducer {
        self.cache
    }
    fn set_cache(&mut self, cache: *mut HfstTransducer) {
        self.cache = cache;
    }

    unsafe fn evaluate(&mut self) -> *mut HfstTransducer {
        unsafe { Box::into_raw(Box::new(HfstTransducer::new_type(format))) }
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-epsilon-arc.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-epsilon-arc.evaluate-fn]
impl PmatchObject for PmatchEpsilonArc {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn set_name(&mut self, name: String) {
        self.name = name;
    }
    fn get_weight(&self) -> f64 {
        self.weight
    }
    fn set_weight(&mut self, weight: f64) {
        self.weight = weight;
    }
    fn get_line_defined(&self) -> i32 {
        self.line_defined
    }
    fn set_line_defined(&mut self, line_defined: i32) {
        self.line_defined = line_defined;
    }
    fn get_my_timer(&self) -> clock_t {
        self.my_timer
    }
    fn set_my_timer(&mut self, my_timer: clock_t) {
        self.my_timer = my_timer;
    }
    fn get_cache(&self) -> *mut HfstTransducer {
        self.cache
    }
    fn set_cache(&mut self, cache: *mut HfstTransducer) {
        self.cache = cache;
    }

    unsafe fn evaluate(&mut self) -> *mut HfstTransducer {
        unsafe {
            Box::into_raw(Box::new(HfstTransducer::new_symbol(
                internal_epsilon,
                format,
            )))
        }
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-transducer-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-transducer-container.evaluate-fn]
impl PmatchObject for PmatchTransducerContainer {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn set_name(&mut self, name: String) {
        self.name = name;
    }
    fn get_weight(&self) -> f64 {
        self.weight
    }
    fn set_weight(&mut self, weight: f64) {
        self.weight = weight;
    }
    fn get_line_defined(&self) -> i32 {
        self.line_defined
    }
    fn set_line_defined(&mut self, line_defined: i32) {
        self.line_defined = line_defined;
    }
    fn get_my_timer(&self) -> clock_t {
        self.my_timer
    }
    fn set_my_timer(&mut self, my_timer: clock_t) {
        self.my_timer = my_timer;
    }
    fn get_cache(&self) -> *mut HfstTransducer {
        self.cache
    }
    fn set_cache(&mut self, cache: *mut HfstTransducer) {
        self.cache = cache;
    }

    unsafe fn evaluate(&mut self) -> *mut HfstTransducer {
        unsafe {
            if (*self.t).get_type() != format {
                (*self.t).convert(format, String::new());
            }
            let retval = Box::into_raw(Box::new(HfstTransducer::new_copy(&*self.t)));
            (*retval).set_final_weights(double_to_float(self.weight), true);
            if self.name != "" {
                (*retval).set_name(&self.name);
            }
            retval
        }
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-function.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-function.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-function.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-function.evaluate-fn]
impl PmatchObject for PmatchFunction {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn set_name(&mut self, name: String) {
        self.name = name;
    }
    fn get_weight(&self) -> f64 {
        self.weight
    }
    fn set_weight(&mut self, weight: f64) {
        self.weight = weight;
    }
    fn get_line_defined(&self) -> i32 {
        self.line_defined
    }
    fn set_line_defined(&mut self, line_defined: i32) {
        self.line_defined = line_defined;
    }
    fn get_my_timer(&self) -> clock_t {
        self.my_timer
    }
    fn set_my_timer(&mut self, my_timer: clock_t) {
        self.my_timer = my_timer;
    }
    fn get_cache(&self) -> *mut HfstTransducer {
        self.cache
    }
    fn set_cache(&mut self, cache: *mut HfstTransducer) {
        self.cache = cache;
    }

    unsafe fn evaluate_args(&mut self, funargs: Vec<*mut dyn PmatchObject>) -> *mut HfstTransducer {
        unsafe {
            if verbose {
                self.my_timer = clock();
                named_object_evaluation_stack_depth += 1;
                write_compilation_stack_indentation_to_err();
                eprintln!("Evaluating call to {}...", self.name);
            }
            if funargs.len() != self.args.len() {
                let errstring = format!(
                    "Function {} expected {} args, got {}\n",
                    self.name,
                    self.args.len(),
                    funargs.len()
                );
                panic!("{}", errstring);
            }
            let mut local_env: BTreeMap<String, *mut dyn PmatchObject> = BTreeMap::new();
            if call_stack().len() != 0 {
                local_env = call_stack().last().unwrap().clone();
            }
            for i in 0..self.args.len() {
                local_env.insert(self.args[i].clone(), funargs[i]);
            }
            call_stack().push(local_env);
            if self.name != "" {
                eval_stack().push(self.name.clone());
            }
            let retval: *mut HfstTransducer = (*self.root).evaluate();
            if self.name != "" {
                eval_stack().pop();
            }
            (*retval).set_final_weights(double_to_float(self.weight), true);
            call_stack().pop();
            if verbose {
                let duration = (clock() - self.my_timer) as f64 / CLOCKS_PER_SEC as f64;
                write_compilation_stack_indentation_to_err();
                eprintln!("Call to {} evaluated in {} seconds", self.name, duration);
                named_object_evaluation_stack_depth -= 1;
            }
            retval
        }
    }

    unsafe fn evaluate(&mut self) -> *mut HfstTransducer {
        unsafe {
            let funargs: Vec<*mut dyn PmatchObject> = Vec::new();
            self.evaluate_args(funargs)
        }
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-funcall.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-funcall.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-funcall.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-funcall.evaluate-fn]
impl PmatchObject for PmatchFuncall {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn set_name(&mut self, name: String) {
        self.name = name;
    }
    fn get_weight(&self) -> f64 {
        self.weight
    }
    fn set_weight(&mut self, weight: f64) {
        self.weight = weight;
    }
    fn get_line_defined(&self) -> i32 {
        self.line_defined
    }
    fn set_line_defined(&mut self, line_defined: i32) {
        self.line_defined = line_defined;
    }
    fn get_my_timer(&self) -> clock_t {
        self.my_timer
    }
    fn set_my_timer(&mut self, my_timer: clock_t) {
        self.my_timer = my_timer;
    }
    fn get_cache(&self) -> *mut HfstTransducer {
        self.cache
    }
    fn set_cache(&mut self, cache: *mut HfstTransducer) {
        self.cache = cache;
    }

    unsafe fn evaluate(&mut self) -> *mut HfstTransducer {
        unsafe {
            if self.name != "" {
                eval_stack().push(self.name.clone());
            }
            let mut evaluated_args: Vec<*mut dyn PmatchObject> = Vec::new();
            for it in (*self.args).iter() {
                evaluated_args.push((**it).evaluate_as_arg());
            }
            let retval: *mut HfstTransducer = (*self.fun).evaluate_args(evaluated_args.clone());
            for it in evaluated_args.iter() {
                drop(Box::from_raw(*it));
            }
            if self.name != "" {
                eval_stack().pop();
            }
            retval
        }
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-builtin-function.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-builtin-function.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-builtin-function.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-builtin-function.evaluate-fn]
impl PmatchObject for PmatchBuiltinFunction {
    fn get_name(&self) -> &str {
        &self.name
    }
    fn set_name(&mut self, name: String) {
        self.name = name;
    }
    fn get_weight(&self) -> f64 {
        self.weight
    }
    fn set_weight(&mut self, weight: f64) {
        self.weight = weight;
    }
    fn get_line_defined(&self) -> i32 {
        self.line_defined
    }
    fn set_line_defined(&mut self, line_defined: i32) {
        self.line_defined = line_defined;
    }
    fn get_my_timer(&self) -> clock_t {
        self.my_timer
    }
    fn set_my_timer(&mut self, my_timer: clock_t) {
        self.my_timer = my_timer;
    }
    fn get_cache(&self) -> *mut HfstTransducer {
        self.cache
    }
    fn set_cache(&mut self, cache: *mut HfstTransducer) {
        self.cache = cache;
    }

    unsafe fn evaluate(&mut self) -> *mut HfstTransducer {
        unsafe {
            if self.name != "" {
                eval_stack().push(self.name.clone());
            }
            self.start_timing();
            let mut retval: *mut HfstTransducer = std::ptr::null_mut();
            if self.type_ == PmatchBuiltin::Interpolate {
                if (*self.args).len() < 3 {
                    let errstring = format!(
                        "Builtin function Interpolate called with {} arguments, but it expects at least 3.\n",
                        (*self.args).len()
                    );
                    panic!("{}", errstring);
                }
                // arguments are in reverse order after parsing
                let args = &mut *self.args;
                let n = args.len();
                retval = (*args[n - 2]).evaluate();
                let interpolator: *mut HfstTransducer = (*args[n - 1]).evaluate();
                for i in (0..(n - 2)).rev() {
                    let tmp: *mut HfstTransducer = (*args[i]).evaluate();
                    (*retval).concatenate(&*interpolator, true);
                    (*retval).concatenate(&*tmp, true);
                    drop(Box::from_raw(tmp));
                }
                drop(Box::from_raw(interpolator));
            }
            (*retval).set_final_weights(double_to_float(self.weight), true);
            self.report_time(String::new());
            if self.name != "" {
                eval_stack().pop();
            }
            retval
        }
    }
}

// ===== body: word-vectors-like =====
// Get the n best candidates in the original space using an insertion sort
// [spec:hfst:def:pmatch-utils.hfst.pmatch.get-top-n-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-top-n-fn]
pub fn get_top_n(
    n: usize,
    vecs: &Vec<WordVector>,
    comparison_point: &mut WordVector,
) -> Vec<(WordVector, WordVecFloat)> {
    let mut retval: Vec<(WordVector, WordVecFloat)> = Vec::new();
    for it in vecs.iter() {
        let cosdist: WordVecFloat = cosine_distance(it.clone(), comparison_point.clone());
        let mut i: usize = 0;
        while i <= retval.len() {
            if i == retval.len() {
                // We made it to the top
                retval.push((it.clone(), cosdist));
                break;
            } else {
                // Walking the list
                if cosdist >= retval[i].1 {
                    if i == 0 && retval.len() == n {
                        break;
                    }
                    retval.insert(i, (it.clone(), cosdist));
                    break;
                } else {
                    i += 1;
                    continue;
                }
            }
        }
        if retval.len() > n {
            retval.remove(0);
        }
    }
    retval
}
// Get the n best candidates in the transformed space using an insertion sort
// [spec:hfst:def:pmatch-utils.hfst.pmatch.get-top-n-transformed-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-top-n-transformed-fn]
pub fn get_top_n_transformed(
    n: usize,
    vecs: &Vec<WordVector>,
    plane_vec: Vec<WordVecFloat>,
    comparison_point: Vec<WordVecFloat>,
    translation_term: WordVecFloat,
    negative: bool,
) -> Vec<(WordVector, WordVecFloat)> {
    let mut retval: Vec<(WordVector, WordVecFloat)> = Vec::new();
    let plane_vec_square_sum: WordVecFloat = square_sum(plane_vec.clone());
    let comparison_point_norm: WordVecFloat = norm(comparison_point.clone());
    for it in vecs.iter() {
        let mut transformed_vec: WordVector = it.clone();

        /*
         * First, given a plane "plane_vec = translation term" and a point,
         * find the multiple of plane_vec which produces a vector going
         * from point to the nearest point in the plane.
         */

        let mut transformed_vec_scaler: WordVecFloat = (translation_term
            - dot_product(transformed_vec.vector.clone(), plane_vec.clone()))
            / plane_vec_square_sum;
        transformed_vec_scaler *= unsafe { vector_similarity_projection_factor };
        if negative {
            transformed_vec.vector = pointwise_minus(
                transformed_vec.vector.clone(),
                pointwise_multiplication(transformed_vec_scaler, plane_vec.clone()),
            );
        } else {
            transformed_vec.vector = pointwise_plus(
                transformed_vec.vector.clone(),
                pointwise_multiplication(transformed_vec_scaler, plane_vec.clone()),
            );
        }
        transformed_vec.norm = norm(transformed_vec.vector.clone());
        let cosdist: WordVecFloat = 1.0
            - dot_product(transformed_vec.vector.clone(), comparison_point.clone())
                / (transformed_vec.norm * comparison_point_norm);
        let mut i: usize = 0;
        while i <= retval.len() {
            if i == retval.len() {
                // We made it to the top
                retval.push((transformed_vec.clone(), cosdist));
                break;
            } else {
                // Walking the list
                if cosdist >= retval[i].1 {
                    if i == 0 && retval.len() == n {
                        break;
                    }
                    retval.insert(i, (transformed_vec.clone(), cosdist));
                    break;
                } else {
                    i += 1;
                    continue;
                }
            }
        }
        if retval.len() > n {
            retval.remove(0);
        }
    }
    retval
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pointwise-minus-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pointwise-minus-fn]
pub fn pointwise_minus(l: Vec<WordVecFloat>, r: Vec<WordVecFloat>) -> Vec<WordVecFloat> {
    let mut ret: Vec<WordVecFloat> = vec![0 as WordVecFloat; l.len()];
    for i in 0..l.len() {
        ret[i] = l[i] - r[i];
    }
    ret
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pointwise-plus-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pointwise-plus-fn]
pub fn pointwise_plus(l: Vec<WordVecFloat>, r: Vec<WordVecFloat>) -> Vec<WordVecFloat> {
    let mut ret: Vec<WordVecFloat> = vec![0 as WordVecFloat; l.len()];
    for i in 0..l.len() {
        ret[i] = l[i] + r[i];
    }
    ret
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pointwise-multiplication-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pointwise-multiplication-fn]
pub fn pointwise_multiplication(scalar: WordVecFloat, r: Vec<WordVecFloat>) -> Vec<WordVecFloat> {
    let mut ret: Vec<WordVecFloat> = vec![0 as WordVecFloat; r.len()];
    for i in 0..r.len() {
        ret[i] = scalar * r[i];
    }
    ret
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.dot-product-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.dot-product-fn]
pub fn dot_product(l: Vec<WordVecFloat>, r: Vec<WordVecFloat>) -> WordVecFloat {
    let mut ret: WordVecFloat = 0 as WordVecFloat;
    for i in 0..l.len() {
        ret += l[i] * r[i];
    }
    ret
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.square-sum-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.square-sum-fn]
pub fn square_sum(v: Vec<WordVecFloat>) -> WordVecFloat {
    let mut ret: WordVecFloat = 0 as WordVecFloat;
    for i in 0..v.len() {
        ret += v[i] * v[i];
    }
    ret
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.norm-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.norm-fn]
pub fn norm(v: Vec<WordVecFloat>) -> WordVecFloat {
    square_sum(v).sqrt()
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.cosine-distance-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.cosine-distance-fn]
pub fn cosine_distance(left: WordVector, right: WordVector) -> WordVecFloat {
    // Sometimes very nearby vectors combined with rounding error will produce
    // a slightly negative distance, so make sure to return at least 0.0
    let retval: WordVecFloat =
        1.0 - dot_product(left.vector, right.vector) / (left.norm * right.norm);
    (0.0 as WordVecFloat).max(retval)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.cosine-distance-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.cosine-distance-fn]
pub fn cosine_distance_vec(left: Vec<WordVecFloat>, right: Vec<WordVecFloat>) -> WordVecFloat {
    let retval: WordVecFloat =
        1.0 - dot_product(left.clone(), right.clone()) / (norm(left) * norm(right));
    (0.0 as WordVecFloat).max(retval)
}
// the general case
// [spec:hfst:def:pmatch-utils.hfst.pmatch.compile-like-arc-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.compile-like-arc-fn]
pub fn compile_like_arc(
    word1: String,
    word2: String,
    nwords: u32,
    is_negative: bool,
) -> *mut dyn PmatchObject {
    unsafe {
        let mut this_word1: WordVector = WordVector::default();
        let mut this_word2: WordVector = WordVector::default();
        {
            let mut it_iter = word_vectors().iter();
            loop {
                if !(this_word1.word.is_empty() || this_word2.word.is_empty()) {
                    break;
                }
                let it = match it_iter.next() {
                    Some(it) => it,
                    None => break,
                };
                if word1 == it.word {
                    this_word1 = it.clone();
                }
                if word2 == it.word {
                    this_word2 = it.clone();
                }
            }
        }
        if this_word1.word.is_empty() && this_word2.word.is_empty() {
            // got no matches
            let word1_o: *mut PmatchString = Box::into_raw(Box::new(PmatchString {
                name: String::new(),
                weight: 0.0,
                line_defined: 0,
                my_timer: 0,
                cache: std::ptr::null_mut(),
                string: word1,
                multichar: false,
            }));
            let word2_o: *mut PmatchString = Box::into_raw(Box::new(PmatchString {
                name: String::new(),
                weight: 0.0,
                line_defined: 0,
                my_timer: 0,
                cache: std::ptr::null_mut(),
                string: word2,
                multichar: false,
            }));
            (*word1_o).multichar = true;
            (*word2_o).multichar = true;
            pmatchwarning("no matches for arguments to Like() operation");
            let binop: *mut PmatchBinaryOperation =
                Box::into_raw(Box::new(PmatchBinaryOperation {
                    name: String::new(),
                    weight: 0.0,
                    line_defined: 0,
                    my_timer: 0,
                    cache: std::ptr::null_mut(),
                    op: PmatchBinaryOp::Disjunct,
                    left: word1_o as *mut dyn PmatchObject,
                    right: word2_o as *mut dyn PmatchObject,
                }));
            return binop as *mut dyn PmatchObject;
        }

        if this_word1.word.is_empty() || this_word2.word.is_empty() {
            // just one match
            pmatchwarning(
                "only one match for arguments to Like() operation, \
using nearest neighbours",
            );
            let mut this_word: WordVector = if this_word1.word.is_empty() {
                this_word2.clone()
            } else {
                this_word1.clone()
            };
            let top_n: Vec<(WordVector, WordVecFloat)> =
                get_top_n(nwords as usize, word_vectors(), &mut this_word);
            let tok: HfstTokenizer = HfstTokenizer::new();
            let retval: *mut HfstTransducer =
                Box::into_raw(Box::new(HfstTransducer::new_type(format)));
            if verbose {
                eprintln!("Inserting into Like({}):", this_word.word);
            }

            for i in 0..top_n.len() {
                if verbose {
                    eprintln!("  {}", top_n[i].0.word);
                }
                let mut tmp: HfstTransducer =
                    HfstTransducer::new_tokenized(&top_n[i].0.word, &tok, format);
                if include_cosine_distances {
                    tmp.set_final_weights(top_n[i].1, false);
                }
                (*retval).disjunct(&tmp, true);
            }
            let container: *mut PmatchTransducerContainer =
                Box::into_raw(Box::new(PmatchTransducerContainer {
                    name: String::new(),
                    weight: 0.0,
                    line_defined: 0,
                    my_timer: 0,
                    cache: std::ptr::null_mut(),
                    t: retval,
                }));
            return container as *mut dyn PmatchObject;
        }

        if variables()
            .entry("vector-similarity-projection-factor".to_string())
            .or_default()
            != "1.0"
        {
            let cstr =
                std::ffi::CString::new(variables()["vector-similarity-projection-factor"].clone())
                    .unwrap();
            vector_similarity_projection_factor =
                libc::strtod(cstr.as_ptr(), std::ptr::null_mut()) as WordVecFloat;
        }
        /*
         * When there are two vectors A and B, we compute the vector A - B that
         * goes from one to the other, and define a hyperplane orthogonal to that
         * vector that intersects the vector at the midpoint between the
         * two. We then add to all vectors a multiple of A - B to move them closer
         * to the plane, reducing the distance that is due to the difference
         * between A and B. (This is like projecting the space to the hyperplane
         * if we go all the way to the plane)
         *
         * The hyperplane is defined by the equation |B - A| = d, where d is a
         * translation term. |B - A| = 0 would be the set of vectors orthogonal to
         * |B - A|. We set d so that the distance from the hyperplane to A is
         * half of the norm of |B - A|.
         *
         */

        let B_minus_A: Vec<WordVecFloat> =
            pointwise_minus(this_word1.vector.clone(), this_word2.vector.clone());
        let hyperplane_translation_term: WordVecFloat =
            dot_product(B_minus_A.clone(), this_word1.vector.clone())
                - square_sum(B_minus_A.clone()) * 0.5;

        let comparison_point: Vec<WordVecFloat>;
        if is_negative == true {
            if verbose {
                eprintln!(
                    "Inserting into Unlike({}, {}):",
                    this_word1.word, this_word2.word
                );
            }
            let mut comparison_scaler: WordVecFloat = (hyperplane_translation_term
                - dot_product(this_word1.vector.clone(), B_minus_A.clone()))
                / square_sum(B_minus_A.clone());
            comparison_scaler *= vector_similarity_projection_factor;
            comparison_point = pointwise_minus(
                this_word1.vector.clone(),
                pointwise_multiplication(comparison_scaler, B_minus_A.clone()),
            );
        } else {
            if verbose {
                eprintln!(
                    "Inserting into Like({}, {}):",
                    this_word1.word, this_word2.word
                );
            }
            comparison_point = pointwise_plus(
                this_word2.vector.clone(),
                pointwise_multiplication(0.5 as WordVecFloat, B_minus_A.clone()),
            );
        }

        let top_n: Vec<(WordVector, WordVecFloat)> = get_top_n_transformed(
            nwords as usize,
            word_vectors(),
            B_minus_A.clone(),
            comparison_point,
            hyperplane_translation_term,
            is_negative,
        );
        let tok: HfstTokenizer = HfstTokenizer::new();
        let retval: *mut HfstTransducer = Box::into_raw(Box::new(HfstTransducer::new_type(format)));
        let mut i: usize = 0;
        while i < top_n.len() && i <= nwords as usize {
            if verbose {
                eprintln!("  {}", top_n[i].0.word);
            }
            let mut tmp: HfstTransducer =
                HfstTransducer::new_tokenized(&top_n[i].0.word, &tok, format);
            if include_cosine_distances {
                tmp.set_final_weights(top_n[i].1, false);
            }
            (*retval).disjunct(&tmp, true);
            // if (include_cosine_distances) {
            //     for (size_t j = i + 1; j < word_vectors.size() && j <= nwords;
            //     ++j) {
            //         HfstTransducer tmp2(word_vectors[i].word + "_cos_" +
            //         word_vectors[j].word, tok, format);
            //         tmp2.set_final_weights(cosine_distance(projected_i,
            //                                                get_projected_vector(word_vectors[j].vector,
            //                                                B_minus_A,
            //                                                hyperplane_translation_term)));
            //         retval->disjunct(tmp2);
            //     }
            // }
            i += 1;
        }
        let container: *mut PmatchTransducerContainer =
            Box::into_raw(Box::new(PmatchTransducerContainer {
                name: String::new(),
                weight: 0.0,
                line_defined: 0,
                my_timer: 0,
                cache: std::ptr::null_mut(),
                t: retval,
            }));
        container as *mut dyn PmatchObject
    }
}
// Single-word Like()
// [spec:hfst:def:pmatch-utils.hfst.pmatch.compile-like-arc-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.compile-like-arc-fn]
pub fn compile_like_arc_word(word: String, nwords: u32) -> *mut dyn PmatchObject {
    unsafe {
        let mut this_word: WordVector = WordVector::default();
        for it in word_vectors().iter() {
            if word == it.word {
                this_word = it.clone();
                break;
            }
        }
        if this_word.word.is_empty() {
            // got no matches
            let word_o: *mut PmatchString = Box::into_raw(Box::new(PmatchString {
                name: String::new(),
                weight: 0.0,
                line_defined: 0,
                my_timer: 0,
                cache: std::ptr::null_mut(),
                string: word,
                multichar: true,
            }));
            pmatchwarning("no matches for argument to Like() operation");
            return word_o as *mut dyn PmatchObject;
        }

        let top_n: Vec<(WordVector, WordVecFloat)> =
            get_top_n(nwords as usize, word_vectors(), &mut this_word);

        let tok: HfstTokenizer = HfstTokenizer::new();
        let retval: *mut HfstTransducer = Box::into_raw(Box::new(HfstTransducer::new_type(format)));
        if verbose {
            eprintln!("Inserting into Like({}):", word);
        }
        for i in 0..top_n.len() {
            if verbose {
                eprintln!("  {}", top_n[i].0.word);
            }
            let mut tmp: HfstTransducer =
                HfstTransducer::new_tokenized(&top_n[i].0.word, &tok, format);
            if include_cosine_distances {
                tmp.set_final_weights(top_n[i].1, false);
            }
            (*retval).disjunct(&tmp, true);
        }
        let container: *mut PmatchTransducerContainer =
            Box::into_raw(Box::new(PmatchTransducerContainer {
                name: String::new(),
                weight: 0.0,
                line_defined: 0,
                my_timer: 0,
                cache: std::ptr::null_mut(),
                t: retval,
            }));
        container as *mut dyn PmatchObject
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.read-vec-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.read-vec-fn]
pub fn read_vec(filename: String) {
    use std::io::Read;
    let mut binary_format = false;
    if filename.len() >= 4 && filename.rfind(".bin") == Some(filename.len() - 4) {
        binary_format = true;
    }
    if word_vectors().len() != 0 {
        word_vectors().clear();
        eprint!(
            "pmatch: vector model file {} overrides earlier one\n",
            filename
        );
    }
    let mut separator: u8 = b' ';
    let infile = match std::fs::File::open(&filename) {
        Ok(f) => f,
        Err(_) => {
            eprint!(
                "pmatch: could not open vector file {} for reading\n",
                filename
            );
            return;
        }
    };
    let mut infile = std::io::BufReader::new(infile);
    let mut all_bytes: Vec<u8> = Vec::new();
    if infile.read_to_end(&mut all_bytes).is_err() {
        eprint!(
            "pmatch: could not open vector file {} for reading\n",
            filename
        );
        return;
    }
    // Cursor over the raw file bytes, mirroring std::ifstream semantics.
    let mut cursor: usize = 0;
    // Read the header line (up to '\n').
    let header_line: String = {
        let start = cursor;
        while cursor < all_bytes.len() && all_bytes[cursor] != b'\n' {
            cursor += 1;
        }
        let line = String::from_utf8_lossy(&all_bytes[start..cursor]).into_owned();
        if cursor < all_bytes.len() {
            cursor += 1; // skip '\n'
        }
        line
    };
    let mut lexicon_size: usize = 0;
    let mut dimension: usize = 0;
    {
        // ss >> lexicon_size; ss.ignore(1); ss >> dimension;
        let bytes = header_line.as_bytes();
        let mut p = 0;
        while p < bytes.len() && (bytes[p] as char).is_whitespace() {
            p += 1;
        }
        let ls_start = p;
        while p < bytes.len() && (bytes[p] as char).is_ascii_digit() {
            p += 1;
        }
        lexicon_size = header_line[ls_start..p].parse::<usize>().unwrap_or(0);
        if p < bytes.len() {
            p += 1; // ss.ignore(1)
        }
        while p < bytes.len() && (bytes[p] as char).is_whitespace() {
            p += 1;
        }
        let d_start = p;
        while p < bytes.len() && (bytes[p] as char).is_ascii_digit() {
            p += 1;
        }
        dimension = header_line[d_start..p].parse::<usize>().unwrap_or(0);
    }
    word_vectors().reserve(lexicon_size + 1);
    let mut words_read: usize = 0;
    if binary_format {
        let vector_data_size: usize = std::mem::size_of::<f32>() * dimension;
        while cursor < all_bytes.len() && words_read <= lexicon_size {
            // The actual number of vectors is 1 more than lexicon_size
            // due to <s>
            // std::getline(infile, line, separator)
            let start = cursor;
            while cursor < all_bytes.len() && all_bytes[cursor] != separator {
                cursor += 1;
            }
            let line = String::from_utf8_lossy(&all_bytes[start..cursor]).into_owned();
            if cursor < all_bytes.len() {
                cursor += 1; // consume separator
            }
            // infile.read(&vector_data[0], vector_data_size)
            let read_end = std::cmp::min(cursor + vector_data_size, all_bytes.len());
            let vector_data: Vec<u8> = all_bytes[cursor..read_end].to_vec();
            cursor = read_end;
            // infile.ignore(1)
            if cursor < all_bytes.len() {
                cursor += 1;
            }
            let mut wv = WordVector::default();
            wv.word = line;
            // This will not compile is WordVectorFloat is not float,
            // in which case a conversion needs to happen, but
            // we can reasonably expect it to be a float for the
            // foreseeable future
            let mut comps: Vec<WordVecFloat> = Vec::new();
            let mut k = 0;
            while k + 4 <= vector_data.len() {
                let f = f32::from_ne_bytes([
                    vector_data[k],
                    vector_data[k + 1],
                    vector_data[k + 2],
                    vector_data[k + 3],
                ]);
                comps.push(f as WordVecFloat);
                k += 4;
            }
            wv.vector = comps;
            wv.norm = norm(wv.vector.clone());
            word_vectors().push(wv);
            words_read += 1;
        }
    } else {
        while cursor < all_bytes.len() && words_read <= lexicon_size {
            // std::getline(infile, line)
            let start = cursor;
            while cursor < all_bytes.len() && all_bytes[cursor] != b'\n' {
                cursor += 1;
            }
            let line = String::from_utf8_lossy(&all_bytes[start..cursor]).into_owned();
            if cursor < all_bytes.len() {
                cursor += 1; // skip '\n'
            }
            if line.is_empty() {
                continue;
            }
            words_read += 1;
            let line_bytes = line.as_bytes();
            let mut pos_opt: Option<usize> = line_bytes.iter().position(|&b| b == separator);
            if pos_opt.is_none() {
                separator = b'\t';
                pos_opt = line_bytes.iter().position(|&b| b == separator);
                if pos_opt.is_none() {
                    eprint!(
                        "pmatch warning: vector file {} doesn't appear to be tab- or \
space-separated\n  (reading line {})\n",
                        filename,
                        words_read + 1
                    );
                    break;
                }
            }
            let mut pos = pos_opt.unwrap();
            let word: String = line[0..pos].to_string();
            let mut components: Vec<WordVecFloat> = Vec::new();
            // while (npos != (nextpos = line.find(separator, pos + 1)))
            loop {
                let nextpos = line_bytes[pos + 1..]
                    .iter()
                    .position(|&b| b == separator)
                    .map(|i| i + pos + 1);
                match nextpos {
                    Some(nextpos) => {
                        // line.substr(pos + 1, nextpos - pos)
                        let sub_end = std::cmp::min(pos + 1 + (nextpos - pos), line.len());
                        let sub = &line[pos + 1..sub_end];
                        let cstr = std::ffi::CString::new(sub).unwrap_or_default();
                        let v = unsafe { libc::strtod(cstr.as_ptr(), std::ptr::null_mut()) }
                            as WordVecFloat;
                        components.push(v);
                        pos = nextpos;
                    }
                    None => break,
                }
            }
            // there can be one more from pos to the newline if there isn't a
            // separator at the end
            if line_bytes[line_bytes.len() - 1] != separator {
                let sub = &line[pos + 1..];
                let cstr = std::ffi::CString::new(sub).unwrap_or_default();
                let v =
                    unsafe { libc::strtof(cstr.as_ptr(), std::ptr::null_mut()) } as WordVecFloat;
                components.push(v);
            }
            if word_vectors().len() != 0 && word_vectors()[0].vector.len() != components.len() {
                eprint!(
                    "pmatch warning: vector file {} appears malformed\n  (reading line {})\n",
                    filename,
                    words_read + 1
                );
                continue;
            }
            let mut wv = WordVector::default();
            wv.word = word;
            wv.vector = components.clone();
            wv.norm = norm(components);
            word_vectors().push(wv);
        }
    }
    unsafe {
        if verbose {
            if word_vectors().len() == 0 {
                eprint!("Tried to read word vector file, empty result\n");
            }
            eprintln!(
                "Read {} vectors of dimensionality {}",
                word_vectors().len(),
                word_vectors()[0].vector.len()
            );
        }
    }
}
// [spec:hfst:def:pmatch-utils.hfst.read-args-fn]
// [spec:hfst:sem:pmatch-utils.hfst.read-args-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.read-args-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.read-args-fn]
pub unsafe fn read_args(filename: *mut c_char, argcount: u32) -> Vec<Vec<String>> {
    use std::io::Read;
    let fname: String = std::ffi::CStr::from_ptr(filename)
        .to_string_lossy()
        .into_owned();
    let mut retval: Vec<Vec<String>> = Vec::new();
    let mut current_tokens: Vec<String> = Vec::new();
    let opened = std::fs::File::open(&fname);
    match opened {
        Err(_) => {
            eprint!("Pmatch: could not open text file {} for reading\n", fname);
        }
        Ok(f) => {
            let mut infile = std::io::BufReader::new(f);
            let mut all_bytes: Vec<u8> = Vec::new();
            if infile.read_to_end(&mut all_bytes).is_err() {
                eprint!("Pmatch: could not open text file {} for reading\n", fname);
                return retval;
            }
            let mut cursor: usize = 0;
            let mut n: usize = 0;
            while cursor < all_bytes.len() {
                // std::getline(infile, line)
                let start = cursor;
                while cursor < all_bytes.len() && all_bytes[cursor] != b'\n' {
                    cursor += 1;
                }
                let line = String::from_utf8_lossy(&all_bytes[start..cursor]).into_owned();
                if cursor < all_bytes.len() {
                    cursor += 1; // skip '\n'
                }
                if !line.is_empty() {
                    current_tokens.clear();
                    n += 1;
                    let line_bytes = line.as_bytes();
                    let mut curpos: i32;
                    let mut nextpos: i32 = -1;
                    loop {
                        curpos = nextpos + 1;
                        // line.find_first_of(" ", curpos)
                        let found = if (curpos as usize) <= line_bytes.len() {
                            line_bytes[curpos as usize..]
                                .iter()
                                .position(|&b| b == b' ')
                                .map(|i| i + curpos as usize)
                        } else {
                            None
                        };
                        nextpos = crate::hfst_data_types::size_t_to_int(match found {
                            Some(p) => p,
                            None => usize::MAX,
                        });
                        // line.substr(curpos, nextpos - curpos)
                        let sub: String = if nextpos == -1 {
                            line[curpos as usize..].to_string()
                        } else {
                            line[curpos as usize..nextpos as usize].to_string()
                        };
                        current_tokens.push(sub);
                        if nextpos == -1 {
                            break;
                        }
                    }
                    if current_tokens.len() != argcount as usize {
                        eprintln!(
                            "Pmatch: line {} in {} contained {} tokens, expected {}",
                            n,
                            fname,
                            current_tokens.len(),
                            argcount
                        );
                    } else {
                        retval.push(current_tokens.clone());
                    }
                }
            }
        }
    }
    retval
}

// ===== body: fileio-compile-driver =====
// ---------------------------------------------------------------------------
// Bison-bridge globals/functions consumed by this group (the bison parser is
// replaced by the nfst-pmatch walk, but 'compile'/'init_globals'/
// 'expand_includes' still reference these). The integrator links them if
// another group provides them; otherwise these definitions stand in.
// ---------------------------------------------------------------------------

// [spec:hfst:def:pmatch-utils.pmatcherror-fn]
// [spec:hfst:sem:pmatch-utils.pmatcherror-fn]
pub static mut PMATCHNERRS: i32 = 0;
pub fn pmatchnerrs() -> &'static mut i32 {
    unsafe { &mut *std::ptr::addr_of_mut!(PMATCHNERRS) }
}
// [spec:hfst:def:pmatch-utils.pmatcherror-fn]
// [spec:hfst:sem:pmatch-utils.pmatcherror-fn]
// [spec:hfst:def:pmatch-utils.pmatchwarning-fn]
// [spec:hfst:sem:pmatch-utils.pmatchwarning-fn]
pub fn pmatchwarning(msg: &str) {
    if should_colourise() {
        eprintln!("\x1b[1m\x1b[33m***pmatch warning:\x1b[0m {}", msg);
    } else {
        eprintln!("***pmatch warning: {}", msg);
    }
}

pub fn pmatcherror(msg: &str) {
    unsafe {
        let parsedata: String;
        if !data.is_null() {
            let cstr = std::ffi::CStr::from_ptr(data);
            let bytes = cstr.to_bytes();
            if bytes.len() < 60 {
                parsedata = String::from_utf8_lossy(bytes).into_owned();
            } else {
                parsedata = String::from_utf8_lossy(&bytes[..59]).into_owned() + "... [truncated]";
            }
        } else {
            parsedata = String::new();
        }
        let mut errmsg = String::new();
        errmsg.push_str("hfst-pmatch:");
        errmsg.push_str("parsing failed: ");
        errmsg.push_str(msg);
        errmsg.push_str("\n*** parsing ");
        errmsg.push_str(&parsedata);
        errmsg.push_str("\n");

        // (Ported verbatim from pmatch_utils.cc:105) the upstream C++ leaks the
        // potentially large parse data here when the caller is not the command
        // line utility that exits after this.
        std::panic::panic_any(errmsg);
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.init-globals-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.init-globals-fn]
pub fn init_globals() {
    definitions().clear();
    variables().clear();
    variables().insert("count-patterns".to_string(), "off".to_string());
    variables().insert("delete-patterns".to_string(), "off".to_string());
    variables().insert("extract-patterns".to_string(), "off".to_string());
    variables().insert("locate-patterns".to_string(), "off".to_string());
    variables().insert("mark-patterns".to_string(), "on".to_string());
    variables().insert("max-context-length".to_string(), "254".to_string());
    variables().insert("max-recursion".to_string(), "5000".to_string());
    variables().insert("need-separators".to_string(), "on".to_string());
    variables().insert("unicode-character-classes".to_string(), "off".to_string());
    variables().insert("xerox-composition".to_string(), "on".to_string());
    variables().insert(
        "vector-similarity-projection-factor".to_string(),
        "1.0".to_string(),
    );
    call_stack().clear();
    eval_stack().clear();
    def_insed_expressions().clear();
    inserted_names().clear();
    unsatisfied_insertions().clear();
    used_definitions().clear();
    function_names().clear();
    capture_names().clear();
    zero_minimization_guard();
    unsafe {
        named_object_evaluation_stack_depth = 0;
        need_delimiters = false;
    }
    *pmatchnerrs() = 0;
    lst_line_map().clear();
    lst_overlap_warned().clear();
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.register-lst-line-numbers-from-transducer-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.register-lst-line-numbers-from-transducer-fn]
pub unsafe fn register_lst_line_numbers_from_transducer(t: *const HfstTransducer, line: i32) {
    if t.is_null() || line <= 0 {
        return;
    }
    let ss: StringSet = (*t).get_alphabet();
    for it in ss.iter() {
        if it.find("@L.") == Some(0) {
            // Keep first occurrence if seen before.
            if !lst_line_map().contains_key(it) {
                lst_line_map().insert(it.clone(), line);
            }
        }
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.expand-includes-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.expand-includes-fn]
pub fn expand_includes(script: &str) -> String {
    if !script.contains("@include\"") {
        return script.to_string();
    }
    let mut in_quoted_literal = false;
    let mut in_curly_literal = false;
    let mut in_comment = false;
    let bytes = script.as_bytes();
    let mut idx: usize = 0;
    let mut retval = String::new();
    while idx < bytes.len() {
        let c = bytes[idx] as char;
        if in_quoted_literal && c == '"' && (idx == 0 || bytes[idx - 1] != b'\\') {
            in_quoted_literal = false;
        } else if in_curly_literal && c == '}' && (idx == 0 || bytes[idx - 1] != b'\\') {
            in_curly_literal = false;
        } else if in_comment && c == '\n' {
            in_comment = false;
        } else if c == '"' {
            in_quoted_literal = true;
        } else if c == '{' {
            in_curly_literal = true;
        } else if c == '!' {
            in_comment = true;
        } else if c == '%' {
            retval.push(bytes[idx] as char);
            idx += 1;
            if idx < bytes.len() {
                retval.push(bytes[idx] as char);
                idx += 1;
            }
            continue;
        } else if bytes[idx..].starts_with(b"@include\"") {
            let terminating_quote_pos = script[idx + 9..].find('"').map(|p| p + idx + 9);
            if let Some(terminating_quote_pos) = terminating_quote_pos {
                let filename_start_pos = idx + 9;
                let filename_len = terminating_quote_pos - filename_start_pos;
                let filepath = unsafe {
                    let owned =
                        script[filename_start_pos..filename_start_pos + filename_len].to_string();
                    let cstr = std::ffi::CString::new(owned).unwrap();
                    path_from_filename(cstr.as_ptr())
                };
                match fs::read(&filepath) {
                    Ok(contents) => {
                        for b in contents {
                            retval.push(b as char);
                        }
                    }
                    Err(_) => {
                        let errstring = format!("could not open file {} for @include\n", filepath);
                        pmatcherror(&errstring);
                    }
                }
                idx += 10 + filename_len;
                continue;
            }
        }
        retval.push(bytes[idx] as char);
        idx += 1;
    }
    retval
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.compile-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.compile-fn]
pub unsafe fn compile(
    pmatch: &str,
    defs: &mut HashMap<String, *mut HfstTransducer>,
    impl_: ImplementationType,
    be_verbose: bool,
    do_flatten: bool,
    do_include_cosine_distances: bool,
    includedir_: String,
) -> HashMap<String, *mut HfstTransducer> {
    // lock here?
    init_globals();
    let expanded_script = expand_includes(pmatch);
    let data_cstring = std::ffi::CString::new(expanded_script.clone()).unwrap();
    data = libc::strdup(data_cstring.as_ptr());
    startptr = data;
    len = libc::strlen(data);
    verbose = be_verbose;
    flatten = do_flatten;
    include_cosine_distances = do_include_cosine_distances;
    *includedir() = includedir_;
    vector_similarity_projection_factor = 1.0;
    for (key, value) in defs.iter() {
        definitions().insert(
            key.clone(),
            PmatchTransducerContainer::new(*value) as *mut dyn PmatchObject,
        );
    }
    format = impl_;
    if verbose {
        timer = clock();
        eprintln!();
    }

    // === SEAM: replaces the bison 'pmatchparse()' call ====================
    // The build-driver group walks the nfst-pmatch parse tree, populating the
    // 'hfst::pmatch' globals exactly as the bison actions would have.
    match nfst_pmatch::parse(&expanded_script) {
        Ok(parsed) => {
            for statement in &parsed.value.statements {
                build_statement(statement);
            }
        }
        Err(_) => {
            *pmatchnerrs() += 1;
        }
    }
    // === END SEAM =========================================================

    libc::free(startptr as *mut libc::c_void);
    let mut retval: HashMap<String, *mut HfstTransducer> = HashMap::new();
    for it in unsatisfied_insertions().iter() {
        if !definitions().contains_key(it) {
            eprint!("Inserted transducer {} was never defined!\n", it);
            return retval;
        }
    }
    if verbose {
        let defs_keys: Vec<String> = definitions().keys().cloned().collect();
        for first in defs_keys.iter() {
            if !used_definitions().contains(first) && first != "TOP" {
                eprint!("Warning: {} defined but never used\n", first);
            }
        }
    }

    if *pmatchnerrs() != 0 {
        data = std::ptr::null_mut();
        len = 0;
        return retval;
    }
    // Our helper for harmonizing all the networks' alphabets with
    // each other
    if verbose {
        eprint!("\nCompiling and harmonizing...\n");
        timer = clock();
    }

    let mut uncount: u32 = 0;
    if inserted_names().len() > 0 || def_insed_expressions().len() > 0 || uncomposed().len() > 0 {
        let mut dummy = HfstTransducer::new_type(format);
        // We keep TOP and any inserted transducers
        let defs_keys: Vec<String> = definitions().keys().cloned().collect();
        for first in defs_keys.iter() {
            if first == "TOP"
                || inserted_names().contains(first)
                || def_insed_expressions().contains_key(first)
                || uncomposed().contains(first)
            {
                if verbose {
                    let second = *definitions().get(first).unwrap();
                    eprint!("\ndefinition...{}={}\n", first, (*second).get_name());
                }
                let tmp: *mut HfstTransducer;
                if def_insed_expressions().contains_key(first) {
                    tmp = (**def_insed_expressions().get(first).unwrap()).evaluate();
                } else {
                    tmp = (**definitions().get(first).unwrap()).evaluate();
                }
                (*tmp).minimize();
                dummy.harmonize(&mut *tmp, true);
                // This is what it will be called in the archive
                // XXX: seems to use the index not the name...)
                if uncomposed().contains(first) {
                    if verbose {
                        eprint!("\nUncompose\n");
                    }
                    if uncount == 0 {
                        (*tmp).set_name(&("UNCOMPOSE LEFT ".to_string() + first));
                        retval.insert("UNCOMPOSE LEFT ".to_string() + first, tmp);
                        uncount += 1;
                    } else if uncount == 1 {
                        (*tmp).set_name(&("UNCOMPOSE RIGHT ".to_string() + first));
                        retval.insert("UNCOMPOSE RIGHT ".to_string() + first, tmp);
                        uncount += 1;
                    } else {
                        eprint!("Uncompose only works once so far...\n");
                        uncount += 1;
                    }
                } else {
                    (*tmp).set_name(first);
                    retval.insert(first.clone(), tmp);
                }
            }
        }

        // Now that dummy is harmonized with everything, we harmonize
        // everything with dummy and minimize the results
        let retval_keys: Vec<String> = retval.keys().cloned().collect();
        for key in retval_keys.iter() {
            let second = *retval.get(key).unwrap();
            (*second).harmonize(&mut dummy, true);
            (*second).minimize();
        }
    } else {
        if definitions().len() == 0 {
            eprint!("warning: pmatch compilation had an empty result\n");
            retval.insert(
                "TOP".to_string(),
                Box::into_raw(Box::new(HfstTransducer::new_type(format))),
            );
        } else if !definitions().contains_key("TOP") {
            eprint!("Pmatch compilation warning: regex or TOP was undefined, using ");
            let first_key = definitions().keys().next().unwrap().clone();
            eprint!("{} as root\n", first_key);
            let tmp = (**definitions().get(&first_key).unwrap()).evaluate();
            (*tmp).minimize();
            (*tmp).set_name("TOP");
            retval.insert("TOP".to_string(), tmp);
        } else {
            let tmp = (**definitions().get("TOP").unwrap()).evaluate();
            (*tmp).minimize();
            (*tmp).set_name("TOP");
            retval.insert("TOP".to_string(), tmp);
        }
    }

    if verbose {
        let duration = (clock() - timer) as f64 / CLOCKS_PER_SEC as f64;
        timer = clock();
        eprint!(
            "Everything compiled and harmonized in {} seconds\n",
            duration
        );
    }

    let mut allowed_initial_symbols: StringSet = StringSet::new();
    let mut disallowed_initial_symbols: StringSet = StringSet::new();
    (**definitions().get("TOP").unwrap()).collect_initial_symbols_into(
        &mut allowed_initial_symbols,
        &mut disallowed_initial_symbols,
    );
    let mut initial_symbols_list = String::new();
    let mut disallowed_initial_symbols_list = String::new();
    // Use this to bail out if there's something suspicious in the final lists
    let mut initial_symbols_ok = true;
    for it in allowed_initial_symbols.iter() {
        if is_special(it) {
            if verbose {
                eprint!(
                    "Not setting initial symbol list due to special symbol {}\n",
                    it
                );
            }
            initial_symbols_ok = false;
        }
        initial_symbols_list.push_str(it);
    }
    for it in disallowed_initial_symbols.iter() {
        if is_special(it) {
            if verbose {
                eprint!(
                    "Not setting initial symbol list due to special symbol {}\n",
                    it
                );
            }
            initial_symbols_ok = false;
        }
        disallowed_initial_symbols_list.push_str(it);
    }
    if allowed_initial_symbols.len() > 200 {
        if verbose {
            eprint!(
                "Not setting initial symbol list due to excess length: {}\n",
                allowed_initial_symbols.len()
            );
        }
        initial_symbols_ok = false;
    }
    if disallowed_initial_symbols.len() > 200 {
        if verbose {
            eprint!(
                "Not setting initial symbol list due to excess length: {}\n",
                disallowed_initial_symbols.len()
            );
        }
        initial_symbols_ok = false;
    }
    if initial_symbols_ok && initial_symbols_list.len() != 0 {
        variables().insert("initial-symbols".to_string(), initial_symbols_list);
    }
    if initial_symbols_ok && disallowed_initial_symbols_list.len() != 0 {
        variables().insert(
            "disallowed-initial-symbols".to_string(),
            disallowed_initial_symbols_list,
        );
    }
    if variables().get("need-separators").map(|s| s.as_str()) == Some("on") {
        let mut not_whitespace = HfstTransducer::new_symbol(internal_identity, format);
        not_whitespace.subtract(&*(*get_utils()).latin1_whitespace_acceptor, true);
        let mut anything = HfstTransducer::new_symbol(internal_identity, format);
        anything.repeat_star();
        let mut begins_and_ends_with_non_whitespace = HfstTransducer::new_copy(&not_whitespace);
        begins_and_ends_with_non_whitespace.concatenate(&anything, true);
        begins_and_ends_with_non_whitespace.concatenate(&not_whitespace, true);
        begins_and_ends_with_non_whitespace.compose(&*(*retval.get("TOP").unwrap()), true);
        let mut is_single_non_whitespace = HfstTransducer::new_copy(&not_whitespace);
        is_single_non_whitespace.compose(&*(*retval.get("TOP").unwrap()), true);
        let empty = HfstTransducer::new_type(format);
        if begins_and_ends_with_non_whitespace.compare(&empty, true) == false
            || is_single_non_whitespace.compare(&empty, true) == false
        {
            let mut whitespace_punct_context =
                HfstTransducer::new_copy(&*(*get_utils()).latin1_whitespace_acceptor);
            whitespace_punct_context.disjunct(&*(*get_utils()).latin1_punct_acceptor, true);
            whitespace_punct_context
                .disjunct(&HfstTransducer::new_symbol("@BOUNDARY@", format), true);
            let top_with_boundaries: *mut HfstTransducer = Box::into_raw(Box::new(
                HfstTransducer::new_symbol_pair(internal_epsilon, LC_ENTRY_SYMBOL, format),
            ));
            (*top_with_boundaries).concatenate(&whitespace_punct_context, true);
            (*top_with_boundaries).concatenate(
                &HfstTransducer::new_symbol_pair(internal_epsilon, LC_EXIT_SYMBOL, format),
                true,
            );
            let mut rc = HfstTransducer::new_symbol_pair(internal_epsilon, RC_ENTRY_SYMBOL, format);
            rc.concatenate(&whitespace_punct_context, true);
            rc.concatenate(
                &HfstTransducer::new_symbol_pair(internal_epsilon, RC_EXIT_SYMBOL, format),
                true,
            );
            (*top_with_boundaries).concatenate(&*(*retval.get("TOP").unwrap()), true);
            (*top_with_boundaries).concatenate(&rc, true);
            drop(Box::from_raw(*retval.get("TOP").unwrap()));
            retval.insert(
                "TOP".to_string(),
                add_pmatch_delimiters(top_with_boundaries),
            );
            (*(*retval.get("TOP").unwrap())).minimize();
            if verbose {
                let duration = (clock() - timer) as f64 / CLOCKS_PER_SEC as f64;
                timer = clock();
                eprint!(
                    "Added automatic context separators in {} seconds\n",
                    duration
                );
            }
        }
    }
    let vars: Vec<(String, String)> = variables()
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    for (key, value) in vars.iter() {
        (*(*retval.get("TOP").unwrap())).set_property(key, value);
    }
    data = std::ptr::null_mut();
    len = 0;
    retval
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.write-compilation-stack-indentation-to-err-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.write-compilation-stack-indentation-to-err-fn]
pub fn write_compilation_stack_indentation_to_err() {
    unsafe {
        // Visually indicate nested definitions
        let mut i = 1;
        while i < named_object_evaluation_stack_depth {
            eprint!("|");
            i += 1;
        }
        if named_object_evaluation_stack_depth > 1 {
            eprint!(" ");
        }
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.read-text-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.read-text-fn]
pub fn read_text(
    filename: String,
    type_: ImplementationType,
    spaced_text: bool,
) -> *mut HfstTransducer {
    let tok = HfstTokenizer::new();
    let retval: *mut HfstTransducer = Box::into_raw(Box::new(HfstTransducer::new_type(type_)));
    match fs::read_to_string(&filename) {
        Err(_) => {
            eprint!(
                "Pmatch: could not open text file {} for reading\n",
                filename
            );
        }
        Ok(contents) => {
            let mut n: usize = 0;
            for line in contents.lines() {
                let line = line.to_string();
                if !line.is_empty() {
                    n += 1;
                    if spaced_text {
                        let _spv = HfstTokenizer::tokenize_space_separated(&line);
                    } else {
                        let spv = tok.tokenize(&line, false); // XXX
                        unsafe {
                            (*retval).disjunct_spv(&spv);
                        }
                    }
                }
            }
        }
    }
    retval
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.read-spaced-text-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.read-spaced-text-fn]
pub fn read_spaced_text(filename: String, type_: ImplementationType) -> *mut HfstTransducer {
    read_text(filename, type_, true)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.path-from-filename-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.path-from-filename-fn]
pub unsafe fn path_from_filename(filename: *const c_char) -> String {
    let mut retval = std::ffi::CStr::from_ptr(filename)
        .to_string_lossy()
        .into_owned();
    if includedir().len() > 0 && retval.len() > 0 {
        // includedir won't be > 0 under Windows until this mechanism is ported
        if retval.as_bytes()[0] != b'/' {
            // not an absolute dir
            retval.insert_str(0, includedir());
        }
    }
    retval
}
// [spec:hfst:def:pmatch-utils.hfst.print-unicode-codepoints-fn]
// [spec:hfst:sem:pmatch-utils.hfst.print-unicode-codepoints-fn]
pub fn print_unicode_codepoints(os: &mut dyn std::io::Write, s: &str) {
    let bytes = s.as_bytes();
    let mut i: usize = 0;
    while i < bytes.len() {
        let c: u8 = bytes[i];
        let mut codepoint: u32 = 0;
        let clen: usize;
        if (c & 0x80) == 0 {
            codepoint = c as u32;
            clen = 1;
        } else if (c & 0xE0) == 0xC0 {
            codepoint = (((c & 0x1F) as u32) << 6) | ((bytes[i + 1] & 0x3F) as u32);
            clen = 2;
        } else if (c & 0xF0) == 0xE0 {
            codepoint = (((c & 0x0F) as u32) << 12)
                | (((bytes[i + 1] & 0x3F) as u32) << 6)
                | ((bytes[i + 2] & 0x3F) as u32);
            clen = 3;
        } else if (c & 0xF8) == 0xF0 {
            codepoint = (((c & 0x07) as u32) << 18)
                | (((bytes[i + 1] & 0x3F) as u32) << 12)
                | (((bytes[i + 2] & 0x3F) as u32) << 6)
                | ((bytes[i + 3] & 0x3F) as u32);
            clen = 4;
        } else {
            codepoint = c as u32;
            clen = 1;
        }
        let _ = write!(os, "U+{:04X}", codepoint);
        i += clen;
        if i < bytes.len() {
            let _ = write!(os, ", ");
        }
    }
}

// ===== body: nfst-ast-builder =====
// ===========================================================================
// nfst -> C++ enum mappers
// ===========================================================================

fn map_unop(op: nfst_pmatch::UnaryOp) -> PmatchUnaryOp {
    use nfst_pmatch::UnaryOp as N;
    match op {
        N::Star => PmatchUnaryOp::RepeatStar,
        N::Plus => PmatchUnaryOp::RepeatPlus,
        N::Reverse => PmatchUnaryOp::Reverse,
        N::Invert => PmatchUnaryOp::Invert,
        N::UpperProject => PmatchUnaryOp::InputProject,
        N::LowerProject => PmatchUnaryOp::OutputProject,
        N::Complement => PmatchUnaryOp::Complement,
        N::TermComplement => PmatchUnaryOp::TermComplement,
        N::Containment => PmatchUnaryOp::Containment,
        N::ContainmentOnce => PmatchUnaryOp::ContainmentOnce,
        N::ContainmentOpt => PmatchUnaryOp::ContainmentOptional,
    }
}
fn map_acceptor(a: nfst_pmatch::Acceptor) -> PmatchPredefined {
    use nfst_pmatch::Acceptor as N;
    match a {
        N::Alpha => PmatchPredefined::Alpha,
        N::UppercaseAlpha => PmatchPredefined::UppercaseAlpha,
        N::LowercaseAlpha => PmatchPredefined::LowercaseAlpha,
        N::Num => PmatchPredefined::Numeral,
        N::Punct => PmatchPredefined::Punctuation,
        N::Whitespace => PmatchPredefined::Whitespace,
    }
}
fn map_arrow(a: nfst_pmatch::ReplaceArrow) -> ReplaceArrow {
    use nfst_pmatch::ReplaceArrow as N;
    match a {
        N::Right => ReplaceArrow::E_REPLACE_RIGHT,
        N::OptionalRight => ReplaceArrow::E_OPTIONAL_REPLACE_RIGHT,
        N::Left => ReplaceArrow::E_REPLACE_LEFT,
        N::OptionalLeft => ReplaceArrow::E_OPTIONAL_REPLACE_LEFT,
        N::LtrLongest => ReplaceArrow::E_LTR_LONGEST_MATCH,
        N::LtrShortest => ReplaceArrow::E_LTR_SHORTEST_MATCH,
        N::RtlLongest => ReplaceArrow::E_RTL_LONGEST_MATCH,
        N::RtlShortest => ReplaceArrow::E_RTL_SHORTEST_MATCH,
        // The pmatch replace grammar does not support <-> arrows; fall back.
        N::LeftRight => ReplaceArrow::E_REPLACE_RIGHT,
        N::OptionalLeftRight => ReplaceArrow::E_OPTIONAL_REPLACE_RIGHT,
    }
}
fn map_mark(m: nfst_pmatch::ContextMark) -> ReplaceType {
    use nfst_pmatch::ContextMark as N;
    match m {
        N::UpperUpper => ReplaceType::REPL_UP,
        N::LowerUpper => ReplaceType::REPL_RIGHT,
        N::UpperLower => ReplaceType::REPL_LEFT,
        N::LowerLower => ReplaceType::REPL_DOWN,
    }
}
fn map_caseop(op: nfst_pmatch::CaseOp, side: Option<nfst_pmatch::CaseSide>) -> PmatchUnaryOp {
    use PmatchUnaryOp::*;
    use nfst_pmatch::CaseOp as Op;
    use nfst_pmatch::CaseSide as S;
    match (op, side) {
        (Op::Cap, None) => Cap,
        (Op::Cap, Some(S::Upper)) => CapUpper,
        (Op::Cap, Some(S::Lower)) => CapLower,
        (Op::OptCap, None) => OptCap,
        (Op::OptCap, Some(S::Upper)) => OptCapUpper,
        (Op::OptCap, Some(S::Lower)) => OptCapLower,
        (Op::ToLower, None) => ToLower,
        (Op::ToLower, Some(S::Upper)) => ToLowerUpper,
        (Op::ToLower, Some(S::Lower)) => ToLowerLower,
        (Op::ToUpper, None) => ToUpper,
        (Op::ToUpper, Some(S::Upper)) => ToUpperUpper,
        (Op::ToUpper, Some(S::Lower)) => ToUpperLower,
        (Op::OptToLower, None) => OptToLower,
        (Op::OptToLower, Some(S::Upper)) => OptToLowerUpper,
        (Op::OptToLower, Some(S::Lower)) => OptToLowerLower,
        (Op::OptToUpper, None) => OptToUpper,
        (Op::OptToUpper, Some(S::Upper)) => OptToUpperUpper,
        (Op::OptToUpper, Some(S::Lower)) => OptToUpperLower,
        (Op::AnyCase, None) => AnyCase,
        (Op::AnyCase, Some(S::Upper)) => AnyCaseUpper,
        (Op::AnyCase, Some(S::Lower)) => AnyCaseLower,
    }
}
// ===========================================================================
// PmatchObject node constructors (mirror the C++ 'new PmatchX(...)' calls; all
// share the 'PmatchObject()' base-field initialisation: name="", weight=0.0,
// line_defined=pmatchlineno (unavailable in the walk -> 0), my_timer=0,
// cache=NULL).
// ===========================================================================

unsafe fn as_obj<T: PmatchObject + 'static>(p: *mut T) -> *mut dyn PmatchObject {
    p
}
unsafe fn pmb_symbol(sym: String) -> *mut dyn PmatchObject {
    Box::into_raw(Box::new(PmatchSymbol {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        my_timer: 0,
        cache: std::ptr::null_mut(),
        sym,
    }))
}
unsafe fn pmb_string(string: String, multichar: bool) -> *mut dyn PmatchObject {
    Box::into_raw(Box::new(PmatchString {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        my_timer: 0,
        cache: std::ptr::null_mut(),
        string,
        multichar,
    }))
}
unsafe fn pmb_question_mark() -> *mut dyn PmatchObject {
    Box::into_raw(Box::new(PmatchQuestionMark {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        my_timer: 0,
        cache: std::ptr::null_mut(),
    }))
}
unsafe fn pmb_empty() -> *mut dyn PmatchObject {
    Box::into_raw(Box::new(PmatchEmpty {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        my_timer: 0,
        cache: std::ptr::null_mut(),
    }))
}
unsafe fn pmb_epsilon_arc() -> *mut dyn PmatchObject {
    Box::into_raw(Box::new(PmatchEpsilonArc {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        my_timer: 0,
        cache: std::ptr::null_mut(),
    }))
}
unsafe fn pmb_acceptor(set: PmatchPredefined) -> *mut dyn PmatchObject {
    Box::into_raw(Box::new(PmatchAcceptor {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        my_timer: 0,
        cache: std::ptr::null_mut(),
        set,
    }))
}
unsafe fn pmb_unary(op: PmatchUnaryOp, root: *mut dyn PmatchObject) -> *mut dyn PmatchObject {
    Box::into_raw(Box::new(PmatchUnaryOperation {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        my_timer: 0,
        cache: std::ptr::null_mut(),
        op,
        root,
    }))
}
unsafe fn pmb_binary(
    op: PmatchBinaryOp,
    left: *mut dyn PmatchObject,
    right: *mut dyn PmatchObject,
) -> *mut dyn PmatchObject {
    Box::into_raw(Box::new(PmatchBinaryOperation {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        my_timer: 0,
        cache: std::ptr::null_mut(),
        op,
        left,
        right,
    }))
}
unsafe fn pmb_ternary(
    op: PmatchTernaryOp,
    left: *mut dyn PmatchObject,
    middle: *mut dyn PmatchObject,
    right: *mut dyn PmatchObject,
) -> *mut dyn PmatchObject {
    Box::into_raw(Box::new(PmatchTernaryOperation {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        my_timer: 0,
        cache: std::ptr::null_mut(),
        op,
        left,
        middle,
        right,
    }))
}
unsafe fn pmb_numeric(
    op: PmatchNumericOp,
    root: *mut dyn PmatchObject,
    values: Vec<i32>,
) -> *mut dyn PmatchObject {
    Box::into_raw(Box::new(PmatchNumericOperation {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        my_timer: 0,
        cache: std::ptr::null_mut(),
        op,
        root,
        values,
    }))
}
unsafe fn pmb_object_pair(
    left: *mut dyn PmatchObject,
    right: *mut dyn PmatchObject,
) -> *mut dyn PmatchObjectPairBase {
    Box::into_raw(Box::new(PmatchObjectPair { left, right }))
}
unsafe fn pmb_markup_container(
    left_of_arrow: *mut dyn PmatchObject,
    left: *mut dyn PmatchObject,
    right: *mut dyn PmatchObject,
) -> *mut dyn PmatchObjectPairBase {
    Box::into_raw(Box::new(PmatchMarkupContainer {
        left,
        right,
        left_of_arrow,
    }))
}
unsafe fn pmb_tc(t: *mut HfstTransducer) -> *mut PmatchTransducerContainer {
    Box::into_raw(Box::new(PmatchTransducerContainer {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        my_timer: 0,
        cache: std::ptr::null_mut(),
        t,
    }))
}
// ===========================================================================
// Small build helpers
// ===========================================================================

unsafe fn ins_transition(name: &str) -> String {
    let cs = std::ffi::CString::new(name).unwrap_or_default();
    get_Ins_transition(cs.as_ptr())
}
unsafe fn path_of(path: &str) -> String {
    let cs = std::ffi::CString::new(path).unwrap_or_default();
    path_from_filename(cs.as_ptr())
}
// STRINGLIKE: QUOTED_LITERAL -> PmatchString, CURLY_LITERAL -> PmatchString
// (multichar), SYMBOL -> PmatchSymbol (no used_definitions / empty-check).
unsafe fn build_stringlike(e: &nfst_pmatch::SpannedExpr) -> *mut dyn PmatchObject {
    use nfst_pmatch::PmatchExpr as PE;
    match &e.value {
        PE::QuotedLiteral(s) => pmb_string(s.clone(), false),
        PE::CurlyLiteral(s) => pmb_string(s.clone(), true),
        PE::Symbol(s) => pmb_symbol(s.clone()),
        _ => build_object(e),
    }
}
// CONCATENATED_STRING_LIST: right-folded Concatenate of STRINGLIKEs.
unsafe fn build_concatenated_string_list(
    items: &[nfst_pmatch::SpannedExpr],
) -> *mut dyn PmatchObject {
    let mut iter = items.iter().rev();
    let last = iter.next().expect("non-empty string list");
    let mut acc = build_stringlike(last);
    for it in iter {
        acc = pmb_binary(PmatchBinaryOp::Concatenate, build_stringlike(it), acc);
    }
    acc
}
// MappingSide -> object: Expr -> build, Dotted([..]) -> PmatchEpsilonArc,
// Dotted([. E .]) -> build.
unsafe fn side_to_obj(side: &nfst_pmatch::MappingSide) -> *mut dyn PmatchObject {
    use nfst_pmatch::MappingSide as MS;
    match side {
        MS::Expr(e) => build_object(e),
        MS::Dotted(None) => pmb_epsilon_arc(),
        MS::Dotted(Some(e)) => build_object(e),
    }
}
// READ_FROM productions: eagerly load the named file into a container.
unsafe fn build_read_file(kind: nfst_pmatch::ReadKind, path: &str) -> *mut dyn PmatchObject {
    use nfst_pmatch::ReadKind as RK;
    let filepath = path_of(path);
    match kind {
        RK::Binary => {
            // HfstInputStream-backed binary reading is deferred in the Rust
            // facade; mirror the structure as far as it goes.
            let mut instream = crate::hfst_input_stream::HfstInputStream::new_filename(&filepath);
            instream.close();
            let read = Box::into_raw(Box::new(HfstTransducer::new_type(format)));
            as_obj(pmb_tc(read))
        }
        RK::Text => as_obj(pmb_tc(read_text(
            filepath,
            ImplementationType::TROPICAL_OPENFST_TYPE,
            false,
        ))),
        RK::Spaced => as_obj(pmb_tc(read_spaced_text(
            filepath,
            ImplementationType::TROPICAL_OPENFST_TYPE,
        ))),
        RK::Prolog => {
            let cs = std::ffi::CString::new(filepath.clone()).unwrap_or_default();
            let mode = std::ffi::CString::new("r").unwrap();
            let f = libc::fopen(cs.as_ptr(), mode.as_ptr());
            if f.is_null() {
                eprintln!("File cannot be opened.");
                as_obj(pmb_tc(Box::into_raw(Box::new(HfstTransducer::new_type(
                    format,
                )))))
            } else {
                let mut linecount: u32 = 0;
                let tmp =
                    crate::hfst_basic_transducer::HfstBasicTransducer::read_in_prolog_format_file(
                        f,
                        &mut linecount,
                    );
                libc::fclose(f);
                let mut t = Box::new(HfstTransducer::new_from_basic_transducer(&tmp, format));
                t.minimize();
                as_obj(pmb_tc(Box::into_raw(t)))
            }
        }
        RK::Regex => {
            let mut regex = String::new();
            if let Ok(contents) = std::fs::read_to_string(&filepath) {
                for line in contents.lines() {
                    regex.push_str(line);
                }
            }
            if regex.is_empty() {
                eprintln!("Failed to read regex from {}.", filepath);
            }
            let mut xre_compiler = crate::xre::XreCompiler::new(format);
            as_obj(pmb_tc(xre_compiler.compile(&regex)))
        }
    }
}
/// Build a 'PmatchObject*' AST node from an 'nfst-pmatch' expression node.
pub unsafe fn build_object(e: &nfst_pmatch::SpannedExpr) -> *mut dyn PmatchObject {
    use nfst_pmatch::BinaryOp as B;
    use nfst_pmatch::PmatchExpr as PE;
    match &e.value {
        // ---- atoms ---------------------------------------------------------
        PE::Symbol(s) => {
            let sym = s.clone();
            if sym.len() == 0 {
                pmb_empty()
            } else {
                used_definitions().insert(sym.clone());
                pmb_symbol(sym)
            }
        }
        PE::Literal(s) => pmb_string(s.clone(), false),
        PE::QuotedLiteral(s) => pmb_string(s.clone(), false),
        PE::CurlyLiteral(s) => pmb_string(s.clone(), true),
        PE::Epsilon => pmb_string(crate::hfst_symbol_defs::internal_epsilon.to_string(), false),
        PE::BoundaryMarker => pmb_string("@BOUNDARY@".to_string(), false),
        PE::Any => pmb_question_mark(),
        PE::Acceptor(a) => pmb_acceptor(map_acceptor(*a)),
        PE::CharacterRange { from, to } => {
            let raw = format!("\"{}-{}\"", from, to);
            let cs = std::ffi::CString::new(raw).unwrap_or_default();
            as_obj(parse_range(cs.as_ptr()))
        }

        // ---- operators -----------------------------------------------------
        PE::Binary(op, l, r) => match op {
            B::Concatenate => pmb_binary(
                PmatchBinaryOp::Concatenate,
                build_object(l),
                build_object(r),
            ),
            B::Compose => pmb_binary(PmatchBinaryOp::Compose, build_object(l), build_object(r)),
            B::LenientCompose => pmb_binary(
                PmatchBinaryOp::LenientCompose,
                build_object(l),
                build_object(r),
            ),
            B::CrossProduct => pmb_binary(
                PmatchBinaryOp::CrossProduct,
                build_object(l),
                build_object(r),
            ),
            B::MergeRight => pmb_binary(PmatchBinaryOp::Merge, build_object(l), build_object(r)),
            B::MergeLeft => {
                // .<m. swaps the operands: Merge($3, $1).
                let lo = build_object(l);
                let ro = build_object(r);
                pmb_binary(PmatchBinaryOp::Merge, ro, lo)
            }
            B::Before => pmb_binary(PmatchBinaryOp::Before, build_object(l), build_object(r)),
            B::After => pmb_binary(PmatchBinaryOp::After, build_object(l), build_object(r)),
            B::Shuffle => pmb_binary(PmatchBinaryOp::Shuffle, build_object(l), build_object(r)),
            B::Union => pmb_binary(PmatchBinaryOp::Disjunct, build_object(l), build_object(r)),
            B::Intersect => pmb_binary(PmatchBinaryOp::Intersect, build_object(l), build_object(r)),
            B::Subtract => pmb_binary(PmatchBinaryOp::Subtract, build_object(l), build_object(r)),
            B::UpperSubtract => pmb_binary(
                PmatchBinaryOp::UpperSubtract,
                build_object(l),
                build_object(r),
            ),
            B::LowerSubtract => pmb_binary(
                PmatchBinaryOp::LowerSubtract,
                build_object(l),
                build_object(r),
            ),
            B::UpperPriorityUnion => pmb_binary(
                PmatchBinaryOp::UpperPriorityUnion,
                build_object(l),
                build_object(r),
            ),
            B::LowerPriorityUnion => pmb_binary(
                PmatchBinaryOp::LowerPriorityUnion,
                build_object(l),
                build_object(r),
            ),
            B::Ignoring => pmb_binary(
                PmatchBinaryOp::InsertFreely,
                build_object(l),
                build_object(r),
            ),
            B::IgnoreInternally => pmb_binary(
                PmatchBinaryOp::IgnoreInternally,
                build_object(l),
                build_object(r),
            ),
            B::LeftQuotient => {
                eprintln!("Left quotient not implemented");
                pmb_empty()
            }
        },
        PE::Unary(op, inner) => pmb_unary(map_unop(*op), build_object(inner)),

        // ---- grouping / weight / pair --------------------------------------
        PE::Group(inner) => build_object(inner),
        PE::Optional(inner) => pmb_unary(PmatchUnaryOp::Optionalize, build_object(inner)),
        PE::BracketedDotted(inner) => match inner {
            Some(b) => build_object(b),
            None => pmb_epsilon_arc(),
        },
        PE::Pair { upper, lower } => pmb_binary(
            PmatchBinaryOp::CrossProduct,
            build_object(upper),
            build_object(lower),
        ),
        PE::Weighted { expr, weight } => {
            let obj = build_object(expr);
            (*obj).set_weight((*obj).get_weight() + *weight);
            obj
        }

        // ---- catenate N ----------------------------------------------------
        PE::RepeatN(inner, n) => pmb_numeric(
            PmatchNumericOp::RepeatN,
            build_object(inner),
            vec![*n as i32],
        ),
        PE::RepeatNPlus(inner, n) => pmb_numeric(
            PmatchNumericOp::RepeatNPlus,
            build_object(inner),
            vec![*n as i32 + 1],
        ),
        PE::RepeatNMinus(inner, n) => pmb_numeric(
            PmatchNumericOp::RepeatNMinus,
            build_object(inner),
            vec![*n as i32 - 1],
        ),
        PE::RepeatNToK(inner, n, k) => pmb_numeric(
            PmatchNumericOp::RepeatNToK,
            build_object(inner),
            vec![*n as i32, *k as i32],
        ),

        // ---- replacement / restriction -------------------------------------
        PE::Replace { arrow, rules } => {
            let mapped_arrow = map_arrow(*arrow);
            let mut rule_ptrs: Vec<*mut PmatchReplaceRuleContainer> = Vec::new();
            for rule in rules.iter() {
                // MAPPINGPAIR_VECTOR -> mapping pairs
                let mut mapping: MappingPairVector = Vec::new();
                for mp in rule.mappings.iter() {
                    use nfst_pmatch::MappingKind as MK;
                    let pair: *mut dyn PmatchObjectPairBase = match &mp.kind {
                        MK::Plain { lower } => {
                            pmb_object_pair(side_to_obj(&mp.upper), side_to_obj(lower))
                        }
                        MK::Markup { pre, post } => {
                            let loa = side_to_obj(&mp.upper);
                            let lom = match pre {
                                Some(s) => side_to_obj(s),
                                None => pmb_epsilon_arc(),
                            };
                            let rom = match post {
                                Some(s) => side_to_obj(s),
                                None => pmb_epsilon_arc(),
                            };
                            pmb_markup_container(loa, lom, rom)
                        }
                    };
                    mapping.push(pair);
                }
                // CONTEXTS_WITH_MARK -> context pairs + type
                let (rtype, context): (ReplaceType, MappingPairVector) = match &rule.contexts {
                    Some(ctxs) => {
                        let mut context: MappingPairVector = Vec::new();
                        for c in ctxs.items.iter() {
                            let l = match &c.left {
                                Some(e) => build_object(e),
                                None => pmb_epsilon_arc(),
                            };
                            let r = match &c.right {
                                Some(e) => build_object(e),
                                None => pmb_epsilon_arc(),
                            };
                            context.push(pmb_object_pair(l, r));
                        }
                        (map_mark(ctxs.mark), context)
                    }
                    None => (ReplaceType::REPL_UP, Vec::new()),
                };
                rule_ptrs.push(Box::into_raw(Box::new(PmatchReplaceRuleContainer {
                    name: String::new(),
                    weight: 0.0,
                    line_defined: 0,
                    my_timer: 0,
                    cache: std::ptr::null_mut(),
                    arrow: mapped_arrow,
                    type_: rtype,
                    mapping,
                    context,
                })));
            }
            as_obj(Box::into_raw(Box::new(PmatchParallelRulesContainer {
                name: String::new(),
                weight: 0.0,
                line_defined: 0,
                my_timer: 0,
                cache: std::ptr::null_mut(),
                arrow: mapped_arrow,
                rules: rule_ptrs,
            })))
        }
        PE::Restriction { body, contexts } => {
            let left = build_object(body);
            let mut ctxs: MappingPairVector = Vec::new();
            for rc in contexts.iter() {
                let l: *mut dyn PmatchObject = match &rc.left {
                    Some(e) => build_object(e),
                    None => {
                        if rc.right.is_some() {
                            pmb_epsilon_arc()
                        } else {
                            pmb_empty()
                        }
                    }
                };
                let r: *mut dyn PmatchObject = match &rc.right {
                    Some(e) => build_object(e),
                    None => {
                        if rc.left.is_some() {
                            pmb_epsilon_arc()
                        } else {
                            pmb_empty()
                        }
                    }
                };
                ctxs.push(pmb_object_pair(l, r));
            }
            as_obj(Box::into_raw(Box::new(PmatchRestrictionContainer {
                name: String::new(),
                weight: 0.0,
                line_defined: 0,
                my_timer: 0,
                cache: std::ptr::null_mut(),
                left,
                contexts: Box::into_raw(Box::new(ctxs)),
            })))
        }

        // ---- pmatch-specific constructs ------------------------------------
        PE::Ins(name) => {
            if !flatten {
                if definitions().get(name).is_none() {
                    unsatisfied_insertions().insert(name.clone());
                }
                let retval = pmb_string(ins_transition(name), false);
                inserted_names().insert(name.clone());
                used_definitions().insert(name.clone());
                retval
            } else if definitions().get(name).is_some() {
                *definitions().get(name).unwrap()
            } else {
                eprintln!("Insertion of {} is undefined and --flatten is in use", name);
                pmb_empty()
            }
        }
        PE::EndTag(name) => {
            let retval = as_obj(make_end_tag(name.clone()));
            need_delimiters = true;
            retval
        }
        PE::Capture(name) => {
            let retval = as_obj(make_capture_tag(name.clone()));
            let captured = make_captured_tag(name.clone());
            if definitions().get(name).is_some() {
                warn(format!(
                    "definition of {} on line {} shadows earlier definition\n",
                    name, 0
                ));
                if let Some(old) = definitions().get(name).copied() {
                    let _ = Box::from_raw(old);
                }
            }
            definitions().insert(name.clone(), as_obj(captured));
            need_delimiters = true;
            retval
        }
        PE::Tag { body, name } => {
            // AddDelimiters(Concatenate(body, make_end_tag(name)))
            let cat = pmb_binary(
                PmatchBinaryOp::Concatenate,
                build_object(body),
                as_obj(make_end_tag(name.clone())),
            );
            pmb_unary(PmatchUnaryOp::AddDelimiters, cat)
        }
        PE::With { body, name, value } => {
            // Concatenate(Concatenate(entry, body), exit)
            let entry = make_with_tag_entry(name.clone(), value.clone());
            let exit = make_with_tag_exit(name.clone());
            let inner = pmb_binary(PmatchBinaryOp::Concatenate, entry, build_object(body));
            pmb_binary(PmatchBinaryOp::Concatenate, inner, exit)
        }
        PE::Counter(name) => as_obj(make_counter(name.clone())),
        PE::CaseOp { op, side, body } => pmb_unary(map_caseop(*op, *side), build_object(body)),
        PE::DefineWrapper(inner) => pmb_unary(PmatchUnaryOp::AddDelimiters, build_object(inner)),
        PE::Explode(items) => pmb_unary(
            PmatchUnaryOp::Explode,
            build_concatenated_string_list(items),
        ),
        PE::Implode(items) => pmb_unary(
            PmatchUnaryOp::Implode,
            build_concatenated_string_list(items),
        ),
        PE::Like {
            args,
            threshold,
            unlike,
        } => {
            // The C++ ARGLIST is in reverse source order; replicate.
            let rargs: Vec<String> = args.iter().rev().cloned().collect();
            let nwords = threshold.unwrap_or(10);
            if *unlike {
                if rargs.len() < 2 {
                    eprintln!(
                        "Unlike() operation takes exactly 2 arguments, got {}",
                        rargs.len()
                    );
                    pmb_empty()
                } else {
                    compile_like_arc(rargs[1].clone(), rargs[0].clone(), nwords, true)
                }
            } else {
                match rargs.len() {
                    0 => compile_like_arc_word(String::new(), 10),
                    1 => compile_like_arc_word(rargs[0].clone(), nwords),
                    _ => compile_like_arc(rargs[0].clone(), rargs[1].clone(), nwords, false),
                }
            }
        }
        PE::Lst(inner) => pmb_unary(PmatchUnaryOp::MakeList, build_object(inner)),
        PE::Exc(inner) => pmb_unary(PmatchUnaryOp::MakeExcList, build_object(inner)),
        PE::Sigma(inner) => pmb_unary(PmatchUnaryOp::MakeSigma, build_object(inner)),
        PE::Interpolate(items) => {
            // FUNCALL_ARGLIST is in reverse source order; replicate.
            let mut argvec: Vec<*mut dyn PmatchObject> = Vec::new();
            for it in items.iter().rev() {
                argvec.push(build_object(it));
            }
            as_obj(Box::into_raw(Box::new(PmatchBuiltinFunction {
                name: String::new(),
                weight: 0.0,
                line_defined: 0,
                my_timer: 0,
                cache: std::ptr::null_mut(),
                args: Box::into_raw(Box::new(argvec)),
                type_: PmatchBuiltin::Interpolate,
            })))
        }
        PE::Substitute(a, b, c) => pmb_ternary(
            PmatchTernaryOp::Substitute,
            build_object(a),
            build_object(b),
            build_object(c),
        ),
        PE::Uncompose(a, b, c) => {
            let left = build_stringlike(a);
            let middle = build_stringlike(b);
            let right = build_stringlike(c);
            uncomposed().insert((*middle).as_string());
            used_definitions().insert((*middle).as_string());
            uncomposed().insert((*right).as_string());
            used_definitions().insert((*right).as_string());
            pmb_ternary(PmatchTernaryOp::Uncompose, left, middle, right)
        }

        // ---- context conditions --------------------------------------------
        PE::Lc(inner) => {
            let retval = pmb_unary(PmatchUnaryOp::LC, build_object(inner));
            need_delimiters = true;
            retval
        }
        PE::Rc(inner) => {
            let retval = pmb_unary(PmatchUnaryOp::RC, build_object(inner));
            need_delimiters = true;
            retval
        }
        PE::Nlc(inner) => {
            let retval = pmb_unary(PmatchUnaryOp::NLC, build_object(inner));
            need_delimiters = true;
            retval
        }
        PE::Nrc(inner) => {
            let retval = pmb_unary(PmatchUnaryOp::NRC, build_object(inner));
            need_delimiters = true;
            retval
        }
        PE::OrContext(items) => {
            let mut result: Option<*mut dyn PmatchObject> = None;
            for it in items.iter() {
                let obj = build_object(it);
                result = match result {
                    None => Some(obj),
                    Some(prev) => Some(pmb_binary(PmatchBinaryOp::Disjunct, prev, obj)),
                };
            }
            // Zero the counter for making minimization guards for disjuncted
            // negative contexts.
            zero_minimization_guard();
            need_delimiters = true;
            result.unwrap_or_else(|| pmb_empty())
        }
        PE::AndContext(items) => {
            let mut result: Option<*mut dyn PmatchObject> = None;
            for it in items.iter() {
                let obj = build_object(it);
                result = match result {
                    None => Some(obj),
                    Some(prev) => Some(pmb_binary(PmatchBinaryOp::Concatenate, prev, obj)),
                };
            }
            need_delimiters = true;
            result.unwrap_or_else(|| pmb_empty())
        }

        // ---- function call -------------------------------------------------
        PE::Call { name, args } => {
            let sym = name.clone();
            let result = if function_names().get(name).is_none() {
                eprintln!("Function {} hasn't been defined", sym);
                pmb_string(String::new(), false)
            } else {
                let mut sym_lookup = sym.clone();
                let fun = symbol_from_global_context(&mut sym_lookup).cast::<PmatchFunction>();
                let mut argvec: Vec<*mut dyn PmatchObject> = Vec::new();
                for a in args.iter().rev() {
                    argvec.push(build_object(a));
                }
                as_obj(Box::into_raw(Box::new(PmatchFuncall {
                    name: String::new(),
                    weight: 0.0,
                    line_defined: 0,
                    my_timer: 0,
                    cache: std::ptr::null_mut(),
                    args: Box::into_raw(Box::new(argvec)),
                    fun,
                })))
            };
            used_definitions().insert(sym);
            result
        }

        // ---- file references -----------------------------------------------
        PE::ReadFile { kind, path } => build_read_file(*kind, path),
        PE::ReadLexc(path) => {
            let filepath = path_of(path);
            as_obj(pmb_tc(HfstTransducer::read_lexc_ptr(
                &filepath, format, verbose,
            )))
        }
        PE::ReadVec(path) => {
            let filepath = path_of(path);
            read_vec(filepath);
            pmb_empty()
        }
    }
}
// EXPRESSION1: EXPRESSION2 END_OF_WEIGHTED_EXPRESSION { weight += w; wrap in
// AddDelimiters if need_delimiters; reset need_delimiters. } The trailing
// weight is folded into a 'Weighted' node by nfst, so only the delimiter wrap
// remains here.
unsafe fn build_expression1(body: &nfst_pmatch::SpannedExpr) -> *mut dyn PmatchObject {
    let obj = build_object(body);
    let result = if need_delimiters {
        pmb_unary(PmatchUnaryOp::AddDelimiters, obj)
    } else {
        obj
    };
    need_delimiters = false;
    result
}
// PMATCH DEFINITION verbose timer report.
unsafe fn report_defined(name: &str) {
    if verbose {
        let duration = (clock() - timer) as f64 / CLOCKS_PER_SEC as f64;
        timer = clock();
        eprintln!("defined {} in {:.2} seconds", name, duration);
    }
}
// PMATCH DEFINITION { shadow check + insert }.
unsafe fn insert_definition(name: String, obj: *mut dyn PmatchObject) {
    if definitions().get(&name).is_some() {
        warn(format!(
            "definition of {} on line {} shadows earlier definition\n",
            name, 0
        ));
        if let Some(old) = definitions().get(&name).copied() {
            let _ = Box::from_raw(old);
        }
    }
    definitions().insert(name, obj);
}
/// Apply one top-level 'nfst-pmatch' statement (definition / def-ins /
/// regex-top / set-variable / list-definition / read-vec), populating the
/// 'hfst::pmatch' globals.
pub unsafe fn build_statement(s: &nfst_pmatch::Spanned<nfst_pmatch::PmatchStatement>) {
    use nfst_pmatch::PmatchStatement as PS;
    use nfst_pmatch::VariableValue;
    match &s.value {
        PS::Define { name, params, body } => match params {
            None => {
                let obj = build_expression1(body);
                (*obj).set_name(name.clone());
                report_defined(name);
                insert_definition(name.clone(), obj);
            }
            Some(args) => {
                let root = build_expression1(body);
                // The C++ ARGLIST is in reverse source order; replicate.
                let fun = Box::into_raw(Box::new(PmatchFunction {
                    name: String::new(),
                    weight: 0.0,
                    line_defined: 0,
                    my_timer: 0,
                    cache: std::ptr::null_mut(),
                    args: args.iter().rev().cloned().collect(),
                    root,
                }));
                (*fun).name = name.clone();
                function_names().insert(name.clone());
                report_defined(name);
                insert_definition(name.clone(), as_obj(fun));
            }
        },
        PS::DefIns { name, body } => {
            let body_obj = build_expression1(body);
            (*body_obj).set_name(name.clone());
            def_insed_expressions().insert(name.clone(), body_obj);
            let def_value = pmb_string(ins_transition(name), false);
            report_defined(name);
            insert_definition(name.clone(), def_value);
        }
        PS::RegexTop { body } => {
            let obj = build_expression1(body);
            (*obj).set_name("TOP".to_string());
            report_defined("TOP");
            insert_definition("TOP".to_string(), obj);
        }
        PS::SetVariable { name, value } => {
            let v = match value {
                VariableValue::Symbol(s) => s.clone(),
                VariableValue::Epsilon => "0".to_string(),
            };
            variables().insert(name.clone(), v);
        }
        PS::ListDefinition { name, body } => {
            // DEFINED_LIST: the name lands on the inner body, the stored value
            // is the MakeSigma wrapper.
            let inner = build_expression1(body);
            (*inner).set_name(name.clone());
            let value = pmb_unary(PmatchUnaryOp::MakeSigma, inner);
            report_defined(name);
            insert_definition(name.clone(), value);
        }
        PS::ReadVec { path } => {
            let filepath = path_of(path);
            read_vec(filepath);
        }
    }
}
impl PmatchCompiler {
    pub fn new(type_: ImplementationType) -> Self {
        PmatchCompiler {
            type_,
            verbose: false,
            flatten: false,
            include_cosine_distances: false,
            includedir: String::new(),
        }
    }

    // Mirrors 'hfst::pmatch::compile', with the bison 'pmatchparse()' step
    // replaced by a walk over the 'nfst-pmatch' AST (the sanctioned deviation).
    pub fn compile(&mut self, src: &str) -> HashMap<String, *mut HfstTransducer> {
        unsafe {
            init_globals();
            let expanded_script = expand_includes(src);
            verbose = self.verbose;
            flatten = self.flatten;
            include_cosine_distances = self.include_cosine_distances;
            *includedir() = self.includedir.clone();
            vector_similarity_projection_factor = 1.0;
            format = self.type_;
            if verbose {
                timer = clock();
                eprintln!();
            }

            // ---- bison-replacement walk ------------------------------------
            match nfst_pmatch::parse(&expanded_script) {
                Ok(file) => {
                    for stmt in &file.value.statements {
                        build_statement(stmt);
                    }
                }
                Err(_e) => {
                    data = std::ptr::null_mut();
                    len = 0;
                    return HashMap::new();
                }
            }

            let mut retval: HashMap<String, *mut HfstTransducer> = HashMap::new();

            for it in unsatisfied_insertions().iter() {
                if definitions().get(it).is_none() {
                    eprintln!("Inserted transducer {} was never defined!", it);
                    data = std::ptr::null_mut();
                    len = 0;
                    return retval;
                }
            }
            if verbose {
                for (k, _v) in definitions().iter() {
                    if used_definitions().get(k).is_none() && k != "TOP" {
                        eprintln!("Warning: {} defined but never used", k);
                    }
                }
            }

            if verbose {
                eprintln!("\nCompiling and harmonizing...");
                timer = clock();
            }

            let mut uncount: u32 = 0;
            if !inserted_names().is_empty()
                || !def_insed_expressions().is_empty()
                || !uncomposed().is_empty()
            {
                let mut dummy = HfstTransducer::new_type(format);
                let keys: Vec<String> = definitions().keys().cloned().collect();
                for key in &keys {
                    if key == "TOP"
                        || inserted_names().get(key).is_some()
                        || def_insed_expressions().get(key).is_some()
                        || uncomposed().get(key).is_some()
                    {
                        let obj_ptr: *mut dyn PmatchObject =
                            if def_insed_expressions().get(key).is_some() {
                                *def_insed_expressions().get(key).unwrap()
                            } else {
                                *definitions().get(key).unwrap()
                            };
                        let tmp = (*obj_ptr).evaluate();
                        (*tmp).minimize();
                        dummy.harmonize(&mut *tmp, false);
                        if uncomposed().get(key).is_some() {
                            if uncount == 0 {
                                (*tmp).set_name(&format!("UNCOMPOSE LEFT {}", key));
                                retval.insert(format!("UNCOMPOSE LEFT {}", key), tmp);
                                uncount += 1;
                            } else if uncount == 1 {
                                (*tmp).set_name(&format!("UNCOMPOSE RIGHT {}", key));
                                retval.insert(format!("UNCOMPOSE RIGHT {}", key), tmp);
                                uncount += 1;
                            } else {
                                eprintln!("Uncompose only works once so far...");
                                uncount += 1;
                            }
                        } else {
                            (*tmp).set_name(key);
                            retval.insert(key.clone(), tmp);
                        }
                    }
                }
                for (_k, v) in retval.iter_mut() {
                    (**v).harmonize(&mut dummy, false);
                    (**v).minimize();
                }
            } else if definitions().is_empty() {
                eprintln!("warning: pmatch compilation had an empty result");
                retval.insert(
                    "TOP".to_string(),
                    Box::into_raw(Box::new(HfstTransducer::new_type(format))),
                );
            } else if definitions().get("TOP").is_none() {
                let (first_key, first_obj) = {
                    let mut it = definitions().iter();
                    let (k, v) = it.next().unwrap();
                    (k.clone(), *v)
                };
                eprintln!(
                    "Pmatch compilation warning: regex or TOP was undefined, using {} as root",
                    first_key
                );
                let tmp = (*first_obj).evaluate();
                (*tmp).minimize();
                (*tmp).set_name("TOP");
                retval.insert("TOP".to_string(), tmp);
            } else {
                let top_obj = *definitions().get("TOP").unwrap();
                let tmp = (*top_obj).evaluate();
                (*tmp).minimize();
                (*tmp).set_name("TOP");
                retval.insert("TOP".to_string(), tmp);
            }

            if verbose {
                let duration = (clock() - timer) as f64 / CLOCKS_PER_SEC as f64;
                timer = clock();
                eprintln!("Everything compiled and harmonized in {} seconds", duration);
            }

            let mut allowed_initial_symbols: StringSet = StringSet::new();
            let mut disallowed_initial_symbols: StringSet = StringSet::new();
            if let Some(top) = definitions().get("TOP").copied() {
                (*top).collect_initial_symbols_into(
                    &mut allowed_initial_symbols,
                    &mut disallowed_initial_symbols,
                );
            }
            let mut initial_symbols_list = String::new();
            let mut disallowed_initial_symbols_list = String::new();
            let mut initial_symbols_ok = true;
            for it in allowed_initial_symbols.iter() {
                if is_special(it) {
                    if verbose {
                        eprintln!(
                            "Not setting initial symbol list due to special symbol {}",
                            it
                        );
                    }
                    initial_symbols_ok = false;
                }
                initial_symbols_list.push_str(it);
            }
            for it in disallowed_initial_symbols.iter() {
                if is_special(it) {
                    if verbose {
                        eprintln!(
                            "Not setting initial symbol list due to special symbol {}",
                            it
                        );
                    }
                    initial_symbols_ok = false;
                }
                disallowed_initial_symbols_list.push_str(it);
            }
            if allowed_initial_symbols.len() > 200 {
                if verbose {
                    eprintln!(
                        "Not setting initial symbol list due to excess length: {}",
                        allowed_initial_symbols.len()
                    );
                }
                initial_symbols_ok = false;
            }
            if disallowed_initial_symbols.len() > 200 {
                if verbose {
                    eprintln!(
                        "Not setting initial symbol list due to excess length: {}",
                        disallowed_initial_symbols.len()
                    );
                }
                initial_symbols_ok = false;
            }
            if initial_symbols_ok && !initial_symbols_list.is_empty() {
                variables().insert("initial-symbols".to_string(), initial_symbols_list);
            }
            if initial_symbols_ok && !disallowed_initial_symbols_list.is_empty() {
                variables().insert(
                    "disallowed-initial-symbols".to_string(),
                    disallowed_initial_symbols_list,
                );
            }

            if variables().get("need-separators").map(|s| s.as_str()) == Some("on") {
                let mut not_whitespace =
                    HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity, format);
                not_whitespace.subtract(&*(*get_utils()).latin1_whitespace_acceptor, true);
                let mut anything =
                    HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity, format);
                anything.repeat_star();
                let mut begins_and_ends_with_non_whitespace =
                    HfstTransducer::new_from_transducer(&not_whitespace);
                begins_and_ends_with_non_whitespace.concatenate(&anything, true);
                begins_and_ends_with_non_whitespace.concatenate(&not_whitespace, true);
                let top_ptr = *retval.get("TOP").unwrap();
                begins_and_ends_with_non_whitespace.compose(&*top_ptr, true);
                let mut is_single_non_whitespace =
                    HfstTransducer::new_from_transducer(&not_whitespace);
                is_single_non_whitespace.compose(&*top_ptr, true);
                let empty = HfstTransducer::new_type(format);
                if !begins_and_ends_with_non_whitespace.compare(&empty, true)
                    || !is_single_non_whitespace.compare(&empty, true)
                {
                    let mut whitespace_punct_context = HfstTransducer::new_from_transducer(
                        &*(*get_utils()).latin1_whitespace_acceptor,
                    );
                    whitespace_punct_context.disjunct(&*(*get_utils()).latin1_punct_acceptor, true);
                    whitespace_punct_context
                        .disjunct(&HfstTransducer::new_symbol("@BOUNDARY@", format), true);
                    let top_with_boundaries =
                        Box::into_raw(Box::new(HfstTransducer::new_symbol_pair(
                            crate::hfst_symbol_defs::internal_epsilon,
                            LC_ENTRY_SYMBOL,
                            format,
                        )));
                    (*top_with_boundaries).concatenate(&whitespace_punct_context, true);
                    (*top_with_boundaries).concatenate(
                        &HfstTransducer::new_symbol_pair(
                            crate::hfst_symbol_defs::internal_epsilon,
                            LC_EXIT_SYMBOL,
                            format,
                        ),
                        true,
                    );
                    let mut rc = HfstTransducer::new_symbol_pair(
                        crate::hfst_symbol_defs::internal_epsilon,
                        RC_ENTRY_SYMBOL,
                        format,
                    );
                    rc.concatenate(&whitespace_punct_context, true);
                    rc.concatenate(
                        &HfstTransducer::new_symbol_pair(
                            crate::hfst_symbol_defs::internal_epsilon,
                            RC_EXIT_SYMBOL,
                            format,
                        ),
                        true,
                    );
                    (*top_with_boundaries).concatenate(&*top_ptr, true);
                    (*top_with_boundaries).concatenate(&rc, true);
                    let _ = Box::from_raw(top_ptr);
                    let new_top = add_pmatch_delimiters(top_with_boundaries);
                    (*new_top).minimize();
                    retval.insert("TOP".to_string(), new_top);
                    if verbose {
                        let duration = (clock() - timer) as f64 / CLOCKS_PER_SEC as f64;
                        timer = clock();
                        eprintln!("Added automatic context separators in {} seconds", duration);
                    }
                }
            }

            let vars: Vec<(String, String)> = variables()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            let top_ptr = *retval.get("TOP").unwrap();
            for (k, v) in &vars {
                (*top_ptr).set_property(k, v);
            }
            data = std::ptr::null_mut();
            len = 0;
            retval
        }
    }

    // [spec:hfst:def:pmatch-compiler.hfst.pmatch.pmatch-compiler.set-include-path-fn]
    // [spec:hfst:sem:pmatch-compiler.hfst.pmatch.pmatch-compiler.set-include-path-fn]
    pub fn set_include_path(&mut self, path: String) {
        self.includedir = path;
    }
}
