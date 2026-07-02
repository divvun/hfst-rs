//! Faithful 1:1 port of tools/src/hfst-compose.cc — the transducer composition
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments). A binary tool:
//! it reads two input streams (firstfile + secondfile) and composes them; the
//! shared scaffolding lives in crate::binary_ops.

use crate::binary_ops::{
    BinaryOpSpec, LoopStyle, PairContext, RetryPolicy, print_do_not_convert_error,
    run_binary_streams_tool,
};
use crate::globals;
use crate::hfst_commandline::{
    EXIT_CONTINUE, convert_transducers, error, extend_options_from_env, hfst_set_program_name,
    print_more_info, print_report_bugs, warning,
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
use hfst::hfst_transducer::{EngineConfig, HfstTransducer};
use std::io::Write;

static mut HARMONIZE_FLAGS: bool = false;
static mut HARMONIZE: bool = true;
// '--xfst flag-is-epsilon' (was the 'flag_is_epsilon_in_composition' file-static
// global in the library; now threaded into compose via EngineConfig).
static mut FLAG_IS_EPSILON: bool = false;
// '--xerox-composition' (was the 'xerox_composition' file-static global in the
// library; now threaded into compose via EngineConfig).
static mut XEROX_COMPOSITION: bool = false;

// [spec:hfst:def:hfst-compose.print-usage-fn]
// [spec:hfst:sem:hfst-compose.print-usage-fn]
fn print_usage() {
    let mut msg = globals::message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let program_name = globals::program_name();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nCompose two transducers\n\n",
        program_name
    );
    print_common_program_options(&mut *msg);
    print_common_binary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Composition options:\n  -x, --xerox-composition=VALUE Whether flag diacritics are treated as ordinary\n                                symbols in composition (default is false).\n  -X, --xfst=VARIABLE    Toggle xfst compatibility option VARIABLE.\nHarmonization:\n  -H, --do-not-harmonize Do not harmonize symbols.\n  -F, --harmonize-flags  Harmonize flag diacritics.\n"
    );
    let _ = write!(msg, "\n");
    print_common_binary_program_parameter_instructions(&mut *msg);
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "Xfst variables are {{flag-is-epsilon (default OFF)}}.\n"
    );
    let _ = write!(
        msg,
        "VALUE can be one of the following: [true|false], [yes|no] or [ON|OFF],\n"
    );
    let _ = write!(msg, "false being the default.\n");
    let _ = write!(
        msg,
        "\nExamples:\n  {} -o cat2dog.hfst cat2mouse.hfst mouse2dog.hfst  composes two automata\n\n",
        program_name
    );
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-compose.parse-options-fn]
// [spec:hfst:sem:hfst-compose.parse-options-fn]
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
            long_options.push(getopt::GetOpt {
                name: "xerox-composition",
                has_arg: 1,
                val: b'x' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "xfst",
                has_arg: 1,
                val: b'X' as i32,
            });
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: binary
            // cases, then common cases, then the tool's own, then the terminal
            // error arm.
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
            } else if c == b'H' as i32 {
                HARMONIZE = false;
                continue;
            } else if c == b'x' as i32 {
                let argument = getopt::optarg();
                if argument == "yes" || argument == "true" || argument == "ON" {
                    XEROX_COMPOSITION = true;
                } else if argument == "no" || argument == "false" || argument == "OFF" {
                    XEROX_COMPOSITION = false;
                } else {
                    let _ = write!(
                        std::io::stderr(),
                        "Error: unknown option to --xerox-composition: '{}'\n",
                        getopt::optarg()
                    );
                    return 1;
                }
                continue;
            } else if c == b'X' as i32 {
                let argument = getopt::optarg();
                if argument == "flag-is-epsilon" {
                    FLAG_IS_EPSILON = true;
                } else {
                    let _ = write!(
                        std::io::stderr(),
                        "Error: unknown option to --xfst: '{}'\n",
                        getopt::optarg()
                    );
                    return 1;
                }
                continue;
            }
            return handle_error_case(c);
        }

        check_binary_params(args);
        check_common_params();
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-compose.compose-streams-fn]
// [spec:hfst:sem:hfst-compose.compose-streams-fn]
// The streams loop lives in crate::binary_ops::run_binary_streams_tool;
// this descriptor plus the pre-apply (harmonize-flags gate with its own
// convert-and-retry) and apply closures in real_main carry the tool's
// behaviour contract.
const SPEC: BinaryOpSpec = BinaryOpSpec {
    tool_name: "hfst-compose",
    mismatch_noun: "composition",
    could_not_verb: "compose",
    could_not_noun: "composition",
    name_op: "compose",
    formula: "\u{2218}",
    verbose_begin: |firstname, secondname| format!("Composing {} and {}", firstname, secondname),
    loop_style: LoopStyle::Compose,
    retry: RetryPolicy::TypeMismatchOnly,
    flush_each_round: false,
    flush_at_end: true,
};

// [spec:hfst:def:hfst-compose.main-fn]
// [spec:hfst:sem:hfst-compose.main-fn]
pub fn run(args: Vec<String>) -> i32 {
    unsafe { real_main(args) }
}

unsafe fn real_main(mut args: Vec<String>) -> i32 {
    unsafe {
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstCompose");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        let harmonize = HARMONIZE;
        let harmonize_flags = HARMONIZE_FLAGS;
        let cfg = EngineConfig {
            flag_is_epsilon_in_composition: FLAG_IS_EPSILON,
            xerox_composition: XEROX_COMPOSITION,
            ..EngineConfig::default()
        };
        let mut pre_apply = |first: &mut HfstTransducer,
                             second: &mut HfstTransducer,
                             ctx: &PairContext|
         -> Result<(), i32> {
            let has_flags = first.has_flag_diacritics() || second.has_flag_diacritics();
            if has_flags {
                if !harmonize_flags {
                    if !globals::SILENT {
                        warning(
                            0,
                            0,
                            "At least one of the arguments contains flag diacritics. Use -F to harmonize them.",
                        );
                    }
                } else if let Err(e) = first.harmonize_flag_diacritics(second, true) {
                    if matches!(e.kind, hfst::error::ErrorKind::TransducerTypeMismatch) {
                        if globals::ALLOW_TRANSDUCER_CONVERSION {
                            if let Err(e) = convert_transducers(first, second) {
                                error(1, 0, &format!("{e}"));
                                return Err(1);
                            }
                            if let Err(e2) = first.harmonize_flag_diacritics(second, true) {
                                error(1, 0, &format!("{e2}"));
                                return Err(1);
                            }
                        } else {
                            print_do_not_convert_error(&SPEC, ctx);
                            return Err(1);
                        }
                    } else {
                        error(1, 0, &format!("{e}"));
                        return Err(1);
                    }
                }
            }
            Ok(())
        };
        run_binary_streams_tool(&SPEC, Some(&mut pre_apply), &mut |first, second| {
            first
                .compose_with_config(second, harmonize, &cfg)
                .map(|_| ())
        })
    }
}
