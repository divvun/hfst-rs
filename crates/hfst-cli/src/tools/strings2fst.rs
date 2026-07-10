//! Faithful 1:1 port of tools/src/hfst-strings2fst.cc — the string compiling
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).
//!
//! Compiles string pairs and pair-strings into transducer(s).
//!
//! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
//! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
//! `parse_options` and threaded into the processing functions. There are no
//! `static mut` globals and no `unsafe`.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, error_at_line, extend_options_from_env, hfst_error, hfst_error_at_line,
    hfst_parse_format_name, hfst_set_program_name, hfst_strtoweight, hfst_warning_at_line,
    redirect_converting, verbose_print,
};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
};
use crate::hfst_tool_metadata::hfst_set_name;
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_data_types::{ImplementationType, Symbol};
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_strings2_fst_tokenizer::{HfstStrings2FstTokenizer, StringPairVector};
use hfst::hfst_transducer::HfstTransducer;
use std::io::{BufRead, Write};

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

impl Default for Options {
    fn default() -> Options {
        Options {
            epsilonname: None,
            has_spaces: false,
            disjunct_strings: false,
            pairstrings: false,
            multichar_symbol_filename: None,
            multichar_symbols: Vec::new(),
            sum_of_weights: 0.0,
            normalize_weights: false,
            logarithmic_weights_e: false,
            logarithmic_weights_10: false,
            warn_negative_weights: true,
            warnings_are_errors: false,
            output_format: ImplementationType::UNSPECIFIED_TYPE,
        }
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

// [spec:hfst:def:hfst-strings2fst.print-usage-fn]
// [spec:hfst:sem:hfst-strings2fst.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    let mut msg = common.message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let program_name = &common.program_name;
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nCompile string pairs and pair-strings into transducer(s)\n\n",
        program_name
    );
    print_common_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Input/Output options:\n  -i, --input=INFILE     Read input strings from INFILE\n  -o, --output=OUTFILE   Write output transducer to OUTFILE\n",
    );
    let _ = write!(
        msg,
        "String and format options:\n  -f, --format=FMT          Write result in FMT format\n  -j, --disjunct-strings    Disjunct all strings instead of transforming\n                            each string into a separate transducer\n      --norm                Divide each weight by sum of all weights\n                            (with option -j)\n      --log                 Take negative natural logarithm of each weight\n      --log10               Take negative 10-based logarithm of each weight\n  -p, --pairstrings         Input is in pairstring format\n  -S, --has-spaces          Input has spaces between symbols/symbol pairs\n  -e, --epsilon=EPS         Interpret string EPS as epsilon.\n  -m, --multichar-symbols=FILE   Strings that must be tokenized as one symbol.\n",
    );
    let _ = write!(msg, "\n");

    let _ = write!(
        msg,
        "If OUTFILE or INFILE is missing or -, standard streams will be used.\nFMT can be {{ foma, openfst-tropical, sfst, \noptimized-lookup-weighted, optimized-lookup-unweighted }}.\nIf EPS is not defined, the default representation of @0@ is used.\nOption --norm precedes option --log.\nThe FILE of option -m lists all multichar-symbols, each symbol\non its own line.\nBackslash '\\' may be used to escape ':', tab and itself. For any\nother symbol x '\\x' means x literally, i.e. is the same as 'x'.\nThe weight of a string can be given after the string separated\nby a tabulator. The weight cannot be zero.\n\n",
    );

    let _ = write!(
        msg,
        "Examples:\n  echo \"cat:dog\" | {}            create cat:dog fst\n  echo \"c:da:ot:g\" | {} -p       same as pairstring\n  echo \"c:d a:o t:g\" | {} -p -S  same as pairstring with spaces\n  echo \"c a t:d o g\" | {} -S     same with spaces\n\n",
        program_name, program_name, program_name, program_name
    );
    let _ = write!(msg, "\n");
}

// [spec:hfst:def:hfst-strings2fst.parse-options-fn]
// [spec:hfst:sem:hfst-strings2fst.parse-options-fn]
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
            name: "disjunct-strings",
            has_arg: getopt::NO_ARGUMENT,
            val: 'j' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "epsilon",
            has_arg: getopt::REQUIRED_ARGUMENT,
            val: 'e' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "norm",
            has_arg: getopt::NO_ARGUMENT,
            val: '2' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "log",
            has_arg: getopt::NO_ARGUMENT,
            val: '3' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "log10",
            has_arg: getopt::NO_ARGUMENT,
            val: '4' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "pairstrings",
            has_arg: getopt::NO_ARGUMENT,
            val: 'p' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "has-spaces",
            has_arg: getopt::NO_ARGUMENT,
            val: 'S' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "multichar-symbols",
            has_arg: getopt::REQUIRED_ARGUMENT,
            val: 'm' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "format",
            has_arg: getopt::REQUIRED_ARGUMENT,
            val: 'f' as i32,
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
        // tool-specific cases
        let cc = c as u8 as char;
        match cc {
            'e' => {
                options.epsilonname = Some(opt.optarg());
                continue;
            }
            '2' => {
                options.normalize_weights = true;
                continue;
            }
            '3' => {
                options.logarithmic_weights_e = true;
                continue;
            }
            '4' => {
                options.logarithmic_weights_10 = true;
                continue;
            }
            'j' => {
                options.disjunct_strings = true;
                continue;
            }
            'S' => {
                options.has_spaces = true;
                continue;
            }
            'p' => {
                options.pairstrings = true;
                continue;
            }
            'm' => {
                options.multichar_symbol_filename = Some(opt.optarg());
                continue;
            }
            'f' => {
                options.output_format = hfst_parse_format_name(&common, &opt.optarg());
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
                        &format!("unrecognised warning option -W{}", optarg),
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
    Ok((common, options))
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
        if tab_pos.is_none() {
            // string_end walks to first '\0', '\n' or '\r'
            let mut se = 0usize;
            while se < line_bytes.len() && line_bytes[se] != b'\n' && line_bytes[se] != b'\r' {
                se += 1;
            }
            string_end_idx = se;
        } else {
            let tab = tab_pos.unwrap();
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
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "Strings2Fst");
    let (common, mut options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

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
            return 1;
        }
    };
    let mut input = match common.input_reader() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("hfst-strings2fst: cannot open input: {e}");
            return 1;
        }
    };
    process_stream(&common, &mut options, &mut outstream, &mut *input);
    0
}
