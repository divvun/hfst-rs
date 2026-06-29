#![allow(static_mut_refs)]
//! Faithful 1:1 port of tools/src/hfst-kill-paths.cc — the path-killing
//! command-line tool: removes every arc whose input or output symbol matches a
//! given symbol (one --symbol, or a list from a --tsv-file), then removes
//! epsilons. Drives the hfst-cli foundation (globals, getopt, commandline,
//! program-options, tool-metadata, inc fragments).

use core::ffi::{c_char, c_int};
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
    HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT, hfst_getopt_common_long,
    hfst_getopt_unary_long, print_common_program_options, print_common_unary_program_options,
    print_common_unary_program_parameter_instructions,
};
use hfst_cli::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary, hfst_set_name_unary};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use std::ffi::{CStr, CString};
use std::io::{BufRead, BufReader, Seek, SeekFrom};

// add tools-specific variables here
static mut SYMBOL: Option<String> = None;
static mut TSV_FILE_NAME: Option<String> = None;
static mut TSV_FILE: Option<std::fs::File> = None;

unsafe fn cstr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

fn fput(f: &mut dyn std::io::Write, s: &str) {
    let _ = f.write_all(s.as_bytes());
}

// [spec:hfst:def:hfst-kill-paths.print-usage-fn]
// [spec:hfst:sem:hfst-kill-paths.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        let mut msg = globals::message_writer();
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        // Usage line
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            &mut *msg,
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\nKill all paths with specific symbols\n\n",
                program_name
            ),
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        fput(
            &mut *msg,
            "Reweighting options:\n  -S, --symbol=SYM           remove arcs with input or output symbol SYM or both\n  -T, --tsv-file=TFILE       read kill rules from TFILE\n\n",
        );
        fput(&mut *msg, "\n");
        print_common_unary_program_parameter_instructions(&mut *msg);
        fput(
            &mut *msg,
            "TFILE should contain lines with tab-separated pairs of SYM and Comment lines starting with # and empty lines are ignored.\n",
        );
        fput(&mut *msg, "\n");
        print_report_bugs();
        fput(&mut *msg, "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-kill-paths.parse-options-fn]
// [spec:hfst:sem:hfst-kill-paths.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let symbol_name = CString::new("symbol").unwrap();
            let tsv_name = CString::new("tsv").unwrap();
            long_options.push(getopt::Option {
                name: symbol_name.as_ptr(),
                has_arg: getopt::REQUIRED_ARGUMENT,
                flag: std::ptr::null_mut(),
                val: 'S' as c_int,
            });
            long_options.push(getopt::Option {
                name: tsv_name.as_ptr(),
                has_arg: getopt::REQUIRED_ARGUMENT,
                flag: std::ptr::null_mut(),
                val: 'T' as c_int,
            });
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}{}",
                HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT, "a:b:F:l:u:I:O:S:eT:A"
            ))
            .unwrap();
            let mut option_index: c_int = 0;
            // add tool-specific options here
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
            // cases, then unary cases, then the tool's own ('S'/'T'), then the
            // terminal error arm.
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
            if c == 'S' as c_int {
                SYMBOL = Some(cstr(getopt::OPTARG));
                continue;
            }
            if c == 'T' as c_int {
                TSV_FILE_NAME = Some(cstr(getopt::OPTARG));
                continue;
            }
            return handle_error_case(c);
        }

        if SYMBOL.is_none() && TSV_FILE_NAME.is_none() {
            error(1, 0, "Either --symbol or --tsv-file is required");
            return 1;
        }

        check_common_params();
        check_unary_params(argc, argv);
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
unsafe fn process_stream(
    instream: &mut HfstInputStream,
    outstream: &mut HfstOutputStream,
) -> c_int {
    unsafe {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let mut trans = HfstTransducer::new_from_stream(instream);
            let inputname = hfst_get_name(&trans, &cstr(globals::INPUTFILENAME));
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
            outstream.redirect(trans.remove_epsilons());
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

        hfst_set_program_name(&argv0, "0.1", "HfstKillPaths");
        let retval = parse_options(argc, argv);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let input_opened = cstr(globals::INPUTFILENAME) != "<stdin>";
        let output_opened = cstr(globals::OUTFILENAME) != "<stdout>";
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            cstr(globals::INPUTFILENAME),
            cstr(globals::OUTFILENAME)
        ));
        verbose_printf("Killing paths\n");
        if let Some(sym) = &SYMBOL {
            verbose_printf(&format!("only if arc has symbol {}\n", sym));
        }

        // here starts the buffer handling part
        let mut instream = if input_opened {
            HfstInputStream::new_filename(&cstr(globals::INPUTFILENAME))
        } else {
            HfstInputStream::new()
        };
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)

        let type_ = instream.get_type();
        let mut outstream = if output_opened {
            HfstOutputStream::new_filename(&cstr(globals::OUTFILENAME), type_, true)
        } else {
            HfstOutputStream::new(type_, true)
        };

        if is_input_stream_in_ol_format(&instream, "hfst-kill-paths") {
            return 1;
        }

        process_stream(&mut instream, &mut outstream)
    }
}
