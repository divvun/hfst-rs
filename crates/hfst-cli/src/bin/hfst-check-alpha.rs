//! Faithful 1:1 port of tools/src/hfst-check-alpha.cc — the tool that compares
//! the compatibility of alphabets within and between automata. Drives the
//! hfst-cli foundation (globals, getopt, commandline, program-options,
//! tool-metadata, inc fragments). A binary tool (two input streams).

use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_exception_defs::FunctionNotImplementedException;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_symbol_defs::StringSet;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_getenv, hfst_set_program_name, print_more_info,
    print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    HFST_GETOPT_BINARY_SHORT, HFST_GETOPT_COMMON_SHORT, hfst_getopt_binary_long,
    hfst_getopt_common_long, print_common_binary_program_options,
    print_common_binary_program_parameter_instructions, print_common_program_options,
};
use hfst_cli::inc::{
    CaseResult, check_binary_params, check_common_params, handle_binary_case, handle_common_case,
    handle_error_case,
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

// [spec:hfst:def:hfst-check-alpha.print-usage-fn]
// [spec:hfst:sem:hfst-check-alpha.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            globals::message_out(),
            &format!(
                "Usage: {} [OPTIONS...] [INFILEs]\nCompare the compatibility of alphabets between INFILEs\n\n",
                program_name
            ),
        );
        print_common_program_options(globals::message_out());
        print_common_binary_program_options(globals::message_out());
        // (tool-specific options and short descriptions)
        fput(globals::message_out(), "Check alpha options:\n");
        fput(globals::message_out(), "\n");
        print_common_binary_program_parameter_instructions(globals::message_out());
        fput(globals::message_out(), "\n");
        print_report_bugs();
        fput(globals::message_out(), "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-check-alpha.fprint-stringset-fn]
// [spec:hfst:sem:hfst-check-alpha.fprint-stringset-fn]
unsafe fn fprint_stringset(outfile: *mut libc::FILE, strings: &StringSet) {
    unsafe {
        let mut first = true;
        for s in strings {
            if !first {
                fput(outfile, ", ");
            }
            fput(outfile, s);
            first = false;
        }
    }
}

// [spec:hfst:def:hfst-check-alpha.parse-options-fn]
// [spec:hfst:sem:hfst-check-alpha.parse-options-fn]
unsafe fn parse_options(mut argc: c_int, mut argv: *mut *mut c_char) -> c_int {
    unsafe {
        extend_options_getenv(&mut argc, &mut argv);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::Option> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_binary_long());
            // add tool-specific options here
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}",
                HFST_GETOPT_COMMON_SHORT, HFST_GETOPT_BINARY_SHORT
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

            // The C switch chains the #include'd case groups in order: binary
            // cases, then common cases, then the tool's own (none here), then the
            // terminal error arm.
            match handle_binary_case(c) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_common_case(c, || print_usage()) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            return handle_error_case(c);
        }

        check_binary_params(argc, argv);
        check_common_params();
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-check-alpha.process-stream-fn]
// [spec:hfst:sem:hfst-check-alpha.process-stream-fn]
unsafe fn process_stream(
    firststream: &mut HfstInputStream,
    secondstream: &mut HfstInputStream,
) -> c_int {
    unsafe {
        let mut continue_reading = firststream.is_good() && secondstream.is_good();
        let mut transducer_n: usize = 0;
        let mut mismatch = libc::EXIT_SUCCESS;
        while continue_reading {
            transducer_n += 1;

            if transducer_n < 2 {
                verbose_printf("Checking alphas...\n");
            } else {
                verbose_printf(&format!("Checking alphas... {}\n", transducer_n));
            }
            // read first alphas
            let first = HfstTransducer::new_from_stream(firststream);
            let mutt: HfstBasicTransducer = first.get_basic_transducer();
            let mut first_transducer_alphabet: StringSet = StringSet::new();
            #[allow(unused_assignments)]
            let mut transducer_knows_alphabet = false;
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| first.get_alphabet())) {
                Ok(alpha) => {
                    first_transducer_alphabet = alpha;
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
            let mut first_found_alphabet: StringSet = StringSet::new();
            // iterate states in random order
            for transitions in mutt.states_and_transitions() {
                for tr_it in transitions {
                    first_found_alphabet.insert(tr_it.get_input_symbol());
                    first_found_alphabet.insert(tr_it.get_output_symbol());
                }
            }
            // read second alphas
            let second = HfstTransducer::new_from_stream(secondstream);
            let secondmutt: HfstBasicTransducer = second.get_basic_transducer();
            let mut second_transducer_alphabet: StringSet = StringSet::new();
            transducer_knows_alphabet = false;
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| second.get_alphabet())) {
                Ok(alpha) => {
                    second_transducer_alphabet = alpha;
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
            let mut second_found_alphabet: StringSet = StringSet::new();
            // iterate states in random order
            for transitions in secondmutt.states_and_transitions() {
                for tr_it in transitions {
                    second_found_alphabet.insert(tr_it.get_input_symbol());
                    second_found_alphabet.insert(tr_it.get_output_symbol());
                }
            }
            // match
            fput(globals::outfile(), "Actual alphabet differences:\n");
            let first_minus_second: StringSet = first_found_alphabet
                .difference(&second_found_alphabet)
                .cloned()
                .collect();
            if !first_minus_second.is_empty() {
                mismatch = libc::EXIT_FAILURE;
                fput(
                    globals::outfile(),
                    &format!(
                        "In first {} but not in second {}:",
                        first.get_name(),
                        second.get_name()
                    ),
                );
                fprint_stringset(globals::outfile(), &first_minus_second);
            } else {
                fput(
                    globals::outfile(),
                    &format!(
                        "First {} alpha is superset of second {}.",
                        first.get_name(),
                        second.get_name()
                    ),
                );
            }
            fput(globals::outfile(), "\n");
            let second_minus_first: StringSet = second_found_alphabet
                .difference(&first_found_alphabet)
                .cloned()
                .collect();
            if !second_minus_first.is_empty() {
                mismatch = libc::EXIT_FAILURE;
                fput(
                    globals::outfile(),
                    &format!(
                        "In second {} but not in first {}:",
                        second.get_name(),
                        second.get_name()
                    ),
                );
                fprint_stringset(globals::outfile(), &second_minus_first);
            } else {
                fput(
                    globals::outfile(),
                    &format!(
                        "Second {} alpha is superset of second {}.",
                        second.get_name(),
                        second.get_name()
                    ),
                );
            }
            fput(globals::outfile(), "\n");
            if globals::VERBOSE {
                fput(
                    globals::outfile(),
                    &format!("{} alphabet:", first.get_name()),
                );
                fprint_stringset(globals::outfile(), &first_found_alphabet);
                fput(globals::outfile(), "\n");
                fput(
                    globals::outfile(),
                    &format!("{} alphabet:", second.get_name()),
                );
                fprint_stringset(globals::outfile(), &second_found_alphabet);
                fput(globals::outfile(), "\n");
            }
            if transducer_knows_alphabet {
                fput(globals::outfile(), "sigma set difference:\n");
                let first_minus_second: StringSet = first_transducer_alphabet
                    .difference(&second_transducer_alphabet)
                    .cloned()
                    .collect();
                let second_minus_first: StringSet = second_transducer_alphabet
                    .difference(&first_transducer_alphabet)
                    .cloned()
                    .collect();
                if !first_minus_second.is_empty() {
                    mismatch = libc::EXIT_FAILURE;
                    fput(
                        globals::outfile(),
                        &format!(
                            "First {} has but second {} does not: ",
                            first.get_name(),
                            second.get_name()
                        ),
                    );
                    fprint_stringset(globals::outfile(), &first_minus_second);
                } else {
                    fput(
                        globals::outfile(),
                        &format!(
                            "First {} alpha is superset of second {}.",
                            first.get_name(),
                            second.get_name()
                        ),
                    );
                }
                fput(globals::outfile(), "\n");
                if !second_minus_first.is_empty() {
                    mismatch = libc::EXIT_FAILURE;
                    fput(
                        globals::outfile(),
                        &format!(
                            "Second {} has but first {} does not: ",
                            second.get_name(),
                            first.get_name()
                        ),
                    );
                    fprint_stringset(globals::outfile(), &second_minus_first);
                } else {
                    fput(
                        globals::outfile(),
                        &format!(
                            "Second {} alpha is superset of first {}.",
                            second.get_name(),
                            first.get_name()
                        ),
                    );
                }
                fput(globals::outfile(), "\n");
                if globals::VERBOSE {
                    fput(
                        globals::outfile(),
                        &format!("First ({}):", first.get_name()),
                    );
                    fprint_stringset(globals::outfile(), &first_transducer_alphabet);
                    fput(globals::outfile(), "\n");
                    fput(
                        globals::outfile(),
                        &format!("Second ({}):", second.get_name()),
                    );
                    fprint_stringset(globals::outfile(), &second_transducer_alphabet);
                    fput(globals::outfile(), "\n");
                }
            } else {
                fput(
                    globals::outfile(),
                    "No internal alphabets to compare in this format\n",
                );
            } // FSTs know their alphas
            continue_reading = firststream.is_good() && secondstream.is_good();
        }

        fput(
            globals::outfile(),
            &format!("\nRead {} transducers in total.\n", transducer_n),
        );
        mismatch
    }
}

