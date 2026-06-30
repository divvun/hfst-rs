//! Faithful 1:1 port of tools/src/hfst-commandline.{h,cc} — the shared
//! command-line infrastructure for every HFST tool: message printers (verbose,
//! debug, error, warning, colourised variants), the wrapped libc mem/IO helpers
//! that print an error and exit on failure, transducer-format parsing/printing,
//! string-to-number parsers, program-name/version bookkeeping, and the
//! HFST_OPTIONS environment-variable option extension.
//!
//! The shipped C portability shims that are compiled out on a normal POSIX
//! build (the '#ifndef HAVE_GETLINE' / HAVE_GETDELIM / HAVE_STRNDUP / HAVE_ISATTY
//! fallbacks and the WINDOWS-only arms) are omitted here, exactly as the SFST
//! backend and the real readline UI were dropped: the wrappers call the system
//! libc functions directly. The non-readline 'readline' fallback IS kept,
//! because the real readline library path is the one that is #if'd out.
//!
//! Globals live once in 'crate::globals'; in C they were '#include'd per tool.

use crate::globals::{self, ColourTristate};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use std::io::{IsTerminal, Write};

// ---------------------------------------------------------------------------
// constants (header #defines / constexpr)
// ---------------------------------------------------------------------------

/// option "character" for colour
pub const GETOPT_COLOUR: i32 = 27;

/// successful return value for argument parsing routine
pub const EXIT_CONTINUE: i32 = 42;

pub const WIKI_URL: &str = "https://github.com/hfst/hfst/wiki";

pub const COLOUR_BOLD: &str = "\x1b[01m";
pub const COLOUR_RED: &str = "\x1b[31m";
pub const COLOUR_GREEN: &str = "\x1b[32m";
pub const COLOUR_YELLOW: &str = "\x1b[33m";
pub const COLOUR_BLUE: &str = "\x1b[34m";
pub const COLOUR_MAGENTA: &str = "\x1b[35m";
pub const COLOUR_CYAN: &str = "\x1b[36m";
pub const COLOUR_RESET: &str = "\x1b[0m";

// PACKAGE_STRING / PACKAGE_BUGREPORT expand to "" when config.h is absent.
const PACKAGE_STRING: &str = "";
const PACKAGE_BUGREPORT: &str = "";

// ---------------------------------------------------------------------------
// internal helpers
// ---------------------------------------------------------------------------

// Current value of errno, as the C read it after a failing libc call.
fn errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

// The OS strerror text for `errnum` (C: strerror(e)).
fn strerror(errnum: i32) -> String {
    std::io::Error::from_raw_os_error(errnum).to_string()
}

// ---------------------------------------------------------------------------
// error / warning printers
// ---------------------------------------------------------------------------

// [spec:hfst:def:hfst-commandline.error-at-line-fn]
// [spec:hfst:sem:hfst-commandline.error-at-line-fn]
pub fn error_at_line(status: i32, errnum: i32, filename: &str, linenum: u32, msg: &str) {
    let f = &mut std::io::stderr();
    let _ = write!(f, "{}.{}: {}", filename, linenum, msg);
    if errnum != 0 {
        let _ = write!(f, "{}", strerror(errnum));
    }
    let _ = writeln!(f);
    if status != 0 {
        std::process::exit(status);
    }
}

// [spec:hfst:def:hfst-commandline.hfst-error-at-line-fn]
// [spec:hfst:sem:hfst-commandline.hfst-error-at-line-fn]
pub fn hfst_error_at_line(status: i32, errnum: i32, filename: &str, linenum: u32, msg: &str) {
    let f = &mut std::io::stderr();
    maybe_print_colour(f, COLOUR_BOLD);
    let _ = write!(f, "{}.{}: ", filename, linenum);
    maybe_print_colour(f, COLOUR_RED);
    let _ = write!(f, "Error: ");
    maybe_print_colour(f, COLOUR_RESET);
    let _ = write!(f, "{}", msg);
    if errnum != 0 {
        maybe_print_colour(f, COLOUR_MAGENTA);
        let _ = write!(f, "{}", strerror(errnum));
        maybe_print_colour(f, COLOUR_RESET);
    }
    if status != 0 {
        std::process::exit(status);
    }
}

