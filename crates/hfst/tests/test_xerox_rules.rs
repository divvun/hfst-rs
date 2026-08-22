// Port of libhfst/src/HfstXeroxRulesTest.cc
//
// Tests the hfst::xeroxRules namespace: the Rule data type plus the replace /
// replace_left / replace_leftmost_longest_match / replace_leftmost_shortest_match
// (and rightmost) functions, the restriction rule, and the before/after rules.
//
// The C++ main loops over several backends. Per the Wave-2 port scope only
// TROPICAL_OPENFST_TYPE is exercised here: SFST / FOMA / XFSM are out of scope
// (is_implementation_type_available returns false for them in this build).
// Each C++ void testX(ImplementationType) becomes
// a Rust helper fn generic over the backend B, plus a #[test] wrapper that runs
// it for TROPICAL (StdVectorFst). The C++ MAIN_TEST driver is not ported; the
// wrappers are the driver.
//
// The C++ replace(rule, bool) / replace_left / replace_leftmost_longest_match etc.
// are renamed in this Rust port: replace_rule, replace_left_rule,
// replace_leftmost_longest_match_rule, and so on, plus _rule_vector variants for
// the std::vector<Rule> overloads. Those renames are applied below.
//
// C++ assert(a.compare(b)) defaults harmonize=true and is mirrored as
// assert!(a.compare(&b, true)). C++ comments marked FAIL flag asserts the original
// author knew were failing; they are still ported faithfully.

use hfst::backend::AlgebraBackend;
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_tokenizer::HfstTokenizer;
use hfst::hfst_transducer::{HfstTransducer, HfstTransducerPair, HfstTransducerPairVector};
use hfst::hfst_xerox_rules as xr;
use hfst::hfst_xerox_rules::ReplaceType::{REPL_DOWN, REPL_LEFT, REPL_RIGHT, REPL_UP};
use hfst::hfst_xerox_rules::Rule;
use hfst_openfst::StdVectorFst;

// The tropical transition-data symbol coding lives in process-global statics
// behind Mutexes. cargo runs every #[test] as a parallel thread in ONE process,
// but each C++ test was its own process. Serializing the tests through this lock
// restores the one-at-a-time-per-process model without touching the library or
// weakening any assertion. into_inner() recovers from a poisoned lock so one
// failing test does not cascade.
static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

// C++ pattern: tmp = left; tmp.compose(right).minimize();
fn compose_minimize<B: AlgebraBackend>(
    left: &HfstTransducer<B>,
    right: &HfstTransducer<B>,
) -> Result<HfstTransducer<B>, hfst::error::Error> {
    let mut t = left.clone();
    t.compose(right, true)?.minimize()?;
    Ok(t)
}

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

// restriction rule a => b _ c ;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test1-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test1-fn]
fn restriction_test1<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let center = HfstTransducer::<B>::new_tokenized("a", &tok)?;

    let context: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("b", &tok)?,
        HfstTransducer::<B>::new_tokenized("c", &tok)?,
    );
    let context_vector: HfstTransducerPairVector<B> = vec![context];

    let input1 = HfstTransducer::<B>::new_tokenized("bac", &tok)?;
    let input2 = HfstTransducer::<B>::new_tokenized("abc", &tok)?;
    let input3 = HfstTransducer::<B>::new_tokenized("abac", &tok)?;
    let input4 = HfstTransducer::<B>::new_tokenized("bcab", &tok)?;
    let result1 = HfstTransducer::<B>::new_tokenized("bac", &tok)?;
    let empty = HfstTransducer::<B>::new();

    let restriction_tr = xr::restriction(&center, &context_vector)?;

    assert!(compose_minimize(&input1, &restriction_tr)?.compare(&result1, true)?);
    assert!(compose_minimize(&input2, &restriction_tr)?.compare(&empty, true)?);
    assert!(compose_minimize(&input3, &restriction_tr)?.compare(&empty, true)?);
    assert!(compose_minimize(&input4, &restriction_tr)?.compare(&empty, true)?);
    Ok(())
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
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let center = HfstTransducer::<B>::new_tokenized("a", &tok)?;

    let context: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("bk", &tok)?,
        HfstTransducer::<B>::new_tokenized("c", &tok)?,
    );
    let context_vector: HfstTransducerPairVector<B> = vec![context];

    let input1 = HfstTransducer::<B>::new_tokenized("bkac", &tok)?;
    let input2 = HfstTransducer::<B>::new_tokenized("abkc", &tok)?;
    let input3 = HfstTransducer::<B>::new_tokenized("abkac", &tok)?;
    let input4 = HfstTransducer::<B>::new_tokenized("bkcabk", &tok)?;
    let result1 = HfstTransducer::<B>::new_tokenized("bkac", &tok)?;
    let empty = HfstTransducer::<B>::new();

    let restriction_tr = xr::restriction(&center, &context_vector)?;

    assert!(compose_minimize(&input1, &restriction_tr)?.compare(&result1, true)?);
    assert!(compose_minimize(&input2, &restriction_tr)?.compare(&empty, true)?);
    assert!(compose_minimize(&input3, &restriction_tr)?.compare(&empty, true)?);
    assert!(compose_minimize(&input4, &restriction_tr)?.compare(&empty, true)?);
    Ok(())
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
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let center = HfstTransducer::<B>::new_tokenized("a", &tok)?;

    let context: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("bb", &tok)?,
        HfstTransducer::<B>::new_tokenized("bb", &tok)?,
    );
    let context_vector: HfstTransducerPairVector<B> = vec![context];

    let input1 = HfstTransducer::<B>::new_tokenized("bbabb", &tok)?;
    let input2 = HfstTransducer::<B>::new_tokenized("abb", &tok)?;
    let input3 = HfstTransducer::<B>::new_tokenized("abbabb", &tok)?;
    let input4 = HfstTransducer::<B>::new_tokenized("bbbbab", &tok)?;
    let result1 = HfstTransducer::<B>::new_tokenized("bbabb", &tok)?;
    let empty = HfstTransducer::<B>::new();

    let restriction_tr = xr::restriction(&center, &context_vector)?;

    assert!(compose_minimize(&input1, &restriction_tr)?.compare(&result1, true)?);
    assert!(compose_minimize(&input2, &restriction_tr)?.compare(&empty, true)?);
    assert!(compose_minimize(&input3, &restriction_tr)?.compare(&empty, true)?);
    assert!(compose_minimize(&input4, &restriction_tr)?.compare(&empty, true)?);
    Ok(())
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
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let center = HfstTransducer::<B>::new_tokenized("ak", &tok)?;

    let context: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("b", &tok)?,
        HfstTransducer::<B>::new_tokenized("c", &tok)?,
    );
    let context_vector: HfstTransducerPairVector<B> = vec![context];

    let input1 = HfstTransducer::<B>::new_tokenized("bakc", &tok)?;
    let input2 = HfstTransducer::<B>::new_tokenized("akbc", &tok)?;
    let input3 = HfstTransducer::<B>::new_tokenized("akbakc", &tok)?;
    let input4 = HfstTransducer::<B>::new_tokenized("bcak", &tok)?;
    let result1 = HfstTransducer::<B>::new_tokenized("bakc", &tok)?;
    let empty = HfstTransducer::<B>::new();

    let restriction_tr = xr::restriction(&center, &context_vector)?;

    assert!(compose_minimize(&input1, &restriction_tr)?.compare(&result1, true)?);
    assert!(compose_minimize(&input2, &restriction_tr)?.compare(&empty, true)?);
    assert!(compose_minimize(&input3, &restriction_tr)?.compare(&empty, true)?);
    assert!(compose_minimize(&input4, &restriction_tr)?.compare(&empty, true)?);
    Ok(())
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
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let center = HfstTransducer::<B>::new_tokenized("b", &tok)?;

    let context: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("b", &tok)?,
        HfstTransducer::<B>::new_tokenized("c", &tok)?,
    );
    let context_vector: HfstTransducerPairVector<B> = vec![context];

    let input1 = HfstTransducer::<B>::new_tokenized("c", &tok)?;
    let input2 = HfstTransducer::<B>::new_tokenized("bc", &tok)?;
    let input3 = HfstTransducer::<B>::new_tokenized("bbc", &tok)?;
    let input4 = HfstTransducer::<B>::new_tokenized("cb", &tok)?;
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

