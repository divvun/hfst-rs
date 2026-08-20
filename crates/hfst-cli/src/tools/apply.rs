//! Text-stream appliers: the tools that run a transducer over running text
//! (as opposed to the word-per-line lookup tools).
//!
//! Contains, as inline modules:
//! - `guess`
//! - `pmatch`
//! - `tokenize`

pub mod guess {
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
        StringVectorVector, compile_generator_from_guesser, get_alphabet_string_tokenizer,
        get_guesses, get_paradigms, is_guesser, read_model_forms,
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

    // [spec:hfst:req:cli.help]
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
        let _ = writeln!(msg);
        let _ = write!(
            msg,
            "The guesser and generator should be constructed using the tool\n\
         hfst-guessify, which can compile a guesser and generator from a\n\
         morphological analyzer. hfst-guessify packages the guesser and\n\
         generator in the same fst-file.\n"
        );
        let _ = writeln!(msg);
        let _ = write!(
            msg,
            "If option -f is used, but a generator has not been compiled\n\
         with the guesser, a generator will be compiled, which will\n\
         increase load time.\n"
        );
        let _ = writeln!(msg);
        let _ = writeln!(msg);
        let _ = writeln!(
            msg,
            "If OUTFILE or INFILE is missing or -, standard streams will be used."
        );
        let _ = writeln!(msg);
    }

    // [spec:hfst:def:hfst-guess.parse-options-fn]
    // [spec:hfst:sem:hfst-guess.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
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
                    let _ = writeln!(out, "{}", string_vector_to_string(it));
                }
            } else {
                for it in guesses.iter_mut() {
                    it.reverse();

                    let _ = writeln!(out, "{}\t{}", line, string_vector_to_string(it));
                }
            }
            let _ = writeln!(out);
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

