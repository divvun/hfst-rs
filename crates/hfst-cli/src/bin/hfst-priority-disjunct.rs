//! Faithful 1:1 port of tools/src/hfst-priority-disjunct.cc — the transducer
//! priority disjunction (priority union) command-line tool. Drives the
//! hfst-cli foundation (globals, getopt, commandline, program-options,
//! tool-metadata, inc fragments). A BINARY tool: it reads two input streams
//! (firstfile + secondfile) and writes their priority union.

use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, conversion_type, convert_transducers, error, extend_options_getenv,
    hfst_set_program_name, hfst_strformat, is_input_stream_in_ol_format, print_more_info,
    print_report_bugs, verbose_printf, warning,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    HFST_GETOPT_BINARY_SHORT, HFST_GETOPT_COMMON_SHORT, hfst_getopt_binary_long,
    hfst_getopt_common_long, print_common_binary_program_options,
    print_common_binary_program_parameter_instructions, print_common_program_options,
};
use hfst_cli::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_binary, hfst_set_name_binary};
use hfst_cli::inc::{
    CaseResult, check_binary_params, check_common_params, handle_binary_case, handle_common_case,
    handle_error_case,
};
use libc::{c_char, c_int};
use std::ffi::{CStr, CString};

static mut HARMONIZE_FLAGS: bool = false;
static mut HARMONIZE: bool = true;

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

