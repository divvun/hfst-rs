//! What each state of a transducer reaches without reading an input
//! symbol: the input symbols it can read next and whether it can end there.
//! A walk asks before it follows an epsilon arc, so it follows only arcs
//! that lead to what it is after.

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

/// What an epsilon walk is after: it enters only states that can lead to it.
#[derive(Clone, Copy)]
pub(in crate::prediction) enum Want<'w> {
    Everything,
    Final,
    Symbol(SymbolNumber),
    Reading(&'w dyn Fn(SymbolNumber) -> bool),
}

impl Want<'_> {
    pub(in crate::prediction) fn met_by(&self, reach: &Reach) -> bool {
        match self {
            Want::Everything => true,
            Want::Final => reach.can_end,
            Want::Symbol(symbol) => reach.symbols.binary_search(symbol).is_ok(),
            Want::Reading(wanted) => reach.symbols.iter().any(|&symbol| wanted(symbol)),
        }
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

/// What the states one walk has asked about reach, and their own input
/// symbols.
#[derive(Default)]
pub(in crate::prediction) struct Reaches {
    reaches: IdMap<TransitionTableIndex, Rc<Reach>>,
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
        self.outgoing
            .entry(state)
            .or_insert_with(|| machine.symbols_from(state).into())
            .clone()
    }

    /// What `root` reaches. The states it reaches over epsilon and flag arcs
    /// are settled a strongly connected component at a time, so each is
    /// looked at once whatever cycles there are.
    pub(in crate::prediction) fn of<T: TransducerTablesInterface>(
        &mut self,
        machine: &Machine<'_, T>,
        root: TransitionTableIndex,
    ) -> Rc<Reach> {
        if let Some(known) = self.reaches.get(&root) {
            return known.clone();
        }
        let mut search = Search::default();
        discover(machine, root, &mut search);
        while let Some(frame) = search.frames.last_mut() {
            let state = frame.state;
            if let Some(&target) = frame.targets.get(frame.next) {
                frame.next += 1;
                self.visit(machine, state, target, &mut search);
                continue;
            }
            search.frames.pop();
            self.close_component(machine, state, &mut search);
        }
        self.reaches.get(&root).cloned().unwrap_or_default()
    }

    fn visit<T: TransducerTablesInterface>(
        &self,
        machine: &Machine<'_, T>,
        state: TransitionTableIndex,
        target: TransitionTableIndex,
        search: &mut Search,
    ) {
        if self.reaches.contains_key(&target) {
            return;
        }
        let Some(order) = search.found.get(&target).map(|seen| seen.order) else {
            discover(machine, target, search);
            return;
        };
        if search.on_open.contains(&target)
            && let Some(current) = search.found.get_mut(&state)
        {
            current.low = current.low.min(order);
        }
    }

    /// After the search leaves `state`: pass its low order up, and when it
    /// roots a component, settle what every state in it reaches.
    fn close_component<T: TransducerTablesInterface>(
        &mut self,
        machine: &Machine<'_, T>,
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
        let reach = self.combine(machine, &members);
        for member in members {
            self.reaches.insert(member, reach.clone());
        }
    }

    /// What one component reaches: its own symbols and finality and those
    /// of the components it leads to, which are settled.
    fn combine<T: TransducerTablesInterface>(
        &mut self,
        machine: &Machine<'_, T>,
        members: &[TransitionTableIndex],
    ) -> Rc<Reach> {
        let mut symbols: Vec<SymbolNumber> = Vec::new();
        let mut can_end = false;
        let mut after: Vec<Rc<Reach>> = Vec::new();
        for &member in members {
            symbols.extend(self.outgoing(machine, member).iter());
            can_end |= machine.final_weight(member).is_some();
            for target in epsilon_targets(machine, member) {
                if members.contains(&target) {
                    continue;
                }
                if let Some(reach) = self.reaches.get(&target) {
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

fn discover<T: TransducerTablesInterface>(
    machine: &Machine<'_, T>,
    state: TransitionTableIndex,
    search: &mut Search,
) {
    let order = search.found.len();
    search.found.insert(state, Found { order, low: order });
    search.open.push(state);
    search.on_open.insert(state);
    search.frames.push(Frame {
        state,
        targets: epsilon_targets(machine, state),
        next: 0,
    });
}
