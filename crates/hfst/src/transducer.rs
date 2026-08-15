//! Port of 'libhfst/src/implementations/optimized-lookup/transducer.{h,cc}'
//! (+ 'find_epsilon_loops.cc'), namespace 'hfst_ol' — the compiled
//! optimized-lookup transducer format and its lookup engine.
//!
//! Binary I/O fidelity: the C++ reads/writes raw struct bytes via
//! 'is.read(reinterpret_cast<char*>(&p), sizeof(T))' (host-endian). This port
//! diverges deliberately (hfst/hfst#328): the optimized-lookup format is read
//! and written LITTLE-ENDIAN ('from_le_bytes'/'to_le_bytes') so the on-disk
//! bytes are portable and deterministic across targets. This is byte-identical
//! to the old native-endian path on the little-endian hosts everyone actually
//! ships on (x86-64/aarch64), and matches the sibling THFST format, which is
//! already explicitly little-endian. 'std::istream' is modelled by ['IStream'],
//! a thin wrapper over '&mut dyn Read' that tracks a fail flag so the C++
//! 'if(!is)' checks port directly; 'std::ostream' becomes '&mut dyn Write'.
//!
//! C++ value-type inheritance (the concrete base 'TransitionIndex' with a
//! derived 'TransitionWIndex' overriding 'final_weight', likewise
//! 'Transition'/'TransitionW') is modelled as a base struct plus a trait that
//! captures the virtual methods, since the tables hand them out through base
//! references. The pure-abstract 'TransducerTablesInterface' becomes a trait
//! object.

use smallvec::SmallVec;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::ControlFlow;
use std::time::Instant;

