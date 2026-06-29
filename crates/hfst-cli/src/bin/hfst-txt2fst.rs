//! Faithful 1:1 port of tools/src/hfst-txt2fst.cc — the transducer text
//! compiling command-line tool. Drives the hfst-cli foundation (globals,
//! getopt, commandline, program-options, tool-metadata, inc fragments).
//!
//! Convert AT&T or prolog format into a binary transducer.

use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, extend_options_getenv, hfst_error, hfst_parse_format_name,
    hfst_set_program_name, hfst_warning, print_more_info, print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT, hfst_getopt_common_long,
    hfst_getopt_unary_long, print_common_program_options, print_common_unary_program_options,
};
use hfst_cli::hfst_tool_metadata::{hfst_set_formula, hfst_set_name};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use libc::{c_char, c_int};
use std::ffi::{CStr, CString};
use std::io::BufRead;

// ---------------------------------------------------------------------------
// Tool-global state. C: file-scope static variables.
// ---------------------------------------------------------------------------

// add tools-specific variables here
static mut OUTPUT_FORMAT: ImplementationType = ImplementationType::UNSPECIFIED_TYPE;
static mut READ_PROLOG_FORMAT: bool = false;
// whether numbers are used instead of symbol names
static mut USE_NUMBERS: bool = false; // not used
// printname for epsilon
static mut EPSILONNAME: *mut c_char = std::ptr::null_mut();

// check if there are epsilon cycles with a negative weight
static mut CHECK_NEGATIVE_EPSILON_CYCLES: bool = false;
static mut WARN_NEGATIVE_WEIGHTS: bool = true;
static mut WARNINGS_ARE_ERRORS: bool = false;

static mut DISJUNCT_MULTIPLE_TRANSDUCERS: bool = false;

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

// Equivalent of the C++ 'feof(inputfile)': no bytes remain on the buffered
// reader (the readers' own EndOfStreamException paths panic_any internally).
fn is_eof(input: &mut dyn BufRead) -> bool {
    match input.fill_buf() {
        Ok(b) => b.is_empty(),
        Err(_) => true,
    }
}

