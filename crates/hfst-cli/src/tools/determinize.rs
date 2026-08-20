//! Faithful 1:1 port of tools/src/hfst-determinize.cc — the transducer
//! determinisation command-line tool. Drives the hfst-cli foundation (globals,
//! getopt, commandline, program-options, tool-metadata, inc fragments).
//!
//! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
//! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
//! `parse_options` and threaded into the processing functions. There are no
//! `static mut` globals and no `unsafe`.

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
use hfst::hfst_transducer::{EngineConfig, HfstTransducer};
use std::borrow::Cow;
use std::io::Write;

/// hfst-determinize's own options (the former tool-specific `static mut`s).
#[derive(Default)]
struct Options {
    /// '-E, --encode-weights': encode weights when determinizing.
    encode_weights: bool,
}

// [spec:hfst:def:hfst-determinize.print-usage-fn]
// [spec:hfst:sem:hfst-determinize.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nDeterminize a transducer\n\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = writeln!(msg, "Command-specific options:");
    let _ = write!(
        msg,
        "  -E, --encode-weights         Encode weights when determinizing\n\
         \x20                             (default is false).\n\n"
    );
    let _ = writeln!(msg);
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = writeln!(msg);
}

// [spec:hfst:def:hfst-determinize.parse-options-fn]
// [spec:hfst:sem:hfst-determinize.parse-options-fn]
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
            name: "encode-weights",
            has_arg: getopt::NO_ARGUMENT,
            val: 'E' as i32,
        });
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: common
        // cases, unary cases, the terminal error arm, then the tool's own
        // 'E' case.
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
        if c == 'E' as i32 {
            options.encode_weights = true;
            continue;
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    check_common_params(&mut common);
    check_unary_params(&mut common, &opt, args);
    Ok((common, options))
}

// [spec:hfst:def:hfst-determinize.process-stream-fn]
// [spec:hfst:sem:hfst-determinize.process-stream-fn]
//
// The stream loop lives in the shared unary driver; this op is the
// per-transducer body it dispatches into.
struct DeterminizeOp {
    encode_weights: bool,
}

impl UnaryToolOp for DeterminizeOp {
    fn verbose_begin(&self, inputname: &str) -> String {
        format!("Determinizing {}", inputname)
    }

    fn name_op(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed("determinize"))
    }

    fn formula(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed("\u{2336}"))
    }

    fn apply<B: AlgebraBackend>(
        &mut self,
        _common: &CommonOptions,
        t: &mut HfstTransducer<B>,
    ) -> hfst::error::Result<()> {
        t.determinize_with_config(&EngineConfig {
            encode_weights: self.encode_weights,
            ..EngineConfig::default()
        })
        .map(|_| ())
    }
}

const SPEC: UnaryOpSpec = UnaryOpSpec {
    tool_name: "hfst-determinize",
    reject_ol: true,
};

// [spec:hfst:def:hfst-determinize.main-fn]
// [spec:hfst:sem:hfst-determinize.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstDeterminize");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    let mut op = DeterminizeOp {
        encode_weights: options.encode_weights,
    };
    run_unary_tool(&common, &SPEC, &mut op)
}
