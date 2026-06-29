//! Faithful 1:1 port of tools/src/hfst-guess.cc — the tool for compiling/using
//! a guesser (and generator) to guess analyses/paradigms of unknown words.
//! Drives the hfst-cli foundation (globals, getopt, commandline,
//! program-options, tool-metadata, inc fragments) and the now-available
//! library helper hfst::generate_model_forms.

use core::ffi::{c_char, c_int};
use hfst::generate_model_forms::{
    StringVectorVector, compile_generator_from_guesser, get_alphabet_string_tokenizer, get_guesses,
    get_paradigms, is_guesser, read_model_forms,
};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_symbol_defs::StringVector;
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
use std::io::{BufRead, Write};

// add tools-specific variables here
static mut GENERATE_MODEL_FORMS: bool = false;
static mut MODEL_FORM_FILENAME: String = String::new();
static mut MAX_NUMBER_OF_GUESSES: usize = 5;
static mut MAX_NUMBER_OF_FORMS: usize = 2;
static mut GENERATE_THRESHOLD: f32 = 50.0;

// The String global is reached through 'addr_of_mut!' to avoid the
// edition-2024 'static_mut_refs' hard error.
fn model_form_filename() -> &'static mut String {
    unsafe { &mut *std::ptr::addr_of_mut!(MODEL_FORM_FILENAME) }
}

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

unsafe fn optarg() -> String {
    unsafe { cstr(getopt::OPTARG) }
}

// [spec:hfst:def:hfst-guess.get-size-t-fn]
// [spec:hfst:sem:hfst-guess.get-size-t-fn]
fn get_size_t(str: &str) -> Result<usize, &'static str> {
    // istringstream extraction into a size_t: skip leading whitespace then
    // consume the leading run of decimal digits; failbit (no digits) -> "fail".
    let trimmed = str.trim_start();
    let digits: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();

    if digits.is_empty() {
        return Err("fail");
    }

    // Mirror the silent saturating behaviour of stream extraction on overflow.
    Ok(digits.parse::<usize>().unwrap_or(usize::MAX))
}

// [spec:hfst:def:hfst-guess.get-float-fn]
// [spec:hfst:sem:hfst-guess.get-float-fn]
fn get_float(str: &str) -> f32 {
    // istringstream extraction into a float: skip leading whitespace, then
    // consume the longest leading run that forms a valid float. Failure -> -1.
    let trimmed = str.trim_start();

    // Find the longest valid float prefix by shrinking from the full string.
    let mut end = trimmed.len();
    while end > 0 {
        if let Ok(value) = trimmed[..end].parse::<f32>() {
            return value;
        }
        end -= 1;
    }

    -1.0
}

// [spec:hfst:def:hfst-guess.print-usage-fn]
// [spec:hfst:sem:hfst-guess.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = globals::message_writer();
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            &mut *msg,
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\n\
                 Use a guesser (and generator) to guess analyses or inflectional\n\
                 paradigms of unknown words.\n\
                 \n",
                program_name
            ),
        );

        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        fput(
            &mut *msg,
            "Guesser options:\n\
             \u{0020} -f, --model-form-filename       Inflectional information for\n\
             \u{0020}                                 generated model forms is read\n\
             \u{0020}                                 from this file.\n\
             \u{0020} -n, --max-number-of-guesses     Maximal number of analysis\n\
             \u{0020}                                 per word form (5 by default).\n\
             \u{0020} -m  --max-number-of-forms       Maximal number of generated model\n\
             \u{0020}                                 forms per guess (2 by default).\n\
             \u{0020} -g  --generate-threshold        Generate only forms whose weight\n\
             \u{0020}                                 is better than the weight of the\n\
             \u{0020}                                 of the best form plus this threshold.\n\
             \u{0020}                                 (50 by default).",
        );
        fput(&mut *msg, "\n");
        fput(
            &mut *msg,
            "The guesser and generator should be constructed using the tool\n\
             hfst-guessify, which can compile a guesser and generator from a\n\
             morphological analyzer. hfst-guessify packages the guesser and\n\
             generator in the same fst-file.\n",
        );
        fput(&mut *msg, "\n");
        fput(
            &mut *msg,
            "If option -f is used, but a generator has not been compiled\n\
             with the guesser, a generator will be compiled, which will\n\
             increase load time.\n",
        );
        fput(&mut *msg, "\n");
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

