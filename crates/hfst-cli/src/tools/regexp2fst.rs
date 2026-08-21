//! Faithful 1:1 port of tools/src/hfst-regexp2fst.cc — the regular expression
//! compiling command-line tool, driving the hfst XreCompiler. Option handling
//! is clap 4 derive through [`crate::cli`].

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, hfst_error_at_line, hfst_parse_format_name, hfst_set_program_name, redirect_converting,
    verbose_print,
};
use crate::hfst_tool_metadata::hfst_set_name;
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst::xre::XreCompiler;
use std::io::BufRead;

/// hfst-regexp2fst's command line.
//
// '-l'/'-S' both wrote `line_separated`, so the last one on the line decided;
// clap reproduces that with mutual overrides_with rather than by inspecting
// match indices.
// [spec:hfst:def:hfst-regexp2fst.parse-options-fn]
// [spec:hfst:sem:hfst-regexp2fst.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Compile (weighted) regular expressions into transducer(s)")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Write result in FMT format: foma, sfst, openfst-tropical
    #[arg(short = 'f', long = "format", value_name = "FMT")]
    format: Option<String>,

    /// Disjunct all regexps instead of transforming each regexp into a
    /// separate transducer
    #[arg(short = 'j', long = "disjunct")]
    disjunct: bool,

    /// Input is line separated (default)
    #[arg(short = 'l', long = "line", overrides_with = "semicolon")]
    line: bool,

    /// Input is semicolon separated
    #[arg(short = 'S', long = "semicolon", overrides_with = "line")]
    semicolon: bool,

    /// Map EPS as zero, i.e. epsilon
    #[arg(
        short = 'e',
        long = "epsilon",
        value_name = "EPS",
        allow_hyphen_values = true
    )]
    epsilon: Option<String>,

    /// Whether flag diacritics are treated as ordinary symbols in composition
    /// (default is false). VALUE is one of true/ON/yes or false/OFF/no
    #[arg(short = 'x', long = "xerox-composition", value_name = "VALUE")]
    xerox_composition: Option<String>,

    /// Toggle xfst compatibility option VARIABLE (only flag-is-epsilon,
    /// default OFF)
    #[arg(short = 'X', long = "xfst", value_name = "VARIABLE")]
    xfst: Option<String>,

    /// Do not expand '?' symbols
    #[arg(short = 'H', long = "do-not-harmonize")]
    do_not_harmonize: bool,

    /// Harmonize flag diacritics
    #[arg(short = 'F', long = "harmonize-flags")]
    harmonize_flags: bool,

    /// Encode weights when minimizing (default is false)
    #[arg(short = 'E', long = "encode-weights")]
    encode_weights: bool,

    /// Determinize result instead of minimizing it
    #[arg(short = 'M', long = "do-not-minimize")]
    do_not_minimize: bool,
}

impl Args {
    /// Case 'x': the two VALUE vocabularies, fatal on anything else.
    fn xerox_composition(&self, common: &CommonOptions) -> Result<bool, i32> {
        match self.xerox_composition.as_deref() {
            None => Ok(false),
            Some("yes") | Some("true") | Some("ON") => Ok(true),
            Some("no") | Some("false") | Some("OFF") => Ok(false),
            Some(other) => {
                error(
                    common,
                    1,
                    0,
                    &format!("unknown option to --xerox-composition: '{}'\n", other),
                );
                Err(1)
            }
        }
    }

    /// Case 'X': the single xfst variable this build knows.
    fn flag_is_epsilon(&self, common: &CommonOptions) -> Result<bool, i32> {
        match self.xfst.as_deref() {
            None => Ok(false),
            Some("flag-is-epsilon") => Ok(true),
            Some(other) => {
                error(
                    common,
                    1,
                    0,
                    &format!("Error: unknown option to --xfst: '{}'\n", other),
                );
                Err(1)
            }
        }
    }

    fn output_format(&self, common: &CommonOptions) -> ImplementationType {
        match self.format.as_deref() {
            Some(name) => hfst_parse_format_name(common, name),
            None => ImplementationType::UNSPECIFIED_TYPE,
        }
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
        // All three rejections happened inside the C getopt loop, before the
        // parameter checks.
        self.output_format(opts);
        self.xerox_composition(opts)?;
        self.flag_is_epsilon(opts)?;
        Ok(())
    }
}

