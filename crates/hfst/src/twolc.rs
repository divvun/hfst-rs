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
//! 'output_symbols', 'diacritics', 'symbol_pairs', 'transducer_type') and the
//! per-container conflict flags ('report_*_conflicts', 'resolve_*_conflicts')
//! were 'static' in C++. Because this port walks the AST and is re-entrant,
//! they are carried as thread-local module statics (the
//! 'OtherSymbolTransducer' config) and as instance fields on the containers
//! (the conflict flags), instead of process-wide mutable statics.
//!
//! # Conventions
//!
//! 'std::set' -> 'BTreeSet', 'std::map' -> 'BTreeMap', 'std::vector' -> 'Vec',
//! 'std::pair<A,B>' -> '(A,B)'. 'HandyMap'/'HandySet' 'has_key'/'has_element'
//! become 'contains_key'/'contains'. C++ owning 'HfstTransducer*' -> owned
//! values / 'Box'. C++ virtual dispatch over the 'Rule' hierarchy -> a Rust
//! ['RuleT'] trait object ('Box<dyn RuleT>'). C++ 'throw' ->
//! 'std::panic::panic_any' of the typed exception. Every C++
//! '// [spec:hfst:def/sem:<id>]' annotation is carried onto its Rust site.
//!
//! # Deferred (record as 'unimplemented! ("deferred: ...")')
//!
//! - The 'HfstOutputStream' binary store paths: 'Rule::store',
//!   'RuleContainer::store' and the stream side of
//!   'TwolCGrammar::compile_and_store'. No binary stream I/O yet; the driver
//!   instead returns the assembled result transducer so a smoke can run.

#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)]
#![allow(clippy::too_many_arguments)]

use std::collections::{BTreeMap, BTreeSet};

#[allow(unused_imports)]
use nfst_twolc::{
    AlphabetPair, RuleCenter, RuleContext, RuleOp, SetDefinition, Spanned, TwolcDefinition,
    TwolcFile, TwolcRegex, TwolcRule, VarMatcher, VariableAssignment, VariableBlock,
};
#[allow(unused_imports)]
use nfst_xre::{BinaryOp, UnaryOp};

#[allow(unused_imports)]
use crate::hfst_data_types::{ImplementationType, StringPair, StringPairVector, StringVector};
use crate::hfst_transducer::HfstTransducer;

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
pub type SymbolRange = Vec<String>;
pub type SymbolPairVector = StringPairVector;
pub type OtherSymbolTransducerVector = Vec<OtherSymbolTransducer>;
pub type VariableValueMap = BTreeMap<String, String>;
pub type RuleCenterPair = (String, String);

// [spec:hfst:def:variable-defs.matcher]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Matcher {
    FREELY,
    MATCHED,
    MIXED,
}

// Typed empty-marker "exceptions" (thrown via panic_any):
// [spec:hfst:def:other-symbol-transducer.empty-symbol-pair-set]
#[derive(Clone, Copy, Debug)]
pub struct EmptySymbolPairSet;
// [spec:hfst:def:other-symbol-transducer.undefined-symbol-pairs-found]
#[derive(Clone, Copy, Debug)]
pub struct UndefinedSymbolPairsFound;
// [spec:hfst:def:variable-defs.empty-container]
#[derive(Clone, Copy, Debug)]
pub struct EmptyContainer;
// [spec:hfst:def:variable-defs.unequal-set-size]
#[derive(Clone, Copy, Debug)]
pub struct UnequalSetSize;

// [spec:hfst:def:other-symbol-transducer.other-symbol-transducer]
pub struct OtherSymbolTransducer {
    pub(crate) is_broken: bool,
    pub(crate) transducer: HfstTransducer,
}
// thread-local config replacing the five OtherSymbolTransducer statics:
pub(crate) struct OstConfig {
    pub(crate) input_symbols: BTreeSet<String>,
    pub(crate) output_symbols: BTreeSet<String>,
    pub(crate) diacritics: BTreeSet<String>,
    pub(crate) symbol_pairs: BTreeSet<SymbolPair>,
    pub(crate) transducer_type: ImplementationType,
}

// Rule hierarchy — RuleT trait + one struct per C++ subclass:
pub trait RuleT {
    // [spec:hfst:def:rule.rule.compile-fn]
    fn compile(&mut self) -> OtherSymbolTransducer;
    fn rule(&self) -> &Rule;
    fn rule_mut(&mut self) -> &mut Rule;
    fn rule_transducer(&self) -> &OtherSymbolTransducer {
        &self.rule().rule_transducer
    }
}
// [spec:hfst:def:rule.rule]
pub struct Rule {
    pub(crate) is_empty: bool,
    pub(crate) name: String,
    pub(crate) center: OtherSymbolTransducer,
    pub(crate) context: OtherSymbolTransducer,
    pub(crate) rule_transducer: OtherSymbolTransducer,
}
pub struct ResultRule {
    pub(crate) base: Rule,
}
// [spec:hfst:def:right-arrow-rule.right-arrow-rule]
pub struct RightArrowRule {
    pub(crate) base: Rule,
}
// [spec:hfst:def:left-arrow-rule.left-arrow-rule]
pub struct LeftArrowRule {
    pub(crate) base: Rule,
}
// [spec:hfst:def:left-restriction-arrow-rule.left-restriction-arrow-rule]
pub struct LeftRestrictionArrowRule {
    pub(crate) base: Rule,
}
// [spec:hfst:def:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule]
pub struct ConflictResolvingRightArrowRule {
    pub(crate) base: RightArrowRule,
    pub(crate) center_pair: SymbolPair,
}
// [spec:hfst:def:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule]
pub struct ConflictResolvingLeftArrowRule {
    pub(crate) base: LeftArrowRule,
    pub(crate) input_symbol: String,
}

// Containers:
// [spec:hfst:def:rule-container.rule-container]
pub struct RuleContainer {
    pub(crate) report: bool,
    pub(crate) rule_vector: Vec<Box<dyn RuleT>>,
}
// [spec:hfst:def:right-arrow-rule-container.right-arrow-rule-container]
pub struct RightArrowRuleContainer {
    pub(crate) base: RuleContainer,
    pub(crate) report_right_arrow_conflicts: bool,
    pub(crate) resolve_right_arrow_conflicts: bool,
    pub(crate) center_to_rule_map: BTreeMap<SymbolPair, usize>,
}
// [spec:hfst:def:left-arrow-rule-container.left-arrow-rule-container]
pub struct LeftArrowRuleContainer {
    pub(crate) base: RuleContainer,
    pub(crate) report_left_arrow_conflicts: bool,
    pub(crate) resolve_left_arrow_conflicts: bool,
    pub(crate) input_to_rule_map: BTreeMap<String, Vec<usize>>,
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
pub struct TwolCGrammar {
    pub(crate) be_quiet: bool,
    pub(crate) be_verbose: bool,
    pub(crate) name_to_rule_subcases: BTreeMap<String, BTreeSet<RuleHandle>>,
    pub(crate) left_arrow_rule_container: LeftArrowRuleContainer,
    pub(crate) right_arrow_rule_container: RightArrowRuleContainer,
    pub(crate) other_rule_container: RuleContainer,
    pub(crate) compiled_rule_container: RuleContainer,
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

// TwolcCompiler — entry point:
// [spec:hfst:def:twolc-compiler.hfst.twolc.twolc-compiler]
pub struct TwolcCompiler {
    pub(crate) format: ImplementationType,
    pub(crate) silent: bool,
    pub(crate) verbose: bool,
    pub(crate) resolve_left_conflicts: bool,
    pub(crate) resolve_right_conflicts: bool,
    pub(crate) sets: BTreeMap<String, SymbolRange>,
    pub(crate) definitions: BTreeMap<String, OtherSymbolTransducer>,
}

// (followed by the full ~190-line doc roster of method/helper signatures with
//  their [spec:hfst:def:...] ids — see the file.)

// ===== body 0 (flattened, module scope) =====
// ───────────────────────────────────────────────────────────────────────────
// OtherSymbolTransducer — 'rule_src/OtherSymbolTransducer.{h,cc}' bodies.
//
// The C++ class kept its alphabet config in five 'static' members; this port
// folds them into the thread-local 'OST_CONFIG' cell (mirroring xre.rs's
// 'CONTAINS_ONLY_COMMENTS' thread_local pattern) so the AST-walk driver is
// re-entrant. The 'apply(member-fn-ptr, ...)' overload family is flattened to
// the named 'apply_*' methods plus concrete readable shims the rule bodies call.
// ───────────────────────────────────────────────────────────────────────────

use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_data_types::implementations::HfstState;

thread_local! {
    /// The former 'OtherSymbolTransducer' 'static' class config (the five C++
    /// statics: 'input_symbols'/'output_symbols'/'diacritics'/'symbol_pairs'/
    /// 'transducer_type'), folded into one thread-local cell.
    static OST_CONFIG: std::cell::RefCell<OstConfig> = std::cell::RefCell::new(OstConfig {
        input_symbols: BTreeSet::new(),
        output_symbols: BTreeSet::new(),
        diacritics: BTreeSet::new(),
        symbol_pairs: BTreeSet::new(),
        transducer_type: ImplementationType::TROPICAL_OPENFST_TYPE,
    });
}

impl OtherSymbolTransducer {
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
    pub fn set_symbol_pairs(symbol_pairs: &BTreeSet<SymbolPair>) {
        OST_CONFIG.with(|c| {
            let mut cfg = c.borrow_mut();
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
            cfg.symbol_pairs
                .insert((TWOLC_DIAMOND.to_string(), TWOLC_DIAMOND.to_string()));
        });
    }

    /// 'static void define_diacritics(const std::vector<std::string> &diacritics)'.
    ///
    /// Records the diacritics and erases their identity / 'X:0' pairs and the
    /// matching input/output symbols from the alphabet config.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.define-diacritics-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.define-diacritics-fn]
    pub fn define_diacritics(diacritics: &[String]) {
        OST_CONFIG.with(|c| {
            let mut cfg = c.borrow_mut();
            cfg.diacritics.clear();
            for d in diacritics.iter() {
                cfg.diacritics.insert(d.clone());
            }
            // Iterate over a snapshot of the diacritics so the mutable erases
            // below do not alias the loop (the C++ iterates 'diacritics' while
            // mutating 'symbol_pairs'/'input_symbols'/'output_symbols').
            let diac: Vec<String> = cfg.diacritics.iter().cloned().collect();
            for it in diac.iter() {
                cfg.symbol_pairs.remove(&(it.clone(), it.clone()));
                cfg.symbol_pairs
                    .remove(&(it.clone(), TWOLC_EPSILON.to_string()));
                cfg.input_symbols.remove(it);
                cfg.output_symbols.remove(it);
            }
        });
    }

    /// 'static void set_transducer_type(ImplementationType transducer_type)'.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.set-transducer-type-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.set-transducer-type-fn]
    pub fn set_transducer_type(transducer_type: ImplementationType) {
        OST_CONFIG.with(|c| c.borrow_mut().transducer_type = transducer_type);
    }

    /// Read the thread-local 'transducer_type'.
    pub(crate) fn config_transducer_type() -> ImplementationType {
        OST_CONFIG.with(|c| c.borrow().transducer_type)
    }

    // -------------------------------------------------------------------------
    // ----- Constructors -----
    // -------------------------------------------------------------------------

    /// 'OtherSymbolTransducer(void)' — empty transducer of the configured type.
    pub fn new() -> Self {
        OtherSymbolTransducer {
            is_broken: false,
            transducer: HfstTransducer::new_type(Self::config_transducer_type()),
        }
    }

    /// 'OtherSymbolTransducer(const std::string &i_symbol,
    ///  const std::string &o_symbol)' — build 'input_symbol:output_symbol'.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.other-symbol-transducer-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.other-symbol-transducer-fn]
    pub fn new_pair(i_symbol: &str, o_symbol: &str) -> Self {
        let transducer_type = Self::config_transducer_type();
        let mut input_symbol = i_symbol.to_string();
        let mut output_symbol = o_symbol.to_string();

        if input_symbol == TWOLC_UNKNOWN {
            input_symbol = HFST_UNKNOWN.to_string();
        }
        if output_symbol == TWOLC_UNKNOWN {
            output_symbol = HFST_UNKNOWN.to_string();
        }

        let mut this = OtherSymbolTransducer {
            is_broken: false,
            transducer: HfstTransducer::new_type(transducer_type),
        };
        this.check_pair(&input_symbol, &output_symbol);
        if this.is_broken {
            return this;
        }
        if input_symbol == HFST_UNKNOWN && output_symbol == HFST_UNKNOWN {
            this.transducer = Self::get_universal().transducer;
        } else {
            let mut fst = HfstBasicTransducer::from_transducer(&this.transducer);
            let target = fst.add_state_new();
            fst.set_final_weight(target, &0.0);

            if input_symbol == HFST_UNKNOWN {
                let input_symbols = OST_CONFIG
                    .with(|c| c.borrow().input_symbols.iter().cloned().collect::<Vec<_>>());
                let pairs = OST_CONFIG.with(|c| c.borrow().symbol_pairs.clone());
                for it in input_symbols.iter() {
                    if pairs.contains(&(it.clone(), output_symbol.clone())) {
                        fst.add_transition(
                            0,
                            &HfstBasicTransition::new_symbols(
                                target,
                                it.clone(),
                                output_symbol.clone(),
                                0.0,
                            ),
                            true,
                        );
                    }
                }
            } else if output_symbol == HFST_UNKNOWN {
                let output_symbols = OST_CONFIG.with(|c| {
                    c.borrow()
                        .output_symbols
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                });
                let pairs = OST_CONFIG.with(|c| c.borrow().symbol_pairs.clone());
                for it in output_symbols.iter() {
                    if pairs.contains(&(input_symbol.clone(), it.clone())) {
                        fst.add_transition(
                            0,
                            &HfstBasicTransition::new_symbols(
                                target,
                                input_symbol.clone(),
                                it.clone(),
                                0.0,
                            ),
                            true,
                        );
                    }
                }
            } else {
                fst.add_transition(
                    0,
                    &HfstBasicTransition::new_symbols(
                        target,
                        input_symbol.clone(),
                        output_symbol.clone(),
                        0.0,
                    ),
                    true,
                );
            }
            this.transducer = HfstTransducer::new_from_basic(&fst, transducer_type);
        }
        this
    }

