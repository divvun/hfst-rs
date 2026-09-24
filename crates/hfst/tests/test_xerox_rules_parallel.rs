// Port of libhfst/src/HfstXeroxRulesTest.cc: parallel replace rules, compiled
// from a rule vector. The porting notes at the top of test_xerox_rules.rs apply
// here too.

mod xerox_rules_common;

use hfst::backend::AlgebraBackend;
use hfst::hfst_tokenizer::HfstTokenizer;
use hfst::hfst_transducer::{HfstTransducer, HfstTransducerPair, HfstTransducerPairVector};
use hfst::hfst_xerox_rules as xr;
use hfst::hfst_xerox_rules::ReplaceType::{REPL_DOWN, REPL_UP};
use hfst::hfst_xerox_rules::Rule;
use hfst_openfst::StdVectorFst;
use xerox_rules_common::{compose_minimize, serialized};

// replace left b <- a ,, a <- b
// [spec:hfst:def:hfst-xerox-rules-test.test9b-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test9b-fn]
fn test9b<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let mapping_pair1: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("b", &tok)?,
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
    );
    let mapping_pair_vector1: HfstTransducerPairVector<B> = vec![mapping_pair1];

    let mapping_pair2: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
        HfstTransducer::<B>::new_tokenized("b", &tok)?,
    );
    let mapping_pair_vector2: HfstTransducerPairVector<B> = vec![mapping_pair2];

    let rule1 = Rule::new_mapping(&mapping_pair_vector1)?;
    let rule2 = Rule::new_mapping(&mapping_pair_vector2)?;

    let rule_vector: Vec<Rule<B>> = vec![rule1, rule2];

    let input1 = HfstTransducer::<B>::new_tokenized("abba", &tok)?;
    let result1 = HfstTransducer::<B>::new_tokenized_pair("baab", "abba", &tok)?;

    let replace_tr = xr::replace_left_rule_vector(&rule_vector, false)?;

    let tmp2 = compose_minimize(&replace_tr, &input1)?;
    assert!(tmp2.compare(&result1, true)?);
    Ok(())
}

#[test]
fn test9b_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test9b::<StdVectorFst>()?;
    Ok(())
}

// Two unconditional rules `upper1 -> lower1 , upper2 -> lower2`, replaced in
// parallel and non-optionally: `input` composes to the single mapping
// `result`.
fn check_two_rule_parallel_replace<B: AlgebraBackend>(
    (upper1, lower1): (&str, &str),
    (upper2, lower2): (&str, &str),
    input: &str,
    (result_input, result_output): (&str, &str),
) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let mapping_pair1: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized(upper1, &tok)?,
        HfstTransducer::<B>::new_tokenized(lower1, &tok)?,
    );
    let mapping_pair2: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized(upper2, &tok)?,
        HfstTransducer::<B>::new_tokenized(lower2, &tok)?,
    );

    let mapping_pair_vector1: HfstTransducerPairVector<B> = vec![mapping_pair1];
    let mapping_pair_vector2: HfstTransducerPairVector<B> = vec![mapping_pair2];

    let rule1 = Rule::new_mapping(&mapping_pair_vector1)?;
    let rule2 = Rule::new_mapping(&mapping_pair_vector2)?;

    let rule_vector: Vec<Rule<B>> = vec![rule1, rule2];

    let input1 = HfstTransducer::<B>::new_tokenized(input, &tok)?;
    let result1 = HfstTransducer::<B>::new_tokenized_pair(result_input, result_output, &tok)?;

    let replace_tr = xr::replace_rule_vector(&rule_vector, false)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result1, true)?);
    Ok(())
}

// a -> b , b -> c
// [spec:hfst:def:hfst-xerox-rules-test.test7a-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test7a-fn]
fn test7a<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    check_two_rule_parallel_replace::<B>(("a", "b"), ("b", "c"), "aab", ("aab", "bbc"))
}

#[test]
fn test7a_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test7a::<StdVectorFst>()?;
    Ok(())
}

// [. .] -> b , a -> c ;
// [spec:hfst:def:hfst-xerox-rules-test.test7b-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test7b-fn]
fn test7b<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    check_two_rule_parallel_replace::<B>(
        ("@_EPSILON_SYMBOL_@", "b"),
        ("a", "c"),
        "a",
        ("@_EPSILON_SYMBOL_@a@_EPSILON_SYMBOL_@", "bcb"),
    )
}

#[test]
fn test7b_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test7b::<StdVectorFst>()?;
    Ok(())
}

