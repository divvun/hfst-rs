//! Faithful 1:1 port of tools/src/hfst-compose-intersect.cc — the
//! compose-intersect command-line tool (compose a lexicon with one or more
//! rule transducers). Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments). This is a
//! BINARY tool: it reads a first stream (the lexicon) and a second stream
//! (the rule file).

use hfst::convert_transducer_format::ConversionFunctions;
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_symbol_defs::internal_identity;
use hfst::hfst_tokenizer::HfstTokenizer;
use hfst::hfst_transducer::{HfstTransducer, HfstTransducerVector};
use hfst::hfst_transducer::{get_encode_weights, set_encode_weights};
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, conversion_type, error, extend_options_getenv, hfst_set_program_name,
    hfst_strformat, is_input_stream_in_ol_format, print_more_info, print_report_bugs,
    verbose_printf, warning,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    HFST_GETOPT_BINARY_SHORT, HFST_GETOPT_COMMON_SHORT, hfst_getopt_binary_long,
    hfst_getopt_common_long, print_common_binary_program_options, print_common_program_options,
};
use hfst_cli::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary};
use hfst_cli::inc::{
    CaseResult, check_binary_params, check_common_params, handle_binary_case, handle_common_case,
    handle_error_case,
};
use libc::{c_char, c_int};
use std::ffi::{CStr, CString};

// static bool insert_missing_flags=false;

// If invert is true, the intersection of the rules is composed with the
// lexicon. Otherwise the lexicon is composed with the intersection of the
// rules.
static mut INVERT: bool = false;
static mut ENCODE_WEIGHTS: bool = false;
static mut FAST_CI: bool = false;
static mut HARMONIZE: bool = false;

unsafe extern "C" {
    #[cfg_attr(target_os = "macos", link_name = "__stdinp")]
    static mut stdin_ptr: *mut libc::FILE;
    #[cfg_attr(target_os = "macos", link_name = "__stdoutp")]
    static mut stdout_ptr: *mut libc::FILE;
}

fn stdin_file() -> *mut libc::FILE {
    unsafe { stdin_ptr }
}
fn stdout_file() -> *mut libc::FILE {
    unsafe { stdout_ptr }
}

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

// [spec:hfst:def:hfst-compose-intersect.print-usage-fn]
// [spec:hfst:sem:hfst-compose-intersect.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            globals::message_out(),
            &format!(
                "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\n\
                 Compose a lexicon with one or more rule transducers.\n\n",
                program_name
            ),
        );
        print_common_program_options(globals::message_out());
        print_common_binary_program_options(globals::message_out());
        fput(
            globals::message_out(),
            "Composition options:\n\
             \x20 -I, --invert                 Compose the intersection of the\n\
             \x20                              rules with the lexicon instead\n\
             \x20                              of composing the lexicon with\n\
             \x20                              the intersection of the rules.\n\
             \x20 -f, --fast                   Faster compose instersect using\n\
             \x20                              more memory.\n\
             \x20 -e, --encode-weights         Encode weights when minimizing\n\
             \x20                              (default is false).\n\
             \x20 -a, --harmonize              Harmonize symbols.\n",
        );
        // print_common_binary_program_parameter_instructions(message_out);
        fput(
            globals::message_out(),
            "\nIf OUTFILE, or either INFILE1 or INFILE2 is missing or -, standard\n\
             streams will be used. INFILE1, INFILE2, or both, must be specified\n\
             The format of INFILE1 and INFILE2 must be the same; the result will\n\
             have the same format as these.\n\
             INFILE1 (the lexicon) must contain exactly one transducer.\n\
             INFILE2 (rule file) may contain several transducers.\n",
        );
        fput(
            globals::message_out(),
            &format!(
                "\nExamples:\n\
                 \x20 {} -o analyzer.hfst lexicon.hfst rules.hfst\n\
                 compose rules with lexicon\n\n",
                program_name
            ),
        );
        print_report_bugs();
        fput(globals::message_out(), "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-compose-intersect.parse-options-fn]
// [spec:hfst:sem:hfst-compose-intersect.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_binary_long());
            long_options.push(getopt::Option {
                name: CString::new("invert").unwrap().into_raw() as *const c_char,
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: b'I' as c_int,
            });
            long_options.push(getopt::Option {
                name: CString::new("encode-weights").unwrap().into_raw() as *const c_char,
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: b'e' as c_int,
            });
            long_options.push(getopt::Option {
                name: CString::new("fast").unwrap().into_raw() as *const c_char,
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: b'f' as c_int,
            });
            long_options.push(getopt::Option {
                name: CString::new("harmonize").unwrap().into_raw() as *const c_char,
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: b'a' as c_int,
            });
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}FIeHfa",
                HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_BINARY_SHORT
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

            // The C switch chains the #include'd case groups in order: binary
            // cases, common cases, the terminal error arm, then the tool's own
            // cases. The tool-specific cases must be tried before the error arm
            // falls through, so we test them ahead of handle_error_case.
            match handle_binary_case(c) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_common_case(c, || print_usage()) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            if c == b'I' as c_int {
                INVERT = true;
                continue;
            } else if c == b'e' as c_int {
                ENCODE_WEIGHTS = true;
                continue;
            } else if c == b'f' as c_int {
                FAST_CI = true;
                continue;
            } else if c == b'a' as c_int {
                HARMONIZE = true;
                continue;
            }
            return handle_error_case(c);
        }

        check_binary_params(argc, argv);
        check_common_params();
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-compose-intersect.string-set]
// (typedef std::set<std::string> StringSet → std::collections::BTreeSet<String>)

