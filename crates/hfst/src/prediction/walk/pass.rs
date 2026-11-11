//! The scratch of one walk: flag values and outputs interned, what each
//! state reaches without reading a symbol, and the steps the walk takes.
//!
//! A word holds only the configurations right after the last symbol it
//! read, its seeds. A generator may write a morpheme's surface before its
//! tags, so the configurations one tag reaches without reading another can
//! spell every morpheme that may follow. So each state first learns, once
//! per walk, which input symbols it can read next and whether it can end
//! there, and a step over a symbol follows only the epsilon arcs that lead
//! to it.

use std::collections::BTreeMap;
use std::hash::Hasher;
use std::rc::Rc;

use smallvec::SmallVec;

use super::hash::{IdHasher, IdMap};
use super::machine::{Budget, Configuration, Edge, Machine};
use super::reach::{Reach, Reaches, Want};
use crate::hfst_flag_diacritics::{FdOperation, FdValue};
use crate::transducer::{SymbolNumber, TransducerTablesInterface, TransitionTableIndex, Weight};

type FlagsId = usize;
type OutputId = usize;

const EMPTY_OUTPUT: OutputId = 0;

/// Where a configuration of one pass is.
#[derive(Clone, Copy, Debug)]
struct Here {
    state: TransitionTableIndex,
    flags: FlagsId,
    output: OutputId,
}

/// A configuration within one pass.
#[derive(Clone, Copy, Debug)]
pub(in crate::prediction) struct Live {
    here: Here,
    weight: Weight,
}

impl Live {
    pub(in crate::prediction) fn weight(&self) -> Weight {
        self.weight
    }
}

/// Configurations, each at the lowest weight it was reached with, in the
/// order they were first reached. They are told apart by state, flags and
/// the symbols of their output, which a hash finds and a walk along the two
/// outputs confirms.
#[derive(Default)]
pub(in crate::prediction) struct LiveSet {
    lives: Vec<Live>,
    index: IdMap<(TransitionTableIndex, FlagsId, u64), SmallVec<[usize; 1]>>,
}

impl LiveSet {
    /// True when `live` is new or lowers the weight its configuration had.
    fn insert(&mut self, live: Live, outputs: &Outputs) -> bool {
        let key = (
            live.here.state,
            live.here.flags,
            outputs.hash(live.here.output),
        );
        let slots = self.index.entry(key).or_default();
        for &at in slots.iter() {
            let known = &mut self.lives[at];
            if !outputs.same(known.here.output, live.here.output) {
                continue;
            }
            if known.weight <= live.weight {
                return false;
            }
            known.weight = live.weight;
            return true;
        }
        slots.push(self.lives.len());
        self.lives.push(live);
        true
    }

    pub(in crate::prediction) fn as_slice(&self) -> &[Live] {
        &self.lives
    }

    pub(in crate::prediction) fn into_vec(self) -> Vec<Live> {
        self.lives
    }
}

/// One symbol written after its parent's, and a hash of the whole output.
#[derive(Clone, Copy, Debug)]
struct Written {
    parent: OutputId,
    symbol: SymbolNumber,
    hash: u64,
}

/// Outputs as a tree: a node is its parent with one more symbol, so a
/// configuration's output is one number however long it is.
struct Outputs {
    nodes: Vec<Written>,
}

impl Outputs {
    fn new() -> Outputs {
        Outputs {
            nodes: vec![Written {
                parent: EMPTY_OUTPUT,
                symbol: 0,
                hash: 0,
            }],
        }
    }

    fn push(&mut self, parent: OutputId, symbol: SymbolNumber) -> OutputId {
        let mut hasher = IdHasher::default();
        hasher.write_u64(self.nodes[parent].hash);
        hasher.write_u16(symbol);
        self.nodes.push(Written {
            parent,
            symbol,
            hash: hasher.finish(),
        });
        self.nodes.len() - 1
    }

    fn hash(&self, at: OutputId) -> u64 {
        self.nodes[at].hash
    }

    /// True when the outputs at `a` and `b` spell the same symbols.
    fn same(&self, mut a: OutputId, mut b: OutputId) -> bool {
        while a != b {
            let (left, right) = (self.nodes[a], self.nodes[b]);
            if a == EMPTY_OUTPUT
                || b == EMPTY_OUTPUT
                || left.hash != right.hash
                || left.symbol != right.symbol
            {
                return false;
            }
            a = left.parent;
            b = right.parent;
        }
        true
    }

