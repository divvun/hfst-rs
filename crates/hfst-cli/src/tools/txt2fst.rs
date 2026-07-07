//! Faithful 1:1 port of tools/src/hfst-txt2fst.cc — the transducer text
//! compiling command-line tool. Drives the hfst-cli foundation (globals,
//! getopt, commandline, program-options, tool-metadata, inc fragments).
//!
//! Convert AT&T or prolog format into a binary transducer.
//!
//! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
//! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
//! `parse_options` and threaded into the processing functions. There are no
//! `static mut` globals and no `unsafe`.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    extend_options_from_env, hfst_error, hfst_parse_format_name, hfst_set_program_name,
    hfst_warning, redirect_converting, verbose_print,
};
use crate::hfst_getopt::{self as getopt, Getopt};
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

/// hfst-txt2fst's own options (the former tool-specific `static mut`s).
struct Options {
    // add tools-specific variables here
    output_format: ImplementationType,
    read_prolog_format: bool,
    // whether numbers are used instead of symbol names
    use_numbers: bool, // not used
    // printname for epsilon (None until set; defaults to "@0@")
    epsilonname: Option<String>,
    // check if there are epsilon cycles with a negative weight
    check_negative_epsilon_cycles: bool,
    warn_negative_weights: bool,
    warnings_are_errors: bool,
    disjunct_multiple_transducers: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            output_format: ImplementationType::UNSPECIFIED_TYPE,
            read_prolog_format: false,
            use_numbers: false,
            epsilonname: None,
            check_negative_epsilon_cycles: false,
            warn_negative_weights: true,
            warnings_are_errors: false,
            disjunct_multiple_transducers: false,
        }
    }
}

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
fn print_usage(common: &CommonOptions) {
    let mut msg = common.message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nConvert AT&T or prolog format into a binary transducer\n\n",
        common.program_name
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
}

// [spec:hfst:def:hfst-txt2fst.parse-options-fn]
// [spec:hfst:sem:hfst-txt2fst.parse-options-fn]
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
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: common
        // cases, then unary cases, then the tool's own, then the terminal
        // error arm.
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
        // add tool-specific cases here
        match c as u8 as char {
            'e' => {
                options.epsilonname = Some(opt.optarg());
                continue;
            }
            'j' => {
                options.disjunct_multiple_transducers = true;
                continue;
            }
            'n' => {
                options.use_numbers = true;
                continue;
            }
            'p' => {
                options.read_prolog_format = true;
                continue;
            }
            'f' => {
                options.output_format = hfst_parse_format_name(&common, &opt.optarg());
                continue;
            }
            'C' => {
                options.check_negative_epsilon_cycles = true;
                continue;
            }
            'W' => {
                let optarg = opt.optarg();
                if optarg == "error" {
                    options.warnings_are_errors = true;
                } else if optarg == "no-error" {
                    options.warnings_are_errors = false;
                } else if optarg == "negative-weights" {
                    options.warn_negative_weights = true;
                } else if optarg == "no-negative-weights" {
                    options.warn_negative_weights = false;
                } else {
                    hfst_error(
                        &common,
                        1,
                        0,
                        &format!("Unrecognised warning switch -W{}", optarg),
                    );
                    return Err(1);
                }
                continue;
            }
            _ => {}
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    check_common_params(&mut common);
    check_unary_params(&mut common, &opt, args);
    if options.epsilonname.is_none() {
        options.epsilonname = Some("@0@".to_string());
        verbose_print(
            &common,
            &format!(
                "Using default epsilon representation {}\n",
                options.epsilonname.clone().unwrap_or_default()
            ),
        );
    }
    if options.output_format == ImplementationType::UNSPECIFIED_TYPE {
        options.output_format = ImplementationType::TROPICAL_OPENFST_TYPE;
        verbose_print(
            &common,
            "Using default output format OpenFst with tropical weight class\n",
        );
    }

    if options.output_format == ImplementationType::XFSM_TYPE
        && options.read_prolog_format
        && options.check_negative_epsilon_cycles
    {
        hfst_error(
            &common,
            1,
            0,
            "Error: checking negative epsilon cycles not supported when reading in prolog format\nand outputting in xfsm format.\n",
        );
        return Err(1);
    }

    Ok((common, options))
}

