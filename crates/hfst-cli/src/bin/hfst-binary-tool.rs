//! Faithful 1:1 port of tools/src/hfst-binary-tool.cc — the GENERIC BINARY
//! TOOL TEMPLATE command-line tool. Drives the hfst-cli foundation (globals,
//! getopt, commandline, program-options, tool-metadata, inc fragments).

use core::ffi::{c_char, c_int};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, extend_options_getenv, hfst_set_program_name, print_more_info,
    print_report_bugs, verbose_printf, warning,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    HFST_GETOPT_BINARY_SHORT, HFST_GETOPT_COMMON_SHORT, hfst_getopt_binary_long,
    hfst_getopt_common_long, print_common_binary_program_options,
    print_common_binary_program_parameter_instructions, print_common_program_options,
};
use hfst_cli::inc::{
    CaseResult, check_binary_params, check_common_params, handle_binary_case, handle_common_case,
    handle_error_case,
};
use std::ffi::{CStr, CString};

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

// [spec:hfst:def:hfst-binary-tool.print-usage-fn]
// [spec:hfst:sem:hfst-binary-tool.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        let mut msg = globals::message_writer();
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            &mut *msg,
            &format!(
                "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nDo things with two transducers\n\n",
                program_name
            ),
        );
        print_common_program_options(&mut *msg);
        print_common_binary_program_options(&mut *msg);
        fput(&mut *msg, "\n");
        print_common_binary_program_parameter_instructions(&mut *msg);
        fput(&mut *msg, "\n");
        fput(
            &mut *msg,
            &format!(
                "\nExamples:\n  {} -o catdog.hfst cat.hfst dog.hfst  does things\n\n",
                program_name
            ),
        );
        print_report_bugs();
        print_more_info();
    }
}

// [spec:hfst:def:hfst-binary-tool.parse-options-fn]
// [spec:hfst:sem:hfst-binary-tool.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_binary_long());
            // add tool-specific options here
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}",
                HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_BINARY_SHORT
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
            // cases, then binary cases, then the tool's own (none here), then
            // the terminal error arm.
            match handle_common_case(c, || print_usage()) {
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
        check_binary_params(argc, argv);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-binary-tool.binaryoperate-streams-fn]
// [spec:hfst:sem:hfst-binary-tool.binaryoperate-streams-fn]
unsafe fn binaryoperate_streams(
    firststream: &mut HfstInputStream,
    secondstream: &mut HfstInputStream,
    outstream: &mut HfstOutputStream,
) -> c_int {
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
                    cstr(globals::FIRSTFILENAME),
                    cstr(globals::SECONDFILENAME)
                ),
            );
        }
        let mut transducer_n: usize = 0;
        while both_inputs {
            transducer_n += 1;
            if transducer_n == 1 {
                verbose_printf(&format!(
                    "Doing things with {} and {}...\n",
                    cstr(globals::FIRSTFILENAME),
                    cstr(globals::SECONDFILENAME)
                ));
            } else {
                verbose_printf(&format!(
                    "Doing things with {} and {}... {}\n",
                    cstr(globals::FIRSTFILENAME),
                    cstr(globals::SECONDFILENAME),
                    transducer_n
                ));
            }
            let mut first = HfstTransducer::new_from_stream(firststream);
            let second = HfstTransducer::new_from_stream(secondstream);
            first.concatenate(&second, true);
            outstream.redirect(&mut first);
            both_inputs = firststream.is_good() && secondstream.is_good();
        }

        if firststream.is_good() {
            warning(
                0,
                0,
                &format!(
                    "Warning: {} contains more transducers than {}; residue skipped\n",
                    cstr(globals::FIRSTFILENAME),
                    cstr(globals::SECONDFILENAME)
                ),
            );
        } else if secondstream.is_good() {
            warning(
                0,
                0,
                &format!(
                    "Warning: {} contains fewer transducers than {}; residue skipped\n",
                    cstr(globals::FIRSTFILENAME),
                    cstr(globals::SECONDFILENAME)
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

        hfst_set_program_name(&argv0, "0.1", "HfstGenericBinaryTool");
        let retval = parse_options(argc, argv);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let first_opened = cstr(globals::FIRSTFILENAME) != "<stdin>";
        let second_opened = cstr(globals::SECONDFILENAME) != "<stdin>";
        let output_opened = cstr(globals::OUTFILENAME) != "<stdout>";
        verbose_printf(&format!(
            "Reading from {} and {}, writing to {}\n",
            cstr(globals::FIRSTFILENAME),
            cstr(globals::SECONDFILENAME),
            cstr(globals::OUTFILENAME)
        ));
        // here starts the buffer handling part
        // (the C wraps each ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch
        // arms are not reproduced here.)
        let mut firststream = if first_opened {
            HfstInputStream::new_filename(&cstr(globals::FIRSTFILENAME))
        } else {
            HfstInputStream::new()
        };
        let mut secondstream = if second_opened {
            HfstInputStream::new_filename(&cstr(globals::SECONDFILENAME))
        } else {
            HfstInputStream::new()
        };
        let type_ = firststream.get_type();
        let mut outstream = if output_opened {
            HfstOutputStream::new_filename(&cstr(globals::OUTFILENAME), type_, true)
        } else {
            HfstOutputStream::new(type_, true)
        };

        // (the C main calls concatenate_streams; the defined function is
        // binaryoperate_streams — the same routine — which is invoked here.)
        binaryoperate_streams(&mut firststream, &mut secondstream, &mut outstream)
    }
}
