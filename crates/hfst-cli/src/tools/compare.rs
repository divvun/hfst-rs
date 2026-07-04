//! Faithful 1:1 port of tools/src/hfst-compare.cc — the transducer comparison
//! command-line tool. Drives the hfst-cli foundation (globals, getopt,
//! commandline, program-options, inc fragments). A binary tool: it reads from
//! two input streams (first + second) and writes a comparison log.

use crate::binary_ops::open_two_input_streams;
use crate::globals;
use crate::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_from_env, hfst_set_program_name, hfst_strformat,
    is_input_stream_in_ol_format, verbose_print,
};
use crate::hfst_getopt as getopt;
use crate::hfst_program_options::{
    hfst_getopt_binary_long, hfst_getopt_common_long, print_common_binary_program_options,
    print_common_binary_program_parameter_instructions, print_common_program_options,
};
use crate::inc::{
    CaseResult, check_binary_params, check_common_params, handle_binary_case, handle_common_case,
    handle_error_case,
};
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_transducer::{AnyTransducer, HfstTransducer};
use std::io::Write;

// Tool-specific option state (C: 'static bool harmonize=true; static bool
// eliminate_flags=false;').
static mut HARMONIZE: bool = true;
static mut ELIMINATE_FLAGS: bool = false;

// [spec:hfst:def:hfst-compare.print-usage-fn]
// [spec:hfst:sem:hfst-compare.print-usage-fn]
fn print_usage() {
    let mut msg = globals::message_writer();
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    let program_name = globals::program_name();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE1 [INFILE2]]\nCompare two transducers\n\n",
        program_name
    );
    print_common_program_options(&mut *msg);
    print_common_binary_program_options(&mut *msg);
    let _ = write!(
        msg,
        "Harmonization:\n  -H, --do-not-harmonize Do not harmonize symbols.\n  -e, --eliminate-flags  Eliminate flag diacritics.\n"
    );
    let _ = write!(msg, "\n");
    print_common_binary_program_parameter_instructions(&mut *msg);
    let _ = write!(msg, "\n");
    let _ = write!(
        msg,
        "\nExamples:\n  $ {0} cat.hfst dog.hfst\n  cat.hfst[1] != dog.hfst[1]\n  $ {0} cat.hfst cat.hfst\n  cat.hfst[1] == cat.hfst[1]\n\n",
        program_name
    );
}

