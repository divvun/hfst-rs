//! Faithful 1:1 port of tools/src/hfst-format.cc — the format-checking
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).
//!
//! This tool is unusual: it #includes globals-common.h and globals-unary.h
//! (so it is a unary tool), but it does the bulk of its work inside
//! parse_options (listing formats, testing a format, or opening the input
//! stream to report its type) and has no process_stream. main is therefore
//! very thin and simply prints the type returned by parse_options.

use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    hfst_set_program_name, hfst_strformat, print_more_info, print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use hfst_cli::inc::{CaseResult, handle_common_case, handle_unary_case};
use std::io::Write;

static mut LIST_FORMATS: bool = false;
// C used a NULL char* as "no format requested"; modelled as Option.
static mut FORMAT_TO_TEST: Option<String> = None;

fn format_to_test() -> Option<String> {
    unsafe { (*std::ptr::addr_of!(FORMAT_TO_TEST)).clone() }
}

// fprintf(stdout, ...): write to file descriptor 1.
fn fput_stdout(s: &str) {
    let _ = std::io::stdout().write_all(s.as_bytes());
    let _ = std::io::stdout().flush();
}

// fprintf(stderr, ...): write to file descriptor 2.
fn fput_stderr(s: &str) {
    let _ = std::io::stderr().write_all(s.as_bytes());
    let _ = std::io::stderr().flush();
}

// [spec:hfst:def:hfst-format.print-usage-fn]
// [spec:hfst:sem:hfst-format.print-usage-fn]
fn print_usage() {
    // c.f.
    // http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\ndetermine HFST transducer format\n\n",
        globals::program_name()
    );

    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Tool-specific options:\n  -l, --list-formats     List available transducer formats\n                         and print them to standard output\n"
    );
    let _ = write!(
        msg,
        "  -t, --test-format FMT  Whether the format FMT is available,\n                         exits with 0 if it is, else with 1\n"
    );
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(msg, "\n");
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-format.parse-options-fn]
// [spec:hfst:sem:hfst-format.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> ImplementationType {
    unsafe {
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            long_options.push(getopt::GetOpt {
                name: "input1",
                has_arg: 1,
                val: '1' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "input2",
                has_arg: 1,
                val: '2' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "list-formats",
                has_arg: 0,
                val: 'l' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "test-format",
                has_arg: 1,
                val: 't' as i32,
            });
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own cases, then the
            // terminal default arm (which here is a no-op, NOT the error arm).
            match handle_common_case(c, print_usage) {
                CaseResult::Return(code) => std::process::exit(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_unary_case(c) {
                CaseResult::Return(code) => std::process::exit(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            let ch = char::from_u32(c as u32);
            match ch {
                Some('1') => {
                    globals::set_input_filename(getopt::optarg());
                    continue;
                }
                Some('2') => {
                    globals::set_input_filename(getopt::optarg());
                    continue;
                }
                Some('l') => {
                    LIST_FORMATS = true;
                    continue;
                }
                Some('t') => {
                    FORMAT_TO_TEST = Some(getopt::optarg());
                    continue;
                }
                _ => {
                    // I suppose it's crucial for this tool to ignore other options.
                    // Unlike most tools, the default arm here is a genuine no-op
                    // (the C 'default: break;'), NOT the common error handler.
                    continue;
                }
            }
        }

        if let Some(fmt) = format_to_test() {
            if (fmt == "sfst"
                && HfstTransducer::is_implementation_type_available(ImplementationType::SFST_TYPE))
                || (fmt == "openfst-tropical"
                    && HfstTransducer::is_implementation_type_available(
                        ImplementationType::TROPICAL_OPENFST_TYPE,
                    ))
                || (fmt == "openfst-log"
                    && HfstTransducer::is_implementation_type_available(
                        ImplementationType::LOG_OPENFST_TYPE,
                    ))
                || (fmt == "foma"
                    && HfstTransducer::is_implementation_type_available(
                        ImplementationType::FOMA_TYPE,
                    ))
                || (fmt == "optimized-lookup-unweighted"
                    && HfstTransducer::is_implementation_type_available(
                        ImplementationType::HFST_OL_TYPE,
                    ))
                || (fmt == "optimized-lookup-weighted"
                    && HfstTransducer::is_implementation_type_available(
                        ImplementationType::HFST_OLW_TYPE,
                    ))
            {
                std::process::exit(0);
            }
            std::process::exit(1);
        }

        if LIST_FORMATS {
            fput_stdout(" Backend                         Names recognized\n\n");

            if HfstTransducer::is_implementation_type_available(ImplementationType::SFST_TYPE) {
                fput_stdout(" SFST                            sfst\n");
            }

            if HfstTransducer::is_implementation_type_available(
                ImplementationType::TROPICAL_OPENFST_TYPE,
            ) {
                fput_stdout(
                    " OpenFst (tropical weights)      openfst-tropical, openfst, ofst, ofst-tropical\n",
                );
            }

            if HfstTransducer::is_implementation_type_available(
                ImplementationType::LOG_OPENFST_TYPE,
            ) {
                fput_stdout(" OpenFst (logarithmic weights)   openfst-log, ofst-log\n");
            }

            if HfstTransducer::is_implementation_type_available(ImplementationType::FOMA_TYPE) {
                fput_stdout(" foma                            foma\n");
            }

            if HfstTransducer::is_implementation_type_available(ImplementationType::HFST_OL_TYPE) {
                fput_stdout(" Optimized lookup (weighted)     optimized-lookup-unweighted, olu\n");
            }

            if HfstTransducer::is_implementation_type_available(ImplementationType::HFST_OLW_TYPE) {
                fput_stdout(
                    " Optimized lookup (unweighted)   optimized-lookup-weighted, olw, optimized-lookup, ol\n",
                );
            }

            std::process::exit(0);
        }

        // (void)inputfilename; (void)inputNamed;

        // The C wraps the stream opening in try/catch on HfstException; on a
        // non-transducer stream it prints an error and exit(1). The Rust ctor
        // currently panics rather than throwing, so the catch arm is mirrored
        // by catching the panic.
        let optind = getopt::OPTIND;
        let remaining = args.len() - optind;
        let free_arg = if remaining == 1 {
            Some(args[optind].clone())
        } else {
            None
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || -> Result<ImplementationType, hfst::error::Error> {
                if globals::input_filename().is_empty() {
                    if remaining == 0 {
                        globals::set_input_filename("<stdin>");
                        let is = HfstInputStream::new()?;
                        return Ok(is.get_type());
                    } else if remaining == 1 {
                        globals::set_input_filename(
                            free_arg
                                .clone()
                                .expect("free_arg is Some when exactly one free argument remains"),
                        );
                    }
                }
                let is = HfstInputStream::new_filename(&globals::input_filename())?;
                Ok(is.get_type())
            },
        ));

        match result {
            Ok(Ok(t)) => t,
            Ok(Err(_)) | Err(_) => {
                fput_stderr("ERROR: The file/stream does not contain transducers.\n");
                std::process::exit(1);
            }
        }
    }
}

// [spec:hfst:def:hfst-format.main-fn]
// [spec:hfst:sem:hfst-format.main-fn]
fn main() {
    unsafe { real_main() };
}

unsafe fn real_main() {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstFormat");
        globals::VERBOSE = true;
        let type_ = parse_options(&mut args);
        verbose_printf(&format!(
            "Transducers in {} are of type {}\n",
            globals::input_filename(),
            hfst_strformat(type_)
        ));
    }
}
