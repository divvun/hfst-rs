//! The optimized-lookup file header: table sizes and property flags.

use super::*;

// [spec:hfst:def:transducer.hfst-ol.header-flag]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HeaderFlag {
    Weighted,
    Deterministic,
    Input_deterministic,
    Minimized,
    Cyclic,
    Has_epsilon_epsilon_transitions,
    Has_input_epsilon_transitions,
    Has_input_epsilon_cycles,
    Has_unweighted_input_epsilon_cycles,
}

// [spec:hfst:def:transducer.hfst-ol.transducer-header]
#[derive(Clone)]
pub struct TransducerHeader {
    pub(crate) number_of_input_symbols: SymbolNumber,
    pub(crate) number_of_symbols: SymbolNumber,
    pub(crate) size_of_transition_index_table: TransitionTableIndex,
    pub(crate) size_of_transition_target_table: TransitionTableIndex,

    pub(crate) number_of_states: StateIdNumber,
    pub(crate) number_of_transitions: TransitionNumber,

    pub(crate) weighted: bool,
    pub(crate) deterministic: bool,
    pub(crate) input_deterministic: bool,
    pub(crate) minimized: bool,
    pub(crate) cyclic: bool,
    pub(crate) has_epsilon_epsilon_transitions: bool,
    pub(crate) has_input_epsilon_transitions: bool,
    pub(crate) has_input_epsilon_cycles: bool,
    pub(crate) has_unweighted_input_epsilon_cycles: bool,
}

impl TransducerHeader {
    // [spec:hfst:def:transducer.hfst-ol.transducer-header.header-error-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.header-error-fn]
    fn header_error() -> crate::error::Error {
        crate::err!(TransducerHasWrongType)
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-header.read-bool-property-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.read-bool-property-fn]
    fn read_bool_property(is: &mut dyn std::io::BufRead) -> crate::error::Result<bool> {
        let prop = read_u32(is)?;
        if prop == 0 {
            return Ok(false);
        }
        if prop == 1 {
            return Ok(true);
        }
        Err(Self::header_error())
    }

    /// 'TransducerHeader(bool weights)' — the header of the one-state, no-arc
    /// transducer. Every property flag below is honest for that shape and only
    /// that shape; [`Self::new_sizes`] is the constructor for real tables.
    pub fn new_weighted(weights: bool) -> Self {
        TransducerHeader {
            number_of_input_symbols: 0,
            number_of_symbols: 1, // epsilon
            size_of_transition_index_table: 1,
            size_of_transition_target_table: 0,
            number_of_states: 1,
            number_of_transitions: 0,
            weighted: weights,
            deterministic: true,
            input_deterministic: true,
            minimized: true,
            cyclic: false,
            has_epsilon_epsilon_transitions: false,
            has_input_epsilon_transitions: false,
            has_input_epsilon_cycles: false,
            has_unweighted_input_epsilon_cycles: false,
        }
    }

