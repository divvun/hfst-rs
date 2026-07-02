#![allow(static_mut_refs)]
//! Faithful 1:1 port of tools/src/hfst-split.cc — the transducer archive
//! exploding tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).

use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, extend_options_getenv, hfst_set_program_name, print_more_info,
    print_report_bugs, verbose_print,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{hfst_getopt_common_long, print_common_program_options};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
};
use std::io::Write;

// add tools-specific variables here
static mut PREFIX: String = String::new();
static mut EXTENSION: String = String::new();

fn prefix() -> String {
    unsafe { (*std::ptr::addr_of!(PREFIX)).clone() }
}
fn extension() -> String {
    unsafe { (*std::ptr::addr_of!(EXTENSION)).clone() }
}

// [spec:hfst:def:hfst-split.print-usage-fn]
// [spec:hfst:sem:hfst-split.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nExtract transducers from archive with systematic file names\n\n",
        globals::program_name()
    );
    print_common_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Input/Output options:\n  -i, --input=INFILE    Read input transducer from INFILE\n  -p, --prefix=PRE      Use the prefix PRE in naming output files\n  -e, --extension=EXT   Use the extension EXT in naming output files\n"
    );
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "If INFILE is omitted or -, stdin is used.\nIf PRE is omitted, no prefix is used.\nIf EXT is omitted, .hfst is used.\nThe extracted files are named \"PRE\" + N + \"EXT\",\nwhere N is the number of the transducer in the archive.\n\nAn example:\n   cat transducer_a transducer_b | hfst-split -p \"rule\" -e \".tr\"\n\nThis command creates files \"rule1.tr\" (equivalent to transducer_a)\nand \"rule2.tr\" (equivalent to transducer_b). \n"
    );
    let _ = write!(msg, "\n");
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-split.parse-options-fn]
// [spec:hfst:sem:hfst-split.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_getenv(args);
        // use of this function requires options are settable on global scope
        *std::ptr::addr_of_mut!(EXTENSION) = ".hfst".to_string();
        *std::ptr::addr_of_mut!(PREFIX) = String::new();
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            // add tool-specific options here
            long_options.push(getopt::GetOpt {
                name: "input",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: b'i' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "prefix",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: b'p' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "extension",
                has_arg: getopt::REQUIRED_ARGUMENT,
                val: b'e' as i32,
            });
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd common case group, then this
            // tool's own input/output cases, then the terminal error arm.
            match handle_common_case(c, print_usage) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match c {
                c if c == b'i' as i32 => {
                    globals::set_input_filename(getopt::optarg());
                    // C: inputfile = hfst_fopen(inputfilename, "r"); if it resolves
                    // to stdin ("-"), reset the name to "<stdin>". Otherwise the C
                    // opened the file eagerly to validate it; mirror that by trying
                    // to open it and erroring through the same path on failure.
                    if globals::input_filename() == "-" {
                        globals::set_input_filename("<stdin>");
                    } else if std::fs::File::open(globals::input_filename()).is_err() {
                        hfst_cli::hfst_commandline::error(
                            1,
                            0,
                            &format!("Could not open '{}'. ", globals::input_filename()),
                        );
                    }
                    globals::INPUT_NAMED = true;
                    continue;
                }
                c if c == b'p' as i32 => {
                    *std::ptr::addr_of_mut!(PREFIX) = getopt::optarg();
                    continue;
                }
                c if c == b'e' as i32 => {
                    *std::ptr::addr_of_mut!(EXTENSION) = getopt::optarg();
                    continue;
                }
                _ => {}
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(args);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-split.process-stream-fn]
// [spec:hfst:sem:hfst-split.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream) -> i32 {
    unsafe {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let outfilename = format!("{}{}{}", prefix(), transducer_n, extension());
            globals::set_output_filename(outfilename.clone());
            verbose_print(&format!(
                "Writing {} of {} to {}...\n",
                transducer_n,
                globals::input_filename(),
                outfilename
            ));
            let mut outstream =
                match HfstOutputStream::new_filename(&outfilename, instream.get_type(), true) {
                    Ok(s) => s,
                    Err(e) => {
                        hfst_cli::hfst_commandline::error(1, 0, &format!("{e}"));
                        return 1;
                    }
                };
            let mut trans = match HfstTransducer::new_from_stream(instream) {
                Ok(t) => t,
                Err(e) => {
                    hfst_cli::hfst_commandline::error(1, 0, &format!("{e}"));
                    return 1;
                }
            };
            if let Err(e) = outstream.redirect(&mut trans) {
                hfst_cli::hfst_commandline::error(1, 0, &format!("{e}"));
                return 1;
            }
            if let Err(e) = outstream.flush() {
                hfst_cli::hfst_commandline::error(1, 0, &format!("{e}"));
                return 1;
            }
            outstream.close();
            globals::set_output_filename("");
        }
        instream.close();
        0
    }
}

// [spec:hfst:def:hfst-split.main-fn]
// [spec:hfst:sem:hfst-split.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstSplit");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        verbose_print(&format!(
            "Reading from {}, writing to {}...{}\n",
            globals::input_filename(),
            prefix(),
            extension()
        ));
        // here starts the buffer handling part
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced faithfully here.)
        let instream_result = if globals::input_filename() != "<stdin>" {
            HfstInputStream::new_filename(&globals::input_filename())
        } else {
            HfstInputStream::new()
        };
        let mut instream = match instream_result {
            Ok(s) => s,
            Err(e) => {
                hfst_cli::hfst_commandline::error(1, 0, &format!("{e}"));
                return 1;
            }
        };

        process_stream(&mut instream)
    }
}
