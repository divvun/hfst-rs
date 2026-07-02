// Port of libhfst/src/HfstXeroxRulesTest.cc
//
// Tests the hfst::xeroxRules namespace: the Rule data type plus the replace /
// replace_left / replace_leftmost_longest_match / replace_leftmost_shortest_match
// (and rightmost) functions, the restriction rule, and the before/after rules.
//
// The C++ main loops over several backends. Per the Wave-2 port scope only
// TROPICAL_OPENFST_TYPE is exercised here: LOG_OPENFST is weak in this port and
// SFST / FOMA / XFSM are out of scope (is_implementation_type_available returns
// false for them in this build). Each C++ void testX(ImplementationType) becomes
// a Rust helper fn taking the type, plus a #[test] wrapper that runs it for
// TROPICAL. The C++ MAIN_TEST driver is not ported; the wrappers are the driver.
//
// The C++ replace(rule, bool) / replace_left / replace_leftmost_longest_match etc.
// are renamed in this Rust port: replace_rule, replace_left_rule,
// replace_leftmost_longest_match_rule, and so on, plus _rule_vector variants for
// the std::vector<Rule> overloads. Those renames are applied below.
//
// C++ assert(a.compare(b)) defaults harmonize=true and is mirrored as
// assert!(a.compare(&b, true)). C++ comments marked FAIL flag asserts the original
// author knew were failing; they are still ported faithfully.

use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_data_types::ImplementationType::{self, TROPICAL_OPENFST_TYPE};
use hfst::hfst_tokenizer::HfstTokenizer;
use hfst::hfst_transducer::{HfstTransducer, HfstTransducerPair, HfstTransducerPairVector};
use hfst::hfst_xerox_rules as xr;
use hfst::hfst_xerox_rules::ReplaceType::{REPL_DOWN, REPL_LEFT, REPL_RIGHT, REPL_UP};
use hfst::hfst_xerox_rules::Rule;

// The tropical/log transition-data symbol coding lives in process-global statics
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
fn compose_minimize(
    left: &HfstTransducer,
    right: &HfstTransducer,
) -> Result<HfstTransducer, hfst::error::Error> {
    let mut t = left.clone();
    t.compose(right, true)?.minimize()?;
    Ok(t)
}

// a -> b || ? - a _
// [spec:hfst:def:hfst-xerox-rules-test.test8-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test8-fn]
fn test8(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let tok = HfstTokenizer::new();
    let a = HfstTransducer::new_tokenized("a", &tok, type_)?;
    let _b = HfstTransducer::new_tokenized("b", &tok, type_)?;

    let identity_pair = HfstTransducer::identity_pair(type_);

    let mut left_mapping = identity_pair.clone();
    left_mapping.subtract(&a, true)?;

    let mapping_pair: HfstTransducerPair = (left_mapping, HfstTransducer::new_type(type_)?);
    let mut mapping_pair_vector: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector.push(mapping_pair);

    let rule = Rule::new_mapping(&mapping_pair_vector)?;

    let input1 = HfstTransducer::new_tokenized("maa", &tok, type_)?;
    let result1 = HfstTransducer::new_tokenized("mba", &tok, type_)?;

    let replace_tr = xr::replace_rule(&rule, false)?;

    let tmp = compose_minimize(&input1, &replace_tr)?;
    assert!(tmp.compare(&result1, true)?);
    Ok(())
}

