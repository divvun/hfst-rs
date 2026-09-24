//! 'evaluate' for the replace-rule, restriction, and mapping-pair containers.

use super::*;

// [spec:hfst:def:pmatch-utils.hfst.pmatch-parallel-rules-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-parallel-rules-container.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-parallel-rules-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-parallel-rules-container.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchParallelRulesContainer<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        let key = self.cache_key();
        if ctx.node_cache_get(key).is_some() {
            self.report_cache(ctx, String::new());
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just checked"));
        }
        let my_timer = self.start_timing(ctx);
        let mut retval: HfstTransducer<B> = match self.arrow {
            ReplaceArrow::E_REPLACE_RIGHT => replace_rule_vector(&self.make_mappings(ctx)?, false)?,
            ReplaceArrow::E_OPTIONAL_REPLACE_RIGHT => {
                replace_rule_vector(&self.make_mappings(ctx)?, true)?
            }
            ReplaceArrow::E_REPLACE_LEFT => {
                replace_left_rule_vector(&self.make_mappings(ctx)?, false)?
            }
            ReplaceArrow::E_OPTIONAL_REPLACE_LEFT => {
                replace_left_rule_vector(&self.make_mappings(ctx)?, true)?
            }
            ReplaceArrow::E_RTL_LONGEST_MATCH => {
                replace_rightmost_longest_match_rule_vector(&self.make_mappings(ctx)?)?
            }
            ReplaceArrow::E_RTL_SHORTEST_MATCH => {
                replace_rightmost_shortest_match_rule_vector(&self.make_mappings(ctx)?)?
            }
            ReplaceArrow::E_LTR_LONGEST_MATCH => {
                replace_leftmost_longest_match_rule_vector(&self.make_mappings(ctx)?)?
            }
            ReplaceArrow::E_LTR_SHORTEST_MATCH => {
                replace_leftmost_shortest_match_rule_vector(&self.make_mappings(ctx)?)?
            }
            ReplaceArrow::E_REPLACE_RIGHT_MARKUP => {
                ctx.pmatcherror("Unrecognized arrow type");
                return Ok(HfstTransducer::new());
            }
        };
        retval.set_final_weights(self.weight as f32, true)?;
        self.report_time(ctx, my_timer, String::new());
        if ctx.node_cache_get(key).is_none() && self.should_use_cache(ctx) {
            ctx.node_cache_put(key, retval);
            ctx.node_cache_get_mut(key)
                .expect("cache populated above")
                .minimize()?;
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just inserted"));
        }
        Ok(retval)
    }
}
impl<B: AlgebraBackend + 'static> PmatchParallelRulesContainer<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-parallel-rules-container.make-mappings-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-parallel-rules-container.make-mappings-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-parallel-rules-container.make-mappings-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-parallel-rules-container.make-mappings-fn]
    pub fn make_mappings(
        &self,
        ctx: &mut PmatchEvalContext<B>,
    ) -> crate::error::Result<Vec<Rule<B>>> {
        self.rules.iter().map(|it| it.make_mapping(ctx)).collect()
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-replace-rule-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-replace-rule-container.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-replace-rule-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-replace-rule-container.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchReplaceRuleContainer<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        let key = self.cache_key();
        if ctx.node_cache_get(key).is_some() {
            self.report_cache(ctx, String::new());
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just checked"));
        }
        let my_timer = self.start_timing(ctx);
        let mut retval: HfstTransducer<B> = match self.arrow {
            ReplaceArrow::E_REPLACE_RIGHT => replace_rule(&self.make_mapping(ctx)?, false)?,
            ReplaceArrow::E_OPTIONAL_REPLACE_RIGHT => replace_rule(&self.make_mapping(ctx)?, true)?,
            ReplaceArrow::E_REPLACE_LEFT => replace_left_rule(&self.make_mapping(ctx)?, false)?,
            ReplaceArrow::E_OPTIONAL_REPLACE_LEFT => {
                replace_left_rule(&self.make_mapping(ctx)?, true)?
            }
            ReplaceArrow::E_RTL_LONGEST_MATCH => {
                replace_rightmost_longest_match_rule(&self.make_mapping(ctx)?)?
            }
            ReplaceArrow::E_RTL_SHORTEST_MATCH => {
                replace_rightmost_shortest_match_rule(&self.make_mapping(ctx)?)?
            }
            ReplaceArrow::E_LTR_LONGEST_MATCH => {
                replace_leftmost_longest_match_rule(&self.make_mapping(ctx)?)?
            }
            ReplaceArrow::E_LTR_SHORTEST_MATCH => {
                replace_leftmost_shortest_match_rule(&self.make_mapping(ctx)?)?
            }
            ReplaceArrow::E_REPLACE_RIGHT_MARKUP => {
                ctx.pmatcherror("Unrecognized arrow");
                return Ok(HfstTransducer::new());
            }
        };
        retval.set_final_weights(self.weight as f32, true)?;
        self.report_time(ctx, my_timer, String::new());
        if ctx.node_cache_get(key).is_none() && self.should_use_cache(ctx) {
            ctx.node_cache_put(key, retval);
            ctx.node_cache_get_mut(key)
                .expect("cache populated above")
                .minimize()?;
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just inserted"));
        }
        Ok(retval)
    }
}
impl<B: AlgebraBackend + 'static> PmatchReplaceRuleContainer<B> {
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-replace-rule-container.make-mapping-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-replace-rule-container.make-mapping-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-replace-rule-container.make-mapping-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-replace-rule-container.make-mapping-fn]
    pub fn make_mapping(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<Rule<B>> {
        let mut pair_vector: HfstTransducerPairVector<B> = Vec::new();
        for it in self.mapping.iter() {
            let pp: TransducerPointerPair<B> = it.evaluate_pair(ctx)?;
            let p: HfstTransducerPair<B> = (
                HfstTransducer::new_copy(&pp.0)?,
                HfstTransducer::new_copy(&pp.1)?,
            );
            pair_vector.push(p);
        }
        if self.context.is_empty() {
            return Rule::new_mapping(&pair_vector);
        }
        let mut context_vector: HfstTransducerPairVector<B> = Vec::new();
        for it in self.context.iter() {
            let pp: TransducerPointerPair<B> = it.evaluate_pair(ctx)?;
            let p: HfstTransducerPair<B> = (
                HfstTransducer::new_copy(&pp.0)?,
                HfstTransducer::new_copy(&pp.1)?,
            );
            context_vector.push(p);
        }
        Rule::new_mapping_context_repl_type(&pair_vector, &context_vector, self.ty)
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-restriction-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-restriction-container.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-restriction-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-restriction-container.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchRestrictionContainer<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        let key = self.cache_key();
        if ctx.node_cache_get(key).is_some() {
            self.report_cache(ctx, String::new());
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just checked"));
        }
        let my_timer = self.start_timing(ctx);
        let mut pair_vector: HfstTransducerPairVector<B> = Vec::new();
        for it in self.contexts.iter() {
            let pp: TransducerPointerPair<B> = it.evaluate_pair(ctx)?;
            let p: HfstTransducerPair<B> = (
                HfstTransducer::new_copy(&pp.0)?,
                HfstTransducer::new_copy(&pp.1)?,
            );
            pair_vector.push(p);
        }
        let l: HfstTransducer<B> = self.left.evaluate(ctx)?;
        let mut retval: HfstTransducer<B> = restriction(&l, &pair_vector)?;
        retval.set_final_weights(self.weight as f32, true)?;
        self.report_time(ctx, my_timer, String::new());
        if ctx.node_cache_get(key).is_none() && self.should_use_cache(ctx) {
            ctx.node_cache_put(key, retval);
            ctx.node_cache_get_mut(key)
                .expect("cache populated above")
                .minimize()?;
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just inserted"));
        }
        Ok(retval)
    }
}
impl<B: AlgebraBackend + 'static> PmatchObjectPairBase<B> for PmatchObjectPair<B> {
    fn get_left(&self) -> ObjRef<B> {
        self.left.clone()
    }
    fn get_right(&self) -> ObjRef<B> {
        self.right.clone()
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-object-pair.evaluate-pair-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-object-pair.evaluate-pair-fn]
    fn evaluate_pair(
        &self,
        ctx: &mut PmatchEvalContext<B>,
    ) -> crate::error::Result<TransducerPointerPair<B>> {
        let first = self.left.evaluate(ctx)?;
        let second = self.right.evaluate(ctx)?;
        Ok((first, second))
    }
}
impl<B: AlgebraBackend + 'static> PmatchObjectPairBase<B> for PmatchMarkupContainer<B> {
    fn get_left(&self) -> ObjRef<B> {
        self.left.clone()
    }
    fn get_right(&self) -> ObjRef<B> {
        self.right.clone()
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-markup-container.evaluate-pair-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-markup-container.evaluate-pair-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-markup-container.evaluate-pair-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-markup-container.evaluate-pair-fn]
    fn evaluate_pair(
        &self,
        ctx: &mut PmatchEvalContext<B>,
    ) -> crate::error::Result<TransducerPointerPair<B>> {
        let loa = self.left_of_arrow.evaluate(ctx)?;
        let lom = self.left.evaluate(ctx)?;
        let rom = self.right.evaluate(ctx)?;
        let tmp_mapping_pair: HfstTransducerPair<B> =
            (HfstTransducer::new_copy(&loa)?, HfstTransducer::new());
        let marks: HfstTransducerPair<B> = (
            HfstTransducer::new_copy(&lom)?,
            HfstTransducer::new_copy(&rom)?,
        );
        let mapping_pair: HfstTransducerPair<B> =
            create_mapping_for_mark_up_replace(&tmp_mapping_pair, &marks)?;
        Ok((
            HfstTransducer::new_copy(&mapping_pair.0)?,
            HfstTransducer::new_copy(&mapping_pair.1)?,
        ))
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-mapping-pairs-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-mapping-pairs-container.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-mapping-pairs-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-mapping-pairs-container.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchMappingPairsContainer<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        ctx.pmatcherror("Should never happen\n");
        unreachable!()
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-contexts-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-contexts-container.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-contexts-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-contexts-container.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchContextsContainer<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        ctx.pmatcherror("Should never happen\n");
        unreachable!()
    }
}
