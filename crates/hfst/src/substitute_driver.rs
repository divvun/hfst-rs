//! The label-substitution engine behind `hfst-substitute`, lifted out of the
//! tool that carried it: the choice of which of the five substitution shapes a
//! request names (pair-for-pair, label-for-label, pair-for-transducer,
//! label-for-transducer, or a delayed disjunction composed in at the end), the
//! delayed `--compose` accumulator, and the batched-versus-in-order strategy a
//! relabel file (`-F`) is applied with.
//!
//! The tool keeps its option parsing, its stream plumbing, the relabel file's
//! own line diagnostics and the output naming; what it hands over is the
//! per-transducer transform. Nothing here touches process stdin/stdout or
//! exits: the progress lines the engine would print go to a
//! [`SubstituteReporter`], and every fallible step returns
//! [`crate::error::Result`].
//!
//! Both the delayed flag and the batched substitution maps live on the engine
//! rather than on a request, because `hfst-substitute` accumulates them across
//! *every* transducer in the stream — the C++ tool held them in file-scope
//! statics, and an engine constructed once outside the stream loop reproduces
//! that lifetime exactly.

use std::collections::BTreeMap;

use crate::backend::AlgebraBackend;
use crate::error::Result;
use crate::hfst_data_types::{StringPair, Symbol};
use crate::hfst_transducer::HfstTransducer;

// [spec:hfst:def:hfst-substitute.hfst-symbol-substitutions]
/// A batch of symbol-for-symbol replacements, applied in one pass.
pub type HfstSymbolSubstitutions = BTreeMap<Symbol, Symbol>;
// [spec:hfst:def:hfst-substitute.hfst-symbol-pair-substitutions]
/// A batch of arc-for-arc replacements, applied in one pass.
pub type HfstSymbolPairSubstitutions = BTreeMap<StringPair, StringPair>;

/// The host's progress stream. `&self` so the engine can hold it across a
/// whole substitution without borrowing conflicts.
pub trait SubstituteReporter {
    /// A `--verbose` progress line, newline included.
    fn verbose(&self, msg: &str);
}

/// One substitution as the engine sees it: which label or arc to replace, and
/// what to replace it with. The four `from`/`to` fields mirror the tool's
/// options directly, because a relabel file rewrites them line by line and the
/// arm the engine picks depends on which of them are set.
#[derive(Default, Clone)]
pub struct SubstituteRequest {
    /// `-f FLABEL` verbatim, or a relabel file's first field.
    pub from_label: Option<String>,
    /// `from_label` parsed as a colon pair, when it is one.
    pub from_pair: Option<StringPair>,
    /// `-t TLABEL` verbatim, or a relabel file's second field.
    pub to_label: Option<String>,
    /// `to_label` parsed as a colon pair, when it is one.
    pub to_pair: Option<StringPair>,
    /// `-T TFILE`, kept for the progress lines: an unnamed replacement
    /// transducer is reported by the file it was read from.
    pub to_transducer_filename: Option<String>,
    /// `-9, --compose`: accumulate label-for-label substitutions into one
    /// transducer and compose them in at the end instead of rewriting arcs.
    pub compose: bool,
}

/// The per-stream substitution state: the `-T` replacement transducer, the
/// delayed `--compose` accumulator, and the relabel file's batched maps.
pub struct SubstituteEngine<B: AlgebraBackend> {
    to_transducer: Option<HfstTransducer<B>>,
    substitution_trans: Option<HfstTransducer<B>>,
    label_substitutions: HfstSymbolSubstitutions,
    pair_substitutions: HfstSymbolPairSubstitutions,
    label_batch_in_use: bool,
    pair_batch_in_use: bool,
    delayed: bool,
}

impl<B: AlgebraBackend> SubstituteEngine<B> {
    /// A new engine over an optional `-T` replacement transducer.
    pub fn new(to_transducer: Option<HfstTransducer<B>>) -> SubstituteEngine<B> {
        SubstituteEngine {
            to_transducer,
            substitution_trans: None,
            label_substitutions: HfstSymbolSubstitutions::new(),
            pair_substitutions: HfstSymbolPairSubstitutions::new(),
            label_batch_in_use: false,
            pair_batch_in_use: false,
            delayed: false,
        }
    }

