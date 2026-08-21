//! Report and metadata tools: the ones that describe, name, or slice an
//! archive rather than transform the transducers in it.
//!
//! Contains, as inline modules:
//! - `dump_alphabets`
//! - `edit_metadata`
//! - `head`
//! - `info`
//! - `name`
//! - `split`
//! - `strip_header`
//! - `tail`
//! - `traverse`

pub mod dump_alphabets {
    //! Faithful 1:1 port of tools/src/hfst-dump-alphabets.cc — the alphabet dump
    //! command-line tool. Option handling is clap 4 derive through
    //! [`crate::cli`]; the rest drives the hfst-cli foundation (globals,
    //! commandline).

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{hfst_set_program_name, verbose_print};
    use hfst::hfst_basic_transducer::HfstBasicTransducer;
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_symbol_defs::StringSet;
    use std::io::Write;

    // add tools-specific variables here
    // [spec:hfst:def:hfst-dump-alphabets.alphadumpformat]
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum AlphaDumpFormat {
        Tsv,
        Vislcg3List,
        Vislcg3Tags,
    }

    /// hfst-dump-alphabets's resolved options (the former tool-specific
    /// `static mut`s).
    struct Options {
        output_format: AlphaDumpFormat,
        print_seen: bool,
        print_meta: bool,
        only_multichars: bool,
    }

    // [spec:hfst:def:hfst-dump-alphabets.is-multichar-fn]
    // [spec:hfst:sem:hfst-dump-alphabets.is-multichar-fn]
    fn is_multichar(s: &str) -> bool {
        if s.len() > 2 {
            return s.starts_with('+') || s.starts_with(' ') || s.starts_with('@');
        }
        false
    }

    /// hfst-dump-alphabets's command line.
    //
    // The two exclusion switches keep their upstream names: '-1,
    // --include-seen' EXCLUDES the alphabet seen in the automaton and '-2,
    // --include-metadata' EXCLUDES the header alphabet — the long names say
    // the opposite of what the cases do, and the usage text spelled them
    // '--exclude-seen' / '--exclude-metadata' while the getopt table did not
    // accept those. Preserved bug-for-bug.
    // [spec:hfst:def:hfst-dump-alphabets.parse-options-fn]
    // [spec:hfst:sem:hfst-dump-alphabets.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "Print alphabets of automaton")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,

        /// Print alphabet in AFORMAT: tsv, vislcg3-list or vislcg3-tags
        #[arg(short = 'f', long = "format", value_name = "AFORMAT")]
        format: Option<String>,

        /// Ignore alphabets seen in automaton
        #[arg(short = '1', long = "include-seen")]
        exclude_seen: bool,

        /// Ignore alphabets from headers
        #[arg(short = '2', long = "include-metadata")]
        exclude_metadata: bool,

        /// Whether the --format note is printed. The C emitted it from inside
        /// the getopt loop, so it appeared only when -v had already been read;
        /// [`ToolArgs::absorb_matches`] recovers that from the match indices.
        #[arg(skip)]
        announce_format: bool,
    }

    impl Args {
        /// Case 'f': the AFORMAT vocabulary. `announce` carries the verbose
        /// note the C printed as it read the option, so resolving the value a
        /// second time for the tool body stays silent.
        fn dump_format(&self, common: &CommonOptions, announce: bool) -> (AlphaDumpFormat, bool) {
            let Some(name) = self.format.as_deref() else {
                return (AlphaDumpFormat::Tsv, false);
            };
            let (format, only_multichars, note) = match name {
                "tsv" => (
                    AlphaDumpFormat::Tsv,
                    false,
                    "printing one symbol per line\n",
                ),
                "vislcg3-list" => (
                    AlphaDumpFormat::Vislcg3List,
                    true,
                    "printing LIST x = x ; for VISL CG 3...\n",
                ),
                "vislcg3-tags" => (
                    AlphaDumpFormat::Vislcg3Tags,
                    true,
                    "printing STRICT-TAGS += for VISL CG 3...\n",
                ),
                other => {
                    eprintln!("Error: unrecognised format {}", other);
                    std::process::exit(1);
                }
            };
            if announce {
                verbose_print(common, note);
            }
            (format, only_multichars)
        }

        fn options(&self, common: &CommonOptions) -> Options {
            let (output_format, only_multichars) = self.dump_format(common, false);
            Options {
                output_format,
                print_seen: !self.exclude_seen,
                print_meta: !self.exclude_metadata,
                only_multichars,
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
            // The C read --format inside its getopt loop, before the parameter
            // checks: the verbose note and the unknown-name refusal both land
            // here for the same ordering.
            self.dump_format(opts, self.announce_format);
            Ok(())
        }

        fn absorb_matches(&mut self, matches: &clap::ArgMatches) {
            // 'f' called verbose_print with whatever verbosity the loop had
            // reached, so '-v -f tsv' printed the note and '-f tsv -v' did not.
            self.announce_format = matches.get_flag("verbose")
                && matches!(
                    (matches.index_of("verbose"), matches.index_of("format")),
                    (Some(verbose), Some(format)) if verbose < format
                );
        }
    }

    // [spec:hfst:def:hfst-dump-alphabets.process-stream-fn]
    // [spec:hfst:sem:hfst-dump-alphabets.process-stream-fn]
    fn process_stream(
        common: &CommonOptions,
        options: &Options,
        instream: &mut HfstInputStream<'_>,
    ) -> i32 {
        // Data output goes to a std stream (the std counterpart of the libc
        // outfile FILE*); `emit` writes a string and ignores errors, matching the
        // old fput/fputs.
        let mut out = match common.output_writer() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("hfst-dump-alphabets: could not open output: {e}");
                return 1;
            }
        };
        let mut emit = |s: &str| {
            let _ = out.write_all(s.as_bytes());
        };
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            if transducer_n < 2 {
                verbose_print(common, "Alphadumping...\n");
            } else {
                verbose_print(common, &format!("Alphadumping... {}\n", transducer_n));
            }
            let any = match instream.read() {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("hfst-dump-alphabets: {e}");
                    return 1;
                }
            };
            // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
            crate::for_any!(any, trans => {
                let mutt = HfstBasicTransducer::new_from_transducer(&trans);
                // unsigned int initial_state = 0; // mutt.get_initial_state();
                let transducer_alphabet = match trans.get_alphabet() {
                    Ok(a) => a,
                    Err(e) => {
                        eprintln!("hfst-dump-alphabets: {e}");
                        return 1;
                    }
                };
                let transducer_knows_alphabet = true;
                let found_alphabet: StringSet = mutt.symbols_used();
                if options.output_format == AlphaDumpFormat::Vislcg3Tags {
                    emit(
                        "## automatically generated VISL CG 3 file from HFST automaton's alphabet data:\n",
                    );
                    emit("## (some statistics here TODO)\n");
                    emit("STRICT-TAGS +=\n");
                } else if options.output_format == AlphaDumpFormat::Vislcg3List {
                    emit(
                        "## automatically generated VISL CG 3 file from HFST automaton's alphabet data:\n",
                    );
                    emit("## (some statistics here TODO)\n");
                }
                if options.print_meta {
                    if transducer_knows_alphabet {
                        for s in transducer_alphabet.iter() {
                            if options.only_multichars && !is_multichar(s) {
                                continue;
                            }
                            if options.output_format == AlphaDumpFormat::Tsv {
                                emit(&format!("{}\n", s));
                            } else if options.output_format == AlphaDumpFormat::Vislcg3Tags {
                                emit(&format!("\t{}\n", s));
                            } else if options.output_format == AlphaDumpFormat::Vislcg3List {
                                emit(&format!("LIST {} = {} ;\n", s, s));
                            }
                        }
                    } else {
                        eprintln!("Error: cannot dump non-existent header alphabet");
                        std::process::exit(1);
                    }
                }
                if options.print_seen {
                    for s in found_alphabet.iter() {
                        if options.only_multichars && !is_multichar(s) {
                            continue;
                        }
                        if options.output_format == AlphaDumpFormat::Tsv {
                            emit(&format!("{}\n", s));
                        } else if options.output_format == AlphaDumpFormat::Vislcg3Tags {
                            emit(&format!("\t{}\n", s));
                        } else if options.output_format == AlphaDumpFormat::Vislcg3List {
                            emit(&format!("LIST {} = {} ;\n", s, s));
                        }
                    }
                }
            });
        } // for each automaton
        if options.output_format == AlphaDumpFormat::Vislcg3Tags {
            emit("\t;\n");
        }
        0
    }

    // [spec:hfst:def:hfst-dump-alphabets.main-fn]
    // [spec:hfst:sem:hfst-dump-alphabets.main-fn]
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstSummarize");
        let (common, args) = cli::parse::<Args>(common, args)?;
        let options = args.options(&common);
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
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // that calls error(EXIT_FAILURE, ...) is not reproduced here.)
        let instream_res = if input_opened {
            HfstInputStream::new_filename(&common.input_filename)
        } else {
            HfstInputStream::new()
        };
        let mut instream = match instream_res {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "hfst-dump-alphabets: {} is not a valid transducer file: {e}",
                    common.input_filename
                );
                return Err(1);
            }
        };
        let _retval = process_stream(&common, &options, &mut instream);

        Ok(())
    }
}

