//! The OtherSymbolTransducer 'apply()' family and the convenience shims over it.

use super::*;

impl<B: AlgebraBackend> OtherSymbolTransducer<B> {
    // -------------------------------------------------------------------------
    // ----- apply() family (member-fn-ptr dispatch flattened) -----
    //
    // Each guards on empty symbol_pairs / is_broken, applies the op, then
    // minimizes (exactly as the C++ 'CALL_MEMBER_FN(...); minimize();').
    // -------------------------------------------------------------------------

    /// True iff there are configured diacritics.
    fn config_has_diacritics(cfg: &OstConfig) -> bool {
        !cfg.diacritics.is_empty()
    }

    /// 'apply(HfstTransducerZeroArgMember p)' — apply a zero-arg 'HfstTransducer'
    /// op then minimize. The C++ member-fn-pointer becomes a closure.
    pub fn apply_zero<F>(&mut self, cfg: &OstConfig, p: F) -> crate::error::Result<&mut Self>
    where
        F: FnOnce(&mut HfstTransducer<B>) -> crate::error::Result<()>,
    {
        if cfg.symbol_pairs.is_empty() {
            crate::bail!(EmptySymbolPairSet);
        }
        if self.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        p(&mut self.transducer)?;
        self.transducer.minimize()?;
        Ok(self)
    }

    /// 'apply(const HfstTransducerOneArgMember, const OtherSymbolTransducer&)'.
    ///
    /// Harmonizes the diacritics of '*this' and a copy of 'another' (when there
    /// are diacritics), applies the binary op against the copy, then minimizes.
    /// The C++ facade ops default to 'harmonize = true', which the closure
    /// passes through.
    pub fn apply_one<F>(
        &mut self,
        cfg: &OstConfig,
        p: F,
        another: &OtherSymbolTransducer<B>,
    ) -> crate::error::Result<&mut Self>
    where
        F: FnOnce(&mut HfstTransducer<B>, &HfstTransducer<B>) -> crate::error::Result<()>,
    {
        if cfg.symbol_pairs.is_empty() {
            crate::bail!(EmptySymbolPairSet);
        }
        if self.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        if another.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        // [spec:hfst:def:other-symbol-transducer.another-copy-fn]
        // [spec:hfst:sem:other-symbol-transducer.another-copy-fn]
        let mut another_copy = another.clone();
        if Self::config_has_diacritics(cfg) {
            self.harmonize_diacritics(cfg, &mut another_copy);
            another_copy.harmonize_diacritics(cfg, self);
        }
        p(&mut self.transducer, &another_copy.transducer)?;
        self.transducer.minimize()?;
        Ok(self)
    }

    /// 'apply(const HfstTransducerBoolArgMember, const OtherSymbolTransducer&)'.
    ///
    /// Like ['apply_one'] but the closure carries the trailing 'bool' (the C++
    /// passes 'true').
    pub fn apply_one_bool<F>(
        &mut self,
        cfg: &OstConfig,
        p: F,
        another: &OtherSymbolTransducer<B>,
    ) -> crate::error::Result<&mut Self>
    where
        F: FnOnce(&mut HfstTransducer<B>, &HfstTransducer<B>, bool) -> crate::error::Result<()>,
    {
        if cfg.symbol_pairs.is_empty() {
            crate::bail!(EmptySymbolPairSet);
        }
        if self.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        if another.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        let mut another_copy = another.clone();
        if Self::config_has_diacritics(cfg) {
            self.harmonize_diacritics(cfg, &mut another_copy);
            another_copy.harmonize_diacritics(cfg, self);
        }
        p(&mut self.transducer, &another_copy.transducer, true)?;
        self.transducer.minimize()?;
        Ok(self)
    }

    /// 'bool apply(const HfstTransducerOneArgMemberBool,
    ///  const OtherSymbolTransducer&) const'.
    ///
    /// Runs the predicate against copies of both transducers and returns its
    /// result (no minimize; the C++ overload is 'const').
    // [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.apply-fn]
    // [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.apply-fn]
    pub fn apply_one_bool_ret<F>(
        &self,
        cfg: &OstConfig,
        p: F,
        another: &OtherSymbolTransducer<B>,
    ) -> crate::error::Result<bool>
    where
        F: FnOnce(&mut HfstTransducer<B>, &HfstTransducer<B>) -> bool,
    {
        if cfg.symbol_pairs.is_empty() {
            crate::bail!(EmptySymbolPairSet);
        }
        if self.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        if another.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        let mut copy = self.clone();
        let another_copy = another.clone();
        Ok(p(&mut copy.transducer, &another_copy.transducer))
    }

