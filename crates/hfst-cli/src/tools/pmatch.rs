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
//!
//! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
//! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
//! `parse_options` and threaded into the processing functions. There are no
//! `static mut` globals and no `unsafe`.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{extend_options_from_env, hfst_set_program_name};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options,
};
use crate::inc::{CaseResult, handle_common_case, handle_error_case, handle_unary_case};
use hfst::pmatch::{PmatchContainer, print_locate_matches};
use hfst::transducer::{INFINITE_WEIGHT, IStream, Weight};
use std::io::{BufRead, Write};

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

/// hfst-pmatch's own options (the former tool-specific `static mut`s).
struct Options {
    blankline_separated: bool,
    count_patterns: VarVal,
    delete_patterns: VarVal,
    extract_patterns: VarVal,
    locate_mode: VarVal,
    print_weights: VarVal,
    mark_patterns: VarVal,
    max_recursion: i32,
    max_context: i32,
    time_cutoff: f64,
    weight_cutoff: Weight,
    profile: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            blankline_separated: true,
            count_patterns: VarVal::NotDefined,
            delete_patterns: VarVal::NotDefined,
            extract_patterns: VarVal::NotDefined,
            locate_mode: VarVal::NotDefined,
            print_weights: VarVal::NotDefined,
            mark_patterns: VarVal::NotDefined,
            max_recursion: -1,
            max_context: -1,
            time_cutoff: 0.0,
            weight_cutoff: INFINITE_WEIGHT,
            profile: false,
        }
    }
}

// The libreadline_getline helper is compiled only under HAVE_READLINE, which is
// not defined in this build; its non-readline-library equivalent is reached via
// hfst_getline in process_input below, so the function body is not reproduced.
// [spec:hfst:def:hfst-pmatch.libreadline-getline-fn]
// [spec:hfst:sem:hfst-pmatch.libreadline-getline-fn]

// [spec:hfst:def:hfst-pmatch.print-usage-fn]
// [spec:hfst:sem:hfst-pmatch.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] TRANSDUCER\nperform matching/lookup on text streams\n\n",
        common.program_name
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
    let _ = writeln!(msg);
}

// [spec:hfst:def:hfst-pmatch.match-and-print-fn]
// [spec:hfst:sem:hfst-pmatch.match-and-print-fn]
fn match_and_print(
    options: &Options,
    container: &mut PmatchContainer,
    outstream: &mut dyn Write,
    input_text: &mut String,
) {
    if !input_text.is_empty() && input_text.as_bytes()[input_text.len() - 1] == b'\n' {
        // Remove final newline
        input_text.pop();
    }
    if !container.is_in_locate_mode() {
        let _ = write!(
            outstream,
            "{}",
            container.do_match(input_text, options.time_cutoff, options.weight_cutoff)
        );
        let _ = writeln!(outstream);
        if options.blankline_separated {
            let _ = writeln!(outstream);
        }
    } else {
        let locations = container.locate(input_text, options.time_cutoff, options.weight_cutoff);
        // bug-for-bug: C tests 'if (print_weights)' on the raw enum, so
        // 'on' (discriminant 0) is false and only off/not_defined are
        // truthy.
        let printed_something = print_locate_matches(
            &locations,
            &mut *outstream,
            (options.print_weights as i32) != 0,
        );
        if printed_something {
            let _ = writeln!(outstream);
        }
    }
}

// [spec:hfst:def:hfst-pmatch.process-input-fn]
// [spec:hfst:sem:hfst-pmatch.process-input-fn]
fn process_input(
    options: &Options,
    container: &mut PmatchContainer,
    outstream: &mut dyn Write,
) -> i32 {
    let mut input_text = String::new();
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    loop {
        // The HAVE_READLINE/isatty branch is compiled out in this build; the
        // active path reads with hfst_getline from stdin. read_until(b'\n')
        // mirrors getline's byte semantics; cstr did a lossy UTF-8 conversion.
        let mut raw_bytes: Vec<u8> = Vec::new();
        let read = input.read_until(b'\n', &mut raw_bytes).unwrap_or_default();
        if read == 0 {
            break;
        }

        let line_str = String::from_utf8_lossy(&raw_bytes).into_owned();
        let line_bytes = line_str.as_bytes();
        if !options.blankline_separated {
            // newline separated
            input_text = line_str.clone();
            match_and_print(options, container, &mut *outstream, &mut input_text);
        } else if line_bytes.is_empty() || line_bytes[0] == b'\n' {
            match_and_print(options, container, &mut *outstream, &mut input_text);
            input_text.clear();
        } else {
            input_text.push_str(&line_str);
        }
    }

    if options.blankline_separated && !input_text.is_empty() {
        match_and_print(options, container, &mut *outstream, &mut input_text);
    }
    if options.count_patterns == VarVal::On {
        let _ = write!(outstream, "\n{}\n", container.get_pattern_count_info());
    }
    if options.profile {
        let _ = write!(outstream, "\n{}\n", container.get_profiling_info());
    }
    0
}

