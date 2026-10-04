//! Format converters: the tools that move a transducer between binary
//! formats and textual representations.
//!
//! Contains, as inline modules:
//! - `expand_equivalences`
//! - `format`
//! - `fst2fst`
//! - `fst2txt`

pub mod expand_equivalences {
    //! Faithful 1:1 port of tools/src/hfst-expand-equivalences.cc — the transducer
    //! label modification tool for equivalence classes. Option handling is clap 4
    //! derive through [`crate::cli`].

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        error, error_at_line, hfst_set_program_name, is_input_stream_in_ol_format,
        print_short_help, verbose_print,
    };
    use hfst::expand_equivalences::{
        FsaLevel, TsvExtensionError, expand_equivalences, read_tsv_extensions,
    };
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;
    use std::io::Write;

    /// hfst-expand-equivalences's command line.
    //
    // Its switch chains the COMMON cases only: the unary long options are in
    // its table (the tool splices HFST_GETOPT_UNARY_LONG in) but '-i' reaches
    // no case, so it falls through to the error arm — which is why '-i' is
    // declared here and then rejected rather than left out of the parser.
    // [spec:hfst:def:hfst-expand-equivalences.parse-options-fn]
    // [spec:hfst:sem:hfst-expand-equivalences.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "Extend transducer arcs for equivalence classes")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,

        /// Convert single symbol ISYM to allow OSYM
        #[arg(
            short = 'f',
            long = "from",
            value_name = "ISYM",
            allow_hyphen_values = true
        )]
        from: Option<String>,

        /// Convert to OSYM
        #[arg(
            short = 't',
            long = "to",
            value_name = "OSYM",
            allow_hyphen_values = true
        )]
        to: Option<String>,

        /// Read extensions in acx format from ACXFILE
        #[arg(short = 'a', long = "acx", value_name = "ACXFILE")]
        acx: Option<String>,

        /// Read extensions in tsv format from TSVFILE
        #[arg(short = 'T', long = "tsv", value_name = "TSVFILE")]
        tsv: Option<String>,

        /// Perform extensions on LEVEL of fsa: upper/first/input/1,
        /// lower/second/output/2, or both (default first)
        #[arg(short = 'l', long = "level", value_name = "LEVEL")]
        level: Option<String>,

        /// Accepted by the option table but reaching no case, i.e. rejected
        #[arg(
            short = 'i',
            long = "input",
            value_name = "INFILE",
            hide = true,
            allow_hyphen_values = true
        )]
        input: Option<String>,

        /// Input transducer file; missing or - reads the standard input
        #[arg(value_name = "INFILE", num_args = 0..)]
        infiles: Vec<String>,
    }

    impl Args {
        /// Case 'l': the three LEVEL vocabularies, fatal on anything else.
        fn level(&self, common: &CommonOptions) -> FsaLevel {
            match self.level.as_deref() {
                None => FsaLevel::First,
                Some("first") | Some("upper") | Some("input") | Some("1") => FsaLevel::First,
                Some("second") | Some("lower") | Some("output") | Some("2") => FsaLevel::Second,
                Some("both") => FsaLevel::Both,
                Some(_) => {
                    error(
                        common,
                        1,
                        0,
                        "The option for level parameter must be one of:\n\
                         upper, first, input; second, lower, output; both, \
                         1 or 2.",
                    );
                    FsaLevel::First
                }
            }
        }
    }

    impl ToolArgs for Args {
        fn common(&self) -> &CommonArgs {
            &self.common
        }

        fn apply_io(&self, opts: &mut CommonOptions) {
            // check-params-unary.h with input_named never set, since '-i' has
            // no case here.
            match self.infiles.len() {
                1 => {
                    opts.input_filename = if self.infiles[0] == "-" {
                        "<stdin>".to_string()
                    } else {
                        self.infiles[0].clone()
                    }
                }
                0 => opts.input_filename = "<stdin>".to_string(),
                _ => error(opts, 1, 0, "no more than one transducer file may be given"),
            }
        }

        fn validate(&self, opts: &CommonOptions) -> ToolResult {
            if self.input.is_some() {
                print_short_help(opts);
                error(opts, 1, 0, "invalid option -i");
                return Err(1);
            }
            self.level(opts);
            Ok(())
        }
    }

    /// hfst-expand-equivalences's resolved tool state.
    ///
    /// C used NULL char* as "unset"; modelled here as `Option<String>`. The C++
    /// `ACX_FILE` was a `FILE*` opened by `hfst_fopen` and only ever tested for
    /// non-null (the libxml ACX-parsing body compiles to nothing without libxml);
    /// here it is just an "opened" flag.
    struct Options {
        only_from_label: Option<String>,
        only_to_label: Option<String>,
        acx_file_name: Option<String>,
        acx_file_opened: bool,
        tsv_file_name: Option<String>,
        // FsaLevel, the TSV reader, and the extension/compose loop now live in
        // hfst::expand_equivalences; this tool keeps only the option-driven LEVEL.
        // The TSV file is opened (as a std stream) and parsed in process_stream, so
        // no libc TSV handle is held here.
        level: FsaLevel,
    }

    // [spec:hfst:def:hfst-expand-equivalences.check-options-fn]
    // [spec:hfst:sem:hfst-expand-equivalences.check-options-fn]
    fn check_options(common: &CommonOptions, options: &mut Options) {
        if options.only_from_label.is_some() || options.only_to_label.is_some() {
            if options.tsv_file_name.is_some() || options.acx_file_name.is_some() {
                error(common, 1, 0, "Only one of -a, -T or -f and -t may be given");
            } else if options.only_from_label.is_none() {
                error(common, 1, 0, "option -t requires -f");
            } else if options.only_to_label.is_none() {
                error(common, 1, 0, "option -f requires -t");
            }
        } else if options.tsv_file_name.is_none() && options.acx_file_name.is_none() {
            error(
                common,
                1,
                0,
                "Must give extension specification file with either -a or -t.",
            );
        } else if options.tsv_file_name.is_some() && options.acx_file_name.is_some() {
            error(common, 1, 0, "Only one of parameters -a, -t, must be used.");
        } else if options.tsv_file_name.is_some() {
            // TSV is opened as a std stream and parsed in process_stream via
            // read_tsv_extensions; no libc handle is opened here. A missing file
            // is reported there (slightly later than the C++, which fopen'd it at
            // this point) with the same fatal error.
        } else if let Some(name) = options.acx_file_name.clone() {
            match std::fs::File::open(&name) {
                Ok(_f) => options.acx_file_opened = true,
                Err(_) => {
                    error(common, 1, 0, &format!("Could not open '{}'", name));
                }
            }
        } else {
            error(common, 1, 0, "Logic error again!");
        }
    }

    // [spec:hfst:def:hfst-expand-equivalences.process-stream-fn]
    // [spec:hfst:sem:hfst-expand-equivalences.process-stream-fn]
    fn process_stream(
        common: &CommonOptions,
        options: &Options,
        instream: &mut HfstInputStream<'_>,
        outstream: &mut HfstOutputStream,
    ) {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let _ = transducer_n; // C++ counts but never reads it
            let any = match instream.read() {
                Ok(v) => v,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return;
                }
            };
            // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
            crate::for_algebra!(any, trans => {
                // Collect the (from, to) extension pairs from whichever source the
                // options selected. The TSV parser and the extension/compose loop now
                // live in hfst::expand_equivalences; the per-extension "extending X by
                // Y" and "Applying extensions on N level" -v traces were diagnostic and
                // are not reproduced.
                let mut pairs: Vec<(String, String)> = Vec::new();
                if let Some(from) = options.only_from_label.clone() {
                    let to = options.only_to_label.clone().unwrap_or_default();
                    verbose_print(common, &format!(
                        "using single commandline extension {} with {}\n",
                        from, to
                    ));
                    pairs.push((from, to));
                } else if let Some(tsv_name) = options.tsv_file_name.clone() {
                    verbose_print(common, &format!("reading extensions from {}...\n", tsv_name));
                    let file = match std::fs::File::open(&tsv_name) {
                        Ok(f) => f,
                        Err(e) => {
                            error(common, 1, 0, &format!("cannot open {}: {}", tsv_name, e));
                            return;
                        }
                    };
                    match read_tsv_extensions(std::io::BufReader::new(file)) {
                        Ok(p) => pairs = p,
                        Err(TsvExtensionError { line, message }) => {
                            error_at_line(1, 0, &tsv_name, line, &message);
                            return;
                        }
                    }
                } else if options.acx_file_opened {
                    verbose_print(common, &format!(
                        "Reading ACX from {}...\n",
                        options.acx_file_name.clone().unwrap_or_default()
                    ));
                    // The libxml ACX-parsing body is gated behind #if HAVE_LIBXML_TREE_H
                    // in the C++ source; without libxml it compiles to nothing, which
                    // is the path reproduced here (no extensions added).
                } else {
                    error(common, 1, 0, "DANGER TERROR HORROR !!!!!!");
                    return;
                }

                let mut trans = match expand_equivalences(trans, &pairs, options.level) {
                    Ok(v) => v,
                    Err(e) => {
                        error(common, 1, 0, &format!("{e}"));
                        return;
                    }
                };
                if let Err(e) = outstream.write(&mut trans) {
                    error(common, 1, 0, &format!("{e}"));
                    return;
                }
            }, else => {
                // Unreachable: the optimized-lookup stream rejection already
                // returned before the loop; keep its text for safety.
                let _ = writeln!(
                    std::io::stderr(),
                    "Error: hfst-expand-equivalences cannot process transducers that are in optimized lookup format."
                );
                return;
            });
        } // for each automaton
    }

    // [spec:hfst:def:hfst-expand-equivalences.main-fn]
    // [spec:hfst:sem:hfst-expand-equivalences.main-fn]
    pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstExpandEquivalences");
        let (common, args) = cli::parse::<Args>(common, args)?;
        let mut options = Options {
            only_from_label: args.from.clone(),
            only_to_label: args.to.clone(),
            acx_file_name: args.acx.clone(),
            acx_file_opened: false,
            tsv_file_name: args.tsv.clone(),
            level: args.level(&common),
        };
        check_options(&common, &mut options);

        // close buffers, we use streams
        let input_opened = common.input_filename != "<stdin>";
        let output_opened = common.output_filename != "<stdout>";
        verbose_print(
            &common,
            &format!(
                "Reading from {}, writing to {}\n",
                common.input_filename, common.output_filename
            ),
        );

        // here starts the buffer handling part
        let mut instream = match if input_opened {
            HfstInputStream::new_filename(&common.input_filename)
        } else {
            HfstInputStream::new()
        } {
            Ok(v) => v,
            Err(e) => {
                error(&common, 1, 0, &format!("{e}"));
                return Err(1);
            }
        };
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)

        let ty = instream.get_type();
        let mut outstream = match if output_opened {
            HfstOutputStream::new_filename(&common.output_filename, ty, true)
        } else {
            HfstOutputStream::new(ty, true)
        } {
            Ok(v) => v,
            Err(e) => {
                error(&common, 1, 0, &format!("{e}"));
                return Err(1);
            }
        };

        if is_input_stream_in_ol_format(&instream, "hfst-expand-equivalences") {
            return Err(1);
        }

        process_stream(&common, &options, &mut instream, &mut outstream);
        instream.close();
        outstream.close();
        Ok(())
    }
}

