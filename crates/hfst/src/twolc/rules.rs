//! The twolc rule family: the 'Rule' base, the arrow rules and the
//! conflict-resolving rules.

use super::*;

// ===========================================================================
// rule_src/Rule.{h,cc} — Rule base data + non-virtual methods.
// ===========================================================================

impl<B: AlgebraBackend> Rule<B> {
    /// 'Rule::Rule(name, center, contexts)' ('Rule.cc'). Disjuncts all
    /// 'contexts' into 'context', then harmonizes the center's diacritics
    /// against the disjuncted context.
    // [spec:hfst:def:rule.rule.rule-fn]
    // [spec:hfst:sem:rule.rule.rule-fn]
    pub fn new(
        cfg: &OstConfig,
        name: &str,
        center: OtherSymbolTransducer<B>,
        contexts: &OtherSymbolTransducerVector<B>,
    ) -> crate::error::Result<Rule<B>> {
        let mut rule = Rule {
            is_empty: false,
            name: unescape_name(name),
            center,
            context: OtherSymbolTransducer::new(cfg)?,
            rule_transducer: OtherSymbolTransducer::new(cfg)?,
        };
        // OtherSymbolTransducerVector contexts_copy = contexts;
        // for (it : contexts_copy) context.apply(disjunct, *it);
        for ctx in contexts.iter() {
            rule.context.disjunct(cfg, ctx)?;
        }
        // this->center.harmonize_diacritics(cfg, context);
        let mut context = std::mem::replace(&mut rule.context, OtherSymbolTransducer::new(cfg)?);
        rule.center.harmonize_diacritics(cfg, &mut context);
        rule.context = context;
        Ok(rule)
    }

    /// 'Rule::Rule(name, RuleVector)' ('Rule.cc') — the intersecting result
    /// constructor. Builds 'rule_transducer = ?*' then intersects the
    /// 'rule_transducer' of each non-empty subcase rule. Produces a
    /// ['ResultRule'] whose 'compile()' is the base no-op.
    // [spec:hfst:def:rule.rule.rule-fn]
    // [spec:hfst:sem:rule.rule.rule-fn]
    pub fn new_from_vector(
        cfg: &OstConfig,
        name: &str,
        v: &[&TwolcRule<B>],
    ) -> crate::error::Result<ResultRule<B>> {
        let mut rule_transducer = OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?;
        rule_transducer.repeat_star(cfg)?;
        let mut is_empty = true;
        for r in v.iter() {
            if !r.rule().empty() {
                rule_transducer.intersect(cfg, r.rule_transducer())?;
                is_empty = false;
            }
        }
        Ok(ResultRule {
            base: Rule {
                is_empty,
                name: unescape_name(name),
                center: OtherSymbolTransducer::new(cfg)?,
                context: OtherSymbolTransducer::new(cfg)?,
                rule_transducer,
            },
        })
    }

    /// 'Rule::empty()' ('Rule.cc'). True when conflict resolution merged this
    /// rule into another (or the intersecting ctor found no non-empty subcase).
    // [spec:hfst:def:rule.rule.empty-fn]
    // [spec:hfst:sem:rule.rule.empty-fn]
    pub fn empty(&self) -> bool {
        self.is_empty
    }

    /// 'Rule::store(out)' ('Rule.cc'). Names the rule, maps the internal TWOLC
    /// symbols back to their HFST-facing forms, and writes the rule transducer to
    /// the binary 'HfstOutputStream'.
    // [spec:hfst:def:rule.rule.store-fn]
    // [spec:hfst:sem:rule.rule.store-fn]
    pub fn store(
        &mut self,
        cfg: &OstConfig,
        out: &mut crate::hfst_output_stream::HfstOutputStream,
    ) -> crate::error::Result<()> {
        if self.is_empty {
            return Ok(());
        }
        self.add_name()?;
        self.rule_transducer.remove_diacritics_from_output(cfg)?;
        self.rule_transducer
            .apply_subst(cfg, TWOLC_EPSILON, HFST_EPSILON, true, true)?;
        self.rule_transducer
            .apply_subst(cfg, "__HFST_TWOLC_.#.", "@#@", true, true)?;
        self.rule_transducer
            .apply_subst(cfg, "__HFST_TWOLC_SPACE", " ", true, true)?;
        self.rule_transducer.apply_subst_pair(
            cfg,
            &(Symbol::new_static("@#@"), Symbol::new_static("@#@")),
            &(Symbol::new_static("@#@"), Symbol::new_static(HFST_EPSILON)),
        )?;
        self.rule_transducer
            .apply_subst(cfg, TWOLC_IDENTITY, HFST_IDENTITY, true, true)?;
        out.redirect(&mut self.rule_transducer.transducer)?;
        Ok(())
    }

