//! The hfst command-line tools as library modules, one per former
//! standalone binary. Each module has an 'execute(args: Vec<String>) ->
//! ToolResult' entry point (the former real_main; args[0] is the program
//! name used in messages). The single 'hfst' multiplexer binary dispatches
//! to these via the TOOLS table below, keyed by the original binary names,
//! and maps the result to the process exit code with cli::exit_code.
//!
//! Small tools are grouped into families, each a module with one child
//! file per tool, re-exported below so every 'tools::<tool>' path is the
//! same whether the tool stands alone or belongs to a family:
//!
//! - `simple.rs`: affix_guessify, determinize, eliminate_flags, insert_freely,
//!   invert, kill_paths, minimize, multiply,
//!   preprocess_for_optimized_lookup_format, project, prune_alphabet,
//!   push_labels, push_weights, realign, remove_epsilons, repeat,
//!   reverse
//! - `inspect.rs`: dump_alphabets, edit_metadata, head, info, name, split,
//!   strip_header, tail, traverse
//! - `binary.rs`: binary_tool, check_alpha, compare, compose, concatenate, conjunct,
//!   disjunct, priority_disjunct, shuffle, subtract
//! - `convert.rs`: expand_equivalences, format, fst2fst, fst2txt
//! - `compile.rs`: guessify, pmatch2fst, twolc
//! - `apply.rs`: guess, pmatch, tokenize

use crate::cli::ToolResult;

mod apply;
mod binary;
mod compile;
mod convert;
mod inspect;
mod simple;

pub mod bhfst;
pub mod compose_intersect;
pub mod flookup;
pub mod fst2strings;
pub mod grep;
pub mod lexc_compiler;
pub mod lookup;
pub mod optimized_lookup;
pub mod pair_test;
pub mod regexp2fst;
pub mod reweight;
pub mod strings2fst;
pub mod substitute;
pub mod summarize;
pub mod txt2fst;
pub mod xfst;

// The family modules' tools, re-exported so 'tools::<tool>' addresses a
// tool by name regardless of which file it lives in.
pub use apply::{guess, pmatch, tokenize};
pub use binary::{
    binary_tool, check_alpha, compare, compose, concatenate, conjunct, disjunct, priority_disjunct,
    shuffle, subtract,
};
pub use compile::{guessify, pmatch2fst, twolc};
pub use convert::{expand_equivalences, format, fst2fst, fst2txt};
pub use inspect::{
    dump_alphabets, edit_metadata, head, info, name, split, strip_header, tail, traverse,
};
pub use simple::{
    affix_guessify, determinize, eliminate_flags, insert_freely, invert, kill_paths, minimize,
    multiply, preprocess_for_optimized_lookup_format, project, prune_alphabet, push_labels,
    push_weights, realign, remove_epsilons, repeat, reverse,
};

/// A tool's `execute` entry point: argv in, the tool's outcome out. The
/// dispatcher turns that outcome into the process exit code.
// [spec:hfst:req:cli.main]
pub type ToolRun = fn(Vec<String>) -> ToolResult;

