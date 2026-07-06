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
//! - ICU grapheme segmentation inside 'unicodeCheck_'.

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
use crate::hfst_data_types::{
    ImplementationType, StringPair, StringPairVector, StringVector, Symbol,
};
use crate::hfst_symbol_defs::{HfstSymbolSubstitutions, StringSet};
use crate::hfst_tokenizer::HfstTokenizer;
use crate::hfst_transducer::HfstTransducer;
use crate::xre::XreCompiler;
use tracing::{debug, error, info, warn};

// [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler]
pub struct LexcCompiler<B: AlgebraBackend> {
    pub(crate) tokenizer: HfstTokenizer,
    pub(crate) xre: XreCompiler<B>,
    pub(crate) initialLexiconName_: String,
    pub(crate) currentLexiconName_: String,
    pub(crate) stringsTrie_: HfstBasicTransducer,
    pub(crate) regexps: BTreeMap<String, HfstTransducer<B>>, // owning HfstTransducer* -> owned
    pub(crate) lexiconNames_: BTreeSet<String>,
    pub(crate) noFlags_: BTreeSet<String>,
    pub(crate) continuations: BTreeSet<String>,
    pub(crate) alphabets: BTreeSet<String>,
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
}
// (followed by a doc-comment roster of every method + lexc-utils helper the
//  bodies fill — public API, AST-walk driver compile/compile_file, and the
//  lexc-utils free helpers with their [spec:hfst:...] annotations.)

// ===== body 0 (flattened, module scope) =====

// ==========================================================================
// lexc-utils.cc — module-scope constants (ported from lexc-utils.h).
// Declared here (the lexc-utils helper body) rather than in the skeleton to
// avoid duplicate-definition collisions.
// ==========================================================================

const LEXC_JOINER_START: &str = "$_LEXC_JOINER.";
const LEXC_JOINER_END: &str = "_$";
const LEXC_FLAG_LEFT_START: &str = "$R.LEXNAME.";
const LEXC_FLAG_RIGHT_START: &str = "$P.LEXNAME.";
const LEXC_FLAG_END: &str = "$";
const LEXC_DFN_START: &str = "@_LEXC_DEFINITION.";
const LEXC_DFN_END: &str = "_@";
const REG_EX_START: &str = "$_REG.";
const REG_EX_END: &str = "_$";

// 'EPSILON_' and the med-alignment direction codes back 'find_med_alingment'.
const EPSILON_: &str = "@@ANOTHER_EPSILON@@";
const SUBSTITUTE: u32 = 2; // diag
const DELETE: u32 = 1; // left
const INSERT: u32 = 0; // down

// ==========================================================================
// lexc-utils.cc — RECODE LEXC STYLE free helpers.
//
// The C++ encoders mutate-and-return a 'std::string&'; here they are pure
// 'fn(&str, …) -> String'. These module-scope helpers are shared with the
// 'compileLexical' body (which calls 'joiner_encode' / 'flag_joiner_encode' /
// 'should_colourise'); they are owned by this registration/lexc-utils body.
// ==========================================================================

/// Port of 'hfst::lexc::stripPercents'.
fn strip_percents_str(s: &str) -> String {
    let stripped = s.replace("%%", "@PERCENT@");
    let stripped = stripped.replace('%', "");
    stripped.replace("@PERCENT@", "%")
}

/// Port of 'hfst::lexc::addPercents'.
fn add_percents(s: &str) -> String {
    let added = s.replace('%', "%%");
    let added = added.replace('<', "%<");
    added.replace('>', "%>")
}

/// Port of 'hfst::lexc::flagJoinerEncode'.
fn flag_joiner_encode(s: &str, left: bool) -> String {
    if left {
        format!("{}{}{}", LEXC_FLAG_LEFT_START, s, LEXC_FLAG_END)
    } else {
        format!("{}{}{}", LEXC_FLAG_RIGHT_START, s, LEXC_FLAG_END)
    }
}

/// Port of 'hfst::lexc::joinerEncode'.
fn joiner_encode(s: &str) -> String {
    format!("{}{}{}", LEXC_JOINER_START, s, LEXC_JOINER_END)
}

/// Port of 'hfst::lexc::joinerDecode'.
fn joiner_decode(s: &str) -> String {
    let j_start = LEXC_JOINER_START.len();
    let j_end = LEXC_JOINER_END.len();
    s[j_start..s.len() - j_end].to_string()
}

/// Port of 'hfst::lexc::regExpresionEncode' (suffix is 'LEXC_JOINER_END', as in
/// the C++).
fn reg_expresion_encode(s: &str) -> String {
    format!("{}{}{}", REG_EX_START, s, LEXC_JOINER_END)
}

/// Port of 'hfst::lexc::regExpresionDecode'.
fn reg_expresion_decode(s: &str) -> String {
    let j_start = REG_EX_START.len();
    let j_end = LEXC_JOINER_END.len();
    s[j_start..s.len() - j_end].to_string()
}

/// Port of 'hfst::lexc::xreDefinitionEncode'.
fn xre_definition_encode(s: &str) -> String {
    format!("{}{}{}", LEXC_DFN_START, s, LEXC_DFN_END)
}

// replaces the first '@ZERO@' with "0" in a string
// [spec:hfst:def:lexc-utils.hfst.lexc.replace-zero-fn]
// [spec:hfst:sem:lexc-utils.hfst.lexc.replace-zero-fn]
fn replace_zero(s: &str) -> String {
    if let Some(start_pos) = s.find("@ZERO@") {
        let mut str = s.to_string();
        str.replace_range(start_pos..start_pos + "@ZERO@".len(), "0");
        str
    } else {
        s.to_string()
    }
}

// [spec:hfst:def:lexc-utils.hfst.lexc.should-colourise-fn]
// [spec:hfst:sem:lexc-utils.hfst.lexc.should-colourise-fn]
// The identical 'should_colourise' static is duplicated in LexcCompiler.cc; the
// single Rust helper ports both copies.
// [spec:hfst:def:lexc-compiler.should-colourise-fn]
// [spec:hfst:sem:lexc-compiler.should-colourise-fn]
fn should_colourise() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stdout())
}

