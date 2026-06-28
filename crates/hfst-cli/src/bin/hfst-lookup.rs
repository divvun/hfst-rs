#![allow(static_mut_refs)]
//! Faithful 1:1 port of tools/src/hfst-lookup.cc — the transducer lookup
//! (apply) command-line tool. Lookup is done from left to right (as opposed to
//! xfst and foma, which look up from right to left; for that behaviour use
//! hfst-flookup). Drives the hfst-cli foundation (globals, getopt, commandline,
//! program-options, inc fragments).
//!
//! This is a unary tool (#includes inc/globals-unary.h, getopt-cases-unary.h,
//! check-params-unary.h); it mirrors hfst-invert's option-parsing skeleton and
//! adds the tool-specific options.

use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_data_types::{
    HfstOneLevelPath, HfstOneLevelPaths, HfstTwoLevelPaths, ImplementationType, StringPairVector,
    StringVector,
};
use hfst::hfst_flag_diacritics::FdOperation;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_strings2_fst_tokenizer::HfstStrings2FstTokenizer;
use hfst::hfst_symbol_defs::StringSet;
use hfst::hfst_symbol_defs::{internal_identity, internal_unknown, is_epsilon};
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, extend_options_getenv, hfst_error, hfst_error_at_line, hfst_set_program_name,
    hfst_setlocale, hfst_strformat, hfst_warning, print_more_info, print_report_bugs,
    verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT, hfst_getopt_common_long,
    hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_parameter_instructions,
};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use libc::{c_char, c_int};
use std::ffi::{CStr, CString};

// ---------------------------------------------------------------------------
// tools-specific global state (the C++ file's static variables)
// ---------------------------------------------------------------------------

static mut LOOKUP_FILE_NAME: *mut c_char = std::ptr::null_mut();
static mut LOOKUP_FILE: *mut libc::FILE = std::ptr::null_mut();
static mut PIPE_INPUT: bool = false;
static mut PIPE_OUTPUT: bool = false;
static mut LINEN: usize = 0;
static mut LOOKUP_GIVEN: bool = false;
static mut INFINITE_CUTOFF: usize = 5;
// max_number is size_t = -1 (SIZE_MAX) by default, meaning "no limit"; modelled
// here as isize -1 (which lookup_fd / lookup_pairs treat as unlimited).
static mut MAX_NUMBER: isize = -1;
const DEFAULT_MAX_NUMBER: isize = 5; // the C++ static MAX_NUMBER = 5
static mut BEAM: f32 = -1.0;

const CASCADE_UNION: i32 = 1;
const CASCADE_PRIORITY_UNION: i32 = 2;
const CASCADE_COMPOSITION: i32 = 3;
static mut CASCADE: i32 = CASCADE_UNION;

// symbols actually seen in (non-ol) transducers
static mut CASCADE_SYMBOLS_SEEN: Vec<StringSet> = Vec::new();
static mut CASCADE_UNKNOWN_OR_IDENTITY_SEEN: Vec<bool> = Vec::new();

// [spec:hfst:def:hfst-lookup.lookup-input-format]
#[derive(Clone, Copy, PartialEq)]
enum LookupInputFormat {
    Utf8TokenInput,
    SpaceSeparatedTokenInput,
    ApertiumInput,
}

// [spec:hfst:def:hfst-lookup.lookup-output-format]
#[derive(Clone, Copy, PartialEq)]
enum LookupOutputFormat {
    XeroxOutput,
    CgOutput,
    ApertiumOutput,
}

static mut INPUT_FORMAT: LookupInputFormat = LookupInputFormat::Utf8TokenInput;
static mut OUTPUT_FORMAT: LookupOutputFormat = LookupOutputFormat::XeroxOutput;
static mut TIME_CUTOFF: f64 = 0.0;

// XFST variables for apply
static mut SHOW_FLAGS: bool = false;
static mut OBEY_FLAGS: bool = true;
static mut PRINT_PAIRS: bool = false;
static mut PRINT_SPACE: bool = false;
static mut QUOTE_SPECIAL: bool = false;

static mut EPSILON_FORMAT: *mut c_char = std::ptr::null_mut();
static mut SPACE_FORMAT: *mut c_char = std::ptr::null_mut();

// the formats for lookup cases go like so:
//  BEGIN LOOKUP LOOKUP LOOKUP... END
// for standard case of more than 0 and less than infinite results:
static mut BEGIN_SETF: *mut c_char = std::ptr::null_mut(); // print before set of lookups
static mut LOOKUPF: *mut c_char = std::ptr::null_mut(); // print before each lookup
static mut END_SETF: *mut c_char = std::ptr::null_mut(); // print for each lookup
// when there are 0 results:
static mut EMPTY_BEGIN_SETF: *mut c_char = std::ptr::null_mut();
static mut EMPTY_LOOKUPF: *mut c_char = std::ptr::null_mut();
static mut EMPTY_END_SETF: *mut c_char = std::ptr::null_mut();
// when there are 0 results and token is unrecognizable by analyser:
static mut UNKNOWN_BEGIN_SETF: *mut c_char = std::ptr::null_mut();
static mut UNKNOWN_LOOKUPF: *mut c_char = std::ptr::null_mut();
static mut UNKNOWN_END_SETF: *mut c_char = std::ptr::null_mut();
// when there are infinite results:
static mut INFINITE_BEGIN_SETF: *mut c_char = std::ptr::null_mut();
static mut INFINITE_LOOKUPF: *mut c_char = std::ptr::null_mut();
static mut INFINITE_END_SETF: *mut c_char = std::ptr::null_mut();

static mut PRINT_STATISTICS: bool = false;
static mut SHOW_PROGRESS_BAR: bool = false;

// predefined formats
// Xerox
const XEROX_BEGIN_SETF: &str = "";
const XEROX_LOOKUPF: &str = "%i\t%l\t%w%n";
const XEROX_END_SETF: &str = "%n";
const XEROX_EMPTY_BEGIN_SETF: &str = "";
const XEROX_EMPTY_LOOKUPF: &str = "%i\t%i+?\t%w%n";
const XEROX_EMPTY_END_SETF: &str = "%n";
const XEROX_UNKNOWN_BEGIN_SETF: &str = "";
const XEROX_UNKNOWN_LOOKUPF: &str = "%i\t%i+?\t%w%n";
const XEROX_UNKNOWN_END_SETF: &str = "%n";
const XEROX_INFINITE_BEGIN_SETF: &str = "";
const XEROX_INFINITE_LOOKUPF: &str = "%i\t%l\t%w%n";
const XEROX_INFINITE_END_SETF: &str = "%i\t[...cyclic...]%n%n";
// CG
const CG_BEGIN_SETF: &str = "\"<%i>\"%n";
const CG_LOOKUPF: &str = "\t\"%b\"%a\t%w%n";
const CG_END_SETF: &str = "%n";
const CG_EMPTY_BEGIN_SETF: &str = "\"<%i>\"%n";
const CG_EMPTY_LOOKUPF: &str = "\t\"%i\" ?\tInf%n";
const CG_EMPTY_END_SETF: &str = "%n";
const CG_UNKNOWN_BEGIN_SETF: &str = "\"<%i>\"%n";
const CG_UNKNOWN_LOOKUPF: &str = "\t\"%i\"\t ?\tInf%n";
const CG_UNKNOWN_END_SETF: &str = "%n";
const CG_INFINITE_BEGIN_SETF: &str = "\"<%i>\"%n";
const CG_INFINITE_LOOKUPF: &str = "\t\"%b\"%a\t%w%n";
const CG_INFINITE_END_SETF: &str = "\t\"%i\"...cyclic...%n%n";
// apertium
const APERTIUM_BEGIN_SETF: &str = "^%i";
const APERTIUM_LOOKUPF: &str = "/%l";
const APERTIUM_END_SETF: &str = "$%m%n";
const APERTIUM_EMPTY_BEGIN_SETF: &str = "^%i";
const APERTIUM_EMPTY_LOOKUPF: &str = "/*%i";
const APERTIUM_EMPTY_END_SETF: &str = "$%m%n";
const APERTIUM_UNKNOWN_BEGIN_SETF: &str = " ";
const APERTIUM_UNKNOWN_LOOKUPF: &str = "%i%m";
const APERTIUM_UNKNOWN_END_SETF: &str = " ";
const APERTIUM_INFINITE_BEGIN_SETF: &str = "^%i";
const APERTIUM_INFINITE_LOOKUPF: &str = "/%l";
const APERTIUM_INFINITE_END_SETF: &str = "/...$%n";

