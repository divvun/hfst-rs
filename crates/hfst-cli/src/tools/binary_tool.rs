//! Faithful 1:1 port of tools/src/hfst-binary-tool.cc — the GENERIC BINARY
//! TOOL TEMPLATE command-line tool. Drives the hfst-cli foundation (globals,
//! getopt, commandline, program-options, tool-metadata, inc fragments).

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    extend_options_from_env, hfst_set_program_name, verbose_print, warning,
};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_binary_long, hfst_getopt_common_long, print_common_binary_program_options,
    print_common_binary_program_parameter_instructions, print_common_program_options,
};
use crate::inc::{
    CaseResult, check_binary_params, check_common_params, handle_binary_case, handle_common_case,
    handle_error_case,
};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use std::io::Write;

/// hfst-binary-tool's own options (the former tool-specific `static mut`s).
/// The skeleton tool has none.
struct Options;

// [spec:hfst:def:hfst-binary-tool.print-usage-fn]
// [spec:hfst:sem:hfst-binary-tool.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    let mut msg = common.message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nDo things with two transducers\n\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    print_common_binary_program_options(&mut *msg);
    let _ = write!(msg, "\n");
    print_common_binary_program_parameter_instructions(&mut *msg);
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "\nExamples:\n  {} -o catdog.hfst cat.hfst dog.hfst  does things\n\n",
        common.program_name
    );
}

// [spec:hfst:def:hfst-binary-tool.parse-options-fn]
// [spec:hfst:sem:hfst-binary-tool.parse-options-fn]
//
// Parse argv into the shared + tool options; `Err(code)` is an exit code the
// caller should return (the former EXIT_CONTINUE sentinel is now `Ok`).
fn parse_options(
    mut common: CommonOptions,
    args: &mut Vec<String>,
) -> Result<(CommonOptions, Options), i32> {
    let options = Options;
    let mut opt = Getopt::new();
    extend_options_from_env(args);
    // use of this function requires options are settable on global scope
    loop {
        let mut long_options: Vec<getopt::GetOpt> = Vec::new();
        long_options.extend(hfst_getopt_common_long());
        long_options.extend(hfst_getopt_binary_long());
        // add tool-specific options here
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: common
        // cases, then binary cases, then the tool's own (none here), then
        // the terminal error arm.
        match handle_common_case(&mut common, &opt, c, print_usage) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        match handle_binary_case(&mut common, &opt, c) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    check_common_params(&mut common);
    check_binary_params(&mut common, &opt, args);
    Ok((common, options))
}

// [spec:hfst:def:hfst-binary-tool.binaryoperate-streams-fn]
// [spec:hfst:sem:hfst-binary-tool.binaryoperate-streams-fn]
fn binaryoperate_streams(
    common: &CommonOptions,
    firststream: &mut HfstInputStream<'_>,
    secondstream: &mut HfstInputStream<'_>,
    outstream: &mut HfstOutputStream,
) -> i32 {
    // (the C opens each stream here; the Rust streams are opened by their
    // constructors, so the explicit open() calls are not reproduced.)
    // should be is_good?
    let mut both_inputs = firststream.is_good() && secondstream.is_good();
    if firststream.get_type() != secondstream.get_type() {
        warning(
            common,
            0,
            0,
            &format!(
                "Tranducer type mismatch in {} and {}; using former type as output\n",
                common.first_filename, common.second_filename
            ),
        );
    }
    let mut transducer_n: usize = 0;
    while both_inputs {
        transducer_n += 1;
        if transducer_n == 1 {
            verbose_print(
                common,
                &format!(
                    "Doing things with {} and {}...\n",
                    common.first_filename, common.second_filename
                ),
            );
        } else {
            verbose_print(
                common,
                &format!(
                    "Doing things with {} and {}... {}\n",
                    common.first_filename, common.second_filename, transducer_n
                ),
            );
        }
        let first = match firststream.read() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("hfst-binary-tool: {e}");
                return 1;
            }
        };
        let second = match secondstream.read() {
            Ok(t) => t,
            Err(e) => {
                eprintln!("hfst-binary-tool: {e}");
                return 1;
            }
        };
        // one dispatch per pair ([dec:hfst:monomorphic-backends]); the
        // C++ concatenate threw TransducerTypeMismatch for mixed operands
        // at runtime, which is now the boundary's mismatch arm.
        use hfst::hfst_transducer::AnyTransducer;
        let code = match (first, second) {
            (AnyTransducer::Tropical(f), AnyTransducer::Tropical(s)) => {
                concatenate_pair(f, s, outstream)
            }
            (AnyTransducer::Log(f), AnyTransducer::Log(s)) => concatenate_pair(f, s, outstream),
            _ => {
                eprintln!("hfst-binary-tool: {}", hfst::err!(TransducerTypeMismatch));
                return 1;
            }
        };
        if code != 0 {
            return code;
        }
        both_inputs = firststream.is_good() && secondstream.is_good();
    }

    if firststream.is_good() {
        warning(
            common,
            0,
            0,
            &format!(
                "Warning: {} contains more transducers than {}; residue skipped\n",
                common.first_filename, common.second_filename
            ),
        );
    } else if secondstream.is_good() {
        warning(
            common,
            0,
            0,
            &format!(
                "Warning: {} contains fewer transducers than {}; residue skipped\n",
                common.first_filename, common.second_filename
            ),
        );
    }
    firststream.close();
    secondstream.close();
    outstream.close();
    0
}

