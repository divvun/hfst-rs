//! Port of the OpenFST-independent part of
//! 'libhfst/src/implementations/optimized-lookup/convert.{h,cc}' (namespace
//! 'hfst_ol').
//!
//! These are the placeholder structures and helpers that the
//! 'HfstBasicTransducer -> hfst_ol::Transducer' conversion (in
//! 'convert_ol_transducer.rs') builds the optimized-lookup index/transition
//! tables from.
//!
//! The C++ '#if HAVE_OPENFST' block — the 'Convert*' classes that turn an
//! 'fst::StdVectorFst' straight into OL tables via a 'static ConvertTransducer*
//! constructing_transducer' file-static pointer plus intra-object raw pointers
//! ('ConvertTransitionIndex' -> 'ConvertTransition' -> the constructing
//! transducer) — had no callers in this port: the live path builds OL tables
//! from an 'HfstBasicTransducer' through the placeholder helpers below. It was
//! dead code, so it has been removed along with its raw pointers and global.
//!
//! Aliasing note: 'write_transitions_from_state_placeholders' /
//! 'add_transitions_with' are passed both a slice of one state's transition
//! placeholders *and* the whole 'state_placeholders' vector — in the C++ these
//! are non-const references but neither is mutated, so both are immutable
//! borrows here and the overlap is fine.

use std::collections::{BTreeMap, BTreeSet};

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

/// The set of flag-diacritic symbol numbers, kept both as an ordered set (the
/// emission order of flags is part of the OL byte format) and as a dense
/// membership mask (contains() sits in the packer's innermost loops).
#[derive(Clone, Default)]
pub struct FlagSymbolSet {
    set: BTreeSet<SymbolNumber>,
    mask: Vec<bool>,
}

