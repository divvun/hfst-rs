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
    //! command-line tool. Drives the hfst-cli foundation (globals, getopt,
    //! commandline, program-options, inc fragments).

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{extend_options_from_env, hfst_set_program_name, verbose_print};
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
        print_common_unary_program_options, print_common_unary_program_parameter_instructions,
    };
    use crate::inc::{
        CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
        handle_unary_case,
    };
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

    /// hfst-dump-alphabets's own options (the former tool-specific `static mut`s).
    struct Options {
        output_format: AlphaDumpFormat,
        print_seen: bool,
        print_meta: bool,
        only_multichars: bool,
    }

    impl Default for Options {
        fn default() -> Options {
            Options {
                output_format: AlphaDumpFormat::Tsv,
                print_seen: true,
                print_meta: true,
                only_multichars: false,
            }
        }
    }

    // [spec:hfst:def:hfst-dump-alphabets.is-multichar-fn]
    // [spec:hfst:sem:hfst-dump-alphabets.is-multichar-fn]
    fn is_multichar(s: &str) -> bool {
        if s.len() > 2 {
            return s.starts_with('+') || s.starts_with(' ') || s.starts_with('@');
        }
        false
    }

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nPrint alphabets of automaton\n\n",
            common.program_name
        );

        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        // fprintf(message_out, (tool-specific options and short descriptions)
        let _ = writeln!(msg, "Alphabet dump options:");
        let _ = writeln!(msg, "  -f, --format=AFORMAT     Print alphabet in AFORAMT");
        let _ = writeln!(
            msg,
            "  -1, --exclude-seen       Ignore alphabets seen in automaton"
        );
        let _ = writeln!(
            msg,
            "  -2, --exclude-metadata   Ignore alphabets from headers"
        );
        let _ = writeln!(msg);
        print_common_unary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(msg);
    }

    // [spec:hfst:def:hfst-dump-alphabets.parse-options-fn]
    // [spec:hfst:sem:hfst-dump-alphabets.parse-options-fn]
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
                name: "format",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: 'f' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "include-seen",
                has_arg: getopt::NO_ARGUMENT,
                val: '1' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "include-metadata",
                has_arg: getopt::NO_ARGUMENT,
                val: '2' as i32,
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
            match c as u8 as char {
                'f' => {
                    let optarg = opt.optarg();
                    if optarg == "tsv" {
                        options.output_format = AlphaDumpFormat::Tsv;
                        options.only_multichars = false;
                        verbose_print(&common, "printing one symbol per line\n");
                    } else if optarg == "vislcg3-list" {
                        options.output_format = AlphaDumpFormat::Vislcg3List;
                        options.only_multichars = true;
                        verbose_print(&common, "printing LIST x = x ; for VISL CG 3...\n");
                    } else if optarg == "vislcg3-tags" {
                        options.output_format = AlphaDumpFormat::Vislcg3Tags;
                        options.only_multichars = true;
                        verbose_print(&common, "printing STRICT-TAGS += for VISL CG 3...\n");
                    } else {
                        eprintln!("Error: unrecognised format {}", optarg);
                        std::process::exit(1);
                    }
                    continue;
                }
                '1' => {
                    options.print_seen = false;
                    continue;
                }
                '2' => {
                    options.print_meta = false;
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

    // [spec:hfst:def:hfst-dump-alphabets.process-stream-fn]
    // [spec:hfst:sem:hfst-dump-alphabets.process-stream-fn]
    fn process_stream(
        common: &CommonOptions,
        options: &Options,
        instream: &mut HfstInputStream<'_>,
    ) -> i32 {
        // Data output goes to a std stream (the std counterpart of the libc
        // outfile FILE*); `emit` writes a string and ignores errors, matching the
        // old fput/fputs. (print_usage's message_out path stays on FILE* until
        // the message_out chunk of io-foundation.)
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
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstSummarize");
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
                return 1;
            }
        };
        let _retval = process_stream(&common, &options, &mut instream);

        0
    }
}

pub mod edit_metadata {
    //! Faithful 1:1 port of tools/src/hfst-edit-metadata.cc — the transducer
    //! metadata tool. Drives the hfst-cli foundation (getopt, commandline,
    //! program-options, inc fragments).
    //!
    //! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
    //! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
    //! `parse_options` and threaded into the processing functions. There are no
    //! `static mut` globals and no `unsafe`.

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        error, extend_options_from_env, hfst_set_program_name, parse_u64, verbose_print, warning,
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
    use std::collections::BTreeMap;
    use std::io::Write;

