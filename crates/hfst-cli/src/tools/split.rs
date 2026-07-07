//! Faithful 1:1 port of tools/src/hfst-split.cc — the transducer archive
//! exploding tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments).
//!
//! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
//! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
//! `parse_options` and threaded into the processing functions. There are no
//! `static mut` globals and no `unsafe`.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, extend_options_from_env, hfst_set_program_name, verbose_print,
};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{hfst_getopt_common_long, print_common_program_options};
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use std::io::Write;

/// hfst-split's own options (the former tool-specific `static mut`s).
struct Options {
    /// '-p, --prefix=PRE': prefix used in naming output files.
    prefix: String,
    /// '-e, --extension=EXT': extension used in naming output files.
    extension: String,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            prefix: String::new(),
            extension: ".hfst".to_string(),
        }
    }
}

// [spec:hfst:def:hfst-split.print-usage-fn]
// [spec:hfst:sem:hfst-split.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nExtract transducers from archive with systematic file names\n\n",
        common.program_name
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
}

// [spec:hfst:def:hfst-split.parse-options-fn]
// [spec:hfst:sem:hfst-split.parse-options-fn]
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
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd common case group, then this
        // tool's own input/output cases, then the terminal error arm.
        match handle_common_case(&mut common, &opt, c, print_usage) {
            CaseResult::Return(code) => return Err(code),
            CaseResult::Break => continue,
            CaseResult::NotHandled => {}
        }
        match c {
            c if c == b'i' as i32 => {
                common.input_filename = opt.optarg();
                // C: inputfile = hfst_fopen(inputfilename, "r"); if it resolves
                // to stdin ("-"), reset the name to "<stdin>". Otherwise the C
                // opened the file eagerly to validate it; mirror that by trying
                // to open it and erroring through the same path on failure.
                if common.input_filename == "-" {
                    common.input_filename = "<stdin>".to_string();
                } else if std::fs::File::open(&common.input_filename).is_err() {
                    error(
                        &common,
                        1,
                        0,
                        &format!("Could not open '{}'. ", common.input_filename),
                    );
                }
                common.input_named = true;
                continue;
            }
            c if c == b'p' as i32 => {
                options.prefix = opt.optarg();
                continue;
            }
            c if c == b'e' as i32 => {
                options.extension = opt.optarg();
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

// [spec:hfst:def:hfst-split.process-stream-fn]
// [spec:hfst:sem:hfst-split.process-stream-fn]
fn process_stream(
    common: &mut CommonOptions,
    options: &Options,
    instream: &mut HfstInputStream,
) -> i32 {
    let mut transducer_n: usize = 0;
    while instream.is_good() {
        transducer_n += 1;
        let outfilename = format!("{}{}{}", options.prefix, transducer_n, options.extension);
        common.output_filename = outfilename.clone();
        verbose_print(
            common,
            &format!(
                "Writing {} of {} to {}...\n",
                transducer_n, common.input_filename, outfilename
            ),
        );
        let mut outstream =
            match HfstOutputStream::new_filename(&outfilename, instream.get_type(), true) {
                Ok(s) => s,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
        let any = match instream.read() {
            Ok(t) => t,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
        crate::for_any!(any, trans => {
            let mut trans = trans;
            if let Err(e) = outstream.redirect(&mut trans) {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
            if let Err(e) = outstream.flush() {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
            outstream.close();
            common.output_filename = String::new();
        });
    }
    instream.close();
    0
}

// [spec:hfst:def:hfst-split.main-fn]
// [spec:hfst:sem:hfst-split.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstSplit");
    let (mut common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    // close buffers, we use streams
    verbose_print(
        &common,
        &format!(
            "Reading from {}, writing to {}...{}\n",
            common.input_filename, options.prefix, options.extension
        ),
    );
    // here starts the buffer handling part
    // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
    // currently panics on a bad file rather than throwing, so the catch arm
    // is not reproduced faithfully here.)
    let instream_result = if common.input_filename != "<stdin>" {
        HfstInputStream::new_filename(&common.input_filename)
    } else {
        HfstInputStream::new()
    };
    let mut instream = match instream_result {
        Ok(s) => s,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return 1;
        }
    };

    process_stream(&mut common, &options, &mut instream)
}
