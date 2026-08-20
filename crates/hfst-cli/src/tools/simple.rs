//! Unary stream transforms: the tools that read one transducer stream,
//! apply a single algebraic or alphabet operation, and write the result.
//!
//! Contains, as inline modules:
//! - `affix_guessify`
//! - `determinize`
//! - `eliminate_flags`
//! - `insert_freely`
//! - `invert`
//! - `kill_paths`
//! - `minimize`
//! - `multiply`
//! - `preprocess_for_optimized_lookup_format`
//! - `project`
//! - `prune_alphabet`
//! - `push_labels`
//! - `push_weights`
//! - `realign`
//! - `remove_epsilons`
//! - `repeat`
//! - `reverse`

pub mod affix_guessify {
    //! Faithful 1:1 port of tools/src/hfst-affix-guessify.cc — the transducer
    //! guesser maker command-line tool. Creates a weighted affix guesser from an
    //! automaton. Drives the hfst-cli foundation (globals, getopt, commandline,
    //! program-options, tool-metadata, inc fragments).

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        error, extend_options_from_env, hfst_set_program_name, hfst_strtoweight,
        is_input_stream_in_ol_format, verbose_print,
    };
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
        print_common_unary_program_options, print_common_unary_program_parameter_instructions,
    };
    use crate::inc::{
        CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
        handle_unary_case,
    };
    use hfst::guessify_fst::{GuessDirection, affix_guessify};
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;
    use std::io::Write;

    /// hfst-affix-guessify's own options (the former tool-specific `static mut`s).
    ///
    /// GuessDirection and the per-transducer affix-guesser construction now live in
    /// hfst::guessify_fst; this tool keeps only the option-driven state + the
    /// stream-driver loop.
    struct Options {
        /// '-D, --direction=DIR': direction of guessing.
        direction: GuessDirection,
        /// '-w, --weight=WEIGHT': weight difference of affix lengths.
        weight: f32,
    }

    impl Default for Options {
        fn default() -> Options {
            Options {
                direction: GuessDirection::GuessSuffix,
                weight: 1.0f32,
            }
        }
    }

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nCreate weighted affix guesser from automaton\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        // (tool-specific options and short descriptions)
        let _ = write!(
            msg,
            "Guesser parameters:\n  -D, --direction=DIR   set direction of guessing\n  -w, --weight=WEIGHT   set weight difference of affix lengths\n\n"
        );
        let _ = writeln!(msg);
        print_common_unary_program_parameter_instructions(&mut *msg);
        let _ = write!(
            msg,
            "DIR is either suffix or prefix, or suffix if omitted.\nWEIGHT is a weight of each arc not in the known suffix or prefix being guessed, as parsed with strtod(3), or 1.0 if omitted.\n"
        );
        let _ = writeln!(msg);
    }

    // [spec:hfst:def:hfst-affix-guessify.parse-options-fn]
    // [spec:hfst:sem:hfst-affix-guessify.parse-options-fn]
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
                name: "weight",
                has_arg: 1, // required_argument
                val: 'w' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "direction",
                has_arg: 1, // required_argument
                val: 'D' as i32,
            });
            // add tool-specific options here
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own ('w'/'D'), then the
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
                x if x == 'w' as i32 => {
                    options.weight = hfst_strtoweight(&common, &opt.optarg());
                    continue;
                }
                x if x == 'D' as i32 => {
                    let optarg = opt.optarg();
                    if optarg.starts_with("prefix") {
                        options.direction = GuessDirection::GuessPrefix;
                    } else if optarg.starts_with("suffix") {
                        options.direction = GuessDirection::GuessSuffix;
                    } else {
                        error(
                            &common,
                            1,
                            0,
                            &format!(
                                "Unable to parse guessing direction from {};\nplease use one of 'prefix' or 'suffix'",
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

    // [spec:hfst:def:hfst-affix-guessify.process-stream-fn]
    // [spec:hfst:sem:hfst-affix-guessify.process-stream-fn]
    fn process_stream(
        common: &CommonOptions,
        options: &Options,
        instream: &mut HfstInputStream<'_>,
        outstream: &mut HfstOutputStream,
    ) -> i32 {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let any = match instream.read() {
                Ok(v) => v,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
            crate::for_algebra!(any, trans => {
                // C: inputname = trans->get_name(); if empty, use inputfilename.
                let inputname = if !trans.get_name().is_empty() {
                    trans.get_name()
                } else {
                    common.input_filename.clone()
                };
                if transducer_n < 2 {
                    verbose_print(common, &format!("Guessifying {}...\n", inputname));
                } else {
                    verbose_print(common, &format!("Guessifying {}... {}\n", inputname, transducer_n));
                }
                let mut t = match affix_guessify(&trans, options.direction, options.weight) {
                    Ok(t) => t,
                    Err(e) => {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                };
                if let Err(e) = outstream.redirect(&mut t) {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            }, else => {
                // Unreachable: the optimized-lookup stream rejection already
                // returned before the loop; keep its text for safety.
                let _ = writeln!(
                    std::io::stderr(),
                    "Error: hfst-affix-guessify cannot process transducers that are in optimized lookup format."
                );
                return 1;
            });
        } // good instream
        0
    }

    // [spec:hfst:def:hfst-affix-guessify.main-fn]
    // [spec:hfst:sem:hfst-affix-guessify.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstAffixGuessify");
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

        // here starts the buffer handling part
        // (the C wraps the ctor in try/catch on HfstException reporting
        // "%s is not a valid transducer file"; the Rust ctor currently panics on
        // a bad file rather than throwing, so the catch arm is not reproduced.)
        let instream_res = if input_opened {
            HfstInputStream::new_filename(&common.input_filename)
        } else {
            HfstInputStream::new()
        };
        let mut instream = match instream_res {
            Ok(s) => s,
            Err(e) => {
                error(&common, 1, 0, &format!("{e}"));
                return 1;
            }
        };

        let ty = instream.get_type();
        let outstream_res = if output_opened {
            HfstOutputStream::new_filename(&common.output_filename, ty, true)
        } else {
            HfstOutputStream::new(ty, true)
        };
        let mut outstream = match outstream_res {
            Ok(s) => s,
            Err(e) => {
                error(&common, 1, 0, &format!("{e}"));
                return 1;
            }
        };

        if is_input_stream_in_ol_format(&instream, "hfst-affix-guessify") {
            return 1;
        }

        process_stream(&common, &options, &mut instream, &mut outstream)
    }
}

pub mod determinize {
    //! Faithful 1:1 port of tools/src/hfst-determinize.cc — the transducer
    //! determinisation command-line tool. Drives the hfst-cli foundation (globals,
    //! getopt, commandline, program-options, tool-metadata, inc fragments).
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
        print_common_unary_program_options, print_common_unary_program_parameter_instructions,
    };
    use crate::inc::{
        CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
        handle_unary_case,
    };
    use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_transducer::{EngineConfig, HfstTransducer};
    use std::borrow::Cow;
    use std::io::Write;

    /// hfst-determinize's own options (the former tool-specific `static mut`s).
    #[derive(Default)]
    struct Options {
        /// '-E, --encode-weights': encode weights when determinizing.
        encode_weights: bool,
    }

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nDeterminize a transducer\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        let _ = writeln!(msg, "Command-specific options:");
        let _ = write!(
            msg,
            "  -E, --encode-weights         Encode weights when determinizing\n\
         \x20                             (default is false).\n\n"
        );
        let _ = writeln!(msg);
        print_common_unary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(msg);
    }

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
                name: "encode-weights",
                has_arg: getopt::NO_ARGUMENT,
                val: 'E' as i32,
            });
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, unary cases, the terminal error arm, then the tool's own
            // 'E' case.
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
            if c == 'E' as i32 {
                options.encode_weights = true;
                continue;
            }
            return Err(handle_error_case(&common, &opt, c));
        }

        check_common_params(&mut common);
        check_unary_params(&mut common, &opt, args);
        Ok((common, options))
    }

    // [spec:hfst:def:hfst-determinize.process-stream-fn]
    // [spec:hfst:sem:hfst-determinize.process-stream-fn]
    //
    // The stream loop lives in the shared unary driver; this op is the
    // per-transducer body it dispatches into.
    struct DeterminizeOp {
        encode_weights: bool,
    }

    impl UnaryToolOp for DeterminizeOp {
        fn verbose_begin(&self, inputname: &str) -> String {
            format!("Determinizing {}", inputname)
        }

        fn name_op(&self) -> Option<Cow<'_, str>> {
            Some(Cow::Borrowed("determinize"))
        }

        fn formula(&self) -> Option<Cow<'_, str>> {
            Some(Cow::Borrowed("\u{2336}"))
        }

        fn apply<B: AlgebraBackend>(
            &mut self,
            _common: &CommonOptions,
            t: &mut HfstTransducer<B>,
        ) -> hfst::error::Result<()> {
            t.determinize_with_config(&EngineConfig {
                encode_weights: self.encode_weights,
                ..EngineConfig::default()
            })
            .map(|_| ())
        }
    }

    const SPEC: UnaryOpSpec = UnaryOpSpec {
        tool_name: "hfst-determinize",
        reject_ol: true,
    };

    // [spec:hfst:def:hfst-determinize.main-fn]
    // [spec:hfst:sem:hfst-determinize.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstDeterminize");
        let (common, options) = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };

        let mut op = DeterminizeOp {
            encode_weights: options.encode_weights,
        };
        run_unary_tool(&common, &SPEC, &mut op)
    }
}

