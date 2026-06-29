//! Faithful 1:1 port of tools/src/hfst-guessify.cc — the tool for compiling a
//! guesser and model form generator from a morphological analyzer. Drives the
//! hfst-cli foundation (globals, getopt, commandline, program-options,
//! tool-metadata, inc fragments) and the ported hfst::guessify_fst library.

use core::ffi::{c_char, c_int};
use hfst::guessify_fst::{CATEGORY_SYMBOL_PREFIX, guessify_analyzer, store_guesser};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
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

// add tools-specific variables here
static mut COMPILE_GENERATOR: bool = true;
static mut DEFAULT_PENALTY: f32 = 1.0;

// [spec:hfst:def:hfst-guessify.get-float-fn]
// [spec:hfst:sem:hfst-guessify.get-float-fn]
fn get_float(str: &str) -> f32 {
    // C: 'std::istringstream in(str); float f; in >> f; if (in.fail()) return -1;
    // return f;'. Mirror the formatted stream extraction: skip leading
    // whitespace, then consume as many leading characters as form a valid float
    // (trailing characters are ignored). On a failed extraction (no float could
    // be read) return -1.
    let trimmed = str.trim_start();
    // Find the longest leading prefix that parses as a float, mirroring the
    // greedy, character-by-character acceptance of istringstream's '>>' for a
    // float. Trailing characters after the number are ignored.
    let mut best: Option<f32> = None;
    for (i, _) in trimmed.char_indices() {
        if let Ok(f) = trimmed[..=i].parse::<f32>() {
            best = Some(f);
        }
    }
    match best {
        Some(f) => f,
        None => -1.0,
    }
}

// [spec:hfst:def:hfst-guessify.print-usage-fn]
// [spec:hfst:sem:hfst-guessify.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = globals::message_writer();
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            &mut *msg,
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\nCompile a morphological analyzer into a guesser and generator.\n\n",
                program_name
            ),
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        fput(
            &mut *msg,
            "Guesser options:\n  -p, --default-penalty           Give penalty for skipping one\n                                  symbol of input (1.0 by default).\n  -G, --do-not-compile-generator  When compiling the guesser, do\n                                  not compile a model form\n                                  generator.\n",
        );
        fput(&mut *msg, "\n");
        fput(
            &mut *msg,
            &format!(
                "All analyses in the morphological analyzer should have the form:\nw o r d f o r m POS {0}CLASS] X Y Z ...\nwhere POS is the part-of-speech tag, {0}CLASS]\nis an inflectional category marker and X, Y and Z are inflectional\nmarkers. The form of the inflectional category marker is fixed.\nCLASS can be any string, which doesn't contain \"]\".\n",
                CATEGORY_SYMBOL_PREFIX
            ),
        );
        fput(&mut *msg, "\n");
        fput(
            &mut *msg,
            "Using the option -d will reduce the size of the guesser file by\napproximately half, but may substantially increase the load time of\nthe guesser when generating model forms. If you only need to guess\nanalyses of unknown word forms, -d has no effect on load time.\n",
        );
        fput(&mut *msg, "\n");
        fput(
            &mut *msg,
            "If OUTFILE or INFILE is missing or -, standard streams will be used.\n",
        );
        fput(&mut *msg, "\n");
        print_report_bugs();
        fput(&mut *msg, "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-guessify.parse-options-fn]
// [spec:hfst:sem:hfst-guessify.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let default_penalty_name = CString::new("default-penalty").unwrap();
            let do_not_compile_generator_name = CString::new("do-not-compile-generator").unwrap();
            long_options.push(getopt::Option {
                name: default_penalty_name.as_ptr(),
                has_arg: 1, // required_argument
                flag: std::ptr::null_mut(),
                val: 'p' as c_int,
            });
            long_options.push(getopt::Option {
                name: do_not_compile_generator_name.as_ptr(),
                has_arg: 0, // no_argument
                flag: std::ptr::null_mut(),
                val: 'G' as c_int,
            });
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}p:G",
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
            // cases, then unary cases, then the tool's own ('G'/'p'), then the
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
                x if x == 'G' as c_int => {
                    COMPILE_GENERATOR = false;
                    continue;
                }
                x if x == 'p' as c_int => {
                    let optarg = cstr(getopt::OPTARG);
                    DEFAULT_PENALTY = get_float(&optarg);

                    if DEFAULT_PENALTY < 0.0 {
                        error(
                            1,
                            0,
                            &format!("Invalid default penalty {}. Give a positive float.", optarg),
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

// [spec:hfst:def:hfst-guessify.process-stream-fn]
// [spec:hfst:sem:hfst-guessify.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream, out: &mut HfstOutputStream) -> c_int {
    unsafe {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let analyzer = HfstTransducer::new_from_stream(instream);

            verbose_printf(&format!(
                "Compiling guesser from the transducer {}.\n",
                analyzer.get_name()
            ));
            let mut guesser = guessify_analyzer(analyzer, DEFAULT_PENALTY);

            if COMPILE_GENERATOR {
                verbose_printf("Compiling generator and storing guesser and generator.\n");
            } else {
                verbose_printf("Storing guesser.\n");
            }

            store_guesser(&mut guesser, out, COMPILE_GENERATOR);
        }

        instream.close();

        0
    }
}

// [spec:hfst:def:hfst-guessify.main-fn]
// [spec:hfst:sem:hfst-guessify.main-fn]
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

        hfst_set_program_name(&argv0, "0.3", "HfstGuessify");
        let retval = parse_options(argc, argv);
        if retval != EXIT_CONTINUE {
            return retval;
        }

        // close buffers, we use streams
        let input_opened = cstr(globals::INPUTFILENAME) != "<stdin>";

        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            cstr(globals::INPUTFILENAME),
            cstr(globals::OUTFILENAME)
        ));

        // here starts the buffer handling part
        // (the C wraps the ctor in try/catch on HfstException reporting
        // "%s is not a valid transducer file"; the Rust ctor currently panics on
        // a bad file rather than throwing, so the catch arm is not reproduced.)
        let mut instream = if input_opened {
            HfstInputStream::new_filename(&cstr(globals::INPUTFILENAME))
        } else {
            HfstInputStream::new()
        };

        let output_opened = cstr(globals::OUTFILENAME) != "<stdout>";
        // (the C wraps the ctor in try/catch on HfstException reporting
        // "%s cannot be opened for writing."; the Rust ctor currently panics
        // rather than throwing, so the catch arm is not reproduced here.)
        let mut outstream = if output_opened {
            HfstOutputStream::new_filename(
                &cstr(globals::OUTFILENAME),
                ImplementationType::HFST_OLW_TYPE,
                true,
            )
        } else {
            HfstOutputStream::new(ImplementationType::HFST_OLW_TYPE, true)
        };

        process_stream(&mut instream, &mut outstream)
    }
}
