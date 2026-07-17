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

// The FST algorithms are allocation-heavy; mimalloc beats the system
// allocator substantially on this workload (house convention for binaries).
// Under the `dhat-heap` profiling feature this is swapped for dhat's tracking
// allocator so the build's heap can be attributed to allocation sites.
#[cfg(not(feature = "dhat-heap"))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static GLOBAL: dhat::Alloc = dhat::Alloc;

/// One-line about strings for the clap subcommand listing, taken from each
/// tool's usage summary line (the sentence after "Usage:" in its
/// print_usage), keyed by the original binary name.
const ABOUTS: &[(&str, &str)] = &[
    (
        "hfst-affix-guessify",
        "Create weighted affix guesser from automaton",
    ),
    ("hfst-lexc", "Compile lexc files into transducer (alias)"),
    ("hfst-union", "Disjunct (union, OR) two transducers (alias)"),
    ("hfst-minus", "Subtract (minus) two transducers (alias)"),
    (
        "hfst-intersect",
        "Conjunct (intersect, AND) two transducers (alias)",
    ),
    (
        "hfst-tokenise",
        "perform matching/lookup on text streams (alias)",
    ),
    (
        "hfst-optimised-lookup",
        "Run a transducer on standard input (one word per line) and print analyses (alias)",
    ),
    ("hfst-binary-tool", "Do things with two transducers"),
    (
        "hfst-bhfst",
        "Pack a THFST acceptor/errmodel pair (+ speller metadata) into a BHFST archive",
    ),
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
        "hfst-twolc",
        "Read a twolc grammar, compile it and store it",
    ),
    (
        "hfst-txt2fst",
        "Convert AT&T or prolog format into a binary transducer",
    ),
    (
        "hfst-xfst",
        "Compile XFST scripts or execute XFST commands interactively",
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

/// `hfst install-symlinks [DIR] [--force]`: create an `hfst-<tool>` symlink
/// for every legacy tool name, pointing at this binary. DIR defaults to the
/// directory containing the running executable; links in that directory are
/// made relative (to the binary's file name) so the installation stays
/// relocatable.
fn install_symlinks(args: &[String]) -> i32 {
    let mut force = false;
    let mut dir: Option<std::path::PathBuf> = None;
    for a in args {
        match a.as_str() {
            "-f" | "--force" => force = true,
            "-h" | "--help" => {
                println!(
                    "Usage: hfst install-symlinks [OPTIONS...] [DIR]\n\
                     Create hfst-<tool> symlinks for all legacy tool names\n\n\
                     \x20 -f, --force  Replace existing files\n\n\
                     DIR defaults to the directory containing the hfst binary."
                );
                return 0;
            }
            other if !other.starts_with('-') && dir.is_none() => dir = Some(other.into()),
            other => {
                eprintln!("hfst install-symlinks: unrecognized argument '{other}'");
                return 1;
            }
        }
    }

    let exe = match std::env::current_exe().and_then(|p| p.canonicalize()) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("hfst install-symlinks: cannot locate the hfst binary: {e}");
            return 1;
        }
    };
    let exe_dir = exe
        .parent()
        .expect("a canonicalized executable path has a parent")
        .to_path_buf();
    let target_dir = match dir {
        Some(d) => match d.canonicalize() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("hfst install-symlinks: {}: {e}", d.display());
                return 1;
            }
        },
        None => exe_dir.clone(),
    };
    // Same directory as the binary: link to the bare file name so the whole
    // directory can be moved; elsewhere: link to the absolute binary path.
    let link_target: std::path::PathBuf = if target_dir == exe_dir {
        exe.file_name()
            .expect("a canonicalized executable path has a file name")
            .into()
    } else {
        exe.clone()
    };

    let (mut created, mut skipped) = (0u32, 0u32);
    for (tool, _) in TOOLS {
        let link = target_dir.join(tool);
        if std::fs::symlink_metadata(&link).is_ok() {
            if force {
                if let Err(e) = std::fs::remove_file(&link) {
                    eprintln!(
                        "hfst install-symlinks: cannot replace {}: {e}",
                        link.display()
                    );
                    return 1;
                }
            } else {
                skipped += 1;
                continue;
            }
        }
        #[cfg(unix)]
        let res = std::os::unix::fs::symlink(&link_target, &link);
        #[cfg(not(unix))]
        let res = std::fs::hard_link(&exe, &link);
        if let Err(e) = res {
            eprintln!(
                "hfst install-symlinks: cannot create {}: {e}",
                link.display()
            );
            return 1;
        }
        created += 1;
    }
    println!(
        "{created} link(s) created in {}{}",
        target_dir.display(),
        if skipped > 0 {
            format!(", {skipped} existing skipped (use --force to replace)")
        } else {
            String::new()
        }
    );
    0
}

fn build_cli() -> Command {
    let mut cmd = Command::new("hfst")
        .version(env!("CARGO_PKG_VERSION"))
        .about("HFST command-line tools: one binary, one subcommand per tool")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("install-symlinks")
                .about("Create hfst-<tool> symlinks for all legacy tool names")
                .disable_help_flag(true)
                .arg(
                    Arg::new("args")
                        .num_args(0..)
                        .allow_hyphen_values(true)
                        .trailing_var_arg(true)
                        .help("[DIR] [--force]"),
                ),
        );
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
    // XRE/xfst/lexc/twolc compilation recurses over the parse tree (the nfst-*
    // parsers and the evaluators each descend per node), so a large real-world
    // grammar — an omorfi-scale regex, a deeply nested lexicon — can exhaust the
    // default 8 MiB main-thread stack and abort. Run the whole tool on a worker
    // thread with a generous stack so valid input compiles (hfst/hfst#287).
    const STACK_SIZE: usize = 512 * 1024 * 1024;
    let worker = std::thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(run_main)
        .expect("spawn hfst worker thread");
    if worker.join().is_err() {
        // A tool panicked; its message was already printed by the panic hook.
        std::process::exit(101);
    }
}

fn run_main() {
    let argv: Vec<String> = std::env::args().collect();

    // Basename dispatch: symlink/hardlink/copy invocation as an original
    // binary name. argv passes through completely unchanged.
    let basename = invoked_basename(argv.first().map(String::as_str).unwrap_or_default());
    if basename != "hfst"
        && let Some(run) = find_tool(&basename)
    {
        std::process::exit(run(argv));
    }
    // Unknown basename: fall through to the subcommand interface.

    // Subcommand dispatch: argv[1] = tool name minus "hfst-". The tool's args
    // are forwarded raw (clap never sees them), so the old getopt flags pass
    // through untouched.
    if let Some(sub) = argv.get(1) {
        if sub == "install-symlinks" {
            std::process::exit(install_symlinks(&argv[2..]));
        }
        if !sub.starts_with('-')
            && let Some(run) = find_tool(&format!("hfst-{sub}"))
        {
            let mut tool_argv = Vec::with_capacity(argv.len() - 1);
            tool_argv.push(format!("hfst {sub}"));
            tool_argv.extend(argv[2..].iter().cloned());
            #[cfg(feature = "dhat-heap")]
            let profiler = dhat::Profiler::new_heap();
            let code = run(tool_argv);
            // Flush dhat-heap.json before process::exit skips destructors.
            #[cfg(feature = "dhat-heap")]
            drop(profiler);
            std::process::exit(code);
        }
    }

    // No tool matched: let clap render --help/--version, the subcommand
    // listing, or the unknown-subcommand error. Every real subcommand was
    // already dispatched above, so this only returns for clap's own paths
    // (e.g. 'hfst help <sub>' exits inside get_matches_from).
    build_cli().get_matches_from(argv);
}