    /// hfst-edit-metadata's own options (the former tool-specific `static mut`s).
    struct Options {
        /// '-a, --add=ANAME=VALUE': the properties to add or replace.
        properties: BTreeMap<String, String>,
        /// whether any '-a' property was given.
        properties_given: bool,
        /// whether all properties should be printed (the default).
        print_all_properties: bool,
        /// '-p, --print[=NAME]': the specific property to print. C used a NULL
        /// char* as "no specific property requested"; modelled as Option.
        print_property: Option<String>,
        /// '-t, --truncate_length=LEN': truncate added property lengths to LEN.
        truncate_length: u64,
    }

    impl Default for Options {
        fn default() -> Options {
            Options {
                properties: BTreeMap::new(),
                properties_given: false,
                print_all_properties: true,
                print_property: None,
                truncate_length: 0,
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
            "Usage: {} [OPTIONS...] [INFILE]\nName a transducer\n\n",
            common.program_name
        );
        let _ = write!(
            msg,
            "Name options:\n\
         \x20 -a, --add=ANAME=VALUE       add or replace property ANAMEwith VALUE\n\
         \x20 -p, --print[=NAME]          print the current PNAME\n\
         \x20 -t, --truncate_length=LEN   truncate added properties' lengths to LEN\n"
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        let _ = writeln!(msg);
        print_common_unary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(msg, "If PNAME is omitted, all values are printed");
        let _ = writeln!(msg);
    }

    // [spec:hfst:def:hfst-edit-metadata.parse-options-fn]
    // [spec:hfst:sem:hfst-edit-metadata.parse-options-fn]
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
                name: "add",
                has_arg: 1, // required_argument
                val: 'a' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "print-name",
                has_arg: 2, // optional_argument
                val: 'p' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "truncate_length",
                has_arg: 1, // required_argument
                val: 't' as i32,
            });
            // add tool-specific options here
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, unary cases, the error arm, then the tool's own cases.
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
            // tool-specific cases
            let ch = c as u8;
            if ch == b'a' {
                let optstr = opt.optarg();
                match optstr.find('=') {
                    None => {
                        error(
                            &common,
                            1,
                            0,
                            &format!("Equals sign `=' missing from {}", optstr),
                        );
                    }
                    Some(idx) => {
                        let property = optstr[..idx].to_string();
                        let value = optstr[idx + 1..].to_string();
                        options.properties.insert(property, value);
                        options.properties_given = true;
                        options.print_all_properties = false;
                    }
                }
                continue;
            } else if ch == b'p' {
                match opt.optarg_opt() {
                    Some(arg) => options.print_property = Some(arg),
                    None => options.print_all_properties = true,
                }
                continue;
            } else if ch == b't' {
                options.truncate_length = parse_u64(&common, &opt.optarg(), 10);
                continue;
            }

            return Err(handle_error_case(&common, &opt, c));
        }

        check_common_params(&mut common);
        check_unary_params(&mut common, &opt, args);
        Ok((common, options))
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
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstEditMetadata");
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

        process_stream(&common, &options, &mut instream, &mut outstream)
    }
}

pub mod head {
    //! Faithful 1:1 port of tools/src/hfst-head.cc — the transducer archive head
    //! splitting tool. Drives the hfst-cli foundation (globals, getopt,
    //! commandline, program-options, tool-metadata, inc fragments).
    //!
    //! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
    //! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
    //! `parse_options` and threaded into the processing functions. There are no
    //! `static mut` globals and no `unsafe`.

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        error, extend_options_from_env, hfst_set_program_name, parse_i64, verbose_print, warning,
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
    use hfst::hfst_transducer::AnyTransducer;
    use std::collections::VecDeque;
    use std::io::Write;

    /// hfst-head's own options (the former tool-specific `static mut`s).
    struct Options {
        /// '-n, --n-first=[-]K': number of transducers to keep from the head.
        head_count: i64,
    }

