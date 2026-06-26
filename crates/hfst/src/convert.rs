//! Port of the OpenFST-independent part of
//! 'libhfst/src/implementations/optimized-lookup/convert.{h,cc}' (namespace
//! 'hfst_ol').
//!
//! These are the placeholder structures and helpers that the
//! 'HfstBasicTransducer -> hfst_ol::Transducer' conversion (in
//! 'ConvertOlTransducer.cc') builds the optimized-lookup index/transition
//! tables from. Everything under '#if HAVE_OPENFST' (the 'Convert*' classes that
//! turn an 'fst::StdVectorFst' into OL tables) is deferred to the rustfst
//! backend.
//!
//! Aliasing note: 'write_transitions_from_state_placeholders' /
//! 'add_transitions_with' are passed both a slice of one state's transition
//! placeholders *and* the whole 'state_placeholders' vector — in the C++ these
//! are non-const references but neither is mutated, so both are immutable
//! borrows here and the overlap is fine.

use std::collections::{BTreeMap, BTreeSet};

use crate::hfst_data_types::size_t_to_uint;
use crate::hfst_exception_defs::HfstFatalException;
use crate::transducer::{
    INFINITE_WEIGHT, NO_TABLE_INDEX, SymbolNumber, TRANSITION_TARGET_TABLE_START, TransducerTable,
    TransitionTableIndex, TransitionW, Weight,
};

// [spec:hfst:def:convert.hfst-ol.hfst-ol-to-basic-state-map]
pub type HfstOlToBasicStateMap = BTreeMap<TransitionTableIndex, u32>;

// [spec:hfst:def:convert.hfst-ol.transition-placeholder]
#[derive(Clone, Copy)]
pub struct TransitionPlaceholder {
    pub target: u32,
    pub input: SymbolNumber,
    pub output: SymbolNumber,
    pub weight: f32,
}

impl TransitionPlaceholder {
    // [spec:hfst:def:convert.hfst-ol.transition-placeholder.transition-placeholder-fn]
    // [spec:hfst:sem:convert.hfst-ol.transition-placeholder.transition-placeholder-fn]
    pub fn new(t: u32, i: SymbolNumber, o: SymbolNumber, w: f32) -> Self {
        TransitionPlaceholder {
            target: t,
            input: i,
            output: o,
            weight: w,
        }
    }
}

// [spec:hfst:def:convert.hfst-ol.state-placeholder.indexing-type]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum IndexingType {
    empty,
    simple_zero_index,
    simple_nonzero_index,
    nonsimple,
}

// [spec:hfst:def:convert.hfst-ol.state-placeholder]
#[derive(Clone)]
pub struct StatePlaceholder {
    pub state_number: u32,
    pub start_index: u32,
    pub first_transition: u32,
    pub symbol_to_transition_placeholder_v: Vec<u32>,
    pub transition_placeholders: Vec<Vec<TransitionPlaceholder>>,
    pub type_: IndexingType,
    pub inputs: SymbolNumber,
    pub final_: bool,
    pub final_weight: f32,
}

impl StatePlaceholder {
    // [spec:hfst:def:convert.hfst-ol.state-placeholder.state-placeholder-fn]
    // [spec:hfst:sem:convert.hfst-ol.state-placeholder.state-placeholder-fn]
    pub fn new(state: u32, finality: bool, first: u32, final_weight: Weight) -> Self {
        StatePlaceholder {
            state_number: state,
            start_index: u32::MAX,
            first_transition: first,
            symbol_to_transition_placeholder_v: Vec::new(),
            transition_placeholders: Vec::new(),
            type_: if state == 0 {
                IndexingType::nonsimple
            } else {
                IndexingType::empty
            },
            inputs: 0,
            final_: finality,
            final_weight,
        }
    }

    pub fn new_default() -> Self {
        StatePlaceholder {
            state_number: u32::MAX,
            start_index: u32::MAX,
            first_transition: u32::MAX,
            symbol_to_transition_placeholder_v: Vec::new(),
            transition_placeholders: Vec::new(),
            type_: IndexingType::empty,
            inputs: 0,
            final_: false,
            final_weight: 0.0,
        }
    }

