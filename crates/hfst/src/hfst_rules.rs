//! Port of 'libhfst/src/HfstRules.cc' — the 'hfst::rules' namespace: two-level
//! rules, replace / restriction / coercion rule-transducer constructors.
//!
//! ABSOLUTE 1:1 literal C++->Rust translation (HFST port, Wave 2). NOT idiomatic.
//! Mirrors structure/control-flow/eval-order; preserves bugs. The functions build
//! transducers via the facade type 'crate::hfst_transducer::HfstTransducer'.
//!
//! NOTE: the 'rules::ReplaceType' and 'rules::TwolType' enums below are declared in
//! 'libhfst/src/HfstTransducer.h' (namespace 'hfst::rules'), NOT in a separate
//! 'HfstRules.h' (no such header exists). They are owned here because the
//! 'HfstRules.cc' body is their primary consumer; the facade module re-uses them.

#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use std::collections::BTreeSet;

use crate::hfst_data_types::ImplementationType;
use crate::hfst_data_types::StringPair;
use crate::hfst_data_types::StringPairSet;
// HfstTransducer, plus the HfstTransducer-dependent aliases
// (HfstTransducerPair, HfstTransducerPairVector), live in the facade module that
// is ported concurrently. Bodies import them from 'crate::hfst_transducer'.
use crate::hfst_transducer::HfstTransducer;
use crate::hfst_transducer::HfstTransducerPair;
use crate::hfst_transducer::HfstTransducerPairVector;

/// \brief The replace direction / type used by the 'rules' namespace.
///
/// Distinct from 'crate::hfst_xerox_rules::ReplaceType': this one additionally
/// carries 'REPL_DOWN_KARTTUNEN'.
// [spec:hfst:def:hfst-transducer.hfst.rules.replace-type]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum ReplaceType {
    REPL_UP,
    REPL_DOWN,
    REPL_RIGHT,
    REPL_LEFT,
    REPL_DOWN_KARTTUNEN,
}

// [spec:hfst:def:hfst-transducer.hfst.rules.twol-type]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum TwolType {
    twol_right,
    twol_left,
    twol_both,
}

// ===== flattened bodies (free fns share scope) =====
use crate::hfst_exception_defs::ContextTransducersAreNotAutomataException;
use crate::hfst_exception_defs::EmptySetOfContextsException;
use crate::hfst_exception_defs::HfstFatalException;
use crate::hfst_exception_defs::TransducerTypeMismatchException;
use crate::hfst_symbol_defs::internal_epsilon;
// Port of 'libhfst/src/HfstRules.cc', lines 1-790 (the 'hfst::rules' namespace
// body, excluding the MAIN_TEST block). 1:1, bug-for-bug.

// [spec:hfst:def:hfst-rules.hfst.rules.replace-fn]
// [spec:hfst:sem:hfst-rules.hfst.rules.replace-fn]
// [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-replace-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-replace-fn]
pub fn replace(
    t: &mut HfstTransducer,
    repl_type: ReplaceType,
    optional: bool,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    let type_ = t.get_type();

    let mut t_proj = t.clone();
    if repl_type == ReplaceType::REPL_UP {
        t_proj.input_project();
    } else if repl_type == ReplaceType::REPL_DOWN {
        t_proj.output_project();
    } else {
        //fprintf(stderr, "ERROR: replace: Impossible replace type\n");
        //exit(1);
        crate::HFST_THROW_MESSAGE!(HfstFatalException, "impossible replace type");
    }

    let pi_star = HfstTransducer::from_string_pair_set(&*alphabet, type_, true);

    // tc = ( .* t_proj .* )
    let mut tc = pi_star.clone();
    tc.concatenate(&t_proj, true);
    tc.concatenate(&pi_star, true);

    // tc_neg = ! ( .* t_proj .* )
    let mut tc_neg = pi_star.clone();
    tc_neg.subtract(&tc, true);

    // retval = ( tc_neg t )* tc_neg
    let mut retval = tc_neg.clone();
    retval.concatenate(&*t, true);
    retval.repeat_star();
    retval.concatenate(&tc_neg, true);

    if optional {
        retval.disjunct(&pi_star, true);
    }

    return retval;
}

