//! Faithful 1:1 port of tools/src/hfst-compose.cc — the transducer composition
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments). A binary tool:
//! it reads two input streams (firstfile + secondfile) and composes them; the
//! shared scaffolding lives in crate::binary_ops.

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
use hfst::hfst_transducer::{EngineConfig, HfstTransducer};
use std::io::Write;

/// hfst-compose's own options (the former tool-specific `static mut`s).
struct Options {
    /// '-F, --harmonize-flags': harmonize flag diacritics.
    harmonize_flags: bool,
    /// '-H, --do-not-harmonize': off harmonizes symbols (default on).
    harmonize: bool,
    /// '--xfst flag-is-epsilon' (was the 'flag_is_epsilon_in_composition'
    /// file-static global in the library; now threaded into compose via
    /// EngineConfig).
    flag_is_epsilon: bool,
    /// '--xerox-composition' (was the 'xerox_composition' file-static global in
    /// the library; now threaded into compose via EngineConfig).
    xerox_composition: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            harmonize_flags: false,
            harmonize: true,
            flag_is_epsilon: false,
            xerox_composition: false,
        }
    }
}

// [spec:hfst:def:hfst-compose.print-usage-fn]
// [spec:hfst:sem:hfst-compose.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    let mut msg = common.message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nCompose two transducers\n\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    print_common_binary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Composition options:\n  -x, --xerox-composition=VALUE Whether flag diacritics are treated as ordinary\n                                symbols in composition (default is false).\n  -X, --xfst=VARIABLE    Toggle xfst compatibility option VARIABLE.\nHarmonization:\n  -H, --do-not-harmonize Do not harmonize symbols.\n  -F, --harmonize-flags  Harmonize flag diacritics.\n"
    );
    let _ = writeln!(msg);
    print_common_binary_program_parameter_instructions(&mut *msg);
    let _ = writeln!(msg);
    let _ = writeln!(msg, "Xfst variables are {{flag-is-epsilon (default OFF)}}.");
    let _ = writeln!(
        msg,
        "VALUE can be one of the following: [true|false], [yes|no] or [ON|OFF],"
    );
    let _ = writeln!(msg, "false being the default.");
    let _ = write!(
        msg,
        "\nExamples:\n  {} -o cat2dog.hfst cat2mouse.hfst mouse2dog.hfst  composes two automata\n\n",
        common.program_name
    );
}

// [spec:hfst:def:hfst-compose.parse-options-fn]
// [spec:hfst:sem:hfst-compose.parse-options-fn]
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
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: binary
        // cases, then common cases, then the tool's own, then the terminal
        // error arm.
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
        } else if c == b'H' as i32 {
            options.harmonize = false;
            continue;
        } else if c == b'x' as i32 {
            let argument = opt.optarg();
            if argument == "yes" || argument == "true" || argument == "ON" {
                options.xerox_composition = true;
            } else if argument == "no" || argument == "false" || argument == "OFF" {
                options.xerox_composition = false;
            } else {
                let _ = writeln!(
                    std::io::stderr(),
                    "Error: unknown option to --xerox-composition: '{}'",
                    opt.optarg()
                );
                return Err(1);
            }
            continue;
        } else if c == b'X' as i32 {
            let argument = opt.optarg();
            if argument == "flag-is-epsilon" {
                options.flag_is_epsilon = true;
            } else {
                let _ = writeln!(
                    std::io::stderr(),
                    "Error: unknown option to --xfst: '{}'",
                    opt.optarg()
                );
                return Err(1);
            }
            continue;
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    check_binary_params(&mut common, &opt, args);
    check_common_params(&mut common);
    Ok((common, options))
}

// [spec:hfst:def:hfst-compose.compose-streams-fn]
// [spec:hfst:sem:hfst-compose.compose-streams-fn]
// The streams loop lives in crate::binary_ops::run_binary_streams_tool;
// this descriptor plus the pre-apply (harmonize-flags gate with its own
// convert-and-retry) and apply closures in run carry the tool's
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
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstCompose");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    let mut op = ComposeOp {
        harmonize: options.harmonize,
        harmonize_flags: options.harmonize_flags,
        cfg: EngineConfig {
            flag_is_epsilon_in_composition: options.flag_is_epsilon,
            xerox_composition: options.xerox_composition,
            ..EngineConfig::default()
        },
    };
    run_binary_streams_tool(&common, &SPEC, &mut op)
}

struct ComposeOp {
    harmonize: bool,
    harmonize_flags: bool,
    cfg: EngineConfig,
}

impl BinaryToolOp for ComposeOp {
    // The harmonize-flags gate. (The C's catch-TransducerTypeMismatch,
    // convert-and-retry arm is gone: operands share a backend by construction
    // at this point — the driver converted at the stream boundary.)
    fn pre_apply<B: AlgebraBackend>(
        &mut self,
        common: &CommonOptions,
        first: &mut HfstTransducer<B>,
        second: &mut HfstTransducer<B>,
        _ctx: &PairContext<'_>,
    ) -> Result<(), i32> {
        let has_flags = first.has_flag_diacritics() || second.has_flag_diacritics();
        if has_flags {
            if !self.harmonize_flags {
                if !common.silent {
                    warning(
                        common,
                        0,
                        0,
                        "At least one of the arguments contains flag diacritics. Use -F to harmonize them.",
                    );
                }
            } else if let Err(e) = first.harmonize_flag_diacritics(second, true) {
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
        first
            .compose_with_config(second, self.harmonize, &self.cfg)
            .map(|_| ())
    }
}
