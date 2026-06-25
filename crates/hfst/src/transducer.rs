//! Port of `libhfst/src/implementations/optimized-lookup/transducer.{h,cc}`
//! (+ `find_epsilon_loops.cc`), namespace `hfst_ol` — the compiled
//! optimized-lookup transducer format and its lookup engine.
//!
//! Binary I/O fidelity: the C++ reads/writes raw struct bytes via
//! `is.read(reinterpret_cast<char*>(&p), sizeof(T))` (host-endian). That is
//! mirrored with native-endian `from_ne_bytes`/`to_ne_bytes`. `std::istream`
//! is modelled by [`IStream`], a thin wrapper over `&mut dyn Read` that tracks
//! a fail flag so the C++ `if(!is)` checks port directly; `std::ostream`
//! becomes `&mut dyn Write`.
//!
//! C++ value-type inheritance (the concrete base `TransitionIndex` with a
//! derived `TransitionWIndex` overriding `final_weight`, likewise
//! `Transition`/`TransitionW`) is modelled as a base struct plus a trait that
//! captures the virtual methods, since the tables hand them out through base
//! references. The pure-abstract `TransducerTablesInterface` becomes a trait
//! object.

use std::collections::{BTreeMap, BTreeSet};

use crate::hfst_data_types::size_t_to_uint;
use crate::hfst_exception_defs::TransducerHasWrongTypeException;
use crate::hfst_flag_diacritics::{FdOperation, FdTable};
use crate::hfst_symbol_defs::{is_default, is_identity, is_unknown};

// [spec:hfst:def:transducer.hfst-ol.symbol-number]
pub type SymbolNumber = u16;
// [spec:hfst:def:transducer.hfst-ol.transition-table-index]
pub type TransitionTableIndex = u32;
// [spec:hfst:def:transducer.hfst-ol.transition-number]
pub type TransitionNumber = u32;
// [spec:hfst:def:transducer.hfst-ol.state-id-number]
pub type StateIdNumber = u32;
// [spec:hfst:def:transducer.hfst-ol.value-number]
pub type ValueNumber = i16;
// [spec:hfst:def:transducer.hfst-ol.weight]
pub type Weight = f32;
// [spec:hfst:def:transducer.hfst-ol.symbol-number-set]
pub type SymbolNumberSet = BTreeSet<SymbolNumber>;
// [spec:hfst:def:transducer.hfst-ol.symbol-number-vector]
pub type SymbolNumberVector = Vec<SymbolNumber>;
// [spec:hfst:def:transducer.hfst-ol.transition-table-index-set]
pub type TransitionTableIndexSet = BTreeSet<TransitionTableIndex>;
// [spec:hfst:def:transducer.hfst-ol.symbol-table]
pub type SymbolTable = Vec<String>;

// for lookup
// [spec:hfst:def:transducer.hfst-ol.string-pair]
pub type StringPair = (String, String);

// for ospell
// [spec:hfst:def:transducer.hfst-ol.flag-diacritic-state]
pub type FlagDiacriticState = Vec<i16>;
// [spec:hfst:def:transducer.hfst-ol.operation-map]
pub type OperationMap = BTreeMap<SymbolNumber, FdOperation>;
// [spec:hfst:def:transducer.hfst-ol.string-symbol-map]
pub type StringSymbolMap = BTreeMap<String, SymbolNumber>;

// for epsilon loop checking
// [spec:hfst:def:transducer.hfst-ol.traversal-state]
#[derive(Clone)]
pub struct TraversalState {
    pub index: TransitionTableIndex,
    pub flags: FlagDiacriticState,
}

impl TraversalState {
    // [spec:hfst:def:transducer.hfst-ol.traversal-state.traversal-state-fn]
    // [spec:hfst:sem:transducer.hfst-ol.traversal-state.traversal-state-fn]
    pub fn new(i: TransitionTableIndex, f: FlagDiacriticState) -> Self {
        TraversalState { index: i, flags: f }
    }

    // Define an operation for checking state equivalence for the
    // purpose of detecting the same situation happening twice
    // [spec:hfst:def:find-epsilon-loops.hfst-ol.traversal-state.operator-fn]
    pub fn operator_eq(&self, rhs: &TraversalState) -> bool {
        if self.index != rhs.index {
            return false;
        }
        for i in 0..self.flags.len() {
            if self.flags[i] != rhs.flags[i] {
                return false;
            }
        }
        true
    }

    // [spec:hfst:def:find-epsilon-loops.hfst-ol.traversal-state.operator-fn]
    // [spec:hfst:sem:find-epsilon-loops.hfst-ol.traversal-state.operator-fn]
    pub fn operator_lt(&self, rhs: &TraversalState) -> bool {
        if self.index < rhs.index {
            return true;
        }
        if self.index > rhs.index {
            return false;
        }
        for i in 0..self.flags.len() {
            if self.flags[i] < rhs.flags[i] {
                return true;
            }
            if self.flags[i] > rhs.flags[i] {
                return false;
            }
        }
        false
    }
}

// `std::set<TraversalState>` orders by `operator<`; mirror that exactly.
impl PartialEq for TraversalState {
    fn eq(&self, other: &Self) -> bool {
        !self.operator_lt(other) && !other.operator_lt(self)
    }
}
impl Eq for TraversalState {}
impl PartialOrd for TraversalState {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for TraversalState {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.operator_lt(other) {
            std::cmp::Ordering::Less
        } else if other.operator_lt(self) {
            std::cmp::Ordering::Greater
        } else {
            std::cmp::Ordering::Equal
        }
    }
}

// [spec:hfst:def:transducer.hfst-ol.traversal-states]
pub type TraversalStates = BTreeSet<TraversalState>;

// parentheses avoid collision with windows macro 'max'
pub const NO_SYMBOL_NUMBER: SymbolNumber = SymbolNumber::MAX;
pub const NO_TABLE_INDEX: TransitionTableIndex = TransitionTableIndex::MAX;
pub const NO_COUNTER: u64 = u64::MAX;
pub const INFINITE_WEIGHT: Weight = NO_TABLE_INDEX as Weight;

// [spec:hfst:def:transducer.hfst-ol.header-flag]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HeaderFlag {
    Weighted,
    Deterministic,
    Input_deterministic,
    Minimized,
    Cyclic,
    Has_epsilon_epsilon_transitions,
    Has_input_epsilon_transitions,
    Has_input_epsilon_cycles,
    Has_unweighted_input_epsilon_cycles,
}

// This is 2^31, hopefully equal to UINT_MAX/2 rounded up.
// For some profound reason it can't be replaced with (UINT_MAX+1)/2.
pub const TRANSITION_TARGET_TABLE_START: TransitionTableIndex = 2147483648u32;
pub const MAX_IO_LEN: u32 = 10000;
pub const MAX_RECURSION_DEPTH: u32 = 5000;

// [spec:hfst:def:transducer.hfst-ol.indexes-transition-table-fn]
// [spec:hfst:sem:transducer.hfst-ol.indexes-transition-table-fn]
#[inline]
pub fn indexes_transition_table(i: TransitionTableIndex) -> bool {
    i >= TRANSITION_TARGET_TABLE_START
}
// [spec:hfst:def:transducer.hfst-ol.indexes-transition-index-table-fn]
// [spec:hfst:sem:transducer.hfst-ol.indexes-transition-index-table-fn]
#[inline]
pub fn indexes_transition_index_table(i: TransitionTableIndex) -> bool {
    i < TRANSITION_TARGET_TABLE_START
}

/// `std::istream` modelled with a fail flag, so the C++ `if(!is)` checks port
/// straight across. Reads are native-endian to mirror `reinterpret_cast`.
pub struct IStream<'a> {
    inner: &'a mut dyn std::io::Read,
    fail: bool,
    eof: bool,
}

impl<'a> IStream<'a> {
    pub fn new(inner: &'a mut dyn std::io::Read) -> Self {
        IStream {
            inner,
            fail: false,
            eof: false,
        }
    }

    /// `!is` — true when the stream is in a good (non-failed) state.
    pub fn good(&self) -> bool {
        !self.fail
    }

