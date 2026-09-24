//! Full port of 'libhfst/src/implementations/optimized-lookup/pmatch.{h,cc}'
//! (namespace 'hfst_ol').
//!
//! The declarations live here; the two halves of the runtime they describe live
//! next door. [`PmatchCore`](crate::pmatch_core::PmatchCore) is everything fixed
//! once an archive is loaded — the `PmatchAlphabet` and every
//! `PmatchTransducer`, the toplevel plus the RTNs in `alphabet.rtns` — and is
//! shared behind an `Arc`. [`PmatchContainer`] is one caller-owned run over such
//! a core, holding every byte a match writes.
//!
//! Ownership scheme (see crate notes): a 'PmatchTransducer' stores NO
//! back-reference to its core or alphabet, and no run state of its own; the
//! engine methods live on a walk that borrows the net out of the core and takes
//! the run state as a parameter.

use std::collections::BTreeMap;
use std::sync::Arc;

use icu::segmenter::GraphemeClusterSegmenter;

use crate::hfst_flag_diacritics::{FdState, FdTable};
use crate::pmatch_core::PmatchCore;
use crate::transducer::{
    NO_COUNTER, NO_SYMBOL_NUMBER, NO_TABLE_INDEX, SymbolNumber, SymbolNumberVector,
    TRANSITION_TARGET_TABLE_START, TransducerAlphabet, TransitionTableIndex, TransitionW,
    TransitionWIndex, Weight, WeightedDoubleTape,
};

pub use crate::pmatch_state::PmatchContainer;

mod alphabet_build;
mod alphabet_query;
mod alphabet_spelling;
mod transducer_tables;

// [spec:hfst:def:pmatch.hfst-ol.rtn-call-stack]
pub type RtnCallStack = Vec<RtnStackFrame>;
// [spec:hfst:def:pmatch.hfst-ol.rtn-call-stacks]
pub type RtnCallStacks = Vec<RtnCallStack>;
// [spec:hfst:def:pmatch.hfst-ol.rtn-vector]
// In C++ this is 'std::vector<PmatchTransducer *>'. Because the shared core owns
// the RTNs, we store owned boxes (Option = the NULL slot) here in the alphabet.
pub type RtnVector = Vec<Option<Box<PmatchTransducer>>>;
// [spec:hfst:def:pmatch.hfst-ol.rtn-name-map]
pub type RtnNameMap = BTreeMap<String, SymbolNumber>;
// [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.local-variables]
// One per active context check / RTN frame of a single walk. The C++ kept this
// on the 'PmatchTransducer', which is what made a net un-re-enterable; it is
// owned by the walk instead.
pub type LocalVariablesStack = Vec<LocalVariables>;
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
    // returns, the caller is suspended higher on the Rust stack, so the return
    // walk borrows the caller's net a second time and resumes it from this
    // frame. [hfst/hfst#354]
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

// [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer]
// Nothing here changes after the archive is read. The tables are held behind an
// 'Arc' so the core they sit in can be shared across threads, and the walk state
// the C++ kept alongside them — the local-variable stack — lives on the walk
// instead, which is what lets an RTN return re-enter a net that is already
// running higher up the Rust stack without cloning it. [hfst/hfst#354]
// [spec:hfst:req:lookup-run-state.pmatch-shared-core]
pub struct PmatchTransducer {
    pub(crate) name: String,
    pub(crate) transition_table: Arc<Vec<TransitionW>>,
    pub(crate) index_table: Arc<Vec<TransitionWIndex>>,
    pub(crate) orig_symbol_count: SymbolNumber,
    // NOTE: no 'alphabet' and no 'core' back-references; the engine methods
    // receive the core and the run state as parameters.
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
