//! Faithful 1:1 port of tools/src/hfst-pmatch.cc — the pmatch utility for
//! continuous matching/lookup on text streams. Drives the hfst-cli foundation
//! (globals, getopt, commandline, program-options, inc fragments) and the
//! hfst optimized-lookup PmatchContainer.
//!
//! This is a unary tool (#includes inc/globals-common.h + inc/globals-unary.h),
//! but it does not use the usual unary HfstInputStream/HfstOutputStream pipeline:
//! it reads its single positional argument as the transducer archive filename,
//! opens it as a plain binary stream, builds a hfst_ol::PmatchContainer from it,
//! and then matches the lines of stdin against it, printing to stdout.

use hfst::pmatch::PmatchContainer;
use hfst::transducer::{INFINITE_WEIGHT, IStream, Weight};
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, extend_options_getenv, hfst_fopen, hfst_getline, hfst_set_program_name,
    hfst_setlocale, hfst_strdup, print_more_info, print_report_bugs,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT, hfst_getopt_common_long,
    hfst_getopt_unary_long, print_common_program_options, print_common_unary_program_options,
};
use hfst_cli::inc::{CaseResult, handle_common_case, handle_error_case, handle_unary_case};
use libc::{c_char, c_int};
use std::ffi::{CStr, CString};

unsafe fn cstr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

unsafe fn fput(f: *mut libc::FILE, s: &str) {
    let c = CString::new(s).unwrap_or_default();
    unsafe { libc::fputs(c.as_ptr(), f) };
}

// 'std::cout' as a FILE* (the C tool passes std::cout as the output stream).
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

static mut BLANKLINE_SEPARATED: bool = true;

// [spec:hfst:def:hfst-pmatch.var-val]
// The discriminants match the C++ enum order (on=0, off=1, not_defined=2) so
// the bug-for-bug 'if (print_weights)' truthiness test below stays faithful:
// 'on' is value 0 and therefore false in a C boolean context.
#[derive(Clone, Copy, PartialEq, Eq)]
enum VarVal {
    On = 0,
    Off = 1,
    NotDefined = 2,
}

static mut COUNT_PATTERNS: VarVal = VarVal::NotDefined;
static mut DELETE_PATTERNS: VarVal = VarVal::NotDefined;
static mut EXTRACT_PATTERNS: VarVal = VarVal::NotDefined;
static mut LOCATE_MODE: VarVal = VarVal::NotDefined;
static mut PRINT_WEIGHTS: VarVal = VarVal::NotDefined;
static mut MARK_PATTERNS: VarVal = VarVal::NotDefined;
static mut MAX_RECURSION: c_int = -1;
static mut MAX_CONTEXT: c_int = -1;

static mut TIME_CUTOFF: f64 = 0.0;
static mut WEIGHT_CUTOFF: Weight = INFINITE_WEIGHT;
static mut PROFILE: bool = false;

// The libreadline_getline helper is compiled only under HAVE_READLINE, which is
// not defined in this build; its non-readline-library equivalent is reached via
// hfst_getline in process_input below, so the function body is not reproduced.
// [spec:hfst:def:hfst-pmatch.libreadline-getline-fn]
// [spec:hfst:sem:hfst-pmatch.libreadline-getline-fn]

