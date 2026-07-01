#![allow(static_mut_refs)]
//! Faithful 1:1 port of tools/src/hfst-repeat.cc — the transducer repetition
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).

use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_getenv, hfst_set_program_name, hfst_strtonumber,
    is_input_stream_in_ol_format, print_more_info, print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use hfst_cli::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary, hfst_set_name_unary};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use std::io::Write;

// add tools-specific variables here
static mut AT_LEAST: u64 = 0;
static mut AT_MOST: u64 = u32::MAX as u64;
static mut FROM_INFINITY: bool = false;
static mut TO_INFINITY: bool = true;

// [spec:hfst:def:hfst-repeat.print-usage-fn]
// [spec:hfst:sem:hfst-repeat.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nRepeat transducer\n\n",
        globals::program_name()
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
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-repeat.parse-options-fn]
// [spec:hfst:sem:hfst-repeat.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_getenv(args);
        // use of this function requires options are settable on global scope
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
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own f/t cases, then the
            // terminal error arm.
            match handle_common_case(c, print_usage) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_unary_case(c) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match c {
                c if c == b'f' as i32 => {
                    let mut from_inf = false;
                    AT_LEAST = hfst_strtonumber(&getopt::optarg(), Some(&mut from_inf)) as u64;
                    FROM_INFINITY = from_inf;
                    continue;
                }
                c if c == b't' as i32 => {
                    let mut to_inf = false;
                    AT_MOST = hfst_strtonumber(&getopt::optarg(), Some(&mut to_inf)) as u64;
                    TO_INFINITY = to_inf;
                    continue;
                }
                _ => {}
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(args);
        if AT_LEAST > AT_MOST {
            error(
                1,
                0,
                &format!("Cannot repeat from {} to {} times\n", AT_LEAST, AT_MOST),
            );
        }
        if FROM_INFINITY && !TO_INFINITY {
            error(
                1,
                0,
                &format!("Cannot repeat from infinity to {} times\n", AT_MOST),
            );
        }
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-repeat.process-stream-fn]
// [spec:hfst:sem:hfst-repeat.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream, outstream: &mut HfstOutputStream) -> i32 {
    unsafe {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let mut trans = match HfstTransducer::new_from_stream(instream) {
                Ok(t) => t,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            };
            let inputname = hfst_get_name(&trans, &globals::input_filename());
            if transducer_n == 1 {
                if !FROM_INFINITY && !TO_INFINITY {
                    verbose_printf(&format!(
                        "Repeating [{}..{}] {}...\n",
                        AT_LEAST, AT_MOST, inputname
                    ));
                } else if FROM_INFINITY && TO_INFINITY {
                    verbose_printf(&format!("Repeating star {}...\n", inputname));
                } else if !FROM_INFINITY && TO_INFINITY {
                    verbose_printf(&format!("Repeating [{}..*] {}...\n", AT_LEAST, inputname));
                } else if FROM_INFINITY && TO_INFINITY {
                    error(1, 0, &format!("Repeating *..{}?", AT_MOST));
                }
            } else if !FROM_INFINITY && !TO_INFINITY {
                verbose_printf(&format!(
                    "Repeating [{}..{}] {}... {}\n",
                    AT_LEAST, AT_MOST, inputname, transducer_n
                ));
            } else if FROM_INFINITY && TO_INFINITY {
                verbose_printf(&format!(
                    "Repeating star {}... {}\n",
                    inputname, transducer_n
                ));
            } else if !FROM_INFINITY && TO_INFINITY {
                verbose_printf(&format!(
                    "Repeating [{}..*] {}... {}\n",
                    AT_LEAST, inputname, transducer_n
                ));
            } else if FROM_INFINITY && TO_INFINITY {
                error(1, 0, &format!("Repeating *..{}?", AT_MOST));
            }

            if !FROM_INFINITY && !TO_INFINITY {
                if let Err(e) = trans.repeat_n_to_k(AT_LEAST as u32, AT_MOST as u32) {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
                let composed_name = format!("repeat-{}-to-{}", AT_LEAST, AT_MOST);
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, &composed_name);
                let composed_name = format!("_{}^{}", AT_LEAST, AT_MOST);
                let src = trans.clone();
                hfst_set_formula_unary(&mut trans, &src, &composed_name);
            } else if FROM_INFINITY && TO_INFINITY {
                if let Err(e) = trans.repeat_star() {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "repeat-star");
                let src = trans.clone();
                hfst_set_formula_unary(&mut trans, &src, "\u{22c6}");
            } else if !FROM_INFINITY && TO_INFINITY {
                if let Err(e) = trans.repeat_n_plus(AT_LEAST as u32) {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
                let composed_name = format!("repeat-{}-plus", AT_LEAST);
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, &composed_name);
                let composed_name = format!("_{}^\u{221e}", AT_LEAST);
                let src = trans.clone();
                hfst_set_formula_unary(&mut trans, &src, &composed_name);
            } else if FROM_INFINITY && !TO_INFINITY {
                error(1, 0, &format!("Repeating *..{}?", AT_MOST));
            }
            if let Err(e) = outstream.redirect(&mut trans) {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        }
        instream.close();
        outstream.close();
        0
    }
}

// [spec:hfst:def:hfst-repeat.main-fn]
// [spec:hfst:sem:hfst-repeat.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstRepeat");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let input_opened = globals::input_filename() != "<stdin>";
        let output_opened = globals::output_filename() != "<stdout>";
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            globals::input_filename(),
            globals::output_filename()
        ));
        if !FROM_INFINITY && !TO_INFINITY {
            verbose_printf(&format!(
                "Repeating from {} to {} times\n",
                AT_LEAST, AT_MOST
            ));
        } else if FROM_INFINITY && TO_INFINITY {
            verbose_printf("Repeating star infinitely\n");
        } else if !FROM_INFINITY && TO_INFINITY {
            verbose_printf(&format!("Repeating from {} to infinite times\n", AT_LEAST));
        } else if FROM_INFINITY && !TO_INFINITY {
            error(
                1,
                0,
                &format!(
                    "Repeating at least infinite butno more than {} times?",
                    AT_MOST
                ),
            );
        }

        // here starts the buffer handling part
        let mut instream = match if input_opened {
            HfstInputStream::new_filename(&globals::input_filename())
        } else {
            HfstInputStream::new()
        } {
            Ok(s) => s,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        };
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)

        let type_ = instream.get_type();
        let mut outstream = match if output_opened {
            HfstOutputStream::new_filename(&globals::output_filename(), type_, true)
        } else {
            HfstOutputStream::new(type_, true)
        } {
            Ok(s) => s,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        };

        if is_input_stream_in_ol_format(&instream, "hfst-repeat") {
            return 1;
        }

        process_stream(&mut instream, &mut outstream)
    }
}