    /// 'apply(const HfstTransducerOneNumArgMember, unsigned int number)'.
    pub fn apply_num<F>(
        &mut self,
        cfg: &OstConfig,
        p: F,
        number: u32,
    ) -> crate::error::Result<&mut Self>
    where
        F: FnOnce(&mut HfstTransducer<B>, u32) -> crate::error::Result<()>,
    {
        self.apply_zero(cfg, |t| p(t, number))
    }

    /// 'apply(const HfstTransducerTwoNumArgMember, unsigned int, unsigned int)'.
    pub fn apply_two_num<F>(
        &mut self,
        cfg: &OstConfig,
        p: F,
        num1: u32,
        num2: u32,
    ) -> crate::error::Result<&mut Self>
    where
        F: FnOnce(&mut HfstTransducer<B>, u32, u32) -> crate::error::Result<()>,
    {
        self.apply_zero(cfg, |t| p(t, num1, num2))
    }

    /// 'apply(const HfstTransducerOneSymbolPairArgMember, const SymbolPair&)'.
    pub fn apply_symbol_pair<F>(
        &mut self,
        cfg: &OstConfig,
        p: F,
        pair: &SymbolPair,
    ) -> crate::error::Result<&mut Self>
    where
        F: FnOnce(&mut HfstTransducer<B>, &SymbolPair) -> crate::error::Result<()>,
    {
        self.apply_zero(cfg, |t| p(t, pair))
    }

    /// 'apply(const HfstTransducerOneSymbolPairBoolArgMember,
    ///  const SymbolPair&, bool)'.
    pub fn apply_symbol_pair_bool<F>(
        &mut self,
        cfg: &OstConfig,
        p: F,
        pair: &SymbolPair,
        b: bool,
    ) -> crate::error::Result<&mut Self>
    where
        F: FnOnce(&mut HfstTransducer<B>, &SymbolPair, bool) -> crate::error::Result<()>,
    {
        self.apply_zero(cfg, |t| p(t, pair, b))
    }

    /// 'apply(const HfstTransducerSubstMember, const std::string&,
    ///  const std::string&, bool, bool)' — 'substitute(str1, str2, b1, b2)'.
    pub fn apply_subst(
        &mut self,
        cfg: &OstConfig,
        str1: &str,
        str2: &str,
        b1: bool,
        b2: bool,
    ) -> crate::error::Result<&mut Self> {
        if cfg.symbol_pairs.is_empty() {
            crate::bail!(EmptySymbolPairSet);
        }
        if self.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        self.transducer.substitute_string(str1, str2, b1, b2)?;
        self.transducer.minimize()?;
        Ok(self)
    }

    /// 'apply(const HfstTransducerSubstPairMember, const SymbolPair&,
    ///  const SymbolPair&)' — 'substitute(pair1, pair2)'.
    pub fn apply_subst_pair(
        &mut self,
        cfg: &OstConfig,
        p1: &SymbolPair,
        p2: &SymbolPair,
    ) -> crate::error::Result<&mut Self> {
        if cfg.symbol_pairs.is_empty() {
            crate::bail!(EmptySymbolPairSet);
        }
        if self.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        self.transducer.substitute_pair_with_pair(p1, p2)?;
        self.transducer.minimize()?;
        Ok(self)
    }

    /// 'apply(const HfstTransducerSubstPairFstMember, const SymbolPair&,
    ///  const OtherSymbolTransducer&, bool)' —
    /// 'substitute(pair, t_copy.transducer, b)'.
    // [spec:hfst:def:other-symbol-transducer.t-copy-fn]
    // [spec:hfst:sem:other-symbol-transducer.t-copy-fn]
    pub fn apply_subst_pair_fst(
        &mut self,
        cfg: &OstConfig,
        p1: &SymbolPair,
        t: &OtherSymbolTransducer<B>,
        b: bool,
    ) -> crate::error::Result<&mut Self> {
        if cfg.symbol_pairs.is_empty() {
            crate::bail!(EmptySymbolPairSet);
        }
        if self.is_broken {
            crate::bail!(UndefinedSymbolPairsFound);
        }
        let mut t_copy = t.clone();
        self.transducer
            .substitute_pair_with_transducer(p1, &mut t_copy.transducer, b)?;
        self.transducer.minimize()?;
        Ok(self)
    }

    // -------------------------------------------------------------------------
    // ----- Concrete convenience shims (readable call sites for the rules) -----
    // -------------------------------------------------------------------------

    /// 'apply(&HfstTransducer::disjunct, another)'.
    pub fn disjunct(
        &mut self,
        cfg: &OstConfig,
        another: &OtherSymbolTransducer<B>,
    ) -> crate::error::Result<&mut Self> {
        self.apply_one(
            cfg,
            |t, o| {
                t.disjunct(o, true)?;
                Ok(())
            },
            another,
        )
    }