pub mod edit_metadata {
    //! Faithful 1:1 port of tools/src/hfst-edit-metadata.cc — the transducer
    //! metadata tool.
    //!
    //! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
    //! lives in [`CommonOptions`] (the shared `-v/-q/-o/-i/…` fields) and a
    //! tool-local [`Options`], threaded into the processing functions. There are
    //! no `static mut` globals and no `unsafe`.

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        error, hfst_set_program_name, parse_u64, verbose_print, warning,
    };
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;
    use std::collections::BTreeMap;
    use std::io::Write;

    /// hfst-edit-metadata's command line.
    //
    // '-p' takes an OPTIONAL argument, so it only ever binds a value written
    // as '-pNAME' or '--print-name=NAME'; a following word is an operand, not
    // the property name.
    // [spec:hfst:def:hfst-edit-metadata.parse-options-fn]
    // [spec:hfst:sem:hfst-edit-metadata.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(
        about = "Name a transducer",
        after_help = "If NAME is omitted from --print-name, all values are printed"
    )]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,

        /// Add or replace property ANAME with VALUE
        #[arg(
            short = 'a',
            long = "add",
            value_name = "ANAME=VALUE",
            action = clap::ArgAction::Append
        )]
        add: Vec<String>,

        /// Print the current NAME; without NAME, print every property
        //
        // The default-missing value is a NUL, which no argv string can carry:
        // it marks the bare '-p' while still giving clap a value to index, so
        // the '-a'-versus-bare-'-p' ordering below is recoverable.
        #[arg(
            short = 'p',
            long = "print-name",
            value_name = "NAME",
            num_args = 0..=1,
            require_equals = true,
            default_missing_value = "\0"
        )]
        print_name: Option<String>,

        /// Truncate added properties' lengths to LEN
        #[arg(short = 't', long = "truncate_length", value_name = "LEN")]
        truncate_length: Option<String>,

        /// Whether every property is printed. Both '-a' and a bare '-p' wrote
        /// this in the C getopt loop and the last write won, so it is recovered
        /// from the match indices rather than from the field values.
        #[arg(skip = true)]
        print_all_properties: bool,
    }

    impl Args {
        /// Case 'a': split each ANAME=VALUE at its first '=', refusing one
        /// without a separator exactly as the C did.
        fn properties(&self, common: &CommonOptions) -> BTreeMap<String, String> {
            let mut properties = BTreeMap::new();
            for spec in &self.add {
                match spec.find('=') {
                    None => {
                        error(
                            common,
                            1,
                            0,
                            &format!("Equals sign `=' missing from {}", spec),
                        );
                    }
                    Some(idx) => {
                        properties.insert(spec[..idx].to_string(), spec[idx + 1..].to_string());
                    }
                }
            }
            properties
        }

        /// Case 't': hfst_strtoul(optarg, 10), fatal on anything else.
        fn truncate_length(&self, common: &CommonOptions) -> u64 {
            match &self.truncate_length {
                Some(len) => parse_u64(common, len, 10),
                None => 0,
            }
        }

        /// The property '-p' names, with the bare-'-p' sentinel read back as
        /// the C's NULL print_property.
        fn named_property(&self) -> Option<String> {
            self.print_name
                .as_deref()
                .filter(|n| *n != "\0")
                .map(str::to_string)
        }

        fn options(&self, common: &CommonOptions) -> Options {
            Options {
                properties: self.properties(common),
                print_all_properties: self.print_all_properties,
                print_property: self.named_property(),
                truncate_length: self.truncate_length(common),
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
            // Both refusals fired inside the C getopt loop, before the
            // parameter checks; run them here for the same ordering.
            self.properties(opts);
            self.truncate_length(opts);
            Ok(())
        }

        // The C loop starts print_all_properties at true, '-a' sets it false
        // and a bare '-p' sets it true, so the flag is decided by whichever
        // came last. A '-p' carrying a value never touches it.
        fn absorb_matches(&mut self, matches: &clap::ArgMatches) {
            let last_add = matches.indices_of("add").and_then(|i| i.max());
            let bare_print = self.print_name.as_deref() == Some("\0");
            self.print_all_properties = match (last_add, bare_print) {
                (None, _) => true,
                (Some(_), false) => false,
                (Some(add), true) => matches
                    .index_of("print_name")
                    .is_some_and(|print| print > add),
            };
        }
    }

    /// hfst-edit-metadata's resolved options (the former tool-specific
    /// `static mut`s).
    struct Options {
        /// '-a, --add=ANAME=VALUE': the properties to add or replace.
        properties: BTreeMap<String, String>,
        /// whether all properties should be printed (the default).
        print_all_properties: bool,
        /// '-p, --print-name[=NAME]': the specific property to print. C used a
        /// NULL char* as "no specific property requested"; modelled as Option.
        print_property: Option<String>,
        /// '-t, --truncate_length=LEN': truncate added property lengths to LEN.
        truncate_length: u64,
    }

    // [spec:hfst:def:hfst-edit-metadata.process-stream-fn]
    // [spec:hfst:sem:hfst-edit-metadata.process-stream-fn]
    fn process_stream(
        common: &CommonOptions,
        options: &Options,
        instream: &mut HfstInputStream<'_>,
        outstream: &mut HfstOutputStream,
    ) -> i32 {
        let mut out = match common.output_writer() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("hfst-edit-metadata: cannot open output: {e}");
                return 1;
            }
        };
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;

            if transducer_n > 1
                && (options.print_all_properties || options.print_property.is_some())
            {
                eprintln!("--- ");
            }

            if transducer_n == 1 {
                verbose_print(common, &format!("Metadata {}...\n", common.input_filename));
            } else {
                verbose_print(
                    common,
                    &format!("Metadata {}...{}\n", common.input_filename, transducer_n),
                );
            }

            let any = match instream.read() {
                Ok(v) => v,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
            crate::for_any!(any, trans => {
                let mut trans = trans;
                if !options.print_all_properties && options.print_property.is_none() {
                    for (key, val) in options.properties.iter() {
                        if key == "type" {
                            warning(
                                common,
                                0,
                                0,
                                "Changing `type' metadata will not change type of transducer in file;\n\
                                 having wrong type may cause breakage, use with caution",
                            );
                        } else if key == "version" {
                            warning(
                                common,
                                0,
                                0,
                                "Changing `version' changes parsing semantics for header;\n\
                                 use with caution",
                            );
                        } else if key == "character-encoding" && !(val == "utf-8" || val == "UTF-8") {
                            error(
                                common,
                                1,
                                0,
                                "Cannot set `character-encoding' to unsupported value;\n\
                                 consider recoding sources of automaton",
                            );
                        }
                        if options.truncate_length > 0 {
                            // C: hfst_strndup(value.c_str(), truncate_length) — copy
                            // up to truncate_length bytes (NUL-terminating early).
                            let bytes = val.as_bytes();
                            let n = (options.truncate_length as usize).min(bytes.len());
                            let truncated = String::from_utf8_lossy(&bytes[..n]).into_owned();
                            trans.set_property(key, &truncated);
                        } else {
                            trans.set_property(key, val);
                        }
                    }
                    if let Err(e) = outstream.redirect(&mut trans) {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                } else {
                    let props = trans.get_properties();
                    if options.print_all_properties {
                        for (key, val) in props.iter() {
                            let _ = writeln!(out, "{}: {}", key, val);
                        }
                    } else {
                        let pp = options.print_property.clone().unwrap_or_default();
                        let _ = writeln!(out, "{}", props.get(&pp).unwrap());
                    }
                }
            });
        }
        instream.close();
        outstream.close();
        0
    }

    // [spec:hfst:def:hfst-edit-metadata.main-fn]
    // [spec:hfst:sem:hfst-edit-metadata.main-fn]
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstEditMetadata");
        let (common, args) = cli::parse::<Args>(common, args)?;
        let options = args.options(&common);

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

        cli::from_code(process_stream(
            &common,
            &options,
            &mut instream,
            &mut outstream,
        ))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use clap::{CommandFactory, FromArgMatches};

        /// Parse an argv the way [`cli::parse`] does, so the ordering hook runs.
        fn parse(argv: &[&str]) -> Args {
            let matches = Args::command()
                .try_get_matches_from(argv)
                .expect("argv parses");
            let mut args = Args::from_arg_matches(&matches).expect("matches convert");
            args.absorb_matches(&matches);
            args
        }

        // The C loop's last write to print_all_properties wins. The bare '-p'
        // rides on a NUL default-missing value so clap has an index to compare;
        // if a clap upgrade ever stops recording one, the last two cases here
        // are what notices.
        #[test]
        fn add_and_bare_print_resolve_by_position() {
            assert!(parse(&["hfst-edit-metadata"]).print_all_properties);
            assert!(parse(&["hfst-edit-metadata", "-p"]).print_all_properties);
            assert!(parse(&["hfst-edit-metadata", "--print-name=name"]).print_all_properties);
            assert!(!parse(&["hfst-edit-metadata", "-a", "k=v"]).print_all_properties);
            assert!(parse(&["hfst-edit-metadata", "-a", "k=v", "-p"]).print_all_properties);
            assert!(!parse(&["hfst-edit-metadata", "-p", "-a", "k=v"]).print_all_properties);
        }

        /// A bare '-p' means "every property"; only an attached value names one.
        #[test]
        fn only_an_attached_value_names_a_property() {
            assert_eq!(parse(&["hfst-edit-metadata", "-p"]).named_property(), None);
            assert_eq!(
                parse(&["hfst-edit-metadata", "--print-name=name"]).named_property(),
                Some("name".to_string())
            );
        }
    }
}

