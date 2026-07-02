//! Faithful 1:1 port of tools/src/hfst-prune-alphabet.cc — the transducer
//! alphabet-pruning command-line tool. Drives the hfst-cli foundation
//! (globals, getopt, commandline, program-options, tool-metadata, inc
//! fragments).

use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_from_env, hfst_set_program_name,
    is_input_stream_in_ol_format, print_more_info, print_report_bugs, verbose_print,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use hfst_cli::hfst_tool_metadata::{hfst_get_name, hfst_set_name_unary};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use std::io::Write;

// add tools-specific variables here
static mut FORCE_PRUNING: bool = false;

// [spec:hfst:def:hfst-prune-alphabet.print-usage-fn]
// [spec:hfst:sem:hfst-prune-alphabet.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nPrune the alphabet of a transducer\n\n",
        globals::program_name()
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Alphabet pruning options:\n  -f, --force            force pruning\n  -S, --safe             prune only if no unknown or identity symbols\n                         are used in the transducer (default)"
    );
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(msg, "\n");
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-prune-alphabet.parse-options-fn]
// [spec:hfst:sem:hfst-prune-alphabet.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_from_env(args);
        // use of this function requires options are settable on global scope
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
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own ('f'/'S'), then the
            // terminal error arm.
            match handle_common_case(c, print_usage) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_unary_case(c) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match c as u8 as char {
                'f' => {
                    FORCE_PRUNING = true;
                    continue;
                }
                'S' => {
                    FORCE_PRUNING = false;
                    continue;
                }
                _ => {}
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(args);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-prune-alphabet.process-stream-fn]
// [spec:hfst:sem:hfst-prune-alphabet.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream, outstream: &mut HfstOutputStream) -> i32 {
    unsafe {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let mut trans = match HfstTransducer::new_from_stream(instream) {
                Ok(v) => v,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            };
            let inputname = hfst_get_name(&trans, &globals::input_filename());
            if transducer_n == 1 {
                verbose_print(&format!("Pruning {}...\n", inputname));
            } else {
                verbose_print(&format!("Pruning {}... {}\n", inputname, transducer_n));
            }

            if let Err(e) = trans.prune_alphabet(FORCE_PRUNING) {
                error(1, 0, &format!("{e}"));
                return 1;
            }
            // C: hfst_set_name(trans, trans, "prune-alphabet"); the dest and src
            // are the same object, which Rust cannot alias mut+const, so the read
            // side is taken from a copy (name is unchanged by the copy).
            let src = trans.clone();
            hfst_set_name_unary(&mut trans, &src, "prune-alphabet");

            if let Err(e) = outstream.redirect(&mut trans) {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        }
        instream.close();
        outstream.close();
        0
    }
}

// [spec:hfst:def:hfst-prune-alphabet.main-fn]
// [spec:hfst:sem:hfst-prune-alphabet.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstPruneAlphabet");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let input_opened = globals::input_filename() != "<stdin>";
        let output_opened = globals::output_filename() != "<stdout>";
        verbose_print(&format!(
            "Reading from {}, writing to {}\n",
            globals::input_filename(),
            globals::output_filename()
        ));

        // here starts the buffer handling part
        let mut instream = match if input_opened {
            HfstInputStream::new_filename(&globals::input_filename())
        } else {
            HfstInputStream::new()
        } {
            Ok(v) => v,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        };
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)

        let ty = instream.get_type();
        let mut outstream = match if output_opened {
            HfstOutputStream::new_filename(&globals::output_filename(), ty, true)
        } else {
            HfstOutputStream::new(ty, true)
        } {
            Ok(v) => v,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        };

        if is_input_stream_in_ol_format(&instream, "hfst-prune-alphabet") {
            return 1;
        }

        process_stream(&mut instream, &mut outstream)
    }
}
