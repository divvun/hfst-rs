//! ABSOLUTE-faithful C++->Rust port of HFST's XRE (Xerox regex) compiler,
//! RESTRUCTURED to walk the `nfst-xre` typed AST instead of the original
//! Flex/Bison grammar. The AST-walk restructuring is the ONE sanctioned
//! structural deviation in this port: the transducer-building BEHAVIOUR must
//! still match the C++ semantic actions in `xre_parse.yy` / `xre_utils.cc`
//! exactly.
//!
//! Ported from `libhfst/src/parsers/XreCompiler.{h,cc}` and
//! `libhfst/src/parsers/xre_utils.{h,cc}`.
//!
//! # C++ globals folded into [`XreCompiler`]
//!
//! The C++ implementation kept compilation state in `xre_utils.cc` file-scope
//! globals (`definitions`, `function_definitions`, `function_arguments`,
//! `symbol_lists`, `format`, `expand_definitions`, `harmonize_`,
//! `harmonize_flags_`, `verbose_`). Because this port walks the AST directly
//! and is re-entrant, those globals become instance fields on [`XreCompiler`]
//! and the per-compile evaluation state, instead of process-wide mutable
//! statics.
//!
//! # Deferred (record as `unimplemented!`)
//!
//! - [`XreExpr::ReadFile`] — `@bin`/`@txt`/`@stxt`/`@pl`/`@re` file I/O loads.
//! - The prolog/regex `@`-loads reached through the same path.
//! - `contains_twolc` (the two-level twolc-contains helper; "doesn't work at
//!   the moment" in the C++ source) — kept as a documented helper that panics
//!   with `unimplemented!`.

#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)] // some 1:1-ported xre_utils helpers are not yet reached by every eval path
#![allow(clippy::too_many_arguments)]

use std::collections::{BTreeMap, BTreeSet};

use nfst_xre::{
    BinaryOp, ContextMark, MappingKind, MappingPair, MappingSide, ReadKind, ReplaceArrow,
    ReplaceContext, ReplaceRule, RestrContext, SpannedXre, SubstituteWhat, UnaryOp, XreExpr, parse,
    parse_all,
};

use crate::hfst_data_types::ImplementationType;
use crate::hfst_transducer::HfstTransducer;

/// Arguments bundle mirroring `hfst::xre::XreConstructorArguments`
/// (`XreCompiler.h`). Carries the four definition maps plus the target
/// implementation format used to seed a fresh [`XreCompiler`]. Owned
/// `HfstTransducer` values replace the C++ `HfstTransducer*` map values
/// (the C++ destructor `delete`d them; ownership lives in the map here).
///
/// `std::map` -> `BTreeMap`, `std::set` -> `BTreeSet` per port conventions.
// [spec:hfst:def:xre-compiler.hfst.xre.xre-constructor-arguments]
#[derive(Clone)]
pub struct XreConstructorArguments {
    /// `std::map<std::string, hfst::HfstTransducer*> definitions`.
    pub definitions: BTreeMap<String, HfstTransducer>,
    /// `std::map<std::string, std::string> function_definitions`.
    pub function_definitions: BTreeMap<String, String>,
    /// `std::map<std::string, unsigned int> function_arguments`.
    pub function_arguments: BTreeMap<String, u32>,
    /// `std::map<std::string, std::set<std::string>> list_definitions`.
    pub list_definitions: BTreeMap<String, BTreeSet<String>>,
    /// `hfst::ImplementationType format`.
    pub format: ImplementationType,
}

impl XreConstructorArguments {
    /// Port of the `XreConstructorArguments(...)` field-copy constructor.
    // [spec:hfst:def:xre-compiler.hfst.xre.xre-constructor-arguments.xre-constructor-arguments-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-constructor-arguments.xre-constructor-arguments-fn]
    pub fn new(
        definitions: BTreeMap<String, HfstTransducer>,
        function_definitions: BTreeMap<String, String>,
        function_arguments: BTreeMap<String, u32>,
        list_definitions: BTreeMap<String, BTreeSet<String>>,
        format: ImplementationType,
    ) -> Self {
        XreConstructorArguments {
            definitions,
            function_definitions,
            function_arguments,
            list_definitions,
            format,
        }
    }
}

/// Lets `XreCompiler::new(..)` accept either an [`ImplementationType`] or a
/// `&XreConstructorArguments`, reproducing the two C++ constructor overloads
/// (`XreCompiler(ImplementationType)` and
/// `XreCompiler(const XreConstructorArguments&)`) behind a single entry point,
/// matching the existing facade call sites in `hfst_transducer.rs`.
pub trait XreCompilerNew {
    /// Build the compiler from `self` (a format, or an args bundle).
    fn into_xre_compiler(self) -> XreCompiler;
}

/// A compiler holding the information needed to compile XREs.
///
/// Port of `hfst::xre::XreCompiler` plus the `xre_utils.cc` file-scope globals
/// it relied on. Field names match the C++ members 1:1
/// (`definitions_`, `function_definitions_`, `function_arguments_`,
/// `list_definitions_`, `format_`, `verbose_`), with the former globals
/// `expand_definitions`, `harmonize_`, `harmonize_flags_` added as instance
/// state so compilation is re-entrant.
// [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler]
pub struct XreCompiler {
    /// `std::map<std::string, hfst::HfstTransducer*> definitions_`.
    /// Owned transducers (C++ stored raw pointers freed by `~XreCompiler`).
    pub(crate) definitions_: BTreeMap<String, HfstTransducer>,
    /// `std::map<std::string, std::string> function_definitions_`.
    pub(crate) function_definitions_: BTreeMap<String, String>,
    /// `std::map<std::string, unsigned int> function_arguments_`.
    pub(crate) function_arguments_: BTreeMap<String, u32>,
    /// `std::map<std::string, std::set<std::string>> list_definitions_`.
    pub(crate) list_definitions_: BTreeMap<String, BTreeSet<String>>,
    /// `hfst::ImplementationType format_` — target type for built transducers.
    pub(crate) format_: ImplementationType,
    /// `bool verbose_` — verbose warnings toggle.
    pub(crate) verbose_: bool,
    /// `xre_utils.cc` global `bool expand_definitions` (default `false`):
    /// whether a defined name expands to its stored transducer.
    pub(crate) expand_definitions_: bool,
    /// `xre_utils.cc` global `bool harmonize_` (default `true`): whether binary
    /// operators harmonize their argument transducers.
    pub(crate) harmonize_: bool,
    /// `xre_utils.cc` global `bool harmonize_flags_` (default `false`): whether
    /// composition harmonizes flag diacritics of its arguments.
    pub(crate) harmonize_flags_: bool,
}