// statistic counting
static mut INPUTS: u64 = 0;
static mut NO_ANALYSES: u64 = 0;
static mut ANALYSED: u64 = 0;
static mut ANALYSES: u64 = 0;

// which transducer in the cascade we are handling
static mut TRANSDUCER_NUMBER: u32 = 0;

// ---------------------------------------------------------------------------
// small C runtime shims used by the tool (strdup/getline have no foundation
// wrapper; reproduced here as raw libc, matching the C++ exactly)
// ---------------------------------------------------------------------------

unsafe fn cstr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

// 'hfst_strdup' replacement: duplicate a &str into a malloc'd C string.
unsafe fn strdup_str(s: &str) -> *mut c_char {
    let c = CString::new(s).unwrap_or_default();
    unsafe { libc::strdup(c.as_ptr()) }
}

unsafe fn fput(f: *mut libc::FILE, s: &str) {
    let c = CString::new(s).unwrap_or_default();
    unsafe { libc::fputs(c.as_ptr(), f) };
}

unsafe fn stdin_file() -> *mut libc::FILE {
    unsafe extern "C" {
        #[cfg_attr(target_os = "macos", link_name = "__stdinp")]
        static mut stdin: *mut libc::FILE;
    }
    unsafe { stdin }
}

unsafe fn stdout_file() -> *mut libc::FILE {
    unsafe extern "C" {
        #[cfg_attr(target_os = "macos", link_name = "__stdoutp")]
        static mut stdout: *mut libc::FILE;
    }
    unsafe { stdout }
}

