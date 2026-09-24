//! Faithful 1:1 port of tools/src/hfst-shuffle.cc — the transducer shuffle
//! command-line tool. A BINARY tool: it reads two input streams (firstfile +
//! secondfile) and writes their shuffle; the shared scaffolding lives in
//! crate::binary_ops and the option layer in crate::cli.

use crate::binary_ops::{
    BinaryOpSpec, BinaryToolOp, LoopStyle, RetryPolicy, run_binary_streams_tool,
};
use crate::cli::{self, BinaryIo, CommonArgs, ToolArgs, ToolResult};
use crate::globals::CommonOptions;
use crate::hfst_commandline::hfst_set_program_name;
use hfst::backend::AlgebraBackend;
use hfst::hfst_transducer::HfstTransducer;

/// hfst-shuffle's command line. The tool declares no options of its own.
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(
    about = "Shuffle two transducers",
    after_help = "Examples:
  hfst-shuffle -o shuffled.hfst cat.hfst dog.hfst"
)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: BinaryIo,
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        self.io.apply(opts);
    }
}

// [spec:hfst:def:hfst-shuffle.shuffle-streams-fn]
// [spec:hfst:sem:hfst-shuffle.shuffle-streams-fn]
// The streams loop lives in crate::binary_ops::run_binary_streams_tool;
// this descriptor plus the apply closure in run carry the tool's
// behaviour contract. The ShuffleAutomata retry policy reproduces the C's
// outer catch (TransducersAreNotAutomataException) around the inner catch
// (TransducerTypeMismatchException).
const SPEC: BinaryOpSpec = BinaryOpSpec {
    tool_name: "hfst-shuffle",
    mismatch_noun: "shuffle",
    could_not_verb: "shuffle",
    could_not_noun: "shuffling",
    name_op: "shuffle",
    formula: "shuffle",
    verbose_begin: |firstname, secondname| format!("Shuffling {} and {}", firstname, secondname),
    loop_style: LoopStyle::Standard,
    retry: RetryPolicy::ShuffleAutomata,
    flush_each_round: false,
    flush_at_end: false,
};

// [spec:hfst:def:hfst-shuffle.main-fn]
// [spec:hfst:sem:hfst-shuffle.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstShuffle");
    let (common, _args) = cli::parse::<Args>(common, args)?;

    cli::from_code(run_binary_streams_tool(&common, &SPEC, &mut ShuffleOp))
}

struct ShuffleOp;

impl BinaryToolOp for ShuffleOp {
    fn apply<B: AlgebraBackend>(
        &mut self,
        first: &mut HfstTransducer<B>,
        second: &HfstTransducer<B>,
    ) -> hfst::error::Result<()> {
        first.shuffle(second, true).map(|_| ())
    }
}
