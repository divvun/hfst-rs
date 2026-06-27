//! Faithful 1:1 port of tools/src/hfst-compose.cc — the transducer composition
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments). A binary tool:
//! it reads two input streams (firstfile + secondfile) and composes them.

use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_exception_defs::TransducerTypeMismatchException;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::{
    HfstTransducer, set_flag_is_epsilon_in_composition, set_xerox_composition,
};
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, conversion_type, convert_transducers, error, extend_options_getenv,
    hfst_set_program_name, hfst_strdup, hfst_strformat, is_input_stream_in_ol_format,
    print_more_info, print_report_bugs, verbose_printf, warning,
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

// [spec:hfst:def:hfst-compose.print-usage-fn]
// [spec:hfst:sem:hfst-compose.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            globals::message_out(),
            &format!(
                "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nCompose two transducers\n\n",
                program_name
            ),
        );
        print_common_program_options(globals::message_out());
        print_common_binary_program_options(globals::message_out());
        fput(
            globals::message_out(),
            "Composition options:\n  -x, --xerox-composition=VALUE Whether flag diacritics are treated as ordinary\n                                symbols in composition (default is false).\n  -X, --xfst=VARIABLE    Toggle xfst compatibility option VARIABLE.\nHarmonization:\n  -H, --do-not-harmonize Do not harmonize symbols.\n  -F, --harmonize-flags  Harmonize flag diacritics.\n",
        );
        fput(globals::message_out(), "\n");
        print_common_binary_program_parameter_instructions(globals::message_out());
        fput(globals::message_out(), "\n");
        fput(
            globals::message_out(),
            "Xfst variables are {flag-is-epsilon (default OFF)}.\n",
        );
        fput(
            globals::message_out(),
            "VALUE can be one of the following: [true|false], [yes|no] or [ON|OFF],\n",
        );
        fput(globals::message_out(), "false being the default.\n");
        fput(
            globals::message_out(),
            &format!(
                "\nExamples:\n  {} -o cat2dog.hfst cat2mouse.hfst mouse2dog.hfst  composes two automata\n\n",
                program_name
            ),
        );
        print_report_bugs();
        fput(globals::message_out(), "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-compose.parse-options-fn]
// [spec:hfst:sem:hfst-compose.parse-options-fn]
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
                name: CString::new("harmonize-flags").unwrap().into_raw() as *const c_char,
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: b'F' as c_int,
            });
            long_options.push(getopt::Option {
                name: CString::new("do-not-harmonize").unwrap().into_raw() as *const c_char,
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: b'H' as c_int,
            });
            long_options.push(getopt::Option {
                name: CString::new("xerox-composition").unwrap().into_raw() as *const c_char,
                has_arg: 1,
                flag: std::ptr::null_mut(),
                val: b'x' as c_int,
            });
            long_options.push(getopt::Option {
                name: CString::new("xfst").unwrap().into_raw() as *const c_char,
                has_arg: 1,
                flag: std::ptr::null_mut(),
                val: b'X' as c_int,
            });
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}FHx:X:",
                HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_BINARY_SHORT
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

            // The C switch chains the #include'd case groups in order: binary
            // cases, then common cases, then the tool's own, then the terminal
            // error arm.
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
            if c == b'F' as c_int {
                HARMONIZE_FLAGS = true;
                continue;
            } else if c == b'H' as c_int {
                HARMONIZE = false;
                continue;
            } else if c == b'x' as c_int {
                let argument = cstr(hfst_strdup(getopt::OPTARG));
                if argument == "yes" || argument == "true" || argument == "ON" {
                    set_xerox_composition(true);
                } else if argument == "no" || argument == "false" || argument == "OFF" {
                    set_xerox_composition(false);
                } else {
                    fput(
                        libc_stderr(),
                        &format!(
                            "Error: unknown option to --xerox-composition: '{}'\n",
                            cstr(getopt::OPTARG)
                        ),
                    );
                    return libc::EXIT_FAILURE;
                }
                continue;
            } else if c == b'X' as c_int {
                let argument = cstr(hfst_strdup(getopt::OPTARG));
                if argument == "flag-is-epsilon" {
                    set_flag_is_epsilon_in_composition(true);
                } else {
                    fput(
                        libc_stderr(),
                        &format!(
                            "Error: unknown option to --xfst: '{}'\n",
                            cstr(getopt::OPTARG)
                        ),
                    );
                    return libc::EXIT_FAILURE;
                }
                continue;
            }
            return handle_error_case(c);
        }

        check_binary_params(argc, argv);
        check_common_params();
        EXIT_CONTINUE
    }
}

unsafe fn libc_stderr() -> *mut libc::FILE {
    unsafe extern "C" {
        #[cfg_attr(target_os = "macos", link_name = "__stderrp")]
        static mut stderr: *mut libc::FILE;
    }
    unsafe { stderr }
}

