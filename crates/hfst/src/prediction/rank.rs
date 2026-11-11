//! Putting the units the walk offers in order with a model
//! (`prediction-model.rank-only`).

use std::cmp::Ordering;

use super::model::{Backoff, Model};
use super::units::UnitKey;
use crate::transducer::Weight;

impl Model<'_> {
    /// The candidates in ranked order, as indices into `candidates`: every
    /// index once, none left out. Candidates the model scores after the
    /// context come first, highest first; the rest follow by the
    /// generator's weight, lowest first. Ties go by the bytes of the key;
    /// candidates alike in all of these keep their order.
    // [spec:hfst:req:prediction-model.rank-only]
    pub fn rank(&self, context: &[UnitKey], candidates: &[(UnitKey, Weight)]) -> Vec<usize> {
        let scores = match self.backoff() {
            Backoff::DropOldest => self.drop_oldest_scores(context, candidates),
            Backoff::StupidBackoff => candidates
                .iter()
                .map(|(key, _)| self.score(context, key))
                .collect(),
        };
        let mut order: Vec<usize> = (0..candidates.len()).collect();
        order.sort_by(|&a, &b| {
            ranked(scores[a], scores[b], candidates[a].1, candidates[b].1).then_with(|| {
                let (a, b) = (&candidates[a].0, &candidates[b].0);
                a.as_str().as_bytes().cmp(b.as_str().as_bytes())
            })
        });
        order
    }

    /// The counts after the longest end of the context after which some
    /// candidate was seen. A candidate not seen after that context is
    /// unscored, however often it was seen after a shorter one.
    // [spec:hfst:sem:prediction-model.order]
    fn drop_oldest_scores(
        &self,
        context: &[UnitKey],
        candidates: &[(UnitKey, Weight)],
    ) -> Vec<Option<f64>> {
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
                return counts;
            }
        }
        vec![None; candidates.len()]
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
}
