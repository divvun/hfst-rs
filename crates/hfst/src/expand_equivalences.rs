//! Equivalence-class arc-extension logic lifted from
//! tools/src/hfst-expand-equivalences.cc.
//!
//! The tool extends a transducer's arcs so that single symbols are allowed to
//! map to whole equivalence classes. This module holds the two reusable,
//! transducer-level pieces: `read_tsv_extensions`, a de-C-ified parser for the
//! TSV extension-file format (the C++ used a getline/strstr/strndup loop), and
//! `expand_equivalences`, the add-extension/compose loop. The owning tool keeps
//! only its option handling, the std-stream plumbing, and the stream-driver
//! loop.

use std::io::BufRead;

use crate::hfst_symbol_defs::{internal_epsilon, internal_identity};
use crate::hfst_transducer::HfstTransducer;

// Which level(s) of the FSA the equivalence extensions apply to. Lifted verbatim
// from the tool-local enum.
// [spec:hfst:def:hfst-expand-equivalences.fsa-level-t]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FsaLevel {
    First,
    Second,
    Both,
}

// A parse error from `read_tsv_extensions`, carrying the 1-based line number and
// the message. The owning tool maps this onto error_at_line(file, line, message)
// — the library does not know the filename, so the caller supplies it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsvExtensionError {
    pub line: u32,
    pub message: String,
}

// Parse a TSV extension file into (from, to) pairs. Each non-comment line is
// `FROM<TAB>TO1<TAB>TO2...`, producing (FROM,TO1),(FROM,TO2),.... A tab-less line
// beginning with '#' is a comment; any other tab-less line is an error. An empty
// FROM or any empty TO field (including the one left by a trailing tab) is an
// error. This is a faithful, de-C-ified rewrite of the tool's getline/strstr/
// strndup loop: `line.find('\t')` replaces the first strstr, and
// `rest.split('\t')` reproduces the field walk — including the trailing-tab
// empty-field error — exactly as the C++ pointer arithmetic did.
pub fn read_tsv_extensions<R: BufRead>(
    reader: R,
) -> Result<Vec<(String, String)>, TsvExtensionError> {
    let mut pairs = Vec::new();
    for (i, line) in reader.lines().enumerate() {
        let line_n = (i + 1) as u32;
        // The C++ loop ran `while hfst_getline(...) != -1`; a read error is the
        // EOF-equivalent here, so stop.
        let Ok(line) = line else { break };
        // C: `if (*line == '\n') continue;` — skip an empty line.
        if line.is_empty() {
            continue;
        }
        let Some(tab_idx) = line.find('\t') else {
            // No tab: a leading '#' is a comment, anything else is an error.
            if line.starts_with('#') {
                continue;
            }
            return Err(TsvExtensionError {
                line: line_n,
                message: "At least one tab required per line".to_string(),
            });
        };
        let from = &line[..tab_idx];
        if from.is_empty() {
            return Err(TsvExtensionError {
                line: line_n,
                message: format!(
                    "First field is empty;\n\
                     if you REALLY want to extend epsilons as equivalent, use @0@ or {}",
                    internal_epsilon
                ),
            });
        }
        for to in line[tab_idx + 1..].split('\t') {
            if to.is_empty() {
                return Err(TsvExtensionError {
                    line: line_n,
                    message: format!(
                        "Extension field seems empty;\n\
                         if you REALLY mean something is equivalent to epsilons, use @0@ or {}",
                        internal_epsilon
                    ),
                });
            }
            pairs.push((from.to_string(), to.to_string()));
        }
    }
    Ok(pairs)
}

// Build the extension transducer from `pairs` — starting from an identity:identity
// pair and disjuncting each FROM:TO remap — close it (minimize, repeat_star,
// minimize), then apply it to `trans` at the requested level, returning the
// expanded transducer. This folds in the tool's add_extension helper (the per-pair
// disjunct) and the level-application block of its process_stream.
// [spec:hfst:def:hfst-expand-equivalences.add-extension-fn]
// [spec:hfst:sem:hfst-expand-equivalences.add-extension-fn]
pub fn expand_equivalences<B: crate::backend::AlgebraBackend>(
    mut trans: HfstTransducer<B>,
    pairs: &[(String, String)],
    level: FsaLevel,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut extensions = HfstTransducer::new_symbol_pair(internal_identity, internal_identity)?;
    for (from, to) in pairs {
        let remap = HfstTransducer::new_symbol_pair(from, to)?;
        extensions.disjunct(&remap, true)?;
    }
    extensions.minimize()?.repeat_star()?.minimize()?;
    Ok(match level {
        FsaLevel::Both => {
            trans.compose(&extensions, true)?;
            // C: trans = extensions->invert().compose(trans);
            extensions.invert()?.compose(&trans, true)?;
            extensions
        }
        FsaLevel::First => {
            // C: trans = extensions->invert().compose(trans);
            extensions.invert()?.compose(&trans, true)?;
            extensions
        }
        FsaLevel::Second => {
            trans.compose(&extensions, true)?;
            trans
        }
    })
}
