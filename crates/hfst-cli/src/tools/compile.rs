//! Source-language compilers: the tools that read a grammar or a transducer
//! and build a new transducer from it.
//!
//! Contains, as inline modules:
//! - `guessify`
//! - `pmatch2fst`
//! - `twolc`

pub mod guessify {
    //! Faithful 1:1 port of tools/src/hfst-guessify.cc — the tool for compiling a
    //! guesser and model form generator from a morphological analyzer, driving
    //! the ported hfst::guessify_fst library. Option handling is clap 4 derive
    //! through [`crate::cli`].

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{error, hfst_set_program_name, verbose_print};
    use hfst::guessify_fst::{guessify_analyzer, store_guesser};
    use hfst::hfst_data_types::ImplementationType;
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;
    use hfst::hfst_transducer::HfstTransducer;

    /// hfst-guessify's command line.
    // [spec:hfst:def:hfst-guessify.parse-options-fn]
    // [spec:hfst:sem:hfst-guessify.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "Compile a morphological analyzer into a guesser and generator")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,

        /// Give penalty for skipping one symbol of input (1.0 by default)
        #[arg(
            short = 'p',
            long = "default-penalty",
            value_name = "PENALTY",
            allow_hyphen_values = true
        )]
        default_penalty: Option<String>,

        /// When compiling the guesser, do not compile a model form generator
        #[arg(short = 'G', long = "do-not-compile-generator")]
        do_not_compile_generator: bool,
    }

    impl Args {
        /// Case 'p': an istringstream float extraction, fatal when it fails or
        /// yields a negative penalty.
        fn default_penalty(&self, common: &CommonOptions) -> f32 {
            let Some(text) = self.default_penalty.as_deref() else {
                return 1.0;
            };
            let penalty = get_float(text);
            if penalty < 0.0 {
                error(
                    common,
                    1,
                    0,
                    &format!("Invalid default penalty {}. Give a positive float.", text),
                );
            }
            penalty
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
            // The rejection happened inside the C getopt loop, before the
            // parameter checks.
            self.default_penalty(opts);
            Ok(())
        }
    }

    /// hfst-guessify's resolved tool state (the former tool-specific `static mut`s).
    struct Options {
        /// '-G, --do-not-compile-generator': compile a model form generator
        /// alongside the guesser (true by default; -G clears it).
        compile_generator: bool,
        /// '-p, --default-penalty': penalty for skipping one symbol of input.
        default_penalty: f32,
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
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.3", "HfstGuessify");
        let (common, args) = cli::parse::<Args>(common, args)?;
        let options = Options {
            compile_generator: !args.do_not_compile_generator,
            default_penalty: args.default_penalty(&common),
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
                return Err(1);
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
                return Err(1);
            }
        };

        cli::from_code(process_stream(
            &common,
            &options,
            &mut instream,
            &mut outstream,
        ))
    }
}

pub mod pmatch2fst {
    //! Faithful 1:1 port of tools/src/hfst-pmatch2fst.cc — the pmatch regular
    //! expression compiling command-line tool. Drives the hfst-cli foundation
    //! (globals, getopt, commandline, program-options) plus the hfst pmatch
    //! compiler and the OL conversion functions.

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{hfst_set_program_name, verbose_print};
    use hfst::hfst_data_types::ImplementationType;
    use hfst::hfst_output_stream::HfstOutputStream;
    use hfst::hfst_transducer::HfstTransducer;
    use hfst::pmatch_compiler::PmatchCompiler;
    use std::io::Read;

    /// hfst-pmatch2fst's command line.
    //
    // '--flatten'/'--cosine-distances' carried the getopt `val`s '1'/'2', and
    // this port's getopt derived its shorts from `val` alone, so '-1'/'-2'
    // have always been accepted spellings of them. Declared here so they
    // still are.
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "Compile regular expressions into transducer(s)\n (Experimental version)")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,

        /// Map EPS as zero
        #[arg(
            short = 'e',
            long = "epsilon",
            value_name = "EPS",
            allow_hyphen_values = true
        )]
        epsilon: Option<String>,

        /// Compile in all RTNs
        #[arg(short = '1', long = "flatten")]
        flatten: bool,

        /// When compiling Like() operations, include cosine distance info
        #[arg(short = '2', long = "cosine-distances")]
        cosine_distances: bool,
    }

    impl ToolArgs for Args {
        fn common(&self) -> &CommonArgs {
            &self.common
        }

        fn apply_io(&self, opts: &mut CommonOptions) {
            self.io.apply(opts);
        }
    }

    /// hfst-pmatch2fst's resolved tool state (the former tool-specific `static mut`s).
    #[derive(Default)]
    struct Options {
        /// C: `static char *epsilonname = NULL;` ('-e, --epsilon').
        #[allow(dead_code)]
        epsilonname: Option<String>,
        /// C: `static bool flatten = false;` ('--flatten').
        flatten: bool,
        /// C: `static bool include_cosine_distances = false;` ('--cosine-distances').
        include_cosine_distances: bool,
    }

    // C: the compilation format, chosen at compile time from the available
    // back-ends. The Rust crate links the tropical OpenFST back-end.

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
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "Pmatch2Fst");
        let (common, args) = cli::parse::<Args>(common, args)?;
        let options = Options {
            epsilonname: args.epsilon.clone(),
            flatten: args.flatten,
            include_cosine_distances: args.cosine_distances,
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
                return Err(1);
            }
        };
        let mut input = match common.input_reader() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("hfst-pmatch2fst: cannot open input: {e}");
                return Err(1);
            }
        };
        process_stream(&common, &options, &mut outstream, &mut *input);
        Ok(())
    }
}

