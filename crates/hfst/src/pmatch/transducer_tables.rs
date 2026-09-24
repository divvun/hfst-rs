//! Reading a PmatchTransducer's tables and walking their entries.

use super::*;

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