pub mod eliminate_flags {
    //! Port of tools/src/hfst-eliminate-flags.cc — the transducer flag elimination
    //! command-line tool. Drives the hfst-cli foundation (getopt, commandline,
    //! program-options, tool-metadata, inc fragments).
    //!
    //! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
    //! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
    //! `parse_options` and threaded into the processing functions. There are no
    //! `static mut` globals and no `unsafe`.

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{error, extend_options_from_env, hfst_set_program_name};
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
        print_common_unary_program_options, print_common_unary_program_parameter_instructions,
    };
    use crate::inc::{
        CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
        handle_unary_case,
    };
    use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_transducer::HfstTransducer;
    use std::borrow::Cow;
    use std::io::Write;

    /// hfst-eliminate-flags's own options (the former tool-specific `static mut`s).
    #[derive(Default)]
    struct Options {
        /// '-F, --flag=FLAG': only eliminate flag FLAG (else all flags).
        flag: Option<String>,
    }

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        // Usage line
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nEliminate flags from a transducer\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        let _ = writeln!(msg, "Command-specific options:");
        let _ = write!(msg, "  -F, --flag=FLAG        Only eliminate flag FLAG\n\n");
        let _ = writeln!(msg);
        print_common_unary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(msg);
    }

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
            long_options.push(getopt::GetOpt {
                name: "flag",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: 'F' as i32,
            });
            // add tool-specific options here
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own ('F'), then the
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
            if c == 'F' as i32 {
                options.flag = Some(opt.optarg());
                continue;
            }
            return Err(handle_error_case(&common, &opt, c));
        }

        check_common_params(&mut common);
        check_unary_params(&mut common, &opt, args);
        Ok((common, options))
    }

    // [spec:hfst:def:hfst-eliminate-flags.process-stream-fn]
    // [spec:hfst:sem:hfst-eliminate-flags.process-stream-fn]
    //
    // The stream loop lives in the shared unary driver; this op is the
    // per-transducer body it dispatches into. The verbose verb names what is being
    // eliminated ("flags" or "flag FLAG"), which the C computes once before the
    // loop; here it is the op's own precomputed field.
    struct EliminateFlagsOp {
        /// '-F, --flag=FLAG', if given.
        flag: Option<String>,
        /// The verbose line's object: "flags", or "flag FLAG".
        flags: String,
    }

    impl UnaryToolOp for EliminateFlagsOp {
        fn verbose_begin(&self, inputname: &str) -> String {
            // The C additionally falls back to the input filename on an empty
            // transducer name, which hfst_get_name has already done: it returns the
            // filename whenever the name is empty, so the guard could only ever
            // re-substitute the same empty filename.
            format!("Eliminating {} {}", self.flags, inputname)
        }

        fn name_op(&self) -> Option<Cow<'_, str>> {
            Some(Cow::Borrowed("eliminate-flags"))
        }

        fn formula(&self) -> Option<Cow<'_, str>> {
            Some(Cow::Borrowed("Id"))
        }

        fn apply<B: AlgebraBackend>(
            &mut self,
            common: &CommonOptions,
            t: &mut HfstTransducer<B>,
        ) -> hfst::error::Result<()> {
            match &self.flag {
                None => t.eliminate_flags().map(|_| ()),
                Some(f) => {
                    if t.eliminate_flag(f).is_err() {
                        // The single-flag failure substitutes the tool's own text
                        // for the error value's, so it is reported here rather than
                        // through the driver's '{e}' path. `error` with a non-zero
                        // status exits the process, so the Err below is never
                        // observed; it stands for the C's `return 1`.
                        error(
                            common,
                            1,
                            0,
                            &format!(
                                "flag feature {} does not occur in the transducer\nonly the flag feature must be given, no value or operator",
                                f
                            ),
                        );
                        return Err(hfst::error::Error::new(hfst::error::ErrorKind::Fatal));
                    }
                    Ok(())
                }
            }
        }
    }

    const SPEC: UnaryOpSpec = UnaryOpSpec {
        tool_name: "hfst-eliminate-flags",
        reject_ol: true,
    };

    // [spec:hfst:def:hfst-eliminate-flags.main-fn]
    // [spec:hfst:sem:hfst-eliminate-flags.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstEliminateFlags");
        let (common, options) = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };

        let flags = match &options.flag {
            None => String::from("flags"),
            Some(f) => format!("flag {}", f),
        };
        let mut op = EliminateFlagsOp {
            flag: options.flag,
            flags,
        };
        run_unary_tool(&common, &SPEC, &mut op)
    }
}

pub mod insert_freely {
    //! Faithful 1:1 port of tools/src/hfst-insert-freely.cc — the freely-insert
    //! a symbol (pair) command-line tool. Drives the hfst-cli foundation (globals,
    //! getopt, commandline, program-options, tool-metadata, inc fragments).
    //!
    //! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
    //! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
    //! `parse_options` and threaded into the processing functions. There are no
    //! `static mut` globals and no `unsafe`.

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        error, extend_options_from_env, hfst_set_program_name, is_input_stream_in_ol_format,
        verbose_print,
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
    use hfst::hfst_data_types::StringPair;
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;
    use hfst::hfst_symbol_defs::{internal_epsilon, label_to_stringpair};
    use std::io::Write;

    /// hfst-insert-freely's own options (the former tool-specific `static mut`s).
    #[derive(Default)]
    struct Options {
        label: Option<String>,
        harmonise_flags: bool,
        symbol_pair: Option<StringPair>,
    }

    // FMT: Copied from hfst-substitute.cc ... should probably go in a library function

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        // Usage line
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nFreely insert a symbol (pair)\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        let _ = write!(
            msg,
            "Option:\n  -a, --symbol-pair=SYM   symbol pair SYM\n  -H, --harmonise   harmonise \n"
        );
        let _ = writeln!(msg);
        print_common_unary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(
            msg,
            "SYM must be either a single alphabeticsymbol or two symbols separated by a colon, :"
        );
        let _ = writeln!(msg);
    }

    // [spec:hfst:def:hfst-insert-freely.parse-options-fn]
    // [spec:hfst:sem:hfst-insert-freely.parse-options-fn]
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
                name: "symbol-pair",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: 'a' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "harmonise",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: 'H' as i32,
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
            match c as u8 {
                b'a' => {
                    // This will probably break for unicode
                    let mut lbl = opt.optarg();
                    if lbl == "@0@" {
                        lbl = internal_epsilon.to_string();
                    }
                    options.symbol_pair = label_to_stringpair(&lbl);
                    if lbl.is_empty() {
                        error(
                            &common,
                            1,
                            0,
                            &format!(
                                "argument of source label option is empty;\nif you REALLY want to replace epsilons with something, use @0@ or {}",
                                internal_epsilon
                            ),
                        );
                    }
                    options.label = Some(lbl);
                    continue;
                }
                b'H' => {
                    options.harmonise_flags = true;
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

    // [spec:hfst:def:hfst-insert-freely.process-stream-fn]
    // [spec:hfst:sem:hfst-insert-freely.process-stream-fn]
    fn process_stream(
        common: &CommonOptions,
        options: &Options,
        instream: &mut HfstInputStream<'_>,
        outstream: &mut HfstOutputStream,
    ) -> i32 {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let any = match instream.read() {
                Ok(v) => v,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
            crate::for_algebra!(any, trans => {
                let mut trans = trans;
                let _inputname = hfst_get_name(&trans, &common.input_filename);
                if transducer_n == 1 {
                    // If harmonize is true, then identity and unknown symbols in the
                    // transducer will be expanded by the symbols in symbol pair.
                    // Otherwise they aren't.
                    let pair = options.symbol_pair.as_ref().expect("symbol pair must be set");
                    if let Err(e) = trans.insert_freely_pair(pair, options.harmonise_flags) {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                    // C: hfst_set_name(trans, trans, "insert-freely") and
                    // hfst_set_formula(trans, trans, "Id"); dest and src are the
                    // same object, so the read side is taken from a copy.
                    let src = trans.clone();
                    hfst_set_name_unary(&mut trans, &src, "insert-freely");
                    hfst_set_formula_unary(&mut trans, &src, "Id");
                }
                if let Err(e) = outstream.redirect(&mut trans) {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            }, else => {
                // Unreachable: the optimized-lookup stream rejection already
                // returned before the loop; keep its text for safety.
                let _ = writeln!(
                    std::io::stderr(),
                    "Error: hfst-insert-freely cannot process transducers that are in optimized lookup format."
                );
                return 1;
            });
        }
        instream.close();
        outstream.close();
        0
    }

    // [spec:hfst:def:hfst-insert-freely.main-fn]
    // [spec:hfst:sem:hfst-insert-freely.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstPush");
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

        if is_input_stream_in_ol_format(&instream, "hfst-insert-freely") {
            return 1;
        }

        process_stream(&common, &options, &mut instream, &mut outstream)
    }
}

pub mod invert {
    //! Faithful 1:1 port of tools/src/hfst-invert.cc — the transducer inversion
    //! command-line tool. Drives the hfst-cli foundation (getopt, commandline,
    //! program-options, tool-metadata, inc fragments).
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
        print_common_unary_program_options, print_common_unary_program_parameter_instructions,
    };
    use crate::inc::{
        CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
        handle_unary_case,
    };
    use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_transducer::HfstTransducer;
    use std::borrow::Cow;
    use std::io::Write;

    /// hfst-invert's own options. The tool has no tool-specific `static mut`s, so
    /// this is empty and carries the type-level marker only.
    #[derive(Default)]
    struct Options;

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nInvert a transducer\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        let _ = writeln!(msg);
        print_common_unary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(msg);
    }

    // [spec:hfst:req:cli.arg-parse]
    //
    // Parse argv into the shared + tool options; `Err(code)` is an exit code the
    // caller should return (the former EXIT_CONTINUE sentinel is now `Ok`).
    fn parse_options(
        mut common: CommonOptions,
        args: &mut Vec<String>,
    ) -> Result<(CommonOptions, Options), i32> {
        let options = Options;
        let mut opt = Getopt::new();
        extend_options_from_env(args);
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own (none here), then the
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
            return Err(handle_error_case(&common, &opt, c));
        }

        check_common_params(&mut common);
        check_unary_params(&mut common, &opt, args);
        Ok((common, options))
    }

    // [spec:hfst:def:hfst-invert.process-stream-fn]
    // [spec:hfst:sem:hfst-invert.process-stream-fn]
    //
    // The stream loop lives in the shared unary driver; this op is the
    // per-transducer body it dispatches into.
    struct InvertOp;

    impl UnaryToolOp for InvertOp {
        fn verbose_begin(&self, inputname: &str) -> String {
            format!("Inverting {}", inputname)
        }

        fn name_op(&self) -> Option<Cow<'_, str>> {
            Some(Cow::Borrowed("invert"))
        }

        fn formula(&self) -> Option<Cow<'_, str>> {
            Some(Cow::Borrowed("\u{207b}\u{00b9}"))
        }

        fn apply<B: AlgebraBackend>(
            &mut self,
            _common: &CommonOptions,
            t: &mut HfstTransducer<B>,
        ) -> hfst::error::Result<()> {
            t.invert().map(|_| ())
        }
    }

    const SPEC: UnaryOpSpec = UnaryOpSpec {
        tool_name: "hfst-invert",
        reject_ol: true,
    };

    // [spec:hfst:def:hfst-invert.main-fn]
    // [spec:hfst:sem:hfst-invert.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstInvert");
        let (common, _options) = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };

        run_unary_tool(&common, &SPEC, &mut InvertOp)
    }
}

