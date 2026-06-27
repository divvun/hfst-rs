//! Faithful 1:1 port of tools/src/hfst-strings2fst.cc — the string compiling
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).
//!
//! Compiles string pairs and pair-strings into transducer(s).

use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_strings2_fst_tokenizer::{
    HfstStrings2FstTokenizer, StringPairVector, UnescapedColsFound,
};
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, error_at_line, extend_options_getenv, hfst_error, hfst_error_at_line,
    hfst_getline, hfst_parse_format_name, hfst_set_program_name, hfst_strtoweight,
    hfst_warning_at_line, print_more_info, print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT, hfst_getopt_common_long,
    hfst_getopt_unary_long, print_common_program_options,
};
use hfst_cli::hfst_tool_metadata::hfst_set_name;
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use libc::{c_char, c_int};
use std::ffi::{CStr, CString};

// ---------------------------------------------------------------------------
// Tool-global state. C: file-scope static variables.
// ---------------------------------------------------------------------------

static mut EPSILONNAME: *mut c_char = std::ptr::null_mut(); // FIX: use this
static mut HAS_SPACES: bool = false;
static mut DISJUNCT_STRINGS: bool = false;
static mut PAIRSTRINGS: bool = false;
static mut MULTICHAR_SYMBOL_FILENAME: *mut c_char = std::ptr::null_mut();
static mut MULTICHAR_SYMBOLS: Vec<String> = Vec::new();

static mut SUM_OF_WEIGHTS: f32 = 0.0;
static mut NORMALIZE_WEIGHTS: bool = false;
static mut LOGARITHMIC_WEIGHTS_E: bool = false;
static mut LOGARITHMIC_WEIGHTS_10: bool = false;

static mut WARN_NEGATIVE_WEIGHTS: bool = true;
static mut WARNINGS_ARE_ERRORS: bool = false;

static mut OUTPUT_FORMAT: ImplementationType = ImplementationType::UNSPECIFIED_TYPE;

unsafe fn cstr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

unsafe fn fput(f: *mut libc::FILE, s: &str) {
    let c = CString::new(s).unwrap_or_default();
    unsafe { libc::fputs(c.as_ptr(), f) };
}

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
        set_errno(0);
        result = -(weight.ln());
        if errno() != 0 {
            error(
                libc::EXIT_FAILURE,
                errno(),
                "unable to take negative logarithm",
            );
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
        set_errno(0);
        result = -(weight.log10());
        if errno() != 0 {
            error(
                libc::EXIT_FAILURE,
                errno(),
                "unable to take negative logarithm",
            );
        }
    }
    result
}

fn errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

fn set_errno(v: i32) {
    unsafe {
        #[cfg(target_os = "macos")]
        {
            *libc::__error() = v;
        }
        #[cfg(target_os = "linux")]
        {
            *libc::__errno_location() = v;
        }
    }
}

// [spec:hfst:def:hfst-strings2fst.print-usage-fn]
// [spec:hfst:sem:hfst-strings2fst.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            globals::message_out(),
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\nCompile string pairs and pair-strings into transducer(s)\n\n",
                program_name
            ),
        );
        print_common_program_options(globals::message_out());
        fput(
            globals::message_out(),
            "Input/Output options:\n  -i, --input=INFILE     Read input strings from INFILE\n  -o, --output=OUTFILE   Write output transducer to OUTFILE\n",
        );
        fput(
            globals::message_out(),
            "String and format options:\n  -f, --format=FMT          Write result in FMT format\n  -j, --disjunct-strings    Disjunct all strings instead of transforming\n                            each string into a separate transducer\n      --norm                Divide each weight by sum of all weights\n                            (with option -j)\n      --log                 Take negative natural logarithm of each weight\n      --log10               Take negative 10-based logarithm of each weight\n  -p, --pairstrings         Input is in pairstring format\n  -S, --has-spaces          Input has spaces between symbols/symbol pairs\n  -e, --epsilon=EPS         Interpret string EPS as epsilon.\n  -m, --multichar-symbols=FILE   Strings that must be tokenized as one symbol.\n",
        );
        fput(globals::message_out(), "\n");

        fput(
            globals::message_out(),
            "If OUTFILE or INFILE is missing or -, standard streams will be used.\nFMT can be { foma, openfst-tropical, openfst-log, sfst, \noptimized-lookup-weighted, optimized-lookup-unweighted }.\nIf EPS is not defined, the default representation of @0@ is used.\nOption --norm precedes option --log.\nThe FILE of option -m lists all multichar-symbols, each symbol\non its own line.\nBackslash '\\' may be used to escape ':', tab and itself. For any\nother symbol x '\\x' means x literally, i.e. is the same as 'x'.\nThe weight of a string can be given after the string separated\nby a tabulator. The weight cannot be zero.\n\n",
        );

        fput(
            globals::message_out(),
            &format!(
                "Examples:\n  echo \"cat:dog\" | {}            create cat:dog fst\n  echo \"c:da:ot:g\" | {} -p       same as pairstring\n  echo \"c:d a:o t:g\" | {} -p -S  same as pairstring with spaces\n  echo \"c a t:d o g\" | {} -S     same with spaces\n\n",
                program_name, program_name, program_name, program_name
            ),
        );
        print_report_bugs();
        fput(globals::message_out(), "\n");
        print_more_info();
        fput(globals::message_out(), "\n");
    }
}

