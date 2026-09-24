//! Loading, validating, writing and copying out the table pair.

use super::*;

impl<T: TransducerTablesInterface> Transducer<T> {
    // [spec:hfst:def:transducer.hfst-ol.transducer.copy-windex-table-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.copy-windex-table-fn]
    pub fn copy_windex_table(&self) -> crate::error::Result<TransducerTable<TransitionWIndex>> {
        if !self.get_header().probe_flag(HeaderFlag::Weighted) {
            crate::bail!(TransducerHasWrongType);
        }
        let mut another = TransducerTable::new();
        for i in 0..self.get_header().index_table_size() {
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
        if !self.get_header().probe_flag(HeaderFlag::Weighted) {
            crate::bail!(TransducerHasWrongType);
        }
        let mut another = TransducerTable::new();
        for i in 0..self.get_header().target_table_size() {
            another.append(TransitionW::new_values(
                self.tbl().get_transition_input(i),
                self.tbl().get_transition_output(i),
                self.tbl().get_transition_target(i),
                self.tbl().get_weight(i),
            ));
        }
        Ok(another)
    }

    /// The weighted table pair, moved out of a transducer the caller owns.
    ///
    /// What [`Self::copy_windex_table`] and [`Self::copy_transitionw_table`]
    /// produce between them, without either copy. Their entrywise rebuild
    /// reads back every field it writes and is bounded by the header's size
    /// fields, which every constructor keeps in step with the tables' lengths,
    /// so the two answers are the same table pair.
    // [spec:hfst:req:table-residency.single-copy-load]
    pub fn into_weighted_tables(self) -> crate::error::Result<T> {
        if !self.get_header().probe_flag(HeaderFlag::Weighted) {
            crate::bail!(TransducerHasWrongType);
        }
        Ok(self
            .tables
            .expect("tables are initialized during container load"))
    }

    // [spec:hfst:def:transducer.hfst-ol.transducer.copy-index-table-fn]
    // [spec:hfst:sem:transducer.hfst-ol.transducer.copy-index-table-fn]
    pub fn copy_index_table(&self) -> crate::error::Result<TransducerTable<TransitionIndex>> {
        if self.get_header().probe_flag(HeaderFlag::Weighted) {
            crate::bail!(TransducerHasWrongType);
        }
        let mut another = TransducerTable::new();
        for i in 0..self.get_header().index_table_size() {
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
        if self.get_header().probe_flag(HeaderFlag::Weighted) {
            crate::bail!(TransducerHasWrongType);
        }
        let mut another = TransducerTable::new();
        for i in 0..self.get_header().target_table_size() {
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
    pub fn load_tables(&mut self, is: &mut dyn std::io::BufRead) -> crate::error::Result<()> {
        if self.get_header().probe_flag(HeaderFlag::Weighted) != T::WEIGHTED {
            crate::bail!(TransducerHasWrongType);
        }
        let its = self.get_header().index_table_size();
        let tts = self.get_header().target_table_size();
        self.tables = Some(T::read_from(is, its, tts)?);
        self.validate_tables()
    }

    /// Reject a table pair whose symbols or targets point outside the tables
    /// and the alphabet they were read alongside. Run once at the stream
    /// boundary; see [`validate_ol_index_entry`].
    fn validate_tables(&self) -> crate::error::Result<()> {
        let index_len = self.get_header().index_table_size();
        let transition_len = self.get_header().target_table_size();
        let symbol_count = self.get_alphabet().get_symbol_table().len();
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
        self.get_alphabet().write(os);
        let weighted = self.get_header().probe_flag(HeaderFlag::Weighted);
        // 'i' is already a TransitionTableIndex (u32), so no narrowing occurs.
        for i in 0..self.get_header().index_table_size() {
            self.tbl().write_index(i, os, weighted);
        }
        for i in 0..self.get_header().target_table_size() {
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
        self.get_header().display();
        self.get_alphabet().display();
        self.tbl().display();
        println!("-------------------------------------------------");
    }
}