pub mod kill_paths {
    //! Faithful 1:1 port of tools/src/hfst-kill-paths.cc — the path-killing
    //! command-line tool: removes every arc whose input or output symbol matches a
    //! given symbol (one --symbol, or a list from a --tsv-file), then removes
    //! epsilons. Drives the hfst-cli foundation (globals, getopt, commandline,
    //! program-options, tool-metadata, inc fragments).

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        error, extend_options_from_env, hfst_set_program_name, is_input_stream_in_ol_format,
        verbose_print,
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
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;
    use hfst::hfst_transducer::HfstTransducer;
    use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};

    /// hfst-kill-paths's own options (the former tool-specific `static mut`s).
    #[derive(Default)]
    struct Options {
        /// '-S, --symbol=SYM': the symbol whose arcs to kill.
        symbol: Option<String>,
        /// '-T, --tsv-file=TFILE': the file listing kill symbols.
        tsv_file_name: Option<String>,
        /// The opened kill-rules file (from `tsv_file_name`).
        tsv_file: Option<std::fs::File>,
    }

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        let mut msg = common.message_writer();
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        // Usage line
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nKill all paths with specific symbols\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        let _ = write!(
            msg,
            "Reweighting options:\n  -S, --symbol=SYM           remove arcs with input or output symbol SYM or both\n  -T, --tsv-file=TFILE       read kill rules from TFILE\n\n"
        );
        let _ = writeln!(msg);
        print_common_unary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(
            msg,
            "TFILE should contain lines with tab-separated pairs of SYM and Comment lines starting with # and empty lines are ignored."
        );
        let _ = writeln!(msg);
    }

    // [spec:hfst:def:hfst-kill-paths.parse-options-fn]
    // [spec:hfst:sem:hfst-kill-paths.parse-options-fn]
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
                name: "symbol",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: 'S' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "tsv",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: 'T' as i32,
            });
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own ('S'/'T'), then the
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
            if c == 'S' as i32 {
                options.symbol = Some(opt.optarg());
                continue;
            }
            if c == 'T' as i32 {
                options.tsv_file_name = Some(opt.optarg());
                continue;
            }
            return Err(handle_error_case(&common, &opt, c));
        }

        if options.symbol.is_none() && options.tsv_file_name.is_none() {
            error(&common, 1, 0, "Either --symbol or --tsv-file is required");
            return Err(1);
        }

        check_common_params(&mut common);
        check_unary_params(&mut common, &opt, args);
        if let Some(name) = &options.tsv_file_name {
            match std::fs::File::open(name) {
                Ok(f) => options.tsv_file = Some(f),
                Err(_) => {
                    error(&common, 1, 0, &format!("Could not open '{}'", name));
                    return Err(1);
                }
            }
        }
        Ok((common, options))
    }

    // [spec:hfst:def:hfst-kill-paths.original-fn]
    // [spec:hfst:sem:hfst-kill-paths.original-fn]
    fn do_killing<B: hfst::backend::AlgebraBackend>(
        symbol: Option<&str>,
        trans: &mut HfstTransducer<B>,
    ) {
        let symbol = symbol.unwrap_or_default();
        *trans = trans.kill_paths(symbol);
    }

    // [spec:hfst:def:hfst-kill-paths.process-stream-fn]
    // [spec:hfst:sem:hfst-kill-paths.process-stream-fn]
    fn process_stream(
        common: &CommonOptions,
        options: &mut Options,
        instream: &mut HfstInputStream<'_>,
        outstream: &mut HfstOutputStream,
    ) -> i32 {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let any = match instream.read() {
                Ok(v) => v,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
            crate::for_algebra!(any, trans => {
                let mut trans = trans;
                let inputname = hfst_get_name(&trans, &common.input_filename);
                if transducer_n == 1 {
                    verbose_print(common, &format!("Path killing {}...\n", inputname));
                } else {
                    verbose_print(common, &format!("Path killing {}...{}\n", inputname, transducer_n));
                }
                if options.tsv_file.is_none() {
                    do_killing(options.symbol.as_deref(), &mut trans);
                    // C: hfst_set_name(trans, trans, "pathkill"); dest and src are the
                    // same object, which Rust cannot alias mut+const, so the read side
                    // is taken from a copy (name/formula are unchanged by the copy).
                    let src = trans.clone();
                    hfst_set_name_unary(&mut trans, &src, "pathkill");
                    hfst_set_formula_unary(&mut trans, &src, "PK");
                } else {
                    // C: rewind(tsv_file) — seek the std file back to the start.
                    if let Some(tsv_file) = options.tsv_file.as_mut() {
                        let _ = tsv_file.seek(SeekFrom::Start(0));
                    }
                    options.symbol = None;
                    let mut _linen: usize = 0;
                    verbose_print(common, &format!(
                        "Reading reweights from {}\n",
                        options.tsv_file_name.clone().unwrap_or_default()
                    ));
                    if let Some(tsv_file) = options.tsv_file.as_mut() {
                        let mut reader = BufReader::new(tsv_file);
                        let mut line = String::new();
                        loop {
                            line.clear();
                            // C: hfst_getline keeps the trailing newline; Ok(0) at EOF.
                            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                                break;
                            }
                            _linen += 1;
                            let bytes = line.as_bytes();
                            if bytes.first() == Some(&b'\n') {
                                continue;
                            }
                            if bytes.first() == Some(&b'#') {
                                continue;
                            }
                            // const char *endptr = line; advance to '\0' or '\n'
                            let mut endptr = 0usize;
                            while endptr < bytes.len() && bytes[endptr] != b'\n' {
                                endptr += 1;
                            }
                            let sym = String::from_utf8_lossy(&bytes[..endptr]).into_owned();
                            verbose_print(common, &format!("Killing patsh with symbol {}\n", sym));
                            do_killing(Some(&sym), &mut trans);
                        } // getline
                    }
                    let src = trans.clone();
                    hfst_set_name_unary(&mut trans, &src, "pathkill");
                    hfst_set_formula_unary(&mut trans, &src, "PK");
                } // if tsv_file
                let reduced = match trans.remove_epsilons() {
                    Ok(t) => t,
                    Err(e) => {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                };
                if let Err(e) = outstream.redirect(reduced) {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            }, else => {
                // Unreachable: the optimized-lookup stream rejection already
                // returned before the loop; keep its text for safety.
                let _ = writeln!(
                    std::io::stderr(),
                    "Error: hfst-kill-paths cannot process transducers that are in optimized lookup format."
                );
                return 1;
            });
        } // foreach transducer
        instream.close();
        outstream.close();
        0
    }

    // [spec:hfst:def:hfst-kill-paths.main-fn]
    // [spec:hfst:sem:hfst-kill-paths.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstKillPaths");
        let (common, mut options) = match parse_options(common, &mut args) {
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
        verbose_print(&common, "Killing paths\n");
        if let Some(sym) = &options.symbol {
            verbose_print(&common, &format!("only if arc has symbol {}\n", sym));
        }

        // here starts the buffer handling part
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
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)

        let ty = instream.get_type();
        let mut outstream = match if output_opened {
            HfstOutputStream::new_filename(&common.output_filename, ty, true)
        } else {
            HfstOutputStream::new(ty, true)
        } {
            Ok(s) => s,
            Err(e) => {
                error(&common, 1, 0, &format!("{e}"));
                return 1;
            }
        };

        if is_input_stream_in_ol_format(&instream, "hfst-kill-paths") {
            return 1;
        }

        process_stream(&common, &mut options, &mut instream, &mut outstream)
    }
}

