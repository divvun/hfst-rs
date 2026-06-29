//! Faithful 1:1 port of tools/src/hfst-shuffle.cc — the transducer shuffle
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments). A BINARY tool:
//! it reads two input streams (firstfile + secondfile) and writes their
//! shuffle.

use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, conversion_type, convert_transducers, error, extend_options_getenv,
    hfst_set_program_name, hfst_strformat, is_input_stream_in_ol_format, print_more_info,
    print_report_bugs, verbose_printf, warning,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    hfst_getopt_binary_long, hfst_getopt_common_long, print_common_binary_program_options,
    print_common_binary_program_parameter_instructions, print_common_program_options,
};
use hfst_cli::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_binary, hfst_set_name_binary};
use hfst_cli::inc::{
    CaseResult, check_binary_params, check_common_params, handle_binary_case, handle_common_case,
    handle_error_case,
};
use std::io::Write;

// [spec:hfst:def:hfst-shuffle.print-usage-fn]
// [spec:hfst:sem:hfst-shuffle.print-usage-fn]
fn print_usage() {
    let mut msg = globals::message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nShuffle two transducers\n\n",
        globals::program_name()
    );
    print_common_program_options(&mut *msg);
    print_common_binary_program_options(&mut *msg);
    let _ = write!(msg, "\n");
    print_common_binary_program_parameter_instructions(&mut *msg);
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "\nExamples:\n  {} -o shuffled.hfst cat.hfst dog.hfst\n\n",
        globals::program_name()
    );
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-shuffle.parse-options-fn]
// [spec:hfst:sem:hfst-shuffle.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_getenv(args);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_binary_long());
            // add tool-specific options here
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: binary
            // cases, then common cases, then the terminal error arm. (The tool
            // defines no options of its own.)
            match handle_binary_case(c) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_common_case(c, print_usage) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            return handle_error_case(c);
        }

        check_binary_params(args);
        check_common_params();
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-shuffle.shuffle-streams-fn]
// [spec:hfst:sem:hfst-shuffle.shuffle-streams-fn]
unsafe fn shuffle_streams(
    firststream: &mut HfstInputStream,
    secondstream: &mut HfstInputStream,
) -> i32 {
    unsafe {
        // there must be at least one transducer in both input streams
        let mut continue_reading = firststream.is_good() && secondstream.is_good();

        let type1 = firststream.get_type();
        let type2 = secondstream.get_type();
        let mut output_type = ImplementationType::UNSPECIFIED_TYPE;
        if type1 != type2 {
            if globals::ALLOW_TRANSDUCER_CONVERSION {
                let ct = conversion_type(type1, type2);
                let mut warnstr = format!(
                    "Transducer type mismatch in {} and {}; ",
                    globals::first_filename(),
                    globals::second_filename()
                );
                if ct == 1 {
                    warnstr.push_str("using former type as output");
                    output_type = type1;
                } else if ct == 2 {
                    warnstr.push_str("using latter type as output");
                    output_type = type2;
                } else if ct == -1 {
                    warnstr
                        .push_str("using former type as output, loss of information is possible");
                    output_type = type1;
                } else {
                    /* should not happen */
                    std::panic::panic_any(String::from(
                        "Error: hfst-shuffle: conversion_type returned an invalid integer",
                    ));
                }
                warning(0, 0, &warnstr);
            } else {
                error(
                    1,
                    0,
                    &format!(
                        "Transducer type mismatch in {} and {}; formats {} and {} are not compatible for shuffle (--do-not-convert was requested)",
                        globals::first_filename(),
                        globals::second_filename(),
                        hfst_strformat(type1),
                        hfst_strformat(type2)
                    ),
                );
            }
        } else {
            output_type = type1;
        }

        let output_named = globals::output_filename() != "<stdout>";
        let mut outstream = if output_named {
            HfstOutputStream::new_filename(&globals::output_filename(), output_type, true)
        } else {
            HfstOutputStream::new(output_type, true)
        };

        let mut first: Option<HfstTransducer> = None;
        let mut second: Option<HfstTransducer> = None;
        let mut transducer_n_first: usize = 0; // transducers read from first stream
        let mut transducer_n_second: usize = 0; // transducers read from second stream
        while continue_reading {
            first = Some(HfstTransducer::new_from_stream(firststream));
            transducer_n_first += 1;
            if secondstream.is_good() {
                second = Some(HfstTransducer::new_from_stream(secondstream));
                transducer_n_second += 1;
            }
            let firstname = hfst_get_name(first.as_ref().unwrap(), &globals::first_filename());
            if second.is_none() {
                // make scan-build happy, this should not happen
                std::panic::panic_any(String::from("Error: second stream has a NULL value."));
            }
            let secondname = hfst_get_name(second.as_ref().unwrap(), &globals::second_filename());
            if transducer_n_first == 1 {
                verbose_printf(&format!("Shuffling {} and {}...\n", firstname, secondname));
            } else {
                verbose_printf(&format!(
                    "Shuffling {} and {}... {}\n",
                    firstname, secondname, transducer_n_first
                ));
            }

            // C: outer try{} catches TransducersAreNotAutomataException; inner
            // try{} catches TransducerTypeMismatchException. The shuffle method
            // panics carrying the concrete exception struct for both; we
            // distinguish them by downcasting the panic payload.
            let shuffle_err = {
                let second_ref = second.as_ref().unwrap();
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    first.as_mut().unwrap().shuffle(second_ref, true);
                }))
                .err()
            };
            if let Some(err) = shuffle_err {
                if err
                    .downcast_ref::<hfst::hfst_exception_defs::TransducersAreNotAutomataException>()
                    .is_some()
                {
                    // outer catch (TransducersAreNotAutomataException)
                    error(
                        1,
                        0,
                        &format!(
                            "Could not shuffle {} and {} [{}]\nat least one of the input arguments is not an automaton",
                            firstname, secondname, transducer_n_first
                        ),
                    );
                } else if err
                    .downcast_ref::<hfst::hfst_exception_defs::TransducerTypeMismatchException>()
                    .is_some()
                {
                    // inner catch (TransducerTypeMismatchException)
                    if globals::ALLOW_TRANSDUCER_CONVERSION {
                        let mut second_t = second.take().unwrap();
                        convert_transducers(first.as_mut().unwrap(), &mut second_t);
                        first.as_mut().unwrap().shuffle(&second_t, true);
                        second = Some(second_t);
                    } else {
                        error(
                            1,
                            0,
                            &format!(
                                "Could not shuffle {} and {} [{}]:\nformats {} and {} are not compatible for shuffling (--do-not-convert was requested)",
                                firstname,
                                secondname,
                                transducer_n_first,
                                hfst_strformat(firststream.get_type()),
                                hfst_strformat(secondstream.get_type())
                            ),
                        );
                    }
                } else {
                    std::panic::resume_unwind(err);
                }
            }

            // C: hfst_set_name(*first, *first, *second, "shuffle"); the dest and
            // first src are the same object, which Rust cannot alias mut+const,
            // so the read side is taken from a copy (name/formula are unchanged
            // by the copy).
            let first_src = first.as_ref().unwrap().clone();
            let second_ref = second.as_ref().unwrap();
            hfst_set_name_binary(first.as_mut().unwrap(), &first_src, second_ref, "shuffle");
            hfst_set_formula_binary(first.as_mut().unwrap(), &first_src, second_ref, "shuffle");
            outstream.redirect(first.as_mut().unwrap());

            continue_reading =
                firststream.is_good() && (secondstream.is_good() || transducer_n_second == 1);

            first = None;
            // delete the transducer of second stream, unless we continue
            // reading the first stream and there is only one transducer in the
            // second stream
            if (continue_reading && secondstream.is_good()) || !continue_reading {
                second = None;
            }
        }

        if firststream.is_good() {
            error(
                1,
                0,
                &format!(
                    "second input '{}' contains fewer transducers than first input '{}'; this is only possible if the second input contains exactly one transducer",
                    globals::second_filename(),
                    globals::first_filename()
                ),
            );
        }

        if secondstream.is_good() {
            error(
                1,
                0,
                &format!(
                    "first input '{}' contains fewer transducers than second input '{}'",
                    globals::first_filename(),
                    globals::second_filename()
                ),
            );
        }

        firststream.close();
        secondstream.close();
        outstream.close();
        0
    }
}

// [spec:hfst:def:hfst-shuffle.main-fn]
// [spec:hfst:sem:hfst-shuffle.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstShuffle");
        let mut retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let first_opened = globals::first_filename() != "<stdin>";
        let second_opened = globals::second_filename() != "<stdin>";
        verbose_printf(&format!(
            "Reading from {} and {}, writing to {}\n",
            globals::first_filename(),
            globals::second_filename(),
            globals::output_filename()
        ));
        // here starts the buffer handling part
        // (the C wraps each ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch
        // arms are not reproduced here.)
        let mut firststream = if first_opened {
            HfstInputStream::new_filename(&globals::first_filename())
        } else {
            HfstInputStream::new()
        };
        let mut secondstream = if second_opened {
            HfstInputStream::new_filename(&globals::second_filename())
        } else {
            HfstInputStream::new()
        };

        if is_input_stream_in_ol_format(&firststream, "hfst-shuffle")
            || is_input_stream_in_ol_format(&secondstream, "hfst-shuffle")
        {
            return 1;
        }

        retval = shuffle_streams(&mut firststream, &mut secondstream);
        retval
    }
}
