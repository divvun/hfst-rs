//! Faithful 1:1 port of tools/src/hfst-concatenate.cc — the transducer
//! concatenation command-line tool.
//!
//! This is a BINARY tool: it reads two input streams (firststream and
//! secondstream) and writes their pairwise concatenation; the shared
//! scaffolding lives in crate::binary_ops and the option layer in
//! crate::cli.

use crate::binary_ops::{
    BinaryOpSpec, BinaryToolOp, LoopStyle, PairContext, RetryPolicy, run_binary_streams_tool,
};
use crate::cli::{self, BinaryIo, CommonArgs, ToolArgs, ToolResult};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{error, hfst_set_program_name, warning};
use hfst::backend::AlgebraBackend;
use hfst::hfst_transducer::HfstTransducer;

/// hfst-concatenate's command line.
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(
    about = "Concatenate two transducers",
    after_help = "Examples:
  hfst-concatenate -o catdog.hfst cat.hfst dog.hfst
concatenates cat.hfst with dog.hfst and writes results to catdog.hfst"
)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: BinaryIo,

    /// Do not harmonize symbols
    #[arg(short = 'H', long = "do-not-harmonize")]
    do_not_harmonize: bool,

    /// Harmonize flag diacritics
    #[arg(short = 'F', long = "harmonize-flags")]
    harmonize_flags: bool,
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        self.io.apply(opts);
    }
}

// [spec:hfst:def:hfst-concatenate.concatenate-streams-fn]
// [spec:hfst:sem:hfst-concatenate.concatenate-streams-fn]
// The streams loop lives in crate::binary_ops::run_binary_streams_tool;
// this descriptor plus the pre-apply/apply closures in run carry the
// tool's behaviour contract.
const SPEC: BinaryOpSpec = BinaryOpSpec {
    tool_name: "hfst-concatenate",
    mismatch_noun: "concatenation",
    could_not_verb: "concatenate",
    could_not_noun: "concatenation",
    name_op: "concatenate",
    formula: "\u{22c5}",
    verbose_begin: |firstname, secondname| {
        format!("Concatenating {} and {}", firstname, secondname)
    },
    loop_style: LoopStyle::Standard,
    retry: RetryPolicy::TypeMismatchOnly,
    flush_each_round: false,
    flush_at_end: true,
};

// [spec:hfst:def:hfst-concatenate.main-fn]
// [spec:hfst:sem:hfst-concatenate.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstConcatenate");
    let (common, args) = cli::parse::<Args>(common, args)?;

    let mut op = ConcatenateOp {
        harmonize: !args.do_not_harmonize,
        harmonize_flags: args.harmonize_flags,
    };
    cli::from_code(run_binary_streams_tool(&common, &SPEC, &mut op))
}

struct ConcatenateOp {
    harmonize: bool,
    harmonize_flags: bool,
}

impl BinaryToolOp for ConcatenateOp {
    fn pre_apply<B: AlgebraBackend>(
        &mut self,
        common: &CommonOptions,
        first: &mut HfstTransducer<B>,
        second: &mut HfstTransducer<B>,
        _ctx: &PairContext<'_>,
    ) -> Result<(), i32> {
        let both_have_flags = first.has_flag_diacritics() && second.has_flag_diacritics();
        if both_have_flags {
            if !self.harmonize_flags {
                if !common.silent {
                    warning(
                        common,
                        0,
                        0,
                        "The arguments contain flag diacritics. Use -F to harmonize them.",
                    );
                }
            } else if let Err(e) = first.harmonize_flag_diacritics(second, false) {
                error(common, 1, 0, &format!("{e}"));
                return Err(1);
            }
        }
        Ok(())
    }

    fn apply<B: AlgebraBackend>(
        &mut self,
        first: &mut HfstTransducer<B>,
        second: &HfstTransducer<B>,
    ) -> hfst::error::Result<()> {
        first.concatenate(second, self.harmonize).map(|_| ())
    }
}