// [spec:hfst:def:hfst-check-alpha.main-fn]
// [spec:hfst:sem:hfst-check-alpha.main-fn]
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

        hfst_set_program_name(&argv0, "0.1", "HfstALphaFix");
        let retval = parse_options(argc, argv);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let first_opened = !globals::FIRSTFILE.is_null();
        let second_opened = !globals::SECONDFILE.is_null();
        if first_opened {
            libc::fclose(globals::FIRSTFILE);
        }
        if second_opened {
            libc::fclose(globals::SECONDFILE);
        }
        verbose_printf(&format!(
            "Reading from {} and {}, writing to {}\n",
            cstr(globals::FIRSTFILENAME),
            cstr(globals::SECONDFILENAME),
            cstr(globals::OUTFILENAME)
        ));
        // here starts the buffer handling part
        // (the C wraps each ctor in try/catch on HfstException, calling error()
        // and returning EXIT_FAILURE; the Rust ctors currently panic on a bad
        // file rather than throwing. We mirror the intent via catch_unwind so the
        // error path and message are preserved.)
        let firststream = if first_opened {
            let name = cstr(globals::FIRSTFILENAME);
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                HfstInputStream::new_filename(&name)
            })) {
                Ok(s) => s,
                Err(_) => {
                    error(
                        libc::EXIT_FAILURE,
                        0,
                        &format!("{} is not a valid transducer file", name),
                    );
                    return libc::EXIT_FAILURE;
                }
            }
        } else {
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(HfstInputStream::new)) {
                Ok(s) => s,
                Err(_) => {
                    error(
                        libc::EXIT_FAILURE,
                        0,
                        &format!(
                            "{} is not a valid transducer file",
                            cstr(globals::FIRSTFILENAME)
                        ),
                    );
                    return libc::EXIT_FAILURE;
                }
            }
        };
        let secondstream = if second_opened {
            let name = cstr(globals::SECONDFILENAME);
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                HfstInputStream::new_filename(&name)
            })) {
                Ok(s) => s,
                Err(_) => {
                    error(
                        libc::EXIT_FAILURE,
                        0,
                        &format!("{} is not a valid transducer file", name),
                    );
                    return libc::EXIT_FAILURE;
                }
            }
        } else {
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(HfstInputStream::new)) {
                Ok(s) => s,
                Err(_) => {
                    error(
                        libc::EXIT_FAILURE,
                        0,
                        &format!(
                            "{} is not a valid transducer file",
                            cstr(globals::SECONDFILENAME)
                        ),
                    );
                    return libc::EXIT_FAILURE;
                }
            }
        };
        let mut firststream = firststream;
        let mut secondstream = secondstream;

        let _retval = process_stream(&mut firststream, &mut secondstream);

        if !globals::OUTFILE.is_null() {
            libc::fclose(globals::OUTFILE);
        }
        libc::free(globals::FIRSTFILENAME as *mut libc::c_void);
        libc::free(globals::SECONDFILENAME as *mut libc::c_void);
        libc::free(globals::OUTFILENAME as *mut libc::c_void);
        libc::EXIT_SUCCESS
    }
}
