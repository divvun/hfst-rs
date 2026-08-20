//! Faithful 1:1 port of tools/src/hfst-push-labels.cc — the label-pushing
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
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use crate::unary_ops::{
    UnaryOpSpec, UnaryToolOp, open_input_stream, open_output_stream_like, unary_streams,
};
use hfst::backend::AlgebraBackend;
use hfst::hfst_data_types::PushType;
use hfst::hfst_transducer::HfstTransducer;
use std::borrow::Cow;
use std::io::Write;

/// hfst-push-labels's own options (the former tool-specific `static mut`s).
#[derive(Default)]
struct Options {
    /// '-p, --push=DIRECTION': push towards the initial state when true.
    push_initial: bool,
}

// [spec:hfst:def:hfst-push-labels.print-usage-fn]
// [spec:hfst:sem:hfst-push-labels.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nPush labels of transducer\n\n",
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

// [spec:hfst:def:hfst-push-labels.parse-options-fn]
// [spec:hfst:sem:hfst-push-labels.parse-options-fn]
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
            val: b'p' as i32,
        });
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: common
        // cases, then unary cases, then the tool's own ('p'), then the
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
        if c == b'p' as i32 {
            let optarg = opt.optarg();
            let lower = optarg.to_ascii_lowercase();
            if lower.starts_with('s') || lower.starts_with('i') || lower.starts_with('b') {
                options.push_initial = true;
            } else if lower.starts_with('e') || lower.starts_with('f') {
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

// [spec:hfst:def:hfst-push-labels.process-stream-fn]
// [spec:hfst:sem:hfst-push-labels.process-stream-fn]
//
// The stream loop lives in the shared unary driver; this op is the
// per-transducer body it dispatches into. Both the verbose verb and the name
// stamp's -i/-f suffix follow the push direction.
struct PushLabelsOp {
    push_initial: bool,
}

impl UnaryToolOp for PushLabelsOp {
    fn verbose_begin(&self, inputname: &str) -> String {
        if self.push_initial {
            format!("Pushing towards start {}", inputname)
        } else {
            format!("Pushing towards end {}", inputname)
        }
    }

    fn verbose_sep(&self) -> &'static str {
        " "
    }

    fn name_op(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed(if self.push_initial {
            "push-labels-i"
        } else {
            "push-labels-f"
        }))
    }

    fn formula(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed("Id"))
    }

    fn apply<B: AlgebraBackend>(
        &mut self,
        _common: &CommonOptions,
        t: &mut HfstTransducer<B>,
    ) -> hfst::error::Result<()> {
        if self.push_initial {
            t.push_labels(PushType::TO_INITIAL_STATE).map(|_| ())
        } else {
            t.push_labels(PushType::TO_FINAL_STATE).map(|_| ())
        }
    }
}

// `reject_ol` is left false because this tool rejects optimized-lookup input
// BEFORE opening the output stream (see run); the flag would reject it after.
const SPEC: UnaryOpSpec = UnaryOpSpec {
    tool_name: "hfst-push-labels",
    reject_ol: false,
};

// [spec:hfst:def:hfst-push-labels.main-fn]
// [spec:hfst:sem:hfst-push-labels.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstPush");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    let mut op = PushLabelsOp {
        push_initial: options.push_initial,
    };

    // This tool orders the optimized-lookup rejection BEFORE the output stream
    // is opened, unlike every other unary tool (and unlike run_unary_tool):
    // rejecting an OL input must not have created/truncated '-o FILE' first.
    // So the driver's steps are composed here in the tool's own order rather
    // than going through run_unary_tool.
    verbose_print(
        &common,
        &format!(
            "Reading from {}, writing to {}\n",
            common.input_filename, common.output_filename
        ),
    );

    let mut instream = match open_input_stream(&common) {
        Ok(s) => s,
        Err(code) => return code,
    };

    if is_input_stream_in_ol_format(&instream, "hfst-push-labels") {
        return 1;
    }

    let mut outstream = match open_output_stream_like(&common, &instream) {
        Ok(s) => s,
        Err(code) => return code,
    };

    unary_streams(&common, &SPEC, &mut op, &mut instream, &mut outstream)
}
