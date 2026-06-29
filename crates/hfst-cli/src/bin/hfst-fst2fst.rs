#![allow(static_mut_refs)]
//! Faithful 1:1 port of tools/src/hfst-fst2fst.cc — the format conversion
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments). A unary tool:
//! it reads one input stream and converts each transducer to another binary
//! implementation format.

use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_getenv, hfst_parse_format_name, hfst_set_program_name,
    hfst_strformat, print_more_info, print_report_bugs, verbose_printf, warning,
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

// tool-specific variables
static mut OUTPUT_TYPE: ImplementationType = ImplementationType::UNSPECIFIED_TYPE;
static mut HFST_FORMAT: bool = true;
static mut OPTIONS: String = String::new();

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

// [spec:hfst:def:hfst-fst2fst.set-output-type-fn]
// [spec:hfst:sem:hfst-fst2fst.set-output-type-fn]
unsafe fn set_output_type(type_: ImplementationType) {
    unsafe {
        if OUTPUT_TYPE != ImplementationType::UNSPECIFIED_TYPE {
            error(libc::EXIT_FAILURE, 0, "Output type defined several times.");
        }
        OUTPUT_TYPE = type_;
    }
}

// [spec:hfst:def:hfst-fst2fst.print-usage-fn]
// [spec:hfst:sem:hfst-fst2fst.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = globals::message_writer();
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            &mut *msg,
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\nConvert transducers between binary formats\n\n",
                program_name
            ),
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        fput(
            &mut *msg,
            "Conversion options:\n\
             \u{20}\u{20}-f, --format=FMT                  Write result in FMT format\n\
             \u{20}\u{20}-b, --use-backend-format          Write result in implementation format, without any HFST wrappers\n\
             \u{20}\u{20}-S, --sfst                        Write output in (HFST's) SFST implementation\n\
             \u{20}\u{20}-F, --foma                        Write output in (HFST's) foma implementation\n\
             \u{20}\u{20}-x, --xfsm                        Write output in native xfsm format\n\
             \u{20}\u{20}-t, --openfst-tropical            Write output in (HFST's) tropical weight (OpenFST) implementation\n\
             \u{20}\u{20}-l, --openfst-log                 Write output in (HFST's) log weight (OpenFST) implementation\n\
             \u{20}\u{20}-O, --optimized-lookup-unweighted Write output in the HFST optimized-lookup implementation\n\
             \u{20}\u{20}-w, --optimized-lookup-weighted   Write output in optimized-lookup (weighted) implementation\n\
             \u{20}\u{20}-Q  --quick                       When converting to optimized-lookup, don't try hard to compress\n",
        );
        fput(&mut *msg, "\n");
        print_common_unary_program_parameter_instructions(&mut *msg);
        fput(
            &mut *msg,
            "FMT must be name of a format usable by libhfst, i.e. one of the following:\n\
             { foma, openfst-tropical, openfst-log, sfst, xfsm\n\
             \u{20}\u{20}optimized-lookup-weighted, optimized-lookup-unweighted }.\n\
             Note that xfsm format is always written in native format without HFST wrappers.\n",
        );
        fput(&mut *msg, "\n");
        print_report_bugs();
        fput(&mut *msg, "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-fst2fst.parse-options-fn]
// [spec:hfst:sem:hfst-fst2fst.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let opt_names = [
                CString::new("use-backend-format").unwrap(),
                CString::new("format").unwrap(),
                CString::new("sfst").unwrap(),
                CString::new("foma").unwrap(),
                CString::new("xfsm").unwrap(),
                CString::new("openfst-tropical").unwrap(),
                CString::new("openfst-log").unwrap(),
                CString::new("optimized-lookup-unweighted").unwrap(),
                CString::new("optimized-lookup-weighted").unwrap(),
                CString::new("quick").unwrap(),
            ];
            long_options.push(getopt::Option {
                name: opt_names[0].as_ptr(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: b'b' as c_int,
            });
            long_options.push(getopt::Option {
                name: opt_names[1].as_ptr(),
                has_arg: 1,
                flag: std::ptr::null_mut(),
                val: b'f' as c_int,
            });
            long_options.push(getopt::Option {
                name: opt_names[2].as_ptr(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: b'S' as c_int,
            });
            long_options.push(getopt::Option {
                name: opt_names[3].as_ptr(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: b'F' as c_int,
            });
            long_options.push(getopt::Option {
                name: opt_names[4].as_ptr(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: b'x' as c_int,
            });
            long_options.push(getopt::Option {
                name: opt_names[5].as_ptr(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: b't' as c_int,
            });
            long_options.push(getopt::Option {
                name: opt_names[6].as_ptr(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: b'l' as c_int,
            });
            long_options.push(getopt::Option {
                name: opt_names[7].as_ptr(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: b'O' as c_int,
            });
            long_options.push(getopt::Option {
                name: opt_names[8].as_ptr(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: b'w' as c_int,
            });
            long_options.push(getopt::Option {
                name: opt_names[9].as_ptr(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: b'Q' as c_int,
            });
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}{}",
                HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT, "SFtlOwQf:bx"
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
            // cases, then unary cases, then the tool's own cases, then the
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
            // add tool-specific cases here
            let ch = c as u8;
            match ch {
                b'f' => {
                    set_output_type(hfst_parse_format_name(&cstr(getopt::OPTARG)));
                    // HAVE_XFSM is not defined in this build: reject xfsm output.
                    if OUTPUT_TYPE == ImplementationType::XFSM_TYPE {
                        error(libc::EXIT_FAILURE, 0, "xfsm back-end is not available");
                    }
                    continue;
                }
                b'b' => {
                    HFST_FORMAT = false;
                    continue;
                }
                b'S' => {
                    set_output_type(ImplementationType::SFST_TYPE);
                    continue;
                }
                b'F' => {
                    set_output_type(ImplementationType::FOMA_TYPE);
                    continue;
                }
                b'x' => {
                    // HAVE_XFSM is not defined in this build.
                    error(libc::EXIT_FAILURE, 0, "xfsm back-end is not available");
                    continue;
                }
                b't' => {
                    set_output_type(ImplementationType::TROPICAL_OPENFST_TYPE);
                    continue;
                }
                b'l' => {
                    set_output_type(ImplementationType::LOG_OPENFST_TYPE);
                    continue;
                }
                b'O' => {
                    set_output_type(ImplementationType::HFST_OL_TYPE);
                    continue;
                }
                b'w' => {
                    set_output_type(ImplementationType::HFST_OLW_TYPE);
                    continue;
                }
                b'Q' => {
                    OPTIONS = "quick".to_string();
                    continue;
                }
                _ => {}
            }
            return handle_error_case(c);
        }

        if OUTPUT_TYPE == ImplementationType::UNSPECIFIED_TYPE {
            error(
                libc::EXIT_FAILURE,
                0,
                "You must specify an output type (one of -S, -F, -t, -x, -l, -O, or -w)",
            );
        }

        check_common_params();
        check_unary_params(argc, argv);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-fst2fst.process-stream-fn]
// [spec:hfst:sem:hfst-fst2fst.process-stream-fn]
unsafe fn process_stream(
    instream: &mut HfstInputStream,
    outstream: &mut HfstOutputStream,
) -> c_int {
    unsafe {
        if instream.get_type() == ImplementationType::FOMA_TYPE
            && !instream.is_hfst_header_included()
        {
            if !globals::SILENT {
                warning(
                    0,
                    0,
                    "converting native foma transducer: \
                     inversion may be needed for hfst-lookup to work as expected \
                     (hfst-flookup works as foma's flookup)\n",
                );
            }
        }

        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let mut orig = HfstTransducer::new_from_stream(instream);

            let inputname = hfst_get_name(&orig, &cstr(globals::INPUTFILENAME));
            if transducer_n == 1 {
                verbose_printf(&format!("Converting {}...\n", inputname));
            } else {
                verbose_printf(&format!("Converting {}...{}\n", inputname, transducer_n));
            }
            // C wraps the conversion in try/catch on HfstException; the Rust
            // conversion currently panics rather than throwing, so the catch arm
            // is not reproduced here.
            orig.convert(OUTPUT_TYPE, OPTIONS.clone());
            // C: hfst_set_name(orig, orig, "convert"); the dest and src are the
            // same object, which Rust cannot alias mut+const, so the read side is
            // taken from a copy (name/formula are unchanged by the copy).
            let src = orig.clone();
            hfst_set_name_unary(&mut orig, &src, "convert");
            hfst_set_formula_unary(&mut orig, &src, "Id");
            outstream.redirect(&mut orig);
        }
        outstream.flush(); // needed for xfsm transducers whose writing is delayed
        instream.close();
        outstream.close();
        libc::EXIT_SUCCESS
    }
}

// [spec:hfst:def:hfst-fst2fst.main-fn]
// [spec:hfst:sem:hfst-fst2fst.main-fn]
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

        hfst_set_program_name(&argv0, "0.1", "HfstFst2Fst");
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
        if HFST_FORMAT && (OUTPUT_TYPE != ImplementationType::XFSM_TYPE) {
            verbose_printf(&format!(
                "Writing {} format transducers with HFST3 headers\n",
                hfst_strformat(OUTPUT_TYPE)
            ));
        } else {
            verbose_printf(&format!(
                "Writing {} format transducers without HFST specific headers\n",
                hfst_strformat(OUTPUT_TYPE)
            ));
        }

        if OUTPUT_TYPE == ImplementationType::XFSM_TYPE {
            if cstr(globals::OUTFILENAME) == "<stdout>" {
                error(
                    libc::EXIT_FAILURE,
                    0,
                    "Writing to standard output not supported for xfsm transducers,\n\
                     use 'hfst-fst2fst [--output|-o] OUTFILE' instead",
                );
                return libc::EXIT_FAILURE;
            }
        }

        // here starts the buffer handling part
        // (the C wraps the ctor in try/catch on FileIsInGZFormatException,
        // ImplementationTypeNotAvailableException and HfstException; the Rust
        // ctor currently panics rather than throwing, so the catch arms are not
        // reproduced here.)
        let mut instream = if input_opened {
            HfstInputStream::new_filename(&cstr(globals::INPUTFILENAME))
        } else {
            HfstInputStream::new()
        };

        let mut outstream = if output_opened {
            HfstOutputStream::new_filename(&cstr(globals::OUTFILENAME), OUTPUT_TYPE, HFST_FORMAT)
        } else {
            HfstOutputStream::new(OUTPUT_TYPE, HFST_FORMAT)
        };

        process_stream(&mut instream, &mut outstream)
    }
}