pub mod minimize {
    //! Port of tools/src/hfst-minimize.cc — the transducer minimisation
    //! command-line tool. Drives the hfst-cli foundation (getopt, commandline,
    //! program-options, tool-metadata, inc fragments).
    //!
    //! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
    //! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
    //! `parse_options` and threaded into the processing functions. There are no
    //! `static mut` globals and no `unsafe`. This is the template the other tools
    //! follow.

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{extend_options_from_env, hfst_set_program_name};
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
        print_common_unary_program_options, print_common_unary_program_parameter_instructions,
    };
    use crate::inc::{
        CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
        handle_unary_case,
    };
    use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_transducer::{EngineConfig, HfstTransducer};
    use std::borrow::Cow;
    use std::io::Write;

    /// hfst-minimize's own options (the former tool-specific `static mut`s).
    #[derive(Default)]
    struct Options {
        /// '-E, --encode-weights': encode weights when minimizing.
        encode_weights: bool,
    }

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nMinimize a transducer\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        let _ = writeln!(msg, "Command-specific options:");
        let _ = write!(
            msg,
            "  -E, --encode-weights         Encode weights when minimizing\n                               (default is false).\n\n"
        );
        print_common_unary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(msg);
    }

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
                name: "encode-weights",
                has_arg: getopt::NO_ARGUMENT,
                val: 'E' as i32,
            });
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, error case, then unary cases, then the tool's own ('E').
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
            if c == 'E' as i32 {
                options.encode_weights = true;
                continue;
            }
            return Err(handle_error_case(&common, &opt, c));
        }

        check_common_params(&mut common);
        check_unary_params(&mut common, &opt, args);
        Ok((common, options))
    }

    // [spec:hfst:def:hfst-minimize.process-stream-fn]
    // [spec:hfst:sem:hfst-minimize.process-stream-fn]
    //
    // The stream loop lives in the shared unary driver; this op is the
    // per-transducer body it dispatches into.
    struct MinimizeOp {
        encode_weights: bool,
    }

    impl UnaryToolOp for MinimizeOp {
        fn verbose_begin(&self, inputname: &str) -> String {
            format!("Minimizing {}", inputname)
        }

        fn name_op(&self) -> Option<Cow<'_, str>> {
            Some(Cow::Borrowed("minimize"))
        }

        fn formula(&self) -> Option<Cow<'_, str>> {
            Some(Cow::Borrowed("M"))
        }

        fn apply<B: AlgebraBackend>(
            &mut self,
            _common: &CommonOptions,
            t: &mut HfstTransducer<B>,
        ) -> hfst::error::Result<()> {
            t.minimize_with_config(&EngineConfig {
                encode_weights: self.encode_weights,
                ..EngineConfig::default()
            })
            .map(|_| ())
        }
    }

    const SPEC: UnaryOpSpec = UnaryOpSpec {
        tool_name: "hfst-minimize",
        reject_ol: true,
    };

    // [spec:hfst:def:hfst-minimize.main-fn]
    // [spec:hfst:sem:hfst-minimize.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstMinimize");
        let (common, options) = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };

        let mut op = MinimizeOp {
            encode_weights: options.encode_weights,
        };
        run_unary_tool(&common, &SPEC, &mut op)
    }
}

pub mod multiply {
    //! Faithful 1:1 port of tools/src/hfst-multiply.cc — the transducer archive
    //! duplication tool (writes the first transducer of an archive repeatedly).
    //! Drives the hfst-cli foundation (globals, getopt, commandline,
    //! program-options, tool-metadata, inc fragments).
    //!
    //! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
    //! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
    //! `parse_options` and threaded into the processing functions. There are no
    //! `static mut` globals and no `unsafe`.

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        extend_options_from_env, hfst_set_program_name, is_input_stream_in_ol_format, parse_u64,
        verbose_print,
    };
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
        print_common_unary_program_options, print_common_unary_program_parameter_instructions,
    };
    use crate::inc::{
        CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
        handle_unary_case,
    };
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;
    use std::io::Write;

    /// hfst-multiply's own options (the former tool-specific `static mut`s).
    struct Options {
        /// '-n, --n-times': duplicate each transducer this many times.
        dupe_count: u64,
    }

    impl Default for Options {
        fn default() -> Self {
            Options { dupe_count: 1 }
        }
    }

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nUse first transducer of an archive repeatedly\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        let _ = write!(
            msg,
            "Archive options:\n  -n, --n-last=NUMBER   Duplicate each transducer NUMBER times\n"
        );
        let _ = writeln!(msg);
        print_common_unary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(
            msg,
            "NUMBER must be a positive integer as parsed by strtoul base 10"
        );
        let _ = writeln!(msg);
    }

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
            long_options.push(getopt::GetOpt {
                name: "n-times",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: 'n' as i32,
            });
            // add tool-specific options here
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own ('n'), then the
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
            if c == 'n' as i32 {
                options.dupe_count = parse_u64(&common, &opt.optarg(), 10);
                continue;
            }
            return Err(handle_error_case(&common, &opt, c));
        }

        check_common_params(&mut common);
        check_unary_params(&mut common, &opt, args);
        Ok((common, options))
    }

    // [spec:hfst:def:hfst-multiply.process-stream-fn]
    // [spec:hfst:sem:hfst-multiply.process-stream-fn]
    fn process_stream(
        common: &CommonOptions,
        options: &Options,
        instream: &mut HfstInputStream<'_>,
        outstream: &mut HfstOutputStream,
    ) -> i32 {
        // C declares 'queue<HfstTransducer> last_n;' here but never uses it.
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let any = match instream.read() {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("hfst-multiply: {e}");
                    return 1;
                }
            };
            // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
            crate::for_algebra!(any, trans => {
                let mut trans = trans;
                let mut inputname = trans.get_name();
                if inputname.is_empty() {
                    inputname = common.input_filename.clone();
                }

                verbose_print(common, &format!(
                    "Duplicate {} times {}...{}\n",
                    inputname, options.dupe_count, transducer_n
                ));
                for _ in 0..options.dupe_count {
                    if let Err(e) = outstream.redirect(&mut trans) {
                        eprintln!("hfst-multiply: {e}");
                        return 1;
                    }
                }
            }, else => {
                // Unreachable: the optimized-lookup stream rejection already
                // returned before the loop; keep its text for safety.
                let _ = writeln!(
                    std::io::stderr(),
                    "Error: hfst-multiply cannot process transducers that are in optimized lookup format."
                );
                return 1;
            });
        }
        instream.close();
        outstream.close();
        0
    }

    // [spec:hfst:def:hfst-multiply.main-fn]
    // [spec:hfst:sem:hfst-multiply.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstDuplicate");
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

        // here starts the buffer handling part
        let mut instream = match if input_opened {
            HfstInputStream::new_filename(&common.input_filename)
        } else {
            HfstInputStream::new()
        } {
            Ok(s) => s,
            Err(e) => {
                eprintln!("hfst-multiply: cannot open input: {e}");
                return 1;
            }
        };
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)

        if is_input_stream_in_ol_format(&instream, "hfst-multiply") {
            return 1;
        }

        let ty = instream.get_type();
        let mut outstream = match if output_opened {
            HfstOutputStream::new_filename(&common.output_filename, ty, true)
        } else {
            HfstOutputStream::new(ty, true)
        } {
            Ok(s) => s,
            Err(e) => {
                eprintln!("hfst-multiply: cannot open output: {e}");
                return 1;
            }
        };

        process_stream(&common, &options, &mut instream, &mut outstream)
    }
}