    /// 'Rule::get_name()' ('Rule.cc').
    // [spec:hfst:def:rule.rule.get-name-fn]
    // [spec:hfst:sem:rule.rule.get-name-fn]
    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    /// 'Rule::add_name()' ('Rule.cc'). Adds the rule name as an info symbol on
    /// 'rule_transducer'.
    // [spec:hfst:def:rule.rule.add-name-fn]
    // [spec:hfst:sem:rule.rule.add-name-fn]
    pub fn add_name(&mut self) -> crate::error::Result<()> {
        let name = self.name.clone();
        self.rule_transducer.add_info_symbol(&name)?;
        Ok(())
    }

    /// 'Rule::get_print_name(s)' ('Rule.cc'). Strips the '__HFST_TWOLC_*'
    /// markers for human-readable display: '__HFST_TWOLC_SPACE' and
    /// '__HFST_TWOLC_RULE_NAME=' become a space, '__HFST_TWOLC_SET_NAME='
    /// and the bare '__HFST_TWOLC_' prefix become empty.
    // [spec:hfst:def:rule.rule.get-print-name-fn]
    // [spec:hfst:sem:rule.rule.get-print-name-fn]
    pub fn get_print_name(s: &str) -> String {
        let mut ss = s.to_string();
        ss = replace_substr(&ss, "__HFST_TWOLC_SPACE", " ");
        ss = replace_substr(&ss, "__HFST_TWOLC_RULE_NAME=", " ");
        ss = replace_substr(&ss, "__HFST_TWOLC_SET_NAME=", "");
        ss = replace_substr(&ss, "__HFST_TWOLC_", "");
        ss
    }

    /// 'Rule::get_universal_language_with_diamonds(cfg, )' ('Rule.cc'). Returns
    /// '?* <D> ?* <D> ?*'.
    // [spec:hfst:def:rule.rule.get-universal-language-with-diamonds-fn]
    // [spec:hfst:sem:rule.rule.get-universal-language-with-diamonds-fn]
    pub fn get_universal_language_with_diamonds(
        cfg: &OstConfig,
    ) -> crate::error::Result<OtherSymbolTransducer<B>> {
        let mut universal = OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?;
        universal.repeat_star(cfg)?;
        let diamond = OtherSymbolTransducer::new_symbol(cfg, TWOLC_DIAMOND)?;
        let mut universal_with_diamonds = universal.clone();
        universal_with_diamonds.concatenate(cfg, &diamond)?;
        universal_with_diamonds.concatenate(cfg, &universal)?;
        universal_with_diamonds.concatenate(cfg, &diamond)?;
        universal_with_diamonds.concatenate(cfg, &universal)?;
        Ok(universal_with_diamonds)
    }

    /// 'Rule::get_center(input, output)' ('Rule.cc'). Returns
    /// '?* <D> input:output <D> ?*'.
    // [spec:hfst:def:rule.rule.get-center-fn]
    // [spec:hfst:sem:rule.rule.get-center-fn]
    pub fn get_center_io(
        cfg: &OstConfig,
        input: &str,
        output: &str,
    ) -> crate::error::Result<OtherSymbolTransducer<B>> {
        let mut unknown = OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?;
        unknown.repeat_star(cfg)?;
        let diamond = OtherSymbolTransducer::new_symbol(cfg, TWOLC_DIAMOND)?;
        let mut center = unknown.clone();
        let center_pair = OtherSymbolTransducer::new_pair(cfg, input, output)?;
        center.concatenate(cfg, &diamond)?;
        center.concatenate(cfg, &center_pair)?;
        center.concatenate(cfg, &diamond)?;
        center.concatenate(cfg, &unknown)?;
        Ok(center)
    }

