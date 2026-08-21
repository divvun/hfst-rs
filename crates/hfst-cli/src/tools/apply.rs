//! Text-stream appliers: the tools that run a transducer over running text
//! (as opposed to the word-per-line lookup tools).
//!
//! Contains, as inline modules:
//! - `guess`
//! - `pmatch`
//! - `tokenize`

pub mod guess {
    //! Faithful 1:1 port of tools/src/hfst-guess.cc — the tool for compiling/using
    //! a guesser (and generator) to guess analyses/paradigms of unknown words,
    //! driving the library helper hfst::generate_model_forms. Option handling is
    //! clap 4 derive through [`crate::cli`].

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{error, hfst_set_program_name, verbose_print};
    use hfst::generate_model_forms::{
        StringVectorVector, compile_generator_from_guesser, get_alphabet_string_tokenizer,
        get_guesses, get_paradigms, is_guesser, read_model_forms,
    };
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_symbol_defs::StringVector;
    use hfst::hfst_transducer::HfstTransducer;
    use std::io::{BufRead, Write};

    /// hfst-guess's command line.
    // [spec:hfst:def:hfst-guess.parse-options-fn]
    // [spec:hfst:sem:hfst-guess.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(
        about = "Use a guesser (and generator) to guess analyses or inflectional paradigms of unknown words"
    )]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,

        /// Inflectional information for generated model forms is read from
        /// this file
        #[arg(
            short = 'f',
            long = "model-form-filename",
            value_name = "FILE",
            allow_hyphen_values = true
        )]
        model_form_filename: Option<String>,

        /// Maximal number of analysis per word form (5 by default)
        #[arg(
            short = 'n',
            long = "max-number-of-guesses",
            value_name = "N",
            allow_hyphen_values = true
        )]
        max_number_of_guesses: Option<String>,

        /// Maximal number of generated model forms per guess (2 by default)
        #[arg(
            short = 'm',
            long = "max-number-of-forms",
            value_name = "N",
            allow_hyphen_values = true
        )]
        max_number_of_forms: Option<String>,

        /// Generate only forms whose weight is better than the weight of the
        /// best form plus this threshold (50 by default)
        #[arg(
            short = 'g',
            long = "generate-threshold",
            value_name = "THRESHOLD",
            allow_hyphen_values = true
        )]
        generate_threshold: Option<String>,
    }

    impl Args {
        /// Case 'g': an istringstream float extraction, fatal on a negative
        /// (or unreadable, which yields -1) threshold.
        fn threshold(&self, common: &CommonOptions) -> f32 {
            let Some(text) = self.generate_threshold.as_deref() else {
                return 50.0;
            };
            let value = get_float(text);
            if value < 0.0 {
                error(
                    common,
                    1,
                    0,
                    &format!(
                        "Invalid generate threshold {}. Give a positive float.",
                        text
                    ),
                );
            }
            value
        }

        /// Cases 'n' and 'm': an istringstream size_t extraction, fatal when
        /// no digits could be read.
        fn count(
            &self,
            common: &CommonOptions,
            given: &Option<String>,
            default: usize,
            what: &str,
        ) -> usize {
            let Some(text) = given.as_deref() else {
                return default;
            };
            match parse_size(text) {
                Ok(value) => value,
                Err(_) => {
                    error(
                        common,
                        1,
                        0,
                        &format!("Invalid {} {}. Give a positive int.", what, text),
                    );
                    default
                }
            }
        }

        fn resolve(&self, common: &CommonOptions) -> Options {
            Options {
                generate_model_forms: self.model_form_filename.is_some(),
                model_form_filename: self.model_form_filename.clone().unwrap_or_default(),
                max_number_of_guesses: self.count(
                    common,
                    &self.max_number_of_guesses,
                    5,
                    "maximal number of guesses",
                ),
                max_number_of_forms: self.count(
                    common,
                    &self.max_number_of_forms,
                    2,
                    "maximal number of generated forms",
                ),
                generate_threshold: self.threshold(common),
            }
        }
    }

    impl ToolArgs for Args {
        fn common(&self) -> &CommonArgs {
            &self.common
        }

        fn apply_io(&self, opts: &mut CommonOptions) {
            self.io.apply(opts);
        }

        fn validate(&self, opts: &CommonOptions) -> ToolResult {
            // Every rejection happened inside the C getopt loop, before the
            // parameter checks.
            self.resolve(opts);
            Ok(())
        }
    }

    /// hfst-guess's resolved tool state (the former tool-specific `static mut`s).
    struct Options {
        generate_model_forms: bool,
        model_form_filename: String,
        max_number_of_guesses: usize,
        max_number_of_forms: usize,
        generate_threshold: f32,
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

    // 'std::ostream << StringVector' concatenates the symbols with no separator
    // (generate_model_forms.cc 'operator<<').
    fn string_vector_to_string(v: &StringVector) -> String {
        v.concat()
    }

    // [spec:hfst:def:hfst-guess.main-fn]
    // [spec:hfst:sem:hfst-guess.main-fn]
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.3", "HfstGuess");
        let (common, args) = cli::parse::<Args>(common, args)?;
        let options = args.resolve(&common);

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
                return Err(1);
            }
        };

        // The C opens an ofstream on outfilename or uses std::cout; the
        // foundation's 'output_writer()' already maps OUTFILE-or-stdout to a
        // std::io::Write.
        let mut out = match common.output_writer() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("hfst-guess: cannot open output: {e}");
                return Err(1);
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
                    return Err(1);
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
            return Err(1);
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
                        return Err(1);
                    }
                });
            } else {
                generator = Some(match instream.read().and_then(|any| any.into_typed()) {
                    Ok(g) => g,
                    Err(e) => {
                        error(&common, 1, 0, &format!("{e}"));
                        return Err(1);
                    }
                });
            }
        }

        let mut tokenizer = match get_alphabet_string_tokenizer(&mut guesser) {
            Ok(t) => t,
            Err(e) => {
                error(&common, 1, 0, &format!("{e}"));
                return Err(1);
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
                    return Err(1);
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
                    return Err(1);
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
                        return Err(1);
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

        Ok(())
    }
}