// [spec:hfst:def:hfst-lookup.print-usage-fn]
// [spec:hfst:sem:hfst-lookup.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            globals::message_out(),
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\n\
                 perform transducer lookup (apply)\n\
                 NOTE: hfst-lookup does lookup from left to right as opposed to xfst and foma\n\
                 \x20     lookup which is carried out from right to left. In order to do lookup\n\
                 \x20     in a similar way as xfst and foma, use 'hfst-flookup' instead.\n\
                 \n",
                program_name
            ),
        );

        print_common_program_options(globals::message_out());
        fput(
            globals::message_out(),
            "Input/Output options:\n\
             \x20 -i, --input=INFILE       Read input transducer from INFILE\n\
             \x20 -o, --output=OUTFILE     Write output to OUTFILE\n\
             \x20 -p, --pipe-mode[=STREAM] Control input and output streams\n",
        );

        fput(
            globals::message_out(),
            "Lookup options:\n\
             \x20 -I, --input-strings=SFILE        Read lookup strings from SFILE\n\
             \x20 -O, --output-format=OFORMAT      Use OFORMAT printing results sets\n\
             \x20 -e, --epsilon-format=EPS         Print epsilon as EPS\n\
             \x20 -F, --input-format=IFORMAT       Use IFORMAT parsing input\n\
             \x20 -x, --statistics                 Print statistics\n\
             \x20 -X, --xfst=VARIABLE              Toggle xfst VARIABLE\n\
             \x20 -c, --cycles=INT                 How many times to follow input epsilon cycles\n\
             \x20                                  (only for non-lookup-optimized transducers)\n\
             \x20 -n, --max-number=INT             Maximum number of results printed for each input\n\
             \x20                                  (only for lookup-optimized transducers)\n\
             \x20 -b, --beam=B                     Output only analyses whose weight is within B from\n\
             \x20                                  the best analysis\n\
             \x20 -t, --time-cutoff=S              Limit search after having used S seconds per input\n\
             \x20                                  (only for lookup-optimized transducers)\n\
             \x20 -C, --cascade=CASCADE            How multiple transducers in input are handled\n\
             \x20 -P, --progress                   Show neat progress bar if possible\n",
        );
        fput(globals::message_out(), "\n");
        print_common_unary_program_parameter_instructions(globals::message_out());
        fput(
            globals::message_out(),
            "OFORMAT is one of {xerox,cg,apertium}, xerox being default\n\
             IFORMAT is one of {text,spaced,apertium}, default being text,\n\
             unless OFORMAT is apertium\n\
             VARIABLEs relevant to lookup are {print-pairs,print-space,\n\
             quote-special,show-flags,obey-flags}\n\
             Input epsilon cycles are followed by default INT=5 times.\n\
             Epsilon is printed by default as an empty string.\n\
             B must be a non-negative float.\n\
             S must be a non-negative float. The default, 0.0, indicates no cutoff.\n\
             If the input contains several transducers, a set containing\n\
             results from all transducers is printed for each input string.\n",
        );
        fput(globals::message_out(), "\n");

        fput(
            globals::message_out(),
            "CASCADE must be one of { union, priority-union, composition }.\n\
             If not specified, defaults to {union}.\n",
        );
        fput(globals::message_out(), "\n");

        fput(
            globals::message_out(),
            "STREAM can be { input, output, both }. If not given, defaults to {both}.\n\
             If input file is not specified with -I, input is read interactively line by\n\
             line from the user. If you redirect input from a file, use --pipe-mode=input.\n\
             --pipe-mode=output is ignored on non-windows platforms.\n",
        );
        fput(globals::message_out(), "\n");

        fput(
            globals::message_out(),
            "Todo:\n\
             \x20 Support --xfst=obey-flags for optimized lookup format.\n\
             \x20 Support --cycles for optimized lookup format.\n",
        );

        fput(
            globals::message_out(),
            "\n\
             Known bugs:\n\
             \x20 'quote-special' quotes spaces that come from 'print-space'\n",
        );

        fput(globals::message_out(), "\n");
        print_report_bugs();
        fput(globals::message_out(), "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-lookup.parse-options-fn]
// [spec:hfst:sem:hfst-lookup.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            for (name, has_arg, val) in [
                ("input-strings", 1, b'I'),
                ("output-format", 1, b'O'),
                ("input-format", 1, b'F'),
                ("statistics", 0, b'x'),
                ("cycles", 1, b'c'),
                ("max-number", 1, b'n'),
                ("xfst", 1, b'X'),
                ("epsilon-format", 1, b'e'),
                ("epsilon-format2", 1, b'E'),
                ("beam", 1, b'b'),
                ("time-cutoff", 1, b't'),
                ("pipe-mode", 2, b'p'),
                ("progress", 0, b'P'),
                ("cascade", 1, b'C'),
            ] {
                long_options.push(getopt::Option {
                    name: CString::new(name).unwrap().into_raw(),
                    has_arg,
                    flag: std::ptr::null_mut(),
                    val: val as c_int,
                });
            }
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}{}",
                HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT, "I:O:F:xc:n:X:e:E:b:t:p::PC:"
            ))
            .unwrap();
            let mut option_index: c_int = 0;
            let c = getopt::getopt_long(
                argc,
                argv,
                short.as_ptr(),
                long_options.as_ptr(),
                &mut option_index,
            );
            if -1 == c {
                break;
            }

            match handle_common_case(c, || print_usage()) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_unary_case(c) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }

            // add tool-specific cases here
            let optarg = cstr(getopt::OPTARG);
            match c as u8 {
                b'I' => {
                    LOOKUP_FILE_NAME = strdup_str(&optarg);
                    let mode = CString::new("r").unwrap();
                    LOOKUP_FILE = libc::fopen(LOOKUP_FILE_NAME, mode.as_ptr());
                    LOOKUP_GIVEN = true;
                }
                b'O' => {
                    if optarg == "xerox" {
                        OUTPUT_FORMAT = LookupOutputFormat::XeroxOutput;
                    } else if optarg == "cg" {
                        OUTPUT_FORMAT = LookupOutputFormat::CgOutput;
                    } else if optarg == "apertium" {
                        OUTPUT_FORMAT = LookupOutputFormat::ApertiumOutput;
                        INPUT_FORMAT = LookupInputFormat::ApertiumInput;
                    } else {
                        hfst_error(
                            libc::EXIT_FAILURE,
                            0,
                            &format!(
                                "Unknown output format {}; valid values are: xerox, cg, apertium\n",
                                optarg
                            ),
                        );
                        return libc::EXIT_FAILURE;
                    }
                }
                b'F' => {
                    if optarg == "text" {
                        INPUT_FORMAT = LookupInputFormat::Utf8TokenInput;
                    } else if optarg == "spaced" {
                        INPUT_FORMAT = LookupInputFormat::SpaceSeparatedTokenInput;
                    } else if optarg == "apertium" {
                        INPUT_FORMAT = LookupInputFormat::ApertiumInput;
                    } else {
                        hfst_error(
                            libc::EXIT_FAILURE,
                            0,
                            &format!(
                                "Unknown input format {}; valid values are:utf8, spaced, apertium\n",
                                optarg
                            ),
                        );
                        return libc::EXIT_FAILURE;
                    }
                }
                b'e' | b'E' => {
                    EPSILON_FORMAT = strdup_str(&optarg);
                }
                b'b' => {
                    BEAM = optarg.parse::<f32>().unwrap_or(0.0);
                    if BEAM < 0.0 {
                        eprint!("Invalid argument for --beam\n");
                        return libc::EXIT_FAILURE;
                    }
                }
                b't' => {
                    TIME_CUTOFF = optarg.parse::<f64>().unwrap_or(0.0);
                    if TIME_CUTOFF < 0.0 {
                        eprint!("Invalid argument for --time-cutoff\n");
                        return libc::EXIT_FAILURE;
                    }
                }
                b'x' => {
                    PRINT_STATISTICS = true;
                }
                b'X' => {
                    if optarg == "print-pairs" {
                        PRINT_PAIRS = true;
                    } else if optarg == "print-space" {
                        PRINT_SPACE = true;
                        SPACE_FORMAT = strdup_str(" ");
                    } else if optarg == "show-flags" {
                        SHOW_FLAGS = true;
                    } else if optarg == "quote-special" {
                        QUOTE_SPECIAL = true;
                    } else if optarg == "obey-flags" {
                        OBEY_FLAGS = false;
                    } else {
                        hfst_error(
                            libc::EXIT_FAILURE,
                            0,
                            &format!("Xfst variable {} unrecognised", optarg),
                        );
                    }
                }
                b'c' => {
                    INFINITE_CUTOFF = optarg.parse::<i32>().unwrap_or(0) as usize;
                }
                b'n' => {
                    MAX_NUMBER = optarg.parse::<i32>().unwrap_or(0) as isize;
                }
                b'p' => {
                    if getopt::OPTARG.is_null() {
                        PIPE_INPUT = true;
                        PIPE_OUTPUT = true;
                    } else if optarg == "both" || optarg == "BOTH" {
                        PIPE_INPUT = true;
                        PIPE_OUTPUT = true;
                    } else if optarg == "input"
                        || optarg == "INPUT"
                        || optarg == "in"
                        || optarg == "IN"
                    {
                        PIPE_INPUT = true;
                    } else if optarg == "output"
                        || optarg == "OUTPUT"
                        || optarg == "out"
                        || optarg == "OUT"
                    {
                        PIPE_OUTPUT = true;
                    } else {
                        hfst_error(
                            libc::EXIT_FAILURE,
                            0,
                            &format!("--pipe-mode argument {} unrecognised", optarg),
                        );
                    }
                }
                b'P' => {
                    SHOW_PROGRESS_BAR = true;
                }
                b'C' => {
                    if optarg == "union" {
                        CASCADE = CASCADE_UNION;
                    } else if optarg == "priority-union" {
                        CASCADE = CASCADE_PRIORITY_UNION;
                    } else if optarg == "composition" {
                        CASCADE = CASCADE_COMPOSITION;
                    } else {
                        hfst_error(
                            libc::EXIT_FAILURE,
                            0,
                            &format!(
                                "--cascade argument {} unrecognised, possible values are\n\
                                 {{ union, priority-union, composition }}",
                                optarg
                            ),
                        );
                    }
                }
                _ => {
                    return handle_error_case(c);
                }
            }
        }

        match OUTPUT_FORMAT {
            LookupOutputFormat::XeroxOutput => {
                BEGIN_SETF = strdup_str(XEROX_BEGIN_SETF);
                LOOKUPF = strdup_str(XEROX_LOOKUPF);
                END_SETF = strdup_str(XEROX_END_SETF);
                EMPTY_BEGIN_SETF = strdup_str(XEROX_EMPTY_BEGIN_SETF);
                EMPTY_LOOKUPF = strdup_str(XEROX_EMPTY_LOOKUPF);
                EMPTY_END_SETF = strdup_str(XEROX_EMPTY_END_SETF);
                UNKNOWN_BEGIN_SETF = strdup_str(XEROX_UNKNOWN_BEGIN_SETF);
                UNKNOWN_LOOKUPF = strdup_str(XEROX_UNKNOWN_LOOKUPF);
                UNKNOWN_END_SETF = strdup_str(XEROX_UNKNOWN_END_SETF);
                INFINITE_BEGIN_SETF = strdup_str(XEROX_INFINITE_BEGIN_SETF);
                INFINITE_LOOKUPF = strdup_str(XEROX_INFINITE_LOOKUPF);
                INFINITE_END_SETF = strdup_str(XEROX_INFINITE_END_SETF);
            }
            LookupOutputFormat::CgOutput => {
                BEGIN_SETF = strdup_str(CG_BEGIN_SETF);
                LOOKUPF = strdup_str(CG_LOOKUPF);
                END_SETF = strdup_str(CG_END_SETF);
                EMPTY_BEGIN_SETF = strdup_str(CG_EMPTY_BEGIN_SETF);
                EMPTY_LOOKUPF = strdup_str(CG_EMPTY_LOOKUPF);
                EMPTY_END_SETF = strdup_str(CG_EMPTY_END_SETF);
                UNKNOWN_BEGIN_SETF = strdup_str(CG_UNKNOWN_BEGIN_SETF);
                UNKNOWN_LOOKUPF = strdup_str(CG_UNKNOWN_LOOKUPF);
                UNKNOWN_END_SETF = strdup_str(CG_UNKNOWN_END_SETF);
                INFINITE_BEGIN_SETF = strdup_str(CG_INFINITE_BEGIN_SETF);
                INFINITE_LOOKUPF = strdup_str(CG_INFINITE_LOOKUPF);
                INFINITE_END_SETF = strdup_str(CG_INFINITE_END_SETF);
            }
            LookupOutputFormat::ApertiumOutput => {
                BEGIN_SETF = strdup_str(APERTIUM_BEGIN_SETF);
                LOOKUPF = strdup_str(APERTIUM_LOOKUPF);
                END_SETF = strdup_str(APERTIUM_END_SETF);
                EMPTY_BEGIN_SETF = strdup_str(APERTIUM_EMPTY_BEGIN_SETF);
                EMPTY_LOOKUPF = strdup_str(APERTIUM_EMPTY_LOOKUPF);
                EMPTY_END_SETF = strdup_str(APERTIUM_EMPTY_END_SETF);
                UNKNOWN_BEGIN_SETF = strdup_str(APERTIUM_UNKNOWN_BEGIN_SETF);
                UNKNOWN_LOOKUPF = strdup_str(APERTIUM_UNKNOWN_LOOKUPF);
                UNKNOWN_END_SETF = strdup_str(APERTIUM_UNKNOWN_END_SETF);
                INFINITE_BEGIN_SETF = strdup_str(APERTIUM_INFINITE_BEGIN_SETF);
                INFINITE_LOOKUPF = strdup_str(APERTIUM_INFINITE_LOOKUPF);
                INFINITE_END_SETF = strdup_str(APERTIUM_INFINITE_END_SETF);
            }
        }

        if !LOOKUP_GIVEN {
            LOOKUP_FILE = stdin_file();
            LOOKUP_FILE_NAME = strdup_str("<stdin>");
        }
        check_common_params();
        check_unary_params(argc, argv);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-lookup.print-prompt-fn]
