//! Faithful 1:1 port of tools/src/hfst-strip-header.cc — the HFST header
//! stripping command-line tool. Drives the hfst-cli foundation (globals,
//! getopt, commandline, program-options, inc fragments).
//!
//! Unlike most unary tools, this one does not build HfstInputStream /
//! HfstOutputStream objects: it opens its input/output as std streams (from the
//! filename globals, with the "<stdin>"/"<stdout>" sentinels) and delegates the
//! byte copy + HFST3-header stripping to hfst_input_stream::strip_hfst3_headers.

use hfst::hfst_input_stream::strip_hfst3_headers;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, hfst_set_program_name, print_more_info, print_report_bugs, verbose_print,
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

// [spec:hfst:def:hfst-strip-header.print-usage-fn]
// [spec:hfst:sem:hfst-strip-header.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nRemove any HFST3 headers\n\n",
        globals::program_name()
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(msg, "\n");
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-strip-header.parse-options-fn]
// [spec:hfst:sem:hfst-strip-header.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the terminal error arm.
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
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(args);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-strip-header.process-stream-fn]
// [spec:hfst:sem:hfst-strip-header.process-stream-fn]
unsafe fn process_stream() -> i32 {
    // De-C-ified: open the input/output as std streams (resolved from the
    // filename globals by globals::input_reader / output_writer, which honour the
    // "<stdin>"/"<stdout>" sentinels) and delegate the HFST3-header stripping to
    // hfst_input_stream::strip_hfst3_headers. The C printed "Stripping..." once
    // per byte under -v; that per-byte trace is dropped (diagnostic only — the
    // stripped output is unchanged).
    let input = match globals::input_reader() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("hfst-strip-header: could not open input: {e}");
            return 1;
        }
    };
    let output = match globals::output_writer() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("hfst-strip-header: could not open output: {e}");
            return 1;
        }
    };

    match strip_hfst3_headers(input, output) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("hfst-strip-header: error while stripping headers: {e}");
            1
        }
    }
}

// [spec:hfst:def:hfst-strip-header.main-fn]
// [spec:hfst:sem:hfst-strip-header.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstStripHeader");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        verbose_print(&format!(
            "Reading from {}, writing to {}\n",
            globals::input_filename(),
            globals::output_filename()
        ));

        process_stream()
    }
}
