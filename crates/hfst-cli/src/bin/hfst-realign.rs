//! Faithful 1:1 port of tools/src/hfst-realign.cc — the transducer realign
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).

use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_getenv, hfst_set_program_name,
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
static mut BOUNDARY_SYMBOL: u8 = b'>';

// [spec:hfst:def:hfst-realign.print-usage-fn]
// [spec:hfst:sem:hfst-realign.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    // Usage line
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nRealign a transducer by pushing labels to the start\n\n",
        globals::program_name()
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Options:\n  -b, --boundary=SYM   treat SYM as a boundary symbol\n"
    );
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(msg, "SYM must be in the alphabet\n");
    let _ = write!(msg, "\n");
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-realign.parse-options-fn]
// [spec:hfst:sem:hfst-realign.parse-options-fn]
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
                name: "boundary",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: b'b' as i32,
            });
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own, then the terminal
            // error arm.
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
            // The C source labels its tool-specific arm 'p' (not 'b'), which
            // merely resets the boundary symbol to its default '>'.
            if c == (b'p' as i32) {
                BOUNDARY_SYMBOL = b'>';
                continue;
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(args);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-realign.process-stream-fn]
// [spec:hfst:sem:hfst-realign.process-stream-fn]
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
                if BOUNDARY_SYMBOL != 0 {
                    verbose_printf(&format!("Pushing towards start {}...\n", inputname));
                } else {
                    verbose_printf(&format!("Pushing towards end {}...\n", inputname));
                }
            } else if BOUNDARY_SYMBOL != 0 {
                verbose_printf(&format!(
                    "Pushing towards start {}... {}\n",
                    inputname, transducer_n
                ));
            } else {
                verbose_printf(&format!(
                    "Pushing towards end {}... {}\n",
                    inputname, transducer_n
                ));
            }

            if let Err(e) = trans.realign() {
                error(1, 0, &format!("{e}"));
                return 1;
            }
            // C: hfst_set_name(trans, trans, "realign"); the dest and src are the
            // same object, which Rust cannot alias mut+const, so the read side is
            // taken from a copy (name/formula are unchanged by the copy).
            let src = trans.clone();
            hfst_set_name_unary(&mut trans, &src, "realign");
            hfst_set_formula_unary(&mut trans, &src, "Id");

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

// [spec:hfst:def:hfst-realign.main-fn]
// [spec:hfst:sem:hfst-realign.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstRealign");
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

        // here starts the buffer handling part
        let instream_result = if input_opened {
            HfstInputStream::new_filename(&globals::input_filename())
        } else {
            HfstInputStream::new()
        };
        let mut instream = match instream_result {
            Ok(s) => s,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        };
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)

        let ty = instream.get_type();
        let outstream_result = if output_opened {
            HfstOutputStream::new_filename(&globals::output_filename(), ty, true)
        } else {
            HfstOutputStream::new(ty, true)
        };
        let mut outstream = match outstream_result {
            Ok(s) => s,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        };

        if is_input_stream_in_ol_format(&instream, "hfst-realign") {
            return 1;
        }

        process_stream(&mut instream, &mut outstream)
    }
}
