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
//!   * the 'lexc_' singleton becomes '&mut self';
//!   * 'static bool firstLexicon' becomes the 'first_lexicon_' field;
//!   * the unused 'static StringVector multichar_symbols' is dropped.
//!
//! # Stream / WINDOWS plumbing dropped — error text via 'eprintln!'.
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

use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_data_types::{
    ImplementationType, StringPair, StringPairVector, StringVector, double_to_float, size_t_to_int,
};
use crate::hfst_symbol_defs::{HfstSymbolSubstitutions, StringSet};
use crate::hfst_tokenizer::HfstTokenizer;
use crate::hfst_transducer::HfstTransducer;
use crate::xre::XreCompiler;

// [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler]
pub struct LexcCompiler {
    pub(crate) format_: ImplementationType,
    pub(crate) tokenizer_: HfstTokenizer,
    pub(crate) xre_: XreCompiler,
    pub(crate) initialLexiconName_: String,
    pub(crate) currentLexiconName_: String,
    pub(crate) stringsTrie_: HfstBasicTransducer,
    pub(crate) regexps_: BTreeMap<String, HfstTransducer>, // owning HfstTransducer* -> owned
    pub(crate) lexiconNames_: BTreeSet<String>,
    pub(crate) noFlags_: BTreeSet<String>,
    pub(crate) continuations_: BTreeSet<String>,
    pub(crate) alphabets_: BTreeSet<String>,
    pub(crate) totalEntries_: usize,
    pub(crate) currentEntries_: usize,
    pub(crate) align_strings_: bool,
    pub(crate) with_flags_: bool,
    pub(crate) minimize_flags_: bool,
    pub(crate) rename_flags_: bool,
    pub(crate) split_characters_: bool,
    pub(crate) treat_warnings_as_errors_: bool,
    pub(crate) warn_everything_: bool,
    pub(crate) warn_missing_lexicons_: bool,
    pub(crate) warn_unused_lexicons_: bool,
    pub(crate) warn_repeated_lexicons_: bool,
    pub(crate) warn_missing_alphabets_: bool,
    pub(crate) warn_one_sided_flags_: bool, // C++ leaves this UNINITIALIZED; default false
    pub(crate) warn_unnecessary_escapes_: bool,
    pub(crate) verbose_: bool,
    pub(crate) quiet_: bool,
    pub(crate) first_lexicon_: bool, // folded 'static bool firstLexicon'
    pub parseErrors_: bool,          // public field in C++ header
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
    unsafe { libc::isatty(1) != 0 }
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

    let mut x = size_t_to_int(s1.len());
    let mut y = size_t_to_int(s2.len());
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

impl LexcCompiler {
    /// Common body of the 'LexcCompiler(impl)' and
    /// 'LexcCompiler(impl, withFlags, alignStrings)' constructors: seeds the
    /// tokenizer with the epsilon/zero multichars + the '#' joiner, registers
    /// '#' as a lexicon name, and configures 'xre_'.
    fn seeded(format: ImplementationType) -> LexcCompiler {
        let mut compiler = LexcCompiler {
            format_: format,
            tokenizer_: HfstTokenizer::new(),
            xre_: XreCompiler::new(format),
            initialLexiconName_: "Root".to_string(),
            currentLexiconName_: String::new(),
            stringsTrie_: HfstBasicTransducer::new(),
            regexps_: BTreeMap::new(),
            lexiconNames_: BTreeSet::new(),
            noFlags_: BTreeSet::new(),
            continuations_: BTreeSet::new(),
            alphabets_: BTreeSet::new(),
            totalEntries_: 0,
            currentEntries_: 0,
            align_strings_: false,
            with_flags_: false,
            minimize_flags_: false,
            rename_flags_: false,
            split_characters_: false,
            treat_warnings_as_errors_: false,
            warn_everything_: false,
            warn_missing_lexicons_: false,
            warn_unused_lexicons_: false,
            warn_repeated_lexicons_: false,
            warn_missing_alphabets_: false,
            warn_one_sided_flags_: false,
            warn_unnecessary_escapes_: false,
            verbose_: false,
            quiet_: false,
            first_lexicon_: true,
            parseErrors_: false,
        };
        compiler
            .tokenizer_
            .add_multichar_symbol("@_EPSILON_SYMBOL_@");
        compiler.tokenizer_.add_multichar_symbol("@0@");
        compiler.tokenizer_.add_multichar_symbol("@ZERO@");
        compiler
            .tokenizer_
            .add_multichar_symbol("@@ANOTHER_EPSILON@@");
        let hash = "#".to_string();
        compiler.lexiconNames_.insert(hash.clone());
        let enc = joiner_encode(&hash);
        compiler.tokenizer_.add_multichar_symbol(&enc);
        compiler.xre_.set_expand_definitions(true);
        compiler.xre_.set_verbosity(!compiler.quiet_);
        compiler
    }

