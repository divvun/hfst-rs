//! Shared tool-state globals, ported from tools/src/inc/globals-common.h +
//! globals-unary.h + globals-binary.h.
//!
//! In C these fragments are '#include'd once per tool .cc; in Rust they live
//! once here and every 'src/bin/<tool>.rs' references 'hfst_cli::globals::*'.
//!
//! The C kept open `FILE*` handles (message_out / outfile / inputfile /
//! firstfile / secondfile). After the io-foundation de-C-ism those `FILE*`
//! statics are gone: only the *FILENAME strings + the "named" flags survive, and
//! a tool opens its streams on demand as Rust `std::io` values through the
//! accessors below (`input_reader`/`output_writer`/`first_reader`/
//! `second_reader`/`message_writer`). The "<stdin>"/"<stdout>" sentinels (or "-",
//! or an unset name) select the standard streams.

use std::ffi::CStr;
use std::io::{BufRead, BufReader, Write};

use libc::c_char;

// colour tristate (hfst-commandline.h enum colour_tristate). Variant names kept
// SCREAMING bug-for-bug (matches the crate's ImplementationType style).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ColourTristate {
    COLOUR_NEVER,
    COLOUR_ALWAYS,
    COLOUR_AUTO,
}

// --- common (globals-common.h) ---

// defaults
pub static mut VERBOSE: bool = false;
pub static mut SILENT: bool = false;
pub static mut DEBUG: bool = false;
pub static mut COLOUR: ColourTristate = ColourTristate::COLOUR_AUTO;

// C: 'FILE *message_out = stdout;' — diagnostic/help output goes to stdout,
// EXCEPT when the tool's data output is itself stdout, in which case messages go
// to stderr so they do not corrupt the data stream. inc.rs sets this flag (the C
// did `message_out = stderr`).
pub static mut MESSAGE_TO_STDERR: bool = false;
pub static mut PROGRAM_NAME: *const c_char = std::ptr::null();
pub static mut HFST_TOOL_VERSION: *const c_char = std::ptr::null();
pub static mut HFST_TOOL_WIKINAME: *const c_char = std::ptr::null();
pub static mut OUTFILENAME: *mut c_char = std::ptr::null_mut();
pub static mut OUTPUT_NAMED: bool = false;

// --- unary (globals-unary.h); C marks these 'static' per-tool ---

pub static mut INPUTFILENAME: *mut c_char = std::ptr::null_mut();
pub static mut INPUT_NAMED: bool = false;

// --- binary (globals-binary.h) ---

pub static mut FIRSTFILENAME: *mut c_char = std::ptr::null_mut();
pub static mut FIRST_NAMED: bool = false;
pub static mut SECONDFILENAME: *mut c_char = std::ptr::null_mut();
pub static mut SECOND_NAMED: bool = false;
pub static mut IS_INPUT_STDIN: bool = true;
pub static mut ALLOW_TRANSDUCER_CONVERSION: bool = true;

// Resolve one of the *FILENAME `*mut c_char` globals to an owned String.
fn filename_of(ptr: *mut c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

// Open a named text input as a buffered reader; the "<stdin>"/"-"/unset names
// select stdin.
fn reader_for(name: String) -> std::io::Result<Box<dyn BufRead>> {
    if name == "<stdin>" || name == "-" || name.is_empty() {
        Ok(Box::new(BufReader::new(std::io::stdin())))
    } else {
        Ok(Box::new(BufReader::new(std::fs::File::open(&name)?)))
    }
}

/// Primary text input (INPUTFILENAME). The std counterpart of the old
/// `inputfile()` FILE*.
pub fn input_reader() -> std::io::Result<Box<dyn BufRead>> {
    reader_for(filename_of(unsafe { INPUTFILENAME }))
}

/// First binary-tool input (FIRSTFILENAME). The std counterpart of `firstfile()`.
pub fn first_reader() -> std::io::Result<Box<dyn BufRead>> {
    reader_for(filename_of(unsafe { FIRSTFILENAME }))
}

/// Second binary-tool input (SECONDFILENAME). The std counterpart of
/// `secondfile()`.
pub fn second_reader() -> std::io::Result<Box<dyn BufRead>> {
    reader_for(filename_of(unsafe { SECONDFILENAME }))
}

/// Primary text output (OUTFILENAME): stdout for the "<stdout>"/"-"/unset name,
/// else the named file. The std counterpart of the old `outfile()` FILE*.
pub fn output_writer() -> std::io::Result<Box<dyn Write>> {
    let name = filename_of(unsafe { OUTFILENAME });
    if name == "<stdout>" || name == "-" || name.is_empty() {
        Ok(Box::new(std::io::stdout()))
    } else {
        Ok(Box::new(std::fs::File::create(&name)?))
    }
}

/// Diagnostic/help output: stderr when the tool's data output is stdout (so the
/// streams don't mix), otherwise stdout. The std counterpart of the old
/// `message_out()` FILE*.
pub fn message_writer() -> Box<dyn Write> {
    if unsafe { MESSAGE_TO_STDERR } {
        Box::new(std::io::stderr())
    } else {
        Box::new(std::io::stdout())
    }
}