#[test]
#[ignore = "PORT DISCREPANCY: the (identity-a, HfstTransducer(type)) mapping cross-products to the EMPTY mapping in both C++ and Rust (second element is the empty language), so the Rust replace yields the identity transducer and 'maa' -> 'maa'. The C++ test asserts 'maa' -> 'mba', which cannot follow from this rule (no 'b' appears anywhere; the mapping is empty) -- a degenerate/quirky upstream test like after_test1. Matching C++ would require replicating a C++-specific empty-mapping replace artifact; needs a live C++ HFST to diff against."]
fn test8_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test8(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// a < b ;
// [spec:hfst:def:hfst-xerox-rules-test.before-test1-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.before-test1-fn]
fn before_test1(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let tok = HfstTokenizer::new();
    let left = HfstTransducer::new_tokenized("a", &tok, type_)?;
    let right = HfstTransducer::new_tokenized("b", &tok, type_)?;

    let input1 = HfstTransducer::new_tokenized("ab", &tok, type_)?;
    let input2 = HfstTransducer::new_tokenized("acb", &tok, type_)?;
    let input3 = HfstTransducer::new_tokenized("ba", &tok, type_)?;
    let input4 = HfstTransducer::new_tokenized("bca", &tok, type_)?;
    let empty = HfstTransducer::new_type(type_)?;

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
    before_test1(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// a < b ;
// [spec:hfst:def:hfst-xerox-rules-test.after-test1-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.after-test1-fn]
fn after_test1(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let tok = HfstTokenizer::new();
    let left = HfstTransducer::new_tokenized("a", &tok, type_)?;
    let right = HfstTransducer::new_tokenized("b", &tok, type_)?;

    let input1 = HfstTransducer::new_tokenized("ba", &tok, type_)?;
    let input2 = HfstTransducer::new_tokenized("bca", &tok, type_)?;
    let input3 = HfstTransducer::new_tokenized("ab", &tok, type_)?;
    let input4 = HfstTransducer::new_tokenized("acb", &tok, type_)?;
    let empty = HfstTransducer::new_type(type_)?;

    // C++ after_test1 uses before(left, right) as well.
    let after_tr = xr::before(&left, &right)?;

    assert!(compose_minimize(&input1, &after_tr)?.compare(&input1, true)?);
    assert!(compose_minimize(&input2, &after_tr)?.compare(&input2, true)?);
    assert!(compose_minimize(&input3, &after_tr)?.compare(&empty, true)?);
    assert!(compose_minimize(&input4, &after_tr)?.compare(&empty, true)?);
    Ok(())
}

#[test]
#[ignore = "PORT DISCREPANCY: ported C++ after_test1 calls before(left,right) (as in the upstream source) with expectations opposite to before_test1, which passes here; the same before() transducer cannot satisfy both, so this self-contradictory upstream test fails"]
fn after_test1_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    after_test1(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// restriction rule a => b _ c ;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test1-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test1-fn]
fn restriction_test1(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let center = HfstTransducer::new_tokenized("a", &tok, type_)?;

    let context: HfstTransducerPair = (
        HfstTransducer::new_tokenized("b", &tok, type_)?,
        HfstTransducer::new_tokenized("c", &tok, type_)?,
    );
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context);

    let input1 = HfstTransducer::new_tokenized("bac", &tok, type_)?;
    let input2 = HfstTransducer::new_tokenized("abc", &tok, type_)?;
    let input3 = HfstTransducer::new_tokenized("abac", &tok, type_)?;
    let input4 = HfstTransducer::new_tokenized("bcab", &tok, type_)?;
    let result1 = HfstTransducer::new_tokenized("bac", &tok, type_)?;
    let empty = HfstTransducer::new_type(type_)?;

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
    restriction_test1(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// restriction rule a => b k _ c ;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test1a-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test1a-fn]
fn restriction_test1a(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let center = HfstTransducer::new_tokenized("a", &tok, type_)?;

    let context: HfstTransducerPair = (
        HfstTransducer::new_tokenized("bk", &tok, type_)?,
        HfstTransducer::new_tokenized("c", &tok, type_)?,
    );
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context);

    let input1 = HfstTransducer::new_tokenized("bkac", &tok, type_)?;
    let input2 = HfstTransducer::new_tokenized("abkc", &tok, type_)?;
    let input3 = HfstTransducer::new_tokenized("abkac", &tok, type_)?;
    let input4 = HfstTransducer::new_tokenized("bkcabk", &tok, type_)?;
    let result1 = HfstTransducer::new_tokenized("bkac", &tok, type_)?;
    let empty = HfstTransducer::new_type(type_)?;

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
    restriction_test1a(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// restriction rule a => bb _ bb ;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test1b-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test1b-fn]
fn restriction_test1b(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let center = HfstTransducer::new_tokenized("a", &tok, type_)?;

    let context: HfstTransducerPair = (
        HfstTransducer::new_tokenized("bb", &tok, type_)?,
        HfstTransducer::new_tokenized("bb", &tok, type_)?,
    );
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context);

    let input1 = HfstTransducer::new_tokenized("bbabb", &tok, type_)?;
    let input2 = HfstTransducer::new_tokenized("abb", &tok, type_)?;
    let input3 = HfstTransducer::new_tokenized("abbabb", &tok, type_)?;
    let input4 = HfstTransducer::new_tokenized("bbbbab", &tok, type_)?;
    let result1 = HfstTransducer::new_tokenized("bbabb", &tok, type_)?;
    let empty = HfstTransducer::new_type(type_)?;

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
    restriction_test1b(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// restriction rule a k => b _ c ;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test2-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test2-fn]
fn restriction_test2(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let center = HfstTransducer::new_tokenized("ak", &tok, type_)?;

    let context: HfstTransducerPair = (
        HfstTransducer::new_tokenized("b", &tok, type_)?,
        HfstTransducer::new_tokenized("c", &tok, type_)?,
    );
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context);

    let input1 = HfstTransducer::new_tokenized("bakc", &tok, type_)?;
    let input2 = HfstTransducer::new_tokenized("akbc", &tok, type_)?;
    let input3 = HfstTransducer::new_tokenized("akbakc", &tok, type_)?;
    let input4 = HfstTransducer::new_tokenized("bcak", &tok, type_)?;
    let result1 = HfstTransducer::new_tokenized("bakc", &tok, type_)?;
    let empty = HfstTransducer::new_type(type_)?;

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
    restriction_test2(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// restriction rule a b => b _ c ;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test3-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test3-fn]
fn restriction_test3(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let center = HfstTransducer::new_tokenized("b", &tok, type_)?;

    let context: HfstTransducerPair = (
        HfstTransducer::new_tokenized("b", &tok, type_)?,
        HfstTransducer::new_tokenized("c", &tok, type_)?,
    );
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context);

    let input1 = HfstTransducer::new_tokenized("c", &tok, type_)?;
    let input2 = HfstTransducer::new_tokenized("bc", &tok, type_)?;
    let input3 = HfstTransducer::new_tokenized("bbc", &tok, type_)?;
    let input4 = HfstTransducer::new_tokenized("cb", &tok, type_)?;
    let result1 = HfstTransducer::new_tokenized("c", &tok, type_)?;
    let empty = HfstTransducer::new_type(type_)?;

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
    restriction_test3(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// restriction rule a => a _ ;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test3a-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test3a-fn]
fn restriction_test3a(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let center = HfstTransducer::new_tokenized("a", &tok, type_)?;
    let epsilon = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?;

    let context: HfstTransducerPair = (HfstTransducer::new_tokenized("a", &tok, type_)?, epsilon);
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context);

    let input1 = HfstTransducer::new_tokenized("c", &tok, type_)?;
    let input2 = HfstTransducer::new_tokenized("aa", &tok, type_)?;
    let input3 = HfstTransducer::new_tokenized("a", &tok, type_)?;
    let input4 = HfstTransducer::new_tokenized("aca", &tok, type_)?;
    let result1 = HfstTransducer::new_tokenized("c", &tok, type_)?;
    let empty = HfstTransducer::new_type(type_)?;

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
    restriction_test3a(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// restriction rule a b => a b _ ;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test3b-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test3b-fn]
fn restriction_test3b(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let center = HfstTransducer::new_tokenized("ab", &tok, type_)?;
    let epsilon = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?;

    let context: HfstTransducerPair = (HfstTransducer::new_tokenized("ab", &tok, type_)?, epsilon);
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context);

    let input1 = HfstTransducer::new_tokenized("ba", &tok, type_)?;
    let input2 = HfstTransducer::new_tokenized("ab", &tok, type_)?;
    let input3 = HfstTransducer::new_tokenized("abab", &tok, type_)?;
    let input4 = HfstTransducer::new_tokenized("abc", &tok, type_)?;
    let result1 = HfstTransducer::new_tokenized("ba", &tok, type_)?;
    let empty = HfstTransducer::new_type(type_)?;

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
    restriction_test3b(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// restriction rule a b => _ a b;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test3c-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test3c-fn]
fn restriction_test3c(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let center = HfstTransducer::new_tokenized("ab", &tok, type_)?;
    let epsilon = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?;

    let context: HfstTransducerPair = (epsilon, HfstTransducer::new_tokenized("ab", &tok, type_)?);
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context);

    let input1 = HfstTransducer::new_tokenized("ba", &tok, type_)?;
    let input2 = HfstTransducer::new_tokenized("ab", &tok, type_)?;
    let input3 = HfstTransducer::new_tokenized("abab", &tok, type_)?;
    let input4 = HfstTransducer::new_tokenized("abc", &tok, type_)?;
    let result1 = HfstTransducer::new_tokenized("ba", &tok, type_)?;
    let empty = HfstTransducer::new_type(type_)?;

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
    restriction_test3c(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// restriction rule a => b _ c , j _ k ;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test4-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test4-fn]
fn restriction_test4(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let center = HfstTransducer::new_tokenized("a", &tok, type_)?;

    let context1: HfstTransducerPair = (
        HfstTransducer::new_tokenized("b", &tok, type_)?,
        HfstTransducer::new_tokenized("c", &tok, type_)?,
    );
    let context2: HfstTransducerPair = (
        HfstTransducer::new_tokenized("j", &tok, type_)?,
        HfstTransducer::new_tokenized("k", &tok, type_)?,
    );
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context1);
    context_vector.push(context2);

    let input1 = HfstTransducer::new_tokenized("bac", &tok, type_)?;
    let input2 = HfstTransducer::new_tokenized("jak", &tok, type_)?;
    let input3 = HfstTransducer::new_tokenized("bacjak", &tok, type_)?;
    let input4 = HfstTransducer::new_tokenized("bajc", &tok, type_)?;

    let result1 = HfstTransducer::new_tokenized("bac", &tok, type_)?;
    let result2 = HfstTransducer::new_tokenized("jak", &tok, type_)?;
    let result3 = HfstTransducer::new_tokenized("bacjak", &tok, type_)?;
    let empty = HfstTransducer::new_type(type_)?;

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
    restriction_test4(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// restriction rule a => b _ , _ c;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test5-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test5-fn]
fn restriction_test5(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let center = HfstTransducer::new_tokenized("a", &tok, type_)?;
    let epsilon = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?;

    let context1: HfstTransducerPair = (
        HfstTransducer::new_tokenized("b", &tok, type_)?,
        epsilon.clone(),
    );
    let context2: HfstTransducerPair = (epsilon, HfstTransducer::new_tokenized("c", &tok, type_)?);
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context1);
    context_vector.push(context2);

    let input1 = HfstTransducer::new_tokenized("bac", &tok, type_)?;
    let input2 = HfstTransducer::new_tokenized("ba", &tok, type_)?;
    let input3 = HfstTransducer::new_tokenized("ac", &tok, type_)?;
    let input4 = HfstTransducer::new_tokenized("abac", &tok, type_)?;

    let result1 = HfstTransducer::new_tokenized("bac", &tok, type_)?;
    let result2 = HfstTransducer::new_tokenized("ba", &tok, type_)?;
    let result3 = HfstTransducer::new_tokenized("ac", &tok, type_)?;
    let empty = HfstTransducer::new_type(type_)?;

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
    restriction_test5(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// restriction rule a => a _ , _ a;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test5a-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test5a-fn]
fn restriction_test5a(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let center = HfstTransducer::new_tokenized("a", &tok, type_)?;
    let epsilon = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?;

    let context1: HfstTransducerPair = (
        HfstTransducer::new_tokenized("a", &tok, type_)?,
        epsilon.clone(),
    );
    let context2: HfstTransducerPair = (epsilon, HfstTransducer::new_tokenized("a", &tok, type_)?);
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context1);
    context_vector.push(context2);

    let input1 = HfstTransducer::new_tokenized("aa", &tok, type_)?;
    let input2 = HfstTransducer::new_tokenized("aaa", &tok, type_)?;
    let input3 = HfstTransducer::new_tokenized("ba", &tok, type_)?;
    let input4 = HfstTransducer::new_tokenized("cac", &tok, type_)?;

    let result1 = HfstTransducer::new_tokenized("aa", &tok, type_)?;
    let result2 = HfstTransducer::new_tokenized("aaa", &tok, type_)?;
    let empty = HfstTransducer::new_type(type_)?;

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
    restriction_test5a(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// restriction rule a b => a b _ , _ a b ;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test6-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test6-fn]
fn restriction_test6(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let center = HfstTransducer::new_tokenized("ab", &tok, type_)?;
    let epsilon = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?;

    let context1: HfstTransducerPair = (
        HfstTransducer::new_tokenized("ab", &tok, type_)?,
        epsilon.clone(),
    );
    let context2: HfstTransducerPair = (epsilon, HfstTransducer::new_tokenized("ab", &tok, type_)?);
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context1);
    context_vector.push(context2);

    let input1 = HfstTransducer::new_tokenized("abab", &tok, type_)?;
    let input2 = HfstTransducer::new_tokenized("ab", &tok, type_)?;
    let input3 = HfstTransducer::new_tokenized("aba", &tok, type_)?;
    let input4 = HfstTransducer::new_tokenized("ababab", &tok, type_)?;
    let result1 = HfstTransducer::new_tokenized("abab", &tok, type_)?;
    let empty = HfstTransducer::new_type(type_)?;

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
    restriction_test6(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// restriction rule [ x ?* y ] | [ z ?* v ] => b _ c ;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test7-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test7-fn]
fn restriction_test7(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    // Identity (normal)
    let identity_pair = HfstTransducer::identity_pair(type_);
    let mut identity = identity_pair.clone();
    identity.repeat_star()?.minimize()?;

    let x = HfstTransducer::new_tokenized("x", &tok, type_)?;
    let y = HfstTransducer::new_tokenized("y", &tok, type_)?;
    let z = HfstTransducer::new_tokenized("z", &tok, type_)?;
    let v = HfstTransducer::new_tokenized("v", &tok, type_)?;
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

    let context1: HfstTransducerPair = (
        HfstTransducer::new_tokenized("b", &tok, type_)?,
        HfstTransducer::new_tokenized("c", &tok, type_)?,
    );
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context1);

    let input1 = HfstTransducer::new_tokenized("bxbzycvc", &tok, type_)?;
    let input2 = HfstTransducer::new_tokenized("xy", &tok, type_)?;
    let input3 = HfstTransducer::new_tokenized("zv", &tok, type_)?;
    let input4 = HfstTransducer::new_tokenized("bxyzvc", &tok, type_)?;
    let result1 = HfstTransducer::new_tokenized("bxbzycvc", &tok, type_)?;
    let empty = HfstTransducer::new_type(type_)?;

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
    restriction_test7(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// restriction rule [ x y | x x y y ] => a _ b, x _ y ;
// [spec:hfst:def:hfst-xerox-rules-test.restriction-test8-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.restriction-test8-fn]
fn restriction_test8(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    let tmp = HfstTransducer::new_tokenized("xxyy", &tok, type_)?;
    let mut center = HfstTransducer::new_tokenized("xy", &tok, type_)?;
    center.disjunct(&tmp, true)?.minimize()?;

    let context1: HfstTransducerPair = (
        HfstTransducer::new_tokenized("a", &tok, type_)?,
        HfstTransducer::new_tokenized("b", &tok, type_)?,
    );
    let context2: HfstTransducerPair = (
        HfstTransducer::new_tokenized("x", &tok, type_)?,
        HfstTransducer::new_tokenized("y", &tok, type_)?,
    );
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context1);
    context_vector.push(context2);

    let input1 = HfstTransducer::new_tokenized("axxyyb", &tok, type_)?;
    let input2 = HfstTransducer::new_tokenized("xxyy", &tok, type_)?;
    let input3 = HfstTransducer::new_tokenized("xy", &tok, type_)?;
    let input4 = HfstTransducer::new_tokenized("xxxyyy", &tok, type_)?;
    let result1 = HfstTransducer::new_tokenized("axxyyb", &tok, type_)?;
    let empty = HfstTransducer::new_type(type_)?;

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
    restriction_test8(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// empty language replacements
// a -> ~[?*]
// [spec:hfst:def:hfst-xerox-rules-test.test10a-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test10a-fn]
fn test10a(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let tok = HfstTokenizer::new();

    let mapping_pair: HfstTransducerPair = (
        HfstTransducer::new_tokenized("a", &tok, type_)?,
        HfstTransducer::new_type(type_)?,
    );
    let mut mapping_pair_vector: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector.push(mapping_pair);

    let rule = Rule::new_mapping(&mapping_pair_vector)?;

    let identity_pair = HfstTransducer::identity_pair(type_);
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
    test10a(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// empty language replacements
// ~[?*] -> a
// [spec:hfst:def:hfst-xerox-rules-test.test10b-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test10b-fn]
fn test10b(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let tok = HfstTokenizer::new();

    let mapping_pair: HfstTransducerPair = (
        HfstTransducer::new_type(type_)?,
        HfstTransducer::new_tokenized("a", &tok, type_)?,
    );
    let mut mapping_pair_vector: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector.push(mapping_pair);

    let rule = Rule::new_mapping(&mapping_pair_vector)?;

    let identity_pair = HfstTransducer::identity_pair(type_);
    let mut result1 = identity_pair.clone();
    result1.repeat_star()?.minimize()?;

    let replace_tr = xr::replace_rule(&rule, false)?;

    assert!(replace_tr.compare(&result1, true)?);
    Ok(())
}

#[test]
fn test10b_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test10b(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// replace left d <- ca || ca_c  ( input: c a c a c a c )
// [spec:hfst:def:hfst-xerox-rules-test.test9a-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test9a-fn]
fn test9a(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let mapping_pair: HfstTransducerPair = (
        HfstTransducer::new_tokenized("d@_EPSILON_SYMBOL_@", &tok, type_)?,
        HfstTransducer::new_tokenized("ca", &tok, type_)?,
    );
    let mut mapping_pair_vector: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector.push(mapping_pair);

    let context: HfstTransducerPair = (
        HfstTransducer::new_tokenized("ca", &tok, type_)?,
        HfstTransducer::new_tokenized("c", &tok, type_)?,
    );
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context);

    let rule = Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_UP)?;

    let input1 = HfstTransducer::new_tokenized("cacacac", &tok, type_)?;
    let result1 = HfstTransducer::new_tokenized_pair(
        "cad@_EPSILON_SYMBOL_@d@_EPSILON_SYMBOL_@c",
        "cacacac",
        &tok,
        type_,
    )?;

    let replace_tr = xr::replace_left_rule(&rule, false)?;

    let tmp2 = compose_minimize(&replace_tr, &input1)?;
    assert!(tmp2.compare(&result1, true)?);
    Ok(())
}

#[test]
fn test9a_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test9a(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// replace left b <- a ,, a <- b
// [spec:hfst:def:hfst-xerox-rules-test.test9b-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test9b-fn]
fn test9b(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let mapping_pair1: HfstTransducerPair = (
        HfstTransducer::new_tokenized("b", &tok, type_)?,
        HfstTransducer::new_tokenized("a", &tok, type_)?,
    );
    let mut mapping_pair_vector1: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector1.push(mapping_pair1);

    let mapping_pair2: HfstTransducerPair = (
        HfstTransducer::new_tokenized("a", &tok, type_)?,
        HfstTransducer::new_tokenized("b", &tok, type_)?,
    );
    let mut mapping_pair_vector2: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector2.push(mapping_pair2);

    let rule1 = Rule::new_mapping(&mapping_pair_vector1)?;
    let rule2 = Rule::new_mapping(&mapping_pair_vector2)?;

    let mut rule_vector: Vec<Rule> = Vec::new();
    rule_vector.push(rule1);
    rule_vector.push(rule2);

    let input1 = HfstTransducer::new_tokenized("abba", &tok, type_)?;
    let result1 = HfstTransducer::new_tokenized_pair("baab", "abba", &tok, type_)?;

    let replace_tr = xr::replace_left_rule_vector(&rule_vector, false)?;

    let tmp2 = compose_minimize(&replace_tr, &input1)?;
    assert!(tmp2.compare(&result1, true)?);
    Ok(())
}

#[test]
fn test9b_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test9b(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// ab->x  ab_a
// [spec:hfst:def:hfst-xerox-rules-test.test1-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test1-fn]
fn test1(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let left_mapping = HfstTransducer::new_tokenized("ab", &tok, type_)?;
    let right_mapping = HfstTransducer::new_tokenized("x", &tok, type_)?;
    let mapping_pair: HfstTransducerPair = (left_mapping, right_mapping);
    let mut mapping_pair_vector: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector.push(mapping_pair);

    let context: HfstTransducerPair = (
        HfstTransducer::new_tokenized("ab", &tok, type_)?,
        HfstTransducer::new_tokenized("a", &tok, type_)?,
    );
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context);

    let input1 = HfstTransducer::new_tokenized("abababa", &tok, type_)?;

    let mut result1 = HfstTransducer::new_tokenized("abababa", &tok, type_)?;
    let r1tmp =
        HfstTransducer::new_tokenized_pair("abababa", "abx@_EPSILON_SYMBOL_@aba", &tok, type_)?;
    let r2tmp =
        HfstTransducer::new_tokenized_pair("abababa", "ababx@_EPSILON_SYMBOL_@a", &tok, type_)?;
    let r3tmp = HfstTransducer::new_tokenized_pair(
        "abababa",
        "abx@_EPSILON_SYMBOL_@x@_EPSILON_SYMBOL_@a",
        &tok,
        type_,
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
    test1(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// a -> x
// [spec:hfst:def:hfst-xerox-rules-test.test1b-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test1b-fn]
fn test1b(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let left_mapping = HfstTransducer::new_tokenized("a", &tok, type_)?;
    let right_mapping = HfstTransducer::new_tokenized("x", &tok, type_)?;
    let mapping_pair: HfstTransducerPair = (left_mapping, right_mapping);
    let mut mapping_pair_vector: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector.push(mapping_pair);

    let context: HfstTransducerPair = (
        HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?,
        HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?,
    );
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context);

    let input1 = HfstTransducer::new_tokenized("aaana", &tok, type_)?;

    let mut bt = HfstBasicTransducer::new();
    let tr =
        HfstBasicTransition::new_symbols(1, "a".to_string(), "a".to_string(), 0.0, bt.coder_mut());
    bt.add_transition(0, &tr, true);
    let tr =
        HfstBasicTransition::new_symbols(1, "a".to_string(), "x".to_string(), 0.0, bt.coder_mut());
    bt.add_transition(0, &tr, true);
    let tr =
        HfstBasicTransition::new_symbols(2, "a".to_string(), "a".to_string(), 0.0, bt.coder_mut());
    bt.add_transition(1, &tr, true);
    let tr =
        HfstBasicTransition::new_symbols(2, "a".to_string(), "x".to_string(), 0.0, bt.coder_mut());
    bt.add_transition(1, &tr, true);
    let tr =
        HfstBasicTransition::new_symbols(3, "a".to_string(), "a".to_string(), 0.0, bt.coder_mut());
    bt.add_transition(2, &tr, true);
    let tr =
        HfstBasicTransition::new_symbols(3, "a".to_string(), "x".to_string(), 0.0, bt.coder_mut());
    bt.add_transition(2, &tr, true);
    let tr =
        HfstBasicTransition::new_symbols(4, "n".to_string(), "n".to_string(), 0.0, bt.coder_mut());
    bt.add_transition(3, &tr, true);
    let tr =
        HfstBasicTransition::new_symbols(5, "a".to_string(), "a".to_string(), 0.0, bt.coder_mut());
    bt.add_transition(4, &tr, true);
    let tr =
        HfstBasicTransition::new_symbols(5, "a".to_string(), "x".to_string(), 0.0, bt.coder_mut());
    bt.add_transition(4, &tr, true);
    bt.set_final_weight(5, &0.0);

    let result1 = HfstTransducer::new_from_basic(&bt, type_)?;
    let result2 = HfstTransducer::new_tokenized_pair("aaana", "xxxnx", &tok, type_)?;

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
    test1b(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// ? -> x
// [spec:hfst:def:hfst-xerox-rules-test.test1c-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test1c-fn]
fn test1c(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_UNKNOWN_SYMBOL_@");
    tok.add_multichar_symbol("@_IDENTITY_SYMBOL_@");

    let left_mapping = HfstTransducer::new_tokenized("@_IDENTITY_SYMBOL_@", &tok, type_)?;
    let right_mapping = HfstTransducer::new_tokenized("x", &tok, type_)?;
    let mapping_pair: HfstTransducerPair = (left_mapping, right_mapping);
    let mut mapping_pair_vector: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector.push(mapping_pair);

    let input1 = HfstTransducer::new_tokenized("s", &tok, type_)?;
    let result1 = HfstTransducer::new_tokenized_pair("s", "x", &tok, type_)?;

    let rule = Rule::new_mapping(&mapping_pair_vector)?;

    let replace_tr = xr::replace_rule(&rule, false)?;

    let tmp2 = compose_minimize(&input1, &replace_tr)?;
    assert!(tmp2.compare(&result1, true)?);
    Ok(())
}

#[test]
fn test1c_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test1c(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// a -> b || .#. _ c;
// [spec:hfst:def:hfst-xerox-rules-test.test1d-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test1d-fn]
fn test1d(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol(".#.");

    let left_mapping = HfstTransducer::new_tokenized("a", &tok, type_)?;
    let right_mapping = HfstTransducer::new_tokenized("b", &tok, type_)?;
    let mapping_pair: HfstTransducerPair = (left_mapping, right_mapping);
    let mut mapping_pair_vector: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector.push(mapping_pair);

    let input1 = HfstTransducer::new_tokenized(".#.ac", &tok, type_)?;
    let input2 = HfstTransducer::new_tokenized("ac", &tok, type_)?;
    let result1 = HfstTransducer::new_tokenized_pair(".#.ac", ".#.ac", &tok, type_)?;
    let result2 = HfstTransducer::new_tokenized_pair("ac", "bc", &tok, type_)?;

    let context: HfstTransducerPair = (
        HfstTransducer::new_tokenized(".#.", &tok, type_)?,
        HfstTransducer::new_tokenized("c", &tok, type_)?,
    );
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context);

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
    test1d(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// a+ @-> x || a _ a
// [spec:hfst:def:hfst-xerox-rules-test.test2a-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test2a-fn]
fn test2a(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_UNKNOWN_SYMBOL_@");
    tok.add_multichar_symbol("@_IDENTITY_SYMBOL_@");
    tok.add_multichar_symbol("@_LM_@");
    tok.add_multichar_symbol("@_RM_@");

    let mut left_mapping = HfstTransducer::new_tokenized("a", &tok, type_)?;
    left_mapping.repeat_plus()?.minimize()?;
    let right_mapping = HfstTransducer::new_tokenized("x", &tok, type_)?;

    let mapping_pair: HfstTransducerPair = (left_mapping, right_mapping);
    let mut mapping_pair_vector: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector.push(mapping_pair);

    let context: HfstTransducerPair = (
        HfstTransducer::new_tokenized("a", &tok, type_)?,
        HfstTransducer::new_tokenized("a", &tok, type_)?,
    );
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context);

    let input1 = HfstTransducer::new_tokenized("aaaa", &tok, type_)?;
    let input2 = HfstTransducer::new_tokenized("aaaaabaaaa", &tok, type_)?;
    let input3 = HfstTransducer::new_tokenized("aaaaabaaaacaaaa", &tok, type_)?;

    let mut result1 = HfstTransducer::new_tokenized("aaaa", &tok, type_)?;
    let r1tmp = HfstTransducer::new_tokenized_pair("aaaa", "ax@_EPSILON_SYMBOL_@a", &tok, type_)?;
    let r2tmp = HfstTransducer::new_tokenized_pair("aaaa", "axaa", &tok, type_)?;
    let r3tmp = HfstTransducer::new_tokenized_pair("aaaa", "aaxa", &tok, type_)?;
    let r4tmp = HfstTransducer::new_tokenized_pair("aaaa", "axxa", &tok, type_)?;

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

    let result4 = HfstTransducer::new_tokenized_pair(
        "aaaaabaaaa",
        "ax@_EPSILON_SYMBOL_@@_EPSILON_SYMBOL_@abax@_EPSILON_SYMBOL_@a",
        &tok,
        type_,
    )?;
    let result5 = HfstTransducer::new_tokenized_pair("aaaaabaaaa", "axxxabaxxa", &tok, type_)?;

    let result6 = HfstTransducer::new_tokenized_pair(
        "aaaaabaaaacaaaa",
        "ax@_EPSILON_SYMBOL_@@_EPSILON_SYMBOL_@abax@_EPSILON_SYMBOL_@acax@_EPSILON_SYMBOL_@a",
        &tok,
        type_,
    )?;
    let result7 =
        HfstTransducer::new_tokenized_pair("aaaaabaaaacaaaa", "axxxabaxxacaxxa", &tok, type_)?;

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
    test2a(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// a+ b+ | b+ a+ @-> x
// [spec:hfst:def:hfst-xerox-rules-test.test2b-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test2b-fn]
fn test2b(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_LM_@");
    tok.add_multichar_symbol("@_RM_@");

    let mut a_plus = HfstTransducer::new_tokenized("a", &tok, type_)?;
    a_plus.repeat_plus()?.minimize()?;
    let mut b_plus = HfstTransducer::new_tokenized("b", &tok, type_)?;
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
    let right_mapping = HfstTransducer::new_tokenized("x", &tok, type_)?;

    let mapping_pair: HfstTransducerPair = (left_mapping, right_mapping);
    let mut mapping_pair_vector: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector.push(mapping_pair);

    let input1 = HfstTransducer::new_tokenized("aabbaa", &tok, type_)?;

    let result1 = HfstTransducer::new_tokenized_pair(
        "aabbaa",
        "x@_EPSILON_SYMBOL_@@_EPSILON_SYMBOL_@@_EPSILON_SYMBOL_@aa",
        &tok,
        type_,
    )?;
    let result2 = HfstTransducer::new_tokenized_pair(
        "aabbaa",
        "aax@_EPSILON_SYMBOL_@@_EPSILON_SYMBOL_@@_EPSILON_SYMBOL_@",
        &tok,
        type_,
    )?;
    let result3 = HfstTransducer::new_tokenized_pair(
        "aabbaa",
        "x@_EPSILON_SYMBOL_@@_EPSILON_SYMBOL_@x@_EPSILON_SYMBOL_@a",
        &tok,
        type_,
    )?;
    let result4 = HfstTransducer::new_tokenized_pair(
        "aabbaa",
        "ax@_EPSILON_SYMBOL_@x@_EPSILON_SYMBOL_@@_EPSILON_SYMBOL_@",
        &tok,
        type_,
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
    let input2 = HfstTransducer::new_tokenized("aabbaax", &tok, type_)?;
    let result5 = HfstTransducer::new_tokenized_pair(
        "aabbaax",
        "x@_EPSILON_SYMBOL_@@_EPSILON_SYMBOL_@x@_EPSILON_SYMBOL_@@_EPSILON_SYMBOL_@x",
        &tok,
        type_,
    )?;
    let context: HfstTransducerPair = (
        HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?,
        HfstTransducer::new_tokenized("x", &tok, type_)?,
    );
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context);

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
    test2b(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// a+ @-> x || c _
// [spec:hfst:def:hfst-xerox-rules-test.test2c-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test2c-fn]
fn test2c(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_UNKNOWN_SYMBOL_@");
    tok.add_multichar_symbol("@_IDENTITY_SYMBOL_@");
    tok.add_multichar_symbol("@_LM_@");
    tok.add_multichar_symbol("@_RM_@");

    let mut left_mapping = HfstTransducer::new_tokenized("a", &tok, type_)?;
    left_mapping.repeat_plus()?.minimize()?;
    let right_mapping = HfstTransducer::new_tokenized("x", &tok, type_)?;

    let mapping_pair: HfstTransducerPair = (left_mapping, right_mapping);
    let mut mapping_pair_vector: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector.push(mapping_pair);

    let context: HfstTransducerPair = (
        HfstTransducer::new_tokenized("c", &tok, type_)?,
        HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?,
    );
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context);

    let input1 = HfstTransducer::new_tokenized("caav", &tok, type_)?;
    let result1 = HfstTransducer::new_tokenized_pair("caav", "cx@_EPSILON_SYMBOL_@v", &tok, type_)?;

    let rule_up =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_UP)?;

    let replace_tr = xr::replace_leftmost_longest_match_rule(&rule_up)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result1, true)?);
    Ok(())
}

#[test]
fn test2c_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test2c(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// test multiple contexts: a -> b ||  x _ x ;
// [spec:hfst:def:hfst-xerox-rules-test.test3a-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test3a-fn]
fn test3a(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_LM_@");
    tok.add_multichar_symbol("@_RM_@");

    let left_mapping = HfstTransducer::new_tokenized("a", &tok, type_)?;
    let right_mapping = HfstTransducer::new_tokenized("b", &tok, type_)?;

    let mapping_pair: HfstTransducerPair = (left_mapping, right_mapping);
    let mut mapping_pair_vector: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector.push(mapping_pair);

    let context: HfstTransducerPair = (
        HfstTransducer::new_tokenized("x", &tok, type_)?,
        HfstTransducer::new_tokenized("x", &tok, type_)?,
    );
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context);

    let input1 = HfstTransducer::new_tokenized("xaxax", &tok, type_)?;

    let mut result1 = HfstTransducer::new_tokenized("xaxax", &tok, type_)?;
    let r1tmp = HfstTransducer::new_tokenized_pair("xaxax", "xbxax", &tok, type_)?;
    let r2tmp = HfstTransducer::new_tokenized_pair("xaxax", "xaxbx", &tok, type_)?;
    let r3tmp = HfstTransducer::new_tokenized_pair("xaxax", "xbxbx", &tok, type_)?;
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
    test3a(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// test multiple contexts: a b -> b ||  x_y, y_z
// [spec:hfst:def:hfst-xerox-rules-test.test3b-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test3b-fn]
fn test3b(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_LM_@");
    tok.add_multichar_symbol("@_RM_@");

    let mut left_mapping = HfstTransducer::new_tokenized("a", &tok, type_)?;
    left_mapping.repeat_plus()?.minimize()?;
    let right_mapping = HfstTransducer::new_tokenized("b", &tok, type_)?;

    let mapping_pair: HfstTransducerPair = (left_mapping, right_mapping);
    let mut mapping_pair_vector: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector.push(mapping_pair);

    let context: HfstTransducerPair = (
        HfstTransducer::new_tokenized("x", &tok, type_)?,
        HfstTransducer::new_tokenized("y", &tok, type_)?,
    );
    let context2: HfstTransducerPair = (
        HfstTransducer::new_tokenized("y", &tok, type_)?,
        HfstTransducer::new_tokenized("z", &tok, type_)?,
    );
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context);
    context_vector.push(context2);

    let input1 = HfstTransducer::new_tokenized("axayaz", &tok, type_)?;

    let mut result1 = HfstTransducer::new_tokenized("axayaz", &tok, type_)?;
    let r1tmp = HfstTransducer::new_tokenized_pair("axayaz", "axbybz", &tok, type_)?;
    let r2tmp = HfstTransducer::new_tokenized_pair("axayaz", "axbyaz", &tok, type_)?;
    let r3tmp = HfstTransducer::new_tokenized_pair("axayaz", "axaybz", &tok, type_)?;
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
    test3b(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// test multiple contexts: a+ -> x  || x x _ y y, y _ x
// [spec:hfst:def:hfst-xerox-rules-test.test3c-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test3c-fn]
fn test3c(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_LM_@");
    tok.add_multichar_symbol("@_RM_@");

    let mut left_mapping = HfstTransducer::new_tokenized("a", &tok, type_)?;
    left_mapping.repeat_plus()?.minimize()?;
    let right_mapping = HfstTransducer::new_tokenized("x", &tok, type_)?;

    let mapping_pair: HfstTransducerPair = (left_mapping, right_mapping);
    let mut mapping_pair_vector: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector.push(mapping_pair);

    let context: HfstTransducerPair = (
        HfstTransducer::new_tokenized("xx", &tok, type_)?,
        HfstTransducer::new_tokenized("yy", &tok, type_)?,
    );
    let context2: HfstTransducerPair = (
        HfstTransducer::new_tokenized("y", &tok, type_)?,
        HfstTransducer::new_tokenized("x", &tok, type_)?,
    );
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context);
    context_vector.push(context2);

    let input1 = HfstTransducer::new_tokenized("axxayyax", &tok, type_)?;

    let mut result1 = HfstTransducer::new_tokenized("axxayyax", &tok, type_)?;
    let r1tmp = HfstTransducer::new_tokenized_pair("axxayyax", "axxayyxx", &tok, type_)?;
    let r2tmp = HfstTransducer::new_tokenized_pair("axxayyax", "axxxyyax", &tok, type_)?;
    let r3tmp = HfstTransducer::new_tokenized_pair("axxayyax", "axxxyyxx", &tok, type_)?;
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
    test3c(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// test multiple contexts: a -> b ;
// [spec:hfst:def:hfst-xerox-rules-test.test3d-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test3d-fn]
fn test3d(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_LM_@");
    tok.add_multichar_symbol("@_RM_@");

    let left_mapping = HfstTransducer::new_tokenized("a", &tok, type_)?;
    let right_mapping = HfstTransducer::new_tokenized("b", &tok, type_)?;

    let mapping_pair: HfstTransducerPair = (left_mapping, right_mapping);
    let mut mapping_pair_vector: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector.push(mapping_pair);

    let context: HfstTransducerPair = (
        HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?,
        HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?,
    );
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context);

    let input1 = HfstTransducer::new_tokenized("xaxax", &tok, type_)?;

    let mut result1 = HfstTransducer::new_tokenized("xaxax", &tok, type_)?;
    let r1tmp = HfstTransducer::new_tokenized_pair("xaxax", "xbxax", &tok, type_)?;
    let r2tmp = HfstTransducer::new_tokenized_pair("xaxax", "xaxbx", &tok, type_)?;
    let r3tmp = HfstTransducer::new_tokenized_pair("xaxax", "xbxbx", &tok, type_)?;
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
    test3d(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// b -> a  || _a ; input: bbba
// [spec:hfst:def:hfst-xerox-rules-test.test4a-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test4a-fn]
fn test4a(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_LM_@");
    tok.add_multichar_symbol("@_RM_@");

    let left_mapping = HfstTransducer::new_tokenized("b", &tok, type_)?;
    let right_mapping = HfstTransducer::new_tokenized("a", &tok, type_)?;

    let mapping_pair: HfstTransducerPair = (left_mapping, right_mapping);
    let mut mapping_pair_vector: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector.push(mapping_pair);

    let context: HfstTransducerPair = (
        HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?,
        HfstTransducer::new_tokenized("a", &tok, type_)?,
    );
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context);

    let input1 = HfstTransducer::new_tokenized("bbba", &tok, type_)?;

    let result1 = HfstTransducer::new_tokenized_pair("bbba", "bbaa", &tok, type_)?;
    let result2 = HfstTransducer::new_tokenized_pair("bbba", "aaaa", &tok, type_)?;
    let r1_tmp = HfstTransducer::new_tokenized_pair("bbba", "baaa", &tok, type_)?;
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
    test4a(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// b -> a  || a _ ; input: abbb
// [spec:hfst:def:hfst-xerox-rules-test.test4b-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test4b-fn]
fn test4b(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_LM_@");
    tok.add_multichar_symbol("@_RM_@");

    let left_mapping = HfstTransducer::new_tokenized("b", &tok, type_)?;
    let right_mapping = HfstTransducer::new_tokenized("a", &tok, type_)?;

    let mapping_pair: HfstTransducerPair = (left_mapping, right_mapping);
    let mut mapping_pair_vector: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector.push(mapping_pair);

    let context: HfstTransducerPair = (
        HfstTransducer::new_tokenized("a", &tok, type_)?,
        HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?,
    );
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context);

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

    let input1 = HfstTransducer::new_tokenized("abbb", &tok, type_)?;

    let result1 = HfstTransducer::new_tokenized_pair("abbb", "aabb", &tok, type_)?;
    let result2 = HfstTransducer::new_tokenized_pair("abbb", "aaaa", &tok, type_)?;
    let r1_tmp = HfstTransducer::new_tokenized_pair("abbb", "aaab", &tok, type_)?;
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
    test4b(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// ab -> x || ab _ a
// [spec:hfst:def:hfst-xerox-rules-test.test4c-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test4c-fn]
fn test4c(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let left_mapping = HfstTransducer::new_tokenized("ab", &tok, type_)?;
    let right_mapping = HfstTransducer::new_tokenized("x", &tok, type_)?;

    let mapping_pair: HfstTransducerPair = (left_mapping, right_mapping);
    let mut mapping_pair_vector: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector.push(mapping_pair);

    let context: HfstTransducerPair = (
        HfstTransducer::new_tokenized("ab", &tok, type_)?,
        HfstTransducer::new_tokenized("a", &tok, type_)?,
    );
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context);

    let input1 = HfstTransducer::new_tokenized("abababa", &tok, type_)?;

    let result1 = HfstTransducer::new_tokenized_pair("abababa", "abababa", &tok, type_)?;
    let r2tmp =
        HfstTransducer::new_tokenized_pair("abababa", "ababx@_EPSILON_SYMBOL_@a", &tok, type_)?;
    let r3tmp =
        HfstTransducer::new_tokenized_pair("abababa", "abx@_EPSILON_SYMBOL_@aba", &tok, type_)?;
    let r4tmp = HfstTransducer::new_tokenized_pair(
        "abababa",
        "abx@_EPSILON_SYMBOL_@x@_EPSILON_SYMBOL_@a",
        &tok,
        type_,
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
    test4c(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// epenthesis rules: 0 -> p || m _ k
// [spec:hfst:def:hfst-xerox-rules-test.test6a-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test6a-fn]
fn test6a(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_LM_@");
    tok.add_multichar_symbol("@_RM_@");

    let left_mapping = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?;
    let right_mapping = HfstTransducer::new_tokenized("p", &tok, type_)?;
    let mapping_pair: HfstTransducerPair = (left_mapping, right_mapping);
    let mut mapping_pair_vector: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector.push(mapping_pair);

    let context: HfstTransducerPair = (
        HfstTransducer::new_tokenized("m", &tok, type_)?,
        HfstTransducer::new_tokenized("k", &tok, type_)?,
    );
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context);

    let input1 = HfstTransducer::new_tokenized("mk", &tok, type_)?;

    let result1 = HfstTransducer::new_tokenized_pair("m@_EPSILON_SYMBOL_@k", "mpk", &tok, type_)?;
    let mut result2 = HfstTransducer::new_tokenized_pair("mk", "mk", &tok, type_)?;
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
    test6a(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// a* -> p ;
// [spec:hfst:def:hfst-xerox-rules-test.test6b-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test6b-fn]
fn test6b(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_LM_@");
    tok.add_multichar_symbol("@_RM_@");
    tok.add_multichar_symbol(".#.");

    let mut left_mapping = HfstTransducer::new_tokenized("a", &tok, type_)?;
    left_mapping.repeat_star()?.minimize()?;

    let right_mapping = HfstTransducer::new_tokenized("p", &tok, type_)?;
    let mapping_pair: HfstTransducerPair = (left_mapping, right_mapping);
    let mut mapping_pair_vector: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector.push(mapping_pair);

    let context: HfstTransducerPair = (
        HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?,
        HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?,
    );
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context);

    let input1 = HfstTransducer::new_tokenized("mak", &tok, type_)?;

    let result1 = HfstTransducer::new_tokenized_pair(
        "@_EPSILON_SYMBOL_@m@_EPSILON_SYMBOL_@a@_EPSILON_SYMBOL_@k@_EPSILON_SYMBOL_@",
        "pmpppkp",
        &tok,
        type_,
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
    test6b(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// 0 -> b || _ a a
// [spec:hfst:def:hfst-xerox-rules-test.test6c-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test6c-fn]
fn test6c(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_LM_@");
    tok.add_multichar_symbol("@_RM_@");

    let left_mapping = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?;
    let right_mapping = HfstTransducer::new_tokenized("b", &tok, type_)?;
    let mapping_pair: HfstTransducerPair = (left_mapping, right_mapping);
    let mut mapping_pair_vector: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector.push(mapping_pair);

    let context: HfstTransducerPair = (
        HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?,
        HfstTransducer::new_tokenized("aa", &tok, type_)?,
    );
    let mut context_vector: HfstTransducerPairVector = Vec::new();
    context_vector.push(context);

    let input1 = HfstTransducer::new_tokenized("aa", &tok, type_)?;

    let result1 = HfstTransducer::new_tokenized_pair("@_EPSILON_SYMBOL_@aa", "baa", &tok, type_)?;

    let rule_up =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector, &context_vector, REPL_UP)?;

    let replace_tr = xr::replace_rule(&rule_up, false)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result1, true)?);
    Ok(())
}

#[test]
fn test6c_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test6c(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// a -> b , b -> c
// [spec:hfst:def:hfst-xerox-rules-test.test7a-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test7a-fn]
fn test7a(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let mapping_pair1: HfstTransducerPair = (
        HfstTransducer::new_tokenized("a", &tok, type_)?,
        HfstTransducer::new_tokenized("b", &tok, type_)?,
    );
    let mapping_pair2: HfstTransducerPair = (
        HfstTransducer::new_tokenized("b", &tok, type_)?,
        HfstTransducer::new_tokenized("c", &tok, type_)?,
    );

    let mut mapping_pair_vector1: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector1.push(mapping_pair1);
    let mut mapping_pair_vector2: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector2.push(mapping_pair2);

    let rule1 = Rule::new_mapping(&mapping_pair_vector1)?;
    let rule2 = Rule::new_mapping(&mapping_pair_vector2)?;

    let mut rule_vector: Vec<Rule> = Vec::new();
    rule_vector.push(rule1);
    rule_vector.push(rule2);

    let input1 = HfstTransducer::new_tokenized("aab", &tok, type_)?;
    let result1 = HfstTransducer::new_tokenized_pair("aab", "bbc", &tok, type_)?;

    let replace_tr = xr::replace_rule_vector(&rule_vector, false)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result1, true)?);
    Ok(())
}

#[test]
fn test7a_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test7a(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// [. .] -> b , a -> c ;
// [spec:hfst:def:hfst-xerox-rules-test.test7b-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test7b-fn]
fn test7b(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let mapping_pair1: HfstTransducerPair = (
        HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?,
        HfstTransducer::new_tokenized("b", &tok, type_)?,
    );
    let mapping_pair2: HfstTransducerPair = (
        HfstTransducer::new_tokenized("a", &tok, type_)?,
        HfstTransducer::new_tokenized("c", &tok, type_)?,
    );

    let mut mapping_pair_vector1: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector1.push(mapping_pair1);
    let mut mapping_pair_vector2: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector2.push(mapping_pair2);

    let rule1 = Rule::new_mapping(&mapping_pair_vector1)?;
    let rule2 = Rule::new_mapping(&mapping_pair_vector2)?;

    let mut rule_vector: Vec<Rule> = Vec::new();
    rule_vector.push(rule1);
    rule_vector.push(rule2);

    let input1 = HfstTransducer::new_tokenized("a", &tok, type_)?;
    let result1 = HfstTransducer::new_tokenized_pair(
        "@_EPSILON_SYMBOL_@a@_EPSILON_SYMBOL_@",
        "bcb",
        &tok,
        type_,
    )?;

    let replace_tr = xr::replace_rule_vector(&rule_vector, false)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result1, true)?);
    Ok(())
}

#[test]
fn test7b_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test7b(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// a+ @-> x , b+ @-> y ; then with contexts
// [spec:hfst:def:hfst-xerox-rules-test.test7c-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test7c-fn]
fn test7c(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let mut left_mapping1 = HfstTransducer::new_tokenized("a", &tok, type_)?;
    left_mapping1.repeat_plus()?.minimize()?;
    let right_mapping1 = HfstTransducer::new_tokenized("x", &tok, type_)?;
    let mapping_pair1: HfstTransducerPair = (left_mapping1, right_mapping1);

    let mut left_mapping2 = HfstTransducer::new_tokenized("b", &tok, type_)?;
    left_mapping2.repeat_plus()?.minimize()?;
    let right_mapping2 = HfstTransducer::new_tokenized("y", &tok, type_)?;
    let mapping_pair2: HfstTransducerPair = (left_mapping2, right_mapping2);

    let mut mapping_pair_vector1: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector1.push(mapping_pair1);
    let mut mapping_pair_vector2: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector2.push(mapping_pair2);

    let rule1 = Rule::new_mapping(&mapping_pair_vector1)?;
    let rule2 = Rule::new_mapping(&mapping_pair_vector2)?;

    let mut rule_vector: Vec<Rule> = Vec::new();
    rule_vector.push(rule1);
    rule_vector.push(rule2);

    let input1 = HfstTransducer::new_tokenized("aaabbb", &tok, type_)?;
    let result1 = HfstTransducer::new_tokenized_pair(
        "aaabbb",
        "x@_EPSILON_SYMBOL_@@_EPSILON_SYMBOL_@y@_EPSILON_SYMBOL_@@_EPSILON_SYMBOL_@",
        &tok,
        type_,
    )?;
    let result1b = HfstTransducer::new_tokenized_pair("aaabbb", "xxxyyy", &tok, type_)?;

    let replace_tr = xr::replace_leftmost_longest_match_rule_vector(&rule_vector)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result1, true)?);

    let replace_tr = xr::replace_leftmost_shortest_match_rule_vector(&rule_vector)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result1b, true)?);

    // With Contexts
    // a -> x \/ m _ ,, b -> y || x _ ;
    let input2 = HfstTransducer::new_tokenized("mab", &tok, type_)?;
    let result2 = HfstTransducer::new_tokenized_pair("mab", "mxb", &tok, type_)?;
    let result3 = HfstTransducer::new_tokenized_pair("mab", "mxy", &tok, type_)?;

    let input3 = HfstTransducer::new_tokenized("maabb", &tok, type_)?;

    let mut result4 =
        HfstTransducer::new_tokenized_pair("maabb", "mx@_EPSILON_SYMBOL_@bb", &tok, type_)?;
    let result4b = HfstTransducer::new_tokenized_pair("maabb", "mxabb", &tok, type_)?;
    result4.disjunct(&result4b, true)?.minimize()?;

    let mut result5 =
        HfstTransducer::new_tokenized_pair("maabb", "mx@_EPSILON_SYMBOL_@yb", &tok, type_)?;
    let result5b = HfstTransducer::new_tokenized_pair(
        "maabb",
        "mx@_EPSILON_SYMBOL_@y@_EPSILON_SYMBOL_@",
        &tok,
        type_,
    )?;
    result5
        .disjunct(&result5b, true)?
        .disjunct(&result4b, true)?
        .minimize()?;

    let context1: HfstTransducerPair = (
        HfstTransducer::new_tokenized("m", &tok, type_)?,
        HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?,
    );
    let context2: HfstTransducerPair = (
        HfstTransducer::new_tokenized("x", &tok, type_)?,
        HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?,
    );
    let mut context_vector1: HfstTransducerPairVector = Vec::new();
    context_vector1.push(context1);
    let mut context_vector2: HfstTransducerPairVector = Vec::new();
    context_vector2.push(context2);

    // replace up
    let rule2a_up =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector1, &context_vector1, REPL_UP)?;
    let rule2b_up =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector2, &context_vector2, REPL_UP)?;

    let mut rule_vector2: Vec<Rule> = Vec::new();
    rule_vector2.push(rule2a_up);
    rule_vector2.push(rule2b_up);

    let replace_tr = xr::replace_rule_vector(&rule_vector2, false)?;
    assert!(compose_minimize(&input2, &replace_tr)?.compare(&result2, true)?);
    assert!(compose_minimize(&input3, &replace_tr)?.compare(&result4, true)?);

    // replace down
    let rule2a_down =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector1, &context_vector1, REPL_DOWN)?;
    let rule2b_down =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector2, &context_vector2, REPL_DOWN)?;

    let mut rule_vector3: Vec<Rule> = Vec::new();
    rule_vector3.push(rule2a_down);
    rule_vector3.push(rule2b_down);

    let replace_tr = xr::replace_rule_vector(&rule_vector3, false)?;
    assert!(compose_minimize(&input2, &replace_tr)?.compare(&result3, true)?);
    assert!(compose_minimize(&input3, &replace_tr)?.compare(&result5, true)?);
    Ok(())
}

#[test]
fn test7c_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test7c(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// 0 .o. [ [. 0 .] -> a \/ _ b a , a b _ ,, [. 0 .] -> b \/ a _ a ]
// [spec:hfst:def:hfst-xerox-rules-test.test7d-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test7d-fn]
fn test7d(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let mapping_pair1: HfstTransducerPair = (
        HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?,
        HfstTransducer::new_tokenized("a", &tok, type_)?,
    );
    let mapping_pair2: HfstTransducerPair = (
        HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?,
        HfstTransducer::new_tokenized("b", &tok, type_)?,
    );

    let mut mapping_pair_vector1: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector1.push(mapping_pair1);
    let mut mapping_pair_vector2: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector2.push(mapping_pair2);

    let context1a: HfstTransducerPair = (
        HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?,
        HfstTransducer::new_tokenized("ba", &tok, type_)?,
    );
    let context1b: HfstTransducerPair = (
        HfstTransducer::new_tokenized("ab", &tok, type_)?,
        HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?,
    );
    let mut context_vector1: HfstTransducerPairVector = Vec::new();
    context_vector1.push(context1a);
    context_vector1.push(context1b);

    let context2: HfstTransducerPair = (
        HfstTransducer::new_tokenized("a", &tok, type_)?,
        HfstTransducer::new_tokenized("a", &tok, type_)?,
    );
    let mut context_vector2: HfstTransducerPairVector = Vec::new();
    context_vector2.push(context2);

    let rule1 =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector1, &context_vector1, REPL_DOWN)?;
    let rule2 =
        Rule::new_mapping_context_repl_type(&mapping_pair_vector2, &context_vector2, REPL_DOWN)?;

    let mut rule_vector: Vec<Rule> = Vec::new();
    rule_vector.push(rule1);
    rule_vector.push(rule2);

    let input1 = HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?;

    let replace_tr = xr::replace_rule_vector(&rule_vector, false)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&input1, true)?);
    Ok(())
}

#[test]
fn test7d_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test7d(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// ? -> x , a -> b
// [spec:hfst:def:hfst-xerox-rules-test.test7e-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test7e-fn]
fn test7e(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_IDENTITY_SYMBOL_@");

    let mapping_pair1: HfstTransducerPair = (
        HfstTransducer::new_tokenized("@_IDENTITY_SYMBOL_@", &tok, type_)?,
        HfstTransducer::new_tokenized("x", &tok, type_)?,
    );
    let mapping_pair2: HfstTransducerPair = (
        HfstTransducer::new_tokenized("a", &tok, type_)?,
        HfstTransducer::new_tokenized("b", &tok, type_)?,
    );

    let mut mapping_pair_vector1: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector1.push(mapping_pair1);
    let mut mapping_pair_vector2: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector2.push(mapping_pair2);

    let rule1 = Rule::new_mapping(&mapping_pair_vector1)?;
    let rule2 = Rule::new_mapping(&mapping_pair_vector2)?;

    let mut rule_vector: Vec<Rule> = Vec::new();
    rule_vector.push(rule1);
    rule_vector.push(rule2);

    let input1 = HfstTransducer::new_tokenized("ak", &tok, type_)?;
    let tmp = HfstTransducer::new_tokenized_pair("ak", "xx", &tok, type_)?;
    let mut result1 = HfstTransducer::new_tokenized_pair("ak", "bx", &tok, type_)?;
    result1.disjunct(&tmp, true)?.minimize()?;

    let replace_tr = xr::replace_rule_vector(&rule_vector, false)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result1, true)?);
    Ok(())
}

#[test]
fn test7e_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test7e(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// a -> b , b -> a
// [spec:hfst:def:hfst-xerox-rules-test.test7f-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test7f-fn]
fn test7f(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let mapping_pair1: HfstTransducerPair = (
        HfstTransducer::new_tokenized("a", &tok, type_)?,
        HfstTransducer::new_tokenized("b", &tok, type_)?,
    );
    let mapping_pair2: HfstTransducerPair = (
        HfstTransducer::new_tokenized("b", &tok, type_)?,
        HfstTransducer::new_tokenized("a", &tok, type_)?,
    );

    let mut mapping_pair_vector1: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector1.push(mapping_pair1);
    let mut mapping_pair_vector2: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector2.push(mapping_pair2);

    let rule1 = Rule::new_mapping(&mapping_pair_vector1)?;
    let rule2 = Rule::new_mapping(&mapping_pair_vector2)?;

    let mut rule_vector: Vec<Rule> = Vec::new();
    rule_vector.push(rule1);
    rule_vector.push(rule2);

    let input1 = HfstTransducer::new_tokenized("aabbaa", &tok, type_)?;
    let result1 = HfstTransducer::new_tokenized_pair("aabbaa", "bbaabb", &tok, type_)?;

    let replace_tr = xr::replace_rule_vector(&rule_vector, false)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result1, true)?);
    Ok(())
}

#[test]
fn test7f_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test7f(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// a -> b b, a -> b
// [spec:hfst:def:hfst-xerox-rules-test.test7g-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test7g-fn]
fn test7g(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");

    let mapping_pair1: HfstTransducerPair = (
        HfstTransducer::new_tokenized("a", &tok, type_)?,
        HfstTransducer::new_tokenized("bb", &tok, type_)?,
    );
    let mapping_pair2: HfstTransducerPair = (
        HfstTransducer::new_tokenized("a", &tok, type_)?,
        HfstTransducer::new_tokenized("b", &tok, type_)?,
    );

    let mut mapping_pair_vector1: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector1.push(mapping_pair1);
    let mut mapping_pair_vector2: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector2.push(mapping_pair2);

    let rule1 = Rule::new_mapping(&mapping_pair_vector1)?;
    let rule2 = Rule::new_mapping(&mapping_pair_vector2)?;

    let mut rule_vector: Vec<Rule> = Vec::new();
    rule_vector.push(rule1);
    rule_vector.push(rule2);

    let input1 = HfstTransducer::new_tokenized("a", &tok, type_)?;
    let mut result1 = HfstTransducer::new_tokenized_pair("a", "b", &tok, type_)?;
    let result_tmp = HfstTransducer::new_tokenized_pair("a@_EPSILON_SYMBOL_@", "bb", &tok, type_)?;
    result1.disjunct(&result_tmp, true)?.minimize()?;

    let replace_tr = xr::replace_rule_vector(&rule_vector, false)?;
    assert!(compose_minimize(&input1, &replace_tr)?.compare(&result1, true)?);
    Ok(())
}

#[test]
fn test7g_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test7g(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

// [..] @-> a;
// [spec:hfst:def:hfst-xerox-rules-test.test7h-fn]
// [spec:hfst:sem:hfst-xerox-rules-test.test7h-fn]
fn test7h(type_: ImplementationType) -> Result<(), hfst::error::Error> {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("@_EPSILON_SYMBOL_@");
    tok.add_multichar_symbol("@_IDENTITY_SYMBOL_@");

    let mapping_pair1: HfstTransducerPair = (
        HfstTransducer::new_tokenized("@_EPSILON_SYMBOL_@", &tok, type_)?,
        HfstTransducer::new_tokenized("a", &tok, type_)?,
    );
    let mut mapping_pair_vector1: HfstTransducerPairVector = Vec::new();
    mapping_pair_vector1.push(mapping_pair1);

    let rule = Rule::new_mapping(&mapping_pair_vector1)?;

    let replace_tr = xr::replace_leftmost_longest_match_rule(&rule)?;

    let mut bt = HfstBasicTransducer::new();
    let tr = HfstBasicTransition::new_symbols(
        1,
        "@_EPSILON_SYMBOL_@".to_string(),
        "a".to_string(),
        0.0,
        bt.coder_mut(),
    );
    bt.add_transition(0, &tr, true);
    let tr = HfstBasicTransition::new_symbols(
        0,
        "@_IDENTITY_SYMBOL_@".to_string(),
        "@_IDENTITY_SYMBOL_@".to_string(),
        0.0,
        bt.coder_mut(),
    );
    bt.add_transition(1, &tr, true);
    let tr =
        HfstBasicTransition::new_symbols(0, "a".to_string(), "a".to_string(), 0.0, bt.coder_mut());
    bt.add_transition(1, &tr, true);
    bt.set_final_weight(1, &0.0);

    let result1 = HfstTransducer::new_from_basic(&bt, type_)?;
    assert!(replace_tr.compare(&result1, true)?);
    Ok(())
}

#[test]
fn test7h_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    test7h(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}
