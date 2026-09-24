//! Faithful 1:1 port of tools/src/hfst-project.cc — the transducer projection
//! command-line tool. Option handling is clap 4 derive through [`crate::cli`].

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{error, hfst_set_program_name};
use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
use hfst::backend::AlgebraBackend;
use hfst::hfst_transducer::HfstTransducer;
use std::borrow::Cow;

/// hfst-project's command line.
// [spec:hfst:def:hfst-project.parse-options-fn]
// [spec:hfst:sem:hfst-project.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Project (extract a level) transducer")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Project extracting tape LEVEL: upper, input, first, analysis or
    /// lower, output, second, generation
    #[arg(short = 'p', long = "project", value_name = "LEVEL")]
    project: Option<String>,
}

impl Args {
    /// Case 'p': the C compares only the FIRST character of the argument,
    /// case-insensitively (strncasecmp with length 1), against each
    /// candidate word. An argument matching none of them is fatal.
    fn project_input(&self, common: &CommonOptions) -> bool {
        let Some(level) = self.project.as_deref() else {
            return false;
        };
        if first_char_matches(level, "upper")
            || first_char_matches(level, "input")
            || first_char_matches(level, "first")
            || first_char_matches(level, "analysis")
        {
            true
        } else if first_char_matches(level, "lower")
            || first_char_matches(level, "output")
            || first_char_matches(level, "second")
            || first_char_matches(level, "generation")
        {
            false
        } else {
            error(
                common,
                1,
                0,
                &format!(
                    "unknown project direction {}\nshould be one of upper, input, analysis, first, lower, output, second or generation\n",
                    level
                ),
            );
            false
        }
    }
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        self.io.apply(opts);
    }

    fn validate(&self, opts: &CommonOptions) -> ToolResult {
        // The C rejected an unknown LEVEL inside the getopt loop, before
        // the parameter checks; run it here for the same ordering.
        self.project_input(opts);
        Ok(())
    }
}

// strncasecmp(optarg, prefix, 1) == 0 — case-insensitive comparison of the
// first byte only (the C calls always pass length 1).
fn first_char_matches(level: &str, prefix: &str) -> bool {
    match level.bytes().next() {
        Some(first) => first.eq_ignore_ascii_case(&prefix.as_bytes()[0]),
        None => false,
    }
}

// [spec:hfst:def:hfst-project.process-stream-fn]
// [spec:hfst:sem:hfst-project.process-stream-fn]
//
// The stream loop lives in the shared unary driver; this op is the
// per-transducer body it dispatches into. Both the verbose verb and the
// name/formula stamp follow the projected tape.
struct ProjectOp {
    project_input: bool,
}

impl UnaryToolOp for ProjectOp {
    fn verbose_begin(&self, inputname: &str) -> String {
        if self.project_input {
            format!("Projecting first {}", inputname)
        } else {
            format!("Projecting second {}", inputname)
        }
    }

    fn verbose_sep(&self) -> &'static str {
        " "
    }

    fn name_op(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed(if self.project_input {
            "project-1st"
        } else {
            "project-2nd"
        }))
    }

    fn formula(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed(if self.project_input {
            "\u{00b9}"
        } else {
            "\u{00b2}"
        }))
    }

    fn apply<B: AlgebraBackend>(
        &mut self,
        _common: &CommonOptions,
        t: &mut HfstTransducer<B>,
    ) -> hfst::error::Result<()> {
        if self.project_input {
            t.input_project().map(|_| ())
        } else {
            t.output_project().map(|_| ())
        }
    }
}

const SPEC: UnaryOpSpec = UnaryOpSpec {
    tool_name: "hfst-project",
    reject_ol: true,
};

// [spec:hfst:def:hfst-project.main-fn]
// [spec:hfst:sem:hfst-project.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstProject");
    let (common, args) = cli::parse::<Args>(common, args)?;

    let mut op = ProjectOp {
        project_input: args.project_input(&common),
    };
    cli::from_code(run_unary_tool(&common, &SPEC, &mut op))
}
