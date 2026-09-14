//! The per-run half of the pmatch runtime.
//!
//! The C++ `hfst_ol::PmatchContainer` kept its tapes, entry and RTN stacks,
//! capture bookkeeping, global flag state and line counter on the same object as
//! the loaded archive, and each `PmatchTransducer` carried the local-variable
//! stack of whichever walk happened to be running it. A match therefore took the
//! whole archive exclusively, and a re-entrant RTN return had to clone the net
//! it was returning into, because the live one was already borrowed higher on
//! the Rust stack (hfst/hfst#354).
//!
//! Here the archive is a [`PmatchCore`] shared behind an `Arc` and
//! [`PmatchContainer`] is one run over it: everything a match writes lives on
//! the run state, and a [`PmatchWalk`] pairs a *borrowed* net with the
//! local-variable stack of that one invocation. Re-entering a net is then just a
//! second shared borrow, so nothing is cloned to make it possible.
// [spec:hfst:req:lookup-run-state.pmatch-shared-core]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use tracing::{debug, warn};

use crate::hfst_data_types::Symbol;
use crate::hfst_flag_diacritics::FdState;
use crate::pmatch::{
    Capture, ContextChecking, LocalVariables, LocalVariablesStack, Location, LocationVector,
    LocationVectorVector, PmatchTransducer, RtnCallStacks, RtnStackFrame, SpecialSymbol,
    WeightedDoubleTapeVector, counter_comp,
};
use crate::pmatch_core::{PmatchCore, PmatchProperties, SymbolAdmitter, tokenize_input};
use crate::transducer::{
    DoubleTape, Encoder, NO_COUNTER, NO_SYMBOL_NUMBER, SymbolNumber, SymbolNumberVector,
    SymbolTable, TransitionTableIndex, UnicodeClassCacheValue, Weight, WeightedDoubleTape,
    unicode_class_of,
};

// [spec:hfst:def:pmatch.hfst-ol.pmatch-container.epsilon-visit-key]
// Identifies a plain-epsilon configuration for the #399 cycle guard.
pub(crate) type EpsilonVisitKey = (SymbolNumber, TransitionTableIndex, u32, u8, i8);

/// Input symbols admitted during a run because the container's alphabet has no
/// tokenization for them.
///
/// The C++ pushed such a symbol straight into the alphabet and the encoder,
/// which made matching a text a write to the loaded archive. The symbols are
/// numbered from the alphabet's post-load symbol count upward — past every
/// number the tables can name — so the engine still routes them onto
/// identity/unknown arcs exactly as it did when the alphabet grew, and the
/// parallel per-symbol vectors answer for them the way `add_symbol` left them.
// [spec:hfst:req:lookup-run-state.pmatch-shared-core]
struct AlphabetOverlay {
    /// Adopted symbols in numbering order; entry `n` is symbol `base + n`.
    symbols: Vec<Symbol>,
    /// Tokenizes only the adopted spellings. Consulted after the container's own
    /// encoder, so a spelling the container can tokenize always wins — which is
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

/// One run over a loaded pmatch archive: the caller-owned scratch of a match,
/// plus a shared handle on the [`PmatchCore`] it matches against.
///
/// Created against a core and reusable across calls — the tapes keep their
/// capacity, the pattern counts and profiling counters accumulate, and an
/// alphabet overlay built by one line is still there for the next. Nothing here
/// is visible to another run state, so several of these can match different
/// texts against one archive at the same time with no lock around the tables.
// [spec:hfst:def:pmatch.hfst-ol.pmatch-container]
// [spec:hfst:req:lookup-run-state.pmatch-shared-core]
// weight_limit is currently read only on paths not yet exercised by a test.
#[allow(dead_code)]
pub struct PmatchContainer {
    pub(crate) core: Arc<PmatchCore>,
    pub(crate) input: SymbolNumberVector,
    // This tracks the ENTRY and EXIT tags
    pub(crate) entry_stack: Vec<u32>,
    pub(crate) rtn_stacks: RtnCallStacks,
    pub(crate) tape: DoubleTape,
    pub(crate) best_result: DoubleTape,
    pub(crate) result: DoubleTape,
    pub(crate) locations: LocationVectorVector,
    pub(crate) tape_locations: WeightedDoubleTapeVector,
    pub(crate) captures: Vec<Capture>,
    pub(crate) best_captures: Vec<Capture>,
    pub(crate) old_captures: Vec<Capture>,
    // The flag state for global flags
    pub(crate) global_flag_state: FdState<SymbolNumber>,
    pub(crate) verbose: bool,

    /// This run's working copy of the archive's declared behaviour; the `set_*`
    /// knobs move this, never the core.
    pub(crate) props: PmatchProperties,

    pub(crate) line_number: u64,
    pub(crate) pattern_counts: BTreeMap<String, usize>,
    /// Traversal tallies for `Counter()` positions, so profiling one run never
    /// shows another run's work. Copied off the core's counter template — which
    /// is as long as the symbol table — on the first tally, and left empty for
    /// the overwhelmingly common run that is not profiling.
    pub(crate) counters: Vec<u64>,
    pub(crate) profile_mode: bool,
    pub(crate) single_codepoint_tokenization: bool,
    pub(crate) recursion_depth_left: u32,
    // An optional time limit for operations
    pub(crate) max_time: f64,
    // When we started work
    pub(crate) start_clock: Option<Instant>,
    // A counter to avoid checking the clock too often
    pub(crate) call_counter: u64,
    // A flag to set for when time has been overstepped
    pub(crate) limit_reached: bool,
    // Weight cutoff
    pub(crate) max_weight: Weight,
    // The global running weight
    pub(crate) running_weight: Weight,
    pub(crate) weight_limit: Weight,
    // This is the depth of the stack from the point of view of the
    // container. When it's 0, we're in the toplevel, even if the
    // stack of variables is bigger due to having passed through a RTN.
    pub(crate) stack_depth: u32,
    // Where in the input the best candidate so far has gotten to
    pub(crate) best_input_pos: u32,
    pub(crate) best_weight: Weight,
    // [PORT NOTE / DIVERGENCE hfst/hfst#399]
    // Current DFS path of plain-epsilon-input configurations. Keyed on
    // (transducer symbol, target transition index, input_pos, context mode,
    // tape direction). Re-entering the same key is pruned. This TERMINATES
    // grammars whose compiled net contains an epsilon-input / unknown-output
    // arc under Kleene star (e.g. `0:?*`), which the C++ engine loops on
    // forever (still-open upstream bug hfst/hfst#399). Guarded strictly to
    // plain (non-flag, non-Ins/RTN) epsilon arcs so flag-diacritic traversal
    // and RTN calls keep their own state and are never pruned by this path.
    pub(crate) epsilon_path: Vec<EpsilonVisitKey>,
    /// Memo of the Unicode class of each symbol, which the C++ kept on the
    /// alphabet — asking the question used to be a write to the loaded archive.
    pub(crate) unicode_cache: Vec<UnicodeClassCacheValue>,
    /// Built on first admission; absent for the overwhelmingly common run whose
    /// input the container can tokenize on its own.
    overlay: Option<AlphabetOverlay>,
}

impl Default for PmatchContainer {
    fn default() -> Self {
        Self::new()
    }
}

/// Tokenizing during a run records what the container cannot spell in this
/// run's overlay, leaving the shared alphabet and encoder untouched.
// [spec:hfst:req:lookup-run-state.pmatch-shared-core]
impl SymbolAdmitter for PmatchContainer {
    fn find_key(&self, buf: &[u8], p: &mut usize) -> Option<SymbolNumber> {
        let start = *p;
        if let Some(k) = self
            .core
            .encoder
            .as_ref()
            .expect("encoder is initialized during container load")
            .find_key(buf, p)
        {
            return Some(k);
        }
        // A failed trie descent leaves 'p' part-way in, so the overlay retry
        // restarts from the spelling's first byte.
        *p = start;
        if let Some(overlay) = self.overlay.as_ref()
            && let Some(k) = overlay.encoder.find_key(buf, p)
        {
            return Some(k);
        }
        *p = start;
        None
    }

    fn admit(&mut self, symbol: Symbol) -> SymbolNumber {
        let base = self.core.symbol_count;
        let overlay = self.overlay.get_or_insert_with(AlphabetOverlay::new);
        let k = base + overlay.symbols.len() as SymbolNumber;
        overlay.encoder.read_input_symbol(&symbol, k as i32);
        overlay.symbols.push(symbol);
        k
    }
}

impl PmatchContainer {
    // PmatchContainer(void)
    // Not used, but apparently needed by swig to construct these
    pub fn new() -> PmatchContainer {
        PmatchContainer::from_core(Arc::new(PmatchCore::new()))
    }

