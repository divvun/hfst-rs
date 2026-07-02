//! The single 'hfst' multiplexer binary. All former standalone hfst-<tool>
//! binaries live as modules under hfst_cli::tools; this binary dispatches to
//! them two ways:
//!
//! 1. Basename dispatch: when invoked via a symlink/hardlink/copy named after
//!    an original binary (e.g. 'hfst-compose'), the matching tool's run() is
//!    called with the ORIGINAL argv unchanged, so output is byte-identical to
//!    the old standalone binary.
//! 2. Subcommand dispatch: 'hfst <sub> [ARGS...]' where <sub> is the tool name
//!    minus the 'hfst-' prefix. The tool argv is rebuilt as
//!    ["hfst <sub>", ARGS...] so program-name-derived message prefixes render
//!    as "hfst <sub>"; the remaining args pass through UNTOUCHED to the tool's
//!    own getopt parser (including -h/--help, which the tool handles itself).
//!
//! clap provides only the outer interface: 'hfst --help' (the subcommand
//! listing), 'hfst --version', and the error/suggestion path for unknown
//! subcommands. It never parses a tool's own flags.

use clap::{Arg, ArgAction, Command};
use hfst_cli::tools::TOOLS;

/// One-line about strings for the clap subcommand listing, taken from each
/// tool's usage summary line (the sentence after "Usage:" in its
/// print_usage), keyed by the original binary name.
const ABOUTS: &[(&str, &str)] = &[
    (
        "hfst-affix-guessify",
        "Create weighted affix guesser from automaton",
    ),
    ("hfst-binary-tool", "Do things with two transducers"),
    (
        "hfst-check-alpha",
        "Compare the compatibility of alphabets between INFILEs",
    ),
    ("hfst-compare", "Compare two transducers"),
    ("hfst-compose", "Compose two transducers"),
    (
        "hfst-compose-intersect",
        "Compose a lexicon with one or more rule transducers.",
    ),
    ("hfst-concatenate", "Concatenate two transducers"),
    ("hfst-conjunct", "Conjunct (intersect, AND) two transducers"),
    ("hfst-determinize", "Determinize a transducer"),
    ("hfst-disjunct", "Disjunct (union, OR) two transducers"),
    ("hfst-dump-alphabets", "Print alphabets of automaton"),
    ("hfst-edit-metadata", "Name a transducer"),
    ("hfst-eliminate-flags", "Eliminate flags from a transducer"),
    (
        "hfst-expand-equivalences",
        "Extend transducer arcs for equivalence classes",
    ),
    (
        "hfst-flookup",
        "Perform transducer lookup (apply), from right to left",
    ),
    ("hfst-format", "determine HFST transducer format"),
    ("hfst-fst2fst", "Convert transducers between binary formats"),
    (
        "hfst-fst2strings",
        "Display the strings recognized by a transducer",
    ),
    (
        "hfst-fst2txt",
        "Print transducer in AT&T, dot, prolog or pckimmo format",
    ),
    (
        "hfst-grep",
        "Search for PATTERN in each FILE or standard input.",
    ),
    (
        "hfst-guess",
        "Use a guesser (and generator) to guess analyses or inflectional paradigms of unknown words",
    ),
    (
        "hfst-guessify",
        "Compile a morphological analyzer into a guesser and generator.",
    ),
    ("hfst-head", "Get first transducers from an archive"),
    ("hfst-info", "show or test HFST versions and features"),
    ("hfst-insert-freely", "Freely insert a symbol (pair)"),
    ("hfst-invert", "Invert a transducer"),
    ("hfst-kill-paths", "Kill all paths with specific symbols"),
    ("hfst-lexc-compiler", "Compile lexc files into transducer"),
    ("hfst-lookup", "perform transducer lookup (apply)"),
    ("hfst-minimize", "Minimize a transducer"),
    (
        "hfst-multiply",
        "Use first transducer of an archive repeatedly",
    ),
    ("hfst-name", "Name a transducer"),
    (
        "hfst-optimized-lookup",
        "Run a transducer on standard input (one word per line) and print analyses",
    ),
    ("hfst-pair-test", "pair test for a twolc rule file."),
    ("hfst-pmatch", "perform matching/lookup on text streams"),
    (
        "hfst-pmatch2fst",
        "Compile regular expressions into transducer(s) (Experimental version)",
    ),
    (
        "hfst-preprocess-for-optimized-lookup-format",
        "Remove epsilons from a transducer",
    ),
    (
        "hfst-priority-disjunct",
        "Disjunct (union, OR) two transducers",
    ),
    ("hfst-project", "Project (extract a level) transducer"),
    ("hfst-prune-alphabet", "Prune the alphabet of a transducer"),
    ("hfst-push-labels", "Push labels of transducer"),
    ("hfst-push-weights", "Push weights of transducer"),
    (
        "hfst-realign",
        "Realign a transducer by pushing labels to the start",
    ),
    (
        "hfst-regexp2fst",
        "Compile (weighted) regular expressions into transducer(s)",
    ),
    ("hfst-remove-epsilons", "Remove epsilons from a transducer"),
    ("hfst-repeat", "Repeat transducer"),
    ("hfst-reverse", "Reverse a transducer"),
    ("hfst-reweight", "Reweight transducer weights simply"),
    ("hfst-shuffle", "Shuffle two transducers"),
    (
        "hfst-split",
        "Extract transducers from archive with systematic file names",
    ),
    (
        "hfst-strings2fst",
        "Compile string pairs and pair-strings into transducer(s)",
    ),
    ("hfst-strip-header", "Remove any HFST3 headers"),
    ("hfst-substitute", "Relabel transducer arcs"),
    ("hfst-subtract", "Subtract (minus) two transducers"),
    ("hfst-summarize", "Calculate the properties of a transducer"),
    ("hfst-tail", "Get last transducers from an archive"),
    ("hfst-tokenize", "perform matching/lookup on text streams"),
    ("hfst-traverse", "Walk through the transducer arc by arc"),
    (
        "hfst-txt2fst",
        "Convert AT&T or prolog format into a binary transducer",
    ),
];

