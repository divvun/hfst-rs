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
use hfst::hfst_exception_defs::HfstFatalException;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use libc::{c_char, c_int};
use std::ffi::CStr;
use std::io::Write;

// ---------------------------------------------------------------------------
// constants (header #defines / constexpr)
// ---------------------------------------------------------------------------

/// option "character" for colour
pub const GETOPT_COLOUR: c_int = 27;

/// successful return value for argument parsing routine
pub const EXIT_CONTINUE: c_int = 42;

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
// internal helpers (no C counterpart; the C used fprintf/vfprintf directly)
// ---------------------------------------------------------------------------

// Write a Rust string slice to a writer verbatim (the C did fprintf(f, "%s", s)
// or fputs); keeps arbitrary bytes including ANSI escapes intact.
fn fput_str(f: &mut dyn Write, s: &str) {
    let _ = f.write_all(s.as_bytes());
}

// Write a NUL-terminated C string pointer to a writer (the C printed globals such
// as program_name with fprintf(f, "%s", p)).
fn fput_cstr(f: &mut dyn Write, p: *const c_char) {
    if !p.is_null() {
        let s = unsafe { CStr::from_ptr(p) }.to_string_lossy();
        let _ = f.write_all(s.as_bytes());
    }
}

// Current value of errno, as the C read it after a failing libc call.
fn errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

// strerror(errnum) appended to a FILE* (C: fprintf(stderr, "%s", strerror(e))).
fn fput_strerror(f: &mut dyn Write, errnum: i32) {
    fput_cstr(f, unsafe { libc::strerror(errnum) });
}

// ---------------------------------------------------------------------------
// error / warning printers
// ---------------------------------------------------------------------------

// [spec:hfst:def:hfst-commandline.error-at-line-fn]
// [spec:hfst:sem:hfst-commandline.error-at-line-fn]
pub fn error_at_line(status: i32, errnum: i32, filename: &str, linenum: u32, msg: &str) {
    let f = &mut std::io::stderr();
    unsafe {
        fput_str(f, &format!("{}.{}: ", filename, linenum));
        fput_str(f, msg);
        if errnum != 0 {
            fput_strerror(f, errnum);
        }
        fput_str(f, "\n");
    }
    if status != 0 {
        std::process::exit(status);
    }
}

// [spec:hfst:def:hfst-commandline.hfst-error-at-line-fn]
// [spec:hfst:sem:hfst-commandline.hfst-error-at-line-fn]
pub fn hfst_error_at_line(status: i32, errnum: i32, filename: &str, linenum: u32, msg: &str) {
    let f = &mut std::io::stderr();
    unsafe {
        maybe_print_colour(f, COLOUR_BOLD);
        fput_str(f, &format!("{}.{}: ", filename, linenum));
        maybe_print_colour(f, COLOUR_RED);
        fput_str(f, "Error: ");
        maybe_print_colour(f, COLOUR_RESET);
        fput_str(f, msg);
        if errnum != 0 {
            maybe_print_colour(f, COLOUR_MAGENTA);
            fput_strerror(f, errnum);
            maybe_print_colour(f, COLOUR_RESET);
        }
    }
    if status != 0 {
        std::process::exit(status);
    }
}

// [spec:hfst:def:hfst-commandline.hfst-warning-at-line-fn]
// [spec:hfst:sem:hfst-commandline.hfst-warning-at-line-fn]
pub fn hfst_warning_at_line(status: i32, errnum: i32, filename: &str, linenum: u32, msg: &str) {
    let f = &mut std::io::stderr();
    unsafe {
        maybe_print_colour(f, COLOUR_BOLD);
        fput_str(f, &format!("{}.{}: ", filename, linenum));
        maybe_print_colour(f, COLOUR_YELLOW);
        fput_str(f, "Warning: ");
        maybe_print_colour(f, COLOUR_RESET);
        fput_str(f, msg);
        if errnum != 0 {
            maybe_print_colour(f, COLOUR_MAGENTA);
            fput_strerror(f, errnum);
            maybe_print_colour(f, COLOUR_RESET);
        }
    }
    if status != 0 {
        std::process::exit(status);
    }
}