/// hfst-regexp2fst's resolved tool state (the former tool-specific `static mut`s).
struct Options {
    /// '-e, --epsilon=EPS': map EPS as zero, i.e. epsilon.
    epsilonname: Option<String>,
    /// '-j, --disjunct': disjunct all regexps into a single transducer.
    disjunct_expressions: bool,
    /// '-l, --line' / '-S, --semicolon': input is line separated (default).
    line_separated: bool,
    /// '-E, --encode-weights': encode weights when minimizing.
    encode_weights: bool,
    /// '-f, --format=FMT': write result in FMT format.
    output_format: ImplementationType,
    /// '-H, --do-not-harmonize': whether to expand '?' symbols.
    harmonize: bool,
    /// '-F, --harmonize-flags': harmonize flag diacritics.
    harmonize_flags: bool,
    /// '-M, --do-not-minimize': determinize result instead of minimizing.
    minimize_result: bool,
    /// '--xfst flag-is-epsilon' (was the 'flag_is_epsilon_in_composition'
    /// file-static global; now threaded into the XRE compiler via
    /// 'set_flag_is_epsilon').
    flag_is_epsilon: bool,
    /// '--xerox-composition' (was the 'xerox_composition' file-static global;
    /// now threaded into the XRE compiler via 'set_xerox_composition').
    xerox_composition: bool,
}

