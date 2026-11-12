//! Faithful 1:1 port of tools/src/hfst-txt2fst.cc — the transducer text
//! compiling command-line tool.
//!
//! Convert AT&T or prolog format into a binary transducer. Option handling is
//! clap 4 derive through [`crate::cli`].

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    hfst_error, hfst_parse_format_name, hfst_set_program_name, hfst_warning, verbose_print,
};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::tools::txt2fst::{TextEvent, Txt2FstOptions, read_text};
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

// [spec:hfst:def:hfst-txt2fst.process-stream-fn]
// [spec:hfst:sem:hfst-txt2fst.process-stream-fn]
// [spec:hfst:req:embed.tools-in-library]
// Every --format reads at tropical and converts at each write
// ([dec:hfst:monomorphic-backends]); the reading is the library's
// read_text, and the tool writes and prints what it is handed.
fn process_stream(
    common: &CommonOptions,
    options: &Options,
    outstream: &mut HfstOutputStream,
    input: &mut dyn BufRead,
) -> i32 {
    let text_options = Txt2FstOptions {
        prolog: options.read_prolog_format,
        epsilon: options.epsilonname.clone().unwrap_or_default(),
        check_negative_epsilon_cycles: options.check_negative_epsilon_cycles,
        warn_negative_weights: options.warn_negative_weights,
        disjunct: options.disjunct_multiple_transducers,
    };
    let mut each = |event: TextEvent<hfst_openfst::StdVectorFst>| {
        match event {
            TextEvent::Progress(message) => verbose_print(common, &message),
            TextEvent::NegativeWeight { weight, .. } => {
                tracing::warn!("Negative weight {:.6} found :-(", weight);
            }
            TextEvent::NegativeEpsilonCycles => {
                if !common.silent {
                    hfst_warning(
                        common,
                        0,
                        0,
                        "Transducer has epsilon cycles with a negative weight.\n",
                    );
                }
            }
            TextEvent::Read(t) => outstream.write_converted(&t)?,
        }
        Ok(())
    };
    if let Err(e) = read_text(input, &common.input_filename, &text_options, &mut each) {
        hfst_error(common, 1, 0, &format!("{}", e));
        return 1;
    }
    outstream.close();
    0
}

// [spec:hfst:def:hfst-txt2fst.main-fn]
// [spec:hfst:sem:hfst-txt2fst.main-fn]
pub(super) fn execute(args: Vec<String>) -> ToolResult {
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
            hfst_error(&common, 1, 0, "xfsm back-end is not available\n");
            return Err(1);
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

#[cfg(test)]
mod tests {
    use super::{Options, process_stream};
    use hfst::file_resolver::{FileResolver, MemoryFiles};
    use hfst::hfst_data_types::ImplementationType;
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;

    fn options(prolog: bool, disjunct: bool) -> Options {
        Options {
            output_format: ImplementationType::TROPICAL_OPENFST_TYPE,
            read_prolog_format: prolog,
            use_numbers: false,
            epsilonname: Some("@0@".to_string()),
            check_negative_epsilon_cycles: true,
            warn_negative_weights: false,
            warnings_are_errors: false,
            disjunct_multiple_transducers: disjunct,
        }
    }

    /// The names of the transducers the tool writes of `text`.
    fn written(text: &str, options: &Options) -> Vec<String> {
        let mut common =
            crate::hfst_commandline::hfst_set_program_name("hfst-txt2fst", "0.1", "HfstTxt2Fst");
        common.input_filename = "in.att".to_string();
        common.silent = true;
        let files = MemoryFiles::default();
        let sink = files.create("out").expect("a file");
        let mut out =
            HfstOutputStream::from_writer(sink, options.output_format, true).expect("a stream");
        assert_eq!(
            process_stream(&common, options, &mut out, &mut text.as_bytes()),
            0
        );
        let bytes = files.get("out").expect("written");
        let mut cursor = std::io::Cursor::new(bytes);
        let mut stream = HfstInputStream::read_from(&mut cursor).expect("a stream");
        let mut names = Vec::new();
        while stream.is_good() {
            names.push(stream.read().expect("a transducer").get_name());
        }
        names
    }

    // [spec:hfst:sem:hfst-txt2fst.process-stream-fn/test]
    // Each transducer the library reads is written, named after the input;
    // joined, they are written as one.
    #[test]
    fn each_transducer_read_is_written() {
        let att = "0\t1\ta\tb\n1\n--\n0\t1\tc\tc\t0.5\n1\n";
        assert_eq!(
            written(att, &options(false, false)),
            ["text(in.att)", "text(in.att)"]
        );
        assert_eq!(written(att, &options(false, true)), [""]);
        let prolog = "network(n).\nsymbol(n, \"a\").\narc(n, 0, 1, \"a\").\nfinal(n, 1).\n";
        assert_eq!(written(prolog, &options(true, false)), ["text(in.att)"]);
    }
}
