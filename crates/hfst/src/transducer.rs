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
//! already explicitly little-endian. 'std::istream' becomes '&mut dyn BufRead'
//! and every reader reports a short or malformed read as an [`crate::error::Error`];
//! 'std::ostream' becomes '&mut dyn Write'.
//!
//! C++ value-type inheritance (the concrete base 'TransitionIndex' with a
//! derived 'TransitionWIndex' overriding 'final_weight', likewise
//! 'Transition'/'TransitionW') is modelled as a base struct plus a trait that
//! captures the virtual methods, since the tables hand them out through base
//! references. The pure-abstract 'TransducerTablesInterface' becomes a trait
//! object.

use smallvec::SmallVec;
use std::collections::{BTreeMap, BTreeSet};

use crate::hfst_data_types::Symbol;
use crate::hfst_flag_diacritics::{FdOperation, FdState, FdTable};

mod alphabet;
mod encoder;
mod graph;
mod header;
mod lookup;
mod lookup_types;
mod table_io;
mod tables;
mod transition;
mod transition_index;
mod traversal;

pub use alphabet::TransducerAlphabet;
pub(crate) use alphabet::{UnicodeClassCacheValue, unicode_class_of};
pub use encoder::{Encoder, OlLetterTrie, OlLetterTrieVector, should_ascii_tokenize};
pub use header::{HeaderFlag, TransducerHeader};
pub use lookup_types::{
    DoubleTape, STransition, StringWeightPair, SymbolPair, Tape, WeightedDoubleTape,
    utf8_sequence_length,
};
pub use tables::{
    TableEntry, TransducerTable, TransducerTables, TransducerTablesInterface, UnweightedTables,
    WeightedTables,
};
pub use transition::{Transition, TransitionEntry, TransitionW};
pub use transition_index::{IndexCtor, IndexEntry, TransitionIndex, TransitionWIndex};

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

// This is 2^31, hopefully equal to UINT_MAX/2 rounded up.
// For some profound reason it can't be replaced with (UINT_MAX+1)/2.
pub const TRANSITION_TARGET_TABLE_START: TransitionTableIndex = 2147483648u32;
pub const MAX_IO_LEN: u32 = 10000;
pub const MAX_RECURSION_DEPTH: u32 = 5000;

// Termination of `get_analyses` rests on the DFS-path-scoped epsilon/flag cycle
// trap (see `TraversalStates`) plus `MAX_RECURSION_DEPTH`, never on a global cap
// over total node visits. A whole-lookup work ceiling MUST NOT be reintroduced:
// it silently truncates enumeration mid-traversal, and because DFS visits arcs
// in table order, WHERE it truncates depends on how the same relation happens to
// be laid out. Two representations of one relation then answer differently, and
// a minimum-weight analysis reachable only past the ceiling is lost outright.
// Enumeration limits belong to the caller — `max_lookups` (a result count) and
// `max_time` (a wall-clock cutoff) — both of which the caller opts into and can
// therefore reason about (hfst/hfst#293, hfst/hfst#476).

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

/// Read exactly `buf.len()` bytes, reporting a short read as a malformed
/// optimized-lookup payload — the one shape every reader below shares.
fn read_exact_bytes(is: &mut dyn std::io::BufRead, buf: &mut [u8]) -> crate::error::Result<()> {
    is.read_exact(buf)
        .map_err(|_| crate::err!(TransducerHasWrongType))
}

// 'is.read(reinterpret_cast<char*>(&p), sizeof(T))' for the integer properties —
// the mirror of the static template 'read_property<T>' that 'TransducerHeader'
// uses to read its fields. Little-endian per hfst/hfst#328 (see module docs).
// [spec:hfst:def:transducer.hfst-ol.transducer-header.read-property-fn]
// [spec:hfst:sem:transducer.hfst-ol.transducer-header.read-property-fn]
fn read_u16(is: &mut dyn std::io::BufRead) -> crate::error::Result<u16> {
    let mut b = [0u8; 2];
    read_exact_bytes(is, &mut b)?;
    Ok(u16::from_le_bytes(b))
}
fn read_u32(is: &mut dyn std::io::BufRead) -> crate::error::Result<u32> {
    let mut b = [0u8; 4];
    read_exact_bytes(is, &mut b)?;
    Ok(u32::from_le_bytes(b))
}

