//! The scratch of one walk: outputs interned, the walker's memo while the
//! walk holds it, and the steps the walk takes.
//!
//! A word holds only the configurations right after the last symbol it
//! read, its seeds. A generator may write a morpheme's surface before its
//! tags, so the configurations one tag reaches without reading another can
//! spell every morpheme that may follow. So each state first learns which
//! input symbols it can read next and whether it can end there, and a step
//! over a symbol follows only the epsilon arcs that lead to it. What a
//! state learns is kept in the memo for later walks.

use std::cell::RefCell;
use std::sync::Arc;

use super::hash::{IdMap, IdSet};
use super::lattice::{Lattice, Reached, Spot};
use super::machine::{Budget, Configuration, Edge, Machine};
use super::memo::{Entry, Frontier, FrontierKey, Memo, Question, Reading, Shelf};
use super::reach::{FINAL, LETTER, Marks, PREFIX, Reach, Role, TAG, Want};
use crate::hfst_flag_diacritics::{FdOperation, FdValue};
use crate::transducer::{SymbolNumber, TransducerTablesInterface, TransitionTableIndex, Weight};

pub(in crate::prediction) type FlagsId = usize;

pub(super) type OutputId = usize;

pub(super) const EMPTY_OUTPUT: OutputId = 0;

/// Where a configuration of one pass is.
#[derive(Clone, Copy, Debug)]
pub(super) struct Here {
    pub(super) state: TransitionTableIndex,
    pub(super) flags: FlagsId,
    pub(super) output: OutputId,
}

/// A configuration within one pass.
#[derive(Clone, Copy, Debug)]
pub(in crate::prediction) struct Live {
    pub(super) here: Here,
    pub(super) weight: Weight,
}

impl Live {
    pub(in crate::prediction) fn weight(&self) -> Weight {
        self.weight
    }
}

/// Configurations, each at the lowest weight it was reached with, in the
/// order they were first reached.
#[derive(Default)]
pub(in crate::prediction) struct LiveSet {
    lives: Vec<Live>,
    index: IdMap<(TransitionTableIndex, FlagsId, OutputId), usize>,
}

impl LiveSet {
    /// True when `live` is new or lowers the weight its configuration had.
    pub(super) fn insert(&mut self, live: Live) -> bool {
        let key = (live.here.state, live.here.flags, live.here.output);
        let next = self.lives.len();
        let at = *self.index.entry(key).or_insert(next);
        if at == next {
            self.lives.push(live);
            return true;
        }
        let known = &mut self.lives[at];
        if known.weight <= live.weight {
            return false;
        }
        known.weight = live.weight;
        true
    }

    pub(in crate::prediction) fn as_slice(&self) -> &[Live] {
        &self.lives
    }

    pub(in crate::prediction) fn into_vec(self) -> Vec<Live> {
        self.lives
    }
}

/// One symbol written after its parent's.
#[derive(Clone, Copy, Debug)]
struct Written {
    parent: OutputId,
    symbol: SymbolNumber,
}

/// Outputs as a tree: a node is its parent with one more symbol, so a
/// configuration's output is one number however long it is. Each node is
/// made once, so two outputs are the same when their numbers are.
pub(super) struct Outputs {
    nodes: Vec<Written>,
    children: IdMap<(OutputId, SymbolNumber), OutputId>,
}

impl Outputs {
    pub(super) fn new() -> Outputs {
        Outputs {
            nodes: vec![Written {
                parent: EMPTY_OUTPUT,
                symbol: 0,
            }],
            children: IdMap::default(),
        }
    }

    pub(super) fn push(&mut self, parent: OutputId, symbol: SymbolNumber) -> OutputId {
        let next = self.nodes.len();
        let at = *self.children.entry((parent, symbol)).or_insert(next);
        if at == next {
            self.nodes.push(Written { parent, symbol });
        }
        at
    }

    pub(super) fn symbols(&self, at: OutputId) -> Vec<SymbolNumber> {
        let mut symbols = Vec::new();
        self.write(at, &mut symbols);
        symbols
    }