// [spec:hfst:def:hfst-compose-intersect.is-special-symbol-fn]
// [spec:hfst:sem:hfst-compose-intersect.is-special-symbol-fn]
fn is_special_symbol(symbol: &str) -> bool {
    let bytes = symbol.as_bytes();
    symbol.len() > 2 && bytes[0] == b'@' && bytes[symbol.len() - 1] == b'@'
}

// [spec:hfst:def:hfst-compose-intersect.check-all-symbols-fn]
// [spec:hfst:sem:hfst-compose-intersect.check-all-symbols-fn]
fn check_all_symbols(lexicon: &HfstTransducer, rule: &HfstTransducer) -> String {
    let rule_b = unsafe {
        *Box::from_raw(ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(rule))
    };

    let mut rule_input_symbols: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();

    for s in 0..=rule_b.get_max_state() {
        for it in rule_b.transitions(s).iter() {
            let input_symbol = it.get_input_symbol();
            rule_input_symbols.insert(input_symbol);
        }
    }

    if rule_input_symbols.contains(internal_identity) {
        return String::new();
    }

    let lexicon_b = unsafe {
        *Box::from_raw(ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(lexicon))
    };

    for s in 0..=lexicon_b.get_max_state() {
        for it in lexicon_b.transitions(s).iter() {
            let output_symbol = it.get_output_symbol();

            if !rule_input_symbols.contains(&output_symbol) {
                return output_symbol;
            }
        }
    }

    String::new()
}

// [spec:hfst:def:hfst-compose-intersect.check-multi-char-symbols-fn]
// [spec:hfst:sem:hfst-compose-intersect.check-multi-char-symbols-fn]
fn check_multi_char_symbols(lexicon: &HfstTransducer, rule: &HfstTransducer) -> String {
    let lexicon_b = unsafe {
        *Box::from_raw(ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(lexicon))
    };
    let rule_b = unsafe {
        *Box::from_raw(ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(rule))
    };

    let tokenizer = HfstTokenizer::new();

    let mut rule_input_symbols: std::collections::BTreeSet<String> =
        std::collections::BTreeSet::new();

    for s in 0..=rule_b.get_max_state() {
        for it in rule_b.transitions(s).iter() {
            let input_symbol = it.get_input_symbol();
            rule_input_symbols.insert(input_symbol);
        }
    }

    for s in 0..=lexicon_b.get_max_state() {
        for it in lexicon_b.transitions(s).iter() {
            let output_symbol = it.get_output_symbol();

            if !rule_input_symbols.contains(&output_symbol) {
                if is_special_symbol(&output_symbol) {
                    continue;
                }

                if tokenizer.tokenize_one_level(&output_symbol, false).len() > 1 {
                    return output_symbol;
                }
            }
        }
    }

    String::new()
}

