//! Full port of 'libhfst/src/implementations/optimized-lookup/pmatch.{h,cc}'
//! (namespace 'hfst_ol').
//!
//! Ownership scheme (see crate notes): 'PmatchContainer' is the sole owner of
//! the 'PmatchAlphabet' and of every 'PmatchTransducer' (the toplevel plus all
//! RTNs, the latter living in 'alphabet.rtns'). 'PmatchTransducer' stores NO
//! back-reference to its container or alphabet; every engine method instead
//! receives '&mut PmatchContainer' as a parameter and reaches the alphabet
//! through it.

use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Instant;

use icu::segmenter::GraphemeClusterSegmenter;
use tracing::{debug, warn};

use crate::hfst_flag_diacritics::{FdState, FdTable};
use crate::transducer::{
    DoubleTape, Encoder, NO_COUNTER, NO_SYMBOL_NUMBER, SymbolNumber, SymbolNumberVector,
    TRANSITION_TARGET_TABLE_START, TransducerAlphabet, TransitionTableIndex, TransitionW,
    TransitionWIndex, Weight, WeightedDoubleTape,
};

// [spec:hfst:def:pmatch.hfst-ol.rtn-call-stack]
pub type RtnCallStack = Vec<RtnStackFrame>;
// [spec:hfst:def:pmatch.hfst-ol.rtn-call-stacks]
pub type RtnCallStacks = Vec<RtnCallStack>;
// [spec:hfst:def:pmatch.hfst-ol.rtn-vector]
// In C++ this is 'std::vector<PmatchTransducer *>'. Because the container owns
// the RTNs, we store owned boxes (Option = the NULL slot) here in the alphabet.
pub type RtnVector = Vec<Option<Box<PmatchTransducer>>>;
// [spec:hfst:def:pmatch.hfst-ol.rtn-name-map]
pub type RtnNameMap = BTreeMap<String, SymbolNumber>;
// [spec:hfst:def:pmatch.hfst-ol.location-vector]
pub type LocationVector = Vec<Location>;
// [spec:hfst:def:pmatch.hfst-ol.location-vector-vector]
pub type LocationVectorVector = Vec<LocationVector>;
// [spec:hfst:def:pmatch.hfst-ol.weighted-double-tape-vector]
pub type WeightedDoubleTapeVector = Vec<WeightedDoubleTape>;

// [spec:hfst:def:pmatch.hfst-ol.special-symbol]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpecialSymbol {
    entry,
    exit,
    LC_entry,
    LC_exit,
    RC_entry,
    RC_exit,
    NLC_entry,
    NLC_exit,
    NRC_entry,
    NRC_exit,
    Pmatch_passthrough,
    boundary,
    Pmatch_input_mark,
    UnicodeAlpha,
    UnicodeUpperAlpha,
    UnicodeLowerAlpha,
    UnicodeWhitespace,
    SPECIALSYMBOL_NR_ITEMS,
}

// [spec:hfst:def:pmatch.hfst-ol.n-byte-grapheme-fn]
// [spec:hfst:sem:pmatch.hfst-ol.n-byte-grapheme-fn]
// Returns the number of UTF-8 bytes of the first grapheme cluster (ICU's
// grapheme break iterator -> the 'icu' crate's GraphemeClusterSegmenter).
pub fn nByte_grapheme(u8_str: &str) -> i32 {
    let segmenter = GraphemeClusterSegmenter::new();
    let mut bounds = segmenter.segment_str(u8_str);
    let begin = bounds.next().unwrap_or(0);
    let end = bounds.next();
    match end {
        None => 0,
        Some(end) => {
            if begin == end {
                0
            } else {
                (end - begin) as i32 // strlen is number of bytes
            }
        }
    }
}

// Byte length of the first grapheme cluster at the front of a raw (possibly
// invalid) UTF-8 byte slice; 0 if there is no complete cluster.
//
// 'initialize_input' calls this once per input position while walking the whole
// input, so validating/segmenting the entire remaining tail on every call is
// O(n^2) (the source of the large-sentence tokenise hang, hfst/hfst#483). Since
// the first grapheme is bounded by its own length, we only look at a small
// prefix: we validate a geometrically growing window and accept the boundary as
// soon as the segmenter reports one strictly inside the validated bytes (so no
// longer cluster could extend past it). This keeps the whole walk O(n).
pub fn nByte_grapheme_bytes(bytes: &[u8]) -> i32 {
    if bytes.is_empty() {
        return 0;
    }
    let mut window = 8usize;
    loop {
        let cap = window.min(bytes.len());
        // Trim to the last complete UTF-8 codepoint so from_utf8 sees a valid
        // prefix even when the window splits a multibyte sequence.
        let valid = match std::str::from_utf8(&bytes[..cap]) {
            Ok(_) => cap,
            Err(e) => e.valid_up_to(),
        };
        let n = nByte_grapheme(std::str::from_utf8(&bytes[..valid]).unwrap_or(""));
        // A boundary strictly inside the validated prefix is final: no cluster
        // can reach past bytes we have already segmented. Otherwise the cluster
        // may continue beyond the window, so widen and retry.
        if (n > 0 && (n as usize) < valid) || cap == bytes.len() {
            return n;
        }
        window *= 2;
    }
}

// [spec:hfst:def:pmatch.hfst-ol.counter-comp-fn]
// [spec:hfst:sem:pmatch.hfst-ol.counter-comp-fn]
pub fn counter_comp(l: (String, u64), r: (String, u64)) -> bool {
    // Descending order
    l.1 > r.1
}

// [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet]
pub struct PmatchAlphabet {
    pub(crate) base: TransducerAlphabet,
    pub(crate) rtns: RtnVector,
    pub(crate) input_mark_symbol: SymbolNumber,
    pub(crate) special_symbols: SymbolNumberVector,
    pub(crate) end_tag_map: BTreeMap<SymbolNumber, String>,
    pub(crate) capture_tag_map: BTreeMap<String, SymbolNumber>,
    pub(crate) captured_tag_map: BTreeMap<String, SymbolNumber>,
    pub(crate) capture2captured: SymbolNumberVector,
    pub(crate) captured2capture: SymbolNumberVector,
    pub(crate) rtn_names: RtnNameMap,
    // For each symbol, either NO_SYMBOL for "no corresponding list" or an index into symbol_lists
    pub(crate) symbol2lists: SymbolNumberVector,
    // For each a symbol, either NO_SYMBOL for "this is not a list" or an index into symbol_list_members
    pub(crate) list2symbols: SymbolNumberVector,
    // For each entry referring to entries in the symbol table, indicate
    // "this symbol is an exclusionary list", ie. symbols not in it
    // will match
    pub(crate) exclusionary_lists: SymbolNumberVector,
    pub(crate) symbol_lists: Vec<SymbolNumberVector>,
    pub(crate) symbol_list_members: Vec<SymbolNumberVector>,
    pub(crate) counters: Vec<u64>,
    pub(crate) guards: SymbolNumberVector,
    pub(crate) global_flags: Vec<bool>,
    pub(crate) printable_vector: Vec<bool>,
}

// [spec:hfst:def:pmatch.hfst-ol.rtn-stack-frame]
#[derive(Clone)]
pub struct RtnStackFrame {
    // C++ stores a raw 'PmatchTransducer * caller'. Since the RTNs are owned by
    // the container's alphabet, this stores the owning symbol of the caller so
    // the engine can look the caller back up. See notes.
    pub caller: SymbolNumber,
    pub caller_index: TransitionTableIndex,
    // The caller's local frame at the moment it made this RTN call. When the RTN
    // returns, the caller is suspended higher on the Rust stack and cannot be
    // borrowed again, so it is rebuilt from its (shared) tables plus this frame.
    // [hfst/hfst#354]
    pub caller_frame: LocalVariables,
}

// [spec:hfst:def:pmatch.hfst-ol.capture]
#[derive(Clone, Copy)]
pub struct Capture {
    pub begin: u32,
    pub end: u32,
    pub name: SymbolNumber,
}

// [spec:hfst:def:pmatch.hfst-ol.location]
#[derive(Clone, Default)]
pub struct Location {
    pub start: u32,
    pub length: u32,
    pub input: String,
    pub middle: String, // composted middle tape
    pub output: String,
    pub tag: String,
    pub weight: Weight,
    pub input_parts: Vec<usize>,  // indices in input_symbol_strings
    pub output_parts: Vec<usize>, // indices in output_symbol_strings
    pub input_symbol_strings: Vec<crate::hfst_data_types::Symbol>,
    pub output_symbol_strings: Vec<crate::hfst_data_types::Symbol>,
}

// [spec:hfst:def:pmatch.hfst-ol.location.operator-fn]
// [spec:hfst:sem:pmatch.hfst-ol.location.operator-fn]
impl PartialEq for Location {
    fn eq(&self, other: &Self) -> bool {
        self.weight == other.weight
    }
}
impl Eq for Location {}
impl PartialOrd for Location {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Location {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.weight
            .partial_cmp(&other.weight)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

// [DIVERGENCE hfst/hfst#335] Drop exact-duplicate Locations from a (typically
// already sorted) location vector in place, keeping the first occurrence of
// each and otherwise preserving order. Two Locations are exact duplicates when
// they agree on span (start + length), output, tag and weight — the fields that
// make up a reported match; every remaining field is derived from these plus
// the non-printable tape symbols that the report never shows. 'Location's own
// 'Eq'/'Ord' compare weight only (a faithful port of the C++ 'operator<' used
// for weight-ranked sorting), so identity is spelled out here rather than
// reusing them.
pub(crate) fn dedup_locations(ls: &mut LocationVector) {
    let mut seen: std::collections::BTreeSet<(u32, u32, String, String, u32)> =
        std::collections::BTreeSet::new();
    ls.retain(|l| {
        let key = (
            l.start,
            l.length,
            l.output.clone(),
            l.tag.clone(),
            l.weight.to_bits(),
        );
        seen.insert(key)
    });
}

// [spec:hfst:def:pmatch.hfst-ol.context-matched-trap]
pub struct ContextMatchedTrap {
    pub polarity: bool,
}

impl ContextMatchedTrap {
    // [spec:hfst:def:pmatch.hfst-ol.context-matched-trap.context-matched-trap-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.context-matched-trap.context-matched-trap-fn]
    pub fn new(p: bool) -> ContextMatchedTrap {
        ContextMatchedTrap { polarity: p }
    }
}

// [spec:hfst:def:pmatch.hfst-ol.pmatch-container]
// weight_limit is currently read only on paths not yet exercised by a test.
#[allow(dead_code)]
pub struct PmatchContainer {
    pub(crate) alphabet: PmatchAlphabet,
    pub(crate) encoder: Option<Encoder>,
    pub(crate) orig_symbol_count: SymbolNumber,
    pub(crate) symbol_count: SymbolNumber,
    pub(crate) toplevel: Option<Box<PmatchTransducer>>,
    pub(crate) input: SymbolNumberVector,
    // This tracks the ENTRY and EXIT tags
    pub(crate) entry_stack: Vec<u32>,
    pub(crate) rtn_stacks: RtnCallStacks,
    // C++ raw 'hfst_ol::Transducer *'; the two uncomposer nets read by
    // 'uncompose' via 'lookup_fd'. Owned box, optional.
    pub(crate) uncompose_left: Option<Box<crate::transducer::Transducer>>,
    pub(crate) uncompose_right: Option<Box<crate::transducer::Transducer>>,
    pub(crate) tape: DoubleTape,
    pub(crate) best_result: DoubleTape,
    pub(crate) result: DoubleTape,
    pub(crate) locations: LocationVectorVector,
    pub(crate) tape_locations: WeightedDoubleTapeVector,
    pub(crate) captures: Vec<Capture>,
    pub(crate) best_captures: Vec<Capture>,
    pub(crate) old_captures: Vec<Capture>,
    pub(crate) possible_first_symbols: Vec<bool>,
    // The flag state for global flags
    pub(crate) global_flag_state: FdState<SymbolNumber>,
    pub(crate) verbose: bool,

    pub(crate) count_patterns: bool,
    pub(crate) delete_patterns: bool,
    pub(crate) extract_patterns: bool,
    pub(crate) locate_mode: bool,
    pub(crate) mark_patterns: bool,
    pub(crate) max_context_length: usize,
    pub(crate) max_recursion: usize,
    pub(crate) need_separators: bool,
    pub(crate) xerox_composition: bool,
    pub(crate) uncomposable: bool,