    /// `is.read(buf, buf.len())`: a short read sets the fail flag.
    pub fn read(&mut self, buf: &mut [u8]) {
        if self.fail {
            return;
        }
        let mut got = 0;
        while got < buf.len() {
            let mut b = [0u8; 1];
            match self.inner.read(&mut b) {
                Ok(0) => {
                    self.eof = true;
                    self.fail = true;
                    return;
                }
                Ok(_) => {
                    buf[got] = b[0];
                    got += 1;
                }
                Err(_) => {
                    self.fail = true;
                    return;
                }
            }
        }
    }

    /// `std::getline(is, str, delim)`: collect bytes up to (not including)
    /// `delim`; an immediate EOF with no bytes sets the fail flag.
    pub fn getline(&mut self, delim: u8) -> String {
        let mut bytes: Vec<u8> = Vec::new();
        let mut got_any = false;
        loop {
            let mut b = [0u8; 1];
            match self.inner.read(&mut b) {
                Ok(0) => {
                    self.eof = true;
                    if !got_any {
                        self.fail = true;
                    }
                    break;
                }
                Ok(_) => {
                    got_any = true;
                    if b[0] == delim {
                        break;
                    }
                    bytes.push(b[0]);
                }
                Err(_) => {
                    self.fail = true;
                    break;
                }
            }
        }
        String::from_utf8_lossy(&bytes).into_owned()
    }

    fn read_u16(&mut self) -> u16 {
        let mut b = [0u8; 2];
        self.read(&mut b);
        u16::from_ne_bytes(b)
    }
    fn read_u32(&mut self) -> u32 {
        let mut b = [0u8; 4];
        self.read(&mut b);
        u32::from_ne_bytes(b)
    }
}

// `os.write(reinterpret_cast<const char*>(&prop), sizeof(prop))` for the
// integer/float properties, native-endian.
// [spec:hfst:def:transducer.hfst-ol.transducer-header.write-property-fn]
// [spec:hfst:sem:transducer.hfst-ol.transducer-header.write-property-fn]
fn write_u16(prop: u16, os: &mut dyn std::io::Write) {
    let _ = os.write_all(&prop.to_ne_bytes());
}
fn write_u32(prop: u32, os: &mut dyn std::io::Write) {
    let _ = os.write_all(&prop.to_ne_bytes());
}
// [spec:hfst:def:transducer.hfst-ol.transducer-header.write-bool-property-fn]
// [spec:hfst:sem:transducer.hfst-ol.transducer-header.write-bool-property-fn]
fn write_bool_property(value: bool, os: &mut dyn std::io::Write) {
    let prop: u32 = if value { 1 } else { 0 };
    let _ = os.write_all(&prop.to_ne_bytes());
}

// [spec:hfst:def:transducer.hfst-ol.transducer-header]
#[derive(Clone)]
pub struct TransducerHeader {
    pub(crate) number_of_input_symbols: SymbolNumber,
    pub(crate) number_of_symbols: SymbolNumber,
    pub(crate) size_of_transition_index_table: TransitionTableIndex,
    pub(crate) size_of_transition_target_table: TransitionTableIndex,

    pub(crate) number_of_states: StateIdNumber,
    pub(crate) number_of_transitions: TransitionNumber,

    pub(crate) weighted: bool,
    pub(crate) deterministic: bool,
    pub(crate) input_deterministic: bool,
    pub(crate) minimized: bool,
    pub(crate) cyclic: bool,
    pub(crate) has_epsilon_epsilon_transitions: bool,
    pub(crate) has_input_epsilon_transitions: bool,
    pub(crate) has_input_epsilon_cycles: bool,
    pub(crate) has_unweighted_input_epsilon_cycles: bool,
}

impl TransducerHeader {
    // [spec:hfst:def:transducer.hfst-ol.transducer-header.header-error-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.header-error-fn]
    fn header_error() -> ! {
        crate::HFST_THROW!(TransducerHasWrongTypeException)
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-header.read-bool-property-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.read-bool-property-fn]
    fn read_bool_property(is: &mut IStream) -> bool {
        let prop = is.read_u32();
        if prop == 0 {
            return false;
        }
        if prop == 1 {
            return true;
        }
        Self::header_error();
    }

    /// `TransducerHeader(bool weights)`.
    pub fn new_weighted(weights: bool) -> Self {
        TransducerHeader {
            number_of_input_symbols: 0,
            number_of_symbols: 1, // epsilon
            size_of_transition_index_table: 1,
            size_of_transition_target_table: 0,
            number_of_states: 1,
            number_of_transitions: 0,
            weighted: weights,
            deterministic: true,
            input_deterministic: true,
            minimized: true,
            cyclic: false,
            has_epsilon_epsilon_transitions: false,
            has_input_epsilon_transitions: false,
            has_input_epsilon_cycles: false,
            has_unweighted_input_epsilon_cycles: false,
        }
    }

