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
    fn as_string(&self, ctx: &mut PmatchEvalContext<B>) -> String {
        String::new()
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
// PmatchObject node constructors (literal ports of the C++ ctors; base fields
// follow 'PmatchObject::PmatchObject()' defaults: name="", weight=0.0,
// line_defined=0 (no lexer line counter in this port), my_timer=0, cache=NULL)
// ---------------------------------------------------------------------------

impl<B: AlgebraBackend + 'static> PmatchSymbol<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-symbol.pmatch-symbol-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-symbol.pmatch-symbol-fn]
    pub fn new(str: Symbol) -> Rc<PmatchSymbol<B>> {
        Rc::new(PmatchSymbol {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            sym: str,
            _marker: std::marker::PhantomData,
        })
    }
}

impl<B: AlgebraBackend + 'static> PmatchString<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string.pmatch-string-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-string.pmatch-string-fn]
    pub fn new(str: Symbol, is_multichar: bool) -> Rc<PmatchString<B>> {
        Rc::new(PmatchString {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            string: str,
            multichar: is_multichar,
            _marker: std::marker::PhantomData,
        })
    }
}

impl<B: AlgebraBackend + 'static> PmatchNumericOperation<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-numeric-operation.pmatch-numeric-operation-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-numeric-operation.pmatch-numeric-operation-fn]
    pub fn new(op: PmatchNumericOp, root: ObjRef<B>) -> Rc<PmatchNumericOperation<B>> {
        Rc::new(PmatchNumericOperation {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            op,
            root,
            values: Vec::new(),
        })
    }
}

impl<B: AlgebraBackend + 'static> PmatchUnaryOperation<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation.pmatch-unary-operation-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-unary-operation.pmatch-unary-operation-fn]
    pub fn new(op: PmatchUnaryOp, root: ObjRef<B>) -> Rc<PmatchUnaryOperation<B>> {
        Rc::new(PmatchUnaryOperation {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            op,
            root,
        })
    }
}

impl<B: AlgebraBackend + 'static> PmatchBinaryOperation<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.pmatch-binary-operation-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.pmatch-binary-operation-fn]
    pub fn new(
        op: PmatchBinaryOp,
        left: ObjRef<B>,
        right: ObjRef<B>,
    ) -> Rc<PmatchBinaryOperation<B>> {
        Rc::new(PmatchBinaryOperation {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            op,
            left,
            right,
        })
    }
}

impl<B: AlgebraBackend + 'static> PmatchTernaryOperation<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-ternary-operation.pmatch-ternary-operation-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-ternary-operation.pmatch-ternary-operation-fn]
    pub fn new(
        op: PmatchTernaryOp,
        left: ObjRef<B>,
        middle: ObjRef<B>,
        right: ObjRef<B>,
    ) -> Rc<PmatchTernaryOperation<B>> {
        Rc::new(PmatchTernaryOperation {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            op,
            left,
            middle,
            right,
        })
    }
}

impl<B: AlgebraBackend + 'static> PmatchFunction<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-function.pmatch-function-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-function.pmatch-function-fn]
    pub fn new(argument_vector: Vec<Symbol>, function_root: ObjRef<B>) -> Rc<PmatchFunction<B>> {
        Rc::new(PmatchFunction {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            args: argument_vector,
            root: function_root,
        })
    }
}

impl<B: AlgebraBackend + 'static> PmatchFuncall<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-funcall.pmatch-funcall-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-funcall.pmatch-funcall-fn]
    pub fn new(argument_vector: Vec<ObjRef<B>>, function: ObjRef<B>) -> Rc<PmatchFuncall<B>> {
        Rc::new(PmatchFuncall {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            args: argument_vector,
            fun: function,
        })
    }
}

impl<B: AlgebraBackend + 'static> PmatchBuiltinFunction<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-builtin-function.pmatch-builtin-function-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-builtin-function.pmatch-builtin-function-fn]
    pub fn new(ty: PmatchBuiltin, argument_vector: Vec<ObjRef<B>>) -> Rc<PmatchBuiltinFunction<B>> {
        Rc::new(PmatchBuiltinFunction {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            args: argument_vector,
            ty,
        })
    }
}

impl<B: AlgebraBackend + 'static> PmatchAcceptor<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-acceptor.pmatch-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-acceptor.pmatch-acceptor-fn]
    pub fn new(s: PmatchPredefined) -> Rc<PmatchAcceptor<B>> {
        Rc::new(PmatchAcceptor {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            set: s,
            _marker: std::marker::PhantomData,
        })
    }
}

impl<B: AlgebraBackend + 'static> PmatchObjectPair<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object-pair.pmatch-object-pair-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object-pair.pmatch-object-pair-fn]
    pub fn new(l: ObjRef<B>, r: ObjRef<B>) -> Rc<PmatchObjectPair<B>> {
        Rc::new(PmatchObjectPair { left: l, right: r })
    }
}

impl<B: AlgebraBackend + 'static> PmatchMarkupContainer<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-markup-container.pmatch-markup-container-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-markup-container.pmatch-markup-container-fn]
    pub fn new(loa: ObjRef<B>, lom: ObjRef<B>, rom: ObjRef<B>) -> Rc<PmatchMarkupContainer<B>> {
        Rc::new(PmatchMarkupContainer {
            left: lom,
            right: rom,
            left_of_arrow: loa,
        })
    }
}

impl<B: AlgebraBackend + 'static> PmatchRestrictionContainer<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-restriction-container.pmatch-restriction-container-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-restriction-container.pmatch-restriction-container-fn]
    pub fn new(l: ObjRef<B>, c: MappingPairVector<B>) -> Rc<PmatchRestrictionContainer<B>> {
        Rc::new(PmatchRestrictionContainer {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            left: l,
            contexts: c,
        })
    }
}

impl<B: AlgebraBackend + 'static> PmatchMappingPairsContainer<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-mapping-pairs-container.pmatch-mapping-pairs-container-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-mapping-pairs-container.pmatch-mapping-pairs-container-fn]
    pub fn new(
        a: ReplaceArrow,
        left: ObjRef<B>,
        right: ObjRef<B>,
    ) -> Rc<PmatchMappingPairsContainer<B>> {
        let mut obj = PmatchMappingPairsContainer {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            arrow: a,
            mapping_pairs: MappingPairVector::new(),
        };
        let pair: PairRef<B> = PmatchObjectPair::new(left, right);
        obj.mapping_pairs.push(pair);
        Rc::new(obj)
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-mapping-pairs-container.push-back-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-mapping-pairs-container.push-back-fn]
    pub fn push_back(&mut self, one_pair: &PmatchMappingPairsContainer<B>) {
        for it in one_pair.mapping_pairs.iter() {
            let pair: PairRef<B> = PmatchObjectPair::new(it.get_left(), it.get_right());
            self.mapping_pairs.push(pair);
        }
    }
}

impl<B: AlgebraBackend + 'static> PmatchContextsContainer<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-contexts-container.pmatch-contexts-container-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-contexts-container.pmatch-contexts-container-fn]
    pub fn new(
        t: ReplaceType,
        context: &PmatchContextsContainer<B>,
    ) -> Rc<PmatchContextsContainer<B>> {
        Rc::new(PmatchContextsContainer {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            ty: t,
            context_pairs: context.context_pairs.clone(),
        })
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-contexts-container.push-back-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-contexts-container.push-back-fn]
    pub fn push_back(&mut self, one_context: &PmatchContextsContainer<B>) {
        for it in one_context.context_pairs.iter() {
            let pair: PairRef<B> = PmatchObjectPair::new(it.get_left(), it.get_right());
            self.context_pairs.push(pair);
        }
    }
}

impl<B: AlgebraBackend + 'static> PmatchReplaceRuleContainer<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-replace-rule-container.pmatch-replace-rule-container-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-replace-rule-container.pmatch-replace-rule-container-fn]
    pub fn new(
        a: ReplaceArrow,
        t: ReplaceType,
        m: MappingPairVector<B>,
        c: MappingPairVector<B>,
    ) -> Rc<PmatchReplaceRuleContainer<B>> {
        Rc::new(PmatchReplaceRuleContainer {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            arrow: a,
            ty: t,
            mapping: m,
            context: c,
        })
    }
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

impl<B: AlgebraBackend + 'static> Default for PmatchEvalContext<B> {
    fn default() -> Self {
        Self::new()
    }
}

impl<B: AlgebraBackend + 'static> PmatchEvalContext<B> {
    pub fn new() -> Self {
        PmatchEvalContext {
            data: String::new(),
            len: 0,
            verbose: false,
            flatten: false,
            include_cosine_distances: false,
            timer: 0,
            minimization_guard_count: 0,
            named_object_evaluation_stack_depth: 0,
            need_delimiters: false,
            vector_similarity_projection_factor: 0.0,
            utils: None,
            pmatchnerrs: 0,
            definitions_table: BTreeMap::new(),
            variables: BTreeMap::new(),
            call_stack: Vec::new(),
            eval_stack: Vec::new(),
            def_insed_expressions: BTreeMap::new(),
            inserted_names: BTreeSet::new(),
            uncomposed: BTreeSet::new(),
            unsatisfied_insertions: BTreeSet::new(),
            used_definitions: BTreeSet::new(),
            function_names: BTreeSet::new(),
            capture_names: BTreeSet::new(),
            word_vectors: Vec::new(),
            named_transducers: BTreeMap::new(),
            includedir: String::new(),
            lst_line_map: BTreeMap::new(),
            lst_overlap_warned: BTreeSet::new(),
            node_caches: HashMap::new(),
        }
    }

    // Mirrors the former free 'init_globals': resets exactly the per-compile
    // subset the C++ reset, leaving 'uncomposed'/'word_vectors'/
    // 'named_transducers'/'includedir' alone (as the original did).
    fn init_globals(&mut self) {
        self.definitions_table.clear();
        self.node_caches.clear();
        self.variables.clear();
        self.variables
            .insert("count-patterns".to_string(), "off".to_string());
        self.variables
            .insert("delete-patterns".to_string(), "off".to_string());
        self.variables
            .insert("extract-patterns".to_string(), "off".to_string());
        self.variables
            .insert("locate-patterns".to_string(), "off".to_string());
        self.variables
            .insert("mark-patterns".to_string(), "on".to_string());
        self.variables
            .insert("max-context-length".to_string(), "254".to_string());
        self.variables
            .insert("max-recursion".to_string(), "5000".to_string());
        self.variables
            .insert("need-separators".to_string(), "on".to_string());
        self.variables
            .insert("unicode-character-classes".to_string(), "off".to_string());
        self.variables
            .insert("xerox-composition".to_string(), "on".to_string());
        self.variables.insert(
            "vector-similarity-projection-factor".to_string(),
            "1.0".to_string(),
        );
        self.call_stack.clear();
        self.eval_stack.clear();
        self.def_insed_expressions.clear();
        self.inserted_names.clear();
        self.unsatisfied_insertions.clear();
        self.used_definitions.clear();
        self.function_names.clear();
        self.capture_names.clear();
        self.zero_minimization_guard();
        self.named_object_evaluation_stack_depth = 0;
        self.need_delimiters = false;
        self.pmatchnerrs = 0;
        self.lst_line_map.clear();
        self.lst_overlap_warned.clear();
    }

    // --- NODE_CACHES (off-node memoization; replaces PmatchObject::cache) ---
    fn node_cache_get(&self, key: usize) -> Option<&HfstTransducer<B>> {
        self.node_caches.get(&key)
    }
    fn node_cache_get_mut(&mut self, key: usize) -> Option<&mut HfstTransducer<B>> {
        self.node_caches.get_mut(&key)
    }
    fn node_cache_put(&mut self, key: usize, t: HfstTransducer<B>) {
        self.node_caches.insert(key, t);
    }

    // --- DEFINITIONS (BTreeMap<String, ObjRef>) ---
    fn definitions_get(&self, k: &str) -> Option<ObjRef<B>> {
        self.definitions_table.get(k).cloned()
    }
    fn definitions_contains(&self, k: &str) -> bool {
        self.definitions_table.contains_key(k)
    }
    fn definitions_insert(&mut self, k: String, v: ObjRef<B>) {
        self.definitions_table.insert(k, v);
    }
    fn definitions_clear(&mut self) {
        self.definitions_table.clear();
    }
    fn definitions_len(&self) -> usize {
        self.definitions_table.len()
    }
    fn definitions_is_empty(&self) -> bool {
        self.definitions_table.is_empty()
    }
    fn definitions_keys(&self) -> Vec<String> {
        self.definitions_table.keys().cloned().collect()
    }
    fn definitions_snapshot(&self) -> Vec<(String, ObjRef<B>)> {
        self.definitions_table
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    // --- DEF_INSED_EXPRESSIONS (BTreeMap<String, ObjRef>) ---
    fn def_insed_expressions_get(&self, k: &str) -> Option<ObjRef<B>> {
        self.def_insed_expressions.get(k).cloned()
    }
    fn def_insed_expressions_contains(&self, k: &str) -> bool {
        self.def_insed_expressions.contains_key(k)
    }
    fn def_insed_expressions_insert(&mut self, k: String, v: ObjRef<B>) {
        self.def_insed_expressions.insert(k, v);
    }
    fn def_insed_expressions_clear(&mut self) {
        self.def_insed_expressions.clear();
    }
    fn def_insed_expressions_len(&self) -> usize {
        self.def_insed_expressions.len()
    }
    fn def_insed_expressions_is_empty(&self) -> bool {
        self.def_insed_expressions.is_empty()
    }

    // --- VARIABLES (BTreeMap<String, String>) ---
    fn variables_get(&self, k: &str) -> Option<String> {
        self.variables.get(k).cloned()
    }
    fn variables_insert(&mut self, k: String, v: String) {
        self.variables.insert(k, v);
    }
    fn variables_clear(&mut self) {
        self.variables.clear();
    }
    fn variables_snapshot(&self) -> Vec<(String, String)> {
        self.variables
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
    fn variables_entry_or_default(&mut self, k: &str) -> String {
        self.variables.entry(k.to_string()).or_default().clone()
    }
    fn variables_index(&self, k: &str) -> String {
        self.variables[k].clone()
    }

    // --- CALL_STACK (Vec<BTreeMap<String, ObjRef>>) ---
    fn call_stack_len(&self) -> usize {
        self.call_stack.len()
    }
    fn call_stack_last_get(&self, k: &str) -> Option<ObjRef<B>> {
        self.call_stack.last().and_then(|f| f.get(k).cloned())
    }
    fn call_stack_last_clone(&self) -> BTreeMap<String, ObjRef<B>> {
        self.call_stack
            .last()
            .expect("call_stack has a frame during evaluation")
            .clone()
    }
    fn call_stack_push(&mut self, frame: BTreeMap<String, ObjRef<B>>) {
        self.call_stack.push(frame);
    }
    fn call_stack_pop(&mut self) {
        self.call_stack.pop();
    }
    fn call_stack_clear(&mut self) {
        self.call_stack.clear();
    }

    // --- EVAL_STACK (Vec<String>) ---
    fn eval_stack_push(&mut self, v: String) {
        self.eval_stack.push(v);
    }
    fn eval_stack_pop(&mut self) {
        self.eval_stack.pop();
    }
    fn eval_stack_last(&self) -> Option<String> {
        self.eval_stack.last().cloned()
    }
    fn eval_stack_clear(&mut self) {
        self.eval_stack.clear();
    }

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

    // --- LST_LINE_MAP (BTreeMap<String, i32>) ---
    fn lst_line_map_contains(&self, k: &str) -> bool {
        self.lst_line_map.contains_key(k)
    }
    fn lst_line_map_get(&self, k: &str) -> Option<i32> {
        self.lst_line_map.get(k).copied()
    }
    fn lst_line_map_insert(&mut self, k: String, v: i32) {
        self.lst_line_map.insert(k, v);
    }
    fn lst_line_map_clear(&mut self) {
        self.lst_line_map.clear();
    }
    fn lst_line_map_snapshot(&self) -> BTreeMap<String, i32> {
        self.lst_line_map.clone()
    }

    // --- WORD_VECTORS (Vec<WordVector>) ---
    fn word_vectors_len(&self) -> usize {
        self.word_vectors.len()
    }
    fn word_vectors_clear(&mut self) {
        self.word_vectors.clear();
    }
    fn word_vectors_reserve(&mut self, n: usize) {
        self.word_vectors.reserve(n);
    }
    fn word_vectors_push(&mut self, wv: WordVector) {
        self.word_vectors.push(wv);
    }
    fn word_vectors_snapshot(&self) -> Vec<WordVector> {
        self.word_vectors.clone()
    }
    fn word_vectors_first_vector_len(&self) -> usize {
        self.word_vectors[0].vector.len()
    }

    // --- utility-transducer cache (formerly the 'UTILS' thread-local) ---
    // The cache is constructed on first use. The utility methods invoked inside
    // 'f' never re-enter 'with_utils', so the '&mut self' borrow held across 'f'
    // cannot double-borrow.
    fn with_utils<R>(
        &mut self,
        f: impl FnOnce(&mut PmatchUtilityTransducers<B>) -> crate::error::Result<R>,
    ) -> crate::error::Result<R> {
        if self.utils.is_none() {
            self.utils = Some(PmatchUtilityTransducers::new()?);
        }
        f(self.utils.as_mut().expect("utils set just above"))
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch.zero-minimization-guard-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.zero-minimization-guard-fn]
    fn zero_minimization_guard(&mut self) {
        self.minimization_guard_count = 0;
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch.make-minimization-guard-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-minimization-guard-fn]
    fn make_minimization_guard(
        &mut self,
    ) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
        let mut guard = String::new();
        if self.minimization_guard_count == 0 {
            guard.push_str(internal_epsilon);
        } else {
            let mgc = self.minimization_guard_count;
            guard.push_str(&format!("@PMATCH_GUARD_{}@", mgc));
        }
        self.minimization_guard_count += 1;
        epsilon_to_symbol_container(self, guard)
    }

    // [spec:hfst:def:pmatch-utils.pmatcherror-fn]
    // [spec:hfst:sem:pmatch-utils.pmatcherror-fn]
    fn pmatcherror(&self, msg: &str) {
        let buf = self.data.clone();
        let bytes = buf.as_bytes();
        let parsedata: String = if bytes.is_empty() {
            String::new()
        } else if bytes.len() < 60 {
            String::from_utf8_lossy(bytes).into_owned()
        } else {
            String::from_utf8_lossy(&bytes[..59]).into_owned() + "... [truncated]"
        };
        let mut errmsg = String::new();
        errmsg.push_str("hfst-pmatch:");
        errmsg.push_str("parsing failed: ");
        errmsg.push_str(msg);
        errmsg.push_str("\n*** parsing ");
        errmsg.push_str(&parsedata);
        errmsg.push('\n');

        std::panic::panic_any(errmsg);
    }
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.add-to-pmatch-symbols-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.add-to-pmatch-symbols-fn]
pub fn add_to_pmatch_symbols(symbols: StringSet) {
    // Declared in pmatch_utils.h but never defined in pmatch_utils.cc.
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.acceptor-from-cstr-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.acceptor-from-cstr-fn]
pub fn acceptor_from_cstr<B: AlgebraBackend>(
    strings: &[&str],
) -> crate::error::Result<HfstTransducer<B>> {
    let mut retval: HfstTransducer<B> = HfstTransducer::new();
    let mut i = 0;
    while i < array_len(strings) {
        let tmp = HfstTransducer::new_symbol(strings[i])?;
        retval.disjunct(&tmp, true)?;
        i += 1;
    }
    retval.minimize()?;
    Ok(retval)
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.array-len-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.array-len-fn]
pub fn array_len(strings: &[&str]) -> usize {
    strings.len()
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

// ===== body: utility-transducers =====
impl<B: AlgebraBackend> PmatchUtilityTransducers<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.pmatch-utility-transducers-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.pmatch-utility-transducers-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.pmatch-utility-transducers-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.pmatch-utility-transducers-fn]
    pub fn new() -> crate::error::Result<PmatchUtilityTransducers<B>> {
        let mut retval = PmatchUtilityTransducers {
            latin1_acceptor: PmatchUtilityTransducers::make_latin1_acceptor()?,
            latin1_alpha_acceptor: PmatchUtilityTransducers::make_latin1_alpha_acceptor()?,
            latin1_lowercase_acceptor: PmatchUtilityTransducers::make_latin1_lowercase_acceptor()?,
            latin1_uppercase_acceptor: PmatchUtilityTransducers::make_latin1_uppercase_acceptor()?,
            combining_accent_acceptor: PmatchUtilityTransducers::make_combining_accent_acceptor()?,
            latin1_numeral_acceptor: PmatchUtilityTransducers::make_latin1_numeral_acceptor()?,
            latin1_punct_acceptor: PmatchUtilityTransducers::make_latin1_punct_acceptor()?,
            latin1_whitespace_acceptor: PmatchUtilityTransducers::make_latin1_whitespace_acceptor(
            )?,
            capify: HfstTransducer::new(),
            lowerfy: HfstTransducer::new(),
        };
        retval.lowerfy = retval.make_lowerfy()?;
        retval.capify = retval.make_capify()?;
        Ok(retval)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-acceptor-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-acceptor-fn]
    pub fn make_latin1_acceptor() -> crate::error::Result<HfstTransducer<B>> {
        let mut retval: HfstTransducer<B> = PmatchUtilityTransducers::make_latin1_alpha_acceptor()?;
        let mut tmp: HfstTransducer<B> = PmatchUtilityTransducers::make_latin1_numeral_acceptor()?;
        retval.disjunct(&tmp, true)?;
        tmp = PmatchUtilityTransducers::make_latin1_punct_acceptor()?;
        retval.disjunct(&tmp, true)?;
        tmp = PmatchUtilityTransducers::make_latin1_whitespace_acceptor()?;
        retval.disjunct(&tmp, true)?;
        retval.minimize()?;
        Ok(retval)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-alpha-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-alpha-acceptor-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-alpha-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-alpha-acceptor-fn]
    pub fn make_latin1_alpha_acceptor() -> crate::error::Result<HfstTransducer<B>> {
        let mut retval: HfstTransducer<B> =
            PmatchUtilityTransducers::make_latin1_lowercase_acceptor()?;
        let tmp: HfstTransducer<B> = PmatchUtilityTransducers::make_latin1_uppercase_acceptor()?;
        retval.disjunct(&tmp, true)?;
        retval.minimize()?;
        Ok(retval)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-lowercase-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-lowercase-acceptor-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-lowercase-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-lowercase-acceptor-fn]
    pub fn make_latin1_lowercase_acceptor() -> crate::error::Result<HfstTransducer<B>> {
        let mut retval: HfstTransducer<B> = acceptor_from_cstr(latin1_lower)?;
        let tmp: HfstTransducer<B> = PmatchUtilityTransducers::make_combining_accent_acceptor()?;
        retval.disjunct(&tmp, true)?;
        retval.minimize()?;
        Ok(retval)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-uppercase-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-uppercase-acceptor-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-uppercase-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-uppercase-acceptor-fn]
    pub fn make_latin1_uppercase_acceptor() -> crate::error::Result<HfstTransducer<B>> {
        let mut retval: HfstTransducer<B> = acceptor_from_cstr(latin1_upper)?;
        let tmp: HfstTransducer<B> = PmatchUtilityTransducers::make_combining_accent_acceptor()?;
        retval.disjunct(&tmp, true)?;
        retval.minimize()?;
        Ok(retval)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-combining-accent-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-combining-accent-acceptor-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-combining-accent-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-combining-accent-acceptor-fn]
    pub fn make_combining_accent_acceptor() -> crate::error::Result<HfstTransducer<B>> {
        acceptor_from_cstr(combining_accents)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-numeral-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-numeral-acceptor-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-numeral-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-numeral-acceptor-fn]
    pub fn make_latin1_numeral_acceptor() -> crate::error::Result<HfstTransducer<B>> {
        let mut retval: HfstTransducer<B> = HfstTransducer::new();
        let num: String = "0123456789".to_string();
        for it in num.chars() {
            retval.disjunct(&HfstTransducer::new_symbol(&it.to_string())?, true)?;
        }
        // retval->minimize(); ?
        Ok(retval)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-punct-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-punct-acceptor-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-punct-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-punct-acceptor-fn]
    pub fn make_latin1_punct_acceptor() -> crate::error::Result<HfstTransducer<B>> {
        acceptor_from_cstr(latin1_punct)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-whitespace-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-latin1-whitespace-acceptor-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-whitespace-acceptor-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-latin1-whitespace-acceptor-fn]
    pub fn make_latin1_whitespace_acceptor() -> crate::error::Result<HfstTransducer<B>> {
        acceptor_from_cstr(latin1_whitespace)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-capify-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-capify-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-capify-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-capify-fn]
    pub fn make_capify(&mut self) -> crate::error::Result<HfstTransducer<B>> {
        let mut retval: HfstTransducer<B> = HfstTransducer::new();
        let tok: HfstTokenizer = HfstTokenizer::new();
        let mut i: usize = 0;
        while i < array_len(latin1_upper) {
            retval.disjunct(
                &HfstTransducer::new_tokenized_pair(latin1_lower[i], latin1_upper[i], &tok)?,
                true,
            )?;
            i += 1;
        }
        let mut accents: HfstTransducer<B> =
            HfstTransducer::new_copy(&self.combining_accent_acceptor)?;
        accents.optionalize()?;
        retval.concatenate(&accents, true)?;
        retval.minimize()?;
        Ok(retval)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.make-lowerfy-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.make-lowerfy-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-lowerfy-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.make-lowerfy-fn]
    pub fn make_lowerfy(&mut self) -> crate::error::Result<HfstTransducer<B>> {
        let mut retval: HfstTransducer<B> = HfstTransducer::new();
        let tok: HfstTokenizer = HfstTokenizer::new();
        let mut i: usize = 0;
        while i < array_len(latin1_upper) {
            retval.disjunct(
                &HfstTransducer::new_tokenized_pair(latin1_upper[i], latin1_lower[i], &tok)?,
                true,
            )?;
            i += 1;
        }
        let mut accents: HfstTransducer<B> =
            HfstTransducer::new_copy(&self.combining_accent_acceptor)?;
        accents.optionalize()?;
        retval.concatenate(&accents, true)?;
        retval.minimize()?;
        Ok(retval)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.get-lowercase-acceptor-from-transducer-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.get-lowercase-acceptor-from-transducer-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.get-lowercase-acceptor-from-transducer-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.get-lowercase-acceptor-from-transducer-fn]
    pub fn get_lowercase_acceptor_from_transducer(
        &mut self,
        t: &HfstTransducer<B>,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let mut lowercase: HfstTransducer<B> = HfstTransducer::new();
        let ss: StringSet = t.get_alphabet()?;
        for it in ss.iter() {
            let us: Vec<char> = it.chars().collect();
            if us.len() == 1
                && icu::properties::CodePointSetData::new::<icu::properties::props::Lowercase>()
                    .contains(us[0])
            {
                lowercase.disjunct(&HfstTransducer::new_symbol(it)?, true)?;
            }
        }
        Ok(lowercase)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.get-uppercase-acceptor-from-transducer-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.get-uppercase-acceptor-from-transducer-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.get-uppercase-acceptor-from-transducer-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.get-uppercase-acceptor-from-transducer-fn]
    pub fn get_uppercase_acceptor_from_transducer(
        &mut self,
        t: &HfstTransducer<B>,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let mut uppercase: HfstTransducer<B> = HfstTransducer::new();
        let ss: StringSet = t.get_alphabet()?;
        for it in ss.iter() {
            let us: Vec<char> = it.chars().collect();
            if us.len() == 1
                && icu::properties::CodePointSetData::new::<icu::properties::props::Uppercase>()
                    .contains(us[0])
            {
                uppercase.disjunct(&HfstTransducer::new_symbol(it)?, true)?;
            }
        }
        Ok(uppercase)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.lowercaser-from-transducer-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.lowercaser-from-transducer-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.lowercaser-from-transducer-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.lowercaser-from-transducer-fn]
    pub fn lowercaser_from_transducer(
        &mut self,
        t: &HfstTransducer<B>,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let mut lowercase: HfstTransducer<B> = HfstTransducer::new();
        let ss: StringSet = t.get_alphabet()?;
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
                    if uppercases_seen.contains(upper.as_str()) {
                        continue;
                    }
                    uppercases_seen.insert(Symbol::from(upper.clone()));
                    let lower: String = icu::casemap::CaseMapper::new()
                        .lowercase_to_string(it, &icu::locale::LanguageIdentifier::UNKNOWN)
                        .into_owned();
                    lowercase.disjunct(&HfstTransducer::new_symbol_pair(&upper, &lower)?, true)?;
                }
            }
        }
        Ok(lowercase)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.uppercaser-from-transducer-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.uppercaser-from-transducer-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.uppercaser-from-transducer-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.uppercaser-from-transducer-fn]
    pub fn uppercaser_from_transducer(
        &mut self,
        t: &HfstTransducer<B>,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let mut uppercase: HfstTransducer<B> = HfstTransducer::new();
        let ss: StringSet = t.get_alphabet()?;
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
                    if uppercases_seen.contains(upper.as_str()) {
                        continue;
                    }
                    uppercases_seen.insert(Symbol::from(upper.clone()));
                    let lower: String = icu::casemap::CaseMapper::new()
                        .lowercase_to_string(it, &icu::locale::LanguageIdentifier::UNKNOWN)
                        .into_owned();
                    uppercase.disjunct(&HfstTransducer::new_symbol_pair(&lower, &upper)?, true)?;
                }
            }
        }
        Ok(uppercase)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.cap-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.cap-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.cap-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.cap-fn]
    pub fn cap(
        &mut self,
        t: &HfstTransducer<B>,
        side: Side,
        optional: bool,
    ) -> crate::error::Result<HfstTransducer<B>> {
        // This is to match flags in t with ?'s in "anything": these composes run
        // with Xerox-style composition enabled.
        let cfg = crate::hfst_transducer::EngineConfig {
            xerox_composition: true,
            ..Default::default()
        };

        let mut retval: HfstTransducer<B>;
        let mut cap: HfstTransducer<B> = self.uppercaser_from_transducer(t)?;
        let mut decap: HfstTransducer<B> = HfstTransducer::new_copy(&cap)?;
        decap.invert()?;
        let mut anything: HfstTransducer<B> = HfstTransducer::identity_pair();
        let mut anything_but_whitespace_star: HfstTransducer<B> =
            HfstTransducer::new_copy(&anything)?;
        anything_but_whitespace_star.subtract(&self.latin1_whitespace_acceptor, true)?;
        anything_but_whitespace_star.repeat_star()?;
        if !optional {
            // don't let lowercased first letters through
            anything.subtract(&self.get_lowercase_acceptor_from_transducer(t)?, true)?;
        }
        // As in the regexp
        // [[[["A":"a" [[\" "]* (" " "A":"a")]* ] .o. [{ab ad}:{ef eh}].u]] .o.
        //   [{ab ad}:{ef eh}] ] .o. [[{ab ad}:{ef eh}].l] .o.
        //   ["e":"E" [[\" "]+ (" " "e":"E")]*]
        if side == Side::Lower {
            retval = HfstTransducer::new_copy(t)?;
            cap.disjunct(&anything, true)?;
            // Cap is the first letter to either capitalize or accept if it's not a
            // lowercase letter
            let mut continuation: HfstTransducer<B> =
                HfstTransducer::new_copy(&anything_but_whitespace_star)?;
            // continuation is the rest of the first word
            let mut more_caps: HfstTransducer<B> =
                HfstTransducer::new_copy(&self.latin1_whitespace_acceptor)?;
            // more_caps is more words to capitalize
            more_caps.concatenate(&cap, true)?;
            more_caps.optionalize()?;
            continuation.concatenate(&more_caps, true)?;
            continuation.repeat_star()?;
            cap.concatenate(&continuation, true)?;
            retval.compose_with_config(&cap, true, &cfg)?;
        } else if side == Side::Upper {
            decap.disjunct(&anything, true)?;
            let mut continuation: HfstTransducer<B> =
                HfstTransducer::new_copy(&anything_but_whitespace_star)?;
            let mut more_decaps: HfstTransducer<B> =
                HfstTransducer::new_copy(&self.latin1_whitespace_acceptor)?;
            more_decaps.concatenate(&decap, true)?;
            more_decaps.optionalize()?;
            continuation.concatenate(&more_decaps, true)?;
            continuation.repeat_star()?;
            retval = HfstTransducer::new_copy(&decap)?;
            retval.concatenate(&continuation, true)?;
            retval.compose_with_config(t, true, &cfg)?;
        } else {
            // both
            decap.disjunct(&anything, true)?;
            let mut continuation: HfstTransducer<B> =
                HfstTransducer::new_copy(&anything_but_whitespace_star)?;
            let mut more_decaps: HfstTransducer<B> =
                HfstTransducer::new_copy(&self.latin1_whitespace_acceptor)?;
            more_decaps.concatenate(&decap, true)?;
            more_decaps.optionalize()?;
            continuation.concatenate(&more_decaps, true)?;
            continuation.repeat_star()?;
            retval = HfstTransducer::new_copy(&decap)?;
            retval.concatenate(&continuation, true)?;
            retval.compose_with_config(t, true, &cfg)?;
            let mut continuation2: HfstTransducer<B> =
                HfstTransducer::new_copy(&anything_but_whitespace_star)?;
            let mut more_caps: HfstTransducer<B> =
                HfstTransducer::new_copy(&self.latin1_whitespace_acceptor)?;
            cap.disjunct(&anything, true)?;
            more_caps.concatenate(&cap, true)?;
            more_caps.optionalize()?;
            continuation2.concatenate(&more_caps, true)?;
            continuation2.repeat_star()?;
            cap.concatenate(&continuation2, true)?;
            retval.compose_with_config(&cap, true, &cfg)?;
            retval.output_project()?;
        }
        retval.minimize()?;
        Ok(retval)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.tolower-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.tolower-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.tolower-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.tolower-fn]
    pub fn tolower(
        &mut self,
        t: &HfstTransducer<B>,
        side: Side,
        optional: bool,
    ) -> crate::error::Result<HfstTransducer<B>> {
        // This is to match flags in t with ?'s in "anything": these composes run
        // with Xerox-style composition enabled.
        let cfg = crate::hfst_transducer::EngineConfig {
            xerox_composition: true,
            ..Default::default()
        };

        let mut anything: HfstTransducer<B> =
            HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
        if !optional {
            anything.subtract(&self.get_uppercase_acceptor_from_transducer(t)?, true)?;
        }
        let mut retval: HfstTransducer<B>;
        if side == Side::Lower {
            let mut lowercase: HfstTransducer<B> = self.lowercaser_from_transducer(t)?;
            lowercase.disjunct(&anything, true)?;
            lowercase.repeat_star()?;
            retval = HfstTransducer::new_copy(t)?;
            retval.compose_with_config(&lowercase, true, &cfg)?;
        } else if side == Side::Upper {
            retval = self.uppercaser_from_transducer(t)?;
            retval.disjunct(&anything, true)?;
            retval.repeat_star()?;
            retval.compose_with_config(t, true, &cfg)?;
        } else {
            // both
            retval = self.uppercaser_from_transducer(t)?;
            retval.disjunct(&anything, true)?;
            retval.repeat_star()?;
            retval.compose_with_config(t, true, &cfg)?;
            let mut lowercase: HfstTransducer<B> = self.lowercaser_from_transducer(t)?;
            lowercase.disjunct(&anything, true)?;
            lowercase.repeat_star()?;
            retval.compose_with_config(&lowercase, true, &cfg)?;
        }
        retval.minimize()?;
        Ok(retval)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-utility-transducers.toupper-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-utility-transducers.toupper-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.toupper-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-utility-transducers.toupper-fn]
    pub fn toupper(
        &mut self,
        t: &HfstTransducer<B>,
        side: Side,
        optional: bool,
    ) -> crate::error::Result<HfstTransducer<B>> {
        // This is to match flags in t with ?'s in "anything": these composes run
        // with Xerox-style composition enabled.
        let cfg = crate::hfst_transducer::EngineConfig {
            xerox_composition: true,
            ..Default::default()
        };

        let mut anything: HfstTransducer<B> =
            HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
        if !optional {
            anything.subtract(&self.get_lowercase_acceptor_from_transducer(t)?, true)?;
        }
        let mut retval: HfstTransducer<B>;
        if side == Side::Lower {
            let mut uppercase: HfstTransducer<B> = self.uppercaser_from_transducer(t)?;
            uppercase.disjunct(&anything, true)?;
            uppercase.repeat_star()?;
            retval = HfstTransducer::new_copy(t)?;
            retval.compose_with_config(&uppercase, true, &cfg)?;
        } else if side == Side::Upper {
            retval = self.lowercaser_from_transducer(t)?;
            retval.disjunct(&anything, true)?;
            retval.repeat_star()?;
            retval.compose_with_config(t, true, &cfg)?;
        } else {
            // both
            retval = self.lowercaser_from_transducer(t)?;
            retval.disjunct(&anything, true)?;
            retval.repeat_star()?;
            retval.compose_with_config(t, true, &cfg)?;
            let mut uppercase: HfstTransducer<B> = self.uppercaser_from_transducer(t)?;
            uppercase.disjunct(&anything, true)?;
            uppercase.repeat_star()?;
            retval.compose_with_config(&uppercase, true, &cfg)?;
        }
        retval.minimize()?;
        Ok(retval)
    }
}

