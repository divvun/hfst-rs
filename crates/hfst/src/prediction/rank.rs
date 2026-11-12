//! Putting the units the walk offers in order with a model
//! (`prediction-model.rank-only`), and scoring the complete words offered
//! beside stems (`prediction-model.word-score`).

use std::cmp::Ordering;

use super::model::{Backoff, Model};
use super::units::UnitKey;
use super::walk::{CandidateOrder, SeenForms};
use crate::transducer::Weight;

/// Where a suggestion stands in a ranking. A complete word's key is its
/// stem's key then its ending's key; a unit's second part is empty.
#[derive(Clone, Copy, Debug)]
pub(in crate::prediction) struct Standing<'k> {
    pub(in crate::prediction) score: Option<f64>,
    pub(in crate::prediction) weight: Weight,
    pub(in crate::prediction) key: [&'k str; 2],
}

impl Standing<'_> {
    /// Scored first, highest first; then unseen, by the generator's weight,
    /// lowest first; then by the bytes of the key.
    // [spec:hfst:req:prediction-model.rank-only]
    pub(in crate::prediction) fn compare(&self, other: &Standing<'_>) -> Ordering {
        ranked(self.score, other.score, self.weight, other.weight).then_with(|| {
            let [a, b] = self.key;
            let [c, d] = other.key;
            a.bytes().chain(b.bytes()).cmp(c.bytes().chain(d.bytes()))
        })
    }
}

/// `candidates` in ranked order by their `scores`, as indices: every index
/// once, none left out. Candidates alike in score, weight and key keep their
/// order.
// [spec:hfst:req:prediction-model.rank-only]
pub fn rank_scored(scores: &[Option<f64>], candidates: &[(UnitKey, Weight)]) -> Vec<usize> {
    let standing = |at: usize| Standing {
        score: scores.get(at).copied().flatten(),
        weight: candidates[at].1,
        key: [candidates[at].0.as_str(), ""],
    };
    let mut order: Vec<usize> = (0..candidates.len()).collect();
    order.sort_by(|&a, &b| standing(a).compare(&standing(b)));
    order
}

/// The scores of a stem's complete words on offer, given the stem's score
/// and the shares of their endings after the context and the stem, and the
/// stem's own score beside them. A word scores its stem's score times its
/// ending's share, and is unseen when either is unscored. The stem keeps
/// its score less theirs, never below zero; an unseen stem stays unseen.
// [spec:hfst:sem:prediction-model.word-score]
pub fn word_scores(stem: Option<f64>, shares: &[Option<f64>]) -> (Vec<Option<f64>>, Option<f64>) {
    let words: Vec<Option<f64>> = shares.iter().map(|&share| Some(stem? * share?)).collect();
    let left = stem.map(|own| {
        words
            .iter()
            .flatten()
            .fold(own, |left, word| left - word)
            .max(0.0)
    });
    (words, left)
}

impl Model<'_> {
    /// The candidates in ranked order, as indices into `candidates`: every
    /// index once, none left out. Candidates the model scores after the
    /// context come first, highest first; the rest follow by the
    /// generator's weight, lowest first. Ties go by the bytes of the key;
    /// candidates alike in all of these keep their order.
    // [spec:hfst:req:prediction-model.rank-only]
    pub fn rank(&self, context: &[UnitKey], candidates: &[(UnitKey, Weight)]) -> Vec<usize> {
        rank_scored(&self.scores(context, candidates), candidates)
    }

    /// Each candidate's score after the context, as [`Model::rank`] reads
    /// it: with drop-oldest backoff its count after the one context the
    /// candidates share, with stupid backoff [`Model::score`]. None for a
    /// candidate the model does not score.
    // [spec:hfst:sem:prediction-model.order]
    pub fn scores(
        &self,
        context: &[UnitKey],
        candidates: &[(UnitKey, Weight)],
    ) -> Vec<Option<f64>> {
        match self.backoff() {
            Backoff::DropOldest => self.drop_oldest(context, candidates).1,
            Backoff::StupidBackoff => candidates
                .iter()
                .map(|(key, _)| self.score(context, key))
                .collect(),
        }
    }

    /// Each candidate's share of what follows the context, at most 1, in the
    /// order of the scores: a stupid-backoff score, which is a ratio of
    /// counts already, or a drop-oldest count over the count of the context
    /// it was counted after. A complete word scores its stem's score times
    /// its ending's share.
    // [spec:hfst:sem:prediction-model.word-score]
    pub fn shares(
        &self,
        context: &[UnitKey],
        candidates: &[(UnitKey, Weight)],
    ) -> Vec<Option<f64>> {
        let shares = match self.backoff() {
            Backoff::DropOldest => {
                let (after, counts) = self.drop_oldest(context, candidates);
                let of = self.context_count(&after);
                counts
                    .into_iter()
                    .map(|count| count.map(|count| count / of))
                    .collect()
            }
            Backoff::StupidBackoff => self.scores(context, candidates),
        };
        // A ratio of counts is at most 1 in any model the builder writes. A
        // damaged file is held to that too, since the cut that keeps the
        // ranking of whole words lazy rests on it.
        shares
            .into_iter()
            .map(|share| share.map(|share| share.min(1.0)))
            .collect()
    }

    /// The longest end of the context after which some candidate was seen,
    /// and the counts after it. A candidate not seen after that context is
    /// unscored, however often it was seen after a shorter one.
    // [spec:hfst:sem:prediction-model.order]
    fn drop_oldest(
        &self,
        context: &[UnitKey],
        candidates: &[(UnitKey, Weight)],
    ) -> (Vec<Option<u32>>, Vec<Option<f64>>) {
        let ids: Vec<Option<u32>> = candidates
            .iter()
            .map(|(key, _)| self.unit_id(key))
            .collect();
        let context = self.context(context);
        for drop in 0..=context.len() {
            let counts: Vec<Option<f64>> = ids
                .iter()
                .map(|id| {
                    id.map(|id| self.count_after(&context[drop..], id))
                        .filter(|&count| count > 0.0)
                })
                .collect();
            if counts.iter().any(Option::is_some) {
                return (context[drop..].to_vec(), counts);
            }
        }
        (Vec::new(), vec![None; candidates.len()])
    }
}

