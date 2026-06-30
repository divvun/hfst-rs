//! Port of the OpenFST-independent part of
//! 'libhfst/src/implementations/optimized-lookup/convert.{h,cc}' (namespace
//! 'hfst_ol').
//!
//! These are the placeholder structures and helpers that the
//! 'HfstBasicTransducer -> hfst_ol::Transducer' conversion (in
//! 'ConvertOlTransducer.cc') builds the optimized-lookup index/transition
//! tables from. The '#if HAVE_OPENFST' block (the 'Convert*' classes that turn
//! an 'fst::StdVectorFst' into OL tables) is ported onto the 'hfst-openfst'
//! rustfst adapter: 'TransduceR' is 'StdVectorFst', 'StdArc' is 'StdTransition',
//! the OpenFST 'ArcIterator' becomes 'get_trs(n).trs().iter()', 'tr->Final(s) !=
//! Zero()' becomes 'is_final(s)', and 'fst::SymbolTable' (Copy/AddTable/Find/
//! iteration) maps onto rustfst's 'SymbolTable' (clone/add_table/get_symbol/
//! get_label/iter). The 'static ConvertTransducer* constructing_transducer'
//! global and the intra-object raw pointers (a 'ConvertTransitionIndex' points
//! at a 'ConvertTransition' owned by the state's transition set, transitions
//! point back at the constructing transducer) are kept as raw pointers exactly
//! as the C++ uses them.
//!
//! Aliasing note: 'write_transitions_from_state_placeholders' /
//! 'add_transitions_with' are passed both a slice of one state's transition
//! placeholders *and* the whole 'state_placeholders' vector — in the C++ these
//! are non-const references but neither is mutated, so both are immutable
//! borrows here and the overlap is fine.

use std::collections::{BTreeMap, BTreeSet};

use hfst_openfst::prelude::*;
use hfst_openfst::{StdTransition, StdVectorFst, SymbolTable as OfstSymbolTable, TropicalWeight};

use crate::hfst_data_types::size_t_to_uint;
use crate::hfst_exception_defs::HfstFatalException;
use crate::hfst_flag_diacritics::FdOperation;
use crate::transducer::{
    HeaderFlag, INFINITE_WEIGHT, NO_SYMBOL_NUMBER, NO_TABLE_INDEX, StateIdNumber, SymbolNumber,
    SymbolNumberSet, SymbolTable, TRANSITION_TARGET_TABLE_START, TableEntry, Transducer,
    TransducerAlphabet, TransducerHeader, TransducerTable, Transition, TransitionIndex,
    TransitionTableIndex, TransitionW, TransitionWIndex, Weight,
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

// ===========================================================================
// '#if HAVE_OPENFST' — the 'Convert*' machinery that turns an
// 'fst::StdVectorFst' ('TransduceR') into an 'hfst_ol::Transducer', ported onto
// the 'hfst-openfst' rustfst adapter.
// ===========================================================================

// [spec:hfst:def:convert.hfst-ol.state-id]
// 'typedef /*fst::StdArc::StateId*/ int StateId;' — rustfst's 'StateId' is 'u32'.
pub type StateId = u32;

// 'NO_ID_NUMBER'/'NO_STATE_ID'/'BIG_STATE_LIMIT'.
pub const NO_ID_NUMBER: StateIdNumber = u32::MAX;
pub const NO_STATE_ID: StateId = u32::MAX;
pub const BIG_STATE_LIMIT: SymbolNumber = 1;

// [spec:hfst:def:convert.hfst-ol.state-id-set]
pub type StateIdSet = BTreeSet<StateId>;
// [spec:hfst:def:convert.hfst-ol.ofst-symbol-set]
pub type OfstSymbolSet = BTreeSet<i64>;
// [spec:hfst:def:convert.hfst-ol.ofst-symbol-count-map]
pub type OfstSymbolCountMap = BTreeMap<i64, u32>;
// [spec:hfst:def:convert.hfst-ol.symbol-set]
pub type SymbolSet = BTreeSet<String>;

// [spec:hfst:def:convert.hfst-ol.transition-label]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct transition_label {
    // [spec:hfst:def:convert.hfst-ol.compare-transition-labels]
    // [spec:hfst:def:convert.hfst-ol.compare-transition-labels.operator-fn]
    // [spec:hfst:sem:convert.hfst-ol.compare-transition-labels.operator-fn]
    // The derived 'Ord' compares 'input_symbol' then 'output_symbol', matching
    // 'compare_transition_labels'.
    input_symbol: i64,
    output_symbol: i64,
}

// [spec:hfst:def:convert.hfst-ol.label-set]
type LabelSet = BTreeSet<transition_label>;

// [spec:hfst:def:convert.hfst-ol.place-holder]
#[derive(Clone, Copy, PartialEq, Eq)]
enum place_holder {
    EMPTY,
    EMPTY_START,
    OCCUPIED_START,
    OCCUPIED,
}

// [spec:hfst:def:convert.hfst-ol.place-holder-vector]
type PlaceHolderVector = Vec<place_holder>;

// [spec:hfst:def:convert.hfst-ol.check-finality-fn]
// [spec:hfst:sem:convert.hfst-ol.check-finality-fn]
// 'tr->Final(s) != fst::TropicalWeight::Zero()' — rustfst's 'is_final' is
// exactly this test.
pub fn check_finality(tr: &StdVectorFst, s: StateId) -> bool {
    tr.is_final(s).unwrap()
}

/*
  A class which can translate between StateId and StateIdNumbers
*/
// [spec:hfst:def:convert.hfst-ol.convert-id-number-map]
pub struct ConvertIdNumberMap {
    // [spec:hfst:def:convert.hfst-ol.convert-id-number-map.id-numbers-to-state-ids]
    id_to_node: BTreeMap<StateIdNumber, StateId>,
    // [spec:hfst:def:convert.hfst-ol.convert-id-number-map.state-ids-to-id-numbers]
    node_to_id: BTreeMap<StateId, StateIdNumber>,
    node_counter: StateIdNumber,
}

impl ConvertIdNumberMap {
    // [spec:hfst:def:convert.hfst-ol.convert-id-number-map.convert-id-number-map-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-id-number-map.convert-id-number-map-fn]
    pub fn new(t: &StdVectorFst) -> Self {
        let mut m = ConvertIdNumberMap {
            id_to_node: BTreeMap::new(),
            node_to_id: BTreeMap::new(),
            node_counter: 0,
        };
        m.set_node_maps(t);
        m
    }

    // [spec:hfst:def:convert.hfst-ol.convert-id-number-map.add-node-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-id-number-map.add-node-fn]
    fn add_node(&mut self, n: StateId, tr: &StdVectorFst) {
        if !self.node_to_id.contains_key(&n) {
            self.node_to_id.insert(n, self.node_counter);
            self.id_to_node.insert(self.node_counter, n);
            self.node_counter += 1;
            let trs = tr.get_trs(n).unwrap();
            for a in trs.trs().iter() {
                self.add_node(a.nextstate, tr);
            }
        }
    }

    // [spec:hfst:def:convert.hfst-ol.convert-id-number-map.set-node-maps-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-id-number-map.set-node-maps-fn]
    fn set_node_maps(&mut self, t: &StdVectorFst) {
        let n = t.start().unwrap();
        self.add_node(n, t);
    }

    // [spec:hfst:def:convert.hfst-ol.convert-id-number-map.get-number-of-nodes-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-id-number-map.get-number-of-nodes-fn]
    pub fn get_number_of_nodes(&self) -> StateIdNumber {
        self.node_counter
    }

