//! Whole-graph analysis: summary statistics, topological sort, path sizes
//! and cycle checks.

use super::*;
use crate::hfst_flag_diacritics::FdOperation;
use crate::hfst_symbol_defs::is_epsilon;

/* A topological sort. */
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.topological-sort]
// [spec:hfst:def:hfst-transition-graph.topological-sort]
// [spec:hfst:sem:hfst-transition-graph.topological-sort]
pub struct TopologicalSort {
    pub distance_of_state: Vec<i32>,
    pub states_at_distance: Vec<BTreeSet<HfstState>>,
}

impl TopologicalSort {
    pub fn new() -> Self {
        TopologicalSort {
            distance_of_state: Vec::new(),
            states_at_distance: Vec::new(),
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.topological-sort.set-biggest-state-number-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.topological-sort.set-biggest-state-number-fn]
    // [spec:hfst:def:hfst-transition-graph.topological-sort.set-biggest-state-number-fn]
    // [spec:hfst:sem:hfst-transition-graph.topological-sort.set-biggest-state-number-fn]
    pub fn set_biggest_state_number(&mut self, biggest_state_number: u32) {
        self.distance_of_state = vec![-1; (biggest_state_number + 1) as usize];
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.topological-sort.set-state-at-distance-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.topological-sort.set-state-at-distance-fn]
    // [spec:hfst:def:hfst-transition-graph.topological-sort.set-state-at-distance-fn]
    // [spec:hfst:sem:hfst-transition-graph.topological-sort.set-state-at-distance-fn]
    pub fn set_state_at_distance(&mut self, state: HfstState, distance: u32, overwrite: bool) {
        if state as usize > self.distance_of_state.len() - 1 {
            tracing::error!(
                "in TopologicalSort::set_state_at_distance: first argument ({}) is out of range (should be < {})",
                state,
                self.distance_of_state.len()
            );
        }
        while (distance + 1) as usize > self.states_at_distance.len() {
            self.states_at_distance.push(BTreeSet::new());
        }
        let previous_distance = self.distance_of_state[state as usize];
        if previous_distance != -1 && previous_distance != distance as i32 && overwrite {
            self.states_at_distance[previous_distance as usize].remove(&state);
        }
        self.states_at_distance[distance as usize].insert(state);
        self.distance_of_state[state as usize] = distance as i32;
    }

    /* The states that have a maximum distance of 'distance'. */
    pub fn get_states_at_distance(&mut self, distance: u32) -> &BTreeSet<HfstState> {
        while distance as usize > self.states_at_distance.len() - 1 {
            self.states_at_distance.push(BTreeSet::new());
        }
        &self.states_at_distance[distance as usize]
    }
}

impl Default for TopologicalSort {
    fn default() -> Self {
        Self::new()
    }
}

// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.sort-distance]
// [spec:hfst:def:hfst-transition-graph.sort-distance]
// [spec:hfst:sem:hfst-transition-graph.sort-distance]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortDistance {
    MaximumDistance,
    MinimumDistance,
}

/// Single-pass arc-traversal statistics of a graph, lifted from hfst-summarize.
/// Holds only the figures the traversal computes; the tool keeps its type-derived
/// flags (is_mutable/weighted), the header alphabet, the derived averages, and all
/// output formatting. Field names match the locals the tool destructures into.
#[derive(Clone, Debug)]
pub struct SummaryStats {
    pub states: usize,
    pub final_states: usize,
    pub arcs: usize,
    pub io_epsilons: usize,
    pub input_epsilons: usize,
    pub output_epsilons: usize,
    pub densest_arcs: usize,
    pub sparsest_arcs: usize,
    pub uniq_input_arcs: usize,
    pub uniq_output_arcs: usize,
    pub most_ambiguous_input: (HfstSymbol, u32),
    pub most_ambiguous_output: (HfstSymbol, u32),
    pub found_alphabet: BTreeSet<HfstSymbol>,
    pub symbol_pairs: BTreeMap<(HfstSymbol, HfstSymbol), u32>,
    pub acceptor: bool,
    pub input_deterministic: bool,
    pub output_deterministic: bool,
    pub cyclic: bool,
    pub cyclic_at_initial_state: bool,
}