// [spec:hfst:def:hfst-rules.hfst.rules.replace-transducer-fn]
// [spec:hfst:sem:hfst-rules.hfst.rules.replace-transducer-fn]
// [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-replace-transducer-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-replace-transducer-fn]
pub fn replace_transducer(
    t: &mut HfstTransducer,
    lm: String,
    rm: String,
    repl_type: ReplaceType,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    t.optimize();

    let type_ = t.get_type();

    // tm = ( L (L >> (R >> t)) R )
    let mut tc = t.clone();
    tc.insert_freely_pair(&(rm.clone(), rm.clone()), true);
    tc.insert_freely_pair(&(lm.clone(), lm.clone()), true);
    let mut tm = HfstTransducer::from_symbol(&lm, type_);
    let rmtr = HfstTransducer::from_symbol(&rm, type_);
    tm.concatenate(&tc, true);
    tm.concatenate(&rmtr, true);

    tm.optimize();

    let mut retval = replace(&mut tm, repl_type, false, alphabet);

    retval.optimize();
    return retval;
}

// [spec:hfst:def:hfst-rules.hfst.rules.replace-context-fn]
// [spec:hfst:sem:hfst-rules.hfst.rules.replace-context-fn]
// [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-replace-context-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-replace-context-fn]
pub fn replace_context(
    t: &mut HfstTransducer,
    m1: String,
    m2: String,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    // ct = .* ( m1 >> ( m2 >> t ))  ||  !(.* m1)

    // m1 >> ( m2 >> t )
    let mut t_copy = t.clone();
    t_copy.insert_freely_pair(&(m1.clone(), m1.clone()), true);
    t_copy.insert_freely_pair(&(m2.clone(), m2.clone()), true);

    let pi_star = HfstTransducer::from_string_pair_set(&*alphabet, t.get_type(), true);

    // arg1 = .* ( m1 >> ( m2 >> t ))
    let mut arg1 = pi_star.clone();

    arg1.concatenate(&t_copy, true);

    // arg2 = !(.* m1)
    let m1_tr = HfstTransducer::from_symbol(&m1, t.get_type());
    let mut tmp = pi_star.clone();
    tmp.concatenate(&m1_tr, true);
    let mut arg2 = pi_star.clone();
    arg2.subtract(&tmp, true);

    // ct = .* ( m1 >> ( m2 >> t ))  ||  !(.* m1)
    let ct = arg1.compose(&arg2, true).clone();

    // mt = m2* m1 .*
    let mut mt = HfstTransducer::from_symbol(&m2, t.get_type());
    mt.repeat_star();
    mt.concatenate(&m1_tr, true);
    mt.concatenate(&pi_star, true);

    // !( (!ct mt) | (ct !mt) )

    // ct !mt
    let mut tmp2 = pi_star.clone();
    tmp2.subtract(&mt, true);
    let mut ct_neg_mt = ct.clone();
    ct_neg_mt.concatenate(&tmp2, true);

    // !ct mt
    let mut neg_ct_mt = pi_star.clone();
    neg_ct_mt.subtract(&ct, true);
    neg_ct_mt.concatenate(&mt, true);

    // disjunction
    let disj = neg_ct_mt.disjunct(&ct_neg_mt, true).clone();

    // negation
    let mut retval = pi_star.clone();
    retval.subtract(&disj, true);

    retval.optimize();
    return retval;
}

/* identical to  ![ .* l [a:. & !a:b] r .* ]  */
// [spec:hfst:def:hfst-rules.hfst.rules.two-level-if-fn]
// [spec:hfst:sem:hfst-rules.hfst.rules.two-level-if-fn]
// [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-two-level-if-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-two-level-if-fn]
pub fn two_level_if(
    context: &mut HfstTransducerPair,
    mappings: &mut StringPairSet,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    if context.0.get_type() != context.1.get_type() {
        crate::HFST_THROW_MESSAGE!(TransducerTypeMismatchException, "rules::two_level_if");
    }
    let type_ = context.0.get_type();

    assert!(context.1.get_type() != ImplementationType::ERROR_TYPE);
    assert!(context.1.get_type() != ImplementationType::ERROR_TYPE);

    // calculate [ a:. ]
    let mut input_to_any: StringPairSet = StringPairSet::new();
    for it in mappings.iter() {
        for alpha_it in alphabet.iter() {
            if alpha_it.0 == it.0 {
                input_to_any.insert((alpha_it.0.clone(), alpha_it.1.clone()));
            }
        }
    }

    // center == [ a:. ]
    let mut center = HfstTransducer::from_string_pair_set(&input_to_any, type_, false);

    // calculate [ .* - a:b ]
    let mut neg_mappings = HfstTransducer::from_string_pair_set(&*alphabet, type_, true);
    //neg_mappings.repeat_star();

    let mappings_tr = HfstTransducer::from_string_pair_set(&*mappings, type_, false);
    neg_mappings.subtract(&mappings_tr, true);

    // center == [ a:. & !a:b ]
    center.intersect(&neg_mappings, true);

    // left context == [ .* l ]
    let mut left_context = HfstTransducer::from_string_pair_set(&*alphabet, type_, true);
    left_context.concatenate(&context.0, true);

    // right_context == [ r .* ]
    let mut right_context = context.1.clone();
    let universal = HfstTransducer::from_string_pair_set(&*alphabet, type_, true);
    right_context.concatenate(&universal, true);

    let inside = left_context
        .concatenate(&center, true)
        .concatenate(&right_context, true)
        .clone();

    let mut universal = universal;
    let retval = universal.subtract(&inside, true).clone();
    return retval;
}

