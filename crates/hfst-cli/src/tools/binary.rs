//! Two-input-stream tools: the binary_ops family (one operation applied
//! pairwise across two archives) and its close relatives.
//!
//! Contains, as inline modules:
//! - `binary_tool`
//! - `check_alpha`
//! - `compare`
//! - `compose`
//! - `concatenate`
//! - `conjunct`
//! - `disjunct`
//! - `priority_disjunct`
//! - `shuffle`
//! - `subtract`

pub mod binary_tool {
    //! Faithful 1:1 port of tools/src/hfst-binary-tool.cc — the GENERIC BINARY
    //! TOOL TEMPLATE command-line tool. Drives the hfst-cli foundation (globals,
    //! getopt, commandline, program-options, tool-metadata, inc fragments).

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        extend_options_from_env, hfst_set_program_name, verbose_print, warning,
    };
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_binary_long, hfst_getopt_common_long, print_common_binary_program_options,
        print_common_binary_program_parameter_instructions, print_common_program_options,
    };
    use crate::inc::{
        CaseResult, check_binary_params, check_common_params, handle_binary_case,
        handle_common_case, handle_error_case,
    };
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_output_stream::HfstOutputStream;
    use hfst::hfst_transducer::HfstTransducer;
    use std::io::Write;

    /// hfst-binary-tool's own options (the former tool-specific `static mut`s).
    /// The skeleton tool has none.
    struct Options;

    // [spec:hfst:def:hfst-binary-tool.print-usage-fn]
    // [spec:hfst:sem:hfst-binary-tool.print-usage-fn]
    fn print_usage(common: &CommonOptions) {
        let mut msg = common.message_writer();
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nDo things with two transducers\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_binary_program_options(&mut *msg);
        let _ = writeln!(msg);
        print_common_binary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(msg);
        let _ = write!(
            msg,
            "\nExamples:\n  {} -o catdog.hfst cat.hfst dog.hfst  does things\n\n",
            common.program_name
        );
    }

    // [spec:hfst:def:hfst-binary-tool.parse-options-fn]
    // [spec:hfst:sem:hfst-binary-tool.parse-options-fn]
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
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_binary_long());
            // add tool-specific options here
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then binary cases, then the tool's own (none here), then
            // the terminal error arm.
            match handle_common_case(&mut common, &opt, c, print_usage) {
                CaseResult::Return(code) => return Err(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_binary_case(&mut common, &opt, c) {
                CaseResult::Return(code) => return Err(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            return Err(handle_error_case(&common, &opt, c));
        }

        check_common_params(&mut common);
        check_binary_params(&mut common, &opt, args);
        Ok((common, options))
    }

    // [spec:hfst:def:hfst-binary-tool.binaryoperate-streams-fn]
    // [spec:hfst:sem:hfst-binary-tool.binaryoperate-streams-fn]
    fn binaryoperate_streams(
        common: &CommonOptions,
        firststream: &mut HfstInputStream<'_>,
        secondstream: &mut HfstInputStream<'_>,
        outstream: &mut HfstOutputStream,
    ) -> i32 {
        // (the C opens each stream here; the Rust streams are opened by their
        // constructors, so the explicit open() calls are not reproduced.)
        // should be is_good?
        let mut both_inputs = firststream.is_good() && secondstream.is_good();
        if firststream.get_type() != secondstream.get_type() {
            warning(
                common,
                0,
                0,
                &format!(
                    "Tranducer type mismatch in {} and {}; using former type as output\n",
                    common.first_filename, common.second_filename
                ),
            );
        }
        let mut transducer_n: usize = 0;
        while both_inputs {
            transducer_n += 1;
            if transducer_n == 1 {
                verbose_print(
                    common,
                    &format!(
                        "Doing things with {} and {}...\n",
                        common.first_filename, common.second_filename
                    ),
                );
            } else {
                verbose_print(
                    common,
                    &format!(
                        "Doing things with {} and {}... {}\n",
                        common.first_filename, common.second_filename, transducer_n
                    ),
                );
            }
            let first = match firststream.read() {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("hfst-binary-tool: {e}");
                    return 1;
                }
            };
            let second = match secondstream.read() {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("hfst-binary-tool: {e}");
                    return 1;
                }
            };
            // one dispatch per pair ([dec:hfst:monomorphic-backends]); the
            // C++ concatenate threw TransducerTypeMismatch for mixed operands
            // at runtime, which is now the boundary's mismatch arm.
            use hfst::hfst_transducer::AnyTransducer;
            let code = match (first, second) {
                (AnyTransducer::Tropical(f), AnyTransducer::Tropical(s)) => {
                    concatenate_pair(f, s, outstream)
                }
                _ => {
                    eprintln!("hfst-binary-tool: {}", hfst::err!(TransducerTypeMismatch));
                    return 1;
                }
            };
            if code != 0 {
                return code;
            }
            both_inputs = firststream.is_good() && secondstream.is_good();
        }

        if firststream.is_good() {
            warning(
                common,
                0,
                0,
                &format!(
                    "Warning: {} contains more transducers than {}; residue skipped\n",
                    common.first_filename, common.second_filename
                ),
            );
        } else if secondstream.is_good() {
            warning(
                common,
                0,
                0,
                &format!(
                    "Warning: {} contains fewer transducers than {}; residue skipped\n",
                    common.first_filename, common.second_filename
                ),
            );
        }
        firststream.close();
        secondstream.close();
        outstream.close();
        0
    }

    // The monomorphic pair body of the skeleton tool.
    fn concatenate_pair<B: hfst::backend::AlgebraBackend>(
        mut first: HfstTransducer<B>,
        second: HfstTransducer<B>,
        outstream: &mut HfstOutputStream,
    ) -> i32 {
        if let Err(e) = first.concatenate(&second, true) {
            eprintln!("hfst-binary-tool: {e}");
            return 1;
        }
        if let Err(e) = outstream.redirect(&mut first) {
            eprintln!("hfst-binary-tool: {e}");
            return 1;
        }
        0
    }

    // [spec:hfst:def:hfst-binary-tool.main-fn]
    // [spec:hfst:sem:hfst-binary-tool.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstGenericBinaryTool");
        let (common, _options) = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };

        // close buffers, we use streams
        let first_opened = common.first_filename != "<stdin>";
        let second_opened = common.second_filename != "<stdin>";
        let output_opened = common.output_filename != "<stdout>";
        verbose_print(
            &common,
            &format!(
                "Reading from {} and {}, writing to {}\n",
                common.first_filename, common.second_filename, common.output_filename
            ),
        );
        // here starts the buffer handling part
        // (the C wraps each ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch
        // arms are not reproduced here.)
        let firststream_res = if first_opened {
            HfstInputStream::new_filename(&common.first_filename)
        } else {
            HfstInputStream::new()
        };
        let mut firststream = match firststream_res {
            Ok(s) => s,
            Err(e) => {
                eprintln!("hfst-binary-tool: {e}");
                return 1;
            }
        };
        let secondstream_res = if second_opened {
            HfstInputStream::new_filename(&common.second_filename)
        } else {
            HfstInputStream::new()
        };
        let mut secondstream = match secondstream_res {
            Ok(s) => s,
            Err(e) => {
                eprintln!("hfst-binary-tool: {e}");
                return 1;
            }
        };
        let ty = firststream.get_type();
        let outstream_res = if output_opened {
            HfstOutputStream::new_filename(&common.output_filename, ty, true)
        } else {
            HfstOutputStream::new(ty, true)
        };
        let mut outstream = match outstream_res {
            Ok(s) => s,
            Err(e) => {
                eprintln!("hfst-binary-tool: {e}");
                return 1;
            }
        };

        // (the C main calls concatenate_streams; the defined function is
        // binaryoperate_streams — the same routine — which is invoked here.)
        binaryoperate_streams(&common, &mut firststream, &mut secondstream, &mut outstream)
    }
}

