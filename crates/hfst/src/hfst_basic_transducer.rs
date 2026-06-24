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
use std::io::{BufRead, Write};

use crate::harmonize_unknown_and_identity_symbols::HarmonizeUnknownAndIdentitySymbols;
use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_data_types::{double_to_float, size_t_to_uint};
use crate::hfst_exception_defs::{
    EmptyStringException, EndOfStreamException, HfstException, NotValidAttFormatException,
    NotValidPrologFormatException, StateIndexOutOfBoundsException, StateIsNotFinalException,
    TransducersAreNotAutomataException,
};
use crate::hfst_flag_diacritics::FdOperation;
use crate::hfst_symbol_defs::{
    HfstSymbolPairSubstitutions, HfstSymbolSubstitutions, StringPair, StringPairSet,
    StringPairVector, StringSet, is_epsilon, is_identity, is_unknown,
};
use crate::hfst_tropical_transducer_transition_data::{
    HfstTropicalTransducerTransitionData, SymbolType, WeightType,
};
use crate::string_utils::replace_all;

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

// C `atof`: parse the leading float, 0.0 on failure. The inputs here are clean
// whitespace-delimited tokens, so a plain parse suffices.
fn atof(s: &str) -> f64 {
    s.trim_start().parse::<f64>().unwrap_or(0.0)
}

// Raw stand-in for `sprintf(ptr + offset, ...)` into a caller-provided buffer:
// copies the pre-formatted bytes and a trailing NUL (as sprintf does), returning
// the byte count excluding the NUL (sprintf's return value).
unsafe fn sprintf_at(ptr: *mut libc::c_char, offset: usize, s: &str) -> usize {
    unsafe {
        let dst = (ptr as *mut u8).add(offset);
        std::ptr::copy_nonoverlapping(s.as_ptr(), dst, s.len());
        *dst.add(s.len()) = 0;
    }
    s.len()
}

// `fgets(buf, 255, file)`: read up to 254 bytes or through a newline; None at
// EOF. A trailing newline (if any) is kept, matching fgets.
unsafe fn c_fgets(file: *mut libc::FILE) -> Option<String> {
    let mut buf = [0u8; 255];
    let r = unsafe { libc::fgets(buf.as_mut_ptr() as *mut libc::c_char, 255, file) };
    if r.is_null() {
        return None;
    }
    let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    Some(String::from_utf8_lossy(&buf[..len]).into_owned())
}

// `std::istream::getline(buf, 255)`: read up to 254 bytes until '\n' (extracted
// and discarded) or EOF. Returns (line, eof_reached) mirroring the stream's
// eofbit after the call.
fn cpp_getline(is: &mut dyn BufRead) -> (String, bool) {
    let mut line: Vec<u8> = Vec::new();
    let mut eof = false;
    loop {
        let mut byte = [0u8; 1];
        match is.read(&mut byte) {
            Ok(0) => {
                eof = true;
                break;
            }
            Ok(_) => {
                if byte[0] == b'\n' {
                    break;
                }
                if line.len() >= 254 {
                    break;
                }
                line.push(byte[0]);
            }
            Err(_) => {
                eof = true;
                break;
            }
        }
    }
    (String::from_utf8_lossy(&line).into_owned(), eof)
}

// Approximation of `std::istream::eof()` for a fresh reader: no bytes remain.
fn is_eof(is: &mut dyn BufRead) -> bool {
    match is.fill_buf() {
        Ok(b) => b.is_empty(),
        Err(_) => true,
    }
}

