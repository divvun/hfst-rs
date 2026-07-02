//! Faithful 1:1 port of tools/src/hfst-affix-guessify.cc — the transducer
//! guesser maker command-line tool. Creates a weighted affix guesser from an
//! automaton. Drives the hfst-cli foundation (globals, getopt, commandline,
//! program-options, tool-metadata, inc fragments).

use crate::globals;
use crate::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_from_env, hfst_set_program_name, hfst_strtoweight,
    is_input_stream_in_ol_format, print_more_info, print_report_bugs, verbose_print,
};
use crate::hfst_getopt as getopt;
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use hfst::guessify_fst::{GuessDirection, affix_guessify};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use std::io::Write;

// add tools-specific variables here
// GuessDirection and the per-transducer affix-guesser construction now live in
// hfst::guessify_fst; this tool keeps only the option-driven globals + the
// stream-driver loop.
static mut DIRECTION: GuessDirection = GuessDirection::GuessSuffix;
static mut WEIGHT: f32 = 1.0f32;
static mut FORMAT: ImplementationType = ImplementationType::TROPICAL_OPENFST_TYPE;

// [spec:hfst:def:hfst-affix-guessify.print-usage-fn]
// [spec:hfst:sem:hfst-affix-guessify.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nCreate weighted affix guesser from automaton\n\n",
        globals::program_name()
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    // (tool-specific options and short descriptions)
    let _ = write!(
        msg,
        "Guesser parameters:\n  -D, --direction=DIR   set direction of guessing\n  -w, --weight=WEIGHT   set weight difference of affix lengths\n\n"
    );
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(
        msg,
        "DIR is either suffix or prefix, or suffix if omitted.\nWEIGHT is a weight of each arc not in the known suffix or prefix being guessed, as parsed with strtod(3), or 1.0 if omitted.\n"
    );
    let _ = write!(msg, "\n");
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-affix-guessify.parse-options-fn]
// [spec:hfst:sem:hfst-affix-guessify.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_from_env(args);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            long_options.push(getopt::GetOpt {
                name: "weight",
                has_arg: 1, // required_argument
                val: 'w' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "direction",
                has_arg: 1, // required_argument
                val: 'D' as i32,
            });
            // add tool-specific options here
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own ('w'/'D'), then the
            // terminal error arm.
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
            match c {
                x if x == 'w' as i32 => {
                    WEIGHT = hfst_strtoweight(&getopt::optarg());
                    continue;
                }
                x if x == 'D' as i32 => {
                    let optarg = getopt::optarg();
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
        check_unary_params(args);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-affix-guessify.process-stream-fn]
// [spec:hfst:sem:hfst-affix-guessify.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream, outstream: &mut HfstOutputStream) -> i32 {
    unsafe {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let trans = match HfstTransducer::new_from_stream(instream) {
                Ok(t) => t,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            };
            // C: inputname = trans->get_name(); if empty, use inputfilename.
            let inputname = if !trans.get_name().is_empty() {
                trans.get_name()
            } else {
                globals::input_filename()
            };
            if transducer_n < 2 {
                verbose_print(&format!("Guessifying {}...\n", inputname));
            } else {
                verbose_print(&format!("Guessifying {}... {}\n", inputname, transducer_n));
            }
            let mut t = match affix_guessify(&trans, DIRECTION, WEIGHT, FORMAT) {
                Ok(t) => t,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            };
            if let Err(e) = outstream.redirect(&mut t) {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        } // good instream
        0
    }
}

// [spec:hfst:def:hfst-affix-guessify.main-fn]
// [spec:hfst:sem:hfst-affix-guessify.main-fn]
pub fn run(args: Vec<String>) -> i32 {
    unsafe { real_main(args) }
}

unsafe fn real_main(mut args: Vec<String>) -> i32 {
    unsafe {
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstAffixGuessify");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let input_opened = globals::input_filename() != "<stdin>";
        let output_opened = globals::output_filename() != "<stdout>";
        verbose_print(&format!(
            "Reading from {}, writing to {}\n",
            globals::input_filename(),
            globals::output_filename()
        ));

        // here starts the buffer handling part
        // (the C wraps the ctor in try/catch on HfstException reporting
        // "%s is not a valid transducer file"; the Rust ctor currently panics on
        // a bad file rather than throwing, so the catch arm is not reproduced.)
        let instream_res = if input_opened {
            HfstInputStream::new_filename(&globals::input_filename())
        } else {
            HfstInputStream::new()
        };
        let mut instream = match instream_res {
            Ok(s) => s,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        };

        let ty = instream.get_type();
        let outstream_res = if output_opened {
            HfstOutputStream::new_filename(&globals::output_filename(), ty, true)
        } else {
            HfstOutputStream::new(ty, true)
        };
        let mut outstream = match outstream_res {
            Ok(s) => s,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        };

        if is_input_stream_in_ol_format(&instream, "hfst-affix-guessify") {
            return 1;
        }

        process_stream(&mut instream, &mut outstream)
    }
}
