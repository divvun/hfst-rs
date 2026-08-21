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
    //! automaton. Option handling is clap 4 derive through [`crate::cli`].

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        error, hfst_set_program_name, hfst_strtoweight, is_input_stream_in_ol_format, verbose_print,
    };
    use hfst::guessify_fst::{GuessDirection, affix_guessify};
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;
    use std::io::Write;

    /// hfst-affix-guessify's command line.
    //
    // GuessDirection and the per-transducer affix-guesser construction live in
    // hfst::guessify_fst; this tool keeps only the option-driven state + the
    // stream-driver loop.
    // [spec:hfst:def:hfst-affix-guessify.parse-options-fn]
    // [spec:hfst:sem:hfst-affix-guessify.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "Create weighted affix guesser from automaton")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,

        /// Set direction of guessing: suffix or prefix, suffix if omitted
        #[arg(short = 'D', long = "direction", value_name = "DIR")]
        direction: Option<String>,

        /// Set weight difference of affix lengths: the weight of each arc not
        /// in the known suffix or prefix being guessed, as parsed with
        /// strtod(3), or 1.0 if omitted
        #[arg(
            short = 'w',
            long = "weight",
            value_name = "WEIGHT",
            allow_hyphen_values = true
        )]
        weight: Option<String>,
    }

    impl Args {
        /// Case 'D': the C accepts any argument that STARTS WITH "prefix" or
        /// "suffix" and rejects everything else.
        fn direction(&self, common: &CommonOptions) -> GuessDirection {
            let Some(dir) = self.direction.as_deref() else {
                return GuessDirection::GuessSuffix;
            };
            if dir.starts_with("prefix") {
                GuessDirection::GuessPrefix
            } else if dir.starts_with("suffix") {
                GuessDirection::GuessSuffix
            } else {
                error(
                    common,
                    1,
                    0,
                    &format!(
                        "Unable to parse guessing direction from {};\nplease use one of 'prefix' or 'suffix'",
                        dir
                    ),
                );
                GuessDirection::GuessSuffix
            }
        }

        /// Case 'w': strtod, fatal on anything else; the C initialiser is 1.0.
        fn weight(&self, common: &CommonOptions) -> f32 {
            match &self.weight {
                Some(w) => hfst_strtoweight(common, w),
                None => 1.0f32,
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
            // Both rejections happened inside the C getopt loop, before the
            // parameter checks; run them here for the same ordering.
            self.weight(opts);
            self.direction(opts);
            Ok(())
        }
    }

    // [spec:hfst:def:hfst-affix-guessify.process-stream-fn]
    // [spec:hfst:sem:hfst-affix-guessify.process-stream-fn]
    fn process_stream(
        common: &CommonOptions,
        direction: GuessDirection,
        weight: f32,
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
                let mut t = match affix_guessify(&trans, direction, weight) {
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
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstAffixGuessify");
        let (common, args) = cli::parse::<Args>(common, args)?;
        let direction = args.direction(&common);
        let weight = args.weight(&common);

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
                return Err(1);
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
                return Err(1);
            }
        };

        if is_input_stream_in_ol_format(&instream, "hfst-affix-guessify") {
            return Err(1);
        }

        cli::from_code(process_stream(
            &common,
            direction,
            weight,
            &mut instream,
            &mut outstream,
        ))
    }
}

pub mod determinize {
    //! Faithful 1:1 port of tools/src/hfst-determinize.cc — the transducer
    //! determinisation command-line tool.
    //!
    //! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
    //! lives in [`CommonOptions`] (the shared -v/-q/-o/-i/... fields), built from
    //! the parsed [`Args`] and threaded into the processing functions. There are
    //! no `static mut` globals and no `unsafe`.

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::hfst_set_program_name;
    use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_transducer::{EngineConfig, HfstTransducer};
    use std::borrow::Cow;

    /// hfst-determinize's command line.
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "Determinize a transducer")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,

        /// Encode weights when determinizing (default is false)
        #[arg(short = 'E', long = "encode-weights")]
        encode_weights: bool,
    }

    impl ToolArgs for Args {
        fn common(&self) -> &CommonArgs {
            &self.common
        }

        fn apply_io(&self, opts: &mut CommonOptions) {
            self.io.apply(opts);
        }
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
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstDeterminize");
        let (common, args) = cli::parse::<Args>(common, args)?;

        let mut op = DeterminizeOp {
            encode_weights: args.encode_weights,
        };
        cli::from_code(run_unary_tool(&common, &SPEC, &mut op))
    }
}

