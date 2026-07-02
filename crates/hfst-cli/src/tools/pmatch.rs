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

use crate::globals;
use crate::hfst_commandline::{
    EXIT_CONTINUE, extend_options_from_env, hfst_set_program_name, print_more_info,
    print_report_bugs,
};
use crate::hfst_getopt as getopt;
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options,
};
use crate::inc::{CaseResult, handle_common_case, handle_error_case, handle_unary_case};
use hfst::pmatch::{PmatchContainer, print_locate_matches};
use hfst::transducer::{INFINITE_WEIGHT, IStream, Weight};
use std::io::{BufRead, Write};

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
static mut MAX_RECURSION: i32 = -1;
static mut MAX_CONTEXT: i32 = -1;

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
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] TRANSDUCER\nperform matching/lookup on text streams\n\n",
        globals::program_name()
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
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
         \x20 -p  --profile           Produce profiling data\n"
    );
    let _ = write!(msg, "Use standard streams for input and output.\n\n");

    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
    let _ = write!(msg, "\n");
}

// [spec:hfst:def:hfst-pmatch.match-and-print-fn]
// [spec:hfst:sem:hfst-pmatch.match-and-print-fn]
unsafe fn match_and_print(
    container: &mut PmatchContainer,
    outstream: &mut dyn Write,
    input_text: &mut String,
) {
    unsafe {
        if !input_text.is_empty() && input_text.as_bytes()[input_text.len() - 1] == b'\n' {
            // Remove final newline
            input_text.pop();
        }
        if !container.is_in_locate_mode() {
            let _ = write!(
                outstream,
                "{}",
                container.do_match(input_text, TIME_CUTOFF, WEIGHT_CUTOFF)
            );
            let _ = write!(outstream, "\n");
            if BLANKLINE_SEPARATED {
                let _ = write!(outstream, "\n");
            }
        } else {
            let locations = container.locate(input_text, TIME_CUTOFF, WEIGHT_CUTOFF);
            // bug-for-bug: C tests 'if (print_weights)' on the raw enum, so
            // 'on' (discriminant 0) is false and only off/not_defined are
            // truthy.
            let printed_something =
                print_locate_matches(&locations, &mut *outstream, (PRINT_WEIGHTS as i32) != 0);
            if printed_something {
                let _ = write!(outstream, "\n");
            }
        }
    }
}

// [spec:hfst:def:hfst-pmatch.process-input-fn]
// [spec:hfst:sem:hfst-pmatch.process-input-fn]
unsafe fn process_input(container: &mut PmatchContainer, outstream: &mut dyn Write) -> i32 {
    unsafe {
        let mut input_text = String::new();
        let stdin = std::io::stdin();
        let mut input = stdin.lock();
        loop {
            // The HAVE_READLINE/isatty branch is compiled out in this build; the
            // active path reads with hfst_getline from stdin. read_until(b'\n')
            // mirrors getline's byte semantics; cstr did a lossy UTF-8 conversion.
            let mut raw_bytes: Vec<u8> = Vec::new();
            let read = match input.read_until(b'\n', &mut raw_bytes) {
                Ok(n) => n,
                Err(_) => 0,
            };
            if !(read > 0) {
                break;
            }

            let line_str = String::from_utf8_lossy(&raw_bytes).into_owned();
            let line_bytes = line_str.as_bytes();
            if !BLANKLINE_SEPARATED {
                // newline separated
                input_text = line_str.clone();
                match_and_print(container, &mut *outstream, &mut input_text);
            } else if line_bytes.is_empty() || line_bytes[0] == b'\n' {
                match_and_print(container, &mut *outstream, &mut input_text);
                input_text.clear();
            } else {
                input_text.push_str(&line_str);
            }
        }

        if BLANKLINE_SEPARATED && !input_text.is_empty() {
            match_and_print(container, &mut *outstream, &mut input_text);
        }
        if COUNT_PATTERNS == VarVal::On {
            let _ = write!(outstream, "\n{}\n", container.get_pattern_count_info());
        }
        if PROFILE {
            let _ = write!(outstream, "\n{}\n", container.get_profiling_info());
        }
        0
    }
}