pub mod pmatch {
    //! Faithful 1:1 port of tools/src/hfst-pmatch.cc — the pmatch utility for
    //! continuous matching/lookup on text streams, driving the hfst
    //! optimized-lookup PmatchContainer. Option handling is clap 4 derive
    //! through [`crate::cli`].
    //!
    //! This is a unary tool (#includes inc/globals-common.h + inc/globals-unary.h),
    //! but it does not use the usual unary HfstInputStream/HfstOutputStream pipeline:
    //! it reads its single positional argument as the transducer archive filename,
    //! opens it as a plain binary stream, builds a hfst_ol::PmatchContainer from it,
    //! and then matches the lines of stdin against it, printing to stdout.

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::hfst_set_program_name;
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

    /// hfst-pmatch's command line.
    //
    // This tool chains the common and unary CASES but never includes
    // check-params-common.h or check-params-unary.h — it resolves the single
    // TRANSDUCER operand itself, with its own diagnostics — so the IO group
    // is declared here rather than flattened in.
    // [spec:hfst:def:hfst-pmatch.parse-options-fn]
    // [spec:hfst:sem:hfst-pmatch.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "perform matching/lookup on text streams")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,

        /// Read the pmatch archive from INFILE
        #[arg(
            short = 'i',
            long = "input",
            value_name = "INFILE",
            allow_hyphen_values = true
        )]
        input: Option<String>,

        /// Newline as input separator (default is blank line)
        #[arg(short = 'n', long = "newline")]
        newline: bool,

        /// Only print tagged parts in output
        #[arg(short = 'x', long = "extract-patterns")]
        extract_patterns: bool,

        /// Only print locations of matches
        #[arg(short = 'l', long = "locate")]
        locate: bool,

        /// In locate mode, include weights of the matches
        #[arg(short = 'w', long = "print-weights")]
        print_weights: bool,

        /// Print the total number of matches when done
        #[arg(short = 'c', long = "count-patterns")]
        count_patterns: bool,

        /// Replace matches with opening tags
        #[arg(short = 'z', long = "delete-patterns")]
        delete_patterns: bool,

        /// Don't tag matched patterns
        #[arg(short = 'm', long = "no-mark-patterns")]
        no_mark_patterns: bool,

        /// Upper limit to context length allowed
        #[arg(
            short = 'b',
            long = "max-context",
            value_name = "N",
            allow_hyphen_values = true
        )]
        max_context: Option<String>,

        /// Upper limit for recursion
        #[arg(
            short = 'r',
            long = "max-recursion",
            value_name = "N",
            allow_hyphen_values = true
        )]
        max_recursion: Option<String>,

        /// Upper limit for allowed weight
        #[arg(
            short = 'W',
            long = "weight-cutoff",
            value_name = "W",
            allow_hyphen_values = true
        )]
        weight_cutoff: Option<String>,

        /// Limit search after having used S seconds per input
        #[arg(
            short = 't',
            long = "time-cutoff",
            value_name = "S",
            allow_hyphen_values = true
        )]
        time_cutoff: Option<String>,

        /// Produce profiling data
        #[arg(short = 'p', long = "profile")]
        profile: bool,

        /// Pmatch archive; a - operand reads the standard input
        #[arg(value_name = "TRANSDUCER", num_args = 0..)]
        infiles: Vec<String>,

        /// True when '-t' was written after '-W', i.e. when it is the '-t'
        /// value that survives in `time_cutoff`.
        #[arg(skip)]
        time_after_weight: bool,
    }

    impl Args {
        /// The C's 'strtod(optarg)' equivalents: a value that does not parse
        /// reads as 0, and only a NEGATIVE one is rejected.
        fn number<T: std::str::FromStr + Default + PartialOrd>(
            given: &Option<String>,
            default: T,
            what: &str,
        ) -> Result<T, i32> {
            let Some(text) = given.as_deref() else {
                return Ok(default);
            };
            let value: T = text.trim().parse::<T>().unwrap_or_default();
            if value < T::default() {
                eprintln!("Invalid argument for --{}", what);
                return Err(1);
            }
            Ok(value)
        }

        /// The single TRANSDUCER operand, resolved the way this tool's own
        /// post-loop block did rather than through check-params-unary.h.
        fn input_filename(&self) -> Result<String, i32> {
            match (self.input.as_deref(), self.infiles.len()) {
                (_, n) if n > 1 => {
                    eprintln!("More than one input file given");
                    Err(1)
                }
                (Some(_), 1) => {
                    eprintln!("More than one input file given");
                    Err(1)
                }
                (None, 1) => {
                    let name = &self.infiles[0];
                    Ok(if name == "-" {
                        "<stdin>".to_string()
                    } else {
                        name.clone()
                    })
                }
                (Some(name), _) => Ok(if name == "-" {
                    "<stdin>".to_string()
                } else {
                    name.to_string()
                }),
                (None, _) => {
                    eprintln!("No input file given");
                    Err(1)
                }
            }
        }

        fn resolve(&self) -> Result<Options, i32> {
            let weight_cutoff: f64 =
                Self::number(&self.weight_cutoff, INFINITE_WEIGHT as f64, "weight-cutoff")?;
            // bug-for-bug: the C's 'case W' has no 'break', so it falls
            // through into 'case t' and writes the time cutoff from the SAME
            // argument. Whichever of the two was written last therefore
            // decides the time cutoff.
            let mut time_cutoff = Self::number(&self.time_cutoff, 0.0f64, "time-cutoff")?;
            if self.weight_cutoff.is_some() {
                let from_weight = Self::number(&self.weight_cutoff, 0.0f64, "time-cutoff")?;
                if self.time_cutoff.is_none() || !self.time_after_weight {
                    time_cutoff = from_weight;
                }
            }
            Ok(Options {
                blankline_separated: !self.newline,
                count_patterns: VarVal::from_flag(self.count_patterns),
                delete_patterns: VarVal::from_flag(self.delete_patterns),
                extract_patterns: VarVal::from_flag(self.extract_patterns),
                locate_mode: VarVal::from_flag(self.locate),
                print_weights: VarVal::from_flag(self.print_weights),
                // '-m' is the only case that sets a variable to 'off'.
                mark_patterns: if self.no_mark_patterns {
                    VarVal::Off
                } else {
                    VarVal::NotDefined
                },
                max_recursion: Self::number(&self.max_recursion, -1i32, "max-recursion")?,
                max_context: Self::number(&self.max_context, -1i32, "max-context")?,
                time_cutoff,
                weight_cutoff: weight_cutoff as Weight,
                profile: self.profile,
            })
        }
    }

    impl VarVal {
        fn from_flag(given: bool) -> VarVal {
            if given {
                VarVal::On
            } else {
                VarVal::NotDefined
            }
        }
    }

    impl ToolArgs for Args {
        fn common(&self) -> &CommonArgs {
            &self.common
        }

        fn apply_io(&self, _opts: &mut CommonOptions) {}

        fn applies_check_common_params(&self) -> bool {
            false
        }

        fn absorb_matches(&mut self, matches: &clap::ArgMatches) {
            self.time_after_weight = match (
                matches.index_of("time_cutoff"),
                matches.index_of("weight_cutoff"),
            ) {
                (Some(time), Some(weight)) => time > weight,
                _ => false,
            };
        }

        fn validate(&self, _opts: &CommonOptions) -> ToolResult {
            self.resolve()?;
            self.input_filename()?;
            Ok(())
        }
    }

    /// hfst-pmatch's resolved tool state (the former tool-specific `static mut`s).
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

    // The libreadline_getline helper is compiled only under HAVE_READLINE, which is
    // not defined in this build; its non-readline-library equivalent is reached via
    // hfst_getline in process_input below, so the function body is not reproduced.
    // [spec:hfst:def:hfst-pmatch.libreadline-getline-fn]
    // [spec:hfst:sem:hfst-pmatch.libreadline-getline-fn]

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

    // [spec:hfst:def:hfst-pmatch.main-fn]
    // [spec:hfst:sem:hfst-pmatch.main-fn]
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstPmatch");
        let (common, args) = cli::parse::<Args>(common, args)?;
        let options = args.resolve()?;
        let inputfilename = args.input_filename()?;
        // HAVE_READLINE: rl_bind_key('\t', rl_insert) to disable tab completion;
        // compiled out in this build.

        let mut file = match std::fs::File::open(&inputfilename) {
            Ok(f) => f,
            Err(_) => {
                eprintln!("Could not open file {}", inputfilename);
                return Err(1);
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
                return Err(1);
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
                return Err(1);
            }
        };
        let rv = process_input(&options, &mut container, &mut *out);
        let _ = out.flush();
        cli::from_code(rv)
    }
}

