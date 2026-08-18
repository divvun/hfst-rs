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
use crate::memory_limit::{self, LimitSource, ResolvedMemoryLimit};
use hfst::backend::AlgebraBackend;
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_transducer::{EngineConfig, FlagDiacriticComposeOverlay, HfstTransducer};
use std::io::Write;

const GETOPT_MEMORY_LIMIT: i32 = 0x100;

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
    /// '--memory-limit=SIZE': allowance for budget-aware compose working data.
    memory_limit_bytes: Option<u64>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            harmonize_flags: false,
            harmonize: true,
            flag_is_epsilon: false,
            xerox_composition: false,
            memory_limit_bytes: None,
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
        "Composition options:\n  -x, --xerox-composition=VALUE Whether flag diacritics are treated as ordinary\n                                symbols in composition (default is false).\n  -X, --xfst=VARIABLE    Toggle xfst compatibility option VARIABLE.\n      --memory-limit=SIZE\n                         Working-memory allowance for budget-aware OpenFst tropical\n                         and Foma compose state (default: 50% of available RAM;\n                         excess spills).\nHarmonization:\n  -H, --do-not-harmonize Do not harmonize symbols.\n  -F, --harmonize-flags  Harmonize flag diacritics.\n"
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
    let _ = writeln!(
        msg,
        "SIZE is an integer byte count with an optional binary K/KB/KiB through T/TB/TiB suffix; 0 forces nonempty budget-aware products to spill."
    );
    let _ = writeln!(
        msg,
        "The allowance is not an RSS ceiling: loaded operands and the final result are not included."
    );
    let _ = writeln!(
        msg,
        "HFST_COMPOSE_MEMORY_LIMIT supplies SIZE when --memory-limit is absent."
    );
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
        long_options.push(getopt::GetOpt {
            name: "memory-limit",
            has_arg: getopt::REQUIRED_ARGUMENT,
            val: GETOPT_MEMORY_LIMIT,
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
        } else if c == GETOPT_MEMORY_LIMIT {
            let argument = opt.optarg();
            options.memory_limit_bytes = match memory_limit::parse_size(&argument) {
                Ok(bytes) => Some(bytes),
                Err(detail) => {
                    let _ = writeln!(
                        std::io::stderr(),
                        "{}: invalid value for --memory-limit: {detail}",
                        common.program_name
                    );
                    return Err(1);
                }
            };
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

    // Resolve the allowance before either input stream is opened, so the
    // automatic 50% value is a stable startup snapshot rather than a moving
    // target as transducers are loaded.
    let memory_limit = match memory_limit::resolve(options.memory_limit_bytes) {
        Ok(limit) => limit,
        Err(detail) => {
            let _ = writeln!(std::io::stderr(), "{}: {detail}", common.program_name);
            return 1;
        }
    };
    let mut op = ComposeOp {
        harmonize: options.harmonize,
        harmonize_flags: options.harmonize_flags,
        flag_overlay: None,
        memory_limit,
        memory_policy_reported: false,
        cfg: EngineConfig {
            flag_is_epsilon_in_composition: options.flag_is_epsilon,
            xerox_composition: options.xerox_composition,
            compose_memory_limit_bytes: Some(memory_limit.allowance_bytes),
            ..EngineConfig::default()
        },
    };
    run_binary_streams_tool(&common, &SPEC, &mut op)
}

struct ComposeOp {
    harmonize: bool,
    harmonize_flags: bool,
    flag_overlay: Option<FlagDiacriticComposeOverlay>,
    memory_limit: ResolvedMemoryLimit,
    memory_policy_reported: bool,
    cfg: EngineConfig,
}

fn supports_compose_memory_limit(implementation: ImplementationType) -> bool {
    implementation == ImplementationType::TROPICAL_OPENFST_TYPE
        || implementation == ImplementationType::FOMA_TYPE
}

fn explicit_memory_limit_name(source: LimitSource) -> Option<&'static str> {
    match source {
        LimitSource::Cli => Some("--memory-limit"),
        LimitSource::Environment => Some("HFST_COMPOSE_MEMORY_LIMIT"),
        LimitSource::Automatic | LimitSource::ProbeFallback => None,
    }
}

impl ComposeOp {
    fn validate_and_report_memory_policy(
        &mut self,
        common: &CommonOptions,
        implementation: ImplementationType,
    ) -> Result<(), i32> {
        if !supports_compose_memory_limit(implementation) {
            if let Some(name) = explicit_memory_limit_name(self.memory_limit.source) {
                error(
                    common,
                    1,
                    0,
                    &format!(
                        "{name} is not supported for {implementation:?} composition; bounded spilling is available for OpenFst tropical and Foma composition"
                    ),
                );
                return Err(1);
            }
            return Ok(());
        }

        if self.memory_policy_reported {
            return Ok(());
        }
        self.memory_policy_reported = true;
        if common.silent {
            return Ok(());
        }

        if self.memory_limit.source == LimitSource::ProbeFallback {
            warning(
                common,
                0,
                0,
                "Could not determine available RAM; using a 0-byte composition memory allowance and spilling immediately. Use --memory-limit to override.",
            );
        }
        if self.memory_limit.cgroup_clamped
            && let Some(requested) = self.memory_limit.requested_bytes
        {
            warning(
                common,
                0,
                0,
                &format!(
                    "Requested composition memory allowance of {requested} bytes exceeds current cgroup headroom; using {} bytes.",
                    self.memory_limit.allowance_bytes
                ),
            );
        }
        Ok(())
    }
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
        self.flag_overlay = None;
        self.validate_and_report_memory_policy(common, <B as hfst::backend::Backend>::TYPE)?;
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
            } else {
                let prepared = if B::SUPPORTS_FLAG_OVERLAY
                    && !self.cfg.flag_is_epsilon_in_composition
                    && !self.cfg.xerox_composition
                {
                    first.prepare_flag_diacritics_for_compose(second).map(Some)
                } else {
                    first.harmonize_flag_diacritics(second, true).map(|()| None)
                };
                match prepared {
                    Ok(overlay) => self.flag_overlay = overlay,
                    Err(e) => {
                        error(common, 1, 0, &format!("{e}"));
                        return Err(1);
                    }
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
        first
            .compose_with_config_and_flag_overlay(
                second,
                self.harmonize,
                &self.cfg,
                self.flag_overlay.as_ref(),
            )
            .map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_memory_limit_backend_scope_includes_foma() {
        assert!(supports_compose_memory_limit(
            ImplementationType::TROPICAL_OPENFST_TYPE
        ));
        assert!(supports_compose_memory_limit(ImplementationType::FOMA_TYPE));
    }
}
