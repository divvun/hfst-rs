//! 'pmatch_compiler' — 1:1 port of 'libhfst/src/parsers/pmatch_utils.{h,cc}',
//! the PMATCH compiler (the 'PmatchObject' lazy-evaluation AST + every
//! 'evaluate()' + every free function in 'namespace hfst::pmatch').
//!
//! The runtime matcher ('pmatch.cc') is ported separately in ['crate::pmatch']
//! and is NOT part of this module.
//!
//! Faithfulness over idiom: C++ identifiers are kept verbatim (Rust casing),
//! bugs are preserved, and shared 'Rc<..>' handles mirror the C++
//! 'PmatchObject*' hierarchy and 'hfst::pmatch' namespace globals. The ONE
//! sanctioned structural deviation is that the bison tree construction is
//! replaced by a walk over the 'nfst-pmatch' parse-only AST (see
//! ['build_object'], ['build_statement'], ['PmatchCompiler']). The C++
//! runtime 'format' plumbing is the backend type parameter 'B' now
//! ([dec:hfst:monomorphic-backends]).

#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(clippy::too_many_arguments)]

use crate::backend::AlgebraBackend;
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_data_types::StringPairSet;
use crate::hfst_data_types::Symbol;
use crate::hfst_data_types::{StringPair, StringVector};
use crate::hfst_symbol_defs::StringSet;
use crate::hfst_symbol_defs::{
    internal_default, internal_epsilon, internal_identity, internal_unknown,
};
use crate::hfst_tokenizer::HfstTokenizer;
use crate::hfst_transducer::FromAnyTransducer;
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
use std::rc::Rc;
use tracing::{debug, error, warn};

/// Shared-ownership handle to a node in the PMATCH lazy-evaluation AST.
///
/// The AST is a DAG (a definition object is shared between the `DEFINITIONS`
/// map, its expression-tree parents, and `CALL_STACK` frames), so the safe
/// representation is reference-counted shared ownership. The nodes are
/// immutable after construction; per-evaluation memoization lives off-node in
/// [`PmatchEvalContext::node_caches`]. Replaces the C++ `PmatchObject*` raw
/// pointer.
pub type ObjRef<B> = Rc<dyn PmatchObject<B>>;

/// Shared-ownership handle to a `PmatchObjectPairBase` (the markup/object-pair
/// hierarchy). Replaces the C++ `PmatchObject*`-pair raw pointer.
pub type PairRef<B> = Rc<dyn PmatchObjectPairBase<B>>;

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
pub type TransducerPointerPair<B> = (HfstTransducer<B>, HfstTransducer<B>);

