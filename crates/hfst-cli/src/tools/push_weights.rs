//! Faithful 1:1 port of tools/src/hfst-push-weights.cc — the weight pushing
//! command-line tool. Pushes the weights of a transducer towards its start or
//! end states. Drives the hfst-cli foundation (globals, getopt, commandline,
//! program-options, tool-metadata, inc fragments).

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
use hfst::hfst_data_types::PushType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use std::io::Write;

/// hfst-push-weights's own options (the former tool-specific `static mut`s).
#[derive(Default)]
struct Options {
    /// '-p, --push=DIRECTION': push towards the start state when true, else the
    /// end state (default is false, i.e. push towards the end/final state).
    push_initial: bool,
}

// strncasecmp(optarg, prefix, 1) == 0 : the first character of optarg matches
// the first character of prefix, case-insensitively. Each candidate prefix here
// starts with a distinct letter, so this is a one-character case-fold compare.
fn first_char_eq_ignore_case(arg: &str, prefix: &str) -> bool {
    match (arg.chars().next(), prefix.chars().next()) {
        (Some(a), Some(b)) => a.eq_ignore_ascii_case(&b),
        (None, None) => true,
        _ => false,
    }
}

// [spec:hfst:def:hfst-push-weights.print-usage-fn]
// [spec:hfst:sem:hfst-push-weights.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    // Usage line
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nPush weights of transducer\n\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Push options:\n  -p, --push=DIRECTION   push to DIRECTION\n"
    );
    let _ = writeln!(msg);
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = writeln!(
        msg,
        "DIRECTION must be one of start, initial, begin or end, final"
    );
    let _ = writeln!(msg);
}

// [spec:hfst:def:hfst-push-weights.parse-options-fn]
// [spec:hfst:sem:hfst-push-weights.parse-options-fn]
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
            name: "push",
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
            let optarg = opt.optarg();
            if first_char_eq_ignore_case(&optarg, "start")
                || first_char_eq_ignore_case(&optarg, "initial")
                || first_char_eq_ignore_case(&optarg, "begin")
            {
                options.push_initial = true;
            } else if first_char_eq_ignore_case(&optarg, "end")
                || first_char_eq_ignore_case(&optarg, "final")
            {
                options.push_initial = false;
            } else {
                error(
                    &common,
                    1,
                    0,
                    &format!(
                        "unknown push direction {}\nshould be one of start, initial, begin, end or final.\n",
                        optarg
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

// [spec:hfst:def:hfst-push-weights.process-stream-fn]
// [spec:hfst:sem:hfst-push-weights.process-stream-fn]
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
                if options.push_initial {
                    verbose_print(common, &format!("Pushing towards start {}...\n", inputname));
                } else {
                    verbose_print(common, &format!("Pushing towards end {}...\n", inputname));
                }
            } else if options.push_initial {
                verbose_print(common, &format!(
                    "Pushing towards start {}... {}\n",
                    inputname, transducer_n
                ));
            } else {
                verbose_print(common, &format!(
                    "Pushing towards end {}... {}\n",
                    inputname, transducer_n
                ));
            }

            if options.push_initial {
                if let Err(e) = trans.push_weights(PushType::TO_INITIAL_STATE) {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
                // C: hfst_set_name(trans, trans, "..."); dest and src alias the
                // same object, which Rust cannot borrow mut+const at once, so the
                // src side is taken from a copy (name/formula survive the copy).
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "push-weights-i");
                hfst_set_formula_unary(&mut trans, &src, "Id");
            } else {
                if let Err(e) = trans.push_weights(PushType::TO_FINAL_STATE) {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "push-weights-f");
                hfst_set_formula_unary(&mut trans, &src, "Id");
            }
            if let Err(e) = outstream.redirect(&mut trans) {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        }, else => {
            // Unreachable: the optimized-lookup stream rejection already
            // returned before the loop; keep its text for safety.
            let _ = writeln!(
                std::io::stderr(),
                "Error: hfst-push-weights cannot process transducers that are in optimized lookup format."
            );
            return 1;
        });
    }
    instream.close();
    outstream.close();
    0
}

// [spec:hfst:def:hfst-push-weights.main-fn]
// [spec:hfst:sem:hfst-push-weights.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstPush");
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
    // (the C wraps the ctor in try/catch on HfstException, reporting that the
    // input is not a valid transducer file; the Rust ctor currently panics on
    // a bad file rather than throwing, so the catch arm is not reproduced.)

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

    if is_input_stream_in_ol_format(&instream, "hfst-push-weights") {
        return 1;
    }

    process_stream(&common, &options, &mut instream, &mut outstream)
}
