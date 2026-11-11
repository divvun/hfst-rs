//! Faithful 1:1 port of tools/src/hfst-kill-paths.cc — the path-killing
//! command-line tool: removes every arc whose input or output symbol matches a
//! given symbol (one --symbol, or a list from a --tsv-file), then removes
//! epsilons. Option handling is clap 4 derive through [`crate::cli`].

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, hfst_set_program_name, is_input_stream_in_ol_format, verbose_print,
};
use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, unary_streams};
use hfst::backend::AlgebraBackend;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst::tools::kill_paths::{kill_paths, kill_symbols};
use std::io::{BufRead, BufReader};

/// hfst-kill-paths's command line. Upstream spells the long name of -T
/// "tsv" while its help calls it --tsv-file; the registered name is the
/// one that parses, so it is kept.
// [spec:hfst:def:hfst-kill-paths.parse-options-fn]
// [spec:hfst:sem:hfst-kill-paths.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Kill all paths with specific symbols")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Remove arcs with input or output symbol SYM or both
    #[arg(short = 'S', long = "symbol", value_name = "SYM")]
    symbol: Option<String>,

    /// Read kill rules from TFILE, which should contain lines with
    /// tab-separated pairs of SYM; comment lines starting with # and empty
    /// lines are ignored
    #[arg(short = 'T', long = "tsv", value_name = "TFILE")]
    tsv: Option<String>,
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        self.io.apply(opts);
    }

    fn validate(&self, opts: &CommonOptions) -> ToolResult {
        // The C ran this check after the getopt loop but BEFORE the
        // parameter checks, so it outranks the too-many-files diagnostics.
        if self.symbol.is_none() && self.tsv.is_none() {
            error(opts, 1, 0, "Either --symbol or --tsv-file is required");
            return Err(1);
        }
        Ok(())
    }
}

/// The tool's option-driven state once the kill-rules file, which the C
/// opened after the parameter checks, has been read.
struct Options {
    /// '-S, --symbol=SYM': the symbol whose arcs to kill.
    symbol: Option<String>,
    /// '-T, --tsv-file=TFILE': the file listing kill symbols, and its text.
    tsv: Option<(String, String)>,
}

impl Options {
    fn open(args: &Args, common: &CommonOptions) -> Result<Options, i32> {
        let tsv = match &args.tsv {
            Some(name) => match read_lines(name) {
                Ok(text) => Some((name.clone(), text)),
                Err(_) => {
                    error(common, 1, 0, &format!("Could not open '{}'", name));
                    return Err(1);
                }
            },
            None => None,
        };
        Ok(Options {
            symbol: args.symbol.clone(),
            tsv,
        })
    }
}

/// The lines of the file `name`, read up to the first that is not UTF-8.
fn read_lines(name: &str) -> std::io::Result<String> {
    let mut reader = BufReader::new(std::fs::File::open(name)?);
    let mut text = String::new();
    while reader.read_line(&mut text).unwrap_or(0) > 0 {}
    Ok(text)
}

// [spec:hfst:def:hfst-kill-paths.original-fn]
// [spec:hfst:sem:hfst-kill-paths.original-fn]
// [spec:hfst:req:embed.tools-in-library]
// The killing and naming are the library's; the stream loop is the shared
// unary driver's.
impl UnaryToolOp for Options {
    fn verbose_begin(&self, inputname: &str) -> String {
        format!("Path killing {}", inputname)
    }

    fn apply<B: AlgebraBackend>(
        &mut self,
        common: &CommonOptions,
        t: &mut HfstTransducer<B>,
    ) -> hfst::error::Result<()> {
        let unkilled = std::mem::replace(t, HfstTransducer::new());
        *t = match &self.tsv {
            None => kill_paths(
                unkilled,
                [self.symbol.as_deref().unwrap_or_default()],
                &mut |_| {},
            )?,
            Some((name, text)) => {
                verbose_print(common, &format!("Reading reweights from {}\n", name));
                let symbols = kill_symbols(text);
                kill_paths(
                    unkilled,
                    symbols.iter().map(String::as_str),
                    &mut |symbol| {
                        verbose_print(common, &format!("Killing patsh with symbol {}\n", symbol));
                    },
                )?
            }
        };
        Ok(())
    }
}

const SPEC: UnaryOpSpec = UnaryOpSpec {
    tool_name: "hfst-kill-paths",
    reject_ol: true,
};

// [spec:hfst:def:hfst-kill-paths.main-fn]
// [spec:hfst:sem:hfst-kill-paths.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstKillPaths");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let mut options = Options::open(&args, &common)?;

    // close buffers, we use streams
    let input_opened = common.input_filename != "<stdin>";
    let output_opened = common.output_filename != "<stdout>";
    verbose_print(
        &common,
        &format!(
            "Reading from {}, writing to {}\n",
            common.input_filename, common.output_filename
        ),
    );
    verbose_print(&common, "Killing paths\n");
    if let Some(sym) = &options.symbol {
        verbose_print(&common, &format!("only if arc has symbol {}\n", sym));
    }

    // here starts the buffer handling part
    let mut instream = match if input_opened {
        HfstInputStream::new_filename(&common.input_filename)
    } else {
        HfstInputStream::new()
    } {
        Ok(s) => s,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return Err(1);
        }
    };
    // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
    // currently panics on a bad file rather than throwing, so the catch arm
    // is not reproduced here.)

    let ty = instream.get_type();
    let mut outstream = match if output_opened {
        HfstOutputStream::new_filename(&common.output_filename, ty, true)
    } else {
        HfstOutputStream::new(ty, true)
    } {
        Ok(s) => s,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return Err(1);
        }
    };

    if is_input_stream_in_ol_format(&instream, "hfst-kill-paths") {
        return Err(1);
    }

    cli::from_code(unary_streams(
        &common,
        &SPEC,
        &mut options,
        &mut instream,
        &mut outstream,
    ))
}
