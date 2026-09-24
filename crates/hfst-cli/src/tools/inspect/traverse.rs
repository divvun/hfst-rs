//! Faithful 1:1 port of tools/src/hfst-traverse.cc — the transducer traversal
//! tool that walks through a transducer arc by arc. Drives the hfst-cli
//! foundation (globals, getopt, commandline, program-options, inc fragments).

use crate::cli::{self, CommonArgs, ToolArgs, ToolResult, UnaryIo};
use crate::globals::CommonOptions;
use crate::hfst_commandline::{error, hfst_readline, hfst_set_program_name, verbose_print};
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_input_stream::HfstInputStream;
use hfst::hfst_output_stream::HfstOutputStream;
use std::collections::BTreeMap;
use std::io::Write;

/// hfst-traverse's command line.
// [spec:hfst:req:cli.arg-parse]
// [spec:hfst:req:cli.help]
#[derive(clap::Parser)]
#[command(about = "Walk through the transducer arc by arc")]
struct Args {
    #[command(flatten)]
    common: CommonArgs,
    #[command(flatten)]
    io: UnaryIo,

    /// Play the Colossal Cave adventure intro on start
    #[arg(short = 'X', long = "cave")]
    cave_mode: bool,
}

impl ToolArgs for Args {
    fn common(&self) -> &CommonArgs {
        &self.common
    }

    fn apply_io(&self, opts: &mut CommonOptions) {
        self.io.apply(opts);
    }
}

/// hfst-traverse's resolved options (the former tool-specific `static mut`s).
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

// [spec:hfst:def:hfst-traverse.main-loop-fn]
// [spec:hfst:sem:hfst-traverse.main-loop-fn]
fn main_loop(common: &CommonOptions, trans: &HfstBasicTransducer) -> i32 {
    let mut msg = common.message_writer();
    let _ = writeln!(msg, "Enter labels to seek all paths");
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
            let _ = writeln!(msg, "On path `{}' are continuations:", path_str);
            let transitions = match trans.index(*state) {
                Ok(v) => v,
                Err(e) => {
                    error(common, 1, 0, &format!("{e}"));
                    return 1;
                }
            };
            if transitions.is_empty() {
                let _ = writeln!(msg, "<Nothing, you've hit a dead end here>");
            }
            for arc in transitions.iter() {
                let _ = writeln!(
                    msg,
                    "{}\t{}",
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
                let _ = writeln!(msg, "Use EOF (Ctrl-D or similar) to quit");
            } else if label == "XYZZY" {
                let _ = writeln!(msg, "Nothing happens");
            }
            let _ = writeln!(msg, "could not advance with {}", label);
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
    instream: &mut HfstInputStream<'_>,
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
                let _ = writeln!(msg, "Nowhere to go");
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
pub(in crate::tools) fn execute(args: Vec<String>) -> ToolResult {
    let argv0 = args.first().cloned().unwrap_or_default();

    let common = hfst_set_program_name(&argv0, "0.1", "HfstDeterminize");
    let (common, args) = cli::parse::<Args>(common, args)?;
    let options = Options {
        cave_mode: args.cave_mode,
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
            return Err(1);
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
            return Err(1);
        }
    };

    cli::from_code(process_stream(&common, &options, &mut instream))
}
