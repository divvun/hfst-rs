//! Port of `libhfst/src/implementations/HfstBasicTransducer.{h,cc}` — the
//! standalone concrete graph type that is HFST's transducer interchange format.
//!
//! This is a large file ported in batches; this module currently covers the
//! type's storage, typedefs, construction, the alphabet operations, and
//! adding/removing/iterating states, transitions and final weights. Later
//! batches add substitution, harmonization, lookup, and AT&T/xfst/prolog I/O.
//!
//! Deferred constructors: `HfstBasicTransducer(FILE*)` (needs the AT&T reader)
//! and `HfstBasicTransducer(const HfstTransducer&)` (needs the facade +
//! ConvertTransducerFormat).

use std::collections::{BTreeMap, BTreeSet};

use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_exception_defs::{StateIndexOutOfBoundsException, StateIsNotFinalException};
use crate::hfst_symbol_defs::{StringPair, StringPairSet, StringSet};
use crate::hfst_tropical_transducer_transition_data::{
    HfstTropicalTransducerTransitionData, SymbolType, WeightType,
};

/// \brief The number of a state in an HfstTransitionGraph.
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-state]
pub use crate::hfst_data_types::implementations::HfstState;

// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-replacement]
pub type HfstReplacement = (HfstState, Vec<(String, String)>);
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-replacements]
pub type HfstReplacements = Vec<HfstReplacement>;
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-replacements-map]
pub type HfstReplacementsMap = BTreeMap<HfstState, HfstReplacements>;

/// \brief Datatype for the states of a transition in a graph.
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transitions]
pub type HfstBasicTransitions = Vec<HfstBasicTransition>;
/// Datatype for the states of a graph and their transitions. Each index of the
/// vector is a state and the transitions on that index are its transitions.
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-states]
pub type HfstBasicStates = Vec<HfstBasicTransitions>;

// --- Class-nested typedefs ---

/// \brief Datatype for a symbol in a transition.
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-symbol]
pub type HfstSymbol = SymbolType;
/// \brief Datatype for a symbol pair in a transition.
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-symbol-pair]
pub type HfstSymbolPair = (HfstSymbol, HfstSymbol);
/// \brief A set of symbol pairs.
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-symbol-pair-set]
pub type HfstSymbolPairSet = BTreeSet<HfstSymbolPair>;
/// \brief A set of symbols.
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-symbol-set]
pub type HfstSymbolSet = BTreeSet<HfstSymbol>;
/// \brief A vector of symbol pairs.
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-symbol-pair-vector]
pub type HfstSymbolPairVector = Vec<HfstSymbolPair>;
/// \brief Datatype for the alphabet of a graph.
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-alphabet]
pub type HfstAlphabet = BTreeSet<HfstSymbol>;

// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.final-weight-map]
pub type FinalWeightMap = BTreeMap<HfstState, WeightType>;

// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-number]
pub type HfstNumber = u32;
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-number-vector]
pub type HfstNumberVector = Vec<HfstNumber>;
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-number-pair]
pub type HfstNumberPair = (HfstNumber, HfstNumber);
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-number-pair-substitutions]
pub type HfstNumberPairSubstitutions = BTreeMap<HfstNumberPair, HfstNumberPair>;

// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer]
#[derive(Clone, Debug)]
pub struct HfstBasicTransducer {
    /* States of the graph and their transitions. */
    pub state_vector: HfstBasicStates,
    /* The final states and their weights in the graph. */
    final_weight_map: FinalWeightMap,
    /* The alphabet of the graph. */
    alphabet: HfstAlphabet,
    /** @brief The name of the graph. */
    pub name: String,
}

// Ported in batches; several protected helpers (check_alphabet,
// swap_state_numbers, the initialize_* reservers, …) are only called by methods
// in not-yet-ported batches (AT&T I/O, substitution). Allowed until complete.
#[allow(dead_code)]
impl HfstBasicTransducer {
    /* The initial state number. */
    const INITIAL_STATE: HfstState = 0;

