//! Faithful 1:1 port of tools/src/hfst-subtract.cc — the transducer subtraction
//! (minus) command-line tool. A BINARY tool: it reads two input streams (first +
//! second); the shared scaffolding lives in crate::binary_ops and the option
//! layer in crate::cli.

use crate::binary_ops::{
    BinaryOpSpec, FlagHarmonizing, FlagHarmonizingOp, LoopStyle, RetryPolicy,
    run_binary_streams_tool,
};
use crate::cli::{self, BinaryIo, CommonArgs, ToolArgs, ToolResult};
use crate::globals::CommonOptions;
use crate::hfst_commandline::hfst_set_program_name;
use hfst::tools::operations::BinaryOptions;

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

    let mut op = FlagHarmonizingOp {
        kind: FlagHarmonizing::Subtract,
        options: BinaryOptions {
            harmonize: !args.do_not_harmonize,
            harmonize_flags: args.harmonize_flags,
        },
    };
    cli::from_code(run_binary_streams_tool(&common, &SPEC, &mut op))
}