// [spec:hfst:def:hfst-txt2fst.process-stream-fn]
// [spec:hfst:sem:hfst-txt2fst.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &Options,
    outstream: &mut HfstOutputStream,
    input: &mut dyn BufRead,
) -> i32 {
    // The parsed --format is matched ONCE into the backend type
    // ([dec:hfst:monomorphic-backends]); optimized-lookup formats build
    // through the basic transducer at tropical and convert at each write,
    // as the C++ HfstTransducer(net, HFST_OL*_TYPE) constructor did.
    match options.output_format {
        ImplementationType::LOG_OPENFST_TYPE => process_stream_typed::<
            hfst::log_weight_transducer::LogFst,
        >(common, options, outstream, input),
        _ => process_stream_typed::<hfst_openfst::StdVectorFst>(common, options, outstream, input),
    }
}

fn process_stream_typed<B: hfst::backend::AlgebraBackend>(
    common: &CommonOptions,
    options: &Options,
    outstream: &mut HfstOutputStream,
    input: &mut dyn BufRead,
) -> i32 {
    let mut transducer_n: usize = 0;
    let mut linecount: u32 = 0;

    let inputfilename = common.input_filename.clone();
    let epsilonname = options.epsilonname.clone().unwrap_or_default();

    // outstream.open();
    while !is_eof(input) {
        transducer_n += 1;
        if transducer_n < 2 {
            verbose_print(common, "Reading transducer table...\n");
        } else {
            verbose_print(
                common,
                &format!("Reading transducer table {}...\n", transducer_n),
            );
        }
        if options.read_prolog_format {
            if options.output_format == ImplementationType::XFSM_TYPE {
                // XFSM output cannot get here in this build: the output
                // stream constructor rejected XFSM_TYPE before the loop.
                // (The C++ arm called prolog_file_to_xfsm_transducer.)
                unreachable!("XFSM_TYPE output stream cannot be created in this build")
            }

            // C: catches NotValidPrologFormatException; the Rust readers
            // panic_any rather than throw, so the catch arm is not reproduced.
            let fsm = match HfstBasicTransducer::read_in_prolog_format_file(input, &mut linecount) {
                Ok(v) => v,
                Err(e) => {
                    hfst_error(common, 1, 0, &format!("{}", e));
                    return 1;
                }
            };

            if options.check_negative_epsilon_cycles {
                verbose_print(
                    common,
                    "Checking if the transducer has epsilon cycles with a negative weight...\n",
                );
                if fsm.has_negative_epsilon_cycles() {
                    if !common.silent {
                        hfst_warning(
                            common,
                            0,
                            0,
                            "Transducer has epsilon cycles with a negative weight.\n",
                        );
                    }
                } else {
                    verbose_print(
                        common,
                        "No epsilon cycles with a negative weight detected...\n",
                    );
                }
            }

            let mut t: HfstTransducer<B> = match HfstTransducer::new_from_basic(&fsm) {
                Ok(v) => v,
                Err(e) => {
                    hfst_error(common, 1, 0, &format!("{}", e));
                    return 1;
                }
            };
            hfst_set_name(&mut t, &inputfilename, "text");
            hfst_set_formula(&mut t, &inputfilename, "T");
            if let Err(e) = redirect_converting(outstream, &mut t) {
                hfst_error(common, 1, 0, &format!("{}", e));
                return 1;
            }
        } else if options.disjunct_multiple_transducers {
            let mut transducers: Vec<HfstTransducer<B>> = Vec::new();
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
                    options.warn_negative_weights,
                ) {
                    Ok(v) => v,
                    Err(e) => {
                        hfst_error(common, 1, 0, &format!("{}", e));
                        return 1;
                    }
                };
                let t: HfstTransducer<B> = match HfstTransducer::new_from_basic(&net) {
                    Ok(v) => v,
                    Err(e) => {
                        hfst_error(common, 1, 0, &format!("{}", e));
                        return 1;
                    }
                };
                transducers.push(t);
            }
            let mut joined: HfstTransducer<B> = HfstTransducer::new();
            for it in transducers.iter() {
                if let Err(e) = joined.disjunct(it, true) {
                    hfst_error(common, 1, 0, &format!("{}", e));
                    return 1;
                }
            }
            // joined.remove_epsilons(); // remove epsilons from the unioned
            // transducers
            if let Err(e) = redirect_converting(outstream, &mut joined) {
                hfst_error(common, 1, 0, &format!("{}", e));
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
                options.warn_negative_weights,
            ) {
                Ok(v) => v,
                Err(e) => {
                    hfst_error(common, 1, 0, &format!("{}", e));
                    return 1;
                }
            };
            let mut t: HfstTransducer<B> = match HfstTransducer::new_from_basic(&net) {
                Ok(v) => v,
                Err(e) => {
                    hfst_error(common, 1, 0, &format!("{}", e));
                    return 1;
                }
            };
            hfst_set_name(&mut t, &inputfilename, "text");
            hfst_set_formula(&mut t, &inputfilename, "T");
            if options.check_negative_epsilon_cycles {
                verbose_print(
                    common,
                    "Checking if the transducer has epsilon cycles with a negative weight...\n",
                );
                let fsm = HfstBasicTransducer::new_from_transducer(&t);
                if fsm.has_negative_epsilon_cycles() {
                    if !common.silent {
                        hfst_warning(
                            common,
                            0,
                            0,
                            "Transducer has epsilon cycles with a negative weight.\n",
                        );
                    }
                } else {
                    verbose_print(
                        common,
                        "No epsilon cycles with a negative weight detected...\n",
                    );
                }
            }
            if let Err(e) = redirect_converting(outstream, &mut t) {
                hfst_error(common, 1, 0, &format!("{}", e));
                return 1;
            }
        }
    }
    outstream.close();
    0
}

