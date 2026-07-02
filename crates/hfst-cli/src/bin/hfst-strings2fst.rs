//! Faithful 1:1 port of tools/src/hfst-strings2fst.cc — the string compiling
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).
//!
//! Compiles string pairs and pair-strings into transducer(s).

use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_strings2_fst_tokenizer::{HfstStrings2FstTokenizer, StringPairVector};
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, error_at_line, extend_options_from_env, hfst_error, hfst_error_at_line,
    hfst_parse_format_name, hfst_set_program_name, hfst_strtoweight, hfst_warning_at_line,
    print_more_info, print_report_bugs, verbose_print,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
};
use hfst_cli::hfst_tool_metadata::hfst_set_name;
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use std::io::{BufRead, Write};

// ---------------------------------------------------------------------------
// Tool-global state. C: file-scope static variables.
// ---------------------------------------------------------------------------

static mut EPSILONNAME: Option<String> = None; // None until set; defaults to "@0@"
static mut HAS_SPACES: bool = false;
static mut DISJUNCT_STRINGS: bool = false;
static mut PAIRSTRINGS: bool = false;
static mut MULTICHAR_SYMBOL_FILENAME: Option<String> = None;
static mut MULTICHAR_SYMBOLS: Vec<String> = Vec::new();

static mut SUM_OF_WEIGHTS: f32 = 0.0;
static mut NORMALIZE_WEIGHTS: bool = false;
static mut LOGARITHMIC_WEIGHTS_E: bool = false;
static mut LOGARITHMIC_WEIGHTS_10: bool = false;

static mut WARN_NEGATIVE_WEIGHTS: bool = true;
static mut WARNINGS_ARE_ERRORS: bool = false;

static mut OUTPUT_FORMAT: ImplementationType = ImplementationType::UNSPECIFIED_TYPE;

// [spec:hfst:def:hfst-strings2fst.divide-by-sum-of-weights-fn]
// [spec:hfst:sem:hfst-strings2fst.divide-by-sum-of-weights-fn]
fn divide_by_sum_of_weights(weight: f32) -> f32 {
    let sum = unsafe { SUM_OF_WEIGHTS };
    if sum == 0.0 {
        return 0.0;
    }
    weight / sum
}

// [spec:hfst:def:hfst-strings2fst.take-negative-logarithm-e-fn]
// [spec:hfst:sem:hfst-strings2fst.take-negative-logarithm-e-fn]
fn take_negative_logarithm_e(weight: f32) -> f32 {
    let result;
    if weight == 0.0 {
        result = 0.0; // shoud be INFINITY, but doesn't work in transitions
    } else {
        result = -(weight.ln());
        // C checked errno (EDOM/ERANGE) after log(); Rust's ln() never sets errno
        // and yields a non-finite result on a domain/range error instead.
        if !result.is_finite() {
            error(1, 0, "unable to take negative logarithm");
        }
    }
    result
}

// [spec:hfst:def:hfst-strings2fst.take-negative-logarithm-10-fn]
// [spec:hfst:sem:hfst-strings2fst.take-negative-logarithm-10-fn]
fn take_negative_logarithm_10(weight: f32) -> f32 {
    let result;
    if weight == 0.0 {
        result = 0.0; // shoud be INFINITY, but doesn't work in transitions
    } else {
        result = -(weight.log10());
        // C checked errno (EDOM/ERANGE) after log10(); Rust's log10() never sets
        // errno and yields a non-finite result on a domain/range error instead.
        if !result.is_finite() {
            error(1, 0, "unable to take negative logarithm");
        }
    }
    result
}

fn last_os_error_code() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

