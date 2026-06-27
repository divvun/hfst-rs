//! Faithful 1:1 port of tools/src/hfst-split.cc — the transducer archive
//! exploding tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).

use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, extend_options_getenv, hfst_fopen, hfst_set_program_name, hfst_strdup,
    print_more_info, print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    HFST_GETOPT_COMMON_SHORT, hfst_getopt_common_long, print_common_program_options,
};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
};
use libc::{c_char, c_int};
use std::ffi::{CStr, CString};

// add tools-specific variables here
static mut PREFIX: *mut c_char = std::ptr::null_mut();
static mut EXTENSION: *mut c_char = std::ptr::null_mut();

// 'stdout'/'stdin' as FILE* (same shape as the foundation's private helpers,
// used here to mirror the C 'inputfile != stdin' / 'outfile != stdout' checks).
fn stdout_file() -> *mut libc::FILE {
    unsafe extern "C" {
        #[cfg_attr(target_os = "macos", link_name = "__stdoutp")]
        static mut stdout: *mut libc::FILE;
    }
    unsafe { stdout }
}

fn stdin_file() -> *mut libc::FILE {
    unsafe extern "C" {
        #[cfg_attr(target_os = "macos", link_name = "__stdinp")]
        static mut stdin: *mut libc::FILE;
    }
    unsafe { stdin }
}

unsafe fn cstr(ptr: *const c_char) -> String {
    if ptr.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned()
    }
}

unsafe fn dup(s: &str) -> *mut c_char {
    let c = CString::new(s).unwrap_or_default();
    unsafe { libc::strdup(c.as_ptr()) }
}

unsafe fn fput(f: *mut libc::FILE, s: &str) {
    let c = CString::new(s).unwrap_or_default();
    unsafe { libc::fputs(c.as_ptr(), f) };
}

// [spec:hfst:def:hfst-split.print-usage-fn]
// [spec:hfst:sem:hfst-split.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            globals::message_out(),
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\nExtract transducers from archive with systematic file names\n\n",
                program_name
            ),
        );
        print_common_program_options(globals::message_out());
        fput(
            globals::message_out(),
            "Input/Output options:\n  -i, --input=INFILE    Read input transducer from INFILE\n  -p, --prefix=PRE      Use the prefix PRE in naming output files\n  -e, --extension=EXT   Use the extension EXT in naming output files\n",
        );
        fput(globals::message_out(), "\n");
        fput(
            globals::message_out(),
            "If INFILE is omitted or -, stdin is used.\nIf PRE is omitted, no prefix is used.\nIf EXT is omitted, .hfst is used.\nThe extracted files are named \"PRE\" + N + \"EXT\",\nwhere N is the number of the transducer in the archive.\n\nAn example:\n   cat transducer_a transducer_b | hfst-split -p \"rule\" -e \".tr\"\n\nThis command creates files \"rule1.tr\" (equivalent to transducer_a)\nand \"rule2.tr\" (equivalent to transducer_b). \n",
        );
        fput(globals::message_out(), "\n");
        print_report_bugs();
        fput(globals::message_out(), "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-split.parse-options-fn]
// [spec:hfst:sem:hfst-split.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        EXTENSION = dup(".hfst");
        PREFIX = dup("");
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            // add tool-specific options here
            long_options.push(getopt::Option {
                name: c"input".as_ptr(),
                has_arg: getopt::REQUIRED_ARGUMENT,
                flag: std::ptr::null_mut(),
                val: b'i' as c_int,
            });
            long_options.push(getopt::Option {
                name: c"prefix".as_ptr(),
                has_arg: getopt::REQUIRED_ARGUMENT,
                flag: std::ptr::null_mut(),
                val: b'p' as c_int,
            });
            long_options.push(getopt::Option {
                name: c"extension".as_ptr(),
                has_arg: getopt::REQUIRED_ARGUMENT,
                flag: std::ptr::null_mut(),
                val: b'e' as c_int,
            });
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!("{}i:p:e:", HFST_GETOPT_COMMON_SHORT)).unwrap();
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

            // The C switch chains the #include'd common case group, then this
            // tool's own input/output cases, then the terminal error arm.
            match handle_common_case(c, || print_usage()) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match c {
                c if c == b'i' as c_int => {
                    globals::INPUTFILENAME = hfst_strdup(getopt::OPTARG);
                    globals::INPUTFILE = hfst_fopen(&cstr(globals::INPUTFILENAME), "r");
                    if globals::INPUTFILE == stdin_file() {
                        libc::free(globals::INPUTFILENAME as *mut libc::c_void);
                        globals::INPUTFILENAME = dup("<stdin>");
                    }
                    globals::INPUT_NAMED = true;
                    continue;
                }
                c if c == b'p' as c_int => {
                    libc::free(PREFIX as *mut libc::c_void);
                    PREFIX = dup(&cstr(getopt::OPTARG));
                    continue;
                }
                c if c == b'e' as c_int => {
                    libc::free(EXTENSION as *mut libc::c_void);
                    EXTENSION = dup(&cstr(getopt::OPTARG));
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

// [spec:hfst:def:hfst-split.process-stream-fn]
// [spec:hfst:sem:hfst-split.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream) -> c_int {
    unsafe {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let outfilename = format!("{}{}{}", cstr(PREFIX), transducer_n, cstr(EXTENSION));
            globals::OUTFILENAME = dup(&outfilename);
            verbose_printf(&format!(
                "Writing {} of {} to {}...\n",
                transducer_n,
                cstr(globals::INPUTFILENAME),
                outfilename
            ));
            let mut outstream =
                HfstOutputStream::new_filename(&outfilename, instream.get_type(), true);
            let mut trans = HfstTransducer::new_from_stream(instream);
            outstream.redirect(&mut trans);
            outstream.flush();
            outstream.close();
            libc::free(globals::OUTFILENAME as *mut libc::c_void);
            globals::OUTFILENAME = std::ptr::null_mut();
        }
        instream.close();
        libc::EXIT_SUCCESS
    }
}

// [spec:hfst:def:hfst-split.main-fn]
// [spec:hfst:sem:hfst-split.main-fn]
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

        hfst_set_program_name(&argv0, "0.1", "HfstSplit");
        let retval = parse_options(argc, argv);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        if globals::INPUTFILE != stdin_file() && !globals::INPUTFILE.is_null() {
            libc::fclose(globals::INPUTFILE);
        }
        if globals::OUTFILE != stdout_file() && !globals::OUTFILE.is_null() {
            libc::fclose(globals::OUTFILE);
        }
        verbose_printf(&format!(
            "Reading from {}, writing to {}...{}\n",
            cstr(globals::INPUTFILENAME),
            cstr(PREFIX),
            cstr(EXTENSION)
        ));
        // here starts the buffer handling part
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced faithfully here.)
        let mut instream = if globals::INPUTFILE != stdin_file() && !globals::INPUTFILE.is_null() {
            HfstInputStream::new_filename(&cstr(globals::INPUTFILENAME))
        } else {
            HfstInputStream::new()
        };

        let retval = process_stream(&mut instream);
        libc::free(globals::INPUTFILENAME as *mut libc::c_void);
        retval
    }
}
