//! Faithful 1:1 port of tools/src/hfst-substitute.cc — the transducer label
//! modification (relabel arcs) command-line tool. Drives the hfst-cli
//! foundation (globals, getopt, commandline, program-options, tool-metadata,
//! inc fragments). A UNARY tool (it #includes globals-unary.h,
//! getopt-cases-unary.h, check-params-unary.h) that additionally reads an
//! optional replacement transducer (-T) and/or a replacement file (-F).

use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_data_types::{ImplementationType, StringPair};
use hfst::hfst_exception_defs::FunctionNotImplementedException;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_symbol_defs::{internal_epsilon, internal_identity, label_to_stringpair};
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, conversion_type, extend_options_getenv, hfst_error, hfst_error_at_line,
    hfst_fopen, hfst_set_program_name, hfst_strformat, hfst_warning, is_input_stream_in_ol_format,
    print_more_info, print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT, hfst_getopt_common_long,
    hfst_getopt_unary_long, print_common_program_options, print_common_unary_program_options,
    print_common_unary_program_parameter_instructions,
};
use hfst_cli::hfst_tool_metadata::{hfst_set_formula_unary, hfst_set_name_unary};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use libc::{c_char, c_int};
use std::collections::BTreeMap;
use std::ffi::{CStr, CString};

// [spec:hfst:def:hfst-substitute.hfst-symbol-substitutions]
type HfstSymbolSubstitutions = BTreeMap<String, String>;
// [spec:hfst:def:hfst-substitute.hfst-symbol-pair-substitutions]
type HfstSymbolPairSubstitutions = BTreeMap<StringPair, StringPair>;

// File-scoped state mirroring the C static globals.
static mut FROM_LABEL: Option<String> = None;
static mut FROM_PAIR: Option<StringPair> = None;
static mut FROM_FILE_NAME: Option<String> = None;
static mut FROM_FILE: *mut libc::FILE = std::ptr::null_mut();
static mut TO_LABEL: Option<String> = None;
static mut TO_PAIR: Option<StringPair> = None;
static mut TO_TRANSDUCER_FILENAME: Option<String> = None;
static mut TO_TRANSDUCER: Option<HfstTransducer> = None;
static mut COMPOSE: bool = false;
static mut SUBSTITUTION_TRANS: Option<HfstTransducer> = None;
static mut DELAYED: bool = false;
static mut LABEL_SUBSTITUTION_MAP: Option<HfstSymbolSubstitutions> = None;
static mut PAIR_SUBSTITUTION_MAP: Option<HfstSymbolPairSubstitutions> = None;
static mut IN_ORDER: bool = false;
static mut ALLOW_TRANSDUCER_CONVERSION: bool = true;

unsafe fn cstr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

unsafe fn fput(f: *mut libc::FILE, s: &str) {
    let c = CString::new(s).unwrap_or_default();
    unsafe { libc::fputs(c.as_ptr(), f) };
}

// Reads one line from a C 'FILE*' the way 'hfst_getline' does: the returned
// string keeps its trailing newline; 'None' marks end of input.
unsafe fn read_line(f: *mut libc::FILE) -> Option<String> {
    unsafe {
        let mut line: *mut c_char = std::ptr::null_mut();
        let mut len: libc::size_t = 0;
        let n = libc::getline(&mut line, &mut len, f);
        if n == -1 {
            if !line.is_null() {
                libc::free(line as *mut libc::c_void);
            }
            None
        } else {
            let s = CStr::from_ptr(line).to_string_lossy().into_owned();
            libc::free(line as *mut libc::c_void);
            Some(s)
        }
    }
}