// equivalent to !(!(.* l) a:b .* | .* a:b !(r .*))
// [spec:hfst:def:hfst-rules.hfst.rules.two-level-only-if-fn]
// [spec:hfst:sem:hfst-rules.hfst.rules.two-level-only-if-fn]
// [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-two-level-only-if-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-two-level-only-if-fn]
pub fn two_level_only_if(
    context: &mut HfstTransducerPair,
    mappings: &mut StringPairSet,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    if context.0.get_type() != context.1.get_type() {
        crate::HFST_THROW_MESSAGE!(TransducerTypeMismatchException, "rules::two_level_only_if");
    }
    let type_ = context.0.get_type();

    assert!(context.1.get_type() != ImplementationType::ERROR_TYPE);
    assert!(context.1.get_type() != ImplementationType::ERROR_TYPE);

    // center = a:b
    let center = HfstTransducer::from_string_pair_set(&*mappings, type_, false);

    // left_neg = !(.* l)
    let mut left = HfstTransducer::from_string_pair_set(&*alphabet, type_, true);
    left.concatenate(&context.0, true);
    let mut left_neg = HfstTransducer::from_string_pair_set(&*alphabet, type_, true);
    left_neg.subtract(&left, true);

    // right_neg = !(r .*)
    let universal = HfstTransducer::from_string_pair_set(&*alphabet, type_, true);
    let mut right = context.1.clone();
    right.concatenate(&universal, true);
    let mut right_neg = HfstTransducer::from_string_pair_set(&*alphabet, type_, true);
    right_neg.subtract(&right, true);

    // left_neg + center + universal  |  universal + center + right_neg
    let mut rule = left_neg.clone();
    rule.concatenate(&center, true);
    rule.concatenate(&universal, true);
    let mut rule_right = universal.clone();
    rule_right.concatenate(&center, true);
    rule_right.concatenate(&right_neg, true);
    rule.disjunct(&rule_right, true);

    let mut rule_neg = HfstTransducer::from_string_pair_set(&*alphabet, type_, true);
    rule_neg.subtract(&rule, true);

    return rule_neg;
}

// [spec:hfst:def:hfst-rules.hfst.rules.two-level-if-and-only-if-fn]
// [spec:hfst:sem:hfst-rules.hfst.rules.two-level-if-and-only-if-fn]
// [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-two-level-if-and-only-if-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-two-level-if-and-only-if-fn]
pub fn two_level_if_and_only_if(
    context: &mut HfstTransducerPair,
    mappings: &mut StringPairSet,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    let mut if_rule = two_level_if(context, mappings, alphabet);
    let only_if_rule = two_level_only_if(context, mappings, alphabet);
    return if_rule.intersect(&only_if_rule, true).clone();
}