    /// 'OtherSymbolTransducer(const std::string &sym)' — build 'symbol:symbol'
    /// (or 'symbol:0' for a diacritic).
    pub fn new_symbol(sym: &str) -> Self {
        let transducer_type = Self::config_transducer_type();
        let mut symbol = sym.to_string();
        if symbol == TWOLC_UNKNOWN {
            symbol = HFST_UNKNOWN.to_string();
        }

        let is_diacritic = OST_CONFIG.with(|c| c.borrow().diacritics.contains(&symbol));

        let mut this = OtherSymbolTransducer {
            is_broken: false,
            transducer: HfstTransducer::new_type(transducer_type),
        };
        if is_diacritic {
            this.check_pair(&symbol, TWOLC_EPSILON);
        } else {
            this.check_pair(&symbol, &symbol);
        }

        if this.is_broken {
            return this;
        }

        if symbol == HFST_UNKNOWN {
            this.transducer = Self::get_universal().transducer;
        } else if is_diacritic {
            this.transducer =
                HfstTransducer::new_symbol_pair(&symbol, TWOLC_EPSILON, transducer_type);
        } else {
            this.transducer = HfstTransducer::new_symbol(&symbol, transducer_type);
        }
        this
    }

    // -------------------------------------------------------------------------
    // ----- Protected helpers -----
    // -------------------------------------------------------------------------

    /// 'void check_pair(const std::string &input_symbol,
    ///  const std::string &output_symbol)' — set 'is_broken' if the pair is not
    /// in the configured alphabet (and report to stderr).
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.check-pair-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.check-pair-fn]
    pub fn check_pair(&mut self, input_symbol: &str, output_symbol: &str) {
        OST_CONFIG.with(|c| {
            let cfg = c.borrow();
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
                    .contains(&(input_symbol.to_string(), output_symbol.to_string()));
            }
        });
        if self.is_broken {
            eprintln!("Unknown pair: {} {}", input_symbol, output_symbol);
        }
    }

    /// 'void add_diamond_transition(void)' — 'add_symbol_to_alphabet(DIAMOND)'.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.add-diamond-transition-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.add-diamond-transition-fn]
    pub fn add_diamond_transition(&mut self) {
        self.add_symbol_to_alphabet(TWOLC_DIAMOND);
    }