    // a basic constructor that's only told information we
    // actually use at the moment
    pub fn new_sizes(
        input_symbols: SymbolNumber,
        symbols: SymbolNumber,
        transition_index_table: TransitionTableIndex,
        transition_table: TransitionTableIndex,
        weights: bool,
    ) -> Self {
        TransducerHeader {
            number_of_input_symbols: input_symbols,
            number_of_symbols: symbols, // epsilon
            size_of_transition_index_table: transition_index_table,
            size_of_transition_target_table: transition_table,
            number_of_states: 0,
            number_of_transitions: 0,
            weighted: weights,
            deterministic: true,
            input_deterministic: true,
            minimized: true,
            cyclic: false,
            has_epsilon_epsilon_transitions: false,
            has_input_epsilon_transitions: false,
            has_input_epsilon_cycles: false,
            has_unweighted_input_epsilon_cycles: false,
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-header.transducer-header-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.transducer-header-fn]
    pub fn new_istream(is: &mut IStream) -> Self {
        let header = TransducerHeader {
            number_of_input_symbols: is.read_u16(),
            number_of_symbols: is.read_u16(),
            size_of_transition_index_table: is.read_u32(),
            size_of_transition_target_table: is.read_u32(),
            number_of_states: is.read_u32(),
            number_of_transitions: is.read_u32(),
            weighted: Self::read_bool_property(is),
            deterministic: Self::read_bool_property(is),
            input_deterministic: Self::read_bool_property(is),
            minimized: Self::read_bool_property(is),
            cyclic: Self::read_bool_property(is),
            has_epsilon_epsilon_transitions: Self::read_bool_property(is),
            has_input_epsilon_transitions: Self::read_bool_property(is),
            has_input_epsilon_cycles: Self::read_bool_property(is),
            has_unweighted_input_epsilon_cycles: Self::read_bool_property(is),
        };
        if !is.good() {
            crate::HFST_THROW!(TransducerHasWrongTypeException);
        }
        header
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-header.symbol-count-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.symbol-count-fn]
    pub fn symbol_count(&self) -> SymbolNumber {
        self.number_of_symbols
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-header.input-symbol-count-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.input-symbol-count-fn]
    pub fn input_symbol_count(&self) -> SymbolNumber {
        self.number_of_input_symbols
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-header.increment-symbol-count-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.increment-symbol-count-fn]
    pub fn increment_symbol_count(&mut self) {
        self.number_of_symbols += 1;
        self.number_of_input_symbols += 1;
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-header.index-table-size-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.index-table-size-fn]
    pub fn index_table_size(&self) -> TransitionTableIndex {
        self.size_of_transition_index_table
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-header.target-table-size-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.target-table-size-fn]
    pub fn target_table_size(&self) -> TransitionTableIndex {
        self.size_of_transition_target_table
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-header.probe-flag-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.probe-flag-fn]
    pub fn probe_flag(&self, flag: HeaderFlag) -> bool {
        match flag {
            HeaderFlag::Weighted => self.weighted,
            HeaderFlag::Deterministic => self.deterministic,
            HeaderFlag::Input_deterministic => self.input_deterministic,
            HeaderFlag::Minimized => self.minimized,
            HeaderFlag::Cyclic => self.cyclic,
            HeaderFlag::Has_epsilon_epsilon_transitions => self.has_epsilon_epsilon_transitions,
            HeaderFlag::Has_input_epsilon_transitions => self.has_input_epsilon_transitions,
            HeaderFlag::Has_input_epsilon_cycles => self.has_input_epsilon_cycles,
            HeaderFlag::Has_unweighted_input_epsilon_cycles => {
                self.has_unweighted_input_epsilon_cycles
            }
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-header.set-flag-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.set-flag-fn]
    // NB: faithful to the C++, which ignores `value` and always sets `true`.
    pub fn set_flag(&mut self, flag: HeaderFlag, _value: bool) {
        match flag {
            HeaderFlag::Weighted => self.weighted = true,
            HeaderFlag::Deterministic => self.deterministic = true,
            HeaderFlag::Input_deterministic => self.input_deterministic = true,
            HeaderFlag::Minimized => self.minimized = true,
            HeaderFlag::Cyclic => self.cyclic = true,
            HeaderFlag::Has_epsilon_epsilon_transitions => {
                self.has_epsilon_epsilon_transitions = true
            }
            HeaderFlag::Has_input_epsilon_transitions => self.has_input_epsilon_transitions = true,
            HeaderFlag::Has_input_epsilon_cycles => self.has_input_epsilon_cycles = true,
            HeaderFlag::Has_unweighted_input_epsilon_cycles => {
                self.has_unweighted_input_epsilon_cycles = true
            }
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-header.display-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.display-fn]
    pub fn display(&self) {
        println!("Transducer properties:");
        println!(" number_of_symbols: {}", self.number_of_symbols);
        println!(" number_of_input_symbols: {}", self.number_of_input_symbols);
        println!(
            " size_of_transition_index_table: {}",
            self.size_of_transition_index_table
        );
        println!(
            " size_of_transition_target_table: {}",
            self.size_of_transition_target_table
        );
        println!(" number_of_states: {}", self.number_of_states);
        println!(" number_of_transitions: {}", self.number_of_transitions);
        println!(" weighted: {}", self.weighted as u32);
        println!(" deterministic: {}", self.deterministic as u32);
        println!(" input_deterministic: {}", self.input_deterministic as u32);
        println!(" minimized: {}", self.minimized as u32);
        println!(" cyclic: {}", self.cyclic as u32);
        println!(
            " has_epsilon_epsilon_transitions: {}",
            self.has_epsilon_epsilon_transitions as u32
        );
        println!(
            " has_input_epsilon_transitions: {}",
            self.has_input_epsilon_transitions as u32
        );
        println!(
            " has_input_epsilon_cycles: {}",
            self.has_input_epsilon_cycles as u32
        );
        println!(
            " has_unweighted_input_epsilon_cycles: {}",
            self.has_unweighted_input_epsilon_cycles as u32
        );
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-header.write-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.write-fn]
    pub fn write(&self, os: &mut dyn std::io::Write) {
        write_u16(self.number_of_input_symbols, os);
        write_u16(self.number_of_symbols, os);
        write_u32(self.size_of_transition_index_table, os);
        write_u32(self.size_of_transition_target_table, os);
        write_u32(self.number_of_states, os);
        write_u32(self.number_of_transitions, os);
        write_bool_property(self.weighted, os);
        write_bool_property(self.deterministic, os);
        write_bool_property(self.input_deterministic, os);
        write_bool_property(self.minimized, os);
        write_bool_property(self.cyclic, os);
        write_bool_property(self.has_epsilon_epsilon_transitions, os);
        write_bool_property(self.has_input_epsilon_transitions, os);
        write_bool_property(self.has_input_epsilon_cycles, os);
        write_bool_property(self.has_unweighted_input_epsilon_cycles, os);
    }
}

// [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.unicode-class-cache-value]
#[derive(Clone, Copy, PartialEq, Eq)]
enum UnicodeClassCacheValue {
    upperalpha,
    loweralpha,
    whitespace,
    no_value,
    other,
}

// [spec:hfst:def:transducer.hfst-ol.transducer-alphabet]
#[derive(Clone)]
pub struct TransducerAlphabet {
    pub(crate) symbol_table: SymbolTable,
    pub(crate) fd_table: FdTable<SymbolNumber>,
    pub(crate) unknown_symbol: SymbolNumber,
    pub(crate) default_symbol: SymbolNumber,
    pub(crate) identity_symbol: SymbolNumber,
    pub(crate) orig_symbol_count: SymbolNumber,
    unicode_cache: Vec<UnicodeClassCacheValue>,
}

impl TransducerAlphabet {
    pub fn new() -> Self {
        let mut symbol_table = SymbolTable::new();
        symbol_table.push("@_EPSILON_SYMBOL_@".to_string());
        TransducerAlphabet {
            symbol_table,
            fd_table: FdTable::new(),
            unknown_symbol: NO_SYMBOL_NUMBER,
            default_symbol: NO_SYMBOL_NUMBER,
            identity_symbol: NO_SYMBOL_NUMBER,
            orig_symbol_count: 1,
            unicode_cache: Vec::new(),
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.transducer-alphabet-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.transducer-alphabet-fn]
    pub fn new_istream(
        is: &mut IStream,
        symbol_count: SymbolNumber,
        preserve_diacritic_strings: bool,
    ) -> Self {
        let mut alpha = TransducerAlphabet {
            symbol_table: SymbolTable::new(),
            fd_table: FdTable::new(),
            unknown_symbol: NO_SYMBOL_NUMBER,
            default_symbol: NO_SYMBOL_NUMBER,
            identity_symbol: NO_SYMBOL_NUMBER,
            orig_symbol_count: 0,
            unicode_cache: Vec::new(),
        };
        let mut i: SymbolNumber = 0;
        while i < symbol_count {
            let mut str = is.getline(b'\0');
            if FdOperation::is_diacritic(&str) {
                alpha.fd_table.define_diacritic(i, &str);
                if !preserve_diacritic_strings {
                    str = String::new();
                }
            } else if is_unknown(&str) {
                alpha.unknown_symbol = i;
            } else if is_default(&str) {
                alpha.default_symbol = i;
            } else if is_identity(&str) {
                alpha.identity_symbol = i;
            }
            if !is.good() {
                crate::HFST_THROW!(TransducerHasWrongTypeException);
            }
            alpha.symbol_table.push(str);
            i += 1;
        }
        alpha.orig_symbol_count = size_t_to_uint(alpha.symbol_table.len()) as SymbolNumber;
        alpha
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.fake-read-alphabet-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.fake-read-alphabet-fn]
    pub fn fake_read_alphabet(is: &mut IStream, symbol_count: SymbolNumber) {
        let mut i: SymbolNumber = 0;
        while i < symbol_count {
            let _str = is.getline(b'\0');
            i += 1;
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.add-symbol-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.add-symbol-fn]
    pub fn add_symbol_str(&mut self, symbol: &str) {
        self.symbol_table.push(symbol.to_string());
    }

    pub fn add_symbol(&mut self, symbol: &String) {
        self.symbol_table.push(symbol.clone());
    }

    pub fn new_symboltable(st: &SymbolTable) -> Self {
        let mut alpha = TransducerAlphabet {
            symbol_table: st.clone(),
            fd_table: FdTable::new(),
            unknown_symbol: NO_SYMBOL_NUMBER,
            default_symbol: NO_SYMBOL_NUMBER,
            identity_symbol: NO_SYMBOL_NUMBER,
            orig_symbol_count: 0,
            unicode_cache: Vec::new(),
        };
        let mut i: SymbolNumber = 0;
        while (i as usize) < alpha.symbol_table.len() {
            if FdOperation::is_diacritic(&alpha.symbol_table[i as usize]) {
                let s = alpha.symbol_table[i as usize].clone();
                alpha.fd_table.define_diacritic(i, &s);
            } else if is_unknown(&alpha.symbol_table[i as usize]) {
                alpha.unknown_symbol = i;
            } else if is_default(&alpha.symbol_table[i as usize]) {
                alpha.default_symbol = i;
            } else if is_identity(&alpha.symbol_table[i as usize]) {
                alpha.identity_symbol = i;
            }
            i += 1;
        }
        alpha.orig_symbol_count = size_t_to_uint(alpha.symbol_table.len()) as SymbolNumber;
        alpha
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.symbol-from-string-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.symbol-from-string-fn]
    pub fn symbol_from_string(&self, symbol_string: &str) -> SymbolNumber {
        for i in 0..self.symbol_table.len() {
            if self.symbol_table[i] == symbol_string {
                return i as SymbolNumber;
            }
        }
        NO_SYMBOL_NUMBER
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.build-string-symbol-map-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.build-string-symbol-map-fn]
    pub fn build_string_symbol_map(&self) -> StringSymbolMap {
        let mut ss_map = StringSymbolMap::new();
        for i in 0..self.symbol_table.len() {
            ss_map.insert(self.symbol_table[i].clone(), i as SymbolNumber);
        }
        ss_map
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.is-like-epsilon-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.is-like-epsilon-fn]
    pub fn is_like_epsilon(&self, symbol: SymbolNumber) -> bool {
        if self.fd_table.is_diacritic(symbol) {
            return true;
        }
        if symbol as usize >= self.symbol_table.len() {
            return false;
        }
        let s = self.symbol_table[symbol as usize].as_bytes();
        // Check for Insert symbols like @I.something@ here
        if s.len() >= 5 && s[0] == b'@' && s[1] == b'I' && s[2] == b'.' && s[s.len() - 1] == b'@' {
            return true;
        }
        false
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.is-meta-arc-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.is-meta-arc-fn]
    pub fn is_meta_arc(&self, symbol: SymbolNumber) -> bool {
        if symbol == NO_SYMBOL_NUMBER {
            return false;
        }
        (symbol == self.unknown_symbol)
            || (symbol == self.default_symbol)
            || (symbol == self.identity_symbol)
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.cache-unicode-class-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.cache-unicode-class-fn]
    pub fn cache_unicode_class(&mut self, symbol: SymbolNumber) {
        while self.unicode_cache.len() <= symbol as usize {
            self.unicode_cache.push(UnicodeClassCacheValue::no_value);
        }
        if self.unicode_cache[symbol as usize] != UnicodeClassCacheValue::no_value {
            return;
        }
        // icu::UnicodeString::fromUTF8 + first code point's class. Rust `char`
        // already carries the same Unicode properties ICU queries here.
        if let Some(c) = self.symbol_table[symbol as usize].chars().next() {
            if c.is_lowercase() {
                self.unicode_cache[symbol as usize] = UnicodeClassCacheValue::loweralpha;
            } else if c.is_uppercase() {
                self.unicode_cache[symbol as usize] = UnicodeClassCacheValue::upperalpha;
            } else if c.is_whitespace() {
                self.unicode_cache[symbol as usize] = UnicodeClassCacheValue::whitespace;
            } else {
                self.unicode_cache[symbol as usize] = UnicodeClassCacheValue::other;
            }
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.is-unicode-alpha-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.is-unicode-alpha-fn]
    pub fn is_unicode_alpha(&mut self, symbol: SymbolNumber) -> bool {
        self.cache_unicode_class(symbol);
        self.unicode_cache[symbol as usize] == UnicodeClassCacheValue::loweralpha
            || self.unicode_cache[symbol as usize] == UnicodeClassCacheValue::upperalpha
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.is-unicode-upperalpha-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.is-unicode-upperalpha-fn]
    pub fn is_unicode_upperalpha(&mut self, symbol: SymbolNumber) -> bool {
        self.cache_unicode_class(symbol);
        self.unicode_cache[symbol as usize] == UnicodeClassCacheValue::upperalpha
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.is-unicode-loweralpha-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.is-unicode-loweralpha-fn]
    pub fn is_unicode_loweralpha(&mut self, symbol: SymbolNumber) -> bool {
        self.cache_unicode_class(symbol);
        self.unicode_cache[symbol as usize] == UnicodeClassCacheValue::loweralpha
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.is-unicode-whitespace-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.is-unicode-whitespace-fn]
    pub fn is_unicode_whitespace(&mut self, symbol: SymbolNumber) -> bool {
        self.cache_unicode_class(symbol);
        self.unicode_cache[symbol as usize] == UnicodeClassCacheValue::whitespace
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.display-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.display-fn]
    pub fn display(&self) {
        println!("Transducer alphabet:");
        for i in 0..self.symbol_table.len() {
            println!(" Symbol {}: {}", i, self.symbol_table[i]);
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.write-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.write-fn]
    pub fn write(&self, os: &mut dyn std::io::Write) {
        for i in self.symbol_table.iter() {
            let _ = os.write_all(i.as_bytes());
            let _ = os.write_all(&[0u8]);
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.has-flag-diacritics-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.has-flag-diacritics-fn]
    pub fn has_flag_diacritics(&self) -> bool {
        self.fd_table.num_features() > 0
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.is-flag-diacritic-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.is-flag-diacritic-fn]
    pub fn is_flag_diacritic(&self, symbol: SymbolNumber) -> bool {
        self.fd_table.is_diacritic(symbol)
    }

    pub fn get_symbol_table(&self) -> &SymbolTable {
        &self.symbol_table
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.string-from-symbol-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.string-from-symbol-fn]
    // represent epsilon as blank string
    pub fn string_from_symbol(&self, symbol: SymbolNumber) -> String {
        if symbol == 0 {
            String::new()
        } else {
            self.symbol_table[symbol as usize].clone()
        }
    }

    pub fn get_fd_table(&self) -> &FdTable<SymbolNumber> {
        &self.fd_table
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.get-operation-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.get-operation-fn]
    pub fn get_operation(&self, symbol: SymbolNumber) -> Option<&FdOperation> {
        self.fd_table.get_operation(symbol)
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.get-unknown-symbol-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.get-unknown-symbol-fn]
    pub fn get_unknown_symbol(&self) -> SymbolNumber {
        self.unknown_symbol
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.get-default-symbol-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.get-default-symbol-fn]
    pub fn get_default_symbol(&self) -> SymbolNumber {
        self.default_symbol
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.get-identity-symbol-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.get-identity-symbol-fn]
    pub fn get_identity_symbol(&self) -> SymbolNumber {
        self.identity_symbol
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.get-orig-symbol-count-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.get-orig-symbol-count-fn]
    pub fn get_orig_symbol_count(&self) -> SymbolNumber {
        self.orig_symbol_count
    }
}

impl Default for TransducerAlphabet {
    fn default() -> Self {
        Self::new()
    }
}

/// Captures the `static const size_t size` + `T(char*)` constructor that
/// `TransducerTable<T>` requires of its entry type for binary loading.
pub trait TableEntry {
    const SIZE: usize;
    fn from_bytes(p: &[u8]) -> Self;
}

/// The (object-safe) virtual surface of `TransitionIndex` (base) used through
/// base references; `TransitionWIndex` overrides `final_weight`. The static
/// `create_final()` lives in [`IndexCtor`] so this stays dyn-compatible.
pub trait IndexEntry {
    fn get_target(&self) -> TransitionTableIndex;
    fn get_input_symbol(&self) -> SymbolNumber;
    fn matches(&self, s: SymbolNumber) -> bool;
    fn final_(&self) -> bool;
    fn final_weight(&self) -> Weight;
    fn write(&self, os: &mut dyn std::io::Write, weighted: bool);
    fn display(&self);
}

/// `static TransitionIndex::create_final()` — a generic-bound-only trait
/// (returns `Self`, so it can't ride on the dyn-safe [`IndexEntry`]).
pub trait IndexCtor {
    fn create_final() -> Self;
}

/// The (object-safe) virtual surface of `Transition` (base); `TransitionW`
/// overrides `get_weight`.
pub trait TransitionEntry {
    fn get_target(&self) -> TransitionTableIndex;
    fn get_output_symbol(&self) -> SymbolNumber;
    fn get_input_symbol(&self) -> SymbolNumber;
    fn matches(&self, s: SymbolNumber) -> bool;
    fn final_(&self) -> bool;
    fn get_weight(&self) -> Weight;
    fn write(&self, os: &mut dyn std::io::Write, weighted: bool);
    fn display(&self);
}

// [spec:hfst:def:transducer.hfst-ol.transition-index]
#[derive(Clone)]
pub struct TransitionIndex {
    pub(crate) input_symbol: SymbolNumber,
    pub(crate) first_transition_index: TransitionTableIndex,
}

impl TransitionIndex {
    pub fn new() -> Self {
        TransitionIndex {
            input_symbol: NO_SYMBOL_NUMBER,
            first_transition_index: NO_TABLE_INDEX,
        }
    }

    pub fn new_values(input: SymbolNumber, first_transition: TransitionTableIndex) -> Self {
        TransitionIndex {
            input_symbol: input,
            first_transition_index: first_transition,
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transition-index.get-target-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-index.get-target-fn]
    pub fn get_target(&self) -> TransitionTableIndex {
        self.first_transition_index
    }
    // [spec:hfst:def:transducer.hfst-ol.transition-index.get-input-symbol-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-index.get-input-symbol-fn]
    pub fn get_input_symbol(&self) -> SymbolNumber {
        self.input_symbol
    }

    // [spec:hfst:def:transducer.hfst-ol.transition-index.create-final-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-index.create-final-fn]
    pub fn create_final() -> TransitionIndex {
        TransitionIndex::new_values(NO_SYMBOL_NUMBER, 1)
    }

    // [spec:hfst:def:transducer.hfst-ol.transition-index.display-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-index.display-fn]
    pub fn display(&self) {
        println!(
            "input_symbol: {}, target: {}{}",
            self.input_symbol,
            self.first_transition_index,
            if self.final_() { " (final)" } else { "" }
        );
    }

    // [spec:hfst:def:transducer.hfst-ol.transition-index.matches-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-index.matches-fn]
    pub fn matches(&self, s: SymbolNumber) -> bool {
        self.input_symbol != NO_SYMBOL_NUMBER && self.input_symbol == s
    }
    // [spec:hfst:def:transducer.hfst-ol.transition-index.final-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-index.final-fn]
    pub fn final_(&self) -> bool {
        self.input_symbol == NO_SYMBOL_NUMBER && self.first_transition_index != NO_TABLE_INDEX
    }
    // [spec:hfst:def:transducer.hfst-ol.transition-index.final-weight-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-index.final-weight-fn]
    pub fn final_weight(&self) -> Weight {
        0.0
    }

    // [spec:hfst:def:transducer.hfst-ol.transition-index.write-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-index.write-fn]
    pub fn write(&self, os: &mut dyn std::io::Write, weighted: bool) {
        write_u16(self.input_symbol, os);
        if !weighted
            && self.input_symbol == NO_SYMBOL_NUMBER
            && self.first_transition_index != NO_TABLE_INDEX
        {
            // Make sure that we write the correct type of final index
            let unweighted_final_index: u32 = 1;
            write_u32(unweighted_final_index, os);
        } else {
            write_u32(self.first_transition_index, os);
        }
    }
}

impl Default for TransitionIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl TableEntry for TransitionIndex {
    const SIZE: usize = 2 + 4; // sizeof(SymbolNumber) + sizeof(TransitionTableIndex)
    fn from_bytes(p: &[u8]) -> Self {
        TransitionIndex {
            input_symbol: u16::from_ne_bytes([p[0], p[1]]),
            first_transition_index: u32::from_ne_bytes([p[2], p[3], p[4], p[5]]),
        }
    }
}

impl IndexEntry for TransitionIndex {
    fn get_target(&self) -> TransitionTableIndex {
        self.get_target()
    }
    fn get_input_symbol(&self) -> SymbolNumber {
        self.get_input_symbol()
    }
    fn matches(&self, s: SymbolNumber) -> bool {
        self.matches(s)
    }
    fn final_(&self) -> bool {
        self.final_()
    }
    fn final_weight(&self) -> Weight {
        self.final_weight()
    }
    fn write(&self, os: &mut dyn std::io::Write, weighted: bool) {
        self.write(os, weighted)
    }
    fn display(&self) {
        self.display()
    }
}

impl IndexCtor for TransitionIndex {
    fn create_final() -> Self {
        TransitionIndex::create_final()
    }
}

// [spec:hfst:def:transducer.hfst-ol.transition-w-index]
#[derive(Clone)]
pub struct TransitionWIndex {
    pub(crate) base: TransitionIndex,
}

impl TransitionWIndex {
    pub fn new() -> Self {
        TransitionWIndex {
            base: TransitionIndex::new(),
        }
    }
    // [spec:hfst:def:transducer.hfst-ol.transition-w-index.transition-w-index-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-w-index.transition-w-index-fn]
    pub fn new_values(input: SymbolNumber, first_transition: TransitionTableIndex) -> Self {
        TransitionWIndex {
            base: TransitionIndex::new_values(input, first_transition),
        }
    }

    pub fn get_target(&self) -> TransitionTableIndex {
        self.base.get_target()
    }
    pub fn get_input_symbol(&self) -> SymbolNumber {
        self.base.get_input_symbol()
    }
    pub fn matches(&self, s: SymbolNumber) -> bool {
        self.base.matches(s)
    }
    pub fn final_(&self) -> bool {
        self.base.final_()
    }

    // [spec:hfst:def:transducer.hfst-ol.transition-w-index.final-weight-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-w-index.final-weight-fn]
    pub fn final_weight(&self) -> Weight {
        // union { TransitionTableIndex i; Weight w; }; weight.i = first; return weight.w;
        Weight::from_bits(self.base.first_transition_index)
    }

    pub fn create_final() -> TransitionWIndex {
        TransitionWIndex::new_values(NO_SYMBOL_NUMBER, 0)
    }

    // [spec:hfst:def:transducer.hfst-ol.transition-w-index.create-final-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-w-index.create-final-fn]
    pub fn create_final_weight(w: Weight) -> TransitionWIndex {
        // union to_weight { TransitionTableIndex i; Weight w; }; weight.w = w;
        TransitionWIndex::new_values(NO_SYMBOL_NUMBER, w.to_bits())
    }

    pub fn write(&self, os: &mut dyn std::io::Write, weighted: bool) {
        self.base.write(os, weighted)
    }
    pub fn display(&self) {
        self.base.display()
    }
}

impl Default for TransitionWIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl TableEntry for TransitionWIndex {
    const SIZE: usize = 2 + 4;
    fn from_bytes(p: &[u8]) -> Self {
        TransitionWIndex {
            base: TransitionIndex::from_bytes(p),
        }
    }
}

impl IndexEntry for TransitionWIndex {
    fn get_target(&self) -> TransitionTableIndex {
        self.get_target()
    }
    fn get_input_symbol(&self) -> SymbolNumber {
        self.get_input_symbol()
    }
    fn matches(&self, s: SymbolNumber) -> bool {
        self.matches(s)
    }
    fn final_(&self) -> bool {
        self.final_()
    }
    fn final_weight(&self) -> Weight {
        self.final_weight()
    }
    fn write(&self, os: &mut dyn std::io::Write, weighted: bool) {
        self.write(os, weighted)
    }
    fn display(&self) {
        self.display()
    }
}

impl IndexCtor for TransitionWIndex {
    fn create_final() -> Self {
        TransitionWIndex::create_final()
    }
}

// [spec:hfst:def:transducer.hfst-ol.transition]
#[derive(Clone)]
pub struct Transition {
    pub(crate) input_symbol: SymbolNumber,
    pub(crate) output_symbol: SymbolNumber,
    pub(crate) target_index: TransitionTableIndex,
}

impl Transition {
    pub fn new_values(
        input: SymbolNumber,
        output: SymbolNumber,
        target: TransitionTableIndex,
    ) -> Self {
        Transition {
            input_symbol: input,
            output_symbol: output,
            target_index: target,
        }
    }

    pub fn new_final(final_: bool) -> Self {
        Transition {
            input_symbol: NO_SYMBOL_NUMBER,
            output_symbol: NO_SYMBOL_NUMBER,
            target_index: if final_ { 1 } else { NO_TABLE_INDEX },
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transition.get-target-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition.get-target-fn]
    pub fn get_target(&self) -> TransitionTableIndex {
        self.target_index
    }
    // [spec:hfst:def:transducer.hfst-ol.transition.get-output-symbol-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition.get-output-symbol-fn]
    pub fn get_output_symbol(&self) -> SymbolNumber {
        self.output_symbol
    }
    // [spec:hfst:def:transducer.hfst-ol.transition.get-input-symbol-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition.get-input-symbol-fn]
    pub fn get_input_symbol(&self) -> SymbolNumber {
        self.input_symbol
    }

    // [spec:hfst:def:transducer.hfst-ol.transition.matches-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition.matches-fn]
    pub fn matches(&self, s: SymbolNumber) -> bool {
        self.input_symbol != NO_SYMBOL_NUMBER && self.input_symbol == s
    }
    // [spec:hfst:def:transducer.hfst-ol.transition.final-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition.final-fn]
    pub fn final_(&self) -> bool {
        self.input_symbol == NO_SYMBOL_NUMBER
            && self.output_symbol == NO_SYMBOL_NUMBER
            && self.target_index == 1
    }
    // [spec:hfst:def:transducer.hfst-ol.transition.get-weight-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition.get-weight-fn]
    pub fn get_weight(&self) -> Weight {
        0.0
    }

    // [spec:hfst:def:transducer.hfst-ol.transition.display-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition.display-fn]
    pub fn display(&self) {
        println!(
            "input_symbol: {}, output_symbol: {}, target: {}{}",
            self.input_symbol,
            self.output_symbol,
            self.target_index,
            if self.final_() { " (final)" } else { "" }
        );
    }

    // [spec:hfst:def:transducer.hfst-ol.transition.write-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition.write-fn]
    pub fn write(&self, os: &mut dyn std::io::Write, weighted: bool) {
        write_u16(self.input_symbol, os);
        write_u16(self.output_symbol, os);
        write_u32(self.target_index, os);
        if weighted {
            // C++ `os << 0.0f` writes the text representation, i.e. "0".
            let _ = os.write_all(format!("{}", 0.0f32).as_bytes());
        }
    }
}

impl TableEntry for Transition {
    const SIZE: usize = 2 * 2 + 4;
    fn from_bytes(p: &[u8]) -> Self {
        Transition {
            input_symbol: u16::from_ne_bytes([p[0], p[1]]),
            output_symbol: u16::from_ne_bytes([p[2], p[3]]),
            target_index: u32::from_ne_bytes([p[4], p[5], p[6], p[7]]),
        }
    }
}

impl TransitionEntry for Transition {
    fn get_target(&self) -> TransitionTableIndex {
        self.get_target()
    }
    fn get_output_symbol(&self) -> SymbolNumber {
        self.get_output_symbol()
    }
    fn get_input_symbol(&self) -> SymbolNumber {
        self.get_input_symbol()
    }
    fn matches(&self, s: SymbolNumber) -> bool {
        self.matches(s)
    }
    fn final_(&self) -> bool {
        self.final_()
    }
    fn get_weight(&self) -> Weight {
        self.get_weight()
    }
    fn write(&self, os: &mut dyn std::io::Write, weighted: bool) {
        self.write(os, weighted)
    }
    fn display(&self) {
        self.display()
    }
}

// [spec:hfst:def:transducer.hfst-ol.transition-w]
#[derive(Clone)]
pub struct TransitionW {
    pub(crate) base: Transition,
    pub(crate) transition_weight: Weight,
}

impl TransitionW {
    pub fn new_values(
        input: SymbolNumber,
        output: SymbolNumber,
        target: TransitionTableIndex,
        w: Weight,
    ) -> Self {
        TransitionW {
            base: Transition::new_values(input, output, target),
            transition_weight: w,
        }
    }

    pub fn new_final(final_: bool, w: Weight) -> Self {
        TransitionW {
            base: Transition::new_final(final_),
            transition_weight: w,
        }
    }

    pub fn get_target(&self) -> TransitionTableIndex {
        self.base.get_target()
    }
    pub fn get_output_symbol(&self) -> SymbolNumber {
        self.base.get_output_symbol()
    }
    pub fn get_input_symbol(&self) -> SymbolNumber {
        self.base.get_input_symbol()
    }
    pub fn matches(&self, s: SymbolNumber) -> bool {
        self.base.matches(s)
    }
    pub fn final_(&self) -> bool {
        self.base.final_()
    }

    // [spec:hfst:def:transducer.hfst-ol.transition-w.get-weight-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-w.get-weight-fn]
    pub fn get_weight(&self) -> Weight {
        self.transition_weight
    }

    // [spec:hfst:def:transducer.hfst-ol.transition-w.display-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-w.display-fn]
    pub fn display(&self) {
        println!(
            "input_symbol: {}, output_symbol: {}, target: {}, weight: {}{}",
            self.base.input_symbol,
            self.base.output_symbol,
            self.base.target_index,
            self.transition_weight,
            if self.final_() { " (final)" } else { "" }
        );
    }

    // [spec:hfst:def:transducer.hfst-ol.transition-w.write-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-w.write-fn]
    pub fn write(&self, os: &mut dyn std::io::Write, weighted: bool) {
        self.base.write(os, false);
        if weighted {
            let _ = os.write_all(&self.transition_weight.to_ne_bytes());
        }
    }
}

impl TableEntry for TransitionW {
    const SIZE: usize = 2 * 2 + 4 + 4;
    // [spec:hfst:def:transducer.hfst-ol.transition-w.transition-w-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-w.transition-w-fn]
    fn from_bytes(p: &[u8]) -> Self {
        TransitionW {
            base: Transition::from_bytes(p),
            transition_weight: f32::from_ne_bytes([p[8], p[9], p[10], p[11]]),
        }
    }
}

impl TransitionEntry for TransitionW {
    fn get_target(&self) -> TransitionTableIndex {
        self.get_target()
    }
    fn get_output_symbol(&self) -> SymbolNumber {
        self.get_output_symbol()
    }
    fn get_input_symbol(&self) -> SymbolNumber {
        self.get_input_symbol()
    }
    fn matches(&self, s: SymbolNumber) -> bool {
        self.matches(s)
    }
    fn final_(&self) -> bool {
        self.final_()
    }
    fn get_weight(&self) -> Weight {
        self.get_weight()
    }
    fn write(&self, os: &mut dyn std::io::Write, weighted: bool) {
        self.write(os, weighted)
    }
    fn display(&self) {
        self.display()
    }
}

// [spec:hfst:def:transducer.hfst-ol.transducer-table]
#[derive(Clone)]
pub struct TransducerTable<T> {
    pub(crate) table: Vec<T>,
}

impl<T: TableEntry + Clone> TransducerTable<T> {
    pub fn new() -> Self {
        TransducerTable { table: Vec::new() }
    }

    pub fn new_filled(size: usize, entry: T) -> Self {
        TransducerTable {
            table: vec![entry; size],
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-table.transducer-table-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-table.transducer-table-fn]
    pub fn new_istream(is: &mut IStream, index_count: TransitionTableIndex) -> Self {
        let total = T::SIZE * index_count as usize;
        let mut buf = vec![0u8; total];
        is.read(&mut buf);
        let mut table = Vec::new();
        let mut remaining = index_count;
        let mut p = 0usize;
        while remaining != 0 {
            table.push(T::from_bytes(&buf[p..p + T::SIZE]));
            remaining -= 1;
            p += T::SIZE;
        }
        TransducerTable { table }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-table.append-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-table.append-fn]
    pub fn append(&mut self, v: T) {
        self.table.push(v);
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-table.set-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-table.set-fn]
    pub fn set(&mut self, index: usize, v: T) {
        self.table[index] = v;
    }

    pub fn at(&self, i: TransitionTableIndex) -> &T {
        if i < TRANSITION_TARGET_TABLE_START {
            &self.table[i as usize]
        } else {
            &self.table[(i - TRANSITION_TARGET_TABLE_START) as usize]
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-table.get-vector-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-table.get-vector-fn]
    pub fn get_vector(&self) -> Vec<T> {
        self.table.clone()
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-table.size-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-table.size-fn]
    pub fn size(&self) -> u32 {
        size_t_to_uint(self.table.len())
    }
}

impl<T: TableEntry + Clone> Default for TransducerTable<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: IndexEntry> TransducerTable<T> {
    // [spec:hfst:def:transducer.hfst-ol.transducer-table.display-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-table.display-fn]
    pub fn display_index(&self) {
        for i in 0..self.table.len() {
            print!("{}", i);
            print!(": ");
            self.table[i].display();
        }
    }
}

impl<T: TransitionEntry> TransducerTable<T> {
    pub fn display_transition(&self) {
        for i in 0..self.table.len() {
            print!("{}", i);
            print!("/{}", i as u64 + TRANSITION_TARGET_TABLE_START as u64);
            print!(": ");
            self.table[i].display();
        }
    }
}

// [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface]
pub trait TransducerTablesInterface {
    fn get_index(&self, i: TransitionTableIndex) -> &dyn IndexEntry;
    fn get_transition(&self, i: TransitionTableIndex) -> &dyn TransitionEntry;
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.get-weight-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.get-weight-fn]
    fn get_weight(&self, i: TransitionTableIndex) -> Weight;
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.get-transition-input-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.get-transition-input-fn]
    fn get_transition_input(&self, i: TransitionTableIndex) -> SymbolNumber;
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.get-transition-output-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.get-transition-output-fn]
    fn get_transition_output(&self, i: TransitionTableIndex) -> SymbolNumber;
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.get-transition-target-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.get-transition-target-fn]
    fn get_transition_target(&self, i: TransitionTableIndex) -> TransitionTableIndex;
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.get-transition-finality-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.get-transition-finality-fn]
    fn get_transition_finality(&self, i: TransitionTableIndex) -> bool;
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.get-index-input-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.get-index-input-fn]
    fn get_index_input(&self, i: TransitionTableIndex) -> SymbolNumber;
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.get-index-target-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.get-index-target-fn]
    fn get_index_target(&self, i: TransitionTableIndex) -> TransitionTableIndex;
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.get-index-finality-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.get-index-finality-fn]
    fn get_index_finality(&self, i: TransitionTableIndex) -> bool;
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.get-final-weight-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.get-final-weight-fn]
    fn get_final_weight(&self, i: TransitionTableIndex) -> Weight;

    // [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.display-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.display-fn]
    fn display(&self) {}
}

// [spec:hfst:def:transducer.hfst-ol.transducer-tables]
pub struct TransducerTables<T1: IndexEntry, T2: TransitionEntry> {
    index_table: TransducerTable<T1>,
    transition_table: TransducerTable<T2>,
}

impl<T1: IndexEntry + TableEntry + Clone + IndexCtor, T2: TransitionEntry + TableEntry + Clone>
    TransducerTables<T1, T2>
{
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables.transducer-tables-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables.transducer-tables-fn]
    pub fn new_istream(
        is: &mut IStream,
        index_table_size: TransitionTableIndex,
        transition_table_size: TransitionTableIndex,
    ) -> Self {
        TransducerTables {
            index_table: TransducerTable::new_istream(is, index_table_size),
            transition_table: TransducerTable::new_istream(is, transition_table_size),
        }
    }

    pub fn new() -> Self {
        TransducerTables {
            index_table: TransducerTable::new_filled(1, T1::create_final()),
            transition_table: TransducerTable::new(),
        }
    }

    pub fn new_tables(
        index_table: TransducerTable<T1>,
        transition_table: TransducerTable<T2>,
    ) -> Self {
        TransducerTables {
            index_table,
            transition_table,
        }
    }
}

impl<T1: IndexEntry + TableEntry + Clone + IndexCtor, T2: TransitionEntry + TableEntry + Clone>
    Default for TransducerTables<T1, T2>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T1: IndexEntry + TableEntry + Clone + IndexCtor, T2: TransitionEntry + TableEntry + Clone>
    TransducerTablesInterface for TransducerTables<T1, T2>
{
    fn get_index(&self, i: TransitionTableIndex) -> &dyn IndexEntry {
        self.index_table.at(i)
    }
    fn get_transition(&self, i: TransitionTableIndex) -> &dyn TransitionEntry {
        self.transition_table.at(i)
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-weight-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-weight-fn]
    fn get_weight(&self, i: TransitionTableIndex) -> Weight {
        self.transition_table.at(i).get_weight()
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-transition-input-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-transition-input-fn]
    fn get_transition_input(&self, i: TransitionTableIndex) -> SymbolNumber {
        self.transition_table.at(i).get_input_symbol()
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-transition-output-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-transition-output-fn]
    fn get_transition_output(&self, i: TransitionTableIndex) -> SymbolNumber {
        self.transition_table.at(i).get_output_symbol()
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-transition-target-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-transition-target-fn]
    fn get_transition_target(&self, i: TransitionTableIndex) -> TransitionTableIndex {
        self.transition_table.at(i).get_target()
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-transition-finality-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-transition-finality-fn]
    fn get_transition_finality(&self, i: TransitionTableIndex) -> bool {
        self.transition_table.at(i).final_()
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-index-input-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-index-input-fn]
    fn get_index_input(&self, i: TransitionTableIndex) -> SymbolNumber {
        self.index_table.at(i).get_input_symbol()
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-index-target-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-index-target-fn]
    fn get_index_target(&self, i: TransitionTableIndex) -> TransitionTableIndex {
        self.index_table.at(i).get_target()
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-index-finality-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-index-finality-fn]
    fn get_index_finality(&self, i: TransitionTableIndex) -> bool {
        self.index_table.at(i).final_()
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-final-weight-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-final-weight-fn]
    fn get_final_weight(&self, i: TransitionTableIndex) -> Weight {
        self.index_table.at(i).final_weight()
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-tables.display-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables.display-fn]
    fn display(&self) {
        println!("Transition index table:");
        self.index_table.display_index();
        println!("Transition table:");
        self.transition_table.display_transition();
    }
}

// There follow some classes for implementing lookup

// [spec:hfst:def:transducer.hfst-ol.ol-letter-trie-vector]
pub type OlLetterTrieVector = Vec<Option<Box<OlLetterTrie>>>;

// [spec:hfst:def:transducer.hfst-ol.ol-letter-trie]
pub struct OlLetterTrie {
    letters: OlLetterTrieVector,
    symbols: SymbolNumberVector,
}

impl OlLetterTrie {
    pub fn new() -> Self {
        let mut letters: OlLetterTrieVector = Vec::with_capacity(u8::MAX as usize + 1);
        for _ in 0..(u8::MAX as usize + 1) {
            letters.push(None);
        }
        OlLetterTrie {
            letters,
            symbols: vec![NO_SYMBOL_NUMBER; u8::MAX as usize + 1],
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.ol-letter-trie.add-string-fn]
    // [spec:hfst:sem:transducer.hfst-ol.ol-letter-trie.add-string-fn]
    // `p` is a 0-terminated byte slice positioned at the current char.
    pub fn add_string(&mut self, p: &[u8], symbol_key: SymbolNumber) {
        if p[1] == 0 {
            self.symbols[p[0] as usize] = symbol_key;
            return;
        }
        if self.letters[p[0] as usize].is_none() {
            self.letters[p[0] as usize] = Some(Box::new(OlLetterTrie::new()));
        }
        let idx = p[0] as usize;
        self.letters[idx]
            .as_mut()
            .unwrap()
            .add_string(&p[1..], symbol_key);
    }

    // [spec:hfst:def:transducer.hfst-ol.ol-letter-trie.has-key-starting-with-fn]
    // [spec:hfst:sem:transducer.hfst-ol.ol-letter-trie.has-key-starting-with-fn]
    pub fn has_key_starting_with(&self, c: u8) -> bool {
        self.letters[c as usize].is_some()
    }

    // [spec:hfst:def:transducer.hfst-ol.ol-letter-trie.find-key-fn]
    // [spec:hfst:sem:transducer.hfst-ol.ol-letter-trie.find-key-fn]
    // `p` is an index into `buf` advanced by reference, mirroring `char ** p`.
    pub fn find_key(&self, buf: &[u8], p: &mut usize) -> SymbolNumber {
        let old_p = *p;
        *p += 1;
        if self.letters[buf[old_p] as usize].is_none() {
            return self.symbols[buf[old_p] as usize];
        }
        let s = self.letters[buf[old_p] as usize]
            .as_ref()
            .unwrap()
            .find_key(buf, p);
        if s == NO_SYMBOL_NUMBER {
            *p -= 1;
            return self.symbols[buf[old_p] as usize];
        }
        s
    }
}

impl Default for OlLetterTrie {
    fn default() -> Self {
        Self::new()
    }
}

// [spec:hfst:def:transducer.hfst-ol.encoder]
pub struct Encoder {
    number_of_input_symbols: SymbolNumber,
    letters: OlLetterTrie,
    ascii_symbols: SymbolNumberVector,
}

impl Encoder {
    // [spec:hfst:def:transducer.hfst-ol.encoder.encoder-fn]
    // [spec:hfst:sem:transducer.hfst-ol.encoder.encoder-fn]
    pub fn new(st: &SymbolTable, input_symbol_count: SymbolNumber) -> Self {
        let mut encoder = Encoder {
            number_of_input_symbols: input_symbol_count,
            letters: OlLetterTrie::new(),
            ascii_symbols: vec![NO_SYMBOL_NUMBER; 128],
        };
        encoder.read_input_symbols(st);
        encoder
    }

    // [spec:hfst:def:transducer.hfst-ol.encoder.read-input-symbols-fn]
    // [spec:hfst:sem:transducer.hfst-ol.encoder.read-input-symbols-fn]
    pub fn read_input_symbols(&mut self, kt: &SymbolTable) {
        for k in 0..self.number_of_input_symbols {
            let sym = kt[k as usize].clone();
            self.read_input_symbol(&sym, k as i32);
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.encoder.read-input-symbol-fn]
    // [spec:hfst:sem:transducer.hfst-ol.encoder.read-input-symbol-fn]
    pub fn read_input_symbol(&mut self, s: &str, s_num: i32) {
        let bytes = s.as_bytes();
        let strlen = bytes.len();
        if strlen == 1
            && should_ascii_tokenize(bytes[0])
            && !self.letters.has_key_starting_with(bytes[0])
        {
            self.ascii_symbols[bytes[0] as usize] = s_num as SymbolNumber;
        }
        // If there's an ascii tokenized symbol shadowing this, remove it
        if strlen > 1
            && should_ascii_tokenize(bytes[0])
            && self.ascii_symbols[bytes[0] as usize] != NO_SYMBOL_NUMBER
        {
            self.ascii_symbols[bytes[0] as usize] = NO_SYMBOL_NUMBER;
        }
        // add_string walks a 0-terminated buffer.
        let mut buf = bytes.to_vec();
        buf.push(0);
        self.letters.add_string(&buf, s_num as SymbolNumber);
    }

    // [spec:hfst:def:transducer.hfst-ol.encoder.find-key-fn]
    // [spec:hfst:sem:transducer.hfst-ol.encoder.find-key-fn]
    pub fn find_key(&self, buf: &[u8], p: &mut usize) -> SymbolNumber {
        if !should_ascii_tokenize(buf[*p])
            || self.ascii_symbols[buf[*p] as usize] == NO_SYMBOL_NUMBER
        {
            return self.letters.find_key(buf, p);
        }
        let s = self.ascii_symbols[buf[*p] as usize];
        *p += 1;
        s
    }
}

// [spec:hfst:def:transducer.hfst-ol.symbol-pair]
#[derive(Clone, Copy)]
pub struct SymbolPair {
    pub input: SymbolNumber,
    pub output: SymbolNumber,
}

impl SymbolPair {
    // [spec:hfst:def:transducer.hfst-ol.symbol-pair.symbol-pair-fn]
    // [spec:hfst:sem:transducer.hfst-ol.symbol-pair.symbol-pair-fn]
    pub fn new() -> Self {
        SymbolPair {
            input: 0,
            output: 0,
        }
    }
    pub fn new_values(i: SymbolNumber, o: SymbolNumber) -> Self {
        SymbolPair {
            input: i,
            output: o,
        }
    }
}

impl Default for SymbolPair {
    fn default() -> Self {
        Self::new()
    }
}

// A vector that can be written to at any position, so that it
// adds new elements if the desired element isn't already present.
// [spec:hfst:def:transducer.hfst-ol.double-tape]
#[derive(Clone)]
pub struct DoubleTape {
    pub inner: Vec<SymbolPair>,
}

impl DoubleTape {
    pub fn new() -> Self {
        DoubleTape { inner: Vec::new() }
    }

    pub fn write_pair(&mut self, pos: u32, in_: SymbolNumber, out: SymbolNumber) {
        while pos as usize >= self.inner.len() {
            self.inner.push(SymbolPair::new());
        }
        self.inner[pos as usize] = SymbolPair::new_values(in_, out);
    }

    pub fn write_vec(&mut self, pos: u32, vec: &Vec<SymbolNumber>) {
        while pos as usize + vec.len() >= self.inner.len() {
            self.inner.push(SymbolPair::new());
        }
        for i in 0..vec.len() {
            self.inner[pos as usize + i] = SymbolPair::new_values(vec[i], vec[i]);
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.double-tape.write-fn]
    // [spec:hfst:sem:transducer.hfst-ol.double-tape.write-fn]
    // The C++ `write(pos, pair<iterator,iterator>)` over a `[start, end)` slice.
    pub fn write_slice(&mut self, pos: u32, slice: &[SymbolNumber]) {
        let size = slice.len();
        while pos as usize + size >= self.inner.len() {
            self.inner.push(SymbolPair::new());
        }
        for i in 0..size {
            self.inner[pos as usize + i] = SymbolPair::new_values(slice[i], slice[i]);
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.double-tape.extract-slice-fn]
    // [spec:hfst:sem:transducer.hfst-ol.double-tape.extract-slice-fn]
    pub fn extract_slice(&self, mut start: u32, stop: u32) -> DoubleTape {
        let mut retval = DoubleTape::new();
        while start < stop {
            retval.inner.push(self.inner[start as usize]);
            start += 1;
        }
        retval
    }
}

impl Default for DoubleTape {
    fn default() -> Self {
        Self::new()
    }
}

// [spec:hfst:def:transducer.hfst-ol.weighted-double-tape]
#[derive(Clone)]
pub struct WeightedDoubleTape {
    pub tape: DoubleTape,
    pub weight: Weight,
}

impl WeightedDoubleTape {
    // [spec:hfst:def:transducer.hfst-ol.weighted-double-tape.weighted-double-tape-fn]
    // [spec:hfst:sem:transducer.hfst-ol.weighted-double-tape.weighted-double-tape-fn]
    pub fn new(dt: DoubleTape, w: Weight) -> Self {
        WeightedDoubleTape {
            tape: dt,
            weight: w,
        }
    }
}

// [spec:hfst:def:transducer.hfst-ol.tape]
#[derive(Clone)]
pub struct Tape {
    pub inner: SymbolNumberVector,
}

impl Tape {
    pub fn new() -> Self {
        Tape { inner: Vec::new() }
    }

    // [spec:hfst:def:transducer.hfst-ol.tape.write-fn]
    // [spec:hfst:sem:transducer.hfst-ol.tape.write-fn]
    pub fn write(&mut self, i: u32, s: SymbolNumber) {
        if self.inner.len() > i as usize {
            self.inner[i as usize] = s;
        } else {
            while self.inner.len() <= i as usize {
                self.inner.push(NO_SYMBOL_NUMBER);
            }
            self.inner[i as usize] = s;
        }
    }

    pub fn at(&self, i: u32) -> SymbolNumber {
        self.inner[i as usize]
    }
}

impl Default for Tape {
    fn default() -> Self {
        Self::new()
    }
}

// [spec:hfst:def:transducer.hfst-ol.should-ascii-tokenize-fn]
// [spec:hfst:sem:transducer.hfst-ol.should-ascii-tokenize-fn]
pub fn should_ascii_tokenize(c: u8) -> bool {
    c <= 127
}

// [spec:hfst:def:transducer.hfst-ol.string-weight-pair]
pub type StringWeightPair = (String, Weight);

// [spec:hfst:def:transducer.hfst-ol.s-transition]
pub struct STransition {
    pub index: TransitionTableIndex,
    pub symbol: SymbolNumber,
    pub weight: Weight,
}

impl STransition {
    pub fn new(i: TransitionTableIndex, s: SymbolNumber) -> Self {
        STransition {
            index: i,
            symbol: s,
            weight: 0.0,
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.s-transition.s-transition-fn]
    // [spec:hfst:sem:transducer.hfst-ol.s-transition.s-transition-fn]
    pub fn new_weighted(i: TransitionTableIndex, s: SymbolNumber, w: Weight) -> Self {
        STransition {
            index: i,
            symbol: s,
            weight: w,
        }
    }
}