// [spec:hfst:def:pmatch-utils.hfst.pmatch.mapping-pair-vector]
pub type MappingPairVector<B> = Vec<PairRef<B>>;

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
// The C++ 'struct PmatchObject' has base fields 'name'/'weight'/'line_defined'
// carried (verbatim) on every node struct below; the trait exposes them through
// accessor methods (get_name/set_name/...) implemented on each node struct. The
// C++ 'my_timer'/'cache' fields are NOT carried on the (now immutable) nodes:
// timing is threaded as a local returned from 'start_timing', and the
// memoization cache is an off-node side-table on 'PmatchEvalContext'.
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.pmatch-object-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.pmatch-object-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch-object.pmatch-object-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.pmatch-object-fn]
pub trait PmatchObject<B: AlgebraBackend + 'static> {
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

    // --- node-identity key for the off-node evaluation cache ---------------
    // The `Rc<dyn ..>` nodes are immutable and shared, so their address is a
    // stable identity. Casting to `*const ()` drops the vtable metadata (safe:
    // both casts are pointer-to-pointer / pointer-to-usize with no deref).
    fn cache_key(&self) -> usize {
        std::ptr::from_ref(self) as *const () as usize
    }

    // --- shared (non-virtual in C++) timing/cache helpers ------------------
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.start-timing-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.start-timing-fn]
    fn start_timing(&self, ctx: &mut PmatchEvalContext<B>) -> clock_t {
        if ctx.verbose && self.get_name() != "" {
            let my_timer = clock();
            ctx.named_object_evaluation_stack_depth += 1;
            write_compilation_stack_indentation_to_err(ctx);
            debug!("Compiling {}...", self.get_name());
            my_timer
        } else {
            0
        }
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.report-time-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.report-time-fn]
    fn report_time(&self, ctx: &mut PmatchEvalContext<B>, my_timer: clock_t, extra_info: String) {
        if ctx.verbose && self.get_name() != "" {
            let duration = (clock() - my_timer) as f64 / CLOCKS_PER_SEC as f64;
            write_compilation_stack_indentation_to_err(ctx);
            debug!(
                "{} compiled in {} seconds{}",
                self.get_name(),
                duration,
                extra_info
            );
            ctx.named_object_evaluation_stack_depth -= 1;
        }
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.report-cache-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.report-cache-fn]
    fn report_cache(&self, ctx: &mut PmatchEvalContext<B>, extra_info: String) {
        if ctx.verbose && self.get_name() != "TOP" {
            ctx.named_object_evaluation_stack_depth += 1;
            write_compilation_stack_indentation_to_err(ctx);
            debug!("{} fetched from cache{}", self.get_name(), extra_info);
            ctx.named_object_evaluation_stack_depth -= 1;
        }
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.should-use-cache-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.should-use-cache-fn]
    fn should_use_cache(&self, ctx: &mut PmatchEvalContext<B>) -> bool {
        self.get_name() != "" && ctx.call_stack_len() == 0
    }

    // --- virtual graph-walk / query methods (base defaults) ----------------
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.is-unweighted-disjunction-of-strings-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.is-unweighted-disjunction-of-strings-fn]
    fn is_unweighted_disjunction_of_strings(&self) -> bool {
        false
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.collect-strings-into-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.collect-strings-into-fn]
    fn collect_strings_into(&self, ctx: &mut PmatchEvalContext<B>, strings: &mut StringVector) {}
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.collect-initial-symbols-into-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.collect-initial-symbols-into-fn]
    fn collect_initial_symbols_into(
        &self,
        allowed: &mut StringSet,
        disallowed: &mut StringSet,
    ) -> crate::error::Result<()> {
        Ok(())
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.get-real-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.get-real-initial-symbols-fn]
    fn get_real_initial_symbols(&self) -> crate::error::Result<StringSet> {
        Ok(StringSet::new())
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.get-real-initial-symbols-from-right-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.get-real-initial-symbols-from-right-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-object.get-real-initial-symbols-from-right-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.get-real-initial-symbols-from-right-fn]
    fn get_real_initial_symbols_from_right(
        &self,
        ctx: &mut PmatchEvalContext<B>,
    ) -> crate::error::Result<StringSet> {
        Ok(StringSet::new())
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.is-left-concatenation-with-context-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.is-left-concatenation-with-context-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-object.is-left-concatenation-with-context-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.is-left-concatenation-with-context-fn]
    fn is_left_concatenation_with_context(&self) -> bool {
        false
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.is-context-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.is-context-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-object.is-context-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.is-context-fn]
    fn is_context(&self) -> bool {
        false
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.is-delimiter-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.is-delimiter-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-object.is-delimiter-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.is-delimiter-fn]
    fn is_delimiter(&self) -> bool {
        false
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.get-initial-symbols-from-unary-root-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.get-initial-symbols-from-unary-root-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-object.get-initial-symbols-from-unary-root-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.get-initial-symbols-from-unary-root-fn]
    fn get_initial_symbols_from_unary_root(
        &self,
        ctx: &mut PmatchEvalContext<B>,
    ) -> crate::error::Result<StringSet> {
        Ok(StringSet::new())
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.get-initial-rc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.get-initial-rc-initial-symbols-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-object.get-initial-rc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.get-initial-rc-initial-symbols-fn]
    fn get_initial_RC_initial_symbols(
        &self,
        ctx: &mut PmatchEvalContext<B>,
    ) -> crate::error::Result<StringSet> {
        Ok(StringSet::new())
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.get-initial-nrc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.get-initial-nrc-initial-symbols-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-object.get-initial-nrc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.get-initial-nrc-initial-symbols-fn]
    fn get_initial_NRC_initial_symbols(
        &self,
        ctx: &mut PmatchEvalContext<B>,
    ) -> crate::error::Result<StringSet> {
        Ok(StringSet::new())
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.expand-ins-arcs-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.expand-ins-arcs-fn]
    fn expand_Ins_arcs(&self, ss: &mut StringSet) {}
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.evaluate-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.evaluate-fn]
    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>>;
    /// The C++ overload 'evaluate(std::vector<PmatchObject*> args)' (base
    /// default).
    fn evaluate_args(
        &self,
        ctx: &mut PmatchEvalContext<B>,
        args: Vec<ObjRef<B>>,
    ) -> crate::error::Result<HfstTransducer<B>> {
        self.evaluate(ctx)
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.evaluate-as-arg-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.evaluate-as-arg-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-object.evaluate-as-arg-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.evaluate-as-arg-fn]
    fn evaluate_as_arg(&self, ctx: &mut PmatchEvalContext<B>) -> ObjRef<B> {
        panic!("evaluate_as_arg called on a PmatchObject that does not support it")
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.as-string-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.as-string-fn]
    // None for an object with no string form, where the C++ base returned "".
    fn as_string(&self, ctx: &mut PmatchEvalContext<B>) -> Option<String> {
        None
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object.as-string-pair-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object.as-string-pair-fn]
    fn as_string_pair(&self, ctx: &mut PmatchEvalContext<B>) -> StringPair {
        (Symbol::new_static(""), Symbol::new_static(""))
    }
}

