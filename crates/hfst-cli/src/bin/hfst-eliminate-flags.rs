//! Faithful 1:1 port of tools/src/hfst-eliminate-flags.cc — the transducer
//! flag elimination command-line tool. Drives the hfst-cli foundation
//! (globals, getopt, commandline, program-options, tool-metadata, inc
//! fragments).

use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::{self, HfstTransducer};
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

// add tools-specific variables here
static mut FLAG: Option<String> = None;

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

// [spec:hfst:def:hfst-eliminate-flags.print-usage-fn]
// [spec:hfst:sem:hfst-eliminate-flags.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        // Usage line
        let mut msg = globals::message_writer();
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            &mut *msg,
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\nEliminate flags from a transducer\n\n",
                program_name
            ),
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        fput(&mut *msg, "Command-specific options:\n");
        fput(
            &mut *msg,
            "  -F, --flag=FLAG        Only eliminate flag FLAG\n\n",
        );
        fput(&mut *msg, "\n");
        print_common_unary_program_parameter_instructions(&mut *msg);
        fput(&mut *msg, "\n");
        print_report_bugs();
        fput(&mut *msg, "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-eliminate-flags.parse-options-fn]
// [spec:hfst:sem:hfst-eliminate-flags.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            long_options.push(getopt::Option {
                name: c"flag".as_ptr(),
                has_arg: 1, // required_argument
                flag: std::ptr::null_mut(),
                val: 'F' as c_int,
            });
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}F:",
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
            // cases, then unary cases, then the tool's own ('F'), then the
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
            if c == 'F' as c_int {
                FLAG = Some(cstr(getopt::OPTARG));
                continue;
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(argc, argv);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-eliminate-flags.process-stream-fn]
// [spec:hfst:sem:hfst-eliminate-flags.process-stream-fn]
unsafe fn process_stream(
    instream: &mut HfstInputStream,
    outstream: &mut HfstOutputStream,
) -> c_int {
    unsafe {
        //instream.open();
        //outstream.open();

        if !globals::SILENT {
            // hfst::set_warning_stream(&std::cerr); — route warnings to stderr.
            let warn: Box<dyn std::io::Write> = Box::new(std::io::stderr());
            hfst_transducer::set_warning_stream(Box::into_raw(Box::new(warn)));
        }

        let flag = (*std::ptr::addr_of!(FLAG)).clone();
        let flags: String = match &flag {
            None => String::from("flags"),
            Some(f) => format!("flag {}", f),
        };
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let mut trans = HfstTransducer::new_from_stream(instream);
            let mut inputname = hfst_get_name(&trans, &cstr(globals::INPUTFILENAME));
            if inputname.is_empty() {
                inputname = cstr(globals::INPUTFILENAME);
            }
            if transducer_n == 1 {
                verbose_printf(&format!("Eliminating {} {}...\n", flags, inputname));
            } else {
                verbose_printf(&format!(
                    "Eliminating {} {}...{}\n",
                    flags, inputname, transducer_n
                ));
            }
            match &flag {
                None => {
                    trans.eliminate_flags();
                }
                Some(f) => {
                    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        trans.eliminate_flag(f);
                    }));
                    if res.is_err() {
                        error(
                            libc::EXIT_FAILURE,
                            0,
                            &format!(
                                "flag feature {} does not occur in the transducer\nonly the flag feature must be given, no value or operator",
                                f
                            ),
                        );
                        return libc::EXIT_FAILURE;
                    }
                }
            }
            // C: hfst_set_name(trans, trans, "eliminate-flags"); the dest and
            // src are the same object, which Rust cannot alias mut+const, so the
            // read side is taken from a copy (name/formula are unchanged by the
            // copy).
            let src = trans.clone();
            hfst_set_name_unary(&mut trans, &src, "eliminate-flags");
            hfst_set_formula_unary(&mut trans, &src, "Id");
            outstream.redirect(&mut trans);
        }
        instream.close();
        outstream.close();
        libc::EXIT_SUCCESS
    }
}

// [spec:hfst:def:hfst-eliminate-flags.main-fn]
// [spec:hfst:sem:hfst-eliminate-flags.main-fn]
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

        hfst_set_program_name(&argv0, "0.1", "HfstEliminateFlags");
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

        if is_input_stream_in_ol_format(&instream, "hfst-eliminate-flags") {
            return libc::EXIT_FAILURE;
        }

        process_stream(&mut instream, &mut outstream)
    }
}
