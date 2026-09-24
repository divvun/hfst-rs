//! ABSOLUTE-faithful C++->Rust port of HFST's TWOLC (two-level rule) compiler,
//! RESTRUCTURED to walk the 'nfst-twolc' typed AST instead of the original
//! Flex/Bison three-pass preprocessor ('htwolcpre1'/'htwolcpre2'/'htwolcpre3').
//! The AST-walk restructuring is the ONE sanctioned structural deviation in
//! this port: the transducer-building, conflict-resolution and
//! variable-expansion ALGORITHMS must still match the C++ exactly.
//!
//! Ported from 'libhfst/src/parsers/TwolcCompiler.{h,cc}', all of
//! 'libhfst/src/parsers/rule_src/*' and all of
//! 'libhfst/src/parsers/variable_src/*'.
//!
//! # The replaced three-pass preprocessor
//!
//! The C++ 'TwolcCompiler::compile' ran three Flex/Bison passes
//! ('hfst::twolcpre1/2/3::parse()') that lexed, completed the alphabet and
//! finally built the grammar via Bison semantic actions. Here a single
//! ['TwolcCompiler::compile'] call invokes 'nfst_twolc::parse' once and an
//! AST-walk driver drives ['TwolCGrammar'] directly. The intermediate
//! string-queue plumbing ('get_total_alphabet_symbol_queue' etc.) disappears.
//!
//! # C++ statics folded into instance / module state
//!
//! 'OtherSymbolTransducer''s class-level config ('input_symbols',
//! 'output_symbols', 'diacritics', 'symbol_pairs') and the per-container
//! conflict flags ('report_*_conflicts', 'resolve_*_conflicts') were 'static'
//! in C++. Because this port walks the AST and is re-entrant, they are
//! carried as a per-compile ['OstConfig'] (the 'OtherSymbolTransducer'
//! config) and as instance fields on the containers (the conflict flags),
//! instead of process-wide mutable statics. The C++ 'transducer_type' static
//! is the backend type parameter 'B' ([dec:hfst:monomorphic-backends]).
//!
//! # Conventions
//!
//! 'std::set' -> 'BTreeSet', 'std::map' -> 'BTreeMap', 'std::vector' -> 'Vec',
//! 'std::pair<A,B>' -> '(A,B)'. 'HandyMap'/'HandySet' 'has_key'/'has_element'
//! become 'contains_key'/'contains'. C++ owning 'HfstTransducer*' -> owned
//! values / 'Box'. C++ virtual dispatch over the 'Rule' hierarchy -> the
//! closed ['TwolcRule'] enum (match-delegation over the six concrete rule
//! structs). C++ 'throw' ->
//! 'std::panic::panic_any' of the typed exception. Every C++
//! '// [spec:hfst:def/sem:<id>]' annotation is carried onto its Rust site.
//!
//! # Store paths
//!
//! The 'HfstOutputStream' binary store paths ('Rule::store',
//! 'RuleContainer::store', 'TwolCGrammar::compile_and_store_stream' and
//! 'TwolcCompiler::compile_and_store') write the per-rule archive the C++
//! driver emitted; 'TwolCGrammar::compile_and_store' additionally offers an
//! in-memory flavour returning the intersection of every compiled rule.

#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)]
#![allow(clippy::too_many_arguments)]

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

// The nfst-twolc AST rule node is renamed to keep the name 'TwolcRule' free
// for the closed rule sum below (the former 'Box<dyn RuleT>').
#[allow(unused_imports)]
use nfst_twolc::{
    AlphabetPair, CenterSide, RuleCenter, RuleContext, RuleOp, SetDefinition, Spanned,
    TwolcDefinition, TwolcFile, TwolcRegex, TwolcRule as AstTwolcRule, VarMatcher,
    VariableAssignment, VariableBlock,
};
#[allow(unused_imports)]
use nfst_xre::{BinaryOp, UnaryOp};

use crate::backend::AlgebraBackend;
#[allow(unused_imports)]
use crate::hfst_data_types::{
    ImplementationType, StringPair, StringPairVector, StringVector, Symbol,
};
use crate::hfst_transducer::HfstTransducer;
use tracing::{debug, error, info, warn};