pub mod pmatch {
    //! Faithful 1:1 port of tools/src/hfst-pmatch.cc — the pmatch utility for
    //! continuous matching/lookup on text streams. Drives the hfst-cli foundation
    //! (globals, getopt, commandline, program-options, inc fragments) and the
    //! hfst optimized-lookup PmatchContainer.
    //!
    //! This is a unary tool (#includes inc/globals-common.h + inc/globals-unary.h),
    //! but it does not use the usual unary HfstInputStream/HfstOutputStream pipeline:
    //! it reads its single positional argument as the transducer archive filename,
    //! opens it as a plain binary stream, builds a hfst_ol::PmatchContainer from it,
    //! and then matches the lines of stdin against it, printing to stdout.
    //!
    //! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
    //! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
    //! `parse_options` and threaded into the processing functions. There are no
    //! `static mut` globals and no `unsafe`.

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{extend_options_from_env, hfst_set_program_name};
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
        print_common_unary_program_options,
    };
    use crate::inc::{CaseResult, handle_common_case, handle_error_case, handle_unary_case};
    use hfst::pmatch::{PmatchContainer, print_locate_matches};
    use hfst::transducer::{INFINITE_WEIGHT, IStream, Weight};
    use std::io::{BufRead, Write};

    // [spec:hfst:def:hfst-pmatch.var-val]
    // The discriminants match the C++ enum order (on=0, off=1, not_defined=2) so
    // the bug-for-bug 'if (print_weights)' truthiness test below stays faithful:
    // 'on' is value 0 and therefore false in a C boolean context.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum VarVal {
        On = 0,
        Off = 1,
        NotDefined = 2,
    }

    /// hfst-pmatch's own options (the former tool-specific `static mut`s).
    struct Options {
        blankline_separated: bool,
        count_patterns: VarVal,
        delete_patterns: VarVal,
        extract_patterns: VarVal,
        locate_mode: VarVal,
        print_weights: VarVal,
        mark_patterns: VarVal,
        max_recursion: i32,
        max_context: i32,
        time_cutoff: f64,
        weight_cutoff: Weight,
        profile: bool,
    }

    impl Default for Options {
        fn default() -> Options {
            Options {
                blankline_separated: true,
                count_patterns: VarVal::NotDefined,
                delete_patterns: VarVal::NotDefined,
                extract_patterns: VarVal::NotDefined,
                locate_mode: VarVal::NotDefined,
                print_weights: VarVal::NotDefined,
                mark_patterns: VarVal::NotDefined,
                max_recursion: -1,
                max_context: -1,
                time_cutoff: 0.0,
                weight_cutoff: INFINITE_WEIGHT,
                profile: false,
            }
        }
    }

    // The libreadline_getline helper is compiled only under HAVE_READLINE, which is
    // not defined in this build; its non-readline-library equivalent is reached via
    // hfst_getline in process_input below, so the function body is not reproduced.
    // [spec:hfst:def:hfst-pmatch.libreadline-getline-fn]
    // [spec:hfst:sem:hfst-pmatch.libreadline-getline-fn]

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] TRANSDUCER\nperform matching/lookup on text streams\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        let _ = write!(
            msg,
            "Pmatch options:\n\
         \x20 -n  --newline           Newline as input separator (default is blank line)\n\
         \x20 -x  --extract-patterns  Only print tagged parts in output\n\
         \x20 -l  --locate            Only print locations of matches\n\
         \x20 -w  --print-weights     In locate mode, include weights of the matches\n\
         \x20 -c  --count-patterns    Print the total number of matches when done\n\
         \x20     --delete-patterns   Replace matches with opening tags\n\
         \x20     --no-mark-patterns  Don't tag matched patterns\n\
         \x20     --max-context       Upper limit to context length allowed\n\
         \x20     --max-recursion     Upper limit for recursion\n\
         \x20     --weight-cutoff=W   Upper limit for allowed weight\n\
         \x20 -t, --time-cutoff=S     Limit search after having used S seconds per input\n\
         \x20 -p  --profile           Produce profiling data\n"
        );
        let _ = write!(msg, "Use standard streams for input and output.\n\n");
        let _ = writeln!(msg);
    }

    // [spec:hfst:def:hfst-pmatch.match-and-print-fn]
    // [spec:hfst:sem:hfst-pmatch.match-and-print-fn]
    fn match_and_print(
        options: &Options,
        container: &mut PmatchContainer,
        outstream: &mut dyn Write,
        input_text: &mut String,
    ) {
        if !input_text.is_empty() && input_text.as_bytes()[input_text.len() - 1] == b'\n' {
            // Remove final newline
            input_text.pop();
        }
        if !container.is_in_locate_mode() {
            let _ = write!(
                outstream,
                "{}",
                container.do_match(input_text, options.time_cutoff, options.weight_cutoff)
            );
            let _ = writeln!(outstream);
            if options.blankline_separated {
                let _ = writeln!(outstream);
            }
        } else {
            let locations =
                container.locate(input_text, options.time_cutoff, options.weight_cutoff);
            // bug-for-bug: C tests 'if (print_weights)' on the raw enum, so
            // 'on' (discriminant 0) is false and only off/not_defined are
            // truthy.
            let printed_something = print_locate_matches(
                &locations,
                &mut *outstream,
                (options.print_weights as i32) != 0,
            );
            if printed_something {
                let _ = writeln!(outstream);
            }
        }
    }

    // [spec:hfst:def:hfst-pmatch.process-input-fn]
    // [spec:hfst:sem:hfst-pmatch.process-input-fn]
    fn process_input(
        options: &Options,
        container: &mut PmatchContainer,
        outstream: &mut dyn Write,
    ) -> i32 {
        let mut input_text = String::new();
        let stdin = std::io::stdin();
        let mut input = stdin.lock();
        loop {
            // The HAVE_READLINE/isatty branch is compiled out in this build; the
            // active path reads with hfst_getline from stdin. read_until(b'\n')
            // mirrors getline's byte semantics; cstr did a lossy UTF-8 conversion.
            let mut raw_bytes: Vec<u8> = Vec::new();
            let read = input.read_until(b'\n', &mut raw_bytes).unwrap_or_default();
            if read == 0 {
                break;
            }

            let line_str = String::from_utf8_lossy(&raw_bytes).into_owned();
            let line_bytes = line_str.as_bytes();
            if !options.blankline_separated {
                // newline separated
                input_text = line_str.clone();
                match_and_print(options, container, &mut *outstream, &mut input_text);
            } else if line_bytes.is_empty() || line_bytes[0] == b'\n' {
                match_and_print(options, container, &mut *outstream, &mut input_text);
                input_text.clear();
            } else {
                input_text.push_str(&line_str);
            }
        }

        if options.blankline_separated && !input_text.is_empty() {
            match_and_print(options, container, &mut *outstream, &mut input_text);
        }
        if options.count_patterns == VarVal::On {
            let _ = write!(outstream, "\n{}\n", container.get_pattern_count_info());
        }
        if options.profile {
            let _ = write!(outstream, "\n{}\n", container.get_profiling_info());
        }
        0
    }

    // [spec:hfst:def:hfst-pmatch.parse-options-fn]
    // [spec:hfst:sem:hfst-pmatch.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
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
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let names: &[(&'static str, i32, i32)] = &[
                ("newline", 0, b'n' as i32),
                ("extract-patterns", 0, b'x' as i32),
                ("locate", 0, b'l' as i32),
                ("print-weights", 0, b'w' as i32),
                ("count-patterns", 0, b'c' as i32),
                ("delete-patterns", 0, b'z' as i32),
                ("no-mark-patterns", 0, b'm' as i32),
                ("max-context", 1, b'b' as i32),
                ("max-recursion", 1, b'r' as i32),
                ("weight-cutoff", 1, b'W' as i32),
                ("time-cutoff", 1, b't' as i32),
                ("profile", 0, b'p' as i32),
            ];
            for (name, has_arg, val) in names.iter() {
                long_options.push(getopt::GetOpt {
                    name,
                    has_arg: *has_arg,
                    val: *val,
                });
            }
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

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
            if c == b'n' as i32 {
                options.blankline_separated = false;
            } else if c == b'x' as i32 {
                options.extract_patterns = VarVal::On;
            } else if c == b'l' as i32 {
                options.locate_mode = VarVal::On;
            } else if c == b'w' as i32 {
                options.print_weights = VarVal::On;
            } else if c == b'c' as i32 {
                options.count_patterns = VarVal::On;
            } else if c == b'z' as i32 {
                options.delete_patterns = VarVal::On;
            } else if c == b'm' as i32 {
                options.mark_patterns = VarVal::Off;
            } else if c == b'b' as i32 {
                options.max_context = opt.optarg().trim().parse::<i32>().unwrap_or(0);
                if options.max_context < 0 {
                    eprintln!("Invalid argument for --max-context");
                    return Err(1);
                }
            } else if c == b'r' as i32 {
                options.max_recursion = opt.optarg().trim().parse::<i32>().unwrap_or(0);
                if options.max_recursion < 0 {
                    eprintln!("Invalid argument for --max-recursion");
                    return Err(1);
                }
            } else if c == b'W' as i32 {
                options.weight_cutoff = opt.optarg().trim().parse::<f64>().unwrap_or(0.0) as Weight;
                if options.weight_cutoff < 0.0 {
                    eprintln!("Invalid argument for --weight-cutoff");
                    return Err(1);
                }
                // NOTE: bug-for-bug — the C 'case W' has no 'break', so it
                // falls through into 'case t' (time-cutoff) below.
                options.time_cutoff = opt.optarg().trim().parse::<f64>().unwrap_or(0.0);
                if options.time_cutoff < 0.0 {
                    eprintln!("Invalid argument for --time-cutoff");
                    return Err(1);
                }
            } else if c == b't' as i32 {
                options.time_cutoff = opt.optarg().trim().parse::<f64>().unwrap_or(0.0);
                if options.time_cutoff < 0.0 {
                    eprintln!("Invalid argument for --time-cutoff");
                    return Err(1);
                }
            } else if c == b'p' as i32 {
                options.profile = true;
            } else {
                return Err(handle_error_case(&common, &opt, c));
            }
        }
        // no more options, we should now be at the input filename
        if (opt.optind + 1) < args.len() {
            eprintln!("More than one input file given");
            Err(1)
        } else if (opt.optind + 1) == args.len() {
            if !common.input_filename.is_empty() {
                eprintln!("More than one input file given");
                Err(1)
            } else {
                common.input_filename = args[opt.optind].clone();
                // C: inputfile = hfst_fopen(inputfilename, "r"); if it resolves to
                // stdin ("-"), reset the name to "<stdin>". The actual archive is
                // (re)opened in run, so only the "-" detection is kept.
                if common.input_filename == "-" {
                    common.input_filename = "<stdin>".to_string();
                }
                Ok((common, options))
            }
        } else if common.input_filename.is_empty() {
            eprintln!("No input file given");
            Err(1)
        } else {
            Ok((common, options))
        }
    }

    // [spec:hfst:def:hfst-pmatch.main-fn]
    // [spec:hfst:sem:hfst-pmatch.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstPmatch");
        let (common, options) = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };
        // HAVE_READLINE: rl_bind_key('\t', rl_insert) to disable tab completion;
        // compiled out in this build.

        let inputfilename = &common.input_filename;
        let mut file = match std::fs::File::open(inputfilename) {
            Ok(f) => f,
            Err(_) => {
                eprintln!("Could not open file {}", inputfilename);
                return 1;
            }
        };
        // The C wraps the container construction + processing in try/catch on
        // HfstException; if the archive is not a valid weighted optimized-lookup
        // pmatch file the catch arm prints a hint and returns 1. The Rust ctor
        // currently panics rather than throwing, so that catch arm is not
        // reproduced here.
        let mut instream = IStream::new(&mut file as &mut dyn std::io::Read);
        let mut container = match PmatchContainer::new_from_stream(&mut instream) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("hfst-pmatch: {e}");
                return 1;
            }
        };
        container.set_verbose(common.verbose);
        if options.extract_patterns != VarVal::NotDefined {
            container.set_extract_patterns(options.extract_patterns == VarVal::On);
        }
        if options.locate_mode != VarVal::NotDefined {
            container.set_locate_mode(options.locate_mode == VarVal::On);
        }
        if options.count_patterns != VarVal::NotDefined {
            container.set_count_patterns(options.count_patterns == VarVal::On);
        }
        if options.delete_patterns != VarVal::NotDefined {
            container.set_delete_patterns(options.delete_patterns == VarVal::On);
        }
        if options.mark_patterns != VarVal::NotDefined {
            container.set_mark_patterns(options.mark_patterns == VarVal::On);
        }
        if options.max_context >= 0 {
            container.set_max_context(options.max_context as usize);
        }
        if options.max_recursion >= 0 {
            container.set_max_recursion(options.max_recursion as usize);
        }
        container.set_profile(options.profile);
        // The C passes std::cout as the output stream; the foundation's
        // output_writer() maps OUTFILENAME (defaulting to "<stdout>") to stdout.
        let mut out = match common.output_writer() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("hfst-pmatch: cannot open output: {e}");
                return 1;
            }
        };
        let rv = process_input(&options, &mut container, &mut *out);
        let _ = out.flush();
        rv
    }
}

