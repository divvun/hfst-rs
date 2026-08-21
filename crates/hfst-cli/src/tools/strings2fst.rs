//! Faithful 1:1 port of tools/src/hfst-strings2fst.cc — the string compiling
//! command-line tool.
//!
//! Compiles string pairs and pair-strings into transducer(s). Option handling
//! is clap 4 derive through [`crate::cli`].

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, error_at_line, hfst_error, hfst_error_at_line, hfst_parse_format_name,
    hfst_set_program_name, hfst_strtoweight, hfst_warning_at_line, redirect_converting,
    verbose_print,
};
use crate::hfst_tool_metadata::hfst_set_name;
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_data_types::{ImplementationType, Symbol};
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_strings2_fst_tokenizer::{HfstStrings2FstTokenizer, StringPairVector};
use hfst::hfst_transducer::HfstTransducer;
use std::io::BufRead;

// ---------------------------------------------------------------------------
// Tool-global state. C: file-scope static variables.
// ---------------------------------------------------------------------------

/// hfst-strings2fst's own options (the former tool-specific `static mut`s).
struct Options {
    /// '-e, --epsilon': symbol interpreted as epsilon. None until set; defaults to "@0@".
    epsilonname: Option<String>,
    /// '-S, --has-spaces': input has spaces between symbols/symbol pairs.
    has_spaces: bool,
    /// '-j, --disjunct-strings': disjunct all strings into a single transducer.
    disjunct_strings: bool,
    /// '-p, --pairstrings': input is in pairstring format.
    pairstrings: bool,
    /// '-m, --multichar-symbols': file listing strings tokenized as one symbol.
    multichar_symbol_filename: Option<String>,
    /// Multichar symbols read from the -m file.
    multichar_symbols: Vec<Symbol>,

    /// Running sum of all path weights (used by `divide_by_sum_of_weights`).
    sum_of_weights: f32,
    /// '--norm': divide each weight by the sum of all weights.
    normalize_weights: bool,
    /// '--log': take negative natural logarithm of each weight.
    logarithmic_weights_e: bool,
    /// '--log10': take negative 10-based logarithm of each weight.
    logarithmic_weights_10: bool,

    /// '-Wnegative-weights': warn on negative weights (default true).
    warn_negative_weights: bool,
    /// '-Werror': treat warnings as errors.
    warnings_are_errors: bool,

    /// '-f, --format': output implementation format.
    output_format: ImplementationType,
}

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

// [spec:hfst:def:hfst-strings2fst.divide-by-sum-of-weights-fn]
// [spec:hfst:sem:hfst-strings2fst.divide-by-sum-of-weights-fn]
fn divide_by_sum_of_weights(sum_of_weights: f32, weight: f32) -> f32 {
    if sum_of_weights == 0.0 {
        return 0.0;
    }
    weight / sum_of_weights
}

// [spec:hfst:def:hfst-strings2fst.take-negative-logarithm-e-fn]
// [spec:hfst:sem:hfst-strings2fst.take-negative-logarithm-e-fn]
fn take_negative_logarithm_e(common: &CommonOptions, weight: f32) -> f32 {
    let result;
    if weight == 0.0 {
        result = 0.0; // shoud be INFINITY, but doesn't work in transitions
    } else {
        result = -(weight.ln());
        // C checked errno (EDOM/ERANGE) after log(); Rust's ln() never sets errno
        // and yields a non-finite result on a domain/range error instead.
        if !result.is_finite() {
            error(common, 1, 0, "unable to take negative logarithm");
        }
    }
    result
}

// [spec:hfst:def:hfst-strings2fst.take-negative-logarithm-10-fn]
// [spec:hfst:sem:hfst-strings2fst.take-negative-logarithm-10-fn]
fn take_negative_logarithm_10(common: &CommonOptions, weight: f32) -> f32 {
    let result;
    if weight == 0.0 {
        result = 0.0; // shoud be INFINITY, but doesn't work in transitions
    } else {
        result = -(weight.log10());
        // C checked errno (EDOM/ERANGE) after log10(); Rust's log10() never sets
        // errno and yields a non-finite result on a domain/range error instead.
        if !result.is_finite() {
            error(common, 1, 0, "unable to take negative logarithm");
        }
    }
    result
}

fn last_os_error_code() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