// Special symbols (OtherSymbolTransducer.h file-scope 'static const's):
pub const TWOLC_IDENTITY: &str = "@_TWOLC_IDENTITY_SYMBOL_@";
pub const HFST_IDENTITY: &str = "@_IDENTITY_SYMBOL_@";
pub const HFST_UNKNOWN: &str = "@_UNKNOWN_SYMBOL_@";
pub const HFST_EPSILON: &str = "@_EPSILON_SYMBOL_@";
pub const TWOLC_UNKNOWN: &str = "__HFST_TWOLC_?";
pub const TWOLC_DIAMOND: &str = "__HFST_TWOLC_DIAMOND";
pub const TWOLC_EPSILON: &str = "__HFST_TWOLC_0";
pub const TWOLC_FREELY_INSERT: &str = "__HFST_TWOLC_FREELY_INSERT";
/// The RELATIVE word boundary: what the grammar's bare '#' lexes to
/// (htwolcpre1's rename, performed by the nfst-twolc lexer). Distinct from
/// the plain '#' symbol, which is the '%#'-escaped literal hash character.
pub const TWOLC_HASH: &str = "__HFST_TWOLC_#";

// Typedefs (grammar_defs.h / OtherSymbolTransducer.h / variable_src/*):
pub type SymbolPair = StringPair;
pub type SymbolRange = Vec<Symbol>;
pub type SymbolPairVector = StringPairVector;
pub type OtherSymbolTransducerVector<B> = Vec<OtherSymbolTransducer<B>>;
pub type VariableValueMap = BTreeMap<String, String>;
pub type RuleCenterPair = (String, String);

// [spec:hfst:def:variable-defs.matcher]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Matcher {
    FREELY,
    MATCHED,
    MIXED,
}

// [spec:hfst:def:variable-defs.empty-container]
#[derive(Clone, Copy, Debug)]
pub struct EmptyContainer;

// [spec:hfst:def:other-symbol-transducer.other-symbol-transducer]
pub struct OtherSymbolTransducer<B: AlgebraBackend> {
    pub(crate) is_broken: bool,
    pub(crate) transducer: HfstTransducer<B>,
}
// per-compile config replacing the OtherSymbolTransducer statics (the C++
// 'transducer_type' static is the backend type parameter 'B' now); `pub` to
// match the `pub` OtherSymbolTransducer/Rule methods that thread it (twolc is a
// `pub mod`).
pub struct OstConfig {
    pub(crate) input_symbols: BTreeSet<Symbol>,
    pub(crate) output_symbols: BTreeSet<Symbol>,
    pub(crate) diacritics: BTreeSet<Symbol>,
    pub(crate) symbol_pairs: BTreeSet<SymbolPair>,
}

// Rule hierarchy — one struct per C++ subclass, summed by the closed
// ['TwolcRule'] enum below (the former 'RuleT' trait / 'Box<dyn RuleT>').
// [spec:hfst:def:rule.rule]
pub struct Rule<B: AlgebraBackend> {
    pub(crate) is_empty: bool,
    pub(crate) name: String,
    pub(crate) center: OtherSymbolTransducer<B>,
    pub(crate) context: OtherSymbolTransducer<B>,
    pub(crate) rule_transducer: OtherSymbolTransducer<B>,
}
pub struct ResultRule<B: AlgebraBackend> {
    pub(crate) base: Rule<B>,
}
// [spec:hfst:def:right-arrow-rule.right-arrow-rule]
pub struct RightArrowRule<B: AlgebraBackend> {
    pub(crate) base: Rule<B>,
}
// [spec:hfst:def:left-arrow-rule.left-arrow-rule]
pub struct LeftArrowRule<B: AlgebraBackend> {
    pub(crate) base: Rule<B>,
}
// [spec:hfst:def:left-restriction-arrow-rule.left-restriction-arrow-rule]
pub struct LeftRestrictionArrowRule<B: AlgebraBackend> {
    pub(crate) base: Rule<B>,
}
// [spec:hfst:def:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule]
pub struct ConflictResolvingRightArrowRule<B: AlgebraBackend> {
    pub(crate) base: RightArrowRule<B>,
    pub(crate) center_pair: SymbolPair,
}
// [spec:hfst:def:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule]
pub struct ConflictResolvingLeftArrowRule<B: AlgebraBackend> {
    pub(crate) base: LeftArrowRule<B>,
    pub(crate) input_symbol: Symbol,
}

