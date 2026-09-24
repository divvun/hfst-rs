//! The replace interface functions: markup mappings and the replace,
//! directed-match, and epenthesis rule builders.

use super::*;

//---------------------------------
//    INTERFACE HELPING FUNCTIONS
//---------------------------------

// used by hfst-regexp parser
// creates markup crossproduct and sets property of the first transducer in the mapping to "isMarkup" = "yes"
// the other transducer in the mapping is set to epsilon transducer
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.create-mapping-for-mark-up-replace-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.create-mapping-for-mark-up-replace-fn]
pub fn create_mapping_for_mark_up_replace<B: AlgebraBackend>(
    mapping_pair: &HfstTransducerPair<B>,
    marks: &HfstTransducerPair<B>,
) -> crate::error::Result<HfstTransducerPair<B>> {
    let mut tok = HfstTokenizer::new();
    let epsilon = String::from("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol(&epsilon);
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let left_mark = marks.0.clone();
    let right_mark = marks.1.clone();

    let mut epsilon_to_left_mark = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?;
    epsilon_to_left_mark
        .cross_product(&left_mark, true)?
        .optimize()?;

    let mut epsilon_to_right_mark = HfstTransducer::new_tokenized(&epsilon, &tok)?;
    epsilon_to_right_mark
        .cross_product(&right_mark, true)?
        .optimize()?;

    //Go through left part of every mapping pair
    // and concatenate: epsilon_to_left_mark.leftMapping.epsilon_to_right_mark
    //then put it into right part of the new transducerPairVector
    let mut mapping_cross_product = epsilon_to_left_mark.clone();
    mapping_cross_product
        .concatenate(&mapping_pair.0, true)?
        .concatenate(&epsilon_to_right_mark, true)?
        .optimize()?;

    mapping_cross_product.set_property("isMarkup", "yes");

    let epsilon_tr = HfstTransducer::new_tokenized(&epsilon, &tok)?;
    let retval: HfstTransducerPair<B> = (mapping_cross_product, epsilon_tr);

    Ok(retval)
}

// DIVERGENCE from upstream C++ (fixes hfst/hfst#571):
//
// An obligatory epenthesis rule whose left-hand side is epsilon and whose
// context is empty — e.g. `[] -> a`, `0 -> a`, `[..] -> a`, all of which parse
// to an @0@:a center — must NOT force one insertion at every position while
// dropping the identity string. Upstream's most_brackets_star_constraint
// (HfstXeroxRules.cc:2402-2437, applied at :706-717 when !optional) does exactly
// that, yielding a 2-state machine where `xy -> axaya` ONLY. The intended
// (and already-correct) semantics are the ones the optional arrow produces:
// free insertion at every position WITH identity preserved.
//
// So for an epsilon-LHS + empty-context rule we route the non-optional path to
// the optional one: we skip most_brackets_star_constraint. A context-full
// epenthesis rule (e.g. `0 -> p || m _ k`) is unaffected — its context is not
// empty, so this returns false and the obligatory constraint still applies.
//
// The check MUST run on a flag-encoded rule so that flag diacritics encoded
// into a context are not misread as an empty context (mirrors bracketed_replace,
// which encode_flags() before inspecting the context; see the flag-complement
// audit deferral in test_flag_complement.rs:402-415).
fn is_epsilon_lhs_empty_context<B: AlgebraBackend>(rule: &Rule<B>) -> crate::error::Result<bool> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let epsilon: HfstTransducer<B> = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?;

    // Evaluate AFTER encode_flags() so encoded flags in a context are not
    // mistaken for an empty context.
    let mut ruletmp = rule.clone();
    ruletmp.encode_flags()?;

    let mapping = ruletmp.get_mapping();
    let context = ruletmp.get_context();

    // Empty (universal) context: exactly one epsilon:epsilon pair, matching the
    // empty-context short circuit in bracketed_replace (:821-825/:1021-1024).
    if context.len() != 1 {
        return Ok(false);
    }
    if !(context[0].0.compare(&epsilon, true)? && context[0].1.compare(&epsilon, true)?) {
        return Ok(false);
    }

    // Epsilon left-hand side: every mapping pair maps epsilon on its left.
    if mapping.is_empty() {
        return Ok(false);
    }
    for pair in mapping.iter() {
        if !pair.0.compare(&epsilon, true)? {
            return Ok(false);
        }
    }

    Ok(true)
}

