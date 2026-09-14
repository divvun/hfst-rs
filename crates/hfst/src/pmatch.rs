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
        inputstream: &mut dyn std::io::BufRead,
        symbol_count: SymbolNumber,
        cont: &mut PmatchCore,
    ) -> crate::error::Result<PmatchAlphabet> {
        // C++ 'PmatchAlphabet(istream, n, cont)' derives from
        // 'TransducerAlphabet(istream, n, true)' then builds the pmatch symbol
        // maps; read the base alphabet from the stream and reuse the same
        // map-building done by 'new_from_alphabet'.
        let base = TransducerAlphabet::read_from(inputstream, symbol_count, true)?;
        Ok(Self::new_from_alphabet(&base, cont))
    }

    // ctor from existing alphabet: PmatchAlphabet(TransducerAlphabet const&, PmatchContainer*)
    pub fn new_from_alphabet(a: &TransducerAlphabet, cont: &mut PmatchCore) -> PmatchAlphabet {
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
        container: &mut PmatchCore,
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
        container: &mut PmatchCore,
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

    // The C++ 'get_rtn' returned a raw mutable 'PmatchTransducer *'. An RTN is
    // load-fixed and lives inside the shared core, so it is only ever borrowed:
    // the walk resolves a symbol to a '&PmatchTransducer' itself.

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
}

// ==================== PmatchTransducer (impl from workflow body agent) ====================
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
impl PmatchTransducer {
    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.pmatch-transducer-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.pmatch-transducer-fn]
    // ctor from istream
    pub fn new_from_stream(
        is: &mut dyn std::io::BufRead,
        index_table_size: TransitionTableIndex,
        transition_table_size: TransitionTableIndex,
        alphabet: &PmatchAlphabet,
        name: String,
    ) -> crate::error::Result<PmatchTransducer> {
        let orig_symbol_count = u32::try_from(alphabet.get_symbol_table().len())
            .expect("value out of u32 range") as SymbolNumber;
        let truncated = || {
            crate::err!(
                Hfst,
                "pmatch archive is truncated: the transducer's tables end early"
            )
        };
        // Both tables come off disk in one batched pass each, so a size field
        // inflated by corruption stops at the short read instead of asking the
        // allocator for the whole claim up front.
        let index_table =
            crate::transducer::TransducerTable::<TransitionWIndex>::read_from(is, index_table_size)
                .map_err(|_| truncated())?;
        let transition_table =
            crate::transducer::TransducerTable::<TransitionW>::read_from(is, transition_table_size)
                .map_err(|_| truncated())?;
        // [spec:hfst:req:table-residency.single-copy-load]
        let index_table = index_table.into_vector();
        let transition_table = transition_table.into_vector();

        // The runtime indexes the alphabet's parallel per-symbol vectors
        // (printability, capture tags, symbol lists, RTNs) with symbol numbers
        // taken straight from these entries, and follows their targets back
        // into the tables. Reject a pair that does not hold together here,
        // where the archive can still be named.
        let symbol_count = alphabet.get_symbol_table().len();
        for (position, entry) in index_table.iter().enumerate() {
            crate::transducer::validate_ol_index_entry(
                position,
                entry.get_input_symbol(),
                entry.get_target(),
                symbol_count,
                transition_table.len(),
            )?;
        }
        for (position, entry) in transition_table.iter().enumerate() {
            crate::transducer::validate_ol_transition_entry(
                position,
                entry.get_input_symbol(),
                entry.get_output_symbol(),
                entry.get_target(),
                symbol_count,
                index_table.len(),
                transition_table.len(),
            )?;
        }

        Ok(PmatchTransducer {
            name,
            transition_table: Arc::new(transition_table),
            index_table: Arc::new(index_table),
            orig_symbol_count,
        })
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
        PmatchTransducer {
            name,
            transition_table: Arc::new(transition_vector),
            index_table: Arc::new(index_vector),
            orig_symbol_count,
        }
    }

    /// The transition entry at `i`, or `None` past the end of the table.
    ///
    /// A state's arcs are walked by incrementing `i` until an entry with no
    /// input symbol ends the run. The terminator is written by the packer, but
    /// the walk is driven by targets read off disk, so "past the end" has to
    /// answer like that terminator rather than index raw.
    #[inline]
    pub(crate) fn transition_at(&self, i: TransitionTableIndex) -> Option<&TransitionW> {
        self.transition_table.get(i as usize)
    }

    /// The index entry at `i`, or `None` past the end of the table.
    ///
    /// The index table is probed at `state + input_symbol` and padded with
    /// blank entries for exactly the alphabet's *input* symbols. The pmatch
    /// runtime encodes the whole alphabet, though — identity, unknown and
    /// output-only symbols are numbered above `input_symbol_count` — so a probe
    /// can legitimately reach past the padding whenever the archive's alphabet
    /// was not harmonized as all-input (any plain optimized-lookup transducer
    /// handed to the runtime). C++ read past the vector and got a non-matching
    /// entry; `None` is that same answer, made explicit.
    #[inline]
    pub(crate) fn index_at(&self, i: TransitionTableIndex) -> Option<&TransitionWIndex> {
        self.index_table.get(i as usize)
    }

    #[inline]
    pub(crate) fn transition_input(&self, i: TransitionTableIndex) -> SymbolNumber {
        self.transition_at(i)
            .map_or(NO_SYMBOL_NUMBER, |t| t.get_input_symbol())
    }

    #[inline]
    pub(crate) fn transition_output(&self, i: TransitionTableIndex) -> SymbolNumber {
        self.transition_at(i)
            .map_or(NO_SYMBOL_NUMBER, |t| t.get_output_symbol())
    }

    #[inline]
    pub(crate) fn transition_target(&self, i: TransitionTableIndex) -> TransitionTableIndex {
        self.transition_at(i)
            .map_or(NO_TABLE_INDEX, |t| t.get_target())
    }

    #[inline]
    pub(crate) fn transition_weight(&self, i: TransitionTableIndex) -> Weight {
        self.transition_at(i).map_or(0.0, |t| t.get_weight())
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.is-final-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.is-final-fn]
    pub fn is_final(&self, i: TransitionTableIndex) -> bool {
        if Self::indexes_transition_table(i) {
            self.transition_at(i - TRANSITION_TARGET_TABLE_START)
                .is_some_and(|t| t.is_final())
        } else {
            self.index_at(i).is_some_and(|e| e.is_final())
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.get-weight-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.get-weight-fn]
    pub fn get_weight(&self, i: TransitionTableIndex) -> Weight {
        if Self::indexes_transition_table(i) {
            self.transition_weight(i - TRANSITION_TARGET_TABLE_START)
        } else {
            self.index_at(i).map_or(0.0, |e| e.final_weight())
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
            return i - TRANSITION_TARGET_TABLE_START;
        }
        match self.index_at(i + input as u32) {
            Some(entry) if entry.get_input_symbol() == input => {
                entry.get_target() - TRANSITION_TARGET_TABLE_START
            }
            // No entry for this (state, symbol) — the same "nothing to walk"
            // answer `is_good` turns into an immediate stop.
            _ => TRANSITION_TARGET_TABLE_START,
        }
    }

    // [spec:hfst:def:pmatch.hfst-ol.pmatch-transducer.final-index-fn]
    // [spec:hfst:sem:pmatch.hfst-ol.pmatch-transducer.final-index-fn]
    pub fn final_index(&self, i: TransitionTableIndex) -> bool {
        if Self::indexes_transition_table(i) {
            self.transition_at(i).is_some_and(|t| t.is_final())
        } else {
            self.index_at(i).is_some_and(|e| e.is_final())
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