    pub(crate) line_number: u64,
    pub(crate) pattern_counts: BTreeMap<String, usize>,
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
    // Per-search memo of plain-epsilon-input configurations already fully
    // explored during the current top-level match attempt. Keyed on
    // (transducer symbol, target transition index, input_pos, context mode,
    // tape direction). Re-entering the same key is pruned. This TERMINATES
    // grammars whose compiled net contains an epsilon-input / unknown-output
    // arc under Kleene star (e.g. `0:?*`), which the C++ engine loops on
    // forever (still-open upstream bug hfst/hfst#399). Guarded strictly to
    // plain (non-flag, non-Ins/RTN) epsilon arcs so flag-diacritic traversal
    // and RTN calls keep their own state and are never pruned by this memo.
    pub(crate) epsilon_visited: std::collections::BTreeSet<EpsilonVisitKey>,
}

// [spec:hfst:def:pmatch.hfst-ol.pmatch-container.epsilon-visit-key]
// Identifies a plain-epsilon configuration for the #399 cycle guard.
pub(crate) type EpsilonVisitKey = (SymbolNumber, TransitionTableIndex, u32, u8, i8);

// [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer]
// The tables (transition_table / index_table) are static once the transducer is
// loaded; only 'local_stack' changes during a walk. They live behind 'Rc' so a
// transducer can be cloned cheaply for a (re-)entrant walk: an RTN returning to
// its caller must run the caller again while the live caller is suspended higher
// on the Rust call stack and cannot be borrowed a second time. Cloning shares
// the tables and gives the re-entrant walk its own 'local_stack'. [hfst/hfst#354]
#[derive(Clone)]
pub struct PmatchTransducer {
    pub(crate) name: String,
    pub(crate) local_stack: Vec<LocalVariables>,
    pub(crate) transition_table: Rc<Vec<TransitionW>>,
    pub(crate) index_table: Rc<Vec<TransitionWIndex>>,
    pub(crate) orig_symbol_count: SymbolNumber,
    // NOTE: no 'alphabet' and no 'container' back-references; the engine methods
    // receive '&mut PmatchContainer' (which owns the alphabet) as a parameter.
}

// [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.context-checking]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ContextChecking {
    none,
    LC,
    NLC,
    RC,
    NRC,
}

// Transducers have static data, ie. tables for describing the states and
// transitions, and dynamic data, which is altered during lookup.
// In pmatch several instances of the same transducer may be operating
// in a stack, so this dynamic data is put in a class of its own.
// [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.local-variables]
#[derive(Clone)]
pub struct LocalVariables {
    pub flag_state: FdState<SymbolNumber>,

    // Used for context checks
    pub tape_step: i8,
    pub max_context_length_remaining: usize,
    pub context_placeholder: u32,
    pub context: ContextChecking,
    pub default_symbol_trap: bool,
    pub negative_context_success: bool,
    pub pending_passthrough: bool,
}

// ==================== PmatchAlphabet (impl from workflow body agent) ====================
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
impl Default for PmatchAlphabet {
    fn default() -> Self {
        Self::new()
    }
}

impl PmatchAlphabet {
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.pmatch-alphabet-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.pmatch-alphabet-fn]
    // ctor from istream: PmatchAlphabet(std::istream&, SymbolNumber, PmatchContainer*)
    // Deferred: the C++ ctor reads via TransducerAlphabet(inputstream, symbol_count, true)
    // and touches hfst::FdOperation::get_feature/get_value plus fd_table mutation,
    // which is part of the istream-reading facade path.
    pub fn new_from_stream(
        inputstream: &mut crate::transducer::IStream<'_>,
        symbol_count: SymbolNumber,
        cont: &mut PmatchContainer,
    ) -> crate::error::Result<PmatchAlphabet> {
        // C++ 'PmatchAlphabet(istream, n, cont)' derives from
        // 'TransducerAlphabet(istream, n, true)' then builds the pmatch symbol
        // maps; read the base alphabet from the stream and reuse the same
        // map-building done by 'new_from_alphabet'.
        let base = TransducerAlphabet::new_istream(inputstream, symbol_count, true)?;
        Ok(Self::new_from_alphabet(&base, cont))
    }

    // ctor from existing alphabet: PmatchAlphabet(TransducerAlphabet const&, PmatchContainer*)
    pub fn new_from_alphabet(a: &TransducerAlphabet, cont: &mut PmatchContainer) -> PmatchAlphabet {
        let base = a.clone();
        let orig_symbol_count = base.get_orig_symbol_count();
        let mut alpha = PmatchAlphabet {
            base,
            rtns: RtnVector::new(),
            input_mark_symbol: 0,
            special_symbols: vec![NO_SYMBOL_NUMBER; SpecialSymbol::SPECIALSYMBOL_NR_ITEMS as usize],
            end_tag_map: BTreeMap::new(),
            capture_tag_map: BTreeMap::new(),
            captured_tag_map: BTreeMap::new(),
            capture2captured: SymbolNumberVector::new(),
            captured2capture: SymbolNumberVector::new(),
            rtn_names: RtnNameMap::new(),
            symbol2lists: SymbolNumberVector::new(),
            list2symbols: SymbolNumberVector::new(),
            exclusionary_lists: SymbolNumberVector::new(),
            symbol_lists: Vec::new(),
            symbol_list_members: Vec::new(),
            counters: Vec::new(),
            guards: SymbolNumberVector::new(),
            global_flags: Vec::new(),
            printable_vector: Vec::new(),
        };
        alpha.symbol2lists = vec![NO_SYMBOL_NUMBER; orig_symbol_count as usize];
        alpha.list2symbols = vec![NO_SYMBOL_NUMBER; orig_symbol_count as usize];
        alpha.capture2captured = vec![NO_SYMBOL_NUMBER; orig_symbol_count as usize];
        alpha.captured2capture = vec![NO_SYMBOL_NUMBER; orig_symbol_count as usize];
        alpha.rtns = (0..orig_symbol_count as usize).map(|_| None).collect();
        // We initialize the vector of which symbols have a printable
        // representation with false, then flip those that actually do to true
        alpha.printable_vector = vec![false; orig_symbol_count as usize];
        alpha.global_flags = vec![false; orig_symbol_count as usize];
        let mut i: SymbolNumber = 1;
        while (i as usize) < alpha.base.symbol_table.len() {
            let sym = alpha.base.symbol_table[i as usize].clone();
            if Self::is_special(&sym) {
                alpha.add_special_symbol(&sym, i, cont);
            } else if sym == "@PMATCH_INPUT_MARK@" {
                alpha.input_mark_symbol = i;
            } else if !alpha.is_flag_diacritic(i) {
                alpha.printable_vector[i as usize] = true;
            } else if Self::is_global_flag(&sym) {
                alpha.global_flags[i as usize] = true;
                // redefine it as a non-global flag, removing the
                // PMATCH_GLOBAL_ part
                let feature = crate::hfst_flag_diacritics::FdOperation::get_feature(&sym)
                    ["PMATCH_GLOBAL_".len()..]
                    .to_string();
                let value = crate::hfst_flag_diacritics::FdOperation::get_value(&sym);
                let new_diacritic = format!(
                    "{}{}{}@",
                    &sym[..3],
                    feature,
                    if value.is_empty() {
                        String::new()
                    } else {
                        format!(".{}", value)
                    }
                );
                alpha.base.fd_table.define_diacritic(i, &new_diacritic);
                // finally go over all other known flag diacritics with the
                // non-globalized feature and mark them global too
                for it in alpha.base.fd_table.get_symbols_with_feature(&feature) {
                    alpha.global_flags[it as usize] = true;
                }
            }
            i += 1;
        }
        alpha
    }

    // PmatchAlphabet(void)
    pub fn new() -> PmatchAlphabet {
        PmatchAlphabet {
            base: TransducerAlphabet::new(),
            rtns: RtnVector::new(),
            input_mark_symbol: 0,
            special_symbols: SymbolNumberVector::new(),
            end_tag_map: BTreeMap::new(),
            capture_tag_map: BTreeMap::new(),
            captured_tag_map: BTreeMap::new(),
            capture2captured: SymbolNumberVector::new(),
            captured2capture: SymbolNumberVector::new(),
            rtn_names: RtnNameMap::new(),
            symbol2lists: SymbolNumberVector::new(),
            list2symbols: SymbolNumberVector::new(),
            exclusionary_lists: SymbolNumberVector::new(),
            symbol_lists: Vec::new(),
            symbol_list_members: Vec::new(),
            counters: Vec::new(),
            guards: SymbolNumberVector::new(),
            global_flags: Vec::new(),
            printable_vector: Vec::new(),
        }
    }

    // ---- forwards to the base TransducerAlphabet (composition) ----
    pub fn get_symbol_table(&self) -> &crate::transducer::SymbolTable {
        self.base.get_symbol_table()
    }
    pub fn string_from_symbol(&self, symbol: SymbolNumber) -> crate::hfst_data_types::Symbol {
        self.base.string_from_symbol(symbol)
    }
    pub fn symbol_from_string(&self, s: &str) -> Option<SymbolNumber> {
        self.base.symbol_from_string(s)
    }
    pub fn build_string_symbol_map(&self) -> crate::transducer::StringSymbolMap {
        self.base.build_string_symbol_map()
    }
    pub fn is_flag_diacritic(&self, s: SymbolNumber) -> bool {
        self.base.is_flag_diacritic(s)
    }
    pub fn get_operation(
        &self,
        s: SymbolNumber,
    ) -> Option<&crate::hfst_flag_diacritics::FdOperation> {
        self.base.get_operation(s)
    }
    pub fn get_fd_table(&self) -> &FdTable<SymbolNumber> {
        self.base.get_fd_table()
    }
    pub fn get_unknown_symbol(&self) -> SymbolNumber {
        self.base.get_unknown_symbol()
    }
    pub fn get_default_symbol(&self) -> SymbolNumber {
        self.base.get_default_symbol()
    }
    pub fn get_identity_symbol(&self) -> SymbolNumber {
        self.base.get_identity_symbol()
    }
    pub fn get_orig_symbol_count(&self) -> SymbolNumber {
        self.base.get_orig_symbol_count()
    }
    pub fn is_unicode_alpha(&mut self, symbol: SymbolNumber) -> bool {
        self.base.is_unicode_alpha(symbol)
    }
    pub fn is_unicode_upperalpha(&mut self, symbol: SymbolNumber) -> bool {
        self.base.is_unicode_upperalpha(symbol)
    }
    pub fn is_unicode_loweralpha(&mut self, symbol: SymbolNumber) -> bool {
        self.base.is_unicode_loweralpha(symbol)
    }
    pub fn is_unicode_whitespace(&mut self, symbol: SymbolNumber) -> bool {
        self.base.is_unicode_whitespace(symbol)
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.add-symbol-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.add-symbol-fn]
    // override void add_symbol(const std::string &)
    pub fn add_symbol(&mut self, symbol: &crate::hfst_data_types::Symbol) {
        self.symbol2lists.push(NO_SYMBOL_NUMBER);
        self.list2symbols.push(NO_SYMBOL_NUMBER);
        self.capture2captured.push(NO_SYMBOL_NUMBER);
        self.captured2capture.push(NO_SYMBOL_NUMBER);
        self.rtns.push(None);
        self.printable_vector.push(true);
        if !self.exclusionary_lists.is_empty() {
            // if there are exclusionary lists, they should all accept the new
            // symbol
            self.symbol2lists[self.base.symbol_table.len()] =
                u16::try_from(self.symbol_lists.len()).expect("value out of u16 range");
            self.symbol_lists.push(self.exclusionary_lists.clone());
            for exc in self.exclusionary_lists.clone() {
                let idx = self.list2symbols[exc as usize] as usize;
                self.symbol_list_members[idx].push(
                    u16::try_from(self.base.symbol_table.len()).expect("value out of u16 range"),
                );
            }
        }
        self.base.add_symbol(symbol);
    }
    // convenience for the &str-taking callers (e.g. add_symbol(new_symbol) where
    // new_symbol is a char*); forwards to add_symbol.
    pub fn add_symbol_str(&mut self, symbol: &str) {
        self.add_symbol(&crate::hfst_data_types::Symbol::new(symbol))
    }