// [spec:hfst:sem:hfst-lookup.print-prompt-fn]
unsafe fn print_prompt() {
    unsafe {
        if !globals::SILENT && !PIPE_INPUT && !LOOKUP_GIVEN {
            eprint!("> ");
        }
    }
}

// [spec:hfst:def:hfst-lookup.lookup-printf-fn]
// [spec:hfst:sem:hfst-lookup.lookup-printf-fn]
unsafe fn lookup_printf(
    format: *const c_char,
    input: Option<&HfstOneLevelPath>,
    result: Option<&HfstOneLevelPath>,
    markup: Option<&str>,
    ofile: *mut libc::FILE,
) -> c_int {
    unsafe {
        let epsilon_format = cstr(EPSILON_FORMAT);
        let space_format = cstr(SPACE_FORMAT);

        // Build the lookupform string (the result side).
        let lookupform: Option<String> = result.map(|r| {
            let mut p = String::new();
            let mut first = true;
            for s in r.second.iter() {
                if !first && PRINT_SPACE {
                    p.push_str(&space_format);
                }
                if is_epsilon(s) {
                    p.push_str(&epsilon_format);
                } else if FdOperation::is_diacritic(s) {
                    if SHOW_FLAGS {
                        p.push_str(s);
                    }
                } else {
                    p.push_str(s);
                }
                first = false;
            }
            p
        });

        // Build the inputform string.
        let inputform: String = match input {
            Some(inp) => {
                let mut p = String::new();
                let mut first = true;
                for s in inp.second.iter() {
                    if !first && PRINT_SPACE {
                        p.push_str(&space_format);
                    }
                    if is_epsilon(s) {
                        p.push_str(&epsilon_format);
                    } else if FdOperation::is_diacritic(s) {
                        if SHOW_FLAGS {
                            p.push_str(s);
                        }
                    } else {
                        p.push_str(s);
                    }
                    first = false;
                }
                p
            }
            None => String::new(),
        };

        // weight
        let w: f32 = match result {
            Some(r) => r.first,
            None => f32::INFINITY,
        };

        // %i, %l, %b, %a, %m substitution sources
        let i = inputform.clone();
        let (l, b, a) = match &lookupform {
            Some(lf) => {
                let l = lf.clone();
                // find the analysis split point (first of '+', ' ', '<', '[')
                let split = lf
                    .find('+')
                    .or_else(|| lf.find(' '))
                    .or_else(|| lf.find('<'))
                    .or_else(|| lf.find('['))
                    .unwrap_or(lf.len());
                let b = lf[..split].to_string();
                let a = lf[split..].to_string();
                (l, b, a)
            }
            None => (String::new(), String::new(), String::new()),
        };
        let m = markup.map(|s| s.to_string()).unwrap_or_default();

        // Walk the format string, substituting %-escapes.
        let format_s = cstr(format);
        let mut res = String::new();
        let mut percent = false;
        for ch in format_s.chars() {
            if percent {
                match ch {
                    'b' => res.push_str(&b),
                    'l' => res.push_str(&l),
                    'i' => res.push_str(&i),
                    'a' => res.push_str(&a),
                    'm' => res.push_str(&m),
                    'n' => res.push('\n'),
                    'w' => {
                        // On non-MSC, the C++ never prints "inf" (the test is
                        // 'if (false)'), always uses %f.
                        res.push_str(&format!("{:.6}", w));
                    }
                    other => {
                        // unknown format, retain % as well
                        res.push('%');
                        res.push(other);
                    }
                }
                percent = false;
            } else if ch == '%' {
                percent = true;
            } else {
                res.push(ch);
            }
        }

        let printed = if !QUOTE_SPECIAL {
            res
        } else {
            get_print_format(&res)
        };
        fput(ofile, &printed);
        printed.len() as c_int
    }
}

// [spec:hfst:def:hfst-lookup.string-to-utf8-fn]
// [spec:hfst:sem:hfst-lookup.string-to-utf8-fn]
unsafe fn string_to_utf8(p: &str) -> Vec<String> {
    unsafe {
        let mut path: Vec<String> = Vec::new();
        let bytes = p.as_bytes();
        let mut idx = 0usize;
        while idx < bytes.len() {
            let c = bytes[idx];
            let u8len: usize = if c <= 127 {
                1
            } else if (c & (128 + 64 + 32 + 16)) == (128 + 64 + 32 + 16) {
                4
            } else if (c & (128 + 64 + 32)) == (128 + 64 + 32) {
                3
            } else if (c & (128 + 64)) == (128 + 64) {
                2
            } else {
                hfst_error_at_line(
                    libc::EXIT_FAILURE,
                    0,
                    &cstr(globals::INPUTFILENAME),
                    LINEN as u32,
                    &format!("{} not valid UTF-8\n", &p[idx..]),
                );
                1
            };
            let end = (idx + u8len).min(bytes.len());
            path.push(String::from_utf8_lossy(&bytes[idx..end]).into_owned());
            idx += u8len;
        }
        path
    }
}

/* Add a '\' in front of ':', ' ' and '\'. */
// [spec:hfst:def:hfst-lookup.escape-special-characters-fn]
// [spec:hfst:sem:hfst-lookup.escape-special-characters-fn]
fn escape_special_characters(s: &str) -> String {
    let mut retval = String::new();
    for ch in s.chars() {
        if ch == ':' || ch == '\\' || ch == ' ' {
            retval.push('\\');
        }
        retval.push(ch);
    }
    retval
}

