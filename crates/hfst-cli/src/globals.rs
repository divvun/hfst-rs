//! Shared tool-state globals, ported from tools/src/inc/globals-common.h +
//! globals-unary.h + globals-binary.h.
//!
//! In C these fragments are '#include'd once per tool .cc; in Rust they live
//! once here and every 'src/bin/<tool>.rs' references 'hfst_cli::globals::*'.
//!
//! The C kept open `FILE*` handles and `char*` filenames. After the de-C-isms
//! those are gone: the string-valued state is plain `String`, reached through the
//! accessors below (the statics are private because edition-2024 forbids
//! references to `static mut` and `String` is not `Copy`). A tool opens its
//! streams on demand through `input_reader`/`output_writer`/`first_reader`/
//! `second_reader`/`message_writer`; the "<stdin>"/"<stdout>" sentinels (or "-",
//! or an unset name) select the standard streams.

use std::io::{BufRead, BufReader, Write};

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
pub static mut OUTPUT_NAMED: bool = false;

// --- unary (globals-unary.h); C marks these 'static' per-tool ---

pub static mut INPUT_NAMED: bool = false;

// --- binary (globals-binary.h) ---

pub static mut FIRST_NAMED: bool = false;
pub static mut SECOND_NAMED: bool = false;
pub static mut IS_INPUT_STDIN: bool = true;
pub static mut ALLOW_TRANSDUCER_CONVERSION: bool = true;

// String-valued tool state. The storage is private; reach it through the
// generated get/set accessors. A "<stdin>"/"<stdout>"/"-"/empty filename selects
// the standard stream.
macro_rules! string_global {
    ($name:ident, $get:ident, $set:ident) => {
        static mut $name: String = String::new();
        /// Clone of the current value (empty when unset).
        pub fn $get() -> String {
            unsafe { (*std::ptr::addr_of!($name)).clone() }
        }
        /// Replace the current value.
        pub fn $set(value: impl Into<String>) {
            unsafe {
                *std::ptr::addr_of_mut!($name) = value.into();
            }
        }
    };
}

string_global!(PROGRAM_NAME, program_name, set_program_name);
string_global!(HFST_TOOL_VERSION, hfst_tool_version, set_hfst_tool_version);
string_global!(
    HFST_TOOL_WIKINAME,
    hfst_tool_wikiname,
    set_hfst_tool_wikiname
);
string_global!(OUTFILENAME, output_filename, set_output_filename);
string_global!(INPUTFILENAME, input_filename, set_input_filename);
string_global!(FIRSTFILENAME, first_filename, set_first_filename);
string_global!(SECONDFILENAME, second_filename, set_second_filename);

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
    reader_for(input_filename())
}

/// First binary-tool input (FIRSTFILENAME). The std counterpart of `firstfile()`.
pub fn first_reader() -> std::io::Result<Box<dyn BufRead>> {
    reader_for(first_filename())
}

/// Second binary-tool input (SECONDFILENAME). The std counterpart of
/// `secondfile()`.
pub fn second_reader() -> std::io::Result<Box<dyn BufRead>> {
    reader_for(second_filename())
}

/// Primary text output (OUTFILENAME): stdout for the "<stdout>"/"-"/unset name,
/// else the named file. The std counterpart of the old `outfile()` FILE*.
pub fn output_writer() -> std::io::Result<Box<dyn Write>> {
    let name = output_filename();
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