pub mod preprocess_for_optimized_lookup_format {
    //! Faithful 1:1 port of tools/src/hfst-preprocess-for-optimized-lookup-format.cc
    //! — the transducer preprocessing tool (the C++ source is the epsilon-removal /
    //! rebuild tool). Drives the hfst-cli foundation (globals, getopt, commandline,
    //! program-options, tool-metadata, inc fragments).

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        error, extend_options_from_env, hfst_set_program_name, verbose_print,
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
    use hfst::hfst_basic_transducer::HfstBasicTransducer;
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;
    use hfst::hfst_transducer::HfstTransducer;
    use std::io::Write;

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        // Usage line
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nRemove epsilons from a transducer\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        let _ = writeln!(msg);
        print_common_unary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(msg);
    }

    // [spec:hfst:req:cli.arg-parse]
    fn parse_options(
        mut common: CommonOptions,
        args: &mut Vec<String>,
    ) -> Result<CommonOptions, i32> {
        let mut opt = Getopt::new();
        extend_options_from_env(args);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the terminal error arm.
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
            return Err(handle_error_case(&common, &opt, c));
        }

        check_common_params(&mut common);
        check_unary_params(&mut common, &opt, args);
        Ok(common)
    }

    // [spec:hfst:def:hfst-preprocess-for-optimized-lookup-format.process-stream-fn]
    // [spec:hfst:sem:hfst-preprocess-for-optimized-lookup-format.process-stream-fn]
    fn process_stream(
        common: &CommonOptions,
        instream: &mut HfstInputStream<'_>,
        outstream: &mut HfstOutputStream,
    ) -> i32 {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let any = match instream.read() {
                Ok(v) => v,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
            crate::for_algebra!(any, trans => {
            let mut trans = trans;
            let inputname = hfst_get_name(&trans, &common.input_filename);
            if transducer_n == 1 {
                verbose_print(common, &format!("Removing epsilons {}...\n", inputname));
            } else {
                verbose_print(common, &format!(
                    "Removing epsilons {}...{}\n",
                    inputname, transducer_n
                ));
            }
            if let Err(e) = trans.remove_epsilons() {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
            if transducer_n == 1 {
                verbose_print(common, &format!("Rebuilding and fixing {}...\n", inputname));
            } else {
                verbose_print(common, &format!(
                    "Rebuilding and fisting {}...{}\n",
                    inputname, transducer_n
                ));
            }
            // C++: HfstBasicTransducer original(trans); — the
            // HfstBasicTransducer(const HfstTransducer&) conversion constructor.
            let original: HfstBasicTransducer =
                match HfstBasicTransducer::try_from_transducer(&trans) {
                    Ok(v) => v,
                    Err(e) => {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                };
            let replication = original.renumber_states();
            trans = match HfstTransducer::new_from_basic(&replication) {
                Ok(v) => v,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            // C: hfst_set_name(trans, trans, "fu"); the dest and src are the same
            // object, which Rust cannot alias mut+const, so the read side is taken
            // from a copy (name/formula are unchanged by the copy).
            let src = trans.clone();
            hfst_set_name_unary(&mut trans, &src, "fu");
            hfst_set_formula_unary(&mut trans, &src, "FU");
            if let Err(e) = trans.remove_epsilons() {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
            if let Err(e) = outstream.redirect(&mut trans) {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
            }, else => {
                // The C++ ran its algebra on whatever type arrived and threw
                // FunctionNotImplemented (uncaught) on optimized-lookup input;
                // report the standard OL rejection instead.
                let _ = writeln!(
                    std::io::stderr(),
                    "Error: hfst-preprocess-for-optimized-lookup-format cannot process transducers that are in optimized lookup format."
                );
                return 1;
            });
        }
        instream.close();
        outstream.close();
        0
    }

    // [spec:hfst:def:hfst-preprocess-for-optimized-lookup-format.main-fn]
    // [spec:hfst:sem:hfst-preprocess-for-optimized-lookup-format.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstPreprocessForOptimizedLookupFormat");
        let common = match parse_options(common, &mut args) {
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

        process_stream(&common, &mut instream, &mut outstream)
    }
}

pub mod project {
    //! Faithful 1:1 port of tools/src/hfst-project.cc — the transducer projection
    //! command-line tool. Drives the hfst-cli foundation (globals, getopt,
    //! commandline, program-options, tool-metadata, inc fragments).

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{error, extend_options_from_env, hfst_set_program_name};
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
        print_common_unary_program_options, print_common_unary_program_parameter_instructions,
    };
    use crate::inc::{
        CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
        handle_unary_case,
    };
    use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_transducer::HfstTransducer;
    use std::borrow::Cow;
    use std::io::Write;

    /// hfst-project's own options (the former tool-specific `static mut`s).
    #[derive(Default)]
    struct Options {
        /// '-p, --project=LEVEL': project extracting the input (first) tape when
        /// true, the output (second) tape when false.
        project_input: bool,
    }

    // strncasecmp(optarg, prefix, 1) == 0 — case-insensitive comparison of the
    // first byte only (the C calls always pass length 1).
    fn first_char_matches(optarg: &Option<String>, prefix: &str) -> bool {
        match optarg.as_ref().and_then(|s| s.bytes().next()) {
            Some(first) => {
                let want = prefix.as_bytes()[0];
                first.eq_ignore_ascii_case(&want)
            }
            None => false,
        }
    }

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nProject (extract a level) transducer\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        let _ = write!(
            msg,
            "Projection options:\n  -p, --project=LEVEL   project extracting tape LEVEL\n"
        );
        let _ = writeln!(msg);
        print_common_unary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(
            msg,
            "LEVEL must be one of upper, input, first, analysis or lower, output, second, generation"
        );
        let _ = writeln!(msg);
    }

    // [spec:hfst:def:hfst-project.parse-options-fn]
    // [spec:hfst:sem:hfst-project.parse-options-fn]
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
                name: "project",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: 'p' as i32,
            });
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own 'p', then the
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
            if c == 'p' as i32 {
                let optarg = opt.optarg_opt();
                if first_char_matches(&optarg, "upper")
                    || first_char_matches(&optarg, "input")
                    || first_char_matches(&optarg, "first")
                    || first_char_matches(&optarg, "analysis")
                {
                    options.project_input = true;
                } else if first_char_matches(&optarg, "lower")
                    || first_char_matches(&optarg, "output")
                    || first_char_matches(&optarg, "second")
                    || first_char_matches(&optarg, "generation")
                {
                    options.project_input = false;
                } else {
                    error(
                        &common,
                        1,
                        0,
                        &format!(
                            "unknown project direction {}\nshould be one of upper, input, analysis, first, lower, output, second or generation\n",
                            opt.optarg()
                        ),
                    );
                    return Err(1);
                }
                continue;
            }
            return Err(handle_error_case(&common, &opt, c));
        }

        check_common_params(&mut common);
        check_unary_params(&mut common, &opt, args);
        Ok((common, options))
    }

    // [spec:hfst:def:hfst-project.process-stream-fn]
    // [spec:hfst:sem:hfst-project.process-stream-fn]
    //
    // The stream loop lives in the shared unary driver; this op is the
    // per-transducer body it dispatches into. Both the verbose verb and the
    // name/formula stamp follow the projected tape.
    struct ProjectOp {
        project_input: bool,
    }

    impl UnaryToolOp for ProjectOp {
        fn verbose_begin(&self, inputname: &str) -> String {
            if self.project_input {
                format!("Projecting first {}", inputname)
            } else {
                format!("Projecting second {}", inputname)
            }
        }

        fn verbose_sep(&self) -> &'static str {
            " "
        }

        fn name_op(&self) -> Option<Cow<'_, str>> {
            Some(Cow::Borrowed(if self.project_input {
                "project-1st"
            } else {
                "project-2nd"
            }))
        }

        fn formula(&self) -> Option<Cow<'_, str>> {
            Some(Cow::Borrowed(if self.project_input {
                "\u{00b9}"
            } else {
                "\u{00b2}"
            }))
        }

        fn apply<B: AlgebraBackend>(
            &mut self,
            _common: &CommonOptions,
            t: &mut HfstTransducer<B>,
        ) -> hfst::error::Result<()> {
            if self.project_input {
                t.input_project().map(|_| ())
            } else {
                t.output_project().map(|_| ())
            }
        }
    }

    const SPEC: UnaryOpSpec = UnaryOpSpec {
        tool_name: "hfst-project",
        reject_ol: true,
    };

    // [spec:hfst:def:hfst-project.main-fn]
    // [spec:hfst:sem:hfst-project.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstProject");
        let (common, options) = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };

        let mut op = ProjectOp {
            project_input: options.project_input,
        };
        run_unary_tool(&common, &SPEC, &mut op)
    }
}

pub mod prune_alphabet {
    //! Faithful 1:1 port of tools/src/hfst-prune-alphabet.cc — the transducer
    //! alphabet-pruning command-line tool. Drives the hfst-cli foundation
    //! (globals, getopt, commandline, program-options, tool-metadata, inc
    //! fragments).

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{extend_options_from_env, hfst_set_program_name};
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
        print_common_unary_program_options, print_common_unary_program_parameter_instructions,
    };
    use crate::inc::{
        CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
        handle_unary_case,
    };
    use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_transducer::HfstTransducer;
    use std::borrow::Cow;
    use std::io::Write;

    /// hfst-prune-alphabet's own options (the former tool-specific `static mut`s).
    #[derive(Default)]
    struct Options {
        /// '-f, --force' sets true; '-S, --safe' sets false (default).
        force_pruning: bool,
    }

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nPrune the alphabet of a transducer\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        let _ = write!(
            msg,
            "Alphabet pruning options:\n  -f, --force            force pruning\n  -S, --safe             prune only if no unknown or identity symbols\n                         are used in the transducer (default)"
        );
        let _ = writeln!(msg);
        print_common_unary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(msg);
    }

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
                name: "force",
                has_arg: getopt::NO_ARGUMENT,
                val: 'f' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "safe",
                has_arg: getopt::NO_ARGUMENT,
                val: 'S' as i32,
            });
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own ('f'/'S'), then the
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
            match c as u8 as char {
                'f' => {
                    options.force_pruning = true;
                    continue;
                }
                'S' => {
                    options.force_pruning = false;
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

    // [spec:hfst:def:hfst-prune-alphabet.process-stream-fn]
    // [spec:hfst:sem:hfst-prune-alphabet.process-stream-fn]
    //
    // The stream loop lives in the shared unary driver; this op is the
    // per-transducer body it dispatches into. The tool stamps a name but no
    // formula, so `formula` keeps the trait default of None.
    struct PruneAlphabetOp {
        force_pruning: bool,
    }

    impl UnaryToolOp for PruneAlphabetOp {
        fn verbose_begin(&self, inputname: &str) -> String {
            format!("Pruning {}", inputname)
        }

        fn verbose_sep(&self) -> &'static str {
            " "
        }

        fn name_op(&self) -> Option<Cow<'_, str>> {
            Some(Cow::Borrowed("prune-alphabet"))
        }

        fn apply<B: AlgebraBackend>(
            &mut self,
            _common: &CommonOptions,
            t: &mut HfstTransducer<B>,
        ) -> hfst::error::Result<()> {
            t.prune_alphabet(self.force_pruning).map(|_| ())
        }
    }

    const SPEC: UnaryOpSpec = UnaryOpSpec {
        tool_name: "hfst-prune-alphabet",
        reject_ol: true,
    };

    // [spec:hfst:def:hfst-prune-alphabet.main-fn]
    // [spec:hfst:sem:hfst-prune-alphabet.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstPruneAlphabet");
        let (common, options) = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };

        let mut op = PruneAlphabetOp {
            force_pruning: options.force_pruning,
        };
        run_unary_tool(&common, &SPEC, &mut op)
    }
}