pub mod head {
    //! Faithful 1:1 port of tools/src/hfst-head.cc — the transducer archive head
    //! splitting tool.
    //!
    //! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
    //! lives in [`CommonOptions`] (the shared `-v/-q/-o/-i/…` fields) and a
    //! tool-local [`Options`], threaded into the processing functions. There are
    //! no `static mut` globals and no `unsafe`.

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        error, hfst_set_program_name, parse_i64, verbose_print, warning,
    };
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;
    use hfst::hfst_transducer::AnyTransducer;
    use std::collections::VecDeque;

    /// hfst-head's own options (the former tool-specific `static mut`s).
    struct Options {
        /// '-n, --n-first=[-]K': number of transducers to keep from the head.
        head_count: i64,
    }

    /// hfst-head's command line.
    // [spec:hfst:def:hfst-head.parse-options-fn]
    // [spec:hfst:sem:hfst-head.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(
        about = "Get first transducers from an archive",
        after_help = "K must be an integer, as parsed by strtoul base 10, and not 0.
If K is omitted default is 1."
    )]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,

        /// Print the first K transducers; with the leading `-', print all but
        /// the last K transducers
        #[arg(
            short = 'n',
            long = "n-first",
            value_name = "[-]K",
            allow_hyphen_values = true
        )]
        n_first: Option<String>,
    }

    impl Args {
        /// Case 'n': hfst_strtol(optarg, 10), fatal on anything else. Without
        /// -n the count stays at the C initialiser of 1.
        fn head_count(&self, common: &CommonOptions) -> i64 {
            match &self.n_first {
                Some(k) => parse_i64(common, k, 10),
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
            // The C rejected a non-numeric K inside the getopt loop, before the
            // parameter checks; run it here for the same ordering. The
            // count-of-0 warning came AFTER them and stays in the tool body.
            self.head_count(opts);
            Ok(())
        }
    }

    // [spec:hfst:def:hfst-head.process-stream-fn]
    // [spec:hfst:sem:hfst-head.process-stream-fn]
    fn process_stream(
        common: &CommonOptions,
        options: &Options,
        instream: &mut HfstInputStream<'_>,
        outstream: &mut HfstOutputStream,
    ) -> i32 {
        let mut transducer_n: usize = 0;
        if options.head_count > 0 {
            while instream.is_good() && (transducer_n < options.head_count as usize) {
                transducer_n += 1;
                let mut trans = match instream.read() {
                    Ok(t) => t,
                    Err(e) => {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                };
                let mut inputname = trans.get_name();
                if inputname.is_empty() {
                    inputname = common.input_filename.clone();
                }
                verbose_print(
                    common,
                    &format!("Forwarding {}...{}\n", inputname, transducer_n),
                );
                if let Err(e) = trans.write(outstream) {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            }
        } else if options.head_count < 0 {
            let mut first_but_n: VecDeque<AnyTransducer> = VecDeque::new();
            verbose_print(
                common,
                &format!("Counting all but last {}\n", options.head_count),
            );
            while instream.is_good() {
                transducer_n += 1;
                let trans = match instream.read() {
                    Ok(t) => t,
                    Err(e) => {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                };
                first_but_n.push_back(trans);
            }
            if (-options.head_count) as usize > first_but_n.len() {
                warning(
                    common,
                    0,
                    0,
                    &format!(
                        "Stream in {} has less than {} automata; Nothing will be written to output",
                        common.input_filename, -options.head_count
                    ),
                );
            }
            for _ in 0..(-options.head_count) {
                if !first_but_n.is_empty() {
                    first_but_n.pop_back();
                }
            }
            while !first_but_n.is_empty() {
                // C: copied the front and popped it afterwards; taking it by
                // value is the same write in one move.
                let mut trans = first_but_n
                    .pop_front()
                    .expect("first_but_n is non-empty per the enclosing while condition");
                let mut inputname = trans.get_name();
                if inputname.is_empty() {
                    inputname = common.input_filename.clone();
                }
                verbose_print(
                    common,
                    &format!("Forwarding {}...{}\n", inputname, transducer_n),
                );
                if let Err(e) = trans.write(outstream) {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
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

    // [spec:hfst:def:hfst-head.main-fn]
    // [spec:hfst:sem:hfst-head.main-fn]
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.2", "HfstHead");
        let (common, args) = cli::parse::<Args>(common, args)?;
        let options = Options {
            head_count: args.head_count(&common),
        };
        // The C emitted this after the common + unary parameter checks.
        if options.head_count == 0 {
            warning(&common, 0, 0, "Argument 0 for count is not sensible");
        }

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
        let instream_result = if input_opened {
            HfstInputStream::new_filename(&common.input_filename)
        } else {
            HfstInputStream::new()
        };
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)
        let mut instream = match instream_result {
            Ok(s) => s,
            Err(e) => {
                error(&common, 1, 0, &format!("{e}"));
                return Err(1);
            }
        };

        let ty = instream.get_type();
        let outstream_result = if output_opened {
            HfstOutputStream::new_filename(&common.output_filename, ty, true)
        } else {
            HfstOutputStream::new(ty, true)
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

pub mod info {
    //! Port of tools/src/hfst-info.cc — the "show or test HFST versions and
    //! features" command-line tool. It reads no transducer streams; it parses
    //! version/feature test options, then prints or tests the build's version and
    //! features. Option handling is clap 4 derive through [`crate::cli`].
    //!
    //! Deliberately NOT faithful in what it reports. Upstream answered `-a/-e/-m`
    //! and `-f` from autoconf's config.h, and this port had those values frozen as
    //! literals copied from a C++ 3.17.1 build — so it announced a version it is
    //! not and backends it does not have. This tool's entire job is to be believed
    //! by a configure script, so it answers from what this build actually is: the
    //! crate version, the upstream interface-compatibility version, and the
    //! backend table below.
    //!
    //! Version tests speak two namespaces. Existing build systems (every Giella
    //! language repo) gate on upstream HFST versions (`--atleast-version=3.16.0`),
    //! which this build satisfies through [`HFST_COMPAT_VERSION`] — the upstream
    //! release whose tool interface it provides. The fork's own version answers
    //! too, so `-a` keeps working against Divvun HFST versions once those are what
    //! scripts ask about. A requirement is met if either version meets it;
    //! identity reporting (`-V`, the listing) never claims to BE upstream HFST.
    //!
    //! Idiomatic option handling: the tool's state lives in a tool-local
    //! [`Options`] built from the parsed [`Args`] and threaded into `run`. The
    //! shared `-v/-q/-o/…` options are accepted and discarded, as the C's
    //! switch did — it never chained the common cases.

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{error, hfst_set_program_name, verbose_print, version_line};

    const EXIT_FAILURE: i32 = 1;
    use std::collections::BTreeSet;

    const PACKAGE_NAME: &str = "Divvun HFST";
    const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");

    // CARGO_PKG_VERSION_{MAJOR,MINOR,PATCH} are pure digit runs — any pre-release
    // tag lands in CARGO_PKG_VERSION_PRE — so a non-digit is a build-time failure
    // rather than something to handle at runtime.
    const fn version_component(s: &str) -> i64 {
        let b = s.as_bytes();
        let mut i = 0;
        let mut v: i64 = 0;
        while i < b.len() {
            assert!(b[i].is_ascii_digit(), "version component is not numeric");
            v = v * 10 + (b[i] - b'0') as i64;
            i += 1;
        }
        v
    }

    /// This build's version in the packed `major*10^8 + minor*10^4 + patch` form
    /// that `-a/-e/-m` compare against — the same encoding `parse_version_string`
    /// produces, so the operand and the subject are on one scale.
    const HFST_LONGVERSION: i64 =
        version_component(env!("CARGO_PKG_VERSION_MAJOR")) * 10000 * 10000
            + version_component(env!("CARGO_PKG_VERSION_MINOR")) * 10000
            + version_component(env!("CARGO_PKG_VERSION_PATCH"));

    /// The upstream HFST release whose command-line interface this build provides:
    /// the C++ oracle the port is validated against (Giella lang builds produce
    /// equivalent artifacts). Configure scripts across the Giella ecosystem gate on
    /// `--atleast-version=3.16.0` in this namespace; without a compat answer no
    /// language repo can configure against this toolchain.
    const HFST_COMPAT_VERSION: &str = "3.17.1";
    const HFST_COMPAT_LONGVERSION: i64 = 3 * 10000 * 10000 + 17 * 10000 + 1;

    /// One backend, as `-f` tests it and as the listing reports it.
    struct Feature {
        label: &'static str,
        /// Every spelling `-f` accepts for it.
        names: &'static [&'static str],
        present: bool,
    }

    /// What this build has. The `-f` gate and the informational listing both read
    /// this one table: the bug it replaces was the two answers disagreeing, with
    /// `-f foma` failing while the listing said "foma supported".
    const FEATURES: &[Feature] = &[
        Feature {
            label: "OpenFst (tropical)",
            names: &["openfst", "OPENFST", "HAVE_OPENFST"],
            present: true,
        },
        Feature {
            label: "foma",
            names: &["foma", "FOMA", "HAVE_FOMA"],
            present: cfg!(feature = "foma"),
        },
        Feature {
            label: "Unicode (ICU)",
            names: &["icu", "ICU", "USE_ICU_UNICODE"],
            present: true,
        },
        // Out of scope for this fork, and named here so asking for one gets a
        // refusal instead of the silence that reads as "old build, didn't say".
        Feature {
            label: "OpenFst (log)",
            names: &["openfst-log", "OPENFST_LOG", "HAVE_OPENFST_LOG"],
            present: false,
        },
        Feature {
            label: "SFST",
            names: &["sfst", "SFST", "HAVE_SFST"],
            present: false,
        },
        Feature {
            label: "xfsm",
            names: &["xfsm", "XFSM", "HAVE_XFSM"],
            present: false,
        },
    ];

    /// hfst-info's own options (the former tool-specific `static mut`s).
    struct Options {
        min_version: i64,
        exact_version: i64,
        max_version: i64,
        // required_features collected as a set<string>; BTreeSet preserves the
        // sorted-iteration order the C++ std::set used.
        required_features: Option<BTreeSet<String>>,
    }

    impl Default for Options {
        fn default() -> Options {
            Options {
                min_version: -1,
                exact_version: -1,
                max_version: -1,
                required_features: None,
            }
        }
    }

    // strtoul(s, &endptr, 10): parse a leading run of base-10 digits from 's',
    // returning the parsed value and the unparsed remainder (the C 'endptr'). Like
    // libc strtoul it accepts no digits (value 0, whole string remaining).
    fn parse_u64_prefix(s: &str) -> (u64, &str) {
        let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
        let val = s[..end].parse::<u64>().unwrap_or(0);
        (val, &s[end..])
    }

    // [spec:hfst:def:hfst-info.parse-version-string-fn]
    // [spec:hfst:sem:hfst-info.parse-version-string-fn]
    fn parse_version_string(common: &CommonOptions, s: &str) -> i64 {
        let (major, endptr) = parse_u64_prefix(s);
        let major = major as i64;
        if endptr.is_empty() {
            return major * 10000 * 10000;
        } else if !endptr.starts_with('.') {
            error(
                common,
                EXIT_FAILURE,
                0,
                &format!("cannot parse version string from {}", endptr),
            );
        }
        let s = &endptr[1..];
        let (minor, endptr) = parse_u64_prefix(s);
        let minor = minor as i64;
        if endptr.is_empty() {
            return (major * 10000 * 10000) + (minor * 10000);
        } else if !endptr.starts_with('.') {
            error(
                common,
                EXIT_FAILURE,
                0,
                &format!("cannot parse version string from {}", endptr),
            );
        }
        let s = &endptr[1..];
        let (patch, endptr) = parse_u64_prefix(s);
        let patch = patch as i64;
        if endptr.is_empty() {
            return (major * 10000 * 10000) + (minor * 10000) + patch;
        } else {
            error(
                common,
                EXIT_FAILURE,
                0,
                &format!("cannot parse version string from {}", endptr),
            );
        }
        -1
    }

    /// hfst-info's command line.
    //
    // This tool's switch handles only its own version/feature options plus
    // help and version: '-v/-q/-s/-d/-o/--colour' are accepted and discarded,
    // and no output file is resolved, which is why the report goes to stdout.
    // [`ToolArgs::applies_common_options`] carries that.
    // [spec:hfst:def:hfst-info.parse-options-fn]
    // [spec:hfst:sem:hfst-info.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(
        about = "show or test HFST versions and features",
        after_help = "MVER, EVER or UVER version vectors must be composed of one to three full stop separated runs of digits.
A requirement is met if either this build's own version or the upstream HFST version it is interface-compatible with (3.17.1) meets it.
FEAT should be name of feature supported by HFST, such as openfst, foma or icu"
    )]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,

        /// Require at least MVER version of HFST
        #[arg(short = 'a', long = "atleast-version", value_name = "MVER")]
        atleast_version: Option<String>,

        /// Require exactly EVER version of HFST
        #[arg(short = 'e', long = "exact-version", value_name = "EVER")]
        exact_version: Option<String>,

        /// Require at most UVER version of HFST
        #[arg(short = 'm', long = "max-version", value_name = "UVER")]
        max_version: Option<String>,

        /// Require named FEAT support from HFST
        #[arg(
            short = 'f',
            long = "require-feature",
            value_name = "FEAT",
            action = clap::ArgAction::Append
        )]
        require_feature: Vec<String>,

        /// Accepted and ignored, as the C's leftover free arguments were
        #[arg(value_name = "INFILE", num_args = 0..)]
        infiles: Vec<String>,
    }

    impl Args {
        fn options(&self, common: &CommonOptions) -> Options {
            let version = |v: &Option<String>| match v {
                Some(s) => parse_version_string(common, s),
                None => -1,
            };
            let required_features = if self.require_feature.is_empty() {
                None
            } else {
                Some(
                    self.require_feature
                        .iter()
                        .cloned()
                        .collect::<BTreeSet<_>>(),
                )
            };
            Options {
                min_version: version(&self.atleast_version),
                exact_version: version(&self.exact_version),
                max_version: version(&self.max_version),
                required_features,
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

        fn validate(&self, opts: &CommonOptions) -> ToolResult {
            // parse_version_string exits on a malformed vector; the C did that
            // inside its getopt loop.
            self.options(opts);
            Ok(())
        }
    }

    // [spec:hfst:def:hfst-info.main-fn]
    // [spec:hfst:sem:hfst-info.main-fn]
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstInfo");
        let (mut common, args) = cli::parse::<Args>(common, args)?;
        let options = args.options(&common);
        let _ = &args.infiles;
        // With no test selected the tool reports everything, so it turns
        // verbosity on itself.
        let feature_count = options.required_features.as_ref().map_or(0, |s| s.len());
        if (options.min_version == -1)
            && (options.max_version == -1)
            && (options.exact_version == -1)
            && (feature_count == 0)
            && (!common.verbose)
        {
            common.verbose = true;
            verbose_print(&common, "No tests selected; printing known data\n");
        }
        version_gate(&common, options.min_version, "at least", |v, req| v < req);
        version_gate(&common, options.exact_version, "exactly", |v, req| v != req);
        // Upstream tested `<` for --max-version, the same comparison as
        // --atleast-version, so it rejected exactly the builds it was meant to
        // accept.
        version_gate(&common, options.max_version, "at most", |v, req| v > req);
        if let Some(features) = options.required_features.as_ref() {
            for f in features.iter() {
                match FEATURES
                    .iter()
                    .find(|feature| feature.names.contains(&f.as_str()))
                {
                    Some(feature) => {
                        verbose_print(
                            &common,
                            &format!("Requiring {} support from library\n", feature.label),
                        );
                        if !feature.present {
                            error(
                                &common,
                                EXIT_FAILURE,
                                0,
                                &format!("Required {} support not present", feature.label),
                            );
                        }
                    }
                    None => error(
                        &common,
                        EXIT_FAILURE,
                        0,
                        &format!(
                            "Required {} support is unrecognised and therefore assumed to be missing",
                            f
                        ),
                    ),
                }
            }
        }
        verbose_print(
            &common,
            &format!(
                "{}\nHFST packaging: {} {}\nHFST version: {}\nHFST long version: {}\nCompatible with upstream HFST: {} (long version {})\n",
                version_line(&common.program_name),
                PACKAGE_NAME,
                PACKAGE_VERSION,
                PACKAGE_VERSION,
                HFST_LONGVERSION,
                HFST_COMPAT_VERSION,
                HFST_COMPAT_LONGVERSION
            ),
        );
        for feature in FEATURES {
            verbose_print(
                &common,
                &format!(
                    "{} {}\n",
                    feature.label,
                    if feature.present {
                        "supported"
                    } else {
                        "not supported"
                    }
                ),
            );
        }

        Ok(())
    }

    /// One `-a/-e/-m` test: `requirement` is -1 when the option was not given, and
    /// `fails` is the failing comparison for one version against it. The gate
    /// passes if either the fork's own version or the upstream interface-compat
    /// version satisfies it — the two namespaces scripts ask in, and a requirement
    /// met in either one is genuinely met.
    fn version_gate(
        common: &CommonOptions,
        requirement: i64,
        relation: &str,
        fails: impl Fn(i64, i64) -> bool,
    ) {
        if requirement == -1 {
            return;
        }
        verbose_print(
            common,
            &format!(
                "Requiring current version {} (upstream-compatible {}) to be {} {}\n",
                HFST_LONGVERSION, HFST_COMPAT_LONGVERSION, relation, requirement
            ),
        );
        if fails(HFST_LONGVERSION, requirement) && fails(HFST_COMPAT_LONGVERSION, requirement) {
            version_requirements_not_met(common);
        }
    }

    // The refusal names both identities so a build script's log says what was
    // actually asked of what, instead of a bare no it would have to guess at.
    fn version_requirements_not_met(common: &CommonOptions) {
        error(
            common,
            EXIT_FAILURE,
            0,
            &format!(
                "Version requirements not met: this is {} {} (long version {}), \
             interface-compatible with upstream HFST {} (long version {})",
                PACKAGE_NAME,
                PACKAGE_VERSION,
                HFST_LONGVERSION,
                HFST_COMPAT_VERSION,
                HFST_COMPAT_LONGVERSION
            ),
        );
    }
}

pub mod name {
    //! Faithful 1:1 port of tools/src/hfst-name.cc — the transducer naming
    //! command-line tool.
    //!
    //! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
    //! lives in [`CommonOptions`] (the shared `-v/-q/-o/-i/…` fields) and a
    //! tool-local [`Options`], threaded into the processing functions. There are
    //! no `static mut` globals and no `unsafe`.

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{hfst_set_program_name, parse_u64, verbose_print};
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;

    /// hfst-name's resolved options (the former tool-specific `static mut`s).
    #[derive(Default)]
    struct Options {
        /// '-n, --name=NAME': the name to set on the transducer.
        transducer_name: String,
        /// whether '-n / --name' was given.
        name_option_given: bool,
        /// '-p, --print-name': only print the current name.
        print_name: bool,
        /// '-t, --truncate_length=LEN': truncate the name to LEN bytes (0 = no limit).
        truncate_length: u64,
    }

    /// hfst-name's command line.
    //
    // '--truncate_length' keeps its upstream underscore: that is the long
    // name the getopt table carried, and Giella scripts spell it that way.
    // '-p' takes no argument here (unlike hfst-edit-metadata's) and, when
    // given together with '-n', overrides it — a warning the tool body emits
    // after the parameter checks.
    // [spec:hfst:def:hfst-name.parse-options-fn]
    // [spec:hfst:sem:hfst-name.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "Name a transducer")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,

        /// Name the transducer NAME
        #[arg(
            short = 'n',
            long = "name",
            value_name = "NAME",
            allow_hyphen_values = true
        )]
        name: Option<String>,

        /// Only print the current name
        #[arg(short = 'p', long = "print-name")]
        print_name: bool,

        /// Truncate name length to LEN
        #[arg(short = 't', long = "truncate_length", value_name = "LEN")]
        truncate_length: Option<String>,
    }

    impl Args {
        /// Case 't': hfst_strtoul(optarg, 10), fatal on anything else.
        fn truncate_length(&self, common: &CommonOptions) -> u64 {
            match &self.truncate_length {
                Some(len) => parse_u64(common, len, 10),
                None => 0,
            }
        }

        fn options(&self, common: &CommonOptions) -> Options {
            Options {
                transducer_name: self.name.clone().unwrap_or_default(),
                name_option_given: self.name.is_some(),
                print_name: self.print_name,
                truncate_length: self.truncate_length(common),
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
            // The C rejected a non-numeric LEN inside the getopt loop, before
            // the parameter checks; run it here for the same ordering.
            self.truncate_length(opts);
            Ok(())
        }
    }

    // [spec:hfst:def:hfst-name.process-stream-fn]
    // [spec:hfst:sem:hfst-name.process-stream-fn]
    fn process_stream(
        common: &CommonOptions,
        options: &Options,
        instream: &mut HfstInputStream<'_>,
        outstream: &mut HfstOutputStream,
    ) -> i32 {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;

            if transducer_n > 1 && options.print_name {
                eprintln!("---");
            }

            if transducer_n == 1 {
                verbose_print(common, &format!("Naming {}...\n", common.input_filename));
            } else {
                verbose_print(
                    common,
                    &format!("Naming {}...{}\n", common.input_filename, transducer_n),
                );
            }

            let any = match instream.read() {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("hfst-name: {e}");
                    return 1;
                }
            };
            // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
            crate::for_any!(any, trans => {
                let mut trans = trans;
                if !options.print_name {
                    let name = options.transducer_name.clone();
                    if options.truncate_length > 0 {
                        // C: hfst_strndup copies at most TRUNCATE_LENGTH bytes.
                        let n = (options.truncate_length as usize).min(name.len());
                        let truncated = String::from_utf8_lossy(&name.as_bytes()[..n]).into_owned();
                        trans.set_name(&truncated);
                    } else {
                        trans.set_name(&name);
                    }
                    if let Err(e) = outstream.redirect(&mut trans) {
                        eprintln!("hfst-name: {e}");
                        return 1;
                    }
                } else {
                    eprintln!("\"{}\"", trans.get_name());
                }
            });
        }
        instream.close();
        outstream.close();
        0
    }

    // [spec:hfst:def:hfst-name.main-fn]
    // [spec:hfst:sem:hfst-name.main-fn]
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstName");
        let (common, args) = cli::parse::<Args>(common, args)?;
        let options = args.options(&common);

        if !options.print_name && !options.name_option_given {
            eprintln!("Error: hfst-name: use either option --print-name  or --name");
            return Err(1);
        }
        if options.print_name && options.name_option_given {
            eprintln!("Warning: option --print-name overrides option --name");
        }

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
                eprintln!("hfst-name: {e}");
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
                eprintln!("hfst-name: {e}");
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

