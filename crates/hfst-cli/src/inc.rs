//! Faithful 1:1 port of the 'tools/src/inc/' fragments that every tool
//! '#include's into its own 'parse_options':
//!
//!   - getopt-cases-common.h / -unary.h / -binary.h / -error.h: the switch-case
//!     bodies handling the shared short/long options.
//!   - check-params-common.h / -unary.h / -binary.h: the post-parse validation
//!     that resolves the in/out filenames from the leftover free arguments.
//!
//! In C these are textual '#include's spliced into a 'switch (c)' / after the
//! getopt loop; here they are translated once into shared helpers the bin mains
//! call. A switch-case fragment becomes a function returning 'CaseResult': the
//! caller tries 'handle_common_case', then the unary/binary handler, then its
//! own tool-specific cases, then 'handle_error_case' (the '?'/':'/default arm).
//!
//! These fragments declare no manifest symbols, so they carry no '[spec]'
//! annotations. Globals live in 'crate::globals'; the leftover free arguments are
//! read from the program's `Vec<String>` after the getopt loop.

use crate::globals::{self, ColourTristate};
use crate::hfst_commandline;
use crate::hfst_getopt;

const EXIT_SUCCESS: i32 = 0;
const EXIT_FAILURE: i32 = 1;

/// A positional filename of "-" means stdin, matching the C++ `hfst_fopen`,
/// which returned `stdin` for "-" (the `<stdin>` sentinel the tools test for).
/// The `-1`/`-2` option cases already do this inline; the leftover free-argument
/// operands go through here.
fn stdin_if_dash(name: &str) -> &str {
    if name == "-" { "<stdin>" } else { name }
}

/// Result of dispatching one getopt character through a fragment handler.
///
/// In C the fragment is a run of 'case' labels inside a 'switch (c)': a matched
/// case either 'break's out of the switch (continuing the getopt loop) or
/// 'return's an exit code from 'parse_options'; an unmatched case falls through
/// to the next '#include'd group.
pub enum CaseResult {
    /// 'c' matched no case in this fragment; try the next handler group.
    NotHandled,
    /// 'c' matched a case that ended in 'break' — continue the getopt loop.
    Break,
    /// 'c' matched a case that ended in 'return <code>' from 'parse_options'.
    Return(i32),
}

// ---------------------------------------------------------------------------
// getopt-cases-common.h
// ---------------------------------------------------------------------------

/// The shared common-option switch cases ('-d -h -V -v -q -s -o --colour').
///
/// 'print_usage' is the tool's own usage printer (per-tool in C; passed in
/// here): invoked by the '-h' case before it returns EXIT_SUCCESS.
pub unsafe fn handle_common_case(c: i32, print_usage: impl FnOnce()) -> CaseResult {
    unsafe {
        if c == b'd' as i32 {
            globals::DEBUG = true;
            CaseResult::Break
        } else if c == b'h' as i32 {
            print_usage();
            CaseResult::Return(EXIT_SUCCESS)
        } else if c == b'V' as i32 {
            hfst_commandline::print_version();
            CaseResult::Return(EXIT_SUCCESS)
        } else if c == b'v' as i32 {
            globals::VERBOSE = true;
            globals::SILENT = false;
            CaseResult::Break
        } else if c == b'q' as i32 || c == b's' as i32 {
            globals::VERBOSE = false;
            globals::SILENT = true;
            CaseResult::Break
        } else if c == b'o' as i32 {
            globals::set_output_filename(hfst_getopt::optarg());
            // A "-" output name means stdout; messages then go to stderr so they
            // do not corrupt the data stream. output_writer() opens the real file
            // (or stdout, for the "<stdout>" sentinel) on demand.
            if globals::output_filename() == "-" {
                globals::set_output_filename("<stdout>");
                globals::MESSAGE_TO_STDERR = true;
            }
            globals::OUTPUT_NAMED = true;
            CaseResult::Break
        } else if c == hfst_commandline::GETOPT_COLOUR {
            match hfst_getopt::optarg_opt().as_deref() {
                None | Some("always") => globals::COLOUR = ColourTristate::COLOUR_ALWAYS,
                Some("never") => globals::COLOUR = ColourTristate::COLOUR_NEVER,
                Some("auto") => globals::COLOUR = ColourTristate::COLOUR_AUTO,
                Some(other) => {
                    hfst_commandline::error(
                        EXIT_FAILURE,
                        0,
                        &format!("--colour must be one of always, never, auto, not {}", other),
                    );
                }
            }
            CaseResult::Break
        } else {
            CaseResult::NotHandled
        }
    }
}

