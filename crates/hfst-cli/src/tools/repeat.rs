//! Faithful 1:1 port of tools/src/hfst-repeat.cc — the transducer repetition
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).
//!
//! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
//! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
//! `parse_options` and threaded into the processing functions. There are no
//! `static mut` globals and no `unsafe`.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, extend_options_from_env, hfst_set_program_name, hfst_strtonumber,
    is_input_stream_in_ol_format, verbose_print,
};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use crate::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary, hfst_set_name_unary};
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use std::io::Write;

/// hfst-repeat's own options (the former tool-specific `static mut`s).
struct Options {
    /// '-f, --from=FNUM': repeat at least FNUM times.
    at_least: u64,
    /// '-t, --to=TNUM': repeat at most TNUM times.
    at_most: u64,
    /// FNUM was parsed as infinity.
    from_infinity: bool,
    /// TNUM was parsed as infinity.
    to_infinity: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            at_least: 0,
            at_most: u32::MAX as u64,
            from_infinity: false,
            to_infinity: true,
        }
    }
}

// [spec:hfst:def:hfst-repeat.print-usage-fn]
// [spec:hfst:sem:hfst-repeat.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nRepeat transducer\n\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Repetition options:\n  -f, --from=FNUM   repeat at least FNUM times\n  -t, --to=TNUM     repeat at most TNUM times\n"
    );
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(
        msg,
        "FNUM and TNUM must be positive integers or infinities as parsed by strtod(3)\nif FNUM is omitted it defaults to 0, if TNUM is omitted it defaults to Inf\nFNUM must be less than TNUM\n"
    );
    let _ = write!(msg, "\n");
}

// [spec:hfst:def:hfst-repeat.parse-options-fn]
// [spec:hfst:sem:hfst-repeat.parse-options-fn]
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
        long_options.push(getopt::GetOpt {
            name: "from",
            has_arg: getopt::REQUIRED_ARGUMENT,
            val: b'f' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "to",
            has_arg: getopt::REQUIRED_ARGUMENT,
            val: b't' as i32,
        });
        // add tool-specific options here
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: common
        // cases, then unary cases, then the tool's own f/t cases, then the
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
            c if c == b'f' as i32 => {
                let mut from_inf = false;
                options.at_least =
                    hfst_strtonumber(&common, &opt.optarg(), Some(&mut from_inf)) as u64;
                options.from_infinity = from_inf;
                continue;
            }
            c if c == b't' as i32 => {
                let mut to_inf = false;
                options.at_most =
                    hfst_strtonumber(&common, &opt.optarg(), Some(&mut to_inf)) as u64;
                options.to_infinity = to_inf;
                continue;
            }
            _ => {}
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    check_common_params(&mut common);
    check_unary_params(&mut common, &opt, args);
    if options.at_least > options.at_most {
        error(
            &common,
            1,
            0,
            &format!(
                "Cannot repeat from {} to {} times\n",
                options.at_least, options.at_most
            ),
        );
    }
    if options.from_infinity && !options.to_infinity {
        error(
            &common,
            1,
            0,
            &format!("Cannot repeat from infinity to {} times\n", options.at_most),
        );
    }
    Ok((common, options))
}

// [spec:hfst:def:hfst-repeat.process-stream-fn]
// [spec:hfst:sem:hfst-repeat.process-stream-fn]
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
            let mut trans = trans;
            let inputname = hfst_get_name(&trans, &common.input_filename);
            if transducer_n == 1 {
                if !options.from_infinity && !options.to_infinity {
                    verbose_print(common, &format!(
                        "Repeating [{}..{}] {}...\n",
                        options.at_least, options.at_most, inputname
                    ));
                } else if options.from_infinity && options.to_infinity {
                    verbose_print(common, &format!("Repeating star {}...\n", inputname));
                } else if !options.from_infinity && options.to_infinity {
                    verbose_print(common, &format!("Repeating [{}..*] {}...\n", options.at_least, inputname));
                } else if options.from_infinity && !options.to_infinity {
                    error(common, 1, 0, &format!("Repeating *..{}?", options.at_most));
                }
            } else if !options.from_infinity && !options.to_infinity {
                verbose_print(common, &format!(
                    "Repeating [{}..{}] {}... {}\n",
                    options.at_least, options.at_most, inputname, transducer_n
                ));
            } else if options.from_infinity && options.to_infinity {
                verbose_print(common, &format!(
                    "Repeating star {}... {}\n",
                    inputname, transducer_n
                ));
            } else if !options.from_infinity && options.to_infinity {
                verbose_print(common, &format!(
                    "Repeating [{}..*] {}... {}\n",
                    options.at_least, inputname, transducer_n
                ));
            } else if options.from_infinity && !options.to_infinity {
                error(common, 1, 0, &format!("Repeating *..{}?", options.at_most));
            }

            if !options.from_infinity && !options.to_infinity {
                if let Err(e) = trans.repeat_n_to_k(options.at_least as u32, options.at_most as u32) {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
                let composed_name = format!("repeat-{}-to-{}", options.at_least, options.at_most);
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, &composed_name);
                let composed_name = format!("_{}^{}", options.at_least, options.at_most);
                let src = trans.clone();
                hfst_set_formula_unary(&mut trans, &src, &composed_name);
            } else if options.from_infinity && options.to_infinity {
                if let Err(e) = trans.repeat_star() {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "repeat-star");
                let src = trans.clone();
                hfst_set_formula_unary(&mut trans, &src, "\u{22c6}");
            } else if !options.from_infinity && options.to_infinity {
                if let Err(e) = trans.repeat_n_plus(options.at_least as u32) {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
                let composed_name = format!("repeat-{}-plus", options.at_least);
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, &composed_name);
                let composed_name = format!("_{}^\u{221e}", options.at_least);
                let src = trans.clone();
                hfst_set_formula_unary(&mut trans, &src, &composed_name);
            } else if options.from_infinity && !options.to_infinity {
                error(common, 1, 0, &format!("Repeating *..{}?", options.at_most));
            }
            if let Err(e) = outstream.redirect(&mut trans) {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        }, else => {
            // Unreachable: the optimized-lookup stream rejection already
            // returned before the loop; keep its text for safety.
            let _ = write!(
                std::io::stderr(),
                "Error: hfst-repeat cannot process transducers that are in optimized lookup format.\n"
            );
            return 1;
        });
    }
    instream.close();
    outstream.close();
    0
}

// [spec:hfst:def:hfst-repeat.main-fn]
// [spec:hfst:sem:hfst-repeat.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstRepeat");
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
    if !options.from_infinity && !options.to_infinity {
        verbose_print(
            &common,
            &format!(
                "Repeating from {} to {} times\n",
                options.at_least, options.at_most
            ),
        );
    } else if options.from_infinity && options.to_infinity {
        verbose_print(&common, "Repeating star infinitely\n");
    } else if !options.from_infinity && options.to_infinity {
        verbose_print(
            &common,
            &format!("Repeating from {} to infinite times\n", options.at_least),
        );
    } else if options.from_infinity && !options.to_infinity {
        error(
            &common,
            1,
            0,
            &format!(
                "Repeating at least infinite butno more than {} times?",
                options.at_most
            ),
        );
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
            return 1;
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
            return 1;
        }
    };

    if is_input_stream_in_ol_format(&instream, "hfst-repeat") {
        return 1;
    }

    process_stream(&common, &options, &mut instream, &mut outstream)
}