/// The closed sum over the six concrete rule kinds. The C++ dispatched
/// virtually over 'Rule*'; the implementor set is closed (exactly these six
/// classes) but 'rule_vector' is genuinely heterogeneous, so — like the
/// facade's one remaining runtime sum at the stream boundary
/// ([dec:hfst:monomorphic-backends]) — the trait object becomes a closed enum
/// and every former virtual call a match-delegation.
pub enum TwolcRule<B: AlgebraBackend> {
    Result(ResultRule<B>),
    RightArrow(RightArrowRule<B>),
    LeftArrow(LeftArrowRule<B>),
    LeftRestrictionArrow(LeftRestrictionArrowRule<B>),
    ConflictResolvingRightArrow(ConflictResolvingRightArrowRule<B>),
    ConflictResolvingLeftArrow(ConflictResolvingLeftArrowRule<B>),
}

impl<B: AlgebraBackend> TwolcRule<B> {
    /// The former 'RuleT::compile' virtual call.
    // [spec:hfst:def:rule.rule.compile-fn]
    pub fn compile(&mut self, cfg: &OstConfig) -> crate::error::Result<OtherSymbolTransducer<B>> {
        match self {
            // A 'ResultRule' (from 'Rule::new_from_vector') compiles like the
            // C++ base 'Rule::compile()': to an empty OtherSymbolTransducer.
            // [spec:hfst:sem:rule.rule.compile-fn]
            TwolcRule::Result(_) => OtherSymbolTransducer::new(cfg),
            TwolcRule::RightArrow(r) => r.compile(cfg),
            TwolcRule::LeftArrow(r) => r.compile(cfg),
            TwolcRule::LeftRestrictionArrow(r) => r.compile(cfg),
            TwolcRule::ConflictResolvingRightArrow(r) => r.compile(cfg),
            TwolcRule::ConflictResolvingLeftArrow(r) => r.compile(cfg),
        }
    }

    /// The shared 'Rule' base data (the former 'RuleT::rule' accessor).
    pub fn rule(&self) -> &Rule<B> {
        match self {
            TwolcRule::Result(r) => &r.base,
            TwolcRule::RightArrow(r) => &r.base,
            TwolcRule::LeftArrow(r) => &r.base,
            TwolcRule::LeftRestrictionArrow(r) => &r.base,
            TwolcRule::ConflictResolvingRightArrow(r) => &r.base.base,
            TwolcRule::ConflictResolvingLeftArrow(r) => &r.base.base,
        }
    }

    /// The former 'RuleT::rule_mut' accessor.
    pub fn rule_mut(&mut self) -> &mut Rule<B> {
        match self {
            TwolcRule::Result(r) => &mut r.base,
            TwolcRule::RightArrow(r) => &mut r.base,
            TwolcRule::LeftArrow(r) => &mut r.base,
            TwolcRule::LeftRestrictionArrow(r) => &mut r.base,
            TwolcRule::ConflictResolvingRightArrow(r) => &mut r.base.base,
            TwolcRule::ConflictResolvingLeftArrow(r) => &mut r.base.base,
        }
    }

    /// The former 'RuleT::rule_transducer' default method.
    pub fn rule_transducer(&self) -> &OtherSymbolTransducer<B> {
        &self.rule().rule_transducer
    }
}

// Containers:
// [spec:hfst:def:rule-container.rule-container]
pub struct RuleContainer<B: AlgebraBackend> {
    pub(crate) report: bool,
    pub(crate) rule_vector: Vec<TwolcRule<B>>,
}
// [spec:hfst:def:right-arrow-rule-container.right-arrow-rule-container]
pub struct RightArrowRuleContainer<B: AlgebraBackend> {
    pub(crate) base: RuleContainer<B>,
    pub(crate) report_right_arrow_conflicts: bool,
    pub(crate) resolve_right_arrow_conflicts: bool,
    pub(crate) center_to_rule_map: BTreeMap<SymbolPair, usize>,
}
// [spec:hfst:def:left-arrow-rule-container.left-arrow-rule-container]
pub struct LeftArrowRuleContainer<B: AlgebraBackend> {
    pub(crate) base: RuleContainer<B>,
    pub(crate) report_left_arrow_conflicts: bool,
    pub(crate) resolve_left_arrow_conflicts: bool,
    pub(crate) input_to_rule_map: BTreeMap<Symbol, Vec<usize>>,
}

