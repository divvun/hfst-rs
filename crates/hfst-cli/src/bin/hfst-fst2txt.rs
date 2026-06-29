//! Faithful 1:1 port of tools/src/hfst-fst2txt.cc — the transducer array
//! printing command-line tool. Prints a transducer in AT&T, dot, prolog or
//! pckimmo text format. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, inc fragments).

use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_print_dot::print_dot_file;
use hfst::hfst_print_pckimmo::print_pckimmo;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_getenv, hfst_set_program_name, print_more_info,
    print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT, hfst_getopt_common_long,
    hfst_getopt_unary_long, print_common_program_options, print_common_unary_program_options,
};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use libc::{c_char, c_int};
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

unsafe fn fput(f: *mut libc::FILE, s: &str) {
    let c = CString::new(s).unwrap_or_default();
    unsafe { libc::fputs(c.as_ptr(), f) };
}

// add tools-specific variables here
static mut USE_NUMBERS: bool = false;
static mut PRINT_WEIGHTS: bool = false;
static mut DO_NOT_PRINT_WEIGHTS: bool = false;

// [spec:hfst:def:hfst-fst2txt.fst-text-format]
#[derive(Clone, Copy, PartialEq, Eq)]
enum FstTextFormat {
    AttText,     // AT&T / OpenFst compatible TSV
    DotText,     // Graphviz / dotty
    PckimmoText, // PCKIMMO format
    PrologText,  // prolog format
}

static mut FORMAT: FstTextFormat = FstTextFormat::AttText;