pub mod check_alpha {
    //! Faithful 1:1 port of tools/src/hfst-check-alpha.cc — the tool that compares
    //! the compatibility of alphabets within and between automata. Drives the
    //! hfst-cli foundation (getopt, commandline, program-options, tool-metadata,
    //! inc fragments). A binary tool (two input streams).
    //!
    //! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
    //! shared `-v/-q/-1/-2/…` fields) built by `parse_options` and threaded into
    //! the processing functions. There are no `static mut` globals and no `unsafe`.

    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        error, extend_options_from_env, hfst_set_program_name, verbose_print,
    };
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_binary_long, hfst_getopt_common_long, print_common_binary_program_options,
        print_common_binary_program_parameter_instructions, print_common_program_options,
    };
    use crate::inc::{
        CaseResult, check_binary_params, check_common_params, handle_binary_case,
        handle_common_case, handle_error_case,
    };
    use hfst::hfst_basic_transducer::HfstBasicTransducer;
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_symbol_defs::StringSet;

    use std::io::Write;

    // [spec:hfst:def:hfst-check-alpha.print-usage-fn]
    // [spec:hfst:sem:hfst-check-alpha.print-usage-fn]
    fn print_usage(common: &CommonOptions) {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = common.message_writer();
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILEs]\nCompare the compatibility of alphabets between INFILEs\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_binary_program_options(&mut *msg);
        // (tool-specific options and short descriptions)
        let _ = writeln!(msg, "Check alpha options:");
        let _ = writeln!(msg);
        print_common_binary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(msg);
    }

    // [spec:hfst:def:hfst-check-alpha.fprint-stringset-fn]
    // [spec:hfst:sem:hfst-check-alpha.fprint-stringset-fn]
    fn fprint_stringset(outfile: &mut dyn Write, strings: &StringSet) {
        let mut first = true;
        for s in strings {
            if !first {
                let _ = write!(outfile, ", ");
            }
            let _ = write!(outfile, "{}", s);
            first = false;
        }
    }

    // [spec:hfst:def:hfst-check-alpha.parse-options-fn]
    // [spec:hfst:sem:hfst-check-alpha.parse-options-fn]
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
            long_options.extend(hfst_getopt_binary_long());
            // add tool-specific options here
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: binary
            // cases, then common cases, then the tool's own (none here), then the
            // terminal error arm.
            match handle_binary_case(&mut common, &opt, c) {
                CaseResult::Return(code) => return Err(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_common_case(&mut common, &opt, c, print_usage) {
                CaseResult::Return(code) => return Err(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            return Err(handle_error_case(&common, &opt, c));
        }

        check_binary_params(&mut common, &opt, args);
        check_common_params(&mut common);
        Ok(common)
    }

    // [spec:hfst:def:hfst-check-alpha.process-stream-fn]
    // [spec:hfst:sem:hfst-check-alpha.process-stream-fn]
    fn process_stream(
        common: &CommonOptions,
        firststream: &mut HfstInputStream<'_>,
        secondstream: &mut HfstInputStream<'_>,
    ) -> i32 {
        let mut out = match common.output_writer() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("hfst-check-alpha: cannot open output: {e}");
                return 1;
            }
        };
        let mut continue_reading = firststream.is_good() && secondstream.is_good();
        let mut transducer_n: usize = 0;
        let mut mismatch = 0;
        while continue_reading {
            transducer_n += 1;

            if transducer_n < 2 {
                verbose_print(common, "Checking alphas...\n");
            } else {
                verbose_print(common, &format!("Checking alphas... {}\n", transducer_n));
            }
            // read first alphas
            let first = match firststream.read() {
                Ok(t) => t,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            // one dispatch per read ([dec:hfst:monomorphic-backends]); the
            // alphabet queries are backend-independent values.
            let (mutt, first_transducer_alphabet): (HfstBasicTransducer, StringSet) = crate::for_any!(&first, t => {
                let mutt = match HfstBasicTransducer::try_from_transducer(t) {
                    Ok(m) => m,
                    Err(e) => {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                };
                let alpha = match t.get_alphabet() {
                    Ok(a) => a,
                    Err(e) => {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                };
                (mutt, alpha)
            });
            let transducer_knows_alphabet = true;
            let first_found_alphabet: StringSet = mutt.symbols_used();
            // read second alphas
            let second = match secondstream.read() {
                Ok(t) => t,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            let (secondmutt, second_transducer_alphabet): (HfstBasicTransducer, StringSet) = crate::for_any!(&second, t => {
                let mutt = match HfstBasicTransducer::try_from_transducer(t) {
                    Ok(m) => m,
                    Err(e) => {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                };
                let alpha = match t.get_alphabet() {
                    Ok(a) => a,
                    Err(e) => {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                };
                (mutt, alpha)
            });
            let second_found_alphabet: StringSet = secondmutt.symbols_used();
            // match
            let _ = writeln!(out, "Actual alphabet differences:");
            let first_minus_second: StringSet = first_found_alphabet
                .difference(&second_found_alphabet)
                .cloned()
                .collect();
            if !first_minus_second.is_empty() {
                mismatch = 1;
                let _ = write!(
                    out,
                    "In first {} but not in second {}:",
                    first.get_name(),
                    second.get_name()
                );
                fprint_stringset(&mut *out, &first_minus_second);
            } else {
                let _ = write!(
                    out,
                    "First {} alpha is superset of second {}.",
                    first.get_name(),
                    second.get_name()
                );
            }
            let _ = writeln!(out);
            let second_minus_first: StringSet = second_found_alphabet
                .difference(&first_found_alphabet)
                .cloned()
                .collect();
            if !second_minus_first.is_empty() {
                mismatch = 1;
                let _ = write!(
                    out,
                    "In second {} but not in first {}:",
                    second.get_name(),
                    second.get_name()
                );
                fprint_stringset(&mut *out, &second_minus_first);
            } else {
                let _ = write!(
                    out,
                    "Second {} alpha is superset of second {}.",
                    second.get_name(),
                    second.get_name()
                );
            }
            let _ = writeln!(out);
            if common.verbose {
                let _ = write!(out, "{} alphabet:", first.get_name());
                fprint_stringset(&mut *out, &first_found_alphabet);
                let _ = writeln!(out);
                let _ = write!(out, "{} alphabet:", second.get_name());
                fprint_stringset(&mut *out, &second_found_alphabet);
                let _ = writeln!(out);
            }
            if transducer_knows_alphabet {
                let _ = writeln!(out, "sigma set difference:");
                let first_minus_second: StringSet = first_transducer_alphabet
                    .difference(&second_transducer_alphabet)
                    .cloned()
                    .collect();
                let second_minus_first: StringSet = second_transducer_alphabet
                    .difference(&first_transducer_alphabet)
                    .cloned()
                    .collect();
                if !first_minus_second.is_empty() {
                    mismatch = 1;
                    let _ = write!(
                        out,
                        "First {} has but second {} does not: ",
                        first.get_name(),
                        second.get_name()
                    );
                    fprint_stringset(&mut *out, &first_minus_second);
                } else {
                    let _ = write!(
                        out,
                        "First {} alpha is superset of second {}.",
                        first.get_name(),
                        second.get_name()
                    );
                }
                let _ = writeln!(out);
                if !second_minus_first.is_empty() {
                    mismatch = 1;
                    let _ = write!(
                        out,
                        "Second {} has but first {} does not: ",
                        second.get_name(),
                        first.get_name()
                    );
                    fprint_stringset(&mut *out, &second_minus_first);
                } else {
                    let _ = write!(
                        out,
                        "Second {} alpha is superset of first {}.",
                        second.get_name(),
                        first.get_name()
                    );
                }
                let _ = writeln!(out);
                if common.verbose {
                    let _ = write!(out, "First ({}):", first.get_name());
                    fprint_stringset(&mut *out, &first_transducer_alphabet);
                    let _ = writeln!(out);
                    let _ = write!(out, "Second ({}):", second.get_name());
                    fprint_stringset(&mut *out, &second_transducer_alphabet);
                    let _ = writeln!(out);
                }
            } else {
                let _ = writeln!(out, "No internal alphabets to compare in this format");
            } // FSTs know their alphas
            continue_reading = firststream.is_good() && secondstream.is_good();
        }

        let _ = write!(out, "\nRead {} transducers in total.\n", transducer_n);
        mismatch
    }

    // [spec:hfst:def:hfst-check-alpha.main-fn]
    // [spec:hfst:sem:hfst-check-alpha.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstALphaFix");
        let common = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };

        // close buffers, we use streams
        let first_opened = common.first_filename != "<stdin>";
        let second_opened = common.second_filename != "<stdin>";
        verbose_print(
            &common,
            &format!(
                "Reading from {} and {}, writing to {}\n",
                common.first_filename, common.second_filename, common.output_filename
            ),
        );
        // here starts the buffer handling part
        // (the C wraps each ctor in try/catch on HfstException, calling error()
        // and returning EXIT_FAILURE; the Rust ctors now return a Result, so the
        // error path and message are preserved via a match on that Result.)
        let firststream = if first_opened {
            let name = common.first_filename.clone();
            match HfstInputStream::new_filename(&name) {
                Ok(s) => s,
                Err(_) => {
                    error(
                        &common,
                        1,
                        0,
                        &format!("{} is not a valid transducer file", name),
                    );
                    return 1;
                }
            }
        } else {
            match HfstInputStream::new() {
                Ok(s) => s,
                Err(_) => {
                    error(
                        &common,
                        1,
                        0,
                        &format!("{} is not a valid transducer file", common.first_filename),
                    );
                    return 1;
                }
            }
        };
        let secondstream = if second_opened {
            let name = common.second_filename.clone();
            match HfstInputStream::new_filename(&name) {
                Ok(s) => s,
                Err(_) => {
                    error(
                        &common,
                        1,
                        0,
                        &format!("{} is not a valid transducer file", name),
                    );
                    return 1;
                }
            }
        } else {
            match HfstInputStream::new() {
                Ok(s) => s,
                Err(_) => {
                    error(
                        &common,
                        1,
                        0,
                        &format!("{} is not a valid transducer file", common.second_filename),
                    );
                    return 1;
                }
            }
        };
        let mut firststream = firststream;
        let mut secondstream = secondstream;

        let _retval = process_stream(&common, &mut firststream, &mut secondstream);

        0
    }
}

pub mod compare {
    //! Faithful 1:1 port of tools/src/hfst-compare.cc — the transducer comparison
    //! command-line tool. Drives the hfst-cli foundation (getopt, commandline,
    //! program-options, inc fragments). A binary tool: it reads from two input
    //! streams (first + second) and writes a comparison log.
    //!
    //! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
    //! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
    //! `parse_options` and threaded into the processing functions. There are no
    //! `static mut` globals and no `unsafe`.

    use crate::binary_ops::open_two_input_streams;
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{
        error, extend_options_from_env, hfst_set_program_name, hfst_strformat,
        is_input_stream_in_ol_format, verbose_print,
    };
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_binary_long, hfst_getopt_common_long, print_common_binary_program_options,
        print_common_binary_program_parameter_instructions, print_common_program_options,
    };
    use crate::inc::{
        CaseResult, check_binary_params, check_common_params, handle_binary_case,
        handle_common_case, handle_error_case,
    };
    use hfst::hfst_input_stream::HfstInputStream;
    use hfst::hfst_transducer::{AnyTransducer, HfstTransducer};
    use std::io::Write;

    /// hfst-compare's own options (the former tool-specific `static mut`s: C's
    /// 'static bool harmonize=true; static bool eliminate_flags=false;').
    struct Options {
        /// '-H, --do-not-harmonize': harmonize symbols before comparing.
        harmonize: bool,
        /// '-e, --eliminate-flags': eliminate flag diacritics before comparing.
        eliminate_flags: bool,
    }

    impl Default for Options {
        fn default() -> Self {
            Options {
                harmonize: true,
                eliminate_flags: false,
            }
        }
    }

    // [spec:hfst:def:hfst-compare.print-usage-fn]
    // [spec:hfst:sem:hfst-compare.print-usage-fn]
    fn print_usage(common: &CommonOptions) {
        let mut msg = common.message_writer();
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nCompare two transducers\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_binary_program_options(&mut *msg);
        let _ = write!(
            msg,
            "Harmonization:\n  -H, --do-not-harmonize Do not harmonize symbols.\n  -e, --eliminate-flags  Eliminate flag diacritics.\n"
        );
        let _ = writeln!(msg);
        print_common_binary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(msg);
        let _ = write!(
            msg,
            "\nExamples:\n  $ {0} cat.hfst dog.hfst\n  cat.hfst[1] != dog.hfst[1]\n  $ {0} cat.hfst cat.hfst\n  cat.hfst[1] == cat.hfst[1]\n\n",
            common.program_name
        );
    }

    // [spec:hfst:def:hfst-compare.parse-options-fn]
    // [spec:hfst:sem:hfst-compare.parse-options-fn]
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
            long_options.extend(hfst_getopt_binary_long());
            // add tool-specific options here
            long_options.push(getopt::GetOpt {
                name: "do-not-harmonize",
                has_arg: getopt::NO_ARGUMENT,
                val: 'H' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "eliminate-flags",
                has_arg: getopt::NO_ARGUMENT,
                val: 'e' as i32,
            });
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then binary cases, then the tool's own ('H'/'e'), then the
            // terminal error arm.
            match handle_common_case(&mut common, &opt, c, print_usage) {
                CaseResult::Return(code) => return Err(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_binary_case(&mut common, &opt, c) {
                CaseResult::Return(code) => return Err(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match c as u8 as char {
                'H' => {
                    options.harmonize = false;
                    continue;
                }
                'e' => {
                    options.eliminate_flags = true;
                    continue;
                }
                _ => {}
            }
            return Err(handle_error_case(&common, &opt, c));
        }

        check_common_params(&mut common);
        check_binary_params(&mut common, &opt, args);
        Ok((common, options))
    }

    // [spec:hfst:def:hfst-compare.compare-streams-fn]
    // [spec:hfst:sem:hfst-compare.compare-streams-fn]
    fn compare_streams(
        common: &CommonOptions,
        options: &Options,
        firststream: &mut HfstInputStream<'_>,
        secondstream: &mut HfstInputStream<'_>,
    ) -> i32 {
        let mut out = match common.output_writer() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("hfst-compare: cannot open output: {e}");
                return 1;
            }
        };
        let mut continue_reading = firststream.is_good() && secondstream.is_good();
        let mut transducer_n_first: usize = 0; // transducers read from first input
        let mut transducer_n_second: usize = 0; // transducers read from second input
        let mut mismatches: usize = 0;

        let mut second: Option<AnyTransducer> = None;

        while continue_reading {
            let mut first = match firststream.read() {
                Ok(v) => v,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            transducer_n_first += 1;
            if secondstream.is_good() {
                second = Some(match secondstream.read() {
                    Ok(v) => v,
                    Err(e) => {
                        error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                });
                transducer_n_second += 1;
            }
            let mut firstname = first.get_name();
            // make scan-build happy, this should not happen
            let second_ref = match second.as_mut() {
                Some(s) => s,
                None => panic!("Error: second stream has a NULL value."),
            };
            let mut secondname = second_ref.get_name();
            if firstname.is_empty() {
                firstname = common.first_filename.clone();
            }
            if secondname.is_empty() {
                secondname = common.second_filename.clone();
            }
            if transducer_n_first == 1 {
                verbose_print(
                    common,
                    &format!("Comparing {} and {}...\n", firstname, secondname),
                );
            } else {
                verbose_print(
                    common,
                    &format!(
                        "Comparing {} and {}... {}\n",
                        firstname, secondname, transducer_n_first
                    ),
                );
            }
            // C: try { ... } catch (TransducerTypeMismatchException). Same-
            // backend operands are a compile-time property of the generic
            // body now, so the mismatch is this boundary's fall-through arm
            // ([dec:hfst:monomorphic-backends]).
            let outcome = match (&mut first, second_ref) {
                (AnyTransducer::Tropical(f), AnyTransducer::Tropical(s)) => {
                    Some(compare_pair(common, options, f, s))
                }
                #[cfg(feature = "foma")]
                (AnyTransducer::Foma(f), AnyTransducer::Foma(s)) => {
                    Some(compare_pair(common, options, f, s))
                }
                _ => None,
            };
            match outcome {
                Some(Ok(equal)) => {
                    if equal {
                        if transducer_n_first == 1 {
                            if !common.silent {
                                let _ = writeln!(out, "{} == {}", firstname, secondname);
                            }
                        } else if !common.silent {
                            let _ = writeln!(
                                out,
                                "{}[{}] == {}[{}]",
                                firstname, transducer_n_first, secondname, transducer_n_second
                            );
                        }
                    } else {
                        if transducer_n_first == 1 {
                            if !common.silent {
                                let _ = writeln!(out, "{} != {}", firstname, secondname);
                            }
                        } else if !common.silent {
                            let _ = writeln!(
                                out,
                                "{}[{}] != {}[{}]",
                                firstname, transducer_n_first, secondname, transducer_n_second
                            );
                        }
                        mismatches += 1;
                    }
                }
                Some(Err(e)) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
                None => {
                    // cannot recover yet, but beautify error messages
                    error(
                        common,
                        2,
                        0,
                        &format!(
                            "Cannot compare `{}' and `{}' [{}]\nthe formats {} and {} are not compatible for comparison",
                            firstname,
                            secondname,
                            transducer_n_first,
                            hfst_strformat(firststream.get_type()),
                            hfst_strformat(secondstream.get_type())
                        ),
                    );
                }
            }

            continue_reading =
                firststream.is_good() && (secondstream.is_good() || transducer_n_second == 1);

            // delete the transducer of second stream, unless we continue reading
            // the first stream and there is only one transducer in the second
            // stream
            if secondstream.is_good() || !continue_reading {
                second = None;
            }
        }

        if firststream.is_good() {
            error(
                common,
                1,
                0,
                &format!(
                    "second input '{}' contains fewer transducers than first input '{}'; this is only possible if the second input contains exactly one transducer",
                    common.second_filename, common.first_filename
                ),
            );
        } else if secondstream.is_good() {
            error(
                common,
                1,
                0,
                &format!(
                    "first input '{}' contains fewer transducers than second input '{}'",
                    common.first_filename, common.second_filename
                ),
            );
        }
        firststream.close();
        secondstream.close();
        let _ = out.flush();
        if mismatches == 0 {
            verbose_print(
                common,
                &format!("All {} transducers matched\n", transducer_n_first),
            );
            0
        } else {
            verbose_print(
                common,
                &format!("{}/{} were not equal\n", mismatches, transducer_n_first),
            );
            1
        }
    }

    // The monomorphic per-pair comparison body (flag elimination + compare).
    fn compare_pair<B: hfst::backend::AlgebraBackend>(
        common: &CommonOptions,
        options: &Options,
        first: &mut HfstTransducer<B>,
        second: &mut HfstTransducer<B>,
    ) -> hfst::error::Result<bool> {
        if options.eliminate_flags {
            verbose_print(common, "Eliminating flags...\n");
            first.eliminate_flags()?;
            second.eliminate_flags()?;
        }
        first.compare(second, options.harmonize)
    }

    // [spec:hfst:def:hfst-compare.main-fn]
    // [spec:hfst:sem:hfst-compare.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstCompare");
        let (common, options) = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };

        // close buffers, we use streams
        verbose_print(
            &common,
            &format!(
                "Reading from {} and {}, writing log to {}\n",
                common.first_filename, common.second_filename, common.output_filename
            ),
        );
        let (mut firststream, mut secondstream) = match open_two_input_streams(&common) {
            Ok(v) => v,
            Err(code) => return code,
        };

        if is_input_stream_in_ol_format(&firststream, "hfst-compare")
            || is_input_stream_in_ol_format(&secondstream, "hfst-compare")
        {
            return 1;
        }

        compare_streams(&common, &options, &mut firststream, &mut secondstream)
    }
}

pub mod compose {
    //! Faithful 1:1 port of tools/src/hfst-compose.cc — the transducer composition
    //! command-line tool. Drives the hfst-cli foundation (globals, getopt,
    //! commandline, program-options, tool-metadata, inc fragments). A binary tool:
    //! it reads two input streams (firstfile + secondfile) and composes them; the
    //! shared scaffolding lives in crate::binary_ops.

    use crate::binary_ops::{
        BinaryOpSpec, BinaryToolOp, LoopStyle, PairContext, RetryPolicy, run_binary_streams_tool,
    };
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{error, extend_options_from_env, hfst_set_program_name, warning};
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_binary_long, hfst_getopt_common_long, print_common_binary_program_options,
        print_common_binary_program_parameter_instructions, print_common_program_options,
    };
    use crate::inc::{
        CaseResult, check_binary_params, check_common_params, handle_binary_case,
        handle_common_case, handle_error_case,
    };
    use crate::memory_limit::{self, LimitSource, ResolvedMemoryLimit};
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_data_types::ImplementationType;
    use hfst::hfst_transducer::{EngineConfig, FlagDiacriticComposeOverlay, HfstTransducer};
    use std::io::Write;

    const GETOPT_MEMORY_LIMIT: i32 = 0x100;

    /// hfst-compose's own options (the former tool-specific `static mut`s).
    struct Options {
        /// '-F, --harmonize-flags': harmonize flag diacritics.
        harmonize_flags: bool,
        /// '-H, --do-not-harmonize': off harmonizes symbols (default on).
        harmonize: bool,
        /// '--xfst flag-is-epsilon' (was the 'flag_is_epsilon_in_composition'
        /// file-static global in the library; now threaded into compose via
        /// EngineConfig).
        flag_is_epsilon: bool,
        /// '--xerox-composition' (was the 'xerox_composition' file-static global in
        /// the library; now threaded into compose via EngineConfig).
        xerox_composition: bool,
        /// '--memory-limit=SIZE': allowance for budget-aware compose working data.
        memory_limit_bytes: Option<u64>,
    }

    impl Default for Options {
        fn default() -> Self {
            Options {
                harmonize_flags: false,
                harmonize: true,
                flag_is_epsilon: false,
                xerox_composition: false,
                memory_limit_bytes: None,
            }
        }
    }

    // [spec:hfst:def:hfst-compose.print-usage-fn]
    // [spec:hfst:sem:hfst-compose.print-usage-fn]
    fn print_usage(common: &CommonOptions) {
        let mut msg = common.message_writer();
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nCompose two transducers\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_binary_program_options(&mut *msg);
        let _ = write!(
            msg,
            "Composition options:\n  -x, --xerox-composition=VALUE Whether flag diacritics are treated as ordinary\n                                symbols in composition (default is false).\n  -X, --xfst=VARIABLE    Toggle xfst compatibility option VARIABLE.\n      --memory-limit=SIZE\n                         Working-memory allowance for budget-aware OpenFst tropical\n                         and Foma compose state (default: 50% of available RAM;\n                         excess spills).\nHarmonization:\n  -H, --do-not-harmonize Do not harmonize symbols.\n  -F, --harmonize-flags  Harmonize flag diacritics.\n"
        );
        let _ = writeln!(msg);
        print_common_binary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(msg);
        let _ = writeln!(msg, "Xfst variables are {{flag-is-epsilon (default OFF)}}.");
        let _ = writeln!(
            msg,
            "VALUE can be one of the following: [true|false], [yes|no] or [ON|OFF],"
        );
        let _ = writeln!(msg, "false being the default.");
        let _ = writeln!(
            msg,
            "SIZE is an integer byte count with an optional binary K/KB/KiB through T/TB/TiB suffix; 0 forces nonempty budget-aware products to spill."
        );
        let _ = writeln!(
            msg,
            "The allowance is not an RSS ceiling: loaded operands and the final result are not included."
        );
        let _ = writeln!(
            msg,
            "HFST_COMPOSE_MEMORY_LIMIT supplies SIZE when --memory-limit is absent."
        );
        let _ = write!(
            msg,
            "\nExamples:\n  {} -o cat2dog.hfst cat2mouse.hfst mouse2dog.hfst  composes two automata\n\n",
            common.program_name
        );
    }

    // [spec:hfst:def:hfst-compose.parse-options-fn]
    // [spec:hfst:sem:hfst-compose.parse-options-fn]
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
            long_options.extend(hfst_getopt_binary_long());
            // add tool-specific options here
            long_options.push(getopt::GetOpt {
                name: "harmonize-flags",
                has_arg: 0,
                val: b'F' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "do-not-harmonize",
                has_arg: 0,
                val: b'H' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "xerox-composition",
                has_arg: 1,
                val: b'x' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "xfst",
                has_arg: 1,
                val: b'X' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "memory-limit",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: GETOPT_MEMORY_LIMIT,
            });
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: binary
            // cases, then common cases, then the tool's own, then the terminal
            // error arm.
            match handle_binary_case(&mut common, &opt, c) {
                CaseResult::Return(code) => return Err(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_common_case(&mut common, &opt, c, print_usage) {
                CaseResult::Return(code) => return Err(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            if c == b'F' as i32 {
                options.harmonize_flags = true;
                continue;
            } else if c == b'H' as i32 {
                options.harmonize = false;
                continue;
            } else if c == b'x' as i32 {
                let argument = opt.optarg();
                if argument == "yes" || argument == "true" || argument == "ON" {
                    options.xerox_composition = true;
                } else if argument == "no" || argument == "false" || argument == "OFF" {
                    options.xerox_composition = false;
                } else {
                    let _ = writeln!(
                        std::io::stderr(),
                        "Error: unknown option to --xerox-composition: '{}'",
                        opt.optarg()
                    );
                    return Err(1);
                }
                continue;
            } else if c == b'X' as i32 {
                let argument = opt.optarg();
                if argument == "flag-is-epsilon" {
                    options.flag_is_epsilon = true;
                } else {
                    let _ = writeln!(
                        std::io::stderr(),
                        "Error: unknown option to --xfst: '{}'",
                        opt.optarg()
                    );
                    return Err(1);
                }
                continue;
            } else if c == GETOPT_MEMORY_LIMIT {
                let argument = opt.optarg();
                options.memory_limit_bytes = match memory_limit::parse_size(&argument) {
                    Ok(bytes) => Some(bytes),
                    Err(detail) => {
                        let _ = writeln!(
                            std::io::stderr(),
                            "{}: invalid value for --memory-limit: {detail}",
                            common.program_name
                        );
                        return Err(1);
                    }
                };
                continue;
            }
            return Err(handle_error_case(&common, &opt, c));
        }

        check_binary_params(&mut common, &opt, args);
        check_common_params(&mut common);
        Ok((common, options))
    }

    // [spec:hfst:def:hfst-compose.compose-streams-fn]
    // [spec:hfst:sem:hfst-compose.compose-streams-fn]
    // The streams loop lives in crate::binary_ops::run_binary_streams_tool;
    // this descriptor plus the pre-apply (harmonize-flags gate with its own
    // convert-and-retry) and apply closures in run carry the tool's
    // behaviour contract.
    const SPEC: BinaryOpSpec = BinaryOpSpec {
        tool_name: "hfst-compose",
        mismatch_noun: "composition",
        could_not_verb: "compose",
        could_not_noun: "composition",
        name_op: "compose",
        formula: "\u{2218}",
        verbose_begin: |firstname, secondname| {
            format!("Composing {} and {}", firstname, secondname)
        },
        loop_style: LoopStyle::Compose,
        retry: RetryPolicy::TypeMismatchOnly,
        flush_each_round: false,
        flush_at_end: true,
    };

    // [spec:hfst:def:hfst-compose.main-fn]
    // [spec:hfst:sem:hfst-compose.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstCompose");
        let (common, options) = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };

        // Resolve the allowance before either input stream is opened, so the
        // automatic 50% value is a stable startup snapshot rather than a moving
        // target as transducers are loaded.
        let memory_limit = match memory_limit::resolve(options.memory_limit_bytes) {
            Ok(limit) => limit,
            Err(detail) => {
                let _ = writeln!(std::io::stderr(), "{}: {detail}", common.program_name);
                return 1;
            }
        };
        let mut op = ComposeOp {
            harmonize: options.harmonize,
            harmonize_flags: options.harmonize_flags,
            flag_overlay: None,
            memory_limit,
            memory_policy_reported: false,
            cfg: EngineConfig {
                flag_is_epsilon_in_composition: options.flag_is_epsilon,
                xerox_composition: options.xerox_composition,
                compose_memory_limit_bytes: Some(memory_limit.allowance_bytes),
                ..EngineConfig::default()
            },
        };
        run_binary_streams_tool(&common, &SPEC, &mut op)
    }

    struct ComposeOp {
        harmonize: bool,
        harmonize_flags: bool,
        flag_overlay: Option<FlagDiacriticComposeOverlay>,
        memory_limit: ResolvedMemoryLimit,
        memory_policy_reported: bool,
        cfg: EngineConfig,
    }

    fn supports_compose_memory_limit(implementation: ImplementationType) -> bool {
        implementation == ImplementationType::TROPICAL_OPENFST_TYPE
            || implementation == ImplementationType::FOMA_TYPE
    }

    fn explicit_memory_limit_name(source: LimitSource) -> Option<&'static str> {
        match source {
            LimitSource::Cli => Some("--memory-limit"),
            LimitSource::Environment => Some("HFST_COMPOSE_MEMORY_LIMIT"),
            LimitSource::Automatic | LimitSource::ProbeFallback => None,
        }
    }

    impl ComposeOp {
        fn validate_and_report_memory_policy(
            &mut self,
            common: &CommonOptions,
            implementation: ImplementationType,
        ) -> Result<(), i32> {
            if !supports_compose_memory_limit(implementation) {
                if let Some(name) = explicit_memory_limit_name(self.memory_limit.source) {
                    error(
                        common,
                        1,
                        0,
                        &format!(
                            "{name} is not supported for {implementation:?} composition; bounded spilling is available for OpenFst tropical and Foma composition"
                        ),
                    );
                    return Err(1);
                }
                return Ok(());
            }

            if self.memory_policy_reported {
                return Ok(());
            }
            self.memory_policy_reported = true;
            if common.silent {
                return Ok(());
            }

            if self.memory_limit.source == LimitSource::ProbeFallback {
                warning(
                    common,
                    0,
                    0,
                    "Could not determine available RAM; using a 0-byte composition memory allowance and spilling immediately. Use --memory-limit to override.",
                );
            }
            if self.memory_limit.cgroup_clamped
                && let Some(requested) = self.memory_limit.requested_bytes
            {
                warning(
                    common,
                    0,
                    0,
                    &format!(
                        "Requested composition memory allowance of {requested} bytes exceeds current cgroup headroom; using {} bytes.",
                        self.memory_limit.allowance_bytes
                    ),
                );
            }
            Ok(())
        }
    }

    impl BinaryToolOp for ComposeOp {
        // The harmonize-flags gate. (The C's catch-TransducerTypeMismatch,
        // convert-and-retry arm is gone: operands share a backend by construction
        // at this point — the driver converted at the stream boundary.)
        fn pre_apply<B: AlgebraBackend>(
            &mut self,
            common: &CommonOptions,
            first: &mut HfstTransducer<B>,
            second: &mut HfstTransducer<B>,
            _ctx: &PairContext<'_>,
        ) -> Result<(), i32> {
            self.flag_overlay = None;
            self.validate_and_report_memory_policy(common, <B as hfst::backend::Backend>::TYPE)?;
            let has_flags = first.has_flag_diacritics() || second.has_flag_diacritics();
            if has_flags {
                if !self.harmonize_flags {
                    if !common.silent {
                        warning(
                            common,
                            0,
                            0,
                            "At least one of the arguments contains flag diacritics. Use -F to harmonize them.",
                        );
                    }
                } else {
                    let prepared = if B::SUPPORTS_FLAG_OVERLAY {
                        first.prepare_flag_diacritics_for_compose(second).map(Some)
                    } else {
                        first.harmonize_flag_diacritics(second, true).map(|()| None)
                    };
                    match prepared {
                        Ok(overlay) => self.flag_overlay = overlay,
                        Err(e) => {
                            error(common, 1, 0, &format!("{e}"));
                            return Err(1);
                        }
                    }
                }
            }
            Ok(())
        }

        fn apply<B: AlgebraBackend>(
            &mut self,
            first: &mut HfstTransducer<B>,
            second: &HfstTransducer<B>,
        ) -> hfst::error::Result<()> {
            first
                .compose_with_config_and_flag_overlay(
                    second,
                    self.harmonize,
                    &self.cfg,
                    self.flag_overlay.as_ref(),
                )
                .map(|_| ())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn compose_memory_limit_backend_scope_includes_foma() {
            assert!(supports_compose_memory_limit(
                ImplementationType::TROPICAL_OPENFST_TYPE
            ));
            assert!(supports_compose_memory_limit(ImplementationType::FOMA_TYPE));
        }
    }
}

pub mod concatenate {
    //! Faithful 1:1 port of tools/src/hfst-concatenate.cc — the transducer
    //! concatenation command-line tool. Drives the hfst-cli foundation (getopt,
    //! commandline, program-options, tool-metadata, inc fragments).
    //!
    //! This is a BINARY tool: it reads two input streams (firststream and
    //! secondstream) and writes their pairwise concatenation; the shared
    //! scaffolding lives in crate::binary_ops.
    //!
    //! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
    //! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
    //! `parse_options` and threaded into the processing functions. There are no
    //! `static mut` globals and no `unsafe`.

    use crate::binary_ops::{
        BinaryOpSpec, BinaryToolOp, LoopStyle, PairContext, RetryPolicy, run_binary_streams_tool,
    };
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{error, extend_options_from_env, hfst_set_program_name, warning};
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_binary_long, hfst_getopt_common_long, print_common_binary_program_options,
        print_common_binary_program_parameter_instructions, print_common_program_options,
    };
    use crate::inc::{
        CaseResult, check_binary_params, check_common_params, handle_binary_case,
        handle_common_case, handle_error_case,
    };
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_transducer::HfstTransducer;
    use std::io::Write;

    /// hfst-concatenate's own options (the former tool-specific `static mut`s).
    struct Options {
        /// '-F, --harmonize-flags': harmonize flag diacritics.
        harmonize_flags: bool,
        /// '-H, --do-not-harmonize': whether to harmonize symbols (default true).
        harmonize: bool,
    }

    impl Default for Options {
        fn default() -> Self {
            Options {
                harmonize_flags: false,
                harmonize: true,
            }
        }
    }

    // [spec:hfst:def:hfst-concatenate.print-usage-fn]
    // [spec:hfst:sem:hfst-concatenate.print-usage-fn]
    fn print_usage(common: &CommonOptions) {
        let mut msg = common.message_writer();
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nConcatenate two transducers\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_binary_program_options(&mut *msg);
        let _ = write!(
            msg,
            "Harmonization:\n  -H, --do-not-harmonize Do not harmonize symbols.\n  -F, --harmonize-flags  Harmonize flag diacritics.\n"
        );
        let _ = writeln!(msg);
        print_common_binary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(msg);
        let _ = write!(
            msg,
            "\nExamples:\n  {} -o catdog.hfst cat.hfst dog.hfst\nconcatenates cat.hfst with dog.hfst and writes results to catdog.hfst\n\n",
            common.program_name
        );
    }

    // [spec:hfst:def:hfst-concatenate.parse-options-fn]
    // [spec:hfst:sem:hfst-concatenate.parse-options-fn]
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
            long_options.extend(hfst_getopt_binary_long());
            // add tool-specific options here
            long_options.push(getopt::GetOpt {
                name: "harmonize-flags",
                has_arg: 0,
                val: b'F' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "do-not-harmonize",
                has_arg: 0,
                val: b'H' as i32,
            });
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: binary
            // cases, then common cases, then the tool's own ('F'/'H'), then the
            // terminal error arm.
            match handle_binary_case(&mut common, &opt, c) {
                CaseResult::Return(code) => return Err(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_common_case(&mut common, &opt, c, print_usage) {
                CaseResult::Return(code) => return Err(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            if c == b'F' as i32 {
                options.harmonize_flags = true;
                continue;
            }
            if c == b'H' as i32 {
                options.harmonize = false;
                continue;
            }
            return Err(handle_error_case(&common, &opt, c));
        }

        check_binary_params(&mut common, &opt, args);
        check_common_params(&mut common);
        Ok((common, options))
    }

    // [spec:hfst:def:hfst-concatenate.concatenate-streams-fn]
    // [spec:hfst:sem:hfst-concatenate.concatenate-streams-fn]
    // The streams loop lives in crate::binary_ops::run_binary_streams_tool;
    // this descriptor plus the pre-apply/apply closures in run carry the
    // tool's behaviour contract.
    const SPEC: BinaryOpSpec = BinaryOpSpec {
        tool_name: "hfst-concatenate",
        mismatch_noun: "concatenation",
        could_not_verb: "concatenate",
        could_not_noun: "concatenation",
        name_op: "concatenate",
        formula: "\u{22c5}",
        verbose_begin: |firstname, secondname| {
            format!("Concatenating {} and {}", firstname, secondname)
        },
        loop_style: LoopStyle::Standard,
        retry: RetryPolicy::TypeMismatchOnly,
        flush_each_round: false,
        flush_at_end: true,
    };

    // [spec:hfst:def:hfst-concatenate.main-fn]
    // [spec:hfst:sem:hfst-concatenate.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstConcatenate");
        let (common, options) = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };

        let mut op = ConcatenateOp {
            harmonize: options.harmonize,
            harmonize_flags: options.harmonize_flags,
        };
        run_binary_streams_tool(&common, &SPEC, &mut op)
    }

    struct ConcatenateOp {
        harmonize: bool,
        harmonize_flags: bool,
    }

    impl BinaryToolOp for ConcatenateOp {
        fn pre_apply<B: AlgebraBackend>(
            &mut self,
            common: &CommonOptions,
            first: &mut HfstTransducer<B>,
            second: &mut HfstTransducer<B>,
            _ctx: &PairContext<'_>,
        ) -> Result<(), i32> {
            let both_have_flags = first.has_flag_diacritics() && second.has_flag_diacritics();
            if both_have_flags {
                if !self.harmonize_flags {
                    if !common.silent {
                        warning(
                            common,
                            0,
                            0,
                            "The arguments contain flag diacritics. Use -F to harmonize them.",
                        );
                    }
                } else if let Err(e) = first.harmonize_flag_diacritics(second, false) {
                    error(common, 1, 0, &format!("{e}"));
                    return Err(1);
                }
            }
            Ok(())
        }

        fn apply<B: AlgebraBackend>(
            &mut self,
            first: &mut HfstTransducer<B>,
            second: &HfstTransducer<B>,
        ) -> hfst::error::Result<()> {
            first.concatenate(second, self.harmonize).map(|_| ())
        }
    }
}

