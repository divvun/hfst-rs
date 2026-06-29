//! Faithful 1:1 port of tools/src/hfst-compare.cc — the transducer comparison
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, inc fragments). A binary tool: it reads from
//! two input streams (first + second) and writes a comparison log.

use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_getenv, hfst_set_program_name, hfst_strformat,
    is_input_stream_in_ol_format, print_more_info, print_report_bugs, verbose_printf,
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
use libc::{c_char, c_int};
use std::ffi::{CStr, CString};

// Tool-specific option state (C: 'static bool harmonize=true; static bool
// eliminate_flags=false;').
static mut HARMONIZE: bool = true;
static mut ELIMINATE_FLAGS: bool = false;

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

// [spec:hfst:def:hfst-compare.print-usage-fn]
// [spec:hfst:sem:hfst-compare.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        let mut msg = globals::message_writer();
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            &mut *msg,
            &format!(
                "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nCompare two transducers\n\n",
                program_name
            ),
        );
        print_common_program_options(&mut *msg);
        print_common_binary_program_options(&mut *msg);
        fput(
            &mut *msg,
            "Harmonization:\n  -H, --do-not-harmonize Do not harmonize symbols.\n  -e, --eliminate-flags  Eliminate flag diacritics.\n",
        );
        fput(&mut *msg, "\n");
        print_common_binary_program_parameter_instructions(&mut *msg);
        fput(&mut *msg, "\n");
        fput(
            &mut *msg,
            &format!(
                "\nExamples:\n  $ {0} cat.hfst dog.hfst\n  cat.hfst[1] != dog.hfst[1]\n  $ {0} cat.hfst cat.hfst\n  cat.hfst[1] == cat.hfst[1]\n\n",
                program_name
            ),
        );
        print_report_bugs();
        fput(&mut *msg, "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-compare.parse-options-fn]
// [spec:hfst:sem:hfst-compare.parse-options-fn]
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
                name: c"do-not-harmonize".as_ptr(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 'H' as c_int,
            });
            long_options.push(getopt::Option {
                name: c"eliminate-flags".as_ptr(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 'e' as c_int,
            });
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}He",
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

            // The C switch chains the #include'd case groups in order: common
            // cases, then binary cases, then the tool's own ('H'/'e'), then the
            // terminal error arm.
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
            match c as u8 as char {
                'H' => {
                    HARMONIZE = false;
                    continue;
                }
                'e' => {
                    ELIMINATE_FLAGS = true;
                    continue;
                }
                _ => {}
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_binary_params(argc, argv);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-compare.compare-streams-fn]
// [spec:hfst:sem:hfst-compare.compare-streams-fn]
unsafe fn compare_streams(
    firststream: &mut HfstInputStream,
    secondstream: &mut HfstInputStream,
) -> c_int {
    unsafe {
        let mut out = match globals::output_writer() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("hfst-compare: cannot open output: {e}");
                return libc::EXIT_FAILURE;
            }
        };
        let mut continue_reading = firststream.is_good() && secondstream.is_good();
        let mut transducer_n_first: usize = 0; // transducers read from first input
        let mut transducer_n_second: usize = 0; // transducers read from second input
        let mut mismatches: usize = 0;

        let mut second: Option<HfstTransducer> = None;

        while continue_reading {
            let mut first = HfstTransducer::new_from_stream(firststream);
            transducer_n_first += 1;
            if secondstream.is_good() {
                second = Some(HfstTransducer::new_from_stream(secondstream));
                transducer_n_second += 1;
            }
            let mut firstname = first.get_name();
            // make scan-build happy, this should not happen
            let second_ref = match second.as_mut() {
                Some(s) => s,
                None => panic!("Error: second stream has a NULL value."),
            };
            let mut secondname = second_ref.get_name();
            if firstname.is_empty() {
                firstname = cstr(globals::FIRSTFILENAME);
            }
            if secondname.is_empty() {
                secondname = cstr(globals::SECONDFILENAME);
            }
            if transducer_n_first == 1 {
                verbose_printf(&format!("Comparing {} and {}...\n", firstname, secondname));
            } else {
                verbose_printf(&format!(
                    "Comparing {} and {}... {}\n",
                    firstname, secondname, transducer_n_first
                ));
            }
            // C: try { ... } catch (TransducerTypeMismatchException). The Rust
            // 'compare' panics with TransducerTypeMismatchException on a type
            // mismatch, so the try is reproduced with catch_unwind.
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if ELIMINATE_FLAGS {
                    verbose_printf("Eliminating flags...\n");
                    first.eliminate_flags();
                    second_ref.eliminate_flags();
                }
                first.compare(second_ref, HARMONIZE)
            }));
            match outcome {
                Ok(equal) => {
                    if equal {
                        if transducer_n_first == 1 {
                            if !globals::SILENT {
                                fput(&mut *out, &format!("{} == {}\n", firstname, secondname));
                            }
                        } else if !globals::SILENT {
                            fput(
                                &mut *out,
                                &format!(
                                    "{}[{}] == {}[{}]\n",
                                    firstname, transducer_n_first, secondname, transducer_n_second
                                ),
                            );
                        }
                    } else {
                        if transducer_n_first == 1 {
                            if !globals::SILENT {
                                fput(&mut *out, &format!("{} != {}\n", firstname, secondname));
                            }
                        } else if !globals::SILENT {
                            fput(
                                &mut *out,
                                &format!(
                                    "{}[{}] != {}[{}]\n",
                                    firstname, transducer_n_first, secondname, transducer_n_second
                                ),
                            );
                        }
                        mismatches += 1;
                    }
                }
                Err(_) => {
                    // cannot recover yet, but beautify error messages
                    error(
                        2,
                        0,
                        &format!(
                            "Cannot compare `{}' and `{}' [{}]\nthe formats {} and {} are not compatible for comparison",
                            firstname,
                            secondname,
                            transducer_n_first,
                            hfst_strformat(firststream.get_type()),
                            hfst_strformat(secondstream.get_type())
                        ),
                    );
                }
            }

            continue_reading =
                firststream.is_good() && (secondstream.is_good() || transducer_n_second == 1);

            // delete the transducer of second stream, unless we continue reading
            // the first stream and there is only one transducer in the second
            // stream
            if (continue_reading && secondstream.is_good()) || !continue_reading {
                second = None;
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
        } else if secondstream.is_good() {
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
        let _ = out.flush();
        if mismatches == 0 {
            verbose_printf(&format!("All {} transducers matched\n", transducer_n_first));
            libc::EXIT_SUCCESS
        } else {
            verbose_printf(&format!(
                "{}/{} were not equal\n",
                mismatches, transducer_n_first
            ));
            libc::EXIT_FAILURE
        }
    }
}