// replace up, left, right, down
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-epenthesis-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-epenthesis-fn]
pub fn replace_rule<B: AlgebraBackend>(
    rule: &Rule<B>,
    optional: bool,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut retval: HfstTransducer<B> = bracketed_replace(rule, optional)?;

    //printf("---bracketed replace done---: \n");
    //retval.optimize().write_in_att_format(stdout, 1);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row

    retval = no_repetition_constraint(&retval)?;

    //printf("-----no_repetition_constraint-----: \n");
    //retval.write_in_att_format(stdout, 1);

    // deals with boundary symbol, must be before most_brackets_star_constraint
    retval = apply_boundary_mark(&retval)?;

    //printf("----after apply_boundary_mark: ----\n");
    //retval.write_in_att_format(stdout, 1);
    // hfst/hfst#571: an epsilon-LHS + empty-context rule must not be forced to
    // insert at every position (see is_epsilon_lhs_empty_context above); treat
    // it as optional and skip most_brackets_star_constraint.
    if !optional && !is_epsilon_lhs_empty_context(rule)? {
        //printf(" ----------  most_brackets_star_constraint --------------\n");
        // Epenthesis rules behave differently if used most_brackets_plus_constraint
        //retval = most_brackets_plus_constraint(retval);
        retval = most_brackets_star_constraint(&retval)?;
        //printf("after non optional: \n");
        //retval.write_in_att_format(stdout, 1);
    }
    retval = remove_b2_constraint(&retval)?;
    retval = remove_markers(&retval)?;
    //printf("after remove_markers: \n");
    //retval.write_in_att_format(stdout, 1);
    Ok(retval)
}

// for parallel rules
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-fn]
pub fn replace_rule_vector<B: AlgebraBackend>(
    rule_vector: &[Rule<B>],
    optional: bool,
) -> crate::error::Result<HfstTransducer<B>> {
    // std::cerr << "replace"<< std::endl;

    // If there is only one rule in the vector, it is not parallel
    let mut retval: HfstTransducer<B> = if rule_vector.len() == 1 {
        bracketed_replace(&rule_vector[0], optional)?
    } else {
        parallel_bracketed_replace(rule_vector, optional)?
    };

    //std::cerr << "after bracketed replace"<< std::endl;
    //         printf("- bracketed replace -\n");
    //         retval.write_in_att_format(stdout, 1);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    retval = no_repetition_constraint(&retval)?;

    //   printf("----after no_repetition_constraint: ----\n");
    //   retval.write_in_att_format(stdout, 1);

    // deals with boundary symbol
    retval = apply_boundary_mark(&retval)?;

    //printf("----after apply_boundary_mark: ----\n");
    //retval.write_in_att_format(stdout, 1);

    // hfst/hfst#571: skip the obligatory constraint when every rule in the
    // vector is an epsilon-LHS + empty-context epenthesis (see
    // is_epsilon_lhs_empty_context). If any rule has a real context or a
    // non-epsilon LHS the constraint still applies.
    let mut all_epsilon_empty = !rule_vector.is_empty();
    for rule in rule_vector.iter() {
        if !is_epsilon_lhs_empty_context(rule)? {
            all_epsilon_empty = false;
            break;
        }
    }
    if !optional && !all_epsilon_empty {
        // Epenthesis rules behave differently if used most_brackets_plus_constraint
        // retval = most_brackets_plus_constraint(retval);
        retval = most_brackets_star_constraint(&retval)?;
    }

    // printf("----after most_brackets_star_constraint: ----\n");
    //  retval.write_in_att_format(stdout, 1);

    retval = remove_b2_constraint(&retval)?;

    //printf("----after remove_b2_constraint: ----\n");
    // retval.write_in_att_format(stdout, 1);

    retval = remove_markers(&retval)?;

    //printf("----after remove_markers: ----\n");
    //retval.write_in_att_format(stdout, 1);
    Ok(retval)
}

