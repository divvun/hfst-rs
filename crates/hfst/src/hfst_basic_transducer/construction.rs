//! Building graphs from graphs: renumbered copies, path disjunction and
//! completion, compile-replace splicing, and intersection and merge.

use super::*;
use crate::hfst_symbol_defs::{StringPairVector, is_epsilon};

/// Shared mutable bookkeeping threaded through the intersecting compose walk
/// (`find_matches`): the product-state map and the visited-state agenda.
pub struct IntersectSearch<'a> {
    state_map: &'a mut StateMap,
    agenda: &'a mut BTreeSet<HfstState>,
}

/// Shared context threaded through the list-merge walk (`find_matches_for_merge`
/// / `handle_list_match`): the product-state map, the visited-state agenda, the
/// set of markers already added, and the read-only list-symbol table.
pub struct MergeContext<'a> {
    state_map: &'a mut StateMap,
    agenda: &'a mut BTreeSet<HfstState>,
    markers_added: &'a mut BTreeSet<HfstSymbol>,
    list_symbols: &'a BTreeMap<HfstSymbol, BTreeSet<HfstSymbol>>,
}

impl HfstBasicTransducer {
    /// Return a copy with the states renumbered in discovery order: state 0
    /// stays 0, and every other state is assigned the next free id the first
    /// time it is reached — either as the running iteration source or as an arc
    /// target. All transitions are copied verbatim with their targets remapped.
    /// This compacts the state numbering; it is the pure-renumber core lifted
    /// from hfst-preprocess-for-optimized-lookup-format.
    ///
    /// Note: state 0 is pre-seeded into the id map, so its final weight is not
    /// copied (matching the long-standing behaviour of the CLI loop this was
    /// lifted from). Callers that need state 0's final weight preserved set it
    /// themselves before/after renumbering.
    pub fn renumber_states(&self) -> HfstBasicTransducer {
        let mut replication = HfstBasicTransducer::new();
        let mut state_count: HfstState = 1;
        let mut rebuilt: BTreeMap<HfstState, HfstState> = BTreeMap::new();
        rebuilt.insert(0, 0);
        for (source_state, state) in self.iter().enumerate() {
            let source_state = source_state as HfstState;
            if let std::collections::btree_map::Entry::Vacant(e) = rebuilt.entry(source_state) {
                replication.add_state(state_count);
                if self.is_final_state(source_state) {
                    replication.set_final_weight(
                        state_count,
                        &self
                            .get_final_weight(source_state)
                            .expect("state was confirmed final via is_final_state"),
                    );
                }
                e.insert(state_count);
                state_count += 1;
            }
            for arc in state.iter() {
                if let std::collections::btree_map::Entry::Vacant(e) =
                    rebuilt.entry(arc.get_target_state())
                {
                    replication.add_state(state_count);
                    if self.is_final_state(arc.get_target_state()) {
                        replication.set_final_weight(
                            state_count,
                            &self
                                .get_final_weight(arc.get_target_state())
                                .expect("state was confirmed final via is_final_state"),
                        );
                    }
                    e.insert(state_count);
                    state_count += 1;
                }
                let isym = arc.get_input_symbol(&self.coder);
                let osym = arc.get_output_symbol(&self.coder);
                let nu = HfstBasicTransition::new_symbols(
                    rebuilt[&arc.get_target_state()],
                    isym,
                    osym,
                    arc.get_weight(),
                    replication.coder_mut(),
                );
                let src_rebuilt = rebuilt[&source_state];
                replication.add_transition(src_rebuilt, &nu, true);
            }
        }
        replication
    }

