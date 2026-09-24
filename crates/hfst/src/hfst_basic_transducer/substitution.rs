//! Substituting symbols, symbol pairs and graphs, weight markers, free
//! insertion, and harmonization.

use super::*;
use crate::harmonize_unknown_and_identity_symbols::HarmonizeUnknownAndIdentitySymbols;
use crate::hfst_symbol_defs::{
    HfstSymbolPairSubstitutions, HfstSymbolSubstitutions, StringPairSet, StringSet, is_epsilon,
    is_identity, is_unknown,
};

// Where a substituting copy of a graph is inserted (origin/target state and
// weight). The C++ also cached a 'const_cast HfstBasicTransducer*' to the
// substituting graph; here the graph is passed to 'add_substitution' at apply
// time, so a mutate-then-read alias never has to be spelled.
pub struct substitution_data {
    pub origin_state: HfstState,
    pub target_state: HfstState,
    pub weight: WeightType,
}

impl substitution_data {
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.substitution-data.substitution-data-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.substitution-data.substitution-data-fn]
    // [spec:hfst:def:hfst-transition-graph.substitution-data.substitution-data-fn]
    // [spec:hfst:sem:hfst-transition-graph.substitution-data.substitution-data-fn]
    pub fn new(origin: HfstState, target: HfstState, weight: WeightType) -> Self {
        substitution_data {
            origin_state: origin,
            target_state: target,
            weight,
        }
    }
}

impl HfstBasicTransducer {
    // --- Substitution (private in-place helpers) ---

    /* In-place substitution of 'old_symbol' with 'new_symbol'. */
    fn substitute_in_place(
        &mut self,
        old_symbol: &HfstSymbol,
        new_symbol: &HfstSymbol,
        input_side: bool,
        output_side: bool,
    ) {
        for s in 0..self.state_vector.len() {
            for i in 0..self.state_vector[s].len() {
                let mut substituting_input_symbol =
                    self.state_vector[s][i].get_input_symbol(&self.coder);
                let mut substituting_output_symbol =
                    self.state_vector[s][i].get_output_symbol(&self.coder);
                let mut substitution_made = false;

                if input_side && substituting_input_symbol == *old_symbol {
                    substituting_input_symbol = new_symbol.clone();
                    substitution_made = true;
                }
                if output_side && substituting_output_symbol == *old_symbol {
                    substituting_output_symbol = new_symbol.clone();
                    substitution_made = true;
                }

                if substitution_made {
                    self.add_symbol_to_alphabet(new_symbol);
                    let target = self.state_vector[s][i].get_target_state();
                    let weight = self.state_vector[s][i].get_weight();
                    let tr = HfstBasicTransition::new_symbols(
                        target,
                        substituting_input_symbol,
                        substituting_output_symbol,
                        weight,
                        self.coder_mut(),
                    );
                    self.state_vector[s][i] = tr;
                }
            }
        }
    }

    /* In-place substitution by number vector: substitutions[from] = to. */
    fn substitute_in_place_numbers(
        &mut self,
        substitutions: &HfstNumberVector,
        no_substitution: u32,
    ) {
        for s in 0..self.state_vector.len() {
            for i in 0..self.state_vector[s].len() {
                let old_inumber = self.state_vector[s][i].get_input_number();
                let old_onumber = self.state_vector[s][i].get_output_number();

                let mut new_inumber = substitutions[old_inumber as usize];
                let mut new_onumber = substitutions[old_onumber as usize];

                if new_inumber != no_substitution || new_onumber != no_substitution {
                    if new_inumber != no_substitution {
                        let sym = self
                            .coder
                            .get_symbol(new_inumber)
                            .expect("substitution target number was interned by this coder");
                        self.add_symbol_to_alphabet(&sym);
                    } else {
                        new_inumber = old_inumber;
                    }

                    if new_onumber != no_substitution {
                        let sym = self
                            .coder
                            .get_symbol(new_onumber)
                            .expect("substitution target number was interned by this coder");
                        self.add_symbol_to_alphabet(&sym);
                    } else {
                        new_onumber = old_onumber;
                    }

                    let target = self.state_vector[s][i].get_target_state();
                    let weight = self.state_vector[s][i].get_weight();
                    let tr = HfstBasicTransition::new_numbers(
                        target,
                        new_inumber,
                        new_onumber,
                        weight,
                        false,
                    );
                    self.state_vector[s][i] = tr;
                }
            }
        }
    }

