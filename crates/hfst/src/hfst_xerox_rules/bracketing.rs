//! Bracketed replace: marker insertion, context expansion, and the
//! single-rule and parallel bracketed-replace builders.

use super::*;

//////////////////////////////////////
// Port of 'libhfst/src/HfstXeroxRules.cc' lines 320..1500 (functions defined in
// that span). Sibling areas of 'crate::hfst_xerox_rules' own everything outside
// this span (e.g. 'decode_flag_diacritics', 'Rule', 'ReplaceType', the 'replace*'
// interface functions); they reach this module via 'use super::*'.

// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.remove-markers-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.remove-markers-fn]
pub fn remove_markers<B: AlgebraBackend>(
    tr: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut retval = tr.clone();

    let left_marker: Symbol = Symbol::new_static("@LM@");
    let right_marker: Symbol = Symbol::new_static("@RM@");

    retval
        .substitute_pair_with_pair(
            &(left_marker.clone(), left_marker.clone()),
            &(
                Symbol::new_static("@_EPSILON_SYMBOL_@"),
                Symbol::new_static("@_EPSILON_SYMBOL_@"),
            ),
        )?
        .optimize()?;
    retval
        .substitute_pair_with_pair(
            &(right_marker.clone(), right_marker.clone()),
            &(
                Symbol::new_static("@_EPSILON_SYMBOL_@"),
                Symbol::new_static("@_EPSILON_SYMBOL_@"),
            ),
        )?
        .optimize()?;

    retval.remove_from_alphabet_string(&left_marker)?;
    retval.remove_from_alphabet_string(&right_marker)?;

    retval.optimize()?;

    retval = decode_flag_diacritics(&retval)?;

    Ok(retval)
}

// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.zero-weight-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.zero-weight-fn]
pub fn zero_weight(f: f32) -> f32 {
    let _ = f;
    0.0
}

/*
 * Generalized Lenient Composition, described in Anssi Yli-Jyrä 2008b
 */
// tmp = t.1 .o. Constr .o. t.1
// (t.1 - tmp.2) .o. t
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.constraint-composition-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.constraint-composition-fn]
pub fn constraint_composition<B: AlgebraBackend>(
    t: &HfstTransducer<B>,
    constraint: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut retval = t.clone();
    retval.transform_weights(zero_weight)?;

    retval.input_project()?.optimize()?;

    let mut tmp = retval.clone();
    tmp.compose(constraint, true)?.optimize()?;

    tmp.compose(&retval, true)?.optimize()?;
    tmp.output_project()?.optimize()?;
    retval.subtract(&tmp, true)?.optimize()?;

    //transform weights to zero
    retval.transform_weights(zero_weight)?;
    retval.compose(t, true)?.optimize()?;

    Ok(retval)
}

// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.insert-freely-all-the-brackets-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.insert-freely-all-the-brackets-fn]
pub fn insert_freely_all_the_brackets<B: AlgebraBackend>(
    t: &mut HfstTransducer<B>,
    optional: bool,
) -> crate::error::Result<()> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let left_marker: String = "@LM@".to_string();
    let right_marker: String = "@RM@".to_string();
    let left_marker2: String = "@LM2@".to_string();
    let right_marker2: String = "@RM2@".to_string();

    tok.add_multichar_symbol(&left_marker);
    tok.add_multichar_symbol(&right_marker);
    tok.add_multichar_symbol(&left_marker2);
    tok.add_multichar_symbol(&right_marker2);

    let left_bracket = HfstTransducer::new_tokenized(&left_marker, &tok)?;
    let right_bracket = HfstTransducer::new_tokenized(&right_marker, &tok)?;

    t.insert_freely(&left_bracket, false)?.optimize()?;
    t.insert_freely(&right_bracket, false)?.optimize()?;

    if !optional {
        let left_bracket2 = HfstTransducer::new_tokenized(&left_marker2, &tok)?;
        let right_bracket2 = HfstTransducer::new_tokenized(&right_marker2, &tok)?;

        t.insert_freely(&left_bracket2, false)?.optimize()?;
        t.insert_freely(&right_bracket2, false)?.optimize()?;
    }
    Ok(())
}

// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.expand-contexts-with-mapping-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.expand-contexts-with-mapping-fn]
pub fn expand_contexts_with_mapping<B: AlgebraBackend>(
    context_vector: &HfstTransducerPairVector<B>,
    mapping_with_brackets_and_tmp_boundary: &HfstTransducer<B>,
    identity_expanded: &HfstTransducer<B>,
    repl_type: ReplaceType,
    optional: bool,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut union_context_replace = HfstTransducer::new();

    let mut tok = HfstTokenizer::new();
    // tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    // HfstTransducer epsilon("@_EPSILON_SYMBOL_@", tok, type);

    for context_pair in context_vector.iter() {
        // Expand context with mapping
        // Cr' = (Rc .*) << Markers (<,>,|) .o. [I:I | <a:b>]*
        // Cr = Cr|Cr'
        // (same for left context)

        // Lc = (*. Lc) << {<,>}

        let identity_pair = HfstTransducer::identity_pair();
        let mut identity_star = identity_pair.clone();
        identity_star.repeat_star()?;

        let mut first_context = identity_star.clone();
        first_context.concatenate(&context_pair.0, true)?;
        first_context.transform_weights(zero_weight)?;
        first_context.optimize()?;

        insert_freely_all_the_brackets(&mut first_context, optional)?;

        // Rc =  (Rc .*) << {<,>}
        let mut second_context = context_pair.1.clone();
        second_context.concatenate(&identity_star, true)?;
        second_context.transform_weights(zero_weight)?;
        second_context.optimize()?;
        insert_freely_all_the_brackets(&mut second_context, optional)?;

        /* RULE:    LC:        RC:
         * up        up        up
         * left        up        down
         * right    down    up
         * down        down    down
         */

        let mut left_context_expanded = HfstTransducer::new();
        let mut right_context_expanded = HfstTransducer::new();

        // both contexts are in upper language
        if repl_type == ReplaceType::REPL_UP {
            // compose them with [I:I | <a:b>]*
            left_context_expanded = first_context.clone();
            right_context_expanded = second_context.clone();

            left_context_expanded.compose(identity_expanded, true)?;
            right_context_expanded.compose(identity_expanded, true)?;
        }
        // left context is in lower language, right in upper ( // )
        if repl_type == ReplaceType::REPL_RIGHT {
            // compose them with [I:I | <a:b>]*

            // left compose opposite way
            left_context_expanded = identity_expanded.clone();
            right_context_expanded = second_context.clone();

            left_context_expanded.compose(&first_context, true)?;
            right_context_expanded.compose(identity_expanded, true)?;
        }
        // right context is in lower language, left in upper ( \\ )
        if repl_type == ReplaceType::REPL_LEFT {
            // compose them with [I:I | <a:b>]*
            left_context_expanded = first_context.clone();
            right_context_expanded = identity_expanded.clone();

            left_context_expanded.compose(identity_expanded, true)?;
            right_context_expanded.compose(&second_context, true)?;
        }
        if repl_type == ReplaceType::REPL_DOWN {
            // compose them with [I:I | <a:b>]*
            left_context_expanded = identity_expanded.clone();
            right_context_expanded = identity_expanded.clone();

            left_context_expanded.compose(&first_context, true)?;
            right_context_expanded.compose(&second_context, true)?;
        }

        left_context_expanded.transform_weights(zero_weight)?;
        right_context_expanded.transform_weights(zero_weight)?;
        left_context_expanded.optimize()?;
        right_context_expanded.optimize()?;

        first_context.disjunct(&left_context_expanded, true)?;
        first_context.optimize()?;

        second_context.disjunct(&right_context_expanded, true)?;
        second_context.optimize()?;

        // add boundary symbol before/after contexts
        let boundary_marker: String = ".#.".to_string();
        tok.add_multichar_symbol(&boundary_marker);
        let boundary = HfstTransducer::new_tokenized(&boundary_marker, &tok)?;

        identity_star.insert_to_alphabet_string(&boundary_marker)?;

        // to first_context
        let first_context_alphabet = first_context.get_alphabet()?;
        let mut has_boundary = false;
        for s in first_context_alphabet.iter() {
            if boundary_marker == *s {
                has_boundary = true;
            }
        }

        if !has_boundary {
            first_context.insert_to_alphabet_string(&boundary_marker)?;
            let mut tmp = boundary.clone();
            tmp.concatenate(&identity_star, true)?.optimize()?;
            tmp.concatenate(&first_context, true)?;
            first_context = tmp;
        }

        // to second_context
        let second_context_alphabet = second_context.get_alphabet()?;
        has_boundary = false;
        for s in second_context_alphabet.iter() {
            if boundary_marker == *s {
                has_boundary = true;
            }
        }

        if !has_boundary {
            second_context.insert_to_alphabet_string(&boundary_marker)?;
            second_context
                .concatenate(&identity_star, true)?
                .concatenate(&boundary, true)?
                .optimize()?;
        }

        // put mapping between (expanded) contexts
        let mut one_context_replace = first_context.clone();
        one_context_replace
            .concatenate(mapping_with_brackets_and_tmp_boundary, true)?
            .concatenate(&second_context, true)?;

        one_context_replace.transform_weights(zero_weight)?;
        union_context_replace.disjunct(&one_context_replace, true)?;
        union_context_replace.optimize()?;
    }
    Ok(union_context_replace)
}