// [spec:hfst:def:hfst-commandline.hfst-warning-at-line-fn]
// [spec:hfst:sem:hfst-commandline.hfst-warning-at-line-fn]
pub fn hfst_warning_at_line(status: i32, errnum: i32, filename: &str, linenum: u32, msg: &str) {
    let f = &mut std::io::stderr();
    maybe_print_colour(f, COLOUR_BOLD);
    let _ = write!(f, "{}.{}: ", filename, linenum);
    maybe_print_colour(f, COLOUR_YELLOW);
    let _ = write!(f, "Warning: ");
    maybe_print_colour(f, COLOUR_RESET);
    let _ = write!(f, "{}", msg);
    if errnum != 0 {
        maybe_print_colour(f, COLOUR_MAGENTA);
        let _ = write!(f, "{}", strerror(errnum));
        maybe_print_colour(f, COLOUR_RESET);
    }
    if status != 0 {
        std::process::exit(status);
    }
}

// [spec:hfst:def:hfst-commandline.error-fn]
// [spec:hfst:sem:hfst-commandline.error-fn]
pub fn error(status: i32, errnum: i32, msg: &str) {
    let f = &mut std::io::stderr();
    let _ = write!(f, "{}: {}", globals::program_name(), msg);
    if errnum != 0 {
        let _ = write!(f, "{}", strerror(errnum));
    }
    let _ = writeln!(f);
    if status != 0 {
        std::process::exit(status);
    }
}

// [spec:hfst:def:hfst-commandline.hfst-error-fn]
// [spec:hfst:sem:hfst-commandline.hfst-error-fn]
pub fn hfst_error(status: i32, errnum: i32, msg: &str) {
    let f = &mut std::io::stderr();
    maybe_print_colour(f, COLOUR_BOLD);
    let _ = write!(f, "{}: ", globals::program_name());
    maybe_print_colour(f, COLOUR_RED);
    let _ = write!(f, "Error: ");
    maybe_print_colour(f, COLOUR_RESET);
    let _ = write!(f, "{}", msg);
    if errnum != 0 {
        maybe_print_colour(f, COLOUR_MAGENTA);
        let _ = write!(f, "{}", strerror(errnum));
        maybe_print_colour(f, COLOUR_RESET);
    }
    let _ = writeln!(f);
    if status != 0 {
        std::process::exit(status);
    }
}

// [spec:hfst:def:hfst-commandline.warning-fn]
// [spec:hfst:sem:hfst-commandline.warning-fn]
pub fn warning(status: i32, errnum: i32, msg: &str) {
    let f = &mut std::io::stderr();
    let _ = write!(f, "{}: warning: {}", globals::program_name(), msg);
    if errnum != 0 {
        let _ = write!(f, "{}", strerror(errnum));
    }
    let _ = writeln!(f);
    if status != 0 {
        std::process::exit(status);
    }
}

// [spec:hfst:def:hfst-commandline.hfst-warning-fn]
// [spec:hfst:sem:hfst-commandline.hfst-warning-fn]
pub fn hfst_warning(status: i32, errnum: i32, msg: &str) {
    let f = &mut std::io::stderr();
    maybe_print_colour(f, COLOUR_BOLD);
    let _ = write!(f, "{}: ", globals::program_name());
    maybe_print_colour(f, COLOUR_YELLOW);
    let _ = write!(f, "Warning: ");
    maybe_print_colour(f, COLOUR_RESET);
    let _ = write!(f, "{}", msg);
    if errnum != 0 {
        maybe_print_colour(f, COLOUR_MAGENTA);
        let _ = write!(f, "{}", strerror(errnum));
        maybe_print_colour(f, COLOUR_RESET);
    }
    let _ = writeln!(f);
    if status != 0 {
        std::process::exit(status);
    }
}

// deprecated; everything's compatible
// [spec:hfst:def:hfst-commandline.get-compatible-fst-format-fn]
// [spec:hfst:sem:hfst-commandline.get-compatible-fst-format-fn]
pub fn get_compatible_fst_format() -> i32 {
    assert!(false);
    -1
}

