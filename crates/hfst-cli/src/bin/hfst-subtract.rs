//! Faithful 1:1 port of tools/src/hfst-subtract.cc — the transducer subtraction
//! (minus) command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments). A BINARY tool:
//! it reads two input streams (first + second); the shared scaffolding lives
//! in hfst_cli::binary_ops.

use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::binary_ops::{
    BinaryOpSpec, LoopStyle, PairContext, RetryPolicy, run_binary_streams_tool,
};
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_from_env, hfst_set_program_name, print_more_info,
    print_report_bugs, warning,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    hfst_getopt_binary_long, hfst_getopt_common_long, print_common_binary_program_options,
    print_common_binary_program_parameter_instructions, print_common_program_options,
};
use hfst_cli::inc::{
    CaseResult, check_binary_params, check_common_params, handle_binary_case, handle_common_case,
    handle_error_case,
};
use std::io::Write;

static mut HARMONIZE_FLAGS: bool = false;
static mut HARMONIZE: bool = true;

// [spec:hfst:def:hfst-subtract.print-usage-fn]
// [spec:hfst:sem:hfst-subtract.print-usage-fn]
fn print_usage() {
    let mut msg = globals::message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nSubtract (minus) two transducers\n\n",
        globals::program_name()
    );
    print_common_program_options(&mut *msg);
    print_common_binary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Flag diacritics:\n  -F, --harmonize-flags  Harmonize flag diacritics\n  -H, --do-not-harmonize Do not harmonize\n",
    );
    let _ = write!(msg, "\n");
    print_common_binary_program_parameter_instructions(&mut *msg);
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "\nExamples:\n  {} -o catdog.hfst cat.hfst dog.hfst  subtracts transducers\n\n",
        globals::program_name()
    );
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-subtract.parse-options-fn]
// [spec:hfst:sem:hfst-subtract.parse-options-fn]
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
                has_arg: getopt::NO_ARGUMENT,
                val: 'F' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "do-not-harmonize",
                has_arg: getopt::NO_ARGUMENT,
                val: 'H' as i32,
            });
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: binary
            // cases, common cases, then the tool's own ('F'/'H'), then the
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
            if c == 'F' as i32 {
                HARMONIZE_FLAGS = true;
                continue;
            }
            if c == 'H' as i32 {
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

// [spec:hfst:def:hfst-subtract.subtract-streams-fn]
// [spec:hfst:sem:hfst-subtract.subtract-streams-fn]
// The streams loop lives in hfst_cli::binary_ops::run_binary_streams_tool;
// this descriptor plus the pre-apply/apply closures in real_main carry the
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
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstSubtract");
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
            if second.has_flag_diacritics() {
                warning(
                    0,
                    0,
                    &format!(
                        "Warning: {} contains flag diacritics. The result of subtraction may be incorrect.",
                        globals::second_filename()
                    ),
                );
            }
            let first_has_flags = first.has_flag_diacritics();
            let second_has_flags = second.has_flag_diacritics();
            if first_has_flags && second_has_flags {
                if !harmonize_flags {
                    if !globals::SILENT {
                        warning(
                            0,
                            0,
                            "The argumentes contain flag diacritics. Use -F to harmonize them.",
                        );
                    }
                } else {
                    // C: 'first->harmonize_flag_diacritics(*second)' — relies
                    // on the default 'insert_renamed_flags=true'.
                    if let Err(e) = first.harmonize_flag_diacritics(second, true) {
                        error(1, 0, &format!("{e}"));
                        return Err(1);
                    }
                }
            }
            Ok(())
        };
        run_binary_streams_tool(&SPEC, Some(&mut pre_apply), &mut |first, second| {
            first.subtract(second, harmonize).map(|_| ())
        })
    }
}
