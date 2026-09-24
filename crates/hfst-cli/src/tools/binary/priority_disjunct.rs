//! Faithful 1:1 port of tools/src/hfst-priority-disjunct.cc — the transducer
//! priority disjunction (priority union) command-line tool. A BINARY tool: it
//! reads two input streams (firstfile + secondfile) and writes their priority
//! union; the shared scaffolding lives in crate::binary_ops and the option
//! layer in crate::cli.

use crate::binary_ops::{
    BinaryOpSpec, BinaryToolOp, LoopStyle, RetryPolicy, run_binary_streams_tool,
};
use crate::cli::{self, BinaryIo, CommonArgs, ToolArgs, ToolResult};
use crate::globals::CommonOptions;
use crate::hfst_commandline::hfst_set_program_name;
use hfst::backend::AlgebraBackend;
use hfst::hfst_transducer::HfstTransducer;

/// hfst-priority-disjunct's command line.
//
// '-H' is accepted and has no effect, and '-F' is not accepted at all:
// upstream's usage text advertises both, its getopt table carried only
// 'do-not-harmonize', and priority_union takes no harmonize parameter, so
// neither static ever reached the operation. Preserved bug-for-bug.
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(
    about = "Disjunct (union, OR) two transducers",
    after_help = "Examples:
  hfst-priority-disjunct -o cat_or_dog.hfst cat.hfst dog.hfst"
)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: BinaryIo,

    /// Do not harmonize symbols (accepted; priority union does not harmonize)
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

// [spec:hfst:def:hfst-priority-disjunct.priority-disjunct-streams-fn]
// [spec:hfst:sem:hfst-priority-disjunct.priority-disjunct-streams-fn]
// The streams loop lives in crate::binary_ops::run_binary_streams_tool;
// this descriptor plus the apply closure in run carry the tool's
// behaviour contract.
const SPEC: BinaryOpSpec = BinaryOpSpec {
    tool_name: "hfst-priority-disjunct",
    mismatch_noun: "priority disjunction",
    could_not_verb: "priority disjunct",
    could_not_noun: "priority disjunction",
    name_op: "union",
    formula: "\u{222a}",
    verbose_begin: |firstname, secondname| format!("Disjuncting {} and {}", firstname, secondname),
    loop_style: LoopStyle::Standard,
    retry: RetryPolicy::AnyError,
    flush_each_round: true,
    flush_at_end: false,
};

// [spec:hfst:def:hfst-priority-disjunct.main-fn]
// [spec:hfst:sem:hfst-priority-disjunct.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstPriorityDisjunct");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let _ = args.do_not_harmonize;

    cli::from_code(run_binary_streams_tool(
        &common,
        &SPEC,
        &mut PriorityDisjunctOp,
    ))
}

struct PriorityDisjunctOp;

impl BinaryToolOp for PriorityDisjunctOp {
    fn apply<B: AlgebraBackend>(
        &mut self,
        first: &mut HfstTransducer<B>,
        second: &HfstTransducer<B>,
    ) -> hfst::error::Result<()> {
        // C: 'first->priority_union(*second)'; no harmonize parameter.
        first.priority_union(second).map(|_| ())
    }
}
