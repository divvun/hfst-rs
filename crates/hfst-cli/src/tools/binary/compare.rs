//! Faithful 1:1 port of tools/src/hfst-compare.cc — the transducer comparison
//! command-line tool. A binary tool: it reads from two input streams (first +
//! second) and writes a comparison log.
//!
//! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
//! lives in [`CommonOptions`] (the shared `-v/-q/-o/-i/…` fields) and the
//! parsed [`Args`], threaded into the processing functions. There are no
//! `static mut` globals and no `unsafe`.

use crate::binary_ops::open_two_input_streams;
use crate::cli::{self, BinaryIo, CommonArgs, ToolArgs, ToolResult};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, hfst_set_program_name, hfst_strformat, is_input_stream_in_ol_format, verbose_print,
};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_transducer::{AnyTransducer, HfstTransducer};
use std::io::Write;

/// hfst-compare's command line (the C's 'static bool harmonize=true;
/// static bool eliminate_flags=false;' pair, now negated flags).
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(
    about = "Compare two transducers",
    after_help = "Examples:
  $ hfst-compare cat.hfst dog.hfst
  cat.hfst[1] != dog.hfst[1]
  $ hfst-compare cat.hfst cat.hfst
  cat.hfst[1] == cat.hfst[1]"
)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: BinaryIo,

    /// Do not harmonize symbols
    #[arg(short = 'H', long = "do-not-harmonize")]
    do_not_harmonize: bool,

    /// Eliminate flag diacritics
    #[arg(short = 'e', long = "eliminate-flags")]
    eliminate_flags: bool,
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        self.io.apply(opts);
    }
}

/// The per-pair knobs `compare_pair` reads.
struct Options {
    /// '-H, --do-not-harmonize' clears this: harmonize symbols before comparing.
    harmonize: bool,
    /// '-e, --eliminate-flags': eliminate flag diacritics before comparing.
    eliminate_flags: bool,
}

