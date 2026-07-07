//! Faithful 1:1 port of tools/src/hfst-project.cc — the transducer projection
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, extend_options_from_env, hfst_set_program_name, is_input_stream_in_ol_format,
    verbose_print,
};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use crate::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary, hfst_set_name_unary};
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use std::io::Write;

/// hfst-project's own options (the former tool-specific `static mut`s).
#[derive(Default)]
struct Options {
    /// '-p, --project=LEVEL': project extracting the input (first) tape when
    /// true, the output (second) tape when false.
    project_input: bool,
}

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
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nProject (extract a level) transducer\n\n",
        common.program_name
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
}

// [spec:hfst:def:hfst-project.parse-options-fn]
// [spec:hfst:sem:hfst-project.parse-options-fn]
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
            name: "project",
            has_arg: getopt::REQUIRED_ARGUMENT,
            val: 'p' as i32,
        });
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: common
        // cases, then unary cases, then the tool's own 'p', then the
        // terminal error arm.
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
        if c == 'p' as i32 {
            let optarg = opt.optarg_opt();
            if first_char_matches(&optarg, "upper")
                || first_char_matches(&optarg, "input")
                || first_char_matches(&optarg, "first")
                || first_char_matches(&optarg, "analysis")
            {
                options.project_input = true;
            } else if first_char_matches(&optarg, "lower")
                || first_char_matches(&optarg, "output")
                || first_char_matches(&optarg, "second")
                || first_char_matches(&optarg, "generation")
            {
                options.project_input = false;
            } else {
                error(
                    &common,
                    1,
                    0,
                    &format!(
                        "unknown project direction {}\nshould be one of upper, input, analysis, first, lower, output, second or generation\n",
                        opt.optarg()
                    ),
                );
                return Err(1);
            }
            continue;
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    check_common_params(&mut common);
    check_unary_params(&mut common, &opt, args);
    Ok((common, options))
}

// [spec:hfst:def:hfst-project.process-stream-fn]
// [spec:hfst:sem:hfst-project.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &Options,
    instream: &mut HfstInputStream<'_>,
    outstream: &mut HfstOutputStream,
) -> i32 {
    let mut transducer_n: usize = 0;
    while instream.is_good() {
        transducer_n += 1;
        let any = match instream.read() {
            Ok(v) => v,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
        crate::for_algebra!(any, trans => {
            let mut trans = trans;
            let inputname = hfst_get_name(&trans, &common.input_filename);
            if transducer_n == 1 {
                if options.project_input {
                    verbose_print(common, &format!("Projecting first {}...\n", inputname));
                } else {
                    verbose_print(common, &format!("Projecting second {}...\n", inputname));
                }
            } else if options.project_input {
                verbose_print(common, &format!(
                    "Projecting first {}... {}\n",
                    inputname, transducer_n
                ));
            } else {
                verbose_print(common, &format!(
                    "Projecting second {}... {}\n",
                    inputname, transducer_n
                ));
            }

            if options.project_input {
                if let Err(e) = trans.input_project() {
                    error(common, 1, 0, &format!("{e}"));
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
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "project-2nd");
                hfst_set_formula_unary(&mut trans, &src, "\u{00b2}");
            }
            if let Err(e) = outstream.redirect(&mut trans) {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        }, else => {
            // Unreachable: the optimized-lookup stream rejection already
            // returned before the loop; keep its text for safety.
            let _ = write!(
                std::io::stderr(),
                "Error: hfst-project cannot process transducers that are in optimized lookup format.\n"
            );
            return 1;
        });
    }
    instream.close();
    outstream.close();
    0
}

// [spec:hfst:def:hfst-project.main-fn]
// [spec:hfst:sem:hfst-project.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstProject");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    // close buffers, we use streams
    let input_opened = common.input_filename != "<stdin>";
    let output_opened = common.output_filename != "<stdout>";
    verbose_print(
        &common,
        &format!(
            "Reading from {}, writing to {}\n",
            common.input_filename, common.output_filename
        ),
    );

    // here starts the buffer handling part
    let mut instream = match if input_opened {
        HfstInputStream::new_filename(&common.input_filename)
    } else {
        HfstInputStream::new()
    } {
        Ok(s) => s,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return 1;
        }
    };
    // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
    // currently panics on a bad file rather than throwing, so the catch arm
    // is not reproduced here.)

    let ty = instream.get_type();
    let mut outstream = match if output_opened {
        HfstOutputStream::new_filename(&common.output_filename, ty, true)
    } else {
        HfstOutputStream::new(ty, true)
    } {
        Ok(s) => s,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return 1;
        }
    };

    if is_input_stream_in_ol_format(&instream, "hfst-project") {
        return 1;
    }

    process_stream(&common, &options, &mut instream, &mut outstream)
}
