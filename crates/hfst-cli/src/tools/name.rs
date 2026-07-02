//! Faithful 1:1 port of tools/src/hfst-name.cc — the transducer naming
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).

use crate::globals;
use crate::hfst_commandline::{
    EXIT_CONTINUE, extend_options_from_env, hfst_set_program_name, parse_u64, verbose_print,
};
use crate::hfst_getopt as getopt;
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use std::io::Write;

// add tools-specific variables here

static mut TRANSDUCER_NAME: String = String::new();
static mut NAME_OPTION_GIVEN: bool = false;
static mut PRINT_NAME: bool = false;
static mut TRUNCATE_LENGTH: u64 = 0;

// [spec:hfst:def:hfst-name.print-usage-fn]
// [spec:hfst:sem:hfst-name.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    // Usage line
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nName a transducer\n\n",
        globals::program_name()
    );
    let _ = write!(
        msg,
        "Name options:\n  -n, --name=NAME      Name the transducer NAME\n  -p, --print-name     Only print the current name\n  -t, --truncate_length=LEN   Truncate name length to LEN\n"
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(msg, "\n");
}

// [spec:hfst:def:hfst-name.parse-options-fn]
// [spec:hfst:sem:hfst-name.parse-options-fn]
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
                name: "name",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: b'n' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "print-name",
                has_arg: getopt::NO_ARGUMENT,
                val: b'p' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "truncate_length",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: b't' as i32,
            });
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the terminal error arm, then the
            // tool's own cases.
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
            // tool-specific cases come before the error arm in the C switch
            // ordering (getopt-cases-error.h precedes them textually but its
            // arms only fire on '?'/ ':' / default, so the named cases below
            // are reached for 'n'/'p'/'t').
            let byte = c as u8;
            match byte {
                b'n' => {
                    *std::ptr::addr_of_mut!(TRANSDUCER_NAME) = getopt::optarg();
                    NAME_OPTION_GIVEN = true;
                    continue;
                }
                b'p' => {
                    PRINT_NAME = true;
                    continue;
                }
                b't' => {
                    TRUNCATE_LENGTH = parse_u64(&getopt::optarg(), 10);
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

// [spec:hfst:def:hfst-name.process-stream-fn]
// [spec:hfst:sem:hfst-name.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream, outstream: &mut HfstOutputStream) -> i32 {
    unsafe {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;

            if transducer_n > 1 && PRINT_NAME {
                eprint!("---\n");
            }

            if transducer_n == 1 {
                verbose_print(&format!("Naming {}...\n", globals::input_filename()));
            } else {
                verbose_print(&format!(
                    "Naming {}...{}\n",
                    globals::input_filename(),
                    transducer_n
                ));
            }

            let mut trans = match HfstTransducer::new_from_stream(instream) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("hfst-name: {e}");
                    return 1;
                }
            };
            if !PRINT_NAME {
                let name = (*std::ptr::addr_of!(TRANSDUCER_NAME)).clone();
                if TRUNCATE_LENGTH > 0 {
                    // C: hfst_strndup copies at most TRUNCATE_LENGTH bytes.
                    let n = (TRUNCATE_LENGTH as usize).min(name.len());
                    let truncated = String::from_utf8_lossy(&name.as_bytes()[..n]).into_owned();
                    trans.set_name(&truncated);
                } else {
                    trans.set_name(&name);
                }
                if let Err(e) = outstream.redirect(&mut trans) {
                    eprintln!("hfst-name: {e}");
                    return 1;
                }
            } else {
                eprint!("\"{}\"\n", trans.get_name());
            }
        }
        instream.close();
        outstream.close();
        0
    }
}

// [spec:hfst:def:hfst-name.main-fn]
// [spec:hfst:sem:hfst-name.main-fn]
pub fn run(args: Vec<String>) -> i32 {
    unsafe { real_main(args) }
}

unsafe fn real_main(mut args: Vec<String>) -> i32 {
    unsafe {
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstName");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }

        if !PRINT_NAME && !NAME_OPTION_GIVEN {
            eprint!("Error: hfst-name: use either option --print-name  or --name\n");
            return 1;
        }
        if PRINT_NAME && NAME_OPTION_GIVEN {
            eprint!("Warning: option --print-name overrides option --name\n");
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
                eprintln!("hfst-name: {e}");
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
                eprintln!("hfst-name: {e}");
                return 1;
            }
        };

        process_stream(&mut instream, &mut outstream)
    }
}
