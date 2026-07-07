//! Faithful 1:1 port of tools/src/hfst-affix-guessify.cc — the transducer
//! guesser maker command-line tool. Creates a weighted affix guesser from an
//! automaton. Drives the hfst-cli foundation (globals, getopt, commandline,
//! program-options, tool-metadata, inc fragments).

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, extend_options_from_env, hfst_set_program_name, hfst_strtoweight,
    is_input_stream_in_ol_format, verbose_print,
};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use hfst::guessify_fst::{GuessDirection, affix_guessify};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use std::io::Write;

/// hfst-affix-guessify's own options (the former tool-specific `static mut`s).
///
/// GuessDirection and the per-transducer affix-guesser construction now live in
/// hfst::guessify_fst; this tool keeps only the option-driven state + the
/// stream-driver loop.
struct Options {
    /// '-D, --direction=DIR': direction of guessing.
    direction: GuessDirection,
    /// '-w, --weight=WEIGHT': weight difference of affix lengths.
    weight: f32,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            direction: GuessDirection::GuessSuffix,
            weight: 1.0f32,
        }
    }
}

// [spec:hfst:def:hfst-affix-guessify.print-usage-fn]
// [spec:hfst:sem:hfst-affix-guessify.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nCreate weighted affix guesser from automaton\n\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    // (tool-specific options and short descriptions)
    let _ = write!(
        msg,
        "Guesser parameters:\n  -D, --direction=DIR   set direction of guessing\n  -w, --weight=WEIGHT   set weight difference of affix lengths\n\n"
    );
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(
        msg,
        "DIR is either suffix or prefix, or suffix if omitted.\nWEIGHT is a weight of each arc not in the known suffix or prefix being guessed, as parsed with strtod(3), or 1.0 if omitted.\n"
    );
    let _ = write!(msg, "\n");
}

// [spec:hfst:def:hfst-affix-guessify.parse-options-fn]
// [spec:hfst:sem:hfst-affix-guessify.parse-options-fn]
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
        long_options.extend(hfst_getopt_unary_long());
        // add tool-specific options here
        long_options.push(getopt::GetOpt {
            name: "weight",
            has_arg: 1, // required_argument
            val: 'w' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "direction",
            has_arg: 1, // required_argument
            val: 'D' as i32,
        });
        // add tool-specific options here
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: common
        // cases, then unary cases, then the tool's own ('w'/'D'), then the
        // terminal error arm.
        match handle_common_case(&mut common, &opt, c, print_usage) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        match handle_unary_case(&mut common, &opt, c) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        match c {
            x if x == 'w' as i32 => {
                options.weight = hfst_strtoweight(&common, &opt.optarg());
                continue;
            }
            x if x == 'D' as i32 => {
                let optarg = opt.optarg();
                if optarg.starts_with("prefix") {
                    options.direction = GuessDirection::GuessPrefix;
                } else if optarg.starts_with("suffix") {
                    options.direction = GuessDirection::GuessSuffix;
                } else {
                    error(
                        &common,
                        1,
                        0,
                        &format!(
                            "Unable to parse guessing direction from {};\nplease use one of 'prefix' or 'suffix'",
                            optarg
                        ),
                    );
                }
                continue;
            }
            _ => {}
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    check_common_params(&mut common);
    check_unary_params(&mut common, &opt, args);
    Ok((common, options))
}

// [spec:hfst:def:hfst-affix-guessify.process-stream-fn]
// [spec:hfst:sem:hfst-affix-guessify.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &Options,
    instream: &mut HfstInputStream,
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
            let trans = trans;
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
            let mut t = match affix_guessify(&trans, options.direction, options.weight) {
                Ok(t) => t,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            if let Err(e) = outstream.redirect(&mut t) {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        }, else => {
            // Unreachable: the optimized-lookup stream rejection already
            // returned before the loop; keep its text for safety.
            let _ = write!(
                std::io::stderr(),
                "Error: hfst-affix-guessify cannot process transducers that are in optimized lookup format.\n"
            );
            return 1;
        });
    } // good instream
    0
}

// [spec:hfst:def:hfst-affix-guessify.main-fn]
// [spec:hfst:sem:hfst-affix-guessify.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstAffixGuessify");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

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
            return 1;
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
            return 1;
        }
    };

    if is_input_stream_in_ol_format(&instream, "hfst-affix-guessify") {
        return 1;
    }

    process_stream(&common, &options, &mut instream, &mut outstream)
}
