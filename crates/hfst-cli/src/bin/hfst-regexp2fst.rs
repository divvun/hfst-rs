//! Faithful 1:1 port of tools/src/hfst-regexp2fst.cc — the regular expression
//! compiling command-line tool. Drives the hfst-cli foundation (globals,
//! getopt, commandline, program-options, tool-metadata, inc fragments) plus the
//! hfst XreCompiler.

use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::{
    HfstTransducer, get_encode_weights, set_encode_weights, set_flag_is_epsilon_in_composition,
    set_minimization, set_xerox_composition,
};
use hfst::xre::XreCompiler;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_getenv, hfst_error_at_line, hfst_getdelim,
    hfst_parse_format_name, hfst_set_program_name, hfst_strdup, print_more_info, print_report_bugs,
    verbose_printf,
};
use hfst_cli::hfst_file_to_mem::hfst_file_to_mem;
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT, hfst_getopt_common_long,
    hfst_getopt_unary_long, print_common_program_options, print_common_unary_program_options,
};
use hfst_cli::hfst_tool_metadata::hfst_set_name;
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use libc::{c_char, c_int};
use std::ffi::{CStr, CString};

// File-scope tool state, mirroring the static globals in the C++ source.
static mut EPSILONNAME: *mut c_char = std::ptr::null_mut();
static mut DISJUNCT_EXPRESSIONS: bool = false;
static mut LINE_SEPARATED: bool = true;
static mut ENCODE_WEIGHTS: bool = false;
static mut OUTPUT_FORMAT: ImplementationType = ImplementationType::UNSPECIFIED_TYPE;
static mut HARMONIZE: bool = true;
static mut HARMONIZE_FLAGS: bool = false;
static mut MINIMIZE_RESULT: bool = true;

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

// [spec:hfst:def:hfst-regexp2fst.print-usage-fn]
// [spec:hfst:sem:hfst-regexp2fst.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            globals::message_out(),
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\n\
                 Compile (weighted) regular expressions into transducer(s)\n",
                program_name
            ),
        );
        print_common_program_options(globals::message_out());
        print_common_unary_program_options(globals::message_out());
        fput(
            globals::message_out(),
            "String and format options:\n\
             \x20 -f, --format=FMT          Write result in FMT format\n\
             \x20 -j, --disjunct            Disjunct all regexps instead of transforming\n\
             \x20                           each regexp into a separate transducer\n\
             \x20 -l, --line                Input is line separated (default)\n\
             \x20 -S, --semicolon           Input is semicolon separated\n\
             \x20 -e, --epsilon=EPS         Map EPS as zero, i.e. epsilon.\n\
             \x20 -x, --xerox-composition=VALUE Whether flag diacritics are treated as ordinary\n\
             \x20                               symbols in composition (default is false).\n\
             \x20 -X, --xfst=VARIABLE       Toggle xfst compatibility option VARIABLE.\n\
             Harmonization and optimization options:\n\
             \x20 -H, --do-not-harmonize    Do not expand '?' symbols.\n\
             \x20 -F, --harmonize-flags     Harmonize flag diacritics.\n\
             \x20 -E, --encode-weights      Encode weights when minimizing (default is false).\n\
             \x20 -M, --do-not-minimize     Determinize result instead of minimizing it.\n",
        );
        fput(globals::message_out(), "\n");

        fput(
            globals::message_out(),
            "If OUTFILE or INFILE is missing or -, standard streams will be used.\n\
             FMT must be one of the following: \
             {foma, sfst, openfst-tropical, openfst-log}.\n\
             If EPS is not defined, the default representation of 0 is used\n\
             VALUEs recognized are {true,ON,yes} and {false,OFF,no}.\n\
             Xfst variables are {flag-is-epsilon (default OFF)}.\n\
             \n",
        );

        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            globals::message_out(),
            &format!(
                "Examples:\n\
                 \x20 echo \" {{cat}}:{{dog}} \" | {0}       create transducer {{cat}}:{{dog}}\n\
                 \x20 echo \" {{cat}}:{{dog}}::3 \" | {0}    same but with weight 3\n\
                 \x20 echo \" c:d a:o::3 t:g \" | {0}    same but with weight 3\n\
                 \x20                                             in the middle\n\
                 \x20 echo \" cat ; dog ; \"3\" \" | {0} -S  create transducers\n\
                 \x20                                             \"cat\" and \"dog\" and \"3\"\n\
                 \n",
                program_name
            ),
        );
        print_report_bugs();
        fput(globals::message_out(), "\n");
        print_more_info();
        fput(globals::message_out(), "\n");
    }
}

