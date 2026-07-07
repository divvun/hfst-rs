//! Faithful 1:1 port of tools/src/hfst-concatenate.cc — the transducer
//! concatenation command-line tool. Drives the hfst-cli foundation (getopt,
//! commandline, program-options, tool-metadata, inc fragments).
//!
//! This is a BINARY tool: it reads two input streams (firststream and
//! secondstream) and writes their pairwise concatenation; the shared
//! scaffolding lives in crate::binary_ops.
//!
//! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
//! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
//! `parse_options` and threaded into the processing functions. There are no
//! `static mut` globals and no `unsafe`.

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

/// hfst-concatenate's own options (the former tool-specific `static mut`s).
struct Options {
    /// '-F, --harmonize-flags': harmonize flag diacritics.
    harmonize_flags: bool,
    /// '-H, --do-not-harmonize': whether to harmonize symbols (default true).
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

// [spec:hfst:def:hfst-concatenate.print-usage-fn]
// [spec:hfst:sem:hfst-concatenate.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    let mut msg = common.message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nConcatenate two transducers\n\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    print_common_binary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Harmonization:\n  -H, --do-not-harmonize Do not harmonize symbols.\n  -F, --harmonize-flags  Harmonize flag diacritics.\n"
    );
    let _ = write!(msg, "\n");
    print_common_binary_program_parameter_instructions(&mut *msg);
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "\nExamples:\n  {} -o catdog.hfst cat.hfst dog.hfst\nconcatenates cat.hfst with dog.hfst and writes results to catdog.hfst\n\n",
        common.program_name
    );
}

// [spec:hfst:def:hfst-concatenate.parse-options-fn]
// [spec:hfst:sem:hfst-concatenate.parse-options-fn]
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
            has_arg: 0,
            val: b'F' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "do-not-harmonize",
            has_arg: 0,
            val: b'H' as i32,
        });
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: binary
        // cases, then common cases, then the tool's own ('F'/'H'), then the
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
        if c == b'F' as i32 {
            options.harmonize_flags = true;
            continue;
        }
        if c == b'H' as i32 {
            options.harmonize = false;
            continue;
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    check_binary_params(&mut common, &opt, args);
    check_common_params(&mut common);
    Ok((common, options))
}

// [spec:hfst:def:hfst-concatenate.concatenate-streams-fn]
// [spec:hfst:sem:hfst-concatenate.concatenate-streams-fn]
// The streams loop lives in crate::binary_ops::run_binary_streams_tool;
// this descriptor plus the pre-apply/apply closures in run carry the
// tool's behaviour contract.
const SPEC: BinaryOpSpec = BinaryOpSpec {
    tool_name: "hfst-concatenate",
    mismatch_noun: "concatenation",
    could_not_verb: "concatenate",
    could_not_noun: "concatenation",
    name_op: "concatenate",
    formula: "\u{22c5}",
    verbose_begin: |firstname, secondname| {
        format!("Concatenating {} and {}", firstname, secondname)
    },
    loop_style: LoopStyle::Standard,
    retry: RetryPolicy::TypeMismatchOnly,
    flush_each_round: false,
    flush_at_end: true,
};

// [spec:hfst:def:hfst-concatenate.main-fn]
// [spec:hfst:sem:hfst-concatenate.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstConcatenate");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    let mut op = ConcatenateOp {
        harmonize: options.harmonize,
        harmonize_flags: options.harmonize_flags,
    };
    run_binary_streams_tool(&common, &SPEC, &mut op)
}

struct ConcatenateOp {
    harmonize: bool,
    harmonize_flags: bool,
}

impl BinaryToolOp for ConcatenateOp {
    fn pre_apply<B: AlgebraBackend>(
        &mut self,
        common: &CommonOptions,
        first: &mut HfstTransducer<B>,
        second: &mut HfstTransducer<B>,
        _ctx: &PairContext,
    ) -> Result<(), i32> {
        let both_have_flags = first.has_flag_diacritics() && second.has_flag_diacritics();
        if both_have_flags {
            if !self.harmonize_flags {
                if !common.silent {
                    warning(
                        common,
                        0,
                        0,
                        "The arguments contain flag diacritics. Use -F to harmonize them.",
                    );
                }
            } else if let Err(e) = first.harmonize_flag_diacritics(second, false) {
                error(common, 1, 0, &format!("{e}"));
                return Err(1);
            }
        }
        Ok(())
    }

    fn apply<B: AlgebraBackend>(
        &mut self,
        first: &mut HfstTransducer<B>,
        second: &HfstTransducer<B>,
    ) -> hfst::error::Result<()> {
        first.concatenate(second, self.harmonize).map(|_| ())
    }
}
