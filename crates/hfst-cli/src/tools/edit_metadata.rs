//! Faithful 1:1 port of tools/src/hfst-edit-metadata.cc — the transducer
//! metadata tool. Drives the hfst-cli foundation (getopt, commandline,
//! program-options, inc fragments).
//!
//! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
//! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
//! `parse_options` and threaded into the processing functions. There are no
//! `static mut` globals and no `unsafe`.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, extend_options_from_env, hfst_set_program_name, parse_u64, verbose_print, warning,
};
use crate::hfst_getopt::{self as getopt, Getopt};
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
use std::collections::BTreeMap;
use std::io::Write;

/// hfst-edit-metadata's own options (the former tool-specific `static mut`s).
struct Options {
    /// '-a, --add=ANAME=VALUE': the properties to add or replace.
    properties: BTreeMap<String, String>,
    /// whether any '-a' property was given.
    properties_given: bool,
    /// whether all properties should be printed (the default).
    print_all_properties: bool,
    /// '-p, --print[=NAME]': the specific property to print. C used a NULL
    /// char* as "no specific property requested"; modelled as Option.
    print_property: Option<String>,
    /// '-t, --truncate_length=LEN': truncate added property lengths to LEN.
    truncate_length: u64,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            properties: BTreeMap::new(),
            properties_given: false,
            print_all_properties: true,
            print_property: None,
            truncate_length: 0,
        }
    }
}

// [spec:hfst:def:hfst-edit-metadata.print-usage-fn]
// [spec:hfst:sem:hfst-edit-metadata.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    // Usage line
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nName a transducer\n\n",
        common.program_name
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
    let _ = writeln!(msg);
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = writeln!(msg, "If PNAME is omitted, all values are printed");
    let _ = writeln!(msg);
}

// [spec:hfst:def:hfst-edit-metadata.parse-options-fn]
// [spec:hfst:sem:hfst-edit-metadata.parse-options-fn]
//
// Parse argv into the shared + tool options; `Err(code)` is an exit code the
// caller should return (the former EXIT_CONTINUE sentinel is now `Ok`).
fn parse_options(
    mut common: CommonOptions,
    args: &mut Vec<String>,
) -> Result<(CommonOptions, Options), i32> {
    let mut options = Options::default();
    let mut opt = Getopt::new();
    extend_options_from_env(args);
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
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: common
        // cases, unary cases, the error arm, then the tool's own cases.
        match handle_common_case(&mut common, &opt, c, print_usage) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        match handle_unary_case(&mut common, &opt, c) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        // tool-specific cases
        let ch = c as u8;
        if ch == b'a' {
            let optstr = opt.optarg();
            match optstr.find('=') {
                None => {
                    error(
                        &common,
                        1,
                        0,
                        &format!("Equals sign `=' missing from {}", optstr),
                    );
                }
                Some(idx) => {
                    let property = optstr[..idx].to_string();
                    let value = optstr[idx + 1..].to_string();
                    options.properties.insert(property, value);
                    options.properties_given = true;
                    options.print_all_properties = false;
                }
            }
            continue;
        } else if ch == b'p' {
            match opt.optarg_opt() {
                Some(arg) => options.print_property = Some(arg),
                None => options.print_all_properties = true,
            }
            continue;
        } else if ch == b't' {
            options.truncate_length = parse_u64(&common, &opt.optarg(), 10);
            continue;
        }

        return Err(handle_error_case(&common, &opt, c));
    }

    check_common_params(&mut common);
    check_unary_params(&mut common, &opt, args);
    Ok((common, options))
}

// [spec:hfst:def:hfst-edit-metadata.process-stream-fn]
// [spec:hfst:sem:hfst-edit-metadata.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &Options,
    instream: &mut HfstInputStream<'_>,
    outstream: &mut HfstOutputStream,
) -> i32 {
    let mut out = match common.output_writer() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("hfst-edit-metadata: cannot open output: {e}");
            return 1;
        }
    };
    let mut transducer_n: usize = 0;
    while instream.is_good() {
        transducer_n += 1;

        if transducer_n > 1 && (options.print_all_properties || options.print_property.is_some()) {
            eprintln!("--- ");
        }

        if transducer_n == 1 {
            verbose_print(common, &format!("Metadata {}...\n", common.input_filename));
        } else {
            verbose_print(
                common,
                &format!("Metadata {}...{}\n", common.input_filename, transducer_n),
            );
        }

        let any = match instream.read() {
            Ok(v) => v,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
        crate::for_any!(any, trans => {
            let mut trans = trans;
            if !options.print_all_properties && options.print_property.is_none() {
                for (key, val) in options.properties.iter() {
                    if key == "type" {
                        warning(
                            common,
                            0,
                            0,
                            "Changing `type' metadata will not change type of transducer in file;\n\
                             having wrong type may cause breakage, use with caution",
                        );
                    } else if key == "version" {
                        warning(
                            common,
                            0,
                            0,
                            "Changing `version' changes parsing semantics for header;\n\
                             use with caution",
                        );
                    } else if key == "character-encoding" && !(val == "utf-8" || val == "UTF-8") {
                        error(
                            common,
                            1,
                            0,
                            "Cannot set `character-encoding' to unsupported value;\n\
                             consider recoding sources of automaton",
                        );
                    }
                    if options.truncate_length > 0 {
                        // C: hfst_strndup(value.c_str(), truncate_length) — copy
                        // up to truncate_length bytes (NUL-terminating early).
                        let bytes = val.as_bytes();
                        let n = (options.truncate_length as usize).min(bytes.len());
                        let truncated = String::from_utf8_lossy(&bytes[..n]).into_owned();
                        trans.set_property(key, &truncated);
                    } else {
                        trans.set_property(key, val);
                    }
                }
                if let Err(e) = outstream.redirect(&mut trans) {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            } else {
                let props = trans.get_properties();
                if options.print_all_properties {
                    for (key, val) in props.iter() {
                        let _ = writeln!(out, "{}: {}", key, val);
                    }
                } else {
                    let pp = options.print_property.clone().unwrap_or_default();
                    let _ = writeln!(out, "{}", props.get(&pp).unwrap());
                }
            }
        });
    }
    instream.close();
    outstream.close();
    0
}

// [spec:hfst:def:hfst-edit-metadata.main-fn]
// [spec:hfst:sem:hfst-edit-metadata.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstEditMetadata");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    // close buffers, we use streams
    let input_opened = common.input_filename != "<stdin>";
    let output_opened = common.output_filename != "<stdout>";
    verbose_print(
        &common,
        &format!(
            "Reading from {}, writing to {}\n",
            common.input_filename, common.output_filename
        ),
    );

    // here starts the buffer handling part
    let mut instream = match if input_opened {
        HfstInputStream::new_filename(&common.input_filename)
    } else {
        HfstInputStream::new()
    } {
        Ok(v) => v,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return 1;
        }
    };
    // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
    // currently panics on a bad file rather than throwing, so the catch arm
    // is not reproduced here.)

    let ty = instream.get_type();
    let mut outstream = match if output_opened {
        HfstOutputStream::new_filename(&common.output_filename, ty, true)
    } else {
        HfstOutputStream::new(ty, true)
    } {
        Ok(v) => v,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return 1;
        }
    };

    process_stream(&common, &options, &mut instream, &mut outstream)
}