// a+ @-> x , b+ @-> y ; then with contexts
// [spec:hfst:def:hfst-xerox-rules-test.test7c-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test7c-fn]
fn test7c<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let mut left_mapping1 = HfstTransducer::<B>::new_tokenized("a", &tok)?;
    left_mapping1.repeat_plus()?.minimize()?;
    let right_mapping1 = HfstTransducer::<B>::new_tokenized("x", &tok)?;
    let mapping_pair1: HfstTransducerPair<B> = (left_mapping1, right_mapping1);

    let mut left_mapping2 = HfstTransducer::<B>::new_tokenized("b", &tok)?;
    left_mapping2.repeat_plus()?.minimize()?;
    let right_mapping2 = HfstTransducer::<B>::new_tokenized("y", &tok)?;
    let mapping_pair2: HfstTransducerPair<B> = (left_mapping2, right_mapping2);

    let mapping_pair_vector1: HfstTransducerPairVector<B> = vec![mapping_pair1];
    let mapping_pair_vector2: HfstTransducerPairVector<B> = vec![mapping_pair2];

    let rule1 = Rule::new_mapping(&mapping_pair_vector1)?;
    let rule2 = Rule::new_mapping(&mapping_pair_vector2)?;

    let rule_vector: Vec<Rule<B>> = vec![rule1, rule2];

    let input1 = HfstTransducer::<B>::new_tokenized("aaabbb", &tok)?;
    let result1 = HfstTransducer::<B>::new_tokenized_pair(
        "aaabbb",
        "x@_EPSILON_SYMBOL_@@_EPSILON_SYMBOL_@y@_EPSILON_SYMBOL_@@_EPSILON_SYMBOL_@",
        &tok,
    )?;
    let result1b = HfstTransducer::<B>::new_tokenized_pair("aaabbb", "xxxyyy", &tok)?;

    let replace_tr = xr::replace_leftmost_longest_match_rule_vector(&rule_vector)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result1, true)?);

    let replace_tr = xr::replace_leftmost_shortest_match_rule_vector(&rule_vector)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result1b, true)?);

    // With Contexts
    // a -> x \/ m _ ,, b -> y || x _ ;
    let input2 = HfstTransducer::<B>::new_tokenized("mab", &tok)?;
    let result2 = HfstTransducer::<B>::new_tokenized_pair("mab", "mxb", &tok)?;
    let result3 = HfstTransducer::<B>::new_tokenized_pair("mab", "mxy", &tok)?;

    let input3 = HfstTransducer::<B>::new_tokenized("maabb", &tok)?;

    let mut result4 =
        HfstTransducer::<B>::new_tokenized_pair("maabb", "mx@_EPSILON_SYMBOL_@bb", &tok)?;
    let result4b = HfstTransducer::<B>::new_tokenized_pair("maabb", "mxabb", &tok)?;
    result4.disjunct(&result4b, true)?.minimize()?;

    let mut result5 =
        HfstTransducer::<B>::new_tokenized_pair("maabb", "mx@_EPSILON_SYMBOL_@yb", &tok)?;
    let result5b = HfstTransducer::<B>::new_tokenized_pair(
        "maabb",
        "mx@_EPSILON_SYMBOL_@y@_EPSILON_SYMBOL_@",
        &tok,
    )?;
    result5
        .disjunct(&result5b, true)?
        .disjunct(&result4b, true)?
        .minimize()?;

    let context1: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("m", &tok)?,
        HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?,
    );
    let context2: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("x", &tok)?,
        HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?,
    );
    let context_vector1: HfstTransducerPairVector<B> = vec![context1];
    let context_vector2: HfstTransducerPairVector<B> = vec![context2];

    // replace up
    let rule2a_up =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector1, &context_vector1, REPL_UP)?;
    let rule2b_up =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector2, &context_vector2, REPL_UP)?;

    let rule_vector2: Vec<Rule<B>> = vec![rule2a_up, rule2b_up];

    let replace_tr = xr::replace_rule_vector(&rule_vector2, false)?;
    assert!(compose_minimize(&input2, &replace_tr)?.compare(&result2, true)?);
    assert!(compose_minimize(&input3, &replace_tr)?.compare(&result4, true)?);

    // replace down
    let rule2a_down =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector1, &context_vector1, REPL_DOWN)?;
    let rule2b_down =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector2, &context_vector2, REPL_DOWN)?;

    let rule_vector3: Vec<Rule<B>> = vec![rule2a_down, rule2b_down];

    let replace_tr = xr::replace_rule_vector(&rule_vector3, false)?;
    assert!(compose_minimize(&input2, &replace_tr)?.compare(&result3, true)?);
    assert!(compose_minimize(&input3, &replace_tr)?.compare(&result5, true)?);
    Ok(())
}

#[test]
fn test7c_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test7c::<StdVectorFst>()?;
    Ok(())
}

