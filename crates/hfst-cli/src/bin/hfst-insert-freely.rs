#![allow(static_mut_refs)]
//! Faithful 1:1 port of tools/src/hfst-insert-freely.cc — the freely-insert
//! a symbol (pair) command-line tool. Drives the hfst-cli foundation (globals,
//! getopt, commandline, program-options, tool-metadata, inc fragments).

use hfst::hfst_data_types::StringPair;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_symbol_defs::{internal_epsilon, label_to_stringpair};
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
use libc::{c_char, c_int};
use std::ffi::{CStr, CString};

// add tools-specific variables here
static mut LABEL: Option<String> = None;
static mut HARMONISE_FLAGS: bool = false;
static mut SYMBOL_PAIR: Option<StringPair> = None;

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

// FMT: Copied from hfst-substitute.cc ... should probably go in a library function

// [spec:hfst:def:hfst-insert-freely.print-usage-fn]
// [spec:hfst:sem:hfst-insert-freely.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        // Usage line
        let mut msg = globals::message_writer();
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            &mut *msg,
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\nFreely insert a symbol (pair)\n\n",
                program_name
            ),
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        fput(
            &mut *msg,
            "Option:\n  -a, --symbol-pair=SYM   symbol pair SYM\n  -H, --harmonise   harmonise \n",
        );
        fput(&mut *msg, "\n");
        print_common_unary_program_parameter_instructions(&mut *msg);
        fput(
            &mut *msg,
            "SYM must be either a single alphabeticsymbol or two symbols separated by a colon, :\n",
        );
        fput(&mut *msg, "\n");
        print_report_bugs();
        fput(&mut *msg, "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-insert-freely.parse-options-fn]
// [spec:hfst:sem:hfst-insert-freely.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            let symbol_pair_name = CString::new("symbol-pair").unwrap();
            let harmonise_name = CString::new("harmonise").unwrap();
            long_options.push(getopt::Option {
                name: symbol_pair_name.as_ptr(),
                has_arg: 1, // required_argument
                flag: std::ptr::null_mut(),
                val: 'a' as c_int,
            });
            long_options.push(getopt::Option {
                name: harmonise_name.as_ptr(),
                has_arg: 1, // required_argument
                flag: std::ptr::null_mut(),
                val: 'H' as c_int,
            });
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}a:H",
                HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT
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
            match c as u8 {
                b'a' => {
                    // This will probably break for unicode
                    let mut lbl = cstr(getopt::OPTARG);
                    if lbl == "@0@" {
                        lbl = internal_epsilon.to_string();
                    }
                    SYMBOL_PAIR = label_to_stringpair(&lbl);
                    if lbl.is_empty() {
                        error(
                            libc::EXIT_FAILURE,
                            0,
                            &format!(
                                "argument of source label option is empty;\nif you REALLY want to replace epsilons with something, use @0@ or {}",
                                internal_epsilon
                            ),
                        );
                    }
                    LABEL = Some(lbl);
                    continue;
                }
                b'H' => {
                    HARMONISE_FLAGS = true;
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

// [spec:hfst:def:hfst-insert-freely.process-stream-fn]
// [spec:hfst:sem:hfst-insert-freely.process-stream-fn]
unsafe fn process_stream(
    instream: &mut HfstInputStream,
    outstream: &mut HfstOutputStream,
) -> c_int {
    unsafe {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let mut trans = HfstTransducer::new_from_stream(instream);
            let _inputname = hfst_get_name(&trans, &cstr(globals::INPUTFILENAME));
            if transducer_n == 1 {
                // If harmonize is true, then identity and unknown symbols in the
                // transducer will be expanded by the symbols in symbol pair.
                // Otherwise they aren't.
                let pair = SYMBOL_PAIR.as_ref().expect("symbol pair must be set");
                trans.insert_freely_pair(pair, HARMONISE_FLAGS);
                // C: hfst_set_name(trans, trans, "insert-freely") and
                // hfst_set_formula(trans, trans, "Id"); dest and src are the
                // same object, so the read side is taken from a copy.
                let src = trans.clone();
                hfst_set_name_unary(&mut trans, &src, "insert-freely");
                hfst_set_formula_unary(&mut trans, &src, "Id");
            }
            outstream.redirect(&mut trans);
        }
        instream.close();
        outstream.close();
        libc::EXIT_SUCCESS
    }
}

// [spec:hfst:def:hfst-insert-freely.main-fn]
// [spec:hfst:sem:hfst-insert-freely.main-fn]
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

        hfst_set_program_name(&argv0, "0.1", "HfstPush");
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

        if is_input_stream_in_ol_format(&instream, "hfst-insert-freely") {
            return libc::EXIT_FAILURE;
        }

        process_stream(&mut instream, &mut outstream)
    }
}
