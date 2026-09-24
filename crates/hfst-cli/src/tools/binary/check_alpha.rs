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
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_symbol_defs::StringSet;

use std::io::Write;

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

// [spec:hfst:def:hfst-check-alpha.fprint-stringset-fn]
// [spec:hfst:sem:hfst-check-alpha.fprint-stringset-fn]
fn fprint_stringset(outfile: &mut dyn Write, strings: &StringSet) {
    let mut first = true;
    for s in strings {
        if !first {
            let _ = write!(outfile, ", ");
        }
        let _ = write!(outfile, "{}", s);
        first = false;
    }
}

// [spec:hfst:def:hfst-check-alpha.process-stream-fn]
// [spec:hfst:sem:hfst-check-alpha.process-stream-fn]
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
    let mut continue_reading = firststream.is_good() && secondstream.is_good();
    let mut transducer_n: usize = 0;
    let mut mismatch = 0;
    while continue_reading {
        transducer_n += 1;

        if transducer_n < 2 {
            verbose_print(common, "Checking alphas...\n");
        } else {
            verbose_print(common, &format!("Checking alphas... {}\n", transducer_n));
        }
        // read first alphas
        let first = match firststream.read() {
            Ok(t) => t,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        // one dispatch per read ([dec:hfst:monomorphic-backends]); the
        // alphabet queries are backend-independent values.
        let (mutt, first_transducer_alphabet): (HfstBasicTransducer, StringSet) = crate::for_any!(&first, t => {
            let mutt = match HfstBasicTransducer::try_from_transducer(t) {
                Ok(m) => m,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            let alpha = match t.get_alphabet() {
                Ok(a) => a,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            (mutt, alpha)
        });
        let transducer_knows_alphabet = true;
        let first_found_alphabet: StringSet = mutt.symbols_used();
        // read second alphas
        let second = match secondstream.read() {
            Ok(t) => t,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        let (secondmutt, second_transducer_alphabet): (HfstBasicTransducer, StringSet) = crate::for_any!(&second, t => {
            let mutt = match HfstBasicTransducer::try_from_transducer(t) {
                Ok(m) => m,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            let alpha = match t.get_alphabet() {
                Ok(a) => a,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            (mutt, alpha)
        });
        let second_found_alphabet: StringSet = secondmutt.symbols_used();
        // match
        let _ = writeln!(out, "Actual alphabet differences:");
        let first_minus_second: StringSet = first_found_alphabet
            .difference(&second_found_alphabet)
            .cloned()
            .collect();
        if !first_minus_second.is_empty() {
            mismatch = 1;
            let _ = write!(
                out,
                "In first {} but not in second {}:",
                first.get_name(),
                second.get_name()
            );
            fprint_stringset(&mut *out, &first_minus_second);
        } else {
            let _ = write!(
                out,
                "First {} alpha is superset of second {}.",
                first.get_name(),
                second.get_name()
            );
        }
        let _ = writeln!(out);
        let second_minus_first: StringSet = second_found_alphabet
            .difference(&first_found_alphabet)
            .cloned()
            .collect();
        if !second_minus_first.is_empty() {
            mismatch = 1;
            let _ = write!(
                out,
                "In second {} but not in first {}:",
                second.get_name(),
                second.get_name()
            );
            fprint_stringset(&mut *out, &second_minus_first);
        } else {
            let _ = write!(
                out,
                "Second {} alpha is superset of second {}.",
                second.get_name(),
                second.get_name()
            );
        }
        let _ = writeln!(out);
        if common.verbose {
            let _ = write!(out, "{} alphabet:", first.get_name());
            fprint_stringset(&mut *out, &first_found_alphabet);
            let _ = writeln!(out);
            let _ = write!(out, "{} alphabet:", second.get_name());
            fprint_stringset(&mut *out, &second_found_alphabet);
            let _ = writeln!(out);
        }
        if transducer_knows_alphabet {
            let _ = writeln!(out, "sigma set difference:");
            let first_minus_second: StringSet = first_transducer_alphabet
                .difference(&second_transducer_alphabet)
                .cloned()
                .collect();
            let second_minus_first: StringSet = second_transducer_alphabet
                .difference(&first_transducer_alphabet)
                .cloned()
                .collect();
            if !first_minus_second.is_empty() {
                mismatch = 1;
                let _ = write!(
                    out,
                    "First {} has but second {} does not: ",
                    first.get_name(),
                    second.get_name()
                );
                fprint_stringset(&mut *out, &first_minus_second);
            } else {
                let _ = write!(
                    out,
                    "First {} alpha is superset of second {}.",
                    first.get_name(),
                    second.get_name()
                );
            }
            let _ = writeln!(out);
            if !second_minus_first.is_empty() {
                mismatch = 1;
                let _ = write!(
                    out,
                    "Second {} has but first {} does not: ",
                    second.get_name(),
                    first.get_name()
                );
                fprint_stringset(&mut *out, &second_minus_first);
            } else {
                let _ = write!(
                    out,
                    "Second {} alpha is superset of first {}.",
                    second.get_name(),
                    first.get_name()
                );
            }
            let _ = writeln!(out);
            if common.verbose {
                let _ = write!(out, "First ({}):", first.get_name());
                fprint_stringset(&mut *out, &first_transducer_alphabet);
                let _ = writeln!(out);
                let _ = write!(out, "Second ({}):", second.get_name());
                fprint_stringset(&mut *out, &second_transducer_alphabet);
                let _ = writeln!(out);
            }
        } else {
            let _ = writeln!(out, "No internal alphabets to compare in this format");
        } // FSTs know their alphas
        continue_reading = firststream.is_good() && secondstream.is_good();
    }

    let _ = write!(out, "\nRead {} transducers in total.\n", transducer_n);
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