pub mod tokenize {
    //! Faithful 1:1 port of tools/src/hfst-tokenize.cc — a replacement for
    //! hfst-proc using pmatch: perform matching/lookup/tokenization on text
    //! streams, driving the hfst optimized-lookup pmatch tokenizer
    //! ('hfst::pmatch_tokenize', 'hfst::pmatch', 'hfst::pmatch_compiler').
    //! Option handling is clap 4 derive through [`crate::cli`].
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

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{hfst_set_program_name, verbose_print};
    use hfst::hfst_data_types::ImplementationType;
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_transducer::HfstTransducer;
    use hfst::pmatch::PmatchContainer;
    use hfst::pmatch_tokenize::{
        OutputFormat, TokenizeInputSettings, TokenizeSettings, make_naive_tokenizer,
        process_input_stream,
    };

    const DEFAULT_FORMAT: ImplementationType = ImplementationType::TROPICAL_OPENFST_TYPE;

    /// hfst-tokenize's command line.
    //
    // This tool chains only the common CASES — not the unary table — so '-i'
    // is free to be its own switch, and it resolves the single RULESET
    // operand itself, never including check-params-common.h.
    //
    // The C++ long table maps '--space-separated' to 'd' and only its
    // short-option string "nkawWmub:t:l:zixcSgCfL" makes '-i' work, so
    // upstream's long spelling silently means --debug. Here both spellings
    // select the space-separated format and '-d' keeps meaning --debug
    // (see tests/cli_option_wiring.rs).
    // [spec:hfst:def:hfst-tokenize.parse-options-fn]
    // [spec:hfst:sem:hfst-tokenize.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "perform matching/lookup on text streams")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,