// [spec:hfst:def:hfst-substitute.print-usage-fn]
// [spec:hfst:sem:hfst-substitute.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            globals::message_out(),
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\nRelabel transducer arcs\n\n",
                program_name
            ),
        );
        print_common_program_options(globals::message_out());
        print_common_unary_program_options(globals::message_out());
        fput(
            globals::message_out(),
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
        fput(globals::message_out(), "\n");
        print_common_unary_program_parameter_instructions(globals::message_out());
        fput(
            globals::message_out(),
            "LABEL must be a symbol name in single arc in transducer,\n\
             or colon separated pair defining an arc.\n\
             If TFILE is specified, FLABEL must be a pair.\n\
             LABELFILE is a 2 column tsv file where col 1 is FLABEL\n\
             and col 2 gives TLABEL specifications.\n",
        );
        fput(
            globals::message_out(),
            &format!(
                "\nExamples:\n\
                 \x20 {pn} -i tr.hfst -o tr_relabeled.hfst -f 'a' -t 'A'\n\
                 \x20     relabel all symbols 'a' with 'A'\n\
                 \x20 {pn} -i tr.hfst -o tr_relabeled.hfst -f 'a:b' -t 'A:B'\n\
                 \x20     relabel all arcs 'a:b' with 'A:B'\n\
                 \x20 {pn} -i tr.hfst -o tr_relabeled.hfst -f 'a:b' -T repl.hfst\n\
                 \x20     replace all arcs 'a:b' with transducer repl.hfst\n\n",
                pn = program_name
            ),
        );
        print_report_bugs();
        fput(globals::message_out(), "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-substitute.parse-options-fn]
// [spec:hfst:sem:hfst-substitute.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            for (name, has_arg, val) in [
                ("from-label\0", 1, b'f'),
                ("from-file\0", 1, b'F'),
                ("to-label\0", 1, b't'),
                ("to-transducer\0", 1, b'T'),
                ("in-order\0", 0, b'R'),
                ("compose\0", 0, b'9'),
                ("do-not-convert\0", 0, b'C'),
            ] {
                long_options.push(getopt::Option {
                    name: name.as_ptr() as *const c_char,
                    has_arg,
                    flag: std::ptr::null_mut(),
                    val: val as c_int,
                });
            }
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}f:F:t:T:R9C",
                HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT
            ))
            .unwrap();
            let mut option_index: c_int = 0;
            let c = getopt::getopt_long(
                argc,
                argv,
                short.as_ptr(),
                long_options.as_ptr(),
                &mut option_index,
            );
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own, then the terminal
            // error arm.
            match handle_common_case(c, || print_usage()) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_unary_case(c) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }

            // add tool-specific cases here
            if c == b'f' as c_int {
                let mut fl = cstr(getopt::OPTARG);
                if fl == "@0@" {
                    fl = internal_epsilon.to_string();
                }
                FROM_PAIR = label_to_stringpair(&fl);
                if fl.is_empty() {
                    hfst_error(
                        libc::EXIT_FAILURE,
                        0,
                        &format!(
                            "argument of source label option is empty;\n\
                             if you REALLY want to replace epsilons with something, use @0@ or {}",
                            internal_epsilon
                        ),
                    );
                }
                FROM_LABEL = Some(fl);
            } else if c == b'F' as c_int {
                let fname = cstr(getopt::OPTARG);
                FROM_FILE = hfst_fopen(&fname, "r");
                FROM_FILE_NAME = Some(fname);
                if FROM_FILE.is_null() {
                    return libc::EXIT_FAILURE;
                }
            } else if c == b't' as c_int {
                let mut tl = cstr(getopt::OPTARG);
                if tl == "@0@" {
                    tl = internal_epsilon.to_string();
                }
                TO_PAIR = label_to_stringpair(&tl);
                if tl.is_empty() {
                    hfst_error(
                        libc::EXIT_FAILURE,
                        0,
                        &format!(
                            "argument of target label option is empty;\n\
                             if you want to substitute something with epsilons, use @0@ or {}",
                            internal_epsilon
                        ),
                    );
                }
                TO_LABEL = Some(tl);
            } else if c == b'T' as c_int {
                let fname = cstr(getopt::OPTARG);
                let f = hfst_fopen(&fname, "r");
                TO_TRANSDUCER_FILENAME = Some(fname);
                if f.is_null() {
                    return libc::EXIT_FAILURE;
                }
                libc::fclose(f);
            } else if c == b'R' as c_int {
                IN_ORDER = true;
            } else if c == b'9' as c_int {
                COMPOSE = true;
            } else if c == b'C' as c_int {
                ALLOW_TRANSDUCER_CONVERSION = false;
            } else {
                return handle_error_case(c);
            }
        }

        if (*(&raw const FROM_LABEL)).is_none() && (*(&raw const FROM_FILE_NAME)).is_none() {
            hfst_error(
                libc::EXIT_FAILURE,
                0,
                "Must state name of labels to rewrite with -f or -F",
            );
            return libc::EXIT_FAILURE;
        }
        if (*(&raw const TO_LABEL)).is_none()
            && (*(&raw const TO_TRANSDUCER_FILENAME)).is_none()
            && (*(&raw const FROM_FILE_NAME)).is_none()
        {
            hfst_error(
                libc::EXIT_FAILURE,
                0,
                "Must give target labels with -t, -T or -F",
            );
            return libc::EXIT_FAILURE;
        }
        check_common_params();
        check_unary_params(argc, argv);
        EXIT_CONTINUE
    }
}

