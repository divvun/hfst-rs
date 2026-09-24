//! 'evaluate' for the unary, numeric, binary, and ternary operation nodes.

use super::*;

// [spec:hfst:def:pmatch-utils.hfst.pmatch-unary-operation.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-unary-operation.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-unary-operation.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchUnaryOperation<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        let key = self.cache_key();
        if ctx.node_cache_get(key).is_some() {
            self.report_cache(ctx, String::new());
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just checked"));
        }
        let my_timer = self.start_timing(ctx);

        // Special optimization cases
        if self.op == PmatchUnaryOp::Implode {
            let mut strings: StringVector = StringVector::new();
            self.root.collect_strings_into(ctx, &mut strings);
            let mut whole_string = String::new();
            for it in strings.iter() {
                whole_string += it;
            }
            let mut retval = if !whole_string.is_empty() {
                HfstTransducer::new_symbol(&whole_string)?
            } else {
                HfstTransducer::new()
            };
            retval.set_final_weights(self.weight as f32, true)?;
            if ctx.node_cache_get(key).is_none() && self.should_use_cache(ctx) {
                ctx.node_cache_put(key, retval);
                self.report_time(
                    ctx,
                    my_timer,
                    " with ".to_string()
                        + &get_size_info(ctx.node_cache_get(key).expect("just inserted")),
                );
                return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just inserted"));
            }
            self.report_time(ctx, my_timer, String::new());
            return Ok(retval);
        } else if self.op == PmatchUnaryOp::Explode {
            let mut strings: StringVector = StringVector::new();
            self.root.collect_strings_into(ctx, &mut strings);
            let mut whole_string = String::new();
            for it in strings.iter() {
                whole_string += it;
            }
            let tok = crate::hfst_tokenizer::HfstTokenizer::new();
            let mut retval = if !whole_string.is_empty() {
                HfstTransducer::new_tokenized(&whole_string, &tok)?
            } else {
                HfstTransducer::new()
            };
            retval.set_final_weights(self.weight as f32, true)?;
            if ctx.node_cache_get(key).is_none() && self.should_use_cache(ctx) {
                ctx.node_cache_put(key, retval);
                return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just inserted"));
            }
            self.report_time(ctx, my_timer, String::new());
            return Ok(retval);
        }

        if !self.name.is_empty() {
            ctx.eval_stack_push(self.name.clone());
        }
        let mut retval: HfstTransducer<B> = self.root.evaluate(ctx)?;
        retval = match self.op {
            PmatchUnaryOp::Cap
            | PmatchUnaryOp::OptCap
            | PmatchUnaryOp::ToLower
            | PmatchUnaryOp::ToUpper
            | PmatchUnaryOp::OptToLower
            | PmatchUnaryOp::OptToUpper
            | PmatchUnaryOp::AnyCase
            | PmatchUnaryOp::CapUpper
            | PmatchUnaryOp::OptCapUpper
            | PmatchUnaryOp::ToLowerUpper
            | PmatchUnaryOp::ToUpperUpper
            | PmatchUnaryOp::OptToLowerUpper
            | PmatchUnaryOp::OptToUpperUpper
            | PmatchUnaryOp::AnyCaseUpper
            | PmatchUnaryOp::CapLower
            | PmatchUnaryOp::OptCapLower
            | PmatchUnaryOp::ToLowerLower
            | PmatchUnaryOp::ToUpperLower
            | PmatchUnaryOp::OptToLowerLower
            | PmatchUnaryOp::OptToUpperLower
            | PmatchUnaryOp::AnyCaseLower => self.apply_case_op(ctx, retval)?,
            PmatchUnaryOp::LC | PmatchUnaryOp::NLC | PmatchUnaryOp::RC | PmatchUnaryOp::NRC => {
                self.apply_context_op(ctx, retval)?
            }
            PmatchUnaryOp::AddDelimiters
            | PmatchUnaryOp::Optionalize
            | PmatchUnaryOp::RepeatStar
            | PmatchUnaryOp::RepeatPlus
            | PmatchUnaryOp::Reverse
            | PmatchUnaryOp::Invert
            | PmatchUnaryOp::InputProject
            | PmatchUnaryOp::OutputProject
            | PmatchUnaryOp::Complement
            | PmatchUnaryOp::Containment
            | PmatchUnaryOp::ContainmentOnce
            | PmatchUnaryOp::ContainmentOptional
            | PmatchUnaryOp::TermComplement
            | PmatchUnaryOp::MakeSigma
            | PmatchUnaryOp::MakeList
            | PmatchUnaryOp::MakeExcList
            | PmatchUnaryOp::Explode
            | PmatchUnaryOp::Implode => self.apply_structural_op(ctx, retval)?,
        };
        retval.set_final_weights(self.weight as f32, true)?;

        if !self.name.is_empty() {
            ctx.eval_stack_pop();
        }
        if ctx.node_cache_get(key).is_none() && self.should_use_cache(ctx) {
            ctx.node_cache_put(key, retval);
            ctx.node_cache_get_mut(key)
                .expect("cache populated above")
                .minimize()?;
            self.report_time(
                ctx,
                my_timer,
                " with ".to_string()
                    + &get_size_info(ctx.node_cache_get(key).expect("just inserted")),
            );
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just inserted"));
        }
        self.report_time(ctx, my_timer, String::new());
        Ok(retval)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-unary-operation.get-initial-symbols-from-unary-root-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-unary-operation.get-initial-symbols-from-unary-root-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation.get-initial-symbols-from-unary-root-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-unary-operation.get-initial-symbols-from-unary-root-fn]
    fn get_initial_symbols_from_unary_root(
        &self,
        ctx: &mut PmatchEvalContext<B>,
    ) -> crate::error::Result<StringSet> {
        PmatchObject_get_real_initial_symbols(ctx, &*self.root)
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-unary-operation.is-context-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-unary-operation.is-context-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation.is-context-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-unary-operation.is-context-fn]
    fn is_context(&self) -> bool {
        self.op == PmatchUnaryOp::LC
            || self.op == PmatchUnaryOp::NLC
            || self.op == PmatchUnaryOp::RC
            || self.op == PmatchUnaryOp::NRC
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-unary-operation.is-delimiter-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-unary-operation.is-delimiter-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation.is-delimiter-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-unary-operation.is-delimiter-fn]
    fn is_delimiter(&self) -> bool {
        self.op == PmatchUnaryOp::AddDelimiters
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-unary-operation.get-initial-rc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-unary-operation.get-initial-rc-initial-symbols-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation.get-initial-rc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-unary-operation.get-initial-rc-initial-symbols-fn]
    fn get_initial_RC_initial_symbols(
        &self,
        ctx: &mut PmatchEvalContext<B>,
    ) -> crate::error::Result<StringSet> {
        if self.op == PmatchUnaryOp::RC {
            let tmp: HfstTransducer<B> = self.root.evaluate(ctx)?;
            return Ok(tmp.get_initial_input_symbols());
        }
        if self.op == PmatchUnaryOp::AddDelimiters {
            return self.root.get_initial_RC_initial_symbols(ctx);
        }
        Ok(StringSet::new())
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-unary-operation.get-initial-nrc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-unary-operation.get-initial-nrc-initial-symbols-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-unary-operation.get-initial-nrc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-unary-operation.get-initial-nrc-initial-symbols-fn]
    fn get_initial_NRC_initial_symbols(
        &self,
        ctx: &mut PmatchEvalContext<B>,
    ) -> crate::error::Result<StringSet> {
        if self.op == PmatchUnaryOp::NRC {
            let tmp: HfstTransducer<B> = self.root.evaluate(ctx)?;
            return Ok(tmp.get_initial_input_symbols());
        }
        if self.op == PmatchUnaryOp::AddDelimiters {
            return self.root.get_initial_NRC_initial_symbols(ctx);
        }
        Ok(StringSet::new())
    }
}