    /// Whether a `--compose` substitution has been deferred and still needs
    /// [`SubstituteEngine::perform_delayed`]. Once set it stays set for the
    /// rest of the stream.
    pub fn is_delayed(&self) -> bool {
        self.delayed
    }

    /// Drop the `-T` replacement transducer at end of stream.
    pub fn release_to_transducer(&mut self) {
        self.to_transducer = None;
    }

    /// Start a fresh delayed-substitution accumulator for the next transducer
    /// in the stream.
    pub fn begin_transducer(&mut self) {
        self.substitution_trans = Some(HfstTransducer::new());
    }

    // Resolves the display name of the replacement transducer: its stored
    // name, or the '-T' filename when unnamed.
    fn to_transducer_name(&self, request: &SubstituteRequest) -> String {
        let n = self
            .to_transducer
            .as_ref()
            .expect("to_transducer present when a transducer substitution is chosen")
            .get_name();
        if n.is_empty() {
            request
                .to_transducer_filename
                .clone()
                .expect("a -T transducer was loaded, so its filename is known")
        } else {
            n
        }
    }

    /// Apply `request` to `trans`. `transducer_n` is the 1-based position of
    /// `trans` in the input stream; it only reaches the progress lines, which
    /// name the position from the second transducer on.
    ///
    /// The arms are tried in the order the tool declares them, and the first
    /// whose `from`/`to` fields are both present wins: pair for pair, label for
    /// label (deferred under `--compose`), pair for transducer, label for
    /// transducer. A request that matches none is a no-op.
    pub fn do_substitute(
        &mut self,
        request: &SubstituteRequest,
        trans: &mut HfstTransducer<B>,
        transducer_n: usize,
        reporter: &dyn SubstituteReporter,
    ) -> Result<()> {
        let has_to_transducer = self.to_transducer.is_some();
        if let (Some(fp), Some(tp)) = (&request.from_pair, &request.to_pair) {
            reporter.verbose(&format!(
                "Substituting pair {}:{} with pair {}:{}...\n",
                fp.0, fp.1, tp.0, tp.1
            ));
            trans.substitute_symbol_pair(fp, tp)?;
        } else if let (Some(fl), Some(tl)) = (&request.from_label, &request.to_label) {
            if request.compose {
                if transducer_n < 2 {
                    reporter.verbose(&format!(
                        "Delaying substitution of label {} with label {}...\n",
                        fl, tl
                    ));
                } else {
                    reporter.verbose(&format!(
                        "Delaying substitution of label {} with label {}... {}\n",
                        fl, tl, transducer_n
                    ));
                }
                let substitution: HfstTransducer<B> = HfstTransducer::new_symbol_pair(fl, tl)?;
                self.substitution_trans
                    .as_mut()
                    .expect("begin_transducer initialized the accumulator")
                    .disjunct(&substitution, true)?;
                self.delayed = true;
            } else {
                if transducer_n < 2 {
                    reporter.verbose(&format!("Substituting label {} with label {}...\n", fl, tl));
                } else {
                    reporter.verbose(&format!(
                        "Substituting label {} with label {}... {}\n",
                        fl, tl, transducer_n
                    ));
                }
                trans.substitute(fl, tl, true, true)?;
            }
        } else if let (Some(fp), true) = (&request.from_pair, has_to_transducer) {
            let to_name = self.to_transducer_name(request);
            if transducer_n < 2 {
                reporter.verbose(&format!(
                    "Substituting pair {}:{} with transducer {}...\n",
                    fp.0, fp.1, to_name
                ));
            } else {
                reporter.verbose(&format!(
                    "Substituting pair {}:{} with transducer {}... {}\n",
                    fp.0, fp.1, to_name, transducer_n
                ));
            }
            let to_t = self
                .to_transducer
                .as_mut()
                .expect("to_transducer present when has_to_transducer is true");
            trans.substitute_symbol_pair_with_transducer(fp, to_t, true)?;
        } else if let (Some(fl), true) = (&request.from_label, has_to_transducer) {
            let to_name = self.to_transducer_name(request);
            if transducer_n < 2 {
                reporter.verbose(&format!(
                    "Substituting id. label {} with transducer {}...\n",
                    fl, to_name
                ));
            } else {
                reporter.verbose(&format!(
                    "Substituting id. label {} with transducer {}... {}\n",
                    fl, to_name, transducer_n
                ));
            }
            let from_arc: StringPair = (Symbol::new(fl), Symbol::new(fl));
            let to_t = self
                .to_transducer
                .as_mut()
                .expect("to_transducer present when has_to_transducer is true");
            trans.substitute_symbol_pair_with_transducer(&from_arc, to_t, true)?;
        }
        Ok(())
    }

