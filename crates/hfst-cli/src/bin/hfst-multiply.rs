#![allow(static_mut_refs)]
//! Faithful 1:1 port of tools/src/hfst-multiply.cc — the transducer archive
//! duplication tool (writes the first transducer of an archive repeatedly).
//! Drives the hfst-cli foundation (globals, getopt, commandline,
//! program-options, tool-metadata, inc fragments).

use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, extend_options_getenv, hfst_set_program_name, hfst_strtoul,
    is_input_stream_in_ol_format, print_more_info, print_report_bugs, verbose_printf,
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
use std::io::Write;

// add tools-specific variables here
static mut DUPE_COUNT: u64 = 1;

// [spec:hfst:def:hfst-multiply.print-usage-fn]
// [spec:hfst:sem:hfst-multiply.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nUse first transducer of an archive repeatedly\n\n",
        globals::program_name()
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Archive options:\n  -n, --n-last=NUMBER   Duplicate each transducer NUMBER times\n"
    );
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(
        msg,
        "NUMBER must be a positive integer as parsed by strtoul base 10\n"
    );
    let _ = write!(msg, "\n");
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-multiply.parse-options-fn]
// [spec:hfst:sem:hfst-multiply.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_getenv(args);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            long_options.push(getopt::GetOpt {
                name: "n-times",
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
                DUPE_COUNT = hfst_strtoul(&getopt::optarg(), 10);
                continue;
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(args);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-multiply.process-stream-fn]
// [spec:hfst:sem:hfst-multiply.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream, outstream: &mut HfstOutputStream) -> i32 {
    unsafe {
        // C declares 'queue<HfstTransducer> last_n;' here but never uses it.
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let mut trans = match HfstTransducer::new_from_stream(instream) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("hfst-multiply: {e}");
                    return 1;
                }
            };
            let mut inputname = trans.get_name();
            if inputname.is_empty() {
                inputname = globals::input_filename();
            }

            verbose_printf(&format!(
                "Duplicate {} times {}...{}\n",
                inputname, DUPE_COUNT, transducer_n
            ));
            for _ in 0..DUPE_COUNT {
                if let Err(e) = outstream.redirect(&mut trans) {
                    eprintln!("hfst-multiply: {e}");
                    return 1;
                }
            }
        }
        instream.close();
        outstream.close();
        0
    }
}

// [spec:hfst:def:hfst-multiply.main-fn]
// [spec:hfst:sem:hfst-multiply.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstDuplicate");
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
        let mut instream = match if input_opened {
            HfstInputStream::new_filename(&globals::input_filename())
        } else {
            HfstInputStream::new()
        } {
            Ok(s) => s,
            Err(e) => {
                eprintln!("hfst-multiply: cannot open input: {e}");
                return 1;
            }
        };
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)

        if is_input_stream_in_ol_format(&instream, "hfst-multiply") {
            return 1;
        }

        let ty = instream.get_type();
        let mut outstream = match if output_opened {
            HfstOutputStream::new_filename(&globals::output_filename(), ty, true)
        } else {
            HfstOutputStream::new(ty, true)
        } {
            Ok(s) => s,
            Err(e) => {
                eprintln!("hfst-multiply: cannot open output: {e}");
                return 1;
            }
        };

        process_stream(&mut instream, &mut outstream)
    }
}
