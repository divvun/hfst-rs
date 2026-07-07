//! Faithful 1:1 port of tools/src/hfst-preprocess-for-optimized-lookup-format.cc
//! — the transducer preprocessing tool (the C++ source is the epsilon-removal /
//! rebuild tool). Drives the hfst-cli foundation (globals, getopt, commandline,
//! program-options, tool-metadata, inc fragments).

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, extend_options_from_env, hfst_set_program_name, verbose_print,
};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use crate::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary, hfst_set_name_unary};
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use std::io::Write;

// [spec:hfst:def:hfst-preprocess-for-optimized-lookup-format.print-usage-fn]
// [spec:hfst:sem:hfst-preprocess-for-optimized-lookup-format.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    // Usage line
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nRemove epsilons from a transducer\n\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(msg, "\n");
}

// [spec:hfst:def:hfst-preprocess-for-optimized-lookup-format.parse-options-fn]
// [spec:hfst:sem:hfst-preprocess-for-optimized-lookup-format.parse-options-fn]
fn parse_options(mut common: CommonOptions, args: &mut Vec<String>) -> Result<CommonOptions, i32> {
    let mut opt = Getopt::new();
    extend_options_from_env(args);
    // use of this function requires options are settable on global scope
    loop {
        let mut long_options: Vec<getopt::GetOpt> = Vec::new();
        long_options.extend(hfst_getopt_common_long());
        long_options.extend(hfst_getopt_unary_long());
        // add tool-specific options here
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: common
        // cases, then unary cases, then the terminal error arm.
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
        return Err(handle_error_case(&common, &opt, c));
    }

    check_common_params(&mut common);
    check_unary_params(&mut common, &opt, args);
    Ok(common)
}

// [spec:hfst:def:hfst-preprocess-for-optimized-lookup-format.process-stream-fn]
// [spec:hfst:sem:hfst-preprocess-for-optimized-lookup-format.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    instream: &mut HfstInputStream,
    outstream: &mut HfstOutputStream,
) -> i32 {
    let mut transducer_n: usize = 0;
    while instream.is_good() {
        transducer_n += 1;
        let any = match instream.read() {
            Ok(v) => v,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
        crate::for_algebra!(any, trans => {
        let mut trans = trans;
        let inputname = hfst_get_name(&trans, &common.input_filename);
        if transducer_n == 1 {
            verbose_print(common, &format!("Removing epsilons {}...\n", inputname));
        } else {
            verbose_print(common, &format!(
                "Removing epsilons {}...{}\n",
                inputname, transducer_n
            ));
        }
        if let Err(e) = trans.remove_epsilons() {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }
        if transducer_n == 1 {
            verbose_print(common, &format!("Rebuilding and fixing {}...\n", inputname));
        } else {
            verbose_print(common, &format!(
                "Rebuilding and fisting {}...{}\n",
                inputname, transducer_n
            ));
        }
        // C++: HfstBasicTransducer original(trans); — the
        // HfstBasicTransducer(const HfstTransducer&) conversion constructor.
        let original: HfstBasicTransducer =
            match HfstBasicTransducer::try_from_transducer(&trans) {
                Ok(v) => v,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
        let replication = original.renumber_states();
        trans = match HfstTransducer::new_from_basic(&replication) {
            Ok(v) => v,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        // C: hfst_set_name(trans, trans, "fu"); the dest and src are the same
        // object, which Rust cannot alias mut+const, so the read side is taken
        // from a copy (name/formula are unchanged by the copy).
        let src = trans.clone();
        hfst_set_name_unary(&mut trans, &src, "fu");
        hfst_set_formula_unary(&mut trans, &src, "FU");
        if let Err(e) = trans.remove_epsilons() {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }
        if let Err(e) = outstream.redirect(&mut trans) {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }
        }, else => {
            // The C++ ran its algebra on whatever type arrived and threw
            // FunctionNotImplemented (uncaught) on optimized-lookup input;
            // report the standard OL rejection instead.
            let _ = write!(
                std::io::stderr(),
                "Error: hfst-preprocess-for-optimized-lookup-format cannot process transducers that are in optimized lookup format.\n"
            );
            return 1;
        });
    }
    instream.close();
    outstream.close();
    0
}

// [spec:hfst:def:hfst-preprocess-for-optimized-lookup-format.main-fn]
// [spec:hfst:sem:hfst-preprocess-for-optimized-lookup-format.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstPreprocessForOptimizedLookupFormat");
    let common = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };
    // close buffers, we use streams
    let input_opened = common.input_filename != "<stdin>";
    let output_opened = common.output_filename != "<stdout>";
    verbose_print(
        &common,
        &format!(
            "Reading from {}, writing to {}\n",
            common.input_filename, common.output_filename
        ),
    );

    // here starts the buffer handling part
    let mut instream = match if input_opened {
        HfstInputStream::new_filename(&common.input_filename)
    } else {
        HfstInputStream::new()
    } {
        Ok(v) => v,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return 1;
        }
    };
    // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
    // currently panics on a bad file rather than throwing, so the catch arm
    // is not reproduced here.)

    let ty = instream.get_type();
    let mut outstream = match if output_opened {
        HfstOutputStream::new_filename(&common.output_filename, ty, true)
    } else {
        HfstOutputStream::new(ty, true)
    } {
        Ok(v) => v,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return 1;
        }
    };

    process_stream(&common, &mut instream, &mut outstream)
}
