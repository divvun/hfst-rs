//! A frozen census of trait-impl methods whose whole body is a constant.
//!
//! The companion to `backend_answers.rs`: that file asserts behaviour, this one
//! catches the shape at the moment it is written. A trait method whose body is
//! `self.clone()` / `false` / `Ok(())` / `None` and nothing else is either an
//! honest statement that this backend has nothing to do, or the bug class —
//! real logic that was never ported, returning a plausible value the caller
//! consumes as fact. The two are indistinguishable from the source, so the
//! census does not judge: it requires that each one be written down here with
//! the reason it is honest.
//!
//! It is worth having because it reaches where behavioural assertions cannot.
//! `FomaTransducer::push_weights` returning `self.clone()` is correct and no
//! test can prove it — foma's line table has no weight field, so there is no
//! observable difference between the right answer and a stub. Sitting in this
//! list with a reason is the only check such a method can get.
//!
//! Scope is every trait impl under `crates/*/src`, and the whole workspace
//! currently holds seven. A new entry means a reviewer had to think; that is
//! the point, and at this rate it is not a tax.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// `(file, method, body, why this constant is the honest answer)`.
const SANCTIONED: &[(&str, &str, &str, &str)] = &[
    (
        "crates/hfst/src/backend.rs",
        "substitute_symbol_fast",
        "None",
        "The C++ fast path is dead code ('if (false && ...)'), so declining sends \
         the facade down the generic basic-transducer path it always took.",
    ),
    (
        "crates/hfst/src/backend_foma.rs",
        "has_weights",
        "false",
        "foma's line table has no weight field, so no net it holds can carry a \
         weight. Stated in its own body rather than inherited from the trait.",
    ),
    (
        "crates/hfst/src/backend_foma.rs",
        "n_best",
        "self.clone()",
        "Unweighted: with every path at weight 0.0 the n best paths are all of \
         them, so there is no shortest-path pruning to do.",
    ),
    (
        "crates/hfst/src/backend_foma.rs",
        "set_final_weights",
        "self.clone()",
        "Unweighted: there is nowhere to put the weight. Diverges from tropical \
         by design, and `has_weights` above says so.",
    ),
    (
        "crates/hfst/src/backend_foma.rs",
        "push_labels",
        "self.clone()",
        "Unweighted: label pushing moves weight mass toward one end, and there \
         is none to move.",
    ),
    (
        "crates/hfst/src/backend_foma.rs",
        "push_weights",
        "self.clone()",
        "Unweighted: as push_labels.",
    ),
    (
        "crates/hfst/src/backend_foma.rs",
        "transform_weights",
        "self.clone()",
        "Unweighted: the transform has no weights to apply itself to.",
    ),
];

/// Bodies that count as constant. Deliberately a closed list — anything with
/// real structure is not this shape, and widening it would start catching
/// one-line delegations, which are the opposite of the defect.
const CONSTANT_BODIES: &[&str] = &[
    "self.clone()",
    "Ok(())",
    "None",
    "true",
    "false",
    "0",
    "0.0",
    "Vec::new()",
    "String::new()",
    "BTreeSet::new()",
    "StringSet::new()",
    "Default::default()",
    "Self::default()",
    "Self::empty()",
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two levels under the workspace root")
        .to_path_buf()
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// A method in a trait impl whose body is one constant expression.
type Finding = (String, String, String);

/// Reads the shape `cargo fmt` produces: a trait impl opens at column 0 and
/// closes with a `}` at column 0, and its methods sit at one indent with their
/// closing brace at `    }`. `cargo fmt --check` is already a gate, so that
/// shape holds; a scan that stops matching shows up as a census mismatch rather
/// than as silence.
fn scan(path: &Path, rel: &str, out: &mut Vec<Finding>) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    let lines: Vec<&str> = text.lines().collect();
    let mut in_trait_impl = false;

    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("impl") {
            // `impl Trait for Type` — an inherent `impl Type` has no `for`.
            in_trait_impl = line.contains(" for ");
        } else if line.starts_with('}') {
            in_trait_impl = false;
        }
        if !in_trait_impl {
            continue;
        }
        let Some(name) = line
            .strip_prefix("    fn ")
            .and_then(|rest| rest.split(['(', '<']).next())
        else {
            continue;
        };

        // Walk to the brace that opens the body; the signature may wrap.
        let Some(open) = (i..lines.len().min(i + 12)).find(|k| lines[*k].ends_with('{')) else {
            continue;
        };
        let Some(close) = (open + 1..lines.len().min(open + 40)).find(|k| lines[*k] == "    }")
        else {
            continue;
        };
        let body: Vec<&str> = lines[open + 1..close]
            .iter()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with("//"))
            .collect();
        if body.len() == 1 && CONSTANT_BODIES.contains(&body[0]) {
            out.push((rel.to_string(), name.to_string(), body[0].to_string()));
        }
    }
}

#[test]
fn constant_bodied_trait_methods_are_all_sanctioned() {
    let root = workspace_root();
    let crates = root.join("crates");
    let mut members: Vec<PathBuf> = std::fs::read_dir(&crates)
        .expect("the workspace has a crates/ directory")
        .flatten()
        .map(|e| e.path().join("src"))
        .filter(|p| p.is_dir())
        .collect();
    members.sort();
    assert!(!members.is_empty(), "found no crate source trees to scan");

    let mut found: Vec<Finding> = Vec::new();
    for src in &members {
        let mut files = Vec::new();
        rust_sources(src, &mut files);
        files.sort();
        for file in &files {
            let rel = file
                .strip_prefix(&root)
                .unwrap_or(file)
                .to_string_lossy()
                .replace('\\', "/");
            scan(file, &rel, &mut found);
        }
    }

    let found: BTreeSet<Finding> = found.into_iter().collect();
    let sanctioned: BTreeSet<Finding> = SANCTIONED
        .iter()
        .map(|(f, m, b, _)| ((*f).to_string(), (*m).to_string(), (*b).to_string()))
        .collect();

    let new: Vec<&Finding> = found.difference(&sanctioned).collect();
    assert!(
        new.is_empty(),
        "a trait method's whole body is now a constant, and it is not in the \
         census. If the constant IS the honest answer for this backend, add it \
         to SANCTIONED with the reason. If real logic is missing, this is the \
         defect the census exists to catch — port it, or fail loudly with \
         `bail!` instead of returning a value the caller will believe.\n{new:#?}"
    );

    let gone: Vec<&Finding> = sanctioned.difference(&found).collect();
    assert!(
        gone.is_empty(),
        "the census lists methods that no longer have a constant body — delete \
         these entries so the list stays a statement about the code as it \
         is.\n{gone:#?}"
    );
}
