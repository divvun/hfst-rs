//! Evaluation of twolc rule centers, contexts and regexes into
//! 'OtherSymbolTransducer's.

use super::compiler::substitute_symbol;
use super::grammar::clone_ost;
use super::*;

impl<B: AlgebraBackend> TwolcCompiler<B> {
    /// Evaluate a rule center. A 'Pair' center becomes a ['SymbolPairVector']
    /// (one entry per alternative); a 'Regex' center is evaluated to an
    /// ['OtherSymbolTransducer']. Variable assignments substitute symbol names.
    pub fn eval_center(
        &mut self,
        cfg: &OstConfig,
        center: &RuleCenter,
        vvm: &VariableValueMap,
    ) -> crate::error::Result<CenterEval<B>> {
        Ok(match center {
            RuleCenter::Pair(pairs) => {
                // A wildcard side ('a:', ':b', 'a:?') becomes the internal
                // 'Any' marker, which ['new_pair'] expands over the declared
                // alphabet — upstream's 'CENTER_SYMBOL: QUESTION_MARK' rewrite
                // to '__HFST_TWOLC_?' ('htwolcpre3-parser.yy'). A named side is
                // an ordinary symbol, so an escaped '%?' stays the literal
                // one-character symbol '?' and is NOT the wildcard.
                let side = |s: &CenterSide| match s {
                    CenterSide::Any => Symbol::new_static(TWOLC_UNKNOWN),
                    CenterSide::Symbol(sym) => substitute_symbol(sym, vvm),
                };
                let mut spv: SymbolPairVector = Vec::new();
                for p in pairs {
                    let (up, lo) = (side(&p.value.upper.value), side(&p.value.lower.value));
                    // A center side may name a set, as in 'Cns:0 <=> ...'.
                    // Upstream's CENTER_PAIR runs every center through
                    // 'Alphabet::get_symbol_pair_vector', so a set reaches rule
                    // construction already expanded into its licensed pairs,
                    // one subrule each.
                    if self.sets.contains_key(up.as_str()) || self.sets.contains_key(lo.as_str()) {
                        let expanded = self.licensed_pairs(cfg, &up, &lo);
                        if expanded.is_empty() {
                            // Upstream silently drops such a rule. It cannot
                            // mean anything, so it is an error here.
                            let site = PairSite {
                                pair: p.span.range.clone(),
                                upper: p.value.upper.span.range.clone(),
                                lower: p.value.lower.span.range.clone(),
                            };
                            self.report_empty_pair_set(cfg, &up, &lo, &site, PairUse::Centre);
                            crate::bail!(EmptySymbolPairSet);
                        }
                        spv.extend(expanded);
                    } else {
                        spv.push((up, lo));
                    }
                }
                CenterEval::Pairs(spv)
            }
            RuleCenter::Regex(e) => CenterEval::Regex(self.eval_regex_with_vars(cfg, e, vvm)?),
        })
    }

    /// Evaluate the positive and negative contexts of a rule into one
    /// ['OtherSymbolTransducerVector']. Each positive context is 'X D ?* D Y';
    /// each negative context is the same, negated ('?* - context'). The C++
    /// negative contexts ('except' clauses) are negated before being added.
    pub fn eval_contexts(
        &mut self,
        cfg: &OstConfig,
        pos: &[RuleContext],
        neg: &[RuleContext],
        vvm: &VariableValueMap,
    ) -> crate::error::Result<OtherSymbolTransducerVector<B>> {
        let mut result: OtherSymbolTransducerVector<B> = pos
            .iter()
            .map(|ctx| self.eval_context(cfg, ctx, vvm))
            .collect::<crate::error::Result<_>>()?;
        for ctx in neg {
            let mut c = self.eval_context(cfg, ctx, vvm)?;
            c.negated(cfg)?;
            result.push(c);
        }
        Ok(result)
    }