    /// 'static bool empty(const HfstBasicTransducer &fsm)' — true iff no
    /// reachable final state (the C++ scans every state for a final marker).
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.empty-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.empty-fn]
    pub fn empty(fsm: &HfstBasicTransducer) -> bool {
        let mut state: HfstState = 0;
        for _ in fsm.iter() {
            if fsm.is_final_state(state) {
                return false;
            }
            state += 1;
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
    fn config_symbol_pairs_empty() -> bool {
        OST_CONFIG.with(|c| c.borrow().symbol_pairs.is_empty())
    }

    /// True iff there are configured diacritics.
    fn config_has_diacritics() -> bool {
        OST_CONFIG.with(|c| !c.borrow().diacritics.is_empty())
    }

    /// 'apply(HfstTransducerZeroArgMember p)' — apply a zero-arg 'HfstTransducer'
    /// op then minimize. The C++ member-fn-pointer becomes a closure.
    pub fn apply_zero<F>(&mut self, p: F) -> &mut Self
    where
        F: FnOnce(&mut HfstTransducer),
    {
        if Self::config_symbol_pairs_empty() {
            std::panic::panic_any(EmptySymbolPairSet);
        }
        if self.is_broken {
            std::panic::panic_any(UndefinedSymbolPairsFound);
        }
        p(&mut self.transducer);
        self.transducer.minimize();
        self
    }

    /// 'apply(const HfstTransducerOneArgMember, const OtherSymbolTransducer&)'.
    ///
    /// Harmonizes the diacritics of '*this' and a copy of 'another' (when there
    /// are diacritics), applies the binary op against the copy, then minimizes.
    /// The C++ facade ops default to 'harmonize = true', which the closure
    /// passes through.
    pub fn apply_one<F>(&mut self, p: F, another: &OtherSymbolTransducer) -> &mut Self
    where
        F: FnOnce(&mut HfstTransducer, &HfstTransducer),
    {
        if Self::config_symbol_pairs_empty() {
            std::panic::panic_any(EmptySymbolPairSet);
        }
        if self.is_broken {
            std::panic::panic_any(UndefinedSymbolPairsFound);
        }
        if another.is_broken {
            std::panic::panic_any(UndefinedSymbolPairsFound);
        }
        // [spec:hfst:def:other-symbol-transducer.another-copy-fn]
        // [spec:hfst:sem:other-symbol-transducer.another-copy-fn]
        let mut another_copy = another.clone();
        if Self::config_has_diacritics() {
            self.harmonize_diacritics(&mut another_copy);
            another_copy.harmonize_diacritics(self);
        }
        p(&mut self.transducer, &another_copy.transducer);
        self.transducer.minimize();
        self
    }

    /// 'apply(const HfstTransducerBoolArgMember, const OtherSymbolTransducer&)'.
    ///
    /// Like ['apply_one'] but the closure carries the trailing 'bool' (the C++
    /// passes 'true').
    pub fn apply_one_bool<F>(&mut self, p: F, another: &OtherSymbolTransducer) -> &mut Self
    where
        F: FnOnce(&mut HfstTransducer, &HfstTransducer, bool),
    {
        if Self::config_symbol_pairs_empty() {
            std::panic::panic_any(EmptySymbolPairSet);
        }
        if self.is_broken {
            std::panic::panic_any(UndefinedSymbolPairsFound);
        }
        if another.is_broken {
            std::panic::panic_any(UndefinedSymbolPairsFound);
        }
        let mut another_copy = another.clone();
        if Self::config_has_diacritics() {
            self.harmonize_diacritics(&mut another_copy);
            another_copy.harmonize_diacritics(self);
        }
        p(&mut self.transducer, &another_copy.transducer, true);
        self.transducer.minimize();
        self
    }

    /// 'bool apply(const HfstTransducerOneArgMemberBool,
    ///  const OtherSymbolTransducer&) const'.
    ///
    /// Runs the predicate against copies of both transducers and returns its
    /// result (no minimize; the C++ overload is 'const').
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.apply-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.apply-fn]
    pub fn apply_one_bool_ret<F>(&self, p: F, another: &OtherSymbolTransducer) -> bool
    where
        F: FnOnce(&mut HfstTransducer, &HfstTransducer) -> bool,
    {
        if Self::config_symbol_pairs_empty() {
            std::panic::panic_any(EmptySymbolPairSet);
        }
        if self.is_broken {
            std::panic::panic_any(UndefinedSymbolPairsFound);
        }
        if another.is_broken {
            std::panic::panic_any(UndefinedSymbolPairsFound);
        }
        let mut copy = self.clone();
        let another_copy = another.clone();
        p(&mut copy.transducer, &another_copy.transducer)
    }

    /// 'apply(const HfstTransducerOneNumArgMember, unsigned int number)'.
    pub fn apply_num<F>(&mut self, p: F, number: u32) -> &mut Self
    where
        F: FnOnce(&mut HfstTransducer, u32),
    {
        if Self::config_symbol_pairs_empty() {
            std::panic::panic_any(EmptySymbolPairSet);
        }
        if self.is_broken {
            std::panic::panic_any(UndefinedSymbolPairsFound);
        }
        p(&mut self.transducer, number);
        self.transducer.minimize();
        self
    }

    /// 'apply(const HfstTransducerTwoNumArgMember, unsigned int, unsigned int)'.
    pub fn apply_two_num<F>(&mut self, p: F, num1: u32, num2: u32) -> &mut Self
    where
        F: FnOnce(&mut HfstTransducer, u32, u32),
    {
        if Self::config_symbol_pairs_empty() {
            std::panic::panic_any(EmptySymbolPairSet);
        }
        if self.is_broken {
            std::panic::panic_any(UndefinedSymbolPairsFound);
        }
        p(&mut self.transducer, num1, num2);
        self.transducer.minimize();
        self
    }

    /// 'apply(const HfstTransducerOneSymbolPairArgMember, const SymbolPair&)'.
    pub fn apply_symbol_pair<F>(&mut self, p: F, pair: &SymbolPair) -> &mut Self
    where
        F: FnOnce(&mut HfstTransducer, &SymbolPair),
    {
        if Self::config_symbol_pairs_empty() {
            std::panic::panic_any(EmptySymbolPairSet);
        }
        if self.is_broken {
            std::panic::panic_any(UndefinedSymbolPairsFound);
        }
        p(&mut self.transducer, pair);
        self.transducer.minimize();
        self
    }

    /// 'apply(const HfstTransducerOneSymbolPairBoolArgMember,
    ///  const SymbolPair&, bool)'.
    pub fn apply_symbol_pair_bool<F>(&mut self, p: F, pair: &SymbolPair, b: bool) -> &mut Self
    where
        F: FnOnce(&mut HfstTransducer, &SymbolPair, bool),
    {
        if Self::config_symbol_pairs_empty() {
            std::panic::panic_any(EmptySymbolPairSet);
        }
        if self.is_broken {
            std::panic::panic_any(UndefinedSymbolPairsFound);
        }
        p(&mut self.transducer, pair, b);
        self.transducer.minimize();
        self
    }

    /// 'apply(const HfstTransducerSubstMember, const std::string&,
    ///  const std::string&, bool, bool)' — 'substitute(str1, str2, b1, b2)'.
    pub fn apply_subst(&mut self, str1: &str, str2: &str, b1: bool, b2: bool) -> &mut Self {
        if Self::config_symbol_pairs_empty() {
            std::panic::panic_any(EmptySymbolPairSet);
        }
        if self.is_broken {
            std::panic::panic_any(UndefinedSymbolPairsFound);
        }
        self.transducer.substitute_string(str1, str2, b1, b2);
        self.transducer.minimize();
        self
    }

    /// 'apply(const HfstTransducerSubstPairMember, const SymbolPair&,
    ///  const SymbolPair&)' — 'substitute(pair1, pair2)'.
    pub fn apply_subst_pair(&mut self, p1: &SymbolPair, p2: &SymbolPair) -> &mut Self {
        if Self::config_symbol_pairs_empty() {
            std::panic::panic_any(EmptySymbolPairSet);
        }
        if self.is_broken {
            std::panic::panic_any(UndefinedSymbolPairsFound);
        }
        self.transducer.substitute_pair_with_pair(p1, p2);
        self.transducer.minimize();
        self
    }

    /// 'apply(const HfstTransducerSubstPairFstMember, const SymbolPair&,
    ///  const OtherSymbolTransducer&, bool)' —
    /// 'substitute(pair, t_copy.transducer, b)'.
    // [spec:hfst:def:other-symbol-transducer.t-copy-fn]
    // [spec:hfst:sem:other-symbol-transducer.t-copy-fn]
    pub fn apply_subst_pair_fst(
        &mut self,
        p1: &SymbolPair,
        t: &OtherSymbolTransducer,
        b: bool,
    ) -> &mut Self {
        if Self::config_symbol_pairs_empty() {
            std::panic::panic_any(EmptySymbolPairSet);
        }
        if self.is_broken {
            std::panic::panic_any(UndefinedSymbolPairsFound);
        }
        let mut t_copy = t.clone();
        self.transducer
            .substitute_symbol_pair_with_transducer(p1, &mut t_copy.transducer, b);
        self.transducer.minimize();
        self
    }

    // -------------------------------------------------------------------------
    // ----- Concrete convenience shims (readable call sites for the rules) -----
    // -------------------------------------------------------------------------

    /// 'apply(&HfstTransducer::disjunct, another)'.
    pub fn disjunct(&mut self, another: &OtherSymbolTransducer) -> &mut Self {
        self.apply_one(
            |t, o| {
                t.disjunct(o, true);
            },
            another,
        )
    }

    /// 'apply(&HfstTransducer::intersect, another)'.
    pub fn intersect(&mut self, another: &OtherSymbolTransducer) -> &mut Self {
        self.apply_one(
            |t, o| {
                t.intersect(o, true);
            },
            another,
        )
    }

    /// 'apply(&HfstTransducer::subtract, another)'.
    pub fn subtract(&mut self, another: &OtherSymbolTransducer) -> &mut Self {
        self.apply_one(
            |t, o| {
                t.subtract(o, true);
            },
            another,
        )
    }

    /// 'apply(&HfstTransducer::concatenate, another)'.
    pub fn concatenate(&mut self, another: &OtherSymbolTransducer) -> &mut Self {
        self.apply_one(
            |t, o| {
                t.concatenate(o, true);
            },
            another,
        )
    }

    /// 'apply(&HfstTransducer::compose, another)'.
    pub fn compose(&mut self, another: &OtherSymbolTransducer) -> &mut Self {
        self.apply_one(
            |t, o| {
                t.compose(o, true);
            },
            another,
        )
    }

    /// 'apply(&HfstTransducer::insert_freely, another)' (bool-arg overload).
    pub fn insert_freely(&mut self, another: &OtherSymbolTransducer) -> &mut Self {
        self.apply_one_bool(
            |t, o, h| {
                t.insert_freely(o, h);
            },
            another,
        )
    }

    /// 'apply(&HfstTransducer::repeat_star)'.
    pub fn repeat_star(&mut self) -> &mut Self {
        self.apply_zero(|t| {
            t.repeat_star();
        })
    }

    /// 'apply(&HfstTransducer::minimize)'.
    pub fn minimize(&mut self) -> &mut Self {
        self.apply_zero(|t| {
            t.minimize();
        })
    }

    /// 'apply(&HfstTransducer::optionalize)'.
    pub fn optionalize(&mut self) -> &mut Self {
        self.apply_zero(|t| {
            t.optionalize();
        })
    }

    /// 'apply(&HfstTransducer::invert)'.
    pub fn invert(&mut self) -> &mut Self {
        self.apply_zero(|t| {
            t.invert();
        })
    }

    /// 'apply(&HfstTransducer::input_project)'.
    pub fn input_project(&mut self) -> &mut Self {
        self.apply_zero(|t| {
            t.input_project();
        })
    }

    /// 'apply(&HfstTransducer::output_project)'.
    pub fn output_project(&mut self) -> &mut Self {
        self.apply_zero(|t| {
            t.output_project();
        })
    }

    /// 'apply(&HfstTransducer::repeat_n, n)'.
    pub fn repeat_n(&mut self, n: u32) -> &mut Self {
        self.apply_num(
            |t, n| {
                t.repeat_n(n);
            },
            n,
        )
    }

    /// 'apply(&HfstTransducer::repeat_n_to_k, n, k)'.
    pub fn repeat_n_to_k(&mut self, n: u32, k: u32) -> &mut Self {
        self.apply_two_num(
            |t, n, k| {
                t.repeat_n_to_k(n, k);
            },
            n,
            k,
        )
    }

    /// Replace the diamond pair '(DIAMOND, DIAMOND)' with the HFST epsilon on
    /// both sides: 'substitute(DIAMOND, HFST_EPSILON, true, true)'.
    pub fn substitute_diamond_to_epsilon(&mut self) -> &mut Self {
        self.apply_subst(TWOLC_DIAMOND, HFST_EPSILON, true, true)
    }

    // -------------------------------------------------------------------------
    // ----- Other instance / static ops -----
    // -------------------------------------------------------------------------

    /// 'OtherSymbolTransducer &harmonize_diacritics(OtherSymbolTransducer &t)'.
    ///
    /// For each diacritic present in 't''s alphabet but missing from '*this''s,
    /// add a 'd:d' self-loop-style transition alongside every 'TWOLC_IDENTITY'
    /// transition leaving a state.
    pub fn harmonize_diacritics(&mut self, t: &mut OtherSymbolTransducer) -> &mut Self {
        // [spec:hfst:def:other-symbol-transducer.basic-fn]
        // [spec:hfst:sem:other-symbol-transducer.basic-fn]
        let mut basic = HfstBasicTransducer::from_transducer(&self.transducer);
        let alphabet: BTreeSet<String> = basic.get_alphabet().clone();

        let basic_t = HfstBasicTransducer::from_transducer(&t.transducer);
        let t_alphabet: BTreeSet<String> = basic_t.get_alphabet().clone();

        let mut missing_diacritics: BTreeSet<String> = BTreeSet::new();
        OST_CONFIG.with(|c| {
            for it in c.borrow().diacritics.iter() {
                if t_alphabet.contains(it) && !alphabet.contains(it) {
                    missing_diacritics.insert(it.clone());
                }
            }
        });
        if missing_diacritics.is_empty() {
            return self;
        }

        // For every state, if it has a TWOLC_IDENTITY-input transition, add a
        // diacritic self-pair transition to that transition's target for each
        // missing diacritic (the C++ 'break's after the first identity arc).
        let num_states = basic.states_and_transitions().len();
        for s in 0..num_states {
            let mut identity_target: Option<HfstState> = None;
            for jt in basic.index(s as HfstState).iter() {
                if jt.get_input_symbol() == TWOLC_IDENTITY {
                    identity_target = Some(jt.get_target_state());
                    break;
                }
            }
            if let Some(target) = identity_target {
                for kt in missing_diacritics.iter() {
                    basic.add_transition(
                        s as HfstState,
                        &HfstBasicTransition::new_symbols(target, kt.clone(), kt.clone(), 0.0),
                        true,
                    );
                }
            }
        }
        self.transducer = HfstTransducer::new_from_basic(&basic, Self::config_transducer_type());
        self
    }

    /// 'static OtherSymbolTransducer get_context(OtherSymbolTransducer &left,
    ///  OtherSymbolTransducer &right)' — build '?* X D ?* D Y ?*'.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.get-context-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.get-context-fn]
    pub fn get_context(
        left: &mut OtherSymbolTransducer,
        right: &mut OtherSymbolTransducer,
    ) -> OtherSymbolTransducer {
        let mut universal = Self::get_universal();
        universal.apply_zero(|t| {
            t.repeat_star();
        });
        let mut result = universal.clone();
        let diamond = OtherSymbolTransducer::new_symbol(TWOLC_DIAMOND);
        universal.apply_zero(|t| {
            t.repeat_star();
        });

        result
            .concatenate(left)
            .concatenate(&diamond)
            .concatenate(&universal)
            .concatenate(&diamond)
            .concatenate(right)
            .concatenate(&universal);
        result
    }

    /// 'static OtherSymbolTransducer get_universal(void)' — a one-symbol
    /// transducer recognizing the identity pair plus every configured pair
    /// except the diamond.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.get-universal-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.get-universal-fn]
    pub fn get_universal() -> OtherSymbolTransducer {
        let transducer_type = Self::config_transducer_type();
        let universal = OtherSymbolTransducer {
            is_broken: false,
            transducer: HfstTransducer::new_type(transducer_type),
        };
        let mut fst = HfstBasicTransducer::from_transducer(&universal.transducer);
        let target = fst.add_state_new();
        fst.set_final_weight(target, &0.0);
        fst.add_transition(
            0,
            &HfstBasicTransition::new_symbols(
                target,
                TWOLC_IDENTITY.to_string(),
                TWOLC_IDENTITY.to_string(),
                0.0,
            ),
            true,
        );
        let pairs = OST_CONFIG.with(|c| c.borrow().symbol_pairs.clone());
        for it in pairs.iter() {
            if it.0 == TWOLC_DIAMOND {
                continue;
            }
            fst.add_transition(
                0,
                &HfstBasicTransition::new_symbols(target, it.0.clone(), it.1.clone(), 0.0),
                true,
            );
        }
        OtherSymbolTransducer {
            is_broken: false,
            transducer: HfstTransducer::new_from_basic(&fst, transducer_type),
        }
    }

    /// 'void add_symbol_to_alphabet(const std::string &symbol)' — round-trip
    /// through the basic transducer to add 'symbol' (prevents harmonization).
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.add-symbol-to-alphabet-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.add-symbol-to-alphabet-fn]
    pub fn add_symbol_to_alphabet(&mut self, symbol: &str) {
        let mut mutable_transducer = HfstBasicTransducer::from_transducer(&self.transducer);
        mutable_transducer.add_symbol_to_alphabet(&symbol.to_string());
        self.transducer =
            HfstTransducer::new_from_basic(&mutable_transducer, Self::config_transducer_type());
    }

    /// 'void remove_diacritics_from_output(void)' — for each diacritic, rewrite
    /// 'd:d' to 'd:0'.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.remove-diacritics-from-output-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.remove-diacritics-from-output-fn]
    pub fn remove_diacritics_from_output(&mut self) {
        let diac = OST_CONFIG.with(|c| c.borrow().diacritics.iter().cloned().collect::<Vec<_>>());
        for it in diac.iter() {
            self.apply_subst_pair(
                &(it.clone(), it.clone()),
                &(it.clone(), TWOLC_EPSILON.to_string()),
            );
        }
    }

    /// 'OtherSymbolTransducer &add_info_symbol(const std::string &info_symbol)'
    /// — append 'info_symbol' to the wrapped transducer's name.
    pub fn add_info_symbol(&mut self, info_symbol: &str) -> &mut Self {
        if self.is_broken {
            std::panic::panic_any(UndefinedSymbolPairsFound);
        }
        let mut name = self.transducer.get_name();
        if !name.is_empty() {
            name += " & ";
        }
        name += info_symbol;
        self.transducer.set_name(&name);
        self
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
        center_t.add_transition(
            source_state as HfstState,
            &HfstBasicTransition::new_symbols(
                target_state as HfstState,
                input.to_string(),
                output.to_string(),
                0.0,
            ),
            true,
        );
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
    pub fn get_inverse_of_upper_projection(&self) -> OtherSymbolTransducer {
        if self.is_broken {
            std::panic::panic_any(UndefinedSymbolPairsFound);
        }
        let fst = HfstBasicTransducer::from_transducer(&self.transducer);
        let mut new_fst = HfstBasicTransducer::new();

        let output_symbols = OST_CONFIG.with(|c| {
            c.borrow()
                .output_symbols
                .iter()
                .cloned()
                .collect::<Vec<_>>()
        });
        let symbol_pairs = OST_CONFIG.with(|c| c.borrow().symbol_pairs.clone());

        let num_states = fst.states_and_transitions().len();
        for state in 0..num_states {
            let st = state as HfstState;
            new_fst.add_state(st);
            if fst.is_final_state(st) {
                let w = fst.get_final_weight(st);
                new_fst.set_final_weight(st, &w);
            }
            for jt in fst.index(st).iter() {
                let input = jt.get_transition_data().get_input_symbol();
                let output = jt.get_transition_data().get_output_symbol();
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
        copy.transducer = HfstTransducer::new_from_basic(&new_fst, Self::config_transducer_type());
        copy.apply_zero(|t| {
            t.minimize();
        });
        copy
    }

    /// 'OtherSymbolTransducer &contained(void)' — '?* X ?*'.
    pub fn contained(&mut self) -> &mut Self {
        // [spec:hfst:def:other-symbol-transducer.universal-fn]
        // [spec:hfst:sem:other-symbol-transducer.universal-fn]
        let mut universal = OtherSymbolTransducer::new_symbol(TWOLC_UNKNOWN);
        universal.apply_zero(|t| {
            t.repeat_star();
        });
        let mut result = universal.clone();
        result.concatenate(self).concatenate(&universal);
        *self = result;
        self
    }

    /// 'OtherSymbolTransducer &contained_once(void)' —
    /// '?* X ?* - ?* X ?* X ?*'.
    pub fn contained_once(&mut self) -> &mut Self {
        let mut universal = OtherSymbolTransducer::new_symbol(TWOLC_UNKNOWN);
        universal.apply_zero(|t| {
            t.repeat_star();
        });
        let mut result1 = universal.clone();
        result1.concatenate(self).concatenate(&universal);
        let mut result2 = universal.clone();
        result2
            .concatenate(self)
            .concatenate(&universal)
            .concatenate(self)
            .concatenate(&universal);
        result1.subtract(&result2);
        *self = result1;
        self
    }

    /// 'OtherSymbolTransducer &negated(void)' — '?* - X'.
    pub fn negated(&mut self) -> &mut Self {
        let mut universal = OtherSymbolTransducer::new_symbol(TWOLC_UNKNOWN);
        universal.apply_zero(|t| {
            t.repeat_star();
        });
        universal.subtract(self);
        *self = universal;
        self
    }

    /// 'OtherSymbolTransducer &term_complemented(void)' — '? - X'.
    pub fn term_complemented(&mut self) -> &mut Self {
        let mut universal = OtherSymbolTransducer::new_symbol(TWOLC_UNKNOWN);
        universal.subtract(self);
        *self = universal;
        self
    }

    /// 'HfstTransducer get_transducer(void) const'.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.get-transducer-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.get-transducer-fn]
    pub fn get_transducer(&self) -> HfstTransducer {
        if self.is_broken {
            std::panic::panic_any(UndefinedSymbolPairsFound);
        }
        HfstTransducer::new_copy(&self.transducer)
    }

    /// 'void get_initial_transition_pairs(SymbolPairVector &pair_container)
    ///  const' — collect the symbol pairs on the transitions leaving the start
    /// state.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.get-initial-transition-pairs-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.get-initial-transition-pairs-fn]
    pub fn get_initial_transition_pairs(&self, pair_container: &mut SymbolPairVector) {
        if self.is_broken {
            std::panic::panic_any(UndefinedSymbolPairsFound);
        }
        let fst = HfstBasicTransducer::from_transducer(&self.transducer);
        for jt in fst.index(0).iter() {
            let input = jt.get_transition_data().get_input_symbol();
            let output = jt.get_transition_data().get_output_symbol();
            pair_container.push((input, output));
        }
    }

    /// 'bool is_empty_intersection(const OtherSymbolTransducer &another,
    ///  StringVector &v)' — true iff '*this' and 'another' share no string;
    /// when non-empty, the first common string is stored in 'v'.
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.is-empty-intersection-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.is-empty-intersection-fn]
    pub fn is_empty_intersection(
        &self,
        another: &OtherSymbolTransducer,
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
    pub fn is_subset(&self, another: &OtherSymbolTransducer) -> bool {
        // Do this properly later.. (preserved C++ comment.)
        let mut another_fst = another.clone();
        another_fst.subtract(self);
        let internal = HfstBasicTransducer::from_transducer(&another_fst.get_transducer());
        Self::empty(&internal)
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
impl Clone for OtherSymbolTransducer {
    fn clone(&self) -> Self {
        OtherSymbolTransducer {
            is_broken: self.is_broken,
            transducer: HfstTransducer::new_copy(&self.transducer),
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

    let fst1_transitions = fst1.index(state1);
    let fst2_transitions = fst2.index(state2);

    let mut fst1_transition_map: BTreeMap<SymbolPair, HfstState> = BTreeMap::new();
    for it in fst1_transitions.iter() {
        fst1_transition_map.insert(
            (it.get_input_symbol(), it.get_output_symbol()),
            it.get_target_state(),
        );
    }

    for it in fst2_transitions.iter() {
        let symbol_pair: SymbolPair = (it.get_input_symbol(), it.get_output_symbol());
        if let Some(&fst1_target) = fst1_transition_map.get(&symbol_pair) {
            let state_pair: (HfstState, HfstState) = (fst1_target, it.get_target_state());
            if !visited_pairs.contains(&state_pair) {
                v.push(format!("{}:{}", symbol_pair.0, symbol_pair.1));
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

impl Rule {
    /// 'Rule::Rule(name, center, contexts)' ('Rule.cc'). Disjuncts all
    /// 'contexts' into 'context', then harmonizes the center's diacritics
    /// against the disjuncted context.
    // [spec:hfst:def:rule.rule.rule-fn]
    // [spec:hfst:sem:rule.rule.rule-fn]
    pub fn new(
        name: &str,
        center: OtherSymbolTransducer,
        contexts: &OtherSymbolTransducerVector,
    ) -> Rule {
        let mut rule = Rule {
            is_empty: false,
            name: unescape_name(name),
            center,
            context: OtherSymbolTransducer::new(),
            rule_transducer: OtherSymbolTransducer::new(),
        };
        // OtherSymbolTransducerVector contexts_copy = contexts;
        // for (it : contexts_copy) context.apply(disjunct, *it);
        for ctx in contexts.iter() {
            rule.context.disjunct(ctx);
        }
        // this->center.harmonize_diacritics(context);
        let mut context = std::mem::replace(&mut rule.context, OtherSymbolTransducer::new());
        rule.center.harmonize_diacritics(&mut context);
        rule.context = context;
        rule
    }

    /// 'Rule::Rule(name, RuleVector)' ('Rule.cc') — the intersecting result
    /// constructor. Builds 'rule_transducer = ?*' then intersects the
    /// 'rule_transducer' of each non-empty subcase rule. Produces a
    /// ['ResultRule'] whose 'compile()' is the base no-op.
    // [spec:hfst:def:rule.rule.rule-fn]
    // [spec:hfst:sem:rule.rule.rule-fn]
    pub fn new_from_vector(name: &str, v: &[&dyn RuleT]) -> ResultRule {
        let mut rule_transducer = OtherSymbolTransducer::new_symbol(TWOLC_UNKNOWN);
        rule_transducer.repeat_star();
        let mut is_empty = true;
        for r in v.iter() {
            if !r.rule().empty() {
                rule_transducer.intersect(r.rule_transducer());
                is_empty = false;
            }
        }
        ResultRule {
            base: Rule {
                is_empty,
                name: unescape_name(name),
                center: OtherSymbolTransducer::new(),
                context: OtherSymbolTransducer::new(),
                rule_transducer,
            },
        }
    }

    /// 'Rule::empty()' ('Rule.cc'). True when conflict resolution merged this
    /// rule into another (or the intersecting ctor found no non-empty subcase).
    // [spec:hfst:def:rule.rule.empty-fn]
    // [spec:hfst:sem:rule.rule.empty-fn]
    pub fn empty(&self) -> bool {
        self.is_empty
    }

    /// 'Rule::store(out)' ('Rule.cc'). Binary 'HfstOutputStream' write path.
    // [spec:hfst:def:rule.rule.store-fn]
    // [spec:hfst:sem:rule.rule.store-fn]
    pub fn store(&mut self) {
        unimplemented!("deferred: HfstOutputStream store")
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
    pub fn add_name(&mut self) {
        let name = self.name.clone();
        self.rule_transducer.add_info_symbol(&name);
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

    /// 'Rule::get_universal_language_with_diamonds()' ('Rule.cc'). Returns
    /// '?* <D> ?* <D> ?*'.
    // [spec:hfst:def:rule.rule.get-universal-language-with-diamonds-fn]
    // [spec:hfst:sem:rule.rule.get-universal-language-with-diamonds-fn]
    pub fn get_universal_language_with_diamonds() -> OtherSymbolTransducer {
        let mut universal = OtherSymbolTransducer::new_symbol(TWOLC_UNKNOWN);
        universal.repeat_star();
        let diamond = OtherSymbolTransducer::new_symbol(TWOLC_DIAMOND);
        let mut universal_with_diamonds = universal.clone();
        universal_with_diamonds
            .concatenate(&diamond)
            .concatenate(&universal)
            .concatenate(&diamond)
            .concatenate(&universal);
        universal_with_diamonds
    }

    /// 'Rule::get_center(input, output)' ('Rule.cc'). Returns
    /// '?* <D> input:output <D> ?*'.
    // [spec:hfst:def:rule.rule.get-center-fn]
    // [spec:hfst:sem:rule.rule.get-center-fn]
    pub fn get_center_io(input: &str, output: &str) -> OtherSymbolTransducer {
        let mut unknown = OtherSymbolTransducer::new_symbol(TWOLC_UNKNOWN);
        unknown.repeat_star();
        let diamond = OtherSymbolTransducer::new_symbol(TWOLC_DIAMOND);
        let mut center = unknown.clone();
        let center_pair = OtherSymbolTransducer::new_pair(input, output);
        center
            .concatenate(&diamond)
            .concatenate(&center_pair)
            .concatenate(&diamond)
            .concatenate(&unknown);
        center
    }

    /// 'Rule::get_center(v: SymbolPairVector)' ('Rule.cc'). Returns
    /// '?* <D> (disjunction of pairs) <D> ?*'.
    // [spec:hfst:def:rule.rule.get-center-fn]
    // [spec:hfst:sem:rule.rule.get-center-fn]
    pub fn get_center_pairs(v: &SymbolPairVector) -> OtherSymbolTransducer {
        let mut unknown = OtherSymbolTransducer::new_symbol(TWOLC_UNKNOWN);
        unknown.repeat_star();
        let diamond = OtherSymbolTransducer::new_symbol(TWOLC_DIAMOND);
        let mut center_pair_transducer = OtherSymbolTransducer::new();
        for pair in v.iter() {
            let p = OtherSymbolTransducer::new_pair(&pair.0, &pair.1);
            center_pair_transducer.disjunct(&p);
        }
        let mut center = unknown.clone();
        center
            .concatenate(&diamond)
            .concatenate(&center_pair_transducer)
            .concatenate(&diamond)
            .concatenate(&unknown);
        center
    }

    /// 'Rule::get_center(restricted_center)' ('Rule.cc'). Returns
    /// '?* <D> restricted_center <D> ?*'.
    // [spec:hfst:def:rule.rule.get-center-fn]
    // [spec:hfst:sem:rule.rule.get-center-fn]
    pub fn get_center_restricted(
        restricted_center: &OtherSymbolTransducer,
    ) -> OtherSymbolTransducer {
        let mut unknown = OtherSymbolTransducer::new_symbol(TWOLC_UNKNOWN);
        unknown.repeat_star();
        let diamond = OtherSymbolTransducer::new_symbol(TWOLC_DIAMOND);
        let mut center = unknown.clone();
        center
            .concatenate(&diamond)
            .concatenate(restricted_center)
            .concatenate(&diamond)
            .concatenate(&unknown);
        center
    }

    /// 'Rule::add_missing_symbols_freely(diacritics)' ('Rule.cc'). For every
    /// diacritic that is not already in 'rule_transducer''s alphabet, add it to
    /// the alphabet and insert the diacritic-pair freely.
    // [spec:hfst:def:rule.rule.add-missing-symbols-freely-fn]
    // [spec:hfst:sem:rule.rule.add-missing-symbols-freely-fn]
    pub fn add_missing_symbols_freely(&mut self, diacritics: &SymbolRange) {
        let symbol_set: BTreeSet<String> = self.rule_transducer.get_transducer().get_alphabet();
        for d in diacritics.iter() {
            if !symbol_set.contains(d) {
                self.rule_transducer.add_symbol_to_alphabet(d);
                self.rule_transducer.apply_symbol_pair(
                    |t_, p_| {
                        t_.insert_freely_pair(p_, false);
                    },
                    &(d.clone(), d.clone()),
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ResultRule — produced by 'Rule::new_from_vector'; its compile() is a no-op
// (the C++ base 'Rule::compile()' returns an empty OtherSymbolTransducer).
// ---------------------------------------------------------------------------

impl RuleT for ResultRule {
    // [spec:hfst:def:rule.rule.compile-fn]
    // [spec:hfst:sem:rule.rule.compile-fn]
    fn compile(&mut self) -> OtherSymbolTransducer {
        OtherSymbolTransducer::new()
    }
    fn rule(&self) -> &Rule {
        &self.base
    }
    fn rule_mut(&mut self) -> &mut Rule {
        &mut self.base
    }
}

// ===========================================================================
// rule_src/RightArrowRule.{h,cc} — '=>' rule.
// ===========================================================================

impl RightArrowRule {
    /// 'RightArrowRule::RightArrowRule(name, center, contexts)'
    /// ('RightArrowRule.cc'). Delegates to the 'Rule' base constructor.
    // [spec:hfst:def:right-arrow-rule.right-arrow-rule.right-arrow-rule-fn]
    // [spec:hfst:sem:right-arrow-rule.right-arrow-rule.right-arrow-rule-fn]
    pub fn new(
        name: &str,
        center: OtherSymbolTransducer,
        contexts: &OtherSymbolTransducerVector,
    ) -> RightArrowRule {
        RightArrowRule {
            base: Rule::new(name, center, contexts),
        }
    }
}

impl RuleT for RightArrowRule {
    /// 'RightArrowRule::compile()' ('RightArrowRule.cc').
    ///
    /// '''text
    /// center.subtract(context).substitute(<D> -> 0);
    /// rule_transducer = ?* ;
    /// rule_transducer.subtract(center);
    /// '''
    ///
    /// MUTATES 'center' in place (subtract the context, then turn the diamonds
    /// into epsilon) before building 'rule_transducer = ?* - center'.
    // [spec:hfst:def:right-arrow-rule.right-arrow-rule.compile-fn]
    // [spec:hfst:sem:right-arrow-rule.right-arrow-rule.compile-fn]
    fn compile(&mut self) -> OtherSymbolTransducer {
        let context = std::mem::replace(&mut self.base.context, OtherSymbolTransducer::new());
        self.base
            .center
            .subtract(&context)
            .substitute_diamond_to_epsilon();
        self.base.context = context;

        let mut rule_transducer = OtherSymbolTransducer::new_symbol(TWOLC_UNKNOWN);
        let center = std::mem::replace(&mut self.base.center, OtherSymbolTransducer::new());
        rule_transducer.repeat_star().subtract(&center);
        self.base.center = center;

        self.base.rule_transducer = rule_transducer.clone();
        rule_transducer
    }
    fn rule(&self) -> &Rule {
        &self.base
    }
    fn rule_mut(&mut self) -> &mut Rule {
        &mut self.base
    }
}

// ===========================================================================
// rule_src/LeftArrowRule.{h,cc} — '<=' rule.
// ===========================================================================

impl LeftArrowRule {
    /// 'LeftArrowRule::LeftArrowRule(name, center, contexts)'
    /// ('LeftArrowRule.cc'). Delegates to the 'Rule' base constructor.
    // [spec:hfst:def:left-arrow-rule.left-arrow-rule.left-arrow-rule-fn]
    // [spec:hfst:sem:left-arrow-rule.left-arrow-rule.left-arrow-rule-fn]
    pub fn new(
        name: &str,
        center: OtherSymbolTransducer,
        contexts: &OtherSymbolTransducerVector,
    ) -> LeftArrowRule {
        LeftArrowRule {
            base: Rule::new(name, center, contexts),
        }
    }
}

impl RuleT for LeftArrowRule {
    /// 'LeftArrowRule::compile()' ('LeftArrowRule.cc').
    ///
    /// '''text
    /// abstract_center = center.get_inverse_of_upper_projection();
    /// context.intersect(abstract_center);
    /// context.subtract(center);
    /// context.substitute(<D> -> 0);
    /// rule_transducer = ?* ;
    /// return rule_transducer.subtract(context);
    /// '''
    // [spec:hfst:def:left-arrow-rule.left-arrow-rule.compile-fn]
    // [spec:hfst:sem:left-arrow-rule.left-arrow-rule.compile-fn]
    fn compile(&mut self) -> OtherSymbolTransducer {
        let abstract_center = self.base.center.get_inverse_of_upper_projection();
        // context.intersect(abstract_center).subtract(center).substitute(<D>->0)
        let center = std::mem::replace(&mut self.base.center, OtherSymbolTransducer::new());
        self.base
            .context
            .intersect(&abstract_center)
            .subtract(&center)
            .substitute_diamond_to_epsilon();
        self.base.center = center;

        let mut rule_transducer = OtherSymbolTransducer::new_symbol(TWOLC_UNKNOWN);
        let context = std::mem::replace(&mut self.base.context, OtherSymbolTransducer::new());
        rule_transducer.repeat_star().subtract(&context);
        self.base.context = context;

        self.base.rule_transducer = rule_transducer.clone();
        rule_transducer
    }
    fn rule(&self) -> &Rule {
        &self.base
    }
    fn rule_mut(&mut self) -> &mut Rule {
        &mut self.base
    }
}

// ===========================================================================
// rule_src/LeftRestrictionArrowRule.{h,cc} — '/<=' rule.
// ===========================================================================

impl LeftRestrictionArrowRule {
    /// 'LeftRestrictionArrowRule::LeftRestrictionArrowRule(name, center,
    /// contexts)' ('LeftRestrictionArrowRule.cc') — 'OtherSymbolTransducer'
    /// center form. Delegates to the 'Rule' base constructor.
    // [spec:hfst:def:left-restriction-arrow-rule.left-restriction-arrow-rule.left-restriction-arrow-rule-fn]
    // [spec:hfst:sem:left-restriction-arrow-rule.left-restriction-arrow-rule.left-restriction-arrow-rule-fn]
    pub fn new(
        name: &str,
        center: OtherSymbolTransducer,
        contexts: &OtherSymbolTransducerVector,
    ) -> LeftRestrictionArrowRule {
        LeftRestrictionArrowRule {
            base: Rule::new(name, center, contexts),
        }
    }

    /// 'LeftRestrictionArrowRule::LeftRestrictionArrowRule(name, SymbolPair
    /// center, contexts)' ('LeftRestrictionArrowRule.cc') — symbol-pair center
    /// form. Builds the center via 'Rule::get_center(first, second)'.
    // [spec:hfst:def:left-restriction-arrow-rule.left-restriction-arrow-rule.left-restriction-arrow-rule-fn]
    // [spec:hfst:sem:left-restriction-arrow-rule.left-restriction-arrow-rule.left-restriction-arrow-rule-fn]
    pub fn new_pair(
        name: &str,
        center: &SymbolPair,
        contexts: &OtherSymbolTransducerVector,
    ) -> LeftRestrictionArrowRule {
        LeftRestrictionArrowRule {
            base: Rule::new(name, Rule::get_center_io(&center.0, &center.1), contexts),
        }
    }
}

impl RuleT for LeftRestrictionArrowRule {
    /// 'LeftRestrictionArrowRule::compile()'
    /// ('LeftRestrictionArrowRule.cc').
    ///
    /// '''text
    /// center.intersect(context).substitute(<D> -> 0);
    /// rule_transducer = ?* ;
    /// rule_transducer.subtract(center);
    /// '''
    // [spec:hfst:def:left-restriction-arrow-rule.left-restriction-arrow-rule.compile-fn]
    // [spec:hfst:sem:left-restriction-arrow-rule.left-restriction-arrow-rule.compile-fn]
    fn compile(&mut self) -> OtherSymbolTransducer {
        let context = std::mem::replace(&mut self.base.context, OtherSymbolTransducer::new());
        self.base
            .center
            .intersect(&context)
            .substitute_diamond_to_epsilon();
        self.base.context = context;

        let mut rule_transducer = OtherSymbolTransducer::new_symbol(TWOLC_UNKNOWN);
        let center = std::mem::replace(&mut self.base.center, OtherSymbolTransducer::new());
        rule_transducer.repeat_star().subtract(&center);
        self.base.center = center;

        self.base.rule_transducer = rule_transducer.clone();
        rule_transducer
    }
    fn rule(&self) -> &Rule {
        &self.base
    }
    fn rule_mut(&mut self) -> &mut Rule {
        &mut self.base
    }
}

// ===========================================================================
// rule_src/ConflictResolvingRightArrowRule.{h,cc} — '=>' single-pair center.
// ===========================================================================

impl ConflictResolvingRightArrowRule {
    /// 'ConflictResolvingRightArrowRule::ConflictResolvingRightArrowRule(name,
    /// center, contexts)' ('ConflictResolvingRightArrowRule.cc'). Builds the
    /// 'RightArrowRule' base from 'get_center(first, second)' and records the
    /// 'center_pair'.
    // [spec:hfst:def:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule-fn]
    // [spec:hfst:sem:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule-fn]
    pub fn new(
        name: &str,
        center: &SymbolPair,
        contexts: &OtherSymbolTransducerVector,
    ) -> ConflictResolvingRightArrowRule {
        ConflictResolvingRightArrowRule {
            base: RightArrowRule::new(name, Rule::get_center_io(&center.0, &center.1), contexts),
            center_pair: center.clone(),
        }
    }

    /// 'ConflictResolvingRightArrowRule::conflicts_this(another)'
    /// ('ConflictResolvingRightArrowRule.cc'). Two '=>'-rules conflict when
    /// they share the same center symbol-pair.
    // [spec:hfst:def:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule.conflicts-this-fn]
    // [spec:hfst:sem:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule.conflicts-this-fn]
    pub fn conflicts_this(&self, another: &ConflictResolvingRightArrowRule) -> bool {
        self.center_pair.0 == another.center_pair.0 && self.center_pair.1 == another.center_pair.1
    }

    /// 'ConflictResolvingRightArrowRule::resolve_conflict(another)'
    /// ('ConflictResolvingRightArrowRule.cc'). Merges 'another''s context into
    /// 'this' (disjunct + minimize) and appends its name.
    // [spec:hfst:def:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule.resolve-conflict-fn]
    // [spec:hfst:sem:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule.resolve-conflict-fn]
    pub fn resolve_conflict(&mut self, another: &ConflictResolvingRightArrowRule) {
        let another_context = another.base.base.context.clone();
        self.base.base.context.disjunct(&another_context).minimize();
        let another_name = another.base.base.name.clone();
        self.base.base.name += " and ";
        self.base.base.name += &another_name;
    }
}

impl RuleT for ConflictResolvingRightArrowRule {
    fn compile(&mut self) -> OtherSymbolTransducer {
        self.base.compile()
    }
    fn rule(&self) -> &Rule {
        &self.base.base
    }
    fn rule_mut(&mut self) -> &mut Rule {
        &mut self.base.base
    }
}

// ===========================================================================
// rule_src/ConflictResolvingLeftArrowRule.{h,cc} — '<=' single-pair center.
// ===========================================================================

/// 'get_wb_fst()' ('ConflictResolvingLeftArrowRule.cc'). Builds the
/// word-boundary framing transducer '.#. ((? - .#.) | <D>)* .#.'.
// [spec:hfst:def:conflict-resolving-left-arrow-rule.get-wb-fst-fn]
// [spec:hfst:sem:conflict-resolving-left-arrow-rule.get-wb-fst-fn]
pub fn get_wb_fst() -> OtherSymbolTransducer {
    let wb = OtherSymbolTransducer::new_pair("__HFST_TWOLC_.#.", "__HFST_TWOLC_.#.");
    let mut no_wb = OtherSymbolTransducer::new_pair(TWOLC_UNKNOWN, TWOLC_UNKNOWN);
    let diamond = OtherSymbolTransducer::new_pair(TWOLC_DIAMOND, TWOLC_DIAMOND);

    no_wb.subtract(&wb);
    no_wb.disjunct(&diamond);
    no_wb.repeat_star();

    let mut result = wb.clone();
    result.concatenate(&no_wb);
    result.concatenate(&wb);

    result
}

/// 'wbize(t)' ('ConflictResolvingLeftArrowRule.cc'). Intersects 't' with the
/// word-boundary framing transducer.
// [spec:hfst:def:conflict-resolving-left-arrow-rule.wbize-fn]
// [spec:hfst:sem:conflict-resolving-left-arrow-rule.wbize-fn]
pub fn wbize(t: &OtherSymbolTransducer) -> OtherSymbolTransducer {
    let mut t_copy = t.clone();
    let wb_fst = get_wb_fst();
    t_copy.intersect(&wb_fst);
    t_copy
}

impl ConflictResolvingLeftArrowRule {
    /// 'ConflictResolvingLeftArrowRule::ConflictResolvingLeftArrowRule(name,
    /// center, contexts)' ('ConflictResolvingLeftArrowRule.cc'). Builds the
    /// 'LeftArrowRule' base from 'get_center(first, second)' and records the
    /// center's input symbol.
    // [spec:hfst:def:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule-fn]
    // [spec:hfst:sem:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule-fn]
    pub fn new(
        name: &str,
        center: &SymbolPair,
        contexts: &OtherSymbolTransducerVector,
    ) -> ConflictResolvingLeftArrowRule {
        ConflictResolvingLeftArrowRule {
            base: LeftArrowRule::new(name, Rule::get_center_io(&center.0, &center.1), contexts),
            input_symbol: center.0.clone(),
        }
    }

    /// 'ConflictResolvingLeftArrowRule::conflicts_this(another, v)'
    /// ('ConflictResolvingLeftArrowRule.cc'). True when 'this''s context has a
    /// non-empty intersection with the word-boundary-framed context of
    /// 'another' (the conflicting string is stored in 'v').
    // [spec:hfst:def:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.conflicts-this-fn]
    // [spec:hfst:sem:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.conflicts-this-fn]
    pub fn conflicts_this(
        &self,
        another: &ConflictResolvingLeftArrowRule,
        v: &mut StringVector,
    ) -> bool {
        !self
            .base
            .base
            .context
            .is_empty_intersection(&wbize(&another.base.base.context), v)
    }

    /// 'ConflictResolvingLeftArrowRule::resolvable_conflict(another)'
    /// ('ConflictResolvingLeftArrowRule.cc'). True when 'this''s context is a
    /// sub-language of the word-boundary-framed context of 'another'.
    // [spec:hfst:def:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.resolvable-conflict-fn]
    // [spec:hfst:sem:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.resolvable-conflict-fn]
    pub fn resolvable_conflict(&self, another: &ConflictResolvingLeftArrowRule) -> bool {
        self.base
            .base
            .context
            .is_subset(&wbize(&another.base.base.context))
    }

    /// 'ConflictResolvingLeftArrowRule::resolve_conflict(another)'
    /// ('ConflictResolvingLeftArrowRule.cc'). Resolves by subtracting
    /// 'another''s context from 'this''s context.
    // [spec:hfst:def:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.resolve-conflict-fn]
    // [spec:hfst:sem:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.resolve-conflict-fn]
    pub fn resolve_conflict(&mut self, another: &ConflictResolvingLeftArrowRule) {
        let another_context = another.base.base.context.clone();
        self.base.base.context.subtract(&another_context);
    }
}

impl RuleT for ConflictResolvingLeftArrowRule {
    fn compile(&mut self) -> OtherSymbolTransducer {
        self.base.compile()
    }
    fn rule(&self) -> &Rule {
        &self.base.base
    }
    fn rule_mut(&mut self) -> &mut Rule {
        &mut self.base.base
    }
}

// ===== body 2 (flattened, module scope) =====
// ───────────────────────────────────────────────────────────────────────────
// Rule containers (rule_src/RuleContainer.cc, RightArrowRuleContainer.cc,
// LeftArrowRuleContainer.cc).
//
// The C++ containers held 'std::vector<Rule*>' and 'delete'd the pointers in
// the destructor; here every rule is OWNED as a 'Box<dyn RuleT>' in
// 'rule_vector', so 'Box' drop replaces the deleting destructor. The C++ maps
// stored 'Rule*' keyed by center-pair / input-symbol; here they store INDICES
// into the owning 'rule_vector' (a 'Rule*' replacement that survives the
// borrow checker because the rules are owned in one place).
//
// Conflict resolution touches only the 'Rule' base data ('context'/'name'),
// reachable through the ['RuleT'] 'rule()'/'rule_mut()' accessors, plus the
// free 'wbize' helper; so the container code never needs to downcast the
// 'Box<dyn RuleT>' back to its concrete conflict-resolving type.
// ───────────────────────────────────────────────────────────────────────────

// [spec:hfst:def:rule-container.rule-container]
impl RuleContainer {
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
    pub fn add_rule(&mut self, rule: Box<dyn RuleT>) -> usize {
        self.rule_vector.push(rule);
        self.rule_vector.len() - 1
    }

    // [spec:hfst:def:rule-container.rule-container.compile-fn]
    // [spec:hfst:sem:rule-container.rule-container.compile-fn]
    //
    // C++ iterates 'rule_vector', optionally prints the print-name and calls
    // '(*it)->compile()'. The verbose message is sent to stderr (the C++
    // 'msg_out' is always 'std::cerr' at the call sites).
    pub fn compile(&mut self, be_verbose: bool) {
        for rule in self.rule_vector.iter_mut() {
            if be_verbose {
                eprintln!("Compiling {}", Rule::get_print_name(&rule.rule().name));
            }
            rule.compile();
        }
    }

    // [spec:hfst:def:rule-container.rule-container.store-fn]
    // [spec:hfst:sem:rule-container.rule-container.store-fn]
    //
    // DEFERRED: the HfstOutputStream binary store path is not yet ported.
    pub fn store(&mut self, _be_verbose: bool) {
        unimplemented!("deferred: HfstOutputStream RuleContainer::store");
    }

    // [spec:hfst:def:rule-container.rule-container.add-missing-symbols-freely-fn]
    // [spec:hfst:sem:rule-container.rule-container.add-missing-symbols-freely-fn]
    pub fn add_missing_symbols_freely(&mut self, diacritics: &SymbolRange) {
        for rule in self.rule_vector.iter_mut() {
            rule.rule_mut().add_missing_symbols_freely(diacritics);
        }
    }

    /// Borrow the rule at 'index' as a '&dyn RuleT' (the 'Rule*' deref the
    /// grammar performs when intersecting subcases by handle).
    pub(crate) fn rule_ref(&self, index: usize) -> &dyn RuleT {
        self.rule_vector[index].as_ref()
    }
}

impl Default for RuleContainer {
    fn default() -> Self {
        RuleContainer::new()
    }
}

// [spec:hfst:def:right-arrow-rule-container.right-arrow-rule-container]
impl RightArrowRuleContainer {
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
    // applied here through the base 'Rule' data so the owned 'Box<dyn RuleT>'
    // need not be downcast.
    pub fn add_rule_and_display_and_resolve_conflicts(
        &mut self,
        mut rule: ConflictResolvingRightArrowRule,
    ) -> usize {
        let center_pair = rule.center_pair.clone();
        if let Some(&existing_index) = self.center_to_rule_map.get(&center_pair) {
            if self.report_right_arrow_conflicts {
                let existing_name = self.base.rule_vector[existing_index].rule().name.clone();
                let incoming_name = rule.base.base.name.clone();
                eprintln!(
                    "There is a =>-rule conflict between {} and {}.",
                    Rule::get_print_name(&existing_name),
                    Rule::get_print_name(&incoming_name)
                );
                eprintln!("Resolving the conflict by joining contexts.");
                eprintln!();
            }

            if self.resolve_right_arrow_conflicts {
                // ConflictResolvingRightArrowRule::resolve_conflict:
                //   existing.context.disjunct(incoming.context).minimize();
                //   existing.name += " and " + incoming.name;
                let incoming_context = clone_ost(&rule.base.base.context);
                let incoming_name = rule.base.base.name.clone();
                {
                    let existing = self.base.rule_vector[existing_index].rule_mut();
                    existing.context.disjunct(&incoming_context);
                    existing.context.minimize();
                    existing.name = format!("{} and {}", existing.name, incoming_name);
                }
                rule.base.base.is_empty = true;
                self.base.add_rule(Box::new(rule))
            } else {
                self.base.add_rule(Box::new(rule))
            }
        } else {
            let index = self.base.add_rule(Box::new(rule));
            self.center_to_rule_map.insert(center_pair, index);
            index
        }
    }

    /// C++ 'RuleContainer::compile' forwarded through the base member.
    pub fn compile(&mut self, be_verbose: bool) {
        self.base.compile(be_verbose);
    }

    pub(crate) fn rule_ref(&self, index: usize) -> &dyn RuleT {
        self.base.rule_ref(index)
    }
}

impl Default for RightArrowRuleContainer {
    fn default() -> Self {
        RightArrowRuleContainer::new()
    }
}

// [spec:hfst:def:left-arrow-rule-container.left-arrow-rule-container]
impl LeftArrowRuleContainer {
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
        mut rule: ConflictResolvingLeftArrowRule,
    ) -> usize {
        let input = rule.input_symbol.clone();
        if let Some(indices) = self.input_to_rule_map.get(&input) {
            let existing_indices: Vec<usize> = indices.clone();
            for existing_index in existing_indices {
                // (*it)->conflicts_this(*rule, conflicting_context):
                //   ! existing.context.is_empty_intersection(wbize(rule.context))
                let wbized_incoming = wbize(&rule.base.base.context);
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
                        eprint!(
                            "There is a <=-rule conflict between {} and {}.\nE.g. in context ",
                            Rule::get_print_name(&existing_name),
                            Rule::get_print_name(&rule.base.base.name)
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
                            eprint!("{} ", symbol_pair);
                        }
                        eprintln!();
                    }
                    if self.resolve_left_arrow_conflicts {
                        // existing.resolvable_conflict(rule):
                        //   existing.context.is_subset(wbize(rule.context))
                        let existing_resolvable = {
                            let existing = self.base.rule_vector[existing_index].rule();
                            existing.context.is_subset(&wbized_incoming)
                        };
                        if existing_resolvable {
                            if self.report_left_arrow_conflicts {
                                let existing_name =
                                    self.base.rule_vector[existing_index].rule().name.clone();
                                eprintln!(
                                    "Resolving the conflict by restricting the context of {}.",
                                    Rule::get_print_name(&existing_name)
                                );
                            }
                            // existing.resolve_conflict(rule):
                            //   existing.context.subtract(rule.context);
                            let incoming_context = clone_ost(&rule.base.base.context);
                            let existing = self.base.rule_vector[existing_index].rule_mut();
                            existing.context.subtract(&incoming_context);
                        } else {
                            // rule.resolvable_conflict(*it):
                            //   rule.context.is_subset(wbize(existing.context))
                            let wbized_existing = {
                                let existing = self.base.rule_vector[existing_index].rule();
                                wbize(&existing.context)
                            };
                            let incoming_resolvable =
                                rule.base.base.context.is_subset(&wbized_existing);
                            if incoming_resolvable {
                                if self.report_left_arrow_conflicts {
                                    eprintln!(
                                        "Resolving the conflict by restricting the context of {}.",
                                        rule.base.base.name
                                    );
                                }
                                // rule.resolve_conflict(*it):
                                //   rule.context.subtract(existing.context);
                                let existing_context = {
                                    let existing = self.base.rule_vector[existing_index].rule();
                                    clone_ost(&existing.context)
                                };
                                rule.base.base.context.subtract(&existing_context);
                            } else if self.report_left_arrow_conflicts {
                                eprintln!("WARNING! The conflict is unresolvable.");
                            }
                        }
                    }
                    if self.report_left_arrow_conflicts {
                        eprintln!();
                    }
                }
            }
        }
        let index = self.base.add_rule(Box::new(rule));
        self.input_to_rule_map.entry(input).or_default().push(index);
        index
    }

    pub fn compile(&mut self, be_verbose: bool) {
        self.base.compile(be_verbose);
    }

    pub(crate) fn rule_ref(&self, index: usize) -> &dyn RuleT {
        self.base.rule_ref(index)
    }
}

impl Default for LeftArrowRuleContainer {
    fn default() -> Self {
        LeftArrowRuleContainer::new()
    }
}

/// Copy an ['OtherSymbolTransducer'] (the C++ copy constructor /
/// 'operator=' that copies 'is_broken' + the wrapped transducer). Used by the
/// container conflict-resolution code, which must read one rule's context while
/// mutating another's.
fn clone_ost(t: &OtherSymbolTransducer) -> OtherSymbolTransducer {
    OtherSymbolTransducer {
        is_broken: t.is_broken,
        transducer: t.transducer.clone(),
    }
}

// ───────────────────────────────────────────────────────────────────────────
// TwolCGrammar (rule_src/TwolCGrammar.cc).
// ───────────────────────────────────────────────────────────────────────────

// [spec:hfst:def:twol-c-grammar.twol-c-grammar]
impl TwolCGrammar {
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
    pub fn define_diacritics(&mut self, diacritics: &SymbolRange) {
        self.diacritics = diacritics.to_vec();
        OtherSymbolTransducer::define_diacritics(diacritics);
    }

    /// Record a subcase handle for 'name''s original (pre-'SUBCASE:') name.
    fn insert_subcase(&mut self, name: &str, handle: RuleHandle) {
        self.name_to_rule_subcases
            .entry(TwolCGrammar::get_original_name(name))
            .or_default()
            .insert(handle);
    }

    /// 'TwolCGrammar::add_rule(name, const SymbolPair &center, oper, contexts)'
    /// — the single-pair center overload ('RIGHT'/'LEFT'/'LEFT_RIGHT'/
    /// 'NOT_LEFT').
    pub fn add_rule_pair(
        &mut self,
        name: &str,
        center: &SymbolPair,
        oper: Operator,
        contexts: &OtherSymbolTransducerVector,
    ) {
        match oper {
            Operator::RIGHT => {
                let rule = ConflictResolvingRightArrowRule::new(name, center, contexts);
                let index = self
                    .right_arrow_rule_container
                    .add_rule_and_display_and_resolve_conflicts(rule);
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Right,
                        index,
                    },
                );
            }
            Operator::LEFT => {
                let rule = ConflictResolvingLeftArrowRule::new(name, center, contexts);
                let index = self
                    .left_arrow_rule_container
                    .add_rule_and_display_and_resolve_conflicts(rule);
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Left,
                        index,
                    },
                );
            }
            Operator::LEFT_RIGHT => {
                let right_rule = ConflictResolvingRightArrowRule::new(name, center, contexts);
                let right_index = self
                    .right_arrow_rule_container
                    .add_rule_and_display_and_resolve_conflicts(right_rule);
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Right,
                        index: right_index,
                    },
                );
                let left_rule = ConflictResolvingLeftArrowRule::new(name, center, contexts);
                let left_index = self
                    .left_arrow_rule_container
                    .add_rule_and_display_and_resolve_conflicts(left_rule);
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Left,
                        index: left_index,
                    },
                );
            }
            Operator::NOT_LEFT => {
                let rule = LeftRestrictionArrowRule::new_pair(name, center, contexts);
                let index = self.other_rule_container.add_rule(Box::new(rule));
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Other,
                        index,
                    },
                );
            }
            _ => panic!("TwolCGrammar::add_rule_pair: unexpected operator {oper:?}"),
        }
    }

    /// 'TwolCGrammar::add_rule(name, const OtherSymbolTransducer &center, oper,
    /// contexts)' — the regex-center overload ('RE_*'). The center is wrapped
    /// by 'Rule::get_center(restricted_center)' ('?* D center D ?*').
    pub fn add_rule_regex(
        &mut self,
        name: &str,
        center: &OtherSymbolTransducer,
        oper: Operator,
        contexts: &OtherSymbolTransducerVector,
    ) {
        let center_fst = Rule::get_center_restricted(center);
        match oper {
            Operator::RE_RIGHT => {
                let rule = RightArrowRule::new(name, clone_ost(&center_fst), contexts);
                let index = self.other_rule_container.add_rule(Box::new(rule));
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Other,
                        index,
                    },
                );
            }
            Operator::RE_LEFT => {
                let rule = LeftArrowRule::new(name, clone_ost(&center_fst), contexts);
                let index = self.other_rule_container.add_rule(Box::new(rule));
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Other,
                        index,
                    },
                );
            }
            Operator::RE_LEFT_RIGHT => {
                let right_rule = RightArrowRule::new(name, clone_ost(&center_fst), contexts);
                let right_index = self.other_rule_container.add_rule(Box::new(right_rule));
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Other,
                        index: right_index,
                    },
                );
                let left_rule = LeftArrowRule::new(name, clone_ost(&center_fst), contexts);
                let left_index = self.other_rule_container.add_rule(Box::new(left_rule));
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Other,
                        index: left_index,
                    },
                );
            }
            Operator::RE_NOT_LEFT => {
                let rule = LeftRestrictionArrowRule::new(name, clone_ost(&center_fst), contexts);
                let index = self.other_rule_container.add_rule(Box::new(rule));
                self.insert_subcase(
                    name,
                    RuleHandle {
                        container: RuleContainerKind::Other,
                        index,
                    },
                );
            }
            _ => panic!("TwolCGrammar::add_rule_regex: unexpected operator {oper:?}"),
        }
    }

    // [spec:hfst:def:twol-c-grammar.twol-c-grammar.add-rule-fn]
    // [spec:hfst:sem:twol-c-grammar.twol-c-grammar.add-rule-fn]
    //
    // 'TwolCGrammar::add_rule(name, const SymbolPairVector &center, oper,
    // contexts)' — the multi-pair center overload, building one rule per center
    // pair named 'name CENTER=<in>:<out>'.
    pub fn add_rule_pairs(
        &mut self,
        name: &str,
        center: &SymbolPairVector,
        oper: Operator,
        contexts: &OtherSymbolTransducerVector,
    ) {
        for pair in center.iter() {
            let center_name = format!("{} CENTER={}:{}", name, pair.0, pair.1);
            match oper {
                Operator::RIGHT => {
                    let rule = ConflictResolvingRightArrowRule::new(&center_name, pair, contexts);
                    let index = self
                        .right_arrow_rule_container
                        .add_rule_and_display_and_resolve_conflicts(rule);
                    self.insert_subcase(
                        &center_name,
                        RuleHandle {
                            container: RuleContainerKind::Right,
                            index,
                        },
                    );
                }
                Operator::LEFT => {
                    let rule = ConflictResolvingLeftArrowRule::new(&center_name, pair, contexts);
                    let index = self
                        .left_arrow_rule_container
                        .add_rule_and_display_and_resolve_conflicts(rule);
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
                        ConflictResolvingRightArrowRule::new(&center_name, pair, contexts);
                    let right_index = self
                        .right_arrow_rule_container
                        .add_rule_and_display_and_resolve_conflicts(right_rule);
                    self.insert_subcase(
                        &center_name,
                        RuleHandle {
                            container: RuleContainerKind::Right,
                            index: right_index,
                        },
                    );
                    let left_rule =
                        ConflictResolvingLeftArrowRule::new(&center_name, pair, contexts);
                    let left_index = self
                        .left_arrow_rule_container
                        .add_rule_and_display_and_resolve_conflicts(left_rule);
                    self.insert_subcase(
                        &center_name,
                        RuleHandle {
                            container: RuleContainerKind::Left,
                            index: left_index,
                        },
                    );
                }
                Operator::NOT_LEFT => {
                    let rule = LeftRestrictionArrowRule::new_pair(&center_name, pair, contexts);
                    let index = self.other_rule_container.add_rule(Box::new(rule));
                    self.insert_subcase(
                        &center_name,
                        RuleHandle {
                            container: RuleContainerKind::Other,
                            index,
                        },
                    );
                }
                _ => panic!("TwolCGrammar::add_rule_pairs: unexpected operator {oper:?}"),
            }
        }
    }

    /// Borrow the rule a ['RuleHandle'] points at as a '&dyn RuleT'.
    fn rule_at(&self, handle: RuleHandle) -> &dyn RuleT {
        match handle.container {
            RuleContainerKind::Left => self.left_arrow_rule_container.rule_ref(handle.index),
            RuleContainerKind::Right => self.right_arrow_rule_container.rule_ref(handle.index),
            RuleContainerKind::Other => self.other_rule_container.rule_ref(handle.index),
        }
    }

    // [spec:hfst:def:twol-c-grammar.twol-c-grammar.compile-and-store-fn]
    // [spec:hfst:sem:twol-c-grammar.twol-c-grammar.compile-and-store-fn]
    //
    // C++ compiles each container, then for every original rule name builds a
    // 'Rule(name, RuleVector)' intersecting its subcases, adds the missing
    // diacritics freely, and stores the result. The binary store path is
    // DEFERRED; this port instead RETURNS the assembled result transducer (the
    // intersection of every compiled rule), so a smoke can drive the compiler.
    pub fn compile_and_store(&mut self) -> HfstTransducer {
        if !self.be_quiet {
            eprintln!("Compiling rules.");
        }

        let verbose = (!self.be_quiet) && self.be_verbose;
        self.left_arrow_rule_container.compile(verbose);
        self.right_arrow_rule_container.compile(verbose);
        self.other_rule_container.compile(verbose);

        // Build one intersecting 'ResultRule' per original rule name. The
        // 'name_to_rule_subcases' map is iterated in its (ordered) key order,
        // mirroring the C++ 'StringRuleSetMap' traversal.
        let names: Vec<String> = self.name_to_rule_subcases.keys().cloned().collect();
        for name in names {
            let handles: Vec<RuleHandle> =
                self.name_to_rule_subcases[&name].iter().copied().collect();
            let subcases: Vec<&dyn RuleT> = handles.iter().map(|&h| self.rule_at(h)).collect();
            let result_rule = Rule::new_from_vector(&name, &subcases);
            self.compiled_rule_container.add_rule(Box::new(result_rule));
        }
        let diacritics = self.diacritics.clone();
        self.compiled_rule_container
            .add_missing_symbols_freely(&diacritics);

        if !self.be_quiet {
            eprintln!("Storing rules.");
        }

        // DEFERRED: 'compiled_rule_container.store(out, ...)'. Instead intersect
        // the compiled result rules into one transducer and return it as the
        // grammar's result (the union of all rule constraints over the shared
        // '?*' universe — intersection of the per-rule 'rule_transducer's).
        assemble_result_transducer(&self.compiled_rule_container)
    }
}

