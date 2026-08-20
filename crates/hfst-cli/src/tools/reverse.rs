//! Faithful 1:1 port of tools/src/hfst-reverse.cc — the transducer reversion
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).
//!
//! The tool's state lives in [`CommonOptions`] (the shared `-v/-q/-o/-i/…`
//! fields), built by `parse_options` and threaded into the processing
//! functions. There are no `static mut` globals and no `unsafe`.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{extend_options_from_env, hfst_set_program_name};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
use hfst::backend::AlgebraBackend;
use hfst::hfst_transducer::HfstTransducer;
use std::borrow::Cow;
use std::io::Write;

// [spec:hfst:def:hfst-reverse.print-usage-fn]
// [spec:hfst:sem:hfst-reverse.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nReverse a transducer\n\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = writeln!(msg);
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = writeln!(msg);
}

// [spec:hfst:def:hfst-reverse.parse-options-fn]
// [spec:hfst:sem:hfst-reverse.parse-options-fn]
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
        // cases, then unary cases, then the tool's own (none here), then the
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
        return Err(handle_error_case(&common, &opt, c));
    }

    check_common_params(&mut common);
    check_unary_params(&mut common, &opt, args);
    Ok(common)
}

// [spec:hfst:def:hfst-reverse.process-stream-fn]
// [spec:hfst:sem:hfst-reverse.process-stream-fn]
//
// The stream loop lives in the shared unary driver; this op is the
// per-transducer body it dispatches into.
struct ReverseOp;

impl UnaryToolOp for ReverseOp {
    fn verbose_begin(&self, inputname: &str) -> String {
        format!("Reversing {}", inputname)
    }

    fn name_op(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed("reverse"))
    }

    fn formula(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed("\u{21c6}"))
    }

    fn apply<B: AlgebraBackend>(
        &mut self,
        _common: &CommonOptions,
        t: &mut HfstTransducer<B>,
    ) -> hfst::error::Result<()> {
        t.reverse().map(|_| ())
    }
}

const SPEC: UnaryOpSpec = UnaryOpSpec {
    tool_name: "hfst-reverse",
    reject_ol: true,
};

// [spec:hfst:def:hfst-reverse.main-fn]
// [spec:hfst:sem:hfst-reverse.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstReverse");
    let common = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    run_unary_tool(&common, &SPEC, &mut ReverseOp)
}