/// One NUL-terminated symbol string off the alphabet block.
///
/// 'std::getline' with a NUL delimiter accepts a final run of bytes the stream
/// ends before terminating; only a read that finds nothing at all fails. That
/// tolerance is preserved here, so an alphabet whose last symbol lost its
/// terminator still loads.
fn read_symbol_string(is: &mut dyn std::io::BufRead) -> crate::error::Result<String> {
    let mut bytes: Vec<u8> = Vec::new();
    match is.read_until(b'\0', &mut bytes) {
        Ok(0) | Err(_) => crate::bail!(TransducerHasWrongType),
        Ok(_) => {}
    }
    if bytes.last() == Some(&b'\0') {
        bytes.pop();
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
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

/** \brief A compiled transducer format, suitable for fast lookup operations. */
// Generic over the table pair (['UnweightedTables'] for HFST_OL_TYPE,
// ['WeightedTables'] for HFST_OLW_TYPE) so the traversal machinery is fully
// monomorphized; the runtime choice between the two instantiations lives at
// the facade's stream-type dispatch, not here.
// Nothing here is written after load: a lookup borrows the machine shared and
// keeps its own scratch in a ['crate::lookup_state::LookupState'], so one
// loaded transducer can serve any number of concurrent traversals without a
// lock. The only mutating method left is ['Transducer::include_symbol_in_alphabet'],
// which the pmatch container calls while assembling the machine.
// [spec:hfst:req:lookup-run-state.immutable-core]
// [spec:hfst:def:transducer.hfst-ol.transducer]
pub struct Transducer<T: TransducerTablesInterface = WeightedTables> {
    header: Option<Box<TransducerHeader>>,
    alphabet: Option<Box<TransducerAlphabet>>,
    tables: Option<T>,
    encoder: Option<Box<Encoder>>,
    // The neutral flag-diacritic state this machine's alphabet defines, built
    // once at load. Never evaluated here — a run state clones it, which is a
    // refcount bump on the flag table rather than a copy of it.
    flag_state_proto: FdState<SymbolNumber>,
}

#[allow(dead_code)]
impl<T: TransducerTablesInterface> Transducer<T> {
    // ---- small accessors mirroring the C++ member dereferences ----
    #[inline]
    fn hdr(&self) -> &TransducerHeader {
        self.header
            .as_deref()
            .expect("header is initialized during container load")
    }
    #[inline]
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
            encoder: None,
            flag_state_proto: FdState::new_default(),
        }
    }

    pub fn read_from(is: &mut dyn std::io::BufRead) -> crate::error::Result<Self> {
        let header = TransducerHeader::read_from(is)?;
        Self::read_from_with_header(header, is)
    }

    /// The tail of 'read_from' once the header has been read — the caller peeks
    /// the Weighted flag to pick the instantiation, then hands the header over.
    pub fn read_from_with_header(
        header: TransducerHeader,
        is: &mut dyn std::io::BufRead,
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
        let alphabet = Box::new(TransducerAlphabet::read_from(
            is,
            header.symbol_count(),
            true,
        )?);
        let encoder = Box::new(Encoder::new(
            alphabet.get_symbol_table(),
            header.input_symbol_count(),
        ));
        let flag_state_proto = FdState::new(alphabet.get_fd_table());
        let mut t = Transducer {
            header: Some(header),
            alphabet: Some(alphabet),
            tables: None,
            encoder: Some(encoder),
            flag_state_proto,
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
        let flag_state_proto = FdState::new(alphabet.get_fd_table());
        let tables = T::new_empty();
        Transducer {
            header: Some(header),
            alphabet: Some(alphabet),
            tables: Some(tables),
            encoder: Some(encoder),
            flag_state_proto,
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
        let flag_state_proto = FdState::new(alphabet.get_fd_table());
        Transducer {
            header: Some(header_box),
            alphabet: Some(alphabet_box),
            tables: Some(tables),
            encoder: Some(encoder),
            flag_state_proto,
        }
    }

    #[inline]
    pub fn get_header(&self) -> &TransducerHeader {
        self.hdr()
    }
    #[inline]
    pub fn get_alphabet(&self) -> &TransducerAlphabet {
        self.alph()
    }
    #[inline]
    pub fn get_encoder(&self) -> &Encoder {
        self.encoder
            .as_deref()
            .expect("encoder is initialized during container load")
    }
    pub fn get_fd_table(&self) -> &FdTable<SymbolNumber> {
        self.alph().get_fd_table()
    }
    /// The neutral flag state a run starts from; see the field.
    pub(crate) fn flag_state_proto(&self) -> &FdState<SymbolNumber> {
        &self.flag_state_proto
    }
    #[inline]
    pub fn get_symbol_table(&self) -> &SymbolTable {
        self.alph().get_symbol_table()
    }
}

impl<T: TransducerTablesInterface> Default for Transducer<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A table is read entry by entry, so its vector grows geometrically and
    // ends up holding up to twice the bytes the entries need.
    // [spec:hfst:req:table-residency.single-copy-load/test]
    // [spec:hfst:req:table-residency.views-not-copies/test]
    #[test]
    fn loaded_table_capacity_equals_len() {
        // Past the read batch, and not a power of two, so the growth sequence
        // does overshoot.
        const ENTRIES: usize = 100_000;
        let mut cursor = std::io::Cursor::new(vec![0u8; ENTRIES * TransitionWIndex::SIZE]);
        let table = TransducerTable::<TransitionWIndex>::read_from(&mut cursor, ENTRIES as u32)
            .expect("the cursor holds exactly the bytes the table asks for");
        assert_eq!(table.as_slice().len(), ENTRIES);
        assert_eq!(table.into_vector().capacity(), ENTRIES);
    }
}