    // ---- static string predicates ----

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-end-tag-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-end-tag-fn]
    pub fn is_end_tag(symbol: &str) -> bool {
        symbol.find("@PMATCH_ENDTAG_") == Some(0) && symbol.rfind('@') == Some(symbol.len() - 1)
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-capture-tag-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-capture-tag-fn]
    pub fn is_capture_tag(symbol: &str) -> bool {
        symbol.find("@PMATCH_CAPTURE_") == Some(0) && symbol.rfind('@') == Some(symbol.len() - 1)
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-captured-tag-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-captured-tag-fn]
    pub fn is_captured_tag(symbol: &str) -> bool {
        symbol.find("@PMATCH_CAPTURED_") == Some(0) && symbol.rfind('@') == Some(symbol.len() - 1)
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-insertion-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-insertion-fn]
    pub fn is_insertion(symbol: &str) -> bool {
        symbol.find("@I.") == Some(0) && symbol.rfind('@') == Some(symbol.len() - 1)
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-guard-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-guard-fn]
    pub fn is_guard(symbol: &str) -> bool {
        symbol.find("@PMATCH_GUARD_") == Some(0) && symbol.rfind('@') == Some(symbol.len() - 1)
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-list-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-list-fn]
    pub fn is_list(symbol: &str) -> bool {
        (symbol.find("@L.") == Some(0) || symbol.find("@X.") == Some(0))
            && symbol.rfind('@') == Some(symbol.len() - 1)
            && symbol.len() > 4
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-underscored-list-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-underscored-list-fn]
    pub fn is_underscored_list(symbol: &str) -> bool {
        (symbol.find("@L.") == Some(0) || symbol.find("@X.") == Some(0))
            && symbol.rfind("_@") == Some(symbol.len() - 2)
            && symbol.len() > 5
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-counter-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-counter-fn]
    pub fn is_counter(symbol: &str) -> bool {
        symbol.find("@PMATCH_COUNTER_") == Some(0) && symbol.rfind('@') == Some(symbol.len() - 1)
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-special-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-special-fn]
    pub fn is_special(symbol: &str) -> bool {
        if symbol.len() < 3 {
            return false;
        }
        if symbol == "@PMATCH_INPUT_MARK@" || symbol == "@PMATCH_BACKTRACK@" {
            // is_special symbols can't be referred to in pmatch scripts
            return false;
        }
        if Self::is_insertion(symbol)
            || symbol == "@BOUNDARY@"
            || symbol == "@UNICODE_ALPHA@"
            || symbol == "@UNICODE_UPPERALPHA@"
            || symbol == "@UNICODE_LOWERALPHA@"
            || symbol == "@UNICODE_WHITESPACE@"
        {
            true
        } else {
            (symbol.find("@PMATCH") == Some(0) && symbol.as_bytes()[symbol.len() - 1] == b'@')
                || Self::is_list(symbol)
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-printable-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-printable-fn]
    pub fn is_printable(symbol: &str) -> bool {
        if symbol.len() < 3 {
            return true;
        }
        symbol.find('@') != Some(0) || symbol.as_bytes()[symbol.len() - 1] != b'@'
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-global-flag-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-global-flag-fn]
    pub fn is_global_flag(symbol: &str) -> bool {
        (symbol.find("@P.") == Some(0) || symbol.find("@C.") == Some(0))
            && symbol.find("PMATCH_GLOBAL_") == Some(3)
            && symbol.rfind('@') == Some(symbol.len() - 1)
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.name-from-insertion-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.name-from-insertion-fn]
    pub fn name_from_insertion(symbol: &str) -> String {
        // C++ symbol.substr(sizeof("@I.") - 1, symbol.size() - (sizeof("@I.@") - 1)):
        // drop the leading "@I." (3 bytes) and the trailing "@", so "@I.Animal@"
        // yields "Animal". 'sizeof("@I.@")' is 5 in C (counts the NUL), so the
        // count is 'len - 4'; the earlier port mistranslated it as '"@I.@".len()
        // - 1' (== 3) and left the trailing "@" on the name, so RTN members never
        // matched their insertion symbol. [upstream hfst/hfst#354]
        symbol[("@I.".len())..(symbol.len() - 1)].to_string()
    }

    // ---- SymbolNumber predicates (member, non-static) ----

    pub fn is_end_tag_sym(&self, symbol: SymbolNumber) -> bool {
        self.end_tag_map.contains_key(&symbol)
    }
    pub fn is_capture_tag_sym(&self, symbol: SymbolNumber) -> bool {
        self.capture2captured[symbol as usize] != NO_SYMBOL_NUMBER
    }
    pub fn is_captured_tag_sym(&self, symbol: SymbolNumber) -> bool {
        self.captured2capture[symbol as usize] != NO_SYMBOL_NUMBER
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-input-mark-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-input-mark-fn]
    pub fn is_input_mark(&self, symbol: SymbolNumber) -> bool {
        self.input_mark_symbol == symbol
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-guard-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-guard-fn]
    pub fn is_guard_sym(&self, symbol: SymbolNumber) -> bool {
        for it in self.guards.iter() {
            if symbol == *it {
                return true;
            }
        }
        false
    }
    pub fn is_counter_sym(&self, symbol: SymbolNumber) -> bool {
        (symbol as usize) < self.counters.len() && self.counters[symbol as usize] != NO_COUNTER
    }
    pub fn is_global_flag_sym(&self, symbol: SymbolNumber) -> bool {
        // 'add_symbol' does not grow 'global_flags' (C++ leaves it at
        // orig_symbol_count too); symbols added later are never global flags.
        (symbol as usize) < self.global_flags.len() && self.global_flags[symbol as usize]
    }
    pub fn is_printable_sym(&self, symbol: SymbolNumber) -> bool {
        (symbol as usize) < self.printable_vector.len() && self.printable_vector[symbol as usize]
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.end-tag-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.end-tag-fn]
    pub fn end_tag(&self, symbol: SymbolNumber) -> String {
        if !self.end_tag_map.contains_key(&symbol) {
            String::new()
        } else {
            format!("</{}>", self.end_tag_map[&symbol])
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.start-tag-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.start-tag-fn]
    pub fn start_tag(&self, symbol: SymbolNumber) -> String {
        if !self.end_tag_map.contains_key(&symbol) {
            String::new()
        } else {
            format!("<{}>", self.end_tag_map[&symbol])
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.is-meta-arc-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.is-meta-arc-fn]
    // override
    pub fn is_meta_arc(&self, symbol: SymbolNumber) -> bool {
        self.base.is_meta_arc(symbol)
            || symbol == self.get_special(SpecialSymbol::UnicodeAlpha)
            || symbol == self.get_special(SpecialSymbol::UnicodeUpperAlpha)
            || symbol == self.get_special(SpecialSymbol::UnicodeLowerAlpha)
            || symbol == self.get_special(SpecialSymbol::UnicodeWhitespace)
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.add-special-symbol-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.add-special-symbol-fn]
    pub fn add_special_symbol(
        &mut self,
        str: &str,
        symbol_number: SymbolNumber,
        container: &mut PmatchContainer,
    ) {
        if str == "@PMATCH_ENTRY@" {
            self.special_symbols[SpecialSymbol::entry as usize] = symbol_number;
        } else if str == "@PMATCH_EXIT@" {
            self.special_symbols[SpecialSymbol::exit as usize] = symbol_number;
        } else if str == "@PMATCH_LC_ENTRY@" {
            self.special_symbols[SpecialSymbol::LC_entry as usize] = symbol_number;
        } else if str == "@PMATCH_RC_ENTRY@" {
            self.special_symbols[SpecialSymbol::RC_entry as usize] = symbol_number;
        } else if str == "@PMATCH_LC_EXIT@" {
            self.special_symbols[SpecialSymbol::LC_exit as usize] = symbol_number;
        } else if str == "@PMATCH_RC_EXIT@" {
            self.special_symbols[SpecialSymbol::RC_exit as usize] = symbol_number;
        } else if str == "@PMATCH_NLC_ENTRY@" {
            self.special_symbols[SpecialSymbol::NLC_entry as usize] = symbol_number;
        } else if str == "@PMATCH_NRC_ENTRY@" {
            self.special_symbols[SpecialSymbol::NRC_entry as usize] = symbol_number;
        } else if str == "@PMATCH_NLC_EXIT@" {
            self.special_symbols[SpecialSymbol::NLC_exit as usize] = symbol_number;
        } else if str == "@PMATCH_NRC_EXIT@" {
            self.special_symbols[SpecialSymbol::NRC_exit as usize] = symbol_number;
        } else if str == "@PMATCH_PASSTHROUGH@" {
            self.special_symbols[SpecialSymbol::Pmatch_passthrough as usize] = symbol_number;
        } else if str == "@BOUNDARY@" {
            self.special_symbols[SpecialSymbol::boundary as usize] = symbol_number;
        } else if str == "@UNICODE_ALPHA@" {
            self.special_symbols[SpecialSymbol::UnicodeAlpha as usize] = symbol_number;
        } else if str == "@UNICODE_UPPERALPHA@" {
            self.special_symbols[SpecialSymbol::UnicodeUpperAlpha as usize] = symbol_number;
        } else if str == "@UNICODE_LOWERALPHA@" {
            self.special_symbols[SpecialSymbol::UnicodeLowerAlpha as usize] = symbol_number;
        } else if str == "@UNICODE_WHITESPACE@" {
            self.special_symbols[SpecialSymbol::UnicodeWhitespace as usize] = symbol_number;
        } else if Self::is_end_tag(str) {
            // Fetch the part between @PMATCH_ENDTAG_ and @
            // str.substr(sizeof("@PMATCH_ENDTAG_") - 1,
            //            str.size() - (sizeof("@PMATCH_ENDTAG_@") - 1))
            let begin = "@PMATCH_ENDTAG_".len();
            let count = str.len() - ("@PMATCH_ENDTAG_@".len());
            self.end_tag_map
                .insert(symbol_number, str[begin..begin + count].to_string());
        } else if Self::is_capture_tag(str) {
            let begin = "@PMATCH_CAPTURE_".len();
            let count = str.len() - ("@PMATCH_CAPTURE_@".len());
            let name_of_capture = str[begin..begin + count].to_string();
            self.capture_tag_map
                .insert(name_of_capture.clone(), symbol_number);
            if self.captured_tag_map.contains_key(&name_of_capture) {
                let captured = self.captured_tag_map[&name_of_capture];
                self.capture2captured[symbol_number as usize] = captured;
                self.captured2capture[captured as usize] = symbol_number;
            }
        } else if Self::is_captured_tag(str) {
            let begin = "@PMATCH_CAPTURED_".len();
            let count = str.len() - ("@PMATCH_CAPTURED_@".len());
            let name_of_captured = str[begin..begin + count].to_string();
            self.captured_tag_map
                .insert(name_of_captured.clone(), symbol_number);
            if self.capture_tag_map.contains_key(&name_of_captured) {
                let capture = self.capture_tag_map[&name_of_captured];
                self.captured2capture[symbol_number as usize] = capture;
                self.capture2captured[capture as usize] = symbol_number;
            }
        } else if Self::is_insertion(str) {
            self.rtn_names
                .insert(Self::name_from_insertion(str), symbol_number);
        } else if Self::is_guard(str) {
            self.guards.push(symbol_number);
        } else if Self::is_underscored_list(str) {
            self.process_underscored_symbol_list(str, symbol_number);
        } else if Self::is_list(str) {
            self.process_symbol_list(str, symbol_number, container);
        } else if Self::is_counter(str) {
            self.process_counter(str.to_string(), symbol_number);
        } else {
            self.printable_vector[symbol_number as usize] = true;
            // it's a regular symbol, we shouldn't be here!
            //        std::cerr << "pmatch: warning: symbol " << str << " was
            //        wrongly given as a special symbol\n";
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.process-underscored-symbol-list-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.process-underscored-symbol-list-fn]
    pub fn process_underscored_symbol_list(&mut self, str: &str, sym: SymbolNumber) {
        let mut list_symbols: SymbolNumberVector = SymbolNumberVector::new();
        let ss = self.build_string_symbol_map();
        // regular list or exlusionary list?
        let polarity = str.as_bytes()[1] == b'L';
        let mut begin = "@L.".len();
        let mut collected_symbols: Vec<crate::hfst_data_types::Symbol> = Vec::new();
        while let Some(stop) = str[begin..].find('_').map(|p| p + begin) {
            // For each underscore after the prelude, grab the substring
            let mut symbol = crate::hfst_data_types::Symbol::new(&str[begin..stop]);
            if symbol.is_empty() {
                // If the symbol _is_ an underscore it looks like we got an empty
                // string
                symbol = crate::hfst_data_types::Symbol::new_static("_");
                begin = stop + 2;
            } else {
                begin = stop + 1;
            }
            collected_symbols.push(symbol);
        }
        // Process the symbols we found
        for it in collected_symbols.iter() {
            let str_sym: SymbolNumber;
            if !ss.contains_key(it) {
                // This symbol isn't mentioned elsewhere in the alphabet
                self.add_symbol(it);
                str_sym = self.base.orig_symbol_count;
                self.base.orig_symbol_count += 1;
            } else {
                str_sym = ss[it];
            }
            list_symbols.push(str_sym);
            if polarity {
                if self.symbol2lists[str_sym as usize] == NO_SYMBOL_NUMBER {
                    self.symbol2lists[str_sym as usize] =
                        u16::try_from(self.symbol_lists.len()).expect("value out of u16 range");
                    self.symbol_lists.push(vec![sym]);
                } else {
                    let idx = self.symbol2lists[str_sym as usize] as usize;
                    self.symbol_lists[idx].push(sym);
                }
            }
        }
        self.list2symbols[sym as usize] =
            u16::try_from(self.symbol_list_members.len()).expect("value out of u16 range");
        if !polarity {
            let mut excl_symbols: SymbolNumberVector = SymbolNumberVector::new();
            self.exclusionary_lists.push(sym);
            let mut candidate_for_list: SymbolNumber = 1;
            while (candidate_for_list as usize) < self.base.symbol_table.len() {
                if Self::is_printable(&self.base.symbol_table[candidate_for_list as usize])
                    && !list_symbols.contains(&candidate_for_list)
                {
                    excl_symbols.push(candidate_for_list);
                    if self.symbol2lists[candidate_for_list as usize] == NO_SYMBOL_NUMBER {
                        // This symbol is not yet associated with any list
                        self.symbol2lists[candidate_for_list as usize] =
                            u16::try_from(self.symbol_lists.len()).expect("value out of u16 range");
                        self.symbol_lists.push(vec![sym]);
                    } else {
                        let idx = self.symbol2lists[candidate_for_list as usize] as usize;
                        self.symbol_lists[idx].push(sym);
                    }
                }
                candidate_for_list += 1;
            }
            self.symbol_list_members.push(excl_symbols);
        } else {
            self.symbol_list_members.push(list_symbols);
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.process-symbol-list-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.process-symbol-list-fn]
    // C++ calls container->symbol_vector_from_symbols, hence the &mut container param.
    pub fn process_symbol_list(
        &mut self,
        str: &str,
        sym: SymbolNumber,
        container: &mut PmatchContainer,
    ) {
        let polarity = str.as_bytes()[1] == b'L';
        let begin = "@L.".len();
        let stop = str.len() - begin - "@".len();

        let list_symbols: SymbolNumberVector =
            container.symbol_vector_from_symbols(&str[begin..begin + stop]);

        // Process the symbols we found
        for it in list_symbols.iter() {
            if polarity {
                if self.symbol2lists[*it as usize] == NO_SYMBOL_NUMBER {
                    self.symbol2lists[*it as usize] =
                        u16::try_from(self.symbol_lists.len()).expect("value out of u16 range");
                    self.symbol_lists.push(vec![sym]);
                } else {
                    let idx = self.symbol2lists[*it as usize] as usize;
                    self.symbol_lists[idx].push(sym);
                }
            }
        }
        self.list2symbols[sym as usize] =
            u16::try_from(self.symbol_list_members.len()).expect("value out of u16 range");
        if !polarity {
            let mut excl_symbols: SymbolNumberVector = SymbolNumberVector::new();
            self.exclusionary_lists.push(sym);
            let mut candidate_for_list: SymbolNumber = 1;
            while (candidate_for_list as usize) < self.base.symbol_table.len() {
                if Self::is_printable(&self.base.symbol_table[candidate_for_list as usize])
                    && !list_symbols.contains(&candidate_for_list)
                {
                    excl_symbols.push(candidate_for_list);
                    if self.symbol2lists[candidate_for_list as usize] == NO_SYMBOL_NUMBER {
                        self.symbol2lists[candidate_for_list as usize] =
                            u16::try_from(self.symbol_lists.len()).expect("value out of u16 range");
                        self.symbol_lists.push(vec![sym]);
                    } else {
                        // NOTE: faithful to the C++ bug — indexes by symbol2lists[sym]
                        // and pushes sym (not candidate_for_list).
                        let idx = self.symbol2lists[sym as usize] as usize;
                        self.symbol_lists[idx].push(sym);
                    }
                }
                candidate_for_list += 1;
            }
            self.symbol_list_members.push(excl_symbols);
        } else {
            self.symbol_list_members.push(list_symbols);
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.process-counter-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.process-counter-fn]
    pub fn process_counter(&mut self, str: String, sym: SymbolNumber) {
        let _ = str;
        // Fill up non-counter spots in the counter vector with blanks
        while self.counters.len() < sym as usize {
            self.counters.push(NO_COUNTER);
        }
        self.counters.push(0);
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.count-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.count-fn]
    pub fn count(&mut self, sym: SymbolNumber) {
        if self.is_counter_sym(sym) {
            self.counters[sym as usize] += 1;
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.add-rtn-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.add-rtn-fn]
    pub fn add_rtn(&mut self, rtn: Box<PmatchTransducer>, name: &str) {
        // C++ 'rtn_names[name]' on std::map default-inserts 0 for an unknown
        // name; mirror that so archives carrying named transducers that TOP
        // never references still load.
        let symbol = *self.rtn_names.entry(name.to_string()).or_insert(0);
        self.rtns[symbol as usize] = Some(rtn);
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.has-rtn-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.has-rtn-fn]
    pub fn has_rtn(&self, name: &str) -> bool {
        if name == "TOP" {
            return true;
        }
        self.rtn_names.contains_key(name)
            && (self.rtn_names[name] as usize) < self.rtns.len()
            && self.rtns[self.rtn_names[name] as usize].is_some()
    }
    pub fn has_rtn_sym(&self, symbol: SymbolNumber) -> bool {
        (symbol as usize) < self.rtns.len() && self.rtns[symbol as usize].is_some()
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.get-rtn-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.get-rtn-fn]
    // Returns the RTN by symbol. In C++ this returns a raw 'PmatchTransducer *'.
    // Because the container owns the RTNs and the engine needs &mut access to
    // both the RTN and the container, the recursive callers instead 'std::mem::take'
    // the box out of the slot for the duration of the call (see notes). This
    // convenience accessor unwraps the Option.
    pub fn get_rtn(&mut self, symbol: SymbolNumber) -> &mut Box<PmatchTransducer> {
        self.rtns[symbol as usize]
            .as_mut()
            .expect("RTN slot occupied for this symbol")
    }
    pub fn get_rtn_by_name(&mut self, name: String) -> &mut Box<PmatchTransducer> {
        let symbol = self.rtn_names[&name];
        self.rtns[symbol as usize]
            .as_mut()
            .expect("RTN slot occupied for this symbol")
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.get-counter-name-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.get-counter-name-fn]
    pub fn get_counter_name(&self, symbol: SymbolNumber) -> String {
        if self.base.symbol_table.len() <= symbol as usize {
            return "INVALID_COUNTER".to_string();
        }
        let name = self.base.symbol_table[symbol as usize].clone();
        if !Self::is_counter(&name) {
            return "INVALID_COUNTER".to_string();
        }
        let begin = "@PMATCH_COUNTER_".len();
        let count = name.len() - "@PMATCH_COUNTER_".len() - 1;
        name[begin..begin + count].to_string()
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.get-special-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.get-special-fn]
    pub fn get_special(&self, special: SpecialSymbol) -> SymbolNumber {
        self.special_symbols[special as usize]
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.get-specials-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.get-specials-fn]
    pub fn get_specials(&self) -> SymbolNumberVector {
        let mut v: SymbolNumberVector = SymbolNumberVector::new();
        for it in self.special_symbols.iter() {
            if *it != NO_SYMBOL_NUMBER {
                v.push(*it);
            }
        }
        v
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.stringify-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.stringify-fn]
    // Reads/writes container.pattern_counts and reads the pattern flags, so the
    // container is passed in (C++ uses the back-pointer 'container').
    // Takes the container fields it touches (the pattern-mode flags by value,
    // the counts map by exclusive reference) rather than the whole container,
    // so the caller can hand this its 'alphabet' and 'pattern_counts' fields as
    // two disjoint borrows.
    pub fn stringify(
        &self,
        str: &DoubleTape,
        count_patterns: bool,
        delete_patterns: bool,
        mark_patterns: bool,
        extract_patterns: bool,
        pattern_counts: &mut BTreeMap<String, usize>,
    ) -> String {
        let mut retval = String::new();
        let mut start_tag_pos: Vec<u32> = Vec::new();
        let mut input_contained_printable_symbol = false;
        for it in str.inner.iter() {
            if !input_contained_printable_symbol && self.is_printable_sym(it.input) {
                input_contained_printable_symbol = true;
            }
            let output = it.output;
            if output == self.special_symbols[SpecialSymbol::entry as usize] {
                start_tag_pos.push(u32::try_from(retval.len()).expect("value out of u32 range"));
            } else if output == self.special_symbols[SpecialSymbol::exit as usize] {
                if !start_tag_pos.is_empty() {
                    start_tag_pos.pop();
                }
            } else if self.is_end_tag_sym(output) {
                if count_patterns && input_contained_printable_symbol {
                    let key = self.start_tag(output);
                    *pattern_counts.entry(key).or_insert(0) += 1;
                }
                let pos: u32 = if start_tag_pos.is_empty() {
                    warn!("end tag without start tag");
                    0
                } else {
                    *start_tag_pos
                        .last()
                        .expect("stack non-empty in else branch")
                };
                if delete_patterns {
                    let how_much_to_delete = retval.len() - pos as usize;
                    retval.replace_range(
                        pos as usize..pos as usize + how_much_to_delete,
                        &self.start_tag(output),
                    );
                } else if mark_patterns && input_contained_printable_symbol {
                    retval.insert_str(pos as usize, &self.start_tag(output));
                    retval.push_str(&self.end_tag(output));
                }
            } else if (!extract_patterns || !start_tag_pos.is_empty())
                && self.is_printable_sym(output)
            {
                retval.push_str(&self.string_from_symbol(output));
            }
        }
        retval
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-alphabet.locatefy-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-alphabet.locatefy-fn]
    pub fn locatefy(
        &self,
        input_offset: u32,
        str: &WeightedDoubleTape,
        count_patterns: bool,
        pattern_counts: &mut BTreeMap<String, usize>,
    ) -> Location {
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
            if self.is_end_tag_sym(output) {
                if count_patterns {
                    let key = self.start_tag(output);
                    *pattern_counts.entry(key).or_insert(0) += 1;
                }
                retval.tag = self.start_tag(output);
                continue;
            }
            if self.is_printable_sym(output) {
                let s = self.string_from_symbol(output);
                retval.output.push_str(&s);
                retval.output_symbol_strings.push(s);
            }
            if self.is_printable_sym(input) {
                let s = self.string_from_symbol(input);
                retval.input.push_str(&s);
                retval.input_symbol_strings.push(s);
                input_offset += 1;
            }
            if self.is_input_mark(output) {
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
}

// ==================== PmatchContainer (impl from workflow body agent) ====================
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
impl Default for PmatchContainer {
    fn default() -> Self {
        Self::new()
    }
}

impl PmatchContainer {
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.pmatch-container-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.pmatch-container-fn]
    // explicit PmatchContainer(std::istream &) — reads a binary pmatch archive:
    // the TOP transducer followed by any UNCOMPOSE L/R nets and RTN sub-nets.
    pub fn new_from_stream(
        is: &mut crate::transducer::IStream<'_>,
    ) -> crate::error::Result<PmatchContainer> {
        use crate::transducer::{Encoder, TransducerAlphabet, TransducerHeader};
        let mut c = PmatchContainer::new();
        c.set_properties();
        c.reset_recursion();
        let mut properties = Self::parse_hfst3_header(is)?;
        let transducer_name: String;
        if !properties.contains_key("name") {
            warn!("TOP not defined in archive, using first as TOP");
            transducer_name = "TOP".to_string();
        } else {
            transducer_name = properties["name"].clone();
            if transducer_name != "TOP" {
                warn!("TOP not defined in archive, using first as TOP");
            }
        }
        let _ = transducer_name;
        if !properties.contains_key("type") {
            warn!("type information missing from archive");
        } else if properties["type"] != "HFST_OLW" {
            warn!("archive type isn't weighted optimized-lookup according to header");
        }
        c.set_properties_map(&properties);
        let header = TransducerHeader::new_istream(is)?;
        c.alphabet = PmatchAlphabet::new_from_stream(is, header.symbol_count(), &mut c)?;
        c.orig_symbol_count = c.alphabet.get_orig_symbol_count();
        c.symbol_count = c.alphabet.get_orig_symbol_count();
        c.global_flag_state = FdState::new(c.alphabet.get_fd_table());
        c.encoder = Some(Encoder::new(
            c.alphabet.get_symbol_table(),
            c.orig_symbol_count,
        ));
        if properties.contains_key("initial-symbols") {
            let initial = properties["initial-symbols"].clone();
            c.collect_first_symbols(&initial);
        }
        let top = PmatchTransducer::new_from_stream(
            is,
            header.index_table_size(),
            header.target_table_size(),
            &c.alphabet,
            "TOP".to_string(),
        );
        c.toplevel = Some(Box::new(top));
        // C++ loops 'while (inputstream.good())' reading further archive members,
        // breaking when parse_hfst3_header throws TransducerHeaderException. A
        // well-formed archive ends in a clean EOF right after the last member, so
        // peek for end-of-stream (now possible via get/putback) instead of
        // catching the throw.
        loop {
            if !is.good() {
                break;
            }
            let probe = is.get();
            if probe < 0 {
                break;
            }
            is.putback(probe as u8);
            properties = Self::parse_hfst3_header(is)?;
            let transducer_name = properties.get("name").cloned().unwrap_or_default();
            if transducer_name.starts_with("UNCOMPOSE LEFT") {
                c.uncompose_left = Some(Box::new(crate::transducer::Transducer::new_istream(is)?));
                if c.verbose {
                    debug!("Reading uncomposer L... {} done", transducer_name);
                }
                c.uncomposable = true;
            } else if transducer_name.starts_with("UNCOMPOSE RIGHT") {
                c.uncompose_right = Some(Box::new(crate::transducer::Transducer::new_istream(is)?));
                if c.verbose {
                    debug!("Reading uncomposer R... {} done", transducer_name);
                }
                c.uncomposable = true;
            } else {
                let rtn_header = TransducerHeader::new_istream(is)?;
                let _dummy = TransducerAlphabet::new_istream(is, rtn_header.symbol_count(), true)?;
                let rtn = PmatchTransducer::new_from_stream(
                    is,
                    rtn_header.index_table_size(),
                    rtn_header.target_table_size(),
                    &c.alphabet,
                    transducer_name.clone(),
                );
                if !c.alphabet.has_rtn(&transducer_name) {
                    c.alphabet.add_rtn(Box::new(rtn), &transducer_name);
                }
                // else: C++ 'delete rtn' — Rust drops it here.
            }
        }
        Ok(c)
    }

    // PmatchContainer(Transducer *t)
    pub fn new_from_transducer(
        toplevel: Box<crate::transducer::Transducer>,
    ) -> crate::error::Result<PmatchContainer> {
        let mut c = PmatchContainer::new();
        c.set_properties();
        c.reset_recursion();
        // TransducerHeader header = t->get_header();
        c.alphabet = PmatchAlphabet::new_from_alphabet(toplevel.get_alphabet(), &mut c);
        c.orig_symbol_count = c.alphabet.get_orig_symbol_count();
        c.symbol_count = c.alphabet.get_orig_symbol_count();
        c.global_flag_state = FdState::new(c.alphabet.get_fd_table());
        c.line_number = 0;
        c.encoder = Some(Encoder::new(
            c.alphabet.get_symbol_table(),
            c.orig_symbol_count,
        ));
        let transitions = toplevel.copy_transitionw_table()?;
        let indices = toplevel.copy_windex_table()?;
        let top = PmatchTransducer::new_from_vectors(
            transitions.get_vector().clone(),
            indices.get_vector().clone(),
            &c.alphabet,
            "TOP".to_string(),
        );
        c.toplevel = Some(Box::new(top));
        Ok(c)
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.pmatch-container-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.pmatch-container-fn]
    // explicit PmatchContainer(std::vector<hfst::HfstTransducer>)
    pub fn new_from_hfst_transducers(
        transducers: Vec<crate::hfst_transducer::HfstTransducer<crate::transducer::Transducer>>,
    ) -> crate::error::Result<PmatchContainer> {
        if transducers.is_empty() {
            let mut c = PmatchContainer::new();
            c.set_properties();
            c.reset_recursion();
            return Ok(c);
        }
        if transducers.len() == 1 {
            // C++: convert transducers[0] to HFST_OLW (unless already), then
            // hfst_transducer_to_hfst_ol(top) to get the optimized-lookup backend,
            // and build the container from it (exactly new_from_transducer). The
            // pmatch runtime pins the weighted optimized-lookup instantiation
            // ([dec:hfst:monomorphic-backends]), so the runtime convert is a
            // static fact of the parameter type now.
            let top = &transducers[0];
            let backend = crate::transducer::Transducer::copy(&top.fst)?;
            let mut c = PmatchContainer::new_from_transducer(Box::new(backend))?;
            // C++ sets these from transducers[0]'s properties before building; the
            // build does not depend on them, so applying them afterwards is
            // equivalent.
            c.set_properties_map(transducers[0].get_properties());
            Ok(c)
        } else {
            // This is the difficult case where we have to make sure multiple
            // optimized-lookup transducers are harmonized with each other.
            use crate::convert_transducer_format::ConversionFunctions;

            let mut c = PmatchContainer::new();
            c.set_properties();
            c.reset_recursion();
            c.set_properties_map(transducers[0].get_properties());

            // A dummy transducer with an alphabet with all the symbols
            // (TROPICAL_OPENFST_TYPE in C++; the tropical backend type here).
            let mut harmonizer: crate::hfst_transducer::HfstTransducer<hfst_openfst::StdVectorFst> =
                crate::hfst_transducer::HfstTransducer::new();
            // First we need to collect a unified alphabet from all the
            // transducers.
            let mut symbols_seen: std::collections::BTreeSet<crate::hfst_data_types::Symbol> =
                std::collections::BTreeSet::new();
            // The TOP member: the last transducer named "TOP" (NULL == none).
            let mut top_index: Option<usize> = None;
            // We collect all the symbols and also locate the TOP member.
            for (i, transducer) in transducers.iter().enumerate() {
                let string_set = transducer.get_alphabet()?;
                for sym in string_set.iter() {
                    if !symbols_seen.contains(sym) {
                        let ht = crate::hfst_transducer::HfstTransducer::new_symbol(sym)?;
                        harmonizer.disjunct(&ht, true)?;
                        symbols_seen.insert(sym.clone());
                    }
                }
                if transducer.get_name() == "TOP" {
                    top_index = Some(i);
                }
            }
            let top_index = match top_index {
                Some(i) => i,
                None => {
                    warn!("TOP not defined in archive, using first as TOP");
                    0
                }
            };
            // Then we convert the harmonizer... (typed now: basic -> weighted
            // OL tables, the former convert(HFST_OLW_TYPE)).
            let harmonizer_net = harmonizer.get_basic_transducer()?;
            let harmonizer_ol_owned = ConversionFunctions::hfst_basic_transducer_to_hfst_ol(
                &harmonizer_net,
                true,
                "",
                None,
            )?;
            let harmonizer_ol = &harmonizer_ol_owned;

            // We take care of TOP first. Already OLW by type (the C++ convert
            // is static); go through an intermediate basic transducer, then
            // harmonize into OL.
            let top = transducers[top_index].clone();
            let intermediate_tmp =
                ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&top)?;
            let harmonized_tmp = ConversionFunctions::hfst_basic_transducer_to_hfst_ol(
                &intermediate_tmp,
                true,                // weighted
                "",                  // no special options
                Some(harmonizer_ol), // harmonize with this
            )?;
            // this will be the alphabet of the entire container
            c.alphabet = PmatchAlphabet::new_from_alphabet(harmonized_tmp.get_alphabet(), &mut c);
            c.orig_symbol_count = c.alphabet.get_orig_symbol_count();
            c.symbol_count = c.alphabet.get_orig_symbol_count();
            c.global_flag_state = FdState::new(c.alphabet.get_fd_table());
            c.encoder = Some(Encoder::new(
                c.alphabet.get_symbol_table(),
                c.orig_symbol_count,
            ));
            let transitions = harmonized_tmp.copy_transitionw_table()?;
            let indices = harmonized_tmp.copy_windex_table()?;
            let top_pt = PmatchTransducer::new_from_vectors(
                transitions.get_vector().clone(),
                indices.get_vector().clone(),
                &c.alphabet,
                "TOP".to_string(),
            );
            c.toplevel = Some(Box::new(top_pt));
            // Then we do the same for the other transducers except without
            // alphabets or encoders because those should be identical. Members
            // named "TOP" left a NULL slot in the C++ 'temporaries' vector and
            // are skipped here.
            for transducer in &transducers {
                if transducer.get_name() == "TOP" {
                    // there's a NULL where TOP should be
                    continue;
                }
                let temp = transducer.clone();
                let intermediate_tmp =
                    ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&temp)?;
                let harmonized_tmp = ConversionFunctions::hfst_basic_transducer_to_hfst_ol(
                    &intermediate_tmp,
                    true,
                    "",
                    Some(harmonizer_ol),
                )?;
                let transitions = harmonized_tmp.copy_transitionw_table()?;
                let indices = harmonized_tmp.copy_windex_table()?;
                let name = transducer.get_name();
                let rtn = PmatchTransducer::new_from_vectors(
                    transitions.get_vector().clone(),
                    indices.get_vector().clone(),
                    &c.alphabet,
                    name.clone(),
                );
                c.alphabet.add_rtn(Box::new(rtn), &name);
            }
            Ok(c)
        }
    }

    // PmatchContainer(void)
    pub fn new() -> PmatchContainer {
        // Not used, but apparently needed by swig to construct these
        PmatchContainer {
            alphabet: PmatchAlphabet::new(),
            encoder: None,
            orig_symbol_count: 0,
            symbol_count: 0,
            toplevel: None,
            input: SymbolNumberVector::new(),
            entry_stack: Vec::new(),
            rtn_stacks: RtnCallStacks::new(),
            uncompose_left: None,
            uncompose_right: None,
            tape: DoubleTape::new(),
            best_result: DoubleTape::new(),
            result: DoubleTape::new(),
            locations: LocationVectorVector::new(),
            tape_locations: WeightedDoubleTapeVector::new(),
            captures: Vec::new(),
            best_captures: Vec::new(),
            old_captures: Vec::new(),
            possible_first_symbols: Vec::new(),
            global_flag_state: FdState::new_default(),
            verbose: false,
            count_patterns: false,
            delete_patterns: false,
            extract_patterns: false,
            locate_mode: false,
            mark_patterns: false,
            max_context_length: 0,
            max_recursion: 0,
            need_separators: false,
            xerox_composition: false,
            uncomposable: false,
            line_number: 0,
            pattern_counts: BTreeMap::new(),
            profile_mode: false,
            single_codepoint_tokenization: false,
            recursion_depth_left: 0,
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
            epsilon_visited: std::collections::BTreeSet::new(),
        }
    }

    // void set_properties(void)
    pub fn set_properties(&mut self) {
        self.count_patterns = false;
        self.delete_patterns = false;
        self.extract_patterns = false;
        self.locate_mode = false;
        self.mark_patterns = true;
        self.max_context_length = 254;
        self.max_recursion = 5000;
        self.need_separators = true;
        self.xerox_composition = true;
        self.uncomposable = false;
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-properties-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-properties-fn]
    // void set_properties(std::map<std::string, std::string> &)
    pub fn set_properties_map(&mut self, properties: &BTreeMap<String, String>) {
        for (first, second) in properties.iter() {
            if first == "count-patterns" {
                if second == "on" {
                    self.count_patterns = true;
                } else if second == "off" {
                    self.count_patterns = false;
                }
            } else if first == "delete-patterns" {
                if second == "on" {
                    self.delete_patterns = true;
                } else if second == "off" {
                    self.delete_patterns = false;
                }
            } else if first == "extract-patterns" {
                if second == "on" {
                    self.extract_patterns = true;
                } else if second == "off" {
                    self.extract_patterns = false;
                }
            } else if first == "locate-patterns" {
                if second == "on" {
                    self.locate_mode = true;
                } else if second == "off" {
                    self.locate_mode = false;
                }
            } else if first == "mark-patterns" {
                if second == "on" {
                    self.mark_patterns = true;
                } else if second == "off" {
                    self.mark_patterns = false;
                }
            } else if first == "max-context-length" {
                // std::stringstream converter(it->second); converter >> max_context_length;
                match second.trim().parse::<usize>() {
                    Ok(v) => {
                        self.max_context_length = v;
                        if self.max_context_length == 0 && second != "0" {
                            self.max_context_length = 254;
                        }
                    }
                    Err(_) => {
                        // failed extraction leaves value 0 (as a freshly default-
                        // initialized stringstream target would)
                        self.max_context_length = 0;
                        if second != "0" {
                            self.max_context_length = 254;
                        }
                    }
                }
            } else if first == "max-recursion" {
                match second.trim().parse::<usize>() {
                    Ok(v) => {
                        self.max_recursion = v;
                        if self.max_recursion == 0 && second != "0" {
                            self.max_recursion = 5000;
                        }
                    }
                    Err(_) => {
                        self.max_recursion = 0;
                        if second != "0" {
                            self.max_recursion = 5000;
                        }
                    }
                }
            } else if first == "need-separators" {
                if second == "on" {
                    self.need_separators = true;
                } else if second == "off" {
                    self.need_separators = false;
                }
            } else if first == "xerox-composition" {
                if second == "off" {
                    self.xerox_composition = false;
                } else if second == "on" {
                    self.xerox_composition = true;
                }
            }
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.collect-first-symbols-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.collect-first-symbols-fn]
    pub fn collect_first_symbols(&mut self, symbol_list: &str) {
        let first_symbols = self.symbol_vector_from_symbols(symbol_list);
        for &it in first_symbols.iter() {
            while it as usize >= self.possible_first_symbols.len() {
                self.possible_first_symbols.push(false);
            }
            self.possible_first_symbols[it as usize] = true;
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.symbol-vector-from-symbols-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.symbol-vector-from-symbols-fn]
    pub fn symbol_vector_from_symbols(&mut self, symbols: &str) -> SymbolNumberVector {
        self.initialize_input(symbols);
        if self.alphabet.get_special(SpecialSymbol::boundary) != NO_SYMBOL_NUMBER {
            // SymbolNumberVector(input.begin() + 1, input.end() - 1)
            return self.input[1..self.input.len() - 1].to_vec();
        }
        self.input.clone()
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.initialize-input-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.initialize-input-fn]
    pub fn initialize_input(&mut self, input_s: &str) {
        self.input.clear();
        // The C++ walks a 0-terminated char buffer with a pointer it advances.
        let mut buf: Vec<u8> = input_s.as_bytes().to_vec();
        buf.push(0);
        let mut p: usize = 0;
        // 'k' lives outside the loop to mirror the C++, where a stale value can
        // carry over when single-codepoint tokenization finds no bytes to take.
        let mut k: Option<SymbolNumber> = None;
        let boundary_sym = self.alphabet.get_special(SpecialSymbol::boundary);
        if boundary_sym != NO_SYMBOL_NUMBER {
            self.input.push(boundary_sym);
        }
        while buf[p] != 0 {
            let original_input_loc = p;
            if self.single_codepoint_tokenization {
                let bytes_to_tokenize = nByte_grapheme_bytes(&buf[p..buf.len() - 1]);
                if bytes_to_tokenize > 0 {
                    // memcpy the first bytes_to_tokenize bytes, NUL terminate,
                    // then find_key on that scratch buffer.
                    let mut scratch: Vec<u8> = buf[p..p + bytes_to_tokenize as usize].to_vec();
                    scratch.push(0);
                    let mut sp: usize = 0;
                    k = self
                        .encoder
                        .as_ref()
                        .expect("encoder is initialized during container load")
                        .find_key(&scratch, &mut sp);
                    if k.is_some() {
                        p += bytes_to_tokenize as usize;
                    }
                }
            } else {
                k = self
                    .encoder
                    .as_ref()
                    .expect("encoder is initialized during container load")
                    .find_key(&buf, &mut p);
            }
            let key = match k {
                Some(key) => key,
                None => {
                    // Regular tokenization failed
                    p = original_input_loc;
                    let mut bytes_to_tokenize = nByte_grapheme_bytes(&buf[p..buf.len() - 1]);
                    if bytes_to_tokenize == 0 {
                        // if utf-8 tokenization fails too, just grab a byte
                        bytes_to_tokenize = 1;
                    }
                    let new_symbol_bytes = buf[p..p + bytes_to_tokenize as usize].to_vec();
                    let new_symbol = crate::hfst_data_types::Symbol::from(
                        String::from_utf8_lossy(&new_symbol_bytes).into_owned(),
                    );
                    p += bytes_to_tokenize as usize;
                    self.alphabet.add_symbol(&new_symbol);
                    self.encoder
                        .as_mut()
                        .expect("encoder is initialized during container load")
                        .read_input_symbol(&new_symbol, self.symbol_count as i32);
                    let key = self.symbol_count;
                    k = Some(key);
                    self.symbol_count += 1;
                    key
                }
            };
            self.input.push(key);
        }
        if boundary_sym != NO_SYMBOL_NUMBER {
            self.input.push(boundary_sym);
        }
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

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.add-rtn-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.add-rtn-fn]
    pub fn add_rtn(
        &mut self,
        rtn: &crate::transducer::Transducer,
        name: &str,
    ) -> crate::error::Result<()> {
        let transitions = rtn.copy_transitionw_table()?;
        let indices = rtn.copy_windex_table()?;
        let pmatch_rtn = Box::new(PmatchTransducer::new_from_vectors(
            transitions.get_vector().clone(),
            indices.get_vector().clone(),
            &self.alphabet,
            name.to_string(),
        ));
        if !self.alphabet.has_rtn(name) {
            self.alphabet.add_rtn(pmatch_rtn, name);
        } else {
            // C++ does 'delete rtn;' here (note: deletes the *argument*, not the
            // freshly-built pmatch_rtn — a faithful bug). We own neither; the
            // argument is borrowed and pmatch_rtn is simply dropped.
            drop(pmatch_rtn);
        }
        Ok(())
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.process-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.process-fn]
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
        let mut nonmatching_locations = DoubleTape::new();
        while self.has_queued_input(input_pos) {
            self.best_result.inner.clear();
            let current_input = self.input[input_pos as usize];
            if self.not_possible_first_symbol(current_input) {
                self.copy_to_result_syms(current_input, current_input);
                input_pos += 1;
                if self.locate_mode && self.alphabet.is_printable_sym(current_input) {
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
            // Clone (sharing tables via Rc) instead of taking the box out, so the
            // slot stays populated: an RTN that returns to the toplevel rebuilds
            // it from this slot while the outer walk is still suspended here.
            // [hfst/hfst#354]
            let mut top = self
                .toplevel
                .as_ref()
                .expect("toplevel present for match")
                .clone();
            top.do_match(input_pos, tape_pos, self);
            if self.candidate_found() {
                // We got some output
                if self.locate_mode {
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
                    // [DIVERGENCE hfst/hfst#335] The optimized-lookup walk can
                    // reach one accepting configuration via several structurally
                    // distinct paths (e.g. a union branch carrying an extra
                    // EndTag or an internal epsilon/flag arc). 'locatefy' projects
                    // away every non-printable, non-endtag symbol, so those paths
                    // collapse to byte-identical Locations and the same match is
                    // reported more than once. Upstream shipped this as an opt-in
                    // '-u'/unique flag; as a successor we dedupe by default,
                    // dropping only exact duplicates (same span, output, tag and
                    // weight) while preserving the (already sorted) order.
                    dedup_locations(&mut ls);
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
                if self.locate_mode && self.alphabet.is_printable_sym(current_input) {
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
        if self.locate_mode && !nonmatching_locations.inner.is_empty() {
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
        self.locate_mode = false;
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
        self.locate_mode = true;
        self.process(input);
        self.locations.clone()
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
        for i in 0..self.alphabet.counters.len() {
            if self.alphabet.counters[i] != NO_COUNTER {
                let counter_name = self.alphabet.get_counter_name(i as SymbolNumber);
                if counter_name.len() > max_name_len {
                    max_name_len = counter_name.len();
                }
                counter_name_val_pairs.push((counter_name, self.alphabet.counters[i]));
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

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.not-possible-first-symbol-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.not-possible-first-symbol-fn]
    pub fn not_possible_first_symbol(&self, sym: SymbolNumber) -> bool {
        if self.possible_first_symbols.is_empty() {
            return false;
        }
        (sym as usize) >= self.possible_first_symbols.len()
            || !self.possible_first_symbols[sym as usize]
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
        self.locate_mode = b;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-extract-patterns-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-extract-patterns-fn]
    pub fn set_extract_patterns(&mut self, b: bool) {
        self.extract_patterns = b;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-single-codepoint-tokenization-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-single-codepoint-tokenization-fn]
    pub fn set_single_codepoint_tokenization(&mut self, b: bool) {
        self.single_codepoint_tokenization = b;
    }

    // [DIVERGENCE hfst/hfst#367] Does the transducer's alphabet carry any real
    // *text* symbol whose surface spans more than one grapheme cluster (e.g. a
    // multichar symbol like "dz")? Such symbols only match under longest-match
    // ('multichar') tokenization; single-codepoint tokenization segments one
    // grapheme at a time and never sees them, so `tokenise` would drop words
    // that `lookup` accepts. Composed characters — precomposed OR decomposed,
    // both a single grapheme cluster — do NOT count and stay fine under either
    // mode. Epsilon / flag / `@...@` special symbols are not input text.
    // The tokeniser uses this to pick the default mode when the user did not
    // force `--tokenize-multichar`.
    pub fn has_multichar_input_symbols(&self) -> bool {
        let symbol_table = self.alphabet.get_symbol_table();
        for (i, s) in symbol_table.iter().enumerate() {
            if s.is_empty() || crate::hfst_symbol_defs::is_epsilon(s) {
                continue;
            }
            // @...@ specials (flags, insertions, ENDTAG, IDENTITY/UNKNOWN, ...)
            // are never plain input text.
            if s.starts_with('@') && s.ends_with('@') {
                continue;
            }
            if self.alphabet.is_flag_diacritic(i as SymbolNumber) {
                continue;
            }
            let first = nByte_grapheme(s);
            if first > 0 && (first as usize) < s.len() {
                return true;
            }
        }
        false
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-count-patterns-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-count-patterns-fn]
    pub fn set_count_patterns(&mut self, b: bool) {
        self.count_patterns = b;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-delete-patterns-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-delete-patterns-fn]
    pub fn set_delete_patterns(&mut self, b: bool) {
        self.delete_patterns = b;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-mark-patterns-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-mark-patterns-fn]
    pub fn set_mark_patterns(&mut self, b: bool) {
        self.mark_patterns = b;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-max-recursion-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-max-recursion-fn]
    pub fn set_max_recursion(&mut self, max: usize) {
        self.max_recursion = max;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.set-max-context-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.set-max-context-fn]
    pub fn set_max_context(&mut self, max: usize) {
        self.max_context_length = max;
    }
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.is-in-locate-mode-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.is-in-locate-mode-fn]
    pub fn is_in_locate_mode(&self) -> bool {
        self.locate_mode
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
    // rebuild the suspended caller when an RTN returns to it. [hfst/hfst#354]
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
        if self.locate_mode {
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
        self.recursion_depth_left = self.max_recursion as u32;
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-container.uncompose-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-container.uncompose-fn]
    pub fn uncompose(&mut self, loc: &mut Location) {
        let verbose = self.verbose;
        if !self.uncomposable {
            if verbose {
                debug!("uncompose disabled");
            }
            return;
        }
        if verbose {
            debug!("uncomposing left {}", loc.input);
        }
        let middle_left = self
            .uncompose_left
            .as_mut()
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
                .uncompose_right
                .as_mut()
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

// ==================== PmatchTransducer (impl from workflow body agent) ====================
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
impl PmatchTransducer {
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.pmatch-transducer-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.pmatch-transducer-fn]
    // ctor from istream
    pub fn new_from_stream(
        is: &mut crate::transducer::IStream<'_>,
        index_table_size: TransitionTableIndex,
        transition_table_size: TransitionTableIndex,
        alphabet: &PmatchAlphabet,
        name: String,
    ) -> PmatchTransducer {
        use crate::transducer::TableEntry;
        let orig_symbol_count = u32::try_from(alphabet.get_symbol_table().len())
            .expect("value out of u32 range") as SymbolNumber;
        // initialize the stack for local variables
        let mut local_variables = LocalVariables {
            flag_state: FdState::new(alphabet.get_fd_table()),
            tape_step: 1,
            max_context_length_remaining: 254,
            context: ContextChecking::none,
            context_placeholder: 0,
            default_symbol_trap: false,
            negative_context_success: false,
            pending_passthrough: false,
        };
        // (kept to mirror C++ field assignment order; all already set above)
        local_variables.tape_step = 1;
        let local_stack: Vec<LocalVariables> = vec![local_variables];

        // Allocate and read tables
        let mut indextab = vec![0u8; TransitionWIndex::SIZE * index_table_size as usize];
        let mut transitiontab = vec![0u8; TransitionW::SIZE * transition_table_size as usize];
        is.read(&mut indextab);
        is.read(&mut transitiontab);
        let mut index_table: Vec<TransitionWIndex> = Vec::with_capacity(index_table_size as usize);
        let mut p = 0usize;
        let mut remaining = index_table_size;
        while remaining != 0 {
            index_table.push(TransitionWIndex::from_bytes(
                &indextab[p..p + TransitionWIndex::SIZE],
            ));
            remaining -= 1;
            p += TransitionWIndex::SIZE;
        }
        let mut transition_table: Vec<TransitionW> =
            Vec::with_capacity(transition_table_size as usize);
        p = 0;
        let mut remaining = transition_table_size;
        while remaining != 0 {
            transition_table.push(TransitionW::from_bytes(
                &transitiontab[p..p + TransitionW::SIZE],
            ));
            remaining -= 1;
            p += TransitionW::SIZE;
        }

        PmatchTransducer {
            name,
            local_stack,
            transition_table: Rc::new(transition_table),
            index_table: Rc::new(index_table),
            orig_symbol_count,
        }
    }

    // ctor from vectors
    pub fn new_from_vectors(
        transition_vector: Vec<TransitionW>,
        index_vector: Vec<TransitionWIndex>,
        alphabet: &PmatchAlphabet,
        name: String,
    ) -> PmatchTransducer {
        let orig_symbol_count = u32::try_from(alphabet.get_symbol_table().len())
            .expect("value out of u32 range") as SymbolNumber;
        // initialize the stack for local variables
        let local_variables = LocalVariables {
            flag_state: FdState::new(alphabet.get_fd_table()),
            tape_step: 1,
            max_context_length_remaining: 254,
            context: ContextChecking::none,
            context_placeholder: 0,
            default_symbol_trap: false,
            negative_context_success: false,
            pending_passthrough: false,
        };
        let local_stack: Vec<LocalVariables> = vec![local_variables];

        PmatchTransducer {
            name,
            local_stack,
            transition_table: Rc::new(transition_vector),
            index_table: Rc::new(index_vector),
            orig_symbol_count,
        }
    }

    // Clone this transducer for a fresh (re-)entrant walk, replacing its stack
    // with a single 'frame'. Used when an RTN returns to a caller that is
    // suspended higher on the Rust stack (so the live object cannot be borrowed
    // again): the tables are shared via 'Rc', and the caller resumes with the
    // frame it held when it made the RTN call. [hfst/hfst#354]
    fn clone_with_frame(&self, frame: LocalVariables) -> PmatchTransducer {
        PmatchTransducer {
            name: self.name.clone(),
            local_stack: vec![frame],
            transition_table: Rc::clone(&self.transition_table),
            index_table: Rc::clone(&self.index_table),
            orig_symbol_count: self.orig_symbol_count,
        }
    }

    // Helper: the owning symbol of this running transducer (the C++ 'this'
    // pointer is identified by an owning symbol in our ownership scheme).
    // The toplevel (name "TOP") has no owning symbol -> NO_SYMBOL_NUMBER.
    fn self_symbol(&self, container: &PmatchContainer) -> SymbolNumber {
        if self.name == "TOP" {
            NO_SYMBOL_NUMBER
        } else {
            match container.alphabet.rtn_names.get(&self.name) {
                Some(s) => *s,
                None => NO_SYMBOL_NUMBER,
            }
        }
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
    // The visited set is PATH-scoped (insert on descent, remove on backtrack),
    // not a global per-attempt memo. A global memo also prunes CONVERGENT paths —
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
        container: &mut PmatchContainer,
    ) -> Option<EpsilonVisitKey> {
        let frame = self
            .local_stack
            .last()
            .expect("local_stack is non-empty during a match walk");
        let key: EpsilonVisitKey = (
            self.self_symbol(container),
            target,
            input_pos,
            frame.context as u8,
            frame.tape_step,
        );
        if container.epsilon_visited.insert(key) {
            Some(key)
        } else {
            None
        }
    }

    // Leave a plain-epsilon configuration entered via 'epsilon_enter', so a
    // sibling branch that later reaches the same state is not mistaken for a
    // cycle. [hfst#399, #354]
    fn epsilon_leave(container: &mut PmatchContainer, key: EpsilonVisitKey) {
        container.epsilon_visited.remove(&key);
    }

    // Helper: run 'f' with the callee RTN (the container's toplevel for
    // NO_SYMBOL_NUMBER, or alphabet.rtns[sym]) and 'container'. The callee is
    // *cloned* from its slot rather than taken out: cloning shares the tables via
    // 'Rc' (cheap) and leaves the original in place, so a re-entrant call — an
    // RTN that recurses into itself, or returns to a caller that is itself an
    // active ancestor — always finds a slot to clone from. The callee's frame is
    // scratch (it pushes its own on entry and the result is recorded on the
    // container), so the clone is simply dropped when 'f' returns. [hfst/hfst#354]
    fn with_rtn<F: FnOnce(&mut PmatchTransducer, &mut PmatchContainer)>(
        container: &mut PmatchContainer,
        sym: SymbolNumber,
        f: F,
    ) {
        if sym == NO_SYMBOL_NUMBER {
            let mut callee = container
                .toplevel
                .as_ref()
                .expect("toplevel present for RTN call")
                .clone();
            f(&mut callee, container);
        } else {
            let mut callee = container.alphabet.rtns[sym as usize]
                .as_ref()
                .expect("RTN slot occupied for RTN call")
                .clone();
            f(&mut callee, container);
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.is-final-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.is-final-fn]
    pub fn is_final(&self, i: TransitionTableIndex) -> bool {
        if Self::indexes_transition_table(i) {
            self.transition_table[(i - TRANSITION_TARGET_TABLE_START) as usize].is_final()
        } else {
            self.index_table[i as usize].is_final()
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.get-weight-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.get-weight-fn]
    pub fn get_weight(&self, i: TransitionTableIndex) -> Weight {
        if Self::indexes_transition_table(i) {
            self.transition_table[(i - TRANSITION_TARGET_TABLE_START) as usize].get_weight()
        } else {
            self.index_table[i as usize].final_weight()
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.make-transition-table-index-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.make-transition-table-index-fn]
    pub fn make_transition_table_index(
        &self,
        i: TransitionTableIndex,
        input: SymbolNumber,
    ) -> TransitionTableIndex {
        if Self::indexes_transition_table(i) {
            i - TRANSITION_TARGET_TABLE_START
        } else if self.index_table[(i + input as u32) as usize].get_input_symbol() == input {
            self.index_table[(i + input as u32) as usize].get_target()
                - TRANSITION_TARGET_TABLE_START
        } else {
            TRANSITION_TARGET_TABLE_START
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.final-index-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.final-index-fn]
    pub fn final_index(&self, i: TransitionTableIndex) -> bool {
        if Self::indexes_transition_table(i) {
            self.transition_table[i as usize].is_final()
        } else {
            self.index_table[i as usize].is_final()
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.indexes-transition-table-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.indexes-transition-table-fn]
    pub fn indexes_transition_table(i: TransitionTableIndex) -> bool {
        i >= TRANSITION_TARGET_TABLE_START
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.is-good-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.is-good-fn]
    pub fn is_good(i: TransitionTableIndex) -> bool {
        i < TRANSITION_TARGET_TABLE_START
    }

    // ---- the mutually recursive lookup-handling functions ----

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.take-epsilons-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.take-epsilons-fn]
    pub fn take_epsilons(
        &mut self,
        input_pos: u32,
        tape_pos: u32,
        i: TransitionTableIndex,
        container: &mut PmatchContainer,
    ) {
        let mut i = self.make_transition_table_index(i, 0);
        while Self::is_good(i) {
            let input = self.transition_table[i as usize].get_input_symbol();
            if input != 0
                && !container.alphabet.is_flag_diacritic(input)
                && !container.alphabet.has_rtn_sym(input)
            {
                return;
            }

            let output = self.transition_table[i as usize].get_output_symbol();
            let target = self.transition_table[i as usize].get_target();
            let old_weight = container.get_weight();
            container.increment_weight(self.transition_table[i as usize].get_weight());

            if self.checking_context() {
                if self.try_exiting_context(output, container) {
                    // We've successfully completed a context check
                    let cp = self
                        .local_stack
                        .last()
                        .expect("local_stack is non-empty during a match walk")
                        .context_placeholder;
                    self.get_analyses(cp, tape_pos, target, container);
                    self.local_stack.pop();
                } else if self
                    .local_stack
                    .last()
                    .expect("local_stack is non-empty during a match walk")
                    .negative_context_success
                {
                    // We've succeeded in a negative context, just back out
                    return;
                } else if container.alphabet.is_flag_diacritic(input) {
                    self.take_flag(input, input_pos, tape_pos, i, container);
                } else if container.alphabet.has_rtn_sym(input) {
                    let caller = self.self_symbol(container);
                    let locals = self
                        .local_stack
                        .last()
                        .expect("local_stack is non-empty during a match walk")
                        .clone();
                    Self::with_rtn(container, input, |rtn, cont| {
                        rtn.rtn_call_in_context(input_pos, tape_pos, caller, target, locals, cont);
                    });
                } else {
                    // Don't alter tapes when checking context
                    // [DIVERGENCE hfst/hfst#399, #354] prune re-entered epsilon
                    // cycles (path-scoped), but keep convergent analyses.
                    if let Some(k) = self.epsilon_enter(input_pos, target, container) {
                        self.get_analyses(input_pos, tape_pos, target, container);
                        Self::epsilon_leave(container, k);
                    }
                }
            } else if input == 0 {
                if container.profile_mode {
                    container.alphabet.count(output);
                }
                if !self.try_entering_context(output, container) {
                    // no context to enter, regular input epsilon
                    container.tape.write_pair(tape_pos, 0, output);

                    // [DIVERGENCE hfst/hfst#399] An entry/exit/capture arc
                    // mutates entry_stack/captures — state the memo key does
                    // not capture — so it is NOT a "plain" epsilon and is never
                    // pruned; only truly plain epsilon-input arcs are memoized.
                    let plain_epsilon = output
                        != container.alphabet.get_special(SpecialSymbol::entry)
                        && output != container.alphabet.get_special(SpecialSymbol::exit)
                        && !container.alphabet.is_capture_tag_sym(output)
                        && !container.alphabet.is_captured_tag_sym(output);

                    let mut orig_entry_stack_back: u32 = 0;
                    // if it's an entry or exit arc, adjust entry stack
                    if output == container.alphabet.get_special(SpecialSymbol::entry) {
                        container.entry_stack.push(input_pos);
                    } else if output == container.alphabet.get_special(SpecialSymbol::exit) {
                        orig_entry_stack_back = *container
                            .entry_stack
                            .last()
                            .expect("exit arc has a matching entry on the stack");
                        container.entry_stack.pop();
                    } else if container.alphabet.is_capture_tag_sym(output) {
                        // if it's a capture tag, remember where we were
                        let capture = Capture {
                            begin: *container
                                .entry_stack
                                .last()
                                .expect("capture tag has a matching entry on the stack"),
                            end: input_pos,
                            name: output,
                        };
                        container.captures.push(capture);
                    } else if container.alphabet.is_captured_tag_sym(output) {
                        // if it's a captured tag, try each previously
                        // captured sequence
                        let key = container.alphabet.captured2capture[output as usize];
                        let cap = container.get_longest_matching_capture(key, input_pos);

                        if cap.1 - cap.0 != 0 {
                            let slice: Vec<SymbolNumber> = container.input[cap.0..cap.1].to_vec();
                            container.tape.write_slice(tape_pos, &slice);
                            let span = (cap.1 - cap.0) as u32;
                            self.get_analyses(input_pos + span, tape_pos + span, target, container);
                        }
                        i += 1;
                        container.set_weight(old_weight);
                        continue;
                    }

                    if !plain_epsilon {
                        self.get_analyses(input_pos, tape_pos + 1, target, container);
                    } else if let Some(k) = self.epsilon_enter(input_pos, target, container) {
                        self.get_analyses(input_pos, tape_pos + 1, target, container);
                        Self::epsilon_leave(container, k);
                    }

                    if output == container.alphabet.get_special(SpecialSymbol::entry) {
                        container.entry_stack.pop();
                    } else if output == container.alphabet.get_special(SpecialSymbol::exit) {
                        container.entry_stack.push(orig_entry_stack_back);
                    } else if container.alphabet.is_capture_tag_sym(output) {
                        container.captures.pop();
                    }
                } else {
                    self.check_context(input_pos, tape_pos, i, container);
                }
            } else if container.alphabet.is_flag_diacritic(input) {
                self.take_flag(input, input_pos, tape_pos, i, container);
            } else if container.alphabet.has_rtn_sym(input) {
                let caller = self.self_symbol(container);
                let caller_frame = self
                    .local_stack
                    .last()
                    .expect("local_stack is non-empty during a match walk")
                    .clone();
                Self::with_rtn(container, input, |rtn, cont| {
                    rtn.rtn_call(input_pos, tape_pos, caller, target, caller_frame, cont);
                });
            }
            i += 1;
            container.set_weight(old_weight);
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.check-context-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.check-context-fn]
    pub fn check_context(
        &mut self,
        input_pos: u32,
        tape_pos: u32,
        i: TransitionTableIndex,
        container: &mut PmatchContainer,
    ) {
        // The context placeholder remembers the position in the input before
        // a context check. If the context check is successful, the placeholder
        // will be used as the input position going forwards.
        self.local_stack
            .last_mut()
            .expect("local_stack is non-empty during a match walk")
            .context_placeholder = input_pos;
        let mut input_pos = input_pos;
        let ctx = self
            .local_stack
            .last()
            .expect("local_stack is non-empty during a match walk")
            .context;
        if ctx == ContextChecking::LC || ctx == ContextChecking::NLC {
            // Jump to the left-hand side of the input
            input_pos = container
                .entry_stack
                .last()
                .expect("entry_stack populated during a left-context check")
                .wrapping_sub(1);
        }
        let target = self.transition_table[i as usize].get_target();
        self.get_analyses(input_pos, tape_pos, target, container);

        // In case we have a negative context, we check to see if the context
        // matched. If it didn't, we schedule a passthrough arc after we've
        // processed epsilons.
        let mut schedule_passthrough = false;
        let ctx = self
            .local_stack
            .last()
            .expect("local_stack is non-empty during a match walk")
            .context;
        if (ctx == ContextChecking::NLC || ctx == ContextChecking::NRC)
            && !self
                .local_stack
                .last()
                .expect("local_stack is non-empty during a match walk")
                .negative_context_success
        {
            schedule_passthrough = true;
        }
        // Pop the local stack that got pushed by entering the context
        self.local_stack.pop();
        if schedule_passthrough {
            self.local_stack
                .last_mut()
                .expect("local_stack is non-empty during a match walk")
                .pending_passthrough = true;
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.take-flag-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.take-flag-fn]
    pub fn take_flag(
        &mut self,
        input: SymbolNumber,
        input_pos: u32,
        tape_pos: u32,
        i: TransitionTableIndex,
        container: &mut PmatchContainer,
    ) {
        let mut old_global_values: Vec<i16> = Vec::new();
        if container.alphabet.is_global_flag_sym(input) {
            old_global_values = container.global_flag_state.get_values().clone();
            let op = container
                .alphabet
                .get_operation(input)
                .expect("flag diacritic has an operation")
                .clone();
            if !container.global_flag_state.apply_operation(&op) {
                return;
            }
        }
        let old_values = self
            .local_stack
            .last()
            .expect("local_stack is non-empty during a match walk")
            .flag_state
            .get_values()
            .clone();
        let op = container
            .alphabet
            .get_operation(input)
            .expect("flag diacritic has an operation")
            .clone();
        if self
            .local_stack
            .last_mut()
            .expect("local_stack is non-empty during a match walk")
            .flag_state
            .apply_operation(&op)
        {
            // flag diacritic allowed
            // generally we shouldn't care to write flags
            //                container->tape.write(tape_pos, input, output);
            let target = self.transition_table[i as usize].get_target();
            self.get_analyses(input_pos, tape_pos, target, container);
        }
        if container.alphabet.is_global_flag_sym(input) {
            container
                .global_flag_state
                .assign_values(&old_global_values);
        }
        self.local_stack
            .last_mut()
            .expect("local_stack is non-empty during a match walk")
            .flag_state
            .assign_values(&old_values);
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.take-transitions-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.take-transitions-fn]
    pub fn take_transitions(
        &mut self,
        input: SymbolNumber,
        input_pos: u32,
        tape_pos: u32,
        i: TransitionTableIndex,
        container: &mut PmatchContainer,
    ) {
        let mut i = self.make_transition_table_index(i, input);

        while Self::is_good(i) {
            let mut this_input = self.transition_table[i as usize].get_input_symbol();
            let mut this_output = self.transition_table[i as usize].get_output_symbol();
            let target = self.transition_table[i as usize].get_target();
            if this_input == NO_SYMBOL_NUMBER {
                return;
            } else if this_input == input {
                let old_weight = container.get_weight();
                container.increment_weight(self.transition_table[i as usize].get_weight());
                if !self.checking_context() {
                    if container.alphabet.is_meta_arc(this_output)
                        || (container.alphabet.list2symbols[this_output as usize]
                            != NO_SYMBOL_NUMBER)
                    {
                        // we got here via a meta-arc, so look back in the
                        // input tape to find the symbol we want to write
                        this_output = container.input[input_pos as usize];
                        this_input = container.input[input_pos as usize];
                    }
                    if this_input
                        == container
                            .alphabet
                            .get_special(SpecialSymbol::Pmatch_passthrough)
                    {
                        self.get_analyses(input_pos, tape_pos, target, container); // awkward
                    } else {
                        container.tape.write_pair(tape_pos, this_input, this_output);
                        self.get_analyses(input_pos + 1, tape_pos + 1, target, container);
                    }
                } else {
                    // Checking context so don't touch output
                    if self
                        .local_stack
                        .last()
                        .expect("local_stack is non-empty during a match walk")
                        .max_context_length_remaining
                        > 0
                    {
                        if (self
                            .local_stack
                            .last()
                            .expect("local_stack is non-empty during a match walk")
                            .tape_step
                            < 0)
                            && (input_pos == 0)
                        {
                            // (C++ marks FIXME here) prevents segfault but
                            self.get_analyses(input_pos, tape_pos, target, container); // awkward
                        } else {
                            self.local_stack
                                .last_mut()
                                .expect("local_stack is non-empty during a match walk")
                                .max_context_length_remaining -= 1;
                            let step = self
                                .local_stack
                                .last()
                                .expect("local_stack is non-empty during a match walk")
                                .tape_step;
                            let new_input_pos = (input_pos as i64 + step as i64) as u32;
                            self.get_analyses(new_input_pos, tape_pos, target, container);
                            self.local_stack
                                .last_mut()
                                .expect("local_stack is non-empty during a match walk")
                                .max_context_length_remaining += 1;
                        }
                    }
                }
                self.local_stack
                    .last_mut()
                    .expect("local_stack is non-empty during a match walk")
                    .default_symbol_trap = false;
                container.set_weight(old_weight);
            } else {
                return;
            }
            i += 1;
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.get-analyses-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.get-analyses-fn]
    pub fn get_analyses(
        &mut self,
        input_pos: u32,
        tape_pos: u32,
        index: TransitionTableIndex,
        container: &mut PmatchContainer,
    ) {
        let i = index;
        if container.get_weight() > container.max_weight {
            return;
        }
        if container.max_time > 0.0 {
            container.call_counter += 1;
            // Have we spent too much time?
            if container.limit_reached
                || (container.call_counter.is_multiple_of(1000000)
                    && (container.candidate_found()
                        // if we have at least something, stop doing more work
                        && container
                            .start_clock
                            .expect("start_clock set when max_time is enabled")
                            .elapsed()
                            .as_secs_f64()
                            > container.max_time))
            {
                container.limit_reached = true;
                return;
            }
        }
        if !container.try_recurse() {
            if container.verbose {
                warn!("out of stack space, truncating result");
            }
            return;
        }
        self.local_stack
            .last_mut()
            .expect("local_stack is non-empty during a match walk")
            .default_symbol_trap = true;
        self.take_epsilons(input_pos, tape_pos, i + 1, container);
        if self
            .local_stack
            .last()
            .expect("local_stack is non-empty during a match walk")
            .pending_passthrough
        {
            self.local_stack
                .last_mut()
                .expect("local_stack is non-empty during a match walk")
                .pending_passthrough = false;
            // A negative context failed (successfully)
            let passthrough = container
                .alphabet
                .get_special(SpecialSymbol::Pmatch_passthrough);
            self.take_transitions(passthrough, input_pos, tape_pos, i + 1, container);
        }
        // Check for finality even if the input string hasn't ended
        if self.is_final(i) {
            let old_weight = container.get_weight();
            container.increment_weight(self.get_weight(i));
            self.handle_final_state(input_pos, tape_pos, container);
            container.set_weight(old_weight);
        }

        if !container.has_queued_input(input_pos) {
            container.unrecurse();
            return;
        }
        let input = container.input[input_pos as usize];

        if container.alphabet.symbol2lists[input as usize] != NO_SYMBOL_NUMBER {
            // At least one symbol list could allow this symbol
            let list_idx = container.alphabet.symbol2lists[input as usize];
            let list = container.alphabet.symbol_lists[list_idx as usize].clone();
            for it in list.iter() {
                self.take_transitions(*it, input_pos, tape_pos, i + 1, container);
            }
        }
        if container.alphabet.get_special(SpecialSymbol::UnicodeAlpha) != NO_SYMBOL_NUMBER
            && container.alphabet.is_unicode_alpha(input)
        {
            let s = container.alphabet.get_special(SpecialSymbol::UnicodeAlpha);
            self.take_transitions(s, input_pos, tape_pos, i + 1, container);
        }
        if container
            .alphabet
            .get_special(SpecialSymbol::UnicodeUpperAlpha)
            != NO_SYMBOL_NUMBER
            && container.alphabet.is_unicode_upperalpha(input)
        {
            let s = container
                .alphabet
                .get_special(SpecialSymbol::UnicodeUpperAlpha);
            self.take_transitions(s, input_pos, tape_pos, i + 1, container);
        }
        if container
            .alphabet
            .get_special(SpecialSymbol::UnicodeLowerAlpha)
            != NO_SYMBOL_NUMBER
            && container.alphabet.is_unicode_loweralpha(input)
        {
            let s = container
                .alphabet
                .get_special(SpecialSymbol::UnicodeLowerAlpha);
            self.take_transitions(s, input_pos, tape_pos, i + 1, container);
        }
        if container
            .alphabet
            .get_special(SpecialSymbol::UnicodeWhitespace)
            != NO_SYMBOL_NUMBER
            && container.alphabet.is_unicode_whitespace(input)
        {
            let s = container
                .alphabet
                .get_special(SpecialSymbol::UnicodeWhitespace);
            self.take_transitions(s, input_pos, tape_pos, i + 1, container);
        }

        // The "normal" case where we have a regular input symbol
        if input < self.orig_symbol_count {
            self.take_transitions(input, input_pos, tape_pos, i + 1, container);
        } else {
            if container.alphabet.get_identity_symbol() != NO_SYMBOL_NUMBER {
                let s = container.alphabet.get_identity_symbol();
                self.take_transitions(s, input_pos, tape_pos, i + 1, container);
            }
            if container.alphabet.get_unknown_symbol() != NO_SYMBOL_NUMBER {
                let s = container.alphabet.get_unknown_symbol();
                self.take_transitions(s, input_pos, tape_pos, i + 1, container);
            }
        }
        if container.alphabet.get_default_symbol() != NO_SYMBOL_NUMBER
            && self
                .local_stack
                .last()
                .expect("local_stack is non-empty during a match walk")
                .default_symbol_trap
        {
            let s = container.alphabet.get_default_symbol();
            self.take_transitions(s, input_pos, tape_pos, i + 1, container);
        }
        container.unrecurse();
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.checking-context-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.checking-context-fn]
    pub fn checking_context(&self) -> bool {
        self.local_stack
            .last()
            .expect("local_stack is non-empty during a match walk")
            .context
            != ContextChecking::none
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.try-entering-context-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.try-entering-context-fn]
    pub fn try_entering_context(
        &mut self,
        symbol: SymbolNumber,
        container: &PmatchContainer,
    ) -> bool {
        let mut new_top: LocalVariables;
        if symbol == container.alphabet.get_special(SpecialSymbol::LC_entry) {
            new_top = self
                .local_stack
                .last()
                .expect("local_stack is non-empty during a match walk")
                .clone();
            new_top.context = ContextChecking::LC;
            new_top.tape_step = -1;
        } else if symbol == container.alphabet.get_special(SpecialSymbol::RC_entry) {
            new_top = self
                .local_stack
                .last()
                .expect("local_stack is non-empty during a match walk")
                .clone();
            new_top.context = ContextChecking::RC;
            new_top.tape_step = 1;
        } else if symbol == container.alphabet.get_special(SpecialSymbol::NLC_entry) {
            new_top = self
                .local_stack
                .last()
                .expect("local_stack is non-empty during a match walk")
                .clone();
            new_top.context = ContextChecking::NLC;
            new_top.tape_step = -1;
        } else if symbol == container.alphabet.get_special(SpecialSymbol::NRC_entry) {
            new_top = self
                .local_stack
                .last()
                .expect("local_stack is non-empty during a match walk")
                .clone();
            new_top.context = ContextChecking::NRC;
            new_top.tape_step = 1;
        } else {
            return false;
        }
        new_top.max_context_length_remaining = container.max_context_length;
        self.local_stack.push(new_top);
        true
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.try-exiting-context-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.try-exiting-context-fn]
    pub fn try_exiting_context(
        &mut self,
        symbol: SymbolNumber,
        container: &PmatchContainer,
    ) -> bool {
        match self
            .local_stack
            .last()
            .expect("local_stack is non-empty during a match walk")
            .context
        {
            ContextChecking::LC
                if symbol == container.alphabet.get_special(SpecialSymbol::LC_exit) =>
            {
                self.exit_context();
                true
            }
            ContextChecking::RC
                if symbol == container.alphabet.get_special(SpecialSymbol::RC_exit) =>
            {
                self.exit_context();
                true
            }
            // NOTE: faithful to C++: the NRC case has no 'else'/'break', so on a
            // non-matching symbol it falls through to the NLC case (and then to
            // default). We reproduce that fallthrough explicitly.
            ContextChecking::NRC => {
                if symbol == container.alphabet.get_special(SpecialSymbol::NRC_exit) {
                    self.local_stack
                        .last_mut()
                        .expect("local_stack is non-empty during a match walk")
                        .negative_context_success = true;
                    return false;
                }
                if symbol == container.alphabet.get_special(SpecialSymbol::NLC_exit) {
                    self.local_stack
                        .last_mut()
                        .expect("local_stack is non-empty during a match walk")
                        .negative_context_success = true;
                    return false;
                }
                false
            }
            ContextChecking::NLC => {
                if symbol == container.alphabet.get_special(SpecialSymbol::NLC_exit) {
                    self.local_stack
                        .last_mut()
                        .expect("local_stack is non-empty during a match walk")
                        .negative_context_success = true;
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
    pub fn exit_context(&mut self) {
        let mut new_top = self
            .local_stack
            .last()
            .expect("local_stack is non-empty during a match walk")
            .clone();
        new_top.context = ContextChecking::none;
        new_top.negative_context_success = false;
        new_top.tape_step = 1;
        self.local_stack.push(new_top);
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.match-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.match-fn]
    pub fn do_match(&mut self, input_pos: u32, tape_pos: u32, container: &mut PmatchContainer) {
        {
            let top = self
                .local_stack
                .last_mut()
                .expect("local_stack is non-empty during a match walk");
            top.context = ContextChecking::none;
            top.tape_step = 1;
            top.context_placeholder = 0;
            top.default_symbol_trap = false;
        }
        // [DIVERGENCE hfst/hfst#399] Fresh epsilon-cycle memo per top-level
        // match attempt (see PmatchContainer::epsilon_visited).
        container.epsilon_visited.clear();
        self.get_analyses(input_pos, tape_pos, 0, container);
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.rtn-call-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.rtn-call-fn]
    pub fn rtn_call(
        &mut self,
        input_pos: u32,
        tape_pos: u32,
        caller: SymbolNumber,
        caller_index: TransitionTableIndex,
        caller_frame: LocalVariables,
        container: &mut PmatchContainer,
    ) {
        container.push_rtn_call(caller_index, caller, caller_frame);
        container.increase_stack_depth();
        let mut new_top = self
            .local_stack
            .last()
            .expect("local_stack is non-empty during a match walk")
            .clone();
        new_top.flag_state = FdState::new(container.alphabet.get_fd_table());
        new_top.tape_step = 1;
        new_top.context = ContextChecking::none;
        new_top.context_placeholder = 0;
        new_top.default_symbol_trap = false;
        self.local_stack.push(new_top);
        self.get_analyses(input_pos, tape_pos, 0, container);
        self.local_stack.pop();
        container
            .decrease_stack_depth()
            .expect("pmatch stack-depth invariant: decrease is balanced with a prior increase");
        container.rtn_stack_pop();
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.rtn-call-in-context-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.rtn-call-in-context-fn]
    pub fn rtn_call_in_context(
        &mut self,
        input_pos: u32,
        tape_pos: u32,
        caller: SymbolNumber,
        caller_index: TransitionTableIndex,
        locals: LocalVariables,
        container: &mut PmatchContainer,
    ) {
        // 'locals' is the caller's frame at the call site; stash a copy for the
        // eventual return before it is repurposed as the callee's own frame.
        container.push_rtn_call(caller_index, caller, locals.clone());
        container.increase_stack_depth();
        let mut new_top = locals;
        new_top.flag_state = FdState::new(container.alphabet.get_fd_table());
        self.local_stack.push(new_top);
        self.get_analyses(input_pos, tape_pos, 0, container);
        self.local_stack.pop();
        container
            .decrease_stack_depth()
            .expect("pmatch stack-depth invariant: decrease is balanced with a prior increase");
        container.rtn_stack_pop();
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.rtn-return-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.rtn-return-fn]
    pub fn rtn_return(&mut self, input_pos: u32, tape_pos: u32, container: &mut PmatchContainer) {
        container
            .decrease_stack_depth()
            .expect("pmatch stack-depth invariant: decrease is balanced with a prior increase");
        let entry_index = container.rtn_stack_top().caller_index;
        self.get_analyses(input_pos, tape_pos, entry_index, container);
        container.increase_stack_depth();
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.handle-final-state-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.handle-final-state-fn]
    pub fn handle_final_state(
        &mut self,
        input_pos: u32,
        tape_pos: u32,
        container: &mut PmatchContainer,
    ) {
        if container.get_stack_depth() > 0 {
            // We're not the toplevel, return to caller. The caller is suspended
            // higher on the Rust call stack, so rebuild it from its shared tables
            // and the frame it held at the RTN call (both immutable / stashed)
            // rather than borrowing the live object again. [hfst/hfst#354]
            let rtn_target = container.get_latest_rtn_caller();
            let caller_frame = container.get_latest_caller_frame();
            let mut caller = if rtn_target == NO_SYMBOL_NUMBER {
                container
                    .toplevel
                    .as_ref()
                    .expect("toplevel present for RTN return")
                    .clone_with_frame(caller_frame)
            } else {
                container.alphabet.rtns[rtn_target as usize]
                    .as_ref()
                    .expect("RTN slot occupied for RTN return")
                    .clone_with_frame(caller_frame)
            };
            caller.rtn_return(input_pos, tape_pos, container);
        } else if container.is_in_locate_mode() {
            container.grab_location(input_pos, tape_pos);
        } else {
            container.note_analysis(input_pos, tape_pos);
        }
    }
}

// Integration helpers: the C++ calls 'alphabet.locatefy(.., this)' and
// 'alphabet.stringify(.., this)' where 'alphabet' is a member and 'this' is the
// container. The alphabet methods only read their own fields and the
// container's pattern-mode flags while mutating 'pattern_counts', so the two
// fields are handed over as disjoint borrows — no aliasing to model.
impl PmatchContainer {
    fn locatefy(&mut self, input_offset: u32, str: &WeightedDoubleTape) -> Location {
        self.alphabet.locatefy(
            input_offset,
            str,
            self.count_patterns,
            &mut self.pattern_counts,
        )
    }
    fn stringify(&mut self, str: &DoubleTape) -> String {
        self.alphabet.stringify(
            str,
            self.count_patterns,
            self.delete_patterns,
            self.mark_patterns,
            self.extract_patterns,
            &mut self.pattern_counts,
        )
    }
}

/// The locate-mode renderer of the hfst-pmatch tool, lifted from
/// tools/src/hfst-pmatch.cc: print one 'start|length|output|tag' line (with
/// '|weight' appended when 'print_weights') for the first location of every
/// matching location vector. Returns whether anything was printed (the tool
/// follows up with a separating blank line if so).
pub fn print_locate_matches(
    locations: &LocationVectorVector,
    outstream: &mut dyn std::io::Write,
    print_weights: bool,
) -> bool {
    let mut printed_something = false;
    for it in locations.iter() {
        if it[0].output != "@_NONMATCHING_@" {
            printed_something = true;
            let _ = write!(
                outstream,
                "{}|{}|{}|{}",
                it[0].start, it[0].length, it[0].output, it[0].tag
            );
            if print_weights {
                let _ = write!(outstream, "|{}", it[0].weight);
            }
            let _ = writeln!(outstream);
        }
    }
    printed_something
}