pub mod push_labels {
    //! Faithful 1:1 port of tools/src/hfst-push-labels.cc — the label-pushing
    //! command-line tool. Drives the hfst-cli foundation (globals, getopt,
    //! commandline, program-options, tool-metadata, inc fragments).

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        error, extend_options_from_env, hfst_set_program_name, is_input_stream_in_ol_format,
        verbose_print,
    };
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
        print_common_unary_program_options, print_common_unary_program_parameter_instructions,
    };
    use crate::inc::{
        CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
        handle_unary_case,
    };
    use crate::unary_ops::{
        UnaryOpSpec, UnaryToolOp, open_input_stream, open_output_stream_like, unary_streams,
    };
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_data_types::PushType;
    use hfst::hfst_transducer::HfstTransducer;
    use std::borrow::Cow;
    use std::io::Write;

    /// hfst-push-labels's own options (the former tool-specific `static mut`s).
    #[derive(Default)]
    struct Options {
        /// '-p, --push=DIRECTION': push towards the initial state when true.
        push_initial: bool,
    }

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nPush labels of transducer\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        let _ = write!(
            msg,
            "Push options:\n  -p, --push=DIRECTION   push to DIRECTION\n"
        );
        let _ = writeln!(msg);
        print_common_unary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(
            msg,
            "DIRECTION must be one of start, initial, begin or end, final"
        );
        let _ = writeln!(msg);
    }

    // [spec:hfst:def:hfst-push-labels.parse-options-fn]
    // [spec:hfst:sem:hfst-push-labels.parse-options-fn]
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
                name: "push",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: b'p' as i32,
            });
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own ('p'), then the
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
            if c == b'p' as i32 {
                let optarg = opt.optarg();
                let lower = optarg.to_ascii_lowercase();
                if lower.starts_with('s') || lower.starts_with('i') || lower.starts_with('b') {
                    options.push_initial = true;
                } else if lower.starts_with('e') || lower.starts_with('f') {
                    options.push_initial = false;
                } else {
                    error(
                        &common,
                        1,
                        0,
                        &format!(
                            "unknown push direction {}\nshould be one of start, initial, begin, end or final.\n",
                            optarg
                        ),
                    );
                    return Err(1);
                }
                continue;
            }
            return Err(handle_error_case(&common, &opt, c));
        }

        check_common_params(&mut common);
        check_unary_params(&mut common, &opt, args);
        Ok((common, options))
    }

    // [spec:hfst:def:hfst-push-labels.process-stream-fn]
    // [spec:hfst:sem:hfst-push-labels.process-stream-fn]
    //
    // The stream loop lives in the shared unary driver; this op is the
    // per-transducer body it dispatches into. Both the verbose verb and the name
    // stamp's -i/-f suffix follow the push direction.
    struct PushLabelsOp {
        push_initial: bool,
    }

    impl UnaryToolOp for PushLabelsOp {
        fn verbose_begin(&self, inputname: &str) -> String {
            if self.push_initial {
                format!("Pushing towards start {}", inputname)
            } else {
                format!("Pushing towards end {}", inputname)
            }
        }

        fn verbose_sep(&self) -> &'static str {
            " "
        }

        fn name_op(&self) -> Option<Cow<'_, str>> {
            Some(Cow::Borrowed(if self.push_initial {
                "push-labels-i"
            } else {
                "push-labels-f"
            }))
        }

        fn formula(&self) -> Option<Cow<'_, str>> {
            Some(Cow::Borrowed("Id"))
        }

        fn apply<B: AlgebraBackend>(
            &mut self,
            _common: &CommonOptions,
            t: &mut HfstTransducer<B>,
        ) -> hfst::error::Result<()> {
            if self.push_initial {
                t.push_labels(PushType::TO_INITIAL_STATE).map(|_| ())
            } else {
                t.push_labels(PushType::TO_FINAL_STATE).map(|_| ())
            }
        }
    }

    // `reject_ol` is left false because this tool rejects optimized-lookup input
    // BEFORE opening the output stream (see run); the flag would reject it after.
    const SPEC: UnaryOpSpec = UnaryOpSpec {
        tool_name: "hfst-push-labels",
        reject_ol: false,
    };

    // [spec:hfst:def:hfst-push-labels.main-fn]
    // [spec:hfst:sem:hfst-push-labels.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstPush");
        let (common, options) = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };

        let mut op = PushLabelsOp {
            push_initial: options.push_initial,
        };

        // This tool orders the optimized-lookup rejection BEFORE the output stream
        // is opened, unlike every other unary tool (and unlike run_unary_tool):
        // rejecting an OL input must not have created/truncated '-o FILE' first.
        // So the driver's steps are composed here in the tool's own order rather
        // than going through run_unary_tool.
        verbose_print(
            &common,
            &format!(
                "Reading from {}, writing to {}\n",
                common.input_filename, common.output_filename
            ),
        );

        let mut instream = match open_input_stream(&common) {
            Ok(s) => s,
            Err(code) => return code,
        };

        if is_input_stream_in_ol_format(&instream, "hfst-push-labels") {
            return 1;
        }

        let mut outstream = match open_output_stream_like(&common, &instream) {
            Ok(s) => s,
            Err(code) => return code,
        };

        unary_streams(&common, &SPEC, &mut op, &mut instream, &mut outstream)
    }
}

pub mod push_weights {
    //! Faithful 1:1 port of tools/src/hfst-push-weights.cc — the weight pushing
    //! command-line tool. Pushes the weights of a transducer towards its start or
    //! end states. Drives the hfst-cli foundation (globals, getopt, commandline,
    //! program-options, tool-metadata, inc fragments).

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{error, extend_options_from_env, hfst_set_program_name};
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
        print_common_unary_program_options, print_common_unary_program_parameter_instructions,
    };
    use crate::inc::{
        CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
        handle_unary_case,
    };
    use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_data_types::PushType;
    use hfst::hfst_transducer::HfstTransducer;
    use std::borrow::Cow;
    use std::io::Write;

    /// hfst-push-weights's own options (the former tool-specific `static mut`s).
    #[derive(Default)]
    struct Options {
        /// '-p, --push=DIRECTION': push towards the start state when true, else the
        /// end state (default is false, i.e. push towards the end/final state).
        push_initial: bool,
    }

    // strncasecmp(optarg, prefix, 1) == 0 : the first character of optarg matches
    // the first character of prefix, case-insensitively. Each candidate prefix here
    // starts with a distinct letter, so this is a one-character case-fold compare.
    fn first_char_eq_ignore_case(arg: &str, prefix: &str) -> bool {
        match (arg.chars().next(), prefix.chars().next()) {
            (Some(a), Some(b)) => a.eq_ignore_ascii_case(&b),
            (None, None) => true,
            _ => false,
        }
    }

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        // Usage line
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nPush weights of transducer\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        let _ = write!(
            msg,
            "Push options:\n  -p, --push=DIRECTION   push to DIRECTION\n"
        );
        let _ = writeln!(msg);
        print_common_unary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(
            msg,
            "DIRECTION must be one of start, initial, begin or end, final"
        );
        let _ = writeln!(msg);
    }

    // [spec:hfst:def:hfst-push-weights.parse-options-fn]
    // [spec:hfst:sem:hfst-push-weights.parse-options-fn]
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
                name: "push",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: 'p' as i32,
            });
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own 'p', then the
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
            if c == 'p' as i32 {
                let optarg = opt.optarg();
                if first_char_eq_ignore_case(&optarg, "start")
                    || first_char_eq_ignore_case(&optarg, "initial")
                    || first_char_eq_ignore_case(&optarg, "begin")
                {
                    options.push_initial = true;
                } else if first_char_eq_ignore_case(&optarg, "end")
                    || first_char_eq_ignore_case(&optarg, "final")
                {
                    options.push_initial = false;
                } else {
                    error(
                        &common,
                        1,
                        0,
                        &format!(
                            "unknown push direction {}\nshould be one of start, initial, begin, end or final.\n",
                            optarg
                        ),
                    );
                    return Err(1);
                }
                continue;
            }
            return Err(handle_error_case(&common, &opt, c));
        }

        check_common_params(&mut common);
        check_unary_params(&mut common, &opt, args);
        Ok((common, options))
    }

    // [spec:hfst:def:hfst-push-weights.process-stream-fn]
    // [spec:hfst:sem:hfst-push-weights.process-stream-fn]
    //
    // The stream loop lives in the shared unary driver; this op is the
    // per-transducer body it dispatches into. Both the verbose verb and the name
    // stamp's -i/-f suffix follow the push direction.
    struct PushWeightsOp {
        push_initial: bool,
    }

    impl UnaryToolOp for PushWeightsOp {
        fn verbose_begin(&self, inputname: &str) -> String {
            if self.push_initial {
                format!("Pushing towards start {}", inputname)
            } else {
                format!("Pushing towards end {}", inputname)
            }
        }

        fn verbose_sep(&self) -> &'static str {
            " "
        }

        fn name_op(&self) -> Option<Cow<'_, str>> {
            Some(Cow::Borrowed(if self.push_initial {
                "push-weights-i"
            } else {
                "push-weights-f"
            }))
        }

        fn formula(&self) -> Option<Cow<'_, str>> {
            Some(Cow::Borrowed("Id"))
        }

        fn apply<B: AlgebraBackend>(
            &mut self,
            _common: &CommonOptions,
            t: &mut HfstTransducer<B>,
        ) -> hfst::error::Result<()> {
            if self.push_initial {
                t.push_weights(PushType::TO_INITIAL_STATE).map(|_| ())
            } else {
                t.push_weights(PushType::TO_FINAL_STATE).map(|_| ())
            }
        }
    }

    const SPEC: UnaryOpSpec = UnaryOpSpec {
        tool_name: "hfst-push-weights",
        reject_ol: true,
    };

    // [spec:hfst:def:hfst-push-weights.main-fn]
    // [spec:hfst:sem:hfst-push-weights.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstPush");
        let (common, options) = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };

        let mut op = PushWeightsOp {
            push_initial: options.push_initial,
        };
        run_unary_tool(&common, &SPEC, &mut op)
    }
}