// [spec:hfst:def:hfst-pmatch.print-usage-fn]
// [spec:hfst:sem:hfst-pmatch.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            globals::message_out(),
            &format!(
                "Usage: {} [OPTIONS...] TRANSDUCER\nperform matching/lookup on text streams\n\n",
                program_name
            ),
        );
        print_common_program_options(globals::message_out());
        print_common_unary_program_options(globals::message_out());
        fput(
            globals::message_out(),
            "Pmatch options:\n\
             \x20 -n  --newline           Newline as input separator (default is blank line)\n\
             \x20 -x  --extract-patterns  Only print tagged parts in output\n\
             \x20 -l  --locate            Only print locations of matches\n\
             \x20 -w  --print-weights     In locate mode, include weights of the matches\n\
             \x20 -c  --count-patterns    Print the total number of matches when done\n\
             \x20     --delete-patterns   Replace matches with opening tags\n\
             \x20     --no-mark-patterns  Don't tag matched patterns\n\
             \x20     --max-context       Upper limit to context length allowed\n\
             \x20     --max-recursion     Upper limit for recursion\n\
             \x20     --weight-cutoff=W   Upper limit for allowed weight\n\
             \x20 -t, --time-cutoff=S     Limit search after having used S seconds per input\n\
             \x20 -p  --profile           Produce profiling data\n",
        );
        fput(
            globals::message_out(),
            "Use standard streams for input and output.\n\n",
        );

        print_report_bugs();
        fput(globals::message_out(), "\n");
        print_more_info();
        fput(globals::message_out(), "\n");
    }
}

// [spec:hfst:def:hfst-pmatch.match-and-print-fn]
// [spec:hfst:sem:hfst-pmatch.match-and-print-fn]
unsafe fn match_and_print(
    container: &mut PmatchContainer,
    outstream: *mut libc::FILE,
    input_text: &mut String,
) {
    unsafe {
        if !input_text.is_empty() && input_text.as_bytes()[input_text.len() - 1] == b'\n' {
            // Remove final newline
            input_text.pop();
        }
        if !container.is_in_locate_mode() {
            fput(
                outstream,
                &container.match_(input_text, TIME_CUTOFF, WEIGHT_CUTOFF),
            );
            fput(outstream, "\n");
            if BLANKLINE_SEPARATED {
                fput(outstream, "\n");
            }
        } else {
            let locations = container.locate(input_text, TIME_CUTOFF, WEIGHT_CUTOFF);
            let mut printed_something = false;
            for it in locations.iter() {
                if it[0].output != "@_NONMATCHING_@" {
                    printed_something = true;
                    fput(
                        outstream,
                        &format!(
                            "{}|{}|{}|{}",
                            it[0].start, it[0].length, it[0].output, it[0].tag
                        ),
                    );
                    // bug-for-bug: C tests 'if (print_weights)' on the raw enum,
                    // so 'on' (discriminant 0) is false and only off/not_defined
                    // are truthy.
                    if (PRINT_WEIGHTS as i32) != 0 {
                        fput(outstream, &format!("|{}", it[0].weight));
                    }
                    fput(outstream, "\n");
                }
            }
            if printed_something {
                fput(outstream, "\n");
            }
        }
    }
}

// [spec:hfst:def:hfst-pmatch.process-input-fn]
// [spec:hfst:sem:hfst-pmatch.process-input-fn]
unsafe fn process_input(container: &mut PmatchContainer, outstream: *mut libc::FILE) -> c_int {
    unsafe {
        let mut input_text = String::new();
        let mut line: *mut c_char = std::ptr::null_mut();
        let mut len: usize = 0;
        loop {
            // The HAVE_READLINE/isatty branch is compiled out in this build; the
            // active path reads with hfst_getline from stdin.
            if !(hfst_getline(&mut line, &mut len, stdin_file()) > 0) {
                break;
            }

            let line_str = cstr(line);
            let line_bytes = line_str.as_bytes();
            if !BLANKLINE_SEPARATED {
                // newline separated
                input_text = line_str.clone();
                match_and_print(container, outstream, &mut input_text);
            } else if line_bytes.is_empty() || line_bytes[0] == b'\n' {
                match_and_print(container, outstream, &mut input_text);
                input_text.clear();
            } else {
                input_text.push_str(&line_str);
            }

            libc::free(line as *mut libc::c_void);
            line = std::ptr::null_mut();
        }

        if BLANKLINE_SEPARATED && !input_text.is_empty() {
            match_and_print(container, outstream, &mut input_text);
        }
        if COUNT_PATTERNS == VarVal::On {
            fput(
                outstream,
                &format!("\n{}\n", container.get_pattern_count_info()),
            );
        }
        if PROFILE {
            fput(
                outstream,
                &format!("\n{}\n", container.get_profiling_info()),
            );
        }
        libc::EXIT_SUCCESS
    }
}

