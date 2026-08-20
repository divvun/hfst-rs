//! Faithful 1:1 port of tools/src/hfst-substitute.cc — the transducer label
//! modification (relabel arcs) command-line tool. Drives the hfst-cli
//! foundation (globals, getopt, commandline, program-options, tool-metadata,
//! inc fragments). A UNARY tool (it #includes globals-unary.h,
//! getopt-cases-unary.h, check-params-unary.h) that additionally reads an
//! optional replacement transducer (-T) and/or a replacement file (-F).
//!
//! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
//! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
//! `parse_options` and threaded into the processing functions. There are no
//! `static mut` globals and no `unsafe`.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    conversion_type, convert_any, error, extend_options_from_env, hfst_error, hfst_error_at_line,
    hfst_set_program_name, hfst_strformat, hfst_warning, is_input_stream_in_ol_format,
    verbose_print,
};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use crate::hfst_tool_metadata::{hfst_set_formula_unary, hfst_set_name_unary};
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use hfst::hfst_data_types::{ImplementationType, StringPair};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_symbol_defs::{internal_epsilon, label_to_stringpair};
use hfst::hfst_transducer::HfstTransducer;
use hfst::substitute_driver::{SubstituteEngine, SubstituteRequest};
use std::io::{BufRead, BufReader, Write};

/// hfst-substitute's own options (the former tool-specific `static mut`s). The
/// `-C, --do-not-convert` flag lives in [`CommonOptions::allow_transducer_conversion`].
#[derive(Default)]
struct Options {
    from_label: Option<String>,
    from_pair: Option<StringPair>,
    from_file_name: Option<String>,
    from_file: Option<BufReader<std::fs::File>>,
    to_label: Option<String>,
    to_pair: Option<StringPair>,
    to_transducer_filename: Option<String>,
    compose: bool,
    in_order: bool,
}

// The substitution the engine is asked for, rebuilt from the options before
// every call: a relabel file rewrites the from/to fields line by line.
fn substitute_request(options: &Options) -> SubstituteRequest {
    SubstituteRequest {
        from_label: options.from_label.clone(),
        from_pair: options.from_pair.clone(),
        to_label: options.to_label.clone(),
        to_pair: options.to_pair.clone(),
        to_transducer_filename: options.to_transducer_filename.clone(),
        compose: options.compose,
    }
}

// Reads one line from a buffered reader the way 'hfst_getline' does: the
// returned string keeps its trailing newline; 'None' marks end of input.
fn read_line(f: &mut dyn BufRead) -> Option<String> {
    let mut s = String::new();
    match f.read_line(&mut s) {
        Ok(0) => None,
        Ok(_) => Some(s),
        Err(_) => None,
    }
}

// [spec:hfst:req:cli.help]
fn print_usage(common: &CommonOptions) {
    let mut msg = common.message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let program_name = &common.program_name;
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nRelabel transducer arcs\n\n",
        program_name
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Relabeling options:\n\
         \x20 -f, --from-label=FLABEL      replace FLABEL\n\
         \x20 -t, --to-label=TLABEL        replace with TLABEL\n\
         \x20 -T, --to-transducer=TFILE    replace with transducer read from TFILE\n\
         \x20 -F, --from-file=LABELFILE    read replacements from LABELFILE\n\
         \x20 -R, --in-order               keep the order of the replacements\n\
         \x20                              (with -F)\n\
         Input options:\n\
         \x20 -C, --do-not-convert         require that transducers in TFILE and INFILE\n\
         \x20                              have the same type\n\
         Transient optimisation schemes:\n\
         \x20 -9, --compose                compose substitutions when possible\n",
    );
    let _ = writeln!(msg);
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(
        msg,
        "LABEL must be a symbol name in single arc in transducer,\n\
         or colon separated pair defining an arc.\n\
         If TFILE is specified, FLABEL must be a pair.\n\
         LABELFILE is a 2 column tsv file where col 1 is FLABEL\n\
         and col 2 gives TLABEL specifications.\n",
    );
    let _ = write!(
        msg,
        "\nExamples:\n\
         \x20 {pn} -i tr.hfst -o tr_relabeled.hfst -f 'a' -t 'A'\n\
         \x20     relabel all symbols 'a' with 'A'\n\
         \x20 {pn} -i tr.hfst -o tr_relabeled.hfst -f 'a:b' -t 'A:B'\n\
         \x20     relabel all arcs 'a:b' with 'A:B'\n\
         \x20 {pn} -i tr.hfst -o tr_relabeled.hfst -f 'a:b' -T repl.hfst\n\
         \x20     replace all arcs 'a:b' with transducer repl.hfst\n\n",
        pn = program_name
    );
}

