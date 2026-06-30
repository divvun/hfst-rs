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
use std::io::{BufRead, Write};

use crate::harmonize_unknown_and_identity_symbols::HarmonizeUnknownAndIdentitySymbols;
use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_data_types::{
    HfstOneLevelPath, HfstTwoLevelPath, HfstTwoLevelPaths, StringVector, double_to_float,
    size_t_to_int, size_t_to_uint,
};
use crate::hfst_epsilon_handler::HfstEpsilonHandler;
use crate::hfst_exception_defs::{
    EmptyStringException, EndOfStreamException, HfstException, NotValidAttFormatException,
    NotValidPrologFormatException, StateIndexOutOfBoundsException, StateIsNotFinalException,
    TransducersAreNotAutomataException,
};
use crate::hfst_flag_diacritics::FdOperation;
use crate::hfst_lookup_flag_diacritics::FlagDiacriticTable;
use crate::hfst_symbol_defs::{
    HfstSymbolPairSubstitutions, HfstSymbolSubstitutions, StringPair, StringPairSet,
    StringPairVector, StringSet, is_epsilon, is_identity, is_unknown,
};
use crate::hfst_tropical_transducer_transition_data::{
    HfstTropicalTransducerTransitionData, SymbolCoder, SymbolType, WeightType,
};
use crate::string_utils::replace_all;

// Raw byte-faithful stand-in for 'fprintf' to an output stream: writes the
// already-formatted 's' verbatim (no NUL handling needed, so any bytes are
// safe). '%f' conversions are pre-rendered as '{:.6}' to match printf's default
// precision; the rest become ordinary 'format!'. Write errors are ignored, as
// the original 'fwrite'-to-FILE path did.
fn w_fputs(w: &mut dyn Write, s: &str) {
    let _ = w.write_all(s.as_bytes());
}

// C 'atoi': parse the leading integer, 0 on failure. State numbers here are
// non-negative, so only leading whitespace and ASCII digits are consumed.
fn atoi(s: &str) -> u32 {
    let s = s.trim_start();
    let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u32>().unwrap_or(0)
}

// C 'atof': parse the leading float, 0.0 on failure. The inputs here are clean
// whitespace-delimited tokens, so a plain parse suffices.
fn atof(s: &str) -> f64 {
    s.trim_start().parse::<f64>().unwrap_or(0.0)
}

// 'fgets(buf, 255, file)': read up to 254 bytes or through a newline; None at
// EOF (when no bytes at all could be read). A trailing newline (if any) is kept,
// matching fgets. Faithful BufRead stand-in for the original C 'c_fgets'.
fn bufread_fgets(is: &mut dyn BufRead) -> Option<String> {
    let mut line: Vec<u8> = Vec::new();
    loop {
        let mut byte = [0u8; 1];
        match is.read(&mut byte) {
            Ok(0) => break, // EOF
            Ok(_) => {
                line.push(byte[0]);
                if byte[0] == b'\n' {
                    break;
                }
                // fgets reads at most 'size - 1' (254) bytes.
                if line.len() >= 254 {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    if line.is_empty() {
        None
    } else {
        Some(String::from_utf8_lossy(&line).into_owned())
    }
}

// Approximation of 'std::istream::eof()' for a fresh reader: no bytes remain.
fn is_eof(is: &mut dyn BufRead) -> bool {
    match is.fill_buf() {
        Ok(b) => b.is_empty(),
        Err(_) => true,
    }
}

// 'get_stripped_line' wrapped in the C++ try/catch: returns None when it would
// throw 'EndOfStreamException'. The panic hook is silenced so the caught
// exception (a 'panic_any') does not print.
fn catch_get_stripped_line(is: &mut dyn BufRead, linecount: &mut u32) -> Option<String> {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        HfstBasicTransducer::get_stripped_line(is, linecount)
    }));
    std::panic::set_hook(prev);
    match r {
        Ok(v) => Some(v),
        Err(e) => {
            if e.downcast_ref::<EndOfStreamException>().is_some() {
                None
            } else {
                std::panic::resume_unwind(e)
            }
        }
    }
}

/// \brief The number of a state in an HfstTransitionGraph.
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-state]
// [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-state]
// [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-state]
pub use crate::hfst_data_types::implementations::HfstState;

// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-replacement]
// [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-replacement]
// [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-replacement]
pub type HfstReplacement = (HfstState, Vec<(String, String)>);
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

/* A topological sort. */
// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.topological-sort]
// [spec:hfst:def:hfst-transition-graph.topological-sort]
// [spec:hfst:sem:hfst-transition-graph.topological-sort]
pub struct TopologicalSort {
    pub distance_of_state: Vec<i32>,
    pub states_at_distance: Vec<BTreeSet<HfstState>>,
}

impl TopologicalSort {
    pub fn new() -> Self {
        TopologicalSort {
            distance_of_state: Vec::new(),
            states_at_distance: Vec::new(),
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.topological-sort.set-biggest-state-number-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.topological-sort.set-biggest-state-number-fn]
    // [spec:hfst:def:hfst-transition-graph.topological-sort.set-biggest-state-number-fn]
    // [spec:hfst:sem:hfst-transition-graph.topological-sort.set-biggest-state-number-fn]
    pub fn set_biggest_state_number(&mut self, biggest_state_number: u32) {
        self.distance_of_state = vec![-1; (biggest_state_number + 1) as usize];
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.topological-sort.set-state-at-distance-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.topological-sort.set-state-at-distance-fn]
    // [spec:hfst:def:hfst-transition-graph.topological-sort.set-state-at-distance-fn]
    // [spec:hfst:sem:hfst-transition-graph.topological-sort.set-state-at-distance-fn]
    pub fn set_state_at_distance(&mut self, state: HfstState, distance: u32, overwrite: bool) {
        if state as usize > self.distance_of_state.len() - 1 {
            eprintln!(
                "ERROR in TopologicalSort::set_state_at_distance: first argument ({}) is out of range (should be < {})",
                state,
                self.distance_of_state.len()
            );
        }
        while (distance + 1) as usize > self.states_at_distance.len() {
            self.states_at_distance.push(BTreeSet::new());
        }
        let previous_distance = self.distance_of_state[state as usize];
        if previous_distance != -1 && previous_distance != distance as i32 && overwrite {
            self.states_at_distance[previous_distance as usize].remove(&state);
        }
        self.states_at_distance[distance as usize].insert(state);
        self.distance_of_state[state as usize] = distance as i32;
    }

    /* The states that have a maximum distance of 'distance'. */
    pub fn get_states_at_distance(&mut self, distance: u32) -> &BTreeSet<HfstState> {
        while distance as usize > self.states_at_distance.len() - 1 {
            self.states_at_distance.push(BTreeSet::new());
        }
        &self.states_at_distance[distance as usize]
    }
}

impl Default for TopologicalSort {
    fn default() -> Self {
        Self::new()
    }
}

// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.sort-distance]
// [spec:hfst:def:hfst-transition-graph.sort-distance]
// [spec:hfst:sem:hfst-transition-graph.sort-distance]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortDistance {
    MaximumDistance,
    MinimumDistance,
}

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

// Where a substituting copy of a graph is inserted (origin/target state, weight,
// and a raw pointer to the substituting graph — the C++ stores a
// 'const_cast' 'HfstBasicTransducer*').
pub struct substitution_data {
    pub origin_state: HfstState,
    pub target_state: HfstState,
    pub weight: WeightType,
    pub substituting_graph: *const HfstBasicTransducer,
}

impl substitution_data {
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.substitution-data.substitution-data-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.substitution-data.substitution-data-fn]
    // [spec:hfst:def:hfst-transition-graph.substitution-data.substitution-data-fn]
    // [spec:hfst:sem:hfst-transition-graph.substitution-data.substitution-data-fn]
    pub fn new(
        origin: HfstState,
        target: HfstState,
        weight: WeightType,
        substituting: *const HfstBasicTransducer,
    ) -> Self {
        substitution_data {
            origin_state: origin,
            target_state: target,
            weight,
            substituting_graph: substituting,
        }
    }
}

/// Single-pass arc-traversal statistics of a graph, lifted from hfst-summarize.
/// Holds only the figures the traversal computes; the tool keeps its type-derived
/// flags (is_mutable/weighted), the header alphabet, the derived averages, and all
/// output formatting. Field names match the locals the tool destructures into.
#[derive(Clone, Debug)]
pub struct SummaryStats {
    pub states: usize,
    pub final_states: usize,
    pub arcs: usize,
    pub io_epsilons: usize,
    pub input_epsilons: usize,
    pub output_epsilons: usize,
    pub densest_arcs: usize,
    pub sparsest_arcs: usize,
    pub uniq_input_arcs: usize,
    pub uniq_output_arcs: usize,
    pub most_ambiguous_input: (String, u32),
    pub most_ambiguous_output: (String, u32),
    pub found_alphabet: BTreeSet<String>,
    pub symbol_pairs: BTreeMap<(String, String), u32>,
    pub acceptor: bool,
    pub input_deterministic: bool,
    pub output_deterministic: bool,
    pub cyclic: bool,
    pub cyclic_at_initial_state: bool,
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

