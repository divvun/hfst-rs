//! The walk: the units a language's generator accepts next from a word in
//! progress, drawn by the language's unit grammar, and the surface forms of a
//! complete word (`docs/spec/prediction.md`).

mod accept;
mod hash;
mod lattice;
mod machine;
mod memo;
mod pass;
mod reach;
mod runs;
mod seeds;
mod words;

#[cfg(test)]
mod accept_tests;
#[cfg(test)]
pub(super) mod fixtures_test;
#[cfg(test)]
mod memo_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod words_tests;

use std::collections::{BTreeMap, HashMap};

pub use accept::Acceptor;
pub use machine::Configuration;
pub(super) use machine::Machine;
use memo::Shelf;
use pass::Purpose;
pub(super) use pass::{Live, Pass};
use reach::{Marks, Role};
pub(super) use runs::Offers;
use seeds::{Finals, Seeds};
pub use words::{Suggest, Suggestion, WholeWord};

use super::rank::rank_scored;
use super::units::{SymbolClass, Unit, UnitGrammar, UnitKey, UnitKind};
use crate::error::Result;
use crate::hfst_data_types::Symbol;
use crate::hfst_flag_diacritics::FdOperation;
use crate::transducer::{SymbolNumber, Transducer, TransducerTablesInterface, Weight};

/// The bounds on one walk. Both are counted per call, so a walk ends whatever
/// cycles the generator has.
// [spec:hfst:req:prediction.bounded]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WalkLimits {
    /// The most symbols one unit may have.
    pub max_unit_symbols: usize,
    /// The most configurations one call may visit.
    pub max_configurations: usize,
}

impl Default for WalkLimits {
    fn default() -> Self {
        WalkLimits {
            max_unit_symbols: 48,
            max_configurations: 1_000_000,
        }
    }
}

/// A word in progress: the units picked so far and every configuration they
/// reach. It keeps the configurations right after the last symbol read, from
/// which the others follow without reading, and the final ones among all of
/// them.
// [spec:hfst:def:prediction.state]
#[derive(Clone, Debug)]
pub struct Word {
    units: Vec<Unit>,
    seeds: Seeds,
    finals: Finals,
    truncated: bool,
}

impl Word {
    pub fn units(&self) -> &[Unit] {
        &self.units
    }

    /// True when a bound cut the walk that reached this word, so it may be
    /// missing configurations.
    pub fn truncated(&self) -> bool {
        self.truncated
    }

    // [spec:hfst:sem:prediction.complete]
    pub fn is_complete(&self) -> bool {
        !self.finals.is_empty()
    }

    /// The word with `candidate`, one of the units offered for it, picked.
    /// Every configuration the candidate reached goes on.
    // [spec:hfst:req:prediction.generator-decides]
    pub fn pick(&self, candidate: &Candidate) -> Word {
        let mut units = self.units.clone();
        units.push(candidate.unit.clone());
        Word {
            units,
            seeds: candidate.seeds.clone(),
            finals: candidate.finals.clone(),
            truncated: self.truncated || candidate.truncated,
        }
    }
}

/// A unit the generator accepts next, with every configuration a path with
/// its key reaches.
// [spec:hfst:def:prediction.unit]
#[derive(Clone, Debug)]
pub struct Candidate {
    /// The symbols of the lowest-weight path with this key.
    pub unit: Unit,
    pub key: UnitKey,
    /// The lowest weight of a path to the unit, with the final weight for an
    /// ending or an enclitic, which make the word complete.
    pub weight: Weight,
    seeds: Seeds,
    finals: Finals,
    truncated: bool,
}

/// A surface form of a complete word, at the lowest weight of the paths that
/// write it.
#[derive(Clone, Debug, PartialEq)]
pub struct Form {
    pub surface: String,
    pub weight: Weight,
}

/// What a walk found.
// [spec:hfst:req:prediction.bounded]
#[derive(Clone, Debug)]
pub struct Next {
    /// In the order of their keys.
    pub candidates: Vec<Candidate>,
    /// The forms of the word, lowest weight first, when it is complete.
    pub forms: Vec<Form>,
    /// A stem can come next. Stems are offered through entry, for the
    /// letters typed, and never listed here: a lexicon has too many.
    pub stem_next: bool,
    /// A bound stopped the walk, so the candidates and forms may not be all
    /// there are.
    pub truncated: bool,
}

/// Scores candidates after a context, which puts them in the order to show
/// them.
pub trait CandidateOrder {
    /// Each candidate's score after the context, none for a candidate it
    /// does not score. A higher score ranks first.
    fn scores(&self, context: &[UnitKey], candidates: &[(UnitKey, Weight)]) -> Vec<Option<f64>>;

