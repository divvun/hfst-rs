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

// The nfst-twolc AST rule node is renamed to keep the name 'TwolcRule' free
// for the closed rule sum below (the former 'Box<dyn RuleT>').
#[allow(unused_imports)]
use nfst_twolc::{
    AlphabetPair, RuleCenter, RuleContext, RuleOp, SetDefinition, Spanned, TwolcDefinition,
    TwolcFile, TwolcRegex, TwolcRule as AstTwolcRule, VarMatcher, VariableAssignment,
    VariableBlock,
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
            TwolcRule::Result(r) => r.compile(cfg),
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
// (RuleVariablesConstIterator is defined with the variable-expansion port below.)
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
    pub(crate) sets: BTreeMap<String, SymbolRange>,
    pub(crate) definitions: BTreeMap<String, OtherSymbolTransducer<B>>,
    /// The twolc source currently being compiled, retained so source-level
    /// diagnostics can render the offending snippet (ariadne). Empty until
    /// `compile`/`build_grammar` runs.
    pub(crate) source: String,
    /// Label shown in diagnostics for `source` (a file name, or `"<twolc>"`).
    pub(crate) source_name: String,
    /// Byte span in `source` of the top-level item currently being walked,
    /// updated as `build_grammar` visits each spanned AST node; the anchor for
    /// `diag_error`/`diag_warning`.
    pub(crate) current_span: std::ops::Range<usize>,
}

// (followed by the full ~190-line doc roster of method/helper signatures with
//  their spec def ids — see the file.)

// ===== body 0 (flattened, module scope) =====
// ───────────────────────────────────────────────────────────────────────────
// OtherSymbolTransducer — 'rule_src/OtherSymbolTransducer.{h,cc}' bodies.
//
// The C++ class kept its alphabet config in five 'static' members; this port
// folds them into a single per-compile 'OstConfig', created by
// 'TwolcCompiler::compile' and threaded by '&OstConfig' (reads) / '&mut
// OstConfig' (the three config setters) through the rule/grammar walk. This
// removes the previous 'OST_CONFIG' thread-global mutable state.
// ───────────────────────────────────────────────────────────────────────────

use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_data_types::implementations::HfstState;

impl Default for OstConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl OstConfig {
    pub(crate) fn new() -> Self {
        OstConfig {
            input_symbols: BTreeSet::new(),
            output_symbols: BTreeSet::new(),
            diacritics: BTreeSet::new(),
            symbol_pairs: BTreeSet::new(),
        }
    }
}

impl<B: AlgebraBackend> OtherSymbolTransducer<B> {
    // -------------------------------------------------------------------------
    // ----- Static config (writes the thread-local OstConfig) -----
    // -------------------------------------------------------------------------

    /// 'static void set_symbol_pairs(const HandySet<SymbolPair> &symbol_pairs)'.
    ///
    /// Clears the three derived sets, re-inserts the supplied pairs, splits them
    /// into the input/output symbol sets, and finally adds the
    /// '(TWOLC_DIAMOND, TWOLC_DIAMOND)' pair.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.set-symbol-pairs-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.set-symbol-pairs-fn]
    pub fn set_symbol_pairs(cfg: &mut OstConfig, symbol_pairs: &BTreeSet<SymbolPair>) {
        cfg.input_symbols.clear();
        cfg.output_symbols.clear();
        cfg.symbol_pairs.clear();
        for it in symbol_pairs.iter() {
            cfg.symbol_pairs.insert(it.clone());
        }
        for it in symbol_pairs.iter() {
            cfg.input_symbols.insert(it.0.clone());
            cfg.output_symbols.insert(it.1.clone());
        }
        cfg.symbol_pairs.insert((
            Symbol::new_static(TWOLC_DIAMOND),
            Symbol::new_static(TWOLC_DIAMOND),
        ));
    }

    /// 'static void define_diacritics(const std::vector<std::string> &diacritics)'.
    ///
    /// Records the diacritics and erases their identity / 'X:0' pairs and the
    /// matching input/output symbols from the alphabet config.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.define-diacritics-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.define-diacritics-fn]
    pub fn define_diacritics(cfg: &mut OstConfig, diacritics: &[Symbol]) {
        cfg.diacritics.clear();
        for d in diacritics.iter() {
            cfg.diacritics.insert(d.clone());
        }
        // Iterate over a snapshot of the diacritics so the mutable erases
        // below do not alias the loop (the C++ iterates 'diacritics' while
        // mutating 'symbol_pairs'/'input_symbols'/'output_symbols').
        let diac: Vec<Symbol> = cfg.diacritics.iter().cloned().collect();
        for it in diac.iter() {
            cfg.symbol_pairs.remove(&(it.clone(), it.clone()));
            cfg.symbol_pairs
                .remove(&(it.clone(), Symbol::new_static(TWOLC_EPSILON)));
            cfg.input_symbols.remove(it);
            cfg.output_symbols.remove(it);
        }
    }