pub mod eliminate_flags {
    //! Port of tools/src/hfst-eliminate-flags.cc — the transducer flag elimination
    //! command-line tool.
    //!
    //! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
    //! lives in [`CommonOptions`] (the shared -v/-q/-o/-i/... fields), built from
    //! the parsed [`Args`] and threaded into the processing functions. There are
    //! no `static mut` globals and no `unsafe`.

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{error, hfst_set_program_name};
    use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_transducer::HfstTransducer;
    use std::borrow::Cow;

    /// hfst-eliminate-flags's command line.
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "Eliminate flags from a transducer")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,

        /// Only eliminate flag FLAG
        #[arg(short = 'F', long = "flag", value_name = "FLAG")]
        flag: Option<String>,
    }

    impl ToolArgs for Args {
        fn common(&self) -> &CommonArgs {
            &self.common
        }

        fn apply_io(&self, opts: &mut CommonOptions) {
            self.io.apply(opts);
        }
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
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstEliminateFlags");
        let (common, args) = cli::parse::<Args>(common, args)?;

        let flags = match &args.flag {
            None => String::from("flags"),
            Some(f) => format!("flag {}", f),
        };
        let mut op = EliminateFlagsOp {
            flag: args.flag,
            flags,
        };
        cli::from_code(run_unary_tool(&common, &SPEC, &mut op))
    }
}

pub mod insert_freely {
    //! Faithful 1:1 port of tools/src/hfst-insert-freely.cc — the freely-insert
    //! a symbol (pair) command-line tool.
    //!
    //! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
    //! lives in [`CommonOptions`] (the shared -v/-q/-o/-i/... fields), built from
    //! the parsed [`Args`] and threaded into the processing functions. There are
    //! no `static mut` globals and no `unsafe`.

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        error, hfst_set_program_name, is_input_stream_in_ol_format, verbose_print,
    };
    use crate::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary, hfst_set_name_unary};
    use hfst::hfst_data_types::StringPair;
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;
    use hfst::hfst_symbol_defs::{internal_epsilon, label_to_stringpair};
    use std::io::Write;

    /// hfst-insert-freely's command line.
    // [spec:hfst:def:hfst-insert-freely.parse-options-fn]
    // [spec:hfst:sem:hfst-insert-freely.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "Freely insert a symbol (pair)")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,

        /// Symbol pair SYM: either a single alphabetic symbol or two symbols
        /// separated by a colon, :
        #[arg(short = 'a', long = "symbol-pair", value_name = "SYM")]
        symbol_pair: Option<String>,

        /// Harmonise
        #[arg(short = 'H')]
        harmonise: bool,

        /// Harmonise; upstream's long spelling takes a required argument that
        /// nothing reads, while its short-option string gives -H none, so
        /// '--harmonise SYM' swallows SYM and '-H SYM' leaves it as the input
        /// operand. The two spellings are separate args so both keep their
        /// upstream arity.
        #[arg(long = "harmonise", value_name = "ARG")]
        harmonise_long: Option<String>,
    }

    impl Args {
        /// Either spelling of -H/--harmonise sets the flag; the long form's
        /// argument is discarded, as upstream discards it.
        fn harmonise_flags(&self) -> bool {
            self.harmonise || self.harmonise_long.is_some()
        }
    }

    impl Args {
        /// Case 'a': "@0@" stands for the internal epsilon, and an empty label
        /// is fatal (the C checks AFTER building the pair from it).
        fn label(&self, common: &CommonOptions) -> Option<StringPair> {
            let lbl = self.symbol_pair.as_deref()?;
            // This will probably break for unicode
            let lbl = if lbl == "@0@" {
                internal_epsilon.to_string()
            } else {
                lbl.to_string()
            };
            let pair = label_to_stringpair(&lbl);
            if lbl.is_empty() {
                error(
                    common,
                    1,
                    0,
                    &format!(
                        "argument of source label option is empty;\nif you REALLY want to replace epsilons with something, use @0@ or {}",
                        internal_epsilon
                    ),
                );
            }
            pair
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
            // The empty-label rejection happened inside the C getopt loop,
            // before the parameter checks; run it here for the same ordering.
            self.label(opts);
            Ok(())
        }
    }

    // [spec:hfst:def:hfst-insert-freely.process-stream-fn]
    // [spec:hfst:sem:hfst-insert-freely.process-stream-fn]
    fn process_stream(
        common: &CommonOptions,
        symbol_pair: Option<&StringPair>,
        harmonise_flags: bool,
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
                    let pair = symbol_pair.expect("symbol pair must be set");
                    if let Err(e) = trans.insert_freely_pair(pair, harmonise_flags) {
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
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstPush");
        let (common, args) = cli::parse::<Args>(common, args)?;
        let symbol_pair = args.label(&common);

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

        if is_input_stream_in_ol_format(&instream, "hfst-insert-freely") {
            return Err(1);
        }

        cli::from_code(process_stream(
            &common,
            symbol_pair.as_ref(),
            args.harmonise_flags(),
            &mut instream,
            &mut outstream,
        ))
    }
}