// empty language replacements
// a -> ~[?*]
// [spec:hfst:def:hfst-xerox-rules-test.test10a-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test10a-fn]
fn test10a<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let tok = HfstTokenizer::new();

    let mapping_pair: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
        HfstTransducer::<B>::new(),
    );
    let mapping_pair_vector: HfstTransducerPairVector<B> = vec![mapping_pair];

    let rule = Rule::new_mapping(&mapping_pair_vector)?;

    let identity_pair = HfstTransducer::<B>::identity_pair();
    let mut result1 = identity_pair.clone();
    result1.repeat_star()?.minimize()?;
    result1.insert_to_alphabet_symbol("a")?;

    let replace_tr = xr::replace_rule(&rule, false)?;

    assert!(replace_tr.compare(&result1, true)?);
    Ok(())
}

#[test]
fn test10a_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test10a::<StdVectorFst>()?;
    Ok(())
}

// empty language replacements
// ~[?*] -> a
// [spec:hfst:def:hfst-xerox-rules-test.test10b-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test10b-fn]
fn test10b<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let tok = HfstTokenizer::new();

    let mapping_pair: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new(),
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
    );
    let mapping_pair_vector: HfstTransducerPairVector<B> = vec![mapping_pair];

    let rule = Rule::new_mapping(&mapping_pair_vector)?;

    let identity_pair = HfstTransducer::<B>::identity_pair();
    let mut result1 = identity_pair.clone();
    result1.repeat_star()?.minimize()?;

    let replace_tr = xr::replace_rule(&rule, false)?;

    assert!(replace_tr.compare(&result1, true)?);
    Ok(())
}

#[test]
fn test10b_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test10b::<StdVectorFst>()?;
    Ok(())
}

// replace left d <- ca || ca_c  ( input: c a c a c a c )
// [spec:hfst:def:hfst-xerox-rules-test.test9a-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test9a-fn]
fn test9a<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let mapping_pair: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("d@_EPSILON_SYMBOL_@", &tok)?,
        HfstTransducer::<B>::new_tokenized("ca", &tok)?,
    );
    let mapping_pair_vector: HfstTransducerPairVector<B> = vec![mapping_pair];

    let context: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("ca", &tok)?,
        HfstTransducer::<B>::new_tokenized("c", &tok)?,
    );
    let context_vector: HfstTransducerPairVector<B> = vec![context];

    let rule = Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_UP)?;

    let input1 = HfstTransducer::<B>::new_tokenized("cacacac", &tok)?;
    let result1 = HfstTransducer::<B>::new_tokenized_pair(
        "cad@_EPSILON_SYMBOL_@d@_EPSILON_SYMBOL_@c",
        "cacacac",
        &tok,
    )?;

    let replace_tr = xr::replace_left_rule(&rule, false)?;

    let tmp2 = compose_minimize(&replace_tr, &input1)?;
    assert!(tmp2.compare(&result1, true)?);
    Ok(())
}

#[test]
fn test9a_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test9a::<StdVectorFst>()?;
    Ok(())
}

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

// ab->x  ab_a
// [spec:hfst:def:hfst-xerox-rules-test.test1-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test1-fn]
fn test1<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let left_mapping = HfstTransducer::<B>::new_tokenized("ab", &tok)?;
    let right_mapping = HfstTransducer::<B>::new_tokenized("x", &tok)?;
    let mapping_pair: HfstTransducerPair<B> = (left_mapping, right_mapping);
    let mapping_pair_vector: HfstTransducerPairVector<B> = vec![mapping_pair];

    let context: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("ab", &tok)?,
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
    );
    let context_vector: HfstTransducerPairVector<B> = vec![context];

    let input1 = HfstTransducer::<B>::new_tokenized("abababa", &tok)?;

    let mut result1 = HfstTransducer::<B>::new_tokenized("abababa", &tok)?;
    let r1tmp =
        HfstTransducer::<B>::new_tokenized_pair("abababa", "abx@_EPSILON_SYMBOL_@aba", &tok)?;
    let r2tmp =
        HfstTransducer::<B>::new_tokenized_pair("abababa", "ababx@_EPSILON_SYMBOL_@a", &tok)?;
    let r3tmp = HfstTransducer::<B>::new_tokenized_pair(
        "abababa",
        "abx@_EPSILON_SYMBOL_@x@_EPSILON_SYMBOL_@a",
        &tok,
    )?;
    result1
        .disjunct(&r1tmp, true)?
        .disjunct(&r2tmp, true)?
        .minimize()?
        .disjunct(&r3tmp, true)?
        .minimize()?;

    let rule = Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_UP)?;

    // Unconditional optional replace
    let replace_tr = xr::replace_rule(&rule, true)?;
    let tmp2 = compose_minimize(&input1, &replace_tr)?;
    assert!(tmp2.compare(&result1, true)?); // FAIL

    // replace up non optional / left most optional
    let replace_tr = xr::replace_rule(&rule, false)?;
    let tmp2 = compose_minimize(&input1, &replace_tr)?;
    assert!(tmp2.compare(&r3tmp, true)?);
    Ok(())
}

