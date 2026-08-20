//! Port of tools/src/hfst-eliminate-flags.cc — the transducer flag elimination
//! command-line tool. Drives the hfst-cli foundation (getopt, commandline,
//! program-options, tool-metadata, inc fragments).
//!
//! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
//! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
//! `parse_options` and threaded into the processing functions. There are no
//! `static mut` globals and no `unsafe`.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{error, extend_options_from_env, hfst_set_program_name};
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

/// hfst-eliminate-flags's own options (the former tool-specific `static mut`s).
#[derive(Default)]
struct Options {
    /// '-F, --flag=FLAG': only eliminate flag FLAG (else all flags).
    flag: Option<String>,
}

// [spec:hfst:def:hfst-eliminate-flags.print-usage-fn]
// [spec:hfst:sem:hfst-eliminate-flags.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    // Usage line
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nEliminate flags from a transducer\n\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = writeln!(msg, "Command-specific options:");
    let _ = write!(msg, "  -F, --flag=FLAG        Only eliminate flag FLAG\n\n");
    let _ = writeln!(msg);
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = writeln!(msg);
}

// [spec:hfst:def:hfst-eliminate-flags.parse-options-fn]
// [spec:hfst:sem:hfst-eliminate-flags.parse-options-fn]
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
        long_options.push(getopt::GetOpt {
            name: "flag",
            has_arg: getopt::REQUIRED_ARGUMENT,
            val: 'F' as i32,
        });
        // add tool-specific options here
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: common
        // cases, then unary cases, then the tool's own ('F'), then the
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
        if c == 'F' as i32 {
            options.flag = Some(opt.optarg());
            continue;
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    check_common_params(&mut common);
    check_unary_params(&mut common, &opt, args);
    Ok((common, options))
}

// [spec:hfst:def:hfst-eliminate-flags.process-stream-fn]
// [spec:hfst:sem:hfst-eliminate-flags.process-stream-fn]
//
// The stream loop lives in the shared unary driver; this op is the
// per-transducer body it dispatches into. The verbose verb names what is being
// eliminated ("flags" or "flag FLAG"), which the C computes once before the
// loop; here it is the op's own precomputed field.
struct EliminateFlagsOp {
    /// '-F, --flag=FLAG', if given.
    flag: Option<String>,
    /// The verbose line's object: "flags", or "flag FLAG".
    flags: String,
}

impl UnaryToolOp for EliminateFlagsOp {
    fn verbose_begin(&self, inputname: &str) -> String {
        // The C additionally falls back to the input filename on an empty
        // transducer name, which hfst_get_name has already done: it returns the
        // filename whenever the name is empty, so the guard could only ever
        // re-substitute the same empty filename.
        format!("Eliminating {} {}", self.flags, inputname)
    }

    fn name_op(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed("eliminate-flags"))
    }

    fn formula(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed("Id"))
    }

    fn apply<B: AlgebraBackend>(
        &mut self,
        common: &CommonOptions,
        t: &mut HfstTransducer<B>,
    ) -> hfst::error::Result<()> {
        match &self.flag {
            None => t.eliminate_flags().map(|_| ()),
            Some(f) => {
                if t.eliminate_flag(f).is_err() {
                    // The single-flag failure substitutes the tool's own text
                    // for the error value's, so it is reported here rather than
                    // through the driver's '{e}' path. `error` with a non-zero
                    // status exits the process, so the Err below is never
                    // observed; it stands for the C's `return 1`.
                    error(
                        common,
                        1,
                        0,
                        &format!(
                            "flag feature {} does not occur in the transducer\nonly the flag feature must be given, no value or operator",
                            f
                        ),
                    );
                    return Err(hfst::error::Error::new(hfst::error::ErrorKind::Fatal));
                }
                Ok(())
            }
        }
    }
}

const SPEC: UnaryOpSpec = UnaryOpSpec {
    tool_name: "hfst-eliminate-flags",
    reject_ol: true,
};

// [spec:hfst:def:hfst-eliminate-flags.main-fn]
// [spec:hfst:sem:hfst-eliminate-flags.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstEliminateFlags");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    let flags = match &options.flag {
        None => String::from("flags"),
        Some(f) => format!("flag {}", f),
    };
    let mut op = EliminateFlagsOp {
        flag: options.flag,
        flags,
    };
    run_unary_tool(&common, &SPEC, &mut op)
}