pub mod realign {
    //! Faithful 1:1 port of tools/src/hfst-realign.cc — the transducer realign
    //! command-line tool. Drives the hfst-cli foundation (globals, getopt,
    //! commandline, program-options, tool-metadata, inc fragments).
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
        print_common_unary_program_options, print_common_unary_program_parameter_instructions,
    };
    use crate::inc::{
        CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
        handle_unary_case,
    };
    use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_transducer::HfstTransducer;
    use std::borrow::Cow;
    use std::io::Write;

    /// hfst-realign's own options (the former tool-specific `static mut`s).
    struct Options {
        /// '-b, --boundary=SYM': treat SYM as a boundary symbol.
        boundary_symbol: u8,
    }

    impl Default for Options {
        fn default() -> Options {
            Options {
                boundary_symbol: b'>',
            }
        }
    }

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        // Usage line
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nRealign a transducer by pushing labels to the start\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        let _ = write!(
            msg,
            "Options:\n  -b, --boundary=SYM   treat SYM as a boundary symbol\n"
        );
        let _ = writeln!(msg);
        print_common_unary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(msg, "SYM must be in the alphabet");
        let _ = writeln!(msg);
    }

    // [spec:hfst:def:hfst-realign.parse-options-fn]
    // [spec:hfst:sem:hfst-realign.parse-options-fn]
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
                name: "boundary",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: b'b' as i32,
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
            // The C source labels its tool-specific arm 'p' (not 'b'), which
            // merely resets the boundary symbol to its default '>'.
            if c == (b'p' as i32) {
                options.boundary_symbol = b'>';
                continue;
            }
            return Err(handle_error_case(&common, &opt, c));
        }

        check_common_params(&mut common);
        check_unary_params(&mut common, &opt, args);
        Ok((common, options))
    }

    // [spec:hfst:def:hfst-realign.process-stream-fn]
    // [spec:hfst:sem:hfst-realign.process-stream-fn]
    //
    // The stream loop lives in the shared unary driver; this op is the
    // per-transducer body it dispatches into. The C's verbose verb is selected by
    // the boundary symbol (a leftover of the push-labels tool it was copied from),
    // so the op carries it.
    struct RealignOp {
        boundary_symbol: u8,
    }

    impl UnaryToolOp for RealignOp {
        fn verbose_begin(&self, inputname: &str) -> String {
            if self.boundary_symbol != 0 {
                format!("Pushing towards start {}", inputname)
            } else {
                format!("Pushing towards end {}", inputname)
            }
        }

        fn verbose_sep(&self) -> &'static str {
            " "
        }

        fn name_op(&self) -> Option<Cow<'_, str>> {
            Some(Cow::Borrowed("realign"))
        }

        fn formula(&self) -> Option<Cow<'_, str>> {
            Some(Cow::Borrowed("Id"))
        }

        fn apply<B: AlgebraBackend>(
            &mut self,
            _common: &CommonOptions,
            t: &mut HfstTransducer<B>,
        ) -> hfst::error::Result<()> {
            t.realign().map(|_| ())
        }
    }

    const SPEC: UnaryOpSpec = UnaryOpSpec {
        tool_name: "hfst-realign",
        reject_ol: true,
    };

    // [spec:hfst:def:hfst-realign.main-fn]
    // [spec:hfst:sem:hfst-realign.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstRealign");
        let (common, options) = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };

        let mut op = RealignOp {
            boundary_symbol: options.boundary_symbol,
        };
        run_unary_tool(&common, &SPEC, &mut op)
    }
}

pub mod remove_epsilons {
    //! Faithful 1:1 port of tools/src/hfst-remove-epsilons.cc — the transducer
    //! epsilon-removal command-line tool. Drives the hfst-cli foundation (globals,
    //! getopt, commandline, program-options, tool-metadata, inc fragments).
    //!
    //! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
    //! shared `-v/-q/-o/-i/…` fields), built by `parse_options` and threaded into
    //! the processing functions. There are no `static mut` globals and no `unsafe`.

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{extend_options_from_env, hfst_set_program_name};
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
        print_common_unary_program_options, print_common_unary_program_parameter_instructions,
    };
    use crate::inc::{
        CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
        handle_unary_case,
    };
    use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_transducer::HfstTransducer;
    use std::borrow::Cow;
    use std::io::Write;

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nRemove epsilons from a transducer\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        let _ = writeln!(msg);
        print_common_unary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(msg);
    }

    // [spec:hfst:req:cli.arg-parse]
    //
    // Parse argv into the shared options; `Err(code)` is an exit code the caller
    // should return (the former EXIT_CONTINUE sentinel is now `Ok`).
    fn parse_options(
        mut common: CommonOptions,
        args: &mut Vec<String>,
    ) -> Result<CommonOptions, i32> {
        let mut opt = Getopt::new();
        extend_options_from_env(args);
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then the terminal error arm, then unary cases. The tool has
            // no own options here.
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
            return Err(handle_error_case(&common, &opt, c));
        }

        check_common_params(&mut common);
        check_unary_params(&mut common, &opt, args);
        Ok(common)
    }

    // [spec:hfst:def:hfst-remove-epsilons.process-stream-fn]
    // [spec:hfst:sem:hfst-remove-epsilons.process-stream-fn]
    //
    // The stream loop lives in the shared unary driver; this op is the
    // per-transducer body it dispatches into.
    struct RemoveEpsilonsOp;

    impl UnaryToolOp for RemoveEpsilonsOp {
        fn verbose_begin(&self, inputname: &str) -> String {
            format!("Removing epsilons {}", inputname)
        }

        fn name_op(&self) -> Option<Cow<'_, str>> {
            Some(Cow::Borrowed("remove-epsilons"))
        }

        fn formula(&self) -> Option<Cow<'_, str>> {
            Some(Cow::Borrowed("Id"))
        }

        fn apply<B: AlgebraBackend>(
            &mut self,
            _common: &CommonOptions,
            t: &mut HfstTransducer<B>,
        ) -> hfst::error::Result<()> {
            t.remove_epsilons().map(|_| ())
        }
    }

    const SPEC: UnaryOpSpec = UnaryOpSpec {
        tool_name: "hfst-remove-epsilons",
        reject_ol: true,
    };

    // [spec:hfst:def:hfst-remove-epsilons.main-fn]
    // [spec:hfst:sem:hfst-remove-epsilons.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstRemoveEpsilons");
        let common = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };

        run_unary_tool(&common, &SPEC, &mut RemoveEpsilonsOp)
    }
}

pub mod repeat {
    //! Faithful 1:1 port of tools/src/hfst-repeat.cc — the transducer repetition
    //! command-line tool. Drives the hfst-cli foundation (globals, getopt,
    //! commandline, program-options, tool-metadata, inc fragments).
    //!
    //! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
    //! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
    //! `parse_options` and threaded into the processing functions. There are no
    //! `static mut` globals and no `unsafe`.

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        error, extend_options_from_env, hfst_set_program_name, hfst_strtonumber,
        is_input_stream_in_ol_format, verbose_print,
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
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;
    use std::io::Write;

    /// hfst-repeat's own options (the former tool-specific `static mut`s).
    struct Options {
        /// '-f, --from=FNUM': repeat at least FNUM times.
        at_least: u64,
        /// '-t, --to=TNUM': repeat at most TNUM times.
        at_most: u64,
        /// FNUM was parsed as infinity.
        from_infinity: bool,
        /// TNUM was parsed as infinity.
        to_infinity: bool,
    }