    // [spec:hfst:def:convert.hfst-ol.convert-id-number-map.get-node-id-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-id-number-map.get-node-id-fn]
    pub fn get_node_id(&self, n: StateId) -> StateIdNumber {
        match self.node_to_id.get(&n) {
            Some(&i) => i,
            None => NO_ID_NUMBER,
        }
    }

    // [spec:hfst:def:convert.hfst-ol.convert-id-number-map.get-id-node-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-id-number-map.get-id-node-fn]
    pub fn get_id_node(&self, n: StateIdNumber) -> StateId {
        match self.id_to_node.get(&n) {
            Some(&i) => i,
            None => NO_STATE_ID,
        }
    }
}

// [spec:hfst:def:convert.hfst-ol.convert-transducer-alphabet]
pub struct ConvertTransducerAlphabet {
    symbol_table: SymbolTable,

    // input and output symbol tables together
    ofst_symbol_table: OfstSymbolTable,

    input_symbols_map: BTreeMap<i64, SymbolNumber>,
    output_symbols_map: BTreeMap<i64, SymbolNumber>,
}

impl ConvertTransducerAlphabet {
    // [spec:hfst:def:convert.hfst-ol.convert-transducer-alphabet.convert-transducer-alphabet-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transducer-alphabet.convert-transducer-alphabet-fn]
    pub fn new(t: &StdVectorFst) -> Self {
        // add an epsilon symbol here??
        let mut ofst_symbol_table: OfstSymbolTable = (**t.input_symbols().unwrap()).clone();
        if let Some(osym) = t.output_symbols() {
            ofst_symbol_table.add_table(&**osym);
        }

        let mut alpha = ConvertTransducerAlphabet {
            symbol_table: SymbolTable::new(),
            ofst_symbol_table,
            input_symbols_map: BTreeMap::new(),
            output_symbols_map: BTreeMap::new(),
        };

        let mut symbol_count_map = OfstSymbolCountMap::new();
        let mut all_symbol_set = SymbolSet::new();

        alpha.get_symbol_info(t, &mut symbol_count_map, &mut all_symbol_set);
        alpha.populate_symbol_table(&symbol_count_map, &all_symbol_set);
        alpha.set_maps(t);

        // 'delete ofst_symbol_table;' — the merged table is only needed during
        // construction; drop it now.
        alpha.ofst_symbol_table = OfstSymbolTable::empty();
        alpha
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transducer-alphabet.inspect-node-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transducer-alphabet.inspect-node-fn]
    fn inspect_node(
        &self,
        t: &StdVectorFst,
        n: StateId,
        visited_nodes: &mut StateIdSet,
        symbol_count_map: &mut OfstSymbolCountMap,
        all_symbol_set: &mut SymbolSet,
    ) {
        if visited_nodes.contains(&n) {
            return;
        }
        visited_nodes.insert(n);

        let mut input_symbols: BTreeSet<String> = BTreeSet::new();
        let isym = t.input_symbols().unwrap().clone();
        let osym = t.output_symbols().cloned();
        let trs = t.get_trs(n).unwrap();
        for arc in trs.trs().iter() {
            let input_symbol_string = isym.get_symbol(arc.ilabel).unwrap_or("").to_string();

            if !FdOperation::is_diacritic(&input_symbol_string) {
                input_symbols.insert(input_symbol_string.clone());
            }
            all_symbol_set.insert(input_symbol_string.clone());
            if let Some(osym) = &osym {
                all_symbol_set.insert(osym.get_symbol(arc.olabel).unwrap_or("").to_string());
            } else {
                all_symbol_set.insert(isym.get_symbol(arc.olabel).unwrap_or("").to_string());
            }

            self.inspect_node(
                t,
                arc.nextstate,
                visited_nodes,
                symbol_count_map,
                all_symbol_set,
            );
        }

        for it in input_symbols.iter() {
            let label = self
                .ofst_symbol_table
                .get_label(it)
                .map(|l| l as i64)
                .unwrap_or(-1);
            *symbol_count_map.entry(label).or_insert(0) += 1;
        }
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transducer-alphabet.get-symbol-info-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transducer-alphabet.get-symbol-info-fn]
    fn get_symbol_info(
        &self,
        t: &StdVectorFst,
        symbol_count_map: &mut OfstSymbolCountMap,
        all_symbol_set: &mut SymbolSet,
    ) {
        symbol_count_map.insert(0, 1);
        let mut visited_nodes = StateIdSet::new();
        let start = t.start().unwrap();
        self.inspect_node(
            t,
            start,
            &mut visited_nodes,
            symbol_count_map,
            all_symbol_set,
        );
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transducer-alphabet.populate-symbol-table-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transducer-alphabet.populate-symbol-table-fn]
    fn populate_symbol_table(
        &mut self,
        input_symbol_counts: &OfstSymbolCountMap,
        all_symbol_set: &SymbolSet,
    ) {
        // a reverse mapping of input_symbol_counts, to sort symbols by frequency.
        // The C++ 'std::multimap<unsigned int, int64>' is ordered ascending by
        // key (count), stable in insertion order for ties; 'input_symbol_counts'
        // is walked in ascending-label order, so a stable sort by count over the
        // label-ordered pairs reproduces the multimap's forward order, and
        // '.rev()' its reverse iteration.
        let mut count_keys: Vec<(u32, i64)> = Vec::new();
        for (&label, &count) in input_symbol_counts.iter() {
            let sym = self
                .ofst_symbol_table
                .get_symbol(label as u32)
                .unwrap_or("")
                .to_string();
            if !FdOperation::is_diacritic(&sym) {
                count_keys.push((count, label));
            } else {
                count_keys.push((0, label));
            }
        }
        count_keys.sort_by_key(|&(count, _)| count);

        let s0 = self
            .ofst_symbol_table
            .get_symbol(0)
            .unwrap_or("")
            .to_string();
        self.symbol_table.push(s0);
        for &(_, label) in count_keys.iter().rev() {
            if label != 0 {
                let s = self
                    .ofst_symbol_table
                    .get_symbol(label as u32)
                    .unwrap_or("")
                    .to_string();
                self.symbol_table.push(s);
            }
        }

        // OpenFST iterates its symbol table in ascending-label order; mirror that.
        let mut entries: Vec<(u32, String)> = self
            .ofst_symbol_table
            .iter()
            .map(|(l, s)| (l, s.to_string()))
            .collect();
        entries.sort_by_key(|&(l, _)| l);
        for (label, sym) in entries {
            if !input_symbol_counts.contains_key(&(label as i64)) && all_symbol_set.contains(&sym) {
                self.symbol_table.push(sym);
            }
        }
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transducer-alphabet.set-maps-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transducer-alphabet.set-maps-fn]
    fn set_maps(&mut self, t: &StdVectorFst) {
        let isym = t.input_symbols().unwrap().clone();
        for (label, sym) in isym.iter() {
            for i in 0..self.symbol_table.len() {
                if self.symbol_table[i] == sym {
                    self.input_symbols_map
                        .insert(label as i64, i as SymbolNumber);
                    break;
                }
            }
        }

        let osym = t.output_symbols().cloned();
        if let Some(osym) = osym {
            for (label, sym) in osym.iter() {
                for i in 0..self.symbol_table.len() {
                    if self.symbol_table[i] == sym {
                        self.output_symbols_map
                            .insert(label as i64, i as SymbolNumber);
                        break;
                    }
                }
            }
        } else {
            self.output_symbols_map = self.input_symbols_map.clone();
        }
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transducer-alphabet.display-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transducer-alphabet.display-fn]
    pub fn display(&self, t: &StdVectorFst) {
        println!("Final reordered symbol table:");
        for i in 0..self.symbol_table.len() {
            println!("{}: {}", i, self.symbol_table[i]);
        }

        println!("Initial input symbols (old/new: string):");
        let isym = t.input_symbols().unwrap().clone();
        for (label, sym) in isym.iter() {
            println!(
                "{}/{}: {}",
                label,
                self.lookup_ofst_input_symbol(label as i64),
                sym
            );
        }
        println!("Initial output symbols: (old/new: string)");
        if t.output_symbols().is_some() {
            for (label, sym) in isym.iter() {
                println!(
                    "{}/{}: {}",
                    label,
                    self.lookup_ofst_output_symbol(label as i64),
                    sym
                );
            }
        } else {
            println!("[NULL]");
        }
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transducer-alphabet.lookup-ofst-input-symbol-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transducer-alphabet.lookup-ofst-input-symbol-fn]
    pub fn lookup_ofst_input_symbol(&self, s: i64) -> SymbolNumber {
        match self.input_symbols_map.get(&s) {
            Some(&i) => i,
            None => NO_SYMBOL_NUMBER,
        }
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transducer-alphabet.lookup-ofst-output-symbol-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transducer-alphabet.lookup-ofst-output-symbol-fn]
    pub fn lookup_ofst_output_symbol(&self, s: i64) -> SymbolNumber {
        match self.output_symbols_map.get(&s) {
            Some(&i) => i,
            None => NO_SYMBOL_NUMBER,
        }
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transducer-alphabet.is-flag-diacritic-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transducer-alphabet.is-flag-diacritic-fn]
    pub fn is_flag_diacritic(&self, symbol: SymbolNumber) -> bool {
        FdOperation::is_diacritic(&self.symbol_table[symbol as usize])
    }

