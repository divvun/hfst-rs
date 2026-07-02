//! Faithful 1:1 port of tools/src/hfst-traverse.cc — the transducer traversal
//! tool that walks through a transducer arc by arc. Drives the hfst-cli
//! foundation (globals, getopt, commandline, program-options, inc fragments).

use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use hfst::hfst_transducer::HfstTransducer;
use hfst_cli::globals;
use hfst_cli::hfst_commandline::{
    EXIT_CONTINUE, error, extend_options_getenv, hfst_readline, hfst_set_program_name,
    print_more_info, print_report_bugs, verbose_printf,
};
use hfst_cli::hfst_getopt as getopt;
use hfst_cli::hfst_program_options::{
    hfst_getopt_common_long, hfst_getopt_unary_long, print_common_program_options,
    print_common_unary_program_options, print_common_unary_program_parameter_instructions,
};
use hfst_cli::inc::{
    CaseResult, check_common_params, check_unary_params, handle_common_case, handle_error_case,
    handle_unary_case,
};
use std::collections::BTreeMap;
use std::io::Write;

// add tools-specific variables here
static mut CAVE_MODE: bool = false;

// The C arclabel readline-completion helpers (arclabel_generator /
// arclabel_completion) are gated behind HAVE_DECL_RL_COMPLETION_MATCHES and the
// GNU readline library. The Rust 'hfst_readline' uses plain 'getline' with no
// readline backend, so — exactly as on a build without readline — those #if
// blocks are not compiled in. Their def/sem annotations are carried below for
// traceability; the bodies are intentionally left out to match the
// no-readline configuration the foundation provides.

// [spec:hfst:def:hfst-traverse.arclabel-generator-fn]
// [spec:hfst:sem:hfst-traverse.arclabel-generator-fn]
// (readline-only: not compiled — see note above)

// [spec:hfst:def:hfst-traverse.arclabel-completion-fn]
// [spec:hfst:sem:hfst-traverse.arclabel-completion-fn]
// (readline-only: not compiled — see note above)

// [spec:hfst:def:hfst-traverse.print-usage-fn]
// [spec:hfst:sem:hfst-traverse.print-usage-fn]
fn print_usage() {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    // Usage line
    let mut msg = globals::message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nWalk through the transducer arc by arc\n\n",
        globals::program_name()
    );

    // options, grouped
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(msg, "\n");
    print_report_bugs();
    let _ = write!(msg, "\n");
    print_more_info();
}

// [spec:hfst:def:hfst-traverse.parse-options-fn]
// [spec:hfst:sem:hfst-traverse.parse-options-fn]
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
                name: "cave",
                has_arg: getopt::NO_ARGUMENT,
                val: 'X' as i32,
            });
            let c = getopt::getopt_long(args, &long_options);
            if -1 == c {
                break;
            }

            // The C switch chains the #include'd case groups in order: common
            // cases, then unary cases, then the tool's own 'X', then the
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
            if c == 'X' as i32 {
                CAVE_MODE = true;
                continue;
            }
            return handle_error_case(c);
        }

        check_common_params();
        check_unary_params(args);
        EXIT_CONTINUE
    }
}

// [spec:hfst:def:hfst-traverse.main-loop-fn]
// [spec:hfst:sem:hfst-traverse.main-loop-fn]
unsafe fn main_loop(trans: &HfstBasicTransducer) -> i32 {
    unsafe {
        let mut msg = globals::message_writer();
        let _ = write!(msg, "Enter labels to seek all paths\n");
        // record current paths with their end states. The C++ uses a
        // multimap<string, HfstState>; a BTreeMap<(String, usize), HfstState>
        // (keyed on an insertion counter to permit duplicate path strings)
        // preserves both the ordered iteration and the multi-value semantics.
        let mut paths: BTreeMap<(String, usize), u32> = BTreeMap::new();
        let mut counter: usize = 0;
        paths.insert((String::new(), counter), 0);
        counter += 1;
        // (The readline completion / history setup is readline-only; omitted as
        // the foundation uses a plain getline-based readline — see note above.)
        loop {
            // print available paths
            for ((path_str, _), state) in paths.iter() {
                let _ = write!(msg, "On path `{}' are continuations:\n", path_str);
                let transitions = match trans.index(*state) {
                    Ok(v) => v,
                    Err(e) => {
                        error(1, 0, &format!("{e}"));
                        return 1;
                    }
                };
                if transitions.is_empty() {
                    let _ = write!(msg, "<Nothing, you've hit a dead end here>\n");
                }
                for arc in transitions.iter() {
                    let _ = write!(
                        msg,
                        "{}\t{}\n",
                        arc.get_input_symbol(trans.coder()),
                        arc.get_output_symbol(trans.coder())
                    );
                }
            }
            let label = match hfst_readline("traverse> ") {
                Some(l) => l,
                None => return 0,
            };
            let mut new_paths: BTreeMap<(String, usize), u32> = BTreeMap::new();
            for ((path_str, _), state) in paths.iter() {
                let transitions = match trans.index(*state) {
                    Ok(v) => v,
                    Err(e) => {
                        error(1, 0, &format!("{e}"));
                        return 1;
                    }
                };
                for arc in transitions.iter() {
                    if arc.get_input_symbol(trans.coder()) == label {
                        let newpath = format!(
                            "{}{}:{} ",
                            path_str,
                            arc.get_input_symbol(trans.coder()),
                            arc.get_output_symbol(trans.coder())
                        );
                        new_paths.insert((newpath, counter), arc.get_target_state());
                        counter += 1;
                    }
                }
            }
            if new_paths.is_empty() {
                if label == "quit" || label.is_empty() {
                    let _ = write!(msg, "Use EOF (Ctrl-D or similar) to quit\n");
                } else if label == "XYZZY" {
                    let _ = write!(msg, "Nothing happens\n");
                }
                let _ = write!(msg, "could not advance with {}\n", label);
            } else {
                paths = new_paths;
            }
            // (add_history is readline-only; omitted — see note above.)
        } // while paths not empty
    }
}