    impl Default for Options {
        fn default() -> Self {
            Options {
                at_least: 0,
                at_most: u32::MAX as u64,
                from_infinity: false,
                to_infinity: true,
            }
        }
    }

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nRepeat transducer\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        let _ = write!(
            msg,
            "Repetition options:\n  -f, --from=FNUM   repeat at least FNUM times\n  -t, --to=TNUM     repeat at most TNUM times\n"
        );
        let _ = writeln!(msg);
        print_common_unary_program_parameter_instructions(&mut *msg);
        let _ = write!(
            msg,
            "FNUM and TNUM must be positive integers or infinities as parsed by strtod(3)\nif FNUM is omitted it defaults to 0, if TNUM is omitted it defaults to Inf\nFNUM must be less than TNUM\n"
        );
        let _ = writeln!(msg);
    }

    // [spec:hfst:def:hfst-repeat.parse-options-fn]
    // [spec:hfst:sem:hfst-repeat.parse-options-fn]
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
            long_options.push(getopt::GetOpt {
                name: "from",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: b'f' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "to",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: b't' as i32,
            });
            // add tool-specific options here
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own f/t cases, then the
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
                c if c == b'f' as i32 => {
                    let mut from_inf = false;
                    options.at_least =
                        hfst_strtonumber(&common, &opt.optarg(), Some(&mut from_inf)) as u64;
                    options.from_infinity = from_inf;
                    continue;
                }
                c if c == b't' as i32 => {
                    let mut to_inf = false;
                    options.at_most =
                        hfst_strtonumber(&common, &opt.optarg(), Some(&mut to_inf)) as u64;
                    options.to_infinity = to_inf;
                    continue;
                }
                _ => {}
            }
            return Err(handle_error_case(&common, &opt, c));
        }

        check_common_params(&mut common);
        check_unary_params(&mut common, &opt, args);
        if options.at_least > options.at_most {
            error(
                &common,
                1,
                0,
                &format!(
                    "Cannot repeat from {} to {} times\n",
                    options.at_least, options.at_most
                ),
            );
        }
        if options.from_infinity && !options.to_infinity {
            error(
                &common,
                1,
                0,
                &format!("Cannot repeat from infinity to {} times\n", options.at_most),
            );
        }
        Ok((common, options))
    }

    // [spec:hfst:def:hfst-repeat.process-stream-fn]
    // [spec:hfst:sem:hfst-repeat.process-stream-fn]
    fn process_stream(
        common: &CommonOptions,
        options: &Options,
        instream: &mut HfstInputStream<'_>,
        outstream: &mut HfstOutputStream,
    ) -> i32 {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let any = match instream.read() {
                Ok(v) => v,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
            crate::for_algebra!(any, trans => {
                let mut trans = trans;
                let inputname = hfst_get_name(&trans, &common.input_filename);
                if transducer_n == 1 {
                    if !options.from_infinity && !options.to_infinity {
                        verbose_print(common, &format!(
                            "Repeating [{}..{}] {}...\n",
                            options.at_least, options.at_most, inputname
                        ));
                    } else if options.from_infinity && options.to_infinity {
                        verbose_print(common, &format!("Repeating star {}...\n", inputname));
                    } else if !options.from_infinity && options.to_infinity {
                        verbose_print(common, &format!("Repeating [{}..*] {}...\n", options.at_least, inputname));
                    } else if options.from_infinity && !options.to_infinity {
                        error(common, 1, 0, &format!("Repeating *..{}?", options.at_most));
                    }
                } else if !options.from_infinity && !options.to_infinity {
                    verbose_print(common, &format!(
                        "Repeating [{}..{}] {}... {}\n",
                        options.at_least, options.at_most, inputname, transducer_n
                    ));
                } else if options.from_infinity && options.to_infinity {
                    verbose_print(common, &format!(
                        "Repeating star {}... {}\n",
                        inputname, transducer_n
                    ));
                } else if !options.from_infinity && options.to_infinity {
                    verbose_print(common, &format!(
                        "Repeating [{}..*] {}... {}\n",
                        options.at_least, inputname, transducer_n
                    ));
                } else if options.from_infinity && !options.to_infinity {
                    error(common, 1, 0, &format!("Repeating *..{}?", options.at_most));
                }

                if !options.from_infinity && !options.to_infinity {
                    if let Err(e) = trans.repeat_n_to_k(options.at_least as u32, options.at_most as u32) {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                    let composed_name = format!("repeat-{}-to-{}", options.at_least, options.at_most);
                    let src = trans.clone();
                    hfst_set_name_unary(&mut trans, &src, &composed_name);
                    let composed_name = format!("_{}^{}", options.at_least, options.at_most);
                    let src = trans.clone();
                    hfst_set_formula_unary(&mut trans, &src, &composed_name);
                } else if options.from_infinity && options.to_infinity {
                    if let Err(e) = trans.repeat_star() {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                    let src = trans.clone();
                    hfst_set_name_unary(&mut trans, &src, "repeat-star");
                    let src = trans.clone();
                    hfst_set_formula_unary(&mut trans, &src, "\u{22c6}");
                } else if !options.from_infinity && options.to_infinity {
                    if let Err(e) = trans.repeat_n_plus(options.at_least as u32) {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                    let composed_name = format!("repeat-{}-plus", options.at_least);
                    let src = trans.clone();
                    hfst_set_name_unary(&mut trans, &src, &composed_name);
                    let composed_name = format!("_{}^\u{221e}", options.at_least);
                    let src = trans.clone();
                    hfst_set_formula_unary(&mut trans, &src, &composed_name);
                } else if options.from_infinity && !options.to_infinity {
                    error(common, 1, 0, &format!("Repeating *..{}?", options.at_most));
                }
                if let Err(e) = outstream.redirect(&mut trans) {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            }, else => {
                // Unreachable: the optimized-lookup stream rejection already
                // returned before the loop; keep its text for safety.
                let _ = writeln!(
                    std::io::stderr(),
                    "Error: hfst-repeat cannot process transducers that are in optimized lookup format."
                );
                return 1;
            });
        }
        instream.close();
        outstream.close();
        0
    }

    // [spec:hfst:def:hfst-repeat.main-fn]
    // [spec:hfst:sem:hfst-repeat.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstRepeat");
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
        if !options.from_infinity && !options.to_infinity {
            verbose_print(
                &common,
                &format!(
                    "Repeating from {} to {} times\n",
                    options.at_least, options.at_most
                ),
            );
        } else if options.from_infinity && options.to_infinity {
            verbose_print(&common, "Repeating star infinitely\n");
        } else if !options.from_infinity && options.to_infinity {
            verbose_print(
                &common,
                &format!("Repeating from {} to infinite times\n", options.at_least),
            );
        } else if options.from_infinity && !options.to_infinity {
            error(
                &common,
                1,
                0,
                &format!(
                    "Repeating at least infinite butno more than {} times?",
                    options.at_most
                ),
            );
        }

        // here starts the buffer handling part
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
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)

        let ty = instream.get_type();
        let mut outstream = match if output_opened {
            HfstOutputStream::new_filename(&common.output_filename, ty, true)
        } else {
            HfstOutputStream::new(ty, true)
        } {
            Ok(s) => s,
            Err(e) => {
                error(&common, 1, 0, &format!("{e}"));
                return 1;
            }
        };

        if is_input_stream_in_ol_format(&instream, "hfst-repeat") {
            return 1;
        }

        process_stream(&common, &options, &mut instream, &mut outstream)
    }
}

pub mod reverse {
    //! Faithful 1:1 port of tools/src/hfst-reverse.cc — the transducer reversion
    //! command-line tool. Drives the hfst-cli foundation (globals, getopt,
    //! commandline, program-options, tool-metadata, inc fragments).
    //!
    //! The tool's state lives in [`CommonOptions`] (the shared `-v/-q/-o/-i/…`
    //! fields), built by `parse_options` and threaded into the processing
    //! functions. There are no `static mut` globals and no `unsafe`.

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{extend_options_from_env, hfst_set_program_name};
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
        print_common_unary_program_options, print_common_unary_program_parameter_instructions,
    };
    use crate::inc::{
        CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
        handle_unary_case,
    };
    use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_transducer::HfstTransducer;
    use std::borrow::Cow;
    use std::io::Write;

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nReverse a transducer\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        let _ = writeln!(msg);
        print_common_unary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(msg);
    }

    // [spec:hfst:req:cli.arg-parse]
    //
    // Parse argv into the shared options; `Err(code)` is an exit code the caller
    // should return (the former EXIT_CONTINUE sentinel is now `Ok`).
    fn parse_options(
        mut common: CommonOptions,
        args: &mut Vec<String>,
    ) -> Result<CommonOptions, i32> {
        let mut opt = Getopt::new();
        extend_options_from_env(args);
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own (none here), then the
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
            return Err(handle_error_case(&common, &opt, c));
        }

        check_common_params(&mut common);
        check_unary_params(&mut common, &opt, args);
        Ok(common)
    }

    // [spec:hfst:def:hfst-reverse.process-stream-fn]
    // [spec:hfst:sem:hfst-reverse.process-stream-fn]
    //
    // The stream loop lives in the shared unary driver; this op is the
    // per-transducer body it dispatches into.
    struct ReverseOp;

    impl UnaryToolOp for ReverseOp {
        fn verbose_begin(&self, inputname: &str) -> String {
            format!("Reversing {}", inputname)
        }

        fn name_op(&self) -> Option<Cow<'_, str>> {
            Some(Cow::Borrowed("reverse"))
        }

        fn formula(&self) -> Option<Cow<'_, str>> {
            Some(Cow::Borrowed("\u{21c6}"))
        }

        fn apply<B: AlgebraBackend>(
            &mut self,
            _common: &CommonOptions,
            t: &mut HfstTransducer<B>,
        ) -> hfst::error::Result<()> {
            t.reverse().map(|_| ())
        }
    }

    const SPEC: UnaryOpSpec = UnaryOpSpec {
        tool_name: "hfst-reverse",
        reject_ol: true,
    };

    // [spec:hfst:def:hfst-reverse.main-fn]
    // [spec:hfst:sem:hfst-reverse.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstReverse");
        let common = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };

        run_unary_tool(&common, &SPEC, &mut ReverseOp)
    }
}
