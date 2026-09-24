//! ABSOLUTE-faithful C++->Rust port of HFST's LEXC (lexicon) compiler,
//! RESTRUCTURED to walk the 'nfst-lexc' typed AST instead of the original
//! Flex/Bison grammar. The AST-walk restructuring is the ONE sanctioned
//! structural deviation in this port: the trie/transducer-building BEHAVIOUR of
//! 'compileLexical' and the 'add*Entry' accumulators must still match the C++
//! semantic actions in 'lexc-parser.yy' / 'LexcCompiler.cc' exactly.
//!
//! Ported from 'libhfst/src/parsers/LexcCompiler.{h,cc}' and
//! 'libhfst/src/parsers/lexc-utils.{h,cc}'.
//!
//! # C++ globals / file-statics folded onto the instance
//!   * the 'lexc' singleton becomes '&mut self';
//!   * 'static bool firstLexicon' becomes the 'first_lexicon' field;
//!   * the unused 'static StringVector multichar_symbols' is dropped.
//!
//! # Stream / WINDOWS plumbing dropped — error text via 'tracing'.
//!
//! # Deferred (record as 'unimplemented!')
//! - 'parse(FILE*)' / 'parse(const char*)' REPLACED by the AST-walk 'compile(&str)'.
//! - lexc-utils.cc Flex bookkeeping helpers (token positions, hand-lexer percent stripping).
//! - 'getStringTries()' / 'getRegexpUnions()' — header-declared, never defined in .cc.

#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::too_many_arguments)]

use std::collections::{BTreeMap, BTreeSet};

use nfst_lexc::{
    Definition, EntrySpec, LexcFile, Lexicon, LexiconEntry, LexiconName, MulticharSymbol, Spanned,
    parse,
};
use nfst_xre::{SpannedXre, pretty_print};

use crate::backend::AlgebraBackend;
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_data_types::{
    ImplementationType, StringPair, StringPairVector, StringVector, Symbol,
};
use crate::hfst_symbol_defs::{HfstSymbolSubstitutions, StringSet};
use crate::hfst_tokenizer::HfstTokenizer;
use crate::hfst_transducer::HfstTransducer;
use crate::xre::XreCompiler;
use tracing::{debug, error, info, warn};

mod compile;
mod entries;
mod graphemes;
mod recode;
use recode::joiner_encode;

/// A grapheme spelled with several code points that the source never declared:
/// where it sits, what to say about it, and what to type instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphemeDiagnostic {
    /// Byte range in the lexc source the caret goes under — the grapheme
    /// itself where it can be located, else the whole entry.
    pub span: std::ops::Range<usize>,
    pub message: String,
    /// Advice rendered beneath the snippet.
    pub notes: Vec<String>,
}

// [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler]
pub struct LexcCompiler<B: AlgebraBackend> {
    pub(crate) tokenizer: HfstTokenizer,
    pub(crate) xre: XreCompiler<B>,
    pub(crate) initialLexiconName_: String,
    pub(crate) currentLexiconName_: String,
    pub(crate) stringsTrie_: HfstBasicTransducer,
    pub(crate) regexps: BTreeMap<Symbol, HfstTransducer<B>>, // owning HfstTransducer* -> owned
    pub(crate) lexiconNames_: BTreeSet<Symbol>,
    pub(crate) noFlags_: BTreeSet<Symbol>,
    pub(crate) continuations: BTreeSet<Symbol>,
    pub(crate) alphabets: BTreeSet<Symbol>,
    pub(crate) totalEntries_: usize,
    pub(crate) currentEntries_: usize,
    pub(crate) align_strings: bool,
    pub(crate) with_flags: bool,
    pub(crate) minimize_flags: bool,
    pub(crate) rename_flags: bool,
    pub(crate) split_characters: bool,
    pub(crate) treat_warnings_as_errors: bool,
    pub(crate) warn_everything: bool,
    pub(crate) warn_missing_lexicons: bool,
    pub(crate) warn_unused_lexicons: bool,
    pub(crate) warn_repeated_lexicons: bool,
    pub(crate) warn_missing_alphabets: bool,
    pub(crate) warn_one_sided_flags: bool, // C++ leaves this UNINITIALIZED; default false
    pub(crate) warn_unnecessary_escapes: bool,
    pub(crate) verbose: bool,
    pub(crate) quiet: bool,
    pub(crate) first_lexicon: bool, // folded 'static bool firstLexicon'
    pub parseErrors_: bool,         // public field in C++ header
    /// Whether composition treats flag diacritics as epsilons (the former
    /// 'flag_is_epsilon_in_composition' file-static global, default 'false');
    /// threaded into 'compile_lexical's composes. hfst-lexc-compiler's
    /// '--xfst flag-is-epsilon' toggles it.
    pub(crate) flag_is_epsilon: bool,
    /// Whether composition treats flag diacritics as ordinary symbols, Xerox-style
    /// (the former 'xerox_composition' file-static global, default 'false');
    /// threaded into 'compile_lexical's composes. hfst-lexc-compiler defaults this
    /// ON, toggled by its '--xerox-composition' option.
    pub(crate) xerox_composition: bool,
    /// The lexc source currently being compiled, retained so token-level
    /// diagnostics can render the offending snippet (ariadne). Empty until
    /// `parse` runs.
    pub(crate) source: String,
    /// Label shown in diagnostics for `source` (a file name, or `"<lexc>"`).
    pub(crate) source_name: String,
    /// Byte span in `source` of the entry currently being walked, updated as
    /// `compile_file` visits each spanned AST node; the anchor for
    /// `error_at_current_token`/`warning_at_current_token`.
    pub(crate) current_span: std::ops::Range<usize>,
    /// Undeclared multi-code-point graphemes reported so far, in source order.
    /// Rendering goes to stderr; this keeps the same data addressable.
    pub(crate) grapheme_diags: Vec<GraphemeDiagnostic>,
}