// [spec:hfst:def:hfst-txt2fst.print-usage-fn]
// [spec:hfst:sem:hfst-txt2fst.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        let mut msg = globals::message_writer();
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            &mut *msg,
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\nConvert AT&T or prolog format into a binary transducer\n\n",
                program_name
            ),
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        fput(
            &mut *msg,
            "Text and format options:\n  -f, --format=FMT    Write result using FMT as backend format\n  -e, --epsilon=EPS   Interpret string EPS as epsilon in att format\n  -p, --prolog        Read prolog format instead of att\n",
        );
        fput(
            &mut *msg,
            "Other options:\n  -C, --check-negative-epsilon-cycles  Issue a warning if there are epsilon cycles\n                                       with a negative weight in the transducer\n  -j, --disjunct                       Disjunct transducers\n",
        );
        fput(&mut *msg, "\n");
        fput(
            &mut *msg,
            "If OUTFILE or INFILE is missing or -, standard streams will be used.\nIf FMT is not given, OpenFst's tropical format will be used.\nThe possible values for FMT are { foma, openfst-tropical, openfst-log,\nsfst, optimized-lookup-weighted, optimized-lookup-unweighted }.\nIf EPS is not given, @0@ will be used.\n\nSpace in transition symbols must be escaped as '@_SPACE_@' when using\natt format.\n",
        );
        fput(&mut *msg, "\n");
        print_report_bugs();
        fput(&mut *msg, "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-txt2fst.parse-options-fn]
// [spec:hfst:sem:hfst-txt2fst.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let epsilon_name = CString::new("epsilon").unwrap();
            let number_name = CString::new("number").unwrap();
            let format_name = CString::new("format").unwrap();
            let prolog_name = CString::new("prolog").unwrap();
            let disjunct_name = CString::new("disjunct").unwrap();
            let check_neg_name = CString::new("check-negative-epsilon-cycles").unwrap();
            let wstuff_name = CString::new("Wstuff").unwrap();
            long_options.push(getopt::Option {
                name: epsilon_name.as_ptr(),
                has_arg: 1, // required_argument
                flag: std::ptr::null_mut(),
                val: 'e' as c_int,
            });
            long_options.push(getopt::Option {
                name: number_name.as_ptr(),
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
                name: prolog_name.as_ptr(),
                has_arg: 0, // no_argument
                flag: std::ptr::null_mut(),
                val: 'p' as c_int,
            });
            long_options.push(getopt::Option {
                name: disjunct_name.as_ptr(),
                has_arg: 0, // no_argument
                flag: std::ptr::null_mut(),
                val: 'j' as c_int,
            });
            long_options.push(getopt::Option {
                name: check_neg_name.as_ptr(),
                has_arg: 0, // no_argument
                flag: std::ptr::null_mut(),
                val: 'C' as c_int,
            });
            long_options.push(getopt::Option {
                name: wstuff_name.as_ptr(),
                has_arg: 1, // required_argument
                flag: std::ptr::null_mut(),
                val: 'W' as c_int,
            });
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}e:nf:pjC",
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
            match c as u8 as char {
                'e' => {
                    EPSILONNAME = hfst_cli::hfst_commandline::hfst_strdup(getopt::OPTARG);
                    continue;
                }
                'j' => {
                    DISJUNCT_MULTIPLE_TRANSDUCERS = true;
                    continue;
                }
                'n' => {
                    USE_NUMBERS = true;
                    continue;
                }
                'p' => {
                    READ_PROLOG_FORMAT = true;
                    continue;
                }
                'f' => {
                    OUTPUT_FORMAT = hfst_parse_format_name(&cstr(getopt::OPTARG));
                    continue;
                }
                'C' => {
                    CHECK_NEGATIVE_EPSILON_CYCLES = true;
                    continue;
                }
                'W' => {
                    let optarg = cstr(getopt::OPTARG);
                    if optarg == "error" {
                        WARNINGS_ARE_ERRORS = true;
                    } else if optarg == "no-error" {
                        WARNINGS_ARE_ERRORS = false;
                    } else if optarg == "negative-weights" {
                        WARN_NEGATIVE_WEIGHTS = true;
                    } else if optarg == "no-negative-weights" {
                        WARN_NEGATIVE_WEIGHTS = false;
                    } else {
                        hfst_error(
                            libc::EXIT_FAILURE,
                            0,
                            &format!("Unrecognised warning switch -W{}", optarg),
                        );
                        return libc::EXIT_FAILURE;
                    }
                    continue;
                }
                _ => {}
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(argc, argv);
        if EPSILONNAME.is_null() {
            let eps = CString::new("@0@").unwrap();
            EPSILONNAME = hfst_cli::hfst_commandline::hfst_strdup(eps.as_ptr());
            verbose_printf(&format!(
                "Using default epsilon representation {}\n",
                cstr(EPSILONNAME)
            ));
        }
        if OUTPUT_FORMAT == ImplementationType::UNSPECIFIED_TYPE {
            OUTPUT_FORMAT = ImplementationType::TROPICAL_OPENFST_TYPE;
            verbose_printf("Using default output format OpenFst with tropical weight class\n");
        }

        if OUTPUT_FORMAT == ImplementationType::XFSM_TYPE
            && READ_PROLOG_FORMAT
            && CHECK_NEGATIVE_EPSILON_CYCLES
        {
            hfst_error(
                libc::EXIT_FAILURE,
                0,
                "Error: checking negative epsilon cycles not supported when reading in prolog format\nand outputting in xfsm format.\n",
            );
            return libc::EXIT_FAILURE;
        }

        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-txt2fst.process-stream-fn]