// ---------------------------------------------------------------------------
// conditional printf wrappers
// ---------------------------------------------------------------------------

// [spec:hfst:def:hfst-commandline.debug-save-transducer-fn]
// [spec:hfst:sem:hfst-commandline.debug-save-transducer-fn]
pub fn debug_save_transducer(t: &HfstTransducer, name: &str) {
    if unsafe { globals::DEBUG } {
        // C built "DEBUG %s" with sprintf; that always succeeds here.
        let mut t = t.clone();
        let debug_name = format!("DEBUG {}", name);
        t.set_name(&debug_name);
        let mut debug_out = HfstOutputStream::new_filename(name, t.get_type(), true);
        debug_printf(&format!(
            "*** DEBUG ({}): saving current transducer to {}\n",
            globals::program_name(),
            name
        ));
        debug_out.redirect(&mut t);
        debug_out.close();
    }
}

// [spec:hfst:def:hfst-commandline.debug-printf-fn]
// [spec:hfst:sem:hfst-commandline.debug-printf-fn]
pub fn debug_printf(msg: &str) {
    if unsafe { globals::DEBUG } {
        let f = &mut std::io::stderr();
        let _ = write!(f, "\nDEBUG: {}\n", msg);
    }
}

// [spec:hfst:def:hfst-commandline.verbose-printf-fn]
// [spec:hfst:sem:hfst-commandline.verbose-printf-fn]
pub fn verbose_printf(msg: &str) {
    if unsafe { globals::VERBOSE } {
        let _ = write!(globals::message_writer(), "{}", msg);
    }
}

// ---------------------------------------------------------------------------
// format conversion helpers
// ---------------------------------------------------------------------------

// [spec:hfst:def:hfst-commandline.conversion-type-fn]
// [spec:hfst:sem:hfst-commandline.conversion-type-fn]
pub fn conversion_type(type1: ImplementationType, type2: ImplementationType) -> i32 {
    if type1 == type2 {
        return 0;
    }
    if HfstTransducer::is_safe_conversion(type2, type1) {
        1
    } else if HfstTransducer::is_safe_conversion(type1, type2) {
        2
    } else {
        -1
    }
}

// [spec:hfst:def:hfst-commandline.convert-transducers-fn]
// [spec:hfst:sem:hfst-commandline.convert-transducers-fn]
pub fn convert_transducers(first: &mut HfstTransducer, second: &mut HfstTransducer) {
    let type1 = first.get_type();
    let type2 = second.get_type();
    let ct = conversion_type(type1, type2);

    if ct == 0 {
    } else if ct == 1 {
        hfst_warning(
            0,
            0,
            &format!(
                "transducers have different types, converting to format {}\n",
                hfst_strformat(type1)
            ),
        );
        second.convert(type1, String::new());
    } else if ct == 2 {
        hfst_warning(
            0,
            0,
            &format!(
                "transducers have different types, converting to format {}\n",
                hfst_strformat(type2)
            ),
        );
        first.convert(type2, String::new());
    } else if ct == -1 {
        hfst_warning(
            0,
            0,
            &format!(
                "transducers have different types, converting to format {}, loss of information is possible\n",
                hfst_strformat(type1)
            ),
        );
        second.convert(type1, String::new());
    } else {
        // This should not happen.
        hfst::HFST_THROW_MESSAGE!(
            Fatal,
            "convert_transducers: conversion_type returned an invalid integer"
        );
    }
}

