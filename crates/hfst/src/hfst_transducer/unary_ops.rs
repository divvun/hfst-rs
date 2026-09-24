//! Unary facade operations: determinization and minimization, repetition,
//! projection, insertion, and weight handling.

use super::*;

impl<B: Backend> HfstTransducer<B> {
    /// Return a copy with every transition labelled `symbol` (on either the
    /// input or output side) removed, surviving states renumbered. Converts to a
    /// basic transducer, applies [`HfstBasicTransducer::kill_paths`], and
    /// converts back to this transducer's type. Lifted from hfst-kill-paths.
    pub fn kill_paths(&self, symbol: &str) -> HfstTransducer<B> {
        let killed = self
            .to_basic()
            .expect("to_basic on a valid transducer cannot fail")
            .kill_paths(symbol);
        HfstTransducer::new_from_basic(&killed)
            .expect("converting a basic transducer to an available backend type cannot fail")
    }
}

// -----------------------------------------------------------------------------
// The mutable FST algebra (tropical instantiation only).
// -----------------------------------------------------------------------------

impl<B: AlgebraBackend> HfstTransducer<B> {
    // -------------------------------------------------------------------------
    // ----- Epsilon removal, determinization, minimization -----
    // -------------------------------------------------------------------------

    pub fn remove_epsilons(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false;
        self.fst = self.fst.remove_epsilons()?;
        Ok(self)
    }

    pub fn determinize(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.determinize_with_config(&EngineConfig::default())
    }

    /// 'determinize', reading 'encode_weights' (the only engine-policy flag this op
    /// consults) from the supplied config. The tropical backend encodes weights iff
    /// 'config.encode_weights'.
    pub fn determinize_with_config(
        &mut self,
        config: &EngineConfig,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false;
        let fst = std::mem::replace(&mut self.fst, B::empty());
        self.fst = fst.determinize(config.encode_weights)?;
        Ok(self)
    }

    pub fn minimize(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.minimize_with_config(&EngineConfig::default())
    }

    /// 'minimize', reading 'encode_weights' from the supplied config (see
    /// 'determinize_with_config').
    pub fn minimize_with_config(
        &mut self,
        config: &EngineConfig,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false;
        let fst = std::mem::replace(&mut self.fst, B::empty());
        self.fst = fst.minimize(config.encode_weights)?;
        Ok(self)
    }

    pub fn optimize(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.optimize_with_config(&EngineConfig::default())
    }

    pub fn optimize_with_config(
        &mut self,
        config: &EngineConfig,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        if config.minimization {
            self.minimize_with_config(config)
        } else {
            self.determinize_with_config(config)
        }
    }

    // -------------------------------------------------------------------------
    // ----- Repeat functions -----
    // -------------------------------------------------------------------------

    pub fn repeat_star(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false;
        self.fst = self.fst.repeat_star();
        Ok(self)
    }

    pub fn repeat_plus(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false;
        self.fst = self.fst.repeat_plus();
        Ok(self)
    }