// [spec:hfst:sem:hfst-txt2fst.process-stream-fn]
unsafe fn process_stream(outstream: &mut HfstOutputStream, input: &mut dyn BufRead) -> c_int {
    unsafe {
        let mut transducer_n: usize = 0;
        let mut linecount: u32 = 0;

        let inputfilename = cstr(globals::INPUTFILENAME);
        let epsilonname = cstr(EPSILONNAME);

        // outstream.open();
        while !is_eof(input) {
            transducer_n += 1;
            if transducer_n < 2 {
                verbose_printf("Reading transducer table...\n");
            } else {
                verbose_printf(&format!("Reading transducer table {}...\n", transducer_n));
            }
            if READ_PROLOG_FORMAT {
                if OUTPUT_FORMAT == ImplementationType::XFSM_TYPE {
                    // C: catches HfstException around prolog_file_to_xfsm_transducer;
                    // the Rust foundation panics rather than throwing, so the catch
                    // arm is not reproduced here.
                    let ifn = CString::new(inputfilename.clone()).unwrap_or_default();
                    let t = HfstTransducer::prolog_file_to_xfsm_transducer(ifn.as_ptr());
                    outstream.redirect(&mut *t);
                    drop(Box::from_raw(t));
                    outstream.flush();
                    break;
                }

                // C: catches NotValidPrologFormatException; the Rust readers
                // panic_any rather than throw, so the catch arm is not reproduced.
                let fsm = HfstBasicTransducer::read_in_prolog_format_file(input, &mut linecount);

                if CHECK_NEGATIVE_EPSILON_CYCLES {
                    verbose_printf(
                        "Checking if the transducer has epsilon cycles with a negative weight...\n",
                    );
                    if fsm.has_negative_epsilon_cycles() {
                        if !globals::SILENT {
                            hfst_warning(
                                0,
                                0,
                                "Transducer has epsilon cycles with a negative weight.\n",
                            );
                        }
                    } else {
                        verbose_printf("No epsilon cycles with a negative weight detected...\n");
                    }
                }

                let mut t = HfstTransducer::new_from_basic(&fsm, OUTPUT_FORMAT);
                hfst_set_name(&mut t, &inputfilename, "text");
                hfst_set_formula(&mut t, &inputfilename, "T");
                outstream.redirect(&mut t);
            } else if DISJUNCT_MULTIPLE_TRANSDUCERS {
                let mut transducers: Vec<HfstTransducer> = Vec::new();
                // C: catches NotValidAttFormatException and prints an error; the
                // Rust readers panic_any rather than throw, so the catch arm is
                // not reproduced here.
                while !is_eof(input) {
                    // C: HfstTransducer(inputfile, type, epsilon, warn) — read the
                    // basic graph from the AT&T file then build the typed transducer.
                    let net = HfstBasicTransducer::read_in_att_format_file(
                        input,
                        &epsilonname,
                        &mut linecount,
                        WARN_NEGATIVE_WEIGHTS,
                    );
                    let t = HfstTransducer::new_from_basic(&net, OUTPUT_FORMAT);
                    transducers.push(t);
                }
                let mut joined = HfstTransducer::new_type(OUTPUT_FORMAT);
                for it in transducers.iter() {
                    joined.disjunct(it, true);
                }
                // joined.remove_epsilons(); // remove epsilons from the unioned
                // transducers
                outstream.redirect(&mut joined);
            } else {
                // C: catches NotValidAttFormatException; the Rust readers panic_any
                // rather than throw, so the catch arm is not reproduced here.
                // C: HfstTransducer(inputfile, type, epsilon, linecount, warn).
                let net = HfstBasicTransducer::read_in_att_format_file(
                    input,
                    &epsilonname,
                    &mut linecount,
                    WARN_NEGATIVE_WEIGHTS,
                );
                let mut t = HfstTransducer::new_from_basic(&net, OUTPUT_FORMAT);
                hfst_set_name(&mut t, &inputfilename, "text");
                hfst_set_formula(&mut t, &inputfilename, "T");
                if CHECK_NEGATIVE_EPSILON_CYCLES {
                    verbose_printf(
                        "Checking if the transducer has epsilon cycles with a negative weight...\n",
                    );
                    let fsm = HfstBasicTransducer::new_from_transducer(&t);
                    if fsm.has_negative_epsilon_cycles() {
                        if !globals::SILENT {
                            hfst_warning(
                                0,
                                0,
                                "Transducer has epsilon cycles with a negative weight.\n",
                            );
                        }
                    } else {
                        verbose_printf("No epsilon cycles with a negative weight detected...\n");
                    }
                }
                outstream.redirect(&mut t);
            }
        }
        outstream.close();
        libc::EXIT_SUCCESS
    }
}