// TwolCGrammar + handles:
// [spec:hfst:def:twol-c-grammar.op.operator]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Operator {
    RIGHT,
    LEFT,
    NOT_LEFT,
    LEFT_RIGHT,
    RE_RIGHT,
    RE_LEFT,
    RE_NOT_LEFT,
    RE_LEFT_RIGHT,
}
// [spec:hfst:def:twol-c-grammar.twol-c-grammar]
pub struct TwolCGrammar<B: AlgebraBackend> {
    pub(crate) be_quiet: bool,
    pub(crate) be_verbose: bool,
    pub(crate) name_to_rule_subcases: BTreeMap<String, BTreeSet<RuleHandle>>,
    pub(crate) left_arrow_rule_container: LeftArrowRuleContainer<B>,
    pub(crate) right_arrow_rule_container: RightArrowRuleContainer<B>,
    pub(crate) other_rule_container: RuleContainer<B>,
    pub(crate) compiled_rule_container: RuleContainer<B>,
    pub(crate) diacritics: SymbolRange,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuleContainerKind {
    Left,
    Right,
    Other,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuleHandle {
    pub(crate) container: RuleContainerKind,
    pub(crate) index: usize,
}

// Variable expansion (variable_src/*):
// [spec:hfst:def:rule-variables.rule-variables]
pub struct RuleVariables {
    pub(crate) freely_blocks: Vec<VariableBlockValues>,
    pub(crate) matched_blocks: Vec<VariableBlockValues>,
    pub(crate) mixed_blocks: Vec<VariableBlockValues>,
    pub(crate) current_variable_block: VariableBlockValues,
}
// [spec:hfst:def:variable-values.variable-values]
#[derive(Clone, Debug)]
pub struct VariableValues {
    pub(crate) variable: String,
    pub(crate) values: Vec<String>,
}
pub type VariableBlockValues = Vec<VariableValues>;
// (RuleVariablesConstIterator is defined with the variable-expansion port in
// 'rule_variables'.)
// [spec:hfst:def:rule-symbol-vector.rule-symbol-vector]
pub struct RuleSymbolVector {
    pub(crate) symbols: Vec<String>,
}
pub(crate) fn matcher_from_var_matcher(m: VarMatcher) -> Matcher {
    match m {
        VarMatcher::Freely => Matcher::FREELY,
        VarMatcher::Matched => Matcher::MATCHED,
        VarMatcher::Mixed => Matcher::MIXED,
    }
}

// TwolcCompiler — entry point (the output format 'ImplementationType' field is
// the backend type parameter 'B' now):
// [spec:hfst:def:twolc-compiler.hfst.twolc.twolc-compiler]
pub struct TwolcCompiler<B: AlgebraBackend> {
    pub(crate) silent: bool,
    pub(crate) verbose: bool,
    pub(crate) resolve_left_conflicts: bool,
    pub(crate) resolve_right_conflicts: bool,
    pub(crate) sets: BTreeMap<Symbol, SymbolRange>,
    pub(crate) definitions: BTreeMap<Symbol, OtherSymbolTransducer<B>>,
    /// The twolc source currently being compiled, retained so source-level
    /// diagnostics can render the offending snippet (ariadne). Empty until
    /// `compile`/`build_grammar` runs.
    pub(crate) source: String,
    /// Label shown in diagnostics for `source` (a file name, or `"<twolc>"`).
    pub(crate) source_name: String,
    /// Where each set's name is written in the `Sets` section, so a diagnostic
    /// about a set can point at its definition.
    pub(crate) set_spans: BTreeMap<Symbol, std::ops::Range<usize>>,
}

// (followed by the full ~190-line doc roster of method/helper signatures with
//  their spec def ids — see the file.)

mod compiler;
mod diagnostics;
mod eval;
mod grammar;
mod other_symbol_apply;
mod other_symbol_transducer;
mod rule_variables;
mod rules;

pub use compiler::{CenterEval, ConcreteRule};
use diagnostics::{PairSite, PairUse};
pub use rule_variables::RuleVariablesConstIterator;
pub use rules::{get_wb_fst, replace_substr, unescape_name, wbize};