    /// A fresh run over an already-loaded archive. The core is shared, not
    /// copied, so this is the constructor a second thread calls.
    // [spec:hfst:req:lookup-run-state.pmatch-shared-core]
    pub fn from_core(core: Arc<PmatchCore>) -> PmatchContainer {
        PmatchContainer {
            input: SymbolNumberVector::new(),
            entry_stack: Vec::new(),
            rtn_stacks: RtnCallStacks::new(),
            tape: DoubleTape::new(),
            best_result: DoubleTape::new(),
            result: DoubleTape::new(),
            locations: LocationVectorVector::new(),
            tape_locations: WeightedDoubleTapeVector::new(),
            captures: Vec::new(),
            best_captures: Vec::new(),
            old_captures: Vec::new(),
            global_flag_state: core.flag_state_proto.clone(),
            verbose: false,
            props: core.props,
            line_number: 0,
            pattern_counts: BTreeMap::new(),
            counters: Vec::new(),
            profile_mode: false,
            single_codepoint_tokenization: false,
            recursion_depth_left: core.props.max_recursion as u32,
            max_time: 0.0,
            start_clock: None,
            call_counter: 0,
            limit_reached: false,
            max_weight: crate::transducer::INFINITE_WEIGHT,
            running_weight: 0.0,
            weight_limit: crate::transducer::INFINITE_WEIGHT,
            stack_depth: 0,
            best_input_pos: 0,
            best_weight: 0.0,
            epsilon_path: Vec::new(),
            unicode_cache: Vec::new(),
            overlay: None,
            core,
        }
    }

    /// A handle on the loaded archive, for starting further runs over it.
    // [spec:hfst:req:lookup-run-state.pmatch-shared-core]
    pub fn core(&self) -> Arc<PmatchCore> {
        Arc::clone(&self.core)
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.pmatch-container-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.pmatch-container-fn]
    pub fn new_from_stream(
        is: &mut crate::transducer::IStream<'_>,
    ) -> crate::error::Result<PmatchContainer> {
        Ok(PmatchContainer::from_core(Arc::new(
            PmatchCore::from_stream(is)?,
        )))
    }

    // PmatchContainer(Transducer *t)
    pub fn new_from_transducer(
        toplevel: crate::transducer::Transducer,
    ) -> crate::error::Result<PmatchContainer> {
        Ok(PmatchContainer::from_core(Arc::new(
            PmatchCore::from_transducer(toplevel)?,
        )))
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.pmatch-container-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.pmatch-container-fn]
    // explicit PmatchContainer(std::vector<hfst::HfstTransducer>)
    pub fn new_from_hfst_transducers(
        transducers: Vec<crate::hfst_transducer::HfstTransducer<crate::transducer::Transducer>>,
    ) -> crate::error::Result<PmatchContainer> {
        Ok(PmatchContainer::from_core(Arc::new(
            PmatchCore::from_hfst_transducers(transducers)?,
        )))
    }

    // void set_properties(void)
    pub fn set_properties(&mut self) {
        self.props = PmatchProperties::new();
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-properties-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-properties-fn]
    pub fn set_properties_map(&mut self, properties: &BTreeMap<String, String>) {
        self.props.set_from_map(properties);
    }

    // ---- overlay-aware views of the shared alphabet ----
    //
    // A symbol admitted by this run sits past everything the core's parallel
    // per-symbol vectors cover, so each of these answers for it the way
    // 'PmatchAlphabet::add_symbol' left the grown vectors.

    /// The first symbol number this run's overlay may use.
    #[inline]
    fn overlay_base(&self) -> SymbolNumber {
        self.core.symbol_count
    }

    // [spec:hfst:req:lookup-run-state.pmatch-shared-core]
    fn symbol_string(&self, symbol: SymbolNumber) -> Symbol {
        let base = self.overlay_base();
        if symbol < base {
            return self.core.alphabet.string_from_symbol(symbol);
        }
        self.overlay
            .as_ref()
            .and_then(|o| o.symbols.get((symbol - base) as usize))
            .expect("a symbol number at or past the alphabet was admitted by this run")
            .clone()
    }

    /// `add_symbol` pushes `true`, so everything a run admits prints.
    #[inline]
    fn is_printable_sym(&self, symbol: SymbolNumber) -> bool {
        (symbol as usize) >= self.core.alphabet.printable_vector.len()
            || self.core.alphabet.printable_vector[symbol as usize]
    }

    /// Which symbol lists could admit `symbol`, as an index into
    /// [`Self::symbol_list`].
    ///
    /// `add_symbol` enrols a new symbol in every exclusionary list, giving it a
    /// fresh `symbol_lists` entry holding a copy of `exclusionary_lists`. Every
    /// admitted symbol gets the same contents, so one index past the end of the
    /// core's table stands for all of them.
    #[inline]
    fn symbol2lists(&self, symbol: SymbolNumber) -> SymbolNumber {
        let alphabet = &self.core.alphabet;
        if (symbol as usize) < alphabet.symbol2lists.len() {
            return alphabet.symbol2lists[symbol as usize];
        }
        if alphabet.exclusionary_lists.is_empty() {
            return NO_SYMBOL_NUMBER;
        }
        u16::try_from(alphabet.symbol_lists.len()).expect("value out of u16 range")
    }

    #[inline]
    fn symbol_list(&self, index: SymbolNumber) -> &SymbolNumberVector {
        let alphabet = &self.core.alphabet;
        alphabet
            .symbol_lists
            .get(index as usize)
            .unwrap_or(&alphabet.exclusionary_lists)
    }

    #[inline]
    fn list2symbols(&self, symbol: SymbolNumber) -> SymbolNumber {
        self.core
            .alphabet
            .list2symbols
            .get(symbol as usize)
            .copied()
            .unwrap_or(NO_SYMBOL_NUMBER)
    }

    #[inline]
    fn is_capture_tag_sym(&self, symbol: SymbolNumber) -> bool {
        self.core
            .alphabet
            .capture2captured
            .get(symbol as usize)
            .is_some_and(|c| *c != NO_SYMBOL_NUMBER)
    }

    #[inline]
    fn is_captured_tag_sym(&self, symbol: SymbolNumber) -> bool {
        self.core
            .alphabet
            .captured2capture
            .get(symbol as usize)
            .is_some_and(|c| *c != NO_SYMBOL_NUMBER)
    }

    #[inline]
    fn captured2capture(&self, symbol: SymbolNumber) -> SymbolNumber {
        self.core
            .alphabet
            .captured2capture
            .get(symbol as usize)
            .copied()
            .unwrap_or(NO_SYMBOL_NUMBER)
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.cache-unicode-class-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.cache-unicode-class-fn]
    fn unicode_class(&mut self, symbol: SymbolNumber) -> UnicodeClassCacheValue {
        while self.unicode_cache.len() <= symbol as usize {
            self.unicode_cache.push(UnicodeClassCacheValue::no_value);
        }
        if self.unicode_cache[symbol as usize] != UnicodeClassCacheValue::no_value {
            return self.unicode_cache[symbol as usize];
        }
        let class = unicode_class_of(&self.symbol_string(symbol));
        self.unicode_cache[symbol as usize] = class;
        class
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.is-unicode-alpha-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.is-unicode-alpha-fn]
    fn is_unicode_alpha(&mut self, symbol: SymbolNumber) -> bool {
        let class = self.unicode_class(symbol);
        class == UnicodeClassCacheValue::loweralpha || class == UnicodeClassCacheValue::upperalpha
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.is-unicode-upperalpha-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.is-unicode-upperalpha-fn]
    fn is_unicode_upperalpha(&mut self, symbol: SymbolNumber) -> bool {
        self.unicode_class(symbol) == UnicodeClassCacheValue::upperalpha
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.is-unicode-loweralpha-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.is-unicode-loweralpha-fn]
    fn is_unicode_loweralpha(&mut self, symbol: SymbolNumber) -> bool {
        self.unicode_class(symbol) == UnicodeClassCacheValue::loweralpha
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.is-unicode-whitespace-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.is-unicode-whitespace-fn]
    fn is_unicode_whitespace(&mut self, symbol: SymbolNumber) -> bool {
        self.unicode_class(symbol) == UnicodeClassCacheValue::whitespace
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.count-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.count-fn]
    fn count(&mut self, sym: SymbolNumber) {
        if self.core.alphabet.is_counter_sym(sym) {
            if self.counters.is_empty() {
                self.counters = self.core.alphabet.counters.clone();
            }
            self.counters[sym as usize] += 1;
        }
    }