pub mod tokenize {
    //! Faithful 1:1 port of tools/src/hfst-tokenize.cc — a replacement for
    //! hfst-proc using pmatch: perform matching/lookup/tokenization on text
    //! streams. Drives the hfst-cli foundation (globals, getopt, commandline,
    //! program-options, inc fragments) and the hfst optimized-lookup pmatch
    //! tokenizer ('hfst::pmatch_tokenize', 'hfst::pmatch', 'hfst::pmatch_compiler').
    //!
    //! This is a unary tool (#includes inc/globals-common.h + inc/globals-unary.h),
    //! but like hfst-pmatch it does not use the usual unary
    //! HfstInputStream/HfstOutputStream pipeline for output: it reads its single
    //! positional argument as the ruleset archive filename, reads lines of stdin
    //! (via 'inputfile'), and prints to stdout.
    //!
    //! The tokenization engine itself (the naive-tokenizer construction and the
    //! input-segmentation drivers) lives in 'hfst::pmatch_tokenize'; this binary
    //! keeps only option parsing and stream opening.

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{extend_options_from_env, hfst_set_program_name, verbose_print};
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{hfst_getopt_common_long, print_common_program_options};
    use crate::inc::{CaseResult, handle_common_case, handle_error_case};
    use hfst::hfst_data_types::ImplementationType;
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_transducer::HfstTransducer;
    use hfst::pmatch::PmatchContainer;
    use hfst::pmatch_tokenize::{
        OutputFormat, TokenizeInputSettings, TokenizeSettings, make_naive_tokenizer,
        process_input_stream,
    };
    use std::io::Write;