    pub fn get_symbol_table(&self) -> &SymbolTable {
        &self.symbol_table
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transducer-alphabet.to-alphabet-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transducer-alphabet.to-alphabet-fn]
    pub fn to_alphabet(&self) -> TransducerAlphabet {
        TransducerAlphabet::new_symboltable(&self.symbol_table)
    }
}

// [spec:hfst:def:convert.hfst-ol.convert-transition]
pub struct ConvertTransition {
    input_symbol: SymbolNumber,
    output_symbol: SymbolNumber,
    // C++ 'union { StateIdNumber target_state_id; TransitionTableIndex
    // target_state_index; };' — both are 'u32'; a single field holds the
    // node id until the tables are laid out, then the table index.
    target_state: u32,
    weight: Weight,

    table_index: TransitionTableIndex, // location in the transition table
}

impl ConvertTransition {
    // [spec:hfst:def:convert.hfst-ol.convert-transition.convert-transition-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transition.convert-transition-fn]
    fn new(a: &StdTransition) -> ConvertTransition {
        let ct = unsafe { constructing_transducer() };
        ConvertTransition {
            input_symbol: ct.get_alphabet().lookup_ofst_input_symbol(a.ilabel as i64),
            output_symbol: ct.get_alphabet().lookup_ofst_output_symbol(a.olabel as i64),
            target_state: ct.get_id_number_map().get_node_id(a.nextstate),
            weight: *a.weight.value(),
            table_index: NO_TABLE_INDEX,
        }
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transition.display-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transition.display-fn]
    pub fn display(&self) {
        println!(
            "  {}:{} at {} ->{} ({})",
            self.input_symbol, self.output_symbol, self.table_index, self.target_state, self.weight
        );
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transition.get-input-symbol-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transition.get-input-symbol-fn]
    pub fn get_input_symbol(&self) -> SymbolNumber {
        self.input_symbol
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transition.set-target-state-index-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transition.set-target-state-index-fn]
    fn set_target_state_index(&mut self) {
        let idx = unsafe { constructing_transducer() }
            .get_state(self.target_state)
            .get_table_index();
        self.target_state = idx;
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transition.set-table-index-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transition.set-table-index-fn]
    pub fn set_table_index(&mut self, i: TransitionTableIndex) {
        self.table_index = i;
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transition.get-table-index-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transition.get-table-index-fn]
    pub fn get_table_index(&self) -> TransitionTableIndex {
        self.table_index
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transition.to-transition-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transition.to-transition-fn]
    fn to_transition<T: ConvertTransitionEntry>(&self) -> T {
        T::ct_values(
            self.input_symbol,
            self.output_symbol,
            self.target_state,
            self.weight,
        )
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transition.numerical-cmp-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transition.numerical-cmp-fn]
    fn numerical_cmp(&self, another_transition: &ConvertTransition) -> bool {
        if self.input_symbol == another_transition.input_symbol {
            if self.output_symbol == another_transition.output_symbol {
                self.target_state < another_transition.target_state
            } else {
                self.output_symbol < another_transition.output_symbol
            }
        } else {
            self.input_symbol < another_transition.input_symbol
        }
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transition.operator-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transition.operator-fn]
    fn lt(&self, another_transition: &ConvertTransition) -> bool {
        let alphabet = unsafe { constructing_transducer() }.get_alphabet();
        if (self.input_symbol == 0) || alphabet.is_flag_diacritic(self.input_symbol) {
            if (another_transition.input_symbol == 0)
                || alphabet.is_flag_diacritic(another_transition.input_symbol)
            {
                self.numerical_cmp(another_transition)
            } else {
                true
            }
        } else if (another_transition.input_symbol != 0)
            && !alphabet.is_flag_diacritic(another_transition.input_symbol)
        {
            self.numerical_cmp(another_transition)
        } else {
            false
        }
    }
}

// [spec:hfst:def:convert.hfst-ol.convert-transition-index]
pub struct ConvertTransitionIndex {
    input_symbol: SymbolNumber,
    // C++ 'union { ConvertTransition* first_transition; TransitionTableIndex
    // first_transition_index; };' — the pointer is used until the table is laid
    // out, then 'first_transition_index' is set and is the only field read.
    first_transition: *mut ConvertTransition,
    first_transition_index: TransitionTableIndex,
}

impl ConvertTransitionIndex {
    // [spec:hfst:def:convert.hfst-ol.convert-transition-index.convert-transition-index-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transition-index.convert-transition-index-fn]
    fn new(input: SymbolNumber, transition: *mut ConvertTransition) -> ConvertTransitionIndex {
        ConvertTransitionIndex {
            input_symbol: input,
            first_transition: transition,
            first_transition_index: NO_TABLE_INDEX,
        }
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transition-index.display-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transition-index.display-fn]
    pub fn display(&self) {
        println!(
            "  input_symbol: {} to transitions starting at {}",
            self.input_symbol, self.first_transition_index
        );
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transition-index.get-input-symbol-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transition-index.get-input-symbol-fn]
    pub fn get_input_symbol(&self) -> SymbolNumber {
        self.input_symbol
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transition-index.get-first-transition-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transition-index.get-first-transition-fn]
    pub fn get_first_transition(&self) -> *mut ConvertTransition {
        self.first_transition
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transition-index.set-first-transition-index-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transition-index.set-first-transition-index-fn]
    pub fn set_first_transition_index(&mut self, i: TransitionTableIndex) {
        self.first_transition_index = i;
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transition-index.to-transition-index-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transition-index.to-transition-index-fn]
    fn to_transition_index<T: ConvertIndexEntry>(&self) -> T {
        T::ct_new(self.input_symbol, self.first_transition_index)
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transition-index.operator-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transition-index.operator-fn]
    fn lt(&self, another_index: &ConvertTransitionIndex) -> bool {
        self.input_symbol < another_index.input_symbol
    }
}

