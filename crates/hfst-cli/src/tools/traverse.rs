//! Faithful 1:1 port of tools/src/hfst-traverse.cc — the transducer traversal
//! tool that walks through a transducer arc by arc. Drives the hfst-cli
//! foundation (globals, getopt, commandline, program-options, inc fragments).

use crate::globals::CommonOptions;
use crate::hfst_commandline::{
    error, extend_options_from_env, hfst_readline, hfst_set_program_name, verbose_print,
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
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use std::collections::BTreeMap;
use std::io::Write;

/// hfst-traverse's own options (the former tool-specific `static mut`s).
#[derive(Default)]
struct Options {
    /// '-X, --cave': play the Colossal Cave adventure intro on start.
    cave_mode: bool,
}

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
fn print_usage(common: &CommonOptions) {
    // c.f. http://www.gnu.org/prep/standards/standards.html#g_t_002d_002dhelp
    // Usage line
    let mut msg = common.message_writer();
    let _ = write!(
        msg,
        "Usage: {} [OPTIONS...] [INFILE]\nWalk through the transducer arc by arc\n\n",
        common.program_name
    );

    // options, grouped
    print_common_program_options(&mut *msg);
    print_common_unary_program_options(&mut *msg);
    let _ = write!(msg, "\n");
    print_common_unary_program_parameter_instructions(&mut *msg);
    let _ = write!(msg, "\n");
}

// [spec:hfst:def:hfst-traverse.parse-options-fn]
// [spec:hfst:sem:hfst-traverse.parse-options-fn]
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
            name: "cave",
            has_arg: getopt::NO_ARGUMENT,
            val: 'X' as i32,
        });
        let c = opt.getopt_long(args, &long_options);
        if -1 == c {
            break;
        }

        // The C switch chains the #include'd case groups in order: common
        // cases, then unary cases, then the tool's own 'X', then the
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
        if c == 'X' as i32 {
            options.cave_mode = true;
            continue;
        }
        return Err(handle_error_case(&common, &opt, c));
    }

    check_common_params(&mut common);
    check_unary_params(&mut common, &opt, args);
    Ok((common, options))
}

// [spec:hfst:def:hfst-traverse.main-loop-fn]
// [spec:hfst:sem:hfst-traverse.main-loop-fn]
fn main_loop(common: &CommonOptions, trans: &HfstBasicTransducer) -> i32 {
    let mut msg = common.message_writer();
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
                    error(common, 1, 0, &format!("{e}"));
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
        let label = match hfst_readline(common, "traverse> ") {
            Some(l) => l,
            None => return 0,
        };
        let mut new_paths: BTreeMap<(String, usize), u32> = BTreeMap::new();
        for ((path_str, _), state) in paths.iter() {
            let transitions = match trans.index(*state) {
                Ok(v) => v,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
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

// [spec:hfst:def:hfst-traverse.process-stream-fn]
// [spec:hfst:sem:hfst-traverse.process-stream-fn]
fn process_stream(
    common: &CommonOptions,
    options: &Options,
    instream: &mut HfstInputStream,
) -> i32 {
    let mut msg = common.message_writer();
    let mut transducer_n: usize = 0;
    // The C++ writes this as `while (instream.is_good())` but its body
    // unconditionally `return`s main_loop() on the first transducer
    // (hfst-traverse.cc:278/325), so it runs exactly once — an `if` here is
    // behaviour-identical and not a never-looping loop.
    if instream.is_good() {
        transducer_n += 1;
        let _ = transducer_n;
        let any = match instream.read() {
            Ok(v) => v,
            Err(e) => {
                error(common, 1, 0, &format!("{e}"));
                return 1;
            }
        };
        // the one runtime dispatch per stream read ([dec:hfst:monomorphic-backends])
        crate::for_any!(any, trans => {
            let mut trans_name = trans.get_name();
            if trans_name.is_empty() {
                trans_name = common.input_filename.clone();
            }
            // HfstBasicTransducer walkable(trans);
            let walkable = match HfstBasicTransducer::try_from_transducer(&trans) {
                Ok(v) => v,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            if options.cave_mode {
                let _ = write!(
                    msg,
                    "WELCOME TO ADVENTURE!! WOULD YOU LIKE INSTRUCTIONS?\n\n"
                );
                let yesno = hfst_readline(common, "").unwrap_or_default();
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
            return main_loop(common, &walkable);
        });
    }
    instream.close();
    0
}

// [spec:hfst:def:hfst-traverse.main-fn]
// [spec:hfst:sem:hfst-traverse.main-fn]
pub fn run(mut args: Vec<String>) -> i32 {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstDeterminize");
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

    // here starts the buffer handling part
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
    // (the C wraps the ctor in try/catch on HfstException; the Rust ctor
    // currently panics on a bad file rather than throwing, so the catch arm
    // is not reproduced here.)

    // The C constructs an HfstOutputStream from the input type even though
    // this tool never writes to it (traversal only reads). Mirror that
    // construction so the buffer-handling part matches the source.
    let ty = instream.get_type();
    let _outstream = match if output_opened {
        HfstOutputStream::new_filename(&common.output_filename, ty, true)
    } else {
        HfstOutputStream::new(ty, true)
    } {
        Ok(v) => v,
        Err(e) => {
            error(&common, 1, 0, &format!("{e}"));
            return 1;
        }
    };

    process_stream(&common, &options, &mut instream)
}
