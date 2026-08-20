//! Source-language compilers: the tools that read a grammar or a transducer
//! and build a new transducer from it.
//!
//! Contains, as inline modules:
//! - `guessify`
//! - `pmatch2fst`
//! - `twolc`

pub mod guessify {
    //! Faithful 1:1 port of tools/src/hfst-guessify.cc — the tool for compiling a
    //! guesser and model form generator from a morphological analyzer. Drives the
    //! hfst-cli foundation (globals, getopt, commandline, program-options,
    //! tool-metadata, inc fragments) and the ported hfst::guessify_fst library.

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
    use hfst::guessify_fst::{CATEGORY_SYMBOL_PREFIX, guessify_analyzer, store_guesser};
    use hfst::hfst_data_types::ImplementationType;
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;
    use hfst::hfst_transducer::HfstTransducer;
    use std::io::Write;

    /// hfst-guessify's own options (the former tool-specific `static mut`s).
    struct Options {
        /// '-G, --do-not-compile-generator': compile a model form generator
        /// alongside the guesser (true by default; -G clears it).
        compile_generator: bool,
        /// '-p, --default-penalty': penalty for skipping one symbol of input.
        default_penalty: f32,
    }

    impl Default for Options {
        fn default() -> Options {
            Options {
                compile_generator: true,
                default_penalty: 1.0,
            }
        }
    }

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
        best.unwrap_or(-1.0)
    }

    // [spec:hfst:def:hfst-guessify.print-usage-fn]
    // [spec:hfst:sem:hfst-guessify.print-usage-fn]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nCompile a morphological analyzer into a guesser and generator.\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        let _ = write!(
            msg,
            "Guesser options:\n  -p, --default-penalty           Give penalty for skipping one\n                                  symbol of input (1.0 by default).\n  -G, --do-not-compile-generator  When compiling the guesser, do\n                                  not compile a model form\n                                  generator.\n"
        );
        let _ = writeln!(msg);
        let _ = write!(
            msg,
            "All analyses in the morphological analyzer should have the form:\nw o r d f o r m POS {0}CLASS] X Y Z ...\nwhere POS is the part-of-speech tag, {0}CLASS]\nis an inflectional category marker and X, Y and Z are inflectional\nmarkers. The form of the inflectional category marker is fixed.\nCLASS can be any string, which doesn't contain \"]\".\n",
            CATEGORY_SYMBOL_PREFIX
        );
        let _ = writeln!(msg);
        let _ = write!(
            msg,
            "Using the option -d will reduce the size of the guesser file by\napproximately half, but may substantially increase the load time of\nthe guesser when generating model forms. If you only need to guess\nanalyses of unknown word forms, -d has no effect on load time.\n"
        );
        let _ = writeln!(msg);
        let _ = writeln!(
            msg,
            "If OUTFILE or INFILE is missing or -, standard streams will be used."
        );
        let _ = writeln!(msg);
    }

    // [spec:hfst:def:hfst-guessify.parse-options-fn]
    // [spec:hfst:sem:hfst-guessify.parse-options-fn]
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
                name: "default-penalty",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: 'p' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "do-not-compile-generator",
                has_arg: getopt::NO_ARGUMENT,
                val: 'G' as i32,
            });
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own ('G'/'p'), then the
            // terminal error arm.
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
            match c {
                x if x == 'G' as i32 => {
                    options.compile_generator = false;
                    continue;
                }
                x if x == 'p' as i32 => {
                    let optarg = opt.optarg();
                    options.default_penalty = get_float(&optarg);

                    if options.default_penalty < 0.0 {
                        error(
                            &common,
                            1,
                            0,
                            &format!("Invalid default penalty {}. Give a positive float.", optarg),
                        );
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

    // [spec:hfst:def:hfst-guessify.process-stream-fn]
    // [spec:hfst:sem:hfst-guessify.process-stream-fn]
    fn process_stream(
        common: &CommonOptions,
        options: &Options,
        instream: &mut HfstInputStream<'_>,
        out: &mut HfstOutputStream,
    ) -> i32 {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let _ = transducer_n; // C++ counts but never reads it
            let any = match instream.read() {
                Ok(t) => t,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            // The C++ guessify_fst converted every input to tropical openfst
            // "so that all operations can be performed"; that conversion is
            // now this one typed extraction at the stream boundary
            // ([dec:hfst:monomorphic-backends]).
            let analyzer: HfstTransducer<hfst_openfst::StdVectorFst> = match any.into_typed() {
                Ok(t) => t,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };

            verbose_print(
                common,
                &format!(
                    "Compiling guesser from the transducer {}.\n",
                    analyzer.get_name()
                ),
            );
            let mut guesser = match guessify_analyzer(analyzer, options.default_penalty) {
                Ok(g) => g,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };

            if options.compile_generator {
                verbose_print(
                    common,
                    "Compiling generator and storing guesser and generator.\n",
                );
            } else {
                verbose_print(common, "Storing guesser.\n");
            }

            if let Err(e) = store_guesser(&mut guesser, out, options.compile_generator) {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        }

        instream.close();

        0
    }

    // [spec:hfst:def:hfst-guessify.main-fn]
    // [spec:hfst:sem:hfst-guessify.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.3", "HfstGuessify");
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
        // "%s is not a valid transducer file"; the Rust ctor currently panics on
        // a bad file rather than throwing, so the catch arm is not reproduced.)
        let instream_result = if input_opened {
            HfstInputStream::new_filename(&common.input_filename)
        } else {
            HfstInputStream::new()
        };
        let mut instream = match instream_result {
            Ok(s) => s,
            Err(e) => {
                error(&common, 1, 0, &format!("{e}"));
                return 1;
            }
        };

        let output_opened = common.output_filename != "<stdout>";
        // (the C wraps the ctor in try/catch on HfstException reporting
        // "%s cannot be opened for writing."; the Rust ctor currently panics
        // rather than throwing, so the catch arm is not reproduced here.)
        let outstream_result = if output_opened {
            HfstOutputStream::new_filename(
                &common.output_filename,
                ImplementationType::HFST_OLW_TYPE,
                true,
            )
        } else {
            HfstOutputStream::new(ImplementationType::HFST_OLW_TYPE, true)
        };
        let mut outstream = match outstream_result {
            Ok(s) => s,
            Err(e) => {
                error(&common, 1, 0, &format!("{e}"));
                return 1;
            }
        };

        process_stream(&common, &options, &mut instream, &mut outstream)
    }
}

pub mod pmatch2fst {
    //! Faithful 1:1 port of tools/src/hfst-pmatch2fst.cc — the pmatch regular
    //! expression compiling command-line tool. Drives the hfst-cli foundation
    //! (globals, getopt, commandline, program-options) plus the hfst pmatch
    //! compiler and the OL conversion functions.

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{extend_options_from_env, hfst_set_program_name, verbose_print};
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
        print_common_unary_program_options,
    };
    use crate::inc::{
        CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
        handle_unary_case,
    };
    use hfst::hfst_data_types::ImplementationType;
    use hfst::hfst_output_stream::HfstOutputStream;
    use hfst::hfst_transducer::HfstTransducer;
    use hfst::pmatch_compiler::PmatchCompiler;
    use std::io::{Read, Write};

    /// hfst-pmatch2fst's own options (the former tool-specific `static mut`s).
    #[derive(Default)]
    struct Options {
        /// C: `static char *epsilonname = NULL;` ('-e, --epsilon').
        epsilonname: Option<String>,
        /// C: `static bool flatten = false;` ('--flatten').
        flatten: bool,
        /// C: `static bool include_cosine_distances = false;` ('--cosine-distances').
        include_cosine_distances: bool,
    }

    // C: the compilation format, chosen at compile time from the available
    // back-ends. The Rust crate links the tropical OpenFST back-end.

    // [spec:hfst:def:hfst-pmatch2fst.print-usage-fn]
    // [spec:hfst:sem:hfst-pmatch2fst.print-usage-fn]
    fn print_usage(common: &CommonOptions) {
        let mut msg = common.message_writer();
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nCompile regular expressions into transducer(s)\n (Experimental version)\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        let _ = write!(
            msg,
            "String and format options:\n  -e, --epsilon=EPS         Map EPS as zero\n      --flatten             Compile in all RTNs\n      --cosine-distances    When compiling Like() operations, include cosine distance info\n"
        );
        let _ = writeln!(msg);

        let _ = write!(
            msg,
            "If OUTFILE or INFILE is missing or -, standard streams will be used.\nIf EPS is not defined, the default representation of 0 is used\nWeights are currently not implemented.\n\n"
        );

        let _ = write!(
            msg,
            "Examples:\n  echo \"Define TOP  UppercaseAlpha Alpha* LC({{professor}}) EndTag(ProfName);\" | {} \n  create matcher that tags \"professor Chomsky\" as \"professor <ProfName>Chomsky</ProfName>\"\n\n",
            common.program_name
        );
        let _ = writeln!(msg);
    }

    // [spec:hfst:def:hfst-pmatch2fst.parse-options-fn]
    // [spec:hfst:sem:hfst-pmatch2fst.parse-options-fn]
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
                name: "epsilon",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: 'e' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "flatten",
                has_arg: getopt::NO_ARGUMENT,
                val: '1' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "cosine-distances",
                has_arg: getopt::NO_ARGUMENT,
                val: '2' as i32,
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
            match c as u8 as char {
                'e' => {
                    options.epsilonname = opt.optarg_opt();
                    continue;
                }
                '1' => {
                    options.flatten = true;
                    continue;
                }
                '2' => {
                    options.include_cosine_distances = true;
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

    // [spec:hfst:def:hfst-pmatch2fst.get-current-dir-name-fn]
    // [spec:hfst:sem:hfst-pmatch2fst.get-current-dir-name-fn]
    fn get_current_dir_name() -> String {
        // The C++ allocates a growing buffer and calls getcwd(); the Rust standard
        // library does the equivalent. On failure (the C++ EACCES throw, or any
        // other error) we return the empty string, matching the C++ fallback path.
        match std::env::current_dir() {
            Ok(p) => p.to_string_lossy().into_owned(),
            Err(_) => String::new(),
        }
    }

    // [spec:hfst:def:hfst-pmatch2fst.process-stream-fn]
    // [spec:hfst:sem:hfst-pmatch2fst.process-stream-fn]
    fn process_stream(
        common: &CommonOptions,
        options: &Options,
        outstream: &mut HfstOutputStream,
        input: &mut dyn Read,
    ) -> i32 {
        // pmatch is pinned to the tropical backend (the C++ compilation_format);
        // the former format argument is the type parameter now.
        let mut comp = PmatchCompiler::<hfst_openfst::StdVectorFst>::new();
        comp.set_verbose(common.verbose);
        comp.set_flatten(options.flatten);
        comp.set_include_cosine_distances(options.include_cosine_distances);
        let mut file_bytes: Vec<u8> = Vec::new();
        let mut definitions: std::collections::HashMap<
            String,
            HfstTransducer<hfst_openfst::StdVectorFst>,
        > = std::collections::HashMap::new();

        let mut includedir = String::new();
        let inputfilename_str = &common.input_filename;
        // C: 'inputfile != stdin'. A real input file is in use only when the
        // input filename is a real name (not the "<stdin>" sentinel).
        if inputfilename_str != "<stdin>" && !inputfilename_str.is_empty() {
            if inputfilename_str.starts_with('/') {
                // absolute path
                includedir = inputfilename_str.clone();
            } else {
                let pwd = get_current_dir_name();
                includedir = format!("{}/{}", pwd, inputfilename_str);
            }
            match includedir.rfind('/') {
                None => {
                    // mysterious, we'll just use the working dir
                    includedir = String::new();
                }
                Some(slashpos) => {
                    includedir = includedir[..slashpos + 1].to_string();
                }
            }
        }
        comp.set_include_path(includedir);

        // C: fgetc loop reading the whole input; read_to_end is the equivalent.
        let _ = input.read_to_end(&mut file_bytes);
        // C: std::string holds bytes; reinterpret the collected bytes as UTF-8.
        let file_contents = String::from_utf8_lossy(&file_bytes).into_owned();
        if file_contents.len() > 1 {
            // C wraps comp.compile in try/catch on HfstException; on a thrown
            // exception it prints e.name and returns EXIT_FAILURE. The Rust
            // compiler panics rather than throwing, so the catch arm is not
            // reproduced (any panic propagates).
            definitions = match comp.compile(&file_contents) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("{e}");
                    return 1;
                }
            };
        }

        // Harmonization + archive writing live in the library
        // ('hfst::pmatch_compiler::write_archive'); verbose progress goes to
        // stderr as before.
        match hfst::pmatch_compiler::write_archive(
            &mut definitions,
            outstream,
            common.verbose,
            &mut std::io::stderr(),
        ) {
            Ok(true) => {}
            Ok(false) => {
                eprintln!("{}: Empty ruleset, nothing to write", common.program_name);
                return 1;
            }
            Err(e) => {
                eprintln!("{e}");
                return 1;
            }
        }
        outstream.close();
        0
    }

    // [spec:hfst:def:hfst-pmatch2fst.main-fn]
    // [spec:hfst:sem:hfst-pmatch2fst.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "Pmatch2Fst");
        let (common, options) = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };
        // close buffers, we use streams
        let output_opened = common.output_filename != "<stdout>";
        verbose_print(
            &common,
            &format!(
                "Reading from {}, writing to {}\n",
                common.input_filename, common.output_filename
            ),
        );
        // here starts the buffer handling part
        let mut outstream = match if output_opened {
            HfstOutputStream::new_filename(
                &common.output_filename,
                ImplementationType::HFST_OLW_TYPE,
                true,
            )
        } else {
            HfstOutputStream::new(ImplementationType::HFST_OLW_TYPE, true)
        } {
            Ok(s) => s,
            Err(e) => {
                eprintln!("hfst-pmatch2fst: cannot open output: {e}");
                return 1;
            }
        };
        let mut input = match common.input_reader() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("hfst-pmatch2fst: cannot open input: {e}");
                return 1;
            }
        };
        process_stream(&common, &options, &mut outstream, &mut *input);
        0
    }
}

