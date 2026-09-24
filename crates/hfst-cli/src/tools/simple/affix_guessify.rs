//! Faithful 1:1 port of tools/src/hfst-affix-guessify.cc — the transducer
//! guesser maker command-line tool. Creates a weighted affix guesser from an
//! automaton. Option handling is clap 4 derive through [`crate::cli`].

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, hfst_set_program_name, hfst_strtoweight, is_input_stream_in_ol_format, verbose_print,
};
use hfst::guessify_fst::{GuessDirection, affix_guessify};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use std::io::Write;

/// hfst-affix-guessify's command line.
//
// GuessDirection and the per-transducer affix-guesser construction live in
// hfst::guessify_fst; this tool keeps only the option-driven state + the
// stream-driver loop.
// [spec:hfst:def:hfst-affix-guessify.parse-options-fn]
// [spec:hfst:sem:hfst-affix-guessify.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Create weighted affix guesser from automaton")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Set direction of guessing: suffix or prefix, suffix if omitted
    #[arg(short = 'D', long = "direction", value_name = "DIR")]
    direction: Option<String>,

    /// Set weight difference of affix lengths: the weight of each arc not
    /// in the known suffix or prefix being guessed, as parsed with
    /// strtod(3), or 1.0 if omitted
    #[arg(
        short = 'w',
        long = "weight",
        value_name = "WEIGHT",
        allow_hyphen_values = true
    )]
    weight: Option<String>,
}

impl Args {
    /// Case 'D': the C accepts any argument that STARTS WITH "prefix" or
    /// "suffix" and rejects everything else.
    fn direction(&self, common: &CommonOptions) -> GuessDirection {
        let Some(dir) = self.direction.as_deref() else {
            return GuessDirection::GuessSuffix;
        };
        if dir.starts_with("prefix") {
            GuessDirection::GuessPrefix
        } else if dir.starts_with("suffix") {
            GuessDirection::GuessSuffix
        } else {
            error(
                common,
                1,
                0,
                &format!(
                    "Unable to parse guessing direction from {};\nplease use one of 'prefix' or 'suffix'",
                    dir
                ),
            );
            GuessDirection::GuessSuffix
        }
    }

    /// Case 'w': strtod, fatal on anything else; the C initialiser is 1.0.
    fn weight(&self, common: &CommonOptions) -> f32 {
        match &self.weight {
            Some(w) => hfst_strtoweight(common, w),
            None => 1.0f32,
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
        // Both rejections happened inside the C getopt loop, before the
        // parameter checks; run them here for the same ordering.
        self.weight(opts);
        self.direction(opts);
        Ok(())
    }
}

// [spec:hfst:def:hfst-affix-guessify.process-stream-fn]
// [spec:hfst:sem:hfst-affix-guessify.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    direction: GuessDirection,
    weight: f32,
    instream: &mut HfstInputStream<'_>,
    outstream: &mut HfstOutputStream,
) -> i32 {
    let mut transducer_n: usize = 0;
    while instream.is_good() {
        transducer_n += 1;
        let any = match instream.read() {
            Ok(v) => v,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
        crate::for_algebra!(any, trans => {
            // C: inputname = trans->get_name(); if empty, use inputfilename.
            let inputname = if !trans.get_name().is_empty() {
                trans.get_name()
            } else {
                common.input_filename.clone()
            };
            if transducer_n < 2 {
                verbose_print(common, &format!("Guessifying {}...\n", inputname));
            } else {
                verbose_print(common, &format!("Guessifying {}... {}\n", inputname, transducer_n));
            }
            let mut t = match affix_guessify(&trans, direction, weight) {
                Ok(t) => t,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            if let Err(e) = outstream.write(&mut t) {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        }, else => {
            // Unreachable: the optimized-lookup stream rejection already
            // returned before the loop; keep its text for safety.
            let _ = writeln!(
                std::io::stderr(),
                "Error: hfst-affix-guessify cannot process transducers that are in optimized lookup format."
            );
            return 1;
        });
    } // good instream
    0
}

// [spec:hfst:def:hfst-affix-guessify.main-fn]
// [spec:hfst:sem:hfst-affix-guessify.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstAffixGuessify");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let direction = args.direction(&common);
    let weight = args.weight(&common);

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

    // here starts the buffer handling part
    // (the C wraps the ctor in try/catch on HfstException reporting
    // "%s is not a valid transducer file"; the Rust ctor currently panics on
    // a bad file rather than throwing, so the catch arm is not reproduced.)
    let instream_res = if input_opened {
        HfstInputStream::new_filename(&common.input_filename)
    } else {
        HfstInputStream::new()
    };
    let mut instream = match instream_res {
        Ok(s) => s,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return Err(1);
        }
    };

    let ty = instream.get_type();
    let outstream_res = if output_opened {
        HfstOutputStream::new_filename(&common.output_filename, ty, true)
    } else {
        HfstOutputStream::new(ty, true)
    };
    let mut outstream = match outstream_res {
        Ok(s) => s,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return Err(1);
        }
    };

    if is_input_stream_in_ol_format(&instream, "hfst-affix-guessify") {
        return Err(1);
    }

    cli::from_code(process_stream(
        &common,
        direction,
        weight,
        &mut instream,
        &mut outstream,
    ))
}