    fn symbols(&self, mut at: OutputId) -> Vec<SymbolNumber> {
        let mut symbols = Vec::new();
        while at != EMPTY_OUTPUT {
            let written = self.nodes[at];
            symbols.push(written.symbol);
            at = written.parent;
        }
        symbols.reverse();
        symbols
    }
}

/// Flag values, each set of them interned once, and the result of each flag
/// diacritic on each.
#[derive(Default)]
struct Flags {
    values: Vec<Box<[FdValue]>>,
    ids: IdMap<Box<[FdValue]>, FlagsId>,
    applied: IdMap<(FlagsId, SymbolNumber), Option<FlagsId>>,
}

impl Flags {
    fn intern(&mut self, values: Box<[FdValue]>) -> FlagsId {
        if let Some(&id) = self.ids.get(&values) {
            return id;
        }
        self.values.push(values.clone());
        self.ids.insert(values, self.values.len() - 1);
        self.values.len() - 1
    }

    /// The flags after reading `input`: unchanged for epsilon, and for a
    /// flag diacritic its result, nothing when it fails.
    // [spec:hfst:req:prediction.flags]
    fn after<T: TransducerTablesInterface>(
        &mut self,
        machine: &Machine<'_, T>,
        flags: FlagsId,
        input: SymbolNumber,
    ) -> Option<FlagsId> {
        let Some(operation) = machine.flag_operation(input) else {
            return Some(flags);
        };
        if let Some(&known) = self.applied.get(&(flags, input)) {
            return known;
        }
        let applied = machine
            .apply(operation, &self.values[flags])
            .map(|values| self.intern(values));
        self.applied.insert((flags, input), applied);
        applied
    }
}

/// What one purpose looks for in the configurations a state reaches
/// without reading a symbol. Each purpose has one predicate, so what a state
/// offers for it is found once per walk.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(in crate::prediction) enum Purpose {
    Tags,
    Prefixes,
    Letters,
}

/// A configuration one state reaches without reading a symbol, relative to
/// that state, from which it can read a symbol of some purpose.
#[derive(Clone, Debug)]
struct Entry {
    state: TransitionTableIndex,
    flags: FlagsId,
    suffix: Vec<SymbolNumber>,
    weight: Weight,
}

/// The entries one state, with given flags, reaches for one purpose.
struct Frontier {
    entries: Vec<Entry>,
    /// For each symbol of the purpose, the entries with arcs that read it.
    by_symbol: BTreeMap<SymbolNumber, Vec<usize>>,
}

/// What a set of seeds can read next for one purpose.
pub(in crate::prediction) struct Readers {
    frontiers: Vec<Rc<Frontier>>,
    /// For each symbol, the seeds that can read it.
    by_symbol: BTreeMap<SymbolNumber, Vec<usize>>,
}

impl Readers {
    /// The symbols some seed can read. Flag values are kept, so each can
    /// be read.
    pub(in crate::prediction) fn symbols(&self) -> Vec<SymbolNumber> {
        self.by_symbol.keys().copied().collect()
    }
}

pub(in crate::prediction) struct Pass<'m, 'a, T: TransducerTablesInterface> {
    machine: &'m Machine<'a, T>,
    pub(in crate::prediction) budget: Budget,
    outputs: Outputs,
    flags: Flags,
    reaches: Reaches,
    frontiers: IdMap<(TransitionTableIndex, FlagsId, Purpose), Rc<Frontier>>,
}

impl<'m, 'a, T: TransducerTablesInterface> Pass<'m, 'a, T> {
    pub(in crate::prediction) fn new(machine: &'m Machine<'a, T>, configurations: usize) -> Self {
        Pass {
            machine,
            budget: Budget::new(configurations),
            outputs: Outputs::new(),
            flags: Flags::default(),
            reaches: Reaches::default(),
            frontiers: IdMap::default(),
        }
    }

    pub(in crate::prediction) fn start(&mut self) -> Live {
        let flags = self.flags.intern(self.machine.neutral_flags());
        Live {
            here: Here {
                state: 0,
                flags,
                output: EMPTY_OUTPUT,
            },
            weight: 0.0,
        }
    }

    pub(in crate::prediction) fn intern(&mut self, config: &Configuration) -> Live {
        let output = config
            .output
            .iter()
            .fold(EMPTY_OUTPUT, |at, &symbol| self.outputs.push(at, symbol));
        Live {
            here: Here {
                state: config.state,
                flags: self.flags.intern(config.flags.clone()),
                output,
            },
            weight: config.weight,
        }
    }

    pub(in crate::prediction) fn materialize(&self, live: &Live) -> Configuration {
        Configuration {
            state: live.here.state,
            flags: self.flags.values[live.here.flags].clone(),
            output: self.outputs.symbols(live.here.output),
            weight: live.weight,
            final_weight: self.machine.final_weight(live.here.state),
        }
    }

