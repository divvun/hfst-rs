//! A class that encapsulates compilation of Xerox fst language scripts
//! expressions into HFST automata.
//!
//! Xerox fst language is described in Finite state morphology (2004) by
//! Beesley and Karttunen.
//!
//! This is a literal 1:1 port of HFST's hfst::xfst::XfstCompiler. It keeps a
//! STACK of HfstTransducer handles plus definitions/variables/lists/aliases
//! maps; each command method mutates them. Where the original bison actions
//! dispatched to xfst->method(args), we instead walk nfst-xfst's XfstCommand
//! AST (the sanctioned structural deviation) and call the same ported command
//! methods 1:1.
//!
//! The C++ source held raw 'HfstTransducer*' that it freely aliased (the stack,
//! 'names'/'definitions' and 'print_name's pointer-identity check). The port
//! expresses that shared ownership with 'NetId' indices into the compiler's
//! push-only 'nets' arena and pointer identity with 'NetId' equality.
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(unused_assignments)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::collections::{BTreeMap, BTreeSet};

use crate::backend::{AlgebraBackend, Backend};
use crate::hfst_basic_transducer::{HfstBasicTransducer, HfstBasicTransitions};
use crate::hfst_data_types::{HfstOneLevelPaths, HfstTwoLevelPaths, ImplementationType};
use crate::hfst_data_types::{StringPair, StringPairSet, StringVector, Symbol};
use crate::hfst_input_stream::HfstInputStream;
use crate::hfst_output_stream::HfstOutputStream;
use crate::hfst_symbol_defs::StringSet;
use crate::hfst_symbol_defs::internal_identity;
use crate::hfst_transducer::{FromAnyTransducer, HfstTransducer};
use crate::hfst_tropical_transducer_transition_data::SymbolCoder;
use crate::lexc::LexcCompiler;
use crate::virtual_flag_frontends::prepare_compose_flag_overlay;
use crate::xre::XreCompiler;
use std::io::BufRead;
use tracing::{debug, error, info};

mod apply;
mod compile_replace;
mod definitions;
mod diagnostics;
mod dispatch;
mod file_io;
mod inspect;
mod network_ops;
mod print_words;
mod printing;
mod session;
mod stack;
mod substitute;
mod test_commands;
mod variables;
pub use diagnostics::{XfstDiagnostic, parse_diagnostics};
use session::run_shell;
use variables::{initialize_variable_explanations, parse_size};

// Used internally in function 'apply_unary_operator'.
// [spec:hfst:def:xfst-compiler.hfst.xfst.unary-operation]
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UnaryOperation {
    DETERMINIZE_NET,
    EPSILON_REMOVE_NET,
    INVERT_NET,
    LOWER_SIDE_NET,
    UPPER_SIDE_NET,
    OPTIONAL_NET,
    ONE_PLUS_NET,
    ZERO_PLUS_NET,
    REVERSE_NET,
    MINIMIZE_NET,
    PRUNE_NET_,
}

// Used internally in function 'apply_binaryoperator(_iteratively)'.
// [spec:hfst:def:xfst-compiler.hfst.xfst.binary-operation]
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BinaryOperation {
    IGNORE_NET,
    INTERSECT_NET,
    COMPOSE_NET,
    CONCATENATE_NET,
    MINUS_NET,
    UNION_NET,
    SHUFFLE_NET,
    CROSSPRODUCT_NET,
}

// Used internally in function 'apply'.
// [spec:hfst:def:xfst-compiler.hfst.xfst.apply-direction]
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ApplyDirection {
    APPLY_UP_DIRECTION,
    APPLY_DOWN_DIRECTION,
}

// Used internally
// [spec:hfst:def:xfst-compiler.hfst.xfst.level]
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    LOWER_LEVEL,
    UPPER_LEVEL,
    BOTH_LEVELS,
}

// [spec:hfst:def:xfst-compiler.hfst.xfst.test-operation]
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TestOperation {
    TEST_SUBLANGUAGE_,
    TEST_OVERLAP_,
}

// [spec:hfst:def:xfst-compiler.hfst.xfst.string-map]
pub type StringMap = BTreeMap<String, String>;

// A shared, mutable handle to a stack/definition transducer. The C++ xfst
// compiler holds raw 'HfstTransducer*' that it freely aliases (e.g. 'name'
// records the stack top in 'names' while it stays on the stack, and
// 'print_name' matches by pointer identity). A 'NetId' index into the
// compiler's push-only 'nets' arena is the safe expression of that shared
// ownership; pointer identity becomes 'NetId' equality.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NetId(usize);