pub mod conjunct {
    //! Faithful 1:1 port of tools/src/hfst-conjunct.cc — the transducer
    //! conjunction (intersect, AND) command-line tool. Drives the hfst-cli
    //! foundation (globals, getopt, commandline, program-options, tool-metadata,
    //! inc fragments). A BINARY tool: it reads two input streams (first + second);
    //! the shared scaffolding lives in crate::binary_ops.

    use crate::binary_ops::{
        BinaryOpSpec, BinaryToolOp, LoopStyle, PairContext, RetryPolicy, run_binary_streams_tool,
    };
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{error, extend_options_from_env, hfst_set_program_name, warning};
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_binary_long, hfst_getopt_common_long, print_common_binary_program_options,
        print_common_binary_program_parameter_instructions, print_common_program_options,
    };
    use crate::inc::{
        CaseResult, check_binary_params, check_common_params, handle_binary_case,
        handle_common_case, handle_error_case,
    };
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_transducer::{FlagDiacriticOverlay, HfstTransducer};
    use std::io::Write;

    /// hfst-conjunct's own options (the former tool-specific `static mut`s).
    struct Options {
        /// '-F, --harmonize-flags': harmonize flag diacritics.
        harmonize_flags: bool,
        /// '-H, --do-not-harmonize': whether to harmonize (default true).
        harmonize: bool,
    }

    impl Default for Options {
        fn default() -> Self {
            Options {
                harmonize_flags: false,
                harmonize: true,
            }
        }
    }

    // [spec:hfst:def:hfst-conjunct.print-usage-fn]
    // [spec:hfst:sem:hfst-conjunct.print-usage-fn]
    fn print_usage(common: &CommonOptions) {
        let mut msg = common.message_writer();
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nConjunct (intersect, AND) two transducers\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_binary_program_options(&mut *msg);
        let _ = write!(
            msg,
            "Flag diacritics:\n  -F, --harmonize-flags  Harmonize flag diacritics\n  -H, --do-not-harmonize Do not harmonize\n"
        );
        let _ = writeln!(msg);
        print_common_binary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(msg);
        let _ = write!(
            msg,
            "\nExamples:\n  {} -o dog.hfst cat_or_dog.hfst dog_or_mouse.hfst\n\n",
            common.program_name
        );
    }

    // [spec:hfst:def:hfst-conjunct.parse-options-fn]
    // [spec:hfst:sem:hfst-conjunct.parse-options-fn]
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
            long_options.extend(hfst_getopt_binary_long());
            // add tool-specific options here
            long_options.push(getopt::GetOpt {
                name: "harmonize-flags",
                has_arg: 0,
                val: 'F' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "do-not-harmonize",
                has_arg: 0,
                val: 'H' as i32,
            });
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: binary
            // cases, common cases, then the tool's own ('F'/'H'), then the
            // terminal error arm.
            match handle_binary_case(&mut common, &opt, c) {
                CaseResult::Return(code) => return Err(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_common_case(&mut common, &opt, c, print_usage) {
                CaseResult::Return(code) => return Err(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            if c == 'F' as i32 {
                options.harmonize_flags = true;
                continue;
            }
            if c == 'H' as i32 {
                options.harmonize = false;
                continue;
            }
            return Err(handle_error_case(&common, &opt, c));
        }

        check_binary_params(&mut common, &opt, args);
        check_common_params(&mut common);
        Ok((common, options))
    }

    // [spec:hfst:def:hfst-conjunct.conjunct-streams-fn]
    // [spec:hfst:sem:hfst-conjunct.conjunct-streams-fn]
    // The streams loop lives in crate::binary_ops::run_binary_streams_tool;
    // this descriptor plus the pre-apply/apply closures in run carry the
    // tool's behaviour contract.
    const SPEC: BinaryOpSpec = BinaryOpSpec {
        tool_name: "hfst-conjunct",
        mismatch_noun: "conjunction",
        could_not_verb: "conjunct",
        could_not_noun: "conjunction",
        name_op: "intersect",
        formula: "\u{2229}",
        verbose_begin: |firstname, secondname| {
            format!("Intersecting {} and {}", firstname, secondname)
        },
        loop_style: LoopStyle::Standard,
        retry: RetryPolicy::AnyError,
        flush_each_round: false,
        flush_at_end: true,
    };

    // [spec:hfst:def:hfst-conjunct.main-fn]
    // [spec:hfst:sem:hfst-conjunct.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstConjunct");
        let (common, options) = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };
        let mut op = ConjunctOp {
            harmonize: options.harmonize,
            harmonize_flags: options.harmonize_flags,
            flag_overlay: None,
        };
        run_binary_streams_tool(&common, &SPEC, &mut op)
    }

    struct ConjunctOp {
        harmonize: bool,
        harmonize_flags: bool,
        flag_overlay: Option<FlagDiacriticOverlay>,
    }

    impl BinaryToolOp for ConjunctOp {
        // [spec:hfst:req:virtual-flag-algebra.intersection]
        fn pre_apply<B: AlgebraBackend>(
            &mut self,
            common: &CommonOptions,
            first: &mut HfstTransducer<B>,
            second: &mut HfstTransducer<B>,
            _ctx: &PairContext<'_>,
        ) -> Result<(), i32> {
            self.flag_overlay = None;
            if first.has_flag_diacritics() || second.has_flag_diacritics() {
                if !self.harmonize_flags {
                    if !common.silent {
                        warning(
                            common,
                            0,
                            0,
                            "At least one of the argumentes contains flag diacritics. Use -F to harmonize them.",
                        );
                    }
                } else {
                    let prepared = if B::SUPPORTS_VIRTUAL_FLAG_INTERSECTION {
                        first
                            .prepare_flag_diacritics_for_operation(second)
                            .map(Some)
                    } else {
                        first.harmonize_flag_diacritics(second, true).map(|()| None)
                    };
                    match prepared {
                        Ok(overlay) => self.flag_overlay = overlay,
                        Err(e) => {
                            error(common, 1, 0, &format!("{e}"));
                            return Err(1);
                        }
                    }
                }
            }
            Ok(())
        }

        fn apply<B: AlgebraBackend>(
            &mut self,
            first: &mut HfstTransducer<B>,
            second: &HfstTransducer<B>,
        ) -> hfst::error::Result<()> {
            first
                .intersect_with_flag_overlay(second, self.harmonize, self.flag_overlay.as_ref())
                .map(|_| ())
        }
    }
}