// [spec:hfst:def:hfst-fst2txt.print-usage-fn]
// [spec:hfst:sem:hfst-fst2txt.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            globals::message_out(),
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\nPrint transducer in AT&T, dot, prolog or pckimmo format\n\n",
                program_name
            ),
        );
        print_common_program_options(globals::message_out());
        print_common_unary_program_options(globals::message_out());
        fput(
            globals::message_out(),
            "Text format options:\n  -w, --print-weights          If weights are printed in all cases\n  -D, --do-not-print-weights   If weights are not printed in any case\n  -f, --format=TFMT            Print output in TFMT format [default=att]\n",
        );
        fput(globals::message_out(), "\n");
        fput(
            globals::message_out(),
            "If OUTFILE or INFILE is missing or -, standard streams will be used.\nUnless explicitly requested with option -w or -D, weights are printed\nif and only if the transducer is in weighted format.\nTFMT is one of {att, dot, prolog, pckimmo}.\n",
        );
        fput(globals::message_out(), "\n");
        print_report_bugs();
        fput(globals::message_out(), "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-fst2txt.parse-options-fn]
// [spec:hfst:sem:hfst-fst2txt.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let print_weights_name = CString::new("print-weights").unwrap();
            let do_not_print_weights_name = CString::new("do-not-print-weights").unwrap();
            let use_numbers_name = CString::new("use-numbers").unwrap();
            let format_name = CString::new("format").unwrap();
            long_options.push(getopt::Option {
                name: print_weights_name.as_ptr(),
                has_arg: 0, // no_argument
                flag: std::ptr::null_mut(),
                val: 'w' as c_int,
            });
            long_options.push(getopt::Option {
                name: do_not_print_weights_name.as_ptr(),
                has_arg: 0, // no_argument
                flag: std::ptr::null_mut(),
                val: 'D' as c_int,
            });
            long_options.push(getopt::Option {
                name: use_numbers_name.as_ptr(),
                has_arg: 0, // no_argument
                flag: std::ptr::null_mut(),
                val: 'n' as c_int,
            });
            long_options.push(getopt::Option {
                name: format_name.as_ptr(),
                has_arg: 1, // required_argument
                flag: std::ptr::null_mut(),
                val: 'f' as c_int,
            });
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}wDnf:",
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
            // cases, then unary cases, then the tool's own, then the terminal
            // error arm.
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
            // add tool-specific cases here
            match c {
                x if x == 'w' as c_int => {
                    PRINT_WEIGHTS = true;
                    continue;
                }
                x if x == 'D' as c_int => {
                    DO_NOT_PRINT_WEIGHTS = true;
                    continue;
                }
                x if x == 'n' as c_int => {
                    USE_NUMBERS = true;
                    continue;
                }
                x if x == 'f' as c_int => {
                    let optarg = cstr(getopt::OPTARG);
                    if optarg == "att"
                        || optarg == "AT&T"
                        || optarg == "openfst"
                        || optarg == "OpenFst"
                    {
                        FORMAT = FstTextFormat::AttText;
                    } else if optarg == "dot" || optarg == "graphviz" || optarg == "GraphViz" {
                        FORMAT = FstTextFormat::DotText;
                    } else if optarg == "pckimmo" {
                        FORMAT = FstTextFormat::PckimmoText;
                    } else if optarg == "prolog" || optarg == "Prolog" {
                        FORMAT = FstTextFormat::PrologText;
                    } else {
                        error(
                            libc::EXIT_FAILURE,
                            0,
                            &format!(
                                "Cannot parse {} as text format; Use one of att, pckimmo, dot, prolog",
                                optarg
                            ),
                        );
                    }
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

// [spec:hfst:def:hfst-fst2txt.process-stream-fn]
// [spec:hfst:sem:hfst-fst2txt.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream, outf: *mut libc::FILE) -> c_int {
    unsafe {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            // C: catches TransducerTypeMismatchException -> error "input
            // transducers do not have the same type"; the Rust ctor currently
            // panics rather than throwing, so the catch arm is not reproduced.
            let mut t = HfstTransducer::new_from_stream(instream);
            let mut inputname = t.get_name();
            if inputname.is_empty() {
                inputname = cstr(globals::INPUTFILENAME);
            }
            if transducer_n == 1 {
                verbose_printf(&format!("Converting {}...\n", inputname));
            } else {
                if instream.get_type() == ImplementationType::XFSM_TYPE {
                    error(
                        libc::EXIT_FAILURE,
                        0,
                        "Writing more than one transducer in text format to file not supported for xfsm transducers,\nuse [hfst-head|hfst-tail|hfst-split] to extract individual transducers from input",
                    );
                    return libc::EXIT_FAILURE;
                }
                verbose_printf(&format!("Converting {}...{}\n", inputname, transducer_n));
            }

            if transducer_n > 1 {
                fput(outf, "--\n");
            }

            let printw: bool; // whether weights are printed
            let type_ = t.get_type();
            if PRINT_WEIGHTS {
                printw = true;
            } else if DO_NOT_PRINT_WEIGHTS {
                printw = false;
            } else if type_ == ImplementationType::SFST_TYPE
                || type_ == ImplementationType::FOMA_TYPE
                || type_ == ImplementationType::XFSM_TYPE
            {
                printw = false;
            } else if type_.is_weighted() {
                // tropical/log OpenFST and weighted optimized-lookup; the prior
                // SFST/foma/xfsm arm already returned false, and the else arm
                // below also yields true, so this is byte-for-byte equivalent to
                // the original `type_ == TROPICAL_OPENFST || type_ == LOG_OPENFST`.
                printw = true;
            } else {
                // this should not happen
                printw = true;
            }
            match FORMAT {
                FstTextFormat::AttText => {
                    if USE_NUMBERS {
                        // xfsm case checked earlier
                        t.write_in_att_format_number(outf, printw);
                    } else {
                        // xfsm not yet supported
                        t.write_in_att_format_file(outf, printw);
                    }
                }
                FstTextFormat::DotText => {
                    // xfsm case checked earlier
                    fput(outf, "// This graph generated with hfst-fst2txt\n");
                    print_dot_file(outf, &mut t);
                }
                FstTextFormat::PckimmoText => {
                    // xfsm case checked earlier
                    print_pckimmo(outf, &t);
                }
                FstTextFormat::PrologText => {
                    // C: catches HfstException -> error "Error encountered when
                    // writing in prolog format". The Rust impl panics; the catch
                    // arm is not reproduced here.
                    if type_ == ImplementationType::XFSM_TYPE {
                        // no name or weights printed
                        let c_outfilename = CString::new(cstr(globals::OUTFILENAME)).unwrap();
                        t.write_xfsm_transducer_in_prolog_format(c_outfilename.as_ptr());
                    } else {
                        let namestr = t.get_name();
                        let alt_namestr = format!("NO_NAME_{}", transducer_n);
                        let namestr = if namestr.is_empty() {
                            if !globals::SILENT {
                                fput(
                                    stderr_file(),
                                    &format!(
                                        "Transducer has no name, giving it a name '{}'...\n",
                                        alt_namestr
                                    ),
                                );
                            }
                            alt_namestr
                        } else {
                            if !globals::SILENT {
                                fput(
                                    stderr_file(),
                                    &format!("Renaming transducer into '{}'...\n", alt_namestr),
                                );
                            }
                            alt_namestr
                        };
                        t.write_in_prolog_format(outf, &namestr, printw);
                    }
                }
            }
            // C: delete t; (Rust drops at end of loop iteration).
        }
        instream.close();
        if outf != stdout_file() {
            libc::fclose(outf);
        }
        libc::EXIT_SUCCESS
    }
}