    /// Append the symbols of the output at `at` to `symbols`.
    fn write(&self, mut at: OutputId, symbols: &mut Vec<SymbolNumber>) {
        let start = symbols.len();
        while at != EMPTY_OUTPUT {
            let written = self.nodes[at];
            symbols.push(written.symbol);
            at = written.parent;
        }
        symbols[start..].reverse();
    }
}

/// Flag values, each set of them interned once, and the result of each flag
/// diacritic on each.
#[derive(Default)]
pub(in crate::prediction) struct Flags {
    values: Vec<Box<[FdValue]>>,
    ids: IdMap<Box<[FdValue]>, FlagsId>,
    applied: IdMap<(FlagsId, SymbolNumber), Option<FlagsId>>,
}

impl Flags {
    pub(in crate::prediction) fn intern(&mut self, values: Box<[FdValue]>) -> FlagsId {
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
    pub(in crate::prediction) fn after<T: TransducerTablesInterface>(
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
/// offers for it is found once per walker.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(in crate::prediction) enum Purpose {
    Tags,
    /// Tags of a run that only a boundary tag can end, and whether it holds
    /// its derivation tag.
    Bounded(bool),
    Prefixes,
    Letters,
}

impl Purpose {
    /// The marks of the symbols the purpose looks for, or of more.
    pub(super) fn marks(self) -> Marks {
        match self {
            Purpose::Tags | Purpose::Bounded(_) => TAG,
            Purpose::Prefixes => PREFIX,
            Purpose::Letters => LETTER,
        }
    }
}

/// What a set of seeds can read next for one purpose.
pub(in crate::prediction) struct Readers {
    /// The frontier of each seed, as the pass's lattice numbers it.
    pub(super) frontiers: Vec<u32>,
    /// Each symbol some seed can read, with that seed, in order of symbol
    /// and then of seed.
    pairs: Vec<(SymbolNumber, u32)>,
}

impl Readers {
    /// The symbols some seed can read. Flag values are kept, so each can
    /// be read.
    pub(in crate::prediction) fn symbols(&self) -> Vec<SymbolNumber> {
        let mut symbols: Vec<SymbolNumber> = self.pairs.iter().map(|&(symbol, _)| symbol).collect();
        symbols.dedup();
        symbols
    }

    /// The seeds that can read `symbol`, in order.
    pub(super) fn reading(&self, symbol: SymbolNumber) -> impl Iterator<Item = &u32> {
        let first = self.pairs.partition_point(|&(read, _)| read < symbol);
        self.pairs[first..]
            .iter()
            .take_while(move |&&(read, _)| read == symbol)
            .map(|(_, seed)| seed)
    }
}

pub(in crate::prediction) struct Pass<'m, 'a, T: TransducerTablesInterface> {
    pub(super) machine: &'m Machine<'a, T>,
    /// What each input symbol does to a run only a boundary tag can end.
    roles: &'m [Role],
    /// The marks each input symbol carries; none for a pass that is no
    /// walker's.
    marks: &'m [Marks],
    pub(in crate::prediction) budget: Budget,
    /// The configurations the pass may visit, as it started.
    pub(super) limit: usize,
    pub(super) outputs: Outputs,
    /// Taken by [`Pass::record`] when the pass is over.
    pub(super) lattice: RefCell<Lattice>,
    /// Where in the lattice each frontier the pass has gone through is.
    slots: IdMap<FrontierKey, u32>,
    /// Reused by every step, so that its tables are allocated once.
    pub(super) reached: Reached,
    memo: Box<Memo>,
    /// Where the memo goes back to when the pass ends, if it outlives it.
    shelf: Option<&'m Shelf>,
    /// The frontiers this pass found after the budget cut it, which may miss
    /// entries and so stay with this pass.
    cut: Vec<FrontierKey>,
}

impl<T: TransducerTablesInterface> Drop for Pass<'_, '_, T> {
    fn drop(&mut self) {
        let Some(shelf) = self.shelf else {
            return;
        };
        for key in std::mem::take(&mut self.cut) {
            self.memo.forget(&key);
        }
        shelf.put(std::mem::take(&mut self.memo));
    }
}

impl<'m, 'a, T: TransducerTablesInterface> Pass<'m, 'a, T> {
    pub(in crate::prediction) fn new(
        machine: &'m Machine<'a, T>,
        roles: &'m [Role],
        configurations: usize,
    ) -> Self {
        Pass::with(machine, (roles, &[]), configurations, Box::default(), None)
    }

    fn with(
        machine: &'m Machine<'a, T>,
        (roles, marks): (&'m [Role], &'m [Marks]),
        configurations: usize,
        memo: Box<Memo>,
        shelf: Option<&'m Shelf>,
    ) -> Self {
        Pass {
            machine,
            roles,
            marks,
            budget: Budget::new(configurations),
            limit: configurations,
            outputs: Outputs::new(),
            lattice: RefCell::default(),
            slots: IdMap::default(),
            reached: Reached::default(),
            memo,
            shelf,
            cut: Vec::new(),
        }
    }

    /// A walker's pass, which starts from the memo on `shelf` and puts it
    /// back when it ends.
    pub(in crate::prediction) fn kept(
        machine: &'m Machine<'a, T>,
        (roles, marks): (&'m [Role], &'m [Marks]),
        configurations: usize,
        shelf: &'m Shelf,
    ) -> Self {
        Pass::with(
            machine,
            (roles, marks),
            configurations,
            shelf.take(),
            Some(shelf),
        )
    }

    pub(in crate::prediction) fn start(&mut self) -> Live {
        let flags = self.memo.flags.intern(self.machine.neutral_flags());
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
                flags: self.memo.flags.intern(config.flags.clone()),
                output,
            },
            weight: config.weight,
        }
    }

