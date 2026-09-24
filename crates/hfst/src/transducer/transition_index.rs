//! Index-table entries, unweighted and weighted.

use super::*;

/// The (object-safe) virtual surface of 'TransitionIndex' (base) used through
/// base references; 'TransitionWIndex' overrides 'final_weight'. The static
/// 'create_final()' lives in ['IndexCtor'] so this stays dyn-compatible.
pub trait IndexEntry {
    fn get_target(&self) -> TransitionTableIndex;
    fn get_input_symbol(&self) -> SymbolNumber;
    fn matches(&self, s: SymbolNumber) -> bool;
    fn is_final(&self) -> bool;
    fn final_weight(&self) -> Weight;
    fn write(&self, os: &mut dyn std::io::Write, weighted: bool);
    fn display(&self);
}

/// 'static TransitionIndex::create_final()' — a generic-bound-only trait
/// (returns 'Self', so it can't ride on the dyn-safe ['IndexEntry']).
pub trait IndexCtor {
    fn create_final() -> Self;
    /// Whether this index type belongs to the weighted table pair — the
    /// static counterpart of the header's 'Weighted' flag.
    const WEIGHTED: bool;
}

// [spec:hfst:def:transducer.hfst-ol.transition-index]
#[derive(Clone)]
pub struct TransitionIndex {
    pub(crate) input_symbol: SymbolNumber,
    pub(crate) first_transition_index: TransitionTableIndex,
}

impl TransitionIndex {
    pub fn new() -> Self {
        TransitionIndex {
            input_symbol: NO_SYMBOL_NUMBER,
            first_transition_index: NO_TABLE_INDEX,
        }
    }

    pub fn new_values(input: SymbolNumber, first_transition: TransitionTableIndex) -> Self {
        TransitionIndex {
            input_symbol: input,
            first_transition_index: first_transition,
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transition-index.transition-index-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-index.transition-index-fn]
    pub fn read_from(is: &mut dyn std::io::BufRead) -> crate::error::Result<Self> {
        Ok(TransitionIndex {
            input_symbol: read_u16(is)?,
            first_transition_index: read_u32(is)?,
        })
    }

    // [spec:hfst:def:transducer.hfst-ol.transition-index.get-target-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-index.get-target-fn]
    pub fn get_target(&self) -> TransitionTableIndex {
        self.first_transition_index
    }
    // [spec:hfst:def:transducer.hfst-ol.transition-index.get-input-symbol-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-index.get-input-symbol-fn]
    pub fn get_input_symbol(&self) -> SymbolNumber {
        self.input_symbol
    }

    // [spec:hfst:def:transducer.hfst-ol.transition-index.create-final-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-index.create-final-fn]
    pub fn create_final() -> TransitionIndex {
        TransitionIndex::new_values(NO_SYMBOL_NUMBER, 1)
    }

    // [spec:hfst:def:transducer.hfst-ol.transition-index.display-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-index.display-fn]
    pub fn display(&self) {
        println!(
            "input_symbol: {}, target: {}{}",
            self.input_symbol,
            self.first_transition_index,
            if self.is_final() { " (final)" } else { "" }
        );
    }

    // [spec:hfst:def:transducer.hfst-ol.transition-index.matches-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-index.matches-fn]
    pub fn matches(&self, s: SymbolNumber) -> bool {
        self.input_symbol != NO_SYMBOL_NUMBER && self.input_symbol == s
    }
    // [spec:hfst:def:transducer.hfst-ol.transition-index.final-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-index.final-fn]
    pub fn is_final(&self) -> bool {
        self.input_symbol == NO_SYMBOL_NUMBER && self.first_transition_index != NO_TABLE_INDEX
    }
    // [spec:hfst:def:transducer.hfst-ol.transition-index.final-weight-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-index.final-weight-fn]
    pub fn final_weight(&self) -> Weight {
        0.0
    }

    // [spec:hfst:def:transducer.hfst-ol.transition-index.write-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-index.write-fn]
    pub fn write(&self, os: &mut dyn std::io::Write, weighted: bool) {
        write_u16(self.input_symbol, os);
        if !weighted
            && self.input_symbol == NO_SYMBOL_NUMBER
            && self.first_transition_index != NO_TABLE_INDEX
        {
            // Make sure that we write the correct type of final index
            let unweighted_final_index: u32 = 1;
            write_u32(unweighted_final_index, os);
        } else {
            write_u32(self.first_transition_index, os);
        }
    }
}

impl Default for TransitionIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl TableEntry for TransitionIndex {
    const SIZE: usize = 2 + 4; // sizeof(SymbolNumber) + sizeof(TransitionTableIndex)
    fn from_bytes(p: &[u8]) -> Self {
        // Little-endian per hfst/hfst#328 (see module docs).
        TransitionIndex {
            input_symbol: u16::from_le_bytes([p[0], p[1]]),
            first_transition_index: u32::from_le_bytes([p[2], p[3], p[4], p[5]]),
        }
    }
}

impl IndexEntry for TransitionIndex {
    fn get_target(&self) -> TransitionTableIndex {
        self.get_target()
    }
    fn get_input_symbol(&self) -> SymbolNumber {
        self.get_input_symbol()
    }
    fn matches(&self, s: SymbolNumber) -> bool {
        self.matches(s)
    }
    fn is_final(&self) -> bool {
        self.is_final()
    }
    fn final_weight(&self) -> Weight {
        self.final_weight()
    }
    fn write(&self, os: &mut dyn std::io::Write, weighted: bool) {
        self.write(os, weighted)
    }
    fn display(&self) {
        self.display()
    }
}

impl IndexCtor for TransitionIndex {
    fn create_final() -> Self {
        TransitionIndex::create_final()
    }
    const WEIGHTED: bool = false;
}

// [spec:hfst:def:transducer.hfst-ol.transition-w-index]
#[derive(Clone)]
pub struct TransitionWIndex {
    pub(crate) base: TransitionIndex,
}

impl TransitionWIndex {
    pub fn new() -> Self {
        TransitionWIndex {
            base: TransitionIndex::new(),
        }
    }
    // [spec:hfst:def:transducer.hfst-ol.transition-w-index.transition-w-index-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-w-index.transition-w-index-fn]
    pub fn new_values(input: SymbolNumber, first_transition: TransitionTableIndex) -> Self {
        TransitionWIndex {
            base: TransitionIndex::new_values(input, first_transition),
        }
    }