    pub fn repeat_n(&mut self, n: u32) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false; // This could be done so that is_trie is preserved
        self.fst = self.fst.repeat_n(n)?;
        Ok(self)
    }

    pub fn repeat_n_plus(&mut self, n: u32) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false; // This could be done so that is_trie is preserved
        let mut a = HfstTransducer::new_copy(self)?;
        let b = HfstTransducer::new_copy(a.repeat_star()?)?;
        self.repeat_n(n)?.concatenate(&b, true)
    }

    pub fn repeat_n_minus(&mut self, n: u32) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false; // This could be done so that is_trie is preserved
        self.fst = self.fst.repeat_le_n(n)?;
        Ok(self)
    }

    pub fn repeat_n_to_k(
        &mut self,
        n: u32,
        k: u32,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false; // This could be done so that is_trie is preserved
        let mut a = HfstTransducer::new_copy(self)?;
        let b = HfstTransducer::new_copy(a.repeat_n_minus(k - n)?)?;
        self.repeat_n(n)?.concatenate(&b, true)
    }

    // -------------------------------------------------------------------------
    // ----- Unary operators -----
    // -------------------------------------------------------------------------

    pub fn optionalize(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false; // This could be done so that is_trie is preserved
        self.fst = self.fst.optionalize()?;
        Ok(self)
    }

    pub fn invert(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false; // This could be done so that is_trie is preserved
        self.fst = self.fst.invert();
        Ok(self)
    }

    pub fn reverse(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false; // This could be done so that is_trie is preserved
        self.fst = self.fst.reverse()?;
        Ok(self)
    }

    pub fn input_project(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false; // This could be done so that is_trie is preserved
        self.fst = self.fst.extract_input_language();
        Ok(self)
    }

    pub fn output_project(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false; // This could be done so that is_trie is preserved
        self.fst = self.fst.extract_output_language();
        Ok(self)
    }

    /// `[? | flag1 | ... | flagN]` — the single-symbol identity universe with
    /// `other`'s flag diacritics inserted as ORDINARY symbols (so subtract
    /// harmonization cannot erase them). This is the building block of every
    /// flag-correct complement (hfst/hfst#349): starring it and subtracting
    /// gives `~A` / `negate`, using it unstarred and subtracting gives the term
    /// complement `\A = [? - A]`. Both must treat flags as plain symbols to
    /// match the Xerox transcript, so both share this constructor.
    pub fn identity_with_flags_of(
        other: &HfstTransducer<B>,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let mut universe = HfstTransducer::new_symbol("@_IDENTITY_SYMBOL_@")?;
        // diacritics will not be harmonized in subtract
        let flags = universe.insert_missing_diacritics_to_alphabet_from(other)?;
        for flag in flags.iter() {
            let tr = HfstTransducer::new_symbol(flag)?;
            universe.disjunct(&tr, true)?;
        }
        Ok(universe)
    }

    pub fn negate(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.is_trie = false; // This could be done so that is_trie is preserved

        if !self.is_automaton()? {
            crate::bail!(TransducerIsNotAutomaton);
        }

        let mut idstar = HfstTransducer::identity_with_flags_of(self)?;
        idstar.repeat_star()?;
        idstar.minimize()?;
        idstar.subtract(self, true)?;
        *self = idstar;
        Ok(self)
    }

    pub fn n_best(&mut self, n: u32) -> crate::error::Result<&mut HfstTransducer<B>> {
        // (Same C++ round-trip through TROPICAL_OPENFST_TYPE as
        // extract_random_paths; each backend answers for itself here.)
        self.fst = self.fst.n_best(n)?;
        Ok(self)
    }

    // -------------------------------------------------------------------------
    // ----- Insert freely -----
    // -------------------------------------------------------------------------

    pub fn insert_freely_pair(
        &mut self,
        symbol_pair: &StringPair,
        harmonize: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        if symbol_pair.0.is_empty() || symbol_pair.1.is_empty() {
            crate::bail!(EmptyString, "insert_freely(const StringPair&)");
        }

        let tr = HfstTransducer::new_symbol_pair(&symbol_pair.0, &symbol_pair.1)?;
        self.insert_freely(&tr, harmonize)
    }

    pub fn insert_freely(
        &mut self,
        tr: &HfstTransducer<B>,
        harmonize: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        /* In this function, this transducer must always be harmonized
        according to tr, not the other way round. */
        // foma or no harmonization -> use our own copy of tr.
        let tr_harmonized: HfstTransducer<B> = match if harmonize {
            self.harmonize_copy(tr)?
        } else {
            None
        } {
            Some(h) => h,
            None => HfstTransducer::new_copy(tr)?,
        };

        let mut net = self.fst.to_basic()?;
        let substituting_net = tr_harmonized.fst.to_basic()?;

        net.insert_freely_graph(&substituting_net)?;
        self.fst = B::from_basic(&net)?;
        Ok(self)
    }

    // -------------------------------------------------------------------------
    // ----- Weight handling -----
    // -------------------------------------------------------------------------

    pub fn set_final_weights(
        &mut self,
        weight: f32,
        increment: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.fst = self.fst.set_final_weights(weight, increment);
        Ok(self)
    }

    pub fn push_labels(
        &mut self,
        push_type: PushType,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        let to_initial_state = push_type == PushType::TO_INITIAL_STATE;
        self.fst = self.fst.push_labels(to_initial_state)?;
        Ok(self)
    }

    /// Realign a transducer by pushing its labels to the start on both sides:
    /// invert, push labels to the initial state, invert back, and push again.
    /// Lifted verbatim from hfst-realign (the boundary-symbol variant is dead /
    /// commented out in the C++; this is the only realignment it performs).
    pub fn realign(&mut self) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.invert()?;
        self.push_labels(PushType::TO_INITIAL_STATE)?;
        self.invert()?;
        self.push_labels(PushType::TO_INITIAL_STATE)
    }

    pub fn push_weights(
        &mut self,
        push_type: PushType,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        let to_initial_state = push_type == PushType::TO_INITIAL_STATE;
        self.fst = self.fst.push_weights(to_initial_state)?;
        Ok(self)
    }

    pub fn transform_weights(
        &mut self,
        func: fn(f32) -> f32,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        self.fst = self.fst.transform_weights(func);
        Ok(self)
    }
}

// -----------------------------------------------------------------------------
// Tropical-only operations (the C++ converted other types to
// TROPICAL_OPENFST_TYPE first; cross-type callers now do the typed conversion
// themselves).
// -----------------------------------------------------------------------------

impl HfstTransducer<StdVectorFst> {
    pub fn prune(&mut self) -> crate::error::Result<&mut HfstTransducer<StdVectorFst>> {
        self.fst = TropicalWeightTransducer::prune(&self.fst)?;
        Ok(self)
    }
}
