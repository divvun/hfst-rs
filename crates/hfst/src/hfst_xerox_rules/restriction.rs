//! Restriction ('=>') and the ordering operators 'before' and 'after'.

use super::*;

//---------------------------------
//    RESTRICTION FUNCTIONS
//---------------------------------

/*
  define U [ ? - %<D%> ] ;

  define CENTER [ x y | x x y y ];

  define L1 [ a ] ;
  define R1 [ b ] ;

  define L2 [ x ] ;
  define R2 [ y ] ;

  define RES1 [ U* L1 %<D%> U* %<D%> R1 U* ] ;
  define RES2 [ U* L2 %<D%> U* %<D%> R2 U* ] ;

  define CEN1 [ U* %<D%> CENTER %<D%> U* ] ;

  define NODU [ U | 0:%<D%> ]* ;
  define NODL [ U | %<D%>:0 ]* ;

  regex U* - [ NODU .o. [ CEN1 - [ RES1 | RES2 ] ] .o. NODL ] ;
*/
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.restriction-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.restriction-fn]
pub fn restriction<B: AlgebraBackend>(
    _center: &HfstTransducer<B>,
    context: &HfstTransducerPairVector<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    //check if the center is automata
    let mut proj1: HfstTransducer<B> = _center.clone();
    proj1.input_project()?;
    let mut proj2: HfstTransducer<B> = _center.clone();
    proj2.output_project()?;

    if !proj1.compare(_center, true)? || !proj2.compare(_center, true)? {
        crate::bail!(TransducersAreNotAutomata, "HfstXeroxRules::restriction");
    }

    let restriction_mark: String = "@_D_@".to_string();

    let mut tok: HfstTokenizer = HfstTokenizer::new();
    tok.add_multichar_symbol(&restriction_mark);
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let mark: HfstTransducer<B> = HfstTransducer::new_tokenized(&restriction_mark, &tok)?;
    let epsilon: HfstTransducer<B> = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?;

    // Identity
    let identity_pair: HfstTransducer<B> = HfstTransducer::identity_pair();
    let mut identity: HfstTransducer<B> = identity_pair.clone();
    identity.repeat_star()?.optimize()?;

    let mut universal_without_d: HfstTransducer<B> = identity.clone();
    universal_without_d.insert_to_alphabet_string(&restriction_mark)?;
    let mut universal_without_d_star: HfstTransducer<B> = universal_without_d.clone();
    universal_without_d_star.repeat_star()?.optimize()?;

    // NODU
    let mut no_d_upper: HfstTransducer<B> =
        HfstTransducer::new_tokenized_pair("@_EPSILON_SYMBOL_@", &restriction_mark, &tok)?;
    no_d_upper
        .disjunct(&universal_without_d, true)?
        .repeat_star()?
        .optimize()?;

    // NODL
    let mut no_d_lower: HfstTransducer<B> =
        HfstTransducer::new_tokenized_pair(&restriction_mark, "@_EPSILON_SYMBOL_@", &tok)?;
    no_d_lower
        .disjunct(&universal_without_d, true)?
        .repeat_star()?
        .optimize()?;

    // 1. Surround center with marks
    // [ U* %<D%> CENTER %<D%> U* ]
    let mut center: HfstTransducer<B> = _center.clone();
    center.insert_to_alphabet_string(&restriction_mark)?;

    let mut center_marked: HfstTransducer<B> = universal_without_d_star.clone();
    center_marked
        .concatenate(&mark, true)?
        .concatenate(&center, true)?
        .concatenate(&mark, true)?
        .concatenate(&universal_without_d_star, true)?
        .optimize()?;

    // 2. Put mark in context
    // [ U* L1 %<D%> U* %<D%> R1 U* ]
    let mut context_marked: HfstTransducer<B> = HfstTransducer::new();
    for (i, context_pair) in context.iter().enumerate() {
        let mut lef_context: HfstTransducer<B> = context_pair.0.clone();
        lef_context.insert_to_alphabet_string(&restriction_mark)?;

        let mut right_context: HfstTransducer<B> = context_pair.1.clone();
        right_context.insert_to_alphabet_string(&restriction_mark)?;

        let mut res: HfstTransducer<B> = universal_without_d_star.clone();
        res.concatenate(&lef_context, true)?
            .concatenate(&mark, true)?
            .concatenate(&universal_without_d_star, true)?
            .concatenate(&mark, true)?
            .concatenate(&right_context, true)?
            .concatenate(&universal_without_d_star, true)?
            .optimize()?;

        if i == 0 {
            context_marked = res;
        } else {
            context_marked.disjunct(&res, true)?.optimize()?;
        }
    }
    let mut center_minus_ctx: HfstTransducer<B> = center_marked.clone();
    center_minus_ctx
        .subtract(&context_marked, true)?
        .optimize()?;

    let mut tmp: HfstTransducer<B> = no_d_upper.clone();
    tmp.compose(&center_minus_ctx, true)?
        .compose(&no_d_lower, true)?
        .optimize()?;

    let mut retval: HfstTransducer<B> = universal_without_d_star.clone();
    retval.subtract(&tmp, true)?.optimize()?;

    retval.remove_from_alphabet_string(&restriction_mark)?;

    // deals with boundary symbol
    retval = apply_boundary_mark(&retval)?;

    Ok(retval)
}

