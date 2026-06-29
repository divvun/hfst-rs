// Regression oracle for hfst_symbol_defs::label_to_stringpair — the escaped-colon
// arc-label parser lifted out of hfst-substitute.cc / hfst-insert-freely.cc (whose
// C++ copies are byte-identical). Pure string parsing, no global symbol table, so
// no serialization lock is needed.
//
// NOTE: the pathological `\:`-at-index-1 input (e.g. "\\:x") is intentionally NOT
// exercised — the C++ loops forever there and the faithful port preserves that, so
// a test on it would hang.

use hfst::hfst_symbol_defs::{internal_epsilon, label_to_stringpair};

#[test]
fn plain_pair_splits_at_the_colon() {
    assert_eq!(
        label_to_stringpair("a:b"),
        Some(("a".to_string(), "b".to_string()))
    );
}

#[test]
fn no_colon_is_not_a_pair() {
    assert_eq!(label_to_stringpair("abc"), None);
}

#[test]
fn first_interior_colon_wins() {
    // "a:b:c" splits at the first genuine separator, output keeps the rest.
    assert_eq!(
        label_to_stringpair("a:b:c"),
        Some(("a".to_string(), "b:c".to_string()))
    );
}

#[test]
fn epsilon_marker_maps_to_internal_epsilon() {
    assert_eq!(
        label_to_stringpair("@0@:b"),
        Some((internal_epsilon.to_string(), "b".to_string()))
    );
    assert_eq!(
        label_to_stringpair("a:@0@"),
        Some(("a".to_string(), internal_epsilon.to_string()))
    );
}

#[test]
fn escaped_colon_is_not_a_separator() {
    // The single colon is escaped, so there is no genuine pair separator.
    assert_eq!(label_to_stringpair("a\\:b"), None);
    // ...but a later unescaped colon does separate, keeping the escaped one.
    assert_eq!(
        label_to_stringpair("a\\:b:c"),
        Some(("a\\:b".to_string(), "c".to_string()))
    );
}

#[test]
fn escaped_backslash_leaves_the_colon_as_separator() {
    // "a\\:b" — the backslash itself is escaped, so the colon separates.
    assert_eq!(
        label_to_stringpair("a\\\\:b"),
        Some(("a\\\\".to_string(), "b".to_string()))
    );
}