// 0 .o. [ [. 0 .] -> a \/ _ b a , a b _ ,, [. 0 .] -> b \/ a _ a ]
// [spec:hfst:def:hfst-xerox-rules-test.test7d-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test7d-fn]
fn test7d<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let mapping_pair1: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?,
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
    );
    let mapping_pair2: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?,
        HfstTransducer::<B>::new_tokenized("b", &tok)?,
    );

    let mapping_pair_vector1: HfstTransducerPairVector<B> = vec![mapping_pair1];
    let mapping_pair_vector2: HfstTransducerPairVector<B> = vec![mapping_pair2];

    let context1a: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?,
        HfstTransducer::<B>::new_tokenized("ba", &tok)?,
    );
    let context1b: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("ab", &tok)?,
        HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?,
    );
    let context_vector1: HfstTransducerPairVector<B> = vec![context1a, context1b];

    let context2: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
    );
    let context_vector2: HfstTransducerPairVector<B> = vec![context2];

    let rule1 =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector1, &context_vector1, REPL_DOWN)?;
    let rule2 =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector2, &context_vector2, REPL_DOWN)?;

    let rule_vector: Vec<Rule<B>> = vec![rule1, rule2];

    let input1 = HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?;

    let replace_tr = xr::replace_rule_vector(&rule_vector, false)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&input1, true)?);
    Ok(())
}

#[test]
fn test7d_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test7d::<StdVectorFst>()?;
    Ok(())
}

// ? -> x , a -> b
// [spec:hfst:def:hfst-xerox-rules-test.test7e-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test7e-fn]
fn test7e<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_IDENTITY_SYMBOL_@");

    let mapping_pair1: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("@_IDENTITY_SYMBOL_@", &tok)?,
        HfstTransducer::<B>::new_tokenized("x", &tok)?,
    );
    let mapping_pair2: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
        HfstTransducer::<B>::new_tokenized("b", &tok)?,
    );

    let mapping_pair_vector1: HfstTransducerPairVector<B> = vec![mapping_pair1];
    let mapping_pair_vector2: HfstTransducerPairVector<B> = vec![mapping_pair2];

    let rule1 = Rule::new_mapping(&mapping_pair_vector1)?;
    let rule2 = Rule::new_mapping(&mapping_pair_vector2)?;

    let rule_vector: Vec<Rule<B>> = vec![rule1, rule2];

    let input1 = HfstTransducer::<B>::new_tokenized("ak", &tok)?;
    let tmp = HfstTransducer::<B>::new_tokenized_pair("ak", "xx", &tok)?;
    let mut result1 = HfstTransducer::<B>::new_tokenized_pair("ak", "bx", &tok)?;
    result1.disjunct(&tmp, true)?.minimize()?;

    let replace_tr = xr::replace_rule_vector(&rule_vector, false)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result1, true)?);
    Ok(())
}

#[test]
fn test7e_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test7e::<StdVectorFst>()?;
    Ok(())
}

// a -> b , b -> a
// [spec:hfst:def:hfst-xerox-rules-test.test7f-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test7f-fn]
fn test7f<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    check_two_rule_parallel_replace::<B>(("a", "b"), ("b", "a"), "aabbaa", ("aabbaa", "bbaabb"))
}

#[test]
fn test7f_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test7f::<StdVectorFst>()?;
    Ok(())
}

// a -> b b, a -> b
// [spec:hfst:def:hfst-xerox-rules-test.test7g-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test7g-fn]
fn test7g<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let mapping_pair1: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
        HfstTransducer::<B>::new_tokenized("bb", &tok)?,
    );
    let mapping_pair2: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
        HfstTransducer::<B>::new_tokenized("b", &tok)?,
    );

    let mapping_pair_vector1: HfstTransducerPairVector<B> = vec![mapping_pair1];
    let mapping_pair_vector2: HfstTransducerPairVector<B> = vec![mapping_pair2];

    let rule1 = Rule::new_mapping(&mapping_pair_vector1)?;
    let rule2 = Rule::new_mapping(&mapping_pair_vector2)?;

    let rule_vector: Vec<Rule<B>> = vec![rule1, rule2];

    let input1 = HfstTransducer::<B>::new_tokenized("a", &tok)?;
    let mut result1 = HfstTransducer::<B>::new_tokenized_pair("a", "b", &tok)?;
    let result_tmp = HfstTransducer::<B>::new_tokenized_pair("a@_EPSILON_SYMBOL_@", "bb", &tok)?;
    result1.disjunct(&result_tmp, true)?.minimize()?;

    let replace_tr = xr::replace_rule_vector(&rule_vector, false)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result1, true)?);
    Ok(())
}

#[test]
fn test7g_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test7g::<StdVectorFst>()?;
    Ok(())
}
