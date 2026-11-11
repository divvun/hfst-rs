//! Port of tools/src/hfst-strings2fst.cc — the string compiling command-line
//! tool, around the library's [`hfst::tools::strings2fst`]. Option handling
//! is clap 4 derive through [`crate::cli`].

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, error_at_line, hfst_error, hfst_error_at_line, hfst_parse_format_name,
    hfst_set_program_name, hfst_warning_at_line, last_os_error_code, verbose_print,
};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::tools::strings2fst::{
    LogBase, StringsCompiler, StringsError, StringsOptions, multichar_symbols, negative_weight,
};
use std::io::BufRead;

/// hfst-strings2fst's command line.
//
// '--norm'/'--log'/'--log10' carried the getopt `val`s '2'/'3'/'4', and this
// port's getopt derived its shorts from `val` alone, so '-2'/'-3'/'-4' have
// always been accepted spellings of them. Declared here so they still are.
// [spec:hfst:def:hfst-strings2fst.parse-options-fn]
// [spec:hfst:sem:hfst-strings2fst.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Compile string pairs and pair-strings into transducer(s)")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Write result in FMT format: foma, openfst-tropical, sfst,
    /// optimized-lookup-weighted, optimized-lookup-unweighted
    #[arg(short = 'f', long = "format", value_name = "FMT")]
    format: Option<String>,

    /// Disjunct all strings instead of transforming each string into a
    /// separate transducer
    #[arg(short = 'j', long = "disjunct-strings")]
    disjunct_strings: bool,

    /// Divide each weight by sum of all weights (with option -j)
    #[arg(short = '2', long = "norm")]
    norm: bool,

    /// Take negative natural logarithm of each weight
    #[arg(short = '3', long = "log")]
    log: bool,

    /// Take negative 10-based logarithm of each weight
    #[arg(short = '4', long = "log10")]
    log10: bool,

    /// Input is in pairstring format
    #[arg(short = 'p', long = "pairstrings")]
    pairstrings: bool,

    /// Input has spaces between symbols/symbol pairs
    #[arg(short = 'S', long = "has-spaces")]
    has_spaces: bool,

    /// Interpret string EPS as epsilon (default @0@)
    #[arg(
        short = 'e',
        long = "epsilon",
        value_name = "EPS",
        allow_hyphen_values = true
    )]
    epsilon: Option<String>,

    /// Strings that must be tokenized as one symbol, one per line of FILE
    #[arg(short = 'm', long = "multichar-symbols", value_name = "FILE")]
    multichar_symbols: Option<String>,

    /// Warning switch: error, no-error, negative-weights, no-negative-weights.
    /// (The C long table spells this '--Wstuff'; the accepted spelling is
    /// kept.)
    #[arg(short = 'W', long = "Wstuff", value_name = "SWITCH")]
    warning: Option<String>,
}