// ===== body: string-symbol-marker-helpers =====
// Helper for faithfully constructing a freshly-'new'ed PmatchObject node's base
// fields, mirroring the C++ 'PmatchObject::PmatchObject()' default constructor
// (name="", weight=0.0, line_defined=pmatchlineno, my_timer uninitialised,
// cache=NULL). There is no lexer line counter in this port, so line_defined=0.

// ---- libc-free C-string helpers for the surviving char*-based pmatch utils ----

// [spec:hfst:def:pmatch-utils.should-colourise-fn]
// [spec:hfst:sem:pmatch-utils.should-colourise-fn]
pub fn should_colourise() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stdout())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.warn-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.warn-fn]
pub fn warn(warning: String) {
    warn!("hfst-pmatch: {}", warning);
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.symbol-in-global-context-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.symbol-in-global-context-fn]
pub fn symbol_in_global_context<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    sym: &str,
) -> bool {
    ctx.definitions_contains(sym)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.symbol-in-local-context-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.symbol-in-local-context-fn]
pub fn symbol_in_local_context<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    sym: &str,
) -> bool {
    if ctx.call_stack_len() == 0 {
        return false;
    }
    ctx.call_stack_last_get(sym).is_some()
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.symbol-from-global-context-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.symbol-from-global-context-fn]
//
// Returns the shared AST node bound to 'sym' in the global definitions. The
// callers always guard with 'symbol_in_global_context', so the C++ NULL branch
// is unreachable.
pub fn symbol_from_global_context<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    sym: &str,
) -> Option<ObjRef<B>> {
    if symbol_in_global_context(ctx, sym) {
        ctx.definitions_get(sym)
    } else {
        None
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.symbol-from-local-context-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.symbol-from-local-context-fn]
pub fn symbol_from_local_context<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    sym: &str,
) -> Option<ObjRef<B>> {
    if symbol_in_local_context(ctx, sym) {
        ctx.call_stack_last_get(sym)
    } else {
        None
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
pub fn is_special(symbol: &str) -> bool {
    if symbol.len() < 3 {
        return false;
    }
    symbol.find('@') == Some(0) && symbol.rfind('@') == Some(symbol.len() - 1)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.get-ins-transition-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-ins-transition-fn]
pub fn get_Ins_transition(s: &str) -> String {
    format!("@I.{}@", s)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.add-pmatch-delimiters-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.add-pmatch-delimiters-fn]
pub fn add_pmatch_delimiters<B: AlgebraBackend>(
    regex: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut delimited_regex = HfstTransducer::new_symbol_pair(internal_epsilon, ENTRY_SYMBOL)?;
    delimited_regex.concatenate(regex, true)?;
    let exit = HfstTransducer::new_symbol_pair(internal_epsilon, EXIT_SYMBOL)?;
    delimited_regex.concatenate(&exit, true)?;
    Ok(delimited_regex)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-end-tag-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-end-tag-fn]
pub fn make_end_tag<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    tag: String,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    epsilon_to_symbol_container(ctx, format!("@PMATCH_ENDTAG_{}@", tag))
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-capture-tag-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-capture-tag-fn]
pub fn make_capture_tag<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    tag: String,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    epsilon_to_symbol_container(ctx, format!("@PMATCH_CAPTURE_{}@", tag))
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-captured-tag-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-captured-tag-fn]
pub fn make_captured_tag<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    tag: String,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    epsilon_to_symbol_container(ctx, format!("@PMATCH_CAPTURED_{}@", tag))
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-with-tag-entry-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-with-tag-entry-fn]
pub fn make_with_tag_entry<B: AlgebraBackend + 'static>(key: String, value: String) -> ObjRef<B> {
    let obj = PmatchString {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        string: Symbol::from(format!("@P.PMATCH_GLOBAL_{}.{}@", key, value)),
        multichar: false,
        _marker: std::marker::PhantomData,
    };
    as_obj(Rc::new(obj))
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-with-tag-exit-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-with-tag-exit-fn]
pub fn make_with_tag_exit<B: AlgebraBackend + 'static>(key: String) -> ObjRef<B> {
    let obj = PmatchString {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        string: Symbol::from(format!("@C.PMATCH_GLOBAL_{}@", key)),
        multichar: false,
        _marker: std::marker::PhantomData,
    };
    as_obj(Rc::new(obj))
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-counter-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-counter-fn]
pub fn make_counter<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    name: String,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    epsilon_to_symbol_container(ctx, format!("@PMATCH_COUNTER_{}@", name))
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.get-non-special-alphabet-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-non-special-alphabet-fn]
pub fn get_non_special_alphabet<B: AlgebraBackend>(
    t: &HfstTransducer<B>,
) -> crate::error::Result<StringSet> {
    let mut retval: StringSet = StringSet::new();
    let alphabet = t.get_alphabet()?;
    for it in alphabet.iter() {
        if PmatchAlphabet::is_printable(it) {
            retval.insert(it.clone());
        }
    }
    Ok(retval)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-list-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-list-fn]
pub fn make_list<B: AlgebraBackend>(
    t: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut transition = String::from("@L.");
    let alphabet = get_non_special_alphabet(t)?;
    for it in alphabet.iter() {
        transition.push_str(it);
        transition.push('_');
    }
    transition.push('@');
    HfstTransducer::new_symbol(&transition)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-exc-list-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-exc-list-fn]
pub fn make_exc_list<B: AlgebraBackend>(
    t: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut transition = String::from("@X.");
    let alphabet = get_non_special_alphabet(t)?;
    for it in alphabet.iter() {
        transition.push_str(it);
        transition.push('_');
    }
    transition.push('@');
    HfstTransducer::new_symbol(&transition)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-sigma-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-sigma-fn]
pub fn make_sigma<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    t: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut retval = HfstTransducer::new();
    let alphabet = get_non_special_alphabet(t)?;
    for it in alphabet.iter() {
        retval.disjunct(&HfstTransducer::new_symbol(it)?, true)?;
    }
    Ok(retval)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.epsilon-to-symbol-container-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.epsilon-to-symbol-container-fn]