// [spec:hfst:def:hfst-rules.hfst.rules.replace-in-context-fn]
// [spec:hfst:sem:hfst-rules.hfst.rules.replace-in-context-fn]
// [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-replace-in-context-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-replace-in-context-fn]
pub fn replace_in_context(
    context: &mut HfstTransducerPair,
    repl_type: ReplaceType,
    t: &mut HfstTransducer,
    optional: bool,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    // test that all transducers have the same type
    if context.0.get_type() != context.1.get_type() || context.0.get_type() != t.get_type() {
        crate::HFST_THROW_MESSAGE!(TransducerTypeMismatchException, "rules::replace_in_context");
    }
    let type_ = t.get_type();

    // test that both context transducers are automata
    // this could be done more efficiently...
    let mut t1_proj = context.0.clone();
    t1_proj.input_project();
    let mut t2_proj = context.1.clone();
    t2_proj.input_project();

    if !t1_proj.compare(&context.0, true) || !t2_proj.compare(&context.1, true) {
        crate::HFST_THROW!(ContextTransducersAreNotAutomataException);
    }

    let leftm: String = "@_LEFT_MARKER_@".to_string();
    let rightm: String = "@_RIGHT_MARKER_@".to_string();
    let epsilon: String = internal_epsilon.to_string();

    // HfstTransducer pi(alphabet, type);

    // Create the insert boundary transducer (.|<>:<L>|<>:<R>)*
    let mut pi1 = alphabet.clone();
    pi1.insert((internal_epsilon.to_string(), leftm.clone()));
    pi1.insert((internal_epsilon.to_string(), rightm.clone()));
    let ibt = HfstTransducer::from_string_pair_set(&pi1, type_, true);

    // Create the remove boundary transducer (.|<L>:<>|<R>:<>)*
    let mut pi2 = alphabet.clone();
    pi2.insert((leftm.clone(), internal_epsilon.to_string()));
    pi2.insert((rightm.clone(), internal_epsilon.to_string()));
    let rbt = HfstTransducer::from_string_pair_set(&pi2, type_, true);

    // Add the markers to the alphabet
    alphabet.insert((leftm.clone(), leftm.clone()));
    alphabet.insert((rightm.clone(), rightm.clone()));

    let pi_star = HfstTransducer::from_string_pair_set(&*alphabet, type_, true);

    // Create the constrain boundary transducer !(.*<L><R>.*)
    let leftm_to_leftm = HfstTransducer::from_isymbol_osymbol(&leftm, &leftm, type_);
    let rightm_to_rightm = HfstTransducer::from_isymbol_osymbol(&rightm, &rightm, type_);
    let mut tmp = pi_star.clone();
    tmp.concatenate(&leftm_to_leftm, true);
    tmp.concatenate(&rightm_to_rightm, true);
    tmp.concatenate(&pi_star, true);
    let mut cbt = pi_star.clone();
    cbt.subtract(&tmp, true);
    cbt.optimize();

    // left context transducer .* (<R> >> (<L> >> LEFT_CONTEXT)) || !(.*<L>)
    let mut lct = replace_context(&mut context.0, leftm.clone(), rightm.clone(), alphabet);

    lct.optimize();

    // right context transducer:
    // reversion( (<R> >> (<L> >> reversion(RIGHT_CONTEXT))) .* || !(<R>.*) )
    let mut right_rev = context.1.clone();

    right_rev.reverse();
    right_rev.optimize();

    let mut rct = replace_context(&mut right_rev, rightm.clone(), leftm.clone(), alphabet);
    rct.reverse();
    rct.optimize();

    // unconditional replace transducer
    let mut rt = HfstTransducer::from_type(type_);
    if repl_type == ReplaceType::REPL_UP
        || repl_type == ReplaceType::REPL_RIGHT
        || repl_type == ReplaceType::REPL_LEFT
        || repl_type == ReplaceType::REPL_DOWN_KARTTUNEN
    {
        rt = replace_transducer(
            t,
            leftm.clone(),
            rightm.clone(),
            ReplaceType::REPL_UP,
            alphabet,
        );
    } else {
        rt = replace_transducer(
            t,
            leftm.clone(),
            rightm.clone(),
            ReplaceType::REPL_DOWN,
            alphabet,
        );
    }
    rt.optimize();

    // build the conditional replacement transducer
    let mut result = ibt.clone();

    result.compose(&cbt, true);
    result.optimize(); // added

    if repl_type == ReplaceType::REPL_UP || repl_type == ReplaceType::REPL_RIGHT {
        result.compose(&rct, true);
    }

    if repl_type == ReplaceType::REPL_UP || repl_type == ReplaceType::REPL_LEFT {
        result.compose(&lct, true);
    }

    result.optimize(); // ADDED

    result.compose(&rt, true);

    if repl_type == ReplaceType::REPL_DOWN
        || repl_type == ReplaceType::REPL_RIGHT
        || repl_type == ReplaceType::REPL_DOWN_KARTTUNEN
    {
        result.compose(&lct, true);
    }

    if repl_type == ReplaceType::REPL_DOWN
        || repl_type == ReplaceType::REPL_LEFT
        || repl_type == ReplaceType::REPL_DOWN_KARTTUNEN
    {
        result.compose(&rct, true);
    }

    result.optimize(); // ADDED

    result.compose(&rbt, true);

    // Remove the markers from the alphabet
    alphabet.remove(&(leftm.clone(), leftm.clone()));
    alphabet.remove(&(rightm.clone(), rightm.clone()));

    if optional {
        let pi_star_ = HfstTransducer::from_string_pair_set(&*alphabet, type_, true);
        result.disjunct(&pi_star_, true);
    }

    result.optimize();
    return result;
}