// [spec:hfst:def:hfst-commandline.is-input-stream-in-ol-format-fn]
// [spec:hfst:sem:hfst-commandline.is-input-stream-in-ol-format-fn]
pub fn is_input_stream_in_ol_format(is: &HfstInputStream, program: &str) -> bool {
    if is.get_type() == ImplementationType::HFST_OL_TYPE
        || is.get_type() == ImplementationType::HFST_OLW_TYPE
    {
        let _ = write!(
            std::io::stderr(),
            "Error: {} cannot process transducers that are in optimized lookup format.\n",
            program
        );
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// string -> number parsers
// ---------------------------------------------------------------------------

// [spec:hfst:def:hfst-commandline.hfst-strtoweight-fn]
// [spec:hfst:sem:hfst-commandline.hfst-strtoweight-fn]
pub fn hfst_strtoweight(s: &str) -> f32 {
    match s.parse::<f64>() {
        Ok(rv) => rv as f32,
        Err(_) => {
            hfst_error(1, errno(), &format!("{} not a weight", s));
            0.0
        }
    }
}

// [spec:hfst:def:hfst-commandline.hfst-strtonumber-fn]
// [spec:hfst:sem:hfst-commandline.hfst-strtonumber-fn]
pub fn hfst_strtonumber(s: &str, infinite: Option<&mut bool>) -> i32 {
    let mut infinite = infinite;
    if let Some(ref mut b) = infinite {
        **b = false;
    }
    let rv = match s.parse::<f64>() {
        Ok(rv) => rv,
        Err(_) => {
            hfst_error(1, errno(), &format!("{} not a number", s));
            return 0;
        }
    };
    if rv.is_infinite() && infinite.is_some() {
        if let Some(b) = infinite {
            *b = true;
        }
        // std::signbit(rv): 1 if negative, 0 otherwise.
        return if rv.is_sign_negative() { 1 } else { 0 };
    } else if rv > i32::MAX as f64 {
        return i32::MAX;
    } else if rv < i32::MIN as f64 {
        return i32::MIN;
    }
    rv.floor() as i32
}

// [spec:hfst:def:hfst-commandline.hfst-strtoul-fn]
// [spec:hfst:sem:hfst-commandline.hfst-strtoul-fn]
pub fn hfst_strtoul(s: &str, base: i32) -> u64 {
    match u64::from_str_radix(s, base as u32) {
        Ok(rv) => rv,
        Err(_) => {
            hfst_error(
                1,
                errno(),
                &format!("{} is not a valid unsigned number string", s),
            );
            0
        }
    }
}

// [spec:hfst:def:hfst-commandline.hfst-strtol-fn]
// [spec:hfst:sem:hfst-commandline.hfst-strtol-fn]
pub fn hfst_strtol(s: &str, base: i32) -> i64 {
    match i64::from_str_radix(s, base as u32) {
        Ok(rv) => rv,
        Err(_) => {
            hfst_error(
                1,
                errno(),
                &format!("{} is not a valid signed number string", s),
            );
            0
        }
    }
}

// ---------------------------------------------------------------------------
// transducer-format name parsing / printing
// ---------------------------------------------------------------------------

// [spec:hfst:def:hfst-commandline.hfst-parse-format-name-fn]
// [spec:hfst:sem:hfst-commandline.hfst-parse-format-name-fn]
pub fn hfst_parse_format_name(s: &str) -> ImplementationType {
    let lower = s.to_ascii_lowercase();
    let rv;
    if lower == "sfst" {
        rv = ImplementationType::SFST_TYPE;
    } else if lower == "openfst-tropical" || lower == "ofst-tropical" {
        rv = ImplementationType::TROPICAL_OPENFST_TYPE;
    } else if lower == "openfst-log" || lower == "ofst-log" {
        rv = ImplementationType::LOG_OPENFST_TYPE;
    } else if lower == "openfst" || lower == "ofst" {
        rv = ImplementationType::TROPICAL_OPENFST_TYPE;
        hfst_warning(
            0,
            0,
            &format!("Ambiguous format name {}, guessing openfst-tropical", s),
        );
    } else if lower == "foma" {
        rv = ImplementationType::FOMA_TYPE;
    } else if lower == "xfsm" {
        rv = ImplementationType::XFSM_TYPE;
    } else if lower == "optimized-lookup-unweighted" || lower == "olu" {
        rv = ImplementationType::HFST_OL_TYPE;
    } else if lower == "optimized-lookup-weighted" || lower == "olw" {
        rv = ImplementationType::HFST_OLW_TYPE;
    } else if lower == "optimized-lookup" || lower == "ol" {
        rv = ImplementationType::HFST_OLW_TYPE;
        hfst_warning(
            0,
            0,
            &format!(
                "Ambiguous format name {}, guessing optimized-lookup-weighted",
                s
            ),
        );
    } else {
        hfst_error(
            1,
            0,
            &format!("Could not parse format name from string {}", s),
        );
        return ImplementationType::UNSPECIFIED_TYPE;
    }
    rv
}

// [spec:hfst:def:hfst-commandline.hfst-strformat-fn]
// [spec:hfst:sem:hfst-commandline.hfst-strformat-fn]
pub fn hfst_strformat(format: ImplementationType) -> &'static str {
    match format {
        ImplementationType::SFST_TYPE => "SFST (1.4 compatible)",
        ImplementationType::TROPICAL_OPENFST_TYPE => "OpenFST, std arc, tropical semiring",
        ImplementationType::LOG_OPENFST_TYPE => "OpenFST, std arc, log semiring",
        ImplementationType::FOMA_TYPE => "foma",
        ImplementationType::XFSM_TYPE => "xfsm",
        ImplementationType::HFST_OL_TYPE => "Hfst's lookup optimized, unweighted",
        ImplementationType::HFST_OLW_TYPE => "Hfst's lookup optimized, weighted",
        ImplementationType::HFST2_TYPE => "Hfst 2 legacy (deprecated)",
        ImplementationType::ERROR_TYPE | ImplementationType::UNSPECIFIED_TYPE => {
            "ERROR (not a HFST supported transducer)"
        }
    }
}

// ---------------------------------------------------------------------------
// file functions
// ---------------------------------------------------------------------------
//
// The FILE*-based helpers (hfst_fopen / hfst_fseek / hfst_ftell / hfst_fread /
// hfst_fwrite / hfst_tmpfile) were removed in the io-foundation de-C-ism: tools
// open their I/O as std streams (globals::input_reader / output_writer / first_
// reader / second_reader, or std::fs::File) and use std::io::{Read, Write, Seek}.
// The fd-level wrappers (hfst_close / hfst_open / hfst_read / hfst_write /
// hfst_mkstemp / hfst_remove) had no callers and were dropped with the libc nuke.

// ---------------------------------------------------------------------------
// interactive line input
// ---------------------------------------------------------------------------

// [spec:hfst:def:hfst-commandline.hfst-readline-fn]
// [spec:hfst:sem:hfst-commandline.hfst-readline-fn]
//
// The non-readline fallback (the real readline-library path is #if'd out): print
// the prompt, then read a line (trailing '\n' kept, as getline did). None at EOF.
pub fn hfst_readline(prompt: &str) -> Option<String> {
    {
        let mut mw = globals::message_writer();
        let _ = write!(mw, "{}", prompt);
        let _ = mw.flush();
    }
    let mut line = String::new();
    match std::io::stdin().read_line(&mut line) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(line),
    }
}