    /* In-place substitution by number-pair map. */
    fn substitute_in_place_number_pairs(&mut self, substitutions: &HfstNumberPairSubstitutions) {
        for s in 0..self.state_vector.len() {
            for i in 0..self.state_vector[s].len() {
                let old_number_pair = (
                    self.state_vector[s][i].get_input_number(),
                    self.state_vector[s][i].get_output_number(),
                );

                if let Some(subst) = substitutions.get(&old_number_pair) {
                    let new_input_number = subst.0;
                    let new_output_number = subst.1;

                    let in_sym = self
                        .coder
                        .get_symbol(new_input_number)
                        .expect("substitution target number was interned by this coder");
                    self.add_symbol_to_alphabet(&in_sym);
                    let out_sym = self
                        .coder
                        .get_symbol(new_output_number)
                        .expect("substitution target number was interned by this coder");
                    self.add_symbol_to_alphabet(&out_sym);

                    let target = self.state_vector[s][i].get_target_state();
                    let weight = self.state_vector[s][i].get_weight();
                    let tr = HfstBasicTransition::new_numbers(
                        target,
                        new_input_number,
                        new_output_number,
                        weight,
                        false,
                    );
                    self.state_vector[s][i] = tr;
                }
            }
        }
    }

    /* In-place removal of all transitions equivalent to 'sp'. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-transitions-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-transitions-fn]
    // [spec:hfst:def:hfst-transition-graph.remove-transitions-fn]
    // [spec:hfst:sem:hfst-transition-graph.remove-transitions-fn]
    pub fn remove_transitions(&mut self, sp: &HfstSymbolPair) {
        let in_match = self.coder.get_number(&sp.0);
        let out_match = self.coder.get_number(&sp.1);

        let mut in_match_used = false;
        let mut out_match_used = false;

        for s in 0..self.state_vector.len() {
            // C++ 'for (i=0; i<size(); i++)' with erase but no 'i--': after an
            // erase the shifted element is skipped — bug preserved.
            let mut i = 0;
            while i < self.state_vector[s].len() {
                let in_tr = self.state_vector[s][i].get_input_number();
                let out_tr = self.state_vector[s][i].get_output_number();
                if in_tr == in_match && out_tr == out_match {
                    self.state_vector[s].remove(i);
                } else {
                    if in_tr == in_match || out_tr == in_match {
                        in_match_used = true;
                    }
                    if in_tr == out_match || out_tr == out_match {
                        out_match_used = true;
                    }
                }
                i += 1;
            }
        }

        if !in_match_used {
            self.alphabet.remove(&sp.0);
        }
        if !out_match_used {
            self.alphabet.remove(&sp.1);
        }
    }

    /* In-place substitution of 'old_sp' with the set 'new_sps'. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.substitute-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.substitute-fn]
    // [spec:hfst:def:hfst-transition-graph.substitute-fn]
    // [spec:hfst:sem:hfst-transition-graph.substitute-fn]
    fn substitute_in_place_pair_set(
        &mut self,
        old_sp: &HfstSymbolPair,
        new_sps: &HfstSymbolPairSet,
    ) {
        if new_sps.is_empty() {
            self.remove_transitions(old_sp);
            return;
        }

        let old_input_number = self.coder.get_number(&old_sp.0);
        let old_output_number = self.coder.get_number(&old_sp.1);

        let mut substitution_performed = false;

        for s in 0..self.state_vector.len() {
            let mut new_transitions: HfstBasicTransitions = Vec::new();

            for i in 0..self.state_vector[s].len() {
                if self.state_vector[s][i].get_input_number() == old_input_number
                    && self.state_vector[s][i].get_output_number() == old_output_number
                {
                    substitution_performed = true;
                    let target = self.state_vector[s][i].get_target_state();
                    let weight = self.state_vector[s][i].get_weight();

                    // change the transition to the first substituting pair
                    let first = new_sps
                        .iter()
                        .next()
                        .expect("new_sps is non-empty; the empty case returned early");
                    let first_in = self.coder.get_number(&first.0);
                    let first_out = self.coder.get_number(&first.1);
                    let tr =
                        HfstBasicTransition::new_numbers(target, first_in, first_out, weight, true);
                    self.state_vector[s][i] = tr;

                    // schedule the rest (C++ iterates from begin, so all of
                    // new_sps incl. the first are appended).
                    for sp in new_sps.iter() {
                        let sp_in = self.coder.get_number(&sp.0);
                        let sp_out = self.coder.get_number(&sp.1);
                        let tr2 =
                            HfstBasicTransition::new_numbers(target, sp_in, sp_out, weight, true);
                        new_transitions.push(tr2);
                    }
                }
            }

            for new_transition in new_transitions.iter() {
                self.state_vector[s].push(new_transition.clone());
            }
        }

        if substitution_performed {
            self.add_symbols_to_alphabet_pair_set(new_sps);
        }

        let mut syms: BTreeSet<u32> = BTreeSet::new();
        syms.insert(old_input_number);
        syms.insert(old_output_number);
        self.prune_alphabet_after_substitution(&syms);
    }

    /* In-place substitution by a user function. */
    fn substitute_in_place_func(
        &mut self,
        func: impl Fn(&HfstSymbolPair, &mut HfstSymbolPairSet) -> bool,
    ) -> crate::error::Result<()> {
        for s in 0..self.state_vector.len() {
            let mut new_transitions: HfstBasicTransitions = Vec::new();

            for i in 0..self.state_vector[s].len() {
                let transition_symbol_pair = (
                    self.state_vector[s][i].get_input_symbol(&self.coder),
                    self.state_vector[s][i].get_output_symbol(&self.coder),
                );
                let mut substituting_transitions: HfstSymbolPairSet = BTreeSet::new();

                // C++ wraps this in try/catch(HfstException){throw e;} — a no-op
                // rethrow, so a thrown exception just propagates.
                let perform_substitution =
                    func(&transition_symbol_pair, &mut substituting_transitions);
                if perform_substitution {
                    let target = self.state_vector[s][i].get_target_state();
                    let weight = self.state_vector[s][i].get_weight();

                    let (fi, fo) = {
                        let first = substituting_transitions.iter().next().expect(
                            "callback populates substituting_transitions when it returns true",
                        );
                        (first.0.clone(), first.1.clone())
                    };
                    if !HfstTropicalTransducerTransitionData::is_valid_symbol(&fi)
                        || !HfstTropicalTransducerTransitionData::is_valid_symbol(&fo)
                    {
                        crate::bail!(EmptyString, "HfstBasicTransducer::substitute");
                    }

                    let tr = HfstBasicTransition::new_symbols(
                        target,
                        fi.clone(),
                        fo.clone(),
                        weight,
                        self.coder_mut(),
                    );
                    self.state_vector[s][i] = tr;

                    self.add_symbol_to_alphabet(&fi);
                    self.add_symbol_to_alphabet(&fo);

                    for sp in substituting_transitions.iter() {
                        if !HfstTropicalTransducerTransitionData::is_valid_symbol(&sp.0)
                            || !HfstTropicalTransducerTransitionData::is_valid_symbol(&sp.1)
                        {
                            crate::bail!(EmptyString, "HfstBasicTransducer::substitute");
                        }
                        let tr2 = HfstBasicTransition::new_symbols(
                            target,
                            sp.0.clone(),
                            sp.1.clone(),
                            weight,
                            self.coder_mut(),
                        );
                        new_transitions.push(tr2);
                        self.add_symbol_to_alphabet(&sp.0);
                        self.add_symbol_to_alphabet(&sp.1);
                    }
                }
            }

            for new_transition in new_transitions.iter() {
                self.state_vector[s].push(new_transition.clone());
            }
        }
        Ok(())
    }