// a < b
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.before-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.before-fn]
pub fn before<B: AlgebraBackend>(
    left: &HfstTransducer<B>,
    right: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    //check if the center is automata
    let mut l_proj1: HfstTransducer<B> = left.clone();
    l_proj1.input_project()?;
    let mut l_proj2: HfstTransducer<B> = left.clone();
    l_proj2.output_project()?;
    let mut r_proj1: HfstTransducer<B> = right.clone();
    r_proj1.input_project()?;
    let mut r_proj2: HfstTransducer<B> = right.clone();
    r_proj2.output_project()?;

    if !l_proj1.compare(left, true)?
        || !l_proj2.compare(left, true)?
        || !r_proj1.compare(right, true)?
        || !r_proj2.compare(right, true)?
    {
        crate::bail!(TransducersAreNotAutomata, "HfstXeroxRules::restriction");
    }

    // Identity
    let identity_pair: HfstTransducer<B> = HfstTransducer::identity_pair();
    let mut identity: HfstTransducer<B> = identity_pair.clone();
    identity.repeat_star()?.optimize()?;

    let mut tmp: HfstTransducer<B> = identity.clone();
    tmp.concatenate(right, true)?
        .concatenate(&identity, true)?
        .concatenate(left, true)?
        .concatenate(&identity, true)?
        .optimize()?;

    let mut retval: HfstTransducer<B> = identity.clone();
    retval.subtract(&tmp, true)?.optimize()?;

    Ok(retval)
}

// a > b
// [spec:hfst:def:hfst-xerox-rules.hfst.xerox-rules.after-fn]
// [spec:hfst:sem:hfst-xerox-rules.hfst.xerox-rules.after-fn]
pub fn after<B: AlgebraBackend>(
    left: &HfstTransducer<B>,
    right: &HfstTransducer<B>,
) -> crate::error::Result<HfstTransducer<B>> {
    //check if the center is automata
    let mut l_proj1: HfstTransducer<B> = left.clone();
    l_proj1.input_project()?;
    let mut l_proj2: HfstTransducer<B> = left.clone();
    l_proj2.output_project()?;
    let mut r_proj1: HfstTransducer<B> = right.clone();
    r_proj1.input_project()?;
    let mut r_proj2: HfstTransducer<B> = right.clone();
    r_proj2.output_project()?;

    if !l_proj1.compare(left, true)?
        || !l_proj2.compare(left, true)?
        || !r_proj1.compare(right, true)?
        || !r_proj2.compare(right, true)?
    {
        crate::bail!(TransducersAreNotAutomata, "HfstXeroxRules::restriction");
    }

    // Identity
    let identity_pair: HfstTransducer<B> = HfstTransducer::identity_pair();
    let mut identity: HfstTransducer<B> = identity_pair.clone();
    identity.repeat_star()?.optimize()?;

    let mut tmp: HfstTransducer<B> = identity.clone();
    tmp.concatenate(left, true)?
        .concatenate(&identity, true)?
        .concatenate(right, true)?
        .concatenate(&identity, true)?
        .optimize()?;

    let mut retval: HfstTransducer<B> = identity.clone();
    retval.subtract(&tmp, true)?.optimize()?;

    Ok(retval)
}