pub mod invert {
    //! Faithful 1:1 port of tools/src/hfst-invert.cc — the transducer inversion
    //! command-line tool.
    //!
    //! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
    //! lives in [`CommonOptions`] (the shared -v/-q/-o/-i/... fields), built from
    //! the parsed [`Args`] and threaded into the processing functions. There are
    //! no `static mut` globals and no `unsafe`.

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::hfst_set_program_name;
    use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_transducer::HfstTransducer;
    use std::borrow::Cow;

    /// hfst-invert's command line: the common and unary options only.
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "Invert a transducer")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,
    }

    impl ToolArgs for Args {
        fn common(&self) -> &CommonArgs {
            &self.common
        }

        fn apply_io(&self, opts: &mut CommonOptions) {
            self.io.apply(opts);
        }
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
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstInvert");
        let (common, _args) = cli::parse::<Args>(common, args)?;

        cli::from_code(run_unary_tool(&common, &SPEC, &mut InvertOp))
    }
}

pub mod kill_paths {
    //! Faithful 1:1 port of tools/src/hfst-kill-paths.cc — the path-killing
    //! command-line tool: removes every arc whose input or output symbol matches a
    //! given symbol (one --symbol, or a list from a --tsv-file), then removes
    //! epsilons. Option handling is clap 4 derive through [`crate::cli`].

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        error, hfst_set_program_name, is_input_stream_in_ol_format, verbose_print,
    };
    use crate::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary, hfst_set_name_unary};
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;
    use hfst::hfst_transducer::HfstTransducer;
    use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};

    /// hfst-kill-paths's command line. Upstream spells the long name of -T
    /// "tsv" while its help calls it --tsv-file; the registered name is the
    /// one that parses, so it is kept.
    // [spec:hfst:def:hfst-kill-paths.parse-options-fn]
    // [spec:hfst:sem:hfst-kill-paths.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "Kill all paths with specific symbols")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,

        /// Remove arcs with input or output symbol SYM or both
        #[arg(short = 'S', long = "symbol", value_name = "SYM")]
        symbol: Option<String>,

        /// Read kill rules from TFILE, which should contain lines with
        /// tab-separated pairs of SYM; comment lines starting with # and empty
        /// lines are ignored
        #[arg(short = 'T', long = "tsv", value_name = "TFILE")]
        tsv: Option<String>,
    }

    impl ToolArgs for Args {
        fn common(&self) -> &CommonArgs {
            &self.common
        }

        fn apply_io(&self, opts: &mut CommonOptions) {
            self.io.apply(opts);
        }

        fn validate(&self, opts: &CommonOptions) -> ToolResult {
            // The C ran this check after the getopt loop but BEFORE the
            // parameter checks, so it outranks the too-many-files diagnostics.
            if self.symbol.is_none() && self.tsv.is_none() {
                error(opts, 1, 0, "Either --symbol or --tsv-file is required");
                return Err(1);
            }
            Ok(())
        }
    }

    /// The tool's option-driven state once the kill-rules file, which the C
    /// opened after the parameter checks, has been opened.
    struct Options {
        /// '-S, --symbol=SYM': the symbol whose arcs to kill.
        symbol: Option<String>,
        /// '-T, --tsv-file=TFILE': the file listing kill symbols.
        tsv_file_name: Option<String>,
        /// The opened kill-rules file (from `tsv_file_name`).
        tsv_file: Option<std::fs::File>,
    }

    impl Options {
        fn open(args: &Args, common: &CommonOptions) -> Result<Options, i32> {
            let mut options = Options {
                symbol: args.symbol.clone(),
                tsv_file_name: args.tsv.clone(),
                tsv_file: None,
            };
            if let Some(name) = &options.tsv_file_name {
                match std::fs::File::open(name) {
                    Ok(f) => options.tsv_file = Some(f),
                    Err(_) => {
                        error(common, 1, 0, &format!("Could not open '{}'", name));
                        return Err(1);
                    }
                }
            }
            Ok(options)
        }
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
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstKillPaths");
        let (common, args) = cli::parse::<Args>(common, args)?;
        let mut options = Options::open(&args, &common)?;

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
            Ok(s) => s,
            Err(e) => {
                error(&common, 1, 0, &format!("{e}"));
                return Err(1);
            }
        };

        if is_input_stream_in_ol_format(&instream, "hfst-kill-paths") {
            return Err(1);
        }

        cli::from_code(process_stream(
            &common,
            &mut options,
            &mut instream,
            &mut outstream,
        ))
    }
}

