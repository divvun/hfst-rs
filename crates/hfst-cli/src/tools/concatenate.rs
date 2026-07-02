//! Faithful 1:1 port of tools/src/hfst-concatenate.cc — the transducer
//! concatenation command-line tool. Drives the hfst-cli foundation (globals,
//! getopt, commandline, program-options, tool-metadata, inc fragments).
//!
//! This is a BINARY tool: it reads two input streams (firststream and
//! secondstream) and writes their pairwise concatenation; the shared
//! scaffolding lives in crate::binary_ops.

use crate::binary_ops::{
    BinaryOpSpec, LoopStyle, PairContext, RetryPolicy, run_binary_streams_tool,
};
use crate::globals;
use crate::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_from_env, hfst_set_program_name, warning,
};
use crate::hfst_getopt as getopt;
use crate::hfst_program_options::{
    hfst_getopt_binary_long, hfst_getopt_common_long, print_common_binary_program_options,
    print_common_binary_program_parameter_instructions, print_common_program_options,
};
use crate::inc::{
    CaseResult, check_binary_params, check_common_params, handle_binary_case, handle_common_case,
    handle_error_case,
};
use hfst::hfst_transducer::HfstTransducer;
use std::io::Write;

static mut HARMONIZE_FLAGS: bool = false;
static mut HARMONIZE: bool = true;

// [spec:hfst:def:hfst-concatenate.print-usage-fn]
// [spec:hfst:sem:hfst-concatenate.print-usage-fn]
fn print_usage() {
    let mut msg = globals::message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let program_name = globals::program_name();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nConcatenate two transducers\n\n",
        program_name
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
        program_name
    );
}

// [spec:hfst:def:hfst-concatenate.parse-options-fn]
// [spec:hfst:sem:hfst-concatenate.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_from_env(args);
        // use of this function requires options are settable on global scope
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
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: binary
            // cases, then common cases, then the tool's own ('F'/'H'), then the
            // terminal error arm.
            match handle_binary_case(c) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_common_case(c, print_usage) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            if c == b'F' as i32 {
                HARMONIZE_FLAGS = true;
                continue;
            }
            if c == b'H' as i32 {
                HARMONIZE = false;
                continue;
            }
            return handle_error_case(c);
        }

        check_binary_params(args);
        check_common_params();
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-concatenate.concatenate-streams-fn]
// [spec:hfst:sem:hfst-concatenate.concatenate-streams-fn]
// The streams loop lives in crate::binary_ops::run_binary_streams_tool;
// this descriptor plus the pre-apply/apply closures in real_main carry the
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
pub fn run(args: Vec<String>) -> i32 {
    unsafe { real_main(args) }
}

unsafe fn real_main(mut args: Vec<String>) -> i32 {
    unsafe {
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstConcatenate");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        let harmonize = HARMONIZE;
        let harmonize_flags = HARMONIZE_FLAGS;
        let mut pre_apply = |first: &mut HfstTransducer,
                             second: &mut HfstTransducer,
                             _ctx: &PairContext|
         -> Result<(), i32> {
            let both_have_flags = first.has_flag_diacritics() && second.has_flag_diacritics();
            if both_have_flags {
                if !harmonize_flags {
                    if !globals::SILENT {
                        warning(
                            0,
                            0,
                            "The arguments contain flag diacritics. Use -F to harmonize them.",
                        );
                    }
                } else if let Err(e) = first.harmonize_flag_diacritics(second, false) {
                    error(1, 0, &format!("{e}"));
                    return Err(1);
                }
            }
            Ok(())
        };
        run_binary_streams_tool(&SPEC, Some(&mut pre_apply), &mut |first, second| {
            first.concatenate(second, harmonize).map(|_| ())
        })
    }
}
