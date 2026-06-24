// Smoke test mirroring the documented example in HfstTokenizer.h.
use hfst::hfst_symbol_defs::symbols::to_string_string_pair_vector;
use hfst::hfst_tokenizer::HfstTokenizer;

fn main() {
    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("<br />");
    tok.add_skip_symbol("<p>");
    tok.add_skip_symbol("</p>");
    let spv = tok.tokenize("<p>A<br />paragraph!</p>", false);
    let rendered = to_string_string_pair_vector(&spv, true);
    println!("{}", rendered);
    // Expected: A <br /> p a r a g r a p h !
    assert_eq!(rendered, "A <br /> p a r a g r a p h !");

    // space-separated
    let ss = HfstTokenizer::tokenize_space_separated("foo bar  baz");
    let ss_rendered = to_string_string_pair_vector(&ss, true);
    println!("{}", ss_rendered);
    assert_eq!(ss_rendered, "foo bar baz");

    // multichar longest-match + skip interaction
    let mut t2 = HfstTokenizer::new();
    t2.add_multichar_symbol("foo");
    let pv = t2.tokenize("foobar", true);
    println!("{}", to_string_string_pair_vector(&pv, true));
    assert_eq!(to_string_string_pair_vector(&pv, true), "foo b a r");

    println!("tokenizer smoke OK");
}