impl Args {
    /// Case 'f': hfst_parse_format_name, itself fatal on an unknown name.
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
                        &format!("unrecognised warning option -W{}", other),
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

/// Print why line `line_n` did not compile, and end the process.
fn fail_line(common: &CommonOptions, line_n: u32, e: &StringsError) {
    let file = &common.input_filename;
    let message = e.to_string();
    match e {
        StringsError::NotAWeight(_) => hfst_error(common, 1, last_os_error_code(), &message),
        StringsError::NegativeWeight(_) => hfst_error_at_line(common, 1, 0, file, line_n, &message),
        StringsError::UnescapedColons { .. } | StringsError::NotUtf8(_) => {
            error_at_line(1, last_os_error_code(), file, line_n, &message)
        }
        StringsError::Logarithm | StringsError::Build(_) => error(common, 1, 0, &message),
    }
}

// [spec:hfst:req:embed.tools-in-library]
// The compiling is the library's StringsCompiler; the tool reads the lines,
// writes each transducer as it comes, and prints what it is told.
fn process_stream(
    common: &CommonOptions,
    options: StringsOptions,
    outstream: &mut HfstOutputStream,
    input: &mut dyn BufRead,
) -> i32 {
    // Every --format compiles at tropical; the optimized-lookup and foma
    // formats convert at each write ([dec:hfst:monomorphic-backends]).
    let disjunct = options.disjunct;
    let normalize = options.normalize;
    let log = options.log;
    let mut compiler = match StringsCompiler::<hfst_openfst::StdVectorFst>::new(options) {
        Ok(c) => c,
        Err(e) => {
            hfst_error(common, 1, 0, &format!("{e}"));
            return 1;
        }
    };
    let mut line_n: u32 = 0;
    let mut line = String::new();
    loop {
        line.clear();
        if input.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        line_n += 1;
        verbose_print(common, &format!("Parsing line {}...\n", line_n));
        let mut warn = |weight: f32| {
            let file = &common.input_filename;
            hfst_warning_at_line(common, 0, 0, file, line_n, &negative_weight(weight));
        };
        let compiled = match compiler.compile_line(&line, &mut warn) {
            Ok(c) => c,
            Err(e) => {
                fail_line(common, line_n, &e);
                return 1;
            }
        };
        if let Some(weight) = compiled.weight {
            verbose_print(
                common,
                &format!("Using final weight {:.6}...\n", weight as f64),
            );
        }
        if let Some(t) = compiled.transducer
            && let Err(e) = outstream.write_converted(&t)
        {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }
    }
    if disjunct {
        if normalize {
            verbose_print(common, "Normalising weights...\n");
        }
        if log.is_some() {
            verbose_print(common, "Taking negative logarithm...\n");
        }
    }
    match compiler.finish() {
        Ok(Some(t)) => {
            if let Err(e) = outstream.write_converted(&t) {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        }
        Ok(None) => {}
        Err(e) => {
            error(common, 1, 0, &e.to_string());
            return 1;
        }
    }
    0
}

// [spec:hfst:def:hfst-strings2fst.main-fn]
// [spec:hfst:sem:hfst-strings2fst.main-fn]
pub(super) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "Strings2Fst");
    let (common, args) = cli::parse::<Args>(common, args)?;

    let (warn_negative_weights, warnings_are_errors) = args.warning_switches(&common);
    let mut output_format = args.output_format(&common);
    let mut options = StringsOptions {
        epsilon: args.epsilon.clone().unwrap_or_else(|| "@0@".to_string()),
        multichar_symbols: Vec::new(),
        pairstrings: args.pairstrings,
        has_spaces: args.has_spaces,
        disjunct: args.disjunct_strings,
        normalize: args.norm,
        log: if args.log {
            Some(LogBase::E)
        } else if args.log10 {
            Some(LogBase::Ten)
        } else {
            None
        },
        warn_negative_weights,
        warnings_are_errors,
    };
    // The default the C applied after the parameter checks.
    if output_format == ImplementationType::UNSPECIFIED_TYPE {
        verbose_print(
            &common,
            "Output format not specified, defaulting to openfst tropical\n",
        );
        output_format = ImplementationType::TROPICAL_OPENFST_TYPE;
    }

    if let Some(fname) = &args.multichar_symbols {
        verbose_print(
            &common,
            &format!("Reading multichar symbols from {}\n", fname),
        );
        match std::fs::read_to_string(fname) {
            Ok(contents) => {
                for symbol in multichar_symbols(&contents) {
                    verbose_print(&common, &format!("Defining multichar symbol {}\n", symbol));
                    options.multichar_symbols.push(symbol);
                }
            }
            Err(_) => {
                error(
                    &common,
                    1,
                    last_os_error_code(),
                    "Multichar symbol file can't be read.",
                );
            }
        }
    }

    // close output buffers, we use output streams
    let output_opened = common.output_filename != "<stdout>";
    verbose_print(
        &common,
        &format!(
            "Reading from {}, writing to {}\n",
            common.input_filename, common.output_filename
        ),
    );
    // here starts the buffer handling part
    let outstream_result = if output_opened {
        HfstOutputStream::new_filename(&common.output_filename, output_format, true)
    } else {
        HfstOutputStream::new(output_format, true)
    };
    let mut outstream = match outstream_result {
        Ok(s) => s,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return Err(1);
        }
    };
    let mut input = match common.input_reader() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("hfst-strings2fst: cannot open input: {e}");
            return Err(1);
        }
    };
    process_stream(&common, options, &mut outstream, &mut *input);
    Ok(())
}
