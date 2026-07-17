//! The hfst command-line tools as library modules, one per former
//! standalone binary. Each module exposes 'pub fn run(args: Vec<String>) ->
//! i32' (the former real_main; args[0] is the program name used in
//! messages). The single 'hfst' multiplexer binary dispatches to these via
//! the TOOLS table below, keyed by the original binary names.

pub mod affix_guessify;
pub mod bhfst;
pub mod binary_tool;
pub mod check_alpha;
pub mod compare;
pub mod compose;
pub mod compose_intersect;
pub mod concatenate;
pub mod conjunct;
pub mod determinize;
pub mod disjunct;
pub mod dump_alphabets;
pub mod edit_metadata;
pub mod eliminate_flags;
pub mod expand_equivalences;
pub mod flookup;
pub mod format;
pub mod fst2fst;
pub mod fst2strings;
pub mod fst2txt;
pub mod grep;
pub mod guess;
pub mod guessify;
pub mod head;
pub mod info;
pub mod insert_freely;
pub mod invert;
pub mod kill_paths;
pub mod lexc_compiler;
pub mod lookup;
pub mod minimize;
pub mod multiply;
pub mod name;
pub mod optimized_lookup;
pub mod pair_test;
pub mod pmatch;
pub mod pmatch2fst;
pub mod preprocess_for_optimized_lookup_format;
pub mod priority_disjunct;
pub mod project;
pub mod prune_alphabet;
pub mod push_labels;
pub mod push_weights;
pub mod realign;
pub mod regexp2fst;
pub mod remove_epsilons;
pub mod repeat;
pub mod reverse;
pub mod reweight;
pub mod shuffle;
pub mod split;
pub mod strings2fst;
pub mod strip_header;
pub mod substitute;
pub mod subtract;
pub mod summarize;
pub mod tail;
pub mod tokenize;
pub mod traverse;
pub mod twolc;
pub mod txt2fst;
pub mod xfst;

/// A tool's `run` entry point: argv in, process exit code out.
pub type ToolRun = fn(Vec<String>) -> i32;

/// Dispatch table mapping the original standalone binary names to the
/// tools' run entry points. Alias names (the C++ suite installed several
/// of these, plus the British spellings Giella builds use) map to the same
/// entry points.
pub const TOOLS: &[(&str, ToolRun)] = &[
    ("hfst-affix-guessify", affix_guessify::run),
    // aliases
    ("hfst-lexc", lexc_compiler::run),
    ("hfst-union", disjunct::run),
    ("hfst-minus", subtract::run),
    ("hfst-intersect", conjunct::run),
    ("hfst-tokenise", tokenize::run),
    ("hfst-optimised-lookup", optimized_lookup::run),
    ("hfst-binary-tool", binary_tool::run),
    ("hfst-bhfst", bhfst::run),
    ("hfst-check-alpha", check_alpha::run),
    ("hfst-compare", compare::run),
    ("hfst-compose", compose::run),
    ("hfst-compose-intersect", compose_intersect::run),
    ("hfst-concatenate", concatenate::run),
    ("hfst-conjunct", conjunct::run),
    ("hfst-determinize", determinize::run),
    ("hfst-disjunct", disjunct::run),
    ("hfst-dump-alphabets", dump_alphabets::run),
    ("hfst-edit-metadata", edit_metadata::run),
    ("hfst-eliminate-flags", eliminate_flags::run),
    ("hfst-expand-equivalences", expand_equivalences::run),
    ("hfst-flookup", flookup::run),
    ("hfst-format", format::run),
    ("hfst-fst2fst", fst2fst::run),
    ("hfst-fst2strings", fst2strings::run),
    ("hfst-fst2txt", fst2txt::run),
    ("hfst-grep", grep::run),
    ("hfst-guess", guess::run),
    ("hfst-guessify", guessify::run),
    ("hfst-head", head::run),
    ("hfst-info", info::run),
    ("hfst-insert-freely", insert_freely::run),
    ("hfst-invert", invert::run),
    ("hfst-kill-paths", kill_paths::run),
    ("hfst-lexc-compiler", lexc_compiler::run),
    ("hfst-lookup", lookup::run),
    ("hfst-minimize", minimize::run),
    ("hfst-multiply", multiply::run),
    ("hfst-name", name::run),
    ("hfst-optimized-lookup", optimized_lookup::run),
    ("hfst-pair-test", pair_test::run),
    ("hfst-pmatch", pmatch::run),
    ("hfst-pmatch2fst", pmatch2fst::run),
    (
        "hfst-preprocess-for-optimized-lookup-format",
        preprocess_for_optimized_lookup_format::run,
    ),
    ("hfst-priority-disjunct", priority_disjunct::run),
    ("hfst-project", project::run),
    ("hfst-prune-alphabet", prune_alphabet::run),
    ("hfst-push-labels", push_labels::run),
    ("hfst-push-weights", push_weights::run),
    ("hfst-realign", realign::run),
    ("hfst-regexp2fst", regexp2fst::run),
    ("hfst-remove-epsilons", remove_epsilons::run),
    ("hfst-repeat", repeat::run),
    ("hfst-reverse", reverse::run),
    ("hfst-reweight", reweight::run),
    ("hfst-shuffle", shuffle::run),
    ("hfst-split", split::run),
    ("hfst-strings2fst", strings2fst::run),
    ("hfst-strip-header", strip_header::run),
    ("hfst-substitute", substitute::run),
    ("hfst-subtract", subtract::run),
    ("hfst-summarize", summarize::run),
    ("hfst-tail", tail::run),
    ("hfst-tokenize", tokenize::run),
    ("hfst-traverse", traverse::run),
    ("hfst-twolc", twolc::run),
    ("hfst-txt2fst", txt2fst::run),
    ("hfst-xfst", xfst::run),
];
