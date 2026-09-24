//! The entry tables and the table pair a transducer looks up through.

use super::*;

/// Captures the 'static const size_t size' + 'T(char*)' constructor that
/// 'TransducerTable<T>' requires of its entry type for binary loading.
pub trait TableEntry {
    const SIZE: usize;
    fn from_bytes(p: &[u8]) -> Self;
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
    pub fn read_from(
        is: &mut dyn std::io::BufRead,
        index_count: TransitionTableIndex,
    ) -> crate::error::Result<Self> {
        // 'index_count' is a header field read straight off disk. Reading it in
        // one 'index_count * T::SIZE' buffer lets a corrupt header ask the
        // allocator for tens of gigabytes, which aborts the process before the
        // short read that would have revealed the corruption. Batching keeps
        // the ask bounded and stops at the first short read, which is reported
        // as the malformed payload it is.
        // [spec:hfst:req:table-residency.untrusted-size-fields]
        const BATCH: usize = 64 * 1024;
        let mut table = Vec::new();
        let mut remaining = index_count as usize;
        let mut buf = vec![0u8; T::SIZE * BATCH.min(remaining.max(1))];
        while remaining != 0 {
            let n = remaining.min(BATCH);
            let chunk = &mut buf[..T::SIZE * n];
            read_exact_bytes(is, chunk)?;
            for p in (0..chunk.len()).step_by(T::SIZE) {
                table.push(T::from_bytes(&chunk[p..p + T::SIZE]));
            }
            remaining -= n;
        }
        // Growing a vector without knowing the final length leaves it holding
        // up to twice the entries' bytes. The excess is capacity the allocator
        // keeps for the life of the transducer.
        // [spec:hfst:req:table-residency.single-copy-load]
        table.shrink_to_fit();
        Ok(TransducerTable { table })
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

    /// The entries, borrowed.
    // [spec:hfst:req:table-residency.views-not-copies]
    pub fn as_slice(&self) -> &[T] {
        &self.table
    }

    /// The entries, moved out of the table.
    // [spec:hfst:req:table-residency.views-not-copies]
    pub fn into_vector(self) -> Vec<T> {
        self.table
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
pub trait TransducerTablesInterface: Sized {
    /// Whether this is the weighted table pair — the static counterpart of
    /// the header's 'Weighted' flag; checked against it at load time.
    const WEIGHTED: bool;
    /// Construct the one-final-index empty table pair ('TransducerTables()').
    fn new_empty() -> Self;
    /// Read both tables from a stream ('TransducerTables(istream&, ...)').
    fn read_from(
        is: &mut dyn std::io::BufRead,
        index_table_size: TransitionTableIndex,
        transition_table_size: TransitionTableIndex,
    ) -> crate::error::Result<Self>;
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
    pub fn read_from(
        is: &mut dyn std::io::BufRead,
        index_table_size: TransitionTableIndex,
        transition_table_size: TransitionTableIndex,
    ) -> crate::error::Result<Self> {
        Ok(TransducerTables {
            index_table: TransducerTable::read_from(is, index_table_size)?,
            transition_table: TransducerTable::read_from(is, transition_table_size)?,
        })
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

    /// The index table and the transition table, moved out of the pair.
    // [spec:hfst:req:table-residency.views-not-copies]
    pub fn into_tables(self) -> (TransducerTable<T1>, TransducerTable<T2>) {
        (self.index_table, self.transition_table)
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
    fn read_from(
        is: &mut dyn std::io::BufRead,
        index_table_size: TransitionTableIndex,
        transition_table_size: TransitionTableIndex,
    ) -> crate::error::Result<Self> {
        TransducerTables::read_from(is, index_table_size, transition_table_size)
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
