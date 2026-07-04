#![allow(static_mut_refs)]
//! Faithful 1:1 port of tools/src/hfst-head.cc — the transducer archive head
//! splitting tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).

use crate::globals;
use crate::hfst_commandline::{
    EXIT_CONTINUE, extend_options_from_env, hfst_set_program_name, parse_i64, verbose_print,
    warning,
};
use crate::hfst_getopt as getopt;
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::AnyTransducer;
use std::collections::VecDeque;
use std::io::Write;

// add tools-specific variables here
static mut HEAD_COUNT: i64 = 1;

// [spec:hfst:def:hfst-head.print-usage-fn]
// [spec:hfst:sem:hfst-head.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    // Usage line
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nGet first transducers from an archive\n\n",
        globals::program_name()
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Archive options:\n  -n, --n-first=[-]K   print the first K transducers;\n                       with the leading `-', print all but last K transducers\n"
    );
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(
        msg,
        "K must be an integer, as parsed by strtoul base 10, and not 0.\nIf K is omitted default is 1."
    );
    let _ = write!(msg, "\n");
}

// [spec:hfst:def:hfst-head.parse-options-fn]
// [spec:hfst:sem:hfst-head.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_from_env(args);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            long_options.push(getopt::GetOpt {
                name: "n-first",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: 'n' as i32,
            });
            // add tool-specific options here
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
                HEAD_COUNT = parse_i64(&getopt::optarg(), 10);
                continue;
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(args);
        if HEAD_COUNT == 0 {
            warning(0, 0, "Argument 0 for count is not sensible");
        }
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-head.process-stream-fn]
// [spec:hfst:sem:hfst-head.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream, outstream: &mut HfstOutputStream) -> i32 {
    unsafe {
        let mut transducer_n: usize = 0;
        if HEAD_COUNT > 0 {
            while instream.is_good() && (transducer_n < HEAD_COUNT as usize) {
                transducer_n += 1;
                let mut trans = match instream.read() {
                    Ok(t) => t,
                    Err(e) => {
                        crate::hfst_commandline::error(1, 0, &format!("{e}"));
                        return 1;
                    }
                };
                let mut inputname = trans.get_name();
                if inputname.is_empty() {
                    inputname = globals::input_filename();
                }
                verbose_print(&format!("Forwarding {}...{}\n", inputname, transducer_n));
                if let Err(e) = trans.write(outstream) {
                    crate::hfst_commandline::error(1, 0, &format!("{e}"));
                    return 1;
                }
            }
        } else if HEAD_COUNT < 0 {
            let mut first_but_n: VecDeque<AnyTransducer> = VecDeque::new();
            verbose_print(&format!("Counting all but last {}\n", HEAD_COUNT));
            while instream.is_good() {
                transducer_n += 1;
                let trans = match instream.read() {
                    Ok(t) => t,
                    Err(e) => {
                        crate::hfst_commandline::error(1, 0, &format!("{e}"));
                        return 1;
                    }
                };
                first_but_n.push_back(trans);
            }
            if (-HEAD_COUNT) as usize > first_but_n.len() {
                warning(
                    0,
                    0,
                    &format!(
                        "Stream in {} has less than {} automata; Nothing will be written to output",
                        globals::input_filename(),
                        -HEAD_COUNT
                    ),
                );
            }
            for _ in 0..(-HEAD_COUNT) {
                if !first_but_n.is_empty() {
                    first_but_n.pop_back();
                }
            }
            while !first_but_n.is_empty() {
                // C: copied the front and popped it afterwards; taking it by
                // value is the same write in one move.
                let mut trans = first_but_n
                    .pop_front()
                    .expect("first_but_n is non-empty per the enclosing while condition");
                let mut inputname = trans.get_name();
                if inputname.is_empty() {
                    inputname = globals::input_filename();
                }
                verbose_print(&format!("Forwarding {}...{}\n", inputname, transducer_n));
                if let Err(e) = trans.write(outstream) {
                    crate::hfst_commandline::error(1, 0, &format!("{e}"));
                    return 1;
                }
            }
        }
        if let Err(e) = outstream.flush() {
            crate::hfst_commandline::error(1, 0, &format!("{e}"));
            return 1;
        }
        instream.close();
        outstream.close();
        0
    }
}

// [spec:hfst:def:hfst-head.main-fn]
// [spec:hfst:sem:hfst-head.main-fn]
pub fn run(args: Vec<String>) -> i32 {
    unsafe { real_main(args) }
}

unsafe fn real_main(mut args: Vec<String>) -> i32 {
    unsafe {
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.2", "HfstHead");
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
                crate::hfst_commandline::error(1, 0, &format!("{e}"));
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
                crate::hfst_commandline::error(1, 0, &format!("{e}"));
                return 1;
            }
        };

        process_stream(&mut instream, &mut outstream)
    }
}