    /// Each candidate's share of what follows the context: none where
    /// [`CandidateOrder::scores`] gives none, otherwise at most 1, and in the
    /// order of the scores. A complete word scores its stem's score times
    /// its ending's share after the stem.
    fn shares(&self, context: &[UnitKey], candidates: &[(UnitKey, Weight)]) -> Vec<Option<f64>>;

    /// Indices into `candidates`, in the order to show them: scored first,
    /// highest first, then the rest by the generator's weight, lowest first,
    /// then by the bytes of the key.
    fn order(&self, context: &[UnitKey], candidates: &[(UnitKey, Weight)]) -> Vec<usize> {
        rank_scored(&self.scores(context, candidates), candidates)
    }
}

/// Scores nothing, so it orders by the generator's weight, lowest first,
/// then by the bytes of the key.
#[derive(Clone, Copy, Debug, Default)]
pub struct GeneratorWeightOrder;

impl CandidateOrder for GeneratorWeightOrder {
    fn scores(&self, _context: &[UnitKey], candidates: &[(UnitKey, Weight)]) -> Vec<Option<f64>> {
        vec![None; candidates.len()]
    }

    fn shares(&self, _context: &[UnitKey], candidates: &[(UnitKey, Weight)]) -> Vec<Option<f64>> {
        vec![None; candidates.len()]
    }
}

/// Where a word in progress is, by the kind of its latest unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Place {
    /// Prefixes and filters can come next, and a stem.
    BeforeStem,
    /// Derivations, compound joins and endings can come next, and after a
    /// boundary unit a stem.
    AfterStem { after_boundary: bool },
    /// Enclitics can come next.
    Complete,
}

impl Place {
    pub(super) fn of(units: &[Unit]) -> Place {
        match units.last().map(|unit| unit.kind) {
            None | Some(UnitKind::Prefix) | Some(UnitKind::Filter) => Place::BeforeStem,
            Some(UnitKind::Stem) => Place::AfterStem {
                after_boundary: false,
            },
            Some(UnitKind::Boundary) => Place::AfterStem {
                after_boundary: true,
            },
            Some(UnitKind::Ending) | Some(UnitKind::Enclitic) => Place::Complete,
        }
    }

    pub(super) fn stem_may_start(self) -> bool {
        matches!(
            self,
            Place::BeforeStem
                | Place::AfterStem {
                    after_boundary: true
                }
        )
    }
}

/// What the unit grammar makes of one input symbol of the generator.
#[derive(Clone, Copy, Debug)]
struct SymbolInfo {
    class: SymbolClass,
    boundary: bool,
    prefix: bool,
    derivation: bool,
    enclitic: bool,
}

impl SymbolInfo {
    fn role(&self) -> Role {
        match self.class {
            SymbolClass::Tag if self.derivation && self.boundary => Role::DerivationBoundary,
            SymbolClass::Tag if self.derivation => Role::Derivation,
            SymbolClass::Tag if self.boundary => Role::Boundary,
            SymbolClass::Tag => Role::Through,
            SymbolClass::Letter | SymbolClass::Flag | SymbolClass::Reserved => Role::Stop,
        }
    }
}

/// Walks one generator's analysis side with one unit grammar. The generator
/// is borrowed and its tables are read where they are loaded.
// [spec:hfst:req:prediction.views]
pub struct Walker<'a, T: TransducerTablesInterface> {
    machine: Machine<'a, T>,
    grammar: &'a UnitGrammar,
    limits: WalkLimits,
    symbols: Vec<SymbolInfo>,
    roles: Vec<Role>,
    marks: Vec<Marks>,
    letters: HashMap<char, SymbolNumber>,
    filters: Vec<(Unit, Vec<FdOperation>)>,
    /// What earlier walks learnt of the generator.
    shelf: Shelf,
}