// [spec:hfst:def:hfst-commandline.set-program-name-fn]
// [spec:hfst:sem:hfst-commandline.set-program-name-fn]
fn set_program_name(argv0: &str) {
    // this's gnulib
    let bytes = argv0.as_bytes();
    let slash = bytes.iter().rposition(|&c| c == b'/');
    let base = match slash {
        Some(i) => i + 1, // slash + 1
        None => 0,        // argv0
    };
    // base - argv0 >= 7 && strncmp(base - 7, "/.libs/", 7) == 0
    let mut start = 0usize;
    if base >= 7 && &bytes[base - 7..base] == b"/.libs/" {
        start = base;
        // strncmp(base, "lt-", 3) == 0
        if bytes.len() >= base + 3 && &bytes[base..base + 3] == b"lt-" {
            start = base + 3;
        }
    }
    let name = &argv0[start..];
    let chosen = if name == "hfst-calculate" {
        "hfst-sfstpl2fst"
    } else {
        name
    };
    globals::set_program_name(chosen);
}

// ---------------------------------------------------------------------------
// customized default printouts for HFST tools
// ---------------------------------------------------------------------------

// [spec:hfst:def:hfst-commandline.hfst-set-program-name-fn]
// [spec:hfst:sem:hfst-commandline.hfst-set-program-name-fn]
pub fn hfst_set_program_name(argv0: &str, version_vector: &str, wikiname: &str) {
    // Install the shared `tracing` subscriber once, idempotently. Library
    // diagnostics are already gated at their call sites (SILENT / verbose), so a
    // permissive (TRACE) subscriber renders exactly what the code chooses to
    // emit. Replaces the former hfst::set_warning_stream(&std::cerr).
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(std::io::stderr)
        .without_time()
        .with_target(false)
        .try_init();
    set_program_name(argv0);
    globals::set_hfst_tool_version(version_vector);
    globals::set_hfst_tool_wikiname(wikiname);
}

