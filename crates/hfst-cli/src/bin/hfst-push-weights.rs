//! Faithful 1:1 port of tools/src/hfst-push-weights.cc — the weight pushing
//! command-line tool. Pushes the weights of a transducer towards its start or
//! end states. Drives the hfst-cli foundation (globals, getopt, commandline,
//! program-options, tool-metadata, inc fragments).

use hfst::hfst_data_types::PushType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_getenv, hfst_set_program_name,
    is_input_stream_in_ol_format, print_more_info, print_report_bugs, verbose_printf,
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
static mut PUSH_INITIAL: bool = false;

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
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    // Usage line
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nPush weights of transducer\n\n",
        globals::program_name()
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Push options:\n  -p, --push=DIRECTION   push to DIRECTION\n"
    );
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(
        msg,
        "DIRECTION must be one of start, initial, begin or end, final\n"
    );
    let _ = write!(msg, "\n");
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-push-weights.parse-options-fn]
// [spec:hfst:sem:hfst-push-weights.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_getenv(args);
        // use of this function requires options are settable on global scope
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
                let optarg = getopt::optarg();
                if first_char_eq_ignore_case(&optarg, "start")
                    || first_char_eq_ignore_case(&optarg, "initial")
                    || first_char_eq_ignore_case(&optarg, "begin")
                {
                    PUSH_INITIAL = true;
                } else if first_char_eq_ignore_case(&optarg, "end")
                    || first_char_eq_ignore_case(&optarg, "final")
                {
                    PUSH_INITIAL = false;
                } else {
                    error(
                        1,
                        0,
                        &format!(
                            "unknown push direction {}\nshould be one of start, initial, begin, end or final.\n",
                            optarg
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

// [spec:hfst:def:hfst-push-weights.process-stream-fn]
// [spec:hfst:sem:hfst-push-weights.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream, outstream: &mut HfstOutputStream) -> i32 {
    unsafe {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let mut trans = HfstTransducer::new_from_stream(instream);
            let inputname = hfst_get_name(&trans, &globals::input_filename());
            if transducer_n == 1 {
                if PUSH_INITIAL {
                    verbose_printf(&format!("Pushing towards start {}...\n", inputname));
                } else {
                    verbose_printf(&format!("Pushing towards end {}...\n", inputname));
                }
            } else if PUSH_INITIAL {
                verbose_printf(&format!(
                    "Pushing towards start {}... {}\n",
                    inputname, transducer_n
                ));
            } else {
                verbose_printf(&format!(
                    "Pushing towards end {}... {}\n",
                    inputname, transducer_n
                ));
            }

            if PUSH_INITIAL {
                trans.push_weights(PushType::TO_INITIAL_STATE);
                // C: hfst_set_name(trans, trans, "..."); dest and src alias the
                // same object, which Rust cannot borrow mut+const at once, so the
                // src side is taken from a copy (name/formula survive the copy).
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "push-weights-i");
                hfst_set_formula_unary(&mut trans, &src, "Id");
            } else {
                trans.push_weights(PushType::TO_FINAL_STATE);
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "push-weights-f");
                hfst_set_formula_unary(&mut trans, &src, "Id");
            }
            outstream.redirect(&mut trans);
        }
        instream.close();
        outstream.close();
        0
    }
}

// [spec:hfst:def:hfst-push-weights.main-fn]
// [spec:hfst:sem:hfst-push-weights.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstPush");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let input_opened = globals::input_filename() != "<stdin>";
        let output_opened = globals::output_filename() != "<stdout>";
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            globals::input_filename(),
            globals::output_filename()
        ));

        // here starts the buffer handling part
        let mut instream = if input_opened {
            HfstInputStream::new_filename(&globals::input_filename())
        } else {
            HfstInputStream::new()
        };
        // (the C wraps the ctor in try/catch on HfstException, reporting that the
        // input is not a valid transducer file; the Rust ctor currently panics on
        // a bad file rather than throwing, so the catch arm is not reproduced.)

        let type_ = instream.get_type();
        let mut outstream = if output_opened {
            HfstOutputStream::new_filename(&globals::output_filename(), type_, true)
        } else {
            HfstOutputStream::new(type_, true)
        };

        if is_input_stream_in_ol_format(&instream, "hfst-push-weights") {
            return 1;
        }

        process_stream(&mut instream, &mut outstream)
    }
}