    /// Evaluate one ['RuleContext'] into 'left D ?* D right' via
    /// ['OtherSymbolTransducer::get_context'].
    pub fn eval_context(
        &mut self,
        cfg: &OstConfig,
        ctx: &RuleContext,
        vvm: &VariableValueMap,
    ) -> crate::error::Result<OtherSymbolTransducer<B>> {
        let mut left = self.eval_regex_with_vars(cfg, &ctx.left, vvm)?;
        let mut right = self.eval_regex_with_vars(cfg, &ctx.right, vvm)?;
        OtherSymbolTransducer::get_context(cfg, &mut left, &mut right)
    }

    /// The member list of set 'sym', or the singleton '[sym]' when 'sym' names
    /// no set ('Alphabet::define_singleton_set').
    pub(super) fn set_of(&self, sym: &str) -> Vec<Symbol> {
        match self.sets.get(sym) {
            Some(members) => members.iter().map(|m| Symbol::new(m.as_str())).collect(),
            None => vec![Symbol::new(sym)],
        }
    }

    // [spec:hfst:def:alphabet.alphabet.is-pair-fn]
    // [spec:hfst:sem:alphabet.alphabet.is-pair-fn]
    //
    // 'Alphabet::is_pair' ('alphabet_src/Alphabet.cc'): whether the concrete
    // 'input:output' is licensed by the (completed) alphabet.
    fn is_pair(&self, cfg: &OstConfig, input: &str, output: &str) -> bool {
        if input == TWOLC_UNKNOWN && output == TWOLC_UNKNOWN {
            return true;
        }
        if cfg.diacritics.contains(input) && input == output {
            return true;
        }
        if cfg.diacritics.contains(input) && output == TWOLC_UNKNOWN {
            return true;
        }
        if input == TWOLC_UNKNOWN {
            return cfg.output_symbols.contains(output);
        }
        if output == TWOLC_UNKNOWN {
            return cfg.input_symbols.contains(input);
        }
        cfg.symbol_pairs
            .contains(&(Symbol::new(input), Symbol::new(output)))
    }

    // 'Alphabet::compute' ('alphabet_src/Alphabet.cc'): expand a pair whose
    // sides may be set names into the disjunction of the matching declared
    // alphabet pairs ('a set pair X:Y contains every declared pair x:y with
    // x in X and y in Y'). Sides that are plain symbols act as singleton
    // sets; an unknown ('?') side expands through 'new_pair'. An empty
    // result reproduces the htwolcpre3 'The pair set ... is empty.'
    // semantic error, which terminated compilation.
    fn pair_transducer(
        &mut self,
        cfg: &OstConfig,
        input: &str,
        output: &str,
        site: &PairSite,
    ) -> crate::error::Result<OtherSymbolTransducer<B>> {
        if cfg.diacritics.contains(input) {
            if input != output && output != TWOLC_EPSILON && output != TWOLC_UNKNOWN {
                self.report_diacritic_pair(input, output, site);
            }
            return OtherSymbolTransducer::new_pair(cfg, input, input);
        }
        let mut pair_transducer = OtherSymbolTransducer::new(cfg)?;
        for (x, y) in self.licensed_pairs(cfg, input, output) {
            let pair = OtherSymbolTransducer::new_pair(cfg, &x, &y)?;
            pair_transducer.disjunct(cfg, &pair)?;
        }
        if pair_transducer.is_empty() {
            self.report_empty_pair_set(cfg, input, output, site, PairUse::Context);
            crate::bail!(EmptySymbolPairSet);
        }
        Ok(pair_transducer)
    }

    /// The concrete pairs 'input:output' denotes: every licensed 'x:y' with x
    /// in the set named by 'input' and y in the set named by 'output'. A side
    /// naming no set is its own singleton set.
    fn licensed_pairs(&self, cfg: &OstConfig, input: &str, output: &str) -> SymbolPairVector {
        let mut pairs = SymbolPairVector::new();
        for x in self.set_of(input) {
            for y in self.set_of(output) {
                if self.is_pair(cfg, &x, &y) {
                    pairs.push((x.clone(), y));
                }
            }
        }
        pairs
    }

