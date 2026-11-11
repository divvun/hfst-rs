use super::*;

const GRAMMAR: &str = r#"
boundary = ["+Der/*"]
prefix = ["PV/*", "TA+"]
enclitic = ["+LI", "+LU", "+GUUQ"]
hidden = ["+Gram/*"]
"#;

const TAGS: &[&str] = &[
    "+Der/vn", "+Der/vv", "+Der/nv", "+NIQ", "+LI", "+LU", "+GUUQ", "+KIP", "+N", "+V", "+Ind",
    "+Abs", "+Sg", "+3Sg", "+1SgPoss", "+Gram/IV", "PV/e+", "PV/ki+", "TA+", "+Pron",
];

fn grammar() -> UnitGrammar {
    UnitGrammar::from_toml(GRAMMAR.as_bytes()).expect("test grammar reads")
}

fn tokenizer() -> AnalysisTokenizer {
    let mut alphabet: Vec<String> = TAGS.iter().map(|t| (*t).to_owned()).collect();
    alphabet.extend(('a'..='z').chain('A'..='Z').map(String::from));
    AnalysisTokenizer::new(&alphabet)
}

fn cut(analysis: &str) -> Vec<(UnitKind, String)> {
    let symbols = tokenizer().tokenize(analysis).expect("analysis tokenizes");
    grammar()
        .segment(&symbols)
        .expect("analysis segments")
        .into_iter()
        .map(|unit| (unit.kind, unit.symbols.concat()))
        .collect()
}

fn refusal(analysis: &str) -> String {
    let symbols = tokenizer().tokenize(analysis).expect("analysis tokenizes");
    grammar()
        .segment(&symbols)
        .expect_err("analysis is refused")
        .to_string()
}

fn units(expected: &[(UnitKind, &str)]) -> Vec<(UnitKind, String)> {
    expected
        .iter()
        .map(|&(kind, text)| (kind, text.to_owned()))
        .collect()
}

// [spec:hfst:sem:prediction-units.stem/test]
// [spec:hfst:sem:prediction-units.ending+1/test]
#[test]
fn stem_and_ending_split_at_the_first_tag() {
    assert_eq!(
        cut("oqar+V+Ind+3Sg"),
        units(&[(UnitKind::Stem, "oqar"), (UnitKind::Ending, "+V+Ind+3Sg")])
    );
}

// [spec:hfst:sem:prediction-units.stem/test]
#[test]
fn a_stem_alone_is_one_unit() {
    assert_eq!(cut("oqar"), units(&[(UnitKind::Stem, "oqar")]));
}

// [spec:hfst:sem:prediction-units.prefix/test]
#[test]
fn each_prefix_is_its_own_unit() {
    assert_eq!(
        cut("PV/ki+PV/e+oqar+V+Ind+3Sg"),
        units(&[
            (UnitKind::Prefix, "PV/ki+"),
            (UnitKind::Prefix, "PV/e+"),
            (UnitKind::Stem, "oqar"),
            (UnitKind::Ending, "+V+Ind+3Sg"),
        ])
    );
}

// [spec:hfst:sem:prediction-units.prefix/test]
#[test]
fn a_tag_before_the_stem_must_be_prefix() {
    let message = refusal("+Voqar");
    assert!(message.contains("\"+V\""), "{message}");
    assert!(message.contains("prefix"), "{message}");
}

// [spec:hfst:sem:prediction-units.prefix/test]
#[test]
fn a_prefix_tag_after_the_stem_is_ordinary() {
    assert_eq!(
        cut("una+PronTA+"),
        units(&[(UnitKind::Stem, "una"), (UnitKind::Ending, "+PronTA+")])
    );
}

// [spec:hfst:sem:prediction-units.boundary+2/test]
// [spec:hfst:sem:prediction-units.stem/test]
#[test]
fn boundary_units_end_at_the_first_boundary_tag() {
    assert_eq!(
        cut("oqar+Gram/IV+NIQ+Der/vn+N+Abs+Sg+1SgPoss"),
        units(&[
            (UnitKind::Stem, "oqar"),
            (UnitKind::Boundary, "+Gram/IV+NIQ+Der/vn"),
            (UnitKind::Ending, "+N+Abs+Sg+1SgPoss"),
        ])
    );
}

// [spec:hfst:sem:prediction-units.boundary+2/test]
#[test]
fn boundary_units_follow_one_another() {
    assert_eq!(
        cut("aak+KIP+Der/nv+Gram/IV+LI+Der/vv+Gram/IV+V+Ind+3Sg"),
        units(&[
            (UnitKind::Stem, "aak"),
            (UnitKind::Boundary, "+KIP+Der/nv"),
            (UnitKind::Boundary, "+Gram/IV+LI+Der/vv"),
            (UnitKind::Ending, "+Gram/IV+V+Ind+3Sg"),
        ])
    );
}

