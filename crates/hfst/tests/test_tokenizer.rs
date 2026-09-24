// Port of test/libhfst/test_tokenizer.cc
//
// Tests HfstTokenizer: longest-match multichar tokenization (single string and
// string pair) and skip symbols.
//
// The C++ main() is a flat sequence of asserts with no loop over implementation
// types (the tokenizer is purely string-based and type-independent), so there
// is no SFST/FOMA/XFSM iteration to skip here and no symbol-table global state
// to serialize.
//
// Shared helper from test/libhfst/auxiliary_functions.cc: verbose_print is
// inlined as a plain message printer (this suite only ever calls it with a
// message, the default ERROR_TYPE).

use hfst::hfst_data_types::StringPair;
use hfst::hfst_symbol_defs::internal_epsilon;
use hfst::hfst_tokenizer::HfstTokenizer;

// Inlined from auxiliary_functions.cc.
fn verbose_print(msg: &str) {
    eprintln!("Testing:\t{msg}...");
}

// C++ StringPair("a", "b").
fn sp(a: &str, b: &str) -> StringPair {
    (a.into(), b.into())
}

// ---------------------------------------------------------------------------
// Tokenization from a single string.
// ---------------------------------------------------------------------------

#[test]
fn tokenize_single_multichar_foo_skip_bar() {
    verbose_print(
        "Tokenization from one string with multichar symbol \"foo\" and skip symbol \"bar\"",
    );
    let mut tok1 = HfstTokenizer::new();
    tok1.add_multichar_symbol("foo");
    tok1.add_skip_symbol("bar");
    let tokenization1 = tok1.tokenize("fobaro", false);
    assert_eq!(tokenization1.len(), 3);
    assert_eq!(tokenization1[0], sp("f", "f"));
    assert_eq!(tokenization1[1], sp("o", "o"));
    assert_eq!(tokenization1[2], sp("o", "o"));
}

#[test]
fn tokenize_single_multichar_foo_skip_fo() {
    verbose_print(
        "Tokenization from one string with multichar symbol \"foo\" and skip symbol \"fo\"",
    );
    let mut tok2 = HfstTokenizer::new();
    tok2.add_multichar_symbol("foo");
    tok2.add_skip_symbol("fo");
    let tokenization2 = tok2.tokenize("foo", false);
    assert_eq!(tokenization2.len(), 1);
    assert_eq!(tokenization2[0], sp("foo", "foo"));
}

#[test]
fn tokenize_single_multichar_fo_skip_foo() {
    verbose_print(
        "Tokenization from one string with multichar symbol \"fo\" and skip symbol \"foo\"",
    );
    let mut tok3 = HfstTokenizer::new();
    tok3.add_multichar_symbol("fo");
    tok3.add_skip_symbol("foo");
    let tokenization3 = tok3.tokenize("foo", false);
    assert_eq!(tokenization3.len(), 0);
}

// ---------------------------------------------------------------------------
// Tokenization from two strings (pair). C++ tok.tokenize(in, out, false) maps
// to the facade's tokenize_pair.
// ---------------------------------------------------------------------------

#[test]
fn tokenize_pair_multichar_foo_skip_bar() {
    verbose_print(
        "Tokenization from two strings with multichar symbol \"foo\" and skip symbol \"bar\"",
    );
    let mut tok4 = HfstTokenizer::new();
    tok4.add_multichar_symbol("foo");
    tok4.add_skip_symbol("bar");
    let tokenization4 = tok4.tokenize_pair("fooba", "foobar", false);
    assert_eq!(tokenization4.len(), 3);
    assert_eq!(tokenization4[0], sp("foo", "foo"));
    assert_eq!(tokenization4[1], sp("b", internal_epsilon));
    assert_eq!(tokenization4[2], sp("a", internal_epsilon));
}