// ---------------------------------------------------------------------------
// getopt-cases-unary.h
// ---------------------------------------------------------------------------

/// The shared unary-tool input-option switch case ('-i / --input').
pub unsafe fn handle_unary_case(c: i32) -> CaseResult {
    unsafe {
        if c == b'i' as i32 {
            globals::set_input_filename(hfst_getopt::optarg());
            if globals::input_filename() == "-" {
                globals::set_input_filename("<stdin>");
            }
            globals::INPUT_NAMED = true;
            CaseResult::Break
        } else {
            CaseResult::NotHandled
        }
    }
}

// ---------------------------------------------------------------------------
// getopt-cases-binary.h
// ---------------------------------------------------------------------------

/// The shared binary-tool input-option switch cases
/// ('-1 / --input1', '-2 / --input2', '-C / --do-not-convert').
pub unsafe fn handle_binary_case(c: i32) -> CaseResult {
    unsafe {
        if c == b'1' as i32 {
            globals::set_first_filename(hfst_getopt::optarg());
            if globals::first_filename() == "-" {
                globals::set_first_filename("<stdin>");
                globals::IS_INPUT_STDIN = true;
            }
            globals::FIRST_NAMED = true;
            CaseResult::Break
        } else if c == b'2' as i32 {
            globals::set_second_filename(hfst_getopt::optarg());
            if globals::second_filename() == "-" {
                globals::set_second_filename("<stdin>");
                globals::IS_INPUT_STDIN = true;
            }
            globals::SECOND_NAMED = true;
            CaseResult::Break
        } else if c == b'C' as i32 {
            globals::ALLOW_TRANSDUCER_CONVERSION = false;
            CaseResult::Break
        } else {
            CaseResult::NotHandled
        }
    }
}

// ---------------------------------------------------------------------------
// getopt-cases-error.h
// ---------------------------------------------------------------------------

/// The shared error switch cases: '?' (unknown option), ':' (missing argument),
/// and the 'default' (invalid option). This is the terminal arm — every 'c'
/// that no earlier handler matched lands here, and each branch calls 'error'
/// (which exits) and then returns EXIT_FAILURE.
pub unsafe fn handle_error_case(c: i32) -> i32 {
    unsafe {
        if c == b'?' as i32 {
            hfst_commandline::print_short_help();
            if hfst_getopt::OPTOPT == b'c' as i32 {
                hfst_commandline::error(
                    EXIT_FAILURE,
                    0,
                    &format!(
                        "Option -{} requires an argument.\n",
                        hfst_getopt::OPTOPT as u8 as char
                    ),
                );
            } else if hfst_getopt::OPTOPT >= 0x20 && hfst_getopt::OPTOPT <= 0x7e {
                hfst_commandline::error(
                    EXIT_FAILURE,
                    0,
                    &format!("Unknown option `-{}'.\n", hfst_getopt::OPTOPT as u8 as char),
                );
            } else {
                hfst_commandline::error(EXIT_FAILURE, 0, "Unknown option");
            }
            EXIT_FAILURE
        } else if c == b':' as i32 {
            hfst_commandline::print_short_help();
            hfst_commandline::error(
                EXIT_FAILURE,
                0,
                &format!(
                    "Option -{} requires an argument",
                    hfst_getopt::OPTOPT as u8 as char
                ),
            );
            EXIT_FAILURE
        } else {
            hfst_commandline::print_short_help();
            hfst_commandline::error(
                EXIT_FAILURE,
                0,
                &format!("invalid option -{}", c as u8 as char),
            );
            EXIT_FAILURE
        }
    }
}

// ---------------------------------------------------------------------------
// check-params-common.h
// ---------------------------------------------------------------------------

/// Post-parse default for the common output stream: if '-o' was never given,
/// point the output at stdout and the messages at stderr.
pub unsafe fn check_common_params() {
    unsafe {
        if !globals::OUTPUT_NAMED {
            globals::set_output_filename("<stdout>");
            // Default data output is stdout, so messages go to stderr (the tool
            // opens stdout on demand via output_writer()).
            globals::MESSAGE_TO_STDERR = true;
        }
    }
}

