//! Faithful 1:1 port of tools/src/hfst-edit-metadata.cc — the transducer
//! metadata tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, inc fragments).

use crate::globals;
use crate::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_from_env, hfst_set_program_name, parse_u64, verbose_print,
    warning,
};
use crate::hfst_getopt as getopt;
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use std::collections::BTreeMap;
use std::io::Write;

// add tools-specific variables here

static mut PROPERTIES: Option<BTreeMap<String, String>> = None;
static mut PROPERTIES_GIVEN: bool = false;
static mut PRINT_ALL_PROPERTIES: bool = true;
// C used a NULL char* as "no specific property requested"; modelled as Option.
static mut PRINT_PROPERTY: Option<String> = None;
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

fn print_property() -> Option<String> {
    unsafe { (*std::ptr::addr_of!(PRINT_PROPERTY)).clone() }
}

// [spec:hfst:def:hfst-edit-metadata.print-usage-fn]
// [spec:hfst:sem:hfst-edit-metadata.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    // Usage line
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nName a transducer\n\n",
        globals::program_name()
    );
    let _ = write!(
        msg,
        "Name options:\n\
         \x20 -a, --add=ANAME=VALUE       add or replace property ANAMEwith VALUE\n\
         \x20 -p, --print[=NAME]          print the current PNAME\n\
         \x20 -t, --truncate_length=LEN   truncate added properties' lengths to LEN\n"
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(msg, "If PNAME is omitted, all values are printed\n");
    let _ = write!(msg, "\n");
}

// [spec:hfst:def:hfst-edit-metadata.parse-options-fn]
// [spec:hfst:sem:hfst-edit-metadata.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_from_env(args);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_unary_long());
            // add tool-specific options here
            long_options.push(getopt::GetOpt {
                name: "add",
                has_arg: 1, // required_argument
                val: 'a' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "print-name",
                has_arg: 2, // optional_argument
                val: 'p' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "truncate_length",
                has_arg: 1, // required_argument
                val: 't' as i32,
            });
            // add tool-specific options here
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, unary cases, the error arm, then the tool's own cases.
            match handle_common_case(c, print_usage) {
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
                let optstr = getopt::optarg();
                match optstr.find('=') {
                    None => {
                        error(1, 0, &format!("Equals sign `=' missing from {}", optstr));
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
                match getopt::optarg_opt() {
                    Some(arg) => *std::ptr::addr_of_mut!(PRINT_PROPERTY) = Some(arg),
                    None => PRINT_ALL_PROPERTIES = true,
                }
                continue;
            } else if ch == b't' {
                TRUNCATE_LENGTH = parse_u64(&getopt::optarg(), 10);
                continue;
            }

            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(args);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-edit-metadata.process-stream-fn]
// [spec:hfst:sem:hfst-edit-metadata.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream, outstream: &mut HfstOutputStream) -> i32 {
    unsafe {
        let mut out = match globals::output_writer() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("hfst-edit-metadata: cannot open output: {e}");
                return 1;
            }
        };
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;

            if transducer_n > 1 && (PRINT_ALL_PROPERTIES || print_property().is_some()) {
                eprint!("--- \n");
            }

            if transducer_n == 1 {
                verbose_print(&format!("Metadata {}...\n", globals::input_filename()));
            } else {
                verbose_print(&format!(
                    "Metadata {}...{}\n",
                    globals::input_filename(),
                    transducer_n
                ));
            }

            let mut trans = match HfstTransducer::new_from_stream(instream) {
                Ok(v) => v,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            };
            if !PRINT_ALL_PROPERTIES && print_property().is_none() {
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
                            1,
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
                if let Err(e) = outstream.redirect(&mut trans) {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            } else {
                let props = trans.get_properties();
                if PRINT_ALL_PROPERTIES {
                    for (key, val) in props.iter() {
                        let _ = write!(out, "{}: {}\n", key, val);
                    }
                } else {
                    let pp = print_property().unwrap_or_default();
                    let _ = write!(out, "{}\n", props.get(&pp).unwrap());
                }
            }
        }
        instream.close();
        outstream.close();
        0
    }
}

// [spec:hfst:def:hfst-edit-metadata.main-fn]
// [spec:hfst:sem:hfst-edit-metadata.main-fn]
pub fn run(args: Vec<String>) -> i32 {
    unsafe { real_main(args) }
}

unsafe fn real_main(mut args: Vec<String>) -> i32 {
    unsafe {
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstEditMetadata");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }

        // close buffers, we use streams
        let input_opened = globals::input_filename() != "<stdin>";
        let output_opened = globals::output_filename() != "<stdout>";
        verbose_print(&format!(
            "Reading from {}, writing to {}\n",
            globals::input_filename(),
            globals::output_filename()
        ));

        // here starts the buffer handling part
        let mut instream = match if input_opened {
            HfstInputStream::new_filename(&globals::input_filename())
        } else {
            HfstInputStream::new()
        } {
            Ok(v) => v,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        };
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)

        let ty = instream.get_type();
        let mut outstream = match if output_opened {
            HfstOutputStream::new_filename(&globals::output_filename(), ty, true)
        } else {
            HfstOutputStream::new(ty, true)
        } {
            Ok(v) => v,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        };

        process_stream(&mut instream, &mut outstream)
    }
}
