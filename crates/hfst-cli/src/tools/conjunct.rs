//! Faithful 1:1 port of tools/src/hfst-conjunct.cc — the transducer
//! conjunction (intersect, AND) command-line tool. Drives the hfst-cli
//! foundation (globals, getopt, commandline, program-options, tool-metadata,
//! inc fragments). A BINARY tool: it reads two input streams (first + second);
//! the shared scaffolding lives in crate::binary_ops.

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

/// hfst-conjunct's own options (the former tool-specific `static mut`s).
struct Options {
    /// '-F, --harmonize-flags': harmonize flag diacritics.
    harmonize_flags: bool,
    /// '-H, --do-not-harmonize': whether to harmonize (default true).
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

// [spec:hfst:def:hfst-conjunct.print-usage-fn]
// [spec:hfst:sem:hfst-conjunct.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    let mut msg = common.message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nConjunct (intersect, AND) two transducers\n\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    print_common_binary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Flag diacritics:\n  -F, --harmonize-flags  Harmonize flag diacritics\n  -H, --do-not-harmonize Do not harmonize\n"
    );
    let _ = write!(msg, "\n");
    print_common_binary_program_parameter_instructions(&mut *msg);
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "\nExamples:\n  {} -o dog.hfst cat_or_dog.hfst dog_or_mouse.hfst\n\n",
        common.program_name
    );
}

// [spec:hfst:def:hfst-conjunct.parse-options-fn]
// [spec:hfst:sem:hfst-conjunct.parse-options-fn]
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
            val: 'F' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "do-not-harmonize",
            has_arg: 0,
            val: 'H' as i32,
        });
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: binary
        // cases, common cases, then the tool's own ('F'/'H'), then the
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
        if c == 'F' as i32 {
            options.harmonize_flags = true;
            continue;
        }
        if c == 'H' as i32 {
            options.harmonize = false;
            continue;
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    check_binary_params(&mut common, &opt, args);
    check_common_params(&mut common);
    Ok((common, options))
}

// [spec:hfst:def:hfst-conjunct.conjunct-streams-fn]
// [spec:hfst:sem:hfst-conjunct.conjunct-streams-fn]
// The streams loop lives in crate::binary_ops::run_binary_streams_tool;
// this descriptor plus the pre-apply/apply closures in run carry the
// tool's behaviour contract.
const SPEC: BinaryOpSpec = BinaryOpSpec {
    tool_name: "hfst-conjunct",
    mismatch_noun: "conjunction",
    could_not_verb: "conjunct",
    could_not_noun: "conjunction",
    name_op: "intersect",
    formula: "\u{2229}",
    verbose_begin: |firstname, secondname| format!("Intersecting {} and {}", firstname, secondname),
    loop_style: LoopStyle::Standard,
    retry: RetryPolicy::AnyError,
    flush_each_round: false,
    flush_at_end: true,
};

// [spec:hfst:def:hfst-conjunct.main-fn]
// [spec:hfst:sem:hfst-conjunct.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstConjunct");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };
    let mut op = ConjunctOp {
        harmonize: options.harmonize,
        harmonize_flags: options.harmonize_flags,
    };
    run_binary_streams_tool(&common, &SPEC, &mut op)
}

struct ConjunctOp {
    harmonize: bool,
    harmonize_flags: bool,
}

impl BinaryToolOp for ConjunctOp {
    fn pre_apply<B: AlgebraBackend>(
        &mut self,
        common: &CommonOptions,
        first: &mut HfstTransducer<B>,
        second: &mut HfstTransducer<B>,
        _ctx: &PairContext,
    ) -> Result<(), i32> {
        if first.has_flag_diacritics() || second.has_flag_diacritics() {
            if !self.harmonize_flags {
                if !common.silent {
                    warning(
                        common,
                        0,
                        0,
                        "At least one of the argumentes contains flag diacritics. Use -F to harmonize them.",
                    );
                }
            } else {
                // C: 'first->harmonize_flag_diacritics(*second)' — relies
                // on the default 'insert_renamed_flags=true'.
                if let Err(e) = first.harmonize_flag_diacritics(second, true) {
                    error(common, 1, 0, &format!("{e}"));
                    return Err(1);
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
        first.intersect(second, self.harmonize).map(|_| ())
    }
}