// [spec:hfst:def:hfst-rules.hfst.rules.replace-up-fn]
// [spec:hfst:sem:hfst-rules.hfst.rules.replace-up-fn]
// [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-replace-up-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-replace-up-fn]
pub fn replace_up(
    context: &mut HfstTransducerPair,
    mapping: &mut HfstTransducer,
    optional: bool,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    return replace_in_context(context, ReplaceType::REPL_UP, mapping, optional, alphabet);
}

// [spec:hfst:def:hfst-rules.hfst.rules.replace-down-fn]
// [spec:hfst:sem:hfst-rules.hfst.rules.replace-down-fn]
// [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-replace-down-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-replace-down-fn]
pub fn replace_down(
    context: &mut HfstTransducerPair,
    mapping: &mut HfstTransducer,
    optional: bool,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    return replace_in_context(context, ReplaceType::REPL_DOWN, mapping, optional, alphabet);
}

// [spec:hfst:def:hfst-rules.hfst.rules.replace-down-karttunen-fn]
// [spec:hfst:sem:hfst-rules.hfst.rules.replace-down-karttunen-fn]
// [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-replace-down-karttunen-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-replace-down-karttunen-fn]
pub fn replace_down_karttunen(
    context: &mut HfstTransducerPair,
    mapping: &mut HfstTransducer,
    optional: bool,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    return replace_in_context(
        context,
        ReplaceType::REPL_DOWN_KARTTUNEN,
        mapping,
        optional,
        alphabet,
    );
}

// [spec:hfst:def:hfst-rules.hfst.rules.replace-right-fn]
// [spec:hfst:sem:hfst-rules.hfst.rules.replace-right-fn]
// [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-replace-right-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-replace-right-fn]
pub fn replace_right(
    context: &mut HfstTransducerPair,
    mapping: &mut HfstTransducer,
    optional: bool,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    return replace_in_context(
        context,
        ReplaceType::REPL_RIGHT,
        mapping,
        optional,
        alphabet,
    );
}

// [spec:hfst:def:hfst-rules.hfst.rules.replace-left-fn]
// [spec:hfst:sem:hfst-rules.hfst.rules.replace-left-fn]
// [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-replace-left-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-replace-left-fn]
pub fn replace_left(
    context: &mut HfstTransducerPair,
    mapping: &mut HfstTransducer,
    optional: bool,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    return replace_in_context(context, ReplaceType::REPL_LEFT, mapping, optional, alphabet);
}

pub fn replace_up_mapping(
    mapping: &mut HfstTransducer,
    optional: bool,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    return replace(mapping, ReplaceType::REPL_UP, optional, alphabet);
}

pub fn replace_down_mapping(
    mapping: &mut HfstTransducer,
    optional: bool,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    return replace(mapping, ReplaceType::REPL_DOWN, optional, alphabet);
}

// Left arrow replace up without context
// [spec:hfst:def:hfst-rules.hfst.rules.left-replace-up-fn]
// [spec:hfst:sem:hfst-rules.hfst.rules.left-replace-up-fn]
// [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-left-replace-up-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-left-replace-up-fn]
pub fn left_replace_up_mapping(
    mapping: &mut HfstTransducer,
    optional: bool,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    if optional {
        return replace_up_mapping(mapping, true, alphabet).invert().clone();
    } else {
        return replace_up_mapping(mapping, false, alphabet)
            .invert()
            .clone();
    }
}

// Left arrow replace up
pub fn left_replace_up(
    context: &mut HfstTransducerPair,
    mapping: &mut HfstTransducer,
    optional: bool,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    if optional {
        return replace_up(context, mapping, true, alphabet)
            .invert()
            .clone();
    } else {
        return replace_up(context, mapping, false, alphabet)
            .invert()
            .clone();
    }
}

