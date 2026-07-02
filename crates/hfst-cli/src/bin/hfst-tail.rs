#![allow(static_mut_refs)]
//! Faithful 1:1 port of tools/src/hfst-tail.cc — the transducer archive
//! tailing command-line tool. Drives the hfst-cli foundation (globals,
//! getopt, commandline, program-options, tool-metadata, inc fragments).

use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, extend_options_getenv, hfst_set_program_name, parse_i64, print_more_info,
    print_report_bugs, verbose_print,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use std::collections::VecDeque;
use std::io::Write;

// add tools-specific variables here
static mut TAIL_COUNT: i64 = -1;

// [spec:hfst:def:hfst-tail.print-usage-fn]
// [spec:hfst:sem:hfst-tail.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nGet last transducers from an archive\n\n",
        globals::program_name()
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Archive options:\n  -n, --n-last=[+]K   Print the last K transducers;\n                      use +K to print transducers starting from the Kth\n",
    );
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(
        msg,
        "K must be an integer, as parsed by strtoul base 10, and not 0.\nif K is omitted, it defaults to +1 (all except the first)\n",
    );
    let _ = write!(msg, "\n");
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-tail.parse-options-fn]
// [spec:hfst:sem:hfst-tail.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_getenv(args);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            long_options.push(getopt::GetOpt {
                name: "n-last",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: 'n' as i32,
            });
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own ('n'), then the
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
            if c == 'n' as i32 {
                let optarg = getopt::optarg();
                if optarg.starts_with('+') {
                    // swap sign haha lol
                    TAIL_COUNT = -parse_i64(&optarg, 10);
                } else {
                    TAIL_COUNT = parse_i64(&optarg, 10);
                }
                continue;
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(args);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-tail.process-stream-fn]
// [spec:hfst:sem:hfst-tail.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream, outstream: &mut HfstOutputStream) -> i32 {
    unsafe {
        let mut last_n: VecDeque<HfstTransducer> = VecDeque::new();
        let mut transducer_n: i64 = 0;
        if TAIL_COUNT > 0 {
            verbose_print(&format!("Counting last {} transducers...\n", TAIL_COUNT));
            while instream.is_good() {
                transducer_n += 1;
                let trans = match HfstTransducer::new_from_stream(instream) {
                    Ok(t) => t,
                    Err(e) => {
                        hfst_cli::hfst_commandline::error(1, 0, &format!("{e}"));
                        return 1;
                    }
                };
                last_n.push_back(trans);
                if last_n.len() as i64 > TAIL_COUNT {
                    last_n.pop_front();
                }
            }
            if TAIL_COUNT < transducer_n {
                transducer_n -= TAIL_COUNT + 1;
            } else {
                transducer_n = 0;
            }
            while !last_n.is_empty() {
                transducer_n += 1;
                verbose_print(&format!(
                    "Forwarding {}...{}\n",
                    globals::input_filename(),
                    transducer_n
                ));
                let mut front = last_n
                    .pop_front()
                    .expect("last_n is non-empty per the enclosing while condition");
                if let Err(e) = outstream.redirect(&mut front) {
                    hfst_cli::hfst_commandline::error(1, 0, &format!("{e}"));
                    return 1;
                }
            }
        } else if TAIL_COUNT < 0 {
            verbose_print(&format!("Skipping {} transducers...\n", -TAIL_COUNT));
            while instream.is_good() {
                transducer_n += 1;
                let mut trans = match HfstTransducer::new_from_stream(instream) {
                    Ok(t) => t,
                    Err(e) => {
                        hfst_cli::hfst_commandline::error(1, 0, &format!("{e}"));
                        return 1;
                    }
                };
                if transducer_n >= -TAIL_COUNT {
                    verbose_print(&format!(
                        "Forwarding {}...{}\n",
                        globals::input_filename(),
                        transducer_n
                    ));
                    if let Err(e) = outstream.redirect(&mut trans) {
                        hfst_cli::hfst_commandline::error(1, 0, &format!("{e}"));
                        return 1;
                    }
                }
            }
        }
        if let Err(e) = outstream.flush() {
            hfst_cli::hfst_commandline::error(1, 0, &format!("{e}"));
            return 1;
        }
        instream.close();
        outstream.close();
        0
    }
}

// [spec:hfst:def:hfst-tail.main-fn]
// [spec:hfst:sem:hfst-tail.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.2", "HfstTail");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let input_opened = globals::input_filename() != "<stdin>";
        let output_opened = globals::output_filename() != "<stdout>";
        verbose_print(&format!(
            "Reading from {}, writing to {}\n",
            globals::input_filename(),
            globals::output_filename()
        ));

        // here starts the buffer handling part
        let instream_result = if input_opened {
            HfstInputStream::new_filename(&globals::input_filename())
        } else {
            HfstInputStream::new()
        };
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)
        let mut instream = match instream_result {
            Ok(s) => s,
            Err(e) => {
                hfst_cli::hfst_commandline::error(1, 0, &format!("{e}"));
                return 1;
            }
        };

        let ty = instream.get_type();
        let outstream_result = if output_opened {
            HfstOutputStream::new_filename(&globals::output_filename(), ty, true)
        } else {
            HfstOutputStream::new(ty, true)
        };
        let mut outstream = match outstream_result {
            Ok(s) => s,
            Err(e) => {
                hfst_cli::hfst_commandline::error(1, 0, &format!("{e}"));
                return 1;
            }
        };

        process_stream(&mut instream, &mut outstream)
    }
}
