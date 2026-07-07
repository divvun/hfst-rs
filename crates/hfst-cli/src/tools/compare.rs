//! Faithful 1:1 port of tools/src/hfst-compare.cc — the transducer comparison
//! command-line tool. Drives the hfst-cli foundation (getopt, commandline,
//! program-options, inc fragments). A binary tool: it reads from two input
//! streams (first + second) and writes a comparison log.
//!
//! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
//! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
//! `parse_options` and threaded into the processing functions. There are no
//! `static mut` globals and no `unsafe`.

use crate::binary_ops::open_two_input_streams;
use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, extend_options_from_env, hfst_set_program_name, hfst_strformat,
    is_input_stream_in_ol_format, verbose_print,
};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_binary_long, hfst_getopt_common_long, print_common_binary_program_options,
    print_common_binary_program_parameter_instructions, print_common_program_options,
};
use crate::inc::{
    CaseResult, check_binary_params, check_common_params, handle_binary_case, handle_common_case,
    handle_error_case,
};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_transducer::{AnyTransducer, HfstTransducer};
use std::io::Write;

/// hfst-compare's own options (the former tool-specific `static mut`s: C's
/// 'static bool harmonize=true; static bool eliminate_flags=false;').
struct Options {
    /// '-H, --do-not-harmonize': harmonize symbols before comparing.
    harmonize: bool,
    /// '-e, --eliminate-flags': eliminate flag diacritics before comparing.
    eliminate_flags: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            harmonize: true,
            eliminate_flags: false,
        }
    }
}

// [spec:hfst:def:hfst-compare.print-usage-fn]
// [spec:hfst:sem:hfst-compare.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    let mut msg = common.message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nCompare two transducers\n\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    print_common_binary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Harmonization:\n  -H, --do-not-harmonize Do not harmonize symbols.\n  -e, --eliminate-flags  Eliminate flag diacritics.\n"
    );
    let _ = write!(msg, "\n");
    print_common_binary_program_parameter_instructions(&mut *msg);
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "\nExamples:\n  $ {0} cat.hfst dog.hfst\n  cat.hfst[1] != dog.hfst[1]\n  $ {0} cat.hfst cat.hfst\n  cat.hfst[1] == cat.hfst[1]\n\n",
        common.program_name
    );
}

// [spec:hfst:def:hfst-compare.parse-options-fn]
// [spec:hfst:sem:hfst-compare.parse-options-fn]
//
// Parse argv into the shared + tool options; `Err(code)` is an exit code the
// caller should return (the former EXIT_CONTINUE sentinel is now `Ok`).
fn parse_options(
    mut common: CommonOptions,
    args: &mut Vec<String>,
) -> Result<(CommonOptions, Options), i32> {
    let mut options = Options::default();
    let mut opt = Getopt::new();
    extend_options_from_env(args);
    loop {
        let mut long_options: Vec<getopt::GetOpt> = Vec::new();
        long_options.extend(hfst_getopt_common_long());
        long_options.extend(hfst_getopt_binary_long());
        // add tool-specific options here
        long_options.push(getopt::GetOpt {
            name: "do-not-harmonize",
            has_arg: getopt::NO_ARGUMENT,
            val: 'H' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "eliminate-flags",
            has_arg: getopt::NO_ARGUMENT,
            val: 'e' as i32,
        });
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: common
        // cases, then binary cases, then the tool's own ('H'/'e'), then the
        // terminal error arm.
        match handle_common_case(&mut common, &opt, c, print_usage) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        match handle_binary_case(&mut common, &opt, c) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        match c as u8 as char {
            'H' => {
                options.harmonize = false;
                continue;
            }
            'e' => {
                options.eliminate_flags = true;
                continue;
            }
            _ => {}
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    check_common_params(&mut common);
    check_binary_params(&mut common, &opt, args);
    Ok((common, options))
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
            (AnyTransducer::Log(f), AnyTransducer::Log(s)) => {
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
                            let _ = write!(out, "{} == {}\n", firstname, secondname);
                        }
                    } else if !common.silent {
                        let _ = write!(
                            out,
                            "{}[{}] == {}[{}]\n",
                            firstname, transducer_n_first, secondname, transducer_n_second
                        );
                    }
                } else {
                    if transducer_n_first == 1 {
                        if !common.silent {
                            let _ = write!(out, "{} != {}\n", firstname, secondname);
                        }
                    } else if !common.silent {
                        let _ = write!(
                            out,
                            "{}[{}] != {}[{}]\n",
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
        if (continue_reading && secondstream.is_good()) || !continue_reading {
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
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstCompare");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    // close buffers, we use streams
    verbose_print(
        &common,
        &format!(
            "Reading from {} and {}, writing log to {}\n",
            common.first_filename, common.second_filename, common.output_filename
        ),
    );
    let (mut firststream, mut secondstream) = match open_two_input_streams(&common) {
        Ok(v) => v,
        Err(code) => return code,
    };

    if is_input_stream_in_ol_format(&firststream, "hfst-compare")
        || is_input_stream_in_ol_format(&secondstream, "hfst-compare")
    {
        return 1;
    }

    compare_streams(&common, &options, &mut firststream, &mut secondstream)
}
