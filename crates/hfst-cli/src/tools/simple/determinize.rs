//! Faithful 1:1 port of tools/src/hfst-determinize.cc — the transducer
//! determinisation command-line tool.
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
use hfst::hfst_transducer::{EngineConfig, HfstTransducer};
use std::borrow::Cow;

/// hfst-determinize's command line.
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Determinize a transducer")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Encode weights when determinizing (default is false)
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

// [spec:hfst:def:hfst-determinize.process-stream-fn]
// [spec:hfst:sem:hfst-determinize.process-stream-fn]
//
// The stream loop lives in the shared unary driver; this op is the
// per-transducer body it dispatches into.
struct DeterminizeOp {
    encode_weights: bool,
}

impl UnaryToolOp for DeterminizeOp {
    fn verbose_begin(&self, inputname: &str) -> String {
        format!("Determinizing {}", inputname)
    }

    fn name_op(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed("determinize"))
    }

    fn formula(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed("\u{2336}"))
    }

    fn apply<B: AlgebraBackend>(
        &mut self,
        _common: &CommonOptions,
        t: &mut HfstTransducer<B>,
    ) -> hfst::error::Result<()> {
        t.determinize_with_config(&EngineConfig {
            encode_weights: self.encode_weights,
            ..EngineConfig::default()
        })
        .map(|_| ())
    }
}

const SPEC: UnaryOpSpec = UnaryOpSpec {
    tool_name: "hfst-determinize",
    reject_ol: true,
};

// [spec:hfst:def:hfst-determinize.main-fn]
// [spec:hfst:sem:hfst-determinize.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstDeterminize");
    let (common, args) = cli::parse::<Args>(common, args)?;

    let mut op = DeterminizeOp {
        encode_weights: args.encode_weights,
    };
    cli::from_code(run_unary_tool(&common, &SPEC, &mut op))
}