// [spec:hfst:def:hfst-txt2fst.main-fn]
// [spec:hfst:sem:hfst-txt2fst.main-fn]
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

        hfst_set_program_name(&argv0, "0.1", "HfstTxt2Fst");
        let retval = parse_options(argc, argv);

        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let output_opened = cstr(globals::OUTFILENAME) != "<stdout>";
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            cstr(globals::INPUTFILENAME),
            cstr(globals::OUTFILENAME)
        ));
        match OUTPUT_FORMAT {
            ImplementationType::SFST_TYPE => {
                verbose_printf("Using SFST as output handler\n");
            }
            ImplementationType::TROPICAL_OPENFST_TYPE => {
                verbose_printf("Using OpenFst's tropical weights as output\n");
            }
            ImplementationType::LOG_OPENFST_TYPE => {
                verbose_printf("Using OpenFst's log weight output\n");
            }
            ImplementationType::FOMA_TYPE => {
                verbose_printf("Using foma as output handler\n");
            }
            ImplementationType::XFSM_TYPE => {
                verbose_printf("Using xfsm as output handler\n");
            }
            ImplementationType::HFST_OL_TYPE => {
                verbose_printf("Using optimized lookup output\n");
            }
            ImplementationType::HFST_OLW_TYPE => {
                verbose_printf("Using optimized lookup weighted output\n");
            }
            _ => {
                hfst_error(
                    libc::EXIT_FAILURE,
                    0,
                    "Unknown format cannot be used as output\n",
                );
                return libc::EXIT_FAILURE;
            }
        }

        if OUTPUT_FORMAT == ImplementationType::XFSM_TYPE {
            if cstr(globals::OUTFILENAME) == "<stdout>" {
                hfst_error(
                    libc::EXIT_FAILURE,
                    0,
                    "Writing to standard output not supported for xfsm transducers,\nuse 'hfst-txt2fst [--output|-o] OUTFILE' instead",
                );
                return libc::EXIT_FAILURE;
            }
            if !READ_PROLOG_FORMAT {
                hfst_error(
                    libc::EXIT_FAILURE,
                    0,
                    "Writing in att format not supported for xfsm transducers,\nuse '--prolog' instead",
                );
                return libc::EXIT_FAILURE;
            }
            if cstr(globals::INPUTFILENAME) == "<stdin>" {
                hfst_error(
                    libc::EXIT_FAILURE,
                    0,
                    "Reading prolog format from standard input not supported for xfsm transducers,\nuse 'hfst-txt2fst [--input|-i] INFILE' instead",
                );
                return libc::EXIT_FAILURE;
            } else {
            }
        }

        // here starts the buffer handling part
        let mut outstream = if output_opened {
            HfstOutputStream::new_filename(&cstr(globals::OUTFILENAME), OUTPUT_FORMAT, true)
        } else {
            HfstOutputStream::new(OUTPUT_FORMAT, true)
        };
        let mut input = match globals::input_reader() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("hfst-txt2fst: cannot open input: {e}");
                return libc::EXIT_FAILURE;
            }
        };
        process_stream(&mut outstream, &mut *input);
        libc::free(globals::INPUTFILENAME as *mut libc::c_void);
        libc::free(globals::OUTFILENAME as *mut libc::c_void);
        libc::EXIT_SUCCESS
    }
}