    // [spec:hfst:def:convert.hfst-ol.state-placeholder.is-simple-fn]
    // [spec:hfst:sem:convert.hfst-ol.state-placeholder.is-simple-fn]
    pub fn is_simple(&self) -> bool {
        self.type_ != IndexingType::nonsimple
    }

    // [spec:hfst:def:convert.hfst-ol.state-placeholder.number-of-transitions-fn]
    // [spec:hfst:sem:convert.hfst-ol.state-placeholder.number-of-transitions-fn]
    pub fn number_of_transitions(&self) -> u32 {
        let mut count: u32 = 0;
        for it in self.transition_placeholders.iter() {
            count += size_t_to_uint(it.len());
        }
        count
    }

    // [spec:hfst:def:convert.hfst-ol.state-placeholder.input-present-fn]
    // [spec:hfst:sem:convert.hfst-ol.state-placeholder.input-present-fn]
    pub fn input_present(&self, input: SymbolNumber) -> bool {
        (input as usize) < self.symbol_to_transition_placeholder_v.len()
            && self.symbol_to_transition_placeholder_v[input as usize] != u32::MAX
    }

    // [spec:hfst:def:convert.hfst-ol.state-placeholder.add-input-fn]
    // [spec:hfst:sem:convert.hfst-ol.state-placeholder.add-input-fn]
    pub fn add_input(&mut self, input: SymbolNumber, flag_symbols: &BTreeSet<SymbolNumber>) {
        if self.input_present(input) {
            return;
        }
        while self.symbol_to_transition_placeholder_v.len() <= input as usize {
            self.symbol_to_transition_placeholder_v.push(u32::MAX);
        }
        self.symbol_to_transition_placeholder_v[input as usize] =
            size_t_to_uint(self.transition_placeholders.len());
        self.transition_placeholders.push(Vec::new());
        self.inputs += 1;
        if self.type_ != IndexingType::nonsimple {
            // Depending on what type of inputs we now have, adjust the index
            // type. Epsilons and flags both index to 0. If we have only one
            // input symbol, we're simple.
            if self.type_ == IndexingType::empty {
                if input == 0 || flag_symbols.contains(&input) {
                    self.type_ = IndexingType::simple_zero_index;
                } else {
                    self.type_ = IndexingType::simple_nonzero_index;
                }
            } else if self.type_ == IndexingType::simple_zero_index {
                if input != 0 && !flag_symbols.contains(&input) {
                    self.type_ = IndexingType::nonsimple;
                }
            } else {
                // simple_nonzero_index
                if self.inputs > 1 || input == 0 || flag_symbols.contains(&input) {
                    self.type_ = IndexingType::nonsimple;
                }
            }
        }
    }

    // [spec:hfst:def:convert.hfst-ol.state-placeholder.get-largest-index-fn]
    // [spec:hfst:sem:convert.hfst-ol.state-placeholder.get-largest-index-fn]
    pub fn get_largest_index(&self) -> SymbolNumber {
        let back = *self.symbol_to_transition_placeholder_v.last().unwrap();
        self.transition_placeholders[back as usize][0].input
    }

    // [spec:hfst:def:convert.hfst-ol.state-placeholder.add-transition-fn]
    // [spec:hfst:sem:convert.hfst-ol.state-placeholder.add-transition-fn]
    pub fn add_transition(&mut self, trans: TransitionPlaceholder) {
        let slot = self.symbol_to_transition_placeholder_v[trans.input as usize] as usize;
        self.transition_placeholders[slot].push(trans);
    }

    pub fn get_transition_placeholders(&self, input: SymbolNumber) -> &Vec<TransitionPlaceholder> {
        &self.transition_placeholders
            [self.symbol_to_transition_placeholder_v[input as usize] as usize]
    }