// [spec:hfst:def:hfst-pmatch.parse-options-fn]
// [spec:hfst:sem:hfst-pmatch.parse-options-fn]
//
// Parse argv into the shared + tool options; `Err(code)` is an exit code the
// caller should return (the former EXIT_CONTINUE sentinel is now `Ok`).
fn parse_options(
    mut common: CommonOptions,
    args: &mut Vec<String>,
) -> Result<(CommonOptions, Options), i32> {
    let mut options = Options::default();
    let mut opt = Getopt::new();
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
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        match handle_common_case(&mut common, &opt, c, print_usage) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        match handle_unary_case(&mut common, &opt, c) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        if c == b'n' as i32 {
            options.blankline_separated = false;
        } else if c == b'x' as i32 {
            options.extract_patterns = VarVal::On;
        } else if c == b'l' as i32 {
            options.locate_mode = VarVal::On;
        } else if c == b'w' as i32 {
            options.print_weights = VarVal::On;
        } else if c == b'c' as i32 {
            options.count_patterns = VarVal::On;
        } else if c == b'z' as i32 {
            options.delete_patterns = VarVal::On;
        } else if c == b'm' as i32 {
            options.mark_patterns = VarVal::Off;
        } else if c == b'b' as i32 {
            options.max_context = opt.optarg().trim().parse::<i32>().unwrap_or(0);
            if options.max_context < 0 {
                eprintln!("Invalid argument for --max-context");
                return Err(1);
            }
        } else if c == b'r' as i32 {
            options.max_recursion = opt.optarg().trim().parse::<i32>().unwrap_or(0);
            if options.max_recursion < 0 {
                eprintln!("Invalid argument for --max-recursion");
                return Err(1);
            }
        } else if c == b'W' as i32 {
            options.weight_cutoff = opt.optarg().trim().parse::<f64>().unwrap_or(0.0) as Weight;
            if options.weight_cutoff < 0.0 {
                eprintln!("Invalid argument for --weight-cutoff");
                return Err(1);
            }
            // NOTE: bug-for-bug — the C 'case W' has no 'break', so it
            // falls through into 'case t' (time-cutoff) below.
            options.time_cutoff = opt.optarg().trim().parse::<f64>().unwrap_or(0.0);
            if options.time_cutoff < 0.0 {
                eprintln!("Invalid argument for --time-cutoff");
                return Err(1);
            }
        } else if c == b't' as i32 {
            options.time_cutoff = opt.optarg().trim().parse::<f64>().unwrap_or(0.0);
            if options.time_cutoff < 0.0 {
                eprintln!("Invalid argument for --time-cutoff");
                return Err(1);
            }
        } else if c == b'p' as i32 {
            options.profile = true;
        } else {
            return Err(handle_error_case(&common, &opt, c));
        }
    }
    // no more options, we should now be at the input filename
    if (opt.optind + 1) < args.len() {
        eprintln!("More than one input file given");
        Err(1)
    } else if (opt.optind + 1) == args.len() {
        if !common.input_filename.is_empty() {
            eprintln!("More than one input file given");
            Err(1)
        } else {
            common.input_filename = args[opt.optind].clone();
            // C: inputfile = hfst_fopen(inputfilename, "r"); if it resolves to
            // stdin ("-"), reset the name to "<stdin>". The actual archive is
            // (re)opened in run, so only the "-" detection is kept.
            if common.input_filename == "-" {
                common.input_filename = "<stdin>".to_string();
            }
            Ok((common, options))
        }
    } else if common.input_filename.is_empty() {
        eprintln!("No input file given");
        Err(1)
    } else {
        Ok((common, options))
    }
}

// [spec:hfst:def:hfst-pmatch.main-fn]
// [spec:hfst:sem:hfst-pmatch.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstPmatch");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };
    // HAVE_READLINE: rl_bind_key('\t', rl_insert) to disable tab completion;
    // compiled out in this build.

    let inputfilename = &common.input_filename;
    let mut file = match std::fs::File::open(inputfilename) {
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
    container.set_verbose(common.verbose);
    if options.extract_patterns != VarVal::NotDefined {
        container.set_extract_patterns(options.extract_patterns == VarVal::On);
    }
    if options.locate_mode != VarVal::NotDefined {
        container.set_locate_mode(options.locate_mode == VarVal::On);
    }
    if options.count_patterns != VarVal::NotDefined {
        container.set_count_patterns(options.count_patterns == VarVal::On);
    }
    if options.delete_patterns != VarVal::NotDefined {
        container.set_delete_patterns(options.delete_patterns == VarVal::On);
    }
    if options.mark_patterns != VarVal::NotDefined {
        container.set_mark_patterns(options.mark_patterns == VarVal::On);
    }
    if options.max_context >= 0 {
        container.set_max_context(options.max_context as usize);
    }
    if options.max_recursion >= 0 {
        container.set_max_recursion(options.max_recursion as usize);
    }
    container.set_profile(options.profile);
    // The C passes std::cout as the output stream; the foundation's
    // output_writer() maps OUTFILENAME (defaulting to "<stdout>") to stdout.
    let mut out = match common.output_writer() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("hfst-pmatch: cannot open output: {e}");
            return 1;
        }
    };
    let rv = process_input(&options, &mut container, &mut *out);
    let _ = out.flush();
    rv
}