// [spec:hfst:def:hfst-compose.compose-streams-fn]
// [spec:hfst:sem:hfst-compose.compose-streams-fn]
unsafe fn compose_streams(
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
                } else
                /* should not happen */
                {
                    std::panic::panic_any(
                        "Error: hfst-compose: conversion_type returned an invalid integer",
                    );
                }
                warning(0, 0, &warnstr);
            } else {
                error(
                    libc::EXIT_FAILURE,
                    0,
                    &format!(
                        "Transducer type mismatch in {} and {}; formats {} and {} are not compatible for composition (--do-not-convert was requested)",
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

        let output_opened = !globals::OUTFILE.is_null();
        let mut outstream = if output_opened {
            HfstOutputStream::new_filename(&cstr(globals::OUTFILENAME), output_type, true)
        } else {
            HfstOutputStream::new(output_type, true)
        };

        let mut first: Option<HfstTransducer> = None;
        let mut second: Option<HfstTransducer> = None;
        let mut transducer_n_first: usize = 0; // transducers read from first stream
        let mut transducer_n_second: usize = 0; // transducers read from second stream
        while continue_reading {
            if firststream.is_good() {
                first = Some(HfstTransducer::new_from_stream(firststream));
                transducer_n_first += 1;
            }
            if secondstream.is_good() {
                second = Some(HfstTransducer::new_from_stream(secondstream));
                transducer_n_second += 1;
            }
            let firstname = hfst_get_name(first.as_ref().unwrap(), &cstr(globals::FIRSTFILENAME));
            if second.is_none() {
                // make scan-build happy, this should not happen
                std::panic::panic_any("Error: second stream has a NULL value.");
            }
            let secondname =
                hfst_get_name(second.as_ref().unwrap(), &cstr(globals::SECONDFILENAME));
            if transducer_n_first == 1 {
                verbose_printf(&format!("Composing {} and {}...\n", firstname, secondname));
            } else {
                verbose_printf(&format!(
                    "Composing {} and {}... {}\n",
                    firstname, secondname, transducer_n_first
                ));
            }

            let has_flags = first.as_ref().unwrap().has_flag_diacritics()
                || second.as_ref().unwrap().has_flag_diacritics();
            if has_flags {
                if !HARMONIZE_FLAGS {
                    if !globals::SILENT {
                        warning(
                            0,
                            0,
                            "At least one of the arguments contains flag diacritics. Use -F to harmonize them.",
                        );
                    }
                } else {
                    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        first
                            .as_mut()
                            .unwrap()
                            .harmonize_flag_diacritics(second.as_mut().unwrap(), true);
                    }));
                    if res.is_err() {
                        let e = res.err().unwrap();
                        if e.downcast_ref::<TransducerTypeMismatchException>()
                            .is_some()
                        {
                            if globals::ALLOW_TRANSDUCER_CONVERSION {
                                convert_transducers(
                                    first.as_mut().unwrap(),
                                    second.as_mut().unwrap(),
                                );
                                first
                                    .as_mut()
                                    .unwrap()
                                    .harmonize_flag_diacritics(second.as_mut().unwrap(), true);
                            } else {
                                error(
                                    libc::EXIT_FAILURE,
                                    0,
                                    &format!(
                                        "Could not compose {} and {} [{}]:\nformats {} and {} are not compatible for composition (--do-not-convert was requested)",
                                        firstname,
                                        secondname,
                                        transducer_n_first,
                                        hfst_strformat(firststream.get_type()),
                                        hfst_strformat(secondstream.get_type())
                                    ),
                                );
                            }
                        } else {
                            std::panic::resume_unwind(e);
                        }
                    }
                }
            }

            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                first
                    .as_mut()
                    .unwrap()
                    .compose(second.as_ref().unwrap(), HARMONIZE);
            }));
            if res.is_err() {
                let e = res.err().unwrap();
                if e.downcast_ref::<TransducerTypeMismatchException>()
                    .is_some()
                {
                    if globals::ALLOW_TRANSDUCER_CONVERSION {
                        convert_transducers(first.as_mut().unwrap(), second.as_mut().unwrap());
                        first
                            .as_mut()
                            .unwrap()
                            .compose(second.as_ref().unwrap(), HARMONIZE);
                    } else {
                        error(
                            libc::EXIT_FAILURE,
                            0,
                            &format!(
                                "Could not compose {} and {} [{}]:\nformats {} and {} are not compatible for composition (--do-not-convert was requested)",
                                firstname,
                                secondname,
                                transducer_n_first,
                                hfst_strformat(firststream.get_type()),
                                hfst_strformat(secondstream.get_type())
                            ),
                        );
                    }
                } else {
                    std::panic::resume_unwind(e);
                }
            }

            // C: hfst_set_name(*first, *first, *second, "compose"); the dest and
            // lhs are the same object, which Rust cannot alias mut+const, so the
            // read side is taken from a copy (name/formula unchanged by copy).
            let first_copy = first.as_ref().unwrap().clone();
            let second_ref = second.as_ref().unwrap();
            hfst_set_name_binary(first.as_mut().unwrap(), &first_copy, second_ref, "compose");
            let second_ref = second.as_ref().unwrap();
            hfst_set_formula_binary(first.as_mut().unwrap(), &first_copy, second_ref, "\u{2218}");

            outstream.redirect(first.as_mut().unwrap());

            continue_reading = (firststream.is_good() && secondstream.is_good())
                || (firststream.is_good() && (transducer_n_second == 1))
                || ((transducer_n_first == 1) && secondstream.is_good());

            if !continue_reading {
                first = None;
                second = None;
            } else {
                if firststream.is_good() {
                    first = None;
                }
                if secondstream.is_good() {
                    second = None;
                }
            }
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
                    "first input '{}' contains fewer transducers than second input '{}'; this is only possible if the first input contains exactly one transducer",
                    cstr(globals::FIRSTFILENAME),
                    cstr(globals::SECONDFILENAME)
                ),
            );
        }

        firststream.close();
        secondstream.close();
        outstream.flush();
        outstream.close();

        libc::EXIT_SUCCESS
    }
}

// [spec:hfst:def:hfst-compose.main-fn]
// [spec:hfst:sem:hfst-compose.main-fn]
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

        hfst_set_program_name(&argv0, "0.1", "HfstCompose");
        let retval = parse_options(argc, argv);
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
        // (the C wraps the ctors in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)
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

        if is_input_stream_in_ol_format(&firststream, "hfst-compose")
            || is_input_stream_in_ol_format(&secondstream, "hfst-compose")
        {
            return libc::EXIT_FAILURE;
        }

        compose_streams(&mut firststream, &mut secondstream)
    }
}
