//! Faithful 1:1 port of tools/src/hfst-compose.cc — the transducer composition
//! command-line tool. A binary tool: it reads two input streams (firstfile +
//! secondfile) and composes them; the shared scaffolding lives in
//! crate::binary_ops and the option layer in crate::cli.

use crate::binary_ops::{
    BinaryOpSpec, BinaryToolOp, LoopStyle, PairContext, RetryPolicy, run_binary_streams_tool,
};
use crate::cli::{self, BinaryIo, CommonArgs, ToolArgs, ToolResult};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{error, hfst_set_program_name, warning};
use crate::memory_limit::{self, LimitSource, ResolvedMemoryLimit};
use hfst::backend::AlgebraBackend;
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_transducer::{EngineConfig, FlagDiacriticComposeOverlay, HfstTransducer};
use std::io::Write;

/// hfst-compose's command line.
// [spec:hfst:def:hfst-compose.parse-options-fn]
// [spec:hfst:sem:hfst-compose.parse-options-fn]
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(
    about = "Compose two transducers",
    after_help = "Xfst variables are {flag-is-epsilon (default OFF)}.
VALUE can be one of the following: [true|false], [yes|no] or [ON|OFF], false being the default.
SIZE, in --memory-limit=SIZE, is an integer byte count with an optional binary K/KB/KiB through T/TB/TiB suffix; 0 forces nonempty budget-aware products to spill.
The allowance is not an RSS ceiling: loaded operands and the final result are not included.
HFST_COMPOSE_MEMORY_LIMIT supplies SIZE when --memory-limit is absent.

Examples:
  hfst-compose -o cat2dog.hfst cat2mouse.hfst mouse2dog.hfst  composes two automata"
)]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: BinaryIo,

    /// Harmonize flag diacritics
    #[arg(short = 'F', long = "harmonize-flags")]
    harmonize_flags: bool,

    /// Do not harmonize symbols
    #[arg(short = 'H', long = "do-not-harmonize")]
    do_not_harmonize: bool,

    /// Whether flag diacritics are treated as ordinary symbols in
    /// composition (default is false)
    #[arg(short = 'x', long = "xerox-composition", value_name = "VALUE")]
    xerox_composition: Option<String>,

    /// Toggle xfst compatibility option VARIABLE
    #[arg(short = 'X', long = "xfst", value_name = "VARIABLE")]
    xfst: Option<String>,

    /// Working-memory allowance for budget-aware OpenFst tropical and Foma
    /// compose state, as --memory-limit=SIZE (default: 50% of available RAM;
    /// excess spills)
    #[arg(long = "memory-limit", value_name = "SIZE")]
    memory_limit: Option<String>,
}

impl Args {
    /// Case 'x': the xerox-composition vocabulary, rejected in the C's
    /// getopt loop with a bare stderr line and EXIT_FAILURE.
    fn xerox(&self) -> Result<bool, i32> {
        match self.xerox_composition.as_deref() {
            None => Ok(false),
            Some("yes") | Some("true") | Some("ON") => Ok(true),
            Some("no") | Some("false") | Some("OFF") => Ok(false),
            Some(other) => {
                let _ = writeln!(
                    std::io::stderr(),
                    "Error: unknown option to --xerox-composition: '{}'",
                    other
                );
                Err(1)
            }
        }
    }

    /// Case 'X': the one xfst variable this tool knows.
    fn flag_is_epsilon(&self) -> Result<bool, i32> {
        match self.xfst.as_deref() {
            None => Ok(false),
            Some("flag-is-epsilon") => Ok(true),
            Some(other) => {
                let _ = writeln!(
                    std::io::stderr(),
                    "Error: unknown option to --xfst: '{}'",
                    other
                );
                Err(1)
            }
        }
    }

    /// Case GETOPT_MEMORY_LIMIT: parse SIZE, or refuse before any input is
    /// opened.
    fn memory_limit_bytes(&self, common: &CommonOptions) -> Result<Option<u64>, i32> {
        let Some(argument) = self.memory_limit.as_deref() else {
            return Ok(None);
        };
        match memory_limit::parse_size(argument) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(detail) => {
                let _ = writeln!(
                    std::io::stderr(),
                    "{}: invalid value for --memory-limit: {detail}",
                    common.program_name
                );
                Err(1)
            }
        }
    }
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        self.io.apply(opts);
    }

    fn validate(&self, opts: &CommonOptions) -> ToolResult {
        // All three were rejected inside the C getopt loop, before the
        // parameter checks; run them here for the same ordering.
        self.xerox()?;
        self.flag_is_epsilon()?;
        self.memory_limit_bytes(opts)?;
        Ok(())
    }
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
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstCompose");
    let (common, args) = cli::parse::<Args>(common, args)?;

    // Resolve the allowance before either input stream is opened, so the
    // automatic 50% value is a stable startup snapshot rather than a moving
    // target as transducers are loaded.
    let memory_limit = match memory_limit::resolve(args.memory_limit_bytes(&common)?) {
        Ok(limit) => limit,
        Err(detail) => {
            let _ = writeln!(std::io::stderr(), "{}: {detail}", common.program_name);
            return Err(1);
        }
    };
    let mut op = ComposeOp {
        harmonize: !args.do_not_harmonize,
        harmonize_flags: args.harmonize_flags,
        flag_overlay: None,
        memory_limit,
        memory_policy_reported: false,
        cfg: EngineConfig {
            flag_is_epsilon_in_composition: args.flag_is_epsilon()?,
            xerox_composition: args.xerox()?,
            compose_memory_limit_bytes: Some(memory_limit.allowance_bytes),
            ..EngineConfig::default()
        },
    };
    cli::from_code(run_binary_streams_tool(&common, &SPEC, &mut op))
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
                let prepared = if B::SUPPORTS_FLAG_OVERLAY {
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