    // --- Substitution (public) ---

    /** @brief Substitute 'old_symbol' with 'new_symbol' in all transitions. */
    pub fn substitute_symbol(
        &mut self,
        old_symbol: &HfstSymbol,
        new_symbol: &HfstSymbol,
        input_side: bool,
        output_side: bool,
    ) -> crate::error::Result<&mut Self> {
        if !HfstTropicalTransducerTransitionData::is_valid_symbol(old_symbol)
            || !HfstTropicalTransducerTransitionData::is_valid_symbol(new_symbol)
        {
            crate::bail!(EmptyString, "HfstBasicTransducer::substitute");
        }

        // If a symbol is substituted with itself, do nothing.
        if old_symbol == new_symbol {
            return Ok(self);
        }
        // If the old symbol is not known to the graph, do nothing.
        if !self.alphabet.contains(old_symbol) {
            return Ok(self);
        }

        // Remove the substituted symbol from the alphabet if both sides.
        if input_side
            && output_side
            && !is_epsilon(old_symbol)
            && !is_unknown(old_symbol)
            && !is_identity(old_symbol)
        {
            self.alphabet.remove(old_symbol);
        }
        self.alphabet.insert(new_symbol.clone());

        self.substitute_in_place(old_symbol, new_symbol, input_side, output_side);

        Ok(self)
    }