    pub(in crate::prediction) fn materialize(&self, live: &Live) -> Configuration {
        Configuration {
            state: live.here.state,
            flags: self.memo.flags.values[live.here.flags].clone(),
            output: self.outputs.symbols(live.here.output),
            weight: live.weight,
            final_weight: self.machine.final_weight(live.here.state),
        }
    }

    pub(super) fn flag_values(&self, flags: FlagsId) -> &[FdValue] {
        &self.memo.flags.values[flags]
    }

    #[cfg(test)]
    pub(super) fn knows_reach(&self, state: TransitionTableIndex) -> bool {
        self.memo.reaches.knows(state)
    }

    /// How many sets of flag values the pass knows.
    pub(super) fn flag_count(&self) -> usize {
        self.memo.flags.values.len()
    }

    pub(in crate::prediction) fn final_weight(&self, live: &Live) -> Option<Weight> {
        self.machine.final_weight(live.here.state)
    }

    pub(in crate::prediction) fn render(&self, live: &Live) -> String {
        self.machine.render(&self.outputs.symbols(live.here.output))
    }

    fn outgoing(&mut self, state: TransitionTableIndex) -> Arc<[SymbolNumber]> {
        self.memo.reaches.outgoing(self.machine, state)
    }

    fn reach(&mut self, state: TransitionTableIndex) -> Arc<Reach> {
        self.memo.reaches.of(self.machine, state)
    }

    fn marks(&mut self, state: TransitionTableIndex) -> Marks {
        self.memo.reaches.marks(self.machine, self.marks, state)
    }

    pub(super) fn met(&mut self, want: Want, state: TransitionTableIndex) -> bool {
        match want {
            Want::Everything => true,
            Want::Final => self.marks(state) & FINAL != 0,
            Want::Symbol(symbol) => match self.marks.get(symbol as usize) {
                Some(&marks) if marks != 0 => self.marks(state) & marks != 0,
                Some(_) | None => self.reach(state).reads(symbol),
            },
            Want::Purpose(purpose) => self.marks(state) & purpose.marks() != 0,
            Want::Boundary(hold) => {
                self.memo
                    .reaches
                    .bounded(self.machine, self.roles, state, hold)
            }
        }
    }