#[test]
fn test1_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test1::<StdVectorFst>()?;
    Ok(())
}

// a -> x
// [spec:hfst:def:hfst-xerox-rules-test.test1b-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test1b-fn]
fn test1b<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let left_mapping = HfstTransducer::<B>::new_tokenized("a", &tok)?;
    let right_mapping = HfstTransducer::<B>::new_tokenized("x", &tok)?;
    let mapping_pair: HfstTransducerPair<B> = (left_mapping, right_mapping);
    let mapping_pair_vector: HfstTransducerPairVector<B> = vec![mapping_pair];

    let context: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?,
        HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?,
    );
    let context_vector: HfstTransducerPairVector<B> = vec![context];

    let input1 = HfstTransducer::<B>::new_tokenized("aaana", &tok)?;

    let mut bt = HfstBasicTransducer::new();
    let tr = HfstBasicTransition::new_symbols(1, "a".into(), "a".into(), 0.0, bt.coder_mut());
    bt.add_transition(0, &tr, true);
    let tr = HfstBasicTransition::new_symbols(1, "a".into(), "x".into(), 0.0, bt.coder_mut());
    bt.add_transition(0, &tr, true);
    let tr = HfstBasicTransition::new_symbols(2, "a".into(), "a".into(), 0.0, bt.coder_mut());
    bt.add_transition(1, &tr, true);
    let tr = HfstBasicTransition::new_symbols(2, "a".into(), "x".into(), 0.0, bt.coder_mut());
    bt.add_transition(1, &tr, true);
    let tr = HfstBasicTransition::new_symbols(3, "a".into(), "a".into(), 0.0, bt.coder_mut());
    bt.add_transition(2, &tr, true);
    let tr = HfstBasicTransition::new_symbols(3, "a".into(), "x".into(), 0.0, bt.coder_mut());
    bt.add_transition(2, &tr, true);
    let tr = HfstBasicTransition::new_symbols(4, "n".into(), "n".into(), 0.0, bt.coder_mut());
    bt.add_transition(3, &tr, true);
    let tr = HfstBasicTransition::new_symbols(5, "a".into(), "a".into(), 0.0, bt.coder_mut());
    bt.add_transition(4, &tr, true);
    let tr = HfstBasicTransition::new_symbols(5, "a".into(), "x".into(), 0.0, bt.coder_mut());
    bt.add_transition(4, &tr, true);
    bt.set_final_weight(5, &0.0);

    let result1 = HfstTransducer::<B>::new_from_basic(&bt)?;
    let result2 = HfstTransducer::<B>::new_tokenized_pair("aaana", "xxxnx", &tok)?;

    let rule = Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_UP)?;

    // Unconditional optional replace
    let replace_tr = xr::replace_rule(&rule, true)?;
    let tmp2 = compose_minimize(&input1, &replace_tr)?;
    assert!(tmp2.compare(&result1, true)?);

    // non optional
    let replace_tr = xr::replace_rule(&rule, false)?;
    let tmp2 = compose_minimize(&input1, &replace_tr)?;
    assert!(tmp2.compare(&result2, true)?);

    // Left most longest match
    let replace_tr = xr::replace_leftmost_longest_match_rule(&rule)?;
    let tmp2 = compose_minimize(&input1, &replace_tr)?;
    assert!(tmp2.compare(&result2, true)?);

    // Left most shortest match
    let replace_tr = xr::replace_leftmost_shortest_match_rule(&rule)?;
    let tmp2 = compose_minimize(&input1, &replace_tr)?;
    assert!(tmp2.compare(&result2, true)?);
    Ok(())
}

#[test]
fn test1b_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test1b::<StdVectorFst>()?;
    Ok(())
}

// ? -> x
// [spec:hfst:def:hfst-xerox-rules-test.test1c-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test1c-fn]
fn test1c<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_UNKNOWN_SYMBOL_@");
    tok.add_multichar_symbol("@_IDENTITY_SYMBOL_@");

    let left_mapping = HfstTransducer::<B>::new_tokenized("@_IDENTITY_SYMBOL_@", &tok)?;
    let right_mapping = HfstTransducer::<B>::new_tokenized("x", &tok)?;
    let mapping_pair: HfstTransducerPair<B> = (left_mapping, right_mapping);
    let mapping_pair_vector: HfstTransducerPairVector<B> = vec![mapping_pair];

    let input1 = HfstTransducer::<B>::new_tokenized("s", &tok)?;
    let result1 = HfstTransducer::<B>::new_tokenized_pair("s", "x", &tok)?;

    let rule = Rule::new_mapping(&mapping_pair_vector)?;

    let replace_tr = xr::replace_rule(&rule, false)?;

    let tmp2 = compose_minimize(&input1, &replace_tr)?;
    assert!(tmp2.compare(&result1, true)?);
    Ok(())
}

#[test]
fn test1c_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test1c::<StdVectorFst>()?;
    Ok(())
}

// a -> b || .#. _ c;
// [spec:hfst:def:hfst-xerox-rules-test.test1d-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test1d-fn]
fn test1d<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol(".#.");

    let left_mapping = HfstTransducer::<B>::new_tokenized("a", &tok)?;
    let right_mapping = HfstTransducer::<B>::new_tokenized("b", &tok)?;
    let mapping_pair: HfstTransducerPair<B> = (left_mapping, right_mapping);
    let mapping_pair_vector: HfstTransducerPairVector<B> = vec![mapping_pair];

    let input1 = HfstTransducer::<B>::new_tokenized(".#.ac", &tok)?;
    let input2 = HfstTransducer::<B>::new_tokenized("ac", &tok)?;
    let result1 = HfstTransducer::<B>::new_tokenized_pair(".#.ac", ".#.ac", &tok)?;
    let result2 = HfstTransducer::<B>::new_tokenized_pair("ac", "bc", &tok)?;

    let context: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized(".#.", &tok)?,
        HfstTransducer::<B>::new_tokenized("c", &tok)?,
    );
    let context_vector: HfstTransducerPairVector<B> = vec![context];

    let rule = Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_UP)?;

    let replace_tr = xr::replace_rule(&rule, false)?;

    let tmp2 = compose_minimize(&input1, &replace_tr)?;
    assert!(tmp2.compare(&result1, true)?);

    let tmp2 = compose_minimize(&input2, &replace_tr)?;
    assert!(tmp2.compare(&result2, true)?);
    Ok(())
}