    /** @brief Substitute all transitions as defined in 'substitutions'. */
    pub fn substitute_symbol_substitutions(
        &mut self,
        substitutions: &HfstSymbolSubstitutions,
    ) -> &mut Self {
        // add symbols to the global HfstTransition alphabet
        for (first, second) in substitutions.iter() {
            let _ = self.get_symbol_number(first);
            let _ = self.get_symbol_number(second);
        }

        // number_substitutions[from_symbol] = to_symbol
        let mut number_substitutions: Vec<u32> = Vec::new();
        let st: usize = self.coder.get_max_number() as usize + number_substitutions.len() + 1;
        let no_substitution = u32::try_from(st).expect("value out of u32 range");

        number_substitutions.resize((self.coder.get_max_number() + 1) as usize, no_substitution);
        for (first, second) in substitutions.iter() {
            let from_symbol = self.get_symbol_number(first);
            let to_symbol = self.get_symbol_number(second);
            number_substitutions[from_symbol as usize] = to_symbol;
        }

        self.substitute_in_place_numbers(&number_substitutions, no_substitution);

        self
    }

    /** @brief Substitute transitions x:y -> X:Y as defined in 'substitutions'. */
    pub fn substitute_symbol_pair_substitutions(
        &mut self,
        substitutions: &HfstSymbolPairSubstitutions,
    ) -> &mut Self {
        // Convert from symbols to numbers
        let mut number_substitutions: HfstNumberPairSubstitutions = BTreeMap::new();
        for (from, to) in substitutions.iter() {
            let from_transition = (
                self.get_symbol_number(&from.0),
                self.get_symbol_number(&from.1),
            );
            let to_transition = (self.get_symbol_number(&to.0), self.get_symbol_number(&to.1));
            number_substitutions.insert(from_transition, to_transition);
        }

        self.substitute_in_place_number_pairs(&number_substitutions);

        self
    }

    /** @brief Substitute all transitions 'sp' with a set of transitions 'sps'. */
    pub fn substitute_pair_with_set(
        &mut self,
        sp: &HfstSymbolPair,
        sps: &HfstSymbolPairSet,
    ) -> crate::error::Result<&mut Self> {
        if !HfstTropicalTransducerTransitionData::is_valid_symbol(&sp.0)
            || !HfstTropicalTransducerTransitionData::is_valid_symbol(&sp.1)
        {
            crate::bail!(EmptyString, "HfstBasicTransducer::substitute");
        }

        for sp in sps.iter() {
            if !HfstTropicalTransducerTransitionData::is_valid_symbol(&sp.0)
                || !HfstTropicalTransducerTransitionData::is_valid_symbol(&sp.1)
            {
                crate::bail!(EmptyString, "HfstBasicTransducer::substitute");
            }
        }

        self.substitute_in_place_pair_set(sp, sps);

        Ok(self)
    }

    /** @brief Substitute all transitions 'old_pair' with 'new_pair'. */
    pub fn substitute_pair(
        &mut self,
        old_pair: &HfstSymbolPair,
        new_pair: &HfstSymbolPair,
    ) -> crate::error::Result<&mut Self> {
        if !HfstTropicalTransducerTransitionData::is_valid_symbol(&old_pair.0)
            || !HfstTropicalTransducerTransitionData::is_valid_symbol(&new_pair.0)
            || !HfstTropicalTransducerTransitionData::is_valid_symbol(&old_pair.1)
            || !HfstTropicalTransducerTransitionData::is_valid_symbol(&new_pair.1)
        {
            crate::bail!(EmptyString, "HfstBasicTransducer::substitute");
        }

        let mut new_pair_set: StringPairSet = BTreeSet::new();
        new_pair_set.insert(new_pair.clone());
        self.substitute_in_place_pair_set(old_pair, &new_pair_set);

        Ok(self)
    }

