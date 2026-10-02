//! The Xerox regular-expression compiler: walks the 'nfst-xre' syntax tree
//! and builds the transducer it denotes.

#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)] // some 1:1-ported xre_utils helpers are not yet reached by every eval path
#![allow(clippy::too_many_arguments)]

use std::collections::{BTreeMap, BTreeSet};

use nfst_xre::{BinaryOp, ParseError, SpannedXre, XreExpr};

use crate::backend::AlgebraBackend;
use crate::hfst_data_types::Symbol;
use crate::hfst_transducer::HfstTransducer;

mod compile;
mod eval;
mod finalization;
mod labels;
mod lexer_support;
mod rules;

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
    /// The multichar symbols a lexc source declared, while one is being
    /// compiled; a regex using any other multichar symbol gets a warning.
    pub(crate) defined_multichar_symbols: Option<BTreeSet<Symbol>>,
}

// ===========================================================================
// XRE compiler: constructors and the public API (ported from XreCompiler.cc,
// walking the nfst-xre AST). The compile driver is in 'compile', the AST
// evaluator in 'eval', and the replace/restriction/substitute builders in
// 'rules'.
// ===========================================================================

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
            defined_multichar_symbols: None,
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
            defined_multichar_symbols: None,
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

    // [spec:hfst:def:xre-utils.hfst.xre.warn-fn]
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
    // [spec:hfst:def:xre-utils.hfst.xre.is-definition-fn]
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
    pub fn add_defined_multichar_symbol(&mut self, symbol: &str) {
        self.defined_multichar_symbols
            .get_or_insert_with(BTreeSet::new)
            .insert(Symbol::new(symbol));
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.remove-defined-multichar-symbols-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.remove-defined-multichar-symbols-fn]
    pub fn remove_defined_multichar_symbols(&mut self) {
        self.defined_multichar_symbols = None;
    }

    // [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.contained-only-comments-fn]
    // [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.contained-only-comments-fn]
    pub fn contained_only_comments(&self) -> bool {
        self.contains_only_comments
    }
}