// [spec:hfst:def:hfst-txt2fst.main-fn]
// [spec:hfst:sem:hfst-txt2fst.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstTxt2Fst");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    // close buffers, we use streams
    let output_opened = common.output_filename != "<stdout>";
    verbose_print(
        &common,
        &format!(
            "Reading from {}, writing to {}\n",
            common.input_filename, common.output_filename
        ),
    );
    match options.output_format {
        ImplementationType::SFST_TYPE => {
            verbose_print(&common, "Using SFST as output handler\n");
        }
        ImplementationType::TROPICAL_OPENFST_TYPE => {
            verbose_print(&common, "Using OpenFst's tropical weights as output\n");
        }
        ImplementationType::LOG_OPENFST_TYPE => {
            verbose_print(&common, "Using OpenFst's log weight output\n");
        }
        ImplementationType::FOMA_TYPE => {
            verbose_print(&common, "Using foma as output handler\n");
        }
        ImplementationType::XFSM_TYPE => {
            verbose_print(&common, "Using xfsm as output handler\n");
        }
        ImplementationType::HFST_OL_TYPE => {
            verbose_print(&common, "Using optimized lookup output\n");
        }
        ImplementationType::HFST_OLW_TYPE => {
            verbose_print(&common, "Using optimized lookup weighted output\n");
        }
        _ => {
            hfst_error(&common, 1, 0, "Unknown format cannot be used as output\n");
            return 1;
        }
    }

    if options.output_format == ImplementationType::XFSM_TYPE {
        if common.output_filename == "<stdout>" {
            hfst_error(
                &common,
                1,
                0,
                "Writing to standard output not supported for xfsm transducers,\nuse 'hfst-txt2fst [--output|-o] OUTFILE' instead",
            );
            return 1;
        }
        if !options.read_prolog_format {
            hfst_error(
                &common,
                1,
                0,
                "Writing in att format not supported for xfsm transducers,\nuse '--prolog' instead",
            );
            return 1;
        }
        if common.input_filename == "<stdin>" {
            hfst_error(
                &common,
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
        HfstOutputStream::new_filename(&common.output_filename, options.output_format, true)
    } else {
        HfstOutputStream::new(options.output_format, true)
    } {
        Ok(v) => v,
        Err(e) => {
            hfst_error(&common, 1, 0, &format!("{}", e));
            return 1;
        }
    };
    let mut input = match common.input_reader() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("hfst-txt2fst: cannot open input: {e}");
            return 1;
        }
    };
    process_stream(&common, &options, &mut outstream, &mut *input);
    0
}
