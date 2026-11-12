//! Complete words offered beside the stems, and the ranking of units,
//! complete words and counted word forms together
//! (`prediction.whole-words`, `prediction-model.word-score`,
//! `prediction-model.form-score`).

use std::cmp::Ordering;
use std::convert::Infallible;
use std::rc::Rc;

use super::{Candidate, CandidateOrder, Next, SeenForm, SeenForms, Walker, Word};
use crate::prediction::rank::{Standing, rank_scored, word_scores};
use crate::prediction::units::{UnitKey, UnitKind};
use crate::transducer::{TransducerTablesInterface, Weight};

/// A complete word on offer: a stem and one of its endings, picked together.
// [spec:hfst:sem:prediction.whole-words]
#[derive(Clone, Debug)]
pub struct WholeWord {
    stem: Candidate,
    ending: Candidate,
    surface: String,
}

impl WholeWord {
    pub fn stem(&self) -> &Candidate {
        &self.stem
    }

    /// The ending, as offered after the stem.
    pub fn ending(&self) -> &Candidate {
        &self.ending
    }

    /// The word it writes, at the lowest weight of its paths: its label.
    pub fn surface(&self) -> &str {
        &self.surface
    }

    /// The lowest weight of a path that writes the word, final weight
    /// included.
    pub fn weight(&self) -> Weight {
        self.ending.weight
    }

    /// Its stem's key, then its ending's key.
    pub fn key(&self) -> String {
        format!("{}{}", self.stem.key.as_str(), self.ending.key.as_str())
    }
}

impl Word {
    /// The word with the stem and then the ending of `word`, a complete
    /// word offered for it, picked.
    // [spec:hfst:sem:prediction.whole-words]
    pub fn pick_word(&self, word: &WholeWord) -> Word {
        self.pick(&word.stem).pick(&word.ending)
    }
}

/// One thing to show: a unit on offer, a complete word, or a word form the
/// model counted.
#[derive(Clone, Debug)]
pub enum Suggestion<'n> {
    Unit(&'n Candidate),
    Word(Box<WholeWord>),
    Form(Box<SeenForm>),
}

impl Suggestion<'_> {
    /// The unit's key, the complete word's, or the counted form itself.
    pub fn key(&self) -> String {
        match self {
            Suggestion::Unit(candidate) => candidate.key.as_str().to_owned(),
            Suggestion::Word(word) => word.key(),
            Suggestion::Form(seen) => seen.form.clone(),
        }
    }
}

/// How suggestions are drawn from what is on offer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Suggest {
    /// How many of its highest-ranked endings complete each stem on offer
    /// as a complete word. 0 offers none.
    pub whole_words: usize,
    /// The most suggestions to give; all when none. With complete words,
    /// all costs a walk for every stem on offer, and with counted forms and
    /// no letters typed, all is every form the model counted.
    pub limit: Option<usize>,
    /// Offer the word forms the model counted, before the word's first
    /// unit (`prediction.seen-forms`).
    pub forms: bool,
    /// Offer units, and the complete words built from them. Without them
    /// only counted forms are offered.
    pub units: bool,
    /// What the score of a unit or a complete word built from units is
    /// multiplied by where counted forms are on offer beside it, from 0 to
    /// 1 (`prediction-model.form-score`).
    pub unit_weight: f64,
}

impl Default for Suggest {
    /// Units alone, with no complete words and no limit.
    fn default() -> Self {
        Suggest {
            whole_words: 0,
            limit: None,
            forms: false,
            units: true,
            unit_weight: 1.0,
        }
    }
}

/// What suggestions are ranked by, and after.
#[derive(Clone, Copy)]
pub struct Ranking<'r> {
    pub order: &'r dyn CandidateOrder,
    /// The keys of the earlier units, the word's own last.
    pub context: &'r [UnitKey],
    /// The forms of the earlier words, oldest first.
    pub before: &'r [String],
    /// The letters typed for the word, in the generator's spelling, which
    /// counted forms on offer start with.
    pub typed: &'r str,
}

