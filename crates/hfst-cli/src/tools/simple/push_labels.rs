//! Faithful 1:1 port of tools/src/hfst-push-labels.cc — the label-pushing
//! command-line tool. Option handling is clap 4 derive through [`crate::cli`].

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, hfst_set_program_name, is_input_stream_in_ol_format, verbose_print,
};
use crate::unary_ops::{
    UnaryOpSpec, UnaryToolOp, open_input_stream, open_output_stream_like, unary_streams,
};
use hfst::backend::AlgebraBackend;
use hfst::hfst_data_types::PushType;
use hfst::hfst_transducer::HfstTransducer;
use std::borrow::Cow;

/// hfst-push-labels's command line.
// [spec:hfst:def:hfst-push-labels.parse-options-fn]
// [spec:hfst:sem:hfst-push-labels.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Push labels of transducer")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Push to DIRECTION: start, initial, begin or end, final
    #[arg(short = 'p', long = "push", value_name = "DIRECTION")]
    push: Option<String>,
}

impl Args {
    /// Case 'p': the C lowercases the argument and tests its first letter
    /// against s/i/b (towards the start) and e/f (towards the end).
    fn push_initial(&self, common: &CommonOptions) -> bool {
        let Some(direction) = self.push.as_deref() else {
            return false;
        };
        let lower = direction.to_ascii_lowercase();
        if lower.starts_with('s') || lower.starts_with('i') || lower.starts_with('b') {
            true
        } else if lower.starts_with('e') || lower.starts_with('f') {
            false
        } else {
            error(
                common,
                1,
                0,
                &format!(
                    "unknown push direction {}\nshould be one of start, initial, begin, end or final.\n",
                    direction
                ),
            );
            false
        }
    }
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        self.io.apply(opts);
    }

    fn validate(&self, opts: &CommonOptions) -> ToolResult {
        // The C rejected an unknown DIRECTION inside the getopt loop,
        // before the parameter checks; run it here for the same ordering.
        self.push_initial(opts);
        Ok(())
    }
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
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstPush");
    let (common, args) = cli::parse::<Args>(common, args)?;

    let mut op = PushLabelsOp {
        push_initial: args.push_initial(&common),
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

    let mut instream = open_input_stream(&common)?;

    if is_input_stream_in_ol_format(&instream, "hfst-push-labels") {
        return Err(1);
    }

    let mut outstream = open_output_stream_like(&common, &instream)?;

    cli::from_code(unary_streams(
        &common,
        &SPEC,
        &mut op,
        &mut instream,
        &mut outstream,
    ))
}