    /// htwolcpre3-parser's 'PAIR' production special-cases a pair whose INPUT
    /// side is the grammar's bare '#': it denotes BOTH the absolute word
    /// boundary and the relative one — '[.#.:.#. | #:output]'. The absolute
    /// boundary is the word edge the context framing wraps every rule in; the
    /// relative '#' is the user-visible boundary symbol with whatever surface
    /// the rule wrote. Without the split, a context ending in '#' fails to
    /// match the word edge (and fails to FORBID it on the other rule
    /// direction), which is exactly how 29 lang-sme boundary rules diverged
    /// from C++. The bare '#' arrives as the [`TWOLC_HASH`] marker (the
    /// nfst-twolc lexer performs htwolcpre1's rename); an escaped '%#'
    /// arrives as the plain '#' symbol and stays a pure literal, no split —
    /// like C++. Alphabet completion supplies the relative-boundary pairs
    /// even when '#' was not explicitly declared.
    fn boundary_pair_transducer(
        &mut self,
        cfg: &OstConfig,
        output: &str,
        site: &PairSite,
    ) -> crate::error::Result<OtherSymbolTransducer<B>> {
        // C++: 'alt_wb = ("#", $3 == __HFST_TWOLC_# ? "#" : $3)' — a bare-'#'
        // output side realizes as the relative boundary symbol itself.
        let output = if output == TWOLC_HASH { "#" } else { output };
        let mut wb = OtherSymbolTransducer::new_pair(cfg, "__HFST_TWOLC_.#.", "__HFST_TWOLC_.#.")?;
        let alt = if self.sets.contains_key(output) {
            self.pair_transducer(cfg, "#", output, site)?
        } else {
            OtherSymbolTransducer::new_pair(cfg, "#", output)?
        };
        wb.disjunct(cfg, &alt)?;
        Ok(wb)
    }

    /// Evaluate a ['TwolcRegex'] with no variable substitution (used by the
    /// 'Definitions' section, which is evaluated before any rule expansion).
    pub fn eval_regex(
        &mut self,
        cfg: &OstConfig,
        e: &Spanned<TwolcRegex>,
    ) -> crate::error::Result<OtherSymbolTransducer<B>> {
        let vvm = VariableValueMap::new();
        self.eval_regex_with_vars(cfg, e, &vvm)
    }

