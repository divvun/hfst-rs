//! ABSOLUTE-faithful C++->Rust port of HFST's XRE (Xerox regex) compiler,
//! RESTRUCTURED to walk the 'nfst-xre' typed AST instead of the original
//! Flex/Bison grammar. The AST-walk restructuring is the ONE sanctioned
//! structural deviation in this port: the transducer-building BEHAVIOUR must
//! still match the C++ semantic actions in 'xre_parse.yy' / 'xre_utils.cc'
//! exactly.
//!
//! Ported from 'libhfst/src/parsers/XreCompiler.{h,cc}' and
//! 'libhfst/src/parsers/xre_utils.{h,cc}'.
//!
//! # C++ globals folded into ['XreCompiler']
//!
//! The C++ implementation kept compilation state in 'xre_utils.cc' file-scope
//! globals ('definitions', 'function_definitions', 'function_arguments',
//! 'symbol_lists', 'format', 'expand_definitions', 'harmonize',
//! 'harmonize_flags', 'verbose'). Because this port walks the AST directly
//! and is re-entrant, those globals become instance fields on ['XreCompiler']
//! and the per-compile evaluation state, instead of process-wide mutable
//! statics.
//!
//! # Deferred (record as 'unimplemented!')
//!
//! - ['XreExpr::ReadFile'] — '@bin'/'@txt'/'@stxt'/'@pl'/'@re' file I/O loads.
//! - The prolog/regex '@'-loads reached through the same path.
//! - 'contains_twolc' (the two-level twolc-contains helper; "doesn't work at
//!   the moment" in the C++ source) — kept as a documented helper that panics
//!   with 'unimplemented!'.

#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)] // some 1:1-ported xre_utils helpers are not yet reached by every eval path
#![allow(clippy::too_many_arguments)]

use std::collections::{BTreeMap, BTreeSet};

use tracing::{error, warn};

use nfst_xre::{
    BinaryOp, ContextMark, MappingKind, MappingPair, MappingSide, ParseError, ReadKind,
    ReplaceArrow, ReplaceContext, ReplaceRule, RestrContext, SpannedXre, SubstituteWhat, UnaryOp,
    XreExpr, parse, parse_all,
};

use crate::backend::AlgebraBackend;
use crate::hfst_data_types::ImplementationType;
use crate::hfst_data_types::Symbol;
use crate::hfst_transducer::HfstTransducer;
use crate::virtual_flag_frontends::prepare_compose_flag_overlay;

/// Arguments bundle mirroring 'hfst::xre::XreConstructorArguments'
/// ('XreCompiler.h'). Carries the four definition maps used to seed a fresh
/// ['XreCompiler']; the C++ 'format' member is the type parameter 'B' now
/// ([dec:hfst:monomorphic-backends]). Owned 'HfstTransducer' values replace
/// the C++ 'HfstTransducer*' map values (the C++ destructor 'delete'd them;
/// ownership lives in the map here).
///
/// 'std::map' -> 'BTreeMap', 'std::set' -> 'BTreeSet' per port conventions.
// [spec:hfst:def:xre-compiler.hfst.xre.xre-constructor-arguments]
pub struct XreConstructorArguments<B: AlgebraBackend> {
    /// 'std::map<std::string, hfst::HfstTransducer*> definitions'.
    pub definitions: BTreeMap<Symbol, HfstTransducer<B>>,
    /// 'std::map<std::string, std::string> function_definitions'. The VALUE is
    /// the function-body regex source text (general text), so it stays String.
    pub function_definitions: BTreeMap<Symbol, String>,
    /// 'std::map<std::string, unsigned int> function_arguments'.
    pub function_arguments: BTreeMap<Symbol, u32>,
    /// 'std::map<std::string, std::set<std::string>> list_definitions'.
    pub list_definitions: BTreeMap<Symbol, BTreeSet<Symbol>>,
}

// Manual impl: a derived Clone would demand 'B: Clone', but
// 'HfstTransducer<B>' is Clone for every 'B: Backend' already.
impl<B: AlgebraBackend> Clone for XreConstructorArguments<B> {
    fn clone(&self) -> Self {
        XreConstructorArguments {
            definitions: self.definitions.clone(),
            function_definitions: self.function_definitions.clone(),
            function_arguments: self.function_arguments.clone(),
            list_definitions: self.list_definitions.clone(),
        }
    }
}

impl<B: AlgebraBackend> XreConstructorArguments<B> {
    /// Port of the 'XreConstructorArguments(...)' field-copy constructor
    /// (the 'format' parameter is the type parameter 'B' now).
    // [spec:hfst:def:xre-compiler.hfst.xre.xre-constructor-arguments.xre-constructor-arguments-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-constructor-arguments.xre-constructor-arguments-fn]
    pub fn new(
        definitions: BTreeMap<Symbol, HfstTransducer<B>>,
        function_definitions: BTreeMap<Symbol, String>,
        function_arguments: BTreeMap<Symbol, u32>,
        list_definitions: BTreeMap<Symbol, BTreeSet<Symbol>>,
    ) -> Self {
        XreConstructorArguments {
            definitions,
            function_definitions,
            function_arguments,
            list_definitions,
        }
    }
}

/// A compiler holding the information needed to compile XREs.
///
/// Port of 'hfst::xre::XreCompiler' plus the 'xre_utils.cc' file-scope globals
/// it relied on. Field names match the C++ members 1:1
/// ('definitions', 'function_definitions', 'function_arguments',
/// 'list_definitions', 'verbose'; the C++ 'format' member is the type
/// parameter 'B' — [dec:hfst:monomorphic-backends]), with the former globals
/// 'expand_definitions', 'harmonize', 'harmonize_flags' added as instance
/// state so compilation is re-entrant.
// [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler]
pub struct XreCompiler<B: AlgebraBackend> {
    /// 'std::map<std::string, hfst::HfstTransducer*> definitions'.
    /// Owned transducers (C++ stored raw pointers freed by '~XreCompiler').
    pub(crate) definitions: BTreeMap<Symbol, HfstTransducer<B>>,
    /// 'std::map<std::string, std::string> function_definitions'. The VALUE is
    /// the function-body regex source text (general text), so it stays String.
    pub(crate) function_definitions: BTreeMap<Symbol, String>,
    /// 'std::map<std::string, unsigned int> function_arguments'.
    pub(crate) function_arguments: BTreeMap<Symbol, u32>,
    /// 'std::map<std::string, std::set<std::string>> list_definitions'.
    pub(crate) list_definitions: BTreeMap<Symbol, BTreeSet<Symbol>>,
    /// 'bool verbose' — verbose warnings toggle.
    pub(crate) verbose: bool,
    /// 'xre_utils.cc' global 'bool expand_definitions' (default 'false'):
    /// whether a defined name expands to its stored transducer.
    pub(crate) expand_definitions: bool,
    /// 'xre_utils.cc' global 'bool harmonize' (default 'true'): whether binary
    /// operators harmonize their argument transducers.
    pub(crate) harmonize: bool,
    /// 'xre_utils.cc' global 'bool harmonize_flags' (default 'false'): whether
    /// composition harmonizes flag diacritics of its arguments.
    pub(crate) harmonize_flags: bool,
    /// Whether 'optimize' on built transducers minimizes (the former
    /// 'can_minimize' / 'set_minimization' file-static global, default 'true').
    /// hfst-regexp2fst's no-minimize option drives this; threaded into the
    /// 'optimize' calls of this compiler's evaluation via [`Self::opt_cfg`].
    pub(crate) minimize_result: bool,
    /// Whether composition treats flag diacritics as epsilons (the former
    /// 'flag_is_epsilon_in_composition' file-static global, default 'false').
    /// hfst-regexp2fst's '--xfst flag-is-epsilon' drives this; threaded into the
    /// 'compose' calls of this compiler's evaluation via [`Self::opt_cfg`].
    pub(crate) flag_is_epsilon: bool,
    /// Whether composition treats flag diacritics as ordinary symbols, Xerox-style
    /// (the former 'xerox_composition' file-static global, default 'false').
    /// hfst-regexp2fst's '--xerox-composition' drives this; threaded into the
    /// 'compose' calls of this compiler's evaluation via [`Self::opt_cfg`].
    pub(crate) xerox_composition: bool,
    /// Whether minimization encodes weights into labels first, so determinize
    /// runs boolean subset construction (the former 'hfst::set_encode_weights'
    /// process-global, default 'false'). hfst-regexp2fst's '-E' /
    /// '--encode-weights' drives this; threaded into the 'optimize' calls of
    /// this compiler's evaluation via [`Self::opt_cfg`].
    pub(crate) encode_weights: bool,
    /// Former 'xre_utils.cc' file-scope global 'bool contains_only_comments':
    /// per-compile flag set by 'compile'/'compile_first' and read by
    /// 'contained_only_comments'. Moved onto the instance to remove the
    /// thread-global mutable state.
    pub(crate) contains_only_comments: bool,
    /// The regex source currently being compiled, retained so diagnostics can
    /// render the offending snippet (ariadne). Empty until `compile` runs.
    pub(crate) source: String,
    /// Label shown in diagnostics for `source` (a file name, or `"<regex>"`).
    pub(crate) source_name: String,
    /// Byte span in `source` of the AST node currently being evaluated, updated
    /// as `eval` visits each spanned node; the anchor for `diag_error`/
    /// `diag_warning`.
    pub(crate) current_span: std::ops::Range<usize>,
}

// ──────────────────────────────────────────────────────────────────────────
// Method / helper roster filled by the body agents (declarations only here).
//
// Public API (XreCompiler.h surface; keep these signatures so the facade calls
// 'XreCompiler::new(type)', 'XreCompiler::new(&args)', 'compile(&str)',
// 'set_verbosity(bool)' keep type-checking):
//
//   fn new() -> XreCompiler<B>                                  // XreCompiler(ImplementationType)
//   fn new_with_args(&XreConstructorArguments<B>) -> XreCompiler<B> // XreCompiler(const XreConstructorArguments&)
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
//   fn compile(&mut self, xre: &str) -> Option<HfstTransducer>
//   [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.compile-first-fn]
//   fn compile_first(&mut self, xre: &str, chars_read: &mut u32) -> Option<HfstTransducer>
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
// definitions/format/expand_definitions/verbose):
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
// (new/new_with_args), the full public API, compile/compile_first, and the
// recursive 'eval'. It delegates the Replace/Restriction/Substitute/ReadFile
// arms and the transducer-building helpers (xfst_label_to_transducer, contains*,
// expand_definition_sym, merge_first_to_second, ...) to the sibling body — see
// the dependency list in NOTES.
// ===========================================================================

use crate::hfst_symbol_defs::{internal_epsilon, internal_identity, internal_unknown};
use crate::hfst_xerox_rules::{after, before};

// Classification of a ':' pair side. The nfst-xre parser only ever produces a
// halfarc atom (Symbol/Epsilon/Any/BoundaryMarker), a Curly, or a 'Group([E])'
// on each side of a Pair; 'None' from the classifier means "bracketed
// expression — evaluate it".
enum XrePairSide {
    Half(Symbol),
    Curly(Symbol),
}

fn xre_pair_side_kind(e: &SpannedXre) -> Option<XrePairSide> {
    match &e.value {
        XreExpr::Symbol(s) => Some(XrePairSide::Half(s.clone())),
        XreExpr::Epsilon => Some(XrePairSide::Half(Symbol::new(internal_epsilon))),
        XreExpr::Any => Some(XrePairSide::Half(Symbol::new(internal_unknown))),
        // nfst-xre actually emits '.#.' as Symbol(".#."); this arm is here for
        // completeness. Per the porting spec the boundary symbol is ".#.".
        XreExpr::BoundaryMarker => Some(XrePairSide::Half(Symbol::new(".#."))),
        XreExpr::Curly(s) => Some(XrePairSide::Curly(s.clone())),
        XreExpr::Pair { .. }
        | XreExpr::Weighted { .. }
        | XreExpr::ReadFile { .. }
        | XreExpr::FunctionCall { .. }
        | XreExpr::Group(_)
        | XreExpr::Optional(_)
        | XreExpr::BracketedDotted(_)
        | XreExpr::Unary(..)
        | XreExpr::Binary(..)
        | XreExpr::RepeatN(..)
        | XreExpr::RepeatNPlus(..)
        | XreExpr::RepeatNMinus(..)
        | XreExpr::RepeatNToK(..)
        | XreExpr::ContainmentWithWeight { .. }
        | XreExpr::Replace { .. }
        | XreExpr::Restriction { .. }
        | XreExpr::Substitute { .. } => None,
    }
}

