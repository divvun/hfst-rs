// Port of libhfst/src/HfstXeroxRulesTest.cc: the before rule and the
// restriction rule. The porting notes at the top of test_xerox_rules.rs apply
// here too.

mod xerox_rules_common;

use hfst::backend::AlgebraBackend;
use hfst::hfst_tokenizer::HfstTokenizer;
use hfst::hfst_transducer::{HfstTransducer, HfstTransducerPair, HfstTransducerPairVector};
use hfst::hfst_xerox_rules as xr;
use hfst_openfst::StdVectorFst;
use xerox_rules_common::{compose_minimize, serialized};

// a < b ;
// [spec:hfst:def:hfst-xerox-rules-test.before-test1-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.before-test1-fn]
fn before_test1<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let tok = HfstTokenizer::new();
    let left = HfstTransducer::<B>::new_tokenized("a", &tok)?;
    let right = HfstTransducer::<B>::new_tokenized("b", &tok)?;

    let input1 = HfstTransducer::<B>::new_tokenized("ab", &tok)?;
    let input2 = HfstTransducer::<B>::new_tokenized("acb", &tok)?;
    let input3 = HfstTransducer::<B>::new_tokenized("ba", &tok)?;
    let input4 = HfstTransducer::<B>::new_tokenized("bca", &tok)?;
    let empty = HfstTransducer::<B>::new();

    let before_tr = xr::before(&left, &right)?;

    assert!(compose_minimize(&input1, &before_tr)?.compare(&input1, true)?);
    assert!(compose_minimize(&input2, &before_tr)?.compare(&input2, true)?);
    assert!(compose_minimize(&input3, &before_tr)?.compare(&empty, true)?);
    assert!(compose_minimize(&input4, &before_tr)?.compare(&empty, true)?);
    Ok(())
}

#[test]
fn before_test1_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    before_test1::<StdVectorFst>()?;
    Ok(())
}

// Restriction `center => left _ right` with a single context, run on four
// inputs: the first composes to `result1`, the other three to the empty
// transducer.
fn check_one_context_restriction<B: AlgebraBackend>(
    center: &str,
    (left, right): (&str, &str),
    inputs: [&str; 4],
    result1: &str,
) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let center = HfstTransducer::<B>::new_tokenized(center, &tok)?;

    let context: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized(left, &tok)?,
        HfstTransducer::<B>::new_tokenized(right, &tok)?,
    );
    let context_vector: HfstTransducerPairVector<B> = vec![context];

    let input1 = HfstTransducer::<B>::new_tokenized(inputs[0], &tok)?;
    let input2 = HfstTransducer::<B>::new_tokenized(inputs[1], &tok)?;
    let input3 = HfstTransducer::<B>::new_tokenized(inputs[2], &tok)?;
    let input4 = HfstTransducer::<B>::new_tokenized(inputs[3], &tok)?;
    let result1 = HfstTransducer::<B>::new_tokenized(result1, &tok)?;
    let empty = HfstTransducer::<B>::new();

    let restriction_tr = xr::restriction(&center, &context_vector)?;

    assert!(compose_minimize(&input1, &restriction_tr)?.compare(&result1, true)?);
    assert!(compose_minimize(&input2, &restriction_tr)?.compare(&empty, true)?);
    assert!(compose_minimize(&input3, &restriction_tr)?.compare(&empty, true)?);
    assert!(compose_minimize(&input4, &restriction_tr)?.compare(&empty, true)?);
    Ok(())
}

// restriction rule a => b _ c ;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test1-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test1-fn]
fn restriction_test1<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    check_one_context_restriction::<B>("a", ("b", "c"), ["bac", "abc", "abac", "bcab"], "bac")
}

#[test]
fn restriction_test1_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    restriction_test1::<StdVectorFst>()?;
    Ok(())
}

// restriction rule a => b k _ c ;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test1a-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test1a-fn]
fn restriction_test1a<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    check_one_context_restriction::<B>(
        "a",
        ("bk", "c"),
        ["bkac", "abkc", "abkac", "bkcabk"],
        "bkac",
    )
}