    // --- states ---

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.states-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.states-fn]
    pub fn states(&self) -> Vec<HfstState> {
        let mut retval: Vec<HfstState> = vec![0; (self.get_max_state() + 1) as usize];
        for i in 0..(self.get_max_state() + 1) {
            retval[i as usize] = i;
        }
        retval
    }

    pub fn states_and_transitions(&self) -> &HfstBasicStates {
        &self.state_vector
    }

    pub fn states_and_transitions_mut(&mut self) -> &mut HfstBasicStates {
        &mut self.state_vector
    }

    // --- Construction, assignment, copying ---

    pub fn new() -> Self {
        let mut alphabet = HfstAlphabet::new();
        Self::initialize_alphabet(&mut alphabet);
        let mut state_vector = HfstBasicStates::new();
        let tr = HfstBasicTransitions::new();
        state_vector.push(tr);
        HfstBasicTransducer {
            state_vector,
            final_weight_map: FinalWeightMap::new(),
            alphabet,
            name: String::new(),
        }
    }

    /** @brief The assignment operator (`operator=` + `assign`). */
    pub fn assign(&mut self, graph: &HfstBasicTransducer) -> &mut Self {
        if self as *const HfstBasicTransducer == graph as *const HfstBasicTransducer {
            return self;
        }
        self.state_vector = graph.state_vector.clone();
        self.final_weight_map = graph.final_weight_map.clone();
        self.alphabet = graph.alphabet.clone();
        assert!(!self.alphabet.contains(""));
        self.name = graph.name.clone();
        self
    }

    // --- Initialization, optimization and debugging ---

    /* Add epsilon, unknown and identity symbols to the alphabet `alpha`. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.initialize-alphabet-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.initialize-alphabet-fn]
    fn initialize_alphabet(alpha: &mut HfstAlphabet) {
        alpha.insert(HfstTropicalTransducerTransitionData::get_epsilon());
        alpha.insert(HfstTropicalTransducerTransitionData::get_unknown());
        alpha.insert(HfstTropicalTransducerTransitionData::get_identity());
    }

    /* Check that all symbols in the transitions are also in the alphabet. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.check-alphabet-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.check-alphabet-fn]
    fn check_alphabet(&self) -> bool {
        for it in self.state_vector.iter() {
            for tr_it in it.iter() {
                let data = tr_it.get_transition_data();
                if !self.alphabet.contains(&data.get_input_symbol()) {
                    return false;
                }
                if !self.alphabet.contains(&data.get_output_symbol()) {
                    return false;
                }
            }
        }
        true
    }

    /* Print the alphabet of the graph to the standard error stream. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-alphabet-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-alphabet-fn]
    pub fn print_alphabet(&self) {
        let first = self.alphabet.iter().next();
        for it in self.alphabet.iter() {
            if Some(it) != first {
                eprint!(", ");
            }
            eprint!("{}", it);
        }
        eprintln!();
    }

    /* Get the number of the `symbol`. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-symbol-number-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-symbol-number-fn]
    fn get_symbol_number(&self, symbol: &HfstSymbol) -> u32 {
        HfstTropicalTransducerTransitionData::get_number(symbol)
    }

    /* For internal optimization: reserve space for `number_of_states` states. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.initialize-state-vector-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.initialize-state-vector-fn]
    fn initialize_state_vector(&mut self, number_of_states: u32) {
        self.state_vector.reserve(number_of_states as usize);
    }

    /* For internal optimization: reserve space for `number_of_transitions`
    transitions for state `state_number`. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.initialize-transition-vector-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.initialize-transition-vector-fn]
    fn initialize_transition_vector(&mut self, state_number: u32, number_of_transitions: u32) {
        self.add_state(state_number);
        self.state_vector[state_number as usize].reserve(number_of_transitions as usize);
    }

    // --- The alphabet ---

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-symbol-to-alphabet-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-symbol-to-alphabet-fn]
    pub fn add_symbol_to_alphabet(&mut self, symbol: &HfstSymbol) {
        self.alphabet.insert(symbol.clone());
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-symbol-from-alphabet-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-symbol-from-alphabet-fn]
    pub fn remove_symbol_from_alphabet(&mut self, symbol: &HfstSymbol) {
        self.alphabet.remove(symbol);
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-symbols-from-alphabet-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-symbols-from-alphabet-fn]
    pub fn remove_symbols_from_alphabet(&mut self, symbols: &HfstSymbolSet) {
        for symbol in symbols.iter() {
            self.alphabet.remove(symbol);
        }
    }

    pub fn add_symbols_to_alphabet_set(&mut self, symbols: &HfstSymbolSet) {
        for symbol in symbols.iter() {
            self.alphabet.insert(symbol.clone());
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-symbols-to-alphabet-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-symbols-to-alphabet-fn]
    pub fn add_symbols_to_alphabet_pair_set(&mut self, symbols: &HfstSymbolPairSet) {
        for symbol in symbols.iter() {
            self.alphabet.insert(symbol.0.clone());
            self.alphabet.insert(symbol.1.clone());
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.prune-alphabet-after-substitution-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.prune-alphabet-after-substitution-fn]
    pub fn prune_alphabet_after_substitution(&mut self, symbols: &BTreeSet<u32>) {
        if symbols.len() == 0 {
            return;
        }

        let mut symbols_found: Vec<bool> = Vec::new();
        symbols_found.resize(
            (HfstTropicalTransducerTransitionData::get_max_number() + 1) as usize,
            false,
        );

        // Go through all transitions
        for it in self.state_vector.iter() {
            for tr_it in it.iter() {
                let data = tr_it.get_transition_data();
                symbols_found[data.get_input_number() as usize] = true;
                symbols_found[data.get_output_number() as usize] = true;
            }
        }

        // Remove symbols in `symbols` from the alphabet if they did not occur.
        for &symbol in symbols.iter() {
            if !symbols_found[symbol as usize] {
                self.alphabet
                    .remove(&HfstTropicalTransducerTransitionData::get_symbol(symbol));
            }
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.symbols-used-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.symbols-used-fn]
    pub fn symbols_used(&self) -> HfstAlphabet {
        let mut retval = HfstAlphabet::new();
        for it in self.state_vector.iter() {
            for tr_it in it.iter() {
                let data = tr_it.get_transition_data();
                retval.insert(data.get_input_symbol());
                retval.insert(data.get_output_symbol());
            }
        }
        retval
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.prune-alphabet-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.prune-alphabet-fn]
    pub fn prune_alphabet(&mut self, force: bool) {
        // Which symbols occur in the graph
        let mut symbols_found = self.symbols_used();

        // Whether unknown or identity symbols are used
        let unknowns_or_identities_used = symbols_found.contains("@_UNKNOWN_SYMBOL_@")
            || symbols_found.contains("@_IDENTITY_SYMBOL_@");

        // We cannot prune if unknowns or identities are used in its transitions.
        if !force && unknowns_or_identities_used {
            return;
        }

        // Special symbols are always known
        symbols_found.insert("@_EPSILON_SYMBOL_@".to_string());
        symbols_found.insert("@_UNKNOWN_SYMBOL_@".to_string());
        symbols_found.insert("@_IDENTITY_SYMBOL_@".to_string());

        // Which symbols in the graph's alphabet did not occur in the graph
        let mut symbols_not_found = HfstAlphabet::new();

        for it in self.alphabet.iter() {
            if !symbols_found.contains(it) {
                symbols_not_found.insert(it.clone());
            }
        }

        // Remove the symbols that did not occur from the alphabet
        for it in symbols_not_found.iter() {
            self.alphabet.remove(it);
        }
    }

    pub fn get_alphabet(&self) -> &HfstAlphabet {
        &self.alphabet
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-transition-pairs-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-transition-pairs-fn]
    pub fn get_transition_pairs(&self) -> StringPairSet {
        let mut retval = StringPairSet::new();
        for it in self.state_vector.iter() {
            for tr_it in it.iter() {
                let data = tr_it.get_transition_data();
                retval.insert(StringPair::from((
                    data.get_input_symbol(),
                    data.get_output_symbol(),
                )));
            }
        }
        retval
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-input-symbols-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-input-symbols-fn]
    pub fn get_input_symbols(&self) -> StringSet {
        let mut retval = StringSet::new();
        for it in self.state_vector.iter() {
            for tr_it in it.iter() {
                let data = tr_it.get_transition_data();
                retval.insert(data.get_input_symbol());
            }
        }
        retval
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-output-symbols-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-output-symbols-fn]
    pub fn get_output_symbols(&self) -> StringSet {
        let mut retval = StringSet::new();
        for it in self.state_vector.iter() {
            for tr_it in it.iter() {
                let data = tr_it.get_transition_data();
                retval.insert(data.get_output_symbol());
            }
        }
        retval
    }

    // --- Adding states and transitions and iterating through them ---

    pub fn add_state_new(&mut self) -> HfstState {
        let tr = HfstBasicTransitions::new();
        self.state_vector.push(tr);
        (self.state_vector.len() - 1) as HfstState
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-state-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-state-fn]
    pub fn add_state(&mut self, s: HfstState) -> HfstState {
        while self.state_vector.len() <= s as usize {
            let tr = HfstBasicTransitions::new();
            self.state_vector.push(tr);
        }
        s
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-max-state-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-max-state-fn]
    pub fn get_max_state(&self) -> HfstState {
        (self.state_vector.len() - 1) as HfstState
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-transition-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-transition-fn]
    pub fn add_transition(
        &mut self,
        s: HfstState,
        transition: &HfstBasicTransition,
        add_symbols_to_alphabet: bool,
    ) {
        let data = transition.get_transition_data().clone();

        self.add_state(s);
        self.add_state(transition.get_target_state());
        if add_symbols_to_alphabet {
            self.alphabet.insert(data.get_input_symbol());
            self.alphabet.insert(data.get_output_symbol());
        }
        self.state_vector[s as usize].push(transition.clone());
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-transition-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-transition-fn]
    pub fn remove_transition(
        &mut self,
        s: HfstState,
        transition: &HfstBasicTransition,
        remove_symbols_from_alphabet: bool,
    ) {
        if !(self.state_vector.len() > s as usize) {
            return;
        }

        // find the transitions to be removed (indices, ascending)
        let mut indices_to_remove: Vec<usize> = Vec::new();
        {
            let transitions = &self.state_vector[s as usize];
            for (i, it) in transitions.iter().enumerate() {
                // weight is ignored
                if it.get_input_symbol() == transition.get_input_symbol()
                    && it.get_output_symbol() == transition.get_output_symbol()
                    && it.get_target_state() == transition.get_target_state()
                {
                    indices_to_remove.push(i);
                }
            }
        }
        // remove in reverse order so that earlier indices stay valid
        for &i in indices_to_remove.iter().rev() {
            self.state_vector[s as usize].remove(i);
        }

        if remove_symbols_from_alphabet {
            let alpha = self.symbols_used();
            if !alpha.contains(&transition.get_input_symbol()) {
                self.remove_symbol_from_alphabet(&transition.get_input_symbol());
            }
            if !alpha.contains(&transition.get_output_symbol()) {
                self.remove_symbol_from_alphabet(&transition.get_output_symbol());
            }
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-final-state-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-final-state-fn]
    pub fn is_final_state(&self, s: HfstState) -> bool {
        self.final_weight_map.contains_key(&s)
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-final-weight-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-final-weight-fn]
    pub fn get_final_weight(&self, s: HfstState) -> WeightType {
        if s > self.get_max_state() {
            crate::HFST_THROW!(StateIndexOutOfBoundsException);
        }
        if let Some(w) = self.final_weight_map.get(&s) {
            return *w;
        }
        crate::HFST_THROW!(StateIsNotFinalException)
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.set-final-weight-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.set-final-weight-fn]
    pub fn set_final_weight(&mut self, s: HfstState, weight: &WeightType) {
        self.add_state(s);
        self.final_weight_map.insert(s, *weight);
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-final-weight-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-final-weight-fn]
    pub fn remove_final_weight(&mut self, s: HfstState) {
        self.final_weight_map.remove(&s);
    }

    /** @brief Sort the transitions of this transducer by input/output symbol. */
    pub fn sort_arcs(&mut self) -> &mut Self {
        for transitions in self.state_vector.iter_mut() {
            transitions.sort();
        }
        self
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.begin-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.begin-fn]
    //
    // The C++ `begin()`/`end()` container iterators map onto Rust slice iterators.
    // `end()` has no Rust analogue; iteration uses `iter()`/`iter_mut()`.
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, HfstBasicTransitions> {
        self.state_vector.iter_mut()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, HfstBasicTransitions> {
        self.state_vector.iter()
    }

    /** @brief Get the transitions of state `s` (`operator[]`). Throws
    `StateIndexOutOfBoundsException` if the state does not exist. */
    pub fn index(&self, s: HfstState) -> &HfstBasicTransitions {
        if s as usize >= self.state_vector.len() {
            crate::HFST_THROW!(StateIndexOutOfBoundsException);
        }
        &self.state_vector[s as usize]
    }

    /** @brief Alternative name for `operator[]`. */
    pub fn transitions(&self, s: HfstState) -> &HfstBasicTransitions {
        self.index(s)
    }

    /** @brief Get mutable transitions. */
    pub fn transitions_mut(&mut self, s: HfstState) -> &mut HfstBasicTransitions {
        if s as usize >= self.state_vector.len() {
            crate::HFST_THROW!(StateIndexOutOfBoundsException);
        }
        &mut self.state_vector[s as usize]
    }

    // --- Reading and writing in AT&T format (helpers) ---

    /* Change state numbers s1 to s2 and vice versa. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.swap-state-numbers-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.swap-state-numbers-fn]
    fn swap_state_numbers(&mut self, s1: HfstState, s2: HfstState) {
        let s1_copy = self.state_vector[s1 as usize].clone();
        self.state_vector[s1 as usize] = self.state_vector[s2 as usize].clone();
        self.state_vector[s2 as usize] = s1_copy;

        // ----- Go through all states -----
        for it in self.state_vector.iter_mut() {
            // Go through all transitions
            for i in 0..it.len() {
                let target = it[i].get_target_state();
                let mut new_target = target;
                if target == s1 {
                    new_target = s2;
                }
                if target == s2 {
                    new_target = s1;
                }

                if new_target != target {
                    let isym = it[i].get_input_symbol();
                    let osym = it[i].get_output_symbol();
                    let w = it[i].get_weight();
                    let tr = HfstBasicTransition::new_symbols(new_target, isym, osym, w);
                    it[i] = tr;
                }
            }
        }

        // Swap final states, if needed. The C++ holds live map iterators, so a
        // later `->second` reads the entry's current value; replicated by
        // capturing presence up front and re-reading the map value each time.
        let s1_present = self.final_weight_map.contains_key(&s1);
        let s2_present = self.final_weight_map.contains_key(&s2);

        if s1_present && s2_present {
            let s1_weight = self.final_weight_map[&s1];
            let s2_val = self.final_weight_map[&s2];
            self.final_weight_map.insert(s1, s2_val);
            self.final_weight_map.insert(s2, s1_weight);
        }
        if s1_present {
            let w = self.final_weight_map[&s1];
            self.final_weight_map.remove(&s1);
            self.final_weight_map.insert(s2, w);
        }
        if s2_present {
            let w = self.final_weight_map[&s2];
            self.final_weight_map.remove(&s2);
            self.final_weight_map.insert(s1, w);
        }
    }
}

impl Default for HfstBasicTransducer {
    fn default() -> Self {
        Self::new()
    }
}