pub mod minimize {
    //! Port of tools/src/hfst-minimize.cc — the transducer minimisation
    //! command-line tool.
    //!
    //! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
    //! lives in [`CommonOptions`] (the shared -v/-q/-o/-i/... fields), built from
    //! the parsed [`Args`] and threaded into the processing functions. There are
    //! no `static mut` globals and no `unsafe`. This is the template the other
    //! unary tools follow.

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::hfst_set_program_name;
    use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_transducer::{EngineConfig, HfstTransducer};
    use std::borrow::Cow;

    /// hfst-minimize's command line.
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "Minimize a transducer")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,

        /// Encode weights when minimizing (default is false)
        #[arg(short = 'E', long = "encode-weights")]
        encode_weights: bool,
    }

    impl ToolArgs for Args {
        fn common(&self) -> &CommonArgs {
            &self.common
        }

        fn apply_io(&self, opts: &mut CommonOptions) {
            self.io.apply(opts);
        }
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
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstMinimize");
        let (common, args) = cli::parse::<Args>(common, args)?;

        let mut op = MinimizeOp {
            encode_weights: args.encode_weights,
        };
        cli::from_code(run_unary_tool(&common, &SPEC, &mut op))
    }
}

pub mod multiply {
    //! Faithful 1:1 port of tools/src/hfst-multiply.cc — the transducer archive
    //! duplication tool (writes the first transducer of an archive repeatedly).
    //!
    //! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
    //! lives in [`CommonOptions`] (the shared -v/-q/-o/-i/... fields), built from
    //! the parsed [`Args`] and threaded into the processing functions. There are
    //! no `static mut` globals and no `unsafe`.

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        hfst_set_program_name, is_input_stream_in_ol_format, parse_u64, verbose_print,
    };
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;
    use std::io::Write;

    /// hfst-multiply's command line.
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "Use first transducer of an archive repeatedly")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,

        /// Duplicate each transducer NUMBER times; NUMBER must be a positive
        /// integer as parsed by strtoul base 10
        #[arg(
            short = 'n',
            long = "n-times",
            value_name = "NUMBER",
            allow_hyphen_values = true
        )]
        n_times: Option<String>,
    }

    impl Args {
        /// Case 'n': strtoul base 10, fatal on anything else. Without -n the
        /// count stays at the C initialiser of 1.
        fn dupe_count(&self, common: &CommonOptions) -> u64 {
            match &self.n_times {
                Some(n) => parse_u64(common, n, 10),
                None => 1,
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
            // The C rejected a non-numeric NUMBER inside the getopt loop,
            // before the parameter checks; run it here for the same ordering.
            self.dupe_count(opts);
            Ok(())
        }
    }

    // [spec:hfst:def:hfst-multiply.process-stream-fn]
    // [spec:hfst:sem:hfst-multiply.process-stream-fn]
    fn process_stream(
        common: &CommonOptions,
        dupe_count: u64,
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
                    inputname, dupe_count, transducer_n
                ));
                for _ in 0..dupe_count {
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
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstDuplicate");
        let (common, args) = cli::parse::<Args>(common, args)?;
        let dupe_count = args.dupe_count(&common);

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
                return Err(1);
            }
        };
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)

        if is_input_stream_in_ol_format(&instream, "hfst-multiply") {
            return Err(1);
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
                return Err(1);
            }
        };

        cli::from_code(process_stream(
            &common,
            dupe_count,
            &mut instream,
            &mut outstream,
        ))
    }
}