impl<B: AlgebraBackend + 'static> PmatchUnaryOperation<B> {
    // Delimiters, repetition, projection, complement, containment, and the
    // sigma and list operators.
    fn apply_structural_op(
        &self,
        ctx: &mut PmatchEvalContext<B>,
        mut retval: HfstTransducer<B>,
    ) -> crate::error::Result<HfstTransducer<B>> {
        if self.op == PmatchUnaryOp::AddDelimiters {
            retval = add_pmatch_delimiters(&retval)?;
        } else if self.op == PmatchUnaryOp::Optionalize {
            retval.optionalize()?;
        } else if self.op == PmatchUnaryOp::RepeatStar {
            retval.repeat_star()?;
        } else if self.op == PmatchUnaryOp::RepeatPlus {
            retval.repeat_plus()?;
        } else if self.op == PmatchUnaryOp::Reverse {
            retval.reverse()?;
        } else if self.op == PmatchUnaryOp::Invert {
            retval.invert()?;
        } else if self.op == PmatchUnaryOp::InputProject {
            retval.input_project()?;
        } else if self.op == PmatchUnaryOp::OutputProject {
            retval.output_project()?;
        } else if self.op == PmatchUnaryOp::Complement {
            // Defined here only for automata, so can project to input
            let mut complement =
                HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
            complement.repeat_star()?;
            complement.subtract(&retval, true)?;
            retval = complement;
        } else if self.op == PmatchUnaryOp::Containment {
            let mut any = HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
            any.repeat_star()?;
            let mut left = HfstTransducer::new_copy(&any)?;
            left.concatenate(&retval, true)?;
            left.concatenate(&any, true)?;
            retval = left;
        } else if self.op == PmatchUnaryOp::ContainmentOnce {
            let mut xre_comp = crate::xre::XreCompiler::new();
            retval = xre_comp.contains_once(&retval)?;
        } else if self.op == PmatchUnaryOp::ContainmentOptional {
            let mut xre_comp = crate::xre::XreCompiler::new();
            retval = xre_comp.contains_once_optional(&retval)?;
        } else if self.op == PmatchUnaryOp::TermComplement {
            let mut any = HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
            let alphabet: StringSet = get_non_special_alphabet(&retval)?;
            for it in alphabet.iter() {
                let symbol = HfstTransducer::new_symbol(it)?;
                any.subtract(&symbol, true)?;
            }
            retval = any;
        } else if self.op == PmatchUnaryOp::MakeSigma {
            retval = make_sigma(ctx, &retval)?;
        } else if self.op == PmatchUnaryOp::MakeList {
            let tmp = make_list(&retval)?;
            register_lst_line_numbers_from_transducer(ctx, &tmp, self.line_defined)?;
            retval = tmp;
        } else if self.op == PmatchUnaryOp::MakeExcList {
            retval = make_exc_list(&retval)?;
        }
        Ok(retval)
    }

    // The Cap/ToLower/ToUpper/AnyCase family, on either side.
    fn apply_case_op(
        &self,
        ctx: &mut PmatchEvalContext<B>,
        mut retval: HfstTransducer<B>,
    ) -> crate::error::Result<HfstTransducer<B>> {
        if self.op == PmatchUnaryOp::Cap {
            retval = ctx.with_utils(|u| u.cap(&retval, Side::Both, false))?;
        } else if self.op == PmatchUnaryOp::OptCap {
            retval = ctx.with_utils(|u| u.cap(&retval, Side::Both, true))?;
        } else if self.op == PmatchUnaryOp::ToLower {
            retval = ctx.with_utils(|u| u.tolower(&retval, Side::Both, false))?;
        } else if self.op == PmatchUnaryOp::ToUpper {
            retval = ctx.with_utils(|u| u.toupper(&retval, Side::Both, false))?;
        } else if self.op == PmatchUnaryOp::OptToLower {
            let mut tmp = ctx.with_utils(|u| u.tolower(&retval, Side::Both, true))?;
            tmp.disjunct(&retval, true)?;
            retval = tmp;
        } else if self.op == PmatchUnaryOp::OptToUpper {
            retval = ctx.with_utils(|u| u.toupper(&retval, Side::Both, true))?;
        } else if self.op == PmatchUnaryOp::AnyCase {
            let (toupper, tolower) = ctx.with_utils(|u| {
                Ok((
                    u.toupper(&retval, Side::Both, true)?,
                    u.tolower(&retval, Side::Both, true)?,
                ))
            })?;
            retval.disjunct(&toupper, true)?;
            retval.disjunct(&tolower, true)?;
        } else if self.op == PmatchUnaryOp::CapUpper {
            retval = ctx.with_utils(|u| u.cap(&retval, Side::Upper, false))?;
        } else if self.op == PmatchUnaryOp::OptCapUpper {
            retval = ctx.with_utils(|u| u.cap(&retval, Side::Upper, true))?;
        } else if self.op == PmatchUnaryOp::ToLowerUpper {
            retval = ctx.with_utils(|u| u.tolower(&retval, Side::Upper, false))?;
        } else if self.op == PmatchUnaryOp::ToUpperUpper {
            retval = ctx.with_utils(|u| u.toupper(&retval, Side::Upper, false))?;
        } else if self.op == PmatchUnaryOp::OptToLowerUpper {
            let mut tmp = ctx.with_utils(|u| u.tolower(&retval, Side::Upper, true))?;
            tmp.disjunct(&retval, true)?;
            retval = tmp;
        } else if self.op == PmatchUnaryOp::OptToUpperUpper {
            retval = ctx.with_utils(|u| u.toupper(&retval, Side::Upper, true))?;
        } else if self.op == PmatchUnaryOp::AnyCaseUpper {
            let (toupper, tolower) = ctx.with_utils(|u| {
                Ok((
                    u.toupper(&retval, Side::Upper, true)?,
                    u.tolower(&retval, Side::Upper, true)?,
                ))
            })?;
            retval.disjunct(&toupper, true)?;
            retval.disjunct(&tolower, true)?;
        } else if self.op == PmatchUnaryOp::CapLower {
            retval = ctx.with_utils(|u| u.cap(&retval, Side::Lower, false))?;
        } else if self.op == PmatchUnaryOp::OptCapLower {
            retval = ctx.with_utils(|u| u.cap(&retval, Side::Lower, true))?;
        } else if self.op == PmatchUnaryOp::ToLowerLower {
            retval = ctx.with_utils(|u| u.tolower(&retval, Side::Lower, false))?;
        } else if self.op == PmatchUnaryOp::ToUpperLower {
            retval = ctx.with_utils(|u| u.toupper(&retval, Side::Lower, false))?;
        } else if self.op == PmatchUnaryOp::OptToLowerLower {
            let mut tmp = ctx.with_utils(|u| u.tolower(&retval, Side::Lower, true))?;
            tmp.disjunct(&retval, true)?;
            retval = tmp;
        } else if self.op == PmatchUnaryOp::OptToUpperLower {
            retval = ctx.with_utils(|u| u.toupper(&retval, Side::Lower, true))?;
        } else if self.op == PmatchUnaryOp::AnyCaseLower {
            let (toupper, tolower) = ctx.with_utils(|u| {
                Ok((
                    u.toupper(&retval, Side::Lower, true)?,
                    u.tolower(&retval, Side::Lower, true)?,
                ))
            })?;
            retval.disjunct(&toupper, true)?;
            retval.disjunct(&tolower, true)?;
        }
        Ok(retval)
    }

    // The LC/NLC/RC/NRC context conditions.
    fn apply_context_op(
        &self,
        ctx: &mut PmatchEvalContext<B>,
        mut retval: HfstTransducer<B>,
    ) -> crate::error::Result<HfstTransducer<B>> {
        if self.op == PmatchUnaryOp::LC {
            if !transducer_has_context_symbol(&retval)? {
                retval.reverse()?;
                let mut tmp = HfstTransducer::new_symbol_pair(
                    crate::hfst_symbol_defs::internal_epsilon,
                    LC_ENTRY_SYMBOL,
                )?;
                tmp.concatenate(&retval, true)?;
                let lc_exit = HfstTransducer::new_symbol_pair(
                    crate::hfst_symbol_defs::internal_epsilon,
                    LC_EXIT_SYMBOL,
                )?;
                tmp.concatenate(&lc_exit, true)?;
                retval = tmp;
            } else if ctx.verbose {
                write_compilation_stack_indentation_to_err(ctx);
                debug!(
                    "** Warning: ignoring nested context condition when compiling {}",
                    ctx.eval_stack_last()
                        .expect("eval_stack has an entry during evaluation")
                );
            }
        } else if self.op == PmatchUnaryOp::NLC {
            if !transducer_has_context_symbol(&retval)? {
                retval.reverse()?;
                let tmp = ctx.make_minimization_guard()?;
                let mut head = tmp.evaluate(ctx)?;
                let passthrough = HfstTransducer::new_symbol(PASSTHROUGH_SYMBOL)?;
                let mut nlc_entry = HfstTransducer::new_symbol_pair(
                    crate::hfst_symbol_defs::internal_epsilon,
                    NLC_ENTRY_SYMBOL,
                )?;
                let nlc_exit = HfstTransducer::new_symbol_pair(
                    crate::hfst_symbol_defs::internal_epsilon,
                    NLC_EXIT_SYMBOL,
                )?;
                nlc_entry.concatenate(&retval, true)?;
                nlc_entry.concatenate(&nlc_exit, true)?;
                nlc_entry.disjunct(&passthrough, true)?;
                head.concatenate(&nlc_entry, true)?;
                retval = head;
            } else if ctx.verbose {
                write_compilation_stack_indentation_to_err(ctx);
                debug!(
                    "** Warning: ignoring nested context condition when compiling {}",
                    ctx.eval_stack_last()
                        .expect("eval_stack has an entry during evaluation")
                );
            }
        } else if self.op == PmatchUnaryOp::RC {
            if !transducer_has_context_symbol(&retval)? {
                let mut tmp = HfstTransducer::new_symbol_pair(
                    crate::hfst_symbol_defs::internal_epsilon,
                    RC_ENTRY_SYMBOL,
                )?;
                tmp.concatenate(&retval, true)?;
                let rc_exit = HfstTransducer::new_symbol_pair(
                    crate::hfst_symbol_defs::internal_epsilon,
                    RC_EXIT_SYMBOL,
                )?;
                tmp.concatenate(&rc_exit, true)?;
                retval = tmp;
            } else if ctx.verbose {
                write_compilation_stack_indentation_to_err(ctx);
                debug!(
                    "** Warning: ignoring nested context condition when compiling {}",
                    ctx.eval_stack_last()
                        .expect("eval_stack has an entry during evaluation")
                );
            }
        } else if self.op == PmatchUnaryOp::NRC {
            if !transducer_has_context_symbol(&retval)? {
                let tmp = ctx.make_minimization_guard()?;
                let mut head = tmp.evaluate(ctx)?;
                let passthrough = HfstTransducer::new_symbol(PASSTHROUGH_SYMBOL)?;
                let mut nrc_entry = HfstTransducer::new_symbol_pair(
                    crate::hfst_symbol_defs::internal_epsilon,
                    NRC_ENTRY_SYMBOL,
                )?;
                let nrc_exit = HfstTransducer::new_symbol_pair(
                    crate::hfst_symbol_defs::internal_epsilon,
                    NRC_EXIT_SYMBOL,
                )?;
                nrc_entry.concatenate(&retval, true)?;
                nrc_entry.concatenate(&nrc_exit, true)?;
                nrc_entry.disjunct(&passthrough, true)?;
                head.concatenate(&nrc_entry, true)?;
                retval = head;
            } else if ctx.verbose {
                write_compilation_stack_indentation_to_err(ctx);
                debug!(
                    "** Warning: ignoring nested context condition when compiling {}",
                    ctx.eval_stack_last()
                        .expect("eval_stack has an entry during evaluation")
                );
            }
        }
        Ok(retval)
    }
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch-numeric-operation.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-numeric-operation.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-numeric-operation.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-numeric-operation.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchNumericOperation<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        let key = self.cache_key();
        if ctx.node_cache_get(key).is_some() {
            self.report_cache(ctx, String::new());
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just checked"));
        }
        let my_timer = self.start_timing(ctx);
        if !self.name.is_empty() {
            ctx.eval_stack_push(self.name.clone());
        }
        let mut tmp: HfstTransducer<B> = self.root.evaluate(ctx)?;
        if self.op == PmatchNumericOp::RepeatN {
            tmp.repeat_n(self.values[0] as u32)?;
        } else if self.op == PmatchNumericOp::RepeatNPlus {
            tmp.repeat_n_plus(self.values[0] as u32)?;
        } else if self.op == PmatchNumericOp::RepeatNMinus {
            tmp.repeat_n_minus(self.values[0] as u32)?;
        } else if self.op == PmatchNumericOp::RepeatNToK {
            tmp.repeat_n_to_k(self.values[0] as u32, self.values[1] as u32)?;
        }
        tmp.set_final_weights(self.weight as f32, true)?;
        if !self.name.is_empty() {
            ctx.eval_stack_pop();
        }
        if ctx.node_cache_get(key).is_none() && self.should_use_cache(ctx) {
            ctx.node_cache_put(key, tmp);
            ctx.node_cache_get_mut(key)
                .expect("cache populated above")
                .minimize()?;
            self.report_time(ctx, my_timer, String::new());
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just inserted"));
        }
        self.report_time(ctx, my_timer, String::new());
        Ok(tmp)
    }
}