    /// This run's tallies, or the untouched template when it never counted.
    #[inline]
    fn counters(&self) -> &[u64] {
        if self.counters.is_empty() {
            &self.core.alphabet.counters
        } else {
            &self.counters
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.initialize-input-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.initialize-input-fn]
    // [spec:hfst:req:lookup-run-state.pmatch-shared-core]
    pub fn initialize_input(&mut self, input_s: &str) {
        let boundary = self.core.alphabet.get_special(SpecialSymbol::boundary);
        let single = self.single_codepoint_tokenization;
        // The tokenizer writes the symbols it produces straight into 'input',
        // which it cannot borrow while admitting into 'self'; hand it the vector
        // and put it back.
        let mut input = std::mem::take(&mut self.input);
        tokenize_input(self, boundary, single, input_s, &mut input);
        self.input = input;
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.has-unsatisfied-rtns-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.has-unsatisfied-rtns-fn]
    pub fn has_unsatisfied_rtns(&self) -> bool {
        false
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.get-unsatisfied-rtn-name-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.get-unsatisfied-rtn-name-fn]
    pub fn get_unsatisfied_rtn_name(&self) -> String {
        String::new()
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.process-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.process-fn]
    // [spec:hfst:req:lookup-run-state.pmatch-shared-core]
    pub fn process(&mut self, input: &str) {
        if self.verbose {
            debug!("PC::processing {}", input);
        }
        self.initialize_input(input);
        let mut input_pos: u32 = 0;
        let mut printable_input_pos: u32 = 0;
        self.running_weight = 0.0;
        self.stack_depth = 0;
        self.best_input_pos = 0;

        self.line_number += 1;
        self.result.inner.clear();
        self.locations.clear();
        self.old_captures.clear();
        self.best_captures.clear();
        self.captures.clear();
        self.reset_recursion();
        // A handle of our own on the archive, so the walk can borrow nets out of
        // it while the run state below is borrowed exclusively.
        let core = Arc::clone(&self.core);
        let toplevel = core
            .toplevel
            .as_deref()
            .expect("toplevel present for match");
        let mut nonmatching_locations = DoubleTape::new();
        while self.has_queued_input(input_pos) {
            self.best_result.inner.clear();
            let current_input = self.input[input_pos as usize];
            if self.core.not_possible_first_symbol(current_input) {
                self.copy_to_result_syms(current_input, current_input);
                input_pos += 1;
                if self.props.locate_mode && self.is_printable_sym(current_input) {
                    printable_input_pos += 1;
                    nonmatching_locations
                        .inner
                        .push(crate::transducer::SymbolPair::new_values(
                            current_input,
                            current_input,
                        ));
                }
                continue;
            }
            self.tape.inner.clear();
            self.tape_locations.clear();
            let tape_pos: u32 = 0;
            let old_input_pos = input_pos;
            // toplevel->match(input_pos, tape_pos);
            PmatchWalk::entering(&core, toplevel).do_match(input_pos, tape_pos, self);
            if self.candidate_found() {
                // We got some output
                if self.props.locate_mode {
                    // First we put into the locations vector all the
                    // nonmatching parts we've seen
                    if !nonmatching_locations.inner.is_empty() {
                        let mut ls: LocationVector = LocationVector::new();
                        let mut nonmatching = self.locatefy(
                            printable_input_pos
                                - u32::try_from(nonmatching_locations.inner.len())
                                    .expect("value out of u32 range"),
                            &WeightedDoubleTape::new(nonmatching_locations.clone(), 0.0),
                        );
                        nonmatching.output = "@_NONMATCHING_@".to_string();
                        if self.verbose {
                            debug!("non-matching {}", nonmatching.input);
                        }
                        ls.push(nonmatching);
                        self.locations.push(ls);
                        nonmatching_locations.inner.clear();
                    }
                    let mut ls: LocationVector = LocationVector::new();
                    let tape_locations = self.tape_locations.clone();
                    for it in tape_locations.iter() {
                        let l = self.locatefy(printable_input_pos, it);
                        if self.verbose {
                            debug!("located? {}:{}", l.input, l.output);
                        }
                        ls.push(l);
                    }
                    ls.sort();
                    // The walk can reach one accepting configuration through
                    // several structurally distinct paths (e.g. a union branch
                    // carrying an extra EndTag), and 'locatefy' projects away
                    // every non-printable, non-endtag symbol, so those paths
                    // collapse to byte-identical Locations. Each is reported
                    // separately, exactly as C++ does: the multiplicity of a
                    // reading inside a cohort is part of the Constraint Grammar
                    // contract that consumers of '-c'/'-g' depend on, so the
                    // vector is passed through unfiltered. (Upstream offers an
                    // opt-in '-u' flag for callers who want uniqueness; it is
                    // not applied here by default.)
                    self.locations.push(ls);
                    printable_input_pos += self.best_input_pos - old_input_pos;
                } else {
                    let best_result = self.best_result.clone();
                    self.copy_to_result(&best_result);
                }
                input_pos = self.best_input_pos;
                let best_captures = std::mem::take(&mut self.best_captures);
                self.old_captures.extend(best_captures.iter().cloned());
                self.best_captures = best_captures;
            }
            if !self.candidate_found() || input_pos == old_input_pos {
                // If no input was consumed, we move one position up
                if self.verbose {
                    debug!("no candidate found");
                }
                self.copy_to_result_syms(current_input, current_input);
                input_pos += 1;
                if self.props.locate_mode && self.is_printable_sym(current_input) {
                    printable_input_pos += 1;
                    nonmatching_locations
                        .inner
                        .push(crate::transducer::SymbolPair::new_values(
                            current_input,
                            current_input,
                        ));
                }
            }
        }
        if self.props.locate_mode && !nonmatching_locations.inner.is_empty() {
            let mut ls: LocationVector = LocationVector::new();
            let mut nonmatching = self.locatefy(
                printable_input_pos
                    - u32::try_from(nonmatching_locations.inner.len())
                        .expect("value out of u32 range"),
                &WeightedDoubleTape::new(nonmatching_locations.clone(), 0.0),
            );
            nonmatching.output = "@_NONMATCHING_@".to_string();
            if self.verbose {
                debug!("nonmatching somethign or other{}", nonmatching.input);
            }
            ls.push(nonmatching);
            self.locations.push(ls);
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.match-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.match-fn]
    pub fn do_match(&mut self, input: &str, time_cutoff: f64, weight_cutoff: Weight) -> String {
        self.max_time = time_cutoff;
        self.max_weight = weight_cutoff;
        if self.max_time > 0.0 {
            self.start_clock = Some(Instant::now());
            self.call_counter = 0;
            self.limit_reached = false;
        }
        self.props.locate_mode = false;
        self.process(input);
        let result = self.result.clone();
        self.stringify(&result)
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.locate-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.locate-fn]
    pub fn locate(
        &mut self,
        input: &str,
        time_cutoff: f64,
        weight_cutoff: Weight,
    ) -> LocationVectorVector {
        if self.verbose {
            debug!("locating {}", input);
        }
        self.max_time = time_cutoff;
        self.max_weight = weight_cutoff;
        if self.max_time > 0.0 {
            self.start_clock = Some(Instant::now());
            self.call_counter = 0;
            self.limit_reached = false;
        }
        self.props.locate_mode = true;
        self.process(input);
        self.locations.clone()
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.stringify-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.stringify-fn]
    // The C++ calls 'alphabet.stringify(.., this)': the alphabet reads its own
    // symbol tables while writing the container's pattern counts. Here the tape
    // is a run's, and so are the counts and the overlay that names any symbol
    // this run admitted, so the whole rendering belongs to the run state.
    fn stringify(&mut self, str: &DoubleTape) -> String {
        let mut retval = String::new();
        let mut start_tag_pos: Vec<u32> = Vec::new();
        let mut input_contained_printable_symbol = false;
        for it in str.inner.clone().iter() {
            if !input_contained_printable_symbol && self.is_printable_sym(it.input) {
                input_contained_printable_symbol = true;
            }
            let output = it.output;
            if output == self.core.alphabet.get_special(SpecialSymbol::entry) {
                start_tag_pos.push(u32::try_from(retval.len()).expect("value out of u32 range"));
            } else if output == self.core.alphabet.get_special(SpecialSymbol::exit) {
                if !start_tag_pos.is_empty() {
                    start_tag_pos.pop();
                }
            } else if self.core.alphabet.is_end_tag_sym(output) {
                if self.props.count_patterns && input_contained_printable_symbol {
                    let key = self.core.alphabet.start_tag(output);
                    *self.pattern_counts.entry(key).or_insert(0) += 1;
                }
                let pos: u32 = if start_tag_pos.is_empty() {
                    warn!("end tag without start tag");
                    0
                } else {
                    *start_tag_pos
                        .last()
                        .expect("stack non-empty in else branch")
                };
                if self.props.delete_patterns {
                    let how_much_to_delete = retval.len() - pos as usize;
                    retval.replace_range(
                        pos as usize..pos as usize + how_much_to_delete,
                        &self.core.alphabet.start_tag(output),
                    );
                } else if self.props.mark_patterns && input_contained_printable_symbol {
                    retval.insert_str(pos as usize, &self.core.alphabet.start_tag(output));
                    retval.push_str(&self.core.alphabet.end_tag(output));
                }
            } else if (!self.props.extract_patterns || !start_tag_pos.is_empty())
                && self.is_printable_sym(output)
            {
                retval.push_str(&self.symbol_string(output));
            }
        }
        retval
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.locatefy-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.locatefy-fn]
    fn locatefy(&mut self, input_offset: u32, str: &WeightedDoubleTape) -> Location {
        let mut retval = Location {
            start: input_offset,
            weight: str.weight,
            ..Default::default()
        };
        let mut input_offset = input_offset;
        let mut input_mark: usize = 0;
        let mut output_mark: usize = 0;

        // We rebuild the original input without special
        // symbols but with IDENTITIES etc. replaced
        for it in str.tape.inner.iter() {
            let input = it.input;
            let output = it.output;
            if self.core.alphabet.is_end_tag_sym(output) {
                if self.props.count_patterns {
                    let key = self.core.alphabet.start_tag(output);
                    *self.pattern_counts.entry(key).or_insert(0) += 1;
                }
                retval.tag = self.core.alphabet.start_tag(output);
                continue;
            }
            if self.is_printable_sym(output) {
                let s = self.symbol_string(output);
                retval.output.push_str(&s);
                retval.output_symbol_strings.push(s);
            }
            if self.is_printable_sym(input) {
                let s = self.symbol_string(input);
                retval.input.push_str(&s);
                retval.input_symbol_strings.push(s);
                input_offset += 1;
            }
            if self.core.alphabet.is_input_mark(output) {
                retval.output_parts.push(output_mark);
                retval.input_parts.push(input_mark);
                output_mark = retval.output_symbol_strings.len();
                input_mark = retval.input_symbol_strings.len();
            }
        }
        if output_mark > 0 {
            retval.output_parts.push(output_mark);
        }
        if input_mark > 0 {
            retval.input_parts.push(input_mark);
        }
        retval.length = input_offset - retval.start;
        retval
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.note-analysis-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.note-analysis-fn]
    pub fn note_analysis(&mut self, input_pos: u32, tape_pos: u32) {
        if (input_pos > self.best_input_pos)
            || (input_pos == self.best_input_pos && self.best_weight > self.running_weight)
        {
            self.best_result = self.tape.extract_slice(0, tape_pos);
            self.best_captures = self.captures.clone();
            self.best_input_pos = input_pos;
            self.best_weight = self.running_weight;
        } else if self.verbose
            && input_pos == self.best_input_pos
            && self.best_weight == self.running_weight
        {
            let discarded = self.tape.extract_slice(0, tape_pos);
            let best_result = self.best_result.clone();
            let kept = self.stringify(&best_result);
            let disc = self.stringify(&discarded);
            debug!(
                "\n\tline {}: conflicting equally weighted matches found, keeping:\n\t{}\n\tdiscarding:\n\t{}\n",
                self.line_number, kept, disc
            );
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.grab-location-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.grab-location-fn]
    pub fn grab_location(&mut self, input_pos: u32, tape_pos: u32) {
        if !self.tape_locations.is_empty() {
            if input_pos < self.best_input_pos {
                // We already have better matches
                return;
            } else if input_pos > self.best_input_pos {
                // The old locations are worse
                self.best_captures.clear();
                self.tape_locations.clear();
            }
        }
        self.best_input_pos = input_pos;
        self.best_captures = self.captures.clone();
        let rv = WeightedDoubleTape::new(self.tape.extract_slice(0, tape_pos), self.running_weight);
        self.tape_locations.push(rv);
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.get-longest-matching-capture-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.get-longest-matching-capture-fn]
    // C++ returns a pair of iterators into 'input'; we return the (begin, end)
    // indices into 'self.input' instead (an empty match is begin == end).
    pub fn get_longest_matching_capture(
        &mut self,
        key: SymbolNumber,
        input_pos: u32,
    ) -> (usize, usize) {
        // longest_so_far(input.begin(), input.begin())
        let mut longest_so_far: (usize, usize) = (0, 0);
        let captures = self.captures.clone();
        for it in captures.iter() {
            if key == it.name
                && self.input_matches_at(input_pos, it.begin as usize, it.end as usize)
                && (it.end - it.begin) as usize > longest_so_far.1 - longest_so_far.0
            {
                longest_so_far.0 = it.begin as usize;
                longest_so_far.1 = it.end as usize;
            }
        }
        let old_captures = self.old_captures.clone();
        for it in old_captures.iter() {
            if key == it.name
                && self.input_matches_at(input_pos, it.begin as usize, it.end as usize)
                && (it.end - it.begin) as usize > longest_so_far.1 - longest_so_far.0
            {
                longest_so_far.0 = it.begin as usize;
                longest_so_far.1 = it.end as usize;
            }
        }
        longest_so_far
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.get-profiling-info-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.get-profiling-info-fn]
    pub fn get_profiling_info(&mut self) -> String {
        let mut retval = String::new();
        let mut max_name_len: usize = 0;
        retval.push_str("Profiling information:\n");
        retval.push_str("  Traversals of Counter() positions:\n");
        let mut counter_name_val_pairs: Vec<(String, u64)> = Vec::new();
        let counters = self.counters().to_vec();
        for (i, tally) in counters.iter().enumerate() {
            if *tally != NO_COUNTER {
                let counter_name = self.core.alphabet.get_counter_name(i as SymbolNumber);
                if counter_name.len() > max_name_len {
                    max_name_len = counter_name.len();
                }
                counter_name_val_pairs.push((counter_name, *tally));
            }
        }
        // std::sort with counter_comp (descending by .1)
        counter_name_val_pairs.sort_by(|a, b| {
            if counter_comp(a.clone(), b.clone()) {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        });
        for it in counter_name_val_pairs.iter() {
            retval.push_str("    ");
            retval.push_str(&it.0);
            let mut spacing_counter = max_name_len + 8 - it.0.len();
            while spacing_counter != 0 {
                retval.push(' ');
                spacing_counter -= 1;
            }
            retval.push_str(&it.1.to_string());
            retval.push('\n');
        }
        retval
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.get-pattern-count-info-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.get-pattern-count-info-fn]
    pub fn get_pattern_count_info(&mut self) -> String {
        let mut total: usize = 0;
        let mut retval = String::from("Pattern\t\t# of matches\n------------------------\n");
        for (first, second) in self.pattern_counts.iter() {
            retval.push_str(first);
            retval.push_str("\t\t");
            retval.push_str(&second.to_string());
            retval.push('\n');
            total += *second;
        }
        retval.push_str("------------------------\n");
        retval.push_str("Total:\t\t");
        retval.push_str(&total.to_string());
        retval.push('\n');
        retval
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.has-queued-input-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.has-queued-input-fn]
    pub fn has_queued_input(&self, input_pos: u32) -> bool {
        // we catch underflow due to left context checking here
        (input_pos as usize) < self.input.len() && (input_pos.wrapping_add(1) != 0)
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.input-matches-at-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.input-matches-at-fn]
    // begin/end are indices into self.input (matching get_longest_matching_capture).
    pub fn input_matches_at(&self, pos: u32, begin: usize, end: usize) -> bool {
        // if (pos + (end - begin) >= input.size()) return false;
        if pos as usize + (end - begin) >= self.input.len() {
            return false;
        }
        let mut i: usize = 0;
        while begin + i != end {
            if self.input[pos as usize + i] != self.input[begin + i] {
                return false;
            }
            i += 1;
        }
        true
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.copy-to-result-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.copy-to-result-fn]
    pub fn copy_to_result(&mut self, best_result: &DoubleTape) {
        for it in best_result.inner.iter() {
            self.result.inner.push(*it);
        }
    }
    pub fn copy_to_result_syms(&mut self, input: SymbolNumber, output: SymbolNumber) {
        self.result
            .inner
            .push(crate::transducer::SymbolPair::new_values(input, output));
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.parse-hfst3-header-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.parse-hfst3-header-fn]
    pub fn parse_hfst3_header(
        f: &mut crate::transducer::IStream<'_>,
    ) -> crate::error::Result<BTreeMap<String, String>> {
        let mut properties: BTreeMap<String, String> = BTreeMap::new();
        let header1 = b"HFST";
        let total = header1.len() + 1; // 'HFST' plus the C-string NUL = 5
        // how much of the header has been found
        let mut matched: Vec<u8> = Vec::new();
        let mut mismatch: i32 = -2; // sentinel for 'no mismatch char read'
        let mut header_loc = 0usize;
        while header_loc < total {
            let c = f.get();
            let expected: i32 = if header_loc < header1.len() {
                header1[header_loc] as i32
            } else {
                0 // header1[4] is the terminating '\0'
            };
            if c != expected {
                mismatch = c;
                break;
            }
            matched.push(c as u8);
            header_loc += 1;
        }
        if header_loc == total {
            let mut len_bytes = [0u8; 2];
            f.read(&mut len_bytes);
            let remaining_header_len = u16::from_ne_bytes(len_bytes) as usize;
            if f.get() != 0 {
                crate::bail!(TransducerHeader);
            }
            let mut headervalue = vec![0u8; remaining_header_len];
            f.read(&mut headervalue);
            if remaining_header_len == 0 || headervalue[remaining_header_len - 1] != 0 {
                crate::bail!(TransducerHeader);
            }
            let cstrlen = |s: &[u8]| -> usize { s.iter().position(|&b| b == 0).unwrap_or(s.len()) };
            let mut i = 0usize;
            while i < remaining_header_len {
                let length = cstrlen(&headervalue[i..]);
                let property = String::from_utf8_lossy(&headervalue[i..i + length]).into_owned();
                i += length + 1;
                let length = cstrlen(&headervalue[i..]);
                let value = String::from_utf8_lossy(&headervalue[i..i + length]).into_owned();
                properties.insert(property, value);
                i += length + 1;
            }
            Ok(properties)
        } else {
            // nope. put back what we've taken: the non-matching character first,
            // then the characters that did match, so the next read sees them in
            // their original order.
            if mismatch >= 0 {
                f.putback(mismatch as u8);
            }
            for &b in matched.iter().rev() {
                f.putback(b);
            }
            crate::bail!(TransducerHeader);
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-verbose-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-verbose-fn]
    pub fn set_verbose(&mut self, b: bool) {
        self.verbose = b;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-locate-mode-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-locate-mode-fn]
    pub fn set_locate_mode(&mut self, b: bool) {
        self.props.locate_mode = b;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-extract-patterns-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-extract-patterns-fn]
    pub fn set_extract_patterns(&mut self, b: bool) {
        self.props.extract_patterns = b;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-single-codepoint-tokenization-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-single-codepoint-tokenization-fn]
    pub fn set_single_codepoint_tokenization(&mut self, b: bool) {
        self.single_codepoint_tokenization = b;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-count-patterns-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-count-patterns-fn]
    pub fn set_count_patterns(&mut self, b: bool) {
        self.props.count_patterns = b;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-delete-patterns-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-delete-patterns-fn]
    pub fn set_delete_patterns(&mut self, b: bool) {
        self.props.delete_patterns = b;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-mark-patterns-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-mark-patterns-fn]
    pub fn set_mark_patterns(&mut self, b: bool) {
        self.props.mark_patterns = b;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-max-recursion-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-max-recursion-fn]
    pub fn set_max_recursion(&mut self, max: usize) {
        self.props.max_recursion = max;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-max-context-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-max-context-fn]
    pub fn set_max_context(&mut self, max: usize) {
        self.props.max_context_length = max;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.is-in-locate-mode-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.is-in-locate-mode-fn]
    pub fn is_in_locate_mode(&self) -> bool {
        self.props.locate_mode
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-profile-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-profile-fn]
    pub fn set_profile(&mut self, b: bool) {
        self.profile_mode = b;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-weight-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-weight-fn]
    pub fn set_weight(&mut self, w: Weight) {
        self.running_weight = w;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.increment-weight-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.increment-weight-fn]
    pub fn increment_weight(&mut self, w: Weight) {
        self.running_weight += w;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.get-weight-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.get-weight-fn]
    pub fn get_weight(&self) -> Weight {
        self.running_weight
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.increase-stack-depth-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.increase-stack-depth-fn]
    pub fn increase_stack_depth(&mut self) {
        self.stack_depth += 1;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.decrease-stack-depth-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.decrease-stack-depth-fn]
    pub fn decrease_stack_depth(&mut self) -> crate::error::Result<()> {
        if self.stack_depth == 0 {
            crate::bail!(Hfst, "pmatch: negative stack depth");
        }
        self.stack_depth -= 1;
        Ok(())
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.push-rtn-call-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.push-rtn-call-fn]
    // C++ takes 'PmatchTransducer * caller'; we take the caller's owning symbol
    // plus a copy of the caller's frame (needed to resume it, see RtnStackFrame).
    pub fn push_rtn_call(
        &mut self,
        return_index: u32,
        caller: SymbolNumber,
        caller_frame: LocalVariables,
    ) {
        let new_top = RtnStackFrame {
            caller,
            caller_index: return_index,
            caller_frame,
        };
        if self.rtn_stacks.len() <= self.stack_depth as usize {
            self.rtn_stacks.push(vec![new_top]);
        } else {
            self.rtn_stacks[self.stack_depth as usize].push(new_top);
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.rtn-stack-top-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.rtn-stack-top-fn]
    pub fn rtn_stack_top(&self) -> RtnStackFrame {
        self.rtn_stacks[self.stack_depth as usize]
            .last()
            .expect("rtn stack at this depth is non-empty")
            .clone()
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.get-latest-rtn-caller-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.get-latest-rtn-caller-fn]
    // Returns the caller's owning symbol (see push_rtn_call).
    pub fn get_latest_rtn_caller(&self) -> SymbolNumber {
        self.rtn_stacks[(self.stack_depth - 1) as usize]
            .last()
            .expect("rtn stack at this depth is non-empty")
            .caller
    }

    // The caller's frame stored alongside get_latest_rtn_caller's symbol; used to
    // resume the suspended caller when an RTN returns to it. [hfst/hfst#354]
    pub fn get_latest_caller_frame(&self) -> LocalVariables {
        self.rtn_stacks[(self.stack_depth - 1) as usize]
            .last()
            .expect("rtn stack at this depth is non-empty")
            .caller_frame
            .clone()
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.rtn-stack-pop-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.rtn-stack-pop-fn]
    pub fn rtn_stack_pop(&mut self) {
        self.rtn_stacks[self.stack_depth as usize].pop();
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.get-stack-depth-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.get-stack-depth-fn]
    pub fn get_stack_depth(&self) -> u32 {
        self.stack_depth
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.candidate-found-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.candidate-found-fn]
    pub fn candidate_found(&self) -> bool {
        if self.props.locate_mode {
            !self.tape_locations.is_empty()
        } else {
            !self.best_result.inner.is_empty()
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.try-recurse-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.try-recurse-fn]
    pub fn try_recurse(&mut self) -> bool {
        if self.recursion_depth_left > 0 {
            self.recursion_depth_left -= 1;
            true
        } else {
            false
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.unrecurse-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.unrecurse-fn]
    pub fn unrecurse(&mut self) {
        self.recursion_depth_left += 1;
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.reset-recursion-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.reset-recursion-fn]
    pub fn reset_recursion(&mut self) {
        self.recursion_depth_left = self.props.max_recursion as u32;
    }

    pub fn has_multichar_input_symbols(&self) -> bool {
        self.core.has_multichar_input_symbols()
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.uncompose-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.uncompose-fn]
    pub fn uncompose(&mut self, loc: &mut Location) {
        let verbose = self.verbose;
        if !self.props.uncomposable {
            if verbose {
                debug!("uncompose disabled");
            }
            return;
        }
        if verbose {
            debug!("uncomposing left {}", loc.input);
        }
        let middle_left = self
            .core
            .uncompose_left
            .as_ref()
            .expect("uncompose_left set when uncomposable")
            .lookup_fd_str(&loc.input, -1, 0.0);
        if middle_left.is_empty() {
            if verbose {
                debug!("empty midleft compose");
            }
            // ambig problems
            return;
        }
        let mut midforms: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for lpath in &middle_left {
            let mut mids = String::new();
            for symbol in &lpath.second {
                if !crate::hfst_flag_diacritics::FdOperation::is_diacritic(symbol) {
                    mids.push_str(symbol);
                }
            }
            if verbose {
                debug!("midleft composed {}", mids);
            }
            let middle_right = self
                .core
                .uncompose_right
                .as_ref()
                .expect("uncompose_right set when uncomposable")
                .lookup_fd_str(&mids, -1, 0.0);
            if middle_right.is_empty() {
                if verbose {
                    debug!("empty midright compose");
                }
                continue;
            }
            for rpath in &middle_right {
                let mut lows = String::new();
                for rsym in &rpath.second {
                    if !crate::hfst_flag_diacritics::FdOperation::is_diacritic(rsym) {
                        lows.push_str(rsym);
                    }
                }
                if verbose {
                    debug!("midright composed {}", lows);
                }
                if lows == loc.output {
                    if verbose {
                        debug!("matched {}", loc.output);
                    }
                    midforms.insert(mids.clone());
                } else if verbose {
                    debug!("no match {}", loc.output);
                }
            }
        }
        if midforms.len() > 1 {
            // ambig problems
        }
        for form in &midforms {
            loc.middle = form.clone();
        }
    }
}

/// One invocation of one net: the net borrowed out of the shared core, plus the
/// local-variable stack that invocation owns.
///
/// The C++ put the stack on the `PmatchTransducer` itself, so a net could only
/// be run once at a time and a re-entrant RTN return had to clone the net to get
/// a second stack (hfst/hfst#354). A walk borrows instead, so re-entering a net
/// costs a `&`.
// [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer]
// [spec:hfst:req:lookup-run-state.pmatch-shared-core]
pub(crate) struct PmatchWalk<'a> {
    core: &'a PmatchCore,
    net: &'a PmatchTransducer,
    /// The owning symbol of `net` — the C++ 'this' pointer in our ownership
    /// scheme. The toplevel (name "TOP") has none, so NO_SYMBOL_NUMBER.
    symbol: SymbolNumber,
    local_stack: LocalVariablesStack,
}

#[allow(clippy::too_many_arguments)]
impl<'a> PmatchWalk<'a> {
    /// Enter `net` with a pristine frame — what the C++ got by cloning the net
    /// out of its slot, whose stack was still the one its constructor built.
    fn entering(core: &'a PmatchCore, net: &'a PmatchTransducer) -> PmatchWalk<'a> {
        PmatchWalk::resuming(
            core,
            net,
            LocalVariables {
                flag_state: core.flag_state_proto.clone(),
                tape_step: 1,
                max_context_length_remaining: 254,
                context: ContextChecking::none,
                context_placeholder: 0,
                default_symbol_trap: false,
                negative_context_success: false,
                pending_passthrough: false,
            },
        )
    }

    /// Resume `net` from the frame it held when it made an RTN call.
    fn resuming(
        core: &'a PmatchCore,
        net: &'a PmatchTransducer,
        frame: LocalVariables,
    ) -> PmatchWalk<'a> {
        let symbol = if net.name == "TOP" {
            NO_SYMBOL_NUMBER
        } else {
            match core.alphabet.rtn_names.get(&net.name) {
                Some(s) => *s,
                None => NO_SYMBOL_NUMBER,
            }
        };
        PmatchWalk {
            core,
            net,
            symbol,
            local_stack: vec![frame],
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.get-rtn-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.get-rtn-fn]
    fn net_for(core: &'a PmatchCore, sym: SymbolNumber) -> &'a PmatchTransducer {
        if sym == NO_SYMBOL_NUMBER {
            core.toplevel
                .as_deref()
                .expect("toplevel present for RTN call")
        } else {
            core.alphabet.rtns[sym as usize]
                .as_deref()
                .expect("RTN slot occupied for RTN call")
        }
    }

    #[inline]
    fn frame(&self) -> &LocalVariables {
        self.local_stack
            .last()
            .expect("local_stack is non-empty during a match walk")
    }

    #[inline]
    fn frame_mut(&mut self) -> &mut LocalVariables {
        self.local_stack
            .last_mut()
            .expect("local_stack is non-empty during a match walk")
    }

    // [PORT NOTE / DIVERGENCE hfst/hfst#399, #354]
    // Try to enter a plain-epsilon configuration on the CURRENT DFS PATH. Returns
    // Some(key) if this exact (transducer, target, input_pos, context, tape_step)
    // config is not already an ANCESTOR on the path being explored — the caller
    // must remove the key (via 'epsilon_leave') once it returns from recursing
    // into 'target'. Returns None when re-entering an ancestor: that is a genuine
    // epsilon cycle (e.g. the '0:?*' loop of hfst#399) and must be pruned to
    // terminate.
    //
    // The stack is PATH-scoped (push on descent, pop on backtrack), not a global
    // per-attempt memo. A global memo also prunes CONVERGENT paths —
    // two distinct plain-epsilon branches that meet at the same state (e.g. the
    // 'cat+N' and 'cat+V' analyses of an ambiguous tokeniser converge on a shared
    // final state) — silently dropping every analysis but the first (hfst#354's
    // "missing wordforms"). Path-scoping keeps only true cycles pruned.
    //
    // Only PLAIN epsilon-input arcs (never flag diacritics, never Ins/RTN arcs,
    // which carry their own FdState/tape state) reach this helper, so it cannot
    // swallow legitimate flag or RTN traversal.
    fn epsilon_enter(
        &self,
        input_pos: u32,
        target: TransitionTableIndex,
        run: &mut PmatchContainer,
    ) -> Option<EpsilonVisitKey> {
        let frame = self.frame();
        let key: EpsilonVisitKey = (
            self.symbol,
            target,
            input_pos,
            frame.context as u8,
            frame.tape_step,
        );
        if run
            .epsilon_path
            .iter()
            .rev()
            .any(|ancestor| *ancestor == key)
        {
            return None;
        }
        run.epsilon_path.push(key);
        Some(key)
    }

    // Leave a plain-epsilon configuration entered via 'epsilon_enter', so a
    // sibling branch that later reaches the same state is not mistaken for a
    // cycle. [hfst#399, #354]
    fn epsilon_leave(run: &mut PmatchContainer, key: EpsilonVisitKey) {
        let left = run.epsilon_path.pop();
        debug_assert_eq!(left, Some(key), "epsilon configurations leave in DFS order");
    }

    // ---- the mutually recursive lookup-handling functions ----

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.take-epsilons-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.take-epsilons-fn]
    fn take_epsilons(
        &mut self,
        input_pos: u32,
        tape_pos: u32,
        i: TransitionTableIndex,
        run: &mut PmatchContainer,
    ) {
        let mut i = self.net.make_transition_table_index(i, 0);
        while PmatchTransducer::is_good(i) {
            let input = self.net.transition_input(i);
            if input != 0
                && !self.core.alphabet.is_flag_diacritic(input)
                && !self.core.alphabet.has_rtn_sym(input)
            {
                return;
            }

            let output = self.net.transition_output(i);
            let target = self.net.transition_target(i);
            let old_weight = run.get_weight();
            run.increment_weight(self.net.transition_weight(i));

            if self.checking_context() {
                if self.try_exiting_context(output) {
                    // We've successfully completed a context check
                    let cp = self.frame().context_placeholder;
                    self.get_analyses(cp, tape_pos, target, run);
                    self.local_stack.pop();
                } else if self.frame().negative_context_success {
                    // We've succeeded in a negative context, just back out
                    return;
                } else if self.core.alphabet.is_flag_diacritic(input) {
                    self.take_flag(input, input_pos, tape_pos, i, run);
                } else if self.core.alphabet.has_rtn_sym(input) {
                    let caller = self.symbol;
                    let locals = self.frame().clone();
                    PmatchWalk::entering(self.core, PmatchWalk::net_for(self.core, input))
                        .rtn_call_in_context(input_pos, tape_pos, caller, target, locals, run);
                } else {
                    // Don't alter tapes when checking context
                    // [DIVERGENCE hfst/hfst#399, #354] prune re-entered epsilon
                    // cycles (path-scoped), but keep convergent analyses.
                    if let Some(k) = self.epsilon_enter(input_pos, target, run) {
                        self.get_analyses(input_pos, tape_pos, target, run);
                        Self::epsilon_leave(run, k);
                    }
                }
            } else if input == 0 {
                if run.profile_mode {
                    run.count(output);
                }
                if !self.try_entering_context(output, run) {
                    // no context to enter, regular input epsilon
                    run.tape.write_pair(tape_pos, 0, output);

                    // [DIVERGENCE hfst/hfst#399] An entry/exit/capture arc
                    // mutates entry_stack/captures — state the memo key does
                    // not capture — so it is NOT a "plain" epsilon and is never
                    // pruned; only truly plain epsilon-input arcs are memoized.
                    let plain_epsilon = output
                        != self.core.alphabet.get_special(SpecialSymbol::entry)
                        && output != self.core.alphabet.get_special(SpecialSymbol::exit)
                        && !run.is_capture_tag_sym(output)
                        && !run.is_captured_tag_sym(output);

                    let mut orig_entry_stack_back: u32 = 0;
                    // if it's an entry or exit arc, adjust entry stack
                    if output == self.core.alphabet.get_special(SpecialSymbol::entry) {
                        run.entry_stack.push(input_pos);
                    } else if output == self.core.alphabet.get_special(SpecialSymbol::exit) {
                        orig_entry_stack_back = *run
                            .entry_stack
                            .last()
                            .expect("exit arc has a matching entry on the stack");
                        run.entry_stack.pop();
                    } else if run.is_capture_tag_sym(output) {
                        // if it's a capture tag, remember where we were
                        let capture = Capture {
                            begin: *run
                                .entry_stack
                                .last()
                                .expect("capture tag has a matching entry on the stack"),
                            end: input_pos,
                            name: output,
                        };
                        run.captures.push(capture);
                    } else if run.is_captured_tag_sym(output) {
                        // if it's a captured tag, try each previously
                        // captured sequence
                        let key = run.captured2capture(output);
                        let cap = run.get_longest_matching_capture(key, input_pos);

                        if cap.1 - cap.0 != 0 {
                            let slice: Vec<SymbolNumber> = run.input[cap.0..cap.1].to_vec();
                            run.tape.write_slice(tape_pos, &slice);
                            let span = (cap.1 - cap.0) as u32;
                            self.get_analyses(input_pos + span, tape_pos + span, target, run);
                        }
                        i += 1;
                        run.set_weight(old_weight);
                        continue;
                    }

                    if !plain_epsilon {
                        self.get_analyses(input_pos, tape_pos + 1, target, run);
                    } else if let Some(k) = self.epsilon_enter(input_pos, target, run) {
                        self.get_analyses(input_pos, tape_pos + 1, target, run);
                        Self::epsilon_leave(run, k);
                    }

                    if output == self.core.alphabet.get_special(SpecialSymbol::entry) {
                        run.entry_stack.pop();
                    } else if output == self.core.alphabet.get_special(SpecialSymbol::exit) {
                        run.entry_stack.push(orig_entry_stack_back);
                    } else if run.is_capture_tag_sym(output) {
                        run.captures.pop();
                    }
                } else {
                    self.check_context(input_pos, tape_pos, i, run);
                }
            } else if self.core.alphabet.is_flag_diacritic(input) {
                self.take_flag(input, input_pos, tape_pos, i, run);
            } else if self.core.alphabet.has_rtn_sym(input) {
                let caller = self.symbol;
                let caller_frame = self.frame().clone();
                PmatchWalk::entering(self.core, PmatchWalk::net_for(self.core, input)).rtn_call(
                    input_pos,
                    tape_pos,
                    caller,
                    target,
                    caller_frame,
                    run,
                );
            }
            i += 1;
            run.set_weight(old_weight);
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.check-context-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.check-context-fn]
    fn check_context(
        &mut self,
        input_pos: u32,
        tape_pos: u32,
        i: TransitionTableIndex,
        run: &mut PmatchContainer,
    ) {
        // The context placeholder remembers the position in the input before
        // a context check. If the context check is successful, the placeholder
        // will be used as the input position going forwards.
        self.frame_mut().context_placeholder = input_pos;
        let mut input_pos = input_pos;
        let ctx = self.frame().context;
        if ctx == ContextChecking::LC || ctx == ContextChecking::NLC {
            // Jump to the left-hand side of the input
            input_pos = run
                .entry_stack
                .last()
                .expect("entry_stack populated during a left-context check")
                .wrapping_sub(1);
        }
        let target = self.net.transition_target(i);
        self.get_analyses(input_pos, tape_pos, target, run);

        // In case we have a negative context, we check to see if the context
        // matched. If it didn't, we schedule a passthrough arc after we've
        // processed epsilons.
        let mut schedule_passthrough = false;
        let ctx = self.frame().context;
        if (ctx == ContextChecking::NLC || ctx == ContextChecking::NRC)
            && !self.frame().negative_context_success
        {
            schedule_passthrough = true;
        }
        // Pop the local stack that got pushed by entering the context
        self.local_stack.pop();
        if schedule_passthrough {
            self.frame_mut().pending_passthrough = true;
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.take-flag-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.take-flag-fn]
    fn take_flag(
        &mut self,
        input: SymbolNumber,
        input_pos: u32,
        tape_pos: u32,
        i: TransitionTableIndex,
        run: &mut PmatchContainer,
    ) {
        let mut old_global_values: Vec<i16> = Vec::new();
        if self.core.alphabet.is_global_flag_sym(input) {
            old_global_values = run.global_flag_state.get_values().clone();
            let op = self
                .core
                .alphabet
                .get_operation(input)
                .expect("flag diacritic has an operation")
                .clone();
            if !run.global_flag_state.apply_operation(&op) {
                return;
            }
        }
        let old_values = self.frame().flag_state.get_values().clone();
        let op = self
            .core
            .alphabet
            .get_operation(input)
            .expect("flag diacritic has an operation")
            .clone();
        if self.frame_mut().flag_state.apply_operation(&op) {
            // flag diacritic allowed
            // generally we shouldn't care to write flags
            //                container->tape.write(tape_pos, input, output);
            let target = self.net.transition_target(i);
            self.get_analyses(input_pos, tape_pos, target, run);
        }
        if self.core.alphabet.is_global_flag_sym(input) {
            run.global_flag_state.assign_values(&old_global_values);
        }
        self.frame_mut().flag_state.assign_values(&old_values);
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.take-transitions-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.take-transitions-fn]
    fn take_transitions(
        &mut self,
        input: SymbolNumber,
        input_pos: u32,
        tape_pos: u32,
        i: TransitionTableIndex,
        run: &mut PmatchContainer,
    ) {
        let mut i = self.net.make_transition_table_index(i, input);

        while PmatchTransducer::is_good(i) {
            let mut this_input = self.net.transition_input(i);
            let mut this_output = self.net.transition_output(i);
            let target = self.net.transition_target(i);
            if this_input == NO_SYMBOL_NUMBER {
                return;
            } else if this_input == input {
                let old_weight = run.get_weight();
                run.increment_weight(self.net.transition_weight(i));
                if !self.checking_context() {
                    if self.core.alphabet.is_meta_arc(this_output)
                        || (run.list2symbols(this_output) != NO_SYMBOL_NUMBER)
                    {
                        // we got here via a meta-arc, so look back in the
                        // input tape to find the symbol we want to write
                        this_output = run.input[input_pos as usize];
                        this_input = run.input[input_pos as usize];
                    }
                    if this_input
                        == self
                            .core
                            .alphabet
                            .get_special(SpecialSymbol::Pmatch_passthrough)
                    {
                        self.get_analyses(input_pos, tape_pos, target, run); // awkward
                    } else {
                        run.tape.write_pair(tape_pos, this_input, this_output);
                        self.get_analyses(input_pos + 1, tape_pos + 1, target, run);
                    }
                } else {
                    // Checking context so don't touch output
                    if self.frame().max_context_length_remaining > 0 {
                        if (self.frame().tape_step < 0) && (input_pos == 0) {
                            // (C++ marks FIXME here) prevents segfault but
                            self.get_analyses(input_pos, tape_pos, target, run); // awkward
                        } else {
                            self.frame_mut().max_context_length_remaining -= 1;
                            let step = self.frame().tape_step;
                            let new_input_pos = (input_pos as i64 + step as i64) as u32;
                            self.get_analyses(new_input_pos, tape_pos, target, run);
                            self.frame_mut().max_context_length_remaining += 1;
                        }
                    }
                }
                self.frame_mut().default_symbol_trap = false;
                run.set_weight(old_weight);
            } else {
                return;
            }
            i += 1;
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.get-analyses-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.get-analyses-fn]
    fn get_analyses(
        &mut self,
        input_pos: u32,
        tape_pos: u32,
        index: TransitionTableIndex,
        run: &mut PmatchContainer,
    ) {
        let i = index;
        if run.get_weight() > run.max_weight {
            return;
        }
        if run.max_time > 0.0 {
            run.call_counter += 1;
            // Have we spent too much time?
            if run.limit_reached
                || (run.call_counter.is_multiple_of(1000000)
                    && (run.candidate_found()
                        // if we have at least something, stop doing more work
                        && run
                            .start_clock
                            .expect("start_clock set when max_time is enabled")
                            .elapsed()
                            .as_secs_f64()
                            > run.max_time))
            {
                run.limit_reached = true;
                return;
            }
        }
        if !run.try_recurse() {
            if run.verbose {
                warn!("out of stack space, truncating result");
            }
            return;
        }
        self.frame_mut().default_symbol_trap = true;
        self.take_epsilons(input_pos, tape_pos, i + 1, run);
        if self.frame().pending_passthrough {
            self.frame_mut().pending_passthrough = false;
            // A negative context failed (successfully)
            let passthrough = self
                .core
                .alphabet
                .get_special(SpecialSymbol::Pmatch_passthrough);
            self.take_transitions(passthrough, input_pos, tape_pos, i + 1, run);
        }
        // Check for finality even if the input string hasn't ended
        if self.net.is_final(i) {
            let old_weight = run.get_weight();
            run.increment_weight(self.net.get_weight(i));
            self.handle_final_state(input_pos, tape_pos, run);
            run.set_weight(old_weight);
        }

        if !run.has_queued_input(input_pos) {
            run.unrecurse();
            return;
        }
        let input = run.input[input_pos as usize];

        let list_idx = run.symbol2lists(input);
        if list_idx != NO_SYMBOL_NUMBER {
            // At least one symbol list could allow this symbol
            let list = run.symbol_list(list_idx).clone();
            for it in list.iter() {
                self.take_transitions(*it, input_pos, tape_pos, i + 1, run);
            }
        }
        if self.core.alphabet.get_special(SpecialSymbol::UnicodeAlpha) != NO_SYMBOL_NUMBER
            && run.is_unicode_alpha(input)
        {
            let s = self.core.alphabet.get_special(SpecialSymbol::UnicodeAlpha);
            self.take_transitions(s, input_pos, tape_pos, i + 1, run);
        }
        if self
            .core
            .alphabet
            .get_special(SpecialSymbol::UnicodeUpperAlpha)
            != NO_SYMBOL_NUMBER
            && run.is_unicode_upperalpha(input)
        {
            let s = self
                .core
                .alphabet
                .get_special(SpecialSymbol::UnicodeUpperAlpha);
            self.take_transitions(s, input_pos, tape_pos, i + 1, run);
        }
        if self
            .core
            .alphabet
            .get_special(SpecialSymbol::UnicodeLowerAlpha)
            != NO_SYMBOL_NUMBER
            && run.is_unicode_loweralpha(input)
        {
            let s = self
                .core
                .alphabet
                .get_special(SpecialSymbol::UnicodeLowerAlpha);
            self.take_transitions(s, input_pos, tape_pos, i + 1, run);
        }
        if self
            .core
            .alphabet
            .get_special(SpecialSymbol::UnicodeWhitespace)
            != NO_SYMBOL_NUMBER
            && run.is_unicode_whitespace(input)
        {
            let s = self
                .core
                .alphabet
                .get_special(SpecialSymbol::UnicodeWhitespace);
            self.take_transitions(s, input_pos, tape_pos, i + 1, run);
        }

        // The "normal" case where we have a regular input symbol
        if input < self.net.orig_symbol_count {
            self.take_transitions(input, input_pos, tape_pos, i + 1, run);
        } else {
            if self.core.alphabet.get_identity_symbol() != NO_SYMBOL_NUMBER {
                let s = self.core.alphabet.get_identity_symbol();
                self.take_transitions(s, input_pos, tape_pos, i + 1, run);
            }
            if self.core.alphabet.get_unknown_symbol() != NO_SYMBOL_NUMBER {
                let s = self.core.alphabet.get_unknown_symbol();
                self.take_transitions(s, input_pos, tape_pos, i + 1, run);
            }
        }
        if self.core.alphabet.get_default_symbol() != NO_SYMBOL_NUMBER
            && self.frame().default_symbol_trap
        {
            let s = self.core.alphabet.get_default_symbol();
            self.take_transitions(s, input_pos, tape_pos, i + 1, run);
        }
        run.unrecurse();
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.checking-context-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.checking-context-fn]
    fn checking_context(&self) -> bool {
        self.frame().context != ContextChecking::none
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.try-entering-context-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.try-entering-context-fn]
    fn try_entering_context(&mut self, symbol: SymbolNumber, run: &PmatchContainer) -> bool {
        let mut new_top: LocalVariables;
        if symbol == self.core.alphabet.get_special(SpecialSymbol::LC_entry) {
            new_top = self.frame().clone();
            new_top.context = ContextChecking::LC;
            new_top.tape_step = -1;
        } else if symbol == self.core.alphabet.get_special(SpecialSymbol::RC_entry) {
            new_top = self.frame().clone();
            new_top.context = ContextChecking::RC;
            new_top.tape_step = 1;
        } else if symbol == self.core.alphabet.get_special(SpecialSymbol::NLC_entry) {
            new_top = self.frame().clone();
            new_top.context = ContextChecking::NLC;
            new_top.tape_step = -1;
        } else if symbol == self.core.alphabet.get_special(SpecialSymbol::NRC_entry) {
            new_top = self.frame().clone();
            new_top.context = ContextChecking::NRC;
            new_top.tape_step = 1;
        } else {
            return false;
        }
        new_top.max_context_length_remaining = run.props.max_context_length;
        self.local_stack.push(new_top);
        true
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.try-exiting-context-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.try-exiting-context-fn]
    fn try_exiting_context(&mut self, symbol: SymbolNumber) -> bool {
        match self.frame().context {
            ContextChecking::LC
                if symbol == self.core.alphabet.get_special(SpecialSymbol::LC_exit) =>
            {
                self.exit_context();
                true
            }
            ContextChecking::RC
                if symbol == self.core.alphabet.get_special(SpecialSymbol::RC_exit) =>
            {
                self.exit_context();
                true
            }
            // NOTE: faithful to C++: the NRC case has no 'else'/'break', so on a
            // non-matching symbol it falls through to the NLC case (and then to
            // default). We reproduce that fallthrough explicitly.
            ContextChecking::NRC => {
                if symbol == self.core.alphabet.get_special(SpecialSymbol::NRC_exit) {
                    self.frame_mut().negative_context_success = true;
                    return false;
                }
                if symbol == self.core.alphabet.get_special(SpecialSymbol::NLC_exit) {
                    self.frame_mut().negative_context_success = true;
                    return false;
                }
                false
            }
            ContextChecking::NLC => {
                if symbol == self.core.alphabet.get_special(SpecialSymbol::NLC_exit) {
                    self.frame_mut().negative_context_success = true;
                    return false;
                }
                false
            }
            // `none`, plus `LC`/`RC` whose exit-symbol guards above did not fire.
            ContextChecking::none | ContextChecking::LC | ContextChecking::RC => false,
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.exit-context-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.exit-context-fn]
    fn exit_context(&mut self) {
        let mut new_top = self.frame().clone();
        new_top.context = ContextChecking::none;
        new_top.negative_context_success = false;
        new_top.tape_step = 1;
        self.local_stack.push(new_top);
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.match-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.match-fn]
    fn do_match(&mut self, input_pos: u32, tape_pos: u32, run: &mut PmatchContainer) {
        {
            let top = self.frame_mut();
            top.context = ContextChecking::none;
            top.tape_step = 1;
            top.context_placeholder = 0;
            top.default_symbol_trap = false;
        }
        // [DIVERGENCE hfst/hfst#399] Fresh epsilon-cycle memo per top-level
        // match attempt (see PmatchContainer::epsilon_path).
        run.epsilon_path.clear();
        self.get_analyses(input_pos, tape_pos, 0, run);
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.rtn-call-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.rtn-call-fn]
    fn rtn_call(
        &mut self,
        input_pos: u32,
        tape_pos: u32,
        caller: SymbolNumber,
        caller_index: TransitionTableIndex,
        caller_frame: LocalVariables,
        run: &mut PmatchContainer,
    ) {
        run.push_rtn_call(caller_index, caller, caller_frame);
        run.increase_stack_depth();
        let mut new_top = self.frame().clone();
        new_top.flag_state = self.core.flag_state_proto.clone();
        new_top.tape_step = 1;
        new_top.context = ContextChecking::none;
        new_top.context_placeholder = 0;
        new_top.default_symbol_trap = false;
        self.local_stack.push(new_top);
        self.get_analyses(input_pos, tape_pos, 0, run);
        self.local_stack.pop();
        run.decrease_stack_depth()
            .expect("pmatch stack-depth invariant: decrease is balanced with a prior increase");
        run.rtn_stack_pop();
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.rtn-call-in-context-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.rtn-call-in-context-fn]
    fn rtn_call_in_context(
        &mut self,
        input_pos: u32,
        tape_pos: u32,
        caller: SymbolNumber,
        caller_index: TransitionTableIndex,
        locals: LocalVariables,
        run: &mut PmatchContainer,
    ) {
        // 'locals' is the caller's frame at the call site; stash a copy for the
        // eventual return before it is repurposed as the callee's own frame.
        run.push_rtn_call(caller_index, caller, locals.clone());
        run.increase_stack_depth();
        let mut new_top = locals;
        new_top.flag_state = self.core.flag_state_proto.clone();
        self.local_stack.push(new_top);
        self.get_analyses(input_pos, tape_pos, 0, run);
        self.local_stack.pop();
        run.decrease_stack_depth()
            .expect("pmatch stack-depth invariant: decrease is balanced with a prior increase");
        run.rtn_stack_pop();
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.rtn-return-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.rtn-return-fn]
    fn rtn_return(&mut self, input_pos: u32, tape_pos: u32, run: &mut PmatchContainer) {
        run.decrease_stack_depth()
            .expect("pmatch stack-depth invariant: decrease is balanced with a prior increase");
        let entry_index = run.rtn_stack_top().caller_index;
        self.get_analyses(input_pos, tape_pos, entry_index, run);
        run.increase_stack_depth();
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.handle-final-state-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.handle-final-state-fn]
    fn handle_final_state(&mut self, input_pos: u32, tape_pos: u32, run: &mut PmatchContainer) {
        if run.get_stack_depth() > 0 {
            // We're not the toplevel, return to caller. The caller is suspended
            // higher on the Rust call stack; with its local-variable stack held
            // by the walk rather than the net, resuming it is a second shared
            // borrow of the same net plus the frame it held at the RTN call.
            // [hfst/hfst#354]
            let core = self.core;
            let rtn_target = run.get_latest_rtn_caller();
            let caller_frame = run.get_latest_caller_frame();
            PmatchWalk::resuming(core, PmatchWalk::net_for(core, rtn_target), caller_frame)
                .rtn_return(input_pos, tape_pos, run);
        } else if run.is_in_locate_mode() {
            run.grab_location(input_pos, tape_pos);
        } else {
            run.note_analysis(input_pos, tape_pos);
        }
    }
}