/*
 * unconditional replace, in multiple contexts
 * first: (.* T<a:b>T .*) - [( .* L1 T<a:b>T R1 .*) u (.* L2 T<a:b>T R2 .*)...],
 *                         where .* = [I:I (+ {tmp_marker (T), <,>} in alphabet) | <a:b>]*
 * then: remove tmp_marker from transducer and alphabet, and do negation:
 *         .* - result from upper operations
 */

// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.bracketed-replace-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.bracketed-replace-fn]
// [spec:hfst:def:hfst-transducer.hfst.xerox-rules.bracketed-replace-fn]
// [spec:hfst:sem:hfst-transducer.hfst.xerox-rules.bracketed-replace-fn]
// [spec:hfst:def:hfst-transducer.hfst.hfst-transducer.hfst.xerox-rules.bracketed-replace-fn]
// [spec:hfst:sem:hfst-transducer.hfst.hfst-transducer.hfst.xerox-rules.bracketed-replace-fn]
pub fn bracketed_replace<B: AlgebraBackend>(
    rule: &Rule<B>,
    optional: bool,
) -> crate::error::Result<HfstTransducer<B>> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_UNKNOWN_SYMBOL_@");
    let left_marker: String = "@LM@".to_string();
    let right_marker: String = "@RM@".to_string();
    let tmp_marker: Symbol = Symbol::new_static("@TMPM@");
    let left_marker2: String = "@LM2@".to_string();
    let right_marker2: String = "@RM2@".to_string();
    let new_epsilon: String = "$Epsilon$".to_string();

    tok.add_multichar_symbol(&left_marker);
    tok.add_multichar_symbol(&right_marker);
    tok.add_multichar_symbol(&left_marker2);
    tok.add_multichar_symbol(&right_marker2);
    tok.add_multichar_symbol(&tmp_marker);
    tok.add_multichar_symbol(&new_epsilon);
    tok.add_multichar_symbol(".#.");

    //first, encode all flag diacritics
    let mut ruletmp = rule.clone();
    ruletmp.encode_flags()?;

    let mapping_pair_vector: HfstTransducerPairVector<B> = ruletmp.get_mapping();
    let context_vector: HfstTransducerPairVector<B> = ruletmp.get_context();
    let repl_type: ReplaceType = ruletmp.get_repl_type();

    // Identity (normal)
    let identity_pair = HfstTransducer::identity_pair();
    let mut identity = identity_pair.clone();
    identity.repeat_star()?.optimize()?;

    let epsilon: HfstTransducer<B> = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?;
    let mut mapping = HfstTransducer::new();
    for (i, pair) in mapping_pair_vector.iter().enumerate() {
        let mut one_mapping_pair = pair.0.clone();

        //markup rules are already cross product in the pair's first member
        //(second is empty), so the cross product should not be done for markup rules
        if pair.0.get_property("isMarkup") != "yes" {
            one_mapping_pair.cross_product(&pair.1, true)?;
        }

        // for removing .#. from the center
        let mut identity_without_boundary = identity.clone();
        identity_without_boundary.insert_to_alphabet_string(".#.")?;
        let mut remove_hash = identity_without_boundary.clone();
        let boundary = HfstTransducer::new_tokenized(".#.", &tok)?;
        remove_hash
            .concatenate(&boundary, true)?
            .concatenate(&identity_without_boundary, true)?
            .optimize()?;

        if i == 0 {
            // remove .#. from the center
            // center - (?* .#. ?*)
            one_mapping_pair.subtract(&remove_hash, false)?.optimize()?;
            one_mapping_pair.remove_from_alphabet_string(".#.")?;
            mapping = one_mapping_pair;
        } else {
            one_mapping_pair.subtract(&remove_hash, false)?.optimize()?;
            one_mapping_pair.remove_from_alphabet_string(".#.")?;
            mapping.disjunct(&one_mapping_pair, true)?.optimize()?;
        }
    }

    // In case of ? -> x replacement
    // If left side is empty, return identity transducer
    // If right side is empty, return identity transducer
    //    with alphabet from the left side
    let empty = HfstTransducer::new();
    if mapping.compare(&empty, true)? {
        mapping = identity.clone();
        if mapping_pair_vector[0].1.compare(&empty, true)? {
            let transducer_alphabet = mapping_pair_vector[0].0.get_alphabet()?;
            for s in transducer_alphabet.iter() {
                mapping.insert_to_alphabet_string(s)?;
            }
        }
    }

    mapping.insert_to_alphabet_string(&left_marker)?;
    mapping.insert_to_alphabet_string(&right_marker)?;
    mapping.insert_to_alphabet_string(&tmp_marker)?;

    let left_bracket = HfstTransducer::new_tokenized(&left_marker, &tok)?;
    let right_bracket = HfstTransducer::new_tokenized(&right_marker, &tok)?;
    let left_bracket2 = HfstTransducer::new_tokenized(&left_marker2, &tok)?;
    let right_bracket2 = HfstTransducer::new_tokenized(&right_marker2, &tok)?;
    let tmp_bracket = HfstTransducer::new_tokenized(&tmp_marker, &tok)?;

    // Surround mapping with brackets
    let mut tmp_mapping = left_bracket.clone();
    tmp_mapping
        .concatenate(&mapping, true)?
        .concatenate(&right_bracket, true)?
        .optimize()?;

    let mut mapping_with_brackets = tmp_mapping.clone();

    // Identity pair
    // for non-optional replacements
    if !optional {
        // non - optional
        // mapping = <a:b> u <2a:a>2

        let mut mapping_with_brackets2 = left_bracket2.clone();
        let mut left_mapping_union = mapping_pair_vector[0].0.clone();
        for pair in &mapping_pair_vector[1..] {
            left_mapping_union.disjunct(&pair.0, true)?.optimize()?;
        }
        // needed in case of ? -> x replacement
        left_mapping_union.insert_to_alphabet_string(&left_marker2)?;
        left_mapping_union.insert_to_alphabet_string(&right_marker2)?;
        left_mapping_union.insert_to_alphabet_string(&left_marker)?;
        left_mapping_union.insert_to_alphabet_string(&right_marker)?;
        left_mapping_union.insert_to_alphabet_string(&tmp_marker)?;

        mapping_with_brackets2
            .concatenate(&left_mapping_union, true)?
            .concatenate(&right_bracket2, true)?
            .optimize()?;

        // mapping_with_brackets...... expanded
        mapping_with_brackets.insert_to_alphabet_string(&left_marker2)?;
        mapping_with_brackets.insert_to_alphabet_string(&right_marker2)?;
        mapping_with_brackets
            .disjunct(&mapping_with_brackets2, true)?
            .optimize()?;
    }

    // Identity with bracketed mapping and marker symbols and TmpMarker in alphabet
    // [I:I | <a:b>]* (+ tmp_marker in alphabet)
    let mut identity_expanded = identity_pair.clone();

    identity_expanded.insert_to_alphabet_string(&left_marker)?;
    identity_expanded.insert_to_alphabet_string(&right_marker)?;
    identity_expanded.insert_to_alphabet_string(&tmp_marker)?;

    if !optional {
        identity_expanded.insert_to_alphabet_string(&left_marker2)?;
        identity_expanded.insert_to_alphabet_string(&right_marker2)?;
    }

    identity_expanded
        .disjunct(&mapping_with_brackets, true)?
        .optimize()?;
    identity_expanded.repeat_star()?.optimize()?;

    // when there aren't any contexts, result is identity_expanded
    if context_vector.len() == 1 {
        let epsilon = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?;
        if context_vector[0].0.compare(&epsilon, true)?
            && context_vector[0].1.compare(&epsilon, true)?
        {
            identity_expanded.remove_from_alphabet_string(&tmp_marker)?;
            return Ok(identity_expanded);
        }
    }

    // Surround mapping with tmp boudaries
    let mut mapping_with_brackets_and_tmp_boundary = tmp_bracket.clone();
    mapping_with_brackets_and_tmp_boundary
        .concatenate(&mapping_with_brackets, true)?
        .concatenate(&tmp_bracket, true)?
        .optimize()?;

    // .* |<a:b>| :*
    let mut bracketed_replace = identity_expanded.clone();
    bracketed_replace
        .concatenate(&mapping_with_brackets_and_tmp_boundary, true)?
        .concatenate(&identity_expanded, true)?
        .optimize()?;

    // Expand all contexts with mapping taking in consideration replace type
    // result is their union
    let union_context_replace = expand_contexts_with_mapping(
        &context_vector,
        &mapping_with_brackets_and_tmp_boundary,
        &identity_expanded,
        repl_type,
        optional,
    )?;

    // subtract all mappings in contexts from replace without contexts
    let mut replace_without_contexts = bracketed_replace.clone();
    replace_without_contexts
        .subtract(&union_context_replace, true)?
        .optimize()?;

    // remove tmpMaprker
    replace_without_contexts
        .substitute_pair_with_pair(
            &(tmp_marker.clone(), tmp_marker.clone()),
            &(
                Symbol::new_static("@_EPSILON_SYMBOL_@"),
                Symbol::new_static("@_EPSILON_SYMBOL_@"),
            ),
        )?
        .optimize()?;
    replace_without_contexts.remove_from_alphabet_string(&tmp_marker)?;
    replace_without_contexts.optimize()?;

    identity_expanded.remove_from_alphabet_string(&tmp_marker)?;

    // final negation
    let mut unconditional_tr = identity_expanded.clone();
    unconditional_tr
        .subtract(&replace_without_contexts, true)?
        .optimize()?;

    Ok(unconditional_tr)
}