// [spec:hfst:def:hfst-compose-intersect.harmonize-rules-fn]
// [spec:hfst:sem:hfst-compose-intersect.harmonize-rules-fn]
fn harmonize_rules(lexicon: &mut HfstTransducer, rules: &mut [HfstTransducer]) {
    for it in rules.iter_mut() {
        it.harmonize(lexicon, false);
    }
}

// [spec:hfst:def:hfst-compose-intersect.compose-streams-fn]
// [spec:hfst:sem:hfst-compose-intersect.compose-streams-fn]
unsafe fn compose_streams(
    firststream: &mut HfstInputStream,
    secondstream: &mut HfstInputStream,
) -> c_int {
    unsafe {
        // there must be at least one transducer in both input streams
        let type1 = firststream.get_type();
        let type2 = secondstream.get_type();
        let mut output_type = ImplementationType::UNSPECIFIED_TYPE;
        if type1 != type2 {
            if globals::ALLOW_TRANSDUCER_CONVERSION {
                let ct = conversion_type(type1, type2);
                let mut warnstr = format!(
                    "Transducer type mismatch in {} and {}; ",
                    cstr(globals::FIRSTFILENAME),
                    cstr(globals::SECONDFILENAME)
                );
                if ct == 1 {
                    warnstr.push_str("using former type as output");
                    output_type = type1;
                } else if ct == 2 {
                    warnstr.push_str("using latter type as output");
                    output_type = type2;
                } else if ct == -1 {
                    warnstr
                        .push_str("using former type as output, loss of information is possible");
                    output_type = type1;
                } else {
                    // should not happen
                    std::panic::panic_any(
                        "Error: hfst-compose-intersect: conversion_type returned an invalid integer",
                    );
                }
                warning(0, 0, &warnstr);
            } else {
                error(
                    libc::EXIT_FAILURE,
                    0,
                    &format!(
                        "Transducer type mismatch in {} and {}; \
                         formats {} and {} are not compatible for compose-intersect \
                         (--do-not-convert was requested)",
                        cstr(globals::FIRSTFILENAME),
                        cstr(globals::SECONDFILENAME),
                        hfst_strformat(type1),
                        hfst_strformat(type2)
                    ),
                );
            }
        } else {
            output_type = type1;
        }

        let mut outstream = if globals::outfile() != stdout_file() {
            HfstOutputStream::new_filename(&cstr(globals::OUTFILENAME), output_type, true)
        } else {
            HfstOutputStream::new(output_type, true)
        };

        let _both_inputs = firststream.is_good() && secondstream.is_good();

        if is_input_stream_in_ol_format(firststream, "hfst-compose-intersect")
            || is_input_stream_in_ol_format(secondstream, "hfst-compose-intersect")
        {
            return libc::EXIT_FAILURE;
        }

        let mut rules: HfstTransducerVector = Vec::new();
        let mut rule_n: usize = 1;

        while secondstream.is_good() {
            let mut rule = HfstTransducer::new_from_stream(secondstream);
            rule.convert(output_type, String::new());
            let rulename = rule.get_name();
            if rulename.len() > 0 {
                verbose_printf(&format!("Reading and minimizing rule {}...\n", rulename));
            } else {
                verbose_printf(&format!("Reading and minimizing rule {}...\n", rule_n));
            }
            let enc = get_encode_weights();
            if ENCODE_WEIGHTS {
                set_encode_weights(true);
            }
            rule.minimize();
            if ENCODE_WEIGHTS {
                set_encode_weights(enc);
            }

            rules.push(rule);
            rule_n += 1;
        }

        while firststream.is_good() {
            verbose_printf("Reading lexicon...");
            let mut lexicon = HfstTransducer::new_from_stream(firststream);
            lexicon.convert(output_type, String::new());
            let lexiconname = hfst_get_name(&lexicon, &cstr(globals::FIRSTFILENAME));
            verbose_printf(&format!(" {} read\n", lexiconname));

            verbose_printf("Computing intersecting composition...\n");

            if rules.len() > 0 {
                let symbol = check_all_symbols(&lexicon, &rules[0]);
                if symbol != "" {
                    warning(
                        0,
                        0,
                        &format!(
                            "\nFound output symbols (e.g. \"{}\") in transducer in\n\
                             file {} which will be filtered out because they are\n\
                             not found on the input tapes of transducers in file\n\
                             {}.",
                            symbol,
                            cstr(globals::FIRSTFILENAME),
                            cstr(globals::SECONDFILENAME)
                        ),
                    );
                } else {
                    let symbol = check_multi_char_symbols(&lexicon, &rules[0]);
                    if symbol != "" {
                        warning(
                            0,
                            0,
                            &format!(
                                "\nFound output multi-char symbols (\"{}\") in \n\
                                 transducer in file {} which are not found on the\n\
                                 input tapes of transducers in file {}.",
                                symbol,
                                cstr(globals::FIRSTFILENAME),
                                cstr(globals::SECONDFILENAME)
                            ),
                        );
                    }
                }
            }

            if HARMONIZE {
                harmonize_rules(&mut lexicon, &mut rules);
            }

            if FAST_CI {
                // To hopefully speed up stuff: Compose intersect the output
                // of the lexicon with the rules and then compose the original
                // lexicon with the result.

                if INVERT {
                    let mut lexicon_input = lexicon.clone();
                    lexicon_input.input_project().minimize();
                    lexicon_input.compose_intersect(&rules, true, true);

                    lexicon_input.compose(&lexicon, true);
                    lexicon = lexicon_input;
                } else {
                    let mut lexicon_output = lexicon.clone();
                    lexicon_output.output_project().minimize();
                    lexicon_output.compose_intersect(&rules, false, true);
                    lexicon.compose(&lexicon_output, true);
                }
            } else {
                lexicon.compose_intersect(&rules, INVERT, true);
            }

            let composed_name = format!(
                "compose({}, intersect({}))",
                lexiconname,
                cstr(globals::SECONDFILENAME)
            );
            lexicon.set_name(&composed_name);
            let src = lexicon.clone();
            hfst_set_formula_unary(&mut lexicon, &src, " \u{2218} \u{22c2}R");

            verbose_printf(&format!(
                "Storing result in {}...\n",
                cstr(globals::OUTFILENAME)
            ));
            outstream.redirect(&mut lexicon);
        }

        firststream.close();
        secondstream.close();
        outstream.close();
        libc::EXIT_SUCCESS
    }
}