// [spec:hfst:def:hfst-compare.compare-streams-fn]
// [spec:hfst:sem:hfst-compare.compare-streams-fn]
fn compare_streams(
    common: &CommonOptions,
    options: &Options,
    firststream: &mut HfstInputStream<'_>,
    secondstream: &mut HfstInputStream<'_>,
) -> i32 {
    let mut out = match common.output_writer() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("hfst-compare: cannot open output: {e}");
            return 1;
        }
    };
    let mut continue_reading = firststream.is_good() && secondstream.is_good();
    let mut transducer_n_first: usize = 0; // transducers read from first input
    let mut transducer_n_second: usize = 0; // transducers read from second input
    let mut mismatches: usize = 0;

    let mut second: Option<AnyTransducer> = None;

    while continue_reading {
        let mut first = match firststream.read() {
            Ok(v) => v,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        transducer_n_first += 1;
        if secondstream.is_good() {
            second = Some(match secondstream.read() {
                Ok(v) => v,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            });
            transducer_n_second += 1;
        }
        let mut firstname = first.get_name();
        // make scan-build happy, this should not happen
        let second_ref = match second.as_mut() {
            Some(s) => s,
            None => panic!("Error: second stream has a NULL value."),
        };
        let mut secondname = second_ref.get_name();
        if firstname.is_empty() {
            firstname = common.first_filename.clone();
        }
        if secondname.is_empty() {
            secondname = common.second_filename.clone();
        }
        if transducer_n_first == 1 {
            verbose_print(
                common,
                &format!("Comparing {} and {}...\n", firstname, secondname),
            );
        } else {
            verbose_print(
                common,
                &format!(
                    "Comparing {} and {}... {}\n",
                    firstname, secondname, transducer_n_first
                ),
            );
        }
        // C: try { ... } catch (TransducerTypeMismatchException). Same-
        // backend operands are a compile-time property of the generic
        // body now, so the mismatch is this boundary's fall-through arm
        // ([dec:hfst:monomorphic-backends]).
        let outcome = match (&mut first, second_ref) {
            (AnyTransducer::Tropical(f), AnyTransducer::Tropical(s)) => {
                Some(compare_pair(common, options, f, s))
            }
            #[cfg(feature = "foma")]
            (AnyTransducer::Foma(f), AnyTransducer::Foma(s)) => {
                Some(compare_pair(common, options, f, s))
            }
            _ => None,
        };
        match outcome {
            Some(Ok(equal)) => {
                if equal {
                    if transducer_n_first == 1 {
                        if !common.silent {
                            let _ = writeln!(out, "{} == {}", firstname, secondname);
                        }
                    } else if !common.silent {
                        let _ = writeln!(
                            out,
                            "{}[{}] == {}[{}]",
                            firstname, transducer_n_first, secondname, transducer_n_second
                        );
                    }
                } else {
                    if transducer_n_first == 1 {
                        if !common.silent {
                            let _ = writeln!(out, "{} != {}", firstname, secondname);
                        }
                    } else if !common.silent {
                        let _ = writeln!(
                            out,
                            "{}[{}] != {}[{}]",
                            firstname, transducer_n_first, secondname, transducer_n_second
                        );
                    }
                    mismatches += 1;
                }
            }
            Some(Err(e)) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
            None => {
                // cannot recover yet, but beautify error messages
                error(
                    common,
                    2,
                    0,
                    &format!(
                        "Cannot compare `{}' and `{}' [{}]\nthe formats {} and {} are not compatible for comparison",
                        firstname,
                        secondname,
                        transducer_n_first,
                        hfst_strformat(firststream.get_type()),
                        hfst_strformat(secondstream.get_type())
                    ),
                );
            }
        }

        continue_reading =
            firststream.is_good() && (secondstream.is_good() || transducer_n_second == 1);

        // delete the transducer of second stream, unless we continue reading
        // the first stream and there is only one transducer in the second
        // stream
        if secondstream.is_good() || !continue_reading {
            second = None;
        }
    }

    if firststream.is_good() {
        error(
            common,
            1,
            0,
            &format!(
                "second input '{}' contains fewer transducers than first input '{}'; this is only possible if the second input contains exactly one transducer",
                common.second_filename, common.first_filename
            ),
        );
    } else if secondstream.is_good() {
        error(
            common,
            1,
            0,
            &format!(
                "first input '{}' contains fewer transducers than second input '{}'",
                common.first_filename, common.second_filename
            ),
        );
    }
    firststream.close();
    secondstream.close();
    let _ = out.flush();
    if mismatches == 0 {
        verbose_print(
            common,
            &format!("All {} transducers matched\n", transducer_n_first),
        );
        0
    } else {
        verbose_print(
            common,
            &format!("{}/{} were not equal\n", mismatches, transducer_n_first),
        );
        1
    }
}

// The monomorphic per-pair comparison body (flag elimination + compare).
fn compare_pair<B: hfst::backend::AlgebraBackend>(
    common: &CommonOptions,
    options: &Options,
    first: &mut HfstTransducer<B>,
    second: &mut HfstTransducer<B>,
) -> hfst::error::Result<bool> {
    if options.eliminate_flags {
        verbose_print(common, "Eliminating flags...\n");
        first.eliminate_flags()?;
        second.eliminate_flags()?;
    }
    first.compare(second, options.harmonize)
}

// [spec:hfst:def:hfst-compare.main-fn]
// [spec:hfst:sem:hfst-compare.main-fn]
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstCompare");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let options = Options {
        harmonize: !args.do_not_harmonize,
        eliminate_flags: args.eliminate_flags,
    };

    // close buffers, we use streams
    verbose_print(
        &common,
        &format!(
            "Reading from {} and {}, writing log to {}\n",
            common.first_filename, common.second_filename, common.output_filename
        ),
    );
    let (mut firststream, mut secondstream) = open_two_input_streams(&common)?;

    if is_input_stream_in_ol_format(&firststream, "hfst-compare")
        || is_input_stream_in_ol_format(&secondstream, "hfst-compare")
    {
        return Err(1);
    }

    cli::from_code(compare_streams(
        &common,
        &options,
        &mut firststream,
        &mut secondstream,
    ))
}