impl CandidateOrder for Model<'_> {
    fn scores(&self, context: &[UnitKey], candidates: &[(UnitKey, Weight)]) -> Vec<Option<f64>> {
        Model::scores(self, context, candidates)
    }

    fn shares(&self, context: &[UnitKey], candidates: &[(UnitKey, Weight)]) -> Vec<Option<f64>> {
        Model::shares(self, context, candidates)
    }

    // [spec:hfst:sem:prediction-model.form-score]
    fn seen_forms<'s>(&'s self, before: &[String], typed: &str) -> Option<Box<dyn SeenForms + 's>> {
        Some(Box::new(self.form_scores(before).into_seen(typed)))
    }
}

fn ranked(a: Option<f64>, b: Option<f64>, a_weight: Weight, b_weight: Weight) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => b.total_cmp(&a),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a_weight.total_cmp(&b_weight),
    }
}

#[cfg(test)]
mod tests {
    use super::super::model::fixture::{STORY, build, grammar, keys, settings};
    use super::*;

    fn ranked_keys(
        backoff: Backoff,
        context: &[&str],
        candidates: &[(&str, Weight)],
    ) -> Vec<String> {
        let (bytes, _) = build(settings(3, backoff), STORY);
        let model = Model::from_bytes(&bytes, &grammar()).unwrap();
        let candidates: Vec<(UnitKey, Weight)> = candidates
            .iter()
            .map(|&(key, weight)| (UnitKey::from(key), weight))
            .collect();
        let order = model.rank(&keys(context), &candidates);
        let mut seen = order.clone();
        seen.sort_unstable();
        assert_eq!(seen, (0..candidates.len()).collect::<Vec<_>>());
        order
            .into_iter()
            .map(|index| candidates[index].0.as_str().to_owned())
            .collect()
    }

    // [spec:hfst:req:prediction-model.rank-only/test]
    #[test]
    fn scored_first_then_unseen_by_weight() {
        let order = ranked_keys(
            Backoff::StupidBackoff,
            &["+V+Ind+3Sg", "nuna"],
            &[
                ("oqar", 1.0),
                ("+N+Rel+Sg", 5.0),
                ("kina", 0.5),
                ("+N+Abs+Sg", 2.0),
                ("sini", 0.0),
                ("ataa", 0.5),
                ("zzz", 0.1),
            ],
        );
        assert_eq!(
            order,
            [
                "+N+Abs+Sg",
                "+N+Rel+Sg",
                "oqar",
                "sini",
                "zzz",
                "ataa",
                "kina"
            ]
        );
    }

