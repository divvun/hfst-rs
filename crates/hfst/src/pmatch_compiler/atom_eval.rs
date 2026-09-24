//! 'evaluate' for symbols, strings, acceptors, containers, and functions.

use super::*;

// [spec:hfst:def:pmatch-utils.hfst.pmatch-object.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.evaluate-fn]
// The C++ overload 'PmatchObject::evaluate(std::vector<PmatchObject*> args)'
// (the base default for the trait 'evaluate_args'); this is the shared
// weight/cache handling. ('pmatchlineno' does not exist in the AST-walk port,
// so the diagnostic uses 'line_defined'.)
pub fn PmatchObject_evaluate_args<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    this: &dyn PmatchObject<B>,
    args: Vec<ObjRef<B>>,
) -> crate::error::Result<HfstTransducer<B>> {
    if args.is_empty() {
        let key = this.cache_key();
        if this.should_use_cache(ctx) {
            if ctx.node_cache_get(key).is_none() {
                let my_timer = this.start_timing(ctx);
                let c = this.evaluate(ctx)?;
                ctx.node_cache_put(key, c);
                this.report_time(ctx, my_timer, String::new());
            }
            HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just inserted"))
        } else {
            let my_timer = this.start_timing(ctx);
            let mut retval = this.evaluate(ctx)?;
            retval.minimize()?;
            this.report_time(ctx, my_timer, String::new());
            Ok(retval)
        }
    } else {
        let errstring = format!(
            "Object {} on line {} has no argument handling",
            this.get_name(),
            this.get_line_defined()
        );
        panic!("{}", errstring);
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-object.expand-ins-arcs-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.expand-ins-arcs-fn]
pub fn PmatchObject_expand_Ins_arcs<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    this: &dyn PmatchObject<B>,
    ss: &mut StringSet,
) -> crate::error::Result<()> {
    {
        let mut did_no_expansions = false;
        let mut expansions_done: StringSet = StringSet::new();
        let mut expanded_symbols: StringSet = StringSet::new();
        let this_name = this.get_name().to_string();
        if !this_name.is_empty() {
            let mut this_name_insed = String::from("@I.");
            this_name_insed.push_str(&this_name);
            this_name_insed.push('@');
            expansions_done.insert(Symbol::from(this_name_insed));
        }
        while !did_no_expansions {
            did_no_expansions = true;
            for it in ss.iter() {
                if it.find("@I.") == Some(0) && it.rfind('@') == Some(it.len() - 1) {
                    // it's an Ins
                    if !expansions_done.contains(it) {
                        let ins_name = it[3..it.len() - 1].to_string();
                        did_no_expansions = false;
                        expansions_done.insert(it.clone());
                        if ctx.definitions_contains(&ins_name) {
                            let mut allowed: StringSet = StringSet::new();
                            let mut disallowed: StringSet = StringSet::new();
                            if ctx.def_insed_expressions_contains(&ins_name) {
                                ctx.def_insed_expressions_get(&ins_name)
                                    .expect("checked with contains just above")
                                    .collect_initial_symbols_into(&mut allowed, &mut disallowed)?;
                            } else {
                                ctx.definitions_get(&ins_name)
                                    .expect("definitions_contains verified above")
                                    .collect_initial_symbols_into(&mut allowed, &mut disallowed)?;
                            }
                            if !allowed.is_empty() {
                                for s in allowed.iter() {
                                    expanded_symbols.insert(s.clone());
                                }
                            } else {
                                expanded_symbols.insert(Symbol::new_static(internal_identity));
                            }
                        }
                    }
                }
            }
        }
        for it in expansions_done.iter() {
            ss.remove(it);
        }
        for s in expanded_symbols.iter() {
            ss.insert(s.clone());
        }
    }
    Ok(())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-object.get-real-initial-symbols-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.get-real-initial-symbols-fn]