pub mod split {
    //! Faithful 1:1 port of tools/src/hfst-split.cc — the transducer archive
    //! exploding tool.
    //!
    //! Option handling is clap 4 derive through [`crate::cli`]: the tool's state
    //! lives in [`CommonOptions`] (the shared `-v/-q/-o/-i/…` fields) and a
    //! tool-local [`Options`], threaded into the processing functions. There are
    //! no `static mut` globals and no `unsafe`.

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{error, hfst_set_program_name, print_short_help, verbose_print};
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;

    /// hfst-split's own options (the former tool-specific `static mut`s).
    struct Options {
        /// '-p, --prefix=PRE': prefix used in naming output files.
        prefix: String,
        /// '-e, --extension=EXT': extension used in naming output files.
        extension: String,
    }

    /// hfst-split's command line.
    //
    // '-o' is REFUSED, not ignored: the tool names its own output files from
    // PRE + N + EXT and its option table never carried the output option, so
    // the shared common group's '-o' is rejected in `validate` the way the C's
    // error arm rejected the unknown letter.
    // [spec:hfst:def:hfst-split.parse-options-fn]
    // [spec:hfst:sem:hfst-split.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(
        about = "Extract transducers from archive with systematic file names",
        after_help = "If INFILE is omitted or -, stdin is used.
If PRE is omitted, no prefix is used.
If EXT is omitted, .hfst is used.
-o/--output is not accepted: this tool names its own output files.
The extracted files are named \"PRE\" + N + \"EXT\", where N is the number of the transducer in the archive.

An example:
   cat transducer_a transducer_b | hfst-split -p \"rule\" -e \".tr\"

This command creates files \"rule1.tr\" (equivalent to transducer_a) and \"rule2.tr\" (equivalent to transducer_b)."
    )]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,

        /// Use the prefix PRE in naming output files
        #[arg(
            short = 'p',
            long = "prefix",
            value_name = "PRE",
            allow_hyphen_values = true
        )]
        prefix: Option<String>,

        /// Use the extension EXT in naming output files
        #[arg(
            short = 'e',
            long = "extension",
            value_name = "EXT",
            allow_hyphen_values = true
        )]
        extension: Option<String>,
    }

    impl Args {
        fn options(&self) -> Options {
            Options {
                prefix: self.prefix.clone().unwrap_or_default(),
                extension: self
                    .extension
                    .clone()
                    .unwrap_or_else(|| ".hfst".to_string()),
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
            // hfst-split's option table has no output option; the C's error
            // arm answered '-o' with the unknown-option refusal.
            if self.common.output.is_some() {
                print_short_help(opts);
                error(opts, 1, 0, "Unknown option `-o'.\n");
            }
            // This tool writes its own 'i' case rather than taking the shared
            // one: it opened INFILE eagerly (hfst_fopen) inside the getopt
            // loop, so an unreadable name is refused here, before the
            // parameter checks.
            if let Some(name) = self.io.input.as_deref()
                && name != "-"
                && std::fs::File::open(name).is_err()
            {
                error(opts, 1, 0, &format!("Could not open '{}'. ", name));
            }
            Ok(())
        }
    }

    // [spec:hfst:def:hfst-split.process-stream-fn]
    // [spec:hfst:sem:hfst-split.process-stream-fn]
    fn process_stream(
        common: &mut CommonOptions,
        options: &Options,
        instream: &mut HfstInputStream<'_>,
    ) -> i32 {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let outfilename = format!("{}{}{}", options.prefix, transducer_n, options.extension);
            common.output_filename = outfilename.clone();
            verbose_print(
                common,
                &format!(
                    "Writing {} of {} to {}...\n",
                    transducer_n, common.input_filename, outfilename
                ),
            );
            let mut outstream =
                match HfstOutputStream::new_filename(&outfilename, instream.get_type(), true) {
                    Ok(s) => s,
                    Err(e) => {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                };
            let any = match instream.read() {
                Ok(t) => t,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
            crate::for_any!(any, trans => {
                let mut trans = trans;
                if let Err(e) = outstream.redirect(&mut trans) {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
                if let Err(e) = outstream.flush() {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
                outstream.close();
                common.output_filename = String::new();
            });
        }
        instream.close();
        0
    }

    // [spec:hfst:def:hfst-split.main-fn]
    // [spec:hfst:sem:hfst-split.main-fn]
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstSplit");
        let (mut common, args) = cli::parse::<Args>(common, args)?;
        let options = args.options();

        // close buffers, we use streams
        verbose_print(
            &common,
            &format!(
                "Reading from {}, writing to {}...{}\n",
                common.input_filename, options.prefix, options.extension
            ),
        );
        // here starts the buffer handling part
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced faithfully here.)
        let instream_result = if common.input_filename != "<stdin>" {
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

        cli::from_code(process_stream(&mut common, &options, &mut instream))
    }
}

pub mod strip_header {
    //! Faithful 1:1 port of tools/src/hfst-strip-header.cc — the HFST header
    //! stripping command-line tool. Drives the hfst-cli foundation (globals,
    //! getopt, commandline, program-options, inc fragments).
    //!
    //! Unlike most unary tools, this one does not build HfstInputStream /
    //! HfstOutputStream objects: it opens its input/output as std streams (from the
    //! filename fields, with the "<stdin>"/"<stdout>" sentinels) and delegates the
    //! byte copy + HFST3-header stripping to hfst_input_stream::strip_hfst3_headers.

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{hfst_set_program_name, verbose_print};
    use hfst::hfst_input_stream::strip_hfst3_headers;

    /// hfst-strip-header's command line. The tool declares no options of its own.
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "Remove any HFST3 headers")]
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

    // [spec:hfst:def:hfst-strip-header.process-stream-fn]
    // [spec:hfst:sem:hfst-strip-header.process-stream-fn]
    fn process_stream(common: &CommonOptions) -> i32 {
        // De-C-ified: open the input/output as std streams (resolved from the
        // filename fields by common.input_reader / output_writer, which honour the
        // "<stdin>"/"<stdout>" sentinels) and delegate the HFST3-header stripping to
        // hfst_input_stream::strip_hfst3_headers. The C printed "Stripping..." once
        // per byte under -v; that per-byte trace is dropped (diagnostic only — the
        // stripped output is unchanged).
        let input = match common.input_reader() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("hfst-strip-header: could not open input: {e}");
                return 1;
            }
        };
        let output = match common.output_writer() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("hfst-strip-header: could not open output: {e}");
                return 1;
            }
        };

        match strip_hfst3_headers(input, output) {
            Ok(()) => 0,
            Err(e) => {
                eprintln!("hfst-strip-header: error while stripping headers: {e}");
                1
            }
        }
    }

    // [spec:hfst:def:hfst-strip-header.main-fn]
    // [spec:hfst:sem:hfst-strip-header.main-fn]
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstStripHeader");
        let (common, _args) = cli::parse::<Args>(common, args)?;
        verbose_print(
            &common,
            &format!(
                "Reading from {}, writing to {}\n",
                common.input_filename, common.output_filename
            ),
        );

        cli::from_code(process_stream(&common))
    }
}

