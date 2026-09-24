//! The reachable graph of a table pair and the properties derived from it.

use super::*;

/// One arc of a [`ReachableGraph`], with its target renumbered densely.
#[derive(Clone, Copy)]
struct GraphArc {
    input: SymbolNumber,
    output: SymbolNumber,
    weight: Weight,
    target: u32,
}

/// The reachable graph of an optimized-lookup table pair, renumbered from the
/// start state in discovery order: `arcs[starts[s]..starts[s + 1]]` are state
/// `s`'s outgoing arcs. Flat rather than per-state vectors because the nets
/// this runs over carry millions of arcs.
struct ReachableGraph {
    starts: Vec<u32>,
    arcs: Vec<GraphArc>,
}

/// Dense state numbers for optimized-lookup addresses. The address space is two
/// disjoint ranges — the index table below [`TRANSITION_TARGET_TABLE_START`],
/// the transition table above — so a pair of direct lookup tables answers in
/// O(1) where a map of a million states would dominate the walk.
struct StateIds {
    index_side: Vec<u32>,
    transition_side: Vec<u32>,
}

impl StateIds {
    const UNASSIGNED: u32 = u32::MAX;

    fn new(index_len: TransitionTableIndex, transition_len: TransitionTableIndex) -> Self {
        StateIds {
            index_side: vec![Self::UNASSIGNED; index_len as usize],
            transition_side: vec![Self::UNASSIGNED; transition_len as usize],
        }
    }

    /// The (side, offset) split of an address, or `None` when it addresses
    /// neither table. Load-time target validation
    /// ([`validate_transition_target`]) rules that out for anything read off
    /// disk, and the conversions only emit in-range targets, so `None` means a
    /// state that cannot be entered — treated as unreachable rather than
    /// panicking mid-walk.
    fn slot(&mut self, address: TransitionTableIndex) -> Option<&mut u32> {
        if indexes_transition_index_table(address) {
            self.index_side.get_mut(address as usize)
        } else {
            self.transition_side
                .get_mut((address - TRANSITION_TARGET_TABLE_START) as usize)
        }
    }

    /// The dense number of `address`, taking `fresh` if it had none. The
    /// second element says whether `fresh` was taken, so the caller knows to
    /// enqueue the state. `None` for an address in neither table.
    fn intern(&mut self, address: TransitionTableIndex, fresh: u32) -> Option<(u32, bool)> {
        let slot = self.slot(address)?;
        if *slot == Self::UNASSIGNED {
            *slot = fresh;
            Some((fresh, true))
        } else {
            Some((*slot, false))
        }
    }
}

impl ReachableGraph {
    /// Whether the graph holds a cycle over the arcs `admits` accepts.
    ///
    /// A visited set alone cannot answer this: re-reaching a state off the
    /// current path is a diamond, which every determinized net is full of, not
    /// a cycle. Hence the three-colour marking — grey for the states on the
    /// path under exploration, black for the fully explored — with a back edge
    /// into grey as the witness. The DFS is iterative because a lexicon-shaped
    /// net is millions of states deep and recursion would blow the stack.
    fn has_cycle_over(&self, admits: impl Fn(&GraphArc) -> bool) -> bool {
        const WHITE: u8 = 0;
        const GREY: u8 = 1;
        const BLACK: u8 = 2;

        let states = self.starts.len().saturating_sub(1);
        let mut colour = vec![WHITE; states];
        // Every state is a root, not just the start: an arc-restricted subgraph
        // can hold a cycle that its own arcs never reach from the start —
        // `a (0:b)*` hides its epsilon cycle behind an input arc. Colours carry
        // across roots, so the total work stays linear in the graph.
        for root in 0..states {
            if colour[root] != WHITE {
                continue;
            }
            colour[root] = GREY;
            // Each frame is a state and how far its arc list has been walked.
            let mut stack = vec![(root, self.starts[root])];
            while let Some(frame) = stack.last_mut() {
                let (state, cursor) = *frame;
                if cursor >= self.starts[state + 1] {
                    colour[state] = BLACK;
                    stack.pop();
                    continue;
                }
                frame.1 = cursor + 1;
                let arc = self.arcs[cursor as usize];
                if !admits(&arc) {
                    continue;
                }
                let target = arc.target as usize;
                match colour[target] {
                    GREY => return true,
                    BLACK => continue,
                    _ => {
                        colour[target] = GREY;
                        stack.push((target, self.starts[target]));
                    }
                }
            }
        }
        false
    }
}

