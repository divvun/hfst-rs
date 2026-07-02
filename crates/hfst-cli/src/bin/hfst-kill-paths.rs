#![allow(static_mut_refs)]
//! Faithful 1:1 port of tools/src/hfst-kill-paths.cc — the path-killing
//! command-line tool: removes every arc whose input or output symbol matches a
//! given symbol (one --symbol, or a list from a --tsv-file), then removes
//! epsilons. Drives the hfst-cli foundation (globals, getopt, commandline,
//! program-options, tool-metadata, inc fragments).

use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_getenv, hfst_set_program_name,
    is_input_stream_in_ol_format, print_more_info, print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use hfst_cli::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary, hfst_set_name_unary};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};

// add tools-specific variables here
static mut SYMBOL: Option<String> = None;
static mut TSV_FILE_NAME: Option<String> = None;
static mut TSV_FILE: Option<std::fs::File> = None;

// [spec:hfst:def:hfst-kill-paths.print-usage-fn]
// [spec:hfst:sem:hfst-kill-paths.print-usage-fn]
fn print_usage() {
    let mut msg = globals::message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    // Usage line
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nKill all paths with specific symbols\n\n",
        globals::program_name()
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Reweighting options:\n  -S, --symbol=SYM           remove arcs with input or output symbol SYM or both\n  -T, --tsv-file=TFILE       read kill rules from TFILE\n\n"
    );
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(
        msg,
        "TFILE should contain lines with tab-separated pairs of SYM and Comment lines starting with # and empty lines are ignored.\n"
    );
    let _ = write!(msg, "\n");
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-kill-paths.parse-options-fn]
// [spec:hfst:sem:hfst-kill-paths.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_getenv(args);
        // use of this function requires options are settable on global scope
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
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own ('S'/'T'), then the
            // terminal error arm.
            match handle_common_case(c, print_usage) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_unary_case(c) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            if c == 'S' as i32 {
                SYMBOL = Some(getopt::optarg());
                continue;
            }
            if c == 'T' as i32 {
                TSV_FILE_NAME = Some(getopt::optarg());
                continue;
            }
            return handle_error_case(c);
        }

        if SYMBOL.is_none() && TSV_FILE_NAME.is_none() {
            error(1, 0, "Either --symbol or --tsv-file is required");
            return 1;
        }

        check_common_params();
        check_unary_params(args);
        if let Some(name) = &TSV_FILE_NAME {
            match std::fs::File::open(name) {
                Ok(f) => TSV_FILE = Some(f),
                Err(_) => {
                    error(1, 0, &format!("Could not open '{}'", name));
                    return 1;
                }
            }
        }
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-kill-paths.original-fn]
// [spec:hfst:sem:hfst-kill-paths.original-fn]
unsafe fn do_killing(trans: &mut HfstTransducer) {
    unsafe {
        let symbol = SYMBOL.clone().unwrap_or_default();
        *trans = trans.kill_paths(&symbol);
    }
}

// [spec:hfst:def:hfst-kill-paths.process-stream-fn]
// [spec:hfst:sem:hfst-kill-paths.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream, outstream: &mut HfstOutputStream) -> i32 {
    unsafe {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let mut trans = match HfstTransducer::new_from_stream(instream) {
                Ok(t) => t,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            };
            let inputname = hfst_get_name(&trans, &globals::input_filename());
            if transducer_n == 1 {
                verbose_printf(&format!("Path killing {}...\n", inputname));
            } else {
                verbose_printf(&format!("Path killing {}...{}\n", inputname, transducer_n));
            }
            if TSV_FILE.is_none() {
                do_killing(&mut trans);
                // C: hfst_set_name(trans, trans, "pathkill"); dest and src are the
                // same object, which Rust cannot alias mut+const, so the read side
                // is taken from a copy (name/formula are unchanged by the copy).
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "pathkill");
                hfst_set_formula_unary(&mut trans, &src, "PK");
            } else {
                // C: rewind(tsv_file) — seek the std file back to the start.
                let tsv_file = TSV_FILE.as_mut().unwrap();
                let _ = tsv_file.seek(SeekFrom::Start(0));
                SYMBOL = None;
                let mut _linen: usize = 0;
                verbose_printf(&format!(
                    "Reading reweights from {}\n",
                    TSV_FILE_NAME.clone().unwrap_or_default()
                ));
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
                    SYMBOL = Some(sym.clone());
                    verbose_printf(&format!("Killing patsh with symbol {}\n", sym));
                    do_killing(&mut trans);
                } // getline
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "pathkill");
                hfst_set_formula_unary(&mut trans, &src, "PK");
            } // if tsv_file
            let reduced = match trans.remove_epsilons() {
                Ok(t) => t,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            };
            if let Err(e) = outstream.redirect(reduced) {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        } // foreach transducer
        instream.close();
        outstream.close();
        0
    }
}

// [spec:hfst:def:hfst-kill-paths.main-fn]
// [spec:hfst:sem:hfst-kill-paths.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstKillPaths");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let input_opened = globals::input_filename() != "<stdin>";
        let output_opened = globals::output_filename() != "<stdout>";
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            globals::input_filename(),
            globals::output_filename()
        ));
        verbose_printf("Killing paths\n");
        if let Some(sym) = &SYMBOL {
            verbose_printf(&format!("only if arc has symbol {}\n", sym));
        }

        // here starts the buffer handling part
        let mut instream = match if input_opened {
            HfstInputStream::new_filename(&globals::input_filename())
        } else {
            HfstInputStream::new()
        } {
            Ok(s) => s,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        };
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)

        let ty = instream.get_type();
        let mut outstream = match if output_opened {
            HfstOutputStream::new_filename(&globals::output_filename(), ty, true)
        } else {
            HfstOutputStream::new(ty, true)
        } {
            Ok(s) => s,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        };

        if is_input_stream_in_ol_format(&instream, "hfst-kill-paths") {
            return 1;
        }

        process_stream(&mut instream, &mut outstream)
    }
}