// [spec:hfst:def:lexc-utils.hfst.lexc.find-med-alingment-fn]
// [spec:hfst:sem:lexc-utils.hfst.lexc.find-med-alingment-fn]
fn find_med_alingment(s1: &[String], s2: &[String]) -> (Vec<String>, Vec<String>) {
    let substitution: i64 = 100;
    let deletion: i64 = 1;
    let insertion: i64 = 1;

    let len1 = s1.len();
    let len2 = s2.len();
    let mut d = vec![vec![0u32; len2 + 1]; len1 + 1];
    let mut dir = vec![vec![0u32; len2 + 1]; len1 + 1];
    d[0][0] = 0;
    dir[0][0] = 0;
    for i in 1..=len1 {
        d[i][0] = (deletion * i as i64) as u32;
        dir[i][0] = DELETE;
    }
    for i in 1..=len2 {
        d[0][i] = (insertion * i as i64) as u32;
        dir[0][i] = INSERT;
    }

    for i in 1..=len1 {
        for j in 1..=len2 {
            let sub = d[i - 1][j - 1] as i64
                + (if s1[i - 1] == s2[j - 1] {
                    0
                } else {
                    substitution
                });
            let ins = d[i][j - 1] as i64 + insertion;
            let del = d[i - 1][j] as i64 + deletion;

            if sub <= ins && sub <= del {
                d[i][j] = sub as u32;
                dir[i][j] = SUBSTITUTE;
            }
            // Prioritise "del" over "ins" on ties so the first string gets its
            // zeroes before the second one (matches the C++ comment).
            else if del <= sub && del <= ins {
                d[i][j] = del as u32;
                dir[i][j] = DELETE;
            } else {
                d[i][j] = ins as u32;
                dir[i][j] = INSERT;
            }
        }
    }

    let mut medcwordin: Vec<String> = Vec::new();
    let mut medcwordout: Vec<String> = Vec::new();

    let mut x = i32::try_from(s1.len()).expect("value out of i32 range");
    let mut y = i32::try_from(s2.len()).expect("value out of i32 range");
    while x > 0 || y > 0 {
        let dir_value = dir[x as usize][y as usize];

        if dir_value == SUBSTITUTE {
            medcwordin.push(s1[(x - 1) as usize].clone());
            medcwordout.push(s2[(y - 1) as usize].clone());
            x -= 1;
            y -= 1;
        } else if dir_value == INSERT {
            medcwordin.push(EPSILON_.to_string());
            medcwordout.push(s2[(y - 1) as usize].clone());
            y -= 1;
        } else {
            medcwordin.push(s1[(x - 1) as usize].clone());
            medcwordout.push(EPSILON_.to_string());
            x -= 1;
        }
    }

    medcwordin.reverse();
    medcwordout.reverse();
    (medcwordin, medcwordout)
}

/// Parse a weight out of an entry gloss formatted '"weight: N"'.
///
/// Mirrors 'handle_string_entry_common' in 'lexc-parser.yy': locate the
/// 'weight:' marker, then the leading numeric run (chars in '-0.123456789'),
/// and parse it. Anything else yields '0.0'.
fn weight_from_gloss(gloss: Option<&str>) -> f64 {
    let gloss = match gloss {
        Some(g) if !g.is_empty() => g,
        _ => return 0.0,
    };
    let wstart_marker = match gloss.find("weight:") {
        Some(p) => p,
        None => return 0.0,
    };
    let digits = "-0.123456789";
    let rest = &gloss[wstart_marker..];
    let off = match rest.find(|c: char| digits.contains(c)) {
        Some(p) => wstart_marker + p,
        None => return 0.0,
    };
    let after = &gloss[off..];
    let end = match after.find(|c: char| !digits.contains(c)) {
        Some(p) => off + p,
        None => gloss.len(),
    };
    gloss[off..end].parse::<f64>().unwrap_or(0.0)
}

// ==========================================================================
// LexcCompiler — constructors, option setters, incremental registration API,
// and the AST-walk driver (ported from LexcCompiler.cc).
// ==========================================================================

impl<B: AlgebraBackend> LexcCompiler<B> {
    /// Common body of the 'LexcCompiler(impl)' and
    /// 'LexcCompiler(impl, withFlags, alignStrings)' constructors: seeds the
    /// tokenizer with the epsilon/zero multichars + the '#' joiner, registers
    /// '#' as a lexicon name, and configures 'xre'.
    fn seeded() -> LexcCompiler<B> {
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
        compiler.lexiconNames_.insert(hash.clone());
        let enc = joiner_encode(&hash);
        compiler.tokenizer.add_multichar_symbol(&enc);
        compiler.xre.set_expand_definitions(true);
        compiler.xre.set_verbosity(!compiler.quiet);
        compiler
    }