// [spec:hfst:def:hfst-strings2fst.print-usage-fn]
// [spec:hfst:sem:hfst-strings2fst.print-usage-fn]
fn print_usage() {
    let mut msg = globals::message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let program_name = globals::program_name();
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
        "If OUTFILE or INFILE is missing or -, standard streams will be used.\nFMT can be {{ foma, openfst-tropical, openfst-log, sfst, \noptimized-lookup-weighted, optimized-lookup-unweighted }}.\nIf EPS is not defined, the default representation of @0@ is used.\nOption --norm precedes option --log.\nThe FILE of option -m lists all multichar-symbols, each symbol\non its own line.\nBackslash '\\' may be used to escape ':', tab and itself. For any\nother symbol x '\\x' means x literally, i.e. is the same as 'x'.\nThe weight of a string can be given after the string separated\nby a tabulator. The weight cannot be zero.\n\n",
    );

    let _ = write!(
        msg,
        "Examples:\n  echo \"cat:dog\" | {}            create cat:dog fst\n  echo \"c:da:ot:g\" | {} -p       same as pairstring\n  echo \"c:d a:o t:g\" | {} -p -S  same as pairstring with spaces\n  echo \"c a t:d o g\" | {} -S     same with spaces\n\n",
        program_name, program_name, program_name, program_name
    );
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
    let _ = write!(msg, "\n");
}