// ============================ constructors =================================
// [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.xre-compiler-fn]
// [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.xre-compiler-fn]
//
// The two C++ ctor overloads differed only in whether the definition maps
// were seeded and which 'format' was targeted; the format is the type
// parameter 'B' now, so they become two plain constructors.
impl<B: AlgebraBackend> XreCompiler<B> {
    /// 'XreCompiler(ImplementationType)' — the target format is 'B'.
    pub fn new() -> XreCompiler<B> {
        XreCompiler {
            definitions: BTreeMap::new(),
            function_definitions: BTreeMap::new(),
            function_arguments: BTreeMap::new(),
            list_definitions: BTreeMap::new(),
            verbose: false,
            expand_definitions: false,
            harmonize: true,
            harmonize_flags: false,
            minimize_result: true,
            flag_is_epsilon: false,
            xerox_composition: false,
            encode_weights: false,
            contains_only_comments: false,
            source: String::new(),
            source_name: String::from("<regex>"),
            current_span: 0..0,
        }
    }

    /// 'XreCompiler(const XreConstructorArguments&)'.
    pub fn new_with_args(args: &XreConstructorArguments<B>) -> XreCompiler<B> {
        XreCompiler {
            definitions: args.definitions.clone(),
            function_definitions: args.function_definitions.clone(),
            function_arguments: args.function_arguments.clone(),
            list_definitions: args.list_definitions.clone(),
            verbose: false,
            expand_definitions: false,
            harmonize: true,
            harmonize_flags: false,
            minimize_result: true,
            flag_is_epsilon: false,
            xerox_composition: false,
            encode_weights: false,
            contains_only_comments: false,
            source: String::new(),
            source_name: String::from("<regex>"),
            current_span: 0..0,
        }
    }
}

impl<B: AlgebraBackend> Default for XreCompiler<B> {
    fn default() -> Self {
        Self::new()
    }
}

// ============================ public API ===================================
impl<B: AlgebraBackend> XreCompiler<B> {
    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.set-verbosity-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.set-verbosity-fn]
    pub fn set_verbosity(&mut self, verbose: bool) {
        self.verbose = verbose;
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.get-verbosity-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.get-verbosity-fn]
    pub fn get_verbosity(&self) -> bool {
        self.verbose
    }

    /// Name shown in source-anchored diagnostics (a file name). Set before
    /// `compile` so warnings point at the right source; defaults to `"<regex>"`,
    /// which is right for the usual inline-regex case.
    pub fn set_source_name(&mut self, name: &str) -> &mut Self {
        self.source_name = name.to_string();
        self
    }

    /// Render an error about a problem in the user's regex source, anchored at
    /// the span of the node currently being evaluated (ariadne).
    fn diag_error(&self, msg: &str) {
        crate::diag::emit(
            &self.source_name,
            &self.source,
            self.current_span.clone(),
            crate::diag::Severity::Error,
            msg,
        );
    }

    /// Render a warning about the user's regex source, anchored at the span of
    /// the node currently being evaluated (ariadne).
    fn diag_warning(&self, msg: &str) {
        crate::diag::emit(
            &self.source_name,
            &self.source,
            self.current_span.clone(),
            crate::diag::Severity::Warning,
            msg,
        );
    }

    /// Render every diagnostic carried by an nfst-xre parse failure, each
    /// anchored at its own span. The front-end parser reports syntax errors
    /// with byte spans into `self.source`; without this the caller's terse
    /// one-liner is all the user sees of a syntax error.
    fn diag_parse_error(&self, e: &ParseError) {
        for d in &e.diagnostics {
            crate::diag::emit(
                &self.source_name,
                &self.source,
                d.span.range.clone(),
                crate::diag::Severity::Error,
                &d.message,
            );
        }
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.set-error-stream-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.set-error-stream-fn]
    // Stream plumbing is deferred (the C++ error_ global is not ported); no-op.
    pub fn set_error_stream<T>(&mut self, _os: T) {}

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.get-error-stream-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.get-error-stream-fn]
    // Returned 'hfst::xre::error_' in C++. Stream plumbing is deferred (no error
    // stream object is modelled), so this is a no-op accessor.
    pub fn get_error_stream(&self) {}

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.get-output-to-console-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.get-output-to-console-fn]
    // Non-WINDOWS build: 'getOutputToConsole' returns 'false'.
    pub fn get_output_to_console(&self) -> bool {
        false
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.get-stream-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.get-stream-fn]
    // Static in C++. Non-WINDOWS build: 'get_stream(oss)' returns 'oss' unchanged.
    pub fn get_stream<T>(oss: T) -> T {
        oss
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.flush-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.flush-fn]
    // Static in C++. Non-WINDOWS build: 'flush(oss)' is a no-op.
    pub fn flush<T>(_oss: T) {}

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.set-expand-definitions-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.set-expand-definitions-fn]
    pub fn set_expand_definitions(&mut self, expand: bool) {
        self.expand_definitions = expand;
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.set-harmonization-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.set-harmonization-fn]
    pub fn set_harmonization(&mut self, harmonize: bool) {
        self.harmonize = harmonize;
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.set-flag-harmonization-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.set-flag-harmonization-fn]
    pub fn set_flag_harmonization(&mut self, harmonize_flags: bool) {
        self.harmonize_flags = harmonize_flags;
    }

    /// Set whether 'optimize' on built transducers minimizes (was the
    /// 'hfst::set_minimization' file-static global; hfst-regexp2fst's no-minimize
    /// option toggles it).
    pub fn set_minimize_result(&mut self, minimize_result: bool) {
        self.minimize_result = minimize_result;
    }

    /// Set whether composition treats flag diacritics as epsilons (was the
    /// 'hfst::set_flag_is_epsilon_in_composition' file-static global; the
    /// '--xfst flag-is-epsilon' option of hfst-regexp2fst toggles it).
    pub fn set_flag_is_epsilon(&mut self, flag_is_epsilon: bool) {
        self.flag_is_epsilon = flag_is_epsilon;
    }

    /// Set whether composition treats flag diacritics as ordinary symbols,
    /// Xerox-style (was the 'hfst::set_xerox_composition' file-static global; the
    /// '--xerox-composition' option of hfst-regexp2fst toggles it).
    pub fn set_xerox_composition(&mut self, xerox_composition: bool) {
        self.xerox_composition = xerox_composition;
    }

    /// Set whether minimization encodes weights into labels first (was the
    /// 'hfst::set_encode_weights' process-global; the '-E' / '--encode-weights'
    /// option of hfst-regexp2fst toggles it).
    pub fn set_encode_weights(&mut self, encode_weights: bool) {
        self.encode_weights = encode_weights;
    }

    /// The [`EngineConfig`](crate::hfst_transducer::EngineConfig) this compiler's
    /// 'optimize' / 'compose' calls run with: the C++ defaults except the
    /// engine-policy flags this compiler exposes ('minimization',
    /// 'flag_is_epsilon_in_composition', 'xerox_composition').
    pub(crate) fn opt_cfg(&self) -> crate::hfst_transducer::EngineConfig {
        crate::hfst_transducer::EngineConfig {
            minimization: self.minimize_result,
            flag_is_epsilon_in_composition: self.flag_is_epsilon,
            xerox_composition: self.xerox_composition,
            encode_weights: self.encode_weights,
            ..crate::hfst_transducer::EngineConfig::default()
        }
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.is-definition-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.is-definition-fn]
    pub fn is_definition(&self, name: &str) -> bool {
        self.definitions.contains_key(name)
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.is-function-definition-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.is-function-definition-fn]
    pub fn is_function_definition(&self, name: &str) -> bool {
        self.function_definitions.contains_key(name)
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.undefine-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.undefine-fn]
    // (Drop of the owned-transducer map handles the C++ 'delete it->second'.)
    pub fn undefine(&mut self, name: &str) {
        self.definitions.remove(name);
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.define-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.define-fn]
    // C++ overload 'define(name, const std::string& xre)'.
    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.get-positions-of-symbol-in-xre-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.get-positions-of-symbol-in-xre-fn]
    pub fn get_positions_of_symbol_in_xre(
        &mut self,
        _symbol: &str,
        xre: &str,
        positions: &mut std::collections::BTreeSet<u32>,
    ) -> bool {
        // The C++ implementation records positions through the flex/bison
        // scanner's global position_symbol/positions state populated during
        // compilation. That position-tracking lives in the lexer we do not port
        // (nfst replaces it), so here we can only validate that the xre
        // compiles; the position set stays empty.
        positions.clear();
        self.compile(xre).is_some()
    }

    pub fn define(&mut self, name: &str, xre: &str) -> bool {
        let Some(tr) = self.compile(xre) else {
            if self.verbose {
                self.diag_error(&format!(
                    "could not parse '{}', leaving '{}' undefined",
                    xre, name
                ));
            }
            return false;
        };
        self.undefine(name);
        self.definitions.insert(Symbol::new(name), tr);
        true
    }

    // C++ overload 'define(name, const HfstTransducer& transducer)'.
    pub fn define_transducer(&mut self, name: &str, transducer: &HfstTransducer<B>) {
        self.undefine(name);
        self.definitions
            .insert(Symbol::new(name), transducer.clone());
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.define-list-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.define-list-fn]
    pub fn define_list(&mut self, name: &str, symbol_list: &BTreeSet<Symbol>) {
        self.list_definitions
            .insert(Symbol::new(name), symbol_list.clone());
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.define-function-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.define-function-fn]
    pub fn define_function(&mut self, name: &str, arguments: u32, xre: &str) -> bool {
        self.function_arguments.insert(Symbol::new(name), arguments);
        self.function_definitions
            .insert(Symbol::new(name), xre.to_string());
        true
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.add-defined-multichar-symbol-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.add-defined-multichar-symbol-fn]
    // The C++ 'defined_multichar_symbols_' global set (used only for a "used but
    // not defined" warning via check_multichar_symbol) was left off the struct;
    // no-op until a field is added.
    pub fn add_defined_multichar_symbol(&mut self, _symbol: &str) {}

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.remove-defined-multichar-symbols-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.remove-defined-multichar-symbols-fn]
    pub fn remove_defined_multichar_symbols(&mut self) {}

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.contained-only-comments-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.contained-only-comments-fn]
    pub fn contained_only_comments(&self) -> bool {
        self.contains_only_comments
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.compile-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.compile-fn]
    // Returns the compiled transducer, or None on parse failure / comments-only
    // (the C++ 'HfstTransducer*' null contract expressed as an Option).
    pub fn compile(&mut self, expression: &str) -> Option<HfstTransducer<B>> {
        self.compile_impl(expression)
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.compile-first-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.compile-first-fn]
    // 'allow_extra_text_at_end' semantics: parse the whole string, keep only the
    // first expression, and report chars_read as that expression's span end.
    //
    // The merged free driver 'hfst::xre::compile_first' (xre_utils.cc) is folded
    // in here: it set 'allow_extra_text_at_end', ran the parser, then returned
    // 'last_compiled' and 'chars_read = cr'. The AST walk reproduces the same
    // behaviour without the flex/bison position counters.
    // [spec:hfst:def:xre-utils.hfst.xre.compile-first-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.compile-first-fn]
    pub fn compile_first(
        &mut self,
        expression: &str,
        chars_read: &mut u32,
    ) -> Option<HfstTransducer<B>> {
        // Retain the source so diagnostics can render the offending snippet.
        self.source = expression.to_string();
        self.contains_only_comments = false;
        // Whitespace/comment-only input: the C++ lexer consumed it to EOF and
        // set contains_only_comments; nfst's parse_all errors on it instead.
        if is_only_whitespace_or_comments(expression) {
            self.contains_only_comments = true;
            *chars_read = 0;
            return None;
        }
        // Reject pathologically deep nesting before it can overflow the parser's
        // recursion; report it as an ordinary parse failure, never an abort.
        if exceeds_max_nesting_depth(expression) {
            *chars_read = 0;
            return None;
        }
        match parse_all(expression) {
            Ok(exprs) if !exprs.is_empty() => {
                let first = &exprs[0];
                // nfst spans exclude the ';' terminator; the C++ lexer position
                // this mirrors sat just past it. Consume through the ';' so the
                // caller's next slice starts on the following expression.
                let bytes = expression.as_bytes();
                let mut end = first.span.end();
                while end < bytes.len() && bytes[end] != b';' {
                    end += 1;
                }
                if end < bytes.len() {
                    end += 1;
                }
                *chars_read = end as u32;
                let mut t = self.eval(first).ok()?;
                t.optimize_with_config(&self.opt_cfg()).ok()?;
                Some(t)
            }
            Ok(_) => {
                self.contains_only_comments = true;
                *chars_read = 0;
                None
            }
            Err(e) => {
                self.diag_parse_error(&e);
                *chars_read = 0;
                None
            }
        }
    }

    // Internal compile driver: parse → eval root → optimize. None on parse error
    // or comments-only (the latter also flips the contains_only_comments flag,
    // matching the 'XRE: (empty) { contains_only_comments = true; }' action).
    fn compile_impl(&mut self, src: &str) -> Option<HfstTransducer<B>> {
        // Retain the source so diagnostics can render the offending snippet.
        self.source = src.to_string();
        self.contains_only_comments = false;
        if is_only_whitespace_or_comments(src) {
            self.contains_only_comments = true;
            return None;
        }
        // Reject pathologically deep nesting before it can overflow the parser's
        // recursion; report it as an ordinary parse failure, never an abort.
        if exceeds_max_nesting_depth(src) {
            return None;
        }
        match parse(src) {
            Ok(expr) => {
                let mut t = self.eval(&expr).ok()?;
                t.optimize_with_config(&self.opt_cfg()).ok()?;
                Some(t)
            }
            Err(e) => {
                // Distinguish comments-only (parse_all yields []) from a real
                // parse error.
                if let Ok(exprs) = parse_all(src)
                    && exprs.is_empty()
                {
                    self.contains_only_comments = true;
                } else {
                    self.diag_parse_error(&e);
                }
                None
            }
        }
    }
}