// [spec:hfst:def:hfst-strings2fst.parse-options-fn]
// [spec:hfst:sem:hfst-strings2fst.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let disjunct_name = CString::new("disjunct-strings").unwrap();
            let epsilon_name = CString::new("epsilon").unwrap();
            let norm_name = CString::new("norm").unwrap();
            let log_name = CString::new("log").unwrap();
            let log10_name = CString::new("log10").unwrap();
            let pairstrings_name = CString::new("pairstrings").unwrap();
            let has_spaces_name = CString::new("has-spaces").unwrap();
            let multichar_name = CString::new("multichar-symbols").unwrap();
            let format_name = CString::new("format").unwrap();
            let wstuff_name = CString::new("Wstuff").unwrap();
            long_options.push(getopt::Option {
                name: disjunct_name.as_ptr(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 'j' as c_int,
            });
            long_options.push(getopt::Option {
                name: epsilon_name.as_ptr(),
                has_arg: 1,
                flag: std::ptr::null_mut(),
                val: 'e' as c_int,
            });
            long_options.push(getopt::Option {
                name: norm_name.as_ptr(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: '2' as c_int,
            });
            long_options.push(getopt::Option {
                name: log_name.as_ptr(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: '3' as c_int,
            });
            long_options.push(getopt::Option {
                name: log10_name.as_ptr(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: '4' as c_int,
            });
            long_options.push(getopt::Option {
                name: pairstrings_name.as_ptr(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 'p' as c_int,
            });
            long_options.push(getopt::Option {
                name: has_spaces_name.as_ptr(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 'S' as c_int,
            });
            long_options.push(getopt::Option {
                name: multichar_name.as_ptr(),
                has_arg: 1,
                flag: std::ptr::null_mut(),
                val: 'm' as c_int,
            });
            long_options.push(getopt::Option {
                name: format_name.as_ptr(),
                has_arg: 1,
                flag: std::ptr::null_mut(),
                val: 'f' as c_int,
            });
            long_options.push(getopt::Option {
                name: wstuff_name.as_ptr(),
                has_arg: 1,
                flag: std::ptr::null_mut(),
                val: 'W' as c_int,
            });
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}je:234pSm:f:W:",
                HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT
            ))
            .unwrap();
            let mut option_index: c_int = 0;
            let c = getopt::getopt_long(
                argc,
                argv,
                short.as_ptr(),
                long_options.as_ptr(),
                &mut option_index,
            );
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own, then the terminal
            // error arm.
            match handle_common_case(c, || print_usage()) {
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
                    EPSILONNAME = hfst_cli::hfst_commandline::hfst_strdup(getopt::OPTARG);
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
                    MULTICHAR_SYMBOL_FILENAME =
                        hfst_cli::hfst_commandline::hfst_strdup(getopt::OPTARG);
                    continue;
                }
                'f' => {
                    OUTPUT_FORMAT = hfst_parse_format_name(&cstr(getopt::OPTARG));
                    continue;
                }
                'W' => {
                    let optarg = cstr(getopt::OPTARG);
                    if optarg == "error" {
                        WARNINGS_ARE_ERRORS = true;
                    } else if optarg == "no-error" {
                        WARNINGS_ARE_ERRORS = false;
                    } else if optarg == "negative-weights" {
                        WARN_NEGATIVE_WEIGHTS = true;
                    } else if optarg == "no-negative-weights" {
                        WARN_NEGATIVE_WEIGHTS = false;
                    } else {
                        hfst_error(
                            libc::EXIT_FAILURE,
                            0,
                            &format!("unrecognised warning option -W{}", optarg),
                        );
                        return libc::EXIT_FAILURE;
                    }
                    continue;
                }
                _ => {}
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(argc, argv);
        if OUTPUT_FORMAT == ImplementationType::UNSPECIFIED_TYPE {
            verbose_printf("Output format not specified, defaulting to openfst tropical\n");
            OUTPUT_FORMAT = ImplementationType::TROPICAL_OPENFST_TYPE;
        }
        if EPSILONNAME.is_null() {
            let eps = CString::new("@0@").unwrap();
            EPSILONNAME = hfst_cli::hfst_commandline::hfst_strdup(eps.as_ptr());
        }
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-strings2fst.process-stream-fn]
// [spec:hfst:sem:hfst-strings2fst.process-stream-fn]
unsafe fn process_stream(outstream: &mut HfstOutputStream) -> c_int {
    unsafe {
        let mut transducer_n: usize = 0;
        let mut line: *mut c_char = std::ptr::null_mut();
        let mut len: usize = 0;
        let mut disjunction = HfstBasicTransducer::new();
        let mut line_n: usize = 0;

        let multichar_symbol_tokenizer = HfstStrings2FstTokenizer::new(
            &*std::ptr::addr_of!(MULTICHAR_SYMBOLS),
            &cstr(EPSILONNAME),
        );

        let inputfilename = cstr(globals::INPUTFILENAME);

        while hfst_getline(&mut line, &mut len, globals::inputfile()) != -1 {
            transducer_n += 1;
            line_n += 1;
            verbose_printf(&format!("Parsing line {}...\n", line_n));

            // parse line end and weight; mutate the C buffer in place.
            let line_bytes = CStr::from_ptr(line).to_bytes();
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
                // change trailing '\n'/'\r' (from tab onward) to '\0'
                let mut p = tab;
                while p < line_bytes.len() {
                    if line_bytes[p] == b'\n' || line_bytes[p] == b'\r' {
                        *line.add(p) = 0;
                    }
                    p += 1;
                }
                let weight_str = cstr(line.add(tab + 1));
                weight = hfst_strtoweight(&weight_str) as f64;
                weighted = true;
                let errm = format!(
                    "Found negative weight {:.6}; negative weights are supported but iffy, if you really need them use -Wno-negative-weights",
                    weight
                );
                if (weight < 0.0) && WARN_NEGATIVE_WEIGHTS {
                    if WARNINGS_ARE_ERRORS {
                        hfst_error_at_line(
                            libc::EXIT_FAILURE,
                            0,
                            &inputfilename,
                            line_n as u32,
                            &errm,
                        );
                    } else {
                        hfst_warning_at_line(0, 0, &inputfilename, line_n as u32, &errm);
                    }
                }
                string_end_idx = tab;
            }
            *line.add(string_end_idx) = 0;

            // Parse the string
            let parse_line = cstr(line);
            let pairstrings = PAIRSTRINGS;
            let has_spaces = HAS_SPACES;
            let tok_ref = &multichar_symbol_tokenizer;
            let pl = parse_line.clone();
            let spv: StringPairVector = match std::panic::catch_unwind(
                std::panic::AssertUnwindSafe(|| {
                    if pairstrings {
                        tok_ref.tokenize_pair_string(&pl, has_spaces)
                    } else {
                        tok_ref.tokenize_string_pair(&pl, has_spaces)
                    }
                }),
            ) {
                Ok(v) => v,
                Err(e) => {
                    if e.downcast_ref::<UnescapedColsFound>().is_some() {
                        if pairstrings {
                            error_at_line(
                                libc::EXIT_FAILURE,
                                errno(),
                                &inputfilename,
                                line_n as u32,
                                &format!(
                                    "String `{}' contains unescaped ':'-symbols,\nwhich are not pair separators. Use `\\:' for literal `:'.",
                                    parse_line
                                ),
                            );
                        } else {
                            error_at_line(
                                libc::EXIT_FAILURE,
                                errno(),
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
                            libc::EXIT_FAILURE,
                            errno(),
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
                verbose_printf(&format!("Using final weight {:.6}...\n", weight));
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
                let mut res = HfstTransducer::new_from_basic(&tr, OUTPUT_FORMAT);
                hfst_set_name(&mut res, "", "string");
                outstream.redirect(&mut res);
            } else {
                // disjunct all strings into a single transducer
                // do not take negative logarithm yet
                disjunction.disjunct_path(&spv, path_weight);
            }
        }
        if !line.is_null() {
            libc::free(line as *mut libc::c_void);
        }
        if DISJUNCT_STRINGS {
            let mut res = HfstTransducer::new_from_basic(&disjunction, OUTPUT_FORMAT);

            if NORMALIZE_WEIGHTS {
                verbose_printf("Normalising weights...\n");
                res.transform_weights(divide_by_sum_of_weights);
            }
            if LOGARITHMIC_WEIGHTS_E {
                verbose_printf("Taking negative logarithm...\n");
                res.transform_weights(take_negative_logarithm_e);
            } else if LOGARITHMIC_WEIGHTS_10 {
                verbose_printf("Taking negative logarithm...\n");
                res.transform_weights(take_negative_logarithm_10);
            }

            hfst_set_name(&mut res, "?", "strings");
            outstream.redirect(&mut res);
        }
        libc::EXIT_SUCCESS
    }
}

// [spec:hfst:def:hfst-strings2fst.main-fn]
// [spec:hfst:sem:hfst-strings2fst.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> c_int {
    unsafe {
        // Build a C-style argv (NULL-terminated) from the Rust args; getopt and
        // extend_options_getenv reorder/replace it in place.
        let c_args: Vec<CString> = std::env::args()
            .map(|a| CString::new(a).unwrap_or_default())
            .collect();
        let mut argv_vec: Vec<*mut c_char> =
            c_args.iter().map(|s| s.as_ptr() as *mut c_char).collect();
        argv_vec.push(std::ptr::null_mut());
        let argc: c_int = c_args.len() as c_int;
        let argv: *mut *mut c_char = argv_vec.as_mut_ptr();
        let argv0 = cstr(*argv);

        hfst_set_program_name(&argv0, "0.1", "Strings2Fst");
        let retval = parse_options(argc, argv);
        if retval != EXIT_CONTINUE {
            return retval;
        }

        if !MULTICHAR_SYMBOL_FILENAME.is_null() {
            let fname = cstr(MULTICHAR_SYMBOL_FILENAME);
            verbose_printf(&format!("Reading multichar symbols from {}\n", fname));
            match std::fs::read_to_string(&fname) {
                Ok(contents) => {
                    for multichar_line in contents.lines() {
                        if !multichar_line.is_empty() {
                            verbose_printf(&format!(
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
                        libc::EXIT_FAILURE,
                        errno(),
                        "Multichar symbol file can't be read.",
                    );
                }
            }
        }

        // close output buffers, we use output streams
        let output_opened = !globals::OUTFILE.is_null();
        if output_opened {
            libc::fclose(globals::OUTFILE);
        }
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            cstr(globals::INPUTFILENAME),
            cstr(globals::OUTFILENAME)
        ));
        // here starts the buffer handling part
        let mut outstream = if output_opened {
            HfstOutputStream::new_filename(&cstr(globals::OUTFILENAME), OUTPUT_FORMAT, true)
        } else {
            HfstOutputStream::new(OUTPUT_FORMAT, true)
        };
        process_stream(&mut outstream);
        libc::EXIT_SUCCESS
    }
}