    /// A header for tables that already exist, told only the sizes.
    ///
    /// Every property flag is left false, meaning "nothing claimed". The C++
    /// hardcoded `deterministic` / `input_deterministic` / `minimized` true
    /// here — assertions about a graph this constructor has never seen, in the
    /// direction that makes a consumer skip work. The flags a walk can decide
    /// are filled in by [`Transducer::write`] when a file is actually emitted;
    /// in-memory queries read the graph, not the header.
    pub fn new_sizes(
        input_symbols: SymbolNumber,
        symbols: SymbolNumber,
        transition_index_table: TransitionTableIndex,
        transition_table: TransitionTableIndex,
        weights: bool,
    ) -> Self {
        TransducerHeader {
            number_of_input_symbols: input_symbols,
            number_of_symbols: symbols, // epsilon
            size_of_transition_index_table: transition_index_table,
            size_of_transition_target_table: transition_table,
            number_of_states: 0,
            number_of_transitions: 0,
            weighted: weights,
            deterministic: false,
            input_deterministic: false,
            minimized: false,
            cyclic: false,
            has_epsilon_epsilon_transitions: false,
            has_input_epsilon_transitions: false,
            has_input_epsilon_cycles: false,
            has_unweighted_input_epsilon_cycles: false,
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-header.transducer-header-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.transducer-header-fn]
    pub fn read_from(is: &mut dyn std::io::BufRead) -> crate::error::Result<Self> {
        Ok(TransducerHeader {
            number_of_input_symbols: read_u16(is)?,
            number_of_symbols: read_u16(is)?,
            size_of_transition_index_table: read_u32(is)?,
            size_of_transition_target_table: read_u32(is)?,
            number_of_states: read_u32(is)?,
            number_of_transitions: read_u32(is)?,
            weighted: Self::read_bool_property(is)?,
            deterministic: Self::read_bool_property(is)?,
            input_deterministic: Self::read_bool_property(is)?,
            minimized: Self::read_bool_property(is)?,
            cyclic: Self::read_bool_property(is)?,
            has_epsilon_epsilon_transitions: Self::read_bool_property(is)?,
            has_input_epsilon_transitions: Self::read_bool_property(is)?,
            has_input_epsilon_cycles: Self::read_bool_property(is)?,
            has_unweighted_input_epsilon_cycles: Self::read_bool_property(is)?,
        })
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-header.symbol-count-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.symbol-count-fn]
    pub fn symbol_count(&self) -> SymbolNumber {
        self.number_of_symbols
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-header.input-symbol-count-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.input-symbol-count-fn]
    pub fn input_symbol_count(&self) -> SymbolNumber {
        self.number_of_input_symbols
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-header.increment-symbol-count-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.increment-symbol-count-fn]
    pub fn increment_symbol_count(&mut self) {
        self.number_of_symbols += 1;
        self.number_of_input_symbols += 1;
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-header.index-table-size-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.index-table-size-fn]
    pub fn index_table_size(&self) -> TransitionTableIndex {
        self.size_of_transition_index_table
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-header.target-table-size-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.target-table-size-fn]
    pub fn target_table_size(&self) -> TransitionTableIndex {
        self.size_of_transition_target_table
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-header.probe-flag-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.probe-flag-fn]
    pub fn probe_flag(&self, flag: HeaderFlag) -> bool {
        match flag {
            HeaderFlag::Weighted => self.weighted,
            HeaderFlag::Deterministic => self.deterministic,
            HeaderFlag::Input_deterministic => self.input_deterministic,
            HeaderFlag::Minimized => self.minimized,
            HeaderFlag::Cyclic => self.cyclic,
            HeaderFlag::Has_epsilon_epsilon_transitions => self.has_epsilon_epsilon_transitions,
            HeaderFlag::Has_input_epsilon_transitions => self.has_input_epsilon_transitions,
            HeaderFlag::Has_input_epsilon_cycles => self.has_input_epsilon_cycles,
            HeaderFlag::Has_unweighted_input_epsilon_cycles => {
                self.has_unweighted_input_epsilon_cycles
            }
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-header.set-flag-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.set-flag-fn]
    // NB: faithful to the C++, which ignores 'value' and always sets 'true'.
    pub fn set_flag(&mut self, flag: HeaderFlag, _value: bool) {
        match flag {
            HeaderFlag::Weighted => self.weighted = true,
            HeaderFlag::Deterministic => self.deterministic = true,
            HeaderFlag::Input_deterministic => self.input_deterministic = true,
            HeaderFlag::Minimized => self.minimized = true,
            HeaderFlag::Cyclic => self.cyclic = true,
            HeaderFlag::Has_epsilon_epsilon_transitions => {
                self.has_epsilon_epsilon_transitions = true
            }
            HeaderFlag::Has_input_epsilon_transitions => self.has_input_epsilon_transitions = true,
            HeaderFlag::Has_input_epsilon_cycles => self.has_input_epsilon_cycles = true,
            HeaderFlag::Has_unweighted_input_epsilon_cycles => {
                self.has_unweighted_input_epsilon_cycles = true
            }
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-header.display-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.display-fn]
    pub fn display(&self) {
        println!("Transducer properties:");
        println!(" number_of_symbols: {}", self.number_of_symbols);
        println!(" number_of_input_symbols: {}", self.number_of_input_symbols);
        println!(
            " size_of_transition_index_table: {}",
            self.size_of_transition_index_table
        );
        println!(
            " size_of_transition_target_table: {}",
            self.size_of_transition_target_table
        );
        println!(" number_of_states: {}", self.number_of_states);
        println!(" number_of_transitions: {}", self.number_of_transitions);
        println!(" weighted: {}", self.weighted as u32);
        println!(" deterministic: {}", self.deterministic as u32);
        println!(" input_deterministic: {}", self.input_deterministic as u32);
        println!(" minimized: {}", self.minimized as u32);
        println!(" cyclic: {}", self.cyclic as u32);
        println!(
            " has_epsilon_epsilon_transitions: {}",
            self.has_epsilon_epsilon_transitions as u32
        );
        println!(
            " has_input_epsilon_transitions: {}",
            self.has_input_epsilon_transitions as u32
        );
        println!(
            " has_input_epsilon_cycles: {}",
            self.has_input_epsilon_cycles as u32
        );
        println!(
            " has_unweighted_input_epsilon_cycles: {}",
            self.has_unweighted_input_epsilon_cycles as u32
        );
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-header.write-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.write-fn]
    pub fn write(&self, os: &mut dyn std::io::Write) {
        write_u16(self.number_of_input_symbols, os);
        write_u16(self.number_of_symbols, os);
        write_u32(self.size_of_transition_index_table, os);
        write_u32(self.size_of_transition_target_table, os);
        write_u32(self.number_of_states, os);
        write_u32(self.number_of_transitions, os);
        write_bool_property(self.weighted, os);
        write_bool_property(self.deterministic, os);
        write_bool_property(self.input_deterministic, os);
        write_bool_property(self.minimized, os);
        write_bool_property(self.cyclic, os);
        write_bool_property(self.has_epsilon_epsilon_transitions, os);
        write_bool_property(self.has_input_epsilon_transitions, os);
        write_bool_property(self.has_input_epsilon_cycles, os);
        write_bool_property(self.has_unweighted_input_epsilon_cycles, os);
    }
}