// ==========================================================================
// LexcCompiler — constructors, option setters and getters, and the
// error/warning helpers (ported from LexcCompiler.cc).
// ==========================================================================

impl<B: AlgebraBackend> LexcCompiler<B> {
    /// Port of 'LexcCompiler(ImplementationType impl)' (unannotated in the .cc),
    /// also the common body of 'LexcCompiler(impl, withFlags, alignStrings)':
    /// seeds the tokenizer with the epsilon/zero multichars + the '#' joiner,
    /// registers '#' as a lexicon name, and configures 'xre'.
    pub fn new() -> LexcCompiler<B> {
        let mut compiler = LexcCompiler {
            tokenizer: HfstTokenizer::new(),
            xre: XreCompiler::new(),
            initialLexiconName_: "Root".to_string(),
            currentLexiconName_: String::new(),
            stringsTrie_: HfstBasicTransducer::new(),
            regexps: BTreeMap::new(),
            lexiconNames_: BTreeSet::new(),
            noFlags_: BTreeSet::new(),
            continuations: BTreeSet::new(),
            alphabets: BTreeSet::new(),
            totalEntries_: 0,
            currentEntries_: 0,
            align_strings: false,
            with_flags: false,
            minimize_flags: false,
            rename_flags: false,
            split_characters: false,
            treat_warnings_as_errors: false,
            warn_everything: false,
            warn_missing_lexicons: false,
            warn_unused_lexicons: false,
            warn_repeated_lexicons: false,
            warn_missing_alphabets: false,
            warn_one_sided_flags: false,
            warn_unnecessary_escapes: false,
            verbose: false,
            quiet: false,
            first_lexicon: true,
            parseErrors_: false,
            flag_is_epsilon: false,
            xerox_composition: false,
            source: String::new(),
            source_name: String::from("<lexc>"),
            current_span: 0..0,
            grapheme_diags: Vec::new(),
        };
        compiler
            .tokenizer
            .add_multichar_symbol("@_EPSILON_SYMBOL_@");
        compiler.tokenizer.add_multichar_symbol("@0@");
        compiler.tokenizer.add_multichar_symbol("@ZERO@");
        compiler
            .tokenizer
            .add_multichar_symbol("@@ANOTHER_EPSILON@@");
        let hash = "#".to_string();
        compiler.lexiconNames_.insert(Symbol::from(hash.clone()));
        let enc = joiner_encode(&hash);
        compiler.tokenizer.add_multichar_symbol(&enc);
        compiler.xre.set_expand_definitions(true);
        compiler.xre.set_verbosity(!compiler.quiet);
        compiler
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.lexc-compiler-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.lexc-compiler-fn]
    /// Port of 'LexcCompiler(impl, withFlags, alignStrings)'.
    pub fn new_with_flags(with_flags: bool, align_strings: bool) -> LexcCompiler<B> {
        let mut compiler = LexcCompiler::new();
        compiler.align_strings = align_strings;
        compiler.with_flags = with_flags;
        compiler
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.reset-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.reset-fn]
    pub fn reset(&mut self) {
        self.tokenizer = HfstTokenizer::new();
        self.tokenizer.add_multichar_symbol("@_EPSILON_SYMBOL_@");
        self.tokenizer.add_multichar_symbol("@0@");
        self.tokenizer.add_multichar_symbol("@ZERO@");
        self.tokenizer.add_multichar_symbol("@@ANOTHER_EPSILON@@");
        self.initialLexiconName_ = "Root".to_string();
        self.totalEntries_ = 0;
        self.currentEntries_ = 0;
        self.parseErrors_ = false;
        self.lexiconNames_.clear();
        self.noFlags_.clear();
        self.continuations.clear();
        self.alphabets.clear();
        self.grapheme_diags.clear();
        self.currentLexiconName_ = String::new(); // ?
        self.lexiconNames_.insert(Symbol::new("#"));
        self.stringsTrie_ = HfstBasicTransducer::new(); // ?
        // The owned regexps transducers are dropped by clear() (C++ delete'd
        // the raw pointers here). The C++ 'static bool firstLexicon' was a
        // function-static and is NOT touched by reset(); first_lexicon is left
        // untouched to mirror that.
        self.regexps.clear();
    }

