//! Reading an optimized-lookup transducer's input side in place: its arcs,
//! finality and flag operations, and the configurations a walk holds.

use smallvec::SmallVec;

use crate::hfst_data_types::Symbol;
use crate::hfst_flag_diacritics::{FdOperation, FdState, FdValue};
use crate::transducer::{
    NO_SYMBOL_NUMBER, SymbolNumber, TRANSITION_TARGET_TABLE_START, Transducer,
    TransducerTablesInterface, TransitionTableIndex, Weight, indexes_transition_table,
};

/// One path through the generator: the state it reached, the flag values it
/// set, the surface symbols it wrote and its weight. Two paths that agree on
/// all but their weight are one configuration, at the lower weight.
// [spec:hfst:def:prediction.state]
#[derive(Clone, Debug, PartialEq)]
pub struct Configuration {
    pub(in crate::prediction) state: TransitionTableIndex,
    pub(in crate::prediction) flags: Box<[FdValue]>,
    pub(in crate::prediction) output: Vec<SymbolNumber>,
    pub(in crate::prediction) weight: Weight,
    pub(in crate::prediction) final_weight: Option<Weight>,
}

impl Configuration {
    /// The state, as the optimized-lookup tables number it.
    pub fn state(&self) -> TransitionTableIndex {
        self.state
    }

    pub fn flag_values(&self) -> &[FdValue] {
        &self.flags
    }

    /// The surface symbols written so far, flags and epsilon left out.
    pub fn output(&self) -> &[SymbolNumber] {
        &self.output
    }

    /// The path's weight, without the final weight.
    pub fn weight(&self) -> Weight {
        self.weight
    }

    pub fn final_weight(&self) -> Option<Weight> {
        self.final_weight
    }

    pub fn is_final(&self) -> bool {
        self.final_weight.is_some()
    }
}

/// The configurations one walk may still visit, and whether a bound cut it
/// short.
// [spec:hfst:req:prediction.bounded]
#[derive(Debug)]
pub(in crate::prediction) struct Budget {
    left: usize,
    /// The configuration bound ran out, so a set of configurations may be
    /// missing some.
    pub(in crate::prediction) configurations_cut: bool,
    /// A unit reached the symbol bound while it could still go on, or a
    /// relaxation walk was cut, so units may be missing.
    pub(in crate::prediction) search_cut: bool,
}

impl Budget {
    pub(in crate::prediction) fn new(configurations: usize) -> Budget {
        Budget {
            left: configurations,
            configurations_cut: false,
            search_cut: false,
        }
    }

    /// False once the bound is spent.
    pub(in crate::prediction) fn spend(&mut self) -> bool {
        if self.left == 0 {
            self.configurations_cut = true;
            return false;
        }
        self.left -= 1;
        true
    }

    pub(in crate::prediction) fn truncated(&self) -> bool {
        self.configurations_cut || self.search_cut
    }
}

pub(in crate::prediction) type Arcs = SmallVec<[TransitionTableIndex; 8]>;

/// An arc of the transition table: what it reads, where it goes, what it
/// writes and what it costs.
#[derive(Clone, Copy, Debug)]
pub(in crate::prediction) struct Edge {
    pub(in crate::prediction) input: SymbolNumber,
    /// None for epsilon and flag diacritics, which write nothing.
    pub(in crate::prediction) output: Option<SymbolNumber>,
    pub(in crate::prediction) target: TransitionTableIndex,
    pub(in crate::prediction) weight: Weight,
}

/// An optimized-lookup transducer read in place through its accessors. The
/// per-symbol flag operations are the only thing built from it, and they come
/// from the alphabet, not the tables.
// [spec:hfst:req:prediction.views]
pub(in crate::prediction) struct Machine<'a, T: TransducerTablesInterface> {
    transducer: &'a Transducer<T>,
    operations: Vec<Option<FdOperation>>,
    input_count: SymbolNumber,
    neutral: FdState<SymbolNumber>,
}

impl<'a, T: TransducerTablesInterface> Machine<'a, T> {
    pub(in crate::prediction) fn new(transducer: &'a Transducer<T>) -> Machine<'a, T> {
        let alphabet = transducer.get_alphabet();
        let operations = (0..transducer.get_symbol_table().len())
            .map(|symbol| {
                SymbolNumber::try_from(symbol)
                    .ok()
                    .and_then(|symbol| alphabet.get_operation(symbol))
                    .cloned()
            })
            .collect();
        Machine {
            transducer,
            operations,
            input_count: transducer.get_header().input_symbol_count(),
            neutral: transducer.flag_state_proto().clone(),
        }
    }

    pub(in crate::prediction) fn transducer(&self) -> &'a Transducer<T> {
        self.transducer
    }

    pub(in crate::prediction) fn input_count(&self) -> SymbolNumber {
        self.input_count
    }

    pub(in crate::prediction) fn name(&self, symbol: SymbolNumber) -> Symbol {
        self.transducer.get_alphabet().string_from_symbol(symbol)
    }

    pub(in crate::prediction) fn flag_operation(
        &self,
        symbol: SymbolNumber,
    ) -> Option<&FdOperation> {
        self.operations
            .get(symbol as usize)
            .and_then(Option::as_ref)
    }

    fn is_flag(&self, symbol: SymbolNumber) -> bool {
        self.flag_operation(symbol).is_some()
    }

