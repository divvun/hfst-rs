//! Drawing units from the generator: prefixes, filters, stems, and the runs
//! of tags after a stem or a complete word.

use std::collections::{HashMap, VecDeque};

use super::pass::{Live, LiveSet, Pass, Purpose};
use super::reach::Role;
use super::{Candidate, Place, SymbolInfo, Walker};
use crate::prediction::units::{SymbolClass, Unit, UnitKey, UnitKind};
use crate::transducer::{SymbolNumber, TransducerTablesInterface, Weight};

/// The units found so far, one per key.
#[derive(Default)]
pub(in crate::prediction) struct Offers {
    by_key: HashMap<UnitKey, Offer>,
}

struct Offer {
    unit: Unit,
    weight: Weight,
    seeds: LiveSet,
    finals: LiveSet,
}

impl Offers {
    /// Units with the same key are one candidate, which keeps every
    /// configuration either reached and the symbols of the lighter. An
    /// ending or an enclitic makes the word complete, so it weighs what the
    /// complete word does.
    // [spec:hfst:def:prediction.unit]
    // [spec:hfst:req:prediction.generator-decides]
    pub(in crate::prediction) fn add<T: TransducerTablesInterface>(
        &mut self,
        pass: &mut Pass<'_, '_, T>,
        key: UnitKey,
        unit: Unit,
        seeds: Vec<Live>,
    ) {
        let finals = pass.finals(&seeds);
        let weight = match unit.kind {
            UnitKind::Ending | UnitKind::Enclitic => finals
                .iter()
                .map(|live| live.weight() + pass.final_weight(live).unwrap_or(0.0))
                .fold(Weight::INFINITY, Weight::min),
            UnitKind::Stem | UnitKind::Prefix | UnitKind::Boundary | UnitKind::Filter => seeds
                .iter()
                .map(Live::weight)
                .fold(Weight::INFINITY, Weight::min),
        };
        let offer = self.by_key.entry(key).or_insert_with(|| Offer {
            unit: unit.clone(),
            weight,
            seeds: LiveSet::default(),
            finals: LiveSet::default(),
        });
        if weight < offer.weight {
            offer.unit = unit;
            offer.weight = weight;
        }
        for live in seeds {
            pass.insert(&mut offer.seeds, live);
        }
        for live in finals {
            pass.insert(&mut offer.finals, live);
        }
    }

    pub(in crate::prediction) fn units(&self) -> impl Iterator<Item = &Unit> {
        self.by_key.values().map(|offer| &offer.unit)
    }

    pub(in crate::prediction) fn retain(&mut self, mut keep: impl FnMut(&Unit) -> bool) {
        self.by_key.retain(|_, offer| keep(&offer.unit));
    }

    /// `cut` marks every candidate as possibly missing configurations.
    pub(in crate::prediction) fn into_candidates<T: TransducerTablesInterface>(
        self,
        pass: &Pass<'_, '_, T>,
        cut: bool,
    ) -> Vec<Candidate> {
        let mut candidates: Vec<Candidate> = self
            .by_key
            .into_iter()
            .map(|(key, offer)| Candidate {
                unit: offer.unit,
                key,
                weight: offer.weight,
                seeds: offer
                    .seeds
                    .as_slice()
                    .iter()
                    .map(|live| pass.materialize(live))
                    .collect(),
                finals: offer
                    .finals
                    .as_slice()
                    .iter()
                    .map(|live| pass.materialize(live))
                    .collect(),
                truncated: cut,
            })
            .collect();
        candidates.sort_by(|a, b| a.key.cmp(&b.key));
        candidates
    }
}

/// A run of tags being read, and what it can still become.
struct TagRun {
    symbols: Vec<SymbolNumber>,
    seeds: Vec<Live>,
    /// No enclitic, boundary or derivation tag yet, after a stem or a
    /// boundary unit.
    ending: bool,
    /// After a complete word: starts with an enclitic tag and has no other.
    enclitic: bool,
    /// The run holds a derivation tag, and can take no other.
    derived: bool,
}

/// What a run of tags becomes with one more tag.
struct Extension {
    boundary: bool,
    ending: bool,
    enclitic: bool,
    derived: bool,
}