pub mod preprocess_for_optimized_lookup_format {
    //! Faithful 1:1 port of tools/src/hfst-preprocess-for-optimized-lookup-format.cc
    //! — the transducer preprocessing tool (the C++ source is the epsilon-removal /
    //! rebuild tool). Option handling is clap 4 derive through [`crate::cli`].

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{error, hfst_set_program_name, verbose_print};
    use crate::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary, hfst_set_name_unary};
    use hfst::hfst_basic_transducer::HfstBasicTransducer;
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;
    use hfst::hfst_transducer::HfstTransducer;
    use std::io::Write;

    /// hfst-preprocess-for-optimized-lookup-format's command line: the common
    /// and unary options only.
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "Remove epsilons from a transducer")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,
    }

    impl ToolArgs for Args {
        fn common(&self) -> &CommonArgs {
            &self.common
        }

        fn apply_io(&self, opts: &mut CommonOptions) {
            self.io.apply(opts);
        }
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
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstPreprocessForOptimizedLookupFormat");
        let (common, _args) = cli::parse::<Args>(common, args)?;
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

        cli::from_code(process_stream(&common, &mut instream, &mut outstream))
    }
}

pub mod project {
    //! Faithful 1:1 port of tools/src/hfst-project.cc — the transducer projection
    //! command-line tool. Option handling is clap 4 derive through [`crate::cli`].

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{error, hfst_set_program_name};
    use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_transducer::HfstTransducer;
    use std::borrow::Cow;

    /// hfst-project's command line.
    // [spec:hfst:def:hfst-project.parse-options-fn]
    // [spec:hfst:sem:hfst-project.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "Project (extract a level) transducer")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,

        /// Project extracting tape LEVEL: upper, input, first, analysis or
        /// lower, output, second, generation
        #[arg(short = 'p', long = "project", value_name = "LEVEL")]
        project: Option<String>,
    }

    impl Args {
        /// Case 'p': the C compares only the FIRST character of the argument,
        /// case-insensitively (strncasecmp with length 1), against each
        /// candidate word. An argument matching none of them is fatal.
        fn project_input(&self, common: &CommonOptions) -> bool {
            let Some(level) = self.project.as_deref() else {
                return false;
            };
            if first_char_matches(level, "upper")
                || first_char_matches(level, "input")
                || first_char_matches(level, "first")
                || first_char_matches(level, "analysis")
            {
                true
            } else if first_char_matches(level, "lower")
                || first_char_matches(level, "output")
                || first_char_matches(level, "second")
                || first_char_matches(level, "generation")
            {
                false
            } else {
                error(
                    common,
                    1,
                    0,
                    &format!(
                        "unknown project direction {}\nshould be one of upper, input, analysis, first, lower, output, second or generation\n",
                        level
                    ),
                );
                false
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
            // The C rejected an unknown LEVEL inside the getopt loop, before
            // the parameter checks; run it here for the same ordering.
            self.project_input(opts);
            Ok(())
        }
    }

    // strncasecmp(optarg, prefix, 1) == 0 — case-insensitive comparison of the
    // first byte only (the C calls always pass length 1).
    fn first_char_matches(level: &str, prefix: &str) -> bool {
        match level.bytes().next() {
            Some(first) => first.eq_ignore_ascii_case(&prefix.as_bytes()[0]),
            None => false,
        }
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
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstProject");
        let (common, args) = cli::parse::<Args>(common, args)?;

        let mut op = ProjectOp {
            project_input: args.project_input(&common),
        };
        cli::from_code(run_unary_tool(&common, &SPEC, &mut op))
    }
}

pub mod prune_alphabet {
    //! Faithful 1:1 port of tools/src/hfst-prune-alphabet.cc — the transducer
    //! alphabet-pruning command-line tool. Option handling is clap 4 derive
    //! through [`crate::cli`].

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::hfst_set_program_name;
    use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_transducer::HfstTransducer;
    use std::borrow::Cow;

