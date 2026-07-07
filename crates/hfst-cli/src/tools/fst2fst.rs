//! Faithful 1:1 port of tools/src/hfst-fst2fst.cc — the format conversion
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments). A unary tool:
//! it reads one input stream and converts each transducer to another binary
//! implementation format.
//!
//! Idiomatic option handling: the tool's state lives in [`CommonOptions`] (the
//! shared `-v/-q/-o/-i/…` fields) and a tool-local [`Options`] — both built by
//! `parse_options` and threaded into the processing functions. There are no
//! `static mut` globals and no `unsafe`.

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    convert_any_with_options, error, extend_options_from_env, hfst_parse_format_name,
    hfst_set_program_name, hfst_strformat, verbose_print, warning,
};
use crate::hfst_getopt::{self as getopt, Getopt};
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use crate::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary, hfst_set_name_unary};
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use std::io::Write;

/// hfst-fst2fst's own options (the former tool-specific `static mut`s).
struct Options {
    /// output implementation format ('-f/-S/-F/-t/-l/-O/-w').
    output_type: ImplementationType,
    /// '-b/--use-backend-format': write in implementation format without HFST
    /// wrappers (default: true, i.e. write HFST3 headers).
    hfst_format: bool,
    /// '-Q/--quick': relax optimized-lookup table packing.
    options: String,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            output_type: ImplementationType::UNSPECIFIED_TYPE,
            hfst_format: true,
            options: String::new(),
        }
    }
}

// [spec:hfst:def:hfst-fst2fst.set-output-type-fn]
// [spec:hfst:sem:hfst-fst2fst.set-output-type-fn]
fn set_output_type(common: &CommonOptions, options: &mut Options, ty: ImplementationType) {
    if options.output_type != ImplementationType::UNSPECIFIED_TYPE {
        error(common, 1, 0, "Output type defined several times.");
    }
    options.output_type = ty;
}

// [spec:hfst:def:hfst-fst2fst.print-usage-fn]
// [spec:hfst:sem:hfst-fst2fst.print-usage-fn]
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nConvert transducers between binary formats\n\n",
        common.program_name
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Conversion options:\n\
         \u{20}\u{20}-f, --format=FMT                  Write result in FMT format\n\
         \u{20}\u{20}-b, --use-backend-format          Write result in implementation format, without any HFST wrappers\n\
         \u{20}\u{20}-S, --sfst                        Write output in (HFST's) SFST implementation\n\
         \u{20}\u{20}-F, --foma                        Write output in (HFST's) foma implementation\n\
         \u{20}\u{20}-x, --xfsm                        Write output in native xfsm format\n\
         \u{20}\u{20}-t, --openfst-tropical            Write output in (HFST's) tropical weight (OpenFST) implementation\n\
         \u{20}\u{20}-l, --openfst-log                 Write output in (HFST's) log weight (OpenFST) implementation\n\
         \u{20}\u{20}-O, --optimized-lookup-unweighted Write output in the HFST optimized-lookup implementation\n\
         \u{20}\u{20}-w, --optimized-lookup-weighted   Write output in optimized-lookup (weighted) implementation\n\
         \u{20}\u{20}-Q  --quick                       When converting to optimized-lookup, don't try hard to compress\n"
    );
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(
        msg,
        "FMT must be name of a format usable by libhfst, i.e. one of the following:\n\
         {{ foma, openfst-tropical, openfst-log, sfst, xfsm\n\
         \u{20}\u{20}optimized-lookup-weighted, optimized-lookup-unweighted }}.\n\
         Note that xfsm format is always written in native format without HFST wrappers.\n"
    );
    let _ = write!(msg, "\n");
}

// [spec:hfst:def:hfst-fst2fst.parse-options-fn]
// [spec:hfst:sem:hfst-fst2fst.parse-options-fn]
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
            name: "use-backend-format",
            has_arg: 0,
            val: b'b' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "format",
            has_arg: 1,
            val: b'f' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "sfst",
            has_arg: 0,
            val: b'S' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "foma",
            has_arg: 0,
            val: b'F' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "xfsm",
            has_arg: 0,
            val: b'x' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "openfst-tropical",
            has_arg: 0,
            val: b't' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "openfst-log",
            has_arg: 0,
            val: b'l' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "optimized-lookup-unweighted",
            has_arg: 0,
            val: b'O' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "optimized-lookup-weighted",
            has_arg: 0,
            val: b'w' as i32,
        });
        long_options.push(getopt::GetOpt {
            name: "quick",
            has_arg: 0,
            val: b'Q' as i32,
        });
        // add tool-specific options here
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: common
        // cases, then unary cases, then the tool's own cases, then the
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
        // add tool-specific cases here
        let ch = c as u8;
        match ch {
            b'f' => {
                let ty = hfst_parse_format_name(&common, &opt.optarg());
                set_output_type(&common, &mut options, ty);
                // HAVE_XFSM is not defined in this build: reject xfsm output.
                if options.output_type == ImplementationType::XFSM_TYPE {
                    error(&common, 1, 0, "xfsm back-end is not available");
                }
                continue;
            }
            b'b' => {
                options.hfst_format = false;
                continue;
            }
            b'S' => {
                set_output_type(&common, &mut options, ImplementationType::SFST_TYPE);
                continue;
            }
            b'F' => {
                set_output_type(&common, &mut options, ImplementationType::FOMA_TYPE);
                continue;
            }
            b'x' => {
                // HAVE_XFSM is not defined in this build.
                error(&common, 1, 0, "xfsm back-end is not available");
                continue;
            }
            b't' => {
                set_output_type(
                    &common,
                    &mut options,
                    ImplementationType::TROPICAL_OPENFST_TYPE,
                );
                continue;
            }
            b'l' => {
                set_output_type(&common, &mut options, ImplementationType::LOG_OPENFST_TYPE);
                continue;
            }
            b'O' => {
                set_output_type(&common, &mut options, ImplementationType::HFST_OL_TYPE);
                continue;
            }
            b'w' => {
                set_output_type(&common, &mut options, ImplementationType::HFST_OLW_TYPE);
                continue;
            }
            b'Q' => {
                options.options = "quick".to_string();
                continue;
            }
            _ => {}
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    if options.output_type == ImplementationType::UNSPECIFIED_TYPE {
        error(
            &common,
            1,
            0,
            "You must specify an output type (one of -S, -F, -t, -x, -l, -O, or -w)",
        );
    }

    check_common_params(&mut common);
    check_unary_params(&mut common, &opt, args);
    Ok((common, options))
}