    /// Port of 'LexcCompiler(ImplementationType impl)' (unannotated in the .cc).
    pub fn new() -> LexcCompiler<B> {
        LexcCompiler::seeded()
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.lexc-compiler-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.lexc-compiler-fn]
    /// Port of 'LexcCompiler(impl, withFlags, alignStrings)'.
    pub fn new_with_flags(with_flags: bool, align_strings: bool) -> LexcCompiler<B> {
        let mut compiler = LexcCompiler::seeded();
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
        self.currentLexiconName_ = String::new(); // ?
        self.lexiconNames_.insert("#".to_string());
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

    /// Port of 'LexcCompiler::unicodeCheck_'. The ICU grapheme-segmentation
    /// guard (auto-adding multi-codepoint graphemes to 'alphabets') is deferred;
    /// for the common single-codepoint-grapheme path the C++ adds nothing, so
    /// this is faithful for ASCII and only skips the multi-codepoint warning.
    fn unicode_check(&mut self, _data: &str) -> &mut Self {
        if self.split_characters {
            return self;
        }
        self
    }

    // ----- incremental registration API -----

    pub fn add_no_flag(&mut self, lexname: &str) -> &mut Self {
        self.noFlags_.insert(lexname.to_string());
        self
    }

    pub fn add_alphabet(&mut self, alpha: &str) -> &mut Self {
        self.alphabets.insert(alpha.to_string());
        self.tokenizer.add_multichar_symbol(alpha);
        if !self.quiet && self.verbose {
            // warn about undefined multichars
            self.xre.add_defined_multichar_symbol(alpha);
        }
        self
    }

    /// Construct vector 'nameJoiner data contJoiner' and add it to the trie.
    pub fn add_string_entry(&mut self, data: &str, continuation: &str, weight: f64) -> &mut Self {
        self.currentEntries_ += 1;
        self.totalEntries_ += 1;
        self.unicode_check(data);
        self.continuations.insert(continuation.to_string());
        let encoded_cont = if self.with_flags {
            if !self.noFlags_.contains(continuation) {
                flag_joiner_encode(continuation, false)
            } else {
                joiner_encode(continuation)
            }
        } else {
            joiner_encode(continuation)
        };
        self.tokenizer.add_multichar_symbol(&encoded_cont);

        // build string pair vector map
        let cur = self.currentLexiconName_.clone();
        let joiner_enc = if self.with_flags {
            if !self.noFlags_.contains(&cur) {
                flag_joiner_encode(&cur, true)
            } else {
                joiner_encode(&cur)
            }
        } else {
            joiner_encode(&cur)
        };
        self.tokenizer.add_multichar_symbol(&joiner_enc);
        self.tokenizer.add_multichar_symbol("0"); // epsilon
        self.tokenizer.add_multichar_symbol("@ZERO@"); // literal zero
        let mut new_vector = self.tokenizer.tokenize(
            &format!("{}{}{}", joiner_enc, data, encoded_cont),
            self.split_characters,
        );
        // "0"      -> "@0@"  (single symbols)
        // "@ZERO@" -> "0"    (everywhere)
        let mut i = 0;
        while i < new_vector.len() {
            if new_vector[i].0 == "0" {
                new_vector[i].0 = Symbol::new_static("@0@");
            }
            if let Some(start_pos) = new_vector[i].0.find("@ZERO@") {
                let mut s = new_vector[i].0.to_string();
                s.replace_range(start_pos..start_pos + "@ZERO@".len(), "0");
                new_vector[i].0 = Symbol::from(s);
            }
            if new_vector[i].1 == "0" {
                new_vector[i].1 = Symbol::new_static("@0@");
            }
            if let Some(start_pos) = new_vector[i].1.find("@ZERO@") {
                let mut s = new_vector[i].1.to_string();
                s.replace_range(start_pos..start_pos + "@ZERO@".len(), "0");
                new_vector[i].1 = Symbol::from(s);
            }
            let first = new_vector[i].0.clone();
            if !self.alphabets.contains(first.as_str()) {
                if first.starts_with('@') && first.ends_with('@') {
                    i += 1;
                    continue;
                }
                if first.starts_with('$') && first.ends_with('$') {
                    i += 1;
                    continue;
                }
                if self.warn_missing_alphabets {
                    let errm = format!("Adding {} to Alphabets [Wmissing-alphabets]", first);
                    if self.treat_warnings_as_errors {
                        self.error_at_current_token(&errm);
                        self.parseErrors_ = true;
                    } else {
                        self.warning_at_current_token(&errm);
                    }
                }
                self.add_alphabet(&first);
            }
            i += 1;
        }
        let w = weight as f32;
        self.stringsTrie_.disjunct_path(&new_vector, w);
        self
    }

    // callback function to stuff so static and uses global singleton :-(
    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.warn-about-one-sided-flags-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.warn-about-one-sided-flags-fn]
    /// In the C++ this was a static tokenize callback reaching the 'lexc'
    /// global; the re-entrant port applies it to the tokenized pairs directly.
    fn warn_about_one_sided_flags(&mut self, symbol_pair: &StringPair) {
        if crate::hfst_flag_diacritics::FdOperation::is_diacritic(&symbol_pair.0) {
            if symbol_pair.0 != symbol_pair.1 {
                let errm = format!(
                    "one-sided flag diacritic {}:{} [-Wone-sided-flags]",
                    symbol_pair.0, symbol_pair.1
                );
                if self.warn_one_sided_flags && self.treat_warnings_as_errors {
                    self.error_at_current_token(&errm);
                    self.parseErrors_ = true;
                }
                if self.warn_one_sided_flags {
                    self.warning_at_current_token(&errm);
                }
            }
            return;
        } else if crate::hfst_flag_diacritics::FdOperation::is_diacritic(&symbol_pair.1) {
            let errm = format!(
                "one-sided flag diacritic {}:{} [-Wone-sided-flags]",
                symbol_pair.0, symbol_pair.1
            );
            if self.warn_one_sided_flags && self.treat_warnings_as_errors {
                self.error_at_current_token(&errm);
                self.parseErrors_ = true;
            }
            if self.warn_one_sided_flags {
                self.warning_at_current_token(&errm);
            }
        }
    }