// ──────────────────────────────────────────────────────────────────────────
// Method / helper roster filled by the body agents (declarations only here).
//
// Public API (XreCompiler.h surface; keep these signatures so the facade calls
// `XreCompiler::new(type)`, `XreCompiler::new(&args)`, `compile(&str)`,
// `set_verbosity(bool)` keep type-checking):
//
//   fn new<A: XreCompilerNew>(arg: A) -> XreCompiler          // both overloads
//   fn default_compiler() -> XreCompiler                       // XreCompiler()
//   [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.define-fn]
//   fn define(&mut self, name: &str, xre: &str) -> bool
//   [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.define-list-fn]
//   fn define_list(&mut self, name: &str, symbol_list: &BTreeSet<String>)
//   fn define_transducer(&mut self, name: &str, transducer: &HfstTransducer)   // define(name, HfstTransducer&)
//   [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.define-function-fn]
//   fn define_function(&mut self, name: &str, arguments: u32, xre: &str) -> bool
//   [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.is-definition-fn]
//   fn is_definition(&self, name: &str) -> bool
//   [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.is-function-definition-fn]
//   fn is_function_definition(&self, name: &str) -> bool
//   [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.undefine-fn]
//   fn undefine(&mut self, name: &str)
//   [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.add-defined-multichar-symbol-fn]
//   fn add_defined_multichar_symbol(&mut self, symbol: &str)
//   [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.remove-defined-multichar-symbols-fn]
//   fn remove_defined_multichar_symbols(&mut self)
//   [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.compile-fn]
//   fn compile(&mut self, xre: &str) -> *mut HfstTransducer    // facade expects raw ptr
//   [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.compile-first-fn]
//   fn compile_first(&mut self, xre: &str, chars_read: &mut u32) -> *mut HfstTransducer
//   [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.contained-only-comments-fn]
//   fn contained_only_comments(&self) -> bool
//   [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.get-positions-of-symbol-in-xre-fn]
//   fn get_positions_of_symbol_in_xre(&mut self, symbol: &str, xre: &str, positions: &mut BTreeSet<u32>) -> bool
//   [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.set-expand-definitions-fn]
//   fn set_expand_definitions(&mut self, expand: bool)
//   [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.set-harmonization-fn]
//   fn set_harmonization(&mut self, harmonize: bool)
//   [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.set-flag-harmonization-fn]
//   fn set_flag_harmonization(&mut self, harmonize_flags: bool)
//   [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.set-verbosity-fn]
//   fn set_verbosity(&mut self, verbose: bool)
//   [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.get-verbosity-fn]
//   fn get_verbosity(&self) -> bool
//   (set_error_stream / get_error_stream / setOutputToConsole / getOutputToConsole / get_stream / flush
//    are WINDOWS/stream plumbing — omit or stub as no-ops; record if needed.)
//
// Top-level compile pipeline (port of xre_utils.cc compile/compile_first):
//   fn compile_impl(&mut self, xre: &str) -> Option<HfstTransducer>             // parse() + eval root
//   fn compile_first_impl(&mut self, xre: &str, chars_read: &mut u32) -> Option<HfstTransducer> // parse_all()/first
//
// AST evaluator (the sanctioned restructuring — one eval arm per XreExpr):
//   fn eval(&mut self, node: &SpannedXre) -> HfstTransducer                     // dispatch on node.value
//   fn eval_symbol(&mut self, s: &str) -> HfstTransducer                        // Symbol -> xfst_label_to_transducer(s,s) / definition expand
//   fn eval_curly(&mut self, s: &str) -> HfstTransducer                         // Curly -> xfst_curly_label_to_transducer
//   fn eval_epsilon(&self) -> HfstTransducer                                    // internal_epsilon arc
//   fn eval_any(&self) -> HfstTransducer                                        // internal_identity ?:? arc
//   fn eval_boundary_marker(&self) -> HfstTransducer                            // ".#." symbol
//   fn eval_pair(&mut self, upper: &SpannedXre, lower: &SpannedXre) -> HfstTransducer
//   fn eval_weighted(&mut self, expr: &SpannedXre, weight: f64) -> HfstTransducer // set_final_weights
//   fn eval_read_file(&mut self, kind: ReadKind, path: &str) -> HfstTransducer    // DEFERRED: unimplemented!
//   fn eval_function_call(&mut self, name: &str, args: &[SpannedXre]) -> HfstTransducer
//   fn eval_group(&mut self, inner: &SpannedXre) -> HfstTransducer
//   fn eval_optional(&mut self, inner: &SpannedXre) -> HfstTransducer           // optionalize()
//   fn eval_bracketed_dotted(&mut self, inner: Option<&SpannedXre>) -> HfstTransducer
//   fn eval_unary(&mut self, op: UnaryOp, inner: &SpannedXre) -> HfstTransducer
//   fn eval_binary(&mut self, op: BinaryOp, lhs: &SpannedXre, rhs: &SpannedXre) -> HfstTransducer
//   fn eval_repeat_n(&mut self, inner: &SpannedXre, n: u32) -> HfstTransducer
//   fn eval_repeat_n_plus(&mut self, inner: &SpannedXre, n: u32) -> HfstTransducer
//   fn eval_repeat_n_minus(&mut self, inner: &SpannedXre, n: u32) -> HfstTransducer
//   fn eval_repeat_n_to_k(&mut self, inner: &SpannedXre, n: u32, k: u32) -> HfstTransducer
//   fn eval_containment_with_weight(&mut self, expr: &SpannedXre, weight: f64) -> HfstTransducer
//   fn eval_replace(&mut self, arrow: ReplaceArrow, rules: &[ReplaceRule]) -> HfstTransducer
//   fn eval_restriction(&mut self, body: &SpannedXre, contexts: &[RestrContext]) -> HfstTransducer
//   fn eval_substitute(&mut self, haystack: &SpannedXre, what: &SubstituteWhat) -> HfstTransducer
//
// Replace/restriction lowering helpers (build hfst_xerox_rules / hfst_rules input):
//   fn build_rules(&mut self, rules: &[ReplaceRule]) -> Vec<crate::hfst_xerox_rules::Rule>
//   fn build_mapping_pair(&mut self, m: &MappingPair) -> HfstTransducerPair-ish    // upper/lower or markup
//   fn build_mapping_side(&mut self, side: &MappingSide) -> HfstTransducer
//   fn build_replace_contexts(&mut self, ctx: &ReplaceContexts) -> (ContextMark, context vector)
//   fn build_replace_context(&mut self, ctx: &ReplaceContext) -> HfstTransducerPair-ish
//   fn build_restr_contexts(&mut self, contexts: &[RestrContext]) -> HfstTransducerPairVector
//   fn apply_replace_arrow(&self, arrow: ReplaceArrow, rules: &[Rule]) -> HfstTransducer  // pick replace fn
//
// Ported xre_utils.cc free helpers (become &self/&mut self methods so they see
// definitions_/format_/expand_definitions_/verbose_):
//   [spec:hfst:def:xre-utils.hfst.xre.xfst-label-to-transducer-fn]
//   fn xfst_label_to_transducer(&mut self, input: &str, output: &str) -> HfstTransducer
//   [spec:hfst:def:xre-utils.hfst.xre.xfst-curly-label-to-transducer-fn]
//   fn xfst_curly_label_to_transducer(&self, input: &str, output: &str) -> HfstTransducer
//   [spec:hfst:def:xre-utils.hfst.xre.is-definition-fn]
//   fn is_definition_sym(&self, symbol: &str) -> bool
//   [spec:hfst:def:xre-utils.hfst.xre.expand-definition-fn]
//   fn expand_definition_sym(&self, symbol: &str) -> HfstTransducer                 // expand_definition(symbol)
//   fn expand_definition_tr(&self, tr: HfstTransducer, symbol: &str) -> HfstTransducer // expand_definition(tr, symbol)
//   [spec:hfst:def:xre-utils.hfst.xre.contains-fn]
//   fn contains(&self, t: &HfstTransducer) -> HfstTransducer                        // [?* t ?*]
//   [spec:hfst:def:xre-utils.hfst.xre.contains-with-weight-fn]
//   fn contains_with_weight(&self, t: &HfstTransducer, weight: f32) -> HfstTransducer
//   [spec:hfst:def:xre-utils.hfst.xre.contains-twolc-fn]
//   fn contains_twolc(&self, t: &HfstTransducer) -> HfstTransducer                  // DEFERRED: unimplemented!
//   [spec:hfst:def:xre-utils.hfst.xre.contains-once-fn]
//   fn contains_once(&self, c: &HfstTransducer) -> HfstTransducer
//   [spec:hfst:def:xre-utils.hfst.xre.contains-once-optional-fn]
//   fn contains_once_optional(&self, t: &HfstTransducer) -> HfstTransducer
//   [spec:hfst:def:xre-utils.hfst.xre.merge-first-to-second-fn]
//   fn merge_first_to_second(&self, tr1: &mut HfstTransducer, tr2: &mut HfstTransducer) // tr2.merge(tr1, args)
//   [spec:hfst:def:xre-utils.hfst.xre.is-valid-function-call-fn]
//   fn is_valid_function_call(&self, name: &str, args: &[HfstTransducer]) -> bool
//   [spec:hfst:def:xre-utils.hfst.xre.get-function-xre-fn]
//   fn get_function_xre(&self, name: &str) -> Option<&str>
//   [spec:hfst:def:xre-utils.hfst.xre.define-function-args-fn]
//   fn define_function_args(&mut self, name: &str, args: &[HfstTransducer]) -> bool
//   [spec:hfst:def:xre-utils.hfst.xre.undefine-function-args-fn]
//   fn undefine_function_args(&mut self, name: &str)
//   [spec:hfst:def:xre-utils.hfst.xre.has-non-identity-pairs-fn]
//   fn has_non_identity_pairs(&self, t: &HfstTransducer) -> bool
//   [spec:hfst:def:xre-utils.hfst.xre.warn-fn]
//   fn warn(&self, msg: &str)
//   [spec:hfst:def:xre-utils.hfst.xre.warn-about-special-symbols-in-replace-fn]
//   fn warn_about_special_symbols_in_replace(&self, t: &HfstTransducer)
//   [spec:hfst:def:xre-utils.hfst.xre.warn-about-hfst-special-symbol-fn]
//   fn warn_about_hfst_special_symbol(&self, symbol: &str)
//   fn warn_about_xfst_special_symbol(&self, symbol: &str)
//   [spec:hfst:def:xre-utils.hfst.xre.check-multichar-symbol-fn]
//   fn check_multichar_symbol(&self, symbol: &str)
//   (escape_enclosing_angle_brackets / unescape_enclosing_angle_brackets /
//    set_substitution_function_symbol / substitution_function — string/symbol
//    plumbing; port as private helpers, record if any are deferred.)
// ──────────────────────────────────────────────────────────────────────────

// ===== body 0 (flattened, module scope) =====
// ===========================================================================
// XRE compiler — core evaluator + public API (ported from XreCompiler.cc, the
// xre_parse.yy semantic actions, and the xre_utils.cc compile() driver,
// RESTRUCTURED to walk the nfst-xre AST). This body owns: constructors
// (XreCompilerNew impls), the full public API, compile/compile_first, and the
// recursive `eval`. It delegates the Replace/Restriction/Substitute/ReadFile
// arms and the transducer-building helpers (xfst_label_to_transducer, contains*,
// expand_definition_sym, merge_first_to_second, ...) to the sibling body — see
// the dependency list in NOTES.
// ===========================================================================

use crate::hfst_exception_defs::HfstException;
use crate::hfst_symbol_defs::{internal_epsilon, internal_identity, internal_unknown};
use crate::hfst_xerox_rules::{after, before};