// `get_stripped_line` wrapped in the C++ try/catch: returns None when it would
// throw `EndOfStreamException`. The panic hook is silenced so the caught
// exception (a `panic_any`) does not print.
fn catch_get_stripped_line(
    is: &mut dyn BufRead,
    file: *mut libc::FILE,
    linecount: &mut u32,
) -> Option<String> {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        HfstBasicTransducer::get_stripped_line(is, file, linecount)
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

// [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.subst-map]
pub type SubstMap = BTreeMap<HfstSymbol, HfstBasicTransducer>;

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

// Where a substituting copy of a graph is inserted (origin/target state, weight,
// and a raw pointer to the substituting graph — the C++ stores a
// `const_cast` `HfstBasicTransducer*`).
pub struct substitution_data {
    pub origin_state: HfstState,
    pub target_state: HfstState,
    pub weight: WeightType,
    pub substituting_graph: *const HfstBasicTransducer,
}

impl substitution_data {
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.substitution-data.substitution-data-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.substitution-data.substitution-data-fn]
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

    /** @brief Write the graph in AT&T format to ostream `os`. */
    pub fn write_in_att_format_os(&self, os: &mut dyn Write, write_weights: bool) {
        let mut source_state: u32 = 0;
        for it in self.state_vector.iter() {
            for tr_it in it.iter() {
                let data = tr_it.get_transition_data().clone();

                let mut isymbol = data.get_input_symbol();
                replace_all(&mut isymbol, " ", "@_SPACE_@");
                replace_all(&mut isymbol, "@_EPSILON_SYMBOL_@", "@0@");
                replace_all(&mut isymbol, "\t", "@_TAB_@");

                let mut osymbol = data.get_output_symbol();
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

    /** @brief Write the graph in AT&T format to FILE `file`. */
    pub unsafe fn write_in_att_format_file(&self, file: *mut libc::FILE, write_weights: bool) {
        unsafe {
            let mut source_state: u32 = 0;
            for it in self.state_vector.iter() {
                for tr_it in it.iter() {
                    let data = tr_it.get_transition_data().clone();

                    let mut isymbol = data.get_input_symbol();
                    replace_all(&mut isymbol, " ", "@_SPACE_@");
                    replace_all(&mut isymbol, "@_EPSILON_SYMBOL_@", "@0@");
                    replace_all(&mut isymbol, "\t", "@_TAB_@");

                    let mut osymbol = data.get_output_symbol();
                    replace_all(&mut osymbol, " ", "@_SPACE_@");
                    replace_all(&mut osymbol, "@_EPSILON_SYMBOL_@", "@0@");
                    replace_all(&mut osymbol, "\t", "@_TAB_@");

                    c_fputs(
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
                        c_fputs(file, "\t");
                        Self::write_weight_file(file, data.get_weight());
                    }
                    c_fputs(file, "\n");
                }
                if self.is_final_state(source_state) {
                    c_fputs(file, &format!("{}", source_state));
                    if write_weights {
                        c_fputs(file, "\t");
                        Self::write_weight_file(file, self.get_final_weight(source_state));
                    }
                    c_fputs(file, "\n");
                }
                source_state += 1;
            }
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-att-format-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-att-format-fn]
    //
    // Writes into a caller-provided C buffer via `sprintf` at a running offset.
    pub unsafe fn write_in_att_format_ptr(&self, ptr: *mut libc::c_char, write_weights: bool) {
        unsafe {
            let mut source_state: u32 = 0;
            let mut cwt: usize = 0; // characters written in total
            #[allow(unused_assignments)]
            let mut cw: usize = 0; // characters written in latest call to sprintf
            for it in self.state_vector.iter() {
                for tr_it in it.iter() {
                    let data = tr_it.get_transition_data().clone();

                    let mut isymbol = data.get_input_symbol();
                    replace_all(&mut isymbol, " ", "@_SPACE_@");
                    replace_all(&mut isymbol, "@_EPSILON_SYMBOL_@", "@0@");
                    replace_all(&mut isymbol, "\t", "@_TAB_@");

                    let mut osymbol = data.get_output_symbol();
                    replace_all(&mut osymbol, " ", "@_SPACE_@");
                    replace_all(&mut osymbol, "@_EPSILON_SYMBOL_@", "@0@");
                    replace_all(&mut osymbol, "\t", "@_TAB_@");

                    cw = sprintf_at(
                        ptr,
                        cwt,
                        &format!(
                            "{}\t{}\t{}\t{}",
                            source_state,
                            tr_it.get_target_state(),
                            isymbol,
                            osymbol
                        ),
                    );
                    cwt += cw;

                    if write_weights {
                        cw = sprintf_at(ptr, cwt, &format!("\t{:.6}", data.get_weight()));
                    }
                    cwt += cw;
                    cw = sprintf_at(ptr, cwt, "\n");
                    cwt += cw;
                }
                if self.is_final_state(source_state) {
                    cw = sprintf_at(ptr, cwt, &format!("{}", source_state));
                    cwt += cw;
                    if write_weights {
                        cw = sprintf_at(
                            ptr,
                            cwt,
                            &format!("\t{:.6}", self.get_final_weight(source_state)),
                        );
                    }
                    cwt += cw;
                    cw = sprintf_at(ptr, cwt, "\n");
                    cwt += cw;
                }
                source_state += 1;
            }
        }
    }

    /** @brief Write the graph in AT&T format to FILE `file` using numbers
    instead of symbol names. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-att-format-number-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-att-format-number-fn]
    //
    // NB: the C++ prints the final-state line *inside* the transition loop (so a
    // multi-transition final state repeats it); preserved bug-for-bug.
    pub unsafe fn write_in_att_format_number_file(
        &self,
        file: *mut libc::FILE,
        write_weights: bool,
    ) {
        unsafe {
            let mut source_state: u32 = 0;
            for it in self.state_vector.iter() {
                for tr_it in it.iter() {
                    let data = tr_it.get_transition_data().clone();

                    c_fputs(
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
                        c_fputs(file, &format!("\t{:.6}", data.get_weight()));
                    }
                    c_fputs(file, "\n");

                    if self.is_final_state(source_state) {
                        c_fputs(file, &format!("{}", source_state));
                        if write_weights {
                            c_fputs(
                                file,
                                &format!("\t{:.6}", self.get_final_weight(source_state)),
                            );
                        }
                        c_fputs(file, "\n");
                    }
                }
                source_state += 1;
            }
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-att-line-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-att-line-fn]
    //
    // sscanf(line, "%s%s%s%s%s", ...) reads up to five whitespace-delimited
    // fields; `n` is how many were read.
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

            let tr =
                HfstBasicTransition::new_symbols(atoi(a(1)), input_symbol, output_symbol, weight);
            self.add_transition(atoi(a(0)), &tr, true);
        } else {
            // line could not be parsed
            return false;
        }
        true
    }

    // HfstBasicTransducer(FILE*) — read an AT&T transducer from `file`.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-basic-transducer-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-basic-transducer-fn]
    pub fn new_from_file(file: *mut libc::FILE) -> Self {
        let mut alphabet = HfstAlphabet::new();
        Self::initialize_alphabet(&mut alphabet);
        let mut state_vector = HfstBasicStates::new();
        state_vector.push(HfstBasicTransitions::new());
        let mut retval = HfstBasicTransducer {
            state_vector,
            final_weight_map: FinalWeightMap::new(),
            alphabet,
            name: String::new(),
        };
        let mut linecount: u32 = 0;
        let read = Self::read_in_att_format_file(file, "@0@", &mut linecount, false);
        retval.assign(&read);
        retval.name = String::new();
        retval
    }

    // Try to get a line from `is` (if `file` is null) or `file`. On success,
    // strip newlines, increment `linecount`, and return the line; else throw
    // EndOfStreamException.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-stripped-line-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-stripped-line-fn]
    pub fn get_stripped_line(
        is: &mut dyn BufRead,
        file: *mut libc::FILE,
        linecount: &mut u32,
    ) -> String {
        let linestr: String;
        if file.is_null() {
            // streams: the C++ condition is inverted (throws when NOT at eof) —
            // bug preserved.
            let (line, eof) = cpp_getline(is);
            if !eof {
                crate::HFST_THROW!(EndOfStreamException);
            }
            linestr = line;
        } else {
            match unsafe { c_fgets(file) } {
                None => crate::HFST_THROW!(EndOfStreamException),
                Some(l) => linestr = l,
            }
        }
        *linecount += 1;

        let mut s = linestr;
        Self::strip_newlines(&mut s)
    }

    // Create a graph from prolog format in `is` (if `file` is null) or `file`.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.read-in-prolog-format-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.read-in-prolog-format-fn]
    pub fn read_in_prolog_format(
        is: &mut dyn BufRead,
        file: *mut libc::FILE,
        linecount: &mut u32,
    ) -> HfstBasicTransducer {
        let mut retval = HfstBasicTransducer::new();
        let mut linestr: String;

        loop {
            match catch_get_stripped_line(is, file, linecount) {
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
            match catch_get_stripped_line(is, file, linecount) {
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
        Self::read_in_prolog_format(is, std::ptr::null_mut(), linecount)
    }

    pub fn read_in_prolog_format_file(
        file: *mut libc::FILE,
        linecount: &mut u32,
    ) -> HfstBasicTransducer {
        let mut dummy = std::io::empty();
        Self::read_in_prolog_format(&mut dummy, file, linecount)
    }

    // Create a graph from AT&T format in `is` (if `file` is null) or `file`.
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.read-in-att-format-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.read-in-att-format-fn]
    pub fn read_in_att_format(
        is: &mut dyn BufRead,
        file: *mut libc::FILE,
        epsilon_symbol: &str,
        linecount: &mut u32,
        warn_negs: bool,
    ) -> HfstBasicTransducer {
        if file.is_null() {
            if is_eof(is) {
                crate::HFST_THROW!(EndOfStreamException);
            }
        } else if unsafe { libc::feof(file) != 0 } {
            crate::HFST_THROW!(EndOfStreamException);
        }

        let mut retval = HfstBasicTransducer::new();
        loop {
            let line: String;
            if file.is_null() {
                // bug preserved: breaks when the getline did NOT reach eof
                let (l, eof) = cpp_getline(is);
                if !eof {
                    break;
                }
                line = l;
            } else {
                match unsafe { c_fgets(file) } {
                    None => break,
                    Some(l) => line = l,
                }
            }

            *linecount += 1;

            let bytes = line.as_bytes();
            // an empty line (with or without newline, incl. windows newline)
            if bytes.is_empty()
                || (bytes.len() == 1 && bytes[0] == b'\n')
                || (bytes.len() == 2 && bytes[0] == b'\r' && bytes[1] == b'\n')
            {
                // make sure that the end-of-file is reached
                if file.is_null() {
                    let mut b = [0u8; 1];
                    let _ = is.read(&mut b);
                } else {
                    unsafe {
                        libc::fgetc(file);
                    }
                }
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
        Self::read_in_att_format(
            is,
            std::ptr::null_mut(),
            epsilon_symbol,
            linecount,
            warn_negs,
        )
    }

    pub fn read_in_att_format_file(
        file: *mut libc::FILE,
        epsilon_symbol: &str,
        linecount: &mut u32,
        warn_negs: bool,
    ) -> HfstBasicTransducer {
        let mut dummy = std::io::empty();
        Self::read_in_att_format(&mut dummy, file, epsilon_symbol, linecount, warn_negs)
    }

    // --- Substitution (private in-place helpers) ---

    /* In-place substitution of `old_symbol` with `new_symbol`. */
    fn substitute_in_place(
        &mut self,
        old_symbol: &HfstSymbol,
        new_symbol: &HfstSymbol,
        input_side: bool,
        output_side: bool,
    ) {
        for s in 0..self.state_vector.len() {
            for i in 0..self.state_vector[s].len() {
                let mut substituting_input_symbol = self.state_vector[s][i].get_input_symbol();
                let mut substituting_output_symbol = self.state_vector[s][i].get_output_symbol();
                let mut substitution_made = false;

                if input_side && self.state_vector[s][i].get_input_symbol() == *old_symbol {
                    substituting_input_symbol = new_symbol.clone();
                    substitution_made = true;
                }
                if output_side && self.state_vector[s][i].get_output_symbol() == *old_symbol {
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
                        self.add_symbol_to_alphabet(
                            &HfstTropicalTransducerTransitionData::get_symbol(new_inumber),
                        );
                    } else {
                        new_inumber = old_inumber;
                    }

                    if new_onumber != no_substitution {
                        self.add_symbol_to_alphabet(
                            &HfstTropicalTransducerTransitionData::get_symbol(new_onumber),
                        );
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

                    self.add_symbol_to_alphabet(&HfstTropicalTransducerTransitionData::get_symbol(
                        new_input_number,
                    ));
                    self.add_symbol_to_alphabet(&HfstTropicalTransducerTransitionData::get_symbol(
                        new_output_number,
                    ));

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

    /* In-place removal of all transitions equivalent to `sp`. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-transitions-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-transitions-fn]
    pub fn remove_transitions(&mut self, sp: &HfstSymbolPair) {
        let in_match = HfstTropicalTransducerTransitionData::get_number(&sp.0);
        let out_match = HfstTropicalTransducerTransitionData::get_number(&sp.1);

        let mut in_match_used = false;
        let mut out_match_used = false;

        for s in 0..self.state_vector.len() {
            // C++ `for (i=0; i<size(); i++)` with erase but no `i--`: after an
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

    /* In-place substitution of `old_sp` with the set `new_sps`. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.substitute-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.substitute-fn]
    fn substitute_in_place_pair_set(
        &mut self,
        old_sp: &HfstSymbolPair,
        new_sps: &HfstSymbolPairSet,
    ) {
        if new_sps.is_empty() {
            self.remove_transitions(old_sp);
            return;
        }

        let old_input_number = HfstTropicalTransducerTransitionData::get_number(&old_sp.0);
        let old_output_number = HfstTropicalTransducerTransitionData::get_number(&old_sp.1);

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
                    let tr = HfstBasicTransition::new_numbers(
                        target,
                        HfstTropicalTransducerTransitionData::get_number(&first.0),
                        HfstTropicalTransducerTransitionData::get_number(&first.1),
                        weight,
                        true,
                    );
                    self.state_vector[s][i] = tr;

                    // schedule the rest (C++ iterates from begin, so all of
                    // new_sps incl. the first are appended).
                    for sp in new_sps.iter() {
                        let tr2 = HfstBasicTransition::new_numbers(
                            target,
                            HfstTropicalTransducerTransitionData::get_number(&sp.0),
                            HfstTropicalTransducerTransitionData::get_number(&sp.1),
                            weight,
                            true,
                        );
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
        func: fn(&HfstSymbolPair, &mut HfstSymbolPairSet) -> bool,
    ) {
        for s in 0..self.state_vector.len() {
            let mut new_transitions: HfstBasicTransitions = Vec::new();

            for i in 0..self.state_vector[s].len() {
                let transition_symbol_pair = (
                    self.state_vector[s][i].get_input_symbol(),
                    self.state_vector[s][i].get_output_symbol(),
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

                    let tr =
                        HfstBasicTransition::new_symbols(target, fi.clone(), fo.clone(), weight);
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

    /** @brief Substitute `old_symbol` with `new_symbol` in all transitions. */
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

    /** @brief Substitute all transitions as defined in `substitutions`. */
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
        let st: usize = HfstTropicalTransducerTransitionData::get_max_number() as usize
            + substitutions.len()
            + 1;
        let no_substitution = size_t_to_uint(st);

        substitutions_.resize(
            (HfstTropicalTransducerTransitionData::get_max_number() + 1) as usize,
            no_substitution,
        );
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

    /** @brief Substitute transitions x:y -> X:Y as defined in `substitutions`. */
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

    /** @brief Substitute all transitions `sp` with a set of transitions `sps`. */
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

    /** @brief Substitute all transitions `old_pair` with `new_pair`. */
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

    /** @brief Substitute all transitions with a set defined by function `func`. */
    pub fn substitute_with_func(
        &mut self,
        func: fn(&HfstSymbolPair, &mut HfstSymbolPairSet) -> bool,
    ) -> &mut Self {
        self.substitute_in_place_func(func);
        self
    }

    /** @brief Substitute transitions `sp` with a copy of `graph`. */
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
                if data.get_input_symbol() == sp.0 && data.get_output_symbol() == sp.1 {
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
    pub fn add_substitution(&mut self, sub: &substitution_data) {
        // Epsilon transition to initial state of the substituting graph.
        let s = self.add_state_new();
        let epsilon_transition = HfstBasicTransition::new_symbols(
            s,
            HfstTropicalTransducerTransitionData::get_epsilon(),
            HfstTropicalTransducerTransitionData::get_epsilon(),
            sub.weight,
        );
        self.add_transition(sub.origin_state, &epsilon_transition, true);

        let offset = s;

        // Copy the graph. The raw-pointer deref mirrors the C++ (the graphs are
        // distinct; aliasing self would be UB there too).
        let graph_ref = unsafe { &*sub.substituting_graph };
        let mut source_state: HfstState = 0;
        for it in graph_ref.state_vector.iter() {
            for tr_it in it.iter() {
                let data = tr_it.get_transition_data();
                let transition = HfstBasicTransition::new_symbols(
                    tr_it.get_target_state() + offset,
                    data.get_input_symbol(),
                    data.get_output_symbol(),
                    data.get_weight(),
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
            );
            self.add_transition(*k + offset, &epsilon_transition, true);
        }
    }

    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.weight2marker-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.weight2marker-fn]
    //
    // The C++ uses `ostringstream <<` (default float text); Rust's `{}` differs
    // textually but round-trips with marker2weight's parse internally.
    pub fn weight2marker(weight: f32) -> String {
        format!("@{}@", weight)
    }

    /** @brief Replace each non-zero transition weight with a `@w@` marker arc. */
    pub fn substitute_weights_with_markers(&mut self) -> &mut Self {
        let limit = self.state_vector.len();
        for state in 0..limit {
            let mut old_indices: Vec<usize> = Vec::new();
            let mut new_transitions: Vec<HfstBasicTransition> = Vec::new();

            for i in 0..self.state_vector[state].len() {
                let data = self.state_vector[state][i].get_transition_data().clone();
                if data.get_weight() != 0.0 {
                    new_transitions.push(HfstBasicTransition::new_symbols(
                        self.state_vector[state][i].get_target_state(),
                        data.get_input_symbol(),
                        data.get_output_symbol(),
                        data.get_weight(),
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
                let marker_transition = HfstBasicTransition::new_symbols(
                    it.get_target_state(),
                    marker.clone(),
                    marker,
                    0.0,
                );
                let new_transition = HfstBasicTransition::new_symbols(
                    new_state,
                    it.get_input_symbol(),
                    it.get_output_symbol(),
                    0.0,
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
                let epsilon_transition =
                    HfstBasicTransition::new_symbols(new_state, marker.clone(), marker, 0.0);
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

    /** @brief Replace `@w@` marker arcs with transition weights. */
    pub fn substitute_markers_with_weights(&mut self) -> &mut Self {
        let limit = self.state_vector.len();
        for state in 0..limit {
            let mut old_indices: Vec<usize> = Vec::new();
            let mut new_transitions: Vec<HfstBasicTransition> = Vec::new();

            for i in 0..self.state_vector[state].len() {
                let data = self.state_vector[state][i].get_transition_data().clone();
                let mut weight: f32 = 0.0;
                if !Self::marker2weight(&data.get_input_symbol(), &mut weight)
                    && Self::marker2weight(&data.get_output_symbol(), &mut weight)
                {
                    new_transitions.push(HfstBasicTransition::new_symbols(
                        self.state_vector[state][i].get_target_state(),
                        data.get_input_symbol(),
                        crate::hfst_symbol_defs::internal_epsilon.to_string(),
                        weight,
                    ));
                    old_indices.push(i);
                } else if Self::marker2weight(&data.get_input_symbol(), &mut weight)
                    && Self::marker2weight(&data.get_output_symbol(), &mut weight)
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

    /** @brief Insert freely any number of `symbol_pair` with weight `weight`. */
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
            );
            self.state_vector[s].push(tr);
        }
        self
    }

    /** @brief Insert freely any of the pairs in `symbol_pairs`. */
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
                );
                self.state_vector[s].push(tr);
            }
        }
        self
    }

    /** @brief Insert freely any number of `graph` in this graph. */
    pub fn insert_freely_graph(&mut self, graph: &HfstBasicTransducer) -> &mut Self {
        let marker_this = HfstTropicalTransducerTransitionData::get_marker(&self.alphabet);
        let marker_graph = HfstTropicalTransducerTransitionData::get_marker(&self.alphabet);
        let mut marker = marker_this;
        if marker_graph > marker {
            marker = marker_graph;
        }

        // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.marker-pair-fn]
        // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.marker-pair-fn]
        let marker_pair = (marker.clone(), marker.clone());
        self.insert_freely_pair(&marker_pair, 0.0);
        self.substitute_pair_with_graph(&marker_pair, graph);
        self.alphabet.remove(&marker); // (C++ flags this line as needing a fix)

        self
    }

    // --- Disjunction ---

    /* Disjunct the transition of path `spv` pointed by `it` to state `s`. */
    // [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.disjunct-fn]
    // [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.disjunct-fn]
    pub fn disjunct(&mut self, spv: &StringPairVector, it: &mut usize, s: HfstState) -> HfstState {
        let mut current_state = s;
        while *it != spv.len() {
            // C++ copies the transition vector before searching it.
            let tr = self.state_vector[current_state as usize].clone();
            let mut transition_found = false;
            let mut next_state: HfstState = 0;

            for tr_it in tr.iter() {
                let data = tr_it.get_transition_data();
                if data.get_input_symbol() == spv[*it].0 && data.get_output_symbol() == spv[*it].1 {
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
                );
                self.add_transition(current_state, &transition, true);
            }

            *it += 1;
            current_state = next_state;
        }
        current_state
    }

    /** @brief Disjunct this graph with a one-path graph defined by `spv`. */
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
                if data.get_input_symbol() != data.get_output_symbol() {
                    crate::HFST_THROW!(TransducersAreNotAutomataException);
                }
                symbols_present.insert(data.get_input_symbol());
            }

            let alpha_snapshot: Vec<HfstSymbol> = self.alphabet.iter().cloned().collect();
            for alpha_it in alpha_snapshot.iter() {
                if !symbols_present.contains(alpha_it) && !Self::is_special_symbol(alpha_it) {
                    let tr = HfstBasicTransition::new_symbols(
                        failure_state,
                        alpha_it.clone(),
                        alpha_it.clone(),
                        0.0,
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
    pub fn flag_purge(&mut self, flag: &str) {
        // (1) Go through all states and transitions
        for s in 0..self.state_vector.len() {
            for i in 0..self.state_vector[s].len() {
                let isym = self.state_vector[s][i].get_input_symbol();
                let osym = self.state_vector[s][i].get_output_symbol();
                if Self::purge_symbol(&isym, flag) || Self::purge_symbol(&osym, flag) {
                    let target = self.state_vector[s][i].get_target_state();
                    let weight = self.state_vector[s][i].get_weight();
                    let tr = HfstBasicTransition::new_symbols(
                        target,
                        "@_EPSILON_SYMBOL_@".to_string(),
                        "@_EPSILON_SYMBOL_@".to_string(),
                        weight,
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

    /** @brief Harmonize this graph and `another` (expand unknown/identity). */
    pub fn harmonize(&mut self, another: &mut HfstBasicTransducer) -> &mut Self {
        let _foo = HarmonizeUnknownAndIdentitySymbols::new(self, another);
        self
    }

    /** @brief Substitute symbols with transducers as defined in `substitution_map`. */
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
                let istr = self.state_vector[s][j].get_input_symbol();
                let ostr = self.state_vector[s][j].get_output_symbol();
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
}

impl Default for HfstBasicTransducer {
    fn default() -> Self {
        Self::new()
    }
}
