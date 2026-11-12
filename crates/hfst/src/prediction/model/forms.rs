//! Word forms read in place: the forms that start with given letters, a
//! form's count and units, and each form's score after the forms of the
//! words before it (`prediction-model.word-forms`,
//! `prediction-model.form-score`).

use std::cmp::Ordering;
use std::ops::Range;

use super::format::le_u32;
use super::read::BACKOFF_FACTOR;
use super::{Backoff, Model, fold, is_character};
use crate::prediction::units::UnitKey;
use crate::prediction::walk::{SeenForm, SeenForms};

impl<'a> Model<'a> {
    /// How many times the run of forms was counted, each form's capitals
    /// folded; 0 when never or when the run is longer than the order. The
    /// empty run's count is that of all forms.
    // [spec:hfst:req:prediction-model.word-forms]
    pub fn form_count(&self, run: &[&str]) -> f64 {
        let ids: Option<Vec<u32>> = run.iter().map(|form| self.form_id(&fold(form))).collect();
        ids.map_or(0.0, |ids| self.form_run_count(&ids))
    }

    /// Every counted form that starts with `letters`, capitals folded, with
    /// its count, in the order of their bytes. The first and the last are
    /// found by binary search, and nothing outside them is read.
    // [spec:hfst:req:prediction-model.word-forms]
    pub fn forms_starting(&self, letters: &str) -> impl Iterator<Item = (&'a str, f64)> + '_ {
        let range = self.form_range(&fold(letters));
        range.filter_map(|id| {
            let id = u32::try_from(id).ok()?;
            Some((self.form_text(id)?, self.form_record_count(id)))
        })
    }

    /// The unit keys of the reading the form, capitals folded, was counted
    /// with most often; none for a form the model did not count.
    // [spec:hfst:req:prediction-model.word-forms]
    pub fn form_units(&self, form: &str) -> Option<Vec<UnitKey>> {
        self.form_id(&fold(form)).map(|id| self.units_of_form(id))
    }

    /// The scores of forms after `before`, the forms of the words before,
    /// oldest first. Only the last order - 1 of them are read.
    // [spec:hfst:sem:prediction-model.form-score]
    pub fn form_scores(&self, before: &[String]) -> FormScores<'_, 'a> {
        let keep = before.len().min(self.order() - 1);
        let context: Vec<Option<u32>> = before[before.len() - keep..]
            .iter()
            .map(|form| self.form_id(&fold(form)))
            .collect();
        let mut levels = Vec::with_capacity(context.len());
        let mut factor = 1.0;
        for drop in 0..context.len() {
            let ids: Option<Vec<u32>> = context[drop..].iter().copied().collect();
            let seen = ids.map_or_else(Vec::new, |ids| self.continuations(&ids));
            let of = seen.iter().map(|&(_, count)| count).sum();
            levels.push(Level { factor, of, seen });
            factor *= BACKOFF_FACTOR;
        }
        FormScores {
            model: self,
            levels,
            unigram_factor: factor,
        }
    }

    fn form_id(&self, form: &str) -> Option<u32> {
        let index = self.search(self.forms, |record| {
            self.string(record)
                .map_or(Ordering::Less, |found| found.cmp(form.as_bytes()))
        })?;
        u32::try_from(index).ok()
    }

    pub(super) fn form_text(&self, id: u32) -> Option<&'a str> {
        let id = id as usize;
        if id >= self.forms.records {
            return None;
        }
        let bytes = self.string(self.forms.record(self.bytes, id))?;
        std::str::from_utf8(bytes).ok()
    }

    fn form_record_count(&self, id: u32) -> f64 {
        let id = id as usize;
        if id >= self.forms.records {
            return 0.0;
        }
        f64::from(f32::from_bits(le_u32(self.forms.record(self.bytes, id), 8)))
    }

    fn units_of_form(&self, id: u32) -> Vec<UnitKey> {
        let id = id as usize;
        if id >= self.forms.records {
            return Vec::new();
        }
        let record = self.forms.record(self.bytes, id);
        let start = le_u32(record, 12) as usize;
        let end = start.saturating_add(le_u32(record, 16) as usize);
        (start..end.min(self.form_units.records))
            .filter_map(|entry| self.key(le_u32(self.form_units.record(self.bytes, entry), 0)))
            .collect()
    }

    /// The ids of the forms that start with the folded `prefix`: the first
    /// whose bytes are not less than it, up to the first after those that
    /// does not start with it.
    // [spec:hfst:req:prediction-model.word-forms]
    fn form_range(&self, prefix: &str) -> Range<usize> {
        let prefix = prefix.as_bytes();
        let text = |index: usize| {
            self.string(self.forms.record(self.bytes, index))
                .unwrap_or_default()
        };
        let start = self.partition(0, |index| text(index) < prefix);
        let end = self.partition(start, |index| text(index).starts_with(prefix));
        start..end
    }