#[test]
fn restriction_test1a_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    restriction_test1a::<StdVectorFst>()?;
    Ok(())
}

// restriction rule a => bb _ bb ;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test1b-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test1b-fn]
fn restriction_test1b<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    check_one_context_restriction::<B>(
        "a",
        ("bb", "bb"),
        ["bbabb", "abb", "abbabb", "bbbbab"],
        "bbabb",
    )
}

#[test]
fn restriction_test1b_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    restriction_test1b::<StdVectorFst>()?;
    Ok(())
}

// restriction rule a k => b _ c ;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test2-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test2-fn]
fn restriction_test2<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    check_one_context_restriction::<B>("ak", ("b", "c"), ["bakc", "akbc", "akbakc", "bcak"], "bakc")
}

#[test]
fn restriction_test2_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    restriction_test2::<StdVectorFst>()?;
    Ok(())
}

// restriction rule a b => b _ c ;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test3-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test3-fn]
fn restriction_test3<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    check_one_context_restriction::<B>("b", ("b", "c"), ["c", "bc", "bbc", "cb"], "c")
}

#[test]
fn restriction_test3_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    restriction_test3::<StdVectorFst>()?;
    Ok(())
}

// restriction rule a => a _ ;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test3a-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test3a-fn]
fn restriction_test3a<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let center = HfstTransducer::<B>::new_tokenized("a", &tok)?;
    let epsilon = HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?;

    let context: HfstTransducerPair<B> = (HfstTransducer::<B>::new_tokenized("a", &tok)?, epsilon);
    let context_vector: HfstTransducerPairVector<B> = vec![context];

    let input1 = HfstTransducer::<B>::new_tokenized("c", &tok)?;
    let input2 = HfstTransducer::<B>::new_tokenized("aa", &tok)?;
    let input3 = HfstTransducer::<B>::new_tokenized("a", &tok)?;
    let input4 = HfstTransducer::<B>::new_tokenized("aca", &tok)?;
    let result1 = HfstTransducer::<B>::new_tokenized("c", &tok)?;
    let empty = HfstTransducer::<B>::new();

    let restriction_tr = xr::restriction(&center, &context_vector)?;

    assert!(compose_minimize(&input1, &restriction_tr)?.compare(&result1, true)?);
    assert!(compose_minimize(&input2, &restriction_tr)?.compare(&empty, true)?);
    assert!(compose_minimize(&input3, &restriction_tr)?.compare(&empty, true)?);
    assert!(compose_minimize(&input4, &restriction_tr)?.compare(&empty, true)?);
    Ok(())
}

#[test]
fn restriction_test3a_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    restriction_test3a::<StdVectorFst>()?;
    Ok(())
}

// restriction rule a b => a b _ ;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test3b-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test3b-fn]
fn restriction_test3b<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let center = HfstTransducer::<B>::new_tokenized("ab", &tok)?;
    let epsilon = HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?;

    let context: HfstTransducerPair<B> = (HfstTransducer::<B>::new_tokenized("ab", &tok)?, epsilon);
    let context_vector: HfstTransducerPairVector<B> = vec![context];

    let input1 = HfstTransducer::<B>::new_tokenized("ba", &tok)?;
    let input2 = HfstTransducer::<B>::new_tokenized("ab", &tok)?;
    let input3 = HfstTransducer::<B>::new_tokenized("abab", &tok)?;
    let input4 = HfstTransducer::<B>::new_tokenized("abc", &tok)?;
    let result1 = HfstTransducer::<B>::new_tokenized("ba", &tok)?;
    let empty = HfstTransducer::<B>::new();

    let restriction_tr = xr::restriction(&center, &context_vector)?;

    assert!(compose_minimize(&input1, &restriction_tr)?.compare(&result1, true)?);
    assert!(compose_minimize(&input2, &restriction_tr)?.compare(&empty, true)?);
    assert!(compose_minimize(&input3, &restriction_tr)?.compare(&empty, true)?);
    assert!(compose_minimize(&input4, &restriction_tr)?.compare(&empty, true)?);
    Ok(())
}

#[test]
fn restriction_test3b_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    restriction_test3b::<StdVectorFst>()?;
    Ok(())
}