// [spec:hfst:def:hfst-lookup.line-to-lookup-path-fn]
// [spec:hfst:sem:hfst-lookup.line-to-lookup-path-fn]
unsafe fn line_to_lookup_path(
    s: &mut String,
    tok: &HfstStrings2FstTokenizer,
    markup: &mut String,
    outside_sigma: &mut bool,
    optimized_lookup: bool,
) -> HfstOneLevelPath {
    unsafe {
        let mut rv = HfstOneLevelPath {
            first: 0.0,
            second: Vec::new(),
        };
        *outside_sigma = false;
        INPUTS += 1;
        match INPUT_FORMAT {
            LookupInputFormat::SpaceSeparatedTokenInput => {
                let escaped = escape_special_characters(s);
                let spv: StringPairVector = tok.tokenize_string_pair(&escaped, true);
                for it in spv.iter() {
                    rv.second.push(it.0.clone());
                }
            }
            LookupInputFormat::Utf8TokenInput => {
                if optimized_lookup {
                    rv.second.push(s.clone());
                } else {
                    let escaped = escape_special_characters(s);
                    let spv: StringPairVector = tok.tokenize_string_pair(&escaped, false);
                    for it in spv.iter() {
                        // todo: check if symbol is known to transducer
                        rv.second.push(it.0.clone());
                    }
                }
            }
            LookupInputFormat::ApertiumInput => {
                let mut real_s = String::new();
                let mut m = String::new();
                let mut inbr = false;
                let chars: Vec<char> = s.chars().collect();
                let mut p = 0usize;
                while p < chars.len() {
                    let ch = chars[p];
                    if inbr {
                        if ch == ']' {
                            m.push(ch);
                            inbr = false;
                        } else if ch == '\\' && p + 1 < chars.len() && chars[p + 1] == ']' {
                            p += 1;
                            m.push(chars[p]);
                        } else {
                            m.push(ch);
                        }
                    } else if ch == '[' {
                        m.push(ch);
                        inbr = true;
                    } else if ch == ']' {
                        m.push(ch);
                        p += 1;
                        continue;
                    } else if ch == '\\' {
                        p += 1;
                        if p < chars.len() {
                            real_s.push(chars[p]);
                        }
                    } else {
                        real_s.push(ch);
                    }
                    p += 1;
                }
                let path = string_to_utf8(&real_s);
                *s = real_s;
                *markup = m;
                rv.second = path;
            }
        }
        rv
    }
}

/* Replace all strings str1 in symbol with str2. */
// [spec:hfst:def:hfst-lookup.replace-all-fn]
// [spec:hfst:sem:hfst-lookup.replace-all-fn]
fn replace_all(symbol: String, str1: &str, str2: &str) -> String {
    if str1.is_empty() {
        return symbol;
    }
    symbol.replace(str1, str2)
}

// [spec:hfst:def:hfst-lookup.get-print-format-fn]
// [spec:hfst:sem:hfst-lookup.get-print-format-fn]
unsafe fn get_print_format(s: &str) -> String {
    unsafe {
        if is_epsilon(s) {
            return cstr(EPSILON_FORMAT);
        }
        if QUOTE_SPECIAL {
            return replace_all(
                replace_all(replace_all(s.to_string(), "\\", "\\\\"), ":", "\\:"),
                " ",
                "\\ ",
            );
        }
        s.to_string()
    }
}

// [spec:hfst:def:hfst-lookup.print-lookup-string-fn]
// [spec:hfst:sem:hfst-lookup.print-lookup-string-fn]
unsafe fn print_lookup_string(s: &StringVector) {
    unsafe {
        for it in s.iter() {
            fput(globals::outfile(), &get_print_format(it));
        }
    }
}

// [spec:hfst:def:hfst-lookup.get-lookup-string-fn]
// [spec:hfst:sem:hfst-lookup.get-lookup-string-fn]
unsafe fn get_lookup_string(s: &StringVector) -> String {
    unsafe {
        let mut retval = String::new();
        for it in s.iter() {
            retval += &get_print_format(it);
        }
        retval
    }
}

// [spec:hfst:def:hfst-lookup.is-possible-to-get-result-fn]
// [spec:hfst:sem:hfst-lookup.is-possible-to-get-result-fn]
fn is_possible_to_get_result(
    s: &HfstOneLevelPath,
    symbols_seen: &StringSet,
    unknown_or_identity_seen: bool,
) -> bool {
    if unknown_or_identity_seen {
        return true;
    }
    for it in s.second.iter() {
        if !symbols_seen.contains(it) {
            return false;
        }
    }
    true
}

// [spec:hfst:def:hfst-lookup.lookup-fd-and-print-fn]
// [spec:hfst:sem:hfst-lookup.lookup-fd-and-print-fn]
#[allow(clippy::too_many_arguments)]
unsafe fn lookup_fd_and_print(
    tr: Option<&HfstBasicTransducer>,
    transducer: Option<&HfstTransducer>,
    results: &mut HfstOneLevelPaths,
    s: &HfstOneLevelPath,
    limit: Option<isize>,
    print_pairs_at_this_point: bool,
    print_fail: bool,
    input_to_print: Option<&HfstOneLevelPath>,
    no_newline: bool,
) {
    unsafe {
        // If we want a StringPairVector representation
        let mut results_spv: HfstTwoLevelPaths = HfstTwoLevelPaths::new();

        if let Some(t) = tr {
            if is_possible_to_get_result(
                s,
                &CASCADE_SYMBOLS_SEEN[TRANSDUCER_NUMBER as usize],
                CASCADE_UNKNOWN_OR_IDENTITY_SEEN[TRANSDUCER_NUMBER as usize],
            ) {
                t.lookup(
                    &s.second,
                    &mut results_spv,
                    limit.map(|l| l as usize),
                    // no weight limit, variable 'beam' defines which paths are printed
                    None,
                    -1,
                    OBEY_FLAGS,
                );
            }
        } else if let Some(big_t) = transducer {
            // TODO: is copying slow?
            let mut lookup_str = String::new();
            for it in s.second.iter() {
                lookup_str += it;
            }
            results_spv = big_t.lookup_pairs(&lookup_str, limit.unwrap_or(-1), TIME_CUTOFF);
        }

        if print_pairs_at_this_point && PRINT_PAIRS {
            // No results, print just the lookup string.
            if results_spv.is_empty() {
                if print_fail {
                    let input = get_lookup_string(&s.second);
                    fput(
                        globals::outfile(),
                        &format!("{}\t{}+?\tinf\n\n", input, input),
                    );
                    libc::fflush(globals::outfile());
                }
            } else {
                let mut lowest_weight: f32 = -1.0;
                let mut first = true;
                for it in results_spv.iter() {
                    if first {
                        lowest_weight = it.first;
                    }
                    first = false;
                    if BEAM < 0.0 || it.first <= (lowest_weight + BEAM) {
                        // print the lookup string
                        if let Some(itp) = input_to_print {
                            print_lookup_string(&itp.second);
                        } else {
                            print_lookup_string(&s.second);
                        }
                        fput(globals::outfile(), "\t");
                        // and the path that yielded the result string
                        let mut first_pair = true;
                        for it2 in it.second.iter() {
                            if SHOW_FLAGS || !FdOperation::is_diacritic(&it2.1) {
                                if PRINT_SPACE && !first_pair {
                                    fput(globals::outfile(), " ");
                                }
                                fput(
                                    globals::outfile(),
                                    &format!(
                                        "{}:{}",
                                        get_print_format(&it2.0),
                                        get_print_format(&it2.1)
                                    ),
                                );
                                first_pair = false;
                            }
                        }
                        // and the weight of that path (add the weight of input)
                        fput(
                            globals::outfile(),
                            &format!("\t{:.6}\n", it.first + s.first),
                        );
                    }
                }
                if !no_newline {
                    fput(globals::outfile(), "\n");
                }
            }
            libc::fflush(globals::outfile());
        }

        // Convert HfstTwoLevelPaths into HfstOneLevelPaths
        for it in results_spv.iter() {
            let mut sv: StringVector = Vec::new();
            for spv_it in it.second.iter() {
                sv.push(spv_it.1.clone());
            }
            results.insert(HfstOneLevelPath {
                first: it.first,
                second: sv,
            });
        }
    }
}