// The comparator functors used by 'std::set<ConvertTransition*,
// ConvertTransitionCompare>' / 'std::set<ConvertTransitionIndex*,
// ConvertTransitionIndexCompare>'. The ordered-insert helpers on
// 'ConvertFstState' apply these to keep their 'Vec' equivalents in set order.
// [spec:hfst:def:convert.hfst-ol.convert-transition-compare]
struct ConvertTransitionCompare;

impl ConvertTransitionCompare {
    // 'return t1->operator<(*t2);'
    // [spec:hfst:def:convert.hfst-ol.convert-transition-compare.operator-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transition-compare.operator-fn]
    fn operator(t1: &ConvertTransition, t2: &ConvertTransition) -> bool {
        t1.lt(t2)
    }
}

// [spec:hfst:def:convert.hfst-ol.convert-transition-index-compare]
struct ConvertTransitionIndexCompare;

impl ConvertTransitionIndexCompare {
    // 'return i1->operator<(*i2);'
    // [spec:hfst:def:convert.hfst-ol.convert-transition-index-compare.operator-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transition-index-compare.operator-fn]
    fn operator(i1: &ConvertTransitionIndex, i2: &ConvertTransitionIndex) -> bool {
        i1.lt(i2)
    }
}

// The two C++ 'TransducerTable<T>' entry types ('TransitionIndex'/
// 'TransitionWIndex' and 'Transition'/'TransitionW') are reached through the
// '<T>' templates 'make_index_table'/'make_transition_table' /
// 'insert_transition_indices'/'append_transitions'/'to_transition*'. These
// local traits capture the constructors/accessor those templates use, so the
// generic Rust methods stand in for the C++ template instantiations.

trait ConvertIndexEntry: Sized {
    fn ct_default() -> Self;
    fn ct_new(input: SymbolNumber, first: TransitionTableIndex) -> Self;
    fn ct_get_input_symbol(&self) -> SymbolNumber;
}

impl ConvertIndexEntry for TransitionIndex {
    fn ct_default() -> Self {
        TransitionIndex::new()
    }
    fn ct_new(input: SymbolNumber, first: TransitionTableIndex) -> Self {
        TransitionIndex::new_values(input, first)
    }
    fn ct_get_input_symbol(&self) -> SymbolNumber {
        self.get_input_symbol()
    }
}

impl ConvertIndexEntry for TransitionWIndex {
    fn ct_default() -> Self {
        TransitionWIndex::new()
    }
    fn ct_new(input: SymbolNumber, first: TransitionTableIndex) -> Self {
        TransitionWIndex::new_values(input, first)
    }
    fn ct_get_input_symbol(&self) -> SymbolNumber {
        self.get_input_symbol()
    }
}

trait ConvertTransitionEntry: Sized {
    fn ct_final(final_: bool, weight: Weight) -> Self;
    fn ct_values(
        input: SymbolNumber,
        output: SymbolNumber,
        target: TransitionTableIndex,
        weight: Weight,
    ) -> Self;
}

impl ConvertTransitionEntry for Transition {
    // 'Transition(bool, Weight)' / 'Transition(in, out, target, weight)' ignore
    // the weight in the unweighted entry type.
    fn ct_final(final_: bool, _weight: Weight) -> Self {
        Transition::new_final(final_)
    }
    fn ct_values(
        input: SymbolNumber,
        output: SymbolNumber,
        target: TransitionTableIndex,
        _weight: Weight,
    ) -> Self {
        Transition::new_values(input, output, target)
    }
}

impl ConvertTransitionEntry for TransitionW {
    fn ct_final(final_: bool, weight: Weight) -> Self {
        TransitionW::new_final(final_, weight)
    }
    fn ct_values(
        input: SymbolNumber,
        output: SymbolNumber,
        target: TransitionTableIndex,
        weight: Weight,
    ) -> Self {
        TransitionW::new_values(input, output, target, weight)
    }
}

// [spec:hfst:def:convert.hfst-ol.convert-fst-state]
pub struct ConvertFstState {
    // 'std::set<ConvertTransition*, ConvertTransitionCompare>' — a Vec kept in
    // 'ConvertTransition::lt' order (set-insert dedups equivalent elements);
    // 'Box' gives the transitions stable addresses so 'ConvertTransitionIndex'
    // can point at them.
    transitions: Vec<Box<ConvertTransition>>,
    // 'std::set<ConvertTransitionIndex*, ConvertTransitionIndexCompare>'.
    transition_indices: Vec<Box<ConvertTransitionIndex>>,

    first_transition_index: TransitionTableIndex,
    table_index: TransitionTableIndex,

    final_: bool,
    weight: Weight,

    id: StateIdNumber,
}

impl ConvertFstState {
    // [spec:hfst:def:convert.hfst-ol.convert-fst-state.convert-fst-state-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-fst-state.convert-fst-state-fn]
    fn new(n: StateId, tr: &StdVectorFst) -> ConvertFstState {
        let mut state = ConvertFstState {
            transitions: Vec::new(),
            transition_indices: Vec::new(),
            first_transition_index: NO_TABLE_INDEX,
            table_index: NO_TABLE_INDEX,
            final_: check_finality(tr, n),
            weight: INFINITE_WEIGHT,
            id: unsafe { constructing_transducer() }
                .get_id_number_map()
                .get_node_id(n),
        };
        state.set_transitions(n, tr);
        state.set_transition_indices();
        if state.final_ {
            if unsafe { constructing_transducer() }.is_weighted() {
                state.weight = *tr.final_weight(n).unwrap().unwrap().value();
            } else {
                let finality: TransitionTableIndex = 1;
                state.weight = f32::from_bits(finality);
            }
        }
        state
    }

    // 'std::set'-style ordered insert of a transition under
    // 'ConvertTransitionCompare': skip an element equivalent under the comparator.
    fn insert_transition(vec: &mut Vec<Box<ConvertTransition>>, t: Box<ConvertTransition>) {
        let mut i = 0;
        while i < vec.len() && ConvertTransitionCompare::operator(&vec[i], &t) {
            i += 1;
        }
        if i < vec.len() && !ConvertTransitionCompare::operator(&t, &vec[i]) {
            return;
        }
        vec.insert(i, t);
    }

    // 'std::set'-style ordered insert of a transition index under
    // 'ConvertTransitionIndexCompare' (by input symbol).
    fn insert_index(vec: &mut Vec<Box<ConvertTransitionIndex>>, idx: Box<ConvertTransitionIndex>) {
        let mut i = 0;
        while i < vec.len() && ConvertTransitionIndexCompare::operator(&vec[i], &idx) {
            i += 1;
        }
        if i < vec.len() && !ConvertTransitionIndexCompare::operator(&idx, &vec[i]) {
            return;
        }
        vec.insert(i, idx);
    }

    // [spec:hfst:def:convert.hfst-ol.convert-fst-state.display-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-fst-state.display-fn]
    pub fn display(&self) {
        print!("{} at index {}", self.id, self.table_index);
        if self.final_ {
            print!(" (final, {})", self.weight);
        }
        println!(":");
        println!(" Transition indices:");
        for i in self.transition_indices.iter() {
            i.display();
        }
        println!(" Transitions:");
        for i in self.transitions.iter() {
            i.display();
        }
    }

