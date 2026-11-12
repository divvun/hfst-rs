//! The configurations a unit reaches, kept as the walk's record of how it
//! reached them and spelled out only when asked for. A walk offers many
//! units and a user picks one, so the configurations of the others, with
//! what each of them wrote, are never spelled out.

use std::ops::Range;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

use super::hash::IdMap;
use super::lattice::{NodeId, Origin, Step};
use super::machine::{Budget, Configuration};
use super::memo::Frontier;
use super::pass::{EMPTY_OUTPUT, FlagsId, Here, Live, LiveSet, Outputs, Pass};
use crate::hfst_flag_diacritics::FdValue;
use crate::transducer::{SymbolNumber, TransducerTablesInterface, TransitionTableIndex, Weight};

/// A configuration the walk started from, as the record keeps it.
#[derive(Clone, Debug)]
struct Given {
    state: TransitionTableIndex,
    flags: FlagsId,
    output: Vec<SymbolNumber>,
    weight: Weight,
    final_weight: Option<Weight>,
}

#[derive(Debug)]
enum Node {
    Given(Vec<Given>),
    Stepped {
        state: TransitionTableIndex,
        flags: FlagsId,
        final_weight: Option<Weight>,
        steps: Range<usize>,
    },
}

/// What one walk recorded of how it reached its spots, with the flag
/// values and final weights spelling them out needs, so that it can be
/// done without the walk.
#[derive(Debug)]
pub(in crate::prediction) struct Record {
    nodes: Vec<Node>,
    steps: Vec<Step>,
    frontiers: Vec<Arc<Frontier>>,
    flags: IdMap<FlagsId, Box<[FdValue]>>,
    /// The most configurations spelling out one set may make.
    limit: usize,
}

impl<T: TransducerTablesInterface> Pass<'_, '_, T> {
    /// The record of this pass, to spell out the configurations of its
    /// spots once it is over. The pass keeps no record after.
    pub(in crate::prediction) fn record(&self) -> Arc<Record> {
        let lattice = self.lattice.take();
        let mut used = vec![false; self.flag_count()];
        let mut mark = |flags: FlagsId| {
            if let Some(seen) = used.get_mut(flags) {
                *seen = true;
            }
        };
        let machine = self.machine;
        let nodes = lattice
            .origins
            .into_iter()
            .map(|origin| match origin {
                Origin::Given(lives) => Node::Given(
                    lives
                        .iter()
                        .map(|live| {
                            mark(live.here.flags);
                            Given {
                                state: live.here.state,
                                flags: live.here.flags,
                                output: self.outputs.symbols(live.here.output),
                                weight: live.weight,
                                final_weight: machine.final_weight(live.here.state),
                            }
                        })
                        .collect(),
                ),
                Origin::Stepped {
                    state,
                    flags,
                    steps,
                } => {
                    mark(flags);
                    Node::Stepped {
                        state,
                        flags,
                        final_weight: machine.final_weight(state),
                        steps,
                    }
                }
            })
            .collect();
        let flags = used
            .iter()
            .enumerate()
            .filter(|&(_, &seen)| seen)
            .map(|(flags, _)| (flags, self.flag_values(flags).into()))
            .collect();
        Arc::new(Record {
            nodes,
            steps: lattice.steps,
            frontiers: lattice.frontiers,
            flags,
            limit: self.limit,
        })
    }
}

/// Spells out the configurations of a record's nodes.
struct Speller<'r> {
    record: &'r Record,
    outputs: Outputs,
    budget: Budget,
    spelled: IdMap<NodeId, Rc<[Live]>>,
    final_weights: IdMap<TransitionTableIndex, Option<Weight>>,
}