// [spec:hfst:def:hfst-commandline.print-short-help-fn]
// [spec:hfst:sem:hfst-commandline.print-short-help-fn]
pub fn print_short_help() {
    let mut mw = globals::message_writer();
    let _ = write!(
        mw,
        "Try ``{} --help'' for more information.\n",
        globals::program_name()
    );
}

// print web site reference
// [spec:hfst:def:hfst-commandline.print-more-info-fn]
// [spec:hfst:sem:hfst-commandline.print-more-info-fn]
pub fn print_more_info() {
    let mut mw = globals::message_writer();
    let _ = write!(
        mw,
        "{} home page: \n<{}/{}>\nGeneral help using HFST software: \n<{}>\n",
        globals::program_name(),
        WIKI_URL,
        globals::hfst_tool_wikiname(),
        WIKI_URL
    );
}

// print version message
// [spec:hfst:def:hfst-commandline.print-version-fn]
// [spec:hfst:sem:hfst-commandline.print-version-fn]
pub fn print_version() {
    let mut mw = globals::message_writer();
    let _ = write!(
        mw,
        "{} {} ({})\n",
        globals::program_name(),
        globals::hfst_tool_version(),
        PACKAGE_STRING
    );
    let _ = write!(
        mw,
        "Copyright (C) 2017 University of Helsinki,\n\
         License GPLv3: GNU GPL version 3 <http://gnu.org/licenses/gpl.html>\n\
         This is free software: you are free to change and redistribute it.\n\
         There is NO WARRANTY, to the extent permitted by law.\n",
    );
}

// [spec:hfst:def:hfst-commandline.print-report-bugs-fn]
// [spec:hfst:sem:hfst-commandline.print-report-bugs-fn]
pub fn print_report_bugs() {
    let mut mw = globals::message_writer();
    let _ = write!(
        mw,
        "Report bugs to <{}> or directly to our bug tracker at:\n<https://github.com/hfst/hfst/issues>\n",
        PACKAGE_BUGREPORT
    );
}

// [spec:hfst:def:hfst-commandline.extend-options-getenv-fn]
// [spec:hfst:sem:hfst-commandline.extend-options-getenv-fn]
//
// Append the space-separated tokens of $HFST_OPTIONS to the program arguments
// (consecutive spaces collapse, as the C strtok loop did); getopt then permutes
// them into place.
pub fn extend_options_getenv(args: &mut Vec<String>) {
    if let Ok(hfstopts) = std::env::var("HFST_OPTIONS") {
        for t in hfstopts.split(' ').filter(|t| !t.is_empty()) {
            args.push(t.to_string());
        }
    }
}

// ---------------------------------------------------------------------------
// colour
// ---------------------------------------------------------------------------

// [spec:hfst:def:hfst-commandline.should-colourise-fn]
// [spec:hfst:sem:hfst-commandline.should-colourise-fn]
pub fn should_colourise() -> bool {
    let colour = unsafe { globals::COLOUR };
    if colour == ColourTristate::COLOUR_AUTO {
        // this is not the best heuristic but wfm
        std::io::stdout().is_terminal()
    } else if colour == ColourTristate::COLOUR_ALWAYS {
        true
    } else if colour == ColourTristate::COLOUR_NEVER {
        false
    } else {
        assert!(false);
        false
    }
}

// [spec:hfst:def:hfst-commandline.maybe-print-colour-fn]
// [spec:hfst:sem:hfst-commandline.maybe-print-colour-fn]
pub fn maybe_print_colour(f: &mut dyn Write, colour: &str) {
    if should_colourise() {
        let _ = write!(f, "{}", colour);
    }
}