/// Whether `src` contains only whitespace and `!`-to-end-of-line XRE comments
/// (what the C++ lexer silently consumed before reporting comments-only).
fn is_only_whitespace_or_comments(src: &str) -> bool {
    src.lines()
        .all(|line| line.trim_start().is_empty() || line.trim_start().starts_with('!'))
}

/// Ceiling on grouping-delimiter nesting depth accepted before parsing.
///
/// The `nfst-xre` parser is recursive-descent and the AST evaluator recurses
/// per node, so pathologically deep bracket/paren nesting (`[[[[…`) exhausts
/// even the worker thread's large stack and aborts the process (SIGABRT), which
/// no `catch_unwind` can recover. Rejecting over-deep input up front turns that
/// abort into an ordinary parse failure. The bound sits far below the empirical
/// overflow point (~12k) yet far above any hand-written grammar's real nesting.
const MAX_NESTING_DEPTH: usize = 4000;

/// Whether `src`'s grouping-delimiter nesting (`[`, `(`, `[.`) ever runs deeper
/// than [`MAX_NESTING_DEPTH`]. `%`-escaped characters and `"…"`-quoted literals
/// are skipped so their brackets never count toward the depth.
fn exceeds_max_nesting_depth(src: &str) -> bool {
    let mut depth: usize = 0;
    let mut in_quote = false;
    let mut chars = src.chars();
    while let Some(c) = chars.next() {
        match c {
            // A '%' escapes the next character (an ordinary literal, never a
            // grouping delimiter); consume it so its bracket does not count.
            '%' => {
                let _ = chars.next();
            }
            '"' => in_quote = !in_quote,
            _ if in_quote => {}
            '[' | '(' => {
                depth += 1;
                if depth > MAX_NESTING_DEPTH {
                    return true;
                }
            }
            ']' | ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    false
}

// ====================== core recursive evaluator ===========================
impl<B: AlgebraBackend> XreCompiler<B> {
    // [spec:hfst:def:xre-utils.hfst.xre.compile-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.compile-fn]
    pub(crate) fn eval(&mut self, e: &SpannedXre) -> crate::error::Result<HfstTransducer<B>> {
        // Anchor any diagnostic emitted while evaluating this node at its span.
        self.current_span = e.span.range.clone();
        Ok(match &e.value {
            // ---- atoms (LABEL: HALFARC) ----
            XreExpr::Symbol(s) => self.label_from_halfarc(s)?,
            XreExpr::Epsilon => self.label_from_halfarc(internal_epsilon)?,
            XreExpr::Any => self.label_from_halfarc(internal_unknown)?,
            XreExpr::BoundaryMarker => self.label_from_halfarc(".#.")?,
            XreExpr::Curly(c) => self.xfst_curly_label_to_transducer(c, c)?,

            // ---- pair ('upper:lower') ----
            XreExpr::Pair { upper, lower } => self.eval_pair(upper, lower)?,

            // ---- grouping ----
            XreExpr::Group(inner) => {
                // REGEXP11: [ REGEXP2 ] -> optimize().
                let mut t = self.eval(inner)?;
                t.optimize_with_config(&self.opt_cfg())?;
                t
            }
            XreExpr::Optional(inner) => {
                // REGEXP11: ( REGEXP2 ) -> optionalize().
                let mut t = self.eval(inner)?;
                t.optionalize()?;
                t
            }
            XreExpr::BracketedDotted(opt) => match opt {
                // '[. E .]' as a bare expression behaves as grouping; '[..]' is
                // epsilon (it only carries replace semantics in mapping
                // position, which is handled by MappingSide::Dotted).
                Some(inner) => self.eval(inner)?,
                None => HfstTransducer::new_symbol(internal_epsilon)?,
            },

            // ---- weighted ('E::w') ----
            XreExpr::Weighted { expr, weight } => {
                let mut t = self.eval(expr)?;
                t.set_final_weights(*weight as f32, true)?;
                // '[E]::w' optimizes after weighting; bare 'LABEL::w' does not.
                if matches!(expr.value, XreExpr::Group(_)) {
                    t.optimize_with_config(&self.opt_cfg())?;
                }
                t
            }

            // ---- operators ----
            XreExpr::Unary(op, inner) => self.eval_unary(*op, inner)?,
            XreExpr::Binary(op, l, r) => self.eval_binary(*op, l, r)?,

            // ---- repetition ----
            XreExpr::RepeatN(inner, n) => {
                let mut t = self.eval(inner)?;
                t.repeat_n(*n)?;
                t
            }
            XreExpr::RepeatNPlus(inner, n) => {
                // REGEXP9: repeat_n_plus($2 + 1).
                let mut t = self.eval(inner)?;
                t.repeat_n_plus(n.wrapping_add(1))?;
                t
            }
            XreExpr::RepeatNMinus(inner, n) => {
                // REGEXP9: repeat_n_minus($2 - 1).
                let mut t = self.eval(inner)?;
                t.repeat_n_minus(n.wrapping_sub(1))?;
                t
            }
            XreExpr::RepeatNToK(inner, n, k) => {
                let mut t = self.eval(inner)?;
                t.repeat_n_to_k(*n, *k)?;
                t
            }

            // ---- containment with explicit weight ('$::w E') ----
            XreExpr::ContainmentWithWeight { expr, weight } => {
                let t = self.eval(expr)?;
                if !t.is_automaton()? {
                    crate::bail!(Hfst, "Containment with weight only works with automata");
                }
                self.contains_with_weight(&t, *weight as f32)?
            }

            // ---- function call ----
            XreExpr::FunctionCall { name, args } => self.eval_function_call(name, args)?,

            // ---- delegated to the sibling body ----
            XreExpr::Replace { arrow, rules } => self.eval_replace(*arrow, rules)?,
            XreExpr::Restriction { body, contexts } => self.eval_restriction(body, contexts)?,
            XreExpr::Substitute { haystack, what } => self.eval_substitute(haystack, what)?,
            XreExpr::ReadFile { kind, path } => self.eval_read_file(*kind, path)?,
        })
    }

    // LABEL: HALFARC. '?' (internal_unknown) becomes a single identity arc;
    // anything else is definition-expanded (gated on expand_definitions).
    fn label_from_halfarc(&self, sym: &str) -> crate::error::Result<HfstTransducer<B>> {
        if sym == internal_unknown {
            HfstTransducer::new_symbol(internal_identity)
        } else {
            self.expand_definition_sym(sym)
        }
    }

    // The ':' productions from LABEL / REGEXP11, dispatched on the kinds of the
    // two sides. Cross-product orderings (including the '{c}:[F]' swap that the
    // grammar performs at xre_parse.yy:1001) are preserved verbatim.
    fn eval_pair(
        &mut self,
        upper: &SpannedXre,
        lower: &SpannedXre,
    ) -> crate::error::Result<HfstTransducer<B>> {
        Ok(
            match (xre_pair_side_kind(upper), xre_pair_side_kind(lower)) {
                (Some(XrePairSide::Half(a)), Some(XrePairSide::Half(b))) => {
                    self.xfst_label_to_transducer(&a, &b)?
                }
                (Some(XrePairSide::Half(a)), Some(XrePairSide::Curly(c))) => {
                    let mut up = self.xfst_label_to_transducer(&a, &a)?;
                    let lo = self.xfst_curly_label_to_transducer(&c, &c)?;
                    up.cross_product(&lo, false)?;
                    up
                }
                (Some(XrePairSide::Curly(c)), Some(XrePairSide::Half(a))) => {
                    let mut up = self.xfst_curly_label_to_transducer(&c, &c)?;
                    let lo = self.xfst_label_to_transducer(&a, &a)?;
                    up.cross_product(&lo, false)?;
                    up
                }
                (Some(XrePairSide::Curly(c1)), Some(XrePairSide::Curly(c2))) => {
                    self.xfst_curly_label_to_transducer(&c1, &c2)?
                }
                (Some(XrePairSide::Half(a)), None) => {
                    // HALFARC : [F]  -> expand_definition(a) x eval(F)
                    let mut up = self.expand_definition_sym(&a)?;
                    let lo = self.eval(lower)?;
                    up.cross_product(&lo, false)?;
                    up
                }
                (None, Some(XrePairSide::Half(b))) => {
                    // [E] : HALFARC  -> eval(E) x expand_definition(b)
                    let mut up = self.eval(upper)?;
                    let lo = self.expand_definition_sym(&b)?;
                    up.cross_product(&lo, false)?;
                    up
                }
                (Some(XrePairSide::Curly(c)), None) => {
                    // {c} : [F]  -> grammar computes eval(F).cross_product(curly).
                    let cur = self.xfst_curly_label_to_transducer(&c, &c)?;
                    let mut lo = self.eval(lower)?;
                    lo.cross_product(&cur, false)?;
                    lo
                }
                (None, Some(XrePairSide::Curly(c))) => {
                    // [E] : {c}  -> eval(E) x curly
                    let mut up = self.eval(upper)?;
                    let lo = self.xfst_curly_label_to_transducer(&c, &c)?;
                    up.cross_product(&lo, false)?;
                    up
                }
                (None, None) => {
                    // [E] : [F]  -> eval(E) x eval(F)
                    let mut up = self.eval(upper)?;
                    let lo = self.eval(lower)?;
                    up.cross_product(&lo, false)?;
                    up
                }
            },
        )
    }

    // REGEXP8/9/10 unary operators.
    fn eval_unary(
        &mut self,
        op: UnaryOp,
        inner: &SpannedXre,
    ) -> crate::error::Result<HfstTransducer<B>> {
        Ok(match op {
            UnaryOp::Star => {
                let mut t = self.eval(inner)?;
                t.repeat_star()?;
                t
            }
            UnaryOp::Plus => {
                let mut t = self.eval(inner)?;
                t.repeat_plus()?;
                t
            }
            UnaryOp::Reverse => {
                let mut t = self.eval(inner)?;
                t.reverse()?;
                t
            }
            UnaryOp::Invert => {
                let mut t = self.eval(inner)?;
                t.invert()?;
                t
            }
            UnaryOp::UpperProject => {
                let mut t = self.eval(inner)?;
                t.input_project()?;
                t
            }
            UnaryOp::LowerProject => {
                let mut t = self.eval(inner)?;
                t.output_project()?;
                t
            }
            UnaryOp::Complement => {
                // ~A : [? | flags(A)]* - A, only for automata. A's flag
                // diacritics enter the identity universe as ORDINARY symbols so
                // subtract harmonization cannot erase them
                // ([spec:hfst:sem:xre-compiler.hfst.xre.complement-compilation-fn]):
                // this DIVERGES from upstream C++ XRE ([?:?]* - A, which swallows
                // flags) to match the Xerox transcript and HfstTransducer::negate().
                let a = self.eval(inner)?;
                if !a.is_automaton()? {
                    crate::bail!(Hfst, "Complement operator ~ is defined only for automata");
                }
                let mut complement = HfstTransducer::identity_with_flags_of(&a)?;
                complement.repeat_star()?;
                complement.optimize_with_config(&self.opt_cfg())?;
                complement.subtract(&a, true)?;
                complement.prune_alphabet(false)?;
                complement
            }
            UnaryOp::TermComplement => {
                // \A : [? | flags(A)] - A. Same flag-ordinary universe as ~A but
                // without the star, so \A matches any SINGLE symbol other than A
                // ([spec:hfst:sem:xre-compiler.hfst.xre.term-complement-compilation-fn]):
                // this DIVERGES from upstream C++ XRE ([?] - A, which swallows
                // flags) to match the Xerox transcript (hfst/hfst#349).
                let a = self.eval(inner)?;
                let mut any = HfstTransducer::identity_with_flags_of(&a)?;
                any.subtract(&a, true)?;
                any.prune_alphabet(false)?;
                any
            }
            UnaryOp::Containment => {
                // $A : transducers fall back to simple containment; automata use
                // the weighted-rule path with weight 0.
                let a = self.eval(inner)?;
                if !a.is_automaton()? {
                    self.contains(&a)?
                } else {
                    self.contains_with_weight(&a, 0.0)?
                }
            }
            UnaryOp::ContainmentOnce => {
                let a = self.eval(inner)?;
                self.contains_once(&a)?
            }
            UnaryOp::ContainmentOpt => {
                let a = self.eval(inner)?;
                self.contains_once_optional(&a)?
            }
        })
    }

    // REGEXP2/3/5/6/7 binary operators.
    fn eval_binary(
        &mut self,
        op: BinaryOp,
        l: &SpannedXre,
        r: &SpannedXre,
    ) -> crate::error::Result<HfstTransducer<B>> {
        Ok(match op {
            BinaryOp::Compose => {
                let mut left = self.eval(l)?;
                let mut right = self.eval(r)?;
                let config = self.opt_cfg();
                let overlay = prepare_compose_flag_overlay(
                    &mut left,
                    &mut right,
                    self.harmonize_flags,
                    &config,
                )?;
                left.compose_with_config_and_flag_overlay(
                    &right,
                    self.harmonize,
                    &config,
                    overlay.as_ref(),
                )?;
                left.optimize_with_config(&config)?;
                left
            }
            BinaryOp::CrossProduct => {
                let mut left = self.eval(l)?;
                let right = self.eval(r)?;
                left.cross_product(&right, false)?;
                left.optimize_with_config(&self.opt_cfg())?;
                left
            }
            BinaryOp::LenientCompose => {
                let mut left = self.eval(l)?;
                let right = self.eval(r)?;
                left.lenient_composition(&right, false)?;
                left.optimize_with_config(&self.opt_cfg())?;
                left
            }
            BinaryOp::MergeRight => {
                // .m>. : merge left into right.
                let mut left = self.eval(l)?;
                let right = self.eval(r)?;
                let mut res = self.merge_first_to_second(&mut left, right)?;
                res.optimize_with_config(&self.opt_cfg())?;
                res
            }
            BinaryOp::MergeLeft => {
                // .<m. : merge right into left.
                let left = self.eval(l)?;
                let mut right = self.eval(r)?;
                let mut res = self.merge_first_to_second(&mut right, left)?;
                res.optimize_with_config(&self.opt_cfg())?;
                res
            }
            BinaryOp::Before => {
                let left = self.eval(l)?;
                let right = self.eval(r)?;
                before(&left, &right)?
            }
            BinaryOp::After => {
                let left = self.eval(l)?;
                let right = self.eval(r)?;
                after(&left, &right)?
            }
            BinaryOp::Union => {
                let mut left = self.eval(l)?;
                let right = self.eval(r)?;
                left.disjunct(&right, self.harmonize)?;
                left
            }
            BinaryOp::Intersect => {
                let mut left = self.eval(l)?;
                let right = self.eval(r)?;
                left.intersect(&right, self.harmonize)?;
                left.optimize_with_config(&self.opt_cfg())?;
                left.prune_alphabet(false)?;
                left
            }
            BinaryOp::Subtract => {
                let mut left = self.eval(l)?;
                let right = self.eval(r)?;
                left.subtract(&right, self.harmonize)?;
                left.prune_alphabet(false)?;
                left
            }
            BinaryOp::UpperPriorityUnion => {
                let mut left = self.eval(l)?;
                let right = self.eval(r)?;
                left.priority_union(&right)?;
                left
            }
            BinaryOp::LowerPriorityUnion => {
                // invert both, priority_union, invert back.
                let mut left = self.eval(l)?;
                let mut right = self.eval(r)?;
                right.invert()?;
                left.invert()?;
                left.priority_union(&right)?;
                left.invert()?;
                left
            }
            BinaryOp::Concatenate => {
                let mut left = self.eval(l)?;
                let right = self.eval(r)?;
                left.concatenate(&right, self.harmonize)?;
                left
            }
            BinaryOp::Ignoring => {
                // harmonize (force), then insert_freely without harmonization.
                let mut left = self.eval(l)?;
                let mut right = self.eval(r)?;
                left.harmonize(&mut right, true)?;
                left.insert_freely(&right, false)?;
                left
            }
            // Operators the C++ grammar rejects with xreerror + YYABORT.
            BinaryOp::Shuffle => crate::bail!(Hfst, "No shuffle"),
            BinaryOp::UpperSubtract => crate::bail!(Hfst, "No upper minus"),
            BinaryOp::LowerSubtract => crate::bail!(Hfst, "No lower minus"),
            BinaryOp::IgnoreInternally => {
                crate::bail!(Hfst, "No ignoring internally")
            }
            BinaryOp::LeftQuotient => crate::bail!(Hfst, "No left quotient"),
        })
    }

    // LABEL: FUNCTION REGEXP_LIST ')'. Because eval is &self, the function
    // arguments are registered in a cloned, augmented compiler (the C++ flow
    // define_function_args -> recursive parse -> undefine_function_args, but
    // re-entrant).
    fn eval_function_call(
        &mut self,
        name: &str,
        args: &[SpannedXre],
    ) -> crate::error::Result<HfstTransducer<B>> {
        let arg_trs: Vec<HfstTransducer<B>> =
            args.iter()
                .map(|a| self.eval(a))
                .collect::<crate::error::Result<Vec<HfstTransducer<B>>>>()?;
        let n_args = arg_trs.len();

        // is_valid_function_call: defined + correct arity.
        let expected = match self.function_arguments.get(name) {
            Some(n) => *n,
            None => {
                crate::bail!(Hfst, format!("No such function defined: '{}'", name))
            }
        };
        if !self.function_definitions.contains_key(name) {
            crate::bail!(Hfst, format!("No such function defined: '{}'", name));
        }
        if expected as usize != n_args {
            crate::bail!(
                Hfst,
                format!(
                    "Wrong number of arguments: function '{}' expects {}, {} given",
                    name, expected, n_args
                )
            );
        }

        // define_function_args: definitions["@name N@"] = arg (1-based).
        let mut sub_defs = self.definitions.clone();
        for (i, arg) in arg_trs.into_iter().enumerate() {
            sub_defs.insert(Symbol::from(format!("@{}{}@", name, i + 1)), arg);
        }
        let mut sub = XreCompiler {
            definitions: sub_defs,
            function_definitions: self.function_definitions.clone(),
            function_arguments: self.function_arguments.clone(),
            list_definitions: self.list_definitions.clone(),
            verbose: self.verbose,
            expand_definitions: self.expand_definitions,
            harmonize: self.harmonize,
            harmonize_flags: self.harmonize_flags,
            minimize_result: self.minimize_result,
            flag_is_epsilon: self.flag_is_epsilon,
            xerox_composition: self.xerox_composition,
            encode_weights: self.encode_weights,
            contains_only_comments: false,
            source: self.source.clone(),
            source_name: self.source_name.clone(),
            current_span: self.current_span.clone(),
        };

        // get_function_xre + recursive compile.
        let body = self
            .function_definitions
            .get(name)
            .cloned()
            .expect("function definition present (checked above)");
        Ok(match parse(&body) {
            Ok(expr) => {
                let mut t = sub.eval(&expr)?;
                t.optimize_with_config(&self.opt_cfg())?;
                t
            }
            Err(e) => {
                // The span is relative to the stored function body, not the
                // caller's source, so anchor the snippet in the body text.
                for d in &e.diagnostics {
                    crate::diag::emit(
                        &format!("<function {}>", name),
                        &body,
                        d.span.range.clone(),
                        crate::diag::Severity::Error,
                        &d.message,
                    );
                }
                crate::bail!(Hfst, format!("Could not parse body of function '{}'", name))
            }
        })
    }
}

// ===== body 1 (flattened, module scope) =====
// =====================================================================
// Body area: labels_replace_containment
// Ported from libhfst/src/parsers/xre_utils.cc + the Replace/Restriction/
// Substitute/Containment-with-weight semantic actions of xre_parse.yy.
//
// CONTRACT with the sibling (driver_eval) body:
//   * This area calls 'self.eval(node: &SpannedXre) -> HfstTransducer', the
//     central recursive AST evaluator, which the driver body MUST provide
//     ('&mut self', since function-call evaluation mutates 'definitions').
//   * This area EXPORTS (for the driver's dispatch arms): the four
//     'contains*' helpers, 'eval_containment' (the '$' arm), the
//     'xfst*_to_transducer' label builders, 'expand_definition_sym/_tr',
//     the function-arg helpers, 'merge_first_to_second', and the
//     'eval_replace/eval_restriction/eval_substitute/
//     eval_containment_with_weight' arms.
//   * Function-arg helpers expect 'name' to be the key as stored by
//     'define_function' (i.e. WITH its trailing '(', as in C++ FUNCTION_NAME);
//     the FunctionCall arm must re-append '(' to the AST 'name'.
// All cross-module references use fully-qualified 'crate::...' paths to avoid
// colliding with the skeleton's / sibling's 'use' imports in this same module.
// =====================================================================

// xre_parse.yy:51 'bool is_weighted()'
fn is_weighted(format: ImplementationType) -> bool {
    format == ImplementationType::TROPICAL_OPENFST_TYPE
}

// xre_parse.yy:41 'float zero_weights(float f)'.
// NOTE: the C++ keeps a 'has_weight_been_zeroed' flag solely to emit a one-time
// "ignoring weights in rule context" warning; 'transform_weights' takes a bare
// 'fn(f32)->f32' that cannot read the instance 'verbose', so that warning is
// dropped. The flag was therefore write-only dead state and is removed entirely;
// the weight-zeroing behaviour (always returning 0.0) is preserved exactly.
fn zero_weights(_f: f32) -> f32 {
    0.0
}

// [spec:hfst:def:xre-utils.hfst.xre.has-non-identity-pairs-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.has-non-identity-pairs-fn]
fn has_non_identity_pairs<B: crate::backend::Backend>(t: &HfstTransducer<B>) -> bool {
    let basic = crate::hfst_basic_transducer::HfstBasicTransducer::from_transducer(t);
    let sps = basic.get_transition_pairs();
    for it in sps.iter() {
        if it.0 != it.1 {
            return true;
        }
    }
    false
}

// Builds the '$3' transducer of the substitute symbol-list grammar
// (xre_parse.yy SYMBOL_LIST). An empty list yields the empty transducer
// (the 'SUB3: RIGHT_BRACKET' alternative).
fn build_symbol_list_transducer<B: AlgebraBackend>(
    symbols: &[Symbol],
    cfg: &crate::hfst_transducer::EngineConfig,
) -> crate::error::Result<HfstTransducer<B>> {
    if symbols.is_empty() {
        return Ok(HfstTransducer::new());
    }
    let first = &symbols[0];
    let mut retval = if first.as_str() == crate::hfst_symbol_defs::internal_unknown {
        HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?
    } else {
        HfstTransducer::new_symbol_pair(first, first)?
    };
    for s in symbols.iter().skip(1) {
        let tmp = if s.as_str() == crate::hfst_symbol_defs::internal_unknown {
            HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?
        } else {
            HfstTransducer::new_symbol_pair(s, s)?
        };
        retval.disjunct(&tmp, false)?;
        retval.optimize_with_config(cfg)?;
    }
    Ok(retval)
}

// ---- former xre_utils.cc string / lexer-support free helpers ----
//
// These are 1:1 ports of the pure 'char*'-style helpers that the original
// flex/bison lexer leaned on. The nfst parser no longer drives them, but they
// are faithful ports kept for completeness. C 'char*' buffers become Rust
// 'String'; C 'strtol'/'strtod' are mirrored by the shared
// 'string_manipulation::parse_int_prefix' / 'parse_float_prefix' scanners.

use crate::string_manipulation::{parse_float_prefix, parse_int_prefix};

// [spec:hfst:def:xre-utils.hfst.xre.get-n-to-k-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.get-n-to-k-fn]
// xre_utils.cc:228. Parses the '{n,k}' / 'n,k' bounds of a repetition token.
fn get_n_to_k(s: &str) -> [i32; 2] {
    let b = s.as_bytes();
    let mut rv = [0i32; 2];
    if b.get(1).copied() == Some(b'{') {
        let (v0, endptr) = parse_int_prefix(b, 2);
        rv[0] = v0;
        let (v1, finalptr) = parse_int_prefix(b, endptr + 1);
        rv[1] = v1;
        assert!(b.get(finalptr).copied() == Some(b'}'));
    } else {
        let (v0, endptr) = parse_int_prefix(b, 1);
        rv[0] = v0;
        let (v1, finalptr) = parse_int_prefix(b, endptr + 1);
        rv[1] = v1;
        assert!(b.get(finalptr).copied().unwrap_or(0) == 0);
    }
    rv
}

// [spec:hfst:def:xre-utils.hfst.xre.strip-newline-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.strip-newline-fn]
// xre_utils.cc:268. Replaces every '\n'/'\r' byte with a nul, in place.
fn strip_newline(s: &str) -> String {
    let mut b = s.as_bytes().to_vec();
    for byte in b.iter_mut() {
        if *byte == b'\n' || *byte == b'\r' {
            *byte = 0;
        }
    }
    String::from_utf8_lossy(&b).into_owned()
}

// [spec:hfst:def:xre-utils.hfst.xre.count-lines-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.count-lines-fn]
// xre_utils.cc:282. Advances the 'lr'/'cr' counters over a chunk of input.
// The former 'hfst::xre::cr'/'lr' file-scope counters are now function-local
// (this faithful port is never driven by the nfst parser), removing the
// thread-global mutable state.
fn count_lines(s: &str) {
    let cr = std::cell::Cell::new(0u32);
    let lr = std::cell::Cell::new(1u32);
    let b = s.as_bytes();
    let mut i: usize = 0;
    while i < b.len() && b[i] != 0 {
        if b[i] == b'\n' {
            lr.set(lr.get() + 1);
        } else if b[i] == b'\r' {
            i += 1;
            if i < b.len() && b[i] == b'\n' {
                cr.set(cr.get() + 1);
            } else {
                i -= 1;
            }
            lr.set(lr.get() + 1);
        }
        cr.set(cr.get() + 1);
        i += 1;
    }
}

// [spec:hfst:def:xre-utils.hfst.xre.strip-curly-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.strip-curly-fn]
// xre_utils.cc:312. Drops an enclosing pair of curly braces.
fn strip_curly(s: &str) -> String {
    let c = s.as_bytes();
    let mut stripped: Vec<u8> = vec![0u8; c.len() + 1];
    let mut i: usize = 0;
    let mut p: usize = 0;
    while p < c.len() && c[p] != 0 {
        let next_is_nul = p + 1 >= c.len() || c[p + 1] == 0;
        if (c[p] == b'{' && i == 0) || (c[p] == b'}' && next_is_nul) {
            if next_is_nul {
                break;
            } else {
                stripped[i] = c[p + 1];
                i += 1;
                p += 2;
            }
        } else {
            stripped[i] = c[p];
            i += 1;
            p += 1;
        }
    }
    stripped[i] = 0;
    String::from_utf8_lossy(&stripped[..i]).into_owned()
}

// [spec:hfst:def:xre-utils.hfst.xre.strip-percents-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.strip-percents-fn]
// xre_utils.cc:347. Removes '%' escape prefixes.
fn strip_percents(s: &str) -> String {
    let c = s.as_bytes();
    let mut stripped: Vec<u8> = vec![0u8; c.len() + 1];
    let mut i: usize = 0;
    let mut p: usize = 0;
    while p < c.len() && c[p] != 0 {
        if c[p] == b'%' {
            if p + 1 >= c.len() || c[p + 1] == 0 {
                break;
            } else {
                stripped[i] = c[p + 1];
                i += 1;
                p += 2;
            }
        } else {
            stripped[i] = c[p];
            i += 1;
            p += 1;
        }
    }
    stripped[i] = 0;
    String::from_utf8_lossy(&stripped[..i]).into_owned()
}

// [spec:hfst:def:xre-utils.hfst.xre.add-percents-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.add-percents-fn]
// xre_utils.cc:381. Prefixes '%' before xfst special characters.
fn add_percents(s: &str) -> String {
    let b = s.as_bytes();
    let mut ns: Vec<u8> = Vec::with_capacity(b.len() * 2 + 1);
    for &ch in b {
        if matches!(
            ch,
            b'@' | b'-'
                | b' '
                | b'|'
                | b'!'
                | b':'
                | b';'
                | b'0'
                | b'\\'
                | b'&'
                | b'?'
                | b'$'
                | b'+'
                | b'*'
                | b'/'
                | b'_'
                | b'('
                | b')'
                | b'{'
                | b'}'
                | b'['
                | b']'
        ) {
            ns.push(b'%');
        }
        ns.push(ch);
    }
    String::from_utf8_lossy(&ns).into_owned()
}

// [spec:hfst:def:xre-utils.hfst.xre.get-quoted-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.get-quoted-fn]
// xre_utils.cc:408. Returns the substring between the first and last '"'.
fn get_quoted(s: &str) -> String {
    let b = s.as_bytes();
    let first = b
        .iter()
        .position(|&c| c == b'"')
        .expect("quoted token contains an opening quote");
    let qstart = first + 1;
    let qend = b
        .iter()
        .rposition(|&c| c == b'"')
        .expect("quoted token contains a closing quote");
    let len = qend - qstart;
    String::from_utf8_lossy(&b[qstart..qstart + len]).into_owned()
}

// [spec:hfst:def:xre-utils.hfst.xre.parse-quoted-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.parse-quoted-fn]
// xre_utils.cc:420. Unescapes a quoted string, writing its utf8 length to
// 'length'. 'throw' becomes 'panic_any'; the deferred error stream is replaced
// by stderr writes. The octal escape preserves the C bug of writing a nul
// WITHOUT advancing the write pointer (so nothing is appended).
fn parse_quoted(s: &str, length: &mut u32) -> String {
    let quoted = get_quoted(s);
    let qb = quoted.as_bytes();
    let mut rv: Vec<u8> = Vec::with_capacity(qb.len() + 1);
    let mut p: usize = 0;
    while p < qb.len() && qb[p] != 0 {
        let cur = qb[p];
        if cur == b'\n' || cur == b'\r' {
            std::panic::panic_any(
                "Unescaped newline characters found inside quoted string.".to_string(),
            );
        } else if cur != b'\\' {
            rv.push(cur);
            p += 1;
        } else {
            let nxt = qb.get(p + 1).copied().unwrap_or(0);
            match nxt {
                b'0' | b'1' | b'2' | b'3' | b'4' | b'5' | b'6' | b'7' => {
                    error!(
                        "XRE unimplemented: parse octal escape in {}",
                        String::from_utf8_lossy(&qb[p..])
                    );
                    p += 5;
                }
                b'a' => {
                    rv.push(0x07);
                    p += 2;
                }
                b'b' => {
                    rv.push(0x08);
                    p += 2;
                }
                b'f' => {
                    rv.push(0x0c);
                    p += 2;
                }
                b'n' => {
                    rv.push(b'\n');
                    p += 2;
                }
                b'r' => {
                    rv.push(b'\r');
                    p += 2;
                }
                b't' => {
                    rv.push(b'\t');
                    p += 2;
                }
                b'u' => {
                    error!(
                        "Unimplemented: parse unicode escapes in {}",
                        String::from_utf8_lossy(&qb[p..])
                    );
                    rv.push(0);
                    p += 6;
                }
                b'v' => {
                    rv.push(0x0b);
                    p += 2;
                }
                b'x' => {
                    // NB: the C source uses base 10 here (a bug); preserved.
                    let (i, endp) = parse_int_prefix(qb, p + 2);
                    if 0 < i && i <= 127 {
                        rv.push(i as u8);
                    } else {
                        error!("XRE unimplemented: parse \\x{}", i);
                        rv.push(0);
                    }
                    assert!(endp != p);
                    p = endp;
                }
                0 => {
                    warn!("End of line after \\ escape");
                    rv.push(0);
                    p += 1;
                }
                other => {
                    rv.push(other);
                    p += 2;
                }
            }
        }
    }
    // C builds a 'std::string' from the buffer, which truncates at the first
    // interior nul left by a '\u'/'\x'/end-of-line escape.
    let end = rv.iter().position(|&c| c == 0).unwrap_or(rv.len());
    let result = String::from_utf8_lossy(&rv[..end]).into_owned();
    *length =
        crate::hfst_tokenizer::HfstTokenizer::check_utf8_correctness_and_calculate_length(&result);
    result
}

// If 'str' is of form "@_<foo>_@", insert pair ("@_<foo>_@", "<foo>") into
// 'substitutions'.
// [spec:hfst:def:xre-utils.hfst.xre.insert-angle-bracket-substitutions-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.insert-angle-bracket-substitutions-fn]
// xre_utils.cc:553.
fn insert_angle_bracket_substitutions(
    str: &str,
    substitutions: &mut crate::hfst_symbol_defs::HfstSymbolSubstitutions,
) {
    if str.len() < 6 {
        return;
    }
    let b = str.as_bytes();
    if &b[0..3] == b"@_<" && &b[b.len() - 3..] == b">_@" {
        let substituting_str = &str[2..str.len() - 2];
        substitutions.insert(Symbol::new(str), Symbol::new(substituting_str));
    }
}

// [spec:hfst:def:xre-utils.hfst.xre.escape-enclosing-angle-brackets-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.escape-enclosing-angle-brackets-fn]
// xre_utils.cc:569. Wraps a '<...>' symbol as "@_<...>_@".
fn escape_enclosing_angle_brackets(s: &str) -> String {
    let b = s.as_bytes();
    if b.is_empty() || b[0] != b'<' {
        return s.to_string();
    }
    let i = b.len() - 1;
    if b[i] != b'>' {
        return s.to_string();
    }
    format!("@_{}_@", s)
}

// [spec:hfst:def:xre-utils.hfst.xre.unescape-enclosing-angle-brackets-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.unescape-enclosing-angle-brackets-fn]
// xre_utils.cc:591. Reverses the "@_<...>_@" wrapping for every alphabet symbol.
fn unescape_enclosing_angle_brackets<B: AlgebraBackend>(
    t: &mut HfstTransducer<B>,
) -> crate::error::Result<()> {
    let mut substitutions: crate::hfst_symbol_defs::HfstSymbolSubstitutions =
        crate::hfst_symbol_defs::HfstSymbolSubstitutions::new();
    let alpha = t.get_alphabet()?;
    for it in alpha.iter() {
        insert_angle_bracket_substitutions(it, &mut substitutions);
    }
    if substitutions.is_empty() {
        return Ok(());
    }
    t.substitute_substitutions(&substitutions)?;
    t.optimize_with_config(&crate::hfst_transducer::EngineConfig::default())?;
    Ok(())
}

// [spec:hfst:def:xre-utils.hfst.xre.get-weight-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.get-weight-fn]
// xre_utils.cc:610. Parses a trailing weight, skipping leading ' '/'\t'/';'.
fn get_weight(s: &str) -> f64 {
    let b = s.as_bytes();
    let mut weightstart: usize = 0;
    while weightstart < b.len()
        && b[weightstart] != 0
        && (b[weightstart] == b' ' || b[weightstart] == b'\t' || b[weightstart] == b';')
    {
        weightstart += 1;
    }
    let (val, endp) = parse_float_prefix(b, weightstart);
    assert!(endp != weightstart);
    val
}

// [spec:hfst:def:xre-utils.should-colourise-fn]
// [spec:hfst:sem:xre-utils.should-colourise-fn]
// xre_utils.cc:95. 'isatty(1)' -> stdout is a terminal.
fn should_colourise() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

impl<B: AlgebraBackend> XreCompiler<B> {
    // ----------------------------------------------------------------
    // Definitions
    // ----------------------------------------------------------------

    // xre_utils.cc:837 'HfstTransducer* expand_definition(const char* symbol)'
    fn expand_definition_sym(&self, symbol: &str) -> crate::error::Result<HfstTransducer<B>> {
        if self.expand_definitions {
            for (k, v) in self.definitions.iter() {
                if k.as_str() == symbol {
                    return Ok(v.clone());
                }
            }
            // PORT DIVERGENCE: upstream reads a bare token as a symbol, so a
            // function name compiles to a literal acceptor for its own name.
            // That is always a missing argument list or a typo.
            if self.function_definitions.contains_key(symbol) {
                crate::bail!(
                    Hfst,
                    format!(
                        "'{0}' is a function, not a network: apply it with arguments as '{0}(...)'. \
                         A bare name never resolves to a function — upstream compiles it as the \
                         literal symbol '{0}' instead, silently.",
                        symbol
                    )
                );
            }
        }
        HfstTransducer::new_symbol_pair(symbol, symbol)
    }

    // [spec:hfst:def:xre-utils.hfst.xre.expand-definition-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.expand-definition-fn]
    // xre_utils.cc:857 'HfstTransducer* expand_definition(HfstTransducer*, const char*)'
    fn expand_definition_tr(
        &self,
        tr: &mut HfstTransducer<B>,
        symbol: &str,
    ) -> crate::error::Result<()> {
        if self.expand_definitions {
            for (k, v) in self.definitions.iter() {
                if k.as_str() == symbol {
                    let alpha = v.get_alphabet()?;
                    let mut v_clone = v.clone();
                    tr.substitute_pair_with_transducer(
                        &(Symbol::new(symbol), Symbol::new(symbol)),
                        &mut v_clone,
                        false, // do not harmonize
                    )?;
                    if !alpha.contains(symbol) {
                        tr.remove_from_alphabet(symbol)?;
                    }
                    break;
                }
            }
        }
        Ok(())
    }

    // ----------------------------------------------------------------
    // Label -> transducer builders
    // ----------------------------------------------------------------

    // [spec:hfst:def:xre-utils.hfst.xre.xfst-label-to-transducer-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.xfst-label-to-transducer-fn]
    // xre_utils.cc:959
    fn xfst_label_to_transducer(
        &self,
        input: &str,
        output: &str,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let input_is_definition = self.definitions.contains_key(input);
        let output_is_definition = self.definitions.contains_key(output);
        let input_is_unknown = input == crate::hfst_symbol_defs::internal_unknown;
        let output_is_unknown = output == crate::hfst_symbol_defs::internal_unknown;

        // definitions -> use cross-product
        if input_is_definition || output_is_definition {
            let mut retval;
            let tmp;
            if input_is_unknown {
                retval = HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
                tmp = self.expand_definition_sym(output)?;
            } else if output_is_unknown {
                tmp = HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
                retval = self.expand_definition_sym(input)?;
            } else {
                retval = self.expand_definition_sym(input)?;
                tmp = self.expand_definition_sym(output)?;
            }
            retval.cross_product(&tmp, true)?;
            return Ok(retval);
        }

        // no definitions
        Ok(if input_is_unknown && output_is_unknown {
            let mut retval = HfstTransducer::new_symbol_pair(
                crate::hfst_symbol_defs::internal_unknown,
                crate::hfst_symbol_defs::internal_unknown,
            )?;
            let id = HfstTransducer::new_symbol_pair(
                crate::hfst_symbol_defs::internal_identity,
                crate::hfst_symbol_defs::internal_identity,
            )?;
            retval
                .disjunct(&id, true)?
                .minimize_with_config(&self.opt_cfg())?;
            retval
        } else if input_is_unknown {
            let mut retval =
                HfstTransducer::new_symbol_pair(crate::hfst_symbol_defs::internal_unknown, output)?;
            let output_tr = HfstTransducer::new_symbol_pair(output, output)?;
            retval
                .disjunct(&output_tr, true)?
                .minimize_with_config(&self.opt_cfg())?;
            retval
        } else if output_is_unknown {
            let mut retval =
                HfstTransducer::new_symbol_pair(input, crate::hfst_symbol_defs::internal_unknown)?;
            let input_tr = HfstTransducer::new_symbol_pair(input, input)?;
            retval
                .disjunct(&input_tr, true)?
                .minimize_with_config(&self.opt_cfg())?;
            retval
        } else {
            HfstTransducer::new_symbol_pair(input, output)?
        })
    }

    // [spec:hfst:def:xre-utils.hfst.xre.xfst-curly-label-to-transducer-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.xfst-curly-label-to-transducer-fn]
    // xre_utils.cc:897
    fn xfst_curly_label_to_transducer(
        &self,
        input: &str,
        output: &str,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let mut retval;

        if input == crate::hfst_symbol_defs::internal_unknown {
            let tok = crate::hfst_tokenizer::HfstTokenizer::new();
            let sv = tok.tokenize_one_level(output, false);
            let first_token = sv[0].clone();
            retval = HfstTransducer::new_symbol_pair(
                crate::hfst_symbol_defs::internal_unknown,
                &first_token,
            )?;
            for it in sv.iter() {
                let tmp = HfstTransducer::new_symbol_pair(it, &first_token)?;
                retval.disjunct(&tmp, false)?;
            }
            for it in sv.iter().skip(1) {
                let tmp =
                    HfstTransducer::new_symbol_pair(crate::hfst_symbol_defs::internal_epsilon, it)?;
                retval.concatenate(&tmp, false)?;
            }
        } else if output == crate::hfst_symbol_defs::internal_unknown {
            let tok = crate::hfst_tokenizer::HfstTokenizer::new();
            let sv = tok.tokenize_one_level(input, false);
            let first_token = sv[0].clone();
            retval = HfstTransducer::new_symbol_pair(
                &first_token,
                crate::hfst_symbol_defs::internal_unknown,
            )?;
            for it in sv.iter() {
                let tmp = HfstTransducer::new_symbol_pair(&first_token, it)?;
                retval.disjunct(&tmp, false)?;
            }
            for it in sv.iter().skip(1) {
                let tmp =
                    HfstTransducer::new_symbol_pair(it, crate::hfst_symbol_defs::internal_epsilon)?;
                retval.concatenate(&tmp, false)?;
            }
        } else {
            let mut tok = crate::hfst_tokenizer::HfstTokenizer::new();
            tok.add_multichar_symbol(crate::hfst_symbol_defs::internal_epsilon);
            retval = HfstTransducer::new_tokenized_pair(input, output, &tok)?;
        }

        retval.minimize_with_config(&self.opt_cfg())?; // it should be safe to minimize
        Ok(retval)
    }

    // ----------------------------------------------------------------
    // Containment helpers
    // ----------------------------------------------------------------

    // [spec:hfst:def:xre-utils.hfst.xre.contains-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.contains-fn]
    // xre_utils.cc:1082 — [?*] t [?*]
    fn contains(&self, t: &HfstTransducer<B>) -> crate::error::Result<HfstTransducer<B>> {
        let mut any = HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
        any.repeat_star()?.minimize_with_config(&self.opt_cfg())?;
        let mut retval = any.clone();
        retval.concatenate(t, true)?.concatenate(&any, true)?;
        retval.optimize_with_config(&self.opt_cfg())?;
        Ok(retval)
    }

    // [spec:hfst:def:xre-utils.hfst.xre.contains-with-weight-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.contains-with-weight-fn]
    // xre_utils.cc:1097 — [ 0::weight -> 0 || _ [t] ] - [?* - $[t]]
    fn contains_with_weight(
        &self,
        t: &HfstTransducer<B>,
        weight: f32,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let mut weighted_epsilon =
            HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon)?;
        weighted_epsilon.set_final_weights(weight, false)?;
        let epsilon = HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon)?;

        // mapping: 0::weight -> 0
        let mapping_pair_vector: Vec<(HfstTransducer<B>, HfstTransducer<B>)> =
            vec![(weighted_epsilon, epsilon.clone())];

        // context: 0 _ [t]
        let context_pair_vector: Vec<(HfstTransducer<B>, HfstTransducer<B>)> =
            vec![(epsilon, t.clone())];

        let rule = crate::hfst_xerox_rules::Rule::new_mapping_context_repl_type(
            &mapping_pair_vector,
            &context_pair_vector,
            crate::hfst_xerox_rules::ReplaceType::REPL_UP,
        )?;
        let mut weighted_rule = crate::hfst_xerox_rules::replace_rule(&rule, false)?;

        // noT = ?* - $[t]
        let mut no_t = HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
        no_t.repeat_star()?.minimize_with_config(&self.opt_cfg())?;
        let one_or_more_t = self.contains(t)?;
        no_t.subtract(&one_or_more_t, true)?;
        no_t.optimize_with_config(&self.opt_cfg())?;

        // return [weighted_rule - noT]
        weighted_rule.subtract(&no_t, true)?;
        weighted_rule.optimize_with_config(&self.opt_cfg())?;
        Ok(weighted_rule)
    }

    // [spec:hfst:def:xre-utils.hfst.xre.contains-once-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.contains-once-fn]
    // xre_utils.cc:1142
    pub fn contains_once(&self, c: &HfstTransducer<B>) -> crate::error::Result<HfstTransducer<B>> {
        // any_star = [?*]
        let mut any_star = HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
        any_star
            .repeat_star()?
            .minimize_with_config(&self.opt_cfg())?;

        // any_plus = [?+]
        let mut any_plus = HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
        any_plus
            .repeat_plus()?
            .minimize_with_config(&self.opt_cfg())?;

        // t1 = [?+ c ?*]
        let mut t1 = any_plus.clone();
        t1.concatenate(c, true)?;
        t1.optimize_with_config(&self.opt_cfg())?;
        t1.concatenate(&any_star, true)?;
        t1.optimize_with_config(&self.opt_cfg())?;

        // t2 = [c ?*]
        let mut t2 = c.clone();
        t2.concatenate(&any_star, true)?;
        t2.optimize_with_config(&self.opt_cfg())?;

        // t1 = [[?+ c ?*] & [c ?*]]
        t1.intersect(&t2, true)?;

        // t3 = [[c ?+] & c]
        let mut t3 = c.clone();
        t3.concatenate(&any_plus, true)?;
        t3.optimize_with_config(&self.opt_cfg())?;
        t3.intersect(c, true)?;
        t3.optimize_with_config(&self.opt_cfg())?;

        // t1 = [t1 | t3]
        t1.disjunct(&t3, true)?;
        t1.optimize_with_config(&self.opt_cfg())?;

        // cont_t1 = $[t1]
        let cont_t1 = self.contains(&t1)?;
        // cont_c = $[c]
        let mut cont_c = self.contains(c)?;

        // $[c] - $[t1]
        cont_c.subtract(&cont_t1, true)?;
        cont_c.optimize_with_config(&self.opt_cfg())?;
        Ok(cont_c)
    }

    // [spec:hfst:def:xre-utils.hfst.xre.contains-once-optional-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.contains-once-optional-fn]
    // xre_utils.cc:1194
    pub fn contains_once_optional(
        &self,
        t: &HfstTransducer<B>,
    ) -> crate::error::Result<HfstTransducer<B>> {
        // neg_t = ~$[t]
        let cont_t = self.contains(t)?;
        let mut neg_t = HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
        neg_t.repeat_star()?;
        neg_t.optimize_with_config(&self.opt_cfg())?;
        neg_t.subtract(&cont_t, true)?;
        neg_t.optimize_with_config(&self.opt_cfg())?;

        let mut retval = self.contains_once(t)?;
        retval.disjunct(&neg_t, true)?;
        retval.optimize_with_config(&self.opt_cfg())?;
        Ok(retval)
    }

    // Driver dispatch for the '$ E' (CONTAINMENT REGEXP8) production
    // (xre_parse.yy:896). Exposed so the unary '$' arm can call it.
    fn eval_containment(
        &mut self,
        t: &HfstTransducer<B>,
    ) -> crate::error::Result<HfstTransducer<B>> {
        if has_non_identity_pairs(t) {
            if self.verbose {
                // NB: faithfully reproduces the C++ missing-space concatenation.
                self.diag_warning("using transducer that is not an automatonin containment");
            }
            self.contains(t) // ..resort to simple containment
        } else {
            self.contains_with_weight(t, 0.0)
        }
    }

    // Driver dispatch arm for 'XreExpr::ContainmentWithWeight'
    // (CONTAINMENT WEIGHT REGEXP8, xre_parse.yy:910).
    fn eval_containment_with_weight(
        &mut self,
        expr: &SpannedXre,
        weight: f64,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let t = self.eval(expr)?;
        if has_non_identity_pairs(&t) {
            crate::bail!(Hfst, "Containment with weight only works with automata");
        }
        self.contains_with_weight(&t, weight as f32)
    }

    // ----------------------------------------------------------------
    // Merge
    // ----------------------------------------------------------------

    // [spec:hfst:def:xre-utils.hfst.xre.merge-first-to-second-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.merge-first-to-second-fn]
    // xre_utils.cc:1214. 'tr1' is optimized then merged into 'tr2' (returned).
    fn merge_first_to_second(
        &self,
        tr1: &mut HfstTransducer<B>,
        mut tr2: HfstTransducer<B>,
    ) -> crate::error::Result<HfstTransducer<B>> {
        // Merge operation creates an XreCompiler that needs this information
        // below; otherwise it would overwrite all of it.
        let args = XreConstructorArguments {
            definitions: self.definitions.clone(),
            function_definitions: self.function_definitions.clone(),
            function_arguments: self.function_arguments.clone(),
            list_definitions: self.list_definitions.clone(),
        };
        tr1.optimize_with_config(&self.opt_cfg())?;
        tr2.merge(tr1, &args)?;
        Ok(tr2)
    }

    // ----------------------------------------------------------------
    // Function-call helpers (definitions named "@<name><N>@", 1-based)
    // 'name' must include its trailing '(' as stored by 'define_function'.
    // ----------------------------------------------------------------

    // [spec:hfst:def:xre-utils.hfst.xre.is-valid-function-call-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.is-valid-function-call-fn]
    fn is_valid_function_call(&self, name: &str, args: &[HfstTransducer<B>]) -> bool {
        let name2xre = self.function_definitions.get(name);
        let name2args = self.function_arguments.get(name);

        if name2xre.is_none() || name2args.is_none() {
            self.diag_error(&format!("No such function defined: '{}'", name));
            return false;
        }

        let number_of_args = *name2args.expect("name2args is Some, checked above");

        if number_of_args as usize != args.len() {
            self.diag_error(&format!(
                "Wrong number of arguments: function '{}' expects {}, {} given",
                name,
                number_of_args as i32,
                args.len() as i32
            ));
            return false;
        }
        true
    }

    // [spec:hfst:def:xre-utils.hfst.xre.get-function-xre-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.get-function-xre-fn]
    fn get_function_xre(&self, name: &str) -> Option<String> {
        self.function_definitions.get(name).cloned()
    }

    // [spec:hfst:def:xre-utils.hfst.xre.define-function-args-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.define-function-args-fn]
    fn define_function_args(&mut self, name: &str, args: &[HfstTransducer<B>]) -> bool {
        if !self.is_valid_function_call(name, args) {
            return false;
        }
        for (i, it) in args.iter().enumerate() {
            let arg_number: u32 = i as u32 + 1;
            let function_arg = Symbol::from(format!("@{}{}@", name, arg_number));
            self.definitions.insert(function_arg, it.clone());
        }
        true
    }

    // [spec:hfst:def:xre-utils.hfst.xre.undefine-function-args-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.undefine-function-args-fn]
    fn undefine_function_args(&mut self, name: &str) {
        let n = match self.function_arguments.get(name) {
            Some(n) => *n,
            None => return,
        };
        for arg_number in 1..=n {
            let function_arg = format!("@{}{}@", name, arg_number);
            self.definitions.remove(function_arg.as_str());
        }
    }

    // ----------------------------------------------------------------
    // Warnings used in replace rules
    // ----------------------------------------------------------------

    // [spec:hfst:def:xre-utils.hfst.xre.warn-about-xfst-special-symbol-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.warn-about-xfst-special-symbol-fn]
    // xre_utils.cc:1245. Warns that xfst-special symbols ('all', '<...>') carry
    // no special meaning in hfst. The deferred error stream becomes stderr.
    fn warn_about_xfst_special_symbol(&self, symbol: &str) {
        if symbol == "all" {
            if self.verbose {
                self.diag_warning("symbol 'all' has no special meaning in hfst");
            }
            return;
        }

        let b = symbol.as_bytes();
        if b.is_empty() || b[0] != b'<' {
            return;
        }
        let mut max_index: usize = 1;
        while max_index < b.len() && b[max_index] != 0 {
            max_index += 1;
        }
        max_index -= 1;
        if max_index < 1 {
            return;
        }

        if b[max_index] != b'>' {
            return;
        }
        if !self.verbose {
            return;
        }
        self.diag_warning(&format!("'{} ' is an ordinary symbol in hfst", symbol));
    }

    // [spec:hfst:def:xre-utils.hfst.xre.warn-about-special-symbols-in-replace-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.warn-about-special-symbols-in-replace-fn]
    fn warn_about_special_symbols_in_replace(
        &self,
        t: &HfstTransducer<B>,
    ) -> crate::error::Result<()> {
        if !self.verbose {
            return Ok(());
        }
        let alphabet = t.get_alphabet()?;
        for it in alphabet.iter() {
            if crate::hfst_transducer::is_special_symbol(it)
                && it.as_str() != crate::hfst_symbol_defs::internal_epsilon
                && it.as_str() != crate::hfst_symbol_defs::internal_unknown
                && it.as_str() != crate::hfst_symbol_defs::internal_identity
            {
                self.diag_warning(&format!(
                    "using special symbol '{}' in replace rule, use substitute instead",
                    it
                ));
            }
        }
        Ok(())
    }

    // ----------------------------------------------------------------
    // Replace (xre_parse.yy: REPLACE / PARALLEL_RULES / RULE / MAPPINGPAIR*)
    // ----------------------------------------------------------------

    // Driver dispatch arm for 'XreExpr::Replace'.
    // Mirrors the 'REPLACE: PARALLEL_RULES' action (xre_parse.yy:365): returns
    // the raw 'replace*' result (the REGEXP2-level '.optimize_with_config(&self.opt_cfg())' is applied by
    // the driver where the grammar reduces a REPLACE to a REGEXP2).
    fn eval_replace(
        &mut self,
        arrow: ReplaceArrow,
        rules: &[ReplaceRule],
    ) -> crate::error::Result<HfstTransducer<B>> {
        let mut rule_vector: Vec<crate::hfst_xerox_rules::Rule<B>> = Vec::new();
        for rule in rules.iter() {
            let r = self.build_replace_rule(rule)?;
            rule_vector.push(r);
        }

        Ok(match arrow {
            ReplaceArrow::Right => {
                crate::hfst_xerox_rules::replace_rule_vector(&rule_vector, false)?
            }
            ReplaceArrow::OptionalRight => {
                crate::hfst_xerox_rules::replace_rule_vector(&rule_vector, true)?
            }
            ReplaceArrow::Left => {
                crate::hfst_xerox_rules::replace_left_rule_vector(&rule_vector, false)?
            }
            ReplaceArrow::OptionalLeft => {
                crate::hfst_xerox_rules::replace_left_rule_vector(&rule_vector, true)?
            }
            ReplaceArrow::RtlLongest => {
                crate::hfst_xerox_rules::replace_rightmost_longest_match_rule_vector(&rule_vector)?
            }
            ReplaceArrow::RtlShortest => {
                crate::hfst_xerox_rules::replace_rightmost_shortest_match_rule_vector(&rule_vector)?
            }
            ReplaceArrow::LtrLongest => {
                crate::hfst_xerox_rules::replace_leftmost_longest_match_rule_vector(&rule_vector)?
            }
            ReplaceArrow::LtrShortest => {
                crate::hfst_xerox_rules::replace_leftmost_shortest_match_rule_vector(&rule_vector)?
            }
            // E_REPLACE_RIGHT_MARKUP / no replace left-right arrow in the
            // xfst grammar: the C++ 'xreerror("Unhandled arrow stuff I
            // suppose")' aborted the parse; here it is a propagated parse
            // error so malformed input yields a clean diagnostic, never a panic.
            ReplaceArrow::LeftRight | ReplaceArrow::OptionalLeftRight => {
                crate::bail!(Hfst, "Unhandled arrow stuff I suppose")
            }
        })
    }

    fn build_replace_rule(
        &mut self,
        rule: &ReplaceRule,
    ) -> crate::error::Result<crate::hfst_xerox_rules::Rule<B>> {
        let mut mapping_pair_vector: Vec<(HfstTransducer<B>, HfstTransducer<B>)> = Vec::new();
        for mp in rule.mappings.iter() {
            let pair = self.build_mapping_pair(mp)?;
            mapping_pair_vector.push(pair);
        }

        Ok(match &rule.contexts {
            None => crate::hfst_xerox_rules::Rule::new_mapping(&mapping_pair_vector)?,
            Some(ctxs) => {
                // CONTEXT_MARK -> ReplaceType (xre_parse.yy:673)
                let repl_type = match ctxs.mark {
                    ContextMark::UpperUpper => crate::hfst_xerox_rules::ReplaceType::REPL_UP,
                    ContextMark::LowerUpper => crate::hfst_xerox_rules::ReplaceType::REPL_RIGHT,
                    ContextMark::UpperLower => crate::hfst_xerox_rules::ReplaceType::REPL_LEFT,
                    ContextMark::LowerLower => crate::hfst_xerox_rules::ReplaceType::REPL_DOWN,
                };
                let mut context_vector: Vec<(HfstTransducer<B>, HfstTransducer<B>)> = Vec::new();
                for cx in ctxs.items.iter() {
                    let pair = self.build_replace_context(cx)?;
                    context_vector.push(pair);
                }
                crate::hfst_xerox_rules::Rule::new_mapping_context_repl_type(
                    &mapping_pair_vector,
                    &context_vector,
                    repl_type,
                )?
            }
        })
    }

    // xre_parse.yy MAPPINGPAIR alternatives.
    fn build_mapping_pair(
        &mut self,
        mp: &MappingPair,
    ) -> crate::error::Result<(HfstTransducer<B>, HfstTransducer<B>)> {
        let upper = self.eval_mapping_side(&mp.upper)?;

        Ok(match &mp.kind {
            MappingKind::Plain { lower } => {
                let lower_tr = self.eval_mapping_side(lower)?;
                // Only the bare 'A -> B' production warns (the dotted forms do
                // not): warn iff both sides are plain expressions.
                if matches!(mp.upper, MappingSide::Expr(_)) && matches!(lower, MappingSide::Expr(_))
                {
                    self.warn_about_special_symbols_in_replace(&upper)?;
                    self.warn_about_special_symbols_in_replace(&lower_tr)?;
                }
                (upper, lower_tr)
            }
            MappingKind::Markup { pre, post } => {
                // marks = (pre|0, post|0); tmpMappingPair = (upper, <empty>)
                let left_mark = match pre {
                    Some(s) => self.eval_mapping_side(s)?,
                    None => HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon)?,
                };
                let right_mark = match post {
                    Some(s) => self.eval_mapping_side(s)?,
                    None => HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon)?,
                };
                let marks = (left_mark, right_mark);
                let tmp_mapping_pair = (upper, HfstTransducer::new());
                crate::hfst_xerox_rules::create_mapping_for_mark_up_replace(
                    &tmp_mapping_pair,
                    &marks,
                )?
            }
        })
    }

    // A mapping side: bare expr, '[. E .]', or '[..]' (-> epsilon).
    fn eval_mapping_side(&mut self, side: &MappingSide) -> crate::error::Result<HfstTransducer<B>> {
        Ok(match side {
            MappingSide::Expr(e) => self.eval(e)?,
            MappingSide::Dotted(None) => {
                HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon)?
            }
            MappingSide::Dotted(Some(e)) => self.eval(e)?,
        })
    }

    // xre_parse.yy CONTEXT alternatives (replace contexts). Empty side -> 0.
    // Contexts must be automata, weights are zeroed, then optimize+prune.
    fn build_replace_context(
        &mut self,
        c: &ReplaceContext,
    ) -> crate::error::Result<(HfstTransducer<B>, HfstTransducer<B>)> {
        let weighted = is_weighted(B::TYPE);

        Ok(match (&c.left, &c.right) {
            (Some(l), Some(r)) => {
                let mut t1 = self.eval(l)?;
                let mut t2 = self.eval(r)?;
                if has_non_identity_pairs(&t1) {
                    crate::bail!(Hfst, "Contexts need to be automata");
                }
                if has_non_identity_pairs(&t2) {
                    crate::bail!(Hfst, "Contexts need to be automata");
                }
                if weighted {
                    t1.transform_weights(zero_weights)?;
                }
                t1.optimize_with_config(&self.opt_cfg())?
                    .prune_alphabet(false)?;
                if weighted {
                    t2.transform_weights(zero_weights)?;
                }
                t2.optimize_with_config(&self.opt_cfg())?
                    .prune_alphabet(false)?;
                (t1, t2)
            }
            (Some(l), None) => {
                let mut t1 = self.eval(l)?;
                if has_non_identity_pairs(&t1) {
                    crate::bail!(Hfst, "Contexts need to be automata");
                }
                if weighted {
                    t1.transform_weights(zero_weights)?;
                }
                t1.optimize_with_config(&self.opt_cfg())?
                    .prune_alphabet(false)?;
                (
                    t1,
                    HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon)?,
                )
            }
            (None, Some(r)) => {
                let mut t1 = self.eval(r)?;
                if has_non_identity_pairs(&t1) {
                    crate::bail!(Hfst, "Contexts need to be automata");
                }
                if weighted {
                    t1.transform_weights(zero_weights)?;
                }
                t1.optimize_with_config(&self.opt_cfg())?
                    .prune_alphabet(false)?;
                (
                    HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon)?,
                    t1,
                )
            }
            (None, None) => {
                let epsilon =
                    HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon)?;
                (epsilon.clone(), epsilon)
            }
        })
    }

    // ----------------------------------------------------------------
    // Restriction (xre_parse.yy REGEXP4 RIGHT_ARROW RESTR_CONTEXTS_VECTOR)
    // ----------------------------------------------------------------

    // Driver dispatch arm for 'XreExpr::Restriction'.
    fn eval_restriction(
        &mut self,
        body: &SpannedXre,
        contexts: &[RestrContext],
    ) -> crate::error::Result<HfstTransducer<B>> {
        let center = self.eval(body)?;
        let mut context_vector: Vec<(HfstTransducer<B>, HfstTransducer<B>)> = Vec::new();
        for c in contexts.iter() {
            let pair = self.build_restr_context(c)?;
            context_vector.push(pair);
        }
        crate::hfst_xerox_rules::restriction(&center, &context_vector)
    }

    // xre_parse.yy RESTR_CONTEXT alternatives. One missing side -> 0 (epsilon),
    // both missing -> <empty> (the bare '_' form).
    fn build_restr_context(
        &mut self,
        c: &RestrContext,
    ) -> crate::error::Result<(HfstTransducer<B>, HfstTransducer<B>)> {
        Ok(match (&c.left, &c.right) {
            (Some(l), Some(r)) => (self.eval(l)?, self.eval(r)?),
            (Some(l), None) => {
                let t1 = self.eval(l)?;
                (
                    t1,
                    HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon)?,
                )
            }
            (None, Some(r)) => {
                let t1 = self.eval(r)?;
                (
                    HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon)?,
                    t1,
                )
            }
            (None, None) => (HfstTransducer::new(), HfstTransducer::new()),
        })
    }

    // ----------------------------------------------------------------
    // Substitute (xre_parse.yy: SUB1 ... productions)
    // ----------------------------------------------------------------

    // Driver dispatch arm for 'XreExpr::Substitute'.
    fn eval_substitute(
        &mut self,
        haystack: &SpannedXre,
        what: &SubstituteWhat,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let mut hay = self.eval(haystack)?;

        Ok(match what {
            // '[ E, a:b, c:d ]  (xre_parse.yy:268)
            SubstituteWhat::Pair { from, to } => {
                // The parser AST carries plain Strings; convert at the symbol
                // boundary.
                hay.substitute_pair_with_pair(
                    &(Symbol::new(&from.0), Symbol::new(&from.1)),
                    &(Symbol::new(&to.0), Symbol::new(&to.1)),
                )?;
                hay.optimize_with_config(&self.opt_cfg())?;
                hay
            }
            // '[ E, b, x y ]  (xre_parse.yy:276 SUB1 SUB2 SUB3)
            SubstituteWhat::Symbol {
                needle,
                replacement,
            } => {
                let hay_alpha = hay.get_alphabet()?;

                if self.definitions.contains_key(needle.as_str()) {
                    if self.verbose {
                        self.diag_warning(
                            "using definition as an ordinary label, cannot substitute",
                        );
                    }
                    hay.optimize_with_config(&self.opt_cfg())?;
                    return Ok(hay);
                }
                if !hay_alpha.contains(needle.as_str()) {
                    hay.optimize_with_config(&self.opt_cfg())?;
                    return Ok(hay);
                }

                // alpha is reassigned to the replacement's alphabet (used both
                // for the diacritic loop and the final remove-from-alphabet).
                let mut repl_tr: HfstTransducer<B> =
                    build_symbol_list_transducer(replacement, &self.opt_cfg())?;
                let alpha3 = repl_tr.get_alphabet()?;
                let tmp = (Symbol::new(needle), Symbol::new(needle));
                let mut tmp_tr = hay.clone();

                let empty = HfstTransducer::new();
                let mut empty_replace_transducer = false;
                if empty.compare(&repl_tr, true)? {
                    empty_replace_transducer = true;
                }
                if empty_replace_transducer {
                    // substitute all transitions {b:a, a:b, b:b} with b:b. The
                    // former 'substitution_function_symbol' global is captured by
                    // this closure instead.
                    let needle_sym = Symbol::new(needle);
                    tmp_tr.substitute_with_func(|p, sps| {
                        if p.0 == needle_sym || p.1 == needle_sym {
                            sps.insert((needle_sym.clone(), needle_sym.clone()));
                            return true;
                        }
                        false
                    })?;
                }
                // substitute b with x | y (no harmonization)
                tmp_tr.substitute_pair_with_transducer(&tmp, &mut repl_tr, false)?;

                if !empty_replace_transducer {
                    // [[a:b].i .o. b -> x|y].i (handles b appearing on the left side)
                    let mapping_pair = (
                        HfstTransducer::new_symbol_pair(needle, needle)?,
                        repl_tr.clone(),
                    );
                    let mapping_pair_vector: Vec<(HfstTransducer<B>, HfstTransducer<B>)> =
                        vec![mapping_pair];
                    let rule = crate::hfst_xerox_rules::Rule::new_mapping(&mapping_pair_vector)?;
                    let mut replace_tr = crate::hfst_xerox_rules::replace_rule(&rule, false)?;

                    // allow flag diacritics to be replaced with themselves
                    for it in alpha3.iter() {
                        if crate::hfst_flag_diacritics::FdOperation::is_diacritic(it) {
                            replace_tr.insert_freely_pair(&(it.clone(), it.clone()), false)?;
                        }
                    }
                    replace_tr.optimize_with_config(&self.opt_cfg())?;
                    tmp_tr
                        .compose_with_config(&replace_tr, true, &self.opt_cfg())?
                        .optimize_with_config(&self.opt_cfg())?;
                    tmp_tr
                        .invert()?
                        .compose_with_config(&replace_tr, true, &self.opt_cfg())?
                        .invert()?;
                }

                if !alpha3.contains(needle.as_str()) {
                    tmp_tr.remove_from_alphabet(needle.as_str())?;
                }
                tmp_tr.optimize_with_config(&self.opt_cfg())?;
                tmp_tr
            }
        })
    }
}

