//! Transition-table entries, unweighted and weighted.

use super::*;

/// The (object-safe) virtual surface of 'Transition' (base); 'TransitionW'
/// overrides 'get_weight'.
pub trait TransitionEntry {
    fn get_target(&self) -> TransitionTableIndex;
    fn get_output_symbol(&self) -> SymbolNumber;
    fn get_input_symbol(&self) -> SymbolNumber;
    fn matches(&self, s: SymbolNumber) -> bool;
    fn is_final(&self) -> bool;
    fn get_weight(&self) -> Weight;
    fn write(&self, os: &mut dyn std::io::Write, weighted: bool);
    fn display(&self);
}

// [spec:hfst:def:transducer.hfst-ol.transition]
#[derive(Clone)]
pub struct Transition {
    pub(crate) input_symbol: SymbolNumber,
    pub(crate) output_symbol: SymbolNumber,
    pub(crate) target_index: TransitionTableIndex,
}

impl Transition {
    pub fn new_values(
        input: SymbolNumber,
        output: SymbolNumber,
        target: TransitionTableIndex,
    ) -> Self {
        Transition {
            input_symbol: input,
            output_symbol: output,
            target_index: target,
        }
    }

    pub fn new_final(is_final: bool) -> Self {
        Transition {
            input_symbol: NO_SYMBOL_NUMBER,
            output_symbol: NO_SYMBOL_NUMBER,
            target_index: if is_final { 1 } else { NO_TABLE_INDEX },
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transition.get-target-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition.get-target-fn]
    pub fn get_target(&self) -> TransitionTableIndex {
        self.target_index
    }
    // [spec:hfst:def:transducer.hfst-ol.transition.get-output-symbol-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition.get-output-symbol-fn]
    pub fn get_output_symbol(&self) -> SymbolNumber {
        self.output_symbol
    }
    // [spec:hfst:def:transducer.hfst-ol.transition.get-input-symbol-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition.get-input-symbol-fn]
    pub fn get_input_symbol(&self) -> SymbolNumber {
        self.input_symbol
    }

    // [spec:hfst:def:transducer.hfst-ol.transition.matches-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition.matches-fn]
    pub fn matches(&self, s: SymbolNumber) -> bool {
        self.input_symbol != NO_SYMBOL_NUMBER && self.input_symbol == s
    }
    // [spec:hfst:def:transducer.hfst-ol.transition.final-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition.final-fn]
    pub fn is_final(&self) -> bool {
        self.input_symbol == NO_SYMBOL_NUMBER
            && self.output_symbol == NO_SYMBOL_NUMBER
            && self.target_index == 1
    }
    // [spec:hfst:def:transducer.hfst-ol.transition.get-weight-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition.get-weight-fn]
    pub fn get_weight(&self) -> Weight {
        0.0
    }

    // [spec:hfst:def:transducer.hfst-ol.transition.display-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition.display-fn]
    pub fn display(&self) {
        println!(
            "input_symbol: {}, output_symbol: {}, target: {}{}",
            self.input_symbol,
            self.output_symbol,
            self.target_index,
            if self.is_final() { " (final)" } else { "" }
        );
    }

    // [spec:hfst:def:transducer.hfst-ol.transition.write-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition.write-fn]
    pub fn write(&self, os: &mut dyn std::io::Write, weighted: bool) {
        write_u16(self.input_symbol, os);
        write_u16(self.output_symbol, os);
        write_u32(self.target_index, os);
        if weighted {
            // C++ 'os << 0.0f' writes the text representation, i.e. "0".
            let _ = os.write_all(format!("{}", 0.0f32).as_bytes());
        }
    }
}

impl TableEntry for Transition {
    const SIZE: usize = 2 * 2 + 4;
    fn from_bytes(p: &[u8]) -> Self {
        // Little-endian per hfst/hfst#328 (see module docs).
        Transition {
            input_symbol: u16::from_le_bytes([p[0], p[1]]),
            output_symbol: u16::from_le_bytes([p[2], p[3]]),
            target_index: u32::from_le_bytes([p[4], p[5], p[6], p[7]]),
        }
    }
}

impl TransitionEntry for Transition {
    fn get_target(&self) -> TransitionTableIndex {
        self.get_target()
    }
    fn get_output_symbol(&self) -> SymbolNumber {
        self.get_output_symbol()
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
    fn get_weight(&self) -> Weight {
        self.get_weight()
    }
    fn write(&self, os: &mut dyn std::io::Write, weighted: bool) {
        self.write(os, weighted)
    }
    fn display(&self) {
        self.display()
    }
}

// [spec:hfst:def:transducer.hfst-ol.transition-w]
#[derive(Clone)]
pub struct TransitionW {
    pub(crate) base: Transition,
    pub(crate) transition_weight: Weight,
}

impl TransitionW {
    pub fn new_values(
        input: SymbolNumber,
        output: SymbolNumber,
        target: TransitionTableIndex,
        w: Weight,
    ) -> Self {
        TransitionW {
            base: Transition::new_values(input, output, target),
            transition_weight: w,
        }
    }

    pub fn new_final(is_final: bool, w: Weight) -> Self {
        TransitionW {
            base: Transition::new_final(is_final),
            transition_weight: w,
        }
    }

    pub fn get_target(&self) -> TransitionTableIndex {
        self.base.get_target()
    }
    pub fn get_output_symbol(&self) -> SymbolNumber {
        self.base.get_output_symbol()
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

    // [spec:hfst:def:transducer.hfst-ol.transition-w.get-weight-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-w.get-weight-fn]
    pub fn get_weight(&self) -> Weight {
        self.transition_weight
    }

    // [spec:hfst:def:transducer.hfst-ol.transition-w.display-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-w.display-fn]
    pub fn display(&self) {
        println!(
            "input_symbol: {}, output_symbol: {}, target: {}, weight: {}{}",
            self.base.input_symbol,
            self.base.output_symbol,
            self.base.target_index,
            self.transition_weight,
            if self.is_final() { " (final)" } else { "" }
        );
    }

    // [spec:hfst:def:transducer.hfst-ol.transition-w.write-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-w.write-fn]
    pub fn write(&self, os: &mut dyn std::io::Write, weighted: bool) {
        self.base.write(os, false);
        if weighted {
            // Little-endian per hfst/hfst#328 (see module docs).
            let _ = os.write_all(&self.transition_weight.to_le_bytes());
        }
    }
}

impl TableEntry for TransitionW {
    const SIZE: usize = 2 * 2 + 4 + 4;
    // [spec:hfst:def:transducer.hfst-ol.transition-w.transition-w-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-w.transition-w-fn]
    fn from_bytes(p: &[u8]) -> Self {
        // Little-endian per hfst/hfst#328 (see module docs).
        TransitionW {
            base: Transition::from_bytes(p),
            transition_weight: f32::from_le_bytes([p[8], p[9], p[10], p[11]]),
        }
    }
}

impl TransitionEntry for TransitionW {
    fn get_target(&self) -> TransitionTableIndex {
        self.get_target()
    }
    fn get_output_symbol(&self) -> SymbolNumber {
        self.get_output_symbol()
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
    fn get_weight(&self) -> Weight {
        self.get_weight()
    }
    fn write(&self, os: &mut dyn std::io::Write, weighted: bool) {
        self.write(os, weighted)
    }
    fn display(&self) {
        self.display()
    }
}