pub mod twolc {
    //! Faithful 1:1 port of tools/src/hfst-twolc/src/hfst-twolc.cc — the twolc
    //! two-level grammar compiling command-line tool — together with its bespoke
    //! option parser libhfst/src/parsers/commandline_src/CommandLine.{h,cc}.
    //! Drives the hfst TwolcCompiler (which replaces the three htwolcpre
    //! Flex/Bison preprocessor passes with the nfst-twolc parser + AST walk).

    use crate::hfst_getopt::{self as getopt, Getopt};
    use hfst::hfst_data_types::ImplementationType;
    use hfst::hfst_output_stream::HfstOutputStream;
    use hfst::twolc::TwolcCompiler;
    use std::io::{Read, Write};

    // The 'PROGRAM_NAME' macro of the C++ CommandLine ("hfst-twolc"): the name
    // baked into the usage/version texts, independent of argv[0].
    const PROGRAM_NAME: &str = "hfst-twolc";

    /// The parsed command line, mirroring the C++ 'class CommandLine' data
    /// members (input_file/output_file stream handles excluded — streams are
    /// opened where they are used).
    // [spec:hfst:def:command-line.command-line]
    struct CommandLine {
        be_verbose: bool,
        be_quiet: bool,
        has_input_file: bool,
        input_file_name: String,
        has_output_file: bool,
        output_file_name: String,
        resolve_left_conflicts: bool,
        resolve_right_conflicts: bool,
        help: bool,
        version: bool,
        usage: bool,
        has_debug_file: bool,
        format: ImplementationType,
    }

