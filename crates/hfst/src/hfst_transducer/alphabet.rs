//! Facade metadata, alphabet maintenance, harmonization, and structural queries.

use super::*;
use crate::convert_transducer_format::ConversionFunctions;

impl<B: Backend> HfstTransducer<B> {
    // -------------------------------------------------------------------------
    // ----- Accessors -----
    // -------------------------------------------------------------------------

    /// \brief The implementation type of the transducer ('type' in C++) —
    /// now a constant of the backend type.
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-type-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-type-fn]
    pub fn get_type(&self) -> ImplementationType {
        B::TYPE
    }

    /// \brief Rename the transducer.
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.set-name-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.set-name-fn]
    pub fn set_name(&mut self, name: &str) {
        self.set_property("name", name);
    }

    /// \brief Get the name of the transducer.
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-name-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-name-fn]
    pub fn get_name(&self) -> String {
        self.get_property("name")
    }

    /// \brief Set arbitrary string property 'property' to 'name'.
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.set-property-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.set-property-fn]
    pub fn set_property(&mut self, property: &str, name: &str) {
        self.props.insert(property.to_string(), name.to_string());
        if property == "name" {
            self.name = name.to_string();
        }
    }

    /// \brief Get arbitrary string property 'property'.
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-property-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-property-fn]
    pub fn get_property(&self, property: &str) -> String {
        match self.props.get(property) {
            Some(v) => v.clone(),
            None => String::new(),
        }
    }

    /// \brief Get all properties from the transducer.
    pub fn get_properties(&self) -> &BTreeMap<String, String> {
        &self.props
    }

    // -------------------------------------------------------------------------
    // ----- Alphabet and harmonization (backend-agnostic surface) -----
    // -------------------------------------------------------------------------

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.insert-to-alphabet-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.insert-to-alphabet-fn]
    pub fn insert_to_alphabet_string(&mut self, symbol: &str) -> crate::error::Result<()> {
        if symbol.is_empty() {
            crate::bail!(EmptyString, "insert_to_alphabet");
        }

        // The C++ per-type dispatch (OL inserts directly; everything else
        // round-trips through the basic transducer) is 'Backend::insert_to_alphabet'.
        self.fst.insert_to_alphabet(symbol)
    }

    pub fn insert_to_alphabet_string_set(
        &mut self,
        symbols: &StringSet,
    ) -> crate::error::Result<()> {
        for symbol in symbols.iter() {
            if symbol.is_empty() {
                crate::bail!(EmptyString, "insert_to_alphabet");
            }
        }

        self.fst.add_symbols_to_alphabet(symbols)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.remove-from-alphabet-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.remove-from-alphabet-fn]
    pub fn remove_from_alphabet_string(&mut self, symbol: &str) -> crate::error::Result<()> {
        if symbol.is_empty() {
            crate::bail!(EmptyString, "remove_from_alphabet");
        }

        self.fst.remove_from_alphabet(symbol)
    }

    pub fn remove_from_alphabet_string_set(
        &mut self,
        symbols: &StringSet,
    ) -> crate::error::Result<()> {
        for symbol in symbols.iter() {
            self.remove_from_alphabet_string(symbol)?;
        }
        Ok(())
    }

    pub fn prune_alphabet(&mut self, force: bool) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.fst.prune_alphabet(force)?;
        Ok(self)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-alphabet-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-alphabet-fn]
    pub fn get_alphabet(&self) -> crate::error::Result<StringSet> {
        Ok(self.fst.get_alphabet())
    }

    /*
      Only harmonize number-to-symbol-encodings.
      \a another is not modifed, but a modifed copy of it is returned.
    */
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.harmonize-symbol-encodings-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.harmonize-symbol-encodings-fn]
    pub fn harmonize_symbol_encodings(&mut self, another: &HfstTransducer<B>) -> HfstTransducer<B> {
        let another_basic = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(another)
            .expect("hfst_transducer_to_hfst_basic_transducer on a valid transducer cannot fail");
        let this_basic = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(&*self)
            .expect("hfst_transducer_to_hfst_basic_transducer on a valid transducer cannot fail");
        *self = HfstTransducer::new_from_basic(&this_basic)
            .expect("converting a basic transducer to an available backend type cannot fail");
        HfstTransducer::new_from_basic(&another_basic)
            .expect("converting a basic transducer to an available backend type cannot fail")
    }

    // test function
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.print-alphabet-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.print-alphabet-fn]
    pub fn print_alphabet(&self) {
        self.fst.print_alphabet();
    }

    // -------------------------------------------------------------------------
    // ----- Missing symbols / diacritics -----
    // -------------------------------------------------------------------------

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.insert-missing-diacritics-to-alphabet-from-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.insert-missing-diacritics-to-alphabet-from-fn]
    pub fn insert_missing_diacritics_to_alphabet_from(
        &mut self,
        another: &HfstTransducer<B>,
    ) -> crate::error::Result<StringSet> {
        let this_alphabet: StringSet = self.get_alphabet()?;
        let another_alphabet: StringSet = another.get_alphabet()?;
        let mut missing_flags: StringSet = StringSet::new();

        for it in another_alphabet.iter() {
            if !this_alphabet.contains(it) && FdOperation::is_diacritic(it) {
                missing_flags.insert(it.clone());
            }
        }
        self.insert_to_alphabet_string_set(&missing_flags)?;
        Ok(missing_flags)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.insert-missing-symbols-to-alphabet-from-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.insert-missing-symbols-to-alphabet-from-fn]
    pub fn insert_missing_symbols_to_alphabet_from(
        &mut self,
        another: &HfstTransducer<B>,
        only_special_symbols: bool,
    ) -> crate::error::Result<()> {
        let this_alphabet: StringSet = self.get_alphabet()?;
        let another_alphabet: StringSet = another.get_alphabet()?;
        let mut missing_symbols: StringSet = StringSet::new();

        for it in another_alphabet.iter() {
            if !this_alphabet.contains(it) {
                if !only_special_symbols {
                    missing_symbols.insert(it.clone());
                } else {
                    if is_special_symbol(it) {
                        missing_symbols.insert(it.clone());
                    }
                }
            }
        }
        self.insert_to_alphabet_string_set(&missing_symbols)?;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // ----- Queries -----
    // -------------------------------------------------------------------------

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-cyclic-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-cyclic-fn]
    pub fn is_cyclic(&self) -> crate::error::Result<bool> {
        self.fst.is_cyclic()
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.number-of-states-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.number-of-states-fn]
    pub fn number_of_states(&self) -> u32 {
        self.fst.number_of_states()
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.number-of-arcs-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.number-of-arcs-fn]
    pub fn number_of_arcs(&self) -> u32 {
        self.fst.number_of_arcs()
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.has-weights-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.has-weights-fn]
    pub fn has_weights(&self) -> bool {
        self.fst.has_weights()
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-infinitely-ambiguous-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-infinitely-ambiguous-fn]
    pub fn is_infinitely_ambiguous(&self) -> crate::error::Result<bool> {
        self.fst.is_infinitely_ambiguous()
    }
}

