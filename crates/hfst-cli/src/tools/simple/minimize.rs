//! Port of tools/src/hfst-minimize.cc — the transducer minimisation
//! command-line tool.
//!
//! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
//! lives in [`CommonOptions`] (the shared -v/-q/-o/-i/... fields), built from
//! the parsed [`Args`] and threaded into the processing functions. There are
//! no `static mut` globals and no `unsafe`. This is the template the other
//! unary tools follow.

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::hfst_set_program_name;
use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
use hfst::backend::AlgebraBackend;
use hfst::hfst_transducer::{EngineConfig, HfstTransducer};
use std::borrow::Cow;

/// hfst-minimize's command line.
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Minimize a transducer")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Encode weights when minimizing (default is false)
    #[arg(short = 'E', long = "encode-weights")]
    encode_weights: bool,
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        self.io.apply(opts);
    }
}

// [spec:hfst:def:hfst-minimize.process-stream-fn]
// [spec:hfst:sem:hfst-minimize.process-stream-fn]
//
// The stream loop lives in the shared unary driver; this op is the
// per-transducer body it dispatches into.
struct MinimizeOp {
    encode_weights: bool,
}

impl UnaryToolOp for MinimizeOp {
    fn verbose_begin(&self, inputname: &str) -> String {
        format!("Minimizing {}", inputname)
    }

    fn name_op(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed("minimize"))
    }

    fn formula(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed("M"))
    }

    fn apply<B: AlgebraBackend>(
        &mut self,
        _common: &CommonOptions,
        t: &mut HfstTransducer<B>,
    ) -> hfst::error::Result<()> {
        t.minimize_with_config(&EngineConfig {
            encode_weights: self.encode_weights,
            ..EngineConfig::default()
        })
        .map(|_| ())
    }
}

const SPEC: UnaryOpSpec = UnaryOpSpec {
    tool_name: "hfst-minimize",
    reject_ol: true,
};

// [spec:hfst:def:hfst-minimize.main-fn]
// [spec:hfst:sem:hfst-minimize.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstMinimize");
    let (common, args) = cli::parse::<Args>(common, args)?;

    let mut op = MinimizeOp {
        encode_weights: args.encode_weights,
    };
    cli::from_code(run_unary_tool(&common, &SPEC, &mut op))
}