// Left arrow replace down (XFST's version)
// [spec:hfst:def:hfst-rules.hfst.rules.left-replace-down-karttunen-fn]
// [spec:hfst:sem:hfst-rules.hfst.rules.left-replace-down-karttunen-fn]
// [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-left-replace-down-karttunen-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-left-replace-down-karttunen-fn]
pub fn left_replace_down_karttunen(
    context: &mut HfstTransducerPair,
    mapping: &mut HfstTransducer,
    optional: bool,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    if optional {
        return replace_down_karttunen(context, mapping, true, alphabet)
            .invert()
            .clone();
    } else {
        return replace_down_karttunen(context, mapping, false, alphabet)
            .invert()
            .clone();
    }
}

// Left arrow replace down (SFST's version)
// [spec:hfst:def:hfst-rules.hfst.rules.left-replace-down-fn]
// [spec:hfst:sem:hfst-rules.hfst.rules.left-replace-down-fn]
// [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-left-replace-down-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-left-replace-down-fn]
pub fn left_replace_down(
    context: &mut HfstTransducerPair,
    mapping: &mut HfstTransducer,
    optional: bool,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    if optional {
        return replace_down(context, mapping, true, alphabet)
            .invert()
            .clone();
    } else {
        return replace_down(context, mapping, false, alphabet)
            .invert()
            .clone();
    }
}

// Left arrow replace left
// [spec:hfst:def:hfst-rules.hfst.rules.left-replace-left-fn]
// [spec:hfst:sem:hfst-rules.hfst.rules.left-replace-left-fn]
// [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-left-replace-left-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-left-replace-left-fn]
pub fn left_replace_left(
    context: &mut HfstTransducerPair,
    mapping: &mut HfstTransducer,
    optional: bool,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    if optional {
        return replace_left(context, mapping, true, alphabet)
            .invert()
            .clone();
    } else {
        return replace_left(context, mapping, false, alphabet)
            .invert()
            .clone();
    }
}

// Left arrow replace right
// [spec:hfst:def:hfst-rules.hfst.rules.left-replace-right-fn]
// [spec:hfst:sem:hfst-rules.hfst.rules.left-replace-right-fn]
// [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-left-replace-right-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-left-replace-right-fn]
pub fn left_replace_right(
    context: &mut HfstTransducerPair,
    mapping: &mut HfstTransducer,
    optional: bool,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    if optional {
        return replace_right(context, mapping, true, alphabet)
            .invert()
            .clone();
    } else {
        return replace_right(context, mapping, false, alphabet)
            .invert()
            .clone();
    }
}