use crate::hfst_data_types::{
    HfstOneLevelPath, HfstOneLevelPaths, HfstTwoLevelPath, HfstTwoLevelPaths, StringVector, Symbol,
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
pub type SymbolTable = Vec<Symbol>;

// for lookup
// [spec:hfst:def:transducer.hfst-ol.string-pair]
pub type StringPair = (Symbol, Symbol);

// for ospell
// [spec:hfst:def:transducer.hfst-ol.flag-diacritic-state]
pub type FlagDiacriticState = Vec<i16>;
// The epsilon-loop guard keys on a snapshot of the flag-diacritic state. A
// transducer has a small, fixed number of flag features (typically well under
// 32), so this snapshot lives inline on the stack; the guard is probed/pushed
// once per epsilon or flag arc, and inline storage keeps that hot path free of
// per-arc heap traffic (spilling to the heap only for unusually large flag
// sets, where it stays correct). See the note on `TraversalStates`.
pub type GuardFlags = SmallVec<[i16; 32]>;
// [spec:hfst:def:transducer.hfst-ol.operation-map]
pub type OperationMap = BTreeMap<SymbolNumber, FdOperation>;
// [spec:hfst:def:transducer.hfst-ol.string-symbol-map]
pub type StringSymbolMap = BTreeMap<Symbol, SymbolNumber>;

// for epsilon loop checking
// [spec:hfst:def:transducer.hfst-ol.traversal-state]
#[derive(Clone)]
pub struct TraversalState {
    pub index: TransitionTableIndex,
    pub flags: GuardFlags,
}

impl TraversalState {
    // [spec:hfst:def:transducer.hfst-ol.traversal-state.traversal-state-fn]
    // [spec:hfst:sem:transducer.hfst-ol.traversal-state.traversal-state-fn]
    // Copies the flag snapshot into inline storage; the caller passes a borrow
    // of the live flag state, so no owning `Vec` is allocated per arc.
    pub fn new(i: TransitionTableIndex, f: &[i16]) -> Self {
        TraversalState {
            index: i,
            flags: SmallVec::from_slice(f),
        }
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
// The C++ used a std::set, but this guard is used strictly as a DFS stack:
// every insert (on entering an epsilon/flag arc) is matched by a remove after
// the recursive descent returns, and it is cleared whenever a real input symbol
// is consumed — so the live contents are exactly the states on the current
// recursion path (always distinct, since a repeat is trapped by `contains`
// before it can be pushed). A `Vec` used as that stack is membership-identical
// but allocation-free in steady state (capacity is reused across the whole
// lookup), whereas the old `BTreeSet` allocated/rebalanced a tree node per arc.
// Profiling a real morphological analyzer showed that per-arc set churn (plus
// the allocator traffic it drove) was >70% of lookup CPU; this removes it.
pub type TraversalStates = Vec<TraversalState>;

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

// Hard ceiling on the TOTAL number of `get_analyses` node visits per lookup.
//
// `recursion_depth_left` (MAX_RECURSION_DEPTH) bounds the DEPTH of a single
// path — it is restored on backtrack — so on an epsilon cycle the C++ engine
// (and this 1:1 port) still recurses 5000 levels deep, producing 5000 junk
// analyses whose weights climb to ~4999 before the depth cap finally trips
// (hfst/hfst#293's "huge weights before printing infinities") and holding all
// of that in memory / running the machine flat out even when `--time-cutoff`
// has bypassed the infinite-ambiguity guard (hfst/hfst#476's "memory hole /
// time-cutoff ineffective"). This counter, by contrast, is NEVER restored on
// backtrack, so it bounds total WORK regardless of cycle shape and terminates
// deterministically on any pathological FST without depending on wall-clock or
// on the infinite-ambiguity pre-check. The ceiling is high enough that no
// legitimate lookup reaches it (a well-formed acyclic path visits far fewer
// nodes than this) but low enough that a runaway cycle stops promptly.
pub const MAX_NODE_VISITS: u64 = 1_000_000;

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
    // the last pushed byte is the next one returned by get()/read()/read_until().
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
        // First drain the put-back get-area (LIFO), as the byte-at-a-time loop
        // did — the last pushed byte is returned first.
        while got < buf.len() {
            if let Some(b) = self.putback.pop() {
                buf[got] = b;
                got += 1;
            } else {
                break;
            }
        }
        // Then bulk-read the remainder straight into 'buf', looping only to
        // service short reads. (This used to read ONE BYTE PER 'inner.read()'
        // call, which turned loading a 565MB pmatch/OL table into ~half a
        // billion syscalls — a multi-minute hang. Filling the caller's buffer
        // directly reads exactly buf.len() bytes with no read-ahead, so the
        // multi-transducer stream framing that reborrows the reader stays
        // correct.)
        while got < buf.len() {
            match self.inner.read(&mut buf[got..]) {
                Ok(0) => {
                    self.eof = true;
                    self.fail = true;
                    return;
                }
                Ok(n) => got += n,
                Err(_) => {
                    self.fail = true;
                    return;
                }
            }
        }
    }

    /// 'std::getline(is, str, delim)': collect bytes up to (not including)
    /// 'delim'; an immediate EOF with no bytes sets the fail flag.
    pub fn read_until(&mut self, delim: u8) -> String {
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
    // properties — the typed mirror of the static template 'read_property<T>'
    // that 'TransducerHeader' uses to read its fields. Little-endian per
    // hfst/hfst#328 (see module docs).
    // [spec:hfst:def:transducer.hfst-ol.transducer-header.read-property-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.read-property-fn]
    fn read_u16(&mut self) -> u16 {
        let mut b = [0u8; 2];
        self.read(&mut b);
        u16::from_le_bytes(b)
    }
    fn read_u32(&mut self) -> u32 {
        let mut b = [0u8; 4];
        self.read(&mut b);
        u32::from_le_bytes(b)
    }
}

// 'os.write(reinterpret_cast<const char*>(&prop), sizeof(prop))' for the
// integer/float properties. Little-endian per hfst/hfst#328 (see module docs).
// [spec:hfst:def:transducer.hfst-ol.transducer-header.write-property-fn]
// [spec:hfst:sem:transducer.hfst-ol.transducer-header.write-property-fn]
fn write_u16(prop: u16, os: &mut dyn std::io::Write) {
    let _ = os.write_all(&prop.to_le_bytes());
}
fn write_u32(prop: u32, os: &mut dyn std::io::Write) {
    let _ = os.write_all(&prop.to_le_bytes());
}
// [spec:hfst:def:transducer.hfst-ol.transducer-header.write-bool-property-fn]
// [spec:hfst:sem:transducer.hfst-ol.transducer-header.write-bool-property-fn]
fn write_bool_property(value: bool, os: &mut dyn std::io::Write) {
    let prop: u32 = if value { 1 } else { 0 };
    let _ = os.write_all(&prop.to_le_bytes());
}

/// The optimized-lookup on-disk format stores the transition-index and
/// transition-target table sizes as [`TransitionTableIndex`] (u32), so a table
/// longer than u32::MAX cannot be represented; report that as a clean error
/// instead of panicking on the narrowing conversion. This mirrors
/// `ol_symbol_number` for the u16 symbol ceiling and is effectively unreachable
/// on real data — the goal is a clear diagnostic in place of a silent wrap or a
/// panic [hfst/hfst#123].
#[inline]
pub fn ol_table_size(table_len: usize) -> crate::error::Result<TransitionTableIndex> {
    u32::try_from(table_len).map_err(|_| {
        crate::err!(
            Hfst,
            "optimized-lookup format: transducer table has more than 2^32-1 entries (u32 table-size limit)"
        )
    })
}

/// Structural check on one index-table entry just read from a stream.
///
/// Both tables come off disk as raw little-endian records, and everything
/// downstream trusts them: a symbol number indexes the alphabet and the
/// per-symbol vectors built alongside it, a target indexes one of the two
/// tables. A corrupt or truncated file therefore surfaces as an out-of-bounds
/// read deep inside the lookup engine, far from anything that could still name
/// the file. Check the invariants once, at the read boundary, and report a real
/// error instead.
///
/// A slot whose input symbol is `NO_SYMBOL_NUMBER` is blank padding or a
/// finality marker; its target field then carries a flag or a packed final
/// weight rather than a table position, so it is left alone.
pub(crate) fn validate_ol_index_entry(
    position: usize,
    input_symbol: SymbolNumber,
    target: TransitionTableIndex,
    symbol_count: usize,
    transition_len: usize,
) -> crate::error::Result<()> {
    if input_symbol == NO_SYMBOL_NUMBER {
        return Ok(());
    }
    if input_symbol as usize >= symbol_count {
        crate::bail!(
            Hfst,
            format!(
                "optimized-lookup transducer is corrupt: index entry {position} has input symbol {input_symbol}, but the alphabet holds {symbol_count} symbols"
            )
        );
    }
    if target < TRANSITION_TARGET_TABLE_START
        || (target - TRANSITION_TARGET_TABLE_START) as usize >= transition_len
    {
        crate::bail!(
            Hfst,
            format!(
                "optimized-lookup transducer is corrupt: index entry {position} targets {target}, which is not one of the {transition_len} transition-table entries"
            )
        );
    }
    Ok(())
}

/// Structural check on one transition-table entry just read from a stream; see
/// [`validate_ol_index_entry`]. A transition target may address either table,
/// so both ranges are accepted.
pub(crate) fn validate_ol_transition_entry(
    position: usize,
    input_symbol: SymbolNumber,
    output_symbol: SymbolNumber,
    target: TransitionTableIndex,
    symbol_count: usize,
    index_len: usize,
    transition_len: usize,
) -> crate::error::Result<()> {
    if input_symbol == NO_SYMBOL_NUMBER {
        return Ok(());
    }
    let bad_symbol = [input_symbol, output_symbol]
        .into_iter()
        .find(|s| *s != NO_SYMBOL_NUMBER && *s as usize >= symbol_count);
    if let Some(symbol) = bad_symbol {
        crate::bail!(
            Hfst,
            format!(
                "optimized-lookup transducer is corrupt: transition {position} uses symbol {symbol}, but the alphabet holds {symbol_count} symbols"
            )
        );
    }
    let in_range = if target >= TRANSITION_TARGET_TABLE_START {
        ((target - TRANSITION_TARGET_TABLE_START) as usize) < transition_len
    } else {
        (target as usize) < index_len
    };
    if !in_range {
        crate::bail!(
            Hfst,
            format!(
                "optimized-lookup transducer is corrupt: transition {position} targets {target}, which is outside both the {index_len}-entry index table and the {transition_len}-entry transition table"
            )
        );
    }
    Ok(())
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
    fn read_bool_property(is: &mut IStream<'_>) -> crate::error::Result<bool> {
        let prop = is.read_u32();
        if prop == 0 {
            return Ok(false);
        }
        if prop == 1 {
            return Ok(true);
        }
        Err(Self::header_error())
    }

    /// 'TransducerHeader(bool weights)' — the header of the one-state, no-arc
    /// transducer. Every property flag below is honest for that shape and only
    /// that shape; [`Self::new_sizes`] is the constructor for real tables.
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

    /// A header for tables that already exist, told only the sizes.
    ///
    /// Every property flag is left false, meaning "nothing claimed". The C++
    /// hardcoded `deterministic` / `input_deterministic` / `minimized` true
    /// here — assertions about a graph this constructor has never seen, in the
    /// direction that makes a consumer skip work. The flags a walk can decide
    /// are filled in by [`Transducer::write`] when a file is actually emitted;
    /// in-memory queries read the graph, not the header.
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
            deterministic: false,
            input_deterministic: false,
            minimized: false,
            cyclic: false,
            has_epsilon_epsilon_transitions: false,
            has_input_epsilon_transitions: false,
            has_input_epsilon_cycles: false,
            has_unweighted_input_epsilon_cycles: false,
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-header.transducer-header-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-header.transducer-header-fn]
    pub fn new_istream(is: &mut IStream<'_>) -> crate::error::Result<Self> {
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
        let symbol_table = vec![Symbol::new_static("@_EPSILON_SYMBOL_@")];
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
        is: &mut IStream<'_>,
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
            let mut str = is.read_until(b'\0');
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
            alpha.symbol_table.push(Symbol::from(str));
            i += 1;
        }
        alpha.orig_symbol_count = u32::try_from(alpha.symbol_table.len())
            .expect("value out of u32 range") as SymbolNumber;
        Ok(alpha)
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.fake-read-alphabet-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.fake-read-alphabet-fn]
    pub fn fake_read_alphabet(is: &mut IStream<'_>, symbol_count: SymbolNumber) {
        let mut i: SymbolNumber = 0;
        while i < symbol_count {
            let _str = is.read_until(b'\0');
            i += 1;
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.add-symbol-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.add-symbol-fn]
    pub fn add_symbol_str(&mut self, symbol: &str) {
        self.symbol_table.push(Symbol::new(symbol));
    }

    pub fn add_symbol(&mut self, symbol: &Symbol) {
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
        for (i, sym) in self.symbol_table.iter().enumerate() {
            println!(" Symbol {}: {}", i, sym);
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
    pub fn string_from_symbol(&self, symbol: SymbolNumber) -> Symbol {
        if symbol == 0 {
            Symbol::default()
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
    /// Whether this index type belongs to the weighted table pair — the
    /// static counterpart of the header's 'Weighted' flag.
    const WEIGHTED: bool;
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
    pub fn new_istream(is: &mut IStream<'_>) -> Self {
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
        // Little-endian per hfst/hfst#328 (see module docs).
        TransitionIndex {
            input_symbol: u16::from_le_bytes([p[0], p[1]]),
            first_transition_index: u32::from_le_bytes([p[2], p[3], p[4], p[5]]),
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
    const WEIGHTED: bool = false;
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
    const WEIGHTED: bool = true;
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
        // Little-endian per hfst/hfst#328 (see module docs).
        Transition {
            input_symbol: u16::from_le_bytes([p[0], p[1]]),
            output_symbol: u16::from_le_bytes([p[2], p[3]]),
            target_index: u32::from_le_bytes([p[4], p[5], p[6], p[7]]),
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
            // Little-endian per hfst/hfst#328 (see module docs).
            let _ = os.write_all(&self.transition_weight.to_le_bytes());
        }
    }
}

impl TableEntry for TransitionW {
    const SIZE: usize = 2 * 2 + 4 + 4;
    // [spec:hfst:def:transducer.hfst-ol.transition-w.transition-w-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transition-w.transition-w-fn]
    fn from_bytes(p: &[u8]) -> Self {
        // Little-endian per hfst/hfst#328 (see module docs).
        TransitionW {
            base: Transition::from_bytes(p),
            transition_weight: f32::from_le_bytes([p[8], p[9], p[10], p[11]]),
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
    pub fn new_istream(is: &mut IStream<'_>, index_count: TransitionTableIndex) -> Self {
        // 'index_count' is a header field read straight off disk. Reading it in
        // one 'index_count * T::SIZE' buffer lets a corrupt header ask the
        // allocator for tens of gigabytes, which aborts the process before the
        // short read that would have revealed the corruption. Batching keeps
        // the ask bounded and stops at the first short read; the caller sees
        // the stream's fail flag and reports a clean error.
        const BATCH: usize = 64 * 1024;
        let mut table = Vec::new();
        let mut remaining = index_count as usize;
        let mut buf = vec![0u8; T::SIZE * BATCH.min(remaining.max(1))];
        while remaining != 0 {
            let n = remaining.min(BATCH);
            let chunk = &mut buf[..T::SIZE * n];
            is.read(chunk);
            if !is.good() {
                break;
            }
            for p in (0..chunk.len()).step_by(T::SIZE) {
                table.push(T::from_bytes(&chunk[p..p + T::SIZE]));
            }
            remaining -= n;
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

    /// The entry at `i`, or `None` when `i` addresses past the end of the table.
    ///
    /// A state's index-table probe is `state + input_symbol`, so the writer pads
    /// the index table with blank entries — one per *input* symbol — to keep the
    /// probe inside the table. A probe can nonetheless carry a higher symbol
    /// number than the padding covers: identity, unknown and output-only symbols
    /// are numbered above `input_symbol_count`, and a transducer with no
    /// transitions at all (the empty language, which is what `compose_intersect`
    /// of an empty rule vector yields) numbers *every* symbol that way. The C++
    /// read past the end of the vector and got whatever was there, which failed
    /// the symbol comparison that follows; `None` is that same "no such entry"
    /// answer, made explicit. `get_transitions_from_state` already guards its own
    /// probe this way.
    pub fn at(&self, i: TransitionTableIndex) -> Option<&T> {
        let offset = if i < TRANSITION_TARGET_TABLE_START {
            i
        } else {
            i - TRANSITION_TARGET_TABLE_START
        };
        self.table.get(offset as usize)
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-table.get-vector-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-table.get-vector-fn]
    pub fn get_vector(&self) -> Vec<T> {
        self.table.clone()
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer-table.size-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-table.size-fn]
    // The on-disk index/target table-size fields are u32 (see the OL header);
    // a table longer than u32::MAX cannot be represented, so report that as a
    // clean error instead of panicking on the narrowing conversion. This is
    // effectively unreachable on real data (such a table would be ~32 GB) but
    // gives a clear diagnostic in place of a silent wrap [hfst/hfst#123].
    pub fn size(&self) -> crate::error::Result<u32> {
        ol_table_size(self.table.len())
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
        for (i, entry) in self.table.iter().enumerate() {
            print!("{}", i);
            print!(": ");
            entry.display();
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

// The C++ 'TransducerTablesInterface' virtual base becomes a generic bound:
// it had exactly two implementations (the weighted and unweighted table
// pairs), and its accessors sit in the innermost lookup loop where C++
// devirtualizes but a Rust 'dyn' cannot. ['Transducer'] is generic over this
// trait, so the whole traversal machinery monomorphizes per table pair; the
// weighted/unweighted runtime choice is made once, where the stream header is
// read (the facade's HFST_OL_TYPE vs HFST_OLW_TYPE distinction), never per
// table access.
// [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface]
// [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.transducer-tables-interface-fn]
// [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.transducer-tables-interface-fn]
pub trait TransducerTablesInterface {
    /// Whether this is the weighted table pair — the static counterpart of
    /// the header's 'Weighted' flag; checked against it at load time.
    const WEIGHTED: bool;
    /// Construct the one-final-index empty table pair ('TransducerTables()').
    fn new_empty() -> Self;
    /// Read both tables from a stream ('TransducerTables(istream&, ...)').
    fn new_istream(
        is: &mut IStream<'_>,
        index_table_size: TransitionTableIndex,
        transition_table_size: TransitionTableIndex,
    ) -> Self;
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
    /// 'get_transition(i)->matches(s)' without the virtual hop.
    fn transition_matches(&self, i: TransitionTableIndex, s: SymbolNumber) -> bool;
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.get-index-input-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.get-index-input-fn]
    fn get_index_input(&self, i: TransitionTableIndex) -> SymbolNumber;
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.get-index-target-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.get-index-target-fn]
    fn get_index_target(&self, i: TransitionTableIndex) -> TransitionTableIndex;
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.get-index-finality-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.get-index-finality-fn]
    fn get_index_finality(&self, i: TransitionTableIndex) -> bool;
    /// 'get_index(i)->matches(s)' without the virtual hop.
    fn index_matches(&self, i: TransitionTableIndex, s: SymbolNumber) -> bool;
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.get-final-weight-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.get-final-weight-fn]
    fn get_final_weight(&self, i: TransitionTableIndex) -> Weight;
    /// 'get_index(i)->write(os, weighted)' — the serialization path.
    fn write_index(&self, i: TransitionTableIndex, os: &mut dyn std::io::Write, weighted: bool);
    /// 'get_transition(i)->write(os, weighted)' — the serialization path.
    fn write_transition(
        &self,
        i: TransitionTableIndex,
        os: &mut dyn std::io::Write,
        weighted: bool,
    );
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.display-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.display-fn]
    fn display(&self);
}

/// The unweighted table pair (HFST_OL_TYPE).
pub type UnweightedTables = TransducerTables<TransitionIndex, Transition>;
/// The weighted table pair (HFST_OLW_TYPE).
pub type WeightedTables = TransducerTables<TransitionWIndex, TransitionW>;

// [spec:hfst:def:transducer.hfst-ol.transducer-tables]
#[derive(Clone)]
pub struct TransducerTables<T1: IndexEntry + Clone, T2: TransitionEntry + Clone> {
    index_table: TransducerTable<T1>,
    transition_table: TransducerTable<T2>,
}

impl<T1: IndexEntry + TableEntry + Clone + IndexCtor, T2: TransitionEntry + TableEntry + Clone>
    TransducerTables<T1, T2>
{
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables.transducer-tables-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables.transducer-tables-fn]
    pub fn new_istream(
        is: &mut IStream<'_>,
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
    const WEIGHTED: bool = T1::WEIGHTED;
    fn new_empty() -> Self {
        Self::new()
    }
    fn new_istream(
        is: &mut IStream<'_>,
        index_table_size: TransitionTableIndex,
        transition_table_size: TransitionTableIndex,
    ) -> Self {
        TransducerTables::new_istream(is, index_table_size, transition_table_size)
    }
    // An index past the end of a table is the blank-padding entry the writer
    // would have supplied had the probing symbol been an input symbol (see
    // ['TransducerTable::at']), so every accessor below answers for a blank
    // entry: no input symbol, no target, not final, matches nothing. That keeps
    // `find_index` / `find_transitions` / `try_epsilon_indices` reporting "no
    // transition" instead of panicking on the raw index.
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-weight-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-weight-fn]
    #[inline]
    fn get_weight(&self, i: TransitionTableIndex) -> Weight {
        self.transition_table.at(i).map_or(0.0, |e| e.get_weight())
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-transition-input-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-transition-input-fn]
    #[inline]
    fn get_transition_input(&self, i: TransitionTableIndex) -> SymbolNumber {
        self.transition_table
            .at(i)
            .map_or(NO_SYMBOL_NUMBER, |e| e.get_input_symbol())
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-transition-output-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-transition-output-fn]
    #[inline]
    fn get_transition_output(&self, i: TransitionTableIndex) -> SymbolNumber {
        self.transition_table
            .at(i)
            .map_or(NO_SYMBOL_NUMBER, |e| e.get_output_symbol())
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-transition-target-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-transition-target-fn]
    #[inline]
    fn get_transition_target(&self, i: TransitionTableIndex) -> TransitionTableIndex {
        self.transition_table
            .at(i)
            .map_or(NO_TABLE_INDEX, |e| e.get_target())
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-transition-finality-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-transition-finality-fn]
    #[inline]
    fn get_transition_finality(&self, i: TransitionTableIndex) -> bool {
        self.transition_table.at(i).is_some_and(|e| e.is_final())
    }
    #[inline]
    fn transition_matches(&self, i: TransitionTableIndex, s: SymbolNumber) -> bool {
        self.transition_table.at(i).is_some_and(|e| e.matches(s))
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-index-input-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-index-input-fn]
    #[inline]
    fn get_index_input(&self, i: TransitionTableIndex) -> SymbolNumber {
        self.index_table
            .at(i)
            .map_or(NO_SYMBOL_NUMBER, |e| e.get_input_symbol())
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-index-target-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-index-target-fn]
    #[inline]
    fn get_index_target(&self, i: TransitionTableIndex) -> TransitionTableIndex {
        self.index_table
            .at(i)
            .map_or(NO_TABLE_INDEX, |e| e.get_target())
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-index-finality-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-index-finality-fn]
    #[inline]
    fn get_index_finality(&self, i: TransitionTableIndex) -> bool {
        self.index_table.at(i).is_some_and(|e| e.is_final())
    }
    #[inline]
    fn index_matches(&self, i: TransitionTableIndex, s: SymbolNumber) -> bool {
        self.index_table.at(i).is_some_and(|e| e.matches(s))
    }
    // [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-final-weight-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-final-weight-fn]
    #[inline]
    fn get_final_weight(&self, i: TransitionTableIndex) -> Weight {
        self.index_table.at(i).map_or(0.0, |e| e.final_weight())
    }
    fn write_index(&self, i: TransitionTableIndex, os: &mut dyn std::io::Write, weighted: bool) {
        self.index_table
            .at(i)
            .expect("write iterates the header sizes, which are the table sizes")
            .write(os, weighted)
    }
    fn write_transition(
        &self,
        i: TransitionTableIndex,
        os: &mut dyn std::io::Write,
        weighted: bool,
    ) {
        self.transition_table
            .at(i)
            .expect("write iterates the header sizes, which are the table sizes")
            .write(os, weighted)
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
            .expect("letters entry was set to Some just above")
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
    //
    // Two passes. Pass 1 indexes every symbol under its OWN, verbatim spelling —
    // that is the C++ walk, and it must win outright. Pass 2 adds the [#439]
    // normalization aliases, and only for spellings pass 1 left unclaimed.
    //
    // The order matters because an alias can collide with a real symbol: U+0387
    // GREEK ANO TELEIA has a singleton canonical decomposition to U+00B7 MIDDLE
    // DOT, so an alphabet containing BOTH (the Giella tokenisers do) had the
    // U+0387 alias overwrite the genuine U+00B7 entry whenever U+0387 came later
    // in the symbol table. Input U+00B7 then encoded to the U+0387 symbol: the
    // U+00B7 analyses vanished and even the echoed surface form was corrupted.
    // Registering aliases only into free slots keeps [#439] (an alias spelling
    // that is not itself a symbol still resolves) without ever shadowing a real
    // symbol.
    pub fn read_input_symbols(&mut self, kt: &SymbolTable) {
        for k in 0..self.number_of_input_symbols {
            let sym = kt[k as usize].clone();
            self.read_input_symbol_form(&sym, k as i32);
        }
        for k in 0..self.number_of_input_symbols {
            let sym = kt[k as usize].clone();
            self.read_alias_forms(&sym, k as i32);
        }
    }

    // True when the encoder already tokenizes exactly `s` to some symbol, so the
    // spelling is spoken for and an alias must not overwrite it. Runs the real
    // 'find_key' and demands it consume the whole string, so a prefix match (a
    // one-byte ascii symbol at the head of a longer spelling, say) is not
    // mistaken for a claim on the longer one.
    fn spelling_is_taken(&self, s: &str) -> bool {
        if s.is_empty() {
            return true;
        }
        // 'find_key' walks a 0-terminated buffer; the terminator is what stops
        // the trie descent reading past the end when the last byte has children.
        let mut buf = s.as_bytes().to_vec();
        buf.push(0);
        let mut p = 0usize;
        self.find_key(&buf, &mut p).is_some() && p == s.len()
    }

    // Register `s`'s normalization aliases against `s_num`, skipping any
    // spelling already claimed — by a real symbol or by an earlier alias.
    fn read_alias_forms(&mut self, s: &str, s_num: i32) {
        for form in Self::normalization_aliases(s) {
            if !self.spelling_is_taken(&form) {
                self.read_input_symbol_form(&form, s_num);
            }
        }
    }

    // [#439] Grapheme cluster is the port's logical tokenization unit, so a base
    // + combining diacritic and its precomposed form are the SAME unit and must
    // match the same symbol. These are the alternative spellings a symbol is
    // additionally indexed under (when they differ from it and from each other),
    // all mapping to the same symbol number, so input in either normalization
    // tokenizes to this symbol. Output is unaffected — the number still maps
    // back to the original surface.
    fn normalization_aliases(s: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let nfc = icu::normalizer::ComposingNormalizerBorrowed::new_nfc().normalize(s);
        if nfc.as_ref() != s {
            out.push(nfc.to_string());
        }
        let nfd = icu::normalizer::DecomposingNormalizerBorrowed::new_nfd().normalize(s);
        if nfd.as_ref() != s && nfd != nfc {
            out.push(nfd.into_owned());
        }
        out
    }

    // [spec:hfst:def:transducer.hfst-ol.encoder.read-input-symbol-fn]
    // [spec:hfst:sem:transducer.hfst-ol.encoder.read-input-symbol-fn]
    // Single-symbol registration, for the incremental callers that add a symbol
    // to an already-built encoder. The symbol's own spelling is registered
    // unconditionally (a real symbol always wins); its aliases only claim
    // spellings nothing else has, on the same rule as the table-wide walk.
    pub fn read_input_symbol(&mut self, s: &str, s_num: i32) {
        self.read_input_symbol_form(s, s_num);
        self.read_alias_forms(s, s_num);
    }

    fn read_input_symbol_form(&mut self, s: &str, s_num: i32) {
        let bytes = s.as_bytes();
        let strlen = bytes.len();
        // A symbol that spells the empty string cannot be tokenized out of the
        // input — it would consume no bytes — and the trie walk below assumes at
        // least one byte before the terminator. An alphabet read from a stream
        // can hold one (two adjacent NUL separators), so drop it here rather
        // than indexing past the end of a one-byte buffer.
        if strlen == 0 {
            return;
        }
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

    pub fn write_vec(&mut self, pos: u32, vec: &[SymbolNumber]) {
        while pos as usize + vec.len() >= self.inner.len() {
            self.inner.push(SymbolPair::new());
        }
        for (i, &v) in vec.iter().enumerate() {
            self.inner[pos as usize + i] = SymbolPair::new_values(v, v);
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
        for (i, &v) in slice.iter().enumerate() {
            self.inner[pos as usize + i] = SymbolPair::new_values(v, v);
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
/// How many bytes to peel off the tape as one UTF-8 character (for
/// representing it as OTHER), judged from the lead byte; `None` on a
/// continuation byte. Like the C original, invalid `11111xxx` lead bytes
/// are leniently treated as 4-byte sequences.
pub fn utf8_sequence_length(lead: u8) -> Option<usize> {
    match lead.leading_ones() {
        0 => Some(1),
        2 => Some(2),
        3 => Some(3),
        n if n >= 4 => Some(4),
        _ => None,
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
// Generic over the table pair (['UnweightedTables'] for HFST_OL_TYPE,
// ['WeightedTables'] for HFST_OLW_TYPE) so the traversal machinery is fully
// monomorphized; the runtime choice between the two instantiations lives at
// the facade's stream-type dispatch, not here.
// [spec:hfst:def:transducer.hfst-ol.transducer]
pub struct Transducer<T: TransducerTablesInterface = WeightedTables> {
    header: Option<Box<TransducerHeader>>,
    alphabet: Option<Box<TransducerAlphabet>>,
    tables: Option<T>,

    // for lookup
    current_weight: Weight,
    // The result set the recursive OL traversal accumulates into. It lives on
    // the transducer (all the traversal methods are '&mut self') and is cleared
    // at the start of each lookup; the C++ pointed a 'HfstTwoLevelPaths *'
    // member at a function-local set, which the owned field replaces directly.
    lookup_paths: HfstTwoLevelPaths,
    encoder: Option<Box<Encoder>>,
    input_tape: Tape,
    output_tape: DoubleTape,
    flag_state: FdState<SymbolNumber>,
    // whether we're going to take a default transition
    traversal_states: TraversalStates,

    max_lookups: isize,
    recursion_depth_left: u32,
    // Global node-visit budget for one lookup (hfst/hfst#293, hfst/hfst#476).
    // Unlike `recursion_depth_left` this is decremented on every `get_analyses`
    // entry and NEVER restored on backtrack, so it caps total traversal work on
    // pathological cyclic FSTs regardless of path depth or wall-clock.
    visits_left: u64,
    max_time: f64,
    start_clock: Option<Instant>,
}

/// One arc of a [`ReachableGraph`], with its target renumbered densely.
#[derive(Clone, Copy)]
struct GraphArc {
    input: SymbolNumber,
    output: SymbolNumber,
    weight: Weight,
    target: u32,
}

/// The reachable graph of an optimized-lookup table pair, renumbered from the
/// start state in discovery order: `arcs[starts[s]..starts[s + 1]]` are state
/// `s`'s outgoing arcs. Flat rather than per-state vectors because the nets
/// this runs over carry millions of arcs.
struct ReachableGraph {
    starts: Vec<u32>,
    arcs: Vec<GraphArc>,
}

/// Dense state numbers for optimized-lookup addresses. The address space is two
/// disjoint ranges — the index table below [`TRANSITION_TARGET_TABLE_START`],
/// the transition table above — so a pair of direct lookup tables answers in
/// O(1) where a map of a million states would dominate the walk.
struct StateIds {
    index_side: Vec<u32>,
    transition_side: Vec<u32>,
}

impl StateIds {
    const UNASSIGNED: u32 = u32::MAX;

    fn new(index_len: TransitionTableIndex, transition_len: TransitionTableIndex) -> Self {
        StateIds {
            index_side: vec![Self::UNASSIGNED; index_len as usize],
            transition_side: vec![Self::UNASSIGNED; transition_len as usize],
        }
    }

    /// The (side, offset) split of an address, or `None` when it addresses
    /// neither table. Load-time target validation
    /// ([`validate_transition_target`]) rules that out for anything read off
    /// disk, and the conversions only emit in-range targets, so `None` means a
    /// state that cannot be entered — treated as unreachable rather than
    /// panicking mid-walk.
    fn slot(&mut self, address: TransitionTableIndex) -> Option<&mut u32> {
        if indexes_transition_index_table(address) {
            self.index_side.get_mut(address as usize)
        } else {
            self.transition_side
                .get_mut((address - TRANSITION_TARGET_TABLE_START) as usize)
        }
    }

    /// The dense number of `address`, taking `fresh` if it had none. The
    /// second element says whether `fresh` was taken, so the caller knows to
    /// enqueue the state. `None` for an address in neither table.
    fn intern(&mut self, address: TransitionTableIndex, fresh: u32) -> Option<(u32, bool)> {
        let slot = self.slot(address)?;
        if *slot == Self::UNASSIGNED {
            *slot = fresh;
            Some((fresh, true))
        } else {
            Some((*slot, false))
        }
    }
}

impl ReachableGraph {
    /// Whether the graph holds a cycle over the arcs `admits` accepts.
    ///
    /// A visited set alone cannot answer this: re-reaching a state off the
    /// current path is a diamond, which every determinized net is full of, not
    /// a cycle. Hence the three-colour marking — grey for the states on the
    /// path under exploration, black for the fully explored — with a back edge
    /// into grey as the witness. The DFS is iterative because a lexicon-shaped
    /// net is millions of states deep and recursion would blow the stack.
    fn has_cycle_over(&self, admits: impl Fn(&GraphArc) -> bool) -> bool {
        const WHITE: u8 = 0;
        const GREY: u8 = 1;
        const BLACK: u8 = 2;

        let states = self.starts.len().saturating_sub(1);
        let mut colour = vec![WHITE; states];
        // Every state is a root, not just the start: an arc-restricted subgraph
        // can hold a cycle that its own arcs never reach from the start —
        // `a (0:b)*` hides its epsilon cycle behind an input arc. Colours carry
        // across roots, so the total work stays linear in the graph.
        for root in 0..states {
            if colour[root] != WHITE {
                continue;
            }
            colour[root] = GREY;
            // Each frame is a state and how far its arc list has been walked.
            let mut stack = vec![(root, self.starts[root])];
            while let Some(frame) = stack.last_mut() {
                let (state, cursor) = *frame;
                if cursor >= self.starts[state + 1] {
                    colour[state] = BLACK;
                    stack.pop();
                    continue;
                }
                frame.1 = cursor + 1;
                let arc = self.arcs[cursor as usize];
                if !admits(&arc) {
                    continue;
                }
                let target = arc.target as usize;
                match colour[target] {
                    GREY => return true,
                    BLACK => continue,
                    _ => {
                        colour[target] = GREY;
                        stack.push((target, self.starts[target]));
                    }
                }
            }
        }
        false
    }
}

#[allow(dead_code)]
impl<T: TransducerTablesInterface> Transducer<T> {
    // ---- small accessors mirroring the C++ member dereferences ----
    fn hdr(&self) -> &TransducerHeader {
        self.header
            .as_deref()
            .expect("header is initialized during container load")
    }
    fn alph(&self) -> &TransducerAlphabet {
        self.alphabet
            .as_deref()
            .expect("alphabet is initialized during container load")
    }
    #[inline]
    fn tbl(&self) -> &T {
        self.tables
            .as_ref()
            .expect("tables are initialized during container load")
    }

    pub fn new() -> Self {
        Transducer {
            header: None,
            alphabet: None,
            tables: None,
            current_weight: 0.0,
            lookup_paths: BTreeSet::new(),
            encoder: None,
            input_tape: Tape::new(),
            output_tape: DoubleTape::new(),
            flag_state: FdState::new_default(),
            traversal_states: TraversalStates::new(),
            max_lookups: -1,
            recursion_depth_left: MAX_RECURSION_DEPTH,
            visits_left: MAX_NODE_VISITS,
            max_time: 0.0,
            start_clock: None,
        }
    }

    pub fn new_istream(is: &mut IStream<'_>) -> crate::error::Result<Self> {
        let header = TransducerHeader::new_istream(is)?;
        Self::new_istream_with_header(header, is)
    }

    /// The tail of 'new_istream' once the header has been read — the caller
    /// (['AnyOlTransducer::new_istream']) peeks the Weighted flag to pick the
    /// instantiation, then hands the header over.
    pub fn new_istream_with_header(
        header: TransducerHeader,
        is: &mut IStream<'_>,
    ) -> crate::error::Result<Self> {
        let header = Box::new(header);
        // The weightedness is now static; a stream of the other flavour is the
        // caller dispatching wrongly (C++ discovered this inside load_tables).
        if header.probe_flag(HeaderFlag::Weighted) != T::WEIGHTED {
            crate::bail!(TransducerHasWrongType);
        }
        // Input symbols are the leading run of the alphabet, so a header
        // claiming more of them than there are symbols is corrupt — and the
        // encoder would walk off the end of the symbol table building its trie.
        if header.input_symbol_count() > header.symbol_count() {
            crate::bail!(
                Hfst,
                format!(
                    "optimized-lookup transducer is corrupt: header declares {} input symbols out of {} symbols",
                    header.input_symbol_count(),
                    header.symbol_count()
                )
            );
        }
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
            lookup_paths: BTreeSet::new(),
            encoder: Some(encoder),
            input_tape: Tape::new(),
            output_tape: DoubleTape::new(),
            flag_state,
            traversal_states: TraversalStates::new(),
            max_lookups: -1,
            recursion_depth_left: MAX_RECURSION_DEPTH,
            visits_left: MAX_NODE_VISITS,
            max_time: 0.0,
            start_clock: None,
        };
        t.load_tables(is)?;
        Ok(t)
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.transducer-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.transducer-fn]
    // (the C++ 'Transducer(bool weighted)' — weightedness is now the type)
    pub fn new_empty() -> Self {
        let header = Box::new(TransducerHeader::new_weighted(T::WEIGHTED));
        let alphabet = Box::new(TransducerAlphabet::new());
        let encoder = Box::new(Encoder::new(
            alphabet.get_symbol_table(),
            header.input_symbol_count(),
        ));
        let flag_state = FdState::new(alphabet.get_fd_table());
        let tables = T::new_empty();
        Transducer {
            header: Some(header),
            alphabet: Some(alphabet),
            tables: Some(tables),
            current_weight: 0.0,
            lookup_paths: BTreeSet::new(),
            encoder: Some(encoder),
            input_tape: Tape::new(),
            output_tape: DoubleTape::new(),
            flag_state,
            traversal_states: TraversalStates::new(),
            max_lookups: -1,
            recursion_depth_left: MAX_RECURSION_DEPTH,
            visits_left: MAX_NODE_VISITS,
            max_time: 0.0,
            start_clock: None,
        }
    }

    // The C++ builds 'encoder'/'flag_state' from the *parameter* alphabet (dot,
    // not arrow), so they reference the caller's alphabet; replicated here.
    pub fn new_from_tables(
        header: &TransducerHeader,
        alphabet: &TransducerAlphabet,
        tables: T,
    ) -> Self {
        let header_box = Box::new(header.clone());
        let alphabet_box = Box::new(alphabet.clone());
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
            lookup_paths: BTreeSet::new(),
            encoder: Some(encoder),
            input_tape: Tape::new(),
            output_tape: DoubleTape::new(),
            flag_state,
            traversal_states: TraversalStates::new(),
            max_lookups: -1,
            recursion_depth_left: MAX_RECURSION_DEPTH,
            visits_left: MAX_NODE_VISITS,
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
        self.encoder
            .as_deref()
            .expect("encoder is initialized during container load")
    }
    pub fn get_fd_table(&self) -> &FdTable<SymbolNumber> {
        self.alph().get_fd_table()
    }
    pub fn get_symbol_table(&self) -> &SymbolTable {
        self.alph().get_symbol_table()
    }

    // The C++ 'get_index'/'get_transition' returned base-class pointers; with
    // the tables monomorphized, expose the scalar reads directly instead.
    #[inline]
    pub fn get_index_input(&self, i: TransitionTableIndex) -> SymbolNumber {
        self.tbl().get_index_input(i)
    }
    #[inline]
    pub fn get_index_target(&self, i: TransitionTableIndex) -> TransitionTableIndex {
        self.tbl().get_index_target(i)
    }
    #[inline]
    pub fn index_matches(&self, i: TransitionTableIndex, s: SymbolNumber) -> bool {
        self.tbl().index_matches(i, s)
    }
    #[inline]
    pub fn get_transition_input(&self, i: TransitionTableIndex) -> SymbolNumber {
        self.tbl().get_transition_input(i)
    }
    #[inline]
    pub fn get_transition_output(&self, i: TransitionTableIndex) -> SymbolNumber {
        self.tbl().get_transition_output(i)
    }
    #[inline]
    pub fn get_transition_target(&self, i: TransitionTableIndex) -> TransitionTableIndex {
        self.tbl().get_transition_target(i)
    }
    #[inline]
    pub fn get_transition_weight(&self, i: TransitionTableIndex) -> Weight {
        self.tbl().get_weight(i)
    }
    #[inline]
    pub fn transition_matches(&self, i: TransitionTableIndex, s: SymbolNumber) -> bool {
        self.tbl().transition_matches(i, s)
    }
    #[inline]
    pub fn get_index_finality(&self, i: TransitionTableIndex) -> bool {
        self.tbl().get_index_finality(i)
    }
    #[inline]
    pub fn get_transition_finality(&self, i: TransitionTableIndex) -> bool {
        self.tbl().get_transition_finality(i)
    }
    #[inline]
    pub fn get_index_final_weight(&self, i: TransitionTableIndex) -> Weight {
        self.tbl().get_final_weight(i)
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

    /// The reachable graph of this table pair, lifted out of the tables once.
    ///
    /// The optimized-lookup encoding keeps no state list to read anything off:
    /// a state is an offset into the index or the transition table, and the two
    /// share one address space. Worse, reading a state's arcs is not O(1) — for
    /// a state in the index table `get_transitions_from_state` scans the whole
    /// alphabet — so on a lexicon-sized net one sweep of the tables costs about
    /// a second. Every question below wants several passes over the same graph,
    /// hence one sweep into a flat adjacency and cheap passes over that.
    ///
    /// This is the walk `hfst_ol_to_hfst_basic_transducer` numbers states with,
    /// minus the interchange transducer it materializes, so anything derived
    /// here describes the graph `to_basic` would build.
    fn reachable_graph(&self) -> ReachableGraph {
        const START: TransitionTableIndex = 0;
        let mut ids = StateIds::new(
            self.hdr().index_table_size(),
            self.hdr().target_table_size(),
        );
        let mut order = vec![START];
        ids.intern(START, 0);

        let mut starts = vec![0u32];
        let mut arcs: Vec<GraphArc> = Vec::new();
        let mut next = 0usize;
        while next < order.len() {
            let state = order[next];
            next += 1;
            for tr in self.get_transitions_from_state(state).iter() {
                let target = self.get_transition_target(*tr);
                let Some((id, is_fresh)) = ids.intern(target, order.len() as u32) else {
                    continue;
                };
                if is_fresh {
                    order.push(target);
                }
                arcs.push(GraphArc {
                    input: self.get_transition_input(*tr),
                    output: self.get_transition_output(*tr),
                    weight: self.get_transition_weight(*tr),
                    target: id,
                });
            }
            starts.push(arcs.len() as u32);
        }
        ReachableGraph { starts, arcs }
    }

    /// Whether `input` consumes nothing off the input tape. Epsilon is symbol
    /// zero; a flag diacritic is scanned by the lookup engine without advancing
    /// the tape, so `find_loop_epsilon_transitions` and
    /// `HfstBasicTransducer::is_infinitely_ambiguous` both count it as epsilon
    /// — at the cost of false positives on flags no path can actually satisfy.
    fn consumes_no_input(&self, input: SymbolNumber) -> bool {
        input == 0 || self.alph().is_flag_diacritic(input)
    }

    /// Whether the transducer has a cycle at all.
    ///
    /// Diverges from the C++, which probes `HeaderFlag::Cyclic`: nothing in
    /// either tree ever calls `TransducerHeader::set_flag`, and every in-memory
    /// constructor hardcodes the flag false, so the probe answered "acyclic"
    /// for every transducer that had not been read off a disk file some other
    /// tool had stamped. Path extraction is guarded on this answer, so a cyclic
    /// net enumerated its infinite language until the disk filled.
    // [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-transducer.is-cyclic-fn]
    // [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-transducer.is-cyclic-fn]
    pub fn is_cyclic(&self) -> bool {
        self.reachable_graph().has_cycle_over(|_| true)
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.is-infinitely-ambiguous-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.is-infinitely-ambiguous-fn]
    /// Infinite ambiguity is an INPUT-EPSILON cycle, not any cycle: `a*` loops
    /// but each turn consumes an input symbol, so one input string still has
    /// finitely many analyses, while `(0:a)*` yields unboundedly many for the
    /// empty input. Same divergence from the flag probe as [`Self::is_cyclic`].
    pub fn is_infinitely_ambiguous(&self) -> bool {
        self.reachable_graph()
            .has_cycle_over(|arc| self.consumes_no_input(arc.input))
    }

    /// This transducer's own header, with every property flag it can decide
    /// filled in from the graph — what a file we write should say about itself.
    ///
    /// The three left false are the three no single walk decides: `Minimized`
    /// needs a minimization to establish, and `Deterministic` /
    /// `Input_deterministic` have no reader in this format (the C++ never
    /// consults them, and epsilon arcs make the question depend on which
    /// reading you take). False here means "not claimed" — the safe direction,
    /// unlike the hardcoded true every conversion used to stamp, which invites
    /// a consumer to skip work it needed.
    fn header_with_graph_properties(&self) -> TransducerHeader {
        let graph = self.reachable_graph();
        let mut header = self.hdr().clone();

        header.cyclic = graph.has_cycle_over(|_| true);
        header.has_input_epsilon_cycles =
            header.cyclic && graph.has_cycle_over(|arc| self.consumes_no_input(arc.input));
        // An unweighted input-epsilon cycle is one whose every arc is free, so
        // going round it costs nothing and no weight cutoff prunes it.
        header.has_unweighted_input_epsilon_cycles = header.has_input_epsilon_cycles
            && graph.has_cycle_over(|arc| self.consumes_no_input(arc.input) && arc.weight == 0.0);

        // "Input epsilon" means the same thing here as it does to the lookup
        // engine and to the cycle flags above: an arc that advances no input
        // tape position, which a flag diacritic does not either. Reading it as
        // literal symbol zero let a flag-only cycle write a header claiming an
        // input-epsilon cycle whose arcs it denied having.
        header.has_input_epsilon_transitions = graph
            .arcs
            .iter()
            .any(|arc| self.consumes_no_input(arc.input));
        // Both tapes, and here epsilon is strictly symbol zero: a flag is
        // written to the output tape, and stripping it again is the lookup
        // formatter's business rather than a property of the graph.
        header.has_epsilon_epsilon_transitions = graph
            .arcs
            .iter()
            .any(|arc| arc.input == 0 && arc.output == 0);
        header
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
            another.append(TransitionIndex::new_values(
                self.tbl().get_index_input(i),
                self.tbl().get_index_target(i),
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
            another.append(Transition::new_values(
                self.tbl().get_transition_input(i),
                self.tbl().get_transition_output(i),
                self.tbl().get_transition_target(i),
            ));
        }
        Ok(another)
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.load-tables-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.load-tables-fn]
    pub fn load_tables(&mut self, is: &mut IStream<'_>) -> crate::error::Result<()> {
        if self.hdr().probe_flag(HeaderFlag::Weighted) != T::WEIGHTED {
            crate::bail!(TransducerHasWrongType);
        }
        let its = self.hdr().index_table_size();
        let tts = self.hdr().target_table_size();
        self.tables = Some(T::new_istream(is, its, tts));
        if !is.good() {
            crate::bail!(TransducerHasWrongType);
        }
        self.validate_tables()
    }

    /// Reject a table pair whose symbols or targets point outside the tables
    /// and the alphabet they were read alongside. Run once at the stream
    /// boundary; see [`validate_ol_index_entry`].
    fn validate_tables(&self) -> crate::error::Result<()> {
        let index_len = self.hdr().index_table_size();
        let transition_len = self.hdr().target_table_size();
        let symbol_count = self.alph().get_symbol_table().len();
        for i in 0..index_len {
            validate_ol_index_entry(
                i as usize,
                self.tbl().get_index_input(i),
                self.tbl().get_index_target(i),
                symbol_count,
                transition_len as usize,
            )?;
        }
        for i in 0..transition_len {
            validate_ol_transition_entry(
                i as usize,
                self.tbl().get_transition_input(i),
                self.tbl().get_transition_output(i),
                self.tbl().get_transition_target(i),
                symbol_count,
                index_len as usize,
                transition_len as usize,
            )?;
        }
        Ok(())
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.write-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.write-fn]
    pub fn write(&self, os: &mut dyn std::io::Write) {
        // The header carried in memory says nothing true about the graph — no
        // constructor computes the property flags and `set_flag` has no callers
        // in either tree — so derive them here rather than write a file that
        // misdescribes itself to whoever reads it next. One walk per file.
        self.header_with_graph_properties().write(os);
        self.alph().write(os);
        let weighted = self.hdr().probe_flag(HeaderFlag::Weighted);
        // 'i' is already a TransitionTableIndex (u32), so no narrowing occurs.
        for i in 0..self.hdr().index_table_size() {
            self.tbl().write_index(i, os, weighted);
        }
        for i in 0..self.hdr().target_table_size() {
            self.tbl().write_transition(i, os, weighted);
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.copy-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.copy-fn]
    // (the C++ 'copy(t, weighted)' — the target weightedness is now the type,
    // and the entrywise copy_*_table rebuild is a table clone. The
    // cross-weightedness combinations threw TransducerHasWrongType in C++ via
    // the copy_*_table guards; they are unrepresentable now.)
    pub fn copy(t: &Transducer<T>) -> crate::error::Result<Transducer<T>>
    where
        T: Clone,
    {
        Ok(Transducer::new_from_tables(
            t.get_header(),
            t.get_alphabet(),
            t.tbl().clone(),
        ))
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
                    let mut transition_i = self.get_index_target(state_index + 1);
                    if !self.index_matches(state_index + 1, 0) {
                        continue;
                    }
                    loop {
                        let input = self.get_transition_input(transition_i);
                        if self.transition_matches(transition_i, symbol) {
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
                    let test_input = self.get_index_input(state_index + 1 + symbol as u32);
                    let test_target = self.get_index_target(state_index + 1 + symbol as u32);
                    if self.index_matches(state_index + 1 + symbol as u32, symbol) {
                        // there are one or more transitions with this input
                        // symbol, starting at test_transition_index.get_target()
                        let mut transition_i = test_target;
                        loop {
                            if self.transition_matches(transition_i, test_input) {
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
            let in_sym = self.get_transition_input(state_index);
            let out_sym = self.get_transition_output(state_index);
            if in_sym != NO_SYMBOL_NUMBER || out_sym != NO_SYMBOL_NUMBER {
                // Oops
                panic!("get_transitions_from_state: malformed transition boundary");
            }

            let mut transition_i = state_index + 1;
            loop {
                if self.get_transition_input(transition_i) != NO_SYMBOL_NUMBER {
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
            self.get_index_target(i + 1 + symbol as u32) - TRANSITION_TARGET_TABLE_START
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.next-e-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.next-e-fn]
    // (declared in transducer.h; defined in pmatch.cc — ported with pmatch.)

    // [spec:hfst:def:transducer.hfst-ol.transducer.has-transitions-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.has-transitions-fn]
    pub fn has_transitions(&self, i: TransitionTableIndex, symbol: SymbolNumber) -> bool {
        if i >= TRANSITION_TARGET_TABLE_START {
            self.get_transition_input(i - TRANSITION_TARGET_TABLE_START) == symbol
        } else {
            self.get_index_input(i + symbol as u32) == symbol
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.has-epsilons-or-flags-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.has-epsilons-or-flags-fn]
    pub fn has_epsilons_or_flags(&self, i: TransitionTableIndex) -> bool {
        if i >= TRANSITION_TARGET_TABLE_START {
            let input = self.get_transition_input(i - TRANSITION_TARGET_TABLE_START);
            input == 0 || self.is_flag(input)
        } else {
            self.get_index_input(i) == 0
        }
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.take-epsilons-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.take-epsilons-fn]
    pub fn take_epsilons(&self, i: TransitionTableIndex) -> STransition {
        if self.get_transition_input(i) != 0 {
            return STransition::new(0, NO_SYMBOL_NUMBER);
        }
        STransition::new_weighted(
            self.get_transition_target(i),
            self.get_transition_output(i),
            self.get_transition_weight(i),
        )
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.take-epsilons-and-flags-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.take-epsilons-and-flags-fn]
    pub fn take_epsilons_and_flags(&self, i: TransitionTableIndex) -> STransition {
        if self.get_transition_input(i) != 0 && !self.is_flag(self.get_transition_input(i)) {
            return STransition::new(0, NO_SYMBOL_NUMBER);
        }
        STransition::new_weighted(
            self.get_transition_target(i),
            self.get_transition_output(i),
            self.get_transition_weight(i),
        )
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.take-non-epsilons-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.take-non-epsilons-fn]
    pub fn take_non_epsilons(&self, i: TransitionTableIndex, symbol: SymbolNumber) -> STransition {
        if self.get_transition_input(i) != symbol {
            return STransition::new(0, NO_SYMBOL_NUMBER);
        }
        STransition::new_weighted(
            self.get_transition_target(i),
            self.get_transition_output(i),
            self.get_transition_weight(i),
        )
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.final-weight-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.final-weight-fn]
    pub fn final_weight(&self, i: TransitionTableIndex) -> Weight {
        if i >= TRANSITION_TARGET_TABLE_START {
            self.get_transition_weight(i - TRANSITION_TARGET_TABLE_START)
        } else {
            self.get_index_final_weight(i)
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
                    let Some(bytes_to_tokenize) = utf8_sequence_length(buf[p]) else {
                        return false; // tokenization failed
                    };
                    let new_symbol =
                        Symbol::from(String::from_utf8_lossy(&buf[p..p + bytes_to_tokenize]));
                    p += bytes_to_tokenize;
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

    /// Whether `input` tokenizes wholly into symbols the alphabet already has.
    ///
    /// [`Self::initialize_input`] answers a different question: it ADOPTS an
    /// unrecognized symbol into the alphabet and carries on, so it fails only on
    /// malformed UTF-8. The C++ `find_next_key` loop in hfst-optimized-lookup is
    /// the strict reading, and the tool distinguishes the two outcomes — a word
    /// it cannot tokenize is reported unanalysable even in fast mode, while a
    /// word that tokenizes and simply has no analysis is not.
    pub fn can_tokenize(&self, input: &str) -> bool {
        let mut buf: Vec<u8> = input.as_bytes().to_vec();
        buf.push(0);
        let mut p: usize = 0;
        let encoder = self
            .encoder
            .as_ref()
            .expect("encoder is initialized during transducer load");
        while buf[p] != 0 {
            if encoder.find_key(&buf, &mut p).is_none() {
                return false;
            }
        }
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
        self.alphabet
            .as_mut()
            .expect("alphabet is initialized during container load")
            .add_symbol_str(sym);
        self.encoder
            .as_mut()
            .expect("encoder is initialized during container load")
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
        self.visits_left = MAX_NODE_VISITS;
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
    pub fn lookup_fd_pairs_cstr(
        &mut self,
        s: &str,
        limit: isize,
        time_cutoff: f64,
    ) -> HfstTwoLevelPaths {
        self.max_lookups = limit;
        self.visits_left = MAX_NODE_VISITS;
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

        // Global work budget (hfst/hfst#293, hfst/hfst#476): count every node
        // visit and stop once the budget is spent. `recursion_depth_left` only
        // caps a single path's depth (it is restored on backtrack), so on an
        // epsilon cycle it would otherwise let the traversal run 5000 levels
        // deep — piling up junk analyses with runaway weights and holding them
        // in memory — before terminating. Decrementing here, with no restore on
        // the return paths below, bounds total traversal work on any FST.
        if self.visits_left == 0 {
            return;
        }
        self.visits_left -= 1;

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
                if self.tbl().get_transition_finality(i) {
                    let old_weight = self.current_weight;
                    let w = self.tbl().get_weight(i);
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
                if self.tbl().get_index_finality(i) {
                    let old_weight = self.current_weight;
                    let w = self.tbl().get_final_weight(i);
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
            let in_s = self.alph().string_from_symbol(pair.input);
            let out_s = self.alph().string_from_symbol(pair.output);
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
            let target = self.tbl().get_transition_target(i);
            let epsilon_reachable = TraversalState::new(target, &flags);
            let tin = self.tbl().get_transition_input(i);
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
        // A symbol beyond this transducer's input alphabet (e.g. one that only
        // exists in another cascade member) has no index transition: the index
        // table is padded only up to input_symbol_count, so gate the lookup on
        // it rather than indexing out of bounds.
        if input < self.hdr().input_symbol_count()
            && self.tbl().get_index_input(i + input as u32) == input
        {
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

impl<T: TransducerTablesInterface> Default for Transducer<T> {
    fn default() -> Self {
        Self::new()
    }
}
