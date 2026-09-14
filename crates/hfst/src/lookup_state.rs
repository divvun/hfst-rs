//! The per-run half of the optimized-lookup engine.
//!
//! The C++ `hfst_ol::Transducer` carried its tapes, flag state, traversal
//! bookkeeping, accumulated results, limits and clock on the same object as the
//! loaded machine, so every lookup took the whole transducer exclusively. This
//! module holds that scratch instead: a [`LookupState`] borrows a loaded
//! [`Transducer`] and owns everything the walk writes, which leaves the machine
//! shareable while any number of run states traverse it.
//!
//! The C++ lookup and epsilon-loop bodies (`transducer.cc`,
//! `find_epsilon_loops.cc`) live here unchanged apart from the receiver split:
//! reads that went through `this->tables`/`this->alphabet` now go through
//! `self.machine`.

use std::collections::BTreeSet;
use std::ops::ControlFlow;
use std::time::Instant;

use crate::hfst_data_types::{
    HfstOneLevelPath, HfstOneLevelPaths, HfstTwoLevelPath, HfstTwoLevelPaths, StringVector, Symbol,
};
use crate::hfst_flag_diacritics::FdState;
use crate::transducer::{
    DoubleTape, Encoder, MAX_RECURSION_DEPTH, NO_SYMBOL_NUMBER, SymbolNumber, SymbolTable,
    TRANSITION_TARGET_TABLE_START, Tape, Transducer, TransducerAlphabet, TransducerHeader,
    TransducerTablesInterface, TransitionTableIndex, TraversalState, TraversalStates, Weight,
    indexes_transition_table, utf8_sequence_length,
};

/// Input symbols admitted during a run because the machine's alphabet has no
/// tokenization for them.
///
/// The C++ pushed such a symbol straight into the machine's alphabet and
/// encoder, which made a lookup a write to the loaded transducer. The symbols
/// are numbered from the machine's symbol-table length upward — past every
/// number the tables can name, and past `orig_symbol_count`, so the engine
/// still routes them onto identity/unknown/default arcs exactly as it did when
/// the alphabet grew.
// [spec:hfst:req:lookup-run-state.out-of-alphabet-overlay]
struct AlphabetOverlay {
    /// Adopted symbols in numbering order; entry `n` is symbol `base + n`.
    symbols: Vec<Symbol>,
    /// Tokenizes only the adopted spellings. Consulted after the machine's own
    /// encoder, so a spelling the machine can tokenize always wins — which is
    /// what the single shared trie did when the symbol was added to it.
    encoder: Encoder,
}

impl AlphabetOverlay {
    fn new() -> Self {
        AlphabetOverlay {
            symbols: Vec::new(),
            encoder: Encoder::new(&SymbolTable::new(), 0),
        }
    }
}

/// Everything one optimized-lookup run writes.
///
/// Created against a machine and reusable across calls: the tapes and the
/// traversal stack keep their capacity, and an alphabet overlay built by one
/// lookup is still there for the next one through the same state. Nothing here
/// is visible to another run state, so the analyses a state returns are a
/// function of the machine, the input and the caller-set limits alone.
// [spec:hfst:req:lookup-run-state.caller-owned-scratch]
pub struct LookupState<'a, T: TransducerTablesInterface> {
    machine: &'a Transducer<T>,

    current_weight: Weight,
    /// The result set the recursive walk accumulates into; cleared at the start
    /// of each lookup. The C++ pointed a `HfstTwoLevelPaths *` member at a
    /// function-local set, which this owned field replaces directly.
    lookup_paths: HfstTwoLevelPaths,
    input_tape: Tape,
    output_tape: DoubleTape,
    flag_state: FdState<SymbolNumber>,
    traversal_states: TraversalStates,

    max_lookups: isize,
    recursion_depth_left: u32,
    max_time: f64,
    start_clock: Option<Instant>,

    /// Built on first admission; absent for the overwhelmingly common run whose
    /// input the machine can tokenize on its own.
    overlay: Option<AlphabetOverlay>,
}