pub fn epsilon_to_symbol_container<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    s: String,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    let tmp = HfstTransducer::new_symbol_pair(internal_epsilon, &s)?;
    let container = PmatchTransducerContainer {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        t: tmp,
    };
    Ok(Rc::new(container))
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-rc-entry-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-rc-entry-fn]
pub fn make_rc_entry<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    epsilon_to_symbol_container(ctx, RC_ENTRY_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-lc-entry-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-lc-entry-fn]
pub fn make_lc_entry<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    epsilon_to_symbol_container(ctx, LC_ENTRY_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-nrc-entry-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-nrc-entry-fn]
pub fn make_nrc_entry<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    epsilon_to_symbol_container(ctx, NRC_ENTRY_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-nlc-entry-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-nlc-entry-fn]
pub fn make_nlc_entry<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    epsilon_to_symbol_container(ctx, NLC_ENTRY_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-rc-exit-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-rc-exit-fn]
pub fn make_rc_exit<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    epsilon_to_symbol_container(ctx, RC_EXIT_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-lc-exit-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-lc-exit-fn]
pub fn make_lc_exit<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    epsilon_to_symbol_container(ctx, LC_EXIT_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-nrc-exit-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-nrc-exit-fn]
pub fn make_nrc_exit<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    epsilon_to_symbol_container(ctx, NRC_EXIT_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-nlc-exit-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-nlc-exit-fn]
pub fn make_nlc_exit<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    epsilon_to_symbol_container(ctx, NLC_EXIT_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.make-passthrough-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.make-passthrough-fn]
pub fn make_passthrough<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    epsilon_to_symbol_container(ctx, PASSTHROUGH_SYMBOL.to_string())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.get-delimited-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-delimited-fn]
pub fn get_delimited_lr(s: &str, delim_left: char, delim_right: char) -> String {
    // The content between the first 'delim_left' and the last 'delim_right'.
    let start = match s.find(delim_left) {
        Some(i) => i + delim_left.len_utf8(),
        None => return String::new(),
    };
    let end = s.rfind(delim_right).unwrap_or(s.len());
    if end <= start {
        String::new()
    } else {
        s[start..end].to_string()
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.get-delimited-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-delimited-fn]
pub fn get_delimited(s: &str, delim: char) -> String {
    get_delimited_lr(s, delim, delim)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.codepoint-to-utf8-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.codepoint-to-utf8-fn]
pub fn codepoint_to_utf8(codepoint: u32) -> String {
    // Invalid codepoints (surrogates, > U+10FFFF) yield the empty string, as
    // the C 'u_parse_err' path did. So does NUL: the original wrote it into a
    // C buffer whose 'std::string(buf)' constructor truncated at the NUL.
    match char::from_u32(codepoint) {
        None | Some('\0') => String::new(),
        Some(c) => String::from(c),
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.parse-range-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.parse-range-fn]
pub fn parse_range<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    s: &str,
) -> crate::error::Result<Rc<PmatchTransducerContainer<B>>> {
    // Reads one codepoint at the cursor: a '\uXXXX' / '\UXXXXXXXX' hex escape, or
    // the next UTF-8 character. Advances the byte cursor past what it consumed.
    fn read_codepoint(bytes: &[u8], quoted: &str, i: &mut usize) -> u32 {
        if bytes.len() - *i >= 6
            && bytes[*i] == b'\\'
            && (bytes[*i + 1] == b'u' || bytes[*i + 1] == b'U')
        {
            let width = if bytes[*i + 1] == b'u' { 6 } else { 10 };
            let end = (*i + width).min(bytes.len());
            let hex = &quoted[*i + 2..end];
            *i += width;
            u32::from_str_radix(hex, 16).unwrap_or(0)
        } else {
            let ch = quoted[*i..]
                .chars()
                .next()
                .expect("index within bounds of the quoted string");
            *i += ch.len_utf8();
            ch as u32
        }
    }
    let quoted = get_delimited(s, '"');
    let bytes = quoted.as_bytes();
    let mut i = 0usize;
    let mut retval = HfstTransducer::new();
    while i < bytes.len() {
        let mut codepoint1 = read_codepoint(bytes, &quoted, &mut i);
        if i >= bytes.len() || bytes[i] != b'-' {
            ctx.pmatcherror(&format!("Could not parse range expression: {}", s));
        }
        i += 1;
        let codepoint2 = read_codepoint(bytes, &quoted, &mut i);
        if codepoint1 == 0 || codepoint2 == 0 {
            ctx.pmatcherror(&format!("Malformed character in range expression: {}", s));
        }
        if codepoint2 < codepoint1 {
            ctx.pmatcherror(&format!(
                "Range expression goes from higher to lower: {}",
                s
            ));
        }
        while codepoint1 <= codepoint2 {
            retval.disjunct(
                &HfstTransducer::new_symbol(&codepoint_to_utf8(codepoint1))?,
                true,
            )?;
            codepoint1 += 1;
        }
    }
    let container = PmatchTransducerContainer {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        t: retval,
    };
    Ok(Rc::new(container))
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.get-size-info-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-size-info-fn]
pub fn get_size_info<B: AlgebraBackend>(net: &HfstTransducer<B>) -> String {
    let tmp = HfstBasicTransducer::from_transducer(net);
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
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchUnaryOperation<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        let key = self.cache_key();
        if ctx.node_cache_get(key).is_some() {
            self.report_cache(ctx, String::new());
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just checked"));
        }
        let my_timer = self.start_timing(ctx);

        // Special optimization cases
        if self.op == PmatchUnaryOp::Implode {
            let mut strings: StringVector = StringVector::new();
            self.root.collect_strings_into(ctx, &mut strings);
            let mut whole_string = String::new();
            for it in strings.iter() {
                whole_string += it;
            }
            let mut retval = if !whole_string.is_empty() {
                HfstTransducer::new_symbol(&whole_string)?
            } else {
                HfstTransducer::new()
            };
            retval.set_final_weights(self.weight as f32, true)?;
            if ctx.node_cache_get(key).is_none() && self.should_use_cache(ctx) {
                ctx.node_cache_put(key, retval);
                self.report_time(
                    ctx,
                    my_timer,
                    " with ".to_string()
                        + &get_size_info(ctx.node_cache_get(key).expect("just inserted")),
                );
                return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just inserted"));
            }
            self.report_time(ctx, my_timer, String::new());
            return Ok(retval);
        } else if self.op == PmatchUnaryOp::Explode {
            let mut strings: StringVector = StringVector::new();
            self.root.collect_strings_into(ctx, &mut strings);
            let mut whole_string = String::new();
            for it in strings.iter() {
                whole_string += it;
            }
            let tok = crate::hfst_tokenizer::HfstTokenizer::new();
            let mut retval = if !whole_string.is_empty() {
                HfstTransducer::new_tokenized(&whole_string, &tok)?
            } else {
                HfstTransducer::new()
            };
            retval.set_final_weights(self.weight as f32, true)?;
            if ctx.node_cache_get(key).is_none() && self.should_use_cache(ctx) {
                ctx.node_cache_put(key, retval);
                return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just inserted"));
            }
            self.report_time(ctx, my_timer, String::new());
            return Ok(retval);
        }

        if !self.name.is_empty() {
            ctx.eval_stack_push(self.name.clone());
        }
        let mut retval: HfstTransducer<B> = self.root.evaluate(ctx)?;
        if self.op == PmatchUnaryOp::AddDelimiters {
            retval = add_pmatch_delimiters(&retval)?;
        } else if self.op == PmatchUnaryOp::Optionalize {
            retval.optionalize()?;
        } else if self.op == PmatchUnaryOp::RepeatStar {
            retval.repeat_star()?;
        } else if self.op == PmatchUnaryOp::RepeatPlus {
            retval.repeat_plus()?;
        } else if self.op == PmatchUnaryOp::Reverse {
            retval.reverse()?;
        } else if self.op == PmatchUnaryOp::Invert {
            retval.invert()?;
        } else if self.op == PmatchUnaryOp::InputProject {
            retval.input_project()?;
        } else if self.op == PmatchUnaryOp::OutputProject {
            retval.output_project()?;
        } else if self.op == PmatchUnaryOp::Complement {
            // Defined here only for automata, so can project to input
            let mut complement =
                HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
            complement.repeat_star()?;
            complement.subtract(&retval, true)?;
            retval = complement;
        } else if self.op == PmatchUnaryOp::Containment {
            let mut any = HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
            any.repeat_star()?;
            let mut left = HfstTransducer::new_copy(&any)?;
            left.concatenate(&retval, true)?;
            left.concatenate(&any, true)?;
            retval = left;
        } else if self.op == PmatchUnaryOp::ContainmentOnce {
            let mut xre_comp = crate::xre::XreCompiler::new();
            retval = xre_comp.contains_once(&retval)?;
        } else if self.op == PmatchUnaryOp::ContainmentOptional {
            let mut xre_comp = crate::xre::XreCompiler::new();
            retval = xre_comp.contains_once_optional(&retval)?;
        } else if self.op == PmatchUnaryOp::TermComplement {
            let mut any = HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
            let alphabet: StringSet = get_non_special_alphabet(&retval)?;
            for it in alphabet.iter() {
                let symbol = HfstTransducer::new_symbol(it)?;
                any.subtract(&symbol, true)?;
            }
            retval = any;
        } else if self.op == PmatchUnaryOp::Cap {
            retval = ctx.with_utils(|u| u.cap(&retval, Side::Both, false))?;
        } else if self.op == PmatchUnaryOp::OptCap {
            retval = ctx.with_utils(|u| u.cap(&retval, Side::Both, true))?;
        } else if self.op == PmatchUnaryOp::ToLower {
            retval = ctx.with_utils(|u| u.tolower(&retval, Side::Both, false))?;
        } else if self.op == PmatchUnaryOp::ToUpper {
            retval = ctx.with_utils(|u| u.toupper(&retval, Side::Both, false))?;
        } else if self.op == PmatchUnaryOp::OptToLower {
            let mut tmp = ctx.with_utils(|u| u.tolower(&retval, Side::Both, true))?;
            tmp.disjunct(&retval, true)?;
            retval = tmp;
        } else if self.op == PmatchUnaryOp::OptToUpper {
            retval = ctx.with_utils(|u| u.toupper(&retval, Side::Both, true))?;
        } else if self.op == PmatchUnaryOp::AnyCase {
            let (toupper, tolower) = ctx.with_utils(|u| {
                Ok((
                    u.toupper(&retval, Side::Both, true)?,
                    u.tolower(&retval, Side::Both, true)?,
                ))
            })?;
            retval.disjunct(&toupper, true)?;
            retval.disjunct(&tolower, true)?;
        } else if self.op == PmatchUnaryOp::CapUpper {
            retval = ctx.with_utils(|u| u.cap(&retval, Side::Upper, false))?;
        } else if self.op == PmatchUnaryOp::OptCapUpper {
            retval = ctx.with_utils(|u| u.cap(&retval, Side::Upper, true))?;
        } else if self.op == PmatchUnaryOp::ToLowerUpper {
            retval = ctx.with_utils(|u| u.tolower(&retval, Side::Upper, false))?;
        } else if self.op == PmatchUnaryOp::ToUpperUpper {
            retval = ctx.with_utils(|u| u.toupper(&retval, Side::Upper, false))?;
        } else if self.op == PmatchUnaryOp::OptToLowerUpper {
            let mut tmp = ctx.with_utils(|u| u.tolower(&retval, Side::Upper, true))?;
            tmp.disjunct(&retval, true)?;
            retval = tmp;
        } else if self.op == PmatchUnaryOp::OptToUpperUpper {
            retval = ctx.with_utils(|u| u.toupper(&retval, Side::Upper, true))?;
        } else if self.op == PmatchUnaryOp::AnyCaseUpper {
            let (toupper, tolower) = ctx.with_utils(|u| {
                Ok((
                    u.toupper(&retval, Side::Upper, true)?,
                    u.tolower(&retval, Side::Upper, true)?,
                ))
            })?;
            retval.disjunct(&toupper, true)?;
            retval.disjunct(&tolower, true)?;
        } else if self.op == PmatchUnaryOp::CapLower {
            retval = ctx.with_utils(|u| u.cap(&retval, Side::Lower, false))?;
        } else if self.op == PmatchUnaryOp::OptCapLower {
            retval = ctx.with_utils(|u| u.cap(&retval, Side::Lower, true))?;
        } else if self.op == PmatchUnaryOp::ToLowerLower {
            retval = ctx.with_utils(|u| u.tolower(&retval, Side::Lower, false))?;
        } else if self.op == PmatchUnaryOp::ToUpperLower {
            retval = ctx.with_utils(|u| u.toupper(&retval, Side::Lower, false))?;
        } else if self.op == PmatchUnaryOp::OptToLowerLower {
            let mut tmp = ctx.with_utils(|u| u.tolower(&retval, Side::Lower, true))?;
            tmp.disjunct(&retval, true)?;
            retval = tmp;
        } else if self.op == PmatchUnaryOp::OptToUpperLower {
            retval = ctx.with_utils(|u| u.toupper(&retval, Side::Lower, true))?;
        } else if self.op == PmatchUnaryOp::AnyCaseLower {
            let (toupper, tolower) = ctx.with_utils(|u| {
                Ok((
                    u.toupper(&retval, Side::Lower, true)?,
                    u.tolower(&retval, Side::Lower, true)?,
                ))
            })?;
            retval.disjunct(&toupper, true)?;
            retval.disjunct(&tolower, true)?;
        } else if self.op == PmatchUnaryOp::MakeSigma {
            retval = make_sigma(ctx, &retval)?;
        } else if self.op == PmatchUnaryOp::MakeList {
            let tmp = make_list(&retval)?;
            register_lst_line_numbers_from_transducer(ctx, &tmp, self.line_defined)?;
            retval = tmp;
        } else if self.op == PmatchUnaryOp::MakeExcList {
            retval = make_exc_list(&retval)?;
        } else if self.op == PmatchUnaryOp::LC {
            if !transducer_has_context_symbol(&retval)? {
                retval.reverse()?;
                let mut tmp = HfstTransducer::new_symbol_pair(
                    crate::hfst_symbol_defs::internal_epsilon,
                    LC_ENTRY_SYMBOL,
                )?;
                tmp.concatenate(&retval, true)?;
                let lc_exit = HfstTransducer::new_symbol_pair(
                    crate::hfst_symbol_defs::internal_epsilon,
                    LC_EXIT_SYMBOL,
                )?;
                tmp.concatenate(&lc_exit, true)?;
                retval = tmp;
            } else if ctx.verbose {
                write_compilation_stack_indentation_to_err(ctx);
                debug!(
                    "** Warning: ignoring nested context condition when compiling {}",
                    ctx.eval_stack_last()
                        .expect("eval_stack has an entry during evaluation")
                );
            }
        } else if self.op == PmatchUnaryOp::NLC {
            if !transducer_has_context_symbol(&retval)? {
                retval.reverse()?;
                let tmp = ctx.make_minimization_guard()?;
                let mut head = tmp.evaluate(ctx)?;
                let passthrough = HfstTransducer::new_symbol(PASSTHROUGH_SYMBOL)?;
                let mut nlc_entry = HfstTransducer::new_symbol_pair(
                    crate::hfst_symbol_defs::internal_epsilon,
                    NLC_ENTRY_SYMBOL,
                )?;
                let nlc_exit = HfstTransducer::new_symbol_pair(
                    crate::hfst_symbol_defs::internal_epsilon,
                    NLC_EXIT_SYMBOL,
                )?;
                nlc_entry.concatenate(&retval, true)?;
                nlc_entry.concatenate(&nlc_exit, true)?;
                nlc_entry.disjunct(&passthrough, true)?;
                head.concatenate(&nlc_entry, true)?;
                retval = head;
            } else if ctx.verbose {
                write_compilation_stack_indentation_to_err(ctx);
                debug!(
                    "** Warning: ignoring nested context condition when compiling {}",
                    ctx.eval_stack_last()
                        .expect("eval_stack has an entry during evaluation")
                );
            }
        } else if self.op == PmatchUnaryOp::RC {
            if !transducer_has_context_symbol(&retval)? {
                let mut tmp = HfstTransducer::new_symbol_pair(
                    crate::hfst_symbol_defs::internal_epsilon,
                    RC_ENTRY_SYMBOL,
                )?;
                tmp.concatenate(&retval, true)?;
                let rc_exit = HfstTransducer::new_symbol_pair(
                    crate::hfst_symbol_defs::internal_epsilon,
                    RC_EXIT_SYMBOL,
                )?;
                tmp.concatenate(&rc_exit, true)?;
                retval = tmp;
            } else if ctx.verbose {
                write_compilation_stack_indentation_to_err(ctx);
                debug!(
                    "** Warning: ignoring nested context condition when compiling {}",
                    ctx.eval_stack_last()
                        .expect("eval_stack has an entry during evaluation")
                );
            }
        } else if self.op == PmatchUnaryOp::NRC {
            if !transducer_has_context_symbol(&retval)? {
                let tmp = ctx.make_minimization_guard()?;
                let mut head = tmp.evaluate(ctx)?;
                let passthrough = HfstTransducer::new_symbol(PASSTHROUGH_SYMBOL)?;
                let mut nrc_entry = HfstTransducer::new_symbol_pair(
                    crate::hfst_symbol_defs::internal_epsilon,
                    NRC_ENTRY_SYMBOL,
                )?;
                let nrc_exit = HfstTransducer::new_symbol_pair(
                    crate::hfst_symbol_defs::internal_epsilon,
                    NRC_EXIT_SYMBOL,
                )?;
                nrc_entry.concatenate(&retval, true)?;
                nrc_entry.concatenate(&nrc_exit, true)?;
                nrc_entry.disjunct(&passthrough, true)?;
                head.concatenate(&nrc_entry, true)?;
                retval = head;
            } else if ctx.verbose {
                write_compilation_stack_indentation_to_err(ctx);
                debug!(
                    "** Warning: ignoring nested context condition when compiling {}",
                    ctx.eval_stack_last()
                        .expect("eval_stack has an entry during evaluation")
                );
            }
        }
        retval.set_final_weights(self.weight as f32, true)?;

        if !self.name.is_empty() {
            ctx.eval_stack_pop();
        }
        if ctx.node_cache_get(key).is_none() && self.should_use_cache(ctx) {
            ctx.node_cache_put(key, retval);
            ctx.node_cache_get_mut(key)
                .expect("cache populated above")
                .minimize()?;
            self.report_time(
                ctx,
                my_timer,
                " with ".to_string()
                    + &get_size_info(ctx.node_cache_get(key).expect("just inserted")),
            );
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just inserted"));
        }
        self.report_time(ctx, my_timer, String::new());
        Ok(retval)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-unary-operation.get-initial-symbols-from-unary-root-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-unary-operation.get-initial-symbols-from-unary-root-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation.get-initial-symbols-from-unary-root-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-unary-operation.get-initial-symbols-from-unary-root-fn]
    fn get_initial_symbols_from_unary_root(
        &self,
        ctx: &mut PmatchEvalContext<B>,
    ) -> crate::error::Result<StringSet> {
        PmatchObject_get_real_initial_symbols(ctx, &*self.root)
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-unary-operation.is-context-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-unary-operation.is-context-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation.is-context-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-unary-operation.is-context-fn]
    fn is_context(&self) -> bool {
        self.op == PmatchUnaryOp::LC
            || self.op == PmatchUnaryOp::NLC
            || self.op == PmatchUnaryOp::RC
            || self.op == PmatchUnaryOp::NRC
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-unary-operation.is-delimiter-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-unary-operation.is-delimiter-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation.is-delimiter-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-unary-operation.is-delimiter-fn]
    fn is_delimiter(&self) -> bool {
        self.op == PmatchUnaryOp::AddDelimiters
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-unary-operation.get-initial-rc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-unary-operation.get-initial-rc-initial-symbols-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation.get-initial-rc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-unary-operation.get-initial-rc-initial-symbols-fn]
    fn get_initial_RC_initial_symbols(
        &self,
        ctx: &mut PmatchEvalContext<B>,
    ) -> crate::error::Result<StringSet> {
        if self.op == PmatchUnaryOp::RC {
            let tmp: HfstTransducer<B> = self.root.evaluate(ctx)?;
            return Ok(tmp.get_initial_input_symbols());
        }
        if self.op == PmatchUnaryOp::AddDelimiters {
            return self.root.get_initial_RC_initial_symbols(ctx);
        }
        Ok(StringSet::new())
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-unary-operation.get-initial-nrc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-unary-operation.get-initial-nrc-initial-symbols-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation.get-initial-nrc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-unary-operation.get-initial-nrc-initial-symbols-fn]
    fn get_initial_NRC_initial_symbols(
        &self,
        ctx: &mut PmatchEvalContext<B>,
    ) -> crate::error::Result<StringSet> {
        if self.op == PmatchUnaryOp::NRC {
            let tmp: HfstTransducer<B> = self.root.evaluate(ctx)?;
            return Ok(tmp.get_initial_input_symbols());
        }
        if self.op == PmatchUnaryOp::AddDelimiters {
            return self.root.get_initial_NRC_initial_symbols(ctx);
        }
        Ok(StringSet::new())
    }
}