// [spec:hfst:def:hfst-compose-intersect.main-fn]
// [spec:hfst:sem:hfst-compose-intersect.main-fn]
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

        hfst_set_program_name(&argv0, "0.1", "HfstComposeIntersect");
        let retval = parse_options(argc, argv);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        if globals::firstfile() != stdin_file() {
            libc::fclose(globals::firstfile());
        }
        if globals::secondfile() != stdin_file() {
            libc::fclose(globals::secondfile());
        }
        if globals::outfile() != stdout_file() {
            libc::fclose(globals::outfile());
        }
        verbose_printf(&format!(
            "Reading from {} and {}, writing to {}\n",
            cstr(globals::FIRSTFILENAME),
            cstr(globals::SECONDFILENAME),
            cstr(globals::OUTFILENAME)
        ));
        // here starts the buffer handling part
        // (the C wraps the ctors in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)
        let mut firststream = if globals::firstfile() != stdin_file() {
            HfstInputStream::new_filename(&cstr(globals::FIRSTFILENAME))
        } else {
            HfstInputStream::new()
        };
        let mut secondstream = if globals::secondfile() != stdin_file() {
            HfstInputStream::new_filename(&cstr(globals::SECONDFILENAME))
        } else {
            HfstInputStream::new()
        };

        compose_streams(&mut firststream, &mut secondstream)
    }
}