    // ('static void set_transducer_type(ImplementationType transducer_type)'
    //  and the 'transducer_type' config read are gone: the transducer type is
    //  the backend type parameter 'B' — [dec:hfst:monomorphic-backends].)
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.set-transducer-type-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.set-transducer-type-fn]

    // -------------------------------------------------------------------------
    // ----- Constructors -----
    // -------------------------------------------------------------------------

    /// 'OtherSymbolTransducer(void)' — empty transducer of the configured type.
    pub fn new(_cfg: &OstConfig) -> crate::error::Result<Self> {
        Ok(OtherSymbolTransducer {
            is_broken: false,
            transducer: HfstTransducer::new(),
        })
    }

    /// 'OtherSymbolTransducer(const std::string &i_symbol,
    ///  const std::string &o_symbol)' — build 'input_symbol:output_symbol'.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.other-symbol-transducer-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.other-symbol-transducer-fn]
    pub fn new_pair(cfg: &OstConfig, i_symbol: &str, o_symbol: &str) -> crate::error::Result<Self> {
        let mut input_symbol = Symbol::new(i_symbol);
        let mut output_symbol = Symbol::new(o_symbol);

        if input_symbol == TWOLC_UNKNOWN {
            input_symbol = Symbol::new_static(HFST_UNKNOWN);
        }
        if output_symbol == TWOLC_UNKNOWN {
            output_symbol = Symbol::new_static(HFST_UNKNOWN);
        }

        let mut this = OtherSymbolTransducer {
            is_broken: false,
            transducer: HfstTransducer::new(),
        };
        this.check_pair(cfg, &input_symbol, &output_symbol);
        if this.is_broken {
            return Ok(this);
        }
        if input_symbol == HFST_UNKNOWN && output_symbol == HFST_UNKNOWN {
            this.transducer = Self::get_universal(cfg)?.transducer;
        } else {
            let mut fst = HfstBasicTransducer::from_transducer(&this.transducer);
            let target = fst.add_state_new();
            fst.set_final_weight(target, &0.0);

            if input_symbol == HFST_UNKNOWN {
                let input_symbols = cfg.input_symbols.iter().cloned().collect::<Vec<_>>();
                let pairs = cfg.symbol_pairs.clone();
                for it in input_symbols.iter() {
                    if pairs.contains(&(it.clone(), output_symbol.clone())) {
                        let tr = HfstBasicTransition::new_symbols(
                            target,
                            it.clone(),
                            output_symbol.clone(),
                            0.0,
                            fst.coder_mut(),
                        );
                        fst.add_transition(0, &tr, true);
                    }
                }
            } else if output_symbol == HFST_UNKNOWN {
                let output_symbols = cfg.output_symbols.iter().cloned().collect::<Vec<_>>();
                let pairs = cfg.symbol_pairs.clone();
                for it in output_symbols.iter() {
                    if pairs.contains(&(input_symbol.clone(), it.clone())) {
                        let tr = HfstBasicTransition::new_symbols(
                            target,
                            input_symbol.clone(),
                            it.clone(),
                            0.0,
                            fst.coder_mut(),
                        );
                        fst.add_transition(0, &tr, true);
                    }
                }
            } else {
                let tr = HfstBasicTransition::new_symbols(
                    target,
                    input_symbol.clone(),
                    output_symbol.clone(),
                    0.0,
                    fst.coder_mut(),
                );
                fst.add_transition(0, &tr, true);
            }
            this.transducer = HfstTransducer::new_from_basic(&fst)?;
        }
        Ok(this)
    }

    /// 'OtherSymbolTransducer(const std::string &sym)' — build 'symbol:symbol'
    /// (or 'symbol:0' for a diacritic).
    pub fn new_symbol(cfg: &OstConfig, sym: &str) -> crate::error::Result<Self> {
        let mut symbol = Symbol::new(sym);
        if symbol == TWOLC_UNKNOWN {
            symbol = Symbol::new_static(HFST_UNKNOWN);
        }

        let is_diacritic = cfg.diacritics.contains(&symbol);

        let mut this = OtherSymbolTransducer {
            is_broken: false,
            transducer: HfstTransducer::new(),
        };
        if is_diacritic {
            this.check_pair(cfg, &symbol, TWOLC_EPSILON);
        } else {
            this.check_pair(cfg, &symbol, &symbol);
        }

        if this.is_broken {
            return Ok(this);
        }

        if symbol == HFST_UNKNOWN {
            this.transducer = Self::get_universal(cfg)?.transducer;
        } else if is_diacritic {
            this.transducer = HfstTransducer::new_symbol_pair(&symbol, TWOLC_EPSILON)?;
        } else {
            this.transducer = HfstTransducer::new_symbol(&symbol)?;
        }
        Ok(this)
    }

    // -------------------------------------------------------------------------
    // ----- Protected helpers -----
    // -------------------------------------------------------------------------

    /// 'void check_pair(const std::string &input_symbol,
    ///  const std::string &output_symbol)' — set 'is_broken' if the pair is not
    /// in the configured alphabet (and report to stderr).
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.check-pair-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.check-pair-fn]
    pub fn check_pair(&mut self, cfg: &OstConfig, input_symbol: &str, output_symbol: &str) {
        {
            // id:id is a valid pair.
            if input_symbol == TWOLC_IDENTITY {
                self.is_broken = false;
            }
            // other:other is a valid pair.
            else if input_symbol == HFST_UNKNOWN && output_symbol == HFST_UNKNOWN {
                self.is_broken = false;
            }
            // eps:eps is valid
            else if input_symbol == TWOLC_EPSILON && output_symbol == TWOLC_EPSILON {
                self.is_broken = false;
            }
            // 0:0 is a valid pair.
            else if input_symbol == HFST_EPSILON && output_symbol == HFST_EPSILON {
                self.is_broken = false;
            }
            // diamond:diamond is a valid pair.
            else if input_symbol == TWOLC_DIAMOND {
                self.is_broken = false;
            }
            // other:X is valid, iff X is an output symbol or 0.
            else if input_symbol == HFST_UNKNOWN {
                self.is_broken =
                    !(output_symbol == TWOLC_EPSILON || cfg.output_symbols.contains(output_symbol));
            }
            // X:other is valid, iff X is an input symbol or 0.
            else if output_symbol == HFST_UNKNOWN {
                self.is_broken =
                    !(input_symbol == TWOLC_EPSILON || cfg.input_symbols.contains(input_symbol));
            }
            // 0:X is valid, iff X is an output symbol.
            else if input_symbol == TWOLC_EPSILON {
                self.is_broken = !cfg.output_symbols.contains(output_symbol);
            }
            // X:0 is valid, iff X is an input symbol or a diacritic.
            else if output_symbol == TWOLC_EPSILON {
                self.is_broken = !cfg.input_symbols.contains(input_symbol);
            }
            // X:X is valid if X is a diacritic.
            else if cfg.diacritics.contains(input_symbol) {
                self.is_broken = false;
            }
            // X:Y is valid iff it has been declared in the alphabet.
            else {
                self.is_broken = !cfg
                    .symbol_pairs
                    .contains(&(Symbol::new(input_symbol), Symbol::new(output_symbol)));
            }
        }
        if self.is_broken {
            error!("Unknown pair: {} {}", input_symbol, output_symbol);
        }
    }

    /// 'void add_diamond_transition(void)' — 'add_symbol_to_alphabet(DIAMOND)'.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.add-diamond-transition-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.add-diamond-transition-fn]
    pub fn add_diamond_transition(&mut self, cfg: &OstConfig) -> crate::error::Result<()> {
        self.add_symbol_to_alphabet(cfg, TWOLC_DIAMOND)?;
        Ok(())
    }

    /// 'static bool empty(const HfstBasicTransducer &fsm)' — true iff no
    /// reachable final state (the C++ scans every state for a final marker).
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.empty-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.empty-fn]
    pub fn empty(fsm: &HfstBasicTransducer) -> bool {
        for (state, _) in fsm.iter().enumerate() {
            let state = state as HfstState;
            if fsm.is_final_state(state) {
                return false;
            }
        }
        true
    }

    // -------------------------------------------------------------------------
    // ----- apply() family (member-fn-ptr dispatch flattened) -----
    //
    // Each guards on empty symbol_pairs / is_broken, applies the op, then
    // minimizes (exactly as the C++ 'CALL_MEMBER_FN(...); minimize();').
    // -------------------------------------------------------------------------

    /// True iff the configured symbol-pair set is empty.
    fn config_symbol_pairs_empty(cfg: &OstConfig) -> bool {
        cfg.symbol_pairs.is_empty()
    }

    /// True iff there are configured diacritics.
    fn config_has_diacritics(cfg: &OstConfig) -> bool {
        !cfg.diacritics.is_empty()
    }

    /// 'apply(HfstTransducerZeroArgMember p)' — apply a zero-arg 'HfstTransducer'
    /// op then minimize. The C++ member-fn-pointer becomes a closure.
    pub fn apply_zero<F>(&mut self, cfg: &OstConfig, p: F) -> crate::error::Result<&mut Self>
    where
        F: FnOnce(&mut HfstTransducer<B>) -> crate::error::Result<()>,
    {
        if Self::config_symbol_pairs_empty(cfg) {
            crate::bail!(EmptySymbolPairSet);
        }
        if self.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        p(&mut self.transducer)?;
        self.transducer.minimize()?;
        Ok(self)
    }

    /// 'apply(const HfstTransducerOneArgMember, const OtherSymbolTransducer&)'.
    ///
    /// Harmonizes the diacritics of '*this' and a copy of 'another' (when there
    /// are diacritics), applies the binary op against the copy, then minimizes.
    /// The C++ facade ops default to 'harmonize = true', which the closure
    /// passes through.
    pub fn apply_one<F>(
        &mut self,
        cfg: &OstConfig,
        p: F,
        another: &OtherSymbolTransducer<B>,
    ) -> crate::error::Result<&mut Self>
    where
        F: FnOnce(&mut HfstTransducer<B>, &HfstTransducer<B>) -> crate::error::Result<()>,
    {
        if Self::config_symbol_pairs_empty(cfg) {
            crate::bail!(EmptySymbolPairSet);
        }
        if self.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        if another.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        // [spec:hfst:def:other-symbol-transducer.another-copy-fn]
        // [spec:hfst:sem:other-symbol-transducer.another-copy-fn]
        let mut another_copy = another.clone();
        if Self::config_has_diacritics(cfg) {
            self.harmonize_diacritics(cfg, &mut another_copy);
            another_copy.harmonize_diacritics(cfg, self);
        }
        p(&mut self.transducer, &another_copy.transducer)?;
        self.transducer.minimize()?;
        Ok(self)
    }

    /// 'apply(const HfstTransducerBoolArgMember, const OtherSymbolTransducer&)'.
    ///
    /// Like ['apply_one'] but the closure carries the trailing 'bool' (the C++
    /// passes 'true').
    pub fn apply_one_bool<F>(
        &mut self,
        cfg: &OstConfig,
        p: F,
        another: &OtherSymbolTransducer<B>,
    ) -> crate::error::Result<&mut Self>
    where
        F: FnOnce(&mut HfstTransducer<B>, &HfstTransducer<B>, bool) -> crate::error::Result<()>,
    {
        if Self::config_symbol_pairs_empty(cfg) {
            crate::bail!(EmptySymbolPairSet);
        }
        if self.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        if another.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        let mut another_copy = another.clone();
        if Self::config_has_diacritics(cfg) {
            self.harmonize_diacritics(cfg, &mut another_copy);
            another_copy.harmonize_diacritics(cfg, self);
        }
        p(&mut self.transducer, &another_copy.transducer, true)?;
        self.transducer.minimize()?;
        Ok(self)
    }

    /// 'bool apply(const HfstTransducerOneArgMemberBool,
    ///  const OtherSymbolTransducer&) const'.
    ///
    /// Runs the predicate against copies of both transducers and returns its
    /// result (no minimize; the C++ overload is 'const').
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.apply-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.apply-fn]
    pub fn apply_one_bool_ret<F>(
        &self,
        cfg: &OstConfig,
        p: F,
        another: &OtherSymbolTransducer<B>,
    ) -> crate::error::Result<bool>
    where
        F: FnOnce(&mut HfstTransducer<B>, &HfstTransducer<B>) -> bool,
    {
        if Self::config_symbol_pairs_empty(cfg) {
            crate::bail!(EmptySymbolPairSet);
        }
        if self.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        if another.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        let mut copy = self.clone();
        let another_copy = another.clone();
        Ok(p(&mut copy.transducer, &another_copy.transducer))
    }

    /// 'apply(const HfstTransducerOneNumArgMember, unsigned int number)'.
    pub fn apply_num<F>(
        &mut self,
        cfg: &OstConfig,
        p: F,
        number: u32,
    ) -> crate::error::Result<&mut Self>
    where
        F: FnOnce(&mut HfstTransducer<B>, u32) -> crate::error::Result<()>,
    {
        if Self::config_symbol_pairs_empty(cfg) {
            crate::bail!(EmptySymbolPairSet);
        }
        if self.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        p(&mut self.transducer, number)?;
        self.transducer.minimize()?;
        Ok(self)
    }

    /// 'apply(const HfstTransducerTwoNumArgMember, unsigned int, unsigned int)'.
    pub fn apply_two_num<F>(
        &mut self,
        cfg: &OstConfig,
        p: F,
        num1: u32,
        num2: u32,
    ) -> crate::error::Result<&mut Self>
    where
        F: FnOnce(&mut HfstTransducer<B>, u32, u32) -> crate::error::Result<()>,
    {
        if Self::config_symbol_pairs_empty(cfg) {
            crate::bail!(EmptySymbolPairSet);
        }
        if self.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        p(&mut self.transducer, num1, num2)?;
        self.transducer.minimize()?;
        Ok(self)
    }

    /// 'apply(const HfstTransducerOneSymbolPairArgMember, const SymbolPair&)'.
    pub fn apply_symbol_pair<F>(
        &mut self,
        cfg: &OstConfig,
        p: F,
        pair: &SymbolPair,
    ) -> crate::error::Result<&mut Self>
    where
        F: FnOnce(&mut HfstTransducer<B>, &SymbolPair) -> crate::error::Result<()>,
    {
        if Self::config_symbol_pairs_empty(cfg) {
            crate::bail!(EmptySymbolPairSet);
        }
        if self.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        p(&mut self.transducer, pair)?;
        self.transducer.minimize()?;
        Ok(self)
    }

    /// 'apply(const HfstTransducerOneSymbolPairBoolArgMember,
    ///  const SymbolPair&, bool)'.
    pub fn apply_symbol_pair_bool<F>(
        &mut self,
        cfg: &OstConfig,
        p: F,
        pair: &SymbolPair,
        b: bool,
    ) -> crate::error::Result<&mut Self>
    where
        F: FnOnce(&mut HfstTransducer<B>, &SymbolPair, bool) -> crate::error::Result<()>,
    {
        if Self::config_symbol_pairs_empty(cfg) {
            crate::bail!(EmptySymbolPairSet);
        }
        if self.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        p(&mut self.transducer, pair, b)?;
        self.transducer.minimize()?;
        Ok(self)
    }

    /// 'apply(const HfstTransducerSubstMember, const std::string&,
    ///  const std::string&, bool, bool)' — 'substitute(str1, str2, b1, b2)'.
    pub fn apply_subst(
        &mut self,
        cfg: &OstConfig,
        str1: &str,
        str2: &str,
        b1: bool,
        b2: bool,
    ) -> crate::error::Result<&mut Self> {
        if Self::config_symbol_pairs_empty(cfg) {
            crate::bail!(EmptySymbolPairSet);
        }
        if self.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        self.transducer.substitute_string(str1, str2, b1, b2)?;
        self.transducer.minimize()?;
        Ok(self)
    }

    /// 'apply(const HfstTransducerSubstPairMember, const SymbolPair&,
    ///  const SymbolPair&)' — 'substitute(pair1, pair2)'.
    pub fn apply_subst_pair(
        &mut self,
        cfg: &OstConfig,
        p1: &SymbolPair,
        p2: &SymbolPair,
    ) -> crate::error::Result<&mut Self> {
        if Self::config_symbol_pairs_empty(cfg) {
            crate::bail!(EmptySymbolPairSet);
        }
        if self.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        self.transducer.substitute_pair_with_pair(p1, p2)?;
        self.transducer.minimize()?;
        Ok(self)
    }

    /// 'apply(const HfstTransducerSubstPairFstMember, const SymbolPair&,
    ///  const OtherSymbolTransducer&, bool)' —
    /// 'substitute(pair, t_copy.transducer, b)'.
    // [spec:hfst:def:other-symbol-transducer.t-copy-fn]
    // [spec:hfst:sem:other-symbol-transducer.t-copy-fn]
    pub fn apply_subst_pair_fst(
        &mut self,
        cfg: &OstConfig,
        p1: &SymbolPair,
        t: &OtherSymbolTransducer<B>,
        b: bool,
    ) -> crate::error::Result<&mut Self> {
        if Self::config_symbol_pairs_empty(cfg) {
            crate::bail!(EmptySymbolPairSet);
        }
        if self.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        let mut t_copy = t.clone();
        self.transducer
            .substitute_symbol_pair_with_transducer(p1, &mut t_copy.transducer, b)?;
        self.transducer.minimize()?;
        Ok(self)
    }

    // -------------------------------------------------------------------------
    // ----- Concrete convenience shims (readable call sites for the rules) -----
    // -------------------------------------------------------------------------

    /// 'apply(&HfstTransducer::disjunct, another)'.
    pub fn disjunct(
        &mut self,
        cfg: &OstConfig,
        another: &OtherSymbolTransducer<B>,
    ) -> crate::error::Result<&mut Self> {
        self.apply_one(
            cfg,
            |t, o| {
                t.disjunct(o, true)?;
                Ok(())
            },
            another,
        )
    }

    /// 'apply(&HfstTransducer::intersect, another)'.
    pub fn intersect(
        &mut self,
        cfg: &OstConfig,
        another: &OtherSymbolTransducer<B>,
    ) -> crate::error::Result<&mut Self> {
        self.apply_one(
            cfg,
            |t, o| {
                t.intersect(o, true)?;
                Ok(())
            },
            another,
        )
    }

    /// 'apply(&HfstTransducer::subtract, another)'.
    pub fn subtract(
        &mut self,
        cfg: &OstConfig,
        another: &OtherSymbolTransducer<B>,
    ) -> crate::error::Result<&mut Self> {
        self.apply_one(
            cfg,
            |t, o| {
                t.subtract(o, true)?;
                Ok(())
            },
            another,
        )
    }

    /// 'apply(&HfstTransducer::concatenate, another)'.
    pub fn concatenate(
        &mut self,
        cfg: &OstConfig,
        another: &OtherSymbolTransducer<B>,
    ) -> crate::error::Result<&mut Self> {
        self.apply_one(
            cfg,
            |t, o| {
                t.concatenate(o, true)?;
                Ok(())
            },
            another,
        )
    }

    /// 'apply(&HfstTransducer::compose, another)'.
    pub fn compose(
        &mut self,
        cfg: &OstConfig,
        another: &OtherSymbolTransducer<B>,
    ) -> crate::error::Result<&mut Self> {
        self.apply_one(
            cfg,
            |t, o| {
                t.compose(o, true)?;
                Ok(())
            },
            another,
        )
    }

    /// 'apply(&HfstTransducer::insert_freely, another)' (bool-arg overload).
    pub fn insert_freely(
        &mut self,
        cfg: &OstConfig,
        another: &OtherSymbolTransducer<B>,
    ) -> crate::error::Result<&mut Self> {
        self.apply_one_bool(
            cfg,
            |t, o, h| {
                t.insert_freely(o, h)?;
                Ok(())
            },
            another,
        )
    }

    /// 'apply(&HfstTransducer::repeat_star)'.
    pub fn repeat_star(&mut self, cfg: &OstConfig) -> crate::error::Result<&mut Self> {
        self.apply_zero(cfg, |t| {
            t.repeat_star()?;
            Ok(())
        })
    }

    /// 'apply(&HfstTransducer::minimize)'.
    pub fn minimize(&mut self, cfg: &OstConfig) -> crate::error::Result<&mut Self> {
        self.apply_zero(cfg, |t| {
            t.minimize()?;
            Ok(())
        })
    }

    /// 'apply(&HfstTransducer::optionalize)'.
    pub fn optionalize(&mut self, cfg: &OstConfig) -> crate::error::Result<&mut Self> {
        self.apply_zero(cfg, |t| {
            t.optionalize()?;
            Ok(())
        })
    }

    /// 'apply(&HfstTransducer::invert)'.
    pub fn invert(&mut self, cfg: &OstConfig) -> crate::error::Result<&mut Self> {
        self.apply_zero(cfg, |t| {
            t.invert()?;
            Ok(())
        })
    }

    /// 'apply(&HfstTransducer::input_project)'.
    pub fn input_project(&mut self, cfg: &OstConfig) -> crate::error::Result<&mut Self> {
        self.apply_zero(cfg, |t| {
            t.input_project()?;
            Ok(())
        })
    }

    /// 'apply(&HfstTransducer::output_project)'.
    pub fn output_project(&mut self, cfg: &OstConfig) -> crate::error::Result<&mut Self> {
        self.apply_zero(cfg, |t| {
            t.output_project()?;
            Ok(())
        })
    }

    /// 'apply(&HfstTransducer::repeat_n, n)'.
    pub fn repeat_n(&mut self, cfg: &OstConfig, n: u32) -> crate::error::Result<&mut Self> {
        self.apply_num(
            cfg,
            |t, n| {
                t.repeat_n(n)?;
                Ok(())
            },
            n,
        )
    }

    /// 'apply(&HfstTransducer::repeat_n_to_k, n, k)'.
    pub fn repeat_n_to_k(
        &mut self,
        cfg: &OstConfig,
        n: u32,
        k: u32,
    ) -> crate::error::Result<&mut Self> {
        self.apply_two_num(
            cfg,
            |t, n, k| {
                t.repeat_n_to_k(n, k)?;
                Ok(())
            },
            n,
            k,
        )
    }

    /// Replace the diamond pair '(DIAMOND, DIAMOND)' with the HFST epsilon on
    /// both sides: 'substitute(DIAMOND, HFST_EPSILON, true, true)'.
    pub fn substitute_diamond_to_epsilon(
        &mut self,
        cfg: &OstConfig,
    ) -> crate::error::Result<&mut Self> {
        self.apply_subst(cfg, TWOLC_DIAMOND, HFST_EPSILON, true, true)
    }

    // -------------------------------------------------------------------------
    // ----- Other instance / static ops -----
    // -------------------------------------------------------------------------

    /// 'OtherSymbolTransducer &harmonize_diacritics(OtherSymbolTransducer &t)'.
    ///
    /// For each diacritic present in 't''s alphabet but missing from '*this''s,
    /// add a 'd:d' self-loop-style transition alongside every 'TWOLC_IDENTITY'
    /// transition leaving a state.
    pub fn harmonize_diacritics(
        &mut self,
        cfg: &OstConfig,
        t: &mut OtherSymbolTransducer<B>,
    ) -> &mut Self {
        // [spec:hfst:def:other-symbol-transducer.basic-fn]
        // [spec:hfst:sem:other-symbol-transducer.basic-fn]
        let mut basic = HfstBasicTransducer::from_transducer(&self.transducer);
        let alphabet: BTreeSet<Symbol> = basic.get_alphabet().clone();

        let basic_t = HfstBasicTransducer::from_transducer(&t.transducer);
        let t_alphabet: BTreeSet<Symbol> = basic_t.get_alphabet().clone();

        let mut missing_diacritics: BTreeSet<Symbol> = BTreeSet::new();
        for it in cfg.diacritics.iter() {
            if t_alphabet.contains(it) && !alphabet.contains(it) {
                missing_diacritics.insert(it.clone());
            }
        }
        if missing_diacritics.is_empty() {
            return self;
        }

        // For every state, if it has a TWOLC_IDENTITY-input transition, add a
        // diacritic self-pair transition to that transition's target for each
        // missing diacritic (the C++ 'break's after the first identity arc).
        let num_states = basic.states_and_transitions().len();
        for s in 0..num_states {
            let mut identity_target: Option<HfstState> = None;
            for jt in basic
                .index(s as HfstState)
                .expect("s is a valid state of this transducer")
                .iter()
            {
                if jt.get_input_symbol(basic.coder()) == TWOLC_IDENTITY {
                    identity_target = Some(jt.get_target_state());
                    break;
                }
            }
            if let Some(target) = identity_target {
                for kt in missing_diacritics.iter() {
                    let tr = HfstBasicTransition::new_symbols(
                        target,
                        kt.clone(),
                        kt.clone(),
                        0.0,
                        basic.coder_mut(),
                    );
                    basic.add_transition(s as HfstState, &tr, true);
                }
            }
        }
        self.transducer = HfstTransducer::new_from_basic(&basic)
            .expect("constructing a transducer from a valid basic transducer cannot fail");
        self
    }

    /// 'static OtherSymbolTransducer get_context(OtherSymbolTransducer &left,
    ///  OtherSymbolTransducer &right)' — build '?* X D ?* D Y ?*'.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.get-context-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.get-context-fn]
    pub fn get_context(
        cfg: &OstConfig,
        left: &mut OtherSymbolTransducer<B>,
        right: &mut OtherSymbolTransducer<B>,
    ) -> crate::error::Result<OtherSymbolTransducer<B>> {
        let mut universal = Self::get_universal(cfg)?;
        universal.apply_zero(cfg, |t| {
            t.repeat_star()?;
            Ok(())
        })?;
        let mut result = universal.clone();
        let diamond = OtherSymbolTransducer::new_symbol(cfg, TWOLC_DIAMOND)?;
        universal.apply_zero(cfg, |t| {
            t.repeat_star()?;
            Ok(())
        })?;

        result.concatenate(cfg, left)?;
        result.concatenate(cfg, &diamond)?;
        result.concatenate(cfg, &universal)?;
        result.concatenate(cfg, &diamond)?;
        result.concatenate(cfg, right)?;
        result.concatenate(cfg, &universal)?;
        Ok(result)
    }

    /// 'static OtherSymbolTransducer get_universal(void)' — a one-symbol
    /// transducer recognizing the identity pair plus every configured pair
    /// except the diamond.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.get-universal-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.get-universal-fn]
    pub fn get_universal(cfg: &OstConfig) -> crate::error::Result<OtherSymbolTransducer<B>> {
        let universal = OtherSymbolTransducer::<B> {
            is_broken: false,
            transducer: HfstTransducer::new(),
        };
        let mut fst = HfstBasicTransducer::from_transducer(&universal.transducer);
        let target = fst.add_state_new();
        fst.set_final_weight(target, &0.0);
        let tr = HfstBasicTransition::new_symbols(
            target,
            Symbol::new_static(TWOLC_IDENTITY),
            Symbol::new_static(TWOLC_IDENTITY),
            0.0,
            fst.coder_mut(),
        );
        fst.add_transition(0, &tr, true);
        let pairs = cfg.symbol_pairs.clone();
        for it in pairs.iter() {
            if it.0 == TWOLC_DIAMOND {
                continue;
            }
            let tr = HfstBasicTransition::new_symbols(
                target,
                it.0.clone(),
                it.1.clone(),
                0.0,
                fst.coder_mut(),
            );
            fst.add_transition(0, &tr, true);
        }
        Ok(OtherSymbolTransducer {
            is_broken: false,
            transducer: HfstTransducer::new_from_basic(&fst)?,
        })
    }

    /// 'void add_symbol_to_alphabet(const std::string &symbol)' — round-trip
    /// through the basic transducer to add 'symbol' (prevents harmonization).
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.add-symbol-to-alphabet-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.add-symbol-to-alphabet-fn]
    pub fn add_symbol_to_alphabet(
        &mut self,
        _cfg: &OstConfig,
        symbol: &str,
    ) -> crate::error::Result<()> {
        let mut mutable_transducer = HfstBasicTransducer::from_transducer(&self.transducer);
        mutable_transducer.add_symbol_to_alphabet(&Symbol::new(symbol));
        self.transducer = HfstTransducer::new_from_basic(&mutable_transducer)?;
        Ok(())
    }

    /// 'void remove_diacritics_from_output(void)' — for each diacritic, rewrite
    /// 'd:d' to 'd:0'.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.remove-diacritics-from-output-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.remove-diacritics-from-output-fn]
    pub fn remove_diacritics_from_output(&mut self, cfg: &OstConfig) -> crate::error::Result<()> {
        let diac = cfg.diacritics.iter().cloned().collect::<Vec<_>>();
        for it in diac.iter() {
            self.apply_subst_pair(
                cfg,
                &(it.clone(), it.clone()),
                &(it.clone(), Symbol::new_static(TWOLC_EPSILON)),
            )?;
        }
        Ok(())
    }

    /// 'OtherSymbolTransducer &add_info_symbol(const std::string &info_symbol)'
    /// — append 'info_symbol' to the wrapped transducer's name.
    pub fn add_info_symbol(&mut self, info_symbol: &str) -> crate::error::Result<&mut Self> {
        if self.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        let mut name = self.transducer.get_name();
        if !name.is_empty() {
            name += " & ";
        }
        name += info_symbol;
        self.transducer.set_name(&name);
        Ok(self)
    }

    /// 'static void add_transition(HfstBasicTransducer &center_t,
    ///  size_t source, size_t target, const std::string &input,
    ///  const std::string &output)'.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.add-transition-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.add-transition-fn]
    pub fn add_transition(
        center_t: &mut HfstBasicTransducer,
        source_state: usize,
        target_state: usize,
        input: &str,
        output: &str,
    ) {
        let tr = HfstBasicTransition::new_symbols(
            target_state as HfstState,
            Symbol::new(input),
            Symbol::new(output),
            0.0,
            center_t.coder_mut(),
        );
        center_t.add_transition(source_state as HfstState, &tr, true);
    }

    /// 'static bool has_symbol(const HfstBasicTransducer &t,
    ///  const std::string &sym)'.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.has-symbol-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.has-symbol-fn]
    pub fn has_symbol(t: &HfstBasicTransducer, sym: &str) -> bool {
        t.get_alphabet().contains(sym)
    }

    /// 'static void set_final(HfstBasicTransducer &center_t, size_t state)'.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.set-final-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.set-final-fn]
    pub fn set_final(center_t: &mut HfstBasicTransducer, state: usize) {
        center_t.set_final_weight(state as HfstState, &0.0);
    }

    /// 'OtherSymbolTransducer get_inverse_of_upper_projection(void)' — a copy in
    /// which every output symbol is replaced by an other-symbol (abstracts a
    /// center to its input side).
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.get-inverse-of-upper-projection-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.get-inverse-of-upper-projection-fn]
    pub fn get_inverse_of_upper_projection(
        &self,
        cfg: &OstConfig,
    ) -> crate::error::Result<OtherSymbolTransducer<B>> {
        if self.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        let fst = HfstBasicTransducer::from_transducer(&self.transducer);
        let mut new_fst = HfstBasicTransducer::new();

        let output_symbols = cfg.output_symbols.iter().cloned().collect::<Vec<_>>();
        let symbol_pairs = cfg.symbol_pairs.clone();

        let num_states = fst.states_and_transitions().len();
        for state in 0..num_states {
            let st = state as HfstState;
            new_fst.add_state(st);
            if fst.is_final_state(st) {
                let w = fst.get_final_weight(st)?;
                new_fst.set_final_weight(st, &w);
            }
            for jt in fst.index(st)?.iter() {
                let input = jt.get_transition_data().get_input_symbol(fst.coder());
                let output = jt.get_transition_data().get_output_symbol(fst.coder());
                let target = jt.get_target_state();
                if input == HFST_UNKNOWN {
                    Self::add_transition(
                        &mut new_fst,
                        state,
                        target as usize,
                        HFST_UNKNOWN,
                        HFST_UNKNOWN,
                    );
                    for kt in output_symbols.iter() {
                        if Self::has_symbol(&fst, kt) {
                            Self::add_transition(
                                &mut new_fst,
                                state,
                                target as usize,
                                HFST_UNKNOWN,
                                kt,
                            );
                        }
                    }
                } else {
                    Self::add_transition(&mut new_fst, state, target as usize, &input, &output);
                    for kt in symbol_pairs.iter() {
                        if kt.0 == input && Self::has_symbol(&fst, &kt.1) {
                            Self::add_transition(
                                &mut new_fst,
                                state,
                                target as usize,
                                &input,
                                &kt.1,
                            );
                        }
                    }
                    if input == TWOLC_EPSILON {
                        Self::add_transition(
                            &mut new_fst,
                            state,
                            target as usize,
                            HFST_EPSILON,
                            HFST_EPSILON,
                        );
                        Self::add_transition(
                            &mut new_fst,
                            state,
                            target as usize,
                            TWOLC_EPSILON,
                            HFST_UNKNOWN,
                        );
                    } else if input != TWOLC_DIAMOND {
                        Self::add_transition(
                            &mut new_fst,
                            state,
                            target as usize,
                            &input,
                            HFST_UNKNOWN,
                        );
                    }
                }
            }
        }
        let mut copy = self.clone();
        copy.transducer = HfstTransducer::new_from_basic(&new_fst)?;
        copy.apply_zero(cfg, |t| {
            t.minimize()?;
            Ok(())
        })?;
        Ok(copy)
    }

    /// 'OtherSymbolTransducer &contained(void)' — '?* X ?*'.
    pub fn contained(&mut self, cfg: &OstConfig) -> crate::error::Result<&mut Self> {
        // [spec:hfst:def:other-symbol-transducer.universal-fn]
        // [spec:hfst:sem:other-symbol-transducer.universal-fn]
        let mut universal = OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?;
        universal.apply_zero(cfg, |t| {
            t.repeat_star()?;
            Ok(())
        })?;
        let mut result = universal.clone();
        result.concatenate(cfg, self)?;
        result.concatenate(cfg, &universal)?;
        *self = result;
        Ok(self)
    }

    /// 'OtherSymbolTransducer &contained_once(void)' —
    /// '?* X ?* - ?* X ?* X ?*'.
    pub fn contained_once(&mut self, cfg: &OstConfig) -> crate::error::Result<&mut Self> {
        let mut universal = OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?;
        universal.apply_zero(cfg, |t| {
            t.repeat_star()?;
            Ok(())
        })?;
        let mut result1 = universal.clone();
        result1.concatenate(cfg, self)?;
        result1.concatenate(cfg, &universal)?;
        let mut result2 = universal.clone();
        result2.concatenate(cfg, self)?;
        result2.concatenate(cfg, &universal)?;
        result2.concatenate(cfg, self)?;
        result2.concatenate(cfg, &universal)?;
        result1.subtract(cfg, &result2)?;
        *self = result1;
        Ok(self)
    }

    /// 'OtherSymbolTransducer &negated(void)' — '?* - X'.
    pub fn negated(&mut self, cfg: &OstConfig) -> crate::error::Result<&mut Self> {
        let mut universal = OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?;
        universal.apply_zero(cfg, |t| {
            t.repeat_star()?;
            Ok(())
        })?;
        universal.subtract(cfg, self)?;
        *self = universal;
        Ok(self)
    }

    /// 'OtherSymbolTransducer &term_complemented(void)' — '? - X'.
    pub fn term_complemented(&mut self, cfg: &OstConfig) -> crate::error::Result<&mut Self> {
        let mut universal = OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?;
        universal.subtract(cfg, self)?;
        *self = universal;
        Ok(self)
    }

    /// 'HfstTransducer get_transducer(void) const'.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.get-transducer-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.get-transducer-fn]
    pub fn get_transducer(&self) -> crate::error::Result<HfstTransducer<B>> {
        if self.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        HfstTransducer::new_copy(&self.transducer)
    }

    /// 'void get_initial_transition_pairs(SymbolPairVector &pair_container)
    ///  const' — collect the symbol pairs on the transitions leaving the start
    /// state.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.get-initial-transition-pairs-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.get-initial-transition-pairs-fn]
    pub fn get_initial_transition_pairs(&self) -> crate::error::Result<SymbolPairVector> {
        if self.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        let mut pair_container = SymbolPairVector::new();
        let fst = HfstBasicTransducer::from_transducer(&self.transducer);
        for jt in fst
            .index(0)
            .expect("s is a valid state of this transducer")
            .iter()
        {
            let input = jt.get_transition_data().get_input_symbol(fst.coder());
            let output = jt.get_transition_data().get_output_symbol(fst.coder());
            pair_container.push((input, output));
        }
        Ok(pair_container)
    }

    /// 'bool is_empty_intersection(const OtherSymbolTransducer &another,
    ///  StringVector &v)' — true iff '*this' and 'another' share no string;
    /// when non-empty, the first common string is stored in 'v'.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.is-empty-intersection-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.is-empty-intersection-fn]
    pub fn is_empty_intersection(
        &self,
        another: &OtherSymbolTransducer<B>,
        v: &mut StringVector,
    ) -> bool {
        let this_fst = HfstBasicTransducer::from_transducer(&self.transducer);
        let another_fst = HfstBasicTransducer::from_transducer(&another.transducer);
        let mut visited_pairs: BTreeSet<(HfstState, HfstState)> = BTreeSet::new();
        visited_pairs.insert((0, 0));
        !have_common_string(0, 0, &this_fst, &another_fst, &mut visited_pairs, v)
    }

    /// 'bool is_subset(const OtherSymbolTransducer &another)' — true iff
    /// 'another' is a subset of '*this' (computed as 'another - *this' empty).
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.is-subset-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.is-subset-fn]
    pub fn is_subset(
        &self,
        cfg: &OstConfig,
        another: &OtherSymbolTransducer<B>,
    ) -> crate::error::Result<bool> {
        // Do this properly later.. (preserved C++ comment.)
        let mut another_fst = another.clone();
        another_fst.subtract(cfg, self)?;
        let internal = HfstBasicTransducer::from_transducer(&another_fst.get_transducer()?);
        Ok(Self::empty(&internal))
    }

    /// 'bool is_empty(void) const' — true iff the wrapped transducer has no
    /// reachable final state.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.is-empty-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.is-empty-fn]
    pub fn is_empty(&self) -> bool {
        Self::empty(&HfstBasicTransducer::from_transducer(&self.transducer))
    }
}