// [spec:hfst:def:hfst-commandline.error-fn]
// [spec:hfst:sem:hfst-commandline.error-fn]
pub fn error(status: i32, errnum: i32, msg: &str) {
    let f = &mut std::io::stderr();
    unsafe {
        fput_cstr(f, globals::PROGRAM_NAME);
        fput_str(f, ": ");
        fput_str(f, msg);
        if errnum != 0 {
            fput_strerror(f, errnum);
        }
        fput_str(f, "\n");
    }
    if status != 0 {
        std::process::exit(status);
    }
}

// [spec:hfst:def:hfst-commandline.hfst-error-fn]
// [spec:hfst:sem:hfst-commandline.hfst-error-fn]
pub fn hfst_error(status: i32, errnum: i32, msg: &str) {
    let f = &mut std::io::stderr();
    unsafe {
        maybe_print_colour(f, COLOUR_BOLD);
        fput_cstr(f, globals::PROGRAM_NAME);
        fput_str(f, ": ");
        maybe_print_colour(f, COLOUR_RED);
        fput_str(f, "Error: ");
        maybe_print_colour(f, COLOUR_RESET);
        fput_str(f, msg);
        if errnum != 0 {
            maybe_print_colour(f, COLOUR_MAGENTA);
            fput_strerror(f, errnum);
            maybe_print_colour(f, COLOUR_RESET);
        }
        fput_str(f, "\n");
    }
    if status != 0 {
        std::process::exit(status);
    }
}

// [spec:hfst:def:hfst-commandline.warning-fn]
// [spec:hfst:sem:hfst-commandline.warning-fn]
pub fn warning(status: i32, errnum: i32, msg: &str) {
    let f = &mut std::io::stderr();
    unsafe {
        fput_cstr(f, globals::PROGRAM_NAME);
        fput_str(f, ": warning: ");
        fput_str(f, msg);
        if errnum != 0 {
            fput_strerror(f, errnum);
        }
        fput_str(f, "\n");
    }
    if status != 0 {
        std::process::exit(status);
    }
}

// [spec:hfst:def:hfst-commandline.hfst-warning-fn]
// [spec:hfst:sem:hfst-commandline.hfst-warning-fn]
pub fn hfst_warning(status: i32, errnum: i32, msg: &str) {
    let f = &mut std::io::stderr();
    unsafe {
        maybe_print_colour(f, COLOUR_BOLD);
        fput_cstr(f, globals::PROGRAM_NAME);
        fput_str(f, ": ");
        maybe_print_colour(f, COLOUR_YELLOW);
        fput_str(f, "Warning: ");
        maybe_print_colour(f, COLOUR_RESET);
        fput_str(f, msg);
        if errnum != 0 {
            maybe_print_colour(f, COLOUR_MAGENTA);
            fput_strerror(f, errnum);
            maybe_print_colour(f, COLOUR_RESET);
        }
        fput_str(f, "\n");
    }
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
            unsafe { cstr_to_string(globals::PROGRAM_NAME) },
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
        unsafe {
            fput_str(f, "\nDEBUG: ");
            fput_str(f, msg);
            fput_str(f, "\n");
        }
    }
}

