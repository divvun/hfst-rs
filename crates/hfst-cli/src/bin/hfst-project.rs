//! Faithful 1:1 port of tools/src/hfst-project.cc — the transducer projection
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).

use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_from_env, hfst_set_program_name,
    is_input_stream_in_ol_format, print_more_info, print_report_bugs, verbose_print,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use hfst_cli::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary, hfst_set_name_unary};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use std::io::Write;

// add tools-specific variables here
static mut PROJECT_INPUT: bool = false;

// strncasecmp(optarg, prefix, 1) == 0 — case-insensitive comparison of the
// first byte only (the C calls always pass length 1).
fn first_char_matches(optarg: &Option<String>, prefix: &str) -> bool {
    match optarg.as_ref().and_then(|s| s.bytes().next()) {
        Some(first) => {
            let want = prefix.as_bytes()[0];
            first.to_ascii_lowercase() == want.to_ascii_lowercase()
        }
        None => false,
    }
}

// [spec:hfst:def:hfst-project.print-usage-fn]
// [spec:hfst:sem:hfst-project.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nProject (extract a level) transducer\n\n",
        globals::program_name()
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Projection options:\n  -p, --project=LEVEL   project extracting tape LEVEL\n"
    );
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(
        msg,
        "LEVEL must be one of upper, input, first, analysis or lower, output, second, generation\n"
    );
    let _ = write!(msg, "\n");
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-project.parse-options-fn]
// [spec:hfst:sem:hfst-project.parse-options-fn]
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
                name: "project",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: 'p' as i32,
            });
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own 'p', then the
            // terminal error arm.
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
            if c == 'p' as i32 {
                let optarg = getopt::optarg_opt();
                if first_char_matches(&optarg, "upper")
                    || first_char_matches(&optarg, "input")
                    || first_char_matches(&optarg, "first")
                    || first_char_matches(&optarg, "analysis")
                {
                    PROJECT_INPUT = true;
                } else if first_char_matches(&optarg, "lower")
                    || first_char_matches(&optarg, "output")
                    || first_char_matches(&optarg, "second")
                    || first_char_matches(&optarg, "generation")
                {
                    PROJECT_INPUT = false;
                } else {
                    error(
                        1,
                        0,
                        &format!(
                            "unknown project direction {}\nshould be one of upper, input, analysis, first, lower, output, second or generation\n",
                            getopt::optarg()
                        ),
                    );
                    return 1;
                }
                continue;
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(args);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-project.process-stream-fn]
// [spec:hfst:sem:hfst-project.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream, outstream: &mut HfstOutputStream) -> i32 {
    unsafe {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let mut trans = match HfstTransducer::new_from_stream(instream) {
                Ok(t) => t,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            };
            let inputname = hfst_get_name(&trans, &globals::input_filename());
            if transducer_n == 1 {
                if PROJECT_INPUT {
                    verbose_print(&format!("Projecting first {}...\n", inputname));
                } else {
                    verbose_print(&format!("Projecting second {}...\n", inputname));
                }
            } else if PROJECT_INPUT {
                verbose_print(&format!(
                    "Projecting first {}... {}\n",
                    inputname, transducer_n
                ));
            } else {
                verbose_print(&format!(
                    "Projecting second {}... {}\n",
                    inputname, transducer_n
                ));
            }

            if PROJECT_INPUT {
                if let Err(e) = trans.input_project() {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
                // C: hfst_set_name(trans, trans, ...); the dest and src are the
                // same object, which Rust cannot alias mut+const, so the read
                // side is taken from a copy (name/formula unchanged by copy).
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "project-1st");
                hfst_set_formula_unary(&mut trans, &src, "\u{00b9}");
            } else {
                if let Err(e) = trans.output_project() {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "project-2nd");
                hfst_set_formula_unary(&mut trans, &src, "\u{00b2}");
            }
            if let Err(e) = outstream.redirect(&mut trans) {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        }
        instream.close();
        outstream.close();
        0
    }
}

// [spec:hfst:def:hfst-project.main-fn]
// [spec:hfst:sem:hfst-project.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstProject");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let input_opened = globals::input_filename() != "<stdin>";
        let output_opened = globals::output_filename() != "<stdout>";
        verbose_print(&format!(
            "Reading from {}, writing to {}\n",
            globals::input_filename(),
            globals::output_filename()
        ));

        // here starts the buffer handling part
        let mut instream = match if input_opened {
            HfstInputStream::new_filename(&globals::input_filename())
        } else {
            HfstInputStream::new()
        } {
            Ok(s) => s,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        };
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)

        let ty = instream.get_type();
        let mut outstream = match if output_opened {
            HfstOutputStream::new_filename(&globals::output_filename(), ty, true)
        } else {
            HfstOutputStream::new(ty, true)
        } {
            Ok(s) => s,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        };

        if is_input_stream_in_ol_format(&instream, "hfst-project") {
            return 1;
        }

        process_stream(&mut instream, &mut outstream)
    }
}