    pub(in crate::prediction) fn final_weight(&self, live: &Live) -> Option<Weight> {
        self.machine.final_weight(live.here.state)
    }

    /// Add `live` to `set`. True when it is new there or lowers the weight
    /// its configuration had.
    pub(in crate::prediction) fn insert(&self, set: &mut LiveSet, live: Live) -> bool {
        set.insert(live, &self.outputs)
    }

    pub(in crate::prediction) fn render(&self, live: &Live) -> String {
        self.machine.render(&self.outputs.symbols(live.here.output))
    }

    fn outgoing(&mut self, state: TransitionTableIndex) -> Rc<[SymbolNumber]> {
        self.reaches.outgoing(self.machine, state)
    }

    fn reach(&mut self, state: TransitionTableIndex) -> Rc<Reach> {
        self.reaches.of(self.machine, state)
    }

    fn along(&mut self, live: &Live, arc: &Edge, flags: FlagsId) -> Live {
        let output = match arc.output {
            Some(output) => self.outputs.push(live.here.output, output),
            None => live.here.output,
        };
        Live {
            here: Here {
                state: arc.target,
                flags,
                output,
            },
            weight: live.weight + arc.weight,
        }
    }

    /// Every configuration `seeds` reach without reading an input symbol on
    /// the way to what `want` asks for, the seeds included. A point reached
    /// at a lower weight goes round again, so each keeps its lowest weight;
    /// the budget ends a cycle that keeps writing output.
    // [spec:hfst:def:prediction.state]
    // [spec:hfst:req:prediction.flags]
    pub(in crate::prediction) fn explore(&mut self, seeds: &[Live], want: Want<'_>) -> Vec<Live> {
        let machine = self.machine;
        let mut set = LiveSet::default();
        let mut agenda: Vec<Live> = Vec::new();
        for seed in seeds {
            if want.met_by(&self.reach(seed.here.state)) && set.insert(*seed, &self.outputs) {
                agenda.push(*seed);
            }
        }
        'agenda: while let Some(live) = agenda.pop() {
            for arc in machine.epsilon_arcs(live.here.state) {
                let arc = machine.arc(arc);
                if !want.met_by(&self.reach(arc.target)) {
                    continue;
                }
                let Some(flags) = self.flags.after(machine, live.here.flags, arc.input) else {
                    continue;
                };
                if !self.budget.spend() {
                    break 'agenda;
                }
                let next = self.along(&live, &arc, flags);
                if set.insert(next, &self.outputs) {
                    agenda.push(next);
                }
            }
        }
        set.into_vec()
    }

    /// The seeds `seeds` reach by reading `symbol`.
    pub(in crate::prediction) fn step(
        &mut self,
        seeds: &[Live],
        symbol: SymbolNumber,
    ) -> Vec<Live> {
        let readers = self.explore(seeds, Want::Symbol(symbol));
        self.step_from(&readers, symbol)
    }

    /// What `state`, with `flags`, reaches for `purpose`, `wanted` being its
    /// predicate.
    fn frontier(
        &mut self,
        state: TransitionTableIndex,
        flags: FlagsId,
        purpose: Purpose,
        wanted: &dyn Fn(SymbolNumber) -> bool,
    ) -> Rc<Frontier> {
        if let Some(known) = self.frontiers.get(&(state, flags, purpose)) {
            return known.clone();
        }
        let root = Live {
            here: Here {
                state,
                flags,
                output: EMPTY_OUTPUT,
            },
            weight: 0.0,
        };
        let mut entries = Vec::new();
        let mut by_symbol: BTreeMap<SymbolNumber, Vec<usize>> = BTreeMap::new();
        for live in self.explore(&[root], Want::Reading(wanted)) {
            let symbols: Vec<SymbolNumber> = self
                .outgoing(live.here.state)
                .iter()
                .copied()
                .filter(|&symbol| wanted(symbol))
                .collect();
            if symbols.is_empty() {
                continue;
            }
            for symbol in symbols {
                by_symbol.entry(symbol).or_default().push(entries.len());
            }
            entries.push(Entry {
                state: live.here.state,
                flags: live.here.flags,
                suffix: self.outputs.symbols(live.here.output),
                weight: live.weight,
            });
        }
        let frontier = Rc::new(Frontier { entries, by_symbol });
        self.frontiers
            .insert((state, flags, purpose), frontier.clone());
        frontier
    }

    /// What `seeds` can read next for `purpose`, `wanted` being its
    /// predicate.
    pub(in crate::prediction) fn readers(
        &mut self,
        seeds: &[Live],
        purpose: Purpose,
        wanted: &dyn Fn(SymbolNumber) -> bool,
    ) -> Readers {
        let frontiers: Vec<Rc<Frontier>> = seeds
            .iter()
            .map(|seed| self.frontier(seed.here.state, seed.here.flags, purpose, wanted))
            .collect();
        let mut by_symbol: BTreeMap<SymbolNumber, Vec<usize>> = BTreeMap::new();
        for (at, frontier) in frontiers.iter().enumerate() {
            for &symbol in frontier.by_symbol.keys() {
                by_symbol.entry(symbol).or_default().push(at);
            }
        }
        Readers {
            frontiers,
            by_symbol,
        }
    }

    /// The seeds `seeds` reach by reading `symbol`, `readers` being what
    /// they can read.
    pub(in crate::prediction) fn step_readers(
        &mut self,
        seeds: &[Live],
        readers: &Readers,
        symbol: SymbolNumber,
    ) -> Vec<Live> {
        let mut from = Vec::new();
        for &at in readers.by_symbol.get(&symbol).into_iter().flatten() {
            let frontier = &readers.frontiers[at];
            for &entry in frontier.by_symbol.get(&symbol).into_iter().flatten() {
                let entry = &frontier.entries[entry];
                let output = entry
                    .suffix
                    .iter()
                    .fold(seeds[at].here.output, |output, &written| {
                        self.outputs.push(output, written)
                    });
                from.push(Live {
                    here: Here {
                        state: entry.state,
                        flags: entry.flags,
                        output,
                    },
                    weight: seeds[at].weight + entry.weight,
                });
            }
        }
        self.step_from(&from, symbol)
    }

    /// Where the arcs reading `symbol` from `lives` themselves lead.
    fn step_from(&mut self, lives: &[Live], symbol: SymbolNumber) -> Vec<Live> {
        let machine = self.machine;
        let mut reached = LiveSet::default();
        for live in lives {
            for arc in machine.symbol_arcs(live.here.state, symbol) {
                if !self.budget.spend() {
                    return reached.into_vec();
                }
                let next = self.along(live, &machine.arc(arc), live.here.flags);
                reached.insert(next, &self.outputs);
            }
        }
        reached.into_vec()
    }

    /// The final configurations `seeds` reach without reading a symbol.
    pub(in crate::prediction) fn finals(&mut self, seeds: &[Live]) -> Vec<Live> {
        let machine = self.machine;
        self.explore(seeds, Want::Final)
            .into_iter()
            .filter(|live| machine.final_weight(live.here.state).is_some())
            .collect()
    }

    /// Every configuration `seeds` reach without reading a symbol.
    pub(in crate::prediction) fn closure(&mut self, seeds: &[Live]) -> Vec<Live> {
        self.explore(seeds, Want::Everything)
    }

    /// True when some of `seeds` can read a symbol `wanted` takes, flag
    /// values included. The search goes deep first and stops at the first
    /// such configuration, so it does not walk what a state reaches in full.
    pub(in crate::prediction) fn can_read(
        &mut self,
        seeds: &[Live],
        wanted: &dyn Fn(SymbolNumber) -> bool,
    ) -> bool {
        let machine = self.machine;
        let want = Want::Reading(wanted);
        let mut seen = LiveSet::default();
        let mut agenda: Vec<Live> = seeds.to_vec();
        while let Some(live) = agenda.pop() {
            if !seen.insert(live, &self.outputs) || !want.met_by(&self.reach(live.here.state)) {
                continue;
            }
            if self
                .outgoing(live.here.state)
                .iter()
                .any(|&symbol| wanted(symbol))
            {
                return true;
            }
            for arc in machine.epsilon_arcs(live.here.state) {
                let arc = machine.arc(arc);
                let Some(flags) = self.flags.after(machine, live.here.flags, arc.input) else {
                    continue;
                };
                if !self.budget.spend() {
                    return false;
                }
                let next = self.along(&live, &arc, flags);
                agenda.push(next);
            }
        }
        false
    }

    /// `live` with `operations` applied in order where it is, or nothing
    /// when one fails.
    // [spec:hfst:req:prediction.flags]
    pub(in crate::prediction) fn apply_all(
        &mut self,
        live: &Live,
        operations: &[FdOperation],
    ) -> Option<Live> {
        let mut values = self.flags.values[live.here.flags].clone();
        for operation in operations {
            values = self.machine.apply(operation, &values)?;
        }
        let mut applied = *live;
        applied.here.flags = self.flags.intern(values);
        Some(applied)
    }
}
