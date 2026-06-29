//! Faithful 1:1 port of tools/src/hfst-affix-guessify.cc — the transducer
//! guesser maker command-line tool. Creates a weighted affix guesser from an
//! automaton. Drives the hfst-cli foundation (globals, getopt, commandline,
//! program-options, tool-metadata, inc fragments).

use core::ffi::{c_char, c_int};
use hfst::guessify_fst::{GuessDirection, affix_guessify};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_getenv, hfst_set_program_name, hfst_strtoweight,
    is_input_stream_in_ol_format, print_more_info, print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT, hfst_getopt_common_long,
    hfst_getopt_unary_long, print_common_program_options, print_common_unary_program_options,
    print_common_unary_program_parameter_instructions,
};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
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

fn fput(f: &mut dyn std::io::Write, s: &str) {
    let _ = f.write_all(s.as_bytes());
}

// add tools-specific variables here
// GuessDirection and the per-transducer affix-guesser construction now live in
// hfst::guessify_fst; this tool keeps only the option-driven globals + the
// stream-driver loop.
static mut DIRECTION: GuessDirection = GuessDirection::GuessSuffix;
static mut WEIGHT: f32 = 1.0f32;
static mut FORMAT: ImplementationType = ImplementationType::TROPICAL_OPENFST_TYPE;

// [spec:hfst:def:hfst-affix-guessify.print-usage-fn]
// [spec:hfst:sem:hfst-affix-guessify.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = globals::message_writer();
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            &mut *msg,
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\nCreate weighted affix guesser from automaton\n\n",
                program_name
            ),
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        // (tool-specific options and short descriptions)
        fput(
            &mut *msg,
            "Guesser parameters:\n  -D, --direction=DIR   set direction of guessing\n  -w, --weight=WEIGHT   set weight difference of affix lengths\n\n",
        );
        fput(&mut *msg, "\n");
        print_common_unary_program_parameter_instructions(&mut *msg);
        fput(
            &mut *msg,
            "DIR is either suffix or prefix, or suffix if omitted.\nWEIGHT is a weight of each arc not in the known suffix or prefix being guessed, as parsed with strtod(3), or 1.0 if omitted.\n",
        );
        fput(&mut *msg, "\n");
        print_report_bugs();
        fput(&mut *msg, "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-affix-guessify.parse-options-fn]
// [spec:hfst:sem:hfst-affix-guessify.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let weight_name = CString::new("weight").unwrap();
            let direction_name = CString::new("direction").unwrap();
            long_options.push(getopt::Option {
                name: weight_name.as_ptr(),
                has_arg: 1, // required_argument
                flag: std::ptr::null_mut(),
                val: 'w' as c_int,
            });
            long_options.push(getopt::Option {
                name: direction_name.as_ptr(),
                has_arg: 1, // required_argument
                flag: std::ptr::null_mut(),
                val: 'D' as c_int,
            });
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}w:D:",
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

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own ('w'/'D'), then the
            // terminal error arm.
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
            match c {
                x if x == 'w' as c_int => {
                    WEIGHT = hfst_strtoweight(&cstr(getopt::OPTARG));
                    continue;
                }
                x if x == 'D' as c_int => {
                    let optarg = cstr(getopt::OPTARG);
                    if optarg.starts_with("prefix") {
                        DIRECTION = GuessDirection::GuessPrefix;
                    } else if optarg.starts_with("suffix") {
                        DIRECTION = GuessDirection::GuessSuffix;
                    } else {
                        error(
                            1,
                            0,
                            &format!(
                                "Unable to parse guessing direction from {};\nplease use one of 'prefix' or 'suffix'",
                                optarg
                            ),
                        );
                    }
                    continue;
                }
                _ => {}
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(argc, argv);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-affix-guessify.process-stream-fn]
// [spec:hfst:sem:hfst-affix-guessify.process-stream-fn]
unsafe fn process_stream(
    instream: &mut HfstInputStream,
    outstream: &mut HfstOutputStream,
) -> c_int {
    unsafe {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let trans = HfstTransducer::new_from_stream(instream);
            // C: inputname = trans->get_name(); if empty, use inputfilename.
            let inputname = if !trans.get_name().is_empty() {
                trans.get_name()
            } else {
                cstr(globals::INPUTFILENAME)
            };
            if transducer_n < 2 {
                verbose_printf(&format!("Guessifying {}...\n", inputname));
            } else {
                verbose_printf(&format!("Guessifying {}... {}\n", inputname, transducer_n));
            }
            let mut t = affix_guessify(&trans, DIRECTION, WEIGHT, FORMAT);
            outstream.redirect(&mut t);
        } // good instream
        0
    }
}

// [spec:hfst:def:hfst-affix-guessify.main-fn]
// [spec:hfst:sem:hfst-affix-guessify.main-fn]
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

        hfst_set_program_name(&argv0, "0.1", "HfstAffixGuessify");
        let retval = parse_options(argc, argv);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let input_opened = cstr(globals::INPUTFILENAME) != "<stdin>";
        let output_opened = cstr(globals::OUTFILENAME) != "<stdout>";
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            cstr(globals::INPUTFILENAME),
            cstr(globals::OUTFILENAME)
        ));

        // here starts the buffer handling part
        // (the C wraps the ctor in try/catch on HfstException reporting
        // "%s is not a valid transducer file"; the Rust ctor currently panics on
        // a bad file rather than throwing, so the catch arm is not reproduced.)
        let mut instream = if input_opened {
            HfstInputStream::new_filename(&cstr(globals::INPUTFILENAME))
        } else {
            HfstInputStream::new()
        };

        let type_ = instream.get_type();
        let mut outstream = if output_opened {
            HfstOutputStream::new_filename(&cstr(globals::OUTFILENAME), type_, true)
        } else {
            HfstOutputStream::new(type_, true)
        };

        if is_input_stream_in_ol_format(&instream, "hfst-affix-guessify") {
            return 1;
        }

        let retval = process_stream(&mut instream, &mut outstream);
        retval
    }
}
