//! Facade symbol and symbol-pair substitution.

use super::*;

impl<B: AlgebraBackend> HfstTransducer<B> {
    // -------------------------------------------------------------------------
    // ----- Substitution functions -----
    // -------------------------------------------------------------------------

    pub fn substitute_with_func(
        &mut self,
        func: impl Fn(&StringPair, &mut StringPairSet) -> bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        let mut net = self.convert_to_basic_transducer()?;
        net.substitute_with_func(func)?;
        self.convert_to_hfst_transducer(net)
    }

    pub fn substitute_string(
        &mut self,
        old_symbol: &str,
        new_symbol: &str,
        input_side: bool,
        output_side: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        // empty strings are not accepted
        if old_symbol.is_empty() || new_symbol.is_empty() {
            crate::bail!(
                EmptyString,
                "substitute(const std::string&, const std::string&, bool, bool)"
            );
        }

        // if there are implementations available, use them: the per-backend
        // both-sides fast path (dead code for tropical — 'if (false && ...)') is
        // 'AlgebraBackend::substitute_symbol_fast'.
        if input_side
            && output_side
            && let Some(tmp) = self.fst.substitute_symbol_fast(old_symbol, new_symbol)
        {
            self.fst = tmp;
            return Ok(self);
        }

        // use the default HfstBasicTransducer function
        let mut net = self.convert_to_basic_transducer()?;
        net.substitute_symbol(
            &Symbol::new(old_symbol),
            &Symbol::new(new_symbol),
            input_side,
            output_side,
        )?;
        self.convert_to_hfst_transducer(net)
    }

    pub fn substitute_pair_with_pair(
        &mut self,
        old_symbol_pair: &StringPair,
        new_symbol_pair: &StringPair,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        // empty strings are not accepted
        if old_symbol_pair.0.is_empty()
            || old_symbol_pair.1.is_empty()
            || new_symbol_pair.0.is_empty()
            || new_symbol_pair.1.is_empty()
        {
            crate::bail!(
                EmptyString,
                "substitute(const StringPair&, const StringPair&)"
            );
        }

        let mut net = self.convert_to_basic_transducer()?;
        net.substitute_pair(old_symbol_pair, new_symbol_pair)?;
        self.convert_to_hfst_transducer(net)?;
        Ok(self)
    }

    pub fn substitute_pair_with_pair_set(
        &mut self,
        old_symbol_pair: &StringPair,
        new_symbol_pair_set: &StringPairSet,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        if old_symbol_pair.0.is_empty() || old_symbol_pair.1.is_empty() {
            crate::bail!(
                EmptyString,
                "substitute(const StringPair&, const StringPairSet&"
            );
        }

        let mut net = self.convert_to_basic_transducer()?;
        net.substitute_pair_with_set(old_symbol_pair, new_symbol_pair_set)?;
        self.convert_to_hfst_transducer(net)
    }

    pub fn substitute_symbol_substitutions(
        &mut self,
        substitutions: &HfstSymbolSubstitutions,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        let mut net = self.convert_to_basic_transducer()?;

        net.substitute_symbol_substitutions(substitutions);

        self.convert_to_hfst_transducer(net)
    }

    pub fn substitute_symbol_pair_substitutions(
        &mut self,
        substitutions: &HfstSymbolPairSubstitutions,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        let mut net = self.convert_to_basic_transducer()?;
        net.substitute_symbol_pair_substitutions(substitutions);
        self.convert_to_hfst_transducer(net)
    }

    pub fn substitute_pair_with_transducer(
        &mut self,
        symbol_pair: &StringPair,
        transducer: &mut HfstTransducer<B>,
        harmonize: bool,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        if symbol_pair.0.is_empty() || symbol_pair.1.is_empty() {
            crate::bail!(
                EmptyString,
                "substitute(const StringPair&, HfstTransducer&)"
            );
        }

        let mut pair_transducer = HfstTransducer::new_symbol_pair(&symbol_pair.0, &symbol_pair.1)?;
        if !harmonize {
            self.insert_missing_symbols_to_alphabet_from(&pair_transducer, false)?;
            pair_transducer.insert_missing_symbols_to_alphabet_from(self, false)?;
        }
        self.insert_missing_symbols_to_alphabet_from(&pair_transducer, true)?;
        pair_transducer.insert_missing_symbols_to_alphabet_from(self, true)?;

        self.harmonize(&mut pair_transducer, false)?;

        if !harmonize {
            self.insert_missing_symbols_to_alphabet_from(transducer, false)?;
            transducer.insert_missing_symbols_to_alphabet_from(self, false)?;
        }
        self.insert_missing_symbols_to_alphabet_from(transducer, true)?;
        transducer.insert_missing_symbols_to_alphabet_from(self, true)?;

        self.harmonize(transducer, false)?;

        self.fst = self
            .fst
            .substitute_string_transducer(symbol_pair.clone(), &transducer.fst);
        Ok(self)
    }

    /// Apply a set of label substitutions by composition — the `--compose` path
    /// of hfst-substitute. `substitutions` is the disjunction of the from:to
    /// symbol pairs to apply. Builds `(substitutions ∪ (identity − input(
    /// substitutions)))*` — the substitutions plus a pass-through identity for
    /// every symbol they do not rewrite — then composes it onto the right of
    /// `self`, minimises, and composes the inverse onto the left. Lifted verbatim
    /// from hfst-substitute's perform_delayed.
    pub fn substitute_by_composition(
        &mut self,
        substitutions: &HfstTransducer<B>,
    ) -> crate::error::Result<&mut HfstTransducer<B>> {
        let mut subs = substitutions.clone();
        let mut sigma_minus_subs = HfstTransducer::new_symbol_pair(
            crate::hfst_symbol_defs::internal_identity,
            crate::hfst_symbol_defs::internal_identity,
        )?;
        let mut subs_in = substitutions.clone();
        subs_in.input_project()?;
        sigma_minus_subs.subtract(&subs_in, true)?;
        subs.disjunct(&sigma_minus_subs, true)?;
        subs.repeat_star()?;
        // Compose on the right, minimise, then compose the inverse on the left
        // (C++: trans = substitution_trans->compose(trans)).
        self.compose(&subs, true)?;
        self.minimize()?;
        subs.invert()?;
        subs.compose(&*self, true)?;
        *self = subs;
        self.minimize()?;
        Ok(self)
    }
}