impl HfstBasicTransducer {
    /// Compute hfst-summarize's single-pass arc-traversal statistics. Walks every
    /// state and transition once, accumulating counts, the seen alphabet, the
    /// per-state input/output ambiguity (→ determinism + most-ambiguous symbols),
    /// epsilon counts, the acceptor flag, and cyclicity. Transcribed verbatim from
    /// the tool's loop; the symbol-pair map is always populated (the tool decides
    /// whether to print it).
    pub fn summarize(&self) -> SummaryStats {
        let mut states: usize = 0;
        let mut final_states: usize = 0;
        let mut arcs: usize = 0;
        let mut io_epsilons: usize = 0;
        let mut input_epsilons: usize = 0;
        let mut output_epsilons: usize = 0;
        let mut densest_arcs: usize = 0;
        let mut sparsest_arcs: usize = 1 << 31;
        let mut uniq_input_arcs: usize = 0;
        let mut uniq_output_arcs: usize = 0;
        let mut most_ambiguous_input: (HfstSymbol, u32) = (HfstSymbol::default(), 0);
        let mut most_ambiguous_output: (HfstSymbol, u32) = (HfstSymbol::default(), 0);
        let mut found_alphabet: BTreeSet<HfstSymbol> = BTreeSet::new();
        let mut symbol_pairs: BTreeMap<(HfstSymbol, HfstSymbol), u32> = BTreeMap::new();
        let mut acceptor = true;
        let mut input_deterministic = true;
        let mut output_deterministic = true;
        let mut cyclic = false;
        let mut cyclic_at_initial_state = false;

        let is_begin_state = |s: u32| s == 0;
        for (source_state, transitions) in self.states_and_transitions().iter().enumerate() {
            let source_state = source_state as u32;
            let s = source_state;
            states += 1;
            if self.is_final_state(s) {
                final_states += 1;
            }
            let mut arcs_here: usize = 0;
            let mut input_ambiguity: BTreeMap<HfstSymbol, u32> = BTreeMap::new();
            let mut output_ambiguity: BTreeMap<HfstSymbol, u32> = BTreeMap::new();

            for tr_it in transitions {
                arcs += 1;
                arcs_here += 1;
                let in_sym = tr_it.get_input_symbol(&self.coder);
                let out_sym = tr_it.get_output_symbol(&self.coder);
                found_alphabet.insert(in_sym.clone());
                found_alphabet.insert(out_sym.clone());

                *symbol_pairs
                    .entry((in_sym.clone(), out_sym.clone()))
                    .or_insert(0) += 1;

                if in_sym != out_sym {
                    acceptor = false;
                }
                if is_epsilon(&in_sym) && is_epsilon(&out_sym) {
                    io_epsilons += 1;
                    input_epsilons += 1;
                    output_epsilons += 1;
                    input_deterministic = false;
                    output_deterministic = false;
                } else if is_epsilon(&in_sym) {
                    input_epsilons += 1;
                    input_deterministic = false;
                } else if is_epsilon(&out_sym) {
                    output_epsilons += 1;
                    output_deterministic = false;
                }
                input_ambiguity.entry(in_sym.clone()).or_insert(0);
                output_ambiguity.entry(out_sym.clone()).or_insert(0);
                let in_amb = input_ambiguity
                    .get_mut(&in_sym)
                    .expect("entry was inserted just above");
                *in_amb += 1;
                if *in_amb > 1 {
                    input_deterministic = false;
                }
                let out_amb = output_ambiguity
                    .get_mut(&out_sym)
                    .expect("entry was inserted just above");
                *out_amb += 1;
                if *out_amb > 1 {
                    output_deterministic = false;
                }
                if is_begin_state(source_state) && (tr_it.get_target_state() == 0) {
                    cyclic = true;
                    cyclic_at_initial_state = true;
                }
                if source_state == tr_it.get_target_state() {
                    cyclic = true;
                }
            }
            if arcs_here > densest_arcs {
                densest_arcs = arcs_here;
            }
            if arcs_here < sparsest_arcs {
                sparsest_arcs = arcs_here;
            }
            for (key, value) in input_ambiguity.iter() {
                if *value > most_ambiguous_input.1 {
                    most_ambiguous_input.0 = key.clone();
                    most_ambiguous_input.1 = *value;
                }
                uniq_input_arcs += 1;
            }
            for (key, value) in output_ambiguity.iter() {
                if *value > most_ambiguous_output.1 {
                    most_ambiguous_output.0 = key.clone();
                    most_ambiguous_output.1 = *value;
                }
                uniq_output_arcs += 1;
            }
        }

        SummaryStats {
            states,
            final_states,
            arcs,
            io_epsilons,
            input_epsilons,
            output_epsilons,
            densest_arcs,
            sparsest_arcs,
            uniq_input_arcs,
            uniq_output_arcs,
            most_ambiguous_input,
            most_ambiguous_output,
            found_alphabet,
            symbol_pairs,
            acceptor,
            input_deterministic,
            output_deterministic,
            cyclic,
            cyclic_at_initial_state,
        }
    }