// [spec:hfst:def:hfst-strings2fst.process-stream-fn]
// [spec:hfst:sem:hfst-strings2fst.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &mut Options,
    outstream: &mut HfstOutputStream,
    input: &mut dyn BufRead,
) -> i32 {
    // The parsed --format is matched ONCE into the backend type
    // ([dec:hfst:monomorphic-backends]); optimized-lookup formats build
    // at tropical and convert at each write.
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
    options: &mut Options,
    outstream: &mut HfstOutputStream,
    input: &mut dyn BufRead,
) -> i32 {
    let mut transducer_n: usize = 0;
    let mut disjunction = HfstBasicTransducer::new();
    let mut line_n: usize = 0;

    let epsilonname = options.epsilonname.clone().unwrap_or_default();
    let multichar_symbol_tokenizer =
        match HfstStrings2FstTokenizer::new(&options.multichar_symbols, &epsilonname) {
            Ok(t) => t,
            Err(e) => {
                hfst_error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };

    let inputfilename = common.input_filename.clone();

    let mut line = String::new();
    loop {
        line.clear();
        // C: hfst_getline keeps the trailing newline; Ok(0) at EOF == getline's -1.
        if input.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        transducer_n += 1;
        let _ = transducer_n; // C++ counts but never reads it
        line_n += 1;
        verbose_print(common, &format!("Parsing line {}...\n", line_n));

        // parse line end and weight (the C++ mutated the buffer in place,
        // writing '\0' at the tab/newline; here we slice instead).
        let line_bytes = line.as_bytes();
        let tab_pos = line_bytes.iter().position(|&b| b == b'\t');
        let mut weight: f64 = 0.0;
        let mut weighted = false;

        let string_end_idx;
        if let Some(tab) = tab_pos {
            // weight string is from tab+1 to the first '\n'/'\r' (the C++
            // inserted a '\0' there).
            let mut we = tab + 1;
            while we < line_bytes.len() && line_bytes[we] != b'\n' && line_bytes[we] != b'\r' {
                we += 1;
            }
            let weight_str = String::from_utf8_lossy(&line_bytes[tab + 1..we]).into_owned();
            weight = hfst_strtoweight(common, &weight_str) as f64;
            weighted = true;
            let errm = format!(
                "Found negative weight {:.6}; negative weights are supported but iffy, if you really need them use -Wno-negative-weights",
                weight
            );
            if (weight < 0.0) && options.warn_negative_weights {
                if options.warnings_are_errors {
                    hfst_error_at_line(common, 1, 0, &inputfilename, line_n as u32, &errm);
                } else {
                    hfst_warning_at_line(common, 0, 0, &inputfilename, line_n as u32, &errm);
                }
            }
            string_end_idx = tab;
        } else {
            // string_end walks to first '\0', '\n' or '\r'
            let mut se = 0usize;
            while se < line_bytes.len() && line_bytes[se] != b'\n' && line_bytes[se] != b'\r' {
                se += 1;
            }
            string_end_idx = se;
        }

        // Parse the string (C: cstr(line) up to the inserted '\0').
        let parse_line = String::from_utf8_lossy(&line_bytes[..string_end_idx]).into_owned();
        let pairstrings = options.pairstrings;
        let has_spaces = options.has_spaces;
        let tok_ref = &multichar_symbol_tokenizer;
        let pl = parse_line.clone();
        let tok_result = if pairstrings {
            tok_ref.tokenize_pair_string(&pl, has_spaces)
        } else {
            tok_ref.tokenize_string_pair(&pl, has_spaces)
        };
        let spv: StringPairVector = match tok_result {
            Ok(v) => v,
            Err(e) => {
                if e.kind == hfst::error::ErrorKind::UnescapedColsFound {
                    if pairstrings {
                        error_at_line(
                            1,
                            last_os_error_code(),
                            &inputfilename,
                            line_n as u32,
                            &format!(
                                "String `{}' contains unescaped ':'-symbols,\nwhich are not pair separators. Use `\\:' for literal `:'.",
                                parse_line
                            ),
                        );
                    } else {
                        error_at_line(
                            1,
                            last_os_error_code(),
                            &inputfilename,
                            line_n as u32,
                            &format!(
                                "String `{}' contains unescaped `:'-symbols,\nwhich are not pair separators. Use `\\:\' for literal `:'.\nIf you are compiling pair strings, use option -p.",
                                parse_line
                            ),
                        );
                    }
                } else {
                    // IncorrectUtf8CodingException
                    error_at_line(
                        1,
                        last_os_error_code(),
                        &inputfilename,
                        line_n as u32,
                        &format!("Input string `{}' is not valid utf-8.", parse_line),
                    );
                }
                // error_at_line with EXIT_FAILURE exits; this is unreachable.
                StringPairVector::new()
            }
        };

        // Handle the weight
        let mut path_weight: f32 = 0.0;

        if weighted {
            options.sum_of_weights += weight as f32;
            path_weight = weight as f32;
            verbose_print(common, &format!("Using final weight {:.6}...\n", weight));
        }

        if !options.disjunct_strings {
            // each string into a transducer
            let mut tr = HfstBasicTransducer::new();

            if options.logarithmic_weights_e {
                path_weight = take_negative_logarithm_e(common, weight as f32);
            } else if options.logarithmic_weights_10 {
                path_weight = take_negative_logarithm_10(common, weight as f32);
            }

            tr.disjunct_path(&spv, path_weight);
            let mut res: HfstTransducer<B> = match HfstTransducer::new_from_basic(&tr) {
                Ok(v) => v,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            hfst_set_name(&mut res, "", "string");
            if let Err(e) = redirect_converting(outstream, &mut res) {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        } else {
            // disjunct all strings into a single transducer
            // do not take negative logarithm yet
            disjunction.disjunct_path(&spv, path_weight);
        }
    }
    // C: free(line); -> owned String, drops at scope end.
    if options.disjunct_strings {
        // The C++ applied these reweightings on the built HfstTransducer via
        // its unconditional `transform_weights(fn(f32)->f32)`. The tool options
        // that these functions used to read from `static mut` are now closed
        // over, so they run on the basic `disjunction` (whose symbol-aware
        // `transform_weights` accepts capturing closures and ignores the symbol
        // args) before the backend conversion — behaviour-identical, since the
        // conversion preserves every arc and final weight.
        if options.normalize_weights {
            verbose_print(common, "Normalising weights...\n");
            let sum_of_weights = options.sum_of_weights;
            disjunction = disjunction
                .transform_weights(|w, _i, _o| divide_by_sum_of_weights(sum_of_weights, w));
        }
        if options.logarithmic_weights_e {
            verbose_print(common, "Taking negative logarithm...\n");
            disjunction =
                disjunction.transform_weights(|w, _i, _o| take_negative_logarithm_e(common, w));
        } else if options.logarithmic_weights_10 {
            verbose_print(common, "Taking negative logarithm...\n");
            disjunction =
                disjunction.transform_weights(|w, _i, _o| take_negative_logarithm_10(common, w));
        }

        let mut res: HfstTransducer<B> = match HfstTransducer::new_from_basic(&disjunction) {
            Ok(v) => v,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };

        hfst_set_name(&mut res, "?", "strings");
        if let Err(e) = redirect_converting(outstream, &mut res) {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }
    }
    0
}

// [spec:hfst:def:hfst-strings2fst.main-fn]
// [spec:hfst:sem:hfst-strings2fst.main-fn]
pub fn run(args: Vec<String>) -> i32 {
    cli::exit_code(execute(args))
}

fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "Strings2Fst");
    let (common, args) = cli::parse::<Args>(common, args)?;

    let (warn_negative_weights, warnings_are_errors) = args.warning_switches(&common);
    let mut options = Options {
        epsilonname: args.epsilon.clone(),
        has_spaces: args.has_spaces,
        disjunct_strings: args.disjunct_strings,
        pairstrings: args.pairstrings,
        multichar_symbol_filename: args.multichar_symbols.clone(),
        multichar_symbols: Vec::new(),
        sum_of_weights: 0.0,
        normalize_weights: args.norm,
        logarithmic_weights_e: args.log,
        logarithmic_weights_10: args.log10,
        warn_negative_weights,
        warnings_are_errors,
        output_format: args.output_format(&common),
    };
    // The two defaults the C applied after the parameter checks.
    if options.output_format == ImplementationType::UNSPECIFIED_TYPE {
        verbose_print(
            &common,
            "Output format not specified, defaulting to openfst tropical\n",
        );
        options.output_format = ImplementationType::TROPICAL_OPENFST_TYPE;
    }
    if options.epsilonname.is_none() {
        options.epsilonname = Some("@0@".to_string());
    }

    if let Some(fname) = options.multichar_symbol_filename.clone() {
        verbose_print(
            &common,
            &format!("Reading multichar symbols from {}\n", fname),
        );
        match std::fs::read_to_string(&fname) {
            Ok(contents) => {
                for multichar_line in contents.lines() {
                    if !multichar_line.is_empty() {
                        verbose_print(
                            &common,
                            &format!("Defining multichar symbol {}\n", multichar_line),
                        );
                        options.multichar_symbols.push(Symbol::new(multichar_line));
                    }
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
        HfstOutputStream::new_filename(&common.output_filename, options.output_format, true)
    } else {
        HfstOutputStream::new(options.output_format, true)
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
    process_stream(&common, &mut options, &mut outstream, &mut *input);
    Ok(())
}
