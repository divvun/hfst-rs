//! Faithful 1:1 port of the shared tool-state globals from
//! tools/src/inc/globals-common.h + globals-unary.h + globals-binary.h.
//!
//! In C these fragments are '#include'd once per tool .cc (the unary/binary
//! variants even mark the symbols 'static', i.e. per-translation-unit). In Rust
//! they live once here and every 'src/bin/<tool>.rs' references
//! 'hfst_cli::globals::*'. These are pure file-scope data definitions with no
//! manifest symbols, so they carry no '[spec]' annotations.
//!
//! The 'static mut' + 'addr_of_mut!' accessor convention matches the rest of
//! the crate (see 'hfst_getopt.rs'): scalars and pointers are read and written
//! by value (no reference needed); std-stream defaults that cannot be
//! const-initialised are stored as 'null_mut()' and substituted through an
//! accessor fn (same shape as getopt's 'stderr_file()').

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

// C: 'FILE *message_out = stdout;' — cannot const-init to stdout, so store null
// and substitute through 'message_out()'.
pub static mut MESSAGE_OUT: *mut libc::FILE = std::ptr::null_mut();
pub static mut PROGRAM_NAME: *const c_char = std::ptr::null();
pub static mut HFST_TOOL_VERSION: *const c_char = std::ptr::null();
pub static mut HFST_TOOL_WIKINAME: *const c_char = std::ptr::null();
pub static mut OUTFILENAME: *mut c_char = std::ptr::null_mut();
pub static mut OUTFILE: *mut libc::FILE = std::ptr::null_mut();
pub static mut OUTPUT_NAMED: bool = false;

// --- unary (globals-unary.h); C marks these 'static' per-tool ---

pub static mut INPUTFILENAME: *mut c_char = std::ptr::null_mut();
// C: 'static FILE *inputfile = stdin;' — substituted through 'inputfile()'.
pub static mut INPUTFILE: *mut libc::FILE = std::ptr::null_mut();
pub static mut INPUT_NAMED: bool = false;

// --- binary (globals-binary.h) ---

pub static mut FIRSTFILENAME: *mut c_char = std::ptr::null_mut();
// C: 'FILE *firstfile = stdin;' — substituted through 'firstfile()'.
pub static mut FIRSTFILE: *mut libc::FILE = std::ptr::null_mut();
pub static mut FIRST_NAMED: bool = false;
pub static mut SECONDFILENAME: *mut c_char = std::ptr::null_mut();
// C: 'FILE *secondfile = stdin;' — substituted through 'secondfile()'.
pub static mut SECONDFILE: *mut libc::FILE = std::ptr::null_mut();
pub static mut SECOND_NAMED: bool = false;
pub static mut IS_INPUT_STDIN: bool = true;
pub static mut ALLOW_TRANSDUCER_CONVERSION: bool = true;

// Stream accessors substituting the std stream when the static is null. All
// message/printf output goes through 'message_out()'. Same shape as getopt's
// 'stderr_file()'.

/// MESSAGE_OUT, else libc 'stdout'.
pub fn message_out() -> *mut libc::FILE {
    let f = unsafe { MESSAGE_OUT };
    if f.is_null() { stdout_file() } else { f }
}

/// OUTFILE, else libc 'stdout'.
pub fn outfile() -> *mut libc::FILE {
    let f = unsafe { OUTFILE };
    if f.is_null() { stdout_file() } else { f }
}

/// INPUTFILE, else libc 'stdin'.
pub fn inputfile() -> *mut libc::FILE {
    let f = unsafe { INPUTFILE };
    if f.is_null() { stdin_file() } else { f }
}

/// FIRSTFILE, else libc 'stdin'.
pub fn firstfile() -> *mut libc::FILE {
    let f = unsafe { FIRSTFILE };
    if f.is_null() { stdin_file() } else { f }
}

/// SECONDFILE, else libc 'stdin'.
pub fn secondfile() -> *mut libc::FILE {
    let f = unsafe { SECONDFILE };
    if f.is_null() { stdin_file() } else { f }
}

// 'stdout'/'stdin' as FILE* (same shape as getopt's 'stderr_file()').
fn stdout_file() -> *mut libc::FILE {
    unsafe extern "C" {
        #[cfg_attr(target_os = "macos", link_name = "__stdoutp")]
        static mut stdout: *mut libc::FILE;
    }
    unsafe { stdout }
}

fn stdin_file() -> *mut libc::FILE {
    unsafe extern "C" {
        #[cfg_attr(target_os = "macos", link_name = "__stdinp")]
        static mut stdin: *mut libc::FILE;
    }
    unsafe { stdin }
}