impl FlagSymbolSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, symbol: SymbolNumber) {
        if self.mask.len() <= symbol as usize {
            self.mask.resize(symbol as usize + 1, false);
        }
        self.mask[symbol as usize] = true;
        self.set.insert(symbol);
    }

    #[inline]
    pub fn contains(&self, symbol: SymbolNumber) -> bool {
        (symbol as usize) < self.mask.len() && self.mask[symbol as usize]
    }

    pub fn iter(&self) -> impl Iterator<Item = &SymbolNumber> {
        self.set.iter()
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
    // Per-group cumulative transition offsets (parallel to
    // transition_placeholders), built by build_symbol_offsets once the
    // transitions are final; answers symbol_offset queries in O(1).
    pub group_offsets: Vec<u32>,
    pub ty: IndexingType,
    pub inputs: SymbolNumber,
    pub is_final: bool,
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
            group_offsets: Vec::new(),
            ty: if state == 0 {
                IndexingType::nonsimple
            } else {
                IndexingType::empty
            },
            inputs: 0,
            is_final: finality,
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
            group_offsets: Vec::new(),
            ty: IndexingType::empty,
            inputs: 0,
            is_final: false,
            final_weight: 0.0,
        }
    }

    // [spec:hfst:def:convert.hfst-ol.state-placeholder.is-simple-fn]
    // [spec:hfst:sem:convert.hfst-ol.state-placeholder.is-simple-fn]
    pub fn is_simple(&self) -> bool {
        self.ty != IndexingType::nonsimple
    }

    // [spec:hfst:def:convert.hfst-ol.state-placeholder.number-of-transitions-fn]
    // [spec:hfst:sem:convert.hfst-ol.state-placeholder.number-of-transitions-fn]
    pub fn number_of_transitions(&self) -> u32 {
        self.transition_placeholders
            .iter()
            .map(|it| u32::try_from(it.len()).expect("value out of u32 range"))
            .sum()
    }

    // [spec:hfst:def:convert.hfst-ol.state-placeholder.input-present-fn]
    // [spec:hfst:sem:convert.hfst-ol.state-placeholder.input-present-fn]
    pub fn input_present(&self, input: SymbolNumber) -> bool {
        (input as usize) < self.symbol_to_transition_placeholder_v.len()
            && self.symbol_to_transition_placeholder_v[input as usize] != u32::MAX
    }

    // [spec:hfst:def:convert.hfst-ol.state-placeholder.add-input-fn]
    // [spec:hfst:sem:convert.hfst-ol.state-placeholder.add-input-fn]
    pub fn add_input(&mut self, input: SymbolNumber, flag_symbols: &FlagSymbolSet) {
        if self.input_present(input) {
            return;
        }
        while self.symbol_to_transition_placeholder_v.len() <= input as usize {
            self.symbol_to_transition_placeholder_v.push(u32::MAX);
        }
        self.symbol_to_transition_placeholder_v[input as usize] =
            u32::try_from(self.transition_placeholders.len()).expect("value out of u32 range");
        self.transition_placeholders.push(Vec::new());
        self.inputs += 1;
        if self.ty != IndexingType::nonsimple {
            // Depending on what type of inputs we now have, adjust the index
            // type. Epsilons and flags both index to 0. If we have only one
            // input symbol, we're simple.
            if self.ty == IndexingType::empty {
                if input == 0 || flag_symbols.contains(input) {
                    self.ty = IndexingType::simple_zero_index;
                } else {
                    self.ty = IndexingType::simple_nonzero_index;
                }
            } else if self.ty == IndexingType::simple_zero_index {
                if input != 0 && !flag_symbols.contains(input) {
                    self.ty = IndexingType::nonsimple;
                }
            } else {
                // simple_nonzero_index
                if self.inputs > 1 || input == 0 || flag_symbols.contains(input) {
                    self.ty = IndexingType::nonsimple;
                }
            }
        }
    }

    // [spec:hfst:def:convert.hfst-ol.state-placeholder.get-largest-index-fn]
    // [spec:hfst:sem:convert.hfst-ol.state-placeholder.get-largest-index-fn]
    pub fn get_largest_index(&self) -> SymbolNumber {
        let back = *self
            .symbol_to_transition_placeholder_v
            .last()
            .expect("symbol_to_transition_placeholder_v is non-empty");
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
        flag_symbols: &FlagSymbolSet,
    ) -> crate::error::Result<u32> {
        if symbol == 0 {
            return Ok(0);
        }
        let mut offset: u32 = 0;
        if self.input_present(0) {
            // if there are epsilons
            offset = u32::try_from(self.get_transition_placeholders(0).len())
                .expect("value out of u32 range");
        }
        for flag_it in flag_symbols.iter() {
            if self.input_present(*flag_it) {
                if symbol == *flag_it {
                    // Flags go to 0 (even if there's no epsilon)
                    return Ok(0);
                }
                offset += u32::try_from(self.get_transition_placeholders(*flag_it).len())
                    .expect("value out of u32 range");
            }
        }
        for i in 1..self.symbol_to_transition_placeholder_v.len() {
            let i = i as SymbolNumber;
            if self.input_present(i) {
                if flag_symbols.contains(i) {
                    // already counted
                    continue;
                }
                if symbol == i {
                    return Ok(offset);
                }
                offset += u32::try_from(self.get_transition_placeholders(i).len())
                    .expect("value out of u32 range");
            }
        }
        let message = String::from(
            "error in conversion between optimized lookup format and \
             HfstTransducer;\ntried to calculate symbol_offset for symbol not \
             present in state",
        );
        crate::bail!(Fatal, message)
    }

    /// One pass over the state's symbol layout, producing every group's offset
    /// (the value symbol_offset would compute per query) in group_offsets.
    /// Epsilon and flag groups sit at offset 0; non-flag groups accumulate in
    /// the same order symbol_offset scans them.
    pub fn build_symbol_offsets(&mut self, flag_symbols: &FlagSymbolSet) {
        self.group_offsets = vec![0; self.transition_placeholders.len()];
        let mut offset: u32 = 0;
        if self.input_present(0) {
            offset = u32::try_from(self.get_transition_placeholders(0).len())
                .expect("value out of u32 range");
        }
        for &flag_it in flag_symbols.iter() {
            if self.input_present(flag_it) {
                offset += u32::try_from(self.get_transition_placeholders(flag_it).len())
                    .expect("value out of u32 range");
            }
        }
        for i in 1..self.symbol_to_transition_placeholder_v.len() {
            let i = i as SymbolNumber;
            if self.input_present(i) && !flag_symbols.contains(i) {
                let group = self.symbol_to_transition_placeholder_v[i as usize] as usize;
                self.group_offsets[group] = offset;
                offset += u32::try_from(self.transition_placeholders[group].len())
                    .expect("value out of u32 range");
            }
        }
    }

    /// O(1) equivalent of symbol_offset for symbols present in this state;
    /// requires build_symbol_offsets to have run.
    pub fn symbol_offset_cached(
        &self,
        symbol: SymbolNumber,
        flag_symbols: &FlagSymbolSet,
    ) -> crate::error::Result<u32> {
        if symbol == 0 || flag_symbols.contains(symbol) {
            return Ok(0);
        }
        if !self.input_present(symbol) {
            let message = String::from(
                "error in conversion between optimized lookup format and \
                 HfstTransducer;\ntried to calculate symbol_offset for symbol not \
                 present in state",
            );
            crate::bail!(Fatal, message)
        }
        Ok(self.group_offsets[self.symbol_to_transition_placeholder_v[symbol as usize] as usize])
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
    // One bit per index position, mirroring indices[p] != NO_TABLE_INDEX. The
    // first-fit search probes millions of positions; the bitset keeps those
    // probes in cache where the u32 vector would not be.
    used_bits: Vec<u64>,
}