/// Intersect every non-empty compiled 'ResultRule''s rule-transducer into one
/// 'HfstTransducer' — the value the deferred 'compile_and_store' store path
/// would otherwise serialise. Starts from '?*' (the universal language over the
/// twolc unknown symbol) so an empty grammar yields the universal automaton,
/// matching 'Rule(name, RuleVector)''s own '?*'-seeded intersection.
fn assemble_result_transducer(container: &RuleContainer) -> HfstTransducer {
    let mut result = OtherSymbolTransducer::new_symbol(TWOLC_UNKNOWN);
    result.repeat_star();
    for rule in container.rule_vector.iter() {
        if !rule.rule().is_empty {
            let rt = clone_ost(rule.rule_transducer());
            result.intersect(&rt);
        }
    }
    result.transducer
}

// ───────────────────────────────────────────────────────────────────────────
// TwolcCompiler — the AST-walk driver (replaces TwolcCompiler.cc + the three
// Flex/Bison preprocessor passes).
// ───────────────────────────────────────────────────────────────────────────

/// The two shapes a rule center can evaluate to: a (possibly multi-)pair list
/// ('a:b | c:d'), or a regex transducer (':[ E ]:'). Drives the
/// ['TwolCGrammar::add_rule_pairs'] vs ['TwolCGrammar::add_rule_regex'] choice.
pub enum CenterEval {
    Pairs(SymbolPairVector),
    Regex(OtherSymbolTransducer),
}