// [spec:hfst:def:hfst-compare.main-fn]
// [spec:hfst:sem:hfst-compare.main-fn]
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

        hfst_set_program_name(&argv0, "0.1", "HfstCompare");
        let retval = parse_options(argc, argv);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let first_is_stdin = cstr(globals::FIRSTFILENAME) == "<stdin>";
        let second_is_stdin = cstr(globals::SECONDFILENAME) == "<stdin>";
        verbose_printf(&format!(
            "Reading from {} and {}, writing log to {}\n",
            cstr(globals::FIRSTFILENAME),
            cstr(globals::SECONDFILENAME),
            cstr(globals::OUTFILENAME)
        ));
        // here starts the buffer handling part
        // (the C wraps each ctor in try/catch on HfstException, calling
        // error(EXIT_FAILURE, ...) on a bad file; the Rust ctor currently panics
        // on a bad file rather than throwing, so the catch arm is not reproduced.)
        let mut firststream = if !first_is_stdin {
            HfstInputStream::new_filename(&cstr(globals::FIRSTFILENAME))
        } else {
            HfstInputStream::new()
        };
        let mut secondstream = if !second_is_stdin {
            HfstInputStream::new_filename(&cstr(globals::SECONDFILENAME))
        } else {
            HfstInputStream::new()
        };

        if is_input_stream_in_ol_format(&firststream, "hfst-compare")
            || is_input_stream_in_ol_format(&secondstream, "hfst-compare")
        {
            return libc::EXIT_FAILURE;
        }

        compare_streams(&mut firststream, &mut secondstream)
    }
}
