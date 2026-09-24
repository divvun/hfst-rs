// Port of libhfst/src/HfstXeroxRulesTest.cc: epenthesis rules, which insert
// material where the left-hand side matches the empty string. The porting
// notes at the top of test_xerox_rules.rs apply here too.

mod xerox_rules_common;

use hfst::backend::AlgebraBackend;
use hfst::hfst_tokenizer::HfstTokenizer;
use hfst::hfst_transducer::{HfstTransducer, HfstTransducerPair, HfstTransducerPairVector};
use hfst::hfst_xerox_rules as xr;
use hfst::hfst_xerox_rules::ReplaceType::REPL_UP;
use hfst::hfst_xerox_rules::Rule;
use hfst_openfst::StdVectorFst;
use xerox_rules_common::{compose_minimize, serialized};

// epenthesis rules: 0 -> p || m _ k
// [spec:hfst:def:hfst-xerox-rules-test.test6a-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test6a-fn]
fn test6a<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_LM_@");
    tok.add_multichar_symbol("@_RM_@");

    let left_mapping = HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?;
    let right_mapping = HfstTransducer::<B>::new_tokenized("p", &tok)?;
    let mapping_pair: HfstTransducerPair<B> = (left_mapping, right_mapping);
    let mapping_pair_vector: HfstTransducerPairVector<B> = vec![mapping_pair];

    let context: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("m", &tok)?,
        HfstTransducer::<B>::new_tokenized("k", &tok)?,
    );
    let context_vector: HfstTransducerPairVector<B> = vec![context];

    let input1 = HfstTransducer::<B>::new_tokenized("mk", &tok)?;

    let result1 = HfstTransducer::<B>::new_tokenized_pair("m@_EPSILON_SYMBOL_@k", "mpk", &tok)?;
    let mut result2 = HfstTransducer::<B>::new_tokenized_pair("mk", "mk", &tok)?;
    result2.disjunct(&result1, true)?.minimize()?;

    let rule_up =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_UP)?;

    // epenthesis rules are covered in basic replace rules
    let replace_tr = xr::replace_rule(&rule_up, false)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result1, true)?);

    let replace_tr = xr::replace_rule(&rule_up, true)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result2, true)?);
    Ok(())
}

#[test]
fn test6a_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test6a::<StdVectorFst>()?;
    Ok(())
}

// a* -> p ;
// [spec:hfst:def:hfst-xerox-rules-test.test6b-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test6b-fn]
fn test6b<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_LM_@");
    tok.add_multichar_symbol("@_RM_@");
    tok.add_multichar_symbol(".#.");

    let mut left_mapping = HfstTransducer::<B>::new_tokenized("a", &tok)?;
    left_mapping.repeat_star()?.minimize()?;

    let right_mapping = HfstTransducer::<B>::new_tokenized("p", &tok)?;
    let mapping_pair: HfstTransducerPair<B> = (left_mapping, right_mapping);
    let mapping_pair_vector: HfstTransducerPairVector<B> = vec![mapping_pair];

    let context: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?,
        HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?,
    );
    let context_vector: HfstTransducerPairVector<B> = vec![context];

    let input1 = HfstTransducer::<B>::new_tokenized("mak", &tok)?;

    let result1 = HfstTransducer::<B>::new_tokenized_pair(
        "@_EPSILON_SYMBOL_@m@_EPSILON_SYMBOL_@a@_EPSILON_SYMBOL_@k@_EPSILON_SYMBOL_@",
        "pmpppkp",
        &tok,
    )?;

    let rule_up =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_UP)?;

    let replace_tr = xr::replace_rule(&rule_up, false)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result1, true)?);
    Ok(())
}

#[test]
fn test6b_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test6b::<StdVectorFst>()?;
    Ok(())
}

// 0 -> b || _ a a
// [spec:hfst:def:hfst-xerox-rules-test.test6c-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test6c-fn]
fn test6c<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_LM_@");
    tok.add_multichar_symbol("@_RM_@");

    let left_mapping = HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?;
    let right_mapping = HfstTransducer::<B>::new_tokenized("b", &tok)?;
    let mapping_pair: HfstTransducerPair<B> = (left_mapping, right_mapping);
    let mapping_pair_vector: HfstTransducerPairVector<B> = vec![mapping_pair];

    let context: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?,
        HfstTransducer::<B>::new_tokenized("aa", &tok)?,
    );
    let context_vector: HfstTransducerPairVector<B> = vec![context];

    let input1 = HfstTransducer::<B>::new_tokenized("aa", &tok)?;

    let result1 = HfstTransducer::<B>::new_tokenized_pair("@_EPSILON_SYMBOL_@aa", "baa", &tok)?;

    let rule_up =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_UP)?;

    let replace_tr = xr::replace_rule(&rule_up, false)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result1, true)?);
    Ok(())
}