impl<T: TransducerTablesInterface> Transducer<T> {
    /// The reachable graph of this table pair, lifted out of the tables once.
    ///
    /// The optimized-lookup encoding keeps no state list to read anything off:
    /// a state is an offset into the index or the transition table, and the two
    /// share one address space. Worse, reading a state's arcs is not O(1) — for
    /// a state in the index table `get_transitions_from_state` scans the whole
    /// alphabet — so on a lexicon-sized net one sweep of the tables costs about
    /// a second. Every question below wants several passes over the same graph,
    /// hence one sweep into a flat adjacency and cheap passes over that.
    ///
    /// This is the walk `hfst_ol_to_hfst_basic_transducer` numbers states with,
    /// minus the interchange transducer it materializes, so anything derived
    /// here describes the graph `to_basic` would build.
    fn reachable_graph(&self) -> ReachableGraph {
        const START: TransitionTableIndex = 0;
        let mut ids = StateIds::new(
            self.hdr().index_table_size(),
            self.hdr().target_table_size(),
        );
        let mut order = vec![START];
        ids.intern(START, 0);

        let mut starts = vec![0u32];
        let mut arcs: Vec<GraphArc> = Vec::new();
        let mut next = 0usize;
        while next < order.len() {
            let state = order[next];
            next += 1;
            for tr in self.get_transitions_from_state(state).iter() {
                let target = self.get_transition_target(*tr);
                let Some((id, is_fresh)) = ids.intern(target, order.len() as u32) else {
                    continue;
                };
                if is_fresh {
                    order.push(target);
                }
                arcs.push(GraphArc {
                    input: self.get_transition_input(*tr),
                    output: self.get_transition_output(*tr),
                    weight: self.get_transition_weight(*tr),
                    target: id,
                });
            }
            starts.push(arcs.len() as u32);
        }
        ReachableGraph { starts, arcs }
    }

    /// Whether `input` consumes nothing off the input tape. Epsilon is symbol
    /// zero; a flag diacritic is scanned by the lookup engine without advancing
    /// the tape, so `find_loop_epsilon_transitions` and
    /// `HfstBasicTransducer::is_infinitely_ambiguous` both count it as epsilon
    /// — at the cost of false positives on flags no path can actually satisfy.
    fn consumes_no_input(&self, input: SymbolNumber) -> bool {
        input == 0 || self.alph().is_flag_diacritic(input)
    }

    /// Whether the transducer has a cycle at all.
    ///
    /// Diverges from the C++, which probes `HeaderFlag::Cyclic`: nothing in
    /// either tree ever calls `TransducerHeader::set_flag`, and every in-memory
    /// constructor hardcodes the flag false, so the probe answered "acyclic"
    /// for every transducer that had not been read off a disk file some other
    /// tool had stamped. Path extraction is guarded on this answer, so a cyclic
    /// net enumerated its infinite language until the disk filled.
    // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-transducer.is-cyclic-fn]
    // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-transducer.is-cyclic-fn]
    pub fn is_cyclic(&self) -> bool {
        self.reachable_graph().has_cycle_over(|_| true)
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.is-infinitely-ambiguous-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.is-infinitely-ambiguous-fn]
    /// Infinite ambiguity is an INPUT-EPSILON cycle, not any cycle: `a*` loops
    /// but each turn consumes an input symbol, so one input string still has
    /// finitely many analyses, while `(0:a)*` yields unboundedly many for the
    /// empty input. Same divergence from the flag probe as [`Self::is_cyclic`].
    pub fn is_infinitely_ambiguous(&self) -> bool {
        self.reachable_graph()
            .has_cycle_over(|arc| self.consumes_no_input(arc.input))
    }

    /// This transducer's own header, with every property flag it can decide
    /// filled in from the graph — what a file we write should say about itself.
    ///
    /// The three left false are the three no single walk decides: `Minimized`
    /// needs a minimization to establish, and `Deterministic` /
    /// `Input_deterministic` have no reader in this format (the C++ never
    /// consults them, and epsilon arcs make the question depend on which
    /// reading you take). False here means "not claimed" — the safe direction,
    /// unlike the hardcoded true every conversion used to stamp, which invites
    /// a consumer to skip work it needed.
    pub(super) fn header_with_graph_properties(&self) -> TransducerHeader {
        let graph = self.reachable_graph();
        let mut header = self.hdr().clone();

        header.cyclic = graph.has_cycle_over(|_| true);
        header.has_input_epsilon_cycles =
            header.cyclic && graph.has_cycle_over(|arc| self.consumes_no_input(arc.input));
        // An unweighted input-epsilon cycle is one whose every arc is free, so
        // going round it costs nothing and no weight cutoff prunes it.
        header.has_unweighted_input_epsilon_cycles = header.has_input_epsilon_cycles
            && graph.has_cycle_over(|arc| self.consumes_no_input(arc.input) && arc.weight == 0.0);

        // "Input epsilon" means the same thing here as it does to the lookup
        // engine and to the cycle flags above: an arc that advances no input
        // tape position, which a flag diacritic does not either. Reading it as
        // literal symbol zero let a flag-only cycle write a header claiming an
        // input-epsilon cycle whose arcs it denied having.
        header.has_input_epsilon_transitions = graph
            .arcs
            .iter()
            .any(|arc| self.consumes_no_input(arc.input));
        // Both tapes, and here epsilon is strictly symbol zero: a flag is
        // written to the output tape, and stripping it again is the lookup
        // formatter's business rather than a property of the graph.
        header.has_epsilon_epsilon_transitions = graph
            .arcs
            .iter()
            .any(|arc| arc.input == 0 && arc.output == 0);
        header
    }
}