// [spec:hfst:def:hfst-commandline.verbose-printf-fn]
// [spec:hfst:sem:hfst-commandline.verbose-printf-fn]
pub fn verbose_printf(msg: &str) {
    if unsafe { globals::VERBOSE } {
        fput_str(&mut *globals::message_writer(), msg);
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
            HfstFatalException,
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
        fput_str(
            &mut std::io::stderr(),
            &format!(
                "Error: {} cannot process transducers that are in optimized lookup format.\n",
                program
            ),
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
    let cs = std::ffi::CString::new(s).unwrap();
    let mut endptr: *mut c_char = std::ptr::null_mut();
    let rv = unsafe { libc::strtod(cs.as_ptr(), &mut endptr) };
    if unsafe { *endptr } == 0 {
        rv as f32
    } else {
        hfst_error(libc::EXIT_FAILURE, errno(), &format!("{} not a weight", s));
        rv as f32
    }
}

// [spec:hfst:def:hfst-commandline.hfst-strtonumber-fn]
// [spec:hfst:sem:hfst-commandline.hfst-strtonumber-fn]
pub fn hfst_strtonumber(s: &str, infinite: Option<&mut bool>) -> i32 {
    let mut infinite = infinite;
    if let Some(ref mut b) = infinite {
        **b = false;
    }
    let cs = std::ffi::CString::new(s).unwrap();
    let mut endptr: *mut c_char = std::ptr::null_mut();
    let rv = unsafe { libc::strtod(cs.as_ptr(), &mut endptr) };
    if unsafe { *endptr } == 0 {
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
    } else {
        hfst_error(libc::EXIT_FAILURE, errno(), &format!("{} not a number", s));
        rv as i32
    }
}

// [spec:hfst:def:hfst-commandline.hfst-strtoul-fn]
// [spec:hfst:sem:hfst-commandline.hfst-strtoul-fn]
pub fn hfst_strtoul(s: &str, base: i32) -> u64 {
    let cs = std::ffi::CString::new(s).unwrap();
    let mut endptr: *mut c_char = std::ptr::null_mut();
    let rv = unsafe { libc::strtoul(cs.as_ptr(), &mut endptr, base) };
    if unsafe { *endptr } == 0 {
        rv as u64
    } else {
        hfst_error(
            libc::EXIT_FAILURE,
            errno(),
            &format!("{} is not a valid unsigned number string", s),
        );
        rv as u64
    }
}

// [spec:hfst:def:hfst-commandline.hfst-strtol-fn]
// [spec:hfst:sem:hfst-commandline.hfst-strtol-fn]
pub fn hfst_strtol(s: &str, base: i32) -> i64 {
    let cs = std::ffi::CString::new(s).unwrap();
    let mut endptr: *mut c_char = std::ptr::null_mut();
    let rv = unsafe { libc::strtol(cs.as_ptr(), &mut endptr, base) };
    if unsafe { *endptr } == 0 {
        rv as i64
    } else {
        hfst_error(
            libc::EXIT_FAILURE,
            errno(),
            &format!("{} is not a valid signed number string", s),
        );
        rv as i64
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
            libc::EXIT_FAILURE,
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

// [spec:hfst:def:hfst-commandline.hfst-close-fn]
// [spec:hfst:sem:hfst-commandline.hfst-close-fn]
pub fn hfst_close(fd: i32) -> i32 {
    let rv = unsafe { libc::close(fd) };
    if rv == -1 {
        hfst_error(libc::EXIT_FAILURE, errno(), "close failed");
    }
    rv
}

// [spec:hfst:def:hfst-commandline.hfst-open-fn]
// [spec:hfst:sem:hfst-commandline.hfst-open-fn]
pub fn hfst_open(pathname: &str, flags: i32) -> i32 {
    let path = std::ffi::CString::new(pathname).unwrap();
    let rv = unsafe { libc::open(path.as_ptr(), flags) };
    if rv == -1 {
        hfst_error(libc::EXIT_FAILURE, errno(), "open failed");
    }
    rv
}

// [spec:hfst:def:hfst-commandline.hfst-read-fn]
// [spec:hfst:sem:hfst-commandline.hfst-read-fn]
pub unsafe fn hfst_read(fd: i32, buf: *mut libc::c_void, count: usize) -> isize {
    if count > isize::MAX as usize {
        hfst_error(
            libc::EXIT_FAILURE,
            0,
            &format!("cannot read {} bytes in one read(2)", count),
        );
    }
    let rv = unsafe { libc::read(fd, buf, count) };
    if rv == -1 {
        hfst_error(libc::EXIT_FAILURE, errno(), "read failed");
    }
    rv
}

// [spec:hfst:def:hfst-commandline.hfst-write-fn]
// [spec:hfst:sem:hfst-commandline.hfst-write-fn]
pub unsafe fn hfst_write(fd: i32, buf: *const libc::c_void, count: usize) -> isize {
    let rv = unsafe { libc::write(fd, buf, count) };
    if rv == -1 {
        hfst_error(libc::EXIT_FAILURE, errno(), "write failed");
    }
    rv
}

// [spec:hfst:def:hfst-commandline.hfst-mkstemp-fn]
// [spec:hfst:sem:hfst-commandline.hfst-mkstemp-fn]
pub unsafe fn hfst_mkstemp(templ: *mut c_char) -> i32 {
    let rv = unsafe { libc::mkstemp(templ) };
    if rv == -1 {
        hfst_error(libc::EXIT_FAILURE, errno(), "mkstemp failed");
    }
    rv
}

// [spec:hfst:def:hfst-commandline.hfst-remove-fn]
// [spec:hfst:sem:hfst-commandline.hfst-remove-fn]
pub fn hfst_remove(filename: &str) -> i32 {
    let cs = std::ffi::CString::new(filename).unwrap();
    let rv = unsafe { libc::remove(cs.as_ptr()) };
    if rv == -1 {
        hfst_error(
            libc::EXIT_FAILURE,
            errno(),
            &format!("remove {} failed", filename),
        );
    }
    rv
}

// ---------------------------------------------------------------------------
// string functions
// ---------------------------------------------------------------------------

// [spec:hfst:def:hfst-commandline.hfst-strdup-fn]
// [spec:hfst:sem:hfst-commandline.hfst-strdup-fn]
pub unsafe fn hfst_strdup(s: *const c_char) -> *mut c_char {
    let rv = unsafe { libc::strdup(s) };
    if rv.is_null() {
        hfst_error(libc::EXIT_FAILURE, errno(), "strdup failed");
    }
    rv
}

// [spec:hfst:def:hfst-commandline.hfst-strndup-fn]
// [spec:hfst:sem:hfst-commandline.hfst-strndup-fn]
pub unsafe fn hfst_strndup(s: *const c_char, n: usize) -> *mut c_char {
    let rv = unsafe { libc::strndup(s, n) };
    if rv.is_null() {
        hfst_error(libc::EXIT_FAILURE, errno(), "strndup failed");
    }
    rv
}

// hfst_getdelim / hfst_getline (the FILE*-based line readers) were removed in the
// io-foundation de-C-ism: tools read from globals::input_reader() (a BufRead) via
// read_line / read_until / lines().

// [spec:hfst:def:hfst-commandline.readline-fn]
// [spec:hfst:sem:hfst-commandline.readline-fn]
// The non-readline fallback. The real readline-library path is #if'd out, so
// this is the active implementation.
fn readline(prompt: &str) -> *mut c_char {
    {
        let mut mw = globals::message_writer();
        fput_str(&mut *mw, prompt);
        let _ = mw.flush();
    }
    let mut line = String::new();
    match std::io::stdin().read_line(&mut line) {
        Ok(0) | Err(_) => std::ptr::null_mut(),
        Ok(_) => {
            // Hand the caller a malloc'd C string (matching getline's buffer, which
            // the caller frees). read_line keeps the trailing '\n', as getline did.
            let c = std::ffi::CString::new(line).unwrap_or_default();
            unsafe { libc::strdup(c.as_ptr()) }
        }
    }
}

// [spec:hfst:def:hfst-commandline.hfst-readline-fn]
// [spec:hfst:sem:hfst-commandline.hfst-readline-fn]
pub fn hfst_readline(prompt: &str) -> *mut c_char {
    readline(prompt)
}

// [spec:hfst:def:hfst-commandline.hfst-setlocale-fn]
// [spec:hfst:sem:hfst-commandline.hfst-setlocale-fn]
pub fn hfst_setlocale() -> *mut c_char {
    let rv = unsafe { libc::setlocale(libc::LC_ALL, b"\0".as_ptr() as *const c_char) };
    if rv.is_null() {
        hfst_error(
            libc::EXIT_FAILURE,
            errno(),
            "Unable to set locale for character settings",
        );
    }
    rv
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
    let cs = std::ffi::CString::new(chosen).unwrap();
    unsafe {
        globals::PROGRAM_NAME = hfst_strdup(cs.as_ptr()) as *const c_char;
    }
}

// ---------------------------------------------------------------------------
// memory functions
// ---------------------------------------------------------------------------

// [spec:hfst:def:hfst-commandline.hfst-malloc-fn]
// [spec:hfst:sem:hfst-commandline.hfst-malloc-fn]
pub unsafe fn hfst_malloc(size: usize) -> *mut libc::c_void {
    let rv = unsafe { libc::malloc(size) };
    if rv.is_null() && size > 0 {
        hfst_error(libc::EXIT_FAILURE, errno(), "malloc failed");
    }
    rv
}

// [spec:hfst:def:hfst-commandline.hfst-calloc-fn]
// [spec:hfst:sem:hfst-commandline.hfst-calloc-fn]
// (Declared in hfst-commandline.h but never defined in the C sources; given the
// natural body matching hfst_malloc/hfst_realloc to satisfy the shared API.)
pub unsafe fn hfst_calloc(nmemb: usize, size: usize) -> *mut libc::c_void {
    let rv = unsafe { libc::calloc(nmemb, size) };
    if rv.is_null() && size > 0 {
        hfst_error(libc::EXIT_FAILURE, errno(), "calloc failed");
    }
    rv
}

// [spec:hfst:def:hfst-commandline.hfst-realloc-fn]
// [spec:hfst:sem:hfst-commandline.hfst-realloc-fn]
pub unsafe fn hfst_realloc(ptr: *mut libc::c_void, size: usize) -> *mut libc::c_void {
    let rv = unsafe { libc::realloc(ptr, size) };
    if rv.is_null() && size > 0 {
        hfst_error(libc::EXIT_FAILURE, errno(), "realloc failed");
    }
    rv
}

// ---------------------------------------------------------------------------
// customized default printouts for HFST tools
// ---------------------------------------------------------------------------

// [spec:hfst:def:hfst-commandline.hfst-set-program-name-fn]
// [spec:hfst:sem:hfst-commandline.hfst-set-program-name-fn]
pub fn hfst_set_program_name(argv0: &str, version_vector: &str, wikiname: &str) {
    set_program_name(argv0);
    let v = std::ffi::CString::new(version_vector).unwrap();
    let w = std::ffi::CString::new(wikiname).unwrap();
    unsafe {
        globals::HFST_TOOL_VERSION = hfst_strdup(v.as_ptr()) as *const c_char;
        globals::HFST_TOOL_WIKINAME = hfst_strdup(w.as_ptr()) as *const c_char;
    }
}

// [spec:hfst:def:hfst-commandline.print-short-help-fn]
// [spec:hfst:sem:hfst-commandline.print-short-help-fn]
pub fn print_short_help() {
    // C printed a one-line pointer to --help via message_out (see literal below).
    let mut mw = globals::message_writer();
    let f = &mut *mw;
    unsafe {
        fput_str(f, "Try ``");
        fput_cstr(f, globals::PROGRAM_NAME);
        fput_str(f, " --help'' for more information.\n");
    }
}

// print web site reference
// [spec:hfst:def:hfst-commandline.print-more-info-fn]
// [spec:hfst:sem:hfst-commandline.print-more-info-fn]
pub fn print_more_info() {
    let mut mw = globals::message_writer();
    let f = &mut *mw;
    unsafe {
        fput_cstr(f, globals::PROGRAM_NAME);
        fput_str(f, " home page: \n<");
        fput_str(f, WIKI_URL);
        fput_str(f, "/");
        fput_cstr(f, globals::HFST_TOOL_WIKINAME);
        fput_str(f, ">\n");
        fput_str(f, "General help using HFST software: \n<");
        fput_str(f, WIKI_URL);
        fput_str(f, ">\n");
    }
}

// print version message
// [spec:hfst:def:hfst-commandline.print-version-fn]
// [spec:hfst:sem:hfst-commandline.print-version-fn]
pub fn print_version() {
    let mut mw = globals::message_writer();
    let f = &mut *mw;
    unsafe {
        fput_cstr(f, globals::PROGRAM_NAME);
        fput_str(f, " ");
        fput_cstr(f, globals::HFST_TOOL_VERSION);
        fput_str(f, &format!(" ({})\n", PACKAGE_STRING));
        fput_str(
            f,
            "Copyright (C) 2017 University of Helsinki,\n\
             License GPLv3: GNU GPL version 3 <http://gnu.org/licenses/gpl.html>\n\
             This is free software: you are free to change and redistribute it.\n\
             There is NO WARRANTY, to the extent permitted by law.\n",
        );
    }
}

// [spec:hfst:def:hfst-commandline.print-report-bugs-fn]
// [spec:hfst:sem:hfst-commandline.print-report-bugs-fn]
pub fn print_report_bugs() {
    let mut mw = globals::message_writer();
    let f = &mut *mw;
    unsafe {
        fput_str(
            f,
            &format!(
                "Report bugs to <{}> or directly to our bug tracker at:\n<https://github.com/hfst/hfst/issues>\n",
                PACKAGE_BUGREPORT
            ),
        );
    }
}

// [spec:hfst:def:hfst-commandline.extend-options-getenv-fn]
// [spec:hfst:sem:hfst-commandline.extend-options-getenv-fn]
pub unsafe fn extend_options_getenv(argc: *mut c_int, argv: *mut *mut *mut c_char) {
    unsafe {
        let hfstopts = libc::getenv(b"HFST_OPTIONS\0".as_ptr() as *const c_char);
        if hfstopts.is_null() {
            return;
        }
        let mut p = hfstopts;
        let mut spaces: u32 = 0;
        while *p != 0 {
            if *p == b' ' as c_char {
                spaces += 1;
            }
            p = p.offset(1);
        }
        // we cannot realloc argv since it's magic
        let new_argv = hfst_malloc(
            std::mem::size_of::<*mut c_char>() * (*argc as usize + spaces as usize + 1),
        ) as *mut *mut c_char;
        libc::memcpy(
            new_argv as *mut libc::c_void,
            *argv as *const libc::c_void,
            std::mem::size_of::<*mut c_char>() * *argc as usize,
        );
        // there's this magic stuff with *argv that we shouldn't free it still
        *argv = new_argv;
        let space = b" \0".as_ptr() as *const c_char;
        let mut new_arg = libc::strtok(hfstopts, space);
        while !new_arg.is_null() {
            let new_arg_spot = (*argv).offset(*argc as isize);
            *new_arg_spot = hfst_strdup(new_arg);
            *argc += 1;
            new_arg = libc::strtok(std::ptr::null_mut(), space);
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
        unsafe { libc::isatty(1) != 0 }
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
        fput_str(f, colour);
    }
}

// ---------------------------------------------------------------------------
// small local utility (no C counterpart)
// ---------------------------------------------------------------------------

// Render a C string pointer as a Rust String for interpolation into a message
// (C interpolated such pointers straight into printf with %s).
unsafe fn cstr_to_string(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    unsafe { std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned() }
}
