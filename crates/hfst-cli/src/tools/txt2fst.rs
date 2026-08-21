//! Faithful 1:1 port of tools/src/hfst-txt2fst.cc — the transducer text
//! compiling command-line tool.
//!
//! Convert AT&T or prolog format into a binary transducer. Option handling is
//! clap 4 derive through [`crate::cli`].

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    hfst_error, hfst_parse_format_name, hfst_set_program_name, hfst_warning, redirect_converting,
    verbose_print,
};
use crate::hfst_tool_metadata::{hfst_set_formula, hfst_set_name};
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use std::io::BufRead;

/// hfst-txt2fst's command line.
// [spec:hfst:def:hfst-txt2fst.parse-options-fn]
// [spec:hfst:sem:hfst-txt2fst.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Convert AT&T or prolog format into a binary transducer")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Write result using FMT as backend format: foma, openfst-tropical, sfst,
    /// optimized-lookup-weighted, optimized-lookup-unweighted
    #[arg(short = 'f', long = "format", value_name = "FMT")]
    format: Option<String>,

    /// Interpret string EPS as epsilon in att format (default @0@)
    #[arg(
        short = 'e',
        long = "epsilon",
        value_name = "EPS",
        allow_hyphen_values = true
    )]
    epsilon: Option<String>,

    /// Read prolog format instead of att
    #[arg(short = 'p', long = "prolog")]
    prolog: bool,

    /// Use numbers instead of symbol names (parsed and ignored, as upstream)
    #[arg(short = 'n', long = "number")]
    number: bool,

    /// Disjunct transducers
    #[arg(short = 'j', long = "disjunct")]
    disjunct: bool,

    /// Issue a warning if there are epsilon cycles with a negative weight in
    /// the transducer
    #[arg(short = 'C', long = "check-negative-epsilon-cycles")]
    check_negative_epsilon_cycles: bool,

    /// Warning switch: error, no-error, negative-weights, no-negative-weights.
    /// (The C long table spells this '--Wstuff'; the accepted spelling is
    /// kept.)
    #[arg(short = 'W', long = "Wstuff", value_name = "SWITCH")]
    warning: Option<String>,
}

impl Args {
    /// Case 'f': hfst_parse_format_name, which is itself fatal on an
    /// unrecognised name.
    fn output_format(&self, common: &CommonOptions) -> ImplementationType {
        match self.format.as_deref() {
            Some(name) => hfst_parse_format_name(common, name),
            None => ImplementationType::UNSPECIFIED_TYPE,
        }
    }

    /// Case 'W': the four -W switches, fatal on anything else.
    fn warning_switches(&self, common: &CommonOptions) -> (bool, bool) {
        let mut warn_negative_weights = true;
        let mut warnings_are_errors = false;
        if let Some(switch) = self.warning.as_deref() {
            match switch {
                "error" => warnings_are_errors = true,
                "no-error" => warnings_are_errors = false,
                "negative-weights" => warn_negative_weights = true,
                "no-negative-weights" => warn_negative_weights = false,
                other => {
                    hfst_error(
                        common,
                        1,
                        0,
                        &format!("Unrecognised warning switch -W{}", other),
                    );
                }
            }
        }
        (warn_negative_weights, warnings_are_errors)
    }
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        self.io.apply(opts);
    }

    fn validate(&self, opts: &CommonOptions) -> ToolResult {
        // Both rejections happened inside the C getopt loop, before the
        // parameter checks.
        self.output_format(opts);
        self.warning_switches(opts);
        Ok(())
    }
}

/// hfst-txt2fst's resolved tool state (the former tool-specific `static mut`s).
struct Options {
    output_format: ImplementationType,
    read_prolog_format: bool,
    // whether numbers are used instead of symbol names
    #[allow(dead_code)]
    use_numbers: bool, // not used
    // printname for epsilon (None until set; defaults to "@0@")
    epsilonname: Option<String>,
    // check if there are epsilon cycles with a negative weight
    check_negative_epsilon_cycles: bool,
    warn_negative_weights: bool,
    #[allow(dead_code)]
    warnings_are_errors: bool,
    disjunct_multiple_transducers: bool,
}

// Equivalent of the C++ 'feof(inputfile)': no bytes remain on the buffered
// reader (the readers' own EndOfStreamException paths panic_any internally).
fn is_eof(input: &mut dyn BufRead) -> bool {
    match input.fill_buf() {
        Ok(b) => b.is_empty(),
        Err(_) => true,
    }
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
        ImplementationType::SFST_TYPE
        | ImplementationType::TROPICAL_OPENFST_TYPE
        | ImplementationType::FOMA_TYPE
        | ImplementationType::XFSM_TYPE
        | ImplementationType::HFST_OL_TYPE
        | ImplementationType::HFST_OLW_TYPE
        | ImplementationType::THFST_TYPE
        | ImplementationType::HFST2_TYPE
        | ImplementationType::UNSPECIFIED_TYPE
        | ImplementationType::ERROR_TYPE => {
            process_stream_typed::<hfst_openfst::StdVectorFst>(common, options, outstream, input)
        }
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
pub fn run(args: Vec<String>) -> i32 {
    cli::exit_code(execute(args))
}

fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstTxt2Fst");
    let (common, args) = cli::parse::<Args>(common, args)?;

    // The defaulting the C did after the parameter checks, with the same
    // -v traces and in the same order.
    let (warn_negative_weights, warnings_are_errors) = args.warning_switches(&common);
    let mut options = Options {
        output_format: args.output_format(&common),
        read_prolog_format: args.prolog,
        use_numbers: args.number,
        epsilonname: args.epsilon.clone(),
        check_negative_epsilon_cycles: args.check_negative_epsilon_cycles,
        warn_negative_weights,
        warnings_are_errors,
        disjunct_multiple_transducers: args.disjunct,
    };
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
        ImplementationType::THFST_TYPE => {
            verbose_print(&common, "Using thfst (directory) output\n");
        }
        ImplementationType::HFST2_TYPE
        | ImplementationType::UNSPECIFIED_TYPE
        | ImplementationType::ERROR_TYPE => {
            hfst_error(&common, 1, 0, "Unknown format cannot be used as output\n");
            return Err(1);
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
            return Err(1);
        }
        if !options.read_prolog_format {
            hfst_error(
                &common,
                1,
                0,
                "Writing in att format not supported for xfsm transducers,\nuse '--prolog' instead",
            );
            return Err(1);
        }
        if common.input_filename == "<stdin>" {
            hfst_error(
                &common,
                1,
                0,
                "Reading prolog format from standard input not supported for xfsm transducers,\nuse 'hfst-txt2fst [--input|-i] INFILE' instead",
            );
            return Err(1);
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
            return Err(1);
        }
    };
    let mut input = match common.input_reader() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("hfst-txt2fst: cannot open input: {e}");
            return Err(1);
        }
    };
    process_stream(&common, &options, &mut outstream, &mut *input);
    Ok(())
}