// [spec:hfst:def:hfst-pmatch.parse-options-fn]
// [spec:hfst:sem:hfst-pmatch.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let names: &[(&str, c_int, c_int)] = &[
                ("newline", 0, b'n' as c_int),
                ("extract-patterns", 0, b'x' as c_int),
                ("locate", 0, b'l' as c_int),
                ("print-weights", 0, b'w' as c_int),
                ("count-patterns", 0, b'c' as c_int),
                ("delete-patterns", 0, b'z' as c_int),
                ("no-mark-patterns", 0, b'm' as c_int),
                ("max-context", 1, b'b' as c_int),
                ("max-recursion", 1, b'r' as c_int),
                ("weight-cutoff", 1, b'W' as c_int),
                ("time-cutoff", 1, b't' as c_int),
                ("profile", 0, b'p' as c_int),
            ];
            // Keep the CStrings alive for the lifetime of getopt_long.
            let name_storage: Vec<CString> = names
                .iter()
                .map(|(n, _, _)| CString::new(*n).unwrap())
                .collect();
            for (i, (_, has_arg, val)) in names.iter().enumerate() {
                long_options.push(getopt::Option {
                    name: name_storage[i].as_ptr(),
                    has_arg: *has_arg,
                    flag: std::ptr::null_mut(),
                    val: *val,
                });
            }
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}nxlwcdmb:r:W:t:p",
                HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT
            ))
            .unwrap();
            let mut option_index: c_int = 0;
            // add tool-specific options here
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
            if c == b'n' as c_int {
                BLANKLINE_SEPARATED = false;
            } else if c == b'x' as c_int {
                EXTRACT_PATTERNS = VarVal::On;
            } else if c == b'l' as c_int {
                LOCATE_MODE = VarVal::On;
            } else if c == b'w' as c_int {
                PRINT_WEIGHTS = VarVal::On;
            } else if c == b'c' as c_int {
                COUNT_PATTERNS = VarVal::On;
            } else if c == b'z' as c_int {
                DELETE_PATTERNS = VarVal::On;
            } else if c == b'm' as c_int {
                MARK_PATTERNS = VarVal::Off;
            } else if c == b'b' as c_int {
                MAX_CONTEXT = libc::atoi(getopt::OPTARG);
                if MAX_CONTEXT < 0 {
                    eprint!("Invalid argument for --max-context\n");
                    return libc::EXIT_FAILURE;
                }
            } else if c == b'r' as c_int {
                MAX_RECURSION = libc::atoi(getopt::OPTARG);
                if MAX_RECURSION < 0 {
                    eprint!("Invalid argument for --max-recursion\n");
                    return libc::EXIT_FAILURE;
                }
            } else if c == b'W' as c_int {
                WEIGHT_CUTOFF = libc::atof(getopt::OPTARG) as Weight;
                if WEIGHT_CUTOFF < 0.0 {
                    eprint!("Invalid argument for --weight-cutoff\n");
                    return libc::EXIT_FAILURE;
                }
                // NOTE: bug-for-bug — the C 'case W' has no 'break', so it
                // falls through into 'case t' (time-cutoff) below.
                TIME_CUTOFF = libc::atof(getopt::OPTARG);
                if TIME_CUTOFF < 0.0 {
                    eprint!("Invalid argument for --time-cutoff\n");
                    return libc::EXIT_FAILURE;
                }
            } else if c == b't' as c_int {
                TIME_CUTOFF = libc::atof(getopt::OPTARG);
                if TIME_CUTOFF < 0.0 {
                    eprint!("Invalid argument for --time-cutoff\n");
                    return libc::EXIT_FAILURE;
                }
            } else if c == b'p' as c_int {
                PROFILE = true;
            } else {
                return handle_error_case(c);
            }
        }
        // no more options, we should now be at the input filename
        if (getopt::OPTIND + 1) < argc {
            eprint!("More than one input file given\n");
            libc::EXIT_FAILURE
        } else if (getopt::OPTIND + 1) == argc {
            if !globals::INPUTFILENAME.is_null() {
                eprint!("More than one input file given\n");
                libc::EXIT_FAILURE
            } else {
                globals::INPUTFILENAME = hfst_strdup(*argv.offset(getopt::OPTIND as isize));
                globals::INPUTFILE = hfst_fopen(&cstr(globals::INPUTFILENAME), "r");
                if globals::INPUTFILE == stdin_file() {
                    libc::free(globals::INPUTFILENAME as *mut libc::c_void);
                    let stdin_name = CString::new("<stdin>").unwrap();
                    globals::INPUTFILENAME = hfst_strdup(stdin_name.as_ptr());
                }
                EXIT_CONTINUE
            }
        } else if globals::INPUTFILENAME.is_null() {
            eprint!("No input file given\n");
            libc::EXIT_FAILURE
        } else {
            EXIT_CONTINUE
        }
    }
}