#[test]
fn test1d_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test1d::<StdVectorFst>()?;
    Ok(())
}

// a+ @-> x || a _ a
// [spec:hfst:def:hfst-xerox-rules-test.test2a-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test2a-fn]
fn test2a<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_UNKNOWN_SYMBOL_@");
    tok.add_multichar_symbol("@_IDENTITY_SYMBOL_@");
    tok.add_multichar_symbol("@_LM_@");
    tok.add_multichar_symbol("@_RM_@");

    let mut left_mapping = HfstTransducer::<B>::new_tokenized("a", &tok)?;
    left_mapping.repeat_plus()?.minimize()?;
    let right_mapping = HfstTransducer::<B>::new_tokenized("x", &tok)?;

    let mapping_pair: HfstTransducerPair<B> = (left_mapping, right_mapping);
    let mapping_pair_vector: HfstTransducerPairVector<B> = vec![mapping_pair];

    let context: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
    );
    let context_vector: HfstTransducerPairVector<B> = vec![context];

    let input1 = HfstTransducer::<B>::new_tokenized("aaaa", &tok)?;
    let input2 = HfstTransducer::<B>::new_tokenized("aaaaabaaaa", &tok)?;
    let input3 = HfstTransducer::<B>::new_tokenized("aaaaabaaaacaaaa", &tok)?;

    let mut result1 = HfstTransducer::<B>::new_tokenized("aaaa", &tok)?;
    let r1tmp = HfstTransducer::<B>::new_tokenized_pair("aaaa", "ax@_EPSILON_SYMBOL_@a", &tok)?;
    let r2tmp = HfstTransducer::<B>::new_tokenized_pair("aaaa", "axaa", &tok)?;
    let r3tmp = HfstTransducer::<B>::new_tokenized_pair("aaaa", "aaxa", &tok)?;
    let r4tmp = HfstTransducer::<B>::new_tokenized_pair("aaaa", "axxa", &tok)?;

    result1
        .disjunct(&r1tmp, true)?
        .minimize()?
        .disjunct(&r2tmp, true)?
        .minimize()?
        .disjunct(&r3tmp, true)?
        .minimize()?;

    let mut result8 = result1.clone();
    result8.disjunct(&r4tmp, true)?.minimize()?;

    let mut result2 = r1tmp.clone();
    result2.disjunct(&r4tmp, true)?.minimize()?;

    let result3 = r1tmp.clone();

    let mut result9 = r1tmp.clone();
    result9.disjunct(&r2tmp, true)?.minimize()?;

    let mut result10 = r1tmp.clone();
    result10.disjunct(&r3tmp, true)?.minimize()?;

    let mut result11 = result10.clone();
    result11.disjunct(&r2tmp, true)?.minimize()?;

    let result4 = HfstTransducer::<B>::new_tokenized_pair(
        "aaaaabaaaa",
        "ax@_EPSILON_SYMBOL_@@_EPSILON_SYMBOL_@abax@_EPSILON_SYMBOL_@a",
        &tok,
    )?;
    let result5 = HfstTransducer::<B>::new_tokenized_pair("aaaaabaaaa", "axxxabaxxa", &tok)?;

    let result6 = HfstTransducer::<B>::new_tokenized_pair(
        "aaaaabaaaacaaaa",
        "ax@_EPSILON_SYMBOL_@@_EPSILON_SYMBOL_@abax@_EPSILON_SYMBOL_@acax@_EPSILON_SYMBOL_@a",
        &tok,
    )?;
    let result7 =
        HfstTransducer::<B>::new_tokenized_pair("aaaaabaaaacaaaa", "axxxabaxxacaxxa", &tok)?;

    let rule_up =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_UP)?;
    let rule_left =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_LEFT)?;
    let rule_right =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_RIGHT)?;
    let rule_down =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_DOWN)?;

    // Unconditional optional replace
    let replace_tr_up = xr::replace_rule(&rule_up, true)?;
    let replace_tr_left = xr::replace_rule(&rule_left, true)?;
    let replace_tr_right = xr::replace_rule(&rule_right, true)?;
    let replace_tr_down = xr::replace_rule(&rule_down, true)?;

    assert!(compose_minimize(&input1, &replace_tr_up)?.compare(&result8, true)?);
    assert!(compose_minimize(&input1, &replace_tr_left)?.compare(&result1, true)?);
    assert!(compose_minimize(&input1, &replace_tr_right)?.compare(&result1, true)?);
    assert!(compose_minimize(&input1, &replace_tr_down)?.compare(&result1, true)?);

    // Non optional replacements
    let replace_tr_up = xr::replace_rule(&rule_up, false)?;
    let replace_tr_left = xr::replace_rule(&rule_left, false)?;
    let replace_tr_right = xr::replace_rule(&rule_right, false)?;
    let replace_tr_down = xr::replace_rule(&rule_down, false)?;

    assert!(compose_minimize(&input1, &replace_tr_up)?.compare(&result2, true)?);
    assert!(compose_minimize(&input1, &replace_tr_left)?.compare(&result10, true)?);
    assert!(compose_minimize(&input1, &replace_tr_right)?.compare(&result9, true)?);
    assert!(compose_minimize(&input1, &replace_tr_down)?.compare(&result11, true)?);

    // Left most longest match
    let replace_tr = xr::replace_leftmost_longest_match_rule(&rule_up)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result3, true)?);
    assert!(compose_minimize(&input2, &replace_tr)?.compare(&result4, true)?);
    assert!(compose_minimize(&input3, &replace_tr)?.compare(&result6, true)?);

    // Left most shortest match
    let replace_tr = xr::replace_leftmost_shortest_match_rule(&rule_up)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&r4tmp, true)?);
    assert!(compose_minimize(&input2, &replace_tr)?.compare(&result5, true)?);
    assert!(compose_minimize(&input3, &replace_tr)?.compare(&result7, true)?);
    Ok(())
}

#[test]
fn test2a_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test2a::<StdVectorFst>()?;
    Ok(())
}

