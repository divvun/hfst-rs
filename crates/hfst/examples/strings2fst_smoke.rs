use hfst::hfst_strings2_fst_tokenizer::{HfstStrings2FstTokenizer, StringVector};

fn pair(a: &str, b: &str) -> (String, String) {
    (a.to_string(), b.to_string())
}

fn main() {
    let mc: StringVector = vec!["##".to_string(), "+NOM".to_string()];
    let tok = HfstStrings2FstTokenizer::new(&mc, "@_EPS_@");

    // identity string pair (no colon)
    assert_eq!(
        tok.tokenize_string_pair("ab", false),
        vec![pair("a", "a"), pair("b", "b")]
    );

    // pair string with a colon: a -> b
    assert_eq!(tok.tokenize_pair_string("a:b", false), vec![pair("a", "b")]);

    // escaped colon "\:" is a literal ":" identity symbol
    assert_eq!(
        tok.tokenize_pair_string("a\\:b", false),
        vec![pair("a", "a"), pair(":", ":"), pair("b", "b")]
    );

    // string pair split at the colon: input "ab", output "cd"
    assert_eq!(
        tok.tokenize_string_pair("ab:cd", false),
        vec![pair("a", "c"), pair("b", "d")]
    );

    // eps maps to the epsilon symbol
    assert_eq!(
        tok.tokenize_pair_string("@_EPS_@", false),
        vec![pair("@_EPSILON_SYMBOL_@", "@_EPSILON_SYMBOL_@")]
    );

    println!("strings2fst OK");
}
