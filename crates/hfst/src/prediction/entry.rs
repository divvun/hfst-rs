//! Entry: letters typed where a stem can start, matched by prefix against
//! the labels of the units the generator accepts there
//! (`prediction.entry`).

use std::collections::{BTreeSet, HashMap, HashSet};

use super::units::{Label, Unit};
use super::walk::{Live, Machine, Next, Offers, Pass, Place, WalkLimits, Walker, Word};
use crate::error::Result;
use crate::transducer::{SymbolNumber, Transducer, TransducerTablesInterface};

/// What typed letters may stand for in the generator's spelling, as a
/// relaxation transducer says.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Spellings {
    pub spellings: Vec<String>,
    /// A bound stopped the walk, so some spellings may be missing.
    pub truncated: bool,
}

/// A transducer from typed letters to the letters they may stand for, so
/// that letters typed without their diacritics still match.
pub trait Relaxation {
    /// The spellings the transducer maps `typed` to. A spelling so far that
    /// `keep` refuses is dropped at once, so the walk follows only what the
    /// caller can use.
    fn spellings(
        &self,
        typed: &str,
        limits: &WalkLimits,
        keep: &mut dyn FnMut(&str) -> bool,
    ) -> Spellings;
}

fn kept(
    verdicts: &mut HashMap<String, bool>,
    keep: &mut dyn FnMut(&str) -> bool,
    spelling: String,
) -> bool {
    *verdicts
        .entry(spelling)
        .or_insert_with_key(|spelling| keep(spelling))
}

/// The relaxation is walked on its input side over the typed letters, one at
/// a time, and its output is what the letters stand for. A path must end in
/// a final state.
// [spec:hfst:sem:prediction.entry]
impl<R: TransducerTablesInterface> Relaxation for Transducer<R> {
    fn spellings(
        &self,
        typed: &str,
        limits: &WalkLimits,
        keep: &mut dyn FnMut(&str) -> bool,
    ) -> Spellings {
        let machine = Machine::new(self);
        let mut pass = Pass::new(&machine, limits.max_configurations);
        let mut verdicts: HashMap<String, bool> = HashMap::new();
        let mut lives = vec![pass.start()];
        for letter in typed.chars() {
            let Some(symbol) = machine.input_symbol(letter.encode_utf8(&mut [0; 4])) else {
                return Spellings::default();
            };
            let closure = pass.closure(&lives);
            lives = closure
                .into_iter()
                .filter(|live| kept(&mut verdicts, keep, pass.render(live)))
                .collect();
            lives = pass.step(&lives, symbol);
        }
        let spellings: BTreeSet<String> = pass
            .finals(&lives)
            .iter()
            .map(|live| pass.render(live))
            .filter(|spelling| kept(&mut verdicts, keep, spelling.clone()))
            .collect();
        Spellings {
            spellings: spellings.into_iter().collect(),
            truncated: pass.budget.truncated(),
        }
    }
}

/// The configurations each spelling reaches in the generator, read letter
/// by letter from where the stem starts.
struct Spelled {
    reached: HashMap<String, Vec<Live>>,
    refused: HashSet<String>,
}

impl Spelled {
    fn new(start: Vec<Live>) -> Spelled {
        Spelled {
            reached: HashMap::from([(String::new(), start)]),
            refused: HashSet::new(),
        }
    }

    /// True when the generator reads `spelling` as letters.
    fn reach<'a, T: TransducerTablesInterface>(
        &mut self,
        walker: &Walker<'a, T>,
        spelling: &str,
        pass: &mut Pass<'_, 'a, T>,
    ) -> bool {
        if self.reached.contains_key(spelling) {
            return true;
        }
        if self.refused.contains(spelling) {
            return false;
        }
        let Some((cut, letter)) = spelling.char_indices().last() else {
            return false;
        };
        let shorter = &spelling[..cut];
        let mut reached = Vec::new();
        if self.reach(walker, shorter, pass)
            && let (Some(symbol), Some(from)) = (walker.letter(letter), self.reached.get(shorter))
        {
            reached = pass.step(from, symbol);
        }
        if reached.is_empty() {
            self.refused.insert(spelling.to_owned());
            return false;
        }
        self.reached.insert(spelling.to_owned(), reached);
        true
    }
}

