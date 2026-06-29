//! Faithful 1:1 port of tools/src/hfst-project.cc — the transducer projection
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).

use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_getenv, hfst_set_program_name,
    is_input_stream_in_ol_format, print_more_info, print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT, hfst_getopt_common_long,
    hfst_getopt_unary_long, print_common_program_options, print_common_unary_program_options,
    print_common_unary_program_parameter_instructions,
};
use hfst_cli::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary, hfst_set_name_unary};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use libc::{c_char, c_int};
use std::ffi::{CStr, CString};
use std::io::Write;

// add tools-specific variables here
static mut PROJECT_INPUT: bool = false;

unsafe fn cstr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

fn fput(f: &mut dyn Write, s: &str) {
    let _ = f.write_all(s.as_bytes());
}

// strncasecmp(optarg, prefix, 1) == 0 — case-insensitive comparison of the
// first byte only (the C calls always pass length 1).
unsafe fn first_char_matches(optarg: *const c_char, prefix: &str) -> bool {
    if optarg.is_null() {
        return false;
    }
    let first = unsafe { *optarg } as u8;
    if first == 0 {
        return false;
    }
    let want = prefix.as_bytes()[0];
    first.to_ascii_lowercase() == want.to_ascii_lowercase()
}

// [spec:hfst:def:hfst-project.print-usage-fn]
// [spec:hfst:sem:hfst-project.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = globals::message_writer();
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            &mut *msg,
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\nProject (extract a level) transducer\n\n",
                program_name
            ),
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        fput(
            &mut *msg,
            "Projection options:\n  -p, --project=LEVEL   project extracting tape LEVEL\n",
        );
        fput(&mut *msg, "\n");
        print_common_unary_program_parameter_instructions(&mut *msg);
        fput(
            &mut *msg,
            "LEVEL must be one of upper, input, first, analysis or lower, output, second, generation\n",
        );
        fput(&mut *msg, "\n");
        print_report_bugs();
        fput(&mut *msg, "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-project.parse-options-fn]
// [spec:hfst:sem:hfst-project.parse-options-fn]
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
                name: c"project".as_ptr(),
                has_arg: 1, // required_argument
                flag: std::ptr::null_mut(),
                val: 'p' as c_int,
            });
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}p:",
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
            // cases, then unary cases, then the tool's own 'p', then the
            // terminal error arm.
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
            if c == 'p' as c_int {
                let optarg = getopt::OPTARG;
                if first_char_matches(optarg, "upper")
                    || first_char_matches(optarg, "input")
                    || first_char_matches(optarg, "first")
                    || first_char_matches(optarg, "analysis")
                {
                    PROJECT_INPUT = true;
                } else if first_char_matches(optarg, "lower")
                    || first_char_matches(optarg, "output")
                    || first_char_matches(optarg, "second")
                    || first_char_matches(optarg, "generation")
                {
                    PROJECT_INPUT = false;
                } else {
                    error(
                        libc::EXIT_FAILURE,
                        0,
                        &format!(
                            "unknown project direction {}\nshould be one of upper, input, analysis, first, lower, output, second or generation\n",
                            cstr(optarg)
                        ),
                    );
                    return libc::EXIT_FAILURE;
                }
                continue;
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(argc, argv);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-project.process-stream-fn]
// [spec:hfst:sem:hfst-project.process-stream-fn]
unsafe fn process_stream(
    instream: &mut HfstInputStream,
    outstream: &mut HfstOutputStream,
) -> c_int {
    unsafe {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let mut trans = HfstTransducer::new_from_stream(instream);
            let inputname = hfst_get_name(&trans, &cstr(globals::INPUTFILENAME));
            if transducer_n == 1 {
                if PROJECT_INPUT {
                    verbose_printf(&format!("Projecting first {}...\n", inputname));
                } else {
                    verbose_printf(&format!("Projecting second {}...\n", inputname));
                }
            } else if PROJECT_INPUT {
                verbose_printf(&format!(
                    "Projecting first {}... {}\n",
                    inputname, transducer_n
                ));
            } else {
                verbose_printf(&format!(
                    "Projecting second {}... {}\n",
                    inputname, transducer_n
                ));
            }

            if PROJECT_INPUT {
                trans.input_project();
                // C: hfst_set_name(trans, trans, ...); the dest and src are the
                // same object, which Rust cannot alias mut+const, so the read
                // side is taken from a copy (name/formula unchanged by copy).
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "project-1st");
                hfst_set_formula_unary(&mut trans, &src, "\u{00b9}");
            } else {
                trans.output_project();
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "project-2nd");
                hfst_set_formula_unary(&mut trans, &src, "\u{00b2}");
            }
            outstream.redirect(&mut trans);
        }
        instream.close();
        outstream.close();
        libc::EXIT_SUCCESS
    }
}

// [spec:hfst:def:hfst-project.main-fn]
// [spec:hfst:sem:hfst-project.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> c_int {
    unsafe {
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

        hfst_set_program_name(&argv0, "0.1", "HfstProject");
        let retval = parse_options(argc, argv);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let input_opened = cstr(globals::INPUTFILENAME) != "<stdin>";
        let output_opened = cstr(globals::OUTFILENAME) != "<stdout>";
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

        if is_input_stream_in_ol_format(&instream, "hfst-project") {
            return libc::EXIT_FAILURE;
        }

        process_stream(&mut instream, &mut outstream)
    }
}
