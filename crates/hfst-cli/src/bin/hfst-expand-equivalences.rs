//! Faithful 1:1 port of tools/src/hfst-expand-equivalences.cc — the transducer
//! label modification tool for equivalence classes. Drives the hfst-cli
//! foundation (globals, getopt, commandline, program-options, tool-metadata,
//! inc fragments).

use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_symbol_defs::{internal_epsilon, internal_identity};
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, error_at_line, extend_options_getenv, hfst_fopen, hfst_getline,
    hfst_set_program_name, hfst_strdup, hfst_strndup, is_input_stream_in_ol_format,
    print_more_info, print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT, hfst_getopt_common_long,
    hfst_getopt_unary_long, print_common_program_options,
};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
};
use libc::{c_char, c_int};
use std::ffi::{CStr, CString};

// Tool-specific static-mut option state, mirroring the C++ file-scope statics.
static mut ONLY_FROM_LABEL: *mut c_char = std::ptr::null_mut();
static mut ONLY_TO_LABEL: *mut c_char = std::ptr::null_mut();
static mut ACX_FILE_NAME: *mut c_char = std::ptr::null_mut();
static mut ACX_FILE: *mut libc::FILE = std::ptr::null_mut();
static mut TSV_FILE_NAME: *mut c_char = std::ptr::null_mut();
static mut TSV_FILE: *mut libc::FILE = std::ptr::null_mut();

// [spec:hfst:def:hfst-expand-equivalences.fsa-level-t]
#[derive(Clone, Copy, PartialEq, Eq)]
enum FsaLevel {
    First,
    Second,
    Both,
}
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

unsafe fn fput(f: *mut libc::FILE, s: &str) {
    let c = CString::new(s).unwrap_or_default();
    unsafe { libc::fputs(c.as_ptr(), f) };
}

// [spec:hfst:def:hfst-expand-equivalences.print-usage-fn]
// [spec:hfst:sem:hfst-expand-equivalences.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            globals::message_out(),
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\nExtend transducer arcs for equivalence classes\n\n",
                program_name
            ),
        );
        print_common_program_options(globals::message_out());
        fput(
            globals::message_out(),
            "Eqv. class extension options:\n\
             \x20 -f, --from=ISYM     convert single symbol ISYM to allow OSYM\n\
             \x20 -t, --to=OSYM       convert to OSYM\n\
             \x20 -a, --acx=ACXFILE   read extensions in acx format from ACXFILE\n\
             \x20 -T, --tsv=TSVFILE   read extensions in tsv format from TSVFILE\n\
             \x20 -l, --level=LEVEL   perform extensions on LEVEL of fsa\n",
        );
        fput(globals::message_out(), "\n");
        fput(
            globals::message_out(),
            "Either ACXFILE, TSVFILE or both ISYM and OSYM must be specified.\n\
             LEVEL should be either {upper, first, 1, input, surface}, \
             {lower, second, 2, output, analysis} or both.\n\
             If LEVEL is omitted, default is first.\n",
        );
        fput(
            globals::message_out(),
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
                            libc::EXIT_FAILURE,
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
                error(
                    libc::EXIT_FAILURE,
                    0,
                    "Only one of -a, -T or -f and -t may be given",
                );
            } else if ONLY_FROM_LABEL.is_null() {
                error(libc::EXIT_FAILURE, 0, "option -t requires -f");
            } else if ONLY_TO_LABEL.is_null() {
                error(libc::EXIT_FAILURE, 0, "option -f requires -t");
            }
        } else if TSV_FILE_NAME.is_null() && ACX_FILE_NAME.is_null() {
            error(
                libc::EXIT_FAILURE,
                0,
                "Must give extension specification file with either -a or -t.",
            );
        } else if (!TSV_FILE_NAME.is_null()) && (!ACX_FILE_NAME.is_null()) {
            error(
                libc::EXIT_FAILURE,
                0,
                "Only one of parameters -a, -t, must be used.",
            );
        } else if !TSV_FILE_NAME.is_null() {
            TSV_FILE = hfst_fopen(&cstr(TSV_FILE_NAME), "r");
        } else if !ACX_FILE_NAME.is_null() {
            ACX_FILE = hfst_fopen(&cstr(ACX_FILE_NAME), "r");
        } else {
            error(libc::EXIT_FAILURE, 0, "Logic error again!");
        }
    }
}