// restriction rule a b => _ a b;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test3c-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test3c-fn]
fn restriction_test3c<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let center = HfstTransducer::<B>::new_tokenized("ab", &tok)?;
    let epsilon = HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?;

    let context: HfstTransducerPair<B> = (epsilon, HfstTransducer::<B>::new_tokenized("ab", &tok)?);
    let context_vector: HfstTransducerPairVector<B> = vec![context];

    let input1 = HfstTransducer::<B>::new_tokenized("ba", &tok)?;
    let input2 = HfstTransducer::<B>::new_tokenized("ab", &tok)?;
    let input3 = HfstTransducer::<B>::new_tokenized("abab", &tok)?;
    let input4 = HfstTransducer::<B>::new_tokenized("abc", &tok)?;
    let result1 = HfstTransducer::<B>::new_tokenized("ba", &tok)?;
    let empty = HfstTransducer::<B>::new();

    let restriction_tr = xr::restriction(&center, &context_vector)?;

    assert!(compose_minimize(&input1, &restriction_tr)?.compare(&result1, true)?);
    assert!(compose_minimize(&input2, &restriction_tr)?.compare(&empty, true)?);
    assert!(compose_minimize(&input3, &restriction_tr)?.compare(&empty, true)?);
    assert!(compose_minimize(&input4, &restriction_tr)?.compare(&empty, true)?);
    Ok(())
}

#[test]
fn restriction_test3c_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    restriction_test3c::<StdVectorFst>()?;
    Ok(())
}

// restriction rule a => b _ c , j _ k ;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test4-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test4-fn]
fn restriction_test4<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let center = HfstTransducer::<B>::new_tokenized("a", &tok)?;

    let context1: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("b", &tok)?,
        HfstTransducer::<B>::new_tokenized("c", &tok)?,
    );
    let context2: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("j", &tok)?,
        HfstTransducer::<B>::new_tokenized("k", &tok)?,
    );
    let context_vector: HfstTransducerPairVector<B> = vec![context1, context2];

    let input1 = HfstTransducer::<B>::new_tokenized("bac", &tok)?;
    let input2 = HfstTransducer::<B>::new_tokenized("jak", &tok)?;
    let input3 = HfstTransducer::<B>::new_tokenized("bacjak", &tok)?;
    let input4 = HfstTransducer::<B>::new_tokenized("bajc", &tok)?;

    let result1 = HfstTransducer::<B>::new_tokenized("bac", &tok)?;
    let result2 = HfstTransducer::<B>::new_tokenized("jak", &tok)?;
    let result3 = HfstTransducer::<B>::new_tokenized("bacjak", &tok)?;
    let empty = HfstTransducer::<B>::new();

    let restriction_tr = xr::restriction(&center, &context_vector)?;

    assert!(compose_minimize(&input1, &restriction_tr)?.compare(&result1, true)?);
    assert!(compose_minimize(&input2, &restriction_tr)?.compare(&result2, true)?);
    assert!(compose_minimize(&input3, &restriction_tr)?.compare(&result3, true)?);
    assert!(compose_minimize(&input4, &restriction_tr)?.compare(&empty, true)?);
    Ok(())
}

#[test]
fn restriction_test4_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    restriction_test4::<StdVectorFst>()?;
    Ok(())
}

// restriction rule a => b _ , _ c;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test5-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test5-fn]
fn restriction_test5<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let center = HfstTransducer::<B>::new_tokenized("a", &tok)?;
    let epsilon = HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?;

    let context1: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("b", &tok)?,
        epsilon.clone(),
    );
    let context2: HfstTransducerPair<B> = (epsilon, HfstTransducer::<B>::new_tokenized("c", &tok)?);
    let context_vector: HfstTransducerPairVector<B> = vec![context1, context2];

    let input1 = HfstTransducer::<B>::new_tokenized("bac", &tok)?;
    let input2 = HfstTransducer::<B>::new_tokenized("ba", &tok)?;
    let input3 = HfstTransducer::<B>::new_tokenized("ac", &tok)?;
    let input4 = HfstTransducer::<B>::new_tokenized("abac", &tok)?;

    let result1 = HfstTransducer::<B>::new_tokenized("bac", &tok)?;
    let result2 = HfstTransducer::<B>::new_tokenized("ba", &tok)?;
    let result3 = HfstTransducer::<B>::new_tokenized("ac", &tok)?;
    let empty = HfstTransducer::<B>::new();

    let restriction_tr = xr::restriction(&center, &context_vector)?;

    assert!(compose_minimize(&input1, &restriction_tr)?.compare(&result1, true)?);
    assert!(compose_minimize(&input2, &restriction_tr)?.compare(&result2, true)?);
    assert!(compose_minimize(&input3, &restriction_tr)?.compare(&result3, true)?);
    assert!(compose_minimize(&input4, &restriction_tr)?.compare(&empty, true)?);
    Ok(())
}