impl<'a, T: TransducerTablesInterface> Walker<'a, T> {
    fn info(&self, symbol: SymbolNumber) -> Option<SymbolInfo> {
        self.symbols.get(symbol as usize).copied()
    }

    pub(in crate::prediction) fn offer(
        &self,
        pass: &mut Pass<'_, 'a, T>,
        offers: &mut Offers,
        kind: UnitKind,
        symbols: &[SymbolNumber],
        seeds: Vec<Live>,
    ) {
        let unit = Unit {
            kind,
            symbols: symbols
                .iter()
                .map(|&symbol| self.machine.name(symbol))
                .collect(),
        };
        offers.add(pass, self.grammar.key(&unit), unit, seeds);
    }

    /// Before a word's first stem, each tag that matches a prefix pattern.
    // [spec:hfst:sem:prediction-units.prefix]
    pub(in crate::prediction) fn prefixes(
        &self,
        seeds: &[Live],
        pass: &mut Pass<'_, 'a, T>,
        offers: &mut Offers,
    ) {
        let is_prefix = |symbol: SymbolNumber| {
            self.info(symbol)
                .is_some_and(|info| info.class == SymbolClass::Tag && info.prefix)
        };
        let readers = pass.readers(seeds, Purpose::Prefixes, &is_prefix);
        for symbol in readers.symbols() {
            let reached = pass.step_readers(seeds, &readers, symbol);
            if !reached.is_empty() {
                self.offer(pass, offers, UnitKind::Prefix, &[symbol], reached);
            }
        }
    }

    /// Each filter some configuration survives, applied to every
    /// configuration the word reaches. The survivors go on from where they
    /// are, so a flag arc that failed before the filter may hold after it.
    // [spec:hfst:sem:prediction-units.filter]
    pub(in crate::prediction) fn filters(
        &self,
        seeds: &[Live],
        pass: &mut Pass<'_, 'a, T>,
        offers: &mut Offers,
    ) {
        let closure = pass.closure(seeds);
        for (unit, operations) in &self.filters {
            let survivors: Vec<Live> = closure
                .iter()
                .filter_map(|live| pass.apply_all(live, operations))
                .collect();
            if !survivors.is_empty() {
                offers.add(pass, self.grammar.key(unit), unit.clone(), survivors);
            }
        }
    }

    /// Stems that go on from `seeds`, each a run of letters and the
    /// configurations after it, letter by letter up to the first tag. A run
    /// after which a tag can come, or the word is final, is a stem.
    // [spec:hfst:sem:prediction-units.stem]
    pub(in crate::prediction) fn stems(
        &self,
        seeds: Vec<(Vec<SymbolNumber>, Vec<Live>)>,
        pass: &mut Pass<'_, 'a, T>,
        offers: &mut Offers,
    ) {
        let is_tag = |symbol: SymbolNumber| self.class(symbol) == SymbolClass::Tag;
        let is_letter = |symbol: SymbolNumber| self.class(symbol) == SymbolClass::Letter;
        let mut queue: VecDeque<(Vec<SymbolNumber>, Vec<Live>)> = seeds.into();
        while let Some((letters, seeds)) = queue.pop_front() {
            let tag_follows = pass.can_read(&seeds, &is_tag);
            if !letters.is_empty() && (tag_follows || !pass.finals(&seeds).is_empty()) {
                self.offer(pass, offers, UnitKind::Stem, &letters, seeds.clone());
            }
            let readers = pass.readers(&seeds, Purpose::Letters, &is_letter);
            for symbol in readers.symbols() {
                if letters.len() >= self.limits.max_unit_symbols {
                    pass.budget.search_cut = true;
                    continue;
                }
                let reached = pass.step_readers(&seeds, &readers, symbol);
                if !reached.is_empty() {
                    let mut longer = letters.clone();
                    longer.push(symbol);
                    queue.push_back((longer, reached));
                }
            }
            if pass.budget.configurations_cut {
                return;
            }
        }
    }

