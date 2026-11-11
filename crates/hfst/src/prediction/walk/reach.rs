//! What each state of a transducer reaches before a walk commits to a path:
//! the input symbols it can read next without reading another, whether it
//! can end there, and whether a run of tags from it can still end at a
//! boundary tag. A walk asks before it follows an arc, so it follows only
//! arcs that lead to what it is after.

use std::rc::Rc;

use super::hash::{IdMap, IdSet};
use super::machine::Machine;
use crate::transducer::{SymbolNumber, TransducerTablesInterface, TransitionTableIndex};

/// What a state reaches without reading an input symbol, flag values left
/// aside, so it holds whatever a path from the state may hold.
#[derive(Debug, Default)]
pub(in crate::prediction) struct Reach {
    /// The input symbols it can read next, in order.
    symbols: Vec<SymbolNumber>,
    can_end: bool,
}

impl Reach {
    pub(in crate::prediction) fn can_end(&self) -> bool {
        self.can_end
    }

    pub(in crate::prediction) fn reads(&self, symbol: SymbolNumber) -> bool {
        self.symbols.binary_search(&symbol).is_ok()
    }

    pub(in crate::prediction) fn reads_any(&self, wanted: &dyn Fn(SymbolNumber) -> bool) -> bool {
        self.symbols.iter().any(|&symbol| wanted(symbol))
    }
}

/// What an input symbol does to a run of tags that only a boundary tag can
/// still end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::prediction) enum Role {
    /// The run can go on through it.
    Through,
    /// It ends the run.
    Boundary,
    /// A run holds at most one derivation tag.
    Derivation,
    /// Ends a run that holds no derivation tag yet, and only that one.
    DerivationBoundary,
    /// The run cannot take it: a letter or no tag.
    Stop,
}

/// What an epsilon walk is after: it enters only states that can lead to it.
#[derive(Clone, Copy)]
pub(in crate::prediction) enum Want<'w> {
    Everything,
    Final,
    Symbol(SymbolNumber),
    Reading(&'w dyn Fn(SymbolNumber) -> bool),
    /// A boundary tag, at the end of a run of tags that go through. True
    /// when the run already holds its derivation tag.
    Boundary(bool),
}

/// A graph over states whose components settle a value each.
trait Graph {
    type Value: Clone;
    fn edges(&mut self, state: TransitionTableIndex) -> Vec<TransitionTableIndex>;
    /// The value of a component, from its members and the components it
    /// leads to, which are settled.
    fn value(
        &mut self,
        members: &[TransitionTableIndex],
        settled: &IdMap<TransitionTableIndex, Self::Value>,
    ) -> Self::Value;
}

fn outgoing_of<T: TransducerTablesInterface>(
    outgoing: &mut IdMap<TransitionTableIndex, Rc<[SymbolNumber]>>,
    machine: &Machine<'_, T>,
    state: TransitionTableIndex,
) -> Rc<[SymbolNumber]> {
    outgoing
        .entry(state)
        .or_insert_with(|| machine.symbols_from(state).into())
        .clone()
}

fn epsilon_targets<T: TransducerTablesInterface>(
    machine: &Machine<'_, T>,
    state: TransitionTableIndex,
) -> Vec<TransitionTableIndex> {
    machine
        .epsilon_arcs(state)
        .into_iter()
        .map(|arc| machine.arc(arc).target)
        .collect()
}

/// Epsilon and flag arcs, valued by what they lead to reading.
struct Epsilons<'g, 'a, T: TransducerTablesInterface> {
    machine: &'g Machine<'a, T>,
    outgoing: &'g mut IdMap<TransitionTableIndex, Rc<[SymbolNumber]>>,
}

impl<T: TransducerTablesInterface> Graph for Epsilons<'_, '_, T> {
    type Value = Rc<Reach>;

    fn edges(&mut self, state: TransitionTableIndex) -> Vec<TransitionTableIndex> {
        epsilon_targets(self.machine, state)
    }