    // [spec:hfst:def:convert.hfst-ol.state-placeholder.symbol-offset-fn]
    // [spec:hfst:sem:convert.hfst-ol.state-placeholder.symbol-offset-fn]
    pub fn symbol_offset(
        &self,
        symbol: SymbolNumber,
        flag_symbols: &BTreeSet<SymbolNumber>,
    ) -> u32 {
        if symbol == 0 {
            return 0;
        }
        let mut offset: u32 = 0;
        if self.input_present(0) {
            // if there are epsilons
            offset = size_t_to_uint(self.get_transition_placeholders(0).len());
        }
        for flag_it in flag_symbols.iter() {
            if self.input_present(*flag_it) {
                if symbol == *flag_it {
                    // Flags go to 0 (even if there's no epsilon)
                    return 0;
                }
                offset += size_t_to_uint(self.get_transition_placeholders(*flag_it).len());
            }
        }
        for i in 1..self.symbol_to_transition_placeholder_v.len() {
            let i = i as SymbolNumber;
            if self.input_present(i) {
                if flag_symbols.contains(&i) {
                    // already counted
                    continue;
                }
                if symbol == i {
                    return offset;
                }
                offset += size_t_to_uint(self.get_transition_placeholders(i).len());
            }
        }
        let message = String::from(
            "error in conversion between optimized lookup format and \
             HfstTransducer;\ntried to calculate symbol_offset for symbol not \
             present in state",
        );
        crate::HFST_THROW_MESSAGE!(HfstFatalException, message)
    }
}

// [spec:hfst:def:convert.hfst-ol.compare-states-by-input-size-fn]
// [spec:hfst:sem:convert.hfst-ol.compare-states-by-input-size-fn]
pub fn compare_states_by_input_size(lhs: &StatePlaceholder, rhs: &StatePlaceholder) -> bool {
    // descending by input size
    lhs.inputs > rhs.inputs
}

// [spec:hfst:def:convert.hfst-ol.compare-states-by-state-number-fn]
// [spec:hfst:sem:convert.hfst-ol.compare-states-by-state-number-fn]
pub fn compare_states_by_state_number(lhs: &StatePlaceholder, rhs: &StatePlaceholder) -> bool {
    // ascending by number
    lhs.state_number < rhs.state_number
}

// [spec:hfst:def:convert.hfst-ol.index-placeholders]
#[derive(Clone)]
pub struct IndexPlaceholders {
    pub indices: Vec<u32>,
    pub targets: Vec<(u32, SymbolNumber)>,
}

impl IndexPlaceholders {
    pub fn new() -> Self {
        IndexPlaceholders {
            indices: Vec::new(),
            targets: Vec::new(),
        }
    }

    // [spec:hfst:def:convert.hfst-ol.index-placeholders.used-fn]
    // [spec:hfst:sem:convert.hfst-ol.index-placeholders.used-fn]
    pub fn used(&self, position: u32) -> bool {
        (position as usize) < self.indices.len()
            && self.indices[position as usize] != NO_TABLE_INDEX
    }

    // [spec:hfst:def:convert.hfst-ol.index-placeholders.assign-fn]
    // [spec:hfst:sem:convert.hfst-ol.index-placeholders.assign-fn]
    pub fn assign(&mut self, position: u32, target: u32, sym: SymbolNumber) {
        while position >= self.indices.len() as u32 {
            self.indices.push(NO_TABLE_INDEX);
        }
        self.indices[position as usize] = size_t_to_uint(self.targets.len());
        self.targets.push((target, sym));
    }

    // [spec:hfst:def:convert.hfst-ol.index-placeholders.get-target-fn]
    // [spec:hfst:sem:convert.hfst-ol.index-placeholders.get-target-fn]
    pub fn get_target(&self, index: u32) -> (u32, SymbolNumber) {
        self.targets[self.indices[index as usize] as usize]
    }