pub mod disjunct {
    //! Faithful 1:1 port of tools/src/hfst-disjunct.cc — the transducer
    //! disjunction (union, OR) command-line tool. Drives the hfst-cli foundation
    //! (globals, getopt, commandline, program-options, tool-metadata, inc
    //! fragments). A BINARY tool: it reads two input streams (firstfile +
    //! secondfile) and writes their disjunction; the shared scaffolding lives in
    //! crate::binary_ops.

    use crate::binary_ops::{
        BinaryOpSpec, BinaryToolOp, LoopStyle, RetryPolicy, run_binary_streams_tool,
    };
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{extend_options_from_env, hfst_set_program_name};
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_binary_long, hfst_getopt_common_long, print_common_binary_program_options,
        print_common_binary_program_parameter_instructions, print_common_program_options,
    };
    use crate::inc::{
        CaseResult, check_binary_params, check_common_params, handle_binary_case,
        handle_common_case, handle_error_case,
    };
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_transducer::HfstTransducer;
    use std::io::Write;

    /// hfst-disjunct's own options (the former tool-specific `static mut`s).
    struct Options {
        /// '-F, --harmonize-flags': harmonize flag diacritics.
        harmonize_flags: bool,
        /// '-H, --do-not-harmonize' clears this (harmonize symbols; default true).
        harmonize: bool,
    }

    impl Default for Options {
        fn default() -> Options {
            Options {
                harmonize_flags: false,
                harmonize: true,
            }
        }
    }

    // [spec:hfst:def:hfst-disjunct.print-usage-fn]
    // [spec:hfst:sem:hfst-disjunct.print-usage-fn]
    fn print_usage(common: &CommonOptions) {
        let mut msg = common.message_writer();
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nDisjunct (union, OR) two transducers\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_binary_program_options(&mut *msg);
        let _ = writeln!(msg);
        print_common_binary_program_parameter_instructions(&mut *msg);
        let _ = write!(
            msg,
            "Harmonization:\n  -H, --do-not-harmonize Do not harmonize symbols.\n  -F, --harmonize-flags  Harmonize flag diacritics.\n"
        );
        let _ = writeln!(msg);
        let _ = write!(
            msg,
            "\nExamples:\n  {} -o cat_or_dog.hfst cat.hfst dog.hfst\n\n",
            common.program_name
        );
    }

    // [spec:hfst:def:hfst-disjunct.parse-options-fn]
    // [spec:hfst:sem:hfst-disjunct.parse-options-fn]
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
            long_options.extend(hfst_getopt_binary_long());
            // add tool-specific options here
            long_options.push(getopt::GetOpt {
                name: "do-not-harmonize",
                has_arg: getopt::NO_ARGUMENT,
                val: b'H' as i32,
            });
            // add tool-specific options here
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then binary cases, then the tool's own ('H'), then the
            // terminal error arm.
            match handle_common_case(&mut common, &opt, c, print_usage) {
                CaseResult::Return(code) => return Err(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_binary_case(&mut common, &opt, c) {
                CaseResult::Return(code) => return Err(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            if c == b'H' as i32 {
                options.harmonize = false;
                continue;
            }
            return Err(handle_error_case(&common, &opt, c));
        }

        check_common_params(&mut common);
        check_binary_params(&mut common, &opt, args);
        Ok((common, options))
    }

    // [spec:hfst:def:hfst-disjunct.disjunct-streams-fn]
    // [spec:hfst:sem:hfst-disjunct.disjunct-streams-fn]
    // The streams loop lives in crate::binary_ops::run_binary_streams_tool;
    // this descriptor plus the apply closure in run carry the tool's
    // behaviour contract.
    const SPEC: BinaryOpSpec = BinaryOpSpec {
        tool_name: "hfst-disjunct",
        mismatch_noun: "disjunction",
        could_not_verb: "disjunct",
        could_not_noun: "disjunction",
        name_op: "union",
        formula: "\u{222a}",
        verbose_begin: |firstname, secondname| {
            format!("Disjuncting {} and {}", firstname, secondname)
        },
        loop_style: LoopStyle::Standard,
        retry: RetryPolicy::AnyError,
        flush_each_round: true,
        flush_at_end: false,
    };

    // [spec:hfst:def:hfst-disjunct.main-fn]
    // [spec:hfst:sem:hfst-disjunct.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstDisjunct");
        let (common, options) = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };

        let _ = options.harmonize_flags;
        let mut op = DisjunctOp {
            harmonize: options.harmonize,
        };
        run_binary_streams_tool(&common, &SPEC, &mut op)
    }

    struct DisjunctOp {
        harmonize: bool,
    }

    impl BinaryToolOp for DisjunctOp {
        fn apply<B: AlgebraBackend>(
            &mut self,
            first: &mut HfstTransducer<B>,
            second: &HfstTransducer<B>,
        ) -> hfst::error::Result<()> {
            first.disjunct(second, self.harmonize).map(|_| ())
        }
    }
}

