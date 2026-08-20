//! Every name the C++ HFST suite installs for a tool this port implements
//! must be dispatchable here.
//!
//! A missing name does not fail loudly. `hfst install-symlinks` iterates
//! TOOLS, so an absent entry simply never gets a symlink, and the bare name
//! then resolves to whatever `hfst` binary sits further down PATH — a
//! distro-packaged C++ build, typically. A Giella build calling
//! `hfst-minimise` would silently run the C++ tool while every neighbouring
//! step ran the Rust one, mixing the two with no signal and quietly
//! invalidating any "built with the Rust toolchain" claim.
//!
//! The C++ suite ships these as symlinks onto the canonical spelling
//! (verified against an installed hfst 3.17.1 tree), so each pair below must
//! dispatch to the same entry point.

use hfst_cli::tools::TOOLS;

fn run_for(name: &str) -> Option<hfst_cli::tools::ToolRun> {
    TOOLS
        .iter()
        .find(|(tool, _, _)| *tool == name)
        .map(|&(_, run, _)| run)
}

/// (alias, canonical) pairs the C++ suite symlinks together.
const ALIASES: &[(&str, &str)] = &[
    // British spellings — Giella builds use these
    ("hfst-determinise", "hfst-determinize"),
    ("hfst-minimise", "hfst-minimize"),
    ("hfst-summarise", "hfst-summarize"),
    ("hfst-tokenise", "hfst-tokenize"),
    ("hfst-optimised-lookup", "hfst-optimized-lookup"),
    // Historical / operator-name aliases
    ("hfst-expand", "hfst-fst2strings"),
    ("hfst-priority-union", "hfst-priority-disjunct"),
    ("hfst-union", "hfst-disjunct"),
    ("hfst-minus", "hfst-subtract"),
    ("hfst-intersect", "hfst-conjunct"),
    ("hfst-lexc", "hfst-lexc-compiler"),
];

#[test]
fn every_cpp_alias_dispatches_to_its_canonical_tool() {
    for (alias, canonical) in ALIASES {
        let a = run_for(alias).unwrap_or_else(|| {
            panic!(
                "'{alias}' is missing from TOOLS: it will get no symlink and \
                 will silently resolve to a C++ hfst further down PATH"
            )
        });
        let c = run_for(canonical)
            .unwrap_or_else(|| panic!("canonical tool '{canonical}' is missing from TOOLS"));
        assert!(
            std::ptr::fn_addr_eq(a, c),
            "'{alias}' does not dispatch to the same entry point as '{canonical}'"
        );
    }
}

#[test]
fn every_tool_name_is_hfst_prefixed_and_unique() {
    // bin/hfst.rs derives each subcommand by stripping "hfst-", and panics if
    // the prefix is absent; a duplicate would shadow a real tool.
    let mut seen = std::collections::BTreeSet::new();
    for (tool, _, about) in TOOLS {
        assert!(
            tool.strip_prefix("hfst-").is_some(),
            "TOOLS entry '{tool}' is not named hfst-<tool>"
        );
        assert!(seen.insert(*tool), "TOOLS lists '{tool}' more than once");
        // The about string is what `hfst --help` prints beside the subcommand;
        // an empty one leaves the tool listed with no description.
        assert!(
            !about.is_empty(),
            "TOOLS entry '{tool}' has no about string"
        );
    }
}