/// 'OtherSymbolTransducer(const OtherSymbolTransducer &another)' /
/// 'operator=' — copy the 'is_broken' flag and the wrapped transducer.
impl<B: AlgebraBackend> Clone for OtherSymbolTransducer<B> {
    fn clone(&self) -> Self {
        OtherSymbolTransducer {
            is_broken: self.is_broken,
            transducer: HfstTransducer::new_copy(&self.transducer)
                .expect("copying a valid transducer cannot fail"),
        }
    }
}

/// 'bool have_common_string(HfstState state1, HfstState state2,
///  const HfstBasicTransducer &fst1, const HfstBasicTransducer &fst2,
///  HandySet<StatePair> &visited_pairs, StringVector &v)' — depth-first search
/// for a string accepted by both transducers, recording the path in 'v'.
// [spec:hfst:def:other-symbol-transducer.have-common-string-fn]
// [spec:hfst:sem:other-symbol-transducer.have-common-string-fn]
fn have_common_string(
    state1: HfstState,
    state2: HfstState,
    fst1: &HfstBasicTransducer,
    fst2: &HfstBasicTransducer,
    visited_pairs: &mut BTreeSet<(HfstState, HfstState)>,
    v: &mut StringVector,
) -> bool {
    if fst1.is_final_state(state1) && fst2.is_final_state(state2) {
        return true;
    }

    let fst1_transitions = fst1
        .index(state1)
        .expect("s is a valid state of this transducer");
    let fst2_transitions = fst2
        .index(state2)
        .expect("s is a valid state of this transducer");

    let mut fst1_transition_map: BTreeMap<SymbolPair, HfstState> = BTreeMap::new();
    for it in fst1_transitions.iter() {
        fst1_transition_map.insert(
            (
                it.get_input_symbol(fst1.coder()),
                it.get_output_symbol(fst1.coder()),
            ),
            it.get_target_state(),
        );
    }

    for it in fst2_transitions.iter() {
        let symbol_pair: SymbolPair = (
            it.get_input_symbol(fst2.coder()),
            it.get_output_symbol(fst2.coder()),
        );
        if let Some(&fst1_target) = fst1_transition_map.get(&symbol_pair) {
            let state_pair: (HfstState, HfstState) = (fst1_target, it.get_target_state());
            if !visited_pairs.contains(&state_pair) {
                v.push(Symbol::from(format!("{}:{}", symbol_pair.0, symbol_pair.1)));
                visited_pairs.insert(state_pair);
                if have_common_string(state_pair.0, state_pair.1, fst1, fst2, visited_pairs, v) {
                    return true;
                } else {
                    v.pop();
                }
            }
        }
    }
    false
}

// ===== body 1 (flattened, module scope) =====
// ===========================================================================
// rule_src/Rule.{h,cc} — Rule base data + non-virtual methods.
// ===========================================================================

impl<B: AlgebraBackend> Rule<B> {
    /// 'Rule::Rule(name, center, contexts)' ('Rule.cc'). Disjuncts all
    /// 'contexts' into 'context', then harmonizes the center's diacritics
    /// against the disjuncted context.
    // [spec:hfst:def:rule.rule.rule-fn]
    // [spec:hfst:sem:rule.rule.rule-fn]
    pub fn new(
        cfg: &OstConfig,
        name: &str,
        center: OtherSymbolTransducer<B>,
        contexts: &OtherSymbolTransducerVector<B>,
    ) -> crate::error::Result<Rule<B>> {
        let mut rule = Rule {
            is_empty: false,
            name: unescape_name(name),
            center,
            context: OtherSymbolTransducer::new(cfg)?,
            rule_transducer: OtherSymbolTransducer::new(cfg)?,
        };
        // OtherSymbolTransducerVector contexts_copy = contexts;
        // for (it : contexts_copy) context.apply(disjunct, *it);
        for ctx in contexts.iter() {
            rule.context.disjunct(cfg, ctx)?;
        }
        // this->center.harmonize_diacritics(cfg, context);
        let mut context = std::mem::replace(&mut rule.context, OtherSymbolTransducer::new(cfg)?);
        rule.center.harmonize_diacritics(cfg, &mut context);
        rule.context = context;
        Ok(rule)
    }

    /// 'Rule::Rule(name, RuleVector)' ('Rule.cc') — the intersecting result
    /// constructor. Builds 'rule_transducer = ?*' then intersects the
    /// 'rule_transducer' of each non-empty subcase rule. Produces a
    /// ['ResultRule'] whose 'compile()' is the base no-op.
    // [spec:hfst:def:rule.rule.rule-fn]
    // [spec:hfst:sem:rule.rule.rule-fn]
    pub fn new_from_vector(
        cfg: &OstConfig,
        name: &str,
        v: &[&TwolcRule<B>],
    ) -> crate::error::Result<ResultRule<B>> {
        let mut rule_transducer = OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?;
        rule_transducer.repeat_star(cfg)?;
        let mut is_empty = true;
        for r in v.iter() {
            if !r.rule().empty() {
                rule_transducer.intersect(cfg, r.rule_transducer())?;
                is_empty = false;
            }
        }
        Ok(ResultRule {
            base: Rule {
                is_empty,
                name: unescape_name(name),
                center: OtherSymbolTransducer::new(cfg)?,
                context: OtherSymbolTransducer::new(cfg)?,
                rule_transducer,
            },
        })
    }

    /// 'Rule::empty()' ('Rule.cc'). True when conflict resolution merged this
    /// rule into another (or the intersecting ctor found no non-empty subcase).
    // [spec:hfst:def:rule.rule.empty-fn]
    // [spec:hfst:sem:rule.rule.empty-fn]
    pub fn empty(&self) -> bool {
        self.is_empty
    }

    /// 'Rule::store(out)' ('Rule.cc'). Names the rule, maps the internal TWOLC
    /// symbols back to their HFST-facing forms, and writes the rule transducer to
    /// the binary 'HfstOutputStream'.
    // [spec:hfst:def:rule.rule.store-fn]
    // [spec:hfst:sem:rule.rule.store-fn]
    pub fn store(
        &mut self,
        cfg: &OstConfig,
        out: &mut crate::hfst_output_stream::HfstOutputStream,
    ) -> crate::error::Result<()> {
        if self.is_empty {
            return Ok(());
        }
        self.add_name()?;
        self.rule_transducer.remove_diacritics_from_output(cfg)?;
        self.rule_transducer
            .apply_subst(cfg, TWOLC_EPSILON, HFST_EPSILON, true, true)?;
        self.rule_transducer
            .apply_subst(cfg, "__HFST_TWOLC_.#.", "@#@", true, true)?;
        self.rule_transducer
            .apply_subst(cfg, "__HFST_TWOLC_SPACE", " ", true, true)?;
        self.rule_transducer.apply_subst_pair(
            cfg,
            &(Symbol::new_static("@#@"), Symbol::new_static("@#@")),
            &(Symbol::new_static("@#@"), Symbol::new_static(HFST_EPSILON)),
        )?;
        self.rule_transducer
            .apply_subst(cfg, TWOLC_IDENTITY, HFST_IDENTITY, true, true)?;
        out.redirect(&mut self.rule_transducer.transducer)?;
        Ok(())
    }

    /// 'Rule::get_name()' ('Rule.cc').
    // [spec:hfst:def:rule.rule.get-name-fn]
    // [spec:hfst:sem:rule.rule.get-name-fn]
    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    /// 'Rule::add_name()' ('Rule.cc'). Adds the rule name as an info symbol on
    /// 'rule_transducer'.
    // [spec:hfst:def:rule.rule.add-name-fn]
    // [spec:hfst:sem:rule.rule.add-name-fn]
    pub fn add_name(&mut self) -> crate::error::Result<()> {
        let name = self.name.clone();
        self.rule_transducer.add_info_symbol(&name)?;
        Ok(())
    }

    /// 'Rule::get_print_name(s)' ('Rule.cc'). Strips the '__HFST_TWOLC_*'
    /// markers for human-readable display: '__HFST_TWOLC_SPACE' and
    /// '__HFST_TWOLC_RULE_NAME=' become a space, '__HFST_TWOLC_SET_NAME='
    /// and the bare '__HFST_TWOLC_' prefix become empty.
    // [spec:hfst:def:rule.rule.get-print-name-fn]
    // [spec:hfst:sem:rule.rule.get-print-name-fn]
    pub fn get_print_name(s: &str) -> String {
        let mut ss = s.to_string();
        ss = replace_substr(&ss, "__HFST_TWOLC_SPACE", " ");
        ss = replace_substr(&ss, "__HFST_TWOLC_RULE_NAME=", " ");
        ss = replace_substr(&ss, "__HFST_TWOLC_SET_NAME=", "");
        ss = replace_substr(&ss, "__HFST_TWOLC_", "");
        ss
    }

    /// 'Rule::get_universal_language_with_diamonds(cfg, )' ('Rule.cc'). Returns
    /// '?* <D> ?* <D> ?*'.
    // [spec:hfst:def:rule.rule.get-universal-language-with-diamonds-fn]
    // [spec:hfst:sem:rule.rule.get-universal-language-with-diamonds-fn]
    pub fn get_universal_language_with_diamonds(
        cfg: &OstConfig,
    ) -> crate::error::Result<OtherSymbolTransducer<B>> {
        let mut universal = OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?;
        universal.repeat_star(cfg)?;
        let diamond = OtherSymbolTransducer::new_symbol(cfg, TWOLC_DIAMOND)?;
        let mut universal_with_diamonds = universal.clone();
        universal_with_diamonds.concatenate(cfg, &diamond)?;
        universal_with_diamonds.concatenate(cfg, &universal)?;
        universal_with_diamonds.concatenate(cfg, &diamond)?;
        universal_with_diamonds.concatenate(cfg, &universal)?;
        Ok(universal_with_diamonds)
    }

    /// 'Rule::get_center(input, output)' ('Rule.cc'). Returns
    /// '?* <D> input:output <D> ?*'.
    // [spec:hfst:def:rule.rule.get-center-fn]
    // [spec:hfst:sem:rule.rule.get-center-fn]
    pub fn get_center_io(
        cfg: &OstConfig,
        input: &str,
        output: &str,
    ) -> crate::error::Result<OtherSymbolTransducer<B>> {
        let mut unknown = OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?;
        unknown.repeat_star(cfg)?;
        let diamond = OtherSymbolTransducer::new_symbol(cfg, TWOLC_DIAMOND)?;
        let mut center = unknown.clone();
        let center_pair = OtherSymbolTransducer::new_pair(cfg, input, output)?;
        center.concatenate(cfg, &diamond)?;
        center.concatenate(cfg, &center_pair)?;
        center.concatenate(cfg, &diamond)?;
        center.concatenate(cfg, &unknown)?;
        Ok(center)
    }

    /// 'Rule::get_center(v: SymbolPairVector)' ('Rule.cc'). Returns
    /// '?* <D> (disjunction of pairs) <D> ?*'.
    // [spec:hfst:def:rule.rule.get-center-fn]
    // [spec:hfst:sem:rule.rule.get-center-fn]
    pub fn get_center_pairs(
        cfg: &OstConfig,
        v: &SymbolPairVector,
    ) -> crate::error::Result<OtherSymbolTransducer<B>> {
        let mut unknown = OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?;
        unknown.repeat_star(cfg)?;
        let diamond = OtherSymbolTransducer::new_symbol(cfg, TWOLC_DIAMOND)?;
        let mut center_pair_transducer = OtherSymbolTransducer::new(cfg)?;
        for pair in v.iter() {
            let p = OtherSymbolTransducer::new_pair(cfg, &pair.0, &pair.1)?;
            center_pair_transducer.disjunct(cfg, &p)?;
        }
        let mut center = unknown.clone();
        center.concatenate(cfg, &diamond)?;
        center.concatenate(cfg, &center_pair_transducer)?;
        center.concatenate(cfg, &diamond)?;
        center.concatenate(cfg, &unknown)?;
        Ok(center)
    }

    /// 'Rule::get_center(restricted_center)' ('Rule.cc'). Returns
    /// '?* <D> restricted_center <D> ?*'.
    // [spec:hfst:def:rule.rule.get-center-fn]
    // [spec:hfst:sem:rule.rule.get-center-fn]
    pub fn get_center_restricted(
        cfg: &OstConfig,
        restricted_center: &OtherSymbolTransducer<B>,
    ) -> crate::error::Result<OtherSymbolTransducer<B>> {
        let mut unknown = OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?;
        unknown.repeat_star(cfg)?;
        let diamond = OtherSymbolTransducer::new_symbol(cfg, TWOLC_DIAMOND)?;
        let mut center = unknown.clone();
        center.concatenate(cfg, &diamond)?;
        center.concatenate(cfg, restricted_center)?;
        center.concatenate(cfg, &diamond)?;
        center.concatenate(cfg, &unknown)?;
        Ok(center)
    }