pub fn PmatchObject_get_real_initial_symbols<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    this: &dyn PmatchObject<B>,
) -> crate::error::Result<StringSet> {
    if this.is_left_concatenation_with_context() {
        return this.get_real_initial_symbols_from_right(ctx);
    }
    if this.is_delimiter() {
        return this.get_initial_symbols_from_unary_root(ctx);
    }
    let tmp: HfstTransducer<B> = this.evaluate(ctx)?;
    let retval: StringSet = tmp.get_initial_input_symbols();
    Ok(retval)
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-object.collect-initial-symbols-into-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-object.collect-initial-symbols-into-fn]
pub fn PmatchObject_collect_initial_symbols_into<B: AlgebraBackend + 'static>(
    ctx: &mut PmatchEvalContext<B>,
    this: &dyn PmatchObject<B>,
    allowed_initial_symbols: &mut StringSet,
    disallowed_initial_symbols: &mut StringSet,
) -> crate::error::Result<()> {
    {
        // One or neither of allowed_initial_symbols and disallowed_initial_symbols
        // will have some symbols inserted to it.

        let mut allowed: StringSet = this.get_real_initial_symbols()?;
        let mut required: StringSet = this.get_initial_RC_initial_symbols(ctx)?;
        let mut disallowed: StringSet = this.get_initial_NRC_initial_symbols(ctx)?;

        // The first input symbols collected in this way may include:
        // - insertion arcs
        // - unknown, identity, default
        // - symbols after flag diacritics
        // - symbols from contexts
        PmatchObject_expand_Ins_arcs(ctx, this, &mut allowed)?;
        PmatchObject_expand_Ins_arcs(ctx, this, &mut required)?;
        PmatchObject_expand_Ins_arcs(ctx, this, &mut disallowed)?;

        if allowed.is_empty() {
            // Probably something went wrong, we'll just not make no judgement
            return Ok(());
        }

        if string_set_has_meta_arc(&mut allowed) {
            if !required.is_empty() && !string_set_has_meta_arc(&mut required) {
                // RC sets a constraint
                for it in required.iter() {
                    if !disallowed.contains(it) {
                        allowed_initial_symbols.insert(it.clone());
                    }
                }
                return Ok(());
            } else {
                // Anything goes except what is disallowed
                if disallowed.is_empty() || string_set_has_meta_arc(&mut disallowed) {
                    return Ok(());
                } else {
                    for s in disallowed.iter() {
                        disallowed_initial_symbols.insert(s.clone());
                    }
                    return Ok(());
                }
            }
        }

        // Now we can assume that "allowed" is nonempty and non-meta.

        if required.is_empty() || string_set_has_meta_arc(&mut required) {
            // RC poses no constraint
            for it in allowed.iter() {
                if !disallowed.contains(it) {
                    allowed_initial_symbols.insert(it.clone());
                }
            }
            return Ok(());
        }

        // Now we can assume that there is a genuine RC constraint.

        for it in required.iter() {
            if allowed.contains(it) && !disallowed.contains(it) {
                allowed_initial_symbols.insert(it.clone());
            }
        }
    }
    Ok(())
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-symbol.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-symbol.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-symbol.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-symbol.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchSymbol<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        if !self.name.is_empty() {
            ctx.eval_stack_push(self.name.clone());
        }
        let my_timer = self.start_timing(ctx);
        let mut retval: HfstTransducer<B>;
        if symbol_in_local_context(ctx, &self.sym) {
            retval = symbol_from_local_context(ctx, &self.sym)
                .expect("symbol_in_local_context verified above")
                .evaluate(ctx)?;
        } else if ctx.definitions_contains(&self.sym) {
            if ctx.flatten && ctx.def_insed_expressions_contains(&self.sym) {
                retval = ctx
                    .def_insed_expressions_get(&self.sym)
                    .expect("checked with contains just above")
                    .evaluate(ctx)?;
            } else {
                retval = symbol_from_global_context(ctx, &self.sym)
                    .expect("definitions_contains verified above")
                    .evaluate(ctx)?;
            }
            ctx.used_definitions_insert(self.sym.clone());
        } else {
            if ctx.verbose {
                debug!(
                    "Warning: interpreting undefined symbol \"{}\" as label on line {}",
                    self.sym, self.line_defined
                );
            }
            retval = HfstTransducer::new_symbol(&self.sym)?;
        }
        retval.set_final_weights(self.weight as f32, true)?;
        retval.minimize()?;
        self.report_time(ctx, my_timer, String::new());
        if !self.name.is_empty() {
            ctx.eval_stack_pop();
        }
        Ok(retval)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-symbol.evaluate-as-arg-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-symbol.evaluate-as-arg-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-symbol.evaluate-as-arg-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-symbol.evaluate-as-arg-fn]
    fn evaluate_as_arg(&self, ctx: &mut PmatchEvalContext<B>) -> ObjRef<B> {
        if symbol_in_local_context(ctx, &self.sym) {
            return symbol_from_local_context(ctx, &self.sym)
                .expect("symbol_in_local_context verified above")
                .evaluate_as_arg(ctx);
        } else if ctx.definitions_contains(&self.sym) {
            ctx.used_definitions_insert(self.sym.clone());
            if ctx.flatten && ctx.def_insed_expressions_contains(&self.sym) {
                return ctx
                    .def_insed_expressions_get(&self.sym)
                    .expect("checked with contains just above")
                    .evaluate_as_arg(ctx);
            } else {
                return symbol_from_global_context(ctx, &self.sym)
                    .expect("definitions_contains verified above")
                    .evaluate_as_arg(ctx);
            }
        } else {
            if ctx.verbose {
                debug!(
                    "Warning: interpreting undefined symbol \"{}\" as label on line {}",
                    self.sym, self.line_defined
                );
            }
            Rc::new(PmatchString {
                name: String::new(),
                weight: 0.0,
                line_defined: 0,
                string: self.sym.clone(),
                multichar: false,
                _marker: std::marker::PhantomData,
            })
        }
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-symbol.collect-strings-into-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-symbol.collect-strings-into-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-symbol.collect-strings-into-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-symbol.collect-strings-into-fn]
    fn collect_strings_into(&self, ctx: &mut PmatchEvalContext<B>, strings: &mut StringVector) {
        if symbol_in_local_context(ctx, &self.sym) {
            symbol_from_local_context(ctx, &self.sym)
                .expect("symbol_in_local_context verified above")
                .collect_strings_into(ctx, strings);
        } else if ctx.definitions_contains(&self.sym) {
            symbol_from_global_context(ctx, &self.sym)
                .expect("definitions_contains verified above")
                .collect_strings_into(ctx, strings);
            ctx.used_definitions_insert(self.sym.clone());
        } else {
            strings.push(self.sym.clone());
        }
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-symbol.as-string-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-symbol.as-string-fn]
    fn as_string(&self, ctx: &mut PmatchEvalContext<B>) -> Option<String> {
        Some(self.sym.to_string())
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-string.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-string.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-string.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchString<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        let key = self.cache_key();
        if ctx.node_cache_get(key).is_some() {
            self.report_cache(ctx, String::new());
            return HfstTransducer::new_copy(ctx.node_cache_get(key).expect("just checked"));
        }
        let my_timer = self.start_timing(ctx);
        let mut tmp: HfstTransducer<B> = if self.multichar {
            let tok = HfstTokenizer::new();
            HfstTransducer::new_tokenized(&self.string, &tok)?
        } else {
            HfstTransducer::new_symbol(&self.string)?
        };
        tmp.set_final_weights(self.weight as f32, true)?;
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

    // [spec:hfst:def:pmatch-utils.hfst.pmatch-string.collect-strings-into-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-string.collect-strings-into-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string.collect-strings-into-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-string.collect-strings-into-fn]
    fn collect_strings_into(&self, ctx: &mut PmatchEvalContext<B>, strings: &mut StringVector) {
        strings.push(self.string.clone());
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch-string.evaluate-as-arg-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch-string.evaluate-as-arg-fn]
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string.evaluate-as-arg-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-string.evaluate-as-arg-fn]
    fn evaluate_as_arg(&self, ctx: &mut PmatchEvalContext<B>) -> ObjRef<B> {
        Rc::new(PmatchString {
            name: self.name.clone(),
            weight: self.weight,
            line_defined: self.line_defined,
            string: self.string.clone(),
            multichar: self.multichar,
            _marker: std::marker::PhantomData,
        })
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string.as-string-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-string.as-string-fn]
    fn as_string(&self, ctx: &mut PmatchEvalContext<B>) -> Option<String> {
        Some(self.string.to_string())
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string.as-string-pair-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-string.as-string-pair-fn]
    fn as_string_pair(&self, ctx: &mut PmatchEvalContext<B>) -> StringPair {
        (self.string.clone(), self.string.clone())
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-string.is-unweighted-disjunction-of-strings-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-string.is-unweighted-disjunction-of-strings-fn]
    fn is_unweighted_disjunction_of_strings(&self) -> bool {
        self.weight == 0.0 && (self.multichar || (self.string.len() < 2))
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-question-mark.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-question-mark.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-question-mark.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-question-mark.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchQuestionMark<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        let my_timer = self.start_timing(ctx);
        let mut retval: HfstTransducer<B> = HfstTransducer::new_symbol(internal_identity)?;
        retval.set_final_weights(self.weight as f32, true)?;
        self.report_time(ctx, my_timer, String::new());
        Ok(retval)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-question-mark.as-string-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-question-mark.as-string-fn]
    fn as_string(&self, ctx: &mut PmatchEvalContext<B>) -> Option<String> {
        Some(internal_unknown.to_string())
    }
    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-question-mark.as-string-pair-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-question-mark.as-string-pair-fn]
    fn as_string_pair(&self, ctx: &mut PmatchEvalContext<B>) -> StringPair {
        (
            Symbol::new_static(internal_identity),
            Symbol::new_static(internal_identity),
        )
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-acceptor.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-acceptor.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-acceptor.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-acceptor.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchAcceptor<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        let my_timer = self.start_timing(ctx);
        let mut retval: HfstTransducer<B> = match self.set {
            PmatchPredefined::Alpha => {
                if ctx
                    .variables_entry_or_default("unicode-character-classes")
                    .as_str()
                    == "on"
                {
                    HfstTransducer::new_symbol("@UNICODE_ALPHA@")?
                } else {
                    ctx.with_utils(|u| HfstTransducer::new_copy(&u.latin1_alpha_acceptor))?
                }
            }
            PmatchPredefined::UppercaseAlpha => {
                if ctx
                    .variables_entry_or_default("unicode-character-classes")
                    .as_str()
                    == "on"
                {
                    HfstTransducer::new_symbol("@UNICODE_UPPERALPHA@")?
                } else {
                    ctx.with_utils(|u| HfstTransducer::new_copy(&u.latin1_uppercase_acceptor))?
                }
            }
            PmatchPredefined::LowercaseAlpha => {
                if ctx
                    .variables_entry_or_default("unicode-character-classes")
                    .as_str()
                    == "on"
                {
                    HfstTransducer::new_symbol("@UNICODE_LOWERALPHA@")?
                } else {
                    ctx.with_utils(|u| HfstTransducer::new_copy(&u.latin1_lowercase_acceptor))?
                }
            }
            PmatchPredefined::Numeral => {
                ctx.with_utils(|u| HfstTransducer::new_copy(&u.latin1_numeral_acceptor))?
            }
            PmatchPredefined::Punctuation => {
                ctx.with_utils(|u| HfstTransducer::new_copy(&u.latin1_punct_acceptor))?
            }
            PmatchPredefined::Whitespace => {
                if ctx
                    .variables_entry_or_default("unicode-character-classes")
                    .as_str()
                    == "on"
                {
                    HfstTransducer::new_symbol("@UNICODE_WHITESPACE@")?
                } else {
                    ctx.with_utils(|u| HfstTransducer::new_copy(&u.latin1_whitespace_acceptor))?
                }
            }
        };
        retval.set_final_weights(self.weight as f32, true)?;
        self.report_time(ctx, my_timer, String::new());
        Ok(retval)
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-empty.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-empty.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchEmpty<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        Ok(HfstTransducer::new())
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-epsilon-arc.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-epsilon-arc.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchEpsilonArc<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        HfstTransducer::new_symbol(internal_epsilon)
    }

    // [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-epsilon-arc.as-string-fn]
    // [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-epsilon-arc.as-string-fn]
    fn as_string(&self, ctx: &mut PmatchEvalContext<B>) -> Option<String> {
        Some(internal_epsilon.to_string())
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-transducer-container.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-transducer-container.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchTransducerContainer<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        let mut retval = HfstTransducer::new_copy(&self.t)?;
        retval.set_final_weights(self.weight as f32, true)?;
        if !self.name.is_empty() {
            retval.set_name(&self.name);
        }
        Ok(retval)
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-function.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-function.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-function.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-function.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchFunction<B> {
    pmatch_object_base_accessors!();

    fn evaluate_args(
        &self,
        ctx: &mut PmatchEvalContext<B>,
        funargs: Vec<ObjRef<B>>,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let my_timer: clock_t = if ctx.verbose {
            let my_timer = clock();
            ctx.named_object_evaluation_stack_depth += 1;
            write_compilation_stack_indentation_to_err(ctx);
            debug!("Evaluating call to {}...", self.name);
            my_timer
        } else {
            0
        };
        if funargs.len() != self.args.len() {
            let errstring = format!(
                "Function {} expected {} args, got {}\n",
                self.name,
                self.args.len(),
                funargs.len()
            );
            panic!("{}", errstring);
        }
        let mut local_env: BTreeMap<String, ObjRef<B>> = BTreeMap::new();
        if ctx.call_stack_len() != 0 {
            local_env = ctx.call_stack_last_clone();
        }
        for (i, arg) in self.args.iter().enumerate() {
            // `local_env`/`call_stack` is a String-keyed frame map (out of this
            // node's scope); the formal arg name bridges Symbol -> String here.
            local_env.insert(arg.to_string(), funargs[i].clone());
        }
        ctx.call_stack_push(local_env);
        if !self.name.is_empty() {
            ctx.eval_stack_push(self.name.clone());
        }
        let mut retval: HfstTransducer<B> = self.root.evaluate(ctx)?;
        if !self.name.is_empty() {
            ctx.eval_stack_pop();
        }
        retval.set_final_weights(self.weight as f32, true)?;
        ctx.call_stack_pop();
        if ctx.verbose {
            let duration = (clock() - my_timer) as f64 / CLOCKS_PER_SEC as f64;
            write_compilation_stack_indentation_to_err(ctx);
            debug!("Call to {} evaluated in {} seconds", self.name, duration);
            ctx.named_object_evaluation_stack_depth -= 1;
        }
        Ok(retval)
    }

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        let funargs: Vec<ObjRef<B>> = Vec::new();
        self.evaluate_args(ctx, funargs)
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-funcall.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-funcall.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-funcall.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-funcall.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchFuncall<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        if !self.name.is_empty() {
            ctx.eval_stack_push(self.name.clone());
        }
        let evaluated_args: Vec<ObjRef<B>> =
            self.args.iter().map(|it| it.evaluate_as_arg(ctx)).collect();
        let retval = self.fun.evaluate_args(ctx, evaluated_args.clone());
        if !self.name.is_empty() {
            ctx.eval_stack_pop();
        }
        retval
    }
}
// [spec:hfst:def:pmatch-utils.hfst.pmatch-builtin-function.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch-builtin-function.evaluate-fn]
// [spec:hfst:def:pmatch-utils.hfst.pmatch.pmatch-builtin-function.evaluate-fn]
// [spec:hfst:sem:pmatch-utils.hfst.pmatch.pmatch-builtin-function.evaluate-fn]
impl<B: AlgebraBackend + 'static> PmatchObject<B> for PmatchBuiltinFunction<B> {
    pmatch_object_base_accessors!();

    fn evaluate(&self, ctx: &mut PmatchEvalContext<B>) -> crate::error::Result<HfstTransducer<B>> {
        if !self.name.is_empty() {
            ctx.eval_stack_push(self.name.clone());
        }
        let my_timer = self.start_timing(ctx);
        let mut retval: HfstTransducer<B> = HfstTransducer::new();
        if self.ty == PmatchBuiltin::Interpolate {
            if self.args.len() < 3 {
                let errstring = format!(
                    "Builtin function Interpolate called with {} arguments, but it expects at least 3.\n",
                    self.args.len()
                );
                panic!("{}", errstring);
            }
            // arguments are in reverse order after parsing
            let n = self.args.len();
            retval = self.args[n - 2].evaluate(ctx)?;
            let interpolator: HfstTransducer<B> = self.args[n - 1].evaluate(ctx)?;
            for i in (0..(n - 2)).rev() {
                let tmp: HfstTransducer<B> = self.args[i].evaluate(ctx)?;
                retval.concatenate(&interpolator, true)?;
                retval.concatenate(&tmp, true)?;
            }
        }
        retval.set_final_weights(self.weight as f32, true)?;
        self.report_time(ctx, my_timer, String::new());
        if !self.name.is_empty() {
            ctx.eval_stack_pop();
        }
        Ok(retval)
    }
}