impl<'a, T: TransducerTablesInterface> Walker<'a, T> {
    fn label_text(&self, unit: &Unit) -> Option<String> {
        match self.grammar().label(unit) {
            Label::Text(text) => Some(text),
            Label::CompleteWord => None,
        }
    }

    /// The units the generator accepts next whose labels start with what
    /// `typed` stands for: prefixes and filters before the word's first
    /// stem, and stems, each completed letter by letter up to its first tag.
    /// A stem's label is its letters. Without a relaxation `typed` stands
    /// for itself. Typing is refused where no stem can start.
    // [spec:hfst:sem:prediction.entry]
    pub fn entry(
        &self,
        word: &Word,
        typed: &str,
        relaxation: Option<&dyn Relaxation>,
    ) -> Result<Next> {
        let place = Place::of(word.units());
        if !place.stem_may_start() {
            crate::bail!(
                FaultyStringInput,
                format!("letters {typed:?} are typed where no stem can start")
            );
        }
        let limits = self.limits();
        let mut pass = self.pass();
        let seeds = self.seeds(&mut pass, word);
        let mut offers = Offers::default();
        if place == Place::BeforeStem {
            self.prefixes(&seeds, &mut pass, &mut offers);
            self.filters(&seeds, &mut pass, &mut offers);
        }
        let labels: Vec<String> = offers
            .units()
            .filter_map(|unit| self.label_text(unit))
            .collect();
        let mut spelled = Spelled::new(seeds);
        let spellings = match relaxation {
            None => vec![typed.to_owned()],
            Some(relaxation) => {
                let found = relaxation.spellings(typed, &limits, &mut |spelling| {
                    labels.iter().any(|label| label.starts_with(spelling))
                        || spelled.reach(self, spelling, &mut pass)
                });
                pass.budget.search_cut |= found.truncated;
                found.spellings
            }
        };
        offers.retain(|unit| {
            self.label_text(unit).is_some_and(|label| {
                spellings
                    .iter()
                    .any(|spelling| label.starts_with(spelling.as_str()))
            })
        });
        let mut stems = Vec::new();
        for spelling in spellings.iter().filter(|spelling| !spelling.is_empty()) {
            if !spelled.reach(self, spelling, &mut pass) {
                continue;
            }
            let letters: Option<Vec<SymbolNumber>> =
                spelling.chars().map(|letter| self.letter(letter)).collect();
            if let (Some(letters), Some(reached)) = (letters, spelled.reached.get(spelling)) {
                stems.push((letters, reached.clone()));
            }
        }
        self.stems(stems, &mut pass, &mut offers);
        Ok(self.found(word, offers, &pass, true))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prediction::units::UnitKind;
    use crate::prediction::walk::fixtures_test::*;

    /// Every letter of the test lexicon stands for itself, and `a` may also
    /// stand for `á`.
    const RELAX_A: &str = "[ a | a:á | b | g | h | i | j | k | l | o | r | u ]*";

    fn kinds(next: &Next) -> Vec<(UnitKind, &str)> {
        next.candidates
            .iter()
            .map(|candidate| (candidate.unit.kind, candidate.key.as_str()))
            .collect()
    }

    // [spec:hfst:sem:prediction.entry/test]
    // [spec:hfst:sem:prediction-units.stem/test]
    #[test]
    fn typed_letters_match_and_complete_stems() {
        let generator = lexc(NOUNS);
        let grammar = grammar(NOUNS_GRAMMAR);
        let walker = walker(&generator, &grammar);
        let start = walker.start();
        let g = walker.entry(&start, "g", None).unwrap();
        assert_eq!(keys(&g), ["girji", "guolli", "gáhkku"]);
        assert!(g.candidates.iter().all(|c| c.unit.kind == UnitKind::Stem));
        assert!(!g.truncated);
        let gu = walker.entry(&start, "gu", None).unwrap();
        assert_eq!(keys(&gu), ["guolli"]);
        assert_eq!(candidate(&gu, "guolli").unit.symbols.len(), 6);
        assert!(
            walker
                .entry(&start, "x", None)
                .unwrap()
                .candidates
                .is_empty()
        );
    }

    // [spec:hfst:sem:prediction.entry/test]
    #[test]
    fn typed_letters_match_prefix_and_filter_labels() {
        let generator = lexc(PERSONS);
        let grammar = grammar(PERSONS_GRAMMAR);
        let walker = walker(&generator, &grammar);
        let start = walker.start();
        assert_eq!(
            kinds(&walker.entry(&start, "ni", None).unwrap()),
            [
                (UnitKind::Filter, "@U.person.NI@"),
                (UnitKind::Stem, "nipaa"),
            ]
        );
        assert_eq!(
            kinds(&walker.entry(&start, "ki", None).unwrap()),
            [
                (UnitKind::Filter, "@U.person.KI@"),
                (UnitKind::Prefix, "PV/kii+"),
            ]
        );
        assert_eq!(
            kinds(&walker.entry(&start, "", None).unwrap()),
            [
                (UnitKind::Filter, "@U.person.KI@"),
                (UnitKind::Filter, "@U.person.NI@"),
                (UnitKind::Prefix, "PV/kii+"),
            ]
        );
    }

    // [spec:hfst:sem:prediction.entry/test]
    #[test]
    fn a_relaxation_lets_letters_typed_without_diacritics_match() {
        let generator = lexc(NOUNS);
        let grammar = grammar(NOUNS_GRAMMAR);
        let walker = walker(&generator, &grammar);
        let relaxation = regex(RELAX_A);
        let start = walker.start();
        assert!(
            walker
                .entry(&start, "gah", None)
                .unwrap()
                .candidates
                .is_empty()
        );
        let relaxed = walker.entry(&start, "gah", Some(&relaxation)).unwrap();
        assert_eq!(keys(&relaxed), ["gáhkku"]);
        let plain = walker.entry(&start, "gu", Some(&relaxation)).unwrap();
        assert_eq!(keys(&plain), ["guolli"]);
    }

    // [spec:hfst:sem:prediction.entry/test]
    #[test]
    fn a_relaxation_reports_the_spellings_it_allows() {
        let relaxation = regex(RELAX_A);
        let limits = WalkLimits::default();
        let mut all = |_: &str| true;
        let found = relaxation.spellings("ga", &limits, &mut all);
        assert_eq!(found.spellings, ["ga", "gá"]);
        assert!(!found.truncated);
        let mut no_accent = |spelling: &str| !spelling.contains('á');
        assert_eq!(
            relaxation
                .spellings("ga", &limits, &mut no_accent)
                .spellings,
            ["ga"]
        );
        assert!(
            relaxation
                .spellings("gz", &limits, &mut all)
                .spellings
                .is_empty()
        );
    }

    // [spec:hfst:sem:prediction.entry/test]
    #[test]
    fn typing_where_no_stem_can_start_is_refused() {
        let generator = lexc(NOUNS);
        let grammar = grammar(NOUNS_GRAMMAR);
        let walker = walker(&generator, &grammar);
        let stem = word_after(&walker, "girji", &[]);
        assert!(!walker.stem_may_start(&stem));
        let refused = walker
            .entry(&stem, "g", None)
            .expect_err("typing is refused");
        assert!(
            refused.to_string().contains("no stem can start"),
            "{refused}"
        );
    }
}