    /// Run the rule constructor 'build' on the single-pair center
    /// 'get_center(center.first, center.second)': the shared shape of every
    /// symbol-pair-center rule constructor.
    fn with_pair_center<T>(
        cfg: &OstConfig,
        name: &str,
        center: &SymbolPair,
        contexts: &OtherSymbolTransducerVector<B>,
        build: impl FnOnce(
            &OstConfig,
            &str,
            OtherSymbolTransducer<B>,
            &OtherSymbolTransducerVector<B>,
        ) -> crate::error::Result<T>,
    ) -> crate::error::Result<T> {
        build(
            cfg,
            name,
            Self::get_center_io(cfg, &center.0, &center.1)?,
            contexts,
        )
    }

    /// 'Rule::get_center(v: SymbolPairVector)' ('Rule.cc'). Returns
    /// '?* <D> (disjunction of pairs) <D> ?*'.
    // [spec:hfst:def:rule.rule.get-center-fn]
    // [spec:hfst:sem:rule.rule.get-center-fn]
    pub fn get_center_pairs(
        cfg: &OstConfig,
        v: &SymbolPairVector,
    ) -> crate::error::Result<OtherSymbolTransducer<B>> {
        let mut unknown = OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?;
        unknown.repeat_star(cfg)?;
        let diamond = OtherSymbolTransducer::new_symbol(cfg, TWOLC_DIAMOND)?;
        let mut center_pair_transducer = OtherSymbolTransducer::new(cfg)?;
        for pair in v.iter() {
            let p = OtherSymbolTransducer::new_pair(cfg, &pair.0, &pair.1)?;
            center_pair_transducer.disjunct(cfg, &p)?;
        }
        let mut center = unknown.clone();
        center.concatenate(cfg, &diamond)?;
        center.concatenate(cfg, &center_pair_transducer)?;
        center.concatenate(cfg, &diamond)?;
        center.concatenate(cfg, &unknown)?;
        Ok(center)
    }

    /// 'Rule::get_center(restricted_center)' ('Rule.cc'). Returns
    /// '?* <D> restricted_center <D> ?*'.
    // [spec:hfst:def:rule.rule.get-center-fn]
    // [spec:hfst:sem:rule.rule.get-center-fn]
    pub fn get_center_restricted(
        cfg: &OstConfig,
        restricted_center: &OtherSymbolTransducer<B>,
    ) -> crate::error::Result<OtherSymbolTransducer<B>> {
        let mut unknown = OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?;
        unknown.repeat_star(cfg)?;
        let diamond = OtherSymbolTransducer::new_symbol(cfg, TWOLC_DIAMOND)?;
        let mut center = unknown.clone();
        center.concatenate(cfg, &diamond)?;
        center.concatenate(cfg, restricted_center)?;
        center.concatenate(cfg, &diamond)?;
        center.concatenate(cfg, &unknown)?;
        Ok(center)
    }

    /// 'Rule::add_missing_symbols_freely(diacritics)' ('Rule.cc'). For every
    /// diacritic that is not already in 'rule_transducer''s alphabet, add it to
    /// the alphabet and insert the diacritic-pair freely.
    // [spec:hfst:def:rule.rule.add-missing-symbols-freely-fn]
    // [spec:hfst:sem:rule.rule.add-missing-symbols-freely-fn]
    pub fn add_missing_symbols_freely(
        &mut self,
        cfg: &OstConfig,
        diacritics: &SymbolRange,
    ) -> crate::error::Result<()> {
        let symbol_set: BTreeSet<Symbol> = self.rule_transducer.get_transducer()?.get_alphabet()?;
        for d in diacritics.iter() {
            if !symbol_set.contains(d) {
                self.rule_transducer.add_symbol_to_alphabet(cfg, d)?;
                self.rule_transducer.apply_symbol_pair(
                    cfg,
                    |t, p| {
                        t.insert_freely_pair(p, false)?;
                        Ok(())
                    },
                    &(d.clone(), d.clone()),
                )?;
            }
        }
        Ok(())
    }
}

// ===========================================================================
// rule_src/RightArrowRule.{h,cc} — '=>' rule.
// ===========================================================================

