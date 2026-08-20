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
    //! label modification tool for equivalence classes. Drives the hfst-cli
    //! foundation (globals, getopt, commandline, program-options, tool-metadata,
    //! inc fragments).

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        error, error_at_line, extend_options_from_env, hfst_set_program_name,
        is_input_stream_in_ol_format, verbose_print,
    };
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    };
    use crate::inc::{
        CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    };
    use hfst::expand_equivalences::{
        FsaLevel, TsvExtensionError, expand_equivalences, read_tsv_extensions,
    };
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;
    use std::io::Write;

    /// hfst-expand-equivalences's own options (the former tool-specific `static mut`s).
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

    impl Default for Options {
        fn default() -> Options {
            Options {
                only_from_label: None,
                only_to_label: None,
                acx_file_name: None,
                acx_file_opened: false,
                tsv_file_name: None,
                level: FsaLevel::First,
            }
        }
    }

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        let mut msg = common.message_writer();
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let program_name = &common.program_name;
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nExtend transducer arcs for equivalence classes\n\n",
            program_name
        );
        print_common_program_options(&mut *msg);
        let _ = write!(
            msg,
            "Eqv. class extension options:\n\
         \x20 -f, --from=ISYM     convert single symbol ISYM to allow OSYM\n\
         \x20 -t, --to=OSYM       convert to OSYM\n\
         \x20 -a, --acx=ACXFILE   read extensions in acx format from ACXFILE\n\
         \x20 -T, --tsv=TSVFILE   read extensions in tsv format from TSVFILE\n\
         \x20 -l, --level=LEVEL   perform extensions on LEVEL of fsa\n"
        );
        let _ = writeln!(msg);
        let _ = write!(
            msg,
            "Either ACXFILE, TSVFILE or both ISYM and OSYM must be specified.\n\
         LEVEL should be either {{upper, first, 1, input, surface}}, \
         {{lower, second, 2, output, analysis}} or both.\n\
         If LEVEL is omitted, default is first.\n"
        );
        let _ = write!(
            msg,
            "Examples:\n\
         \x20 {} -o rox.hfst -a romanian.acx ro.hfst  extend romanian char\
         equivalences\n\n",
            program_name
        );
    }

    // [spec:hfst:def:hfst-expand-equivalences.parse-options-fn]
    // [spec:hfst:sem:hfst-expand-equivalences.parse-options-fn]
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
                name: "from",
                has_arg: 1, // required_argument
                val: b'f' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "to",
                has_arg: 1,
                val: b't' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "acx",
                has_arg: 1,
                val: b'a' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "tsv",
                has_arg: 1,
                val: b'T' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "level",
                has_arg: 1,
                val: b'l' as i32,
            });
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd common cases, then the tool's
            // own cases, then the terminal error arm.
            match handle_common_case(&mut common, &opt, c, print_usage) {
                CaseResult::Return(code) => return Err(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match c as u8 {
                b'f' => {
                    options.only_from_label = Some(opt.optarg());
                    continue;
                }
                b't' => {
                    options.only_to_label = Some(opt.optarg());
                    continue;
                }
                b'a' => {
                    options.acx_file_name = Some(opt.optarg());
                    continue;
                }
                b'T' => {
                    options.tsv_file_name = Some(opt.optarg());
                    continue;
                }
                b'l' => {
                    let optarg = opt.optarg();
                    if optarg == "first" || optarg == "upper" || optarg == "input" || optarg == "1"
                    {
                        options.level = FsaLevel::First;
                    } else if optarg == "second"
                        || optarg == "lower"
                        || optarg == "output"
                        || optarg == "2"
                    {
                        options.level = FsaLevel::Second;
                    } else if optarg == "both" {
                        options.level = FsaLevel::Both;
                    } else {
                        error(
                            &common,
                            1,
                            0,
                            "The option for level parameter must be one of:\n\
                         upper, first, input; second, lower, output; both, \
                         1 or 2.",
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
                if let Err(e) = outstream.redirect(&mut trans) {
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
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstExpandEquivalences");
        let (common, mut options) = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
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
                return 1;
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
                return 1;
            }
        };

        if is_input_stream_in_ol_format(&instream, "hfst-expand-equivalences") {
            return 1;
        }

        process_stream(&common, &options, &mut instream, &mut outstream);
        instream.close();
        outstream.close();
        0
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

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        extend_options_from_env, hfst_set_program_name, hfst_strformat, verbose_print,
    };
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
        print_common_unary_program_options, print_common_unary_program_parameter_instructions,
    };
    use crate::inc::{CaseResult, handle_common_case, handle_unary_case};
    use hfst::hfst_data_types::ImplementationType;
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_transducer::is_implementation_type_available;
    use std::io::Write;

    /// hfst-format's own options (the former tool-specific `static mut`s).
    #[derive(Default)]
    struct Options {
        /// '-l, --list-formats': list available transducer formats.
        list_formats: bool,
        /// '-t, --test-format FMT': the format to test. C used a NULL char* as
        /// "no format requested"; modelled as Option.
        format_to_test: Option<String>,
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

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f.
        // http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\ndetermine HFST transducer format\n\n",
            common.program_name
        );

        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        let _ = write!(
            msg,
            "Tool-specific options:\n  -l, --list-formats     List available transducer formats\n                         and print them to standard output\n"
        );
        let _ = write!(
            msg,
            "  -t, --test-format FMT  Whether the format FMT is available,\n                         exits with 0 if it is, else with 1\n"
        );
        let _ = writeln!(msg);
        print_common_unary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(msg);
    }

    // [spec:hfst:def:hfst-format.parse-options-fn]
    // [spec:hfst:sem:hfst-format.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
    //
    // This tool does the bulk of its work here (listing formats, testing a format,
    // or opening the input stream to report its type) and returns the (updated)
    // shared options plus the resolved transducer type; the terminal arms
    // `std::process::exit` directly.
    fn parse_options(
        mut common: CommonOptions,
        args: &mut Vec<String>,
    ) -> (CommonOptions, ImplementationType) {
        let mut options = Options::default();
        let mut opt = Getopt::new();
        extend_options_from_env(args);
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            long_options.push(getopt::GetOpt {
                name: "input1",
                has_arg: 1,
                val: '1' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "input2",
                has_arg: 1,
                val: '2' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "list-formats",
                has_arg: 0,
                val: 'l' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "test-format",
                has_arg: 1,
                val: 't' as i32,
            });
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own cases, then the
            // terminal default arm (which here is a no-op, NOT the error arm).
            match handle_common_case(&mut common, &opt, c, print_usage) {
                CaseResult::Return(code) => std::process::exit(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_unary_case(&mut common, &opt, c) {
                CaseResult::Return(code) => std::process::exit(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            let ch = char::from_u32(c as u32);
            match ch {
                Some('1') => {
                    common.input_filename = opt.optarg();
                    continue;
                }
                Some('2') => {
                    common.input_filename = opt.optarg();
                    continue;
                }
                Some('l') => {
                    options.list_formats = true;
                    continue;
                }
                Some('t') => {
                    options.format_to_test = Some(opt.optarg());
                    continue;
                }
                _ => {
                    // I suppose it's crucial for this tool to ignore other options.
                    // Unlike most tools, the default arm here is a genuine no-op
                    // (the C 'default: break;'), NOT the common error handler.
                    continue;
                }
            }
        }

        if let Some(fmt) = options.format_to_test.clone() {
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
                std::process::exit(0);
            }
            std::process::exit(1);
        }

        if options.list_formats {
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
                fput_stdout(" Optimized lookup (weighted)     optimized-lookup-unweighted, olu\n");
            }

            if is_implementation_type_available(ImplementationType::HFST_OLW_TYPE) {
                fput_stdout(
                    " Optimized lookup (unweighted)   optimized-lookup-weighted, olw, optimized-lookup, ol\n",
                );
            }

            if is_implementation_type_available(ImplementationType::THFST_TYPE) {
                fput_stdout(" THFST (divvunspell speller format)          thfst\n");
            }

            std::process::exit(0);
        }

        // (void)inputfilename; (void)inputNamed;

        // The C wraps the stream opening in try/catch on HfstException; on a
        // non-transducer stream it prints an error and exit(1). The Rust ctor
        // currently panics rather than throwing, so the catch arm is mirrored
        // by catching the panic.
        let optind = opt.optind;
        let remaining = args.len() - optind;
        let free_arg = if remaining == 1 {
            Some(args[optind].clone())
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

        match result {
            Ok(Ok((t, resolved))) => {
                common.input_filename = resolved;
                (common, t)
            }
            Ok(Err(_)) | Err(_) => {
                fput_stderr("ERROR: The file/stream does not contain transducers.\n");
                std::process::exit(1);
            }
        }
    }

    // [spec:hfst:def:hfst-format.main-fn]
    // [spec:hfst:sem:hfst-format.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let mut common = hfst_set_program_name(&argv0, "0.1", "HfstFormat");
        common.verbose = true;
        let (common, ty) = parse_options(common, &mut args);
        verbose_print(
            &common,
            &format!(
                "Transducers in {} are of type {}\n",
                common.input_filename,
                hfst_strformat(ty)
            ),
        );
        0
    }
}

pub mod fst2fst {
    //! Faithful 1:1 port of tools/src/hfst-fst2fst.cc — the format conversion
    //! command-line tool. Drives the hfst-cli foundation (globals, getopt,
    //! commandline, program-options, tool-metadata, inc fragments). A unary tool:
    //! it reads one input stream and converts each transducer to another binary
    //! implementation format.
    //!
    //! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
    //! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
    //! `parse_options` and threaded into the processing functions. There are no
    //! `static mut` globals and no `unsafe`.

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        convert_any_with_options, error, extend_options_from_env, hfst_parse_format_name,
        hfst_set_program_name, hfst_strformat, verbose_print, warning,
    };
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
        print_common_unary_program_options, print_common_unary_program_parameter_instructions,
    };
    use crate::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary, hfst_set_name_unary};
    use crate::inc::{
        CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
        handle_unary_case,
    };
    use hfst::hfst_data_types::ImplementationType;
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;
    use std::io::Write;

    /// hfst-fst2fst's own options (the former tool-specific `static mut`s).
    struct Options {
        /// output implementation format ('-f/-S/-F/-t/-l/-O/-w').
        output_type: ImplementationType,
        /// '-b/--use-backend-format': write in implementation format without HFST
        /// wrappers (default: true, i.e. write HFST3 headers).
        hfst_format: bool,
        /// '-Q/--quick': relax optimized-lookup table packing.
        options: String,
    }

    impl Default for Options {
        fn default() -> Options {
            Options {
                output_type: ImplementationType::UNSPECIFIED_TYPE,
                hfst_format: true,
                options: String::new(),
            }
        }
    }

    // [spec:hfst:def:hfst-fst2fst.set-output-type-fn]
    // [spec:hfst:sem:hfst-fst2fst.set-output-type-fn]
    fn set_output_type(common: &CommonOptions, options: &mut Options, ty: ImplementationType) {
        if options.output_type != ImplementationType::UNSPECIFIED_TYPE {
            error(common, 1, 0, "Output type defined several times.");
        }
        options.output_type = ty;
    }

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nConvert transducers between binary formats\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        let _ = write!(
            msg,
            "Conversion options:\n\
         \u{20}\u{20}-f, --format=FMT                  Write result in FMT format\n\
         \u{20}\u{20}-b, --use-backend-format          Write result in implementation format, without any HFST wrappers\n\
         \u{20}\u{20}-S, --sfst                        Write output in (HFST's) SFST implementation\n\
         \u{20}\u{20}-F, --foma                        Write output in (HFST's) foma implementation\n\
         \u{20}\u{20}-x, --xfsm                        Write output in native xfsm format\n\
         \u{20}\u{20}-t, --openfst-tropical            Write output in (HFST's) tropical weight (OpenFST) implementation\n\
         \u{20}\u{20}-O, --optimized-lookup-unweighted Write output in the HFST optimized-lookup implementation\n\
         \u{20}\u{20}-w, --optimized-lookup-weighted   Write output in optimized-lookup (weighted) implementation\n\
         \u{20}\u{20}-Q  --quick                       When converting to optimized-lookup, don't try hard to compress\n\
         \u{20}\u{20}    --format=thfst                Write output as a divvunspell .thfst directory (use -f thfst -o OUT.thfst)\n"
        );
        let _ = writeln!(msg);
        print_common_unary_program_parameter_instructions(&mut *msg);
        let _ = write!(
            msg,
            "FMT must be name of a format usable by libhfst, i.e. one of the following:\n\
         {{ foma, openfst-tropical, sfst, xfsm, thfst\n\
         \u{20}\u{20}optimized-lookup-weighted, optimized-lookup-unweighted }}.\n\
         Note that xfsm format is always written in native format without HFST wrappers,\n\
         and thfst is a directory format written without HFST wrappers (use -o OUT.thfst).\n"
        );
        let _ = writeln!(msg);
    }

    // [spec:hfst:def:hfst-fst2fst.parse-options-fn]
    // [spec:hfst:sem:hfst-fst2fst.parse-options-fn]
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
                name: "use-backend-format",
                has_arg: 0,
                val: b'b' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "format",
                has_arg: 1,
                val: b'f' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "sfst",
                has_arg: 0,
                val: b'S' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "foma",
                has_arg: 0,
                val: b'F' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "xfsm",
                has_arg: 0,
                val: b'x' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "openfst-tropical",
                has_arg: 0,
                val: b't' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "optimized-lookup-unweighted",
                has_arg: 0,
                val: b'O' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "optimized-lookup-weighted",
                has_arg: 0,
                val: b'w' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "quick",
                has_arg: 0,
                val: b'Q' as i32,
            });
            // add tool-specific options here
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own cases, then the
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
            // add tool-specific cases here
            let ch = c as u8;
            match ch {
                b'f' => {
                    let ty = hfst_parse_format_name(&common, &opt.optarg());
                    set_output_type(&common, &mut options, ty);
                    // HAVE_XFSM is not defined in this build: reject xfsm output.
                    if options.output_type == ImplementationType::XFSM_TYPE {
                        error(&common, 1, 0, "xfsm back-end is not available");
                    }
                    continue;
                }
                b'b' => {
                    options.hfst_format = false;
                    continue;
                }
                b'S' => {
                    set_output_type(&common, &mut options, ImplementationType::SFST_TYPE);
                    continue;
                }
                b'F' => {
                    set_output_type(&common, &mut options, ImplementationType::FOMA_TYPE);
                    continue;
                }
                b'x' => {
                    // HAVE_XFSM is not defined in this build.
                    error(&common, 1, 0, "xfsm back-end is not available");
                    continue;
                }
                b't' => {
                    set_output_type(
                        &common,
                        &mut options,
                        ImplementationType::TROPICAL_OPENFST_TYPE,
                    );
                    continue;
                }
                b'O' => {
                    set_output_type(&common, &mut options, ImplementationType::HFST_OL_TYPE);
                    continue;
                }
                b'w' => {
                    set_output_type(&common, &mut options, ImplementationType::HFST_OLW_TYPE);
                    continue;
                }
                b'Q' => {
                    options.options = "quick".to_string();
                    continue;
                }
                _ => {}
            }
            return Err(handle_error_case(&common, &opt, c));
        }

        if options.output_type == ImplementationType::UNSPECIFIED_TYPE {
            error(
                &common,
                1,
                0,
                "You must specify an output type (one of -S, -F, -t, -x, -l, -O, or -w)",
            );
        }

        check_common_params(&mut common);
        check_unary_params(&mut common, &opt, args);
        Ok((common, options))
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
                if let Err(e) = outstream.redirect(&mut orig) {
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
            // needed for xfsm transducers whose writing is delayed
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }
        instream.close();
        outstream.close();
        0
    }

    // [spec:hfst:def:hfst-fst2fst.main-fn]
    // [spec:hfst:sem:hfst-fst2fst.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstFst2Fst");
        let (common, options) = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
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
        if options.hfst_format && (options.output_type != ImplementationType::XFSM_TYPE) {
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

        if options.output_type == ImplementationType::XFSM_TYPE
            && common.output_filename == "<stdout>"
        {
            error(
                &common,
                1,
                0,
                "Writing to standard output not supported for xfsm transducers,\n\
                 use 'hfst-fst2fst [--output|-o] OUTFILE' instead",
            );
            return 1;
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
            return 1;
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
                return 1;
            }
        };

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
                return 1;
            }
        };

        process_stream(&common, &options, &mut instream, &mut outstream)
    }
}