// [spec:hfst:def:hfst-substitute.parse-options-fn]
// [spec:hfst:sem:hfst-substitute.parse-options-fn]
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
        for (name, has_arg, val) in [
            ("from-label", getopt::REQUIRED_ARGUMENT, b'f'),
            ("from-file", getopt::REQUIRED_ARGUMENT, b'F'),
            ("to-label", getopt::REQUIRED_ARGUMENT, b't'),
            ("to-transducer", getopt::REQUIRED_ARGUMENT, b'T'),
            ("in-order", getopt::NO_ARGUMENT, b'R'),
            ("compose", getopt::NO_ARGUMENT, b'9'),
            ("do-not-convert", getopt::NO_ARGUMENT, b'C'),
        ] {
            long_options.push(getopt::GetOpt {
                name,
                has_arg,
                val: val as i32,
            });
        }
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
        if c == b'f' as i32 {
            let mut fl = opt.optarg();
            if fl == "@0@" {
                fl = internal_epsilon.to_string();
            }
            options.from_pair = label_to_stringpair(&fl);
            if fl.is_empty() {
                hfst_error(
                    &common,
                    1,
                    0,
                    &format!(
                        "argument of source label option is empty;\n\
                         if you REALLY want to replace epsilons with something, use @0@ or {}",
                        internal_epsilon
                    ),
                );
            }
            options.from_label = Some(fl);
        } else if c == b'F' as i32 {
            let fname = opt.optarg();
            match std::fs::File::open(&fname) {
                Ok(f) => options.from_file = Some(BufReader::new(f)),
                Err(_) => {
                    error(&common, 1, 0, &format!("Could not open '{}'", fname));
                    options.from_file_name = Some(fname);
                    return Err(1);
                }
            }
            options.from_file_name = Some(fname);
        } else if c == b't' as i32 {
            let mut tl = opt.optarg();
            if tl == "@0@" {
                tl = internal_epsilon.to_string();
            }
            options.to_pair = label_to_stringpair(&tl);
            if tl.is_empty() {
                hfst_error(
                    &common,
                    1,
                    0,
                    &format!(
                        "argument of target label option is empty;\n\
                         if you want to substitute something with epsilons, use @0@ or {}",
                        internal_epsilon
                    ),
                );
            }
            options.to_label = Some(tl);
        } else if c == b'T' as i32 {
            let fname = opt.optarg();
            // C: probe the file with hfst_fopen then immediately fclose; here
            // we just check it opens (the std File drops/closes at scope end).
            match std::fs::File::open(&fname) {
                Ok(_f) => {}
                Err(_) => {
                    error(&common, 1, 0, &format!("Could not open '{}'", fname));
                    options.to_transducer_filename = Some(fname);
                    return Err(1);
                }
            }
            options.to_transducer_filename = Some(fname);
        } else if c == b'R' as i32 {
            options.in_order = true;
        } else if c == b'9' as i32 {
            options.compose = true;
        } else if c == b'C' as i32 {
            common.allow_transducer_conversion = false;
        } else {
            return Err(handle_error_case(&common, &opt, c));
        }
    }

    if options.from_label.is_none() && options.from_file_name.is_none() {
        hfst_error(
            &common,
            1,
            0,
            "Must state name of labels to rewrite with -f or -F",
        );
        return Err(1);
    }
    if options.to_label.is_none()
        && options.to_transducer_filename.is_none()
        && options.from_file_name.is_none()
    {
        hfst_error(&common, 1, 0, "Must give target labels with -t, -T or -F");
        return Err(1);
    }
    check_common_params(&mut common);
    check_unary_params(&mut common, &opt, args);
    Ok((common, options))
}

