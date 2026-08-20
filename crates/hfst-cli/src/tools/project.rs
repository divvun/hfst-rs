//! Faithful 1:1 port of tools/src/hfst-project.cc — the transducer projection
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).

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

/// hfst-project's own options (the former tool-specific `static mut`s).
#[derive(Default)]
struct Options {
    /// '-p, --project=LEVEL': project extracting the input (first) tape when
    /// true, the output (second) tape when false.
    project_input: bool,
}

// strncasecmp(optarg, prefix, 1) == 0 — case-insensitive comparison of the
// first byte only (the C calls always pass length 1).
fn first_char_matches(optarg: &Option<String>, prefix: &str) -> bool {
    match optarg.as_ref().and_then(|s| s.bytes().next()) {
        Some(first) => {
            let want = prefix.as_bytes()[0];
            first.eq_ignore_ascii_case(&want)
        }
        None => false,
    }
}

// [spec:hfst:def:hfst-project.print-usage-fn]
// [spec:hfst:sem:hfst-project.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nProject (extract a level) transducer\n\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Projection options:\n  -p, --project=LEVEL   project extracting tape LEVEL\n"
    );
    let _ = writeln!(msg);
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = writeln!(
        msg,
        "LEVEL must be one of upper, input, first, analysis or lower, output, second, generation"
    );
    let _ = writeln!(msg);
}

// [spec:hfst:def:hfst-project.parse-options-fn]
// [spec:hfst:sem:hfst-project.parse-options-fn]
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
            name: "project",
            has_arg: getopt::REQUIRED_ARGUMENT,
            val: 'p' as i32,
        });
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: common
        // cases, then unary cases, then the tool's own 'p', then the
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
        if c == 'p' as i32 {
            let optarg = opt.optarg_opt();
            if first_char_matches(&optarg, "upper")
                || first_char_matches(&optarg, "input")
                || first_char_matches(&optarg, "first")
                || first_char_matches(&optarg, "analysis")
            {
                options.project_input = true;
            } else if first_char_matches(&optarg, "lower")
                || first_char_matches(&optarg, "output")
                || first_char_matches(&optarg, "second")
                || first_char_matches(&optarg, "generation")
            {
                options.project_input = false;
            } else {
                error(
                    &common,
                    1,
                    0,
                    &format!(
                        "unknown project direction {}\nshould be one of upper, input, analysis, first, lower, output, second or generation\n",
                        opt.optarg()
                    ),
                );
                return Err(1);
            }
            continue;
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    check_common_params(&mut common);
    check_unary_params(&mut common, &opt, args);
    Ok((common, options))
}

// [spec:hfst:def:hfst-project.process-stream-fn]
// [spec:hfst:sem:hfst-project.process-stream-fn]
//
// The stream loop lives in the shared unary driver; this op is the
// per-transducer body it dispatches into. Both the verbose verb and the
// name/formula stamp follow the projected tape.
struct ProjectOp {
    project_input: bool,
}

impl UnaryToolOp for ProjectOp {
    fn verbose_begin(&self, inputname: &str) -> String {
        if self.project_input {
            format!("Projecting first {}", inputname)
        } else {
            format!("Projecting second {}", inputname)
        }
    }

    fn verbose_sep(&self) -> &'static str {
        " "
    }

    fn name_op(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed(if self.project_input {
            "project-1st"
        } else {
            "project-2nd"
        }))
    }

    fn formula(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed(if self.project_input {
            "\u{00b9}"
        } else {
            "\u{00b2}"
        }))
    }

    fn apply<B: AlgebraBackend>(
        &mut self,
        _common: &CommonOptions,
        t: &mut HfstTransducer<B>,
    ) -> hfst::error::Result<()> {
        if self.project_input {
            t.input_project().map(|_| ())
        } else {
            t.output_project().map(|_| ())
        }
    }
}

const SPEC: UnaryOpSpec = UnaryOpSpec {
    tool_name: "hfst-project",
    reject_ol: true,
};

// [spec:hfst:def:hfst-project.main-fn]
// [spec:hfst:sem:hfst-project.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstProject");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    let mut op = ProjectOp {
        project_input: options.project_input,
    };
    run_unary_tool(&common, &SPEC, &mut op)
}
