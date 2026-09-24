//! Containment, replace, restriction and substitute: the operators lowered
//! onto 'hfst_xerox_rules'.

use nfst_xre::{
    ContextMark, MappingKind, MappingPair, MappingSide, ReplaceArrow, ReplaceContext, ReplaceRule,
    RestrContext, SubstituteWhat,
};

use super::*;
use crate::convert_transducer_format::ConversionFunctions;
use crate::hfst_data_types::ImplementationType;

// Ported from libhfst/src/parsers/xre_utils.cc and the Replace/Restriction/
// Substitute/Containment-with-weight semantic actions of xre_parse.yy.
//
// Function-arg helpers expect 'name' as stored by 'define_function', with its
// trailing '(' (as in C++ FUNCTION_NAME); the FunctionCall arm re-appends it.

// xre_parse.yy:51 'bool is_weighted()'
fn is_weighted(format: ImplementationType) -> bool {
    format == ImplementationType::TROPICAL_OPENFST_TYPE
}

// xre_parse.yy:41 'float zero_weights(float f)'.
// NOTE: the C++ keeps a 'has_weight_been_zeroed' flag solely to emit a one-time
// "ignoring weights in rule context" warning; 'transform_weights' takes a bare
// 'fn(f32)->f32' that cannot read the instance 'verbose', so that warning is
// dropped. The flag was therefore write-only dead state and is removed entirely;
// the weight-zeroing behaviour (always returning 0.0) is preserved exactly.
fn zero_weights(_f: f32) -> f32 {
    0.0
}

// [spec:hfst:def:xre-utils.hfst.xre.has-non-identity-pairs-fn]
// [spec:hfst:sem:xre-utils.hfst.xre.has-non-identity-pairs-fn]
fn has_non_identity_pairs<B: crate::backend::Backend>(t: &HfstTransducer<B>) -> bool {
    let basic = ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(t)
        .expect("hfst_transducer_to_hfst_basic_transducer on a valid transducer cannot fail");
    let sps = basic.get_transition_pairs();
    for it in sps.iter() {
        if it.0 != it.1 {
            return true;
        }
    }
    false
}

// Builds the '$3' transducer of the substitute symbol-list grammar
// (xre_parse.yy SYMBOL_LIST). An empty list yields the empty transducer
// (the 'SUB3: RIGHT_BRACKET' alternative).
fn build_symbol_list_transducer<B: AlgebraBackend>(
    symbols: &[Symbol],
    cfg: &crate::hfst_transducer::EngineConfig,
) -> crate::error::Result<HfstTransducer<B>> {
    if symbols.is_empty() {
        return Ok(HfstTransducer::new());
    }
    let first = &symbols[0];
    let mut retval = if first.as_str() == crate::hfst_symbol_defs::internal_unknown {
        HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?
    } else {
        HfstTransducer::new_symbol_pair(first, first)?
    };
    for s in symbols.iter().skip(1) {
        let tmp = if s.as_str() == crate::hfst_symbol_defs::internal_unknown {
            HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?
        } else {
            HfstTransducer::new_symbol_pair(s, s)?
        };
        retval.disjunct(&tmp, false)?;
        retval.optimize_with_config(cfg)?;
    }
    Ok(retval)
}

impl<B: AlgebraBackend> XreCompiler<B> {
    // ----------------------------------------------------------------
    // Containment helpers
    // ----------------------------------------------------------------

    // [spec:hfst:def:xre-utils.hfst.xre.contains-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.contains-fn]
    // xre_utils.cc:1082 — [?*] t [?*]
    pub(super) fn contains(
        &self,
        t: &HfstTransducer<B>,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let mut any = HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
        any.repeat_star()?.minimize_with_config(&self.opt_cfg())?;
        let mut retval = any.clone();
        retval.concatenate(t, true)?.concatenate(&any, true)?;
        retval.optimize_with_config(&self.opt_cfg())?;
        Ok(retval)
    }

