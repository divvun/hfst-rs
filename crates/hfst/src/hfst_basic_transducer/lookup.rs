//! Lookup, and the checks for whether a lookup can go on forever.

use super::*;
use crate::hfst_data_types::{HfstOneLevelPath, HfstTwoLevelPath, HfstTwoLevelPaths, StringVector};
use crate::hfst_epsilon_handler::HfstEpsilonHandler;
use crate::hfst_flag_diacritics::FdOperation;
use crate::hfst_lookup_flag_diacritics::{FlagConfigurations, FlagDiacriticTable};
use crate::hfst_symbol_defs::{StringPair, StringSet, is_epsilon, is_identity, is_unknown};

/// Read-only parameters threaded unchanged through the recursive
/// [`HfstBasicTransducer::lookup_recursive`] walk, grouped so the recursion
/// passes a single reference instead of re-threading five arguments.
pub struct LookupParams<'a> {
    lookup_path: &'a StringVector,
    alphabet: &'a StringSet,
    max_epsilon_cycles: usize,
    max_weight: Option<&'a f32>,
    max_number: i32,
}

/// Mutable accumulators threaded through the recursive lookup walk: the result
/// set being built, the current path, the optional flag-diacritic trail, and
/// the non-progressing-cycle trap.
pub struct LookupAcc<'a> {
    results: &'a mut HfstTwoLevelPaths,
    path_so_far: &'a mut HfstTwoLevelPath,
    flag_diacritic_path: Option<&'a mut StringVector>,
    /// The flag configurations this walk has stood in.
    // [spec:hfst:req:general-lookup-termination.non-progressing-cycle]
    flag_configurations: FlagConfigurations,
    /// Which of them is in force where the walk stands, carried down the
    /// recursion rather than re-derived from the trail above: a flag arc knows
    /// what it does to the registers, and nothing else touches them. `None` when
    /// the caller did not ask for flags to be obeyed.
    // [spec:hfst:req:general-lookup-termination.non-progressing-cycle]
    flags: Option<usize>,
    /// Every situation the walk currently stands in without having consumed
    /// input to get there — pushed on entering a non-consuming arc, popped on
    /// leaving it, emptied for the descent below a consuming one. Named for the
    /// optimized-lookup engine's set of the same job.
    // [spec:hfst:req:general-lookup-termination.non-progressing-cycle]
    traversal_states: LookupTraversalStates,
}

/// The non-progressing-cycle trap's stack: a state paired with the flag
/// configuration in force on arrival there. A `Vec` used as a DFS stack rather
/// than a set: every push is matched by a pop, so its live contents are exactly
/// the situations on the current non-consuming path, and a repeat is refused
/// before it can be pushed, so the entries are always distinct.
// [spec:hfst:req:general-lookup-termination.non-progressing-cycle]
pub type LookupTraversalStates = Vec<(HfstState, Option<usize>)>;