// Resolves the display name of the replacement transducer: its stored name, or
// the '-T' filename when unnamed.
unsafe fn to_transducer_name() -> String {
    unsafe {
        let n = (*(&raw const TO_TRANSDUCER)).as_ref().unwrap().get_name();
        if n.is_empty() {
            (*(&raw const TO_TRANSDUCER_FILENAME)).clone().unwrap()
        } else {
            n
        }
    }
}

// The 'HfstBasicTransducer&' overload of 'do_substitute' (used by the
// internal-format fallback).
unsafe fn do_substitute_basic(trans: &mut HfstBasicTransducer, transducer_n: usize) {
    unsafe {
        let from_pair = (*(&raw const FROM_PAIR)).clone();
        let to_pair = (*(&raw const TO_PAIR)).clone();
        let from_label = (*(&raw const FROM_LABEL)).clone();
        let to_label = (*(&raw const TO_LABEL)).clone();
        let has_to_transducer = (*(&raw const TO_TRANSDUCER)).is_some();
        if let (Some(fp), Some(tp)) = (&from_pair, &to_pair) {
            // (Both branches of the C 'transducer_n' test print the same text.)
            verbose_printf(&format!(
                "Substituting pair {}:{} with pair {}:{}...\n",
                fp.0, fp.1, tp.0, tp.1
            ));
            trans.substitute_symbol_pair(fp, tp);
        } else if let (Some(fl), Some(tl)) = (&from_label, &to_label) {
            if transducer_n < 2 {
                verbose_printf(&format!("Substituting label {} with label {}...\n", fl, tl));
            } else {
                verbose_printf(&format!(
                    "Substituting label {} with label {}... {}\n",
                    fl, tl, transducer_n
                ));
            }
            trans.substitute_symbol(fl, tl, true, true);
        } else if let (Some(fp), true) = (&from_pair, has_to_transducer) {
            let to_name = to_transducer_name();
            if transducer_n < 2 {
                verbose_printf(&format!(
                    "Substituting pair {}:{} with transducer {}...\n",
                    fp.0, fp.1, to_name
                ));
            } else {
                verbose_printf(&format!(
                    "Substituting pair {}:{} with transducer {}... {}\n",
                    fp.0, fp.1, to_name, transducer_n
                ));
            }
            let graph = HfstBasicTransducer::from_transducer(
                (*(&raw const TO_TRANSDUCER)).as_ref().unwrap(),
            );
            trans.substitute_symbol_pair_with_transducer(fp, &graph);
        } else if let (Some(fl), true) = (&from_label, has_to_transducer) {
            let to_name = to_transducer_name();
            if transducer_n < 2 {
                verbose_printf(&format!(
                    "Substituting id. label {} with transducer {}...\n",
                    fl, to_name
                ));
            } else {
                verbose_printf(&format!(
                    "Substituting id. label {} with transducer {}... {}\n",
                    fl, to_name, transducer_n
                ));
            }
            // [spec:hfst:def:hfst-substitute.from-arc-fn]
            // [spec:hfst:sem:hfst-substitute.from-arc-fn]
            let from_arc: StringPair = (fl.clone(), fl.clone());
            let graph = HfstBasicTransducer::from_transducer(
                (*(&raw const TO_TRANSDUCER)).as_ref().unwrap(),
            );
            trans.substitute_symbol_pair_with_transducer(&from_arc, &graph);
        }
    }
}