// ---------------------------------------------------------------------------
// check-params-unary.h
// ---------------------------------------------------------------------------

/// Post-parse resolution of the unary input file from the leftover free
/// argument ('args[optind]'). 'optind' is read from the getopt globals.
pub unsafe fn check_unary_params(args: &[String]) {
    unsafe {
        let optind = hfst_getopt::OPTIND;
        let remaining = args.len() - optind;
        if !globals::INPUT_NAMED {
            if remaining == 1 {
                globals::set_input_filename(args[optind].clone());
                if globals::input_filename() == "-" {
                    globals::set_input_filename("<stdin>");
                }
            } else if remaining > 1 {
                hfst_commandline::error(
                    EXIT_FAILURE,
                    0,
                    "no more than one transducer file may be given",
                );
            } else {
                globals::set_input_filename("<stdin>");
            }
        } else if remaining > 0 {
            hfst_commandline::error(
                EXIT_FAILURE,
                0,
                "no more than one transducer filename may be given",
            );
        }
    }
}

// ---------------------------------------------------------------------------
// check-params-binary.h
// ---------------------------------------------------------------------------

/// Post-parse resolution of the two binary input files from the leftover free
/// arguments, honouring whichever of '-1'/'-2' was already supplied.
pub unsafe fn check_binary_params(args: &[String]) {
    unsafe {
        let optind = hfst_getopt::OPTIND;
        let remaining = args.len() - optind;
        if globals::FIRST_NAMED && globals::SECOND_NAMED {
            if remaining > 0 {
                // hfst-tool file1 file2 file3
                hfst_commandline::error(
                    EXIT_FAILURE,
                    0,
                    "No more than two transducer files may be given",
                );
            }
        } else if !globals::FIRST_NAMED && !globals::SECOND_NAMED {
            // neither input given in options:
            if remaining == 2 {
                // hfst-tool file1 file2
                globals::set_first_filename(stdin_if_dash(&args[optind]));
                globals::set_second_filename(stdin_if_dash(&args[optind + 1]));
                globals::IS_INPUT_STDIN = false;
            } else if remaining == 1 {
                // hfst-tool file2 < file1
                globals::set_second_filename(stdin_if_dash(&args[optind]));
                globals::set_first_filename("<stdin>");
                globals::IS_INPUT_STDIN = true;
            } else if remaining > 2 {
                hfst_commandline::error(
                    EXIT_FAILURE,
                    0,
                    "no more than two transducer filenames may be given",
                );
            } else {
                // hfst-tool < file1
                hfst_commandline::error(
                    EXIT_FAILURE,
                    0,
                    "at least one input must be from a named file",
                );
            }
        } else if !globals::FIRST_NAMED {
            if remaining == 1 {
                // hfst-tool file1 -2 file2
                globals::set_first_filename(stdin_if_dash(&args[optind]));
                globals::IS_INPUT_STDIN = false;
            } else if remaining == 0 {
                // hfst-tool -2 file2 < file1
                globals::set_first_filename("<stdin>");
                globals::IS_INPUT_STDIN = true;
            } else {
                // hfst-tool -2 file2 file1 file3
                hfst_commandline::error(
                    EXIT_FAILURE,
                    0,
                    "no more than two transducer filenames may be given",
                );
            }
        } else if !globals::SECOND_NAMED {
            if remaining == 1 {
                // hfst-tool file2 -1 file1
                globals::set_second_filename(stdin_if_dash(&args[optind]));
                globals::IS_INPUT_STDIN = false;
            } else if remaining == 0 {
                // hfst-tool -1 file1 < file2
                globals::set_second_filename("<stdin>");
                globals::IS_INPUT_STDIN = true;
            } else {
                // hfst-tool -1 file1 file2 file3
                hfst_commandline::error(
                    EXIT_FAILURE,
                    0,
                    "no more than two transducer filenames may be given",
                );
            }
        } else {
            // hfst-tool < file1
            hfst_commandline::error(
                EXIT_FAILURE,
                0,
                "at least one transducer filename must be given",
            );
        }
    }
}