// a+ b+ | b+ a+ @-> x
// [spec:hfst:def:hfst-xerox-rules-test.test2b-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test2b-fn]
fn test2b<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_LM_@");
    tok.add_multichar_symbol("@_RM_@");

    let mut a_plus = HfstTransducer::<B>::new_tokenized("a", &tok)?;
    a_plus.repeat_plus()?.minimize()?;
    let mut b_plus = HfstTransducer::<B>::new_tokenized("b", &tok)?;
    b_plus.repeat_plus()?.minimize()?;

    // a+ b+
    let mut mtmp1 = a_plus.clone();
    mtmp1.concatenate(&b_plus, true)?.minimize()?;
    // b+ a+
    let mut mtmp2 = b_plus.clone();
    mtmp2.concatenate(&a_plus, true)?.minimize()?;
    // a+ b+ | b+ a+ -> x
    let mut left_mapping = mtmp1.clone();
    left_mapping.disjunct(&mtmp2, true)?.minimize()?;
    let right_mapping = HfstTransducer::<B>::new_tokenized("x", &tok)?;

    let mapping_pair: HfstTransducerPair<B> = (left_mapping, right_mapping);
    let mapping_pair_vector: HfstTransducerPairVector<B> = vec![mapping_pair];

    let input1 = HfstTransducer::<B>::new_tokenized("aabbaa", &tok)?;

    let result1 = HfstTransducer::<B>::new_tokenized_pair(
        "aabbaa",
        "x@_EPSILON_SYMBOL_@@_EPSILON_SYMBOL_@@_EPSILON_SYMBOL_@aa",
        &tok,
    )?;
    let result2 = HfstTransducer::<B>::new_tokenized_pair(
        "aabbaa",
        "aax@_EPSILON_SYMBOL_@@_EPSILON_SYMBOL_@@_EPSILON_SYMBOL_@",
        &tok,
    )?;
    let result3 = HfstTransducer::<B>::new_tokenized_pair(
        "aabbaa",
        "x@_EPSILON_SYMBOL_@@_EPSILON_SYMBOL_@x@_EPSILON_SYMBOL_@a",
        &tok,
    )?;
    let result4 = HfstTransducer::<B>::new_tokenized_pair(
        "aabbaa",
        "ax@_EPSILON_SYMBOL_@x@_EPSILON_SYMBOL_@@_EPSILON_SYMBOL_@",
        &tok,
    )?;

    let rule_up = Rule::new_mapping(&mapping_pair_vector)?;

    // leftmost longest match
    let replace_tr = xr::replace_leftmost_longest_match_rule(&rule_up)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result1, true)?);

    // rightmost longest match
    let replace_tr = xr::replace_rightmost_longest_match_rule(&rule_up)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result2, true)?);

    // leftmost shortest match
    let replace_tr = xr::replace_leftmost_shortest_match_rule(&rule_up)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result3, true)?);

    // rightmost shortest match
    let replace_tr = xr::replace_rightmost_shortest_match_rule(&rule_up)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result4, true)?);

    // in context
    // a+ b+ | b+ a+ @-> x \/ _ x ;  input: aabbaax
    let input2 = HfstTransducer::<B>::new_tokenized("aabbaax", &tok)?;
    let result5 = HfstTransducer::<B>::new_tokenized_pair(
        "aabbaax",
        "x@_EPSILON_SYMBOL_@@_EPSILON_SYMBOL_@x@_EPSILON_SYMBOL_@@_EPSILON_SYMBOL_@x",
        &tok,
    )?;
    let context: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?,
        HfstTransducer::<B>::new_tokenized("x", &tok)?,
    );
    let context_vector: HfstTransducerPairVector<B> = vec![context];

    let rule_down =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_DOWN)?;

    // leftmost longest match in context
    let replace_tr = xr::replace_leftmost_longest_match_rule(&rule_down)?;
    assert!(compose_minimize(&input2, &replace_tr)?.compare(&result5, true)?);
    Ok(())
}

#[test]
fn test2b_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test2b::<StdVectorFst>()?;
    Ok(())
}

// a+ @-> x || c _
// [spec:hfst:def:hfst-xerox-rules-test.test2c-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test2c-fn]
fn test2c<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_UNKNOWN_SYMBOL_@");
    tok.add_multichar_symbol("@_IDENTITY_SYMBOL_@");
    tok.add_multichar_symbol("@_LM_@");
    tok.add_multichar_symbol("@_RM_@");

    let mut left_mapping = HfstTransducer::<B>::new_tokenized("a", &tok)?;
    left_mapping.repeat_plus()?.minimize()?;
    let right_mapping = HfstTransducer::<B>::new_tokenized("x", &tok)?;

    let mapping_pair: HfstTransducerPair<B> = (left_mapping, right_mapping);
    let mapping_pair_vector: HfstTransducerPairVector<B> = vec![mapping_pair];

    let context: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("c", &tok)?,
        HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?,
    );
    let context_vector: HfstTransducerPairVector<B> = vec![context];

    let input1 = HfstTransducer::<B>::new_tokenized("caav", &tok)?;
    let result1 = HfstTransducer::<B>::new_tokenized_pair("caav", "cx@_EPSILON_SYMBOL_@v", &tok)?;

    let rule_up =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_UP)?;

    let replace_tr = xr::replace_leftmost_longest_match_rule(&rule_up)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result1, true)?);
    Ok(())
}

#[test]
fn test2c_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test2c::<StdVectorFst>()?;
    Ok(())
}

// test multiple contexts: a -> b ||  x _ x ;
// [spec:hfst:def:hfst-xerox-rules-test.test3a-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test3a-fn]
fn test3a<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_LM_@");
    tok.add_multichar_symbol("@_RM_@");

    let left_mapping = HfstTransducer::<B>::new_tokenized("a", &tok)?;
    let right_mapping = HfstTransducer::<B>::new_tokenized("b", &tok)?;

    let mapping_pair: HfstTransducerPair<B> = (left_mapping, right_mapping);
    let mapping_pair_vector: HfstTransducerPairVector<B> = vec![mapping_pair];

    let context: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("x", &tok)?,
        HfstTransducer::<B>::new_tokenized("x", &tok)?,
    );
    let context_vector: HfstTransducerPairVector<B> = vec![context];

    let input1 = HfstTransducer::<B>::new_tokenized("xaxax", &tok)?;

    let mut result1 = HfstTransducer::<B>::new_tokenized("xaxax", &tok)?;
    let r1tmp = HfstTransducer::<B>::new_tokenized_pair("xaxax", "xbxax", &tok)?;
    let r2tmp = HfstTransducer::<B>::new_tokenized_pair("xaxax", "xaxbx", &tok)?;
    let r3tmp = HfstTransducer::<B>::new_tokenized_pair("xaxax", "xbxbx", &tok)?;
    result1
        .disjunct(&r1tmp, true)?
        .disjunct(&r2tmp, true)?
        .disjunct(&r3tmp, true)?
        .minimize()?;

    let rule_up =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_UP)?;

    let replace_tr = xr::replace_rule(&rule_up, true)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result1, true)?);
    Ok(())
}

