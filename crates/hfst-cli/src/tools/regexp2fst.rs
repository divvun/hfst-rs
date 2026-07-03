//! Faithful 1:1 port of tools/src/hfst-regexp2fst.cc — the regular expression
//! compiling command-line tool. Drives the hfst-cli foundation (globals,
//! getopt, commandline, program-options, tool-metadata, inc fragments) plus the
//! hfst XreCompiler.

use crate::globals;
use crate::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_from_env, hfst_error_at_line, hfst_parse_format_name,
    hfst_set_program_name, verbose_print,
};
use crate::hfst_getopt as getopt;
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

// File-scope tool state, mirroring the static globals in the C++ source.
static mut EPSILONNAME: Option<String> = None;
static mut DISJUNCT_EXPRESSIONS: bool = false;
static mut LINE_SEPARATED: bool = true;
static mut ENCODE_WEIGHTS: bool = false;
static mut OUTPUT_FORMAT: ImplementationType = ImplementationType::UNSPECIFIED_TYPE;
static mut HARMONIZE: bool = true;
static mut HARMONIZE_FLAGS: bool = false;
static mut MINIMIZE_RESULT: bool = true;
// '--xfst flag-is-epsilon' (was the 'flag_is_epsilon_in_composition' file-static
// global; now threaded into the XRE compiler via 'set_flag_is_epsilon').
static mut FLAG_IS_EPSILON: bool = false;
// '--xerox-composition' (was the 'xerox_composition' file-static global; now
// threaded into the XRE compiler via 'set_xerox_composition').
static mut XEROX_COMPOSITION: bool = false;

// [spec:hfst:def:hfst-regexp2fst.print-usage-fn]
// [spec:hfst:sem:hfst-regexp2fst.print-usage-fn]
fn print_usage() {
    let mut msg = globals::message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\n\
         Compile (weighted) regular expressions into transducer(s)\n",
        globals::program_name()
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
        globals::program_name()
    );
    let _ = write!(msg, "\n");
}