impl<B: AlgebraBackend> RightArrowRule<B> {
    /// 'RightArrowRule::RightArrowRule(name, center, contexts)'
    /// ('RightArrowRule.cc'). Delegates to the 'Rule' base constructor.
    // [spec:hfst:def:right-arrow-rule.right-arrow-rule.right-arrow-rule-fn]
    // [spec:hfst:sem:right-arrow-rule.right-arrow-rule.right-arrow-rule-fn]
    pub fn new(
        cfg: &OstConfig,
        name: &str,
        center: OtherSymbolTransducer<B>,
        contexts: &OtherSymbolTransducerVector<B>,
    ) -> crate::error::Result<RightArrowRule<B>> {
        Ok(RightArrowRule {
            base: Rule::new(cfg, name, center, contexts)?,
        })
    }
}

impl<B: AlgebraBackend> RightArrowRule<B> {
    /// 'RightArrowRule::compile()' ('RightArrowRule.cc').
    ///
    /// '''text
    /// center.subtract(cfg, context).substitute(<D> -> 0);
    /// rule_transducer = ?* ;
    /// rule_transducer.subtract(cfg, center);
    /// '''
    ///
    /// MUTATES 'center' in place (subtract the context, then turn the diamonds
    /// into epsilon) before building 'rule_transducer = ?* - center'.
    // [spec:hfst:def:right-arrow-rule.right-arrow-rule.compile-fn]
    // [spec:hfst:sem:right-arrow-rule.right-arrow-rule.compile-fn]
    pub fn compile(&mut self, cfg: &OstConfig) -> crate::error::Result<OtherSymbolTransducer<B>> {
        let context = std::mem::replace(&mut self.base.context, OtherSymbolTransducer::new(cfg)?);
        self.base.center.subtract(cfg, &context)?;
        self.base.center.substitute_diamond_to_epsilon(cfg)?;
        self.base.context = context;

        let mut rule_transducer = OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?;
        let center = std::mem::replace(&mut self.base.center, OtherSymbolTransducer::new(cfg)?);
        rule_transducer.repeat_star(cfg)?;
        rule_transducer.subtract(cfg, &center)?;
        self.base.center = center;

        self.base.rule_transducer = rule_transducer.clone();
        Ok(rule_transducer)
    }
}

// ===========================================================================
// rule_src/LeftArrowRule.{h,cc} — '<=' rule.
// ===========================================================================

impl<B: AlgebraBackend> LeftArrowRule<B> {
    /// 'LeftArrowRule::LeftArrowRule(name, center, contexts)'
    /// ('LeftArrowRule.cc'). Delegates to the 'Rule' base constructor.
    // [spec:hfst:def:left-arrow-rule.left-arrow-rule.left-arrow-rule-fn]
    // [spec:hfst:sem:left-arrow-rule.left-arrow-rule.left-arrow-rule-fn]
    pub fn new(
        cfg: &OstConfig,
        name: &str,
        center: OtherSymbolTransducer<B>,
        contexts: &OtherSymbolTransducerVector<B>,
    ) -> crate::error::Result<LeftArrowRule<B>> {
        Ok(LeftArrowRule {
            base: Rule::new(cfg, name, center, contexts)?,
        })
    }
}

impl<B: AlgebraBackend> LeftArrowRule<B> {
    /// 'LeftArrowRule::compile()' ('LeftArrowRule.cc').
    ///
    /// '''text
    /// abstract_center = center.get_inverse_of_upper_projection(cfg);
    /// context.intersect(cfg, abstract_center);
    /// context.subtract(cfg, center);
    /// context.substitute(<D> -> 0);
    /// rule_transducer = ?* ;
    /// return rule_transducer.subtract(cfg, context);
    /// '''
    // [spec:hfst:def:left-arrow-rule.left-arrow-rule.compile-fn]
    // [spec:hfst:sem:left-arrow-rule.left-arrow-rule.compile-fn]
    pub fn compile(&mut self, cfg: &OstConfig) -> crate::error::Result<OtherSymbolTransducer<B>> {
        let abstract_center = self.base.center.get_inverse_of_upper_projection(cfg)?;
        // context.intersect(cfg, abstract_center).subtract(cfg, center).substitute(<D>->0)
        let center = std::mem::replace(&mut self.base.center, OtherSymbolTransducer::new(cfg)?);
        self.base.context.intersect(cfg, &abstract_center)?;
        self.base.context.subtract(cfg, &center)?;
        self.base.context.substitute_diamond_to_epsilon(cfg)?;
        self.base.center = center;

        let mut rule_transducer = OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?;
        let context = std::mem::replace(&mut self.base.context, OtherSymbolTransducer::new(cfg)?);
        rule_transducer.repeat_star(cfg)?;
        rule_transducer.subtract(cfg, &context)?;
        self.base.context = context;

        self.base.rule_transducer = rule_transducer.clone();
        Ok(rule_transducer)
    }
}

