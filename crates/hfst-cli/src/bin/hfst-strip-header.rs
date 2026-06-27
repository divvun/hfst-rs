//! Faithful 1:1 port of tools/src/hfst-strip-header.cc — the HFST header
//! stripping command-line tool. Drives the hfst-cli foundation (globals,
//! getopt, commandline, program-options, inc fragments).
//!
//! Unlike most unary tools, this one does not build HfstInputStream /
//! HfstOutputStream objects: it copies raw bytes from the input FILE* to the
//! output FILE*, dropping any embedded "HFST3" headers (and the NUL-terminated
//! text that follows them).

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
unsafe fn process_stream(f_in: *mut libc::FILE, f_out: *mut libc::FILE) -> c_int {
    unsafe {
        // "HFST3" plus the trailing NUL terminator (index 5).
        let header: &[u8] = b"HFST3\0";
        let mut header_loc: usize = 0; // how much of the header has been found
        loop {
            let mut c = libc::fgetc(f_in);
            if c == libc::EOF {
                return libc::EXIT_SUCCESS;
            }
            verbose_printf("Stripping...\n");
            if c == header[header_loc] as c_int {
                if header_loc == 5 {
                    // we've found the whole header (incl. null terminator);
                    // eat text until the next null terminator
                    loop {
                        c = libc::fgetc(f_in);
                        if c == b'\0' as c_int || c == libc::EOF {
                            break;
                        }
                    }
                    header_loc = 0;
                } else {
                    header_loc += 1; // look for the next character now
                }
            } else if header_loc > 0 {
                // flush the characters that could have been header but turned
                // out not to be
                for &b in &header[0..header_loc] {
                    libc::fputc(b as c_int, f_out);
                }
                header_loc = 0;
                // the character we just grabbed could be the start of the
                // header, so put it back
                libc::ungetc(c, f_in);
            } else {
                libc::fputc(c, f_out);
            }
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

        let retval = process_stream(globals::inputfile(), globals::outfile());

        // The C frees inputfilename/outfilename here; in the Rust foundation
        // those are static-mut C strings owned by the globals module, so they
        // are left in place.
        retval
    }
}