    // [spec:hfst:def:convert.hfst-ol.convert-fst-state.set-transitions-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-fst-state.set-transitions-fn]
    fn set_transitions(&mut self, n: StateId, tr: &StdVectorFst) {
        let trs = tr.get_trs(n).unwrap();
        for a in trs.trs().iter() {
            let t = Box::new(ConvertTransition::new(a));
            Self::insert_transition(&mut self.transitions, t);
        }
    }

    // [spec:hfst:def:convert.hfst-ol.convert-fst-state.set-transition-indices-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-fst-state.set-transition-indices-fn]
    fn set_transition_indices(&mut self) {
        let mut previous_symbol: SymbolNumber = NO_SYMBOL_NUMBER;
        let mut _position: SymbolNumber = 0;

        let mut zero_transitions = false;
        for i in 0..self.transitions.len() {
            let t_ptr: *mut ConvertTransition =
                &*self.transitions[i] as *const ConvertTransition as *mut ConvertTransition;
            let input_symbol = self.transitions[i].get_input_symbol();
            if previous_symbol != input_symbol {
                if unsafe { constructing_transducer() }
                    .get_alphabet()
                    .is_flag_diacritic(input_symbol)
                {
                    if !zero_transitions {
                        Self::insert_index(
                            &mut self.transition_indices,
                            Box::new(ConvertTransitionIndex::new(0, t_ptr)),
                        );

                        previous_symbol = input_symbol;
                        zero_transitions = true;
                    }
                } else {
                    Self::insert_index(
                        &mut self.transition_indices,
                        Box::new(ConvertTransitionIndex::new(input_symbol, t_ptr)),
                    );

                    previous_symbol = input_symbol;
                }
            }
            if input_symbol == 0 {
                zero_transitions = true;
            }
            _position += 1;
        }
    }

    // [spec:hfst:def:convert.hfst-ol.convert-fst-state.get-input-symbols-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-fst-state.get-input-symbols-fn]
    fn get_input_symbols(&self) -> SymbolNumberSet {
        let mut input_symbols = SymbolNumberSet::new();
        for it in self.transition_indices.iter() {
            input_symbols.insert(it.get_input_symbol());
        }
        input_symbols
    }

    // [spec:hfst:def:convert.hfst-ol.convert-fst-state.number-of-input-symbols-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-fst-state.number-of-input-symbols-fn]
    pub fn number_of_input_symbols(&self) -> SymbolNumber {
        size_t_to_uint(self.transition_indices.len()) as SymbolNumber
    }
    // [spec:hfst:def:convert.hfst-ol.convert-fst-state.number-of-transitions-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-fst-state.number-of-transitions-fn]
    pub fn number_of_transitions(&self) -> SymbolNumber {
        size_t_to_uint(self.transitions.len()) as SymbolNumber
    }
    // [spec:hfst:def:convert.hfst-ol.convert-fst-state.is-final-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-fst-state.is-final-fn]
    pub fn is_final(&self) -> bool {
        self.final_
    }
    // [spec:hfst:def:convert.hfst-ol.convert-fst-state.is-big-state-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-fst-state.is-big-state-fn]
    pub fn is_big_state(&self) -> bool {
        self.transition_indices.len() > BIG_STATE_LIMIT as usize
    }
    // [spec:hfst:def:convert.hfst-ol.convert-fst-state.is-start-state-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-fst-state.is-start-state-fn]
    pub fn is_start_state(&self) -> bool {
        self.id == 0
    }
    // [spec:hfst:def:convert.hfst-ol.convert-fst-state.get-id-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-fst-state.get-id-fn]
    pub fn get_id(&self) -> StateIdNumber {
        self.id
    }

    // [spec:hfst:def:convert.hfst-ol.convert-fst-state.get-first-transition-index-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-fst-state.get-first-transition-index-fn]
    pub fn get_first_transition_index(&self) -> TransitionTableIndex {
        self.first_transition_index
    }

    // [spec:hfst:def:convert.hfst-ol.convert-fst-state.set-table-index-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-fst-state.set-table-index-fn]
    pub fn set_table_index(&mut self, i: TransitionTableIndex) {
        self.table_index = i;
    }
    // [spec:hfst:def:convert.hfst-ol.convert-fst-state.get-table-index-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-fst-state.get-table-index-fn]
    pub fn get_table_index(&self) -> TransitionTableIndex {
        self.table_index
    }

    // [spec:hfst:def:convert.hfst-ol.convert-fst-state.set-transition-table-indices-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-fst-state.set-transition-table-indices-fn]
    fn set_transition_table_indices(
        &mut self,
        place: TransitionTableIndex,
    ) -> TransitionTableIndex {
        self.first_transition_index = place;

        // lay out the transitions sequentially with a space between each state
        let mut place = place;
        for i in 0..self.transitions.len() {
            self.transitions[i].set_table_index(place);
            place += 1;
        }
        place += 1;

        // update the TransitionIndex's to store the table location of the
        // associated transition
        for j in 0..self.transition_indices.len() {
            let first = self.transition_indices[j].get_first_transition();
            let table_index = unsafe { (*first).get_table_index() };
            self.transition_indices[j].set_first_transition_index(table_index);
        }

        place
    }

    // [spec:hfst:def:convert.hfst-ol.convert-fst-state.set-transition-target-indices-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-fst-state.set-transition-target-indices-fn]
    fn set_transition_target_indices(&mut self) {
        for i in 0..self.transitions.len() {
            self.transitions[i].set_target_state_index();
        }
    }

    // [spec:hfst:def:convert.hfst-ol.convert-fst-state.insert-transition-indices-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-fst-state.insert-transition-indices-fn]
    fn insert_transition_indices<T: ConvertIndexEntry + TableEntry + Clone>(
        &self,
        index_table: &mut TransducerTable<T>,
    ) {
        // only the start state and big states have
        // entries in the transition index table
        if !self.is_big_state() && !self.is_start_state() {
            return;
        }

        let mut i = self.table_index;

        if self.final_ {
            let existing_input = index_table.at(i).ct_get_input_symbol();
            index_table.set(i as usize, T::ct_new(existing_input, self.weight.to_bits()));
        }

        i += 1;

        for ind in self.transition_indices.iter() {
            index_table.set(
                (i + ind.get_input_symbol() as u32) as usize,
                ind.to_transition_index::<T>(),
            );
        }
    }

    // [spec:hfst:def:convert.hfst-ol.convert-fst-state.append-transitions-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-fst-state.append-transitions-fn]
    fn append_transitions<T: ConvertTransitionEntry + TableEntry + Clone>(
        &self,
        transition_table: &mut TransducerTable<T>,
        place: TransitionTableIndex,
    ) -> TransitionTableIndex {
        let mut place = place;
        while place < self.get_first_transition_index() {
            transition_table.append(T::ct_final(self.final_, self.weight));
            place += 1;
        }

        for it in self.transitions.iter() {
            transition_table.append(it.to_transition::<T>());
            place += 1;
        }
        place
    }
}

// [spec:hfst:def:convert.hfst-ol.fst-state-compare]
// [spec:hfst:def:convert.hfst-ol.fst-state-compare.operator-fn]
// [spec:hfst:sem:convert.hfst-ol.fst-state-compare.operator-fn]
// NB: as in the C++, this is not a strict weak ordering (when the first state
// has at least as many transition indices as the second it falls through to the
// id comparison); the same set-insert procedure is used as for the transition
// sets, which reproduces the C++ behaviour for the well-ordered cases.
fn fst_state_less(s1: *mut ConvertFstState, s2: *mut ConvertFstState) -> bool {
    unsafe {
        if (*s1).transition_indices.len() < (*s2).transition_indices.len() {
            return true;
        }
        (*s1).id < (*s2).id
    }
}