    // ----- option setters / getters -----

    pub fn set_verbosity(&mut self, verbose: u32) -> &mut Self {
        if verbose == 0 {
            self.quiet = true;
            self.verbose = false;
        } else if verbose == 1 {
            self.quiet = false;
            self.verbose = false;
        } else {
            self.quiet = false;
            self.verbose = true;
        }
        self.xre.set_verbosity(!self.quiet);
        self
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.get-verbosity-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.get-verbosity-fn]
    pub fn get_verbosity(&self) -> u32 {
        if self.quiet && !self.verbose {
            return 0;
        }
        if !self.quiet && !self.verbose {
            return 1;
        }
        if !self.quiet && self.verbose {
            return 2;
        }
        std::panic::panic_any("LexcCompiler::getVerbosity() failed".to_string())
    }

    pub fn set_treat_warnings_as_errors(&mut self, value: bool) -> &mut Self {
        self.treat_warnings_as_errors = value;
        self
    }

    pub fn set_align_strings(&mut self, value: bool) -> &mut Self {
        self.align_strings = value;
        self
    }

    pub fn set_with_flags(&mut self, value: bool) -> &mut Self {
        self.with_flags = value;
        self
    }

    pub fn set_minimize_flags(&mut self, value: bool) -> &mut Self {
        self.minimize_flags = value;
        self
    }

    pub fn set_rename_flags(&mut self, value: bool) -> &mut Self {
        self.rename_flags = value;
        self
    }

    /// Set whether composition treats flag diacritics as epsilons (was the
    /// 'hfst::set_flag_is_epsilon_in_composition' file-static global; the
    /// '--xfst flag-is-epsilon' option of hfst-lexc-compiler toggles it).
    pub fn set_flag_is_epsilon(&mut self, value: bool) -> &mut Self {
        self.flag_is_epsilon = value;
        self
    }

    /// Set whether composition treats flag diacritics as ordinary symbols,
    /// Xerox-style (was the 'hfst::set_xerox_composition' file-static global; the
    /// '--xerox-composition' option of hfst-lexc-compiler toggles it).
    pub fn set_xerox_composition(&mut self, value: bool) -> &mut Self {
        self.xerox_composition = value;
        self
    }

    /// Set whether minimization encodes weights into labels first (was the
    /// 'hfst::set_encode_weights' process-global that C++ minimize read; the
    /// '-E' / '--encode-weights' option of hfst-lexc toggles it). Forwarded to
    /// the embedded [`XreCompiler`] so regex compilation inside lexc obeys it.
    pub fn set_encode_weights(&mut self, value: bool) -> &mut Self {
        self.xre.set_encode_weights(value);
        self
    }