// [spec:hfst:def:hfst-traverse.process-stream-fn]
// [spec:hfst:sem:hfst-traverse.process-stream-fn]
unsafe fn process_stream(instream: &mut HfstInputStream) -> i32 {
    unsafe {
        let mut msg = globals::message_writer();
        let mut transducer_n: usize = 0;
        while instream.is_good() {
            transducer_n += 1;
            let _ = transducer_n;
            let trans = match HfstTransducer::new_from_stream(instream) {
                Ok(v) => v,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            };
            let mut trans_name = trans.get_name();
            if trans_name.is_empty() {
                trans_name = globals::input_filename();
            }
            // HfstBasicTransducer walkable(trans);
            let walkable = match trans.get_basic_transducer() {
                Ok(v) => v,
                Err(e) => {
                    error(1, 0, &format!("{e}"));
                    return 1;
                }
            };
            if CAVE_MODE {
                let _ = write!(
                    msg,
                    "WELCOME TO ADVENTURE!! WOULD YOU LIKE INSTRUCTIONS?\n\n"
                );
                let yesno = hfst_readline("").unwrap_or_default();
                if yesno == "YES" || yesno == "yes" {
                    let _ = write!(
                        msg,
                        "SOMEWHERE NEARBY IS COLOSSAL CAVE \
                         WHERE OTHERS HAVE FOUND\n\
                         FORTUNES IN TREASURES AND GOLD, \
                         THOUGH IT IS RUMORED\n\
                         THAT SOME WHO ENTER ARE NEVER SEEN AGAIN. \
                         MAGIC IS SAID\n\
                         TO WORK IN THE CAVE.  I WILL BE YOUR EYES AND HANDS. \
                         DIRECT\n\
                         ME WITH COMMANDS OF 1 ARC LABEL.\n\
                         (ERRORS, COMPLAINTS, SUGGESTIONS TO HFST-BUGS)\n\
                         (IF STUCK TYPE HELP FOR SOME HINTS)\n\n",
                    );
                }
                let _ = write!(
                    msg,
                    "YOU ARE STANDING AT THE END OF A ROAD BEFORE A \
                     SMALL FINITE\n\
                     STATE AUTOMATON . AROUND YOU IS A FOREST. A SMALL\n\
                     STREAM OF ARCS FLOWS OUT OF THE AUTOMATON AND \
                     DOWN A GULLY:\n\n",
                );
            } else {
                let _ = write!(msg, "Traversing automaton {}\n\n", trans_name);
            }
            if walkable.state_vector.is_empty() {
                let _ = write!(msg, "Nowhere to go\n");
                return 0;
            }
            return main_loop(&walkable);
        }
        instream.close();
        0
    }
}

// [spec:hfst:def:hfst-traverse.main-fn]
// [spec:hfst:sem:hfst-traverse.main-fn]
fn main() {
    let code = unsafe { real_main() };
    std::process::exit(code);
}

unsafe fn real_main() -> i32 {
    unsafe {
        let mut args: Vec<String> = std::env::args().collect();
        let argv0 = args.first().cloned().unwrap_or_default();

        hfst_set_program_name(&argv0, "0.1", "HfstDeterminize");
        let retval = parse_options(&mut args);
        if retval != EXIT_CONTINUE {
            return retval;
        }
        // close buffers, we use streams
        let input_opened = globals::input_filename() != "<stdin>";
        let output_opened = globals::output_filename() != "<stdout>";
        verbose_printf(&format!(
            "Reading from {}, writing to {}\n",
            globals::input_filename(),
            globals::output_filename()
        ));

        // here starts the buffer handling part
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
        // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
        // currently panics on a bad file rather than throwing, so the catch arm
        // is not reproduced here.)

        // The C constructs an HfstOutputStream from the input type even though
        // this tool never writes to it (traversal only reads). Mirror that
        // construction so the buffer-handling part matches the source.
        let ty = instream.get_type();
        let _outstream = match if output_opened {
            HfstOutputStream::new_filename(&globals::output_filename(), ty, true)
        } else {
            HfstOutputStream::new(ty, true)
        } {
            Ok(v) => v,
            Err(e) => {
                error(1, 0, &format!("{e}"));
                return 1;
            }
        };

        process_stream(&mut instream)
    }
}