    impl Default for Options {
        fn default() -> Self {
            Options { head_count: 1 }
        }
    }

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        // Usage line
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nGet first transducers from an archive\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        let _ = write!(
            msg,
            "Archive options:\n  -n, --n-first=[-]K   print the first K transducers;\n                       with the leading `-', print all but last K transducers\n"
        );
        let _ = writeln!(msg);
        print_common_unary_program_parameter_instructions(&mut *msg);
        let _ = write!(
            msg,
            "K must be an integer, as parsed by strtoul base 10, and not 0.\nIf K is omitted default is 1."
        );
        let _ = writeln!(msg);
    }

    // [spec:hfst:def:hfst-head.parse-options-fn]
    // [spec:hfst:sem:hfst-head.parse-options-fn]
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
            long_options.push(getopt::GetOpt {
                name: "n-first",
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
                options.head_count = parse_i64(&common, &opt.optarg(), 10);
                continue;
            }
            return Err(handle_error_case(&common, &opt, c));
        }

        check_common_params(&mut common);
        check_unary_params(&mut common, &opt, args);
        if options.head_count == 0 {
            warning(&common, 0, 0, "Argument 0 for count is not sensible");
        }
        Ok((common, options))
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
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.2", "HfstHead");
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
                return 1;
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
                return 1;
            }
        };

        process_stream(&common, &options, &mut instream, &mut outstream)
    }
}

pub mod info {
    //! Port of tools/src/hfst-info.cc — the "show or test HFST versions and
    //! features" command-line tool. It reads no transducer streams; it parses
    //! version/feature test options, then prints or tests the build's version and
    //! features. Drives the hfst-cli foundation (globals, getopt, commandline,
    //! program-options).
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
    //! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
    //! shared `-v/-q/-o/…` fields) and a tool-local [`Options`] — both built by
    //! `parse_options` and threaded into `run`. There are no `static mut` globals
    //! and no `unsafe`.

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        error, extend_options_from_env, hfst_set_program_name, print_version, verbose_print,
        version_line,
    };
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{hfst_getopt_common_long, print_common_program_options};

    const EXIT_SUCCESS: i32 = 0;
    const EXIT_FAILURE: i32 = 1;
    use std::collections::BTreeSet;
    use std::io::Write;

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

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nshow or test HFST versions and features\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        let _ = write!(
            msg,
            "Test features:\n  -a, --atleast-version=MVER   require at least MVER version of HFST\n  -e, --exact-version=EVER     require exactly EVER version of HFST\n  -m, --max-version=UVER       require at most UVER version of HFST\n  -f, --requirefeature=FEAT    require named FEAT support from HFST\n"
        );
        let _ = writeln!(msg);
        let _ = write!(
            msg,
            "MVER, EVER or UVER version vectors must be composed of one to three full stop separated runs of digits.\nA requirement is met if either this build's own version or the upstream HFST version\nit is interface-compatible with ({HFST_COMPAT_VERSION}) meets it.\nFEAT should be name of feature supported by HFST, such as openfst, foma or icu\n\n"
        );
    }

    // [spec:hfst:def:hfst-info.parse-options-fn]
    // [spec:hfst:sem:hfst-info.parse-options-fn]
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
            long_options.push(getopt::GetOpt {
                name: "atleast-version",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: b'a' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "exact-version",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: b'e' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "max-version",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: b'm' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "require-feature",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: b'f' as i32,
            });
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }
            // The C switch handles only a/e/m/f/h/V; every other accepted option
            // (the common -v/-q/-s/-d/-o/--colour) falls through with no action.
            if c == b'a' as i32 {
                options.min_version = parse_version_string(&common, &opt.optarg());
            } else if c == b'e' as i32 {
                options.exact_version = parse_version_string(&common, &opt.optarg());
            } else if c == b'm' as i32 {
                options.max_version = parse_version_string(&common, &opt.optarg());
            } else if c == b'f' as i32 {
                options
                    .required_features
                    .get_or_insert_with(BTreeSet::new)
                    .insert(opt.optarg());
            } else if c == b'h' as i32 {
                print_usage(&common);
                return Err(EXIT_SUCCESS);
            } else if c == b'V' as i32 {
                print_version(&common);
                return Err(EXIT_SUCCESS);
            }
        }
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
        Ok((common, options))
    }

    // [spec:hfst:def:hfst-info.main-fn]
    // [spec:hfst:sem:hfst-info.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstInfo");
        let (common, options) = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };
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

        EXIT_SUCCESS
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
    //! command-line tool. Drives the hfst-cli foundation (globals, getopt,
    //! commandline, program-options, tool-metadata, inc fragments).
    //!
    //! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
    //! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
    //! `parse_options` and threaded into the processing functions. There are no
    //! `static mut` globals and no `unsafe`.

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        extend_options_from_env, hfst_set_program_name, parse_u64, verbose_print,
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

    /// hfst-name's own options (the former tool-specific `static mut`s).
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

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        // Usage line
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nName a transducer\n\n",
            common.program_name
        );
        let _ = write!(
            msg,
            "Name options:\n  -n, --name=NAME      Name the transducer NAME\n  -p, --print-name     Only print the current name\n  -t, --truncate_length=LEN   Truncate name length to LEN\n"
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        let _ = writeln!(msg);
        print_common_unary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(msg);
    }

    // [spec:hfst:def:hfst-name.parse-options-fn]
    // [spec:hfst:sem:hfst-name.parse-options-fn]
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
                name: "name",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: b'n' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "print-name",
                has_arg: getopt::NO_ARGUMENT,
                val: b'p' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "truncate_length",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: b't' as i32,
            });
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the terminal error arm, then the
            // tool's own cases.
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
            // tool-specific cases come before the error arm in the C switch
            // ordering (getopt-cases-error.h precedes them textually but its
            // arms only fire on '?'/ ':' / default, so the named cases below
            // are reached for 'n'/'p'/'t').
            let byte = c as u8;
            match byte {
                b'n' => {
                    options.transducer_name = opt.optarg();
                    options.name_option_given = true;
                    continue;
                }
                b'p' => {
                    options.print_name = true;
                    continue;
                }
                b't' => {
                    options.truncate_length = parse_u64(&common, &opt.optarg(), 10);
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
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstName");
        let (common, options) = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };

        if !options.print_name && !options.name_option_given {
            eprintln!("Error: hfst-name: use either option --print-name  or --name");
            return 1;
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
                eprintln!("hfst-name: {e}");
                return 1;
            }
        };

        process_stream(&common, &options, &mut instream, &mut outstream)
    }
}