// HfstTransducer (optimized-lookup) variant.
// [spec:hfst:def:hfst-lookup.lookup-simple-fn]
// [spec:hfst:sem:hfst-lookup.lookup-simple-fn]
#[allow(clippy::too_many_arguments)]
unsafe fn lookup_simple_ol(
    s: &HfstOneLevelPath,
    t: &HfstTransducer,
    infinity: &mut bool,
    print_pairs_at_this_point: bool,
    print_fail: bool,
    input_to_print: Option<&HfstOneLevelPath>,
    no_newline: bool,
) -> HfstOneLevelPaths {
    unsafe {
        let mut results: HfstOneLevelPaths = HfstOneLevelPaths::new();
        if TIME_CUTOFF == 0.0 && t.is_lookup_infinitely_ambiguous_string_vector(&s.second) {
            let maxnum: isize = if MAX_NUMBER == -1 {
                DEFAULT_MAX_NUMBER
            } else {
                MAX_NUMBER
            };
            if !globals::SILENT {
                if MAX_NUMBER == -1 {
                    hfst_warning(
                        0,
                        0,
                        &format!(
                            "Got infinite results, number of results limited to {}\n\
                             (can be controlled with --max-number=N)",
                            maxnum
                        ),
                    );
                } else {
                    hfst_warning(
                        0,
                        0,
                        &format!(
                            "Got infinite results, number of results limited to {}",
                            maxnum
                        ),
                    );
                }
            }
            if PRINT_PAIRS {
                lookup_fd_and_print(
                    None,
                    Some(t),
                    &mut results,
                    s,
                    Some(maxnum),
                    print_pairs_at_this_point,
                    print_fail,
                    input_to_print,
                    no_newline,
                );
            } else {
                results = t.lookup_fd_string_vector(&s.second, maxnum, TIME_CUTOFF);
            }
            *infinity = true;
        } else if PRINT_PAIRS {
            lookup_fd_and_print(
                None,
                Some(t),
                &mut results,
                s,
                Some(MAX_NUMBER),
                print_pairs_at_this_point,
                print_fail,
                input_to_print,
                no_newline,
            );
        } else {
            results = t.lookup_fd_string_vector(&s.second, MAX_NUMBER, TIME_CUTOFF);
        }

        if results.is_empty() {
            verbose_printf("Got no results\n");
        }
        results
    }
}

// HfstBasicTransducer variant.
#[allow(clippy::too_many_arguments)]
unsafe fn lookup_simple_basic(
    s: &HfstOneLevelPath,
    t: &HfstBasicTransducer,
    infinity: &mut bool,
    print_pairs_at_this_point: bool,
    print_fail: bool,
    input_to_print: Option<&HfstOneLevelPath>,
    no_newline: bool,
) -> HfstOneLevelPaths {
    unsafe {
        let mut results: HfstOneLevelPaths = HfstOneLevelPaths::new();

        let possible = is_possible_to_get_result(
            s,
            &CASCADE_SYMBOLS_SEEN[TRANSDUCER_NUMBER as usize],
            CASCADE_UNKNOWN_OR_IDENTITY_SEEN[TRANSDUCER_NUMBER as usize],
        );

        if possible && TIME_CUTOFF == 0.0 && t.is_lookup_infinitely_ambiguous_path(s, OBEY_FLAGS) {
            if !globals::SILENT && INFINITE_CUTOFF > 0 {
                hfst_warning(
                    0,
                    0,
                    &format!(
                        "Got infinite results, number of cycles limited to {}",
                        INFINITE_CUTOFF
                    ),
                );
            }
            lookup_fd_and_print(
                Some(t),
                None,
                &mut results,
                s,
                Some(INFINITE_CUTOFF as isize),
                print_pairs_at_this_point,
                print_fail,
                input_to_print,
                no_newline,
            );
            *infinity = true;
        } else {
            lookup_fd_and_print(
                Some(t),
                None,
                &mut results,
                s,
                None,
                print_pairs_at_this_point,
                print_fail,
                input_to_print,
                no_newline,
            );
        }

        if results.is_empty() {
            verbose_printf("Got no results\n");
        }
        results
    }
}

// HfstTransducer (optimized-lookup) cascade variant.
unsafe fn lookup_cascading_ol(
    s: &HfstOneLevelPath,
    cascade: &[HfstTransducer],
    infinity: &mut bool,
) -> HfstOneLevelPaths {
    unsafe {
        let mut results: HfstOneLevelPaths = HfstOneLevelPaths::new();

        // go through all transducers in the cascade
        for i in 0..cascade.len() {
            let result: HfstOneLevelPaths;

            if CASCADE == CASCADE_COMPOSITION && i != 0 {
                let mut composed: HfstOneLevelPaths = HfstOneLevelPaths::new();
                // use previous value of 'results' as input to composition
                let prev: Vec<HfstOneLevelPath> = results.iter().cloned().collect();
                for it in prev.iter() {
                    let one_result = lookup_simple_ol(
                        it,
                        &cascade[i],
                        infinity,
                        (i + 1) == cascade.len(),
                        false,
                        Some(s),
                        true,
                    );
                    for inner in one_result.iter() {
                        // add the weights
                        composed.insert(HfstOneLevelPath {
                            first: inner.first + it.first,
                            second: inner.second.clone(),
                        });
                    }
                }
                // zero 'results'
                results = HfstOneLevelPaths::new();

                // cascading composition done
                if ((i + 1) == cascade.len()) && PRINT_PAIRS {
                    if composed.is_empty() {
                        let mut input = String::new();
                        for it in s.second.iter() {
                            input += it;
                        }
                        fput(
                            globals::outfile(),
                            &format!("{}\t{}+?\tinf\n\n", input, input),
                        );
                    } else {
                        fput(globals::outfile(), "\n");
                    }
                    libc::fflush(globals::outfile());
                }
                result = composed;
            } else {
                result = lookup_simple_ol(s, &cascade[i], infinity, false, false, None, false);
            }

            // (C++ tests 'if (infinity)' on the pointer — always true here.)
            verbose_printf(&format!("Inf results @ level {}\n", i));

            for it in result.iter() {
                results.insert(it.clone());
            }

            if CASCADE == CASCADE_PRIORITY_UNION && !results.is_empty() {
                verbose_printf(&format!(
                    "results found @ level {}, skipping rest of transducers (--cascade=priority-union)\n",
                    i
                ));
                break;
            }
        }
        results
    }
}