// [spec:hfst:def:hfst-regexp2fst.process-stream-fn]
// [spec:hfst:sem:hfst-regexp2fst.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &Options,
    outstream: &mut HfstOutputStream,
    input: &mut dyn BufRead,
) -> i32 {
    // The parsed --format is matched ONCE into the compiler's backend
    // type parameter ([dec:hfst:monomorphic-backends]); optimized-lookup
    // formats compile at tropical and convert at each write.
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
    let mut line_count: u32 = 0;
    let mut comp = XreCompiler::<B>::new();
    comp.set_source_name(&common.input_filename);
    comp.set_verbosity(common.verbose);
    comp.set_error_stream(());
    comp.set_harmonization(options.harmonize);
    comp.set_flag_harmonization(options.harmonize_flags);
    comp.set_minimize_result(options.minimize_result);
    comp.set_flag_is_epsilon(options.flag_is_epsilon);
    comp.set_xerox_composition(options.xerox_composition);
    comp.set_encode_weights(options.encode_weights);
    let _ = &options.epsilonname;
    let mut disjunction: HfstTransducer<B> = HfstTransducer::new();

    let mut first_line: Option<String> = None;

    if !options.line_separated {
        // C: read the whole input into a NUL-terminated buffer and walk it
        // with a char* cursor. Here we read it into a String and track a byte
        // offset; compile_first reports how many bytes it consumed.
        let mut content = String::new();
        let _ = input.read_to_string(&mut content);
        let mut offset: usize = 0;
        let mut chars_read: u32 = 0;

        loop {
            transducer_n += 1;
            verbose_print(
                common,
                &format!("Compiling expression #{}\n", transducer_n as i32),
            );
            let remaining = &content[offset..];
            let compiled = comp.compile_first(remaining, &mut chars_read);
            // (the C wraps compile_first in try/catch on HfstException; the
            // Rust path currently panics rather than throwing, so the catch
            // arm that calls hfst_error is not reproduced here.)
            if compiled.is_none() {
                if comp.contained_only_comments() {
                    if transducer_n == 1 {
                        error(
                            common,
                            1,
                            0,
                            &format!(
                                "{}: XRE parsing failed: expression #{} \
                                 contains only whitespace or comments",
                                common.input_filename, transducer_n as u32
                            ),
                        );
                    }
                    break;
                } else {
                    error(
                        common,
                        1,
                        0,
                        &format!(
                            "{}: XRE parsing failed \
                             in expression #{} separated by semicolons",
                            common.input_filename, transducer_n as u32
                        ),
                    );
                }
            }
            offset += chars_read as usize;
            if let Some(mut compiled) = compiled {
                if options.disjunct_expressions {
                    if let Err(e) = disjunction.disjunct(&compiled, options.harmonize) {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                } else {
                    hfst_set_name(&mut compiled, "?", "xre");
                    if let Err(e) = redirect_converting(outstream, &mut compiled) {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                }
                // C: delete compiled; -> owned, drops here.
            }
            if offset >= content.len() {
                break;
            }
        }
    } else {
        let mut input_contains_only_whitespace_or_comments = true;
        let mut line = String::new();
        loop {
            line.clear();
            if input.read_line(&mut line).unwrap_or(0) == 0 {
                if input_contains_only_whitespace_or_comments {
                    error(
                        common,
                        1,
                        0,
                        &format!(
                            "{}: XRE parsing failed: \
                             input contains only whitespace or comments",
                            common.input_filename
                        ),
                    );
                }
                break;
            }
            if first_line.is_none() {
                first_line = Some(line.clone());
            }
            // Skip leading '\n', '\r' and ' ' (C: pointer-walk over exp).
            let exp = line.trim_start_matches(['\n', '\r', ' ']).to_string();
            line_count += 1;
            if exp.is_empty() {
                verbose_print(
                    common,
                    &format!("Skipping whitespace expression #{}", line_count),
                );
                continue;
            }
            transducer_n += 1;
            let _ = transducer_n; // C++ counts but never reads it
            verbose_print(common, &format!("Compiling expression {}\n", line_count));
            let compiled = comp.compile(&exp);
            // (the C wraps compile in try/catch on HfstException calling
            // hfst_error_at_line; the Rust path panics rather than throwing,
            // so the catch arm is not reproduced here.)
            let Some(mut compiled) = compiled else {
                if !comp.contained_only_comments() {
                    hfst_error_at_line(
                        common,
                        1,
                        0,
                        &common.input_filename,
                        line_count,
                        "XRE parsing failed\n",
                    );
                }
                continue;
            };
            input_contains_only_whitespace_or_comments = false;

            if options.disjunct_expressions {
                if let Err(e) = disjunction.disjunct(&compiled, options.harmonize) {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            } else {
                hfst_set_name(&mut compiled, "?", "xre");
                if let Err(e) = redirect_converting(outstream, &mut compiled) {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            }
            // C: delete compiled; -> owned, drops here.
        }
    }

    if options.disjunct_expressions {
        // Both branches of the C++ if/else set the same name.
        hfst_set_name(&mut disjunction, "?", "xre");
        if let Err(e) = redirect_converting(outstream, &mut disjunction) {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }
    }
    // C: free(line); free(first_line); -> owned String/Option, drop here.
    drop(first_line);
    0
}

// [spec:hfst:def:hfst-regexp2fst.main-fn]
// [spec:hfst:sem:hfst-regexp2fst.main-fn]
pub fn run(args: Vec<String>) -> i32 {
    cli::exit_code(execute(args))
}

fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.2", "Regexp2Fst");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let mut options = Options {
        epsilonname: args.epsilon.clone(),
        disjunct_expressions: args.disjunct,
        // Line separation is the default; only '-S' as the last of the pair
        // turns it off.
        line_separated: !args.semicolon,
        encode_weights: args.encode_weights,
        output_format: args.output_format(&common),
        harmonize: !args.do_not_harmonize,
        harmonize_flags: args.harmonize_flags,
        minimize_result: !args.do_not_minimize,
        flag_is_epsilon: args.flag_is_epsilon(&common)?,
        xerox_composition: args.xerox_composition(&common)?,
    };
    // The default the C applied after the parameter checks.
    if options.output_format == ImplementationType::UNSPECIFIED_TYPE {
        verbose_print(
            &common,
            "Output format not specified, defaulting to openfst tropical\n",
        );
        options.output_format = ImplementationType::TROPICAL_OPENFST_TYPE;
    }
    if common.debug {
        // xredebug = 1;
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
    // here starts the buffer handling part
    let mut outstream = match if output_opened {
        HfstOutputStream::new_filename(&common.output_filename, options.output_format, true)
    } else {
        HfstOutputStream::new(options.output_format, true)
    } {
        Ok(s) => s,
        Err(e) => {
            eprintln!("hfst-regexp2fst: cannot open output: {e}");
            return Err(1);
        }
    };
    let mut input = match common.input_reader() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("hfst-regexp2fst: cannot open input: {e}");
            return Err(1);
        }
    };
    process_stream(&common, &options, &mut outstream, &mut *input);

    Ok(())
}