#[test]
fn restriction_test5_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    restriction_test5::<StdVectorFst>()?;
    Ok(())
}

// restriction rule a => a _ , _ a;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test5a-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test5a-fn]
fn restriction_test5a<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let center = HfstTransducer::<B>::new_tokenized("a", &tok)?;
    let epsilon = HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?;

    let context1: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
        epsilon.clone(),
    );
    let context2: HfstTransducerPair<B> = (epsilon, HfstTransducer::<B>::new_tokenized("a", &tok)?);
    let context_vector: HfstTransducerPairVector<B> = vec![context1, context2];

    let input1 = HfstTransducer::<B>::new_tokenized("aa", &tok)?;
    let input2 = HfstTransducer::<B>::new_tokenized("aaa", &tok)?;
    let input3 = HfstTransducer::<B>::new_tokenized("ba", &tok)?;
    let input4 = HfstTransducer::<B>::new_tokenized("cac", &tok)?;

    let result1 = HfstTransducer::<B>::new_tokenized("aa", &tok)?;
    let result2 = HfstTransducer::<B>::new_tokenized("aaa", &tok)?;
    let empty = HfstTransducer::<B>::new();

    let restriction_tr = xr::restriction(&center, &context_vector)?;

    assert!(compose_minimize(&input1, &restriction_tr)?.compare(&result1, true)?);
    assert!(compose_minimize(&input2, &restriction_tr)?.compare(&result2, true)?);
    assert!(compose_minimize(&input3, &restriction_tr)?.compare(&empty, true)?);
    assert!(compose_minimize(&input4, &restriction_tr)?.compare(&empty, true)?);
    Ok(())
}

#[test]
fn restriction_test5a_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    restriction_test5a::<StdVectorFst>()?;
    Ok(())
}

// restriction rule a b => a b _ , _ a b ;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test6-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test6-fn]
fn restriction_test6<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let center = HfstTransducer::<B>::new_tokenized("ab", &tok)?;
    let epsilon = HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?;

    let context1: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("ab", &tok)?,
        epsilon.clone(),
    );
    let context2: HfstTransducerPair<B> =
        (epsilon, HfstTransducer::<B>::new_tokenized("ab", &tok)?);
    let context_vector: HfstTransducerPairVector<B> = vec![context1, context2];

    let input1 = HfstTransducer::<B>::new_tokenized("abab", &tok)?;
    let input2 = HfstTransducer::<B>::new_tokenized("ab", &tok)?;
    let input3 = HfstTransducer::<B>::new_tokenized("aba", &tok)?;
    let input4 = HfstTransducer::<B>::new_tokenized("ababab", &tok)?;
    let result1 = HfstTransducer::<B>::new_tokenized("abab", &tok)?;
    let empty = HfstTransducer::<B>::new();

    let restriction_tr = xr::restriction(&center, &context_vector)?;

    assert!(compose_minimize(&input1, &restriction_tr)?.compare(&result1, true)?);
    assert!(compose_minimize(&input2, &restriction_tr)?.compare(&empty, true)?);
    assert!(compose_minimize(&input3, &restriction_tr)?.compare(&empty, true)?);
    assert!(compose_minimize(&input4, &restriction_tr)?.compare(&input4, true)?);
    Ok(())
}

#[test]
fn restriction_test6_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    restriction_test6::<StdVectorFst>()?;
    Ok(())
}