    fn value(
        &mut self,
        members: &[TransitionTableIndex],
        settled: &IdMap<TransitionTableIndex, Rc<Reach>>,
    ) -> Rc<Reach> {
        let mut symbols: Vec<SymbolNumber> = Vec::new();
        let mut can_end = false;
        let mut after: Vec<Rc<Reach>> = Vec::new();
        for &member in members {
            symbols.extend(outgoing_of(self.outgoing, self.machine, member).iter());
            can_end |= self.machine.final_weight(member).is_some();
            for target in epsilon_targets(self.machine, member) {
                if let Some(reach) = settled.get(&target).filter(|_| !members.contains(&target)) {
                    after.push(reach.clone());
                }
            }
        }
        if symbols.is_empty() && !can_end && after.len() == 1 {
            return after.swap_remove(0);
        }
        for reach in &after {
            symbols.extend(&reach.symbols);
            can_end |= reach.can_end;
        }
        symbols.sort_unstable();
        symbols.dedup();
        Rc::new(Reach { symbols, can_end })
    }
}

/// Epsilon and flag arcs and the arcs of tags a run goes on through, valued
/// by whether they lead to a boundary tag.
struct Tags<'g, 'a, T: TransducerTablesInterface> {
    machine: &'g Machine<'a, T>,
    outgoing: &'g mut IdMap<TransitionTableIndex, Rc<[SymbolNumber]>>,
    roles: &'g [Role],
    /// For a run that can still take a derivation tag, the settled values
    /// of runs that hold one, where a derivation arc leads; none for a run
    /// that holds one.
    held: Option<&'g mut IdMap<TransitionTableIndex, bool>>,
}

impl<T: TransducerTablesInterface> Tags<'_, '_, T> {
    fn role(&self, symbol: SymbolNumber) -> Role {
        self.roles
            .get(symbol as usize)
            .copied()
            .unwrap_or(Role::Stop)
    }

    fn ends(&self, symbol: SymbolNumber) -> bool {
        match self.role(symbol) {
            Role::Boundary => true,
            Role::DerivationBoundary => self.held.is_some(),
            Role::Through | Role::Derivation | Role::Stop => false,
        }
    }

    /// True when a run that takes its derivation tag into `target` can
    /// still end at a boundary tag.
    fn after_derivation(&mut self, target: TransitionTableIndex) -> bool {
        let Some(held) = self.held.as_deref_mut() else {
            return false;
        };
        if !held.contains_key(&target) {
            let mut holding = Tags {
                machine: self.machine,
                outgoing: &mut *self.outgoing,
                roles: self.roles,
                held: None,
            };
            settle(&mut holding, held, target);
        }
        held.get(&target) == Some(&true)
    }
}

impl<T: TransducerTablesInterface> Graph for Tags<'_, '_, T> {
    type Value = bool;

    fn edges(&mut self, state: TransitionTableIndex) -> Vec<TransitionTableIndex> {
        let mut targets = epsilon_targets(self.machine, state);
        for &symbol in outgoing_of(self.outgoing, self.machine, state).iter() {
            if self.role(symbol) == Role::Through {
                let arcs = self.machine.symbol_arcs(state, symbol);
                targets.extend(arcs.into_iter().map(|arc| self.machine.arc(arc).target));
            }
        }
        targets
    }

    fn value(
        &mut self,
        members: &[TransitionTableIndex],
        settled: &IdMap<TransitionTableIndex, bool>,
    ) -> bool {
        for &member in members {
            let own = outgoing_of(self.outgoing, self.machine, member);
            if own.iter().any(|&symbol| self.ends(symbol)) {
                return true;
            }
            for &symbol in own.iter() {
                if self.role(symbol) != Role::Derivation || self.held.is_none() {
                    continue;
                }
                for arc in self.machine.symbol_arcs(member, symbol) {
                    if self.after_derivation(self.machine.arc(arc).target) {
                        return true;
                    }
                }
            }
            let leads = self
                .edges(member)
                .into_iter()
                .any(|target| !members.contains(&target) && settled.get(&target) == Some(&true));
            if leads {
                return true;
            }
        }
        false
    }
}

struct Frame {
    state: TransitionTableIndex,
    targets: Vec<TransitionTableIndex>,
    next: usize,
}

/// The order a state was found in and the lowest order it reaches back to.
struct Found {
    order: usize,
    low: usize,
}

/// One search: the states found, and those whose component is still open.
#[derive(Default)]
struct Search {
    found: IdMap<TransitionTableIndex, Found>,
    open: Vec<TransitionTableIndex>,
    on_open: IdSet<TransitionTableIndex>,
    frames: Vec<Frame>,
}

