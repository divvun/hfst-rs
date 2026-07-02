//! Faithful 1:1 port of tools/src/hfst-txt2fst.cc — the transducer text
//! compiling command-line tool. Drives the hfst-cli foundation (globals,
//! getopt, commandline, program-options, tool-metadata, inc fragments).
//!
//! Convert AT&T or prolog format into a binary transducer.

use crate::globals;
use crate::hfst_commandline::{
    EXIT_CONTINUE, extend_options_from_env, hfst_error, hfst_parse_format_name,
    hfst_set_program_name, hfst_warning, print_more_info, print_report_bugs, verbose_print,
};
use crate::hfst_getopt as getopt;
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options,
};
use crate::hfst_tool_metadata::{hfst_set_formula, hfst_set_name};
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use std::io::{BufRead, Write};

// ---------------------------------------------------------------------------
// Tool-global state. C: file-scope static variables.
// ---------------------------------------------------------------------------

// add tools-specific variables here
static mut OUTPUT_FORMAT: ImplementationType = ImplementationType::UNSPECIFIED_TYPE;
static mut READ_PROLOG_FORMAT: bool = false;
// whether numbers are used instead of symbol names
static mut USE_NUMBERS: bool = false; // not used
// printname for epsilon (None until set; defaults to "@0@")
static mut EPSILONNAME: Option<String> = None;

// check if there are epsilon cycles with a negative weight
static mut CHECK_NEGATIVE_EPSILON_CYCLES: bool = false;
static mut WARN_NEGATIVE_WEIGHTS: bool = true;
static mut WARNINGS_ARE_ERRORS: bool = false;

static mut DISJUNCT_MULTIPLE_TRANSDUCERS: bool = false;

// Equivalent of the C++ 'feof(inputfile)': no bytes remain on the buffered
// reader (the readers' own EndOfStreamException paths panic_any internally).
fn is_eof(input: &mut dyn BufRead) -> bool {
    match input.fill_buf() {
        Ok(b) => b.is_empty(),
        Err(_) => true,
    }
}

