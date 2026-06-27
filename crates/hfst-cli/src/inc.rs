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
//! annotations. Globals live in 'crate::globals' (in C they were '#include'd per
//! tool); the wrapped libc/format helpers live in 'crate::hfst_commandline'.

use crate::globals::{self, ColourTristate};
use crate::hfst_commandline;
use crate::hfst_getopt;
use libc::{EXIT_FAILURE, EXIT_SUCCESS, c_char, c_int};

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
    Return(c_int),
}

// ---------------------------------------------------------------------------
// local std-stream accessors + small string helpers (the C used the 'stdin' /
// 'stdout' / 'stderr' macros and the 'hfst_strdup' wrapper directly)
// ---------------------------------------------------------------------------

fn stdin_file() -> *mut libc::FILE {
    unsafe extern "C" {
        static mut stdin: *mut libc::FILE;
    }
    unsafe { stdin }
}
fn stdout_file() -> *mut libc::FILE {
    unsafe extern "C" {
        static mut stdout: *mut libc::FILE;
    }
    unsafe { stdout }
}
fn stderr_file() -> *mut libc::FILE {
    unsafe extern "C" {
        static mut stderr: *mut libc::FILE;
    }
    unsafe { stderr }
}

// 'hfst_strdup("literal")': duplicate a Rust string into a fresh C buffer.
fn strdup_str(s: &str) -> *mut c_char {
    let cs = std::ffi::CString::new(s).unwrap();
    unsafe { hfst_commandline::hfst_strdup(cs.as_ptr()) }
}

// Render a C string pointer as a Rust String for the &str-taking wrappers
// (hfst_fopen) and for '%s' interpolation (the C passed the char* straight in).
unsafe fn cstr_to_string(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    unsafe { std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned() }
}

// ---------------------------------------------------------------------------
// getopt-cases-common.h
// ---------------------------------------------------------------------------