    /// Return a copy with every transition whose input or output symbol equals
    /// `symbol` removed, surviving states renumbered in discovery order. This is
    /// the kill-paths transform lifted from hfst-kill-paths: the discovery-order
    /// rebuild of [`Self::renumber_states`] plus a per-arc filter. Unlike
    /// `renumber_states` it seeds state 0's final weight up front (matching the
    /// CLI loop), so an accepting start state is preserved.
    pub fn kill_paths(&self, symbol: &str) -> HfstBasicTransducer {
        let mut replication = HfstBasicTransducer::new();
        let mut state_count: HfstState = 1;
        let mut rebuilt: BTreeMap<HfstState, HfstState> = BTreeMap::new();
        rebuilt.insert(0, 0);
        if self.is_final_state(0) {
            replication.set_final_weight(
                0,
                &self
                    .get_final_weight(0)
                    .expect("state was confirmed final via is_final_state"),
            );
        }
        for (source_state, state) in self.iter().enumerate() {
            let source_state = source_state as HfstState;
            if let std::collections::btree_map::Entry::Vacant(e) = rebuilt.entry(source_state) {
                replication.add_state(state_count);
                if self.is_final_state(source_state) {
                    replication.set_final_weight(
                        state_count,
                        &self
                            .get_final_weight(source_state)
                            .expect("state was confirmed final via is_final_state"),
                    );
                }
                e.insert(state_count);
                state_count += 1;
            }
            for arc in state.iter() {
                if arc.get_input_symbol(&self.coder) == symbol
                    || arc.get_output_symbol(&self.coder) == symbol
                {
                    // killed arc: do not replicate
                    continue;
                }
                if let std::collections::btree_map::Entry::Vacant(e) =
                    rebuilt.entry(arc.get_target_state())
                {
                    replication.add_state(state_count);
                    if self.is_final_state(arc.get_target_state()) {
                        replication.set_final_weight(
                            state_count,
                            &self
                                .get_final_weight(arc.get_target_state())
                                .expect("state was confirmed final via is_final_state"),
                        );
                    }
                    e.insert(state_count);
                    state_count += 1;
                }
                let isym = arc.get_input_symbol(&self.coder);
                let osym = arc.get_output_symbol(&self.coder);
                let nu = HfstBasicTransition::new_symbols(
                    rebuilt[&arc.get_target_state()],
                    isym,
                    osym,
                    arc.get_weight(),
                    replication.coder_mut(),
                );
                replication.add_transition(rebuilt[&source_state], &nu, true);
            }
        }
        replication
    }

    /// Return a copy with every weight transformed by `f`, surviving states
    /// renumbered in discovery order (the do_reweight rebuild from hfst-reweight).
    /// `f` receives the current weight together with the transition's symbols so
    /// it can reweight conditionally: `(w, None, None)` for a state's final weight
    /// and `(w, Some(input), Some(output))` for an arc weight; it returns the new
    /// weight. Unlike the unconditional backend `transform_weights`, this is
    /// symbol-aware. State 0's final weight is seeded up front (matching the CLI
    /// loop, like [`Self::kill_paths`]).
    pub fn transform_weights<F>(&self, f: F) -> HfstBasicTransducer
    where
        F: Fn(f32, Option<&str>, Option<&str>) -> f32,
    {
        let mut replication = HfstBasicTransducer::new();
        let mut state_count: HfstState = 1;
        let mut rebuilt: BTreeMap<HfstState, HfstState> = BTreeMap::new();
        rebuilt.insert(0, 0);
        if self.is_final_state(0) {
            replication.set_final_weight(
                0,
                &f(
                    self.get_final_weight(0)
                        .expect("state was confirmed final via is_final_state"),
                    None,
                    None,
                ),
            );
        }
        for (source_state, state) in self.iter().enumerate() {
            let source_state = source_state as HfstState;
            if let std::collections::btree_map::Entry::Vacant(e) = rebuilt.entry(source_state) {
                replication.add_state(state_count);
                if self.is_final_state(source_state) {
                    replication.set_final_weight(
                        state_count,
                        &f(
                            self.get_final_weight(source_state)
                                .expect("state was confirmed final via is_final_state"),
                            None,
                            None,
                        ),
                    );
                }
                e.insert(state_count);
                state_count += 1;
            }
            for arc in state.iter() {
                let target = arc.get_target_state();
                if let std::collections::btree_map::Entry::Vacant(e) = rebuilt.entry(target) {
                    replication.add_state(state_count);
                    if self.is_final_state(target) {
                        replication.set_final_weight(
                            state_count,
                            &f(
                                self.get_final_weight(target)
                                    .expect("state was confirmed final via is_final_state"),
                                None,
                                None,
                            ),
                        );
                    }
                    e.insert(state_count);
                    state_count += 1;
                }
                let isym = arc.get_input_symbol(&self.coder);
                let osym = arc.get_output_symbol(&self.coder);
                let nuweight = f(arc.get_weight(), Some(&isym), Some(&osym));
                let nu = HfstBasicTransition::new_symbols(
                    rebuilt[&target],
                    isym,
                    osym,
                    nuweight,
                    replication.coder_mut(),
                );
                replication.add_transition(rebuilt[&source_state], &nu, true);
            }
        }
        replication
    }

