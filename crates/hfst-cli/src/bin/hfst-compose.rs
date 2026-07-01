//! Faithful 1:1 port of tools/src/hfst-compose.cc — the transducer composition
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments). A binary tool:
//! it reads two input streams (firstfile + secondfile) and composes them.

use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::{EngineConfig, HfstTransducer};
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

static mut HARMONIZE_FLAGS: bool = false;
static mut HARMONIZE: bool = true;
// '--xfst flag-is-epsilon' (was the 'flag_is_epsilon_in_composition' file-static
// global in the library; now threaded into compose via EngineConfig).
static mut FLAG_IS_EPSILON: bool = false;
// '--xerox-composition' (was the 'xerox_composition' file-static global in the
// library; now threaded into compose via EngineConfig).
static mut XEROX_COMPOSITION: bool = false;

// [spec:hfst:def:hfst-compose.print-usage-fn]
// [spec:hfst:sem:hfst-compose.print-usage-fn]
fn print_usage() {
    let mut msg = globals::message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let program_name = globals::program_name();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nCompose two transducers\n\n",
        program_name
    );
    print_common_program_options(&mut *msg);
    print_common_binary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Composition options:\n  -x, --xerox-composition=VALUE Whether flag diacritics are treated as ordinary\n                                symbols in composition (default is false).\n  -X, --xfst=VARIABLE    Toggle xfst compatibility option VARIABLE.\nHarmonization:\n  -H, --do-not-harmonize Do not harmonize symbols.\n  -F, --harmonize-flags  Harmonize flag diacritics.\n"
    );
    let _ = write!(msg, "\n");
    print_common_binary_program_parameter_instructions(&mut *msg);
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "Xfst variables are {{flag-is-epsilon (default OFF)}}.\n"
    );
    let _ = write!(
        msg,
        "VALUE can be one of the following: [true|false], [yes|no] or [ON|OFF],\n"
    );
    let _ = write!(msg, "false being the default.\n");
    let _ = write!(
        msg,
        "\nExamples:\n  {} -o cat2dog.hfst cat2mouse.hfst mouse2dog.hfst  composes two automata\n\n",
        program_name
    );
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-compose.parse-options-fn]
// [spec:hfst:sem:hfst-compose.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_getenv(args);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_binary_long());
            // add tool-specific options here
            long_options.push(getopt::GetOpt {
                name: "harmonize-flags",
                has_arg: 0,
                val: b'F' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "do-not-harmonize",
                has_arg: 0,
                val: b'H' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "xerox-composition",
                has_arg: 1,
                val: b'x' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "xfst",
                has_arg: 1,
                val: b'X' as i32,
            });
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: binary
            // cases, then common cases, then the tool's own, then the terminal
            // error arm.
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
            if c == b'F' as i32 {
                HARMONIZE_FLAGS = true;
                continue;
            } else if c == b'H' as i32 {
                HARMONIZE = false;
                continue;
            } else if c == b'x' as i32 {
                let argument = getopt::optarg();
                if argument == "yes" || argument == "true" || argument == "ON" {
                    XEROX_COMPOSITION = true;
                } else if argument == "no" || argument == "false" || argument == "OFF" {
                    XEROX_COMPOSITION = false;
                } else {
                    let _ = write!(
                        std::io::stderr(),
                        "Error: unknown option to --xerox-composition: '{}'\n",
                        getopt::optarg()
                    );
                    return 1;
                }
                continue;
            } else if c == b'X' as i32 {
                let argument = getopt::optarg();
                if argument == "flag-is-epsilon" {
                    FLAG_IS_EPSILON = true;
                } else {
                    let _ = write!(
                        std::io::stderr(),
                        "Error: unknown option to --xfst: '{}'\n",
                        getopt::optarg()
                    );
                    return 1;
                }
                continue;
            }
            return handle_error_case(c);
        }

        check_binary_params(args);
        check_common_params();
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-compose.compose-streams-fn]
// [spec:hfst:sem:hfst-compose.compose-streams-fn]
unsafe fn compose_streams(
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
                } else
                /* should not happen */
                {
                    std::panic::panic_any(
                        "Error: hfst-compose: conversion_type returned an invalid integer",
                    );
                }
                warning(0, 0, &warnstr);
            } else {
                error(
                    1,
                    0,
                    &format!(
                        "Transducer type mismatch in {} and {}; formats {} and {} are not compatible for composition (--do-not-convert was requested)",
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

        let output_opened = globals::output_filename() != "<stdout>";
        let mut outstream = match if output_opened {
            HfstOutputStream::new_filename(&globals::output_filename(), output_type, true)
        } else {
            HfstOutputStream::new(output_type, true)
        } {
            Ok(v) => v,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        };

        let mut first: Option<HfstTransducer> = None;
        let mut second: Option<HfstTransducer> = None;
        let mut transducer_n_first: usize = 0; // transducers read from first stream
        let mut transducer_n_second: usize = 0; // transducers read from second stream
        while continue_reading {
            if firststream.is_good() {
                first = Some(match HfstTransducer::new_from_stream(firststream) {
                    Ok(v) => v,
                    Err(e) => {
                        error(1, 0, &format!("{e}"));
                        return 1;
                    }
                });
                transducer_n_first += 1;
            }
            if secondstream.is_good() {
                second = Some(match HfstTransducer::new_from_stream(secondstream) {
                    Ok(v) => v,
                    Err(e) => {
                        error(1, 0, &format!("{e}"));
                        return 1;
                    }
                });
                transducer_n_second += 1;
            }
            let firstname = hfst_get_name(first.as_ref().unwrap(), &globals::first_filename());
            if second.is_none() {
                // make scan-build happy, this should not happen
                std::panic::panic_any("Error: second stream has a NULL value.");
            }
            let secondname = hfst_get_name(second.as_ref().unwrap(), &globals::second_filename());
            if transducer_n_first == 1 {
                verbose_printf(&format!("Composing {} and {}...\n", firstname, secondname));
            } else {
                verbose_printf(&format!(
                    "Composing {} and {}... {}\n",
                    firstname, secondname, transducer_n_first
                ));
            }

            let has_flags = first.as_ref().unwrap().has_flag_diacritics()
                || second.as_ref().unwrap().has_flag_diacritics();
            if has_flags {
                if !HARMONIZE_FLAGS {
                    if !globals::SILENT {
                        warning(
                            0,
                            0,
                            "At least one of the arguments contains flag diacritics. Use -F to harmonize them.",
                        );
                    }
                } else {
                    let harmonize_res = first
                        .as_mut()
                        .expect("first transducer is present")
                        .harmonize_flag_diacritics(
                            second.as_mut().expect("second transducer is present"),
                            true,
                        );
                    if let Err(e) = harmonize_res {
                        if matches!(e.kind, hfst::error::ErrorKind::TransducerTypeMismatch) {
                            if globals::ALLOW_TRANSDUCER_CONVERSION {
                                convert_transducers(
                                    first.as_mut().expect("first transducer is present"),
                                    second.as_mut().expect("second transducer is present"),
                                );
                                if let Err(e2) = first
                                    .as_mut()
                                    .expect("first transducer is present")
                                    .harmonize_flag_diacritics(
                                        second.as_mut().expect("second transducer is present"),
                                        true,
                                    )
                                {
                                    error(1, 0, &format!("{e2}"));
                                    return 1;
                                }
                            } else {
                                error(
                                    1,
                                    0,
                                    &format!(
                                        "Could not compose {} and {} [{}]:\nformats {} and {} are not compatible for composition (--do-not-convert was requested)",
                                        firstname,
                                        secondname,
                                        transducer_n_first,
                                        hfst_strformat(firststream.get_type()),
                                        hfst_strformat(secondstream.get_type())
                                    ),
                                );
                                return 1;
                            }
                        } else {
                            error(1, 0, &format!("{e}"));
                            return 1;
                        }
                    }
                }
            }

            let cfg = EngineConfig {
                flag_is_epsilon_in_composition: FLAG_IS_EPSILON,
                xerox_composition: XEROX_COMPOSITION,
                ..EngineConfig::default()
            };
            let compose_res = first
                .as_mut()
                .expect("first transducer is present")
                .compose_with_config(
                    second.as_ref().expect("second transducer is present"),
                    HARMONIZE,
                    &cfg,
                )
                .map(|_| ());
            if let Err(e) = compose_res {
                if matches!(e.kind, hfst::error::ErrorKind::TransducerTypeMismatch) {
                    if globals::ALLOW_TRANSDUCER_CONVERSION {
                        convert_transducers(
                            first.as_mut().expect("first transducer is present"),
                            second.as_mut().expect("second transducer is present"),
                        );
                        if let Err(e2) = first
                            .as_mut()
                            .expect("first transducer is present")
                            .compose_with_config(
                                second.as_ref().expect("second transducer is present"),
                                HARMONIZE,
                                &cfg,
                            )
                        {
                            error(1, 0, &format!("{e2}"));
                            return 1;
                        }
                    } else {
                        error(
                            1,
                            0,
                            &format!(
                                "Could not compose {} and {} [{}]:\nformats {} and {} are not compatible for composition (--do-not-convert was requested)",
                                firstname,
                                secondname,
                                transducer_n_first,
                                hfst_strformat(firststream.get_type()),
                                hfst_strformat(secondstream.get_type())
                            ),
                        );
                        return 1;
                    }
                } else {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            }

            // C: hfst_set_name(*first, *first, *second, "compose"); the dest and
            // lhs are the same object, which Rust cannot alias mut+const, so the
            // read side is taken from a copy (name/formula unchanged by copy).
            let first_copy = first.as_ref().unwrap().clone();
            let second_ref = second.as_ref().unwrap();
            hfst_set_name_binary(first.as_mut().unwrap(), &first_copy, second_ref, "compose");
            let second_ref = second.as_ref().unwrap();
            hfst_set_formula_binary(first.as_mut().unwrap(), &first_copy, second_ref, "\u{2218}");

            if let Err(e) = outstream.redirect(first.as_mut().expect("first transducer is present"))
            {
                error(1, 0, &format!("{e}"));
                return 1;
            }

            continue_reading = (firststream.is_good() && secondstream.is_good())
                || (firststream.is_good() && (transducer_n_second == 1))
                || ((transducer_n_first == 1) && secondstream.is_good());

            if !continue_reading {
                first = None;
                second = None;
            } else {
                if firststream.is_good() {
                    first = None;
                }
                if secondstream.is_good() {
                    second = None;
                }
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
                    "first input '{}' contains fewer transducers than second input '{}'; this is only possible if the first input contains exactly one transducer",
                    globals::first_filename(),
                    globals::second_filename()
                ),
            );
        }

        firststream.close();
        secondstream.close();
        if let Err(e) = outstream.flush() {
            error(1, 0, &format!("{e}"));
            return 1;
        }
        outstream.close();

        0
    }
}

// [spec:hfst:def:hfst-compose.main-fn]
// [spec:hfst:sem:hfst-compose.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstCompose");
        let retval = parse_options(&mut args);
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
        // (the C wraps the ctors in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)
        let mut firststream = match if first_opened {
            HfstInputStream::new_filename(&globals::first_filename())
        } else {
            HfstInputStream::new()
        } {
            Ok(v) => v,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        };
        let mut secondstream = match if second_opened {
            HfstInputStream::new_filename(&globals::second_filename())
        } else {
            HfstInputStream::new()
        } {
            Ok(v) => v,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        };

        if is_input_stream_in_ol_format(&firststream, "hfst-compose")
            || is_input_stream_in_ol_format(&secondstream, "hfst-compose")
        {
            return 1;
        }

        compose_streams(&mut firststream, &mut secondstream)
    }
}