impl Speller<'_> {
    /// Every configuration at `node`, each at the lowest weight it was
    /// reached with. A step adds the weight of the entry and then that of
    /// the arc to each configuration it starts from, as a walk over
    /// configurations does.
    fn spell(&mut self, node: NodeId) -> Rc<[Live]> {
        if let Some(known) = self.spelled.get(&node) {
            return known.clone();
        }
        let record = self.record;
        let mut set = LiveSet::default();
        match record.nodes.get(node) {
            None => {}
            Some(Node::Given(given)) => {
                for seed in given {
                    self.final_weights.insert(seed.state, seed.final_weight);
                    let output = seed
                        .output
                        .iter()
                        .fold(EMPTY_OUTPUT, |at, &symbol| self.outputs.push(at, symbol));
                    let here = Here {
                        state: seed.state,
                        flags: seed.flags,
                        output,
                    };
                    set.insert(Live {
                        here,
                        weight: seed.weight,
                    });
                }
            }
            Some(Node::Stepped {
                state,
                flags,
                final_weight,
                steps,
            }) => {
                self.final_weights.insert(*state, *final_weight);
                let steps = record.steps.get(steps.clone()).unwrap_or(&[]);
                'steps: for step in steps {
                    let from = self.spell(step.from);
                    let frontier = &record.frontiers[step.frontier as usize];
                    let Some(entry) = frontier.entries.get(step.entry as usize) else {
                        continue;
                    };
                    let suffix = frontier.suffix(entry);
                    for live in from.iter() {
                        if !self.budget.spend() {
                            break 'steps;
                        }
                        let mut output = suffix.iter().fold(live.here.output, |at, &symbol| {
                            self.outputs.push(at, symbol)
                        });
                        if let Some(written) = step.written {
                            output = self.outputs.push(output, written);
                        }
                        let here = Here {
                            state: *state,
                            flags: *flags,
                            output,
                        };
                        let weight = live.weight + entry.weight + step.weight;
                        set.insert(Live { here, weight });
                    }
                }
            }
        }
        let spelled: Rc<[Live]> = set.into_vec().into();
        self.spelled.insert(node, spelled.clone());
        spelled
    }

    fn configuration(&self, live: &Live) -> Configuration {
        Configuration {
            state: live.here.state,
            flags: self
                .record
                .flags
                .get(&live.here.flags)
                .cloned()
                .unwrap_or_default(),
            output: self.outputs.symbols(live.here.output),
            weight: live.weight,
            final_weight: self.final_weights.get(&live.here.state).copied().flatten(),
        }
    }
}

#[derive(Debug)]
enum Source {
    Spelled,
    Recorded {
        record: Arc<Record>,
        nodes: Vec<NodeId>,
    },
}

/// The configurations right after a unit, or a word's last unit, spelled
/// out when first asked for.
#[derive(Debug)]
struct SeedsInner {
    source: Source,
    spelled: OnceLock<(Vec<Configuration>, bool)>,
}

#[derive(Clone, Debug)]
pub(in crate::prediction) struct Seeds(Arc<SeedsInner>);

impl Seeds {
    pub(in crate::prediction) fn spelled(configurations: Vec<Configuration>) -> Seeds {
        Seeds(Arc::new(SeedsInner {
            source: Source::Spelled,
            spelled: OnceLock::from((configurations, false)),
        }))
    }

    pub(in crate::prediction) fn recorded(record: Arc<Record>, nodes: Vec<NodeId>) -> Seeds {
        Seeds(Arc::new(SeedsInner {
            source: Source::Recorded { record, nodes },
            spelled: OnceLock::new(),
        }))
    }

    /// The configurations, and whether a bound cut spelling them out.
    pub(in crate::prediction) fn get(&self) -> (&[Configuration], bool) {
        let (configurations, cut) = self.0.spelled.get_or_init(|| match &self.0.source {
            Source::Spelled => (Vec::new(), false),
            Source::Recorded { record, nodes } => spell_nodes(record, nodes),
        });
        (configurations, *cut)
    }

    #[cfg(test)]
    pub(in crate::prediction) fn is_spelled(&self) -> bool {
        self.0.spelled.get().is_some()
    }

    pub(in crate::prediction) fn iter(&self) -> std::slice::Iter<'_, Configuration> {
        self.get().0.iter()
    }
}

fn spell_nodes(record: &Record, nodes: &[NodeId]) -> (Vec<Configuration>, bool) {
    let mut speller = Speller {
        record,
        outputs: Outputs::new(),
        budget: Budget::new(record.limit),
        spelled: IdMap::default(),
        final_weights: IdMap::default(),
    };
    let mut set = LiveSet::default();
    for &node in nodes {
        for &live in speller.spell(node).iter() {
            set.insert(live);
        }
    }
    let configurations = set
        .as_slice()
        .iter()
        .map(|live| speller.configuration(live))
        .collect();
    (configurations, speller.budget.configurations_cut)
}

/// The final configurations some seeds reach without reading a symbol,
/// found when first asked for. Whether there are any is known at once.
#[derive(Clone, Debug)]
pub(in crate::prediction) struct Finals {
    seeds: Seeds,
    any: bool,
    found: Arc<OnceLock<Vec<Configuration>>>,
}

impl Finals {
    pub(in crate::prediction) fn of(seeds: Seeds, any: bool) -> Finals {
        Finals {
            seeds,
            any,
            found: Arc::new(OnceLock::new()),
        }
    }

    pub(in crate::prediction) fn found(seeds: Seeds, finals: Vec<Configuration>) -> Finals {
        Finals {
            seeds,
            any: !finals.is_empty(),
            found: Arc::new(OnceLock::from(finals)),
        }
    }

    pub(in crate::prediction) fn is_empty(&self) -> bool {
        !self.any
    }

    /// The final configurations, finding them with `find` if they are not
    /// known yet.
    pub(in crate::prediction) fn get_or(
        &self,
        find: impl FnOnce(&Seeds) -> Vec<Configuration>,
    ) -> &[Configuration] {
        if !self.any {
            return &[];
        }
        self.found.get_or_init(|| find(&self.seeds))
    }
}