// [spec:hfst:def:hfst-regexp2fst.parse-options-fn]
// [spec:hfst:sem:hfst-regexp2fst.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let push_opt =
                |v: &mut Vec<getopt::Option>, name: &'static str, has_arg: c_int, val: c_int| {
                    let c = CString::new(name).unwrap();
                    let ptr = c.into_raw();
                    v.push(getopt::Option {
                        name: ptr,
                        has_arg,
                        flag: std::ptr::null_mut(),
                        val,
                    });
                };
            push_opt(&mut long_options, "disjunct", 0, 'j' as c_int);
            push_opt(&mut long_options, "epsilon", 1, 'e' as c_int);
            push_opt(&mut long_options, "line", 0, 'l' as c_int);
            push_opt(&mut long_options, "semicolon", 0, 'S' as c_int);
            push_opt(&mut long_options, "format", 1, 'f' as c_int);
            push_opt(&mut long_options, "do-not-harmonize", 0, 'H' as c_int);
            push_opt(&mut long_options, "harmonize-flags", 0, 'F' as c_int);
            push_opt(&mut long_options, "encode-weights", 0, 'E' as c_int);
            push_opt(&mut long_options, "xerox-composition", 1, 'x' as c_int);
            push_opt(&mut long_options, "xfst", 1, 'X' as c_int);
            push_opt(&mut long_options, "do-not-minimize", 0, 'M' as c_int);
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}je:lSf:HFEx:X:M",
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
            match c as u8 as char {
                'e' => {
                    EPSILONNAME = hfst_strdup(getopt::OPTARG);
                    continue;
                }
                'j' => {
                    DISJUNCT_EXPRESSIONS = true;
                    continue;
                }
                'S' => {
                    LINE_SEPARATED = false;
                    continue;
                }
                'l' => {
                    LINE_SEPARATED = true;
                    continue;
                }
                'f' => {
                    OUTPUT_FORMAT = hfst_parse_format_name(&cstr(getopt::OPTARG));
                    continue;
                }
                'H' => {
                    HARMONIZE = false;
                    continue;
                }
                'F' => {
                    HARMONIZE_FLAGS = true;
                    continue;
                }
                'E' => {
                    ENCODE_WEIGHTS = true;
                    continue;
                }
                'M' => {
                    MINIMIZE_RESULT = false;
                    continue;
                }
                'x' => {
                    let argument = cstr(getopt::OPTARG);
                    if argument == "yes" || argument == "true" || argument == "ON" {
                        set_xerox_composition(true);
                    } else if argument == "no" || argument == "false" || argument == "OFF" {
                        set_xerox_composition(false);
                    } else {
                        error(
                            libc::EXIT_FAILURE,
                            0,
                            &format!(
                                "unknown option to --xerox-composition: '{}'\n",
                                cstr(getopt::OPTARG)
                            ),
                        );
                        return libc::EXIT_FAILURE;
                    }
                    continue;
                }
                'X' => {
                    let argument = cstr(getopt::OPTARG);
                    if argument == "flag-is-epsilon" {
                        set_flag_is_epsilon_in_composition(true);
                    } else {
                        error(
                            libc::EXIT_FAILURE,
                            0,
                            &format!(
                                "Error: unknown option to --xfst: '{}'\n",
                                cstr(getopt::OPTARG)
                            ),
                        );
                        return libc::EXIT_FAILURE;
                    }
                    continue;
                }
                _ => {}
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(argc, argv);
        if OUTPUT_FORMAT == ImplementationType::UNSPECIFIED_TYPE {
            verbose_printf("Output format not specified, defaulting to openfst tropical\n");
            OUTPUT_FORMAT = ImplementationType::TROPICAL_OPENFST_TYPE;
        }
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-regexp2fst.process-stream-fn]
// [spec:hfst:sem:hfst-regexp2fst.process-stream-fn]
unsafe fn process_stream(outstream: &mut HfstOutputStream) -> c_int {
    unsafe {
        let mut transducer_n: usize = 0;
        let mut line: *mut c_char = std::ptr::null_mut();
        let mut len: usize = 0;
        let mut line_count: u32 = 0;
        let mut comp = XreCompiler::new(OUTPUT_FORMAT);
        comp.set_verbosity(globals::VERBOSE);
        comp.set_error_stream(());
        comp.set_harmonization(HARMONIZE);
        comp.set_flag_harmonization(HARMONIZE_FLAGS);
        set_minimization(MINIMIZE_RESULT);
        let mut disjunction = HfstTransducer::new_type(OUTPUT_FORMAT);

        let delim: c_char = if LINE_SEPARATED { b'\n' } else { b';' } as c_char;
        let mut first_line: *mut c_char = std::ptr::null_mut();

        if !LINE_SEPARATED {
            let mut filebuf_ = hfst_file_to_mem(&cstr(globals::INPUTFILENAME));
            let mut chars_read: u32 = 0;

            loop {
                transducer_n += 1;
                verbose_printf(&format!("Compiling expression #{}\n", transducer_n as i32));
                // Build a &str view of the remaining buffer at the current cursor.
                let remaining = cstr(filebuf_);
                let compiled = comp.compile_first(&remaining, &mut chars_read);
                // (the C wraps compile_first in try/catch on HfstException; the
                // Rust path currently panics rather than throwing, so the catch
                // arm that calls hfst_error is not reproduced here.)
                if compiled.is_null() {
                    if comp.contained_only_comments() {
                        if transducer_n == 1 {
                            error(
                                libc::EXIT_FAILURE,
                                0,
                                &format!(
                                    "{}: XRE parsing failed: expression #{} \
                                     contains only whitespace or comments",
                                    cstr(globals::INPUTFILENAME),
                                    transducer_n as u32
                                ),
                            );
                        }
                        break;
                    } else {
                        error(
                            libc::EXIT_FAILURE,
                            0,
                            &format!(
                                "{}: XRE parsing failed \
                                 in expression #{} separated by semicolons",
                                cstr(globals::INPUTFILENAME),
                                transducer_n as u32
                            ),
                        );
                    }
                }
                for _ in 0..chars_read {
                    filebuf_ = filebuf_.add(1);
                }
                if !compiled.is_null() {
                    if DISJUNCT_EXPRESSIONS {
                        disjunction.disjunct(&*compiled, HARMONIZE);
                    } else {
                        hfst_set_name(&mut *compiled, "?", "xre");
                        outstream.redirect(&mut *compiled);
                    }
                    // C: delete compiled;
                    drop(Box::from_raw(compiled));
                }
                if *filebuf_ == 0 {
                    break;
                }
            }
        } else {
            let mut input_contains_only_whitespace_or_comments = true;
            loop {
                if hfst_getdelim(&mut line, &mut len, delim as i32, globals::inputfile()) == -1 {
                    if input_contains_only_whitespace_or_comments {
                        error(
                            libc::EXIT_FAILURE,
                            0,
                            &format!(
                                "{}: XRE parsing failed: \
                                 input contains only whitespace or comments",
                                cstr(globals::INPUTFILENAME)
                            ),
                        );
                    }
                    break;
                }
                if first_line.is_null() {
                    first_line = libc::strdup(line);
                }
                let mut exp = line;
                while *exp == b'\n' as c_char || *exp == b'\r' as c_char || *exp == b' ' as c_char {
                    exp = exp.add(1);
                }
                line_count += 1;
                if *exp == 0 {
                    verbose_printf(&format!("Skipping whitespace expression #{}", line_count));
                    continue;
                }
                transducer_n += 1;
                verbose_printf(&format!("Compiling expression {}\n", line_count));
                let compiled = comp.compile(&cstr(exp));
                // (the C wraps compile in try/catch on HfstException calling
                // hfst_error_at_line; the Rust path panics rather than throwing,
                // so the catch arm is not reproduced here.)
                if compiled.is_null() {
                    if !comp.contained_only_comments() {
                        hfst_error_at_line(
                            libc::EXIT_FAILURE,
                            0,
                            &cstr(globals::INPUTFILENAME),
                            line_count,
                            "XRE parsing failed\n",
                        );
                    }
                    continue;
                }
                input_contains_only_whitespace_or_comments = false;

                if DISJUNCT_EXPRESSIONS {
                    disjunction.disjunct(&*compiled, HARMONIZE);
                } else {
                    hfst_set_name(&mut *compiled, "?", "xre");
                    outstream.redirect(&mut *compiled);
                }
                // C: delete compiled;
                drop(Box::from_raw(compiled));
            }
        }

        if DISJUNCT_EXPRESSIONS {
            // Both branches of the C++ if/else set the same name.
            hfst_set_name(&mut disjunction, "?", "xre");
            outstream.redirect(&mut disjunction);
        }
        libc::free(line as *mut libc::c_void);
        libc::free(first_line as *mut libc::c_void);
        libc::EXIT_SUCCESS
    }
}