    /// Evaluate a ['TwolcRegex'], substituting any variable symbol with its
    /// assigned value. Mirrors the 'xre.rs' 'eval'/'eval_unary'/'eval_binary'
    /// recursion shape, but over the smaller twolc regex sublanguage and
    /// building ['OtherSymbolTransducer']s.
    fn eval_regex_with_vars(
        &mut self,
        cfg: &OstConfig,
        e: &Spanned<TwolcRegex>,
        vvm: &VariableValueMap,
    ) -> crate::error::Result<OtherSymbolTransducer<B>> {
        Ok(match &e.value {
            TwolcRegex::Symbol(s) => {
                let sym = substitute_symbol(s, vvm);
                // A symbol naming a definition expands to its transducer; a
                // symbol naming a set expands to the declared pairs of the
                // 'sym:sym' pair set (the 'Alphabet::compute' semantics);
                // otherwise it is a literal 'sym:sym' pair.
                let site = PairSite::whole(&e.span.range);
                if let Some(def) = self.definitions.get(sym.as_str()) {
                    clone_ost(def)
                } else if self.sets.contains_key(sym.as_str()) {
                    self.pair_transducer(cfg, &sym, &sym, &site)?
                } else if sym.as_str() == TWOLC_HASH {
                    self.boundary_pair_transducer(cfg, TWOLC_HASH, &site)?
                } else {
                    OtherSymbolTransducer::new_symbol(cfg, &sym)?
                }
            }
            TwolcRegex::Pair { upper, lower } => {
                let up = symbol_of(upper, vvm);
                let lo = symbol_of(lower, vvm);
                let site = PairSite {
                    pair: e.span.range.clone(),
                    upper: upper.span.range.clone(),
                    lower: lower.span.range.clone(),
                };
                if up.as_str() == TWOLC_HASH {
                    // C++ dispatches on the '#' input side before any set
                    // handling, so '#:Set' also takes the boundary split.
                    self.boundary_pair_transducer(cfg, lo.as_str(), &site)?
                } else if self.sets.contains_key(up.as_str()) || self.sets.contains_key(lo.as_str())
                {
                    self.pair_transducer(cfg, &up, &lo, &site)?
                } else {
                    OtherSymbolTransducer::new_pair(cfg, &up, &lo)?
                }
            }
            // A standalone '0' regexp (an empty context side, or an explicit
            // '0') denotes the EMPTY STRING, so it must be a real epsilon
            // (HFST_EPSILON) — C++ htwolcpre3's 'RE_LIST: /* empty */' builds
            // 'OtherSymbolTransducer(HFST_EPSILON)'. Using the two-level zero
            // placeholder 'TWOLC_EPSILON' (__HFST_TWOLC_0) instead makes the empty
            // side a literal symbol that only harmonizes when a 0:0 pair happens
            // to be in the alphabet; otherwise 'get_context' yields an unmatchable
            // context and the rule over-restricts (the center is forbidden
            // everywhere). TWOLC_EPSILON stays correct for a PAIR side such as
            // 'h:0' (handled by 'symbol_of'/'new_pair'), which is the realized
            // zero, not an empty string.
            TwolcRegex::Epsilon => OtherSymbolTransducer::new_symbol(cfg, HFST_EPSILON)?,
            TwolcRegex::Any => OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?,
            TwolcRegex::Group(inner) => self.eval_regex_with_vars(cfg, inner, vvm)?,
            TwolcRegex::Optional(inner) => {
                let mut t = self.eval_regex_with_vars(cfg, inner, vvm)?;
                t.apply_zero(cfg, |t| {
                    t.optionalize()?;
                    Ok(())
                })?;
                t
            }
            TwolcRegex::Binary(op, l, r) => self.eval_binary(cfg, *op, l, r, vvm)?,
            TwolcRegex::Unary(op, inner) => self.eval_unary(cfg, *op, inner, vvm)?,
            TwolcRegex::RepeatN(inner, n) => {
                let mut t = self.eval_regex_with_vars(cfg, inner, vvm)?;
                t.apply_num(
                    cfg,
                    |t, n| {
                        t.repeat_n(n)?;
                        Ok(())
                    },
                    *n,
                )?;
                t
            }
            TwolcRegex::RepeatNToK(inner, n, k) => {
                let mut t = self.eval_regex_with_vars(cfg, inner, vvm)?;
                t.apply_two_num(
                    cfg,
                    |t, a, b| {
                        t.repeat_n_to_k(a, b)?;
                        Ok(())
                    },
                    *n,
                    *k,
                )?;
                t
            }
        })
    }

