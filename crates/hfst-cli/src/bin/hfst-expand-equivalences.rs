//! Faithful 1:1 port of tools/src/hfst-expand-equivalences.cc — the transducer
//! label modification tool for equivalence classes. Drives the hfst-cli
//! foundation (globals, getopt, commandline, program-options, tool-metadata,
//! inc fragments).

use core::ffi::{c_char, c_int};
use hfst::expand_equivalences::{
    FsaLevel, TsvExtensionError, expand_equivalences, read_tsv_extensions,
};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, error_at_line, extend_options_getenv, hfst_set_program_name, hfst_strdup,
    is_input_stream_in_ol_format, print_more_info, print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT, hfst_getopt_common_long,
    hfst_getopt_unary_long, print_common_program_options,
};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
};
use std::ffi::{CStr, CString};

// Tool-specific static-mut option state, mirroring the C++ file-scope statics.
static mut ONLY_FROM_LABEL: *mut c_char = std::ptr::null_mut();
static mut ONLY_TO_LABEL: *mut c_char = std::ptr::null_mut();
static mut ACX_FILE_NAME: *mut c_char = std::ptr::null_mut();
// C: ACX_FILE was a 'FILE*' opened by hfst_fopen and only ever tested for
// non-null (the libxml ACX-parsing body compiles to nothing without libxml).
// Here it is just an "opened" flag.
static mut ACX_FILE_OPENED: bool = false;
static mut TSV_FILE_NAME: *mut c_char = std::ptr::null_mut();

// FsaLevel, the TSV reader, and the extension/compose loop now live in
// hfst::expand_equivalences; this tool keeps only the option-driven LEVEL global.
// The TSV file is opened (as a std stream) and parsed in process_stream, so no
// libc TSV handle is held here.
static mut LEVEL: FsaLevel = FsaLevel::First;

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

// [spec:hfst:def:hfst-expand-equivalences.print-usage-fn]
// [spec:hfst:sem:hfst-expand-equivalences.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        let mut msg = globals::message_writer();
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            &mut *msg,
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\nExtend transducer arcs for equivalence classes\n\n",
                program_name
            ),
        );
        print_common_program_options(&mut *msg);
        fput(
            &mut *msg,
            "Eqv. class extension options:\n\
             \x20 -f, --from=ISYM     convert single symbol ISYM to allow OSYM\n\
             \x20 -t, --to=OSYM       convert to OSYM\n\
             \x20 -a, --acx=ACXFILE   read extensions in acx format from ACXFILE\n\
             \x20 -T, --tsv=TSVFILE   read extensions in tsv format from TSVFILE\n\
             \x20 -l, --level=LEVEL   perform extensions on LEVEL of fsa\n",
        );
        fput(&mut *msg, "\n");
        fput(
            &mut *msg,
            "Either ACXFILE, TSVFILE or both ISYM and OSYM must be specified.\n\
             LEVEL should be either {upper, first, 1, input, surface}, \
             {lower, second, 2, output, analysis} or both.\n\
             If LEVEL is omitted, default is first.\n",
        );
        fput(
            &mut *msg,
            &format!(
                "Examples:\n\
                 \x20 {} -o rox.hfst -a romanian.acx ro.hfst  extend romanian char\
                 equivalences\n\n",
                program_name
            ),
        );
        print_report_bugs();
        print_more_info();
    }
}

// [spec:hfst:def:hfst-expand-equivalences.parse-options-fn]
// [spec:hfst:sem:hfst-expand-equivalences.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let from_name = CString::new("from").unwrap();
            let to_name = CString::new("to").unwrap();
            let acx_name = CString::new("acx").unwrap();
            let tsv_name = CString::new("tsv").unwrap();
            let level_name = CString::new("level").unwrap();
            long_options.push(getopt::Option {
                name: from_name.as_ptr(),
                has_arg: 1, // required_argument
                flag: std::ptr::null_mut(),
                val: b'f' as c_int,
            });
            long_options.push(getopt::Option {
                name: to_name.as_ptr(),
                has_arg: 1,
                flag: std::ptr::null_mut(),
                val: b't' as c_int,
            });
            long_options.push(getopt::Option {
                name: acx_name.as_ptr(),
                has_arg: 1,
                flag: std::ptr::null_mut(),
                val: b'a' as c_int,
            });
            long_options.push(getopt::Option {
                name: tsv_name.as_ptr(),
                has_arg: 1,
                flag: std::ptr::null_mut(),
                val: b'T' as c_int,
            });
            long_options.push(getopt::Option {
                name: level_name.as_ptr(),
                has_arg: 1,
                flag: std::ptr::null_mut(),
                val: b'l' as c_int,
            });
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}f:t:a:T:l:",
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

            // The C switch chains the #include'd common cases, then the tool's
            // own cases, then the terminal error arm.
            match handle_common_case(c, || print_usage()) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match c as u8 {
                b'f' => {
                    ONLY_FROM_LABEL = hfst_strdup(getopt::OPTARG);
                    continue;
                }
                b't' => {
                    ONLY_TO_LABEL = hfst_strdup(getopt::OPTARG);
                    continue;
                }
                b'a' => {
                    ACX_FILE_NAME = hfst_strdup(getopt::OPTARG);
                    continue;
                }
                b'T' => {
                    TSV_FILE_NAME = hfst_strdup(getopt::OPTARG);
                    continue;
                }
                b'l' => {
                    let optarg = cstr(getopt::OPTARG);
                    if optarg == "first" || optarg == "upper" || optarg == "input" || optarg == "1"
                    {
                        LEVEL = FsaLevel::First;
                    } else if optarg == "second"
                        || optarg == "lower"
                        || optarg == "output"
                        || optarg == "2"
                    {
                        LEVEL = FsaLevel::Second;
                    } else if optarg == "both" {
                        LEVEL = FsaLevel::Both;
                    } else {
                        error(
                            1,
                            0,
                            "The option for level parameter must be one of:\n\
                             upper, first, input; second, lower, output; both, \
                             1 or 2.",
                        );
                    }
                    continue;
                }
                _ => {}
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(argc, argv);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-expand-equivalences.check-options-fn]
// [spec:hfst:sem:hfst-expand-equivalences.check-options-fn]
unsafe fn check_options(_argc: c_int, _argv: *mut *mut c_char) {
    unsafe {
        if (!ONLY_FROM_LABEL.is_null()) || (!ONLY_TO_LABEL.is_null()) {
            if (!TSV_FILE_NAME.is_null()) || (!ACX_FILE_NAME.is_null()) {
                error(1, 0, "Only one of -a, -T or -f and -t may be given");
            } else if ONLY_FROM_LABEL.is_null() {
                error(1, 0, "option -t requires -f");
            } else if ONLY_TO_LABEL.is_null() {
                error(1, 0, "option -f requires -t");
            }
        } else if TSV_FILE_NAME.is_null() && ACX_FILE_NAME.is_null() {
            error(
                1,
                0,
                "Must give extension specification file with either -a or -t.",
            );
        } else if (!TSV_FILE_NAME.is_null()) && (!ACX_FILE_NAME.is_null()) {
            error(1, 0, "Only one of parameters -a, -t, must be used.");
        } else if !TSV_FILE_NAME.is_null() {
            // TSV is opened as a std stream and parsed in process_stream via
            // read_tsv_extensions; no libc handle is opened here. A missing file
            // is reported there (slightly later than the C++, which fopen'd it at
            // this point) with the same fatal error.
        } else if !ACX_FILE_NAME.is_null() {
            let name = cstr(ACX_FILE_NAME);
            match std::fs::File::open(&name) {
                Ok(_f) => ACX_FILE_OPENED = true,
                Err(_) => {
                    error(1, 0, &format!("Could not open '{}'", name));
                }
            }
        } else {
            error(1, 0, "Logic error again!");
        }
    }
}