// [spec:hfst:def:hfst-regexp2fst.main-fn]
// [spec:hfst:sem:hfst-regexp2fst.main-fn]
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

        hfst_set_program_name(&argv0, "0.2", "Regexp2Fst");
        let retval = parse_options(argc, argv);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        if globals::DEBUG {
            // xredebug = 1;
        }

        let enc = get_encode_weights();
        if ENCODE_WEIGHTS {
            set_encode_weights(true);
        }

        // close buffers, we use streams
        let output_opened = !globals::OUTFILE.is_null();
        if output_opened {
            libc::fclose(globals::OUTFILE);
        }
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            cstr(globals::INPUTFILENAME),
            cstr(globals::OUTFILENAME)
        ));
        // here starts the buffer handling part
        let mut outstream = if output_opened {
            HfstOutputStream::new_filename(&cstr(globals::OUTFILENAME), OUTPUT_FORMAT, true)
        } else {
            HfstOutputStream::new(OUTPUT_FORMAT, true)
        };
        process_stream(&mut outstream);

        if ENCODE_WEIGHTS {
            set_encode_weights(enc);
        }

        libc::free(globals::INPUTFILENAME as *mut libc::c_void);
        libc::free(globals::OUTFILENAME as *mut libc::c_void);
        libc::EXIT_SUCCESS
    }
}