impl<'r> Ranking<'r> {
    /// A ranking after units alone: no forms before and no letters typed.
    pub fn after_units(order: &'r dyn CandidateOrder, context: &'r [UnitKey]) -> Ranking<'r> {
        Ranking {
            order,
            context,
            before: &[],
            typed: "",
        }
    }
}

/// A suggestion with the score it ranks by.
struct Ranked<'n> {
    suggestion: Suggestion<'n>,
    score: Option<f64>,
}

impl Ranked<'_> {
    fn standing(&self) -> Standing<'_> {
        match &self.suggestion {
            Suggestion::Unit(candidate) => Standing {
                score: self.score,
                weight: candidate.weight,
                key: [candidate.key.as_str(), ""],
            },
            Suggestion::Word(word) => Standing {
                score: self.score,
                weight: word.weight(),
                key: [word.stem.key.as_str(), word.ending.key.as_str()],
            },
            Suggestion::Form(seen) => Standing {
                score: self.score,
                weight: 0.0,
                key: [seen.form.as_str(), ""],
            },
        }
    }
}

/// The suggestions found so far, with the counted forms on offer beside
/// them, if any.
struct Pool<'n, 's> {
    found: Vec<Ranked<'n>>,
    seen: Option<&'s dyn SeenForms>,
}

impl<'n> Pool<'n, '_> {
    /// Add a complete word built from units. One that is a counted form on
    /// offer is offered once, as that form, with the higher of its two
    /// scores.
    // [spec:hfst:sem:prediction-model.form-score]
    fn add_word(&mut self, word: WholeWord, score: Option<f64>) {
        let Some((form, form_score)) = self.seen.and_then(|seen| seen.find(word.surface())) else {
            self.found.push(Ranked {
                suggestion: Suggestion::Word(Box::new(word)),
                score,
            });
            return;
        };
        let known = self.found.iter_mut().find(
            |ranked| matches!(&ranked.suggestion, Suggestion::Form(seen) if seen.form == form.form),
        );
        match known {
            Some(known) => known.score = higher(known.score, score),
            None => self.found.push(Ranked {
                suggestion: Suggestion::Form(Box::new(form)),
                score: higher(form_score, score),
            }),
        }
    }
}

/// The higher of two scores; a score is higher than none.
fn higher(a: Option<f64>, b: Option<f64>) -> Option<f64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (Some(score), None) | (None, Some(score)) => Some(score),
        (None, None) => None,
    }
}