/// Settle `root` and everything it leads to, a strongly connected component
/// at a time, so each state is looked at once whatever cycles there are.
fn settle<G: Graph>(
    graph: &mut G,
    settled: &mut IdMap<TransitionTableIndex, G::Value>,
    root: TransitionTableIndex,
) {
    let mut search = Search::default();
    discover(graph, root, &mut search);
    while let Some(frame) = search.frames.last_mut() {
        let state = frame.state;
        if let Some(&target) = frame.targets.get(frame.next) {
            frame.next += 1;
            if settled.contains_key(&target) {
                continue;
            }
            match search.found.get(&target).map(|seen| seen.order) {
                None => discover(graph, target, &mut search),
                Some(order) if search.on_open.contains(&target) => {
                    if let Some(current) = search.found.get_mut(&state) {
                        current.low = current.low.min(order);
                    }
                }
                Some(_) => {}
            }
            continue;
        }
        search.frames.pop();
        close_component(graph, settled, state, &mut search);
    }
}

fn discover<G: Graph>(graph: &mut G, state: TransitionTableIndex, search: &mut Search) {
    let order = search.found.len();
    search.found.insert(state, Found { order, low: order });
    search.open.push(state);
    search.on_open.insert(state);
    search.frames.push(Frame {
        state,
        targets: graph.edges(state),
        next: 0,
    });
}

/// After the search leaves `state`: pass its low order up, and when it
/// roots a component, settle every state in it.
fn close_component<G: Graph>(
    graph: &mut G,
    settled: &mut IdMap<TransitionTableIndex, G::Value>,
    state: TransitionTableIndex,
    search: &mut Search,
) {
    let Some(&Found { order, low }) = search.found.get(&state) else {
        return;
    };
    if let Some(parent) = search.frames.last()
        && let Some(above) = search.found.get_mut(&parent.state)
    {
        above.low = above.low.min(low);
    }
    if low != order {
        return;
    }
    let mut members = Vec::new();
    while let Some(member) = search.open.pop() {
        search.on_open.remove(&member);
        members.push(member);
        if member == state {
            break;
        }
    }
    let value = graph.value(&members, settled);
    for member in members {
        settled.insert(member, value.clone());
    }
}

/// What the states one walk has asked about reach, and their own input
/// symbols.
#[derive(Default)]
pub(in crate::prediction) struct Reaches {
    reaches: IdMap<TransitionTableIndex, Rc<Reach>>,
    /// Whether a run that holds its derivation tag can end at a boundary.
    held: IdMap<TransitionTableIndex, bool>,
    /// Whether a run that can still take one can.
    free: IdMap<TransitionTableIndex, bool>,
    outgoing: IdMap<TransitionTableIndex, Rc<[SymbolNumber]>>,
}

impl Reaches {
    /// The input symbols, neither epsilon nor flags, that `state` itself
    /// has arcs for.
    pub(in crate::prediction) fn outgoing<T: TransducerTablesInterface>(
        &mut self,
        machine: &Machine<'_, T>,
        state: TransitionTableIndex,
    ) -> Rc<[SymbolNumber]> {
        outgoing_of(&mut self.outgoing, machine, state)
    }

    /// What `root` reaches over epsilon and flag arcs.
    pub(in crate::prediction) fn of<T: TransducerTablesInterface>(
        &mut self,
        machine: &Machine<'_, T>,
        root: TransitionTableIndex,
    ) -> Rc<Reach> {
        if !self.reaches.contains_key(&root) {
            let Reaches {
                reaches, outgoing, ..
            } = self;
            settle(&mut Epsilons { machine, outgoing }, reaches, root);
        }
        self.reaches.get(&root).cloned().unwrap_or_default()
    }

    /// True when a run of tags from `root` can end at a boundary tag, going
    /// through only tags `roles` lets it, and through a derivation tag only
    /// when it does not `hold` one already.
    pub(in crate::prediction) fn bounded<T: TransducerTablesInterface>(
        &mut self,
        machine: &Machine<'_, T>,
        roles: &[Role],
        root: TransitionTableIndex,
        hold: bool,
    ) -> bool {
        let Reaches {
            held,
            free,
            outgoing,
            ..
        } = self;
        let (settled, mut tags) = if hold {
            let tags = Tags {
                machine,
                outgoing,
                roles,
                held: None,
            };
            (held, tags)
        } else {
            let tags = Tags {
                machine,
                outgoing,
                roles,
                held: Some(held),
            };
            (free, tags)
        };
        if !settled.contains_key(&root) {
            settle(&mut tags, settled, root);
        }
        settled.get(&root).copied().unwrap_or(false)
    }
}