    pub fn get_target(&self) -> TransitionTableIndex {
        self.base.get_target()
    }
    pub fn get_input_symbol(&self) -> SymbolNumber {
        self.base.get_input_symbol()
    }
    pub fn matches(&self, s: SymbolNumber) -> bool {
        self.base.matches(s)
    }
    pub fn is_final(&self) -> bool {
        self.base.is_final()
    }

    // [spec:hfst:def:transducer.hfst-ol.transition-w-index.final-weight-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-w-index.final-weight-fn]
    pub fn final_weight(&self) -> Weight {
        // union { TransitionTableIndex i; Weight w; }; weight.i = first; return weight.w;
        Weight::from_bits(self.base.first_transition_index)
    }

    pub fn create_final() -> TransitionWIndex {
        TransitionWIndex::new_values(NO_SYMBOL_NUMBER, 0)
    }

    // [spec:hfst:def:transducer.hfst-ol.transition-w-index.create-final-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-w-index.create-final-fn]
    pub fn create_final_weight(w: Weight) -> TransitionWIndex {
        // union to_weight { TransitionTableIndex i; Weight w; }; weight.w = w;
        TransitionWIndex::new_values(NO_SYMBOL_NUMBER, w.to_bits())
    }

    pub fn write(&self, os: &mut dyn std::io::Write, weighted: bool) {
        self.base.write(os, weighted)
    }
    pub fn display(&self) {
        self.base.display()
    }
}

impl Default for TransitionWIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl TableEntry for TransitionWIndex {
    const SIZE: usize = 2 + 4;
    fn from_bytes(p: &[u8]) -> Self {
        TransitionWIndex {
            base: TransitionIndex::from_bytes(p),
        }
    }
}

impl IndexEntry for TransitionWIndex {
    fn get_target(&self) -> TransitionTableIndex {
        self.get_target()
    }
    fn get_input_symbol(&self) -> SymbolNumber {
        self.get_input_symbol()
    }
    fn matches(&self, s: SymbolNumber) -> bool {
        self.matches(s)
    }
    fn is_final(&self) -> bool {
        self.is_final()
    }
    fn final_weight(&self) -> Weight {
        self.final_weight()
    }
    fn write(&self, os: &mut dyn std::io::Write, weighted: bool) {
        self.write(os, weighted)
    }
    fn display(&self) {
        self.display()
    }
}

impl IndexCtor for TransitionWIndex {
    fn create_final() -> Self {
        TransitionWIndex::create_final()
    }
    const WEIGHTED: bool = true;
}