    /// 'Rule::add_missing_symbols_freely(diacritics)' ('Rule.cc'). For every
    /// diacritic that is not already in 'rule_transducer''s alphabet, add it to
    /// the alphabet and insert the diacritic-pair freely.
    // [spec:hfst:def:rule.rule.add-missing-symbols-freely-fn]
    // [spec:hfst:sem:rule.rule.add-missing-symbols-freely-fn]
    pub fn add_missing_symbols_freely(
        &mut self,
        cfg: &OstConfig,
        diacritics: &SymbolRange,
    ) -> crate::error::Result<()> {
        let symbol_set: BTreeSet<Symbol> = self.rule_transducer.get_transducer()?.get_alphabet()?;
        for d in diacritics.iter() {
            if !symbol_set.contains(d) {
                self.rule_transducer.add_symbol_to_alphabet(cfg, d)?;
                self.rule_transducer.apply_symbol_pair(
                    cfg,
                    |t, p| {
                        t.insert_freely_pair(p, false)?;
                        Ok(())
                    },
                    &(d.clone(), d.clone()),
                )?;
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ResultRule — produced by 'Rule::new_from_vector'; its compile() is a no-op
// (the C++ base 'Rule::compile()' returns an empty OtherSymbolTransducer).
// ---------------------------------------------------------------------------

impl<B: AlgebraBackend> ResultRule<B> {
    // [spec:hfst:def:rule.rule.compile-fn]
    // [spec:hfst:sem:rule.rule.compile-fn]
    pub fn compile(&mut self, cfg: &OstConfig) -> crate::error::Result<OtherSymbolTransducer<B>> {
        OtherSymbolTransducer::new(cfg)
    }
}

// ===========================================================================
// rule_src/RightArrowRule.{h,cc} — '=>' rule.
// ===========================================================================

impl<B: AlgebraBackend> RightArrowRule<B> {
    /// 'RightArrowRule::RightArrowRule(name, center, contexts)'
    /// ('RightArrowRule.cc'). Delegates to the 'Rule' base constructor.
    // [spec:hfst:def:right-arrow-rule.right-arrow-rule.right-arrow-rule-fn]
    // [spec:hfst:sem:right-arrow-rule.right-arrow-rule.right-arrow-rule-fn]
    pub fn new(
        cfg: &OstConfig,
        name: &str,
        center: OtherSymbolTransducer<B>,
        contexts: &OtherSymbolTransducerVector<B>,
    ) -> crate::error::Result<RightArrowRule<B>> {
        Ok(RightArrowRule {
            base: Rule::new(cfg, name, center, contexts)?,
        })
    }
}

impl<B: AlgebraBackend> RightArrowRule<B> {
    /// 'RightArrowRule::compile()' ('RightArrowRule.cc').
    ///
    /// '''text
    /// center.subtract(cfg, context).substitute(<D> -> 0);
    /// rule_transducer = ?* ;
    /// rule_transducer.subtract(cfg, center);
    /// '''
    ///
    /// MUTATES 'center' in place (subtract the context, then turn the diamonds
    /// into epsilon) before building 'rule_transducer = ?* - center'.
    // [spec:hfst:def:right-arrow-rule.right-arrow-rule.compile-fn]
    // [spec:hfst:sem:right-arrow-rule.right-arrow-rule.compile-fn]
    pub fn compile(&mut self, cfg: &OstConfig) -> crate::error::Result<OtherSymbolTransducer<B>> {
        let context = std::mem::replace(&mut self.base.context, OtherSymbolTransducer::new(cfg)?);
        self.base.center.subtract(cfg, &context)?;
        self.base.center.substitute_diamond_to_epsilon(cfg)?;
        self.base.context = context;

        let mut rule_transducer = OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?;
        let center = std::mem::replace(&mut self.base.center, OtherSymbolTransducer::new(cfg)?);
        rule_transducer.repeat_star(cfg)?;
        rule_transducer.subtract(cfg, &center)?;
        self.base.center = center;

        self.base.rule_transducer = rule_transducer.clone();
        Ok(rule_transducer)
    }
}

// ===========================================================================
// rule_src/LeftArrowRule.{h,cc} — '<=' rule.
// ===========================================================================

impl<B: AlgebraBackend> LeftArrowRule<B> {
    /// 'LeftArrowRule::LeftArrowRule(name, center, contexts)'
    /// ('LeftArrowRule.cc'). Delegates to the 'Rule' base constructor.
    // [spec:hfst:def:left-arrow-rule.left-arrow-rule.left-arrow-rule-fn]
    // [spec:hfst:sem:left-arrow-rule.left-arrow-rule.left-arrow-rule-fn]
    pub fn new(
        cfg: &OstConfig,
        name: &str,
        center: OtherSymbolTransducer<B>,
        contexts: &OtherSymbolTransducerVector<B>,
    ) -> crate::error::Result<LeftArrowRule<B>> {
        Ok(LeftArrowRule {
            base: Rule::new(cfg, name, center, contexts)?,
        })
    }
}

impl<B: AlgebraBackend> LeftArrowRule<B> {
    /// 'LeftArrowRule::compile()' ('LeftArrowRule.cc').
    ///
    /// '''text
    /// abstract_center = center.get_inverse_of_upper_projection(cfg);
    /// context.intersect(cfg, abstract_center);
    /// context.subtract(cfg, center);
    /// context.substitute(<D> -> 0);
    /// rule_transducer = ?* ;
    /// return rule_transducer.subtract(cfg, context);
    /// '''
    // [spec:hfst:def:left-arrow-rule.left-arrow-rule.compile-fn]
    // [spec:hfst:sem:left-arrow-rule.left-arrow-rule.compile-fn]
    pub fn compile(&mut self, cfg: &OstConfig) -> crate::error::Result<OtherSymbolTransducer<B>> {
        let abstract_center = self.base.center.get_inverse_of_upper_projection(cfg)?;
        // context.intersect(cfg, abstract_center).subtract(cfg, center).substitute(<D>->0)
        let center = std::mem::replace(&mut self.base.center, OtherSymbolTransducer::new(cfg)?);
        self.base.context.intersect(cfg, &abstract_center)?;
        self.base.context.subtract(cfg, &center)?;
        self.base.context.substitute_diamond_to_epsilon(cfg)?;
        self.base.center = center;

        let mut rule_transducer = OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?;
        let context = std::mem::replace(&mut self.base.context, OtherSymbolTransducer::new(cfg)?);
        rule_transducer.repeat_star(cfg)?;
        rule_transducer.subtract(cfg, &context)?;
        self.base.context = context;

        self.base.rule_transducer = rule_transducer.clone();
        Ok(rule_transducer)
    }
}

// ===========================================================================
// rule_src/LeftRestrictionArrowRule.{h,cc} — '/<=' rule.
// ===========================================================================

impl<B: AlgebraBackend> LeftRestrictionArrowRule<B> {
    /// 'LeftRestrictionArrowRule::LeftRestrictionArrowRule(name, center,
    /// contexts)' ('LeftRestrictionArrowRule.cc') — 'OtherSymbolTransducer'
    /// center form. Delegates to the 'Rule' base constructor.
    // [spec:hfst:def:left-restriction-arrow-rule.left-restriction-arrow-rule.left-restriction-arrow-rule-fn]
    // [spec:hfst:sem:left-restriction-arrow-rule.left-restriction-arrow-rule.left-restriction-arrow-rule-fn]
    pub fn new(
        cfg: &OstConfig,
        name: &str,
        center: OtherSymbolTransducer<B>,
        contexts: &OtherSymbolTransducerVector<B>,
    ) -> crate::error::Result<LeftRestrictionArrowRule<B>> {
        Ok(LeftRestrictionArrowRule {
            base: Rule::new(cfg, name, center, contexts)?,
        })
    }

    /// 'LeftRestrictionArrowRule::LeftRestrictionArrowRule(name, SymbolPair
    /// center, contexts)' ('LeftRestrictionArrowRule.cc') — symbol-pair center
    /// form. Builds the center via 'Rule::get_center(first, second)'.
    // [spec:hfst:def:left-restriction-arrow-rule.left-restriction-arrow-rule.left-restriction-arrow-rule-fn]
    // [spec:hfst:sem:left-restriction-arrow-rule.left-restriction-arrow-rule.left-restriction-arrow-rule-fn]
    pub fn new_pair(
        cfg: &OstConfig,
        name: &str,
        center: &SymbolPair,
        contexts: &OtherSymbolTransducerVector<B>,
    ) -> crate::error::Result<LeftRestrictionArrowRule<B>> {
        Ok(LeftRestrictionArrowRule {
            base: Rule::new(
                cfg,
                name,
                Rule::get_center_io(cfg, &center.0, &center.1)?,
                contexts,
            )?,
        })
    }
}

impl<B: AlgebraBackend> LeftRestrictionArrowRule<B> {
    /// 'LeftRestrictionArrowRule::compile()'
    /// ('LeftRestrictionArrowRule.cc').
    ///
    /// '''text
    /// center.intersect(cfg, context).substitute(<D> -> 0);
    /// rule_transducer = ?* ;
    /// rule_transducer.subtract(cfg, center);
    /// '''
    // [spec:hfst:def:left-restriction-arrow-rule.left-restriction-arrow-rule.compile-fn]
    // [spec:hfst:sem:left-restriction-arrow-rule.left-restriction-arrow-rule.compile-fn]
    pub fn compile(&mut self, cfg: &OstConfig) -> crate::error::Result<OtherSymbolTransducer<B>> {
        let context = std::mem::replace(&mut self.base.context, OtherSymbolTransducer::new(cfg)?);
        self.base.center.intersect(cfg, &context)?;
        self.base.center.substitute_diamond_to_epsilon(cfg)?;
        self.base.context = context;

        let mut rule_transducer = OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?;
        let center = std::mem::replace(&mut self.base.center, OtherSymbolTransducer::new(cfg)?);
        rule_transducer.repeat_star(cfg)?;
        rule_transducer.subtract(cfg, &center)?;
        self.base.center = center;

        self.base.rule_transducer = rule_transducer.clone();
        Ok(rule_transducer)
    }
}

// ===========================================================================
// rule_src/ConflictResolvingRightArrowRule.{h,cc} — '=>' single-pair center.
// ===========================================================================

impl<B: AlgebraBackend> ConflictResolvingRightArrowRule<B> {
    /// 'ConflictResolvingRightArrowRule::ConflictResolvingRightArrowRule(name,
    /// center, contexts)' ('ConflictResolvingRightArrowRule.cc'). Builds the
    /// 'RightArrowRule' base from 'get_center(first, second)' and records the
    /// 'center_pair'.
    // [spec:hfst:def:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule-fn]
    // [spec:hfst:sem:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule-fn]
    pub fn new(
        cfg: &OstConfig,
        name: &str,
        center: &SymbolPair,
        contexts: &OtherSymbolTransducerVector<B>,
    ) -> crate::error::Result<ConflictResolvingRightArrowRule<B>> {
        Ok(ConflictResolvingRightArrowRule {
            base: RightArrowRule::new(
                cfg,
                name,
                Rule::get_center_io(cfg, &center.0, &center.1)?,
                contexts,
            )?,
            center_pair: center.clone(),
        })
    }

    /// 'ConflictResolvingRightArrowRule::conflicts_this(another)'
    /// ('ConflictResolvingRightArrowRule.cc'). Two '=>'-rules conflict when
    /// they share the same center symbol-pair.
    // [spec:hfst:def:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule.conflicts-this-fn]
    // [spec:hfst:sem:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule.conflicts-this-fn]
    pub fn conflicts_this(&self, another: &ConflictResolvingRightArrowRule<B>) -> bool {
        self.center_pair.0 == another.center_pair.0 && self.center_pair.1 == another.center_pair.1
    }

    /// 'ConflictResolvingRightArrowRule::resolve_conflict(another)'
    /// ('ConflictResolvingRightArrowRule.cc'). Merges 'another''s context into
    /// 'this' (disjunct + minimize) and appends its name.
    // [spec:hfst:def:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule.resolve-conflict-fn]
    // [spec:hfst:sem:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule.resolve-conflict-fn]
    pub fn resolve_conflict(
        &mut self,
        cfg: &OstConfig,
        another: &ConflictResolvingRightArrowRule<B>,
    ) -> crate::error::Result<()> {
        let another_context = another.base.base.context.clone();
        self.base.base.context.disjunct(cfg, &another_context)?;
        self.base.base.context.minimize(cfg)?;
        let another_name = another.base.base.name.clone();
        self.base.base.name += " and ";
        self.base.base.name += &another_name;
        Ok(())
    }
}

impl<B: AlgebraBackend> ConflictResolvingRightArrowRule<B> {
    pub fn compile(&mut self, cfg: &OstConfig) -> crate::error::Result<OtherSymbolTransducer<B>> {
        self.base.compile(cfg)
    }
}

// ===========================================================================
// rule_src/ConflictResolvingLeftArrowRule.{h,cc} — '<=' single-pair center.
// ===========================================================================

/// 'get_wb_fst(cfg)' ('ConflictResolvingLeftArrowRule.cc'). Builds the
/// word-boundary framing transducer '.#. ((? - .#.) | <D>)* .#.'.
// [spec:hfst:def:conflict-resolving-left-arrow-rule.get-wb-fst-fn]
// [spec:hfst:sem:conflict-resolving-left-arrow-rule.get-wb-fst-fn]
pub fn get_wb_fst<B: AlgebraBackend>(
    cfg: &OstConfig,
) -> crate::error::Result<OtherSymbolTransducer<B>> {
    let wb = OtherSymbolTransducer::new_pair(cfg, "__HFST_TWOLC_.#.", "__HFST_TWOLC_.#.")?;
    let mut no_wb = OtherSymbolTransducer::new_pair(cfg, TWOLC_UNKNOWN, TWOLC_UNKNOWN)?;
    let diamond = OtherSymbolTransducer::new_pair(cfg, TWOLC_DIAMOND, TWOLC_DIAMOND)?;

    no_wb.subtract(cfg, &wb)?;
    no_wb.disjunct(cfg, &diamond)?;
    no_wb.repeat_star(cfg)?;

    let mut result = wb.clone();
    result.concatenate(cfg, &no_wb)?;
    result.concatenate(cfg, &wb)?;

    Ok(result)
}

/// 'wbize(cfg, t)' ('ConflictResolvingLeftArrowRule.cc'). Intersects 't' with the
/// word-boundary framing transducer.
// [spec:hfst:def:conflict-resolving-left-arrow-rule.wbize-fn]
// [spec:hfst:sem:conflict-resolving-left-arrow-rule.wbize-fn]
pub fn wbize<B: AlgebraBackend>(
    cfg: &OstConfig,
    t: &OtherSymbolTransducer<B>,
) -> crate::error::Result<OtherSymbolTransducer<B>> {
    let mut t_copy = t.clone();
    let wb_fst = get_wb_fst(cfg)?;
    t_copy.intersect(cfg, &wb_fst)?;
    Ok(t_copy)
}

impl<B: AlgebraBackend> ConflictResolvingLeftArrowRule<B> {
    /// 'ConflictResolvingLeftArrowRule::ConflictResolvingLeftArrowRule(name,
    /// center, contexts)' ('ConflictResolvingLeftArrowRule.cc'). Builds the
    /// 'LeftArrowRule' base from 'get_center(first, second)' and records the
    /// center's input symbol.
    // [spec:hfst:def:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule-fn]
    // [spec:hfst:sem:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule-fn]
    pub fn new(
        cfg: &OstConfig,
        name: &str,
        center: &SymbolPair,
        contexts: &OtherSymbolTransducerVector<B>,
    ) -> crate::error::Result<ConflictResolvingLeftArrowRule<B>> {
        Ok(ConflictResolvingLeftArrowRule {
            base: LeftArrowRule::new(
                cfg,
                name,
                Rule::get_center_io(cfg, &center.0, &center.1)?,
                contexts,
            )?,
            input_symbol: center.0.clone(),
        })
    }

    /// 'ConflictResolvingLeftArrowRule::conflicts_this(another, v)'
    /// ('ConflictResolvingLeftArrowRule.cc'). True when 'this''s context has a
    /// non-empty intersection with the word-boundary-framed context of
    /// 'another' (the conflicting string is stored in 'v').
    // [spec:hfst:def:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.conflicts-this-fn]
    // [spec:hfst:sem:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.conflicts-this-fn]
    pub fn conflicts_this(
        &self,
        cfg: &OstConfig,
        another: &ConflictResolvingLeftArrowRule<B>,
        v: &mut StringVector,
    ) -> crate::error::Result<bool> {
        Ok(!self
            .base
            .base
            .context
            .is_empty_intersection(&wbize(cfg, &another.base.base.context)?, v))
    }

    /// 'ConflictResolvingLeftArrowRule::resolvable_conflict(another)'
    /// ('ConflictResolvingLeftArrowRule.cc'). True when 'this''s context is a
    /// sub-language of the word-boundary-framed context of 'another'.
    // [spec:hfst:def:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.resolvable-conflict-fn]
    // [spec:hfst:sem:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.resolvable-conflict-fn]
    pub fn resolvable_conflict(
        &self,
        cfg: &OstConfig,
        another: &ConflictResolvingLeftArrowRule<B>,
    ) -> crate::error::Result<bool> {
        self.base
            .base
            .context
            .is_subset(cfg, &wbize(cfg, &another.base.base.context)?)
    }

    /// 'ConflictResolvingLeftArrowRule::resolve_conflict(another)'
    /// ('ConflictResolvingLeftArrowRule.cc'). Resolves by subtracting
    /// 'another''s context from 'this''s context.
    // [spec:hfst:def:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.resolve-conflict-fn]
    // [spec:hfst:sem:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.resolve-conflict-fn]
    pub fn resolve_conflict(
        &mut self,
        cfg: &OstConfig,
        another: &ConflictResolvingLeftArrowRule<B>,
    ) -> crate::error::Result<()> {
        let another_context = another.base.base.context.clone();
        self.base.base.context.subtract(cfg, &another_context)?;
        Ok(())
    }
}

impl<B: AlgebraBackend> ConflictResolvingLeftArrowRule<B> {
    pub fn compile(&mut self, cfg: &OstConfig) -> crate::error::Result<OtherSymbolTransducer<B>> {
        self.base.compile(cfg)
    }
}

// ===== body 2 (flattened, module scope) =====
// ───────────────────────────────────────────────────────────────────────────
// Rule containers (rule_src/RuleContainer.cc, RightArrowRuleContainer.cc,
// LeftArrowRuleContainer.cc).
//
// The C++ containers held 'std::vector<Rule*>' and 'delete'd the pointers in
// the destructor; here every rule is OWNED as a ['TwolcRule'] enum value in
// 'rule_vector', so vector drop replaces the deleting destructor. The C++ maps
// stored 'Rule*' keyed by center-pair / input-symbol; here they store INDICES
// into the owning 'rule_vector' (a 'Rule*' replacement that survives the
// borrow checker because the rules are owned in one place).
//
// Conflict resolution touches only the 'Rule' base data ('context'/'name'),
// reachable through the ['TwolcRule'] 'rule()'/'rule_mut()' accessors, plus
// the free 'wbize' helper; so the container code never needs to match the
// enum back to its concrete conflict-resolving variant.
// ───────────────────────────────────────────────────────────────────────────

// C++ '~RuleContainer' iterates 'rule_vector' and 'delete's every 'Rule*'.
// Here each rule is owned as a ['TwolcRule'] in 'rule_vector', so dropping
// 'rule_vector' is the deleting destructor.
// [spec:hfst:def:rule-container.rule-container.rule-container-fn]
// [spec:hfst:sem:rule-container.rule-container.rule-container-fn]
// [spec:hfst:def:rule-container.rule-container]
impl<B: AlgebraBackend> RuleContainer<B> {
    /// C++ 'RuleContainer::RuleContainer(void): report(true) {}'.
    pub fn new() -> Self {
        RuleContainer {
            report: true,
            rule_vector: Vec::new(),
        }
    }

    // [spec:hfst:def:rule-container.rule-container.add-rule-fn]
    // [spec:hfst:sem:rule-container.rule-container.add-rule-fn]
    //
    // C++ 'rule_vector.push_back(rule)'. Returns the index of the stored rule
    // (the 'Rule*' replacement used by the grammar's subcase handles).
    pub fn add_rule(&mut self, rule: TwolcRule<B>) -> usize {
        self.rule_vector.push(rule);
        self.rule_vector.len() - 1
    }

    // [spec:hfst:def:rule-container.rule-container.compile-fn]
    // [spec:hfst:sem:rule-container.rule-container.compile-fn]
    //
    // C++ iterates 'rule_vector', optionally prints the print-name and calls
    // '(*it)->compile()'. The verbose message is sent to stderr (the C++
    // 'msg_out' is always 'std::cerr' at the call sites).
    pub fn compile(&mut self, cfg: &OstConfig, be_verbose: bool) -> crate::error::Result<()> {
        for rule in self.rule_vector.iter_mut() {
            if be_verbose {
                debug!("Compiling {}", Rule::<B>::get_print_name(&rule.rule().name));
            }
            rule.compile(cfg)?;
        }
        Ok(())
    }

    // [spec:hfst:def:rule-container.rule-container.store-fn]
    // [spec:hfst:sem:rule-container.rule-container.store-fn]
    //
    // C++ takes (HfstOutputStream &out, std::ostream &msg_out, bool be_verbose);
    // the progress messages go to stderr here (as elsewhere in this port).
    pub fn store(
        &mut self,
        cfg: &OstConfig,
        out: &mut crate::hfst_output_stream::HfstOutputStream,
        be_verbose: bool,
    ) -> crate::error::Result<()> {
        for rule in self.rule_vector.iter_mut() {
            if be_verbose {
                let name = Rule::<B>::get_print_name(&rule.rule().get_name());
                debug!("Storing {name}");
            }
            rule.rule_mut().store(cfg, out)?;
        }
        Ok(())
    }

    // [spec:hfst:def:rule-container.rule-container.add-missing-symbols-freely-fn]
    // [spec:hfst:sem:rule-container.rule-container.add-missing-symbols-freely-fn]
    pub fn add_missing_symbols_freely(
        &mut self,
        cfg: &OstConfig,
        diacritics: &SymbolRange,
    ) -> crate::error::Result<()> {
        for rule in self.rule_vector.iter_mut() {
            rule.rule_mut()
                .add_missing_symbols_freely(cfg, diacritics)?;
        }
        Ok(())
    }

    /// Borrow the rule at 'index' (the 'Rule*' deref the grammar performs when
    /// intersecting subcases by handle).
    pub(crate) fn rule_ref(&self, index: usize) -> &TwolcRule<B> {
        &self.rule_vector[index]
    }
}

impl<B: AlgebraBackend> Default for RuleContainer<B> {
    fn default() -> Self {
        RuleContainer::new()
    }
}

// [spec:hfst:def:right-arrow-rule-container.right-arrow-rule-container]
impl<B: AlgebraBackend> RightArrowRuleContainer<B> {
    /// C++ default state: 'report_right_arrow_conflicts = true',
    /// 'resolve_right_arrow_conflicts = true' (file-scope static initialisers).
    pub fn new() -> Self {
        RightArrowRuleContainer {
            base: RuleContainer::new(),
            report_right_arrow_conflicts: true,
            resolve_right_arrow_conflicts: true,
            center_to_rule_map: BTreeMap::new(),
        }
    }

    // [spec:hfst:def:right-arrow-rule-container.right-arrow-rule-container.set-report-right-arrow-conflicts-fn]
    // [spec:hfst:sem:right-arrow-rule-container.right-arrow-rule-container.set-report-right-arrow-conflicts-fn]
    pub fn set_report_right_arrow_conflicts(&mut self, option: bool) {
        self.report_right_arrow_conflicts = option;
    }

    // [spec:hfst:def:right-arrow-rule-container.right-arrow-rule-container.set-resolve-right-arrow-conflicts-fn]
    // [spec:hfst:sem:right-arrow-rule-container.right-arrow-rule-container.set-resolve-right-arrow-conflicts-fn]
    pub fn set_resolve_right_arrow_conflicts(&mut self, option: bool) {
        self.resolve_right_arrow_conflicts = option;
    }

    // [spec:hfst:def:right-arrow-rule-container.right-arrow-rule-container.add-rule-and-display-and-resolve-conflicts-fn]
    // [spec:hfst:sem:right-arrow-rule-container.right-arrow-rule-container.add-rule-and-display-and-resolve-conflicts-fn]
    //
    // C++ keyed conflict on 'center_pair': if the map already holds a rule with
    // the same center-pair, the rules conflict (the map lookup IS the
    // 'conflicts_this' test). When resolving, the EXISTING rule's contexts are
    // joined with the incoming rule's ('disjunct' + 'minimize'), the incoming
    // rule's name is appended to the existing rule's name, and the incoming
    // rule is marked 'is_empty' (so the subcase intersection skips it). The
    // incoming rule is still pushed into 'rule_vector' here — unlike C++, which
    // leaks it — so the grammar's subcase handle for it stays valid; an empty
    // rule contributes nothing to the intersection.
    //
    // The conflict-resolution itself ('disjunct'/'minimize' of 'context', name
    // append) is the 'ConflictResolvingRightArrowRule::resolve_conflict' body,
    // applied here through the base 'Rule' data so the owned ['TwolcRule']
    // need not be matched back to its concrete variant.
    pub fn add_rule_and_display_and_resolve_conflicts(
        &mut self,
        cfg: &OstConfig,
        mut rule: ConflictResolvingRightArrowRule<B>,
    ) -> crate::error::Result<usize> {
        let center_pair = rule.center_pair.clone();
        if let Some(&existing_index) = self.center_to_rule_map.get(&center_pair) {
            if self.report_right_arrow_conflicts {
                let existing_name = self.base.rule_vector[existing_index].rule().name.clone();
                let incoming_name = rule.base.base.name.clone();
                warn!(
                    "There is a =>-rule conflict between {} and {}.\nResolving the conflict by joining contexts.",
                    Rule::<B>::get_print_name(&existing_name),
                    Rule::<B>::get_print_name(&incoming_name)
                );
            }

            if self.resolve_right_arrow_conflicts {
                // ConflictResolvingRightArrowRule::resolve_conflict:
                //   existing.context.disjunct(cfg, incoming.context).minimize(cfg);
                //   existing.name += " and " + incoming.name;
                let incoming_context = clone_ost(&rule.base.base.context);
                let incoming_name = rule.base.base.name.clone();
                {
                    let existing = self.base.rule_vector[existing_index].rule_mut();
                    existing.context.disjunct(cfg, &incoming_context)?;
                    existing.context.minimize(cfg)?;
                    existing.name = format!("{} and {}", existing.name, incoming_name);
                }
                rule.base.base.is_empty = true;
                Ok(self
                    .base
                    .add_rule(TwolcRule::ConflictResolvingRightArrow(rule)))
            } else {
                Ok(self
                    .base
                    .add_rule(TwolcRule::ConflictResolvingRightArrow(rule)))
            }
        } else {
            let index = self
                .base
                .add_rule(TwolcRule::ConflictResolvingRightArrow(rule));
            self.center_to_rule_map.insert(center_pair, index);
            Ok(index)
        }
    }

    /// C++ 'RuleContainer::compile' forwarded through the base member.
    pub fn compile(&mut self, cfg: &OstConfig, be_verbose: bool) -> crate::error::Result<()> {
        self.base.compile(cfg, be_verbose)?;
        Ok(())
    }

    pub(crate) fn rule_ref(&self, index: usize) -> &TwolcRule<B> {
        self.base.rule_ref(index)
    }
}

impl<B: AlgebraBackend> Default for RightArrowRuleContainer<B> {
    fn default() -> Self {
        RightArrowRuleContainer::new()
    }
}

// [spec:hfst:def:left-arrow-rule-container.left-arrow-rule-container]
impl<B: AlgebraBackend> LeftArrowRuleContainer<B> {
    /// C++ default state: 'resolve_left_arrow_conflicts = false',
    /// 'report_left_arrow_conflicts = false' (file-scope static initialisers).
    pub fn new() -> Self {
        LeftArrowRuleContainer {
            base: RuleContainer::new(),
            report_left_arrow_conflicts: false,
            resolve_left_arrow_conflicts: false,
            input_to_rule_map: BTreeMap::new(),
        }
    }

    // [spec:hfst:def:left-arrow-rule-container.left-arrow-rule-container.set-resolve-left-arrow-conflicts-fn]
    // [spec:hfst:sem:left-arrow-rule-container.left-arrow-rule-container.set-resolve-left-arrow-conflicts-fn]
    pub fn set_resolve_left_arrow_conflicts(&mut self, option: bool) {
        self.resolve_left_arrow_conflicts = option;
    }

    // [spec:hfst:def:left-arrow-rule-container.left-arrow-rule-container.set-report-left-arrow-conflicts-fn]
    // [spec:hfst:sem:left-arrow-rule-container.left-arrow-rule-container.set-report-left-arrow-conflicts-fn]
    pub fn set_report_left_arrow_conflicts(&mut self, option: bool) {
        self.report_left_arrow_conflicts = option;
    }

    // [spec:hfst:def:left-arrow-rule-container.left-arrow-rule-container.add-rule-and-display-and-resolve-conflicts-fn]
    // [spec:hfst:sem:left-arrow-rule-container.left-arrow-rule-container.add-rule-and-display-and-resolve-conflicts-fn]
    //
    // C++ groups '<='-rules by their center input symbol. For every previously
    // added rule sharing the input symbol, if the incoming rule conflicts
    // ('(*it)->conflicts_this(*rule, ctx)' — a non-empty intersection of the
    // existing context with the word-boundary-ized incoming context), the
    // conflict is reported and, if resolution is enabled, resolved by
    // restricting (subtracting) whichever rule's context is resolvable.
    //
    // The conflict predicates / resolution touch only the base 'Rule' context
    // (via 'is_empty_intersection'/'is_subset'/'subtract') plus the free
    // 'wbize' helper, so they are applied here through the base 'Rule' data
    // without downcasting. The incoming rule is then filed under its input
    // symbol and pushed into 'rule_vector'.
    pub fn add_rule_and_display_and_resolve_conflicts(
        &mut self,
        cfg: &OstConfig,
        mut rule: ConflictResolvingLeftArrowRule<B>,
    ) -> crate::error::Result<usize> {
        let input = rule.input_symbol.clone();
        if let Some(indices) = self.input_to_rule_map.get(&input) {
            let existing_indices: Vec<usize> = indices.clone();
            for existing_index in existing_indices {
                // (*it)->conflicts_this(*rule, conflicting_context):
                //   ! existing.context.is_empty_intersection(wbize(cfg, rule.context))
                let wbized_incoming = wbize(cfg, &rule.base.base.context)?;
                let mut conflicting_context: StringVector = Vec::new();
                let conflicts = {
                    let existing = self.base.rule_vector[existing_index].rule();
                    !existing
                        .context
                        .is_empty_intersection(&wbized_incoming, &mut conflicting_context)
                };
                if conflicts {
                    if self.report_left_arrow_conflicts {
                        let existing_name =
                            self.base.rule_vector[existing_index].rule().name.clone();
                        let mut line = format!(
                            "There is a <=-rule conflict between {} and {}.\nE.g. in context ",
                            Rule::<B>::get_print_name(&existing_name),
                            Rule::<B>::get_print_name(&rule.base.base.name)
                        );
                        let mut diamond_seen = false;
                        for sp in conflicting_context.iter() {
                            let mut symbol_pair = sp.replace(TWOLC_EPSILON, "");
                            if symbol_pair == "__HFST_TWOLC_DIAMOND:__HFST_TWOLC_DIAMOND" {
                                if diamond_seen {
                                    continue;
                                }
                                symbol_pair = "_".to_string();
                                diamond_seen = true;
                            } else if symbol_pair
                                == "@_TWOLC_IDENTITY_SYMBOL_@:@_TWOLC_IDENTITY_SYMBOL_@"
                            {
                                symbol_pair = "?".to_string();
                            }
                            line.push_str(&format!("{} ", symbol_pair));
                        }
                        warn!("{}", line);
                    }
                    if self.resolve_left_arrow_conflicts {
                        // existing.resolvable_conflict(rule):
                        //   existing.context.is_subset(wbize(cfg, rule.context))
                        let existing_resolvable = {
                            let existing = self.base.rule_vector[existing_index].rule();
                            existing.context.is_subset(cfg, &wbized_incoming)?
                        };
                        if existing_resolvable {
                            if self.report_left_arrow_conflicts {
                                let existing_name =
                                    self.base.rule_vector[existing_index].rule().name.clone();
                                warn!(
                                    "Resolving the conflict by restricting the context of {}.",
                                    Rule::<B>::get_print_name(&existing_name)
                                );
                            }
                            // existing.resolve_conflict(rule):
                            //   existing.context.subtract(cfg, rule.context);
                            let incoming_context = clone_ost(&rule.base.base.context);
                            let existing = self.base.rule_vector[existing_index].rule_mut();
                            existing.context.subtract(cfg, &incoming_context)?;
                        } else {
                            // rule.resolvable_conflict(*it):
                            //   rule.context.is_subset(wbize(cfg, existing.context))
                            let wbized_existing = {
                                let existing = self.base.rule_vector[existing_index].rule();
                                wbize(cfg, &existing.context)?
                            };
                            let incoming_resolvable =
                                rule.base.base.context.is_subset(cfg, &wbized_existing)?;
                            if incoming_resolvable {
                                if self.report_left_arrow_conflicts {
                                    warn!(
                                        "Resolving the conflict by restricting the context of {}.",
                                        rule.base.base.name
                                    );
                                }
                                // rule.resolve_conflict(*it):
                                //   rule.context.subtract(cfg, existing.context);
                                let existing_context = {
                                    let existing = self.base.rule_vector[existing_index].rule();
                                    clone_ost(&existing.context)
                                };
                                rule.base.base.context.subtract(cfg, &existing_context)?;
                            } else if self.report_left_arrow_conflicts {
                                warn!("The conflict is unresolvable.");
                            }
                        }
                    }
                }
            }
        }
        let index = self
            .base
            .add_rule(TwolcRule::ConflictResolvingLeftArrow(rule));
        self.input_to_rule_map.entry(input).or_default().push(index);
        Ok(index)
    }

    pub fn compile(&mut self, cfg: &OstConfig, be_verbose: bool) -> crate::error::Result<()> {
        self.base.compile(cfg, be_verbose)?;
        Ok(())
    }

    pub(crate) fn rule_ref(&self, index: usize) -> &TwolcRule<B> {
        self.base.rule_ref(index)
    }
}

impl<B: AlgebraBackend> Default for LeftArrowRuleContainer<B> {
    fn default() -> Self {
        LeftArrowRuleContainer::new()
    }
}

/// Copy an ['OtherSymbolTransducer'] (the C++ copy constructor /
/// 'operator=' that copies 'is_broken' + the wrapped transducer). Used by the
/// container conflict-resolution code, which must read one rule's context while
/// mutating another's.
fn clone_ost<B: AlgebraBackend>(t: &OtherSymbolTransducer<B>) -> OtherSymbolTransducer<B> {
    OtherSymbolTransducer {
        is_broken: t.is_broken,
        transducer: t.transducer.clone(),
    }
}

// ───────────────────────────────────────────────────────────────────────────
// TwolCGrammar (rule_src/TwolCGrammar.cc).
// ───────────────────────────────────────────────────────────────────────────

// [spec:hfst:def:twol-c-grammar.twol-c-grammar]
impl<B: AlgebraBackend> TwolCGrammar<B> {
    // [spec:hfst:def:twol-c-grammar.twol-c-grammar.twol-c-grammar-fn]
    // [spec:hfst:sem:twol-c-grammar.twol-c-grammar.twol-c-grammar-fn]
    //
    // C++ ctor wires the container conflict flags:
    //   left.set_report_left_arrow_conflicts(! be_quiet);
    //   left.set_resolve_left_arrow_conflicts(resolve_left_conflicts);
    //   right.set_report_right_arrow_conflicts(be_verbose);
    //   right.set_resolve_right_arrow_conflicts(resolve_right_conflicts);
    pub fn new(
        be_quiet: bool,
        be_verbose: bool,
        resolve_left_conflicts: bool,
        resolve_right_conflicts: bool,
    ) -> Self {
        let mut left_arrow_rule_container = LeftArrowRuleContainer::new();
        let mut right_arrow_rule_container = RightArrowRuleContainer::new();
        left_arrow_rule_container.set_report_left_arrow_conflicts(!be_quiet);
        left_arrow_rule_container.set_resolve_left_arrow_conflicts(resolve_left_conflicts);
        right_arrow_rule_container.set_report_right_arrow_conflicts(be_verbose);
        right_arrow_rule_container.set_resolve_right_arrow_conflicts(resolve_right_conflicts);
        TwolCGrammar {
            be_quiet,
            be_verbose,
            name_to_rule_subcases: BTreeMap::new(),
            left_arrow_rule_container,
            right_arrow_rule_container,
            other_rule_container: RuleContainer::new(),
            compiled_rule_container: RuleContainer::new(),
            diacritics: Vec::new(),
        }
    }

    // [spec:hfst:def:twol-c-grammar.twol-c-grammar.get-original-name-fn]
    // [spec:hfst:sem:twol-c-grammar.twol-c-grammar.get-original-name-fn]
    //
    // C++ 'name.substr(0, name.find("SUBCASE:"))'.
    pub fn get_original_name(name: &str) -> String {
        match name.find("SUBCASE:") {
            Some(pos) => name[..pos].to_string(),
            None => name.to_string(),
        }
    }

    // [spec:hfst:def:twol-c-grammar.twol-c-grammar.define-diacritics-fn]
    // [spec:hfst:sem:twol-c-grammar.twol-c-grammar.define-diacritics-fn]
    pub fn define_diacritics(&mut self, cfg: &mut OstConfig, diacritics: &SymbolRange) {
        self.diacritics = diacritics.to_vec();
        OtherSymbolTransducer::<B>::define_diacritics(cfg, diacritics);
    }

    /// Record a subcase handle for 'name''s original (pre-'SUBCASE:') name.
    fn insert_subcase(&mut self, name: &str, handle: RuleHandle) {
        self.name_to_rule_subcases
            .entry(Self::get_original_name(name))
            .or_default()
            .insert(handle);
    }

    /// 'TwolCGrammar::add_rule(name, const SymbolPair &center, oper, contexts)'
    /// — the single-pair center overload ('RIGHT'/'LEFT'/'LEFT_RIGHT'/
    /// 'NOT_LEFT').
    pub fn add_rule_pair(
        &mut self,
        cfg: &OstConfig,
        name: &str,
        center: &SymbolPair,
        oper: Operator,
        contexts: &OtherSymbolTransducerVector<B>,
    ) -> crate::error::Result<()> {
        match oper {
            Operator::RIGHT => {
                let rule = ConflictResolvingRightArrowRule::new(cfg, name, center, contexts)?;
                let index = self
                    .right_arrow_rule_container
                    .add_rule_and_display_and_resolve_conflicts(cfg, rule)?;
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Right,
                        index,
                    },
                );
            }
            Operator::LEFT => {
                let rule = ConflictResolvingLeftArrowRule::new(cfg, name, center, contexts)?;
                let index = self
                    .left_arrow_rule_container
                    .add_rule_and_display_and_resolve_conflicts(cfg, rule)?;
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Left,
                        index,
                    },
                );
            }
            Operator::LEFT_RIGHT => {
                let right_rule = ConflictResolvingRightArrowRule::new(cfg, name, center, contexts)?;
                let right_index = self
                    .right_arrow_rule_container
                    .add_rule_and_display_and_resolve_conflicts(cfg, right_rule)?;
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Right,
                        index: right_index,
                    },
                );
                let left_rule = ConflictResolvingLeftArrowRule::new(cfg, name, center, contexts)?;
                let left_index = self
                    .left_arrow_rule_container
                    .add_rule_and_display_and_resolve_conflicts(cfg, left_rule)?;
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Left,
                        index: left_index,
                    },
                );
            }
            Operator::NOT_LEFT => {
                let rule = LeftRestrictionArrowRule::new_pair(cfg, name, center, contexts)?;
                let index = self
                    .other_rule_container
                    .add_rule(TwolcRule::LeftRestrictionArrow(rule));
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Other,
                        index,
                    },
                );
            }
            Operator::RE_RIGHT
            | Operator::RE_LEFT
            | Operator::RE_NOT_LEFT
            | Operator::RE_LEFT_RIGHT => {
                panic!("TwolCGrammar::add_rule_pair: unexpected operator {oper:?}")
            }
        }
        Ok(())
    }

    /// 'TwolCGrammar::add_rule(name, const OtherSymbolTransducer &center, oper,
    /// contexts)' — the regex-center overload ('RE_*'). The center is wrapped
    /// by 'Rule::get_center(restricted_center)' ('?* D center D ?*').
    pub fn add_rule_regex(
        &mut self,
        cfg: &OstConfig,
        name: &str,
        center: &OtherSymbolTransducer<B>,
        oper: Operator,
        contexts: &OtherSymbolTransducerVector<B>,
    ) -> crate::error::Result<()> {
        let center_fst = Rule::get_center_restricted(cfg, center)?;
        match oper {
            Operator::RE_RIGHT => {
                let rule = RightArrowRule::new(cfg, name, clone_ost(&center_fst), contexts)?;
                let index = self
                    .other_rule_container
                    .add_rule(TwolcRule::RightArrow(rule));
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Other,
                        index,
                    },
                );
            }
            Operator::RE_LEFT => {
                let rule = LeftArrowRule::new(cfg, name, clone_ost(&center_fst), contexts)?;
                let index = self
                    .other_rule_container
                    .add_rule(TwolcRule::LeftArrow(rule));
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Other,
                        index,
                    },
                );
            }
            Operator::RE_LEFT_RIGHT => {
                let right_rule = RightArrowRule::new(cfg, name, clone_ost(&center_fst), contexts)?;
                let right_index = self
                    .other_rule_container
                    .add_rule(TwolcRule::RightArrow(right_rule));
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Other,
                        index: right_index,
                    },
                );
                let left_rule = LeftArrowRule::new(cfg, name, clone_ost(&center_fst), contexts)?;
                let left_index = self
                    .other_rule_container
                    .add_rule(TwolcRule::LeftArrow(left_rule));
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Other,
                        index: left_index,
                    },
                );
            }
            Operator::RE_NOT_LEFT => {
                let rule =
                    LeftRestrictionArrowRule::new(cfg, name, clone_ost(&center_fst), contexts)?;
                let index = self
                    .other_rule_container
                    .add_rule(TwolcRule::LeftRestrictionArrow(rule));
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Other,
                        index,
                    },
                );
            }
            Operator::RIGHT | Operator::LEFT | Operator::NOT_LEFT | Operator::LEFT_RIGHT => {
                panic!("TwolCGrammar::add_rule_regex: unexpected operator {oper:?}")
            }
        }
        Ok(())
    }

    // [spec:hfst:def:twol-c-grammar.twol-c-grammar.add-rule-fn]
    // [spec:hfst:sem:twol-c-grammar.twol-c-grammar.add-rule-fn]
    //
    // 'TwolCGrammar::add_rule(name, const SymbolPairVector &center, oper,
    // contexts)' — the multi-pair center overload, building one rule per center
    // pair named 'name CENTER=<in>:<out>'.
    pub fn add_rule_pairs(
        &mut self,
        cfg: &OstConfig,
        name: &str,
        center: &SymbolPairVector,
        oper: Operator,
        contexts: &OtherSymbolTransducerVector<B>,
    ) -> crate::error::Result<()> {
        for pair in center.iter() {
            let center_name = format!("{} CENTER={}:{}", name, pair.0, pair.1);
            match oper {
                Operator::RIGHT => {
                    let rule =
                        ConflictResolvingRightArrowRule::new(cfg, &center_name, pair, contexts)?;
                    let index = self
                        .right_arrow_rule_container
                        .add_rule_and_display_and_resolve_conflicts(cfg, rule)?;
                    self.insert_subcase(
                        &center_name,
                        RuleHandle {
                            container: RuleContainerKind::Right,
                            index,
                        },
                    );
                }
                Operator::LEFT => {
                    let rule =
                        ConflictResolvingLeftArrowRule::new(cfg, &center_name, pair, contexts)?;
                    let index = self
                        .left_arrow_rule_container
                        .add_rule_and_display_and_resolve_conflicts(cfg, rule)?;
                    self.insert_subcase(
                        &center_name,
                        RuleHandle {
                            container: RuleContainerKind::Left,
                            index,
                        },
                    );
                }
                Operator::LEFT_RIGHT => {
                    let right_rule =
                        ConflictResolvingRightArrowRule::new(cfg, &center_name, pair, contexts)?;
                    let right_index = self
                        .right_arrow_rule_container
                        .add_rule_and_display_and_resolve_conflicts(cfg, right_rule)?;
                    self.insert_subcase(
                        &center_name,
                        RuleHandle {
                            container: RuleContainerKind::Right,
                            index: right_index,
                        },
                    );
                    let left_rule =
                        ConflictResolvingLeftArrowRule::new(cfg, &center_name, pair, contexts)?;
                    let left_index = self
                        .left_arrow_rule_container
                        .add_rule_and_display_and_resolve_conflicts(cfg, left_rule)?;
                    self.insert_subcase(
                        &center_name,
                        RuleHandle {
                            container: RuleContainerKind::Left,
                            index: left_index,
                        },
                    );
                }
                Operator::NOT_LEFT => {
                    let rule =
                        LeftRestrictionArrowRule::new_pair(cfg, &center_name, pair, contexts)?;
                    let index = self
                        .other_rule_container
                        .add_rule(TwolcRule::LeftRestrictionArrow(rule));
                    self.insert_subcase(
                        &center_name,
                        RuleHandle {
                            container: RuleContainerKind::Other,
                            index,
                        },
                    );
                }
                Operator::RE_RIGHT
                | Operator::RE_LEFT
                | Operator::RE_NOT_LEFT
                | Operator::RE_LEFT_RIGHT => {
                    panic!("TwolCGrammar::add_rule_pairs: unexpected operator {oper:?}")
                }
            }
        }
        Ok(())
    }

    /// Borrow the rule a ['RuleHandle'] points at.
    fn rule_at(&self, handle: RuleHandle) -> &TwolcRule<B> {
        match handle.container {
            RuleContainerKind::Left => self.left_arrow_rule_container.rule_ref(handle.index),
            RuleContainerKind::Right => self.right_arrow_rule_container.rule_ref(handle.index),
            RuleContainerKind::Other => self.other_rule_container.rule_ref(handle.index),
        }
    }

    // The compile phase shared by both 'compile_and_store' flavours: compile
    // each container, then for every original rule name build a
    // 'Rule(name, RuleVector)' intersecting its subcases, and add the missing
    // diacritics freely. Fills 'compiled_rule_container'.
    fn compile_rules(&mut self, cfg: &OstConfig) -> crate::error::Result<()> {
        if !self.be_quiet {
            info!("Compiling rules.");
        }

        let verbose = (!self.be_quiet) && self.be_verbose;
        self.left_arrow_rule_container.compile(cfg, verbose)?;
        self.right_arrow_rule_container.compile(cfg, verbose)?;
        self.other_rule_container.compile(cfg, verbose)?;

        // Build one intersecting 'ResultRule' per original rule name. The
        // 'name_to_rule_subcases' map is iterated in its (ordered) key order,
        // mirroring the C++ 'StringRuleSetMap' traversal.
        let names: Vec<String> = self.name_to_rule_subcases.keys().cloned().collect();
        for name in names {
            let handles: Vec<RuleHandle> =
                self.name_to_rule_subcases[&name].iter().copied().collect();
            let subcases: Vec<&TwolcRule<B>> = handles.iter().map(|&h| self.rule_at(h)).collect();
            let result_rule = Rule::new_from_vector(cfg, &name, &subcases)?;
            self.compiled_rule_container
                .add_rule(TwolcRule::Result(result_rule));
        }
        let diacritics = self.diacritics.clone();
        self.compiled_rule_container
            .add_missing_symbols_freely(cfg, &diacritics)?;
        Ok(())
    }

    // [spec:hfst:def:twol-c-grammar.twol-c-grammar.compile-and-store-fn]
    // [spec:hfst:sem:twol-c-grammar.twol-c-grammar.compile-and-store-fn]
    //
    // C++ compiles each container, then for every original rule name builds a
    // 'Rule(name, RuleVector)' intersecting its subcases, adds the missing
    // diacritics freely, and stores the result. This flavour RETURNS the
    // assembled result transducer (the intersection of every compiled rule)
    // instead of writing to a stream, so a smoke can drive the compiler;
    // 'compile_and_store_stream' below is the 1:1 stream-store path.
    pub fn compile_and_store(
        &mut self,
        cfg: &OstConfig,
    ) -> crate::error::Result<HfstTransducer<B>> {
        self.compile_rules(cfg)?;

        if !self.be_quiet {
            info!("Storing rules.");
        }

        // Intersect the compiled result rules into one transducer and return
        // it as the grammar's result (the union of all rule constraints over
        // the shared '?*' universe — intersection of the per-rule
        // 'rule_transducer's).
        assemble_result_transducer(cfg, &self.compiled_rule_container)
    }

    // [spec:hfst:def:twol-c-grammar.twol-c-grammar.compile-and-store-fn]
    // [spec:hfst:sem:twol-c-grammar.twol-c-grammar.compile-and-store-fn]
    //
    // The 1:1 port of the C++ 'compile_and_store(HfstOutputStream &out)' store
    // path: compile every rule, then write ONE transducer PER RULE (named via
    // 'Rule::store''s info-symbol/name mapping) into the binary output stream
    // — the rule archive 'hfst-twolc' emits and 'hfst-compose-intersect'
    // consumes.
    pub fn compile_and_store_stream(
        &mut self,
        cfg: &OstConfig,
        out: &mut crate::hfst_output_stream::HfstOutputStream,
    ) -> crate::error::Result<()> {
        self.compile_rules(cfg)?;

        if !self.be_quiet {
            info!("Storing rules.");
        }

        let verbose = (!self.be_quiet) && self.be_verbose;
        self.compiled_rule_container.store(cfg, out, verbose)
    }
}