/// The shared common-option switch cases ('-d -h -V -v -q -s -o --colour').
///
/// 'print_usage' is the tool's own usage printer (per-tool in C; passed in
/// here): invoked by the '-h' case before it returns EXIT_SUCCESS.
pub unsafe fn handle_common_case(c: c_int, print_usage: impl FnOnce()) -> CaseResult {
    unsafe {
        if c == b'd' as c_int {
            globals::DEBUG = true;
            CaseResult::Break
        } else if c == b'h' as c_int {
            print_usage();
            CaseResult::Return(EXIT_SUCCESS)
        } else if c == b'V' as c_int {
            hfst_commandline::print_version();
            CaseResult::Return(EXIT_SUCCESS)
        } else if c == b'v' as c_int {
            globals::VERBOSE = true;
            globals::SILENT = false;
            CaseResult::Break
        } else if c == b'q' as c_int || c == b's' as c_int {
            globals::VERBOSE = false;
            globals::SILENT = true;
            CaseResult::Break
        } else if c == b'o' as c_int {
            globals::OUTFILENAME = hfst_commandline::hfst_strdup(hfst_getopt::OPTARG);
            let name = cstr_to_string(globals::OUTFILENAME);
            globals::OUTFILE = hfst_commandline::hfst_fopen(&name, "w");
            if globals::OUTFILE == stdout_file() {
                libc::free(globals::OUTFILENAME as *mut libc::c_void);
                globals::OUTFILENAME = strdup_str("<stdout>");
                globals::MESSAGE_OUT = stderr_file();
            }
            globals::OUTPUT_NAMED = true;
            CaseResult::Break
        } else if c == hfst_commandline::GETOPT_COLOUR {
            let optarg = hfst_getopt::OPTARG;
            if optarg.is_null() {
                globals::COLOUR = ColourTristate::COLOUR_ALWAYS;
            } else if libc::strcmp(optarg, c"always".as_ptr()) == 0 {
                globals::COLOUR = ColourTristate::COLOUR_ALWAYS;
            } else if libc::strcmp(optarg, c"never".as_ptr()) == 0 {
                globals::COLOUR = ColourTristate::COLOUR_NEVER;
            } else if libc::strcmp(optarg, c"auto".as_ptr()) == 0 {
                globals::COLOUR = ColourTristate::COLOUR_AUTO;
            } else {
                hfst_commandline::error(
                    EXIT_FAILURE,
                    0,
                    &format!(
                        "--colour must be one of always, never, auto, not {}",
                        cstr_to_string(optarg)
                    ),
                );
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
pub unsafe fn handle_unary_case(c: c_int) -> CaseResult {
    unsafe {
        if c == b'i' as c_int {
            globals::INPUTFILENAME = hfst_commandline::hfst_strdup(hfst_getopt::OPTARG);
            let name = cstr_to_string(globals::INPUTFILENAME);
            globals::INPUTFILE = hfst_commandline::hfst_fopen(&name, "r");
            if globals::INPUTFILE == stdin_file() {
                libc::free(globals::INPUTFILENAME as *mut libc::c_void);
                globals::INPUTFILENAME = strdup_str("<stdin>");
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
pub unsafe fn handle_binary_case(c: c_int) -> CaseResult {
    unsafe {
        if c == b'1' as c_int {
            globals::FIRSTFILENAME = hfst_commandline::hfst_strdup(hfst_getopt::OPTARG);
            let name = cstr_to_string(globals::FIRSTFILENAME);
            globals::FIRSTFILE = hfst_commandline::hfst_fopen(&name, "r");
            if globals::FIRSTFILE == stdin_file() {
                libc::free(globals::FIRSTFILENAME as *mut libc::c_void);
                globals::FIRSTFILENAME = strdup_str("<stdin>");
                globals::IS_INPUT_STDIN = true;
            }
            globals::FIRST_NAMED = true;
            CaseResult::Break
        } else if c == b'2' as c_int {
            globals::SECONDFILENAME = hfst_commandline::hfst_strdup(hfst_getopt::OPTARG);
            let name = cstr_to_string(globals::SECONDFILENAME);
            globals::SECONDFILE = hfst_commandline::hfst_fopen(&name, "r");
            if globals::SECONDFILE == stdin_file() {
                libc::free(globals::SECONDFILENAME as *mut libc::c_void);
                globals::SECONDFILENAME = strdup_str("<stdin>");
                globals::IS_INPUT_STDIN = true;
            }
            globals::SECOND_NAMED = true;
            CaseResult::Break
        } else if c == b'C' as c_int {
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
pub unsafe fn handle_error_case(c: c_int) -> c_int {
    unsafe {
        if c == b'?' as c_int {
            hfst_commandline::print_short_help();
            if hfst_getopt::OPTOPT == b'c' as c_int {
                hfst_commandline::error(
                    EXIT_FAILURE,
                    0,
                    &format!(
                        "Option -{} requires an argument.\n",
                        hfst_getopt::OPTOPT as u8 as char
                    ),
                );
            } else if libc::isprint(hfst_getopt::OPTOPT) != 0 {
                hfst_commandline::error(
                    EXIT_FAILURE,
                    0,
                    &format!("Unknown option `-{}'.\n", hfst_getopt::OPTOPT as u8 as char),
                );
            } else {
                hfst_commandline::error(EXIT_FAILURE, 0, "Unknown option");
            }
            EXIT_FAILURE
        } else if c == b':' as c_int {
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
            globals::OUTFILENAME = strdup_str("<stdout>");
            globals::OUTFILE = stdout_file();
            globals::MESSAGE_OUT = stderr_file();
        }
    }
}

// ---------------------------------------------------------------------------
// check-params-unary.h
// ---------------------------------------------------------------------------

/// Post-parse resolution of the unary input file from the leftover free
/// argument ('argv[optind]'). 'optind' is read from the getopt globals.
pub unsafe fn check_unary_params(argc: c_int, argv: *mut *mut c_char) {
    unsafe {
        let optind = hfst_getopt::OPTIND;
        if !globals::INPUT_NAMED {
            if (argc - optind) == 1 {
                globals::INPUTFILENAME =
                    hfst_commandline::hfst_strdup(*argv.offset(optind as isize));
                let name = cstr_to_string(globals::INPUTFILENAME);
                globals::INPUTFILE = hfst_commandline::hfst_fopen(&name, "r");
                if globals::INPUTFILE == stdin_file() {
                    libc::free(globals::INPUTFILENAME as *mut libc::c_void);
                    globals::INPUTFILENAME = strdup_str("<stdin>");
                }
            } else if (argc - optind) > 1 {
                hfst_commandline::error(
                    EXIT_FAILURE,
                    0,
                    "no more than one transducer file may be given",
                );
            } else {
                globals::INPUTFILE = stdin_file();
                globals::INPUTFILENAME = strdup_str("<stdin>");
            }
        } else if (argc - optind) > 0 {
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
pub unsafe fn check_binary_params(argc: c_int, argv: *mut *mut c_char) {
    unsafe {
        let optind = hfst_getopt::OPTIND;
        if globals::FIRST_NAMED && globals::SECOND_NAMED {
            if (argc - optind) > 0 {
                // hfst-tool file1 file2 file3
                hfst_commandline::error(
                    EXIT_FAILURE,
                    0,
                    "No more than two transducer files may be given",
                );
            }
        } else if !globals::FIRST_NAMED && !globals::SECOND_NAMED {
            // neither input given in options:
            if (argc - optind) == 2 {
                // hfst-tool file1 file2
                globals::FIRSTFILENAME =
                    hfst_commandline::hfst_strdup(*argv.offset(optind as isize));
                let fname = cstr_to_string(globals::FIRSTFILENAME);
                globals::FIRSTFILE = hfst_commandline::hfst_fopen(&fname, "r");
                globals::SECONDFILENAME =
                    hfst_commandline::hfst_strdup(*argv.offset((optind + 1) as isize));
                let sname = cstr_to_string(globals::SECONDFILENAME);
                globals::SECONDFILE = hfst_commandline::hfst_fopen(&sname, "r");
                globals::IS_INPUT_STDIN = false;
            } else if (argc - optind) == 1 {
                // hfst-tool file2 < file1
                globals::SECONDFILENAME =
                    hfst_commandline::hfst_strdup(*argv.offset(optind as isize));
                let sname = cstr_to_string(globals::SECONDFILENAME);
                globals::SECONDFILE = hfst_commandline::hfst_fopen(&sname, "r");
                globals::FIRSTFILENAME = strdup_str("<stdin>");
                globals::FIRSTFILE = stdin_file();
                globals::IS_INPUT_STDIN = true;
            } else if (argc - optind) > 2 {
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
            if (argc - optind) == 1 {
                // hfst-tool file1 -2 file2
                globals::FIRSTFILENAME =
                    hfst_commandline::hfst_strdup(*argv.offset(optind as isize));
                let fname = cstr_to_string(globals::FIRSTFILENAME);
                globals::FIRSTFILE = hfst_commandline::hfst_fopen(&fname, "r");
                globals::IS_INPUT_STDIN = false;
            } else if (argc - optind) == 0 {
                // hfst-tool -2 file2 < file1
                globals::FIRSTFILENAME = strdup_str("<stdin>");
                globals::FIRSTFILE = stdin_file();
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
            if (argc - optind) == 1 {
                // hfst-tool file2 -1 file1
                globals::SECONDFILENAME =
                    hfst_commandline::hfst_strdup(*argv.offset(optind as isize));
                let sname = cstr_to_string(globals::SECONDFILENAME);
                globals::SECONDFILE = hfst_commandline::hfst_fopen(&sname, "r");
                globals::IS_INPUT_STDIN = false;
            } else if (argc - optind) == 0 {
                // hfst-tool -1 file1 < file2
                globals::SECONDFILENAME = strdup_str("<stdin>");
                globals::SECONDFILE = stdin_file();
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