pub mod format {
    //! Faithful 1:1 port of tools/src/hfst-format.cc — the format-checking
    //! command-line tool. Drives the hfst-cli foundation (globals, getopt,
    //! commandline, program-options, tool-metadata, inc fragments).
    //!
    //! This tool is unusual: it #includes globals-common.h and globals-unary.h
    //! (so it is a unary tool), but it does the bulk of its work inside
    //! parse_options (listing formats, testing a format, or opening the input
    //! stream to report its type) and has no process_stream. main is therefore
    //! very thin and simply prints the type returned by parse_options.

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{hfst_set_program_name, hfst_strformat, verbose_print};
    use clap::CommandFactory;
    use hfst::hfst_data_types::ImplementationType;
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_transducer::is_implementation_type_available;
    use std::io::Write;

    /// hfst-format's command line.
    //
    // Its switch chains the common and unary cases and then ends in a
    // 'default: break;' rather than the shared error arm — an option it does
    // not know is silently discarded so the tool still answers about the file
    // it was pointed at. [`cli::drop_unknown_options`] is what reproduces
    // that; [`cli::parse`] itself is always strict.
    //
    // '-i', '-1' and '-2' all write the one input filename, so the last of
    // them on the line decides.
    // [spec:hfst:def:hfst-format.parse-options-fn]
    // [spec:hfst:sem:hfst-format.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "determine HFST transducer format")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,

