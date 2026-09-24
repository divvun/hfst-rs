//! Faithful 1:1 port of tools/src/hfst-subtract.cc — the transducer subtraction
//! (minus) command-line tool. A BINARY tool: it reads two input streams (first +
//! second); the shared scaffolding lives in crate::binary_ops and the option
//! layer in crate::cli.

use crate::binary_ops::{
    BinaryOpSpec, BinaryToolOp, LoopStyle, PairContext, RetryPolicy, run_binary_streams_tool,
};
use crate::cli::{self, BinaryIo, CommonArgs, ToolArgs, ToolResult};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{error, hfst_set_program_name, warning};
use hfst::backend::AlgebraBackend;
use hfst::hfst_transducer::FlagDiacriticOverlay;
use hfst::hfst_transducer::HfstTransducer;

/// hfst-subtract's command line.
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(
    about = "Subtract (minus) two transducers",
    after_help = "Examples:
  hfst-subtract -o catdog.hfst cat.hfst dog.hfst  subtracts transducers"
)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: BinaryIo,

    /// Harmonize flag diacritics
    #[arg(short = 'F', long = "harmonize-flags")]
    harmonize_flags: bool,

    /// Do not harmonize
    #[arg(short = 'H', long = "do-not-harmonize")]
    do_not_harmonize: bool,
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        self.io.apply(opts);
    }
}

// [spec:hfst:def:hfst-subtract.subtract-streams-fn]
// [spec:hfst:sem:hfst-subtract.subtract-streams-fn]
// The streams loop lives in crate::binary_ops::run_binary_streams_tool;
// this descriptor plus the pre-apply/apply closures in run carry the
// tool's behaviour contract.
const SPEC: BinaryOpSpec = BinaryOpSpec {
    tool_name: "hfst-subtract",
    mismatch_noun: "subtraction",
    could_not_verb: "subtract",
    could_not_noun: "subtraction",
    name_op: "subtract",
    formula: "\u{2212}",
    verbose_begin: |firstname, secondname| format!("Subtracting {} from {}", secondname, firstname),
    loop_style: LoopStyle::Standard,
    retry: RetryPolicy::AnyError,
    flush_each_round: false,
    flush_at_end: true,
};

// [spec:hfst:def:hfst-subtract.main-fn]
// [spec:hfst:sem:hfst-subtract.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstSubtract");
    let (common, args) = cli::parse::<Args>(common, args)?;

    let mut op = SubtractOp {
        harmonize: !args.do_not_harmonize,
        harmonize_flags: args.harmonize_flags,
        flag_overlay: None,
    };
    cli::from_code(run_binary_streams_tool(&common, &SPEC, &mut op))
}

struct SubtractOp {
    harmonize: bool,
    harmonize_flags: bool,
    flag_overlay: Option<FlagDiacriticOverlay>,
}

impl BinaryToolOp for SubtractOp {
    fn pre_apply<B: AlgebraBackend>(
        &mut self,
        common: &CommonOptions,
        first: &mut HfstTransducer<B>,
        second: &mut HfstTransducer<B>,
        _ctx: &PairContext<'_>,
    ) -> Result<(), i32> {
        self.flag_overlay = None;
        if second.has_flag_diacritics() {
            warning(
                common,
                0,
                0,
                &format!(
                    "Warning: {} contains flag diacritics. The result of subtraction may be incorrect.",
                    common.second_filename
                ),
            );
        }
        let first_has_flags = first.has_flag_diacritics();
        let second_has_flags = second.has_flag_diacritics();
        if first_has_flags && second_has_flags {
            if !self.harmonize_flags {
                if !common.silent {
                    warning(
                        common,
                        0,
                        0,
                        "The argumentes contain flag diacritics. Use -F to harmonize them.",
                    );
                }
            } else {
                let prepared = if B::SUPPORTS_VIRTUAL_FLAG_SUBTRACTION {
                    first
                        .prepare_flag_diacritics_for_operation(second)
                        .map(Some)
                } else {
                    // C: 'first->harmonize_flag_diacritics(*second)' — relies
                    // on the default 'insert_renamed_flags=true'.
                    first.harmonize_flag_diacritics(second, true).map(|()| None)
                };
                match prepared {
                    Ok(overlay) => self.flag_overlay = overlay,
                    Err(e) => {
                        error(common, 1, 0, &format!("{e}"));
                        return Err(1);
                    }
                }
            }
        }
        Ok(())
    }

    fn apply<B: AlgebraBackend>(
        &mut self,
        first: &mut HfstTransducer<B>,
        second: &HfstTransducer<B>,
    ) -> hfst::error::Result<()> {
        first
            .subtract_with_flag_overlay(second, self.harmonize, self.flag_overlay.as_ref())
            .map(|_| ())
    }
}