// Former xre_utils.cc file-scope global `bool contains_only_comments`. The
// skeleton did not fold this onto the instance, so it stays a (thread-local)
// global exactly as in C++; set by compile()/compile_first(), read by
// contained_only_comments().
thread_local! {
    static CONTAINS_ONLY_COMMENTS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

// Classification of a `:` pair side. The nfst-xre parser only ever produces a
// halfarc atom (Symbol/Epsilon/Any/BoundaryMarker), a Curly, or a `Group([E])`
// on each side of a Pair; `None` from the classifier means "bracketed
// expression — evaluate it".
enum XrePairSide {
    Half(String),
    Curly(String),
}

fn xre_pair_side_kind(e: &SpannedXre) -> Option<XrePairSide> {
    match &e.value {
        XreExpr::Symbol(s) => Some(XrePairSide::Half(s.clone())),
        XreExpr::Epsilon => Some(XrePairSide::Half(internal_epsilon.to_string())),
        XreExpr::Any => Some(XrePairSide::Half(internal_unknown.to_string())),
        // nfst-xre actually emits `.#.` as Symbol(".#."); this arm is here for
        // completeness. Per the porting spec the boundary symbol is ".#.".
        XreExpr::BoundaryMarker => Some(XrePairSide::Half(".#.".to_string())),
        XreExpr::Curly(s) => Some(XrePairSide::Curly(s.clone())),
        _ => None,
    }
}

// ============================ constructors =================================
// [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.xre-compiler-fn]
// [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.xre-compiler-fn]
//
// Reproduces the two C++ ctor overloads behind the skeleton's polymorphic
// `XreCompiler::new`. ASSUMPTION: the skeleton trait is
// `pub trait XreCompilerNew { fn into_xre_compiler(self) -> XreCompiler; }`. If the skeleton
// named the trait method differently, rename `build` below to match.
impl XreCompilerNew for ImplementationType {
    fn into_xre_compiler(self) -> XreCompiler {
        XreCompiler {
            definitions_: BTreeMap::new(),
            function_definitions_: BTreeMap::new(),
            function_arguments_: BTreeMap::new(),
            list_definitions_: BTreeMap::new(),
            format_: self,
            verbose_: false,
            expand_definitions_: false,
            harmonize_: true,
            harmonize_flags_: false,
        }
    }
}

impl XreCompilerNew for &XreConstructorArguments {
    fn into_xre_compiler(self) -> XreCompiler {
        XreCompiler {
            definitions_: self.definitions.clone(),
            function_definitions_: self.function_definitions.clone(),
            function_arguments_: self.function_arguments.clone(),
            list_definitions_: self.list_definitions.clone(),
            format_: self.format,
            verbose_: false,
            expand_definitions_: false,
            harmonize_: true,
            harmonize_flags_: false,
        }
    }
}

// ============================ public API ===================================
impl XreCompiler {
    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.set-verbosity-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.set-verbosity-fn]
    pub fn set_verbosity(&mut self, verbose: bool) {
        self.verbose_ = verbose;
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.get-verbosity-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.get-verbosity-fn]
    pub fn get_verbosity(&self) -> bool {
        self.verbose_
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.set-error-stream-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.set-error-stream-fn]
    // Stream plumbing is deferred (the C++ error_ global is not ported); no-op.
    pub fn set_error_stream<T>(&mut self, _os: T) {}

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.set-expand-definitions-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.set-expand-definitions-fn]
    pub fn set_expand_definitions(&mut self, expand: bool) {
        self.expand_definitions_ = expand;
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.set-harmonization-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.set-harmonization-fn]
    pub fn set_harmonization(&mut self, harmonize: bool) {
        self.harmonize_ = harmonize;
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.set-flag-harmonization-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.set-flag-harmonization-fn]
    pub fn set_flag_harmonization(&mut self, harmonize_flags: bool) {
        self.harmonize_flags_ = harmonize_flags;
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.is-definition-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.is-definition-fn]
    pub fn is_definition(&self, name: &str) -> bool {
        self.definitions_.contains_key(name)
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.is-function-definition-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.is-function-definition-fn]
    pub fn is_function_definition(&self, name: &str) -> bool {
        self.function_definitions_.contains_key(name)
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.undefine-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.undefine-fn]
    // (Drop of the owned-transducer map handles the C++ `delete it->second`.)
    pub fn undefine(&mut self, name: &str) {
        self.definitions_.remove(name);
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.define-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.define-fn]
    // C++ overload `define(name, const std::string& xre)`.
    pub fn define(&mut self, name: &str, xre: &str) -> bool {
        let compiled = self.compile(xre);
        if compiled.is_null() {
            if self.verbose_ {
                eprintln!(
                    "error: could not parse '{}', leaving '{}' undefined",
                    xre, name
                );
            }
            return false;
        }
        self.undefine(name);
        let tr = *unsafe { Box::from_raw(compiled) };
        self.definitions_.insert(name.to_string(), tr);
        true
    }

    // C++ overload `define(name, const HfstTransducer& transducer)`.
    pub fn define_transducer(&mut self, name: &str, transducer: &HfstTransducer) {
        self.undefine(name);
        self.definitions_
            .insert(name.to_string(), transducer.clone());
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.define-list-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.define-list-fn]
    pub fn define_list(&mut self, name: &str, symbol_list: &BTreeSet<String>) {
        self.list_definitions_
            .insert(name.to_string(), symbol_list.clone());
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.define-function-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.define-function-fn]
    pub fn define_function(&mut self, name: &str, arguments: u32, xre: &str) -> bool {
        self.function_arguments_.insert(name.to_string(), arguments);
        self.function_definitions_
            .insert(name.to_string(), xre.to_string());
        true
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.add-defined-multichar-symbol-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.add-defined-multichar-symbol-fn]
    // The C++ `defined_multichar_symbols_` global set (used only for a "used but
    // not defined" warning via check_multichar_symbol) was left off the struct;
    // no-op until a field is added.
    pub fn add_defined_multichar_symbol(&mut self, _symbol: &str) {}

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.remove-defined-multichar-symbols-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.remove-defined-multichar-symbols-fn]
    pub fn remove_defined_multichar_symbols(&mut self) {}

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.contained-only-comments-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.contained-only-comments-fn]
    pub fn contained_only_comments(&self) -> bool {
        CONTAINS_ONLY_COMMENTS.with(|c| c.get())
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.compile-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.compile-fn]
    // Returns a raw owning pointer (Box::into_raw) on success, or null on parse
    // failure / comments-only — matching the C++ `HfstTransducer*` contract the
    // facade frees via Box::from_raw.
    pub fn compile(&mut self, expression: &str) -> *mut HfstTransducer {
        match self.compile_impl(expression) {
            Some(t) => Box::into_raw(Box::new(t)),
            None => std::ptr::null_mut(),
        }
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.compile-first-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.compile-first-fn]
    // `allow_extra_text_at_end` semantics: parse the whole string, keep only the
    // first expression, and report chars_read as that expression's span end.
    pub fn compile_first(&mut self, expression: &str, chars_read: &mut u32) -> *mut HfstTransducer {
        CONTAINS_ONLY_COMMENTS.with(|c| c.set(false));
        match parse_all(expression) {
            Ok(exprs) if !exprs.is_empty() => {
                let first = &exprs[0];
                *chars_read = first.span.end() as u32;
                let mut t = self.eval(first);
                t.optimize();
                Box::into_raw(Box::new(t))
            }
            Ok(_) => {
                CONTAINS_ONLY_COMMENTS.with(|c| c.set(true));
                *chars_read = 0;
                std::ptr::null_mut()
            }
            Err(_) => {
                *chars_read = 0;
                std::ptr::null_mut()
            }
        }
    }

    // Internal compile driver: parse → eval root → optimize. None on parse error
    // or comments-only (the latter also flips the contains_only_comments flag,
    // matching the `XRE: (empty) { contains_only_comments = true; }` action).
    fn compile_impl(&mut self, src: &str) -> Option<HfstTransducer> {
        CONTAINS_ONLY_COMMENTS.with(|c| c.set(false));
        match parse(src) {
            Ok(expr) => {
                let mut t = self.eval(&expr);
                t.optimize();
                Some(t)
            }
            Err(_) => {
                // Distinguish comments-only (parse_all yields []) from a real
                // parse error.
                if let Ok(exprs) = parse_all(src) {
                    if exprs.is_empty() {
                        CONTAINS_ONLY_COMMENTS.with(|c| c.set(true));
                    }
                }
                None
            }
        }
    }
}