/// One concrete rule produced by expanding a ['TwolcRule']'s 'where'-variables:
/// a fully-substituted name, the evaluated center, the operator and the
/// evaluated (positive + negated-negative) contexts.
pub struct ConcreteRule {
    pub name: String,
    pub center: CenterEval,
    pub oper: Operator,
    pub contexts: OtherSymbolTransducerVector,
}

// [spec:hfst:def:twolc-compiler.hfst.twolc.twolc-compiler]
impl TwolcCompiler {
    /// Construct with the C++ default flags: 'silent = false',
    /// 'verbose = false', 'resolve_left_conflicts = false',
    /// 'resolve_right_conflicts = true' (the upstream 'htwolc' defaults).
    pub fn new(format: ImplementationType) -> Self {
        TwolcCompiler::new_with_options(format, false, false, false, true)
    }

    /// Construct with explicit flags (the C++ 'TwolcCompiler::compile'
    /// parameters 'silent', 'verbose', 'resolve_left_conflicts',
    /// 'resolve_right_conflicts').
    pub fn new_with_options(
        format: ImplementationType,
        silent: bool,
        verbose: bool,
        resolve_left: bool,
        resolve_right: bool,
    ) -> Self {
        TwolcCompiler {
            format,
            silent,
            verbose,
            resolve_left_conflicts: resolve_left,
            resolve_right_conflicts: resolve_right,
            sets: BTreeMap::new(),
            definitions: BTreeMap::new(),
        }
    }

