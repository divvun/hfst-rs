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
use std::io::Write;

use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_exception_defs::{
    HfstException, StateIndexOutOfBoundsException, StateIsNotFinalException,
};
use crate::hfst_symbol_defs::{StringPair, StringPairSet, StringSet};
use crate::hfst_tropical_transducer_transition_data::{
    HfstTropicalTransducerTransitionData, SymbolType, WeightType,
};

// Raw byte-faithful stand-in for `fprintf` to a C `FILE*`: writes the
// already-formatted `s` verbatim with `fwrite` (no NUL handling needed, so any
// bytes are safe). `%f` conversions are pre-rendered as `{:.6}` to match
// printf's default precision; the rest become ordinary `format!`.
unsafe fn c_fputs(file: *mut libc::FILE, s: &str) {
    unsafe {
        libc::fwrite(s.as_ptr() as *const libc::c_void, 1, s.len(), file);
    }
}

// C `atoi`: parse the leading integer, 0 on failure. State numbers here are
// non-negative, so only leading whitespace and ASCII digits are consumed.
fn atoi(s: &str) -> u32 {
    let s = s.trim_start();
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u32>().unwrap_or(0)
}

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

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-weight-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-weight-fn]
    pub unsafe fn write_weight_file(file: *mut libc::FILE, weight: f32) {
        unsafe {
            c_fputs(file, &format!("{:.6}", weight));
        }
    }

    // The C++ ostream `<<` float formatting (6 significant digits) differs from
    // the FILE `%f` path above, and Rust's default `{}` differs from both;
    // forgiven unless a ported test proves the exact text.
    pub fn write_weight_os(os: &mut dyn Write, weight: f32) {
        let _ = write!(os, "{}", weight);
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.xfstize-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.xfstize-fn]
    //
    // Iterates bytes (C++ `for (char pos : symbol)` over a byte string); the
    // escaped chars are ASCII and never appear as UTF-8 continuation bytes, so
    // multibyte symbols are reconstructed byte-for-byte.
    pub fn xfstize(symbol: &mut String) {
        let mut escaped_symbol: Vec<u8> = Vec::new();
        for pos in symbol.bytes() {
            if pos == b'%' {
                escaped_symbol.extend_from_slice(b"\"%\"");
            } else if pos == b'"' {
                escaped_symbol.extend_from_slice(b"%\"");
            } else if pos == b'?' {
                escaped_symbol.extend_from_slice(b"\"?\"");
            } else {
                escaped_symbol.push(pos);
            }
        }
        *symbol = String::from_utf8(escaped_symbol).unwrap();
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.xfstize-symbol-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.xfstize-symbol-fn]
    pub fn xfstize_symbol(symbol: &mut String) {
        Self::xfstize(symbol);
        crate::string_utils::replace_all(symbol, "@_EPSILON_SYMBOL_@", "0");
        crate::string_utils::replace_all(symbol, "@_UNKNOWN_SYMBOL_@", "?");
        crate::string_utils::replace_all(symbol, "@_IDENTITY_SYMBOL_@", "?");
        crate::string_utils::replace_all(symbol, "\t", "@_TAB_@");
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-xfst-state-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-xfst-state-fn]
    pub fn print_xfst_state_os(&self, os: &mut dyn Write, state: HfstState) {
        if state == Self::INITIAL_STATE {
            let _ = write!(os, "S");
        }
        if self.is_final_state(state) {
            let _ = write!(os, "f");
        }
        let _ = write!(os, "s{}", state);
    }

    pub unsafe fn print_xfst_state_file(&self, file: *mut libc::FILE, state: HfstState) {
        unsafe {
            if state == Self::INITIAL_STATE {
                c_fputs(file, "S");
            }
            if self.is_final_state(state) {
                c_fputs(file, "f");
            }
            c_fputs(file, &format!("s{}", state));
        }
    }

    pub fn print_xfst_arc_os(
        &self,
        os: &mut dyn Write,
        data: &HfstTropicalTransducerTransitionData,
    ) {
        // replace all spaces, epsilons and tabs
        if data.get_input_symbol() != data.get_output_symbol() {
            let _ = write!(os, "<");
        }
        let mut s = data.get_input_symbol();
        Self::xfstize_symbol(&mut s);
        let _ = write!(os, "{}", s);
        if data.get_input_symbol() != data.get_output_symbol()
            || data.get_output_symbol() == "@_UNKNOWN_SYMBOL_@"
        {
            s = data.get_output_symbol();
            Self::xfstize_symbol(&mut s);
            let _ = write!(os, ":{}", s);
        }
        if data.get_input_symbol() != data.get_output_symbol() {
            let _ = write!(os, ">");
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-xfst-arc-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-xfst-arc-fn]
    pub unsafe fn print_xfst_arc_file(
        &self,
        file: *mut libc::FILE,
        data: &HfstTropicalTransducerTransitionData,
    ) {
        unsafe {
            if data.get_input_symbol() != data.get_output_symbol() {
                c_fputs(file, "<");
            }
            // replace all spaces, epsilons and tabs
            let mut s = data.get_input_symbol();
            Self::xfstize_symbol(&mut s);
            c_fputs(file, &s);
            if data.get_input_symbol() != data.get_output_symbol()
                || data.get_output_symbol() == "@_UNKNOWN_SYMBOL_@"
            {
                s = data.get_output_symbol();
                Self::xfstize_symbol(&mut s);
                c_fputs(file, &format!(":{}", s));
            }
            if data.get_input_symbol() != data.get_output_symbol() {
                c_fputs(file, ">");
            }
        }
    }

    /** @brief Write the graph in xfst text format to ostream `os`. */
    pub fn write_in_xfst_format(&self, os: &mut dyn Write, write_weights: bool) {
        let _ = write_weights; // todo
        let mut source_state: u32 = 0;
        for it in self.state_vector.iter() {
            self.print_xfst_state_os(os, source_state);
            let _ = write!(os, ":\t");

            if it.is_empty() {
                let _ = write!(os, "(no arcs)");
            } else {
                for (i, tr_it) in it.iter().enumerate() {
                    if i != 0 {
                        let _ = write!(os, ", ");
                    }
                    let data = tr_it.get_transition_data();
                    self.print_xfst_arc_os(os, data);

                    let _ = write!(os, " -> ");
                    self.print_xfst_state_os(os, tr_it.get_target_state());
                }
            }
            let _ = writeln!(os, ".");
            source_state += 1;
        }
    }

    // note: unknown and identity are both '?'
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.prologize-symbol-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.prologize-symbol-fn]
    pub fn prologize_symbol(symbol: &str) -> String {
        if symbol == "0" {
            return "%0".to_string();
        }
        if symbol == "?" {
            return "%?".to_string();
        }
        if symbol == "@_EPSILON_SYMBOL_@" {
            return "0".to_string();
        }
        if symbol == "@_UNKNOWN_SYMBOL_@" {
            return "?".to_string();
        }
        if symbol == "@_IDENTITY_SYMBOL_@" {
            return "?".to_string();
        }
        // prepend a backslash to a double quote and to a backslash
        let mut retval = symbol.to_string();
        crate::string_utils::replace_all(&mut retval, "\\", "\\\\");
        crate::string_utils::replace_all(&mut retval, "\"", "\\\"");
        retval
    }

    // caveat: '?' is always unknown
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.deprologize-symbol-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.deprologize-symbol-fn]
    pub fn deprologize_symbol(symbol: &str) -> String {
        if symbol == "%0" {
            return "0".to_string();
        }
        if symbol == "%?" {
            return "?".to_string();
        }
        if symbol == "0" {
            return "@_EPSILON_SYMBOL_@".to_string();
        }
        if symbol == "?" {
            return "@_UNKNOWN_SYMBOL_@".to_string();
        }
        // remove the escaping backslash in front of a double quote and a backslash
        let mut retval = symbol.to_string();
        crate::string_utils::replace_all(&mut retval, "\\\"", "\"");
        crate::string_utils::replace_all(&mut retval, "\\\\", "\\");
        retval
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-prolog-arc-symbols-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-prolog-arc-symbols-fn]
    pub unsafe fn print_prolog_arc_symbols_file(
        file: *mut libc::FILE,
        data: &HfstTropicalTransducerTransitionData,
    ) {
        unsafe {
            let symbol = Self::prologize_symbol(&data.get_input_symbol());
            c_fputs(file, &format!("\"{}\"", symbol));

            if data.get_input_symbol() != data.get_output_symbol()
                || data.get_input_symbol() == "@_UNKNOWN_SYMBOL_@"
            {
                let symbol = Self::prologize_symbol(&data.get_output_symbol());
                c_fputs(file, &format!(":\"{}\"", symbol));
            }
        }
    }

    pub fn print_prolog_arc_symbols_os(
        os: &mut dyn Write,
        data: &HfstTropicalTransducerTransitionData,
    ) {
        let symbol = Self::prologize_symbol(&data.get_input_symbol());
        let _ = write!(os, "\"{}\"", symbol);

        if data.get_input_symbol() != data.get_output_symbol()
            || data.get_input_symbol() == "@_UNKNOWN_SYMBOL_@"
        {
            let symbol = Self::prologize_symbol(&data.get_output_symbol());
            let _ = write!(os, ":\"{}\"", symbol);
        }
    }

    /** @brief Write the graph in prolog format to FILE `file`. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-prolog-format-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-prolog-format-fn]
    pub unsafe fn write_in_prolog_format_file(
        &self,
        file: *mut libc::FILE,
        name: &str,
        write_weights: bool,
    ) {
        unsafe {
            let mut source_state: u32 = 0;
            let identifier = name;
            // Print the name.
            if name.contains(',') {
                let msg = "no commas allowed in the name of prolog networks".to_string();
                crate::HFST_THROW_MESSAGE!(HfstException, msg);
            }
            c_fputs(file, &format!("network({}).\n", identifier));

            // Print symbols that are in the alphabet but not used in arcs.
            let mut symbols_used_ = self.symbols_used();
            Self::initialize_alphabet(&mut symbols_used_); // exclude special symbols
            for it in self.alphabet.iter() {
                if !symbols_used_.contains(it) {
                    c_fputs(
                        file,
                        &format!(
                            "symbol({}, \"{}\").\n",
                            identifier,
                            Self::prologize_symbol(it)
                        ),
                    );
                }
            }

            // Print arcs.
            for it in self.state_vector.iter() {
                for tr_it in it.iter() {
                    c_fputs(
                        file,
                        &format!(
                            "arc({}, {}, {}, ",
                            identifier,
                            source_state,
                            tr_it.get_target_state()
                        ),
                    );
                    let data = tr_it.get_transition_data();
                    Self::print_prolog_arc_symbols_file(file, data);
                    if write_weights {
                        c_fputs(file, ", ");
                        Self::write_weight_file(file, data.get_weight());
                    }
                    c_fputs(file, ").\n");
                }
                source_state += 1;
            }

            // Print final states.
            for (k, v) in self.final_weight_map.iter() {
                c_fputs(file, &format!("final({}, {}", identifier, k));
                if write_weights {
                    c_fputs(file, ", ");
                    Self::write_weight_file(file, *v);
                }
                c_fputs(file, ").\n");
            }
        }
    }

    /** @brief Write the graph in prolog format to ostream `os`. */
    pub fn write_in_prolog_format_os(&self, os: &mut dyn Write, name: &str, write_weights: bool) {
        let mut source_state: u32 = 0;

        // Print the name.
        if name.contains(',') {
            let msg = "no commas allowed in the name of prolog networks".to_string();
            crate::HFST_THROW_MESSAGE!(HfstException, msg);
        }
        let _ = writeln!(os, "network({}).", name);

        // Print symbols that are in the alphabet but not used in arcs.
        let mut symbols_used_ = self.symbols_used();
        Self::initialize_alphabet(&mut symbols_used_); // exclude special symbols
        for it in self.alphabet.iter() {
            if !symbols_used_.contains(it) {
                let _ = writeln!(os, "symbol({}, \"{}\").", name, Self::prologize_symbol(it));
            }
        }

        // Print arcs.
        for it in self.state_vector.iter() {
            for tr_it in it.iter() {
                let _ = write!(
                    os,
                    "arc({}, {}, {}, ",
                    name,
                    source_state,
                    tr_it.get_target_state()
                );
                let data = tr_it.get_transition_data();
                Self::print_prolog_arc_symbols_os(os, data);
                if write_weights {
                    let _ = write!(os, ", ");
                    Self::write_weight_os(os, data.get_weight());
                }
                let _ = writeln!(os, ").");
            }
            source_state += 1;
        }

        // Print final states.
        for (k, v) in self.final_weight_map.iter() {
            let _ = write!(os, "final({}, {}", name, k);
            if write_weights {
                let _ = write!(os, ", ");
                Self::write_weight_os(os, *v);
            }
            let _ = writeln!(os, ").");
        }
    }

    // If `str` is of format ".+", change it to .+ and return true. Else false.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.strip-quotes-from-both-sides-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.strip-quotes-from-both-sides-fn]
    pub fn strip_quotes_from_both_sides(str: &mut String) -> bool {
        if str.len() < 3 {
            return false;
        }
        let bytes = str.as_bytes();
        if bytes[0] != b'"' || bytes[str.len() - 1] != b'"' {
            return false;
        }
        str.remove(0); // erase(0, 1)
        str.pop(); // erase(length-1, 1)
        true
    }

    // If `str` is of format .+)\." change it to .+ and return true. Else false.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.strip-ending-parenthesis-and-comma-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.strip-ending-parenthesis-and-comma-fn]
    pub fn strip_ending_parenthesis_and_comma(str: &mut String) -> bool {
        if str.len() < 3 {
            return false;
        }
        let bytes = str.as_bytes();
        if bytes[str.len() - 2] != b')' || bytes[str.len() - 1] != b'.' {
            return false;
        }
        str.truncate(str.len() - 2); // erase(length-2)
        true
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.parse-prolog-network-line-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.parse-prolog-network-line-fn]
    //
    // sscanf(line, "network(%s", namearr): match the literal prefix, then `%s`
    // (skip leading whitespace, read one non-whitespace token).
    pub fn parse_prolog_network_line(line: &str, graph: &mut HfstBasicTransducer) -> bool {
        // 'network(NAME).'
        let n;
        let mut namearr = String::new();
        if let Some(rest) = line.strip_prefix("network(") {
            let tok: String = rest
                .trim_start()
                .chars()
                .take_while(|c| !c.is_whitespace())
                .collect();
            if tok.is_empty() {
                n = 0;
            } else {
                namearr = tok;
                n = 1;
            }
        } else {
            n = 0;
        }
        if n != 1 {
            return false;
        }

        let mut namestr = namearr;
        // strip the ending ")." from namestr
        if !Self::strip_ending_parenthesis_and_comma(&mut namestr) {
            return false;
        }

        graph.name = namestr;
        true
    }

    // Get positions of `c` in `str`. If `esc` precedes `c`, `c` is not included.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-positions-of-unescaped-char-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-positions-of-unescaped-char-fn]
    pub fn get_positions_of_unescaped_char(str: &str, c: char, esc: char) -> Vec<u32> {
        let mut retval: Vec<u32> = Vec::new();
        let bytes = str.as_bytes();
        for i in 0..str.len() {
            if bytes[i] == c as u8 {
                if i == 0 {
                    retval.push(i as u32);
                } else if bytes[i - 1] == esc as u8 {
                    // skip escaped chars
                } else {
                    retval.push(i as u32);
                }
            }
        }
        retval
    }

    // Extract input/output symbols from prolog arc `str` of format "foo":"bar"
    // or "foo". Return whether symbols were successfully extracted.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-prolog-arc-symbols-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-prolog-arc-symbols-fn]
    pub fn get_prolog_arc_symbols(str: &str, isymbol: &mut String, osymbol: &mut String) -> bool {
        // find positions of non-escaped double quotes
        let quote_positions = Self::get_positions_of_unescaped_char(str, '"', '\\');

        // "foo"
        if quote_positions.len() == 2 {
            if quote_positions[0] != 0 || quote_positions[1] != (str.len() - 1) as u32 {
                return false; // extra characters outside quotes
            }
        }
        // "foo":"bar"
        else if quote_positions.len() == 4 {
            if quote_positions[0] != 0 || quote_positions[3] != (str.len() - 1) as u32 {
                return false; // extra characters outside quotes
            }
            if quote_positions[2] - quote_positions[1] != 2 {
                return false; // missing colon between inner quotes
            }
            if str.as_bytes()[(quote_positions[1] + 1) as usize] != b':' {
                return false; // else than colon between inner quotes
            }
        }
        // not valid prolog arc
        else {
            return false;
        }

        // "foo"
        if quote_positions.len() == 2 {
            // "foo" -> foo
            let start = (quote_positions[0] + 1) as usize;
            let len = (quote_positions[1] - quote_positions[0] - 1) as usize;
            let symbol = str[start..start + len].to_string();
            *isymbol = Self::deprologize_symbol(&symbol);
            if *isymbol == "@_UNKNOWN_SYMBOL_@" {
                // single unknown -> identity
                *isymbol = "@_IDENTITY_SYMBOL_@".to_string();
            }
            *osymbol = isymbol.clone();
        }
        // "foo":"bar"
        else {
            let s1 = (quote_positions[0] + 1) as usize;
            let l1 = (quote_positions[1] - quote_positions[0] - 1) as usize;
            let insymbol = str[s1..s1 + l1].to_string();
            let s2 = (quote_positions[2] + 1) as usize;
            let l2 = (quote_positions[3] - quote_positions[2] - 1) as usize;
            let outsymbol = str[s2..s2 + l2].to_string();
            *isymbol = Self::deprologize_symbol(&insymbol);
            *osymbol = Self::deprologize_symbol(&outsymbol);
        }

        true
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.extract-weight-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.extract-weight-fn]
    pub fn extract_weight(symbol: &mut String, weight: &mut f32) -> bool {
        let last_double_quote = symbol.rfind('"');
        let last_space = symbol.rfind(' ');

        // at least one double quote should be found
        let ldq = match last_double_quote {
            None => return false,
            Some(p) => p,
        };

        match last_space {
            None => {
                // no weight
            }
            Some(ls) => {
                if ldq > ls {
                    // no weight, last space is part of a symbol
                } else if ldq + 2 == ls && ls < symbol.len() - 1 {
                    // + 2 because of the comma
                    let buffer = &symbol[ls + 1..];
                    match buffer.parse::<f32>() {
                        Ok(w) => *weight = w,
                        Err(_) => return false, // a float could not be read
                    }
                    symbol.truncate(ls - 1); // get rid of the comma and weight
                } else {
                    return false; // not valid symbol and weight
                }
            }
        }
        true
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.parse-prolog-arc-line-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.parse-prolog-arc-line-fn]
    pub fn parse_prolog_arc_line(line: &str, graph: &mut HfstBasicTransducer) -> bool {
        // sscanf(line, "arc(%[^,], %[^,], %[^,], %[^\t\n]", ...): four scanset
        // fields separated by a literal comma plus optional whitespace.
        let mut n = 0;
        let mut namestr = String::new();
        let mut sourcestr = String::new();
        let mut targetstr = String::new();
        let mut symbolstr = String::new();
        if let Some(mut rest) = line.strip_prefix("arc(") {
            let f1: String = rest.chars().take_while(|&c| c != ',').collect();
            if !f1.is_empty() {
                namestr = f1.clone();
                n = 1;
                rest = &rest[f1.len()..];
                if let Some(r) = rest.strip_prefix(',') {
                    rest = r.trim_start();
                    let f2: String = rest.chars().take_while(|&c| c != ',').collect();
                    if !f2.is_empty() {
                        sourcestr = f2.clone();
                        n = 2;
                        rest = &rest[f2.len()..];
                        if let Some(r) = rest.strip_prefix(',') {
                            rest = r.trim_start();
                            let f3: String = rest.chars().take_while(|&c| c != ',').collect();
                            if !f3.is_empty() {
                                targetstr = f3.clone();
                                n = 3;
                                rest = &rest[f3.len()..];
                                if let Some(r) = rest.strip_prefix(',') {
                                    rest = r.trim_start();
                                    let f4: String = rest
                                        .chars()
                                        .take_while(|&c| c != '\t' && c != '\n')
                                        .collect();
                                    if !f4.is_empty() {
                                        symbolstr = f4;
                                        n = 4;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut symbol = symbolstr;
        // strip the ending ")." from symbolstr
        if !Self::strip_ending_parenthesis_and_comma(&mut symbol) {
            return false;
        }

        if n != 4 {
            return false;
        }
        if namestr != graph.name {
            return false;
        }

        let source: u32 = atoi(&sourcestr);
        let target: u32 = atoi(&targetstr);

        // handle the weight that might be included in symbol string
        let mut weight: f32 = 0.0;
        if !Self::extract_weight(&mut symbol, &mut weight) {
            return false;
        }

        let mut isymbol = String::new();
        let mut osymbol = String::new();

        if !Self::get_prolog_arc_symbols(&symbol, &mut isymbol, &mut osymbol) {
            return false;
        }

        graph.add_transition(
            source,
            &HfstBasicTransition::new_symbols(target, isymbol, osymbol, weight),
            true,
        );
        true
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.parse-prolog-final-line-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.parse-prolog-final-line-fn]
    pub fn parse_prolog_final_line(line: &str, graph: &mut HfstBasicTransducer) -> bool {
        // 'final(NAME, number).' or 'final(NAME, number, weight).'
        let mut weight: f32 = 0.0;
        let number_of_commas = line.chars().filter(|&c| c == ',').count();

        let namestr: String;
        let finalstr: String;

        if number_of_commas == 1 {
            // sscanf(line, "final(%[^,], %[^)]).", namestr, finalstr)
            let rest = match line.strip_prefix("final(") {
                Some(r) => r,
                None => return false,
            };
            let name: String = rest.chars().take_while(|&c| c != ',').collect();
            if name.is_empty() {
                return false;
            }
            let after = &rest[name.len()..];
            let r = match after.strip_prefix(',') {
                Some(x) => x.trim_start(),
                None => return false,
            };
            let fin: String = r.chars().take_while(|&c| c != ')').collect();
            if fin.is_empty() {
                return false;
            }
            namestr = name;
            finalstr = fin;
        } else if number_of_commas == 2 {
            // sscanf(line, "final(%[^,], %[^,], %[^)]).", namestr, finalstr, weightstr)
            let rest = match line.strip_prefix("final(") {
                Some(r) => r,
                None => return false,
            };
            let name: String = rest.chars().take_while(|&c| c != ',').collect();
            if name.is_empty() {
                return false;
            }
            let after = &rest[name.len()..];
            let r = match after.strip_prefix(',') {
                Some(x) => x.trim_start(),
                None => return false,
            };
            let fin: String = r.chars().take_while(|&c| c != ',').collect();
            if fin.is_empty() {
                return false;
            }
            let after2 = &r[fin.len()..];
            let r2 = match after2.strip_prefix(',') {
                Some(x) => x.trim_start(),
                None => return false,
            };
            let weightstr: String = r2.chars().take_while(|&c| c != ')').collect();
            if weightstr.is_empty() {
                return false;
            }
            match weightstr.parse::<f32>() {
                Ok(w) => weight = w,
                Err(_) => return false, // a float could not be read
            }
            namestr = name;
            finalstr = fin;
        } else {
            return false;
        }

        if namestr != graph.name {
            return false;
        }

        graph.set_final_weight(atoi(&finalstr), &weight);
        true
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.parse-prolog-symbol-line-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.parse-prolog-symbol-line-fn]
    pub fn parse_prolog_symbol_line(line: &str, graph: &mut HfstBasicTransducer) -> bool {
        // sscanf(line, "symbol(%[^,], %s", namearr, symbolarr)
        let mut n = 0;
        let mut namearr = String::new();
        let mut symbolarr = String::new();
        if let Some(rest) = line.strip_prefix("symbol(") {
            let name: String = rest.chars().take_while(|&c| c != ',').collect();
            if !name.is_empty() {
                namearr = name.clone();
                n = 1;
                let after = &rest[name.len()..];
                if let Some(after_comma) = after.strip_prefix(',') {
                    let sym: String = after_comma
                        .trim_start()
                        .chars()
                        .take_while(|c| !c.is_whitespace())
                        .collect();
                    if !sym.is_empty() {
                        symbolarr = sym;
                        n = 2;
                    }
                }
            }
        }

        if n != 2 {
            return false;
        }

        let namestr = namearr;
        let mut symbolstr = symbolarr;

        if namestr != graph.name {
            return false;
        }

        if !Self::strip_ending_parenthesis_and_comma(&mut symbolstr) {
            return false;
        }

        if !Self::strip_quotes_from_both_sides(&mut symbolstr) {
            return false;
        }

        graph.add_symbol_to_alphabet(&Self::deprologize_symbol(&symbolstr));
        true
    }

    // Erase newlines from the end of `str` and return `str`.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.strip-newlines-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.strip-newlines-fn]
    pub fn strip_newlines(str: &mut String) -> String {
        let mut i: i64 = str.len() as i64 - 1;
        while i >= 0 {
            let b = str.as_bytes()[i as usize];
            if b == b'\n' || b == b'\r' {
                str.remove(i as usize);
            } else {
                break;
            }
            i -= 1;
        }
        str.clone()
    }

    /** @brief Write the graph in xfst text format to FILE `file`. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-xfst-format-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-xfst-format-fn]
    pub unsafe fn write_in_xfst_format_file(&self, file: *mut libc::FILE, write_weights: bool) {
        unsafe {
            let _ = write_weights;
            let mut source_state: u32 = 0;
            for it in self.state_vector.iter() {
                self.print_xfst_state_file(file, source_state);
                c_fputs(file, ":\t");

                if it.is_empty() {
                    c_fputs(file, "(no arcs)");
                } else {
                    for (i, tr_it) in it.iter().enumerate() {
                        if i != 0 {
                            c_fputs(file, ", ");
                        }
                        let data = tr_it.get_transition_data();
                        self.print_xfst_arc_file(file, data);

                        c_fputs(file, " -> ");
                        self.print_xfst_state_file(file, tr_it.get_target_state());
                    }
                }
                c_fputs(file, ".\n");
                source_state += 1;
            }
        }
    }
}

impl Default for HfstBasicTransducer {
    fn default() -> Self {
        Self::new()
    }
}