pub mod priority_disjunct {
    //! Faithful 1:1 port of tools/src/hfst-priority-disjunct.cc — the transducer
    //! priority disjunction (priority union) command-line tool. Drives the
    //! hfst-cli foundation (globals, getopt, commandline, program-options,
    //! tool-metadata, inc fragments). A BINARY tool: it reads two input streams
    //! (firstfile + secondfile) and writes their priority union; the shared
    //! scaffolding lives in crate::binary_ops.

    use crate::binary_ops::{
        BinaryOpSpec, BinaryToolOp, LoopStyle, RetryPolicy, run_binary_streams_tool,
    };
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{extend_options_from_env, hfst_set_program_name};
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_binary_long, hfst_getopt_common_long, print_common_binary_program_options,
        print_common_binary_program_parameter_instructions, print_common_program_options,
    };
    use crate::inc::{
        CaseResult, check_binary_params, check_common_params, handle_binary_case,
        handle_common_case, handle_error_case,
    };
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_transducer::HfstTransducer;
    use std::io::Write;

    /// hfst-priority-disjunct's own options (the former tool-specific `static mut`s).
    struct Options {
        /// '-F, --harmonize-flags': harmonize flag diacritics.
        harmonize_flags: bool,
        /// '-H, --do-not-harmonize': off harmonizes symbols (default on).
        harmonize: bool,
    }

    impl Default for Options {
        fn default() -> Self {
            Options {
                harmonize_flags: false,
                harmonize: true,
            }
        }
    }

    // [spec:hfst:def:hfst-priority-disjunct.print-usage-fn]
    // [spec:hfst:sem:hfst-priority-disjunct.print-usage-fn]
    fn print_usage(common: &CommonOptions) {
        let mut msg = common.message_writer();
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nDisjunct (union, OR) two transducers\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_binary_program_options(&mut *msg);
        let _ = writeln!(msg);
        print_common_binary_program_parameter_instructions(&mut *msg);
        let _ = write!(
            msg,
            "Harmonization:\n  -H, --do-not-harmonize Do not harmonize symbols.\n  -F, --harmonize-flags  Harmonize flag diacritics.\n"
        );
        let _ = writeln!(msg);
        let _ = write!(
            msg,
            "\nExamples:\n  {} -o cat_or_dog.hfst cat.hfst dog.hfst\n\n",
            common.program_name
        );
    }

    // [spec:hfst:def:hfst-priority-disjunct.parse-options-fn]
    // [spec:hfst:sem:hfst-priority-disjunct.parse-options-fn]
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
            long_options.extend(hfst_getopt_binary_long());
            // add tool-specific options here
            long_options.push(getopt::GetOpt {
                name: "do-not-harmonize",
                has_arg: getopt::NO_ARGUMENT,
                val: b'H' as i32,
            });
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: binary
            // cases, then common cases, then the tool's own ('H'), then the
            // terminal error arm.
            match handle_binary_case(&mut common, &opt, c) {
                CaseResult::Return(code) => return Err(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_common_case(&mut common, &opt, c, print_usage) {
                CaseResult::Return(code) => return Err(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            if c == b'H' as i32 {
                options.harmonize = false;
                continue;
            }
            return Err(handle_error_case(&common, &opt, c));
        }

        check_binary_params(&mut common, &opt, args);
        check_common_params(&mut common);
        Ok((common, options))
    }

    // [spec:hfst:def:hfst-priority-disjunct.priority-disjunct-streams-fn]
    // [spec:hfst:sem:hfst-priority-disjunct.priority-disjunct-streams-fn]
    // The streams loop lives in crate::binary_ops::run_binary_streams_tool;
    // this descriptor plus the apply closure in run carry the tool's
    // behaviour contract.
    const SPEC: BinaryOpSpec = BinaryOpSpec {
        tool_name: "hfst-priority-disjunct",
        mismatch_noun: "priority disjunction",
        could_not_verb: "priority disjunct",
        could_not_noun: "priority disjunction",
        name_op: "union",
        formula: "\u{222a}",
        verbose_begin: |firstname, secondname| {
            format!("Disjuncting {} and {}", firstname, secondname)
        },
        loop_style: LoopStyle::Standard,
        retry: RetryPolicy::AnyError,
        flush_each_round: true,
        flush_at_end: false,
    };

    // [spec:hfst:def:hfst-priority-disjunct.main-fn]
    // [spec:hfst:sem:hfst-priority-disjunct.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstPriorityDisjunct");
        let (common, options) = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };

        let _ = options.harmonize_flags;
        let _ = options.harmonize;
        run_binary_streams_tool(&common, &SPEC, &mut PriorityDisjunctOp)
    }

    struct PriorityDisjunctOp;

    impl BinaryToolOp for PriorityDisjunctOp {
        fn apply<B: AlgebraBackend>(
            &mut self,
            first: &mut HfstTransducer<B>,
            second: &HfstTransducer<B>,
        ) -> hfst::error::Result<()> {
            // C: 'first->priority_union(*second)'; no harmonize parameter.
            first.priority_union(second).map(|_| ())
        }
    }
}