    // [spec:hfst:def:twolc-compiler.hfst.twolc.twolc-compiler.compile-fn]
    // [spec:hfst:sem:twolc-compiler.hfst.twolc.twolc-compiler.compile-fn]
    //
    // Replaces the three Flex/Bison passes with 'nfst_twolc::parse' + an
    // AST-walk: set the transducer type, register the alphabet / diacritics /
    // sets / definitions, build the grammar, drive every rule (expanding
    // 'where'-variables and evaluating centers + contexts), and return the
    // intersected result transducer as an owned raw pointer (the same
    // 'Box::into_raw' convention as 'XreCompiler::compile'). A parse failure
    // yields a null pointer.
    pub fn compile(&mut self, input: &str) -> *mut HfstTransducer {
        let file = match nfst_twolc::parse(input) {
            Ok(f) => f,
            Err(_) => return std::ptr::null_mut(),
        };
        let twolc_file = &file.value;

        OtherSymbolTransducer::set_transducer_type(self.format);

        let mut grammar = TwolCGrammar::new(
            self.silent,
            self.verbose,
            self.resolve_left_conflicts,
            self.resolve_right_conflicts,
        );

        self.register_alphabet(&twolc_file.alphabet);
        self.register_diacritics(&twolc_file.diacritics, &mut grammar);
        self.register_sets(&twolc_file.sets);
        self.register_definitions(&twolc_file.definitions);

        for rule in twolc_file.rules.iter() {
            self.drive_rule(&rule.value, &mut grammar);
        }

        let result = grammar.compile_and_store();
        Box::into_raw(Box::new(result))
    }

