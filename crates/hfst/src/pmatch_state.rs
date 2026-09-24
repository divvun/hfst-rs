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

mod matching;
mod output;
mod setup;
mod walk;
mod walk_bookkeeping;

use walk::PmatchWalk;

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
}