// [spec:hfst:def:hfst-expand-equivalences.process-stream-fn]
// [spec:hfst:sem:hfst-expand-equivalences.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream, outstream: &mut HfstOutputStream) {
    unsafe {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let _ = transducer_n; // C++ counts but never reads it
            let trans = HfstTransducer::new_from_stream(instream);

            // Collect the (from, to) extension pairs from whichever source the
            // options selected. The TSV parser and the extension/compose loop now
            // live in hfst::expand_equivalences; the per-extension "extending X by
            // Y" and "Applying extensions on N level" -v traces were diagnostic and
            // are not reproduced.
            let mut pairs: Vec<(String, String)> = Vec::new();
            if !ONLY_FROM_LABEL.is_null() {
                verbose_printf(&format!(
                    "using single commandline extension {} with {}\n",
                    cstr(ONLY_FROM_LABEL),
                    cstr(ONLY_TO_LABEL)
                ));
                pairs.push((cstr(ONLY_FROM_LABEL), cstr(ONLY_TO_LABEL)));
            } else if !TSV_FILE_NAME.is_null() {
                let tsv_name = cstr(TSV_FILE_NAME);
                verbose_printf(&format!("reading extensions from {}...\n", tsv_name));
                let file = match std::fs::File::open(&tsv_name) {
                    Ok(f) => f,
                    Err(e) => {
                        error(1, 0, &format!("cannot open {}: {}", tsv_name, e));
                        return;
                    }
                };
                match read_tsv_extensions(std::io::BufReader::new(file)) {
                    Ok(p) => pairs = p,
                    Err(TsvExtensionError { line, message }) => {
                        error_at_line(1, 0, &tsv_name, line, &message);
                        return;
                    }
                }
            } else if ACX_FILE_OPENED {
                verbose_printf(&format!("Reading ACX from {}...\n", cstr(ACX_FILE_NAME)));
                // The libxml ACX-parsing body is gated behind #if HAVE_LIBXML_TREE_H
                // in the C++ source; without libxml it compiles to nothing, which
                // is the path reproduced here (no extensions added).
            } else {
                error(1, 0, "DANGER TERROR HORROR !!!!!!");
                return;
            }

            let mut trans = expand_equivalences(trans, &pairs, LEVEL);
            outstream.redirect(&mut trans);
        } // for each automaton
    }
}

// [spec:hfst:def:hfst-expand-equivalences.main-fn]
// [spec:hfst:sem:hfst-expand-equivalences.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> c_int {
    unsafe {
        let c_args: Vec<CString> = std::env::args()
            .map(|a| CString::new(a).unwrap_or_default())
            .collect();
        let mut argv_vec: Vec<*mut c_char> =
            c_args.iter().map(|s| s.as_ptr() as *mut c_char).collect();
        argv_vec.push(std::ptr::null_mut());
        let argc: c_int = c_args.len() as c_int;
        let argv: *mut *mut c_char = argv_vec.as_mut_ptr();
        let argv0 = cstr(*argv);

        hfst_set_program_name(&argv0, "0.1", "HfstExpandEquivalences");
        let retval = parse_options(argc, argv);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        check_options(argc, argv);

        // close buffers, we use streams
        let input_opened = cstr(globals::INPUTFILENAME) != "<stdin>";
        let output_opened = cstr(globals::OUTFILENAME) != "<stdout>";
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            cstr(globals::INPUTFILENAME),
            cstr(globals::OUTFILENAME)
        ));

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

        if is_input_stream_in_ol_format(&instream, "hfst-expand-equivalences") {
            return 1;
        }

        process_stream(&mut instream, &mut outstream);
        instream.close();
        outstream.close();
        0
    }
}