/// Intersect every non-empty compiled 'ResultRule''s rule-transducer into one
/// 'HfstTransducer' — the value the deferred 'compile_and_store' store path
/// would otherwise serialise. Starts from '?*' (the universal language over the
/// twolc unknown symbol) so an empty grammar yields the universal automaton,
/// matching 'Rule(name, RuleVector)''s own '?*'-seeded intersection.
fn assemble_result_transducer<B: AlgebraBackend>(
    cfg: &OstConfig,
    container: &RuleContainer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut result = OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?;
    result.repeat_star(cfg)?;
    for rule in container.rule_vector.iter() {
        if !rule.rule().is_empty {
            let rt = clone_ost(rule.rule_transducer());
            result.intersect(cfg, &rt)?;
        }
    }
    Ok(result.transducer)
}

// ───────────────────────────────────────────────────────────────────────────
// TwolcCompiler — the AST-walk driver (replaces TwolcCompiler.cc + the three
// Flex/Bison preprocessor passes).
// ───────────────────────────────────────────────────────────────────────────

/// The two shapes a rule center can evaluate to: a (possibly multi-)pair list
/// ('a:b | c:d'), or a regex transducer (':[ E ]:'). Drives the
/// ['TwolCGrammar::add_rule_pairs'] vs ['TwolCGrammar::add_rule_regex'] choice.
pub enum CenterEval<B: AlgebraBackend> {
    Pairs(SymbolPairVector),
    Regex(OtherSymbolTransducer<B>),
}

