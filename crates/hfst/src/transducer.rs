//! Port of 'libhfst/src/implementations/optimized-lookup/transducer.{h,cc}'
//! (+ 'find_epsilon_loops.cc'), namespace 'hfst_ol' — the compiled
//! optimized-lookup transducer format and its lookup engine.
//!
//! Binary I/O fidelity: the C++ reads/writes raw struct bytes via
//! 'is.read(reinterpret_cast<char*>(&p), sizeof(T))' (host-endian). That is
//! mirrored with native-endian 'from_ne_bytes'/'to_ne_bytes'. 'std::istream'
//! is modelled by ['IStream'], a thin wrapper over '&mut dyn Read' that tracks
//! a fail flag so the C++ 'if(!is)' checks port directly; 'std::ostream'
//! becomes '&mut dyn Write'.
//!
//! C++ value-type inheritance (the concrete base 'TransitionIndex' with a
//! derived 'TransitionWIndex' overriding 'final_weight', likewise
//! 'Transition'/'TransitionW') is modelled as a base struct plus a trait that
//! captures the virtual methods, since the tables hand them out through base
//! references. The pure-abstract 'TransducerTablesInterface' becomes a trait
//! object.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::ControlFlow;
use std::time::Instant;

use crate::hfst_data_types::{
    HfstOneLevelPath, HfstOneLevelPaths, HfstTwoLevelPath, HfstTwoLevelPaths, StringVector,
};
use crate::hfst_flag_diacritics::{FdOperation, FdState, FdTable};
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
}

// State equivalence for the purpose of detecting the same situation
// happening twice.
// 'operator==' is declared in transducer.h and defined in
// find-epsilon-loops.cc — one function, two ids.
// [spec:hfst:def:find-epsilon-loops.hfst-ol.traversal-state.operator-fn]
// [spec:hfst:def:transducer.hfst-ol.traversal-state.operator-fn]
// [spec:hfst:sem:transducer.hfst-ol.traversal-state.operator-fn]
impl PartialEq for TraversalState {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}
impl Eq for TraversalState {}
impl PartialOrd for TraversalState {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
// 'std::set<TraversalState>' orders by 'operator<'; mirror that exactly.
// [spec:hfst:def:find-epsilon-loops.hfst-ol.traversal-state.operator-fn]
// [spec:hfst:sem:find-epsilon-loops.hfst-ol.traversal-state.operator-fn]
impl Ord for TraversalState {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.index
            .cmp(&other.index)
            .then_with(|| self.flags.cmp(&other.flags))
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

/// 'std::istream' modelled with a fail flag, so the C++ 'if(!is)' checks port
/// straight across. Reads are native-endian to mirror 'reinterpret_cast'.
pub struct IStream<'a> {
    // Boxed so the stream can either BORROW a reader ('new') or OWN one
    // ('new_owned', for the stdin/file-backed *InputStream constructors that the
    // C++ builds around an ifstream/cin).
    inner: Box<dyn std::io::Read + 'a>,
    fail: bool,
    eof: bool,
    // Bytes pushed back via 'putback'/'unget' (std::istream's get-area), LIFO:
    // the last pushed byte is the next one returned by get()/read()/getline().
    putback: Vec<u8>,
}

impl<'a> IStream<'a> {
    pub fn new(inner: &'a mut dyn std::io::Read) -> Self {
        IStream {
            inner: Box::new(inner),
            fail: false,
            eof: false,
            putback: Vec::new(),
        }
    }

    /// Construct an IStream that OWNS its reader (e.g. an opened file or stdin),
    /// for the backend '*InputStream::new'/'new_filename' constructors.
    pub fn new_owned(inner: impl std::io::Read + 'a) -> Self {
        IStream {
            inner: Box::new(inner),
            fail: false,
            eof: false,
            putback: Vec::new(),
        }
    }

    /// Consume the stream into a single 'Read' that first yields any remaining
    /// put-back bytes (LIFO order) and then the rest of the underlying reader.
    /// Used by 'HfstInputStream(std::istream&)' to adopt a borrowed stream as its
    /// owned source.
    pub fn into_reader(self) -> Box<dyn std::io::Read + 'a> {
        if self.putback.is_empty() {
            return self.inner;
        }
        // 'putback' is LIFO (last pushed is read first); reverse it so a Cursor
        // replays the bytes in read order ahead of the underlying reader.
        use std::io::Read as _;
        let mut pending = self.putback;
        pending.reverse();
        Box::new(std::io::Cursor::new(pending).chain(self.inner))
    }

    /// '!is' — true when the stream is in a good (non-failed) state.
    pub fn good(&self) -> bool {
        !self.fail
    }

    /// 'is.clear()': reset the fail/eof state (e.g. after a short read while
    /// peeking).
    pub fn clear(&mut self) {
        self.fail = false;
        self.eof = false;
    }

    /// 'is.get()': read and return the next byte, or -1 at end of stream
    /// (mirrors std::istream::get()'s int return).
    pub fn get(&mut self) -> i32 {
        if let Some(b) = self.putback.pop() {
            return b as i32;
        }
        if self.fail {
            return -1;
        }
        let mut b = [0u8; 1];
        match self.inner.read(&mut b) {
            Ok(0) => {
                self.eof = true;
                -1
            }
            Ok(_) => b[0] as i32,
            Err(_) => {
                self.fail = true;
                -1
            }
        }
    }

    /// 'is.putback(c)' / 'is.unget()': return a byte to the get-area so the
    /// next read sees it again.
    pub fn putback(&mut self, c: u8) {
        self.putback.push(c);
    }

    /// Read all remaining bytes (the put-back get-area first, then the reader to
    /// EOF). Used by the backend 'read_transducer' (load one FST from the prefix,
    /// then put the unused remainder back).
    pub fn read_to_end(&mut self) -> Vec<u8> {
        let mut buf: Vec<u8> = Vec::new();
        while let Some(b) = self.putback.pop() {
            buf.push(b);
        }
        let _ = std::io::Read::read_to_end(&mut self.inner, &mut buf);
        buf
    }