// The 'HfstTransducer&' overload of 'do_substitute'.
unsafe fn do_substitute(trans: &mut HfstTransducer, transducer_n: usize) {
    unsafe {
        let from_pair = (*(&raw const FROM_PAIR)).clone();
        let to_pair = (*(&raw const TO_PAIR)).clone();
        let from_label = (*(&raw const FROM_LABEL)).clone();
        let to_label = (*(&raw const TO_LABEL)).clone();
        let has_to_transducer = (*(&raw const TO_TRANSDUCER)).is_some();
        if let (Some(fp), Some(tp)) = (&from_pair, &to_pair) {
            verbose_printf(&format!(
                "Substituting pair {}:{} with pair {}:{}...\n",
                fp.0, fp.1, tp.0, tp.1
            ));
            trans.substitute_symbol_pair(fp, tp);
        } else if let (Some(fl), Some(tl)) = (&from_label, &to_label) {
            if COMPOSE {
                if transducer_n < 2 {
                    verbose_printf(&format!(
                        "Delaying substitution of label {} with label {}...\n",
                        fl, tl
                    ));
                } else {
                    verbose_printf(&format!(
                        "Delaying substitution of label {} with label {}... {}\n",
                        fl, tl, transducer_n
                    ));
                }
                let substitution = HfstTransducer::new_symbol_pair(fl, tl, trans.get_type());
                (*(&raw mut SUBSTITUTION_TRANS))
                    .as_mut()
                    .unwrap()
                    .disjunct(&substitution, true);
                DELAYED = true;
            } else {
                if transducer_n < 2 {
                    verbose_printf(&format!("Substituting label {} with label {}...\n", fl, tl));
                } else {
                    verbose_printf(&format!(
                        "Substituting label {} with label {}... {}\n",
                        fl, tl, transducer_n
                    ));
                }
                trans.substitute(fl, tl, true, true);
            }
        } else if let (Some(fp), true) = (&from_pair, has_to_transducer) {
            let to_name = to_transducer_name();
            if transducer_n < 2 {
                verbose_printf(&format!(
                    "Substituting pair {}:{} with transducer {}...\n",
                    fp.0, fp.1, to_name
                ));
            } else {
                verbose_printf(&format!(
                    "Substituting pair {}:{} with transducer {}... {}\n",
                    fp.0, fp.1, to_name, transducer_n
                ));
            }
            let to_t = (*(&raw mut TO_TRANSDUCER)).as_mut().unwrap();
            trans.substitute_symbol_pair_with_transducer(fp, to_t, true);
        } else if let (Some(fl), true) = (&from_label, has_to_transducer) {
            let to_name = to_transducer_name();
            if transducer_n < 2 {
                verbose_printf(&format!(
                    "Substituting id. label {} with transducer {}...\n",
                    fl, to_name
                ));
            } else {
                verbose_printf(&format!(
                    "Substituting id. label {} with transducer {}... {}\n",
                    fl, to_name, transducer_n
                ));
            }
            let from_arc: StringPair = (fl.clone(), fl.clone());
            let to_t = (*(&raw mut TO_TRANSDUCER)).as_mut().unwrap();
            trans.substitute_symbol_pair_with_transducer(&from_arc, to_t, true);
        }
    }
}

// [spec:hfst:def:hfst-substitute.perform-delayed-fn]
// [spec:hfst:sem:hfst-substitute.perform-delayed-fn]
unsafe fn perform_delayed(trans: &mut HfstTransducer) {
    unsafe {
        verbose_printf("Finalising substitution transducer...\n");
        let mut sigma_minus_subs =
            HfstTransducer::new_symbol_pair(internal_identity, internal_identity, trans.get_type());
        let mut subs_in = (*(&raw const SUBSTITUTION_TRANS)).as_ref().unwrap().clone();
        subs_in.input_project();
        sigma_minus_subs.subtract(&subs_in, true);
        (*(&raw mut SUBSTITUTION_TRANS))
            .as_mut()
            .unwrap()
            .disjunct(&sigma_minus_subs, true);
        (*(&raw mut SUBSTITUTION_TRANS))
            .as_mut()
            .unwrap()
            .repeat_star();
        verbose_printf("Composing delayed substitutions on right...\n");
        trans.compose((*(&raw const SUBSTITUTION_TRANS)).as_ref().unwrap(), true);
        verbose_printf("Minimising...\n");
        trans.minimize();
        (*(&raw mut SUBSTITUTION_TRANS)).as_mut().unwrap().invert();
        verbose_printf("Composing delayed substitutions on left...\n");
        // C: 'trans = substitution_trans->compose(trans)' — composes in place on
        // the substitution net, then copies the result into 'trans'.
        (*(&raw mut SUBSTITUTION_TRANS))
            .as_mut()
            .unwrap()
            .compose(&*trans, true);
        *trans = (*(&raw const SUBSTITUTION_TRANS)).as_ref().unwrap().clone();
        verbose_printf("Minimising...\n");
        trans.minimize();
    }
}