/// One concrete rule produced by expanding a ['TwolcRule']'s 'where'-variables:
/// a fully-substituted name, the evaluated center, the operator and the
/// evaluated (positive + negated-negative) contexts.
pub struct ConcreteRule<B: AlgebraBackend> {
    pub name: String,
    pub center: CenterEval<B>,
    pub oper: Operator,
    pub contexts: OtherSymbolTransducerVector<B>,
}

// [spec:hfst:def:twolc-compiler.hfst.twolc.twolc-compiler]
impl<B: AlgebraBackend> TwolcCompiler<B> {
    /// Construct with the C++ default flags: 'silent = false',
    /// 'verbose = false', 'resolve_left_conflicts = false',
    /// 'resolve_right_conflicts = true' (the upstream 'htwolc' defaults).
    /// The C++ 'format' parameter is the backend type parameter 'B' now.
    pub fn new() -> Self {
        Self::new_with_options(false, false, false, true)
    }

    /// Construct with explicit flags (the C++ 'TwolcCompiler::compile'
    /// parameters 'silent', 'verbose', 'resolve_left_conflicts',
    /// 'resolve_right_conflicts').
    pub fn new_with_options(
        silent: bool,
        verbose: bool,
        resolve_left: bool,
        resolve_right: bool,
    ) -> Self {
        TwolcCompiler {
            silent,
            verbose,
            resolve_left_conflicts: resolve_left,
            resolve_right_conflicts: resolve_right,
            sets: BTreeMap::new(),
            definitions: BTreeMap::new(),
            source: String::new(),
            source_name: String::from("<twolc>"),
            current_span: 0..0,
        }
    }

    /// Name shown in source-anchored diagnostics (the twolc file name). Set
    /// before `compile` so warnings point at the right file; defaults to
    /// `"<twolc>"`.
    pub fn set_source_name(&mut self, name: &str) -> &mut Self {
        self.source_name = name.to_string();
        self
    }

    /// Render a source-anchored error at the current top-level item's span.
    fn diag_error(&self, msg: &str) {
        crate::diag::emit(
            &self.source_name,
            &self.source,
            self.current_span.clone(),
            crate::diag::Severity::Error,
            msg,
        );
    }

    /// Render a source-anchored warning at the current top-level item's span.
    fn diag_warning(&self, msg: &str) {
        crate::diag::emit(
            &self.source_name,
            &self.source,
            self.current_span.clone(),
            crate::diag::Severity::Warning,
            msg,
        );
    }

    // [spec:hfst:def:twolc-compiler.hfst.twolc.twolc-compiler.compile-fn]
    // [spec:hfst:sem:twolc-compiler.hfst.twolc.twolc-compiler.compile-fn]
    //
    // Replaces the three Flex/Bison passes with 'nfst_twolc::parse' + an
    // AST-walk: set the transducer type, register the alphabet / diacritics /
    // sets / definitions, build the grammar, drive every rule (expanding
    // 'where'-variables and evaluating centers + contexts), and return the
    // intersected result transducer (the same Option contract as
    // 'XreCompiler::compile'). A parse failure yields None.
    pub fn compile(&mut self, input: &str) -> Option<HfstTransducer<B>> {
        let (cfg, mut grammar) = self.build_grammar(input)?;
        let result = grammar.compile_and_store(&cfg).ok()?;
        Some(result)
    }

    // [spec:hfst:def:twolc-compiler.hfst.twolc.twolc-compiler.compile-fn]
    // [spec:hfst:sem:twolc-compiler.hfst.twolc.twolc-compiler.compile-fn]
    //
    // The stream flavour of 'compile': same parse + grammar walk, but the
    // result is stored through 'TwolCGrammar::compile_and_store_stream' — one
    // named transducer per rule into the binary output stream (the archive the
    // C++ 'hfst-twolc' driver writes). A parse or compile failure yields None.
    pub fn compile_and_store(
        &mut self,
        input: &str,
        out: &mut crate::hfst_output_stream::HfstOutputStream,
    ) -> Option<()> {
        let (cfg, mut grammar) = self.build_grammar(input)?;
        grammar.compile_and_store_stream(&cfg, out).ok()?;
        Some(())
    }

    // The shared front half of both 'compile' flavours: parse the grammar
    // source and walk the AST into a ready-to-compile 'TwolCGrammar' (plus the
    // per-compile alphabet config). A parse failure reports its diagnostics
    // (unless silent) and yields None, like the C++ preprocessor passes that
    // printed to their error stream and made the driver exit.
    fn build_grammar(&mut self, input: &str) -> Option<(OstConfig, TwolCGrammar<B>)> {
        // Retain the source so diagnostics can render the offending snippet.
        self.source = input.to_string();
        let file = match nfst_twolc::parse(input) {
            Ok(f) => f,
            Err(e) => {
                if !self.silent {
                    for d in &e.diagnostics {
                        self.current_span = d.span.range.clone();
                        self.diag_error(&d.message);
                    }
                }
                return None;
            }
        };
        let twolc_file = &file.value;

        // Per-compile alphabet config (formerly the 'OST_CONFIG' thread-local),
        // threaded by reference through the rule/grammar walk. (The C++
        // 'set_transducer_type' call is gone: the type is the parameter 'B'.)
        let mut cfg = OstConfig::new();

        let mut grammar = TwolCGrammar::new(
            self.silent,
            self.verbose,
            self.resolve_left_conflicts,
            self.resolve_right_conflicts,
        );

        // Sets are registered before the alphabet so the completion pass can
        // recognise (and skip) set-name pair sides, as the htwolcpre1 lexer's
        // '__HFST_TWOLC_SET_NAME=' marking let the C++ passes do.
        self.register_sets(&twolc_file.sets);
        self.register_alphabet(&mut cfg, twolc_file).ok()?;
        self.register_diacritics(&mut cfg, &twolc_file.diacritics, &mut grammar);
        self.register_definitions(&cfg, &twolc_file.definitions)
            .ok()?;

        for rule in twolc_file.rules.iter() {
            self.current_span = rule.span.range.clone();
            self.drive_rule(&cfg, &rule.value, &mut grammar).ok()?;
        }

        Some((cfg, grammar))
    }

