// Regression oracle for ImplementationType::is_weighted — the weighting predicate
// lifted out of hfst-fst2txt's bespoke per-type check. The weighted HFST backends
// are tropical/log OpenFST and the weighted optimized-lookup format.

use hfst::hfst_data_types::ImplementationType as IT;

#[test]
fn weighted_types() {
    assert!(IT::TROPICAL_OPENFST_TYPE.is_weighted());
    assert!(IT::LOG_OPENFST_TYPE.is_weighted());
    assert!(IT::HFST_OLW_TYPE.is_weighted());
}

#[test]
fn unweighted_types() {
    assert!(!IT::SFST_TYPE.is_weighted());
    assert!(!IT::FOMA_TYPE.is_weighted());
    assert!(!IT::XFSM_TYPE.is_weighted());
    assert!(!IT::HFST_OL_TYPE.is_weighted());
    assert!(!IT::HFST2_TYPE.is_weighted());
    assert!(!IT::UNSPECIFIED_TYPE.is_weighted());
    assert!(!IT::ERROR_TYPE.is_weighted());
}