    // --- Disjunction ---

    /* Disjunct the transition of path 'spv' pointed by 'it' to state 's'. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.disjunct-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.disjunct-fn]
    // [spec:hfst:def:hfst-transition-graph.disjunct-fn]
    // [spec:hfst:sem:hfst-transition-graph.disjunct-fn]
    pub fn disjunct(&mut self, spv: &StringPairVector, it: &mut usize, s: HfstState) -> HfstState {
        let mut current_state = s;
        while *it != spv.len() {
            // C++ copies the transition vector before searching it.
            let tr = self.state_vector[current_state as usize].clone();
            let found = tr.iter().find(|tr_it| {
                let data = tr_it.get_transition_data();
                data.get_input_symbol(&self.coder) == spv[*it].0
                    && data.get_output_symbol(&self.coder) == spv[*it].1
            });

            let next_state = match found {
                Some(tr_it) => tr_it.get_target_state(),
                None => {
                    let next_state = self.add_state_new();
                    let transition = HfstBasicTransition::new_symbols(
                        next_state,
                        spv[*it].0.clone(),
                        spv[*it].1.clone(),
                        0.0,
                        self.coder_mut(),
                    );
                    self.add_transition(current_state, &transition, true);
                    next_state
                }
            };

            *it += 1;
            current_state = next_state;
        }
        current_state
    }

    /** @brief Disjunct this graph with a one-path graph defined by 'spv'. */
    pub fn disjunct_path(&mut self, spv: &StringPairVector, weight: WeightType) -> &mut Self {
        let mut it: usize = 0;
        let final_state = self.disjunct(spv, &mut it, Self::INITIAL_STATE);

        if self.is_final_state(final_state) {
            let old_weight = self
                .get_final_weight(final_state)
                .expect("state was confirmed final via is_final_state");
            if old_weight < weight {
                return self; // smaller-weight path remains
            }
        }
        self.set_final_weight(final_state, &weight);
        self
    }

    /** @brief Make the graph complete (add a failure state). */
    pub fn complete(&mut self) -> crate::error::Result<&mut Self> {
        let failure_state = self.add_state_new();

        for s in 0..self.state_vector.len() {
            let mut symbols_present: BTreeSet<HfstSymbol> = BTreeSet::new();

            for i in 0..self.state_vector[s].len() {
                let data = self.state_vector[s][i].get_transition_data().clone();
                let isym = data.get_input_symbol(&self.coder);
                let osym = data.get_output_symbol(&self.coder);
                if isym != osym {
                    crate::bail!(TransducersAreNotAutomata);
                }
                symbols_present.insert(isym);
            }

            let alpha_snapshot: Vec<HfstSymbol> = self.alphabet.iter().cloned().collect();
            for alpha_it in alpha_snapshot.iter() {
                if !symbols_present.contains(alpha_it) && !Self::is_special_symbol(alpha_it) {
                    let tr = HfstBasicTransition::new_symbols(
                        failure_state,
                        alpha_it.clone(),
                        alpha_it.clone(),
                        0.0,
                        self.coder_mut(),
                    );
                    self.add_transition(s as HfstState, &tr, true);
                }
            }
        }
        Ok(self)
    }