#[test]
fn test3a_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test3a::<StdVectorFst>()?;
    Ok(())
}

// test multiple contexts: a b -> b ||  x_y, y_z
// [spec:hfst:def:hfst-xerox-rules-test.test3b-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test3b-fn]
fn test3b<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_LM_@");
    tok.add_multichar_symbol("@_RM_@");

    let mut left_mapping = HfstTransducer::<B>::new_tokenized("a", &tok)?;
    left_mapping.repeat_plus()?.minimize()?;
    let right_mapping = HfstTransducer::<B>::new_tokenized("b", &tok)?;

    let mapping_pair: HfstTransducerPair<B> = (left_mapping, right_mapping);
    let mapping_pair_vector: HfstTransducerPairVector<B> = vec![mapping_pair];

    let context: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("x", &tok)?,
        HfstTransducer::<B>::new_tokenized("y", &tok)?,
    );
    let context2: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("y", &tok)?,
        HfstTransducer::<B>::new_tokenized("z", &tok)?,
    );
    let context_vector: HfstTransducerPairVector<B> = vec![context, context2];

    let input1 = HfstTransducer::<B>::new_tokenized("axayaz", &tok)?;

    let mut result1 = HfstTransducer::<B>::new_tokenized("axayaz", &tok)?;
    let r1tmp = HfstTransducer::<B>::new_tokenized_pair("axayaz", "axbybz", &tok)?;
    let r2tmp = HfstTransducer::<B>::new_tokenized_pair("axayaz", "axbyaz", &tok)?;
    let r3tmp = HfstTransducer::<B>::new_tokenized_pair("axayaz", "axaybz", &tok)?;
    result1
        .disjunct(&r1tmp, true)?
        .disjunct(&r2tmp, true)?
        .disjunct(&r3tmp, true)?
        .minimize()?;

    let rule_up =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_UP)?;

    let replace_tr = xr::replace_rule(&rule_up, true)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result1, true)?);
    Ok(())
}

#[test]
fn test3b_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test3b::<StdVectorFst>()?;
    Ok(())
}

// test multiple contexts: a+ -> x  || x x _ y y, y _ x
// [spec:hfst:def:hfst-xerox-rules-test.test3c-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test3c-fn]
fn test3c<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_LM_@");
    tok.add_multichar_symbol("@_RM_@");

    let mut left_mapping = HfstTransducer::<B>::new_tokenized("a", &tok)?;
    left_mapping.repeat_plus()?.minimize()?;
    let right_mapping = HfstTransducer::<B>::new_tokenized("x", &tok)?;

    let mapping_pair: HfstTransducerPair<B> = (left_mapping, right_mapping);
    let mapping_pair_vector: HfstTransducerPairVector<B> = vec![mapping_pair];

    let context: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("xx", &tok)?,
        HfstTransducer::<B>::new_tokenized("yy", &tok)?,
    );
    let context2: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("y", &tok)?,
        HfstTransducer::<B>::new_tokenized("x", &tok)?,
    );
    let context_vector: HfstTransducerPairVector<B> = vec![context, context2];

    let input1 = HfstTransducer::<B>::new_tokenized("axxayyax", &tok)?;

    let mut result1 = HfstTransducer::<B>::new_tokenized("axxayyax", &tok)?;
    let r1tmp = HfstTransducer::<B>::new_tokenized_pair("axxayyax", "axxayyxx", &tok)?;
    let r2tmp = HfstTransducer::<B>::new_tokenized_pair("axxayyax", "axxxyyax", &tok)?;
    let r3tmp = HfstTransducer::<B>::new_tokenized_pair("axxayyax", "axxxyyxx", &tok)?;
    result1
        .disjunct(&r1tmp, true)?
        .disjunct(&r2tmp, true)?
        .disjunct(&r3tmp, true)?
        .minimize()?;

    let rule_up =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_UP)?;

    let replace_tr = xr::replace_rule(&rule_up, true)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result1, true)?);
    Ok(())
}

#[test]
fn test3c_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test3c::<StdVectorFst>()?;
    Ok(())
}

// test multiple contexts: a -> b ;
// [spec:hfst:def:hfst-xerox-rules-test.test3d-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test3d-fn]
fn test3d<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_LM_@");
    tok.add_multichar_symbol("@_RM_@");

    let left_mapping = HfstTransducer::<B>::new_tokenized("a", &tok)?;
    let right_mapping = HfstTransducer::<B>::new_tokenized("b", &tok)?;

    let mapping_pair: HfstTransducerPair<B> = (left_mapping, right_mapping);
    let mapping_pair_vector: HfstTransducerPairVector<B> = vec![mapping_pair];

    let context: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?,
        HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?,
    );
    let context_vector: HfstTransducerPairVector<B> = vec![context];

    let input1 = HfstTransducer::<B>::new_tokenized("xaxax", &tok)?;

    let mut result1 = HfstTransducer::<B>::new_tokenized("xaxax", &tok)?;
    let r1tmp = HfstTransducer::<B>::new_tokenized_pair("xaxax", "xbxax", &tok)?;
    let r2tmp = HfstTransducer::<B>::new_tokenized_pair("xaxax", "xaxbx", &tok)?;
    let r3tmp = HfstTransducer::<B>::new_tokenized_pair("xaxax", "xbxbx", &tok)?;
    result1
        .disjunct(&r1tmp, true)?
        .disjunct(&r2tmp, true)?
        .disjunct(&r3tmp, true)?
        .minimize()?;

    let rule_up =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_UP)?;

    let replace_tr = xr::replace_rule(&rule_up, true)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result1, true)?);
    Ok(())
}

#[test]
fn test3d_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test3d::<StdVectorFst>()?;
    Ok(())
}