impl<B: AlgebraBackend> HfstTransducer<B> {
    // -------------------------------------------------------------------------
    // ----- Harmonization -----
    // -------------------------------------------------------------------------

    /*
       Harmonize this transducer with a copy of another.
       another is not modifed, but a modified copy of it is returned.
       Flag diacritics from the alphabet of this transducer are inserted
       to the alphabet of the copy of another, so that they are excluded
       from harmonization.
       (The C++ returned NULL for foma inputs, harmonizing them not at all.
       Every backend is harmonized here, through the interchange graph. The
       Option shape is kept — callers still handle the None case.)
    */
    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.harmonize-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.harmonize-fn]
    pub fn harmonize_copy(
        &mut self,
        another: &HfstTransducer<B>,
    ) -> crate::error::Result<Option<HfstTransducer<B>>> {
        if self.anonymous && another.anonymous {
            crate::bail!(Fatal, "harmonize_copy with anonymous transducers");
        }

        // (The C++ pre-inserted flag diacritics for foma inputs only. The
        // harmonization below runs on the interchange graph, which treats
        // flags alike whatever backend the operands came from.)

        // The C++ copied 'another' before converting, because its conversion
        // consumed the source. 'to_basic' builds a fresh graph from
        // a shared reference, so the copy is a second full transducer nobody
        // reads — on a flag-harmonized operand that is gigabytes.
        let another_basic = another.to_basic()?;
        self.harmonize_onto(another_basic).map(Some)
    }

    /// [`Self::harmonize_copy`] for a caller that is finished with `another`,
    /// which is then released before the harmonized copy is built rather than
    /// standing beside it.
    pub fn harmonize_copy_owned(
        &mut self,
        another: HfstTransducer<B>,
    ) -> crate::error::Result<Option<HfstTransducer<B>>> {
        if self.anonymous && another.anonymous {
            crate::bail!(Fatal, "harmonize_copy with anonymous transducers");
        }

        let another_basic = another.to_basic()?;
        drop(another);
        self.harmonize_onto(another_basic).map(Some)
    }

    /// Harmonize this transducer against `another_basic` in place and return
    /// the matching harmonized copy of it.
    fn harmonize_onto(
        &mut self,
        mut another_basic: HfstBasicTransducer,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let mut this_basic = self.convert_to_basic_transducer()?;

        this_basic.harmonize(&mut another_basic);

        // The two graphs carry independent symbol codings; reindex both
        // onto one shared coder so that, after each is converted back to an
        // OpenFst transducer, identical symbols carry identical labels (the
        // per-graph-coder replacement for the former process-global
        // numbering on which the subsequent binary op relies). Intern every
        // symbol of BOTH graphs (coder + full alphabet) into the shared
        // coder FIRST, so even alphabet-only symbols agree before either
        // graph adopts the coding.
        let mut canonical = crate::hfst_tropical_transducer_transition_data::SymbolCoder::new();
        this_basic.intern_into(&mut canonical);
        another_basic.intern_into(&mut canonical);
        this_basic.reindex_into(&mut canonical);
        another_basic.reindex_into(&mut canonical);

        self.convert_to_hfst_transducer(this_basic)?;
        HfstTransducer::new_from_basic_owned(another_basic)
    }

    /*  Harmonize symbol-to-number encodings and expand unknown and
    identity symbols. */
    pub fn harmonize(
        &mut self,
        another: &mut HfstTransducer<B>,
        force: bool,
    ) -> crate::error::Result<()> {
        if self.anonymous && another.anonymous {
            return Ok(());
        }

        // Prevent flag diacritics from being harmonized by inserting them to
        // the alphabet.
        let this_alphabet = self.get_alphabet()?;
        let another_alphabet = another.get_alphabet()?;

        for it in another_alphabet.iter() {
            if FdOperation::is_diacritic(it) && !this_alphabet.contains(it) {
                self.insert_to_alphabet_string(it)?;
            }
        }

        for it in this_alphabet.iter() {
            if FdOperation::is_diacritic(it) && !another_alphabet.contains(it) {
                another.insert_to_alphabet_string(it)?;
            }
        }

        let _ = force;

        let mut this_basic = self.convert_to_basic_transducer()?;
        let mut another_basic = another.convert_to_basic_transducer()?;

        this_basic.harmonize(&mut another_basic);

        // Reindex both graphs onto one shared symbol coding so that, after
        // each is converted back to an OpenFst transducer, identical symbols
        // carry identical labels for the subsequent binary op (the
        // per-graph-coder replacement for the former process-global numbering).
        // Intern both graphs' symbols (coder + alphabet) into the shared
        // coder first so alphabet-only symbols agree too.
        let mut canonical = crate::hfst_tropical_transducer_transition_data::SymbolCoder::new();
        this_basic.intern_into(&mut canonical);
        another_basic.intern_into(&mut canonical);
        this_basic.reindex_into(&mut canonical);
        another_basic.reindex_into(&mut canonical);

        self.convert_to_hfst_transducer(this_basic)?;
        another.convert_to_hfst_transducer(another_basic)?;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // ----- compare, queries (HfstTransducer.cc ~1681-2663) -----
    // -------------------------------------------------------------------------

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.compare-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.compare-fn]
    pub fn compare(
        &self,
        another: &HfstTransducer<B>,
        harmonize: bool,
    ) -> crate::error::Result<bool> {
        let mut one_copy = HfstTransducer::new_copy(self)?;
        let mut another_copy = HfstTransducer::new_copy(another)?;

        /* prevent harmonization, if needed */
        if !harmonize {
            one_copy.insert_missing_symbols_to_alphabet_from(&another_copy, false)?;
            another_copy.insert_missing_symbols_to_alphabet_from(&one_copy, false)?;
        }
        /* always prevent harmonizing special symbols */
        one_copy.insert_missing_symbols_to_alphabet_from(&another_copy, true)?;
        another_copy.insert_missing_symbols_to_alphabet_from(&one_copy, true)?;

        another_copy = one_copy
            .harmonize_copy(&another_copy)?
            .expect("harmonize_copy returns Some for tropical types");

        one_copy.determinize()?;
        another_copy.determinize()?;

        // No caller configures equivalence-checking, so the former global
        // 'encode_weights' is read at its C++ default (false) here.
        one_copy.fst.are_equivalent(&another_copy.fst, false)
    }

    pub fn compare_default(&self, another: &HfstTransducer<B>) -> crate::error::Result<bool> {
        self.compare(another, true)
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.is-automaton-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.is-automaton-fn]
    pub fn is_automaton(&self) -> crate::error::Result<bool> {
        Ok(self.fst.is_automaton())
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-initial-input-symbols-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-initial-input-symbols-fn]
    pub fn get_initial_input_symbols(&self) -> StringSet {
        self.fst.get_initial_input_symbols()
    }

    // [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.get-first-input-symbols-fn]
    // [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.get-first-input-symbols-fn]
    pub fn get_first_input_symbols(&self) -> crate::error::Result<StringSet> {
        Ok(self.fst.get_first_input_symbols())
    }
}