// ===== body: binary-ternary-numeric-eval =====
// [spec:hfst:def:pmatch-utils.hfst.pmatch-numeric-operation.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-numeric-operation.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-numeric-operation.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-numeric-operation.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchNumericOperation<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        let key = self.cache_key();
        if ctx.node_cache_get(key).is_some() {
            self.report_cache(ctx, String::new());
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just checked"));
        }
        let my_timer = self.start_timing(ctx);
        if !self.name.is_empty() {
            ctx.eval_stack_push(self.name.clone());
        }
        let mut tmp: HfstTransducer<B> = self.root.evaluate(ctx)?;
        if self.op == PmatchNumericOp::RepeatN {
            tmp.repeat_n(self.values[0] as u32)?;
        } else if self.op == PmatchNumericOp::RepeatNPlus {
            tmp.repeat_n_plus(self.values[0] as u32)?;
        } else if self.op == PmatchNumericOp::RepeatNMinus {
            tmp.repeat_n_minus(self.values[0] as u32)?;
        } else if self.op == PmatchNumericOp::RepeatNToK {
            tmp.repeat_n_to_k(self.values[0] as u32, self.values[1] as u32)?;
        }
        tmp.set_final_weights(self.weight as f32, true)?;
        if !self.name.is_empty() {
            ctx.eval_stack_pop();
        }
        if ctx.node_cache_get(key).is_none() && self.should_use_cache(ctx) {
            ctx.node_cache_put(key, tmp);
            ctx.node_cache_get_mut(key)
                .expect("cache populated above")
                .minimize()?;
            self.report_time(ctx, my_timer, String::new());
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just inserted"));
        }
        self.report_time(ctx, my_timer, String::new());
        Ok(tmp)
    }
}
// Pass a map from Lst() symbol to line number. Emits one warning per Lst()
// symbol (so line numbers stay unambiguous).
// [spec:hfst:def:pmatch-utils.hfst.fix-list-overlap-fn]
// [spec:hfst:sem:pmatch-utils.hfst.fix-list-overlap-fn]
pub fn fix_list_overlap<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    lhs: &mut HfstTransducer<B>,
    rhs: &mut HfstTransducer<B>,
    list_set: &StringSet,
    literal_set: &StringSet,
    lst_line_map: &BTreeMap<String, i32>,
) -> crate::error::Result<()> {
    {
        for sym in list_set.iter() {
            if !sym.starts_with("@L.") {
                continue;
            }

            let mut overlapping_chars: Vec<String> = Vec::new();
            let mut retained_chars: Vec<String> = Vec::new();
            let mut lst_line: i32 = -1;
            if let Some(line) = lst_line_map.get(sym.as_str()) {
                lst_line = *line;
            }

            // Parse the list content: @L.a_b_c@ -> a, b, c
            let mut start: usize = 3; // Skip @L.
            let mut end: Option<usize> = sym[start..].find('_').map(|i| i + start);
            while let Some(e) = end {
                let sub = sym[start..e].to_string();
                if literal_set.contains(sub.as_str()) {
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
                if literal_set.contains(last.as_str()) {
                    overlapping_chars.push(last);
                }
            }

            if overlapping_chars.is_empty() {
                continue;
            }

            // Ensure each Lst() only triggers a single warning per compilation.
            let warn_key: String = if lst_line >= 0 {
                let mut wk = lst_line.to_string();
                wk.push('\t');
                wk.push_str(sym);
                wk
            } else {
                sym.to_string()
            };
            if ctx.lst_overlap_warned_contains(&warn_key) {
                continue;
            }
            ctx.lst_overlap_warned_insert(warn_key);
            let mut newlist = String::from("@L.");
            for s in retained_chars.iter() {
                newlist.push_str(s);
                newlist.push('_');
            }
            newlist.push('@');
            let newsym: StringPair = (Symbol::from(newlist.clone()), Symbol::from(newlist.clone()));
            let mut newpairs: StringPairSet = StringPairSet::new();
            newpairs.insert(newsym);
            let mut optimise_msg = String::new();
            if ctx.verbose {
                optimise_msg.push_str(&format!(
                    "Automatically optimising:Removing the following symbols from Lst() (line {}):",
                    if lst_line >= 0 {
                        lst_line.to_string()
                    } else {
                        "?".to_string()
                    }
                ));
            }
            for overlapping_char in overlapping_chars.iter() {
                let overlapsym: StringPair = (
                    Symbol::from(overlapping_char.clone()),
                    Symbol::from(overlapping_char.clone()),
                );
                if ctx.verbose {
                    optimise_msg.push_str(&format!("\n  '{}' (", overlapping_char));
                    let mut buf: Vec<u8> = Vec::new();
                    print_unicode_codepoints(&mut buf, overlapping_char);
                    optimise_msg.push_str(&String::from_utf8_lossy(&buf));
                    optimise_msg.push(')');
                }
                newpairs.insert(overlapsym);
            }
            if ctx.verbose {
                debug!("{}", optimise_msg);
                debug!(
                    "Replacing all {} instances with new list: {} and abovementioned disjunction ",
                    sym, newlist
                );
            }
            let oldsym: StringPair = (sym.clone(), sym.clone());
            lhs.substitute_pair_with_pair_set(&oldsym, &newpairs)?;
            rhs.substitute_pair_with_pair_set(&oldsym, &newpairs)?;
        }
    }
    Ok(())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchBinaryOperation<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        let key = self.cache_key();
        if ctx.node_cache_get(key).is_some() {
            self.report_cache(ctx, String::new());
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just checked"));
        }
        let my_timer = self.start_timing(ctx);

        // Special optimization cases
        if self.op == PmatchBinaryOp::Disjunct
            && self.left.is_unweighted_disjunction_of_strings()
            && self.right.is_unweighted_disjunction_of_strings()
        {
            let mut strings: StringVector = StringVector::new();
            self.left.collect_strings_into(ctx, &mut strings);
            self.right.collect_strings_into(ctx, &mut strings);
            let tok = crate::hfst_tokenizer::HfstTokenizer::new();
            let mut retval = HfstTransducer::new();
            for it in strings.iter() {
                let spv = tok.tokenize(it, false); // XXX
                retval.disjunct_spv(&spv)?;
            }
            retval.set_final_weights(self.weight as f32, true)?;
            if ctx.node_cache_get(key).is_none() && self.should_use_cache(ctx) {
                ctx.node_cache_put(key, retval);
                // No minimization because we did it the clever way!
                self.report_time(
                    ctx,
                    my_timer,
                    format!(
                        " with {}",
                        get_size_info(ctx.node_cache_get(key).expect("just inserted"))
                    ),
                );
                return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just inserted"));
            }
            self.report_time(ctx, my_timer, String::new());
            return Ok(retval);
        }

        if !self.name.is_empty() {
            ctx.eval_stack_push(self.name.clone());
        }
        // General cases
        let mut lhs: HfstTransducer<B> = self.left.evaluate(ctx)?;
        let mut rhs: HfstTransducer<B> = self.right.evaluate(ctx)?;
        match self.op {
            PmatchBinaryOp::Concatenate => {
                lhs.concatenate(&rhs, true)?;
            }
            PmatchBinaryOp::Compose => {
                lhs.compose(&rhs, true)?;
            }
            PmatchBinaryOp::CrossProduct => {
                lhs.cross_product(&rhs, true)?;
            }
            PmatchBinaryOp::LenientCompose => {
                lhs.lenient_composition(&rhs, true)?;
            }
            PmatchBinaryOp::Disjunct => {
                let lhs_syms = lhs.get_alphabet()?;
                let rhs_syms = rhs.get_alphabet()?;
                let lst_lines = ctx.lst_line_map_snapshot();
                fix_list_overlap(ctx, &mut lhs, &mut rhs, &lhs_syms, &rhs_syms, &lst_lines)?;
                fix_list_overlap(ctx, &mut rhs, &mut lhs, &rhs_syms, &lhs_syms, &lst_lines)?;
                lhs.disjunct(&rhs, true)?;
            }
            PmatchBinaryOp::Intersect => {
                lhs.intersect(&rhs, true)?;
            }
            PmatchBinaryOp::Subtract => {
                if ctx.verbose {
                    warn_on_nonsubtractable_symbols(ctx, &lhs)?;
                    warn_on_nonsubtractable_symbols(ctx, &rhs)?;
                }
                lhs.subtract(&rhs, true)?;
            }
            PmatchBinaryOp::UpperSubtract => {
                ctx.pmatcherror("Upper subtraction not implemented.");
                return Ok(lhs);
            }
            PmatchBinaryOp::LowerSubtract => {
                ctx.pmatcherror("Lower subtraction not implemented.");
                return Ok(lhs);
            }
            PmatchBinaryOp::UpperPriorityUnion => {
                lhs.priority_union(&rhs)?;
            }
            PmatchBinaryOp::LowerPriorityUnion => {
                lhs.invert()?;
                rhs.invert()?;
                lhs.priority_union(&rhs)?;
                lhs.invert()?;
            }
            PmatchBinaryOp::Shuffle => {
                let __res = lhs.shuffle(&rhs, true).map(|_| ());
                if let Err(e) = __res {
                    if matches!(e.kind, crate::error::ErrorKind::TransducersAreNotAutomata) {
                        pmatchwarning(
                            "tried to shuffle with non-automaton transducers,\n    shuffling with their input projection instead.",
                        );
                        lhs.input_project()?;
                        rhs.input_project()?;
                        lhs.shuffle(&rhs, true)?;
                    } else {
                        return Err(e);
                    }
                }
            }
            PmatchBinaryOp::Before => {
                lhs = crate::hfst_xerox_rules::before(&lhs, &rhs)?;
            }
            PmatchBinaryOp::After => {
                lhs = crate::hfst_xerox_rules::after(&lhs, &rhs)?;
            }
            PmatchBinaryOp::InsertFreely => {
                lhs.insert_freely(&rhs, false)?;
            }
            PmatchBinaryOp::IgnoreInternally => {
                let right_part: HfstTransducer<B> = HfstTransducer::new_copy(&lhs)?;
                let mut middle_part: HfstTransducer<B> = HfstTransducer::new_copy(&lhs)?;
                middle_part.disjunct(&rhs, true)?;
                middle_part.repeat_star()?;
                lhs.concatenate(&middle_part, true)?;
                lhs.concatenate(&right_part, true)?;
            }
            PmatchBinaryOp::Merge => {
                // hfst::xre::merge_first_to_second(lhs, rhs)
                let args = crate::xre::XreConstructorArguments::new(
                    BTreeMap::new(),
                    BTreeMap::new(),
                    BTreeMap::new(),
                    BTreeMap::new(),
                );
                lhs.optimize()?;
                let __res = rhs.merge(&lhs, &args).map(|_| ());
                match __res {
                    Ok(()) => {
                        // NB: mirrors the C++ aliasing (lhs now becomes rhs).
                        lhs = std::mem::take(&mut rhs);
                    }
                    Err(e) => {
                        if matches!(e.kind, crate::error::ErrorKind::TransducersAreNotAutomata) {
                            ctx.pmatcherror(
                                "Error: transducers must be automata in merge operation.",
                            );
                            unreachable!("pmatcherror panics");
                        } else {
                            return Err(e);
                        }
                    }
                }
            }
        }
        drop(rhs);
        lhs.set_final_weights(self.weight as f32, true)?;
        if !self.name.is_empty() {
            ctx.eval_stack_pop();
        }
        if ctx.node_cache_get(key).is_none() && self.should_use_cache(ctx) {
            ctx.node_cache_put(key, lhs);
            ctx.node_cache_get_mut(key)
                .expect("cache populated above")
                .minimize()?;
            self.report_time(
                ctx,
                my_timer,
                format!(
                    " with {}",
                    get_size_info(ctx.node_cache_get(key).expect("just inserted"))
                ),
            );
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just inserted"));
        }
        self.report_time(ctx, my_timer, String::new());
        Ok(lhs)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.get-real-initial-symbols-from-right-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.get-real-initial-symbols-from-right-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.get-real-initial-symbols-from-right-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.get-real-initial-symbols-from-right-fn]
    fn get_real_initial_symbols_from_right(
        &self,
        ctx: &mut PmatchEvalContext<B>,
    ) -> crate::error::Result<StringSet> {
        PmatchObject_get_real_initial_symbols(ctx, &*self.right)
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.is-left-concatenation-with-context-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.is-left-concatenation-with-context-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.is-left-concatenation-with-context-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.is-left-concatenation-with-context-fn]
    fn is_left_concatenation_with_context(&self) -> bool {
        self.op == PmatchBinaryOp::Concatenate && self.left.is_context()
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.get-initial-rc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.get-initial-rc-initial-symbols-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.get-initial-rc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.get-initial-rc-initial-symbols-fn]
    fn get_initial_RC_initial_symbols(
        &self,
        ctx: &mut PmatchEvalContext<B>,
    ) -> crate::error::Result<StringSet> {
        let mut retval: StringSet = StringSet::new();
        if self.op == PmatchBinaryOp::Concatenate {
            let left_ss: StringSet = self.left.get_initial_RC_initial_symbols(ctx)?;
            let mut right_ss: StringSet = StringSet::new();
            if self.right.is_context() || self.right.is_delimiter() {
                right_ss = self.right.get_initial_NRC_initial_symbols(ctx)?;
            }
            for it in left_ss.iter() {
                retval.insert(it.clone());
            }
            for it in right_ss.iter() {
                retval.insert(it.clone());
            }
            return Ok(retval);
        }
        Ok(retval)
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.get-initial-nrc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.get-initial-nrc-initial-symbols-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.get-initial-nrc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.get-initial-nrc-initial-symbols-fn]
    fn get_initial_NRC_initial_symbols(
        &self,
        ctx: &mut PmatchEvalContext<B>,
    ) -> crate::error::Result<StringSet> {
        let mut retval: StringSet = StringSet::new();
        if self.op == PmatchBinaryOp::Concatenate {
            let left_ss: StringSet = self.left.get_initial_NRC_initial_symbols(ctx)?;
            let mut right_ss: StringSet = StringSet::new();
            if self.right.is_context() || self.right.is_delimiter() {
                right_ss = self.right.get_initial_NRC_initial_symbols(ctx)?;
            }
            for it in left_ss.iter() {
                retval.insert(it.clone());
            }
            for it in right_ss.iter() {
                retval.insert(it.clone());
            }
            return Ok(retval);
        }
        Ok(retval)
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.collect-strings-into-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.collect-strings-into-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.collect-strings-into-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.collect-strings-into-fn]
    fn collect_strings_into(&self, ctx: &mut PmatchEvalContext<B>, strings: &mut StringVector) {
        self.left.collect_strings_into(ctx, strings);
        self.right.collect_strings_into(ctx, strings);
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.as-string-pair-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.as-string-pair-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.as-string-pair-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.as-string-pair-fn]
    fn as_string_pair(&self, ctx: &mut PmatchEvalContext<B>) -> StringPair {
        if self.op == PmatchBinaryOp::CrossProduct {
            let left_string: String = self.left.as_string(ctx);
            let right_string: String = self.right.as_string(ctx);
            return (Symbol::from(left_string), Symbol::from(right_string));
        }
        (Symbol::new_static(""), Symbol::new_static(""))
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.is-unweighted-disjunction-of-strings-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.is-unweighted-disjunction-of-strings-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.is-unweighted-disjunction-of-strings-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.is-unweighted-disjunction-of-strings-fn]
    fn is_unweighted_disjunction_of_strings(&self) -> bool {
        self.weight == 0.0
            && self.op == PmatchBinaryOp::Disjunct
            && self.left.is_unweighted_disjunction_of_strings()
            && self.right.is_unweighted_disjunction_of_strings()
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-ternary-operation.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-ternary-operation.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-ternary-operation.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-ternary-operation.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchTernaryOperation<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        let key = self.cache_key();
        if ctx.node_cache_get(key).is_some() {
            self.report_cache(ctx, String::new());
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just checked"));
        }
        let my_timer = self.start_timing(ctx);
        if !self.name.is_empty() {
            ctx.eval_stack_push(self.name.clone());
        }
        let mut retval: HfstTransducer<B> = self.left.evaluate(ctx)?;
        if self.op == PmatchTernaryOp::Substitute {
            let middle_pair: StringPair = self.middle.as_string_pair(ctx);
            let right_pair: StringPair = self.right.as_string_pair(ctx);
            if !right_pair.0.is_empty() || !right_pair.1.is_empty() {
                retval.substitute_pair_with_pair(&middle_pair, &right_pair)?;
            } else {
                let mut tmp: HfstTransducer<B> = self.right.evaluate(ctx)?;
                retval.substitute_pair_with_transducer(&middle_pair, &mut tmp, true)?;
            }
        } else if self.op == PmatchTernaryOp::Uncompose {
            let _unc_left: HfstTransducer<B> = self.middle.evaluate(ctx)?;
            let _unc_right: HfstTransducer<B> = self.right.evaluate(ctx)?;
        }
        retval.set_final_weights(self.weight as f32, true)?;
        if ctx.node_cache_get(key).is_none() && self.should_use_cache(ctx) {
            ctx.node_cache_put(key, retval);
            ctx.node_cache_get_mut(key)
                .expect("cache populated above")
                .minimize()?;
            self.report_time(ctx, my_timer, String::new());
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just inserted"));
        }
        self.report_time(ctx, my_timer, String::new());
        if !self.name.is_empty() {
            ctx.eval_stack_pop();
        }
        Ok(retval)
    }
}
// [spec:hfst:def:pmatch-utils.hfst.transducer-has-context-symbol-fn]
// [spec:hfst:sem:pmatch-utils.hfst.transducer-has-context-symbol-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.transducer-has-context-symbol-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.transducer-has-context-symbol-fn]
pub fn transducer_has_context_symbol<B: AlgebraBackend>(
    t: &HfstTransducer<B>,
) -> crate::error::Result<bool> {
    let ss: StringSet = t.get_alphabet()?;
    Ok(ss.contains(LC_ENTRY_SYMBOL)
        || ss.contains(NLC_ENTRY_SYMBOL)
        || ss.contains(RC_ENTRY_SYMBOL)
        || ss.contains(NRC_ENTRY_SYMBOL))
}
// [spec:hfst:def:pmatch-utils.hfst.warn-on-nonsubtractable-symbols-fn]
// [spec:hfst:sem:pmatch-utils.hfst.warn-on-nonsubtractable-symbols-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.warn-on-nonsubtractable-symbols-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.warn-on-nonsubtractable-symbols-fn]
pub fn warn_on_nonsubtractable_symbols<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    t: &HfstTransducer<B>,
) -> crate::error::Result<()> {
    let alphabet: StringSet = t.get_alphabet()?;
    for it in alphabet.iter() {
        if it.len() < 3 {
            continue;
        } else if it.starts_with("@PMATCH") || it.starts_with("@I") || it.starts_with("@L") {
            write_compilation_stack_indentation_to_err(ctx);
            warn!("subtracting with nonsubtractable symbol {}", it);
        }
    }
    Ok(())
}

// ===== body: replace-restriction-containers =====
// [spec:hfst:def:pmatch-utils.hfst.pmatch-parallel-rules-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-parallel-rules-container.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-parallel-rules-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-parallel-rules-container.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchParallelRulesContainer<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        let key = self.cache_key();
        if ctx.node_cache_get(key).is_some() {
            self.report_cache(ctx, String::new());
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just checked"));
        }
        let my_timer = self.start_timing(ctx);
        let mut retval: HfstTransducer<B> = match self.arrow {
            ReplaceArrow::E_REPLACE_RIGHT => replace_rule_vector(&self.make_mappings(ctx)?, false)?,
            ReplaceArrow::E_OPTIONAL_REPLACE_RIGHT => {
                replace_rule_vector(&self.make_mappings(ctx)?, true)?
            }
            ReplaceArrow::E_REPLACE_LEFT => {
                replace_left_rule_vector(&self.make_mappings(ctx)?, false)?
            }
            ReplaceArrow::E_OPTIONAL_REPLACE_LEFT => {
                replace_left_rule_vector(&self.make_mappings(ctx)?, true)?
            }
            ReplaceArrow::E_RTL_LONGEST_MATCH => {
                replace_rightmost_longest_match_rule_vector(&self.make_mappings(ctx)?)?
            }
            ReplaceArrow::E_RTL_SHORTEST_MATCH => {
                replace_rightmost_shortest_match_rule_vector(&self.make_mappings(ctx)?)?
            }
            ReplaceArrow::E_LTR_LONGEST_MATCH => {
                replace_leftmost_longest_match_rule_vector(&self.make_mappings(ctx)?)?
            }
            ReplaceArrow::E_LTR_SHORTEST_MATCH => {
                replace_leftmost_shortest_match_rule_vector(&self.make_mappings(ctx)?)?
            }
            ReplaceArrow::E_REPLACE_RIGHT_MARKUP => {
                ctx.pmatcherror("Unrecognized arrow type");
                return Ok(HfstTransducer::new());
            }
        };
        retval.set_final_weights(self.weight as f32, true)?;
        self.report_time(ctx, my_timer, String::new());
        if ctx.node_cache_get(key).is_none() && self.should_use_cache(ctx) {
            ctx.node_cache_put(key, retval);
            ctx.node_cache_get_mut(key)
                .expect("cache populated above")
                .minimize()?;
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just inserted"));
        }
        Ok(retval)
    }
}
impl<B: AlgebraBackend + 'static> PmatchParallelRulesContainer<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-parallel-rules-container.make-mappings-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-parallel-rules-container.make-mappings-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-parallel-rules-container.make-mappings-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-parallel-rules-container.make-mappings-fn]
    pub fn make_mappings(
        &self,
        ctx: &mut PmatchEvalContext<B>,
    ) -> crate::error::Result<Vec<Rule<B>>> {
        self.rules.iter().map(|it| it.make_mapping(ctx)).collect()
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-replace-rule-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-replace-rule-container.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-replace-rule-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-replace-rule-container.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchReplaceRuleContainer<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        let key = self.cache_key();
        if ctx.node_cache_get(key).is_some() {
            self.report_cache(ctx, String::new());
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just checked"));
        }
        let my_timer = self.start_timing(ctx);
        let mut retval: HfstTransducer<B> = match self.arrow {
            ReplaceArrow::E_REPLACE_RIGHT => replace_rule(&self.make_mapping(ctx)?, false)?,
            ReplaceArrow::E_OPTIONAL_REPLACE_RIGHT => replace_rule(&self.make_mapping(ctx)?, true)?,
            ReplaceArrow::E_REPLACE_LEFT => replace_left_rule(&self.make_mapping(ctx)?, false)?,
            ReplaceArrow::E_OPTIONAL_REPLACE_LEFT => {
                replace_left_rule(&self.make_mapping(ctx)?, true)?
            }
            ReplaceArrow::E_RTL_LONGEST_MATCH => {
                replace_rightmost_longest_match_rule(&self.make_mapping(ctx)?)?
            }
            ReplaceArrow::E_RTL_SHORTEST_MATCH => {
                replace_rightmost_shortest_match_rule(&self.make_mapping(ctx)?)?
            }
            ReplaceArrow::E_LTR_LONGEST_MATCH => {
                replace_leftmost_longest_match_rule(&self.make_mapping(ctx)?)?
            }
            ReplaceArrow::E_LTR_SHORTEST_MATCH => {
                replace_leftmost_shortest_match_rule(&self.make_mapping(ctx)?)?
            }
            ReplaceArrow::E_REPLACE_RIGHT_MARKUP => {
                ctx.pmatcherror("Unrecognized arrow");
                return Ok(HfstTransducer::new());
            }
        };
        retval.set_final_weights(self.weight as f32, true)?;
        self.report_time(ctx, my_timer, String::new());
        if ctx.node_cache_get(key).is_none() && self.should_use_cache(ctx) {
            ctx.node_cache_put(key, retval);
            ctx.node_cache_get_mut(key)
                .expect("cache populated above")
                .minimize()?;
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just inserted"));
        }
        Ok(retval)
    }
}
impl<B: AlgebraBackend + 'static> PmatchReplaceRuleContainer<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-replace-rule-container.make-mapping-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-replace-rule-container.make-mapping-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-replace-rule-container.make-mapping-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-replace-rule-container.make-mapping-fn]
    pub fn make_mapping(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<Rule<B>> {
        let mut pair_vector: HfstTransducerPairVector<B> = Vec::new();
        for it in self.mapping.iter() {
            let pp: TransducerPointerPair<B> = it.evaluate_pair(ctx)?;
            let p: HfstTransducerPair<B> = (
                HfstTransducer::new_copy(&pp.0)?,
                HfstTransducer::new_copy(&pp.1)?,
            );
            pair_vector.push(p);
        }
        if self.context.is_empty() {
            return Rule::new_mapping(&pair_vector);
        }
        let mut context_vector: HfstTransducerPairVector<B> = Vec::new();
        for it in self.context.iter() {
            let pp: TransducerPointerPair<B> = it.evaluate_pair(ctx)?;
            let p: HfstTransducerPair<B> = (
                HfstTransducer::new_copy(&pp.0)?,
                HfstTransducer::new_copy(&pp.1)?,
            );
            context_vector.push(p);
        }
        Rule::new_mapping_context_repl_type(&pair_vector, &context_vector, self.ty)
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-restriction-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-restriction-container.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-restriction-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-restriction-container.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchRestrictionContainer<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        let key = self.cache_key();
        if ctx.node_cache_get(key).is_some() {
            self.report_cache(ctx, String::new());
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just checked"));
        }
        let my_timer = self.start_timing(ctx);
        let mut pair_vector: HfstTransducerPairVector<B> = Vec::new();
        for it in self.contexts.iter() {
            let pp: TransducerPointerPair<B> = it.evaluate_pair(ctx)?;
            let p: HfstTransducerPair<B> = (
                HfstTransducer::new_copy(&pp.0)?,
                HfstTransducer::new_copy(&pp.1)?,
            );
            pair_vector.push(p);
        }
        let l: HfstTransducer<B> = self.left.evaluate(ctx)?;
        let mut retval: HfstTransducer<B> = restriction(&l, &pair_vector)?;
        retval.set_final_weights(self.weight as f32, true)?;
        self.report_time(ctx, my_timer, String::new());
        if ctx.node_cache_get(key).is_none() && self.should_use_cache(ctx) {
            ctx.node_cache_put(key, retval);
            ctx.node_cache_get_mut(key)
                .expect("cache populated above")
                .minimize()?;
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just inserted"));
        }
        Ok(retval)
    }
}
impl<B: AlgebraBackend + 'static> PmatchObjectPairBase<B> for PmatchObjectPair<B> {
    fn get_left(&self) -> ObjRef<B> {
        self.left.clone()
    }
    fn get_right(&self) -> ObjRef<B> {
        self.right.clone()
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object-pair.evaluate-pair-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object-pair.evaluate-pair-fn]
    fn evaluate_pair(
        &self,
        ctx: &mut PmatchEvalContext<B>,
    ) -> crate::error::Result<TransducerPointerPair<B>> {
        let first = self.left.evaluate(ctx)?;
        let second = self.right.evaluate(ctx)?;
        Ok((first, second))
    }
}
impl<B: AlgebraBackend + 'static> PmatchObjectPairBase<B> for PmatchMarkupContainer<B> {
    fn get_left(&self) -> ObjRef<B> {
        self.left.clone()
    }
    fn get_right(&self) -> ObjRef<B> {
        self.right.clone()
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-markup-container.evaluate-pair-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-markup-container.evaluate-pair-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-markup-container.evaluate-pair-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-markup-container.evaluate-pair-fn]
    fn evaluate_pair(
        &self,
        ctx: &mut PmatchEvalContext<B>,
    ) -> crate::error::Result<TransducerPointerPair<B>> {
        let loa = self.left_of_arrow.evaluate(ctx)?;
        let lom = self.left.evaluate(ctx)?;
        let rom = self.right.evaluate(ctx)?;
        let tmp_mapping_pair: HfstTransducerPair<B> =
            (HfstTransducer::new_copy(&loa)?, HfstTransducer::new());
        let marks: HfstTransducerPair<B> = (
            HfstTransducer::new_copy(&lom)?,
            HfstTransducer::new_copy(&rom)?,
        );
        let mapping_pair: HfstTransducerPair<B> =
            create_mapping_for_mark_up_replace(&tmp_mapping_pair, &marks)?;
        Ok((
            HfstTransducer::new_copy(&mapping_pair.0)?,
            HfstTransducer::new_copy(&mapping_pair.1)?,
        ))
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-mapping-pairs-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-mapping-pairs-container.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-mapping-pairs-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-mapping-pairs-container.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchMappingPairsContainer<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        ctx.pmatcherror("Should never happen\n");
        unreachable!()
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-contexts-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-contexts-container.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-contexts-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-contexts-container.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchContextsContainer<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        ctx.pmatcherror("Should never happen\n");
        unreachable!()
    }
}