    /// hfst-prune-alphabet's command line.
    //
    // The C cases assign the SAME flag ('f' sets it, 'S' clears it), so the
    // last of the two on the command line decides; mutual overrides_with is
    // how clap says that.
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "Prune the alphabet of a transducer")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,

        /// Force pruning
        #[arg(short = 'f', long = "force", overrides_with = "safe")]
        force: bool,

        /// Prune only if no unknown or identity symbols are used in the
        /// transducer (default)
        #[arg(short = 'S', long = "safe", overrides_with = "force")]
        safe: bool,
    }

    impl ToolArgs for Args {
        fn common(&self) -> &CommonArgs {
            &self.common
        }

        fn apply_io(&self, opts: &mut CommonOptions) {
            self.io.apply(opts);
        }
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
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstPruneAlphabet");
        let (common, args) = cli::parse::<Args>(common, args)?;

        let mut op = PruneAlphabetOp {
            force_pruning: args.force,
        };
        cli::from_code(run_unary_tool(&common, &SPEC, &mut op))
    }
}

pub mod push_labels {
    //! Faithful 1:1 port of tools/src/hfst-push-labels.cc — the label-pushing
    //! command-line tool. Option handling is clap 4 derive through [`crate::cli`].

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        error, hfst_set_program_name, is_input_stream_in_ol_format, verbose_print,
    };
    use crate::unary_ops::{
        UnaryOpSpec, UnaryToolOp, open_input_stream, open_output_stream_like, unary_streams,
    };
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_data_types::PushType;
    use hfst::hfst_transducer::HfstTransducer;
    use std::borrow::Cow;

    /// hfst-push-labels's command line.
    // [spec:hfst:def:hfst-push-labels.parse-options-fn]
    // [spec:hfst:sem:hfst-push-labels.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "Push labels of transducer")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,

        /// Push to DIRECTION: start, initial, begin or end, final
        #[arg(short = 'p', long = "push", value_name = "DIRECTION")]
        push: Option<String>,
    }

    impl Args {
        /// Case 'p': the C lowercases the argument and tests its first letter
        /// against s/i/b (towards the start) and e/f (towards the end).
        fn push_initial(&self, common: &CommonOptions) -> bool {
            let Some(direction) = self.push.as_deref() else {
                return false;
            };
            let lower = direction.to_ascii_lowercase();
            if lower.starts_with('s') || lower.starts_with('i') || lower.starts_with('b') {
                true
            } else if lower.starts_with('e') || lower.starts_with('f') {
                false
            } else {
                error(
                    common,
                    1,
                    0,
                    &format!(
                        "unknown push direction {}\nshould be one of start, initial, begin, end or final.\n",
                        direction
                    ),
                );
                false
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
            // The C rejected an unknown DIRECTION inside the getopt loop,
            // before the parameter checks; run it here for the same ordering.
            self.push_initial(opts);
            Ok(())
        }
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
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstPush");
        let (common, args) = cli::parse::<Args>(common, args)?;

        let mut op = PushLabelsOp {
            push_initial: args.push_initial(&common),
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

        let mut instream = open_input_stream(&common)?;

        if is_input_stream_in_ol_format(&instream, "hfst-push-labels") {
            return Err(1);
        }

        let mut outstream = open_output_stream_like(&common, &instream)?;

        cli::from_code(unary_streams(
            &common,
            &SPEC,
            &mut op,
            &mut instream,
            &mut outstream,
        ))
    }
}

pub mod push_weights {
    //! Faithful 1:1 port of tools/src/hfst-push-weights.cc — the weight pushing
    //! command-line tool. Pushes the weights of a transducer towards its start or
    //! end states. Option handling is clap 4 derive through [`crate::cli`].

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{error, hfst_set_program_name};
    use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_data_types::PushType;
    use hfst::hfst_transducer::HfstTransducer;
    use std::borrow::Cow;

    /// hfst-push-weights's command line.
    // [spec:hfst:def:hfst-push-weights.parse-options-fn]
    // [spec:hfst:sem:hfst-push-weights.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "Push weights of transducer")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,

        /// Push to DIRECTION: start, initial, begin or end, final
        #[arg(short = 'p', long = "push", value_name = "DIRECTION")]
        push: Option<String>,
    }

    impl Args {
        /// Case 'p': the C matches only the FIRST character of the argument
        /// case-insensitively against each candidate word; the default (no
        /// -p at all) is to push towards the end/final state.
        fn push_initial(&self, common: &CommonOptions) -> bool {
            let Some(direction) = self.push.as_deref() else {
                return false;
            };
            if first_char_eq_ignore_case(direction, "start")
                || first_char_eq_ignore_case(direction, "initial")
                || first_char_eq_ignore_case(direction, "begin")
            {
                true
            } else if first_char_eq_ignore_case(direction, "end")
                || first_char_eq_ignore_case(direction, "final")
            {
                false
            } else {
                error(
                    common,
                    1,
                    0,
                    &format!(
                        "unknown push direction {}\nshould be one of start, initial, begin, end or final.\n",
                        direction
                    ),
                );
                false
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
            // The C rejected an unknown DIRECTION inside the getopt loop,
            // before the parameter checks; run it here for the same ordering.
            self.push_initial(opts);
            Ok(())
        }
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
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstPush");
        let (common, args) = cli::parse::<Args>(common, args)?;

        let mut op = PushWeightsOp {
            push_initial: args.push_initial(&common),
        };
        cli::from_code(run_unary_tool(&common, &SPEC, &mut op))
    }
}

pub mod realign {
    //! Faithful 1:1 port of tools/src/hfst-realign.cc — the transducer realign
    //! command-line tool.
    //!
    //! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
    //! lives in [`CommonOptions`] (the shared -v/-q/-o/-i/... fields), built from
    //! the parsed [`Args`] and threaded into the processing functions. There are
    //! no `static mut` globals and no `unsafe`.

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{error, hfst_set_program_name, print_short_help};
    use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_transducer::HfstTransducer;
    use std::borrow::Cow;

    /// The boundary symbol the C initialises and never changes; see
    /// [`Args::validate`] for why -b cannot change it.
    const DEFAULT_BOUNDARY_SYMBOL: u8 = b'>';

    /// hfst-realign's command line.
    // [spec:hfst:def:hfst-realign.parse-options-fn]
    // [spec:hfst:sem:hfst-realign.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "Realign a transducer by pushing labels to the start")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,

        /// Treat SYM as a boundary symbol; SYM must be in the alphabet
        #[arg(short = 'b', long = "boundary", value_name = "SYM")]
        boundary: Option<String>,
    }

    impl ToolArgs for Args {
        fn common(&self) -> &CommonArgs {
            &self.common
        }

        fn apply_io(&self, opts: &mut CommonOptions) {
            self.io.apply(opts);
        }

        fn validate(&self, opts: &CommonOptions) -> ToolResult {
            // Upstream registers --boundary under the option value 'b' but
            // labels its own switch arm 'p', so a returned 'b' matches no case
            // and falls through to the default error arm: giving -b/--boundary
            // is fatal, and the arm that would have set the symbol is dead.
            // Preserved as-is, so the option parses and then rejects.
            if self.boundary.is_some() {
                print_short_help(opts);
                error(opts, 1, 0, "invalid option -b");
                return Err(1);
            }
            Ok(())
        }
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
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstRealign");
        let (common, _args) = cli::parse::<Args>(common, args)?;

        let mut op = RealignOp {
            boundary_symbol: DEFAULT_BOUNDARY_SYMBOL,
        };
        cli::from_code(run_unary_tool(&common, &SPEC, &mut op))
    }
}