        /// Read input transducer from INFILE
        #[arg(
            short = 'i',
            long = "input",
            value_name = "INFILE",
            allow_hyphen_values = true,
            overrides_with_all = ["input1", "input2"]
        )]
        input: Option<String>,

        /// Alias of --input
        #[arg(
            short = '1',
            long = "input1",
            value_name = "INFILE",
            allow_hyphen_values = true,
            overrides_with_all = ["input", "input2"]
        )]
        input1: Option<String>,

        /// Alias of --input
        #[arg(
            short = '2',
            long = "input2",
            value_name = "INFILE",
            allow_hyphen_values = true,
            overrides_with_all = ["input", "input1"]
        )]
        input2: Option<String>,

        /// List available transducer formats and print them to standard output
        #[arg(short = 'l', long = "list-formats")]
        list_formats: bool,

        /// Whether the format FMT is available, exits with 0 if it is, else
        /// with 1
        #[arg(short = 't', long = "test-format", value_name = "FMT")]
        test_format: Option<String>,

        /// Input transducer file; missing or - reads the standard input
        #[arg(value_name = "INFILE", num_args = 0..)]
        infiles: Vec<String>,
    }

    impl Args {
        /// The one input filename the three spellings share.
        fn input_filename(&self) -> String {
            self.input
                .clone()
                .or_else(|| self.input1.clone())
                .or_else(|| self.input2.clone())
                .unwrap_or_default()
        }
    }

    impl ToolArgs for Args {
        fn common(&self) -> &CommonArgs {
            &self.common
        }

        /// The operand is resolved against the requested format below, not by
        /// check-params-unary.h, which this tool never included.
        fn apply_io(&self, _opts: &mut CommonOptions) {}

        fn applies_check_common_params(&self) -> bool {
            false
        }
    }

    // fprintf(stdout, ...): write to file descriptor 1.
    fn fput_stdout(s: &str) {
        let _ = std::io::stdout().write_all(s.as_bytes());
        let _ = std::io::stdout().flush();
    }

    // fprintf(stderr, ...): write to file descriptor 2.
    fn fput_stderr(s: &str) {
        let _ = std::io::stderr().write_all(s.as_bytes());
        let _ = std::io::stderr().flush();
    }

    // [spec:hfst:def:hfst-format.main-fn]
    // [spec:hfst:sem:hfst-format.main-fn]
    pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let mut common = hfst_set_program_name(&argv0, "0.1", "HfstFormat");
        common.verbose = true;

        // The 'default: break;' arm: an option this tool does not declare is
        // dropped before the strict parse ever sees it. build() materializes
        // the implicit '-h/--help' argument first — on an unbuilt Command it
        // is invisible to get_arguments(), and the dropper would discard the
        // very token that asks for help.
        let mut cmd = Args::command();
        cmd.build();
        let args = cli::drop_unknown_options(&cmd, cli::normalize_argv(&cmd, args));
        let (mut common, args) = cli::parse::<Args>(common, args)?;
        common.input_filename = args.input_filename();

        // Everything below ran after the C's getopt loop, still inside
        // parse_options; the terminal arms exit outright.
        if let Some(fmt) = args.test_format.as_deref() {
            if (fmt == "sfst" && is_implementation_type_available(ImplementationType::SFST_TYPE))
                || (fmt == "openfst-tropical"
                    && is_implementation_type_available(ImplementationType::TROPICAL_OPENFST_TYPE))
                || (fmt == "foma"
                    && is_implementation_type_available(ImplementationType::FOMA_TYPE))
                || (fmt == "optimized-lookup-unweighted"
                    && is_implementation_type_available(ImplementationType::HFST_OL_TYPE))
                || (fmt == "optimized-lookup-weighted"
                    && is_implementation_type_available(ImplementationType::HFST_OLW_TYPE))
                || (fmt == "thfst"
                    && is_implementation_type_available(ImplementationType::THFST_TYPE))
            {
                return Ok(());
            }
            return Err(1);
        }

        if args.list_formats {
            fput_stdout(" Backend                         Names recognized\n\n");

            if is_implementation_type_available(ImplementationType::SFST_TYPE) {
                fput_stdout(" SFST                            sfst\n");
            }

            if is_implementation_type_available(ImplementationType::TROPICAL_OPENFST_TYPE) {
                fput_stdout(
                    " OpenFst (tropical weights)      openfst-tropical, openfst, ofst, ofst-tropical\n",
                );
            }

            if is_implementation_type_available(ImplementationType::FOMA_TYPE) {
                fput_stdout(" foma                            foma\n");
            }

            if is_implementation_type_available(ImplementationType::HFST_OL_TYPE) {
                fput_stdout(" Optimized lookup (unweighted)   optimized-lookup-unweighted, olu\n");
            }

            if is_implementation_type_available(ImplementationType::HFST_OLW_TYPE) {
                fput_stdout(
                    " Optimized lookup (weighted)     optimized-lookup-weighted, olw, optimized-lookup, ol\n",
                );
            }

            if is_implementation_type_available(ImplementationType::THFST_TYPE) {
                fput_stdout(" THFST (divvunspell speller format)          thfst\n");
            }

            return Ok(());
        }

        // The C wraps the stream opening in try/catch on HfstException; on a
        // non-transducer stream it prints an error and exit(1). The Rust ctor
        // currently panics rather than throwing, so the catch arm is mirrored
        // by catching the panic.
        let remaining = args.infiles.len();
        let free_arg = if remaining == 1 {
            Some(args.infiles[0].clone())
        } else {
            None
        };
        let input_filename = common.input_filename.clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || -> Result<(ImplementationType, String), hfst::error::Error> {
                if input_filename.is_empty() {
                    if remaining == 0 {
                        let is = HfstInputStream::new()?;
                        return Ok((is.get_type(), "<stdin>".to_string()));
                    } else if remaining == 1 {
                        let resolved = free_arg
                            .clone()
                            .expect("free_arg is Some when exactly one free argument remains");
                        let is = HfstInputStream::new_filename(&resolved)?;
                        return Ok((is.get_type(), resolved));
                    }
                }
                let is = HfstInputStream::new_filename(&input_filename)?;
                Ok((is.get_type(), input_filename.clone()))
            },
        ));

        let ty = match result {
            Ok(Ok((ty, resolved))) => {
                common.input_filename = resolved;
                ty
            }
            Ok(Err(_)) | Err(_) => {
                fput_stderr("ERROR: The file/stream does not contain transducers.\n");
                return Err(1);
            }
        };

        verbose_print(
            &common,
            &format!(
                "Transducers in {} are of type {}\n",
                common.input_filename,
                hfst_strformat(ty)
            ),
        );
        Ok(())
    }
}