pub mod shuffle {
    //! Faithful 1:1 port of tools/src/hfst-shuffle.cc — the transducer shuffle
    //! command-line tool. Drives the hfst-cli foundation (globals, getopt,
    //! commandline, program-options, tool-metadata, inc fragments). A BINARY tool:
    //! it reads two input streams (firstfile + secondfile) and writes their
    //! shuffle; the shared scaffolding lives in crate::binary_ops.

    use crate::binary_ops::{
        BinaryOpSpec, BinaryToolOp, LoopStyle, RetryPolicy, run_binary_streams_tool,
    };
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{extend_options_from_env, hfst_set_program_name};
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_binary_long, hfst_getopt_common_long, print_common_binary_program_options,
        print_common_binary_program_parameter_instructions, print_common_program_options,
    };
    use crate::inc::{
        CaseResult, check_binary_params, check_common_params, handle_binary_case,
        handle_common_case, handle_error_case,
    };
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_transducer::HfstTransducer;
    use std::io::Write;

    // [spec:hfst:def:hfst-shuffle.print-usage-fn]
    // [spec:hfst:sem:hfst-shuffle.print-usage-fn]
    fn print_usage(common: &CommonOptions) {
        let mut msg = common.message_writer();
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nShuffle two transducers\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_binary_program_options(&mut *msg);
        let _ = writeln!(msg);
        print_common_binary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(msg);
        let _ = write!(
            msg,
            "\nExamples:\n  {} -o shuffled.hfst cat.hfst dog.hfst\n\n",
            common.program_name
        );
    }

    // [spec:hfst:def:hfst-shuffle.parse-options-fn]
    // [spec:hfst:sem:hfst-shuffle.parse-options-fn]
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
            long_options.extend(hfst_getopt_binary_long());
            // add tool-specific options here
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: binary
            // cases, then common cases, then the terminal error arm. (The tool
            // defines no options of its own.)
            match handle_binary_case(&mut common, &opt, c) {
                CaseResult::Return(code) => return Err(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_common_case(&mut common, &opt, c, print_usage) {
                CaseResult::Return(code) => return Err(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            return Err(handle_error_case(&common, &opt, c));
        }

        check_binary_params(&mut common, &opt, args);
        check_common_params(&mut common);
        Ok(common)
    }

    // [spec:hfst:def:hfst-shuffle.shuffle-streams-fn]
    // [spec:hfst:sem:hfst-shuffle.shuffle-streams-fn]
    // The streams loop lives in crate::binary_ops::run_binary_streams_tool;
    // this descriptor plus the apply closure in run carry the tool's
    // behaviour contract. The ShuffleAutomata retry policy reproduces the C's
    // outer catch (TransducersAreNotAutomataException) around the inner catch
    // (TransducerTypeMismatchException).
    const SPEC: BinaryOpSpec = BinaryOpSpec {
        tool_name: "hfst-shuffle",
        mismatch_noun: "shuffle",
        could_not_verb: "shuffle",
        could_not_noun: "shuffling",
        name_op: "shuffle",
        formula: "shuffle",
        verbose_begin: |firstname, secondname| {
            format!("Shuffling {} and {}", firstname, secondname)
        },
        loop_style: LoopStyle::Standard,
        retry: RetryPolicy::ShuffleAutomata,
        flush_each_round: false,
        flush_at_end: false,
    };

    // [spec:hfst:def:hfst-shuffle.main-fn]
    // [spec:hfst:sem:hfst-shuffle.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstShuffle");
        let common = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };

        run_binary_streams_tool(&common, &SPEC, &mut ShuffleOp)
    }

    struct ShuffleOp;

    impl BinaryToolOp for ShuffleOp {
        fn apply<B: AlgebraBackend>(
            &mut self,
            first: &mut HfstTransducer<B>,
            second: &HfstTransducer<B>,
        ) -> hfst::error::Result<()> {
            first.shuffle(second, true).map(|_| ())
        }
    }
}