    /// Port of 'LexcCompiler(ImplementationType impl)' (unannotated in the .cc).
    pub fn new(format: ImplementationType) -> LexcCompiler {
        LexcCompiler::seeded(format)
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.lexc-compiler-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.lexc-compiler-fn]
    /// Port of 'LexcCompiler(impl, withFlags, alignStrings)'.
    pub fn new_with_flags(
        format: ImplementationType,
        with_flags: bool,
        align_strings: bool,
    ) -> LexcCompiler {
        let mut compiler = LexcCompiler::seeded(format);
        compiler.align_strings_ = align_strings;
        compiler.with_flags_ = with_flags;
        compiler
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.reset-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.reset-fn]
    pub fn reset(&mut self) {
        self.tokenizer_ = HfstTokenizer::new();
        self.tokenizer_.add_multichar_symbol("@_EPSILON_SYMBOL_@");
        self.tokenizer_.add_multichar_symbol("@0@");
        self.tokenizer_.add_multichar_symbol("@ZERO@");
        self.tokenizer_.add_multichar_symbol("@@ANOTHER_EPSILON@@");
        self.initialLexiconName_ = "Root".to_string();
        self.totalEntries_ = 0;
        self.currentEntries_ = 0;
        self.parseErrors_ = false;
        self.lexiconNames_.clear();
        self.noFlags_.clear();
        self.continuations_.clear();
        self.alphabets_.clear();
        self.currentLexiconName_ = String::new(); // ?
        self.lexiconNames_.insert("#".to_string());
        self.stringsTrie_ = HfstBasicTransducer::new(); // ?
        // The owned regexps_ transducers are dropped by clear() (C++ delete'd
        // the raw pointers here). The C++ 'static bool firstLexicon' was a
        // function-static and is NOT touched by reset(); first_lexicon_ is left
        // untouched to mirror that.
        self.regexps_.clear();
    }

    // ----- option setters / getters -----

    pub fn set_verbosity(&mut self, verbose: u32) -> &mut Self {
        if verbose == 0 {
            self.quiet_ = true;
            self.verbose_ = false;
        } else if verbose == 1 {
            self.quiet_ = false;
            self.verbose_ = false;
        } else {
            self.quiet_ = false;
            self.verbose_ = true;
        }
        self.xre_.set_verbosity(!self.quiet_);
        self
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.get-verbosity-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.get-verbosity-fn]
    pub fn get_verbosity(&self) -> u32 {
        if self.quiet_ && !self.verbose_ {
            return 0;
        }
        if !self.quiet_ && !self.verbose_ {
            return 1;
        }
        if !self.quiet_ && self.verbose_ {
            return 2;
        }
        std::panic::panic_any("LexcCompiler::getVerbosity() failed".to_string())
    }

    pub fn set_treat_warnings_as_errors(&mut self, value: bool) -> &mut Self {
        self.treat_warnings_as_errors_ = value;
        self
    }

    pub fn set_align_strings(&mut self, value: bool) -> &mut Self {
        self.align_strings_ = value;
        self
    }

    pub fn set_with_flags(&mut self, value: bool) -> &mut Self {
        self.with_flags_ = value;
        self
    }

    pub fn set_minimize_flags(&mut self, value: bool) -> &mut Self {
        self.minimize_flags_ = value;
        self
    }

