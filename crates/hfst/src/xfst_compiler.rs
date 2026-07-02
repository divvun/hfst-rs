//! A class that encapsulates compilation of Xerox fst language scripts
//! expressions into HFST automata.
//!
//! Xerox fst language is described in Finite state morphology (2004) by
//! Beesley and Karttunen.
//!
//! This is a literal 1:1 port of HFST's hfst::xfst::XfstCompiler. It keeps a
//! STACK of HfstTransducer handles plus definitions/variables/lists/aliases
//! maps; each command method mutates them. Where the original bison actions
//! dispatched to xfst_->method(args), we instead walk nfst-xfst's XfstCommand
//! AST (the sanctioned structural deviation) and call the same ported command
//! methods 1:1.
//!
//! The C++ source held raw 'HfstTransducer*' that it freely aliased (the stack,
//! 'names_'/'definitions_' and 'print_name's pointer-identity check). The port
//! expresses that shared ownership with 'NetRef = Rc<RefCell<HfstTransducer>>'
//! and pointer identity with 'Rc::ptr_eq'. The only remaining 'unsafe' wraps C
//! FFI (libc / hfst_fopen / HfstInputStream) and ownership recovery from
//! pointer-returning HFST APIs.
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_mut)]
#![allow(unused_assignments)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use crate::hfst_basic_transducer::{HfstBasicTransducer, HfstBasicTransitions};
use crate::hfst_data_types::{HfstOneLevelPaths, HfstTwoLevelPaths, ImplementationType};
use crate::hfst_data_types::{StringPair, StringPairSet, StringVector};
use crate::hfst_input_stream::HfstInputStream;
use crate::hfst_output_stream::HfstOutputStream;
use crate::hfst_symbol_defs::StringSet;
use crate::hfst_symbol_defs::internal_identity;
use crate::hfst_transducer::HfstTransducer;
use crate::hfst_tropical_transducer_transition_data::SymbolCoder;
use crate::lexc::LexcCompiler;
use crate::xre::XreCompiler;
use std::io::BufRead;
use tracing::{debug, error, info, warn};

// [spec:hfst:def:xfst-compiler.apply-end-string]
static APPLY_END_STRING: &str = "<ctrl-d>";

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
// records the stack top in 'names_' while it stays on the stack, and
// 'print_name' matches by pointer identity). 'Rc<RefCell<..>>' is the safe
// expression of that shared ownership; pointer identity becomes 'Rc::ptr_eq'.
pub type NetRef = Rc<RefCell<HfstTransducer>>;

// @brief Xfst compiler contains all the methods and variables a session of
// XFST script parser needs.
// [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler]
pub struct XfstCompiler {
    /* Whether readline library is used when reading user input. */
    pub use_readline_: bool,
    /* Whether the lexc parser must be reset before reading lexc (set true after
    the first lexc read; was a file-static bool). */
    has_lexc_been_read_: bool,
    /* Whether interactive text is read from standard input. */
    pub read_interactive_text_from_stdin_: bool,
    /* Windows-specific: whether output, error messages and warnings are printed to the console. */
    pub output_to_console_: bool,
    /* The regular expression compiler. */
    pub xre_: XreCompiler,
    /* The lexc compiler. */
    pub lexc_: LexcCompiler,
    pub original_definitions_: BTreeMap<String, String>,
    pub definitions_: BTreeMap<String, NetRef>,
    pub original_function_definitions_: BTreeMap<String, String>,
    pub function_definitions_: BTreeMap<String, String>,
    pub function_arguments_: BTreeMap<String, u32>,
    // std::stack mirror: top = last element; pop = pop_back, push = push_back.
    pub stack_: Vec<NetRef>,
    pub names_: BTreeMap<String, NetRef>,
    pub aliases_: BTreeMap<String, String>,
    pub variables_: BTreeMap<String, String>,
    pub properties_: BTreeMap<String, String>,
    pub lists_: BTreeMap<String, BTreeSet<String>>,
    pub format_: ImplementationType,
    pub verbose_: bool,
    pub verbose_prompt_: bool,
    /* The latest regex that has been compiled when 'compile_regex' has been
    called. The xfst lexer often needs to parse regexps in order to determine
    where they end before giving them to the actual parser. By storing the result
    in this variable, there is no need to parse a regexp again on the parse level. */
    pub latest_regex_compiled: Option<NetRef>,
    // Whether the script has encountered the quit command ('quit', 'exit', etc.).
    // Needed in interactive mode, where user input is read line by line.
    pub quit_requested_: bool,
    // Whether the compiler has encountered an error when compiling input given to
    // 'parse' or 'parse_line' function that should quit the compilation and make
    // the function return a non-zero value. Note that if the variable 'quit-on-fail'
    // is false, fail_flag_ will always be false.
    pub fail_flag_: bool,
    pub restricted_mode_: bool,
    /* Engine-policy flags set by the 'set' command (was a cluster of file-static
    globals in HfstTransducer.cc). Threaded into the transducer ops this compiler
    invokes. */
    pub engine_config_: crate::hfst_transducer::EngineConfig,
}

