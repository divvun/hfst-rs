//! Faithful 1:1 port of tools/src/hfst-prune-alphabet.cc — the transducer
//! alphabet-pruning command-line tool. Drives the hfst-cli foundation
//! (globals, getopt, commandline, program-options, tool-metadata, inc
//! fragments).

use crate::globals::CommonOptions;
use crate::hfst_commandline::{extend_options_from_env, hfst_set_program_name};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
use hfst::backend::AlgebraBackend;
use hfst::hfst_transducer::HfstTransducer;
use std::borrow::Cow;
use std::io::Write;

/// hfst-prune-alphabet's own options (the former tool-specific `static mut`s).
#[derive(Default)]
struct Options {
    /// '-f, --force' sets true; '-S, --safe' sets false (default).
    force_pruning: bool,
}

// [spec:hfst:def:hfst-prune-alphabet.print-usage-fn]
// [spec:hfst:sem:hfst-prune-alphabet.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nPrune the alphabet of a transducer\n\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Alphabet pruning options:\n  -f, --force            force pruning\n  -S, --safe             prune only if no unknown or identity symbols\n                         are used in the transducer (default)"
    );
    let _ = writeln!(msg);
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = writeln!(msg);
}

// [spec:hfst:def:hfst-prune-alphabet.parse-options-fn]
// [spec:hfst:sem:hfst-prune-alphabet.parse-options-fn]
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
        long_options.extend(hfst_getopt_unary_long());
        // add tool-specific options here
        long_options.push(getopt::GetOpt {
            name: "force",
            has_arg: getopt::NO_ARGUMENT,
            val: 'f' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "safe",
            has_arg: getopt::NO_ARGUMENT,
            val: 'S' as i32,
        });
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: common
        // cases, then unary cases, then the tool's own ('f'/'S'), then the
        // terminal error arm.
        match handle_common_case(&mut common, &opt, c, print_usage) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        match handle_unary_case(&mut common, &opt, c) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        match c as u8 as char {
            'f' => {
                options.force_pruning = true;
                continue;
            }
            'S' => {
                options.force_pruning = false;
                continue;
            }
            _ => {}
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    check_common_params(&mut common);
    check_unary_params(&mut common, &opt, args);
    Ok((common, options))
}

// [spec:hfst:def:hfst-prune-alphabet.process-stream-fn]
// [spec:hfst:sem:hfst-prune-alphabet.process-stream-fn]
//
// The stream loop lives in the shared unary driver; this op is the
// per-transducer body it dispatches into. The tool stamps a name but no
// formula, so `formula` keeps the trait default of None.
struct PruneAlphabetOp {
    force_pruning: bool,
}

impl UnaryToolOp for PruneAlphabetOp {
    fn verbose_begin(&self, inputname: &str) -> String {
        format!("Pruning {}", inputname)
    }

    fn verbose_sep(&self) -> &'static str {
        " "
    }

    fn name_op(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed("prune-alphabet"))
    }

    fn apply<B: AlgebraBackend>(
        &mut self,
        _common: &CommonOptions,
        t: &mut HfstTransducer<B>,
    ) -> hfst::error::Result<()> {
        t.prune_alphabet(self.force_pruning).map(|_| ())
    }
}

const SPEC: UnaryOpSpec = UnaryOpSpec {
    tool_name: "hfst-prune-alphabet",
    reject_ol: true,
};

// [spec:hfst:def:hfst-prune-alphabet.main-fn]
// [spec:hfst:sem:hfst-prune-alphabet.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstPruneAlphabet");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    let mut op = PruneAlphabetOp {
        force_pruning: options.force_pruning,
    };
    run_unary_tool(&common, &SPEC, &mut op)
}