    /** @brief Substitute all transitions with a set defined by function 'func'. */
    pub fn substitute_with_func(
        &mut self,
        func: impl Fn(&HfstSymbolPair, &mut HfstSymbolPairSet) -> bool,
    ) -> crate::error::Result<&mut Self> {
        self.substitute_in_place_func(func)?;
        Ok(self)
    }

    /** @brief Substitute transitions 'sp' with a copy of 'graph'. */
    pub fn substitute_pair_with_graph(
        &mut self,
        sp: &HfstSymbolPair,
        graph: &HfstBasicTransducer,
    ) -> crate::error::Result<&mut Self> {
        if !(HfstTropicalTransducerTransitionData::is_valid_symbol(&sp.0)
            && HfstTropicalTransducerTransitionData::is_valid_symbol(&sp.1))
        {
            crate::bail!(
                EmptyString,
                "HfstBasicTransducer::substitute(const HfstSymbolPair&, const HfstBasicTransducer&)"
            );
        }

        // If neither symbol is known to the graph, do nothing.
        if !self.alphabet.contains(&sp.0) && !self.alphabet.contains(&sp.1) {
            return Ok(self);
        }

        let mut substitutions: Vec<substitution_data> = Vec::new();

        for s in 0..self.state_vector.len() {
            // The transitions that are substituted, i.e. removed.
            let mut old_indices: Vec<usize> = Vec::new();

            for i in 0..self.state_vector[s].len() {
                let data = self.state_vector[s][i].get_transition_data().clone();
                if data.get_input_symbol(&self.coder) == sp.0
                    && data.get_output_symbol(&self.coder) == sp.1
                {
                    substitutions.push(substitution_data::new(
                        s as HfstState,
                        self.state_vector[s][i].get_target_state(),
                        data.get_weight(),
                    ));
                    old_indices.push(i);
                }
            }
            // C++ erases collected forward iterators (UB after the first erase);
            // the evident intent is to remove all matches — done in reverse.
            for &i in old_indices.iter().rev() {
                self.state_vector[s].remove(i);
            }
        }

        // Every substitution here inserts the same graph.
        for substitution in substitutions.iter() {
            self.add_substitution(substitution, graph);
        }
        Ok(self)
    }

