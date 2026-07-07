//! Faithful 1:1 port of tools/src/hfst-guess.cc — the tool for compiling/using
//! a guesser (and generator) to guess analyses/paradigms of unknown words.
//! Drives the hfst-cli foundation (getopt, commandline, program-options,
//! tool-metadata, inc fragments) and the now-available library helper
//! hfst::generate_model_forms.
//!
//! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
//! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
//! `parse_options` and threaded into the processing functions. There are no
//! `static mut` globals and no `unsafe`.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, extend_options_from_env, hfst_set_program_name, verbose_print,
};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options,
};
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use hfst::generate_model_forms::{
    StringVectorVector, compile_generator_from_guesser, get_alphabet_string_tokenizer, get_guesses,
    get_paradigms, is_guesser, read_model_forms,
};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_symbol_defs::StringVector;
use hfst::hfst_transducer::HfstTransducer;
use std::io::{BufRead, Write};

/// hfst-guess's own options (the former tool-specific `static mut`s).
struct Options {
    generate_model_forms: bool,
    model_form_filename: String,
    max_number_of_guesses: usize,
    max_number_of_forms: usize,
    generate_threshold: f32,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            generate_model_forms: false,
            model_form_filename: String::new(),
            max_number_of_guesses: 5,
            max_number_of_forms: 2,
            generate_threshold: 50.0,
        }
    }
}

// [spec:hfst:def:hfst-guess.get-size-t-fn]
// [spec:hfst:sem:hfst-guess.get-size-t-fn]
fn parse_size(str: &str) -> Result<usize, &'static str> {
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
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\n\
         Use a guesser (and generator) to guess analyses or inflectional\n\
         paradigms of unknown words.\n\
         \n",
        common.program_name
    );

    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
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
         \u{0020}                                 (50 by default)."
    );
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "The guesser and generator should be constructed using the tool\n\
         hfst-guessify, which can compile a guesser and generator from a\n\
         morphological analyzer. hfst-guessify packages the guesser and\n\
         generator in the same fst-file.\n"
    );
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "If option -f is used, but a generator has not been compiled\n\
         with the guesser, a generator will be compiled, which will\n\
         increase load time.\n"
    );
    let _ = write!(msg, "\n");
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "If OUTFILE or INFILE is missing or -, standard streams will be used.\n"
    );
    let _ = write!(msg, "\n");
}