// ===========================================================================
// rule_src/LeftRestrictionArrowRule.{h,cc} — '/<=' rule.
// ===========================================================================

impl<B: AlgebraBackend> LeftRestrictionArrowRule<B> {
    /// 'LeftRestrictionArrowRule::LeftRestrictionArrowRule(name, center,
    /// contexts)' ('LeftRestrictionArrowRule.cc') — 'OtherSymbolTransducer'
    /// center form. Delegates to the 'Rule' base constructor.
    // [spec:hfst:def:left-restriction-arrow-rule.left-restriction-arrow-rule.left-restriction-arrow-rule-fn]
    // [spec:hfst:sem:left-restriction-arrow-rule.left-restriction-arrow-rule.left-restriction-arrow-rule-fn]
    pub fn new(
        cfg: &OstConfig,
        name: &str,
        center: OtherSymbolTransducer<B>,
        contexts: &OtherSymbolTransducerVector<B>,
    ) -> crate::error::Result<LeftRestrictionArrowRule<B>> {
        Ok(LeftRestrictionArrowRule {
            base: Rule::new(cfg, name, center, contexts)?,
        })
    }

    /// 'LeftRestrictionArrowRule::LeftRestrictionArrowRule(name, SymbolPair
    /// center, contexts)' ('LeftRestrictionArrowRule.cc') — symbol-pair center
    /// form. Builds the center via 'Rule::get_center(first, second)'.
    // [spec:hfst:def:left-restriction-arrow-rule.left-restriction-arrow-rule.left-restriction-arrow-rule-fn]
    // [spec:hfst:sem:left-restriction-arrow-rule.left-restriction-arrow-rule.left-restriction-arrow-rule-fn]
    pub fn new_pair(
        cfg: &OstConfig,
        name: &str,
        center: &SymbolPair,
        contexts: &OtherSymbolTransducerVector<B>,
    ) -> crate::error::Result<LeftRestrictionArrowRule<B>> {
        Ok(LeftRestrictionArrowRule {
            base: Rule::with_pair_center(cfg, name, center, contexts, Rule::new)?,
        })
    }
}

impl<B: AlgebraBackend> LeftRestrictionArrowRule<B> {
    /// 'LeftRestrictionArrowRule::compile()'
    /// ('LeftRestrictionArrowRule.cc').
    ///
    /// '''text
    /// center.intersect(cfg, context).substitute(<D> -> 0);
    /// rule_transducer = ?* ;
    /// rule_transducer.subtract(cfg, center);
    /// '''
    // [spec:hfst:def:left-restriction-arrow-rule.left-restriction-arrow-rule.compile-fn]
    // [spec:hfst:sem:left-restriction-arrow-rule.left-restriction-arrow-rule.compile-fn]
    pub fn compile(&mut self, cfg: &OstConfig) -> crate::error::Result<OtherSymbolTransducer<B>> {
        let context = std::mem::replace(&mut self.base.context, OtherSymbolTransducer::new(cfg)?);
        self.base.center.intersect(cfg, &context)?;
        self.base.center.substitute_diamond_to_epsilon(cfg)?;
        self.base.context = context;

        let mut rule_transducer = OtherSymbolTransducer::new_symbol(cfg, TWOLC_UNKNOWN)?;
        let center = std::mem::replace(&mut self.base.center, OtherSymbolTransducer::new(cfg)?);
        rule_transducer.repeat_star(cfg)?;
        rule_transducer.subtract(cfg, &center)?;
        self.base.center = center;

        self.base.rule_transducer = rule_transducer.clone();
        Ok(rule_transducer)
    }
}

// ===========================================================================
// rule_src/ConflictResolvingRightArrowRule.{h,cc} — '=>' single-pair center.
// ===========================================================================

