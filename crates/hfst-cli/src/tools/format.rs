//! Faithful 1:1 port of tools/src/hfst-format.cc — the format-checking
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).
//!
//! This tool is unusual: it #includes globals-common.h and globals-unary.h
//! (so it is a unary tool), but it does the bulk of its work inside
//! parse_options (listing formats, testing a format, or opening the input
//! stream to report its type) and has no process_stream. main is therefore
//! very thin and simply prints the type returned by parse_options.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    extend_options_from_env, hfst_set_program_name, hfst_strformat, verbose_print,
};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use crate::inc::{CaseResult, handle_common_case, handle_unary_case};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_transducer::is_implementation_type_available;
use std::io::Write;

/// hfst-format's own options (the former tool-specific `static mut`s).
#[derive(Default)]
struct Options {
    /// '-l, --list-formats': list available transducer formats.
    list_formats: bool,
    /// '-t, --test-format FMT': the format to test. C used a NULL char* as
    /// "no format requested"; modelled as Option.
    format_to_test: Option<String>,
}

// fprintf(stdout, ...): write to file descriptor 1.
fn fput_stdout(s: &str) {
    let _ = std::io::stdout().write_all(s.as_bytes());
    let _ = std::io::stdout().flush();
}

// fprintf(stderr, ...): write to file descriptor 2.
fn fput_stderr(s: &str) {
    let _ = std::io::stderr().write_all(s.as_bytes());
    let _ = std::io::stderr().flush();
}

// [spec:hfst:def:hfst-format.print-usage-fn]
// [spec:hfst:sem:hfst-format.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    // c.f.
    // http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\ndetermine HFST transducer format\n\n",
        common.program_name
    );

    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Tool-specific options:\n  -l, --list-formats     List available transducer formats\n                         and print them to standard output\n"
    );
    let _ = write!(
        msg,
        "  -t, --test-format FMT  Whether the format FMT is available,\n                         exits with 0 if it is, else with 1\n"
    );
    let _ = writeln!(msg);
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = writeln!(msg);
}