    // [spec:hfst:def:convert.hfst-ol.index-placeholders.fits-fn]
    // [spec:hfst:sem:convert.hfst-ol.index-placeholders.fits-fn]
    pub fn fits(
        &self,
        state: &StatePlaceholder,
        flag_symbols: &BTreeSet<SymbolNumber>,
        position: u32,
    ) -> bool {
        if self.used(position) {
            return false;
        }
        for it in state.transition_placeholders.iter() {
            let mut index_offset = it[0].input;
            if flag_symbols.contains(&index_offset) {
                index_offset = 0;
            }
            if self.used(index_offset as u32 + position + 1) {
                return false;
            }
        }
        true
    }

    // [spec:hfst:def:convert.hfst-ol.index-placeholders.unsuitable-fn]
    // [spec:hfst:sem:convert.hfst-ol.index-placeholders.unsuitable-fn]
    pub fn unsuitable(&self, index: u32, symbols: SymbolNumber, packing_aggression: f32) -> bool {
        if self.used(index) {
            return true;
        }

        let mut filled: u32 = 0;
        for i in 0..symbols {
            filled += self.used(index + i as u32 + 1) as u32;
            if filled as f32 >= packing_aggression * symbols as f32 {
                return true; // too full
            }
        }
        false
    }
}

impl Default for IndexPlaceholders {
    fn default() -> Self {
        Self::new()
    }
}

// [spec:hfst:def:convert.hfst-ol.write-transitions-from-state-placeholders-fn]
// [spec:hfst:sem:convert.hfst-ol.write-transitions-from-state-placeholders-fn]
pub fn write_transitions_from_state_placeholders(
    transition_table: &mut TransducerTable<TransitionW>,
    state_placeholders: &[StatePlaceholder],
    flag_symbols: &BTreeSet<SymbolNumber>,
) {
    for idx in 0..state_placeholders.len() {
        let it = &state_placeholders[idx];

        // Insert a finality marker unless this is the first state, the finality
        // of which is determined by the index table
        if it.state_number != 0 {
            transition_table.append(TransitionW::new_final(it.final_, it.final_weight));
        }

        // Then we iterate through the symbols each state has. First we do a pass
        // for epsilon and flags (they have to come first), then everything else.
        if it.input_present(0) {
            add_transitions_with(
                0,
                it.get_transition_placeholders(0),
                transition_table,
                state_placeholders,
                flag_symbols,
            );
        }
        for flag_it in flag_symbols.iter() {
            if it.input_present(*flag_it) {
                add_transitions_with(
                    *flag_it,
                    it.get_transition_placeholders(*flag_it),
                    transition_table,
                    state_placeholders,
                    flag_symbols,
                );
            }
        }
        for i in 1..it.symbol_to_transition_placeholder_v.len() {
            let i = i as SymbolNumber;
            if !it.input_present(i) || flag_symbols.contains(&i) {
                continue;
            }
            add_transitions_with(
                i,
                it.get_transition_placeholders(i),
                transition_table,
                state_placeholders,
                flag_symbols,
            );
        }
    }

    // one final padding transition
    transition_table.append(TransitionW::new_final(false, INFINITE_WEIGHT));
}

// [spec:hfst:def:convert.hfst-ol.add-transitions-with-fn]
// [spec:hfst:sem:convert.hfst-ol.add-transitions-with-fn]
pub fn add_transitions_with(
    symbol: SymbolNumber,
    transitions: &[TransitionPlaceholder],
    transition_table: &mut TransducerTable<TransitionW>,
    state_placeholders: &[StatePlaceholder],
    _flag_symbols: &BTreeSet<SymbolNumber>,
) {
    for it in transitions.iter() {
        // before writing each transition, find out whether its target is simple
        // (ie. should point directly to TA entry)
        let target: u32;
        if state_placeholders[it.target as usize].is_simple() {
            target = state_placeholders[it.target as usize].first_transition
                + TRANSITION_TARGET_TABLE_START
                - 1;
        } else {
            target = state_placeholders[it.target as usize].start_index;
        }
        transition_table.append(TransitionW::new_values(
            symbol, it.output, target, it.weight,
        ));
    }
}