// Return the string "@N@" where N is the string representation of i.
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.get-marker-string-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.get-marker-string-fn]
fn get_marker_string(i: u32) -> String {
    let oss: String = i.to_string();
    String::from("@") + &oss + &String::from("@")
}

// Return the number representation of N in string "@N@".
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.get-marker-number-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.get-marker-number-fn]
fn get_marker_number(str: &str) -> u32 {
    let number_str = str[1..str.len() - 1].to_string();
    let _ = number_str;
    // iss should be iss(number_str); i guess, but cannot be fixed, because some
    // HfstXeroxRules tests will fail...
    // unsigned int retval; iss >> retval;
    //return retval;
    100000
}

// Bracketed replace for parallel rules.
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.parallel-bracketed-replace-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.parallel-bracketed-replace-fn]
pub fn parallel_bracketed_replace<B: AlgebraBackend>(
    rule_vector: &[Rule<B>],
    optional: bool,
) -> crate::error::Result<HfstTransducer<B>> {
    // For each parallel rule, we need to concatenate a special marker symbol
    // to its output side. This is needed so that overlapping mappings with
    // different weights and contexts are kept separate. If we have N rules,
    // we need marker symbols "@1@", "@2@", ... , "@N@" ("@0@" is reserved
    // for epsilon symbol). At the end, we must substitute any marker symbols
    // with epsilons.

    let mut marker_symbols: StringSet = StringSet::new(); // "@1@", "@2@", ... , "@N@"
    let mut marker_substitutions: HfstSymbolSubstitutions = HfstSymbolSubstitutions::new();
    for i in 0..rule_vector.len() {
        let marker_string = get_marker_string((i + 1) as u32);
        marker_symbols.insert(Symbol::from(marker_string.clone()));
        marker_substitutions.insert(
            Symbol::from(marker_string.clone()),
            Symbol::new_static(internal_epsilon),
        );
    }

    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let left_marker: String = "@LM@".to_string();
    let right_marker: String = "@RM@".to_string();

    let left_marker2: String = "@LM2@".to_string();
    let right_marker2: String = "@RM2@".to_string();

    let tmp_marker: Symbol = Symbol::new_static("@TMPM@");
    tok.add_multichar_symbol(&left_marker);
    tok.add_multichar_symbol(&right_marker);
    tok.add_multichar_symbol(&left_marker2);
    tok.add_multichar_symbol(&right_marker2);
    tok.add_multichar_symbol(&tmp_marker);
    tok.add_multichar_symbol(".#.");

    let left_bracket = HfstTransducer::new_tokenized(&left_marker, &tok)?;
    let right_bracket = HfstTransducer::new_tokenized(&right_marker, &tok)?;
    let left_bracket2 = HfstTransducer::new_tokenized(&left_marker2, &tok)?;
    let right_bracket2 = HfstTransducer::new_tokenized(&right_marker2, &tok)?;
    let tmp_bracket = HfstTransducer::new_tokenized(&tmp_marker, &tok)?;

    // Identity pair (unknowns/identities must not be expanded to marker
    // symbols)
    let mut identity_pair = HfstTransducer::identity_pair();
    identity_pair.insert_to_alphabet_string_set(&marker_symbols)?;

    let mut identity = identity_pair.clone();
    // unknowns/identities must not be expanded to marker symbols
    identity.insert_to_alphabet_string_set(&marker_symbols)?;
    identity.repeat_star()?.optimize()?;

    let mut identity_expanded = identity_pair.clone();
    identity_expanded.insert_to_alphabet_string(&left_marker)?;
    identity_expanded.insert_to_alphabet_string(&right_marker)?;
    identity_expanded.insert_to_alphabet_string(&left_marker2)?;
    identity_expanded.insert_to_alphabet_string(&right_marker2)?;
    identity_expanded.insert_to_alphabet_string(&tmp_marker)?;
    identity_expanded.insert_to_alphabet_string_set(&marker_symbols)?;
    // will be expanded with mappings

    // for removing .#. from the center
    let mut identity_without_boundary = identity.clone();
    identity_without_boundary.insert_to_alphabet_string(".#.")?;
    // (must not be expanded to marker symbols)
    identity_without_boundary.insert_to_alphabet_string_set(&marker_symbols)?;
    let mut remove_hash = identity_without_boundary.clone();
    let boundary = HfstTransducer::new_tokenized(".#.", &tok)?;
    remove_hash
        .concatenate(&boundary, true)?
        .concatenate(&identity_without_boundary, true)?
        .optimize()?;

    let mut mapping_with_brackets_vector: HfstTransducerVector<B> = Vec::new();
    let mut no_contexts = true;

    // go through vector and do everything for each rule
    for (i, rule) in rule_vector.iter().enumerate() {
        let mut ruletmp = rule.clone();
        ruletmp.encode_flags()?;

        let mapping_pair_vector = ruletmp.get_mapping();
        let mut mapping = HfstTransducer::new();
        for (j, mapping_pair) in mapping_pair_vector.iter().enumerate() {
            // i+1 because @0@ is epsilon..
            let marker_string = get_marker_string((i + 1) as u32);
            let marker = HfstTransducer::new_symbol(&marker_string)?;
            let mut one_mapping_pair = mapping_pair.0.clone();
            // unknowns/identities must not be expanded to marker symbols
            one_mapping_pair.insert_to_alphabet_string_set(&marker_symbols)?;
            let mut mapping_output = mapping_pair.1.clone();
            mapping_output.insert_to_alphabet_string_set(&marker_symbols)?;
            one_mapping_pair.cross_product(mapping_output.concatenate(&marker, true)?, true)?;

            if j == 0 {
                // remove .#. from the center
                // center - (?* .#. ?*)
                one_mapping_pair.subtract(&remove_hash, false)?.optimize()?;
                one_mapping_pair.remove_from_alphabet_string(".#.")?;
                mapping = one_mapping_pair;
            } else {
                one_mapping_pair.subtract(&remove_hash, false)?.optimize()?;
                one_mapping_pair.remove_from_alphabet_string(".#.")?;
                mapping.disjunct(&one_mapping_pair, true)?.optimize()?;
            }
        }

        let context_vector = ruletmp.get_context();

        // when there aren't any contexts, result is identity_expanded
        if context_vector.len() == 1 {
            let epsilon = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?;
            if !(context_vector[0].0.compare(&epsilon, true)?
                && context_vector[0].1.compare(&epsilon, true)?)
            {
                no_contexts = false;
            }
        }

        //////////////////////////////////////////////////////////////////
        // In case of ? -> x replacement
        // If left side is empty, return identity transducer
        // If right side is empty, return identity transducer
        //    with alphabet from the left side
        let empty = HfstTransducer::new();

        if mapping.compare(&empty, true)? {
            mapping = identity.clone();
            if mapping_pair_vector[0].1.compare(&empty, true)? {
                let transducer_alphabet = mapping_pair_vector[0].0.get_alphabet()?;
                for s in transducer_alphabet.iter() {
                    mapping.insert_to_alphabet_string(s)?;
                }
            }
        }
        //////////////////////////////////////////////////////////////////

        mapping.insert_to_alphabet_string(&left_marker)?;
        mapping.insert_to_alphabet_string(&right_marker)?;
        mapping.insert_to_alphabet_string(&tmp_marker)?;

        // Surround mapping with brackets
        let mut mapping_with_brackets = left_bracket.clone();
        mapping_with_brackets
            .concatenate(&mapping, true)?
            .concatenate(&right_bracket, true)?
            .optimize()?;

        // non - optional
        // mapping = <a:b> u <2a:a>2
        if !optional {
            // needed in case of ? -> x replacement
            mapping.insert_to_alphabet_string(&left_marker2)?;
            mapping.insert_to_alphabet_string(&right_marker2)?;
            mapping_with_brackets.insert_to_alphabet_string(&left_marker2)?;
            mapping_with_brackets.insert_to_alphabet_string(&right_marker2)?;

            let mut mapping_project = mapping.clone();
            mapping_project.input_project()?.optimize()?;

            let mut mapping_with_brackets_non_optional = left_bracket2.clone();

            mapping_with_brackets_non_optional
                .concatenate(&mapping_project, true)?
                .concatenate(&right_bracket2, true)?
                .optimize()?;
            // mapping_with_brackets...... expanded
            mapping_with_brackets
                .disjunct(&mapping_with_brackets_non_optional, true)?
                .optimize()?;
        }

        identity_expanded
            .disjunct(&mapping_with_brackets, true)?
            .optimize()?;
        mapping_with_brackets_vector.push(mapping_with_brackets);
    }

    identity_expanded.repeat_star()?.optimize()?;

    // if none of the rules have contexts, return identity_expanded
    if no_contexts {
        identity_expanded.remove_from_alphabet_string(&tmp_marker)?;
        // substitute markers with epsilons
        identity_expanded.substitute_symbol_substitutions(&marker_substitutions)?;
        identity_expanded.remove_from_alphabet_string_set(&marker_symbols)?;
        return Ok(identity_expanded);
    }

    // if they have contexts, process them
    if rule_vector.len() != mapping_with_brackets_vector.len() {
        crate::bail!(TransducerTypeMismatch, "Vector sizes don't match");
    }

    let context_replace_map: std::collections::BTreeMap<
        String,
        crate::hfst_basic_transducer::HfstBasicTransducer,
    > = std::collections::BTreeMap::new();
    let _ = &context_replace_map;

    let mut union_context_replace = HfstTransducer::new();
    let mut bracketed_replace = HfstTransducer::new();
    for i in 0..rule_vector.len() {
        let mut ruletmp = rule_vector[i].clone();
        ruletmp.encode_flags()?;

        // Surround mapping with brackets with tmp boudaries
        let mut mapping_with_brackets_and_tmp_boundary = tmp_bracket.clone();
        mapping_with_brackets_and_tmp_boundary
            .concatenate(&mapping_with_brackets_vector[i], true)?
            .concatenate(&tmp_bracket, true)?
            .optimize()?;
        // .* |<a:b>| :*
        let mut bracketed_replace_tmp = identity_expanded.clone();
        bracketed_replace_tmp
            .concatenate(&mapping_with_brackets_and_tmp_boundary, true)?
            .concatenate(&identity_expanded, true)?
            .optimize()?;

        bracketed_replace_tmp.transform_weights(zero_weight)?;
        bracketed_replace
            .disjunct(&bracketed_replace_tmp, true)?
            .optimize()?;

        //Create context part
        // For each context that uses the output side (REPL_DOWN,
        // REPL_LEFT, REPL_RIGHT) we must freely allow all markers that can
        // be generated by other rules.
        let mut cont = ruletmp.get_context();

        if ruletmp.get_repl_type() != ReplaceType::REPL_UP {
            for cont_it in cont.iter_mut() {
                for sit in marker_symbols.iter() {
                    if get_marker_number(sit) != i as u32 {
                        let marker_pair = (sit.clone(), sit.clone());
                        // 'false' makes sure harmonization is not done
                        cont_it.0.insert_freely_pair(&marker_pair, false)?;
                        cont_it.1.insert_freely_pair(&marker_pair, false)?;
                    }
                }
            }
        }

        let mut union_context_replace_tmp = expand_contexts_with_mapping(
            &cont,
            &mapping_with_brackets_and_tmp_boundary,
            &identity_expanded,
            ruletmp.get_repl_type(),
            optional,
        )?;

        union_context_replace_tmp.transform_weights(zero_weight)?;

        union_context_replace
            .disjunct(&union_context_replace_tmp, true)?
            .optimize()?;
    }

    // subtract all mappings in contexts from replace without contexts
    let mut replace_without_contexts = bracketed_replace.clone();
    replace_without_contexts
        .subtract(&union_context_replace, true)?
        .optimize()?;

    // remove tmpMaprker
    replace_without_contexts
        .substitute_pair_with_pair(
            &(tmp_marker.clone(), tmp_marker.clone()),
            &(
                Symbol::new_static("@_EPSILON_SYMBOL_@"),
                Symbol::new_static("@_EPSILON_SYMBOL_@"),
            ),
        )?
        .optimize()?;
    replace_without_contexts.remove_from_alphabet_string(&tmp_marker)?;
    replace_without_contexts.optimize()?;

    identity_expanded.remove_from_alphabet_string(&tmp_marker)?;

    // final negation
    let mut unconditional_tr = identity_expanded.clone();
    unconditional_tr
        .subtract(&replace_without_contexts, true)?
        .optimize()?;

    // substitute markers with epsilons
    unconditional_tr.substitute_symbol_substitutions(&marker_substitutions)?;
    unconditional_tr.remove_from_alphabet_string_set(&marker_symbols)?;

    Ok(unconditional_tr)
}