    /// Evaluate a ['TwolcRegex::Unary'] node.
    fn eval_unary(
        &mut self,
        cfg: &OstConfig,
        op: UnaryOp,
        inner: &Spanned<TwolcRegex>,
        vvm: &VariableValueMap,
    ) -> crate::error::Result<OtherSymbolTransducer<B>> {
        let mut t = self.eval_regex_with_vars(cfg, inner, vvm)?;
        match op {
            UnaryOp::Star => {
                t.apply_zero(cfg, |t| {
                    t.repeat_star()?;
                    Ok(())
                })?;
            }
            UnaryOp::Plus => {
                t.apply_zero(cfg, |t| {
                    t.repeat_plus()?;
                    Ok(())
                })?;
            }
            UnaryOp::Reverse => {
                t.apply_zero(cfg, |t| {
                    t.reverse()?;
                    Ok(())
                })?;
            }
            UnaryOp::Invert => {
                t.apply_zero(cfg, |t| {
                    t.invert()?;
                    Ok(())
                })?;
            }
            UnaryOp::UpperProject => {
                t.apply_zero(cfg, |t| {
                    t.input_project()?;
                    Ok(())
                })?;
            }
            UnaryOp::LowerProject => {
                t.apply_zero(cfg, |t| {
                    t.output_project()?;
                    Ok(())
                })?;
            }
            UnaryOp::Complement => {
                t.negated(cfg)?;
            }
            UnaryOp::TermComplement => {
                t.term_complemented(cfg)?;
            }
            UnaryOp::Containment => {
                t.contained(cfg)?;
            }
            UnaryOp::ContainmentOnce => {
                t.contained_once(cfg)?;
            }
            UnaryOp::ContainmentOpt => {
                t.contained(cfg)?;
            }
        }
        Ok(t)
    }

    /// Evaluate a ['TwolcRegex::Binary'] node.
    fn eval_binary(
        &mut self,
        cfg: &OstConfig,
        op: BinaryOp,
        l: &Spanned<TwolcRegex>,
        r: &Spanned<TwolcRegex>,
        vvm: &VariableValueMap,
    ) -> crate::error::Result<OtherSymbolTransducer<B>> {
        let mut left = self.eval_regex_with_vars(cfg, l, vvm)?;
        let right = self.eval_regex_with_vars(cfg, r, vvm)?;
        match op {
            BinaryOp::Concatenate => {
                left.concatenate(cfg, &right)?;
            }
            BinaryOp::Union => {
                left.disjunct(cfg, &right)?;
            }
            BinaryOp::Intersect => {
                left.intersect(cfg, &right)?;
            }
            BinaryOp::Subtract => {
                left.subtract(cfg, &right)?;
            }
            BinaryOp::Compose => {
                left.apply_one_bool(
                    cfg,
                    |t, o, h| {
                        t.compose(o, h)?;
                        Ok(())
                    },
                    &right,
                )?;
            }
            other @ BinaryOp::LenientCompose
            | other @ BinaryOp::CrossProduct
            | other @ BinaryOp::MergeRight
            | other @ BinaryOp::MergeLeft
            | other @ BinaryOp::Before
            | other @ BinaryOp::After
            | other @ BinaryOp::Shuffle
            | other @ BinaryOp::UpperSubtract
            | other @ BinaryOp::LowerSubtract
            | other @ BinaryOp::UpperPriorityUnion
            | other @ BinaryOp::LowerPriorityUnion
            | other @ BinaryOp::Ignoring
            | other @ BinaryOp::IgnoreInternally
            | other @ BinaryOp::LeftQuotient => {
                std::panic::panic_any(format!(
                    "twolc regex: unsupported binary operator {other:?}"
                ));
            }
        }
        Ok(left)
    }
}

/// Resolve a 'TwolcRegex' operand expected to be a single symbol (the upper or
/// lower side of a 'Pair'), applying variable substitution.
fn symbol_of(e: &Spanned<TwolcRegex>, vvm: &VariableValueMap) -> Symbol {
    match &e.value {
        TwolcRegex::Symbol(s) => substitute_symbol(s, vvm),
        TwolcRegex::Epsilon => Symbol::new_static(TWOLC_EPSILON),
        TwolcRegex::Any => Symbol::new_static(TWOLC_UNKNOWN),
        TwolcRegex::Group(inner) => symbol_of(inner, vvm),
        TwolcRegex::Pair { .. }
        | TwolcRegex::Optional(_)
        | TwolcRegex::Binary(..)
        | TwolcRegex::Unary(..)
        | TwolcRegex::RepeatN(..)
        | TwolcRegex::RepeatNToK(..) => {
            std::panic::panic_any("twolc pair side must be a single symbol".to_string())
        }
    }
}
