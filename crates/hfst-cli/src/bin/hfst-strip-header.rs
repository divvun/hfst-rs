//! Faithful 1:1 port of tools/src/hfst-strip-header.cc — the HFST header
//! stripping command-line tool. Drives the hfst-cli foundation (globals,
//! getopt, commandline, program-options, inc fragments).
//!
//! Unlike most unary tools, this one does not build HfstInputStream /
//! HfstOutputStream objects: it opens its input/output as std streams (from the
//! filename globals, with the "<stdin>"/"<stdout>" sentinels) and delegates the
//! byte copy + HFST3-header stripping to hfst_input_stream::strip_hfst3_headers.

use hfst::hfst_input_stream::strip_hfst3_headers;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, hfst_set_program_name, print_more_info, print_report_bugs, verbose_printf,
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

// [spec:hfst:def:hfst-strip-header.print-usage-fn]
// [spec:hfst:sem:hfst-strip-header.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            globals::message_out(),
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\nRemove any HFST3 headers\n\n",
                program_name
            ),
        );
        print_common_program_options(globals::message_out());
        print_common_unary_program_options(globals::message_out());
        fput(globals::message_out(), "\n");
        print_common_unary_program_parameter_instructions(globals::message_out());
        fput(globals::message_out(), "\n");
        print_report_bugs();
        fput(globals::message_out(), "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-strip-header.parse-options-fn]
// [spec:hfst:sem:hfst-strip-header.parse-options-fn]
unsafe fn parse_options(argc: c_int, argv: *mut *mut c_char) -> c_int {
    unsafe {
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}",
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
            // cases, then unary cases, then the terminal error arm.
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
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(argc, argv);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-strip-header.process-stream-fn]
// [spec:hfst:sem:hfst-strip-header.process-stream-fn]
unsafe fn process_stream() -> c_int {
    // De-C-ified: open the input/output as std streams from the filename globals
    // ("<stdin>"/"<stdout>" sentinels select the standard streams) and delegate
    // the HFST3-header stripping to hfst_input_stream::strip_hfst3_headers. The
    // C printed "Stripping..." once per byte under -v; that per-byte trace is
    // dropped (diagnostic only — the stripped output is unchanged).
    let (in_name, out_name) = unsafe { (cstr(globals::INPUTFILENAME), cstr(globals::OUTFILENAME)) };

    let input: Box<dyn std::io::Read> = if in_name == "<stdin>" {
        Box::new(std::io::stdin())
    } else {
        match std::fs::File::open(&in_name) {
            Ok(f) => Box::new(f),
            Err(e) => {
                eprintln!("hfst-strip-header: could not open input {in_name}: {e}");
                return libc::EXIT_FAILURE;
            }
        }
    };
    let output: Box<dyn std::io::Write> = if out_name == "<stdout>" {
        Box::new(std::io::stdout())
    } else {
        match std::fs::File::create(&out_name) {
            Ok(f) => Box::new(f),
            Err(e) => {
                eprintln!("hfst-strip-header: could not open output {out_name}: {e}");
                return libc::EXIT_FAILURE;
            }
        }
    };

    match strip_hfst3_headers(input, output) {
        Ok(()) => libc::EXIT_SUCCESS,
        Err(e) => {
            eprintln!("hfst-strip-header: error while stripping headers: {e}");
            libc::EXIT_FAILURE
        }
    }
}

// [spec:hfst:def:hfst-strip-header.main-fn]
// [spec:hfst:sem:hfst-strip-header.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> c_int {
    unsafe {
        // Build a C-style argv (NULL-terminated) from the Rust args; getopt
        // reorders it in place.
        let c_args: Vec<CString> = std::env::args()
            .map(|a| CString::new(a).unwrap_or_default())
            .collect();
        let mut argv_vec: Vec<*mut c_char> =
            c_args.iter().map(|s| s.as_ptr() as *mut c_char).collect();
        argv_vec.push(std::ptr::null_mut());
        let argc: c_int = c_args.len() as c_int;
        let argv: *mut *mut c_char = argv_vec.as_mut_ptr();
        let argv0 = cstr(*argv);

        hfst_set_program_name(&argv0, "0.1", "HfstStripHeader");
        let retval = parse_options(argc, argv);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            cstr(globals::INPUTFILENAME),
            cstr(globals::OUTFILENAME)
        ));

        let retval = process_stream();

        // The C frees inputfilename/outfilename here; in the Rust foundation
        // those are static-mut C strings owned by the globals module, so they
        // are left in place.
        retval
    }
}