    // [spec:hfst:def:xre-utils.hfst.xre.contains-with-weight-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.contains-with-weight-fn]
    // xre_utils.cc:1097 — [ 0::weight -> 0 || _ [t] ] - [?* - $[t]]
    pub(super) fn contains_with_weight(
        &self,
        t: &HfstTransducer<B>,
        weight: f32,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let mut weighted_epsilon =
            HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon)?;
        weighted_epsilon.set_final_weights(weight, false)?;
        let epsilon = HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon)?;

        // mapping: 0::weight -> 0
        let mapping_pair_vector: Vec<(HfstTransducer<B>, HfstTransducer<B>)> =
            vec![(weighted_epsilon, epsilon.clone())];

        // context: 0 _ [t]
        let context_pair_vector: Vec<(HfstTransducer<B>, HfstTransducer<B>)> =
            vec![(epsilon, t.clone())];

        let rule = crate::hfst_xerox_rules::Rule::new_mapping_context_repl_type(
            &mapping_pair_vector,
            &context_pair_vector,
            crate::hfst_xerox_rules::ReplaceType::REPL_UP,
        )?;
        let mut weighted_rule = crate::hfst_xerox_rules::replace_rule(&rule, false)?;

        // noT = ?* - $[t]
        let mut no_t = HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
        no_t.repeat_star()?.minimize_with_config(&self.opt_cfg())?;
        let one_or_more_t = self.contains(t)?;
        no_t.subtract(&one_or_more_t, true)?;
        no_t.optimize_with_config(&self.opt_cfg())?;

        // return [weighted_rule - noT]
        weighted_rule.subtract(&no_t, true)?;
        weighted_rule.optimize_with_config(&self.opt_cfg())?;
        Ok(weighted_rule)
    }

    // [spec:hfst:def:xre-utils.hfst.xre.contains-once-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.contains-once-fn]
    // xre_utils.cc:1142
    pub fn contains_once(&self, c: &HfstTransducer<B>) -> crate::error::Result<HfstTransducer<B>> {
        // any_star = [?*]
        let mut any_star = HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
        any_star
            .repeat_star()?
            .minimize_with_config(&self.opt_cfg())?;

        // any_plus = [?+]
        let mut any_plus = HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
        any_plus
            .repeat_plus()?
            .minimize_with_config(&self.opt_cfg())?;

        // t1 = [?+ c ?*]
        let mut t1 = any_plus.clone();
        t1.concatenate(c, true)?;
        t1.optimize_with_config(&self.opt_cfg())?;
        t1.concatenate(&any_star, true)?;
        t1.optimize_with_config(&self.opt_cfg())?;

        // t2 = [c ?*]
        let mut t2 = c.clone();
        t2.concatenate(&any_star, true)?;
        t2.optimize_with_config(&self.opt_cfg())?;

        // t1 = [[?+ c ?*] & [c ?*]]
        t1.intersect(&t2, true)?;

        // t3 = [[c ?+] & c]
        let mut t3 = c.clone();
        t3.concatenate(&any_plus, true)?;
        t3.optimize_with_config(&self.opt_cfg())?;
        t3.intersect(c, true)?;
        t3.optimize_with_config(&self.opt_cfg())?;

        // t1 = [t1 | t3]
        t1.disjunct(&t3, true)?;
        t1.optimize_with_config(&self.opt_cfg())?;

        // cont_t1 = $[t1]
        let cont_t1 = self.contains(&t1)?;
        // cont_c = $[c]
        let mut cont_c = self.contains(c)?;

        // $[c] - $[t1]
        cont_c.subtract(&cont_t1, true)?;
        cont_c.optimize_with_config(&self.opt_cfg())?;
        Ok(cont_c)
    }

    // [spec:hfst:def:xre-utils.hfst.xre.contains-once-optional-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.contains-once-optional-fn]
    // xre_utils.cc:1194
    pub fn contains_once_optional(
        &self,
        t: &HfstTransducer<B>,
    ) -> crate::error::Result<HfstTransducer<B>> {
        // neg_t = ~$[t]
        let cont_t = self.contains(t)?;
        let mut neg_t = HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_identity)?;
        neg_t.repeat_star()?;
        neg_t.optimize_with_config(&self.opt_cfg())?;
        neg_t.subtract(&cont_t, true)?;
        neg_t.optimize_with_config(&self.opt_cfg())?;

        let mut retval = self.contains_once(t)?;
        retval.disjunct(&neg_t, true)?;
        retval.optimize_with_config(&self.opt_cfg())?;
        Ok(retval)
    }

    // Driver dispatch for the '$ E' (CONTAINMENT REGEXP8) production
    // (xre_parse.yy:896). Exposed so the unary '$' arm can call it.
    fn eval_containment(
        &mut self,
        t: &HfstTransducer<B>,
    ) -> crate::error::Result<HfstTransducer<B>> {
        if has_non_identity_pairs(t) {
            if self.verbose {
                // NB: faithfully reproduces the C++ missing-space concatenation.
                self.diag_warning("using transducer that is not an automatonin containment");
            }
            self.contains(t) // ..resort to simple containment
        } else {
            self.contains_with_weight(t, 0.0)
        }
    }

    // Driver dispatch arm for 'XreExpr::ContainmentWithWeight'
    // (CONTAINMENT WEIGHT REGEXP8, xre_parse.yy:910).
    fn eval_containment_with_weight(
        &mut self,
        expr: &SpannedXre,
        weight: f64,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let t = self.eval(expr)?;
        if has_non_identity_pairs(&t) {
            crate::bail!(Hfst, "Containment with weight only works with automata");
        }
        self.contains_with_weight(&t, weight as f32)
    }

    // ----------------------------------------------------------------
    // Warnings used in replace rules
    // ----------------------------------------------------------------

    // [spec:hfst:def:xre-utils.hfst.xre.warn-about-xfst-special-symbol-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.warn-about-xfst-special-symbol-fn]
    // xre_utils.cc:1245. Warns that xfst-special symbols ('all', '<...>') carry
    // no special meaning in hfst. The deferred error stream becomes stderr.
    fn warn_about_xfst_special_symbol(&self, symbol: &str) {
        if symbol == "all" {
            if self.verbose {
                self.diag_warning("symbol 'all' has no special meaning in hfst");
            }
            return;
        }

        let b = symbol.as_bytes();
        if b.is_empty() || b[0] != b'<' {
            return;
        }
        let mut max_index: usize = 1;
        while max_index < b.len() && b[max_index] != 0 {
            max_index += 1;
        }
        max_index -= 1;
        if max_index < 1 {
            return;
        }

        if b[max_index] != b'>' {
            return;
        }
        if !self.verbose {
            return;
        }
        self.diag_warning(&format!("'{} ' is an ordinary symbol in hfst", symbol));
    }

    // [spec:hfst:def:xre-utils.hfst.xre.warn-about-special-symbols-in-replace-fn]
    // [spec:hfst:sem:xre-utils.hfst.xre.warn-about-special-symbols-in-replace-fn]
    fn warn_about_special_symbols_in_replace(
        &self,
        t: &HfstTransducer<B>,
    ) -> crate::error::Result<()> {
        if !self.verbose {
            return Ok(());
        }
        let alphabet = t.get_alphabet()?;
        for it in alphabet.iter() {
            if crate::hfst_transducer::is_special_symbol(it)
                && it.as_str() != crate::hfst_symbol_defs::internal_epsilon
                && it.as_str() != crate::hfst_symbol_defs::internal_unknown
                && it.as_str() != crate::hfst_symbol_defs::internal_identity
            {
                self.diag_warning(&format!(
                    "using special symbol '{}' in replace rule, use substitute instead",
                    it
                ));
            }
        }
        Ok(())
    }

    // ----------------------------------------------------------------
    // Replace (xre_parse.yy: REPLACE / PARALLEL_RULES / RULE / MAPPINGPAIR*)
    // ----------------------------------------------------------------

    // Driver dispatch arm for 'XreExpr::Replace'.
    // Mirrors the 'REPLACE: PARALLEL_RULES' action (xre_parse.yy:365): returns
    // the raw 'replace*' result (the REGEXP2-level '.optimize_with_config(&self.opt_cfg())' is applied by
    // the driver where the grammar reduces a REPLACE to a REGEXP2).
    pub(super) fn eval_replace(
        &mut self,
        arrow: ReplaceArrow,
        rules: &[ReplaceRule],
    ) -> crate::error::Result<HfstTransducer<B>> {
        let mut rule_vector: Vec<crate::hfst_xerox_rules::Rule<B>> = Vec::new();
        for rule in rules.iter() {
            let r = self.build_replace_rule(rule)?;
            rule_vector.push(r);
        }

        Ok(match arrow {
            ReplaceArrow::Right => {
                crate::hfst_xerox_rules::replace_rule_vector(&rule_vector, false)?
            }
            ReplaceArrow::OptionalRight => {
                crate::hfst_xerox_rules::replace_rule_vector(&rule_vector, true)?
            }
            ReplaceArrow::Left => {
                crate::hfst_xerox_rules::replace_left_rule_vector(&rule_vector, false)?
            }
            ReplaceArrow::OptionalLeft => {
                crate::hfst_xerox_rules::replace_left_rule_vector(&rule_vector, true)?
            }
            ReplaceArrow::RtlLongest => {
                crate::hfst_xerox_rules::replace_rightmost_longest_match_rule_vector(&rule_vector)?
            }
            ReplaceArrow::RtlShortest => {
                crate::hfst_xerox_rules::replace_rightmost_shortest_match_rule_vector(&rule_vector)?
            }
            ReplaceArrow::LtrLongest => {
                crate::hfst_xerox_rules::replace_leftmost_longest_match_rule_vector(&rule_vector)?
            }
            ReplaceArrow::LtrShortest => {
                crate::hfst_xerox_rules::replace_leftmost_shortest_match_rule_vector(&rule_vector)?
            }
            // E_REPLACE_RIGHT_MARKUP / no replace left-right arrow in the
            // xfst grammar: the C++ 'xreerror("Unhandled arrow stuff I
            // suppose")' aborted the parse; here it is a propagated parse
            // error so malformed input yields a clean diagnostic, never a panic.
            ReplaceArrow::LeftRight | ReplaceArrow::OptionalLeftRight => {
                crate::bail!(Hfst, "Unhandled arrow stuff I suppose")
            }
        })
    }

    fn build_replace_rule(
        &mut self,
        rule: &ReplaceRule,
    ) -> crate::error::Result<crate::hfst_xerox_rules::Rule<B>> {
        let mut mapping_pair_vector: Vec<(HfstTransducer<B>, HfstTransducer<B>)> = Vec::new();
        for mp in rule.mappings.iter() {
            let pair = self.build_mapping_pair(mp)?;
            mapping_pair_vector.push(pair);
        }

        Ok(match &rule.contexts {
            None => crate::hfst_xerox_rules::Rule::new_mapping(&mapping_pair_vector)?,
            Some(ctxs) => {
                // CONTEXT_MARK -> ReplaceType (xre_parse.yy:673)
                let repl_type = match ctxs.mark {
                    ContextMark::UpperUpper => crate::hfst_xerox_rules::ReplaceType::REPL_UP,
                    ContextMark::LowerUpper => crate::hfst_xerox_rules::ReplaceType::REPL_RIGHT,
                    ContextMark::UpperLower => crate::hfst_xerox_rules::ReplaceType::REPL_LEFT,
                    ContextMark::LowerLower => crate::hfst_xerox_rules::ReplaceType::REPL_DOWN,
                };
                let mut context_vector: Vec<(HfstTransducer<B>, HfstTransducer<B>)> = Vec::new();
                for cx in ctxs.items.iter() {
                    let pair = self.build_replace_context(cx)?;
                    context_vector.push(pair);
                }
                crate::hfst_xerox_rules::Rule::new_mapping_context_repl_type(
                    &mapping_pair_vector,
                    &context_vector,
                    repl_type,
                )?
            }
        })
    }

    // xre_parse.yy MAPPINGPAIR alternatives.
    fn build_mapping_pair(
        &mut self,
        mp: &MappingPair,
    ) -> crate::error::Result<(HfstTransducer<B>, HfstTransducer<B>)> {
        let upper = self.eval_mapping_side(&mp.upper)?;

        Ok(match &mp.kind {
            MappingKind::Plain { lower } => {
                let lower_tr = self.eval_mapping_side(lower)?;
                // Only the bare 'A -> B' production warns (the dotted forms do
                // not): warn iff both sides are plain expressions.
                if matches!(mp.upper, MappingSide::Expr(_)) && matches!(lower, MappingSide::Expr(_))
                {
                    self.warn_about_special_symbols_in_replace(&upper)?;
                    self.warn_about_special_symbols_in_replace(&lower_tr)?;
                }
                (upper, lower_tr)
            }
            MappingKind::Markup { pre, post } => {
                // marks = (pre|0, post|0); tmpMappingPair = (upper, <empty>)
                let left_mark = match pre {
                    Some(s) => self.eval_mapping_side(s)?,
                    None => HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon)?,
                };
                let right_mark = match post {
                    Some(s) => self.eval_mapping_side(s)?,
                    None => HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon)?,
                };
                let marks = (left_mark, right_mark);
                let tmp_mapping_pair = (upper, HfstTransducer::new());
                crate::hfst_xerox_rules::create_mapping_for_mark_up_replace(
                    &tmp_mapping_pair,
                    &marks,
                )?
            }
        })
    }

    // A mapping side: bare expr, '[. E .]', or '[..]' (-> epsilon).
    fn eval_mapping_side(&mut self, side: &MappingSide) -> crate::error::Result<HfstTransducer<B>> {
        Ok(match side {
            MappingSide::Expr(e) => self.eval(e)?,
            MappingSide::Dotted(None) => {
                HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon)?
            }
            MappingSide::Dotted(Some(e)) => self.eval(e)?,
        })
    }

    // xre_parse.yy CONTEXT alternatives (replace contexts). Empty side -> 0.
    // Contexts must be automata, weights are zeroed, then optimize+prune.
    fn build_replace_context(
        &mut self,
        c: &ReplaceContext,
    ) -> crate::error::Result<(HfstTransducer<B>, HfstTransducer<B>)> {
        let weighted = is_weighted(B::TYPE);

        Ok(match (&c.left, &c.right) {
            (Some(l), Some(r)) => {
                let mut t1 = self.eval(l)?;
                let mut t2 = self.eval(r)?;
                if has_non_identity_pairs(&t1) {
                    crate::bail!(Hfst, "Contexts need to be automata");
                }
                if has_non_identity_pairs(&t2) {
                    crate::bail!(Hfst, "Contexts need to be automata");
                }
                if weighted {
                    t1.transform_weights(zero_weights)?;
                }
                t1.optimize_with_config(&self.opt_cfg())?
                    .prune_alphabet(false)?;
                if weighted {
                    t2.transform_weights(zero_weights)?;
                }
                t2.optimize_with_config(&self.opt_cfg())?
                    .prune_alphabet(false)?;
                (t1, t2)
            }
            (Some(l), None) => {
                let mut t1 = self.eval(l)?;
                if has_non_identity_pairs(&t1) {
                    crate::bail!(Hfst, "Contexts need to be automata");
                }
                if weighted {
                    t1.transform_weights(zero_weights)?;
                }
                t1.optimize_with_config(&self.opt_cfg())?
                    .prune_alphabet(false)?;
                (
                    t1,
                    HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon)?,
                )
            }
            (None, Some(r)) => {
                let mut t1 = self.eval(r)?;
                if has_non_identity_pairs(&t1) {
                    crate::bail!(Hfst, "Contexts need to be automata");
                }
                if weighted {
                    t1.transform_weights(zero_weights)?;
                }
                t1.optimize_with_config(&self.opt_cfg())?
                    .prune_alphabet(false)?;
                (
                    HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon)?,
                    t1,
                )
            }
            (None, None) => {
                let epsilon =
                    HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon)?;
                (epsilon.clone(), epsilon)
            }
        })
    }

    // ----------------------------------------------------------------
    // Restriction (xre_parse.yy REGEXP4 RIGHT_ARROW RESTR_CONTEXTS_VECTOR)
    // ----------------------------------------------------------------

    // Driver dispatch arm for 'XreExpr::Restriction'.
    pub(super) fn eval_restriction(
        &mut self,
        body: &SpannedXre,
        contexts: &[RestrContext],
    ) -> crate::error::Result<HfstTransducer<B>> {
        let center = self.eval(body)?;
        let mut context_vector: Vec<(HfstTransducer<B>, HfstTransducer<B>)> = Vec::new();
        for c in contexts.iter() {
            let pair = self.build_restr_context(c)?;
            context_vector.push(pair);
        }
        crate::hfst_xerox_rules::restriction(&center, &context_vector)
    }

    // xre_parse.yy RESTR_CONTEXT alternatives. One missing side -> 0 (epsilon),
    // both missing -> <empty> (the bare '_' form).
    fn build_restr_context(
        &mut self,
        c: &RestrContext,
    ) -> crate::error::Result<(HfstTransducer<B>, HfstTransducer<B>)> {
        Ok(match (&c.left, &c.right) {
            (Some(l), Some(r)) => (self.eval(l)?, self.eval(r)?),
            (Some(l), None) => {
                let t1 = self.eval(l)?;
                (
                    t1,
                    HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon)?,
                )
            }
            (None, Some(r)) => {
                let t1 = self.eval(r)?;
                (
                    HfstTransducer::new_symbol(crate::hfst_symbol_defs::internal_epsilon)?,
                    t1,
                )
            }
            (None, None) => (HfstTransducer::new(), HfstTransducer::new()),
        })
    }

    // ----------------------------------------------------------------
    // Substitute (xre_parse.yy: SUB1 ... productions)
    // ----------------------------------------------------------------

    // Driver dispatch arm for 'XreExpr::Substitute'.
    pub(super) fn eval_substitute(
        &mut self,
        haystack: &SpannedXre,
        what: &SubstituteWhat,
    ) -> crate::error::Result<HfstTransducer<B>> {
        let mut hay = self.eval(haystack)?;

        Ok(match what {
            // '[ E, a:b, c:d ]  (xre_parse.yy:268)
            SubstituteWhat::Pair { from, to } => {
                // The parser AST carries plain Strings; convert at the symbol
                // boundary.
                hay.substitute_pair_with_pair(
                    &(Symbol::new(&from.0), Symbol::new(&from.1)),
                    &(Symbol::new(&to.0), Symbol::new(&to.1)),
                )?;
                hay.optimize_with_config(&self.opt_cfg())?;
                hay
            }
            // '[ E, b, x y ]  (xre_parse.yy:276 SUB1 SUB2 SUB3)
            SubstituteWhat::Symbol {
                needle,
                replacement,
            } => {
                let hay_alpha = hay.get_alphabet()?;

                if self.definitions.contains_key(needle.as_str()) {
                    if self.verbose {
                        self.diag_warning(
                            "using definition as an ordinary label, cannot substitute",
                        );
                    }
                    hay.optimize_with_config(&self.opt_cfg())?;
                    return Ok(hay);
                }
                if !hay_alpha.contains(needle.as_str()) {
                    hay.optimize_with_config(&self.opt_cfg())?;
                    return Ok(hay);
                }

                // alpha is reassigned to the replacement's alphabet (used both
                // for the diacritic loop and the final remove-from-alphabet).
                let mut repl_tr: HfstTransducer<B> =
                    build_symbol_list_transducer(replacement, &self.opt_cfg())?;
                let alpha3 = repl_tr.get_alphabet()?;
                let tmp = (Symbol::new(needle), Symbol::new(needle));
                let mut tmp_tr = hay.clone();

                let empty = HfstTransducer::new();
                let mut empty_replace_transducer = false;
                if empty.compare(&repl_tr, true)? {
                    empty_replace_transducer = true;
                }
                if empty_replace_transducer {
                    // substitute all transitions {b:a, a:b, b:b} with b:b. The
                    // former 'substitution_function_symbol' global is captured by
                    // this closure instead.
                    let needle_sym = Symbol::new(needle);
                    tmp_tr.substitute_with_func(|p, sps| {
                        if p.0 == needle_sym || p.1 == needle_sym {
                            sps.insert((needle_sym.clone(), needle_sym.clone()));
                            return true;
                        }
                        false
                    })?;
                }
                // substitute b with x | y (no harmonization)
                tmp_tr.substitute_pair_with_transducer(&tmp, &mut repl_tr, false)?;

                if !empty_replace_transducer {
                    // [[a:b].i .o. b -> x|y].i (handles b appearing on the left side)
                    let mapping_pair = (
                        HfstTransducer::new_symbol_pair(needle, needle)?,
                        repl_tr.clone(),
                    );
                    let mapping_pair_vector: Vec<(HfstTransducer<B>, HfstTransducer<B>)> =
                        vec![mapping_pair];
                    let rule = crate::hfst_xerox_rules::Rule::new_mapping(&mapping_pair_vector)?;
                    let mut replace_tr = crate::hfst_xerox_rules::replace_rule(&rule, false)?;

                    // allow flag diacritics to be replaced with themselves
                    for it in alpha3.iter() {
                        if crate::hfst_flag_diacritics::FdOperation::is_diacritic(it) {
                            replace_tr.insert_freely_pair(&(it.clone(), it.clone()), false)?;
                        }
                    }
                    replace_tr.optimize_with_config(&self.opt_cfg())?;
                    tmp_tr
                        .compose_with_config(&replace_tr, true, &self.opt_cfg())?
                        .optimize_with_config(&self.opt_cfg())?;
                    tmp_tr
                        .invert()?
                        .compose_with_config(&replace_tr, true, &self.opt_cfg())?
                        .invert()?;
                }

                if !alpha3.contains(needle.as_str()) {
                    tmp_tr.remove_from_alphabet_string(needle.as_str())?;
                }
                tmp_tr.optimize_with_config(&self.opt_cfg())?;
                tmp_tr
            }
        })
    }
}