impl<B: AlgebraBackend> ConflictResolvingRightArrowRule<B> {
    /// 'ConflictResolvingRightArrowRule::ConflictResolvingRightArrowRule(name,
    /// center, contexts)' ('ConflictResolvingRightArrowRule.cc'). Builds the
    /// 'RightArrowRule' base from 'get_center(first, second)' and records the
    /// 'center_pair'.
    // [spec:hfst:def:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule-fn]
    // [spec:hfst:sem:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule-fn]
    pub fn new(
        cfg: &OstConfig,
        name: &str,
        center: &SymbolPair,
        contexts: &OtherSymbolTransducerVector<B>,
    ) -> crate::error::Result<ConflictResolvingRightArrowRule<B>> {
        Ok(ConflictResolvingRightArrowRule {
            base: Rule::with_pair_center(cfg, name, center, contexts, RightArrowRule::new)?,
            center_pair: center.clone(),
        })
    }

    /// 'ConflictResolvingRightArrowRule::conflicts_this(another)'
    /// ('ConflictResolvingRightArrowRule.cc'). Two '=>'-rules conflict when
    /// they share the same center symbol-pair.
    // [spec:hfst:def:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule.conflicts-this-fn]
    // [spec:hfst:sem:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule.conflicts-this-fn]
    pub fn conflicts_this(&self, another: &ConflictResolvingRightArrowRule<B>) -> bool {
        self.center_pair.0 == another.center_pair.0 && self.center_pair.1 == another.center_pair.1
    }

    /// 'ConflictResolvingRightArrowRule::resolve_conflict(another)'
    /// ('ConflictResolvingRightArrowRule.cc'). Merges 'another''s context into
    /// 'this' (disjunct + minimize) and appends its name.
    // [spec:hfst:def:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule.resolve-conflict-fn]
    // [spec:hfst:sem:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule.resolve-conflict-fn]
    pub fn resolve_conflict(
        &mut self,
        cfg: &OstConfig,
        another: &ConflictResolvingRightArrowRule<B>,
    ) -> crate::error::Result<()> {
        let another_context = another.base.base.context.clone();
        self.base.base.context.disjunct(cfg, &another_context)?;
        self.base.base.context.minimize(cfg)?;
        let another_name = another.base.base.name.clone();
        self.base.base.name += " and ";
        self.base.base.name += &another_name;
        Ok(())
    }
}

impl<B: AlgebraBackend> ConflictResolvingRightArrowRule<B> {
    pub fn compile(&mut self, cfg: &OstConfig) -> crate::error::Result<OtherSymbolTransducer<B>> {
        self.base.compile(cfg)
    }
}

// ===========================================================================
// rule_src/ConflictResolvingLeftArrowRule.{h,cc} — '<=' single-pair center.
// ===========================================================================

/// 'get_wb_fst(cfg)' ('ConflictResolvingLeftArrowRule.cc'). Builds the
/// word-boundary framing transducer '.#. ((? - .#.) | <D>)* .#.'.
// [spec:hfst:def:conflict-resolving-left-arrow-rule.get-wb-fst-fn]
// [spec:hfst:sem:conflict-resolving-left-arrow-rule.get-wb-fst-fn]
pub fn get_wb_fst<B: AlgebraBackend>(
    cfg: &OstConfig,
) -> crate::error::Result<OtherSymbolTransducer<B>> {
    let wb = OtherSymbolTransducer::new_pair(cfg, "__HFST_TWOLC_.#.", "__HFST_TWOLC_.#.")?;
    let mut no_wb = OtherSymbolTransducer::new_pair(cfg, TWOLC_UNKNOWN, TWOLC_UNKNOWN)?;
    let diamond = OtherSymbolTransducer::new_pair(cfg, TWOLC_DIAMOND, TWOLC_DIAMOND)?;

    no_wb.subtract(cfg, &wb)?;
    no_wb.disjunct(cfg, &diamond)?;
    no_wb.repeat_star(cfg)?;

    let mut result = wb.clone();
    result.concatenate(cfg, &no_wb)?;
    result.concatenate(cfg, &wb)?;

    Ok(result)
}

/// 'wbize(cfg, t)' ('ConflictResolvingLeftArrowRule.cc'). Intersects 't' with the
/// word-boundary framing transducer.
// [spec:hfst:def:conflict-resolving-left-arrow-rule.wbize-fn]
// [spec:hfst:sem:conflict-resolving-left-arrow-rule.wbize-fn]
pub fn wbize<B: AlgebraBackend>(
    cfg: &OstConfig,
    t: &OtherSymbolTransducer<B>,
) -> crate::error::Result<OtherSymbolTransducer<B>> {
    let mut t_copy = t.clone();
    let wb_fst = get_wb_fst(cfg)?;
    t_copy.intersect(cfg, &wb_fst)?;
    Ok(t_copy)
}