impl HfstBasicTransducer {
    /// From state `s`, follow the transition whose label pair is
    /// `(isymbol, osymbol)`. When no exact transition matches but the state has an
    /// `@_IDENTITY_SYMBOL_@:@_IDENTITY_SYMBOL_@` identity transition AND the
    /// queried pair is an unknown identity (`isymbol == osymbol` and not in
    /// `known_symbols`), follow the identity transition instead. Returns the
    /// target state, or `None` when no transition applies. This is the pair-path
    /// recogniser step lifted from hfst-pair-test.
    pub fn pair_target_state(
        &self,
        s: HfstState,
        isymbol: &str,
        osymbol: &str,
        known_symbols: &BTreeSet<String>,
    ) -> Option<HfstState> {
        let mut identity_target: Option<HfstState> = None;
        for it in self
            .transitions(s)
            .expect("s is a valid state of this transducer")
            .iter()
        {
            if it.get_input_symbol(&self.coder) == isymbol
                && it.get_output_symbol(&self.coder) == osymbol
            {
                return Some(it.get_target_state());
            }
            if it.get_input_symbol(&self.coder) == crate::hfst_symbol_defs::internal_identity
                && it.get_output_symbol(&self.coder) == crate::hfst_symbol_defs::internal_identity
            {
                identity_target = Some(it.get_target_state());
            }
        }
        if isymbol == osymbol && !known_symbols.contains(isymbol) {
            identity_target
        } else {
            None
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-possible-flag-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-possible-flag-fn]
    // [spec:hfst:def:hfst-transition-graph.is-possible-flag-fn]
    // [spec:hfst:sem:hfst-transition-graph.is-possible-flag-fn]
    pub fn is_possible_flag(symbol: HfstSymbol, fds: &mut StringVector, obey_flags: bool) -> bool {
        if FdOperation::is_diacritic(&symbol) {
            let mut fd_t = FlagDiacriticTable::new();
            fds.push(symbol);
            if (!obey_flags) || fd_t.is_valid_string(fds) {
                return true;
            } else {
                fds.pop();
                return false;
            }
        }
        false
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-lookup-infinitely-ambiguous-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-lookup-infinitely-ambiguous-fn]
    // [spec:hfst:def:hfst-transition-graph.is-lookup-infinitely-ambiguous-fn]
    // [spec:hfst:sem:hfst-transition-graph.is-lookup-infinitely-ambiguous-fn]
    pub fn is_lookup_infinitely_ambiguous_recursive(
        &self,
        s: &HfstOneLevelPath,
        index: &mut u32,
        state: HfstState,
        epsilon_path_states: &mut BTreeSet<HfstState>,
        fds: &mut StringVector,
        obey_flags: bool,
    ) -> bool {
        // Whether the end of the lookup path s has been reached
        let mut only_epsilons = false;
        if s.second.len() as u32 == *index {
            only_epsilons = true;
        }

        let transitions = self
            .index(state)
            .expect("state is a valid state reached during the walk");
        for transition in transitions.iter() {
            // CASE 1: input epsilons (and flags) do not consume a path symbol.
            let in_sym = transition.get_input_symbol(&self.coder);
            let possible_flag = Self::is_possible_flag(in_sym.clone(), fds, obey_flags);
            if is_epsilon(&in_sym) || possible_flag {
                epsilon_path_states.insert(state);
                if epsilon_path_states.contains(&transition.get_target_state()) {
                    return true;
                }
                if self.is_lookup_infinitely_ambiguous_recursive(
                    s,
                    index,
                    transition.get_target_state(),
                    epsilon_path_states,
                    fds,
                    obey_flags,
                ) {
                    return true;
                }
                epsilon_path_states.remove(&state);
                if possible_flag {
                    fds.pop();
                }
            }
            // CASE 2: other input symbols consume a path symbol.
            else if !only_epsilons {
                let mut continu = false;
                if in_sym == s.second[*index as usize]
                    || ((in_sym == "@_UNKNOWN_SYMBOL_@" || in_sym == "@_IDENTITY_SYMBOL_@")
                        && !self.alphabet.contains(&s.second[*index as usize]))
                {
                    continu = true;
                }

                if continu {
                    *index += 1; // consume an input symbol
                    let mut empty_set: BTreeSet<HfstState> = BTreeSet::new();
                    if self.is_lookup_infinitely_ambiguous_recursive(
                        s,
                        index,
                        transition.get_target_state(),
                        &mut empty_set,
                        fds,
                        obey_flags,
                    ) {
                        return true;
                    }
                    *index -= 1; // add the input symbol back
                }
            }
        }
        false
    }

    pub fn is_lookup_infinitely_ambiguous_path(
        &self,
        s: &HfstOneLevelPath,
        obey_flags: bool,
    ) -> bool {
        let mut epsilon_path_states: BTreeSet<HfstState> = BTreeSet::new();
        epsilon_path_states.insert(0);
        let mut index: u32 = 0;
        let mut fds: StringVector = Vec::new();

        self.is_lookup_infinitely_ambiguous_recursive(
            s,
            &mut index,
            Self::INITIAL_STATE,
            &mut epsilon_path_states,
            &mut fds,
            obey_flags,
        )
    }

    pub fn is_lookup_infinitely_ambiguous_string_vector(
        &self,
        s: &StringVector,
        obey_flags: bool,
    ) -> bool {
        let mut epsilon_path_states: BTreeSet<HfstState> = BTreeSet::new();
        epsilon_path_states.insert(0);
        let mut index: u32 = 0;
        let path = HfstOneLevelPath {
            first: 0.0,
            second: s.clone(),
        };
        let mut fds: StringVector = Vec::new();

        self.is_lookup_infinitely_ambiguous_recursive(
            &path,
            &mut index,
            Self::INITIAL_STATE,
            &mut epsilon_path_states,
            &mut fds,
            obey_flags,
        )
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.push-back-to-two-level-path-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.push-back-to-two-level-path-fn]
    // [spec:hfst:def:hfst-transition-graph.push-back-to-two-level-path-fn]
    // [spec:hfst:sem:hfst-transition-graph.push-back-to-two-level-path-fn]
    pub fn push_back_to_two_level_path(
        path: &mut HfstTwoLevelPath,
        sp: &StringPair,
        weight: f32,
        fds_so_far: Option<&mut StringVector>,
    ) {
        path.second.push(sp.clone());
        path.first += weight;
        if let Some(fds) = fds_so_far
            && FdOperation::is_diacritic(&sp.0)
        {
            fds.push(sp.0.clone());
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.pop-back-from-two-level-path-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.pop-back-from-two-level-path-fn]
    // [spec:hfst:def:hfst-transition-graph.pop-back-from-two-level-path-fn]
    // [spec:hfst:sem:hfst-transition-graph.pop-back-from-two-level-path-fn]
    pub fn pop_back_from_two_level_path(
        path: &mut HfstTwoLevelPath,
        weight: f32,
        fds_so_far: Option<&mut StringVector>,
    ) {
        if let Some(fds) = fds_so_far {
            let sp = path
                .second
                .last()
                .expect("pop_back is only called on a non-empty path")
                .clone();
            if FdOperation::is_diacritic(&sp.0) {
                fds.pop();
            }
        }
        path.second.pop();
        path.first -= weight;
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-to-results-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-to-results-fn]
    // [spec:hfst:def:hfst-transition-graph.add-to-results-fn]
    // [spec:hfst:sem:hfst-transition-graph.add-to-results-fn]
    pub fn add_to_results(
        results: &mut HfstTwoLevelPaths,
        path_so_far: &mut HfstTwoLevelPath,
        final_weight: f32,
        max_weight: Option<&f32>,
    ) {
        path_so_far.first += final_weight;

        match max_weight {
            None => {
                results.insert(path_so_far.clone());
            }
            Some(mw) => {
                // Include unless the weight strictly exceeds the max. partial_cmp
                // keeps the C++ beam semantics for a NaN weight (not `Greater`,
                // so kept), which a plain `<=` would invert.
                if !matches!(
                    path_so_far.first.partial_cmp(mw),
                    Some(std::cmp::Ordering::Greater)
                ) {
                    results.insert(path_so_far.clone());
                }
            }
        }
        path_so_far.first -= final_weight;
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-possible-transition-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-possible-transition-fn]
    // [spec:hfst:def:hfst-transition-graph.is-possible-transition-fn]
    // [spec:hfst:sem:hfst-transition-graph.is-possible-transition-fn]
    pub fn is_possible_transition(
        transition: &HfstBasicTransition,
        lookup_path: &StringVector,
        lookup_index: u32,
        alphabet: &StringSet,
        input_symbol_consumed: &mut bool,
        fds_so_far: Option<&mut StringVector>,
        coder: &SymbolCoder,
    ) -> bool {
        let isymbol = transition.get_input_symbol(coder);

        // If we are not at the end of lookup_path,
        if (lookup_index != lookup_path.len() as u32)
            && (isymbol == lookup_path[lookup_index as usize]
                || ((is_identity(&isymbol) || is_unknown(&isymbol))
                    && !alphabet.contains(&lookup_path[lookup_index as usize])))
        {
            *input_symbol_consumed = true;
            return true;
        }
        // Epsilons and flag diacritics can always be taken.
        if is_epsilon(&isymbol) {
            *input_symbol_consumed = false;
            return true;
        }
        if FdOperation::is_diacritic(&isymbol) {
            match fds_so_far {
                None => {
                    *input_symbol_consumed = false;
                    return true;
                }
                Some(fds) => {
                    let mut fd_t = FlagDiacriticTable::new();
                    fds.push(isymbol.clone());
                    let valid = fd_t.is_valid_string(fds);
                    fds.pop();
                    if valid {
                        *input_symbol_consumed = false;
                        return true;
                    }
                }
            }
        }
        false
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.lookup-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.lookup-fn]
    // [spec:hfst:req:general-lookup-termination.non-progressing-cycle]
    // [spec:hfst:req:general-lookup-termination.progress-resets-the-trap]
    // [spec:hfst:req:general-lookup-termination.no-cycle-count-termination]
    // [spec:hfst:sem:general-lookup-termination.enumeration-divergence]
    pub fn lookup_recursive(
        &self,
        params: &LookupParams<'_>,
        acc: &mut LookupAcc<'_>,
        state: HfstState,
        mut lookup_index: u32,
        mut eh: HfstEpsilonHandler,
    ) {
        // Check the input-epsilon-cycle, weight and result-count limits.
        if !eh.can_continue(state) {
            return;
        }
        if let Some(mw) = params.max_weight
            && acc.path_so_far.first > *mw
        {
            return;
        }
        if params.max_number >= 0 && (params.max_number as usize) <= acc.results.len() {
            return;
        }

        // At the end of lookup_path and in a final state -> a valid result.
        if lookup_index == params.lookup_path.len() as u32 && self.is_final_state(state) {
            Self::add_to_results(
                acc.results,
                acc.path_so_far,
                self.get_final_weight(state)
                    .expect("state was confirmed final via is_final_state"),
                params.max_weight,
            );
        }

        let transitions = self
            .index(state)
            .expect("state is a valid state reached during the walk");
        for transition in transitions.iter() {
            let mut input_symbol_consumed = false;
            if Self::is_possible_transition(
                transition,
                params.lookup_path,
                lookup_index,
                params.alphabet,
                &mut input_symbol_consumed,
                acc.flag_diacritic_path.as_deref_mut(),
                &self.coder,
            ) {
                let target = transition.get_target_state();
                let tr_isym = transition.get_input_symbol(&self.coder);
                // An arc that consumes nothing and lands in a situation already
                // on this walk's non-consuming path closes a cycle that made no
                // progress: what a walk does next is settled by where it stands
                // and what its flag registers hold, so the sub-search below this
                // arc is the one already running above it, and following it can
                // only repeat work. Refuse it.
                let situation = match input_symbol_consumed {
                    true => None,
                    false => {
                        let arrival = acc
                            .flags
                            .map(|from| acc.flag_configurations.after(from, &tr_isym));
                        let here = (target, arrival);
                        if acc.traversal_states.contains(&here) {
                            continue;
                        }
                        Some(here)
                    }
                };

                let istr;
                let ostr;
                // identity symbol is replaced with the lookup symbol
                if is_identity(&tr_isym) {
                    istr = params.lookup_path[lookup_index as usize].clone();
                    ostr = istr.clone();
                } else {
                    if is_unknown(&tr_isym) {
                        istr = params.lookup_path[lookup_index as usize].clone();
                    } else {
                        istr = tr_isym;
                    }
                    ostr = transition.get_output_symbol(&self.coder);
                }

                Self::push_back_to_two_level_path(
                    acc.path_so_far,
                    &(istr, ostr),
                    transition.get_weight(),
                    acc.flag_diacritic_path.as_deref_mut(),
                );

                match situation {
                    // Input was consumed: the walk has progressed, so every
                    // situation recorded above is reachable again legitimately.
                    // The trap starts empty below this arc and is handed back on
                    // the way out.
                    None => {
                        lookup_index += 1;
                        let ehp = HfstEpsilonHandler::new(params.max_epsilon_cycles);
                        let outer = std::mem::take(&mut acc.traversal_states);
                        self.lookup_recursive(params, acc, target, lookup_index, ehp);
                        acc.traversal_states = outer;
                        lookup_index -= 1;
                    }
                    // Nothing consumed: record the situation for the descent and
                    // drop it again on the way out, so the trap always holds
                    // exactly the current path. A flag arc's rewrite of the
                    // registers travels with it, and is undone the same way.
                    Some(here) => {
                        eh.push_back(state);
                        let outer = std::mem::replace(&mut acc.flags, here.1);
                        acc.traversal_states.push(here);
                        self.lookup_recursive(params, acc, target, lookup_index, eh.clone());
                        acc.traversal_states.pop();
                        acc.flags = outer;
                    }
                }

                Self::pop_back_from_two_level_path(
                    acc.path_so_far,
                    transition.get_weight(),
                    acc.flag_diacritic_path.as_deref_mut(),
                );
            }
        }
    }

    /** @brief Look up 'lookup_path', collecting two-level paths into 'results'. */
    // [spec:hfst:def:hfst-transition-graph.lookup-fn]
    // [spec:hfst:sem:hfst-transition-graph.lookup-fn]
    pub fn lookup(
        &self,
        lookup_path: &StringVector,
        results: &mut HfstTwoLevelPaths,
        max_epsilon_cycles: Option<usize>,
        max_weight: Option<&f32>,
        max_number: i32,
        obey_flags: bool,
    ) {
        let state: HfstState = 0;
        let lookup_index: u32 = 0;
        let mut path_so_far = HfstTwoLevelPath {
            first: 0.0,
            second: Vec::new(),
        };
        let alphabet = self.get_alphabet().clone();
        let mut flag_diacritic_path: Option<StringVector> =
            if obey_flags { Some(Vec::new()) } else { None };

        // How many times the caller will let an input-epsilon cycle be
        // followed, or the standing default when they said nothing. This is a
        // limit on enumeration, not the walk's termination story — that is the
        // non-progressing-cycle trap in `lookup_recursive`, which holds however
        // large a cycle count is asked for.
        // [spec:hfst:req:general-lookup-termination.no-cycle-count-termination]
        let max_epsilon_cycles = max_epsilon_cycles.unwrap_or(100000);
        let params = LookupParams {
            lookup_path,
            alphabet: &alphabet,
            max_epsilon_cycles,
            max_weight,
            max_number,
        };
        let mut acc = LookupAcc {
            results,
            path_so_far: &mut path_so_far,
            flag_diacritic_path: flag_diacritic_path.as_mut(),
            flag_configurations: FlagConfigurations::new(),
            flags: obey_flags.then_some(FlagConfigurations::INITIAL),
            traversal_states: LookupTraversalStates::new(),
        };
        let eh = HfstEpsilonHandler::new(max_epsilon_cycles);
        self.lookup_recursive(&params, &mut acc, state, lookup_index, eh);
    }
}