    impl CommandLine {
        // [spec:hfst:def:command-line.command-line.print-version-fn]
        // [spec:hfst:sem:command-line.command-line.print-version-fn]
        fn print_version(&self) {
            // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dversion
            let f = &mut std::io::stderr();
            let _ = write!(
                f,
                "\n{}\n{}\n",
                crate::hfst_commandline::version_line(PROGRAM_NAME),
                crate::hfst_commandline::VERSION_COPYRIGHT_BLOCK
            );
        }

        // [spec:hfst:def:command-line.command-line.print-usage-fn]
        // [spec:hfst:sem:command-line.command-line.print-usage-fn]
        fn print_usage(&self) {
            let f = &mut std::io::stderr();
            let _ = write!(
                f,
                "\nUsage: {0} [OPTIONS...] INFILE\n\
             Usage: {0} [OPTIONS...] -i INFILE\n\
             Usage: {0} [OPTIONS...] --input=INFILE\n\
             Usage: cat INFILE | {0} [OPTIONS...]\n\
             An input file has to be given either using the option -i or\n\
             --input, as the last commandline argument or from STDIN.\n\n",
                PROGRAM_NAME
            );
        }

        // [spec:hfst:def:command-line.command-line.print-help-fn]
        // [spec:hfst:sem:command-line.command-line.print-help-fn]
        fn print_help(&self) {
            self.print_usage();
            let f = &mut std::io::stderr();
            let _ = write!(
                f,
                "\nRead a twolc grammar, compile it and store it. If INFILE is \n\
             missing, the grammar is read from STDIN. If there is no output\n\
             file given using -o or --output, the compiled grammar is\n\
             written to STDOUT.\n\n"
            );
            let _ = write!(
                f,
                "Common options:\n\
             \x20 -h, --help               Print help message\n\
             \x20 -V, --version            Print version info\n\
             \x20 -u, --usage              Print usage\n\
             \x20 -v, --verbose            Print verbosely while processing\n\
             \x20 -q, --quiet              Do not print output\n\
             \x20 -s, --silent             Alias of --quiet\n"
            );
            let _ = write!(
                f,
                "Input/Output options:\n\
             \x20 -i, --input=INFILE       Read input transducer from INFILE\n\
             \x20 -o, --output=OUTFILE     Write output transducer to OUTFILE\n"
            );
            let _ = write!(
                f,
                "TwolC grammar options:\n\
             \x20 -R, --resolve            Resolve left-arrow conflicts.\n\
             \x20 -D, --dont-resolve-right Don't resolve right-arrow conflicts.\n\
             \x20 -f, --format=FORMAT      Store result in format FORMAT.\n\n"
            );
            let _ = write!(
                f,
                "Format may be one of openfst-tropical, foma or sfst.\n\n"
            );
            let _ = write!(
                f,
                "By default format is openfst-tropical. By default right arrow \n\
             conflicts are resolved and left arrow conflicts are not resolved.\n\n"
            );
        }

