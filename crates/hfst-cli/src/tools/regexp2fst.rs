//! Faithful 1:1 port of tools/src/hfst-regexp2fst.cc — the regular expression
//! compiling command-line tool. Drives the hfst-cli foundation (globals,
//! getopt, commandline, program-options, tool-metadata, inc fragments) plus the
//! hfst XreCompiler.
//!
//! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
//! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
//! `parse_options` and threaded into the processing functions. There are no
//! `static mut` globals and no `unsafe`.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, extend_options_from_env, hfst_error_at_line, hfst_parse_format_name,
    hfst_set_program_name, redirect_converting, verbose_print,
};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options,
};
use crate::hfst_tool_metadata::hfst_set_name;
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst::xre::XreCompiler;
use std::io::{BufRead, Write};

/// hfst-regexp2fst's own options (the former tool-specific `static mut`s).
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

impl Default for Options {
    fn default() -> Options {
        Options {
            epsilonname: None,
            disjunct_expressions: false,
            line_separated: true,
            encode_weights: false,
            output_format: ImplementationType::UNSPECIFIED_TYPE,
            harmonize: true,
            harmonize_flags: false,
            minimize_result: true,
            flag_is_epsilon: false,
            xerox_composition: false,
        }
    }
}

// [spec:hfst:def:hfst-regexp2fst.print-usage-fn]
// [spec:hfst:sem:hfst-regexp2fst.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    let mut msg = common.message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\n\
         Compile (weighted) regular expressions into transducer(s)\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "String and format options:\n\
         \x20 -f, --format=FMT          Write result in FMT format\n\
         \x20 -j, --disjunct            Disjunct all regexps instead of transforming\n\
         \x20                           each regexp into a separate transducer\n\
         \x20 -l, --line                Input is line separated (default)\n\
         \x20 -S, --semicolon           Input is semicolon separated\n\
         \x20 -e, --epsilon=EPS         Map EPS as zero, i.e. epsilon.\n\
         \x20 -x, --xerox-composition=VALUE Whether flag diacritics are treated as ordinary\n\
         \x20                               symbols in composition (default is false).\n\
         \x20 -X, --xfst=VARIABLE       Toggle xfst compatibility option VARIABLE.\n\
         Harmonization and optimization options:\n\
         \x20 -H, --do-not-harmonize    Do not expand '?' symbols.\n\
         \x20 -F, --harmonize-flags     Harmonize flag diacritics.\n\
         \x20 -E, --encode-weights      Encode weights when minimizing (default is false).\n\
         \x20 -M, --do-not-minimize     Determinize result instead of minimizing it.\n"
    );
    let _ = write!(msg, "\n");

    let _ = write!(
        msg,
        "If OUTFILE or INFILE is missing or -, standard streams will be used.\n\
         FMT must be one of the following: \
         {{foma, sfst, openfst-tropical, openfst-log}}.\n\
         If EPS is not defined, the default representation of 0 is used\n\
         VALUEs recognized are {{true,ON,yes}} and {{false,OFF,no}}.\n\
         Xfst variables are {{flag-is-epsilon (default OFF)}}.\n\
         \n"
    );

    let _ = write!(
        msg,
        "Examples:\n\
         \x20 echo \" {{cat}}:{{dog}} \" | {0}       create transducer {{cat}}:{{dog}}\n\
         \x20 echo \" {{cat}}:{{dog}}::3 \" | {0}    same but with weight 3\n\
         \x20 echo \" c:d a:o::3 t:g \" | {0}    same but with weight 3\n\
         \x20                                             in the middle\n\
         \x20 echo \" cat ; dog ; \"3\" \" | {0} -S  create transducers\n\
         \x20                                             \"cat\" and \"dog\" and \"3\"\n\
         \n",
        common.program_name
    );
    let _ = write!(msg, "\n");
}

// [spec:hfst:def:hfst-regexp2fst.parse-options-fn]
// [spec:hfst:sem:hfst-regexp2fst.parse-options-fn]
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
        let tool_opts: [(&'static str, i32, i32); 11] = [
            ("disjunct", getopt::NO_ARGUMENT, 'j' as i32),
            ("epsilon", getopt::REQUIRED_ARGUMENT, 'e' as i32),
            ("line", getopt::NO_ARGUMENT, 'l' as i32),
            ("semicolon", getopt::NO_ARGUMENT, 'S' as i32),
            ("format", getopt::REQUIRED_ARGUMENT, 'f' as i32),
            ("do-not-harmonize", getopt::NO_ARGUMENT, 'H' as i32),
            ("harmonize-flags", getopt::NO_ARGUMENT, 'F' as i32),
            ("encode-weights", getopt::NO_ARGUMENT, 'E' as i32),
            ("xerox-composition", getopt::REQUIRED_ARGUMENT, 'x' as i32),
            ("xfst", getopt::REQUIRED_ARGUMENT, 'X' as i32),
            ("do-not-minimize", getopt::NO_ARGUMENT, 'M' as i32),
        ];
        for (name, has_arg, val) in tool_opts {
            long_options.push(getopt::GetOpt { name, has_arg, val });
        }
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
        match c as u8 as char {
            'e' => {
                options.epsilonname = Some(opt.optarg());
                continue;
            }
            'j' => {
                options.disjunct_expressions = true;
                continue;
            }
            'S' => {
                options.line_separated = false;
                continue;
            }
            'l' => {
                options.line_separated = true;
                continue;
            }
            'f' => {
                options.output_format = hfst_parse_format_name(&common, &opt.optarg());
                continue;
            }
            'H' => {
                options.harmonize = false;
                continue;
            }
            'F' => {
                options.harmonize_flags = true;
                continue;
            }
            'E' => {
                options.encode_weights = true;
                continue;
            }
            'M' => {
                options.minimize_result = false;
                continue;
            }
            'x' => {
                let argument = opt.optarg();
                if argument == "yes" || argument == "true" || argument == "ON" {
                    options.xerox_composition = true;
                } else if argument == "no" || argument == "false" || argument == "OFF" {
                    options.xerox_composition = false;
                } else {
                    error(
                        &common,
                        1,
                        0,
                        &format!("unknown option to --xerox-composition: '{}'\n", argument),
                    );
                    return Err(1);
                }
                continue;
            }
            'X' => {
                let argument = opt.optarg();
                if argument == "flag-is-epsilon" {
                    options.flag_is_epsilon = true;
                } else {
                    error(
                        &common,
                        1,
                        0,
                        &format!("Error: unknown option to --xfst: '{}'\n", argument),
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
    Ok((common, options))
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
        ImplementationType::LOG_OPENFST_TYPE => process_stream_typed::<
            hfst::log_weight_transducer::LogFst,
        >(common, options, outstream, input),
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
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.2", "Regexp2Fst");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };
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
            return 1;
        }
    };
    let mut input = match common.input_reader() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("hfst-regexp2fst: cannot open input: {e}");
            return 1;
        }
    };
    process_stream(&common, &options, &mut outstream, &mut *input);

    0
}
