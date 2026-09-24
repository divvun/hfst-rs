//! Faithful 1:1 port of tools/src/hfst-remove-epsilons.cc — the transducer
//! epsilon-removal command-line tool.
//!
//! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
//! lives in [`CommonOptions`] (the shared -v/-q/-o/-i/... fields), built from
//! the parsed [`Args`] and threaded into the processing functions. There are
//! no `static mut` globals and no `unsafe`.

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::hfst_set_program_name;
use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
use hfst::backend::AlgebraBackend;
use hfst::hfst_transducer::HfstTransducer;
use std::borrow::Cow;

/// hfst-remove-epsilons's command line: the common and unary options only.
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Remove epsilons from a transducer")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        self.io.apply(opts);
    }
}

// [spec:hfst:def:hfst-remove-epsilons.process-stream-fn]
// [spec:hfst:sem:hfst-remove-epsilons.process-stream-fn]
//
// The stream loop lives in the shared unary driver; this op is the
// per-transducer body it dispatches into.
struct RemoveEpsilonsOp;

impl UnaryToolOp for RemoveEpsilonsOp {
    fn verbose_begin(&self, inputname: &str) -> String {
        format!("Removing epsilons {}", inputname)
    }

    fn name_op(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed("remove-epsilons"))
    }

    fn formula(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed("Id"))
    }

    fn apply<B: AlgebraBackend>(
        &mut self,
        _common: &CommonOptions,
        t: &mut HfstTransducer<B>,
    ) -> hfst::error::Result<()> {
        t.remove_epsilons().map(|_| ())
    }
}

const SPEC: UnaryOpSpec = UnaryOpSpec {
    tool_name: "hfst-remove-epsilons",
    reject_ol: true,
};

// [spec:hfst:def:hfst-remove-epsilons.main-fn]
// [spec:hfst:sem:hfst-remove-epsilons.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstRemoveEpsilons");
    let (common, _args) = cli::parse::<Args>(common, args)?;

    cli::from_code(run_unary_tool(&common, &SPEC, &mut RemoveEpsilonsOp))
}
