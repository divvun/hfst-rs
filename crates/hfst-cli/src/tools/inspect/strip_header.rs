//! Faithful 1:1 port of tools/src/hfst-strip-header.cc — the HFST header
//! stripping command-line tool. Drives the hfst-cli foundation (globals,
//! getopt, commandline, program-options, inc fragments).
//!
//! Unlike most unary tools, this one does not build HfstInputStream /
//! HfstOutputStream objects: it opens its input/output as std streams (from the
//! filename fields, with the "<stdin>"/"<stdout>" sentinels) and delegates the
//! byte copy + HFST3-header stripping to hfst_input_stream::strip_hfst3_headers.

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{hfst_set_program_name, verbose_print};
use hfst::hfst_input_stream::strip_hfst3_headers;

/// hfst-strip-header's command line. The tool declares no options of its own.
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Remove any HFST3 headers")]
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

// [spec:hfst:def:hfst-strip-header.process-stream-fn]
// [spec:hfst:sem:hfst-strip-header.process-stream-fn]
fn process_stream(common: &CommonOptions) -> i32 {
    // De-C-ified: open the input/output as std streams (resolved from the
    // filename fields by common.input_reader / output_writer, which honour the
    // "<stdin>"/"<stdout>" sentinels) and delegate the HFST3-header stripping to
    // hfst_input_stream::strip_hfst3_headers. The C printed "Stripping..." once
    // per byte under -v; that per-byte trace is dropped (diagnostic only — the
    // stripped output is unchanged).
    let input = match common.input_reader() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("hfst-strip-header: could not open input: {e}");
            return 1;
        }
    };
    let output = match common.output_writer() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("hfst-strip-header: could not open output: {e}");
            return 1;
        }
    };

    match strip_hfst3_headers(input, output) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("hfst-strip-header: error while stripping headers: {e}");
            1
        }
    }
}

// [spec:hfst:def:hfst-strip-header.main-fn]
// [spec:hfst:sem:hfst-strip-header.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstStripHeader");
    let (common, _args) = cli::parse::<Args>(common, args)?;
    verbose_print(
        &common,
        &format!(
            "Reading from {}, writing to {}\n",
            common.input_filename, common.output_filename
        ),
    );

    cli::from_code(process_stream(&common))
}