// @brief Xfst compiler contains all the methods and variables a session of
// XFST script parser needs.
// [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler]
pub struct XfstCompiler<B: AlgebraBackend> {
    /* Whether readline library is used when reading user input. */
    pub use_readline: bool,
    /* Whether the lexc parser must be reset before reading lexc (set true after
    the first lexc read; was a file-static bool). */
    has_lexc_been_read: bool,
    /* Whether interactive text is read from standard input. */
    pub read_interactive_text_from_stdin: bool,
    /* Windows-specific: whether output, error messages and warnings are printed to the console. */
    pub output_to_console: bool,
    /* The regular expression compiler. */
    pub xre: XreCompiler<B>,
    /* The lexc compiler. */
    pub lexc: LexcCompiler<B>,
    pub original_definitions: BTreeMap<Symbol, String>,
    pub definitions: BTreeMap<Symbol, NetId>,
    pub original_function_definitions: BTreeMap<Symbol, String>,
    pub function_definitions: BTreeMap<Symbol, String>,
    pub function_arguments: BTreeMap<Symbol, u32>,
    // std::stack mirror: top = last element; pop = pop_back, push = push_back.
    pub stack: Vec<NetId>,
    pub names: BTreeMap<Symbol, NetId>,
    pub aliases: BTreeMap<Symbol, String>,
    pub variables: BTreeMap<String, String>,
    pub properties: BTreeMap<String, String>,
    pub lists: BTreeMap<Symbol, BTreeSet<Symbol>>,
    /// Typed operational state for the textual `harmonize-flags` variable.
    harmonize_flags: bool,
    pub verbose: bool,
    pub verbose_prompt: bool,
    /* The latest regex that has been compiled when 'compile_regex' has been
    called. The xfst lexer often needs to parse regexps in order to determine
    where they end before giving them to the actual parser. By storing the result
    in this variable, there is no need to parse a regexp again on the parse level. */
    pub latest_regex_compiled: Option<NetId>,
    // Whether the script has encountered the quit command ('quit', 'exit', etc.).
    // Needed in interactive mode, where user input is read line by line.
    pub quit_requested: bool,
    // Whether the compiler has encountered an error when compiling input given to
    // 'parse' function that should quit the compilation and make
    // the function return a non-zero value. Note that if the variable 'quit-on-fail'
    // is false, fail_flag will always be false.
    pub fail_flag: bool,
    pub restricted_mode: bool,
    /* Engine-policy flags set by the 'set' command (was a cluster of file-static
    globals in HfstTransducer.cc). Threaded into the transducer ops this compiler
    invokes. */
    pub engine_config: crate::hfst_transducer::EngineConfig,
    // Push-only arena backing every 'NetId'. Slots are never individually
    // freed (a net dropped from the stack/definitions stays here, leaked until
    // the compiler drops); this preserves the raw-pointer aliasing the C++
    // relied on.
    nets: Vec<HfstTransducer<B>>,
    /// The script text currently being walked, retained so a failure can be
    /// rendered with the offending line and a caret under it.
    source: String,
    /// Label shown in diagnostics: a script file name, or the sentinel for a
    /// line typed at the prompt.
    source_name: String,
    /// Byte span in 'source' of the command currently being evaluated. Every
    /// command-level diagnostic anchors here, so the many sites that only
    /// report a failure gain a source position without each carrying one.
    current_span: std::ops::Range<usize>,
}

/// Source label for xfst input that came from no file — a line typed at the
/// interactive prompt, or a script handed straight to the library.
const REPL_SOURCE_NAME: &str = "<xfst>";