    /// Register the 'Alphabet' section: collect the declared symbol pairs and
    /// publish them to ['OtherSymbolTransducer::set_symbol_pairs'] (which also
    /// inserts the diamond:diamond pair).
    pub fn register_alphabet(
        &mut self,
        cfg: &mut OstConfig,
        twolc_file: &TwolcFile,
    ) -> crate::error::Result<()> {
        let mut symbol_pairs: BTreeSet<SymbolPair> = BTreeSet::new();
        for p in &twolc_file.alphabet {
            symbol_pairs.insert((
                normalize_symbol(&p.value.upper),
                normalize_symbol(&p.value.lower),
            ));
        }
        // htwolcpre2's 'complete_alphabet()': add all explicit X:Y pairs used
        // anywhere in the grammar (rule centers, contexts, definitions —
        // 'where'-variables expanded first, as htwolcpre1 expanded them before
        // pre2 ran) which are missing from the Alphabet section, plus the
        // absolute word boundary pair.
        self.collect_grammar_pairs(twolc_file, &mut symbol_pairs)?;
        symbol_pairs.insert((
            Symbol::new_static("__HFST_TWOLC_.#."),
            Symbol::new_static("__HFST_TWOLC_.#."),
        ));
        OtherSymbolTransducer::<B>::set_symbol_pairs(cfg, &symbol_pairs);
        Ok(())
    }

    /// htwolcpre2's 'complete_alphabet()' / 'insert_alphabet_pairs()': collect
    /// every explicit concrete 'X:Y' pair appearing anywhere in the grammar
    /// (rule centers, rule contexts, 'except' contexts and definition bodies)
    /// into 'pairs', so pairs used only in rules (e.g. 'e:0') complete the
    /// Alphabet section. Rules with 'where'-variables are expanded first (as
    /// htwolcpre1 did before the completion pass ran), so variable centers
    /// like 'Vx:Vy' contribute their substituted pairs. One-sided pairs
    /// ('e:', ':i' — an 'Any' side) are skipped, as the C++ token scan skipped
    /// pairs with an internal '__HFST_TWOLC_?' side; pairs with a set-name
    /// side are skipped like the C++ 'is_set_pair' filter skipped the marked
    /// '__HFST_TWOLC_SET_NAME=' pairs everywhere they could be observed.
    fn collect_grammar_pairs(
        &mut self,
        file: &TwolcFile,
        pairs: &mut BTreeSet<SymbolPair>,
    ) -> crate::error::Result<()> {
        let empty_vvm = VariableValueMap::new();
        for def in &file.definitions {
            self.collect_regex_pairs(&def.value.body, &empty_vvm, pairs);
        }
        for rule in &file.rules {
            for vvm in self.variable_assignments(&rule.value)? {
                match &rule.value.center {
                    RuleCenter::Pair(ps) => {
                        for p in ps {
                            let upper = substitute_symbol(&p.upper, &vvm);
                            let lower = substitute_symbol(&p.lower, &vvm);
                            self.insert_grammar_pair(upper, lower, pairs);
                        }
                    }
                    RuleCenter::Regex(e) => self.collect_regex_pairs(e, &vvm, pairs),
                }
                for ctx in rule
                    .value
                    .positive_contexts
                    .iter()
                    .chain(rule.value.negative_contexts.iter())
                {
                    self.collect_regex_pairs(&ctx.left, &vvm, pairs);
                    self.collect_regex_pairs(&ctx.right, &vvm, pairs);
                }
            }
        }
        Ok(())
    }

    /// The per-rule variable assignments the ['RuleVariables'] odometer yields
    /// (a single empty assignment when the rule has no 'where'-clause) — the
    /// same expansion ['expand_rule_variables'] drives rules with.
    fn variable_assignments(
        &self,
        rule: &AstTwolcRule,
    ) -> crate::error::Result<Vec<VariableValueMap>> {
        let rule_variables = match rule.variables.as_ref() {
            Some(blocks) if !blocks.is_empty() => self.build_rule_variables(blocks),
            _ => RuleVariables::new(),
        };
        if rule_variables.empty() {
            return Ok(vec![VariableValueMap::new()]);
        }
        let mut result: Vec<VariableValueMap> = Vec::new();
        let mut it = rule_variables.begin()?;
        let end = rule_variables.end()?;
        while it.ne(&end) {
            let mut vvm = VariableValueMap::new();
            it.set_values(&mut vvm);
            result.push(vvm);
            it.increment();
        }
        Ok(result)
    }

    /// The regex walk of ['collect_grammar_pairs']: record every 'Pair' node
    /// whose both sides are concrete symbols under the variable assignment.
    fn collect_regex_pairs(
        &self,
        e: &Spanned<TwolcRegex>,
        vvm: &VariableValueMap,
        pairs: &mut BTreeSet<SymbolPair>,
    ) {
        match &e.value {
            TwolcRegex::Pair { upper, lower } => {
                if let (Some(u), Some(l)) = (
                    Self::concrete_symbol(upper, vvm),
                    Self::concrete_symbol(lower, vvm),
                ) {
                    self.insert_grammar_pair(u, l, pairs);
                }
            }
            TwolcRegex::Group(inner) | TwolcRegex::Optional(inner) => {
                self.collect_regex_pairs(inner, vvm, pairs);
            }
            TwolcRegex::Binary(_, l, r) => {
                self.collect_regex_pairs(l, vvm, pairs);
                self.collect_regex_pairs(r, vvm, pairs);
            }
            TwolcRegex::Unary(_, inner)
            | TwolcRegex::RepeatN(inner, _)
            | TwolcRegex::RepeatNToK(inner, _, _) => {
                self.collect_regex_pairs(inner, vvm, pairs);
            }
            TwolcRegex::Symbol(_) | TwolcRegex::Epsilon | TwolcRegex::Any => {}
        }
    }

    /// Insert one collected pair, skipping pairs with a set-name side (the
    /// 'is_set_pair' filter).
    fn insert_grammar_pair(&self, upper: Symbol, lower: Symbol, pairs: &mut BTreeSet<SymbolPair>) {
        if self.sets.contains_key(upper.as_str()) || self.sets.contains_key(lower.as_str()) {
            return;
        }
        pairs.insert((upper, lower));
    }

    /// Resolve a pair side to a concrete internal symbol under the variable
    /// assignment, or None when the side is non-concrete ('Any' / a nested
    /// expression).
    fn concrete_symbol(e: &Spanned<TwolcRegex>, vvm: &VariableValueMap) -> Option<Symbol> {
        match &e.value {
            TwolcRegex::Symbol(s) => Some(substitute_symbol(s, vvm)),
            TwolcRegex::Epsilon => Some(Symbol::new_static(TWOLC_EPSILON)),
            TwolcRegex::Group(inner) => Self::concrete_symbol(inner, vvm),
            TwolcRegex::Pair { .. }
            | TwolcRegex::Any
            | TwolcRegex::Optional(_)
            | TwolcRegex::Binary(..)
            | TwolcRegex::Unary(..)
            | TwolcRegex::RepeatN(..)
            | TwolcRegex::RepeatNToK(..) => None,
        }
    }

    /// Register the 'Diacritics' section: publish the diacritic list to both the
    /// 'OtherSymbolTransducer' config and the grammar.
    pub fn register_diacritics(
        &mut self,
        cfg: &mut OstConfig,
        diacritics: &[Spanned<String>],
        grammar: &mut TwolCGrammar<B>,
    ) {
        let list: SymbolRange = diacritics
            .iter()
            .map(|d| Symbol::from(d.value.clone()))
            .collect();
        grammar.define_diacritics(cfg, &list);
    }

    /// Register the 'Sets' section: record each set's ordered member list, so a
    /// 'Symbol' naming a set expands to the disjunction of its members.
    pub fn register_sets(&mut self, sets: &[Spanned<SetDefinition>]) {
        for s in sets {
            self.sets.insert(
                s.value.name.clone(),
                s.value.members.iter().map(Symbol::new).collect(),
            );
        }
    }

    /// Register the 'Definitions' section: evaluate each named regex body to an
    /// ['OtherSymbolTransducer'] so a 'Symbol' naming a definition expands to
    /// it (mirrors the C++ 'NameToRegexMap').
    pub fn register_definitions(
        &mut self,
        cfg: &OstConfig,
        defs: &[Spanned<TwolcDefinition>],
    ) -> crate::error::Result<()> {
        for d in defs {
            self.current_span = d.span.range.clone();
            let t = self.eval_regex(cfg, &d.value.body)?;
            self.definitions.insert(d.value.name.clone(), t);
        }
        Ok(())
    }

    /// Drive one ['TwolcRule']: expand its 'where'-variables into concrete
    /// rules and feed each into the grammar via the matching 'add_rule'
    /// overload.
    pub fn drive_rule(
        &mut self,
        cfg: &OstConfig,
        rule: &AstTwolcRule,
        grammar: &mut TwolCGrammar<B>,
    ) -> crate::error::Result<()> {
        for concrete in self.expand_rule_variables(cfg, rule)? {
            match concrete.center {
                CenterEval::Pairs(pairs) => {
                    grammar.add_rule_pairs(
                        cfg,
                        &concrete.name,
                        &pairs,
                        concrete.oper,
                        &concrete.contexts,
                    )?;
                }
                CenterEval::Regex(center) => {
                    grammar.add_rule_regex(
                        cfg,
                        &concrete.name,
                        &center,
                        concrete.oper,
                        &concrete.contexts,
                    )?;
                }
            }
        }
        Ok(())
    }

    /// Expand a ['TwolcRule']'s 'where'-blocks into concrete rules using the
    /// ['RuleVariables'] odometer. With no 'where'-clause the rule yields one
    /// concrete rule with an empty variable assignment.
    pub fn expand_rule_variables(
        &mut self,
        cfg: &OstConfig,
        rule: &AstTwolcRule,
    ) -> crate::error::Result<Vec<ConcreteRule<B>>> {
        let regex_center = matches!(rule.center, RuleCenter::Regex(_));
        let oper = Self::operator_of(rule.operator, regex_center);

        // No variables: emit the rule once with no substitutions. The variable
        // markers in names only matter when a 'where'-clause is present, so the
        // rule name is used as-is.
        let blocks_opt = rule.variables.as_ref();
        let rule_variables = match blocks_opt {
            Some(blocks) if !blocks.is_empty() => self.build_rule_variables(blocks),
            _ => RuleVariables::new(),
        };

        let mut result: Vec<ConcreteRule<B>> = Vec::new();

        if rule_variables.empty() {
            let empty_vvm = VariableValueMap::new();
            if let Some(cr) = self.build_concrete_rule(cfg, rule, oper, &empty_vvm)? {
                result.push(cr);
            }
            return Ok(result);
        }

        // Odometer over the cross-product of the where-blocks.
        let mut it = rule_variables.begin()?;
        let end = rule_variables.end()?;
        while it.ne(&end) {
            let mut vvm = VariableValueMap::new();
            it.set_values(&mut vvm);
            if let Some(cr) = self.build_concrete_rule(cfg, rule, oper, &vvm)? {
                result.push(cr);
            }
            it.increment();
        }
        Ok(result)
    }

    /// Build a single ['ConcreteRule'] from a rule template and a variable
    /// assignment: substitute the assignment into the name/center symbols, then
    /// evaluate the center and contexts.
    fn build_concrete_rule(
        &mut self,
        cfg: &OstConfig,
        rule: &AstTwolcRule,
        oper: Operator,
        vvm: &VariableValueMap,
    ) -> crate::error::Result<Option<ConcreteRule<B>>> {
        // Compose the subcase-qualified name. The C++ rule template carried a
        // '__HFST_TWOLC_RULE_NAME' marker that 'RuleSymbolVector' rewrote with
        // the 'SUBCASE:'/'var=value' markers; here the rule's own name plays
        // that role and the marker rewrite is applied directly.
        let name = build_rule_name(&rule.name, vvm);

        let center = self.eval_center(cfg, &rule.center, vvm)?;
        let contexts =
            self.eval_contexts(cfg, &rule.positive_contexts, &rule.negative_contexts, vvm)?;
        Ok(Some(ConcreteRule {
            name,
            center,
            oper,
            contexts,
        }))
    }

    /// AST 'where'-blocks -> ['RuleVariables'] (the C++
    /// 'set_variable'/'add_values'/'set_matcher' sequence per block).
    pub fn build_rule_variables(&self, blocks: &[VariableBlock]) -> RuleVariables {
        let mut rv = RuleVariables::new();
        for block in blocks {
            for assignment in block.assignments.iter() {
                rv.set_variable(&assignment.name);
                // A value that names a Set expands to the set's members: the
                // 'where'-variable iterates over them ('where Cx in (DelCns)'
                // ranges over g8, m8, n8, h8). nfst_twolc keeps the where-clause
                // verbatim (the values are raw source strings), so the set
                // reference is resolved here — a plain symbol is its own
                // singleton via 'set_of'.
                let expanded: Vec<String> = assignment
                    .values
                    .iter()
                    .flat_map(|v| self.set_of(v).into_iter().map(|s| s.to_string()))
                    .collect();
                rv.add_values(&expanded);
            }
            rv.set_matcher(matcher_from_var_matcher(block.matcher));
        }
        rv
    }

    /// Map a ['RuleOp'] + center kind to a ['Operator']. Pair centers use the
    /// plain operators; regex centers (':[ E ]:') use the 'RE_*' variants. The
    /// '<=>' operator maps to 'LEFT_RIGHT'/'RE_LEFT_RIGHT', '/<=' to
    /// 'NOT_LEFT'/'RE_NOT_LEFT'.
    pub fn operator_of(op: RuleOp, regex_center: bool) -> Operator {
        match (op, regex_center) {
            (RuleOp::Right, false) => Operator::RIGHT,
            (RuleOp::Left, false) => Operator::LEFT,
            (RuleOp::LeftRight, false) => Operator::LEFT_RIGHT,
            (RuleOp::NotLeft, false) => Operator::NOT_LEFT,
            (RuleOp::Right, true) => Operator::RE_RIGHT,
            (RuleOp::Left, true) => Operator::RE_LEFT,
            (RuleOp::LeftRight, true) => Operator::RE_LEFT_RIGHT,
            (RuleOp::NotLeft, true) => Operator::RE_NOT_LEFT,
        }
    }

    /// Evaluate a rule center. A 'Pair' center becomes a ['SymbolPairVector']
    /// (one entry per alternative); a 'Regex' center is evaluated to an
    /// ['OtherSymbolTransducer']. Variable assignments substitute symbol names.
    pub fn eval_center(
        &mut self,
        cfg: &OstConfig,
        center: &RuleCenter,
        vvm: &VariableValueMap,
    ) -> crate::error::Result<CenterEval<B>> {
        Ok(match center {
            RuleCenter::Pair(pairs) => {
                let mut spv: SymbolPairVector = Vec::new();
                for p in pairs {
                    let upper = substitute_symbol(&p.upper, vvm);
                    let lower = substitute_symbol(&p.lower, vvm);
                    spv.push((upper, lower));
                }
                CenterEval::Pairs(spv)
            }
            RuleCenter::Regex(e) => CenterEval::Regex(self.eval_regex_with_vars(cfg, e, vvm)?),
        })
    }

    /// Evaluate the positive and negative contexts of a rule into one
    /// ['OtherSymbolTransducerVector']. Each positive context is 'X D ?* D Y';
    /// each negative context is the same, negated ('?* - context'). The C++
    /// negative contexts ('except' clauses) are negated before being added.
    pub fn eval_contexts(
        &mut self,
        cfg: &OstConfig,
        pos: &[RuleContext],
        neg: &[RuleContext],
        vvm: &VariableValueMap,
    ) -> crate::error::Result<OtherSymbolTransducerVector<B>> {
        let mut result: OtherSymbolTransducerVector<B> = pos
            .iter()
            .map(|ctx| self.eval_context(cfg, ctx, vvm))
            .collect::<crate::error::Result<_>>()?;
        for ctx in neg {
            let mut c = self.eval_context(cfg, ctx, vvm)?;
            c.negated(cfg)?;
            result.push(c);
        }
        Ok(result)
    }

    /// Evaluate one ['RuleContext'] into 'left D ?* D right' via
    /// ['OtherSymbolTransducer::get_context'].
    pub fn eval_context(
        &mut self,
        cfg: &OstConfig,
        ctx: &RuleContext,
        vvm: &VariableValueMap,
    ) -> crate::error::Result<OtherSymbolTransducer<B>> {
        let mut left = self.eval_regex_with_vars(cfg, &ctx.left, vvm)?;
        let mut right = self.eval_regex_with_vars(cfg, &ctx.right, vvm)?;
        OtherSymbolTransducer::get_context(cfg, &mut left, &mut right)
    }

    /// The member list of set 'sym', or the singleton '[sym]' when 'sym' names
    /// no set ('Alphabet::define_singleton_set').
    fn set_of(&self, sym: &str) -> Vec<Symbol> {
        match self.sets.get(sym) {
            Some(members) => members.iter().map(|m| normalize_symbol(m)).collect(),
            None => vec![Symbol::new(sym)],
        }
    }

    // [spec:hfst:def:alphabet.alphabet.is-pair-fn]
    // [spec:hfst:sem:alphabet.alphabet.is-pair-fn]
    //
    // 'Alphabet::is_pair' ('alphabet_src/Alphabet.cc'): whether the concrete
    // 'input:output' is licensed by the (completed) alphabet.
    fn is_pair(&self, cfg: &OstConfig, input: &str, output: &str) -> bool {
        if input == TWOLC_UNKNOWN && output == TWOLC_UNKNOWN {
            return true;
        }
        if cfg.diacritics.contains(input) && input == output {
            return true;
        }
        if cfg.diacritics.contains(input) && output == TWOLC_UNKNOWN {
            return true;
        }
        if input == TWOLC_UNKNOWN {
            return cfg.output_symbols.contains(output);
        }
        if output == TWOLC_UNKNOWN {
            return cfg.input_symbols.contains(input);
        }
        cfg.symbol_pairs
            .contains(&(Symbol::new(input), Symbol::new(output)))
    }

    // 'Alphabet::compute' ('alphabet_src/Alphabet.cc'): expand a pair whose
    // sides may be set names into the disjunction of the matching declared
    // alphabet pairs ('a set pair X:Y contains every declared pair x:y with
    // x in X and y in Y'). Sides that are plain symbols act as singleton
    // sets; an unknown ('?') side expands through 'new_pair'. An empty
    // result reproduces the htwolcpre3 'The pair set ... is empty.'
    // semantic error, which terminated compilation.
    fn pair_transducer(
        &mut self,
        cfg: &OstConfig,
        input: &str,
        output: &str,
    ) -> crate::error::Result<OtherSymbolTransducer<B>> {
        if cfg.diacritics.contains(input) {
            if input != output && output != TWOLC_EPSILON && output != TWOLC_UNKNOWN {
                self.diag_warning(&format!(
                    "Diacritic {input} in pair {input}:{output} will correspond 0."
                ));
            }
            return OtherSymbolTransducer::new_pair(cfg, input, input);
        }
        let mut pair_transducer = OtherSymbolTransducer::new(cfg)?;
        for x in self.set_of(input) {
            for y in self.set_of(output) {
                if self.is_pair(cfg, &x, &y) {
                    let pair = OtherSymbolTransducer::new_pair(cfg, &x, &y)?;
                    pair_transducer.disjunct(cfg, &pair)?;
                }
            }
        }
        if pair_transducer.is_empty() {
            if input == output {
                self.diag_error(&format!(
                    "The pair set {} is empty.",
                    Rule::<B>::get_print_name(input)
                ));
            } else {
                self.diag_error(&format!(
                    "The pair set {}:{} is empty.",
                    Rule::<B>::get_print_name(input),
                    Rule::<B>::get_print_name(output)
                ));
            }
            crate::bail!(EmptySymbolPairSet);
        }
        Ok(pair_transducer)
    }