// [spec:hfst:def:hfst-expand-equivalences.add-extension-fn]
// [spec:hfst:sem:hfst-expand-equivalences.add-extension-fn]
unsafe fn add_extension(t: &mut HfstTransducer, from: &str, to: &str) {
    unsafe {
        verbose_printf(&format!("extending {} by {}\n", from, to));
        let remap = HfstTransducer::new_symbol_pair(from, to, t.get_type());
        t.disjunct(&remap, true);
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
            let mut trans = HfstTransducer::new_from_stream(instream);
            let mut extensions = HfstTransducer::new_symbol_pair(
                internal_identity,
                internal_identity,
                trans.get_type(),
            );
            if !ONLY_FROM_LABEL.is_null() {
                verbose_printf(&format!(
                    "using single commandline extension {} with {}\n",
                    cstr(ONLY_FROM_LABEL),
                    cstr(ONLY_TO_LABEL)
                ));
                add_extension(
                    &mut extensions,
                    &cstr(ONLY_FROM_LABEL),
                    &cstr(ONLY_TO_LABEL),
                );
            } else if !TSV_FILE.is_null() {
                let mut line: *mut c_char = std::ptr::null_mut();
                let mut len: usize = 0;
                let mut line_n: usize = 0;
                verbose_printf(&format!(
                    "reading extensions from {}...\n",
                    cstr(TSV_FILE_NAME)
                ));
                while hfst_getline(&mut line, &mut len, TSV_FILE) != -1 {
                    line_n += 1;
                    if *line == b'\n' as c_char {
                        continue;
                    }
                    let tab = libc::strstr(line, c"\t".as_ptr());
                    if tab.is_null() {
                        if *line == b'#' as c_char {
                            // a comment is a line starting with # without tabs
                            continue;
                        } else {
                            error_at_line(
                                libc::EXIT_FAILURE,
                                0,
                                &cstr(TSV_FILE_NAME),
                                line_n as u32,
                                "At least one tab required per line",
                            );
                        }
                    }
                    let from_char = hfst_strndup(line, tab.offset_from(line) as usize);
                    if libc::strlen(from_char) == 0 {
                        error_at_line(
                            libc::EXIT_FAILURE,
                            0,
                            &cstr(TSV_FILE_NAME),
                            line_n as u32,
                            &format!(
                                "First field is empty;\n\
                                 if you REALLY want to extend epsilons as \
                                 equivalent, use @0@ or {}",
                                internal_epsilon
                            ),
                        );
                    }
                    let mut endstr = tab.offset(1);
                    let mut tab = libc::strstr(endstr, c"\t".as_ptr());
                    while !tab.is_null() {
                        let to_char = hfst_strndup(endstr, tab.offset_from(endstr) as usize);
                        if libc::strlen(to_char) == 0 {
                            error_at_line(
                                libc::EXIT_FAILURE,
                                0,
                                &cstr(TSV_FILE_NAME),
                                line_n as u32,
                                &format!(
                                    "Extension field seems empty;\n\
                                     if you REALLY mean something is equivalent\
                                      to epsilons, use @0@ or {}",
                                    internal_epsilon
                                ),
                            );
                        }
                        add_extension(&mut extensions, &cstr(from_char), &cstr(to_char));
                        libc::free(to_char as *mut libc::c_void);
                        endstr = tab.offset(1);
                        tab = libc::strstr(endstr, c"\t".as_ptr());
                    }
                    let tab = endstr;
                    while (*endstr != 0) && (*endstr != b'\n' as c_char) {
                        endstr = endstr.offset(1);
                    }
                    let to_char = hfst_strndup(tab, endstr.offset_from(tab) as usize);
                    if libc::strlen(to_char) == 0 {
                        error_at_line(
                            libc::EXIT_FAILURE,
                            0,
                            &cstr(TSV_FILE_NAME),
                            line_n as u32,
                            &format!(
                                "Extension field seems empty;\n\
                                 if you REALLY mean something is equivalent\
                                  to epsilons, use @0@ or {}",
                                internal_epsilon
                            ),
                        );
                    }
                    add_extension(&mut extensions, &cstr(from_char), &cstr(to_char));
                } // while getline
            } else if !ACX_FILE.is_null() {
                verbose_printf(&format!("Reading ACX from {}...\n", cstr(ACX_FILE_NAME)));
                // The libxml ACX-parsing body is gated behind #if HAVE_LIBXML_TREE_H
                // in the C++ source; without libxml it compiles to nothing, which
                // is the path reproduced here.
            } else {
                error(libc::EXIT_FAILURE, 0, "DANGER TERROR HORROR !!!!!!");
            }
            extensions.minimize().repeat_star().minimize();
            match LEVEL {
                FsaLevel::Both => {
                    verbose_printf("Applying extensions on second level\n");
                    trans.compose(&extensions, true);
                    verbose_printf("Applying extensions on first level\n");
                    // trans = extensions->invert().compose(trans);
                    extensions.invert().compose(&trans, true);
                    trans = extensions.clone();
                }
                FsaLevel::First => {
                    verbose_printf("Applying extensions on first level\n");
                    // trans = extensions->invert().compose(trans);
                    extensions.invert().compose(&trans, true);
                    trans = extensions.clone();
                }
                FsaLevel::Second => {
                    verbose_printf("Applying extensions on second level\n");
                    trans.compose(&extensions, true);
                }
            }
            outstream.redirect(&mut trans);
            // C: delete extensions; the Rust binding drops at end of scope.
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
            return libc::EXIT_FAILURE;
        }

        process_stream(&mut instream, &mut outstream);
        instream.close();
        outstream.close();
        libc::EXIT_SUCCESS
    }
}