// [spec:hfst:def:hfst-compare.parse-options-fn]
// [spec:hfst:sem:hfst-compare.parse-options-fn]
unsafe fn parse_options(args: &mut Vec<String>) -> i32 {
    unsafe {
        extend_options_from_env(args);
        // use of this function requires options are settable on global scope
        loop {
            let mut long_options: Vec<getopt::GetOpt> = Vec::new();
            long_options.extend(hfst_getopt_common_long());
            long_options.extend(hfst_getopt_binary_long());
            // add tool-specific options here
            long_options.push(getopt::GetOpt {
                name: "do-not-harmonize",
                has_arg: 0,
                val: 'H' as i32,
            });
            long_options.push(getopt::GetOpt {
                name: "eliminate-flags",
                has_arg: 0,
                val: 'e' as i32,
            });
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then binary cases, then the tool's own ('H'/'e'), then the
            // terminal error arm.
            match handle_common_case(c, print_usage) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match handle_binary_case(c) {
                CaseResult::Return(code) => return code,
                CaseResult::Break => continue,
                CaseResult::NotHandled => {}
            }
            match c as u8 as char {
                'H' => {
                    HARMONIZE = false;
                    continue;
                }
                'e' => {
                    ELIMINATE_FLAGS = true;
                    continue;
                }
                _ => {}
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_binary_params(args);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-compare.compare-streams-fn]
// [spec:hfst:sem:hfst-compare.compare-streams-fn]
unsafe fn compare_streams(
    firststream: &mut HfstInputStream,
    secondstream: &mut HfstInputStream,
) -> i32 {
    unsafe {
        let mut out = match globals::output_writer() {
            Ok(w) => w,
            Err(e) => {
                eprintln!("hfst-compare: cannot open output: {e}");
                return 1;
            }
        };
        let mut continue_reading = firststream.is_good() && secondstream.is_good();
        let mut transducer_n_first: usize = 0; // transducers read from first input
        let mut transducer_n_second: usize = 0; // transducers read from second input
        let mut mismatches: usize = 0;

        let mut second: Option<AnyTransducer> = None;

        while continue_reading {
            let mut first = match firststream.read() {
                Ok(v) => v,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            };
            transducer_n_first += 1;
            if secondstream.is_good() {
                second = Some(match secondstream.read() {
                    Ok(v) => v,
                    Err(e) => {
                        error(1, 0, &format!("{e}"));
                        return 1;
                    }
                });
                transducer_n_second += 1;
            }
            let mut firstname = first.get_name();
            // make scan-build happy, this should not happen
            let second_ref = match second.as_mut() {
                Some(s) => s,
                None => panic!("Error: second stream has a NULL value."),
            };
            let mut secondname = second_ref.get_name();
            if firstname.is_empty() {
                firstname = globals::first_filename();
            }
            if secondname.is_empty() {
                secondname = globals::second_filename();
            }
            if transducer_n_first == 1 {
                verbose_print(&format!("Comparing {} and {}...\n", firstname, secondname));
            } else {
                verbose_print(&format!(
                    "Comparing {} and {}... {}\n",
                    firstname, secondname, transducer_n_first
                ));
            }
            // C: try { ... } catch (TransducerTypeMismatchException). Same-
            // backend operands are a compile-time property of the generic
            // body now, so the mismatch is this boundary's fall-through arm
            // ([dec:hfst:monomorphic-backends]).
            let outcome = match (&mut first, second_ref) {
                (AnyTransducer::Tropical(f), AnyTransducer::Tropical(s)) => {
                    Some(compare_pair(f, s))
                }
                (AnyTransducer::Log(f), AnyTransducer::Log(s)) => Some(compare_pair(f, s)),
                _ => None,
            };
            match outcome {
                Some(Ok(equal)) => {
                    if equal {
                        if transducer_n_first == 1 {
                            if !globals::SILENT {
                                let _ = write!(out, "{} == {}\n", firstname, secondname);
                            }
                        } else if !globals::SILENT {
                            let _ = write!(
                                out,
                                "{}[{}] == {}[{}]\n",
                                firstname, transducer_n_first, secondname, transducer_n_second
                            );
                        }
                    } else {
                        if transducer_n_first == 1 {
                            if !globals::SILENT {
                                let _ = write!(out, "{} != {}\n", firstname, secondname);
                            }
                        } else if !globals::SILENT {
                            let _ = write!(
                                out,
                                "{}[{}] != {}[{}]\n",
                                firstname, transducer_n_first, secondname, transducer_n_second
                            );
                        }
                        mismatches += 1;
                    }
                }
                Some(Err(e)) => {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
                None => {
                    // cannot recover yet, but beautify error messages
                    error(
                        2,
                        0,
                        &format!(
                            "Cannot compare `{}' and `{}' [{}]\nthe formats {} and {} are not compatible for comparison",
                            firstname,
                            secondname,
                            transducer_n_first,
                            hfst_strformat(firststream.get_type()),
                            hfst_strformat(secondstream.get_type())
                        ),
                    );
                }
            }

            continue_reading =
                firststream.is_good() && (secondstream.is_good() || transducer_n_second == 1);

            // delete the transducer of second stream, unless we continue reading
            // the first stream and there is only one transducer in the second
            // stream
            if (continue_reading && secondstream.is_good()) || !continue_reading {
                second = None;
            }
        }

        if firststream.is_good() {
            error(
                1,
                0,
                &format!(
                    "second input '{}' contains fewer transducers than first input '{}'; this is only possible if the second input contains exactly one transducer",
                    globals::second_filename(),
                    globals::first_filename()
                ),
            );
        } else if secondstream.is_good() {
            error(
                1,
                0,
                &format!(
                    "first input '{}' contains fewer transducers than second input '{}'",
                    globals::first_filename(),
                    globals::second_filename()
                ),
            );
        }
        firststream.close();
        secondstream.close();
        let _ = out.flush();
        if mismatches == 0 {
            verbose_print(&format!("All {} transducers matched\n", transducer_n_first));
            0
        } else {
            verbose_print(&format!(
                "{}/{} were not equal\n",
                mismatches, transducer_n_first
            ));
            1
        }
    }
}

// The monomorphic per-pair comparison body (flag elimination + compare).
unsafe fn compare_pair<B: hfst::backend::AlgebraBackend>(
    first: &mut HfstTransducer<B>,
    second: &mut HfstTransducer<B>,
) -> hfst::error::Result<bool> {
    unsafe {
        if ELIMINATE_FLAGS {
            verbose_print("Eliminating flags...\n");
            first.eliminate_flags()?;
            second.eliminate_flags()?;
        }
        first.compare(second, HARMONIZE)
    }
}

// [spec:hfst:def:hfst-compare.main-fn]
// [spec:hfst:sem:hfst-compare.main-fn]
pub fn run(args: Vec<String>) -> i32 {
    unsafe { real_main(args) }
}

unsafe fn real_main(mut args: Vec<String>) -> i32 {
    unsafe {
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstCompare");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        verbose_print(&format!(
            "Reading from {} and {}, writing log to {}\n",
            globals::first_filename(),
            globals::second_filename(),
            globals::output_filename()
        ));
        let (mut firststream, mut secondstream) = match open_two_input_streams() {
            Ok(v) => v,
            Err(code) => return code,
        };

        if is_input_stream_in_ol_format(&firststream, "hfst-compare")
            || is_input_stream_in_ol_format(&secondstream, "hfst-compare")
        {
            return 1;
        }

        compare_streams(&mut firststream, &mut secondstream)
    }
}