    pub fn add_string_pair_entry(
        &mut self,
        upper: &str,
        lower: &str,
        continuation: &str,
        weight: f64,
    ) -> &mut Self {
        self.currentEntries_ += 1;
        self.totalEntries_ += 1;
        self.unicode_check(upper);
        self.unicode_check(lower);
        self.continuations.insert(continuation.to_string());
        let encoded_cont = if self.with_flags {
            if !self.noFlags_.contains(continuation) {
                flag_joiner_encode(continuation, false)
            } else {
                joiner_encode(continuation)
            }
        } else {
            joiner_encode(continuation)
        };
        self.tokenizer.add_multichar_symbol(&encoded_cont);

        // build string pair vector map
        let cur = self.currentLexiconName_.clone();
        let joiner_enc = if self.with_flags {
            if !self.noFlags_.contains(&cur) {
                flag_joiner_encode(&cur, true)
            } else {
                joiner_encode(&cur)
            }
        } else {
            joiner_encode(&cur)
        };
        self.tokenizer.add_multichar_symbol(&joiner_enc);
        self.tokenizer.add_multichar_symbol("0"); // epsilon
        self.tokenizer.add_multichar_symbol("@ZERO@"); // literal zero

        let mut new_vector: StringPairVector;

        if self.align_strings {
            let tmp = self
                .tokenizer
                .tokenize_pair(upper, lower, self.split_characters);
            let mut one: Vec<String> = Vec::new();
            let mut two: Vec<String> = Vec::new();
            for it in &tmp {
                if it.0 != "@_EPSILON_SYMBOL_@" {
                    one.push(it.0.to_string());
                }
                if it.1 != "@_EPSILON_SYMBOL_@" {
                    two.push(it.1.to_string());
                }
            }

            let med_vectors = find_med_alingment(&one, &two);
            let as1: String = med_vectors.0.concat();
            let as2: String = med_vectors.1.concat();

            new_vector = self.tokenizer.tokenize_pair(
                &format!("{}{}{}", joiner_enc, as1, encoded_cont),
                &format!("{}{}{}", joiner_enc, as2, encoded_cont),
                self.split_characters,
            );
        } else {
            let upper_v = self.tokenizer.tokenize(upper, self.split_characters);
            let lower_v = self.tokenizer.tokenize(lower, self.split_characters);

            let upper_size = i32::try_from(upper_v.len()).expect("value out of i32 range");
            let lower_size = i32::try_from(lower_v.len()).expect("value out of i32 range");

            if upper_size > lower_size {
                let mut epsilons = String::new();
                for _ in 1..=(upper_size - lower_size) {
                    epsilons.push_str("@@ANOTHER_EPSILON@@");
                }
                new_vector = self.tokenizer.tokenize_pair(
                    &format!("{}{}{}", joiner_enc, upper, encoded_cont),
                    &format!("{}{}{}{}", joiner_enc, lower, epsilons, encoded_cont),
                    self.split_characters,
                );
            } else if upper_size < lower_size {
                let mut epsilons = String::new();
                for _ in 1..=(lower_size - upper_size) {
                    epsilons.push_str("@@ANOTHER_EPSILON@@");
                }
                new_vector = self.tokenizer.tokenize_pair(
                    &format!("{}{}{}{}", joiner_enc, upper, epsilons, encoded_cont),
                    &format!("{}{}{}", joiner_enc, lower, encoded_cont),
                    self.split_characters,
                );
            } else {
                new_vector = self.tokenizer.tokenize_pair(
                    &format!("{}{}{}", joiner_enc, upper, encoded_cont),
                    &format!("{}{}{}", joiner_enc, lower, encoded_cont),
                    self.split_characters,
                );
            }
        }

        // The C++ passed 'warn_about_one_sided_flags' as the tokenize callback;
        // the re-entrant port tokenizes plainly then applies it to each pair.
        for idx in 0..new_vector.len() {
            let sp = new_vector[idx].clone();
            self.warn_about_one_sided_flags(&sp);
        }

        let mut i = 0;
        while i < new_vector.len() {
            if new_vector[i].0 == "0" {
                new_vector[i].0 = Symbol::new_static("@0@");
            }
            if let Some(start_pos) = new_vector[i].0.find("@ZERO@") {
                let mut s = new_vector[i].0.to_string();
                s.replace_range(start_pos..start_pos + "@ZERO@".len(), "0");
                new_vector[i].0 = Symbol::from(s);
            }
            if new_vector[i].1 == "0" {
                new_vector[i].1 = Symbol::new_static("@0@");
            }
            if let Some(start_pos) = new_vector[i].1.find("@ZERO@") {
                let mut s = new_vector[i].1.to_string();
                s.replace_range(start_pos..start_pos + "@ZERO@".len(), "0");
                new_vector[i].1 = Symbol::from(s);
            }
            let first = new_vector[i].0.clone();
            if !self.alphabets.contains(first.as_str()) {
                if first.starts_with('@') && first.ends_with('@') {
                    i += 1;
                    continue;
                }
                if first.starts_with('$') && first.ends_with('$') {
                    i += 1;
                    continue;
                }
                if self.warn_missing_alphabets {
                    let errm = format!("Adding {} to Alphabets [-Wmissing-alphabets]", first);
                    if self.treat_warnings_as_errors {
                        self.error_at_current_token(&errm);
                        self.parseErrors_ = true;
                    } else {
                        self.warning_at_current_token(&errm);
                    }
                }
                self.add_alphabet(&first);
            }
            let second = new_vector[i].1.clone();
            if !self.alphabets.contains(second.as_str()) {
                if second.starts_with('@') && second.ends_with('@') {
                    i += 1;
                    continue;
                }
                if second.starts_with('$') && second.ends_with('$') {
                    i += 1;
                    continue;
                }
                if self.warn_missing_alphabets {
                    let errm = format!("Adding {} to Alphabets [-Wmissing-alphabets]", second);
                    if self.treat_warnings_as_errors {
                        self.error_at_current_token(&errm);
                        self.parseErrors_ = true;
                    } else {
                        self.warning_at_current_token(&errm);
                    }
                }
                self.add_alphabet(&second);
            }
            i += 1;
        }

        let w = weight as f32;
        self.stringsTrie_.disjunct_path(&new_vector, w);
        self
    }