    /// The [`EngineConfig`](crate::hfst_transducer::EngineConfig) the composes in
    /// 'compile_lexical' run with: C++ defaults except 'flag_is_epsilon_in_composition'
    /// and 'xerox_composition'.
    fn compose_cfg(&self) -> crate::hfst_transducer::EngineConfig {
        crate::hfst_transducer::EngineConfig {
            flag_is_epsilon_in_composition: self.flag_is_epsilon,
            xerox_composition: self.xerox_composition,
            ..crate::hfst_transducer::EngineConfig::default()
        }
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.set-warning-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.set-warning-fn]
    pub fn set_warning(&mut self, warning: &str, value: bool) {
        match warning {
            "-Wone-sided-flags" => self.warn_one_sided_flags = value,
            "-Wmissing-lexicons" => self.warn_missing_lexicons = value,
            "-Wunused-lexicons" => self.warn_unused_lexicons = value,
            "-Wrepeated-lexicons" => self.warn_repeated_lexicons = value,
            "-Wmissing-alphabets" => self.warn_missing_alphabets = value,
            "-Wunnecessary-escapes" => self.warn_unnecessary_escapes = value,
            _ => error!("unknown warning {}", warning),
        }
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.set-error-stream-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.set-error-stream-fn]
    /// The C++ stored an 'std::ostream*' and forwarded it to 'xre'; the port
    /// drops the stream plumbing (errors go to stderr), so this is a no-op.
    pub fn set_error_stream<T>(&mut self, _os: T) {}

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.get-error-stream-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.get-error-stream-fn]
    /// The C++ returned the stored 'error_' ostream pointer; the port drops the
    /// stream plumbing (errors go to stderr), so there is no stored stream to
    /// return.
    pub fn get_error_stream(&self) {}

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.get-stream-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.get-stream-fn]
    /// On non-WINDOWS the C++ 'get_stream' just returns its argument; the WINDOWS
    /// console-redirection branch is dropped along with the rest of the stream
    /// plumbing, so this returns the passed stream unchanged.
    pub fn get_stream<T>(&mut self, oss: T) -> T {
        oss
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.flush-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.flush-fn]
    /// On non-WINDOWS the C++ 'flush' is a no-op ('(void)oss'); the WINDOWS
    /// console-flush branch is dropped with the rest of the stream plumbing.
    pub fn flush<T>(&mut self, _oss: T) {}

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.set-output-to-console-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.set-output-to-console-fn]
    /// On non-WINDOWS the C++ 'setOutputToConsole' is a no-op ('(void)value');
    /// the WINDOWS-only 'output_to_console' field is dropped from the port.
    pub fn set_output_to_console(&mut self, _value: bool) {}

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.get-output-to-console-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.get-output-to-console-fn]
    /// On non-WINDOWS the C++ 'getOutputToConsole' always returns false; the
    /// WINDOWS-only 'output_to_console' field is dropped from the port.
    pub fn get_output_to_console(&self) -> bool {
        false
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.is-quiet-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.is-quiet-fn]
    pub fn is_quiet(&self) -> bool {
        self.quiet
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.are-warnings-treated-as-errors-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.are-warnings-treated-as-errors-fn]
    pub fn are_warnings_treated_as_errors(&self) -> bool {
        self.treat_warnings_as_errors
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.is-strict-alphabets-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.is-strict-alphabets-fn]
    pub fn is_strict_alphabets(&self) -> bool {
        self.warn_missing_alphabets
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.set-strict-alphabets-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.set-strict-alphabets-fn]
    pub fn set_strict_alphabets(&mut self, strictness: bool) {
        self.warn_missing_alphabets = strictness;
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.has-split-characters-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.has-split-characters-fn]
    pub fn has_split_characters(&self) -> bool {
        self.split_characters
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.set-split-characters-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.set-split-characters-fn]
    pub fn set_split_characters(&mut self, splitness: bool) {
        self.split_characters = splitness;
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.is-warning-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.is-warning-fn]
    pub fn is_warning(&self, warning: &str) -> bool {
        if warning == "-Wone-sided-flags" {
            self.warn_one_sided_flags
        } else if warning == "-Wmissing-lexicons" {
            self.warn_missing_lexicons
        } else if warning == "-Wunused-lexicons" {
            self.warn_unused_lexicons
        } else if warning == "-Wrepeated-lexicons" {
            self.warn_repeated_lexicons
        } else if warning == "-Wmissing-alphabets" {
            self.warn_missing_alphabets
        } else if warning == "-Wunnecessary-escapes" {
            self.warn_unnecessary_escapes
        } else {
            error!("unknown warning {}", warning);
            false
        }
    }

    // ----- error / warning helpers (lexc-utils.cc, re-entrant) -----

    // [spec:hfst:def:lexc-utils.hfst.lexc.error-at-current-token-fn]
    // [spec:hfst:sem:lexc-utils.hfst.lexc.error-at-current-token-fn]
    /// The C++ free function printed Flex token positions; the AST-walk port has
    /// no hand-lexer position state, so it just writes the message to stderr.
    fn error_at_current_token(&self, format: &str) {
        crate::diag::emit(
            &self.source_name,
            &self.source,
            self.current_span.clone(),
            crate::diag::Severity::Error,
            format,
        );
    }

    // [spec:hfst:def:lexc-utils.hfst.lexc.warning-at-current-token-fn]
    // [spec:hfst:sem:lexc-utils.hfst.lexc.warning-at-current-token-fn]
    fn warning_at_current_token(&self, format: &str) {
        crate::diag::emit(
            &self.source_name,
            &self.source,
            self.current_span.clone(),
            crate::diag::Severity::Warning,
            format,
        );
    }

    // [spec:hfst:def:lexc-utils.hfst.lexc.strip-percents-fn]
    // [spec:hfst:sem:lexc-utils.hfst.lexc.strip-percents-fn]
    //
    // Port of the char*-returning 'hfst::lexc::strip_percents(const char *s,
    // bool do_zeros)' (distinct from the std::string '&'-returning
    // 'stripPercents', which is ported as the 'strip_percents_str' free fn).
    // The C++ reached the 'lexc' singleton for warnings; the re-entrant port
    // takes '&mut self'. NULL becomes 'None'. The computed-but-unused 'err'
    // ostream at the top of the C++ body is dropped with the rest of the stream
    // plumbing (errors go to stderr).
    fn strip_percents(&mut self, s: &str, do_zeros: bool) -> Option<String> {
        let bytes = s.as_bytes();
        let mut rv: Vec<u8> = Vec::new();
        let mut c: usize = 0;
        let mut escaping = false;
        let mut in_at = false;
        while c < bytes.len() && bytes[c] != b'\0' {
            let cb = bytes[c];
            if in_at {
                if cb == b'@' {
                    in_at = false;
                }
                rv.push(cb);
                c += 1;
            } else if escaping {
                if cb != b'0' {
                    if (cb != b':')
                        && (cb != b'<')
                        && (cb != b' ')
                        && (cb != b';')
                        && (cb != b'%')
                        && (cb != b'"')
                        && (cb != b'@')
                        && (cb != b'!')
                        && (cb != b'>')
                        && (cb != b'#')
                    {
                        let errmsg = if (cb as i8) > 0 {
                            format!("Unnecessary escape %{} [-Wunnecessary-escapes]", cb as char)
                        } else {
                            let rest = String::from_utf8_lossy(&bytes[c..]).into_owned();
                            format!("Unnecessary escape %{} [-Wunnecessary-escapes]", rest)
                        };
                        if self.is_warning("-Wunnecessary-escapes")
                            && self.are_warnings_treated_as_errors()
                        {
                            self.error_at_current_token(&errmsg);
                            self.parseErrors_ = true;
                        } else if self.is_warning("-Wunnecessary-escapes") {
                            self.warning_at_current_token(&errmsg);
                        }
                    }
                    rv.push(cb);
                } else {
                    let escaped = b"@ZERO@";
                    for &e in escaped {
                        rv.push(e);
                    }
                }
                escaping = false;
                c += 1;
            } else if cb == b'%' {
                escaping = true;
                c += 1;
            } else if cb == b'@' {
                in_at = true;
                rv.push(cb);
                c += 1;
            } else if do_zeros && (cb == b'0') {
                let escaped = b"@0@";
                for &e in escaped {
                    rv.push(e);
                }
                c += 1;
            } else {
                rv.push(cb);
                c += 1;
            }
        }
        if escaping {
            // fprintf(stderr, "Stray escape char %% in %s", s);
            self.warning_at_current_token("Stray escape char %%\n");
            return None;
        }
        Some(String::from_utf8_lossy(&rv).into_owned())
    }
}

impl<B: AlgebraBackend> Default for LexcCompiler<B> {
    fn default() -> Self {
        Self::new()
    }
}