    /* Add a copy of the substituting graph with epsilon transitions between
    states and with weight as defined in `sub`. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-substitution-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-substitution-fn]
    // [spec:hfst:def:hfst-transition-graph.add-substitution-fn]
    // [spec:hfst:sem:hfst-transition-graph.add-substitution-fn]
    pub fn add_substitution(&mut self, sub: &substitution_data, graph_ref: &HfstBasicTransducer) {
        // Epsilon transition to initial state of the substituting graph.
        let s = self.add_state_new();
        let epsilon_transition = HfstBasicTransition::new_symbols(
            s,
            HfstTropicalTransducerTransitionData::get_epsilon(),
            HfstTropicalTransducerTransitionData::get_epsilon(),
            sub.weight,
            self.coder_mut(),
        );
        self.add_transition(sub.origin_state, &epsilon_transition, true);

        let offset = s;

        // The substituting graph is supplied by the caller *after* any
        // harmonization has finished, so the mutate-then-read ordering the C++
        // handled with a cached 'const_cast' pointer needs no alias here.
        // The substituting graph has its own coder; resolve its arc symbols
        // through *its* coding, then re-intern them into this graph's coder.
        let graph_coder = graph_ref.coder();
        for (source_state, it) in graph_ref.state_vector.iter().enumerate() {
            let source_state = source_state as HfstState;
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

        // Epsilon transitions from final states of the graph.
        for (k, v) in graph_ref.final_weight_map.iter() {
            let epsilon_transition = HfstBasicTransition::new_symbols(
                sub.target_state,
                HfstTropicalTransducerTransitionData::get_epsilon(),
                HfstTropicalTransducerTransitionData::get_epsilon(),
                *v,
                self.coder_mut(),
            );
            self.add_transition(*k + offset, &epsilon_transition, true);
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.weight2marker-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.weight2marker-fn]
    //
    // The C++ uses 'ostringstream <<' (default float text); Rust's '{}' differs
    // textually but round-trips with marker2weight's parse internally.
    // [spec:hfst:def:hfst-transition-graph.std.string-weight2marker-fn]
    // [spec:hfst:sem:hfst-transition-graph.std.string-weight2marker-fn]
    pub fn weight2marker(weight: f32) -> HfstSymbol {
        HfstSymbol::from(format!("@{}@", weight))
    }

    /** @brief Replace each non-zero transition weight with a '@w@' marker arc. */
    pub fn substitute_weights_with_markers(&mut self) -> &mut Self {
        let limit = self.state_vector.len();
        for state in 0..limit {
            let mut old_indices: Vec<usize> = Vec::new();
            let mut new_transitions: Vec<HfstBasicTransition> = Vec::new();

            for i in 0..self.state_vector[state].len() {
                let data = self.state_vector[state][i].get_transition_data().clone();
                if data.get_weight() != 0.0 {
                    let target = self.state_vector[state][i].get_target_state();
                    let isym = data.get_input_symbol(&self.coder);
                    let osym = data.get_output_symbol(&self.coder);
                    new_transitions.push(HfstBasicTransition::new_symbols(
                        target,
                        isym,
                        osym,
                        data.get_weight(),
                        self.coder_mut(),
                    ));
                    old_indices.push(i);
                }
            }

            // Remove the substituted transitions (stack LIFO = reverse position).
            for &i in old_indices.iter().rev() {
                self.state_vector[state].remove(i);
            }

            // Add the substituting transitions.
            for it in new_transitions.iter() {
                let new_state = self.add_state_new();
                let marker = Self::weight2marker(it.get_weight());
                let it_target = it.get_target_state();
                let it_isym = it.get_input_symbol(&self.coder);
                let it_osym = it.get_output_symbol(&self.coder);
                let marker_transition = HfstBasicTransition::new_symbols(
                    it_target,
                    marker.clone(),
                    marker,
                    0.0,
                    self.coder_mut(),
                );
                let new_transition = HfstBasicTransition::new_symbols(
                    new_state,
                    it_isym,
                    it_osym,
                    0.0,
                    self.coder_mut(),
                );
                let source_state = u32::try_from(state).expect("value out of u32 range");
                self.add_transition(source_state, &new_transition, true);
                self.add_transition(new_state, &marker_transition, true);
            }
        }

        // Go through the final states (snapshot first; the C++ iterates the map
        // while inserting weight-0 finals that it then skips).
        let mut final_states_to_remove: BTreeSet<HfstState> = BTreeSet::new();
        let finals: Vec<(HfstState, f32)> = self
            .final_weight_map
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect();
        for (k, v) in finals {
            if v != 0.0 {
                let new_state = self.add_state_new();
                self.set_final_weight(new_state, &0.0);
                let marker = Self::weight2marker(v);
                let epsilon_transition = HfstBasicTransition::new_symbols(
                    new_state,
                    marker.clone(),
                    marker,
                    0.0,
                    self.coder_mut(),
                );
                self.add_transition(k, &epsilon_transition, true);
                final_states_to_remove.insert(k);
            }
        }
        for it in final_states_to_remove.iter() {
            self.final_weight_map.remove(it);
        }

        self
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.marker2weight-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.marker2weight-fn]
    // [spec:hfst:def:hfst-transition-graph.marker2weight-fn]
    // [spec:hfst:sem:hfst-transition-graph.marker2weight-fn]
    pub fn marker2weight(str: &str) -> Option<f32> {
        if str.len() < 3 {
            return None;
        }
        let bytes = str.as_bytes();
        if bytes[0] != b'@' || bytes[str.len() - 1] != b'@' {
            return None;
        }
        let weight_string = &str[1..str.len() - 1];
        weight_string.parse::<f32>().ok()
    }

