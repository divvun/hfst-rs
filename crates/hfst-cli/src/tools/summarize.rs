//! Port of tools/src/hfst-summarize.cc — the transducer properties
//! command-line tool, around the library's [`hfst::tools::summarize`].
//! Option handling is clap 4 derive through [`crate::cli`].

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{error, hfst_set_program_name, parse_u64, verbose_print};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::tools::summarize::{Summary, SummaryOptions, write_total};

/// hfst-summarize's command line.
// [spec:hfst:def:hfst-summarize.parse-options-fn]
// [spec:hfst:sem:hfst-summarize.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Calculate the properties of a transducer")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Print info about symbol pairs that occur at most N times (default is
    /// infinity)
    #[arg(
        short = 'S',
        long = "print-symbol-pair-statistics",
        value_name = "N",
        num_args = 0..=1,
        require_equals = true,
        allow_hyphen_values = true
    )]
    print_symbol_pair_statistics: Option<Option<String>>,
}

impl Args {
    /// Case 'S'. The C read an OPTIONAL_ARGUMENT, so the flag alone means "no
    /// threshold" (-1, which the unsigned comparison in the report turns into
    /// "every pair"); a supplied N is parsed with strtoul. The leading '=' the
    /// C's getopt left on an attached value is stripped the same way, so
    /// '-S=5' and '-S5' both read 5.
    fn symbol_pair_threshold(&self, common: &CommonOptions) -> i32 {
        let Some(Some(value)) = self.print_symbol_pair_statistics.as_ref() else {
            return -1;
        };
        let value = value.strip_prefix('=').unwrap_or(value);
        let threshold = parse_u64(common, value, 10) as i32;
        if threshold < 0 {
            error(
                common,
                1,
                0,
                &format!(
                    "{} is not a valid argument for option --print-symbol-pair-statistics\n",
                    threshold as u32
                ),
            );
        }
        if threshold == 0 {
            error(
                common,
                1,
                0,
                "0 is not a valid argument for option --print-symbol-pair-statistics\n",
            );
        }
        threshold
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
        // The threshold rejections happened inside the C getopt loop, before
        // the parameter checks; run them here for the same ordering.
        self.symbol_pair_threshold(opts);
        Ok(())
    }
}

// [spec:hfst:req:embed.tools-in-library]
// The summary and its report are the library's; the tool reads the stream
// and writes the report.
fn process_stream(
    common: &CommonOptions,
    options: &SummaryOptions,
    instream: &mut HfstInputStream<'_>,
) -> i32 {
    let mut out = match common.output_writer() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("hfst-summarize: cannot open output: {e}");
            return 1;
        }
    };
    let mut transducer_n: usize = 0;
    while instream.is_good() {
        transducer_n += 1;
        if transducer_n < 2 {
            verbose_print(common, "Summarizing...\n");
        } else {
            verbose_print(common, &format!("Summarizing... {}\n", transducer_n));
        }
        let summary = instream
            .read()
            .and_then(|t| Summary::of(&t, common.verbose));
        match summary {
            Ok(summary) => {
                let _ = summary.write_report(&mut *out, transducer_n, options);
            }
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        }
    }
    let _ = write_total(&mut *out, transducer_n);
    0
}

// [spec:hfst:def:hfst-summarize.main-fn]
// [spec:hfst:sem:hfst-summarize.main-fn]
pub(super) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstSummarize");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let options = SummaryOptions {
        verbose: common.verbose,
        symbol_pairs: args
            .print_symbol_pair_statistics
            .is_some()
            .then(|| args.symbol_pair_threshold(&common)),
    };
    // close buffers, we use streams
    let input_opened = common.input_filename != "<stdin>";
    verbose_print(
        &common,
        &format!(
            "Reading from {}, writing to {}\n",
            common.input_filename, common.output_filename
        ),
    );
    // here starts the buffer handling part
    // (the C wraps the ctor in try/catch on HfstException; on a bad file the
    // Rust ctor currently panics rather than throwing, so the catch arm that
    // reports "%s is not a valid transducer file" is not reproduced here.)
    let instream_result = if input_opened {
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
    let retval = process_stream(&common, &options, &mut instream);
    let _ = retval;
    Ok(())
}
