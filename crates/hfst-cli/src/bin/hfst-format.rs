//! Faithful 1:1 port of tools/src/hfst-format.cc — the format-checking
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).
//!
//! This tool is unusual: it #includes globals-common.h and globals-unary.h
//! (so it is a unary tool), but it does the bulk of its work inside
//! parse_options (listing formats, testing a format, or opening the input
//! stream to report its type) and has no process_stream. main is therefore
//! very thin and simply prints the type returned by parse_options.

use core::ffi::{c_char, c_int};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    hfst_set_program_name, hfst_strformat, print_more_info, print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT, hfst_getopt_common_long,
    hfst_getopt_unary_long, print_common_program_options, print_common_unary_program_options,
    print_common_unary_program_parameter_instructions,
};
use hfst_cli::inc::{CaseResult, handle_common_case, handle_unary_case};
use std::ffi::{CStr, CString};

static mut LIST_FORMATS: bool = false;
static mut FORMAT_TO_TEST: *mut c_char = std::ptr::null_mut();

unsafe fn cstr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

fn fput(f: &mut dyn std::io::Write, s: &str) {
    let _ = f.write_all(s.as_bytes());
}

// fprintf(stdout, ...): write to file descriptor 1.
fn fput_stdout(s: &str) {
    use std::io::Write;
    let _ = std::io::stdout().write_all(s.as_bytes());
    let _ = std::io::stdout().flush();
}

// fprintf(stderr, ...): write to file descriptor 2.
fn fput_stderr(s: &str) {
    use std::io::Write;
    let _ = std::io::stderr().write_all(s.as_bytes());
    let _ = std::io::stderr().flush();
}

// [spec:hfst:def:hfst-format.print-usage-fn]
// [spec:hfst:sem:hfst-format.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f.
        // http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = globals::message_writer();
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            &mut *msg,
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\ndetermine HFST transducer format\n\n",
                program_name
            ),
        );

        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        fput(
            &mut *msg,
            "Tool-specific options:\n  -l, --list-formats     List available transducer formats\n                         and print them to standard output\n",
        );
        fput(
            &mut *msg,
            "  -t, --test-format FMT  Whether the format FMT is available,\n                         exits with 0 if it is, else with 1\n",
        );
        fput(&mut *msg, "\n");
        print_common_unary_program_parameter_instructions(&mut *msg);
        fput(&mut *msg, "\n");
        print_report_bugs();
        fput(&mut *msg, "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-format.parse-options-fn]
// [spec:hfst:sem:hfst-format.parse-options-fn]
unsafe fn parse_options(argc: c_int, argv: *mut *mut c_char) -> ImplementationType {
    unsafe {
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            long_options.push(getopt::Option {
                name: b"input1\0".as_ptr() as *const c_char,
                has_arg: 1,
                flag: std::ptr::null_mut(),
                val: '1' as c_int,
            });
            long_options.push(getopt::Option {
                name: b"input2\0".as_ptr() as *const c_char,
                has_arg: 1,
                flag: std::ptr::null_mut(),
                val: '2' as c_int,
            });
            long_options.push(getopt::Option {
                name: b"list-formats\0".as_ptr() as *const c_char,
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 'l' as c_int,
            });
            long_options.push(getopt::Option {
                name: b"test-format\0".as_ptr() as *const c_char,
                has_arg: 1,
                flag: std::ptr::null_mut(),
                val: 't' as c_int,
            });
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}1:2:lt:",
                HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT
            ))
            .unwrap();
            let mut option_index: c_int = 0;
            let c = getopt::getopt_long(
                argc,
                argv,
                short.as_ptr(),
                long_options.as_ptr(),
                &mut option_index,
            );
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own cases, then the
            // terminal default arm (which here is a no-op, NOT the error arm).
            match handle_common_case(c, || print_usage()) {
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
                    globals::INPUTFILENAME =
                        hfst_cli::hfst_commandline::hfst_strdup(getopt::OPTARG);
                    continue;
                }
                Some('2') => {
                    globals::INPUTFILENAME =
                        hfst_cli::hfst_commandline::hfst_strdup(getopt::OPTARG);
                    continue;
                }
                Some('l') => {
                    LIST_FORMATS = true;
                    continue;
                }
                Some('t') => {
                    FORMAT_TO_TEST = hfst_cli::hfst_commandline::hfst_strdup(getopt::OPTARG);
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

        if !FORMAT_TO_TEST.is_null() {
            let fmt = cstr(FORMAT_TO_TEST);
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
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if globals::INPUTFILENAME.is_null() {
                if (argc - getopt::OPTIND) == 0 {
                    globals::INPUTFILENAME = hfst_cli::hfst_commandline::hfst_strdup(
                        b"<stdin>\0".as_ptr() as *const c_char,
                    );
                    let is = HfstInputStream::new();
                    return is.get_type();
                } else if (argc - getopt::OPTIND) == 1 {
                    globals::INPUTFILENAME = *argv.offset(getopt::OPTIND as isize);
                }
            }
            let is = HfstInputStream::new_filename(&cstr(globals::INPUTFILENAME));
            is.get_type()
        }));

        match result {
            Ok(t) => t,
            Err(_) => {
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
        // Build a C-style argv (NULL-terminated) from the Rust args; getopt
        // reorders/replaces it in place.
        let c_args: Vec<CString> = std::env::args()
            .map(|a| CString::new(a).unwrap_or_default())
            .collect();
        let mut argv_vec: Vec<*mut c_char> =
            c_args.iter().map(|s| s.as_ptr() as *mut c_char).collect();
        argv_vec.push(std::ptr::null_mut());
        let argc: c_int = c_args.len() as c_int;
        let argv: *mut *mut c_char = argv_vec.as_mut_ptr();
        let argv0 = cstr(*argv);

        hfst_set_program_name(&argv0, "0.1", "HfstFormat");
        globals::VERBOSE = true;
        let type_ = parse_options(argc, argv);
        verbose_printf(&format!(
            "Transducers in {} are of type {}\n",
            cstr(globals::INPUTFILENAME),
            hfst_strformat(type_)
        ));
    }
}
