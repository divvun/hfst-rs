//! Faithful 1:1 port of tools/src/hfst-check-alpha.cc — the tool that compares
//! the compatibility of alphabets within and between automata. A binary tool
//! (two input streams).
//!
//! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
//! lives in [`CommonOptions`] (the shared `-v/-q/-1/-2/…` fields), built from
//! the parsed [`Args`] and threaded into the processing functions. There are no
//! `static mut` globals and no `unsafe`.

use crate::cli::{self, BinaryIo, CommonArgs, ToolArgs, ToolResult};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{error, hfst_set_program_name, verbose_print};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::tools::check_alpha::AlphabetComparison;
use hfst::tools::summarize::write_total;

/// hfst-check-alpha's command line. The tool declares no options of its own
/// (its C usage printed an empty "Check alpha options:" heading).
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Compare the compatibility of alphabets between INFILEs")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: BinaryIo,
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        self.io.apply(opts);
    }
}

// [spec:hfst:req:embed.tools-in-library]
// The comparison and its report are the library's; the tool reads the two
// streams pair by pair and writes the reports.
fn process_stream(
    common: &CommonOptions,
    firststream: &mut HfstInputStream<'_>,
    secondstream: &mut HfstInputStream<'_>,
) -> i32 {
    let mut out = match common.output_writer() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("hfst-check-alpha: cannot open output: {e}");
            return 1;
        }
    };
    let mut transducer_n: usize = 0;
    let mut mismatch = 0;
    while firststream.is_good() && secondstream.is_good() {
        transducer_n += 1;
        if transducer_n < 2 {
            verbose_print(common, "Checking alphas...\n");
        } else {
            verbose_print(common, &format!("Checking alphas... {}\n", transducer_n));
        }
        let compared = firststream.read().and_then(|first| {
            let second = secondstream.read()?;
            AlphabetComparison::of(&first, &second)
        });
        match compared {
            Ok(comparison) => {
                if comparison.mismatch() {
                    mismatch = 1;
                }
                let _ = comparison.write_report(&mut *out, common.verbose);
            }
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        }
    }
    let _ = write_total(&mut *out, transducer_n);
    mismatch
}

// [spec:hfst:def:hfst-check-alpha.main-fn]
// [spec:hfst:sem:hfst-check-alpha.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstALphaFix");
    let (common, _args) = cli::parse::<Args>(common, args)?;

    // close buffers, we use streams
    let first_opened = common.first_filename != "<stdin>";
    let second_opened = common.second_filename != "<stdin>";
    verbose_print(
        &common,
        &format!(
            "Reading from {} and {}, writing to {}\n",
            common.first_filename, common.second_filename, common.output_filename
        ),
    );
    // here starts the buffer handling part
    // (the C wraps each ctor in try/catch on HfstException, calling error()
    // and returning EXIT_FAILURE; the Rust ctors now return a Result, so the
    // error path and message are preserved via a match on that Result.)
    let firststream = if first_opened {
        let name = common.first_filename.clone();
        match HfstInputStream::new_filename(&name) {
            Ok(s) => s,
            Err(_) => {
                error(
                    &common,
                    1,
                    0,
                    &format!("{} is not a valid transducer file", name),
                );
                return Err(1);
            }
        }
    } else {
        match HfstInputStream::new() {
            Ok(s) => s,
            Err(_) => {
                error(
                    &common,
                    1,
                    0,
                    &format!("{} is not a valid transducer file", common.first_filename),
                );
                return Err(1);
            }
        }
    };
    let secondstream = if second_opened {
        let name = common.second_filename.clone();
        match HfstInputStream::new_filename(&name) {
            Ok(s) => s,
            Err(_) => {
                error(
                    &common,
                    1,
                    0,
                    &format!("{} is not a valid transducer file", name),
                );
                return Err(1);
            }
        }
    } else {
        match HfstInputStream::new() {
            Ok(s) => s,
            Err(_) => {
                error(
                    &common,
                    1,
                    0,
                    &format!("{} is not a valid transducer file", common.second_filename),
                );
                return Err(1);
            }
        }
    };
    let mut firststream = firststream;
    let mut secondstream = secondstream;

    let _retval = process_stream(&common, &mut firststream, &mut secondstream);

    Ok(())
}