pub mod twolc {
    //! Faithful 1:1 port of tools/src/hfst-twolc/src/hfst-twolc.cc — the twolc
    //! two-level grammar compiling command-line tool — together with its bespoke
    //! option parser libhfst/src/parsers/commandline_src/CommandLine.{h,cc}.
    //! Drives the hfst TwolcCompiler (which replaces the three htwolcpre
    //! Flex/Bison preprocessor passes with the nfst-twolc parser + AST walk).
    //!
    //! Option handling is clap 4 derive through [`crate::cli`], but this tool
    //! shares none of the common option layer: `CommandLine` declared its OWN
    //! table, so its '-d' takes an argument (a debug file, not --debug), it has
    //! a '-u/--usage' nothing else has, no '--colour', and '-h'/'-V' print its
    //! own texts to stderr after parsing rather than exiting inside it. The
    //! collision question — which wins when a tool re-declares a common short —
    //! never arises here: nothing prepends the common table to this one, so
    //! every letter means what `CommandLine` says it means.

    use crate::cli::{self, CommonArgs, ErrorStyle, ToolArgs, ToolResult};
    use crate::globals::CommonOptions;
    use hfst::hfst_data_types::ImplementationType;
    use hfst::hfst_output_stream::HfstOutputStream;
    use hfst::twolc::TwolcCompiler;
    use std::io::{Read, Write};

    // The 'PROGRAM_NAME' macro of the C++ CommandLine ("hfst-twolc"): the name
    // baked into the usage/version texts, independent of argv[0].
    const PROGRAM_NAME: &str = "hfst-twolc";