// Implements the PmatchObject base-field accessors (get_name/set_name/...)
// that every node struct carries verbatim over its own name/weight/
// line_defined fields.
macro_rules! pmatch_object_base_accessors {
    () => {
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
    };
}

// ---------------------------------------------------------------------------
// PmatchObject node structs
// ---------------------------------------------------------------------------

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-symbol]
pub struct PmatchSymbol<B: AlgebraBackend + 'static> {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    // This handles argumentless function calls and definition invocations,
    // which are the same thing under the hood.
    pub sym: Symbol,
    _marker: std::marker::PhantomData<B>,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string]
pub struct PmatchString<B: AlgebraBackend + 'static> {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub string: Symbol,
    pub multichar: bool,
    _marker: std::marker::PhantomData<B>,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-question-mark]
pub struct PmatchQuestionMark<B: AlgebraBackend + 'static> {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    _marker: std::marker::PhantomData<B>,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-numeric-operation]
pub struct PmatchNumericOperation<B: AlgebraBackend + 'static> {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub op: PmatchNumericOp,
    pub root: ObjRef<B>,
    pub values: Vec<i32>,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation]
pub struct PmatchUnaryOperation<B: AlgebraBackend + 'static> {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub op: PmatchUnaryOp,
    pub root: ObjRef<B>,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation]
pub struct PmatchBinaryOperation<B: AlgebraBackend + 'static> {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub op: PmatchBinaryOp,
    pub left: ObjRef<B>,
    pub right: ObjRef<B>,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-ternary-operation]
pub struct PmatchTernaryOperation<B: AlgebraBackend + 'static> {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub op: PmatchTernaryOp,
    pub left: ObjRef<B>,
    pub middle: ObjRef<B>,
    pub right: ObjRef<B>,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-transducer-container]
pub struct PmatchTransducerContainer<B: AlgebraBackend + 'static> {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub t: HfstTransducer<B>,
}

impl<B: AlgebraBackend + 'static> PmatchTransducerContainer<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-transducer-container.pmatch-transducer-container-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-transducer-container.pmatch-transducer-container-fn]
    pub fn new(t: HfstTransducer<B>) -> Rc<PmatchTransducerContainer<B>> {
        Rc::new(PmatchTransducerContainer {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            t,
        })
    }
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-function]
pub struct PmatchFunction<B: AlgebraBackend + 'static> {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub args: Vec<Symbol>,
    pub root: ObjRef<B>,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-funcall]
pub struct PmatchFuncall<B: AlgebraBackend + 'static> {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub args: Vec<ObjRef<B>>,
    pub fun: ObjRef<B>,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-builtin-function]
pub struct PmatchBuiltinFunction<B: AlgebraBackend + 'static> {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub args: Vec<ObjRef<B>>,
    pub ty: PmatchBuiltin,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-epsilon-arc]
pub struct PmatchEpsilonArc<B: AlgebraBackend + 'static> {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    _marker: std::marker::PhantomData<B>,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-empty]
pub struct PmatchEmpty<B: AlgebraBackend + 'static> {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    _marker: std::marker::PhantomData<B>,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-acceptor]
pub struct PmatchAcceptor<B: AlgebraBackend + 'static> {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub set: PmatchPredefined,
    _marker: std::marker::PhantomData<B>,
}

// ---------------------------------------------------------------------------
// PmatchObjectPair hierarchy (NOT PmatchObject subclasses; carries the virtual
// evaluate_pair()). The 'PmatchObjectPairBase' trait preserves the C++ virtual
// dispatch between 'PmatchObjectPair' and 'PmatchMarkupContainer'.
// ---------------------------------------------------------------------------