        // [spec:hfst:def:command-line.command-line.parse-options-fn]
        // [spec:hfst:sem:command-line.command-line.parse-options-fn]
        //
        // The C++ error paths call 'exit(1)' directly; here they return
        // Err(1) and 'run' propagates the exit code.
        // The two leading standalone 'if's for "tropical-weight"/"tropical" set
        // `form` and then fall into the terminal error arm (bug-for-bug from the C,
        // see the -f handler below), so those writes are intentionally never read.
        #[allow(unused_assignments)]
        fn parse_options(&mut self, args: &mut Vec<String>) -> Result<(), i32> {
            let mut resolve_left = false;
            let mut resolve_right = true;
            let mut verbose = false;
            let mut silent = false;
            let mut outfilename: Option<String> = None;
            let mut output_named = false;
            let mut input_named = false;
            let mut is_debug = false;
            let mut infilename: Option<String> = None;
            let mut debug_file_name: Option<String> = None;
            let mut form = ImplementationType::TROPICAL_OPENFST_TYPE;

            // The getopt parser state (was the file-scope static-mut globals) lives
            // in this owned value and is threaded through the loop.
            let mut opt = Getopt::new();
            loop {
                // The C long-option table names '--resolve-left' where the help
                // text (and the Giella build macros) say '--resolve'; both names
                // are accepted here, mapping to the same 'R'.
                let long_options: [(&'static str, i32, i32); 13] = [
                    ("help", getopt::NO_ARGUMENT, 'h' as i32),
                    ("version", getopt::NO_ARGUMENT, 'V' as i32),
                    ("verbose", getopt::NO_ARGUMENT, 'v' as i32),
                    ("quiet", getopt::NO_ARGUMENT, 'q' as i32),
                    ("silent", getopt::NO_ARGUMENT, 's' as i32),
                    ("usage", getopt::NO_ARGUMENT, 'u' as i32),
                    ("input", getopt::REQUIRED_ARGUMENT, 'i' as i32),
                    ("output", getopt::REQUIRED_ARGUMENT, 'o' as i32),
                    ("resolve", getopt::NO_ARGUMENT, 'R' as i32),
                    ("resolve-left", getopt::NO_ARGUMENT, 'R' as i32),
                    ("dont-resolve-right", getopt::NO_ARGUMENT, 'D' as i32),
                    ("debug_file", getopt::REQUIRED_ARGUMENT, 'd' as i32),
                    ("format", getopt::REQUIRED_ARGUMENT, 'f' as i32),
                ];
                let table: Vec<getopt::GetOpt> = long_options
                    .iter()
                    .map(|&(name, has_arg, val)| getopt::GetOpt { name, has_arg, val })
                    .collect();
                let c = opt.getopt_long(args, &table);
                if -1 == c {
                    break;
                }

                match c as u8 as char {
                    'h' => {
                        self.help = true;
                    }
                    'V' => {
                        self.version = true;
                    }
                    'u' => {
                        self.usage = true;
                    }
                    'v' => {
                        verbose = true;
                    }
                    'q' => {
                        silent = true;
                    }
                    's' => {
                        silent = true;
                    }
                    'R' => {
                        resolve_left = true;
                    }
                    'D' => {
                        resolve_right = false;
                    }
                    'i' => {
                        input_named = true;
                        infilename = Some(opt.optarg());
                    }
                    'd' => {
                        is_debug = true;
                        debug_file_name = Some(opt.optarg());
                    }
                    'o' => {
                        output_named = true;
                        outfilename = Some(opt.optarg());
                    }
                    'f' => {
                        let optarg = opt.optarg();
                        // The two leading standalone 'if's are preserved
                        // bug-for-bug from the C: "tropical-weight" and
                        // "tropical" set the format but still fall into the
                        // else-if chain's terminal error arm.
                        if optarg == "tropical-weight" {
                            form = ImplementationType::TROPICAL_OPENFST_TYPE;
                        }
                        if optarg == "tropical" {
                            form = ImplementationType::TROPICAL_OPENFST_TYPE;
                        }
                        if optarg == "tropical-openfst"
                            || optarg == "openfst-tropical"
                            || optarg == "openfst"
                            || optarg == "weighted"
                            || optarg == "weight"
                        {
                            form = ImplementationType::TROPICAL_OPENFST_TYPE;
                        } else if optarg == "sfst" {
                            form = ImplementationType::SFST_TYPE;
                        } else if optarg == "foma" || optarg == "unweighted" {
                            form = ImplementationType::FOMA_TYPE;
                        } else {
                            eprintln!(
                                "Unknown format \"{}\".Try running with option -h or --help.",
                                optarg
                            );
                            return Err(1);
                        }
                    }
                    ':' => {
                        let optopt = opt.optopt;
                        eprintln!(
                            "Missing argument for -{}. Try using --help.",
                            optopt as u8 as char
                        );
                        return Err(1);
                    }
                    _ => {
                        let optopt = opt.optopt;
                        eprintln!(
                            "Unknown commandline option: -{}. Try using --help.",
                            optopt as u8 as char
                        );
                        return Err(1);
                    }
                }
            }

            let optind = opt.optind;
            if !input_named {
                if (args.len() - optind) == 1 {
                    input_named = true;
                    infilename = Some(args[optind].clone());
                } else if (args.len() - optind) > 1 {
                    eprintln!("no more than one input rule file may be given");
                    return Err(1);
                }
            } else if (args.len() - optind) > 0 {
                eprintln!("no more than one input rule file may be given");
                return Err(1);
            }

            self.be_verbose = verbose;
            self.be_quiet = silent;
            self.has_input_file = input_named;
            self.has_output_file = output_named;
            self.resolve_left_conflicts = resolve_left;
            self.resolve_right_conflicts = resolve_right;
            if self.has_input_file {
                self.input_file_name = infilename.unwrap_or_default();
            }
            if self.has_output_file {
                self.output_file_name = outfilename.unwrap_or_default();
            }
            self.format = form;

            if is_debug {
                self.has_debug_file = true;
                self.has_input_file = true;
                self.input_file_name = debug_file_name.unwrap_or_default();
            }

            Ok(())
        }

        // [spec:hfst:def:command-line.command-line.command-line-fn]
        // [spec:hfst:sem:command-line.command-line.command-line-fn]
        fn new(args: &mut Vec<String>) -> Result<Self, i32> {
            let mut cl = CommandLine {
                be_verbose: false,
                be_quiet: false,
                has_input_file: false,
                input_file_name: String::new(),
                has_output_file: false,
                output_file_name: String::new(),
                resolve_left_conflicts: false,
                resolve_right_conflicts: true,
                help: false,
                version: false,
                usage: false,
                has_debug_file: false,
                format: ImplementationType::TROPICAL_OPENFST_TYPE,
            };
            cl.parse_options(args)?;
            Ok(cl)
        }

        /// 'CommandLine::set_input_file': the whole grammar source, from the named
        /// file or stdin. The C++ returned a stream; the Rust compiler front end
        /// takes the source as one string.
        fn read_input(&self) -> Result<String, i32> {
            if self.has_input_file {
                match std::fs::read_to_string(&self.input_file_name) {
                    Ok(s) => Ok(s),
                    Err(_) => {
                        eprintln!("File {} could not be opened!", self.input_file_name);
                        // The C++ printed the __HFST_TWOLC_DIE token to stdout for
                        // the driver script; preserved.
                        print!("__HFST_TWOLC_DIE");
                        Err(1)
                    }
                }
            } else {
                let mut s = String::new();
                match std::io::stdin().read_to_string(&mut s) {
                    Ok(_) => Ok(s),
                    Err(_) => {
                        eprintln!("File <stdin> could not be opened!");
                        print!("__HFST_TWOLC_DIE");
                        Err(1)
                    }
                }
            }
        }
    }