// replace left
pub fn replace_left_rule<B: AlgebraBackend>(
    rule: &Rule<B>,
    optional: bool,
) -> crate::error::Result<HfstTransducer<B>> {
    let mapping_pair_vector: HfstTransducerPairVector<B> = rule.get_mapping();
    //HfstTransducer newMapping = rule.get_mapping();
    //newMapping.invert().optimize();

    let mut new_mapping_pair_vector: HfstTransducerPairVector<B> = HfstTransducerPairVector::new();
    for pair in &mapping_pair_vector {
        // in every mapping pair invert first and second
        //HfstTransducer newMapping = rule.get_mapping();
        let first: HfstTransducer<B> = pair.0.clone();
        let second: HfstTransducer<B> = pair.1.clone();
        new_mapping_pair_vector.push((second, first));
    }

    let new_rule: Rule<B> = Rule::new_mapping_context_repl_type(
        &new_mapping_pair_vector,
        &rule.get_context(),
        rule.get_repl_type(),
    )?;
    let mut retval: HfstTransducer<B> = replace_rule(&new_rule, optional)?;

    retval.invert()?.optimize()?;
    Ok(retval)
}

// replace left parallel
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-left-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-left-fn]
pub fn replace_left_rule_vector<B: AlgebraBackend>(
    rule_vector: &[Rule<B>],
    optional: bool,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut left_rule_vector: Vec<Rule<B>> = Vec::new();

    for rule in rule_vector {
        let mapping_pair_vector: HfstTransducerPairVector<B> = rule.get_mapping();
        //HfstTransducer newMapping = rule.get_mapping();
        //newMapping.invert().optimize();

        let mut new_mapping_pair_vector: HfstTransducerPairVector<B> =
            HfstTransducerPairVector::new();
        for pair in &mapping_pair_vector {
            // in every mapping pair invert first and second
            //HfstTransducer newMapping = rule.get_mapping();
            let first: HfstTransducer<B> = pair.0.clone();
            let second: HfstTransducer<B> = pair.1.clone();
            new_mapping_pair_vector.push((second, first));
        }

        let new_rule: Rule<B> = Rule::new_mapping_context_repl_type(
            &new_mapping_pair_vector,
            &rule.get_context(),
            rule.get_repl_type(),
        )?;

        left_rule_vector.push(new_rule);
    }

    let mut retval: HfstTransducer<B> = replace_rule_vector(&left_rule_vector, optional)?;
    retval.invert()?.optimize()?;

    Ok(retval)
}

// left to right
pub fn replace_leftmost_longest_match_rule<B: AlgebraBackend>(
    rule: &Rule<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut unconditional_tr: HfstTransducer<B> = bracketed_replace(rule, true)?;
    //unconditional_tr = bracketed_replace(rule, true);

    //printf("LM unconditional_tr: \n");
    //unconditional_tr.write_in_att_format(stdout, 1);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    // it should be before left_most_constraint
    unconditional_tr = no_repetition_constraint(&unconditional_tr)?;

    let mut retval: HfstTransducer<B> = left_most_constraint(&unconditional_tr)?;

    //to remove empty strings
    retval = one_betterthan_none_constraint(&retval)?;

    // printf("left_most_constraint: \n");
    // retval.write_in_att_format(stdout, 1);
    retval = longest_match_left_most_constraint(&retval)?;

    //printf("longest_match_left_most_constraint: \n");
    //retval.write_in_att_format(stdout, 1);

    retval = remove_b2_constraint(&retval)?;
    retval = remove_markers(&retval)?;

    // deals with boundary symbol
    retval = apply_boundary_mark(&retval)?;

    Ok(retval)
}