    /// Construct transducer 'nameJoiner XRE contJoiner' and add it to the trie.
    pub fn add_xre_entry(
        &mut self,
        regexp: &str,
        continuation: &str,
        weight: f64,
    ) -> crate::error::Result<&mut Self> {
        self.currentEntries_ += 1;
        self.totalEntries_ += 1;
        self.continuations.insert(continuation.to_string());
        let encoded_cont = if self.with_flags {
            if !self.noFlags_.contains(continuation) {
                flag_joiner_encode(continuation, false)
            } else {
                joiner_encode(continuation)
            }
        } else {
            joiner_encode(continuation)
        };
        self.tokenizer.add_multichar_symbol(&encoded_cont);

        let Some(mut new_paths) = self.xre.compile(regexp) else {
            self.error_at_current_token("Unable to parse regular expression");
            self.parseErrors_ = true;
            return Ok(self);
        };
        new_paths.optimize()?;
        let new_alphabets = new_paths.get_alphabet()?;
        for new_alpha in &new_alphabets {
            if self.alphabets.contains(new_alpha.as_str()) {
                continue;
            }
            if matches!(
                new_alpha.as_str(),
                "@_EPSILON_SYMBOL_@" | "@_UNKNOWN_SYMBOL_@" | "@_IDENTITY_SYMBOL_@"
            ) {
                continue;
            }
            let errm = format!(
                "implicit Alphabet {} in regex [-Wmissing-alphabets]",
                new_alpha
            );
            if new_alpha.chars().count() > 1 {
                self.warning_at_current_token(&errm);
                warn!("you shoudl add {} to Multichar_Symbols section", new_alpha);
            } else if self.warn_missing_alphabets && self.treat_warnings_as_errors {
                self.error_at_current_token(&errm);
                self.parseErrors_ = true;
            } else if self.warn_missing_alphabets {
                self.warning_at_current_token(&errm);
            }
            self.add_alphabet(new_alpha);
        }

        // encode key; keep regexps with different continuations separate
        let mut regex_key = format!("{}_{}", self.currentLexiconName_, continuation);
        regex_key = reg_expresion_encode(&regex_key);
        self.tokenizer.add_multichar_symbol(&regex_key);

        let entry = match self.regexps.entry(regex_key.clone()) {
            std::collections::btree_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::btree_map::Entry::Vacant(e) => e.insert(HfstTransducer::new()),
        };
        entry.disjunct(&new_paths, true)?.optimize()?;

        if !self.quiet && (self.currentEntries_ % 10000) == 0 {
            info!("{}...", self.currentEntries_);
        }

        // add key to trie
        let cur = self.currentLexiconName_.clone();
        let joiner_enc = if self.with_flags {
            if !self.noFlags_.contains(&cur) {
                flag_joiner_encode(&cur, true)
            } else {
                joiner_encode(&cur)
            }
        } else {
            joiner_encode(&cur)
        };
        self.tokenizer.add_multichar_symbol(&joiner_enc);
        let new_vector = self.tokenizer.tokenize(
            &format!("{}{}{}", joiner_enc, regex_key, encoded_cont),
            false,
        );
        let w = weight as f32;
        self.stringsTrie_.disjunct_path(&new_vector, w);
        Ok(self)
    }

    pub fn add_xre_definition(&mut self, definition_name: &str, xre: &str) -> &mut Self {
        // FIXME: collect implicit characters
        self.xre.define(definition_name, xre);
        if !self.quiet {
            info!(
                "Defined '{}': ? Kb., ? states, ? arcs, ? paths.",
                definition_name
            );
        }
        self
    }

    pub fn set_current_lexicon_name(&mut self, lexicon_name: &str) -> &mut Self {
        self.currentLexiconName_ = lexicon_name.to_string();

        if self.lexiconNames_.contains(lexicon_name) {
            if !self.warn_repeated_lexicons && self.treat_warnings_as_errors {
                self.error_at_current_token(
                    "Lexicon is defined more than once! [-Wrepeated-lexicons]",
                );
                self.parseErrors_ = true;
            } else if self.warn_repeated_lexicons {
                self.warning_at_current_token(
                    "Lexicon is defined more than once! [-Wrepeated-lexicons]",
                );
            }
        }

        self.lexiconNames_.insert(lexicon_name.to_string());
        if !self.noFlags_.contains(lexicon_name) {
            // [spec:hfst:def:lexc-compiler.hfst.lexc.encoded-name-fn]
            // [spec:hfst:sem:lexc-compiler.hfst.lexc.encoded-name-fn]
            // NOTE: faithful to the C++, the second encode is applied to the
            // already-'$P'-encoded string (flagJoinerEncode mutated in place).
            let mut encoded_name = flag_joiner_encode(lexicon_name, false);
            self.tokenizer.add_multichar_symbol(&encoded_name);
            encoded_name = flag_joiner_encode(&encoded_name, true);
            self.tokenizer.add_multichar_symbol(&encoded_name);
        } else {
            let encoded_name = joiner_encode(lexicon_name);
            self.tokenizer.add_multichar_symbol(&encoded_name);
        }

        if self.first_lexicon && lexicon_name == "Root" {
            self.set_initial_lexicon_name(lexicon_name);
        } else if self.first_lexicon && lexicon_name != "Root" {
            if self.treat_warnings_as_errors {
                self.error_at_current_token("first lexicon is not named Root");
                self.parseErrors_ = true;
            } else {
                self.warning_at_current_token("first lexicon is not named Root");
            }
            self.set_initial_lexicon_name(lexicon_name);
        } else if !self.first_lexicon && lexicon_name == "Root" {
            if self.treat_warnings_as_errors {
                self.error_at_current_token("Root is not first the first lexicon");
                self.parseErrors_ = true;
            } else {
                self.warning_at_current_token("Root is not first the first lexicon");
            }
            self.set_initial_lexicon_name(lexicon_name);
        }
        if !self.quiet {
            let mut line = String::new();
            if !self.first_lexicon {
                line.push_str(&format!("{} ", self.currentEntries_));
            }
            line.push_str(&format!("{}...", lexicon_name));
            info!("{}", line);
        }
        self.first_lexicon = false;

        self.currentEntries_ = 0;
        self
    }

    pub fn set_initial_lexicon_name(&mut self, lexicon_name: &str) -> &mut Self {
        self.initialLexiconName_ = lexicon_name.to_string();
        self.lexiconNames_.insert(lexicon_name.to_string());
        // for connectedness calculation:
        self.continuations.insert(lexicon_name.to_string());
        self
    }