    // --- Topological sort / path sizes ---

    /* Get a topological (maximum/minimum distance) sort of this graph. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.topsort-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.topsort-fn]
    // [spec:hfst:def:hfst-transition-graph.std.vector-std.set-hfst-state-topsort-fn]
    // [spec:hfst:sem:hfst-transition-graph.std.vector-std.set-hfst-state-topsort-fn]
    pub fn topsort(&self, dist: SortDistance) -> Vec<BTreeSet<HfstState>> {
        let mut current_distance: u32 = 0; // topological distance
        let mut top_sort = TopologicalSort::new();

        let st = self.state_vector.len();
        if st == 0 {
            return Vec::new();
        }
        let st = st - 1;
        let biggest_state_number = u32::try_from(st).expect("value out of u32 range");
        top_sort.set_biggest_state_number(biggest_state_number);

        top_sort.set_state_at_distance(0, current_distance, dist == SortDistance::MaximumDistance);
        let mut new_states_found; // end condition for the do-while loop

        loop {
            new_states_found = false;
            let mut new_states: BTreeSet<HfstState> = BTreeSet::new();

            // states accessible from the current set of states
            let states = top_sort.get_states_at_distance(current_distance).clone();
            for state in states.iter() {
                let transitions = &self.state_vector[*state as usize];
                for transition in transitions.iter() {
                    new_states_found = true;
                    new_states.insert(transition.get_target_state());
                }
            }

            for new_state in new_states.iter() {
                top_sort.set_state_at_distance(
                    *new_state,
                    current_distance + 1,
                    dist == SortDistance::MaximumDistance,
                );
            }
            current_distance += 1;

            if !new_states_found {
                break;
            }
        }

        top_sort.states_at_distance
    }

    /** The length of the longest string accepted by this graph, or -1. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.longest-path-size-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.longest-path-size-fn]
    // [spec:hfst:def:hfst-transition-graph.longest-path-size-fn]
    // [spec:hfst:sem:hfst-transition-graph.longest-path-size-fn]
    pub fn longest_path_size(&self) -> i32 {
        let states_sorted = self.topsort(SortDistance::MaximumDistance);
        let st = states_sorted.len();
        if st > 0 {
            for distance in (0..=i32::try_from(st - 1).expect("value out of i32 range")).rev() {
                let states = &states_sorted[distance as usize];
                for state in states.iter() {
                    if self.is_final_state(*state) {
                        return distance;
                    }
                }
            }
        }
        -1
    }

    /** The lengths of strings accepted by this graph, in descending order. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.path-sizes-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.path-sizes-fn]
    // [spec:hfst:def:hfst-transition-graph.std.vector-unsigned-int-path-sizes-fn]
    // [spec:hfst:sem:hfst-transition-graph.std.vector-unsigned-int-path-sizes-fn]
    pub fn path_sizes(&self) -> Vec<u32> {
        let mut result: Vec<u32> = Vec::new();
        let states_sorted = self.topsort(SortDistance::MinimumDistance);
        let st = states_sorted.len();
        if st > 0 {
            for distance in (0..=i32::try_from(st - 1).expect("value out of i32 range")).rev() {
                let states = &states_sorted[distance as usize];
                for state in states.iter() {
                    if self.is_final_state(*state) {
                        result.push(distance as u32);
                        break;
                    }
                }
            }
        }
        result
    }

    // --- Cycle detection ---

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.has-negative-epsilon-cycles-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.has-negative-epsilon-cycles-fn]
    pub fn has_negative_epsilon_cycles_recursive(
        &self,
        state: HfstState,
        total_weight: f32,
        state_weights: &mut BTreeMap<HfstState, f32>,
    ) -> bool {
        if let Some(w) = state_weights.get(&state) {
            // cycle detected
            if total_weight - *w < 0.0 {
                return true; // cycle with negative weight
            }
            return false; // cycle with positive weight
        }
        state_weights.insert(state, total_weight);

        let transitions = self
            .index(state)
            .expect("state is a valid state reached during the walk");
        for transition in transitions.iter() {
            if is_epsilon(&transition.get_input_symbol(&self.coder))
                && is_epsilon(&transition.get_output_symbol(&self.coder))
                && self.has_negative_epsilon_cycles_recursive(
                    transition.get_target_state(),
                    total_weight + transition.get_weight(),
                    state_weights,
                )
            {
                return true;
            }
        }
        state_weights.remove(&state);
        false
    }

    // [spec:hfst:def:hfst-transition-graph.has-negative-epsilon-cycles-fn]
    // [spec:hfst:sem:hfst-transition-graph.has-negative-epsilon-cycles-fn]
    pub fn has_negative_epsilon_cycles(&self) -> bool {
        let mut has_negative_epsilon_transitions = false;
        for it in self.state_vector.iter() {
            for tr_it in it.iter() {
                if is_epsilon(&tr_it.get_input_symbol(&self.coder))
                    && is_epsilon(&tr_it.get_output_symbol(&self.coder))
                    && tr_it.get_weight() < 0.0
                {
                    has_negative_epsilon_transitions = true;
                    break;
                }
            }
        }
        if !has_negative_epsilon_transitions {
            return false;
        }

        let mut state_weights: BTreeMap<HfstState, f32> = BTreeMap::new();
        for state in Self::INITIAL_STATE..(self.get_max_state() + 1) {
            if self.has_negative_epsilon_cycles_recursive(state, 0.0, &mut state_weights) {
                return true;
            }
        }
        false
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-infinitely-ambiguous-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-infinitely-ambiguous-fn]
    pub fn is_infinitely_ambiguous_recursive(
        &self,
        state: HfstState,
        epsilon_path_states: &mut BTreeSet<HfstState>,
        states_handled: &mut Vec<u32>,
    ) -> bool {
        if states_handled[state as usize] != 0 {
            return false;
        }

        let transitions = self
            .index(state)
            .expect("state is a valid state reached during the walk");
        for transition in transitions.iter() {
            // Diacritics are also treated as epsilons (may yield false positives).
            if is_epsilon(&transition.get_input_symbol(&self.coder))
                || FdOperation::is_diacritic(&transition.get_input_symbol(&self.coder))
            {
                epsilon_path_states.insert(state);
                if epsilon_path_states.contains(&transition.get_target_state()) {
                    return true;
                }
                if self.is_infinitely_ambiguous_recursive(
                    transition.get_target_state(),
                    epsilon_path_states,
                    states_handled,
                ) {
                    return true;
                }
                epsilon_path_states.remove(&state);
            }
        }
        states_handled[state as usize] = 1;
        false
    }

    // [spec:hfst:def:hfst-transition-graph.is-infinitely-ambiguous-fn]
    // [spec:hfst:sem:hfst-transition-graph.is-infinitely-ambiguous-fn]
    pub fn is_infinitely_ambiguous(&self) -> bool {
        let mut epsilon_path_states: BTreeSet<HfstState> = BTreeSet::new();
        let max_state = self.get_max_state();
        let mut states_handled: Vec<u32> = vec![0; (max_state + 1) as usize];

        for state in Self::INITIAL_STATE..(max_state + 1) {
            if self.is_infinitely_ambiguous_recursive(
                state,
                &mut epsilon_path_states,
                &mut states_handled,
            ) {
                return true;
            }
        }
        false
    }
}
