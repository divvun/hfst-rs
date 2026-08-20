//! The hfst command-line tools as library modules, one per former
//! standalone binary. Each module exposes 'pub fn run(args: Vec<String>) ->
//! i32' (the former real_main; args[0] is the program name used in
//! messages). The single 'hfst' multiplexer binary dispatches to these via
//! the TOOLS table below, keyed by the original binary names.
//!
//! Small tools are grouped into family files, each holding its tools as
//! inline modules and re-exported below so every 'tools::<tool>' path is
//! the same whether the tool has its own file or shares a family one:
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

/// A tool's `run` entry point: argv in, process exit code out.
pub type ToolRun = fn(Vec<String>) -> i32;

/// Dispatch table mapping the original standalone binary names to the
/// tools' run entry points and the one-line about strings the `hfst`
/// multiplexer shows in its subcommand listing (each taken from the
/// tool's usage summary line — the sentence after "Usage:" in its
/// print_usage). Alias names (the C++ suite installed several of these,
/// plus the British spellings Giella builds use) map to the same entry
/// points.
pub const TOOLS: &[(&str, ToolRun, &str)] = &[
    (
        "hfst-affix-guessify",
        affix_guessify::run,
        "Create weighted affix guesser from automaton",
    ),
    // aliases. Every name the C++ suite installs for a tool this port
    // implements must appear here: a missing alias does not fail loudly, it
    // silently resolves to whatever hfst binary sits further down PATH, so a
    // build can mix Rust and C++ tools without any signal.
    (
        "hfst-lexc",
        lexc_compiler::run,
        "Compile lexc files into transducer (alias)",
    ),
    (
        "hfst-union",
        disjunct::run,
        "Disjunct (union, OR) two transducers (alias)",
    ),
    (
        "hfst-minus",
        subtract::run,
        "Subtract (minus) two transducers (alias)",
    ),
    (
        "hfst-intersect",
        conjunct::run,
        "Conjunct (intersect, AND) two transducers (alias)",
    ),
    (
        "hfst-expand",
        fst2strings::run,
        "Display the strings recognized by a transducer",
    ),
    (
        "hfst-priority-union",
        priority_disjunct::run,
        "Disjunct (union, OR) two transducers",
    ),
    // British spellings (the C++ suite symlinks these; Giella builds use them)
    (
        "hfst-tokenise",
        tokenize::run,
        "perform matching/lookup on text streams (alias)",
    ),
    (
        "hfst-optimised-lookup",
        optimized_lookup::run,
        "Run a transducer on standard input (one word per line) and print analyses (alias)",
    ),
    (
        "hfst-determinise",
        determinize::run,
        "Determinize a transducer",
    ),
    ("hfst-minimise", minimize::run, "Minimize a transducer"),
    (
        "hfst-summarise",
        summarize::run,
        "Calculate the properties of a transducer",
    ),
    (
        "hfst-binary-tool",
        binary_tool::run,
        "Do things with two transducers",
    ),
    (
        "hfst-bhfst",
        bhfst::run,
        "Pack a THFST acceptor/errmodel pair (+ speller metadata) into a BHFST archive",
    ),
    (
        "hfst-check-alpha",
        check_alpha::run,
        "Compare the compatibility of alphabets between INFILEs",
    ),
    ("hfst-compare", compare::run, "Compare two transducers"),
    ("hfst-compose", compose::run, "Compose two transducers"),
    (
        "hfst-compose-intersect",
        compose_intersect::run,
        "Compose a lexicon with one or more rule transducers.",
    ),
    (
        "hfst-concatenate",
        concatenate::run,
        "Concatenate two transducers",
    ),
    (
        "hfst-conjunct",
        conjunct::run,
        "Conjunct (intersect, AND) two transducers",
    ),
    (
        "hfst-determinize",
        determinize::run,
        "Determinize a transducer",
    ),
    (
        "hfst-disjunct",
        disjunct::run,
        "Disjunct (union, OR) two transducers",
    ),
    (
        "hfst-dump-alphabets",
        dump_alphabets::run,
        "Print alphabets of automaton",
    ),
    (
        "hfst-edit-metadata",
        edit_metadata::run,
        "Name a transducer",
    ),
    (
        "hfst-eliminate-flags",
        eliminate_flags::run,
        "Eliminate flags from a transducer",
    ),
    (
        "hfst-expand-equivalences",
        expand_equivalences::run,
        "Extend transducer arcs for equivalence classes",
    ),
    (
        "hfst-flookup",
        flookup::run,
        "Perform transducer lookup (apply), from right to left",
    ),
    (
        "hfst-format",
        format::run,
        "determine HFST transducer format",
    ),
    (
        "hfst-fst2fst",
        fst2fst::run,
        "Convert transducers between binary formats",
    ),
    (
        "hfst-fst2strings",
        fst2strings::run,
        "Display the strings recognized by a transducer",
    ),
    (
        "hfst-fst2txt",
        fst2txt::run,
        "Print transducer in AT&T, dot, prolog or pckimmo format",
    ),
    (
        "hfst-grep",
        grep::run,
        "Search for PATTERN in each FILE or standard input.",
    ),
    (
        "hfst-guess",
        guess::run,
        "Use a guesser (and generator) to guess analyses or inflectional paradigms of unknown words",
    ),
    (
        "hfst-guessify",
        guessify::run,
        "Compile a morphological analyzer into a guesser and generator.",
    ),
    (
        "hfst-head",
        head::run,
        "Get first transducers from an archive",
    ),
    (
        "hfst-info",
        info::run,
        "show or test HFST versions and features",
    ),
    (
        "hfst-insert-freely",
        insert_freely::run,
        "Freely insert a symbol (pair)",
    ),
    ("hfst-invert", invert::run, "Invert a transducer"),
    (
        "hfst-kill-paths",
        kill_paths::run,
        "Kill all paths with specific symbols",
    ),
    (
        "hfst-lexc-compiler",
        lexc_compiler::run,
        "Compile lexc files into transducer",
    ),
    (
        "hfst-lookup",
        lookup::run,
        "perform transducer lookup (apply)",
    ),
    ("hfst-minimize", minimize::run, "Minimize a transducer"),
    (
        "hfst-multiply",
        multiply::run,
        "Use first transducer of an archive repeatedly",
    ),
    ("hfst-name", name::run, "Name a transducer"),
    (
        "hfst-optimized-lookup",
        optimized_lookup::run,
        "Run a transducer on standard input (one word per line) and print analyses",
    ),
    (
        "hfst-pair-test",
        pair_test::run,
        "pair test for a twolc rule file.",
    ),
    (
        "hfst-pmatch",
        pmatch::run,
        "perform matching/lookup on text streams",
    ),
    (
        "hfst-pmatch2fst",
        pmatch2fst::run,
        "Compile regular expressions into transducer(s) (Experimental version)",
    ),
    (
        "hfst-preprocess-for-optimized-lookup-format",
        preprocess_for_optimized_lookup_format::run,
        "Remove epsilons from a transducer",
    ),
    (
        "hfst-priority-disjunct",
        priority_disjunct::run,
        "Disjunct (union, OR) two transducers",
    ),
    (
        "hfst-project",
        project::run,
        "Project (extract a level) transducer",
    ),
    (
        "hfst-prune-alphabet",
        prune_alphabet::run,
        "Prune the alphabet of a transducer",
    ),
    (
        "hfst-push-labels",
        push_labels::run,
        "Push labels of transducer",
    ),
    (
        "hfst-push-weights",
        push_weights::run,
        "Push weights of transducer",
    ),
    (
        "hfst-realign",
        realign::run,
        "Realign a transducer by pushing labels to the start",
    ),
    (
        "hfst-regexp2fst",
        regexp2fst::run,
        "Compile (weighted) regular expressions into transducer(s)",
    ),
    (
        "hfst-remove-epsilons",
        remove_epsilons::run,
        "Remove epsilons from a transducer",
    ),
    ("hfst-repeat", repeat::run, "Repeat transducer"),
    ("hfst-reverse", reverse::run, "Reverse a transducer"),
    (
        "hfst-reweight",
        reweight::run,
        "Reweight transducer weights simply",
    ),
    ("hfst-shuffle", shuffle::run, "Shuffle two transducers"),
    (
        "hfst-split",
        split::run,
        "Extract transducers from archive with systematic file names",
    ),
    (
        "hfst-strings2fst",
        strings2fst::run,
        "Compile string pairs and pair-strings into transducer(s)",
    ),
    (
        "hfst-strip-header",
        strip_header::run,
        "Remove any HFST3 headers",
    ),
    (
        "hfst-substitute",
        substitute::run,
        "Relabel transducer arcs",
    ),
    (
        "hfst-subtract",
        subtract::run,
        "Subtract (minus) two transducers",
    ),
    (
        "hfst-summarize",
        summarize::run,
        "Calculate the properties of a transducer",
    ),
    (
        "hfst-tail",
        tail::run,
        "Get last transducers from an archive",
    ),
    (
        "hfst-tokenize",
        tokenize::run,
        "perform matching/lookup on text streams",
    ),
    (
        "hfst-traverse",
        traverse::run,
        "Walk through the transducer arc by arc",
    ),
    (
        "hfst-twolc",
        twolc::run,
        "Read a twolc grammar, compile it and store it",
    ),
    (
        "hfst-txt2fst",
        txt2fst::run,
        "Convert AT&T or prolog format into a binary transducer",
    ),
    (
        "hfst-xfst",
        xfst::run,
        "Compile XFST scripts or execute XFST commands interactively",
    ),
];