    const DEFAULT_FORMAT: ImplementationType = ImplementationType::TROPICAL_OPENFST_TYPE;

    /// hfst-tokenize's own options (the former tool-specific `static mut`s).
    struct Options {
        /// Input is apertium-style superblanks (overrides blankline_separated).
        superblanks: bool,
        /// Input is separated by blank lines (as opposed to single newlines).
        blankline_separated: bool,
        keep_newlines: bool,
        tokenizer_filename: String,
        /// 'static TokenizeSettings settings;' — default-constructed as in C++.
        settings: TokenizeSettings,
    }

    impl Default for Options {
        fn default() -> Options {
            Options {
                superblanks: false,
                blankline_separated: true,
                keep_newlines: false,
                tokenizer_filename: String::new(),
                settings: TokenizeSettings::default(),
            }
        }
    }

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        let mut msg = common.message_writer();
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let _ = write!(
            msg,
            "Usage: {} [--segment | --xerox | --cg | --giella-cg] [OPTIONS...] RULESET\nperform matching/lookup on text streams\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        let _ = write!(
            msg,
            "  -n, --newline            Newline as input separator (default is blank line)\n\
         \x20 -a, --print-all          Print nonmatching text\n\
         \x20 -w, --print-weight       Print weights (overrides earlier -W option)\n\
         \x20 -W, --no-weights         Don't print weights (default; overrides earlier -w, or -w implied by -g, options)\n\
         \x20 -m, --tokenize-multichar Tokenize multicharacter symbols\n\
         \x20                          (by default only one grapheme is tokenized at a time\n\
         \x20                          regardless of what is present in the alphabet)\n\
         \x20 -b, --beam=B             Output only analyses whose weight is within B from best result\n\
         \x20 -tS, --time-cutoff=S     Limit search after having used S seconds per input\n\
         \x20 -lN, --weight-classes=N  Output no more than N best weight classes\n\
         \x20                          (where analyses with equal weight constitute a class\n\
         \x20 -u, --unique             Remove duplicate analyses (the default)\n\
         \x20     --duplicates         Keep duplicate analyses, as upstream does\n\
         \x20 -z, --segment            Segmenting / tokenization mode (default)\n\
         \x20 -i, --space-separated    Tokenization with one sentence per line, space-separated tokens\n\
         \x20 -x, --xerox              Xerox output\n\
         \x20 -c, --cg                 Constraint Grammar output\n\
         \x20 -S, --superblanks        Ignore contents of unescaped [] (cf. apertium-destxt); flush on NUL\n\
         \x20 -g, --giella-cg          CG format used in Giella infrastructure (implies -w and -l2,\n\
         \x20                          treats @PMATCH_INPUT_MARK@ as subreading separator,\n\
         \x20                          expects tags to be Multichar_symbols, flush on NUL)\n\
         \x20 -C  --conllu             CoNLL-U format\n\
         \x20 -f, --finnpos            FinnPos output\n\
         \x20 -L, --visl               VISL input and output (implies -W, handles <s> as blocks and <STYLE> inline)\n",
        );
        let _ = write!(
            msg,
            "Use standard streams for input and output (for now).\n\n"
        );
        let _ = writeln!(msg);
    }

    // [spec:hfst:def:hfst-tokenize.parse-options-fn]
    // [spec:hfst:sem:hfst-tokenize.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
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
            // tool-specific options
            let names: &[(&str, i32, i32)] = &[
                ("newline", getopt::NO_ARGUMENT, b'n' as i32),
                ("keep-newline", getopt::NO_ARGUMENT, b'k' as i32),
                ("print-all", getopt::NO_ARGUMENT, b'a' as i32),
                ("print-weights", getopt::NO_ARGUMENT, b'w' as i32),
                ("no-weights", getopt::NO_ARGUMENT, b'W' as i32),
                ("tokenize-multichar", getopt::NO_ARGUMENT, b'm' as i32),
                ("beam", getopt::REQUIRED_ARGUMENT, b'b' as i32),
                ("time-cutoff", getopt::REQUIRED_ARGUMENT, b't' as i32),
                ("weight-classes", getopt::REQUIRED_ARGUMENT, b'l' as i32),
                ("unique", getopt::NO_ARGUMENT, b'u' as i32),
                // PORT ADDITION: uniqueness is the default here, so the opt-out is
                // the option upstream has no counterpart for. Long-only, since a
                // short letter would be one upstream could later claim.
                ("duplicates", getopt::NO_ARGUMENT, 0x100 + b'u' as i32),
                ("segment", getopt::NO_ARGUMENT, b'z' as i32),
                // C++ declares this long option as 'd' and only ever reaches the
                // space-separated case through the 'i' in its short-option string
                // "nkawWmub:t:l:zixcSgCfL", so upstream --space-separated silently
                // means --debug. This getopt carries no short string — `val` is the
                // sole channel for both spellings — so 'd' would lose the option to
                // the common --debug case and leave -i unknown. 'i' serves both;
                // --debug keeps 'd' via the common table.
                ("space-separated", getopt::NO_ARGUMENT, b'i' as i32),
                ("xerox", getopt::NO_ARGUMENT, b'x' as i32),
                ("cg", getopt::NO_ARGUMENT, b'c' as i32),
                ("superblanks", getopt::NO_ARGUMENT, b'S' as i32),
                ("giella-cg", getopt::NO_ARGUMENT, b'g' as i32),
                ("gtd", getopt::NO_ARGUMENT, b'g' as i32),
                ("conllu", getopt::NO_ARGUMENT, b'C' as i32),
                ("finnpos", getopt::NO_ARGUMENT, b'f' as i32),
                ("visl", getopt::NO_ARGUMENT, b'L' as i32),
            ];
            for &(name, has_arg, val) in names {
                long_options.push(getopt::GetOpt { name, has_arg, val });
            }
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            match handle_common_case(&mut common, &opt, c, print_usage) {
                CaseResult::Return(code) => return Err(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            if c == b'k' as i32 {
                options.keep_newlines = true;
                options.blankline_separated = false;
            } else if c == b'n' as i32 {
                options.blankline_separated = false;
            } else if c == b'a' as i32 {
                options.settings.print_all = true;
            } else if c == b'w' as i32 {
                options.settings.print_weights = true;
            } else if c == b'W' as i32 {
                options.settings.print_weights = false;
            } else if c == b'm' as i32 {
                options.settings.tokenize_multichar = true;
            } else if c == b't' as i32 {
                options.settings.time_cutoff = opt.optarg().trim().parse::<f64>().unwrap_or(0.0);
                if options.settings.time_cutoff < 0.0 {
                    eprintln!("Invalid argument for --time-cutoff");
                    return Err(1);
                }
            } else if c == b'u' as i32 {
                options.settings.dedupe = true;
            } else if c == 0x100 + b'u' as i32 {
                options.settings.dedupe = false;
            } else if c == b'b' as i32 {
                options.settings.beam = opt.optarg().trim().parse::<f64>().unwrap_or(0.0) as f32;
                if options.settings.beam < 0.0 {
                    eprintln!("Invalid argument for --beam");
                    return Err(1);
                }
            } else if c == b'l' as i32 {
                options.settings.max_weight_classes =
                    opt.optarg().trim().parse::<i32>().unwrap_or(0);
                if options.settings.max_weight_classes < 1 {
                    eprintln!("Invalid or no argument --weight-classes count");
                    return Err(1);
                }
            } else if c == b'z' as i32 {
                options.settings.output_format = OutputFormat::tokenize;
            } else if c == b'i' as i32 {
                options.settings.output_format = OutputFormat::space_separated;
            } else if c == b'x' as i32 {
                options.settings.output_format = OutputFormat::xerox;
            } else if c == b'c' as i32 {
                options.settings.output_format = OutputFormat::cg;
            } else if c == b'C' as i32 {
                options.settings.output_format = OutputFormat::conllu;
            } else if c == b'S' as i32 {
                options.superblanks = true;
            } else if c == b'g' as i32 {
                options.settings.output_format = OutputFormat::giellacg;
                options.settings.print_weights = true;
                options.settings.print_all = true;
                options.settings.dedupe = true;
                options.settings.hack_uncompose = true;
                options.settings.verbose = false;
                if options.settings.max_weight_classes == i32::MAX {
                    options.settings.max_weight_classes = 2;
                }
            } else if c == b'L' as i32 {
                options.settings.output_format = OutputFormat::visl;
                options.settings.print_weights = false;
                options.settings.print_all = true;
                options.settings.dedupe = true;
                options.settings.verbose = false;
            } else if c == b'f' as i32 {
                options.settings.output_format = OutputFormat::finnpos;
            } else {
                return Err(handle_error_case(&common, &opt, c));
            }

            if common.verbose {
                options.settings.verbose = true;
            }
        }

        // no more options, we should now be at the input filename
        let argc = args.len();
        if (opt.optind + 1) < argc {
            eprintln!("More than one input file given");
            Err(1)
        } else if (opt.optind + 1) == argc {
            options.tokenizer_filename = args[opt.optind].clone();
            Ok((common, options))
        } else {
            eprintln!("No input file given");
            Err(1)
        }
    }

    // [spec:hfst:def:hfst-tokenize.first-transducer-is-called-top-fn]
    // [spec:hfst:sem:hfst-tokenize.first-transducer-is-called-top-fn]
    // (Defined in the C++ source but never called there; kept for fidelity.)
    #[allow(dead_code)]
    fn first_transducer_is_called_top<B: hfst::backend::Backend>(
        dictionary: &HfstTransducer<B>,
    ) -> bool {
        dictionary.get_name() == "TOP"
    }

    // [spec:hfst:def:hfst-tokenize.main-fn]
    // [spec:hfst:sem:hfst-tokenize.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstTokenize");
        let (common, options) = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };

        let tokenizer_filename = options.tokenizer_filename.clone();
        verbose_print(
            &common,
            &format!(
                "Reading from {}, writing to {}\n",
                tokenizer_filename, common.output_filename
            ),
        );
        let mut file = match std::fs::File::open(&tokenizer_filename) {
            Ok(f) => f,
            Err(_) => {
                eprintln!("Could not open file {}", tokenizer_filename);
                return 1;
            }
        };
        // The C wraps the rest in try/catch on HfstException (and a nested catch
        // on TransducerHeaderException around parse_hfst3_header); the Rust ports
        // currently panic rather than throw, so those catch arms are not
        // reproduced here.
        //
        // To decide whether we're working with something produced by a pmatch
        // ruleset, we want to know whether the first transducer is named TOP. To
        // do this, rather than load the whole thing into a HfstTransducer, we read
        // just the header variables with parse_hfst3_header, then rewind.
        let first_header_attributes = {
            let mut hdr_stream =
                hfst::transducer::IStream::new(&mut file as &mut dyn std::io::Read);
            match PmatchContainer::parse_hfst3_header(&mut hdr_stream) {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("hfst-tokenize: {e}");
                    return 1;
                }
            }
        };
        use std::io::Seek;
        let _ = file.seek(std::io::SeekFrom::Start(0));

        let mut stdout = std::io::stdout();
        // Text input is read from the standard input stream (C: 'inputfile()').
        let mut input = match common.input_reader() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("hfst-tokenize: cannot open input: {e}");
                return 1;
            }
        };
        // The tool-level input-mode switches, handed to the library driver.
        let input_settings = TokenizeInputSettings {
            superblanks: options.superblanks,
            blankline_separated: options.blankline_separated,
            keep_newlines: options.keep_newlines,
            verbose: common.verbose,
        };
        let mut msg = common.message_writer();
        if first_header_attributes.get("name").map(|s| s.as_str()) != Some("TOP") {
            verbose_print(&common, "No TOP automaton found, using naive tokeniser?\n");
            let mut is = match HfstInputStream::new_filename(&tokenizer_filename) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("hfst-tokenize: {e}");
                    return 1;
                }
            };
            // C++ built the naive tokenizer's helper transducers in
            // default_format (tropical); the dictionary converts to the same
            // backend at this boundary ([dec:hfst:monomorphic-backends]).
            let _ = DEFAULT_FORMAT;
            let mut dictionary: HfstTransducer<hfst_openfst::StdVectorFst> =
                match is.read().and_then(|any| any.into_typed()) {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("hfst-tokenize: {e}");
                        return 1;
                    }
                };
            let mut container = match make_naive_tokenizer(&mut dictionary) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("hfst-tokenize: {e}");
                    return 1;
                }
            };
            container.set_verbose(common.verbose);
            // [#367] Auto-enable multichar (longest-match) tokenization when the
            // transducer carries multichar text symbols, so tokenise matches lookup
            // without -m; -m still forces it, single-grapheme alphabets stay
            // single-codepoint.
            let single_codepoint = if options.settings.tokenize_multichar {
                false
            } else {
                !container.has_multichar_input_symbols()
            };
            container.set_single_codepoint_tokenization(single_codepoint);
            process_input_stream(
                &mut container,
                &mut *input,
                &mut stdout,
                &mut *msg,
                &options.settings,
                &input_settings,
            )
        } else {
            verbose_print(
                &common,
                "TOP automaton seen, treating as pmatch script...\n",
            );
            let mut is = hfst::transducer::IStream::new(&mut file as &mut dyn std::io::Read);
            let mut container = match PmatchContainer::new_from_stream(&mut is) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("hfst-tokenize: {e}");
                    return 1;
                }
            };
            container.set_verbose(common.verbose);
            // [#367] Auto-enable multichar (longest-match) tokenization when the
            // transducer carries multichar text symbols, so tokenise matches lookup
            // without -m; -m still forces it, single-grapheme alphabets stay
            // single-codepoint.
            let single_codepoint = if options.settings.tokenize_multichar {
                false
            } else {
                !container.has_multichar_input_symbols()
            };
            container.set_single_codepoint_tokenization(single_codepoint);
            process_input_stream(
                &mut container,
                &mut *input,
                &mut stdout,
                &mut *msg,
                &options.settings,
                &input_settings,
            )
        }
    }
}