// The monomorphic pair body of the skeleton tool.
fn concatenate_pair<B: hfst::backend::AlgebraBackend>(
    mut first: HfstTransducer<B>,
    second: HfstTransducer<B>,
    outstream: &mut HfstOutputStream,
) -> i32 {
    if let Err(e) = first.concatenate(&second, true) {
        eprintln!("hfst-binary-tool: {e}");
        return 1;
    }
    if let Err(e) = outstream.redirect(&mut first) {
        eprintln!("hfst-binary-tool: {e}");
        return 1;
    }
    0
}

// [spec:hfst:def:hfst-binary-tool.main-fn]
// [spec:hfst:sem:hfst-binary-tool.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstGenericBinaryTool");
    let (common, _options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    // close buffers, we use streams
    let first_opened = common.first_filename != "<stdin>";
    let second_opened = common.second_filename != "<stdin>";
    let output_opened = common.output_filename != "<stdout>";
    verbose_print(
        &common,
        &format!(
            "Reading from {} and {}, writing to {}\n",
            common.first_filename, common.second_filename, common.output_filename
        ),
    );
    // here starts the buffer handling part
    // (the C wraps each ctor in try/catch on HfstException; the Rust ctor
    // currently panics on a bad file rather than throwing, so the catch
    // arms are not reproduced here.)
    let firststream_res = if first_opened {
        HfstInputStream::new_filename(&common.first_filename)
    } else {
        HfstInputStream::new()
    };
    let mut firststream = match firststream_res {
        Ok(s) => s,
        Err(e) => {
            eprintln!("hfst-binary-tool: {e}");
            return 1;
        }
    };
    let secondstream_res = if second_opened {
        HfstInputStream::new_filename(&common.second_filename)
    } else {
        HfstInputStream::new()
    };
    let mut secondstream = match secondstream_res {
        Ok(s) => s,
        Err(e) => {
            eprintln!("hfst-binary-tool: {e}");
            return 1;
        }
    };
    let ty = firststream.get_type();
    let outstream_res = if output_opened {
        HfstOutputStream::new_filename(&common.output_filename, ty, true)
    } else {
        HfstOutputStream::new(ty, true)
    };
    let mut outstream = match outstream_res {
        Ok(s) => s,
        Err(e) => {
            eprintln!("hfst-binary-tool: {e}");
            return 1;
        }
    };

    // (the C main calls concatenate_streams; the defined function is
    // binaryoperate_streams — the same routine — which is invoked here.)
    binaryoperate_streams(&common, &mut firststream, &mut secondstream, &mut outstream)
}