    /// Register the 'Alphabet' section: collect the declared symbol pairs and
    /// publish them to ['OtherSymbolTransducer::set_symbol_pairs'] (which also
    /// inserts the diamond:diamond pair).
    pub fn register_alphabet(&mut self, pairs: &[Spanned<AlphabetPair>]) {
        let mut symbol_pairs: BTreeSet<SymbolPair> = BTreeSet::new();
        for p in pairs {
            symbol_pairs.insert((p.value.upper.clone(), p.value.lower.clone()));
        }
        OtherSymbolTransducer::set_symbol_pairs(&symbol_pairs);
    }

    /// Register the 'Diacritics' section: publish the diacritic list to both the
    /// 'OtherSymbolTransducer' config and the grammar.
    pub fn register_diacritics(
        &mut self,
        diacritics: &[Spanned<String>],
        grammar: &mut TwolCGrammar,
    ) {
        let list: SymbolRange = diacritics.iter().map(|d| d.value.clone()).collect();
        grammar.define_diacritics(&list);
    }

    /// Register the 'Sets' section: record each set's ordered member list, so a
    /// 'Symbol' naming a set expands to the disjunction of its members.
    pub fn register_sets(&mut self, sets: &[Spanned<SetDefinition>]) {
        for s in sets {
            self.sets
                .insert(s.value.name.clone(), s.value.members.clone());
        }
    }

    /// Register the 'Definitions' section: evaluate each named regex body to an
    /// ['OtherSymbolTransducer'] so a 'Symbol' naming a definition expands to
    /// it (mirrors the C++ 'NameToRegexMap').
    pub fn register_definitions(&mut self, defs: &[Spanned<TwolcDefinition>]) {
        for d in defs {
            let t = self.eval_regex(&d.value.body);
            self.definitions.insert(d.value.name.clone(), t);
        }
    }

    /// Drive one ['TwolcRule']: expand its 'where'-variables into concrete
    /// rules and feed each into the grammar via the matching 'add_rule'
    /// overload.
    pub fn drive_rule(&mut self, rule: &TwolcRule, grammar: &mut TwolCGrammar) {
        for concrete in self.expand_rule_variables(rule) {
            match concrete.center {
                CenterEval::Pairs(pairs) => {
                    grammar.add_rule_pairs(
                        &concrete.name,
                        &pairs,
                        concrete.oper,
                        &concrete.contexts,
                    );
                }
                CenterEval::Regex(center) => {
                    grammar.add_rule_regex(
                        &concrete.name,
                        &center,
                        concrete.oper,
                        &concrete.contexts,
                    );
                }
            }
        }
    }

    /// Expand a ['TwolcRule']'s 'where'-blocks into concrete rules using the
    /// ['RuleVariables'] odometer. With no 'where'-clause the rule yields one
    /// concrete rule with an empty variable assignment.
    pub fn expand_rule_variables(&mut self, rule: &TwolcRule) -> Vec<ConcreteRule> {
        let regex_center = matches!(rule.center, RuleCenter::Regex(_));
        let oper = TwolcCompiler::operator_of(rule.operator, regex_center);

        // No variables: emit the rule once with no substitutions. The variable
        // markers in names only matter when a 'where'-clause is present, so the
        // rule name is used as-is.
        let blocks_opt = rule.variables.as_ref();
        let rule_variables = match blocks_opt {
            Some(blocks) if !blocks.is_empty() => TwolcCompiler::build_rule_variables(blocks),
            _ => RuleVariables::new(),
        };

        let mut result: Vec<ConcreteRule> = Vec::new();

        if rule_variables.empty() {
            let empty_vvm = VariableValueMap::new();
            if let Some(cr) = self.build_concrete_rule(rule, oper, &empty_vvm) {
                result.push(cr);
            }
            return result;
        }

        // Odometer over the cross-product of the where-blocks.
        let mut it = rule_variables.begin();
        let end = rule_variables.end();
        while it.ne(&end) {
            let mut vvm = VariableValueMap::new();
            it.set_values(&mut vvm);
            if let Some(cr) = self.build_concrete_rule(rule, oper, &vvm) {
                result.push(cr);
            }
            it.increment();
        }
        result
    }

