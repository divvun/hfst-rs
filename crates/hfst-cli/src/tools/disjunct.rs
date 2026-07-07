//! Faithful 1:1 port of tools/src/hfst-disjunct.cc — the transducer
//! disjunction (union, OR) command-line tool. Drives the hfst-cli foundation
//! (globals, getopt, commandline, program-options, tool-metadata, inc
//! fragments). A BINARY tool: it reads two input streams (firstfile +
//! secondfile) and writes their disjunction; the shared scaffolding lives in
//! crate::binary_ops.

use crate::binary_ops::{
    BinaryOpSpec, BinaryToolOp, LoopStyle, RetryPolicy, run_binary_streams_tool,
};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{extend_options_from_env, hfst_set_program_name};
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

/// hfst-disjunct's own options (the former tool-specific `static mut`s).
struct Options {
    /// '-F, --harmonize-flags': harmonize flag diacritics.
    harmonize_flags: bool,
    /// '-H, --do-not-harmonize' clears this (harmonize symbols; default true).
    harmonize: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            harmonize_flags: false,
            harmonize: true,
        }
    }
}

// [spec:hfst:def:hfst-disjunct.print-usage-fn]
// [spec:hfst:sem:hfst-disjunct.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    let mut msg = common.message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nDisjunct (union, OR) two transducers\n\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    print_common_binary_program_options(&mut *msg);
    let _ = write!(msg, "\n");
    print_common_binary_program_parameter_instructions(&mut *msg);
    let _ = write!(
        msg,
        "Harmonization:\n  -H, --do-not-harmonize Do not harmonize symbols.\n  -F, --harmonize-flags  Harmonize flag diacritics.\n"
    );
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "\nExamples:\n  {} -o cat_or_dog.hfst cat.hfst dog.hfst\n\n",
        common.program_name
    );
}

// [spec:hfst:def:hfst-disjunct.parse-options-fn]
// [spec:hfst:sem:hfst-disjunct.parse-options-fn]
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
            name: "do-not-harmonize",
            has_arg: getopt::NO_ARGUMENT,
            val: b'H' as i32,
        });
        // add tool-specific options here
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: common
        // cases, then binary cases, then the tool's own ('H'), then the
        // terminal error arm.
        match handle_common_case(&mut common, &opt, c, print_usage) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        match handle_binary_case(&mut common, &opt, c) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        if c == b'H' as i32 {
            options.harmonize = false;
            continue;
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    check_common_params(&mut common);
    check_binary_params(&mut common, &opt, args);
    Ok((common, options))
}

// [spec:hfst:def:hfst-disjunct.disjunct-streams-fn]
// [spec:hfst:sem:hfst-disjunct.disjunct-streams-fn]
// The streams loop lives in crate::binary_ops::run_binary_streams_tool;
// this descriptor plus the apply closure in run carry the tool's
// behaviour contract.
const SPEC: BinaryOpSpec = BinaryOpSpec {
    tool_name: "hfst-disjunct",
    mismatch_noun: "disjunction",
    could_not_verb: "disjunct",
    could_not_noun: "disjunction",
    name_op: "union",
    formula: "\u{222a}",
    verbose_begin: |firstname, secondname| format!("Disjuncting {} and {}", firstname, secondname),
    loop_style: LoopStyle::Standard,
    retry: RetryPolicy::AnyError,
    flush_each_round: true,
    flush_at_end: false,
};

// [spec:hfst:def:hfst-disjunct.main-fn]
// [spec:hfst:sem:hfst-disjunct.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstDisjunct");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    let _ = options.harmonize_flags;
    let mut op = DisjunctOp {
        harmonize: options.harmonize,
    };
    run_binary_streams_tool(&common, &SPEC, &mut op)
}

struct DisjunctOp {
    harmonize: bool,
}

impl BinaryToolOp for DisjunctOp {
    fn apply<B: AlgebraBackend>(
        &mut self,
        first: &mut HfstTransducer<B>,
        second: &HfstTransducer<B>,
    ) -> hfst::error::Result<()> {
        first.disjunct(second, self.harmonize).map(|_| ())
    }
}