// left to right
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-leftmost-longest-match-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-leftmost-longest-match-fn]
pub fn replace_leftmost_longest_match_rule_vector<B: AlgebraBackend>(
    rule_vector: &[Rule<B>],
) -> crate::error::Result<HfstTransducer<B>> {
    //printf("\n replace_leftmost_longest_match \n");

    let mut unconditional_tr: HfstTransducer<B> = if rule_vector.len() == 1 {
        bracketed_replace(&rule_vector[0], true)?
    } else {
        parallel_bracketed_replace(rule_vector, true)?
    };

    //printf("retval unconditional 1 \n");
    // unconditional_tr.write_in_att_format(stdout, 1);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    // it should be before left_most_constraint
    unconditional_tr = no_repetition_constraint(&unconditional_tr)?;
    //printf("unconditional_tr epenthesis \n");
    //unconditional_tr.write_in_att_format(stdout, 1);

    let mut retval: HfstTransducer<B> = left_most_constraint(&unconditional_tr)?;

    //to remove empty strings
    retval = one_betterthan_none_constraint(&retval)?;

    retval = longest_match_left_most_constraint(&retval)?;
    //printf("retval longest_match_left_most_constraint \n");
    //retval.write_in_att_format(stdout, 1);

    // remove LM2, RM2
    retval = remove_b2_constraint(&retval)?;

    //printf("retval remove_b2_constraint \n");
    //retval.write_in_att_format(stdout, 1);

    retval = remove_markers(&retval)?;

    //       printf("LM remove_markers: \n");
    //        retval.write_in_att_format(stdout, 1);

    // deals with boundary symbol
    retval = apply_boundary_mark(&retval)?;

    // printf("LM apply_boundary_mark: \n");
    // retval.write_in_att_format(stdout, 1);

    Ok(retval)
}

// right to left
pub fn replace_rightmost_longest_match_rule<B: AlgebraBackend>(
    rule: &Rule<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let unconditional_tr: HfstTransducer<B> = bracketed_replace(rule, true)?;
    //unconditional_tr = bracketed_replace(rule, true);

    let mut retval: HfstTransducer<B> = right_most_constraint(&unconditional_tr)?;
    //retval = right_most_constraint(unconditional_tr);

    //printf("right_most_constraint: \n");
    //retval.write_in_att_format(stdout, 1);

    retval = longest_match_right_most_constraint(&retval)?;

    //printf("longest_match_left_most_constraint: \n");
    //retval.write_in_att_format(stdout, 1);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    retval = no_repetition_constraint(&retval)?;
    // remove LM2, RM2
    retval = remove_b2_constraint(&retval)?;

    retval = remove_markers(&retval)?;

    // deals with boundary symbol
    retval = apply_boundary_mark(&retval)?;

    Ok(retval)
}

// right to left
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-rightmost-longest-match-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-rightmost-longest-match-fn]
pub fn replace_rightmost_longest_match_rule_vector<B: AlgebraBackend>(
    rule_vector: &[Rule<B>],
) -> crate::error::Result<HfstTransducer<B>> {
    let unconditional_tr: HfstTransducer<B> = if rule_vector.len() == 1 {
        bracketed_replace(&rule_vector[0], true)?
    } else {
        parallel_bracketed_replace(rule_vector, true)?
    };

    let mut retval: HfstTransducer<B> = right_most_constraint(&unconditional_tr)?;
    //retval = right_most_constraint(unconditional_tr);

    //printf("right_most_constraint: \n");
    //retval.write_in_att_format(stdout, 1);

    retval = longest_match_right_most_constraint(&retval)?;

    //printf("longest_match_left_most_constraint: \n");
    //retval.write_in_att_format(stdout, 1);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    retval = no_repetition_constraint(&retval)?;
    // remove LM2, RM2
    retval = remove_b2_constraint(&retval)?;

    retval = remove_markers(&retval)?;

    // deals with boundary symbol
    retval = apply_boundary_mark(&retval)?;

    Ok(retval)
}