    // ----- AST-walk driver (replaces parse(FILE*) / parse(filename)) -----

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.parse-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.parse-fn]
    /// INCREMENTAL entry point: parse 'lexc_source' via 'nfst_lexc::parse' and
    /// walk the typed AST through the registration API, accumulating its
    /// lexicons/entries into 'self' (the trie 'stringsTrie_', 'regexps',
    /// 'lexiconNames_', etc.). This is the AST-walk port of the Flex/Bison
    /// 'parse(FILE*)' / 'parse(const char*)': both '.cc' overloads ran
    /// 'hlexcparse()' (accumulating into the singleton) then
    /// 'xre.remove_defined_multichar_symbols()' and set 'parseErrors_' on
    /// failure. It does NOT run 'compileLexical': call 'compile_lexical' once
    /// after every source has been parsed. Multi-file flow: call 'parse' on each
    /// source into one compiler, then 'compile_lexical' once. Returns '&mut self'
    /// to mirror the C++ 'LexcCompiler &' chaining return.
    /// Name shown in source-anchored diagnostics (the lexc file name). Set
    /// before `parse` so warnings point at the right file; defaults to
    /// `"<lexc>"`.
    pub fn set_source_name(&mut self, name: &str) -> &mut Self {
        self.source_name = name.to_string();
        self
    }

    pub fn parse(&mut self, lexc_source: &str) -> crate::error::Result<&mut Self> {
        // Retain the source so diagnostics can render the offending snippet.
        self.source = lexc_source.to_string();
        match nfst_lexc::parse(lexc_source) {
            Ok(ast) => {
                self.compile_file(&ast.value)?;
                // mirrors 'xre.remove_defined_multichar_symbols()' in parse()
                self.xre.remove_defined_multichar_symbols();
            }
            Err(_e) => {
                // mirrors the 'hlexcnerrs > 0' branch setting parseErrors_
                self.parseErrors_ = true;
            }
        }
        Ok(self)
    }

    /// PUBLIC entry point: parse a single 'lexc_source' into 'self', then run
    /// 'compileLexical'. This is the AST-walk port of the Flex/Bison driver where
    /// tools called 'parse(...)' followed by 'compileLexical()'; it is exactly
    /// that pairing for the single-source case. Returns None on parse error
    /// (the C++ 'compileLexical' null contract expressed as an Option).
    pub fn compile(&mut self, lexc_source: &str) -> Option<HfstTransducer<B>> {
        self.parse(lexc_source).ok();
        self.compile_lexical().ok().flatten()
    }

    /// Walk the typed AST, dispatching each section/entry to the matching
    /// registration method. This is the AST-walk equivalent of the bison
    /// semantic actions ('handle_multichar' / 'handle_noflag' /
    /// 'handle_definition' / 'handle_lexicon_name' / 'handle_string_entry' /
    /// 'handle_string_pair_entry' / 'handle_regexp_entry').
    pub fn compile_file(&mut self, ast: &LexcFile) -> crate::error::Result<()> {
        for mc in &ast.multichars {
            self.current_span = mc.span.range.clone();
            self.add_alphabet(&mc.value.0);
        }
        for nf in &ast.noflags {
            self.current_span = nf.span.range.clone();
            self.add_no_flag(&nf.value.0);
        }
        for def in &ast.definitions {
            self.current_span = def.span.range.clone();
            let body = nfst_xre::pretty_print(&def.value.body);
            self.add_xre_definition(&def.value.name, &body);
        }
        for lex in &ast.lexicons {
            self.current_span = lex.span.range.clone();
            // mirror the C++ titlecase-'Lexicon' warning (lexc-parser.yy
            // LEXICON_START_WRONG_CASE), emitted before the lexicon is set.
            if lex.value.case_warning {
                if self.treat_warnings_as_errors {
                    self.error_at_current_token(
                        "Keyword 'Lexicon' used instead of 'LEXICON'. [--Werror]",
                    );
                    self.parseErrors_ = true;
                } else {
                    self.warning_at_current_token("Titlecase Lexicon parsed as LEXICON");
                }
            }
            self.set_current_lexicon_name(&lex.value.name);
            for entry in &lex.value.entries {
                self.current_span = entry.span.range.clone();
                let e = &entry.value;
                let weight = weight_from_gloss(e.gloss.as_deref());
                match &e.spec {
                    EntrySpec::Empty => {
                        self.add_string_entry("", &e.continuation, weight);
                    }
                    EntrySpec::String(s) => {
                        self.add_string_entry(s, &e.continuation, weight);
                    }
                    EntrySpec::Pair { upper, lower } => {
                        self.add_string_pair_entry(upper, lower, &e.continuation, weight);
                    }
                    EntrySpec::Regex(xre) => {
                        let r = nfst_xre::pretty_print(xre);
                        self.add_xre_entry(&r, &e.continuation, weight)?;
                    }
                }
            }
        }
        Ok(())
    }
}