impl<B: AlgebraBackend> XfstCompiler<B> {
    // Resolve a 'NetId' to the transducer it names in the arena.
    pub fn net(&self, id: NetId) -> &HfstTransducer<B> {
        &self.nets[id.0]
    }
    // Resolve a 'NetId' to the transducer it names in the arena, mutably.
    fn net_mut(&mut self, id: NetId) -> &mut HfstTransducer<B> {
        &mut self.nets[id.0]
    }
    // Push a transducer into the arena and return the 'NetId' naming it.
    fn alloc_net(&mut self, t: HfstTransducer<B>) -> NetId {
        let id = NetId(self.nets.len());
        self.nets.push(t);
        id
    }
    // Two disjoint mutable references into the arena. 'a' and 'b' must name
    // distinct slots (as the stack always holds distinct 'NetId's); this
    // replaces the two-cell 'borrow_mut()' pair the 'Rc<RefCell<..>>' model
    // allowed when 'result' and 't' were separate cells.
    fn net_pair_mut(
        &mut self,
        a: NetId,
        b: NetId,
    ) -> (&mut HfstTransducer<B>, &mut HfstTransducer<B>) {
        assert!(a.0 != b.0, "net_pair_mut requires distinct NetIds");
        if a.0 < b.0 {
            let (lo, hi) = self.nets.split_at_mut(b.0);
            (&mut lo[a.0], &mut hi[0])
        } else {
            let (lo, hi) = self.nets.split_at_mut(a.0);
            (&mut hi[0], &mut lo[b.0])
        }
    }
}