// [spec:hfst:def:convert.hfst-ol.state-set]
fn state_set_insert(set: &mut Vec<*mut ConvertFstState>, x: *mut ConvertFstState) {
    let mut i = 0;
    while i < set.len() && fst_state_less(set[i], x) {
        i += 1;
    }
    if i < set.len() && !fst_state_less(x, set[i]) {
        return;
    }
    set.insert(i, x);
}

// [spec:hfst:def:convert.hfst-ol.convert-transition-table-indices]
pub struct ConvertTransitionTableIndices {
    indices: PlaceHolderVector,
    lower_bound: usize,
    lower_bound_test_count: u32,
    number_of_input_symbols: SymbolNumber,
}

impl ConvertTransitionTableIndices {
    // [spec:hfst:def:convert.hfst-ol.convert-transition-table-indices.convert-transition-table-indices-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transition-table-indices.convert-transition-table-indices-fn]
    pub fn new(input_symbol_count: SymbolNumber) -> Self {
        let mut x = ConvertTransitionTableIndices {
            indices: Vec::new(),
            lower_bound: 0,
            lower_bound_test_count: 0,
            number_of_input_symbols: input_symbol_count,
        };
        x.get_more_space();
        x
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transition-table-indices.get-more-space-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transition-table-indices.get-more-space-fn]
    fn get_more_space(&mut self) {
        for _i in 0..(self.number_of_input_symbols as u32 + 1) {
            self.indices.push(place_holder::EMPTY);
        }
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transition-table-indices.state-fits-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transition-table-indices.state-fits-fn]
    fn state_fits(&self, input_symbols: &SymbolNumberSet, final_state: bool, index: usize) -> bool {
        if (self.indices[index] == place_holder::EMPTY_START)
            || (self.indices[index] == place_holder::OCCUPIED_START)
        {
            return false;
        }

        if final_state && (self.indices[index] == place_holder::OCCUPIED) {
            return false;
        }

        // The input symbols start after the finality indicator.
        let input_symbol_start = index + 1;

        // The node fits, if every one of its input symbols goes on
        // an EMPTY or EMPTY_START index.
        for &input_symbol in input_symbols.iter() {
            let pos = input_symbol_start + input_symbol as usize;
            if (self.indices[pos] == place_holder::OCCUPIED)
                || (self.indices[pos] == place_holder::OCCUPIED_START)
            {
                return false;
            }
        }

        true
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transition-table-indices.insert-state-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transition-table-indices.insert-state-fn]
    fn insert_state(&mut self, input_symbols: &SymbolNumberSet, final_state: bool, index: usize) {
        if final_state || (self.indices[index] == place_holder::OCCUPIED) {
            self.indices[index] = place_holder::OCCUPIED_START;
        } else {
            self.indices[index] = place_holder::EMPTY_START;
        }

        // The input symbols start after the finality indicator.
        let input_symbol_start = index + 1;

        for &input_symbol in input_symbols.iter() {
            let pos = input_symbol_start + input_symbol as usize;
            if self.indices[pos] == place_holder::EMPTY {
                self.indices[pos] = place_holder::OCCUPIED;
            } else {
                self.indices[pos] = place_holder::OCCUPIED_START;
            }
        }
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transition-table-indices.last-full-index-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transition-table-indices.last-full-index-fn]
    pub fn last_full_index(&self) -> usize {
        let mut i = self.indices.len() - 1;
        while i != 0 {
            if self.indices[i] != place_holder::EMPTY {
                return i;
            }
            i -= 1;
        }
        0
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transition-table-indices.add-state-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transition-table-indices.add-state-fn]
    fn add_state(&mut self, state: &ConvertFstState) -> usize {
        if self.lower_bound_test_count >= 1 {
            self.lower_bound_test_count = 0;
            if self.indices.len() > 2000 && self.lower_bound < (self.indices.len() - 2000) {
                self.lower_bound = self.indices.len() - 1000;
            }

            self.lower_bound += 1;
        }

        let final_state = state.is_final();

        let state_input_symbols = state.get_input_symbols();

        self.lower_bound_test_count += 1;

        let mut index = self.lower_bound;
        while index < self.indices.len() {
            if (index + self.number_of_input_symbols as usize + 1) >= self.indices.len() {
                self.get_more_space();
            }

            if self.state_fits(&state_input_symbols, final_state, index) {
                self.insert_state(&state_input_symbols, final_state, index);
                return index;
            }
            index += 1;
        }
        u32::MAX as usize
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transition-table-indices.size-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transition-table-indices.size-fn]
    pub fn size(&self) -> usize {
        self.indices.len()
    }
}

// [spec:hfst:def:convert.hfst-ol.convert-transducer-header]
pub struct ConvertTransducerHeader;