    /* Print the alphabet of the graph to the standard error stream. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-alphabet-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-alphabet-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.print-alphabet-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.print-alphabet-fn]
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

    /* Get the number of the 'symbol'. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-symbol-number-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-symbol-number-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.get-symbol-number-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.get-symbol-number-fn]
    pub fn get_symbol_number(&mut self, symbol: &HfstSymbol) -> u32 {
        self.coder.get_number(symbol)
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

    // --- The alphabet ---

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-symbol-to-alphabet-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-symbol-to-alphabet-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.add-symbol-to-alphabet-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.add-symbol-to-alphabet-fn]
    pub fn add_symbol_to_alphabet(&mut self, symbol: &HfstSymbol) {
        self.alphabet.insert(symbol.clone());
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-symbol-from-alphabet-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-symbol-from-alphabet-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.remove-symbol-from-alphabet-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.remove-symbol-from-alphabet-fn]
    pub fn remove_symbol_from_alphabet(&mut self, symbol: &HfstSymbol) {
        self.alphabet.remove(symbol);
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-symbols-from-alphabet-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-symbols-from-alphabet-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.remove-symbols-from-alphabet-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.remove-symbols-from-alphabet-fn]
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
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.add-symbols-to-alphabet-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.add-symbols-to-alphabet-fn]
    pub fn add_symbols_to_alphabet_pair_set(&mut self, symbols: &HfstSymbolPairSet) {
        for symbol in symbols.iter() {
            self.alphabet.insert(symbol.0.clone());
            self.alphabet.insert(symbol.1.clone());
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.prune-alphabet-after-substitution-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.prune-alphabet-after-substitution-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.prune-alphabet-after-substitution-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.prune-alphabet-after-substitution-fn]
    pub fn prune_alphabet_after_substitution(&mut self, symbols: &BTreeSet<u32>) {
        if symbols.len() == 0 {
            return;
        }

        let mut symbols_found: Vec<bool> = Vec::new();
        symbols_found.resize((self.coder.get_max_number() + 1) as usize, false);

        // Go through all transitions
        for it in self.state_vector.iter() {
            for tr_it in it.iter() {
                let data = tr_it.get_transition_data();
                symbols_found[data.get_input_number() as usize] = true;
                symbols_found[data.get_output_number() as usize] = true;
            }
        }

        // Remove symbols in 'symbols' from the alphabet if they did not occur.
        for &symbol in symbols.iter() {
            if !symbols_found[symbol as usize] {
                self.alphabet.remove(&self.coder.get_symbol(symbol));
            }
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.symbols-used-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.symbols-used-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.symbols-used-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.symbols-used-fn]
    pub fn symbols_used(&self) -> HfstAlphabet {
        let mut retval = HfstAlphabet::new();
        for it in self.state_vector.iter() {
            for tr_it in it.iter() {
                let data = tr_it.get_transition_data();
                retval.insert(data.get_input_symbol(&self.coder));
                retval.insert(data.get_output_symbol(&self.coder));
            }
        }
        retval
    }

    /// The set of input symbols occurring on this graph's transitions — the
    /// input-only sibling of [`Self::symbols_used`]. Used by alphabet-compatibility
    /// diagnostics (e.g. hfst-compose-intersect checks whether a rule's input
    /// alphabet covers a lexicon's output symbols).
    pub fn input_symbols_used(&self) -> HfstAlphabet {
        let mut retval = HfstAlphabet::new();
        for it in self.state_vector.iter() {
            for tr_it in it.iter() {
                retval.insert(tr_it.get_transition_data().get_input_symbol(&self.coder));
            }
        }
        retval
    }

    /// From state `s`, follow the transition whose label pair is
    /// `(isymbol, osymbol)`. When no exact transition matches but the state has an
    /// `@_IDENTITY_SYMBOL_@:@_IDENTITY_SYMBOL_@` identity transition AND the
    /// queried pair is an unknown identity (`isymbol == osymbol` and not in
    /// `known_symbols`), follow the identity transition instead. Returns the
    /// target state, or `None` when no transition applies. This is the pair-path
    /// recogniser step lifted from hfst-pair-test.
    pub fn pair_target_state(
        &self,
        s: HfstState,
        isymbol: &str,
        osymbol: &str,
        known_symbols: &BTreeSet<String>,
    ) -> Option<HfstState> {
        let mut identity_target: Option<HfstState> = None;
        for it in self.transitions(s).iter() {
            if it.get_input_symbol(&self.coder) == isymbol
                && it.get_output_symbol(&self.coder) == osymbol
            {
                return Some(it.get_target_state());
            }
            if it.get_input_symbol(&self.coder) == crate::hfst_symbol_defs::internal_identity
                && it.get_output_symbol(&self.coder) == crate::hfst_symbol_defs::internal_identity
            {
                identity_target = Some(it.get_target_state());
            }
        }
        if isymbol == osymbol && !known_symbols.contains(isymbol) {
            identity_target
        } else {
            None
        }
    }

    /// Return a copy with the states renumbered in discovery order: state 0
    /// stays 0, and every other state is assigned the next free id the first
    /// time it is reached — either as the running iteration source or as an arc
    /// target. All transitions are copied verbatim with their targets remapped.
    /// This compacts the state numbering; it is the pure-renumber core lifted
    /// from hfst-preprocess-for-optimized-lookup-format.
    ///
    /// Note: state 0 is pre-seeded into the id map, so its final weight is not
    /// copied (matching the long-standing behaviour of the CLI loop this was
    /// lifted from). Callers that need state 0's final weight preserved set it
    /// themselves before/after renumbering.
    pub fn renumber_states(&self) -> HfstBasicTransducer {
        let mut replication = HfstBasicTransducer::new();
        let mut state_count: HfstState = 1;
        let mut rebuilt: BTreeMap<HfstState, HfstState> = BTreeMap::new();
        rebuilt.insert(0, 0);
        let mut source_state: HfstState = 0;
        for state in self.iter() {
            if !rebuilt.contains_key(&source_state) {
                replication.add_state(state_count);
                if self.is_final_state(source_state) {
                    replication.set_final_weight(state_count, &self.get_final_weight(source_state));
                }
                rebuilt.insert(source_state, state_count);
                state_count += 1;
            }
            for arc in state.iter() {
                if !rebuilt.contains_key(&arc.get_target_state()) {
                    replication.add_state(state_count);
                    if self.is_final_state(arc.get_target_state()) {
                        replication.set_final_weight(
                            state_count,
                            &self.get_final_weight(arc.get_target_state()),
                        );
                    }
                    rebuilt.insert(arc.get_target_state(), state_count);
                    state_count += 1;
                }
                let isym = arc.get_input_symbol(&self.coder);
                let osym = arc.get_output_symbol(&self.coder);
                let nu = HfstBasicTransition::new_symbols(
                    rebuilt[&arc.get_target_state()],
                    isym,
                    osym,
                    arc.get_weight(),
                    replication.coder_mut(),
                );
                let src_rebuilt = rebuilt[&source_state];
                replication.add_transition(src_rebuilt, &nu, true);
            }
            source_state += 1;
        }
        replication
    }

    /// Return a copy with every transition whose input or output symbol equals
    /// `symbol` removed, surviving states renumbered in discovery order. This is
    /// the kill-paths transform lifted from hfst-kill-paths: the discovery-order
    /// rebuild of [`Self::renumber_states`] plus a per-arc filter. Unlike
    /// `renumber_states` it seeds state 0's final weight up front (matching the
    /// CLI loop), so an accepting start state is preserved.
    pub fn kill_paths(&self, symbol: &str) -> HfstBasicTransducer {
        let mut replication = HfstBasicTransducer::new();
        let mut state_count: HfstState = 1;
        let mut rebuilt: BTreeMap<HfstState, HfstState> = BTreeMap::new();
        rebuilt.insert(0, 0);
        if self.is_final_state(0) {
            replication.set_final_weight(0, &self.get_final_weight(0));
        }
        let mut source_state: HfstState = 0;
        for state in self.iter() {
            if !rebuilt.contains_key(&source_state) {
                replication.add_state(state_count);
                if self.is_final_state(source_state) {
                    replication.set_final_weight(state_count, &self.get_final_weight(source_state));
                }
                rebuilt.insert(source_state, state_count);
                state_count += 1;
            }
            for arc in state.iter() {
                if arc.get_input_symbol(&self.coder) == symbol
                    || arc.get_output_symbol(&self.coder) == symbol
                {
                    // killed arc: do not replicate
                    continue;
                }
                if !rebuilt.contains_key(&arc.get_target_state()) {
                    replication.add_state(state_count);
                    if self.is_final_state(arc.get_target_state()) {
                        replication.set_final_weight(
                            state_count,
                            &self.get_final_weight(arc.get_target_state()),
                        );
                    }
                    rebuilt.insert(arc.get_target_state(), state_count);
                    state_count += 1;
                }
                let isym = arc.get_input_symbol(&self.coder);
                let osym = arc.get_output_symbol(&self.coder);
                let nu = HfstBasicTransition::new_symbols(
                    rebuilt[&arc.get_target_state()],
                    isym,
                    osym,
                    arc.get_weight(),
                    replication.coder_mut(),
                );
                replication.add_transition(rebuilt[&source_state], &nu, true);
            }
            source_state += 1;
        }
        replication
    }

    /// Return a copy with every weight transformed by `f`, surviving states
    /// renumbered in discovery order (the do_reweight rebuild from hfst-reweight).
    /// `f` receives the current weight together with the transition's symbols so
    /// it can reweight conditionally: `(w, None, None)` for a state's final weight
    /// and `(w, Some(input), Some(output))` for an arc weight; it returns the new
    /// weight. Unlike the unconditional backend `transform_weights`, this is
    /// symbol-aware. State 0's final weight is seeded up front (matching the CLI
    /// loop, like [`Self::kill_paths`]).
    pub fn transform_weights<F>(&self, f: F) -> HfstBasicTransducer
    where
        F: Fn(f32, Option<&str>, Option<&str>) -> f32,
    {
        let mut replication = HfstBasicTransducer::new();
        let mut state_count: HfstState = 1;
        let mut rebuilt: BTreeMap<HfstState, HfstState> = BTreeMap::new();
        rebuilt.insert(0, 0);
        if self.is_final_state(0) {
            replication.set_final_weight(0, &f(self.get_final_weight(0), None, None));
        }
        let mut source_state: HfstState = 0;
        for state in self.iter() {
            if !rebuilt.contains_key(&source_state) {
                replication.add_state(state_count);
                if self.is_final_state(source_state) {
                    replication.set_final_weight(
                        state_count,
                        &f(self.get_final_weight(source_state), None, None),
                    );
                }
                rebuilt.insert(source_state, state_count);
                state_count += 1;
            }
            for arc in state.iter() {
                let target = arc.get_target_state();
                if !rebuilt.contains_key(&target) {
                    replication.add_state(state_count);
                    if self.is_final_state(target) {
                        replication.set_final_weight(
                            state_count,
                            &f(self.get_final_weight(target), None, None),
                        );
                    }
                    rebuilt.insert(target, state_count);
                    state_count += 1;
                }
                let isym = arc.get_input_symbol(&self.coder);
                let osym = arc.get_output_symbol(&self.coder);
                let nuweight = f(arc.get_weight(), Some(&isym), Some(&osym));
                let nu = HfstBasicTransition::new_symbols(
                    rebuilt[&target],
                    isym,
                    osym,
                    nuweight,
                    replication.coder_mut(),
                );
                replication.add_transition(rebuilt[&source_state], &nu, true);
            }
            source_state += 1;
        }
        replication
    }

    /// Compute hfst-summarize's single-pass arc-traversal statistics. Walks every
    /// state and transition once, accumulating counts, the seen alphabet, the
    /// per-state input/output ambiguity (→ determinism + most-ambiguous symbols),
    /// epsilon counts, the acceptor flag, and cyclicity. Transcribed verbatim from
    /// the tool's loop; the symbol-pair map is always populated (the tool decides
    /// whether to print it).
    pub fn summarize(&self) -> SummaryStats {
        let mut states: usize = 0;
        let mut final_states: usize = 0;
        let mut arcs: usize = 0;
        let mut io_epsilons: usize = 0;
        let mut input_epsilons: usize = 0;
        let mut output_epsilons: usize = 0;
        let mut densest_arcs: usize = 0;
        let mut sparsest_arcs: usize = 1 << 31;
        let mut uniq_input_arcs: usize = 0;
        let mut uniq_output_arcs: usize = 0;
        let mut most_ambiguous_input: (String, u32) = (String::new(), 0);
        let mut most_ambiguous_output: (String, u32) = (String::new(), 0);
        let mut found_alphabet: BTreeSet<String> = BTreeSet::new();
        let mut symbol_pairs: BTreeMap<(String, String), u32> = BTreeMap::new();
        let mut acceptor = true;
        let mut input_deterministic = true;
        let mut output_deterministic = true;
        let mut cyclic = false;
        let mut cyclic_at_initial_state = false;

        let mut source_state: u32 = 0;
        let is_begin_state = |s: u32| s == 0;
        for transitions in self.states_and_transitions() {
            let s = source_state;
            states += 1;
            if self.is_final_state(s) {
                final_states += 1;
            }
            let mut arcs_here: usize = 0;
            let mut input_ambiguity: BTreeMap<String, u32> = BTreeMap::new();
            let mut output_ambiguity: BTreeMap<String, u32> = BTreeMap::new();

            for tr_it in transitions {
                arcs += 1;
                arcs_here += 1;
                let in_sym = tr_it.get_input_symbol(&self.coder);
                let out_sym = tr_it.get_output_symbol(&self.coder);
                found_alphabet.insert(in_sym.clone());
                found_alphabet.insert(out_sym.clone());

                *symbol_pairs
                    .entry((in_sym.clone(), out_sym.clone()))
                    .or_insert(0) += 1;

                if in_sym != out_sym {
                    acceptor = false;
                }
                if is_epsilon(&in_sym) && is_epsilon(&out_sym) {
                    io_epsilons += 1;
                    input_epsilons += 1;
                    output_epsilons += 1;
                    input_deterministic = false;
                    output_deterministic = false;
                } else if is_epsilon(&in_sym) {
                    input_epsilons += 1;
                    input_deterministic = false;
                } else if is_epsilon(&out_sym) {
                    output_epsilons += 1;
                    output_deterministic = false;
                }
                input_ambiguity.entry(in_sym.clone()).or_insert(0);
                output_ambiguity.entry(out_sym.clone()).or_insert(0);
                let in_amb = input_ambiguity.get_mut(&in_sym).unwrap();
                *in_amb += 1;
                if *in_amb > 1 {
                    input_deterministic = false;
                }
                let out_amb = output_ambiguity.get_mut(&out_sym).unwrap();
                *out_amb += 1;
                if *out_amb > 1 {
                    output_deterministic = false;
                }
                if is_begin_state(source_state) && (tr_it.get_target_state() == 0) {
                    cyclic = true;
                    cyclic_at_initial_state = true;
                }
                if source_state == tr_it.get_target_state() {
                    cyclic = true;
                }
            }
            if arcs_here > densest_arcs {
                densest_arcs = arcs_here;
            }
            if arcs_here < sparsest_arcs {
                sparsest_arcs = arcs_here;
            }
            for (key, value) in input_ambiguity.iter() {
                if *value > most_ambiguous_input.1 {
                    most_ambiguous_input.0 = key.clone();
                    most_ambiguous_input.1 = *value;
                }
                uniq_input_arcs += 1;
            }
            for (key, value) in output_ambiguity.iter() {
                if *value > most_ambiguous_output.1 {
                    most_ambiguous_output.0 = key.clone();
                    most_ambiguous_output.1 = *value;
                }
                uniq_output_arcs += 1;
            }
            source_state += 1;
        }

        SummaryStats {
            states,
            final_states,
            arcs,
            io_epsilons,
            input_epsilons,
            output_epsilons,
            densest_arcs,
            sparsest_arcs,
            uniq_input_arcs,
            uniq_output_arcs,
            most_ambiguous_input,
            most_ambiguous_output,
            found_alphabet,
            symbol_pairs,
            acceptor,
            input_deterministic,
            output_deterministic,
            cyclic,
            cyclic_at_initial_state,
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.prune-alphabet-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.prune-alphabet-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.prune-alphabet-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.prune-alphabet-fn]
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

    /// This graph's own symbol<->number coding (idiom5 keystone). All tropical
    /// symbol resolution (number->string) and interning (string->number) for
    /// this graph's arcs goes through it; binary ops harmonize two graphs'
    /// codings via [`SymbolCoder::create_translator_from`].
    pub fn coder(&self) -> &SymbolCoder {
        &self.coder
    }

    pub fn coder_mut(&mut self) -> &mut SymbolCoder {
        &mut self.coder
    }

    /// Intern this graph's coder symbols *and* its full alphabet into the shared
    /// `canonical` coder, without changing this graph. Call this for every graph
    /// participating in a binary op BEFORE [`Self::reindex_into`] so that
    /// `canonical` already holds a number for every symbol any of them uses; that
    /// makes the per-graph numbering agree even for alphabet-only symbols (which a
    /// graph's own coder may lack until interned).
    pub fn intern_into(&self, canonical: &mut SymbolCoder) {
        for symbol in self.coder.number2symbol_slice() {
            if !symbol.is_empty() {
                canonical.get_number(symbol);
            }
        }
        for symbol in self.alphabet.iter() {
            if !symbol.is_empty() {
                canonical.get_number(symbol);
            }
        }
    }

    /// Re-number every arc so its symbols are coded by the shared `canonical`
    /// coder, then adopt a clone of `canonical` as this graph's own coding. Pair
    /// with [`Self::intern_into`]: intern *all* participating graphs into one
    /// `canonical` first, then `reindex_into` each. After that they all share one
    /// numbering, so their symbol numbers can be combined directly — the
    /// per-graph-coder replacement for the former process-global numbering,
    /// applied ONCE at a binary-op boundary.
    pub fn reindex_into(&mut self, canonical: &mut SymbolCoder) {
        // translator[old_number] = number of the same symbol in the shared coding.
        let translator = canonical.create_translator_from(&self.coder);
        for transitions in self.state_vector.iter_mut() {
            for i in 0..transitions.len() {
                let tr = &transitions[i];
                let new_in = translator[tr.get_input_number() as usize];
                let new_out = translator[tr.get_output_number() as usize];
                transitions[i] = HfstBasicTransition::new_numbers(
                    tr.get_target_state(),
                    new_in,
                    new_out,
                    tr.get_weight(),
                    false,
                );
            }
        }
        self.coder = canonical.clone();
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-transition-pairs-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-transition-pairs-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.get-transition-pairs-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.get-transition-pairs-fn]
    pub fn get_transition_pairs(&self) -> StringPairSet {
        let mut retval = StringPairSet::new();
        for it in self.state_vector.iter() {
            for tr_it in it.iter() {
                let data = tr_it.get_transition_data();
                retval.insert(StringPair::from((
                    data.get_input_symbol(&self.coder),
                    data.get_output_symbol(&self.coder),
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
                retval.insert(data.get_input_symbol(&self.coder));
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
                retval.insert(data.get_output_symbol(&self.coder));
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
        if !(self.state_vector.len() > s as usize) {
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
    pub fn index(&self, s: HfstState) -> &HfstBasicTransitions {
        if s as usize >= self.state_vector.len() {
            crate::HFST_THROW!(StateIndexOutOfBoundsException);
        }
        &self.state_vector[s as usize]
    }

    /** @brief Alternative name for 'operator[]'. */
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
                    let isym = it[i].get_input_symbol(coder);
                    let osym = it[i].get_output_symbol(coder);
                    let w = it[i].get_weight();
                    let tr = HfstBasicTransition::new_symbols(new_target, isym, osym, w, coder);
                    it[i] = tr;
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

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-weight-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-weight-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.write-weight-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.write-weight-fn]
    pub fn write_weight_file(file: &mut dyn Write, weight: f32) {
        w_fputs(file, &format!("{:.6}", weight));
    }

    // The C++ ostream '<<' float formatting (6 significant digits) differs from
    // the FILE '%f' path above, and Rust's default '{}' differs from both;
    // forgiven unless a ported test proves the exact text.
    pub fn write_weight_os(os: &mut dyn Write, weight: f32) {
        let _ = write!(os, "{}", weight);
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.xfstize-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.xfstize-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.xfstize-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.xfstize-fn]
    //
    // Iterates bytes (C++ 'for (char pos : symbol)' over a byte string); the
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
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.xfstize-symbol-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.xfstize-symbol-fn]
    pub fn xfstize_symbol(symbol: &mut String) {
        Self::xfstize(symbol);
        crate::string_utils::replace_all(symbol, "@_EPSILON_SYMBOL_@", "0");
        crate::string_utils::replace_all(symbol, "@_UNKNOWN_SYMBOL_@", "?");
        crate::string_utils::replace_all(symbol, "@_IDENTITY_SYMBOL_@", "?");
        crate::string_utils::replace_all(symbol, "\t", "@_TAB_@");
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-xfst-state-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-xfst-state-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.print-xfst-state-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.print-xfst-state-fn]
    pub fn print_xfst_state_os(&self, os: &mut dyn Write, state: HfstState) {
        if state == Self::INITIAL_STATE {
            let _ = write!(os, "S");
        }
        if self.is_final_state(state) {
            let _ = write!(os, "f");
        }
        let _ = write!(os, "s{}", state);
    }

    pub fn print_xfst_state_file(&self, file: &mut dyn Write, state: HfstState) {
        if state == Self::INITIAL_STATE {
            w_fputs(file, "S");
        }
        if self.is_final_state(state) {
            w_fputs(file, "f");
        }
        w_fputs(file, &format!("s{}", state));
    }

    pub fn print_xfst_arc_os(
        &self,
        os: &mut dyn Write,
        data: &HfstTropicalTransducerTransitionData,
    ) {
        let isym = data.get_input_symbol(&self.coder);
        let osym = data.get_output_symbol(&self.coder);
        // replace all spaces, epsilons and tabs
        if isym != osym {
            let _ = write!(os, "<");
        }
        let mut s = isym.clone();
        Self::xfstize_symbol(&mut s);
        let _ = write!(os, "{}", s);
        if isym != osym || osym == "@_UNKNOWN_SYMBOL_@" {
            s = osym.clone();
            Self::xfstize_symbol(&mut s);
            let _ = write!(os, ":{}", s);
        }
        if isym != osym {
            let _ = write!(os, ">");
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-xfst-arc-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-xfst-arc-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.print-xfst-arc-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.print-xfst-arc-fn]
    pub fn print_xfst_arc_file(
        &self,
        file: &mut dyn Write,
        data: &HfstTropicalTransducerTransitionData,
    ) {
        let isym = data.get_input_symbol(&self.coder);
        let osym = data.get_output_symbol(&self.coder);
        if isym != osym {
            w_fputs(file, "<");
        }
        // replace all spaces, epsilons and tabs
        let mut s = isym.clone();
        Self::xfstize_symbol(&mut s);
        w_fputs(file, &s);
        if isym != osym || osym == "@_UNKNOWN_SYMBOL_@" {
            s = osym.clone();
            Self::xfstize_symbol(&mut s);
            w_fputs(file, &format!(":{}", s));
        }
        if isym != osym {
            w_fputs(file, ">");
        }
    }

    /** @brief Write the graph in xfst text format to ostream 'os'. */
    // [spec:hfst:def:hfst-transition-graph.write-in-xfst-format-fn]
    // [spec:hfst:sem:hfst-transition-graph.write-in-xfst-format-fn]
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
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.print-prolog-arc-symbols-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.print-prolog-arc-symbols-fn]
    pub fn print_prolog_arc_symbols_file(
        file: &mut dyn Write,
        data: &HfstTropicalTransducerTransitionData,
        coder: &SymbolCoder,
    ) {
        let isym = data.get_input_symbol(coder);
        let osym = data.get_output_symbol(coder);
        let symbol = Self::prologize_symbol(&isym);
        w_fputs(file, &format!("\"{}\"", symbol));

        if isym != osym || isym == "@_UNKNOWN_SYMBOL_@" {
            let symbol = Self::prologize_symbol(&osym);
            w_fputs(file, &format!(":\"{}\"", symbol));
        }
    }

    pub fn print_prolog_arc_symbols_os(
        os: &mut dyn Write,
        data: &HfstTropicalTransducerTransitionData,
        coder: &SymbolCoder,
    ) {
        let isym = data.get_input_symbol(coder);
        let osym = data.get_output_symbol(coder);
        let symbol = Self::prologize_symbol(&isym);
        let _ = write!(os, "\"{}\"", symbol);

        if isym != osym || isym == "@_UNKNOWN_SYMBOL_@" {
            let symbol = Self::prologize_symbol(&osym);
            let _ = write!(os, ":\"{}\"", symbol);
        }
    }

    /** @brief Write the graph in prolog format to FILE 'file'. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-prolog-format-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-prolog-format-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.write-in-prolog-format-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.write-in-prolog-format-fn]
    pub fn write_in_prolog_format_file(
        &self,
        file: &mut dyn Write,
        name: &str,
        write_weights: bool,
    ) {
        let mut source_state: u32 = 0;
        let identifier = name;
        // Print the name.
        if name.contains(',') {
            let msg = "no commas allowed in the name of prolog networks".to_string();
            crate::HFST_THROW_MESSAGE!(HfstException, msg);
        }
        w_fputs(file, &format!("network({}).\n", identifier));

        // Print symbols that are in the alphabet but not used in arcs.
        let mut symbols_used_ = self.symbols_used();
        Self::initialize_alphabet(&mut symbols_used_); // exclude special symbols
        for it in self.alphabet.iter() {
            if !symbols_used_.contains(it) {
                w_fputs(
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
                w_fputs(
                    file,
                    &format!(
                        "arc({}, {}, {}, ",
                        identifier,
                        source_state,
                        tr_it.get_target_state()
                    ),
                );
                let data = tr_it.get_transition_data();
                Self::print_prolog_arc_symbols_file(file, data, &self.coder);
                if write_weights {
                    w_fputs(file, ", ");
                    Self::write_weight_file(file, data.get_weight());
                }
                w_fputs(file, ").\n");
            }
            source_state += 1;
        }

        // Print final states.
        for (k, v) in self.final_weight_map.iter() {
            w_fputs(file, &format!("final({}, {}", identifier, k));
            if write_weights {
                w_fputs(file, ", ");
                Self::write_weight_file(file, *v);
            }
            w_fputs(file, ").\n");
        }
    }

    /** @brief Write the graph in prolog format to ostream 'os'. */
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
                Self::print_prolog_arc_symbols_os(os, data, &self.coder);
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

    // If 'str' is of format ".+", change it to .+ and return true. Else false.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.strip-quotes-from-both-sides-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.strip-quotes-from-both-sides-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.strip-quotes-from-both-sides-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.strip-quotes-from-both-sides-fn]
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

    // If 'str' is of format .+)\." change it to .+ and return true. Else false.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.strip-ending-parenthesis-and-comma-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.strip-ending-parenthesis-and-comma-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.strip-ending-parenthesis-and-comma-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.strip-ending-parenthesis-and-comma-fn]
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
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.parse-prolog-network-line-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.parse-prolog-network-line-fn]
    //
    // sscanf(line, "network(%s", namearr): match the literal prefix, then '%s'
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

    // Get positions of 'c' in 'str'. If 'esc' precedes 'c', 'c' is not included.
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

    // Extract input/output symbols from prolog arc 'str' of format "foo":"bar"
    // or "foo". Return whether symbols were successfully extracted.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-prolog-arc-symbols-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-prolog-arc-symbols-fn]
    // [spec:hfst:def:hfst-transition-graph.get-prolog-arc-symbols-fn]
    // [spec:hfst:sem:hfst-transition-graph.get-prolog-arc-symbols-fn]
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
    // [spec:hfst:def:hfst-transition-graph.extract-weight-fn]
    // [spec:hfst:sem:hfst-transition-graph.extract-weight-fn]
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
    // [spec:hfst:def:hfst-transition-graph.parse-prolog-arc-line-fn]
    // [spec:hfst:sem:hfst-transition-graph.parse-prolog-arc-line-fn]
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

        let tr =
            HfstBasicTransition::new_symbols(target, isymbol, osymbol, weight, graph.coder_mut());
        graph.add_transition(source, &tr, true);
        true
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.parse-prolog-final-line-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.parse-prolog-final-line-fn]
    // [spec:hfst:def:hfst-transition-graph.parse-prolog-final-line-fn]
    // [spec:hfst:sem:hfst-transition-graph.parse-prolog-final-line-fn]
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
    // [spec:hfst:def:hfst-transition-graph.parse-prolog-symbol-line-fn]
    // [spec:hfst:sem:hfst-transition-graph.parse-prolog-symbol-line-fn]
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

    // Erase newlines from the end of 'str' and return 'str'.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.strip-newlines-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.strip-newlines-fn]
    // [spec:hfst:def:hfst-transition-graph.std.string-strip-newlines-fn]
    // [spec:hfst:sem:hfst-transition-graph.std.string-strip-newlines-fn]
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

    /** @brief Write the graph in xfst text format to FILE 'file'. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-xfst-format-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-xfst-format-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.write-in-xfst-format-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.write-in-xfst-format-fn]
    pub fn write_in_xfst_format_file(&self, file: &mut dyn Write, write_weights: bool) {
        let _ = write_weights;
        let mut source_state: u32 = 0;
        for it in self.state_vector.iter() {
            self.print_xfst_state_file(file, source_state);
            w_fputs(file, ":\t");

            if it.is_empty() {
                w_fputs(file, "(no arcs)");
            } else {
                for (i, tr_it) in it.iter().enumerate() {
                    if i != 0 {
                        w_fputs(file, ", ");
                    }
                    let data = tr_it.get_transition_data();
                    self.print_xfst_arc_file(file, data);

                    w_fputs(file, " -> ");
                    self.print_xfst_state_file(file, tr_it.get_target_state());
                }
            }
            w_fputs(file, ".\n");
            source_state += 1;
        }
    }

    /** @brief Write the graph in AT&T format to ostream 'os'. */
    pub fn write_in_att_format_os(&self, os: &mut dyn Write, write_weights: bool) {
        let mut source_state: u32 = 0;
        for it in self.state_vector.iter() {
            for tr_it in it.iter() {
                let data = tr_it.get_transition_data().clone();

                let mut isymbol = data.get_input_symbol(&self.coder);
                replace_all(&mut isymbol, " ", "@_SPACE_@");
                replace_all(&mut isymbol, "@_EPSILON_SYMBOL_@", "@0@");
                replace_all(&mut isymbol, "\t", "@_TAB_@");

                let mut osymbol = data.get_output_symbol(&self.coder);
                replace_all(&mut osymbol, " ", "@_SPACE_@");
                replace_all(&mut osymbol, "@_EPSILON_SYMBOL_@", "@0@");
                replace_all(&mut osymbol, "\t", "@_TAB_@");

                let _ = write!(
                    os,
                    "{}\t{}\t{}\t{}",
                    source_state,
                    tr_it.get_target_state(),
                    isymbol,
                    osymbol
                );

                if write_weights {
                    let _ = write!(os, "\t");
                    Self::write_weight_os(os, data.get_weight());
                }
                let _ = write!(os, "\n");
            }
            if self.is_final_state(source_state) {
                let _ = write!(os, "{}", source_state);
                if write_weights {
                    let _ = write!(os, "\t");
                    Self::write_weight_os(os, self.get_final_weight(source_state));
                }
                let _ = write!(os, "\n");
            }
            source_state += 1;
        }
    }

    /** @brief Write the graph in AT&T format to FILE 'file'. */
    pub fn write_in_att_format_file(&self, file: &mut dyn Write, write_weights: bool) {
        let mut source_state: u32 = 0;
        for it in self.state_vector.iter() {
            for tr_it in it.iter() {
                let data = tr_it.get_transition_data().clone();

                let mut isymbol = data.get_input_symbol(&self.coder);
                replace_all(&mut isymbol, " ", "@_SPACE_@");
                replace_all(&mut isymbol, "@_EPSILON_SYMBOL_@", "@0@");
                replace_all(&mut isymbol, "\t", "@_TAB_@");

                let mut osymbol = data.get_output_symbol(&self.coder);
                replace_all(&mut osymbol, " ", "@_SPACE_@");
                replace_all(&mut osymbol, "@_EPSILON_SYMBOL_@", "@0@");
                replace_all(&mut osymbol, "\t", "@_TAB_@");

                w_fputs(
                    file,
                    &format!(
                        "{}\t{}\t{}\t{}",
                        source_state,
                        tr_it.get_target_state(),
                        isymbol,
                        osymbol
                    ),
                );

                if write_weights {
                    w_fputs(file, "\t");
                    Self::write_weight_file(file, data.get_weight());
                }
                w_fputs(file, "\n");
            }
            if self.is_final_state(source_state) {
                w_fputs(file, &format!("{}", source_state));
                if write_weights {
                    w_fputs(file, "\t");
                    Self::write_weight_file(file, self.get_final_weight(source_state));
                }
                w_fputs(file, "\n");
            }
            source_state += 1;
        }
    }

    /** @brief Write the graph in AT&T format to FILE 'file' using numbers
    instead of symbol names. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-att-format-number-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-att-format-number-fn]
    //
    // NB: the C++ prints the final-state line *inside* the transition loop (so a
    // multi-transition final state repeats it); preserved bug-for-bug.
    // [spec:hfst:def:hfst-transition-graph.write-in-att-format-number-fn]
    // [spec:hfst:sem:hfst-transition-graph.write-in-att-format-number-fn]
    pub fn write_in_att_format_number_file(&self, file: &mut dyn Write, write_weights: bool) {
        let mut source_state: u32 = 0;
        for it in self.state_vector.iter() {
            for tr_it in it.iter() {
                let data = tr_it.get_transition_data().clone();

                w_fputs(
                    file,
                    &format!(
                        "{}\t{}\t{}\t{}",
                        source_state,
                        tr_it.get_target_state(),
                        tr_it.get_input_number(),
                        tr_it.get_output_number()
                    ),
                );

                if write_weights {
                    w_fputs(file, &format!("\t{:.6}", data.get_weight()));
                }
                w_fputs(file, "\n");

                if self.is_final_state(source_state) {
                    w_fputs(file, &format!("{}", source_state));
                    if write_weights {
                        w_fputs(
                            file,
                            &format!("\t{:.6}", self.get_final_weight(source_state)),
                        );
                    }
                    w_fputs(file, "\n");
                }
            }
            source_state += 1;
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-att-line-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-att-line-fn]
    //
    // sscanf(line, "%s%s%s%s%s", ...) reads up to five whitespace-delimited
    // fields; 'n' is how many were read.
    // [spec:hfst:def:hfst-transition-graph.add-att-line-fn]
    // [spec:hfst:sem:hfst-transition-graph.add-att-line-fn]
    pub fn add_att_line(&mut self, line: &str, epsilon_symbol: &str, warn_negs: bool) -> bool {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        let n = tokens.len().min(5);
        let a = |i: usize| -> &str { tokens.get(i).copied().unwrap_or("") };

        // set value of weight
        let mut weight: f32 = 0.0;
        if n == 2 {
            // a final state line with weight
            weight = double_to_float(atof(a(1)));
        }
        if n == 5 {
            // a transition line with weight
            weight = double_to_float(atof(a(4)));
        }
        if (weight < 0.0) && warn_negs {
            eprintln!("Negative weight {:.6} found :-(", weight);
        }

        if n == 1 || n == 2 {
            // a final state line
            self.set_final_weight(atoi(a(0)), &weight);
        } else if n == 4 || n == 5 {
            // a transition line
            let mut input_symbol = a(2).to_string();
            let mut output_symbol = a(3).to_string();

            // replace "@_SPACE_@"s with " " and "@0@"s with "@_EPSILON_SYMBOL_@"
            replace_all(&mut input_symbol, "@_SPACE_@", " ");
            replace_all(&mut input_symbol, "@0@", "@_EPSILON_SYMBOL_@");
            replace_all(&mut input_symbol, "@_TAB_@", "\t");
            replace_all(&mut input_symbol, "@_COLON_@", ":");

            replace_all(&mut output_symbol, "@_SPACE_@", " ");
            replace_all(&mut output_symbol, "@0@", "@_EPSILON_SYMBOL_@");
            replace_all(&mut output_symbol, "@_TAB_@", "\t");
            replace_all(&mut output_symbol, "@_COLON_@", ":");

            if epsilon_symbol == input_symbol {
                input_symbol = "@_EPSILON_SYMBOL_@".to_string();
            }
            if epsilon_symbol == output_symbol {
                output_symbol = "@_EPSILON_SYMBOL_@".to_string();
            }

            let tr = HfstBasicTransition::new_symbols(
                atoi(a(1)),
                input_symbol,
                output_symbol,
                weight,
                self.coder_mut(),
            );
            self.add_transition(atoi(a(0)), &tr, true);
        } else {
            // line could not be parsed
            return false;
        }
        true
    }

    // HfstBasicTransducer(FILE*) — read an AT&T transducer from 'file'.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-basic-transducer-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-basic-transducer-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-transition-graph-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-transition-graph-fn]
    pub fn new_from_file(file: &mut dyn BufRead) -> Self {
        let mut alphabet = HfstAlphabet::new();
        Self::initialize_alphabet(&mut alphabet);
        let mut state_vector = HfstBasicStates::new();
        state_vector.push(HfstBasicTransitions::new());
        let mut retval = HfstBasicTransducer {
            state_vector,
            final_weight_map: FinalWeightMap::new(),
            alphabet,
            name: String::new(),
            coder: SymbolCoder::new(),
        };
        let mut linecount: u32 = 0;
        let read = Self::read_in_att_format_file(file, "@0@", &mut linecount, false);
        retval.assign(&read);
        retval.name = String::new();
        retval
    }

    // Try to get a line from 'is' (if 'file' is null) or 'file'. On success,
    // strip newlines, increment 'linecount', and return the line; else throw
    // EndOfStreamException.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-stripped-line-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-stripped-line-fn]
    // [spec:hfst:def:hfst-transition-graph.std.string-get-stripped-line-fn]
    // [spec:hfst:sem:hfst-transition-graph.std.string-get-stripped-line-fn]
    pub fn get_stripped_line(is: &mut dyn BufRead, linecount: &mut u32) -> String {
        let linestr = match bufread_fgets(is) {
            None => crate::HFST_THROW!(EndOfStreamException),
            Some(l) => l,
        };
        *linecount += 1;

        let mut s = linestr;
        Self::strip_newlines(&mut s)
    }

    // Create a graph from prolog format in 'is' (if 'file' is null) or 'file'.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.read-in-prolog-format-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.read-in-prolog-format-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst-transition-graph-read-in-prolog-format-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst-transition-graph-read-in-prolog-format-fn]
    pub fn read_in_prolog_format(is: &mut dyn BufRead, linecount: &mut u32) -> HfstBasicTransducer {
        let mut retval = HfstBasicTransducer::new();
        let mut linestr: String;

        loop {
            match catch_get_stripped_line(is, linecount) {
                Some(l) => linestr = l,
                None => crate::HFST_THROW!(NotValidPrologFormatException),
            }

            if linestr.len() != 0 && linestr.as_bytes()[0] == b'#' {
                continue; // comment line
            } else {
                break; // first non-comment line
            }
        }

        if !Self::parse_prolog_network_line(&linestr, &mut retval) {
            let mut message = String::from("first line not valid prolog: ");
            message.push_str(&linestr);
            crate::HFST_THROW_MESSAGE!(NotValidPrologFormatException, message);
        }

        loop {
            match catch_get_stripped_line(is, linecount) {
                Some(l) => {
                    linestr = l;
                    if linestr.is_empty() {
                        // prolog separator
                        return retval;
                    }
                }
                None => return retval,
            }

            if !(Self::parse_prolog_arc_line(&linestr, &mut retval)
                || Self::parse_prolog_final_line(&linestr, &mut retval)
                || Self::parse_prolog_symbol_line(&linestr, &mut retval))
            {
                let mut message = String::from("line not valid prolog: ");
                message.push_str(&linestr);
                crate::HFST_THROW_MESSAGE!(NotValidPrologFormatException, message);
            }
        }
    }

    pub fn read_in_prolog_format_is(
        is: &mut dyn BufRead,
        linecount: &mut u32,
    ) -> HfstBasicTransducer {
        Self::read_in_prolog_format(is, linecount)
    }

    pub fn read_in_prolog_format_file(
        file: &mut dyn BufRead,
        linecount: &mut u32,
    ) -> HfstBasicTransducer {
        Self::read_in_prolog_format(file, linecount)
    }

    // Create a graph from AT&T format in 'is' (if 'file' is null) or 'file'.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.read-in-att-format-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.read-in-att-format-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst-transition-graph-read-in-att-format-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst-transition-graph-read-in-att-format-fn]
    pub fn read_in_att_format(
        is: &mut dyn BufRead,
        epsilon_symbol: &str,
        linecount: &mut u32,
        warn_negs: bool,
    ) -> HfstBasicTransducer {
        if is_eof(is) {
            crate::HFST_THROW!(EndOfStreamException);
        }

        let mut retval = HfstBasicTransducer::new();
        loop {
            let line: String = match bufread_fgets(is) {
                None => break,
                Some(l) => l,
            };

            *linecount += 1;

            let bytes = line.as_bytes();
            // an empty line (with or without newline, incl. windows newline)
            if bytes.is_empty()
                || (bytes.len() == 1 && bytes[0] == b'\n')
                || (bytes.len() == 2 && bytes[0] == b'\r' && bytes[1] == b'\n')
            {
                // make sure that the end-of-file is reached (C++ 'fgetc(file)')
                let mut b = [0u8; 1];
                let _ = is.read(&mut b);
                break;
            }

            if bytes[0] == b'-' {
                // transducer separator line is "--"
                return retval;
            }

            if !retval.add_att_line(&line, epsilon_symbol, warn_negs) {
                let message = line.clone();
                crate::HFST_THROW_MESSAGE!(NotValidAttFormatException, message);
            }
        }
        retval
    }

    pub fn read_in_att_format_is(
        is: &mut dyn BufRead,
        epsilon_symbol: &str,
        linecount: &mut u32,
        warn_negs: bool,
    ) -> HfstBasicTransducer {
        Self::read_in_att_format(is, epsilon_symbol, linecount, warn_negs)
    }

    pub fn read_in_att_format_file(
        file: &mut dyn BufRead,
        epsilon_symbol: &str,
        linecount: &mut u32,
        warn_negs: bool,
    ) -> HfstBasicTransducer {
        Self::read_in_att_format(file, epsilon_symbol, linecount, warn_negs)
    }

    // --- Substitution (private in-place helpers) ---

    /* In-place substitution of 'old_symbol' with 'new_symbol'. */
    fn substitute_in_place(
        &mut self,
        old_symbol: &HfstSymbol,
        new_symbol: &HfstSymbol,
        input_side: bool,
        output_side: bool,
    ) {
        for s in 0..self.state_vector.len() {
            for i in 0..self.state_vector[s].len() {
                let mut substituting_input_symbol =
                    self.state_vector[s][i].get_input_symbol(&self.coder);
                let mut substituting_output_symbol =
                    self.state_vector[s][i].get_output_symbol(&self.coder);
                let mut substitution_made = false;

                if input_side && substituting_input_symbol == *old_symbol {
                    substituting_input_symbol = new_symbol.clone();
                    substitution_made = true;
                }
                if output_side && substituting_output_symbol == *old_symbol {
                    substituting_output_symbol = new_symbol.clone();
                    substitution_made = true;
                }

                if substitution_made {
                    self.add_symbol_to_alphabet(new_symbol);
                    let target = self.state_vector[s][i].get_target_state();
                    let weight = self.state_vector[s][i].get_weight();
                    let tr = HfstBasicTransition::new_symbols(
                        target,
                        substituting_input_symbol,
                        substituting_output_symbol,
                        weight,
                        self.coder_mut(),
                    );
                    self.state_vector[s][i] = tr;
                }
            }
        }
    }

    /* In-place substitution by number vector: substitutions[from] = to. */
    fn substitute_in_place_numbers(
        &mut self,
        substitutions: &HfstNumberVector,
        no_substitution: u32,
    ) {
        for s in 0..self.state_vector.len() {
            for i in 0..self.state_vector[s].len() {
                let old_inumber = self.state_vector[s][i].get_input_number();
                let old_onumber = self.state_vector[s][i].get_output_number();

                let mut new_inumber = substitutions[old_inumber as usize];
                let mut new_onumber = substitutions[old_onumber as usize];

                if new_inumber != no_substitution || new_onumber != no_substitution {
                    if new_inumber != no_substitution {
                        let sym = self.coder.get_symbol(new_inumber);
                        self.add_symbol_to_alphabet(&sym);
                    } else {
                        new_inumber = old_inumber;
                    }

                    if new_onumber != no_substitution {
                        let sym = self.coder.get_symbol(new_onumber);
                        self.add_symbol_to_alphabet(&sym);
                    } else {
                        new_onumber = old_onumber;
                    }

                    let target = self.state_vector[s][i].get_target_state();
                    let weight = self.state_vector[s][i].get_weight();
                    let tr = HfstBasicTransition::new_numbers(
                        target,
                        new_inumber,
                        new_onumber,
                        weight,
                        false,
                    );
                    self.state_vector[s][i] = tr;
                }
            }
        }
    }

    /* In-place substitution by number-pair map. */
    fn substitute_in_place_number_pairs(&mut self, substitutions: &HfstNumberPairSubstitutions) {
        for s in 0..self.state_vector.len() {
            for i in 0..self.state_vector[s].len() {
                let old_number_pair = (
                    self.state_vector[s][i].get_input_number(),
                    self.state_vector[s][i].get_output_number(),
                );

                if let Some(subst) = substitutions.get(&old_number_pair) {
                    let new_input_number = subst.0;
                    let new_output_number = subst.1;

                    let in_sym = self.coder.get_symbol(new_input_number);
                    self.add_symbol_to_alphabet(&in_sym);
                    let out_sym = self.coder.get_symbol(new_output_number);
                    self.add_symbol_to_alphabet(&out_sym);

                    let target = self.state_vector[s][i].get_target_state();
                    let weight = self.state_vector[s][i].get_weight();
                    let tr = HfstBasicTransition::new_numbers(
                        target,
                        new_input_number,
                        new_output_number,
                        weight,
                        false,
                    );
                    self.state_vector[s][i] = tr;
                }
            }
        }
    }

    /* In-place removal of all transitions equivalent to 'sp'. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-transitions-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-transitions-fn]
    // [spec:hfst:def:hfst-transition-graph.remove-transitions-fn]
    // [spec:hfst:sem:hfst-transition-graph.remove-transitions-fn]
    pub fn remove_transitions(&mut self, sp: &HfstSymbolPair) {
        let in_match = self.coder.get_number(&sp.0);
        let out_match = self.coder.get_number(&sp.1);

        let mut in_match_used = false;
        let mut out_match_used = false;

        for s in 0..self.state_vector.len() {
            // C++ 'for (i=0; i<size(); i++)' with erase but no 'i--': after an
            // erase the shifted element is skipped — bug preserved.
            let mut i = 0;
            while i < self.state_vector[s].len() {
                let in_tr = self.state_vector[s][i].get_input_number();
                let out_tr = self.state_vector[s][i].get_output_number();
                if in_tr == in_match && out_tr == out_match {
                    self.state_vector[s].remove(i);
                } else {
                    if in_tr == in_match || out_tr == in_match {
                        in_match_used = true;
                    }
                    if in_tr == out_match || out_tr == out_match {
                        out_match_used = true;
                    }
                }
                i += 1;
            }
        }

        if !in_match_used {
            self.alphabet.remove(&sp.0);
        }
        if !out_match_used {
            self.alphabet.remove(&sp.1);
        }
    }

    /* In-place substitution of 'old_sp' with the set 'new_sps'. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.substitute-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.substitute-fn]
    // [spec:hfst:def:hfst-transition-graph.substitute-fn]
    // [spec:hfst:sem:hfst-transition-graph.substitute-fn]
    fn substitute_in_place_pair_set(
        &mut self,
        old_sp: &HfstSymbolPair,
        new_sps: &HfstSymbolPairSet,
    ) {
        if new_sps.is_empty() {
            self.remove_transitions(old_sp);
            return;
        }

        let old_input_number = self.coder.get_number(&old_sp.0);
        let old_output_number = self.coder.get_number(&old_sp.1);

        let mut substitution_performed = false;

        for s in 0..self.state_vector.len() {
            let mut new_transitions: HfstBasicTransitions = Vec::new();

            for i in 0..self.state_vector[s].len() {
                if self.state_vector[s][i].get_input_number() == old_input_number
                    && self.state_vector[s][i].get_output_number() == old_output_number
                {
                    substitution_performed = true;
                    let target = self.state_vector[s][i].get_target_state();
                    let weight = self.state_vector[s][i].get_weight();

                    // change the transition to the first substituting pair
                    let first = new_sps.iter().next().unwrap();
                    let first_in = self.coder.get_number(&first.0);
                    let first_out = self.coder.get_number(&first.1);
                    let tr =
                        HfstBasicTransition::new_numbers(target, first_in, first_out, weight, true);
                    self.state_vector[s][i] = tr;

                    // schedule the rest (C++ iterates from begin, so all of
                    // new_sps incl. the first are appended).
                    for sp in new_sps.iter() {
                        let sp_in = self.coder.get_number(&sp.0);
                        let sp_out = self.coder.get_number(&sp.1);
                        let tr2 =
                            HfstBasicTransition::new_numbers(target, sp_in, sp_out, weight, true);
                        new_transitions.push(tr2);
                    }
                }
            }

            for new_transition in new_transitions.iter() {
                self.state_vector[s].push(new_transition.clone());
            }
        }

        if substitution_performed {
            self.add_symbols_to_alphabet_pair_set(new_sps);
        }

        let mut syms: BTreeSet<u32> = BTreeSet::new();
        syms.insert(old_input_number);
        syms.insert(old_output_number);
        self.prune_alphabet_after_substitution(&syms);
    }

    /* In-place substitution by a user function. */
    fn substitute_in_place_func(
        &mut self,
        func: impl Fn(&HfstSymbolPair, &mut HfstSymbolPairSet) -> bool,
    ) {
        for s in 0..self.state_vector.len() {
            let mut new_transitions: HfstBasicTransitions = Vec::new();

            for i in 0..self.state_vector[s].len() {
                let transition_symbol_pair = (
                    self.state_vector[s][i].get_input_symbol(&self.coder),
                    self.state_vector[s][i].get_output_symbol(&self.coder),
                );
                let mut substituting_transitions: HfstSymbolPairSet = BTreeSet::new();

                // C++ wraps this in try/catch(HfstException){throw e;} — a no-op
                // rethrow, so a thrown exception just propagates.
                let perform_substitution =
                    func(&transition_symbol_pair, &mut substituting_transitions);
                if perform_substitution {
                    let target = self.state_vector[s][i].get_target_state();
                    let weight = self.state_vector[s][i].get_weight();

                    let (fi, fo) = {
                        let first = substituting_transitions.iter().next().unwrap();
                        (first.0.clone(), first.1.clone())
                    };
                    if !HfstTropicalTransducerTransitionData::is_valid_symbol(&fi)
                        || !HfstTropicalTransducerTransitionData::is_valid_symbol(&fo)
                    {
                        crate::HFST_THROW_MESSAGE!(
                            EmptyStringException,
                            "HfstBasicTransducer::substitute"
                        );
                    }

                    let tr = HfstBasicTransition::new_symbols(
                        target,
                        fi.clone(),
                        fo.clone(),
                        weight,
                        self.coder_mut(),
                    );
                    self.state_vector[s][i] = tr;

                    self.add_symbol_to_alphabet(&fi);
                    self.add_symbol_to_alphabet(&fo);

                    for sp in substituting_transitions.iter() {
                        if !HfstTropicalTransducerTransitionData::is_valid_symbol(&sp.0)
                            || !HfstTropicalTransducerTransitionData::is_valid_symbol(&sp.1)
                        {
                            crate::HFST_THROW_MESSAGE!(
                                EmptyStringException,
                                "HfstBasicTransducer::substitute"
                            );
                        }
                        let tr2 = HfstBasicTransition::new_symbols(
                            target,
                            sp.0.clone(),
                            sp.1.clone(),
                            weight,
                            self.coder_mut(),
                        );
                        new_transitions.push(tr2);
                        self.add_symbol_to_alphabet(&sp.0);
                        self.add_symbol_to_alphabet(&sp.1);
                    }
                }
            }

            for new_transition in new_transitions.iter() {
                self.state_vector[s].push(new_transition.clone());
            }
        }
    }

    // --- Substitution (public) ---

    /** @brief Substitute 'old_symbol' with 'new_symbol' in all transitions. */
    pub fn substitute_symbol(
        &mut self,
        old_symbol: &HfstSymbol,
        new_symbol: &HfstSymbol,
        input_side: bool,
        output_side: bool,
    ) -> &mut Self {
        if !HfstTropicalTransducerTransitionData::is_valid_symbol(old_symbol)
            || !HfstTropicalTransducerTransitionData::is_valid_symbol(new_symbol)
        {
            crate::HFST_THROW_MESSAGE!(EmptyStringException, "HfstBasicTransducer::substitute");
        }

        // If a symbol is substituted with itself, do nothing.
        if old_symbol == new_symbol {
            return self;
        }
        // If the old symbol is not known to the graph, do nothing.
        if !self.alphabet.contains(old_symbol) {
            return self;
        }

        // Remove the substituted symbol from the alphabet if both sides.
        if input_side && output_side {
            if !is_epsilon(old_symbol) && !is_unknown(old_symbol) && !is_identity(old_symbol) {
                self.alphabet.remove(old_symbol);
            }
        }
        self.alphabet.insert(new_symbol.clone());

        self.substitute_in_place(old_symbol, new_symbol, input_side, output_side);

        self
    }

    pub fn substitute_symbols(&mut self, substitutions: &HfstSymbolSubstitutions) -> &mut Self {
        self.substitute_symbol_substitutions(substitutions)
    }

    /** @brief Substitute all transitions as defined in 'substitutions'. */
    pub fn substitute_symbol_substitutions(
        &mut self,
        substitutions: &HfstSymbolSubstitutions,
    ) -> &mut Self {
        // add symbols to the global HfstTransition alphabet
        for (first, second) in substitutions.iter() {
            let _ = self.get_symbol_number(first);
            let _ = self.get_symbol_number(second);
        }

        // substitutions_[from_symbol] = to_symbol
        let mut substitutions_: Vec<u32> = Vec::new();
        let st: usize = self.coder.get_max_number() as usize + substitutions.len() + 1;
        let no_substitution = size_t_to_uint(st);

        substitutions_.resize((self.coder.get_max_number() + 1) as usize, no_substitution);
        for (first, second) in substitutions.iter() {
            let from_symbol = self.get_symbol_number(first);
            let to_symbol = self.get_symbol_number(second);
            substitutions_[from_symbol as usize] = to_symbol;
        }

        self.substitute_in_place_numbers(&substitutions_, no_substitution);

        self
    }

    pub fn substitute_symbol_pairs(
        &mut self,
        substitutions: &HfstSymbolPairSubstitutions,
    ) -> &mut Self {
        self.substitute_symbol_pair_substitutions(substitutions)
    }

    /** @brief Substitute transitions x:y -> X:Y as defined in 'substitutions'. */
    pub fn substitute_symbol_pair_substitutions(
        &mut self,
        substitutions: &HfstSymbolPairSubstitutions,
    ) -> &mut Self {
        // Convert from symbols to numbers
        let mut substitutions_: HfstNumberPairSubstitutions = BTreeMap::new();
        for (from, to) in substitutions.iter() {
            let from_transition = (
                self.get_symbol_number(&from.0),
                self.get_symbol_number(&from.1),
            );
            let to_transition = (self.get_symbol_number(&to.0), self.get_symbol_number(&to.1));
            substitutions_.insert(from_transition, to_transition);
        }

        self.substitute_in_place_number_pairs(&substitutions_);

        self
    }

    /** @brief Substitute all transitions 'sp' with a set of transitions 'sps'. */
    pub fn substitute_pair_with_set(
        &mut self,
        sp: &HfstSymbolPair,
        sps: &HfstSymbolPairSet,
    ) -> &mut Self {
        if !HfstTropicalTransducerTransitionData::is_valid_symbol(&sp.0)
            || !HfstTropicalTransducerTransitionData::is_valid_symbol(&sp.1)
        {
            crate::HFST_THROW_MESSAGE!(EmptyStringException, "HfstBasicTransducer::substitute");
        }

        for sp in sps.iter() {
            if !HfstTropicalTransducerTransitionData::is_valid_symbol(&sp.0)
                || !HfstTropicalTransducerTransitionData::is_valid_symbol(&sp.1)
            {
                crate::HFST_THROW_MESSAGE!(EmptyStringException, "HfstBasicTransducer::substitute");
            }
        }

        self.substitute_in_place_pair_set(sp, sps);

        self
    }

    /** @brief Substitute all transitions 'old_pair' with 'new_pair'. */
    pub fn substitute_pair(
        &mut self,
        old_pair: &HfstSymbolPair,
        new_pair: &HfstSymbolPair,
    ) -> &mut Self {
        if !HfstTropicalTransducerTransitionData::is_valid_symbol(&old_pair.0)
            || !HfstTropicalTransducerTransitionData::is_valid_symbol(&new_pair.0)
            || !HfstTropicalTransducerTransitionData::is_valid_symbol(&old_pair.1)
            || !HfstTropicalTransducerTransitionData::is_valid_symbol(&new_pair.1)
        {
            crate::HFST_THROW_MESSAGE!(EmptyStringException, "HfstBasicTransducer::substitute");
        }

        let mut new_pair_set: StringPairSet = BTreeSet::new();
        new_pair_set.insert(new_pair.clone());
        self.substitute_in_place_pair_set(old_pair, &new_pair_set);

        self
    }

    /** @brief Substitute all transitions with a set defined by function 'func'. */
    pub fn substitute_with_func(
        &mut self,
        func: impl Fn(&HfstSymbolPair, &mut HfstSymbolPairSet) -> bool,
    ) -> &mut Self {
        self.substitute_in_place_func(func);
        self
    }

    /** @brief Substitute transitions 'sp' with a copy of 'graph'. */
    pub fn substitute_pair_with_graph(
        &mut self,
        sp: &HfstSymbolPair,
        graph: &HfstBasicTransducer,
    ) -> &mut Self {
        if !(HfstTropicalTransducerTransitionData::is_valid_symbol(&sp.0)
            && HfstTropicalTransducerTransitionData::is_valid_symbol(&sp.1))
        {
            crate::HFST_THROW_MESSAGE!(
                EmptyStringException,
                "HfstBasicTransducer::substitute(const HfstSymbolPair&, const HfstBasicTransducer&)"
            );
        }

        // If neither symbol is known to the graph, do nothing.
        if !self.alphabet.contains(&sp.0) && !self.alphabet.contains(&sp.1) {
            return self;
        }

        let graph_ptr = graph as *const HfstBasicTransducer;
        let mut substitutions: Vec<substitution_data> = Vec::new();

        for s in 0..self.state_vector.len() {
            // The transitions that are substituted, i.e. removed.
            let mut old_indices: Vec<usize> = Vec::new();

            for i in 0..self.state_vector[s].len() {
                let data = self.state_vector[s][i].get_transition_data().clone();
                if data.get_input_symbol(&self.coder) == sp.0
                    && data.get_output_symbol(&self.coder) == sp.1
                {
                    substitutions.push(substitution_data::new(
                        s as HfstState,
                        self.state_vector[s][i].get_target_state(),
                        data.get_weight(),
                        graph_ptr,
                    ));
                    old_indices.push(i);
                }
            }
            // C++ erases collected forward iterators (UB after the first erase);
            // the evident intent is to remove all matches — done in reverse.
            for &i in old_indices.iter().rev() {
                self.state_vector[s].remove(i);
            }
        }

        for substitution in substitutions.iter() {
            self.add_substitution(substitution);
        }
        self
    }

    /* Add a copy of the substituting graph with epsilon transitions between
    states and with weight as defined in `sub`. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-substitution-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-substitution-fn]
    // [spec:hfst:def:hfst-transition-graph.add-substitution-fn]
    // [spec:hfst:sem:hfst-transition-graph.add-substitution-fn]
    pub fn add_substitution(&mut self, sub: &substitution_data) {
        // Epsilon transition to initial state of the substituting graph.
        let s = self.add_state_new();
        let epsilon_transition = HfstBasicTransition::new_symbols(
            s,
            HfstTropicalTransducerTransitionData::get_epsilon(),
            HfstTropicalTransducerTransitionData::get_epsilon(),
            sub.weight,
            self.coder_mut(),
        );
        self.add_transition(sub.origin_state, &epsilon_transition, true);

        let offset = s;

        // SAFETY-ISLAND [substitute-alias]: the substitution map's pointed-to
        // graph is mutated (`get_mut` + `harmonize`) after the pointer is stored
        // and before it is read here, so a shared `&'a` held across the `&mut` of
        // the same map fails the borrow check. The graphs are distinct from `self`
        // (aliasing self would be UB in the C++ too); read-only deref.
        let graph_ref = unsafe { &*sub.substituting_graph };
        // The substituting graph has its own coder; resolve its arc symbols
        // through *its* coding, then re-intern them into this graph's coder.
        let graph_coder = graph_ref.coder();
        let mut source_state: HfstState = 0;
        for it in graph_ref.state_vector.iter() {
            for tr_it in it.iter() {
                let data = tr_it.get_transition_data();
                let isym = data.get_input_symbol(graph_coder);
                let osym = data.get_output_symbol(graph_coder);
                let transition = HfstBasicTransition::new_symbols(
                    tr_it.get_target_state() + offset,
                    isym,
                    osym,
                    data.get_weight(),
                    self.coder_mut(),
                );
                self.add_transition(source_state + offset, &transition, true);
            }
            source_state += 1;
        }

        // Epsilon transitions from final states of the graph.
        for (k, v) in graph_ref.final_weight_map.iter() {
            let epsilon_transition = HfstBasicTransition::new_symbols(
                sub.target_state,
                HfstTropicalTransducerTransitionData::get_epsilon(),
                HfstTropicalTransducerTransitionData::get_epsilon(),
                *v,
                self.coder_mut(),
            );
            self.add_transition(*k + offset, &epsilon_transition, true);
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.weight2marker-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.weight2marker-fn]
    //
    // The C++ uses 'ostringstream <<' (default float text); Rust's '{}' differs
    // textually but round-trips with marker2weight's parse internally.
    // [spec:hfst:def:hfst-transition-graph.std.string-weight2marker-fn]
    // [spec:hfst:sem:hfst-transition-graph.std.string-weight2marker-fn]
    pub fn weight2marker(weight: f32) -> String {
        format!("@{}@", weight)
    }

    /** @brief Replace each non-zero transition weight with a '@w@' marker arc. */
    pub fn substitute_weights_with_markers(&mut self) -> &mut Self {
        let limit = self.state_vector.len();
        for state in 0..limit {
            let mut old_indices: Vec<usize> = Vec::new();
            let mut new_transitions: Vec<HfstBasicTransition> = Vec::new();

            for i in 0..self.state_vector[state].len() {
                let data = self.state_vector[state][i].get_transition_data().clone();
                if data.get_weight() != 0.0 {
                    let target = self.state_vector[state][i].get_target_state();
                    let isym = data.get_input_symbol(&self.coder);
                    let osym = data.get_output_symbol(&self.coder);
                    new_transitions.push(HfstBasicTransition::new_symbols(
                        target,
                        isym,
                        osym,
                        data.get_weight(),
                        self.coder_mut(),
                    ));
                    old_indices.push(i);
                }
            }

            // Remove the substituted transitions (stack LIFO = reverse position).
            for &i in old_indices.iter().rev() {
                self.state_vector[state].remove(i);
            }

            // Add the substituting transitions.
            for it in new_transitions.iter() {
                let new_state = self.add_state_new();
                let marker = Self::weight2marker(it.get_weight());
                let it_target = it.get_target_state();
                let it_isym = it.get_input_symbol(&self.coder);
                let it_osym = it.get_output_symbol(&self.coder);
                let marker_transition = HfstBasicTransition::new_symbols(
                    it_target,
                    marker.clone(),
                    marker,
                    0.0,
                    self.coder_mut(),
                );
                let new_transition = HfstBasicTransition::new_symbols(
                    new_state,
                    it_isym,
                    it_osym,
                    0.0,
                    self.coder_mut(),
                );
                let source_state = size_t_to_uint(state);
                self.add_transition(source_state, &new_transition, true);
                self.add_transition(new_state, &marker_transition, true);
            }
        }

        // Go through the final states (snapshot first; the C++ iterates the map
        // while inserting weight-0 finals that it then skips).
        let mut final_states_to_remove: BTreeSet<HfstState> = BTreeSet::new();
        let finals: Vec<(HfstState, f32)> = self
            .final_weight_map
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect();
        for (k, v) in finals {
            if v != 0.0 {
                let new_state = self.add_state_new();
                self.set_final_weight(new_state, &0.0);
                let marker = Self::weight2marker(v);
                let epsilon_transition = HfstBasicTransition::new_symbols(
                    new_state,
                    marker.clone(),
                    marker,
                    0.0,
                    self.coder_mut(),
                );
                self.add_transition(k, &epsilon_transition, true);
                final_states_to_remove.insert(k);
            }
        }
        for it in final_states_to_remove.iter() {
            self.final_weight_map.remove(it);
        }

        self
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.marker2weight-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.marker2weight-fn]
    // [spec:hfst:def:hfst-transition-graph.marker2weight-fn]
    // [spec:hfst:sem:hfst-transition-graph.marker2weight-fn]
    pub fn marker2weight(str: &str, weight: &mut f32) -> bool {
        if str.len() < 3 {
            return false;
        }
        let bytes = str.as_bytes();
        if bytes[0] != b'@' || bytes[str.len() - 1] != b'@' {
            return false;
        }
        let weight_string = &str[1..str.len() - 1];
        match weight_string.parse::<f32>() {
            Ok(w) => *weight = w,
            Err(_) => return false,
        }
        true
    }

    /** @brief Replace '@w@' marker arcs with transition weights. */
    pub fn substitute_markers_with_weights(&mut self) -> &mut Self {
        let limit = self.state_vector.len();
        for state in 0..limit {
            let mut old_indices: Vec<usize> = Vec::new();
            let mut new_transitions: Vec<HfstBasicTransition> = Vec::new();

            for i in 0..self.state_vector[state].len() {
                let data = self.state_vector[state][i].get_transition_data().clone();
                let isym = data.get_input_symbol(&self.coder);
                let osym = data.get_output_symbol(&self.coder);
                let mut weight: f32 = 0.0;
                if !Self::marker2weight(&isym, &mut weight)
                    && Self::marker2weight(&osym, &mut weight)
                {
                    let target = self.state_vector[state][i].get_target_state();
                    new_transitions.push(HfstBasicTransition::new_symbols(
                        target,
                        isym,
                        crate::hfst_symbol_defs::internal_epsilon.to_string(),
                        weight,
                        self.coder_mut(),
                    ));
                    old_indices.push(i);
                } else if Self::marker2weight(&isym, &mut weight)
                    && Self::marker2weight(&osym, &mut weight)
                {
                    old_indices.push(i);
                }
            }

            for &i in old_indices.iter().rev() {
                self.state_vector[state].remove(i);
            }
            for new_transition in new_transitions.iter() {
                self.state_vector[state].push(new_transition.clone());
            }
        }

        // Remove weight-marker symbols from the alphabet.
        let mut weight_markers: Vec<HfstSymbol> = Vec::new();
        for it in self.alphabet.iter() {
            let mut foo: f32 = 0.0;
            if Self::marker2weight(it, &mut foo) {
                weight_markers.push(it.clone());
            }
        }
        for it in weight_markers.iter() {
            self.alphabet.remove(it);
        }

        self
    }

    // aliases
    pub fn substitute_symbol_pair(
        &mut self,
        old_symbol_pair: &StringPair,
        new_symbol_pair: &StringPair,
    ) -> &mut Self {
        self.substitute_pair(old_symbol_pair, new_symbol_pair)
    }

    pub fn substitute_symbol_pair_with_set(
        &mut self,
        old_symbol_pair: &StringPair,
        new_symbol_pair_set: &StringPairSet,
    ) -> &mut Self {
        self.substitute_pair_with_set(old_symbol_pair, new_symbol_pair_set)
    }

    pub fn substitute_symbol_pair_with_transducer(
        &mut self,
        symbol_pair: &StringPair,
        transducer: &HfstBasicTransducer,
    ) -> &mut Self {
        self.substitute_pair_with_graph(symbol_pair, transducer)
    }

    // --- Insert freely ---

    /** @brief Insert freely any number of 'symbol_pair' with weight 'weight'. */
    pub fn insert_freely_pair(
        &mut self,
        symbol_pair: &HfstSymbolPair,
        weight: WeightType,
    ) -> &mut Self {
        if !(HfstTropicalTransducerTransitionData::is_valid_symbol(&symbol_pair.0)
            && HfstTropicalTransducerTransitionData::is_valid_symbol(&symbol_pair.1))
        {
            crate::HFST_THROW_MESSAGE!(
                EmptyStringException,
                "HfstBasicTransducer::insert_freely(const HfstSymbolPair&, W)"
            );
        }

        self.alphabet.insert(symbol_pair.0.clone());
        self.alphabet.insert(symbol_pair.1.clone());

        for s in 0..self.state_vector.len() {
            // self-loop on each state
            let tr = HfstBasicTransition::new_symbols(
                s as HfstState,
                symbol_pair.0.clone(),
                symbol_pair.1.clone(),
                weight,
                self.coder_mut(),
            );
            self.state_vector[s].push(tr);
        }
        self
    }

    /** @brief Insert freely any of the pairs in 'symbol_pairs'. */
    pub fn insert_freely_set(
        &mut self,
        symbol_pairs: &HfstSymbolPairSet,
        weight: WeightType,
    ) -> &mut Self {
        for symbol_pair in symbol_pairs.iter() {
            if !(HfstTropicalTransducerTransitionData::is_valid_symbol(&symbol_pair.0)
                && HfstTropicalTransducerTransitionData::is_valid_symbol(&symbol_pair.1))
            {
                crate::HFST_THROW_MESSAGE!(
                    EmptyStringException,
                    "HfstBasicTransducer::insert_freely(const HfstSymbolPairSet&, W)"
                );
            }
            self.alphabet.insert(symbol_pair.0.clone());
            self.alphabet.insert(symbol_pair.1.clone());
        }

        for s in 0..self.state_vector.len() {
            for symbol_pair in symbol_pairs.iter() {
                let tr = HfstBasicTransition::new_symbols(
                    s as HfstState,
                    symbol_pair.0.clone(),
                    symbol_pair.1.clone(),
                    weight,
                    self.coder_mut(),
                );
                self.state_vector[s].push(tr);
            }
        }
        self
    }

    /** @brief Insert freely any number of 'graph' in this graph. */
    pub fn insert_freely_graph(&mut self, graph: &HfstBasicTransducer) -> &mut Self {
        let marker_this = HfstTropicalTransducerTransitionData::get_marker(&self.alphabet);
        let marker_graph = HfstTropicalTransducerTransitionData::get_marker(&self.alphabet);
        let mut marker = marker_this;
        if marker_graph > marker {
            marker = marker_graph;
        }

        // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.marker-pair-fn]
        // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.marker-pair-fn]
        // [spec:hfst:def:hfst-transition-graph.marker-pair-fn]
        // [spec:hfst:sem:hfst-transition-graph.marker-pair-fn]
        let marker_pair = (marker.clone(), marker.clone());
        self.insert_freely_pair(&marker_pair, 0.0);
        self.substitute_pair_with_graph(&marker_pair, graph);
        self.alphabet.remove(&marker); // (C++ flags this line as needing a fix)

        self
    }

    // --- Disjunction ---

    /* Disjunct the transition of path 'spv' pointed by 'it' to state 's'. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.disjunct-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.disjunct-fn]
    // [spec:hfst:def:hfst-transition-graph.disjunct-fn]
    // [spec:hfst:sem:hfst-transition-graph.disjunct-fn]
    pub fn disjunct(&mut self, spv: &StringPairVector, it: &mut usize, s: HfstState) -> HfstState {
        let mut current_state = s;
        while *it != spv.len() {
            // C++ copies the transition vector before searching it.
            let tr = self.state_vector[current_state as usize].clone();
            let mut transition_found = false;
            let mut next_state: HfstState = 0;

            for tr_it in tr.iter() {
                let data = tr_it.get_transition_data();
                if data.get_input_symbol(&self.coder) == spv[*it].0
                    && data.get_output_symbol(&self.coder) == spv[*it].1
                {
                    transition_found = true;
                    next_state = tr_it.get_target_state();
                    break;
                }
            }

            if !transition_found {
                next_state = self.add_state_new();
                let transition = HfstBasicTransition::new_symbols(
                    next_state,
                    spv[*it].0.clone(),
                    spv[*it].1.clone(),
                    0.0,
                    self.coder_mut(),
                );
                self.add_transition(current_state, &transition, true);
            }

            *it += 1;
            current_state = next_state;
        }
        current_state
    }

    /** @brief Disjunct this graph with a one-path graph defined by 'spv'. */
    pub fn disjunct_path(&mut self, spv: &StringPairVector, weight: WeightType) -> &mut Self {
        let mut it: usize = 0;
        let final_state = self.disjunct(spv, &mut it, Self::INITIAL_STATE);

        if self.is_final_state(final_state) {
            let old_weight = self.get_final_weight(final_state);
            if old_weight < weight {
                return self; // smaller-weight path remains
            }
        }
        self.set_final_weight(final_state, &weight);
        self
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-special-symbol-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-special-symbol-fn]
    // [spec:hfst:def:hfst-transition-graph.is-special-symbol-fn]
    // [spec:hfst:sem:hfst-transition-graph.is-special-symbol-fn]
    pub fn is_special_symbol(symbol: &str) -> bool {
        if symbol.len() < 2 {
            return false;
        }
        let bytes = symbol.as_bytes();
        if bytes[0] == b'@' && bytes[1] == b'_' {
            return true;
        }
        false
    }

    /** @brief Make the graph complete (add a failure state). */
    pub fn complete(&mut self) -> &mut Self {
        let failure_state = self.add_state_new();
        let mut current_state: HfstState = 0;

        for s in 0..self.state_vector.len() {
            let mut symbols_present: BTreeSet<HfstSymbol> = BTreeSet::new();

            for i in 0..self.state_vector[s].len() {
                let data = self.state_vector[s][i].get_transition_data().clone();
                let isym = data.get_input_symbol(&self.coder);
                let osym = data.get_output_symbol(&self.coder);
                if isym != osym {
                    crate::HFST_THROW!(TransducersAreNotAutomataException);
                }
                symbols_present.insert(isym);
            }

            let alpha_snapshot: Vec<HfstSymbol> = self.alphabet.iter().cloned().collect();
            for alpha_it in alpha_snapshot.iter() {
                if !symbols_present.contains(alpha_it) && !Self::is_special_symbol(alpha_it) {
                    let tr = HfstBasicTransition::new_symbols(
                        failure_state,
                        alpha_it.clone(),
                        alpha_it.clone(),
                        0.0,
                        self.coder_mut(),
                    );
                    self.add_transition(current_state, &tr, true);
                }
            }
            current_state += 1;
        }
        self
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-flags-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-flags-fn]
    // [spec:hfst:def:hfst-transition-graph.get-flags-fn]
    // [spec:hfst:sem:hfst-transition-graph.get-flags-fn]
    pub fn get_flags(&self) -> StringSet {
        let mut flags = StringSet::new();
        for it in self.alphabet.iter() {
            if FdOperation::is_diacritic(it) {
                flags.insert(it.clone());
            }
        }
        flags
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.purge-symbol-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.purge-symbol-fn]
    // [spec:hfst:def:hfst-transition-graph.purge-symbol-fn]
    // [spec:hfst:sem:hfst-transition-graph.purge-symbol-fn]
    pub fn purge_symbol(symbol: &str, flag: &str) -> bool {
        if !FdOperation::is_diacritic(symbol) {
            return false;
        }
        if flag.is_empty() {
            return true;
        } else if FdOperation::get_feature(symbol) == flag {
            return true;
        }
        false
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.flag-purge-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.flag-purge-fn]
    // [spec:hfst:def:hfst-transition-graph.flag-purge-fn]
    // [spec:hfst:sem:hfst-transition-graph.flag-purge-fn]
    pub fn flag_purge(&mut self, flag: &str) {
        // (1) Go through all states and transitions
        for s in 0..self.state_vector.len() {
            for i in 0..self.state_vector[s].len() {
                let isym = self.state_vector[s][i].get_input_symbol(&self.coder);
                let osym = self.state_vector[s][i].get_output_symbol(&self.coder);
                if Self::purge_symbol(&isym, flag) || Self::purge_symbol(&osym, flag) {
                    let target = self.state_vector[s][i].get_target_state();
                    let weight = self.state_vector[s][i].get_weight();
                    let tr = HfstBasicTransition::new_symbols(
                        target,
                        "@_EPSILON_SYMBOL_@".to_string(),
                        "@_EPSILON_SYMBOL_@".to_string(),
                        weight,
                        self.coder_mut(),
                    );
                    self.state_vector[s][i] = tr;
                }
            }
        }
        // (2) Go through the alphabet
        let mut extra_symbols = StringSet::new();
        for it in self.alphabet.iter() {
            if Self::purge_symbol(it, flag) {
                extra_symbols.insert(it.clone());
            }
        }
        self.remove_symbols_from_alphabet(&extra_symbols);
    }

    // --- Harmonization ---

    /** @brief Harmonize this graph and 'another' (expand unknown/identity). */
    pub fn harmonize(&mut self, another: &mut HfstBasicTransducer) -> &mut Self {
        let _foo = HarmonizeUnknownAndIdentitySymbols::new(self, another);
        self
    }

    /** @brief Substitute symbols with transducers as defined in 'substitution_map'. */
    pub fn substitute_subst_map(
        &mut self,
        substitution_map: &mut SubstMap,
        harmonize: bool,
    ) -> &mut Self {
        let mut symbol_found = false;
        for (first, _) in substitution_map.iter() {
            if !HfstTropicalTransducerTransitionData::is_valid_symbol(first) {
                crate::HFST_THROW_MESSAGE!(
                    EmptyStringException,
                    "HfstBasicTransducer::substitute (const std::map<HfstSymbol, HfstBasicTransducer> &)"
                );
            }
            if !symbol_found && self.alphabet.contains(first) {
                symbol_found = true;
            }
        }

        // If none of the symbols is known to the graph, do nothing.
        if !symbol_found {
            return self;
        }

        let mut substitutions_performed_for_symbols: StringSet = BTreeSet::new();
        let mut substitutions: Vec<substitution_data> = Vec::new();

        for s in 0..self.state_vector.len() {
            let mut old_indices: Vec<usize> = Vec::new();

            for j in 0..self.state_vector[s].len() {
                let istr = self.state_vector[s][j].get_input_symbol(&self.coder);
                let ostr = self.state_vector[s][j].get_output_symbol(&self.coder);
                let map_in_found = substitution_map.contains_key(&istr);
                let map_out_found = substitution_map.contains_key(&ostr);

                if !map_in_found && !map_out_found {
                    // nothing
                } else if istr != ostr {
                    let msg = "symbol to be substituted must not occur only on one side of \
                               transition"
                        .to_string();
                    crate::HFST_THROW_MESSAGE!(HfstException, msg);
                } else {
                    let target = self.state_vector[s][j].get_target_state();
                    let weight = self.state_vector[s][j].get_weight();
                    // raw pointer into the map value (the substituting graph)
                    let graph_ptr =
                        substitution_map.get(&istr).unwrap() as *const HfstBasicTransducer;
                    substitutions.push(substitution_data::new(
                        s as HfstState,
                        target,
                        weight,
                        graph_ptr,
                    ));
                    old_indices.push(j);
                    substitutions_performed_for_symbols.insert(istr.clone());
                }
            }
            for &j in old_indices.iter().rev() {
                self.state_vector[s].remove(j);
            }
        }

        // Remove all symbols that were substituted.
        for sym_it in substitutions_performed_for_symbols.iter() {
            if sym_it != "@_EPSILON_SYMBOL_@"
                && sym_it != "@_UNKNOWN_SYMBOL_@"
                && sym_it != "@_IDENTITY_SYMBOL_@"
            {
                self.remove_symbol_from_alphabet(sym_it);
            }
        }

        // Harmonize the resulting and the substituting graphs, if needed.
        if harmonize {
            for sym_it in substitutions_performed_for_symbols.iter() {
                let graph = substitution_map.get_mut(sym_it).unwrap();
                self.harmonize(graph);
            }
        }

        // Add the substitutions (reads the now-harmonized graphs via raw ptr).
        for substitution in substitutions.iter() {
            self.add_substitution(substitution);
        }
        self
    }

    // --- Topological sort / path sizes ---

    /* Get a topological (maximum/minimum distance) sort of this graph. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.topsort-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.topsort-fn]
    // [spec:hfst:def:hfst-transition-graph.std.vector-std.set-hfst-state-topsort-fn]
    // [spec:hfst:sem:hfst-transition-graph.std.vector-std.set-hfst-state-topsort-fn]
    pub fn topsort(&self, dist: SortDistance) -> Vec<BTreeSet<HfstState>> {
        let mut current_distance: u32 = 0; // topological distance
        let mut top_sort = TopologicalSort::new();

        let st = self.state_vector.len();
        if st == 0 {
            return Vec::new();
        }
        let st = st - 1;
        let biggest_state_number = size_t_to_uint(st);
        top_sort.set_biggest_state_number(biggest_state_number);

        top_sort.set_state_at_distance(0, current_distance, dist == SortDistance::MaximumDistance);
        let mut new_states_found; // end condition for the do-while loop

        loop {
            new_states_found = false;
            let mut new_states: BTreeSet<HfstState> = BTreeSet::new();

            // states accessible from the current set of states
            let states = top_sort.get_states_at_distance(current_distance).clone();
            for state in states.iter() {
                let transitions = &self.state_vector[*state as usize];
                for transition in transitions.iter() {
                    new_states_found = true;
                    new_states.insert(transition.get_target_state());
                }
            }

            for new_state in new_states.iter() {
                top_sort.set_state_at_distance(
                    *new_state,
                    current_distance + 1,
                    dist == SortDistance::MaximumDistance,
                );
            }
            current_distance += 1;

            if !new_states_found {
                break;
            }
        }

        top_sort.states_at_distance
    }

    /** The length of the longest string accepted by this graph, or -1. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.longest-path-size-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.longest-path-size-fn]
    // [spec:hfst:def:hfst-transition-graph.longest-path-size-fn]
    // [spec:hfst:sem:hfst-transition-graph.longest-path-size-fn]
    pub fn longest_path_size(&self) -> i32 {
        let states_sorted = self.topsort(SortDistance::MaximumDistance);
        let st = states_sorted.len();
        if st > 0 {
            for distance in (0..=size_t_to_int(st - 1)).rev() {
                let states = &states_sorted[distance as usize];
                for state in states.iter() {
                    if self.is_final_state(*state) {
                        return distance;
                    }
                }
            }
        }
        -1
    }

    /** The lengths of strings accepted by this graph, in descending order. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.path-sizes-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.path-sizes-fn]
    // [spec:hfst:def:hfst-transition-graph.std.vector-unsigned-int-path-sizes-fn]
    // [spec:hfst:sem:hfst-transition-graph.std.vector-unsigned-int-path-sizes-fn]
    pub fn path_sizes(&self) -> Vec<u32> {
        let mut result: Vec<u32> = Vec::new();
        let states_sorted = self.topsort(SortDistance::MinimumDistance);
        let st = states_sorted.len();
        if st > 0 {
            for distance in (0..=size_t_to_int(st - 1)).rev() {
                let states = &states_sorted[distance as usize];
                for state in states.iter() {
                    if self.is_final_state(*state) {
                        result.push(distance as u32);
                        break;
                    }
                }
            }
        }
        result
    }

    // --- Cycle detection ---

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.has-negative-epsilon-cycles-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.has-negative-epsilon-cycles-fn]
    pub fn has_negative_epsilon_cycles_recursive(
        &self,
        state: HfstState,
        total_weight: f32,
        state_weights: &mut BTreeMap<HfstState, f32>,
    ) -> bool {
        if let Some(w) = state_weights.get(&state) {
            // cycle detected
            if total_weight - *w < 0.0 {
                return true; // cycle with negative weight
            }
            return false; // cycle with positive weight
        }
        state_weights.insert(state, total_weight);

        let transitions = self.index(state);
        for transition in transitions.iter() {
            if is_epsilon(&transition.get_input_symbol(&self.coder))
                && is_epsilon(&transition.get_output_symbol(&self.coder))
                && self.has_negative_epsilon_cycles_recursive(
                    transition.get_target_state(),
                    total_weight + transition.get_weight(),
                    state_weights,
                )
            {
                return true;
            }
        }
        state_weights.remove(&state);
        false
    }

    // [spec:hfst:def:hfst-transition-graph.has-negative-epsilon-cycles-fn]
    // [spec:hfst:sem:hfst-transition-graph.has-negative-epsilon-cycles-fn]
    pub fn has_negative_epsilon_cycles(&self) -> bool {
        let mut has_negative_epsilon_transitions = false;
        for it in self.state_vector.iter() {
            for tr_it in it.iter() {
                if is_epsilon(&tr_it.get_input_symbol(&self.coder))
                    && is_epsilon(&tr_it.get_output_symbol(&self.coder))
                    && tr_it.get_weight() < 0.0
                {
                    has_negative_epsilon_transitions = true;
                    break;
                }
            }
        }
        if !has_negative_epsilon_transitions {
            return false;
        }

        let mut state_weights: BTreeMap<HfstState, f32> = BTreeMap::new();
        for state in Self::INITIAL_STATE..(self.get_max_state() + 1) {
            if self.has_negative_epsilon_cycles_recursive(state, 0.0, &mut state_weights) {
                return true;
            }
        }
        false
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-infinitely-ambiguous-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-infinitely-ambiguous-fn]
    pub fn is_infinitely_ambiguous_recursive(
        &self,
        state: HfstState,
        epsilon_path_states: &mut BTreeSet<HfstState>,
        states_handled: &mut Vec<u32>,
    ) -> bool {
        if states_handled[state as usize] != 0 {
            return false;
        }

        let transitions = self.index(state);
        for transition in transitions.iter() {
            // Diacritics are also treated as epsilons (may yield false positives).
            if is_epsilon(&transition.get_input_symbol(&self.coder))
                || FdOperation::is_diacritic(&transition.get_input_symbol(&self.coder))
            {
                epsilon_path_states.insert(state);
                if epsilon_path_states.contains(&transition.get_target_state()) {
                    return true;
                }
                if self.is_infinitely_ambiguous_recursive(
                    transition.get_target_state(),
                    epsilon_path_states,
                    states_handled,
                ) {
                    return true;
                }
                epsilon_path_states.remove(&state);
            }
        }
        states_handled[state as usize] = 1;
        false
    }

    // [spec:hfst:def:hfst-transition-graph.is-infinitely-ambiguous-fn]
    // [spec:hfst:sem:hfst-transition-graph.is-infinitely-ambiguous-fn]
    pub fn is_infinitely_ambiguous(&self) -> bool {
        let mut epsilon_path_states: BTreeSet<HfstState> = BTreeSet::new();
        let max_state = self.get_max_state();
        let mut states_handled: Vec<u32> = vec![0; (max_state + 1) as usize];

        for state in Self::INITIAL_STATE..(max_state + 1) {
            if self.is_infinitely_ambiguous_recursive(
                state,
                &mut epsilon_path_states,
                &mut states_handled,
            ) {
                return true;
            }
        }
        false
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-possible-flag-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-possible-flag-fn]
    // [spec:hfst:def:hfst-transition-graph.is-possible-flag-fn]
    // [spec:hfst:sem:hfst-transition-graph.is-possible-flag-fn]
    pub fn is_possible_flag(symbol: String, fds: &mut StringVector, obey_flags: bool) -> bool {
        if FdOperation::is_diacritic(&symbol) {
            let mut fd_t = FlagDiacriticTable::new();
            fds.push(symbol);
            if (!obey_flags) || fd_t.is_valid_string(fds) {
                return true;
            } else {
                fds.pop();
                return false;
            }
        }
        false
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-lookup-infinitely-ambiguous-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-lookup-infinitely-ambiguous-fn]
    // [spec:hfst:def:hfst-transition-graph.is-lookup-infinitely-ambiguous-fn]
    // [spec:hfst:sem:hfst-transition-graph.is-lookup-infinitely-ambiguous-fn]
    pub fn is_lookup_infinitely_ambiguous_recursive(
        &self,
        s: &HfstOneLevelPath,
        index: &mut u32,
        state: HfstState,
        epsilon_path_states: &mut BTreeSet<HfstState>,
        fds: &mut StringVector,
        obey_flags: bool,
    ) -> bool {
        // Whether the end of the lookup path s has been reached
        let mut only_epsilons = false;
        if s.second.len() as u32 == *index {
            only_epsilons = true;
        }

        let transitions = self.index(state);
        for transition in transitions.iter() {
            // CASE 1: input epsilons (and flags) do not consume a path symbol.
            let in_sym = transition.get_input_symbol(&self.coder);
            let possible_flag = Self::is_possible_flag(in_sym.clone(), fds, obey_flags);
            if is_epsilon(&in_sym) || possible_flag {
                epsilon_path_states.insert(state);
                if epsilon_path_states.contains(&transition.get_target_state()) {
                    return true;
                }
                if self.is_lookup_infinitely_ambiguous_recursive(
                    s,
                    index,
                    transition.get_target_state(),
                    epsilon_path_states,
                    fds,
                    obey_flags,
                ) {
                    return true;
                }
                epsilon_path_states.remove(&state);
                if possible_flag {
                    fds.pop();
                }
            }
            // CASE 2: other input symbols consume a path symbol.
            else if !only_epsilons {
                let mut continu = false;
                if in_sym == s.second[*index as usize] {
                    continu = true;
                } else if (in_sym == "@_UNKNOWN_SYMBOL_@" || in_sym == "@_IDENTITY_SYMBOL_@")
                    && !self.alphabet.contains(&s.second[*index as usize])
                {
                    continu = true;
                }

                if continu {
                    *index += 1; // consume an input symbol
                    let mut empty_set: BTreeSet<HfstState> = BTreeSet::new();
                    if self.is_lookup_infinitely_ambiguous_recursive(
                        s,
                        index,
                        transition.get_target_state(),
                        &mut empty_set,
                        fds,
                        obey_flags,
                    ) {
                        return true;
                    }
                    *index -= 1; // add the input symbol back
                }
            }
        }
        false
    }

    pub fn is_lookup_infinitely_ambiguous_path(
        &self,
        s: &HfstOneLevelPath,
        obey_flags: bool,
    ) -> bool {
        let mut epsilon_path_states: BTreeSet<HfstState> = BTreeSet::new();
        epsilon_path_states.insert(0);
        let mut index: u32 = 0;
        let mut fds: StringVector = Vec::new();

        self.is_lookup_infinitely_ambiguous_recursive(
            s,
            &mut index,
            Self::INITIAL_STATE,
            &mut epsilon_path_states,
            &mut fds,
            obey_flags,
        )
    }

    pub fn is_lookup_infinitely_ambiguous_string_vector(
        &self,
        s: &StringVector,
        obey_flags: bool,
    ) -> bool {
        let mut epsilon_path_states: BTreeSet<HfstState> = BTreeSet::new();
        epsilon_path_states.insert(0);
        let mut index: u32 = 0;
        let path = HfstOneLevelPath {
            first: 0.0,
            second: s.clone(),
        };
        let mut fds: StringVector = Vec::new();

        self.is_lookup_infinitely_ambiguous_recursive(
            &path,
            &mut index,
            Self::INITIAL_STATE,
            &mut epsilon_path_states,
            &mut fds,
            obey_flags,
        )
    }

    // --- Lookup ---

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.push-back-to-two-level-path-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.push-back-to-two-level-path-fn]
    // [spec:hfst:def:hfst-transition-graph.push-back-to-two-level-path-fn]
    // [spec:hfst:sem:hfst-transition-graph.push-back-to-two-level-path-fn]
    pub fn push_back_to_two_level_path(
        path: &mut HfstTwoLevelPath,
        sp: &StringPair,
        weight: f32,
        fds_so_far: Option<&mut StringVector>,
    ) {
        path.second.push(sp.clone());
        path.first += weight;
        if let Some(fds) = fds_so_far {
            if FdOperation::is_diacritic(&sp.0) {
                fds.push(sp.0.clone());
            }
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.pop-back-from-two-level-path-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.pop-back-from-two-level-path-fn]
    // [spec:hfst:def:hfst-transition-graph.pop-back-from-two-level-path-fn]
    // [spec:hfst:sem:hfst-transition-graph.pop-back-from-two-level-path-fn]
    pub fn pop_back_from_two_level_path(
        path: &mut HfstTwoLevelPath,
        weight: f32,
        fds_so_far: Option<&mut StringVector>,
    ) {
        if let Some(fds) = fds_so_far {
            let sp = path.second.last().unwrap().clone();
            if FdOperation::is_diacritic(&sp.0) {
                fds.pop();
            }
        }
        path.second.pop();
        path.first -= weight;
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-to-results-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-to-results-fn]
    // [spec:hfst:def:hfst-transition-graph.add-to-results-fn]
    // [spec:hfst:sem:hfst-transition-graph.add-to-results-fn]
    pub fn add_to_results(
        results: &mut HfstTwoLevelPaths,
        path_so_far: &mut HfstTwoLevelPath,
        final_weight: f32,
        max_weight: Option<&f32>,
    ) {
        path_so_far.first += final_weight;

        match max_weight {
            None => {
                results.insert(path_so_far.clone());
            }
            Some(mw) => {
                if !(path_so_far.first > *mw) {
                    results.insert(path_so_far.clone());
                }
            }
        }
        path_so_far.first -= final_weight;
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-possible-transition-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-possible-transition-fn]
    // [spec:hfst:def:hfst-transition-graph.is-possible-transition-fn]
    // [spec:hfst:sem:hfst-transition-graph.is-possible-transition-fn]
    pub fn is_possible_transition(
        transition: &HfstBasicTransition,
        lookup_path: &StringVector,
        lookup_index: u32,
        alphabet: &StringSet,
        input_symbol_consumed: &mut bool,
        fds_so_far: Option<&mut StringVector>,
        coder: &SymbolCoder,
    ) -> bool {
        let isymbol = transition.get_input_symbol(coder);

        // If we are not at the end of lookup_path,
        if !(lookup_index == lookup_path.len() as u32) {
            if isymbol == lookup_path[lookup_index as usize]
                || ((is_identity(&isymbol) || is_unknown(&isymbol))
                    && !alphabet.contains(&lookup_path[lookup_index as usize]))
            {
                *input_symbol_consumed = true;
                return true;
            }
        }
        // Epsilons and flag diacritics can always be taken.
        if is_epsilon(&isymbol) {
            *input_symbol_consumed = false;
            return true;
        }
        if FdOperation::is_diacritic(&isymbol) {
            match fds_so_far {
                None => {
                    *input_symbol_consumed = false;
                    return true;
                }
                Some(fds) => {
                    let mut fd_t = FlagDiacriticTable::new();
                    fds.push(isymbol.clone());
                    let valid = fd_t.is_valid_string(fds);
                    fds.pop();
                    if valid {
                        *input_symbol_consumed = false;
                        return true;
                    }
                }
            }
        }
        false
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.lookup-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.lookup-fn]
    pub fn lookup_recursive(
        &self,
        lookup_path: &StringVector,
        results: &mut HfstTwoLevelPaths,
        state: HfstState,
        mut lookup_index: u32,
        path_so_far: &mut HfstTwoLevelPath,
        alphabet: &StringSet,
        mut eh: HfstEpsilonHandler,
        max_epsilon_cycles: usize,
        max_weight: Option<&f32>,
        max_number: i32,
        mut flag_diacritic_path: Option<&mut StringVector>,
    ) {
        // Check the input-epsilon-cycle, weight and result-count limits.
        if !eh.can_continue(state) {
            return;
        }
        if let Some(mw) = max_weight {
            if path_so_far.first > *mw {
                return;
            }
        }
        if max_number >= 0 && (max_number as usize) <= results.len() {
            return;
        }

        // At the end of lookup_path and in a final state -> a valid result.
        if lookup_index == lookup_path.len() as u32 && self.is_final_state(state) {
            Self::add_to_results(
                results,
                path_so_far,
                self.get_final_weight(state),
                max_weight,
            );
        }

        let transitions = self.index(state);
        for transition in transitions.iter() {
            let mut input_symbol_consumed = false;
            if Self::is_possible_transition(
                transition,
                lookup_path,
                lookup_index,
                alphabet,
                &mut input_symbol_consumed,
                flag_diacritic_path.as_mut().map(|r| &mut **r),
                &self.coder,
            ) {
                let istr;
                let ostr;
                let tr_isym = transition.get_input_symbol(&self.coder);
                // identity symbol is replaced with the lookup symbol
                if is_identity(&tr_isym) {
                    istr = lookup_path[lookup_index as usize].clone();
                    ostr = istr.clone();
                } else {
                    if is_unknown(&tr_isym) {
                        istr = lookup_path[lookup_index as usize].clone();
                    } else {
                        istr = tr_isym;
                    }
                    ostr = transition.get_output_symbol(&self.coder);
                }

                Self::push_back_to_two_level_path(
                    path_so_far,
                    &(istr, ostr),
                    transition.get_weight(),
                    flag_diacritic_path.as_mut().map(|r| &mut **r),
                );

                if input_symbol_consumed {
                    lookup_index += 1;
                    let ehp = HfstEpsilonHandler::new(max_epsilon_cycles);
                    self.lookup_recursive(
                        lookup_path,
                        results,
                        transition.get_target_state(),
                        lookup_index,
                        path_so_far,
                        alphabet,
                        ehp,
                        max_epsilon_cycles,
                        max_weight,
                        max_number,
                        flag_diacritic_path.as_mut().map(|r| &mut **r),
                    );
                    lookup_index -= 1;
                } else {
                    eh.push_back(state);
                    self.lookup_recursive(
                        lookup_path,
                        results,
                        transition.get_target_state(),
                        lookup_index,
                        path_so_far,
                        alphabet,
                        eh.clone(),
                        max_epsilon_cycles,
                        max_weight,
                        max_number,
                        flag_diacritic_path.as_mut().map(|r| &mut **r),
                    );
                }

                Self::pop_back_from_two_level_path(
                    path_so_far,
                    transition.get_weight(),
                    flag_diacritic_path.as_mut().map(|r| &mut **r),
                );
            }
        }
    }

    // --- compile-replace regexp paths ---

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.check-regexp-state-for-cycle-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.check-regexp-state-for-cycle-fn]
    // [spec:hfst:def:hfst-transition-graph.check-regexp-state-for-cycle-fn]
    // [spec:hfst:sem:hfst-transition-graph.check-regexp-state-for-cycle-fn]
    pub fn check_regexp_state_for_cycle(s: HfstState, states_visited: &BTreeSet<HfstState>) {
        if states_visited.contains(&s) {
            panic!("error: loop detected inside compile-replace regular expression");
        }
    }

    // Returns whether tr is "^]":"^]". Throws (panics) if tr is not allowed.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.check-regexp-transition-end-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.check-regexp-transition-end-fn]
    // [spec:hfst:def:hfst-transition-graph.check-regexp-transition-end-fn]
    // [spec:hfst:sem:hfst-transition-graph.check-regexp-transition-end-fn]
    pub fn check_regexp_transition_end(
        tr: &HfstBasicTransition,
        input_side: bool,
        coder: &SymbolCoder,
    ) -> bool {
        let istr = tr.get_input_symbol(coder);
        let ostr = tr.get_output_symbol(coder);

        if input_side && is_epsilon(&istr) {
        } else if !input_side && is_epsilon(&ostr) {
        } else if (input_side && Self::is_special_symbol(&istr))
            || (!input_side && Self::is_special_symbol(&ostr))
        {
            panic!("error: special symbol detected in compile-replace regular expression");
        } else {
        }

        if (input_side && istr == "^[") || (!input_side && ostr == "^[") {
            panic!("error: ^[ detected inside compile-replace regular expression");
        }
        if (input_side && istr == "^]") || (!input_side && ostr == "^]") {
            return true;
        }
        false
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-regexp-paths-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-regexp-paths-fn]
    // [spec:hfst:def:hfst-transition-graph.find-regexp-paths-fn]
    // [spec:hfst:sem:hfst-transition-graph.find-regexp-paths-fn]
    // [spec:hfst:def:hfst-transition-graph.void-find-regexp-paths-fn]
    // [spec:hfst:sem:hfst-transition-graph.void-find-regexp-paths-fn]
    pub fn find_regexp_paths(
        &self,
        s: HfstState,
        states_visited: &mut BTreeSet<HfstState>,
        path: &mut Vec<(String, String)>,
        full_paths: &mut HfstReplacements,
        input_side: bool,
    ) {
        // no cycles allowed inside "^[" and "^]"
        Self::check_regexp_state_for_cycle(s, states_visited);
        states_visited.insert(s);

        let transitions = self.index(s);
        for transition in transitions.iter() {
            // closing bracket
            if Self::check_regexp_transition_end(transition, input_side, &self.coder) {
                // cannot lead to a state already visited
                Self::check_regexp_state_for_cycle(transition.get_target_state(), states_visited);
                path.push((
                    transition.get_input_symbol(&self.coder),
                    transition.get_output_symbol(&self.coder),
                ));
                full_paths.push((transition.get_target_state(), path.clone()));
                path.pop();
            } else {
                path.push((
                    transition.get_input_symbol(&self.coder),
                    transition.get_output_symbol(&self.coder),
                ));
                self.find_regexp_paths(
                    transition.get_target_state(),
                    states_visited,
                    path,
                    full_paths,
                    input_side,
                );
                path.pop();
            }
        }
        states_visited.remove(&s);
    }

    pub fn find_regexp_paths_driver(
        &self,
        s: HfstState,
        full_paths: &mut HfstReplacements,
        input_side: bool,
    ) {
        let transitions = self.index(s);
        for transition in transitions.iter() {
            let istr = transition.get_input_symbol(&self.coder);
            let ostr = transition.get_output_symbol(&self.coder);
            if (input_side && istr == "^[") || (!input_side && ostr == "^[") {
                let mut states_visited: BTreeSet<HfstState> = BTreeSet::new();
                states_visited.insert(s);
                let mut path: Vec<(String, String)> = Vec::new();
                path.push((istr.clone(), ostr.clone()));
                self.find_regexp_paths(
                    transition.get_target_state(),
                    &mut states_visited,
                    &mut path,
                    full_paths,
                    input_side,
                );
            }
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-replacements-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-replacements-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst-replacements-map-find-replacements-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst-replacements-map-find-replacements-fn]
    pub fn find_replacements(&self, input_side: bool) -> HfstReplacementsMap {
        let mut replacements = HfstReplacementsMap::new();
        let mut state: u32 = 0;
        for _it in self.state_vector.iter() {
            let mut full_paths: HfstReplacements = Vec::new();
            self.find_regexp_paths_driver(state, &mut full_paths, input_side);
            if full_paths.len() > 0 {
                replacements.insert(state, full_paths);
            }
            state += 1;
        }
        replacements
    }

    // Attach a copy of 'graph' between states 'state1' and 'state2' with epsilon
    // transitions.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.insert-transducer-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.insert-transducer-fn]
    // [spec:hfst:def:hfst-transition-graph.insert-transducer-fn]
    // [spec:hfst:sem:hfst-transition-graph.insert-transducer-fn]
    pub fn insert_transducer(
        &mut self,
        state1: HfstState,
        state2: HfstState,
        graph: &HfstBasicTransducer,
    ) {
        let offset = self.add_state_new();
        // 'graph' has its own coder; resolve its arc symbols through *its* coding,
        // then re-intern them into this graph's coder.
        let graph_coder = graph.coder();
        let mut source_state: u32 = 0;
        for it in graph.state_vector.iter() {
            for tr_it in it.iter() {
                let data = tr_it.get_transition_data();
                let isym = data.get_input_symbol(graph_coder);
                let osym = data.get_output_symbol(graph_coder);
                let transition = HfstBasicTransition::new_symbols(
                    tr_it.get_target_state() + offset,
                    isym,
                    osym,
                    data.get_weight(),
                    self.coder_mut(),
                );
                self.add_transition(source_state + offset, &transition, true);
            }
            source_state += 1;
        }

        // Epsilon transitions from final states of 'graph'.
        let finals: Vec<(HfstState, f32)> = graph
            .final_weight_map
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect();
        for (k, v) in finals {
            let epsilon_transition = HfstBasicTransition::new_symbols(
                state2,
                HfstTropicalTransducerTransitionData::get_epsilon(),
                HfstTropicalTransducerTransitionData::get_epsilon(),
                v,
                self.coder_mut(),
            );
            self.add_transition(k + offset, &epsilon_transition, true);
        }

        // Initial transition.
        let epsilon_transition = HfstBasicTransition::new_symbols(
            offset,
            HfstTropicalTransducerTransitionData::get_epsilon(),
            HfstTropicalTransducerTransitionData::get_epsilon(),
            0.0,
            self.coder_mut(),
        );
        self.add_transition(state1, &epsilon_transition, true);
    }

    /** @brief Look up 'lookup_path', collecting two-level paths into 'results'. */
    // [spec:hfst:def:hfst-transition-graph.lookup-fn]
    // [spec:hfst:sem:hfst-transition-graph.lookup-fn]
    pub fn lookup(
        &self,
        lookup_path: &StringVector,
        results: &mut HfstTwoLevelPaths,
        max_epsilon_cycles: Option<usize>,
        max_weight: Option<&f32>,
        max_number: i32,
        obey_flags: bool,
    ) {
        let state: HfstState = 0;
        let lookup_index: u32 = 0;
        let mut path_so_far = HfstTwoLevelPath {
            first: 0.0,
            second: Vec::new(),
        };
        let alphabet = self.get_alphabet().clone();
        let mut flag_diacritic_path: Option<StringVector> =
            if obey_flags { Some(Vec::new()) } else { None };

        match max_epsilon_cycles {
            Some(mec) => {
                let eh = HfstEpsilonHandler::new(mec);
                self.lookup_recursive(
                    lookup_path,
                    results,
                    state,
                    lookup_index,
                    &mut path_so_far,
                    &alphabet,
                    eh,
                    mec,
                    max_weight,
                    max_number,
                    flag_diacritic_path.as_mut(),
                );
            }
            None => {
                let eh = HfstEpsilonHandler::new(100000);
                self.lookup_recursive(
                    lookup_path,
                    results,
                    state,
                    lookup_index,
                    &mut path_so_far,
                    &alphabet,
                    eh,
                    100000,
                    max_weight,
                    max_number,
                    flag_diacritic_path.as_mut(),
                );
            }
        }
    }

    // --- Intersection / merge ---

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-target-state-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-target-state-fn]
    // [spec:hfst:def:hfst-transition-graph.find-target-state-fn]
    // [spec:hfst:sem:hfst-transition-graph.find-target-state-fn]
    pub fn find_target_state(
        target1: HfstState,
        target2: HfstState,
        state_map: &mut StateMap,
        intersection: &mut HfstBasicTransducer,
        was_new_state: &mut bool,
    ) -> HfstState {
        let state_pair = (target1, target2);
        if let Some(s) = state_map.get(&state_pair) {
            *was_new_state = false;
            return *s;
        }
        let retval = intersection.add_state_new();
        state_map.insert(state_pair, retval);
        *was_new_state = true;
        retval
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.handle-match-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.handle-match-fn]
    // [spec:hfst:def:hfst-transition-graph.handle-match-fn]
    // [spec:hfst:sem:hfst-transition-graph.handle-match-fn]
    pub fn handle_match(
        graph1: &HfstBasicTransducer,
        tr1: &HfstBasicTransition,
        graph2: &HfstBasicTransducer,
        tr2: &HfstBasicTransition,
        intersection: &mut HfstBasicTransducer,
        state: HfstState,
        state_map: &mut StateMap,
    ) -> HfstState {
        let target1 = tr1.get_target_state();
        let target2 = tr2.get_target_state();
        let mut was_new_state = false;
        let retval = Self::find_target_state(
            target1,
            target2,
            state_map,
            intersection,
            &mut was_new_state,
        );
        // the sum of weights is copied to the resulting intersection
        let transition_weight = tr1.get_weight() + tr2.get_weight();
        // tr1's labels resolve through graph1's coder; re-intern into the result.
        let isym = tr1.get_input_symbol(graph1.coder());
        let osym = tr1.get_output_symbol(graph1.coder());
        let tr = HfstBasicTransition::new_symbols(
            retval,
            isym,
            osym,
            transition_weight,
            intersection.coder_mut(),
        );
        intersection.add_transition(state, &tr, true);
        if was_new_state && (graph1.is_final_state(target1) && graph2.is_final_state(target2)) {
            let final_weight = graph1.get_final_weight(target1) + graph2.get_final_weight(target2);
            intersection.set_final_weight(retval, &final_weight);
        }
        retval
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-matches-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-matches-fn]
    // [spec:hfst:def:hfst-transition-graph.find-matches-fn]
    // [spec:hfst:sem:hfst-transition-graph.find-matches-fn]
    pub fn find_matches(
        graph1: &HfstBasicTransducer,
        state1: HfstState,
        graph2: &HfstBasicTransducer,
        state2: HfstState,
        intersection: &mut HfstBasicTransducer,
        state: HfstState,
        state_map: &mut StateMap,
        agenda: &mut BTreeSet<HfstState>,
    ) {
        agenda.insert(state); // do not handle 'state' twice
        let tr1 = &graph1.state_vector[state1 as usize];
        let tr2 = &graph2.state_vector[state2 as usize];

        if tr1.len() == 0 || tr2.len() == 0 {
            return; // no matches possible
        }
        let mut start_search_from: u32 = 0;

        for transition1 in tr1.iter() {
            let transition_data1 = transition1.get_transition_data();

            for j in start_search_from..tr2.len() as u32 {
                let transition2 = &tr2[j as usize];
                let transition_data2 = transition2.get_transition_data();
                if transition_data2.less_than_ignore_weight(transition_data1) {
                    // no match found, continue searching
                } else if transition_data1.less_than_ignore_weight(transition_data2) {
                    start_search_from = j;
                    break;
                } else {
                    // match found
                    let target = Self::handle_match(
                        graph1,
                        transition1,
                        graph2,
                        transition2,
                        intersection,
                        state,
                        state_map,
                    );
                    if !agenda.contains(&target) {
                        Self::find_matches(
                            graph1,
                            transition1.get_target_state(),
                            graph2,
                            transition2.get_target_state(),
                            intersection,
                            target,
                            state_map,
                            agenda,
                        );
                    }
                    start_search_from = j + 1;
                    break;
                }
            }
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.intersect-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.intersect-fn]
    pub fn intersect(
        graph1: &mut HfstBasicTransducer,
        graph2: &mut HfstBasicTransducer,
    ) -> HfstBasicTransducer {
        let mut retval = HfstBasicTransducer::new();
        let mut state_map: StateMap = BTreeMap::new();
        let mut agenda: BTreeSet<HfstState> = BTreeSet::new();
        graph1.sort_arcs();
        graph2.sort_arcs();
        state_map.insert((0, 0), 0); // initial states

        if graph1.is_final_state(0) && graph2.is_final_state(0) {
            let final_weight = graph1.get_final_weight(0).min(graph2.get_final_weight(0));
            retval.set_final_weight(0, &final_weight);
        }

        Self::find_matches(
            graph1,
            0,
            graph2,
            0,
            &mut retval,
            0,
            &mut state_map,
            &mut agenda,
        );

        retval
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.handle-non-list-match-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.handle-non-list-match-fn]
    // [spec:hfst:def:hfst-transition-graph.handle-non-list-match-fn]
    // [spec:hfst:sem:hfst-transition-graph.handle-non-list-match-fn]
    pub fn handle_non_list_match(
        graph: &HfstBasicTransducer,
        graph_transition: &HfstBasicTransition,
        merger: &HfstBasicTransducer,
        merger_target: HfstState,
        result: &mut HfstBasicTransducer,
        result_state: HfstState,
        state_map: &mut StateMap,
    ) -> HfstState {
        let graph_target = graph_transition.get_target_state();
        let mut was_new_state = false;
        let retval = Self::find_target_state(
            graph_target,
            merger_target,
            state_map,
            result,
            &mut was_new_state,
        );
        let isym = graph_transition.get_input_symbol(graph.coder());
        let osym = graph_transition.get_output_symbol(graph.coder());
        let tr = HfstBasicTransition::new_symbols(
            retval,
            isym,
            osym,
            graph_transition.get_weight(),
            result.coder_mut(),
        );
        result.add_transition(result_state, &tr, true);
        if was_new_state
            && (graph.is_final_state(graph_target) && merger.is_final_state(merger_target))
        {
            let final_weight =
                graph.get_final_weight(graph_target) + merger.get_final_weight(merger_target);
            result.set_final_weight(retval, &final_weight);
        }
        retval
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.handle-list-match-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.handle-list-match-fn]
    // [spec:hfst:def:hfst-transition-graph.handle-list-match-fn]
    // [spec:hfst:sem:hfst-transition-graph.handle-list-match-fn]
    pub fn handle_list_match(
        graph: &HfstBasicTransducer,
        graph_transition: &HfstBasicTransition,
        merger: &HfstBasicTransducer,
        merger_transition: &HfstBasicTransition,
        result: &mut HfstBasicTransducer,
        result_state: HfstState,
        state_map: &mut StateMap,
        markers_added: &mut BTreeSet<String>,
    ) -> HfstState {
        let graph_target = graph_transition.get_target_state();
        let merger_target = merger_transition.get_target_state();
        let mut was_new_state = false;
        let retval = Self::find_target_state(
            graph_target,
            merger_target,
            state_map,
            result,
            &mut was_new_state,
        );
        let transition_weight = graph_transition.get_weight() + merger_transition.get_weight();

        // testing: add a marker
        let extra_state = result.add_state_new();
        let graph_isym = graph_transition.get_input_symbol(graph.coder());
        let graph_osym = graph_transition.get_output_symbol(graph.coder());
        let marker_tr = HfstBasicTransition::new_symbols(
            extra_state,
            format!("@{}@", graph_isym),
            format!("@{}@", graph_osym),
            0.0,
            result.coder_mut(),
        );
        result.add_transition(result_state, &marker_tr, true);
        markers_added.insert(format!("@{}@", graph_isym));

        let merger_isym = merger_transition.get_input_symbol(merger.coder());
        let merger_osym = merger_transition.get_output_symbol(merger.coder());
        let merger_tr = HfstBasicTransition::new_symbols(
            retval,
            merger_isym,
            merger_osym,
            transition_weight,
            result.coder_mut(),
        );
        result.add_transition(extra_state, &merger_tr, true);
        if was_new_state
            && (graph.is_final_state(graph_target) && merger.is_final_state(merger_target))
        {
            let final_weight =
                graph.get_final_weight(graph_target) + merger.get_final_weight(merger_target);
            result.set_final_weight(retval, &final_weight);
        }
        retval
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-list-symbol-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-list-symbol-fn]
    // [spec:hfst:def:hfst-transition-graph.is-list-symbol-fn]
    // [spec:hfst:sem:hfst-transition-graph.is-list-symbol-fn]
    pub fn is_list_symbol(
        transition_data: &HfstTropicalTransducerTransitionData,
        list_symbols: &BTreeMap<String, BTreeSet<String>>,
        coder: &SymbolCoder,
    ) -> bool {
        let isymbol = transition_data.get_input_symbol(coder);
        let osymbol = transition_data.get_output_symbol(coder);

        if isymbol != osymbol {
            panic!("is_list_symbol: input and output symbols must be the same");
        }
        list_symbols.contains_key(&isymbol)
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-matches-for-merge-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-matches-for-merge-fn]
    #[allow(clippy::too_many_arguments)]
    // [spec:hfst:def:hfst-transition-graph.find-matches-for-merge-fn]
    // [spec:hfst:sem:hfst-transition-graph.find-matches-for-merge-fn]
    pub fn find_matches_for_merge(
        graph: &HfstBasicTransducer,
        graph_state: HfstState,
        merger: &HfstBasicTransducer,
        merger_state: HfstState,
        result: &mut HfstBasicTransducer,
        result_state: HfstState,
        state_map: &mut StateMap,
        agenda: &mut BTreeSet<HfstState>,
        list_symbols: &BTreeMap<String, BTreeSet<String>>,
        markers_added: &mut BTreeSet<String>,
    ) {
        agenda.insert(result_state); // do not handle 'result_state' twice
        let graph_transitions = &graph.state_vector[graph_state as usize];
        let merger_transitions = &merger.state_vector[merger_state as usize];

        if graph_transitions.len() == 0 {
            return; // no matches possible
        }

        for graph_transition in graph_transitions.iter() {
            let graph_transition_data = graph_transition.get_transition_data();

            // List symbols must be checked separately.
            if Self::is_list_symbol(graph_transition_data, list_symbols, graph.coder()) {
                let symbol_list =
                    &list_symbols[&graph_transition_data.get_input_symbol(graph.coder())];
                let mut list_match_found = false;
                for merger_transition in merger_transitions.iter() {
                    let merger_transition_data = merger_transition.get_transition_data();
                    let isymbol = merger_transition_data.get_input_symbol(merger.coder());
                    let osymbol = merger_transition_data.get_output_symbol(merger.coder());

                    if isymbol != osymbol {
                        panic!("find_matches_for_merge: input and output symbols must be the same");
                    }

                    if symbol_list.contains(&isymbol) {
                        list_match_found = true;
                        let target = Self::handle_list_match(
                            graph,
                            graph_transition,
                            merger,
                            merger_transition,
                            result,
                            result_state,
                            state_map,
                            markers_added,
                        );
                        if !agenda.contains(&target) {
                            Self::find_matches_for_merge(
                                graph,
                                graph_transition.get_target_state(),
                                merger,
                                merger_transition.get_target_state(),
                                result,
                                target,
                                state_map,
                                agenda,
                                list_symbols,
                                markers_added,
                            );
                        }
                    }
                }
                if list_match_found {
                    continue;
                }
            }
            // Not a list symbol (or no match): copy the symbol as such, using
            // merger_state as the merger transition target state.
            let target = Self::handle_non_list_match(
                graph,
                graph_transition,
                merger,
                merger_state,
                result,
                result_state,
                state_map,
            );
            if !agenda.contains(&target) {
                Self::find_matches_for_merge(
                    graph,
                    graph_transition.get_target_state(),
                    merger,
                    merger_state,
                    result,
                    target,
                    state_map,
                    agenda,
                    list_symbols,
                    markers_added,
                );
            }
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.merge-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.merge-fn]
    // [spec:hfst:def:hfst-transition-graph.hfst-transition-graph-fn]
    // [spec:hfst:sem:hfst-transition-graph.hfst-transition-graph-fn]
    pub fn merge(
        graph: &mut HfstBasicTransducer,
        merger: &mut HfstBasicTransducer,
        list_symbols: &BTreeMap<String, BTreeSet<String>>,
        markers_added: &mut BTreeSet<String>,
    ) -> HfstBasicTransducer {
        let mut result = HfstBasicTransducer::new();
        let mut state_map: StateMap = BTreeMap::new();
        let mut agenda: BTreeSet<HfstState> = BTreeSet::new();
        graph.sort_arcs();
        merger.sort_arcs();
        state_map.insert((0, 0), 0); // initial states

        if graph.is_final_state(0) && merger.is_final_state(0) {
            let final_weight = graph.get_final_weight(0) + merger.get_final_weight(0);
            result.set_final_weight(0, &final_weight);
        }

        // The C++ catches the const char* throws and rethrows as
        // TransducersAreNotAutomataException.
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Self::find_matches_for_merge(
                graph,
                0,
                merger,
                0,
                &mut result,
                0,
                &mut state_map,
                &mut agenda,
                list_symbols,
                markers_added,
            )
        }));
        std::panic::set_hook(prev);
        if let Err(e) = r {
            let msg = if let Some(s) = e.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else {
                std::panic::resume_unwind(e)
            };
            crate::HFST_THROW_MESSAGE!(TransducersAreNotAutomataException, msg);
        }

        result
    }
}

impl Default for HfstBasicTransducer {
    fn default() -> Self {
        Self::new()
    }
}
