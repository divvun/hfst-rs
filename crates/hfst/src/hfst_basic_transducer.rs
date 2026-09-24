//! Port of 'libhfst/src/implementations/HfstBasicTransducer.{h,cc}' — the
//! standalone concrete graph type that is HFST's transducer interchange format.
//!
//! This is a large file ported in batches; this module currently covers the
//! type's storage, typedefs, construction, the alphabet operations, and
//! adding/removing/iterating states, transitions and final weights. Later
//! batches add substitution, harmonization, lookup, and AT&T/xfst/prolog I/O.
//!
//! Deferred constructors: 'HfstBasicTransducer(FILE*)' (needs the AT&T reader)
//! and 'HfstBasicTransducer(const HfstTransducer&)' (needs the facade +
//! ConvertTransducerFormat).

use std::collections::{BTreeMap, BTreeSet};

use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_tropical_transducer_transition_data::{
    HfstTropicalTransducerTransitionData, SymbolCoder, SymbolType, WeightType,
};

mod alphabet;
mod analysis;
mod att_format;
mod construction;
mod lookup;
mod prolog_format;
mod substitution;
mod xfst_format;

pub use analysis::{SortDistance, SummaryStats, TopologicalSort};
pub use construction::{IntersectSearch, MergeContext};
pub use lookup::{LookupAcc, LookupParams, LookupTraversalStates};
pub use substitution::substitution_data;

// C 'atoi': parse the leading integer, 0 on failure. State numbers here are
// non-negative, so only leading whitespace and ASCII digits are consumed.
fn parse_state_number(s: &str) -> u32 {
    let s = s.trim_start();
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u32>().unwrap_or(0)
}

/// \brief The number of a state in an HfstTransitionGraph.
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-state]
// [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-state]
// [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-state]
pub use crate::hfst_data_types::implementations::HfstState;

// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-replacement]
// [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-replacement]
// [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-replacement]
pub type HfstReplacement = (HfstState, Vec<(HfstSymbol, HfstSymbol)>);
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-replacements]
// [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-replacements]
// [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-replacements]
pub type HfstReplacements = Vec<HfstReplacement>;
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-replacements-map]
// [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-replacements-map]
// [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-replacements-map]
pub type HfstReplacementsMap = BTreeMap<HfstState, HfstReplacements>;

/// \brief Datatype for the states of a transition in a graph.
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transitions]
pub type HfstBasicTransitions = Vec<HfstBasicTransition>;
/// Datatype for the states of a graph and their transitions. Each index of the
/// vector is a state and the transitions on that index are its transitions.
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-states]
// [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-basic-states]
// [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-basic-states]
pub type HfstBasicStates = Vec<HfstBasicTransitions>;

// --- Class-nested typedefs ---

/// \brief Datatype for a symbol in a transition.
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-symbol]
// [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-symbol]
// [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-symbol]
pub type HfstSymbol = SymbolType;
/// \brief Datatype for a symbol pair in a transition.
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-symbol-pair]
// [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-symbol-pair]
// [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-symbol-pair]
pub type HfstSymbolPair = (HfstSymbol, HfstSymbol);
/// \brief A set of symbol pairs.
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-symbol-pair-set]
// [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-symbol-pair-set]
// [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-symbol-pair-set]
pub type HfstSymbolPairSet = BTreeSet<HfstSymbolPair>;
/// \brief A set of symbols.
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-symbol-set]
// [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-symbol-set]
// [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-symbol-set]
pub type HfstSymbolSet = BTreeSet<HfstSymbol>;
/// \brief A vector of symbol pairs.
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-symbol-pair-vector]
// [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-symbol-pair-vector]
// [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-symbol-pair-vector]
pub type HfstSymbolPairVector = Vec<HfstSymbolPair>;
/// \brief Datatype for the alphabet of a graph.
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-alphabet]
pub type HfstAlphabet = BTreeSet<HfstSymbol>;

// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.final-weight-map]
// [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.final-weight-map]
// [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.final-weight-map]
pub type FinalWeightMap = BTreeMap<HfstState, WeightType>;

// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-number]
// [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-number]
// [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-number]
pub type HfstNumber = u32;
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-number-vector]
// [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-number-vector]
// [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-number-vector]
pub type HfstNumberVector = Vec<HfstNumber>;
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-number-pair]
// [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-number-pair]
// [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-number-pair]
pub type HfstNumberPair = (HfstNumber, HfstNumber);
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-number-pair-substitutions]
// [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-number-pair-substitutions]
// [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-number-pair-substitutions]
pub type HfstNumberPairSubstitutions = BTreeMap<HfstNumberPair, HfstNumberPair>;

// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.subst-map]
// [spec:hfst:def:hfst-transition-graph.subst-map]
// [spec:hfst:sem:hfst-transition-graph.subst-map]
pub type SubstMap = BTreeMap<HfstSymbol, HfstBasicTransducer>;

// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.state-pair]
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.state-pair]
// [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.state-pair]
pub type StatePair = (HfstState, HfstState);
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.state-map]
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.state-map]
// [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.state-map]
pub type StateMap = BTreeMap<StatePair, HfstState>;

// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer]
// [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph]
// [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph]
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
    /* This graph's own symbol<->number coding (idiom5 keystone). All tropical
    symbol resolution (number->string) and interning (string->number) for this
    graph's arcs goes through it; binary ops harmonize two graphs' codings via
    'SymbolCoder::create_translator_from'. There is no longer a process-global
    coder. */
    coder: SymbolCoder,
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
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.states-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.states-fn]
    pub fn states(&self) -> Vec<HfstState> {
        let mut retval: Vec<HfstState> = vec![0; (self.get_max_state() + 1) as usize];
        for i in 0..(self.get_max_state() + 1) {
            retval[i as usize] = i;
        }
        retval
    }

    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.states-and-transitions-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.states-and-transitions-fn]
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
            coder: SymbolCoder::new(),
        }
    }

    /** @brief The assignment operator ('operator=' + 'assign'). */
    pub fn assign(&mut self, graph: &HfstBasicTransducer) -> &mut Self {
        if std::ptr::eq(self, graph) {
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

    /* Add epsilon, unknown and identity symbols to the alphabet 'alpha'. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.initialize-alphabet-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.initialize-alphabet-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.initialize-alphabet-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.initialize-alphabet-fn]
    fn initialize_alphabet(alpha: &mut HfstAlphabet) {
        alpha.insert(HfstTropicalTransducerTransitionData::get_epsilon());
        alpha.insert(HfstTropicalTransducerTransitionData::get_unknown());
        alpha.insert(HfstTropicalTransducerTransitionData::get_identity());
    }

    /* Check that all symbols in the transitions are also in the alphabet. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.check-alphabet-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.check-alphabet-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.check-alphabet-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.check-alphabet-fn]
    fn check_alphabet(&self) -> bool {
        for it in self.state_vector.iter() {
            for tr_it in it.iter() {
                let data = tr_it.get_transition_data();
                if !self.alphabet.contains(&data.get_input_symbol(&self.coder)) {
                    return false;
                }
                if !self.alphabet.contains(&data.get_output_symbol(&self.coder)) {
                    return false;
                }
            }
        }
        true
    }

    /* For internal optimization: reserve space for 'number_of_states' states. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.initialize-state-vector-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.initialize-state-vector-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.initialize-state-vector-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.initialize-state-vector-fn]
    fn initialize_state_vector(&mut self, number_of_states: u32) {
        self.state_vector.reserve(number_of_states as usize);
    }

    /* For internal optimization: reserve space for 'number_of_transitions'
    transitions for state `state_number`. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.initialize-transition-vector-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.initialize-transition-vector-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.initialize-transition-vector-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.initialize-transition-vector-fn]
    pub fn initialize_transition_vector(&mut self, state_number: u32, number_of_transitions: u32) {
        self.add_state(state_number);
        self.state_vector[state_number as usize].reserve(number_of_transitions as usize);
    }

    // --- Adding states and transitions and iterating through them ---

    pub fn add_state_new(&mut self) -> HfstState {
        let tr = HfstBasicTransitions::new();
        self.state_vector.push(tr);
        (self.state_vector.len() - 1) as HfstState
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-state-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-state-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.add-state-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.add-state-fn]
    pub fn add_state(&mut self, s: HfstState) -> HfstState {
        while self.state_vector.len() <= s as usize {
            let tr = HfstBasicTransitions::new();
            self.state_vector.push(tr);
        }
        s
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-max-state-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-max-state-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.get-max-state-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.get-max-state-fn]
    pub fn get_max_state(&self) -> HfstState {
        (self.state_vector.len() - 1) as HfstState
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-transition-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-transition-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.add-transition-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.add-transition-fn]
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
            self.alphabet.insert(data.get_input_symbol(&self.coder));
            self.alphabet.insert(data.get_output_symbol(&self.coder));
        }
        self.state_vector[s as usize].push(transition.clone());
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-transition-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-transition-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.remove-transition-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.remove-transition-fn]
    pub fn remove_transition(
        &mut self,
        s: HfstState,
        transition: &HfstBasicTransition,
        remove_symbols_from_alphabet: bool,
    ) {
        if self.state_vector.len() <= s as usize {
            return;
        }

        let tr_isym = transition.get_input_symbol(&self.coder);
        let tr_osym = transition.get_output_symbol(&self.coder);

        // find the transitions to be removed (indices, ascending)
        let mut indices_to_remove: Vec<usize> = Vec::new();
        {
            let transitions = &self.state_vector[s as usize];
            for (i, it) in transitions.iter().enumerate() {
                // weight is ignored
                if it.get_input_symbol(&self.coder) == tr_isym
                    && it.get_output_symbol(&self.coder) == tr_osym
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
            if !alpha.contains(&tr_isym) {
                self.remove_symbol_from_alphabet(&tr_isym);
            }
            if !alpha.contains(&tr_osym) {
                self.remove_symbol_from_alphabet(&tr_osym);
            }
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-final-state-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-final-state-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.is-final-state-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.is-final-state-fn]
    pub fn is_final_state(&self, s: HfstState) -> bool {
        self.final_weight_map.contains_key(&s)
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-final-weight-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-final-weight-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.get-final-weight-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.get-final-weight-fn]
    pub fn get_final_weight(&self, s: HfstState) -> crate::error::Result<WeightType> {
        if s > self.get_max_state() {
            crate::bail!(StateIndexOutOfBounds);
        }
        if let Some(w) = self.final_weight_map.get(&s) {
            return Ok(*w);
        }
        crate::bail!(StateIsNotFinal)
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.set-final-weight-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.set-final-weight-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.set-final-weight-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.set-final-weight-fn]
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
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.begin-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.begin-fn]
    //
    // The C++ 'begin()'/'end()' container iterators map onto Rust slice iterators.
    // 'end()' has no Rust analogue; iteration uses 'iter()'/'iter_mut()'.
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, HfstBasicTransitions> {
        self.state_vector.iter_mut()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, HfstBasicTransitions> {
        self.state_vector.iter()
    }

    /** @brief Get the transitions of state 's' ('operator[]'). Throws
    `StateIndexOutOfBoundsException` if the state does not exist. */
    pub fn index(&self, s: HfstState) -> crate::error::Result<&HfstBasicTransitions> {
        if s as usize >= self.state_vector.len() {
            crate::bail!(StateIndexOutOfBounds);
        }
        Ok(&self.state_vector[s as usize])
    }

    /** @brief Alternative name for 'operator[]'. */
    pub fn transitions(&self, s: HfstState) -> crate::error::Result<&HfstBasicTransitions> {
        self.index(s)
    }

    /** @brief Get mutable transitions. */
    pub fn transitions_mut(
        &mut self,
        s: HfstState,
    ) -> crate::error::Result<&mut HfstBasicTransitions> {
        if s as usize >= self.state_vector.len() {
            crate::bail!(StateIndexOutOfBounds);
        }
        Ok(&mut self.state_vector[s as usize])
    }

    /* Change state numbers s1 to s2 and vice versa. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.swap-state-numbers-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.swap-state-numbers-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.swap-state-numbers-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.swap-state-numbers-fn]
    fn swap_state_numbers(&mut self, s1: HfstState, s2: HfstState) {
        let s1_copy = self.state_vector[s1 as usize].clone();
        self.state_vector[s1 as usize] = self.state_vector[s2 as usize].clone();
        self.state_vector[s2 as usize] = s1_copy;

        // ----- Go through all states -----
        // Split the borrow: interning while iterating state_vector needs both
        // '&mut state_vector' and '&mut coder'.
        let Self {
            state_vector,
            coder,
            ..
        } = self;
        for it in state_vector.iter_mut() {
            // Go through all transitions
            for tr in it.iter_mut() {
                let target = tr.get_target_state();
                let mut new_target = target;
                if target == s1 {
                    new_target = s2;
                }
                if target == s2 {
                    new_target = s1;
                }

                if new_target != target {
                    let isym = tr.get_input_symbol(coder);
                    let osym = tr.get_output_symbol(coder);
                    let w = tr.get_weight();
                    *tr = HfstBasicTransition::new_symbols(new_target, isym, osym, w, coder);
                }
            }
        }

        // Swap final states, if needed. The C++ holds live map iterators, so a
        // later '->second' reads the entry's current value; replicated by
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