pub mod remove_epsilons {
    //! Faithful 1:1 port of tools/src/hfst-remove-epsilons.cc — the transducer
    //! epsilon-removal command-line tool.
    //!
    //! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
    //! lives in [`CommonOptions`] (the shared -v/-q/-o/-i/... fields), built from
    //! the parsed [`Args`] and threaded into the processing functions. There are
    //! no `static mut` globals and no `unsafe`.

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::hfst_set_program_name;
    use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_transducer::HfstTransducer;
    use std::borrow::Cow;

    /// hfst-remove-epsilons's command line: the common and unary options only.
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "Remove epsilons from a transducer")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,
    }

    impl ToolArgs for Args {
        fn common(&self) -> &CommonArgs {
            &self.common
        }

        fn apply_io(&self, opts: &mut CommonOptions) {
            self.io.apply(opts);
        }
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
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstRemoveEpsilons");
        let (common, _args) = cli::parse::<Args>(common, args)?;

        cli::from_code(run_unary_tool(&common, &SPEC, &mut RemoveEpsilonsOp))
    }
}

pub mod repeat {
    //! Faithful 1:1 port of tools/src/hfst-repeat.cc — the transducer repetition
    //! command-line tool.
    //!
    //! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
    //! lives in [`CommonOptions`] (the shared -v/-q/-o/-i/... fields) and a
    //! tool-local [`Options`] built from the parsed [`Args`]. There are no
    //! `static mut` globals and no `unsafe`.

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        error, hfst_set_program_name, hfst_strtonumber, is_input_stream_in_ol_format, verbose_print,
    };
    use crate::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary, hfst_set_name_unary};
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;
    use std::io::Write;

    /// hfst-repeat's command line.
    // [spec:hfst:def:hfst-repeat.parse-options-fn]
    // [spec:hfst:sem:hfst-repeat.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "Repeat transducer")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,

        /// Repeat at least FNUM times; a positive integer or an infinity as
        /// parsed by strtod(3), 0 if omitted, and less than TNUM
        #[arg(
            short = 'f',
            long = "from",
            value_name = "FNUM",
            allow_hyphen_values = true
        )]
        from: Option<String>,

        /// Repeat at most TNUM times; a positive integer or an infinity as
        /// parsed by strtod(3), Inf if omitted
        #[arg(
            short = 't',
            long = "to",
            value_name = "TNUM",
            allow_hyphen_values = true
        )]
        to: Option<String>,
    }

    impl ToolArgs for Args {
        fn common(&self) -> &CommonArgs {
            &self.common
        }

        fn apply_io(&self, opts: &mut CommonOptions) {
            self.io.apply(opts);
        }

        fn validate(&self, opts: &CommonOptions) -> ToolResult {
            // Both numbers were parsed inside the C getopt loop, so a
            // non-numeric FNUM/TNUM is rejected before the parameter checks;
            // the range checks run after them, in Options::resolve.
            Options::parse_bounds(self, opts);
            Ok(())
        }
    }

    /// hfst-repeat's option-driven state (the former tool-specific `static mut`s).
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

    impl Options {
        /// The 'f' and 't' cases: strtod each bound and note whether it came
        /// out infinite. Fatal on a non-number.
        fn parse_bounds(args: &Args, common: &CommonOptions) -> Options {
            let mut options = Options::default();
            if let Some(from) = &args.from {
                let mut from_inf = false;
                options.at_least = hfst_strtonumber(common, from, Some(&mut from_inf)) as u64;
                options.from_infinity = from_inf;
            }
            if let Some(to) = &args.to {
                let mut to_inf = false;
                options.at_most = hfst_strtonumber(common, to, Some(&mut to_inf)) as u64;
                options.to_infinity = to_inf;
            }
            options
        }

        /// The post-loop validation the C ran AFTER the parameter checks.
        fn resolve(args: &Args, common: &CommonOptions) -> Options {
            let options = Options::parse_bounds(args, common);
            if options.at_least > options.at_most {
                error(
                    common,
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
                    common,
                    1,
                    0,
                    &format!("Cannot repeat from infinity to {} times\n", options.at_most),
                );
            }
            options
        }
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
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstRepeat");
        let (common, args) = cli::parse::<Args>(common, args)?;
        let options = Options::resolve(&args, &common);

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
            Ok(s) => s,
            Err(e) => {
                error(&common, 1, 0, &format!("{e}"));
                return Err(1);
            }
        };

        if is_input_stream_in_ol_format(&instream, "hfst-repeat") {
            return Err(1);
        }

        cli::from_code(process_stream(
            &common,
            &options,
            &mut instream,
            &mut outstream,
        ))
    }
}

pub mod reverse {
    //! Faithful 1:1 port of tools/src/hfst-reverse.cc — the transducer reversion
    //! command-line tool.
    //!
    //! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
    //! lives in [`CommonOptions`] (the shared -v/-q/-o/-i/... fields), built from
    //! the parsed [`Args`] and threaded into the processing functions. There are
    //! no `static mut` globals and no `unsafe`.

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::hfst_set_program_name;
    use crate::unary_ops::{UnaryOpSpec, UnaryToolOp, run_unary_tool};
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_transducer::HfstTransducer;
    use std::borrow::Cow;

    /// hfst-reverse's command line: the common and unary options only.
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "Reverse a transducer")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,
    }

    impl ToolArgs for Args {
        fn common(&self) -> &CommonArgs {
            &self.common
        }

        fn apply_io(&self, opts: &mut CommonOptions) {
            self.io.apply(opts);
        }
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
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstReverse");
        let (common, _args) = cli::parse::<Args>(common, args)?;

        cli::from_code(run_unary_tool(&common, &SPEC, &mut ReverseOp))
    }
}
