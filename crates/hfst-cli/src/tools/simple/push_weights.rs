//! Faithful 1:1 port of tools/src/hfst-push-weights.cc — the weight pushing
//! command-line tool. Pushes the weights of a transducer towards its start or
//! end states. Option handling is clap 4 derive through [`crate::cli`].

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{error, hfst_set_program_name};
use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
use hfst::backend::AlgebraBackend;
use hfst::hfst_data_types::PushType;
use hfst::hfst_transducer::HfstTransducer;
use std::borrow::Cow;

/// hfst-push-weights's command line.
// [spec:hfst:def:hfst-push-weights.parse-options-fn]
// [spec:hfst:sem:hfst-push-weights.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Push weights of transducer")]
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
    /// Case 'p': the C matches only the FIRST character of the argument
    /// case-insensitively against each candidate word; the default (no
    /// -p at all) is to push towards the end/final state.
    fn push_initial(&self, common: &CommonOptions) -> bool {
        let Some(direction) = self.push.as_deref() else {
            return false;
        };
        if first_char_eq_ignore_case(direction, "start")
            || first_char_eq_ignore_case(direction, "initial")
            || first_char_eq_ignore_case(direction, "begin")
        {
            true
        } else if first_char_eq_ignore_case(direction, "end")
            || first_char_eq_ignore_case(direction, "final")
        {
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

// [spec:hfst:def:hfst-push-weights.process-stream-fn]
// [spec:hfst:sem:hfst-push-weights.process-stream-fn]
//
// The stream loop lives in the shared unary driver; this op is the
// per-transducer body it dispatches into. Both the verbose verb and the name
// stamp's -i/-f suffix follow the push direction.
struct PushWeightsOp {
    push_initial: bool,
}

impl UnaryToolOp for PushWeightsOp {
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
            "push-weights-i"
        } else {
            "push-weights-f"
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
            t.push_weights(PushType::TO_INITIAL_STATE).map(|_| ())
        } else {
            t.push_weights(PushType::TO_FINAL_STATE).map(|_| ())
        }
    }
}

const SPEC: UnaryOpSpec = UnaryOpSpec {
    tool_name: "hfst-push-weights",
    reject_ol: true,
};

// [spec:hfst:def:hfst-push-weights.main-fn]
// [spec:hfst:sem:hfst-push-weights.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstPush");
    let (common, args) = cli::parse::<Args>(common, args)?;

    let mut op = PushWeightsOp {
        push_initial: args.push_initial(&common),
    };
    cli::from_code(run_unary_tool(&common, &SPEC, &mut op))
}