pub mod split {
    //! Faithful 1:1 port of tools/src/hfst-split.cc — the transducer archive
    //! exploding tool. Drives the hfst-cli foundation (globals, getopt,
    //! commandline, program-options, tool-metadata, inc fragments).
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
    use crate::hfst_program_options::{hfst_getopt_common_long, print_common_program_options};
    use crate::inc::{
        CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    };
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;
    use std::io::Write;

    /// hfst-split's own options (the former tool-specific `static mut`s).
    struct Options {
        /// '-p, --prefix=PRE': prefix used in naming output files.
        prefix: String,
        /// '-e, --extension=EXT': extension used in naming output files.
        extension: String,
    }

    impl Default for Options {
        fn default() -> Options {
            Options {
                prefix: String::new(),
                extension: ".hfst".to_string(),
            }
        }
    }

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nExtract transducers from archive with systematic file names\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        let _ = write!(
            msg,
            "Input/Output options:\n  -i, --input=INFILE    Read input transducer from INFILE\n  -p, --prefix=PRE      Use the prefix PRE in naming output files\n  -e, --extension=EXT   Use the extension EXT in naming output files\n"
        );
        let _ = writeln!(msg);
        let _ = write!(
            msg,
            "If INFILE is omitted or -, stdin is used.\nIf PRE is omitted, no prefix is used.\nIf EXT is omitted, .hfst is used.\nThe extracted files are named \"PRE\" + N + \"EXT\",\nwhere N is the number of the transducer in the archive.\n\nAn example:\n   cat transducer_a transducer_b | hfst-split -p \"rule\" -e \".tr\"\n\nThis command creates files \"rule1.tr\" (equivalent to transducer_a)\nand \"rule2.tr\" (equivalent to transducer_b). \n"
        );
        let _ = writeln!(msg);
    }

    // [spec:hfst:def:hfst-split.parse-options-fn]
    // [spec:hfst:sem:hfst-split.parse-options-fn]
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
            // add tool-specific options here
            long_options.push(getopt::GetOpt {
                name: "input",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: b'i' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "prefix",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: b'p' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "extension",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: b'e' as i32,
            });
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd common case group, then this
            // tool's own input/output cases, then the terminal error arm.
            match handle_common_case(&mut common, &opt, c, print_usage) {
                CaseResult::Return(code) => return Err(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match c {
                c if c == b'i' as i32 => {
                    common.input_filename = opt.optarg();
                    // C: inputfile = hfst_fopen(inputfilename, "r"); if it resolves
                    // to stdin ("-"), reset the name to "<stdin>". Otherwise the C
                    // opened the file eagerly to validate it; mirror that by trying
                    // to open it and erroring through the same path on failure.
                    if common.input_filename == "-" {
                        common.input_filename = "<stdin>".to_string();
                    } else if std::fs::File::open(&common.input_filename).is_err() {
                        error(
                            &common,
                            1,
                            0,
                            &format!("Could not open '{}'. ", common.input_filename),
                        );
                    }
                    common.input_named = true;
                    continue;
                }
                c if c == b'p' as i32 => {
                    options.prefix = opt.optarg();
                    continue;
                }
                c if c == b'e' as i32 => {
                    options.extension = opt.optarg();
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
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstSplit");
        let (mut common, options) = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };

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
                return 1;
            }
        };

        process_stream(&mut common, &options, &mut instream)
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

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{extend_options_from_env, hfst_set_program_name, verbose_print};
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
        print_common_unary_program_options, print_common_unary_program_parameter_instructions,
    };
    use crate::inc::{
        CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
        handle_unary_case,
    };
    use hfst::hfst_input_stream::strip_hfst3_headers;
    use std::io::Write;

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nRemove any HFST3 headers\n\n",
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
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstStripHeader");
        let common = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };
        verbose_print(
            &common,
            &format!(
                "Reading from {}, writing to {}\n",
                common.input_filename, common.output_filename
            ),
        );

        process_stream(&common)
    }
}