// [spec:hfst:def:hfst-rules.hfst.rules.restriction-fn]
// [spec:hfst:sem:hfst-rules.hfst.rules.restriction-fn]
// [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-restriction-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-restriction-fn]
pub fn restriction(
    contexts: &mut HfstTransducerPairVector,
    mapping: &mut HfstTransducer,
    alphabet: &mut StringPairSet,
    twol_type: TwolType,
    direction: i32,
) -> HfstTransducer {
    // Make sure that contexts contains at least one transducer pair and that
    // all transducers in the set have the same type.
    let mut type_ = ImplementationType::ERROR_TYPE;
    let mut type_defined = false;
    for it in contexts.iter() {
        if !type_defined {
            type_ = it.0.get_type();
            type_defined = true;
        } else {
            if type_ != it.0.get_type() {
                crate::HFST_THROW_MESSAGE!(TransducerTypeMismatchException, "rules::restriction");
            }
        }
        if type_ != it.1.get_type() {
            crate::HFST_THROW_MESSAGE!(TransducerTypeMismatchException, "rules::restriction");
        }
    }
    if !type_defined {
        crate::HFST_THROW_MESSAGE!(EmptySetOfContextsException, "rules::restriction");
    }

    let marker: String = "@_MARKER_@".to_string();
    let mt = HfstTransducer::from_symbol(&marker, type_);
    let pi_star = HfstTransducer::from_string_pair_set(&*alphabet, type_, true);

    // center transducer
    let mut l1 = HfstTransducer::from_symbol(internal_epsilon, type_);
    l1.concatenate(&pi_star, true);
    l1.concatenate(&mt, true);
    l1.concatenate(&*mapping, true);
    l1.concatenate(&mt, true);
    l1.concatenate(&pi_star, true);

    let mut tmp = HfstTransducer::from_type(type_);
    if direction == 0 {
        tmp = pi_star.clone();
    } else if direction == 1 {
        tmp = mapping.input_project().compose(&pi_star, true).clone();
    } else {
        tmp = pi_star.clone();
        tmp.compose(mapping.output_project(), true);
    }

    // context transducer
    // pi_star + left[i] + mt + tmp + mt + + right[i] + pi_star
    let mut l2 = HfstTransducer::from_type(type_);
    for it in contexts.iter() {
        let mut ct = HfstTransducer::from_symbol(internal_epsilon, type_);
        ct.concatenate(&pi_star, true);
        ct.concatenate(&it.0, true);
        ct.concatenate(&mt, true);
        ct.concatenate(&tmp, true);
        ct.concatenate(&mt, true);
        ct.concatenate(&it.1, true);
        ct.concatenate(&pi_star, true);
        l2.disjunct(&ct, true);
    }

    let result = HfstTransducer::from_type(type_);

    if twol_type == TwolType::twol_right {
        // TheAlphabet - ( l1 - l2 ).substitute(marker,epsilon, true, true)
        let mut retval = HfstTransducer::from_string_pair_set(&*alphabet, type_, true);
        let mut tmp1 = l1.clone();
        tmp1.subtract(&l2, true);
        tmp1.substitute(&marker, internal_epsilon, true, true);
        retval.subtract(&tmp1, true);
        return retval;
    } else if twol_type == TwolType::twol_left {
        // TheAlphabet - ( l2 - l1 ).substitute(marker,epsilon, true, true)
        let mut retval = HfstTransducer::from_string_pair_set(&*alphabet, type_, true);
        let mut tmp1 = l2.clone();
        tmp1.subtract(&l1, true);
        tmp1.substitute(&marker, internal_epsilon, true, true);
        retval.subtract(&tmp1, true);
        return retval;
    } else if twol_type == TwolType::twol_both {
        // TheAlphabet - ( l1 - l2 ).substitute(marker,epsilon, true, true)
        // TheAlphabet - ( l2 - l1 ).substitute(marker,epsilon, true, true)
        // intersect
        let mut retval1 = HfstTransducer::from_string_pair_set(&*alphabet, type_, true);
        let mut tmp1 = l1.clone();
        tmp1.subtract(&l2, true);
        tmp1.substitute(&marker, internal_epsilon, true, true);
        retval1.subtract(&tmp1, true);

        let mut retval2 = HfstTransducer::from_string_pair_set(&*alphabet, type_, true);
        let mut tmp2 = l2.clone();
        tmp2.subtract(&l1, true);
        tmp2.substitute(&marker, internal_epsilon, true, true);
        retval2.subtract(&tmp2, true);

        return retval1.intersect(&retval2, true).clone();
    } else {
        assert!(false);
        return HfstTransducer::from_type(type_); // make compiler happy
    }
}

pub fn restriction_default(
    contexts: &mut HfstTransducerPairVector,
    mapping: &mut HfstTransducer,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    return restriction(contexts, mapping, alphabet, TwolType::twol_right, 0);
}

// [spec:hfst:def:hfst-rules.hfst.rules.coercion-fn]
// [spec:hfst:sem:hfst-rules.hfst.rules.coercion-fn]
// [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-coercion-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-coercion-fn]
pub fn coercion(
    contexts: &mut HfstTransducerPairVector,
    mapping: &mut HfstTransducer,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    return restriction(contexts, mapping, alphabet, TwolType::twol_left, 0);
}

// [spec:hfst:def:hfst-rules.hfst.rules.restriction-and-coercion-fn]
// [spec:hfst:sem:hfst-rules.hfst.rules.restriction-and-coercion-fn]
// [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-restriction-and-coercion-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-restriction-and-coercion-fn]
pub fn restriction_and_coercion(
    contexts: &mut HfstTransducerPairVector,
    mapping: &mut HfstTransducer,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    return restriction(contexts, mapping, alphabet, TwolType::twol_both, 0);
}

// [spec:hfst:def:hfst-rules.hfst.rules.surface-restriction-fn]
// [spec:hfst:sem:hfst-rules.hfst.rules.surface-restriction-fn]
// [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-surface-restriction-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-surface-restriction-fn]
pub fn surface_restriction(
    contexts: &mut HfstTransducerPairVector,
    mapping: &mut HfstTransducer,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    return restriction(contexts, mapping, alphabet, TwolType::twol_right, 1);
}