// ====================== core recursive evaluator ===========================
impl XreCompiler {
    // [spec:hfst:def:xre-utils.hfst.xre.compile-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.compile-fn]
    pub(crate) fn eval(&mut self, e: &SpannedXre) -> HfstTransducer {
        let fmt = self.format_;
        match &e.value {
            // ---- atoms (LABEL: HALFARC) ----
            XreExpr::Symbol(s) => self.label_from_halfarc(s),
            XreExpr::Epsilon => self.label_from_halfarc(internal_epsilon),
            XreExpr::Any => self.label_from_halfarc(internal_unknown),
            XreExpr::BoundaryMarker => self.label_from_halfarc(".#."),
            XreExpr::Curly(c) => self.xfst_curly_label_to_transducer(c, c),

            // ---- pair (`upper:lower`) ----
            XreExpr::Pair { upper, lower } => self.eval_pair(upper, lower),

            // ---- grouping ----
            XreExpr::Group(inner) => {
                // REGEXP11: [ REGEXP2 ] -> optimize().
                let mut t = self.eval(inner);
                t.optimize();
                t
            }
            XreExpr::Optional(inner) => {
                // REGEXP11: ( REGEXP2 ) -> optionalize().
                let mut t = self.eval(inner);
                t.optionalize();
                t
            }
            XreExpr::BracketedDotted(opt) => match opt {
                // `[. E .]` as a bare expression behaves as grouping; `[..]` is
                // epsilon (it only carries replace semantics in mapping
                // position, which is handled by MappingSide::Dotted).
                Some(inner) => self.eval(inner),
                None => HfstTransducer::new_symbol(internal_epsilon, fmt),
            },

            // ---- weighted (`E::w`) ----
            XreExpr::Weighted { expr, weight } => {
                let mut t = self.eval(expr);
                t.set_final_weights(*weight as f32, true);
                // `[E]::w` optimizes after weighting; bare `LABEL::w` does not.
                if matches!(expr.value, XreExpr::Group(_)) {
                    t.optimize();
                }
                t
            }

            // ---- operators ----
            XreExpr::Unary(op, inner) => self.eval_unary(*op, inner),
            XreExpr::Binary(op, l, r) => self.eval_binary(*op, l, r),

            // ---- repetition ----
            XreExpr::RepeatN(inner, n) => {
                let mut t = self.eval(inner);
                t.repeat_n(*n);
                t
            }
            XreExpr::RepeatNPlus(inner, n) => {
                // REGEXP9: repeat_n_plus($2 + 1).
                let mut t = self.eval(inner);
                t.repeat_n_plus(n.wrapping_add(1));
                t
            }
            XreExpr::RepeatNMinus(inner, n) => {
                // REGEXP9: repeat_n_minus($2 - 1).
                let mut t = self.eval(inner);
                t.repeat_n_minus(n.wrapping_sub(1));
                t
            }
            XreExpr::RepeatNToK(inner, n, k) => {
                let mut t = self.eval(inner);
                t.repeat_n_to_k(*n, *k);
                t
            }

            // ---- containment with explicit weight (`$::w E`) ----
            XreExpr::ContainmentWithWeight { expr, weight } => {
                let t = self.eval(expr);
                if !t.is_automaton() {
                    crate::HFST_THROW_MESSAGE!(
                        HfstException,
                        "Containment with weight only works with automata"
                    );
                }
                self.contains_with_weight(&t, *weight as f32)
            }

            // ---- function call ----
            XreExpr::FunctionCall { name, args } => self.eval_function_call(name, args),

            // ---- delegated to the sibling body ----
            XreExpr::Replace { arrow, rules } => self.eval_replace(*arrow, rules),
            XreExpr::Restriction { body, contexts } => self.eval_restriction(body, contexts),
            XreExpr::Substitute { haystack, what } => self.eval_substitute(haystack, what),
            XreExpr::ReadFile { kind, path } => self.eval_read_file(*kind, path),
        }
    }

    // LABEL: HALFARC. `?` (internal_unknown) becomes a single identity arc;
    // anything else is definition-expanded (gated on expand_definitions_).
    fn label_from_halfarc(&self, sym: &str) -> HfstTransducer {
        if sym == internal_unknown {
            HfstTransducer::new_symbol(internal_identity, self.format_)
        } else {
            self.expand_definition_sym(sym)
        }
    }

    // The `:` productions from LABEL / REGEXP11, dispatched on the kinds of the
    // two sides. Cross-product orderings (including the `{c}:[F]` swap that the
    // grammar performs at xre_parse.yy:1001) are preserved verbatim.
    fn eval_pair(&mut self, upper: &SpannedXre, lower: &SpannedXre) -> HfstTransducer {
        match (xre_pair_side_kind(upper), xre_pair_side_kind(lower)) {
            (Some(XrePairSide::Half(a)), Some(XrePairSide::Half(b))) => {
                self.xfst_label_to_transducer(&a, &b)
            }
            (Some(XrePairSide::Half(a)), Some(XrePairSide::Curly(c))) => {
                let mut up = self.xfst_label_to_transducer(&a, &a);
                let lo = self.xfst_curly_label_to_transducer(&c, &c);
                up.cross_product(&lo, false);
                up
            }
            (Some(XrePairSide::Curly(c)), Some(XrePairSide::Half(a))) => {
                let mut up = self.xfst_curly_label_to_transducer(&c, &c);
                let lo = self.xfst_label_to_transducer(&a, &a);
                up.cross_product(&lo, false);
                up
            }
            (Some(XrePairSide::Curly(c1)), Some(XrePairSide::Curly(c2))) => {
                self.xfst_curly_label_to_transducer(&c1, &c2)
            }
            (Some(XrePairSide::Half(a)), None) => {
                // HALFARC : [F]  -> expand_definition(a) x eval(F)
                let mut up = self.expand_definition_sym(&a);
                let lo = self.eval(lower);
                up.cross_product(&lo, false);
                up
            }
            (None, Some(XrePairSide::Half(b))) => {
                // [E] : HALFARC  -> eval(E) x expand_definition(b)
                let mut up = self.eval(upper);
                let lo = self.expand_definition_sym(&b);
                up.cross_product(&lo, false);
                up
            }
            (Some(XrePairSide::Curly(c)), None) => {
                // {c} : [F]  -> grammar computes eval(F).cross_product(curly).
                let cur = self.xfst_curly_label_to_transducer(&c, &c);
                let mut lo = self.eval(lower);
                lo.cross_product(&cur, false);
                lo
            }
            (None, Some(XrePairSide::Curly(c))) => {
                // [E] : {c}  -> eval(E) x curly
                let mut up = self.eval(upper);
                let lo = self.xfst_curly_label_to_transducer(&c, &c);
                up.cross_product(&lo, false);
                up
            }
            (None, None) => {
                // [E] : [F]  -> eval(E) x eval(F)
                let mut up = self.eval(upper);
                let lo = self.eval(lower);
                up.cross_product(&lo, false);
                up
            }
        }
    }

    // REGEXP8/9/10 unary operators.
    fn eval_unary(&mut self, op: UnaryOp, inner: &SpannedXre) -> HfstTransducer {
        match op {
            UnaryOp::Star => {
                let mut t = self.eval(inner);
                t.repeat_star();
                t
            }
            UnaryOp::Plus => {
                let mut t = self.eval(inner);
                t.repeat_plus();
                t
            }
            UnaryOp::Reverse => {
                let mut t = self.eval(inner);
                t.reverse();
                t
            }
            UnaryOp::Invert => {
                let mut t = self.eval(inner);
                t.invert();
                t
            }
            UnaryOp::UpperProject => {
                let mut t = self.eval(inner);
                t.input_project();
                t
            }
            UnaryOp::LowerProject => {
                let mut t = self.eval(inner);
                t.output_project();
                t
            }
            UnaryOp::Complement => {
                // ~A : [?:?]* - A, only for automata.
                let a = self.eval(inner);
                if !a.is_automaton() {
                    crate::HFST_THROW_MESSAGE!(
                        HfstException,
                        "Complement operator ~ is defined only for automata"
                    );
                }
                let mut complement = HfstTransducer::identity_pair(self.format_);
                complement.repeat_star();
                complement.optimize();
                complement.subtract(&a, true);
                complement.prune_alphabet(false);
                complement
            }
            UnaryOp::TermComplement => {
                // \A : [?] - A
                let a = self.eval(inner);
                let mut any = HfstTransducer::new_symbol(internal_identity, self.format_);
                any.subtract(&a, true);
                any
            }
            UnaryOp::Containment => {
                // $A : transducers fall back to simple containment; automata use
                // the weighted-rule path with weight 0.
                let a = self.eval(inner);
                if !a.is_automaton() {
                    self.contains(&a)
                } else {
                    self.contains_with_weight(&a, 0.0)
                }
            }
            UnaryOp::ContainmentOnce => {
                let a = self.eval(inner);
                self.contains_once(&a)
            }
            UnaryOp::ContainmentOpt => {
                let a = self.eval(inner);
                self.contains_once_optional(&a)
            }
        }
    }

    // REGEXP2/3/5/6/7 binary operators.
    fn eval_binary(&mut self, op: BinaryOp, l: &SpannedXre, r: &SpannedXre) -> HfstTransducer {
        match op {
            BinaryOp::Compose => {
                let mut left = self.eval(l);
                let mut right = self.eval(r);
                // Flag-diacritic harmonization only matters when flag
                // harmonization is enabled; the verbose "not harmonized" warning
                // is skipped (has_flags() in the facade is a deferred port).
                if self.harmonize_flags_
                    && left.has_flag_diacritics()
                    && right.has_flag_diacritics()
                {
                    left.harmonize_flag_diacritics(&mut right, true);
                }
                left.compose(&right, self.harmonize_);
                left.optimize();
                left
            }
            BinaryOp::CrossProduct => {
                let mut left = self.eval(l);
                let right = self.eval(r);
                left.cross_product(&right, false);
                left.optimize();
                left
            }
            BinaryOp::LenientCompose => {
                let mut left = self.eval(l);
                let right = self.eval(r);
                left.lenient_composition(&right, false);
                left.optimize();
                left
            }
            BinaryOp::MergeRight => {
                // .m>. : merge left into right.
                let mut left = self.eval(l);
                let right = self.eval(r);
                let mut res = self.merge_first_to_second(&mut left, right);
                res.optimize();
                res
            }
            BinaryOp::MergeLeft => {
                // .<m. : merge right into left.
                let left = self.eval(l);
                let mut right = self.eval(r);
                let mut res = self.merge_first_to_second(&mut right, left);
                res.optimize();
                res
            }
            BinaryOp::Before => {
                let left = self.eval(l);
                let right = self.eval(r);
                before(&left, &right)
            }
            BinaryOp::After => {
                let left = self.eval(l);
                let right = self.eval(r);
                after(&left, &right)
            }
            BinaryOp::Union => {
                let mut left = self.eval(l);
                let right = self.eval(r);
                left.disjunct(&right, self.harmonize_);
                left
            }
            BinaryOp::Intersect => {
                let mut left = self.eval(l);
                let right = self.eval(r);
                left.intersect(&right, self.harmonize_);
                left.optimize();
                left.prune_alphabet(false);
                left
            }
            BinaryOp::Subtract => {
                let mut left = self.eval(l);
                let right = self.eval(r);
                left.subtract(&right, self.harmonize_);
                left.prune_alphabet(false);
                left
            }
            BinaryOp::UpperPriorityUnion => {
                let mut left = self.eval(l);
                let right = self.eval(r);
                left.priority_union(&right);
                left
            }
            BinaryOp::LowerPriorityUnion => {
                // invert both, priority_union, invert back.
                let mut left = self.eval(l);
                let mut right = self.eval(r);
                right.invert();
                left.invert();
                left.priority_union(&right);
                left.invert();
                left
            }
            BinaryOp::Concatenate => {
                let mut left = self.eval(l);
                let right = self.eval(r);
                left.concatenate(&right, self.harmonize_);
                left
            }
            BinaryOp::Ignoring => {
                // harmonize (force), then insert_freely without harmonization.
                let mut left = self.eval(l);
                let mut right = self.eval(r);
                left.harmonize(&mut right, true);
                left.insert_freely(&right, false);
                left
            }
            // Operators the C++ grammar rejects with xreerror + YYABORT.
            BinaryOp::Shuffle => crate::HFST_THROW_MESSAGE!(HfstException, "No shuffle"),
            BinaryOp::UpperSubtract => crate::HFST_THROW_MESSAGE!(HfstException, "No upper minus"),
            BinaryOp::LowerSubtract => crate::HFST_THROW_MESSAGE!(HfstException, "No lower minus"),
            BinaryOp::IgnoreInternally => {
                crate::HFST_THROW_MESSAGE!(HfstException, "No ignoring internally")
            }
            BinaryOp::LeftQuotient => crate::HFST_THROW_MESSAGE!(HfstException, "No left quotient"),
        }
    }

