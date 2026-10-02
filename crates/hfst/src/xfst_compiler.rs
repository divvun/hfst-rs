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
use tracing::{error, info};

mod ambiguity;
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

/// How a script run ended: it reached its last command, or a command asked
/// to quit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Quit,
}

/// Why a command failed: the message, and any advice to print under it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandError {
    pub message: String,
    pub notes: Vec<String>,
    /// Set when the failure was already printed where it happened, as for an
    /// error inside a sourced file, so the driver does not print it twice.
    reported: bool,
}

impl CommandError {
    pub fn new(message: impl Into<String>) -> Self {
        CommandError {
            message: message.into(),
            notes: Vec::new(),
            reported: false,
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn with_notes(mut self, notes: Vec<String>) -> Self {
        self.notes.extend(notes);
        self
    }

    fn empty_stack() -> Self {
        CommandError::new("empty stack: this command needs a network on the stack")
    }

    fn need_two() -> Self {
        CommandError::new("not enough networks on the stack: this operation needs two")
    }

    // [spec:hfst:req:xfst-cmd.no-placeholders]
    /// The error for a command this compiler has no implementation of: it
    /// fails by name rather than printing a stand-in.
    fn not_supported(command: &str) -> Self {
        CommandError::new(format!("'{command}' is not supported"))
    }
}

impl From<crate::error::Error> for CommandError {
    fn from(e: crate::error::Error) -> Self {
        CommandError::new(e.to_string())
    }
}

impl From<std::io::Error> for CommandError {
    fn from(e: std::io::Error) -> Self {
        CommandError::new(e.to_string())
    }
}

impl core::fmt::Display for CommandError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.message)
    }
}

impl core::error::Error for CommandError {}

/// The result of one command.
pub type CmdResult<T = ()> = Result<T, CommandError>;

/// A script that stopped: the file or label it came from and what went wrong
/// where. Every diagnostic in it has already been printed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptError {
    pub source_name: String,
    pub diagnostics: Vec<XfstDiagnostic>,
}

impl core::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let messages: Vec<&str> = self
            .diagnostics
            .iter()
            .map(|d| d.message.as_str())
            .collect();
        write!(f, "{}: {}", self.source_name, messages.join("; "))
    }
}

impl core::error::Error for ScriptError {}

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
    /// How many 'source' commands deep the running script is.
    source_depth: u32,
}

/// The deepest 'source' nesting allowed, so a script that sources itself
/// fails instead of overflowing the stack.
const MAX_SOURCE_DEPTH: u32 = 64;

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
            restricted_mode: false,
            engine_config: crate::hfst_transducer::EngineConfig::default(),
            nets: Vec::new(),
            source: String::new(),
            source_name: String::from(REPL_SOURCE_NAME),
            current_span: 0..0,
            source_depth: 0,
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
            .insert("encode-weights".to_string(), "OFF".to_string());
        c.variables
            .insert("flag-is-epsilon".to_string(), "OFF".to_string());
        c.variables
            .insert("harmonize-flags".to_string(), "OFF".to_string());
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
            .insert("med-cutoff".to_string(), "15".to_string());
        c.variables.insert("med-limit".to_string(), "3".to_string());
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
            .insert("quit-on-fail".to_string(), "ON".to_string());
        c.variables
            .insert("retokenize".to_string(), "ON".to_string());
        c.variables
            .insert("show-flags".to_string(), "OFF".to_string());
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
    // [spec:hfst:req:xfst-cmd.errors-are-values]
    /// Parse `src` as an XFST script and run its commands in order.
    ///
    /// Every failure is reported to stderr against its span in `src` as it
    /// happens. A failed command stops the run when `quit-on-fail` is `ON`
    /// and the input is not the interactive prompt; otherwise the run goes
    /// on. The returned error has already been reported, so callers use it
    /// for the outcome, not to print it again.
    pub fn parse(&mut self, src: &str) -> Result<Flow, ScriptError> {
        // Retaining the script text is what turns every downstream failure into
        // a located one: line and column come from ariadne rendering a span
        // against this string.
        self.source = src.to_string();
        let script = match nfst_xfst::parse(src) {
            Ok(s) => s,
            Err(e) => {
                let diagnostics = parse_diagnostics(src, &e);
                for d in &diagnostics {
                    crate::diag::emit_with_notes(
                        &self.source_name,
                        src,
                        d.span.clone(),
                        crate::diag::Severity::Error,
                        &d.message,
                        &d.notes,
                    );
                }
                return Err(ScriptError {
                    source_name: self.source_name.clone(),
                    diagnostics,
                });
            }
        };
        for c in &script.value.commands {
            self.current_span = c.span.range.clone();
            match self.eval_command(&c.value) {
                Ok(Flow::Continue) => {}
                Ok(Flow::Quit) => return Ok(Flow::Quit),
                Err(e) => {
                    if !e.reported {
                        self.diag_error_with_notes(&e.message, &e.notes);
                    }
                    if !self.stops_on_failure() {
                        continue;
                    }
                    let failure = XfstDiagnostic {
                        span: self.current_span.clone(),
                        message: e.message,
                        notes: e.notes,
                    };
                    return Err(ScriptError {
                        source_name: self.source_name.clone(),
                        diagnostics: vec![failure],
                    });
                }
            }
        }
        Ok(Flow::Continue)
    }

    // [spec:hfst:req:xfst-cmd.source]
    /// Run the script in `path`, resolved against the working directory, as
    /// part of this session. Its diagnostics name `path` and point into it.
    pub fn source_file(&mut self, path: &str) -> CmdResult<Flow> {
        if self.source_depth >= MAX_SOURCE_DEPTH {
            return Err(CommandError::new(format!(
                "'source' is nested more than {} deep; does a script source itself?",
                MAX_SOURCE_DEPTH
            )));
        }
        self.check_filename(path)?;
        let text = std::fs::read_to_string(path)
            .map_err(|e| CommandError::new(format!("could not read '{}': {}", path, e)))?;

        let saved_source = std::mem::take(&mut self.source);
        let saved_name = std::mem::replace(&mut self.source_name, path.to_string());
        let saved_span = self.current_span.clone();
        self.source_depth += 1;
        let outcome = self.parse(&text);
        self.source_depth -= 1;
        self.source = saved_source;
        self.source_name = saved_name;
        self.current_span = saved_span;

        outcome.map_err(|e| CommandError {
            message: e.to_string(),
            notes: Vec::new(),
            reported: true,
        })
    }

    /// Whether a failed command ends the run: `quit-on-fail` is `ON` and the
    /// input is a script rather than the interactive prompt.
    fn stops_on_failure(&self) -> bool {
        self.variables["quit-on-fail"] == "ON" && !self.read_interactive_text_from_stdin
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