impl<'a, T: TransducerTablesInterface> LookupState<'a, T> {
    pub fn new(machine: &'a Transducer<T>) -> Self {
        LookupState {
            machine,
            current_weight: 0.0,
            lookup_paths: BTreeSet::new(),
            input_tape: Tape::new(),
            output_tape: DoubleTape::new(),
            // A refcount bump on the machine's flag table, not a copy of it.
            flag_state: machine.flag_state_proto().clone(),
            traversal_states: TraversalStates::new(),
            max_lookups: -1,
            recursion_depth_left: MAX_RECURSION_DEPTH,
            max_time: 0.0,
            start_clock: None,
            overlay: None,
        }
    }

    #[inline]
    fn hdr(&self) -> &'a TransducerHeader {
        self.machine.get_header()
    }

    #[inline]
    fn alph(&self) -> &'a TransducerAlphabet {
        self.machine.get_alphabet()
    }

    // ---- the out-of-alphabet overlay ----

    /// The first symbol number the overlay may use. Past every number the
    /// tables can name, since load-time validation bounds those by the symbol
    /// count and the only other grower — `include_symbol_in_alphabet` — runs
    /// before any state can borrow the machine.
    #[inline]
    fn overlay_base(&self) -> SymbolNumber {
        u32::try_from(self.alph().get_symbol_table().len()).expect("value out of u32 range")
            as SymbolNumber
    }

    /// Admit `symbol` to this run, returning the number it was given.
    // [spec:hfst:req:lookup-run-state.out-of-alphabet-overlay]
    fn adopt_symbol(&mut self, symbol: Symbol) -> SymbolNumber {
        let base = self.overlay_base();
        let overlay = self.overlay.get_or_insert_with(AlphabetOverlay::new);
        let k = base + overlay.symbols.len() as SymbolNumber;
        overlay.encoder.read_input_symbol(&symbol, k as i32);
        overlay.symbols.push(symbol);
        k
    }

    /// The string a symbol number stands for, reading the machine's alphabet
    /// first and this run's overlay after it — the one place a number admitted
    /// by [`Self::adopt_symbol`] is turned back into text, since the meta-arc
    /// handling carries such symbols through the tapes as bare numbers.
    // [spec:hfst:req:lookup-run-state.out-of-alphabet-overlay]
    fn symbol_string(&self, symbol: SymbolNumber) -> Symbol {
        let base = self.overlay_base();
        if symbol < base {
            return self.alph().string_from_symbol(symbol);
        }
        self.overlay
            .as_ref()
            .and_then(|o| o.symbols.get((symbol - base) as usize))
            .expect("a symbol number at or past the alphabet was admitted by this run")
            .clone()
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.initialize-input-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.initialize-input-fn]
    // [spec:hfst:req:lookup-run-state.out-of-alphabet-overlay]
    pub fn initialize_input(&mut self, input: &str) -> bool {
        let mut buf: Vec<u8> = input.as_bytes().to_vec();
        buf.push(0);
        let mut i: u32 = 0;
        let mut p: usize = 0;
        while buf[p] != 0 {
            let original_input_loc = p;
            let k = match self.machine.get_encoder().find_key(&buf, &mut p) {
                Some(k) => k,
                None => {
                    // A failed trie descent leaves 'p' one byte in, so every
                    // retry below restarts from the character's first byte.
                    p = original_input_loc;
                    match self.overlay_find_key(&buf, &mut p) {
                        Some(k) => k,
                        None => {
                            // Admit what we assume to be an unknown utf-8 symbol
                            p = original_input_loc;
                            let Some(bytes_to_tokenize) = utf8_sequence_length(buf[p]) else {
                                return false; // tokenization failed
                            };
                            let new_symbol = Symbol::from(String::from_utf8_lossy(
                                &buf[p..p + bytes_to_tokenize],
                            ));
                            p += bytes_to_tokenize;
                            self.adopt_symbol(new_symbol)
                        }
                    }
                }
            };
            self.input_tape.write(i, k);
            i += 1;
        }
        self.input_tape.write(i, NO_SYMBOL_NUMBER);
        true
    }

    fn overlay_find_key(&self, buf: &[u8], p: &mut usize) -> Option<SymbolNumber> {
        self.overlay.as_ref()?.encoder.find_key(buf, p)
    }

    // ---- lookup entry points ----

    pub fn lookup_fd_strvec(
        &mut self,
        s: &StringVector,
        limit: isize,
        time_cutoff: f64,
    ) -> HfstOneLevelPaths {
        let mut input_str = String::new();
        for it in s.iter() {
            input_str.push_str(it);
        }
        self.lookup_fd(&input_str, limit, time_cutoff)
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.lookup-fd-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.lookup-fd-fn]
    // [spec:hfst:req:lookup-run-state.caller-owned-scratch]
    pub fn lookup_fd(&mut self, s: &str, limit: isize, time_cutoff: f64) -> HfstOneLevelPaths {
        self.max_lookups = limit;
        self.max_time = 0.0;
        if time_cutoff > 0.0 {
            self.max_time = time_cutoff;
            self.start_clock = Some(Instant::now());
        }
        let mut results: HfstOneLevelPaths = BTreeSet::new();
        if !self.initialize_input(s) {
            return results;
        }
        self.lookup_paths.clear();
        self.traversal_states.clear();
        self.get_analyses(0, 0, 0);
        let paths = std::mem::take(&mut self.lookup_paths);
        for it in paths.iter() {
            let mut output_path = HfstOneLevelPath {
                first: it.first,
                second: Vec::new(),
            };
            for v_it in it.second.iter() {
                output_path.second.push(v_it.1.clone());
            }
            results.insert(output_path);
        }
        results
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.lookup-fd-pairs-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.lookup-fd-pairs-fn]
    // [spec:hfst:req:lookup-run-state.caller-owned-scratch]
    pub fn lookup_fd_pairs(
        &mut self,
        s: &str,
        limit: isize,
        time_cutoff: f64,
    ) -> HfstTwoLevelPaths {
        self.max_lookups = limit;
        self.max_time = 0.0;
        if time_cutoff > 0.0 {
            self.max_time = time_cutoff;
            self.start_clock = Some(Instant::now());
        }
        self.lookup_paths.clear();
        if !self.initialize_input(s) {
            return std::mem::take(&mut self.lookup_paths);
        }
        self.traversal_states.clear();
        self.get_analyses(0, 0, 0);
        std::mem::take(&mut self.lookup_paths)
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.is-lookup-infinitely-ambiguous-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.is-lookup-infinitely-ambiguous-fn]
    // [spec:hfst:req:lookup-run-state.caller-owned-scratch]
    pub fn is_lookup_infinitely_ambiguous(&mut self, s: &str) -> bool {
        if !self.initialize_input(s) {
            return false;
        }
        self.traversal_states.clear();
        // C++: try { find_loop(0, 0); } catch (bool e) { ... return e; }
        match self.find_loop(0, 0) {
            ControlFlow::Continue(_) => false,
            ControlFlow::Break(()) => {
                self.current_weight = 0.0;
                self.flag_state = self.machine.flag_state_proto().clone();
                true
            }
        }
    }

    pub fn is_lookup_infinitely_ambiguous_strvec(&mut self, s: &StringVector) -> bool {
        let mut input_str = String::new();
        for it in s.iter() {
            input_str.push_str(it);
        }
        self.is_lookup_infinitely_ambiguous(&input_str)
    }

    // ---- the walk ----

    // [spec:hfst:def:transducer.hfst-ol.transducer.try-epsilon-transitions-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.try-epsilon-transitions-fn]
    fn try_epsilon_transitions(
        &mut self,
        input_pos: u32,
        output_pos: u32,
        mut i: TransitionTableIndex,
    ) -> bool {
        let mut found_transition = false;
        loop {
            let input = self.machine.get_transition_input(i);
            let output = self.machine.get_transition_output(i);
            let target = self.machine.get_transition_target(i);
            let weight = self.machine.get_transition_weight(i);
            let old_weight = self.current_weight;
            if input == 0 {
                // epsilon
                //
                // Non-progressing-loop trap (hfst/hfst#293, hfst/hfst#476).
                // The C++ engine only loop-guarded the FLAG branch below; a
                // plain epsilon cycle was left to recurse `MAX_RECURSION_DEPTH`
                // (5000) levels deep, emitting 5000 junk analyses whose weights
                // climbed to ~4999 (the "huge weights before infinity") and
                // running unbounded on large FSTs. Guard it exactly like the
                // flag branch: this `traversal_states` set is DFS-path-scoped
                // (inserted before the recursive call, removed after) and is
                // cleared by `find_transitions`/`get_analyses` whenever a real
                // input symbol is consumed, so it only ever traps a cycle that
                // returns to the same (target, flags) at the SAME input
                // position — never a sibling branch or a genuine re-entry after
                // progress. Convergent analyses therefore survive.
                let epsilon_reachable = TraversalState::new(target, self.flag_state.get_values());
                if self.traversal_states.contains(&epsilon_reachable) {
                    // We've been here before at this input, back out.
                    i += 1;
                    continue;
                }
                // push on enter / pop on leave — the stack top is always the
                // state we pushed, so pop() is the exact counterpart of the old
                // set's remove(&epsilon_reachable).
                self.traversal_states.push(epsilon_reachable);
                self.output_tape.write_pair(output_pos, input, output);
                self.current_weight += weight;
                self.get_analyses(input_pos, output_pos + 1, target);
                found_transition = true;
                self.current_weight = old_weight;
                self.traversal_states.pop();
                i += 1;
            } else if self.alph().is_flag_diacritic(input) {
                let flags = self.flag_state.get_values().clone();
                let op = self
                    .alph()
                    .get_operation(input)
                    .expect("flag diacritic symbol has an operation")
                    .clone();
                if self.flag_state.apply_operation(&op) {
                    // flag diacritic allowed
                    let flag_reachable = TraversalState::new(target, &flags);
                    if self.traversal_states.contains(&flag_reachable) {
                        // We've been here before at this input, back out
                        self.flag_state.assign_values(&flags);
                        i += 1;
                        continue;
                    }
                    self.traversal_states.push(flag_reachable);
                    self.output_tape.write_pair(output_pos, input, output);
                    self.current_weight += weight;
                    self.get_analyses(input_pos, output_pos + 1, target);
                    found_transition = true;
                    self.current_weight = old_weight;
                    self.traversal_states.pop();
                }
                self.flag_state.assign_values(&flags);
                i += 1;
            } else {
                // it's not epsilon and it's not a flag, so nothing to do
                return found_transition;
            }
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.try-epsilon-indices-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.try-epsilon-indices-fn]
    fn try_epsilon_indices(
        &mut self,
        input_pos: u32,
        output_pos: u32,
        i: TransitionTableIndex,
    ) -> bool {
        if self.machine.get_index_input(i) == 0 {
            let target = self.machine.get_index_target(i) - TRANSITION_TARGET_TABLE_START;
            self.try_epsilon_transitions(input_pos, output_pos, target);
            true
        } else {
            false
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.find-transitions-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.find-transitions-fn]
    // [spec:hfst:req:ol-lookup-enumeration.meta-arc-output]
    // [spec:hfst:sem:ol-lookup-enumeration.meta-arc-restriction]
    fn find_transitions(
        &mut self,
        input: SymbolNumber,
        input_pos: u32,
        output_pos: u32,
        mut i: TransitionTableIndex,
    ) -> bool {
        let mut found_transition = false;
        while self.machine.get_transition_input(i) != NO_SYMBOL_NUMBER {
            if self.machine.get_transition_input(i) == input {
                let old_weight = self.current_weight;
                // We're not going to find an epsilon / flag loop
                self.traversal_states.clear();
                let mut output = self.machine.get_transition_output(i);
                if self.alph().is_meta_arc(output) {
                    // we got here via default, identity or unknown, so look back
                    // in the input tape to find the symbol we want to write
                    output = self.input_tape.at(input_pos - 1);
                }
                self.output_tape.write_pair(output_pos, input, output);
                let w = self.machine.get_transition_weight(i);
                self.current_weight += w;
                let target = self.machine.get_transition_target(i);
                self.get_analyses(input_pos, output_pos + 1, target);
                self.current_weight = old_weight;
                found_transition = true;
            } else {
                return found_transition;
            }
            i += 1;
        }
        found_transition
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.find-index-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.find-index-fn]
    fn find_index(
        &mut self,
        input: SymbolNumber,
        input_pos: u32,
        output_pos: u32,
        i: TransitionTableIndex,
    ) -> bool {
        if self.machine.get_index_input(i + input as u32) == input {
            let target =
                self.machine.get_index_target(i + input as u32) - TRANSITION_TARGET_TABLE_START;
            self.find_transitions(input, input_pos, output_pos, target);
            true
        } else {
            false
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.get-analyses-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.get-analyses-fn]
    // [spec:hfst:req:ol-lookup-enumeration.no-internal-work-cap]
    // [spec:hfst:req:ol-lookup-enumeration.representation-independence]
    // [spec:hfst:req:ol-lookup-enumeration.out-of-alphabet-input]
    fn get_analyses(&mut self, input_pos: u32, output_pos: u32, mut i: TransitionTableIndex) {
        let mut found_transition = false;

        if self.recursion_depth_left == 0 {
            return;
        }
        if self.max_lookups >= 0 && self.lookup_paths.len() as isize >= self.max_lookups {
            // Back out because we have enough results already
            return;
        }
        if self.max_time > 0.0 {
            // quit if we've overspent our time
            if let Some(sc) = self.start_clock
                && sc.elapsed().as_secs_f64() > self.max_time
            {
                return;
            }
        }
        self.recursion_depth_left -= 1;
        if indexes_transition_table(i) {
            i -= TRANSITION_TARGET_TABLE_START;
            // First we check for finality and collect the result
            if self.input_tape.at(input_pos) == NO_SYMBOL_NUMBER
                && (self.max_lookups < 0 || (self.lookup_paths.len() as isize) < self.max_lookups)
            {
                self.output_tape
                    .write_pair(output_pos, NO_SYMBOL_NUMBER, NO_SYMBOL_NUMBER);
                if self.machine.get_transition_finality(i) {
                    let old_weight = self.current_weight;
                    let w = self.machine.get_transition_weight(i);
                    self.current_weight += w;
                    self.note_analysis();
                    self.current_weight = old_weight;
                }
            }

            // Then we check epsilons
            found_transition |= self.try_epsilon_transitions(input_pos, output_pos, i + 1);

            if self.input_tape.at(input_pos) == NO_SYMBOL_NUMBER {
                // No more input
                self.recursion_depth_left += 1;
                return;
            }

            let input = self.input_tape.at(input_pos);
            let input_pos = input_pos + 1;

            if input < self.alph().get_orig_symbol_count() {
                // Input is in the alphabet
                found_transition |= self.find_transitions(input, input_pos, output_pos, i + 1);
            } else {
                if self.alph().get_identity_symbol() != NO_SYMBOL_NUMBER {
                    let id = self.alph().get_identity_symbol();
                    found_transition |= self.find_transitions(id, input_pos, output_pos, i + 1);
                }
                if self.alph().get_unknown_symbol() != NO_SYMBOL_NUMBER {
                    let unk = self.alph().get_unknown_symbol();
                    found_transition |= self.find_transitions(unk, input_pos, output_pos, i + 1);
                }
            }
            if self.alph().get_default_symbol() != NO_SYMBOL_NUMBER && !found_transition {
                let def = self.alph().get_default_symbol();
                self.find_transitions(def, input_pos, output_pos, i + 1);
            }
        } else {
            if self.input_tape.at(input_pos) == NO_SYMBOL_NUMBER
                && (self.max_lookups < 0 || (self.lookup_paths.len() as isize) < self.max_lookups)
            {
                self.output_tape
                    .write_pair(output_pos, NO_SYMBOL_NUMBER, NO_SYMBOL_NUMBER);
                if self.machine.get_index_finality(i) {
                    let old_weight = self.current_weight;
                    let w = self.machine.get_index_final_weight(i);
                    self.current_weight += w;
                    self.note_analysis();
                    self.current_weight = old_weight;
                }
            }

            found_transition |= self.try_epsilon_indices(input_pos, output_pos, i + 1);

            if self.input_tape.at(input_pos) == NO_SYMBOL_NUMBER {
                self.recursion_depth_left += 1;
                return;
            }

            let input = self.input_tape.at(input_pos);
            let input_pos = input_pos + 1;

            if input < self.alph().get_orig_symbol_count() {
                // Input is in the alphabet
                found_transition |= self.find_index(input, input_pos, output_pos, i + 1);
            } else {
                if self.alph().get_identity_symbol() != NO_SYMBOL_NUMBER {
                    let id = self.alph().get_identity_symbol();
                    found_transition |= self.find_index(id, input_pos, output_pos, i + 1);
                }
                if self.alph().get_unknown_symbol() != NO_SYMBOL_NUMBER {
                    let unk = self.alph().get_unknown_symbol();
                    found_transition |= self.find_index(unk, input_pos, output_pos, i + 1);
                }
            }
            // If we have a default symbol defined and we didn't find an index,
            // check for that
            if self.alph().get_default_symbol() != NO_SYMBOL_NUMBER && !found_transition {
                let def = self.alph().get_default_symbol();
                self.find_index(def, input_pos, output_pos, i + 1);
            }
        }
        self.output_tape
            .write_pair(output_pos, NO_SYMBOL_NUMBER, NO_SYMBOL_NUMBER);
        self.recursion_depth_left += 1;
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.note-analysis-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.note-analysis-fn]
    fn note_analysis(&mut self) {
        let mut result = HfstTwoLevelPath {
            first: 0.0,
            second: Vec::new(),
        };
        let mut idx = 0usize;
        while self.output_tape.inner[idx].output != NO_SYMBOL_NUMBER {
            let pair = self.output_tape.inner[idx];
            let in_s = self.symbol_string(pair.input);
            let out_s = self.symbol_string(pair.output);
            result.second.push((in_s, out_s));
            idx += 1;
        }
        result.first = self.current_weight;
        self.lookup_paths.insert(result);
    }

    // ---- find_epsilon_loops.cc ----

    // [spec:hfst:def:find-epsilon-loops.hfst-ol.transducer.find-loop-epsilon-transitions-fn]
    // [spec:hfst:sem:find-epsilon-loops.hfst-ol.transducer.find-loop-epsilon-transitions-fn]
    // [spec:hfst:def:transducer.hfst-ol.transducer.find-loop-epsilon-transitions-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.find-loop-epsilon-transitions-fn]
    fn find_loop_epsilon_transitions(
        &mut self,
        input_pos: u32,
        mut i: TransitionTableIndex,
    ) -> ControlFlow<(), bool> {
        let flags = self.flag_state.get_values().clone();
        let mut found_transition = false;
        loop {
            let target = self.machine.get_transition_target(i);
            let epsilon_reachable = TraversalState::new(target, &flags);
            let tin = self.machine.get_transition_input(i);
            if tin == 0 {
                // epsilon
                // We try to trap non-progressing loops
                if self.traversal_states.contains(&epsilon_reachable) {
                    // We've been here before
                    return ControlFlow::Break(());
                }
                self.traversal_states.push(epsilon_reachable.clone());
                self.find_loop(input_pos, target)?;
                self.traversal_states.pop();
                found_transition = true;
                i += 1;
            } else if self.alph().is_flag_diacritic(tin) {
                let op = self
                    .alph()
                    .get_operation(tin)
                    .expect("flag diacritic symbol has an operation")
                    .clone();
                if self.flag_state.apply_operation(&op) {
                    // flag diacritic allowed
                    if self.traversal_states.contains(&epsilon_reachable) {
                        // We've been here before
                        return ControlFlow::Break(());
                    }
                    self.traversal_states.push(epsilon_reachable.clone());
                    // C++ leak preserved: the shared field took the nested
                    // call's exit value here (no unconditional set like the
                    // epsilon arm), so this REPLACES the accumulator.
                    found_transition = self.find_loop(input_pos, target)?;
                    self.traversal_states.pop();
                }
                self.flag_state.assign_values(&flags);
                i += 1;
            } else {
                // it's not epsilon and it's not a flag, so nothing to do
                return ControlFlow::Continue(found_transition);
            }
        }
    }

    // [spec:hfst:def:find-epsilon-loops.hfst-ol.transducer.find-loop-epsilon-indices-fn]
    // [spec:hfst:sem:find-epsilon-loops.hfst-ol.transducer.find-loop-epsilon-indices-fn]
    // [spec:hfst:def:transducer.hfst-ol.transducer.find-loop-epsilon-indices-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.find-loop-epsilon-indices-fn]
    fn find_loop_epsilon_indices(
        &mut self,
        input_pos: u32,
        i: TransitionTableIndex,
    ) -> ControlFlow<(), bool> {
        if self.machine.get_index_input(i) == 0 {
            let target = self.machine.get_index_target(i) - TRANSITION_TARGET_TABLE_START;
            self.find_loop_epsilon_transitions(input_pos, target)?;
            ControlFlow::Continue(true)
        } else {
            ControlFlow::Continue(false)
        }
    }

    // [spec:hfst:def:find-epsilon-loops.hfst-ol.transducer.find-loop-transitions-fn]
    // [spec:hfst:sem:find-epsilon-loops.hfst-ol.transducer.find-loop-transitions-fn]
    // [spec:hfst:def:transducer.hfst-ol.transducer.find-loop-transitions-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.find-loop-transitions-fn]
    fn find_loop_transitions(
        &mut self,
        input: SymbolNumber,
        input_pos: u32,
        mut i: TransitionTableIndex,
    ) -> ControlFlow<(), bool> {
        let mut found_transition = false;
        while self.machine.get_transition_input(i) != NO_SYMBOL_NUMBER {
            if self.machine.get_transition_input(i) == input {
                // We're not going to find an epsilon / flag loop
                self.traversal_states.clear();
                let target = self.machine.get_transition_target(i);
                self.find_loop(input_pos, target)?;
                found_transition = true;
            } else {
                return ControlFlow::Continue(found_transition);
            }
            i += 1;
        }
        ControlFlow::Continue(found_transition)
    }

    // [spec:hfst:def:find-epsilon-loops.hfst-ol.transducer.find-loop-index-fn]
    // [spec:hfst:sem:find-epsilon-loops.hfst-ol.transducer.find-loop-index-fn]
    // [spec:hfst:def:transducer.hfst-ol.transducer.find-loop-index-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.find-loop-index-fn]
    fn find_loop_index(
        &mut self,
        input: SymbolNumber,
        input_pos: u32,
        i: TransitionTableIndex,
    ) -> ControlFlow<(), bool> {
        // A symbol beyond this transducer's input alphabet (e.g. one that only
        // exists in another cascade member) has no index transition: the index
        // table is padded only up to input_symbol_count, so gate the lookup on
        // it rather than indexing out of bounds.
        if input < self.hdr().input_symbol_count()
            && self.machine.get_index_input(i + input as u32) == input
        {
            let target =
                self.machine.get_index_target(i + input as u32) - TRANSITION_TARGET_TABLE_START;
            self.find_loop_transitions(input, input_pos, target)?;
            ControlFlow::Continue(true)
        } else {
            ControlFlow::Continue(false)
        }
    }

    // [spec:hfst:def:find-epsilon-loops.hfst-ol.transducer.find-loop-fn]
    // [spec:hfst:sem:find-epsilon-loops.hfst-ol.transducer.find-loop-fn]
    // [spec:hfst:def:transducer.hfst-ol.transducer.find-loop-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.find-loop-fn]
    fn find_loop(&mut self, input_pos: u32, mut i: TransitionTableIndex) -> ControlFlow<(), bool> {
        let mut found_transition = false;

        if indexes_transition_table(i) {
            i -= TRANSITION_TARGET_TABLE_START;
            found_transition |= self.find_loop_epsilon_transitions(input_pos, i + 1)?;

            // input-string ended.
            if self.input_tape.at(input_pos) == NO_SYMBOL_NUMBER {
                return ControlFlow::Continue(found_transition);
            }

            let input = self.input_tape.at(input_pos);
            let input_pos = input_pos + 1;

            found_transition |= self.find_loop_transitions(input, input_pos, i + 1)?;
            if self.alph().get_default_symbol() != NO_SYMBOL_NUMBER && !found_transition {
                let def = self.alph().get_default_symbol();
                found_transition |= self.find_loop_transitions(def, input_pos, i + 1)?;
            }
        } else {
            found_transition |= self.find_loop_epsilon_indices(input_pos, i + 1)?;

            if self.input_tape.at(input_pos) == NO_SYMBOL_NUMBER {
                // input-string ended.
                return ControlFlow::Continue(found_transition);
            }

            let input = self.input_tape.at(input_pos);
            let input_pos = input_pos + 1;

            found_transition |= self.find_loop_index(input, input_pos, i + 1)?;
            // If we have a default symbol defined and we didn't find an index,
            // check for that
            if self.alph().get_default_symbol() != NO_SYMBOL_NUMBER && !found_transition {
                let def = self.alph().get_default_symbol();
                found_transition |= self.find_loop_index(def, input_pos, i + 1)?;
            }
        }
        ControlFlow::Continue(found_transition)
    }
}