pub trait PmatchObjectPairBase<B: AlgebraBackend + 'static> {
    fn get_left(&self) -> ObjRef<B>;
    fn get_right(&self) -> ObjRef<B>;
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object-pair.evaluate-pair-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object-pair.evaluate-pair-fn]
    fn evaluate_pair(
        &self,
        ctx: &mut PmatchEvalContext<B>,
    ) -> crate::error::Result<TransducerPointerPair<B>> {
        panic!("evaluate_pair called on a PmatchObject that is not a pair")
    }
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object-pair]
pub struct PmatchObjectPair<B: AlgebraBackend + 'static> {
    pub left: ObjRef<B>,
    pub right: ObjRef<B>,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-markup-container]
pub struct PmatchMarkupContainer<B: AlgebraBackend + 'static> {
    pub left: ObjRef<B>,
    pub right: ObjRef<B>,
    pub left_of_arrow: ObjRef<B>,
}

// ---------------------------------------------------------------------------
// Replace-rule / restriction container nodes (PmatchObject subclasses)
// ---------------------------------------------------------------------------

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-restriction-container]
pub struct PmatchRestrictionContainer<B: AlgebraBackend + 'static> {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub left: ObjRef<B>,
    pub contexts: MappingPairVector<B>,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-mapping-pairs-container]
pub struct PmatchMappingPairsContainer<B: AlgebraBackend + 'static> {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub arrow: ReplaceArrow,
    pub mapping_pairs: MappingPairVector<B>,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-contexts-container]
pub struct PmatchContextsContainer<B: AlgebraBackend + 'static> {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub ty: ReplaceType,
    pub context_pairs: MappingPairVector<B>,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-replace-rule-container]
pub struct PmatchReplaceRuleContainer<B: AlgebraBackend + 'static> {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub arrow: ReplaceArrow,
    pub ty: ReplaceType,
    pub mapping: MappingPairVector<B>,
    pub context: MappingPairVector<B>,
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-parallel-rules-container]
pub struct PmatchParallelRulesContainer<B: AlgebraBackend + 'static> {
    pub name: String,
    pub weight: f64,
    pub line_defined: i32,
    pub arrow: ReplaceArrow,
    pub rules: Vec<Rc<PmatchReplaceRuleContainer<B>>>,
}

// ---------------------------------------------------------------------------
// PmatchUtilityTransducers (cached character-class acceptors)
// ---------------------------------------------------------------------------

// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers]
pub struct PmatchUtilityTransducers<B: AlgebraBackend> {
    // Character class acceptors
    pub latin1_acceptor: HfstTransducer<B>,
    pub latin1_alpha_acceptor: HfstTransducer<B>,
    pub latin1_lowercase_acceptor: HfstTransducer<B>,
    pub latin1_uppercase_acceptor: HfstTransducer<B>,
    pub combining_accent_acceptor: HfstTransducer<B>,
    pub latin1_numeral_acceptor: HfstTransducer<B>,
    pub latin1_punct_acceptor: HfstTransducer<B>,
    pub latin1_whitespace_acceptor: HfstTransducer<B>,
    pub capify: HfstTransducer<B>,
    pub lowerfy: HfstTransducer<B>,
}

// ---------------------------------------------------------------------------
// Per-compile evaluation context (formerly the 'hfst::pmatch' namespace
// globals). Every field is private per-compile working state, owned by the
// 'PmatchCompiler' (field 'eval_ctx') and threaded as '&mut PmatchEvalContext'
// through the recursive 'PmatchObject::evaluate' walk. This replaces the
// previous 'thread_local!' globals, eliminating process/thread-global mutable
// state.
// ---------------------------------------------------------------------------
pub struct PmatchEvalContext<B: AlgebraBackend + 'static> {
    // --- scalar / pointer state ---
    data: String,
    len: usize,
    verbose: bool,
    flatten: bool,
    include_cosine_distances: bool,
    timer: clock_t,
    minimization_guard_count: i32,
    named_object_evaluation_stack_depth: i32,
    need_delimiters: bool,
    vector_similarity_projection_factor: WordVecFloat,
    utils: Option<PmatchUtilityTransducers<B>>,
    pmatchnerrs: i32,
    // --- collection state ---
    definitions_table: BTreeMap<String, ObjRef<B>>,
    variables: BTreeMap<String, String>,
    call_stack: Vec<BTreeMap<String, ObjRef<B>>>,
    eval_stack: Vec<String>,
    def_insed_expressions: BTreeMap<String, ObjRef<B>>,
    inserted_names: BTreeSet<Symbol>,
    uncomposed: BTreeSet<Symbol>,
    unsatisfied_insertions: BTreeSet<Symbol>,
    used_definitions: BTreeSet<Symbol>,
    function_names: BTreeSet<Symbol>,
    capture_names: BTreeSet<Symbol>,
    word_vectors: Vec<WordVector>,
    named_transducers: BTreeMap<String, HfstTransducer<B>>,
    includedir: String,
    lst_line_map: BTreeMap<String, i32>,
    lst_overlap_warned: BTreeSet<String>,
    // Off-node memoization side-table for the (now immutable) AST nodes,
    // keyed by node identity (see `PmatchObject::cache_key`). Replaces the C++
    // `PmatchObject::cache` field.
    node_caches: HashMap<usize, HfstTransducer<B>>,
}

