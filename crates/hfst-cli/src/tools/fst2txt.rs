//! Faithful 1:1 port of tools/src/hfst-fst2txt.cc — the transducer array
//! printing command-line tool. Prints a transducer in AT&T, dot, prolog or
//! pckimmo text format. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, inc fragments).

use crate::globals;
use crate::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_from_env, hfst_set_program_name, verbose_print,
};
use crate::hfst_getopt as getopt;
use crate::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options,
};
use crate::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use hfst::hfst_data_types::ImplementationType;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_print_dot::print_dot_file;
use hfst::hfst_print_pckimmo::print_pckimmo;
use hfst::hfst_transducer::HfstTransducer;
use std::io::Write;

// add tools-specific variables here
static mut USE_NUMBERS: bool = false;
static mut PRINT_WEIGHTS: bool = false;
static mut DO_NOT_PRINT_WEIGHTS: bool = false;

// [spec:hfst:def:hfst-fst2txt.fst-text-format]
#[derive(Clone, Copy, PartialEq, Eq)]
enum FstTextFormat {
    AttText,     // AT&T / OpenFst compatible TSV
    DotText,     // Graphviz / dotty
    PckimmoText, // PCKIMMO format
    PrologText,  // prolog format
}

static mut FORMAT: FstTextFormat = FstTextFormat::AttText;

// [spec:hfst:def:hfst-fst2txt.print-usage-fn]
// [spec:hfst:sem:hfst-fst2txt.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nPrint transducer in AT&T, dot, prolog or pckimmo format\n\n",
        globals::program_name()
    );
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Text format options:\n  -w, --print-weights          If weights are printed in all cases\n  -D, --do-not-print-weights   If weights are not printed in any case\n  -f, --format=TFMT            Print output in TFMT format [default=att]\n"
    );
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "If OUTFILE or INFILE is missing or -, standard streams will be used.\nUnless explicitly requested with option -w or -D, weights are printed\nif and only if the transducer is in weighted format.\nTFMT is one of {{att, dot, prolog, pckimmo}}.\n"
    );
    let _ = write!(msg, "\n");
}

// [spec:hfst:def:hfst-fst2txt.parse-options-fn]
// [spec:hfst:sem:hfst-fst2txt.parse-options-fn]
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
                name: "print-weights",
                has_arg: 0, // no_argument
                val: 'w' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "do-not-print-weights",
                has_arg: 0, // no_argument
                val: 'D' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "use-numbers",
                has_arg: 0, // no_argument
                val: 'n' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "format",
                has_arg: 1, // required_argument
                val: 'f' as i32,
            });
            // add tool-specific options here
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own, then the terminal
            // error arm.
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
            match c {
                x if x == 'w' as i32 => {
                    PRINT_WEIGHTS = true;
                    continue;
                }
                x if x == 'D' as i32 => {
                    DO_NOT_PRINT_WEIGHTS = true;
                    continue;
                }
                x if x == 'n' as i32 => {
                    USE_NUMBERS = true;
                    continue;
                }
                x if x == 'f' as i32 => {
                    let optarg = getopt::optarg();
                    if optarg == "att"
                        || optarg == "AT&T"
                        || optarg == "openfst"
                        || optarg == "OpenFst"
                    {
                        FORMAT = FstTextFormat::AttText;
                    } else if optarg == "dot" || optarg == "graphviz" || optarg == "GraphViz" {
                        FORMAT = FstTextFormat::DotText;
                    } else if optarg == "pckimmo" {
                        FORMAT = FstTextFormat::PckimmoText;
                    } else if optarg == "prolog" || optarg == "Prolog" {
                        FORMAT = FstTextFormat::PrologText;
                    } else {
                        error(
                            1,
                            0,
                            &format!(
                                "Cannot parse {} as text format; Use one of att, pckimmo, dot, prolog",
                                optarg
                            ),
                        );
                    }
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

// [spec:hfst:def:hfst-fst2txt.process-stream-fn]
// [spec:hfst:sem:hfst-fst2txt.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream, outf: &mut dyn std::io::Write) -> i32 {
    unsafe {
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            // C: catches TransducerTypeMismatchException -> error "input
            // transducers do not have the same type"; the Rust ctor currently
            // panics rather than throwing, so the catch arm is not reproduced.
            let any = match instream.read() {
                Ok(t) => t,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            };
            // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
            let code =
                crate::for_any!(any, t => process_one(t, outf, transducer_n, instream.get_type()));
            if code != 0 {
                return code;
            }
        }
        instream.close();
        0
    }
}