    /// 'apply(&HfstTransducer::intersect, another)'.
    pub fn intersect(
        &mut self,
        cfg: &OstConfig,
        another: &OtherSymbolTransducer<B>,
    ) -> crate::error::Result<&mut Self> {
        self.apply_one(
            cfg,
            |t, o| {
                t.intersect(o, true)?;
                Ok(())
            },
            another,
        )
    }

    /// 'apply(&HfstTransducer::subtract, another)'.
    pub fn subtract(
        &mut self,
        cfg: &OstConfig,
        another: &OtherSymbolTransducer<B>,
    ) -> crate::error::Result<&mut Self> {
        self.apply_one(
            cfg,
            |t, o| {
                t.subtract(o, true)?;
                Ok(())
            },
            another,
        )
    }

    /// 'apply(&HfstTransducer::concatenate, another)'.
    pub fn concatenate(
        &mut self,
        cfg: &OstConfig,
        another: &OtherSymbolTransducer<B>,
    ) -> crate::error::Result<&mut Self> {
        self.apply_one(
            cfg,
            |t, o| {
                t.concatenate(o, true)?;
                Ok(())
            },
            another,
        )
    }

    /// 'apply(&HfstTransducer::compose, another)'.
    pub fn compose(
        &mut self,
        cfg: &OstConfig,
        another: &OtherSymbolTransducer<B>,
    ) -> crate::error::Result<&mut Self> {
        self.apply_one(
            cfg,
            |t, o| {
                t.compose(o, true)?;
                Ok(())
            },
            another,
        )
    }

    /// 'apply(&HfstTransducer::insert_freely, another)' (bool-arg overload).
    pub fn insert_freely(
        &mut self,
        cfg: &OstConfig,
        another: &OtherSymbolTransducer<B>,
    ) -> crate::error::Result<&mut Self> {
        self.apply_one_bool(
            cfg,
            |t, o, h| {
                t.insert_freely(o, h)?;
                Ok(())
            },
            another,
        )
    }

    /// 'apply(&HfstTransducer::repeat_star)'.
    pub fn repeat_star(&mut self, cfg: &OstConfig) -> crate::error::Result<&mut Self> {
        self.apply_zero(cfg, |t| {
            t.repeat_star()?;
            Ok(())
        })
    }

    /// 'apply(&HfstTransducer::minimize)'.
    pub fn minimize(&mut self, cfg: &OstConfig) -> crate::error::Result<&mut Self> {
        self.apply_zero(cfg, |t| {
            t.minimize()?;
            Ok(())
        })
    }

    /// 'apply(&HfstTransducer::optionalize)'.
    pub fn optionalize(&mut self, cfg: &OstConfig) -> crate::error::Result<&mut Self> {
        self.apply_zero(cfg, |t| {
            t.optionalize()?;
            Ok(())
        })
    }

    /// 'apply(&HfstTransducer::invert)'.
    pub fn invert(&mut self, cfg: &OstConfig) -> crate::error::Result<&mut Self> {
        self.apply_zero(cfg, |t| {
            t.invert()?;
            Ok(())
        })
    }

    /// 'apply(&HfstTransducer::input_project)'.
    pub fn input_project(&mut self, cfg: &OstConfig) -> crate::error::Result<&mut Self> {
        self.apply_zero(cfg, |t| {
            t.input_project()?;
            Ok(())
        })
    }

    /// 'apply(&HfstTransducer::output_project)'.
    pub fn output_project(&mut self, cfg: &OstConfig) -> crate::error::Result<&mut Self> {
        self.apply_zero(cfg, |t| {
            t.output_project()?;
            Ok(())
        })
    }

    /// 'apply(&HfstTransducer::repeat_n, n)'.
    pub fn repeat_n(&mut self, cfg: &OstConfig, n: u32) -> crate::error::Result<&mut Self> {
        self.apply_num(
            cfg,
            |t, n| {
                t.repeat_n(n)?;
                Ok(())
            },
            n,
        )
    }

    /// 'apply(&HfstTransducer::repeat_n_to_k, n, k)'.
    pub fn repeat_n_to_k(
        &mut self,
        cfg: &OstConfig,
        n: u32,
        k: u32,
    ) -> crate::error::Result<&mut Self> {
        self.apply_two_num(
            cfg,
            |t, n, k| {
                t.repeat_n_to_k(n, k)?;
                Ok(())
            },
            n,
            k,
        )
    }

    /// Replace the diamond pair '(DIAMOND, DIAMOND)' with the HFST epsilon on
    /// both sides: 'substitute(DIAMOND, HFST_EPSILON, true, true)'.
    pub fn substitute_diamond_to_epsilon(
        &mut self,
        cfg: &OstConfig,
    ) -> crate::error::Result<&mut Self> {
        self.apply_subst(cfg, TWOLC_DIAMOND, HFST_EPSILON, true, true)
    }
}