// libc stdout FILE* helper (the C compares 'outf != stdout').
fn stdout_file() -> *mut libc::FILE {
    unsafe extern "C" {
        #[cfg_attr(target_os = "macos", link_name = "__stdoutp")]
        static mut stdout: *mut libc::FILE;
    }
    unsafe { stdout }
}

// libc stderr FILE* helper (the C writes diagnostics with 'fprintf(stderr, ...)').
fn stderr_file() -> *mut libc::FILE {
    unsafe extern "C" {
        #[cfg_attr(target_os = "macos", link_name = "__stderrp")]
        static mut stderr: *mut libc::FILE;
    }
    unsafe { stderr }
}

// [spec:hfst:def:hfst-fst2txt.main-fn]
// [spec:hfst:sem:hfst-fst2txt.main-fn]
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

        hfst_set_program_name(&argv0, "0.3", "HfstFst2Txt");
        let retval = parse_options(argc, argv);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let input_opened = !globals::INPUTFILE.is_null();
        if input_opened {
            libc::fclose(globals::INPUTFILE);
        }

        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            cstr(globals::INPUTFILENAME),
            cstr(globals::OUTFILENAME)
        ));
        // here starts the buffer handling part
        // (the C wraps the ctor in try/catch on HfstException -> error
        // "%s is not a valid transducer file"; the Rust ctor currently panics
        // rather than throwing, so the catch arm is not reproduced here.)
        let mut instream = if input_opened {
            HfstInputStream::new_filename(&cstr(globals::INPUTFILENAME))
        } else {
            HfstInputStream::new()
        };

        if instream.get_type() == ImplementationType::XFSM_TYPE {
            if FORMAT == FstTextFormat::DotText {
                error(
                    libc::EXIT_FAILURE,
                    0,
                    "Output format 'dot' not supported for xfsm transducers, use 'prolog'",
                );
                return libc::EXIT_FAILURE;
            }
            if FORMAT == FstTextFormat::PckimmoText {
                error(
                    libc::EXIT_FAILURE,
                    0,
                    "Output format 'pckimmo' not supported for xfsm transducers, use 'prolog'",
                );
                return libc::EXIT_FAILURE;
            }
            if FORMAT == FstTextFormat::AttText {
                error(
                    libc::EXIT_FAILURE,
                    0,
                    "Output format 'att' not supported for xfsm transducers, use 'prolog'",
                );
                return libc::EXIT_FAILURE;
            }
            if USE_NUMBERS {
                error(
                    libc::EXIT_FAILURE,
                    0,
                    "Option '--use-numbers' not supported for xfsm transducers",
                );
                return libc::EXIT_FAILURE;
            }
            if cstr(globals::INPUTFILENAME) == "<stdin>" {
                error(
                    libc::EXIT_FAILURE,
                    0,
                    "Reading from standard input not supported for xfsm transducers,\nuse 'hfst-fst2txt [--input|-i] INFILE' instead",
                );
                return libc::EXIT_FAILURE;
            }
            if cstr(globals::OUTFILENAME) == "<stdout>" {
                error(
                    libc::EXIT_FAILURE,
                    0,
                    "Writing to standard output not supported for xfsm transducers,\nuse 'hfst-fst2txt [--output|-o] OUTFILE' instead",
                );
                return libc::EXIT_FAILURE;
            }
        }

        let retval = process_stream(&mut instream, globals::outfile());

        // C: free(inputfilename); free(outfilename); (the foundation owns these
        // allocations; not freed here).
        retval
    }
}