pub mod subtract {
    //! Faithful 1:1 port of tools/src/hfst-subtract.cc — the transducer subtraction
    //! (minus) command-line tool. Drives the hfst-cli foundation (globals, getopt,
    //! commandline, program-options, tool-metadata, inc fragments). A BINARY tool:
    //! it reads two input streams (first + second); the shared scaffolding lives
    //! in crate::binary_ops.

    use crate::binary_ops::{
        BinaryOpSpec, BinaryToolOp, LoopStyle, PairContext, RetryPolicy, run_binary_streams_tool,
    };
    use crate::globals::CommonOptions;
    use crate::hfst_commandline::{error, extend_options_from_env, hfst_set_program_name, warning};
    use crate::hfst_getopt::{self as getopt, Getopt};
    use crate::hfst_program_options::{
        hfst_getopt_binary_long, hfst_getopt_common_long, print_common_binary_program_options,
        print_common_binary_program_parameter_instructions, print_common_program_options,
    };
    use crate::inc::{
        CaseResult, check_binary_params, check_common_params, handle_binary_case,
        handle_common_case, handle_error_case,
    };
    use hfst::backend::AlgebraBackend;
    use hfst::hfst_transducer::FlagDiacriticOverlay;
    use hfst::hfst_transducer::HfstTransducer;
    use std::io::Write;

