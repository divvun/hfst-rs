//! Faithful 1:1 port of tools/src/hfst-guessify.cc — the tool for compiling a
//! guesser and model form generator from a morphological analyzer. Drives the
//! hfst-cli foundation (globals, getopt, commandline, program-options,
//! tool-metadata, inc fragments) and the ported hfst::guessify_fst library.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, extend_options_from_env, hfst_set_program_name, verbose_print,
};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options,
};
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use hfst::guessify_fst::{CATEGORY_SYMBOL_PREFIX, guessify_analyzer, store_guesser};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use std::io::Write;

/// hfst-guessify's own options (the former tool-specific `static mut`s).
struct Options {
    /// '-G, --do-not-compile-generator': compile a model form generator
    /// alongside the guesser (true by default; -G clears it).
    compile_generator: bool,
    /// '-p, --default-penalty': penalty for skipping one symbol of input.
    default_penalty: f32,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            compile_generator: true,
            default_penalty: 1.0,
        }
    }
}

// [spec:hfst:def:hfst-guessify.get-float-fn]
// [spec:hfst:sem:hfst-guessify.get-float-fn]
fn get_float(str: &str) -> f32 {
    // C: 'std::istringstream in(str); float f; in >> f; if (in.fail()) return -1;
    // return f;'. Mirror the formatted stream extraction: skip leading
    // whitespace, then consume as many leading characters as form a valid float
    // (trailing characters are ignored). On a failed extraction (no float could
    // be read) return -1.
    let trimmed = str.trim_start();
    // Find the longest leading prefix that parses as a float, mirroring the
    // greedy, character-by-character acceptance of istringstream's '>>' for a
    // float. Trailing characters after the number are ignored.
    let mut best: Option<f32> = None;
    for (i, _) in trimmed.char_indices() {
        if let Ok(f) = trimmed[..=i].parse::<f32>() {
            best = Some(f);
        }
    }
    match best {
        Some(f) => f,
        None => -1.0,
    }
}

// [spec:hfst:def:hfst-guessify.print-usage-fn]
// [spec:hfst:sem:hfst-guessify.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nCompile a morphological analyzer into a guesser and generator.\n\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Guesser options:\n  -p, --default-penalty           Give penalty for skipping one\n                                  symbol of input (1.0 by default).\n  -G, --do-not-compile-generator  When compiling the guesser, do\n                                  not compile a model form\n                                  generator.\n"
    );
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "All analyses in the morphological analyzer should have the form:\nw o r d f o r m POS {0}CLASS] X Y Z ...\nwhere POS is the part-of-speech tag, {0}CLASS]\nis an inflectional category marker and X, Y and Z are inflectional\nmarkers. The form of the inflectional category marker is fixed.\nCLASS can be any string, which doesn't contain \"]\".\n",
        CATEGORY_SYMBOL_PREFIX
    );
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "Using the option -d will reduce the size of the guesser file by\napproximately half, but may substantially increase the load time of\nthe guesser when generating model forms. If you only need to guess\nanalyses of unknown word forms, -d has no effect on load time.\n"
    );
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "If OUTFILE or INFILE is missing or -, standard streams will be used.\n"
    );
    let _ = write!(msg, "\n");
}

// [spec:hfst:def:hfst-guessify.parse-options-fn]
// [spec:hfst:sem:hfst-guessify.parse-options-fn]
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
            name: "default-penalty",
            has_arg: getopt::REQUIRED_ARGUMENT,
            val: 'p' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "do-not-compile-generator",
            has_arg: getopt::NO_ARGUMENT,
            val: 'G' as i32,
        });
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: common
        // cases, then unary cases, then the tool's own ('G'/'p'), then the
        // terminal error arm.
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
        match c {
            x if x == 'G' as i32 => {
                options.compile_generator = false;
                continue;
            }
            x if x == 'p' as i32 => {
                let optarg = opt.optarg();
                options.default_penalty = get_float(&optarg);

                if options.default_penalty < 0.0 {
                    error(
                        &common,
                        1,
                        0,
                        &format!("Invalid default penalty {}. Give a positive float.", optarg),
                    );
                }

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

// [spec:hfst:def:hfst-guessify.process-stream-fn]
// [spec:hfst:sem:hfst-guessify.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &Options,
    instream: &mut HfstInputStream<'_>,
    out: &mut HfstOutputStream,
) -> i32 {
    let mut transducer_n: usize = 0;
    while instream.is_good() {
        transducer_n += 1;
        let _ = transducer_n; // C++ counts but never reads it
        let any = match instream.read() {
            Ok(t) => t,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        // The C++ guessify_fst converted every input to tropical openfst
        // "so that all operations can be performed"; that conversion is
        // now this one typed extraction at the stream boundary
        // ([dec:hfst:monomorphic-backends]).
        let analyzer: HfstTransducer<hfst_openfst::StdVectorFst> = match any.into_typed() {
            Ok(t) => t,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };

        verbose_print(
            common,
            &format!(
                "Compiling guesser from the transducer {}.\n",
                analyzer.get_name()
            ),
        );
        let mut guesser = match guessify_analyzer(analyzer, options.default_penalty) {
            Ok(g) => g,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };

        if options.compile_generator {
            verbose_print(
                common,
                "Compiling generator and storing guesser and generator.\n",
            );
        } else {
            verbose_print(common, "Storing guesser.\n");
        }

        if let Err(e) = store_guesser(&mut guesser, out, options.compile_generator) {
            error(common, 1, 0, &format!("{e}"));
            return 1;
        }
    }

    instream.close();

    0
}

// [spec:hfst:def:hfst-guessify.main-fn]
// [spec:hfst:sem:hfst-guessify.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.3", "HfstGuessify");
    let (common, options) = match parse_options(common, &mut args) {
        Ok(v) => v,
        Err(code) => return code,
    };

    // close buffers, we use streams
    let input_opened = common.input_filename != "<stdin>";

    verbose_print(
        &common,
        &format!(
            "Reading from {}, writing to {}\n",
            common.input_filename, common.output_filename
        ),
    );

    // here starts the buffer handling part
    // (the C wraps the ctor in try/catch on HfstException reporting
    // "%s is not a valid transducer file"; the Rust ctor currently panics on
    // a bad file rather than throwing, so the catch arm is not reproduced.)
    let instream_result = if input_opened {
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

    let output_opened = common.output_filename != "<stdout>";
    // (the C wraps the ctor in try/catch on HfstException reporting
    // "%s cannot be opened for writing."; the Rust ctor currently panics
    // rather than throwing, so the catch arm is not reproduced here.)
    let outstream_result = if output_opened {
        HfstOutputStream::new_filename(
            &common.output_filename,
            ImplementationType::HFST_OLW_TYPE,
            true,
        )
    } else {
        HfstOutputStream::new(ImplementationType::HFST_OLW_TYPE, true)
    };
    let mut outstream = match outstream_result {
        Ok(s) => s,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return 1;
        }
    };

    process_stream(&common, &options, &mut instream, &mut outstream)
}