// [spec:hfst:def:hfst-substitute.process-stream-fn]
// [spec:hfst:sem:hfst-substitute.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream) -> c_int {
    unsafe {
        let mut symbol_pair_map_in_use = false;
        let mut symbol_map_in_use = false;

        let mut transducer_n: usize = 0;

        let mut output_type = ImplementationType::UNSPECIFIED_TYPE;

        if (*(&raw const TO_TRANSDUCER_FILENAME)).is_some() {
            let to_fname = (*(&raw const TO_TRANSDUCER_FILENAME)).clone().unwrap();
            // (C wraps the ctor in try/catch on NotTransducerStreamException; the
            // Rust ctor panics on a bad file rather than throwing.)
            let mut tostream = HfstInputStream::new_filename(&to_fname);
            TO_TRANSDUCER = Some(HfstTransducer::new_from_stream(&mut tostream));
            tostream.close();
            let to_transducer_type = (*(&raw const TO_TRANSDUCER)).as_ref().unwrap().get_type();
            let instream_type = instream.get_type();
            if to_transducer_type != instream_type {
                if ALLOW_TRANSDUCER_CONVERSION {
                    let ct = conversion_type(instream_type, to_transducer_type);
                    let mut warnstr = format!(
                        "Transducer type mismatch in {} and {}; ",
                        cstr(globals::INPUTFILENAME),
                        to_fname
                    );
                    if ct == 1 {
                        warnstr.push_str("using former type as output");
                        output_type = instream_type;
                    } else if ct == 2 {
                        warnstr.push_str("using latter type as output");
                        output_type = to_transducer_type;
                    } else if ct == -1 {
                        warnstr.push_str(
                            "using former type as output, loss of information is possible",
                        );
                        output_type = instream_type;
                    } else {
                        /* should not happen */
                        std::panic::panic_any(String::from(
                            "Error: hfst-disjunct: conversion_type returned an invalid integer",
                        ));
                    }
                    hfst_warning(0, 0, &warnstr);
                    (*(&raw mut TO_TRANSDUCER))
                        .as_mut()
                        .unwrap()
                        .convert(output_type, String::new());
                } else {
                    hfst_error(
                        libc::EXIT_FAILURE,
                        0,
                        &format!(
                            "Transducer type mismatch in {} and {}; \
                             formats {} and {} are not compatible for substitution \
                             (--do-not-convert was requested)",
                            cstr(globals::INPUTFILENAME),
                            to_fname,
                            hfst_strformat(instream_type),
                            hfst_strformat(to_transducer_type)
                        ),
                    );
                }
            } else {
                output_type = instream.get_type();
            }
        } else {
            output_type = instream.get_type();
        }

        let output_named = !globals::OUTFILE.is_null();
        let mut outstream = if output_named {
            HfstOutputStream::new_filename(&cstr(globals::OUTFILENAME), output_type, true)
        } else {
            HfstOutputStream::new(output_type, true)
        };

        let mut fallback: Option<HfstBasicTransducer> = None;
        let mut warned_already = false;
        // NOTE: as in the C source, 'fellback' is not reset between transducers.
        let mut fellback = false;
        while instream.is_good() {
            transducer_n += 1;
            let mut trans = HfstTransducer::new_from_stream(instream);
            let inputname = {
                let n = trans.get_name();
                if n.is_empty() {
                    cstr(globals::INPUTFILENAME)
                } else {
                    n
                }
            };
            if transducer_n == 1 {
                verbose_printf(&format!("performing substitutions in {}...\n", inputname));
            } else {
                verbose_printf(&format!(
                    "performing substitutions in {}... {}\n",
                    inputname, transducer_n
                ));
            }
            // initialize delayed substitutor automaton
            SUBSTITUTION_TRANS = Some(HfstTransducer::new_type(trans.get_type()));
            if !FROM_FILE.is_null() {
                let from_file_name = (*(&raw const FROM_FILE_NAME)).clone().unwrap();
                let mut line_n: u32 = 0;
                verbose_printf(&format!(
                    "reading substitutions from {}...\n",
                    from_file_name
                ));
                while let Some(line) = read_line(FROM_FILE) {
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
                                    libc::EXIT_FAILURE,
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
                    FROM_PAIR = label_to_stringpair(&fl);
                    TO_PAIR = label_to_stringpair(&tl);
                    if fl.is_empty() {
                        hfst_error_at_line(
                            libc::EXIT_FAILURE,
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
                            libc::EXIT_FAILURE,
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
                    FROM_LABEL = Some(fl.clone());
                    TO_LABEL = Some(tl.clone());

                    if (*(&raw const FROM_PAIR)).is_some() && (*(&raw const TO_PAIR)).is_some() {
                        if !IN_ORDER {
                            (*(&raw mut PAIR_SUBSTITUTION_MAP))
                                .as_mut()
                                .unwrap()
                                .insert(
                                    (*(&raw const FROM_PAIR)).clone().unwrap(),
                                    (*(&raw const TO_PAIR)).clone().unwrap(),
                                );
                            symbol_pair_map_in_use = true;
                        } else {
                            do_substitute(&mut trans, transducer_n);
                        }
                    } else if !fl.is_empty() && !tl.is_empty() {
                        if !IN_ORDER {
                            (*(&raw mut LABEL_SUBSTITUTION_MAP))
                                .as_mut()
                                .unwrap()
                                .insert(fl.clone(), tl.clone());
                            symbol_map_in_use = true;
                        } else {
                            do_substitute(&mut trans, transducer_n);
                        }
                    } else {
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            unsafe { do_substitute(&mut trans, transducer_n) };
                        }));
                        if let Err(payload) = result {
                            if payload
                                .downcast_ref::<FunctionNotImplementedException>()
                                .is_some()
                            {
                                if !warned_already {
                                    if !globals::SILENT {
                                        hfst_warning(
                                            0,
                                            0,
                                            "substitution is not supported for this transducer type \
                                             falling back to internal formats and trying...",
                                        );
                                    }
                                    fallback = Some(HfstBasicTransducer::from_transducer(&trans));
                                    warned_already = true;
                                }
                                do_substitute_basic(
                                    (*(&raw mut fallback)).as_mut().unwrap(),
                                    transducer_n,
                                );
                                fellback = true;
                            } else {
                                std::panic::resume_unwind(payload);
                            }
                        }
                    }
                } // while getline

                // perform label-to-label substitution right away
                if !IN_ORDER && symbol_map_in_use {
                    trans.substitute_substitutions(
                        (*(&raw const LABEL_SUBSTITUTION_MAP)).as_ref().unwrap(),
                    );
                    symbol_map_in_use = false;
                }

                // perform symbol pair-to-symbol pair substitution right away
                if !IN_ORDER && symbol_pair_map_in_use {
                    trans.substitute_symbol_pairs(
                        (*(&raw const PAIR_SUBSTITUTION_MAP)).as_ref().unwrap(),
                    );
                    symbol_pair_map_in_use = false;
                }
            }
            // if not from file
            else {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    unsafe { do_substitute(&mut trans, transducer_n) };
                }));
                if let Err(payload) = result {
                    if payload
                        .downcast_ref::<FunctionNotImplementedException>()
                        .is_some()
                    {
                        if !warned_already {
                            if !globals::SILENT {
                                hfst_warning(
                                    0,
                                    0,
                                    "substitution is not supported for this transducer type \
                                     falling back to internal formats and trying...",
                                );
                            }
                            fallback = Some(HfstBasicTransducer::from_transducer(&trans));
                        }
                        do_substitute_basic((*(&raw mut fallback)).as_mut().unwrap(), transducer_n);
                        fellback = true;
                    } else {
                        std::panic::resume_unwind(payload);
                    }
                }
            }
            if fellback {
                let ty = trans.get_type();
                trans =
                    HfstTransducer::new_from_basic((*(&raw const fallback)).as_ref().unwrap(), ty);
            } else if DELAYED {
                perform_delayed(&mut trans);
            }
            if !FROM_FILE.is_null() {
                let from_file_name = (*(&raw const FROM_FILE_NAME)).clone().unwrap();
                let src = trans.clone();
                hfst_set_name_unary(
                    &mut trans,
                    &src,
                    &format!("substitute-from-{}", from_file_name),
                );
                let src = trans.clone();
                hfst_set_formula_unary(&mut trans, &src, &format!("♲{}", from_file_name));
            } else if (*(&raw const FROM_LABEL)).is_some() && (*(&raw const TO_LABEL)).is_some() {
                let fl = (*(&raw const FROM_LABEL)).clone().unwrap();
                let tl = (*(&raw const TO_LABEL)).clone().unwrap();
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, &format!("substitute-{}-with-{}", fl, tl));
                let src = trans.clone();
                hfst_set_formula_unary(&mut trans, &src, &format!("{} ♲ {}", fl, tl));
            } else if (*(&raw const TO_TRANSDUCER_FILENAME)).is_some() {
                if (*(&raw const FROM_LABEL)).is_none() {
                    // make scan-build happy, this should not happen
                    std::panic::panic_any(String::from("Error: from_label has a NULL value."));
                }
                let fl = (*(&raw const FROM_LABEL)).clone().unwrap();
                let tf = (*(&raw const TO_TRANSDUCER_FILENAME)).clone().unwrap();
                let src = trans.clone();
                hfst_set_name_unary(
                    &mut trans,
                    &src,
                    &format!("substitute-{}-with-net-{}", fl, tf),
                );
                let src = trans.clone();
                hfst_set_formula_unary(&mut trans, &src, &format!("{} ♲ {}", fl, tf));
            }
            // delete fallback
            fallback = None;
            outstream.redirect(&mut trans);
        }
        // delete to_transducer
        TO_TRANSDUCER = None;
        instream.close();
        outstream.close();
        libc::EXIT_SUCCESS
    }
}