impl XfstCompiler {
    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.xfst-compiler-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.xfst-compiler-fn]
    // @brief Construct compiler for unknown format transducers.
    pub fn new() -> Self {
        Self::new_with_impl(ImplementationType::TROPICAL_OPENFST_TYPE)
    }

    // @brief Create compiler for @a impl format transducers
    pub fn new_with_impl(impl_: ImplementationType) -> Self {
        let mut c = XfstCompiler {
            use_readline_: false,
            has_lexc_been_read_: false,
            read_interactive_text_from_stdin_: false,
            output_to_console_: false,
            xre_: XreCompiler::new(impl_),
            lexc_: LexcCompiler::new(impl_),
            original_definitions_: BTreeMap::new(),
            definitions_: BTreeMap::new(),
            original_function_definitions_: BTreeMap::new(),
            function_definitions_: BTreeMap::new(),
            function_arguments_: BTreeMap::new(),
            stack_: Vec::new(),
            names_: BTreeMap::new(),
            aliases_: BTreeMap::new(),
            variables_: BTreeMap::new(),
            properties_: BTreeMap::new(),
            lists_: BTreeMap::new(),
            format_: impl_,
            verbose_: false,
            verbose_prompt_: false,
            latest_regex_compiled: None,
            quit_requested_: false,
            fail_flag_: false,
            restricted_mode_: false,
            engine_config_: crate::hfst_transducer::EngineConfig::default(),
        };
        c.xre_.set_expand_definitions(true);
        c.xre_.set_verbosity(c.verbose_);
        c.xre_.set_flag_harmonization(false);
        // c.xre_.set_error_stream(...);
        c.lexc_.set_verbosity(if c.verbose_ { 2 } else { 0 });
        // c.lexc_.set_error_stream(...);
        // XFST defaults Xerox-style composition ON.
        c.engine_config_.xerox_composition = true;
        c.variables_.insert("assert".to_string(), "OFF".to_string());
        c.variables_.insert(
            "att-epsilon".to_string(),
            "@0@ | @_EPSILON_SYMBOL_@".to_string(),
        );
        c.variables_
            .insert("char-encoding".to_string(), "UTF-8".to_string());
        c.variables_.insert(
            "copyright-owner".to_string(),
            "Copyleft (c) University of Helsinki".to_string(),
        );
        c.variables_
            .insert("directory".to_string(), "OFF".to_string());
        c.variables_
            .insert("encode-weights".to_string(), "OFF".to_string());
        c.variables_
            .insert("flag-is-epsilon".to_string(), "OFF".to_string());
        c.variables_
            .insert("harmonize-flags".to_string(), "OFF".to_string());
        c.variables_
            .insert("hopcroft-min".to_string(), "ON".to_string());
        c.variables_
            .insert("lexc-minimize-flags".to_string(), "OFF".to_string());
        c.variables_
            .insert("lexc-rename-flags".to_string(), "OFF".to_string());
        c.variables_
            .insert("lexc-with-flags".to_string(), "OFF".to_string());
        c.variables_.insert(
            "lookup-cycle-cutoff".to_string(),
            LOOKUP_CYCLE_CUTOFF.to_string(),
        );
        c.variables_
            .insert("maximum-weight".to_string(), "OFF".to_string());
        c.variables_.insert("minimal".to_string(), "ON".to_string());
        c.variables_
            .insert("name-nets".to_string(), "OFF".to_string());
        c.variables_
            .insert("obey-flags".to_string(), "ON".to_string());
        c.variables_
            .insert("precision".to_string(), WEIGHT_PRECISION.to_string());
        c.variables_
            .insert("print-foma-sigma".to_string(), "OFF".to_string());
        c.variables_
            .insert("print-pairs".to_string(), "OFF".to_string());
        c.variables_
            .insert("print-sigma".to_string(), "OFF".to_string());
        c.variables_
            .insert("print-space".to_string(), "OFF".to_string());
        c.variables_
            .insert("print-weight".to_string(), "OFF".to_string());
        c.variables_.insert(
            "print-words-cycle-cutoff".to_string(),
            PRINT_WORDS_CYCLE_CUTOFF.to_string(),
        );
        c.variables_
            .insert("quit-on-fail".to_string(), "OFF".to_string());
        c.variables_
            .insert("quote-special".to_string(), "OFF".to_string());
        c.variables_
            .insert("random-seed".to_string(), "ON".to_string());
        c.variables_
            .insert("recode-cp1252".to_string(), "NEVER".to_string());
        c.variables_
            .insert("recursive-define".to_string(), "OFF".to_string());
        c.variables_
            .insert("retokenize".to_string(), "ON".to_string());
        c.variables_
            .insert("show-flags".to_string(), "OFF".to_string());
        c.variables_
            .insert("sort-arcs".to_string(), "MAYBE".to_string());
        c.variables_
            .insert("use-timer".to_string(), "OFF".to_string());
        c.variables_
            .insert("verbose".to_string(), "OFF".to_string());
        c.variables_
            .insert("xerox-composition".to_string(), "ON".to_string());
        initialize_variable_explanations();
        c.prompt();
        c
    }

    // @brief Print parts of automaton with epsilon loops
    // @todo unimplemented yet
    pub fn collect_epsilon_loops(&mut self) -> &mut Self {
        warn!("cannot collect epsilon loops");
        // PROMPT_AND_RETURN_THIS
        self.prompt();
        return self;
    }

    // @brief Print arc count for @a level
    pub fn print_arc_count_level(
        &mut self,
        level: &str,
        oss: &mut dyn std::io::Write,
    ) -> &mut Self {
        let _ = write!(oss, "missing {} arc count\n", level);
        self.flush();
        // PROMPT_AND_RETURN_THIS
        self.prompt();
        return self;
    }

    // @brief Print arc count
    pub fn print_arc_count(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let _ = write!(oss, "missing arc count\n");
        self.flush();
        // PROMPT_AND_RETURN_THIS
        self.prompt();
        return self;
    }

    // @brief Print file info
    pub fn print_file_info(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        warn!("file info not implemented (cf. summarize)");
        // PROMPT_AND_RETURN_THIS
        self.prompt();
        return self;
    }

    // @brief Print flag diacritics
    pub fn print_flags(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let _ = write!(oss, "missing print flags\n");
        self.flush();
        // PROMPT_AND_RETURN_THIS
        self.prompt();
        return self;
    }

    // @brief Print label mappings
    pub fn print_labelmaps(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let _ = write!(oss, "missing label-maps\n");
        self.flush();
        // PROMPT_AND_RETURN_THIS
        self.prompt();
        return self;
    }

    // @brief Print properties of top network
    pub fn print_properties(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let _ = write!(oss, "missing print properties\n");
        self.flush();
        return self;
    }

    // @brief Print properties of network named @a name
    pub fn print_properties_name(&mut self, name: &str, oss: &mut dyn std::io::Write) -> &mut Self {
        warn!("missing print properties");
        // PROMPT_AND_RETURN_THIS
        self.prompt();
        return self;
    }

    // @brief Print nnumber of symbols in network
    pub fn print_sigma_count(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let _ = write!(oss, "missing print sigma count\n");
        self.flush();
        // PROMPT_AND_RETURN_THIS
        self.prompt();
        return self;
    }

    // @brief Print number of paths with all symbols on @a level
    pub fn print_sigma_word_count_level(
        &mut self,
        level: &str,
        oss: &mut dyn std::io::Write,
    ) -> &mut Self {
        let _ = write!(oss, "missing {} sigma word count\n", level);
        self.flush();
        // PROMPT_AND_RETURN_THIS
        self.prompt();
        return self;
    }

    // @brief Print number of paths with all symbols
    pub fn print_sigma_word_count(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let _ = write!(oss, "missing sigma word count\n");
        self.flush();
        // PROMPT_AND_RETURN_THIS
        self.prompt();
        return self;
    }

    // @brief Print size of network named @a name
    pub fn print_size_name(&mut self, name: &str, oss: &mut dyn std::io::Write) -> &mut Self {
        let _ = write!(oss, "{:>10}", name);
        let _ = write!(oss, ": ? bytes. ? states, ? arcs, ? paths.\n");
        self.flush();
        // PROMPT_AND_RETURN_THIS
        self.prompt();
        return self;
    }

    // @brief Print size of top network
    pub fn print_size(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let _ = write!(oss, "? bytes. ? states, ? arcs, ? paths.\n");
        self.flush();
        // PROMPT_AND_RETURN_THIS
        self.prompt();
        return self;
    }

    // @brief Read lexicons from @a indata
    pub fn read_lexc(&mut self, indata: &str) -> &mut Self {
        // The C++ declares read_lexc(const char* indata) but provides no
        // definition (it is never invoked; the parser uses read_lexc_from_file).
        // Mirror that by doing nothing and returning self.
        return self;
    }

    // @brief Sort top network of the stack
    // @todo HFST automata sort or not by default
    pub fn sort_net(&mut self) -> &mut Self {
        warn!("missing sort net");
        // PRINT_INFO_PROMPT_AND_RETURN_THIS
        self.print_transducer_info();
        self.prompt();
        return self;
    }

    // @brief Substring top network of stack
    // @todo unimplementedd
    pub fn substring_net(&mut self) -> &mut Self {
        warn!("missing substring net");
        // PRINT_INFO_PROMPT_AND_RETURN_THIS
        self.print_transducer_info();
        self.prompt();
        return self;
    }

    /* Compile a regex string starting from \a indata, store the resulting
    transducer to variable XfstCompiler::latest_regex_compiled and
    store the number of characters read from \a indata to \a chars_read.

    This function is used by the xfst lexer to determine where a regex
    starting from \a indata ends. */

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.parse-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.parse-fn]
    // @brief Parse @a src as an XFST script using nfst-xfst and walk the
    // resulting commands. Replaces the bison-action dispatch.
    pub fn parse(&mut self, src: &str) -> i32 {
        // The bison parser used to be driven by hxfstparse(); here we instead
        // parse the whole script with nfst-xfst and walk the resulting command
        // list, calling the same ported command-handler methods. The CHECK
        // macro that the bison actions appended ('if get_fail_flag() YYABORT')
        // becomes a per-command fail-flag test, and the QUIT action that
        // returned EXIT_SUCCESS becomes the quit_requested_ test.
        let script = match nfst_xfst::parse(src) {
            Ok(s) => s,
            Err(e) => {
                for d in &e.diagnostics {
                    error!("{}", d.message);
                }
                return 1;
            }
        };
        for c in &script.value.commands {
            if let Err(e) = self.eval_command(&c.value) {
                error!("{}", e);
                return 1;
            }
            // QUIT action returned EXIT_SUCCESS immediately.
            if self.quit_requested_ {
                return 0;
            }
            // CHECK: if get_fail_flag() { YYABORT; }
            if self.get_fail_flag() {
                return 1;
            }
        }
        0
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.parse-line-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.parse-line-fn]
    // @brief Parse @a line
    pub fn parse_line(&mut self, line: String) -> i32 {
        // The C++ drove the bison line parser (hxfst_scan_string + hxfstparse);
        // here we route the line through the nfst-xfst-backed parse() instead.
        let rv = self.parse(&line);
        return rv;
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.set-error-stream-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.set-error-stream-fn]
    /* Set the stream where error messages and warnings are printed. */
    /* Get the stream where error messages and warnings are printed. */
    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.set-output-stream-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.set-output-stream-fn]
    /* Set the stream where output is printed. */
    /* Get the stream where output is printed. */
    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.xfst-fclose-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.xfst-fclose-fn]
    /* A wrapper around file close function. */
    pub fn xfst_fclose(&mut self, name: &str) -> i32 {
        // The redesigned signature carries no FILE handle (file I/O is done via
        // std::fs / HfstInputStream elsewhere), so there is nothing to close;
        // mirror the success path of the C++ wrapper.
        let retval: i32 = 0;
        if retval != 0 {
            error!("could not close file {}", name);
            self.flush();
            self.xfst_fail();
        }
        return retval;
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.xfst-fopen-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.xfst-fopen-fn]
    /* A wrapper around file open function. */
    pub fn xfst_fopen(&mut self, path: &str, mode: &str) {
        match crate::hfst_data_types::hfst_fopen(path, mode) {
            Err(_) => {
                error!("could not open file {}", path);
                self.flush();
                self.xfst_fail();
            }
            Ok(f) => {
                // The redesigned signature returns no handle, so the freshly
                // opened file is closed again here (dropped).
                drop(f);
            }
        }
    }

    /* Get the output stream. */
    /* Get the error stream. */
    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.flush-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.flush-fn]
    /* Flush the stream. */

    // ------------------------------------------------------------------
    // protected
    // ------------------------------------------------------------------

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-print-symbol-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-print-symbol-fn]
    // @brief Get the print symbol for \a symbol.
    // @see print_flags
    fn get_print_symbol(&mut self, symbol: &str) -> String {
        if self.variables_["show-flags"] == "OFF" && // show no flags
            crate::hfst_flag_diacritics::FdOperation::is_diacritic(symbol)
        // symbol is flag
        {
            return String::new(); // print nothing
        }
        if crate::hfst_symbol_defs::internal_epsilon == symbol {
            return String::new();
        }
        if crate::hfst_symbol_defs::internal_unknown == symbol
            || crate::hfst_symbol_defs::internal_identity == symbol
        {
            return "?".to_string();
        }
        return symbol.to_string();
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.convert-to-common-format-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.convert-to-common-format-fn]
    // @brief Convert format of \a t read from file \a filename to common
    // format used by this xfst compiler and print a warning message
    // about loss of information during conversion, if needed.
    fn convert_to_common_format(&mut self, t: &NetRef, filename: Option<&str>) {
        // CHECK_FILENAME equivalent: if (!check_filename(filename)) return;
        if !self.check_filename(filename.unwrap_or("")) {
            return;
        }

        let t_type = t.borrow().get_type();
        if t_type != self.format_ {
            if t_type == ImplementationType::HFST_OL_TYPE
                || t_type == ImplementationType::HFST_OLW_TYPE
            {
                if self.verbose_ {
                    warn!(
                        "transducer is in optimized lookup format, 'apply up' is the only operation it supports"
                    );
                }
                return;
            }

            if self.verbose_ {
                let mut line = format!(
                    "converting transducer type from {} to {}",
                    crate::hfst_data_types::implementation_type_to_format(t_type),
                    crate::hfst_data_types::implementation_type_to_format(self.format_)
                );
                if filename.is_some() {
                    line.push_str(&format!(
                        " when reading from file '{}'",
                        to_filename(filename)
                    ));
                }
                if !HfstTransducer::is_safe_conversion(t_type, self.format_) {
                    line.push_str(" (loss of information is possible)");
                }
                warn!("{}", line);
            }
            t.borrow_mut().convert(self.format_, String::new());
        }
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.open-hfst-input-stream-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.open-hfst-input-stream-fn]
    // @brief Open HfstInputStream to file \a filename.
    // Print an error message and return NULL, if not succesful.
    fn open_hfst_input_stream(&mut self, filename: &str) -> *mut HfstInputStream<'_> {
        // assert(infilename != NULL): filename is always a valid &str here.
        if !self.check_filename(filename) {
            return std::ptr::null_mut();
        }

        match crate::hfst_data_types::hfst_fopen(filename, "r") {
            Err(_) => {
                error!("Could not open file {}", filename);
                self.flush();
                self.xfst_fail();
                return std::ptr::null_mut();
            }
            Ok(infile) => {
                // close the probe handle (the real read goes through
                // HfstInputStream below); dropping cannot fail.
                drop(infile);
            }
        }

        // try { new HfstInputStream(infilename) } catch (NotTransducerStreamException)
        let fname = filename.to_string();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            HfstInputStream::new_filename(&fname)
        }));
        match result {
            Ok(Ok(instream)) => Box::into_raw(Box::new(instream)),
            Ok(Err(_)) | Err(_) => {
                error!(
                    "Unable to read transducers from {}",
                    to_filename(Some(filename))
                );
                self.flush();
                self.xfst_fail();
                std::ptr::null_mut()
            }
        }
    }

    // @brief Read transducers from file \a infilename and either push
    // them to the stack (if \a definitions is false) or add them as definitions
    // (if \a definitions is true).
    fn load_stack_or_definitions(&mut self, infilename: &str, definitions: bool) -> &mut Self {
        // CHECK_FILENAME(infilename)
        if !self.check_filename(infilename) {
            return self;
        }
        // Try to open the stream to file infilename
        let instream = self.open_hfst_input_stream(infilename);
        // IF_NULL_PROMPT_AND_RETURN_THIS(instream)
        if instream.is_null() {
            if self.variables_["quit-on-fail"] == "ON" {
                self.fail_flag_ = true;
            }
            self.prompt();
            return self;
        }

        // Read transducers from stream. Constructing a HfstTransducer from a
        // HfstInputStream is not yet available in the ported facade (the binary
        // HFST stream reader is deferred), so this read loop reports the
        // limitation instead of looping. The per-transducer handling
        // (convert_to_common_format, add_loaded_definition / stack push) is
        // preserved in add_loaded_definition and is reachable once the stream
        // reader lands.
        let _ = definitions;
        if unsafe { (*instream).is_good() } {
            warn!("loading transducers from a binary HFST file is not yet supported");
        }

        unsafe {
            (*instream).close();
            // std::unique_ptr<HfstInputStream> went out of scope here
            drop(Box::from_raw(instream));
        }
        // PROMPT_AND_RETURN_THIS
        self.prompt();
        self
    }

    // @brief Add a transducer definition with name given by 't.get_name()'
    // and value \a t.
    fn add_loaded_definition(&mut self, t: NetRef) -> &mut Self {
        let def_name = t.borrow().get_name();
        if def_name.is_empty() {
            warn!("loaded transducer definition has no name, skipping it");
            return self;
        }
        if self.definitions_.contains_key(&def_name) {
            warn!(
                "a definition named '{}' already exists, overwriting it",
                def_name
            );
            // overwriting drops the previous Rc.
            self.definitions_.remove(&def_name);
        }
        self.definitions_.insert(def_name, t);
        return self;
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.xfst-fail-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.xfst-fail-fn]
    // @brief Set fail flag to true if quit-on-fail is ON,
    // else do nothing.
    fn xfst_fail(&mut self) {
        if self.variables_["quit-on-fail"] == "ON" {
            self.fail_flag_ = true;
        }
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.xfst-lesser-fail-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.xfst-lesser-fail-fn]
    // @brief Set fail flag to true if quit-on-fail is ON and hfst-xfst
    // is not used in interactive mode, else do nothing.
    fn xfst_lesser_fail(&mut self) {
        if self.variables_["quit-on-fail"] == "ON" && !self.read_interactive_text_from_stdin_ {
            self.fail_flag_ = true;
        }
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.print-level-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.print-level-fn]
    fn print_level(&mut self, whole_path: &Vec<u32>, shortest_path: &Vec<u32>) {
        print!("Level {}", whole_path.len() as i32);
        if shortest_path.len() < whole_path.len() {
            print!(" (= {})", shortest_path.len() as i32);
        }
        self.flush();
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.can-level-be-reached-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.can-level-be-reached-fn]
    fn can_level_be_reached(&mut self, level: i32, whole_path_length: usize) -> bool {
        // EOF is -1
        if level == -1 || level == 0 {
            println!("could not read level number (type '0' if you wish to exit program)");
            self.flush();
            return false;
        } else if level < 0 || level > whole_path_length as i32 {
            println!(
                "no such level: '{}' (current level is {})",
                level, whole_path_length as i32
            );
            self.flush();
            return false;
        }
        return true;
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.can-arc-be-followed-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.can-arc-be-followed-fn]
    fn can_transition_be_followed(&mut self, number: i32, number_of_transitions: u32) -> bool {
        // EOF is -1
        if number == -1 || number == 0 {
            println!("could not read arc number");
            self.flush();
            return false;
        } else if number < 1 || number > number_of_transitions as i32 {
            if number_of_transitions < 1 {
                println!("state has no arcs");
            } else {
                println!("arc number must be between 1 and {}", number_of_transitions);
            }
            self.flush();
            return false;
        }
        return true;
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.print-arcs-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.print-arcs-fn]
    fn print_transitions(
        &mut self,
        transitions: &HfstBasicTransitions,
        coder: &SymbolCoder,
    ) -> u32 {
        let mut first_loop = true;
        let mut arc_number: u32 = 1;
        for transition in transitions.iter() {
            if first_loop {
                print!("Arcs:");
                first_loop = false;
            } else {
                print!(", ");
            }
            self.flush();
            let isymbol = transition.get_input_symbol(coder);
            let osymbol = transition.get_output_symbol(coder);

            if isymbol == osymbol {
                print!(" {}. {}", arc_number, isymbol);
            } else {
                print!(" {}. {}:{}", arc_number, isymbol, osymbol);
            }
            self.flush();
            arc_number += 1;
        }
        println!();
        self.flush();
        return arc_number - 1;
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.top-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.top-fn]
    // @brief The topmost transducer in the stack.
    // If empty, print a warning message and return NULL.
    fn top(&mut self) -> Option<NetRef> {
        if self.stack_.is_empty() {
            // EMPTY_STACK
            warn!("Empty stack.");
            self.xfst_lesser_fail();
            self.prompt();
            return None;
        }
        let retval = self.stack_.last().unwrap().clone();
        {
            let t = retval.borrow();
            if t.get_type() == ImplementationType::HFST_OL_TYPE
                || t.get_type() == ImplementationType::HFST_OLW_TYPE
            {
                warn!(
                    "Operation not supported for optimized lookup format. Consider 'remove-optimization' to convert into ordinary format."
                );
                self.prompt();
                return None;
            }
        }
        Some(retval)
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.xfst-getline-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.xfst-getline-fn]
    // @brief Get next line from \a file. Return NULL if end of file is reached.
    // Use \a promptstr as prompt for readline, or print it to stderr if readline is not in use.
    fn xfst_getline(&mut self, promptstr: &str) -> Option<String> {
        // The HAVE_READLINE and WINDOWS branches are not ported; mirror the
        // generic getline path: print the prompt, then read a line from stdin.
        print!("{}", promptstr);
        self.flush();

        let mut line = String::new();
        // getline keeps the trailing newline; read == -1 (EOF) returns NULL.
        match std::io::stdin().lock().read_line(&mut line) {
            Ok(0) => None,
            Ok(_) => Some(line),
            Err(_) => None,
        }
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.remove-newline-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.remove-newline-fn]
    // @brief Remove newline ('\n' and '\r') from the end of \a str.
    fn remove_newline(&mut self, str_: String) -> String {
        // The C++ replaces every '\n'/'\r' with '\0' in place; read back as a
        // C-string the result is everything up to the first newline/return.
        match str_.find(|c| c == '\n' || c == '\r') {
            Some(idx) => str_[..idx].to_string(),
            None => str_,
        }
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.current-history-index-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.current-history-index-fn]
    // @brief Get current readline history index.
    fn current_history_index(&mut self) -> i32 {
        // HAVE_READLINE is not in use; mirror the #else branch.
        return -1;
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.ignore-history-after-index-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.ignore-history-after-index-fn]
    // @brief Remove all readline history after \a index.
    fn ignore_history_after_index(&mut self, index: i32) {
        // HAVE_READLINE is not in use; the whole body is conditional on it, so
        // there is nothing to do.
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-precision-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-precision-fn]
    // @brief Get the precision that is used when printing weights.
    fn get_precision(&mut self) -> i32 {
        // std::istringstream iss(variables_["precision"]); iss >> retval;
        let s = self.variables_["precision"].clone();
        Self::atoi(&s)
    }

    // ------------------------------------------------------------------
    // private
    // ------------------------------------------------------------------

    fn error_message(&self, message: &str) -> &Self {
        error!("{}", message);
        return self;
    }

    fn print_transducer_info(&mut self) -> &mut Self {
        if self.verbose_ && !self.stack_.is_empty() {
            let top = self.stack_.last().unwrap().clone();
            {
                let t = top.borrow();
                if t.get_type() != self.format_ {
                    return self;
                }
                println!(
                    "? bytes. {} states, {} arcs, ? paths",
                    t.number_of_states(),
                    t.number_of_arcs()
                );
            }
            let print_sigma_on =
                self.variables_.get("print-sigma").map(|s| s.as_str()) == Some("ON");
            if print_sigma_on {
                let mut out = std::io::stdout();
                let _ = self.print_sigma(&mut out, false);
            }
        }
        self
    }

    fn add_prop_line(&mut self, line: &str) -> &mut Self {
        // Split the line into name (up to the first ':') and value.
        let bytes = line.as_bytes();
        let mut p = 0usize;
        while p < bytes.len() && bytes[p] != b':' {
            p += 1;
        }
        let name = line[..p].to_string();
        if p >= bytes.len() {
            // *p == '\0': no colon in line (assert(*p != '\0') is a no-op in release)
            error!("no colon in line");
        }
        // skip the colon, then skip leading whitespace of the value
        let mut q = if p < bytes.len() { p + 1 } else { p };
        while q < bytes.len()
            && (bytes[q] == b' '
                || bytes[q] == b'\t'
                || bytes[q] == b'\n'
                || bytes[q] == 0x0b
                || bytes[q] == 0x0c
                || bytes[q] == b'\r')
        {
            q += 1;
        }
        let value = line[q..].to_string();
        self.properties_.insert(name, value);
        return self;
    }

    fn print_bool(&mut self, value: bool) -> &mut Self {
        let printval = if value { 1 } else { 0 };
        print!("{}, (1 = TRUE, 0 = FALSE)\n", printval);
        self.flush();
        return self;
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-stream-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-stream-fn]
    /* A wrapper around stream objects, see flush() for more information. */
    fn get_stream(&mut self) {
        // On Unix (non-WINDOWS) get_stream is the identity on the stream passed
        // in; the print_* methods here write directly to their own 'oss' writer,
        // so there is nothing to redirect. The WINDOWS console-buffering branch
        // is not ported.
    }

    // ------------------------------------------------------------------
    // nfst-xfst command-dispatch driver (replaces the bison action dispatch)
    // ------------------------------------------------------------------

    // @brief Dispatch a single XfstCommand parsed by nfst-xfst, calling the
    // corresponding ported command-handler method 1:1. Each arm mirrors the
    // bison action of the matching grammar production in xfst-parser.yy.
    fn eval_command(&mut self, cmd: &nfst_xfst::XfstCommand) -> crate::error::Result<()> {
        use nfst_xfst::{ApplyKind, NetworkOp, ReadCmd, RedirectKind, SubstituteCmd, XfstCommand};
        match cmd {
            // ── regex / define ──────────────────────────────
            XfstCommand::Regex(xre) => {
                // compile_regex stored the freshly compiled regex into
                // latest_regex_compiled, then read_regex pushed a copy of it.
                if self.latest_regex_compiled.is_some() {
                    self.latest_regex_compiled = None;
                }
                let compiled = self.compile_spanned_xre(xre)?;
                self.latest_regex_compiled = Some(compiled);
                self.read_regex("")?;
            }
            XfstCommand::Define { name, body } => {
                let tr = self.compile_spanned_xre(body)?;
                self.define_transducer(name, tr);
                self.prompt();
            }
            XfstCommand::DefineFunction { name, params, body } => {
                let prototype = format!("{}({})", name, params.join(", "));
                let xre = nfst_xre::pretty_print(body);
                self.define_function(&prototype, &xre);
            }
            XfstCommand::DefineAlias { name, body } => {
                self.define_alias(name, body);
            }
            XfstCommand::DefineList { name, members } => {
                // 'list NAME a-b' becomes a range; 'list NAME s1 s2 ...' a list.
                if members.len() == 1 && members[0].contains('-') {
                    let m = &members[0];
                    let idx = m.find('-').unwrap();
                    let start = &m[..idx];
                    let end = &m[idx + 1..];
                    self.define_list_by_range(name, start, end);
                } else {
                    self.define_list(name, &members.join(" "));
                }
            }
            XfstCommand::Undefine(names) => {
                self.undefine(&names.join(" "));
            }
            XfstCommand::Unlist(name) => {
                self.unlist(name);
            }

            // ── stack ───────────────────────────────────────
            XfstCommand::Clear => {
                self.clear();
            }
            XfstCommand::Pop => {
                self.pop();
            }
            XfstCommand::Push(name) => {
                if name.is_empty() {
                    self.push_latest()?;
                } else {
                    self.push(name)?;
                }
            }
            XfstCommand::Turn => {
                self.turn();
            }
            XfstCommand::Rotate => {
                self.rotate();
            }
            XfstCommand::LoadStack(name) => {
                self.load_stack(name);
            }
            XfstCommand::LoadDefinitions(name) => {
                self.load_definitions(name);
            }

            // ── network ops ─────────────────────────────────
            XfstCommand::Network(op) => match op {
                NetworkOp::Compose => {
                    self.compose_net();
                }
                NetworkOp::Concatenate => {
                    self.concatenate_net();
                }
                NetworkOp::Intersect => {
                    self.intersect_net();
                }
                NetworkOp::Union => {
                    self.union_net();
                }
                NetworkOp::Minus => {
                    self.minus_net();
                }
                NetworkOp::Crossproduct => {
                    self.crossproduct_net();
                }
                NetworkOp::Ignore => {
                    self.ignore_net();
                }
                NetworkOp::Invert => {
                    self.invert_net();
                }
                NetworkOp::Reverse => {
                    self.reverse_net();
                }
                NetworkOp::Determinize => {
                    self.determinize_net();
                }
                NetworkOp::Minimize => {
                    self.minimize_net();
                }
                NetworkOp::EpsilonRemove => {
                    self.epsilon_remove_net();
                }
                NetworkOp::PruneNet => {
                    self.prune_net();
                }
                NetworkOp::Negate => {
                    self.negate_net();
                }
                NetworkOp::OnePlus => {
                    self.one_plus_net();
                }
                NetworkOp::ZeroPlus => {
                    self.zero_plus_net();
                }
                NetworkOp::Sort => {
                    self.sort_net();
                }
                NetworkOp::Shuffle => {
                    self.shuffle_net();
                }
                NetworkOp::Substring => {
                    self.substring_net();
                }
                NetworkOp::Cleanup => {
                    self.cleanup_net();
                }
                NetworkOp::Complete => {
                    self.complete_net();
                }
                NetworkOp::LowerSide => {
                    self.lower_side_net();
                }
                NetworkOp::UpperSide => {
                    self.upper_side_net();
                }
                NetworkOp::Sigma => {
                    self.sigma_net()?;
                }
                NetworkOp::LabelNet => {
                    self.label_net()?;
                }
                NetworkOp::Inspect => {
                    self.inspect_net()?;
                }
                NetworkOp::TwosidedFlags => {
                    self.twosided_flags();
                }
                NetworkOp::EliminateAll => {
                    self.eliminate_flags();
                }
                NetworkOp::CollectEpsilonLoops => {
                    self.collect_epsilon_loops();
                }
                NetworkOp::CompactSigma => {
                    self.compact_sigma();
                }
                NetworkOp::View => {
                    self.view_net();
                }
                NetworkOp::ExtractAmbiguous
                | NetworkOp::ExtractUnambiguous
                | NetworkOp::Ambiguous => {
                    // hxfsterror("unimplemetend ambiguous\n"); return EXIT_FAILURE;
                    error!("unimplemetend ambiguous");
                    self.fail_flag_ = true;
                }
                NetworkOp::CompileReplaceLower => {
                    self.compile_replace_lower_net()?;
                }
                NetworkOp::CompileReplaceUpper => {
                    self.compile_replace_upper_net()?;
                }
                NetworkOp::EliminateFlag(name) => {
                    self.eliminate_flag(name);
                }
                NetworkOp::Name(name) => {
                    self.name_net(name);
                }
            },

            // ── apply / lookup ──────────────────────────────
            XfstCommand::Apply(kind, input) => {
                let s: String = match input {
                    Some(s) => s.clone(),
                    None => {
                        // The bison grammar read from stdin here (apply_up(stdin)).
                        use std::io::Read;
                        let mut buf = String::new();
                        let _ = std::io::stdin().read_to_string(&mut buf);
                        buf
                    }
                };
                match kind {
                    ApplyKind::Up => {
                        self.apply_up(&s)?;
                    }
                    ApplyKind::Down => {
                        self.apply_down(&s)?;
                    }
                    ApplyKind::Med => {
                        self.apply_med(&s);
                    }
                }
            }
            XfstCommand::LookupOptimize => {
                self.lookup_optimize();
            }
            XfstCommand::RemoveOptimization => {
                self.remove_optimization();
            }

            // ── read / save ─────────────────────────────────
            XfstCommand::Read(rc) => match rc {
                ReadCmd::Text(s) => {
                    if s.contains('\n') {
                        self.read_text(s);
                    } else {
                        self.read_text_from_file(s)?;
                    }
                }
                ReadCmd::Spaced(s) => {
                    if s.contains('\n') {
                        self.read_spaced(s);
                    } else {
                        self.read_spaced_from_file(s)?;
                    }
                }
                ReadCmd::Prolog(p) => {
                    let s = std::fs::read_to_string(p).unwrap_or_default();
                    self.read_prolog(&s);
                }
                ReadCmd::Props(p) => {
                    let s = std::fs::read_to_string(p).unwrap_or_default();
                    self.read_props(&s);
                }
                ReadCmd::Lexc(p) => {
                    self.read_lexc_from_file(p);
                }
                ReadCmd::Att(p) => {
                    self.read_att_from_file(p);
                }
            },
            XfstCommand::Save(sc) => {
                self.eval_save(sc)?;
            }

            // ── print ───────────────────────────────────────
            XfstCommand::Print(p) => {
                let mut out = std::io::stdout();
                self.eval_print(p, &mut out)?;
            }

            // ── test ────────────────────────────────────────
            XfstCommand::Test(kind) => {
                self.eval_test(*kind, false)?;
            }

            // ── variables / show ────────────────────────────
            XfstCommand::Set { var, value } => {
                // int i = nametoken_to_number(value);
                // if (i != -1) set(var, i); else set(var, value);
                let trimmed = value.trim_start();
                let digits: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
                match digits.parse::<u32>() {
                    Ok(i) if !digits.is_empty() => {
                        self.set_number(var, i);
                    }
                    _ => {
                        self.set(var, value);
                    }
                }
            }
            XfstCommand::Show(opt) => match opt {
                Some(name) => {
                    self.show(name);
                }
                None => {
                    self.show_all();
                }
            },
            XfstCommand::Echo(text) => {
                self.echo(text);
            }
            XfstCommand::System(command) => {
                self.system(command);
            }
            XfstCommand::Source(_path) => {
                // hxfsterror("source not implemented yywrap\n"); return EXIT_FAILURE;
                error!("source not implemented yywrap");
                self.fail_flag_ = true;
            }
            XfstCommand::Quit => {
                self.quit("bye");
            }

            // ── substitute ──────────────────────────────────
            XfstCommand::Substitute(sub) => match sub {
                SubstituteCmd::Symbol { from, to, scope: _ } => {
                    // substitute_symbol expects a quoted list: "s1" "s2" ...
                    let list: String = from
                        .iter()
                        .map(|s| format!("\"{}\"", s))
                        .collect::<Vec<_>>()
                        .join(" ");
                    self.substitute_symbol(&list, to)?;
                }
                SubstituteCmd::Label { from, to, scope: _ } => {
                    self.substitute_label(&from.join(" "), to);
                }
                SubstituteCmd::Named { def, label } => {
                    self.substitute_named(def, label)?;
                }
            },

            // ── help / misc ─────────────────────────────────
            XfstCommand::Apropos(opt) => {
                let text = opt.as_deref().unwrap_or("");
                self.apropos(text);
            }
            XfstCommand::Describe(text) => {
                self.describe(text);
            }
            XfstCommand::Assert(inner) => {
                // The bison grammar only allows 'assert TEST' productions, each
                // of which calls test_X(true).
                if let XfstCommand::Test(kind) = &inner.value {
                    self.eval_test(*kind, true)?;
                } else {
                    self.eval_command(&inner.value)?;
                }
            }
            XfstCommand::AddProps(content) => {
                self.add_props(content);
            }
            XfstCommand::EditProps => {
                // hxfsterror("NETWORK PROPERTY EDITOR unimplemented\n");
                // return EXIT_FAILURE;
                error!("NETWORK PROPERTY EDITOR unimplemented");
                self.fail_flag_ = true;
            }
            XfstCommand::Hfst(data) => {
                self.hfst(data);
            }
            XfstCommand::For => {
                // 'for' has no standalone production in the bison grammar.
                self.prompt();
            }

            // ── i/o redirect wrapper ────────────────────────
            XfstCommand::Redirected { command, redirect } => {
                let inner = &command.value;
                match redirect.kind {
                    RedirectKind::Out | RedirectKind::Append => {
                        if self.check_filename(&redirect.path) {
                            let opened = match redirect.kind {
                                RedirectKind::Append => std::fs::OpenOptions::new()
                                    .append(true)
                                    .create(true)
                                    .open(&redirect.path),
                                _ => std::fs::File::create(&redirect.path),
                            };
                            if let Ok(mut f) = opened {
                                match inner {
                                    XfstCommand::Print(p) => {
                                        self.eval_print(p, &mut f)?;
                                    }
                                    XfstCommand::Save(nfst_xfst::SaveCmd::Att(_)) => {
                                        self.write_att(&mut f);
                                    }
                                    _ => {
                                        self.eval_command(inner)?;
                                    }
                                }
                            }
                        }
                    }
                    RedirectKind::In => {
                        let path = &redirect.path;
                        match inner {
                            XfstCommand::Apply(kind, _) => {
                                let s = std::fs::read_to_string(path).unwrap_or_default();
                                match kind {
                                    ApplyKind::Up => {
                                        self.apply_up(&s)?;
                                    }
                                    ApplyKind::Down => {
                                        self.apply_down(&s)?;
                                    }
                                    ApplyKind::Med => {
                                        self.apply_med(&s);
                                    }
                                }
                            }
                            XfstCommand::AddProps(_) => {
                                let s = std::fs::read_to_string(path).unwrap_or_default();
                                self.add_props(&s);
                            }
                            XfstCommand::Read(ReadCmd::Props(_)) => {
                                let s = std::fs::read_to_string(path).unwrap_or_default();
                                self.read_props(&s);
                            }
                            XfstCommand::LoadStack(_) => {
                                self.load_stack(path);
                            }
                            XfstCommand::LoadDefinitions(_) => {
                                self.load_definitions(path);
                            }
                            _ => {
                                self.eval_command(inner)?;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    // @brief Compile a fully-parsed SpannedXre into a transducer by walking
    // it with self.xre_. Mirrors the regex-compile path the bison actions used.
    // The XreCompiler::compile string entry point parses then walks the tree;
    // here the tree is already parsed, so we walk it directly and optimize,
    // returning a shared handle just like xre_.compile.
    fn compile_spanned_xre(&mut self, xre: &nfst_xre::SpannedXre) -> crate::error::Result<NetRef> {
        let mut t = self.xre_.eval(xre)?;
        t.optimize();
        Ok(Rc::new(RefCell::new(t)))
    }

    // @brief Dispatch a parsed PrintCmd to the corresponding print_* method,
    // writing to \a oss (stdout for plain commands, a file for redirected ones).
    fn eval_print(
        &mut self,
        p: &nfst_xfst::PrintCmd,
        oss: &mut dyn std::io::Write,
    ) -> crate::error::Result<()> {
        use nfst_xfst::PrintCmd as P;
        match p {
            P::Net => {
                self.print_net(oss)?;
            }
            P::Stack => {
                self.print_stack(oss);
            }
            P::Sigma => {
                self.print_sigma(oss, true)?;
            }
            P::SigmaCount => {
                self.print_sigma_count(oss);
            }
            P::SigmaWordCount => {
                self.print_sigma_word_count(oss);
            }
            P::Size => {
                self.print_size(oss);
            }
            P::LongestString => {
                self.print_longest_string(oss)?;
            }
            P::LongestStringSize => {
                self.print_longest_string_size(oss)?;
            }
            P::ShortestString => {
                self.print_shortest_string(oss);
            }
            P::ShortestStringSize => {
                self.print_shortest_string_size(oss);
            }
            P::Flags => {
                self.print_flags(oss);
            }
            P::Labels(opt) => match opt {
                Some(name) => {
                    self.print_labels_name(name, oss);
                }
                None => {
                    self.print_labels(oss);
                }
            },
            P::LabelCount => {
                self.print_label_count(oss);
            }
            P::LabelMaps => {
                self.print_labelmaps(oss);
            }
            P::Name => {
                self.print_name(oss);
            }
            P::Aliases => {
                self.print_aliases(oss);
            }
            P::Arccount => {
                self.print_arc_count(oss);
            }
            P::Defined => {
                self.print_defined(oss);
            }
            P::Dir => {
                self.print_dir("*", oss);
            }
            P::FileInfo => {
                self.print_file_info(oss);
            }
            P::List => {
                self.print_list(oss);
            }
            P::Lists => {
                self.print_list(oss);
            }
            P::Words(n) => {
                self.print_words("", n.unwrap_or(0), oss)?;
            }
            P::LowerWords(n) => {
                self.print_lower_words("", n.unwrap_or(0), oss)?;
            }
            P::UpperWords(n) => {
                self.print_upper_words("", n.unwrap_or(0), oss)?;
            }
            P::RandomWords(n) => {
                self.print_random_words("", n.unwrap_or(15), oss)?;
            }
            P::RandomLower(n) => {
                self.print_random_lower("", n.unwrap_or(15), oss)?;
            }
            P::RandomUpper(n) => {
                self.print_random_upper("", n.unwrap_or(15), oss)?;
            }
            P::Props => {
                self.print_properties(oss);
            }
        }
        Ok(())
    }

    // @brief Dispatch a parsed SaveCmd to the corresponding write_* method.
    // The save commands that the bison grammar wrote to a 'std::ofstream'
    // open the named file here; the ones that take a filename directly are
    // passed through.
    fn eval_save(&mut self, s: &nfst_xfst::SaveCmd) -> crate::error::Result<()> {
        use nfst_xfst::SaveCmd as S;
        match s {
            S::Stack(p) => {
                self.write_stack(p)?;
            }
            S::Definitions(p) => {
                self.write_definitions(p)?;
            }
            S::Definition(p) => {
                self.write_definition(p, "")?;
            }
            S::Prolog(p) => {
                if self.check_filename(p) {
                    if let Ok(mut f) = std::fs::File::create(p) {
                        self.write_prolog(&mut f);
                    }
                }
            }
            S::Spaced(p) => {
                if self.check_filename(p) {
                    if let Ok(mut f) = std::fs::File::create(p) {
                        self.write_spaced(&mut f);
                    }
                }
            }
            S::Text(p) => {
                if self.check_filename(p) {
                    if let Ok(mut f) = std::fs::File::create(p) {
                        self.write_text(&mut f);
                    }
                }
            }
            S::Dot(p) => {
                if self.check_filename(p) {
                    if let Ok(mut f) = std::fs::File::create(p) {
                        self.write_dot(&mut f);
                    }
                }
            }
            S::Att(p) => {
                if p.is_empty() {
                    let mut out = std::io::stdout();
                    self.write_att(&mut out);
                } else if self.check_filename(p) {
                    if let Ok(mut f) = std::fs::File::create(p) {
                        self.write_att(&mut f);
                    }
                }
            }
        }
        Ok(())
    }

    // @brief Dispatch a parsed TestKind to the corresponding test_* method.
    // \a assertion is true when the test was wrapped in 'assert'.
    fn eval_test(
        &mut self,
        kind: nfst_xfst::TestKind,
        assertion: bool,
    ) -> crate::error::Result<()> {
        use nfst_xfst::TestKind as T;
        match kind {
            T::Eq => {
                self.test_eq(assertion)?;
            }
            T::Funct => {
                self.test_funct(assertion);
            }
            T::Id => {
                self.test_id(assertion)?;
            }
            T::Null => {
                self.test_null(false, assertion)?;
            }
            T::Nonnull => {
                self.test_nonnull(assertion)?;
            }
            T::Overlap => {
                self.test_overlap(assertion)?;
            }
            T::Sublanguage => {
                self.test_sublanguage(assertion)?;
            }
            T::Unambiguous => {
                self.test_unambiguous(assertion);
            }
            T::InfinitelyAmbiguous => {
                self.test_infinitely_ambiguous(assertion)?;
            }
            T::LowerBounded => {
                self.test_lower_bounded(assertion)?;
            }
            T::LowerUni => {
                self.test_lower_uni(assertion)?;
            }
            T::UpperBounded => {
                self.test_upper_bounded(assertion)?;
            }
            T::UpperUni => {
                self.test_upper_uni(assertion)?;
            }
        }
        Ok(())
    }
}

// ====================================================================
// vars-print-control group: VARIABLES + PRINT + control
// ====================================================================
//
// Output convention: the C++ output() returns *output_ (std::cout by
// default) and error() returns *error_ (std::cerr by default); get_stream()
// is the identity on non-Windows and flush() is a no-op there. We mirror that
// Unix behaviour directly: output() writes to stdout via print!, diagnostics
// go through the tracing macros, and the print_* methods write their 'stream'
// content to the oss writer passed in (the C++ '*oss' target).
impl XfstCompiler {
    // @brief Print @a text to stdout
    pub fn echo(&mut self, text: &str) -> &mut Self {
        print!("{}\n", text);
        self.prompt();
        return self;
    }

    // @brief Stop parser, print quit message
    pub fn quit(&mut self, message: &str) -> &mut Self {
        if self.verbose_ && (message == "dodongo") {
            print!("dislikes smoke.\n");
        } else if self.verbose_ {
            print!("{}.\n", message);
        } else {
            // ;
        }
        self.quit_requested_ = true;
        return self;
    }

    // @brief Execute @c system()
    pub fn system(&mut self, command: &str) -> &mut Self {
        if self.restricted_mode_ {
            warn!("Restricted mode (--restricted-mode) is in use, system calls are disabled");
            self.xfst_lesser_fail();
            self.prompt();
            return self;
        }
        let rv = run_shell(command);
        if rv != 0 {
            warn!("system {} returned {}", command, rv);
        }
        self.prompt();
        return self;
    }

    // @brief Set variable @c name = @c text
    pub fn set(&mut self, name: &str, text: &str) -> &mut Self {
        if !self.variables_.contains_key(name) {
            if name == "compose-flag-as-special" {
                warn!("variable compose-flag-as-special not found, using flag-is-epsilon instead");
                self.variables_
                    .insert("flag-is-epsilon".to_string(), text.to_string());
                if self.verbose_ {
                    print!("variable flag-is-epsilon = {}\n", text);
                }
                self.prompt();
                return self;
            } else {
                error!("no such variable: '{}'", name);
                self.prompt();
                return self;
            }
        }
        self.variables_.insert(name.to_string(), text.to_string());
        if name == "hopcroft-min" {
            if text == "ON" {
                self.engine_config_.minimization_algorithm =
                    crate::hfst_transducer::MinimizationAlgorithm::HOPCROFT;
            }
            if text == "OFF" {
                self.engine_config_.minimization_algorithm =
                    crate::hfst_transducer::MinimizationAlgorithm::BRZOZOWSKI;
            }
        }
        if name == "encode-weights" {
            if text == "ON" {
                self.engine_config_.encode_weights = true;
            }
            if text == "OFF" {
                self.engine_config_.encode_weights = false;
            }
        }
        if name == "harmonize-flags" {
            if text == "ON" {
                self.xre_.set_flag_harmonization(true);
            }
            if text == "OFF" {
                self.xre_.set_flag_harmonization(false);
            }
        }
        if name == "xerox-composition" {
            if text == "ON" {
                self.engine_config_.xerox_composition = true;
            }
            if text == "OFF" {
                self.engine_config_.xerox_composition = false;
            }
        }
        if name == "flag-is-epsilon" {
            if text == "ON" {
                self.engine_config_.flag_is_epsilon_in_composition = true;
            }
            if text == "OFF" {
                self.engine_config_.flag_is_epsilon_in_composition = false;
            }
        }
        if name == "minimal" {
            // 'set minimal' was left unwired in this port (the C++ would toggle
            // 'minimization'); the compiler's 'optimize' calls run with the default
            // config, so this stays a no-op, as before.
            let _ = text;
        }

        if self.verbose_ {
            print!("variable {} = {}\n", name, text);
        }

        self.prompt();
        return self;
    }

    // @brief Set variable @c name = @c number
    pub fn set_number(&mut self, name: &str, number: u32) -> &mut Self {
        if !self.variables_.contains_key(name) {
            error!("no such variable: '{}'", name);
            self.prompt();
            return self;
        }
        let num = format!("{}", number);
        self.variables_.insert(name.to_string(), num);
        self.prompt();
        return self;
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-fn]
    // @brief Get variable \a name.
    pub fn get(&mut self, name: &str) -> String {
        if !self.variables_.contains_key(name) {
            return String::new();
        }
        return self.variables_[name].clone();
    }

    // @brief Show named variable
    pub fn show(&mut self, name: &str) -> &mut Self {
        if !self.variables_.contains_key(name) {
            error!("no such variable: '{}'", name);
            self.prompt();
            return self;
        }
        print!("variable {} = {}\n", name, self.variables_[name]);
        self.prompt();
        return self;
    }

    // @brief Show all variables
    pub fn show_all(&mut self) -> &mut Self {
        let vars: Vec<(String, String)> = self
            .variables_
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (first, second) in vars.iter() {
            if first == "copyright-owner" {
                print!("{:>20}: {}\n", first, second);
            } else {
                let explanation = variable_explanations_get(first);
                print!("{:>20}: {:>6}: {}\n", first, second, explanation);
            }
        }
        self.prompt();
        return self;
    }

    // @brief Print aliases
    pub fn print_aliases(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let aliases: Vec<(String, String)> = self
            .aliases_
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (first, second) in aliases.iter() {
            let _ = write!(oss, "{:>10}", "alias ");
            let _ = write!(oss, "{} {}", first, second);
        }
        self.flush();
        self.prompt();
        return self;
    }

    // @brief Print definition
    pub fn print_defined(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let mut definitions = false;
        let defs: Vec<(String, String)> = self
            .original_definitions_
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (first, second) in defs.iter() {
            definitions = true;
            let _ = write!(oss, "{:>10}", first);
            let _ = write!(oss, " {}\n", second);
        }
        if !definitions {
            let _ = write!(oss, "No defined symbols.\n");
        }

        definitions = false;
        let funcs: Vec<(String, String)> = self
            .original_function_definitions_
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (first, second) in funcs.iter() {
            definitions = true;
            let _ = write!(oss, "{:>10}", first);
            let _ = write!(oss, " {}\n", second);
        }
        if !definitions {
            let _ = write!(oss, "No function definitions.\n");
        }

        self.flush();
        self.prompt();
        return self;
    }

    // @brief Print directory contents
    pub fn print_dir(&mut self, glob: &str, oss: &mut dyn std::io::Write) -> &mut Self {
        match glob::glob(glob) {
            Ok(paths) => {
                for entry in paths.flatten() {
                    let _ = write!(oss, "{}\n", entry.display());
                }
            }
            Err(e) => {
                let _ = write!(oss, "glob({}) = {}\n", glob, e);
            }
        }
        self.prompt();
        return self;
    }

    pub fn print_labels_tr(
        &mut self,
        oss: &mut dyn std::io::Write,
        tr: &HfstTransducer,
    ) -> &mut Self {
        let mut label_set: BTreeSet<(String, String)> = BTreeSet::new();
        let fsm = HfstBasicTransducer::new_from_transducer(tr);

        for it in fsm.iter() {
            for tr_it in it.iter() {
                label_set.insert((
                    tr_it.get_input_symbol(fsm.coder()),
                    tr_it.get_output_symbol(fsm.coder()),
                ));
            }
        }

        let _ = write!(oss, "Labels: ");
        let first_elem = label_set.iter().next().cloned();
        for it in label_set.iter() {
            if Some(it) != first_elem.as_ref() {
                let _ = write!(oss, ", ");
            }
            let _ = write!(oss, "{}", it.0);
            if it.0 != it.1 {
                let _ = write!(oss, ":{}", it.1);
            }
        }
        let _ = write!(oss, "\n");
        let _ = write!(oss, "Size: {}\n", label_set.len() as i32);

        self.flush();
        self.prompt();
        return self;
    }

    // @brief Print labels in network @a name
    pub fn print_labels_name(&mut self, name: &str, oss: &mut dyn std::io::Write) -> &mut Self {
        match self.definitions_.get(name).cloned() {
            None => {
                let _ = write!(oss, "no such definition '{}'\n", name);
            }
            Some(tr) => {
                return self.print_labels_tr(oss, &tr.borrow());
            }
        }
        self.flush();
        self.prompt();
        return self;
    }

    // @brief Print labels
    pub fn print_labels(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let Some(topmost) = self.top() else {
            self.xfst_lesser_fail();
            return self;
        };
        return self.print_labels_tr(oss, &topmost.borrow());
    }

    // @brief Print label count
    pub fn print_label_count(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let Some(topmost) = self.top() else {
            self.xfst_lesser_fail();
            return self;
        };

        let mut label_map: BTreeMap<(String, String), u32> = BTreeMap::new();
        let fsm = HfstBasicTransducer::new_from_transducer(&topmost.borrow());

        for it in fsm.iter() {
            for tr_it in it.iter() {
                *label_map
                    .entry((
                        tr_it.get_input_symbol(fsm.coder()),
                        tr_it.get_output_symbol(fsm.coder()),
                    ))
                    .or_insert(0) += 1;
            }
        }

        let mut index: u32 = 1;
        let first_key = label_map.keys().next().cloned();
        for (key, value) in label_map.iter() {
            if Some(key) != first_key.as_ref() {
                let _ = write!(oss, "   ");
            }
            let _ = write!(oss, "{}. ", index);
            let _ = write!(oss, "{}", key.0);
            if key.0 != key.1 {
                let _ = write!(oss, ":{}", key.1);
            }
            let _ = write!(oss, " {}", value);
            index += 1;
        }
        let _ = write!(oss, "\n");

        self.flush();
        self.prompt();
        return self;
    }

    // @brief Print list named @a name
    pub fn print_list_name(&mut self, name: &str, oss: &mut dyn std::io::Write) -> &mut Self {
        if !self.lists_.contains_key(name) {
            let _ = write!(oss, "No such list defined: {}\n", name);
            self.flush();
            self.prompt();
            return self;
        }
        let l = self.lists_[name].clone();
        let _ = write!(oss, "{:>10}", name);
        let _ = write!(oss, ": ");
        for s in l.iter() {
            let _ = write!(oss, "{} ", s);
        }
        let _ = write!(oss, "\n");
        self.flush();
        self.prompt();
        return self;
    }

    // @brief Print all lists
    pub fn print_list(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        if self.lists_.len() == 0 {
            let _ = write!(oss, "No lists defined.\n");
            self.flush();
            self.prompt();
            return self;
        }
        let lists: Vec<(String, BTreeSet<String>)> = self
            .lists_
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (first, second) in lists.iter() {
            // HERE
            let _ = write!(oss, "{:>10}", first);
            let _ = write!(oss, " ");
            for s in second.iter() {
                let _ = write!(oss, "{} ", s);
            }
            let _ = write!(oss, "\n");
        }
        self.flush();
        self.prompt();
        return self;
    }

    pub fn shortest_string(
        &mut self,
        transducer: &HfstTransducer,
        paths: &mut HfstTwoLevelPaths,
    ) -> &mut Self {
        transducer.extract_shortest_paths(paths);
        return self;
    }

    // @brief Print shortest string of network
    pub fn print_shortest_string(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let Some(topmost) = self.top() else {
            self.xfst_lesser_fail();
            return self;
        };

        let mut paths = HfstTwoLevelPaths::new();
        self.shortest_string(&topmost.borrow(), &mut paths);

        if paths.len() == 0 {
            print!("transducer is empty\n");
        } else {
            self.print_paths_two(&paths, oss, -1);
        }
        self.flush();
        self.prompt();
        return self;
    }

    // @brief Print length of shortest string
    pub fn print_shortest_string_size(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let Some(topmost) = self.top() else {
            self.xfst_lesser_fail();
            return self;
        };

        let mut paths = HfstTwoLevelPaths::new();
        self.shortest_string(&topmost.borrow(), &mut paths);

        if paths.len() == 0 {
            print!("transducer is empty\n");
        } else {
            let _ = write!(
                oss,
                "{}\n",
                paths.iter().next().unwrap().second.len() as i32
            );
        }
        self.flush();
        self.prompt();
        return self;
    }

    // @brief Print longest string in network
    pub fn print_longest_string(
        &mut self,
        oss: &mut dyn std::io::Write,
    ) -> crate::error::Result<&mut Self> {
        return self.print_longest_string_or_its_size(oss, false);
    }

    // @brief Print length of longest string
    pub fn print_longest_string_size(
        &mut self,
        oss: &mut dyn std::io::Write,
    ) -> crate::error::Result<&mut Self> {
        return self.print_longest_string_or_its_size(oss, true);
    }

    // @brief Print strings of lower language
    pub fn print_lower_words(
        &mut self,
        name: &str,
        number: u32,
        oss: &mut dyn std::io::Write,
    ) -> crate::error::Result<&mut Self> {
        return self.print_words_level(name, number, oss, Level::LOWER_LEVEL);
    }

    // @brief Print random strings of lower language
    pub fn print_random_lower(
        &mut self,
        name: &str,
        number: u32,
        oss: &mut dyn std::io::Write,
    ) -> crate::error::Result<&mut Self> {
        let mut paths = HfstTwoLevelPaths::new();

        // [spec:hfst:def:xfst-compiler.hfst.xfst.tmp-fn]
        // [spec:hfst:sem:xfst-compiler.hfst.xfst.tmp-fn]
        let mut tmp = HfstTransducer::new_type(self.format_)?;
        if name.is_empty() {
            let Some(temp) = self.top() else {
                return Ok(self);
            };
            tmp = HfstTransducer::new_from_transducer(&temp.borrow());
        } else {
            match self.definitions_.get(name).cloned() {
                None => {
                    let _ = write!(oss, "no such definition '{}'\n", name);
                    self.flush();
                    self.prompt();
                    return Ok(self);
                }
                Some(it) => {
                    tmp = HfstTransducer::new_from_transducer(&it.borrow());
                }
            }
        }

        tmp.output_project()?;
        tmp.extract_random_paths(&mut paths, number as i32)?;
        self.print_paths_two(&paths, oss, -1);
        self.flush();
        self.prompt();
        return Ok(self);
    }

    // @brief Print astrings of upper language
    pub fn print_upper_words(
        &mut self,
        name: &str,
        number: u32,
        oss: &mut dyn std::io::Write,
    ) -> crate::error::Result<&mut Self> {
        return self.print_words_level(name, number, oss, Level::UPPER_LEVEL);
    }

    // @brief Print random strings of upper language
    pub fn print_random_upper(
        &mut self,
        name: &str,
        number: u32,
        oss: &mut dyn std::io::Write,
    ) -> crate::error::Result<&mut Self> {
        let mut paths = HfstTwoLevelPaths::new();

        let mut tmp = HfstTransducer::new_type(self.format_)?;
        if name.is_empty() {
            let Some(temp) = self.top() else {
                return Ok(self);
            };
            tmp = HfstTransducer::new_from_transducer(&temp.borrow());
        } else {
            match self.definitions_.get(name).cloned() {
                None => {
                    let _ = write!(oss, "no such definition '{}\n", name);
                    self.flush();
                    self.prompt();
                    return Ok(self);
                }
                Some(it) => {
                    tmp = HfstTransducer::new_from_transducer(&it.borrow());
                }
            }
        }

        tmp.input_project()?;
        tmp.extract_random_paths(&mut paths, number as i32)?;
        self.print_paths_two(&paths, oss, -1);
        self.flush();
        self.prompt();
        return Ok(self);
    }

    // @brief Print pair strings of language
    pub fn print_words(
        &mut self,
        name: &str,
        number: u32,
        oss: &mut dyn std::io::Write,
    ) -> crate::error::Result<&mut Self> {
        return self.print_words_level(name, number, oss, Level::BOTH_LEVELS);
    }

    // @brief Print random pair strings of language
    pub fn print_random_words(
        &mut self,
        name: &str,
        number: u32,
        oss: &mut dyn std::io::Write,
    ) -> crate::error::Result<&mut Self> {
        let tmp: NetRef;
        if name.is_empty() {
            let Some(t) = self.top() else {
                return Ok(self);
            };
            tmp = t;
        } else {
            match self.definitions_.get(name).cloned() {
                None => {
                    let _ = write!(oss, "no such definition '{}'\n", name);
                    self.flush();
                    self.prompt();
                    return Ok(self);
                }
                Some(it) => {
                    tmp = it;
                }
            }
        }

        let mut paths = HfstTwoLevelPaths::new();
        tmp.borrow()
            .extract_random_paths(&mut paths, number as i32)?;
        self.print_paths_two(&paths, oss, -1);
        self.flush();
        self.prompt();
        return Ok(self);
    }

    // @brief Print name of top network
    pub fn print_name(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let Some(tmp) = self.top() else {
            self.xfst_lesser_fail();
            return self;
        };

        let entries: Vec<(String, NetRef)> = self
            .names_
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        for (first, second) in entries.iter() {
            if Rc::ptr_eq(&tmp, second) {
                let _ = write!(oss, "Name {}\n", first);
                self.flush();
                self.prompt();
                return self;
            }
        }

        let _ = write!(oss, "No name.\n");
        self.flush();
        self.prompt();
        return self;
    }

    // @brief View top network
    pub fn view_net(&mut self) -> &mut Self {
        let Some(tmp) = self.top() else {
            self.xfst_lesser_fail();
            return self;
        };
        let dotfilename = format!(
            "{}/hfst_view_dot_{}",
            std::env::temp_dir().to_string_lossy(),
            std::process::id()
        );
        let pngfilename = format!(
            "{}/hfst_view_png_{}",
            std::env::temp_dir().to_string_lossy(),
            std::process::id()
        );
        if false || self.verbose_ {
            debug!(
                "Writing net in dot format to temporary file '{}'.",
                dotfilename
            );
        }
        {
            let mut dotfile = match std::fs::File::create(&dotfilename) {
                Ok(f) => f,
                Err(_) => {
                    self.prompt();
                    return self;
                }
            };
            crate::hfst_print_dot::print_dot_os(&mut dotfile, &mut *tmp.borrow_mut());
        }
        if false || self.verbose_ {
            debug!("Wrote net, closing file and converting into png format.");
        }
        let cmd1 = format!("dot -Tpng {} > {} 2> /dev/null", dotfilename, pngfilename);
        if run_shell(&cmd1) != 0 {
            error!("Converting failed.");
            self.xfst_lesser_fail();
        }
        if false || self.verbose_ {
            debug!("Converted to png format, viewing the graph.");
        }
        let cmd2 = format!("/usr/bin/xdg-open {} 2> /dev/null &", pngfilename);
        if run_shell(&cmd2) != 0 {
            error!("Viewing failed.");
            self.xfst_lesser_fail();
        }
        self.prompt();
        return self;
    }

    // @brief Print network
    pub fn print_net(&mut self, oss: &mut dyn std::io::Write) -> crate::error::Result<&mut Self> {
        if self.variables_["print-sigma"] == "ON" {
            self.print_sigma(oss, false /*do not prompt*/)?;
        }
        let Some(tmp) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };
        let basic = HfstBasicTransducer::new_from_transducer(&tmp.borrow());
        basic.write_in_xfst_format(oss, self.variables_["print-weight"] == "ON");
        self.flush();
        self.prompt();
        return Ok(self);
    }

    // @brief Print network named @a name
    pub fn print_net_name(
        &mut self,
        name: &str,
        oss: &mut dyn std::io::Write,
    ) -> crate::error::Result<&mut Self> {
        match self.definitions_.get(name).cloned() {
            None => {
                error!("no such defined network: '{}'", name);
                self.prompt();
                return Ok(self);
            }
            Some(it) => {
                if self.variables_["print-sigma"] == "ON" {
                    self.stack_.push(it.clone());
                    self.print_sigma(oss, false /*do not prompt*/)?;
                    self.stack_.pop();
                }
                let basic = HfstBasicTransducer::new_from_transducer(&it.borrow());
                basic.write_in_xfst_format(oss, self.variables_["print-weight"] == "ON");
                self.flush();
                self.prompt();
                return Ok(self);
            }
        }
    }

    // @brief Print all symbols of network
    pub fn print_sigma(
        &mut self,
        oss: &mut dyn std::io::Write,
        prompt: bool,
    ) -> crate::error::Result<&mut Self> {
        let Some(t) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };
        let alpha = t.borrow().get_alphabet()?;

        // find out whether unknown or identity is used in transitions
        let mut unknown = false;
        let mut identity = false;
        let _ = is_unknown_or_identity_used_in_transducer(&t.borrow(), &mut unknown, &mut identity);

        self.print_alphabet(&alpha, unknown, identity, oss);
        if prompt {
            self.prompt();
        }
        self.flush();
        return Ok(self);
    }

    // @brief Print all symbols of network named @a name
    pub fn print_sigma_name(&mut self, _name: &str, oss: &mut dyn std::io::Write) -> &mut Self {
        let _ = write!(oss, "missing print sigma\n");
        self.flush();
        self.prompt();
        return self;
    }

    // @brief Print all networks in stack
    pub fn print_stack(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let mut tmp: Vec<NetRef> = Vec::new();
        let mut i = 0;
        while !self.stack_.is_empty() {
            let _ = write!(
                oss,
                "{:>10}",
                format!("{}: ? bytes. ? states, ? arcs, ? paths.", i)
            );
            let _ = write!(oss, "\n");
            tmp.push(self.stack_.last().unwrap().clone());
            self.stack_.pop();
            i += 1;
        }
        while !tmp.is_empty() {
            self.stack_.push(tmp.last().unwrap().clone());
            tmp.pop();
        }
        self.flush();
        self.prompt();
        return self;
    }

    // @brief Write top transducer in att format to @a outfile
    pub fn write_att(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let Some(tmp) = self.top() else {
            self.xfst_lesser_fail();
            return self;
        };
        let fsm = HfstBasicTransducer::new_from_transducer(&tmp.borrow());
        fsm.write_in_att_format_os(oss, self.variables_["print-weight"] == "ON");
        self.flush();
        self.prompt();
        return self;
    }

    // @brief Search help directory
    // @todo helps have not been written or copied
    pub fn apropos(&mut self, text: &str) -> &mut Self {
        let mut message = String::new();
        if !get_help_message(text, &mut message, HELP_MODE_APROPOS) {
            print!("nothing found for '{}'\n", text);
        } else {
            print!("{}", message);
        }
        self.prompt();
        return self;
    }

    // @brief Print help topics
    // @todo helps have not been written or copied
    pub fn describe(&mut self, text: &str) -> &mut Self {
        let help_mode = if text == "" {
            HELP_MODE_ALL_COMMANDS
        } else {
            HELP_MODE_ONE_COMMAND
        };
        let mut message = String::new();
        if !get_help_message(text, &mut message, help_mode) {
            print!("no help found for '{}'\n", text);
        } else {
            print!("{}", message);
        }
        self.prompt();
        return self;
    }

    // @brief Name top of stack
    // @todo HFST automata do not remember their names
    pub fn name_net(&mut self, name: &str) -> &mut Self {
        if self.stack_.len() < 1 {
            warn!("Empty stack.");
            self.xfst_lesser_fail();
            return self;
        }
        let t = self.stack_.last().unwrap().clone();
        t.borrow_mut().set_name(name);
        self.names_.insert(name.to_string(), t);
        self.print_transducer_info();
        self.prompt();
        return self;
    }

    // @brief Sekrit HFST raw command mode!
    pub fn hfst(&mut self, data: &str) -> &mut Self {
        info!("HFST: {}", data);
        self.prompt();
        return self;
    }

    // @brief Get current stack of compiler
    pub fn get_stack(&self) -> &Vec<NetRef> {
        return &self.stack_;
    }

    // @brief Define whether readline library is used to read input in apply up etc.
    pub fn set_readline(&mut self, readline: bool) -> &mut Self {
        self.use_readline_ = readline;
        return self;
    }

    // @brief Define whether input is read from stdin in apply up etc.
    pub fn set_read_interactive_text_from_stdin(&mut self, value: bool) -> &mut Self {
        self.read_interactive_text_from_stdin_ = value;
        return self;
    }

    // @brief Define whether output is printed directly to windows console.
    pub fn set_output_to_console(&mut self, value: bool) -> &mut Self {
        self.output_to_console_ = value;
        // hfst::print_output_to_console(output_to_console_);
        return self;
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-readline-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-readline-fn]
    // @brief Whether readline is used to read input in apply up etc.
    pub fn get_readline(&mut self) -> bool {
        return self.use_readline_;
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-read-interactive-text-from-stdin-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-read-interactive-text-from-stdin-fn]
    // @brief Whether stdin is used to read input in apply up etc.
    pub fn get_read_interactive_text_from_stdin(&mut self) -> bool {
        return self.read_interactive_text_from_stdin_;
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-output-to-console-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-output-to-console-fn]
    // @brief Whether output is printed directly to windows console.
    pub fn get_output_to_console(&mut self) -> bool {
        return self.output_to_console_;
    }

    // @brief Define wheter prompts and XFST outputs are printed.
    pub fn set_verbosity(&mut self, verbosity: bool) -> &mut Self {
        self.verbose_ = verbosity;
        self.xre_.set_verbosity(verbosity);
        self.lexc_.set_verbosity(if self.verbose_ { 2 } else { 0 });
        return self;
    }

    // @brief Define wheter prompts are printed.
    pub fn set_prompt_verbosity(&mut self, verbosity: bool) -> &mut Self {
        self.verbose_prompt_ = verbosity;
        return self;
    }

    // @brief Explicitly print the prompt to stdout.
    pub fn prompt(&mut self) -> &Self {
        if self.verbose_prompt_ && self.verbose_ {
            // On windows, prompt is always printed to console. On other platforms,
            // this has no effect.
            print!("hfst[{}]: ", self.stack_.len());
        }
        return self;
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-prompt-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-prompt-fn]
    // @brief Get the prompt string.
    pub fn get_prompt(&self) -> String {
        return format!("hfst[{}]: ", self.stack_.len());
    }

    // @brief Allow read and write operations only in current directory, do not allow system calls.
    pub fn set_restricted_mode(&mut self, value: bool) -> &mut Self {
        self.restricted_mode_ = value;
        return self;
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-restricted-mode-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-restricted-mode-fn]
    // @brief Whether restricted mode is on.
    pub fn get_restricted_mode(&self) -> bool {
        return self.restricted_mode_;
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.quit-requested-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.quit-requested-fn]
    // @brief Whether it has been requested to quit the program.
    pub fn quit_requested(&self) -> bool {
        return self.quit_requested_;
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.unknown-command-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.unknown-command-fn]
    // @brief Handle unknown command \a s.
    //  @return Whether the parser should go on, 0 signifying true.
    pub fn unknown_command(&mut self, s: &str) -> i32 {
        if self.variables_["quit-on-fail"] == "ON" {
            if self.verbose_ {
                error!("Command {} is not recognised.", s);
                // fprintf(stderr, "Command %s is not recognised.\n", s);
            }
            return 1;
        }
        error!("Command {} is not recognised.", s);
        // fprintf(stderr, "Command %s is not recognised.\n", s);
        self.prompt();
        return 0;
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-fail-flag-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-fail-flag-fn]
    // For xfst parser.
    pub fn get_fail_flag(&self) -> bool {
        return self.fail_flag_;
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.set-error-stream-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.set-error-stream-fn]
    /* Set the stream where error messages and warnings are printed. */
    pub fn set_error_stream(&mut self) {
        // error_ = &os;
        // this->xre_.set_error_stream(this->error_);
        // this->lexc_.set_error_stream(this->error_);
    }

    /* Get the stream where error messages and warnings are printed. */
    pub fn get_error_stream(&mut self) {
        // return *error_;
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.set-output-stream-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.set-output-stream-fn]
    /* Set the stream where output is printed. */
    pub fn set_output_stream(&mut self) {
        // output_ = &os;
    }

    /* Get the stream where output is printed. */
    pub fn get_output_stream(&mut self) {
        // return *output_;
    }

    /* Get the output stream. */
    pub fn output(&mut self) {
        // return *output_;
    }

    /* Get the error stream. */
    pub fn error(&mut self) {
        // return *error_;
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.flush-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.flush-fn]
    /* Flush the stream. */
    pub fn flush(&mut self) {
        // On Unix and Mac this is a no-op; the WINDOWS console-buffering branch
        // is not ported.
        use std::io::Write as _;
        let _ = std::io::stdout().flush();
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.print-paths-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.print-paths-fn]
    // @brief Print \a n first paths (or all, if n is negative)
    // from \a paths to \a outfile.
    fn print_paths_two(
        &mut self,
        paths: &HfstTwoLevelPaths,
        oss: &mut dyn std::io::Write,
        mut n: i32,
    ) -> bool {
        let mut retval = false; // if anything was printed
        let _precision = self.get_precision();

        // go through at most n paths
        for it in paths.iter() {
            if n == 0 {
                break;
            }
            let path = it.second.clone();
            let mut something_printed = false; // to control printing spaces

            if self.variables_["obey-flags"] == "ON" {
                let path_input =
                    crate::hfst_symbol_defs::symbols::to_string_vector_from_string_pair_vector(
                        &path, true, /*input side*/
                    );
                if !is_valid_string(&path_input) {
                    continue;
                }
            }

            retval = true; // something will be printed

            // go through the path
            for p in path.iter() {
                let print_symbol = self.get_print_symbol(&p.0);

                // see if symbol separator (space) is needed
                if self.variables_["print-space"] == "ON" // print space required
                    && something_printed                  // not first symbol shown
                    && print_symbol != ""
                // something to show
                {
                    let _ = write!(oss, " ");
                }

                let _ = write!(oss, "{}", print_symbol);

                if print_symbol != "" {
                    something_printed = true;
                }

                let print_symbol = self.get_print_symbol(&p.1);

                // see if output symbol is needed
                if print_symbol != "" // something to show
                    && p.0 != p.1
                // input and output symbols differ
                {
                    let _ = write!(oss, ":{}", print_symbol);
                }
            } // path gone through

            // if needed, print the weight
            if self.variables_["print-weight"] == "ON" {
                let _ = write!(oss, "\t{}", it.first);
            }

            let _ = write!(oss, "\n");
            n -= 1;
        } // at most n paths gone through

        self.flush();
        return retval;
    }

    // @brief Print \a n first paths (or all, if n is negative)
    // from \a paths to \a outfile.
    fn print_paths_one(
        &mut self,
        paths: &HfstOneLevelPaths,
        oss: &mut dyn std::io::Write,
        mut n: i32,
    ) -> bool {
        let mut retval = false; // if anything was printed
        let _precision = self.get_precision();

        // go through at most n paths
        for it in paths.iter() {
            let path = it.second.clone();
            let mut something_printed = false; // to control printing spaces

            if (self.variables_["obey-flags"] == "ON") && !is_valid_string(&path) {
                continue;
            }

            retval = true; // something will be printed

            // go through the path
            for p in path.iter() {
                let print_symbol = self.get_print_symbol(p);

                // see if symbol separator (space) is needed
                if self.variables_["print-space"] == "ON" // print space required
                    && something_printed                  // not first symbol shown
                    && print_symbol != ""
                // something to show
                {
                    let _ = write!(oss, " ");
                }

                let _ = write!(oss, "{}", print_symbol);

                if print_symbol != "" {
                    something_printed = true;
                }
            } // path gone through

            // if needed, print the weight
            if self.variables_["print-weight"] == "ON" {
                let _ = write!(oss, "\t{}", it.first);
            }

            let _ = write!(oss, "\n");
            n -= 1;
        } // at most n paths gone through

        self.flush();
        return retval;
    }

    // A method used by function print_longest_string_or_its_size.
    fn print_one_string_or_its_size(
        &mut self,
        oss: &mut dyn std::io::Write,
        paths: &HfstTwoLevelPaths,
        level: &str,
        print_size: bool,
    ) -> &mut Self {
        let _ = write!(oss, "{}: ", level);
        if print_size {
            let _ = write!(
                oss,
                "{}\n",
                paths.iter().next().unwrap().second.len() as i32
            );
        } else {
            self.print_paths_two(paths, oss, 1);
        }
        self.flush();
        return self;
    }

    // @brief Print the longest string of topmost transducer in the stack
    // (if print_size is false) or the size of that string (if print_size is true)
    // to \a outfile.
    fn print_longest_string_or_its_size(
        &mut self,
        oss: &mut dyn std::io::Write,
        print_size: bool,
    ) -> crate::error::Result<&mut Self> {
        let Some(topmost) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };

        // Variables needed to find out some properties about the transducer
        let mut tmp_lower = HfstTransducer::new_from_transducer(&topmost.borrow());
        let mut tmp_upper = HfstTransducer::new_from_transducer(&topmost.borrow());
        tmp_lower.output_project()?.remove_epsilons()?;
        tmp_upper.input_project()?.remove_epsilons()?;

        let mut paths_upper = HfstTwoLevelPaths::new();
        let mut paths_lower = HfstTwoLevelPaths::new();
        let mut upper_is_cyclic = false;
        let mut lower_is_cyclic = false;
        let mut transducer_is_empty = false;

        // Transducer is empty if neither upper..
        let obey_flags_upper = self.variables_["obey-flags"] == "ON";
        match tmp_upper.extract_longest_paths(&mut paths_upper, obey_flags_upper) {
            Ok(v) => {
                transducer_is_empty = !v;
            }
            Err(e) => {
                if matches!(e.kind, crate::error::ErrorKind::TransducerIsCyclic) {
                    upper_is_cyclic = true;
                } else {
                    return Err(e);
                }
            }
        }

        // ..nor lower paths can be extracted.
        let obey_flags_lower = self.variables_["obey-flags"] == "ON";
        match tmp_lower.extract_longest_paths(&mut paths_lower, obey_flags_lower) {
            Ok(v) => {
                transducer_is_empty = !v;
            }
            Err(e) => {
                if matches!(e.kind, crate::error::ErrorKind::TransducerIsCyclic) {
                    lower_is_cyclic = true;
                } else {
                    return Err(e);
                }
            }
        }

        // Print the results:
        // first, the special cases,
        if upper_is_cyclic && lower_is_cyclic {
            print!("transducer is cyclic\n");
        } else if transducer_is_empty {
            print!("transducer is empty\n");
        }
        // then the usual:
        else {
            // warn about flag diacritics
            if self.variables_["show-flags"] == "OFF"
                && (tmp_upper.has_flag_diacritics() || tmp_lower.has_flag_diacritics())
            {
                warn!(
                    "longest string may have flag diacritics that are not shown\n         but are used in calculating its length (use 'eliminate flags')"
                );
            }

            // print one longest string of the upper level, if not cyclic
            if upper_is_cyclic {
                let _ = write!(oss, "Upper level is cyclic.\n");
            } else {
                self.print_one_string_or_its_size(oss, &paths_upper, "Upper", print_size);
            }

            // print one longest string of the lower level, if not cyclic
            if lower_is_cyclic {
                let _ = write!(oss, "Lower level is cyclic.\n");
            } else {
                self.print_one_string_or_its_size(oss, &paths_lower, "Lower", print_size);
            }
        }

        self.flush();
        self.prompt();
        return Ok(self);
    }

    // @brief Try to extract a maximum of \a number paths from topmost
    // transducer in the stack and print them to \a outfile. \a level
    // defines whether the input or output level is printed or both are printed.
    fn print_words_level(
        &mut self,
        name: &str,
        number: u32,
        oss: &mut dyn std::io::Write,
        level: Level,
    ) -> crate::error::Result<&mut Self> {
        // [spec:hfst:def:xfst-compiler.hfst.xfst.temp-fn]
        // [spec:hfst:sem:xfst-compiler.hfst.xfst.temp-fn]
        let mut temp = HfstTransducer::new_type(self.format_)?;
        if name.is_empty() {
            let Some(tmp) = self.top() else {
                return Ok(self);
            };
            temp = HfstTransducer::new_from_transducer(&tmp.borrow());
        } else {
            match self.definitions_.get(name).cloned() {
                None => {
                    let _ = write!(oss, "no such definition '{}'\n", name);
                    self.flush();
                    self.prompt();
                    return Ok(self);
                }
                Some(it) => {
                    temp = HfstTransducer::new_from_transducer(&it.borrow());
                }
            }
        }

        match level {
            Level::UPPER_LEVEL => {
                temp.input_project()?;
            }
            Level::LOWER_LEVEL => {
                temp.output_project()?;
            }
            Level::BOTH_LEVELS => {}
        }

        let mut results = HfstTwoLevelPaths::new();

        let obey_flags_off = self.variables_["obey-flags"] == "OFF";
        let result = if obey_flags_off {
            temp.extract_paths(&mut results, number as i32, -1)
        } else {
            temp.extract_paths_fd(&mut results, number as i32, -1, true)
        };
        if let Err(e) = result {
            if matches!(e.kind, crate::error::ErrorKind::TransducerIsCyclic) {
                let cutoff = crate::hfst_data_types::size_t_to_uint(string_to_size_t(
                    &self.variables_["print-words-cycle-cutoff"],
                ));
                warn!(
                    "transducer is cyclic, limiting the number of cycles to {}",
                    cutoff
                );
                if obey_flags_off {
                    temp.extract_paths(&mut results, number as i32, cutoff as i32)?;
                } else {
                    temp.extract_paths_fd(&mut results, number as i32, cutoff as i32, true)?;
                }
            } else {
                return Err(e);
            }
        }

        self.print_paths_two(&results, oss, -1);

        self.prompt();
        return Ok(self);
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.print-alphabet-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.print-alphabet-fn]
    // @brief Print alphabet \a alpha to \a outfile. \a unknown and \a identity
    // define whether these symbols occur in the transitions of the transducer
    // whose alphabet we are printing.
    fn print_alphabet(
        &mut self,
        alpha: &StringSet,
        unknown: bool,
        identity: bool,
        oss: &mut dyn std::io::Write,
    ) {
        let mut sigma_count: u32 = 0;
        let _ = write!(oss, "Sigma: ");
        if self.variables_["print-foma-sigma"] == "ON" {
            if unknown {
                let _ = write!(oss, "?");
            }
            if identity {
                if unknown {
                    let _ = write!(oss, ", ");
                }
                let _ = write!(oss, "@");
            }
        } else
        // xfst-style sigma print
        {
            if unknown || identity {
                let _ = write!(oss, "?");
            }
        }

        let mut first_symbol = true;
        for it in alpha.iter() {
            if !is_special_symbol(it) {
                if !first_symbol || unknown || identity {
                    let _ = write!(oss, ", ");
                }
                if it == "?" {
                    let _ = write!(oss, "\"?\"");
                } else if it == "@" && self.variables_["print-foma-sigma"] == "ON" {
                    let _ = write!(oss, "\"@\"");
                } else {
                    let _ = write!(oss, "{}", it);
                }
                sigma_count += 1;
                first_symbol = false;
            }
        }
        let _ = write!(oss, "\n");
        let _ = write!(oss, "Size: {}.\n", sigma_count);
        self.flush();
    }
}

// Replacement for C system(3): run `command` through the shell and return its
// exit code (or -1 if it could not be launched), so the callers' `!= 0` checks
// keep working without libc.
fn run_shell(command: &str) -> i32 {
    match std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .status()
    {
        Ok(status) => status.code().unwrap_or(-1),
        Err(_) => -1,
    }
}

// [spec:hfst:def:xfst-compiler.hfst.xfst.is-special-symbol-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.is-special-symbol-fn]
fn is_special_symbol(s: &str) -> bool {
    if s == crate::hfst_symbol_defs::internal_epsilon
        || s == crate::hfst_symbol_defs::internal_unknown
        || s == crate::hfst_symbol_defs::internal_identity
    {
        return true;
    }
    return false;
}

// [spec:hfst:def:xfst-compiler.hfst.xfst.is-unknown-or-identity-used-in-transducer-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.is-unknown-or-identity-used-in-transducer-fn]
fn is_unknown_or_identity_used_in_transducer(
    t: &HfstTransducer,
    unknown: &mut bool,
    identity: &mut bool,
) -> bool {
    *unknown = false;
    *identity = false;

    let fsm = HfstBasicTransducer::new_from_transducer(t);
    for it in fsm.iter() {
        for tr_it in it.iter() {
            let istr = tr_it.get_input_symbol(fsm.coder());
            let ostr = tr_it.get_input_symbol(fsm.coder());
            if istr == crate::hfst_symbol_defs::internal_unknown
                || ostr == crate::hfst_symbol_defs::internal_unknown
            {
                *unknown = true;
            } else if istr == crate::hfst_symbol_defs::internal_identity
                || ostr == crate::hfst_symbol_defs::internal_identity
            // should not happen
            {
                *identity = true;
            } else {
                // ;
            }
            if *unknown == true && *identity == true {
                return true;
            }
        }
    }
    if *unknown == true || *identity == true {
        return true;
    } else {
        return false;
    }
}

// [spec:hfst:def:xfst-compiler.hfst.xfst.is-valid-string-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.is-valid-string-fn]
fn is_valid_string(sv: &crate::hfst_symbol_defs::StringVector) -> bool {
    use crate::hfst_flag_diacritics::FdOperation;
    // map features to latest values
    let mut values: BTreeMap<String, String> = BTreeMap::new();
    // and keep track of features whose values have been negatively set
    let mut negative_values: BTreeSet<String> = BTreeSet::new();

    for it in sv.iter() {
        if FdOperation::is_diacritic(it) {
            let opstr = FdOperation::get_operator(it);
            assert!(opstr.len() == 1);
            let op = opstr.as_bytes()[0] as char;
            let feat = FdOperation::get_feature(it);
            let val = FdOperation::get_value(it);

            let is_negatively_set = negative_values.contains(&feat);

            match op {
                'P' => {
                    // positive set
                    values.insert(feat.clone(), val.clone());
                }
                'N' => {
                    // negative set
                    values.insert(feat.clone(), val.clone());
                    negative_values.insert(feat.clone());
                }
                'R' => {
                    // require
                    if val.is_empty() {
                        // empty require
                        if values.get(&feat).map(|v| v.is_empty()).unwrap_or(true) {
                            return false;
                        } else {
                            // nonempty require
                            let current = values.get(&feat).cloned().unwrap_or_default();
                            if is_negatively_set || (current != val) {
                                return false;
                            }
                        }
                    }
                }
                'D' => {
                    // disallow
                    let current = values.get(&feat).cloned().unwrap_or_default();
                    if val.is_empty() {
                        // empty disallow
                        if current != "" {
                            return false;
                        }
                    } else {
                        if (!is_negatively_set) && (current == val) {
                            // nonempty disallow
                            return false;
                        }
                    }
                }
                'C' => {
                    // clear
                    values.insert(feat.clone(), String::new());
                }
                'U' => {
                    // unification
                    let current = values.get(&feat).cloned().unwrap_or_default();
                    if current.is_empty() // if the feature is unset or
                        || ((!is_negatively_set) && (current == val)) // the feature is at this value already or
                        || (is_negatively_set && (current != val))
                    // the feature is negatively set to something else
                    {
                        values.insert(feat.clone(), val.clone());
                    } else {
                        return false;
                    }
                }
                _ => {
                    error!("ERROR: line: {}", line!());
                    panic!(); // for the compiler's peace of mind
                }
            }
        }
    }
    return true;
}

// [spec:hfst:def:xfst-compiler.hfst.xfst.string-to-size-t-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.string-to-size-t-fn]
fn string_to_size_t(str_: &str) -> usize {
    // Mirror 'std::istringstream iss(str); size_t size; iss >> size;':
    // read the leading integer, defaulting to 0 if none is present.
    let trimmed = str_.trim_start();
    let digits: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<usize>().unwrap_or(0)
}

// Help-message support: 'xfst_help_message.h' (get_help_message and the
// HELP_MODE_* constants) was not ported, mirroring the C++ todo that 'helps
// have not been written or copied'. We reproduce the documented behaviour:
// no help is ever found.
const HELP_MODE_APROPOS: i32 = 0;
const HELP_MODE_ALL_COMMANDS: i32 = 1;
const HELP_MODE_ONE_COMMAND: i32 = 2;

fn get_help_message(_text: &str, _message: &mut String, _help_mode: i32) -> bool {
    false
}

// A side table mirroring the C++ file-static 'variable_explanations_' map,
// consulted by show_all. Populated lazily on first use.
fn variable_explanations_get(key: &str) -> String {
    let explanations: &[(&str, &str)] = &[
        (
            "assert",
            "quit the application if test result is 0 and quit-on-fail is ON",
        ),
        (
            "att-epsilon",
            "epsilon symbol used when reading from att files",
        ),
        ("char-encoding", "character encoding used"),
        ("copyright-owner", ""),
        ("directory", "<NOT IMPLEMENTED>"),
        ("encode-weights", "encode weights when minimizing"),
        (
            "flag-is-epsilon",
            "treat flag diacritics as epsilons in composition",
        ),
        (
            "harmonize-flags",
            "harmonize flag diacritics before composition",
        ),
        ("hopcroft-min", "use hopcroft's minimization algorithm"),
        (
            "lexc-minimize-flags",
            "if 'lexc-with-flags' == ON, minimize number of flags",
        ),
        (
            "lexc-rename-flags",
            "if 'lexc-minimize-flags' == ON, rename flags",
        ),
        (
            "lexc-with-flags",
            "use flags to hyperminimize result from lexc files",
        ),
        ("maximum-weight", "maximum weight of paths printed in apply"),
        ("minimal", "minimize networks after operations"),
        (
            "name-nets",
            "stores the name of the network when using 'define'",
        ),
        ("obey-flags", "obey flag diacritic constraints"),
        ("precision", "todo: precision to use when printing weights"),
        ("print-foma-sigma", "print identities as '@'"),
        ("print-pairs", "show both sides (upper and lower) of labels"),
        ("print-sigma", "show sigma when printing a network"),
        (
            "print-space",
            "insert a space between symbols when printing words",
        ),
        (
            "print-weight",
            "show weights when printing words or networks",
        ),
        (
            "quit-on-fail",
            "quit the application if a command cannot be executed",
        ),
        (
            "quote-special",
            "enclose special characters in double quotes",
        ),
        ("random-seed", "<EXPLANATION MISSING>"),
        ("recode-cp1252", "<NOT SUPPORTED>"),
        ("recursive-define", "<EXPLANATION MISSING>"),
        (
            "retokenize",
            "retokenize regular expressions in 'compile-replace'",
        ),
        ("show-flags", "show flag diacritics when printing"),
        ("sort-arcs", "<NOT IMPLEMENTED>"),
        ("use-timer", "<NOT IMPLEMENTED>"),
        ("verbose", "print more information"),
        (
            "xerox-composition",
            "treat flag diacritics as ordinary symbols in composition",
        ),
    ];
    for (k, v) in explanations.iter() {
        if *k == key {
            return v.to_string();
        }
    }
    String::new()
}

impl Default for XfstCompiler {
    fn default() -> Self {
        Self::new()
    }
}

const WEIGHT_PRECISION: &str = "5";
const LOOKUP_CYCLE_CUTOFF: &str = "5";
const PRINT_WORDS_CYCLE_CUTOFF: &str = "5";

// [spec:hfst:def:xfst-compiler.hfst.xfst.initialize-variable-explanations-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.initialize-variable-explanations-fn]
fn initialize_variable_explanations() {
    // The C++ free function populates a file-static variable_explanations_ map;
    // the port resolves explanations lazily via variable_explanations_get, so
    // there is nothing to initialize here.
}

// ===== integrated bodies 1-6 =====
fn strstrip(s: &str) -> String {
    let bytes = s.as_bytes();
    let is_space = |b: u8| matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r');
    let mut start = 0;
    while start < bytes.len() && is_space(bytes[start]) {
        start += 1;
    }
    let mut end = bytes.len();
    while end > start && is_space(bytes[end - 1]) {
        end -= 1;
    }
    String::from_utf8_lossy(&bytes[start..end]).into_owned()
}

// [spec:hfst:def:xfst-compiler.hfst.xfst.string-to-float-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.string-to-float-fn]
fn string_to_float(str_: &str) -> f32 {
    // Mirror 'std::istringstream >> float': skip leading whitespace, read the
    // leading numeric prefix, and yield 0 if nothing parses.
    let t = str_.trim_start();
    let bytes = t.as_bytes();
    let mut end = 0;
    if end < bytes.len() && (bytes[end] == b'+' || bytes[end] == b'-') {
        end += 1;
    }
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    if end < bytes.len() && bytes[end] == b'.' {
        end += 1;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
    }
    if end < bytes.len() && (bytes[end] == b'e' || bytes[end] == b'E') {
        let mut e = end + 1;
        if e < bytes.len() && (bytes[e] == b'+' || bytes[e] == b'-') {
            e += 1;
        }
        let mut saw = false;
        while e < bytes.len() && bytes[e].is_ascii_digit() {
            e += 1;
            saw = true;
        }
        if saw {
            end = e;
        }
    }
    t[..end].parse::<f32>().unwrap_or(0.0)
}

// [spec:hfst:def:xfst-compiler.hfst.xfst.extract-output-paths-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.extract-output-paths-fn]
fn extract_output_paths(paths: &HfstTwoLevelPaths) -> HfstOneLevelPaths {
    let mut retval = HfstOneLevelPaths::new();
    for it in paths.iter() {
        let mut new_path: Vec<String> = Vec::new();
        let path = &it.second;
        for p in path.iter() {
            if p.1 != "@0@" && p.1 != "@_EPSILON_SYMBOL_@" {
                if p.1 == "@_UNKNOWN_SYMBOL_@" {
                    new_path.push("?".to_string());
                } else {
                    new_path.push(p.1.clone());
                }
            }
        }
        retval.insert(crate::hfst_data_types::HfstOneLevelPath {
            first: it.first,
            second: new_path,
        });
    }
    retval
}

// [spec:hfst:def:xfst-compiler.hfst.xfst.to-filename-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.to-filename-fn]
fn to_filename(file: Option<&str>) -> &str {
    if file.is_none() {
        return "<stdin>";
    } else {
        return file.unwrap();
    }
}

// The following three helpers are guarded by '#ifdef FOO' in the C++ source
// (an undefined macro), so they are compiled out and unused there. They are
// ported 1:1 for completeness and stay unused here too.

// Convert 'str' to upper case.
// [spec:hfst:def:xfst-compiler.hfst.xfst.to-upper-case-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.to-upper-case-fn]
// [spec:hfst:def:xfst-help-message.hfst.xfst.to-upper-case-fn]
// [spec:hfst:sem:xfst-help-message.hfst.xfst.to-upper-case-fn]
#[allow(dead_code)]
fn to_upper_case(str_: &str) -> String {
    let str_bytes = str_.as_bytes();
    let mut retval = String::new();
    for i in 0..str_bytes.len() {
        if str_bytes[i] >= 97 && str_bytes[i] <= 122 {
            retval.push((str_bytes[i] - 32) as char);
        } else {
            retval.push(str_bytes[i] as char);
        }
    }
    return retval;
}

// Whether 'c' is allowed before or after a word when
// searching for the word in text.
// [spec:hfst:def:xfst-compiler.hfst.xfst.allow-char-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.allow-char-fn]
#[allow(dead_code)]
fn allow_char(c: u8) -> bool {
    let allowed_chars = b" \n\t.,;:?!-/'\"<>()|";
    for i in 0..allowed_chars.len() {
        if allowed_chars[i] == c {
            return true;
        }
    }
    return false;
}

// Whether word 'str_' is found in text 'text_'.
// Punctuation characters and upper/lower case are handled in this function.
// [spec:hfst:def:xfst-compiler.hfst.xfst.string-found-fn]
// [spec:hfst:sem:xfst-compiler.hfst.xfst.string-found-fn]
#[allow(dead_code)]
fn string_found(str_: &str, text_: &str) -> bool {
    let str_ = to_upper_case(str_);
    let text = to_upper_case(text_);
    let text_bytes = text.as_bytes();
    let pos = match text.find(&str_) {
        None => {
            return false;
        }
        Some(p) => p,
    };
    if pos == 0 || allow_char(text_bytes[pos - 1]) {
        if pos + str_.len() == text.len() || allow_char(text_bytes[pos + str_.len()]) {
            return true;
        }
    }
    return false;
}

impl XfstCompiler {
    // @brief Define alias for command sequence
    pub fn define_alias(&mut self, name: &str, commands: &str) -> &mut Self {
        self.aliases_.insert(name.to_string(), commands.to_string());
        self.prompt();
        self
    }

    // @brief Define list by range
    // @todo lists are not supported by HFST
    // @todo Unicode ranges are not supported
    pub fn define_list_by_range(&mut self, name: &str, start: &str, end: &str) -> &mut Self {
        if (start.len() > 1) || (end.len() > 1) {
            warn!("unsupported unicode range {}-{}", start, end);
        }
        let mut l: BTreeSet<String> = BTreeSet::new();
        let start_c = start.as_bytes().first().copied().unwrap_or(0);
        let end_c = end.as_bytes().first().copied().unwrap_or(0);
        let mut c = start_c;
        while c < end_c {
            let s = (c as char).to_string();
            l.insert(s);
            c += 1;
        }
        self.lists_.insert(name.to_string(), l);
        self
    }

    // @brief Define list by labels
    // @todo lists are not supportedd by HFST
    pub fn define_list(&mut self, name: &str, list: &str) -> &mut Self {
        if self.definitions_.contains_key(name) {
            error!(
                "Error: '{}' has already been defined as a transducer variable.",
                name
            );
            error!("It cannot have an incompatible definition as a list.");
            error!("Please undefine the definition first.");
            // MAYBE_QUIT
            if self.variables_["quit-on-fail"] == "ON" {
                self.fail_flag_ = true;
            }
            self.prompt();
            return self;
        }
        let mut l: BTreeSet<String> = BTreeSet::new();
        for token in list.split(' ') {
            if token.is_empty() {
                continue;
            }
            l.insert(token.to_string());
        }
        self.lists_.insert(name.to_string(), l.clone());
        self.xre_.define_list(name, &l); // XRE
        self.prompt();
        self
    }

    // @brief Define regex macro
    pub fn define_xre(&mut self, name: &str, xre: &str) -> &mut Self {
        // When calling this function, the regex 'indata' should already have
        // been compiled into a transducer which should have been stored to
        // the variable latest_regex_compiled.

        if self.lists_.contains_key(name) {
            error!(
                "Error: '{}' has already been defined as a list variable.",
                name
            );
            error!("It cannot have an incompatible definition as a transducer.");
            error!("Please undefine the variable first.");
            // MAYBE_QUIT
            if self.variables_["quit-on-fail"] == "ON" {
                self.fail_flag_ = true;
            }
            self.prompt();
            return self;
        }

        if self.latest_regex_compiled.is_some() {
            match self.xre_.compile(xre).map(|t| Rc::new(RefCell::new(t))) {
                Some(compiled) => {
                    self.define_transducer(name, compiled);
                    self.original_definitions_
                        .insert(name.to_string(), xre.to_string());
                }
                None => {
                    error!("Could not define variable '{}'", name);
                    self.xfst_fail();
                }
            }
        } else {
            error!("Could not define variable '{}'", name);
            self.xfst_fail();
        }
        self.prompt();
        self
    }

    // @brief Define regex macro
    pub fn define(&mut self, name: &str) -> &mut Self {
        // GET_TOP(top)
        let Some(top) = self.top() else {
            self.xfst_lesser_fail();
            return self;
        };
        self.stack_.pop();
        self.define_transducer(name, top);

        self.original_definitions_
            .insert(name.to_string(), "<net taken from stack>".to_string());
        self.prompt();
        self
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.define-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.define-fn]
    // @brief Define transducer
    pub fn define_transducer(&mut self, name: &str, transducer: NetRef) {
        let was_defined = self.xre_.is_definition(name);
        self.xre_.define_transducer(name, &transducer.borrow());
        if self.variables_["name-nets"] == "ON" {
            transducer.borrow_mut().set_name(name);
        }
        // overwriting drops the previous Rc.
        self.definitions_.remove(name);
        self.definitions_.insert(name.to_string(), transducer);

        if self.verbose_ {
            if was_defined {
                print!("Redefined");
            } else {
                print!("Defined");
            }
            println!(" '{}'", name);
        }
    }

    // @brief Define regex macro function
    // @todo Regex parser does not support macro functions
    pub fn define_function(&mut self, prototype: &str, xre: &str) -> &mut Self {
        let mut name = String::new();
        let mut arguments: Vec<String> = Vec::new();

        if !Self::extract_function_name(prototype, &mut name) {
            error!(
                "Error extracting function name from prototype '{}'",
                prototype
            );
            self.xfst_fail();
            self.prompt();
            return self;
        }

        if !Self::extract_function_arguments(prototype, &mut arguments) {
            error!(
                "Error extracting function arguments from prototype '{}'",
                prototype
            );
            self.xfst_fail();
            self.prompt();
            return self;
        }

        let xre_converted =
            Self::convert_argument_symbols(&arguments, xre, &name, &mut self.xre_, false);
        if xre_converted.is_empty() {
            error!("Error parsing function definition '{}'", xre);
            self.xfst_fail();
            self.prompt();
            return self;
        }

        let was_defined = self.xre_.is_function_definition(&name);

        if !self.xre_.define_function(
            &name,
            crate::hfst_data_types::size_t_to_uint(arguments.len()),
            &xre_converted,
        ) {
            // XRE
            error!("Error when defining function");
            self.xfst_fail();
            self.prompt();
            return self;
        }

        if self.verbose_ {
            if was_defined {
                print!("Redefined");
            } else {
                print!("Defined");
            }
            println!(" function '{}@{})", name, arguments.len() as i32);
        }

        self.function_arguments_.insert(
            name.clone(),
            crate::hfst_data_types::size_t_to_uint(arguments.len()),
        );
        let fdef = Self::convert_argument_symbols(&arguments, xre, "", &mut self.xre_, true);
        self.function_definitions_.insert(name.clone(), fdef);
        self.original_function_definitions_
            .insert(prototype.to_string(), xre.to_string());

        self.prompt();
        self
    }

    // @brief Remove definition
    pub fn undefine(&mut self, name_list: &str) -> &mut Self {
        for name in name_list.split(' ') {
            if name.is_empty() {
                continue;
            }
            if self.definitions_.remove(name).is_some() {
                self.xre_.undefine(name); // XRE
            }
        }
        self.prompt();
        self
    }

    // @brief Remove list
    // @todo HFST does not support lists
    pub fn unlist(&mut self, name: &str) -> &mut Self {
        if self.lists_.contains_key(name) {
            self.lists_.remove(name);
        }
        self.prompt();
        self
    }

    // @brief Load regex macros from file
    // @todo Definition names cannot be stored in HFST automata binaries
    pub fn load_definitions(&mut self, infilename: &str) -> &mut Self {
        // CHECK_FILENAME(infilename)
        if !self.check_filename(infilename) {
            return self;
        }
        self.load_stack_or_definitions(infilename, true /* definitions*/)
    }

    // @brief Clear stack
    pub fn clear(&mut self) -> &mut Self {
        while !self.stack_.is_empty() {
            self.stack_.pop();
        }
        if self.latest_regex_compiled.is_some() {
            self.latest_regex_compiled = None;
        }
        self.prompt();
        self
    }

    // @brief Pop stack
    pub fn pop(&mut self) -> &mut Self {
        if self.stack_.is_empty() {
            println!("Stack is empty.");
        } else {
            self.stack_.pop();
        }
        self.prompt();
        self
    }

    // @brief Push definition on stack
    pub fn push(&mut self, name: &str) -> crate::error::Result<&mut Self> {
        if !self.definitions_.contains_key(name) {
            println!("no such defined network: '{}'", name);
            self.prompt();
            return Ok(self);
        }

        let def = self.definitions_[name].clone();
        let t = Rc::new(RefCell::new(HfstTransducer::new_copy(&def.borrow())?));
        self.stack_.push(t);
        // PRINT_INFO_PROMPT_AND_RETURN_THIS
        self.print_transducer_info();
        self.prompt();
        Ok(self)
    }

    // @brief Push last definition on stack
    pub fn push_latest(&mut self) -> crate::error::Result<&mut Self> {
        let defs: Vec<NetRef> = self.definitions_.values().cloned().collect();
        for def in defs {
            let t = Rc::new(RefCell::new(HfstTransducer::new_copy(&def.borrow())?));
            self.stack_.push(t);
        }

        // PRINT_INFO_PROMPT_AND_RETURN_THIS
        self.print_transducer_info();
        self.prompt();
        Ok(self)
    }

    // @brief Reverse stack
    pub fn turn(&mut self) -> &mut Self {
        let mut tmp: std::collections::VecDeque<NetRef> = std::collections::VecDeque::new();
        while !self.stack_.is_empty() {
            tmp.push_back(self.stack_.pop().unwrap());
        }
        while !tmp.is_empty() {
            self.stack_.push(tmp.pop_front().unwrap());
        }
        // PRINT_INFO_PROMPT_AND_RETURN_THIS
        self.print_transducer_info();
        self.prompt();
        self
    }

    // @brief Move top of stack to bottom
    pub fn rotate(&mut self) -> &mut Self {
        if self.stack_.is_empty() {
            self.prompt();
            return self;
        }

        let mut tmp: Vec<NetRef> = Vec::new();
        while !self.stack_.is_empty() {
            tmp.push(self.stack_.pop().unwrap());
        }
        self.stack_ = tmp;

        // PRINT_INFO_PROMPT_AND_RETURN_THIS
        self.print_transducer_info();
        self.prompt();
        self
    }

    // @brief Load stack from file
    pub fn load_stack(&mut self, infilename: &str) -> &mut Self {
        // CHECK_FILENAME(infilename)
        if !self.check_filename(infilename) {
            return self;
        }
        self.load_stack_or_definitions(infilename, false)
    }

    pub fn compile_regex(&mut self, indata: &str, chars_read: &mut u32) -> &mut Self {
        if self.latest_regex_compiled.is_some() {
            self.latest_regex_compiled = None;
        }
        self.latest_regex_compiled = self
            .xre_
            .compile_first(indata, chars_read)
            .map(|t| Rc::new(RefCell::new(t))); // XRE
        self
    }

    // Store function name in 'prototype' to 'name'.
    // Return whether extraction succeeded.
    // 'prototype' must be of format "functionname(arg1, arg2, ... argN)"
    // [spec:hfst:def:xfst-compiler.hfst.xfst.extract-function-name-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.extract-function-name-fn]
    fn extract_function_name(prototype: &str, name: &mut String) -> bool {
        for ch in prototype.chars() {
            name.push(ch);
            if ch == '(' {
                return true;
            }
        }
        false // no starting parenthesis found
    }

    // Store names of function arguments in 'prototype' to 'args'.
    // Return whether extraction succeeded.
    // 'prototype' must be of format "functionname(arg1, arg2, ... argN)"
    // [spec:hfst:def:xfst-compiler.hfst.xfst.extract-function-arguments-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.extract-function-arguments-fn]
    fn extract_function_arguments(prototype: &str, args: &mut Vec<String>) -> bool {
        let p: Vec<char> = prototype.chars().collect();
        let at = |i: usize| -> char { p.get(i).copied().unwrap_or('\0') };

        // skip the function name
        let mut i: usize = 0;
        while at(i) != '(' {
            if at(i) == '\0' {
                return false; // function name ended too early
            }
            i += 1;
        }
        i += 1; // skip the "(" in function name

        // start scanning the argument list "arg1, arg2, ... argN )"
        let mut arg = String::new();
        while at(i) != ')' {
            if at(i) == '\0' {
                // no closing parenthesis found
                return false;
            } else if at(i) == ' ' {
                // skip whitespace
            } else if at(i) == ',' {
                // end of argument
                args.push(arg.clone());
                arg = String::new();
            } else {
                arg.push(at(i));
            }
            i += 1;
        }
        // last argument
        args.push(arg);

        true
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.convert-argument-symbols-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.convert-argument-symbols-fn]
    fn convert_argument_symbols(
        arguments: &[String],
        xre: &str,
        function_name: &str,
        xre_: &mut XreCompiler,
        user_friendly_argument_names: bool,
    ) -> String {
        let mut retval: String = xre.to_string();
        let mut arg_number: u32 = 1;

        for argument in arguments.iter() {
            let mut arg_positions: BTreeSet<u32> = BTreeSet::new();
            if !xre_.get_positions_of_symbol_in_xre(
                argument.as_str(),
                retval.as_str(),
                &mut arg_positions,
            ) {
                // XRE
                return String::new();
            }

            let substituting_argument: String = if user_friendly_argument_names {
                format!("ARGUMENT{}", arg_number)
            } else {
                format!("\"@{}{}@\"", function_name, arg_number)
            };

            let retval_bytes: Vec<u8> = retval.clone().into_bytes();
            let mut new_retval = String::new();

            // go through retval
            let mut i: u32 = 0;
            while (i as usize) < retval_bytes.len() {
                // argument to be replaced begins at this position
                if arg_positions.contains(&i) {
                    arg_positions.remove(&i); // case will not be handled again

                    new_retval.push_str(&substituting_argument);
                    // skip rest of the original symbol by advancing i to
                    // point to the last char in the original symbol
                    let mut offset: u32 = 1;
                    while offset < (argument.len() as u32) {
                        i += 1;
                        offset += 1;
                    }
                }
                // else, just copy
                else {
                    new_retval.push(retval_bytes[i as usize] as char);
                }
                i += 1;
            }

            retval = new_retval;
            arg_number += 1;
        }

        retval
    }

    // @brief Compose stack
    pub fn compose_net(&mut self) -> &mut Self {
        self.apply_binary_operation_iteratively(BinaryOperation::COMPOSE_NET)
    }

    // @brief concatenate stack
    pub fn concatenate_net(&mut self) -> &mut Self {
        self.apply_binary_operation_iteratively(BinaryOperation::CONCATENATE_NET)
    }

    // @brief Crossproduct top of stack
    pub fn crossproduct_net(&mut self) -> &mut Self {
        self.apply_binary_operation(BinaryOperation::CROSSPRODUCT_NET)
    }

    // @brief Ignore top of stack with second automaton
    pub fn ignore_net(&mut self) -> &mut Self {
        self.apply_binary_operation(BinaryOperation::IGNORE_NET)
    }

    // @brief Intersect stack
    pub fn intersect_net(&mut self) -> &mut Self {
        self.apply_binary_operation_iteratively(BinaryOperation::INTERSECT_NET)
    }

    // @brief Subtract second from top of stack
    pub fn minus_net(&mut self) -> &mut Self {
        self.apply_binary_operation(BinaryOperation::MINUS_NET)
    }

    // @brief Shuffle top network with second
    pub fn shuffle_net(&mut self) -> &mut Self {
        self.apply_binary_operation_iteratively(BinaryOperation::SHUFFLE_NET)
    }

    // @brief Disjunct the stack
    pub fn union_net(&mut self) -> &mut Self {
        self.apply_binary_operation_iteratively(BinaryOperation::UNION_NET)
    }

    // @brief Apply operation on two top transducers in the stack.
    // The top transducers are popped, the operation is applied
    // (the topmost transducer is the first transducer in the operation),
    // and the result is pushed to the top of the stack.
    // If the stack has less than two transducers, print a warning.
    fn apply_binary_operation(&mut self, operation: BinaryOperation) -> &mut Self {
        if self.stack_.len() < 2 {
            self.error_message("Not enough networks on stack. Operation requires at least 2.");
            self.flush();
            self.xfst_lesser_fail();
            return self;
        }
        let result = self.stack_.last().unwrap().clone();
        self.stack_.pop();
        let another = self.stack_.last().unwrap().clone();
        self.stack_.pop();
        let another_inner = another.borrow().clone();

        match operation {
            BinaryOperation::IGNORE_NET => {
                result.borrow_mut().insert_freely(&another_inner, true);
            }
            BinaryOperation::MINUS_NET => {
                result.borrow_mut().subtract(&another_inner, true);
            }
            BinaryOperation::CROSSPRODUCT_NET => {
                let __prev_hook = std::panic::take_hook();
                std::panic::set_hook(Box::new(|_| {}));
                let __res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    result.borrow_mut().cross_product(&another_inner, true);
                }));
                std::panic::set_hook(__prev_hook);
                if let Err(e) = __res {
                    if e.downcast_ref::<crate::error::Error>()
                        .filter(|__e| {
                            matches!(__e.kind, crate::error::ErrorKind::TransducersAreNotAutomata)
                        })
                        .is_some()
                    {
                        self.error_message("transducers are not automata");
                        self.flush();
                        self.xfst_fail();
                        self.stack_.push(another);
                        self.stack_.push(result);
                        self.prompt();
                        return self;
                    } else {
                        std::panic::resume_unwind(e);
                    }
                }
            }
            _ => {
                self.error_message("ERROR: unknown binary operation");
                self.flush();
                self.xfst_fail();
            }
        }

        result.borrow_mut().optimize();
        self.stack_.push(result);
        self.print_transducer_info();
        self.prompt();
        self
    }

    // @brief Apply operation on all transducers in the stack.
    // The top transducer (n1) is popped, the operation is applied iteratively
    // for all next transducers (n2, n3, n4 ...) in the stack:
    // [[[n1 OPERATION n2] OPERATION n3] OPERATION n4] ...
    // popping each of them and the result is pushed to the stack.
    // If the stack is empty, print a warning.
    fn apply_binary_operation_iteratively(&mut self, operation: BinaryOperation) -> &mut Self {
        if self.stack_.len() < 2 {
            self.error_message("Not enough networks on stack. Operation requires at least 2.");
            self.flush();
            self.xfst_lesser_fail();
            return self;
        }
        let result = self.stack_.last().unwrap().clone();

        self.stack_.pop();
        while !self.stack_.is_empty() {
            let t = self.stack_.last().unwrap().clone();

            let t_type = t.borrow().get_type();
            let result_type = result.borrow().get_type();
            if t_type != result_type {
                self.error_message("Stack contains transducers whose type differs.");
                self.flush();
                self.xfst_lesser_fail();
                break;
            }

            match operation {
                BinaryOperation::INTERSECT_NET => {
                    result.borrow_mut().intersect(&t.borrow(), true);
                }
                BinaryOperation::IGNORE_NET => {
                    result.borrow_mut().insert_freely(&t.borrow(), true);
                }
                BinaryOperation::COMPOSE_NET => {
                    let both_have_flags =
                        result.borrow().has_flag_diacritics() && t.borrow().has_flag_diacritics();
                    if both_have_flags {
                        if self.variables_["harmonize-flags"] == "OFF" {
                            if self.verbose_ {
                                self.error_message(
                                    "Both composition arguments contain flag \
                                     diacritics. Set harmonize-flags ON to \
                                     harmonize them.",
                                );
                                self.flush();
                            }
                        } else {
                            let mut rb = result.borrow_mut();
                            let mut tb = t.borrow_mut();
                            rb.harmonize_flag_diacritics(&mut tb, true);
                        }
                    }

                    let cfg = self.engine_config_;
                    let __prev_hook = std::panic::take_hook();
                    std::panic::set_hook(Box::new(|_| {}));
                    let __res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        result
                            .borrow_mut()
                            .compose_with_config(&t.borrow(), true, &cfg);
                    }));
                    std::panic::set_hook(__prev_hook);
                    if let Err(e) = __res {
                        if e.downcast_ref::<crate::error::Error>()
                            .filter(|__e| {
                                matches!(
                                    __e.kind,
                                    crate::error::ErrorKind::FlagDiacriticsAreNotIdentities
                                )
                            })
                            .is_some()
                        {
                            self.error_message(
                                "Error: flag diacritics must be identities in \
                                 composition if flag-is-epsilon is ON.\n\
                                 I.e. only FLAG:FLAG is allowed, not FLAG1:FLAG2, \
                                 FLAG:bar or foo:FLAG\n\
                                 Apply twosided flag-diacritics (tfd) before \
                                 composition.",
                            );
                            self.flush();
                            self.xfst_lesser_fail();
                            self.prompt();
                            return self;
                        } else {
                            std::panic::resume_unwind(e);
                        }
                    }
                }
                BinaryOperation::CONCATENATE_NET => {
                    result.borrow_mut().concatenate(&t.borrow(), true);
                }
                BinaryOperation::UNION_NET => {
                    result.borrow_mut().disjunct(&t.borrow(), true);
                }
                BinaryOperation::SHUFFLE_NET => {
                    result.borrow_mut().shuffle(&t.borrow(), true);
                }
                _ => {
                    self.error_message("ERROR: unknown binary operation");
                    self.flush();
                }
            }
            self.stack_.pop();
        }
        result.borrow_mut().optimize();
        self.stack_.push(result);
        self.print_transducer_info();
        self.prompt();
        self
    }

    // @brief Remove unnecessary symbols using ?
    // @todo HFST does not support ?
    pub fn compact_sigma(&mut self) -> &mut Self {
        let Some(top) = self.top() else {
            self.xfst_lesser_fail();
            return self;
        };
        top.borrow_mut().prune_alphabet(true);
        self.prompt();
        self
    }

    // @brief Eliminate flag diacritic
    // @todo unimplemented yet
    pub fn eliminate_flag(&mut self, name: &str) -> &mut Self {
        let Some(tmp) = self.top() else {
            self.xfst_lesser_fail();
            return self;
        };
        // [spec:hfst:def:xfst-compiler.hfst.xfst.name-fn]
        // [spec:hfst:sem:xfst-compiler.hfst.xfst.name-fn]
        let name_ = name.to_string();
        let __prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let __res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            tmp.borrow_mut().eliminate_flag(name);
        }));
        std::panic::set_hook(__prev_hook);
        if let Err(__e) = __res {
            let __name = match __e.downcast_ref::<crate::error::Error>() {
                Some(__ex) => __ex.message.clone().unwrap_or_default(),
                None => String::new(),
            };
            error!("could not eliminate flag '{}': {}", name, __name);
            if self.variables_["quit-on-fail"] == "ON" {
                self.fail_flag_ = true;
            }
        }
        let _ = name_;
        self.prompt();
        self
    }

    // @brief Eliminate all flag diacritics
    // @todo unimplemented yet
    pub fn eliminate_flags(&mut self) -> &mut Self {
        let Some(tmp) = self.top() else {
            self.xfst_lesser_fail();
            return self;
        };
        tmp.borrow_mut().eliminate_flags();
        self.prompt();
        self
    }

    pub fn twosided_flags(&mut self) -> &mut Self {
        let Some(tmp) = self.top() else {
            self.xfst_lesser_fail();
            return self;
        };
        tmp.borrow_mut().twosided_flag_diacritics();
        self.prompt();
        self
    }

    // @brief do some label pushing
    // @todo HFST automata cannot push labels
    pub fn cleanup_net(&mut self) -> &mut Self {
        warn!("cannot cleanup net");
        if self.stack_.len() < 1 {
            warn!("Empty stack.");
            self.xfst_lesser_fail();
            return self;
        }
        self.print_transducer_info();
        self.prompt();
        self
    }

    // @brief Make transducer functional
    // @todo unimplemented
    pub fn complete_net(&mut self) -> &mut Self {
        let Some(topmost) = self.top() else {
            self.xfst_lesser_fail();
            return self;
        };
        let mut fsm = HfstBasicTransducer::new_from_transducer(&topmost.borrow());
        fsm.complete();
        let topmost_type = topmost.borrow().get_type();
        let result = Rc::new(RefCell::new(HfstTransducer::from_basic_transducer(
            &fsm,
            topmost_type,
        )));
        self.stack_.pop();
        result.borrow_mut().optimize();
        self.stack_.push(result);
        self.print_transducer_info();
        self.prompt();
        self
    }

    // @brief Determinize top of stack
    pub fn determinize_net(&mut self) -> &mut Self {
        self.apply_unary_operation(UnaryOperation::DETERMINIZE_NET)
    }

    // @brief Remove epsilons from top of stack
    pub fn epsilon_remove_net(&mut self) -> &mut Self {
        self.apply_unary_operation(UnaryOperation::EPSILON_REMOVE_NET)
    }

    // @brief invert top of stack
    pub fn invert_net(&mut self) -> &mut Self {
        self.apply_unary_operation(UnaryOperation::INVERT_NET)
    }

    // @brief Make top of stack label network
    // @todo Find out wtf this is
    pub fn label_net(&mut self) -> crate::error::Result<&mut Self> {
        let Some(topmost) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };
        let topmost_type = topmost.borrow().get_type();
        let result = Rc::new(RefCell::new(HfstTransducer::new_type(topmost_type)?));
        let mut label_set: BTreeSet<(String, String)> = BTreeSet::new();
        let fsm = HfstBasicTransducer::new_from_transducer(&topmost.borrow());
        for it in fsm.iter() {
            for tr_it in it.iter() {
                label_set.insert((
                    tr_it.get_input_symbol(fsm.coder()),
                    tr_it.get_output_symbol(fsm.coder()),
                ));
            }
        }
        let result_type = result.borrow().get_type();
        for it in label_set.iter() {
            let label_tr = HfstTransducer::new_symbol_pair(&it.0, &it.1, result_type)?;
            result.borrow_mut().disjunct(&label_tr, true)?;
        }
        result.borrow_mut().minimize()?;
        self.stack_.pop();
        self.stack_.push(result);
        self.print_transducer_info();
        self.prompt();
        Ok(self)
    }

    // @brief Project input for top of stack
    pub fn lower_side_net(&mut self) -> &mut Self {
        self.apply_unary_operation(UnaryOperation::LOWER_SIDE_NET)
    }

    // @brief Project output for top of stack
    pub fn upper_side_net(&mut self) -> &mut Self {
        self.apply_unary_operation(UnaryOperation::UPPER_SIDE_NET)
    }

    // @brief Minimize top of stack
    pub fn minimize_net(&mut self) -> &mut Self {
        self.apply_unary_operation(UnaryOperation::MINIMIZE_NET)
    }

    // @brief Negate top of stack
    pub fn negate_net(&mut self) -> &mut Self {
        if self.stack_.len() < 1 {
            warn!("Empty stack.");
            self.xfst_lesser_fail();
            return self;
        }

        let t = self.stack_.last().unwrap().clone();
        let t_op = t.clone();

        let __prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let __res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            t_op.borrow_mut().negate();
        }));
        std::panic::set_hook(__prev_hook);
        if let Err(__e) = __res {
            if __e
                .downcast_ref::<crate::error::Error>()
                .filter(|__e| matches!(__e.kind, crate::error::ErrorKind::TransducerIsNotAutomaton))
                .is_some()
            {
                error!("Error: Negation is defined only for automata.");
                error!(
                    "Use expression [[?:?]* - A] instead where A is the transducer to be negated."
                );
                self.xfst_lesser_fail();
                return self;
            } else {
                std::panic::resume_unwind(__e);
            }
        }

        t.borrow_mut().optimize();
        self.print_transducer_info();
        self.prompt();
        self
    }

    // @brief Kleene plus top network of stack
    pub fn one_plus_net(&mut self) -> &mut Self {
        self.apply_unary_operation(UnaryOperation::ONE_PLUS_NET)
    }

    // @brief Kleene star top network of stack
    pub fn zero_plus_net(&mut self) -> &mut Self {
        self.apply_unary_operation(UnaryOperation::ZERO_PLUS_NET)
    }

    // @brief Prune top network of stack
    // @todo Most of HFST automata are pruned by default?
    pub fn prune_net(&mut self) -> &mut Self {
        self.apply_unary_operation(UnaryOperation::PRUNE_NET_)
    }

    // @brief Reverse top network of the stack
    pub fn reverse_net(&mut self) -> &mut Self {
        self.apply_unary_operation(UnaryOperation::REVERSE_NET)
    }

    // @brief Sigma top network of stack
    // @todo Find out wtf this is
    pub fn sigma_net(&mut self) -> crate::error::Result<&mut Self> {
        let Some(tmp) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };
        let mut alpha: StringSet = tmp.borrow().get_alphabet()?;
        alpha.remove("@_UNKNOWN_SYMBOL_@");
        alpha.remove("@_IDENTITY_SYMBOL_@");
        alpha.remove("@_EPSILON_SYMBOL_@");
        let alpha_ = crate::hfst_symbol_defs::symbols::to_string_pair_set(&alpha);
        let sigma = Rc::new(RefCell::new(HfstTransducer::new_string_pair_set(
            &alpha_,
            self.format_,
            false,
        )?));
        sigma.borrow_mut().optimize()?;
        self.stack_.push(sigma);
        self.print_transducer_info();
        self.prompt();
        Ok(self)
    }

    // @brief Interactive network traversal tool
    pub fn inspect_net(&mut self) -> crate::error::Result<&mut Self> {
        let Some(t) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };

        let net = HfstBasicTransducer::new_from_transducer(&t.borrow());

        const INSPECT_NET_HELP_MSG: &str =
            "'N' transits arc N, '-N' returns to level N, '<' to previous level, '0' quits.\n";
        print!("{}", INSPECT_NET_HELP_MSG);

        // path of states visited, can contain loops
        let mut whole_path: Vec<u32> = Vec::new();
        // shortest path of states to current state, no loops
        let mut shortest_path: Vec<u32> = Vec::new();

        Self::append_state_to_paths(&mut whole_path, &mut shortest_path, 0);
        self.print_level(&whole_path, &shortest_path);

        if net.is_final_state(0) {
            print!(" (final)");
        }

        println!();

        // transitions of current state
        let mut transitions: HfstBasicTransitions = net.index(0)?.clone();
        // number of arcs in current state
        let mut number_of_arcs = self.print_transitions(&transitions, net.coder());

        // index after which the history added during inspect_net is ignored
        let ind = self.current_history_index();

        // the while loop begins, keep on reading from user
        loop {
            let line = match self.xfst_getline("") {
                Some(l) => l,
                None => break,
            };
            // case (1): back to previous state
            if line == "<\n" || line == "<" {
                if whole_path.len() < 2 {
                    self.ignore_history_after_index(ind);
                    self.prompt();
                    return Ok(self);
                } else {
                    let __lvl = (whole_path.len() - 1) as u32;
                    if !Self::return_to_level(&mut whole_path, &mut shortest_path, __lvl) {
                        error!("FATAL ERROR: could not return to level '{}'", __lvl as i32);
                        self.ignore_history_after_index(ind);
                        self.prompt();
                        return Ok(self);
                    }
                }
            }
            // case (2): back to state number N
            else if line.as_bytes().first() == Some(&b'-') {
                let level = Self::atoi(&line[1..]); // skip '-'
                if !self.can_level_be_reached(level, whole_path.len()) {
                    continue;
                } else if !Self::return_to_level(&mut whole_path, &mut shortest_path, level as u32)
                {
                    error!("FATAL ERROR: could not return to level '{}'", level);
                    self.ignore_history_after_index(ind);
                    self.prompt();
                    return Ok(self);
                }
            }
            // case (3): exit program
            else if line == "0\n" || line == "0" {
                self.ignore_history_after_index(ind);
                self.prompt();
                return Ok(self);
            }
            // case (4): follow arc
            else {
                let number = Self::atoi(&line); // FIX: atoi is not portable
                if !self.can_transition_be_followed(number, number_of_arcs) {
                    continue;
                } else {
                    let tr = transitions[(number - 1) as usize].clone();
                    print!(
                        "  {}:{} --> ",
                        tr.get_input_symbol(net.coder()),
                        tr.get_output_symbol(net.coder())
                    );
                    Self::append_state_to_paths(
                        &mut whole_path,
                        &mut shortest_path,
                        tr.get_target_state(),
                    );
                }
            }

            // update transitions and number of arcs and print information about
            // current level
            transitions = net
                .index(*whole_path.last().expect("path is non-empty"))?
                .clone();
            self.print_level(&whole_path, &shortest_path);
            if net.is_final_state(*whole_path.last().unwrap()) {
                print!(" (final)");
            }
            println!();
            number_of_arcs = self.print_transitions(&transitions, net.coder());
        } // end of while loop

        self.ignore_history_after_index(ind);
        self.prompt();
        Ok(self)
    }

    // @brief Repeat 0..1 times
    pub fn optional_net(&mut self) -> &mut Self {
        self.apply_unary_operation(UnaryOperation::OPTIONAL_NET)
    }

    // internal function
    pub fn compile_replace_net(&mut self, level: Level) -> crate::error::Result<&mut Self> {
        assert!(level != Level::BOTH_LEVELS);

        let Some(tmp) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };
        let mut tmp_cp = HfstTransducer::new_copy(&tmp.borrow())?;

        if level == Level::UPPER_LEVEL {
            tmp_cp.input_project()?;
        } else {
            // LOWER_LEVEL
            tmp_cp.output_project()?;
        }

        if Self::is_well_formed_for_compile_replace(&tmp_cp, &mut self.xre_)? {
            if self.verbose_ {
                debug!("Network is well-formed.");
            }
        } else {
            if self.verbose_ {
                debug!("Network is not well-formed.");
            }
            self.xfst_lesser_fail();
            self.prompt();
            return Ok(self);
        }

        let level_is_upper = level == Level::UPPER_LEVEL;
        let level_not_upper = level != Level::UPPER_LEVEL;
        let retokenize_on = self.variables_["retokenize"] == "ON";

        let mut fsm = HfstBasicTransducer::new_from_transducer(&tmp.borrow());
        let mut early_return = false;
        {
            let xre_ptr: *mut XreCompiler = &mut self.xre_;
            let __prev_hook = std::panic::take_hook();
            std::panic::set_hook(Box::new(|_| {}));
            let __res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let replacement_map = fsm.find_replacements(level_is_upper);

                for (start_state, replacements) in replacement_map.iter() {
                    for (end_state, sp) in replacements.iter() {
                        let regexp = Self::to_regexp(sp, level_is_upper, retokenize_on);
                        let literal_regexp = Self::to_literal_regexp(sp, level_not_upper);

                        let mut cross_product_regexp = String::from("[ ");
                        if level_is_upper {
                            cross_product_regexp.push_str(&regexp);
                            cross_product_regexp.push_str(" ] .x. [ ");
                            cross_product_regexp.push_str(&literal_regexp);
                            cross_product_regexp.push_str(" ]");
                        } else {
                            cross_product_regexp.push_str(&literal_regexp);
                            cross_product_regexp.push_str(" ] .x. [ ");
                            cross_product_regexp.push_str(&regexp);
                            cross_product_regexp.push_str(" ]");
                        }

                        let Some(mut replacement) =
                            (unsafe { (*xre_ptr).compile(&cross_product_regexp) })
                        else {
                            error!(
                                "Could not compile regular expression in compile-replace: {}.",
                                cross_product_regexp
                            );
                            early_return = true;
                            return;
                        };

                        let _ = replacement.optimize();
                        let repl = HfstBasicTransducer::new_from_transducer(&replacement);
                        fsm.insert_transducer(*start_state, *end_state, &repl);
                    }
                }
            }));
            std::panic::set_hook(__prev_hook);
            if let Err(__e) = __res {
                let __msg: String = if let Some(s) = __e.downcast_ref::<&str>() {
                    (*s).to_string()
                } else if let Some(s) = __e.downcast_ref::<String>() {
                    s.clone()
                } else {
                    String::new()
                };
                error!("compile_replace threw an error: '{}'", __msg);
            }
        }

        if early_return {
            self.xfst_lesser_fail();
            self.prompt();
            return Ok(self);
        }

        let result = Rc::new(RefCell::new(HfstTransducer::from_basic_transducer(
            &fsm,
            self.format_,
        )));

        // filter out regexps
        let mut cr = Self::contains_regexp_markers_on_one_side(&mut self.xre_, level_is_upper);
        cr.optimize()?;

        result.borrow_mut().subtract(&cr, true)?.optimize()?;
        result
            .borrow_mut()
            .substitute("@EPSILON_MARKER@", "@_EPSILON_SYMBOL_@", true, true)?;
        self.stack_.pop();
        self.stack_.push(result);

        self.prompt();
        Ok(self)
    }

    // @brief Compile-replace lower
    pub fn compile_replace_lower_net(&mut self) -> crate::error::Result<&mut Self> {
        self.compile_replace_net(Level::LOWER_LEVEL)
    }

    // @brief Compile-replace upper
    pub fn compile_replace_upper_net(&mut self) -> crate::error::Result<&mut Self> {
        self.compile_replace_net(Level::UPPER_LEVEL)
    }

    // @brief Apply \a operation on top transducer in the stack.
    // If the stack is empty, print a warning.
    fn apply_unary_operation(&mut self, operation: UnaryOperation) -> &mut Self {
        let Some(result) = self.top() else {
            self.xfst_lesser_fail();
            return self;
        };
        self.stack_.pop();
        let result_op = result.clone();

        match operation {
            UnaryOperation::DETERMINIZE_NET => {
                result_op.borrow_mut().determinize();
            }
            UnaryOperation::EPSILON_REMOVE_NET => {
                result_op.borrow_mut().remove_epsilons();
            }
            UnaryOperation::INVERT_NET => {
                result_op.borrow_mut().invert();
            }
            UnaryOperation::LOWER_SIDE_NET => {
                result_op.borrow_mut().output_project();
            }
            UnaryOperation::UPPER_SIDE_NET => {
                result_op.borrow_mut().input_project();
            }
            UnaryOperation::ZERO_PLUS_NET => {
                result_op.borrow_mut().repeat_star();
            }
            UnaryOperation::ONE_PLUS_NET => {
                result_op.borrow_mut().repeat_plus();
            }
            UnaryOperation::OPTIONAL_NET => {
                result_op.borrow_mut().optionalize();
            }
            UnaryOperation::REVERSE_NET => {
                result_op.borrow_mut().reverse();
            }
            UnaryOperation::MINIMIZE_NET => {
                // implicit minimization requested, do not use optimize()
                result_op.borrow_mut().minimize();
            }
            UnaryOperation::PRUNE_NET_ => {
                result_op.borrow_mut().prune();
            }
        }

        if operation != UnaryOperation::MINIMIZE_NET
            && operation != UnaryOperation::DETERMINIZE_NET
            && operation != UnaryOperation::EPSILON_REMOVE_NET
        {
            result_op.borrow_mut().optimize();
        }
        self.stack_.push(result);
        self.print_transducer_info();

        self.prompt();
        self
    }

    // For 'inspect_net': append state \a state to paths.
    // [spec:hfst:def:xfst-compiler.hfst.xfst.append-state-to-paths-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.append-state-to-paths-fn]
    fn append_state_to_paths(whole_path: &mut Vec<u32>, shortest_path: &mut Vec<u32>, state: u32) {
        whole_path.push(state);
        let mut idx: Option<usize> = None;
        for (i, it) in shortest_path.iter().enumerate() {
            if *it == state {
                idx = Some(i);
                break;
            }
        }
        if let Some(i) = idx {
            shortest_path.truncate(i);
        }
        shortest_path.push(state);
    }

    // For 'inspect_net': return to level \a level.
    // Return whether the operation succeeded.
    // [spec:hfst:def:xfst-compiler.hfst.xfst.return-to-level-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.return-to-level-fn]
    fn return_to_level(
        whole_path: &mut Vec<u32>,
        shortest_path: &mut Vec<u32>,
        level: u32,
    ) -> bool {
        if (whole_path.len() as u32) < level || level == 0 {
            return false;
        }

        whole_path.truncate(level as usize);
        let state = *whole_path.last().unwrap();
        let mut idx: Option<usize> = None;
        for (i, it) in shortest_path.iter().enumerate() {
            if *it == state {
                idx = Some(i);
                break;
            }
        }
        if let Some(i) = idx {
            shortest_path.truncate(i);
        }
        shortest_path.push(state);
        true
    }

    // A C-style atoi used by 'inspect_net' to parse user input.
    fn atoi(s: &str) -> i32 {
        let bytes = s.as_bytes();
        let mut i = 0usize;
        while i < bytes.len()
            && (bytes[i] == b' '
                || bytes[i] == b'\t'
                || bytes[i] == b'\n'
                || bytes[i] == b'\r'
                || bytes[i] == 0x0b
                || bytes[i] == 0x0c)
        {
            i += 1;
        }
        let mut sign: i32 = 1;
        if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
            if bytes[i] == b'-' {
                sign = -1;
            }
            i += 1;
        }
        let mut result: i32 = 0;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            result = result
                .wrapping_mul(10)
                .wrapping_add((bytes[i] - b'0') as i32);
            i += 1;
        }
        sign * result
    }

    // Returns an automaton that contains one ore more "^[" "^]" expressions.
    // [spec:hfst:def:xfst-compiler.hfst.xfst.contains-regexps-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.contains-regexps-fn]
    fn contains_regexps(xre_: &mut XreCompiler) -> HfstTransducer {
        let not_bracket_star = xre_.compile("[? - \"^[\" - \"^]\"]* ;").unwrap(); // XRE
        xre_.define_transducer("TempNotBracketStar", &not_bracket_star); // XRE
        // all paths that contain one or more well-formed ^[ ^] expressions
        let well_formed = xre_
            .compile(
                "TempNotBracketStar \"^[\" TempNotBracketStar  [ \"^]\" TempNotBracketStar \"^[\"  TempNotBracketStar ]*  \"^]\" TempNotBracketStar ;",
            )
            .unwrap();
        xre_.undefine("TempNotBracketStar");
        well_formed
    }

    // XRE
    // [spec:hfst:def:xfst-compiler.hfst.xfst.contains-regexp-markers-on-one-side-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.contains-regexp-markers-on-one-side-fn]
    fn contains_regexp_markers_on_one_side(
        xre_: &mut XreCompiler,
        input_side: bool,
    ) -> HfstTransducer {
        if input_side {
            xre_.compile(
                "[?:?|0:?|?:0]* [\"^[\":? | \"^]\":? | \"^[\":0 | \"^]\":0] [?:?|0:?|?:0]*",
            )
        } else {
            // output side
            xre_.compile(
                "[?:?|0:?|?:0]* [?:\"^[\" | ?:\"^]\" | 0:\"^[\" | 0:\"^]\"] [?:?|0:?|?:0]*",
            )
        }
        .unwrap()
    }

    // @pre \a t must be an automaton  XRE
    // [spec:hfst:def:xfst-compiler.hfst.xfst.is-well-formed-for-compile-replace-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.is-well-formed-for-compile-replace-fn]
    fn is_well_formed_for_compile_replace(
        t: &HfstTransducer,
        xre_: &mut XreCompiler,
    ) -> crate::error::Result<bool> {
        let well_formed = Self::contains_regexps(xre_);
        // subtract those paths from copy of t
        let mut tc = HfstTransducer::new_copy(t)?;
        tc.subtract(&well_formed, true)?;
        // all paths that contain one or more ^[ or ^]
        let brackets = xre_.compile("$[ \"^[\" | \"^]\" ] ;").unwrap();

        // test if the result is empty
        tc.intersect(&brackets, true)?;
        let empty = HfstTransducer::new_type(tc.get_type())?;
        let value = empty.compare(&tc, false)?;
        Ok(value)
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.to-literal-regexp-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.to-literal-regexp-fn]
    fn to_literal_regexp(path: &Vec<(String, String)>, input_side: bool) -> String {
        let mut pathstr = String::from("[");
        for it in path.iter() {
            let symbol = if input_side { &it.0 } else { &it.1 };
            if symbol.as_str() != crate::hfst_symbol_defs::internal_epsilon {
                pathstr.push_str("\"");
                pathstr.push_str(symbol);
                pathstr.push_str("\" ");
            }
        }
        pathstr.push_str("]");
        if pathstr == "[]" {
            pathstr = String::from("[0]");
        }
        pathstr
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.to-regexp-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.to-regexp-fn]
    fn to_regexp(path: &Vec<(String, String)>, input_side: bool, retokenize: bool) -> String {
        let mut pathstr = String::from("[");
        for it in path.iter() {
            let symbol = if input_side { &it.0 } else { &it.1 };
            // ignore "^[" and "^]"
            if symbol.as_str() != "^]" && symbol.as_str() != "^[" {
                if symbol.as_str() != crate::hfst_symbol_defs::internal_epsilon {
                    pathstr.push_str(symbol);
                    if !retokenize {
                        pathstr.push_str(" ");
                    }
                }
            } else {
                // For better alignment
                pathstr.push_str("\"@EPSILON_MARKER@\"");
                if !retokenize {
                    pathstr.push_str(" ");
                }
            }
        }
        pathstr.push_str("]");
        if pathstr == "[]" {
            pathstr = String::from("[0]");
        }
        pathstr
    }

    // @brief Perform lookdowns on top of the stack, one per line
    // @todo lookdown is missing from HFST
    pub fn apply_up(&mut self, indata: &str) -> crate::error::Result<&mut Self> {
        // strtok splits on '\n' and skips empty tokens.
        for line in indata.split('\n').filter(|s| !s.is_empty()) {
            if line == APPLY_END_STRING {
                break;
            }
            self.apply_up_line(line)?;
        }
        self.prompt();
        Ok(self)
    }

    // @brief Perform lookups on top of the stack, one per line
    // @todo lookup is missing from HFST
    pub fn apply_down(&mut self, indata: &str) -> crate::error::Result<&mut Self> {
        // strtok splits on '\n' and skips empty tokens.
        for line in indata.split('\n').filter(|s| !s.is_empty()) {
            if line == APPLY_END_STRING {
                break;
            }
            self.apply_down_line(line)?;
        }
        self.prompt();
        Ok(self)
    }

    // @brief Perform lookmeds on top of the stack, one per line
    // @todo lookmed is missing from HFST
    pub fn apply_med(&mut self, indata: &str) -> &mut Self {
        // strtok splits on '\n' and skips empty tokens.
        for line in indata.split('\n').filter(|s| !s.is_empty()) {
            self.apply_med_line(line);
        }
        self.prompt();
        self
    }

    pub fn lookup_optimize(&mut self) -> &mut Self {
        if self.stack_.len() < 1 {
            // EMPTY_STACK
            warn!("Empty stack.");
            self.xfst_lesser_fail();
            self.prompt();
            return self;
        }

        let t = self.stack_.last().unwrap().clone();
        let t_type = t.borrow().get_type();

        let to_format: ImplementationType;
        if t_type == ImplementationType::HFST_OL_TYPE || t_type == ImplementationType::HFST_OLW_TYPE
        {
            info!("Network is already optimized for lookup.");
            self.prompt();
            return self;
        } else if t_type == ImplementationType::TROPICAL_OPENFST_TYPE
            || t_type == ImplementationType::LOG_OPENFST_TYPE
        {
            to_format = ImplementationType::HFST_OLW_TYPE;
        } else {
            to_format = ImplementationType::HFST_OL_TYPE;
        }

        if self.verbose_ {
            debug!(
                "converting transducer type from {} to {}, this might take a while...",
                crate::hfst_data_types::implementation_type_to_format(t_type),
                crate::hfst_data_types::implementation_type_to_format(to_format)
            );
        }

        let mut temp: Vec<NetRef> = Vec::new();
        while !self.stack_.is_empty() {
            let top = self.stack_.last().unwrap().clone();
            top.borrow_mut().convert(to_format, String::new());
            temp.push(top);
            self.stack_.pop();
        }
        while !temp.is_empty() {
            self.stack_.push(temp.last().unwrap().clone());
            temp.pop();
        }

        self.prompt();
        self
    }

    pub fn remove_optimization(&mut self) -> &mut Self {
        if self.stack_.len() < 1 {
            // EMPTY_STACK
            warn!("Empty stack.");
            self.xfst_lesser_fail();
            self.prompt();
            return self;
        }
        let t = self.stack_.last().unwrap().clone();
        let t_type = t.borrow().get_type();

        if t_type != ImplementationType::HFST_OL_TYPE && t_type != ImplementationType::HFST_OLW_TYPE
        {
            info!("Network is already in ordinary format.");
            self.prompt();
            return self;
        }

        if self.verbose_ {
            debug!(
                "converting transducer type from {} to {}, this might take a while...",
                crate::hfst_data_types::implementation_type_to_format(t_type),
                crate::hfst_data_types::implementation_type_to_format(self.format_)
            );
            if !HfstTransducer::is_safe_conversion(t_type, self.format_) {
                warn!(
                    "converting from weighted to unweighted format, loss of information is possible"
                );
            }
        }

        let mut temp: Vec<NetRef> = Vec::new();
        while !self.stack_.is_empty() {
            let top = self.stack_.last().unwrap().clone();
            top.borrow_mut().convert(self.format_, String::new());
            temp.push(top);
            self.stack_.pop();
        }
        while !temp.is_empty() {
            self.stack_.push(temp.last().unwrap().clone());
            temp.pop();
        }

        self.prompt();
        self
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.get-apply-prompt-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.get-apply-prompt-fn]
    // @brief Get the prompt that is used when applying up or down
    // (as specified by \a direction).
    fn get_apply_prompt(&mut self, direction: ApplyDirection) -> String {
        if !self.verbose_ {
            return String::new();
        }
        if direction == ApplyDirection::APPLY_UP_DIRECTION {
            return "apply up> ".to_string();
        } else if direction == ApplyDirection::APPLY_DOWN_DIRECTION {
            return "apply down> ".to_string();
        }
        String::new()
    }

    // @brief Perform lookup on the top transducer using strings in \a infile.
    // \a direction specifies whether apply is done on input (up) or output (down)
    // side. The results are printed to standard output.
    fn apply(
        &mut self,
        indata: &str,
        direction: ApplyDirection,
    ) -> crate::error::Result<&mut Self> {
        // The C++ overload read lines from a FILE*; here lines come from stdin
        // via 'xfst_getline' so the 'indata' source is unused.
        let _ = indata;

        if self.stack_.len() < 1 {
            // EMPTY_STACK
            warn!("Empty stack.");
            self.xfst_lesser_fail();
            self.prompt();
            return Ok(self);
        }
        let top = self.stack_.last().unwrap().clone();
        // number of cycles needs to be limited for an infinitely ambiguous ol
        // transducer because it doesn't support
        // is_lookup_infinitely_ambiguous(const string &)
        let mut ol_cutoff: usize = string_to_size_t(&self.variables_["lookup-cycle-cutoff"]);

        // Owned inverted copy for apply-up; None means operate on the shared top.
        let mut owned_t: Option<HfstTransducer> = None;
        // Basic transducer used for ordinary (non-OL) lookups.
        let mut fsm: Option<HfstBasicTransducer> = None;

        if direction == ApplyDirection::APPLY_UP_DIRECTION {
            let ty = top.borrow().get_type();
            if ty == ImplementationType::HFST_OL_TYPE || ty == ImplementationType::HFST_OLW_TYPE {
                warn!(
                    "Operation not supported for optimized lookup format. Consider 'remove-optimization' to convert into ordinary format."
                );
                self.prompt();
                return Ok(self);
            }

            // lookdown not yet implemented in HFST
            if self.verbose_ {
                warn!(
                    "apply up not implemented, inverting transducer and performing apply down\nfor faster performance, invert and minimize top network and do apply down instead"
                );
            }
            let mut c = HfstTransducer::new_copy(&top.borrow())?;
            // the user has been warned for possible slow performance
            c.invert()?.minimize()?;
            owned_t = Some(c);
        }

        let work_type = match &owned_t {
            Some(c) => c.get_type(),
            None => top.borrow().get_type(),
        };

        if work_type != ImplementationType::HFST_OL_TYPE
            && work_type != ImplementationType::HFST_OLW_TYPE
        {
            fsm = Some(match &owned_t {
                Some(c) => HfstBasicTransducer::new_from_transducer(c),
                None => HfstBasicTransducer::new_from_transducer(&top.borrow()),
            });
        } else {
            // this gets ignored by ol transducer's
            // is_lookup_infinitely_ambiguous
            let foo: Vec<String> = Vec::new();
            let inf = match &owned_t {
                Some(c) => c.is_lookup_infinitely_ambiguous_string_vector(&foo),
                None => top
                    .borrow()
                    .is_lookup_infinitely_ambiguous_string_vector(&foo),
            };
            if inf {
                ol_cutoff = string_to_size_t(&self.variables_["lookup-cycle-cutoff"]);
                if self.verbose_ {
                    warn!(
                        "transducer is infinitely ambiguous, limiting number of cycles to {}",
                        ol_cutoff
                    );
                }
            }
        }

        // prompt is printed only when reading from the user (always stdin here)
        let promptstr: String = if self.verbose_ {
            self.get_apply_prompt(direction)
        } else {
            String::new()
        };

        let ind = self.current_history_index(); // readline history to return to

        // get lines from stdin..
        loop {
            let line_opt = self.xfst_getline(&promptstr);
            // .. until end of file...
            match line_opt {
                None => {
                    // the next command must start on a fresh line
                    println!();
                    break;
                }
                Some(line) => {
                    let line = self.remove_newline(line);
                    // .. or until special end string
                    if line == APPLY_END_STRING {
                        break;
                    }
                    // perform lookup/lookdown
                    if let Some(fsm_ref) = fsm.as_ref() {
                        self.lookup_basic(&line, fsm_ref);
                    } else {
                        match &owned_t {
                            Some(c) => {
                                self.lookup(&line, c, ol_cutoff)?;
                            }
                            None => {
                                let tref = top.borrow();
                                self.lookup(&line, &tref, ol_cutoff)?;
                            }
                        }
                    }
                }
            }
        }

        // ignore all readline history given to the apply command
        self.ignore_history_after_index(ind);

        self.prompt();
        Ok(self)
    }

    fn lookup(
        &mut self,
        line: &str,
        t: &HfstTransducer,
        cutoff: usize,
    ) -> crate::error::Result<&mut Self> {
        let token = strstrip(line);

        let paths = if self.variables_["obey-flags"] == "ON" {
            t.lookup_fd_string(&token, cutoff as isize, 0.0)?
        } else {
            t.lookup_string(&token, cutoff as isize, 0.0)?
        };

        let mut out = std::io::stdout();
        let printed = self.print_paths_one(&paths, &mut out, -1);
        if !printed {
            println!("???");
        }
        Ok(self)
    }

    fn lookup_basic(&mut self, line: &str, t: &HfstBasicTransducer) -> &mut Self {
        let token = strstrip(line);

        let alpha = t.get_input_symbols();
        let mut tok = crate::hfst_tokenizer::HfstTokenizer::new();
        for it in alpha.iter() {
            tok.add_multichar_symbol(it);
        }
        // XXX: seting for splitc-chars ?
        let lookup_path: Vec<String> = tok.tokenize_one_level(&token, false);

        let mut cutoff: usize = usize::MAX; // (size_t)-1
        if t.is_lookup_infinitely_ambiguous_string_vector(
            &lookup_path,
            self.variables_["obey-flags"] == "ON",
        ) {
            cutoff = string_to_size_t(&self.variables_["lookup-cycle-cutoff"]);
            if self.verbose_ {
                warn!(
                    "lookup is infinitely ambiguous, limiting the number of cycles to {}",
                    cutoff
                );
            }
        }

        let mut results: HfstTwoLevelPaths = HfstTwoLevelPaths::new();

        if self.variables_["maximum-weight"] == "OFF" {
            t.lookup(
                &lookup_path,
                &mut results,
                Some(cutoff),
                None,
                -1, /*max_number*/
                self.variables_["obey-flags"] == "ON",
            );
        } else {
            let max_weight: f32 = string_to_float(&self.variables_["maximum-weight"]);
            t.lookup(
                &lookup_path,
                &mut results,
                Some(cutoff),
                Some(&max_weight),
                -1, /*max_number*/
                self.variables_["obey-flags"] == "ON",
            );
        }

        let mut printed = false; // if anything was printed

        if self.variables_["print-pairs"] == "OFF" {
            let paths = extract_output_paths(&results);
            let mut out = std::io::stdout();
            printed = self.print_paths_one(&paths, &mut out, -1);
        } else {
            let mut out = std::io::stdout();
            printed = self.print_paths_two(&results, &mut out, -1);
        }

        if !printed {
            println!("???");
        }
        self
    }

    // apply_down_line -> apply_up_line
    fn apply_up_line(&mut self, line: &str) -> crate::error::Result<&mut Self> {
        // GET_TOP(t)
        let Some(t) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };
        // lookdown not yet implemented in HFST
        if self.verbose_ {
            warn!(
                "apply up not implemented, inverting transducer and performing apply down\nfor faster performance, invert and minimize top network and do apply down instead"
            );
        }
        let mut copy = HfstTransducer::new_copy(&t.borrow())?;
        // the user has been warned for possible slow performance
        copy.invert()?.minimize()?;
        let fsm = HfstBasicTransducer::new_from_transducer(&copy);
        self.lookup_basic(line, &fsm);
        Ok(self)
    }

    // apply_up_line -> apply_down_line
    fn apply_down_line(&mut self, line: &str) -> crate::error::Result<&mut Self> {
        if self.stack_.len() < 1 {
            // EMPTY_STACK
            warn!("Empty stack.");
            self.xfst_lesser_fail();
            self.prompt();
            return Ok(self);
        }
        let t = self.stack_.last().unwrap().clone();
        let t_type = t.borrow().get_type();
        if t_type != ImplementationType::HFST_OL_TYPE && t_type != ImplementationType::HFST_OLW_TYPE
        {
            // hfst_fprintf(warnstream_, "lookup might be slow, consider
            // 'convert net'\n");
            let fsm = HfstBasicTransducer::new_from_transducer(&t.borrow());
            return Ok(self.lookup_basic(line, &fsm));
        }

        let mut ol_cutoff: usize = string_to_size_t(&self.variables_["lookup-cycle-cutoff"]); // -1; fix this
        // this gets ignored by ol transducer's is_lookup_infinitely_ambiguous
        let foo: Vec<String> = Vec::new();
        if t.borrow()
            .is_lookup_infinitely_ambiguous_string_vector(&foo)
        {
            ol_cutoff = string_to_size_t(&self.variables_["lookup-cycle-cutoff"]);
            if self.verbose_ {
                warn!(
                    "transducer is infinitely ambiguous, limiting number of cycles to {}",
                    ol_cutoff
                );
            }
        }

        self.lookup(line, &t.borrow(), ol_cutoff)
    }

    fn apply_med_line(&mut self, line: &str) -> &mut Self {
        let _ = line;
        warn!("Missing apply med");
        self
    }

    // @brief Add properties from text, one property per line
    // @todo properties cannot be stored in HFST automata
    pub fn add_props(&mut self, indata: &str) -> &mut Self {
        for line in indata.split('\n').filter(|l| !l.is_empty()) {
            self.add_prop_line(line);
        }
        self.prompt();
        self
    }

    // @brief Save @a name network in dot form in @a outfile
    pub fn write_dot_name(&mut self, name: &str, oss: &mut dyn std::io::Write) -> &mut Self {
        let _ = oss;
        if self.stack_.len() < 1 {
            warn!("Empty stack.");
            self.xfst_lesser_fail();
            self.prompt();
            return self;
        }
        let mut outfile = match std::fs::File::create(name) {
            Ok(f) => f,
            Err(_) => {
                error!("Could not open file {}", name);
                self.xfst_fail();
                self.prompt();
                return self;
            }
        };
        let Some(tmp) = self.top() else {
            self.xfst_lesser_fail();
            return self;
        };
        crate::hfst_print_dot::print_dot_os(&mut outfile, &mut *tmp.borrow_mut());
        self.prompt();
        self
    }

    // @brief Save top networks dot form in @a outfile
    pub fn write_dot(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        if self.stack_.len() < 1 {
            warn!("Empty stack.");
            self.xfst_lesser_fail();
            self.prompt();
            return self;
        }
        let Some(tmp) = self.top() else {
            self.xfst_lesser_fail();
            return self;
        };
        crate::hfst_print_dot::print_dot_os(oss, &mut *tmp.borrow_mut());
        let _ = oss.flush();
        self.prompt();
        self
    }

    // @brief Save top networks prolog form in @a outfile
    pub fn write_prolog(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        if self.stack_.len() < 1 {
            warn!("Empty stack.");
            self.xfst_lesser_fail();
            self.prompt();
            return self;
        }
        let mut reverse_stack: Vec<NetRef> = Vec::new();
        while self.stack_.len() != 0 {
            let tr = self.stack_.last().unwrap().clone();
            let mut name = tr.borrow().get_name();
            if name.is_empty() {
                name = "NO_NAME".to_string();
            }
            let fsm = HfstBasicTransducer::new_from_transducer(&tr.borrow());
            let write_weights = self.variables_["print-weight"] == "ON";
            fsm.write_in_prolog_format_os(oss, &name, write_weights);
            if self.stack_.len() != 1 {
                // separator
                let _ = writeln!(oss);
            }
            reverse_stack.push(tr);
            self.stack_.pop();
        }
        while reverse_stack.len() != 0 {
            self.stack_.push(reverse_stack.last().unwrap().clone());
            reverse_stack.pop();
        }
        let _ = oss.flush();
        self.prompt();
        self
    }

    // @brief Save top networks spaced paths form in @a outfile
    pub fn write_spaced(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let _ = writeln!(oss, "missing write spaced");
        let _ = oss.flush();
        self.prompt();
        self
    }

    // @brief Save top networks paths form in @a outfile
    pub fn write_text(&mut self, oss: &mut dyn std::io::Write) -> &mut Self {
        let _ = writeln!(oss, "missing write text");
        let _ = oss.flush();
        self.prompt();
        self
    }

    // @brief Save function @a name in @a outfile
    // @todo HFST does not support function macros in automata
    pub fn write_function(&mut self, name: &str, outfilename: &str) -> &mut Self {
        /*if (outfile == 0)
        {
          hfst_fprintf(outstream_, "%10s: %p\n", name, functions_[name]);
          }*/
        let _ = (name, outfilename);
        self.prompt();
        self
    }

    // @brief Save definition @a name in @a outfile
    // @todo HFST does not support saving name of definition in file
    pub fn write_definition(
        &mut self,
        name: &str,
        outfilename: &str,
    ) -> crate::error::Result<&mut Self> {
        if !self.definitions_.contains_key(name) {
            error!("no such defined network: '{}'", name);
            self.prompt();
            return Ok(self);
        }

        let mut outstream = if !outfilename.is_empty() {
            HfstOutputStream::new_filename(outfilename, self.format_, true)?
        } else {
            HfstOutputStream::new(self.format_, true)?
        };
        let def_ptr = self.definitions_[name].clone();
        let mut tmp = HfstTransducer::new_copy(&def_ptr.borrow())?;
        if self.variables_["name-nets"] == "ON" {
            tmp.set_name(name);
        }
        outstream.operator_shl(&mut tmp)?;
        outstream.close();
        self.prompt();
        Ok(self)
    }

    // @brief Save all definitions in @a outfile
    // @todo HFST does not support saving name of definition in file
    pub fn write_definitions(&mut self, outfilename: &str) -> crate::error::Result<&mut Self> {
        if self.definitions_.is_empty() {
            warn!("no defined networks");
            self.prompt();
            return Ok(self);
        }

        let mut outstream = if !outfilename.is_empty() {
            HfstOutputStream::new_filename(outfilename, self.format_, true)?
        } else {
            HfstOutputStream::new(self.format_, true)?
        };
        for (name, def) in self.definitions_.iter() {
            let mut tmp = HfstTransducer::new_copy(&def.borrow())?;
            tmp.set_name(name);
            outstream.operator_shl(&mut tmp)?;
        }
        outstream.close();
        self.prompt();
        Ok(self)
    }

    // @brief Save all transducers in stack to @a outfile
    pub fn write_stack(&mut self, outfilename: &str) -> crate::error::Result<&mut Self> {
        if self.stack_.len() < 1 {
            warn!("Empty stack.");
            self.xfst_lesser_fail();
            return Ok(self);
        }

        if !self.check_filename(outfilename) {
            return Ok(self);
        }

        let top_type = self.stack_.last().unwrap().borrow().get_type();
        let mut outstream = if !outfilename.is_empty() {
            HfstOutputStream::new_filename(outfilename, top_type, true)?
        } else {
            HfstOutputStream::new(top_type, true)?
        };
        let mut tmp: Vec<NetRef> = Vec::new();
        while !self.stack_.is_empty() {
            tmp.push(self.stack_.last().unwrap().clone());
            self.stack_.pop();
        }
        while !tmp.is_empty() {
            let t = tmp.last().unwrap().clone();
            outstream.operator_shl(&mut *t.borrow_mut())?;
            self.stack_.push(t);
            tmp.pop();
        }
        outstream.close();
        self.prompt();
        Ok(self)
    }

    // @brief Read properties from @a indata, one per line
    // @todo HFST automata do not support properties
    pub fn read_props(&mut self, indata: &str) -> &mut Self {
        for line in indata.split('\n').filter(|l| !l.is_empty()) {
            self.add_prop_line(line);
        }
        self.prompt();
        self
    }

    // @brief Compile regex of @a indata and save on stack.
    // Actually, the function assumes that the function compile_regex has
    // been called earlier when extracting the portion of input that
    // constitutes the regex \a indata.
    pub fn read_regex(&mut self, indata: &str) -> crate::error::Result<&mut Self> {
        // When calling this function, the regex \a indata should already have
        // been compiled into a transducer which should have been stored to
        // the variable latest_regex_compiled.
        let compiled = self.latest_regex_compiled.clone();
        if let Some(compiled) = compiled {
            let t = Rc::new(RefCell::new(HfstTransducer::new_copy(&compiled.borrow())?));
            t.borrow_mut().optimize()?;
            self.stack_.push(t);
            self.print_transducer_info();
        } else {
            error!("Error reading regex '{}'.", indata);
            self.xfst_fail();
        }
        self.prompt();
        Ok(self)
    }

    // @brief Read prolog form transducer from @a indata
    pub fn read_prolog(&mut self, indata: &str) -> &mut Self {
        let _ = indata;
        warn!("missing read prolog");
        self.print_transducer_info();
        self.prompt();
        self
    }

    // @brief Read spaced form transducer from @a infile
    pub fn read_spaced_from_file(&mut self, filename: &str) -> crate::error::Result<&mut Self> {
        if !self.check_filename(filename) {
            return Ok(self);
        }
        self.read_text_or_spaced(filename, true) // spaces are used
    }

    // @brief Read spaced form transducer from @a indata
    pub fn read_spaced(&mut self, indata: &str) -> &mut Self {
        let _ = indata;
        warn!("missing read spaced");
        self.print_transducer_info();
        self.prompt();
        self
    }

    // @brief Read text form transducer from @a infile
    pub fn read_text_from_file(&mut self, filename: &str) -> crate::error::Result<&mut Self> {
        if !self.check_filename(filename) {
            return Ok(self);
        }
        self.read_text_or_spaced(filename, false) // spaces are not used
    }

    // @brief Read text form transducer from @a indata
    pub fn read_text(&mut self, indata: &str) -> &mut Self {
        let _ = indata;
        warn!("missing read text");
        self.print_transducer_info();
        self.prompt();
        self
    }

    // @brief Read lexicons from @a infile
    pub fn read_lexc_from_file(&mut self, filename: &str) -> &mut Self {
        if !self.check_filename(filename) {
            return self;
        }

        if self.variables_["lexc-with-flags"] == "ON" {
            self.lexc_.set_with_flags(true);
            if self.variables_["lexc-minimize-flags"] == "ON" {
                self.lexc_.set_minimize_flags(true);
                if self.variables_["lexc-rename-flags"] == "ON" {
                    self.lexc_.set_rename_flags(true);
                }
            }
        }

        // The C++ 'lexc_.parse(FILE*)' path is replaced by the AST-walk
        // 'lexc_.compile(&str)', so the file contents are read into a string.
        let indata = match std::fs::read_to_string(filename) {
            Ok(s) => s,
            Err(_) => {
                error!("could not read lexc file");
                self.xfst_fail();
                self.prompt();
                return self;
            }
        };

        if self.has_lexc_been_read_ {
            self.lexc_.reset();
        } else {
            self.has_lexc_been_read_ = true;
        }

        let Some(mut t) = self.lexc_.compile(&indata) else {
            error!("error compiling file in lexc format");
            self.xfst_fail();
            self.prompt();
            return self;
        };

        t.optimize();
        self.stack_.push(Rc::new(RefCell::new(t)));
        self.print_transducer_info();
        self.prompt();
        self
    }

    // @brief Read a transducer in att format from file @a filename
    pub fn read_att_from_file(&mut self, filename: &str) -> &mut Self {
        if !self.check_filename(filename) {
            return self;
        }
        let infile = match std::fs::File::open(filename) {
            Ok(f) => f,
            Err(_) => {
                error!("could not read att file {}", filename);
                self.xfst_fail();
                self.prompt();
                return self;
            }
        };
        let mut reader = std::io::BufReader::new(infile);

        let att_eps = self.variables_["att-epsilon"].clone();
        let att_eps_default = att_eps == "@0@ | @_EPSILON_SYMBOL_@";
        let fmt = self.format_;
        let result = if att_eps_default {
            HfstTransducer::read_in_att_format_file(
                &mut reader,
                fmt,
                crate::hfst_symbol_defs::internal_epsilon,
                false,
            )
        } else {
            HfstTransducer::read_in_att_format_file(&mut reader, fmt, &att_eps, false)
        };
        match result {
            Ok(r) => {
                // recover ownership of the heap transducer the reader leaked (Box::leak)
                let tmp = unsafe { *Box::from_raw(std::ptr::from_mut(r)) };
                let net = Rc::new(RefCell::new(tmp));
                net.borrow_mut().optimize();
                self.stack_.push(net);
                self.print_transducer_info();
            }
            Err(_e) => {
                error!("error reading in att format");
                self.xfst_fail();
            }
        }
        self.prompt();
        self
    }

    // [spec:hfst:def:xfst-compiler.hfst.xfst.xfst-compiler.check-filename-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.xfst-compiler.check-filename-fn]
    pub fn check_filename(&mut self, filename: &str) -> bool {
        if self.restricted_mode_ {
            let fn_ = filename.to_string();
            if fn_.contains('/') || fn_.contains('\\') {
                warn!(
                    "Restricted mode (--restricted-mode) is in use, write and read operations are allowed\nonly in current directory (i.e. filenames cannot contain '/' or '\\')"
                );
                self.xfst_lesser_fail();
                self.prompt();
                return false;
            }
        }
        self.prompt();
        true
    }

    // @brief Read strings (with or without spaces between the symbols,
    // as defined by \a spaces) from \a infile, disjunct them into
    // a single transducer and push it to the stack.
    fn read_text_or_spaced(
        &mut self,
        filename: &str,
        spaces: bool,
    ) -> crate::error::Result<&mut Self> {
        if !self.check_filename(filename) {
            return Ok(self);
        }
        let infile = match std::fs::File::open(filename) {
            Ok(f) => f,
            Err(_) => {
                error!("Could not open file {}", filename);
                self.xfst_fail();
                self.prompt();
                return Ok(self);
            }
        };

        let tmp = Rc::new(RefCell::new(HfstTransducer::new_type(self.format_)?));
        let mcs: Vec<String> = Vec::new(); // no multichar symbols
        // [spec:hfst:def:xfst-compiler.hfst.xfst.tok-fn]
        // [spec:hfst:sem:xfst-compiler.hfst.xfst.tok-fn]
        let tok = crate::hfst_strings2_fst_tokenizer::HfstStrings2FstTokenizer::new(
            &mcs,
            crate::hfst_symbol_defs::internal_epsilon,
        )?;
        let mut reader = std::io::BufReader::new(infile);

        loop {
            let mut buf = String::new();
            match reader.read_line(&mut buf) {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }
            let line = self.remove_newline(buf);
            let spv = tok.tokenize_pair_string(&line, spaces)?;
            // [spec:hfst:def:xfst-compiler.hfst.xfst.line-tr-fn]
            // [spec:hfst:sem:xfst-compiler.hfst.xfst.line-tr-fn]
            let line_tr = HfstTransducer::new_string_pair_vector(&spv, self.format_)?;
            tmp.borrow_mut().disjunct(&line_tr, true)?;
        }

        // The file is closed when 'reader' is dropped.

        tmp.borrow_mut().minimize()?; // a trie should be easily minimizable
        self.stack_.push(tmp);
        self.print_transducer_info();
        self.prompt();
        Ok(self)
    }

    pub fn substitute_named(
        &mut self,
        variable: &str,
        label: &str,
    ) -> crate::error::Result<&mut Self> {
        // GET_TOP(top)
        let Some(top) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };

        if !self.definitions_.contains_key(variable) {
            error!("no such definition '{}', cannot substitute", variable);
            // MAYBE_QUIT
            if self.variables_["quit-on-fail"] == "ON" {
                self.fail_flag_ = true;
            }
            self.prompt();
            return Ok(self);
        }
        let def_ptr = self.definitions_[variable].clone();

        // [spec:hfst:def:xfst-compiler.hfst.xfst.labelstr-fn]
        // [spec:hfst:sem:xfst-compiler.hfst.xfst.labelstr-fn]
        let mut labelstr = label.to_string();
        if labelstr == "?" {
            labelstr = String::from("@_IDENTITY_SYMBOL_@");
        }
        if labelstr == "0" {
            labelstr = String::from("@_EPSILON_SYMBOL_@");
        }

        let mut alpha = top.borrow().get_alphabet()?;
        if !alpha.contains(&labelstr) {
            error!("no occurrences of label '{}', cannot substitute", label);
            // MAYBE_QUIT
            if self.variables_["quit-on-fail"] == "ON" {
                self.fail_flag_ = true;
            }
            self.prompt();
            return Ok(self);
        }

        let fsm = HfstBasicTransducer::new_from_transducer(&top.borrow());

        for it in fsm.iter() {
            for tr_it in it {
                let isymbol = tr_it.get_input_symbol(fsm.coder());
                let osymbol = tr_it.get_output_symbol(fsm.coder());
                if isymbol != osymbol && (isymbol == labelstr || osymbol == labelstr) {
                    error!(
                        "label '{}' is used as a symbol on one side of an arc, cannot substitute",
                        label
                    );
                    // MAYBE_QUIT
                    if self.variables_["quit-on-fail"] == "ON" {
                        self.fail_flag_ = true;
                    }
                    self.prompt();
                    return Ok(self);
                }
            }
        }

        // [spec:hfst:def:xfst-compiler.hfst.xfst.labelpair-fn]
        // [spec:hfst:sem:xfst-compiler.hfst.xfst.labelpair-fn]
        let labelpair: StringPair = (labelstr.clone(), labelstr.clone());
        alpha = def_ptr.borrow().get_alphabet()?;
        top.borrow_mut().substitute_pair_with_transducer(
            &labelpair,
            &mut *def_ptr.borrow_mut(),
            false,
        )?;

        if labelstr != "@_EPSILON_SYMBOL_@"
            && labelstr != "@_IDENTITY_SYMBOL_@"
            && !alpha.contains(&labelstr)
        {
            top.borrow_mut().remove_from_alphabet_string(&labelstr)?;
        }

        // MAYBE_MINIMIZE(top)
        top.borrow_mut().optimize()?;
        self.prompt();
        Ok(self)
    }

    // @brief Substitute all labels @a list by @a target.
    pub fn substitute_label(&mut self, list: &str, target: &str) -> &mut Self {
        // GET_TOP(top)
        let Some(top) = self.top() else {
            self.xfst_lesser_fail();
            return self;
        };

        // tokenize list into labels
        let mut symbol_pairs: StringPairSet = StringPairSet::new();

        if list != "NOTHING" {
            let labels = Self::tokenize_string(list, ' ');
            for label in labels.iter() {
                // tokenize labels into string pairs
                let sv = Self::tokenize_string(label, ':');
                match Self::catch_symbol_vector_to_symbol_pair(&sv) {
                    Some(sp) => {
                        symbol_pairs.insert(sp);
                    }
                    None => {
                        error!("could not substitute with '{}'", list);
                        // MAYBE_QUIT
                        if self.variables_["quit-on-fail"] == "ON" {
                            self.fail_flag_ = true;
                        }
                        self.prompt();
                        return self;
                    }
                }
            }
        }

        // tokenize target label into string pair
        let target_vector = Self::tokenize_string(target, ':');
        match Self::catch_symbol_vector_to_symbol_pair(&target_vector) {
            Some(target_label) => {
                let fsm = HfstBasicTransducer::new_from_transducer(&top.borrow());
                let mut target_label_found = false;

                for it in fsm.iter() {
                    if target_label_found {
                        break;
                    }
                    for tr_it in it {
                        if target_label.0 == tr_it.get_input_symbol(fsm.coder())
                            && target_label.1 == tr_it.get_output_symbol(fsm.coder())
                        {
                            target_label_found = true;
                            break;
                        }
                    }
                }
                if !target_label_found {
                    error!(
                        "no occurrences of '{}:{}', cannot substitute",
                        target_label.0, target_label.1
                    );
                    self.prompt();
                    return self;
                }

                top.borrow_mut()
                    .substitute_symbol_pair_with_set(&target_label, &symbol_pairs);
            }
            None => {
                error!("could not substitute '{}'", target);
                // MAYBE_QUIT
                if self.variables_["quit-on-fail"] == "ON" {
                    self.fail_flag_ = true;
                }
            }
        }

        // MAYBE_MINIMIZE(top)
        top.borrow_mut().optimize();
        self.prompt();
        self
    }

    // @brief Substitute all symbols in @a list by @a target.
    pub fn substitute_symbol(
        &mut self,
        list: &str,
        target: &str,
    ) -> crate::error::Result<&mut Self> {
        // GET_TOP(top)
        let Some(top) = self.top() else {
            self.xfst_lesser_fail();
            return Ok(self);
        };

        let alpha = top.borrow().get_alphabet()?;
        if !alpha.contains(target) {
            error!("no occurrences of symbol '{}', cannot substitute", target);
            // MAYBE_QUIT
            if self.variables_["quit-on-fail"] == "ON" {
                self.fail_flag_ = true;
            }
            self.prompt();
            return Ok(self);
        }

        self.stack_.pop();

        // [spec:hfst:def:xfst-compiler.hfst.xfst.liststr-fn]
        // [spec:hfst:sem:xfst-compiler.hfst.xfst.liststr-fn]
        let mut liststr = list.to_string();
        if liststr == "\"NOTHING\"" {
            // list is given in quoted format: "foo" "bar" ...
            liststr = String::new();
        }

        // use regex parser to build the substitution: [ [TR] , "s" , L ]
        self.xre_
            .define_transducer("TempXfstTransducerName", &top.borrow()); // XRE
        let mut subst_regex = String::from("`[ [TempXfstTransducerName] , ");
        subst_regex.push_str(&format!("\"{}\" , {} ]", target, liststr));
        let substituted = self
            .xre_
            .compile(&subst_regex)
            .map(|t| Rc::new(RefCell::new(t))); // XRE
        self.xre_.undefine("TempXfstTransducerName"); // XRE
        drop(top);

        if let Some(substituted) = substituted {
            // MAYBE_MINIMIZE(substituted)
            substituted.borrow_mut().optimize()?;
            self.stack_.push(substituted);
            self.print_transducer_info();
        } else {
            error!("fatal error in substitution");
            self.fail_flag_ = true;
        }
        self.prompt();
        Ok(self)
    }

    // @brief Test top transducer in stack for equivalence
    // @todo tests are not implemented
    pub fn test_eq(&mut self, assertion: bool) -> crate::error::Result<&mut Self> {
        if self.stack_.len() < 2 {
            warn!("Not enough networks on stack.\nOperation requires at least 2.");
            self.xfst_lesser_fail();
            return Ok(self);
        }
        let first = self.stack_.last().unwrap().clone();
        self.stack_.pop();
        let second = self.stack_.last().unwrap().clone();
        self.stack_.pop();
        let result = first.borrow().compare(&second.borrow(), false)?;
        self.print_bool(result);
        self.stack_.push(second);
        self.stack_.push(first);
        // MAYBE_ASSERT(assertion, result)
        if !result
            && ((self.variables_["assert"] == "ON" || assertion)
                && (self.variables_["quit-on-fail"] == "ON"))
        {
            self.fail_flag_ = true;
        }
        Ok(self)
    }

    // @brief Test top transducer in stack for functionality
    // @todo tests are not implemented
    pub fn test_funct(&mut self, assertion: bool) -> &mut Self {
        let _ = assertion;
        warn!("test funct missing");
        self.prompt();
        self
    }

    // @brief Test top transducer in stack for identity
    // @todo tests are not implemented
    pub fn test_id(&mut self, assertion: bool) -> crate::error::Result<&mut Self> {
        let Some(tmp) = self.top() else {
            return Ok(self);
        };

        let mut tmp_input = HfstTransducer::new_from_transducer(&tmp.borrow());
        tmp_input.input_project()?;
        let mut tmp_output = HfstTransducer::new_from_transducer(&tmp.borrow());
        tmp_output.output_project()?;

        let result = tmp_input.compare(&tmp_output, false)?;
        self.print_bool(result);
        // MAYBE_ASSERT(assertion, result)
        if !result
            && ((self.variables_["assert"] == "ON" || assertion)
                && (self.variables_["quit-on-fail"] == "ON"))
        {
            self.fail_flag_ = true;
        }
        self.prompt();
        Ok(self)
    }

    // @brief Test top transducer in stack for upper language boundedness
    // @todo tests are not implemented
    pub fn test_upper_bounded(&mut self, assertion: bool) -> crate::error::Result<&mut Self> {
        let Some(temp) = self.top() else {
            return Ok(self);
        };

        let mut tmp = HfstTransducer::new_from_transducer(&temp.borrow());
        tmp.output_project()?;
        tmp.remove_epsilons()?; // needed for testing cyclicity

        let result = !tmp.is_cyclic()?;
        self.print_bool(result);
        // MAYBE_ASSERT(assertion, result)
        if !result
            && ((self.variables_["assert"] == "ON" || assertion)
                && (self.variables_["quit-on-fail"] == "ON"))
        {
            self.fail_flag_ = true;
        }
        self.prompt();
        Ok(self)
    }

    pub fn test_uni(&mut self, level: Level, assertion: bool) -> crate::error::Result<&mut Self> {
        let Some(temp) = self.top() else {
            return Ok(self);
        };

        let mut tmp = HfstTransducer::new_from_transducer(&temp.borrow());
        tmp.input_project()?;
        let id = HfstTransducer::new_symbol(internal_identity, tmp.get_type())?;
        let mut value = false;

        if level == Level::UPPER_LEVEL {
            value = id.compare(&tmp, false)?;
        } else if level == Level::LOWER_LEVEL {
            value = !id.compare(&tmp, false)?;
        } else {
            error!("ERROR: argument given to function 'test_uni' not recognized");
        }
        self.print_bool(value);
        // MAYBE_ASSERT(assertion, value)
        if !value
            && ((self.variables_["assert"] == "ON" || assertion)
                && (self.variables_["quit-on-fail"] == "ON"))
        {
            self.fail_flag_ = true;
        }
        self.prompt();
        Ok(self)
    }

    // @brief Test top transducer in stack for upper language universality
    // @todo tests are not implemented
    pub fn test_upper_uni(&mut self, assertion: bool) -> crate::error::Result<&mut Self> {
        self.test_uni(Level::UPPER_LEVEL, assertion)
    }

    // @brief Test top transducer in stack for lower language boundedness
    // @todo tests are not implemented
    pub fn test_lower_bounded(&mut self, assertion: bool) -> crate::error::Result<&mut Self> {
        let Some(temp) = self.top() else {
            return Ok(self);
        };

        let mut tmp = HfstTransducer::new_from_transducer(&temp.borrow());
        tmp.input_project()?;
        tmp.remove_epsilons()?; // needed for testing cyclicity

        let result = !tmp.is_cyclic()?;
        self.print_bool(result);
        // MAYBE_ASSERT(assertion, result)
        if !result
            && ((self.variables_["assert"] == "ON" || assertion)
                && (self.variables_["quit-on-fail"] == "ON"))
        {
            self.fail_flag_ = true;
        }
        self.prompt();
        Ok(self)
    }

    // @brief Test top transducer in stack for lower language universality
    // @todo tests are not implemented
    pub fn test_lower_uni(&mut self, assertion: bool) -> crate::error::Result<&mut Self> {
        self.test_uni(Level::LOWER_LEVEL, assertion)
    }

    // @brief Test top transducer in stack for not emptiness
    // @todo tests are not implemented
    pub fn test_nonnull(&mut self, assertion: bool) -> crate::error::Result<&mut Self> {
        self.test_null(true, assertion)
    }

    // @brief Test top transducer in stack for emptiness
    // \a invert_test_result defines whether the result is inverted
    // (so that 'test_nonnull' can be implemented with the same function).
    // @todo tests are not implemented
    pub fn test_null(
        &mut self,
        invert_test_result: bool,
        assertion: bool,
    ) -> crate::error::Result<&mut Self> {
        let Some(tmp) = self.top() else {
            return Ok(self);
        };

        let empty = HfstTransducer::new_type(tmp.borrow().get_type())?;
        let mut value = empty.compare(&tmp.borrow(), false)?;
        if invert_test_result {
            value = !value;
        }
        self.print_bool(value);

        // MAYBE_ASSERT(assertion, value)
        if !value
            && ((self.variables_["assert"] == "ON" || assertion)
                && (self.variables_["quit-on-fail"] == "ON"))
        {
            self.fail_flag_ = true;
        }
        self.prompt();
        Ok(self)
    }

    // @brief Print the result of \a operation when applied to the whole stack.
    fn test_operation(
        &mut self,
        operation: TestOperation,
        assertion: bool,
    ) -> crate::error::Result<&mut Self> {
        if self.stack_.len() < 2 {
            warn!("Not enough networks on stack. Operation requires at least 2.");
            self.xfst_lesser_fail();
            self.prompt();
            return Ok(self);
        }
        // [spec:hfst:def:xfst-compiler.hfst.xfst.copied-stack-fn]
        // [spec:hfst:sem:xfst-compiler.hfst.xfst.copied-stack-fn]
        let mut copied_stack: Vec<NetRef> = self.stack_.clone();

        let mut topmost_transducer =
            HfstTransducer::new_from_transducer(&copied_stack.last().unwrap().borrow());
        copied_stack.pop();

        let empty = HfstTransducer::new_type(topmost_transducer.get_type())?;

        while !copied_stack.is_empty() {
            let next_transducer =
                HfstTransducer::new_from_transducer(&copied_stack.last().unwrap().borrow());
            copied_stack.pop();

            match operation {
                TestOperation::TEST_OVERLAP_ => {
                    topmost_transducer.intersect(&next_transducer, true)?;
                    if topmost_transducer.compare(&empty, true)? {
                        self.print_bool(false);
                        // MAYBE_ASSERT(assertion, false)
                        let value = false;
                        if !value
                            && ((self.variables_["assert"] == "ON" || assertion)
                                && (self.variables_["quit-on-fail"] == "ON"))
                        {
                            self.fail_flag_ = true;
                        }
                        self.prompt();
                        return Ok(self);
                    }
                }
                TestOperation::TEST_SUBLANGUAGE_ => {
                    // [spec:hfst:def:xfst-compiler.hfst.xfst.intersection-fn]
                    // [spec:hfst:sem:xfst-compiler.hfst.xfst.intersection-fn]
                    let mut intersection = HfstTransducer::new_from_transducer(&topmost_transducer);
                    intersection.intersect(&next_transducer, true)?;
                    if !intersection.compare(&topmost_transducer, true)? {
                        self.print_bool(false);
                        // MAYBE_ASSERT(assertion, false)
                        let value = false;
                        if !value
                            && ((self.variables_["assert"] == "ON" || assertion)
                                && (self.variables_["quit-on-fail"] == "ON"))
                        {
                            self.fail_flag_ = true;
                        }
                        self.prompt();
                        return Ok(self);
                    }
                    topmost_transducer = next_transducer;
                }
            }
        }
        self.print_bool(true);
        // MAYBE_ASSERT(assertion, true)
        let value = true;
        if !value
            && ((self.variables_["assert"] == "ON" || assertion)
                && (self.variables_["quit-on-fail"] == "ON"))
        {
            self.fail_flag_ = true;
        }
        self.prompt();
        Ok(self)
    }

    // @brief Test top transducer in stack for overlapping
    // @todo tests are not implemented
    pub fn test_overlap(&mut self, assertion: bool) -> crate::error::Result<&mut Self> {
        self.test_operation(TestOperation::TEST_OVERLAP_, assertion)
    }

    // @brief Test top transducer in stack for sublanguage
    // @todo tests are not implemented
    pub fn test_sublanguage(&mut self, assertion: bool) -> crate::error::Result<&mut Self> {
        self.test_operation(TestOperation::TEST_SUBLANGUAGE_, assertion)
    }

    // @brief Test top transducer in stack for unambiguity
    // @todo tests are not implemented
    pub fn test_unambiguous(&mut self, assertion: bool) -> &mut Self {
        let _ = assertion;
        warn!("test unambiguous missing");
        self.prompt();
        self
    }

    pub fn test_infinitely_ambiguous(
        &mut self,
        assertion: bool,
    ) -> crate::error::Result<&mut Self> {
        let Some(tmp) = self.top() else {
            return Ok(self);
        };
        let value = tmp.borrow().is_infinitely_ambiguous()?;
        self.print_bool(value);
        // MAYBE_ASSERT(assertion, value)
        if !value
            && ((self.variables_["assert"] == "ON" || assertion)
                && (self.variables_["quit-on-fail"] == "ON"))
        {
            self.fail_flag_ = true;
        }
        self.prompt();
        Ok(self)
    }

    // Tokenize string \a s using \a c as separator.
    // [spec:hfst:def:xfst-compiler.hfst.xfst.tokenize-string-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.tokenize-string-fn]
    fn tokenize_string(s: &str, c: char) -> StringVector {
        let mut retval: StringVector = Vec::new();
        let bytes = s.as_bytes();
        let mut pos: usize = 0;
        for i in 0..bytes.len() {
            if bytes[i] == c as u8 {
                retval.push(s[pos..i].to_string());
                pos = i + 1;
            }
        }
        retval.push(s[pos..].to_string());
        retval
    }

    // Convert StringVector \a sv into StringPair.
    // [spec:hfst:def:xfst-compiler.hfst.xfst.symbol-vector-to-symbol-pair-fn]
    // [spec:hfst:sem:xfst-compiler.hfst.xfst.symbol-vector-to-symbol-pair-fn]
    fn symbol_vector_to_symbol_pair(sv: &StringVector) -> StringPair {
        let mut sp: StringPair = (String::new(), String::new());
        if sv.len() == 2 {
            if sv[0] == "?" {
                sp.0 = String::from("@_UNKNOWN_SYMBOL_@");
            } else if sv[0] == "0" {
                sp.0 = String::from("@_EPSILON_SYMBOL_@");
            } else {
                sp.0 = sv[0].clone();
            }

            if sv[1] == "?" {
                sp.1 = String::from("@_UNKNOWN_SYMBOL_@");
            } else if sv[1] == "0" {
                sp.1 = String::from("@_EPSILON_SYMBOL_@");
            } else {
                sp.1 = sv[1].clone();
            }
        } else if sv.len() == 1 {
            if sv[0] == "?" {
                // special case "?"
                sp.0 = String::from("@_IDENTITY_SYMBOL_@");
            } else if sv[0] == "0" {
                sp.0 = String::from("@_EPSILON_SYMBOL_@");
            } else {
                sp.0 = sv[0].clone();
            }
            sp.1 = sp.0.clone();
        } else {
            std::panic::panic_any("error: symbol vector cannot be converted into symbol pair");
        }
        sp
    }

    // Wraps 'symbol_vector_to_symbol_pair' in the C++ try/catch: returns None
    // when it would throw (a 'panic_any' carrying a const char* message). The
    // panic hook is silenced so the caught exception does not print.
    fn catch_symbol_vector_to_symbol_pair(sv: &StringVector) -> Option<StringPair> {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Self::symbol_vector_to_symbol_pair(sv)
        }));
        std::panic::set_hook(prev);
        match r {
            Ok(v) => Some(v),
            Err(e) => {
                if e.downcast_ref::<&str>().is_some() {
                    None
                } else {
                    std::panic::resume_unwind(e)
                }
            }
        }
    }
}