/// Dispatch table mapping the original standalone binary names to the
/// tools' entry points and the one-line about strings the `hfst`
/// multiplexer shows in its subcommand listing (each taken from the
/// tool's usage summary line — the sentence after "Usage:" in its
/// print_usage). Alias names (the C++ suite installed several of these,
/// plus the British spellings Giella builds use) map to the same entry
/// points.
// [spec:hfst:req:cli.dispatch]
pub const TOOLS: &[(&str, ToolRun, &str)] = &[
    (
        "hfst-affix-guessify",
        affix_guessify::execute,
        "Create weighted affix guesser from automaton",
    ),
    // aliases. Every name the C++ suite installs for a tool this port
    // implements must appear here: a missing alias does not fail loudly, it
    // silently resolves to whatever hfst binary sits further down PATH, so a
    // build can mix Rust and C++ tools without any signal.
    (
        "hfst-lexc",
        lexc_compiler::execute,
        "Compile lexc files into transducer (alias)",
    ),
    (
        "hfst-union",
        disjunct::execute,
        "Disjunct (union, OR) two transducers (alias)",
    ),
    (
        "hfst-minus",
        subtract::execute,
        "Subtract (minus) two transducers (alias)",
    ),
    (
        "hfst-intersect",
        conjunct::execute,
        "Conjunct (intersect, AND) two transducers (alias)",
    ),
    (
        "hfst-expand",
        fst2strings::execute,
        "Display the strings recognized by a transducer",
    ),
    (
        "hfst-priority-union",
        priority_disjunct::execute,
        "Disjunct (union, OR) two transducers",
    ),
    // British spellings (the C++ suite symlinks these; Giella builds use them)
    (
        "hfst-tokenise",
        tokenize::execute,
        "perform matching/lookup on text streams (alias)",
    ),
    (
        "hfst-optimised-lookup",
        optimized_lookup::execute,
        "Run a transducer on standard input (one word per line) and print analyses (alias)",
    ),
    (
        "hfst-determinise",
        determinize::execute,
        "Determinize a transducer",
    ),
    ("hfst-minimise", minimize::execute, "Minimize a transducer"),
    (
        "hfst-summarise",
        summarize::execute,
        "Calculate the properties of a transducer",
    ),
    (
        "hfst-binary-tool",
        binary_tool::execute,
        "Do things with two transducers",
    ),
    (
        "hfst-bhfst",
        bhfst::execute,
        "Pack a THFST acceptor/errmodel pair (+ speller metadata) into a BHFST archive",
    ),
    (
        "hfst-check-alpha",
        check_alpha::execute,
        "Compare the compatibility of alphabets between INFILEs",
    ),
    ("hfst-compare", compare::execute, "Compare two transducers"),
    ("hfst-compose", compose::execute, "Compose two transducers"),
    (
        "hfst-compose-intersect",
        compose_intersect::execute,
        "Compose a lexicon with one or more rule transducers.",
    ),
    (
        "hfst-concatenate",
        concatenate::execute,
        "Concatenate two transducers",
    ),
    (
        "hfst-conjunct",
        conjunct::execute,
        "Conjunct (intersect, AND) two transducers",
    ),
    (
        "hfst-determinize",
        determinize::execute,
        "Determinize a transducer",
    ),
    (
        "hfst-disjunct",
        disjunct::execute,
        "Disjunct (union, OR) two transducers",
    ),
    (
        "hfst-dump-alphabets",
        dump_alphabets::execute,
        "Print alphabets of automaton",
    ),
    (
        "hfst-edit-metadata",
        edit_metadata::execute,
        "Name a transducer",
    ),
    (
        "hfst-eliminate-flags",
        eliminate_flags::execute,
        "Eliminate flags from a transducer",
    ),
    (
        "hfst-expand-equivalences",
        expand_equivalences::execute,
        "Extend transducer arcs for equivalence classes",
    ),
    (
        "hfst-flookup",
        flookup::execute,
        "Perform transducer lookup (apply), from right to left",
    ),
    (
        "hfst-format",
        format::execute,
        "determine HFST transducer format",
    ),
    (
        "hfst-fst2fst",
        fst2fst::execute,
        "Convert transducers between binary formats",
    ),
    (
        "hfst-fst2strings",
        fst2strings::execute,
        "Display the strings recognized by a transducer",
    ),
    (
        "hfst-fst2txt",
        fst2txt::execute,
        "Print transducer in AT&T, dot, prolog or pckimmo format",
    ),
    (
        "hfst-grep",
        grep::execute,
        "Search for PATTERN in each FILE or standard input.",
    ),
    (
        "hfst-guess",
        guess::execute,
        "Use a guesser (and generator) to guess analyses or inflectional paradigms of unknown words",
    ),
    (
        "hfst-guessify",
        guessify::execute,
        "Compile a morphological analyzer into a guesser and generator.",
    ),
    (
        "hfst-head",
        head::execute,
        "Get first transducers from an archive",
    ),
    (
        "hfst-info",
        info::execute,
        "show or test HFST versions and features",
    ),
    (
        "hfst-insert-freely",
        insert_freely::execute,
        "Freely insert a symbol (pair)",
    ),
    ("hfst-invert", invert::execute, "Invert a transducer"),
    (
        "hfst-kill-paths",
        kill_paths::execute,
        "Kill all paths with specific symbols",
    ),
    (
        "hfst-lexc-compiler",
        lexc_compiler::execute,
        "Compile lexc files into transducer",
    ),
    (
        "hfst-lookup",
        lookup::execute,
        "perform transducer lookup (apply)",
    ),
    ("hfst-minimize", minimize::execute, "Minimize a transducer"),
    (
        "hfst-multiply",
        multiply::execute,
        "Use first transducer of an archive repeatedly",
    ),
    ("hfst-name", name::execute, "Name a transducer"),
    (
        "hfst-optimized-lookup",
        optimized_lookup::execute,
        "Run a transducer on standard input (one word per line) and print analyses",
    ),
    (
        "hfst-pair-test",
        pair_test::execute,
        "pair test for a twolc rule file.",
    ),
    (
        "hfst-pmatch",
        pmatch::execute,
        "perform matching/lookup on text streams",
    ),
    (
        "hfst-pmatch2fst",
        pmatch2fst::execute,
        "Compile regular expressions into transducer(s) (Experimental version)",
    ),
    (
        "hfst-preprocess-for-optimized-lookup-format",
        preprocess_for_optimized_lookup_format::execute,
        "Remove epsilons from a transducer",
    ),
    (
        "hfst-priority-disjunct",
        priority_disjunct::execute,
        "Disjunct (union, OR) two transducers",
    ),
    (
        "hfst-project",
        project::execute,
        "Project (extract a level) transducer",
    ),
    (
        "hfst-prune-alphabet",
        prune_alphabet::execute,
        "Prune the alphabet of a transducer",
    ),
    (
        "hfst-push-labels",
        push_labels::execute,
        "Push labels of transducer",
    ),
    (
        "hfst-push-weights",
        push_weights::execute,
        "Push weights of transducer",
    ),
    (
        "hfst-realign",
        realign::execute,
        "Realign a transducer by pushing labels to the start",
    ),
    (
        "hfst-regexp2fst",
        regexp2fst::execute,
        "Compile (weighted) regular expressions into transducer(s)",
    ),
    (
        "hfst-remove-epsilons",
        remove_epsilons::execute,
        "Remove epsilons from a transducer",
    ),
    ("hfst-repeat", repeat::execute, "Repeat transducer"),
    ("hfst-reverse", reverse::execute, "Reverse a transducer"),
    (
        "hfst-reweight",
        reweight::execute,
        "Reweight transducer weights simply",
    ),
    ("hfst-shuffle", shuffle::execute, "Shuffle two transducers"),
    (
        "hfst-split",
        split::execute,
        "Extract transducers from archive with systematic file names",
    ),
    (
        "hfst-strings2fst",
        strings2fst::execute,
        "Compile string pairs and pair-strings into transducer(s)",
    ),
    (
        "hfst-strip-header",
        strip_header::execute,
        "Remove any HFST3 headers",
    ),
    (
        "hfst-substitute",
        substitute::execute,
        "Relabel transducer arcs",
    ),
    (
        "hfst-subtract",
        subtract::execute,
        "Subtract (minus) two transducers",
    ),
    (
        "hfst-summarize",
        summarize::execute,
        "Calculate the properties of a transducer",
    ),
    (
        "hfst-tail",
        tail::execute,
        "Get last transducers from an archive",
    ),
    (
        "hfst-tokenize",
        tokenize::execute,
        "perform matching/lookup on text streams",
    ),
    (
        "hfst-traverse",
        traverse::execute,
        "Walk through the transducer arc by arc",
    ),
    (
        "hfst-twolc",
        twolc::execute,
        "Read a twolc grammar, compile it and store it",
    ),
    (
        "hfst-txt2fst",
        txt2fst::execute,
        "Convert AT&T or prolog format into a binary transducer",
    ),
    (
        "hfst-xfst",
        xfst::execute,
        "Compile XFST scripts or execute XFST commands interactively",
    ),
];