// [spec:hfst:sem:prediction-units.boundary+2/test]
// [spec:hfst:sem:prediction-units.enclitic/test]
#[test]
fn an_enclitic_name_before_der_is_derivation() {
    assert_eq!(
        cut("oqar+LI+Der/vv+V+Ind+3Sg"),
        units(&[
            (UnitKind::Stem, "oqar"),
            (UnitKind::Boundary, "+LI+Der/vv"),
            (UnitKind::Ending, "+V+Ind+3Sg"),
        ])
    );
    assert_eq!(
        cut("oqar+V+Ind+3Sg+LI"),
        units(&[
            (UnitKind::Stem, "oqar"),
            (UnitKind::Ending, "+V+Ind+3Sg"),
            (UnitKind::Enclitic, "+LI"),
        ])
    );
}

// [spec:hfst:sem:prediction-units.stem/test]
#[test]
fn letters_after_a_boundary_start_a_stem() {
    assert_eq!(
        cut("oqar+NIQ+Der/vnilu+N+Abs+Sg"),
        units(&[
            (UnitKind::Stem, "oqar"),
            (UnitKind::Boundary, "+NIQ+Der/vn"),
            (UnitKind::Stem, "ilu"),
            (UnitKind::Ending, "+N+Abs+Sg"),
        ])
    );
}

// [spec:hfst:sem:prediction-units.enclitic/test]
#[test]
fn each_enclitic_runs_to_the_next_enclitic_tag() {
    assert_eq!(
        cut("nuna+N+Abs+Sg+GUUQ+Sg+LU"),
        units(&[
            (UnitKind::Stem, "nuna"),
            (UnitKind::Ending, "+N+Abs+Sg"),
            (UnitKind::Enclitic, "+GUUQ+Sg"),
            (UnitKind::Enclitic, "+LU"),
        ])
    );
}

// [spec:hfst:sem:prediction-units.enclitic/test]
#[test]
fn an_enclitic_can_follow_a_bare_stem() {
    assert_eq!(
        cut("una+LU"),
        units(&[(UnitKind::Stem, "una"), (UnitKind::Enclitic, "+LU")])
    );
}

// [spec:hfst:sem:prediction-units.ending+1/test]
#[test]
fn letters_after_an_ending_are_refused() {
    let message = refusal("oqar+V+Ind+3Sgilu");
    assert!(message.contains("\"i\""), "{message}");
    let message = refusal("nuna+N+Abs+LUilu");
    assert!(message.contains("\"i\""), "{message}");
}

#[test]
fn an_analysis_without_a_stem_is_refused() {
    assert!(refusal("PV/e+").contains("no stem"));
    let empty: [&str; 0] = [];
    assert!(grammar().segment(&empty).is_err());
}

// [spec:hfst:sem:prediction-units.filter/test]
#[test]
fn analyses_never_hold_flags_or_reserved_symbols() {
    let g = grammar();
    let flagged = ["n", "u", "n", "a", "@U.person.NI@", "+N"];
    let message = g.segment(&flagged).expect_err("flag refused").to_string();
    assert!(message.contains("@U.person.NI@"), "{message}");
    let epsilon = ["n", "u", "n", "a", "@_EPSILON_SYMBOL_@", "+N"];
    assert!(g.segment(&epsilon).is_err());
}

#[test]
fn the_tokenizer_takes_the_longest_symbol() {
    let alphabet = ["+N", "+Nom", "+", "N", "o", "m", "TA+", "T", "A"];
    let symbols = AnalysisTokenizer::new(&alphabet)
        .tokenize("TA+No+Nom+N")
        .expect("analysis tokenizes");
    assert_eq!(symbols, ["TA+", "N", "o", "+Nom", "+N"]);
}

#[test]
fn the_tokenizer_names_where_no_symbol_starts() {
    let tokenizer = AnalysisTokenizer::new(&["ŋ", "a", "+N"]);
    assert_eq!(
        tokenizer.tokenize("ŋa+N").expect("tokenizes"),
        ["ŋ", "a", "+N"]
    );
    let message = tokenizer.tokenize("ŋa+Xa").expect_err("+X").to_string();
    assert!(message.contains("character 2"), "{message}");
    assert!(tokenizer.tokenize("").expect("empty").is_empty());
}

#[test]
fn the_tokenizer_never_reads_flags_or_reserved_symbols() {
    let tokenizer = AnalysisTokenizer::new(&["@U.person.NI@", "@_EPSILON_SYMBOL_@", "a"]);
    assert!(tokenizer.tokenize("a@U.person.NI@").is_err());
    assert!(tokenizer.tokenize("@_EPSILON_SYMBOL_@").is_err());
}

