//! Faithful 1:1 port of tools/src/hfst-subtract.cc — the transducer subtraction
//! (minus) command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments). A BINARY tool:
//! it reads two input streams (first + second); the shared scaffolding lives
//! in crate::binary_ops.

use crate::binary_ops::{
    BinaryOpSpec, BinaryToolOp, LoopStyle, PairContext, RetryPolicy, run_binary_streams_tool,
};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{error, extend_options_from_env, hfst_set_program_name, warning};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_binary_long, hfst_getopt_common_long, print_common_binary_program_options,
    print_common_binary_program_parameter_instructions, print_common_program_options,
};
use crate::inc::{
    CaseResult, check_binary_params, check_common_params, handle_binary_case, handle_common_case,
    handle_error_case,
};
use hfst::backend::AlgebraBackend;
use hfst::hfst_transducer::HfstTransducer;
use std::io::Write;

/// hfst-subtract's own options (the former tool-specific `static mut`s).
struct Options {
    /// '-F, --harmonize-flags': harmonize flag diacritics.
    harmonize_flags: bool,
    /// '-H, --do-not-harmonize': off harmonizes symbols (default on).
    harmonize: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            harmonize_flags: false,
            harmonize: true,
        }
    }
}

// [spec:hfst:def:hfst-subtract.print-usage-fn]
// [spec:hfst:sem:hfst-subtract.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    let mut msg = common.message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nSubtract (minus) two transducers\n\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    print_common_binary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Flag diacritics:\n  -F, --harmonize-flags  Harmonize flag diacritics\n  -H, --do-not-harmonize Do not harmonize\n",
    );
    let _ = writeln!(msg);
    print_common_binary_program_parameter_instructions(&mut *msg);
    let _ = writeln!(msg);
    let _ = write!(
        msg,
        "\nExamples:\n  {} -o catdog.hfst cat.hfst dog.hfst  subtracts transducers\n\n",
        common.program_name
    );
}

// [spec:hfst:def:hfst-subtract.parse-options-fn]
// [spec:hfst:sem:hfst-subtract.parse-options-fn]
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
        long_options.extend(hfst_getopt_binary_long());
        // add tool-specific options here
        long_options.push(getopt::GetOpt {
            name: "harmonize-flags",
            has_arg: getopt::NO_ARGUMENT,
            val: 'F' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "do-not-harmonize",
            has_arg: getopt::NO_ARGUMENT,
            val: 'H' as i32,
        });
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: binary
        // cases, common cases, then the tool's own ('F'/'H'), then the
        // terminal error arm.
        match handle_binary_case(&mut common, &opt, c) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        match handle_common_case(&mut common, &opt, c, print_usage) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        if c == 'F' as i32 {
            options.harmonize_flags = true;
            continue;
        }
        if c == 'H' as i32 {
            options.harmonize = false;
            continue;
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    check_binary_params(&mut common, &opt, args);
    check_common_params(&mut common);
    Ok((common, options))
}

// [spec:hfst:def:hfst-subtract.subtract-streams-fn]
// [spec:hfst:sem:hfst-subtract.subtract-streams-fn]
// The streams loop lives in crate::binary_ops::run_binary_streams_tool;
// this descriptor plus the pre-apply/apply closures in run carry the
// tool's behaviour contract.
const SPEC: BinaryOpSpec = BinaryOpSpec {
    tool_name: "hfst-subtract",
    mismatch_noun: "subtraction",
    could_not_verb: "subtract",
    could_not_noun: "subtraction",
    name_op: "subtract",
    formula: "\u{2212}",
    verbose_begin: |firstname, secondname| format!("Subtracting {} from {}", secondname, firstname),
    loop_style: LoopStyle::Standard,
    retry: RetryPolicy::AnyError,
    flush_each_round: false,
    flush_at_end: true,
};

// [spec:hfst:def:hfst-subtract.main-fn]
// [spec:hfst:sem:hfst-subtract.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstSubtract");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    let mut op = SubtractOp {
        harmonize: options.harmonize,
        harmonize_flags: options.harmonize_flags,
    };
    run_binary_streams_tool(&common, &SPEC, &mut op)
}

struct SubtractOp {
    harmonize: bool,
    harmonize_flags: bool,
}

impl BinaryToolOp for SubtractOp {
    fn pre_apply<B: AlgebraBackend>(
        &mut self,
        common: &CommonOptions,
        first: &mut HfstTransducer<B>,
        second: &mut HfstTransducer<B>,
        _ctx: &PairContext<'_>,
    ) -> Result<(), i32> {
        if second.has_flag_diacritics() {
            warning(
                common,
                0,
                0,
                &format!(
                    "Warning: {} contains flag diacritics. The result of subtraction may be incorrect.",
                    common.second_filename
                ),
            );
        }
        let first_has_flags = first.has_flag_diacritics();
        let second_has_flags = second.has_flag_diacritics();
        if first_has_flags && second_has_flags {
            if !self.harmonize_flags {
                if !common.silent {
                    warning(
                        common,
                        0,
                        0,
                        "The argumentes contain flag diacritics. Use -F to harmonize them.",
                    );
                }
            } else {
                // C: 'first->harmonize_flag_diacritics(*second)' — relies
                // on the default 'insert_renamed_flags=true'.
                if let Err(e) = first.harmonize_flag_diacritics(second, true) {
                    error(common, 1, 0, &format!("{e}"));
                    return Err(1);
                }
            }
        }
        Ok(())
    }

    fn apply<B: AlgebraBackend>(
        &mut self,
        first: &mut HfstTransducer<B>,
        second: &HfstTransducer<B>,
    ) -> hfst::error::Result<()> {
        first.subtract(second, self.harmonize).map(|_| ())
    }
}