// ===== body: atom-symbol-function-eval =====
// [spec:hfst:def:pmatch-utils.hfst.pmatch-object.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.evaluate-fn]
// The C++ overload 'PmatchObject::evaluate(std::vector<PmatchObject*> args)'
// (the base default for the trait 'evaluate_args'); this is the shared
// weight/cache handling. ('pmatchlineno' does not exist in the AST-walk port,
// so the diagnostic uses 'line_defined'.)
pub fn PmatchObject_evaluate_args<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    this: &dyn PmatchObject<B>,
    args: Vec<ObjRef<B>>,
) -> crate::error::Result<HfstTransducer<B>> {
    if args.is_empty() {
        let key = this.cache_key();
        if this.should_use_cache(ctx) {
            if ctx.node_cache_get(key).is_none() {
                let my_timer = this.start_timing(ctx);
                let c = this.evaluate(ctx)?;
                ctx.node_cache_put(key, c);
                this.report_time(ctx, my_timer, String::new());
            }
            HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just inserted"))
        } else {
            let my_timer = this.start_timing(ctx);
            let mut retval = this.evaluate(ctx)?;
            retval.minimize()?;
            this.report_time(ctx, my_timer, String::new());
            Ok(retval)
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
// [spec:hfst:def:pmatch-utils.hfst.pmatch-object.expand-ins-arcs-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.expand-ins-arcs-fn]
pub fn PmatchObject_expand_Ins_arcs<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    this: &dyn PmatchObject<B>,
    ss: &mut StringSet,
) -> crate::error::Result<()> {
    {
        let mut did_no_expansions = false;
        let mut expansions_done: StringSet = StringSet::new();
        let mut expanded_symbols: StringSet = StringSet::new();
        let this_name = this.get_name().to_string();
        if !this_name.is_empty() {
            let mut this_name_insed = String::from("@I.");
            this_name_insed.push_str(&this_name);
            this_name_insed.push('@');
            expansions_done.insert(Symbol::from(this_name_insed));
        }
        while !did_no_expansions {
            did_no_expansions = true;
            for it in ss.iter() {
                if it.find("@I.") == Some(0) && it.rfind('@') == Some(it.len() - 1) {
                    // it's an Ins
                    if !expansions_done.contains(it) {
                        let ins_name = it[3..it.len() - 1].to_string();
                        did_no_expansions = false;
                        expansions_done.insert(it.clone());
                        if ctx.definitions_contains(&ins_name) {
                            let mut allowed: StringSet = StringSet::new();
                            let mut disallowed: StringSet = StringSet::new();
                            if ctx.def_insed_expressions_contains(&ins_name) {
                                ctx.def_insed_expressions_get(&ins_name)
                                    .expect("checked with contains just above")
                                    .collect_initial_symbols_into(&mut allowed, &mut disallowed)?;
                            } else {
                                ctx.definitions_get(&ins_name)
                                    .expect("definitions_contains verified above")
                                    .collect_initial_symbols_into(&mut allowed, &mut disallowed)?;
                            }
                            if !allowed.is_empty() {
                                for s in allowed.iter() {
                                    expanded_symbols.insert(s.clone());
                                }
                            } else {
                                expanded_symbols.insert(Symbol::new_static(internal_identity));
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
    Ok(())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-object.get-real-initial-symbols-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.get-real-initial-symbols-fn]
pub fn PmatchObject_get_real_initial_symbols<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    this: &dyn PmatchObject<B>,
) -> crate::error::Result<StringSet> {
    if this.is_left_concatenation_with_context() {
        return this.get_real_initial_symbols_from_right(ctx);
    }
    if this.is_delimiter() {
        return this.get_initial_symbols_from_unary_root(ctx);
    }
    let tmp: HfstTransducer<B> = this.evaluate(ctx)?;
    let retval: StringSet = tmp.get_initial_input_symbols();
    Ok(retval)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-object.collect-initial-symbols-into-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.collect-initial-symbols-into-fn]
pub fn PmatchObject_collect_initial_symbols_into<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    this: &dyn PmatchObject<B>,
    allowed_initial_symbols: &mut StringSet,
    disallowed_initial_symbols: &mut StringSet,
) -> crate::error::Result<()> {
    {
        // One or neither of allowed_initial_symbols and disallowed_initial_symbols
        // will have some symbols inserted to it.

        let mut allowed: StringSet = this.get_real_initial_symbols()?;
        let mut required: StringSet = this.get_initial_RC_initial_symbols(ctx)?;
        let mut disallowed: StringSet = this.get_initial_NRC_initial_symbols(ctx)?;

        // The first input symbols collected in this way may include:
        // - insertion arcs
        // - unknown, identity, default
        // - symbols after flag diacritics
        // - symbols from contexts
        PmatchObject_expand_Ins_arcs(ctx, this, &mut allowed)?;
        PmatchObject_expand_Ins_arcs(ctx, this, &mut required)?;
        PmatchObject_expand_Ins_arcs(ctx, this, &mut disallowed)?;

        if allowed.is_empty() {
            // Probably something went wrong, we'll just not make no judgement
            return Ok(());
        }

        if string_set_has_meta_arc(&mut allowed) {
            if !required.is_empty() && !string_set_has_meta_arc(&mut required) {
                // RC sets a constraint
                for it in required.iter() {
                    if !disallowed.contains(it) {
                        allowed_initial_symbols.insert(it.clone());
                    }
                }
                return Ok(());
            } else {
                // Anything goes except what is disallowed
                if disallowed.is_empty() || string_set_has_meta_arc(&mut disallowed) {
                    return Ok(());
                } else {
                    for s in disallowed.iter() {
                        disallowed_initial_symbols.insert(s.clone());
                    }
                    return Ok(());
                }
            }
        }

        // Now we can assume that "allowed" is nonempty and non-meta.

        if required.is_empty() || string_set_has_meta_arc(&mut required) {
            // RC poses no constraint
            for it in allowed.iter() {
                if !disallowed.contains(it) {
                    allowed_initial_symbols.insert(it.clone());
                }
            }
            return Ok(());
        }

        // Now we can assume that there is a genuine RC constraint.

        for it in required.iter() {
            if allowed.contains(it) && !disallowed.contains(it) {
                allowed_initial_symbols.insert(it.clone());
            }
        }
    }
    Ok(())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-symbol.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-symbol.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-symbol.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-symbol.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchSymbol<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        if !self.name.is_empty() {
            ctx.eval_stack_push(self.name.clone());
        }
        let my_timer = self.start_timing(ctx);
        let mut retval: HfstTransducer<B>;
        if symbol_in_local_context(ctx, &self.sym) {
            retval = symbol_from_local_context(ctx, &self.sym)
                .expect("symbol_in_local_context verified above")
                .evaluate(ctx)?;
        } else if symbol_in_global_context(ctx, &self.sym) {
            if ctx.flatten && ctx.def_insed_expressions_contains(&self.sym) {
                retval = ctx
                    .def_insed_expressions_get(&self.sym)
                    .expect("checked with contains just above")
                    .evaluate(ctx)?;
            } else {
                retval = symbol_from_global_context(ctx, &self.sym)
                    .expect("symbol_in_global_context verified above")
                    .evaluate(ctx)?;
            }
            ctx.used_definitions_insert(self.sym.clone());
        } else {
            if ctx.verbose {
                debug!(
                    "Warning: interpreting undefined symbol \"{}\" as label on line {}",
                    self.sym, self.line_defined
                );
            }
            retval = HfstTransducer::new_symbol(&self.sym)?;
        }
        retval.set_final_weights(self.weight as f32, true)?;
        retval.minimize()?;
        self.report_time(ctx, my_timer, String::new());
        if !self.name.is_empty() {
            ctx.eval_stack_pop();
        }
        Ok(retval)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-symbol.evaluate-as-arg-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-symbol.evaluate-as-arg-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-symbol.evaluate-as-arg-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-symbol.evaluate-as-arg-fn]
    fn evaluate_as_arg(&self, ctx: &mut PmatchEvalContext<B>) -> ObjRef<B> {
        if symbol_in_local_context(ctx, &self.sym) {
            return symbol_from_local_context(ctx, &self.sym)
                .expect("symbol_in_local_context verified above")
                .evaluate_as_arg(ctx);
        } else if symbol_in_global_context(ctx, &self.sym) {
            ctx.used_definitions_insert(self.sym.clone());
            if ctx.flatten && ctx.def_insed_expressions_contains(&self.sym) {
                return ctx
                    .def_insed_expressions_get(&self.sym)
                    .expect("checked with contains just above")
                    .evaluate_as_arg(ctx);
            } else {
                return symbol_from_global_context(ctx, &self.sym)
                    .expect("symbol_in_global_context verified above")
                    .evaluate_as_arg(ctx);
            }
        } else {
            if ctx.verbose {
                debug!(
                    "Warning: interpreting undefined symbol \"{}\" as label on line {}",
                    self.sym, self.line_defined
                );
            }
            Rc::new(PmatchString {
                name: String::new(),
                weight: 0.0,
                line_defined: 0,
                string: self.sym.clone(),
                multichar: false,
                _marker: std::marker::PhantomData,
            })
        }
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-symbol.collect-strings-into-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-symbol.collect-strings-into-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-symbol.collect-strings-into-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-symbol.collect-strings-into-fn]
    fn collect_strings_into(&self, ctx: &mut PmatchEvalContext<B>, strings: &mut StringVector) {
        if symbol_in_local_context(ctx, &self.sym) {
            symbol_from_local_context(ctx, &self.sym)
                .expect("symbol_in_local_context verified above")
                .collect_strings_into(ctx, strings);
        } else if symbol_in_global_context(ctx, &self.sym) {
            symbol_from_global_context(ctx, &self.sym)
                .expect("symbol_in_global_context verified above")
                .collect_strings_into(ctx, strings);
            ctx.used_definitions_insert(self.sym.clone());
        } else {
            strings.push(self.sym.clone());
        }
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-symbol.as-string-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-symbol.as-string-fn]
    fn as_string(&self, ctx: &mut PmatchEvalContext<B>) -> String {
        self.sym.to_string()
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-string.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-string.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-string.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchString<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        let key = self.cache_key();
        if ctx.node_cache_get(key).is_some() {
            self.report_cache(ctx, String::new());
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just checked"));
        }
        let my_timer = self.start_timing(ctx);
        let mut tmp: HfstTransducer<B> = if self.multichar {
            let tok = HfstTokenizer::new();
            HfstTransducer::new_tokenized(&self.string, &tok)?
        } else {
            HfstTransducer::new_symbol(&self.string)?
        };
        tmp.set_final_weights(self.weight as f32, true)?;
        if ctx.node_cache_get(key).is_none() && self.should_use_cache(ctx) {
            ctx.node_cache_put(key, tmp);
            ctx.node_cache_get_mut(key)
                .expect("cache populated above")
                .minimize()?;
            self.report_time(ctx, my_timer, String::new());
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just inserted"));
        }
        self.report_time(ctx, my_timer, String::new());
        Ok(tmp)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-string.collect-strings-into-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-string.collect-strings-into-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string.collect-strings-into-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-string.collect-strings-into-fn]
    fn collect_strings_into(&self, ctx: &mut PmatchEvalContext<B>, strings: &mut StringVector) {
        strings.push(self.string.clone());
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-string.evaluate-as-arg-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-string.evaluate-as-arg-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string.evaluate-as-arg-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-string.evaluate-as-arg-fn]
    fn evaluate_as_arg(&self, ctx: &mut PmatchEvalContext<B>) -> ObjRef<B> {
        Rc::new(PmatchString {
            name: self.name.clone(),
            weight: self.weight,
            line_defined: self.line_defined,
            string: self.string.clone(),
            multichar: self.multichar,
            _marker: std::marker::PhantomData,
        })
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string.as-string-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-string.as-string-fn]
    fn as_string(&self, ctx: &mut PmatchEvalContext<B>) -> String {
        self.string.to_string()
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string.as-string-pair-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-string.as-string-pair-fn]
    fn as_string_pair(&self, ctx: &mut PmatchEvalContext<B>) -> StringPair {
        (self.string.clone(), self.string.clone())
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string.is-unweighted-disjunction-of-strings-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-string.is-unweighted-disjunction-of-strings-fn]
    fn is_unweighted_disjunction_of_strings(&self) -> bool {
        self.weight == 0.0 && (self.multichar || (self.string.len() < 2))
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-question-mark.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-question-mark.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-question-mark.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-question-mark.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchQuestionMark<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        let my_timer = self.start_timing(ctx);
        let mut retval: HfstTransducer<B> = HfstTransducer::new_symbol(internal_identity)?;
        retval.set_final_weights(self.weight as f32, true)?;
        self.report_time(ctx, my_timer, String::new());
        Ok(retval)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-question-mark.as-string-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-question-mark.as-string-fn]
    fn as_string(&self, ctx: &mut PmatchEvalContext<B>) -> String {
        internal_unknown.to_string()
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-question-mark.as-string-pair-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-question-mark.as-string-pair-fn]
    fn as_string_pair(&self, ctx: &mut PmatchEvalContext<B>) -> StringPair {
        (
            Symbol::new_static(internal_identity),
            Symbol::new_static(internal_identity),
        )
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-acceptor.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-acceptor.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-acceptor.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-acceptor.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchAcceptor<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        let my_timer = self.start_timing(ctx);
        let mut retval: HfstTransducer<B> = match self.set {
            PmatchPredefined::Alpha => {
                if ctx
                    .variables_entry_or_default("unicode-character-classes")
                    .as_str()
                    == "on"
                {
                    HfstTransducer::new_symbol("@UNICODE_ALPHA@")?
                } else {
                    ctx.with_utils(|u| HfstTransducer::new_copy(&u.latin1_alpha_acceptor))?
                }
            }
            PmatchPredefined::UppercaseAlpha => {
                if ctx
                    .variables_entry_or_default("unicode-character-classes")
                    .as_str()
                    == "on"
                {
                    HfstTransducer::new_symbol("@UNICODE_UPPERALPHA@")?
                } else {
                    ctx.with_utils(|u| HfstTransducer::new_copy(&u.latin1_uppercase_acceptor))?
                }
            }
            PmatchPredefined::LowercaseAlpha => {
                if ctx
                    .variables_entry_or_default("unicode-character-classes")
                    .as_str()
                    == "on"
                {
                    HfstTransducer::new_symbol("@UNICODE_LOWERALPHA@")?
                } else {
                    ctx.with_utils(|u| HfstTransducer::new_copy(&u.latin1_lowercase_acceptor))?
                }
            }
            PmatchPredefined::Numeral => {
                ctx.with_utils(|u| HfstTransducer::new_copy(&u.latin1_numeral_acceptor))?
            }
            PmatchPredefined::Punctuation => {
                ctx.with_utils(|u| HfstTransducer::new_copy(&u.latin1_punct_acceptor))?
            }
            PmatchPredefined::Whitespace => {
                if ctx
                    .variables_entry_or_default("unicode-character-classes")
                    .as_str()
                    == "on"
                {
                    HfstTransducer::new_symbol("@UNICODE_WHITESPACE@")?
                } else {
                    ctx.with_utils(|u| HfstTransducer::new_copy(&u.latin1_whitespace_acceptor))?
                }
            }
        };
        retval.set_final_weights(self.weight as f32, true)?;
        self.report_time(ctx, my_timer, String::new());
        Ok(retval)
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-empty.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-empty.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchEmpty<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        Ok(HfstTransducer::new())
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-epsilon-arc.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-epsilon-arc.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchEpsilonArc<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        HfstTransducer::new_symbol(internal_epsilon)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-epsilon-arc.as-string-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-epsilon-arc.as-string-fn]
    fn as_string(&self, ctx: &mut PmatchEvalContext<B>) -> String {
        internal_epsilon.to_string()
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-transducer-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-transducer-container.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchTransducerContainer<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        let mut retval = HfstTransducer::new_copy(&self.t)?;
        retval.set_final_weights(self.weight as f32, true)?;
        if !self.name.is_empty() {
            retval.set_name(&self.name);
        }
        Ok(retval)
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-function.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-function.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-function.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-function.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchFunction<B> {
    pmatch_object_base_accessors!();

    fn evaluate_args(
        &self,
        ctx: &mut PmatchEvalContext<B>,
        funargs: Vec<ObjRef<B>>,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let my_timer: clock_t = if ctx.verbose {
            let my_timer = clock();
            ctx.named_object_evaluation_stack_depth += 1;
            write_compilation_stack_indentation_to_err(ctx);
            debug!("Evaluating call to {}...", self.name);
            my_timer
        } else {
            0
        };
        if funargs.len() != self.args.len() {
            let errstring = format!(
                "Function {} expected {} args, got {}\n",
                self.name,
                self.args.len(),
                funargs.len()
            );
            panic!("{}", errstring);
        }
        let mut local_env: BTreeMap<String, ObjRef<B>> = BTreeMap::new();
        if ctx.call_stack_len() != 0 {
            local_env = ctx.call_stack_last_clone();
        }
        for (i, arg) in self.args.iter().enumerate() {
            // `local_env`/`call_stack` is a String-keyed frame map (out of this
            // node's scope); the formal arg name bridges Symbol -> String here.
            local_env.insert(arg.to_string(), funargs[i].clone());
        }
        ctx.call_stack_push(local_env);
        if !self.name.is_empty() {
            ctx.eval_stack_push(self.name.clone());
        }
        let mut retval: HfstTransducer<B> = self.root.evaluate(ctx)?;
        if !self.name.is_empty() {
            ctx.eval_stack_pop();
        }
        retval.set_final_weights(self.weight as f32, true)?;
        ctx.call_stack_pop();
        if ctx.verbose {
            let duration = (clock() - my_timer) as f64 / CLOCKS_PER_SEC as f64;
            write_compilation_stack_indentation_to_err(ctx);
            debug!("Call to {} evaluated in {} seconds", self.name, duration);
            ctx.named_object_evaluation_stack_depth -= 1;
        }
        Ok(retval)
    }

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        let funargs: Vec<ObjRef<B>> = Vec::new();
        self.evaluate_args(ctx, funargs)
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-funcall.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-funcall.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-funcall.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-funcall.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchFuncall<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        if !self.name.is_empty() {
            ctx.eval_stack_push(self.name.clone());
        }
        let evaluated_args: Vec<ObjRef<B>> =
            self.args.iter().map(|it| it.evaluate_as_arg(ctx)).collect();
        let retval = self.fun.evaluate_args(ctx, evaluated_args.clone());
        if !self.name.is_empty() {
            ctx.eval_stack_pop();
        }
        retval
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-builtin-function.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-builtin-function.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-builtin-function.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-builtin-function.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchBuiltinFunction<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        if !self.name.is_empty() {
            ctx.eval_stack_push(self.name.clone());
        }
        let my_timer = self.start_timing(ctx);
        let mut retval: HfstTransducer<B> = HfstTransducer::new();
        if self.ty == PmatchBuiltin::Interpolate {
            if self.args.len() < 3 {
                let errstring = format!(
                    "Builtin function Interpolate called with {} arguments, but it expects at least 3.\n",
                    self.args.len()
                );
                panic!("{}", errstring);
            }
            // arguments are in reverse order after parsing
            let n = self.args.len();
            retval = self.args[n - 2].evaluate(ctx)?;
            let interpolator: HfstTransducer<B> = self.args[n - 1].evaluate(ctx)?;
            for i in (0..(n - 2)).rev() {
                let tmp: HfstTransducer<B> = self.args[i].evaluate(ctx)?;
                retval.concatenate(&interpolator, true)?;
                retval.concatenate(&tmp, true)?;
            }
        }
        retval.set_final_weights(self.weight as f32, true)?;
        self.report_time(ctx, my_timer, String::new());
        if !self.name.is_empty() {
            ctx.eval_stack_pop();
        }
        Ok(retval)
    }
}

// ===== body: word-vectors-like =====
// Get the n best candidates in the original space using an insertion sort
// [spec:hfst:def:pmatch-utils.hfst.pmatch.get-top-n-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.get-top-n-fn]
pub fn get_top_n(
    n: usize,
    vecs: &[WordVector],
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
pub fn get_top_n_transformed<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    n: usize,
    vecs: &[WordVector],
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
        transformed_vec_scaler *= ctx.vector_similarity_projection_factor;
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
    for x in v.iter() {
        ret += x * x;
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
pub fn compile_like_arc<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    word1: String,
    word2: String,
    nwords: u32,
    is_negative: bool,
) -> crate::error::Result<ObjRef<B>> {
    {
        let mut this_word1: WordVector = WordVector::default();
        let mut this_word2: WordVector = WordVector::default();
        {
            let wv_snapshot = ctx.word_vectors_snapshot();
            let mut it_iter = wv_snapshot.iter();
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
            let word1_o = Rc::new(PmatchString {
                name: String::new(),
                weight: 0.0,
                line_defined: 0,
                string: Symbol::from(word1),
                multichar: true,
                _marker: std::marker::PhantomData,
            });
            let word2_o = Rc::new(PmatchString {
                name: String::new(),
                weight: 0.0,
                line_defined: 0,
                string: Symbol::from(word2),
                multichar: true,
                _marker: std::marker::PhantomData,
            });
            pmatchwarning("no matches for arguments to Like() operation");
            let binop = Rc::new(PmatchBinaryOperation {
                name: String::new(),
                weight: 0.0,
                line_defined: 0,
                op: PmatchBinaryOp::Disjunct,
                left: as_obj(word1_o),
                right: as_obj(word2_o),
            });
            return Ok(as_obj(binop));
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
            let top_n: Vec<(WordVector, WordVecFloat)> = get_top_n(
                nwords as usize,
                &ctx.word_vectors_snapshot(),
                &mut this_word,
            );
            let tok: HfstTokenizer = HfstTokenizer::new();
            let mut retval: HfstTransducer<B> = HfstTransducer::new();
            if ctx.verbose {
                debug!("Inserting into Like({}):", this_word.word);
            }

            for entry in top_n.iter() {
                if ctx.verbose {
                    debug!("  {}", entry.0.word);
                }
                let mut tmp: HfstTransducer<B> =
                    HfstTransducer::new_tokenized(&entry.0.word, &tok)?;
                if ctx.include_cosine_distances {
                    tmp.set_final_weights(entry.1, false)?;
                }
                retval.disjunct(&tmp, true)?;
            }
            let container = Rc::new(PmatchTransducerContainer {
                name: String::new(),
                weight: 0.0,
                line_defined: 0,
                t: retval,
            });
            return Ok(as_obj(container));
        }

        if ctx.variables_entry_or_default("vector-similarity-projection-factor") != "1.0" {
            ctx.vector_similarity_projection_factor =
                crate::string_manipulation::parse_float_prefix_str(
                    &ctx.variables_index("vector-similarity-projection-factor"),
                ) as WordVecFloat;
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

        let comparison_point: Vec<WordVecFloat> = if is_negative {
            if ctx.verbose {
                debug!(
                    "Inserting into Unlike({}, {}):",
                    this_word1.word, this_word2.word
                );
            }
            let mut comparison_scaler: WordVecFloat = (hyperplane_translation_term
                - dot_product(this_word1.vector.clone(), B_minus_A.clone()))
                / square_sum(B_minus_A.clone());
            comparison_scaler *= ctx.vector_similarity_projection_factor;
            pointwise_minus(
                this_word1.vector.clone(),
                pointwise_multiplication(comparison_scaler, B_minus_A.clone()),
            )
        } else {
            if ctx.verbose {
                debug!(
                    "Inserting into Like({}, {}):",
                    this_word1.word, this_word2.word
                );
            }
            pointwise_plus(
                this_word2.vector.clone(),
                pointwise_multiplication(0.5 as WordVecFloat, B_minus_A.clone()),
            )
        };

        let top_n: Vec<(WordVector, WordVecFloat)> = get_top_n_transformed(
            ctx,
            nwords as usize,
            &ctx.word_vectors_snapshot(),
            B_minus_A.clone(),
            comparison_point,
            hyperplane_translation_term,
            is_negative,
        );
        let tok: HfstTokenizer = HfstTokenizer::new();
        let mut retval: HfstTransducer<B> = HfstTransducer::new();
        let mut i: usize = 0;
        while i < top_n.len() && i <= nwords as usize {
            if ctx.verbose {
                debug!("  {}", top_n[i].0.word);
            }
            let mut tmp: HfstTransducer<B> = HfstTransducer::new_tokenized(&top_n[i].0.word, &tok)?;
            if ctx.include_cosine_distances {
                tmp.set_final_weights(top_n[i].1, false)?;
            }
            retval.disjunct(&tmp, true)?;
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
        let container = Rc::new(PmatchTransducerContainer {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            t: retval,
        });
        Ok(as_obj(container))
    }
}
// Single-word Like()
// [spec:hfst:def:pmatch-utils.hfst.pmatch.compile-like-arc-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.compile-like-arc-fn]
pub fn compile_like_arc_word<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    word: String,
    nwords: u32,
) -> crate::error::Result<ObjRef<B>> {
    {
        let mut this_word: WordVector = WordVector::default();
        for it in ctx.word_vectors_snapshot().iter() {
            if word == it.word {
                this_word = it.clone();
                break;
            }
        }
        if this_word.word.is_empty() {
            // got no matches
            let word_o = Rc::new(PmatchString {
                name: String::new(),
                weight: 0.0,
                line_defined: 0,
                string: Symbol::from(word),
                multichar: true,
                _marker: std::marker::PhantomData,
            });
            pmatchwarning("no matches for argument to Like() operation");
            return Ok(as_obj(word_o));
        }

        let top_n: Vec<(WordVector, WordVecFloat)> = get_top_n(
            nwords as usize,
            &ctx.word_vectors_snapshot(),
            &mut this_word,
        );

        let tok: HfstTokenizer = HfstTokenizer::new();
        let mut retval: HfstTransducer<B> = HfstTransducer::new();
        if ctx.verbose {
            debug!("Inserting into Like({}):", word);
        }
        for entry in top_n.iter() {
            if ctx.verbose {
                debug!("  {}", entry.0.word);
            }
            let mut tmp: HfstTransducer<B> = HfstTransducer::new_tokenized(&entry.0.word, &tok)?;
            if ctx.include_cosine_distances {
                tmp.set_final_weights(entry.1, false)?;
            }
            retval.disjunct(&tmp, true)?;
        }
        let container = Rc::new(PmatchTransducerContainer {
            name: String::new(),
            weight: 0.0,
            line_defined: 0,
            t: retval,
        });
        Ok(as_obj(container))
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.read-vec-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.read-vec-fn]
pub fn read_vec<B: AlgebraBackend + 'static>(ctx: &mut PmatchEvalContext<B>, filename: String) {
    use std::io::Read;
    let mut binary_format = false;
    if filename.len() >= 4 && filename.rfind(".bin") == Some(filename.len() - 4) {
        binary_format = true;
    }
    if ctx.word_vectors_len() != 0 {
        ctx.word_vectors_clear();
        warn!(
            "pmatch: vector model file {} overrides earlier one",
            filename
        );
    }
    let mut separator: u8 = b' ';
    let infile = match std::fs::File::open(&filename) {
        Ok(f) => f,
        Err(_) => {
            error!(
                "pmatch: could not open vector file {} for reading",
                filename
            );
            return;
        }
    };
    let mut infile = std::io::BufReader::new(infile);
    let mut all_bytes: Vec<u8> = Vec::new();
    if infile.read_to_end(&mut all_bytes).is_err() {
        error!(
            "pmatch: could not open vector file {} for reading",
            filename
        );
        return;
    }
    // Cursor over the raw file bytes, mirroring std::ifstream semantics.
    // Read the header line (up to '\n').
    let header_end = all_bytes
        .iter()
        .position(|&b| b == b'\n')
        .unwrap_or(all_bytes.len());
    let header_line: String = String::from_utf8_lossy(&all_bytes[..header_end]).into_owned();
    let mut cursor: usize = (header_end + 1).min(all_bytes.len());
    let lexicon_size: usize;
    let dimension: usize;
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
    ctx.word_vectors_reserve(lexicon_size + 1);
    let mut words_read: usize = 0;
    if binary_format {
        let vector_data_size: usize = std::mem::size_of::<f32>() * dimension;
        while cursor < all_bytes.len() && words_read <= lexicon_size {
            // The actual number of vectors is 1 more than lexicon_size
            // due to <s>
            // std::getline(infile, line, separator)
            let word_end = all_bytes[cursor..]
                .iter()
                .position(|&b| b == separator)
                .map_or(all_bytes.len(), |i| cursor + i);
            let line = String::from_utf8_lossy(&all_bytes[cursor..word_end]).into_owned();
            cursor = (word_end + 1).min(all_bytes.len());
            // infile.read(&vector_data[0], vector_data_size)
            let read_end = std::cmp::min(cursor + vector_data_size, all_bytes.len());
            let vector_data: Vec<u8> = all_bytes[cursor..read_end].to_vec();
            cursor = read_end;
            // infile.ignore(1)
            if cursor < all_bytes.len() {
                cursor += 1;
            }
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
            let wv = WordVector {
                word: line,
                norm: norm(comps.clone()),
                vector: comps,
            };
            ctx.word_vectors_push(wv);
            words_read += 1;
        }
    } else {
        let text = &all_bytes[cursor..];
        // 'std::getline(infile, line)' consumes a trailing final newline
        // without yielding an extra empty line after it.
        let text = text.strip_suffix(b"\n").unwrap_or(text);
        for raw_line in text.split(|&b| b == b'\n') {
            if words_read > lexicon_size {
                break;
            }
            let line = String::from_utf8_lossy(raw_line).into_owned();
            if line.is_empty() {
                continue;
            }
            words_read += 1;
            let line_bytes = line.as_bytes();
            let mut pos = match line_bytes.iter().position(|&b| b == separator) {
                Some(p) => p,
                None => {
                    separator = b'\t';
                    match line_bytes.iter().position(|&b| b == separator) {
                        Some(p) => p,
                        None => {
                            warn!(
                                "pmatch: vector file {} doesn't appear to be tab- or \
space-separated\n  (reading line {})",
                                filename,
                                words_read + 1
                            );
                            break;
                        }
                    }
                }
            };
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
                        let v =
                            crate::string_manipulation::parse_float_prefix_str(sub) as WordVecFloat;
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
                let v = crate::string_manipulation::parse_float_prefix_str(sub) as WordVecFloat;
                components.push(v);
            }
            if ctx.word_vectors_len() != 0
                && ctx.word_vectors_first_vector_len() != components.len()
            {
                warn!(
                    "pmatch: vector file {} appears malformed\n  (reading line {})",
                    filename,
                    words_read + 1
                );
                continue;
            }
            let wv = WordVector {
                word,
                vector: components.clone(),
                norm: norm(components),
            };
            ctx.word_vectors_push(wv);
        }
    }
    if ctx.verbose {
        if ctx.word_vectors_len() == 0 {
            debug!("Tried to read word vector file, empty result");
        }
        debug!(
            "Read {} vectors of dimensionality {}",
            ctx.word_vectors_len(),
            ctx.word_vectors_first_vector_len()
        );
    }
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
// [spec:hfst:def:pmatch-utils.pmatchwarning-fn]
// [spec:hfst:sem:pmatch-utils.pmatchwarning-fn]
pub fn pmatchwarning(msg: &str) {
    warn!("pmatch: {}", msg);
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.register-lst-line-numbers-from-transducer-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.register-lst-line-numbers-from-transducer-fn]
pub fn register_lst_line_numbers_from_transducer<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    t: &HfstTransducer<B>,
    line: i32,
) -> crate::error::Result<()> {
    if line <= 0 {
        return Ok(());
    }
    let ss: StringSet = t.get_alphabet()?;
    for it in ss.iter() {
        if it.find("@L.") == Some(0) {
            // Keep first occurrence if seen before.
            if !ctx.lst_line_map_contains(it) {
                ctx.lst_line_map_insert(it.to_string(), line);
            }
        }
    }
    Ok(())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.expand-includes-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.expand-includes-fn]
pub fn expand_includes<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    script: &str,
) -> String {
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
                let filepath = path_from_filename(
                    ctx,
                    &script[filename_start_pos..filename_start_pos + filename_len],
                );
                match fs::read(&filepath) {
                    Ok(contents) => {
                        for b in contents {
                            retval.push(b as char);
                        }
                    }
                    Err(_) => {
                        let errstring = format!("could not open file {} for @include\n", filepath);
                        ctx.pmatcherror(&errstring);
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
// Render a parser failure as human-readable text. The bison port used to
// swallow 'nfst_pmatch::parse' errors and let compilation fall through to the
// downstream "Empty ruleset, nothing to write" message, which hides the real
// cause — e.g. a reserved predefined acceptor name (`Alpha`, `Whitespace`, ...)
// used as a `Define` target ("expected definition name after `Define`").
fn format_pmatch_parse_error(e: &nfst_pmatch::ParseError) -> String {
    if e.diagnostics.is_empty() {
        return "pmatch: syntax error".to_string();
    }
    e.diagnostics
        .iter()
        .map(|d| format!("pmatch: syntax error: {}", d.message))
        .collect::<Vec<_>>()
        .join("\n")
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch.compile-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.compile-fn]
pub fn compile<B: AlgebraBackend + FromAnyTransducer + 'static>(
    pmatch: &str,
    defs: &HashMap<String, HfstTransducer<B>>,
    be_verbose: bool,
    do_flatten: bool,
    do_include_cosine_distances: bool,
    includedir: String,
) -> crate::error::Result<HashMap<String, HfstTransducer<B>>> {
    // lock here?
    let mut ctx_owned = PmatchEvalContext::new();
    let ctx = &mut ctx_owned;
    ctx.init_globals();
    let expanded_script = expand_includes(ctx, pmatch);
    ctx.data = expanded_script.clone();
    ctx.len = ctx.data.clone().len();
    ctx.verbose = be_verbose;
    ctx.flatten = do_flatten;
    ctx.include_cosine_distances = do_include_cosine_distances;
    ctx.includedir = includedir;
    ctx.vector_similarity_projection_factor = 1.0;
    for (key, value) in defs.iter() {
        ctx.definitions_insert(
            key.clone(),
            as_obj(PmatchTransducerContainer::new(HfstTransducer::new_copy(
                value,
            )?)),
        );
    }
    if ctx.verbose {
        ctx.timer = clock();
        debug!("");
    }

    // === SEAM: replaces the bison 'pmatchparse()' call ====================
    // The build-driver group walks the nfst-pmatch parse tree, populating the
    // 'hfst::pmatch' globals exactly as the bison actions would have.
    match nfst_pmatch::parse(&expanded_script) {
        Ok(parsed) => {
            for statement in &parsed.value.statements {
                if let Err(e) = build_statement(ctx, statement) {
                    error!("{}", e);
                    ctx.pmatchnerrs += 1;
                }
            }
        }
        Err(e) => {
            error!("{}", format_pmatch_parse_error(&e));
            ctx.pmatchnerrs += 1;
        }
    }
    // === END SEAM =========================================================

    let mut retval: HashMap<String, HfstTransducer<B>> = HashMap::new();
    for it in ctx.unsatisfied_insertions_snapshot().into_iter() {
        if !ctx.definitions_contains(it.as_str()) {
            error!("Inserted transducer {} was never defined!", it);
            return Ok(retval);
        }
    }
    if ctx.verbose {
        let defs_keys: Vec<String> = ctx.definitions_keys();
        for first in defs_keys.iter() {
            if !ctx.used_definitions_contains(first) && first != "TOP" {
                debug!("Warning: {} defined but never used", first);
            }
        }
    }

    if ctx.pmatchnerrs != 0 {
        ctx.data = String::new();
        ctx.len = 0;
        return Ok(retval);
    }
    // Our helper for harmonizing all the networks' alphabets with
    // each other
    if ctx.verbose {
        debug!("Compiling and harmonizing...");
        ctx.timer = clock();
    }

    let mut uncount: u32 = 0;
    if ctx.inserted_names_len() > 0
        || ctx.def_insed_expressions_len() > 0
        || ctx.uncomposed_len() > 0
    {
        let mut dummy = HfstTransducer::new();
        // We keep TOP and any inserted transducers
        let defs_keys: Vec<String> = ctx.definitions_keys();
        for first in defs_keys.iter() {
            if first == "TOP"
                || ctx.inserted_names_contains(first)
                || ctx.def_insed_expressions_contains(first)
                || ctx.uncomposed_contains(first)
            {
                if ctx.verbose {
                    let second = ctx
                        .definitions_get(first)
                        .expect("first is a definitions key");
                    debug!("definition...{}={}", first, second.get_name().to_string());
                }
                let mut tmp: HfstTransducer<B> = if ctx.def_insed_expressions_contains(first) {
                    ctx.def_insed_expressions_get(first)
                        .expect("checked with contains just above")
                        .evaluate(ctx)?
                } else {
                    ctx.definitions_get(first)
                        .expect("first is a definitions key")
                        .evaluate(ctx)?
                };
                tmp.minimize()?;
                dummy.harmonize(&mut tmp, true)?;
                // This is what it will be called in the archive
                // XXX: seems to use the index not the name...)
                if ctx.uncomposed_contains(first) {
                    if ctx.verbose {
                        debug!("Uncompose");
                    }
                    if uncount == 0 {
                        tmp.set_name(&("UNCOMPOSE LEFT ".to_string() + first));
                        retval.insert("UNCOMPOSE LEFT ".to_string() + first, tmp);
                        uncount += 1;
                    } else if uncount == 1 {
                        tmp.set_name(&("UNCOMPOSE RIGHT ".to_string() + first));
                        retval.insert("UNCOMPOSE RIGHT ".to_string() + first, tmp);
                        uncount += 1;
                    } else {
                        warn!("Uncompose only works once so far...");
                        uncount += 1;
                    }
                } else {
                    tmp.set_name(first);
                    retval.insert(first.clone(), tmp);
                }
            }
        }

        // Now that dummy is harmonized with everything, we harmonize
        // everything with dummy and minimize the results
        for second in retval.values_mut() {
            second.harmonize(&mut dummy, true)?;
            second.minimize()?;
        }
    } else {
        if ctx.definitions_len() == 0 {
            warn!("pmatch compilation had an empty result");
            retval.insert("TOP".to_string(), HfstTransducer::new());
        } else if !ctx.definitions_contains("TOP") {
            let first_key = ctx
                .definitions_keys()
                .into_iter()
                .next()
                .expect("definitions non-empty in this branch");
            warn!(
                "Pmatch compilation: regex or TOP was undefined, using {} as root",
                first_key
            );
            let mut tmp = ctx
                .definitions_get(&first_key)
                .expect("first_key is a definitions key")
                .evaluate(ctx)?;
            tmp.minimize()?;
            tmp.set_name("TOP");
            retval.insert("TOP".to_string(), tmp);
        } else {
            let mut tmp = ctx
                .definitions_get("TOP")
                .expect("TOP present in this branch")
                .evaluate(ctx)?;
            tmp.minimize()?;
            tmp.set_name("TOP");
            retval.insert("TOP".to_string(), tmp);
        }
    }

    if ctx.verbose {
        let duration = (clock() - ctx.timer) as f64 / CLOCKS_PER_SEC as f64;
        ctx.timer = clock();
        debug!("Everything compiled and harmonized in {} seconds", duration);
    }

    let mut allowed_initial_symbols: StringSet = StringSet::new();
    let mut disallowed_initial_symbols: StringSet = StringSet::new();
    ctx.definitions_get("TOP")
        .expect("TOP defined by this point")
        .collect_initial_symbols_into(
            &mut allowed_initial_symbols,
            &mut disallowed_initial_symbols,
        )?;
    let mut initial_symbols_list = String::new();
    let mut disallowed_initial_symbols_list = String::new();
    // Use this to bail out if there's something suspicious in the final lists
    let mut initial_symbols_ok = true;
    for it in allowed_initial_symbols.iter() {
        if is_special(it) {
            if ctx.verbose {
                debug!(
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
            if ctx.verbose {
                debug!(
                    "Not setting initial symbol list due to special symbol {}",
                    it
                );
            }
            initial_symbols_ok = false;
        }
        disallowed_initial_symbols_list.push_str(it);
    }
    if allowed_initial_symbols.len() > 200 {
        if ctx.verbose {
            debug!(
                "Not setting initial symbol list due to excess length: {}",
                allowed_initial_symbols.len()
            );
        }
        initial_symbols_ok = false;
    }
    if disallowed_initial_symbols.len() > 200 {
        if ctx.verbose {
            debug!(
                "Not setting initial symbol list due to excess length: {}",
                disallowed_initial_symbols.len()
            );
        }
        initial_symbols_ok = false;
    }
    if initial_symbols_ok && !initial_symbols_list.is_empty() {
        ctx.variables_insert("initial-symbols".to_string(), initial_symbols_list);
    }
    if initial_symbols_ok && !disallowed_initial_symbols_list.is_empty() {
        ctx.variables_insert(
            "disallowed-initial-symbols".to_string(),
            disallowed_initial_symbols_list,
        );
    }
    if ctx.variables_get("need-separators").as_deref() == Some("on") {
        let whitespace_acc =
            ctx.with_utils(|u| HfstTransducer::new_copy(&u.latin1_whitespace_acceptor))?;
        let punct_acc = ctx.with_utils(|u| HfstTransducer::new_copy(&u.latin1_punct_acceptor))?;
        let mut not_whitespace = HfstTransducer::new_symbol(internal_identity)?;
        not_whitespace.subtract(&whitespace_acc, true)?;
        let mut anything = HfstTransducer::new_symbol(internal_identity)?;
        anything.repeat_star()?;
        let mut begins_and_ends_with_non_whitespace = HfstTransducer::new_copy(&not_whitespace)?;
        begins_and_ends_with_non_whitespace.concatenate(&anything, true)?;
        begins_and_ends_with_non_whitespace.concatenate(&not_whitespace, true)?;
        begins_and_ends_with_non_whitespace
            .compose(retval.get("TOP").expect("TOP defined above"), true)?;
        let mut is_single_non_whitespace = HfstTransducer::new_copy(&not_whitespace)?;
        is_single_non_whitespace.compose(retval.get("TOP").expect("TOP defined above"), true)?;
        let empty = HfstTransducer::new();
        if !begins_and_ends_with_non_whitespace.compare(&empty, true)?
            || !is_single_non_whitespace.compare(&empty, true)?
        {
            let mut whitespace_punct_context = HfstTransducer::new_copy(&whitespace_acc)?;
            whitespace_punct_context.disjunct(&punct_acc, true)?;
            whitespace_punct_context.disjunct(&HfstTransducer::new_symbol("@BOUNDARY@")?, true)?;
            let mut top_with_boundaries: HfstTransducer<B> =
                HfstTransducer::new_symbol_pair(internal_epsilon, LC_ENTRY_SYMBOL)?;
            top_with_boundaries.concatenate(&whitespace_punct_context, true)?;
            top_with_boundaries.concatenate(
                &HfstTransducer::new_symbol_pair(internal_epsilon, LC_EXIT_SYMBOL)?,
                true,
            )?;
            let mut rc = HfstTransducer::new_symbol_pair(internal_epsilon, RC_ENTRY_SYMBOL)?;
            rc.concatenate(&whitespace_punct_context, true)?;
            rc.concatenate(
                &HfstTransducer::new_symbol_pair(internal_epsilon, RC_EXIT_SYMBOL)?,
                true,
            )?;
            top_with_boundaries.concatenate(retval.get("TOP").expect("TOP defined above"), true)?;
            top_with_boundaries.concatenate(&rc, true)?;
            retval.insert(
                "TOP".to_string(),
                add_pmatch_delimiters(&top_with_boundaries)?,
            );
            retval
                .get_mut("TOP")
                .expect("TOP defined above")
                .minimize()?;
            if ctx.verbose {
                let duration = (clock() - ctx.timer) as f64 / CLOCKS_PER_SEC as f64;
                ctx.timer = clock();
                debug!("Added automatic context separators in {} seconds", duration);
            }
        }
    }
    let vars: Vec<(String, String)> = ctx.variables_snapshot();
    let top = retval
        .get_mut("TOP")
        .expect("TOP present in result by this point");
    for (key, value) in vars.iter() {
        top.set_property(key, value);
    }
    ctx.data = String::new();
    ctx.len = 0;
    Ok(retval)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.write-compilation-stack-indentation-to-err-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.write-compilation-stack-indentation-to-err-fn]
pub fn write_compilation_stack_indentation_to_err<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
) {
    // Visually indicate nested definitions
    let mut indentation = String::new();
    let mut i = 1;
    while i < ctx.named_object_evaluation_stack_depth {
        indentation.push('|');
        i += 1;
    }
    if ctx.named_object_evaluation_stack_depth > 1 {
        indentation.push(' ');
    }
    if !indentation.is_empty() {
        debug!("{}", indentation);
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.read-text-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.read-text-fn]
pub fn read_text<B: AlgebraBackend>(
    filename: String,
    spaced_text: bool,
) -> crate::error::Result<HfstTransducer<B>> {
    let tok = HfstTokenizer::new();
    let mut retval: HfstTransducer<B> = HfstTransducer::new();
    match fs::read_to_string(&filename) {
        Err(_) => {
            error!("Pmatch: could not open text file {} for reading", filename);
        }
        Ok(contents) => {
            for line in contents.lines() {
                let line = line.to_string();
                if !line.is_empty() {
                    if spaced_text {
                        let _spv = HfstTokenizer::tokenize_space_separated(&line);
                    } else {
                        let spv = tok.tokenize(&line, false); // XXX
                        retval.disjunct_spv(&spv)?;
                    }
                }
            }
        }
    }
    Ok(retval)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.read-spaced-text-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.read-spaced-text-fn]
pub fn read_spaced_text<B: AlgebraBackend>(
    filename: String,
) -> crate::error::Result<HfstTransducer<B>> {
    read_text(filename, true)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.path-from-filename-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.path-from-filename-fn]
pub fn path_from_filename<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    filename: &str,
) -> String {
    let mut retval = filename.to_string();
    if !ctx.includedir.is_empty() && !retval.is_empty() {
        // includedir won't be > 0 under Windows until this mechanism is ported
        if retval.as_bytes()[0] != b'/' {
            // not an absolute dir
            retval.insert_str(0, &ctx.includedir.clone());
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
        let codepoint: u32;
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

fn as_obj<B: AlgebraBackend + 'static, T: PmatchObject<B> + 'static>(p: Rc<T>) -> ObjRef<B> {
    p
}
fn pmb_symbol<B: AlgebraBackend + 'static>(sym: Symbol) -> ObjRef<B> {
    Rc::new(PmatchSymbol {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        sym,
        _marker: std::marker::PhantomData,
    })
}
fn pmb_string<B: AlgebraBackend + 'static>(string: Symbol, multichar: bool) -> ObjRef<B> {
    Rc::new(PmatchString {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        string,
        multichar,
        _marker: std::marker::PhantomData,
    })
}
fn pmb_question_mark<B: AlgebraBackend + 'static>() -> ObjRef<B> {
    Rc::new(PmatchQuestionMark {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        _marker: std::marker::PhantomData,
    })
}
fn pmb_empty<B: AlgebraBackend + 'static>() -> ObjRef<B> {
    Rc::new(PmatchEmpty {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        _marker: std::marker::PhantomData,
    })
}
fn pmb_epsilon_arc<B: AlgebraBackend + 'static>() -> ObjRef<B> {
    Rc::new(PmatchEpsilonArc {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        _marker: std::marker::PhantomData,
    })
}
fn pmb_acceptor<B: AlgebraBackend + 'static>(set: PmatchPredefined) -> ObjRef<B> {
    Rc::new(PmatchAcceptor {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        set,
        _marker: std::marker::PhantomData,
    })
}
fn pmb_unary<B: AlgebraBackend + 'static>(op: PmatchUnaryOp, root: ObjRef<B>) -> ObjRef<B> {
    Rc::new(PmatchUnaryOperation {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        op,
        root,
    })
}
fn pmb_binary<B: AlgebraBackend + 'static>(
    op: PmatchBinaryOp,
    left: ObjRef<B>,
    right: ObjRef<B>,
) -> ObjRef<B> {
    Rc::new(PmatchBinaryOperation {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        op,
        left,
        right,
    })
}
fn pmb_ternary<B: AlgebraBackend + 'static>(
    op: PmatchTernaryOp,
    left: ObjRef<B>,
    middle: ObjRef<B>,
    right: ObjRef<B>,
) -> ObjRef<B> {
    Rc::new(PmatchTernaryOperation {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        op,
        left,
        middle,
        right,
    })
}
fn pmb_numeric<B: AlgebraBackend + 'static>(
    op: PmatchNumericOp,
    root: ObjRef<B>,
    values: Vec<i32>,
) -> ObjRef<B> {
    Rc::new(PmatchNumericOperation {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        op,
        root,
        values,
    })
}
fn pmb_object_pair<B: AlgebraBackend + 'static>(left: ObjRef<B>, right: ObjRef<B>) -> PairRef<B> {
    Rc::new(PmatchObjectPair { left, right })
}
fn pmb_markup_container<B: AlgebraBackend + 'static>(
    left_of_arrow: ObjRef<B>,
    left: ObjRef<B>,
    right: ObjRef<B>,
) -> PairRef<B> {
    Rc::new(PmatchMarkupContainer {
        left,
        right,
        left_of_arrow,
    })
}
fn pmb_tc<B: AlgebraBackend + 'static>(t: HfstTransducer<B>) -> Rc<PmatchTransducerContainer<B>> {
    Rc::new(PmatchTransducerContainer {
        name: String::new(),
        weight: 0.0,
        line_defined: 0,
        t,
    })
}
// ===========================================================================
// Small build helpers
// ===========================================================================