    /// hfst-subtract's own options (the former tool-specific `static mut`s).
    struct Options {
        /// '-F, --harmonize-flags': harmonize flag diacritics.
        harmonize_flags: bool,
        /// '-H, --do-not-harmonize': off harmonizes symbols (default on).
        harmonize: bool,
    }

    impl Default for Options {
        fn default() -> Self {
            Options {
                harmonize_flags: false,
                harmonize: true,
            }
        }
    }

    // [spec:hfst:def:hfst-subtract.print-usage-fn]
    // [spec:hfst:sem:hfst-subtract.print-usage-fn]
    fn print_usage(common: &CommonOptions) {
        let mut msg = common.message_writer();
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let _ = write!(
            msg,
            "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nSubtract (minus) two transducers\n\n",
            common.program_name
        );
        print_common_program_options(&mut *msg);
        print_common_binary_program_options(&mut *msg);
        let _ = write!(
            msg,
            "Flag diacritics:\n  -F, --harmonize-flags  Harmonize flag diacritics\n  -H, --do-not-harmonize Do not harmonize\n",
        );
        let _ = writeln!(msg);
        print_common_binary_program_parameter_instructions(&mut *msg);
        let _ = writeln!(msg);
        let _ = write!(
            msg,
            "\nExamples:\n  {} -o catdog.hfst cat.hfst dog.hfst  subtracts transducers\n\n",
            common.program_name
        );
    }

    // [spec:hfst:def:hfst-subtract.parse-options-fn]
    // [spec:hfst:sem:hfst-subtract.parse-options-fn]
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
            long_options.extend(hfst_getopt_binary_long());
            // add tool-specific options here
            long_options.push(getopt::GetOpt {
                name: "harmonize-flags",
                has_arg: getopt::NO_ARGUMENT,
                val: 'F' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "do-not-harmonize",
                has_arg: getopt::NO_ARGUMENT,
                val: 'H' as i32,
            });
            let c = opt.getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: binary
            // cases, common cases, then the tool's own ('F'/'H'), then the
            // terminal error arm.
            match handle_binary_case(&mut common, &opt, c) {
                CaseResult::Return(code) => return Err(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_common_case(&mut common, &opt, c, print_usage) {
                CaseResult::Return(code) => return Err(code),
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            if c == 'F' as i32 {
                options.harmonize_flags = true;
                continue;
            }
            if c == 'H' as i32 {
                options.harmonize = false;
                continue;
            }
            return Err(handle_error_case(&common, &opt, c));
        }

        check_binary_params(&mut common, &opt, args);
        check_common_params(&mut common);
        Ok((common, options))
    }

    // [spec:hfst:def:hfst-subtract.subtract-streams-fn]
    // [spec:hfst:sem:hfst-subtract.subtract-streams-fn]
    // The streams loop lives in crate::binary_ops::run_binary_streams_tool;
    // this descriptor plus the pre-apply/apply closures in run carry the
    // tool's behaviour contract.
    const SPEC: BinaryOpSpec = BinaryOpSpec {
        tool_name: "hfst-subtract",
        mismatch_noun: "subtraction",
        could_not_verb: "subtract",
        could_not_noun: "subtraction",
        name_op: "subtract",
        formula: "\u{2212}",
        verbose_begin: |firstname, secondname| {
            format!("Subtracting {} from {}", secondname, firstname)
        },
        loop_style: LoopStyle::Standard,
        retry: RetryPolicy::AnyError,
        flush_each_round: false,
        flush_at_end: true,
    };

    // [spec:hfst:def:hfst-subtract.main-fn]
    // [spec:hfst:sem:hfst-subtract.main-fn]
    pub fn run(mut args: Vec<String>) -> i32 {
        let argv0 = args.first().cloned().unwrap_or_default();

        let common = hfst_set_program_name(&argv0, "0.1", "HfstSubtract");
        let (common, options) = match parse_options(common, &mut args) {
            Ok(v) => v,
            Err(code) => return code,
        };

        let mut op = SubtractOp {
            harmonize: options.harmonize,
            harmonize_flags: options.harmonize_flags,
            flag_overlay: None,
        };
        run_binary_streams_tool(&common, &SPEC, &mut op)
    }

    struct SubtractOp {
        harmonize: bool,
        harmonize_flags: bool,
        flag_overlay: Option<FlagDiacriticOverlay>,
    }

    impl BinaryToolOp for SubtractOp {
        fn pre_apply<B: AlgebraBackend>(
            &mut self,
            common: &CommonOptions,
            first: &mut HfstTransducer<B>,
            second: &mut HfstTransducer<B>,
            _ctx: &PairContext<'_>,
        ) -> Result<(), i32> {
            self.flag_overlay = None;
            if second.has_flag_diacritics() {
                warning(
                    common,
                    0,
                    0,
                    &format!(
                        "Warning: {} contains flag diacritics. The result of subtraction may be incorrect.",
                        common.second_filename
                    ),
                );
            }
            let first_has_flags = first.has_flag_diacritics();
            let second_has_flags = second.has_flag_diacritics();
            if first_has_flags && second_has_flags {
                if !self.harmonize_flags {
                    if !common.silent {
                        warning(
                            common,
                            0,
                            0,
                            "The argumentes contain flag diacritics. Use -F to harmonize them.",
                        );
                    }
                } else {
                    let prepared = if B::SUPPORTS_VIRTUAL_FLAG_SUBTRACTION {
                        first
                            .prepare_flag_diacritics_for_operation(second)
                            .map(Some)
                    } else {
                        // C: 'first->harmonize_flag_diacritics(*second)' — relies
                        // on the default 'insert_renamed_flags=true'.
                        first.harmonize_flag_diacritics(second, true).map(|()| None)
                    };
                    match prepared {
                        Ok(overlay) => self.flag_overlay = overlay,
                        Err(e) => {
                            error(common, 1, 0, &format!("{e}"));
                            return Err(1);
                        }
                    }
                }
            }
            Ok(())
        }

        fn apply<B: AlgebraBackend>(
            &mut self,
            first: &mut HfstTransducer<B>,
            second: &HfstTransducer<B>,
        ) -> hfst::error::Result<()> {
            first
                .subtract_with_flag_overlay(second, self.harmonize, self.flag_overlay.as_ref())
                .map(|_| ())
        }
    }
}