impl IndexPlaceholders {
    pub fn new() -> Self {
        IndexPlaceholders {
            indices: Vec::new(),
            targets: Vec::new(),
            used_bits: Vec::new(),
        }
    }

    // [spec:hfst:def:convert.hfst-ol.index-placeholders.used-fn]
    // [spec:hfst:sem:convert.hfst-ol.index-placeholders.used-fn]
    #[inline]
    pub fn used(&self, position: u32) -> bool {
        let word = (position >> 6) as usize;
        word < self.used_bits.len() && (self.used_bits[word] >> (position & 63)) & 1 == 1
    }

    // [spec:hfst:def:convert.hfst-ol.index-placeholders.assign-fn]
    // [spec:hfst:sem:convert.hfst-ol.index-placeholders.assign-fn]
    pub fn assign(&mut self, position: u32, target: u32, sym: SymbolNumber) {
        while position >= self.indices.len() as u32 {
            self.indices.push(NO_TABLE_INDEX);
        }
        self.indices[position as usize] =
            u32::try_from(self.targets.len()).expect("value out of u32 range");
        self.targets.push((target, sym));
        let word = (position >> 6) as usize;
        if word >= self.used_bits.len() {
            self.used_bits.resize(word + 1, 0);
        }
        self.used_bits[word] |= 1u64 << (position & 63);
    }

    /// Count set bits in positions [start, start + len).
    fn count_used(&self, start: u32, len: u32) -> u32 {
        let end = start as u64 + len as u64; // exclusive
        let last_word = ((end - 1) >> 6) as usize;
        let mut count = 0u32;
        let mut word_idx = (start >> 6) as usize;
        while word_idx < self.used_bits.len() && word_idx <= last_word {
            let mut word = self.used_bits[word_idx];
            let word_start = (word_idx as u64) << 6;
            if word_start < start as u64 {
                word &= !0u64 << (start as u64 - word_start);
            }
            if word_start + 64 > end {
                word &= !0u64 >> (word_start + 64 - end);
            }
            count += word.count_ones();
            word_idx += 1;
        }
        count
    }

    // [spec:hfst:def:convert.hfst-ol.index-placeholders.get-target-fn]
    // [spec:hfst:sem:convert.hfst-ol.index-placeholders.get-target-fn]
    pub fn get_target(&self, index: u32) -> (u32, SymbolNumber) {
        self.targets[self.indices[index as usize] as usize]
    }

    // [spec:hfst:def:convert.hfst-ol.index-placeholders.fits-fn]
    // [spec:hfst:sem:convert.hfst-ol.index-placeholders.fits-fn]
    // Takes the state's flag-resolved index offsets (position-invariant, so the
    // caller computes them once per state instead of once per probed position).
    #[inline]
    pub fn fits(&self, state_offsets: &[SymbolNumber], position: u32) -> bool {
        if self.used(position) {
            return false;
        }
        for &index_offset in state_offsets {
            if self.used(index_offset as u32 + position + 1) {
                return false;
            }
        }
        true
    }

    // [spec:hfst:def:convert.hfst-ol.index-placeholders.unsuitable-fn]
    // [spec:hfst:sem:convert.hfst-ol.index-placeholders.unsuitable-fn]
    // The C++ scans position by position with an early exit; since the running
    // count only grows and the threshold is fixed, "some prefix reaches the
    // threshold" is equivalent to "the full window's count reaches it", which a
    // word-level popcount answers with identical results.
    pub fn unsuitable(&self, index: u32, symbols: SymbolNumber, packing_aggression: f32) -> bool {
        if self.used(index) {
            return true;
        }
        if symbols == 0 {
            return false;
        }
        let filled = self.count_used(index + 1, symbols as u32);
        filled as f32 >= packing_aggression * symbols as f32
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
    flag_symbols: &FlagSymbolSet,
) {
    for idx in 0..state_placeholders.len() {
        let it = &state_placeholders[idx];

        // Insert a finality marker unless this is the first state, the finality
        // of which is determined by the index table
        if it.state_number != 0 {
            transition_table.append(TransitionW::new_final(it.is_final, it.final_weight));
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
            if !it.input_present(i) || flag_symbols.contains(i) {
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
    _flag_symbols: &FlagSymbolSet,
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

// 'NO_STATE_ID' (the C++ 'NO_ID_NUMBER'/'BIG_STATE_LIMIT' were used only by the
// removed Convert* fst->OL classes).
pub const NO_STATE_ID: StateId = u32::MAX;
