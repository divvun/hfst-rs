//! Faithful 1:1 port of tools/src/hfst-dump-alphabets.cc — the alphabet dump
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, inc fragments).

use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_exception_defs::FunctionNotImplementedException;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_symbol_defs::StringSet;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, extend_options_getenv, hfst_set_program_name, print_more_info,
    print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_UNARY_SHORT, hfst_getopt_common_long,
    hfst_getopt_unary_long, print_common_program_options, print_common_unary_program_options,
    print_common_unary_program_parameter_instructions,
};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use libc::{c_char, c_int};
use std::ffi::{CStr, CString};
use std::io::Write;

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

// add tools-specific variables here
// [spec:hfst:def:hfst-dump-alphabets.alphadumpformat]
#[derive(Clone, Copy, PartialEq, Eq)]
enum AlphaDumpFormat {
    Tsv,
    Vislcg3List,
    Vislcg3Tags,
}

static mut OUTPUT_FORMAT: AlphaDumpFormat = AlphaDumpFormat::Tsv;
static mut PRINT_SEEN: bool = true;
static mut PRINT_META: bool = true;
static mut ONLY_MULTICHARS: bool = false;

// [spec:hfst:def:hfst-dump-alphabets.is-multichar-fn]
// [spec:hfst:sem:hfst-dump-alphabets.is-multichar-fn]
fn is_multichar(s: &str) -> bool {
    if s.len() > 2 {
        if s.starts_with('+') || s.starts_with(' ') || s.starts_with('@') {
            return true;
        } else {
            return false;
        }
    }
    false
}