    /// The input symbol spelled `name`.
    pub(in crate::prediction) fn input_symbol(&self, name: &str) -> Option<SymbolNumber> {
        let table = self.transducer.get_symbol_table();
        (1..self.input_count).find(|&symbol| table.get(symbol as usize).is_some_and(|s| s == name))
    }

    /// The operation of the flag diacritic `name`, as this transducer numbers
    /// its features and values.
    pub(in crate::prediction) fn operation(&self, name: &str) -> Option<FdOperation> {
        self.transducer
            .get_fd_table()
            .get_operation_by_string(name)
            .cloned()
    }

    pub(in crate::prediction) fn neutral_flags(&self) -> Box<[FdValue]> {
        self.neutral.get_values().clone().into_boxed_slice()
    }

    /// The flag values after `operation`, or nothing when it fails, as
    /// lookup decides it.
    // [spec:hfst:req:prediction.flags]
    pub(in crate::prediction) fn apply(
        &self,
        operation: &FdOperation,
        flags: &[FdValue],
    ) -> Option<Box<[FdValue]>> {
        let mut state = self.neutral.clone();
        state.assign_values(flags);
        state
            .apply_operation(operation)
            .then(|| state.get_values().clone().into_boxed_slice())
    }

    /// The surface the symbols spell.
    pub(in crate::prediction) fn render(&self, output: &[SymbolNumber]) -> String {
        let mut surface = String::new();
        for &symbol in output {
            surface.push_str(&self.name(symbol));
        }
        surface
    }

    pub(in crate::prediction) fn final_weight(
        &self,
        state: TransitionTableIndex,
    ) -> Option<Weight> {
        self.transducer
            .final_index(state)
            .then(|| self.transducer.final_weight(state))
    }

    pub(in crate::prediction) fn arc(&self, arc: TransitionTableIndex) -> Edge {
        let t = self.transducer;
        let output = t.get_transition_output(arc);
        let writes = output != 0 && output != NO_SYMBOL_NUMBER && !self.is_flag(output);
        Edge {
            input: t.get_transition_input(arc),
            output: writes.then_some(output),
            target: t.get_transition_target(arc),
            weight: t.get_transition_weight(arc),
        }
    }

    /// The arcs from `state` whose input is epsilon or a flag diacritic. An
    /// index-table state keeps them under input 0; a state kept in the
    /// transition table alone lists its arcs after its own entry.
    pub(in crate::prediction) fn epsilon_arcs(&self, state: TransitionTableIndex) -> Arcs {
        let t = self.transducer;
        let mut arcs = Arcs::new();
        let is_epsilon = |input: SymbolNumber| input == 0 || self.is_flag(input);
        if indexes_transition_table(state) {
            let mut arc = state - TRANSITION_TARGET_TABLE_START + 1;
            while t.get_transition_input(arc) != NO_SYMBOL_NUMBER {
                if is_epsilon(t.get_transition_input(arc)) {
                    arcs.push(arc);
                }
                arc += 1;
            }
        } else if t.get_index_input(state + 1) == 0 {
            let Some(mut arc) = t
                .get_index_target(state + 1)
                .checked_sub(TRANSITION_TARGET_TABLE_START)
            else {
                return arcs;
            };
            while is_epsilon(t.get_transition_input(arc)) {
                arcs.push(arc);
                arc += 1;
            }
        }
        arcs
    }

    /// The arcs from `state` that read `symbol`.
    pub(in crate::prediction) fn symbol_arcs(
        &self,
        state: TransitionTableIndex,
        symbol: SymbolNumber,
    ) -> Arcs {
        let t = self.transducer;
        let mut arcs = Arcs::new();
        if indexes_transition_table(state) {
            let mut arc = state - TRANSITION_TARGET_TABLE_START + 1;
            while t.get_transition_input(arc) != NO_SYMBOL_NUMBER {
                if t.get_transition_input(arc) == symbol {
                    arcs.push(arc);
                }
                arc += 1;
            }
            return arcs;
        }
        let slot = state + 1 + TransitionTableIndex::from(symbol);
        if t.get_index_input(slot) != symbol {
            return arcs;
        }
        let Some(mut arc) = t
            .get_index_target(slot)
            .checked_sub(TRANSITION_TARGET_TABLE_START)
        else {
            return arcs;
        };
        while t.get_transition_input(arc) == symbol {
            arcs.push(arc);
            arc += 1;
        }
        arcs
    }

    /// The input symbols, neither epsilon nor flags, that `state` has arcs
    /// for, in order. An index-table state is probed at every input symbol,
    /// since its slots are interleaved with other states'.
    pub(in crate::prediction) fn symbols_from(
        &self,
        state: TransitionTableIndex,
    ) -> Vec<SymbolNumber> {
        let t = self.transducer;
        let mut symbols = Vec::new();
        if indexes_transition_table(state) {
            let mut arc = state - TRANSITION_TARGET_TABLE_START + 1;
            loop {
                let input = t.get_transition_input(arc);
                if input == NO_SYMBOL_NUMBER {
                    break;
                }
                if input != 0 && !self.is_flag(input) {
                    symbols.push(input);
                }
                arc += 1;
            }
            symbols.sort_unstable();
            symbols.dedup();
            return symbols;
        }
        for symbol in 1..self.input_count {
            if !self.is_flag(symbol)
                && t.get_index_input(state + 1 + TransitionTableIndex::from(symbol)) == symbol
            {
                symbols.push(symbol);
            }
        }
        symbols
    }
}