// [spec:hfst:def:hfst-regexp2fst.parse-options-fn]
// [spec:hfst:sem:hfst-regexp2fst.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_from_env(args);
        // use of this function requires options are settable on global scope
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
            match c as u8 as char {
                'e' => {
                    EPSILONNAME = Some(getopt::optarg());
                    continue;
                }
                'j' => {
                    DISJUNCT_EXPRESSIONS = true;
                    continue;
                }
                'S' => {
                    LINE_SEPARATED = false;
                    continue;
                }
                'l' => {
                    LINE_SEPARATED = true;
                    continue;
                }
                'f' => {
                    OUTPUT_FORMAT = hfst_parse_format_name(&getopt::optarg());
                    continue;
                }
                'H' => {
                    HARMONIZE = false;
                    continue;
                }
                'F' => {
                    HARMONIZE_FLAGS = true;
                    continue;
                }
                'E' => {
                    ENCODE_WEIGHTS = true;
                    continue;
                }
                'M' => {
                    MINIMIZE_RESULT = false;
                    continue;
                }
                'x' => {
                    let argument = getopt::optarg();
                    if argument == "yes" || argument == "true" || argument == "ON" {
                        XEROX_COMPOSITION = true;
                    } else if argument == "no" || argument == "false" || argument == "OFF" {
                        XEROX_COMPOSITION = false;
                    } else {
                        error(
                            1,
                            0,
                            &format!("unknown option to --xerox-composition: '{}'\n", argument),
                        );
                        return 1;
                    }
                    continue;
                }
                'X' => {
                    let argument = getopt::optarg();
                    if argument == "flag-is-epsilon" {
                        FLAG_IS_EPSILON = true;
                    } else {
                        error(
                            1,
                            0,
                            &format!("Error: unknown option to --xfst: '{}'\n", argument),
                        );
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
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-regexp2fst.process-stream-fn]
// [spec:hfst:sem:hfst-regexp2fst.process-stream-fn]
unsafe fn process_stream(outstream: &mut HfstOutputStream, input: &mut dyn BufRead) -> i32 {
    unsafe {
        let mut transducer_n: usize = 0;
        let mut line_count: u32 = 0;
        let mut comp = XreCompiler::new(OUTPUT_FORMAT);
        comp.set_verbosity(globals::VERBOSE);
        comp.set_error_stream(());
        comp.set_harmonization(HARMONIZE);
        comp.set_flag_harmonization(HARMONIZE_FLAGS);
        comp.set_minimize_result(MINIMIZE_RESULT);
        comp.set_flag_is_epsilon(FLAG_IS_EPSILON);
        comp.set_xerox_composition(XEROX_COMPOSITION);
        comp.set_encode_weights(ENCODE_WEIGHTS);
        let mut disjunction = match HfstTransducer::new_type(OUTPUT_FORMAT) {
            Ok(t) => t,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        };

        let mut first_line: Option<String> = None;

        if !LINE_SEPARATED {
            // C: read the whole input into a NUL-terminated buffer and walk it
            // with a char* cursor. Here we read it into a String and track a byte
            // offset; compile_first reports how many bytes it consumed.
            let mut content = String::new();
            let _ = input.read_to_string(&mut content);
            let mut offset: usize = 0;
            let mut chars_read: u32 = 0;

            loop {
                transducer_n += 1;
                verbose_print(&format!("Compiling expression #{}\n", transducer_n as i32));
                let remaining = &content[offset..];
                let compiled = comp.compile_first(remaining, &mut chars_read);
                // (the C wraps compile_first in try/catch on HfstException; the
                // Rust path currently panics rather than throwing, so the catch
                // arm that calls hfst_error is not reproduced here.)
                if compiled.is_none() {
                    if comp.contained_only_comments() {
                        if transducer_n == 1 {
                            error(
                                1,
                                0,
                                &format!(
                                    "{}: XRE parsing failed: expression #{} \
                                     contains only whitespace or comments",
                                    globals::input_filename(),
                                    transducer_n as u32
                                ),
                            );
                        }
                        break;
                    } else {
                        error(
                            1,
                            0,
                            &format!(
                                "{}: XRE parsing failed \
                                 in expression #{} separated by semicolons",
                                globals::input_filename(),
                                transducer_n as u32
                            ),
                        );
                    }
                }
                offset += chars_read as usize;
                if let Some(mut compiled) = compiled {
                    if DISJUNCT_EXPRESSIONS {
                        if let Err(e) = disjunction.disjunct(&compiled, HARMONIZE) {
                            error(1, 0, &format!("{e}"));
                            return 1;
                        }
                    } else {
                        hfst_set_name(&mut compiled, "?", "xre");
                        if let Err(e) = outstream.redirect(&mut compiled) {
                            error(1, 0, &format!("{e}"));
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
                            1,
                            0,
                            &format!(
                                "{}: XRE parsing failed: \
                                 input contains only whitespace or comments",
                                globals::input_filename()
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
                    verbose_print(&format!("Skipping whitespace expression #{}", line_count));
                    continue;
                }
                transducer_n += 1;
                verbose_print(&format!("Compiling expression {}\n", line_count));
                let compiled = comp.compile(&exp);
                // (the C wraps compile in try/catch on HfstException calling
                // hfst_error_at_line; the Rust path panics rather than throwing,
                // so the catch arm is not reproduced here.)
                let Some(mut compiled) = compiled else {
                    if !comp.contained_only_comments() {
                        hfst_error_at_line(
                            1,
                            0,
                            &globals::input_filename(),
                            line_count,
                            "XRE parsing failed\n",
                        );
                    }
                    continue;
                };
                input_contains_only_whitespace_or_comments = false;

                if DISJUNCT_EXPRESSIONS {
                    if let Err(e) = disjunction.disjunct(&compiled, HARMONIZE) {
                        error(1, 0, &format!("{e}"));
                        return 1;
                    }
                } else {
                    hfst_set_name(&mut compiled, "?", "xre");
                    if let Err(e) = outstream.redirect(&mut compiled) {
                        error(1, 0, &format!("{e}"));
                        return 1;
                    }
                }
                // C: delete compiled; -> owned, drops here.
            }
        }

        if DISJUNCT_EXPRESSIONS {
            // Both branches of the C++ if/else set the same name.
            hfst_set_name(&mut disjunction, "?", "xre");
            if let Err(e) = outstream.redirect(&mut disjunction) {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        }
        // C: free(line); free(first_line); -> owned String/Option, drop here.
        drop(first_line);
        0
    }
}

// [spec:hfst:def:hfst-regexp2fst.main-fn]
// [spec:hfst:sem:hfst-regexp2fst.main-fn]
pub fn run(args: Vec<String>) -> i32 {
    unsafe { real_main(args) }
}

unsafe fn real_main(mut args: Vec<String>) -> i32 {
    unsafe {
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.2", "Regexp2Fst");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        if globals::DEBUG {
            // xredebug = 1;
        }

        // close buffers, we use streams
        let output_opened = globals::output_filename() != "<stdout>";
        verbose_print(&format!(
            "Reading from {}, writing to {}\n",
            globals::input_filename(),
            globals::output_filename()
        ));
        // here starts the buffer handling part
        let mut outstream = match if output_opened {
            HfstOutputStream::new_filename(&globals::output_filename(), OUTPUT_FORMAT, true)
        } else {
            HfstOutputStream::new(OUTPUT_FORMAT, true)
        } {
            Ok(s) => s,
            Err(e) => {
                eprintln!("hfst-regexp2fst: cannot open output: {e}");
                return 1;
            }
        };
        let mut input = match globals::input_reader() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("hfst-regexp2fst: cannot open input: {e}");
                return 1;
            }
        };
        process_stream(&mut outstream, &mut *input);

        0
    }
}