impl ConvertTransducerHeader {
    // [spec:hfst:def:convert.hfst-ol.convert-transducer-header.full-traversal-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transducer-header.full-traversal-fn]
    fn full_traversal(
        h: &mut TransducerHeader,
        tr: &StdVectorFst,
        n: StateId,
        visited_nodes: &mut StateIdSet,
        nodes_in_path: &mut StateIdSet,
        all_input_symbols: &mut OfstSymbolSet,
    ) {
        if visited_nodes.contains(&n) {
            return;
        }
        visited_nodes.insert(n);
        nodes_in_path.insert(n);

        if h.weighted && !h.has_unweighted_input_epsilon_cycles {
            let mut epsilon_nodes = StateIdSet::new();
            Self::find_input_epsilon_cycles(n, n, &mut epsilon_nodes, true, tr, h);
        }
        if !h.has_input_epsilon_cycles {
            let mut epsilon_nodes = StateIdSet::new();
            Self::find_input_epsilon_cycles(n, n, &mut epsilon_nodes, false, tr, h);
        }

        let mut node_input_symbols: OfstSymbolSet = OfstSymbolSet::new();
        let mut transition_labels: LabelSet = LabelSet::new();

        let isym = tr.input_symbols().unwrap().clone();
        let trs = tr.get_trs(n).unwrap();
        for a in trs.trs().iter() {
            let l = transition_label {
                input_symbol: a.ilabel as i64,
                output_symbol: a.olabel as i64,
            };
            let target = a.nextstate;

            h.number_of_transitions += 1;
            let sym = isym.get_symbol(a.ilabel).unwrap_or("").to_string();
            if !FdOperation::is_diacritic(&sym) {
                all_input_symbols.insert(a.ilabel as i64);
            }

            if l.input_symbol == 0 {
                h.has_input_epsilon_transitions = true;
                if l.output_symbol == 0 {
                    h.has_epsilon_epsilon_transitions = true;
                }
            }

            if node_input_symbols.contains(&l.input_symbol) {
                h.input_deterministic = false;
            } else {
                node_input_symbols.insert(l.input_symbol);
            }

            if transition_labels.contains(&l) {
                h.deterministic = false;
            } else {
                transition_labels.insert(l);
            }

            if nodes_in_path.contains(&target) {
                h.cyclic = true;
            }

            Self::full_traversal(
                h,
                tr,
                target,
                visited_nodes,
                nodes_in_path,
                all_input_symbols,
            );
        }
        nodes_in_path.remove(&n);
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transducer-header.find-input-epsilon-cycles-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transducer-header.find-input-epsilon-cycles-fn]
    fn find_input_epsilon_cycles(
        n: StateId,
        start: StateId,
        epsilon_targets: &mut StateIdSet,
        unweighted_only: bool,
        tr: &StdVectorFst,
        h: &mut TransducerHeader,
    ) {
        let isym = tr.input_symbols().unwrap().clone();
        let trs = tr.get_trs(n).unwrap();
        for a in trs.trs().iter() {
            let sym = isym.get_symbol(a.ilabel).unwrap_or("").to_string();
            if a.ilabel != 0 || FdOperation::is_diacritic(&sym) {
                continue;
            } else if a.weight != TropicalWeight::zero() {
                continue;
            }

            let target = a.nextstate;
            if start == target {
                if unweighted_only {
                    h.has_unweighted_input_epsilon_cycles = true;
                }
                h.has_input_epsilon_cycles = true;
                return;
            }

            if epsilon_targets.contains(&target) {
                epsilon_targets.insert(target);
                Self::find_input_epsilon_cycles(
                    target,
                    start,
                    epsilon_targets,
                    unweighted_only,
                    tr,
                    h,
                );
            }

            if h.has_input_epsilon_cycles || h.has_unweighted_input_epsilon_cycles {
                return;
            }
        }
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transducer-header.compute-header-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transducer-header.compute-header-fn]
    pub fn compute_header(
        header: &mut TransducerHeader,
        t: &StdVectorFst,
        symbol_count: SymbolNumber,
        number_of_index_table_entries: TransitionTableIndex,
        number_of_target_table_entries: TransitionTableIndex,
        weighted: bool,
    ) {
        // Initial values, many will be modified by the following function calls
        header.number_of_input_symbols = 0;
        header.number_of_symbols = symbol_count;
        header.size_of_transition_index_table = number_of_index_table_entries;
        header.size_of_transition_target_table = number_of_target_table_entries;
        header.number_of_states = 0;
        header.number_of_transitions = 0;
        header.weighted = weighted;
        header.deterministic = true;
        header.input_deterministic = true;
        header.minimized = false; // (upstream convert.cc:1010 leaves this hardcoded false)
        header.cyclic = false;
        header.has_epsilon_epsilon_transitions = false;
        header.has_input_epsilon_transitions = false;
        header.has_input_epsilon_cycles = false;
        header.has_unweighted_input_epsilon_cycles = false;

        let mut nodes = StateIdSet::new();
        let mut nodes_in_path = StateIdSet::new();
        let mut input_symbols: OfstSymbolSet = OfstSymbolSet::new();
        input_symbols.insert(0);
        let start = t.start().unwrap();
        Self::full_traversal(
            header,
            t,
            start,
            &mut nodes,
            &mut nodes_in_path,
            &mut input_symbols,
        );

        header.number_of_input_symbols = input_symbols.len() as SymbolNumber;
        header.number_of_states = nodes.len() as StateIdNumber;
        if !header.weighted {
            header.has_unweighted_input_epsilon_cycles = header.has_input_epsilon_cycles;
        }
    }
}

// 'static ConvertTransducer* ConvertTransducer::constructing_transducer = NULL;'
// The static-mut-ness is gone (safe thread-local Cell); the value is still a
// transient self-borrow into the in-construction transducer (the C++ static
// pointer pattern), so 'constructing_transducer' stays an unsafe-deref island.
thread_local! {
    static CONSTRUCTING_TRANSDUCER: std::cell::Cell<*mut ConvertTransducer> =
        const { std::cell::Cell::new(std::ptr::null_mut()) };
}

// Read-only access to the transducer currently being constructed, exactly as
// the C++ reaches it through the 'constructing_transducer' static.
//
// SAFETY-ISLAND [convert-engine]: the format-conversion graph builder is
// self-referential — a `ConvertTransducer` republishes a raw pointer to itself
// through `CONSTRUCTING_TRANSDUCER` so the `ConvertFstState`/`ConvertTransition`
// it builds can read back the source `fst`. The pointer is non-null and valid for
// the whole construction (the box outlives every state built from it); the
// returned `&'a` is only ever read, never aliased mutably. The other raw-pointer
// `unsafe` blocks in this file belong to the same island. Removing it needs an
// index/arena rewrite of the conversion algorithm (deferred).
unsafe fn constructing_transducer<'a>() -> &'a ConvertTransducer {
    unsafe {
        let p = CONSTRUCTING_TRANSDUCER.with(|c| c.get());
        &*p
    }
}

// [spec:hfst:def:convert.hfst-ol.convert-transducer.add-input-symbols-fn]
// [spec:hfst:sem:convert.hfst-ol.convert-transducer.add-input-symbols-fn]
fn convert_transducer_add_input_symbols(
    fst: &StdVectorFst,
    n: StateId,
    input_symbols: &mut SymbolNumberSet,
    visited_nodes: &mut StateIdSet,
) {
    let trs = fst.get_trs(n).unwrap();
    for a in trs.trs().iter() {
        input_symbols.insert(a.ilabel as SymbolNumber);
        if !visited_nodes.contains(&a.nextstate) {
            visited_nodes.insert(a.nextstate);
            convert_transducer_add_input_symbols(fst, a.nextstate, input_symbols, visited_nodes);
        }
    }
}

// [spec:hfst:def:convert.hfst-ol.convert-transducer.number-of-input-symbols-fn]
// [spec:hfst:sem:convert.hfst-ol.convert-transducer.number-of-input-symbols-fn]
fn convert_transducer_number_of_input_symbols(fst: &StdVectorFst) -> SymbolNumber {
    let mut input_symbol_set = SymbolNumberSet::new();
    input_symbol_set.insert(0);
    let mut visited_nodes = StateIdSet::new();
    let start = fst.start().unwrap();
    convert_transducer_add_input_symbols(fst, start, &mut input_symbol_set, &mut visited_nodes);
    input_symbol_set.len() as SymbolNumber
}

// [spec:hfst:def:convert.hfst-ol.convert-transducer]
pub struct ConvertTransducer {
    fst: *const StdVectorFst,
    id_number_map: Option<Box<ConvertIdNumberMap>>,
    fst_indices: Option<Box<ConvertTransitionTableIndices>>,
    index_table_size: usize,

    header: TransducerHeader,
    alphabet: ConvertTransducerAlphabet,
    states: Vec<Box<ConvertFstState>>,
}