#[test]
fn test6c_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test6c::<StdVectorFst>()?;
    Ok(())
}

// [..] -> a;  (obligatory epsilon-LHS empty-context epenthesis)
//
// hfst/hfst#571 DIVERGENCE from upstream C++. This test formerly locked the
// BUGGY 2-state golden FST for `[..] @-> a` that forced exactly one insertion at
// every position and DROPPED identity (input `xy` -> `axaya` ONLY). That machine
// is the still-open upstream bug: mostBracketsStarConstraint makes epsilon-LHS
// empty-context insertion mandatory everywhere. The port now skips that
// constraint for the epsilon-LHS + empty-context shape (see
// is_epsilon_lhs_empty_context in hfst_xerox_rules.rs), so the OBLIGATORY `->`
// arrow yields the SAME free-insertion-with-identity language as the OPTIONAL
// arrow. This test asserts the corrected semantics:
//   * non-optional == optional (the core of the fix), and
//   * `xy` survives unchanged AND `axaya` (insertion at every gap) is accepted.
// [spec:hfst:def:hfst-xerox-rules-test.test7h-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test7h-fn]
fn test7h<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let mapping_pair1: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?,
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
    );
    let mapping_pair_vector1: HfstTransducerPairVector<B> = vec![mapping_pair1];

    let rule = Rule::new_mapping(&mapping_pair_vector1)?;

    // The fix routes the non-optional path to the optional one for this shape.
    let obligatory = xr::replace_rule(&rule, false)?;
    let optional = xr::replace_rule(&rule, true)?;
    assert!(obligatory.compare(&optional, true)?);

    // The set of outputs for input `xy`.
    let xy = HfstTransducer::<B>::new_tokenized("xy", &tok)?;
    let composed = compose_minimize(&xy, &obligatory)?;

    // Identity preserved: `xy -> xy` is one accepted mapping (IMPOSSIBLE under
    // the bug, which forced insertion at every position).
    let xy_to_xy = HfstTransducer::<B>::new_tokenized_pair("xy", "xy", &tok)?;
    let mut missing_identity = xy_to_xy.clone();
    missing_identity.subtract(&composed, true)?.minimize()?;
    assert!(missing_identity.compare(&HfstTransducer::<B>::new(), true)?);

    // Free insertion still available: `xy -> axaya` (insertion at every gap) is
    // also an accepted mapping.
    let xy_to_axaya = HfstTransducer::<B>::new_tokenized_pair(
        "@_EPSILON_SYMBOL_@x@_EPSILON_SYMBOL_@y@_EPSILON_SYMBOL_@",
        "axaya",
        &tok,
    )?;
    let mut missing_insertion = xy_to_axaya.clone();
    missing_insertion.subtract(&composed, true)?.minimize()?;
    assert!(missing_insertion.compare(&HfstTransducer::<B>::new(), true)?);
    Ok(())
}

#[test]
fn test7h_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test7h::<StdVectorFst>()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// hfst/hfst#571 regression coverage.
//
// An obligatory epsilon-LHS + empty-context epenthesis (`[] -> a`, `0 -> a`,
// which both parse to an @0@:a center) must preserve identity and allow free
// insertion at every position, exactly like the optional arrow — it must NOT
// force one insertion everywhere while dropping identity. A context-full
// epenthesis (`0 -> a || b _ c`) MUST be unaffected. See the divergence note in
// hfst_xerox_rules.rs (is_epsilon_lhs_empty_context) and
// docs/spec/port/libhfst/src/HfstXeroxRules.md (replace-fn).
// ---------------------------------------------------------------------------

// Returns true iff `relation` accepts the exact input:output mapping `pair`.
fn accepts_mapping<B: AlgebraBackend>(
    relation: &HfstTransducer<B>,
    pair: &HfstTransducer<B>,
) -> Result<bool, hfst::error::Error> {
    let mut missing = pair.clone();
    missing.subtract(relation, true)?.minimize()?;
    missing.compare(&HfstTransducer::<B>::new(), true)
}

