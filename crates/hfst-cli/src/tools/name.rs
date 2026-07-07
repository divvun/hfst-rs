//! Faithful 1:1 port of tools/src/hfst-name.cc — the transducer naming
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).
//!
//! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
//! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
//! `parse_options` and threaded into the processing functions. There are no
//! `static mut` globals and no `unsafe`.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    extend_options_from_env, hfst_set_program_name, parse_u64, verbose_print,
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
use std::io::Write;

/// hfst-name's own options (the former tool-specific `static mut`s).
#[derive(Default)]
struct Options {
    /// '-n, --name=NAME': the name to set on the transducer.
    transducer_name: String,
    /// whether '-n / --name' was given.
    name_option_given: bool,
    /// '-p, --print-name': only print the current name.
    print_name: bool,
    /// '-t, --truncate_length=LEN': truncate the name to LEN bytes (0 = no limit).
    truncate_length: u64,
}

// [spec:hfst:def:hfst-name.print-usage-fn]
// [spec:hfst:sem:hfst-name.print-usage-fn]
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
        "Name options:\n  -n, --name=NAME      Name the transducer NAME\n  -p, --print-name     Only print the current name\n  -t, --truncate_length=LEN   Truncate name length to LEN\n"
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(msg, "\n");
}

// [spec:hfst:def:hfst-name.parse-options-fn]
// [spec:hfst:sem:hfst-name.parse-options-fn]
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
            name: "name",
            has_arg: getopt::REQUIRED_ARGUMENT,
            val: b'n' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "print-name",
            has_arg: getopt::NO_ARGUMENT,
            val: b'p' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "truncate_length",
            has_arg: getopt::REQUIRED_ARGUMENT,
            val: b't' as i32,
        });
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: common
        // cases, then unary cases, then the terminal error arm, then the
        // tool's own cases.
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
        // tool-specific cases come before the error arm in the C switch
        // ordering (getopt-cases-error.h precedes them textually but its
        // arms only fire on '?'/ ':' / default, so the named cases below
        // are reached for 'n'/'p'/'t').
        let byte = c as u8;
        match byte {
            b'n' => {
                options.transducer_name = opt.optarg();
                options.name_option_given = true;
                continue;
            }
            b'p' => {
                options.print_name = true;
                continue;
            }
            b't' => {
                options.truncate_length = parse_u64(&common, &opt.optarg(), 10);
                continue;
            }
            _ => {}
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    check_common_params(&mut common);
    check_unary_params(&mut common, &opt, args);
    Ok((common, options))
}

// [spec:hfst:def:hfst-name.process-stream-fn]
// [spec:hfst:sem:hfst-name.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &Options,
    instream: &mut HfstInputStream<'_>,
    outstream: &mut HfstOutputStream,
) -> i32 {
    let mut transducer_n: usize = 0;
    while instream.is_good() {
        transducer_n += 1;

        if transducer_n > 1 && options.print_name {
            eprint!("---\n");
        }

        if transducer_n == 1 {
            verbose_print(common, &format!("Naming {}...\n", common.input_filename));
        } else {
            verbose_print(
                common,
                &format!("Naming {}...{}\n", common.input_filename, transducer_n),
            );
        }

        let any = match instream.read() {
            Ok(v) => v,
            Err(e) => {
                eprintln!("hfst-name: {e}");
                return 1;
            }
        };
        // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
        crate::for_any!(any, trans => {
            let mut trans = trans;
            if !options.print_name {
                let name = options.transducer_name.clone();
                if options.truncate_length > 0 {
                    // C: hfst_strndup copies at most TRUNCATE_LENGTH bytes.
                    let n = (options.truncate_length as usize).min(name.len());
                    let truncated = String::from_utf8_lossy(&name.as_bytes()[..n]).into_owned();
                    trans.set_name(&truncated);
                } else {
                    trans.set_name(&name);
                }
                if let Err(e) = outstream.redirect(&mut trans) {
                    eprintln!("hfst-name: {e}");
                    return 1;
                }
            } else {
                eprint!("\"{}\"\n", trans.get_name());
            }
        });
    }
    instream.close();
    outstream.close();
    0
}

// [spec:hfst:def:hfst-name.main-fn]
// [spec:hfst:sem:hfst-name.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstName");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    if !options.print_name && !options.name_option_given {
        eprint!("Error: hfst-name: use either option --print-name  or --name\n");
        return 1;
    }
    if options.print_name && options.name_option_given {
        eprint!("Warning: option --print-name overrides option --name\n");
    }

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
            eprintln!("hfst-name: {e}");
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
            eprintln!("hfst-name: {e}");
            return 1;
        }
    };

    process_stream(&common, &options, &mut instream, &mut outstream)
}
