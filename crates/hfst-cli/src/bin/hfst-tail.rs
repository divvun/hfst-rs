#![allow(static_mut_refs)]
//! Faithful 1:1 port of tools/src/hfst-tail.cc — the transducer archive
//! tailing command-line tool. Drives the hfst-cli foundation (globals,
//! getopt, commandline, program-options, tool-metadata, inc fragments).

use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, extend_options_getenv, hfst_set_program_name, hfst_strtol, print_more_info,
    print_report_bugs, verbose_printf,
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
use std::collections::VecDeque;
use std::ffi::{CStr, CString};
use std::io::Write;

// add tools-specific variables here
static mut TAIL_COUNT: i64 = -1;

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

// [spec:hfst:def:hfst-tail.print-usage-fn]
// [spec:hfst:sem:hfst-tail.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = globals::message_writer();
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            &mut *msg,
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\nGet last transducers from an archive\n\n",
                program_name
            ),
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        fput(
            &mut *msg,
            "Archive options:\n  -n, --n-last=[+]K   Print the last K transducers;\n                      use +K to print transducers starting from the Kth\n",
        );
        fput(&mut *msg, "\n");
        print_common_unary_program_parameter_instructions(&mut *msg);
        fput(
            &mut *msg,
            "K must be an integer, as parsed by strtoul base 10, and not 0.\nif K is omitted, it defaults to +1 (all except the first)\n",
        );
        fput(&mut *msg, "\n");
        print_report_bugs();
        fput(&mut *msg, "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-tail.parse-options-fn]
// [spec:hfst:sem:hfst-tail.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let n_last = CString::new("n-last").unwrap();
            long_options.push(getopt::Option {
                name: n_last.as_ptr(),
                has_arg: 1, // required_argument
                flag: std::ptr::null_mut(),
                val: 'n' as c_int,
            });
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}n:",
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
            // cases, then unary cases, then the tool's own ('n'), then the
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
            if c == 'n' as c_int {
                let optarg = cstr(getopt::OPTARG);
                if optarg.starts_with('+') {
                    // swap sign haha lol
                    TAIL_COUNT = -hfst_strtol(&optarg, 10);
                } else {
                    TAIL_COUNT = hfst_strtol(&optarg, 10);
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

// [spec:hfst:def:hfst-tail.process-stream-fn]
// [spec:hfst:sem:hfst-tail.process-stream-fn]
unsafe fn process_stream(
    instream: &mut HfstInputStream,
    outstream: &mut HfstOutputStream,
) -> c_int {
    unsafe {
        let mut last_n: VecDeque<HfstTransducer> = VecDeque::new();
        let mut transducer_n: i64 = 0;
        if TAIL_COUNT > 0 {
            verbose_printf(&format!("Counting last {} transducers...\n", TAIL_COUNT));
            while instream.is_good() {
                transducer_n += 1;
                let trans = HfstTransducer::new_from_stream(instream);
                last_n.push_back(trans);
                if last_n.len() as i64 > TAIL_COUNT {
                    last_n.pop_front();
                }
            }
            if TAIL_COUNT < transducer_n {
                transducer_n -= TAIL_COUNT + 1;
            } else {
                transducer_n = 0;
            }
            while !last_n.is_empty() {
                transducer_n += 1;
                verbose_printf(&format!(
                    "Forwarding {}...{}\n",
                    cstr(globals::INPUTFILENAME),
                    transducer_n
                ));
                let mut front = last_n.pop_front().unwrap();
                outstream.redirect(&mut front);
            }
        } else if TAIL_COUNT < 0 {
            verbose_printf(&format!("Skipping {} transducers...\n", -TAIL_COUNT));
            while instream.is_good() {
                transducer_n += 1;
                let mut trans = HfstTransducer::new_from_stream(instream);
                if transducer_n >= -TAIL_COUNT {
                    verbose_printf(&format!(
                        "Forwarding {}...{}\n",
                        cstr(globals::INPUTFILENAME),
                        transducer_n
                    ));
                    outstream.redirect(&mut trans);
                }
            }
        }
        outstream.flush();
        instream.close();
        outstream.close();
        libc::EXIT_SUCCESS
    }
}

// [spec:hfst:def:hfst-tail.main-fn]
// [spec:hfst:sem:hfst-tail.main-fn]
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

        hfst_set_program_name(&argv0, "0.2", "HfstTail");
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

        process_stream(&mut instream, &mut outstream)
    }
}