    // [spec:hfst:req:prediction-model.rank-only/test]
    // [spec:hfst:sem:prediction-model.order/test]
    #[test]
    fn drop_oldest_ranks_after_one_context() {
        let order = ranked_keys(
            Backoff::DropOldest,
            &["+V+Ind+3Sg", "nuna"],
            &[
                ("oqar", 0.0),
                ("+N+Rel+Sg", 3.0),
                ("+N+Abs+Sg", 2.0),
                ("sini", 1.0),
            ],
        );
        assert_eq!(order, ["+N+Abs+Sg", "+N+Rel+Sg", "oqar", "sini"]);
        let order = ranked_keys(
            Backoff::DropOldest,
            &["+V+Ind+3Sg", "nuna"],
            &[("sini", 0.0), ("nuna", 1.0), ("oqar", 2.0)],
        );
        assert_eq!(order, ["nuna", "oqar", "sini"]);
    }

    // [spec:hfst:req:prediction-model.rank-only/test]
    #[test]
    fn nothing_is_dropped_or_added() {
        assert!(ranked_keys(Backoff::StupidBackoff, &["nuna"], &[]).is_empty());
        let order = ranked_keys(
            Backoff::DropOldest,
            &["zzz"],
            &[("yyy", 1.0), ("xxx", 1.0), ("yyy", 0.5), ("oqar", 9.0)],
        );
        assert_eq!(order, ["oqar", "yyy", "xxx", "yyy"]);
    }

    fn scored(backoff: Backoff, context: &[&str], candidates: &[&str]) -> [Vec<Option<f64>>; 2] {
        let (bytes, _) = build(settings(3, backoff), STORY);
        let model = Model::from_bytes(&bytes, &grammar()).unwrap();
        let candidates: Vec<(UnitKey, Weight)> = candidates
            .iter()
            .map(|&key| (UnitKey::from(key), 0.0))
            .collect();
        let context = keys(context);
        [
            model.scores(&context, &candidates),
            model.shares(&context, &candidates),
        ]
    }

    // [spec:hfst:sem:prediction-model.word-score/test]
    #[test]
    fn drop_oldest_shares_divide_by_the_context_count() {
        // sini never comes right before nuna, so the counts are those after
        // nuna, which was seen three times.
        let [scores, shares] = scored(
            Backoff::DropOldest,
            &["sini", "nuna"],
            &["+N+Abs+Sg", "+N+Rel+Sg", "oqar"],
        );
        assert_eq!(scores, [Some(2.0), Some(1.0), None]);
        assert_eq!(shares, [Some(2.0 / 3.0), Some(1.0 / 3.0), None]);
        let [scores, shares] = scored(Backoff::DropOldest, &["zzz"], &["nuna", "yyy"]);
        assert_eq!(scores, [Some(3.0), None]);
        assert_eq!(shares, [Some(0.3), None]);
    }

    // [spec:hfst:sem:prediction-model.word-score/test]
    #[test]
    fn stupid_backoff_shares_are_its_scores() {
        let [scores, shares] = scored(
            Backoff::StupidBackoff,
            &["+V+Ind+3Sg", "nuna"],
            &["+N+Abs+Sg", "oqar", "zzz"],
        );
        assert_eq!(scores, shares);
        assert_eq!(scores[0], Some(0.5));
        assert!(shares.iter().flatten().all(|&share| share <= 1.0));
    }

    // [spec:hfst:sem:prediction-model.word-score/test]
    #[test]
    fn a_stem_keeps_what_its_words_leave() {
        assert_eq!(
            word_scores(Some(0.5), &[Some(0.25), Some(0.5)]),
            (vec![Some(0.125), Some(0.25)], Some(0.125))
        );
        assert_eq!(
            word_scores(Some(0.5), &[Some(0.75), Some(0.5)]),
            (vec![Some(0.375), Some(0.25)], Some(0.0))
        );
        assert_eq!(word_scores(Some(0.5), &[None]), (vec![None], Some(0.5)));
        assert_eq!(word_scores(None, &[Some(0.25)]), (vec![None], None));
        assert_eq!(word_scores(Some(0.5), &[]), (vec![], Some(0.5)));
    }

    // [spec:hfst:req:prediction-model.rank-only/test]
    #[test]
    fn a_word_stands_after_its_stem() {
        let stem = Standing {
            score: Some(0.25),
            weight: 1.0,
            key: ["nuna", ""],
        };
        let word = Standing {
            key: ["nuna", "+N+Abs+Sg"],
            ..stem
        };
        let lighter = Standing {
            score: None,
            weight: 0.0,
            key: ["nunap", ""],
        };
        assert_eq!(stem.compare(&word), Ordering::Less);
        assert_eq!(word.compare(&lighter), Ordering::Less);
        let unseen = Standing {
            score: None,
            ..word
        };
        assert_eq!(lighter.compare(&unseen), Ordering::Less);
    }
}