pub mod tail {
    //! Faithful 1:1 port of tools/src/hfst-tail.cc — the transducer archive
    //! tailing command-line tool. Drives the hfst-cli foundation (globals,
    //! getopt, commandline, program-options, tool-metadata, inc fragments).

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        error, extend_options_from_env, hfst_set_program_name, parse_i64, verbose_print,
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
    use hfst::hfst_transducer::AnyTransducer;
    use std::collections::VecDeque;
    use std::io::Write;

    /// hfst-tail's own options (the former tool-specific `static mut`s).
    struct Options {
        /// '-n, --n-last=[+]K': how many trailing transducers to keep.
        tail_count: i64,
    }

    impl Default for Options {
        fn default() -> Self {
            Options { tail_count: -1 }
        }
    }

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nGet last transducers from an archive\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        let _ = write!(
            msg,
            "Archive options:\n  -n, --n-last=[+]K   Print the last K transducers;\n                      use +K to print transducers starting from the Kth\n",
        );
        let _ = writeln!(msg);
        print_common_unary_program_parameter_instructions(&mut *msg);
        let _ = write!(
            msg,
            "K must be an integer, as parsed by strtoul base 10, and not 0.\nif K is omitted, it defaults to +1 (all except the first)\n",
        );
        let _ = writeln!(msg);
    }

    // [spec:hfst:def:hfst-tail.parse-options-fn]
    // [spec:hfst:sem:hfst-tail.parse-options-fn]
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
                name: "n-last",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: 'n' as i32,
            });
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
                let optarg = opt.optarg();
                if optarg.starts_with('+') {
                    // swap sign haha lol
                    options.tail_count = -parse_i64(&common, &optarg, 10);
                } else {
                    options.tail_count = parse_i64(&common, &optarg, 10);
                }
                continue;
            }
            return Err(handle_error_case(&common, &opt, c));
        }

        check_common_params(&mut common);
        check_unary_params(&mut common, &opt, args);
        Ok((common, options))
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
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.2", "HfstTail");
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
                return 1;
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
                return 1;
            }
        };

        process_stream(&common, &options, &mut instream, &mut outstream)
    }
}

pub mod traverse {
    //! Faithful 1:1 port of tools/src/hfst-traverse.cc — the transducer traversal
    //! tool that walks through a transducer arc by arc. Drives the hfst-cli
    //! foundation (globals, getopt, commandline, program-options, inc fragments).

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        error, extend_options_from_env, hfst_readline, hfst_set_program_name, verbose_print,
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
    use hfst::hfst_basic_transducer::HfstBasicTransducer;
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;
    use std::collections::BTreeMap;
    use std::io::Write;

    /// hfst-traverse's own options (the former tool-specific `static mut`s).
    #[derive(Default)]
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

    // [spec:hfst:req:cli.help]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        // Usage line
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE]\nWalk through the transducer arc by arc\n\n",
            common.program_name
        );

        // options, grouped
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
                name: "cave",
                has_arg: getopt::NO_ARGUMENT,
                val: 'X' as i32,
            });
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own 'X', then the
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
            if c == 'X' as i32 {
                options.cave_mode = true;
                continue;
            }
            return Err(handle_error_case(&common, &opt, c));
        }

        check_common_params(&mut common);
        check_unary_params(&mut common, &opt, args);
        Ok((common, options))
    }

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
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstDeterminize");
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
                return 1;
            }
        };

        process_stream(&common, &options, &mut instream)
    }
}