    /** @brief Replace '@w@' marker arcs with transition weights. */
    pub fn substitute_markers_with_weights(&mut self) -> &mut Self {
        let limit = self.state_vector.len();
        for state in 0..limit {
            let mut old_indices: Vec<usize> = Vec::new();
            let mut new_transitions: Vec<HfstBasicTransition> = Vec::new();

            for i in 0..self.state_vector[state].len() {
                let data = self.state_vector[state][i].get_transition_data().clone();
                let isym = data.get_input_symbol(&self.coder);
                let osym = data.get_output_symbol(&self.coder);
                match (Self::marker2weight(&isym), Self::marker2weight(&osym)) {
                    (None, Some(weight)) => {
                        let target = self.state_vector[state][i].get_target_state();
                        new_transitions.push(HfstBasicTransition::new_symbols(
                            target,
                            isym,
                            HfstSymbol::new_static(crate::hfst_symbol_defs::internal_epsilon),
                            weight,
                            self.coder_mut(),
                        ));
                        old_indices.push(i);
                    }
                    (Some(_), Some(_)) => {
                        old_indices.push(i);
                    }
                    _ => {}
                }
            }

            for &i in old_indices.iter().rev() {
                self.state_vector[state].remove(i);
            }
            for new_transition in new_transitions.iter() {
                self.state_vector[state].push(new_transition.clone());
            }
        }

        // Remove weight-marker symbols from the alphabet.
        let mut weight_markers: Vec<HfstSymbol> = Vec::new();
        for it in self.alphabet.iter() {
            if Self::marker2weight(it).is_some() {
                weight_markers.push(it.clone());
            }
        }
        for it in weight_markers.iter() {
            self.alphabet.remove(it);
        }

        self
    }

    // --- Insert freely ---

    /** @brief Insert freely any number of 'symbol_pair' with weight 'weight'. */
    pub fn insert_freely_pair(
        &mut self,
        symbol_pair: &HfstSymbolPair,
        weight: WeightType,
    ) -> crate::error::Result<&mut Self> {
        if !(HfstTropicalTransducerTransitionData::is_valid_symbol(&symbol_pair.0)
            && HfstTropicalTransducerTransitionData::is_valid_symbol(&symbol_pair.1))
        {
            crate::bail!(
                EmptyString,
                "HfstBasicTransducer::insert_freely(const HfstSymbolPair&, W)"
            );
        }

        self.alphabet.insert(symbol_pair.0.clone());
        self.alphabet.insert(symbol_pair.1.clone());

        for s in 0..self.state_vector.len() {
            // self-loop on each state
            let tr = HfstBasicTransition::new_symbols(
                s as HfstState,
                symbol_pair.0.clone(),
                symbol_pair.1.clone(),
                weight,
                self.coder_mut(),
            );
            self.state_vector[s].push(tr);
        }
        Ok(self)
    }

    /** @brief Insert freely any of the pairs in 'symbol_pairs'. */
    pub fn insert_freely_set(
        &mut self,
        symbol_pairs: &HfstSymbolPairSet,
        weight: WeightType,
    ) -> crate::error::Result<&mut Self> {
        for symbol_pair in symbol_pairs.iter() {
            if !(HfstTropicalTransducerTransitionData::is_valid_symbol(&symbol_pair.0)
                && HfstTropicalTransducerTransitionData::is_valid_symbol(&symbol_pair.1))
            {
                crate::bail!(
                    EmptyString,
                    "HfstBasicTransducer::insert_freely(const HfstSymbolPairSet&, W)"
                );
            }
            self.alphabet.insert(symbol_pair.0.clone());
            self.alphabet.insert(symbol_pair.1.clone());
        }

        for s in 0..self.state_vector.len() {
            for symbol_pair in symbol_pairs.iter() {
                let tr = HfstBasicTransition::new_symbols(
                    s as HfstState,
                    symbol_pair.0.clone(),
                    symbol_pair.1.clone(),
                    weight,
                    self.coder_mut(),
                );
                self.state_vector[s].push(tr);
            }
        }
        Ok(self)
    }