    /// The runs of tags after a stem or a boundary unit, or after a
    /// complete word. A run ends at its first boundary tag, as a boundary
    /// unit. A run with no boundary, derivation or enclitic tag after which
    /// some configuration is final is an ending, and goes on, since a longer
    /// ending or a boundary unit may extend it. After a complete word a run
    /// that starts with an enclitic tag, has no other and after which some
    /// configuration is final is an enclitic. A run that only a boundary
    /// tag can still make a unit goes on only while one is in reach, so a
    /// generator that lets a Der tag be left out does not send the walk
    /// through every ending after every morpheme name.
    // [spec:hfst:req:prediction.generator-decides]
    // [spec:hfst:sem:prediction-units.boundary+3]
    // [spec:hfst:sem:prediction-units.ending+1]
    // [spec:hfst:sem:prediction-units.enclitic]
    pub(in crate::prediction) fn tag_runs(
        &self,
        seeds: &[Live],
        complete: bool,
        place: Place,
        pass: &mut Pass<'_, 'a, T>,
        offers: &mut Offers,
    ) {
        let after_stem = matches!(place, Place::AfterStem { .. });
        let is_tag = |symbol: SymbolNumber| self.class(symbol) == SymbolClass::Tag;
        let goes_on = |symbol: SymbolNumber| {
            self.roles
                .get(symbol as usize)
                .is_some_and(|&role| role != Role::Stop)
        };
        let goes_on_derived = |symbol: SymbolNumber| {
            self.roles
                .get(symbol as usize)
                .is_some_and(|&role| matches!(role, Role::Through | Role::Boundary))
        };
        let mut queue = VecDeque::from([TagRun {
            symbols: Vec::new(),
            seeds: seeds.to_vec(),
            ending: after_stem,
            enclitic: complete,
            derived: false,
        }]);
        while let Some(run) = queue.pop_front() {
            let readers = if run.ending || run.enclitic {
                pass.readers(&run.seeds, Purpose::Tags, &is_tag)
            } else if run.derived {
                pass.readers(&run.seeds, Purpose::Bounded(true), &goes_on_derived)
            } else {
                pass.readers(&run.seeds, Purpose::Bounded(false), &goes_on)
            };
            for symbol in readers.symbols() {
                let Some(extension) = self.extend(&run, symbol, after_stem) else {
                    continue;
                };
                if run.symbols.len() >= self.limits.max_unit_symbols {
                    pass.budget.search_cut = true;
                    continue;
                }
                let reached = pass.step_readers(&run.seeds, &readers, symbol);
                if reached.is_empty() {
                    continue;
                }
                let mut symbols = run.symbols.clone();
                symbols.push(symbol);
                if extension.boundary {
                    self.offer(pass, offers, UnitKind::Boundary, &symbols, reached);
                    continue;
                }
                if (extension.ending || extension.enclitic) && !pass.finals(&reached).is_empty() {
                    let kind = if extension.ending {
                        UnitKind::Ending
                    } else {
                        UnitKind::Enclitic
                    };
                    self.offer(pass, offers, kind, &symbols, reached.clone());
                }
                if !extension.ending
                    && !extension.enclitic
                    && !pass.bounded(&reached, extension.derived)
                {
                    continue;
                }
                queue.push_back(TagRun {
                    symbols,
                    seeds: reached,
                    ending: extension.ending,
                    enclitic: extension.enclitic,
                    derived: extension.derived,
                });
            }
            if pass.budget.configurations_cut {
                return;
            }
        }
    }

    /// What `run` can become with `symbol` after it, or nothing when the
    /// symbol is not a tag or the run could become no unit with it.
    /// A second derivation tag ends the search there, so chains of
    /// derivation tags are never walked. A run with a derivation tag in it
    /// is never an ending: only a boundary tag can end it.
    // [spec:hfst:sem:prediction-units.boundary+3]
    fn extend(&self, run: &TagRun, symbol: SymbolNumber, after_stem: bool) -> Option<Extension> {
        let info = self.info(symbol)?;
        if info.class != SymbolClass::Tag || (info.derivation && run.derived) {
            return None;
        }
        let enclitic = if run.symbols.is_empty() {
            run.enclitic && info.enclitic
        } else {
            run.enclitic && !info.enclitic && !info.boundary
        };
        let extension = Extension {
            boundary: after_stem && info.boundary,
            ending: run.ending && !info.enclitic && !info.boundary && !info.derivation,
            enclitic,
            derived: run.derived || info.derivation,
        };
        (after_stem || extension.enclitic).then_some(extension)
    }
}