// [spec:hfst:def:hfst-lookup.lookup-cascading-fn]
// [spec:hfst:sem:hfst-lookup.lookup-cascading-fn]
unsafe fn lookup_cascading_basic(
    s: &HfstOneLevelPath,
    cascade: &[HfstBasicTransducer],
    infinity: &mut bool,
) -> HfstOneLevelPaths {
    unsafe {
        let mut results: HfstOneLevelPaths = HfstOneLevelPaths::new();

        // go through all transducers in the cascade
        for i in 0..cascade.len() {
            TRANSDUCER_NUMBER = i as u32; // needed for lookup_simple

            let result: HfstOneLevelPaths;
            if CASCADE == CASCADE_COMPOSITION && i != 0 {
                let mut composed: HfstOneLevelPaths = HfstOneLevelPaths::new();
                // use previous value of 'results' as input to composition
                let prev: Vec<HfstOneLevelPath> = results.iter().cloned().collect();
                for it in prev.iter() {
                    // if last transducer in cascade, print results if
                    // --print-pairs is requested
                    let one_result = lookup_simple_basic(
                        it,
                        &cascade[i],
                        infinity,
                        (i + 1) == cascade.len(),
                        false,
                        Some(s),
                        true,
                    );
                    for inner in one_result.iter() {
                        // add the weights
                        composed.insert(HfstOneLevelPath {
                            first: inner.first + it.first,
                            second: inner.second.clone(),
                        });
                    }
                }
                // zero 'results'
                results = HfstOneLevelPaths::new();

                // cascading composition done
                if ((i + 1) == cascade.len()) && PRINT_PAIRS {
                    if composed.is_empty() {
                        let mut input = String::new();
                        for it in s.second.iter() {
                            input += it;
                        }
                        fput(
                            globals::outfile(),
                            &format!("{}\t{}+?\tinf\n\n", input, input),
                        );
                    } else {
                        fput(globals::outfile(), "\n");
                    }
                    libc::fflush(globals::outfile());
                }
                result = composed;
            } else {
                result = lookup_simple_basic(
                    s,
                    &cascade[i],
                    infinity,
                    CASCADE != CASCADE_COMPOSITION,
                    false,
                    None,
                    false,
                );
            }

            // (C++ tests 'if (infinity)' on the pointer — always true here.)
            verbose_printf(&format!("Inf results @ level {}\n", i));

            for it in result.iter() {
                results.insert(it.clone());
            }

            if CASCADE == CASCADE_PRIORITY_UNION && !results.is_empty() {
                verbose_printf(&format!(
                    "results found @ level {}, skipping rest of transducers (--cascade=priority-union)\n",
                    i
                ));
                break;
            }
        }
        results
    }
}

// limits kvs with beam
// [spec:hfst:def:hfst-lookup.print-lookups-fn]
// [spec:hfst:sem:hfst-lookup.print-lookups-fn]
unsafe fn print_lookups(
    kvs: &HfstOneLevelPaths,
    kv: &HfstOneLevelPath,
    markup: Option<&str>,
    outside_sigma: bool,
    inf: bool,
    ofile: *mut libc::FILE,
) {
    unsafe {
        let mut lowest_weight: f32 = -1.0;

        if outside_sigma {
            lookup_printf(UNKNOWN_BEGIN_SETF, Some(kv), None, markup, ofile);
            lookup_printf(UNKNOWN_LOOKUPF, Some(kv), None, markup, ofile);
            lookup_printf(UNKNOWN_END_SETF, Some(kv), None, markup, ofile);
            NO_ANALYSES += 1;
        } else if kvs.is_empty() {
            lookup_printf(EMPTY_BEGIN_SETF, Some(kv), None, markup, ofile);
            lookup_printf(EMPTY_LOOKUPF, Some(kv), None, markup, ofile);
            lookup_printf(EMPTY_END_SETF, Some(kv), None, markup, ofile);
            NO_ANALYSES += 1;
        } else if inf {
            ANALYSED += 1;
            lookup_printf(INFINITE_BEGIN_SETF, Some(kv), None, markup, ofile);
            let mut first = true;
            for lkv in kvs.iter() {
                if first {
                    lowest_weight = lkv.first;
                }
                first = false;
                if BEAM < 0.0 || lkv.first <= (lowest_weight + BEAM) {
                    lookup_printf(INFINITE_LOOKUPF, Some(kv), Some(lkv), markup, ofile);
                    ANALYSES += 1;
                }
            }
            lookup_printf(INFINITE_END_SETF, Some(kv), None, markup, ofile);
        } else {
            ANALYSED += 1;
            lookup_printf(BEGIN_SETF, Some(kv), None, markup, ofile);
            let mut first = true;
            for lkv in kvs.iter() {
                if first {
                    lowest_weight = lkv.first;
                }
                first = false;
                if BEAM < 0.0 || lkv.first <= (lowest_weight + BEAM) {
                    lookup_printf(LOOKUPF, Some(kv), Some(lkv), markup, ofile);
                    ANALYSES += 1;
                }
            }
            lookup_printf(END_SETF, Some(kv), None, markup, ofile);
        }
    }
}

unsafe fn perform_lookups_ol(
    origin: &HfstOneLevelPath,
    cascade: &[HfstTransducer],
    unknown: bool,
    infinite: &mut bool,
) -> HfstOneLevelPaths {
    unsafe {
        if !unknown {
            if cascade.len() == 1 {
                lookup_simple_ol(origin, &cascade[0], infinite, true, true, None, false)
            } else {
                lookup_cascading_ol(origin, cascade, infinite)
            }
        } else {
            HfstOneLevelPaths::new()
        }
    }
}

// [spec:hfst:def:hfst-lookup.perform-lookups-fn]
// [spec:hfst:sem:hfst-lookup.perform-lookups-fn]
unsafe fn perform_lookups_basic(
    origin: &HfstOneLevelPath,
    cascade: &[HfstBasicTransducer],
    unknown: bool,
    infinite: &mut bool,
) -> HfstOneLevelPaths {
    unsafe {
        if !unknown {
            if cascade.len() == 1 {
                lookup_simple_basic(origin, &cascade[0], infinite, true, true, None, false)
            } else {
                lookup_cascading_basic(origin, cascade, infinite)
            }
        } else {
            HfstOneLevelPaths::new()
        }
    }
}