pub fn replace_leftmost_shortest_match_rule<B: AlgebraBackend>(
    rule: &Rule<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut unconditional_tr: HfstTransducer<B> = bracketed_replace(rule, true)?;
    //    unconditional_tr = bracketed_replace(rule, true);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    //has to be before left_most_constraint
    unconditional_tr = no_repetition_constraint(&unconditional_tr)?;

    let mut retval: HfstTransducer<B> = left_most_constraint(&unconditional_tr)?;
    //to remove empty strings
    retval = one_betterthan_none_constraint(&retval)?;

    retval = shortest_match_left_most_constraint(&retval)?;

    //printf("sh tr: \n");
    //retval.write_in_att_format(stdout, 1);

    // remove LM2, RM2
    retval = remove_b2_constraint(&retval)?;

    retval = remove_markers(&retval)?;

    // deals with boundary symbol
    retval = apply_boundary_mark(&retval)?;

    Ok(retval)
}

// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-leftmost-shortest-match-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-leftmost-shortest-match-fn]
pub fn replace_leftmost_shortest_match_rule_vector<B: AlgebraBackend>(
    rule_vector: &[Rule<B>],
) -> crate::error::Result<HfstTransducer<B>> {
    let mut unconditional_tr: HfstTransducer<B> = if rule_vector.len() == 1 {
        bracketed_replace(&rule_vector[0], true)?
    } else {
        parallel_bracketed_replace(rule_vector, true)?
    };

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    unconditional_tr = no_repetition_constraint(&unconditional_tr)?;

    let mut retval: HfstTransducer<B> = left_most_constraint(&unconditional_tr)?;

    //to remove empty strings
    retval = one_betterthan_none_constraint(&retval)?;

    retval = shortest_match_left_most_constraint(&retval)?;

    //printf("sh tr: \n");
    //retval.write_in_att_format(stdout, 1);

    // remove LM2, RM2
    retval = remove_b2_constraint(&retval)?;

    retval = remove_markers(&retval)?;

    // deals with boundary symbol
    retval = apply_boundary_mark(&retval)?;

    Ok(retval)
}

pub fn replace_rightmost_shortest_match_rule<B: AlgebraBackend>(
    rule: &Rule<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let unconditional_tr: HfstTransducer<B> = bracketed_replace(rule, true)?;
    //unconditional_tr = bracketed_replace( rule, true);

    let mut retval: HfstTransducer<B> = right_most_constraint(&unconditional_tr)?;
    //retval = right_most_constraint(unconditional_tr);
    retval = shortest_match_right_most_constraint(&retval)?;

    //printf("sh tr: \n");
    //retval.write_in_att_format(stdout, 1);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    retval = no_repetition_constraint(&retval)?;
    // remove LM2, RM2
    retval = remove_b2_constraint(&retval)?;

    retval = remove_markers(&retval)?;

    // deals with boundary symbol
    retval = apply_boundary_mark(&retval)?;

    Ok(retval)
}

// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.replace-rightmost-shortest-match-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.replace-rightmost-shortest-match-fn]
pub fn replace_rightmost_shortest_match_rule_vector<B: AlgebraBackend>(
    rule_vector: &[Rule<B>],
) -> crate::error::Result<HfstTransducer<B>> {
    let unconditional_tr: HfstTransducer<B> = if rule_vector.len() == 1 {
        bracketed_replace(&rule_vector[0], true)?
    } else {
        parallel_bracketed_replace(rule_vector, true)?
    };
    let mut retval: HfstTransducer<B> = right_most_constraint(&unconditional_tr)?;
    //retval = right_most_constraint(unconditional_tr);
    retval = shortest_match_right_most_constraint(&retval)?;

    //printf("sh tr: \n");
    //retval.write_in_att_format(stdout, 1);

    // for epenthesis rules
    // it can't have more than one epsilon repetition in a row
    retval = no_repetition_constraint(&retval)?;
    // remove LM2, RM2
    retval = remove_b2_constraint(&retval)?;

    retval = remove_markers(&retval)?;

    // deals with boundary symbol
    retval = apply_boundary_mark(&retval)?;

    Ok(retval)
}