// [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchBinaryOperation<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        let key = self.cache_key();
        if ctx.node_cache_get(key).is_some() {
            self.report_cache(ctx, String::new());
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just checked"));
        }
        let my_timer = self.start_timing(ctx);

        // Special optimization cases
        if self.op == PmatchBinaryOp::Disjunct
            && self.left.is_unweighted_disjunction_of_strings()
            && self.right.is_unweighted_disjunction_of_strings()
        {
            let mut strings: StringVector = StringVector::new();
            self.left.collect_strings_into(ctx, &mut strings);
            self.right.collect_strings_into(ctx, &mut strings);
            let tok = crate::hfst_tokenizer::HfstTokenizer::new();
            let mut retval = HfstTransducer::new();
            for it in strings.iter() {
                let spv = tok.tokenize(it, false); // XXX
                retval.disjunct_spv(&spv)?;
            }
            retval.set_final_weights(self.weight as f32, true)?;
            if ctx.node_cache_get(key).is_none() && self.should_use_cache(ctx) {
                ctx.node_cache_put(key, retval);
                // No minimization because we did it the clever way!
                self.report_time(
                    ctx,
                    my_timer,
                    format!(
                        " with {}",
                        get_size_info(ctx.node_cache_get(key).expect("just inserted"))
                    ),
                );
                return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just inserted"));
            }
            self.report_time(ctx, my_timer, String::new());
            return Ok(retval);
        }

        if !self.name.is_empty() {
            ctx.eval_stack_push(self.name.clone());
        }
        // General cases
        let mut lhs: HfstTransducer<B> = self.left.evaluate(ctx)?;
        let mut rhs: HfstTransducer<B> = self.right.evaluate(ctx)?;
        match self.op {
            PmatchBinaryOp::Concatenate => {
                lhs.concatenate(&rhs, true)?;
            }
            PmatchBinaryOp::Compose => {
                lhs.compose(&rhs, true)?;
            }
            PmatchBinaryOp::CrossProduct => {
                lhs.cross_product(&rhs, true)?;
            }
            PmatchBinaryOp::LenientCompose => {
                lhs.lenient_composition(&rhs, true)?;
            }
            PmatchBinaryOp::Disjunct => {
                let lhs_syms = lhs.get_alphabet()?;
                let rhs_syms = rhs.get_alphabet()?;
                let lst_lines = ctx.lst_line_map_snapshot();
                fix_list_overlap(ctx, &mut lhs, &mut rhs, &lhs_syms, &rhs_syms, &lst_lines)?;
                fix_list_overlap(ctx, &mut rhs, &mut lhs, &rhs_syms, &lhs_syms, &lst_lines)?;
                lhs.disjunct(&rhs, true)?;
            }
            PmatchBinaryOp::Intersect => {
                lhs.intersect(&rhs, true)?;
            }
            PmatchBinaryOp::Subtract => {
                if ctx.verbose {
                    warn_on_nonsubtractable_symbols(ctx, &lhs)?;
                    warn_on_nonsubtractable_symbols(ctx, &rhs)?;
                }
                lhs.subtract(&rhs, true)?;
            }
            PmatchBinaryOp::UpperSubtract => {
                ctx.pmatcherror("Upper subtraction not implemented.");
                return Ok(lhs);
            }
            PmatchBinaryOp::LowerSubtract => {
                ctx.pmatcherror("Lower subtraction not implemented.");
                return Ok(lhs);
            }
            PmatchBinaryOp::UpperPriorityUnion => {
                lhs.priority_union(&rhs)?;
            }
            PmatchBinaryOp::LowerPriorityUnion => {
                lhs.invert()?;
                rhs.invert()?;
                lhs.priority_union(&rhs)?;
                lhs.invert()?;
            }
            PmatchBinaryOp::Shuffle => {
                let __res = lhs.shuffle(&rhs, true).map(|_| ());
                if let Err(e) = __res {
                    if matches!(e.kind, crate::error::ErrorKind::TransducersAreNotAutomata) {
                        pmatchwarning(
                            "tried to shuffle with non-automaton transducers,\n    shuffling with their input projection instead.",
                        );
                        lhs.input_project()?;
                        rhs.input_project()?;
                        lhs.shuffle(&rhs, true)?;
                    } else {
                        return Err(e);
                    }
                }
            }
            PmatchBinaryOp::Before => {
                lhs = crate::hfst_xerox_rules::before(&lhs, &rhs)?;
            }
            PmatchBinaryOp::After => {
                lhs = crate::hfst_xerox_rules::after(&lhs, &rhs)?;
            }
            PmatchBinaryOp::InsertFreely => {
                lhs.insert_freely(&rhs, false)?;
            }
            PmatchBinaryOp::IgnoreInternally => {
                let right_part: HfstTransducer<B> = HfstTransducer::new_copy(&lhs)?;
                let mut middle_part: HfstTransducer<B> = HfstTransducer::new_copy(&lhs)?;
                middle_part.disjunct(&rhs, true)?;
                middle_part.repeat_star()?;
                lhs.concatenate(&middle_part, true)?;
                lhs.concatenate(&right_part, true)?;
            }
            PmatchBinaryOp::Merge => {
                // hfst::xre::merge_first_to_second(lhs, rhs)
                let args = crate::xre::XreConstructorArguments::new(
                    BTreeMap::new(),
                    BTreeMap::new(),
                    BTreeMap::new(),
                    BTreeMap::new(),
                );
                lhs.optimize()?;
                let __res = rhs.merge(&lhs, &args).map(|_| ());
                match __res {
                    Ok(()) => {
                        // NB: mirrors the C++ aliasing (lhs now becomes rhs).
                        lhs = std::mem::take(&mut rhs);
                    }
                    Err(e) => {
                        if matches!(e.kind, crate::error::ErrorKind::TransducersAreNotAutomata) {
                            ctx.pmatcherror(
                                "Error: transducers must be automata in merge operation.",
                            );
                            unreachable!("pmatcherror panics");
                        } else {
                            return Err(e);
                        }
                    }
                }
            }
        }
        drop(rhs);
        lhs.set_final_weights(self.weight as f32, true)?;
        if !self.name.is_empty() {
            ctx.eval_stack_pop();
        }
        if ctx.node_cache_get(key).is_none() && self.should_use_cache(ctx) {
            ctx.node_cache_put(key, lhs);
            ctx.node_cache_get_mut(key)
                .expect("cache populated above")
                .minimize()?;
            self.report_time(
                ctx,
                my_timer,
                format!(
                    " with {}",
                    get_size_info(ctx.node_cache_get(key).expect("just inserted"))
                ),
            );
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just inserted"));
        }
        self.report_time(ctx, my_timer, String::new());
        Ok(lhs)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.get-real-initial-symbols-from-right-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.get-real-initial-symbols-from-right-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.get-real-initial-symbols-from-right-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.get-real-initial-symbols-from-right-fn]
    fn get_real_initial_symbols_from_right(
        &self,
        ctx: &mut PmatchEvalContext<B>,
    ) -> crate::error::Result<StringSet> {
        PmatchObject_get_real_initial_symbols(ctx, &*self.right)
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.is-left-concatenation-with-context-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.is-left-concatenation-with-context-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.is-left-concatenation-with-context-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.is-left-concatenation-with-context-fn]
    fn is_left_concatenation_with_context(&self) -> bool {
        self.op == PmatchBinaryOp::Concatenate && self.left.is_context()
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.get-initial-rc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.get-initial-rc-initial-symbols-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.get-initial-rc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.get-initial-rc-initial-symbols-fn]
    fn get_initial_RC_initial_symbols(
        &self,
        ctx: &mut PmatchEvalContext<B>,
    ) -> crate::error::Result<StringSet> {
        let mut retval: StringSet = StringSet::new();
        if self.op == PmatchBinaryOp::Concatenate {
            let left_ss: StringSet = self.left.get_initial_RC_initial_symbols(ctx)?;
            let mut right_ss: StringSet = StringSet::new();
            if self.right.is_context() || self.right.is_delimiter() {
                right_ss = self.right.get_initial_NRC_initial_symbols(ctx)?;
            }
            for it in left_ss.iter() {
                retval.insert(it.clone());
            }
            for it in right_ss.iter() {
                retval.insert(it.clone());
            }
            return Ok(retval);
        }
        Ok(retval)
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.get-initial-nrc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.get-initial-nrc-initial-symbols-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.get-initial-nrc-initial-symbols-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.get-initial-nrc-initial-symbols-fn]
    fn get_initial_NRC_initial_symbols(
        &self,
        ctx: &mut PmatchEvalContext<B>,
    ) -> crate::error::Result<StringSet> {
        let mut retval: StringSet = StringSet::new();
        if self.op == PmatchBinaryOp::Concatenate {
            let left_ss: StringSet = self.left.get_initial_NRC_initial_symbols(ctx)?;
            let mut right_ss: StringSet = StringSet::new();
            if self.right.is_context() || self.right.is_delimiter() {
                right_ss = self.right.get_initial_NRC_initial_symbols(ctx)?;
            }
            for it in left_ss.iter() {
                retval.insert(it.clone());
            }
            for it in right_ss.iter() {
                retval.insert(it.clone());
            }
            return Ok(retval);
        }
        Ok(retval)
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.collect-strings-into-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.collect-strings-into-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.collect-strings-into-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.collect-strings-into-fn]
    fn collect_strings_into(&self, ctx: &mut PmatchEvalContext<B>, strings: &mut StringVector) {
        self.left.collect_strings_into(ctx, strings);
        self.right.collect_strings_into(ctx, strings);
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.as-string-pair-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.as-string-pair-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.as-string-pair-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.as-string-pair-fn]
    fn as_string_pair(&self, ctx: &mut PmatchEvalContext<B>) -> StringPair {
        if self.op == PmatchBinaryOp::CrossProduct {
            let left_string: String = self.left.as_string(ctx).unwrap_or_default();
            let right_string: String = self.right.as_string(ctx).unwrap_or_default();
            return (Symbol::from(left_string), Symbol::from(right_string));
        }
        (Symbol::new_static(""), Symbol::new_static(""))
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-binary-operation.is-unweighted-disjunction-of-strings-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-binary-operation.is-unweighted-disjunction-of-strings-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-binary-operation.is-unweighted-disjunction-of-strings-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-binary-operation.is-unweighted-disjunction-of-strings-fn]
    fn is_unweighted_disjunction_of_strings(&self) -> bool {
        self.weight == 0.0
            && self.op == PmatchBinaryOp::Disjunct
            && self.left.is_unweighted_disjunction_of_strings()
            && self.right.is_unweighted_disjunction_of_strings()
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-ternary-operation.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-ternary-operation.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-ternary-operation.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-ternary-operation.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchTernaryOperation<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        let key = self.cache_key();
        if ctx.node_cache_get(key).is_some() {
            self.report_cache(ctx, String::new());
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just checked"));
        }
        let my_timer = self.start_timing(ctx);
        if !self.name.is_empty() {
            ctx.eval_stack_push(self.name.clone());
        }
        let mut retval: HfstTransducer<B> = self.left.evaluate(ctx)?;
        if self.op == PmatchTernaryOp::Substitute {
            let middle_pair: StringPair = self.middle.as_string_pair(ctx);
            let right_pair: StringPair = self.right.as_string_pair(ctx);
            if !right_pair.0.is_empty() || !right_pair.1.is_empty() {
                retval.substitute_pair_with_pair(&middle_pair, &right_pair)?;
            } else {
                let mut tmp: HfstTransducer<B> = self.right.evaluate(ctx)?;
                retval.substitute_pair_with_transducer(&middle_pair, &mut tmp, true)?;
            }
        } else if self.op == PmatchTernaryOp::Uncompose {
            let _unc_left: HfstTransducer<B> = self.middle.evaluate(ctx)?;
            let _unc_right: HfstTransducer<B> = self.right.evaluate(ctx)?;
        }
        retval.set_final_weights(self.weight as f32, true)?;
        if ctx.node_cache_get(key).is_none() && self.should_use_cache(ctx) {
            ctx.node_cache_put(key, retval);
            ctx.node_cache_get_mut(key)
                .expect("cache populated above")
                .minimize()?;
            self.report_time(ctx, my_timer, String::new());
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just inserted"));
        }
        self.report_time(ctx, my_timer, String::new());
        if !self.name.is_empty() {
            ctx.eval_stack_pop();
        }
        Ok(retval)
    }
}
// [spec:hfst:def:pmatch-utils.hfst.transducer-has-context-symbol-fn]
// [spec:hfst:sem:pmatch-utils.hfst.transducer-has-context-symbol-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.transducer-has-context-symbol-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.transducer-has-context-symbol-fn]
pub fn transducer_has_context_symbol<B: AlgebraBackend>(
    t: &HfstTransducer<B>,
) -> crate::error::Result<bool> {
    let ss: StringSet = t.get_alphabet()?;
    Ok(ss.contains(LC_ENTRY_SYMBOL)
        || ss.contains(NLC_ENTRY_SYMBOL)
        || ss.contains(RC_ENTRY_SYMBOL)
        || ss.contains(NRC_ENTRY_SYMBOL))
}
// [spec:hfst:def:pmatch-utils.hfst.warn-on-nonsubtractable-symbols-fn]
// [spec:hfst:sem:pmatch-utils.hfst.warn-on-nonsubtractable-symbols-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.warn-on-nonsubtractable-symbols-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.warn-on-nonsubtractable-symbols-fn]
pub fn warn_on_nonsubtractable_symbols<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    t: &HfstTransducer<B>,
) -> crate::error::Result<()> {
    let alphabet: StringSet = t.get_alphabet()?;
    for it in alphabet.iter() {
        if it.len() < 3 {
            continue;
        } else if it.starts_with("@PMATCH") || it.starts_with("@I") || it.starts_with("@L") {
            write_compilation_stack_indentation_to_err(ctx);
            warn!("subtracting with nonsubtractable symbol {}", it);
        }
    }
    Ok(())
}
