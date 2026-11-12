use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

use super::corpus::{Corpus, Fold, characters};
use super::external::Recording;
use super::report::{Report, Row, Typed};
use super::user::UnitTypist;
use super::walks::Walks;
use super::{RunSettings, Variant};
use crate::error::Result;
use crate::prediction::model::{Model, ModelBuilder, ModelSettings, generator_accepts};
use crate::prediction::units::{AnalysisTokenizer, UnitGrammar, UnitKey};
use crate::prediction::walk::{WalkLimits, Walker};
use crate::transducer::{Transducer, TransducerTablesInterface};

/// How many tokens one job types at most. A fold is typed in parts, so that
/// the threads stay busy to the end of a run although a word may cost
/// anything from nothing to minutes.
const PART: usize = 16;

/// Scores variants of unit prediction, and recordings from other systems,
/// on one language's analysed text.
pub struct Harness<'a, T: TransducerTablesInterface> {
    generator: &'a Transducer<T>,
    grammar: &'a UnitGrammar,
    settings: RunSettings,
    tokenizer: AnalysisTokenizer,
    /// One check for every build and every target, so each reading is
    /// looked up once.
    accepts: Box<dyn Fn(&str) -> bool + 'a>,
}

impl<'a, T: TransducerTablesInterface> Harness<'a, T> {
    /// The grammar is checked against the generator here.
    pub fn new(
        generator: &'a Transducer<T>,
        grammar: &'a UnitGrammar,
        settings: RunSettings,
    ) -> Result<Harness<'a, T>> {
        Walker::new(generator, grammar, settings.limits)?;
        Ok(Harness {
            generator,
            grammar,
            settings,
            tokenizer: AnalysisTokenizer::new(generator.get_alphabet().get_symbol_table()),
            accepts: generator_accepts(generator),
        })
    }

    /// Read hfst-lookup output, with each token's target.
    pub fn corpus<'t>(&self, text: &'t str) -> Result<Corpus<'t>> {
        Corpus::read(text, self.grammar, &self.tokenizer, &*self.accepts)
    }

    /// The model a fold scores with, built from the parts the fold leaves
    /// out of its score and from nothing else.
    // [spec:hfst:req:prediction-evaluation.held-out]
    pub fn build_model(
        &self,
        corpus: &Corpus<'_>,
        fold: &Fold,
        settings: ModelSettings,
    ) -> Result<Vec<u8>> {
        let accepts = &self.accepts;
        let mut builder = ModelBuilder::new(
            self.grammar,
            &self.tokenizer,
            Box::new(move |analysis: &str| accepts(analysis)),
            settings,
        )?;
        for tokens in &fold.built_from {
            builder.add_text(corpus.text_of(tokens.clone()))?;
        }
        builder.write()
    }

    /// Score every variant on every fold, and every recording on the whole
    /// text, which is the text the folds score between them. Variants with
    /// equal model settings share each fold's model. The models are built
    /// first, one at a time, since the generator check they share is not
    /// thread-safe; the folds are then typed in parts on
    /// [`RunSettings::threads`] threads, each part with every variant.
    // [spec:hfst:req:prediction-evaluation.held-out]
    // [spec:hfst:req:prediction-evaluation.report]
    pub fn run(
        &self,
        corpus: &Corpus<'_>,
        variants: &[Variant],
        recordings: &[Recording],
    ) -> Result<Report>
    where
        T: Sync,
    {
        self.run_in_parts(corpus, variants, recordings, PART)
    }

    /// [`Harness::run`], with each fold typed in parts of at most `part_len`
    /// tokens, which score as the whole fold does.
    pub(super) fn run_in_parts(
        &self,
        corpus: &Corpus<'_>,
        variants: &[Variant],
        recordings: &[Recording],
        part_len: usize,
    ) -> Result<Report>
    where
        T: Sync,
    {
        let folds = corpus.folds(self.settings.folds)?;
        let models = self.build_models(corpus, &folds, variants)?;
        let mut jobs: Vec<Job> = Vec::new();
        for (fold, split) in folds.iter().enumerate() {
            let scored = split.scored.clone();
            for start in scored.clone().step_by(part_len.max(1)) {
                jobs.push(Job {
                    fold,
                    part: start..start.saturating_add(part_len).min(scored.end),
                });
            }
        }
        let scorer = self.scorer();
        let scored = in_parallel(&jobs, self.settings.threads, |job| -> Result<Vec<Row>> {
            let ranking = models.ranked_by[job.fold]
                .iter()
                .map(|&model| Model::from_bytes(&models.built[model], self.grammar))
                .collect::<Result<Vec<Model<'_>>>>()?;
            let typing: Vec<(&Model<'_>, &Variant)> = ranking.iter().zip(variants).collect();
            let mut rows: Vec<Row> = variants
                .iter()
                .map(|variant| Row::new(&variant.name, true))
                .collect();
            scorer.score(
                corpus,
                folds[job.fold].scored.clone(),
                job.part.clone(),
                &typing,
                &mut rows,
            )?;
            Ok(rows)
        });
        let mut rows: Vec<Row> = variants
            .iter()
            .map(|variant| Row::new(&variant.name, true))
            .collect();
        for part in scored {
            for (row, typed) in rows.iter_mut().zip(part?) {
                row.absorb(&typed);
            }
        }
        rows.extend(
            recordings
                .iter()
                .map(|recording| score_recording(corpus, recording)),
        );
        Ok(Report { rows })
    }

    /// The models the folds' variants rank with, built one at a time.
    /// Variants with equal model settings share each fold's model.
    fn build_models(
        &self,
        corpus: &Corpus<'_>,
        folds: &[Fold],
        variants: &[Variant],
    ) -> Result<Models> {
        let mut known: Vec<(usize, ModelSettings)> = Vec::new();
        let mut built: Vec<Vec<u8>> = Vec::new();
        let mut ranked_by = Vec::new();
        for (fold, split) in folds.iter().enumerate() {
            let mut by_variant = Vec::new();
            for settings in variants.iter().map(|variant| variant.model) {
                let model = match known.iter().position(|&model| model == (fold, settings)) {
                    Some(model) => model,
                    None => {
                        built.push(self.build_model(corpus, split, settings)?);
                        known.push((fold, settings));
                        built.len() - 1
                    }
                };
                by_variant.push(model);
            }
            ranked_by.push(by_variant);
        }
        Ok(Models { built, ranked_by })
    }

    pub(super) fn scorer(&self) -> Scorer<'a, T> {
        Scorer {
            generator: self.generator,
            grammar: self.grammar,
            limits: self.settings.limits,
        }
    }
}