// [spec:hfst:def:hfst-guess.parse-options-fn]
// [spec:hfst:sem:hfst-guess.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let opt_generate_threshold = CString::new("generate-threshold").unwrap();
            let opt_model_form_filename = CString::new("model-form-filename").unwrap();
            let opt_max_number_of_guesses = CString::new("max-number-of-guesses").unwrap();
            let opt_max_number_of_forms = CString::new("max-number-of-forms").unwrap();
            long_options.push(getopt::Option {
                name: opt_generate_threshold.as_ptr(),
                has_arg: getopt::REQUIRED_ARGUMENT,
                flag: std::ptr::null_mut(),
                val: 'g' as c_int,
            });
            long_options.push(getopt::Option {
                name: opt_model_form_filename.as_ptr(),
                has_arg: getopt::REQUIRED_ARGUMENT,
                flag: std::ptr::null_mut(),
                val: 'f' as c_int,
            });
            long_options.push(getopt::Option {
                name: opt_max_number_of_guesses.as_ptr(),
                has_arg: getopt::REQUIRED_ARGUMENT,
                flag: std::ptr::null_mut(),
                val: 'n' as c_int,
            });
            long_options.push(getopt::Option {
                name: opt_max_number_of_forms.as_ptr(),
                has_arg: getopt::REQUIRED_ARGUMENT,
                flag: std::ptr::null_mut(),
                val: 'm' as c_int,
            });
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}{}",
                HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT, "f:m:n:g:"
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
                'f' => {
                    GENERATE_MODEL_FORMS = true;
                    *model_form_filename() = optarg();
                    continue;
                }
                'g' => {
                    GENERATE_THRESHOLD = get_float(&optarg());
                    if GENERATE_THRESHOLD < 0.0 {
                        error(
                            1,
                            0,
                            &format!(
                                "Invalid generate threshold {}. Give a positive float.",
                                optarg()
                            ),
                        );
                    }
                    continue;
                }
                'n' => {
                    match get_size_t(&optarg()) {
                        Ok(v) => MAX_NUMBER_OF_GUESSES = v,
                        Err(_msg) => {
                            error(
                                1,
                                0,
                                &format!(
                                    "Invalid maximal number of guesses {}. Give a positive int.",
                                    optarg()
                                ),
                            );
                        }
                    }
                    continue;
                }
                'm' => {
                    match get_size_t(&optarg()) {
                        Ok(v) => MAX_NUMBER_OF_FORMS = v,
                        Err(_msg) => {
                            error(
                                1,
                                0,
                                &format!(
                                    "Invalid maximal number of generated forms {}. Give a positive int.",
                                    optarg()
                                ),
                            );
                        }
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

// 'std::ostream << StringVector' concatenates the symbols with no separator
// (generate_model_forms.cc 'operator<<').
fn string_vector_to_string(v: &StringVector) -> String {
    v.concat()
}

// [spec:hfst:def:hfst-guess.main-fn]
// [spec:hfst:sem:hfst-guess.main-fn]
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

        hfst_set_program_name(&argv0, "0.3", "HfstGuess");
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
        // "<inputfilename> is not a valid transducer file"; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)
        let mut instream = if input_opened {
            HfstInputStream::new_filename(&cstr(globals::INPUTFILENAME))
        } else {
            HfstInputStream::new()
        };

        // The C opens an ofstream on outfilename or uses std::cout; the
        // foundation's 'output_writer()' already maps OUTFILE-or-stdout to a
        // std::io::Write.
        let mut out = match globals::output_writer() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("hfst-guess: cannot open output: {e}");
                return 1;
            }
        };

        // (the C wraps the HfstTransducer ctor in try/catch reporting "Error
        // when reading guesser from file <inputfilename>"; the Rust ctor panics
        // rather than throwing, so that catch arm is not reproduced here.)
        let mut guesser = HfstTransducer::new_from_stream(&mut instream);

        if !is_guesser(&guesser) {
            error(
                1,
                0,
                &format!(
                    "The transducer in {} is not a guesser.",
                    cstr(globals::INPUTFILENAME)
                ),
            );
            return 1;
        }

        let mut generator: Option<HfstTransducer> = None;

        if GENERATE_MODEL_FORMS {
            if !instream.is_good() {
                verbose_printf(&format!(
                    "No generator found in {}. Compiling generator from guesser.\n",
                    cstr(globals::INPUTFILENAME)
                ));

                generator = Some(compile_generator_from_guesser(&guesser));
            } else {
                generator = Some(HfstTransducer::new_from_stream(&mut instream));
            }
        }

        let mut tokenizer = get_alphabet_string_tokenizer(&mut guesser);

        let mut model_forms: StringVectorVector = StringVectorVector::new();

        if GENERATE_MODEL_FORMS {
            verbose_printf(&format!(
                "Reading inflectional information for model forms\nfrom {}.\n",
                model_form_filename()
            ));

            match read_model_forms(model_form_filename().as_str(), &mut tokenizer) {
                Ok(mf) => model_forms = mf,
                Err(e) => {
                    eprintln!("Invalid model form line in model form file:");
                    eprintln!("{}", e.line);
                    return 1;
                }
            }
        }

        let stdin = std::io::stdin();
        for line_result in stdin.lock().lines() {
            // std::getline returns the line without the trailing newline.
            let line = match line_result {
                Ok(l) => l,
                Err(_) => break,
            };

            let mut guesses =
                get_guesses(&line, &mut guesser, MAX_NUMBER_OF_GUESSES, &mut tokenizer);

            if GENERATE_MODEL_FORMS {
                // make scan-build happy, this should not happen
                let gen_tr = generator
                    .as_mut()
                    .unwrap_or_else(|| panic!("Error: generator has a NULL value."));
                let paradigms = get_paradigms(
                    &line,
                    &guesses,
                    gen_tr,
                    &model_forms,
                    MAX_NUMBER_OF_FORMS,
                    GENERATE_THRESHOLD,
                );

                for it in &paradigms {
                    fput(&mut *out, &format!("{}\n", string_vector_to_string(it)));
                }
            } else {
                for it in guesses.iter_mut() {
                    it.reverse();

                    fput(
                        &mut *out,
                        &format!("{}\t{}\n", line, string_vector_to_string(it)),
                    );
                }
            }
            fput(&mut *out, "\n");
        }

        // The C deletes/flushes the output ofstream when it is a file; flush the
        // std::io::Write to mirror it.
        let _ = out.flush();

        // free(inputfilename); free(outfilename); delete guesser; delete
        // generator — handled by the foundation/Drop in Rust.
        drop(guesser);
        drop(generator);

        0
    }
}