    /** @brief Insert freely any number of 'graph' in this graph. */
    pub fn insert_freely_graph(
        &mut self,
        graph: &HfstBasicTransducer,
    ) -> crate::error::Result<&mut Self> {
        let marker_this = HfstTropicalTransducerTransitionData::get_marker(&self.alphabet);
        let marker_graph = HfstTropicalTransducerTransitionData::get_marker(&self.alphabet);
        let mut marker = marker_this;
        if marker_graph > marker {
            marker = marker_graph;
        }

        // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.marker-pair-fn]
        // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.marker-pair-fn]
        // [spec:hfst:def:hfst-transition-graph.marker-pair-fn]
        // [spec:hfst:sem:hfst-transition-graph.marker-pair-fn]
        let marker_pair = (marker.clone(), marker.clone());
        self.insert_freely_pair(&marker_pair, 0.0)?;
        self.substitute_pair_with_graph(&marker_pair, graph)?;
        self.alphabet.remove(&marker); // (C++ flags this line as needing a fix)

        Ok(self)
    }

    // --- Harmonization ---

    /** @brief Harmonize this graph and 'another' (expand unknown/identity). */
    pub fn harmonize(&mut self, another: &mut HfstBasicTransducer) -> &mut Self {
        let _foo = HarmonizeUnknownAndIdentitySymbols::new(self, another);
        self
    }

    /** @brief Substitute symbols with transducers as defined in 'substitution_map'. */
    pub fn substitute_subst_map(
        &mut self,
        substitution_map: &mut SubstMap,
        harmonize: bool,
    ) -> crate::error::Result<&mut Self> {
        for first in substitution_map.keys() {
            if !HfstTropicalTransducerTransitionData::is_valid_symbol(first) {
                crate::bail!(
                    EmptyString,
                    "HfstBasicTransducer::substitute (const std::map<HfstSymbol, HfstBasicTransducer> &)"
                );
            }
        }
        let symbol_found = substitution_map
            .iter()
            .any(|(first, _)| self.alphabet.contains(first));

        // If none of the symbols is known to the graph, do nothing.
        if !symbol_found {
            return Ok(self);
        }

        let mut substitutions_performed_for_symbols: StringSet = BTreeSet::new();
        // Each substitution remembers the map key it came from; the graph itself
        // is fetched from the map after harmonization, avoiding the C++ pointer
        // aliased across the intervening 'get_mut'.
        let mut substitutions: Vec<(substitution_data, HfstSymbol)> = Vec::new();

        for s in 0..self.state_vector.len() {
            let mut old_indices: Vec<usize> = Vec::new();

            for j in 0..self.state_vector[s].len() {
                let istr = self.state_vector[s][j].get_input_symbol(&self.coder);
                let ostr = self.state_vector[s][j].get_output_symbol(&self.coder);
                let map_in_found = substitution_map.contains_key(&istr);
                let map_out_found = substitution_map.contains_key(&ostr);

                if !map_in_found && !map_out_found {
                    // nothing
                } else if istr != ostr {
                    let msg = "symbol to be substituted must not occur only on one side of \
                               transition"
                        .to_string();
                    crate::bail!(Hfst, msg);
                } else {
                    let target = self.state_vector[s][j].get_target_state();
                    let weight = self.state_vector[s][j].get_weight();
                    substitutions.push((
                        substitution_data::new(s as HfstState, target, weight),
                        istr.clone(),
                    ));
                    old_indices.push(j);
                    substitutions_performed_for_symbols.insert(istr.clone());
                }
            }
            for &j in old_indices.iter().rev() {
                self.state_vector[s].remove(j);
            }
        }

        // Remove all symbols that were substituted.
        for sym_it in substitutions_performed_for_symbols.iter() {
            if sym_it != "@_EPSILON_SYMBOL_@"
                && sym_it != "@_UNKNOWN_SYMBOL_@"
                && sym_it != "@_IDENTITY_SYMBOL_@"
            {
                self.remove_symbol_from_alphabet(sym_it);
            }
        }

        // Harmonize the resulting and the substituting graphs, if needed.
        if harmonize {
            for sym_it in substitutions_performed_for_symbols.iter() {
                let graph = substitution_map
                    .get_mut(sym_it)
                    .expect("sym_it is a key drawn from substitution_map");
                self.harmonize(graph);
            }
        }

        // Add the substitutions, reading each now-harmonized graph back out of
        // the map by its key (a fresh shared borrow, after all mutation).
        for (substitution, sym) in substitutions.iter() {
            let graph = substitution_map
                .get(sym)
                .expect("sym was taken from substitution_map keys");
            self.add_substitution(substitution, graph);
        }
        Ok(self)
    }
}