// [spec:hfst:def:hfst-substitute.main-fn]
// [spec:hfst:sem:hfst-substitute.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> c_int {
    unsafe {
        // Build a C-style argv (NULL-terminated) from the Rust args; getopt and
        // extend_options_getenv reorder/replace it in place.
        let c_args: Vec<CString> = std::env::args()
            .map(|a| CString::new(a).unwrap_or_default())
            .collect();
        let mut argv_vec: Vec<*mut c_char> =
            c_args.iter().map(|s| s.as_ptr() as *mut c_char).collect();
        argv_vec.push(std::ptr::null_mut());
        let argc: c_int = c_args.len() as c_int;
        let argv: *mut *mut c_char = argv_vec.as_mut_ptr();
        let argv0 = cstr(*argv);

        hfst_set_program_name(&argv0, "0.1", "HfstSubstitute");
        let retval = parse_options(argc, argv);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let input_opened = !globals::INPUTFILE.is_null();
        let output_opened = !globals::OUTFILE.is_null();
        if input_opened {
            libc::fclose(globals::INPUTFILE);
        }
        if output_opened {
            libc::fclose(globals::OUTFILE);
        }
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            cstr(globals::INPUTFILENAME),
            cstr(globals::OUTFILENAME)
        ));

        if !FROM_FILE.is_null() {
            LABEL_SUBSTITUTION_MAP = Some(HfstSymbolSubstitutions::new());
            PAIR_SUBSTITUTION_MAP = Some(HfstSymbolPairSubstitutions::new());
        }

        // here starts the buffer handling part
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing.)
        let mut instream = if input_opened {
            HfstInputStream::new_filename(&cstr(globals::INPUTFILENAME))
        } else {
            HfstInputStream::new()
        };

        if is_input_stream_in_ol_format(&instream, "hfst-substitute") {
            return libc::EXIT_FAILURE;
        }

        process_stream(&mut instream)
    }
}