impl<'a, T: TransducerTablesInterface> Walker<'a, T> {
    /// The grammar is checked against the generator's analysis-side
    /// alphabet first.
    pub fn new(
        generator: &'a Transducer<T>,
        grammar: &'a UnitGrammar,
        limits: WalkLimits,
    ) -> Result<Walker<'a, T>> {
        let machine = Machine::new(generator);
        let table = generator.get_symbol_table();
        let inputs = table
            .get(..machine.input_count() as usize)
            .unwrap_or(table.as_slice());
        // Flag diacritics are numbered after the input symbols, but they are
        // on the analysis side too.
        let flags = (inputs.len()..table.len())
            .filter(|&at| {
                SymbolNumber::try_from(at).is_ok_and(|at| machine.flag_operation(at).is_some())
            })
            .map(|at| &table[at]);
        let analysis: Vec<&Symbol> = inputs.iter().chain(flags).collect();
        grammar.validate(&analysis)?;
        let symbols: Vec<SymbolInfo> = inputs
            .iter()
            .map(|name| SymbolInfo {
                class: grammar.classify(name),
                boundary: grammar.is_boundary(name),
                prefix: grammar.is_prefix(name),
                derivation: grammar.is_derivation(name),
                enclitic: grammar.is_enclitic(name),
            })
            .collect();
        let roles = symbols.iter().map(SymbolInfo::role).collect();
        let marks = symbols
            .iter()
            .map(|info| reach::marks_of(info.class, info.prefix))
            .collect();
        let letters = inputs
            .iter()
            .zip(&symbols)
            .enumerate()
            .filter(|(_, (_, info))| info.class == SymbolClass::Letter)
            .filter_map(|(at, (name, _))| {
                Some((name.chars().next()?, SymbolNumber::try_from(at).ok()?))
            })
            .collect();
        let mut filters = Vec::new();
        for filter in grammar.filters() {
            let mut operations = Vec::new();
            for flag in &filter.flags {
                operations.push(machine.operation(flag).ok_or_else(|| {
                    crate::err!(
                        Hfst,
                        format!(
                            "unit grammar: filter {:?} sets {flag}, which the generator does not define",
                            filter.name
                        )
                    )
                })?);
            }
            filters.push((filter.unit(), operations));
        }
        Ok(Walker {
            machine,
            grammar,
            limits,
            symbols,
            roles,
            marks,
            letters,
            filters,
            shelf: Shelf::default(),
        })
    }

    /// The generator, as it was lent: the walk holds no copy of it.
    pub fn generator(&self) -> &'a Transducer<T> {
        self.machine.transducer()
    }

    pub fn grammar(&self) -> &'a UnitGrammar {
        self.grammar
    }

    pub fn limits(&self) -> WalkLimits {
        self.limits
    }

    pub(super) fn pass(&self) -> Pass<'_, 'a, T> {
        Pass::kept(
            &self.machine,
            (&self.roles, &self.marks),
            self.limits.max_configurations,
            &self.shelf,
        )
    }

    /// The word's seeds in `pass`. When the word is complete, its final
    /// configurations are found in `pass` too, if they are not known yet.
    pub(super) fn seeds(&self, pass: &mut Pass<'_, 'a, T>, word: &Word) -> Vec<Live> {
        let (configurations, cut) = word.seeds.get();
        pass.budget.configurations_cut |= cut;
        let seeds: Vec<Live> = configurations
            .iter()
            .map(|config| pass.intern(config))
            .collect();
        word.finals.get_or(|_| pass.final_configurations(&seeds));
        seeds
    }

    pub(super) fn letter(&self, letter: char) -> Option<SymbolNumber> {
        self.letters.get(&letter).copied()
    }

    /// The empty word: every configuration the generator reaches from its
    /// start state without reading an analysis symbol.
    // [spec:hfst:def:prediction.state]
    pub fn start(&self) -> Word {
        let mut pass = self.pass();
        let start = pass.start();
        let seeds = Seeds::spelled(vec![pass.materialize(&start)]);
        Word {
            units: Vec::new(),
            finals: Finals::found(seeds.clone(), pass.final_configurations(&[start])),
            seeds,
            truncated: pass.budget.truncated(),
        }
    }

    /// Every configuration the word reaches.
    // [spec:hfst:def:prediction.state]
    pub fn configurations(&self, word: &Word) -> Vec<Configuration> {
        let mut pass = self.pass();
        let seeds = self.seeds(&mut pass, word);
        let closure = pass.closure(&seeds);
        closure.iter().map(|live| pass.materialize(live)).collect()
    }

    /// True when letters typed now start a stem: before the word's first
    /// stem, or after a boundary unit, as after a compound join.
    pub fn stem_may_start(&self, word: &Word) -> bool {
        Place::of(&word.units).stem_may_start()
    }

    /// The units the generator accepts next, drawn by the grammar, and the
    /// word's forms when it is complete. Before a stem these are prefixes
    /// and filters; after a stem or a boundary unit, boundary units and
    /// endings; after an ending, enclitics. Stems come through
    /// [`Walker::entry`].
    // [spec:hfst:req:prediction.generator-decides]
    pub fn next(&self, word: &Word) -> Next {
        let mut pass = self.pass();
        let seeds = self.seeds(&mut pass, word);
        let mut offers = Offers::default();
        let place = Place::of(&word.units);
        match place {
            Place::BeforeStem => {
                self.prefixes(&seeds, &mut pass, &mut offers);
                self.filters(&seeds, &mut pass, &mut offers);
            }
            Place::AfterStem { .. } | Place::Complete => self.tag_runs(
                &seeds,
                word.is_complete(),
                place,
                false,
                &mut pass,
                &mut offers,
            ),
        }
        let stem_next = place.stem_may_start()
            && pass.can_read(&seeds, Purpose::Letters, &|symbol| {
                self.class(symbol) == SymbolClass::Letter
            });
        self.found(word, offers, &pass, stem_next)
    }

    /// The endings among what [`Walker::next`] offers, found without walking
    /// the runs that can only become boundary units, which after a stem of
    /// a language with long chains of derivations are most of the walk.
    /// Where a bound cuts the full walk short, this one may find endings it
    /// missed. A word with no stem yet has none.
    // [spec:hfst:sem:prediction.whole-words]
    pub fn endings(&self, word: &Word) -> Next {
        let mut pass = self.pass();
        let seeds = self.seeds(&mut pass, word);
        let mut offers = Offers::default();
        let place = Place::of(&word.units);
        match place {
            Place::AfterStem { .. } => {
                self.tag_runs(&seeds, false, place, true, &mut pass, &mut offers)
            }
            Place::BeforeStem | Place::Complete => {}
        }
        self.found(word, offers, &pass, false)
    }

    pub(super) fn found(
        &self,
        word: &Word,
        offers: Offers,
        pass: &Pass<'_, 'a, T>,
        stem_next: bool,
    ) -> Next {
        let cut = word.truncated || pass.budget.configurations_cut;
        Next {
            candidates: offers.into_candidates(pass, cut),
            forms: self.forms(word),
            stem_next,
            truncated: word.truncated || pass.budget.truncated(),
        }
    }

    pub(super) fn class(&self, symbol: SymbolNumber) -> SymbolClass {
        self.symbols
            .get(symbol as usize)
            .map_or(SymbolClass::Reserved, |info| info.class)
    }

    /// Every surface form the word's final configurations wrote, each at the
    /// lowest weight among the paths that wrote it, final weight included.
    // [spec:hfst:sem:prediction.complete]
    pub fn forms(&self, word: &Word) -> Vec<Form> {
        self.forms_of(&word.finals)
    }

    fn forms_of(&self, finals: &Finals) -> Vec<Form> {
        let finals = finals.get_or(|seeds| {
            let mut pass = self.pass();
            let seeds: Vec<Live> = seeds.iter().map(|config| pass.intern(config)).collect();
            pass.final_configurations(&seeds)
        });
        let mut best: BTreeMap<String, Weight> = BTreeMap::new();
        for config in finals {
            let Some(final_weight) = config.final_weight else {
                continue;
            };
            let weight = config.weight + final_weight;
            best.entry(self.machine.render(&config.output))
                .and_modify(|known| *known = known.min(weight))
                .or_insert(weight);
        }
        let mut forms: Vec<Form> = best
            .into_iter()
            .map(|(surface, weight)| Form { surface, weight })
            .collect();
        forms.sort_by(|a, b| {
            a.weight
                .total_cmp(&b.weight)
                .then_with(|| a.surface.cmp(&b.surface))
        });
        forms
    }

    /// The word's own form when it is complete; otherwise the word completed
    /// by the ending `order` ranks first among those the walk reaches. None
    /// when no ending is in reach. `context` is what `order` ranks after:
    /// the keys of earlier units, the word's own last.
    // [spec:hfst:sem:prediction.preview]
    pub fn preview(
        &self,
        word: &Word,
        context: &[UnitKey],
        order: &dyn CandidateOrder,
    ) -> Option<String> {
        if word.is_complete() {
            return self.forms(word).into_iter().next().map(|form| form.surface);
        }
        self.preview_from(word, &self.next(word), context, order)
    }

    /// [`Walker::preview`] from `next`, which must be what [`Walker::next`]
    /// found for `word`, so a caller that has that walk does not walk the
    /// word again.
    // [spec:hfst:sem:prediction.preview]
    pub fn preview_from(
        &self,
        word: &Word,
        next: &Next,
        context: &[UnitKey],
        order: &dyn CandidateOrder,
    ) -> Option<String> {
        if word.is_complete() {
            return next.forms.first().map(|form| form.surface.clone());
        }
        let endings: Vec<&Candidate> = next
            .candidates
            .iter()
            .filter(|candidate| candidate.unit.kind == UnitKind::Ending)
            .collect();
        let ranked: Vec<(UnitKey, Weight)> = endings
            .iter()
            .map(|candidate| (candidate.key.clone(), candidate.weight))
            .collect();
        let first = order.order(context, &ranked).into_iter().next()?;
        let ending = endings.get(first)?;
        self.forms_of(&ending.finals)
            .into_iter()
            .next()
            .map(|form| form.surface)
    }

    /// What the word's paths have written by its last unit, each surface
    /// once. A path only adds to what it has written, so every form and
    /// every preview of the word, and of each word built on from it, starts
    /// with one of these. A caller can rule out a preview by them without
    /// walking for it.
    pub fn preview_starts(&self, word: &Word) -> Vec<String> {
        let mut starts: Vec<String> = word
            .seeds
            .iter()
            .map(|config| self.machine.render(&config.output))
            .collect();
        starts.sort_unstable();
        starts.dedup();
        starts
    }
}