    // LABEL: FUNCTION REGEXP_LIST ')'. Because eval is &self, the function
    // arguments are registered in a cloned, augmented compiler (the C++ flow
    // define_function_args -> recursive parse -> undefine_function_args, but
    // re-entrant). The canonical function key carries the trailing '(' that the
    // C++ FUNCTION_NAME token includes; nfst-xre strips it from the AST name, so
    // it is reconstructed here (and used to build the "@name N@" arg symbols).
    fn eval_function_call(&mut self, name: &str, args: &[SpannedXre]) -> HfstTransducer {
        let fname = format!("{}(", name);

        let arg_trs: Vec<HfstTransducer> = args.iter().map(|a| self.eval(a)).collect();
        let n_args = arg_trs.len();

        // is_valid_function_call: defined + correct arity.
        let expected = match self.function_arguments_.get(&fname) {
            Some(n) => *n,
            None => crate::HFST_THROW_MESSAGE!(
                HfstException,
                format!("No such function defined: '{}'", name)
            ),
        };
        if !self.function_definitions_.contains_key(&fname) {
            crate::HFST_THROW_MESSAGE!(
                HfstException,
                format!("No such function defined: '{}'", name)
            );
        }
        if expected as usize != n_args {
            crate::HFST_THROW_MESSAGE!(
                HfstException,
                format!(
                    "Wrong number of arguments: function '{}' expects {}, {} given",
                    name, expected, n_args
                )
            );
        }

        // define_function_args: definitions["@name N@"] = arg (1-based).
        let mut sub_defs = self.definitions_.clone();
        for (i, arg) in arg_trs.into_iter().enumerate() {
            sub_defs.insert(format!("@{}{}@", fname, i + 1), arg);
        }
        let mut sub = XreCompiler {
            definitions_: sub_defs,
            function_definitions_: self.function_definitions_.clone(),
            function_arguments_: self.function_arguments_.clone(),
            list_definitions_: self.list_definitions_.clone(),
            format_: self.format_,
            verbose_: self.verbose_,
            expand_definitions_: self.expand_definitions_,
            harmonize_: self.harmonize_,
            harmonize_flags_: self.harmonize_flags_,
        };

        // get_function_xre + recursive compile.
        let body = self
            .function_definitions_
            .get(&fname)
            .cloned()
            .expect("function definition present (checked above)");
        match parse(&body) {
            Ok(expr) => {
                let mut t = sub.eval(&expr);
                t.optimize();
                t
            }
            Err(_) => crate::HFST_THROW_MESSAGE!(
                HfstException,
                format!("Could not parse body of function '{}'", name)
            ),
        }
    }
}

// ===== body 1 (flattened, module scope) =====
// =====================================================================
// Body area: labels_replace_containment
// Ported from libhfst/src/parsers/xre_utils.cc + the Replace/Restriction/
// Substitute/Containment-with-weight semantic actions of xre_parse.yy.
//
// CONTRACT with the sibling (driver_eval) body:
//   * This area calls `self.eval(node: &SpannedXre) -> HfstTransducer`, the
//     central recursive AST evaluator, which the driver body MUST provide
//     (`&mut self`, since function-call evaluation mutates `definitions_`).
//   * This area EXPORTS (for the driver's dispatch arms): the four
//     `contains*` helpers, `eval_containment` (the `$` arm), the
//     `xfst_*_to_transducer` label builders, `expand_definition_sym/_tr`,
//     the function-arg helpers, `merge_first_to_second`, and the
//     `eval_replace/eval_restriction/eval_substitute/
//     eval_containment_with_weight` arms.
//   * Function-arg helpers expect `name` to be the key as stored by
//     `define_function` (i.e. WITH its trailing '(', as in C++ FUNCTION_NAME);
//     the FunctionCall arm must re-append '(' to the AST `name`.
// All cross-module references use fully-qualified `crate::...` paths to avoid
// colliding with the skeleton's / sibling's `use` imports in this same module.
// =====================================================================

// ---- former xre_parse.yy file-scope globals (folded onto thread-locals) ----

thread_local! {
    // xre_parse.yy:40 `bool has_weight_been_zeroed`
    static HAS_WEIGHT_BEEN_ZEROED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    // xre_utils.cc:201 `std::string substitution_function_symbol`
    static SUBSTITUTION_FUNCTION_SYMBOL: std::cell::RefCell<String> =
        const { std::cell::RefCell::new(String::new()) };
}

// xre_parse.yy:51 `bool is_weighted()`
fn is_weighted(format: ImplementationType) -> bool {
    format == ImplementationType::TROPICAL_OPENFST_TYPE
        || format == ImplementationType::LOG_OPENFST_TYPE
}

// xre_parse.yy:41 `float zero_weights(float f)`.
// NOTE: the C++ emits a one-time "ignoring weights in rule context" warning
// here; `transform_weights` takes a bare `fn(f32)->f32` that cannot read the
// instance `verbose_`, so the warning is dropped. The weight-zeroing behaviour
// is preserved exactly.
fn zero_weights(f: f32) -> f32 {
    HAS_WEIGHT_BEEN_ZEROED.with(|c| {
        if !c.get() && f != 0.0 {
            c.set(true);
        }
    });
    0.0
}

// [spec:hfst:def:xre-utils.hfst.xre.set-substitution-function-symbol-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.set-substitution-function-symbol-fn]
fn set_substitution_function_symbol(symbol: &str) {
    SUBSTITUTION_FUNCTION_SYMBOL.with(|s| {
        *s.borrow_mut() = symbol.to_string();
    });
}

// [spec:hfst:def:xre-utils.hfst.xre.substitution-function-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.substitution-function-fn]
fn substitution_function(p: &(String, String), sps: &mut BTreeSet<(String, String)>) -> bool {
    let sym = SUBSTITUTION_FUNCTION_SYMBOL.with(|s| s.borrow().clone());
    if p.0 == sym || p.1 == sym {
        sps.insert((sym.clone(), sym));
        return true;
    }
    false
}

// [spec:hfst:def:xre-utils.hfst.xre.has-non-identity-pairs-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.has-non-identity-pairs-fn]
fn has_non_identity_pairs(t: &HfstTransducer) -> bool {
    let basic = crate::hfst_basic_transducer::HfstBasicTransducer::from_transducer(t);
    let sps = basic.get_transition_pairs();
    for it in sps.iter() {
        if it.0 != it.1 {
            return true;
        }
    }
    false
}

// Builds the `$3` transducer of the substitute symbol-list grammar
// (xre_parse.yy SYMBOL_LIST). An empty list yields the empty transducer
// (the `SUB3: RIGHT_BRACKET` alternative).
fn build_symbol_list_transducer(
    symbols: &Vec<String>,
    format: ImplementationType,
) -> HfstTransducer {
    if symbols.is_empty() {
        return HfstTransducer::new_type(format);
    }
    let first = &symbols[0];
    let mut retval = if first.as_str() == crate::hfst_symbol_defs::internal_unknown {
        HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity, format)
    } else {
        HfstTransducer::new_symbol_pair(first, first, format)
    };
    for s in symbols.iter().skip(1) {
        let tmp = if s.as_str() == crate::hfst_symbol_defs::internal_unknown {
            HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity, format)
        } else {
            HfstTransducer::new_symbol_pair(s, s, format)
        };
        retval.disjunct(&tmp, false);
        retval.optimize();
    }
    retval
}

impl XreCompiler {
    // ----------------------------------------------------------------
    // Definitions
    // ----------------------------------------------------------------

    // xre_utils.cc:837 `HfstTransducer* expand_definition(const char* symbol)`
    fn expand_definition_sym(&self, symbol: &str) -> HfstTransducer {
        if self.expand_definitions_ {
            for (k, v) in self.definitions_.iter() {
                if k.as_str() == symbol {
                    return v.clone();
                }
            }
        }
        HfstTransducer::new_symbol_pair(symbol, symbol, self.format_)
    }