// The per-transducer body, generic over the backend (text output only needs
// the common Backend surface).
unsafe fn process_one<B: hfst::backend::Backend>(
    mut t: HfstTransducer<B>,
    outf: &mut dyn std::io::Write,
    transducer_n: usize,
    stream_type: ImplementationType,
) -> i32 {
    unsafe {
        {
            let mut inputname = t.get_name();
            if inputname.is_empty() {
                inputname = globals::input_filename();
            }
            if transducer_n == 1 {
                verbose_print(&format!("Converting {}...\n", inputname));
            } else {
                if stream_type == ImplementationType::XFSM_TYPE {
                    error(
                        1,
                        0,
                        "Writing more than one transducer in text format to file not supported for xfsm transducers,\nuse [hfst-head|hfst-tail|hfst-split] to extract individual transducers from input",
                    );
                    return 1;
                }
                verbose_print(&format!("Converting {}...{}\n", inputname, transducer_n));
            }

            if transducer_n > 1 {
                let _ = outf.write_all(b"--\n");
            }

            let printw: bool; // whether weights are printed
            let ty = t.get_type();
            if PRINT_WEIGHTS {
                printw = true;
            } else if DO_NOT_PRINT_WEIGHTS {
                printw = false;
            } else if ty == ImplementationType::SFST_TYPE
                || ty == ImplementationType::FOMA_TYPE
                || ty == ImplementationType::XFSM_TYPE
            {
                printw = false;
            } else if ty.is_weighted() {
                // tropical/log OpenFST and weighted optimized-lookup; the prior
                // SFST/foma/xfsm arm already returned false, and the else arm
                // below also yields true, so this is byte-for-byte equivalent to
                // the original `ty == TROPICAL_OPENFST || ty == LOG_OPENFST`.
                printw = true;
            } else {
                // this should not happen
                printw = true;
            }
            let write_result = match FORMAT {
                FstTextFormat::AttText => {
                    if USE_NUMBERS {
                        // xfsm case checked earlier
                        t.write_in_att_format_number(outf, printw)
                    } else {
                        // xfsm not yet supported
                        t.write_in_att_format_file(outf, printw)
                    }
                }
                FstTextFormat::DotText => {
                    // xfsm case checked earlier
                    outf.write_all(b"// This graph generated with hfst-fst2txt\n")
                        .and_then(|()| print_dot_file(outf, &mut t))
                }
                FstTextFormat::PckimmoText => {
                    // xfsm case checked earlier
                    print_pckimmo(outf, &t)
                }
                FstTextFormat::PrologText => {
                    // C: catches HfstException -> error "Error encountered when
                    // writing in prolog format". The Rust impl panics; the catch
                    // arm is not reproduced here.
                    if ty == ImplementationType::XFSM_TYPE {
                        // XFSM streams cannot be read in this build (the
                        // backend is compiled out); the C++ arm called
                        // write_xfsm_transducer_in_prolog_format here.
                        unreachable!("XFSM_TYPE cannot be read from an HFST stream in this build")
                    } else {
                        let namestr = t.get_name();
                        let alt_namestr = format!("NO_NAME_{}", transducer_n);
                        let namestr = if namestr.is_empty() {
                            if !globals::SILENT {
                                eprint!(
                                    "Transducer has no name, giving it a name '{}'...\n",
                                    alt_namestr
                                );
                            }
                            alt_namestr
                        } else {
                            if !globals::SILENT {
                                eprint!("Renaming transducer into '{}'...\n", alt_namestr);
                            }
                            alt_namestr
                        };
                        if let Err(e) = t.write_in_prolog_format(outf, &namestr, printw) {
                            error(
                                1,
                                0,
                                &format!("Error encountered when writing in prolog format: {e}"),
                            );
                            return 1;
                        }
                        Ok(())
                    }
                }
            };
            if let Err(e) = write_result {
                error(
                    1,
                    0,
                    &format!("Error encountered when writing in text format: {e}"),
                );
                return 1;
            }
            // C: delete t; (Rust drops at end of loop iteration).
        }
        0
    }
}

// [spec:hfst:def:hfst-fst2txt.main-fn]
// [spec:hfst:sem:hfst-fst2txt.main-fn]
pub fn run(args: Vec<String>) -> i32 {
    unsafe { real_main(args) }
}

unsafe fn real_main(mut args: Vec<String>) -> i32 {
    unsafe {
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.3", "HfstFst2Txt");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let input_opened = globals::input_filename() != "<stdin>";

        verbose_print(&format!(
            "Reading from {}, writing to {}\n",
            globals::input_filename(),
            globals::output_filename()
        ));
        // here starts the buffer handling part
        // (the C wraps the ctor in try/catch on HfstException -> error
        // "%s is not a valid transducer file"; the Rust ctor currently panics
        // rather than throwing, so the catch arm is not reproduced here.)
        let mut instream = match if input_opened {
            HfstInputStream::new_filename(&globals::input_filename())
        } else {
            HfstInputStream::new()
        } {
            Ok(s) => s,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        };

        if instream.get_type() == ImplementationType::XFSM_TYPE {
            if FORMAT == FstTextFormat::DotText {
                error(
                    1,
                    0,
                    "Output format 'dot' not supported for xfsm transducers, use 'prolog'",
                );
                return 1;
            }
            if FORMAT == FstTextFormat::PckimmoText {
                error(
                    1,
                    0,
                    "Output format 'pckimmo' not supported for xfsm transducers, use 'prolog'",
                );
                return 1;
            }
            if FORMAT == FstTextFormat::AttText {
                error(
                    1,
                    0,
                    "Output format 'att' not supported for xfsm transducers, use 'prolog'",
                );
                return 1;
            }
            if USE_NUMBERS {
                error(
                    1,
                    0,
                    "Option '--use-numbers' not supported for xfsm transducers",
                );
                return 1;
            }
            if globals::input_filename() == "<stdin>" {
                error(
                    1,
                    0,
                    "Reading from standard input not supported for xfsm transducers,\nuse 'hfst-fst2txt [--input|-i] INFILE' instead",
                );
                return 1;
            }
            if globals::output_filename() == "<stdout>" {
                error(
                    1,
                    0,
                    "Writing to standard output not supported for xfsm transducers,\nuse 'hfst-fst2txt [--output|-o] OUTFILE' instead",
                );
                return 1;
            }
        }

        let mut out = match globals::output_writer() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("hfst-fst2txt: cannot open output: {e}");
                return 1;
            }
        };
        let retval = process_stream(&mut instream, &mut *out);

        // C: free(inputfilename); free(outfilename); (the foundation owns these
        // allocations; not freed here).
        retval
    }
}