    pub fn set_rename_flags(&mut self, value: bool) -> &mut Self {
        self.rename_flags_ = value;
        self
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.set-warning-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.set-warning-fn]
    pub fn set_warning(&mut self, warning: &str, value: bool) {
        match warning {
            "-Wone-sided-flags" => self.warn_one_sided_flags_ = value,
            "-Wmissing-lexicons" => self.warn_missing_lexicons_ = value,
            "-Wunused-lexicons" => self.warn_unused_lexicons_ = value,
            "-Wrepeated-lexicons" => self.warn_repeated_lexicons_ = value,
            "-Wmissing-alphabets" => self.warn_missing_alphabets_ = value,
            "-Wunnecessary-escapes" => self.warn_unnecessary_escapes_ = value,
            _ => eprintln!("unknown warning {}", warning),
        }
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.set-error-stream-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.set-error-stream-fn]
    /// The C++ stored an 'std::ostream*' and forwarded it to 'xre_'; the port
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
    /// the WINDOWS-only 'output_to_console_' field is dropped from the port.
    pub fn set_output_to_console(&mut self, _value: bool) {}

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.get-output-to-console-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.get-output-to-console-fn]
    /// On non-WINDOWS the C++ 'getOutputToConsole' always returns false; the
    /// WINDOWS-only 'output_to_console_' field is dropped from the port.
    pub fn get_output_to_console(&self) -> bool {
        false
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.is-quiet-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.is-quiet-fn]
    pub fn is_quiet(&self) -> bool {
        self.quiet_
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.are-warnings-treated-as-errors-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.are-warnings-treated-as-errors-fn]
    pub fn are_warnings_treated_as_errors(&self) -> bool {
        self.treat_warnings_as_errors_
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.is-strict-alphabets-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.is-strict-alphabets-fn]
    pub fn is_strict_alphabets(&self) -> bool {
        self.warn_missing_alphabets_
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.set-strict-alphabets-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.set-strict-alphabets-fn]
    pub fn set_strict_alphabets(&mut self, strictness: bool) {
        self.warn_missing_alphabets_ = strictness;
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.has-split-characters-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.has-split-characters-fn]
    pub fn has_split_characters(&self) -> bool {
        self.split_characters_
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.set-split-characters-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.set-split-characters-fn]
    pub fn set_split_characters(&mut self, splitness: bool) {
        self.split_characters_ = splitness;
    }

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.is-warning-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.is-warning-fn]
    pub fn is_warning(&self, warning: &str) -> bool {
        if warning == "-Wone-sided-flags" {
            self.warn_one_sided_flags_
        } else if warning == "-Wmissing-lexicons" {
            self.warn_missing_lexicons_
        } else if warning == "-Wunused-lexicons" {
            self.warn_unused_lexicons_
        } else if warning == "-Wrepeated-lexicons" {
            self.warn_repeated_lexicons_
        } else if warning == "-Wmissing-alphabets" {
            self.warn_missing_alphabets_
        } else if warning == "-Wunnecessary-escapes" {
            self.warn_unnecessary_escapes_
        } else {
            eprintln!("unknown warning {}", warning);
            false
        }
    }

    // ----- error / warning helpers (lexc-utils.cc, re-entrant) -----

    // [spec:hfst:def:lexc-utils.hfst.lexc.error-at-current-token-fn]
    // [spec:hfst:sem:lexc-utils.hfst.lexc.error-at-current-token-fn]
    /// The C++ free function printed Flex token positions; the AST-walk port has
    /// no hand-lexer position state, so it just writes the message to stderr.
    fn error_at_current_token(&self, format: &str) {
        if should_colourise() {
            eprintln!("\u{1b}[01m\u{1b}[31m{}\u{1b}[0m", format);
        } else {
            eprintln!("{}", format);
        }
    }

    // [spec:hfst:def:lexc-utils.hfst.lexc.warning-at-current-token-fn]
    // [spec:hfst:sem:lexc-utils.hfst.lexc.warning-at-current-token-fn]
    fn warning_at_current_token(&self, format: &str) {
        if should_colourise() {
            eprintln!("\u{1b}[01m\u{1b}[33m{}\u{1b}[0m", format);
        } else {
            eprintln!("{}", format);
        }
    }

    // [spec:hfst:def:lexc-utils.hfst.lexc.strip-percents-fn]
    // [spec:hfst:sem:lexc-utils.hfst.lexc.strip-percents-fn]
    //
    // Port of the char*-returning 'hfst::lexc::strip_percents(const char *s,
    // bool do_zeros)' (distinct from the std::string '&'-returning
    // 'stripPercents', which is ported as the 'strip_percents_str' free fn).
    // The C++ reached the 'lexc_' singleton for warnings; the re-entrant port
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
    /// guard (auto-adding multi-codepoint graphemes to 'alphabets_') is deferred;
    /// for the common single-codepoint-grapheme path the C++ adds nothing, so
    /// this is faithful for ASCII and only skips the multi-codepoint warning.
    fn unicode_check_(&mut self, _data: &str) -> &mut Self {
        if self.split_characters_ {
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
        self.alphabets_.insert(alpha.to_string());
        self.tokenizer_.add_multichar_symbol(alpha);
        if !self.quiet_ && self.verbose_ {
            // warn about undefined multichars
            self.xre_.add_defined_multichar_symbol(alpha);
        }
        self
    }

    /// Construct vector 'nameJoiner data contJoiner' and add it to the trie.
    pub fn add_string_entry(&mut self, data: &str, continuation: &str, weight: f64) -> &mut Self {
        self.currentEntries_ += 1;
        self.totalEntries_ += 1;
        self.unicode_check_(data);
        self.continuations_.insert(continuation.to_string());
        let encoded_cont = if self.with_flags_ {
            if !self.noFlags_.contains(continuation) {
                flag_joiner_encode(continuation, false)
            } else {
                joiner_encode(continuation)
            }
        } else {
            joiner_encode(continuation)
        };
        self.tokenizer_.add_multichar_symbol(&encoded_cont);

        // build string pair vector map
        let cur = self.currentLexiconName_.clone();
        let joiner_enc = if self.with_flags_ {
            if !self.noFlags_.contains(&cur) {
                flag_joiner_encode(&cur, true)
            } else {
                joiner_encode(&cur)
            }
        } else {
            joiner_encode(&cur)
        };
        self.tokenizer_.add_multichar_symbol(&joiner_enc);
        self.tokenizer_.add_multichar_symbol("0"); // epsilon
        self.tokenizer_.add_multichar_symbol("@ZERO@"); // literal zero
        let mut new_vector = self.tokenizer_.tokenize(
            &format!("{}{}{}", joiner_enc, data, encoded_cont),
            self.split_characters_,
        );
        // "0"      -> "@0@"  (single symbols)
        // "@ZERO@" -> "0"    (everywhere)
        let mut i = 0;
        while i < new_vector.len() {
            if new_vector[i].0 == "0" {
                new_vector[i].0 = "@0@".to_string();
            }
            if let Some(start_pos) = new_vector[i].0.find("@ZERO@") {
                new_vector[i]
                    .0
                    .replace_range(start_pos..start_pos + "@ZERO@".len(), "0");
            }
            if new_vector[i].1 == "0" {
                new_vector[i].1 = "@0@".to_string();
            }
            if let Some(start_pos) = new_vector[i].1.find("@ZERO@") {
                new_vector[i]
                    .1
                    .replace_range(start_pos..start_pos + "@ZERO@".len(), "0");
            }
            let first = new_vector[i].0.clone();
            if !self.alphabets_.contains(&first) {
                if first.starts_with('@') && first.ends_with('@') {
                    i += 1;
                    continue;
                }
                if first.starts_with('$') && first.ends_with('$') {
                    i += 1;
                    continue;
                }
                if self.warn_missing_alphabets_ {
                    let errm = format!("Adding {} to Alphabets [Wmissing-alphabets]", first);
                    if self.treat_warnings_as_errors_ {
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
        let w = double_to_float(weight);
        self.stringsTrie_.disjunct_path(&new_vector, w);
        self
    }

    // callback function to stuff so static and uses global singleton :-(
    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.warn-about-one-sided-flags-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.warn-about-one-sided-flags-fn]
    /// In the C++ this was a static tokenize callback reaching the 'lexc_'
    /// global; the re-entrant port applies it to the tokenized pairs directly.
    fn warn_about_one_sided_flags(&mut self, symbol_pair: &StringPair) {
        if crate::hfst_flag_diacritics::FdOperation::is_diacritic(&symbol_pair.0) {
            if symbol_pair.0 != symbol_pair.1 {
                let errm = format!(
                    "one-sided flag diacritic {}:{} [-Wone-sided-flags]",
                    symbol_pair.0, symbol_pair.1
                );
                if self.warn_one_sided_flags_ && self.treat_warnings_as_errors_ {
                    self.error_at_current_token(&errm);
                    self.parseErrors_ = true;
                }
                if self.warn_one_sided_flags_ {
                    self.warning_at_current_token(&errm);
                }
            }
            return;
        } else if crate::hfst_flag_diacritics::FdOperation::is_diacritic(&symbol_pair.1) {
            let errm = format!(
                "one-sided flag diacritic {}:{} [-Wone-sided-flags]",
                symbol_pair.0, symbol_pair.1
            );
            if self.warn_one_sided_flags_ && self.treat_warnings_as_errors_ {
                self.error_at_current_token(&errm);
                self.parseErrors_ = true;
            }
            if self.warn_one_sided_flags_ {
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
        self.unicode_check_(upper);
        self.unicode_check_(lower);
        self.continuations_.insert(continuation.to_string());
        let encoded_cont = if self.with_flags_ {
            if !self.noFlags_.contains(continuation) {
                flag_joiner_encode(continuation, false)
            } else {
                joiner_encode(continuation)
            }
        } else {
            joiner_encode(continuation)
        };
        self.tokenizer_.add_multichar_symbol(&encoded_cont);

        // build string pair vector map
        let cur = self.currentLexiconName_.clone();
        let joiner_enc = if self.with_flags_ {
            if !self.noFlags_.contains(&cur) {
                flag_joiner_encode(&cur, true)
            } else {
                joiner_encode(&cur)
            }
        } else {
            joiner_encode(&cur)
        };
        self.tokenizer_.add_multichar_symbol(&joiner_enc);
        self.tokenizer_.add_multichar_symbol("0"); // epsilon
        self.tokenizer_.add_multichar_symbol("@ZERO@"); // literal zero

        let mut new_vector: StringPairVector;

        if self.align_strings_ {
            let tmp = self
                .tokenizer_
                .tokenize_pair(upper, lower, self.split_characters_);
            let mut one: Vec<String> = Vec::new();
            let mut two: Vec<String> = Vec::new();
            for it in &tmp {
                if it.0 != "@_EPSILON_SYMBOL_@" {
                    one.push(it.0.clone());
                }
                if it.1 != "@_EPSILON_SYMBOL_@" {
                    two.push(it.1.clone());
                }
            }

            let med_vectors = find_med_alingment(&one, &two);
            let as1: String = med_vectors.0.concat();
            let as2: String = med_vectors.1.concat();

            new_vector = self.tokenizer_.tokenize_pair(
                &format!("{}{}{}", joiner_enc, as1, encoded_cont),
                &format!("{}{}{}", joiner_enc, as2, encoded_cont),
                self.split_characters_,
            );
        } else {
            let upper_v = self.tokenizer_.tokenize(upper, self.split_characters_);
            let lower_v = self.tokenizer_.tokenize(lower, self.split_characters_);

            let upper_size = size_t_to_int(upper_v.len());
            let lower_size = size_t_to_int(lower_v.len());

            if upper_size > lower_size {
                let mut epsilons = String::new();
                for _ in 1..=(upper_size - lower_size) {
                    epsilons.push_str("@@ANOTHER_EPSILON@@");
                }
                new_vector = self.tokenizer_.tokenize_pair(
                    &format!("{}{}{}", joiner_enc, upper, encoded_cont),
                    &format!("{}{}{}{}", joiner_enc, lower, epsilons, encoded_cont),
                    self.split_characters_,
                );
            } else if upper_size < lower_size {
                let mut epsilons = String::new();
                for _ in 1..=(lower_size - upper_size) {
                    epsilons.push_str("@@ANOTHER_EPSILON@@");
                }
                new_vector = self.tokenizer_.tokenize_pair(
                    &format!("{}{}{}{}", joiner_enc, upper, epsilons, encoded_cont),
                    &format!("{}{}{}", joiner_enc, lower, encoded_cont),
                    self.split_characters_,
                );
            } else {
                new_vector = self.tokenizer_.tokenize_pair(
                    &format!("{}{}{}", joiner_enc, upper, encoded_cont),
                    &format!("{}{}{}", joiner_enc, lower, encoded_cont),
                    self.split_characters_,
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
                new_vector[i].0 = "@0@".to_string();
            }
            if let Some(start_pos) = new_vector[i].0.find("@ZERO@") {
                new_vector[i]
                    .0
                    .replace_range(start_pos..start_pos + "@ZERO@".len(), "0");
            }
            if new_vector[i].1 == "0" {
                new_vector[i].1 = "@0@".to_string();
            }
            if let Some(start_pos) = new_vector[i].1.find("@ZERO@") {
                new_vector[i]
                    .1
                    .replace_range(start_pos..start_pos + "@ZERO@".len(), "0");
            }
            let first = new_vector[i].0.clone();
            if !self.alphabets_.contains(&first) {
                if first.starts_with('@') && first.ends_with('@') {
                    i += 1;
                    continue;
                }
                if first.starts_with('$') && first.ends_with('$') {
                    i += 1;
                    continue;
                }
                if self.warn_missing_alphabets_ {
                    let errm = format!("Adding {} to Alphabets [-Wmissing-alphabets]", first);
                    if self.treat_warnings_as_errors_ {
                        self.error_at_current_token(&errm);
                        self.parseErrors_ = true;
                    } else {
                        self.warning_at_current_token(&errm);
                    }
                }
                self.add_alphabet(&first);
            }
            let second = new_vector[i].1.clone();
            if !self.alphabets_.contains(&second) {
                if second.starts_with('@') && second.ends_with('@') {
                    i += 1;
                    continue;
                }
                if second.starts_with('$') && second.ends_with('$') {
                    i += 1;
                    continue;
                }
                if self.warn_missing_alphabets_ {
                    let errm = format!("Adding {} to Alphabets [-Wmissing-alphabets]", second);
                    if self.treat_warnings_as_errors_ {
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

        let w = double_to_float(weight);
        self.stringsTrie_.disjunct_path(&new_vector, w);
        self
    }

    /// Construct transducer 'nameJoiner XRE contJoiner' and add it to the trie.
    pub fn add_xre_entry(&mut self, regexp: &str, continuation: &str, weight: f64) -> &mut Self {
        self.currentEntries_ += 1;
        self.totalEntries_ += 1;
        self.continuations_.insert(continuation.to_string());
        let encoded_cont = if self.with_flags_ {
            if !self.noFlags_.contains(continuation) {
                flag_joiner_encode(continuation, false)
            } else {
                joiner_encode(continuation)
            }
        } else {
            joiner_encode(continuation)
        };
        self.tokenizer_.add_multichar_symbol(&encoded_cont);

        let new_paths_ptr = self.xre_.compile(regexp);
        if new_paths_ptr.is_null() {
            self.error_at_current_token("Unable to parse regular expression");
            self.parseErrors_ = true;
            return self;
        }

        let mut new_paths = unsafe { *Box::from_raw(new_paths_ptr) };
        new_paths.optimize();
        let new_alphabets = new_paths.get_alphabet();
        for new_alpha in &new_alphabets {
            if self.alphabets_.contains(new_alpha) {
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
                eprintln!("you shoudl add {} to Multichar_Symbols section", new_alpha);
            } else if self.warn_missing_alphabets_ && self.treat_warnings_as_errors_ {
                self.error_at_current_token(&errm);
                self.parseErrors_ = true;
            } else if self.warn_missing_alphabets_ {
                self.warning_at_current_token(&errm);
            }
            self.add_alphabet(new_alpha);
        }

        // encode key; keep regexps with different continuations separate
        let mut regex_key = format!("{}_{}", self.currentLexiconName_, continuation);
        regex_key = reg_expresion_encode(&regex_key);
        self.tokenizer_.add_multichar_symbol(&regex_key);

        let format = self.format_;
        let entry = self
            .regexps_
            .entry(regex_key.clone())
            .or_insert_with(|| HfstTransducer::new_type(format));
        entry.disjunct(&new_paths, true).optimize();

        if !self.quiet_ && (self.currentEntries_ % 10000) == 0 {
            eprint!("{}...", self.currentEntries_);
        }

        // add key to trie
        let cur = self.currentLexiconName_.clone();
        let joiner_enc = if self.with_flags_ {
            if !self.noFlags_.contains(&cur) {
                flag_joiner_encode(&cur, true)
            } else {
                joiner_encode(&cur)
            }
        } else {
            joiner_encode(&cur)
        };
        self.tokenizer_.add_multichar_symbol(&joiner_enc);
        let new_vector = self.tokenizer_.tokenize(
            &format!("{}{}{}", joiner_enc, regex_key, encoded_cont),
            false,
        );
        let w = double_to_float(weight);
        self.stringsTrie_.disjunct_path(&new_vector, w);
        self
    }

    pub fn add_xre_definition(&mut self, definition_name: &str, xre: &str) -> &mut Self {
        // FIXME: collect implicit characters
        self.xre_.define(definition_name, xre);
        if !self.quiet_ {
            eprintln!(
                "Defined '{}': ? Kb., ? states, ? arcs, ? paths.",
                definition_name
            );
        }
        self
    }

    pub fn set_current_lexicon_name(&mut self, lexicon_name: &str) -> &mut Self {
        self.currentLexiconName_ = lexicon_name.to_string();

        if self.lexiconNames_.contains(lexicon_name) {
            if !self.warn_repeated_lexicons_ && self.treat_warnings_as_errors_ {
                self.error_at_current_token(
                    "Lexicon is defined more than once! [-Wrepeated-lexicons]",
                );
                self.parseErrors_ = true;
            } else if self.warn_repeated_lexicons_ {
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
            self.tokenizer_.add_multichar_symbol(&encoded_name);
            encoded_name = flag_joiner_encode(&encoded_name, true);
            self.tokenizer_.add_multichar_symbol(&encoded_name);
        } else {
            let encoded_name = joiner_encode(lexicon_name);
            self.tokenizer_.add_multichar_symbol(&encoded_name);
        }

        if self.first_lexicon_ && lexicon_name == "Root" {
            self.set_initial_lexicon_name(lexicon_name);
        } else if self.first_lexicon_ && lexicon_name != "Root" {
            if self.treat_warnings_as_errors_ {
                self.error_at_current_token("first lexicon is not named Root");
                self.parseErrors_ = true;
            } else {
                self.warning_at_current_token("first lexicon is not named Root");
            }
            self.set_initial_lexicon_name(lexicon_name);
        } else if !self.first_lexicon_ && lexicon_name == "Root" {
            if self.treat_warnings_as_errors_ {
                self.error_at_current_token("Root is not first the first lexicon");
                self.parseErrors_ = true;
            } else {
                self.warning_at_current_token("Root is not first the first lexicon");
            }
            self.set_initial_lexicon_name(lexicon_name);
        }
        if !self.first_lexicon_ && !self.quiet_ {
            eprint!("{} ", self.currentEntries_);
        }
        if !self.quiet_ {
            eprint!("{}...", lexicon_name);
        }
        self.first_lexicon_ = false;

        self.currentEntries_ = 0;
        self
    }

    pub fn set_initial_lexicon_name(&mut self, lexicon_name: &str) -> &mut Self {
        self.initialLexiconName_ = lexicon_name.to_string();
        self.lexiconNames_.insert(lexicon_name.to_string());
        // for connectedness calculation:
        self.continuations_.insert(lexicon_name.to_string());
        self
    }

    // ----- AST-walk driver (replaces parse(FILE*) / parse(filename)) -----

    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.parse-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.parse-fn]
    /// INCREMENTAL entry point: parse 'lexc_source' via 'nfst_lexc::parse' and
    /// walk the typed AST through the registration API, accumulating its
    /// lexicons/entries into 'self' (the trie 'stringsTrie_', 'regexps_',
    /// 'lexiconNames_', etc.). This is the AST-walk port of the Flex/Bison
    /// 'parse(FILE*)' / 'parse(const char*)': both '.cc' overloads ran
    /// 'hlexcparse()' (accumulating into the singleton) then
    /// 'xre_.remove_defined_multichar_symbols()' and set 'parseErrors_' on
    /// failure. It does NOT run 'compileLexical': call 'compile_lexical' once
    /// after every source has been parsed. Multi-file flow: call 'parse' on each
    /// source into one compiler, then 'compile_lexical' once. Returns '&mut self'
    /// to mirror the C++ 'LexcCompiler &' chaining return.
    pub fn parse(&mut self, lexc_source: &str) -> &mut Self {
        match nfst_lexc::parse(lexc_source) {
            Ok(ast) => {
                self.compile_file(&ast.value);
                // mirrors 'xre_.remove_defined_multichar_symbols()' in parse()
                self.xre_.remove_defined_multichar_symbols();
            }
            Err(_e) => {
                // mirrors the 'hlexcnerrs > 0' branch setting parseErrors_
                self.parseErrors_ = true;
            }
        }
        self
    }

    /// PUBLIC entry point: parse a single 'lexc_source' into 'self', then run
    /// 'compileLexical'. This is the AST-walk port of the Flex/Bison driver where
    /// tools called 'parse(...)' followed by 'compileLexical()'; it is exactly
    /// that pairing for the single-source case. Returns a raw owning pointer
    /// (null on parse error), matching the C++ 'compileLexical' contract.
    pub fn compile(&mut self, lexc_source: &str) -> *mut HfstTransducer {
        self.parse(lexc_source);
        self.compile_lexical()
    }

    /// Walk the typed AST, dispatching each section/entry to the matching
    /// registration method. This is the AST-walk equivalent of the bison
    /// semantic actions ('handle_multichar' / 'handle_noflag' /
    /// 'handle_definition' / 'handle_lexicon_name' / 'handle_string_entry' /
    /// 'handle_string_pair_entry' / 'handle_regexp_entry').
    pub fn compile_file(&mut self, ast: &LexcFile) {
        for mc in &ast.multichars {
            self.add_alphabet(&mc.value.0);
        }
        for nf in &ast.noflags {
            self.add_no_flag(&nf.value.0);
        }
        for def in &ast.definitions {
            let body = nfst_xre::pretty_print(&def.value.body);
            self.add_xre_definition(&def.value.name, &body);
        }
        for lex in &ast.lexicons {
            // mirror the C++ titlecase-'Lexicon' warning (lexc-parser.yy
            // LEXICON_START_WRONG_CASE), emitted before the lexicon is set.
            if lex.value.case_warning {
                if self.treat_warnings_as_errors_ {
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
                        self.add_xre_entry(&r, &e.continuation, weight);
                    }
                }
            }
        }
    }
}

// ===== body 1 (flattened, module scope) =====
impl LexcCompiler {
    // [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.compile-lexical-fn]
    // [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.compile-lexical-fn]
    //
    // Port of 'LexcCompiler::compileLexical()' (LexcCompiler.cc ~1300-1699).
    //
    // The C++ 'std::ostream *err = get_stream(error_)' plumbing and every
    // 'flush(err)' are folded into plain 'eprintln!'/'eprint!' to stderr, per the
    // module docs (the non-WINDOWS code path). The 'if (debug)' AT&T dumps are
    // omitted because the file-global 'bool debug' is always 'false'. The
    // 'COLOUR_*' '#define's are inlined as literal ANSI escapes to avoid a
    // duplicate module-scope definition with the lexc-utils body (which owns
    // 'should_colourise').
    pub fn compile_lexical(&mut self) -> *mut HfstTransducer {
        if self.parseErrors_ {
            eprintln!("compilation aborted due to previous errors");
            return std::ptr::null_mut();
        }
        let mut warnings_generated = false;
        self.print_connectedness(&mut warnings_generated);
        if warnings_generated && self.treat_warnings_as_errors_ {
            if should_colourise() {
                eprint!("\u{1b}[31m*** ERROR: \u{1b}[0m");
            }
            eprintln!("missing or unused LEXICONs (see above) and -Werror has been enabled");
            return std::ptr::null_mut();
        }

        let mut lexicons = HfstTransducer::new_from_basic(&self.stringsTrie_, self.format_);

        lexicons.optimize();

        // repeat star to overgenerate
        lexicons.repeat_star().optimize();

        let mut small_substitutions = HfstSymbolSubstitutions::new();
        small_substitutions.insert("@0@".to_string(), "@_EPSILON_SYMBOL_@".to_string());
        small_substitutions.insert(
            "@@ANOTHER_EPSILON@@".to_string(),
            "@_EPSILON_SYMBOL_@".to_string(),
        );
        small_substitutions.insert("@ZERO@".to_string(), "0".to_string());

        lexicons.substitute_symbol_substitutions(&small_substitutions);
        lexicons.prune_alphabet(true);

        let mut joiners_trie = HfstBasicTransducer::new();

        let mut all_joiners_to_epsilon = HfstSymbolSubstitutions::new();

        if !self.with_flags_ {
            let start_joiner = joiner_encode(&self.initialLexiconName_);
            let start =
                HfstTransducer::new_tokenized(&start_joiner, &self.tokenizer_, self.format_);
            let end_string = joiner_encode("#");
            let end = HfstTransducer::new_tokenized(&end_string, &self.tokenizer_, self.format_);
            // lexicons = start.concatenate(lexicons).concatenate(end).optimize();
            let mut bracketed = start;
            bracketed
                .concatenate(&lexicons, true)
                .concatenate(&end, true)
                .optimize();
            lexicons = bracketed;

            for s in &self.lexiconNames_ {
                if self.verbose_ {
                    eprint!("Morphotaxing... {} ", s);
                }
                let joiner_enc = joiner_encode(s);

                // joiners trie version (later compose)
                let doubled = format!("{}{}", joiner_enc, joiner_enc);
                let new_vector = self.tokenizer_.tokenize(&doubled, false);
                joiners_trie.disjunct_path(&new_vector, 0.0f32);

                all_joiners_to_epsilon.insert(joiner_enc, "@_EPSILON_SYMBOL_@".to_string());
            }

            let root_joiner = joiner_encode(&self.initialLexiconName_);
            let hash_joiner = joiner_encode("#");

            all_joiners_to_epsilon.insert(root_joiner, "@_EPSILON_SYMBOL_@".to_string());
            all_joiners_to_epsilon.insert(hash_joiner, "@_EPSILON_SYMBOL_@".to_string());
        } else {
            let root_p = flag_joiner_encode(&self.initialLexiconName_, false);
            let root_r = flag_joiner_encode(&self.initialLexiconName_, true);

            let start_p = HfstTransducer::new_tokenized(&root_p, &self.tokenizer_, self.format_);
            let _start_r = HfstTransducer::new_tokenized(&root_r, &self.tokenizer_, self.format_);

            let end_string_p = flag_joiner_encode("#", false);
            let end_string_r = flag_joiner_encode("#", true);

            self.tokenizer_.add_multichar_symbol(&end_string_p);
            self.tokenizer_.add_multichar_symbol(&end_string_r);

            let _end_p =
                HfstTransducer::new_tokenized(&end_string_p, &self.tokenizer_, self.format_);
            let end_r =
                HfstTransducer::new_tokenized(&end_string_r, &self.tokenizer_, self.format_);

            // lexicons = startP.concatenate(lexicons).concatenate(endR).optimize();
            let mut bracketed = start_p;
            bracketed
                .concatenate(&lexicons, true)
                .concatenate(&end_r, true)
                .optimize();
            lexicons = bracketed;

            for s in &self.lexiconNames_ {
                if self.verbose_ {
                    eprint!("Morphotaxing... {} ", s);
                }
                let flag_p_string = flag_joiner_encode(s, false);
                let flag_r_string = flag_joiner_encode(s, true);

                // joiners trie version (later compose)
                let combined = format!("{}{}", flag_p_string, flag_r_string);
                let new_vector = self.tokenizer_.tokenize(&combined, false);
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
                let alph2 = tr.get_output_symbol();

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
            self.tokenizer_.add_multichar_symbol(alph);
            let new_vector = self.tokenizer_.tokenize(alph, false);
            joiners_trie.disjunct_path(&new_vector, 0.0f32);
        }

        let mut joiners_all = HfstTransducer::new_from_basic(&joiners_trie, self.format_);

        joiners_all.repeat_star();
        joiners_all.optimize();

        lexicons.compose(&joiners_all, true).optimize();

        let mut all_substitutions = HfstSymbolSubstitutions::new();
        if self.with_flags_ {
            if self.verbose_ {
                eprintln!();
                eprintln!("Changing flags...");
            }
            let mut fake_flags_to_real_flags = HfstSymbolSubstitutions::new();
            // Change fake flags to real flags
            lexicons.prune_alphabet(true);

            let transducer_alphabet = lexicons.get_alphabet();
            for s in &transducer_alphabet {
                if s.starts_with('$') && s.ends_with('$') && s.len() > 2 {
                    let alph = s.replace('$', "@");
                    fake_flags_to_real_flags.insert(s.clone(), alph);
                }
            }
            all_substitutions.extend(fake_flags_to_real_flags);
        } else {
            all_substitutions.extend(all_joiners_to_epsilon);
        }

        lexicons
            .substitute_symbol_substitutions(&all_substitutions)
            .optimize();
        lexicons.prune_alphabet(true);

        // replace reg exp key with transducers
        if self.verbose_ {
            eprintln!();
            eprintln!("Inserting regular expressions...");
        }

        // substitute all reg expression into special, unharmonizible symbols
        let mut fake_regexpr_to_real = HfstSymbolSubstitutions::new();
        for key in self.regexps_.keys() {
            if key.starts_with('$') {
                // TODO: do this only for strings that look like $.....$
                let alph = key.replace('$', "@");
                fake_regexpr_to_real.insert(key.clone(), alph);
            }
        }
        lexicons
            .substitute_symbol_substitutions(&fake_regexpr_to_real)
            .optimize();
        lexicons.prune_alphabet(true);

        let mut reg_mark_to_tr: crate::hfst_basic_transducer::SubstMap = BTreeMap::new();

        for (key, tr) in self.regexps_.iter() {
            let alph = if key.starts_with('$') {
                // TODO: do this only for strings that look like $.....$
                key.replace('$', "@")
            } else {
                key.clone()
            };
            let btr = HfstBasicTransducer::from_transducer(tr);
            reg_mark_to_tr.insert(alph, btr);
        }

        let mut lexicons_basic = HfstBasicTransducer::from_transducer(&lexicons);
        lexicons_basic.substitute_subst_map(&mut reg_mark_to_tr, true);

        lexicons_basic.prune_alphabet(true);

        let mut rv = HfstTransducer::new_from_basic(&lexicons_basic, self.format_);

        // Preserve only first flag of consecutive P and R lexname flag series,
        // e.g. change P.LEXNAME.1 R.LEXNAME.1 P.LEXNAME.2 R.LEXNAME.2 into
        // P.LEXNAME.1
        if self.with_flags_ {
            let transducer_alphabet = rv.get_alphabet();
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

            let mut xre_comp = XreCompiler::new(self.format_);

            let flag_filter_ptr = xre_comp.compile(&flag_remover_regexp);
            let mut flag_filter = unsafe { *Box::from_raw(flag_filter_ptr) };
            flag_filter.optimize();
            let mut inverted_flag_filter = flag_filter.clone();
            inverted_flag_filter.invert().optimize();

            // [ [FLAG1 | FLAG2 ... FLAGN] -> 0 || [FLAG1 | FLAG2 ... FLAGN] _
            // ].inv
            //                        .o.
            //                       RESULT
            //                        .o.
            // [FLAG1 | FLAG2 ... FLAGN] -> 0 || [FLAG1 | FLAG2 ... FLAGN] _
            let mut filtered_lexicons = inverted_flag_filter;
            filtered_lexicons.compose(&rv, true);
            filtered_lexicons.compose(&flag_filter, true).optimize();

            rv.assign(&filtered_lexicons);
        }

        rv.optimize();

        if !self.quiet_ {
            eprintln!();
        }

        Box::into_raw(Box::new(rv))
    }

    // Port of 'LexcCompiler::printConnectedness(bool &warnings_generated)'
    // (LexcCompiler.cc ~1701-1765): the missing/unused-lexicon validation. The
    // C++ 'set_difference' over the sorted 'std::set's becomes 'BTreeSet'
    // differences (also sorted). The 'COLOUR_*' escapes are inlined to stderr,
    // and 'flush(err)' is dropped (folded into 'eprintln!').
    pub fn print_connectedness(&self, warnings_generated: &mut bool) -> &Self {
        if self.lexiconNames_ != self.continuations_ {
            let lex_minus_cont: Vec<&String> = self
                .lexiconNames_
                .difference(&self.continuations_)
                .collect();
            let cont_minus_lex: Vec<&String> = self
                .continuations_
                .difference(&self.lexiconNames_)
                .collect();
            if !cont_minus_lex.is_empty() {
                for s in &cont_minus_lex {
                    if !self.quiet_ && self.warn_missing_lexicons_ {
                        if should_colourise() && self.treat_warnings_as_errors_ {
                            eprint!("\u{1b}[31mERROR: \u{1b}[0m");
                        } else if should_colourise() {
                            eprint!("\u{1b}[33mWarning: \u{1b}[0m");
                        }
                        eprintln!(
                            "Sublexicon is mentioned but not defined. [-Wmissing-lexicons] ({}) ",
                            s
                        );
                    }
                    *warnings_generated = true;
                }
            }
            if !lex_minus_cont.is_empty() {
                *warnings_generated = true;
                if !self.quiet_ && self.warn_unused_lexicons_ {
                    if should_colourise() && self.treat_warnings_as_errors_ {
                        eprint!("\u{1b}[31mERROR: \u{1b}[0m");
                    } else if should_colourise() {
                        eprint!("\u{1b}[33mWarning: \u{1b}[0m");
                    }
                    eprintln!("Sublexicons defined but not used [-Wunused-lexicons]");
                    let mut line = String::new();
                    for s in &lex_minus_cont {
                        line.push_str(s);
                        line.push(' ');
                    }
                    eprintln!("{}", line);
                }
            }
        }
        self
    }
}
