//! Faithful 1:1 port of tools/src/hfst-strip-header.cc — the HFST header
//! stripping command-line tool. Drives the hfst-cli foundation (globals,
//! getopt, commandline, program-options, inc fragments).
//!
//! Unlike most unary tools, this one does not build HfstInputStream /
//! HfstOutputStream objects: it opens its input/output as std streams (from the
//! filename fields, with the "<stdin>"/"<stdout>" sentinels) and delegates the
//! byte copy + HFST3-header stripping to hfst_input_stream::strip_hfst3_headers.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{extend_options_from_env, hfst_set_program_name, verbose_print};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use hfst::hfst_input_stream::strip_hfst3_headers;
use std::io::Write;

// [spec:hfst:def:hfst-strip-header.print-usage-fn]
// [spec:hfst:sem:hfst-strip-header.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nRemove any HFST3 headers\n\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = writeln!(msg);
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = writeln!(msg);
}

// [spec:hfst:def:hfst-strip-header.parse-options-fn]
// [spec:hfst:sem:hfst-strip-header.parse-options-fn]
//
// Parse argv into the shared options; `Err(code)` is an exit code the caller
// should return (the former EXIT_CONTINUE sentinel is now `Ok`).
fn parse_options(mut common: CommonOptions, args: &mut Vec<String>) -> Result<CommonOptions, i32> {
    let mut opt = Getopt::new();
    extend_options_from_env(args);
    loop {
        let mut long_options: Vec<getopt::GetOpt> = Vec::new();
        long_options.extend(hfst_getopt_common_long());
        long_options.extend(hfst_getopt_unary_long());
        // add tool-specific options here
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: common
        // cases, then unary cases, then the terminal error arm.
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
        return Err(handle_error_case(&common, &opt, c));
    }

    check_common_params(&mut common);
    check_unary_params(&mut common, &opt, args);
    Ok(common)
}

// [spec:hfst:def:hfst-strip-header.process-stream-fn]
// [spec:hfst:sem:hfst-strip-header.process-stream-fn]
fn process_stream(common: &CommonOptions) -> i32 {
    // De-C-ified: open the input/output as std streams (resolved from the
    // filename fields by common.input_reader / output_writer, which honour the
    // "<stdin>"/"<stdout>" sentinels) and delegate the HFST3-header stripping to
    // hfst_input_stream::strip_hfst3_headers. The C printed "Stripping..." once
    // per byte under -v; that per-byte trace is dropped (diagnostic only — the
    // stripped output is unchanged).
    let input = match common.input_reader() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("hfst-strip-header: could not open input: {e}");
            return 1;
        }
    };
    let output = match common.output_writer() {
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
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstStripHeader");
    let common = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };
    verbose_print(
        &common,
        &format!(
            "Reading from {}, writing to {}\n",
            common.input_filename, common.output_filename
        ),
    );

    process_stream(&common)
}