pub mod fst2txt {
    //! Faithful 1:1 port of tools/src/hfst-fst2txt.cc — the transducer array
    //! printing command-line tool. Prints a transducer in AT&T, dot, prolog or
    //! pckimmo text format. Drives the hfst-cli foundation (globals, getopt,
    //! commandline, program-options, inc fragments).

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
    use hfst::hfst_data_types::ImplementationType;
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_print_dot::print_dot_file;
    use hfst::hfst_print_pckimmo::print_pckimmo;
    use hfst::hfst_transducer::HfstTransducer;
    use std::io::Write;

    // [spec:hfst:def:hfst-fst2txt.fst-text-format]
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum FstTextFormat {
        Att,     // AT&T / OpenFst compatible TSV
        Dot,     // Graphviz / dotty
        Pckimmo, // PCKIMMO format
        Prolog,  // prolog format
    }

    /// hfst-fst2txt's own options (the former tool-specific `static mut`s).
    struct Options {
        use_numbers: bool,
        print_weights: bool,
        do_not_print_weights: bool,
        format: FstTextFormat,
    }

    impl Default for Options {
        fn default() -> Options {
            Options {
                use_numbers: false,
                print_weights: false,
                do_not_print_weights: false,
                format: FstTextFormat::Att,
            }
        }
    }

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nPrint transducer in AT&T, dot, prolog or pckimmo format\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        let _ = write!(
            msg,
            "Text format options:\n  -w, --print-weights          If weights are printed in all cases\n  -D, --do-not-print-weights   If weights are not printed in any case\n  -f, --format=TFMT            Print output in TFMT format [default=att]\n"
        );
        let _ = writeln!(msg);
        let _ = write!(
            msg,
            "If OUTFILE or INFILE is missing or -, standard streams will be used.\nUnless explicitly requested with option -w or -D, weights are printed\nif and only if the transducer is in weighted format.\nTFMT is one of {{att, dot, prolog, pckimmo}}.\n"
        );
        let _ = writeln!(msg);
    }

    // [spec:hfst:def:hfst-fst2txt.parse-options-fn]
    // [spec:hfst:sem:hfst-fst2txt.parse-options-fn]
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
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            long_options.push(getopt::GetOpt {
                name: "print-weights",
                has_arg: 0, // no_argument
                val: 'w' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "do-not-print-weights",
                has_arg: 0, // no_argument
                val: 'D' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "use-numbers",
                has_arg: 0, // no_argument
                val: 'n' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "format",
                has_arg: 1, // required_argument
                val: 'f' as i32,
            });
            // add tool-specific options here
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
            match c {
                x if x == 'w' as i32 => {
                    options.print_weights = true;
                    continue;
                }
                x if x == 'D' as i32 => {
                    options.do_not_print_weights = true;
                    continue;
                }
                x if x == 'n' as i32 => {
                    options.use_numbers = true;
                    continue;
                }
                x if x == 'f' as i32 => {
                    let optarg = opt.optarg();
                    if optarg == "att"
                        || optarg == "AT&T"
                        || optarg == "openfst"
                        || optarg == "OpenFst"
                    {
                        options.format = FstTextFormat::Att;
                    } else if optarg == "dot" || optarg == "graphviz" || optarg == "GraphViz" {
                        options.format = FstTextFormat::Dot;
                    } else if optarg == "pckimmo" {
                        options.format = FstTextFormat::Pckimmo;
                    } else if optarg == "prolog" || optarg == "Prolog" {
                        options.format = FstTextFormat::Prolog;
                    } else {
                        error(
                            &common,
                            1,
                            0,
                            &format!(
                                "Cannot parse {} as text format; Use one of att, pckimmo, dot, prolog",
                                optarg
                            ),
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
            let code = crate::for_any!(any, t => process_one(common, options, t, outf, transducer_n, instream.get_type()));
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
        stream_type: ImplementationType,
    ) -> i32 {
        {
            let mut inputname = t.get_name();
            if inputname.is_empty() {
                inputname = common.input_filename.clone();
            }
            if transducer_n == 1 {
                verbose_print(common, &format!("Converting {}...\n", inputname));
            } else {
                if stream_type == ImplementationType::XFSM_TYPE {
                    error(
                        common,
                        1,
                        0,
                        "Writing more than one transducer in text format to file not supported for xfsm transducers,\nuse [hfst-head|hfst-tail|hfst-split] to extract individual transducers from input",
                    );
                    return 1;
                }
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
            // non-weighted one (SFST/foma/xfsm). Weighted formats — and the
            // "should not happen" fallthrough — both print, so they share the else.
            let printw: bool = if options.print_weights {
                true
            } else {
                !(options.do_not_print_weights
                    || ty == ImplementationType::SFST_TYPE
                    || ty == ImplementationType::FOMA_TYPE
                    || ty == ImplementationType::XFSM_TYPE)
            };
            let write_result = match options.format {
                FstTextFormat::Att => {
                    if options.use_numbers {
                        // xfsm case checked earlier
                        t.write_in_att_format_number(outf, printw)
                    } else {
                        // xfsm not yet supported
                        t.write_in_att_format_file(outf, printw)
                    }
                }
                FstTextFormat::Dot => {
                    // xfsm case checked earlier
                    outf.write_all(b"// This graph generated with hfst-fst2txt\n")
                        .and_then(|()| print_dot_file(outf, &mut t))
                }
                FstTextFormat::Pckimmo => {
                    // xfsm case checked earlier
                    print_pckimmo(outf, &t)
                }
                FstTextFormat::Prolog => {
                    // C: catches HfstException -> error "Error encountered when
                    // writing in prolog format". The Rust impl panics; the catch
                    // arm is not reproduced here.
                    if ty == ImplementationType::XFSM_TYPE {
                        // XFSM streams cannot be read in this build (the
                        // backend is compiled out); the C++ arm called
                        // write_xfsm_transducer_in_prolog_format here.
                        unreachable!("XFSM_TYPE cannot be read from an HFST stream in this build")
                    } else {
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
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.3", "HfstFst2Txt");
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
                return 1;
            }
        };

        if instream.get_type() == ImplementationType::XFSM_TYPE {
            if options.format == FstTextFormat::Dot {
                error(
                    &common,
                    1,
                    0,
                    "Output format 'dot' not supported for xfsm transducers, use 'prolog'",
                );
                return 1;
            }
            if options.format == FstTextFormat::Pckimmo {
                error(
                    &common,
                    1,
                    0,
                    "Output format 'pckimmo' not supported for xfsm transducers, use 'prolog'",
                );
                return 1;
            }
            if options.format == FstTextFormat::Att {
                error(
                    &common,
                    1,
                    0,
                    "Output format 'att' not supported for xfsm transducers, use 'prolog'",
                );
                return 1;
            }
            if options.use_numbers {
                error(
                    &common,
                    1,
                    0,
                    "Option '--use-numbers' not supported for xfsm transducers",
                );
                return 1;
            }
            if common.input_filename == "<stdin>" {
                error(
                    &common,
                    1,
                    0,
                    "Reading from standard input not supported for xfsm transducers,\nuse 'hfst-fst2txt [--input|-i] INFILE' instead",
                );
                return 1;
            }
            if common.output_filename == "<stdout>" {
                error(
                    &common,
                    1,
                    0,
                    "Writing to standard output not supported for xfsm transducers,\nuse 'hfst-fst2txt [--output|-o] OUTFILE' instead",
                );
                return 1;
            }
        }

        let mut out = match common.output_writer() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("hfst-fst2txt: cannot open output: {e}");
                return 1;
            }
        };
        let retval = process_stream(&common, &options, &mut instream, &mut *out);

        // C: free(inputfilename); free(outfilename); (the foundation owns these
        // allocations; not freed here).
        retval
    }
}