    /// Build a single ['ConcreteRule'] from a rule template and a variable
    /// assignment: substitute the assignment into the name/center symbols, then
    /// evaluate the center and contexts.
    fn build_concrete_rule(
        &mut self,
        rule: &TwolcRule,
        oper: Operator,
        vvm: &VariableValueMap,
    ) -> Option<ConcreteRule> {
        // Compose the subcase-qualified name. The C++ rule template carried a
        // '__HFST_TWOLC_RULE_NAME' marker that 'RuleSymbolVector' rewrote with
        // the 'SUBCASE:'/'var=value' markers; here the rule's own name plays
        // that role and the marker rewrite is applied directly.
        let name = build_rule_name(&rule.name, vvm);

        let center = self.eval_center(&rule.center, vvm);
        let contexts = self.eval_contexts(&rule.positive_contexts, &rule.negative_contexts, vvm);
        Some(ConcreteRule {
            name,
            center,
            oper,
            contexts,
        })
    }

    /// AST 'where'-blocks -> ['RuleVariables'] (the C++
    /// 'set_variable'/'add_values'/'set_matcher' sequence per block).
    pub fn build_rule_variables(blocks: &[VariableBlock]) -> RuleVariables {
        let mut rv = RuleVariables::new();
        for block in blocks {
            for assignment in block.assignments.iter() {
                rv.set_variable(&assignment.name);
                rv.add_values(&assignment.values);
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
    pub fn eval_center(&mut self, center: &RuleCenter, vvm: &VariableValueMap) -> CenterEval {
        match center {
            RuleCenter::Pair(pairs) => {
                let mut spv: SymbolPairVector = Vec::new();
                for p in pairs {
                    let upper = substitute_symbol(&p.upper, vvm);
                    let lower = substitute_symbol(&p.lower, vvm);
                    spv.push((upper, lower));
                }
                CenterEval::Pairs(spv)
            }
            RuleCenter::Regex(e) => CenterEval::Regex(self.eval_regex_with_vars(e, vvm)),
        }
    }

    /// Evaluate the positive and negative contexts of a rule into one
    /// ['OtherSymbolTransducerVector']. Each positive context is 'X D ?* D Y';
    /// each negative context is the same, negated ('?* - context'). The C++
    /// negative contexts ('except' clauses) are negated before being added.
    pub fn eval_contexts(
        &mut self,
        pos: &[RuleContext],
        neg: &[RuleContext],
        vvm: &VariableValueMap,
    ) -> OtherSymbolTransducerVector {
        let mut result: OtherSymbolTransducerVector = Vec::new();
        for ctx in pos {
            result.push(self.eval_context(ctx, vvm));
        }
        for ctx in neg {
            let mut c = self.eval_context(ctx, vvm);
            c.negated();
            result.push(c);
        }
        result
    }

    /// Evaluate one ['RuleContext'] into 'left D ?* D right' via
    /// ['OtherSymbolTransducer::get_context'].
    pub fn eval_context(
        &mut self,
        ctx: &RuleContext,
        vvm: &VariableValueMap,
    ) -> OtherSymbolTransducer {
        let mut left = self.eval_regex_with_vars(&ctx.left, vvm);
        let mut right = self.eval_regex_with_vars(&ctx.right, vvm);
        OtherSymbolTransducer::get_context(&mut left, &mut right)
    }

    /// Evaluate a ['TwolcRegex'] with no variable substitution (used by the
    /// 'Definitions' section, which is evaluated before any rule expansion).
    pub fn eval_regex(&mut self, e: &Spanned<TwolcRegex>) -> OtherSymbolTransducer {
        let vvm = VariableValueMap::new();
        self.eval_regex_with_vars(e, &vvm)
    }

    /// Evaluate a ['TwolcRegex'], substituting any variable symbol with its
    /// assigned value. Mirrors the 'xre.rs' 'eval'/'eval_unary'/'eval_binary'
    /// recursion shape, but over the smaller twolc regex sublanguage and
    /// building ['OtherSymbolTransducer']s.
    fn eval_regex_with_vars(
        &mut self,
        e: &Spanned<TwolcRegex>,
        vvm: &VariableValueMap,
    ) -> OtherSymbolTransducer {
        match &e.value {
            TwolcRegex::Symbol(s) => {
                let sym = substitute_symbol(s, vvm);
                // A symbol naming a definition expands to its transducer; a
                // symbol naming a set expands to the disjunction of its members;
                // otherwise it is a literal 'sym:sym' pair.
                if let Some(def) = self.definitions.get(&sym) {
                    clone_ost(def)
                } else if let Some(members) = self.sets.get(&sym).cloned() {
                    let mut t = OtherSymbolTransducer::new();
                    for m in &members {
                        let member_sym = substitute_symbol(m, vvm);
                        let pair = OtherSymbolTransducer::new_symbol(&member_sym);
                        t.disjunct(&pair);
                    }
                    t
                } else {
                    OtherSymbolTransducer::new_symbol(&sym)
                }
            }
            TwolcRegex::Pair { upper, lower } => {
                let up = symbol_of(upper, vvm);
                let lo = symbol_of(lower, vvm);
                OtherSymbolTransducer::new_pair(&up, &lo)
            }
            TwolcRegex::Epsilon => OtherSymbolTransducer::new_symbol(TWOLC_EPSILON),
            TwolcRegex::Any => OtherSymbolTransducer::new_symbol(TWOLC_UNKNOWN),
            TwolcRegex::Group(inner) => self.eval_regex_with_vars(inner, vvm),
            TwolcRegex::Optional(inner) => {
                let mut t = self.eval_regex_with_vars(inner, vvm);
                t.apply_zero(|t_| {
                    t_.optionalize();
                });
                t
            }
            TwolcRegex::Binary(op, l, r) => self.eval_binary(*op, l, r, vvm),
            TwolcRegex::Unary(op, inner) => self.eval_unary(*op, inner, vvm),
            TwolcRegex::RepeatN(inner, n) => {
                let mut t = self.eval_regex_with_vars(inner, vvm);
                t.apply_num(
                    |t_, n_| {
                        t_.repeat_n(n_);
                    },
                    *n,
                );
                t
            }
            TwolcRegex::RepeatNToK(inner, n, k) => {
                let mut t = self.eval_regex_with_vars(inner, vvm);
                t.apply_two_num(
                    |t_, a_, b_| {
                        t_.repeat_n_to_k(a_, b_);
                    },
                    *n,
                    *k,
                );
                t
            }
        }
    }

    /// Evaluate a ['TwolcRegex::Unary'] node.
    fn eval_unary(
        &mut self,
        op: UnaryOp,
        inner: &Spanned<TwolcRegex>,
        vvm: &VariableValueMap,
    ) -> OtherSymbolTransducer {
        let mut t = self.eval_regex_with_vars(inner, vvm);
        match op {
            UnaryOp::Star => {
                t.apply_zero(|t_| {
                    t_.repeat_star();
                });
            }
            UnaryOp::Plus => {
                t.apply_zero(|t_| {
                    t_.repeat_plus();
                });
            }
            UnaryOp::Reverse => {
                t.apply_zero(|t_| {
                    t_.reverse();
                });
            }
            UnaryOp::Invert => {
                t.apply_zero(|t_| {
                    t_.invert();
                });
            }
            UnaryOp::UpperProject => {
                t.apply_zero(|t_| {
                    t_.input_project();
                });
            }
            UnaryOp::LowerProject => {
                t.apply_zero(|t_| {
                    t_.output_project();
                });
            }
            UnaryOp::Complement => {
                t.negated();
            }
            UnaryOp::TermComplement => {
                t.term_complemented();
            }
            UnaryOp::Containment => {
                t.contained();
            }
            UnaryOp::ContainmentOnce => {
                t.contained_once();
            }
            UnaryOp::ContainmentOpt => {
                t.contained();
            }
        }
        t
    }

    /// Evaluate a ['TwolcRegex::Binary'] node.
    fn eval_binary(
        &mut self,
        op: BinaryOp,
        l: &Spanned<TwolcRegex>,
        r: &Spanned<TwolcRegex>,
        vvm: &VariableValueMap,
    ) -> OtherSymbolTransducer {
        let mut left = self.eval_regex_with_vars(l, vvm);
        let right = self.eval_regex_with_vars(r, vvm);
        match op {
            BinaryOp::Concatenate => {
                left.concatenate(&right);
            }
            BinaryOp::Union => {
                left.disjunct(&right);
            }
            BinaryOp::Intersect => {
                left.intersect(&right);
            }
            BinaryOp::Subtract => {
                left.subtract(&right);
            }
            BinaryOp::Compose => {
                left.apply_one_bool(
                    |t_, o_, h_| {
                        t_.compose(o_, h_);
                    },
                    &right,
                );
            }
            other => {
                std::panic::panic_any(format!(
                    "twolc regex: unsupported binary operator {other:?}"
                ));
            }
        }
        left
    }
}

/// Substitute a single symbol via a variable assignment: if 'sym' is a variable
/// in 'vvm', return its value, else 'sym' unchanged (the 'RuleSymbolVector'
/// per-symbol 'vvm' lookup).
fn substitute_symbol(sym: &str, vvm: &VariableValueMap) -> String {
    match vvm.get(sym) {
        Some(val) => val.clone(),
        None => sym.to_string(),
    }
}

/// Resolve a 'TwolcRegex' operand expected to be a single symbol (the upper or
/// lower side of a 'Pair'), applying variable substitution.
fn symbol_of(e: &Spanned<TwolcRegex>, vvm: &VariableValueMap) -> String {
    match &e.value {
        TwolcRegex::Symbol(s) => substitute_symbol(s, vvm),
        TwolcRegex::Epsilon => TWOLC_EPSILON.to_string(),
        TwolcRegex::Any => TWOLC_UNKNOWN.to_string(),
        TwolcRegex::Group(inner) => symbol_of(inner, vvm),
        _ => std::panic::panic_any("twolc pair side must be a single symbol".to_string()),
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

// [spec:hfst:def:rule.unescape-name-fn]
// [spec:hfst:sem:rule.unescape-name-fn]
pub fn unescape_name(name: &str) -> String {
    replace_substr(
        &replace_substr(name, "__HFST_TWOLC_RULE_NAME=", ""),
        "__HFST_TWOLC_SPACE",
        " ",
    )
}

impl RuleVariables {
    // [spec:hfst:def:rule-variables.rule-variables.rule-variables-fn]
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
// lockstep diagonal over a block's variables (equal-size sets). 'mixed' is deferred.
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
    pub fn begin(&self) -> RuleVariablesConstIterator {
        RuleVariablesConstIterator::new(self, false)
    }
    // [spec:hfst:def:rule-variables.rule-variables.end-fn]
    pub fn end(&self) -> RuleVariablesConstIterator {
        RuleVariablesConstIterator::new(self, true)
    }
}

// One odometer dimension: a list of (variable, its values) that advance together
// (length 1 => an independent 'freely' variable; length >1 => a 'matched' block,
// lockstep over equal-size value sets). 'size' is the number of positions.
#[derive(Clone)]
pub(crate) struct VarDim {
    pub(crate) vars: Vec<VariableValues>,
    pub(crate) size: usize,
}

// [spec:hfst:def:rule-variables-const-iterator.rule-variables-const-iterator]
pub struct RuleVariablesConstIterator {
    dims: Vec<VarDim>,
    indices: Vec<usize>,
    at_end: bool,
}

impl RuleVariablesConstIterator {
    fn new(rv: &RuleVariables, end: bool) -> Self {
        if !rv.mixed_blocks.is_empty() {
            unimplemented!("deferred: `mixed` where-clause variable expansion");
        }
        let mut dims: Vec<VarDim> = Vec::new();
        // freely: every variable is its own independent dimension.
        for block in &rv.freely_blocks {
            for vv in block {
                dims.push(VarDim {
                    size: vv.values.len(),
                    vars: vec![vv.clone()],
                });
            }
        }
        // matched: each block is one lockstep dimension (equal set sizes required).
        for block in &rv.matched_blocks {
            let size = block.first().map(|v| v.values.len()).unwrap_or(0);
            for vv in block {
                if vv.values.len() != size {
                    std::panic::panic_any(UnequalSetSize);
                }
            }
            dims.push(VarDim {
                size,
                vars: block.clone(),
            });
        }
        let any_empty = dims.iter().any(|d| d.size == 0);
        let indices = vec![0usize; dims.len()];
        RuleVariablesConstIterator {
            dims,
            indices,
            at_end: end || any_empty,
        }
    }

    // [spec:hfst:def:rule-variables-const-iterator.rule-variables-const-iterator.set-values-fn]
    pub fn set_values(&self, vvm: &mut VariableValueMap) {
        for (dim, &idx) in self.dims.iter().zip(self.indices.iter()) {
            for vv in &dim.vars {
                vvm.insert(vv.variable.clone(), vv.values[idx].clone());
            }
        }
    }

    // [spec:hfst:def:rule-variables-const-iterator.rule-variables-const-iterator.operator-increment-fn]
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