struct Models {
    built: Vec<Vec<u8>>,
    /// For each fold, the index in `built` of each variant's model.
    ranked_by: Vec<Vec<usize>>,
}

/// Part of one fold, typed with every variant.
struct Job {
    fold: usize,
    part: Range<usize>,
}

/// What typing needs of a harness, without its generator check, so that
/// threads can share it.
pub(super) struct Scorer<'a, T: TransducerTablesInterface> {
    generator: &'a Transducer<T>,
    grammar: &'a UnitGrammar,
    limits: WalkLimits,
}

impl<T: TransducerTablesInterface> Scorer<'_, T> {
    /// Type the tokens of `part`, which lies in `scored`, as part of
    /// `scored` typed as one text, once with each variant of `typing` and
    /// the model it ranks with, adding each variant's words to its row of
    /// `rows`. The context is the keys of each word's target from the start
    /// of `scored`, as the model builder counts them; a token with no target
    /// breaks it, as it breaks the builder's runs. Tokens with no
    /// characters, such as punctuation, are not typed. Each word is typed
    /// from an empty word, so `scored` typed in parts scores as it does
    /// typed whole, and the variants share the word's walks, since a walk
    /// depends only on where in the word it is made.
    // [spec:hfst:def:prediction-evaluation.keystrokes]
    pub(super) fn score(
        &self,
        corpus: &Corpus<'_>,
        scored: Range<usize>,
        part: Range<usize>,
        typing: &[(&Model<'_>, &Variant)],
        rows: &mut [Row],
    ) -> Result<()> {
        let mut typists = typing
            .iter()
            .map(|&(model, variant)| {
                UnitTypist::new(self.generator, self.grammar, self.limits, model, variant)
            })
            .collect::<Result<Vec<_>>>()?;
        let mut walks = Walks::new(Walker::new(self.generator, self.grammar, self.limits)?);
        let kept = |model: &Model<'_>| model.order().saturating_sub(1);
        let keep = typing.iter().map(|&(model, _)| kept(model)).max();
        let mut context: Vec<UnitKey> = Vec::new();
        for index in scored.start..part.end {
            let Some(token) = corpus.tokens().get(index) else {
                break;
            };
            let target = corpus.target(index);
            let length = characters(token.surface);
            if length > 0 && index >= part.start {
                let units = target.map_or(0, |target| target.units.len());
                for ((typist, &(model, _)), row) in typists.iter_mut().zip(typing).zip(&mut *rows) {
                    let before = &context[context.len().saturating_sub(kept(model))..];
                    let typed = match target {
                        Some(target) => {
                            typist.type_word(token.surface, target, before, &mut walks)?
                        }
                        None => Typed::in_full(length),
                    };
                    row.add(units, length, &typed);
                }
                walks.clear();
            }
            match target {
                Some(target) => {
                    context.extend(target.keys.iter().cloned());
                    let excess = context.len().saturating_sub(keep.unwrap_or(0));
                    context.drain(..excess);
                }
                None => context.clear(),
            }
        }
        Ok(())
    }
}

/// `work` applied to every job on up to `threads` threads, the answers in
/// the jobs' order. A panic on a thread is raised again here.
fn in_parallel<J: Sync, A: Send>(
    jobs: &[J],
    threads: usize,
    work: impl Fn(&J) -> A + Sync,
) -> Vec<A> {
    let next = AtomicUsize::new(0);
    let worker = || {
        let mut done = Vec::new();
        loop {
            let at = next.fetch_add(1, AtomicOrdering::Relaxed);
            let Some(job) = jobs.get(at) else {
                break done;
            };
            done.push((at, work(job)));
        }
    };
    let workers = threads.clamp(1, jobs.len().max(1));
    let mut answers: Vec<(usize, A)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers).map(|_| scope.spawn(worker)).collect();
        handles
            .into_iter()
            .flat_map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    });
    answers.sort_by_key(|(at, _)| *at);
    answers.into_iter().map(|(_, answer)| answer).collect()
}

/// A recording scored on every word of the text.
// [spec:hfst:req:prediction-evaluation.external]
fn score_recording(corpus: &Corpus<'_>, recording: &Recording) -> Row {
    let mut row = Row::new(&recording.name, false);
    for (index, token) in corpus.tokens().iter().enumerate() {
        let length = characters(token.surface);
        if length > 0 {
            let units = corpus.target(index).map_or(0, |target| target.units.len());
            row.add(units, length, &recording.type_word(token.surface));
        }
    }
    row
}