// [spec:hfst:def:hfst-fst2fst.process-stream-fn]
// [spec:hfst:sem:hfst-fst2fst.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &Options,
    instream: &mut HfstInputStream<'_>,
    outstream: &mut HfstOutputStream,
) -> i32 {
    if instream.get_type() == ImplementationType::FOMA_TYPE && !instream.is_hfst_header_included() {
        if !common.silent {
            warning(
                common,
                0,
                0,
                "converting native foma transducer: \
                 inversion may be needed for hfst-lookup to work as expected \
                 (hfst-flookup works as foma's flookup)\n",
            );
        }
    }

    let mut transducer_n: usize = 0;
    while instream.is_good() {
        transducer_n += 1;
        let orig = match instream.read() {
            Ok(v) => v,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };

        let inputname = hfst_get_name(&orig, &common.input_filename);
        if transducer_n == 1 {
            verbose_print(common, &format!("Converting {}...\n", inputname));
        } else {
            verbose_print(
                common,
                &format!("Converting {}...{}\n", inputname, transducer_n),
            );
        }
        // The typed cross-format conversion at the stream boundary
        // ([dec:hfst:monomorphic-backends]): to_basic/from_basic between
        // the algebra backends, to_ol(weighted, options) for OL output.
        // C wraps the conversion in try/catch on HfstException; the Rust
        // conversion currently panics rather than throwing, so the catch arm
        // is not reproduced here.
        let converted = match convert_any_with_options(orig, options.output_type, &options.options)
        {
            Ok(v) => v,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        // C: hfst_set_name(orig, orig, "convert"); the dest and src are the
        // same object, which Rust cannot alias mut+const, so the read side is
        // taken from a copy (name/formula are unchanged by the copy).
        let code = crate::for_any!(converted, orig => {
            let mut orig = orig;
            let src = orig.clone();
            hfst_set_name_unary(&mut orig, &src, "convert");
            hfst_set_formula_unary(&mut orig, &src, "Id");
            if let Err(e) = outstream.redirect(&mut orig) {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
            0
        });
        if code != 0 {
            return code;
        }
    }
    if let Err(e) = outstream.flush() {
        // needed for xfsm transducers whose writing is delayed
        error(common, 1, 0, &format!("{e}"));
        return 1;
    }
    instream.close();
    outstream.close();
    0
}

// [spec:hfst:def:hfst-fst2fst.main-fn]
// [spec:hfst:sem:hfst-fst2fst.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstFst2Fst");
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
    if options.hfst_format && (options.output_type != ImplementationType::XFSM_TYPE) {
        verbose_print(
            &common,
            &format!(
                "Writing {} format transducers with HFST3 headers\n",
                hfst_strformat(options.output_type)
            ),
        );
    } else {
        verbose_print(
            &common,
            &format!(
                "Writing {} format transducers without HFST specific headers\n",
                hfst_strformat(options.output_type)
            ),
        );
    }

    if options.output_type == ImplementationType::XFSM_TYPE {
        if common.output_filename == "<stdout>" {
            error(
                &common,
                1,
                0,
                "Writing to standard output not supported for xfsm transducers,\n\
                 use 'hfst-fst2fst [--output|-o] OUTFILE' instead",
            );
            return 1;
        }
    }

    // here starts the buffer handling part
    // (the C wraps the ctor in try/catch on FileIsInGZFormatException,
    // ImplementationTypeNotAvailableException and HfstException; the Rust
    // ctor currently panics rather than throwing, so the catch arms are not
    // reproduced here.)
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

    let mut outstream = match if output_opened {
        HfstOutputStream::new_filename(
            &common.output_filename,
            options.output_type,
            options.hfst_format,
        )
    } else {
        HfstOutputStream::new(options.output_type, options.hfst_format)
    } {
        Ok(v) => v,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return 1;
        }
    };

    process_stream(&common, &options, &mut instream, &mut outstream)
}