// [] -> a || _  (no context): obligatory == optional, identity + free insertion.
fn test571_epsilon_lhs_no_context<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let mapping_pair: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?,
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
    );
    let mpv: HfstTransducerPairVector<B> = vec![mapping_pair];
    let rule = Rule::new_mapping(&mpv)?;

    let obligatory = xr::replace_rule(&rule, false)?;
    let optional = xr::replace_rule(&rule, true)?;
    // The core of the fix: the obligatory arrow yields the optional language.
    assert!(obligatory.compare(&optional, true)?);

    let xy = HfstTransducer::<B>::new_tokenized("xy", &tok)?;
    let composed = compose_minimize(&xy, &obligatory)?;

    // xy accepted unchanged (identity preserved).
    let xy_to_xy = HfstTransducer::<B>::new_tokenized_pair("xy", "xy", &tok)?;
    assert!(accepts_mapping(&composed, &xy_to_xy)?);

    // axaya still accepted (insertion at every gap).
    let xy_to_axaya = HfstTransducer::<B>::new_tokenized_pair(
        "@_EPSILON_SYMBOL_@x@_EPSILON_SYMBOL_@y@_EPSILON_SYMBOL_@",
        "axaya",
        &tok,
    )?;
    assert!(accepts_mapping(&composed, &xy_to_axaya)?);

    // And a middle-of-the-road output like `axy` is accepted too.
    let xy_to_axy = HfstTransducer::<B>::new_tokenized_pair("@_EPSILON_SYMBOL_@xy", "axy", &tok)?;
    assert!(accepts_mapping(&composed, &xy_to_axy)?);

    // The obligatory arrow must ALSO be exactly the optional arrow's `xy` set,
    // so composing the optional rule with `xy` gives the same relation.
    let composed_opt = compose_minimize(&xy, &optional)?;
    assert!(composed.compare(&composed_opt, true)?);

    // The empty input keeps the empty string (bug dropped it).
    let eps = HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?;
    let composed_eps = compose_minimize(&eps, &obligatory)?;
    let eps_id =
        HfstTransducer::<B>::new_tokenized_pair("@_EPSILON_SYMBOL_@", "@_EPSILON_SYMBOL_@", &tok)?;
    assert!(accepts_mapping(&composed_eps, &eps_id)?);
    Ok(())
}

#[test]
fn test571_epsilon_lhs_no_context_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test571_epsilon_lhs_no_context::<StdVectorFst>()?;
    Ok(())
}

// Context-full epenthesis `0 -> a || b _ c` MUST stay obligatory and NOT change:
// `bc -> bac`, and `xy` (no context match) passes through unchanged.
fn test571_context_full_unchanged<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let mapping_pair: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?,
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
    );
    let mpv: HfstTransducerPairVector<B> = vec![mapping_pair];

    let context: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("b", &tok)?,
        HfstTransducer::<B>::new_tokenized("c", &tok)?,
    );
    let ctxv: HfstTransducerPairVector<B> = vec![context];

    let rule = Rule::new_mapping_context_repl_type(&mpv, &ctxv, REPL_UP)?;
    let obligatory = xr::replace_rule(&rule, false)?;

    // bc -> bac (obligatory insertion in-context).
    let bc = HfstTransducer::<B>::new_tokenized("bc", &tok)?;
    let bac = HfstTransducer::<B>::new_tokenized_pair("b@_EPSILON_SYMBOL_@c", "bac", &tok)?;
    assert!(compose_minimize(&bc, &obligatory)?.compare(&bac, true)?);

    // xy -> xy (no context, no insertion). The obligatory constraint still holds
    // here, so this must NOT gain the free-insertion behavior.
    let xy = HfstTransducer::<B>::new_tokenized("xy", &tok)?;
    assert!(compose_minimize(&xy, &obligatory)?.compare(&xy, true)?);
    Ok(())
}

#[test]
fn test571_context_full_unchanged_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test571_context_full_unchanged::<StdVectorFst>()?;
    Ok(())
}

// The optional arrow `[] (->) a || _` MUST be unchanged by the fix: identity
// preserved, free insertion available.
fn test571_optional_unchanged<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let mapping_pair: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?,
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
    );
    let mpv: HfstTransducerPairVector<B> = vec![mapping_pair];
    let rule = Rule::new_mapping(&mpv)?;

    let optional = xr::replace_rule(&rule, true)?;
    let xy = HfstTransducer::<B>::new_tokenized("xy", &tok)?;
    let composed = compose_minimize(&xy, &optional)?;

    let xy_to_xy = HfstTransducer::<B>::new_tokenized_pair("xy", "xy", &tok)?;
    assert!(accepts_mapping(&composed, &xy_to_xy)?);

    let xy_to_axaya = HfstTransducer::<B>::new_tokenized_pair(
        "@_EPSILON_SYMBOL_@x@_EPSILON_SYMBOL_@y@_EPSILON_SYMBOL_@",
        "axaya",
        &tok,
    )?;
    assert!(accepts_mapping(&composed, &xy_to_axaya)?);
    Ok(())
}

#[test]
fn test571_optional_unchanged_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test571_optional_unchanged::<StdVectorFst>()?;
    Ok(())
}