// [spec:hfst:def:hfst-txt2fst.print-usage-fn]
// [spec:hfst:sem:hfst-txt2fst.print-usage-fn]
fn print_usage() {
    let mut msg = globals::message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nConvert AT&T or prolog format into a binary transducer\n\n",
        globals::program_name()
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Text and format options:\n  -f, --format=FMT    Write result using FMT as backend format\n  -e, --epsilon=EPS   Interpret string EPS as epsilon in att format\n  -p, --prolog        Read prolog format instead of att\n",
    );
    let _ = write!(
        msg,
        "Other options:\n  -C, --check-negative-epsilon-cycles  Issue a warning if there are epsilon cycles\n                                       with a negative weight in the transducer\n  -j, --disjunct                       Disjunct transducers\n",
    );
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "If OUTFILE or INFILE is missing or -, standard streams will be used.\nIf FMT is not given, OpenFst's tropical format will be used.\nThe possible values for FMT are {{ foma, openfst-tropical, openfst-log,\nsfst, optimized-lookup-weighted, optimized-lookup-unweighted }}.\nIf EPS is not given, @0@ will be used.\n\nSpace in transition symbols must be escaped as '@_SPACE_@' when using\natt format.\n",
    );
    let _ = write!(msg, "\n");
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-txt2fst.parse-options-fn]
// [spec:hfst:sem:hfst-txt2fst.parse-options-fn]
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
                name: "epsilon",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: 'e' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "number",
                has_arg: getopt::NO_ARGUMENT,
                val: 'n' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "format",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: 'f' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "prolog",
                has_arg: getopt::NO_ARGUMENT,
                val: 'p' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "disjunct",
                has_arg: getopt::NO_ARGUMENT,
                val: 'j' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "check-negative-epsilon-cycles",
                has_arg: getopt::NO_ARGUMENT,
                val: 'C' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "Wstuff",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: 'W' as i32,
            });
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own, then the terminal
            // error arm.
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
            // add tool-specific cases here
            match c as u8 as char {
                'e' => {
                    EPSILONNAME = Some(getopt::optarg());
                    continue;
                }
                'j' => {
                    DISJUNCT_MULTIPLE_TRANSDUCERS = true;
                    continue;
                }
                'n' => {
                    USE_NUMBERS = true;
                    continue;
                }
                'p' => {
                    READ_PROLOG_FORMAT = true;
                    continue;
                }
                'f' => {
                    OUTPUT_FORMAT = hfst_parse_format_name(&getopt::optarg());
                    continue;
                }
                'C' => {
                    CHECK_NEGATIVE_EPSILON_CYCLES = true;
                    continue;
                }
                'W' => {
                    let optarg = getopt::optarg();
                    if optarg == "error" {
                        WARNINGS_ARE_ERRORS = true;
                    } else if optarg == "no-error" {
                        WARNINGS_ARE_ERRORS = false;
                    } else if optarg == "negative-weights" {
                        WARN_NEGATIVE_WEIGHTS = true;
                    } else if optarg == "no-negative-weights" {
                        WARN_NEGATIVE_WEIGHTS = false;
                    } else {
                        hfst_error(1, 0, &format!("Unrecognised warning switch -W{}", optarg));
                        return 1;
                    }
                    continue;
                }
                _ => {}
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(args);
        if (*std::ptr::addr_of!(EPSILONNAME)).is_none() {
            *std::ptr::addr_of_mut!(EPSILONNAME) = Some("@0@".to_string());
            verbose_print(&format!(
                "Using default epsilon representation {}\n",
                (*std::ptr::addr_of!(EPSILONNAME))
                    .clone()
                    .unwrap_or_default()
            ));
        }
        if OUTPUT_FORMAT == ImplementationType::UNSPECIFIED_TYPE {
            OUTPUT_FORMAT = ImplementationType::TROPICAL_OPENFST_TYPE;
            verbose_print("Using default output format OpenFst with tropical weight class\n");
        }

        if OUTPUT_FORMAT == ImplementationType::XFSM_TYPE
            && READ_PROLOG_FORMAT
            && CHECK_NEGATIVE_EPSILON_CYCLES
        {
            hfst_error(
                1,
                0,
                "Error: checking negative epsilon cycles not supported when reading in prolog format\nand outputting in xfsm format.\n",
            );
            return 1;
        }

        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-txt2fst.process-stream-fn]