    /// hfst-twolc's command line, i.e. the C++ `CommandLine`'s option table.
    ///
    /// clap's own '--help' is switched off: '-h' only records the request here
    /// and `execute` prints `print_help` afterwards, which is what lets
    /// '-f bogus -h' fail on the format instead of printing help.
    // [spec:hfst:def:command-line.command-line.parse-options-fn]
    // [spec:hfst:sem:command-line.command-line.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(
        about = "Read a twolc grammar, compile it and store it",
        disable_help_flag = true
    )]
    struct Args {
        /// Never populated: this tool's switch does not chain
        /// getopt-cases-common.h.
        #[arg(skip)]
        common: CommonArgs,

        /// Print help message
        #[arg(short = 'h', long = "help")]
        help: bool,

        /// Print version info
        #[arg(short = 'V', long = "version")]
        version: bool,

        /// Print usage
        #[arg(short = 'u', long = "usage")]
        usage: bool,

        /// Print verbosely while processing
        #[arg(short = 'v', long = "verbose")]
        verbose: bool,

        /// Do not print output
        #[arg(short = 'q', long = "quiet")]
        quiet: bool,

        /// Alias of --quiet
        #[arg(short = 's', long = "silent")]
        silent: bool,

        /// Read input transducer from INFILE
        #[arg(
            short = 'i',
            long = "input",
            value_name = "INFILE",
            allow_hyphen_values = true
        )]
        input: Option<String>,

        /// Write output transducer to OUTFILE
        #[arg(
            short = 'o',
            long = "output",
            value_name = "OUTFILE",
            allow_hyphen_values = true
        )]
        output: Option<String>,

        /// Resolve left-arrow conflicts. (The C table names this
        /// '--resolve-left'; the help text and the Giella build macros say
        /// '--resolve'. Both are accepted.)
        #[arg(short = 'R', long = "resolve", alias = "resolve-left")]
        resolve: bool,

        /// Don't resolve right-arrow conflicts
        #[arg(short = 'D', long = "dont-resolve-right")]
        dont_resolve_right: bool,

        /// Read the grammar from DEBUGFILE instead. (The C table spells this
        /// '--debug_file', with an underscore; the accepted spelling is kept.)
        #[arg(short = 'd', long = "debug_file", value_name = "DEBUGFILE")]
        debug_file: Option<String>,

        /// Store result in format FORMAT: openfst-tropical, foma or sfst
        #[arg(short = 'f', long = "format", value_name = "FORMAT")]
        format: Option<String>,

        /// Input rule file; missing reads the standard input
        #[arg(value_name = "INFILE", num_args = 0..)]
        infiles: Vec<String>,
    }

    impl Args {
        /// Case 'f'. The two leading standalone 'if's are preserved
        /// bug-for-bug from the C: "tropical-weight" and "tropical" set the
        /// format but still fall into the else-if chain's terminal error arm,
        /// so both are rejected.
        fn format(&self) -> Result<ImplementationType, i32> {
            let Some(name) = self.format.as_deref() else {
                return Ok(ImplementationType::TROPICAL_OPENFST_TYPE);
            };
            match name {
                "tropical-openfst" | "openfst-tropical" | "openfst" | "weighted" | "weight" => {
                    Ok(ImplementationType::TROPICAL_OPENFST_TYPE)
                }
                "sfst" => Ok(ImplementationType::SFST_TYPE),
                "foma" | "unweighted" => Ok(ImplementationType::FOMA_TYPE),
                other => {
                    eprintln!(
                        "Unknown format \"{}\".Try running with option -h or --help.",
                        other
                    );
                    Err(1)
                }
            }
        }

        /// The post-loop operand resolution: at most one rule file, from '-i'
        /// or the single free argument.
        fn input_file(&self) -> Result<Option<String>, i32> {
            match (&self.input, self.infiles.len()) {
                (Some(_), n) if n > 0 => {
                    eprintln!("no more than one input rule file may be given");
                    Err(1)
                }
                (Some(name), _) => Ok(Some(name.clone())),
                (None, 1) => Ok(Some(self.infiles[0].clone())),
                (None, 0) => Ok(None),
                (None, _) => {
                    eprintln!("no more than one input rule file may be given");
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

        fn applies_common_options(&self) -> bool {
            false
        }

        fn error_style() -> ErrorStyle {
            ErrorStyle::Twolc
        }

        fn validate(&self, _opts: &CommonOptions) -> ToolResult {
            // The format check ran inside the C's loop and the operand check
            // right after it — both before main got to print help.
            self.format()?;
            self.input_file()?;
            Ok(())
        }
    }

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

        // [spec:hfst:def:command-line.command-line.command-line-fn]
        // [spec:hfst:sem:command-line.command-line.command-line-fn]
        //
        // The C++ ctor ran the getopt loop; here clap has already run and the
        // parsed Args are folded into the same data members.
        fn from_args(args: &Args) -> Result<Self, i32> {
            let input_file_name = args.input_file()?;
            let mut cl = CommandLine {
                be_verbose: args.verbose,
                be_quiet: args.quiet || args.silent,
                has_input_file: input_file_name.is_some(),
                input_file_name: input_file_name.unwrap_or_default(),
                has_output_file: args.output.is_some(),
                output_file_name: args.output.clone().unwrap_or_default(),
                resolve_left_conflicts: args.resolve,
                resolve_right_conflicts: !args.dont_resolve_right,
                help: args.help,
                version: args.version,
                usage: args.usage,
                has_debug_file: false,
                format: args.format()?,
            };
            if let Some(name) = &args.debug_file {
                cl.has_debug_file = true;
                cl.has_input_file = true;
                cl.input_file_name = name.clone();
            }
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
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        // The C++ driver linked the library's warning/error streams to stderr;
        // here that is the shared tracing subscriber the other tools install via
        // hfst_set_program_name (the library's info!/error! diagnostics would
        // otherwise be dropped).
        let argv0 = args.first().cloned().unwrap_or_default();
        let common = crate::hfst_commandline::hfst_set_program_name(&argv0, "0", "HfstTwolc");

        let (_common, args) = cli::parse::<Args>(common, args)?;
        let command_line = CommandLine::from_args(&args)?;

        if command_line.help || command_line.version {
            if command_line.version {
                command_line.print_version();
            }
            if command_line.help {
                command_line.print_help();
            }
            return Ok(());
        }
        if command_line.usage {
            command_line.print_usage();
            return Ok(());
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

        let input = command_line.read_input()?;

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
                return Err(1);
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
                return Err(1);
            }
        }
        if command_line.has_output_file {
            if let Err(e) = out.flush() {
                eprintln!("This is an hfst interface bug:\n{}", e);
                return Err(1);
            }
            out.close();
        }
        Ok(())
    }
}
