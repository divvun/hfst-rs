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
use hfst::hfst_data_types::{ImplementationType, StringPair, Symbol};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_symbol_defs::{internal_epsilon, label_to_stringpair};
use hfst::hfst_transducer::HfstTransducer;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};

// [spec:hfst:def:hfst-substitute.hfst-symbol-substitutions]
type HfstSymbolSubstitutions = BTreeMap<Symbol, Symbol>;
// [spec:hfst:def:hfst-substitute.hfst-symbol-pair-substitutions]
type HfstSymbolPairSubstitutions = BTreeMap<StringPair, StringPair>;

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
    delayed: bool,
    label_substitution_map: Option<HfstSymbolSubstitutions>,
    pair_substitution_map: Option<HfstSymbolPairSubstitutions>,
    in_order: bool,
}

// The C statics 'to_transducer' and 'substitution_trans' carry the loop's
// backend type parameter now ([dec:hfst:monomorphic-backends]), so they live
// in this state struct threaded through the typed body instead of in statics.
struct SubstState<B: hfst::backend::AlgebraBackend> {
    to_transducer: Option<HfstTransducer<B>>,
    substitution_trans: Option<HfstTransducer<B>>,
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

// [spec:hfst:def:hfst-substitute.print-usage-fn]
// [spec:hfst:sem:hfst-substitute.print-usage-fn]
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
    let _ = write!(msg, "\n");
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

// Resolves the display name of the replacement transducer: its stored name, or
// the '-T' filename when unnamed.
fn to_transducer_name<B: hfst::backend::AlgebraBackend>(
    options: &Options,
    state: &SubstState<B>,
) -> String {
    let n = state.to_transducer.as_ref().unwrap().get_name();
    if n.is_empty() {
        options.to_transducer_filename.clone().unwrap()
    } else {
        n
    }
}

// The 'HfstTransducer&' overload of 'do_substitute'.
fn do_substitute<B: hfst::backend::AlgebraBackend>(
    common: &CommonOptions,
    options: &mut Options,
    trans: &mut HfstTransducer<B>,
    transducer_n: usize,
    state: &mut SubstState<B>,
) -> hfst::error::Result<()> {
    let from_pair = options.from_pair.clone();
    let to_pair = options.to_pair.clone();
    let from_label = options.from_label.clone();
    let to_label = options.to_label.clone();
    let has_to_transducer = state.to_transducer.is_some();
    if let (Some(fp), Some(tp)) = (&from_pair, &to_pair) {
        verbose_print(
            common,
            &format!(
                "Substituting pair {}:{} with pair {}:{}...\n",
                fp.0, fp.1, tp.0, tp.1
            ),
        );
        trans.substitute_symbol_pair(fp, tp)?;
    } else if let (Some(fl), Some(tl)) = (&from_label, &to_label) {
        if options.compose {
            if transducer_n < 2 {
                verbose_print(
                    common,
                    &format!(
                        "Delaying substitution of label {} with label {}...\n",
                        fl, tl
                    ),
                );
            } else {
                verbose_print(
                    common,
                    &format!(
                        "Delaying substitution of label {} with label {}... {}\n",
                        fl, tl, transducer_n
                    ),
                );
            }
            let substitution: HfstTransducer<B> = HfstTransducer::new_symbol_pair(fl, tl)?;
            state
                .substitution_trans
                .as_mut()
                .expect("substitution_trans initialized per transducer above")
                .disjunct(&substitution, true)?;
            options.delayed = true;
        } else {
            if transducer_n < 2 {
                verbose_print(
                    common,
                    &format!("Substituting label {} with label {}...\n", fl, tl),
                );
            } else {
                verbose_print(
                    common,
                    &format!(
                        "Substituting label {} with label {}... {}\n",
                        fl, tl, transducer_n
                    ),
                );
            }
            trans.substitute(fl, tl, true, true)?;
        }
    } else if let (Some(fp), true) = (&from_pair, has_to_transducer) {
        let to_name = to_transducer_name(options, state);
        if transducer_n < 2 {
            verbose_print(
                common,
                &format!(
                    "Substituting pair {}:{} with transducer {}...\n",
                    fp.0, fp.1, to_name
                ),
            );
        } else {
            verbose_print(
                common,
                &format!(
                    "Substituting pair {}:{} with transducer {}... {}\n",
                    fp.0, fp.1, to_name, transducer_n
                ),
            );
        }
        let to_t = state
            .to_transducer
            .as_mut()
            .expect("to_transducer present when has_to_transducer is true");
        trans.substitute_symbol_pair_with_transducer(fp, to_t, true)?;
    } else if let (Some(fl), true) = (&from_label, has_to_transducer) {
        let to_name = to_transducer_name(options, state);
        if transducer_n < 2 {
            verbose_print(
                common,
                &format!(
                    "Substituting id. label {} with transducer {}...\n",
                    fl, to_name
                ),
            );
        } else {
            verbose_print(
                common,
                &format!(
                    "Substituting id. label {} with transducer {}... {}\n",
                    fl, to_name, transducer_n
                ),
            );
        }
        let from_arc: StringPair = (Symbol::new(fl), Symbol::new(fl));
        let to_t = state
            .to_transducer
            .as_mut()
            .expect("to_transducer present when has_to_transducer is true");
        trans.substitute_symbol_pair_with_transducer(&from_arc, to_t, true)?;
    }
    Ok(())
}

// [spec:hfst:def:hfst-substitute.perform-delayed-fn]
// [spec:hfst:sem:hfst-substitute.perform-delayed-fn]
fn perform_delayed<B: hfst::backend::AlgebraBackend>(
    common: &CommonOptions,
    trans: &mut HfstTransducer<B>,
    state: &SubstState<B>,
) -> hfst::error::Result<()> {
    verbose_print(common, "Finalising substitution transducer...\n");
    trans.substitute_by_composition(
        state
            .substitution_trans
            .as_ref()
            .expect("substitution_trans initialized per transducer above"),
    )?;
    Ok(())
}

// [spec:hfst:def:hfst-substitute.process-stream-fn]
// [spec:hfst:sem:hfst-substitute.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &mut Options,
    instream: &mut HfstInputStream,
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
        ImplementationType::LOG_OPENFST_TYPE => {
            process_loop::<hfst::log_weight_transducer::LogFst>(
                common,
                options,
                instream,
                &mut outstream,
                to_any,
            )
        }
        _ => process_loop::<hfst_openfst::StdVectorFst>(
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
    instream: &mut HfstInputStream,
    outstream: &mut HfstOutputStream,
    to_any: Option<hfst::hfst_transducer::AnyTransducer>,
) -> i32 {
    let mut symbol_pair_map_in_use = false;
    let mut symbol_map_in_use = false;

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
    let mut state = SubstState {
        to_transducer,
        substitution_trans: None,
    };

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
        state.substitution_trans = Some(HfstTransducer::new());
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

                if options.from_pair.is_some() && options.to_pair.is_some() {
                    if !options.in_order {
                        options.pair_substitution_map.as_mut().unwrap().insert(
                            options.from_pair.clone().unwrap(),
                            options.to_pair.clone().unwrap(),
                        );
                        symbol_pair_map_in_use = true;
                    } else if let Err(e) =
                        do_substitute(common, options, &mut trans, transducer_n, &mut state)
                    {
                        hfst_error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                } else if !fl.is_empty() && !tl.is_empty() {
                    if !options.in_order {
                        options
                            .label_substitution_map
                            .as_mut()
                            .unwrap()
                            .insert(Symbol::new(&fl), Symbol::new(&tl));
                        symbol_map_in_use = true;
                    } else if let Err(e) =
                        do_substitute(common, options, &mut trans, transducer_n, &mut state)
                    {
                        hfst_error(common, 1, 0, &format!("{e}"));
                        return 1;
                    }
                } else if let Err(e) =
                    do_substitute(common, options, &mut trans, transducer_n, &mut state)
                {
                    hfst_error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            } // while getline

            // perform label-to-label substitution right away
            if !options.in_order && symbol_map_in_use {
                if let Err(e) = trans.substitute_substitutions(
                    options
                        .label_substitution_map
                        .as_ref()
                        .expect("label_substitution_map initialized when from_file present"),
                ) {
                    hfst_error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
                symbol_map_in_use = false;
            }

            // perform symbol pair-to-symbol pair substitution right away
            if !options.in_order && symbol_pair_map_in_use {
                if let Err(e) = trans.substitute_symbol_pairs(
                    options
                        .pair_substitution_map
                        .as_ref()
                        .expect("pair_substitution_map initialized when from_file present"),
                ) {
                    hfst_error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
                symbol_pair_map_in_use = false;
            }
        }
        // if not from file
        else if let Err(e) = do_substitute(common, options, &mut trans, transducer_n, &mut state)
        {
            hfst_error(common, 1, 0, &format!("{e}"));
            return 1;
        }
        if options.delayed {
            if let Err(e) = perform_delayed(common, &mut trans, &state) {
                hfst_error(common, 1, 0, &format!("{e}"));
                return 1;
            }
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
    state.to_transducer = None;
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

    if options.from_file.is_some() {
        options.label_substitution_map = Some(HfstSymbolSubstitutions::new());
        options.pair_substitution_map = Some(HfstSymbolPairSubstitutions::new());
    }

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