    /// 'is.read(buf, buf.len())': a short read sets the fail flag.
    pub fn read(&mut self, buf: &mut [u8]) {
        if self.fail {
            return;
        }
        let mut got = 0;
        while got < buf.len() {
            if let Some(b) = self.putback.pop() {
                buf[got] = b;
                got += 1;
                continue;
            }
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

    /// 'std::getline(is, str, delim)': collect bytes up to (not including)
    /// 'delim'; an immediate EOF with no bytes sets the fail flag.
    pub fn getline(&mut self, delim: u8) -> String {
        let mut bytes: Vec<u8> = Vec::new();
        let mut got_any = false;
        loop {
            if let Some(b) = self.putback.pop() {
                got_any = true;
                if b == delim {
                    break;
                }
                bytes.push(b);
                continue;
            }
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

    // 'is.read(reinterpret_cast<char*>(&p), sizeof(T))' for the integer
    // properties, native-endian — the typed mirror of the static template
    // 'read_property<T>' that 'TransducerHeader' uses to read its fields.
    // [spec:hfst:def:transducer.hfst-ol.transducer-header.read-property-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.read-property-fn]
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

// 'os.write(reinterpret_cast<const char*>(&prop), sizeof(prop))' for the
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
    fn header_error() -> crate::error::Error {
        crate::err!(TransducerHasWrongType)
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-header.read-bool-property-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.read-bool-property-fn]
    fn read_bool_property(is: &mut IStream) -> crate::error::Result<bool> {
        let prop = is.read_u32();
        if prop == 0 {
            return Ok(false);
        }
        if prop == 1 {
            return Ok(true);
        }
        Err(Self::header_error())
    }

    /// 'TransducerHeader(bool weights)'.
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
    pub fn new_istream(is: &mut IStream) -> crate::error::Result<Self> {
        let header = TransducerHeader {
            number_of_input_symbols: is.read_u16(),
            number_of_symbols: is.read_u16(),
            size_of_transition_index_table: is.read_u32(),
            size_of_transition_target_table: is.read_u32(),
            number_of_states: is.read_u32(),
            number_of_transitions: is.read_u32(),
            weighted: Self::read_bool_property(is)?,
            deterministic: Self::read_bool_property(is)?,
            input_deterministic: Self::read_bool_property(is)?,
            minimized: Self::read_bool_property(is)?,
            cyclic: Self::read_bool_property(is)?,
            has_epsilon_epsilon_transitions: Self::read_bool_property(is)?,
            has_input_epsilon_transitions: Self::read_bool_property(is)?,
            has_input_epsilon_cycles: Self::read_bool_property(is)?,
            has_unweighted_input_epsilon_cycles: Self::read_bool_property(is)?,
        };
        if !is.good() {
            crate::bail!(TransducerHasWrongType);
        }
        Ok(header)
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
    // NB: faithful to the C++, which ignores 'value' and always sets 'true'.
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
    ) -> crate::error::Result<Self> {
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
                crate::bail!(TransducerHasWrongType);
            }
            alpha.symbol_table.push(str);
            i += 1;
        }
        alpha.orig_symbol_count = u32::try_from(alpha.symbol_table.len())
            .expect("value out of u32 range") as SymbolNumber;
        Ok(alpha)
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
        alpha.orig_symbol_count = u32::try_from(alpha.symbol_table.len())
            .expect("value out of u32 range") as SymbolNumber;
        alpha
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.symbol-from-string-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.symbol-from-string-fn]
    pub fn symbol_from_string(&self, symbol_string: &str) -> Option<SymbolNumber> {
        for i in 0..self.symbol_table.len() {
            if self.symbol_table[i] == symbol_string {
                return Some(i as SymbolNumber);
            }
        }
        None
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
        // icu::UnicodeString::fromUTF8 + first code point's class. Rust 'char'
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

/// Captures the 'static const size_t size' + 'T(char*)' constructor that
/// 'TransducerTable<T>' requires of its entry type for binary loading.
pub trait TableEntry {
    const SIZE: usize;
    fn from_bytes(p: &[u8]) -> Self;
}

/// The (object-safe) virtual surface of 'TransitionIndex' (base) used through
/// base references; 'TransitionWIndex' overrides 'final_weight'. The static
/// 'create_final()' lives in ['IndexCtor'] so this stays dyn-compatible.
pub trait IndexEntry {
    fn get_target(&self) -> TransitionTableIndex;
    fn get_input_symbol(&self) -> SymbolNumber;
    fn matches(&self, s: SymbolNumber) -> bool;
    fn is_final(&self) -> bool;
    fn final_weight(&self) -> Weight;
    fn write(&self, os: &mut dyn std::io::Write, weighted: bool);
    fn display(&self);
}

/// 'static TransitionIndex::create_final()' — a generic-bound-only trait
/// (returns 'Self', so it can't ride on the dyn-safe ['IndexEntry']).
pub trait IndexCtor {
    fn create_final() -> Self;
}

/// The (object-safe) virtual surface of 'Transition' (base); 'TransitionW'
/// overrides 'get_weight'.
pub trait TransitionEntry {
    fn get_target(&self) -> TransitionTableIndex;
    fn get_output_symbol(&self) -> SymbolNumber;
    fn get_input_symbol(&self) -> SymbolNumber;
    fn matches(&self, s: SymbolNumber) -> bool;
    fn is_final(&self) -> bool;
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

    // [spec:hfst:def:transducer.hfst-ol.transition-index.transition-index-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-index.transition-index-fn]
    pub fn new_istream(is: &mut IStream) -> Self {
        let mut ti = TransitionIndex {
            input_symbol: NO_SYMBOL_NUMBER,
            first_transition_index: 0,
        };
        ti.input_symbol = is.read_u16();
        ti.first_transition_index = is.read_u32();
        ti
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
            if self.is_final() { " (final)" } else { "" }
        );
    }

    // [spec:hfst:def:transducer.hfst-ol.transition-index.matches-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-index.matches-fn]
    pub fn matches(&self, s: SymbolNumber) -> bool {
        self.input_symbol != NO_SYMBOL_NUMBER && self.input_symbol == s
    }
    // [spec:hfst:def:transducer.hfst-ol.transition-index.final-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-index.final-fn]
    pub fn is_final(&self) -> bool {
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
    fn is_final(&self) -> bool {
        self.is_final()
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
    pub fn is_final(&self) -> bool {
        self.base.is_final()
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
    fn is_final(&self) -> bool {
        self.is_final()
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

    pub fn new_final(is_final: bool) -> Self {
        Transition {
            input_symbol: NO_SYMBOL_NUMBER,
            output_symbol: NO_SYMBOL_NUMBER,
            target_index: if is_final { 1 } else { NO_TABLE_INDEX },
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
    pub fn is_final(&self) -> bool {
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
            if self.is_final() { " (final)" } else { "" }
        );
    }

    // [spec:hfst:def:transducer.hfst-ol.transition.write-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition.write-fn]
    pub fn write(&self, os: &mut dyn std::io::Write, weighted: bool) {
        write_u16(self.input_symbol, os);
        write_u16(self.output_symbol, os);
        write_u32(self.target_index, os);
        if weighted {
            // C++ 'os << 0.0f' writes the text representation, i.e. "0".
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
    fn is_final(&self) -> bool {
        self.is_final()
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

    pub fn new_final(is_final: bool, w: Weight) -> Self {
        TransitionW {
            base: Transition::new_final(is_final),
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
    pub fn is_final(&self) -> bool {
        self.base.is_final()
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
            if self.is_final() { " (final)" } else { "" }
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
    fn is_final(&self) -> bool {
        self.is_final()
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
        u32::try_from(self.table.len()).expect("value out of u32 range")
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
    // 'virtual ~TransducerTablesInterface() {}' — the empty virtual destructor
    // exists only so deleting through a base 'Box<dyn TransducerTablesInterface>'
    // runs the concrete type's destructor. Rust does this automatically via the
    // trait object's vtable drop glue, so there is nothing to write.
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.transducer-tables-interface-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.transducer-tables-interface-fn]
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
        self.transition_table.at(i).is_final()
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
        self.index_table.at(i).is_final()
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
    // 'p' is a 0-terminated byte slice positioned at the current char.
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
    // 'p' is an index into 'buf' advanced by reference, mirroring 'char ** p'.
    // 'None' is the C++ NO_SYMBOL_NUMBER "no tokenization found" result.
    pub fn find_key(&self, buf: &[u8], p: &mut usize) -> Option<SymbolNumber> {
        // The leaf table stores NO_SYMBOL_NUMBER for unmapped bytes.
        fn leaf(sym: SymbolNumber) -> Option<SymbolNumber> {
            (sym != NO_SYMBOL_NUMBER).then_some(sym)
        }
        let old_p = *p;
        *p += 1;
        match self.letters[buf[old_p] as usize].as_ref() {
            None => leaf(self.symbols[buf[old_p] as usize]),
            Some(child) => match child.find_key(buf, p) {
                Some(s) => Some(s),
                None => {
                    *p -= 1;
                    leaf(self.symbols[buf[old_p] as usize])
                }
            },
        }
    }
}

impl Drop for OlLetterTrie {
    // [spec:hfst:def:transducer.hfst-ol.ol-letter-trie.ol-letter-trie-fn]
    // [spec:hfst:sem:transducer.hfst-ol.ol-letter-trie.ol-letter-trie-fn]
    fn drop(&mut self) {
        for i in 0..self.letters.len() {
            // 'delete letters[i]; letters[i] = 0;' — dropping the 'Box' frees the
            // child trie (which recursively frees its children) and resetting the
            // slot to 'None' mirrors the null assignment.
            self.letters[i] = None;
        }
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
    pub fn find_key(&self, buf: &[u8], p: &mut usize) -> Option<SymbolNumber> {
        if !should_ascii_tokenize(buf[*p])
            || self.ascii_symbols[buf[*p] as usize] == NO_SYMBOL_NUMBER
        {
            return self.letters.find_key(buf, p);
        }
        let s = self.ascii_symbols[buf[*p] as usize];
        *p += 1;
        Some(s)
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

    pub fn write_pair(&mut self, pos: u32, input: SymbolNumber, out: SymbolNumber) {
        while pos as usize >= self.inner.len() {
            self.inner.push(SymbolPair::new());
        }
        self.inner[pos as usize] = SymbolPair::new_values(input, out);
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
    // The C++ 'write(pos, pair<iterator,iterator>)' over a '[start, end)' slice.
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

// [spec:hfst:def:transducer.hfst-ol.n-byte-utf8-fn]
// [spec:hfst:sem:transducer.hfst-ol.n-byte-utf8-fn]
// [spec:hfst:def:ospell.hfst-ol.n-byte-utf8-fn]
// [spec:hfst:sem:ospell.hfst-ol.n-byte-utf8-fn]
// (declared in transducer.h, defined in ospell.cc — one function, two ids)
pub fn nByte_utf8(c: u8) -> i32 {
    /* utility function to determine how many bytes to peel off as
    a utf-8 character for representing as OTHER */
    if c <= 127 {
        1
    } else if (c & (128 + 64 + 32 + 16)) == (128 + 64 + 32 + 16) {
        4
    } else if (c & (128 + 64 + 32)) == (128 + 64 + 32) {
        3
    } else if (c & (128 + 64)) == (128 + 64) {
        2
    } else {
        0
    }
}

// 'void increment_mutator(void)' on 'TreeNode' (declared in transducer.h:1503)
// is only declared, never defined anywhere in the codebase — effectively dead.
// A faithful port is an empty stub; there is no behavior to replicate. 'TreeNode'
// itself lives in the 'ospell' module; this inherent impl is a same-crate split.
impl crate::ospell::TreeNode {
    // [spec:hfst:def:transducer.hfst-ol.tree-node.increment-mutator-fn]
    // [spec:hfst:sem:transducer.hfst-ol.tree-node.increment-mutator-fn]
    pub fn increment_mutator(&mut self) {}
}

/** \brief A compiled transducer format, suitable for fast lookup operations. */
// [spec:hfst:def:transducer.hfst-ol.transducer]
pub struct Transducer {
    header: Option<Box<TransducerHeader>>,
    alphabet: Option<Box<TransducerAlphabet>>,
    tables: Option<Box<dyn TransducerTablesInterface>>,

    // for lookup
    current_weight: Weight,
    // SAFETY-ISLAND [ol-lookup-paths]: raw aliasing pointer exactly as the C++
    // 'HfstTwoLevelPaths *' — it aliases a function-local result set across the
    // recursive '&mut self' 'get_analyses'/'note_analysis' OL traversal (set in
    // 'lookup_fd'/'lookup_fd_pairs', read at the deref sites below). A safe
    // '&mut HfstTwoLevelPaths' would have to thread through the entire recursive
    // lookup hot path; valid only across the 'get_analyses' call it brackets.
    lookup_paths: *mut HfstTwoLevelPaths,
    encoder: Option<Box<Encoder>>,
    input_tape: Tape,
    output_tape: DoubleTape,
    flag_state: FdState<SymbolNumber>,
    // whether we're going to take a default transition
    traversal_states: TraversalStates,

    max_lookups: isize,
    recursion_depth_left: u32,
    max_time: f64,
    start_clock: Option<Instant>,
}

#[allow(dead_code)]
impl Transducer {
    // ---- small accessors mirroring the C++ member dereferences ----
    fn hdr(&self) -> &TransducerHeader {
        self.header.as_deref().unwrap()
    }
    fn alph(&self) -> &TransducerAlphabet {
        self.alphabet.as_deref().unwrap()
    }
    fn tbl(&self) -> &dyn TransducerTablesInterface {
        self.tables.as_deref().unwrap()
    }

    pub fn new() -> Self {
        Transducer {
            header: None,
            alphabet: None,
            tables: None,
            current_weight: 0.0,
            lookup_paths: std::ptr::null_mut(),
            encoder: None,
            input_tape: Tape::new(),
            output_tape: DoubleTape::new(),
            flag_state: FdState::new_default(),
            traversal_states: TraversalStates::new(),
            max_lookups: -1,
            recursion_depth_left: MAX_RECURSION_DEPTH,
            max_time: 0.0,
            start_clock: None,
        }
    }

    pub fn new_istream(is: &mut IStream) -> crate::error::Result<Self> {
        let header = Box::new(TransducerHeader::new_istream(is)?);
        let alphabet = Box::new(TransducerAlphabet::new_istream(
            is,
            header.symbol_count(),
            true,
        )?);
        let encoder = Box::new(Encoder::new(
            alphabet.get_symbol_table(),
            header.input_symbol_count(),
        ));
        let flag_state = FdState::new(alphabet.get_fd_table());
        let mut t = Transducer {
            header: Some(header),
            alphabet: Some(alphabet),
            tables: None,
            current_weight: 0.0,
            lookup_paths: std::ptr::null_mut(),
            encoder: Some(encoder),
            input_tape: Tape::new(),
            output_tape: DoubleTape::new(),
            flag_state,
            traversal_states: TraversalStates::new(),
            max_lookups: -1,
            recursion_depth_left: MAX_RECURSION_DEPTH,
            max_time: 0.0,
            start_clock: None,
        };
        t.load_tables(is)?;
        Ok(t)
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.transducer-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.transducer-fn]
    pub fn new_weighted(weighted: bool) -> Self {
        let header = Box::new(TransducerHeader::new_weighted(weighted));
        let alphabet = Box::new(TransducerAlphabet::new());
        let encoder = Box::new(Encoder::new(
            alphabet.get_symbol_table(),
            header.input_symbol_count(),
        ));
        let flag_state = FdState::new(alphabet.get_fd_table());
        let tables: Box<dyn TransducerTablesInterface> = if weighted {
            Box::new(TransducerTables::<TransitionWIndex, TransitionW>::new())
        } else {
            Box::new(TransducerTables::<TransitionIndex, Transition>::new())
        };
        Transducer {
            header: Some(header),
            alphabet: Some(alphabet),
            tables: Some(tables),
            current_weight: 0.0,
            lookup_paths: std::ptr::null_mut(),
            encoder: Some(encoder),
            input_tape: Tape::new(),
            output_tape: DoubleTape::new(),
            flag_state,
            traversal_states: TraversalStates::new(),
            max_lookups: -1,
            recursion_depth_left: MAX_RECURSION_DEPTH,
            max_time: 0.0,
            start_clock: None,
        }
    }

    // The C++ builds 'encoder'/'flag_state' from the *parameter* alphabet (dot,
    // not arrow), so they reference the caller's alphabet; replicated here.
    pub fn new_from_tables_unweighted(
        header: &TransducerHeader,
        alphabet: &TransducerAlphabet,
        index_table: TransducerTable<TransitionIndex>,
        transition_table: TransducerTable<Transition>,
    ) -> Self {
        let header_box = Box::new(header.clone());
        let alphabet_box = Box::new(alphabet.clone());
        let tables: Box<dyn TransducerTablesInterface> =
            Box::new(TransducerTables::<TransitionIndex, Transition>::new_tables(
                index_table,
                transition_table,
            ));
        let encoder = Box::new(Encoder::new(
            alphabet.get_symbol_table(),
            header.input_symbol_count(),
        ));
        let flag_state = FdState::new(alphabet.get_fd_table());
        Transducer {
            header: Some(header_box),
            alphabet: Some(alphabet_box),
            tables: Some(tables),
            current_weight: 0.0,
            lookup_paths: std::ptr::null_mut(),
            encoder: Some(encoder),
            input_tape: Tape::new(),
            output_tape: DoubleTape::new(),
            flag_state,
            traversal_states: TraversalStates::new(),
            max_lookups: -1,
            recursion_depth_left: MAX_RECURSION_DEPTH,
            max_time: 0.0,
            start_clock: None,
        }
    }

    pub fn new_from_tables_weighted(
        header: &TransducerHeader,
        alphabet: &TransducerAlphabet,
        index_table: TransducerTable<TransitionWIndex>,
        transition_table: TransducerTable<TransitionW>,
    ) -> Self {
        let header_box = Box::new(header.clone());
        let alphabet_box = Box::new(alphabet.clone());
        let tables: Box<dyn TransducerTablesInterface> = Box::new(TransducerTables::<
            TransitionWIndex,
            TransitionW,
        >::new_tables(
            index_table, transition_table
        ));
        let encoder = Box::new(Encoder::new(
            alphabet.get_symbol_table(),
            header.input_symbol_count(),
        ));
        let flag_state = FdState::new(alphabet.get_fd_table());
        Transducer {
            header: Some(header_box),
            alphabet: Some(alphabet_box),
            tables: Some(tables),
            current_weight: 0.0,
            lookup_paths: std::ptr::null_mut(),
            encoder: Some(encoder),
            input_tape: Tape::new(),
            output_tape: DoubleTape::new(),
            flag_state,
            traversal_states: TraversalStates::new(),
            max_lookups: -1,
            recursion_depth_left: MAX_RECURSION_DEPTH,
            max_time: 0.0,
            start_clock: None,
        }
    }

    pub fn get_header(&self) -> &TransducerHeader {
        self.hdr()
    }
    pub fn get_alphabet(&self) -> &TransducerAlphabet {
        self.alph()
    }
    pub fn get_encoder(&self) -> &Encoder {
        self.encoder.as_deref().unwrap()
    }
    pub fn get_fd_table(&self) -> &FdTable<SymbolNumber> {
        self.alph().get_fd_table()
    }
    pub fn get_symbol_table(&self) -> &SymbolTable {
        self.alph().get_symbol_table()
    }

    pub fn get_index(&self, i: TransitionTableIndex) -> &dyn IndexEntry {
        self.tbl().get_index(i)
    }
    pub fn get_transition(&self, i: TransitionTableIndex) -> &dyn TransitionEntry {
        self.tbl().get_transition(i)
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.final-index-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.final-index-fn]
    pub fn final_index(&self, i: TransitionTableIndex) -> bool {
        if indexes_transition_table(i) {
            self.tbl().get_transition_finality(i)
        } else {
            self.tbl().get_index_finality(i)
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.is-infinitely-ambiguous-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.is-infinitely-ambiguous-fn]
    pub fn is_infinitely_ambiguous(&self) -> bool {
        self.hdr().probe_flag(HeaderFlag::Has_input_epsilon_cycles)
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.is-lookup-infinitely-ambiguous-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.is-lookup-infinitely-ambiguous-fn]
    pub fn is_lookup_infinitely_ambiguous_str(&mut self, s: &str) -> bool {
        if !self.initialize_input(s) {
            return false;
        }
        self.traversal_states.clear();
        // C++: try { find_loop(0, 0); } catch (bool e) { ... return e; }
        match self.find_loop(0, 0) {
            ControlFlow::Continue(_) => false,
            ControlFlow::Break(()) => {
                self.current_weight = 0.0;
                let fs = FdState::new(self.alph().get_fd_table());
                self.flag_state = fs;
                true
            }
        }
    }

    pub fn is_lookup_infinitely_ambiguous_strvec(&mut self, s: &StringVector) -> bool {
        let mut input_str = String::new();
        for it in s.iter() {
            input_str.push_str(it);
        }
        self.is_lookup_infinitely_ambiguous_str(&input_str)
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.copy-windex-table-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.copy-windex-table-fn]
    pub fn copy_windex_table(&self) -> crate::error::Result<TransducerTable<TransitionWIndex>> {
        if !self.hdr().probe_flag(HeaderFlag::Weighted) {
            crate::bail!(TransducerHasWrongType);
        }
        let mut another = TransducerTable::new();
        for i in 0..self.hdr().index_table_size() {
            another.append(TransitionWIndex::new_values(
                self.tbl().get_index_input(i),
                self.tbl().get_index_target(i),
            ));
        }
        Ok(another)
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer.copy-transitionw-table-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.copy-transitionw-table-fn]
    pub fn copy_transitionw_table(&self) -> crate::error::Result<TransducerTable<TransitionW>> {
        if !self.hdr().probe_flag(HeaderFlag::Weighted) {
            crate::bail!(TransducerHasWrongType);
        }
        let mut another = TransducerTable::new();
        for i in 0..self.hdr().target_table_size() {
            another.append(TransitionW::new_values(
                self.tbl().get_transition_input(i),
                self.tbl().get_transition_output(i),
                self.tbl().get_transition_target(i),
                self.tbl().get_weight(i),
            ));
        }
        Ok(another)
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer.copy-index-table-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.copy-index-table-fn]
    pub fn copy_index_table(&self) -> crate::error::Result<TransducerTable<TransitionIndex>> {
        if self.hdr().probe_flag(HeaderFlag::Weighted) {
            crate::bail!(TransducerHasWrongType);
        }
        let mut another = TransducerTable::new();
        for i in 0..self.hdr().index_table_size() {
            // tables->get_index(i) returns a base TransitionIndex; copy its data
            let idx = self.tbl().get_index(i);
            another.append(TransitionIndex::new_values(
                idx.get_input_symbol(),
                idx.get_target(),
            ));
        }
        Ok(another)
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer.copy-transition-table-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.copy-transition-table-fn]
    pub fn copy_transition_table(&self) -> crate::error::Result<TransducerTable<Transition>> {
        if self.hdr().probe_flag(HeaderFlag::Weighted) {
            crate::bail!(TransducerHasWrongType);
        }
        let mut another = TransducerTable::new();
        for i in 0..self.hdr().target_table_size() {
            let tr = self.tbl().get_transition(i);
            another.append(Transition::new_values(
                tr.get_input_symbol(),
                tr.get_output_symbol(),
                tr.get_target(),
            ));
        }
        Ok(another)
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.load-tables-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.load-tables-fn]
    pub fn load_tables(&mut self, is: &mut IStream) -> crate::error::Result<()> {
        let weighted = self.hdr().probe_flag(HeaderFlag::Weighted);
        let its = self.hdr().index_table_size();
        let tts = self.hdr().target_table_size();
        if weighted {
            self.tables = Some(Box::new(
                TransducerTables::<TransitionWIndex, TransitionW>::new_istream(is, its, tts),
            ));
        } else {
            self.tables = Some(Box::new(
                TransducerTables::<TransitionIndex, Transition>::new_istream(is, its, tts),
            ));
        }
        if !is.good() {
            crate::bail!(TransducerHasWrongType);
        }
        Ok(())
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.write-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.write-fn]
    pub fn write(&self, os: &mut dyn std::io::Write) {
        self.hdr().write(os);
        self.alph().write(os);
        let weighted = self.hdr().probe_flag(HeaderFlag::Weighted);
        for i in 0..self.hdr().index_table_size() {
            self.tbl()
                .get_index(u32::try_from(i as usize).expect("value out of u32 range"))
                .write(os, weighted);
        }
        for i in 0..self.hdr().target_table_size() {
            self.tbl()
                .get_transition(u32::try_from(i as usize).expect("value out of u32 range"))
                .write(os, weighted);
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.copy-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.copy-fn]
    pub fn copy(t: &Transducer, weighted: bool) -> crate::error::Result<Transducer> {
        if weighted {
            Ok(Transducer::new_from_tables_weighted(
                t.get_header(),
                t.get_alphabet(),
                t.copy_windex_table()?,
                t.copy_transitionw_table()?,
            ))
        } else {
            Ok(Transducer::new_from_tables_unweighted(
                t.get_header(),
                t.get_alphabet(),
                t.copy_index_table()?,
                t.copy_transition_table()?,
            ))
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.display-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.display-fn]
    pub fn display(&self) {
        println!("-----Displaying optimized-lookup transducer------");
        self.hdr().display();
        self.alph().display();
        self.tbl().display();
        println!("-------------------------------------------------");
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.get-transitions-from-state-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.get-transitions-from-state-fn]
    pub fn get_transitions_from_state(
        &self,
        state_index: TransitionTableIndex,
    ) -> TransitionTableIndexSet {
        let mut transitions = TransitionTableIndexSet::new();

        if indexes_transition_index_table(state_index) {
            // for each input symbol that has a transition from this state
            for symbol in 0..self.hdr().symbol_count() {
                // There may be flags at index 0 even if there aren't any
                // epsilons, so those have to be checked for
                if self.alph().is_like_epsilon(symbol) {
                    let mut transition_i = self.get_index(state_index + 1).get_target();
                    if !self.get_index(state_index + 1).matches(0) {
                        continue;
                    }
                    loop {
                        let input = self.get_transition(transition_i).get_input_symbol();
                        if self.get_transition(transition_i).matches(symbol) {
                            transitions.insert(transition_i);
                        // There could still be epsilons here, or other flags
                        } else if input != 0 && !self.alph().is_like_epsilon(input) {
                            break;
                        }
                        transition_i += 1;
                    }
                } else {
                    // not a flag.
                    // The C++ reads get_index(state_index+1+symbol) unconditionally;
                    // for output-only symbols (whose number can reach symbol_count)
                    // this indexes past the index table — a benign out-of-bounds read
                    // in C++ that yields a non-matching entry. Guard it to the
                    // intended "no entry beyond the table => no transitions" semantics.
                    if state_index + 1 + symbol as u32 >= self.hdr().index_table_size() {
                        continue;
                    }
                    let test_input = self
                        .get_index(state_index + 1 + symbol as u32)
                        .get_input_symbol();
                    let test_target = self.get_index(state_index + 1 + symbol as u32).get_target();
                    if self
                        .get_index(state_index + 1 + symbol as u32)
                        .matches(symbol)
                    {
                        // there are one or more transitions with this input
                        // symbol, starting at test_transition_index.get_target()
                        let mut transition_i = test_target;
                        loop {
                            if self.get_transition(transition_i).matches(test_input) {
                                transitions.insert(transition_i);
                            } else {
                                break;
                            }
                            transition_i += 1;
                        }
                    }
                }
            }
        } else {
            // indexes transition table
            let in_sym = self.get_transition(state_index).get_input_symbol();
            let out_sym = self.get_transition(state_index).get_output_symbol();
            if in_sym != NO_SYMBOL_NUMBER || out_sym != NO_SYMBOL_NUMBER {
                // Oops
                panic!("get_transitions_from_state: malformed transition boundary");
            }

            let mut transition_i = state_index + 1;
            loop {
                if self.get_transition(transition_i).get_input_symbol() != NO_SYMBOL_NUMBER {
                    transitions.insert(transition_i);
                } else {
                    break;
                }
                transition_i += 1;
            }
        }
        transitions
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.next-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.next-fn]
    pub fn next(&self, i: TransitionTableIndex, symbol: SymbolNumber) -> TransitionTableIndex {
        if i >= TRANSITION_TARGET_TABLE_START {
            i - TRANSITION_TARGET_TABLE_START + 1
        } else {
            self.get_index(i + 1 + symbol as u32).get_target() - TRANSITION_TARGET_TABLE_START
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.next-e-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.next-e-fn]
    // (declared in transducer.h; defined in pmatch.cc — ported with pmatch.)

    // [spec:hfst:def:transducer.hfst-ol.transducer.has-transitions-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.has-transitions-fn]
    pub fn has_transitions(&self, i: TransitionTableIndex, symbol: SymbolNumber) -> bool {
        if i >= TRANSITION_TARGET_TABLE_START {
            self.get_transition(i - TRANSITION_TARGET_TABLE_START)
                .get_input_symbol()
                == symbol
        } else {
            self.get_index(i + symbol as u32).get_input_symbol() == symbol
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.has-epsilons-or-flags-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.has-epsilons-or-flags-fn]
    pub fn has_epsilons_or_flags(&self, i: TransitionTableIndex) -> bool {
        if i >= TRANSITION_TARGET_TABLE_START {
            let input = self
                .get_transition(i - TRANSITION_TARGET_TABLE_START)
                .get_input_symbol();
            input == 0 || self.is_flag(input)
        } else {
            self.get_index(i).get_input_symbol() == 0
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.take-epsilons-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.take-epsilons-fn]
    pub fn take_epsilons(&self, i: TransitionTableIndex) -> STransition {
        if self.get_transition(i).get_input_symbol() != 0 {
            return STransition::new(0, NO_SYMBOL_NUMBER);
        }
        STransition::new_weighted(
            self.get_transition(i).get_target(),
            self.get_transition(i).get_output_symbol(),
            self.get_transition(i).get_weight(),
        )
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.take-epsilons-and-flags-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.take-epsilons-and-flags-fn]
    pub fn take_epsilons_and_flags(&self, i: TransitionTableIndex) -> STransition {
        if self.get_transition(i).get_input_symbol() != 0
            && !self.is_flag(self.get_transition(i).get_input_symbol())
        {
            return STransition::new(0, NO_SYMBOL_NUMBER);
        }
        STransition::new_weighted(
            self.get_transition(i).get_target(),
            self.get_transition(i).get_output_symbol(),
            self.get_transition(i).get_weight(),
        )
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.take-non-epsilons-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.take-non-epsilons-fn]
    pub fn take_non_epsilons(&self, i: TransitionTableIndex, symbol: SymbolNumber) -> STransition {
        if self.get_transition(i).get_input_symbol() != symbol {
            return STransition::new(0, NO_SYMBOL_NUMBER);
        }
        STransition::new_weighted(
            self.get_transition(i).get_target(),
            self.get_transition(i).get_output_symbol(),
            self.get_transition(i).get_weight(),
        )
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.final-weight-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.final-weight-fn]
    pub fn final_weight(&self, i: TransitionTableIndex) -> Weight {
        if i >= TRANSITION_TARGET_TABLE_START {
            self.get_transition(i - TRANSITION_TARGET_TABLE_START)
                .get_weight()
        } else {
            self.get_index(i).final_weight()
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.is-flag-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.is-flag-fn]
    pub fn is_flag(&self, symbol: SymbolNumber) -> bool {
        self.alph().is_flag_diacritic(symbol)
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer.is-weighted-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.is-weighted-fn]
    pub fn is_weighted(&self) -> bool {
        self.hdr().probe_flag(HeaderFlag::Weighted)
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.get-unknown-symbol-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.get-unknown-symbol-fn]
    pub fn get_unknown_symbol(&self) -> SymbolNumber {
        self.alph().get_unknown_symbol()
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer.get-string-symbol-map-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.get-string-symbol-map-fn]
    pub fn get_string_symbol_map(&self) -> StringSymbolMap {
        self.alph().build_string_symbol_map()
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.initialize-input-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.initialize-input-fn]
    pub fn initialize_input(&mut self, input: &str) -> bool {
        let mut buf: Vec<u8> = input.as_bytes().to_vec();
        buf.push(0);
        let mut i: u32 = 0;
        let mut p: usize = 0;
        while buf[p] != 0 {
            let original_input_loc = p;
            let k = match self
                .encoder
                .as_ref()
                .expect("encoder is initialized during transducer load")
                .find_key(&buf, &mut p)
            {
                Some(k) => k,
                None => {
                    // Add what we assume to be an unknown utf-8 symbol to the alphabet
                    p = original_input_loc;
                    let bytes_to_tokenize = nByte_utf8(buf[p]);
                    if bytes_to_tokenize == 0 {
                        return false; // tokenization failed
                    }
                    let new_symbol =
                        String::from_utf8_lossy(&buf[p..p + bytes_to_tokenize as usize])
                            .into_owned();
                    p += bytes_to_tokenize as usize;
                    self.alphabet
                        .as_mut()
                        .expect("alphabet is initialized during transducer load")
                        .add_symbol(&new_symbol);
                    let k = u32::try_from(self.alph().get_symbol_table().len() - 1)
                        .expect("value out of u32 range")
                        as SymbolNumber;
                    self.encoder
                        .as_mut()
                        .expect("encoder is initialized during transducer load")
                        .read_input_symbol(&new_symbol, k as i32);
                    k
                }
            };
            self.input_tape.write(i, k);
            i += 1;
        }
        self.input_tape.write(i, NO_SYMBOL_NUMBER);
        true
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.include-symbol-in-alphabet-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.include-symbol-in-alphabet-fn]
    pub fn include_symbol_in_alphabet(&mut self, sym: &str) {
        if self.alph().symbol_from_string(sym).is_some() {
            return;
        }
        let key = u32::try_from(self.alph().get_symbol_table().len())
            .expect("value out of u32 range") as SymbolNumber;
        self.alphabet.as_mut().unwrap().add_symbol_str(sym);
        self.encoder
            .as_mut()
            .unwrap()
            .read_input_symbol(sym, key as i32);
    }

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
        self.lookup_fd_str(&input_str, limit, time_cutoff)
    }

    pub fn lookup_fd_str(&mut self, s: &str, limit: isize, time_cutoff: f64) -> HfstOneLevelPaths {
        self.lookup_fd_cstr(s, limit, time_cutoff)
    }

    pub fn lookup_fd_pairs_str(
        &mut self,
        s: &str,
        limit: isize,
        time_cutoff: f64,
    ) -> HfstTwoLevelPaths {
        self.lookup_fd_pairs_cstr(s, limit, time_cutoff)
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.lookup-fd-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.lookup-fd-fn]
    pub fn lookup_fd_cstr(&mut self, s: &str, limit: isize, time_cutoff: f64) -> HfstOneLevelPaths {
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
        let mut paths: Box<HfstTwoLevelPaths> = Box::new(BTreeSet::new());
        self.lookup_paths = paths.as_mut() as *mut HfstTwoLevelPaths;
        self.traversal_states.clear();
        self.get_analyses(0, 0, 0);
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
        self.lookup_paths = std::ptr::null_mut();
        results
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.lookup-fd-pairs-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.lookup-fd-pairs-fn]
    pub fn lookup_fd_pairs_cstr(
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
        let mut results: Box<HfstTwoLevelPaths> = Box::new(BTreeSet::new());
        self.lookup_paths = results.as_mut() as *mut HfstTwoLevelPaths;
        if !self.initialize_input(s) {
            self.lookup_paths = std::ptr::null_mut();
            return *results;
        }
        self.traversal_states.clear();
        self.get_analyses(0, 0, 0);
        self.lookup_paths = std::ptr::null_mut();
        *results
    }

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
            let input = self.tbl().get_transition_input(i);
            let output = self.tbl().get_transition_output(i);
            let target = self.tbl().get_transition_target(i);
            let weight = self.tbl().get_weight(i);
            let old_weight = self.current_weight;
            if input == 0 {
                // epsilon
                self.output_tape.write_pair(output_pos, input, output);
                self.current_weight += weight;
                self.get_analyses(input_pos, output_pos + 1, target);
                found_transition = true;
                self.current_weight = old_weight;
                i += 1;
            } else if self.alph().is_flag_diacritic(input) {
                let flags = self.flag_state.get_values().clone();
                let op = self.alph().get_operation(input).unwrap().clone();
                if self.flag_state.apply_operation(&op) {
                    // flag diacritic allowed
                    let flag_reachable = TraversalState::new(target, flags.clone());
                    if self.traversal_states.contains(&flag_reachable) {
                        // We've been here before at this input, back out
                        self.flag_state.assign_values(&flags);
                        i += 1;
                        continue;
                    }
                    self.traversal_states.insert(flag_reachable.clone());
                    self.output_tape.write_pair(output_pos, input, output);
                    self.current_weight += weight;
                    self.get_analyses(input_pos, output_pos + 1, target);
                    found_transition = true;
                    self.current_weight = old_weight;
                    self.traversal_states.remove(&flag_reachable);
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
        if self.tbl().get_index_input(i) == 0 {
            let target = self.tbl().get_index_target(i) - TRANSITION_TARGET_TABLE_START;
            self.try_epsilon_transitions(input_pos, output_pos, target);
            true
        } else {
            false
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.find-transitions-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.find-transitions-fn]
    fn find_transitions(
        &mut self,
        input: SymbolNumber,
        input_pos: u32,
        output_pos: u32,
        mut i: TransitionTableIndex,
    ) -> bool {
        let mut found_transition = false;
        while self.tbl().get_transition_input(i) != NO_SYMBOL_NUMBER {
            if self.tbl().get_transition_input(i) == input {
                let old_weight = self.current_weight;
                // We're not going to find an epsilon / flag loop
                self.traversal_states.clear();
                let mut output = self.tbl().get_transition_output(i);
                if self.alph().is_meta_arc(output) {
                    // we got here via default, identity or unknown, so look back
                    // in the input tape to find the symbol we want to write
                    output = self.input_tape.at(input_pos - 1);
                }
                self.output_tape.write_pair(output_pos, input, output);
                let w = self.tbl().get_weight(i);
                self.current_weight += w;
                let target = self.tbl().get_transition_target(i);
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
        if self.tbl().get_index_input(i + input as u32) == input {
            let target =
                self.tbl().get_index_target(i + input as u32) - TRANSITION_TARGET_TABLE_START;
            self.find_transitions(input, input_pos, output_pos, target);
            true
        } else {
            false
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.get-analyses-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.get-analyses-fn]
    fn get_analyses(&mut self, input_pos: u32, output_pos: u32, mut i: TransitionTableIndex) {
        let mut found_transition = false;

        if self.recursion_depth_left == 0 {
            return;
        }
        if self.max_lookups >= 0
            && unsafe { (*self.lookup_paths).len() } as isize >= self.max_lookups
        {
            // Back out because we have enough results already
            return;
        }
        if self.max_time > 0.0 {
            // quit if we've overspent our time
            if let Some(sc) = self.start_clock {
                if sc.elapsed().as_secs_f64() > self.max_time {
                    return;
                }
            }
        }
        self.recursion_depth_left -= 1;
        if indexes_transition_table(i) {
            i -= TRANSITION_TARGET_TABLE_START;
            // First we check for finality and collect the result
            if self.input_tape.at(input_pos) == NO_SYMBOL_NUMBER {
                if self.max_lookups < 0
                    || (unsafe { (*self.lookup_paths).len() } as isize) < self.max_lookups
                {
                    self.output_tape
                        .write_pair(output_pos, NO_SYMBOL_NUMBER, NO_SYMBOL_NUMBER);
                    if self.tbl().get_transition_finality(i) {
                        let old_weight = self.current_weight;
                        let w = self.tbl().get_weight(i);
                        self.current_weight += w;
                        self.note_analysis();
                        self.current_weight = old_weight;
                    }
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
            if self.input_tape.at(input_pos) == NO_SYMBOL_NUMBER {
                if self.max_lookups < 0
                    || (unsafe { (*self.lookup_paths).len() } as isize) < self.max_lookups
                {
                    self.output_tape
                        .write_pair(output_pos, NO_SYMBOL_NUMBER, NO_SYMBOL_NUMBER);
                    if self.tbl().get_index_finality(i) {
                        let old_weight = self.current_weight;
                        let w = self.tbl().get_final_weight(i);
                        self.current_weight += w;
                        self.note_analysis();
                        self.current_weight = old_weight;
                    }
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
            let in_s = self.alph().string_from_symbol(pair.input);
            let out_s = self.alph().string_from_symbol(pair.output);
            result.second.push((in_s, out_s));
            idx += 1;
        }
        result.first = self.current_weight;
        unsafe {
            (*self.lookup_paths).insert(result);
        }
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
            let target = self.tbl().get_transition_target(i);
            let epsilon_reachable = TraversalState::new(target, flags.clone());
            let tin = self.tbl().get_transition_input(i);
            if tin == 0 {
                // epsilon
                // We try to trap non-progressing loops
                if self.traversal_states.contains(&epsilon_reachable) {
                    // We've been here before
                    return ControlFlow::Break(());
                }
                self.traversal_states.insert(epsilon_reachable.clone());
                self.find_loop(input_pos, target)?;
                self.traversal_states.remove(&epsilon_reachable);
                found_transition = true;
                i += 1;
            } else if self.alph().is_flag_diacritic(tin) {
                let op = self.alph().get_operation(tin).unwrap().clone();
                if self.flag_state.apply_operation(&op) {
                    // flag diacritic allowed
                    if self.traversal_states.contains(&epsilon_reachable) {
                        // We've been here before
                        return ControlFlow::Break(());
                    }
                    self.traversal_states.insert(epsilon_reachable.clone());
                    // C++ leak preserved: the shared field took the nested
                    // call's exit value here (no unconditional set like the
                    // epsilon arm), so this REPLACES the accumulator.
                    found_transition = self.find_loop(input_pos, target)?;
                    self.traversal_states.remove(&epsilon_reachable);
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
        if self.tbl().get_index_input(i) == 0 {
            let target = self.tbl().get_index_target(i) - TRANSITION_TARGET_TABLE_START;
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
        while self.tbl().get_transition_input(i) != NO_SYMBOL_NUMBER {
            if self.tbl().get_transition_input(i) == input {
                // We're not going to find an epsilon / flag loop
                self.traversal_states.clear();
                let target = self.tbl().get_transition_target(i);
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
        if self.tbl().get_index_input(i + input as u32) == input {
            let target =
                self.tbl().get_index_target(i + input as u32) - TRANSITION_TARGET_TABLE_START;
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

impl Default for Transducer {
    fn default() -> Self {
        Self::new()
    }
}