// [spec:hfst:def:hfst-pmatch.parse-options-fn]
// [spec:hfst:sem:hfst-pmatch.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_from_env(args);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let names: &[(&'static str, i32, i32)] = &[
                ("newline", 0, b'n' as i32),
                ("extract-patterns", 0, b'x' as i32),
                ("locate", 0, b'l' as i32),
                ("print-weights", 0, b'w' as i32),
                ("count-patterns", 0, b'c' as i32),
                ("delete-patterns", 0, b'z' as i32),
                ("no-mark-patterns", 0, b'm' as i32),
                ("max-context", 1, b'b' as i32),
                ("max-recursion", 1, b'r' as i32),
                ("weight-cutoff", 1, b'W' as i32),
                ("time-cutoff", 1, b't' as i32),
                ("profile", 0, b'p' as i32),
            ];
            for (name, has_arg, val) in names.iter() {
                long_options.push(getopt::GetOpt {
                    name,
                    has_arg: *has_arg,
                    val: *val,
                });
            }
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            match handle_common_case(c, print_usage) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_unary_case(c) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            if c == b'n' as i32 {
                BLANKLINE_SEPARATED = false;
            } else if c == b'x' as i32 {
                EXTRACT_PATTERNS = VarVal::On;
            } else if c == b'l' as i32 {
                LOCATE_MODE = VarVal::On;
            } else if c == b'w' as i32 {
                PRINT_WEIGHTS = VarVal::On;
            } else if c == b'c' as i32 {
                COUNT_PATTERNS = VarVal::On;
            } else if c == b'z' as i32 {
                DELETE_PATTERNS = VarVal::On;
            } else if c == b'm' as i32 {
                MARK_PATTERNS = VarVal::Off;
            } else if c == b'b' as i32 {
                MAX_CONTEXT = getopt::optarg().trim().parse::<i32>().unwrap_or(0);
                if MAX_CONTEXT < 0 {
                    eprint!("Invalid argument for --max-context\n");
                    return 1;
                }
            } else if c == b'r' as i32 {
                MAX_RECURSION = getopt::optarg().trim().parse::<i32>().unwrap_or(0);
                if MAX_RECURSION < 0 {
                    eprint!("Invalid argument for --max-recursion\n");
                    return 1;
                }
            } else if c == b'W' as i32 {
                WEIGHT_CUTOFF = getopt::optarg().trim().parse::<f64>().unwrap_or(0.0) as Weight;
                if WEIGHT_CUTOFF < 0.0 {
                    eprint!("Invalid argument for --weight-cutoff\n");
                    return 1;
                }
                // NOTE: bug-for-bug — the C 'case W' has no 'break', so it
                // falls through into 'case t' (time-cutoff) below.
                TIME_CUTOFF = getopt::optarg().trim().parse::<f64>().unwrap_or(0.0);
                if TIME_CUTOFF < 0.0 {
                    eprint!("Invalid argument for --time-cutoff\n");
                    return 1;
                }
            } else if c == b't' as i32 {
                TIME_CUTOFF = getopt::optarg().trim().parse::<f64>().unwrap_or(0.0);
                if TIME_CUTOFF < 0.0 {
                    eprint!("Invalid argument for --time-cutoff\n");
                    return 1;
                }
            } else if c == b'p' as i32 {
                PROFILE = true;
            } else {
                return handle_error_case(c);
            }
        }
        // no more options, we should now be at the input filename
        if (getopt::OPTIND + 1) < args.len() {
            eprint!("More than one input file given\n");
            1
        } else if (getopt::OPTIND + 1) == args.len() {
            if !globals::input_filename().is_empty() {
                eprint!("More than one input file given\n");
                1
            } else {
                globals::set_input_filename(args[getopt::OPTIND].clone());
                // C: inputfile = hfst_fopen(inputfilename, "r"); if it resolves to
                // stdin ("-"), reset the name to "<stdin>". The actual archive is
                // (re)opened in real_main, so only the "-" detection is kept.
                if globals::input_filename() == "-" {
                    globals::set_input_filename("<stdin>");
                }
                EXIT_CONTINUE
            }
        } else if globals::input_filename().is_empty() {
            eprint!("No input file given\n");
            1
        } else {
            EXIT_CONTINUE
        }
    }
}

// [spec:hfst:def:hfst-pmatch.main-fn]
// [spec:hfst:sem:hfst-pmatch.main-fn]
pub fn run(args: Vec<String>) -> i32 {
    unsafe { real_main(args) }
}

unsafe fn real_main(mut args: Vec<String>) -> i32 {
    unsafe {
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstPmatch");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // HAVE_READLINE: rl_bind_key('\t', rl_insert) to disable tab completion;
        // compiled out in this build.

        let inputfilename = globals::input_filename();
        let mut file = match std::fs::File::open(&inputfilename) {
            Ok(f) => f,
            Err(_) => {
                eprintln!("Could not open file {}", inputfilename);
                return 1;
            }
        };
        // The C wraps the container construction + processing in try/catch on
        // HfstException; if the archive is not a valid weighted optimized-lookup
        // pmatch file the catch arm prints a hint and returns 1. The Rust ctor
        // currently panics rather than throwing, so that catch arm is not
        // reproduced here.
        let mut instream = IStream::new(&mut file as &mut dyn std::io::Read);
        let mut container = match PmatchContainer::new_from_stream(&mut instream) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("hfst-pmatch: {e}");
                return 1;
            }
        };
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
        // The C passes std::cout as the output stream; the foundation's
        // output_writer() maps OUTFILENAME (defaulting to "<stdout>") to stdout.
        let mut out = match globals::output_writer() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("hfst-pmatch: cannot open output: {e}");
                return 1;
            }
        };
        let rv = process_input(&mut container, &mut *out);
        let _ = out.flush();
        rv
    }
}