// b -> a  || _a ; input: bbba
// [spec:hfst:def:hfst-xerox-rules-test.test4a-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test4a-fn]
fn test4a<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_LM_@");
    tok.add_multichar_symbol("@_RM_@");

    let left_mapping = HfstTransducer::<B>::new_tokenized("b", &tok)?;
    let right_mapping = HfstTransducer::<B>::new_tokenized("a", &tok)?;

    let mapping_pair: HfstTransducerPair<B> = (left_mapping, right_mapping);
    let mapping_pair_vector: HfstTransducerPairVector<B> = vec![mapping_pair];

    let context: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?,
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
    );
    let context_vector: HfstTransducerPairVector<B> = vec![context];

    let input1 = HfstTransducer::<B>::new_tokenized("bbba", &tok)?;

    let result1 = HfstTransducer::<B>::new_tokenized_pair("bbba", "bbaa", &tok)?;
    let result2 = HfstTransducer::<B>::new_tokenized_pair("bbba", "aaaa", &tok)?;
    let r1_tmp = HfstTransducer::<B>::new_tokenized_pair("bbba", "baaa", &tok)?;
    let mut result3 = input1.clone();
    result3.disjunct(&result1, true)?.minimize()?;

    let mut result4 = result3.clone();
    result4
        .disjunct(&result2, true)?
        .minimize()?
        .disjunct(&r1_tmp, true)?
        .minimize()?;

    let rule_up =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_UP)?;
    let rule_left =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_LEFT)?;
    let rule_right =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_RIGHT)?;
    let rule_down =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_DOWN)?;

    // Unconditional optional replace
    let replace_tr_up = xr::replace_rule(&rule_up, true)?;
    let replace_tr_left = xr::replace_rule(&rule_left, true)?;
    let replace_tr_right = xr::replace_rule(&rule_right, true)?;
    let replace_tr_down = xr::replace_rule(&rule_down, true)?;

    assert!(compose_minimize(&input1, &replace_tr_up)?.compare(&result3, true)?);
    assert!(compose_minimize(&input1, &replace_tr_left)?.compare(&result4, true)?);
    assert!(compose_minimize(&input1, &replace_tr_right)?.compare(&result3, true)?);
    assert!(compose_minimize(&input1, &replace_tr_down)?.compare(&result4, true)?);

    // Non optional
    let replace_tr_up = xr::replace_rule(&rule_up, false)?;
    let replace_tr_left = xr::replace_rule(&rule_left, false)?;
    let replace_tr_right = xr::replace_rule(&rule_right, false)?;
    let replace_tr_down = xr::replace_rule(&rule_down, false)?;

    assert!(compose_minimize(&input1, &replace_tr_up)?.compare(&result1, true)?);
    assert!(compose_minimize(&input1, &replace_tr_left)?.compare(&result2, true)?);
    assert!(compose_minimize(&input1, &replace_tr_right)?.compare(&result1, true)?);
    assert!(compose_minimize(&input1, &replace_tr_down)?.compare(&result2, true)?);
    Ok(())
}

#[test]
fn test4a_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test4a::<StdVectorFst>()?;
    Ok(())
}

// b -> a  || a _ ; input: abbb
// [spec:hfst:def:hfst-xerox-rules-test.test4b-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test4b-fn]
fn test4b<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_LM_@");
    tok.add_multichar_symbol("@_RM_@");

    let left_mapping = HfstTransducer::<B>::new_tokenized("b", &tok)?;
    let right_mapping = HfstTransducer::<B>::new_tokenized("a", &tok)?;

    let mapping_pair: HfstTransducerPair<B> = (left_mapping, right_mapping);
    let mapping_pair_vector: HfstTransducerPairVector<B> = vec![mapping_pair];

    let context: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
        HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?,
    );
    let context_vector: HfstTransducerPairVector<B> = vec![context];

    let rule_up =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_UP)?;
    let rule_left =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_LEFT)?;
    let rule_right =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_RIGHT)?;
    let rule_down =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_DOWN)?;

    // Unconditional optional replace
    let replace_tr_up = xr::replace_rule(&rule_up, true)?;
    let replace_tr_left = xr::replace_rule(&rule_left, true)?;
    let replace_tr_right = xr::replace_rule(&rule_right, true)?;
    let replace_tr_down = xr::replace_rule(&rule_down, true)?;

    let input1 = HfstTransducer::<B>::new_tokenized("abbb", &tok)?;

    let result1 = HfstTransducer::<B>::new_tokenized_pair("abbb", "aabb", &tok)?;
    let result2 = HfstTransducer::<B>::new_tokenized_pair("abbb", "aaaa", &tok)?;
    let r1_tmp = HfstTransducer::<B>::new_tokenized_pair("abbb", "aaab", &tok)?;
    let mut result3 = input1.clone();
    result3.disjunct(&result1, true)?.minimize()?;

    let mut result4 = result3.clone();
    result4
        .disjunct(&r1_tmp, true)?
        .minimize()?
        .disjunct(&result2, true)?
        .minimize()?;

    assert!(compose_minimize(&input1, &replace_tr_up)?.compare(&result3, true)?);
    assert!(compose_minimize(&input1, &replace_tr_left)?.compare(&result3, true)?);
    assert!(compose_minimize(&input1, &replace_tr_right)?.compare(&result4, true)?);
    assert!(compose_minimize(&input1, &replace_tr_down)?.compare(&result4, true)?);

    // Non optional
    let replace_tr_up = xr::replace_rule(&rule_up, false)?;
    let replace_tr_left = xr::replace_rule(&rule_left, false)?;
    let replace_tr_right = xr::replace_rule(&rule_right, false)?;
    let replace_tr_down = xr::replace_rule(&rule_down, false)?;

    assert!(compose_minimize(&input1, &replace_tr_up)?.compare(&result1, true)?);
    assert!(compose_minimize(&input1, &replace_tr_left)?.compare(&result1, true)?);
    assert!(compose_minimize(&input1, &replace_tr_right)?.compare(&result2, true)?);
    assert!(compose_minimize(&input1, &replace_tr_down)?.compare(&result2, true)?);
    Ok(())
}

#[test]
fn test4b_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test4b::<StdVectorFst>()?;
    Ok(())
}

