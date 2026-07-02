//! Faithful 1:1 port of tools/src/hfst-binary-tool.cc — the GENERIC BINARY
//! TOOL TEMPLATE command-line tool. Drives the hfst-cli foundation (globals,
//! getopt, commandline, program-options, tool-metadata, inc fragments).

use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, extend_options_getenv, hfst_set_program_name, print_more_info,
    print_report_bugs, verbose_print, warning,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    hfst_getopt_binary_long, hfst_getopt_common_long, print_common_binary_program_options,
    print_common_binary_program_parameter_instructions, print_common_program_options,
};
use hfst_cli::inc::{
    CaseResult, check_binary_params, check_common_params, handle_binary_case, handle_common_case,
    handle_error_case,
};
use std::io::Write;

// [spec:hfst:def:hfst-binary-tool.print-usage-fn]
// [spec:hfst:sem:hfst-binary-tool.print-usage-fn]
fn print_usage() {
    let mut msg = globals::message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let program_name = globals::program_name();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nDo things with two transducers\n\n",
        program_name
    );
    print_common_program_options(&mut *msg);
    print_common_binary_program_options(&mut *msg);
    let _ = write!(msg, "\n");
    print_common_binary_program_parameter_instructions(&mut *msg);
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "\nExamples:\n  {} -o catdog.hfst cat.hfst dog.hfst  does things\n\n",
        program_name
    );
    print_report_bugs();
    print_more_info();
}

// [spec:hfst:def:hfst-binary-tool.parse-options-fn]
// [spec:hfst:sem:hfst-binary-tool.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_getenv(args);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_binary_long());
            // add tool-specific options here
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then binary cases, then the tool's own (none here), then
            // the terminal error arm.
            match handle_common_case(c, print_usage) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_binary_case(c) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_binary_params(args);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-binary-tool.binaryoperate-streams-fn]
// [spec:hfst:sem:hfst-binary-tool.binaryoperate-streams-fn]
unsafe fn binaryoperate_streams(
    firststream: &mut HfstInputStream,
    secondstream: &mut HfstInputStream,
    outstream: &mut HfstOutputStream,
) -> i32 {
    unsafe {
        // (the C opens each stream here; the Rust streams are opened by their
        // constructors, so the explicit open() calls are not reproduced.)
        // should be is_good?
        let mut both_inputs = firststream.is_good() && secondstream.is_good();
        if firststream.get_type() != secondstream.get_type() {
            warning(
                0,
                0,
                &format!(
                    "Tranducer type mismatch in {} and {}; using former type as output\n",
                    globals::first_filename(),
                    globals::second_filename()
                ),
            );
        }
        let mut transducer_n: usize = 0;
        while both_inputs {
            transducer_n += 1;
            if transducer_n == 1 {
                verbose_print(&format!(
                    "Doing things with {} and {}...\n",
                    globals::first_filename(),
                    globals::second_filename()
                ));
            } else {
                verbose_print(&format!(
                    "Doing things with {} and {}... {}\n",
                    globals::first_filename(),
                    globals::second_filename(),
                    transducer_n
                ));
            }
            let mut first = match HfstTransducer::new_from_stream(firststream) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("hfst-binary-tool: {e}");
                    return 1;
                }
            };
            let second = match HfstTransducer::new_from_stream(secondstream) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("hfst-binary-tool: {e}");
                    return 1;
                }
            };
            if let Err(e) = first.concatenate(&second, true) {
                eprintln!("hfst-binary-tool: {e}");
                return 1;
            }
            if let Err(e) = outstream.redirect(&mut first) {
                eprintln!("hfst-binary-tool: {e}");
                return 1;
            }
            both_inputs = firststream.is_good() && secondstream.is_good();
        }

        if firststream.is_good() {
            warning(
                0,
                0,
                &format!(
                    "Warning: {} contains more transducers than {}; residue skipped\n",
                    globals::first_filename(),
                    globals::second_filename()
                ),
            );
        } else if secondstream.is_good() {
            warning(
                0,
                0,
                &format!(
                    "Warning: {} contains fewer transducers than {}; residue skipped\n",
                    globals::first_filename(),
                    globals::second_filename()
                ),
            );
        }
        firststream.close();
        secondstream.close();
        outstream.close();
        0
    }
}

// [spec:hfst:def:hfst-binary-tool.main-fn]
// [spec:hfst:sem:hfst-binary-tool.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstGenericBinaryTool");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let first_opened = globals::first_filename() != "<stdin>";
        let second_opened = globals::second_filename() != "<stdin>";
        let output_opened = globals::output_filename() != "<stdout>";
        verbose_print(&format!(
            "Reading from {} and {}, writing to {}\n",
            globals::first_filename(),
            globals::second_filename(),
            globals::output_filename()
        ));
        // here starts the buffer handling part
        // (the C wraps each ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch
        // arms are not reproduced here.)
        let firststream_res = if first_opened {
            HfstInputStream::new_filename(&globals::first_filename())
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
            HfstInputStream::new_filename(&globals::second_filename())
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
            HfstOutputStream::new_filename(&globals::output_filename(), ty, true)
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
        binaryoperate_streams(&mut firststream, &mut secondstream, &mut outstream)
    }
}