// restriction rule [ x ?* y ] | [ z ?* v ] => b _ c ;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test7-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test7-fn]
fn restriction_test7<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    // Identity (normal)
    let identity_pair = HfstTransducer::<B>::identity_pair();
    let mut identity = identity_pair.clone();
    identity.repeat_star()?.minimize()?;

    let x = HfstTransducer::<B>::new_tokenized("x", &tok)?;
    let y = HfstTransducer::<B>::new_tokenized("y", &tok)?;
    let z = HfstTransducer::<B>::new_tokenized("z", &tok)?;
    let v = HfstTransducer::<B>::new_tokenized("v", &tok)?;
    let mut z_sth_v = z.clone();
    z_sth_v
        .concatenate(&identity, true)?
        .concatenate(&v, true)?
        .minimize()?;

    let mut center = x.clone();
    center
        .concatenate(&identity, true)?
        .concatenate(&y, true)?
        .minimize()?;
    center.disjunct(&z_sth_v, true)?.minimize()?;

    let context1: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("b", &tok)?,
        HfstTransducer::<B>::new_tokenized("c", &tok)?,
    );
    let context_vector: HfstTransducerPairVector<B> = vec![context1];

    let input1 = HfstTransducer::<B>::new_tokenized("bxbzycvc", &tok)?;
    let input2 = HfstTransducer::<B>::new_tokenized("xy", &tok)?;
    let input3 = HfstTransducer::<B>::new_tokenized("zv", &tok)?;
    let input4 = HfstTransducer::<B>::new_tokenized("bxyzvc", &tok)?;
    let result1 = HfstTransducer::<B>::new_tokenized("bxbzycvc", &tok)?;
    let empty = HfstTransducer::<B>::new();

    let restriction_tr = xr::restriction(&center, &context_vector)?;

    assert!(compose_minimize(&input1, &restriction_tr)?.compare(&result1, true)?);
    assert!(compose_minimize(&input2, &restriction_tr)?.compare(&empty, true)?);
    assert!(compose_minimize(&input3, &restriction_tr)?.compare(&empty, true)?);
    assert!(compose_minimize(&input4, &restriction_tr)?.compare(&empty, true)?);
    Ok(())
}

#[test]
fn restriction_test7_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    restriction_test7::<StdVectorFst>()?;
    Ok(())
}

// restriction rule [ x y | x x y y ] => a _ b, x _ y ;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test8-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test8-fn]
fn restriction_test8<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let tmp = HfstTransducer::<B>::new_tokenized("xxyy", &tok)?;
    let mut center = HfstTransducer::<B>::new_tokenized("xy", &tok)?;
    center.disjunct(&tmp, true)?.minimize()?;

    let context1: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
        HfstTransducer::<B>::new_tokenized("b", &tok)?,
    );
    let context2: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("x", &tok)?,
        HfstTransducer::<B>::new_tokenized("y", &tok)?,
    );
    let context_vector: HfstTransducerPairVector<B> = vec![context1, context2];

    let input1 = HfstTransducer::<B>::new_tokenized("axxyyb", &tok)?;
    let input2 = HfstTransducer::<B>::new_tokenized("xxyy", &tok)?;
    let input3 = HfstTransducer::<B>::new_tokenized("xy", &tok)?;
    let input4 = HfstTransducer::<B>::new_tokenized("xxxyyy", &tok)?;
    let result1 = HfstTransducer::<B>::new_tokenized("axxyyb", &tok)?;
    let empty = HfstTransducer::<B>::new();

    let restriction_tr = xr::restriction(&center, &context_vector)?;

    assert!(compose_minimize(&input1, &restriction_tr)?.compare(&result1, true)?);
    assert!(compose_minimize(&input2, &restriction_tr)?.compare(&empty, true)?);
    assert!(compose_minimize(&input3, &restriction_tr)?.compare(&empty, true)?);
    assert!(compose_minimize(&input4, &restriction_tr)?.compare(&input4, true)?);
    Ok(())
}

#[test]
fn restriction_test8_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    restriction_test8::<StdVectorFst>()?;
    Ok(())
}
