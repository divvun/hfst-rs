use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

use super::corpus::{Corpus, Fold, characters};
use super::external::Recording;
use super::report::{Report, Row, Typed};
use super::user::UnitTypist;
use super::{RunSettings, Variant};
use crate::error::Result;
use crate::prediction::model::{Model, ModelBuilder, ModelSettings, generator_accepts};
use crate::prediction::units::{AnalysisTokenizer, UnitGrammar, UnitKey};
use crate::prediction::walk::{WalkLimits, Walker};
use crate::transducer::{Transducer, TransducerTablesInterface};

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
    /// thread-safe; the folds and variants are then typed on
    /// [`RunSettings::threads`] threads.
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
        let folds = corpus.folds(self.settings.folds)?;
        let mut models: Vec<(usize, ModelSettings, Vec<u8>)> = Vec::new();
        let mut jobs: Vec<Job> = Vec::new();
        for (fold, part) in folds.iter().enumerate() {
            for (variant, settings) in variants.iter().map(|variant| variant.model).enumerate() {
                let built = models
                    .iter()
                    .position(|(built, alike, _)| *built == fold && *alike == settings);
                let model = match built {
                    Some(model) => model,
                    None => {
                        models.push((fold, settings, self.build_model(corpus, part, settings)?));
                        models.len() - 1
                    }
                };
                jobs.push(Job {
                    fold,
                    variant,
                    model,
                });
            }
        }
        let scorer = self.scorer();
        let scored = in_parallel(&jobs, self.settings.threads, |job| -> Result<Row> {
            let variant = &variants[job.variant];
            let model = Model::from_bytes(&models[job.model].2, self.grammar)?;
            let mut row = Row::new(&variant.name, true);
            scorer.score(
                corpus,
                folds[job.fold].scored.clone(),
                &model,
                variant,
                &mut row,
            )?;
            Ok(row)
        });
        let mut rows: Vec<Row> = variants
            .iter()
            .map(|variant| Row::new(&variant.name, true))
            .collect();
        for (job, row) in jobs.iter().zip(scored) {
            rows[job.variant].absorb(&row?);
        }
        rows.extend(
            recordings
                .iter()
                .map(|recording| score_recording(corpus, recording)),
        );
        Ok(Report { rows })
    }

    pub(super) fn scorer(&self) -> Scorer<'a, T> {
        Scorer {
            generator: self.generator,
            grammar: self.grammar,
            limits: self.settings.limits,
        }
    }
}

/// One fold typed with one variant, by their indices, and the index of the
/// model built for the pair.
struct Job {
    fold: usize,
    variant: usize,
    model: usize,
}

/// What typing needs of a harness, without its generator check, so that
/// threads can share it.
pub(super) struct Scorer<'a, T: TransducerTablesInterface> {
    generator: &'a Transducer<T>,
    grammar: &'a UnitGrammar,
    limits: WalkLimits,
}

impl<T: TransducerTablesInterface> Scorer<'_, T> {
    /// Type the tokens with one model, as one text. The context is the keys
    /// of each word's target, as the model builder counts them; a token with
    /// no target breaks it, as it breaks the builder's runs. Tokens with no
    /// characters, such as punctuation, are not typed.
    // [spec:hfst:def:prediction-evaluation.keystrokes]
    pub(super) fn score(
        &self,
        corpus: &Corpus<'_>,
        tokens: Range<usize>,
        model: &Model<'_>,
        variant: &Variant,
        row: &mut Row,
    ) -> Result<()> {
        let mut typist =
            UnitTypist::new(self.generator, self.grammar, self.limits, model, variant)?;
        let keep = model.order().saturating_sub(1);
        let mut context: Vec<UnitKey> = Vec::new();
        for index in tokens {
            let Some(token) = corpus.tokens().get(index) else {
                break;
            };
            let target = corpus.target(index);
            let length = characters(token.surface);
            if length > 0 {
                let typed = match target {
                    Some(target) => typist.type_word(token.surface, target, &context)?,
                    None => Typed::in_full(length),
                };
                row.add(
                    target.map_or(0, |target| target.units.len()),
                    length,
                    &typed,
                );
            }
            match target {
                Some(target) => {
                    context.extend(target.keys.iter().cloned());
                    let excess = context.len().saturating_sub(keep);
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