impl<B: AlgebraBackend> ConflictResolvingLeftArrowRule<B> {
    /// 'ConflictResolvingLeftArrowRule::ConflictResolvingLeftArrowRule(name,
    /// center, contexts)' ('ConflictResolvingLeftArrowRule.cc'). Builds the
    /// 'LeftArrowRule' base from 'get_center(first, second)' and records the
    /// center's input symbol.
    // [spec:hfst:def:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule-fn]
    // [spec:hfst:sem:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule-fn]
    pub fn new(
        cfg: &OstConfig,
        name: &str,
        center: &SymbolPair,
        contexts: &OtherSymbolTransducerVector<B>,
    ) -> crate::error::Result<ConflictResolvingLeftArrowRule<B>> {
        Ok(ConflictResolvingLeftArrowRule {
            base: Rule::with_pair_center(cfg, name, center, contexts, LeftArrowRule::new)?,
            input_symbol: center.0.clone(),
        })
    }

    /// 'ConflictResolvingLeftArrowRule::conflicts_this(another, v)'
    /// ('ConflictResolvingLeftArrowRule.cc'). True when 'this''s context has a
    /// non-empty intersection with the word-boundary-framed context of
    /// 'another' (the conflicting string is stored in 'v').
    // [spec:hfst:def:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.conflicts-this-fn]
    // [spec:hfst:sem:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.conflicts-this-fn]
    pub fn conflicts_this(
        &self,
        cfg: &OstConfig,
        another: &ConflictResolvingLeftArrowRule<B>,
        v: &mut StringVector,
    ) -> crate::error::Result<bool> {
        Ok(!self
            .base
            .base
            .context
            .is_empty_intersection(&wbize(cfg, &another.base.base.context)?, v))
    }

    /// 'ConflictResolvingLeftArrowRule::resolvable_conflict(another)'
    /// ('ConflictResolvingLeftArrowRule.cc'). True when 'this''s context is a
    /// sub-language of the word-boundary-framed context of 'another'.
    // [spec:hfst:def:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.resolvable-conflict-fn]
    // [spec:hfst:sem:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.resolvable-conflict-fn]
    pub fn resolvable_conflict(
        &self,
        cfg: &OstConfig,
        another: &ConflictResolvingLeftArrowRule<B>,
    ) -> crate::error::Result<bool> {
        self.base
            .base
            .context
            .is_subset(cfg, &wbize(cfg, &another.base.base.context)?)
    }

    /// 'ConflictResolvingLeftArrowRule::resolve_conflict(another)'
    /// ('ConflictResolvingLeftArrowRule.cc'). Resolves by subtracting
    /// 'another''s context from 'this''s context.
    // [spec:hfst:def:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.resolve-conflict-fn]
    // [spec:hfst:sem:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.resolve-conflict-fn]
    pub fn resolve_conflict(
        &mut self,
        cfg: &OstConfig,
        another: &ConflictResolvingLeftArrowRule<B>,
    ) -> crate::error::Result<()> {
        let another_context = another.base.base.context.clone();
        self.base.base.context.subtract(cfg, &another_context)?;
        Ok(())
    }
}

impl<B: AlgebraBackend> ConflictResolvingLeftArrowRule<B> {
    pub fn compile(&mut self, cfg: &OstConfig) -> crate::error::Result<OtherSymbolTransducer<B>> {
        self.base.compile(cfg)
    }
}

// ===== integration shims: string_manipulation.cc helpers used by the rule names =====
// [spec:hfst:def:string-manipulation.replace-substr-fn]
// [spec:hfst:sem:string-manipulation.replace-substr-fn]
pub fn replace_substr(s: &str, substr: &str, replacement: &str) -> String {
    if substr.is_empty() {
        return s.to_string();
    }
    s.replace(substr, replacement)
}

// [spec:hfst:def:string-manipulation.unescape-name-fn]
// [spec:hfst:sem:string-manipulation.unescape-name-fn]
pub fn unescape_name(name: &str) -> String {
    replace_substr(
        &replace_substr(name, "__HFST_TWOLC_RULE_NAME=", ""),
        "__HFST_TWOLC_SPACE",
        " ",
    )
}