// [spec:hfst:sem:hfst-txt2fst.process-stream-fn]
unsafe fn process_stream(outstream: &mut HfstOutputStream, input: &mut dyn BufRead) -> i32 {
    unsafe {
        let mut transducer_n: usize = 0;
        let mut linecount: u32 = 0;

        let inputfilename = globals::input_filename();
        let epsilonname = (*std::ptr::addr_of!(EPSILONNAME))
            .clone()
            .unwrap_or_default();

        // outstream.open();
        while !is_eof(input) {
            transducer_n += 1;
            if transducer_n < 2 {
                verbose_print("Reading transducer table...\n");
            } else {
                verbose_print(&format!("Reading transducer table {}...\n", transducer_n));
            }
            if READ_PROLOG_FORMAT {
                if OUTPUT_FORMAT == ImplementationType::XFSM_TYPE {
                    // C: catches HfstException around prolog_file_to_xfsm_transducer;
                    // the Rust foundation panics rather than throwing, so the catch
                    // arm is not reproduced here.
                    let mut t = HfstTransducer::prolog_file_to_xfsm_transducer(&inputfilename);
                    if let Err(e) = outstream.redirect(&mut t) {
                        hfst_error(1, 0, &format!("{}", e));
                        return 1;
                    }
                    if let Err(e) = outstream.flush() {
                        hfst_error(1, 0, &format!("{}", e));
                        return 1;
                    }
                    break;
                }

                // C: catches NotValidPrologFormatException; the Rust readers
                // panic_any rather than throw, so the catch arm is not reproduced.
                let fsm =
                    match HfstBasicTransducer::read_in_prolog_format_file(input, &mut linecount) {
                        Ok(v) => v,
                        Err(e) => {
                            hfst_error(1, 0, &format!("{}", e));
                            return 1;
                        }
                    };

                if CHECK_NEGATIVE_EPSILON_CYCLES {
                    verbose_print(
                        "Checking if the transducer has epsilon cycles with a negative weight...\n",
                    );
                    if fsm.has_negative_epsilon_cycles() {
                        if !globals::SILENT {
                            hfst_warning(
                                0,
                                0,
                                "Transducer has epsilon cycles with a negative weight.\n",
                            );
                        }
                    } else {
                        verbose_print("No epsilon cycles with a negative weight detected...\n");
                    }
                }

                let mut t = match HfstTransducer::new_from_basic(&fsm, OUTPUT_FORMAT) {
                    Ok(v) => v,
                    Err(e) => {
                        hfst_error(1, 0, &format!("{}", e));
                        return 1;
                    }
                };
                hfst_set_name(&mut t, &inputfilename, "text");
                hfst_set_formula(&mut t, &inputfilename, "T");
                if let Err(e) = outstream.redirect(&mut t) {
                    hfst_error(1, 0, &format!("{}", e));
                    return 1;
                }
            } else if DISJUNCT_MULTIPLE_TRANSDUCERS {
                let mut transducers: Vec<HfstTransducer> = Vec::new();
                // C: catches NotValidAttFormatException and prints an error; the
                // Rust readers panic_any rather than throw, so the catch arm is
                // not reproduced here.
                while !is_eof(input) {
                    // C: HfstTransducer(inputfile, type, epsilon, warn) — read the
                    // basic graph from the AT&T file then build the typed transducer.
                    let net = match HfstBasicTransducer::read_in_att_format_file(
                        input,
                        &epsilonname,
                        &mut linecount,
                        WARN_NEGATIVE_WEIGHTS,
                    ) {
                        Ok(v) => v,
                        Err(e) => {
                            hfst_error(1, 0, &format!("{}", e));
                            return 1;
                        }
                    };
                    let t = match HfstTransducer::new_from_basic(&net, OUTPUT_FORMAT) {
                        Ok(v) => v,
                        Err(e) => {
                            hfst_error(1, 0, &format!("{}", e));
                            return 1;
                        }
                    };
                    transducers.push(t);
                }
                let mut joined = match HfstTransducer::new_type(OUTPUT_FORMAT) {
                    Ok(v) => v,
                    Err(e) => {
                        hfst_error(1, 0, &format!("{}", e));
                        return 1;
                    }
                };
                for it in transducers.iter() {
                    if let Err(e) = joined.disjunct(it, true) {
                        hfst_error(1, 0, &format!("{}", e));
                        return 1;
                    }
                }
                // joined.remove_epsilons(); // remove epsilons from the unioned
                // transducers
                if let Err(e) = outstream.redirect(&mut joined) {
                    hfst_error(1, 0, &format!("{}", e));
                    return 1;
                }
            } else {
                // C: catches NotValidAttFormatException; the Rust readers panic_any
                // rather than throw, so the catch arm is not reproduced here.
                // C: HfstTransducer(inputfile, type, epsilon, linecount, warn).
                let net = match HfstBasicTransducer::read_in_att_format_file(
                    input,
                    &epsilonname,
                    &mut linecount,
                    WARN_NEGATIVE_WEIGHTS,
                ) {
                    Ok(v) => v,
                    Err(e) => {
                        hfst_error(1, 0, &format!("{}", e));
                        return 1;
                    }
                };
                let mut t = match HfstTransducer::new_from_basic(&net, OUTPUT_FORMAT) {
                    Ok(v) => v,
                    Err(e) => {
                        hfst_error(1, 0, &format!("{}", e));
                        return 1;
                    }
                };
                hfst_set_name(&mut t, &inputfilename, "text");
                hfst_set_formula(&mut t, &inputfilename, "T");
                if CHECK_NEGATIVE_EPSILON_CYCLES {
                    verbose_print(
                        "Checking if the transducer has epsilon cycles with a negative weight...\n",
                    );
                    let fsm = HfstBasicTransducer::new_from_transducer(&t);
                    if fsm.has_negative_epsilon_cycles() {
                        if !globals::SILENT {
                            hfst_warning(
                                0,
                                0,
                                "Transducer has epsilon cycles with a negative weight.\n",
                            );
                        }
                    } else {
                        verbose_print("No epsilon cycles with a negative weight detected...\n");
                    }
                }
                if let Err(e) = outstream.redirect(&mut t) {
                    hfst_error(1, 0, &format!("{}", e));
                    return 1;
                }
            }
        }
        outstream.close();
        0
    }
}

