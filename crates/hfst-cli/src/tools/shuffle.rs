//! Faithful 1:1 port of tools/src/hfst-shuffle.cc — the transducer shuffle
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments). A BINARY tool:
//! it reads two input streams (firstfile + secondfile) and writes their
//! shuffle; the shared scaffolding lives in crate::binary_ops.

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

// [spec:hfst:def:hfst-shuffle.print-usage-fn]
// [spec:hfst:sem:hfst-shuffle.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    let mut msg = common.message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nShuffle two transducers\n\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    print_common_binary_program_options(&mut *msg);
    let _ = writeln!(msg);
    print_common_binary_program_parameter_instructions(&mut *msg);
    let _ = writeln!(msg);
    let _ = write!(
        msg,
        "\nExamples:\n  {} -o shuffled.hfst cat.hfst dog.hfst\n\n",
        common.program_name
    );
}

// [spec:hfst:def:hfst-shuffle.parse-options-fn]
// [spec:hfst:sem:hfst-shuffle.parse-options-fn]
//
// Parse argv into the shared options; `Err(code)` is an exit code the caller
// should return (the former EXIT_CONTINUE sentinel is now `Ok`).
fn parse_options(mut common: CommonOptions, args: &mut Vec<String>) -> Result<CommonOptions, i32> {
    let mut opt = Getopt::new();
    extend_options_from_env(args);
    loop {
        let mut long_options: Vec<getopt::GetOpt> = Vec::new();
        long_options.extend(hfst_getopt_common_long());
        long_options.extend(hfst_getopt_binary_long());
        // add tool-specific options here
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: binary
        // cases, then common cases, then the terminal error arm. (The tool
        // defines no options of its own.)
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
        return Err(handle_error_case(&common, &opt, c));
    }

    check_binary_params(&mut common, &opt, args);
    check_common_params(&mut common);
    Ok(common)
}

// [spec:hfst:def:hfst-shuffle.shuffle-streams-fn]
// [spec:hfst:sem:hfst-shuffle.shuffle-streams-fn]
// The streams loop lives in crate::binary_ops::run_binary_streams_tool;
// this descriptor plus the apply closure in run carry the tool's
// behaviour contract. The ShuffleAutomata retry policy reproduces the C's
// outer catch (TransducersAreNotAutomataException) around the inner catch
// (TransducerTypeMismatchException).
const SPEC: BinaryOpSpec = BinaryOpSpec {
    tool_name: "hfst-shuffle",
    mismatch_noun: "shuffle",
    could_not_verb: "shuffle",
    could_not_noun: "shuffling",
    name_op: "shuffle",
    formula: "shuffle",
    verbose_begin: |firstname, secondname| format!("Shuffling {} and {}", firstname, secondname),
    loop_style: LoopStyle::Standard,
    retry: RetryPolicy::ShuffleAutomata,
    flush_each_round: false,
    flush_at_end: false,
};

// [spec:hfst:def:hfst-shuffle.main-fn]
// [spec:hfst:sem:hfst-shuffle.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstShuffle");
    let common = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    run_binary_streams_tool(&common, &SPEC, &mut ShuffleOp)
}

struct ShuffleOp;

impl BinaryToolOp for ShuffleOp {
    fn apply<B: AlgebraBackend>(
        &mut self,
        first: &mut HfstTransducer<B>,
        second: &HfstTransducer<B>,
    ) -> hfst::error::Result<()> {
        first.shuffle(second, true).map(|_| ())
    }
}
