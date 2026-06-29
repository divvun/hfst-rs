//! Faithful 1:1 port of tools/src/hfst-edit-metadata.cc — the transducer
//! metadata tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, inc fragments).

use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_getenv, hfst_set_program_name, hfst_strtoul,
    print_more_info, print_report_bugs, verbose_printf, warning,
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
use std::collections::BTreeMap;
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

fn fput(f: &mut dyn std::io::Write, s: &str) {
    let _ = f.write_all(s.as_bytes());
}

// add tools-specific variables here

static mut PROPERTIES: Option<BTreeMap<String, String>> = None;
static mut PROPERTIES_GIVEN: bool = false;
static mut PRINT_ALL_PROPERTIES: bool = true;
static mut PRINT_PROPERTY: *mut c_char = std::ptr::null_mut();
static mut TRUNCATE_LENGTH: u64 = 0;

unsafe fn properties() -> &'static mut BTreeMap<String, String> {
    unsafe {
        let p = &raw mut PROPERTIES;
        if (*p).is_none() {
            *p = Some(BTreeMap::new());
        }
        (*p).as_mut().unwrap()
    }
}

// [spec:hfst:def:hfst-edit-metadata.print-usage-fn]
// [spec:hfst:sem:hfst-edit-metadata.print-usage-fn]
unsafe fn print_usage() {
    unsafe {
        // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
        // Usage line
        let mut msg = globals::message_writer();
        let program_name = cstr(globals::PROGRAM_NAME);
        fput(
            &mut *msg,
            &format!(
                "Usage: {} [OPTIONS...] [INFILE]\nName a transducer\n\n",
                program_name
            ),
        );
        fput(
            &mut *msg,
            "Name options:\n\
             \x20 -a, --add=ANAME=VALUE       add or replace property ANAMEwith VALUE\n\
             \x20 -p, --print[=NAME]          print the current PNAME\n\
             \x20 -t, --truncate_length=LEN   truncate added properties' lengths to LEN\n",
        );
        print_common_program_options(&mut *msg);
        print_common_unary_program_options(&mut *msg);
        fput(&mut *msg, "\n");
        print_common_unary_program_parameter_instructions(&mut *msg);
        fput(&mut *msg, "If PNAME is omitted, all values are printed\n");
        fput(&mut *msg, "\n");
        print_report_bugs();
        fput(&mut *msg, "\n");
        print_more_info();
    }
}

// [spec:hfst:def:hfst-edit-metadata.parse-options-fn]
// [spec:hfst:sem:hfst-edit-metadata.parse-options-fn]
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
                name: c"add".as_ptr(),
                has_arg: 1, // required_argument
                flag: std::ptr::null_mut(),
                val: 'a' as c_int,
            });
            long_options.push(getopt::Option {
                name: c"print-name".as_ptr(),
                has_arg: 2, // optional_argument
                flag: std::ptr::null_mut(),
                val: 'p' as c_int,
            });
            long_options.push(getopt::Option {
                name: c"truncate_length".as_ptr(),
                has_arg: 1, // required_argument
                flag: std::ptr::null_mut(),
                val: 't' as c_int,
            });
            long_options.push(getopt::Option {
                name: std::ptr::null(),
                has_arg: 0,
                flag: std::ptr::null_mut(),
                val: 0,
            });
            let short = CString::new(format!(
                "{}{}a:p::t:",
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
            // cases, unary cases, the error arm, then the tool's own cases.
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
            // tool-specific cases
            let ch = c as u8;
            if ch == b'a' {
                let optarg = getopt::OPTARG;
                let optstr = cstr(optarg);
                match optstr.find('=') {
                    None => {
                        error(
                            libc::EXIT_FAILURE,
                            0,
                            &format!("Equals sign `=' missing from {}", optstr),
                        );
                    }
                    Some(idx) => {
                        let property = optstr[..idx].to_string();
                        let value = optstr[idx + 1..].to_string();
                        properties().insert(property, value);
                        PROPERTIES_GIVEN = true;
                        PRINT_ALL_PROPERTIES = false;
                    }
                }
                continue;
            } else if ch == b'p' {
                if !getopt::OPTARG.is_null() {
                    PRINT_PROPERTY = hfst_cli::hfst_commandline::hfst_strdup(getopt::OPTARG);
                } else {
                    PRINT_ALL_PROPERTIES = true;
                }
                continue;
            } else if ch == b't' {
                TRUNCATE_LENGTH = hfst_strtoul(&cstr(getopt::OPTARG), 10);
                continue;
            }

            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(argc, argv);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-edit-metadata.process-stream-fn]
// [spec:hfst:sem:hfst-edit-metadata.process-stream-fn]
unsafe fn process_stream(
    instream: &mut HfstInputStream,
    outstream: &mut HfstOutputStream,
) -> c_int {
    unsafe {
        let mut out = match globals::output_writer() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("hfst-edit-metadata: cannot open output: {e}");
                return libc::EXIT_FAILURE;
            }
        };
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;

            if transducer_n > 1 && (PRINT_ALL_PROPERTIES || !PRINT_PROPERTY.is_null()) {
                eprint!("--- \n");
            }

            if transducer_n == 1 {
                verbose_printf(&format!("Metadata {}...\n", cstr(globals::INPUTFILENAME)));
            } else {
                verbose_printf(&format!(
                    "Metadata {}...{}\n",
                    cstr(globals::INPUTFILENAME),
                    transducer_n
                ));
            }

            let mut trans = HfstTransducer::new_from_stream(instream);
            if !PRINT_ALL_PROPERTIES && PRINT_PROPERTY.is_null() {
                for (key, val) in properties().iter() {
                    if key == "type" {
                        warning(
                            0,
                            0,
                            "Changing `type' metadata will not change type of transducer in file;\n\
                             having wrong type may cause breakage, use with caution",
                        );
                    } else if key == "version" {
                        warning(
                            0,
                            0,
                            "Changing `version' changes parsing semantics for header;\n\
                             use with caution",
                        );
                    } else if key == "character-encoding" && !(val == "utf-8" || val == "UTF-8") {
                        error(
                            libc::EXIT_FAILURE,
                            0,
                            "Cannot set `character-encoding' to unsupported value;\n\
                             consider recoding sources of automaton",
                        );
                    }
                    if TRUNCATE_LENGTH > 0 {
                        // C: hfst_strndup(value.c_str(), truncate_length) — copy
                        // up to truncate_length bytes (NUL-terminating early).
                        let bytes = val.as_bytes();
                        let n = (TRUNCATE_LENGTH as usize).min(bytes.len());
                        let truncated = String::from_utf8_lossy(&bytes[..n]).into_owned();
                        trans.set_property(key, &truncated);
                    } else {
                        trans.set_property(key, val);
                    }
                }
                outstream.redirect(&mut trans);
            } else {
                let props = trans.get_properties();
                if PRINT_ALL_PROPERTIES {
                    for (key, val) in props.iter() {
                        fput(&mut *out, &format!("{}: {}\n", key, val));
                    }
                } else {
                    let pp = cstr(PRINT_PROPERTY);
                    fput(&mut *out, &format!("{}\n", props.get(&pp).unwrap()));
                }
            }
        }
        instream.close();
        outstream.close();
        libc::EXIT_SUCCESS
    }
}

// [spec:hfst:def:hfst-edit-metadata.main-fn]
// [spec:hfst:sem:hfst-edit-metadata.main-fn]
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

        hfst_set_program_name(&argv0, "0.1", "HfstEditMetadata");
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

        process_stream(&mut instream, &mut outstream)
    }
}