    // [spec:hfst:def:xre-utils.hfst.xre.expand-definition-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.expand-definition-fn]
    // xre_utils.cc:857 `HfstTransducer* expand_definition(HfstTransducer*, const char*)`
    fn expand_definition_tr(&self, tr: &mut HfstTransducer, symbol: &str) {
        if self.expand_definitions_ {
            for (k, v) in self.definitions_.iter() {
                if k.as_str() == symbol {
                    let alpha = v.get_alphabet();
                    let mut v_clone = v.clone();
                    tr.substitute_pair_with_transducer(
                        &(symbol.to_string(), symbol.to_string()),
                        &mut v_clone,
                        false, // do not harmonize
                    );
                    if !alpha.contains(symbol) {
                        tr.remove_from_alphabet(symbol);
                    }
                    break;
                }
            }
        }
    }

    // ----------------------------------------------------------------
    // Label -> transducer builders
    // ----------------------------------------------------------------

    // [spec:hfst:def:xre-utils.hfst.xre.xfst-label-to-transducer-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.xfst-label-to-transducer-fn]
    // xre_utils.cc:959
    fn xfst_label_to_transducer(&self, input: &str, output: &str) -> HfstTransducer {
        let fmt = self.format_;
        let input_is_definition = self.definitions_.contains_key(input);
        let output_is_definition = self.definitions_.contains_key(output);
        let input_is_unknown = input == crate::hfst_symbol_defs::internal_unknown;
        let output_is_unknown = output == crate::hfst_symbol_defs::internal_unknown;

        // definitions -> use cross-product
        if input_is_definition || output_is_definition {
            let mut retval;
            let tmp;
            if input_is_unknown {
                retval =
                    HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity, fmt);
                tmp = self.expand_definition_sym(output);
            } else if output_is_unknown {
                tmp = HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity, fmt);
                retval = self.expand_definition_sym(input);
            } else {
                retval = self.expand_definition_sym(input);
                tmp = self.expand_definition_sym(output);
            }
            retval.cross_product(&tmp, true);
            return retval;
        }

        // no definitions
        if input_is_unknown && output_is_unknown {
            let mut retval = HfstTransducer::new_symbol_pair(
                crate::hfst_symbol_defs::internal_unknown,
                crate::hfst_symbol_defs::internal_unknown,
                fmt,
            );
            let id = HfstTransducer::new_symbol_pair(
                crate::hfst_symbol_defs::internal_identity,
                crate::hfst_symbol_defs::internal_identity,
                fmt,
            );
            retval.disjunct(&id, true).minimize();
            retval
        } else if input_is_unknown {
            let mut retval = HfstTransducer::new_symbol_pair(
                crate::hfst_symbol_defs::internal_unknown,
                output,
                fmt,
            );
            let output_tr = HfstTransducer::new_symbol_pair(output, output, fmt);
            retval.disjunct(&output_tr, true).minimize();
            retval
        } else if output_is_unknown {
            let mut retval = HfstTransducer::new_symbol_pair(
                input,
                crate::hfst_symbol_defs::internal_unknown,
                fmt,
            );
            let input_tr = HfstTransducer::new_symbol_pair(input, input, fmt);
            retval.disjunct(&input_tr, true).minimize();
            retval
        } else {
            HfstTransducer::new_symbol_pair(input, output, fmt)
        }
    }

    // [spec:hfst:def:xre-utils.hfst.xre.xfst-curly-label-to-transducer-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.xfst-curly-label-to-transducer-fn]
    // xre_utils.cc:897
    fn xfst_curly_label_to_transducer(&self, input: &str, output: &str) -> HfstTransducer {
        let fmt = self.format_;
        let mut retval;

        if input == crate::hfst_symbol_defs::internal_unknown {
            let tok = crate::hfst_tokenizer::HfstTokenizer::new();
            let sv = tok.tokenize_one_level(output, false);
            let first_token = sv[0].clone();
            retval = HfstTransducer::new_symbol_pair(
                crate::hfst_symbol_defs::internal_unknown,
                &first_token,
                fmt,
            );
            for it in sv.iter() {
                let tmp = HfstTransducer::new_symbol_pair(it, &first_token, fmt);
                retval.disjunct(&tmp, false);
            }
            for it in sv.iter().skip(1) {
                let tmp = HfstTransducer::new_symbol_pair(
                    crate::hfst_symbol_defs::internal_epsilon,
                    it,
                    fmt,
                );
                retval.concatenate(&tmp, false);
            }
        } else if output == crate::hfst_symbol_defs::internal_unknown {
            let tok = crate::hfst_tokenizer::HfstTokenizer::new();
            let sv = tok.tokenize_one_level(input, false);
            let first_token = sv[0].clone();
            retval = HfstTransducer::new_symbol_pair(
                &first_token,
                crate::hfst_symbol_defs::internal_unknown,
                fmt,
            );
            for it in sv.iter() {
                let tmp = HfstTransducer::new_symbol_pair(&first_token, it, fmt);
                retval.disjunct(&tmp, false);
            }
            for it in sv.iter().skip(1) {
                let tmp = HfstTransducer::new_symbol_pair(
                    it,
                    crate::hfst_symbol_defs::internal_epsilon,
                    fmt,
                );
                retval.concatenate(&tmp, false);
            }
        } else {
            let mut tok = crate::hfst_tokenizer::HfstTokenizer::new();
            tok.add_multichar_symbol(crate::hfst_symbol_defs::internal_epsilon);
            retval = HfstTransducer::new_tokenized_pair(input, output, &tok, fmt);
        }

        retval.minimize(); // it should be safe to minimize
        retval
    }

    // ----------------------------------------------------------------
    // Containment helpers
    // ----------------------------------------------------------------

    // [spec:hfst:def:xre-utils.hfst.xre.contains-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.contains-fn]
    // xre_utils.cc:1082 — [?*] t [?*]
    fn contains(&self, t: &HfstTransducer) -> HfstTransducer {
        let mut any =
            HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity, self.format_);
        any.repeat_star().minimize();
        let mut retval = any.clone();
        retval.concatenate(t, true).concatenate(&any, true);
        retval.optimize();
        retval
    }

    // [spec:hfst:def:xre-utils.hfst.xre.contains-with-weight-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.contains-with-weight-fn]
    // xre_utils.cc:1097 — [ 0::weight -> 0 || _ [t] ] - [?* - $[t]]
    fn contains_with_weight(&self, t: &HfstTransducer, weight: f32) -> HfstTransducer {
        let fmt = self.format_;

        let mut weighted_epsilon =
            HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon, fmt);
        weighted_epsilon.set_final_weights(weight, false);
        let epsilon = HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon, fmt);

        // mapping: 0::weight -> 0
        let mut mapping_pair_vector: Vec<(HfstTransducer, HfstTransducer)> = Vec::new();
        mapping_pair_vector.push((weighted_epsilon, epsilon.clone()));

        // context: 0 _ [t]
        let mut context_pair_vector: Vec<(HfstTransducer, HfstTransducer)> = Vec::new();
        context_pair_vector.push((epsilon, t.clone()));

        let rule = crate::hfst_xerox_rules::Rule::new_mapping_context_repl_type(
            &mapping_pair_vector,
            &context_pair_vector,
            crate::hfst_xerox_rules::ReplaceType::REPL_UP,
        );
        let mut weighted_rule = crate::hfst_xerox_rules::replace_rule(&rule, false);

        // noT = ?* - $[t]
        let mut no_t =
            HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity, t.get_type());
        no_t.repeat_star().minimize();
        let one_or_more_t = self.contains(t);
        no_t.subtract(&one_or_more_t, true);
        no_t.optimize();

        // return [weighted_rule - noT]
        weighted_rule.subtract(&no_t, true);
        weighted_rule.optimize();
        weighted_rule
    }

    // [spec:hfst:def:xre-utils.hfst.xre.contains-once-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.contains-once-fn]
    // xre_utils.cc:1142
    pub fn contains_once(&self, c: &HfstTransducer) -> HfstTransducer {
        let fmt = self.format_;

        // any_star = [?*]
        let mut any_star =
            HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity, fmt);
        any_star.repeat_star().minimize();

        // any_plus = [?+]
        let mut any_plus =
            HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity, fmt);
        any_plus.repeat_plus().minimize();

        // t1 = [?+ c ?*]
        let mut t1 = any_plus.clone();
        t1.concatenate(c, true);
        t1.optimize();
        t1.concatenate(&any_star, true);
        t1.optimize();

        // t2 = [c ?*]
        let mut t2 = c.clone();
        t2.concatenate(&any_star, true);
        t2.optimize();

        // t1 = [[?+ c ?*] & [c ?*]]
        t1.intersect(&t2, true);

        // t3 = [[c ?+] & c]
        let mut t3 = c.clone();
        t3.concatenate(&any_plus, true);
        t3.optimize();
        t3.intersect(c, true);
        t3.optimize();

        // t1 = [t1 | t3]
        t1.disjunct(&t3, true);
        t1.optimize();

        // cont_t1 = $[t1]
        let cont_t1 = self.contains(&t1);
        // cont_c = $[c]
        let mut cont_c = self.contains(c);

        // $[c] - $[t1]
        cont_c.subtract(&cont_t1, true);
        cont_c.optimize();
        cont_c
    }

    // [spec:hfst:def:xre-utils.hfst.xre.contains-once-optional-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.contains-once-optional-fn]
    // xre_utils.cc:1194
    pub fn contains_once_optional(&self, t: &HfstTransducer) -> HfstTransducer {
        let fmt = self.format_;

        // neg_t = ~$[t]
        let cont_t = self.contains(t);
        let mut neg_t = HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity, fmt);
        neg_t.repeat_star();
        neg_t.optimize();
        neg_t.subtract(&cont_t, true);
        neg_t.optimize();

        let mut retval = self.contains_once(t);
        retval.disjunct(&neg_t, true);
        retval.optimize();
        retval
    }

    // Driver dispatch for the `$ E` (CONTAINMENT REGEXP8) production
    // (xre_parse.yy:896). Exposed so the unary `$` arm can call it.
    fn eval_containment(&mut self, t: &HfstTransducer) -> HfstTransducer {
        if has_non_identity_pairs(t) {
            if self.verbose_ {
                // NB: faithfully reproduces the C++ missing-space concatenation.
                eprint!("using transducer that is not an automatonin containment\n");
            }
            self.contains(t) // ..resort to simple containment
        } else {
            self.contains_with_weight(t, 0.0)
        }
    }

    // Driver dispatch arm for `XreExpr::ContainmentWithWeight`
    // (CONTAINMENT WEIGHT REGEXP8, xre_parse.yy:910).
    fn eval_containment_with_weight(&mut self, expr: &SpannedXre, weight: f64) -> HfstTransducer {
        let t = self.eval(expr);
        if has_non_identity_pairs(&t) {
            std::panic::panic_any("Containment with weight only works with automata".to_string());
        }
        self.contains_with_weight(&t, weight as f32)
    }

    // ----------------------------------------------------------------
    // Merge
    // ----------------------------------------------------------------

    // [spec:hfst:def:xre-utils.hfst.xre.merge-first-to-second-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.merge-first-to-second-fn]
    // xre_utils.cc:1214. `tr1` is optimized then merged into `tr2` (returned).
    fn merge_first_to_second(
        &self,
        tr1: &mut HfstTransducer,
        mut tr2: HfstTransducer,
    ) -> HfstTransducer {
        // Merge operation creates an XreCompiler that needs this information
        // below; otherwise it would overwrite all of it.
        let args = XreConstructorArguments {
            definitions: self.definitions_.clone(),
            function_definitions: self.function_definitions_.clone(),
            function_arguments: self.function_arguments_.clone(),
            list_definitions: self.list_definitions_.clone(),
            format: self.format_,
        };
        tr1.optimize();
        tr2.merge(tr1, &args);
        tr2
    }

    // ----------------------------------------------------------------
    // Function-call helpers (definitions named "@<name><N>@", 1-based)
    // `name` must include its trailing '(' as stored by `define_function`.
    // ----------------------------------------------------------------

    // [spec:hfst:def:xre-utils.hfst.xre.is-valid-function-call-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.is-valid-function-call-fn]
    fn is_valid_function_call(&self, name: &str, args: &Vec<HfstTransducer>) -> bool {
        let name2xre = self.function_definitions_.get(name);
        let name2args = self.function_arguments_.get(name);

        if name2xre.is_none() || name2args.is_none() {
            eprintln!("No such function defined: '{}'", name);
            return false;
        }

        let number_of_args = *name2args.unwrap();

        if number_of_args as usize != args.len() {
            eprintln!(
                "Wrong number of arguments: function '{}' expects {}, {} given",
                name,
                number_of_args as i32,
                args.len() as i32
            );
            return false;
        }
        true
    }

    // [spec:hfst:def:xre-utils.hfst.xre.get-function-xre-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.get-function-xre-fn]
    fn get_function_xre(&self, name: &str) -> Option<String> {
        self.function_definitions_.get(name).cloned()
    }

    // [spec:hfst:def:xre-utils.hfst.xre.define-function-args-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.define-function-args-fn]
    fn define_function_args(&mut self, name: &str, args: &Vec<HfstTransducer>) -> bool {
        if !self.is_valid_function_call(name, args) {
            return false;
        }
        let mut arg_number: u32 = 1;
        for it in args.iter() {
            let function_arg = format!("@{}{}@", name, arg_number);
            self.definitions_.insert(function_arg, it.clone());
            arg_number += 1;
        }
        true
    }

    // [spec:hfst:def:xre-utils.hfst.xre.undefine-function-args-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.undefine-function-args-fn]
    fn undefine_function_args(&mut self, name: &str) {
        let n = match self.function_arguments_.get(name) {
            Some(n) => *n,
            None => return,
        };
        for arg_number in 1..=n {
            let function_arg = format!("@{}{}@", name, arg_number);
            self.definitions_.remove(&function_arg);
        }
    }

    // ----------------------------------------------------------------
    // Warnings used in replace rules
    // ----------------------------------------------------------------

    // [spec:hfst:def:xre-utils.hfst.xre.warn-about-special-symbols-in-replace-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.warn-about-special-symbols-in-replace-fn]
    fn warn_about_special_symbols_in_replace(&self, t: &HfstTransducer) {
        if !self.verbose_ {
            return;
        }
        let alphabet = t.get_alphabet();
        for it in alphabet.iter() {
            if HfstTransducer::is_special_symbol(it)
                && it.as_str() != crate::hfst_symbol_defs::internal_epsilon
                && it.as_str() != crate::hfst_symbol_defs::internal_unknown
                && it.as_str() != crate::hfst_symbol_defs::internal_identity
            {
                eprintln!(
                    "warning: using special symbol '{}' in replace rule, use substitute instead",
                    it
                );
            }
        }
    }

    // ----------------------------------------------------------------
    // Replace (xre_parse.yy: REPLACE / PARALLEL_RULES / RULE / MAPPINGPAIR*)
    // ----------------------------------------------------------------

    // Driver dispatch arm for `XreExpr::Replace`.
    // Mirrors the `REPLACE: PARALLEL_RULES` action (xre_parse.yy:365): returns
    // the raw `replace*` result (the REGEXP2-level `.optimize()` is applied by
    // the driver where the grammar reduces a REPLACE to a REGEXP2).
    fn eval_replace(&mut self, arrow: ReplaceArrow, rules: &Vec<ReplaceRule>) -> HfstTransducer {
        let mut rule_vector: Vec<crate::hfst_xerox_rules::Rule> = Vec::new();
        for rule in rules.iter() {
            let r = self.build_replace_rule(rule);
            rule_vector.push(r);
        }

        match arrow {
            ReplaceArrow::Right => {
                crate::hfst_xerox_rules::replace_rule_vector(&rule_vector, false)
            }
            ReplaceArrow::OptionalRight => {
                crate::hfst_xerox_rules::replace_rule_vector(&rule_vector, true)
            }
            ReplaceArrow::Left => {
                crate::hfst_xerox_rules::replace_left_rule_vector(&rule_vector, false)
            }
            ReplaceArrow::OptionalLeft => {
                crate::hfst_xerox_rules::replace_left_rule_vector(&rule_vector, true)
            }
            ReplaceArrow::RtlLongest => {
                crate::hfst_xerox_rules::replace_rightmost_longest_match_rule_vector(&rule_vector)
            }
            ReplaceArrow::RtlShortest => {
                crate::hfst_xerox_rules::replace_rightmost_shortest_match_rule_vector(&rule_vector)
            }
            ReplaceArrow::LtrLongest => {
                crate::hfst_xerox_rules::replace_leftmost_longest_match_rule_vector(&rule_vector)
            }
            ReplaceArrow::LtrShortest => {
                crate::hfst_xerox_rules::replace_leftmost_shortest_match_rule_vector(&rule_vector)
            }
            // E_REPLACE_RIGHT_MARKUP / no replace left-right arrow in the
            // xfst grammar: `xreerror("Unhandled arrow stuff I suppose")`.
            ReplaceArrow::LeftRight | ReplaceArrow::OptionalLeftRight => {
                std::panic::panic_any("Unhandled arrow stuff I suppose".to_string())
            }
        }
    }

    fn build_replace_rule(&mut self, rule: &ReplaceRule) -> crate::hfst_xerox_rules::Rule {
        let mut mapping_pair_vector: Vec<(HfstTransducer, HfstTransducer)> = Vec::new();
        for mp in rule.mappings.iter() {
            let pair = self.build_mapping_pair(mp);
            mapping_pair_vector.push(pair);
        }

        match &rule.contexts {
            None => crate::hfst_xerox_rules::Rule::new_mapping(&mapping_pair_vector),
            Some(ctxs) => {
                // CONTEXT_MARK -> ReplaceType (xre_parse.yy:673)
                let repl_type = match ctxs.mark {
                    ContextMark::UpperUpper => crate::hfst_xerox_rules::ReplaceType::REPL_UP,
                    ContextMark::LowerUpper => crate::hfst_xerox_rules::ReplaceType::REPL_RIGHT,
                    ContextMark::UpperLower => crate::hfst_xerox_rules::ReplaceType::REPL_LEFT,
                    ContextMark::LowerLower => crate::hfst_xerox_rules::ReplaceType::REPL_DOWN,
                };
                let mut context_vector: Vec<(HfstTransducer, HfstTransducer)> = Vec::new();
                for cx in ctxs.items.iter() {
                    let pair = self.build_replace_context(cx);
                    context_vector.push(pair);
                }
                crate::hfst_xerox_rules::Rule::new_mapping_context_repl_type(
                    &mapping_pair_vector,
                    &context_vector,
                    repl_type,
                )
            }
        }
    }

    // xre_parse.yy MAPPINGPAIR alternatives.
    fn build_mapping_pair(&mut self, mp: &MappingPair) -> (HfstTransducer, HfstTransducer) {
        let fmt = self.format_;
        let upper = self.eval_mapping_side(&mp.upper);

        match &mp.kind {
            MappingKind::Plain { lower } => {
                let lower_tr = self.eval_mapping_side(lower);
                // Only the bare `A -> B` production warns (the dotted forms do
                // not): warn iff both sides are plain expressions.
                if matches!(mp.upper, MappingSide::Expr(_)) && matches!(lower, MappingSide::Expr(_))
                {
                    self.warn_about_special_symbols_in_replace(&upper);
                    self.warn_about_special_symbols_in_replace(&lower_tr);
                }
                (upper, lower_tr)
            }
            MappingKind::Markup { pre, post } => {
                // marks = (pre|0, post|0); tmpMappingPair = (upper, <empty>)
                let left_mark = match pre {
                    Some(s) => self.eval_mapping_side(s),
                    None => {
                        HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon, fmt)
                    }
                };
                let right_mark = match post {
                    Some(s) => self.eval_mapping_side(s),
                    None => {
                        HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon, fmt)
                    }
                };
                let marks = (left_mark, right_mark);
                let tmp_mapping_pair = (upper, HfstTransducer::new_type(fmt));
                crate::hfst_xerox_rules::create_mapping_for_mark_up_replace(
                    &tmp_mapping_pair,
                    &marks,
                )
            }
        }
    }

    // A mapping side: bare expr, `[. E .]`, or `[..]` (-> epsilon).
    fn eval_mapping_side(&mut self, side: &MappingSide) -> HfstTransducer {
        match side {
            MappingSide::Expr(e) => self.eval(&**e),
            MappingSide::Dotted(None) => {
                HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon, self.format_)
            }
            MappingSide::Dotted(Some(e)) => self.eval(&**e),
        }
    }

    // xre_parse.yy CONTEXT alternatives (replace contexts). Empty side -> 0.
    // Contexts must be automata, weights are zeroed, then optimize+prune.
    fn build_replace_context(&mut self, c: &ReplaceContext) -> (HfstTransducer, HfstTransducer) {
        let fmt = self.format_;
        let weighted = is_weighted(fmt);

        match (&c.left, &c.right) {
            (Some(l), Some(r)) => {
                let mut t1 = self.eval(&**l);
                let mut t2 = self.eval(&**r);
                if has_non_identity_pairs(&t1) {
                    std::panic::panic_any("Contexts need to be automata".to_string());
                }
                if has_non_identity_pairs(&t2) {
                    std::panic::panic_any("Contexts need to be automata".to_string());
                }
                if weighted {
                    HAS_WEIGHT_BEEN_ZEROED.with(|cc| cc.set(false));
                    t1.transform_weights(zero_weights);
                }
                t1.optimize().prune_alphabet(false);
                if weighted {
                    t2.transform_weights(zero_weights);
                    HAS_WEIGHT_BEEN_ZEROED.with(|cc| cc.set(false));
                }
                t2.optimize().prune_alphabet(false);
                (t1, t2)
            }
            (Some(l), None) => {
                let mut t1 = self.eval(&**l);
                if has_non_identity_pairs(&t1) {
                    std::panic::panic_any("Contexts need to be automata".to_string());
                }
                if weighted {
                    HAS_WEIGHT_BEEN_ZEROED.with(|cc| cc.set(false));
                    t1.transform_weights(zero_weights);
                    HAS_WEIGHT_BEEN_ZEROED.with(|cc| cc.set(false));
                }
                t1.optimize().prune_alphabet(false);
                (
                    t1,
                    HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon, fmt),
                )
            }
            (None, Some(r)) => {
                let mut t1 = self.eval(&**r);
                if has_non_identity_pairs(&t1) {
                    std::panic::panic_any("Contexts need to be automata".to_string());
                }
                if weighted {
                    HAS_WEIGHT_BEEN_ZEROED.with(|cc| cc.set(false));
                    t1.transform_weights(zero_weights);
                    HAS_WEIGHT_BEEN_ZEROED.with(|cc| cc.set(false));
                }
                t1.optimize().prune_alphabet(false);
                (
                    HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon, fmt),
                    t1,
                )
            }
            (None, None) => {
                let epsilon =
                    HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon, fmt);
                (epsilon.clone(), epsilon)
            }
        }
    }

    // ----------------------------------------------------------------
    // Restriction (xre_parse.yy REGEXP4 RIGHT_ARROW RESTR_CONTEXTS_VECTOR)
    // ----------------------------------------------------------------

    // Driver dispatch arm for `XreExpr::Restriction`.
    fn eval_restriction(
        &mut self,
        body: &SpannedXre,
        contexts: &Vec<RestrContext>,
    ) -> HfstTransducer {
        let center = self.eval(body);
        let mut context_vector: Vec<(HfstTransducer, HfstTransducer)> = Vec::new();
        for c in contexts.iter() {
            let pair = self.build_restr_context(c);
            context_vector.push(pair);
        }
        crate::hfst_xerox_rules::restriction(&center, &context_vector)
    }

    // xre_parse.yy RESTR_CONTEXT alternatives. One missing side -> 0 (epsilon),
    // both missing -> <empty> (the bare `_` form).
    fn build_restr_context(&mut self, c: &RestrContext) -> (HfstTransducer, HfstTransducer) {
        let fmt = self.format_;
        match (&c.left, &c.right) {
            (Some(l), Some(r)) => (self.eval(&**l), self.eval(&**r)),
            (Some(l), None) => {
                let t1 = self.eval(&**l);
                (
                    t1,
                    HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon, fmt),
                )
            }
            (None, Some(r)) => {
                let t1 = self.eval(&**r);
                (
                    HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon, fmt),
                    t1,
                )
            }
            (None, None) => (HfstTransducer::new_type(fmt), HfstTransducer::new_type(fmt)),
        }
    }

    // ----------------------------------------------------------------
    // Substitute (xre_parse.yy: SUB1 ... productions)
    // ----------------------------------------------------------------

    // Driver dispatch arm for `XreExpr::Substitute`.
    fn eval_substitute(&mut self, haystack: &SpannedXre, what: &SubstituteWhat) -> HfstTransducer {
        let fmt = self.format_;
        let mut hay = self.eval(haystack);

        match what {
            // `[ E, a:b, c:d ]  (xre_parse.yy:268)
            SubstituteWhat::Pair { from, to } => {
                hay.substitute_pair_with_pair(from, to);
                hay.optimize();
                hay
            }
            // `[ E, b, x y ]  (xre_parse.yy:276 SUB1 SUB2 SUB3)
            SubstituteWhat::Symbol {
                needle,
                replacement,
            } => {
                let hay_alpha = hay.get_alphabet();

                if self.definitions_.contains_key(needle) {
                    if self.verbose_ {
                        eprint!("using definition as an ordinary label, cannot substitute\n");
                    }
                    hay.optimize();
                    return hay;
                }
                if !hay_alpha.contains(needle) {
                    hay.optimize();
                    return hay;
                }

                // alpha is reassigned to the replacement's alphabet (used both
                // for the diacritic loop and the final remove-from-alphabet).
                let mut repl_tr = build_symbol_list_transducer(replacement, fmt);
                let alpha3 = repl_tr.get_alphabet();
                let tmp = (needle.clone(), needle.clone());
                let mut tmp_tr = hay.clone();

                let empty = HfstTransducer::new_type(fmt);
                let mut empty_replace_transducer = false;
                if empty.compare(&repl_tr, true) {
                    empty_replace_transducer = true;
                }
                if empty_replace_transducer {
                    // substitute all transitions {b:a, a:b, b:b} with b:b
                    set_substitution_function_symbol(needle);
                    tmp_tr.substitute_with_func(substitution_function);
                }
                // substitute b with x | y (no harmonization)
                tmp_tr.substitute_pair_with_transducer(&tmp, &mut repl_tr, false);

                if !empty_replace_transducer {
                    // [[a:b].i .o. b -> x|y].i (handles b appearing on the left side)
                    let mapping_pair = (
                        HfstTransducer::new_symbol_pair(needle, needle, fmt),
                        repl_tr.clone(),
                    );
                    let mut mapping_pair_vector: Vec<(HfstTransducer, HfstTransducer)> = Vec::new();
                    mapping_pair_vector.push(mapping_pair);
                    let rule = crate::hfst_xerox_rules::Rule::new_mapping(&mapping_pair_vector);
                    let mut replace_tr = crate::hfst_xerox_rules::replace_rule(&rule, false);

                    // allow flag diacritics to be replaced with themselves
                    for it in alpha3.iter() {
                        if crate::hfst_flag_diacritics::FdOperation::is_diacritic(it) {
                            replace_tr.insert_freely_pair(&(it.clone(), it.clone()), false);
                        }
                    }
                    replace_tr.optimize();
                    tmp_tr.compose(&replace_tr, true).optimize();
                    tmp_tr.invert().compose(&replace_tr, true).invert();
                }

                if !alpha3.contains(needle) {
                    tmp_tr.remove_from_alphabet(needle.as_str());
                }
                tmp_tr.optimize();
                tmp_tr
            }
        }
    }
}

// ===== integration shims: XreCompiler::new (both overloads via XreCompilerNew) + deferred eval_read_file =====
impl XreCompiler {
    /// `XreCompiler(ImplementationType)` / `XreCompiler(const XreConstructorArguments&)`
    /// — both C++ constructor overloads behind one entry point.
    pub fn new<A: XreCompilerNew>(arg: A) -> Self {
        arg.into_xre_compiler()
    }

    /// `@bin`/`@txt`/`@stxt`/`@pl`/`@re` file-load evaluation — deferred (file I/O
    /// + AT&T/prolog readers not yet ported).
    fn eval_read_file(&mut self, _kind: ReadKind, _path: &str) -> HfstTransducer {
        unimplemented!("deferred: ReadFile @-load file I/O")
    }
}
