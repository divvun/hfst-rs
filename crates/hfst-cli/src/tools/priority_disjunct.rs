//! Faithful 1:1 port of tools/src/hfst-priority-disjunct.cc — the transducer
//! priority disjunction (priority union) command-line tool. Drives the
//! hfst-cli foundation (globals, getopt, commandline, program-options,
//! tool-metadata, inc fragments). A BINARY tool: it reads two input streams
//! (firstfile + secondfile) and writes their priority union; the shared
//! scaffolding lives in crate::binary_ops.

use crate::binary_ops::{BinaryOpSpec, LoopStyle, RetryPolicy, run_binary_streams_tool};
use crate::globals;
use crate::hfst_commandline::{EXIT_CONTINUE, extend_options_from_env, hfst_set_program_name};
use crate::hfst_getopt as getopt;
use crate::hfst_program_options::{
    hfst_getopt_binary_long, hfst_getopt_common_long, print_common_binary_program_options,
    print_common_binary_program_parameter_instructions, print_common_program_options,
};
use crate::inc::{
    CaseResult, check_binary_params, check_common_params, handle_binary_case, handle_common_case,
    handle_error_case,
};
use std::io::Write;

static mut HARMONIZE_FLAGS: bool = false;
static mut HARMONIZE: bool = true;

// [spec:hfst:def:hfst-priority-disjunct.print-usage-fn]
// [spec:hfst:sem:hfst-priority-disjunct.print-usage-fn]
fn print_usage() {
    let mut msg = globals::message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nDisjunct (union, OR) two transducers\n\n",
        globals::program_name()
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
        globals::program_name()
    );
}

// [spec:hfst:def:hfst-priority-disjunct.parse-options-fn]
// [spec:hfst:sem:hfst-priority-disjunct.parse-options-fn]
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
                name: "do-not-harmonize",
                has_arg: getopt::NO_ARGUMENT,
                val: b'H' as i32,
            });
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: binary
            // cases, then common cases, then the tool's own ('H'), then the
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

// [spec:hfst:def:hfst-priority-disjunct.priority-disjunct-streams-fn]
// [spec:hfst:sem:hfst-priority-disjunct.priority-disjunct-streams-fn]
// The streams loop lives in crate::binary_ops::run_binary_streams_tool;
// this descriptor plus the apply closure in real_main carry the tool's
// behaviour contract.
const SPEC: BinaryOpSpec = BinaryOpSpec {
    tool_name: "hfst-priority-disjunct",
    mismatch_noun: "priority disjunction",
    could_not_verb: "priority disjunct",
    could_not_noun: "priority disjunction",
    name_op: "union",
    formula: "\u{222a}",
    verbose_begin: |firstname, secondname| format!("Disjuncting {} and {}", firstname, secondname),
    loop_style: LoopStyle::Standard,
    retry: RetryPolicy::AnyError,
    flush_each_round: true,
    flush_at_end: false,
};

// [spec:hfst:def:hfst-priority-disjunct.main-fn]
// [spec:hfst:sem:hfst-priority-disjunct.main-fn]
pub fn run(args: Vec<String>) -> i32 {
    unsafe { real_main(args) }
}

unsafe fn real_main(mut args: Vec<String>) -> i32 {
    unsafe {
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstPriorityDisjunct");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        let _ = HARMONIZE_FLAGS;
        let _ = HARMONIZE;
        run_binary_streams_tool(&SPEC, None, &mut |first, second| {
            // C: 'first->priority_union(*second)'; no harmonize parameter.
            first.priority_union(second).map(|_| ())
        })
    }
}
