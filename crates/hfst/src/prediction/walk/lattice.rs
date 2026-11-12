//! Drawing units over points rather than configurations. Which units the
//! generator accepts, and at what weight, depends only on the state and
//! flag values of each configuration and the lowest weight among those that
//! share them, never on what they wrote. A generator that writes a morpheme
//! before its tags makes many configurations at one point, one for each
//! spelling of each morpheme so far, so a walk steps once per point and
//! records how it got there. The configurations of a point, with what they
//! wrote, are spelled out from that record only for the units offered.

use std::ops::Range;
use std::sync::Arc;

use super::hash::IdMap;
use super::memo::{Frontier, Question};
use super::pass::{EMPTY_OUTPUT, FlagsId, Here, Live, Pass, Purpose, Readers};
use super::reach::Want;
use crate::transducer::{SymbolNumber, TransducerTablesInterface, TransitionTableIndex, Weight};

pub(in crate::prediction) type NodeId = usize;

/// The configurations of one pass at one state with one set of flag
/// values, at the lowest weight among them.
#[derive(Clone, Copy, Debug)]
pub(in crate::prediction) struct Spot {
    pub(super) state: TransitionTableIndex,
    pub(super) flags: FlagsId,
    pub(super) weight: Weight,
    /// Where the lattice records how they were reached.
    pub(super) node: NodeId,
}

impl Spot {
    pub(in crate::prediction) fn weight(&self) -> Weight {
        self.weight
    }

    /// The spot as a configuration that has written nothing.
    pub(super) fn bare(&self) -> Live {
        Live {
            here: Here {
                state: self.state,
                flags: self.flags,
                output: EMPTY_OUTPUT,
            },
            weight: self.weight,
        }
    }
}

/// A step that reached a node: from each configuration of an earlier node,
/// along a frontier entry and over an arc that reads a symbol.
#[derive(Clone, Copy, Debug)]
pub(super) struct Step {
    pub(super) from: NodeId,
    pub(super) frontier: u32,
    pub(super) entry: u32,
    pub(super) written: Option<SymbolNumber>,
    pub(super) weight: Weight,
}

#[derive(Debug)]
pub(super) enum Origin {
    /// Configurations the walk started from.
    Given(Arc<[Live]>),
    /// Configurations steps reached, all at one state with the flag values
    /// of the entries they went through.
    Stepped {
        state: TransitionTableIndex,
        flags: FlagsId,
        steps: Range<usize>,
    },
}

/// How each spot of one pass was reached.
#[derive(Debug, Default)]
pub(in crate::prediction) struct Lattice {
    pub(super) origins: Vec<Origin>,
    /// The steps of every node, a node's together.
    pub(super) steps: Vec<Step>,
    /// The frontiers steps went through.
    pub(super) frontiers: Vec<Arc<Frontier>>,
}

impl Lattice {
    fn add(&mut self, origin: Origin) -> NodeId {
        self.origins.push(origin);
        self.origins.len() - 1
    }

    pub(super) fn frontier(&mut self, frontier: Arc<Frontier>) -> u32 {
        self.frontiers.push(frontier);
        u32::try_from(self.frontiers.len() - 1).expect("a pass goes through under 2^32 frontiers")
    }
}

/// Spots being reached by one step, in the order first reached, and the
/// steps that reach each.
#[derive(Default)]
pub(super) struct Reached {
    spots: Vec<Spot>,
    index: IdMap<(TransitionTableIndex, FlagsId), usize>,
    steps: Vec<(usize, Step)>,
}

impl Reached {
    fn add(&mut self, state: TransitionTableIndex, flags: FlagsId, weight: Weight, step: Step) {
        let next = self.spots.len();
        let at = *self.index.entry((state, flags)).or_insert(next);
        if at == next {
            self.spots.push(Spot {
                state,
                flags,
                weight,
                node: 0,
            });
        } else {
            let spot = &mut self.spots[at];
            spot.weight = spot.weight.min(weight);
        }
        self.steps.push((at, step));
    }