fn ins_transition(name: &str) -> String {
    get_Ins_transition(name)
}
fn path_of<B: AlgebraBackend + 'static>(ctx: &mut PmatchEvalContext<B>, path: &str) -> String {
    path_from_filename(ctx, path)
}
// STRINGLIKE: QUOTED_LITERAL -> PmatchString, CURLY_LITERAL -> PmatchString
// (multichar), SYMBOL -> PmatchSymbol (no used_definitions / empty-check).
fn build_stringlike<B: AlgebraBackend + FromAnyTransducer + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    e: &nfst_pmatch::SpannedExpr,
) -> crate::error::Result<ObjRef<B>> {
    use nfst_pmatch::PmatchExpr as PE;
    Ok(match &e.value {
        PE::QuotedLiteral(s) => pmb_string(s.clone(), false),
        PE::CurlyLiteral(s) => pmb_string(s.clone(), true),
        PE::Symbol(s) => pmb_symbol(s.clone()),
        PE::Literal(_)
        | PE::Epsilon
        | PE::Any
        | PE::BoundaryMarker
        | PE::Acceptor(_)
        | PE::CharacterRange { .. }
        | PE::Binary(..)
        | PE::Unary(..)
        | PE::Group(_)
        | PE::Optional(_)
        | PE::BracketedDotted(_)
        | PE::Pair { .. }
        | PE::Weighted { .. }
        | PE::RepeatN(..)
        | PE::RepeatNPlus(..)
        | PE::RepeatNMinus(..)
        | PE::RepeatNToK(..)
        | PE::Replace { .. }
        | PE::Restriction { .. }
        | PE::Ins(_)
        | PE::EndTag(_)
        | PE::Capture(_)
        | PE::Tag { .. }
        | PE::With { .. }
        | PE::Counter(_)
        | PE::CaseOp { .. }
        | PE::DefineWrapper(_)
        | PE::Explode(_)
        | PE::Implode(_)
        | PE::Like { .. }
        | PE::Lst(_)
        | PE::Exc(_)
        | PE::Sigma(_)
        | PE::Interpolate(_)
        | PE::Substitute(..)
        | PE::Uncompose(..)
        | PE::Lc(_)
        | PE::Rc(_)
        | PE::Nlc(_)
        | PE::Nrc(_)
        | PE::OrContext(_)
        | PE::AndContext(_)
        | PE::Call { .. }
        | PE::ReadFile { .. }
        | PE::ReadLexc(_)
        | PE::ReadVec(_) => build_object(ctx, e)?,
    })
}
// CONCATENATED_STRING_LIST: right-folded Concatenate of STRINGLIKEs.
fn build_concatenated_string_list<B: AlgebraBackend + FromAnyTransducer + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    items: &[nfst_pmatch::SpannedExpr],
) -> crate::error::Result<ObjRef<B>> {
    let mut iter = items.iter().rev();
    let last = iter.next().expect("non-empty string list");
    let mut acc = build_stringlike(ctx, last)?;
    for it in iter {
        acc = pmb_binary(PmatchBinaryOp::Concatenate, build_stringlike(ctx, it)?, acc);
    }
    Ok(acc)
}
// MappingSide -> object: Expr -> build, Dotted([..]) -> PmatchEpsilonArc,
// Dotted([. E .]) -> build.
fn side_to_obj<B: AlgebraBackend + FromAnyTransducer + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    side: &nfst_pmatch::MappingSide,
) -> crate::error::Result<ObjRef<B>> {
    use nfst_pmatch::MappingSide as MS;
    Ok(match side {
        MS::Expr(e) => build_object(ctx, e)?,
        MS::Dotted(None) => pmb_epsilon_arc(),
        MS::Dotted(Some(e)) => build_object(ctx, e)?,
    })
}
// READ_FROM productions: eagerly load the named file into a container.
fn build_read_file<B: AlgebraBackend + FromAnyTransducer + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    kind: nfst_pmatch::ReadKind,
    path: &str,
) -> crate::error::Result<ObjRef<B>> {
    use nfst_pmatch::ReadKind as RK;
    let filepath = path_of(ctx, path);
    match kind {
        RK::Binary => {
            let mut instream = crate::hfst_input_stream::HfstInputStream::new_filename(&filepath)?;
            let read = instream.read()?.into_typed::<B>()?;
            instream.close();
            Ok(as_obj(pmb_tc(read)))
        }
        RK::Text => Ok(as_obj(pmb_tc(read_text(filepath, false)?))),
        RK::Spaced => Ok(as_obj(pmb_tc(read_spaced_text(filepath)?))),
        RK::Prolog => match std::fs::File::open(&filepath) {
            Err(_) => {
                error!("File cannot be opened.");
                Ok(as_obj(pmb_tc(HfstTransducer::new())))
            }
            Ok(f) => {
                let mut reader = std::io::BufReader::new(f);
                let mut linecount: u32 = 0;
                let tmp =
                    crate::hfst_basic_transducer::HfstBasicTransducer::read_in_prolog_format_file(
                        &mut reader,
                        &mut linecount,
                    )?;
                let mut t = Box::new(HfstTransducer::new_from_basic_transducer(&tmp));
                t.minimize()?;
                Ok(as_obj(pmb_tc(*t)))
            }
        },
        RK::Regex => {
            let mut regex = String::new();
            if let Ok(contents) = std::fs::read_to_string(&filepath) {
                for line in contents.lines() {
                    regex.push_str(line);
                }
            }
            if regex.is_empty() {
                error!("Failed to read regex from {}.", filepath);
            }
            let mut xre_compiler = crate::xre::XreCompiler::new();
            let compiled = xre_compiler.compile(&regex).unwrap_or_default();
            Ok(as_obj(pmb_tc(compiled)))
        }
    }
}
/// Build a 'PmatchObject*' AST node from an 'nfst-pmatch' expression node.
pub fn build_object<B: AlgebraBackend + FromAnyTransducer + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    e: &nfst_pmatch::SpannedExpr,
) -> crate::error::Result<ObjRef<B>> {
    use nfst_pmatch::BinaryOp as NBinOp;
    use nfst_pmatch::PmatchExpr as PE;
    Ok(match &e.value {
        // ---- atoms ---------------------------------------------------------
        PE::Symbol(s) => {
            let sym = s.clone();
            if sym.is_empty() {
                pmb_empty()
            } else {
                ctx.used_definitions_insert(sym.clone());
                pmb_symbol(sym)
            }
        }
        PE::Literal(s) => pmb_string(s.clone(), false),
        PE::QuotedLiteral(s) => pmb_string(s.clone(), false),
        PE::CurlyLiteral(s) => pmb_string(s.clone(), true),
        PE::Epsilon => pmb_string(
            Symbol::from(crate::hfst_symbol_defs::internal_epsilon),
            false,
        ),
        PE::BoundaryMarker => pmb_string(Symbol::from("@BOUNDARY@"), false),
        PE::Any => pmb_question_mark(),
        PE::Acceptor(a) => pmb_acceptor(map_acceptor(*a)),
        PE::CharacterRange { from, to } => {
            let raw = format!("\"{}-{}\"", from, to);
            as_obj(parse_range(ctx, &raw)?)
        }

        // ---- operators -----------------------------------------------------
        PE::Binary(op, l, r) => match op {
            NBinOp::Concatenate => pmb_binary(
                PmatchBinaryOp::Concatenate,
                build_object(ctx, l)?,
                build_object(ctx, r)?,
            ),
            NBinOp::Compose => pmb_binary(
                PmatchBinaryOp::Compose,
                build_object(ctx, l)?,
                build_object(ctx, r)?,
            ),
            NBinOp::LenientCompose => pmb_binary(
                PmatchBinaryOp::LenientCompose,
                build_object(ctx, l)?,
                build_object(ctx, r)?,
            ),
            NBinOp::CrossProduct => pmb_binary(
                PmatchBinaryOp::CrossProduct,
                build_object(ctx, l)?,
                build_object(ctx, r)?,
            ),
            NBinOp::MergeRight => pmb_binary(
                PmatchBinaryOp::Merge,
                build_object(ctx, l)?,
                build_object(ctx, r)?,
            ),
            NBinOp::MergeLeft => {
                // .<m. swaps the operands: Merge($3, $1).
                let lo = build_object(ctx, l)?;
                let ro = build_object(ctx, r)?;
                pmb_binary(PmatchBinaryOp::Merge, ro, lo)
            }
            NBinOp::Before => pmb_binary(
                PmatchBinaryOp::Before,
                build_object(ctx, l)?,
                build_object(ctx, r)?,
            ),
            NBinOp::After => pmb_binary(
                PmatchBinaryOp::After,
                build_object(ctx, l)?,
                build_object(ctx, r)?,
            ),
            NBinOp::Shuffle => pmb_binary(
                PmatchBinaryOp::Shuffle,
                build_object(ctx, l)?,
                build_object(ctx, r)?,
            ),
            NBinOp::Union => pmb_binary(
                PmatchBinaryOp::Disjunct,
                build_object(ctx, l)?,
                build_object(ctx, r)?,
            ),
            NBinOp::Intersect => pmb_binary(
                PmatchBinaryOp::Intersect,
                build_object(ctx, l)?,
                build_object(ctx, r)?,
            ),
            NBinOp::Subtract => pmb_binary(
                PmatchBinaryOp::Subtract,
                build_object(ctx, l)?,
                build_object(ctx, r)?,
            ),
            NBinOp::UpperSubtract => pmb_binary(
                PmatchBinaryOp::UpperSubtract,
                build_object(ctx, l)?,
                build_object(ctx, r)?,
            ),
            NBinOp::LowerSubtract => pmb_binary(
                PmatchBinaryOp::LowerSubtract,
                build_object(ctx, l)?,
                build_object(ctx, r)?,
            ),
            NBinOp::UpperPriorityUnion => pmb_binary(
                PmatchBinaryOp::UpperPriorityUnion,
                build_object(ctx, l)?,
                build_object(ctx, r)?,
            ),
            NBinOp::LowerPriorityUnion => pmb_binary(
                PmatchBinaryOp::LowerPriorityUnion,
                build_object(ctx, l)?,
                build_object(ctx, r)?,
            ),
            NBinOp::Ignoring => pmb_binary(
                PmatchBinaryOp::InsertFreely,
                build_object(ctx, l)?,
                build_object(ctx, r)?,
            ),
            NBinOp::IgnoreInternally => pmb_binary(
                PmatchBinaryOp::IgnoreInternally,
                build_object(ctx, l)?,
                build_object(ctx, r)?,
            ),
            NBinOp::LeftQuotient => {
                warn!("Left quotient not implemented");
                pmb_empty()
            }
        },
        PE::Unary(op, inner) => pmb_unary(map_unop(*op), build_object(ctx, inner)?),

        // ---- grouping / weight / pair --------------------------------------
        PE::Group(inner) => build_object(ctx, inner)?,
        PE::Optional(inner) => pmb_unary(PmatchUnaryOp::Optionalize, build_object(ctx, inner)?),
        PE::BracketedDotted(inner) => match inner {
            Some(b) => build_object(ctx, b)?,
            None => pmb_epsilon_arc(),
        },
        PE::Pair { upper, lower } => pmb_binary(
            PmatchBinaryOp::CrossProduct,
            build_object(ctx, upper)?,
            build_object(ctx, lower)?,
        ),
        PE::Weighted { expr, weight } => {
            let mut obj = build_object(ctx, expr)?;
            let new_weight = obj.get_weight() + *weight;
            Rc::get_mut(&mut obj)
                .expect("freshly built node is uniquely owned")
                .set_weight(new_weight);
            obj
        }

        // ---- catenate N ----------------------------------------------------
        PE::RepeatN(inner, n) => pmb_numeric(
            PmatchNumericOp::RepeatN,
            build_object(ctx, inner)?,
            vec![*n as i32],
        ),
        PE::RepeatNPlus(inner, n) => pmb_numeric(
            PmatchNumericOp::RepeatNPlus,
            build_object(ctx, inner)?,
            vec![*n as i32 + 1],
        ),
        PE::RepeatNMinus(inner, n) => pmb_numeric(
            PmatchNumericOp::RepeatNMinus,
            build_object(ctx, inner)?,
            vec![*n as i32 - 1],
        ),
        PE::RepeatNToK(inner, n, k) => pmb_numeric(
            PmatchNumericOp::RepeatNToK,
            build_object(ctx, inner)?,
            vec![*n as i32, *k as i32],
        ),

        // ---- replacement / restriction -------------------------------------
        PE::Replace { arrow, rules } => {
            let mapped_arrow = map_arrow(*arrow);
            let mut rule_ptrs: Vec<Rc<PmatchReplaceRuleContainer<B>>> = Vec::new();
            for rule in rules.iter() {
                // MAPPINGPAIR_VECTOR -> mapping pairs
                let mut mapping: MappingPairVector<B> = Vec::new();
                for mp in rule.mappings.iter() {
                    use nfst_pmatch::MappingKind as MK;
                    let pair: PairRef<B> = match &mp.kind {
                        MK::Plain { lower } => {
                            pmb_object_pair(side_to_obj(ctx, &mp.upper)?, side_to_obj(ctx, lower)?)
                        }
                        MK::Markup { pre, post } => {
                            let loa = side_to_obj(ctx, &mp.upper)?;
                            let lom = match pre {
                                Some(s) => side_to_obj(ctx, s)?,
                                None => pmb_epsilon_arc(),
                            };
                            let rom = match post {
                                Some(s) => side_to_obj(ctx, s)?,
                                None => pmb_epsilon_arc(),
                            };
                            pmb_markup_container(loa, lom, rom)
                        }
                    };
                    mapping.push(pair);
                }
                // CONTEXTS_WITH_MARK -> context pairs + type
                let (rtype, context): (ReplaceType, MappingPairVector<B>) = match &rule.contexts {
                    Some(ctxs) => {
                        let mut context: MappingPairVector<B> = Vec::new();
                        for c in ctxs.items.iter() {
                            let l = match &c.left {
                                Some(e) => build_object(ctx, e)?,
                                None => pmb_epsilon_arc(),
                            };
                            let r = match &c.right {
                                Some(e) => build_object(ctx, e)?,
                                None => pmb_epsilon_arc(),
                            };
                            context.push(pmb_object_pair(l, r));
                        }
                        (map_mark(ctxs.mark), context)
                    }
                    None => (ReplaceType::REPL_UP, Vec::new()),
                };
                rule_ptrs.push(Rc::new(PmatchReplaceRuleContainer {
                    name: String::new(),
                    weight: 0.0,
                    line_defined: 0,
                    arrow: mapped_arrow,
                    ty: rtype,
                    mapping,
                    context,
                }));
            }
            as_obj(Rc::new(PmatchParallelRulesContainer {
                name: String::new(),
                weight: 0.0,
                line_defined: 0,
                arrow: mapped_arrow,
                rules: rule_ptrs,
            }))
        }
        PE::Restriction { body, contexts } => {
            let left = build_object(ctx, body)?;
            let mut ctxs: MappingPairVector<B> = Vec::new();
            for rc in contexts.iter() {
                let l: ObjRef<B> = match &rc.left {
                    Some(e) => build_object(ctx, e)?,
                    None => {
                        if rc.right.is_some() {
                            pmb_epsilon_arc()
                        } else {
                            pmb_empty()
                        }
                    }
                };
                let r: ObjRef<B> = match &rc.right {
                    Some(e) => build_object(ctx, e)?,
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
            as_obj(Rc::new(PmatchRestrictionContainer {
                name: String::new(),
                weight: 0.0,
                line_defined: 0,
                left,
                contexts: ctxs,
            }))
        }

        // ---- pmatch-specific constructs ------------------------------------
        PE::Ins(name) => {
            if !ctx.flatten {
                if !ctx.definitions_contains(name) {
                    ctx.unsatisfied_insertions_insert(name.clone());
                }
                let retval = pmb_string(Symbol::from(ins_transition(name)), false);
                ctx.inserted_names_insert(name.clone());
                ctx.used_definitions_insert(name.clone());
                retval
            } else if ctx.definitions_contains(name) {
                ctx.definitions_get(name)
                    .expect("definitions_contains verified above")
            } else {
                error!(
                    "Insertion of {} is undefined and --ctx.flatten is in use",
                    name
                );
                pmb_empty()
            }
        }
        PE::EndTag(name) => {
            let retval = as_obj(make_end_tag(ctx, name.to_string())?);
            ctx.need_delimiters = true;
            retval
        }
        PE::Capture(name) => {
            let retval = as_obj(make_capture_tag(ctx, name.to_string())?);
            let captured = make_captured_tag(ctx, name.to_string())?;
            if ctx.definitions_contains(name) {
                warn(format!(
                    "definition of {} on line {} shadows earlier definition\n",
                    name, 0
                ));
            }
            ctx.definitions_insert(name.to_string(), as_obj(captured));
            ctx.need_delimiters = true;
            retval
        }
        PE::Tag { body, name } => {
            // AddDelimiters(Concatenate(body, make_end_tag(ctx, name)))
            let cat = pmb_binary(
                PmatchBinaryOp::Concatenate,
                build_object(ctx, body)?,
                as_obj(make_end_tag(ctx, name.to_string())?),
            );
            pmb_unary(PmatchUnaryOp::AddDelimiters, cat)
        }
        PE::With { body, name, value } => {
            // Concatenate(Concatenate(entry, body), exit)
            let entry = make_with_tag_entry(name.to_string(), value.to_string());
            let exit = make_with_tag_exit(name.to_string());
            let inner = pmb_binary(PmatchBinaryOp::Concatenate, entry, build_object(ctx, body)?);
            pmb_binary(PmatchBinaryOp::Concatenate, inner, exit)
        }
        PE::Counter(name) => as_obj(make_counter(ctx, name.to_string())?),
        PE::CaseOp { op, side, body } => {
            pmb_unary(map_caseop(*op, *side), build_object(ctx, body)?)
        }
        PE::DefineWrapper(inner) => {
            pmb_unary(PmatchUnaryOp::AddDelimiters, build_object(ctx, inner)?)
        }
        PE::Explode(items) => pmb_unary(
            PmatchUnaryOp::Explode,
            build_concatenated_string_list(ctx, items)?,
        ),
        PE::Implode(items) => pmb_unary(
            PmatchUnaryOp::Implode,
            build_concatenated_string_list(ctx, items)?,
        ),
        PE::Like {
            args,
            threshold,
            unlike,
        } => {
            // The C++ ARGLIST is in reverse source order; replicate.
            let rargs: Vec<String> = args.iter().rev().map(|a| a.to_string()).collect();
            let nwords = threshold.unwrap_or(10);
            if *unlike {
                if rargs.len() < 2 {
                    error!(
                        "Unlike() operation takes exactly 2 arguments, got {}",
                        rargs.len()
                    );
                    pmb_empty()
                } else {
                    compile_like_arc(ctx, rargs[1].clone(), rargs[0].clone(), nwords, true)?
                }
            } else {
                match rargs.len() {
                    0 => compile_like_arc_word(ctx, String::new(), 10)?,
                    1 => compile_like_arc_word(ctx, rargs[0].clone(), nwords)?,
                    _ => compile_like_arc(ctx, rargs[0].clone(), rargs[1].clone(), nwords, false)?,
                }
            }
        }
        PE::Lst(inner) => pmb_unary(PmatchUnaryOp::MakeList, build_object(ctx, inner)?),
        PE::Exc(inner) => pmb_unary(PmatchUnaryOp::MakeExcList, build_object(ctx, inner)?),
        PE::Sigma(inner) => pmb_unary(PmatchUnaryOp::MakeSigma, build_object(ctx, inner)?),
        PE::Interpolate(items) => {
            // FUNCALL_ARGLIST is in reverse source order; replicate.
            let argvec: Vec<ObjRef<B>> = items
                .iter()
                .rev()
                .map(|it| build_object(ctx, it))
                .collect::<crate::error::Result<_>>()?;
            as_obj(Rc::new(PmatchBuiltinFunction {
                name: String::new(),
                weight: 0.0,
                line_defined: 0,
                args: argvec,
                ty: PmatchBuiltin::Interpolate,
            }))
        }
        PE::Substitute(a, b, c) => pmb_ternary(
            PmatchTernaryOp::Substitute,
            build_object(ctx, a)?,
            build_object(ctx, b)?,
            build_object(ctx, c)?,
        ),
        PE::Uncompose(a, b, c) => {
            let left = build_stringlike(ctx, a)?;
            let middle = build_stringlike(ctx, b)?;
            let right = build_stringlike(ctx, c)?;
            let middle_str = Symbol::from(middle.as_string(ctx));
            ctx.uncomposed_insert(middle_str.clone());
            ctx.used_definitions_insert(middle_str);
            let right_str = Symbol::from(right.as_string(ctx));
            ctx.uncomposed_insert(right_str.clone());
            ctx.used_definitions_insert(right_str);
            pmb_ternary(PmatchTernaryOp::Uncompose, left, middle, right)
        }

        // ---- context conditions --------------------------------------------
        PE::Lc(inner) => {
            let retval = pmb_unary(PmatchUnaryOp::LC, build_object(ctx, inner)?);
            ctx.need_delimiters = true;
            retval
        }
        PE::Rc(inner) => {
            let retval = pmb_unary(PmatchUnaryOp::RC, build_object(ctx, inner)?);
            ctx.need_delimiters = true;
            retval
        }
        PE::Nlc(inner) => {
            let retval = pmb_unary(PmatchUnaryOp::NLC, build_object(ctx, inner)?);
            ctx.need_delimiters = true;
            retval
        }
        PE::Nrc(inner) => {
            let retval = pmb_unary(PmatchUnaryOp::NRC, build_object(ctx, inner)?);
            ctx.need_delimiters = true;
            retval
        }
        PE::OrContext(items) => {
            let mut result: Option<ObjRef<B>> = None;
            for it in items.iter() {
                let obj = build_object(ctx, it)?;
                result = match result {
                    None => Some(obj),
                    Some(prev) => Some(pmb_binary(PmatchBinaryOp::Disjunct, prev, obj)),
                };
            }
            // Zero the counter for making minimization guards for disjuncted
            // negative contexts.
            ctx.zero_minimization_guard();
            ctx.need_delimiters = true;
            result.unwrap_or_else(|| pmb_empty())
        }
        PE::AndContext(items) => {
            let mut result: Option<ObjRef<B>> = None;
            for it in items.iter() {
                let obj = build_object(ctx, it)?;
                result = match result {
                    None => Some(obj),
                    Some(prev) => Some(pmb_binary(PmatchBinaryOp::Concatenate, prev, obj)),
                };
            }
            ctx.need_delimiters = true;
            result.unwrap_or_else(|| pmb_empty())
        }

        // ---- function call -------------------------------------------------
        PE::Call { name, args } => {
            let sym = name.clone();
            let result = if !ctx.function_names_contains(name) {
                error!("Function {} hasn't been defined", sym);
                pmb_string(Symbol::default(), false)
            } else {
                let fun = symbol_from_global_context(ctx, &sym)
                    .expect("a defined function name is bound in global definitions");
                let argvec: Vec<ObjRef<B>> = args
                    .iter()
                    .rev()
                    .map(|a| build_object(ctx, a))
                    .collect::<crate::error::Result<_>>()?;
                as_obj(Rc::new(PmatchFuncall {
                    name: String::new(),
                    weight: 0.0,
                    line_defined: 0,
                    args: argvec,
                    fun,
                }))
            };
            ctx.used_definitions_insert(sym);
            result
        }

        // ---- file references -----------------------------------------------
        PE::ReadFile { kind, path } => build_read_file(ctx, *kind, path)?,
        PE::ReadLexc(path) => {
            let filepath = path_of(ctx, path);
            as_obj(pmb_tc(HfstTransducer::read_lexc(&filepath, ctx.verbose)?))
        }
        PE::ReadVec(path) => {
            let filepath = path_of(ctx, path);
            read_vec(ctx, filepath);
            pmb_empty()
        }
    })
}
// EXPRESSION1: EXPRESSION2 END_OF_WEIGHTED_EXPRESSION { weight += w; wrap in
// AddDelimiters if need_delimiters; reset need_delimiters. } The trailing
// weight is folded into a 'Weighted' node by nfst, so only the delimiter wrap
// remains here.
fn build_expression1<B: AlgebraBackend + FromAnyTransducer + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    body: &nfst_pmatch::SpannedExpr,
) -> crate::error::Result<ObjRef<B>> {
    let obj = build_object(ctx, body)?;
    let result = if ctx.need_delimiters {
        pmb_unary(PmatchUnaryOp::AddDelimiters, obj)
    } else {
        obj
    };
    ctx.need_delimiters = false;
    Ok(result)
}
// PMATCH DEFINITION verbose timer report.
fn report_defined<B: AlgebraBackend + 'static>(ctx: &mut PmatchEvalContext<B>, name: &str) {
    if ctx.verbose {
        let duration = (clock() - ctx.timer) as f64 / CLOCKS_PER_SEC as f64;
        ctx.timer = clock();
        debug!("defined {} in {:.2} seconds", name, duration);
    }
}
// PMATCH DEFINITION { shadow check + insert }.
fn insert_definition<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    name: String,
    obj: ObjRef<B>,
) {
    if ctx.definitions_contains(&name) {
        warn(format!(
            "definition of {} on line {} shadows earlier definition\n",
            name, 0
        ));
    }
    ctx.definitions_insert(name, obj);
}
/// Apply one top-level 'nfst-pmatch' statement (definition / def-ins /
/// regex-top / set-variable / list-definition / read-vec), populating the
/// 'hfst::pmatch' globals.
pub fn build_statement<B: AlgebraBackend + FromAnyTransducer + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    s: &nfst_pmatch::Spanned<nfst_pmatch::PmatchStatement>,
) -> crate::error::Result<()> {
    use nfst_pmatch::PmatchStatement as PS;
    use nfst_pmatch::VariableValue;
    match &s.value {
        PS::Define { name, params, body } => match params {
            None => {
                let mut obj = build_expression1(ctx, body)?;
                // Under --flatten, `Ins(X)` inlines by returning X's shared
                // definition node (see PE::Ins), so a `Define A Ins(X)` body can
                // hand us an object that already aliases an existing definition
                // (Rc strong count > 1). Only rename when we uniquely own it;
                // an aliased inline keeps its source name — used solely for
                // eval-stack labels — instead of panicking in Rc::get_mut or
                // mutating the shared node out from under the other definition.
                if let Some(node) = Rc::get_mut(&mut obj) {
                    node.set_name(name.to_string());
                }
                report_defined(ctx, name);
                insert_definition(ctx, name.to_string(), obj);
            }
            Some(args) => {
                let root = build_expression1(ctx, body)?;
                // The C++ ARGLIST is in reverse source order; replicate.
                let mut fun = Rc::new(PmatchFunction {
                    name: String::new(),
                    weight: 0.0,
                    line_defined: 0,
                    args: args.iter().rev().cloned().collect(),
                    root,
                });
                Rc::get_mut(&mut fun)
                    .expect("freshly built node is uniquely owned")
                    .name = name.to_string();
                ctx.function_names_insert(name.clone());
                report_defined(ctx, name);
                insert_definition(ctx, name.to_string(), as_obj(fun));
            }
        },
        PS::DefIns { name, body } => {
            let mut body_obj = build_expression1(ctx, body)?;
            Rc::get_mut(&mut body_obj)
                .expect("freshly built node is uniquely owned")
                .set_name(name.to_string());
            ctx.def_insed_expressions_insert(name.to_string(), body_obj);
            let def_value = pmb_string(Symbol::from(ins_transition(name)), false);
            report_defined(ctx, name);
            insert_definition(ctx, name.to_string(), def_value);
        }
        PS::RegexTop { body } => {
            let mut obj = build_expression1(ctx, body)?;
            Rc::get_mut(&mut obj)
                .expect("freshly built node is uniquely owned")
                .set_name("TOP".to_string());
            report_defined(ctx, "TOP");
            insert_definition(ctx, "TOP".to_string(), obj);
        }
        PS::SetVariable { name, value } => {
            let v = match value {
                VariableValue::Symbol(s) => s.to_string(),
                VariableValue::Epsilon => "0".to_string(),
            };
            ctx.variables_insert(name.to_string(), v);
        }
        PS::ListDefinition { name, body } => {
            // DEFINED_LIST: the name lands on the inner body, the stored value
            // is the MakeSigma wrapper.
            let mut inner = build_expression1(ctx, body)?;
            Rc::get_mut(&mut inner)
                .expect("freshly built node is uniquely owned")
                .set_name(name.to_string());
            let value = pmb_unary(PmatchUnaryOp::MakeSigma, inner);
            report_defined(ctx, name);
            insert_definition(ctx, name.to_string(), value);
        }
        PS::ReadVec { path } => {
            let filepath = path_of(ctx, path);
            read_vec(ctx, filepath);
        }
    }
    Ok(())
}
// [spec:hfst:def:pmatch-compiler.hfst.pmatch.pmatch-compiler.pmatch-compiler-fn]
// [spec:hfst:sem:pmatch-compiler.hfst.pmatch.pmatch-compiler.pmatch-compiler-fn]
//
// The C++ default constructor fixed the format to TROPICAL_OPENFST_TYPE; the
// format is the type parameter 'B' now ([dec:hfst:monomorphic-backends]).
impl<B: AlgebraBackend + FromAnyTransducer + 'static> Default for PmatchCompiler<B> {
    fn default() -> Self {
        PmatchCompiler::new()
    }
}