impl<B: AlgebraBackend + FromAnyTransducer> XfstCompiler<B> {
    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.xfst-compiler-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.xfst-compiler-fn]
    // @brief Create compiler for transducers of the backend type 'B'
    // (the 'impl' format argument is the type parameter now).
    pub fn new() -> Self {
        let mut c = XfstCompiler {
            use_readline: false,
            has_lexc_been_read: false,
            read_interactive_text_from_stdin: false,
            output_to_console: false,
            xre: XreCompiler::new(),
            lexc: LexcCompiler::new(),
            original_definitions: BTreeMap::new(),
            definitions: BTreeMap::new(),
            original_function_definitions: BTreeMap::new(),
            function_definitions: BTreeMap::new(),
            function_arguments: BTreeMap::new(),
            stack: Vec::new(),
            names: BTreeMap::new(),
            aliases: BTreeMap::new(),
            variables: BTreeMap::new(),
            properties: BTreeMap::new(),
            lists: BTreeMap::new(),
            harmonize_flags: false,
            verbose: false,
            verbose_prompt: false,
            latest_regex_compiled: None,
            quit_requested: false,
            fail_flag: false,
            restricted_mode: false,
            engine_config: crate::hfst_transducer::EngineConfig::default(),
            nets: Vec::new(),
            source: String::new(),
            source_name: String::from(REPL_SOURCE_NAME),
            current_span: 0..0,
        };
        c.xre.set_expand_definitions(true);
        c.xre.set_verbosity(c.verbose);
        c.xre.set_flag_harmonization(false);
        // c.xre.set_error_stream(...);
        c.lexc.set_verbosity(if c.verbose { 2 } else { 0 });
        // c.lexc.set_error_stream(...);
        // XFST defaults Xerox-style composition ON. In C++ this was the
        // 'hfst::xerox_composition' file-static global shared with the XRE
        // compiler; mirror the setting into 'c.xre' so 'read regex' composes
        // the same way.
        c.engine_config.xerox_composition = true;
        c.xre.set_xerox_composition(true);
        c.variables.insert("assert".to_string(), "OFF".to_string());
        c.variables.insert(
            "att-epsilon".to_string(),
            "@0@ | @_EPSILON_SYMBOL_@".to_string(),
        );
        c.variables
            .insert("char-encoding".to_string(), "UTF-8".to_string());
        // Empty by design: this names the owner of the network the USER is
        // compiling, not the owner of the compiler. Upstream defaulted it to
        // "Copyleft (c) University of Helsinki", which stamped Helsinki onto
        // every third-party grammar that never set it.
        c.variables
            .insert("copyright-owner".to_string(), String::new());
        c.variables
            .insert("directory".to_string(), "OFF".to_string());
        c.variables
            .insert("encode-weights".to_string(), "OFF".to_string());
        c.variables
            .insert("flag-is-epsilon".to_string(), "OFF".to_string());
        c.variables
            .insert("harmonize-flags".to_string(), "OFF".to_string());
        c.variables
            .insert("hopcroft-min".to_string(), "ON".to_string());
        c.variables
            .insert("lexc-minimize-flags".to_string(), "OFF".to_string());
        c.variables
            .insert("lexc-rename-flags".to_string(), "OFF".to_string());
        c.variables
            .insert("lexc-with-flags".to_string(), "OFF".to_string());
        c.variables.insert(
            "lookup-cycle-cutoff".to_string(),
            LOOKUP_CYCLE_CUTOFF.to_string(),
        );
        c.variables
            .insert("maximum-weight".to_string(), "OFF".to_string());
        c.variables.insert("minimal".to_string(), "ON".to_string());
        c.variables
            .insert("name-nets".to_string(), "OFF".to_string());
        c.variables
            .insert("obey-flags".to_string(), "ON".to_string());
        c.variables
            .insert("precision".to_string(), WEIGHT_PRECISION.to_string());
        c.variables
            .insert("print-foma-sigma".to_string(), "OFF".to_string());
        c.variables
            .insert("print-pairs".to_string(), "OFF".to_string());
        c.variables
            .insert("print-sigma".to_string(), "OFF".to_string());
        c.variables
            .insert("print-space".to_string(), "OFF".to_string());
        c.variables
            .insert("print-weight".to_string(), "OFF".to_string());
        c.variables.insert(
            "print-words-cycle-cutoff".to_string(),
            PRINT_WORDS_CYCLE_CUTOFF.to_string(),
        );
        c.variables
            .insert("quit-on-fail".to_string(), "OFF".to_string());
        c.variables
            .insert("quote-special".to_string(), "OFF".to_string());
        c.variables
            .insert("random-seed".to_string(), "ON".to_string());
        c.variables
            .insert("recode-cp1252".to_string(), "NEVER".to_string());
        c.variables
            .insert("recursive-define".to_string(), "OFF".to_string());
        c.variables
            .insert("retokenize".to_string(), "ON".to_string());
        c.variables
            .insert("show-flags".to_string(), "OFF".to_string());
        c.variables
            .insert("sort-arcs".to_string(), "MAYBE".to_string());
        c.variables
            .insert("use-timer".to_string(), "OFF".to_string());
        c.variables.insert("verbose".to_string(), "OFF".to_string());
        c.variables
            .insert("xerox-composition".to_string(), "ON".to_string());
        initialize_variable_explanations();
        c.prompt();
        c
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.parse-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.parse-fn]
    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.parse-line-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.parse-line-fn]
    // @brief Parse @a src as an XFST script using nfst-xfst and walk the
    // resulting commands. Replaces the bison-action dispatch.
    pub fn parse(&mut self, src: &str) -> i32 {
        // The bison parser used to be driven by hxfstparse(); here we instead
        // parse the whole script with nfst-xfst and walk the resulting command
        // list, calling the same ported command-handler methods. The CHECK
        // macro that the bison actions appended ('if get_fail_flag() YYABORT')
        // becomes a per-command fail-flag test, and the QUIT action that
        // returned EXIT_SUCCESS becomes the quit_requested test.
        //
        // Retaining the script text is what turns every downstream failure into
        // a located one: line and column come from ariadne rendering a span
        // against this string.
        self.source = src.to_string();
        let script = match nfst_xfst::parse(src) {
            Ok(s) => s,
            Err(e) => {
                for d in parse_diagnostics(src, &e) {
                    crate::diag::emit_with_notes(
                        &self.source_name,
                        src,
                        d.span,
                        crate::diag::Severity::Error,
                        &d.message,
                        &d.notes,
                    );
                }
                return 1;
            }
        };
        for c in &script.value.commands {
            self.current_span = c.span.range.clone();
            if let Err(e) = self.eval_command(&c.value) {
                self.diag_error(&e.to_string());
                return 1;
            }
            // QUIT action returned EXIT_SUCCESS immediately.
            if self.quit_requested {
                return 0;
            }
            // CHECK: if get_fail_flag() { YYABORT; }
            if self.get_fail_flag() {
                return 1;
            }
        }
        0
    }

    /// Name shown in source-anchored diagnostics — the script file the text
    /// about to be parsed came from. Defaults to the interactive sentinel,
    /// which is right for a line typed at the prompt.
    pub fn set_source_name(&mut self, name: &str) -> &mut Self {
        self.source_name = if name.is_empty() {
            String::from(REPL_SOURCE_NAME)
        } else {
            name.to_string()
        };
        self
    }
}

impl<B: AlgebraBackend + FromAnyTransducer> Default for XfstCompiler<B> {
    fn default() -> Self {
        Self::new()
    }
}

const WEIGHT_PRECISION: &str = "5";
const LOOKUP_CYCLE_CUTOFF: &str = "5";
const PRINT_WORDS_CYCLE_CUTOFF: &str = "5";