    // --- compile-replace regexp paths ---

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.check-regexp-state-for-cycle-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.check-regexp-state-for-cycle-fn]
    // [spec:hfst:def:hfst-transition-graph.check-regexp-state-for-cycle-fn]
    // [spec:hfst:sem:hfst-transition-graph.check-regexp-state-for-cycle-fn]
    pub fn check_regexp_state_for_cycle(
        s: HfstState,
        states_visited: &BTreeSet<HfstState>,
    ) -> crate::error::Result<()> {
        if states_visited.contains(&s) {
            crate::bail!(
                Hfst,
                "error: loop detected inside compile-replace regular expression"
            );
        }
        Ok(())
    }

    // Returns whether tr is "^]":"^]". Errors if tr is not allowed.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.check-regexp-transition-end-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.check-regexp-transition-end-fn]
    // [spec:hfst:def:hfst-transition-graph.check-regexp-transition-end-fn]
    // [spec:hfst:sem:hfst-transition-graph.check-regexp-transition-end-fn]
    pub fn check_regexp_transition_end(
        tr: &HfstBasicTransition,
        input_side: bool,
        coder: &SymbolCoder,
    ) -> crate::error::Result<bool> {
        let istr = tr.get_input_symbol(coder);
        let ostr = tr.get_output_symbol(coder);

        if (input_side && is_epsilon(&istr)) || (!input_side && is_epsilon(&ostr)) {
        } else if (input_side && Self::is_special_symbol(&istr))
            || (!input_side && Self::is_special_symbol(&ostr))
        {
            crate::bail!(
                Hfst,
                "error: special symbol detected in compile-replace regular expression"
            );
        }

        if (input_side && istr == "^[") || (!input_side && ostr == "^[") {
            crate::bail!(
                Hfst,
                "error: ^[ detected inside compile-replace regular expression"
            );
        }
        if (input_side && istr == "^]") || (!input_side && ostr == "^]") {
            return Ok(true);
        }
        Ok(false)
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-regexp-paths-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-regexp-paths-fn]
    // [spec:hfst:def:hfst-transition-graph.find-regexp-paths-fn]
    // [spec:hfst:sem:hfst-transition-graph.find-regexp-paths-fn]
    // [spec:hfst:def:hfst-transition-graph.void-find-regexp-paths-fn]
    // [spec:hfst:sem:hfst-transition-graph.void-find-regexp-paths-fn]
    pub fn find_regexp_paths(
        &self,
        s: HfstState,
        states_visited: &mut BTreeSet<HfstState>,
        path: &mut Vec<(HfstSymbol, HfstSymbol)>,
        full_paths: &mut HfstReplacements,
        input_side: bool,
    ) -> crate::error::Result<()> {
        // no cycles allowed inside "^[" and "^]"
        Self::check_regexp_state_for_cycle(s, states_visited)?;
        states_visited.insert(s);

        let transitions = self
            .index(s)
            .expect("s is a valid state reached during the walk");
        for transition in transitions.iter() {
            // closing bracket
            if Self::check_regexp_transition_end(transition, input_side, &self.coder)? {
                // cannot lead to a state already visited
                Self::check_regexp_state_for_cycle(transition.get_target_state(), states_visited)?;
                path.push((
                    transition.get_input_symbol(&self.coder),
                    transition.get_output_symbol(&self.coder),
                ));
                full_paths.push((transition.get_target_state(), path.clone()));
                path.pop();
            } else {
                path.push((
                    transition.get_input_symbol(&self.coder),
                    transition.get_output_symbol(&self.coder),
                ));
                self.find_regexp_paths(
                    transition.get_target_state(),
                    states_visited,
                    path,
                    full_paths,
                    input_side,
                )?;
                path.pop();
            }
        }
        states_visited.remove(&s);
        Ok(())
    }

    pub fn find_regexp_paths_driver(
        &self,
        s: HfstState,
        full_paths: &mut HfstReplacements,
        input_side: bool,
    ) -> crate::error::Result<()> {
        let transitions = self
            .index(s)
            .expect("s is a valid state reached during the walk");
        for transition in transitions.iter() {
            let istr = transition.get_input_symbol(&self.coder);
            let ostr = transition.get_output_symbol(&self.coder);
            if (input_side && istr == "^[") || (!input_side && ostr == "^[") {
                let mut states_visited: BTreeSet<HfstState> = BTreeSet::new();
                states_visited.insert(s);
                let mut path: Vec<(HfstSymbol, HfstSymbol)> = Vec::new();
                path.push((istr.clone(), ostr.clone()));
                self.find_regexp_paths(
                    transition.get_target_state(),
                    &mut states_visited,
                    &mut path,
                    full_paths,
                    input_side,
                )?;
            }
        }
        Ok(())
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-replacements-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-replacements-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst-replacements-map-find-replacements-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst-replacements-map-find-replacements-fn]
    pub fn find_replacements(&self, input_side: bool) -> crate::error::Result<HfstReplacementsMap> {
        let mut replacements = HfstReplacementsMap::new();
        for (state, _it) in self.state_vector.iter().enumerate() {
            let state = state as u32;
            let mut full_paths: HfstReplacements = Vec::new();
            self.find_regexp_paths_driver(state, &mut full_paths, input_side)?;
            if !full_paths.is_empty() {
                replacements.insert(state, full_paths);
            }
        }
        Ok(replacements)
    }

    // Attach a copy of 'graph' between states 'state1' and 'state2' with epsilon
    // transitions.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.insert-transducer-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.insert-transducer-fn]
    // [spec:hfst:def:hfst-transition-graph.insert-transducer-fn]
    // [spec:hfst:sem:hfst-transition-graph.insert-transducer-fn]
    pub fn insert_transducer(
        &mut self,
        state1: HfstState,
        state2: HfstState,
        graph: &HfstBasicTransducer,
    ) {
        let offset = self.add_state_new();
        // 'graph' has its own coder; resolve its arc symbols through *its* coding,
        // then re-intern them into this graph's coder.
        let graph_coder = graph.coder();
        for (source_state, it) in graph.state_vector.iter().enumerate() {
            let source_state = source_state as u32;
            for tr_it in it.iter() {
                let data = tr_it.get_transition_data();
                let isym = data.get_input_symbol(graph_coder);
                let osym = data.get_output_symbol(graph_coder);
                let transition = HfstBasicTransition::new_symbols(
                    tr_it.get_target_state() + offset,
                    isym,
                    osym,
                    data.get_weight(),
                    self.coder_mut(),
                );
                self.add_transition(source_state + offset, &transition, true);
            }
        }

        // Epsilon transitions from final states of 'graph'.
        let finals: Vec<(HfstState, f32)> = graph
            .final_weight_map
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect();
        for (k, v) in finals {
            let epsilon_transition = HfstBasicTransition::new_symbols(
                state2,
                HfstTropicalTransducerTransitionData::get_epsilon(),
                HfstTropicalTransducerTransitionData::get_epsilon(),
                v,
                self.coder_mut(),
            );
            self.add_transition(k + offset, &epsilon_transition, true);
        }

        // Initial transition.
        let epsilon_transition = HfstBasicTransition::new_symbols(
            offset,
            HfstTropicalTransducerTransitionData::get_epsilon(),
            HfstTropicalTransducerTransitionData::get_epsilon(),
            0.0,
            self.coder_mut(),
        );
        self.add_transition(state1, &epsilon_transition, true);
    }

    // --- Intersection / merge ---

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-target-state-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-target-state-fn]
    // [spec:hfst:def:hfst-transition-graph.find-target-state-fn]
    // [spec:hfst:sem:hfst-transition-graph.find-target-state-fn]
    pub fn find_target_state(
        target1: HfstState,
        target2: HfstState,
        state_map: &mut StateMap,
        intersection: &mut HfstBasicTransducer,
        was_new_state: &mut bool,
    ) -> HfstState {
        let state_pair = (target1, target2);
        if let Some(s) = state_map.get(&state_pair) {
            *was_new_state = false;
            return *s;
        }
        let retval = intersection.add_state_new();
        state_map.insert(state_pair, retval);
        *was_new_state = true;
        retval
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.handle-match-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.handle-match-fn]
    // [spec:hfst:def:hfst-transition-graph.handle-match-fn]
    // [spec:hfst:sem:hfst-transition-graph.handle-match-fn]
    pub fn handle_match(
        graph1: &HfstBasicTransducer,
        tr1: &HfstBasicTransition,
        graph2: &HfstBasicTransducer,
        tr2: &HfstBasicTransition,
        intersection: &mut HfstBasicTransducer,
        state: HfstState,
        state_map: &mut StateMap,
    ) -> HfstState {
        let target1 = tr1.get_target_state();
        let target2 = tr2.get_target_state();
        let mut was_new_state = false;
        let retval = Self::find_target_state(
            target1,
            target2,
            state_map,
            intersection,
            &mut was_new_state,
        );
        // the sum of weights is copied to the resulting intersection
        let transition_weight = tr1.get_weight() + tr2.get_weight();
        // tr1's labels resolve through graph1's coder; re-intern into the result.
        let isym = tr1.get_input_symbol(graph1.coder());
        let osym = tr1.get_output_symbol(graph1.coder());
        let tr = HfstBasicTransition::new_symbols(
            retval,
            isym,
            osym,
            transition_weight,
            intersection.coder_mut(),
        );
        intersection.add_transition(state, &tr, true);
        if was_new_state && (graph1.is_final_state(target1) && graph2.is_final_state(target2)) {
            let final_weight = graph1
                .get_final_weight(target1)
                .expect("state was confirmed final via is_final_state")
                + graph2
                    .get_final_weight(target2)
                    .expect("state was confirmed final via is_final_state");
            intersection.set_final_weight(retval, &final_weight);
        }
        retval
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-matches-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-matches-fn]
    // [spec:hfst:def:hfst-transition-graph.find-matches-fn]
    // [spec:hfst:sem:hfst-transition-graph.find-matches-fn]
    pub fn find_matches(
        graph1: &HfstBasicTransducer,
        state1: HfstState,
        graph2: &HfstBasicTransducer,
        state2: HfstState,
        intersection: &mut HfstBasicTransducer,
        state: HfstState,
        search: &mut IntersectSearch<'_>,
    ) {
        search.agenda.insert(state); // do not handle 'state' twice
        let tr1 = &graph1.state_vector[state1 as usize];
        let tr2 = &graph2.state_vector[state2 as usize];

        if tr1.is_empty() || tr2.is_empty() {
            return; // no matches possible
        }
        let mut start_search_from: u32 = 0;

        for transition1 in tr1.iter() {
            let transition_data1 = transition1.get_transition_data();

            for j in start_search_from..tr2.len() as u32 {
                let transition2 = &tr2[j as usize];
                let transition_data2 = transition2.get_transition_data();
                if transition_data2.less_than_ignore_weight(transition_data1) {
                    // no match found, continue searching
                } else if transition_data1.less_than_ignore_weight(transition_data2) {
                    start_search_from = j;
                    break;
                } else {
                    // match found
                    let target = Self::handle_match(
                        graph1,
                        transition1,
                        graph2,
                        transition2,
                        intersection,
                        state,
                        search.state_map,
                    );
                    if !search.agenda.contains(&target) {
                        Self::find_matches(
                            graph1,
                            transition1.get_target_state(),
                            graph2,
                            transition2.get_target_state(),
                            intersection,
                            target,
                            search,
                        );
                    }
                    start_search_from = j + 1;
                    break;
                }
            }
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.intersect-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.intersect-fn]
    pub fn intersect(
        graph1: &mut HfstBasicTransducer,
        graph2: &mut HfstBasicTransducer,
    ) -> HfstBasicTransducer {
        let mut retval = HfstBasicTransducer::new();
        let mut state_map: StateMap = BTreeMap::new();
        let mut agenda: BTreeSet<HfstState> = BTreeSet::new();
        graph1.sort_arcs();
        graph2.sort_arcs();
        state_map.insert((0, 0), 0); // initial states

        if graph1.is_final_state(0) && graph2.is_final_state(0) {
            let final_weight = graph1
                .get_final_weight(0)
                .expect("state was confirmed final via is_final_state")
                .min(
                    graph2
                        .get_final_weight(0)
                        .expect("state was confirmed final via is_final_state"),
                );
            retval.set_final_weight(0, &final_weight);
        }

        let mut search = IntersectSearch {
            state_map: &mut state_map,
            agenda: &mut agenda,
        };
        Self::find_matches(graph1, 0, graph2, 0, &mut retval, 0, &mut search);

        retval
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.handle-non-list-match-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.handle-non-list-match-fn]
    // [spec:hfst:def:hfst-transition-graph.handle-non-list-match-fn]
    // [spec:hfst:sem:hfst-transition-graph.handle-non-list-match-fn]
    pub fn handle_non_list_match(
        graph: &HfstBasicTransducer,
        graph_transition: &HfstBasicTransition,
        merger: &HfstBasicTransducer,
        merger_target: HfstState,
        result: &mut HfstBasicTransducer,
        result_state: HfstState,
        state_map: &mut StateMap,
    ) -> HfstState {
        let graph_target = graph_transition.get_target_state();
        let mut was_new_state = false;
        let retval = Self::find_target_state(
            graph_target,
            merger_target,
            state_map,
            result,
            &mut was_new_state,
        );
        let isym = graph_transition.get_input_symbol(graph.coder());
        let osym = graph_transition.get_output_symbol(graph.coder());
        let tr = HfstBasicTransition::new_symbols(
            retval,
            isym,
            osym,
            graph_transition.get_weight(),
            result.coder_mut(),
        );
        result.add_transition(result_state, &tr, true);
        if was_new_state
            && (graph.is_final_state(graph_target) && merger.is_final_state(merger_target))
        {
            let final_weight = graph
                .get_final_weight(graph_target)
                .expect("state was confirmed final via is_final_state")
                + merger
                    .get_final_weight(merger_target)
                    .expect("state was confirmed final via is_final_state");
            result.set_final_weight(retval, &final_weight);
        }
        retval
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.handle-list-match-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.handle-list-match-fn]
    // [spec:hfst:def:hfst-transition-graph.handle-list-match-fn]
    // [spec:hfst:sem:hfst-transition-graph.handle-list-match-fn]
    // Compile-replace list-match helper ported 1:1 from the C++; parameter list
    // mirrors that signature.
    pub fn handle_list_match(
        graph: &HfstBasicTransducer,
        graph_transition: &HfstBasicTransition,
        merger: &HfstBasicTransducer,
        merger_transition: &HfstBasicTransition,
        result: &mut HfstBasicTransducer,
        result_state: HfstState,
        ctx: &mut MergeContext<'_>,
    ) -> HfstState {
        let graph_target = graph_transition.get_target_state();
        let merger_target = merger_transition.get_target_state();
        let mut was_new_state = false;
        let retval = Self::find_target_state(
            graph_target,
            merger_target,
            ctx.state_map,
            result,
            &mut was_new_state,
        );
        let transition_weight = graph_transition.get_weight() + merger_transition.get_weight();

        // testing: add a marker
        let extra_state = result.add_state_new();
        let graph_isym = graph_transition.get_input_symbol(graph.coder());
        let graph_osym = graph_transition.get_output_symbol(graph.coder());
        let marker_tr = HfstBasicTransition::new_symbols(
            extra_state,
            HfstSymbol::from(format!("@{}@", graph_isym)),
            HfstSymbol::from(format!("@{}@", graph_osym)),
            0.0,
            result.coder_mut(),
        );
        result.add_transition(result_state, &marker_tr, true);
        ctx.markers_added
            .insert(HfstSymbol::from(format!("@{}@", graph_isym)));

        let merger_isym = merger_transition.get_input_symbol(merger.coder());
        let merger_osym = merger_transition.get_output_symbol(merger.coder());
        let merger_tr = HfstBasicTransition::new_symbols(
            retval,
            merger_isym,
            merger_osym,
            transition_weight,
            result.coder_mut(),
        );
        result.add_transition(extra_state, &merger_tr, true);
        if was_new_state
            && (graph.is_final_state(graph_target) && merger.is_final_state(merger_target))
        {
            let final_weight = graph
                .get_final_weight(graph_target)
                .expect("state was confirmed final via is_final_state")
                + merger
                    .get_final_weight(merger_target)
                    .expect("state was confirmed final via is_final_state");
            result.set_final_weight(retval, &final_weight);
        }
        retval
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-list-symbol-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-list-symbol-fn]
    // [spec:hfst:def:hfst-transition-graph.is-list-symbol-fn]
    // [spec:hfst:sem:hfst-transition-graph.is-list-symbol-fn]
    pub fn is_list_symbol(
        transition_data: &HfstTropicalTransducerTransitionData,
        list_symbols: &BTreeMap<HfstSymbol, BTreeSet<HfstSymbol>>,
        coder: &SymbolCoder,
    ) -> crate::error::Result<bool> {
        let isymbol = transition_data.get_input_symbol(coder);
        let osymbol = transition_data.get_output_symbol(coder);

        if isymbol != osymbol {
            crate::bail!(
                TransducersAreNotAutomata,
                "is_list_symbol: input and output symbols must be the same"
            );
        }
        Ok(list_symbols.contains_key(&isymbol))
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-matches-for-merge-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-matches-for-merge-fn]
    // [spec:hfst:def:hfst-transition-graph.find-matches-for-merge-fn]
    // [spec:hfst:sem:hfst-transition-graph.find-matches-for-merge-fn]
    pub fn find_matches_for_merge(
        graph: &HfstBasicTransducer,
        graph_state: HfstState,
        merger: &HfstBasicTransducer,
        merger_state: HfstState,
        result: &mut HfstBasicTransducer,
        result_state: HfstState,
        ctx: &mut MergeContext<'_>,
    ) -> crate::error::Result<()> {
        ctx.agenda.insert(result_state); // do not handle 'result_state' twice
        let graph_transitions = &graph.state_vector[graph_state as usize];
        let merger_transitions = &merger.state_vector[merger_state as usize];

        if graph_transitions.is_empty() {
            return Ok(()); // no matches possible
        }

        for graph_transition in graph_transitions.iter() {
            let graph_transition_data = graph_transition.get_transition_data();

            // List symbols must be checked separately.
            if Self::is_list_symbol(graph_transition_data, ctx.list_symbols, graph.coder())? {
                // Clone the (small) symbol set so the read borrow of `ctx` ends
                // before the `&mut ctx` calls below.
                let symbol_list = ctx.list_symbols
                    [&graph_transition_data.get_input_symbol(graph.coder())]
                    .clone();
                let mut list_match_found = false;
                for merger_transition in merger_transitions.iter() {
                    let merger_transition_data = merger_transition.get_transition_data();
                    let isymbol = merger_transition_data.get_input_symbol(merger.coder());
                    let osymbol = merger_transition_data.get_output_symbol(merger.coder());

                    if isymbol != osymbol {
                        crate::bail!(
                            TransducersAreNotAutomata,
                            "find_matches_for_merge: input and output symbols must be the same"
                        );
                    }

                    if symbol_list.contains(&isymbol) {
                        list_match_found = true;
                        let target = Self::handle_list_match(
                            graph,
                            graph_transition,
                            merger,
                            merger_transition,
                            result,
                            result_state,
                            ctx,
                        );
                        if !ctx.agenda.contains(&target) {
                            Self::find_matches_for_merge(
                                graph,
                                graph_transition.get_target_state(),
                                merger,
                                merger_transition.get_target_state(),
                                result,
                                target,
                                ctx,
                            )?;
                        }
                    }
                }
                if list_match_found {
                    continue;
                }
            }
            // Not a list symbol (or no match): copy the symbol as such, using
            // merger_state as the merger transition target state.
            let target = Self::handle_non_list_match(
                graph,
                graph_transition,
                merger,
                merger_state,
                result,
                result_state,
                ctx.state_map,
            );
            if !ctx.agenda.contains(&target) {
                Self::find_matches_for_merge(
                    graph,
                    graph_transition.get_target_state(),
                    merger,
                    merger_state,
                    result,
                    target,
                    ctx,
                )?;
            }
        }
        Ok(())
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.merge-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.merge-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst-transition-graph-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst-transition-graph-fn]
    pub fn merge(
        graph: &mut HfstBasicTransducer,
        merger: &mut HfstBasicTransducer,
        list_symbols: &BTreeMap<HfstSymbol, BTreeSet<HfstSymbol>>,
        markers_added: &mut BTreeSet<HfstSymbol>,
    ) -> crate::error::Result<HfstBasicTransducer> {
        let mut result = HfstBasicTransducer::new();
        let mut state_map: StateMap = BTreeMap::new();
        let mut agenda: BTreeSet<HfstState> = BTreeSet::new();
        graph.sort_arcs();
        merger.sort_arcs();
        state_map.insert((0, 0), 0); // initial states

        if graph.is_final_state(0) && merger.is_final_state(0) {
            let final_weight = graph.get_final_weight(0)? + merger.get_final_weight(0)?;
            result.set_final_weight(0, &final_weight);
        }

        // The C++ caught the const char* throws here and rethrew them as
        // TransducersAreNotAutomataException; the helpers now carry that
        // kind in their Error directly.
        let mut ctx = MergeContext {
            state_map: &mut state_map,
            agenda: &mut agenda,
            markers_added,
            list_symbols,
        };
        Self::find_matches_for_merge(graph, 0, merger, 0, &mut result, 0, &mut ctx)?;

        Ok(result)
    }
}