// ===== body 1 (flattened, module scope) =====
impl<B: AlgebraBackend> LexcCompiler<B> {
    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.compile-lexical-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.compile-lexical-fn]
    //
    // Port of 'LexcCompiler::compileLexical()' (LexcCompiler.cc ~1300-1699).
    //
    // The C++ 'std::ostream *err = get_stream(error_)' plumbing and every
    // 'flush(err)' are folded into 'tracing' diagnostics, per the
    // module docs (the non-WINDOWS code path). The 'if (debug)' AT&T dumps are
    // omitted because the file-global 'bool debug' is always 'false'. The
    // 'COLOUR_*' '#define's are inlined as literal ANSI escapes to avoid a
    // duplicate module-scope definition with the lexc-utils body (which owns
    // 'should_colourise').
    pub fn compile_lexical(&mut self) -> crate::error::Result<Option<HfstTransducer<B>>> {
        if self.parseErrors_ {
            error!("compilation aborted due to previous errors");
            return Ok(None);
        }
        let mut warnings_generated = false;
        self.print_connectedness(&mut warnings_generated);
        if warnings_generated && self.treat_warnings_as_errors {
            error!("missing or unused LEXICONs (see above) and -Werror has been enabled");
            return Ok(None);
        }

        let mut lexicons: HfstTransducer<B> = HfstTransducer::new_from_basic(&self.stringsTrie_)?;

        lexicons.optimize()?;

        // repeat star to overgenerate
        lexicons.repeat_star()?.optimize()?;

        let mut small_substitutions = HfstSymbolSubstitutions::new();
        small_substitutions.insert(
            Symbol::new_static("@0@"),
            Symbol::new_static("@_EPSILON_SYMBOL_@"),
        );
        small_substitutions.insert(
            Symbol::new_static("@@ANOTHER_EPSILON@@"),
            Symbol::new_static("@_EPSILON_SYMBOL_@"),
        );
        small_substitutions.insert(Symbol::new_static("@ZERO@"), Symbol::new_static("0"));

        lexicons.substitute_symbol_substitutions(&small_substitutions)?;
        lexicons.prune_alphabet(true)?;

        let mut joiners_trie = HfstBasicTransducer::new();

        let mut all_joiners_to_epsilon = HfstSymbolSubstitutions::new();

        if !self.with_flags {
            let start_joiner = joiner_encode(&self.initialLexiconName_);
            let start = HfstTransducer::new_tokenized(&start_joiner, &self.tokenizer)?;
            let end_string = joiner_encode("#");
            let end = HfstTransducer::new_tokenized(&end_string, &self.tokenizer)?;
            // lexicons = start.concatenate(lexicons).concatenate(end).optimize();
            let mut bracketed = start;
            bracketed
                .concatenate(&lexicons, true)?
                .concatenate(&end, true)?
                .optimize()?;
            lexicons = bracketed;

            for s in &self.lexiconNames_ {
                if self.verbose {
                    debug!("Morphotaxing... {} ", s);
                }
                let joiner_enc = joiner_encode(s);

                // joiners trie version (later compose)
                let doubled = format!("{}{}", joiner_enc, joiner_enc);
                let new_vector = self.tokenizer.tokenize(&doubled, false);
                joiners_trie.disjunct_path(&new_vector, 0.0f32);

                all_joiners_to_epsilon.insert(
                    Symbol::from(joiner_enc),
                    Symbol::new_static("@_EPSILON_SYMBOL_@"),
                );
            }

            let root_joiner = joiner_encode(&self.initialLexiconName_);
            let hash_joiner = joiner_encode("#");

            all_joiners_to_epsilon.insert(
                Symbol::from(root_joiner),
                Symbol::new_static("@_EPSILON_SYMBOL_@"),
            );
            all_joiners_to_epsilon.insert(
                Symbol::from(hash_joiner),
                Symbol::new_static("@_EPSILON_SYMBOL_@"),
            );
        } else {
            let root_p = flag_joiner_encode(&self.initialLexiconName_, false);
            let root_r = flag_joiner_encode(&self.initialLexiconName_, true);

            let start_p = HfstTransducer::new_tokenized(&root_p, &self.tokenizer)?;
            let _start_r: HfstTransducer<B> =
                HfstTransducer::new_tokenized(&root_r, &self.tokenizer)?;

            let end_string_p = flag_joiner_encode("#", false);
            let end_string_r = flag_joiner_encode("#", true);

            self.tokenizer.add_multichar_symbol(&end_string_p);
            self.tokenizer.add_multichar_symbol(&end_string_r);

            let _end_p: HfstTransducer<B> =
                HfstTransducer::new_tokenized(&end_string_p, &self.tokenizer)?;
            let end_r = HfstTransducer::new_tokenized(&end_string_r, &self.tokenizer)?;

            // lexicons = startP.concatenate(lexicons).concatenate(endR).optimize();
            let mut bracketed = start_p;
            bracketed
                .concatenate(&lexicons, true)?
                .concatenate(&end_r, true)?
                .optimize()?;
            lexicons = bracketed;

            for s in &self.lexiconNames_ {
                if self.verbose {
                    debug!("Morphotaxing... {} ", s);
                }
                let flag_p_string = flag_joiner_encode(s, false);
                let flag_r_string = flag_joiner_encode(s, true);

                // joiners trie version (later compose)
                let combined = format!("{}{}", flag_p_string, flag_r_string);
                let new_vector = self.tokenizer.tokenize(&combined, false);
                joiners_trie.disjunct_path(&new_vector, 0.0f32);
            }
        }

        // get right side of every pair
        let fsm = HfstBasicTransducer::from_transducer(&lexicons);
        let mut right_symbols: StringSet = BTreeSet::new();
        // Go through all states
        for state in fsm.states_and_transitions() {
            // Go through all transitions
            for tr in state {
                let alph2 = tr.get_output_symbol(fsm.coder());

                if !alph2.starts_with("@@ANOTHER_EPSILON@@")
                    && !alph2.starts_with("$_LEXC_JOINER.")
                    && !alph2.starts_with("$P.LEXNAME.")
                    && !alph2.starts_with("$R.LEXNAME.")
                    && !alph2.starts_with("@_")
                {
                    right_symbols.insert(alph2);
                }
            }
        }

        for alph in &right_symbols {
            self.tokenizer.add_multichar_symbol(alph);
            let new_vector = self.tokenizer.tokenize(alph, false);
            joiners_trie.disjunct_path(&new_vector, 0.0f32);
        }

        let mut joiners_all: HfstTransducer<B> = HfstTransducer::new_from_basic(&joiners_trie)?;

        joiners_all.repeat_star()?;
        joiners_all.optimize()?;

        lexicons
            .compose_with_config(&joiners_all, true, &self.compose_cfg())?
            .optimize()?;

        let mut all_substitutions = HfstSymbolSubstitutions::new();
        if self.with_flags {
            if self.verbose {
                debug!("Changing flags...");
            }
            let mut fake_flags_to_real_flags = HfstSymbolSubstitutions::new();
            // Change fake flags to real flags
            lexicons.prune_alphabet(true)?;

            let transducer_alphabet = lexicons.get_alphabet()?;
            for s in &transducer_alphabet {
                if s.starts_with('$') && s.ends_with('$') && s.len() > 2 {
                    let alph = s.replace('$', "@");
                    fake_flags_to_real_flags.insert(s.clone(), Symbol::from(alph));
                }
            }
            all_substitutions.extend(fake_flags_to_real_flags);
        } else {
            all_substitutions.extend(all_joiners_to_epsilon);
        }

        lexicons
            .substitute_symbol_substitutions(&all_substitutions)?
            .optimize()?;
        lexicons.prune_alphabet(true)?;

        // replace reg exp key with transducers
        if self.verbose {
            debug!("Inserting regular expressions...");
        }

        // substitute all reg expression into special, unharmonizible symbols
        let mut fake_regexpr_to_real = HfstSymbolSubstitutions::new();
        for key in self.regexps.keys() {
            if key.starts_with('$') {
                // TODO: do this only for strings that look like $.....$
                let alph = key.replace('$', "@");
                fake_regexpr_to_real.insert(Symbol::new(key), Symbol::from(alph));
            }
        }
        lexicons
            .substitute_symbol_substitutions(&fake_regexpr_to_real)?
            .optimize()?;
        lexicons.prune_alphabet(true)?;

        let mut reg_mark_to_tr: crate::hfst_basic_transducer::SubstMap = BTreeMap::new();

        for (key, tr) in self.regexps.iter() {
            let alph = if key.starts_with('$') {
                // TODO: do this only for strings that look like $.....$
                key.replace('$', "@")
            } else {
                key.clone()
            };
            let btr = HfstBasicTransducer::from_transducer(tr);
            reg_mark_to_tr.insert(Symbol::from(alph), btr);
        }

        let mut lexicons_basic = HfstBasicTransducer::from_transducer(&lexicons);
        lexicons_basic.substitute_subst_map(&mut reg_mark_to_tr, true)?;

        lexicons_basic.prune_alphabet(true);

        let mut rv: HfstTransducer<B> = HfstTransducer::new_from_basic(&lexicons_basic)?;

        // Preserve only first flag of consecutive P and R lexname flag series,
        // e.g. change P.LEXNAME.1 R.LEXNAME.1 P.LEXNAME.2 R.LEXNAME.2 into
        // P.LEXNAME.1
        if self.with_flags {
            let transducer_alphabet = rv.get_alphabet()?;
            let mut flag_d: StringSet = BTreeSet::new();
            for s in &transducer_alphabet {
                if s.starts_with("@P.LEXNAME") || s.starts_with("@R.LEXNAME") {
                    flag_d.insert(s.clone());
                }
            }

            // Construct a rule for consecutive flag removal:
            // [FLAG1 | FLAG2 ... FLAGN] -> 0 || [FLAG1 | FLAG2 ... FLAGN] _
            // and also an inverted rule
            let mut flag_remover_regexp = String::from("[ ");
            let mut first_flag = true;

            for it in &flag_d {
                if !first_flag {
                    flag_remover_regexp.push_str("| ");
                }
                flag_remover_regexp.push('"');
                flag_remover_regexp.push_str(it);
                flag_remover_regexp.push_str("\" ");
                first_flag = false;
            }
            flag_remover_regexp.push(']');
            let context_regexp = flag_remover_regexp.clone();
            flag_remover_regexp.push_str(" -> 0 || ");
            flag_remover_regexp.push_str(&context_regexp);
            flag_remover_regexp.push_str(" _ ");

            let mut xre_comp: XreCompiler<B> = XreCompiler::new();

            let mut flag_filter = xre_comp
                .compile(&flag_remover_regexp)
                .expect("flag-remover regexp is generated internally and always compiles");
            flag_filter.optimize()?;
            let mut inverted_flag_filter = flag_filter.clone();
            inverted_flag_filter.invert()?.optimize()?;

            // [ [FLAG1 | FLAG2 ... FLAGN] -> 0 || [FLAG1 | FLAG2 ... FLAGN] _
            // ].inv
            //                        .o.
            //                       RESULT
            //                        .o.
            // [FLAG1 | FLAG2 ... FLAGN] -> 0 || [FLAG1 | FLAG2 ... FLAGN] _
            let mut filtered_lexicons = inverted_flag_filter;
            let cfg = self.compose_cfg();
            filtered_lexicons.compose_with_config(&rv, true, &cfg)?;
            filtered_lexicons
                .compose_with_config(&flag_filter, true, &cfg)?
                .optimize()?;

            rv.assign(&filtered_lexicons)?;
        }

        rv.optimize()?;

        Ok(Some(rv))
    }

    // Port of 'LexcCompiler::printConnectedness(bool &warnings_generated)'
    // (LexcCompiler.cc ~1701-1765): the missing/unused-lexicon validation. The
    // C++ 'set_difference' over the sorted 'std::set's becomes 'BTreeSet'
    // differences (also sorted). The 'COLOUR_*' escapes are dropped (the
    // 'tracing' subscriber owns formatting), and 'flush(err)' is dropped.
    pub fn print_connectedness(&self, warnings_generated: &mut bool) -> &Self {
        if self.lexiconNames_ != self.continuations {
            let lex_minus_cont: Vec<&String> =
                self.lexiconNames_.difference(&self.continuations).collect();
            let cont_minus_lex: Vec<&String> =
                self.continuations.difference(&self.lexiconNames_).collect();
            if !cont_minus_lex.is_empty() {
                for s in &cont_minus_lex {
                    if !self.quiet && self.warn_missing_lexicons {
                        if self.treat_warnings_as_errors {
                            error!(
                                "Sublexicon is mentioned but not defined. [-Wmissing-lexicons] ({}) ",
                                s
                            );
                        } else {
                            warn!(
                                "Sublexicon is mentioned but not defined. [-Wmissing-lexicons] ({}) ",
                                s
                            );
                        }
                    }
                    *warnings_generated = true;
                }
            }
            if !lex_minus_cont.is_empty() {
                *warnings_generated = true;
                if !self.quiet && self.warn_unused_lexicons {
                    let mut line = String::new();
                    for s in &lex_minus_cont {
                        line.push_str(s);
                        line.push(' ');
                    }
                    if self.treat_warnings_as_errors {
                        error!(
                            "Sublexicons defined but not used [-Wunused-lexicons]\n{}",
                            line
                        );
                    } else {
                        warn!(
                            "Sublexicons defined but not used [-Wunused-lexicons]\n{}",
                            line
                        );
                    }
                }
            }
        }
        self
    }
}