    /// Evaluate a ['TwolcRegex'] with no variable substitution (used by the
    /// 'Definitions' section, which is evaluated before any rule expansion).
    pub fn eval_regex(
        &mut self,
        cfg: &OstConfig,
        e: &Spanned<TwolcRegex>,
    ) -> crate::error::Result<OtherSymbolTransducer<B>> {
        let vvm = VariableValueMap::new();
        self.eval_regex_with_vars(cfg, e, &vvm)
    }

    /// Evaluate a ['TwolcRegex'], substituting any variable symbol with its
    /// assigned value. Mirrors the 'xre.rs' 'eval'/'eval_unary'/'eval_binary'
    /// recursion shape, but over the smaller twolc regex sublanguage and
    /// building ['OtherSymbolTransducer']s.
    fn eval_regex_with_vars(
        &mut self,
        cfg: &OstConfig,
        e: &Spanned<TwolcRegex>,
        vvm: &VariableValueMap,
    ) -> crate::error::Result<OtherSymbolTransducer<B>> {
        Ok(match &e.value {
            TwolcRegex::Symbol(s) => {
                let sym = substitute_symbol(s, vvm);
                // A symbol naming a definition expands to its transducer; a
                // symbol naming a set expands to the declared pairs of the
                // 'sym:sym' pair set (the 'Alphabet::compute' semantics);
                // otherwise it is a literal 'sym:sym' pair.
                if let Some(def) = self.definitions.get(sym.as_str()) {
                    clone_ost(def)
                } else if self.sets.contains_key(sym.as_str()) {
                    self.pair_transducer(cfg, &sym, &sym)?
                } else {
                    OtherSymbolTransducer::new_symbol(cfg, &sym)?
                }
            }
            TwolcRegex::Pair { upper, lower } => {
                let up = symbol_of(upper, vvm);
                let lo = symbol_of(lower, vvm);
                if self.sets.contains_key(up.as_str()) || self.sets.contains_key(lo.as_str()) {
                    self.pair_transducer(cfg, &up, &lo)?
                } else {
                    OtherSymbolTransducer::new_pair(cfg, &up, &lo)?
                }
            }
            // A standalone '0' regexp (an empty context side, or an explicit
            // '0') denotes the EMPTY STRING, so it must be a real epsilon
            // (HFST_EPSILON) — C++ htwolcpre3's 'RE_LIST: /* empty */' builds
            // 'OtherSymbolTransducer(HFST_EPSILON)'. Using the two-level zero
            // placeholder 'TWOLC_EPSILON' (__HFST_TWOLC_0) instead makes the empty
            // side a literal symbol that only harmonizes when a 0:0 pair happens
            // to be in the alphabet; otherwise 'get_context' yields an unmatchable
            // context and the rule over-restricts (the center is forbidden
            // everywhere). TWOLC_EPSILON stays correct for a PAIR side such as
            // 'h:0' (handled by 'symbol_of'/'new_pair'), which is the realized
            // zero, not an empty string.
            TwolcRegex::Epsilon => OtherSymbolTransducer::new_symbol(cfg, HFST_EPSILON)?,
            TwolcRegex::Any => OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?,
            TwolcRegex::Group(inner) => self.eval_regex_with_vars(cfg, inner, vvm)?,
            TwolcRegex::Optional(inner) => {
                let mut t = self.eval_regex_with_vars(cfg, inner, vvm)?;
                t.apply_zero(cfg, |t| {
                    t.optionalize()?;
                    Ok(())
                })?;
                t
            }
            TwolcRegex::Binary(op, l, r) => self.eval_binary(cfg, *op, l, r, vvm)?,
            TwolcRegex::Unary(op, inner) => self.eval_unary(cfg, *op, inner, vvm)?,
            TwolcRegex::RepeatN(inner, n) => {
                let mut t = self.eval_regex_with_vars(cfg, inner, vvm)?;
                t.apply_num(
                    cfg,
                    |t, n| {
                        t.repeat_n(n)?;
                        Ok(())
                    },
                    *n,
                )?;
                t
            }
            TwolcRegex::RepeatNToK(inner, n, k) => {
                let mut t = self.eval_regex_with_vars(cfg, inner, vvm)?;
                t.apply_two_num(
                    cfg,
                    |t, a, b| {
                        t.repeat_n_to_k(a, b)?;
                        Ok(())
                    },
                    *n,
                    *k,
                )?;
                t
            }
        })
    }

    /// Evaluate a ['TwolcRegex::Unary'] node.
    fn eval_unary(
        &mut self,
        cfg: &OstConfig,
        op: UnaryOp,
        inner: &Spanned<TwolcRegex>,
        vvm: &VariableValueMap,
    ) -> crate::error::Result<OtherSymbolTransducer<B>> {
        let mut t = self.eval_regex_with_vars(cfg, inner, vvm)?;
        match op {
            UnaryOp::Star => {
                t.apply_zero(cfg, |t| {
                    t.repeat_star()?;
                    Ok(())
                })?;
            }
            UnaryOp::Plus => {
                t.apply_zero(cfg, |t| {
                    t.repeat_plus()?;
                    Ok(())
                })?;
            }
            UnaryOp::Reverse => {
                t.apply_zero(cfg, |t| {
                    t.reverse()?;
                    Ok(())
                })?;
            }
            UnaryOp::Invert => {
                t.apply_zero(cfg, |t| {
                    t.invert()?;
                    Ok(())
                })?;
            }
            UnaryOp::UpperProject => {
                t.apply_zero(cfg, |t| {
                    t.input_project()?;
                    Ok(())
                })?;
            }
            UnaryOp::LowerProject => {
                t.apply_zero(cfg, |t| {
                    t.output_project()?;
                    Ok(())
                })?;
            }
            UnaryOp::Complement => {
                t.negated(cfg)?;
            }
            UnaryOp::TermComplement => {
                t.term_complemented(cfg)?;
            }
            UnaryOp::Containment => {
                t.contained(cfg)?;
            }
            UnaryOp::ContainmentOnce => {
                t.contained_once(cfg)?;
            }
            UnaryOp::ContainmentOpt => {
                t.contained(cfg)?;
            }
        }
        Ok(t)
    }

    /// Evaluate a ['TwolcRegex::Binary'] node.
    fn eval_binary(
        &mut self,
        cfg: &OstConfig,
        op: BinaryOp,
        l: &Spanned<TwolcRegex>,
        r: &Spanned<TwolcRegex>,
        vvm: &VariableValueMap,
    ) -> crate::error::Result<OtherSymbolTransducer<B>> {
        let mut left = self.eval_regex_with_vars(cfg, l, vvm)?;
        let right = self.eval_regex_with_vars(cfg, r, vvm)?;
        match op {
            BinaryOp::Concatenate => {
                left.concatenate(cfg, &right)?;
            }
            BinaryOp::Union => {
                left.disjunct(cfg, &right)?;
            }
            BinaryOp::Intersect => {
                left.intersect(cfg, &right)?;
            }
            BinaryOp::Subtract => {
                left.subtract(cfg, &right)?;
            }
            BinaryOp::Compose => {
                left.apply_one_bool(
                    cfg,
                    |t, o, h| {
                        t.compose(o, h)?;
                        Ok(())
                    },
                    &right,
                )?;
            }
            other @ BinaryOp::LenientCompose
            | other @ BinaryOp::CrossProduct
            | other @ BinaryOp::MergeRight
            | other @ BinaryOp::MergeLeft
            | other @ BinaryOp::Before
            | other @ BinaryOp::After
            | other @ BinaryOp::Shuffle
            | other @ BinaryOp::UpperSubtract
            | other @ BinaryOp::LowerSubtract
            | other @ BinaryOp::UpperPriorityUnion
            | other @ BinaryOp::LowerPriorityUnion
            | other @ BinaryOp::Ignoring
            | other @ BinaryOp::IgnoreInternally
            | other @ BinaryOp::LeftQuotient => {
                std::panic::panic_any(format!(
                    "twolc regex: unsupported binary operator {other:?}"
                ));
            }
        }
        Ok(left)
    }
}

/// Map a surface symbol from the nfst-twolc AST into the internal
/// '__HFST_TWOLC_' namespace the rule machinery expects: the epsilon '0' and
/// the absolute word boundary '.#.'. (The htwolcpre1 lexer performed these
/// renamings on every symbol token; the other reserved tokens are structural
/// and never reach the AST as symbols.)
fn normalize_symbol(sym: &str) -> Symbol {
    match sym {
        "0" => Symbol::new_static(TWOLC_EPSILON),
        ".#." => Symbol::new_static("__HFST_TWOLC_.#."),
        _ => Symbol::new(sym),
    }
}

/// Substitute a single symbol via a variable assignment: if 'sym' is a variable
/// in 'vvm', return its value, else 'sym' unchanged (the 'RuleSymbolVector'
/// per-symbol 'vvm' lookup). The result is mapped into the internal symbol
/// namespace (the htwolcpre1 lexer renamings).
fn substitute_symbol(sym: &str, vvm: &VariableValueMap) -> Symbol {
    match vvm.get(sym) {
        Some(val) => normalize_symbol(val),
        None => normalize_symbol(sym),
    }
}

/// Resolve a 'TwolcRegex' operand expected to be a single symbol (the upper or
/// lower side of a 'Pair'), applying variable substitution.
fn symbol_of(e: &Spanned<TwolcRegex>, vvm: &VariableValueMap) -> Symbol {
    match &e.value {
        TwolcRegex::Symbol(s) => substitute_symbol(s, vvm),
        TwolcRegex::Epsilon => Symbol::new_static(TWOLC_EPSILON),
        TwolcRegex::Any => Symbol::new_static(TWOLC_UNKNOWN),
        TwolcRegex::Group(inner) => symbol_of(inner, vvm),
        TwolcRegex::Pair { .. }
        | TwolcRegex::Optional(_)
        | TwolcRegex::Binary(..)
        | TwolcRegex::Unary(..)
        | TwolcRegex::RepeatN(..)
        | TwolcRegex::RepeatNToK(..) => {
            std::panic::panic_any("twolc pair side must be a single symbol".to_string())
        }
    }
}

/// Build a subcase-qualified rule name from a template name + a variable
/// assignment, reproducing the 'RuleSymbolVector::replace_variables' marker
/// logic: when the assignment is non-empty, append a 'SUBCASE:' marker and one
/// ' var=value' marker per variable (so ['TwolCGrammar::get_original_name'] can
/// recover the original name by splitting at 'SUBCASE:').
fn build_rule_name(name: &str, vvm: &VariableValueMap) -> String {
    if vvm.is_empty() {
        return name.to_string();
    }
    let mut result = format!("{name} SUBCASE:");
    for (k, v) in vvm.iter() {
        result.push_str(&format!(" {k}={v}"));
    }
    result
}

// ===== body 3 (flattened, module scope) =====

// ===== integration shims: string_manipulation.cc helpers used by the rule names =====
// [spec:hfst:def:string-manipulation.replace-substr-fn]
// [spec:hfst:sem:string-manipulation.replace-substr-fn]
pub fn replace_substr(s: &str, substr: &str, replacement: &str) -> String {
    if substr.is_empty() {
        return s.to_string();
    }
    s.replace(substr, replacement)
}

// [spec:hfst:def:string-manipulation.unescape-name-fn]
// [spec:hfst:sem:string-manipulation.unescape-name-fn]
pub fn unescape_name(name: &str) -> String {
    replace_substr(
        &replace_substr(name, "__HFST_TWOLC_RULE_NAME=", ""),
        "__HFST_TWOLC_SPACE",
        " ",
    )
}

impl RuleVariables {
    pub fn new() -> Self {
        RuleVariables {
            freely_blocks: Vec::new(),
            matched_blocks: Vec::new(),
            mixed_blocks: Vec::new(),
            current_variable_block: Vec::new(),
        }
    }
}

// ===== integration: variable_src/ where-clause expansion (body[3] re-port) =====
// Ports libhfst/src/parsers/variable_src/{RuleVariables,RuleVariablesConstIterator,
// RuleSymbolVector}. The 'freely' matcher is a full cross-product; 'matched' is a
// lockstep diagonal over a block's variables (equal-size sets); 'mixed' is the
// cross-product filtered to combinations whose per-variable value POSITIONS are
// pairwise distinct (the MixedConstContainerIterator 'equal_indices' skip).
impl RuleVariables {
    // [spec:hfst:def:rule-variables.rule-variables.set-variable-fn]
    pub fn set_variable(&mut self, var: &str) {
        self.current_variable_block.push(VariableValues {
            variable: var.to_string(),
            values: Vec::new(),
        });
    }
    // [spec:hfst:def:rule-variables.rule-variables.add-value-fn]
    pub fn add_value(&mut self, value: &str) {
        if let Some(vv) = self.current_variable_block.last_mut() {
            vv.values.push(value.to_string());
        }
    }
    // [spec:hfst:def:rule-variables.rule-variables.add-values-fn]
    pub fn add_values(&mut self, values: &[String]) {
        for v in values {
            self.add_value(v);
        }
    }
    // [spec:hfst:def:rule-variables.rule-variables.set-matcher-fn]
    pub fn set_matcher(&mut self, matcher: Matcher) {
        let block = std::mem::take(&mut self.current_variable_block);
        match matcher {
            Matcher::FREELY => self.freely_blocks.push(block),
            Matcher::MATCHED => self.matched_blocks.push(block),
            Matcher::MIXED => self.mixed_blocks.push(block),
        }
    }
    // [spec:hfst:def:rule-variables.rule-variables.empty-fn]
    pub fn empty(&self) -> bool {
        self.freely_blocks.is_empty()
            && self.matched_blocks.is_empty()
            && self.mixed_blocks.is_empty()
    }
    // [spec:hfst:def:rule-variables.rule-variables.begin-fn]
    pub fn begin(&self) -> crate::error::Result<RuleVariablesConstIterator> {
        RuleVariablesConstIterator::new(self, false)
    }
    // [spec:hfst:def:rule-variables.rule-variables.end-fn]
    pub fn end(&self) -> crate::error::Result<RuleVariablesConstIterator> {
        RuleVariablesConstIterator::new(self, true)
    }
}

// One odometer dimension: a list of (variable, its values) that advance together
// (length 1 => an independent 'freely' variable; length >1 => a 'matched' block,
// lockstep over equal-size value sets). 'size' is the number of positions.
//
// For a 'mixed' block, 'combos' is non-empty: each position is a full tuple of
// per-variable value POSITIONS (pairwise distinct). For freely/matched 'combos'
// is empty and the shared position index is the value index for every variable.
#[derive(Clone)]
pub(crate) struct VarDim {
    pub(crate) vars: Vec<VariableValues>,
    pub(crate) size: usize,
    pub(crate) combos: Vec<Vec<usize>>,
}

// [spec:hfst:def:rule-variables-const-iterator.rule-variables-const-iterator]
pub struct RuleVariablesConstIterator {
    dims: Vec<VarDim>,
    indices: Vec<usize>,
    at_end: bool,
}

impl RuleVariablesConstIterator {
    fn new(rv: &RuleVariables, end: bool) -> crate::error::Result<Self> {
        let mut dims: Vec<VarDim> = Vec::new();
        // freely: every variable is its own independent dimension.
        for block in &rv.freely_blocks {
            for vv in block {
                dims.push(VarDim {
                    size: vv.values.len(),
                    vars: vec![vv.clone()],
                    combos: Vec::new(),
                });
            }
        }
        // matched: each block is one lockstep dimension (equal set sizes required).
        for block in &rv.matched_blocks {
            let size = block.first().map(|v| v.values.len()).unwrap_or(0);
            for vv in block {
                if vv.values.len() != size {
                    crate::bail!(UnequalSetSize);
                }
            }
            dims.push(VarDim {
                size,
                vars: block.clone(),
                combos: Vec::new(),
            });
        }
        // mixed: one dimension whose positions are the cross-product of the
        // block's per-variable value POSITIONS, keeping only the tuples whose
        // positions are pairwise distinct (the C++ 'equal_indices' skip). The
        // odometer below advances position 0 fastest, mirroring
        // ConstContainerIterator::operator++.
        for block in &rv.mixed_blocks {
            let lens: Vec<usize> = block.iter().map(|vv| vv.values.len()).collect();
            let n = lens.len();
            let mut combos: Vec<Vec<usize>> = Vec::new();
            if n != 0 && lens.iter().all(|&l| l != 0) {
                let mut idx = vec![0usize; n];
                loop {
                    let mut seen: std::collections::BTreeSet<usize> =
                        std::collections::BTreeSet::new();
                    let mut distinct = true;
                    for &k in &idx {
                        if !seen.insert(k) {
                            distinct = false;
                            break;
                        }
                    }
                    if distinct {
                        combos.push(idx.clone());
                    }
                    // advance the mixed-radix odometer (position 0 fastest)
                    let mut i = 0;
                    while i < n {
                        idx[i] += 1;
                        if idx[i] < lens[i] {
                            break;
                        }
                        idx[i] = 0;
                        i += 1;
                    }
                    if i == n {
                        break;
                    }
                }
            }
            dims.push(VarDim {
                size: combos.len(),
                vars: block.clone(),
                combos,
            });
        }
        let any_empty = dims.iter().any(|d| d.size == 0);
        let indices = vec![0usize; dims.len()];
        Ok(RuleVariablesConstIterator {
            dims,
            indices,
            at_end: end || any_empty,
        })
    }

    // [spec:hfst:def:rule-variables-const-iterator.rule-variables-const-iterator.set-values-fn]
    pub fn set_values(&self, vvm: &mut VariableValueMap) {
        for (dim, &idx) in self.dims.iter().zip(self.indices.iter()) {
            if dim.combos.is_empty() {
                // freely / matched: every variable shares the position index.
                for vv in &dim.vars {
                    vvm.insert(vv.variable.clone(), vv.values[idx].clone());
                }
            } else {
                // mixed: each variable takes its own position from the tuple.
                let combo = &dim.combos[idx];
                for (k, vv) in dim.vars.iter().enumerate() {
                    vvm.insert(vv.variable.clone(), vv.values[combo[k]].clone());
                }
            }
        }
    }

    pub fn increment(&mut self) {
        if self.at_end {
            return;
        }
        // odometer: advance the rightmost dimension, carry to the left.
        let mut i = self.dims.len();
        loop {
            if i == 0 {
                self.at_end = true;
                return;
            }
            i -= 1;
            self.indices[i] += 1;
            if self.indices[i] < self.dims[i].size {
                return;
            }
            self.indices[i] = 0;
        }
    }

    pub fn ne(&self, other: &RuleVariablesConstIterator) -> bool {
        self.at_end != other.at_end || (!self.at_end && self.indices != other.indices)
    }
}
