//! Faithful 1:1 port of tools/src/hfst-disjunct.cc — the transducer
//! disjunction (union, OR) command-line tool. A BINARY tool: it reads two input
//! streams (firstfile + secondfile) and writes their disjunction; the shared
//! scaffolding lives in crate::binary_ops and the option layer in crate::cli.

use crate::binary_ops::{
    BinaryOpSpec, BinaryToolOp, LoopStyle, RetryPolicy, run_binary_streams_tool,
};
use crate::cli::{self, BinaryIo, CommonArgs, ToolArgs, ToolResult};
use crate::globals::CommonOptions;
use crate::hfst_commandline::hfst_set_program_name;
use hfst::backend::AlgebraBackend;
use hfst::hfst_transducer::HfstTransducer;

/// hfst-disjunct's command line.
//
// '-F, --harmonize-flags' is DELIBERATELY absent: upstream's usage text
// advertises it, but its getopt table never carried the option and the
// harmonize_flags static stayed false, so the flag was never accepted.
// Preserved bug-for-bug — the usage text is what stops advertising it.
// [spec:hfst:def:hfst-disjunct.parse-options-fn]
// [spec:hfst:sem:hfst-disjunct.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(
    about = "Disjunct (union, OR) two transducers",
    after_help = "Examples:
  hfst-disjunct -o cat_or_dog.hfst cat.hfst dog.hfst"
)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: BinaryIo,

    /// Do not harmonize symbols
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

// [spec:hfst:def:hfst-disjunct.disjunct-streams-fn]
// [spec:hfst:sem:hfst-disjunct.disjunct-streams-fn]
// The streams loop lives in crate::binary_ops::run_binary_streams_tool;
// this descriptor plus the apply closure in run carry the tool's
// behaviour contract.
const SPEC: BinaryOpSpec = BinaryOpSpec {
    tool_name: "hfst-disjunct",
    mismatch_noun: "disjunction",
    could_not_verb: "disjunct",
    could_not_noun: "disjunction",
    name_op: "union",
    formula: "\u{222a}",
    verbose_begin: |firstname, secondname| format!("Disjuncting {} and {}", firstname, secondname),
    loop_style: LoopStyle::Standard,
    retry: RetryPolicy::AnyError,
    flush_each_round: true,
    flush_at_end: false,
};

// [spec:hfst:def:hfst-disjunct.main-fn]
// [spec:hfst:sem:hfst-disjunct.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstDisjunct");
    let (common, args) = cli::parse::<Args>(common, args)?;

    let mut op = DisjunctOp {
        harmonize: !args.do_not_harmonize,
    };
    cli::from_code(run_binary_streams_tool(&common, &SPEC, &mut op))
}

struct DisjunctOp {
    harmonize: bool,
}

impl BinaryToolOp for DisjunctOp {
    fn apply<B: AlgebraBackend>(
        &mut self,
        first: &mut HfstTransducer<B>,
        second: &HfstTransducer<B>,
    ) -> hfst::error::Result<()> {
        first.disjunct(second, self.harmonize).map(|_| ())
    }
}