pub mod fst2fst {
    //! Faithful 1:1 port of tools/src/hfst-fst2fst.cc — the format conversion
    //! command-line tool. A unary tool: it reads one input stream and converts
    //! each transducer to another binary implementation format. Option handling
    //! is clap 4 derive through [`crate::cli`].

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        convert_any_with_options, error, hfst_parse_format_name, hfst_set_program_name,
        hfst_strformat, verbose_print, warning,
    };
    use crate::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary, hfst_set_name_unary};
    use clap::ArgAction;
    use hfst::dhfst_writer::{SourceModel, WriteOptions, Written};
    use hfst::hfst_data_types::ImplementationType;
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;
    use hfst::hfst_transducer::AnyTransducer;
    use std::io::Write;

    /// The format name `-f` takes for the DHFST error-model format. DHFST is
    /// an output mode, not an implementation type: hfst writes it but never
    /// reads it, so it has no place in the type-name vocabulary.
    // [spec:hfst:def:dhfst.fst2fst]
    const DHFST_FORMAT: &str = "dhfst";

    /// The fallback depth bound `-f dhfst` writes with unless told otherwise.
    const DHFST_DEFAULT_DEPTH: u32 = 4;

    /// One occurrence of an output-type option, in the order it was written.
    ///
    /// Six different options write the single `output_type`, and the
    /// diagnostic for giving several depends on the order they were written
    /// in. A derive struct cannot carry that, so the occurrences are
    /// recovered from the match indices and replayed.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum TypeOpt {
        /// '-f FMT', carrying its position among the --format values.
        Format(usize),
        Sfst,
        Foma,
        Tropical,
        OlUnweighted,
        OlWeighted,
    }

    /// hfst-fst2fst's command line.
    // [spec:hfst:def:hfst-fst2fst.parse-options-fn]
    // [spec:hfst:sem:hfst-fst2fst.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "Convert transducers between binary formats")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,

        /// Write result in FMT format: foma, openfst-tropical, sfst, thfst,
        /// dhfst, optimized-lookup-weighted, optimized-lookup-unweighted
        #[arg(short = 'f', long = "format", value_name = "FMT", action = ArgAction::Append)]
        format: Vec<String>,

        /// With -f dhfst, the longest fallback chain a state may have
        /// (default 4)
        #[arg(long = "max-fallback-depth", value_name = "N")]
        max_fallback_depth: Option<u32>,

        /// With -f dhfst, allow fallback chains of any length
        #[arg(long = "unbounded-fallback")]
        unbounded_fallback: bool,

        /// Write result in implementation format, without any HFST wrappers
        #[arg(short = 'b', long = "use-backend-format")]
        use_backend_format: bool,

        /// Write output in (HFST's) SFST implementation
        #[arg(short = 'S', long = "sfst", action = ArgAction::Count)]
        sfst: u8,

        /// Write output in (HFST's) foma implementation
        #[arg(short = 'F', long = "foma", action = ArgAction::Count)]
        foma: u8,

        /// Write output in (HFST's) tropical weight (OpenFST) implementation
        #[arg(short = 't', long = "openfst-tropical", action = ArgAction::Count)]
        openfst_tropical: u8,

        /// Write output in the HFST optimized-lookup implementation
        #[arg(short = 'O', long = "optimized-lookup-unweighted", action = ArgAction::Count)]
        optimized_lookup_unweighted: u8,

        /// Write output in optimized-lookup (weighted) implementation
        #[arg(short = 'w', long = "optimized-lookup-weighted", action = ArgAction::Count)]
        optimized_lookup_weighted: u8,

        /// When converting to optimized-lookup, don't try hard to compress
        #[arg(short = 'Q', long = "quick")]
        quick: bool,

        /// The output-type options in the order they were written.
        #[arg(skip)]
        type_order: Vec<TypeOpt>,
    }

    impl Args {
        /// Replay the output-type options in command-line order, which is what
        /// the C's getopt loop did, and answer with the resolved type.
        // [spec:hfst:def:hfst-fst2fst.set-output-type-fn]
        // [spec:hfst:sem:hfst-fst2fst.set-output-type-fn]
        fn output_type(&self, common: &CommonOptions) -> ImplementationType {
            let mut output_type = ImplementationType::UNSPECIFIED_TYPE;
            fn set(
                common: &CommonOptions,
                output_type: &mut ImplementationType,
                ty: ImplementationType,
            ) {
                if *output_type != ImplementationType::UNSPECIFIED_TYPE {
                    error(common, 1, 0, "Output type defined several times.");
                }
                *output_type = ty;
            }
            for opt in &self.type_order {
                match opt {
                    TypeOpt::Format(nth) => {
                        let name = self
                            .format
                            .get(*nth)
                            .map(String::as_str)
                            .unwrap_or_default();
                        // DHFST is written from weighted optimized lookup, so
                        // that is the type the transducers convert to.
                        // [spec:hfst:sem:dhfst.fst2fst+1]
                        let ty = if name.eq_ignore_ascii_case(DHFST_FORMAT) {
                            ImplementationType::HFST_OLW_TYPE
                        } else {
                            hfst_parse_format_name(common, name)
                        };
                        if ty == ImplementationType::XFSM_TYPE {
                            error(common, 1, 0, "xfsm back-end is not available");
                        }
                        set(common, &mut output_type, ty);
                    }
                    TypeOpt::Sfst => set(common, &mut output_type, ImplementationType::SFST_TYPE),
                    TypeOpt::Foma => set(common, &mut output_type, ImplementationType::FOMA_TYPE),
                    TypeOpt::Tropical => set(
                        common,
                        &mut output_type,
                        ImplementationType::TROPICAL_OPENFST_TYPE,
                    ),
                    TypeOpt::OlUnweighted => {
                        set(common, &mut output_type, ImplementationType::HFST_OL_TYPE)
                    }
                    TypeOpt::OlWeighted => {
                        set(common, &mut output_type, ImplementationType::HFST_OLW_TYPE)
                    }
                }
            }
            output_type
        }

        /// Whether `-f dhfst` asked for DHFST output.
        fn writes_dhfst(&self) -> bool {
            self.format
                .iter()
                .any(|f| f.eq_ignore_ascii_case(DHFST_FORMAT))
        }

        /// The fallback depth bound for DHFST output; `None` for no bound.
        fn dhfst_depth(&self) -> Option<u32> {
            if self.unbounded_fallback {
                None
            } else {
                Some(self.max_fallback_depth.unwrap_or(DHFST_DEFAULT_DEPTH))
            }
        }

        /// Refuse the DHFST options anywhere they would do nothing.
        // [spec:hfst:sem:dhfst.fst2fst+1]
        fn validate_dhfst(&self, opts: &CommonOptions) -> ToolResult {
            let depth_given = self.max_fallback_depth.is_some() || self.unbounded_fallback;
            let refusal = if !self.writes_dhfst() && depth_given {
                Some("--max-fallback-depth and --unbounded-fallback apply only to -f dhfst")
            } else if self.max_fallback_depth.is_some() && self.unbounded_fallback {
                Some("--max-fallback-depth and --unbounded-fallback are mutually exclusive")
            } else if self.writes_dhfst() && self.use_backend_format {
                Some("DHFST has no HFST3 header to leave out, so -b does not apply to -f dhfst")
            } else {
                None
            };
            match refusal {
                Some(message) => {
                    error(opts, 1, 0, message);
                    Err(1)
                }
                None => Ok(()),
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

        fn absorb_matches(&mut self, matches: &clap::ArgMatches) {
            let mut order: Vec<(usize, TypeOpt)> = Vec::new();
            for (nth, index) in matches
                .indices_of("format")
                .into_iter()
                .flatten()
                .enumerate()
            {
                order.push((index, TypeOpt::Format(nth)));
            }
            for (id, opt) in [
                ("sfst", TypeOpt::Sfst),
                ("foma", TypeOpt::Foma),
                ("openfst_tropical", TypeOpt::Tropical),
                ("optimized_lookup_unweighted", TypeOpt::OlUnweighted),
                ("optimized_lookup_weighted", TypeOpt::OlWeighted),
            ] {
                // A Count arg is always "present" with its zero default, so
                // the count is what says whether it was written at all, and
                // the default's index 0 is not a command-line position. A
                // count keeps one index however often it was repeated, so the
                // extra occurrences are pinned to the last position seen —
                // enough, since the second of them is already fatal.
                let count = matches.get_count(id) as usize;
                let mut indices: Vec<usize> = matches
                    .indices_of(id)
                    .into_iter()
                    .flatten()
                    .filter(|index| *index > 0)
                    .collect();
                while indices.len() < count {
                    let last = indices.last().copied().unwrap_or(0);
                    indices.push(last);
                }
                for index in indices.into_iter().take(count) {
                    order.push((index, opt));
                }
            }
            order.sort_by_key(|(index, _)| *index);
            self.type_order = order.into_iter().map(|(_, opt)| opt).collect();
        }

        fn validate(&self, opts: &CommonOptions) -> ToolResult {
            // The type resolution ran inside the C getopt loop and the
            // must-specify test right after it, both before the parameter
            // checks.
            if self.output_type(opts) == ImplementationType::UNSPECIFIED_TYPE {
                error(
                    opts,
                    1,
                    0,
                    "You must specify an output type (one of -S, -F, -t, -x, -l, -O, or -w)",
                );
                return Err(1);
            }
            self.validate_dhfst(opts)
        }
    }

    /// hfst-fst2fst's resolved tool state (the former tool-specific `static mut`s).
    struct Options {
        /// output implementation format ('-f/-S/-F/-t/-l/-O/-w').
        output_type: ImplementationType,
        /// '-b/--use-backend-format': write in implementation format without HFST
        /// wrappers (default: true, i.e. write HFST3 headers).
        hfst_format: bool,
        /// '-Q/--quick': relax optimized-lookup table packing.
        options: String,
        /// '-f dhfst': write DHFST from the weighted optimized lookup the
        /// transducer converts to.
        dhfst: bool,
        /// '--max-fallback-depth' / '--unbounded-fallback': the DHFST fallback
        /// depth bound, `None` for no bound.
        dhfst_depth: Option<u32>,
    }

    // [spec:hfst:def:hfst-fst2fst.process-stream-fn]
    // [spec:hfst:sem:hfst-fst2fst.process-stream-fn]
    fn process_stream(
        common: &CommonOptions,
        options: &Options,
        instream: &mut HfstInputStream<'_>,
        outstream: &mut HfstOutputStream,
    ) -> i32 {
        if instream.get_type() == ImplementationType::FOMA_TYPE
            && !instream.is_hfst_header_included()
            && !common.silent
        {
            warning(
                common,
                0,
                0,
                "converting native foma transducer: \
                 inversion may be needed for hfst-lookup to work as expected \
                 (hfst-flookup works as foma's flookup)\n",
            );
        }

        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let orig = match instream.read() {
                Ok(v) => v,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };

            let inputname = hfst_get_name(&orig, &common.input_filename);
            if transducer_n == 1 {
                verbose_print(common, &format!("Converting {}...\n", inputname));
            } else {
                verbose_print(
                    common,
                    &format!("Converting {}...{}\n", inputname, transducer_n),
                );
            }
            // The typed cross-format conversion at the stream boundary
            // ([dec:hfst:monomorphic-backends]): to_basic/from_basic between
            // the algebra backends, to_ol(weighted, options) for OL output.
            // C wraps the conversion in try/catch on HfstException; the Rust
            // conversion currently panics rather than throwing, so the catch arm
            // is not reproduced here.
            let converted =
                match convert_any_with_options(orig, options.output_type, &options.options) {
                    Ok(v) => v,
                    Err(e) => {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                };
            // C: hfst_set_name(orig, orig, "convert"); the dest and src are the
            // same object, which Rust cannot alias mut+const, so the read side is
            // taken from a copy (name/formula are unchanged by the copy).
            let code = crate::for_any!(converted, orig => {
                let mut orig = orig;
                let src = orig.clone();
                hfst_set_name_unary(&mut orig, &src, "convert");
                hfst_set_formula_unary(&mut orig, &src, "Id");
                if let Err(e) = outstream.write(&mut orig) {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
                0
            });
            if code != 0 {
                return code;
            }
        }
        if let Err(e) = outstream.flush() {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }
        instream.close();
        outstream.close();
        0
    }

    /// Write the one transducer of `instream` as DHFST: converted to weighted
    /// optimized lookup exactly as `-f olw` converts it, then encoded and
    /// self-checked by the DHFST writer. Nothing is written unless the check
    /// passes.
    // [spec:hfst:sem:dhfst.fst2fst+1]
    fn write_dhfst(
        common: &CommonOptions,
        options: &Options,
        instream: &mut HfstInputStream<'_>,
    ) -> i32 {
        let orig = match instream.read() {
            Ok(v) => v,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        if instream.is_good() {
            error(
                common,
                1,
                0,
                "a DHFST file holds one transducer, and the input has more than one",
            );
            return 1;
        }
        verbose_print(
            common,
            &format!(
                "Converting {}...\n",
                hfst_get_name(&orig, &common.input_filename)
            ),
        );
        // Conversion to HFST_OLW_TYPE always yields the OlW variant; any other
        // would be a bug in the conversion path.
        let olw = match convert_any_with_options(orig, options.output_type, &options.options) {
            Ok(AnyTransducer::OlW(t)) => t,
            Ok(AnyTransducer::Tropical(_) | AnyTransducer::OlU(_) | AnyTransducer::Thfst(_)) => {
                error(
                    common,
                    1,
                    0,
                    "internal error: the conversion did not yield weighted optimized lookup",
                );
                return 1;
            }
            #[cfg(feature = "foma")]
            Ok(AnyTransducer::Foma(_)) => {
                error(
                    common,
                    1,
                    0,
                    "internal error: the conversion did not yield weighted optimized lookup",
                );
                return 1;
            }
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        let model = match SourceModel::from_olw(&olw) {
            Ok(m) => m,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        drop(olw);
        // The meta section names the source by its file name, as dhfst-tools
        // does, so from the same file the two differ only in the writer.
        let source_name = match common.input_filename.as_str() {
            "<stdin>" => String::new(),
            path => std::path::Path::new(path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        };
        let write_options = WriteOptions {
            max_fallback_depth: options.dhfst_depth,
            threads: 0,
            source_name,
        };
        let written = match hfst::dhfst_writer::write(&model, &write_options) {
            Ok(w) => w,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        let saved = common
            .output_writer()
            .and_then(|mut out| out.write_all(&written.bytes).and_then(|()| out.flush()));
        if let Err(e) = saved {
            error(
                common,
                1,
                0,
                &format!("cannot write {}: {e}", common.output_filename),
            );
            return 1;
        }
        verbose_print(common, &dhfst_report(&model, &written, options.dhfst_depth));
        0
    }

    /// The DHFST writer's report, as `-v` prints it.
    fn dhfst_report(model: &SourceModel, written: &Written, depth: Option<u32>) -> String {
        let r = &written.report;
        let pc = |x: u64| 100.0 * x as f64 / r.source_arcs.max(1) as f64;
        let defaults: u64 = r.defaults.iter().sum();
        let entries = r.explicit_arcs + r.blockers + defaults;
        let b = &r.answered_by;
        format!(
            "DHFST source: {} states, {} arcs, {} symbols, {} duplicate arcs dropped\n\
             encoding: {entries} entries ({:.2}% of arcs) = explicit {} + blockers {} + defaults {defaults} \
             (identity {}, substitution {}, deletion {}, insertion {}); classes {}, class pairs {}\n\
             fallbacks: {} states; depth bound {}, longest chain {}; states per chain length {:?}\n\
             source arcs answered by: own explicit {:.2}%, own default identity {:.2}% \
             substitution {:.2}% deletion {:.2}% insertion {:.2}%, inherited explicit {:.2}%, \
             inherited default {:.2}%\n\
             checked: {} (state, pair) resolutions of the encoding; {} (state, input) queries, \
             {} arcs, read back from the written bytes: all equal to the source\n\
             wrote {} bytes of DHFST\n",
            model.states().len(),
            r.source_arcs,
            model.symbols().len(),
            model.duplicate_arcs(),
            pc(entries),
            r.explicit_arcs,
            r.blockers,
            r.defaults[0],
            r.defaults[1],
            r.defaults[2],
            r.defaults[3],
            r.classes,
            r.class_pairs,
            r.states_with_fallback,
            depth.map_or("none".to_string(), |d| d.to_string()),
            r.max_depth,
            r.depth_histogram,
            pc(b[0]),
            pc(b[1]),
            pc(b[2]),
            pc(b[3]),
            pc(b[4]),
            pc(b[5]),
            pc(b[6]),
            r.pairs_checked,
            r.queries_checked,
            r.arcs_checked,
            written.bytes.len(),
        )
    }

    // [spec:hfst:def:hfst-fst2fst.main-fn]
    // [spec:hfst:sem:hfst-fst2fst.main-fn]
    pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstFst2Fst");
        let (common, args) = cli::parse::<Args>(common, args)?;
        let options = Options {
            output_type: args.output_type(&common),
            hfst_format: !args.use_backend_format,
            options: if args.quick {
                "quick".to_string()
            } else {
                String::new()
            },
            dhfst: args.writes_dhfst(),
            dhfst_depth: args.dhfst_depth(),
        };
        // close buffers, we use streams
        let input_opened = common.input_filename != "<stdin>";
        let output_opened = common.output_filename != "<stdout>";
        verbose_print(
            &common,
            &format!(
                "Reading from {}, writing to {}\n",
                common.input_filename, common.output_filename
            ),
        );
        if options.dhfst {
            verbose_print(&common, "Writing a DHFST error model\n");
        } else if options.hfst_format {
            verbose_print(
                &common,
                &format!(
                    "Writing {} format transducers with HFST3 headers\n",
                    hfst_strformat(options.output_type)
                ),
            );
        } else {
            verbose_print(
                &common,
                &format!(
                    "Writing {} format transducers without HFST specific headers\n",
                    hfst_strformat(options.output_type)
                ),
            );
        }

        // THFST is a directory format with no byte-stream encoding, so it can never
        // be written to standard output [spec:hfst:sem:thfst-backend.stream-io].
        if options.output_type == ImplementationType::THFST_TYPE
            && common.output_filename == "<stdout>"
        {
            error(
                &common,
                1,
                0,
                "Writing to standard output not supported for thfst transducers,\n\
                 use 'hfst-fst2fst [--output|-o] OUT.thfst' instead",
            );
            return Err(1);
        }

        // here starts the buffer handling part
        // (the C wraps the ctor in try/catch on FileIsInGZFormatException,
        // ImplementationTypeNotAvailableException and HfstException; the Rust
        // ctor currently panics rather than throwing, so the catch arms are not
        // reproduced here.)
        let mut instream = match if input_opened {
            HfstInputStream::new_filename(&common.input_filename)
        } else {
            HfstInputStream::new()
        } {
            Ok(v) => v,
            Err(e) => {
                error(&common, 1, 0, &format!("{e}"));
                return Err(1);
            }
        };

        // DHFST bypasses the transducer output stream: the output is created
        // only once the written bytes have passed the self-check.
        if options.dhfst {
            return cli::from_code(write_dhfst(&common, &options, &mut instream));
        }

        let mut outstream = match if output_opened {
            HfstOutputStream::new_filename(
                &common.output_filename,
                options.output_type,
                options.hfst_format,
            )
        } else {
            HfstOutputStream::new(options.output_type, options.hfst_format)
        } {
            Ok(v) => v,
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

    #[cfg(test)]
    mod tests {
        use super::Args;
        use clap::Parser;

        fn args(argv: &[&str]) -> Args {
            Args::try_parse_from(argv).expect("the options parse")
        }

        // [spec:hfst:def:dhfst.fst2fst/test]
        #[test]
        fn resolves_the_dhfst_options() {
            assert!(!args(&["hfst-fst2fst", "-f", "olw"]).writes_dhfst());
            let default = args(&["hfst-fst2fst", "-f", "DHFST"]);
            assert!(default.writes_dhfst());
            assert_eq!(default.dhfst_depth(), Some(4));
            let bounded = args(&["hfst-fst2fst", "-f", "dhfst", "--max-fallback-depth", "1"]);
            assert_eq!(bounded.dhfst_depth(), Some(1));
            let unbounded = args(&["hfst-fst2fst", "-f", "dhfst", "--unbounded-fallback"]);
            assert_eq!(unbounded.dhfst_depth(), None);
        }
    }
}

pub mod fst2txt {
    //! Faithful 1:1 port of tools/src/hfst-fst2txt.cc — the transducer array
    //! printing command-line tool. Prints a transducer in AT&T, dot, prolog or
    //! pckimmo text format. Option handling is clap 4 derive through
    //! [`crate::cli`].

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{error, hfst_set_program_name, verbose_print};
    use hfst::hfst_data_types::ImplementationType;
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_print_dot::print_dot_file;
    use hfst::hfst_print_pckimmo::print_pckimmo;
    use hfst::hfst_transducer::HfstTransducer;

    // [spec:hfst:def:hfst-fst2txt.fst-text-format]
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum FstTextFormat {
        Att,     // AT&T / OpenFst compatible TSV
        Dot,     // Graphviz / dotty
        Pckimmo, // PCKIMMO format
        Prolog,  // prolog format
    }

    /// hfst-fst2txt's command line.
    // [spec:hfst:def:hfst-fst2txt.parse-options-fn]
    // [spec:hfst:sem:hfst-fst2txt.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "Print transducer in AT&T, dot, prolog or pckimmo format")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,

        /// If weights are printed in all cases
        #[arg(short = 'w', long = "print-weights")]
        print_weights: bool,

        /// If weights are not printed in any case
        #[arg(short = 'D', long = "do-not-print-weights")]
        do_not_print_weights: bool,

        /// Print symbol numbers instead of names
        #[arg(short = 'n', long = "use-numbers")]
        use_numbers: bool,

        /// Print output in TFMT format: att, dot, prolog or pckimmo
        /// [default: att]
        #[arg(short = 'f', long = "format", value_name = "TFMT")]
        format: Option<String>,
    }

    impl Args {
        /// Case 'f': the four text-format vocabularies, fatal on anything else.
        // [spec:hfst:def:hfst-fst2txt.fst-text-format]
        fn text_format(&self, common: &CommonOptions) -> FstTextFormat {
            let Some(name) = self.format.as_deref() else {
                return FstTextFormat::Att;
            };
            match name {
                "att" | "AT&T" | "openfst" | "OpenFst" => FstTextFormat::Att,
                "dot" | "graphviz" | "GraphViz" => FstTextFormat::Dot,
                "pckimmo" => FstTextFormat::Pckimmo,
                "prolog" | "Prolog" => FstTextFormat::Prolog,
                other => {
                    error(
                        common,
                        1,
                        0,
                        &format!(
                            "Cannot parse {} as text format; Use one of att, pckimmo, dot, prolog",
                            other
                        ),
                    );
                    FstTextFormat::Att
                }
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
            // The rejection happened inside the C getopt loop, before the
            // parameter checks.
            self.text_format(opts);
            Ok(())
        }
    }

    /// hfst-fst2txt's resolved tool state (the former tool-specific `static mut`s).
    struct Options {
        use_numbers: bool,
        print_weights: bool,
        do_not_print_weights: bool,
        format: FstTextFormat,
    }

    // [spec:hfst:def:hfst-fst2txt.process-stream-fn]
    // [spec:hfst:sem:hfst-fst2txt.process-stream-fn]
    fn process_stream(
        common: &CommonOptions,
        options: &Options,
        instream: &mut HfstInputStream<'_>,
        outf: &mut dyn std::io::Write,
    ) -> i32 {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            // C: catches TransducerTypeMismatchException -> error "input
            // transducers do not have the same type"; the Rust ctor currently
            // panics rather than throwing, so the catch arm is not reproduced.
            let any = match instream.read() {
                Ok(t) => t,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
            let code =
                crate::for_any!(any, t => process_one(common, options, t, outf, transducer_n));
            if code != 0 {
                return code;
            }
        }
        instream.close();
        0
    }

    // The per-transducer body, generic over the backend (text output only needs
    // the common Backend surface).
    fn process_one<B: hfst::backend::Backend>(
        common: &CommonOptions,
        options: &Options,
        mut t: HfstTransducer<B>,
        outf: &mut dyn std::io::Write,
        transducer_n: usize,
    ) -> i32 {
        {
            let mut inputname = t.get_name();
            if inputname.is_empty() {
                inputname = common.input_filename.clone();
            }
            if transducer_n == 1 {
                verbose_print(common, &format!("Converting {}...\n", inputname));
            } else {
                verbose_print(
                    common,
                    &format!("Converting {}...{}\n", inputname, transducer_n),
                );
            }

            if transducer_n > 1 {
                let _ = outf.write_all(b"--\n");
            }

            let ty = t.get_type();
            // Weights are printed unless explicitly suppressed or the format is a
            // non-weighted one (SFST/foma). Weighted formats — and the
            // "should not happen" fallthrough — both print, so they share the else.
            let printw: bool = if options.print_weights {
                true
            } else {
                !(options.do_not_print_weights
                    || ty == ImplementationType::SFST_TYPE
                    || ty == ImplementationType::FOMA_TYPE)
            };
            let write_result = match options.format {
                FstTextFormat::Att => {
                    if options.use_numbers {
                        t.write_in_att_format_number(outf, printw)
                    } else {
                        t.write_in_att_format_file(outf, printw)
                    }
                }
                FstTextFormat::Dot => outf
                    .write_all(b"// This graph generated with hfst-fst2txt\n")
                    .and_then(|()| print_dot_file(outf, &mut t)),
                FstTextFormat::Pckimmo => print_pckimmo(outf, &t),
                FstTextFormat::Prolog => {
                    // C: catches HfstException -> error "Error encountered when
                    // writing in prolog format". The Rust impl panics; the catch
                    // arm is not reproduced here.
                    let namestr = t.get_name();
                    let alt_namestr = format!("NO_NAME_{}", transducer_n);
                    let namestr = if namestr.is_empty() {
                        if !common.silent {
                            eprintln!(
                                "Transducer has no name, giving it a name '{}'...",
                                alt_namestr
                            );
                        }
                        alt_namestr
                    } else {
                        if !common.silent {
                            eprintln!("Renaming transducer into '{}'...", alt_namestr);
                        }
                        alt_namestr
                    };
                    if let Err(e) = t.write_in_prolog_format(outf, &namestr, printw) {
                        error(
                            common,
                            1,
                            0,
                            &format!("Error encountered when writing in prolog format: {e}"),
                        );
                        return 1;
                    }
                    Ok(())
                }
            };
            if let Err(e) = write_result {
                error(
                    common,
                    1,
                    0,
                    &format!("Error encountered when writing in text format: {e}"),
                );
                return 1;
            }
            // C: delete t; (Rust drops at end of loop iteration).
        }
        0
    }

    // [spec:hfst:def:hfst-fst2txt.main-fn]
    // [spec:hfst:sem:hfst-fst2txt.main-fn]
    pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.3", "HfstFst2Txt");
        let (common, args) = cli::parse::<Args>(common, args)?;
        let options = Options {
            use_numbers: args.use_numbers,
            print_weights: args.print_weights,
            do_not_print_weights: args.do_not_print_weights,
            format: args.text_format(&common),
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
        // (the C wraps the ctor in try/catch on HfstException -> error
        // "%s is not a valid transducer file"; the Rust ctor currently panics
        // rather than throwing, so the catch arm is not reproduced here.)
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

        let mut out = match common.output_writer() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("hfst-fst2txt: cannot open output: {e}");
                return Err(1);
            }
        };
        let retval = process_stream(&common, &options, &mut instream, &mut *out);

        // C: free(inputfilename); free(outfilename); (the foundation owns these
        // allocations; not freed here).
        cli::from_code(retval)
    }
}