    pub fn run(args: Vec<String>) -> i32 {
        real_main(args)
    }

    fn real_main(mut args: Vec<String>) -> i32 {
        // The C++ driver linked the library's warning/error streams to stderr;
        // here that is the shared tracing subscriber the other tools install via
        // hfst_set_program_name (the library's info!/error! diagnostics would
        // otherwise be dropped).
        let argv0 = args.first().cloned().unwrap_or_default();
        crate::hfst_commandline::hfst_set_program_name(&argv0, "0", "HfstTwolc");

        let command_line = match CommandLine::new(&mut args) {
            Ok(cl) => cl,
            Err(code) => return code,
        };

        if command_line.help || command_line.version {
            if command_line.version {
                command_line.print_version();
            }
            if command_line.help {
                command_line.print_help();
            }
            return 0;
        }
        if command_line.usage {
            command_line.print_usage();
            return 0;
        }
        if !command_line.be_quiet {
            if !command_line.has_input_file {
                eprintln!("Reading input from STDIN.");
            } else {
                eprintln!("Reading input from {}.", command_line.input_file_name);
            }
            if !command_line.has_output_file {
                eprintln!("Writing output to STDOUT.");
            } else {
                eprintln!("Writing output to {}.", command_line.output_file_name);
            }
        }
        if command_line.be_verbose {
            eprintln!("Verbose mode.");
        }

        let input = match command_line.read_input() {
            Ok(s) => s,
            Err(code) => return code,
        };

        // Test that the output file is okay (the C++ opened it up front before
        // running the preprocessor passes).
        let mut out = match if command_line.has_output_file {
            HfstOutputStream::new_filename(
                &command_line.output_file_name,
                command_line.format,
                true,
            )
        } else {
            HfstOutputStream::new(command_line.format, true)
        } {
            Ok(s) => s,
            Err(_) => {
                eprintln!(
                    "File {} could not be opened!",
                    command_line.output_file_name
                );
                print!("__HFST_TWOLC_DIE");
                return 1;
            }
        };

        // The three htwolcpre parse passes + TwolCGrammar build + compile_and_store
        // collapse into the library's TwolcCompiler (nfst-twolc parse + AST walk +
        // per-rule stream store). The --format value is matched ONCE here into
        // the compiler's backend type parameter ([dec:hfst:monomorphic-backends]);
        // the rules are compiled at the requested type and stored to a same-type
        // stream (mirroring C++ htwolcpre3, whose OtherSymbolTransducer is typed by
        // the --format transducer_type). SFST/XFSM still never reach this point (the
        // output stream constructor above rejects them).
        // Name shown in source-anchored diagnostics: the named input file, or the
        // library default ("<twolc>") when reading from stdin.
        let source_name = if command_line.has_input_file {
            command_line.input_file_name.clone()
        } else {
            String::from("<twolc>")
        };
        let compiled = match command_line.format {
            #[cfg(feature = "foma")]
            ImplementationType::FOMA_TYPE => {
                TwolcCompiler::<hfst::backend_foma::FomaTransducer>::new_with_options(
                    command_line.be_quiet,
                    command_line.be_verbose,
                    command_line.resolve_left_conflicts,
                    command_line.resolve_right_conflicts,
                )
                .set_source_name(&source_name)
                .compile_and_store(&input, &mut out)
            }
            _ => TwolcCompiler::<hfst_openfst::StdVectorFst>::new_with_options(
                command_line.be_quiet,
                command_line.be_verbose,
                command_line.resolve_left_conflicts,
                command_line.resolve_right_conflicts,
            )
            .set_source_name(&source_name)
            .compile_and_store(&input, &mut out),
        };
        match compiled {
            Some(()) => {}
            None => {
                // A pass failing made the C++ driver exit(1).
                return 1;
            }
        }
        if command_line.has_output_file {
            if let Err(e) = out.flush() {
                eprintln!("This is an hfst interface bug:\n{}", e);
                return 1;
            }
            out.close();
        }
        0
    }
}