    /// Record the spots as nodes of `lattice`, each with its steps, and
    /// clear them.
    fn drain_into(&mut self, lattice: &mut Lattice) -> Vec<Spot> {
        let mut starts = vec![0usize; self.spots.len() + 1];
        for &(at, _) in &self.steps {
            starts[at + 1] += 1;
        }
        for at in 1..starts.len() {
            starts[at] += starts[at - 1];
        }
        let base = lattice.steps.len();
        let mut placed = starts.clone();
        let filler = self.steps.first().map(|&(_, step)| step);
        if let Some(filler) = filler {
            lattice.steps.resize(base + self.steps.len(), filler);
        }
        for &(at, step) in &self.steps {
            lattice.steps[base + placed[at]] = step;
            placed[at] += 1;
        }
        for (at, spot) in self.spots.iter_mut().enumerate() {
            spot.node = lattice.add(Origin::Stepped {
                state: spot.state,
                flags: spot.flags,
                steps: base + starts[at]..base + starts[at + 1],
            });
        }
        self.index.clear();
        self.steps.clear();
        std::mem::take(&mut self.spots)
    }
}

impl<'m, 'a, T: TransducerTablesInterface> Pass<'m, 'a, T> {
    /// `lives` gathered by state and flag values, in the order each point
    /// first comes.
    pub(in crate::prediction) fn spots(&mut self, lives: &[Live]) -> Vec<Spot> {
        let mut index: IdMap<(TransitionTableIndex, FlagsId), usize> = IdMap::default();
        let mut groups: Vec<(Spot, Vec<Live>)> = Vec::new();
        for live in lives {
            let point = (live.here.state, live.here.flags);
            let next = groups.len();
            let at = *index.entry(point).or_insert(next);
            if at == next {
                let spot = Spot {
                    state: point.0,
                    flags: point.1,
                    weight: live.weight,
                    node: 0,
                };
                groups.push((spot, Vec::new()));
            }
            let (spot, members) = &mut groups[at];
            spot.weight = spot.weight.min(live.weight);
            members.push(*live);
        }
        let lattice = self.lattice.get_mut();
        groups
            .into_iter()
            .map(|(mut spot, members)| {
                spot.node = lattice.add(Origin::Given(members.into()));
                spot
            })
            .collect()
    }

    /// The spots `seeds` reach by reading `symbol`, `readers` being what
    /// they can read.
    pub(in crate::prediction) fn step_spots(
        &mut self,
        seeds: &[Spot],
        readers: &Readers,
        symbol: SymbolNumber,
    ) -> Vec<Spot> {
        let lattice = self.lattice.get_mut();
        let reached = &mut self.reached;
        'seeds: for &at in readers.reading(symbol) {
            let seed = seeds[at as usize];
            let frontier = readers.frontiers[at as usize];
            for (read, entry) in lattice.frontiers[frontier as usize].reading(symbol) {
                if !self.budget.spend() {
                    break 'seeds;
                }
                let step = Step {
                    from: seed.node,
                    frontier,
                    entry: read.entry,
                    written: read.written,
                    weight: read.weight,
                };
                let weight = seed.weight + entry.weight + read.weight;
                reached.add(read.target, entry.flags, weight, step);
            }
        }
        reached.drain_into(lattice)
    }

    /// The final points `spots` reach without reading a symbol, each at the
    /// lowest weight of a path to it.
    pub(in crate::prediction) fn final_points(&mut self, spots: &[Spot]) -> Vec<Live> {
        let bare: Vec<Live> = spots.iter().map(Spot::bare).collect();
        let machine = self.machine;
        self.explore_as(&bare, Want::Final, false)
            .into_iter()
            .filter(|live| machine.final_weight(live.here.state).is_some())
            .collect()
    }

    /// True when some of `spots` can read a symbol of `purpose`, `wanted`
    /// being its predicate, flag values included.
    pub(in crate::prediction) fn reads(
        &mut self,
        spots: &[Spot],
        purpose: Purpose,
        wanted: &dyn Fn(SymbolNumber) -> bool,
    ) -> bool {
        spots.iter().any(|spot| {
            self.answer(
                spot.state,
                spot.flags,
                Question::Reads(purpose),
                &|pass, point| pass.can_read(point, purpose, wanted),
            )
        })
    }

    /// True when some of `spots` reach a final state without reading a
    /// symbol, flag values included.
    pub(in crate::prediction) fn ends(&mut self, spots: &[Spot]) -> bool {
        spots.iter().any(|spot| {
            self.answer(spot.state, spot.flags, Question::Ends, &|pass, point| {
                pass.can_end(point)
            })
        })
    }

    /// True when a run of tags from some of `spots` can still end at a
    /// boundary tag, `hold` saying whether it holds its derivation tag.
    pub(in crate::prediction) fn bounded(&mut self, spots: &[Spot], hold: bool) -> bool {
        spots
            .iter()
            .any(|spot| self.met(Want::Boundary(hold), spot.state))
    }
}
