//! Faithful 1:1 port of tools/src/hfst-push-labels.cc — the label-pushing
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).

use crate::globals;
use crate::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_from_env, hfst_set_program_name,
    is_input_stream_in_ol_format, print_more_info, print_report_bugs, verbose_print,
};
use crate::hfst_getopt as getopt;
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
use hfst::hfst_transducer::HfstTransducer;
use std::io::Write;

// add tools-specific variables here
static mut PUSH_INITIAL: bool = false;

// [spec:hfst:def:hfst-push-labels.print-usage-fn]
// [spec:hfst:sem:hfst-push-labels.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nPush labels of transducer\n\n",
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

// [spec:hfst:def:hfst-push-labels.parse-options-fn]
// [spec:hfst:sem:hfst-push-labels.parse-options-fn]
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
                name: "push",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: b'p' as i32,
            });
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own ('p'), then the
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
            if c == b'p' as i32 {
                let optarg = getopt::optarg();
                let lower = optarg.to_ascii_lowercase();
                if lower.starts_with('s') || lower.starts_with('i') || lower.starts_with('b') {
                    PUSH_INITIAL = true;
                } else if lower.starts_with('e') || lower.starts_with('f') {
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

// [spec:hfst:def:hfst-push-labels.process-stream-fn]
// [spec:hfst:sem:hfst-push-labels.process-stream-fn]
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
                if PUSH_INITIAL {
                    verbose_print(&format!("Pushing towards start {}...\n", inputname));
                } else {
                    verbose_print(&format!("Pushing towards end {}...\n", inputname));
                }
            } else if PUSH_INITIAL {
                verbose_print(&format!(
                    "Pushing towards start {}... {}\n",
                    inputname, transducer_n
                ));
            } else {
                verbose_print(&format!(
                    "Pushing towards end {}... {}\n",
                    inputname, transducer_n
                ));
            }

            if PUSH_INITIAL {
                if let Err(e) = trans.push_labels(PushType::TO_INITIAL_STATE) {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
                // C: hfst_set_name(trans, trans, ...); dest and src are the same
                // object, which Rust cannot alias mut+const, so the read side is
                // taken from a copy (name/formula are unchanged by the copy).
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "push-labels-i");
                hfst_set_formula_unary(&mut trans, &src, "Id");
            } else {
                if let Err(e) = trans.push_labels(PushType::TO_FINAL_STATE) {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "push-labels-f");
                hfst_set_formula_unary(&mut trans, &src, "Id");
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

// [spec:hfst:def:hfst-push-labels.main-fn]
// [spec:hfst:sem:hfst-push-labels.main-fn]
pub fn run(args: Vec<String>) -> i32 {
    unsafe { real_main(args) }
}

unsafe fn real_main(mut args: Vec<String>) -> i32 {
    unsafe {
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstPush");
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

        if is_input_stream_in_ol_format(&instream, "hfst-push-labels") {
            return 1;
        }

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

        process_stream(&mut instream, &mut outstream)
    }
}