macro_rules! pmatch_ctx_string_set {
    ($elem:ty, $field:ident, $contains:ident, $insert:ident, $clear:ident, $len:ident, $is_empty:ident, $snapshot:ident) => {
        fn $contains(&self, k: &str) -> bool {
            self.$field.contains(k)
        }
        fn $insert(&mut self, k: $elem) {
            self.$field.insert(k);
        }
        fn $clear(&mut self) {
            self.$field.clear();
        }
        fn $len(&self) -> usize {
            self.$field.len()
        }
        fn $is_empty(&self) -> bool {
            self.$field.is_empty()
        }
        fn $snapshot(&self) -> Vec<$elem> {
            self.$field.iter().cloned().collect()
        }
    };
}

impl<B: AlgebraBackend + 'static> PmatchEvalContext<B> {
    // --- symbol-set collections (definition/insertion/function names) ---
    pmatch_ctx_string_set!(
        Symbol,
        inserted_names,
        inserted_names_contains,
        inserted_names_insert,
        inserted_names_clear,
        inserted_names_len,
        inserted_names_is_empty,
        inserted_names_snapshot
    );
    pmatch_ctx_string_set!(
        Symbol,
        uncomposed,
        uncomposed_contains,
        uncomposed_insert,
        uncomposed_clear,
        uncomposed_len,
        uncomposed_is_empty,
        uncomposed_snapshot
    );
    pmatch_ctx_string_set!(
        Symbol,
        unsatisfied_insertions,
        unsatisfied_insertions_contains,
        unsatisfied_insertions_insert,
        unsatisfied_insertions_clear,
        unsatisfied_insertions_len,
        unsatisfied_insertions_is_empty,
        unsatisfied_insertions_snapshot
    );
    pmatch_ctx_string_set!(
        Symbol,
        used_definitions,
        used_definitions_contains,
        used_definitions_insert,
        used_definitions_clear,
        used_definitions_len,
        used_definitions_is_empty,
        used_definitions_snapshot
    );
    pmatch_ctx_string_set!(
        Symbol,
        function_names,
        function_names_contains,
        function_names_insert,
        function_names_clear,
        function_names_len,
        function_names_is_empty,
        function_names_snapshot
    );
    pmatch_ctx_string_set!(
        Symbol,
        capture_names,
        capture_names_contains,
        capture_names_insert,
        capture_names_clear,
        capture_names_len,
        capture_names_is_empty,
        capture_names_snapshot
    );
    // `lst_overlap_warned` keys are synthetic "<line>\t<symbol>" dedup strings
    // (general text, not symbol-shaped), so this set stays `String`.
    pmatch_ctx_string_set!(
        String,
        lst_overlap_warned,
        lst_overlap_warned_contains,
        lst_overlap_warned_insert,
        lst_overlap_warned_clear,
        lst_overlap_warned_len,
        lst_overlap_warned_is_empty,
        lst_overlap_warned_snapshot
    );
}

/// Facade mirroring the C++ 'hfst::pmatch::compile': construct the
/// 'PmatchObject' definitions + TOP from a pmatch source string and return the
/// evaluated transducers ('map<string, HfstTransducer*>' in C++).
pub struct PmatchCompiler<B: AlgebraBackend + 'static> {
    pub verbose: bool,
    pub flatten: bool,
    pub include_cosine_distances: bool,
    pub includedir: String,
    pub definitions: BTreeMap<String, HfstTransducer<B>>,
    // Per-compile working state (formerly the 'hfst::pmatch' namespace globals).
    // Persists across 'compile' so 'define' can read the definition table after
    // a compile, mirroring the old thread-local persistence.
    eval_ctx: PmatchEvalContext<B>,
}

mod atom_eval;
mod builder;
mod compile_driver;
mod constructors;
mod eval_context;
mod list_symbols;
mod operation_eval;
mod rule_eval;
mod symbol_helpers;
mod utility_transducers;
mod word_vectors;

pub use atom_eval::*;
pub use builder::*;
pub use compile_driver::*;
pub use list_symbols::*;
pub use operation_eval::*;
pub use symbol_helpers::*;
pub use utility_transducers::*;
pub use word_vectors::*;