// [spec:hfst:def:hfst-dump-alphabets.print-usage-fn]
// [spec:hfst:sem:hfst-dump-alphabets.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let mut msg = globals::message_writer();
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            &mut *msg,
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\nPrint alphabets of automaton\n\n",
                program_name
            ),
        );

        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        // fprintf(message_out, (tool-specific options and short descriptions)
        fput(&mut *msg, "Alphabet dump options:\n");
        fput(
            &mut *msg,
            "  -f, --format=AFORMAT     Print alphabet in AFORAMT\n",
        );
        fput(
            &mut *msg,
            "  -1, --exclude-seen       Ignore alphabets seen in automaton\n",
        );
        fput(
            &mut *msg,
            "  -2, --exclude-metadata   Ignore alphabets from headers\n",
        );
        fput(&mut *msg, "\n");
        print_common_unary_program_parameter_instructions(&mut *msg);
        fput(&mut *msg, "\n");
        print_report_bugs();
        fput(&mut *msg, "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-dump-alphabets.parse-options-fn]
// [spec:hfst:sem:hfst-dump-alphabets.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            long_options.push(getopt::Option {
                name: c"format".as_ptr(),
                has_arg: getopt::REQUIRED_ARGUMENT,
                flag: std::ptr::null_mut(),
                val: 'f' as c_int,
            });
            long_options.push(getopt::Option {
                name: c"include-seen".as_ptr(),
                has_arg: getopt::NO_ARGUMENT,
                flag: std::ptr::null_mut(),
                val: '1' as c_int,
            });
            long_options.push(getopt::Option {
                name: c"include-metadata".as_ptr(),
                has_arg: getopt::NO_ARGUMENT,
                flag: std::ptr::null_mut(),
                val: '2' as c_int,
            });
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}f:12",
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
            // add tool-specific cases here
            match c as u8 as char {
                'f' => {
                    let optarg = cstr(getopt::OPTARG);
                    if optarg == "tsv" {
                        OUTPUT_FORMAT = AlphaDumpFormat::Tsv;
                        ONLY_MULTICHARS = false;
                        verbose_printf("printing one symbol per line\n");
                    } else if optarg == "vislcg3-list" {
                        OUTPUT_FORMAT = AlphaDumpFormat::Vislcg3List;
                        ONLY_MULTICHARS = true;
                        verbose_printf("printing LIST x = x ; for VISL CG 3...\n");
                    } else if optarg == "vislcg3-tags" {
                        OUTPUT_FORMAT = AlphaDumpFormat::Vislcg3Tags;
                        ONLY_MULTICHARS = true;
                        verbose_printf("printing STRICT-TAGS += for VISL CG 3...\n");
                    } else {
                        eprintln!("Error: unrecognised format {}", optarg);
                        std::process::exit(libc::EXIT_FAILURE);
                    }
                    continue;
                }
                '1' => {
                    PRINT_SEEN = false;
                    continue;
                }
                '2' => {
                    PRINT_META = false;
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

// [spec:hfst:def:hfst-dump-alphabets.process-stream-fn]
// [spec:hfst:sem:hfst-dump-alphabets.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream) -> c_int {
    unsafe {
        // Data output goes to a std stream (the std counterpart of the libc
        // outfile FILE*); `emit` writes a string and ignores errors, matching the
        // old fput/fputs. (print_usage's message_out path stays on FILE* until
        // the message_out chunk of io-foundation.)
        let mut out = match globals::output_writer() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("hfst-dump-alphabets: could not open output: {e}");
                return libc::EXIT_FAILURE;
            }
        };
        let mut emit = |s: &str| {
            let _ = out.write_all(s.as_bytes());
        };
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            if transducer_n < 2 {
                verbose_printf("Alphadumping...\n");
            } else {
                verbose_printf(&format!("Alphadumping... {}\n", transducer_n));
            }
            let trans = HfstTransducer::new_from_stream(instream);
            let mutt = HfstBasicTransducer::new_from_transducer(&trans);
            // unsigned int initial_state = 0; // mutt.get_initial_state();
            let mut transducer_alphabet = StringSet::new();
            let transducer_knows_alphabet;
            // C wraps get_alphabet in try/catch on FunctionNotImplementedException;
            // the Rust facade throws via panic_any, so catch_unwind reproduces it.
            let prev = std::panic::take_hook();
            std::panic::set_hook(Box::new(|_| {}));
            let caught =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| trans.get_alphabet()));
            std::panic::set_hook(prev);
            match caught {
                Ok(alpha) => {
                    transducer_alphabet = alpha;
                    transducer_knows_alphabet = true;
                }
                Err(e) => {
                    if e.downcast_ref::<FunctionNotImplementedException>()
                        .is_some()
                    {
                        transducer_knows_alphabet = false;
                    } else {
                        std::panic::resume_unwind(e);
                    }
                }
            }
            let found_alphabet: StringSet = mutt.symbols_used();
            if OUTPUT_FORMAT == AlphaDumpFormat::Vislcg3Tags {
                emit(
                    "## automatically generated VISL CG 3 file from HFST automaton's alphabet data:\n",
                );
                emit("## (some statistics here TODO)\n");
                emit("STRICT-TAGS +=\n");
            } else if OUTPUT_FORMAT == AlphaDumpFormat::Vislcg3List {
                emit(
                    "## automatically generated VISL CG 3 file from HFST automaton's alphabet data:\n",
                );
                emit("## (some statistics here TODO)\n");
            }
            if PRINT_META {
                if transducer_knows_alphabet {
                    for s in transducer_alphabet.iter() {
                        if ONLY_MULTICHARS && !is_multichar(s) {
                            continue;
                        }
                        if OUTPUT_FORMAT == AlphaDumpFormat::Tsv {
                            emit(&format!("{}\n", s));
                        } else if OUTPUT_FORMAT == AlphaDumpFormat::Vislcg3Tags {
                            emit(&format!("\t{}\n", s));
                        } else if OUTPUT_FORMAT == AlphaDumpFormat::Vislcg3List {
                            emit(&format!("LIST {} = {} ;\n", s, s));
                        }
                    }
                } else {
                    eprintln!("Error: cannot dump non-existent header alphabet");
                    std::process::exit(libc::EXIT_FAILURE);
                }
            }
            if PRINT_SEEN {
                for s in found_alphabet.iter() {
                    if ONLY_MULTICHARS && !is_multichar(s) {
                        continue;
                    }
                    if OUTPUT_FORMAT == AlphaDumpFormat::Tsv {
                        emit(&format!("{}\n", s));
                    } else if OUTPUT_FORMAT == AlphaDumpFormat::Vislcg3Tags {
                        emit(&format!("\t{}\n", s));
                    } else if OUTPUT_FORMAT == AlphaDumpFormat::Vislcg3List {
                        emit(&format!("LIST {} = {} ;\n", s, s));
                    }
                }
            }
        } // for each automaton
        if OUTPUT_FORMAT == AlphaDumpFormat::Vislcg3Tags {
            emit("\t;\n");
        }
        libc::EXIT_SUCCESS
    }
}

// [spec:hfst:def:hfst-dump-alphabets.main-fn]
// [spec:hfst:sem:hfst-dump-alphabets.main-fn]
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

        hfst_set_program_name(&argv0, "0.1", "HfstSummarize");
        let retval = parse_options(argc, argv);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let input_opened = cstr(globals::INPUTFILENAME) != "<stdin>";
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            cstr(globals::INPUTFILENAME),
            cstr(globals::OUTFILENAME)
        ));
        // here starts the buffer handling part
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // that calls error(EXIT_FAILURE, ...) is not reproduced here.)
        let mut instream = if input_opened {
            HfstInputStream::new_filename(&cstr(globals::INPUTFILENAME))
        } else {
            HfstInputStream::new()
        };
        let _retval = process_stream(&mut instream);

        libc::EXIT_SUCCESS
    }
}