// [spec:hfst:def:hfst-pmatch.main-fn]
// [spec:hfst:sem:hfst-pmatch.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> c_int {
    unsafe {
        // Build a C-style argv (NULL-terminated) from the Rust args; getopt and
        // extend_options_getenv reorder/replace it in place.
        let c_args: Vec<CString> = std::env::args()
            .map(|a| CString::new(a).unwrap_or_default())
            .collect();
        let mut argv_vec: Vec<*mut c_char> =
            c_args.iter().map(|s| s.as_ptr() as *mut c_char).collect();
        argv_vec.push(std::ptr::null_mut());
        let argc: c_int = c_args.len() as c_int;
        let argv: *mut *mut c_char = argv_vec.as_mut_ptr();
        let argv0 = cstr(*argv);

        hfst_set_program_name(&argv0, "0.1", "HfstPmatch");
        hfst_setlocale();
        let retval = parse_options(argc, argv);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // HAVE_READLINE: rl_bind_key('\t', rl_insert) to disable tab completion;
        // compiled out in this build.

        let inputfilename = cstr(globals::INPUTFILENAME);
        let mut file = match std::fs::File::open(&inputfilename) {
            Ok(f) => f,
            Err(_) => {
                eprintln!("Could not open file {}", inputfilename);
                return libc::EXIT_FAILURE;
            }
        };
        // The C wraps the container construction + processing in try/catch on
        // HfstException; if the archive is not a valid weighted optimized-lookup
        // pmatch file the catch arm prints a hint and returns 1. The Rust ctor
        // currently panics rather than throwing, so that catch arm is not
        // reproduced here.
        let mut instream = IStream::new(&mut file as &mut dyn std::io::Read);
        let mut container = PmatchContainer::new_from_stream(&mut instream);
        container.set_verbose(globals::VERBOSE);
        if EXTRACT_PATTERNS != VarVal::NotDefined {
            container.set_extract_patterns(EXTRACT_PATTERNS == VarVal::On);
        }
        if LOCATE_MODE != VarVal::NotDefined {
            container.set_locate_mode(LOCATE_MODE == VarVal::On);
        }
        if COUNT_PATTERNS != VarVal::NotDefined {
            container.set_count_patterns(COUNT_PATTERNS == VarVal::On);
        }
        if DELETE_PATTERNS != VarVal::NotDefined {
            container.set_delete_patterns(DELETE_PATTERNS == VarVal::On);
        }
        if MARK_PATTERNS != VarVal::NotDefined {
            container.set_mark_patterns(MARK_PATTERNS == VarVal::On);
        }
        if MAX_CONTEXT >= 0 {
            container.set_max_context(MAX_CONTEXT as usize);
        }
        if MAX_RECURSION >= 0 {
            container.set_max_recursion(MAX_RECURSION as usize);
        }
        container.set_profile(PROFILE);
        process_input(&mut container, stdout_file())
    }
}