pub mod tail {
    //! Faithful 1:1 port of tools/src/hfst-tail.cc — the transducer archive
    //! tailing command-line tool. Option handling is clap 4 derive through
    //! [`crate::cli`].

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{error, hfst_set_program_name, parse_i64, verbose_print};
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;
    use hfst::hfst_transducer::AnyTransducer;
    use std::collections::VecDeque;

    /// hfst-tail's own options (the former tool-specific `static mut`s).
    struct Options {
        /// '-n, --n-last=[+]K': how many trailing transducers to keep.
        tail_count: i64,
    }

    /// hfst-tail's command line.
    // [spec:hfst:def:hfst-tail.parse-options-fn]
    // [spec:hfst:sem:hfst-tail.parse-options-fn]
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(
        about = "Get last transducers from an archive",
        after_help = "K must be an integer, as parsed by strtoul base 10, and not 0.
if K is omitted, it defaults to +1 (all except the first)"
    )]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,

        /// Print the last K transducers; use +K to print transducers starting
        /// from the Kth
        #[arg(
            short = 'n',
            long = "n-last",
            value_name = "[+]K",
            allow_hyphen_values = true
        )]
        n_last: Option<String>,
    }

    impl Args {
        /// Case 'n': a leading '+' negates the parsed count, which is what
        /// selects the skip-the-first-K mode. Without -n the count stays at the
        /// C initialiser of -1, i.e. '+1'.
        fn tail_count(&self, common: &CommonOptions) -> i64 {
            match self.n_last.as_deref() {
                // swap sign haha lol
                Some(k) if k.starts_with('+') => -parse_i64(common, k, 10),
                Some(k) => parse_i64(common, k, 10),
                None => -1,
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
            // The C rejected a non-numeric K inside the getopt loop, before the
            // parameter checks; run it here for the same ordering.
            self.tail_count(opts);
            Ok(())
        }
    }

    // [spec:hfst:def:hfst-tail.process-stream-fn]
    // [spec:hfst:sem:hfst-tail.process-stream-fn]
    fn process_stream(
        common: &CommonOptions,
        options: &Options,
        instream: &mut HfstInputStream<'_>,
        outstream: &mut HfstOutputStream,
    ) -> i32 {
        let mut last_n: VecDeque<AnyTransducer> = VecDeque::new();
        let mut transducer_n: i64 = 0;
        if options.tail_count > 0 {
            verbose_print(
                common,
                &format!("Counting last {} transducers...\n", options.tail_count),
            );
            while instream.is_good() {
                transducer_n += 1;
                let trans = match instream.read() {
                    Ok(t) => t,
                    Err(e) => {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                };
                last_n.push_back(trans);
                if last_n.len() as i64 > options.tail_count {
                    last_n.pop_front();
                }
            }
            if options.tail_count < transducer_n {
                transducer_n -= options.tail_count + 1;
            } else {
                transducer_n = 0;
            }
            while !last_n.is_empty() {
                transducer_n += 1;
                verbose_print(
                    common,
                    &format!("Forwarding {}...{}\n", common.input_filename, transducer_n),
                );
                let mut front = last_n
                    .pop_front()
                    .expect("last_n is non-empty per the enclosing while condition");
                if let Err(e) = front.write(outstream) {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            }
        } else if options.tail_count < 0 {
            verbose_print(
                common,
                &format!("Skipping {} transducers...\n", -options.tail_count),
            );
            while instream.is_good() {
                transducer_n += 1;
                let mut trans = match instream.read() {
                    Ok(t) => t,
                    Err(e) => {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                };
                if transducer_n >= -options.tail_count {
                    verbose_print(
                        common,
                        &format!("Forwarding {}...{}\n", common.input_filename, transducer_n),
                    );
                    if let Err(e) = trans.write(outstream) {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                }
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

    // [spec:hfst:def:hfst-tail.main-fn]
    // [spec:hfst:sem:hfst-tail.main-fn]
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.2", "HfstTail");
        let (common, args) = cli::parse::<Args>(common, args)?;
        let options = Options {
            tail_count: args.tail_count(&common),
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
        let instream_result = if input_opened {
            HfstInputStream::new_filename(&common.input_filename)
        } else {
            HfstInputStream::new()
        };
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)
        let mut instream = match instream_result {
            Ok(s) => s,
            Err(e) => {
                error(&common, 1, 0, &format!("{e}"));
                return Err(1);
            }
        };

        let ty = instream.get_type();
        let outstream_result = if output_opened {
            HfstOutputStream::new_filename(&common.output_filename, ty, true)
        } else {
            HfstOutputStream::new(ty, true)
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

pub mod traverse {
    //! Faithful 1:1 port of tools/src/hfst-traverse.cc — the transducer traversal
    //! tool that walks through a transducer arc by arc. Drives the hfst-cli
    //! foundation (globals, getopt, commandline, program-options, inc fragments).

    use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{error, hfst_readline, hfst_set_program_name, verbose_print};
    use hfst::hfst_basic_transducer::HfstBasicTransducer;
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;
    use std::collections::BTreeMap;
    use std::io::Write;

    /// hfst-traverse's command line.
    // [spec:hfst:req:cli.arg-parse]
    // [spec:hfst:req:cli.help]
    #[derive(clap::Parser)]
    #[command(about = "Walk through the transducer arc by arc")]
    struct Args {
        #[command(flatten)]
        common: CommonArgs,
        #[command(flatten)]
        io: UnaryIo,

        /// Play the Colossal Cave adventure intro on start
        #[arg(short = 'X', long = "cave")]
        cave_mode: bool,
    }

    impl ToolArgs for Args {
        fn common(&self) -> &CommonArgs {
            &self.common
        }

        fn apply_io(&self, opts: &mut CommonOptions) {
            self.io.apply(opts);
        }
    }

    /// hfst-traverse's resolved options (the former tool-specific `static mut`s).
    struct Options {
        /// '-X, --cave': play the Colossal Cave adventure intro on start.
        cave_mode: bool,
    }

    // The C arclabel readline-completion helpers (arclabel_generator /
    // arclabel_completion) are gated behind HAVE_DECL_RL_COMPLETION_MATCHES and the
    // GNU readline library. The Rust 'hfst_readline' uses plain 'getline' with no
    // readline backend, so — exactly as on a build without readline — those #if
    // blocks are not compiled in. Their def/sem annotations are carried below for
    // traceability; the bodies are intentionally left out to match the
    // no-readline configuration the foundation provides.

    // [spec:hfst:def:hfst-traverse.arclabel-generator-fn]
    // [spec:hfst:sem:hfst-traverse.arclabel-generator-fn]
    // (readline-only: not compiled — see note above)

    // [spec:hfst:def:hfst-traverse.arclabel-completion-fn]
    // [spec:hfst:sem:hfst-traverse.arclabel-completion-fn]
    // (readline-only: not compiled — see note above)

    // [spec:hfst:def:hfst-traverse.main-loop-fn]
    // [spec:hfst:sem:hfst-traverse.main-loop-fn]
    fn main_loop(common: &CommonOptions, trans: &HfstBasicTransducer) -> i32 {
        let mut msg = common.message_writer();
        let _ = writeln!(msg, "Enter labels to seek all paths");
        // record current paths with their end states. The C++ uses a
        // multimap<string, HfstState>; a BTreeMap<(String, usize), HfstState>
        // (keyed on an insertion counter to permit duplicate path strings)
        // preserves both the ordered iteration and the multi-value semantics.
        let mut paths: BTreeMap<(String, usize), u32> = BTreeMap::new();
        let mut counter: usize = 0;
        paths.insert((String::new(), counter), 0);
        counter += 1;
        // (The readline completion / history setup is readline-only; omitted as
        // the foundation uses a plain getline-based readline — see note above.)
        loop {
            // print available paths
            for ((path_str, _), state) in paths.iter() {
                let _ = writeln!(msg, "On path `{}' are continuations:", path_str);
                let transitions = match trans.index(*state) {
                    Ok(v) => v,
                    Err(e) => {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                };
                if transitions.is_empty() {
                    let _ = writeln!(msg, "<Nothing, you've hit a dead end here>");
                }
                for arc in transitions.iter() {
                    let _ = writeln!(
                        msg,
                        "{}\t{}",
                        arc.get_input_symbol(trans.coder()),
                        arc.get_output_symbol(trans.coder())
                    );
                }
            }
            let label = match hfst_readline(common, "traverse> ") {
                Some(l) => l,
                None => return 0,
            };
            let mut new_paths: BTreeMap<(String, usize), u32> = BTreeMap::new();
            for ((path_str, _), state) in paths.iter() {
                let transitions = match trans.index(*state) {
                    Ok(v) => v,
                    Err(e) => {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                };
                for arc in transitions.iter() {
                    if arc.get_input_symbol(trans.coder()) == label {
                        let newpath = format!(
                            "{}{}:{} ",
                            path_str,
                            arc.get_input_symbol(trans.coder()),
                            arc.get_output_symbol(trans.coder())
                        );
                        new_paths.insert((newpath, counter), arc.get_target_state());
                        counter += 1;
                    }
                }
            }
            if new_paths.is_empty() {
                if label == "quit" || label.is_empty() {
                    let _ = writeln!(msg, "Use EOF (Ctrl-D or similar) to quit");
                } else if label == "XYZZY" {
                    let _ = writeln!(msg, "Nothing happens");
                }
                let _ = writeln!(msg, "could not advance with {}", label);
            } else {
                paths = new_paths;
            }
            // (add_history is readline-only; omitted — see note above.)
        } // while paths not empty
    }

    // [spec:hfst:def:hfst-traverse.process-stream-fn]
    // [spec:hfst:sem:hfst-traverse.process-stream-fn]
    fn process_stream(
        common: &CommonOptions,
        options: &Options,
        instream: &mut HfstInputStream<'_>,
    ) -> i32 {
        let mut msg = common.message_writer();
        let mut transducer_n: usize = 0;
        // The C++ writes this as `while (instream.is_good())` but its body
        // unconditionally `return`s main_loop() on the first transducer
        // (hfst-traverse.cc:278/325), so it runs exactly once — an `if` here is
        // behaviour-identical and not a never-looping loop.
        if instream.is_good() {
            transducer_n += 1;
            let _ = transducer_n;
            let any = match instream.read() {
                Ok(v) => v,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
            crate::for_any!(any, trans => {
                let mut trans_name = trans.get_name();
                if trans_name.is_empty() {
                    trans_name = common.input_filename.clone();
                }
                // HfstBasicTransducer walkable(trans);
                let walkable = match HfstBasicTransducer::try_from_transducer(&trans) {
                    Ok(v) => v,
                    Err(e) => {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                };
                if options.cave_mode {
                    let _ = write!(
                        msg,
                        "WELCOME TO ADVENTURE!! WOULD YOU LIKE INSTRUCTIONS?\n\n"
                    );
                    let yesno = hfst_readline(common, "").unwrap_or_default();
                    if yesno == "YES" || yesno == "yes" {
                        let _ = write!(
                            msg,
                            "SOMEWHERE NEARBY IS COLOSSAL CAVE \
                             WHERE OTHERS HAVE FOUND\n\
                             FORTUNES IN TREASURES AND GOLD, \
                             THOUGH IT IS RUMORED\n\
                             THAT SOME WHO ENTER ARE NEVER SEEN AGAIN. \
                             MAGIC IS SAID\n\
                             TO WORK IN THE CAVE.  I WILL BE YOUR EYES AND HANDS. \
                             DIRECT\n\
                             ME WITH COMMANDS OF 1 ARC LABEL.\n\
                             (ERRORS, COMPLAINTS, SUGGESTIONS TO HFST-BUGS)\n\
                             (IF STUCK TYPE HELP FOR SOME HINTS)\n\n",
                        );
                    }
                    let _ = write!(
                        msg,
                        "YOU ARE STANDING AT THE END OF A ROAD BEFORE A \
                         SMALL FINITE\n\
                         STATE AUTOMATON . AROUND YOU IS A FOREST. A SMALL\n\
                         STREAM OF ARCS FLOWS OUT OF THE AUTOMATON AND \
                         DOWN A GULLY:\n\n",
                    );
                } else {
                    let _ = write!(msg, "Traversing automaton {}\n\n", trans_name);
                }
                if walkable.state_vector.is_empty() {
                    let _ = writeln!(msg, "Nowhere to go");
                    return 0;
                }
                return main_loop(common, &walkable);
            });
        }
        instream.close();
        0
    }

    // [spec:hfst:def:hfst-traverse.main-fn]
    // [spec:hfst:sem:hfst-traverse.main-fn]
    pub fn run(args: Vec<String>) -> i32 {
        cli::exit_code(execute(args))
    }

    fn execute(args: Vec<String>) -> ToolResult {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstDeterminize");
        let (common, args) = cli::parse::<Args>(common, args)?;
        let options = Options {
            cave_mode: args.cave_mode,
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
                return Err(1);
            }
        };
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)

        // The C constructs an HfstOutputStream from the input type even though
        // this tool never writes to it (traversal only reads). Mirror that
        // construction so the buffer-handling part matches the source.
        let ty = instream.get_type();
        let _outstream = match if output_opened {
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

        cli::from_code(process_stream(&common, &options, &mut instream))
    }
}
