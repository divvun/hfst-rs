//! Faithful 1:1 port of tools/src/hfst-prune-alphabet.cc — the transducer
//! alphabet-pruning command-line tool. Option handling is clap 4 derive
//! through [`crate::cli`].

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::hfst_set_program_name;
use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
use hfst::backend::AlgebraBackend;
use hfst::hfst_transducer::HfstTransducer;
use std::borrow::Cow;

/// hfst-prune-alphabet's command line.
//
// The C cases assign the SAME flag ('f' sets it, 'S' clears it), so the
// last of the two on the command line decides; mutual overrides_with is
// how clap says that.
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Prune the alphabet of a transducer")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Force pruning
    #[arg(short = 'f', long = "force", overrides_with = "safe")]
    force: bool,

    /// Prune only if no unknown or identity symbols are used in the
    /// transducer (default)
    #[arg(short = 'S', long = "safe", overrides_with = "force")]
    safe: bool,
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        self.io.apply(opts);
    }
}

// [spec:hfst:def:hfst-prune-alphabet.process-stream-fn]
// [spec:hfst:sem:hfst-prune-alphabet.process-stream-fn]
//
// The stream loop lives in the shared unary driver; this op is the
// per-transducer body it dispatches into. The tool stamps a name but no
// formula, so `formula` keeps the trait default of None.
struct PruneAlphabetOp {
    force_pruning: bool,
}

impl UnaryToolOp for PruneAlphabetOp {
    fn verbose_begin(&self, inputname: &str) -> String {
        format!("Pruning {}", inputname)
    }

    fn verbose_sep(&self) -> &'static str {
        " "
    }

    fn name_op(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed("prune-alphabet"))
    }

    fn apply<B: AlgebraBackend>(
        &mut self,
        _common: &CommonOptions,
        t: &mut HfstTransducer<B>,
    ) -> hfst::error::Result<()> {
        t.prune_alphabet(self.force_pruning).map(|_| ())
    }
}

const SPEC: UnaryOpSpec = UnaryOpSpec {
    tool_name: "hfst-prune-alphabet",
    reject_ol: true,
};

// [spec:hfst:def:hfst-prune-alphabet.main-fn]
// [spec:hfst:sem:hfst-prune-alphabet.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstPruneAlphabet");
    let (common, args) = cli::parse::<Args>(common, args)?;

    let mut op = PruneAlphabetOp {
        force_pruning: args.force,
    };
    cli::from_code(run_unary_tool(&common, &SPEC, &mut op))
}