// [spec:hfst:def:hfst-strings2fst.parse-options-fn]
// [spec:hfst:sem:hfst-strings2fst.parse-options-fn]
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
            // tool-specific cases
            let cc = c as u8 as char;
            match cc {
                'e' => {
                    EPSILONNAME = Some(getopt::optarg());
                    continue;
                }
                '2' => {
                    NORMALIZE_WEIGHTS = true;
                    continue;
                }
                '3' => {
                    LOGARITHMIC_WEIGHTS_E = true;
                    continue;
                }
                '4' => {
                    LOGARITHMIC_WEIGHTS_10 = true;
                    continue;
                }
                'j' => {
                    DISJUNCT_STRINGS = true;
                    continue;
                }
                'S' => {
                    HAS_SPACES = true;
                    continue;
                }
                'p' => {
                    PAIRSTRINGS = true;
                    continue;
                }
                'm' => {
                    MULTICHAR_SYMBOL_FILENAME = Some(getopt::optarg());
                    continue;
                }
                'f' => {
                    OUTPUT_FORMAT = hfst_parse_format_name(&getopt::optarg());
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
                        hfst_error(1, 0, &format!("unrecognised warning option -W{}", optarg));
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
        if OUTPUT_FORMAT == ImplementationType::UNSPECIFIED_TYPE {
            verbose_print("Output format not specified, defaulting to openfst tropical\n");
            OUTPUT_FORMAT = ImplementationType::TROPICAL_OPENFST_TYPE;
        }
        if (*std::ptr::addr_of!(EPSILONNAME)).is_none() {
            *std::ptr::addr_of_mut!(EPSILONNAME) = Some("@0@".to_string());
        }
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-strings2fst.process-stream-fn]
// [spec:hfst:sem:hfst-strings2fst.process-stream-fn]
unsafe fn process_stream(outstream: &mut HfstOutputStream, input: &mut dyn BufRead) -> i32 {
    unsafe {
        let mut transducer_n: usize = 0;
        let mut disjunction = HfstBasicTransducer::new();
        let mut line_n: usize = 0;

        let epsilonname = (*std::ptr::addr_of!(EPSILONNAME))
            .clone()
            .unwrap_or_default();
        let multichar_symbol_tokenizer = match HfstStrings2FstTokenizer::new(
            &*std::ptr::addr_of!(MULTICHAR_SYMBOLS),
            &epsilonname,
        ) {
            Ok(t) => t,
            Err(e) => {
                hfst_error(1, 0, &format!("{e}"));
                return 1;
            }
        };

        let inputfilename = globals::input_filename();

        let mut line = String::new();
        loop {
            line.clear();
            // C: hfst_getline keeps the trailing newline; Ok(0) at EOF == getline's -1.
            if input.read_line(&mut line).unwrap_or(0) == 0 {
                break;
            }
            transducer_n += 1;
            line_n += 1;
            verbose_print(&format!("Parsing line {}...\n", line_n));

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
                weight = hfst_strtoweight(&weight_str) as f64;
                weighted = true;
                let errm = format!(
                    "Found negative weight {:.6}; negative weights are supported but iffy, if you really need them use -Wno-negative-weights",
                    weight
                );
                if (weight < 0.0) && WARN_NEGATIVE_WEIGHTS {
                    if WARNINGS_ARE_ERRORS {
                        hfst_error_at_line(1, 0, &inputfilename, line_n as u32, &errm);
                    } else {
                        hfst_warning_at_line(0, 0, &inputfilename, line_n as u32, &errm);
                    }
                }
                string_end_idx = tab;
            }

            // Parse the string (C: cstr(line) up to the inserted '\0').
            let parse_line = String::from_utf8_lossy(&line_bytes[..string_end_idx]).into_owned();
            let pairstrings = PAIRSTRINGS;
            let has_spaces = HAS_SPACES;
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
                SUM_OF_WEIGHTS += weight as f32;
                path_weight = weight as f32;
                verbose_print(&format!("Using final weight {:.6}...\n", weight));
            }

            if !DISJUNCT_STRINGS {
                // each string into a transducer
                let mut tr = HfstBasicTransducer::new();

                if LOGARITHMIC_WEIGHTS_E {
                    path_weight = take_negative_logarithm_e(weight as f32);
                } else if LOGARITHMIC_WEIGHTS_10 {
                    path_weight = take_negative_logarithm_10(weight as f32);
                }

                tr.disjunct_path(&spv, path_weight);
                let mut res = match HfstTransducer::new_from_basic(&tr, OUTPUT_FORMAT) {
                    Ok(v) => v,
                    Err(e) => {
                        error(1, 0, &format!("{e}"));
                        return 1;
                    }
                };
                hfst_set_name(&mut res, "", "string");
                if let Err(e) = outstream.redirect(&mut res) {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            } else {
                // disjunct all strings into a single transducer
                // do not take negative logarithm yet
                disjunction.disjunct_path(&spv, path_weight);
            }
        }
        // C: free(line); -> owned String, drops at scope end.
        if DISJUNCT_STRINGS {
            let mut res = match HfstTransducer::new_from_basic(&disjunction, OUTPUT_FORMAT) {
                Ok(v) => v,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            };

            if NORMALIZE_WEIGHTS {
                verbose_print("Normalising weights...\n");
                if let Err(e) = res.transform_weights(divide_by_sum_of_weights) {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            }
            if LOGARITHMIC_WEIGHTS_E {
                verbose_print("Taking negative logarithm...\n");
                if let Err(e) = res.transform_weights(take_negative_logarithm_e) {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            } else if LOGARITHMIC_WEIGHTS_10 {
                verbose_print("Taking negative logarithm...\n");
                if let Err(e) = res.transform_weights(take_negative_logarithm_10) {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            }

            hfst_set_name(&mut res, "?", "strings");
            if let Err(e) = outstream.redirect(&mut res) {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        }
        0
    }
}

// [spec:hfst:def:hfst-strings2fst.main-fn]
// [spec:hfst:sem:hfst-strings2fst.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "Strings2Fst");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }

        if let Some(fname) = (*std::ptr::addr_of!(MULTICHAR_SYMBOL_FILENAME)).clone() {
            verbose_print(&format!("Reading multichar symbols from {}\n", fname));
            match std::fs::read_to_string(&fname) {
                Ok(contents) => {
                    for multichar_line in contents.lines() {
                        if !multichar_line.is_empty() {
                            verbose_print(&format!(
                                "Defining multichar symbol {}\n",
                                multichar_line
                            ));
                            (*std::ptr::addr_of_mut!(MULTICHAR_SYMBOLS))
                                .push(multichar_line.to_string());
                        }
                    }
                }
                Err(_) => {
                    error(
                        1,
                        last_os_error_code(),
                        "Multichar symbol file can't be read.",
                    );
                }
            }
        }

        // close output buffers, we use output streams
        let output_opened = globals::output_filename() != "<stdout>";
        verbose_print(&format!(
            "Reading from {}, writing to {}\n",
            globals::input_filename(),
            globals::output_filename()
        ));
        // here starts the buffer handling part
        let outstream_result = if output_opened {
            HfstOutputStream::new_filename(&globals::output_filename(), OUTPUT_FORMAT, true)
        } else {
            HfstOutputStream::new(OUTPUT_FORMAT, true)
        };
        let mut outstream = match outstream_result {
            Ok(s) => s,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        };
        let mut input = match globals::input_reader() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("hfst-strings2fst: cannot open input: {e}");
                return 1;
            }
        };
        process_stream(&mut outstream, &mut *input);
        0
    }
}