    /// The first index from `low` of the form table at which `before` is
    /// false, where it is true for every index before that one.
    fn partition(&self, low: usize, before: impl Fn(usize) -> bool) -> usize {
        let (mut low, mut high) = (low, self.forms.records);
        while low < high {
            let middle = low + (high - low) / 2;
            if before(middle) {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        low
    }

    /// How many times the run of form ids was counted. The empty run's count
    /// is that of all forms.
    fn form_run_count(&self, ids: &[u32]) -> f64 {
        match ids {
            [] => self.forms_total,
            [form] => self.form_record_count(*form),
            [_, _, ..] => {
                let Some(&table) = self.form_runs.get(ids.len() - 2) else {
                    return 0.0;
                };
                let found = self.search(table, |record| {
                    (0..ids.len())
                        .map(|i| le_u32(record, 4 * i))
                        .cmp(ids.iter().copied())
                });
                found.map_or(0.0, |index| {
                    f64::from(f32::from_bits(le_u32(
                        table.record(self.bytes, index),
                        4 * ids.len(),
                    )))
                })
            }
        }
    }

    /// The forms seen right after the run of form ids, each with its count
    /// there, in the order of their ids. The runs that start with `ids` are
    /// consecutive in their table, so two binary searches find them.
    fn continuations(&self, ids: &[u32]) -> Vec<(u32, f64)> {
        let Some(&table) = self.form_runs.get(ids.len().saturating_sub(1)) else {
            return Vec::new();
        };
        let start_of = |index: usize| {
            let record = table.record(self.bytes, index);
            (0..ids.len()).map(move |i| le_u32(record, 4 * i))
        };
        let bound = |low: usize, before: &dyn Fn(Ordering) -> bool| {
            let (mut low, mut high) = (low, table.records);
            while low < high {
                let middle = low + (high - low) / 2;
                if before(start_of(middle).cmp(ids.iter().copied())) {
                    low = middle + 1;
                } else {
                    high = middle;
                }
            }
            low
        };
        let start = bound(0, &|order| order == Ordering::Less);
        let end = bound(start, &|order| order != Ordering::Greater);
        (start..end)
            .map(|index| {
                let record = table.record(self.bytes, index);
                let count = f64::from(f32::from_bits(le_u32(record, 4 * (ids.len() + 1))));
                (le_u32(record, 4 * ids.len()), count)
            })
            .collect()
    }
}

/// The forms seen after one end of the context.
struct Level {
    /// What stupid backoff multiplies a score at this level by.
    factor: f64,
    /// How many forms were counted right after this end of the context.
    of: f64,
    /// The forms seen right after it, by id, with their counts there.
    seen: Vec<(u32, f64)>,
}

impl Level {
    fn count(&self, id: u32) -> Option<f64> {
        self.seen
            .binary_search_by_key(&id, |&(seen, _)| seen)
            .ok()
            .map(|at| self.seen[at].1)
            .filter(|&count| count > 0.0)
    }

    fn sees_any(&self, ids: &Range<usize>) -> bool {
        let first = self
            .seen
            .partition_point(|&(id, _)| (id as usize) < ids.start);
        self.seen
            .get(first)
            .is_some_and(|&(id, count)| (id as usize) < ids.end && count > 0.0)
    }
}

/// What the model knows of the forms after one context: for each end of the
/// context, longest first, the forms seen right after it.
pub struct FormScores<'m, 'a> {
    model: &'m Model<'a>,
    levels: Vec<Level>,
    /// What stupid backoff multiplies a form's share of all forms by.
    unigram_factor: f64,
}

impl<'m, 'a> FormScores<'m, 'a> {
    /// The forms on offer for letters typed: those that start with them,
    /// capitals folded, and that hold a character other than spaces and
    /// punctuation.
    // [spec:hfst:sem:prediction.seen-forms]
    pub fn offered(&self, typed: &str) -> Offered {
        let range = self.model.form_range(&fold(typed));
        let level = match self.model.backoff() {
            Backoff::StupidBackoff => None,
            Backoff::DropOldest => Some(
                self.levels
                    .iter()
                    .position(|level| level.of > 0.0 && level.sees_any(&range))
                    .unwrap_or(self.levels.len()),
            ),
        };
        Offered {
            range,
            level,
            words_only: !typed.chars().any(is_character),
        }
    }

    /// True for a form on offer: one that holds a character other than
    /// spaces and punctuation, which every form does that starts with
    /// letters typed that hold one.
    fn is_offered(&self, offered: &Offered, id: u32) -> bool {
        !offered.words_only
            || self
                .text(id)
                .is_some_and(|form| form.chars().any(is_character))
    }

    /// A form's score after the context, as a share of the forms counted
    /// after the end of the context it is scored after. With stupid backoff
    /// that is the longest end after which it was seen, times 0.4 for each
    /// form dropped, down to its share of all forms. With drop-oldest
    /// backoff it is the one end chosen for the forms on offer, and a form
    /// not seen there is unscored.
    // [spec:hfst:sem:prediction-model.form-score]
    pub fn score(&self, offered: &Offered, id: u32) -> Option<f64> {
        let unigram = |factor: f64| {
            let total = self.model.forms_total;
            let count = self.model.form_record_count(id);
            (total > 0.0 && count > 0.0).then(|| factor * count / total)
        };
        match offered.level {
            None => self
                .levels
                .iter()
                .find_map(|level| {
                    let count = level.count(id)?;
                    (level.of > 0.0).then(|| level.factor * count / level.of)
                })
                .or_else(|| unigram(self.unigram_factor)),
            Some(at) => match self.levels.get(at) {
                Some(level) => level.count(id).map(|count| count / level.of),
                None => unigram(1.0),
            },
        }
    }

    /// The forms on offer, best first, the first `limit` of them when there
    /// is a limit, each with its score. Scored forms come first, highest
    /// first, then unscored ones; ties go by the bytes of the form.
    // [spec:hfst:sem:prediction-model.form-score]
    // [spec:hfst:req:prediction-model.rank-only]
    pub fn best(&self, offered: &Offered, limit: Option<usize>) -> Vec<(u32, Option<f64>)> {
        let mut found: Vec<(u32, Option<f64>)> = offered
            .range
            .clone()
            .filter_map(|id| u32::try_from(id).ok())
            .filter(|&id| self.is_offered(offered, id))
            .map(|id| (id, self.score(offered, id)))
            .collect();
        let compare = |a: &(u32, Option<f64>), b: &(u32, Option<f64>)| match (a.1, b.1) {
            (Some(x), Some(y)) => y.total_cmp(&x).then(a.0.cmp(&b.0)),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => a.0.cmp(&b.0),
        };
        if let Some(limit) = limit
            && limit < found.len()
        {
            let Some(last) = limit.checked_sub(1) else {
                return Vec::new();
            };
            found.select_nth_unstable_by(last, compare);
            found.truncate(limit);
        }
        found.sort_by(compare);
        found
    }

    /// The text of a form, capitals folded.
    pub fn text(&self, id: u32) -> Option<&'a str> {
        self.model.form_text(id)
    }

    /// The form on offer whose text is `form` with capitals folded.
    pub fn find(&self, offered: &Offered, form: &str) -> Option<u32> {
        let id = self.model.form_id(&fold(form))?;
        (offered.range.contains(&(id as usize)) && self.is_offered(offered, id)).then_some(id)
    }

    fn seen(&self, id: u32) -> Option<SeenForm> {
        Some(SeenForm {
            form: self.text(id)?.to_owned(),
            units: self.model.units_of_form(id),
        })
    }

    /// The forms on offer for `typed`, as a ranking of candidates reads
    /// them.
    pub fn into_seen(self, typed: &str) -> impl SeenForms + 'm {
        let offered = self.offered(typed);
        OwnedOffer {
            scores: self,
            offered,
        }
    }
}

/// Which forms are on offer, and how they are scored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offered {
    /// The ids of the forms that start with the letters typed.
    range: Range<usize>,
    /// With drop-oldest backoff, the one end of the context the forms are
    /// scored after: the longest after which one of them was seen. The
    /// number of levels stands for no context.
    level: Option<usize>,
    /// The letters typed hold no character other than spaces and
    /// punctuation, so the forms that start with them must be checked for
    /// one.
    words_only: bool,
}

struct OwnedOffer<'m, 'a> {
    scores: FormScores<'m, 'a>,
    offered: Offered,
}

impl SeenForms for OwnedOffer<'_, '_> {
    fn best(&self, limit: Option<usize>) -> Vec<(SeenForm, Option<f64>)> {
        self.scores
            .best(&self.offered, limit)
            .into_iter()
            .filter_map(|(id, score)| Some((self.scores.seen(id)?, score)))
            .collect()
    }

    fn find(&self, surface: &str) -> Option<(SeenForm, Option<f64>)> {
        let id = self.scores.find(&self.offered, surface)?;
        Some((self.scores.seen(id)?, self.scores.score(&self.offered, id)))
    }
}