// [spec:hfst:def:hfst-rules.hfst.rules.surface-coercion-fn]
// [spec:hfst:sem:hfst-rules.hfst.rules.surface-coercion-fn]
// [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-surface-coercion-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-surface-coercion-fn]
pub fn surface_coercion(
    contexts: &mut HfstTransducerPairVector,
    mapping: &mut HfstTransducer,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    return restriction(contexts, mapping, alphabet, TwolType::twol_left, 1);
}

// [spec:hfst:def:hfst-rules.hfst.rules.surface-restriction-and-coercion-fn]
// [spec:hfst:sem:hfst-rules.hfst.rules.surface-restriction-and-coercion-fn]
// [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-surface-restriction-and-coercion-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-surface-restriction-and-coercion-fn]
pub fn surface_restriction_and_coercion(
    contexts: &mut HfstTransducerPairVector,
    mapping: &mut HfstTransducer,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    return restriction(contexts, mapping, alphabet, TwolType::twol_both, 1);
}

// [spec:hfst:def:hfst-rules.hfst.rules.deep-restriction-fn]
// [spec:hfst:sem:hfst-rules.hfst.rules.deep-restriction-fn]
// [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-deep-restriction-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-deep-restriction-fn]
pub fn deep_restriction(
    contexts: &mut HfstTransducerPairVector,
    mapping: &mut HfstTransducer,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    return restriction(contexts, mapping, alphabet, TwolType::twol_right, -1);
}

// [spec:hfst:def:hfst-rules.hfst.rules.deep-coercion-fn]
// [spec:hfst:sem:hfst-rules.hfst.rules.deep-coercion-fn]
// [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-deep-coercion-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-deep-coercion-fn]
pub fn deep_coercion(
    contexts: &mut HfstTransducerPairVector,
    mapping: &mut HfstTransducer,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    return restriction(contexts, mapping, alphabet, TwolType::twol_left, -1);
}

// [spec:hfst:def:hfst-rules.hfst.rules.deep-restriction-and-coercion-fn]
// [spec:hfst:sem:hfst-rules.hfst.rules.deep-restriction-and-coercion-fn]
// [spec:hfst:def:hfst-transducer.hfst.rules.hfst-transducer-deep-restriction-and-coercion-fn]
// [spec:hfst:sem:hfst-transducer.hfst.rules.hfst-transducer-deep-restriction-and-coercion-fn]
pub fn deep_restriction_and_coercion(
    contexts: &mut HfstTransducerPairVector,
    mapping: &mut HfstTransducer,
    alphabet: &mut StringPairSet,
) -> HfstTransducer {
    return restriction(contexts, mapping, alphabet, TwolType::twol_both, -1);
}
// (unwrapped mod rules_b {)

// [spec:hfst:def:hfst-rules.right-arrow-test1-fn]
// [spec:hfst:sem:hfst-rules.right-arrow-test1-fn]
// [spec:hfst:def:hfst-rules.right-arrow-test2-fn]
// [spec:hfst:sem:hfst-rules.right-arrow-test2-fn]
// [spec:hfst:def:hfst-rules.right-arrow-test3-fn]
// [spec:hfst:sem:hfst-rules.right-arrow-test3-fn]
// [spec:hfst:def:hfst-rules.right-arrow-test4-fn]
// [spec:hfst:sem:hfst-rules.right-arrow-test4-fn]
// [spec:hfst:def:hfst-rules.left-arrow-test1-fn]
// [spec:hfst:sem:hfst-rules.left-arrow-test1-fn]
// [spec:hfst:def:hfst-rules.left-arrow-test2-fn]
// [spec:hfst:sem:hfst-rules.left-arrow-test2-fn]
// [spec:hfst:def:hfst-rules.left-arrow-test3-fn]
// [spec:hfst:sem:hfst-rules.left-arrow-test3-fn]
// [spec:hfst:def:hfst-rules.left-arrow-test4-fn]
// [spec:hfst:sem:hfst-rules.left-arrow-test4-fn]
// [spec:hfst:def:hfst-rules.main-fn]
// [spec:hfst:sem:hfst-rules.main-fn]
//
// The entire assigned source range [790, 1580] of libhfst/src/HfstRules.cc
// lies inside the '#else // MAIN_TEST was defined' block (lines 780-1580,
// i.e. the '#ifdef MAIN_TEST' test section). Per the porting instructions,
// MAIN_TEST sections are skipped. All definitions in this range
// (right_arrow_test1..4, left_arrow_test1..4, and main) are test code, so
// there is no production code to port in this area.
