//! Faithful 1:1 port of tools/src/hfst-realign.cc — the transducer realign
//! command-line tool.
//!
//! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
//! lives in [`CommonOptions`] (the shared -v/-q/-o/-i/... fields), built from
//! the parsed [`Args`] and threaded into the processing functions. There are
//! no `static mut` globals and no `unsafe`.

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{error, hfst_set_program_name, print_short_help};
use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
use hfst::backend::AlgebraBackend;
use hfst::hfst_transducer::HfstTransducer;
use std::borrow::Cow;

/// The boundary symbol the C initialises and never changes; see
/// [`Args::validate`] for why -b cannot change it.
const DEFAULT_BOUNDARY_SYMBOL: u8 = b'>';

/// hfst-realign's command line.
// [spec:hfst:def:hfst-realign.parse-options-fn]
// [spec:hfst:sem:hfst-realign.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Realign a transducer by pushing labels to the start")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Treat SYM as a boundary symbol; SYM must be in the alphabet
    #[arg(short = 'b', long = "boundary", value_name = "SYM")]
    boundary: Option<String>,
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        self.io.apply(opts);
    }

    fn validate(&self, opts: &CommonOptions) -> ToolResult {
        // Upstream registers --boundary under the option value 'b' but
        // labels its own switch arm 'p', so a returned 'b' matches no case
        // and falls through to the default error arm: giving -b/--boundary
        // is fatal, and the arm that would have set the symbol is dead.
        // Preserved as-is, so the option parses and then rejects.
        if self.boundary.is_some() {
            print_short_help(opts);
            error(opts, 1, 0, "invalid option -b");
            return Err(1);
        }
        Ok(())
    }
}

// [spec:hfst:def:hfst-realign.process-stream-fn]
// [spec:hfst:sem:hfst-realign.process-stream-fn]
//
// The stream loop lives in the shared unary driver; this op is the
// per-transducer body it dispatches into. The C's verbose verb is selected by
// the boundary symbol (a leftover of the push-labels tool it was copied from),
// so the op carries it.
struct RealignOp {
    boundary_symbol: u8,
}

impl UnaryToolOp for RealignOp {
    fn verbose_begin(&self, inputname: &str) -> String {
        if self.boundary_symbol != 0 {
            format!("Pushing towards start {}", inputname)
        } else {
            format!("Pushing towards end {}", inputname)
        }
    }

    fn verbose_sep(&self) -> &'static str {
        " "
    }

    fn name_op(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed("realign"))
    }

    fn formula(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed("Id"))
    }

    fn apply<B: AlgebraBackend>(
        &mut self,
        _common: &CommonOptions,
        t: &mut HfstTransducer<B>,
    ) -> hfst::error::Result<()> {
        t.realign().map(|_| ())
    }
}

const SPEC: UnaryOpSpec = UnaryOpSpec {
    tool_name: "hfst-realign",
    reject_ol: true,
};

// [spec:hfst:def:hfst-realign.main-fn]
// [spec:hfst:sem:hfst-realign.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstRealign");
    let (common, _args) = cli::parse::<Args>(common, args)?;

    let mut op = RealignOp {
        boundary_symbol: DEFAULT_BOUNDARY_SYMBOL,
    };
    cli::from_code(run_unary_tool(&common, &SPEC, &mut op))
}