    // [spec:hfst:def:hfst-substitute.perform-delayed-fn]
    // [spec:hfst:sem:hfst-substitute.perform-delayed-fn]
    /// Compose the accumulated `--compose` substitution disjunction into
    /// `trans`. Only meaningful once [`SubstituteEngine::is_delayed`] holds.
    pub fn perform_delayed(
        &self,
        trans: &mut HfstTransducer<B>,
        reporter: &dyn SubstituteReporter,
    ) -> Result<()> {
        reporter.verbose("Finalising substitution transducer...\n");
        trans.substitute_by_composition(
            self.substitution_trans
                .as_ref()
                .expect("begin_transducer initialized the accumulator"),
        )?;
        Ok(())
    }

    /// Take one relabel-file entry. Unless `in_order` is set, a well-formed
    /// entry joins a batch that [`SubstituteEngine::flush_batched`] applies in
    /// one pass at end of file; `in_order` applies it immediately instead, so
    /// later entries see the effect of earlier ones.
    ///
    /// `request` must already carry the entry's parsed fields — the relabel
    /// file's own diagnostics (empty fields, missing tab) are the caller's,
    /// because only the caller knows the file and line to name.
    pub fn apply_relabel_entry(
        &mut self,
        request: &SubstituteRequest,
        trans: &mut HfstTransducer<B>,
        transducer_n: usize,
        in_order: bool,
        reporter: &dyn SubstituteReporter,
    ) -> Result<()> {
        let from_empty = request.from_label.as_ref().is_none_or(|s| s.is_empty());
        let to_empty = request.to_label.as_ref().is_none_or(|s| s.is_empty());

        if request.from_pair.is_some() && request.to_pair.is_some() {
            if !in_order {
                if let (Some(fp), Some(tp)) = (&request.from_pair, &request.to_pair) {
                    self.pair_substitutions.insert(fp.clone(), tp.clone());
                }
                self.pair_batch_in_use = true;
                return Ok(());
            }
        } else if !from_empty && !to_empty {
            if !in_order {
                if let (Some(fl), Some(tl)) = (&request.from_label, &request.to_label) {
                    self.label_substitutions
                        .insert(Symbol::new(fl), Symbol::new(tl));
                }
                self.label_batch_in_use = true;
                return Ok(());
            }
        }
        self.do_substitute(request, trans, transducer_n, reporter)
    }

    /// Apply — and disarm — whichever relabel batches
    /// [`SubstituteEngine::apply_relabel_entry`] filled. A no-op under
    /// `in_order`, where every entry was applied as it was read.
    ///
    /// The maps themselves are kept: a stream of several transducers reads its
    /// relabel file once, and every later transducer is substituted with the
    /// same accumulated replacements.
    pub fn flush_batched(&mut self, trans: &mut HfstTransducer<B>, in_order: bool) -> Result<()> {
        // perform label-to-label substitution right away
        if !in_order && self.label_batch_in_use {
            trans.substitute_substitutions(&self.label_substitutions)?;
            self.label_batch_in_use = false;
        }

        // perform symbol pair-to-symbol pair substitution right away
        if !in_order && self.pair_batch_in_use {
            trans.substitute_symbol_pairs(&self.pair_substitutions)?;
            self.pair_batch_in_use = false;
        }
        Ok(())
    }
}