unsafe fn process_stream(inputstream: &mut HfstInputStream, outstream: *mut libc::FILE) -> c_int {
    unsafe {
        let mut cascade: Vec<HfstTransducer> = Vec::new();
        let mut cascade_mut: Vec<HfstBasicTransducer> = Vec::new();
        // set to false if non-ol transducer is pushed into the cascade
        let mut only_optimized_lookup = true;

        let mut transducer_n: usize = 0;
        let mut mc_symbols: StringVector = Vec::new();
        let mut id_or_unk_seen = false;
        while inputstream.is_good() {
            transducer_n += 1;
            // [spec:hfst:def:hfst-lookup.trans-fn]
            // [spec:hfst:sem:hfst-lookup.trans-fn]
            let trans = HfstTransducer::new_from_stream(inputstream);
            let type_ = trans.get_type();
            let mut symbols_seen: StringSet = StringSet::new();

            if type_ != ImplementationType::HFST_OL_TYPE
                && type_ != ImplementationType::HFST_OLW_TYPE
            {
                only_optimized_lookup = false;
            }

            let mut inputname = trans.get_name();
            if inputname.is_empty() {
                inputname = cstr(globals::INPUTFILENAME);
            }
            if transducer_n == 1 {
                verbose_printf(&format!("Reading {}...\n", inputname));
            } else {
                verbose_printf(&format!("Reading {}...{}\n", inputname, transducer_n));
            }

            // add multicharacter symbols to mc_symbols
            if type_ == ImplementationType::SFST_TYPE
                || type_ == ImplementationType::TROPICAL_OPENFST_TYPE
                || type_ == ImplementationType::LOG_OPENFST_TYPE
                || type_ == ImplementationType::FOMA_TYPE
            {
                // [spec:hfst:def:hfst-lookup.basic-fn]
                // [spec:hfst:sem:hfst-lookup.basic-fn]
                let basic = trans.get_basic_transducer();
                for it in basic.iter() {
                    for tr_it in it.iter() {
                        let mcs = tr_it.get_input_symbol();
                        symbols_seen.insert(mcs.clone());
                        if mcs == internal_unknown || mcs == internal_identity {
                            id_or_unk_seen = true;
                        }
                        if mcs.chars().count() > 1 {
                            mc_symbols.push(mcs.clone());
                            verbose_printf(&format!("multicharacter symbol: {}\n", mcs));
                        }
                    }
                }
                cascade_mut.push(basic);
                CASCADE_SYMBOLS_SEEN.push(symbols_seen);
                if id_or_unk_seen {
                    CASCADE_UNKNOWN_OR_IDENTITY_SEEN.push(true);
                } else {
                    CASCADE_UNKNOWN_OR_IDENTITY_SEEN.push(false);
                }
            }

            cascade.push(trans);
            id_or_unk_seen = false;
        }

        inputstream.close();

        if !OBEY_FLAGS
            && (inputstream.get_type() == ImplementationType::HFST_OL_TYPE
                || inputstream.get_type() == ImplementationType::HFST_OLW_TYPE)
        {
            hfst_error(
                libc::EXIT_FAILURE,
                0,
                "not obeying flags not supported on optimized lookup transducers",
            );
        }

        // if transducer type is other than optimized_lookup,
        // convert to HfstBasicTransducer
        let mut line: String;

        let input_tokenizer = HfstStrings2FstTokenizer::new(&mc_symbols, &cstr(EPSILON_FORMAT));

        if !only_optimized_lookup && !globals::SILENT {
            hfst_warning(
                0,
                0,
                &format!(
                    "It is not possible to perform fast lookups with {} format automata.\n\
                     Using HFST basic transducer format and performing slow lookups",
                    hfst_strformat(cascade[0].get_type())
                ),
            );
        }

        let mut filesize: i64 = -1;
        if SHOW_PROGRESS_BAR {
            eprint!("Counting file size...\n");
            libc::fseek(LOOKUP_FILE, 0, libc::SEEK_END);
            filesize = libc::ftell(LOOKUP_FILE) as i64;
            eprint!("{}... rewinding\n", filesize);
            libc::rewind(LOOKUP_FILE);
        }
        print_prompt();
        let mut filepos: i64 = libc::ftell(LOOKUP_FILE) as i64;
        loop {
            let mut raw_line: *mut c_char = std::ptr::null_mut();
            let mut llen: libc::size_t = 0;
            if libc::getline(&mut raw_line, &mut llen, LOOKUP_FILE) == -1 {
                libc::free(raw_line as *mut libc::c_void);
                break;
            }
            line = cstr(raw_line);
            libc::free(raw_line as *mut libc::c_void);

            LINEN += 1;

            // strip trailing '\n'/'\r' ('\r' is possible on Windows)
            if let Some(pos) = line.find(['\n', '\r']) {
                line.truncate(pos);
            }
            verbose_printf(&format!("Looking up {}...\n", line));
            filepos = libc::ftell(LOOKUP_FILE) as i64;
            if SHOW_PROGRESS_BAR {
                if filesize != -1 {
                    eprint!("{} / {}...\r", filepos, filesize);
                } else {
                    eprint!("{} / ?...\r", LINEN);
                }
            }

            let mut markup = String::new();
            let mut unknown = false;
            let mut infinite = false;

            let kv = line_to_lookup_path(
                &mut line,
                &input_tokenizer,
                &mut markup,
                &mut unknown,
                only_optimized_lookup,
            );

            if globals::VERBOSE {
                verbose_printf("Tokenized to: ");
                for s in kv.second.iter() {
                    verbose_printf(&format!("{} ", s));
                }
                verbose_printf("\n");
            }

            let kvs = if only_optimized_lookup {
                perform_lookups_ol(&kv, &cascade, unknown, &mut infinite)
            } else {
                perform_lookups_basic(&kv, &cascade_mut, unknown, &mut infinite)
            };

            if !PRINT_PAIRS {
                // printing was already done in function lookup_fd
                let markup_opt = if markup.is_empty() {
                    None
                } else {
                    Some(markup.as_str())
                };
                print_lookups(&kvs, &kv, markup_opt, unknown, infinite, outstream);
                libc::fflush(outstream);
            }

            print_prompt();
        } // while lines in input

        if SHOW_PROGRESS_BAR {
            eprint!("{}/{}... Done\n", filepos, filesize);
        }

        if PRINT_STATISTICS {
            fput(
                outstream,
                &format!(
                    "Strings\tFound\tMissing\tResults\n{}\t{}\t{}\t{}\n",
                    INPUTS, ANALYSED, NO_ANALYSES, ANALYSES
                ),
            );
            fput(
                outstream,
                &format!(
                    "Coverage\tAmbiguity\n{:.6}\t{:.6}\n",
                    ANALYSED as f32 / INPUTS as f32,
                    ANALYSES as f32 / INPUTS as f32
                ),
            );
        }
        libc::EXIT_SUCCESS
    }
}

// [spec:hfst:def:hfst-lookup.main-fn]
// [spec:hfst:sem:hfst-lookup.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> c_int {
    unsafe {
        // initialise strdup'd defaults (the C++ does this at static init time)
        EPSILON_FORMAT = strdup_str("");
        SPACE_FORMAT = strdup_str("");

        hfst_setlocale();

        let c_args: Vec<CString> = std::env::args()
            .map(|a| CString::new(a).unwrap_or_default())
            .collect();
        let mut argv_vec: Vec<*mut c_char> =
            c_args.iter().map(|s| s.as_ptr() as *mut c_char).collect();
        argv_vec.push(std::ptr::null_mut());
        let argc: c_int = c_args.len() as c_int;
        let argv: *mut *mut c_char = argv_vec.as_mut_ptr();
        let argv0 = cstr(*argv);

        hfst_set_program_name(&argv0, "0.6", "HfstLookup");

        let retval = parse_options(argc, argv);
        if retval != EXIT_CONTINUE {
            return retval;
        }

        // close buffers, we use streams
        if globals::INPUTFILE != stdin_file() {
            libc::fclose(globals::INPUTFILE);
        }
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            cstr(globals::INPUTFILENAME),
            cstr(globals::OUTFILENAME)
        ));
        verbose_printf(&format!(
            "Output formats:\n\
             \x20 regular:'{}''{}...''{}',\n\
             \x20 unanalysed:'{}''{}''{}',\n\
             \x20 untokenised:'{}''{}''{}',\n\
             \x20 infinite:'{}''{}''{}\n\
             \x20 epsilon: '{}', space: '{}', flags: {}\n",
            cstr(BEGIN_SETF),
            cstr(LOOKUPF),
            cstr(END_SETF),
            cstr(EMPTY_BEGIN_SETF),
            cstr(EMPTY_LOOKUPF),
            cstr(EMPTY_END_SETF),
            cstr(UNKNOWN_BEGIN_SETF),
            cstr(UNKNOWN_LOOKUPF),
            cstr(UNKNOWN_END_SETF),
            cstr(INFINITE_BEGIN_SETF),
            cstr(INFINITE_LOOKUPF),
            cstr(INFINITE_END_SETF),
            cstr(EPSILON_FORMAT),
            cstr(SPACE_FORMAT),
            SHOW_FLAGS as i32
        ));

        // here starts the buffer handling part
        // (C++ wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // emitting "%s is not a valid transducer file" is not reproduced here.)
        let mut instream = if globals::INPUTFILE != stdin_file() {
            HfstInputStream::new_filename(&cstr(globals::INPUTFILENAME))
        } else {
            HfstInputStream::new()
        };

        process_stream(&mut instream, globals::outfile());

        if globals::OUTFILE != stdout_file() {
            libc::fclose(globals::OUTFILE);
        }
        // (free(inputfilename)/free(outfilename) in C++ are no-ops here.)
        libc::EXIT_SUCCESS
    }
}