// [spec:hfst:def:hfst-priority-disjunct.print-usage-fn]
// [spec:hfst:sem:hfst-priority-disjunct.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            globals::message_out(),
            &format!(
                "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nDisjunct (union, OR) two transducers\n\n",
                program_name
            ),
        );
        print_common_program_options(globals::message_out());
        print_common_binary_program_options(globals::message_out());
        fput(globals::message_out(), "\n");
        print_common_binary_program_parameter_instructions(globals::message_out());
        fput(
            globals::message_out(),
            "Harmonization:\n  -H, --do-not-harmonize Do not harmonize symbols.\n  -F, --harmonize-flags  Harmonize flag diacritics.\n",
        );
        fput(globals::message_out(), "\n");
        fput(
            globals::message_out(),
            &format!(
                "\nExamples:\n  {} -o cat_or_dog.hfst cat.hfst dog.hfst\n\n",
                program_name
            ),
        );
        print_report_bugs();
        fput(globals::message_out(), "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-priority-disjunct.parse-options-fn]
// [spec:hfst:sem:hfst-priority-disjunct.parse-options-fn]
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
                name: CString::new("do-not-harmonize").unwrap().into_raw(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: b'H' as c_int,
            });
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}H",
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

            // The C switch chains the #include'd case groups in order: binary
            // cases, then common cases, then the tool's own ('H'), then the
            // terminal error arm.
            match handle_binary_case(c) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_common_case(c, || print_usage()) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            if c == b'H' as c_int {
                HARMONIZE = false;
                continue;
            }
            return handle_error_case(c);
        }

        check_binary_params(argc, argv);
        check_common_params();
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-priority-disjunct.priority-disjunct-streams-fn]
// [spec:hfst:sem:hfst-priority-disjunct.priority-disjunct-streams-fn]
unsafe fn priority_disjunct_streams(
    firststream: &mut HfstInputStream,
    secondstream: &mut HfstInputStream,
) -> c_int {
    unsafe {
        // there must be at least one transducer in both input streams
        let mut continue_reading = firststream.is_good() && secondstream.is_good();

        let type1 = firststream.get_type();
        let type2 = secondstream.get_type();
        let mut output_type = ImplementationType::UNSPECIFIED_TYPE;
        if type1 != type2 {
            if globals::ALLOW_TRANSDUCER_CONVERSION {
                let ct = conversion_type(type1, type2);
                let mut warnstr = format!(
                    "Transducer type mismatch in {} and {}; ",
                    cstr(globals::FIRSTFILENAME),
                    cstr(globals::SECONDFILENAME)
                );
                if ct == 1 {
                    warnstr.push_str("using former type as output");
                    output_type = type1;
                } else if ct == 2 {
                    warnstr.push_str("using latter type as output");
                    output_type = type2;
                } else if ct == -1 {
                    warnstr
                        .push_str("using former type as output, loss of information is possible");
                    output_type = type1;
                } else {
                    /* should not happen */
                    std::panic::panic_any(String::from(
                        "Error: hfst-priority-disjunct: conversion_type returned an invalid integer",
                    ));
                }
                warning(0, 0, &warnstr);
            } else {
                error(
                    libc::EXIT_FAILURE,
                    0,
                    &format!(
                        "Transducer type mismatch in {} and {}; formats {} and {} are not compatible for priority disjunction (--do-not-convert was requested)",
                        cstr(globals::FIRSTFILENAME),
                        cstr(globals::SECONDFILENAME),
                        hfst_strformat(type1),
                        hfst_strformat(type2)
                    ),
                );
            }
        } else {
            output_type = type1;
        }

        let output_named = !globals::OUTFILE.is_null();
        let mut outstream = if output_named {
            HfstOutputStream::new_filename(&cstr(globals::OUTFILENAME), output_type, true)
        } else {
            HfstOutputStream::new(output_type, true)
        };

        let mut first: Option<HfstTransducer> = None;
        let mut second: Option<HfstTransducer> = None;
        let mut transducer_n_first: usize = 0; // transducers read from first stream
        let mut transducer_n_second: usize = 0; // transducers read from second stream
        while continue_reading {
            first = Some(HfstTransducer::new_from_stream(firststream));
            transducer_n_first += 1;
            if secondstream.is_good() {
                second = Some(HfstTransducer::new_from_stream(secondstream));
                transducer_n_second += 1;
            }
            let first_t = first.as_mut().unwrap();
            let firstname = hfst_get_name(first_t, &cstr(globals::FIRSTFILENAME));
            if second.is_none() {
                // make scan-build happy, this should not happen
                std::panic::panic_any(String::from("Error: second stream has a NULL value."));
            }
            let secondname =
                hfst_get_name(second.as_ref().unwrap(), &cstr(globals::SECONDFILENAME));
            if transducer_n_first == 1 {
                verbose_printf(&format!(
                    "Disjuncting {} and {}...\n",
                    firstname, secondname
                ));
            } else {
                verbose_printf(&format!(
                    "Disjuncting {} and {}... {}\n",
                    firstname, secondname, transducer_n_first
                ));
            }
            let mismatch = {
                let second_ref = second.as_ref().unwrap();
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    first.as_mut().unwrap().priority_union(second_ref); // harmonize
                }))
                .is_err()
            };
            if mismatch {
                if globals::ALLOW_TRANSDUCER_CONVERSION {
                    let mut second_t = second.take().unwrap();
                    convert_transducers(first.as_mut().unwrap(), &mut second_t);
                    first.as_mut().unwrap().priority_union(&second_t); // , harmonize);
                    second = Some(second_t);
                } else {
                    error(
                        libc::EXIT_FAILURE,
                        0,
                        &format!(
                            "Could not priority disjunct {} and {} [{}]:\nformats {} and {} are not compatible for priority disjunction (--do-not-convert was requested)",
                            firstname,
                            secondname,
                            transducer_n_first,
                            hfst_strformat(firststream.get_type()),
                            hfst_strformat(secondstream.get_type())
                        ),
                    );
                }
            }
            // C: hfst_set_name(*first, *first, *second, "union"); the dest and
            // first src are the same object, which Rust cannot alias mut+const,
            // so the read side is taken from a copy (name/formula are unchanged
            // by the copy).
            let first_src = first.as_ref().unwrap().clone();
            let second_ref = second.as_ref().unwrap();
            hfst_set_name_binary(first.as_mut().unwrap(), &first_src, second_ref, "union");
            hfst_set_formula_binary(first.as_mut().unwrap(), &first_src, second_ref, "\u{222a}");
            outstream.redirect(first.as_mut().unwrap());

            continue_reading =
                firststream.is_good() && (secondstream.is_good() || transducer_n_second == 1);

            first = None;
            // delete the transducer of second stream, unless we continue
            // reading the first stream and there is only one transducer in the
            // second stream
            if (continue_reading && secondstream.is_good()) || !continue_reading {
                second = None;
            }

            outstream.flush();
        }

        if firststream.is_good() {
            error(
                libc::EXIT_FAILURE,
                0,
                &format!(
                    "second input '{}' contains fewer transducers than first input '{}'; this is only possible if the second input contains exactly one transducer",
                    cstr(globals::SECONDFILENAME),
                    cstr(globals::FIRSTFILENAME)
                ),
            );
        }

        if secondstream.is_good() {
            error(
                libc::EXIT_FAILURE,
                0,
                &format!(
                    "first input '{}' contains fewer transducers than second input '{}'",
                    cstr(globals::FIRSTFILENAME),
                    cstr(globals::SECONDFILENAME)
                ),
            );
        }

        firststream.close();
        secondstream.close();
        outstream.close();
        let _ = HARMONIZE_FLAGS;
        let _ = HARMONIZE;
        libc::EXIT_SUCCESS
    }
}

// [spec:hfst:def:hfst-priority-disjunct.main-fn]
// [spec:hfst:sem:hfst-priority-disjunct.main-fn]
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

        hfst_set_program_name(&argv0, "0.1", "HfstPriorityDisjunct");
        let mut retval = parse_options(argc, argv);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let first_opened = !globals::FIRSTFILE.is_null();
        let second_opened = !globals::SECONDFILE.is_null();
        let output_opened = !globals::OUTFILE.is_null();
        if first_opened {
            libc::fclose(globals::FIRSTFILE);
        }
        if second_opened {
            libc::fclose(globals::SECONDFILE);
        }
        if output_opened {
            libc::fclose(globals::OUTFILE);
        }
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

        if is_input_stream_in_ol_format(&firststream, "hfst-priority-disjunct")
            || is_input_stream_in_ol_format(&secondstream, "hfst-priority-disjunct")
        {
            return libc::EXIT_FAILURE;
        }

        retval = priority_disjunct_streams(&mut firststream, &mut secondstream);
        retval
    }
}