    /// `live` over `arc`, with `flags` after it. What the arc writes is
    /// added to the output when the walk `writes`.
    fn along(&mut self, live: &Live, arc: &Edge, flags: FlagsId, writes: bool) -> Live {
        let output = match arc.output {
            Some(output) if writes => self.outputs.push(live.here.output, output),
            Some(_) | None => live.here.output,
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
    pub(in crate::prediction) fn explore(&mut self, seeds: &[Live], want: Want) -> Vec<Live> {
        self.explore_as(seeds, want, true)
    }

    /// [`Pass::explore`], keeping what each path writes only when it
    /// `writes`. Without it, the configurations are points, each at the
    /// lowest weight of a path to it.
    pub(super) fn explore_as(&mut self, seeds: &[Live], want: Want, writes: bool) -> Vec<Live> {
        let machine = self.machine;
        let mut set = LiveSet::default();
        let mut agenda: Vec<Live> = Vec::new();
        for seed in seeds {
            if self.met(want, seed.here.state) && set.insert(*seed) {
                agenda.push(*seed);
            }
        }
        'agenda: while let Some(live) = agenda.pop() {
            for arc in machine.epsilon_arcs(live.here.state) {
                let arc = machine.arc(arc);
                if !self.met(want, arc.target) {
                    continue;
                }
                let Some(flags) = self.memo.flags.after(machine, live.here.flags, arc.input) else {
                    continue;
                };
                if !self.budget.spend() {
                    break 'agenda;
                }
                let next = self.along(&live, &arc, flags, writes);
                if set.insert(next) {
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
    ) -> Arc<Frontier> {
        let key = (state, flags, purpose);
        if let Some(known) = self.memo.frontiers.get(&key) {
            return known.clone();
        }
        #[cfg(test)]
        {
            self.memo.computed += 1;
        }
        let root = Live {
            here: Here {
                state,
                flags,
                output: EMPTY_OUTPUT,
            },
            weight: 0.0,
        };
        let want = match purpose {
            Purpose::Bounded(hold) => Want::Boundary(hold),
            Purpose::Tags | Purpose::Prefixes | Purpose::Letters => Want::Purpose(purpose),
        };
        let machine = self.machine;
        let mut frontier = Frontier::default();
        for live in self.explore(&[root], want) {
            let at =
                u32::try_from(frontier.entries.len()).expect("a frontier holds under 2^32 entries");
            let before = frontier.readings.len();
            for &symbol in self.outgoing(live.here.state).iter() {
                if !wanted(symbol) {
                    continue;
                }
                for arc in machine.symbol_arcs(live.here.state, symbol) {
                    let arc = machine.arc(arc);
                    frontier.readings.push(Reading {
                        symbol,
                        entry: at,
                        target: arc.target,
                        written: arc.output,
                        weight: arc.weight,
                    });
                }
            }
            if frontier.readings.len() == before {
                continue;
            }
            let start = frontier.symbols.len();
            self.outputs.write(live.here.output, &mut frontier.symbols);
            let span = |at: usize| u32::try_from(at).expect("a frontier writes under 2^32 symbols");
            frontier.entries.push(Entry {
                flags: live.here.flags,
                suffix: span(start)..span(frontier.symbols.len()),
                weight: live.weight,
            });
        }
        frontier
            .readings
            .sort_by_key(|read| (read.symbol, read.entry));
        let frontier = Arc::new(frontier);
        if self.budget.configurations_cut {
            self.cut.push(key);
        }
        self.memo.entries += frontier.entries.len();
        self.memo.frontiers.insert(key, frontier.clone());
        frontier
    }

    /// What `seeds` can read next for `purpose`, `wanted` being its
    /// predicate.
    pub(in crate::prediction) fn readers(
        &mut self,
        seeds: &[Spot],
        purpose: Purpose,
        wanted: &dyn Fn(SymbolNumber) -> bool,
    ) -> Readers {
        let mut frontiers = Vec::with_capacity(seeds.len());
        let mut pairs = Vec::new();
        for (at, seed) in seeds.iter().enumerate() {
            let key = (seed.state, seed.flags, purpose);
            let slot = match self.slots.get(&key) {
                Some(&slot) => slot,
                None => {
                    let frontier = self.frontier(seed.state, seed.flags, purpose, wanted);
                    let slot = self.lattice.get_mut().frontier(frontier);
                    self.slots.insert(key, slot);
                    slot
                }
            };
            let at = u32::try_from(at).expect("a run holds under 2^32 seeds");
            let frontier = &self.lattice.get_mut().frontiers[slot as usize];
            pairs.extend(frontier.symbols_read().map(|symbol| (symbol, at)));
            frontiers.push(slot);
        }
        pairs.sort_unstable();
        Readers { frontiers, pairs }
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
                let next = self.along(live, &machine.arc(arc), live.here.flags, true);
                reached.insert(next);
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

    /// The final configurations `seeds` reach without reading a symbol,
    /// spelled out.
    pub(in crate::prediction) fn final_configurations(
        &mut self,
        seeds: &[Live],
    ) -> Vec<Configuration> {
        let finals = self.finals(seeds);
        finals.iter().map(|live| self.materialize(live)).collect()
    }

    /// Every configuration `seeds` reach without reading a symbol.
    pub(in crate::prediction) fn closure(&mut self, seeds: &[Live]) -> Vec<Live> {
        self.explore(seeds, Want::Everything)
    }

    /// True when some of `seeds` can read a symbol of `purpose`, `wanted`
    /// being its predicate, flag values included. The search goes deep
    /// first and stops at the first such configuration, so it does not walk
    /// what a state reaches in full.
    pub(in crate::prediction) fn can_read(
        &mut self,
        seeds: &[Live],
        purpose: Purpose,
        wanted: &dyn Fn(SymbolNumber) -> bool,
    ) -> bool {
        self.search(seeds, Want::Purpose(purpose), &|pass, state| {
            pass.outgoing(state).iter().any(|&symbol| wanted(symbol))
        })
    }

    /// True when some of `seeds` reach a final state without reading a
    /// symbol, flag values included.
    pub(super) fn can_end(&mut self, seeds: &[Live]) -> bool {
        self.search(seeds, Want::Final, &|pass, state| {
            pass.machine.final_weight(state).is_some()
        })
    }

    /// True when some point `seeds` reach without reading a symbol, through
    /// states `want` admits, is `found`. The search goes deep first and
    /// stops at the first, so it does not walk what a state reaches in
    /// full.
    fn search(
        &mut self,
        seeds: &[Live],
        want: Want,
        found: &dyn Fn(&mut Self, TransitionTableIndex) -> bool,
    ) -> bool {
        let machine = self.machine;
        let mut seen: IdSet<(TransitionTableIndex, FlagsId)> = IdSet::default();
        let mut agenda: Vec<(TransitionTableIndex, FlagsId)> = seeds
            .iter()
            .map(|seed| (seed.here.state, seed.here.flags))
            .collect();
        while let Some((state, flags)) = agenda.pop() {
            if !seen.insert((state, flags)) || !self.met(want, state) {
                continue;
            }
            if found(self, state) {
                return true;
            }
            for arc in machine.epsilon_arcs(state) {
                let arc = machine.arc(arc);
                let Some(flags) = self.memo.flags.after(machine, flags, arc.input) else {
                    continue;
                };
                if !self.budget.spend() {
                    return false;
                }
                agenda.push((arc.target, flags));
            }
        }
        false
    }

    /// The answer to `question` about the point `state` with `flags`, as
    /// the memo knows it or `ask` finds it. An answer the bound cut short
    /// is not kept.
    pub(super) fn answer(
        &mut self,
        state: TransitionTableIndex,
        flags: FlagsId,
        question: Question,
        ask: &dyn Fn(&mut Self, &[Live]) -> bool,
    ) -> bool {
        let key = (state, flags, question);
        if let Some(&known) = self.memo.answers.get(&key) {
            return known;
        }
        let point = Live {
            here: Here {
                state,
                flags,
                output: EMPTY_OUTPUT,
            },
            weight: 0.0,
        };
        let found = ask(self, &[point]);
        if !self.budget.configurations_cut {
            self.memo.answers.insert(key, found);
        }
        found
    }

    /// `live` with `operations` applied in order where it is, or nothing
    /// when one fails.
    // [spec:hfst:req:prediction.flags]
    pub(in crate::prediction) fn apply_all(
        &mut self,
        live: &Live,
        operations: &[FdOperation],
    ) -> Option<Live> {
        let mut values = self.memo.flags.values[live.here.flags].clone();
        for operation in operations {
            values = self.machine.apply(operation, &values)?;
        }
        let mut applied = *live;
        applied.here.flags = self.memo.flags.intern(values);
        Some(applied)
    }
}