impl<B: AlgebraBackend + FromAnyTransducer + 'static> PmatchCompiler<B> {
    pub fn new() -> Self {
        PmatchCompiler {
            verbose: false,
            flatten: false,
            include_cosine_distances: false,
            includedir: String::new(),
            definitions: BTreeMap::new(),
            eval_ctx: PmatchEvalContext::new(),
        }
    }

    // [spec:hfst:def:pmatch-compiler.hfst.pmatch.pmatch-compiler.set-flatten-fn]
    // [spec:hfst:sem:pmatch-compiler.hfst.pmatch.pmatch-compiler.set-flatten-fn]
    pub fn set_flatten(&mut self, val: bool) {
        self.flatten = val;
    }

    // [spec:hfst:def:pmatch-compiler.hfst.pmatch.pmatch-compiler.set-verbose-fn]
    // [spec:hfst:sem:pmatch-compiler.hfst.pmatch.pmatch-compiler.set-verbose-fn]
    pub fn set_verbose(&mut self, val: bool) {
        self.verbose = val;
    }

    // [spec:hfst:def:pmatch-compiler.hfst.pmatch.pmatch-compiler.set-include-cosine-distances-fn]
    // [spec:hfst:sem:pmatch-compiler.hfst.pmatch.pmatch-compiler.set-include-cosine-distances-fn]
    pub fn set_include_cosine_distances(&mut self, val: bool) {
        self.include_cosine_distances = val;
    }

    // [spec:hfst:def:pmatch-compiler.hfst.pmatch.pmatch-compiler.define-fn]
    // [spec:hfst:sem:pmatch-compiler.hfst.pmatch.pmatch-compiler.define-fn]
    //
    // Reads the global 'definitions' map (populated by 'compile') and stores the
    // evaluated transducer into the member 'definitions', mirroring the C++.
    pub fn define(&mut self, name: &str, pmatch: &str) -> crate::error::Result<()> {
        self.compile(pmatch)?;
        let ctx = &mut self.eval_ctx;
        if ctx.definitions_contains(name) {
            let obj = ctx
                .definitions_get(name)
                .expect("definitions_contains verified above");
            let evaluated = obj.evaluate(ctx)?;
            self.definitions.insert(name.to_string(), evaluated);
        }
        Ok(())
    }

    // [spec:hfst:def:pmatch-compiler.hfst.pmatch.pmatch-compiler.compile-fn]
    // [spec:hfst:sem:pmatch-compiler.hfst.pmatch.pmatch-compiler.compile-fn]
    // Mirrors 'hfst::pmatch::compile', with the bison 'pmatchparse()' step
    // replaced by a walk over the 'nfst-pmatch' AST (the sanctioned deviation).
    pub fn compile(
        &mut self,
        src: &str,
    ) -> crate::error::Result<HashMap<String, HfstTransducer<B>>> {
        {
            let ctx = &mut self.eval_ctx;
            ctx.init_globals();
            let expanded_script = expand_includes(ctx, src);
            ctx.verbose = self.verbose;
            ctx.flatten = self.flatten;
            ctx.include_cosine_distances = self.include_cosine_distances;
            ctx.includedir = self.includedir.clone();
            ctx.vector_similarity_projection_factor = 1.0;
            if ctx.verbose {
                ctx.timer = clock();
                debug!("");
            }

            // ---- bison-replacement walk ------------------------------------
            match nfst_pmatch::parse(&expanded_script) {
                Ok(file) => {
                    for stmt in &file.value.statements {
                        if let Err(e) = build_statement(ctx, stmt) {
                            error!("{}", e);
                            ctx.data = String::new();
                            ctx.len = 0;
                            return Ok(HashMap::new());
                        }
                    }
                }
                Err(e) => {
                    error!("{}", format_pmatch_parse_error(&e));
                    ctx.data = String::new();
                    ctx.len = 0;
                    return Ok(HashMap::new());
                }
            }

            let mut retval: HashMap<String, HfstTransducer<B>> = HashMap::new();

            for it in ctx.unsatisfied_insertions_snapshot().into_iter() {
                if !ctx.definitions_contains(it.as_str()) {
                    error!("Inserted transducer {} was never defined!", it);
                    ctx.data = String::new();
                    ctx.len = 0;
                    return Ok(retval);
                }
            }
            if ctx.verbose {
                for (k, _v) in ctx.definitions_snapshot() {
                    if !ctx.used_definitions_contains(&k) && k != "TOP" {
                        debug!("Warning: {} defined but never used", k);
                    }
                }
            }

            if ctx.verbose {
                debug!("Compiling and harmonizing...");
                ctx.timer = clock();
            }

            let mut uncount: u32 = 0;
            if !ctx.inserted_names_is_empty()
                || !ctx.def_insed_expressions_is_empty()
                || !ctx.uncomposed_is_empty()
            {
                let mut dummy = HfstTransducer::new();
                let keys: Vec<String> = ctx.definitions_keys();
                for key in &keys {
                    if key == "TOP"
                        || ctx.inserted_names_contains(key)
                        || ctx.def_insed_expressions_contains(key)
                        || ctx.uncomposed_contains(key)
                    {
                        let obj_ptr: ObjRef<B> = if ctx.def_insed_expressions_contains(key) {
                            ctx.def_insed_expressions_get(key)
                                .expect("checked with contains just above")
                        } else {
                            ctx.definitions_get(key).expect("key is a definitions key")
                        };
                        let mut tmp: HfstTransducer<B> = obj_ptr.evaluate(ctx)?;
                        tmp.minimize()?;
                        dummy.harmonize(&mut tmp, false)?;
                        if ctx.uncomposed_contains(key) {
                            if uncount == 0 {
                                tmp.set_name(&format!("UNCOMPOSE LEFT {}", key));
                                retval.insert(format!("UNCOMPOSE LEFT {}", key), tmp);
                                uncount += 1;
                            } else if uncount == 1 {
                                tmp.set_name(&format!("UNCOMPOSE RIGHT {}", key));
                                retval.insert(format!("UNCOMPOSE RIGHT {}", key), tmp);
                                uncount += 1;
                            } else {
                                warn!("Uncompose only works once so far...");
                                uncount += 1;
                            }
                        } else {
                            tmp.set_name(key);
                            retval.insert(key.clone(), tmp);
                        }
                    }
                }
                for v in retval.values_mut() {
                    v.harmonize(&mut dummy, false)?;
                    v.minimize()?;
                }
            } else if ctx.definitions_is_empty() {
                warn!("pmatch compilation had an empty result");
                retval.insert("TOP".to_string(), HfstTransducer::new());
            } else if !ctx.definitions_contains("TOP") {
                let (first_key, first_obj) = {
                    let snap = ctx.definitions_snapshot();
                    let mut it = snap.iter();
                    let (k, v) = it.next().expect("definitions non-empty in this branch");
                    (k.clone(), v.clone())
                };
                warn!(
                    "Pmatch compilation: regex or TOP was undefined, using {} as root",
                    first_key
                );
                let mut tmp: HfstTransducer<B> = first_obj.evaluate(ctx)?;
                tmp.minimize()?;
                tmp.set_name("TOP");
                retval.insert("TOP".to_string(), tmp);
            } else {
                let top_obj = ctx
                    .definitions_get("TOP")
                    .expect("TOP present in this branch");
                let mut tmp: HfstTransducer<B> = top_obj.evaluate(ctx)?;
                tmp.minimize()?;
                tmp.set_name("TOP");
                retval.insert("TOP".to_string(), tmp);
            }

            if ctx.verbose {
                let duration = (clock() - ctx.timer) as f64 / CLOCKS_PER_SEC as f64;
                ctx.timer = clock();
                debug!("Everything compiled and harmonized in {} seconds", duration);
            }

            let mut allowed_initial_symbols: StringSet = StringSet::new();
            let mut disallowed_initial_symbols: StringSet = StringSet::new();
            if let Some(top) = ctx.definitions_get("TOP") {
                top.collect_initial_symbols_into(
                    &mut allowed_initial_symbols,
                    &mut disallowed_initial_symbols,
                )?;
            }
            let mut initial_symbols_list = String::new();
            let mut disallowed_initial_symbols_list = String::new();
            let mut initial_symbols_ok = true;
            for it in allowed_initial_symbols.iter() {
                if is_special(it) {
                    if ctx.verbose {
                        debug!(
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
                    if ctx.verbose {
                        debug!(
                            "Not setting initial symbol list due to special symbol {}",
                            it
                        );
                    }
                    initial_symbols_ok = false;
                }
                disallowed_initial_symbols_list.push_str(it);
            }
            if allowed_initial_symbols.len() > 200 {
                if ctx.verbose {
                    debug!(
                        "Not setting initial symbol list due to excess length: {}",
                        allowed_initial_symbols.len()
                    );
                }
                initial_symbols_ok = false;
            }
            if disallowed_initial_symbols.len() > 200 {
                if ctx.verbose {
                    debug!(
                        "Not setting initial symbol list due to excess length: {}",
                        disallowed_initial_symbols.len()
                    );
                }
                initial_symbols_ok = false;
            }
            if initial_symbols_ok && !initial_symbols_list.is_empty() {
                ctx.variables_insert("initial-symbols".to_string(), initial_symbols_list);
            }
            if initial_symbols_ok && !disallowed_initial_symbols_list.is_empty() {
                ctx.variables_insert(
                    "disallowed-initial-symbols".to_string(),
                    disallowed_initial_symbols_list,
                );
            }

            if ctx.variables_get("need-separators").as_deref() == Some("on") {
                let whitespace_acc =
                    ctx.with_utils(|u| HfstTransducer::new_copy(&u.latin1_whitespace_acceptor))?;
                let punct_acc =
                    ctx.with_utils(|u| HfstTransducer::new_copy(&u.latin1_punct_acceptor))?;
                let mut not_whitespace =
                    HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
                not_whitespace.subtract(&whitespace_acc, true)?;
                let mut anything =
                    HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
                anything.repeat_star()?;
                let mut begins_and_ends_with_non_whitespace =
                    HfstTransducer::new_from_transducer(&not_whitespace);
                begins_and_ends_with_non_whitespace.concatenate(&anything, true)?;
                begins_and_ends_with_non_whitespace.concatenate(&not_whitespace, true)?;
                begins_and_ends_with_non_whitespace
                    .compose(retval.get("TOP").expect("TOP defined above"), true)?;
                let mut is_single_non_whitespace =
                    HfstTransducer::new_from_transducer(&not_whitespace);
                is_single_non_whitespace
                    .compose(retval.get("TOP").expect("TOP defined above"), true)?;
                let empty = HfstTransducer::new();
                if !begins_and_ends_with_non_whitespace.compare(&empty, true)?
                    || !is_single_non_whitespace.compare(&empty, true)?
                {
                    let mut whitespace_punct_context =
                        HfstTransducer::new_from_transducer(&whitespace_acc);
                    whitespace_punct_context.disjunct(&punct_acc, true)?;
                    whitespace_punct_context
                        .disjunct(&HfstTransducer::new_symbol("@BOUNDARY@")?, true)?;
                    let mut top_with_boundaries = HfstTransducer::new_symbol_pair(
                        crate::hfst_symbol_defs::internal_epsilon,
                        LC_ENTRY_SYMBOL,
                    )?;
                    top_with_boundaries.concatenate(&whitespace_punct_context, true)?;
                    top_with_boundaries.concatenate(
                        &HfstTransducer::new_symbol_pair(
                            crate::hfst_symbol_defs::internal_epsilon,
                            LC_EXIT_SYMBOL,
                        )?,
                        true,
                    )?;
                    let mut rc = HfstTransducer::new_symbol_pair(
                        crate::hfst_symbol_defs::internal_epsilon,
                        RC_ENTRY_SYMBOL,
                    )?;
                    rc.concatenate(&whitespace_punct_context, true)?;
                    rc.concatenate(
                        &HfstTransducer::new_symbol_pair(
                            crate::hfst_symbol_defs::internal_epsilon,
                            RC_EXIT_SYMBOL,
                        )?,
                        true,
                    )?;
                    top_with_boundaries
                        .concatenate(retval.get("TOP").expect("TOP defined above"), true)?;
                    top_with_boundaries.concatenate(&rc, true)?;
                    let mut new_top = add_pmatch_delimiters(&top_with_boundaries)?;
                    new_top.minimize()?;
                    retval.insert("TOP".to_string(), new_top);
                    if ctx.verbose {
                        let duration = (clock() - ctx.timer) as f64 / CLOCKS_PER_SEC as f64;
                        ctx.timer = clock();
                        debug!("Added automatic context separators in {} seconds", duration);
                    }
                }
            }

            let vars: Vec<(String, String)> = ctx.variables_snapshot();
            let top = retval
                .get_mut("TOP")
                .expect("TOP present in result by this point");
            for (k, v) in &vars {
                top.set_property(k, v);
            }
            ctx.data = String::new();
            ctx.len = 0;
            Ok(retval)
        }
    }

    // [spec:hfst:def:pmatch-compiler.hfst.pmatch.pmatch-compiler.set-include-path-fn]
    // [spec:hfst:sem:pmatch-compiler.hfst.pmatch.pmatch-compiler.set-include-path-fn]
    pub fn set_include_path(&mut self, path: String) {
        self.includedir = path;
    }
}

// ---------------------------------------------------------------------------
// Archive writing, lifted from tools/src/hfst-pmatch2fst.cc: turn a compiled
// ruleset (the definitions map from 'PmatchCompiler::compile') into a
// weighted optimized-lookup archive with every transducer harmonized against
// one shared alphabet, TOP first. The tool keeps its option parsing, stream
// opening and the includedir computation; verbose progress goes to a
// caller-supplied writer.
// ---------------------------------------------------------------------------

/// Build the shared harmonizer for a compiled ruleset: a transducer holding
/// the union of every definition's alphabet; ['write_archive'] turns it into
/// the optimized-lookup backend that harmonizes each archive member. Returns
/// None when the definitions carry no symbols at all (an empty ruleset).
pub fn build_archive_harmonizer<B: AlgebraBackend>(
    definitions: &HashMap<String, HfstTransducer<B>>,
) -> crate::error::Result<Option<HfstTransducer<B>>> {
    // A dummy transducer with an alphabet with all the symbols
    let mut harmonizer = HfstTransducer::new();
    // First we need to collect a unified alphabet from all the transducers.
    let mut symbols_seen: StringSet = BTreeSet::new();
    // Iterate in key order to mirror std::map's ordered iteration.
    let mut keys: Vec<&String> = definitions.keys().collect();
    keys.sort();
    for key in &keys {
        let t = &definitions[*key];
        let string_set = t.get_alphabet()?;
        for sym in string_set.iter() {
            if !symbols_seen.contains(sym) {
                harmonizer.insert_to_alphabet(sym)?;
                symbols_seen.insert(sym.clone());
            }
        }
    }
    if symbols_seen.is_empty() {
        // We don't recognise anything, go home early
        return Ok(None);
    }

    Ok(Some(harmonizer))
}

/// Write a compiled ruleset as a weighted optimized-lookup archive: TOP
/// first, then the remaining definitions in name order, each harmonized
/// against the shared alphabet from ['build_archive_harmonizer']. Returns
/// false — writing nothing — when the ruleset is empty (no symbols or no TOP
/// definition); the caller reports that in its own voice. Verbose progress
/// (with timings) goes to 'msg'.
pub fn write_archive<B: AlgebraBackend>(
    definitions: &mut HashMap<String, HfstTransducer<B>>,
    outstream: &mut crate::hfst_output_stream::HfstOutputStream,
    verbose: bool,
    msg: &mut dyn std::io::Write,
) -> crate::error::Result<bool> {
    use crate::convert_transducer_format::ConversionFunctions;

    let mut timer: clock_t = 0;
    if verbose {
        timer = clock();
        let _ = write!(msg, "Building hfst-ol alphabet... ");
    }

    let harmonizer = match build_archive_harmonizer(definitions)? {
        Some(h) => h,
        None => return Ok(false),
    };
    // Use these for naughty intermediate steps to make sure
    // everything has the same alphabet
    // C passed 'HfstTransducer* harmonizer' to the conversion functions,
    // which read its hfst_ol backend; the Rust conversion builds and returns
    // that weighted-OL backend as an owned value, so borrow it below.
    // The harmonizer holds only an alphabet (no transitions), so it is
    // converted with the 'harmonizer_alphabet' option: every symbol is numbered
    // as a potential input symbol, giving the shared header a correct
    // input_symbol_count. Without it each harmonized member's index table is
    // under-padded and the runtime crashes on an out-of-bounds lookup.
    // [upstream hfst/hfst#354]
    let harmonizer_basic =
        ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&harmonizer)?;
    let harmonizer_ol = ConversionFunctions::hfst_basic_transducer_to_hfst_ol(
        &harmonizer_basic,
        true, // weighted
        "harmonizer_alphabet",
        None,
    )?;

    if verbose {
        let duration = (clock() - timer) as f64 / CLOCKS_PER_SEC as f64;
        timer = clock();
        let _ = writeln!(msg, "built in {:.2} seconds", duration);
        let _ = write!(msg, "Converting TOP... ");
    }

    // When done compiling everything, look for TOP and output it first. TOP is
    // the multi-million-arc archive payload: convert it straight from the
    // backend (no basic-transducer intermediate) and drop the source before
    // wrapping, so only one large representation is live at a time — the old
    // tropical+basic+OL+copy chain owned the process's peak RSS.
    let Some(top) = definitions.remove("TOP") else {
        return Ok(false);
    };
    let properties: BTreeMap<String, String> = top.get_properties().clone();
    let harmonized_tmp = top.to_hfst_ol(
        true,                 // weighted
        "",                   // no special options
        Some(&harmonizer_ol), // harmonize with this
    )?;
    drop(top);
    let mut output_tmp = HfstTransducer::wrap(harmonized_tmp);
    output_tmp.set_name("TOP");
    for (k, v) in properties.iter() {
        output_tmp.set_property(k, v);
    }
    outstream.redirect(&mut output_tmp)?;

    if verbose {
        let duration = (clock() - timer) as f64 / CLOCKS_PER_SEC as f64;
        timer = clock();
        let _ = writeln!(msg, "converted in {:.2} seconds", duration);
    }

    let mut rest_keys: Vec<String> = definitions.keys().cloned().collect();
    rest_keys.sort();
    for key in &rest_keys {
        let t = &definitions[key];
        if verbose {
            let _ = writeln!(msg, "Converting {}... ", key);
            timer = clock();
        }
        let harmonized_tmp = if !key.contains("UNCOMPOSE") {
            t.to_hfst_ol(
                true,                 // weighted
                "empty_alphabet",     // empty alphabet in RTNs, they'll use the main one
                Some(&harmonizer_ol), // harmonize with this
            )
        } else {
            t.to_hfst_ol(
                true,                 // weighted
                "",                   // alphabet in UNCs,
                Some(&harmonizer_ol), // harmonize with this
            )
        };
        let harmonized_tmp = harmonized_tmp?;
        let mut output_tmp = HfstTransducer::wrap(harmonized_tmp);
        output_tmp.set_name(key);
        outstream.redirect(&mut output_tmp)?;
        if verbose {
            let duration = (clock() - timer) as f64 / CLOCKS_PER_SEC as f64;
            let _ = writeln!(msg, "converted in {:.2} seconds", duration);
        }
    }
    Ok(true)
}