impl ConvertTransducer {
    // [spec:hfst:def:convert.hfst-ol.convert-transducer.convert-transducer-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transducer.convert-transducer-fn]
    pub fn new(tr: *const StdVectorFst, weighted: bool) -> Box<ConvertTransducer> {
        // IDIOM-STAGE-2: the fst is reached via a raw pointer here because it is
        // stored in 'self.fst' and republished through the CONSTRUCTING_TRANSDUCER
        // thread-local for the read-back callbacks (the C++ file-static pattern);
        // a borrow can't outlive that. The fst-reading helpers below all take
        // '&StdVectorFst', so dereference once at this boundary.
        let tr_ref: &StdVectorFst = unsafe { &*tr };
        // C++ member-initialiser order: fst, id_number_map, fst_indices, header,
        // alphabet.
        let id_number_map = Box::new(ConvertIdNumberMap::new(tr_ref));
        let fst_indices = Box::new(ConvertTransitionTableIndices::new(
            convert_transducer_number_of_input_symbols(tr_ref),
        ));
        let header = TransducerHeader::new_weighted(weighted);
        let alphabet = ConvertTransducerAlphabet::new(tr_ref);

        let mut bx = Box::new(ConvertTransducer {
            fst: tr,
            id_number_map: Some(id_number_map),
            fst_indices: Some(fst_indices),
            index_table_size: 0,
            header,
            alphabet,
            states: Vec::new(),
        });

        CONSTRUCTING_TRANSDUCER.with(|c| c.set(bx.as_mut() as *mut ConvertTransducer));
        // C++ 'id_number_map = new ConvertIdNumberMap(tr);' a second time (the
        // first allocation leaks); we simply replace it.
        bx.id_number_map = Some(Box::new(ConvertIdNumberMap::new(tr_ref)));

        let p: *mut ConvertTransducer = bx.as_mut();
        unsafe {
            // std::cout << "Creating state structures" << std::endl;
            (*p).read_nodes();
            // std::cout << "Laying out transition table" << std::endl;
            (*p).set_transition_table_indices();
            // std::cout << "Laying out transition index table" << std::endl;
            (*p).set_index_table_indices();

            (*p).index_table_size = (*p).fst_indices.as_ref().unwrap().size();
            (*p).fst_indices = None;

            // std::cout << "Computing header properties" << std::endl;
            let symbol_count = (*p).alphabet.get_symbol_table().len() as SymbolNumber;
            let target_table_entries = (*p).count_transitions();
            let index_table_size = (*p).index_table_size as TransitionTableIndex;
            ConvertTransducerHeader::compute_header(
                &mut (*p).header,
                tr_ref,
                symbol_count,
                index_table_size,
                target_table_entries,
                weighted,
            );

            (*p).id_number_map = None;
        }
        CONSTRUCTING_TRANSDUCER.with(|c| c.set(std::ptr::null_mut()));
        bx
    }

    pub fn get_id_number_map(&self) -> &ConvertIdNumberMap {
        self.id_number_map.as_deref().unwrap()
    }
    pub fn get_alphabet(&self) -> &ConvertTransducerAlphabet {
        &self.alphabet
    }
    pub fn get_state(&self, s: StateIdNumber) -> &ConvertFstState {
        &*self.states[s as usize]
    }
    // [spec:hfst:def:convert.hfst-ol.convert-transducer.is-weighted-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transducer.is-weighted-fn]
    pub fn is_weighted(&self) -> bool {
        self.header.probe_flag(HeaderFlag::Weighted)
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transducer.read-nodes-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transducer.read-nodes-fn]
    fn read_nodes(&mut self) {
        let number_of_nodes = self.id_number_map.as_ref().unwrap().get_number_of_nodes();
        for id in 0..number_of_nodes {
            let n = self.id_number_map.as_ref().unwrap().get_id_node(id);
            // self.fst is the IDIOM-STAGE-2 island raw pointer (see new()).
            let state = ConvertFstState::new(n, unsafe { &*self.fst });
            self.states.push(Box::new(state));
        }
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transducer.set-transition-table-indices-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transducer.set-transition-table-indices-fn]
    fn set_transition_table_indices(&mut self) {
        let mut place = TRANSITION_TARGET_TABLE_START;
        for i in 0..self.states.len() {
            place = self.states[i].set_transition_table_indices(place);
        }
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transducer.set-index-table-indices-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transducer.set-index-table-indices-fn]
    fn set_index_table_indices(&mut self) {
        let mut state_set: Vec<*mut ConvertFstState> = Vec::new();

        for i in 1..self.states.len() {
            let p: *mut ConvertFstState = &mut *self.states[i];
            state_set_insert(&mut state_set, p);
        }

        let start_state: *mut ConvertFstState = &mut *self.states[0];
        let start_state_index = self
            .fst_indices
            .as_mut()
            .unwrap()
            .add_state(unsafe { &*start_state });

        unsafe {
            (*start_state).set_table_index(start_state_index as TransitionTableIndex);
        }

        for k in (0..state_set.len()).rev() {
            let state = state_set[k];
            let state_index: TransitionTableIndex;
            if unsafe { (*state).is_big_state() } {
                state_index =
                    self.fst_indices
                        .as_mut()
                        .unwrap()
                        .add_state(unsafe { &*state }) as TransitionTableIndex;
            } else {
                state_index = unsafe { (*state).get_first_transition_index() } - 1;
                if state_index < TRANSITION_TARGET_TABLE_START {
                    tracing::error!("FIXME!");
                    // C++ does a bare throw here (no active exception, so it terminates).
                    std::process::abort();
                }
            }

            unsafe {
                (*state).set_table_index(state_index);
            }
        }

        // now that the state object's all know their table location, update the
        // transition objects with that information
        for i in 0..self.states.len() {
            self.states[i].set_transition_target_indices();
        }
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transducer.count-transitions-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transducer.count-transitions-fn]
    fn count_transitions(&self) -> TransitionTableIndex {
        let mut transition_count: TransitionTableIndex = 0;
        for it in self.states.iter() {
            // Separator between states;
            transition_count += 1;

            transition_count += it.number_of_transitions() as TransitionTableIndex;
        }
        transition_count
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transducer.display-states-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transducer.display-states-fn]
    pub fn display_states(&self) {
        println!("Transducer states:");
        for it in self.states.iter() {
            it.display();
        }
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transducer.display-tables-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transducer.display-tables-fn]
    pub fn display_tables(&self) {
        println!("Transducer tables:");
        println!("----------");
        if self.is_weighted() {
            println!(" Transition index table:");
            self.make_index_table::<TransitionWIndex>(
                self.index_table_size as TransitionTableIndex,
            )
            .display_index();
            println!(" Transition table:");
            self.make_transition_table::<TransitionW>()
                .display_transition();
        } else {
            println!(" Transition index table:");
            self.make_index_table::<TransitionIndex>(self.index_table_size as TransitionTableIndex)
                .display_index();
            println!(" Transition table:");
            self.make_transition_table::<Transition>()
                .display_transition();
        }
        println!("----------");
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transducer.make-index-table-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transducer.make-index-table-fn]
    fn make_index_table<T: ConvertIndexEntry + TableEntry + Clone>(
        &self,
        index_table_size: TransitionTableIndex,
    ) -> TransducerTable<T> {
        let mut index_table =
            TransducerTable::<T>::new_filled(index_table_size as usize, T::ct_default());

        for state in self.states.iter() {
            state.insert_transition_indices(&mut index_table);
        }

        index_table
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transducer.make-transition-table-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transducer.make-transition-table-fn]
    fn make_transition_table<T: ConvertTransitionEntry + TableEntry + Clone>(
        &self,
    ) -> TransducerTable<T> {
        let mut transition_table = TransducerTable::<T>::new();
        let mut place = TRANSITION_TARGET_TABLE_START;
        for it in self.states.iter() {
            place = it.append_transitions(&mut transition_table, place);
        }
        transition_table.append(T::ct_final(false, INFINITE_WEIGHT));

        transition_table
    }

    // [spec:hfst:def:convert.hfst-ol.convert-transducer.to-transducer-fn]
    // [spec:hfst:sem:convert.hfst-ol.convert-transducer.to-transducer-fn]
    pub fn to_transducer(&self) -> Transducer {
        // std::cout << "Building new transducer" << std::endl;
        if self.is_weighted() {
            Transducer::new_from_tables_weighted(
                &self.header,
                &self.alphabet.to_alphabet(),
                self.make_index_table::<TransitionWIndex>(
                    self.index_table_size as TransitionTableIndex,
                ),
                self.make_transition_table::<TransitionW>(),
            )
        } else {
            Transducer::new_from_tables_unweighted(
                &self.header,
                &self.alphabet.to_alphabet(),
                self.make_index_table::<TransitionIndex>(
                    self.index_table_size as TransitionTableIndex,
                ),
                self.make_transition_table::<Transition>(),
            )
        }
    }
}