// ===== integration shims: deferred eval_read_file =====
impl<B: AlgebraBackend> XreCompiler<B> {
    /// '@bin'/'@txt'/'@stxt'/'@pl'/'@re' file-load evaluation. Ports the
    /// xre_parse.yy READ_BIN/READ_TEXT/READ_SPACED/READ_PROLOG/READ_RE actions.
    fn eval_read_file(
        &mut self,
        kind: ReadKind,
        path: &str,
    ) -> crate::error::Result<HfstTransducer<B>> {
        use crate::hfst_basic_transducer::HfstBasicTransducer;
        match kind {
            // READ_BIN: HfstInputStream instream(path); new HfstTransducer(instream);
            // t.convert(format). The stream reader returns the runtime sum
            // ('AnyTransducer'); 'B' here carries only 'AlgebraBackend' (the
            // facade seeds compilers without 'FromAnyTransducer'), so extract
            // the typed value through the interchange transducer — exactly
            // 'into_typed's general (convert) arm — keeping the facade
            // metadata like the C++ convert did.
            ReadKind::Binary => {
                let mut instream = crate::hfst_input_stream::HfstInputStream::new_filename(path)?;
                let any = instream.read()?;
                instream.close();
                let src_type = any.get_type();
                if src_type != B::TYPE {
                    // C++ hfst-xfst THROWS TransducerTypeMismatchException for a
                    // cross-backend `read regex @"file"` ("loading automata in
                    // different formats (OpenFst, foma) is not supported in XFST
                    // scripts"); we are more permissive and convert through the
                    // basic transducer. That is not a problem, but the
                    // conversion should never be silent: log it at info level
                    // (the tracing level filter decides visibility).
                    let mut line = format!(
                        "converting transducer type from {} to {} when reading from file '{}'",
                        crate::hfst_data_types::implementation_type_to_format(src_type),
                        crate::hfst_data_types::implementation_type_to_format(B::TYPE),
                        path
                    );
                    if !crate::hfst_transducer::is_safe_conversion(src_type, B::TYPE) {
                        line.push_str(" (loss of information is possible)");
                    }
                    tracing::info!("{}", line);
                }
                let net = any.to_basic()?;
                let mut retval: HfstTransducer<B> = HfstTransducer::new_from_basic(&net)?;
                retval.name = any.get_name();
                retval.props = any.get_properties().clone();
                Ok(retval)
            }
            // READ_TEXT / READ_SPACED: tokenize each line and disjunct it into a
            // basic transducer, then build a transducer of the compiler format and
            // optimize. READ_TEXT uses the multichar tokenizer; READ_SPACED splits
            // on spaces.
            ReadKind::Text | ReadKind::Spaced => {
                let Ok(contents) = std::fs::read_to_string(path) else {
                    crate::bail!(Hfst, format!("File cannot be opened: '{}'", path));
                };
                let mut tmp = HfstBasicTransducer::new();
                let tok = crate::hfst_tokenizer::HfstTokenizer::new();
                for raw in contents.lines() {
                    let line = strip_newline(raw);
                    let spv = if kind == ReadKind::Spaced {
                        crate::hfst_tokenizer::HfstTokenizer::tokenize_space_separated(&line)
                    } else {
                        tok.tokenize(&line, false)
                    };
                    tmp.disjunct_path(&spv, 0.0);
                }
                let mut retval = HfstTransducer::new_from_basic(&tmp)?;
                retval.optimize_with_config(&self.opt_cfg())?;
                Ok(retval)
            }
            // READ_PROLOG: read_in_prolog_format then build of the compiler format.
            ReadKind::Prolog => {
                let f = match std::fs::File::open(path) {
                    Ok(f) => f,
                    Err(_) => crate::bail!(Hfst, format!("File cannot be opened: '{}'", path)),
                };
                let mut reader = std::io::BufReader::new(f);
                let mut linecount: u32 = 0;
                let tmp =
                    HfstBasicTransducer::read_in_prolog_format_file(&mut reader, &mut linecount)?;
                let mut retval = HfstTransducer::new_from_basic(&tmp)?;
                retval.optimize_with_config(&self.opt_cfg())?;
                Ok(retval)
            }
            // READ_RE: read the file content and re-compile it as a regex (the C++
            // spins up a fresh scanner; the ported compiler re-parses the string).
            ReadKind::Regex => {
                let Ok(contents) = std::fs::read_to_string(path) else {
                    crate::bail!(Hfst, format!("File cannot be opened: '{}'", path));
                };
                self.compile(&contents)
                    .ok_or_else(|| crate::err!(Hfst, "read-regex: regex did not compile"))
            }
        }
    }
}