// ab -> x || ab _ a
// [spec:hfst:def:hfst-xerox-rules-test.test4c-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test4c-fn]
fn test4c<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let left_mapping = HfstTransducer::<B>::new_tokenized("ab", &tok)?;
    let right_mapping = HfstTransducer::<B>::new_tokenized("x", &tok)?;

    let mapping_pair: HfstTransducerPair<B> = (left_mapping, right_mapping);
    let mapping_pair_vector: HfstTransducerPairVector<B> = vec![mapping_pair];

    let context: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("ab", &tok)?,
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
    );
    let context_vector: HfstTransducerPairVector<B> = vec![context];

    let input1 = HfstTransducer::<B>::new_tokenized("abababa", &tok)?;

    let result1 = HfstTransducer::<B>::new_tokenized_pair("abababa", "abababa", &tok)?;
    let r2tmp =
        HfstTransducer::<B>::new_tokenized_pair("abababa", "ababx@_EPSILON_SYMBOL_@a", &tok)?;
    let r3tmp =
        HfstTransducer::<B>::new_tokenized_pair("abababa", "abx@_EPSILON_SYMBOL_@aba", &tok)?;
    let r4tmp = HfstTransducer::<B>::new_tokenized_pair(
        "abababa",
        "abx@_EPSILON_SYMBOL_@x@_EPSILON_SYMBOL_@a",
        &tok,
    )?;

    let mut result2 = result1.clone();
    result2
        .disjunct(&r2tmp, true)?
        .disjunct(&r3tmp, true)?
        .minimize()?;

    let mut result3 = result2.clone();
    result3.disjunct(&r4tmp, true)?.minimize()?;

    let mut result4 = r2tmp.clone();
    result4.disjunct(&r3tmp, true)?.minimize()?;

    let rule_up =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_UP)?;
    let rule_left =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_LEFT)?;
    let rule_right =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_RIGHT)?;
    let rule_down =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_DOWN)?;

    // Unconditional optional replace
    let replace_tr_up = xr::replace_rule(&rule_up, true)?;
    let replace_tr_left = xr::replace_rule(&rule_left, true)?;
    let replace_tr_right = xr::replace_rule(&rule_right, true)?;
    let replace_tr_down = xr::replace_rule(&rule_down, true)?;

    assert!(compose_minimize(&input1, &replace_tr_up)?.compare(&result3, true)?);
    assert!(compose_minimize(&input1, &replace_tr_left)?.compare(&result2, true)?);
    assert!(compose_minimize(&input1, &replace_tr_right)?.compare(&result2, true)?);
    assert!(compose_minimize(&input1, &replace_tr_down)?.compare(&result2, true)?);

    // non optional
    let replace_tr_up = xr::replace_rule(&rule_up, false)?;
    let replace_tr_left = xr::replace_rule(&rule_left, false)?;
    let replace_tr_right = xr::replace_rule(&rule_right, false)?;
    let replace_tr_down = xr::replace_rule(&rule_down, false)?;

    assert!(compose_minimize(&input1, &replace_tr_up)?.compare(&r4tmp, true)?);
    assert!(compose_minimize(&input1, &replace_tr_left)?.compare(&r2tmp, true)?);
    assert!(compose_minimize(&input1, &replace_tr_right)?.compare(&r3tmp, true)?);
    assert!(compose_minimize(&input1, &replace_tr_down)?.compare(&result4, true)?);
    Ok(())
}

#[test]
fn test4c_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test4c::<StdVectorFst>()?;
    Ok(())
}

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

// a -> b , b -> c
// [spec:hfst:def:hfst-xerox-rules-test.test7a-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test7a-fn]
fn test7a<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let mapping_pair1: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
        HfstTransducer::<B>::new_tokenized("b", &tok)?,
    );
    let mapping_pair2: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("b", &tok)?,
        HfstTransducer::<B>::new_tokenized("c", &tok)?,
    );

    let mapping_pair_vector1: HfstTransducerPairVector<B> = vec![mapping_pair1];
    let mapping_pair_vector2: HfstTransducerPairVector<B> = vec![mapping_pair2];

    let rule1 = Rule::new_mapping(&mapping_pair_vector1)?;
    let rule2 = Rule::new_mapping(&mapping_pair_vector2)?;

    let rule_vector: Vec<Rule<B>> = vec![rule1, rule2];

    let input1 = HfstTransducer::<B>::new_tokenized("aab", &tok)?;
    let result1 = HfstTransducer::<B>::new_tokenized_pair("aab", "bbc", &tok)?;

    let replace_tr = xr::replace_rule_vector(&rule_vector, false)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result1, true)?);
    Ok(())
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
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let mapping_pair1: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("@_EPSILON_SYMBOL_@", &tok)?,
        HfstTransducer::<B>::new_tokenized("b", &tok)?,
    );
    let mapping_pair2: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
        HfstTransducer::<B>::new_tokenized("c", &tok)?,
    );

    let mapping_pair_vector1: HfstTransducerPairVector<B> = vec![mapping_pair1];
    let mapping_pair_vector2: HfstTransducerPairVector<B> = vec![mapping_pair2];

    let rule1 = Rule::new_mapping(&mapping_pair_vector1)?;
    let rule2 = Rule::new_mapping(&mapping_pair_vector2)?;

    let rule_vector: Vec<Rule<B>> = vec![rule1, rule2];

    let input1 = HfstTransducer::<B>::new_tokenized("a", &tok)?;
    let result1 = HfstTransducer::<B>::new_tokenized_pair(
        "@_EPSILON_SYMBOL_@a@_EPSILON_SYMBOL_@",
        "bcb",
        &tok,
    )?;

    let replace_tr = xr::replace_rule_vector(&rule_vector, false)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result1, true)?);
    Ok(())
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
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let mapping_pair1: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
        HfstTransducer::<B>::new_tokenized("b", &tok)?,
    );
    let mapping_pair2: HfstTransducerPair<B> = (
        HfstTransducer::<B>::new_tokenized("b", &tok)?,
        HfstTransducer::<B>::new_tokenized("a", &tok)?,
    );

    let mapping_pair_vector1: HfstTransducerPairVector<B> = vec![mapping_pair1];
    let mapping_pair_vector2: HfstTransducerPairVector<B> = vec![mapping_pair2];

    let rule1 = Rule::new_mapping(&mapping_pair_vector1)?;
    let rule2 = Rule::new_mapping(&mapping_pair_vector2)?;

    let rule_vector: Vec<Rule<B>> = vec![rule1, rule2];

    let input1 = HfstTransducer::<B>::new_tokenized("aabbaa", &tok)?;
    let result1 = HfstTransducer::<B>::new_tokenized_pair("aabbaa", "bbaabb", &tok)?;

    let replace_tr = xr::replace_rule_vector(&rule_vector, false)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result1, true)?);
    Ok(())
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