fn find_tool(name: &str) -> Option<fn(Vec<String>) -> i32> {
    TOOLS
        .iter()
        .find(|(tool, _)| *tool == name)
        .map(|&(_, run)| run)
}

fn about_for(name: &str) -> &'static str {
    ABOUTS
        .iter()
        .find(|(tool, _)| *tool == name)
        .map(|&(_, about)| about)
        .unwrap_or("")
}

/// The invoked basename: file stem of argv[0], with a possible .exe suffix
/// stripped.
fn invoked_basename(argv0: &str) -> String {
    let base = std::path::Path::new(argv0)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    match base.strip_suffix(".exe") {
        Some(stem) => stem.to_string(),
        None => base,
    }
}

fn build_cli() -> Command {
    let mut cmd = Command::new("hfst")
        .version(env!("CARGO_PKG_VERSION"))
        .about("HFST command-line tools: one binary, one subcommand per tool")
        .subcommand_required(true)
        .arg_required_else_help(true);
    for (tool, _) in TOOLS {
        let sub = tool
            .strip_prefix("hfst-")
            .expect("every TOOLS entry is named hfst-<tool>");
        cmd = cmd.subcommand(
            Command::new(sub)
                .about(about_for(tool))
                .disable_help_flag(true)
                .arg(
                    Arg::new("args")
                        .num_args(0..)
                        .allow_hyphen_values(true)
                        .trailing_var_arg(true)
                        .action(ArgAction::Append)
                        .help("Arguments passed through untouched to the tool's own getopt parser"),
                ),
        );
    }
    cmd
}

fn main() {
    let argv: Vec<String> = std::env::args().collect();

    // Basename dispatch: symlink/hardlink/copy invocation as an original
    // binary name. argv passes through completely unchanged.
    let basename = invoked_basename(argv.first().map(String::as_str).unwrap_or_default());
    if basename != "hfst" {
        if let Some(run) = find_tool(&basename) {
            std::process::exit(run(argv));
        }
        // Unknown basename: fall through to the subcommand interface.
    }

    // Subcommand dispatch: argv[1] = tool name minus "hfst-". The tool's args
    // are forwarded raw (clap never sees them), so the old getopt flags pass
    // through untouched.
    if let Some(sub) = argv.get(1) {
        if !sub.starts_with('-') {
            if let Some(run) = find_tool(&format!("hfst-{sub}")) {
                let mut tool_argv = Vec::with_capacity(argv.len() - 1);
                tool_argv.push(format!("hfst {sub}"));
                tool_argv.extend(argv[2..].iter().cloned());
                std::process::exit(run(tool_argv));
            }
        }
    }

    // No tool matched: let clap render --help/--version, the subcommand
    // listing, or the unknown-subcommand error. Every real subcommand was
    // already dispatched above, so this only returns for clap's own paths
    // (e.g. 'hfst help <sub>' exits inside get_matches_from).
    build_cli().get_matches_from(argv);
}
