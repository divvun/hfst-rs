#![allow(static_mut_refs)]
//! Faithful 1:1 port of tools/src/hfst-repeat.cc — the transducer repetition
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).

use core::ffi::{c_char, c_int};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_getenv, hfst_set_program_name, hfst_strtonumber,
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
use std::ffi::{CStr, CString};
use std::io::Write;

// add tools-specific variables here
static mut AT_LEAST: u64 = 0;
static mut AT_MOST: u64 = u32::MAX as u64;
static mut FROM_INFINITY: bool = false;
static mut TO_INFINITY: bool = true;

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

// [spec:hfst:def:hfst-repeat.print-usage-fn]
// [spec:hfst:sem:hfst-repeat.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = globals::message_writer();
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            &mut *msg,
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\nRepeat transducer\n\n",
                program_name
            ),
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        fput(
            &mut *msg,
            "Repetition options:\n  -f, --from=FNUM   repeat at least FNUM times\n  -t, --to=TNUM     repeat at most TNUM times\n",
        );
        fput(&mut *msg, "\n");
        print_common_unary_program_parameter_instructions(&mut *msg);
        fput(
            &mut *msg,
            "FNUM and TNUM must be positive integers or infinities as parsed by strtod(3)\nif FNUM is omitted it defaults to 0, if TNUM is omitted it defaults to Inf\nFNUM must be less than TNUM\n",
        );
        fput(&mut *msg, "\n");
        print_report_bugs();
        fput(&mut *msg, "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-repeat.parse-options-fn]
// [spec:hfst:sem:hfst-repeat.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            let from_name = CString::new("from").unwrap();
            let to_name = CString::new("to").unwrap();
            long_options.push(getopt::Option {
                name: from_name.as_ptr(),
                has_arg: 1, // required_argument
                flag: std::ptr::null_mut(),
                val: b'f' as c_int,
            });
            long_options.push(getopt::Option {
                name: to_name.as_ptr(),
                has_arg: 1, // required_argument
                flag: std::ptr::null_mut(),
                val: b't' as c_int,
            });
            // add tool-specific options here
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}f:t:",
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
            // cases, then unary cases, then the tool's own f/t cases, then the
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
            match c {
                c if c == b'f' as c_int => {
                    let mut from_inf = false;
                    AT_LEAST = hfst_strtonumber(&cstr(getopt::OPTARG), Some(&mut from_inf)) as u64;
                    FROM_INFINITY = from_inf;
                    continue;
                }
                c if c == b't' as c_int => {
                    let mut to_inf = false;
                    AT_MOST = hfst_strtonumber(&cstr(getopt::OPTARG), Some(&mut to_inf)) as u64;
                    TO_INFINITY = to_inf;
                    continue;
                }
                _ => {}
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(argc, argv);
        if AT_LEAST > AT_MOST {
            error(
                1,
                0,
                &format!("Cannot repeat from {} to {} times\n", AT_LEAST, AT_MOST),
            );
        }
        if FROM_INFINITY && !TO_INFINITY {
            error(
                1,
                0,
                &format!("Cannot repeat from infinity to {} times\n", AT_MOST),
            );
        }
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-repeat.process-stream-fn]
// [spec:hfst:sem:hfst-repeat.process-stream-fn]
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
                if !FROM_INFINITY && !TO_INFINITY {
                    verbose_printf(&format!(
                        "Repeating [{}..{}] {}...\n",
                        AT_LEAST, AT_MOST, inputname
                    ));
                } else if FROM_INFINITY && TO_INFINITY {
                    verbose_printf(&format!("Repeating star {}...\n", inputname));
                } else if !FROM_INFINITY && TO_INFINITY {
                    verbose_printf(&format!("Repeating [{}..*] {}...\n", AT_LEAST, inputname));
                } else if FROM_INFINITY && TO_INFINITY {
                    error(1, 0, &format!("Repeating *..{}?", AT_MOST));
                }
            } else if !FROM_INFINITY && !TO_INFINITY {
                verbose_printf(&format!(
                    "Repeating [{}..{}] {}... {}\n",
                    AT_LEAST, AT_MOST, inputname, transducer_n
                ));
            } else if FROM_INFINITY && TO_INFINITY {
                verbose_printf(&format!(
                    "Repeating star {}... {}\n",
                    inputname, transducer_n
                ));
            } else if !FROM_INFINITY && TO_INFINITY {
                verbose_printf(&format!(
                    "Repeating [{}..*] {}... {}\n",
                    AT_LEAST, inputname, transducer_n
                ));
            } else if FROM_INFINITY && TO_INFINITY {
                error(1, 0, &format!("Repeating *..{}?", AT_MOST));
            }

            if !FROM_INFINITY && !TO_INFINITY {
                trans.repeat_n_to_k(AT_LEAST as u32, AT_MOST as u32);
                let composed_name = format!("repeat-{}-to-{}", AT_LEAST, AT_MOST);
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, &composed_name);
                let composed_name = format!("_{}^{}", AT_LEAST, AT_MOST);
                let src = trans.clone();
                hfst_set_formula_unary(&mut trans, &src, &composed_name);
            } else if FROM_INFINITY && TO_INFINITY {
                trans.repeat_star();
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "repeat-star");
                let src = trans.clone();
                hfst_set_formula_unary(&mut trans, &src, "\u{22c6}");
            } else if !FROM_INFINITY && TO_INFINITY {
                trans.repeat_n_plus(AT_LEAST as u32);
                let composed_name = format!("repeat-{}-plus", AT_LEAST);
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, &composed_name);
                let composed_name = format!("_{}^\u{221e}", AT_LEAST);
                let src = trans.clone();
                hfst_set_formula_unary(&mut trans, &src, &composed_name);
            } else if FROM_INFINITY && !TO_INFINITY {
                error(1, 0, &format!("Repeating *..{}?", AT_MOST));
            }
            outstream.redirect(&mut trans);
        }
        instream.close();
        outstream.close();
        0
    }
}

// [spec:hfst:def:hfst-repeat.main-fn]
// [spec:hfst:sem:hfst-repeat.main-fn]
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

        hfst_set_program_name(&argv0, "0.1", "HfstRepeat");
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
        if !FROM_INFINITY && !TO_INFINITY {
            verbose_printf(&format!(
                "Repeating from {} to {} times\n",
                AT_LEAST, AT_MOST
            ));
        } else if FROM_INFINITY && TO_INFINITY {
            verbose_printf("Repeating star infinitely\n");
        } else if !FROM_INFINITY && TO_INFINITY {
            verbose_printf(&format!("Repeating from {} to infinite times\n", AT_LEAST));
        } else if FROM_INFINITY && !TO_INFINITY {
            error(
                1,
                0,
                &format!(
                    "Repeating at least infinite butno more than {} times?",
                    AT_MOST
                ),
            );
        }

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

        if is_input_stream_in_ol_format(&instream, "hfst-repeat") {
            return 1;
        }

        process_stream(&mut instream, &mut outstream)
    }
}