/// True when `stem`, standing as it does before its complete words are
/// found, can place neither itself nor one of them among the first `limit`
/// of `found`: when it stands after the last of those. Its words and its own
/// score beside them are at most its score, since a share is at most 1, and
/// their keys extend its key. An unseen stem's words are unseen, and weigh
/// at least what the stem does while no weight in the generator is below
/// zero. A negative weight could let an unseen word past the cut. A word
/// that is a counted form on offer stands with that form at the higher of
/// their scores, so it places only where the form or the stem could.
fn cannot_place(found: &[Ranked<'_>], limit: usize, stem: &Ranked<'_>) -> bool {
    let Some(at) = limit.checked_sub(1) else {
        return true;
    };
    if found.len() <= at {
        return false;
    }
    let mut standings: Vec<Standing<'_>> = found.iter().map(Ranked::standing).collect();
    let (_, last, _) = standings.select_nth_unstable_by(at, |a, b| a.compare(b));
    stem.standing().compare(last) == Ordering::Greater
}

impl<'a, T: TransducerTablesInterface> Walker<'a, T> {
    /// The suggestions for `word`, drawn from `offered`, what a walk found
    /// for it, and ranked by `ranking`. Each stem on offer brings the
    /// complete words of its `settings.whole_words` endings that rank
    /// highest after the context and the stem, scored as the model scores
    /// words (`prediction-model.word-score`), and is itself scored by what
    /// they leave. Every endings walk is a walk of its own, so stems are
    /// completed in their ranked order and only while one of them can still
    /// place a suggestion among the first `settings.limit`. With
    /// `settings.forms`, a word with no unit yet is also offered the forms
    /// the model counted that start with the letters typed.
    // [spec:hfst:sem:prediction.whole-words]
    // [spec:hfst:sem:prediction.seen-forms]
    pub fn suggest<'n>(
        &self,
        word: &Word,
        offered: &'n [Candidate],
        ranking: &Ranking<'_>,
        settings: Suggest,
    ) -> Vec<Suggestion<'n>> {
        let walked = self.suggest_with(word, offered, ranking, settings, |_, picked| {
            Ok::<_, Infallible>(Rc::new(self.endings(picked)))
        });
        let Ok(suggestions) = walked;
        suggestions
    }

    /// [`Walker::suggest`], with what is offered after each stem from
    /// `after_stem`, which is given the stem and the word with it picked,
    /// and must give what [`Walker::endings`] or [`Walker::next`] finds for
    /// that word. Counted forms on offer are scored by the model's form
    /// counts, and units and complete words built from them by the unit
    /// model's shares times `settings.unit_weight`; where no counted form is
    /// on offer, units are scored by the unit model alone.
    // [spec:hfst:sem:prediction.whole-words]
    // [spec:hfst:sem:prediction-model.word-score]
    // [spec:hfst:sem:prediction-model.form-score]
    // [spec:hfst:sem:prediction.seen-forms]
    pub fn suggest_with<'n, E>(
        &self,
        word: &Word,
        offered: &'n [Candidate],
        ranking: &Ranking<'_>,
        settings: Suggest,
        mut after_stem: impl FnMut(&Candidate, &Word) -> Result<Rc<Next>, E>,
    ) -> Result<Vec<Suggestion<'n>>, E> {
        let seen = if settings.forms && word.units().is_empty() {
            ranking.order.seen_forms(ranking.before, ranking.typed)
        } else {
            None
        };
        let mut pool = Pool {
            found: Vec::new(),
            seen: seen.as_deref(),
        };
        if let Some(seen) = &seen {
            pool.found.extend(
                seen.best(settings.limit)
                    .into_iter()
                    .map(|(form, score)| Ranked {
                        suggestion: Suggestion::Form(Box::new(form)),
                        score,
                    }),
            );
        }
        if settings.units {
            self.add_units(word, offered, ranking, settings, &mut pool, &mut after_stem)?;
        }
        let mut found = pool.found;
        found.sort_by(|a, b| a.standing().compare(&b.standing()));
        if let Some(limit) = settings.limit {
            found.truncate(limit);
        }
        Ok(found.into_iter().map(|ranked| ranked.suggestion).collect())
    }

    /// Rank the units on offer into `pool`, each stem with its complete
    /// words. Beside counted forms, scores are the unit model's shares
    /// times the unit weight, so they stand on the same scale as a form's.
    // [spec:hfst:sem:prediction.whole-words]
    // [spec:hfst:sem:prediction-model.form-score]
    fn add_units<'n, E>(
        &self,
        word: &Word,
        offered: &'n [Candidate],
        ranking: &Ranking<'_>,
        settings: Suggest,
        pool: &mut Pool<'n, '_>,
        after_stem: &mut impl FnMut(&Candidate, &Word) -> Result<Rc<Next>, E>,
    ) -> Result<(), E> {
        let (order, context) = (ranking.order, ranking.context);
        let pairs: Vec<(UnitKey, Weight)> = offered
            .iter()
            .map(|candidate| (candidate.key.clone(), candidate.weight))
            .collect();
        let scores = match pool.seen {
            None => order.scores(context, &pairs),
            Some(_) => order
                .shares(context, &pairs)
                .into_iter()
                .map(|share| share.map(|share| share * settings.unit_weight))
                .collect(),
        };
        let mut stems: Vec<(&'n Candidate, Option<f64>)> = Vec::new();
        for at in rank_scored(&scores, &pairs) {
            let candidate = &offered[at];
            let score = scores.get(at).copied().flatten();
            if settings.whole_words > 0 && candidate.unit.kind == UnitKind::Stem {
                stems.push((candidate, score));
            } else {
                pool.found.push(Ranked {
                    suggestion: Suggestion::Unit(candidate),
                    score,
                });
            }
        }
        for (stem, score) in stems {
            let standing = Ranked {
                suggestion: Suggestion::Unit(stem),
                score,
            };
            if settings
                .limit
                .is_some_and(|limit| cannot_place(&pool.found, limit, &standing))
            {
                break;
            }
            let after = after_stem(stem, &word.pick(stem))?;
            let (words, shares): (Vec<WholeWord>, Vec<Option<f64>>) = self
                .whole_words(stem, &after, context, order, settings.whole_words)
                .into_iter()
                .unzip();
            let (scores, left) = word_scores(score, &shares);
            pool.found.push(Ranked {
                suggestion: Suggestion::Unit(stem),
                score: left,
            });
            for (word, score) in words.into_iter().zip(scores) {
                pool.add_word(word, score);
            }
        }
        Ok(())
    }

    /// The complete words of `stem`: the endings offered `after` it that
    /// `order` ranks highest after the context and the stem, at most
    /// `count` of them, each with its share there.
    // [spec:hfst:sem:prediction.whole-words]
    fn whole_words(
        &self,
        stem: &Candidate,
        after: &Next,
        context: &[UnitKey],
        order: &dyn CandidateOrder,
        count: usize,
    ) -> Vec<(WholeWord, Option<f64>)> {
        let endings: Vec<&Candidate> = after
            .candidates
            .iter()
            .filter(|candidate| candidate.unit.kind == UnitKind::Ending)
            .collect();
        if endings.is_empty() {
            return Vec::new();
        }
        let pairs: Vec<(UnitKey, Weight)> = endings
            .iter()
            .map(|ending| (ending.key.clone(), ending.weight))
            .collect();
        let mut after_stem = context.to_vec();
        after_stem.push(stem.key.clone());
        let shares = order.shares(&after_stem, &pairs);
        order
            .order(&after_stem, &pairs)
            .into_iter()
            .filter_map(|at| {
                let word = self.whole_word_of(stem, endings.get(at)?)?;
                Some((word, shares.get(at).copied().flatten()))
            })
            .take(count)
            .collect()
    }

    fn whole_word_of(&self, stem: &Candidate, ending: &Candidate) -> Option<WholeWord> {
        let form = self.forms_of(&ending.finals).into_iter().next()?;
        Some(WholeWord {
            stem: stem.clone(),
            ending: ending.clone(),
            surface: form.surface,
        })
    }

    /// The complete word with `key`, [`WholeWord::key`]: a stem in
    /// `offered`, what a walk found for `word`, whose key starts `key`, and
    /// an ending the generator accepts after it whose key is the rest.
    /// Stems are tried in the order of their keys.
    // [spec:hfst:sem:prediction.whole-words]
    pub fn whole_word(&self, word: &Word, offered: &[Candidate], key: &str) -> Option<WholeWord> {
        let mut stems: Vec<&Candidate> = offered
            .iter()
            .filter(|candidate| candidate.unit.kind == UnitKind::Stem)
            .collect();
        stems.sort_by(|a, b| a.key.cmp(&b.key));
        stems.into_iter().find_map(|stem| {
            let rest = key.strip_prefix(stem.key.as_str())?;
            let after = self.endings(&word.pick(stem));
            let ending = after.candidates.iter().find(|candidate| {
                candidate.unit.kind == UnitKind::Ending && candidate.key.as_str() == rest
            })?;
            self.whole_word_of(stem, ending)
        })
    }
}