        /// Newline as input separator (default is blank line)
        #[arg(short = 'n', long = "newline")]
        newline: bool,

        /// Retain newlines as separators in the output
        #[arg(short = 'k', long = "keep-newline")]
        keep_newline: bool,

        /// Print nonmatching text
        #[arg(short = 'a', long = "print-all")]
        print_all: bool,

        /// Print weights (overrides earlier -W option)
        #[arg(short = 'w', long = "print-weights")]
        print_weights: bool,

        /// Don't print weights (default; overrides earlier -w, or -w implied
        /// by -g, options)
        #[arg(short = 'W', long = "no-weights")]
        no_weights: bool,

        /// Tokenize multicharacter symbols (by default only one grapheme is
        /// tokenized at a time regardless of what is present in the alphabet)
        #[arg(short = 'm', long = "tokenize-multichar")]
        tokenize_multichar: bool,

        /// Output only analyses whose weight is within B from best result
        #[arg(
            short = 'b',
            long = "beam",
            value_name = "B",
            allow_hyphen_values = true
        )]
        beam: Option<String>,

        /// Limit search after having used S seconds per input
        #[arg(
            short = 't',
            long = "time-cutoff",
            value_name = "S",
            allow_hyphen_values = true
        )]
        time_cutoff: Option<String>,

        /// Output no more than N best weight classes (where analyses with
        /// equal weight constitute a class
        #[arg(
            short = 'l',
            long = "weight-classes",
            value_name = "N",
            allow_hyphen_values = true
        )]
        weight_classes: Option<String>,

        /// Remove duplicate analyses (the default)
        #[arg(short = 'u', long = "unique")]
        unique: bool,

        /// Keep duplicate analyses, as upstream does. (PORT ADDITION:
        /// uniqueness is the default here, so the opt-out is the option
        /// upstream has no counterpart for. Long-only, since a short letter
        /// would be one upstream could later claim.)
        #[arg(long = "duplicates")]
        duplicates: bool,

        /// Segmenting / tokenization mode (default)
        #[arg(short = 'z', long = "segment")]
        segment: bool,

        /// Tokenization with one sentence per line, space-separated tokens
        #[arg(short = 'i', long = "space-separated")]
        space_separated: bool,

        /// Xerox output
        #[arg(short = 'x', long = "xerox")]
        xerox: bool,

        /// Constraint Grammar output
        #[arg(short = 'c', long = "cg")]
        cg: bool,

        /// Ignore contents of unescaped [] (cf. apertium-destxt); flush on NUL
        #[arg(short = 'S', long = "superblanks")]
        superblanks: bool,

        /// CG format used in Giella infrastructure (implies -w and -l2,
        /// treats @PMATCH_INPUT_MARK@ as subreading separator, expects tags
        /// to be Multichar_symbols, flush on NUL)
        #[arg(short = 'g', long = "giella-cg", alias = "gtd")]
        giella_cg: bool,

        /// CoNLL-U format
        #[arg(short = 'C', long = "conllu")]
        conllu: bool,

        /// FinnPos output
        #[arg(short = 'f', long = "finnpos")]
        finnpos: bool,

        /// VISL input and output (implies -W, handles <s> as blocks and
        /// <STYLE> inline)
        #[arg(short = 'L', long = "visl")]
        visl: bool,

        /// Ruleset archive file
        #[arg(value_name = "RULESET", num_args = 0..)]
        infiles: Vec<String>,

        /// The tool-specific option occurrences in command-line order. The C
        /// loop's arms overwrite shared settings ('-w'/'-W'/'-g'/'-L' all
        /// write print_weights; every format switch writes output_format), so
        /// the LAST writer wins and a derive struct alone cannot say which
        /// that was; `absorb_matches` rebuilds the order from the match
        /// indices and `resolve` replays it.
        #[arg(skip)]
        events: Vec<Event>,
    }

    /// One iteration of the C option loop, in occurrence order: the common
    /// verbosity writes (which `continue` past the loop-tail verbose check)
    /// and every tool-specific arm (which reach it).
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Event {
        /// '-v' (true) / '-q' / '-s' (false) flipping the shared verbose flag.
        Verbose(bool),
        Newline,
        KeepNewline,
        PrintAll,
        PrintWeights,
        NoWeights,
        TokenizeMultichar,
        Beam,
        TimeCutoff,
        WeightClasses,
        Unique,
        Duplicates,
        Segment,
        SpaceSeparated,
        Xerox,
        Cg,
        Superblanks,
        GiellaCg,
        Conllu,
        Finnpos,
        Visl,
    }

    impl Args {
        /// Replay the C getopt loop over the ordered occurrences: each arm's
        /// writes, then the loop-tail 'if (verbose) settings.verbose = true'
        /// that ran after every TOOL-specific arm (the common cases hit
        /// 'continue' first).
        fn resolve(&self) -> Result<Options, i32> {
            let mut options = Options::default();
            let mut verbose = false;
            for event in &self.events {
                match event {
                    Event::Verbose(on) => {
                        verbose = *on;
                        continue;
                    }
                    Event::KeepNewline => {
                        options.keep_newlines = true;
                        options.blankline_separated = false;
                    }
                    Event::Newline => options.blankline_separated = false,
                    Event::PrintAll => options.settings.print_all = true,
                    Event::PrintWeights => options.settings.print_weights = true,
                    Event::NoWeights => options.settings.print_weights = false,
                    Event::TokenizeMultichar => options.settings.tokenize_multichar = true,
                    Event::TimeCutoff => {
                        let text = self.time_cutoff.as_deref().unwrap_or_default();
                        options.settings.time_cutoff = text.trim().parse::<f64>().unwrap_or(0.0);
                        if options.settings.time_cutoff < 0.0 {
                            eprintln!("Invalid argument for --time-cutoff");
                            return Err(1);
                        }
                    }
                    Event::Unique => options.settings.dedupe = true,
                    Event::Duplicates => options.settings.dedupe = false,
                    Event::Beam => {
                        let text = self.beam.as_deref().unwrap_or_default();
                        options.settings.beam = text.trim().parse::<f64>().unwrap_or(0.0) as f32;
                        if options.settings.beam < 0.0 {
                            eprintln!("Invalid argument for --beam");
                            return Err(1);
                        }
                    }
                    Event::WeightClasses => {
                        let text = self.weight_classes.as_deref().unwrap_or_default();
                        options.settings.max_weight_classes =
                            text.trim().parse::<i32>().unwrap_or(0);
                        if options.settings.max_weight_classes < 1 {
                            eprintln!("Invalid or no argument --weight-classes count");
                            return Err(1);
                        }
                    }
                    Event::Segment => options.settings.output_format = OutputFormat::tokenize,
                    Event::SpaceSeparated => {
                        options.settings.output_format = OutputFormat::space_separated;
                    }
                    Event::Xerox => options.settings.output_format = OutputFormat::xerox,
                    Event::Cg => options.settings.output_format = OutputFormat::cg,
                    Event::Conllu => options.settings.output_format = OutputFormat::conllu,
                    Event::Superblanks => options.superblanks = true,
                    Event::GiellaCg => {
                        options.settings.output_format = OutputFormat::giellacg;
                        options.settings.print_weights = true;
                        options.settings.print_all = true;
                        options.settings.dedupe = true;
                        options.settings.hack_uncompose = true;
                        options.settings.verbose = false;
                        if options.settings.max_weight_classes == i32::MAX {
                            options.settings.max_weight_classes = 2;
                        }
                    }
                    Event::Visl => {
                        options.settings.output_format = OutputFormat::visl;
                        options.settings.print_weights = false;
                        options.settings.print_all = true;
                        options.settings.dedupe = true;
                        options.settings.verbose = false;
                    }
                    Event::Finnpos => options.settings.output_format = OutputFormat::finnpos,
                }

                if verbose {
                    options.settings.verbose = true;
                }
            }
            Ok(options)
        }

        /// The single RULESET operand, resolved the way this tool's own
        /// post-loop block did rather than through check-params-common.h.
        /// (No '-' mapping: the C handed the operand straight to fopen.)
        fn tokenizer_filename(&self) -> Result<String, i32> {
            match self.infiles.len() {
                1 => Ok(self.infiles[0].clone()),
                0 => {
                    eprintln!("No input file given");
                    Err(1)
                }
                _ => {
                    eprintln!("More than one input file given");
                    Err(1)
                }
            }
        }
    }

    impl ToolArgs for Args {
        fn common(&self) -> &CommonArgs {
            &self.common
        }

        fn apply_io(&self, _opts: &mut CommonOptions) {}

        fn applies_check_common_params(&self) -> bool {
            false
        }

        fn absorb_matches(&mut self, matches: &clap::ArgMatches) {
            let ids: &[(&str, Event)] = &[
                ("verbose", Event::Verbose(true)),
                ("quiet", Event::Verbose(false)),
                ("silent", Event::Verbose(false)),
                ("newline", Event::Newline),
                ("keep_newline", Event::KeepNewline),
                ("print_all", Event::PrintAll),
                ("print_weights", Event::PrintWeights),
                ("no_weights", Event::NoWeights),
                ("tokenize_multichar", Event::TokenizeMultichar),
                ("beam", Event::Beam),
                ("time_cutoff", Event::TimeCutoff),
                ("weight_classes", Event::WeightClasses),
                ("unique", Event::Unique),
                ("duplicates", Event::Duplicates),
                ("segment", Event::Segment),
                ("space_separated", Event::SpaceSeparated),
                ("xerox", Event::Xerox),
                ("cg", Event::Cg),
                ("superblanks", Event::Superblanks),
                ("giella_cg", Event::GiellaCg),
                ("conllu", Event::Conllu),
                ("finnpos", Event::Finnpos),
                ("visl", Event::Visl),
            ];
            // A flag never written still holds its "false" default, and clap
            // gives that default an index too — only a CommandLine value
            // source is an occurrence.
            let mut ordered: Vec<(usize, Event)> = ids
                .iter()
                .filter(|(id, _)| {
                    matches.value_source(id) == Some(clap::parser::ValueSource::CommandLine)
                })
                .filter_map(|(id, event)| matches.index_of(id).map(|i| (i, *event)))
                .collect();
            ordered.sort_by_key(|(i, _)| *i);
            self.events = ordered.into_iter().map(|(_, event)| event).collect();
        }

        fn validate(&self, _opts: &CommonOptions) -> ToolResult {
            // The value rejections happened inside the C loop and the operand
            // diagnostics right after it.
            self.resolve()?;
            self.tokenizer_filename()?;
            Ok(())
        }
    }

    /// hfst-tokenize's resolved tool state (the former tool-specific `static mut`s).
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
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstTokenize");
        let (common, args) = cli::parse::<Args>(common, args)?;
        let mut options = args.resolve()?;
        options.tokenizer_filename = args.tokenizer_filename()?;

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
                return Err(1);
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
                    return Err(1);
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
                return Err(1);
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
                    return Err(1);
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
                        return Err(1);
                    }
                };
            let mut container = match make_naive_tokenizer(&mut dictionary) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("hfst-tokenize: {e}");
                    return Err(1);
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
            cli::from_code(process_input_stream(
                &mut container,
                &mut *input,
                &mut stdout,
                &mut *msg,
                &options.settings,
                &input_settings,
            ))
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
                    return Err(1);
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
            cli::from_code(process_input_stream(
                &mut container,
                &mut *input,
                &mut stdout,
                &mut *msg,
                &options.settings,
                &input_settings,
            ))
        }
    }
}
