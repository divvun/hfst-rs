#![allow(static_mut_refs)]
//! Faithful 1:1 port of tools/src/hfst-fst2fst.cc — the format conversion
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, tool-metadata, inc fragments). A unary tool:
//! it reads one input stream and converts each transducer to another binary
//! implementation format.

use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_getenv, hfst_parse_format_name, hfst_set_program_name,
    hfst_strformat, print_more_info, print_report_bugs, verbose_print, warning,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use hfst_cli::hfst_tool_metadata::{hfst_get_name, hfst_set_formula_unary, hfst_set_name_unary};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use std::io::Write;

// tool-specific variables
static mut OUTPUT_TYPE: ImplementationType = ImplementationType::UNSPECIFIED_TYPE;
static mut HFST_FORMAT: bool = true;
static mut OPTIONS: String = String::new();

// [spec:hfst:def:hfst-fst2fst.set-output-type-fn]
// [spec:hfst:sem:hfst-fst2fst.set-output-type-fn]
unsafe fn set_output_type(ty: ImplementationType) {
    unsafe {
        if OUTPUT_TYPE != ImplementationType::UNSPECIFIED_TYPE {
            error(1, 0, "Output type defined several times.");
        }
        OUTPUT_TYPE = ty;
    }
}

// [spec:hfst:def:hfst-fst2fst.print-usage-fn]
// [spec:hfst:sem:hfst-fst2fst.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nConvert transducers between binary formats\n\n",
        globals::program_name()
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
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-fst2fst.parse-options-fn]
// [spec:hfst:sem:hfst-fst2fst.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_getenv(args);
        // use of this function requires options are settable on global scope
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
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own cases, then the
            // terminal error arm.
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
            // add tool-specific cases here
            let ch = c as u8;
            match ch {
                b'f' => {
                    set_output_type(hfst_parse_format_name(&getopt::optarg()));
                    // HAVE_XFSM is not defined in this build: reject xfsm output.
                    if OUTPUT_TYPE == ImplementationType::XFSM_TYPE {
                        error(1, 0, "xfsm back-end is not available");
                    }
                    continue;
                }
                b'b' => {
                    HFST_FORMAT = false;
                    continue;
                }
                b'S' => {
                    set_output_type(ImplementationType::SFST_TYPE);
                    continue;
                }
                b'F' => {
                    set_output_type(ImplementationType::FOMA_TYPE);
                    continue;
                }
                b'x' => {
                    // HAVE_XFSM is not defined in this build.
                    error(1, 0, "xfsm back-end is not available");
                    continue;
                }
                b't' => {
                    set_output_type(ImplementationType::TROPICAL_OPENFST_TYPE);
                    continue;
                }
                b'l' => {
                    set_output_type(ImplementationType::LOG_OPENFST_TYPE);
                    continue;
                }
                b'O' => {
                    set_output_type(ImplementationType::HFST_OL_TYPE);
                    continue;
                }
                b'w' => {
                    set_output_type(ImplementationType::HFST_OLW_TYPE);
                    continue;
                }
                b'Q' => {
                    OPTIONS = "quick".to_string();
                    continue;
                }
                _ => {}
            }
            return handle_error_case(c);
        }

        if OUTPUT_TYPE == ImplementationType::UNSPECIFIED_TYPE {
            error(
                1,
                0,
                "You must specify an output type (one of -S, -F, -t, -x, -l, -O, or -w)",
            );
        }

        check_common_params();
        check_unary_params(args);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-fst2fst.process-stream-fn]
// [spec:hfst:sem:hfst-fst2fst.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream, outstream: &mut HfstOutputStream) -> i32 {
    unsafe {
        if instream.get_type() == ImplementationType::FOMA_TYPE
            && !instream.is_hfst_header_included()
        {
            if !globals::SILENT {
                warning(
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
            let mut orig = match HfstTransducer::new_from_stream(instream) {
                Ok(v) => v,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            };

            let inputname = hfst_get_name(&orig, &globals::input_filename());
            if transducer_n == 1 {
                verbose_print(&format!("Converting {}...\n", inputname));
            } else {
                verbose_print(&format!("Converting {}...{}\n", inputname, transducer_n));
            }
            // C wraps the conversion in try/catch on HfstException; the Rust
            // conversion currently panics rather than throwing, so the catch arm
            // is not reproduced here.
            if let Err(e) = orig.convert(OUTPUT_TYPE, OPTIONS.clone()) {
                error(1, 0, &format!("{e}"));
                return 1;
            }
            // C: hfst_set_name(orig, orig, "convert"); the dest and src are the
            // same object, which Rust cannot alias mut+const, so the read side is
            // taken from a copy (name/formula are unchanged by the copy).
            let src = orig.clone();
            hfst_set_name_unary(&mut orig, &src, "convert");
            hfst_set_formula_unary(&mut orig, &src, "Id");
            if let Err(e) = outstream.redirect(&mut orig) {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        }
        if let Err(e) = outstream.flush() {
            // needed for xfsm transducers whose writing is delayed
            error(1, 0, &format!("{e}"));
            return 1;
        }
        instream.close();
        outstream.close();
        0
    }
}

// [spec:hfst:def:hfst-fst2fst.main-fn]
// [spec:hfst:sem:hfst-fst2fst.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstFst2Fst");
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
        if HFST_FORMAT && (OUTPUT_TYPE != ImplementationType::XFSM_TYPE) {
            verbose_print(&format!(
                "Writing {} format transducers with HFST3 headers\n",
                hfst_strformat(OUTPUT_TYPE)
            ));
        } else {
            verbose_print(&format!(
                "Writing {} format transducers without HFST specific headers\n",
                hfst_strformat(OUTPUT_TYPE)
            ));
        }

        if OUTPUT_TYPE == ImplementationType::XFSM_TYPE {
            if globals::output_filename() == "<stdout>" {
                error(
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

        let mut outstream = match if output_opened {
            HfstOutputStream::new_filename(&globals::output_filename(), OUTPUT_TYPE, HFST_FORMAT)
        } else {
            HfstOutputStream::new(OUTPUT_TYPE, HFST_FORMAT)
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
