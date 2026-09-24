//! Port of tools/src/hfst-eliminate-flags.cc — the transducer flag elimination
//! command-line tool.
//!
//! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
//! lives in [`CommonOptions`] (the shared -v/-q/-o/-i/... fields), built from
//! the parsed [`Args`] and threaded into the processing functions. There are
//! no `static mut` globals and no `unsafe`.

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{error, hfst_set_program_name};
use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
use hfst::backend::AlgebraBackend;
use hfst::hfst_transducer::HfstTransducer;
use std::borrow::Cow;

/// hfst-eliminate-flags's command line.
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Eliminate flags from a transducer")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Only eliminate flag FLAG
    #[arg(short = 'F', long = "flag", value_name = "FLAG")]
    flag: Option<String>,
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        self.io.apply(opts);
    }
}

// [spec:hfst:def:hfst-eliminate-flags.process-stream-fn]
// [spec:hfst:sem:hfst-eliminate-flags.process-stream-fn]
//
// The stream loop lives in the shared unary driver; this op is the
// per-transducer body it dispatches into. The verbose verb names what is being
// eliminated ("flags" or "flag FLAG"), which the C computes once before the
// loop; here it is the op's own precomputed field.
struct EliminateFlagsOp {
    /// '-F, --flag=FLAG', if given.
    flag: Option<String>,
    /// The verbose line's object: "flags", or "flag FLAG".
    flags: String,
}

impl UnaryToolOp for EliminateFlagsOp {
    fn verbose_begin(&self, inputname: &str) -> String {
        // The C additionally falls back to the input filename on an empty
        // transducer name, which hfst_get_name has already done: it returns the
        // filename whenever the name is empty, so the guard could only ever
        // re-substitute the same empty filename.
        format!("Eliminating {} {}", self.flags, inputname)
    }

    fn name_op(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed("eliminate-flags"))
    }

    fn formula(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed("Id"))
    }

    fn apply<B: AlgebraBackend>(
        &mut self,
        common: &CommonOptions,
        t: &mut HfstTransducer<B>,
    ) -> hfst::error::Result<()> {
        match &self.flag {
            None => t.eliminate_flags().map(|_| ()),
            Some(f) => {
                if t.eliminate_flag(f).is_err() {
                    // The single-flag failure substitutes the tool's own text
                    // for the error value's, so it is reported here rather than
                    // through the driver's '{e}' path. `error` with a non-zero
                    // status exits the process, so the Err below is never
                    // observed; it stands for the C's `return 1`.
                    error(
                        common,
                        1,
                        0,
                        &format!(
                            "flag feature {} does not occur in the transducer\nonly the flag feature must be given, no value or operator",
                            f
                        ),
                    );
                    return Err(hfst::error::Error::new(hfst::error::ErrorKind::Fatal));
                }
                Ok(())
            }
        }
    }
}

const SPEC: UnaryOpSpec = UnaryOpSpec {
    tool_name: "hfst-eliminate-flags",
    reject_ol: true,
};

// [spec:hfst:def:hfst-eliminate-flags.main-fn]
// [spec:hfst:sem:hfst-eliminate-flags.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstEliminateFlags");
    let (common, args) = cli::parse::<Args>(common, args)?;

    let flags = match &args.flag {
        None => String::from("flags"),
        Some(f) => format!("flag {}", f),
    };
    let mut op = EliminateFlagsOp {
        flag: args.flag,
        flags,
    };
    cli::from_code(run_unary_tool(&common, &SPEC, &mut op))
}