// [spec:hfst:def:hfst-guess.parse-options-fn]
// [spec:hfst:sem:hfst-guess.parse-options-fn]
//
// Parse argv into the shared + tool options; `Err(code)` is an exit code the
// caller should return (the former EXIT_CONTINUE sentinel is now `Ok`).
fn parse_options(
    mut common: CommonOptions,
    args: &mut Vec<String>,
) -> Result<(CommonOptions, Options), i32> {
    let mut options = Options::default();
    let mut opt = Getopt::new();
    extend_options_from_env(args);
    loop {
        let mut long_options: Vec<getopt::GetOpt> = Vec::new();
        long_options.extend(hfst_getopt_common_long());
        long_options.extend(hfst_getopt_unary_long());
        // add tool-specific options here
        long_options.push(getopt::GetOpt {
            name: "generate-threshold",
            has_arg: getopt::REQUIRED_ARGUMENT,
            val: 'g' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "model-form-filename",
            has_arg: getopt::REQUIRED_ARGUMENT,
            val: 'f' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "max-number-of-guesses",
            has_arg: getopt::REQUIRED_ARGUMENT,
            val: 'n' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "max-number-of-forms",
            has_arg: getopt::REQUIRED_ARGUMENT,
            val: 'm' as i32,
        });
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: common
        // cases, then unary cases, then the tool's own, then the terminal
        // error arm.
        match handle_common_case(&mut common, &opt, c, print_usage) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        match handle_unary_case(&mut common, &opt, c) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        // add tool-specific cases here
        match c as u8 as char {
            'f' => {
                options.generate_model_forms = true;
                options.model_form_filename = opt.optarg();
                continue;
            }
            'g' => {
                options.generate_threshold = get_float(&opt.optarg());
                if options.generate_threshold < 0.0 {
                    error(
                        &common,
                        1,
                        0,
                        &format!(
                            "Invalid generate threshold {}. Give a positive float.",
                            opt.optarg()
                        ),
                    );
                }
                continue;
            }
            'n' => {
                match parse_size(&opt.optarg()) {
                    Ok(v) => options.max_number_of_guesses = v,
                    Err(_msg) => {
                        error(
                            &common,
                            1,
                            0,
                            &format!(
                                "Invalid maximal number of guesses {}. Give a positive int.",
                                opt.optarg()
                            ),
                        );
                    }
                }
                continue;
            }
            'm' => {
                match parse_size(&opt.optarg()) {
                    Ok(v) => options.max_number_of_forms = v,
                    Err(_msg) => {
                        error(
                            &common,
                            1,
                            0,
                            &format!(
                                "Invalid maximal number of generated forms {}. Give a positive int.",
                                opt.optarg()
                            ),
                        );
                    }
                }
                continue;
            }
            _ => {}
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    check_common_params(&mut common);
    check_unary_params(&mut common, &opt, args);
    Ok((common, options))
}

// 'std::ostream << StringVector' concatenates the symbols with no separator
// (generate_model_forms.cc 'operator<<').
fn string_vector_to_string(v: &StringVector) -> String {
    v.concat()
}

// [spec:hfst:def:hfst-guess.main-fn]
// [spec:hfst:sem:hfst-guess.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.3", "HfstGuess");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    // close buffers, we use streams
    let input_opened = common.input_filename != "<stdin>";

    verbose_print(
        &common,
        &format!(
            "Reading from {}, writing to {}\n",
            common.input_filename, common.output_filename
        ),
    );

    // here starts the buffer handling part
    // (the C wraps the ctor in try/catch on HfstException reporting
    // "<inputfilename> is not a valid transducer file"; the Rust ctor
    // currently panics on a bad file rather than throwing, so the catch arm
    // is not reproduced here.)
    let mut instream = match if input_opened {
        HfstInputStream::new_filename(&common.input_filename)
    } else {
        HfstInputStream::new()
    } {
        Ok(s) => s,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return 1;
        }
    };

    // The C opens an ofstream on outfilename or uses std::cout; the
    // foundation's 'output_writer()' already maps OUTFILE-or-stdout to a
    // std::io::Write.
    let mut out = match common.output_writer() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("hfst-guess: cannot open output: {e}");
            return 1;
        }
    };

    // (the C wraps the HfstTransducer ctor in try/catch reporting "Error
    // when reading guesser from file <inputfilename>"; the Rust ctor panics
    // rather than throwing, so that catch arm is not reproduced here.)
    // The lookup engine of get_guesses/get_paradigms is pinned to the
    // weighted optimized-lookup backend; any other input converts here at
    // the stream boundary ([dec:hfst:monomorphic-backends]), as the C++
    // guesser lookup path did through its own conversions.
    let mut guesser: HfstTransducer<hfst::transducer::Transducer> =
        match instream.read().and_then(|any| any.into_typed()) {
            Ok(t) => t,
            Err(e) => {
                error(&common, 1, 0, &format!("{e}"));
                return 1;
            }
        };

    if !is_guesser(&guesser) {
        error(
            &common,
            1,
            0,
            &format!(
                "The transducer in {} is not a guesser.",
                common.input_filename
            ),
        );
        return 1;
    }

    let mut generator: Option<HfstTransducer<hfst::transducer::Transducer>> = None;

    if options.generate_model_forms {
        if !instream.is_good() {
            verbose_print(
                &common,
                &format!(
                    "No generator found in {}. Compiling generator from guesser.\n",
                    common.input_filename
                ),
            );

            generator = Some(match compile_generator_from_guesser(&guesser) {
                Ok(g) => g,
                Err(e) => {
                    error(&common, 1, 0, &format!("{e}"));
                    return 1;
                }
            });
        } else {
            generator = Some(match instream.read().and_then(|any| any.into_typed()) {
                Ok(g) => g,
                Err(e) => {
                    error(&common, 1, 0, &format!("{e}"));
                    return 1;
                }
            });
        }
    }

    let mut tokenizer = match get_alphabet_string_tokenizer(&mut guesser) {
        Ok(t) => t,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return 1;
        }
    };

    let mut model_forms: StringVectorVector = StringVectorVector::new();

    if options.generate_model_forms {
        verbose_print(
            &common,
            &format!(
                "Reading inflectional information for model forms\nfrom {}.\n",
                options.model_form_filename
            ),
        );

        match read_model_forms(options.model_form_filename.as_str(), &mut tokenizer) {
            Ok(mf) => model_forms = mf,
            Err(e) => {
                eprintln!("{e}");
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

        let mut guesses = match get_guesses(
            &line,
            &mut guesser,
            options.max_number_of_guesses,
            &mut tokenizer,
        ) {
            Ok(g) => g,
            Err(e) => {
                error(&common, 1, 0, &format!("{e}"));
                return 1;
            }
        };

        if options.generate_model_forms {
            // make scan-build happy, this should not happen
            let gen_tr = generator
                .as_mut()
                .unwrap_or_else(|| panic!("Error: generator has a NULL value."));
            let paradigms = match get_paradigms(
                &line,
                &guesses,
                gen_tr,
                &model_forms,
                options.max_number_of_forms,
                options.generate_threshold,
            ) {
                Ok(p) => p,
                Err(e) => {
                    error(&common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };

            for it in &paradigms {
                let _ = write!(out, "{}\n", string_vector_to_string(it));
            }
        } else {
            for it in guesses.iter_mut() {
                it.reverse();

                let _ = write!(out, "{}\t{}\n", line, string_vector_to_string(it));
            }
        }
        let _ = write!(out, "\n");
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