// [spec:hfst:def:hfst-format.parse-options-fn]
// [spec:hfst:sem:hfst-format.parse-options-fn]
//
// This tool does the bulk of its work here (listing formats, testing a format,
// or opening the input stream to report its type) and returns the (updated)
// shared options plus the resolved transducer type; the terminal arms
// `std::process::exit` directly.
fn parse_options(
    mut common: CommonOptions,
    args: &mut Vec<String>,
) -> (CommonOptions, ImplementationType) {
    let mut options = Options::default();
    let mut opt = Getopt::new();
    extend_options_from_env(args);
    loop {
        let mut long_options: Vec<getopt::GetOpt> = Vec::new();
        long_options.extend(hfst_getopt_common_long());
        long_options.extend(hfst_getopt_unary_long());
        long_options.push(getopt::GetOpt {
            name: "input1",
            has_arg: 1,
            val: '1' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "input2",
            has_arg: 1,
            val: '2' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "list-formats",
            has_arg: 0,
            val: 'l' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "test-format",
            has_arg: 1,
            val: 't' as i32,
        });
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: common
        // cases, then unary cases, then the tool's own cases, then the
        // terminal default arm (which here is a no-op, NOT the error arm).
        match handle_common_case(&mut common, &opt, c, print_usage) {
            CaseResult::Return(code) => std::process::exit(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        match handle_unary_case(&mut common, &opt, c) {
            CaseResult::Return(code) => std::process::exit(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        let ch = char::from_u32(c as u32);
        match ch {
            Some('1') => {
                common.input_filename = opt.optarg();
                continue;
            }
            Some('2') => {
                common.input_filename = opt.optarg();
                continue;
            }
            Some('l') => {
                options.list_formats = true;
                continue;
            }
            Some('t') => {
                options.format_to_test = Some(opt.optarg());
                continue;
            }
            _ => {
                // I suppose it's crucial for this tool to ignore other options.
                // Unlike most tools, the default arm here is a genuine no-op
                // (the C 'default: break;'), NOT the common error handler.
                continue;
            }
        }
    }

    if let Some(fmt) = options.format_to_test.clone() {
        if (fmt == "sfst" && is_implementation_type_available(ImplementationType::SFST_TYPE))
            || (fmt == "openfst-tropical"
                && is_implementation_type_available(ImplementationType::TROPICAL_OPENFST_TYPE))
            || (fmt == "foma" && is_implementation_type_available(ImplementationType::FOMA_TYPE))
            || (fmt == "optimized-lookup-unweighted"
                && is_implementation_type_available(ImplementationType::HFST_OL_TYPE))
            || (fmt == "optimized-lookup-weighted"
                && is_implementation_type_available(ImplementationType::HFST_OLW_TYPE))
            || (fmt == "thfst" && is_implementation_type_available(ImplementationType::THFST_TYPE))
        {
            std::process::exit(0);
        }
        std::process::exit(1);
    }

    if options.list_formats {
        fput_stdout(" Backend                         Names recognized\n\n");

        if is_implementation_type_available(ImplementationType::SFST_TYPE) {
            fput_stdout(" SFST                            sfst\n");
        }

        if is_implementation_type_available(ImplementationType::TROPICAL_OPENFST_TYPE) {
            fput_stdout(
                " OpenFst (tropical weights)      openfst-tropical, openfst, ofst, ofst-tropical\n",
            );
        }

        if is_implementation_type_available(ImplementationType::FOMA_TYPE) {
            fput_stdout(" foma                            foma\n");
        }

        if is_implementation_type_available(ImplementationType::HFST_OL_TYPE) {
            fput_stdout(" Optimized lookup (weighted)     optimized-lookup-unweighted, olu\n");
        }

        if is_implementation_type_available(ImplementationType::HFST_OLW_TYPE) {
            fput_stdout(
                " Optimized lookup (unweighted)   optimized-lookup-weighted, olw, optimized-lookup, ol\n",
            );
        }

        if is_implementation_type_available(ImplementationType::THFST_TYPE) {
            fput_stdout(" THFST (divvunspell speller format)          thfst\n");
        }

        std::process::exit(0);
    }

    // (void)inputfilename; (void)inputNamed;

    // The C wraps the stream opening in try/catch on HfstException; on a
    // non-transducer stream it prints an error and exit(1). The Rust ctor
    // currently panics rather than throwing, so the catch arm is mirrored
    // by catching the panic.
    let optind = opt.optind;
    let remaining = args.len() - optind;
    let free_arg = if remaining == 1 {
        Some(args[optind].clone())
    } else {
        None
    };
    let input_filename = common.input_filename.clone();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || -> Result<(ImplementationType, String), hfst::error::Error> {
            if input_filename.is_empty() {
                if remaining == 0 {
                    let is = HfstInputStream::new()?;
                    return Ok((is.get_type(), "<stdin>".to_string()));
                } else if remaining == 1 {
                    let resolved = free_arg
                        .clone()
                        .expect("free_arg is Some when exactly one free argument remains");
                    let is = HfstInputStream::new_filename(&resolved)?;
                    return Ok((is.get_type(), resolved));
                }
            }
            let is = HfstInputStream::new_filename(&input_filename)?;
            Ok((is.get_type(), input_filename.clone()))
        },
    ));

    match result {
        Ok(Ok((t, resolved))) => {
            common.input_filename = resolved;
            (common, t)
        }
        Ok(Err(_)) | Err(_) => {
            fput_stderr("ERROR: The file/stream does not contain transducers.\n");
            std::process::exit(1);
        }
    }
}

// [spec:hfst:def:hfst-format.main-fn]
// [spec:hfst:sem:hfst-format.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let mut common = hfst_set_program_name(&argv0, "0.1", "HfstFormat");
    common.verbose = true;
    let (common, ty) = parse_options(common, &mut args);
    verbose_print(
        &common,
        &format!(
            "Transducers in {} are of type {}\n",
            common.input_filename,
            hfst_strformat(ty)
        ),
    );
    0
}