// [spec:hfst:def:hfst-substitute.process-stream-fn]
// [spec:hfst:sem:hfst-substitute.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &mut Options,
    instream: &mut HfstInputStream<'_>,
) -> i32 {
    let mut to_any: Option<hfst::hfst_transducer::AnyTransducer> = None;
    let mut output_type = ImplementationType::UNSPECIFIED_TYPE;

    if options.to_transducer_filename.is_some() {
        let to_fname = options.to_transducer_filename.clone().unwrap();
        // (C wraps the ctor in try/catch on NotTransducerStreamException; the
        // Rust ctor panics on a bad file rather than throwing.)
        let mut tostream = match HfstInputStream::new_filename(&to_fname) {
            Ok(s) => s,
            Err(e) => {
                hfst_error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        let mut to_transducer = match tostream.read() {
            Ok(t) => t,
            Err(e) => {
                hfst_error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        tostream.close();
        let to_transducer_type = to_transducer.get_type();
        let instream_type = instream.get_type();
        if to_transducer_type != instream_type {
            if common.allow_transducer_conversion {
                let ct = conversion_type(instream_type, to_transducer_type);
                let mut warnstr = format!(
                    "Transducer type mismatch in {} and {}; ",
                    common.input_filename, to_fname
                );
                if ct == 1 {
                    warnstr.push_str("using former type as output");
                    output_type = instream_type;
                } else if ct == 2 {
                    warnstr.push_str("using latter type as output");
                    output_type = to_transducer_type;
                } else if ct == -1 {
                    warnstr
                        .push_str("using former type as output, loss of information is possible");
                    output_type = instream_type;
                } else {
                    /* should not happen */
                    std::panic::panic_any(String::from(
                        "Error: hfst-disjunct: conversion_type returned an invalid integer",
                    ));
                }
                hfst_warning(common, 0, 0, &warnstr);
                to_transducer = match convert_any(to_transducer, output_type) {
                    Ok(t) => t,
                    Err(e) => {
                        hfst_error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                };
            } else {
                hfst_error(
                    common,
                    1,
                    0,
                    &format!(
                        "Transducer type mismatch in {} and {}; \
                         formats {} and {} are not compatible for substitution \
                         (--do-not-convert was requested)",
                        common.input_filename,
                        to_fname,
                        hfst_strformat(instream_type),
                        hfst_strformat(to_transducer_type)
                    ),
                );
            }
        } else {
            output_type = instream.get_type();
        }
        to_any = Some(to_transducer);
    } else {
        output_type = instream.get_type();
    }

    let output_named = common.output_filename != "<stdout>";
    let mut outstream = match if output_named {
        HfstOutputStream::new_filename(&common.output_filename, output_type, true)
    } else {
        HfstOutputStream::new(output_type, true)
    } {
        Ok(s) => s,
        Err(e) => {
            hfst_error(common, 1, 0, &format!("{e}"));
            return 1;
        }
    };

    // The resolved output type is matched ONCE into the loop's backend
    // type parameter ([dec:hfst:monomorphic-backends]); OL streams were
    // rejected before this point.
    match output_type {
        ImplementationType::SFST_TYPE
        | ImplementationType::TROPICAL_OPENFST_TYPE
        | ImplementationType::FOMA_TYPE
        | ImplementationType::XFSM_TYPE
        | ImplementationType::HFST_OL_TYPE
        | ImplementationType::HFST_OLW_TYPE
        | ImplementationType::THFST_TYPE
        | ImplementationType::HFST2_TYPE
        | ImplementationType::UNSPECIFIED_TYPE
        | ImplementationType::ERROR_TYPE => process_loop::<hfst_openfst::StdVectorFst>(
            common,
            options,
            instream,
            &mut outstream,
            to_any,
        ),
    }
}

fn process_loop<B: hfst::backend::AlgebraBackend + hfst::hfst_transducer::FromAnyTransducer>(
    common: &CommonOptions,
    options: &mut Options,
    instream: &mut HfstInputStream<'_>,
    outstream: &mut HfstOutputStream,
    to_any: Option<hfst::hfst_transducer::AnyTransducer>,
) -> i32 {
    let mut transducer_n: usize = 0;

    let to_transducer: Option<HfstTransducer<B>> = match to_any {
        Some(a) => match a.into_typed() {
            Ok(t) => Some(t),
            Err(e) => {
                hfst_error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        },
        None => None,
    };
    let mut engine: SubstituteEngine<B> = SubstituteEngine::new(to_transducer);
    let reporter = crate::CliVerboseReporter::new(common);

    while instream.is_good() {
        transducer_n += 1;
        let mut trans: HfstTransducer<B> = match instream.read().and_then(|any| any.into_typed()) {
            Ok(t) => t,
            Err(e) => {
                hfst_error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        let inputname = {
            let n = trans.get_name();
            if n.is_empty() {
                common.input_filename.clone()
            } else {
                n
            }
        };
        if transducer_n == 1 {
            verbose_print(
                common,
                &format!("performing substitutions in {}...\n", inputname),
            );
        } else {
            verbose_print(
                common,
                &format!(
                    "performing substitutions in {}... {}\n",
                    inputname, transducer_n
                ),
            );
        }
        // initialize delayed substitutor automaton
        engine.begin_transducer();
        if options.from_file.is_some() {
            let from_file_name = options.from_file_name.clone().unwrap();
            let mut line_n: u32 = 0;
            verbose_print(
                common,
                &format!("reading substitutions from {}...\n", from_file_name),
            );
            while let Some(line) = read_line(options.from_file.as_mut().unwrap()) {
                line_n += 1;
                if line.starts_with('\n') {
                    continue;
                }
                let tab = match line.find('\t') {
                    None => {
                        if line.starts_with('#') {
                            continue;
                        } else {
                            hfst_error_at_line(
                                common,
                                1,
                                0,
                                &from_file_name,
                                line_n,
                                "At least one tab required per line",
                            );
                            continue;
                        }
                    }
                    Some(t) => t,
                };
                // 'endstr' advances from the tab to the end of line or newline.
                let rest = &line[tab + 1..];
                let end = rest.find('\n').unwrap_or(rest.len());
                let fl = line[0..tab].to_string();
                let tl = rest[0..end].to_string();
                options.from_pair = label_to_stringpair(&fl);
                options.to_pair = label_to_stringpair(&tl);
                if fl.is_empty() {
                    hfst_error_at_line(
                        common,
                        1,
                        0,
                        &from_file_name,
                        line_n,
                        &format!(
                            "First field is empty;\n\
                             if you REALLY want to replace epsilons withsomething, use @0@ or {}",
                            internal_epsilon
                        ),
                    );
                }
                if tl.is_empty() {
                    hfst_error_at_line(
                        common,
                        1,
                        0,
                        &from_file_name,
                        line_n,
                        &format!(
                            "Second field seems empty;\n\
                             if you want to substitute something with epsilons, use @0@ or {}",
                            internal_epsilon
                        ),
                    );
                }
                options.from_label = Some(fl.clone());
                options.to_label = Some(tl.clone());

                if let Err(e) = engine.apply_relabel_entry(
                    &substitute_request(options),
                    &mut trans,
                    transducer_n,
                    options.in_order,
                    &reporter,
                ) {
                    hfst_error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            } // while getline

            if let Err(e) = engine.flush_batched(&mut trans, options.in_order) {
                hfst_error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        }
        // if not from file
        else if let Err(e) = engine.do_substitute(
            &substitute_request(options),
            &mut trans,
            transducer_n,
            &reporter,
        ) {
            hfst_error(common, 1, 0, &format!("{e}"));
            return 1;
        }
        if engine.is_delayed()
            && let Err(e) = engine.perform_delayed(&mut trans, &reporter)
        {
            hfst_error(common, 1, 0, &format!("{e}"));
            return 1;
        }
        if options.from_file.is_some() {
            let from_file_name = options.from_file_name.clone().unwrap();
            let src = trans.clone();
            hfst_set_name_unary(
                &mut trans,
                &src,
                &format!("substitute-from-{}", from_file_name),
            );
            let src = trans.clone();
            hfst_set_formula_unary(&mut trans, &src, &format!("♲{}", from_file_name));
        } else if options.from_label.is_some() && options.to_label.is_some() {
            let fl = options.from_label.clone().unwrap();
            let tl = options.to_label.clone().unwrap();
            let src = trans.clone();
            hfst_set_name_unary(&mut trans, &src, &format!("substitute-{}-with-{}", fl, tl));
            let src = trans.clone();
            hfst_set_formula_unary(&mut trans, &src, &format!("{} ♲ {}", fl, tl));
        } else if options.to_transducer_filename.is_some() {
            if options.from_label.is_none() {
                // make scan-build happy, this should not happen
                std::panic::panic_any(String::from("Error: from_label has a NULL value."));
            }
            let fl = options.from_label.clone().unwrap();
            let tf = options.to_transducer_filename.clone().unwrap();
            let src = trans.clone();
            hfst_set_name_unary(
                &mut trans,
                &src,
                &format!("substitute-{}-with-net-{}", fl, tf),
            );
            let src = trans.clone();
            hfst_set_formula_unary(&mut trans, &src, &format!("{} ♲ {}", fl, tf));
        }
        if let Err(e) = outstream.redirect(&mut trans) {
            hfst_error(common, 1, 0, &format!("{e}"));
            return 1;
        }
    }
    // delete to_transducer
    engine.release_to_transducer();
    instream.close();
    outstream.close();
    0
}

// [spec:hfst:def:hfst-substitute.main-fn]
// [spec:hfst:sem:hfst-substitute.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstSubstitute");
    let (common, mut options) = match parse_options(common, &mut args) {
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

    // (the C allocated the two batched substitution maps here; they are now
    // fields of the library engine, allocated with it in process_loop.)

    // here starts the buffer handling part
    // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
    // currently panics on a bad file rather than throwing.)
    let mut instream = match if input_opened {
        HfstInputStream::new_filename(&common.input_filename)
    } else {
        HfstInputStream::new()
    } {
        Ok(s) => s,
        Err(e) => {
            hfst_error(&common, 1, 0, &format!("{e}"));
            return 1;
        }
    };

    if is_input_stream_in_ol_format(&instream, "hfst-substitute") {
        return 1;
    }

    process_stream(&common, &mut options, &mut instream)
}