// [spec:hfst:def:hfst-txt2fst.main-fn]
// [spec:hfst:sem:hfst-txt2fst.main-fn]
pub fn run(args: Vec<String>) -> i32 {
    unsafe { real_main(args) }
}

unsafe fn real_main(mut args: Vec<String>) -> i32 {
    unsafe {
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstTxt2Fst");
        let retval = parse_options(&mut args);

        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let output_opened = globals::output_filename() != "<stdout>";
        verbose_print(&format!(
            "Reading from {}, writing to {}\n",
            globals::input_filename(),
            globals::output_filename()
        ));
        match OUTPUT_FORMAT {
            ImplementationType::SFST_TYPE => {
                verbose_print("Using SFST as output handler\n");
            }
            ImplementationType::TROPICAL_OPENFST_TYPE => {
                verbose_print("Using OpenFst's tropical weights as output\n");
            }
            ImplementationType::LOG_OPENFST_TYPE => {
                verbose_print("Using OpenFst's log weight output\n");
            }
            ImplementationType::FOMA_TYPE => {
                verbose_print("Using foma as output handler\n");
            }
            ImplementationType::XFSM_TYPE => {
                verbose_print("Using xfsm as output handler\n");
            }
            ImplementationType::HFST_OL_TYPE => {
                verbose_print("Using optimized lookup output\n");
            }
            ImplementationType::HFST_OLW_TYPE => {
                verbose_print("Using optimized lookup weighted output\n");
            }
            _ => {
                hfst_error(1, 0, "Unknown format cannot be used as output\n");
                return 1;
            }
        }

        if OUTPUT_FORMAT == ImplementationType::XFSM_TYPE {
            if globals::output_filename() == "<stdout>" {
                hfst_error(
                    1,
                    0,
                    "Writing to standard output not supported for xfsm transducers,\nuse 'hfst-txt2fst [--output|-o] OUTFILE' instead",
                );
                return 1;
            }
            if !READ_PROLOG_FORMAT {
                hfst_error(
                    1,
                    0,
                    "Writing in att format not supported for xfsm transducers,\nuse '--prolog' instead",
                );
                return 1;
            }
            if globals::input_filename() == "<stdin>" {
                hfst_error(
                    1,
                    0,
                    "Reading prolog format from standard input not supported for xfsm transducers,\nuse 'hfst-txt2fst [--input|-i] INFILE' instead",
                );
                return 1;
            } else {
            }
        }

        // here starts the buffer handling part
        let mut outstream = match if output_opened {
            HfstOutputStream::new_filename(&globals::output_filename(), OUTPUT_FORMAT, true)
        } else {
            HfstOutputStream::new(OUTPUT_FORMAT, true)
        } {
            Ok(v) => v,
            Err(e) => {
                hfst_error(1, 0, &format!("{}", e));
                return 1;
            }
        };
        let mut input = match globals::input_reader() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("hfst-txt2fst: cannot open input: {e}");
                return 1;
            }
        };
        process_stream(&mut outstream, &mut *input);
        0
    }
}