// [spec:hfst:sem:prediction-units.boundary+2/test]
// [spec:hfst:sem:prediction-units.stem/test]
#[test]
fn a_listed_character_closes_a_compound_join() {
    let analysis = [
        "g", "u", "o", "l", "l", "i", "+N", "+Cmp", "#", "b", "i", "v", "d", "u", "+N", "+Sg",
    ];
    let joined = UnitGrammar::from_toml(b"tags = [\"#\"]\nboundary = [\"#\"]\n")
        .expect("test grammar reads");
    let kinds_and_text: Vec<(UnitKind, String)> = joined
        .segment(&analysis)
        .expect("analysis segments")
        .into_iter()
        .map(|unit| (unit.kind, unit.symbols.concat()))
        .collect();
    assert_eq!(
        kinds_and_text,
        units(&[
            (UnitKind::Stem, "guolli"),
            (UnitKind::Boundary, "+N+Cmp#"),
            (UnitKind::Stem, "bivdu"),
            (UnitKind::Ending, "+N+Sg"),
        ])
    );
    let unlisted = UnitGrammar::from_toml(b"boundary = [\"+Cmp\"]").expect("test grammar reads");
    let stems: Vec<String> = unlisted
        .segment(&analysis)
        .expect("analysis segments")
        .into_iter()
        .filter(|unit| unit.kind == UnitKind::Stem)
        .map(|unit| unit.symbols.concat())
        .collect();
    assert_eq!(stems, ["guolli", "#bivdu"]);
}

const DERIVING: &str = r#"
boundary = ["+Der/*"]
derivation = ["+NIQ", "+KIP", "+LI"]
enclitic = ["+LI", "+LU"]
hidden = ["+Gram/*"]
"#;

fn derived(analysis: &str) -> crate::error::Result<Vec<(UnitKind, String)>> {
    let grammar = UnitGrammar::from_toml(DERIVING.as_bytes()).expect("test grammar reads");
    let symbols = tokenizer().tokenize(analysis).expect("analysis tokenizes");
    Ok(grammar
        .segment(&symbols)?
        .into_iter()
        .map(|unit| (unit.kind, unit.symbols.concat()))
        .collect())
}

// [spec:hfst:sem:prediction-units.boundary+2/test]
#[test]
fn derivation_tags_begin_units_boundaries_end() {
    assert_eq!(
        derived("oqar+NIQ+Der/vn+N+Abs+Sg").expect("segments"),
        units(&[
            (UnitKind::Stem, "oqar"),
            (UnitKind::Boundary, "+NIQ+Der/vn"),
            (UnitKind::Ending, "+N+Abs+Sg"),
        ])
    );
    assert_eq!(
        derived("oqar+Gram/IV+NIQ+Der/vn+KIP+Der/nv+V+Ind+3Sg").expect("segments"),
        units(&[
            (UnitKind::Stem, "oqar"),
            (UnitKind::Boundary, "+Gram/IV+NIQ+Der/vn"),
            (UnitKind::Boundary, "+KIP+Der/nv"),
            (UnitKind::Ending, "+V+Ind+3Sg"),
        ])
    );
}

// [spec:hfst:sem:prediction-units.boundary+2/test]
#[test]
fn a_chain_of_derivation_tags_is_refused() {
    let message = derived("oqar+NIQ+KIP+Der/nv+V+Ind+3Sg")
        .expect_err("+KIP does not begin its unit")
        .to_string();
    assert!(
        message.contains("\"+KIP\" at symbol 5 does not begin"),
        "{message}"
    );
    let message = derived("oqar+N+NIQ+Der/vn+N+Abs+Sg")
        .expect_err("+N comes before +NIQ")
        .to_string();
    assert!(message.contains("\"+NIQ\""), "{message}");
}

// [spec:hfst:sem:prediction-units.ending+1/test]
#[test]
fn a_derivation_tag_no_boundary_ends_is_refused() {
    let message = derived("oqar+NIQ+N+Abs+Sg")
        .expect_err("no Der tag ends +NIQ")
        .to_string();
    assert!(message.contains("no boundary tag ends"), "{message}");
    let message = derived("oqar+V+Ind+NIQ+3Sg")
        .expect_err("+NIQ is in an ending")
        .to_string();
    assert!(message.contains("\"+NIQ\""), "{message}");
}

// [spec:hfst:sem:prediction-units.enclitic/test]
// [spec:hfst:sem:prediction-units.boundary+2/test]
#[test]
fn derivation_tags_may_begin_enclitics_only() {
    assert_eq!(
        derived("oqar+LI+Der/vv+V+Ind+3Sg+LI").expect("segments"),
        units(&[
            (UnitKind::Stem, "oqar"),
            (UnitKind::Boundary, "+LI+Der/vv"),
            (UnitKind::Ending, "+V+Ind+3Sg"),
            (UnitKind::Enclitic, "+LI"),
        ])
    );
    let message = derived("oqar+V+Ind+3Sg+LU+KIP")
        .expect_err("+KIP inside an enclitic")
        .to_string();
    assert!(message.contains("inside an enclitic"), "{message}");
}
