//! Table reads and the single-step moves the lookup engines walk with.

use super::*;

impl<T: TransducerTablesInterface> Transducer<T> {
    // The C++ 'get_index'/'get_transition' returned base-class pointers; with
    // the tables monomorphized, expose the scalar reads directly instead.
    #[inline]
    pub fn get_index_input(&self, i: TransitionTableIndex) -> SymbolNumber {
        self.tbl().get_index_input(i)
    }
    #[inline]
    pub fn get_index_target(&self, i: TransitionTableIndex) -> TransitionTableIndex {
        self.tbl().get_index_target(i)
    }
    #[inline]
    pub fn index_matches(&self, i: TransitionTableIndex, s: SymbolNumber) -> bool {
        self.tbl().index_matches(i, s)
    }
    #[inline]
    pub fn get_transition_input(&self, i: TransitionTableIndex) -> SymbolNumber {
        self.tbl().get_transition_input(i)
    }
    #[inline]
    pub fn get_transition_output(&self, i: TransitionTableIndex) -> SymbolNumber {
        self.tbl().get_transition_output(i)
    }
    #[inline]
    pub fn get_transition_target(&self, i: TransitionTableIndex) -> TransitionTableIndex {
        self.tbl().get_transition_target(i)
    }
    #[inline]
    pub fn get_transition_weight(&self, i: TransitionTableIndex) -> Weight {
        self.tbl().get_weight(i)
    }
    #[inline]
    pub fn transition_matches(&self, i: TransitionTableIndex, s: SymbolNumber) -> bool {
        self.tbl().transition_matches(i, s)
    }
    #[inline]
    pub fn get_index_finality(&self, i: TransitionTableIndex) -> bool {
        self.tbl().get_index_finality(i)
    }
    #[inline]
    pub fn get_transition_finality(&self, i: TransitionTableIndex) -> bool {
        self.tbl().get_transition_finality(i)
    }
    #[inline]
    pub fn get_index_final_weight(&self, i: TransitionTableIndex) -> Weight {
        self.tbl().get_final_weight(i)
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.final-index-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.final-index-fn]
    pub fn final_index(&self, i: TransitionTableIndex) -> bool {
        if indexes_transition_table(i) {
            self.tbl().get_transition_finality(i)
        } else {
            self.tbl().get_index_finality(i)
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.get-transitions-from-state-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.get-transitions-from-state-fn]
    pub fn get_transitions_from_state(
        &self,
        state_index: TransitionTableIndex,
    ) -> TransitionTableIndexSet {
        let mut transitions = TransitionTableIndexSet::new();

        if indexes_transition_index_table(state_index) {
            // for each input symbol that has a transition from this state
            for symbol in 0..self.hdr().symbol_count() {
                // There may be flags at index 0 even if there aren't any
                // epsilons, so those have to be checked for
                if self.alph().is_like_epsilon(symbol) {
                    let mut transition_i = self.get_index_target(state_index + 1);
                    if !self.index_matches(state_index + 1, 0) {
                        continue;
                    }
                    loop {
                        let input = self.get_transition_input(transition_i);
                        if self.transition_matches(transition_i, symbol) {
                            transitions.insert(transition_i);
                        // There could still be epsilons here, or other flags
                        } else if input != 0 && !self.alph().is_like_epsilon(input) {
                            break;
                        }
                        transition_i += 1;
                    }
                } else {
                    // not a flag.
                    // The C++ reads get_index(state_index+1+symbol) unconditionally;
                    // for output-only symbols (whose number can reach symbol_count)
                    // this indexes past the index table — a benign out-of-bounds read
                    // in C++ that yields a non-matching entry. Guard it to the
                    // intended "no entry beyond the table => no transitions" semantics.
                    if state_index + 1 + symbol as u32 >= self.hdr().index_table_size() {
                        continue;
                    }
                    let test_input = self.get_index_input(state_index + 1 + symbol as u32);
                    let test_target = self.get_index_target(state_index + 1 + symbol as u32);
                    if self.index_matches(state_index + 1 + symbol as u32, symbol) {
                        // there are one or more transitions with this input
                        // symbol, starting at test_transition_index.get_target()
                        let mut transition_i = test_target;
                        loop {
                            if self.transition_matches(transition_i, test_input) {
                                transitions.insert(transition_i);
                            } else {
                                break;
                            }
                            transition_i += 1;
                        }
                    }
                }
            }
        } else {
            // indexes transition table
            let in_sym = self.get_transition_input(state_index);
            let out_sym = self.get_transition_output(state_index);
            if in_sym != NO_SYMBOL_NUMBER || out_sym != NO_SYMBOL_NUMBER {
                // Oops
                panic!("get_transitions_from_state: malformed transition boundary");
            }

            let mut transition_i = state_index + 1;
            loop {
                if self.get_transition_input(transition_i) != NO_SYMBOL_NUMBER {
                    transitions.insert(transition_i);
                } else {
                    break;
                }
                transition_i += 1;
            }
        }
        transitions
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.next-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.next-fn]
    pub fn next(&self, i: TransitionTableIndex, symbol: SymbolNumber) -> TransitionTableIndex {
        if i >= TRANSITION_TARGET_TABLE_START {
            i - TRANSITION_TARGET_TABLE_START + 1
        } else {
            self.get_index_target(i + 1 + symbol as u32) - TRANSITION_TARGET_TABLE_START
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.next-e-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.next-e-fn]
    // (declared in transducer.h; defined in pmatch.cc — ported with pmatch.)

    // [spec:hfst:def:transducer.hfst-ol.transducer.has-transitions-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.has-transitions-fn]
    pub fn has_transitions(&self, i: TransitionTableIndex, symbol: SymbolNumber) -> bool {
        if i >= TRANSITION_TARGET_TABLE_START {
            self.get_transition_input(i - TRANSITION_TARGET_TABLE_START) == symbol
        } else {
            self.get_index_input(i + symbol as u32) == symbol
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.has-epsilons-or-flags-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.has-epsilons-or-flags-fn]
    pub fn has_epsilons_or_flags(&self, i: TransitionTableIndex) -> bool {
        if i >= TRANSITION_TARGET_TABLE_START {
            let input = self.get_transition_input(i - TRANSITION_TARGET_TABLE_START);
            input == 0 || self.is_flag(input)
        } else {
            self.get_index_input(i) == 0
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.take-epsilons-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.take-epsilons-fn]
    pub fn take_epsilons(&self, i: TransitionTableIndex) -> STransition {
        if self.get_transition_input(i) != 0 {
            return STransition::new(0, NO_SYMBOL_NUMBER);
        }
        STransition::new_weighted(
            self.get_transition_target(i),
            self.get_transition_output(i),
            self.get_transition_weight(i),
        )
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.take-epsilons-and-flags-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.take-epsilons-and-flags-fn]
    pub fn take_epsilons_and_flags(&self, i: TransitionTableIndex) -> STransition {
        if self.get_transition_input(i) != 0 && !self.is_flag(self.get_transition_input(i)) {
            return STransition::new(0, NO_SYMBOL_NUMBER);
        }
        STransition::new_weighted(
            self.get_transition_target(i),
            self.get_transition_output(i),
            self.get_transition_weight(i),
        )
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.take-non-epsilons-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.take-non-epsilons-fn]
    pub fn take_non_epsilons(&self, i: TransitionTableIndex, symbol: SymbolNumber) -> STransition {
        if self.get_transition_input(i) != symbol {
            return STransition::new(0, NO_SYMBOL_NUMBER);
        }
        STransition::new_weighted(
            self.get_transition_target(i),
            self.get_transition_output(i),
            self.get_transition_weight(i),
        )
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.final-weight-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.final-weight-fn]
    pub fn final_weight(&self, i: TransitionTableIndex) -> Weight {
        if i >= TRANSITION_TARGET_TABLE_START {
            self.get_transition_weight(i - TRANSITION_TARGET_TABLE_START)
        } else {
            self.get_index_final_weight(i)
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.is-flag-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.is-flag-fn]
    pub fn is_flag(&self, symbol: SymbolNumber) -> bool {
        self.alph().is_flag_diacritic(symbol)
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer.is-weighted-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.is-weighted-fn]
    pub fn is_weighted(&self) -> bool {
        self.hdr().probe_flag(HeaderFlag::Weighted)
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.get-unknown-symbol-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.get-unknown-symbol-fn]
    #[inline]
    pub fn get_unknown_symbol(&self) -> SymbolNumber {
        self.alph().get_unknown_symbol()
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer.get-string-symbol-map-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.get-string-symbol-map-fn]
    pub fn get_string_symbol_map(&self) -> StringSymbolMap {
        self.alph().build_string_symbol_map()
    }
}
