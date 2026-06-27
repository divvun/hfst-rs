//! Faithful 1:1 port of tools/src/hfst-name.cc — the transducer naming
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).

use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, extend_options_getenv, hfst_set_program_name, hfst_strdup, hfst_strndup,
    hfst_strtoul, print_more_info, print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT, hfst_getopt_common_long,
    hfst_getopt_unary_long, print_common_program_options, print_common_unary_program_options,
    print_common_unary_program_parameter_instructions,
};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use libc::{c_char, c_int};
use std::ffi::{CStr, CString};

// add tools-specific variables here

static mut TRANSDUCER_NAME: *mut c_char = std::ptr::null_mut();
static mut NAME_OPTION_GIVEN: bool = false;
static mut PRINT_NAME: bool = false;
static mut TRUNCATE_LENGTH: u64 = 0;

unsafe fn cstr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

unsafe fn fput(f: *mut libc::FILE, s: &str) {
    let c = CString::new(s).unwrap_or_default();
    unsafe { libc::fputs(c.as_ptr(), f) };
}

// [spec:hfst:def:hfst-name.print-usage-fn]
// [spec:hfst:sem:hfst-name.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        // Usage line
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            globals::message_out(),
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\nName a transducer\n\n",
                program_name
            ),
        );
        fput(
            globals::message_out(),
            "Name options:\n  -n, --name=NAME      Name the transducer NAME\n  -p, --print-name     Only print the current name\n  -t, --truncate_length=LEN   Truncate name length to LEN\n",
        );
        print_common_program_options(globals::message_out());
        print_common_unary_program_options(globals::message_out());
        fput(globals::message_out(), "\n");
        print_common_unary_program_parameter_instructions(globals::message_out());
        fput(globals::message_out(), "\n");
        print_report_bugs();
        fput(globals::message_out(), "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-name.parse-options-fn]
// [spec:hfst:sem:hfst-name.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            long_options.push(getopt::Option {
                name: c"name".as_ptr(),
                has_arg: getopt::REQUIRED_ARGUMENT,
                flag: std::ptr::null_mut(),
                val: b'n' as c_int,
            });
            long_options.push(getopt::Option {
                name: c"print-name".as_ptr(),
                has_arg: getopt::NO_ARGUMENT,
                flag: std::ptr::null_mut(),
                val: b'p' as c_int,
            });
            long_options.push(getopt::Option {
                name: c"truncate_length".as_ptr(),
                has_arg: getopt::REQUIRED_ARGUMENT,
                flag: std::ptr::null_mut(),
                val: b't' as c_int,
            });
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}n:pt:",
                HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT
            ))
            .unwrap();
            let mut option_index: c_int = 0;
            // add tool-specific options here
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
            // cases, then unary cases, then the terminal error arm, then the
            // tool's own cases.
            match handle_common_case(c, || print_usage()) {
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
            let c_u8 = c as u8;
            match c_u8 {
                b'n' => {
                    TRANSDUCER_NAME = hfst_strdup(getopt::OPTARG);
                    NAME_OPTION_GIVEN = true;
                    continue;
                }
                b'p' => {
                    PRINT_NAME = true;
                    continue;
                }
                b't' => {
                    TRUNCATE_LENGTH = hfst_strtoul(&cstr(getopt::OPTARG), 10);
                    continue;
                }
                _ => {}
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(argc, argv);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-name.process-stream-fn]
// [spec:hfst:sem:hfst-name.process-stream-fn]
unsafe fn process_stream(
    instream: &mut HfstInputStream,
    outstream: &mut HfstOutputStream,
) -> c_int {
    unsafe {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;

            if transducer_n > 1 && PRINT_NAME {
                eprint!("---\n");
            }

            if transducer_n == 1 {
                verbose_printf(&format!("Naming {}...\n", cstr(globals::INPUTFILENAME)));
            } else {
                verbose_printf(&format!(
                    "Naming {}...{}\n",
                    cstr(globals::INPUTFILENAME),
                    transducer_n
                ));
            }

            let mut trans = HfstTransducer::new_from_stream(instream);
            if !PRINT_NAME {
                if TRUNCATE_LENGTH > 0 {
                    let truncated = hfst_strndup(TRANSDUCER_NAME, TRUNCATE_LENGTH as usize);
                    trans.set_name(&cstr(truncated));
                } else {
                    trans.set_name(&cstr(TRANSDUCER_NAME));
                }
                outstream.redirect(&mut trans);
            } else {
                eprint!("\"{}\"\n", trans.get_name());
            }
        }
        instream.close();
        outstream.close();
        libc::EXIT_SUCCESS
    }
}

// [spec:hfst:def:hfst-name.main-fn]
// [spec:hfst:sem:hfst-name.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> c_int {
    unsafe {
        // add tools-specific variable initialisation here (C: strdup(""))
        TRANSDUCER_NAME = hfst_strdup(c"".as_ptr());

        // Build a C-style argv (NULL-terminated) from the Rust args; getopt and
        // extend_options_getenv reorder/replace it in place.
        let c_args: Vec<CString> = std::env::args()
            .map(|a| CString::new(a).unwrap_or_default())
            .collect();
        let mut argv_vec: Vec<*mut c_char> =
            c_args.iter().map(|s| s.as_ptr() as *mut c_char).collect();
        argv_vec.push(std::ptr::null_mut());
        let argc: c_int = c_args.len() as c_int;
        let argv: *mut *mut c_char = argv_vec.as_mut_ptr();
        let argv0 = cstr(*argv);

        hfst_set_program_name(&argv0, "0.1", "HfstName");
        let retval = parse_options(argc, argv);
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
        let input_opened = !globals::INPUTFILE.is_null();
        let output_opened = !globals::OUTFILE.is_null();
        if input_opened {
            libc::fclose(globals::INPUTFILE);
        }
        if output_opened {
            libc::fclose(globals::OUTFILE);
        }
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            cstr(globals::INPUTFILENAME),
            cstr(globals::OUTFILENAME)
        ));

        // here starts the buffer handling part
        let mut instream = if input_opened {
            HfstInputStream::new_filename(&cstr(globals::INPUTFILENAME))
        } else {
            HfstInputStream::new()
        };
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)

        let type_ = instream.get_type();
        let mut outstream = if output_opened {
            HfstOutputStream::new_filename(&cstr(globals::OUTFILENAME), type_, true)
        } else {
            HfstOutputStream::new(type_, true)
        };

        process_stream(&mut instream, &mut outstream)
    }
}
