// Port of test/libhfst/test_tokenizer.cc
//
// Tests HfstTokenizer: longest-match multichar tokenization (single string and
// string pair), skip symbols, and UTF-8 correctness checking.
//
// The C++ main() is a flat sequence of asserts with no loop over implementation
// types (the tokenizer is purely string-based and type-independent), so there
// is no SFST/FOMA/XFSM iteration to skip here and no symbol-table global state
// to serialize.
//
// Shared helpers from test/libhfst/auxiliary_functions.cc: get_bin is inlined
// (used to build raw byte sequences for the UTF-8 cases). verbose_print is
// inlined as a plain message printer (this suite only ever calls it with a
// message, the default ERROR_TYPE).

use hfst::hfst_data_types::StringPair;
use hfst::hfst_symbol_defs::internal_epsilon;
use hfst::hfst_tokenizer::HfstTokenizer;

// Inlined from auxiliary_functions.cc.
fn verbose_print(msg: &str) {
    eprintln!("Testing:\t{msg}...");
}

// Inlined from auxiliary_functions.cc: assemble a byte from its bit factors,
// most significant first.
#[allow(clippy::too_many_arguments)]
fn get_bin(
    fact_128: u8,
    fact_64: u8,
    fact_32: u8,
    fact_16: u8,
    fact_8: u8,
    fact_4: u8,
    fact_2: u8,
    fact_1: u8,
) -> u8 {
    fact_128 * 128
        + fact_64 * 64
        + fact_32 * 32
        + fact_16 * 16
        + fact_8 * 8
        + fact_4 * 4
        + fact_2 * 2
        + fact_1
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

// ---------------------------------------------------------------------------
// UTF-8 correctness: positive cases (valid UTF-8, must NOT throw).
//
// In C++ check_utf8_correctness took a const char* and threw
// IncorrectUtf8CodingException on malformed bytes. The Rust facade takes &str
// (valid UTF-8 by construction) and the port collapsed the ICU validity check
// to a no-op, so for valid input these calls simply return -- faithfully
// matching the C++ "must not throw" expectation.
// ---------------------------------------------------------------------------

#[test]
fn check_utf8_correctness_accepts_valid_sequences() {
    // Empty string.
    HfstTokenizer::check_utf8_correctness("");

    // ASCII string "ab".
    HfstTokenizer::check_utf8_correctness("ab");

    // 11110000 10011111 10010010 10101001 == F0 9F 92 A9 == U+1F4A9 (valid).
    verbose_print("Case: 11110000 10011111 10010010 10101001");
    let bytes = [
        get_bin(1, 1, 1, 1, 0, 0, 0, 0),
        get_bin(1, 0, 0, 1, 1, 1, 1, 1),
        get_bin(1, 0, 0, 1, 0, 0, 1, 0),
        get_bin(1, 0, 1, 0, 1, 0, 0, 1),
    ];
    let valid = std::str::from_utf8(&bytes).expect("F0 9F 92 A9 is valid UTF-8");
    HfstTokenizer::check_utf8_correctness(valid);
}
