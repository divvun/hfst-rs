//! Faithful 1:1 port of tools/src/hfst-split.cc — the transducer archive
//! exploding tool.
//!
//! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
//! lives in [`CommonOptions`] (the shared `-v/-q/-o/-i/…` fields) and a
//! tool-local [`Options`], threaded into the processing functions. There are
//! no `static mut` globals and no `unsafe`.

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{error, hfst_set_program_name, print_short_help, verbose_print};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;

/// hfst-split's own options (the former tool-specific `static mut`s).
struct Options {
    /// '-p, --prefix=PRE': prefix used in naming output files.
    prefix: String,
    /// '-e, --extension=EXT': extension used in naming output files.
    extension: String,
}

/// hfst-split's command line.
//
// '-o' is REFUSED, not ignored: the tool names its own output files from
// PRE + N + EXT and its option table never carried the output option, so
// the shared common group's '-o' is rejected in `validate` the way the C's
// error arm rejected the unknown letter.
// [spec:hfst:def:hfst-split.parse-options-fn]
// [spec:hfst:sem:hfst-split.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(
    about = "Extract transducers from archive with systematic file names",
    after_help = "If INFILE is omitted or -, stdin is used.
If PRE is omitted, no prefix is used.
If EXT is omitted, .hfst is used.
-o/--output is not accepted: this tool names its own output files.
The extracted files are named \"PRE\" + N + \"EXT\", where N is the number of the transducer in the archive.

An example:
   cat transducer_a transducer_b | hfst-split -p \"rule\" -e \".tr\"

This command creates files \"rule1.tr\" (equivalent to transducer_a) and \"rule2.tr\" (equivalent to transducer_b)."
)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Use the prefix PRE in naming output files
    #[arg(
        short = 'p',
        long = "prefix",
        value_name = "PRE",
        allow_hyphen_values = true
    )]
    prefix: Option<String>,

    /// Use the extension EXT in naming output files
    #[arg(
        short = 'e',
        long = "extension",
        value_name = "EXT",
        allow_hyphen_values = true
    )]
    extension: Option<String>,
}

impl Args {
    fn options(&self) -> Options {
        Options {
            prefix: self.prefix.clone().unwrap_or_default(),
            extension: self
                .extension
                .clone()
                .unwrap_or_else(|| ".hfst".to_string()),
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
        // hfst-split's option table has no output option; the C's error
        // arm answered '-o' with the unknown-option refusal.
        if self.common.output.is_some() {
            print_short_help(opts);
            error(opts, 1, 0, "Unknown option `-o'.\n");
        }
        // This tool writes its own 'i' case rather than taking the shared
        // one: it opened INFILE eagerly (hfst_fopen) inside the getopt
        // loop, so an unreadable name is refused here, before the
        // parameter checks.
        if let Some(name) = self.io.input.as_deref()
            && name != "-"
            && std::fs::File::open(name).is_err()
        {
            error(opts, 1, 0, &format!("Could not open '{}'. ", name));
        }
        Ok(())
    }
}

// [spec:hfst:def:hfst-split.process-stream-fn]
// [spec:hfst:sem:hfst-split.process-stream-fn]
fn process_stream(
    common: &mut CommonOptions,
    options: &Options,
    instream: &mut HfstInputStream<'_>,
) -> i32 {
    let mut transducer_n: usize = 0;
    while instream.is_good() {
        transducer_n += 1;
        let outfilename = format!("{}{}{}", options.prefix, transducer_n, options.extension);
        common.output_filename = outfilename.clone();
        verbose_print(
            common,
            &format!(
                "Writing {} of {} to {}...\n",
                transducer_n, common.input_filename, outfilename
            ),
        );
        let mut outstream =
            match HfstOutputStream::new_filename(&outfilename, instream.get_type(), true) {
                Ok(s) => s,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
        let any = match instream.read() {
            Ok(t) => t,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
        crate::for_any!(any, trans => {
            let mut trans = trans;
            if let Err(e) = outstream.redirect(&mut trans) {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
            if let Err(e) = outstream.flush() {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
            outstream.close();
            common.output_filename = String::new();
        });
    }
    instream.close();
    0
}

// [spec:hfst:def:hfst-split.main-fn]
// [spec:hfst:sem:hfst-split.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstSplit");
    let (mut common, args) = cli::parse::<Args>(common, args)?;
    let options = args.options();

    // close buffers, we use streams
    verbose_print(
        &common,
        &format!(
            "Reading from {}, writing to {}...{}\n",
            common.input_filename, options.prefix, options.extension
        ),
    );
    // here starts the buffer handling part
    // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
    // currently panics on a bad file rather than throwing, so the catch arm
    // is not reproduced faithfully here.)
    let instream_result = if common.input_filename != "<stdin>" {
        HfstInputStream::new_filename(&common.input_filename)
    } else {
        HfstInputStream::new()
    };
    let mut instream = match instream_result {
        Ok(s) => s,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return Err(1);
        }
    };

    cli::from_code(process_stream(&mut common, &options, &mut instream))
}
