use super::pattern::Pattern;
use super::*;

fn grammar(text: &str) -> UnitGrammar {
    UnitGrammar::from_toml(text.as_bytes()).expect("test grammar reads")
}

fn unit(kind: UnitKind, symbols: &[&str]) -> Unit {
    Unit {
        kind,
        symbols: symbols.iter().map(Symbol::new).collect(),
    }
}

fn refusal(text: &str) -> String {
    UnitGrammar::from_toml(text.as_bytes())
        .expect_err("grammar is refused")
        .to_string()
}

const EVERY_KEY: &str = r#"
boundary = ["+Der/*", "+Cmp"]
prefix = ["PV/*"]
derivation = ["+NIQ", "+K*"]
enclitic = ["+Qst", "+Foc/*"]
hidden = ["+Sem/*"]

[[filter]]
name = "ni-"
label = "ni-"
flags = ["@U.person.NI@"]

[[filter]]
name = "ki-"
label = "ki-"
flags = ["@U.person.KI@", "@D.order.cnj@"]

[labels]
"+V+TV+Der/NomAg" = "-eaddji"
"guolli" = "guolli (fish)"
"+N+Sg+Nom" = "nominative"

[[class]]
name = "noun"
patterns = ["+Der/NomAg", "+N"]

[[class]]
name = "verb"
patterns = ["+V"]
"#;

// [spec:hfst:syn:prediction-units.format+2/test]
#[test]
fn a_grammar_with_every_key_reads() {
    let g = grammar(EVERY_KEY);
    assert!(g.is_boundary("+Der/NomAg"));
    assert!(g.is_boundary("+Cmp"));
    assert!(!g.is_boundary("+Cmp/SgNom"));
    assert!(g.is_prefix("PV/e+"));
    assert!(g.is_derivation("+NIQ"));
    assert!(g.is_derivation("+KIP"));
    assert!(!g.is_derivation("+N"));
    assert!(g.is_enclitic("+Foc/ge"));
    assert!(g.is_hidden("+Sem/Hum"));
    assert!(!g.is_hidden("+N"));
    assert_eq!(g.filters().len(), 2);
}

// [spec:hfst:syn:prediction-units.format+2/test]
#[test]
fn every_key_is_optional() {
    let g = grammar("");
    assert!(g.filters().is_empty());
    assert!(!g.is_boundary("+Der/vn"));
    assert_eq!(g.class_of(&unit(UnitKind::Ending, &["+N"])), None);
    g.validate::<&str>(&[])
        .expect("an empty grammar names nothing");
}

// [spec:hfst:syn:prediction-units.format+2/test]
#[test]
fn an_unknown_key_is_refused_by_name() {
    assert!(refusal("boundaries = [\"+Der/*\"]").contains("boundaries"));
    let filter = "[[filter]]\nname = \"ni-\"\nlabel = \"ni-\"\nflag = [\"@U.person.NI@\"]\n";
    assert!(refusal(filter).contains("flag"));
    let class = "[[class]]\nname = \"noun\"\npatterns = [\"+N\"]\npos = \"N\"\n";
    assert!(refusal(class).contains("pos"));
}

// [spec:hfst:syn:prediction-units.format+2/test]
#[test]
fn a_missing_field_or_wrong_type_is_refused() {
    assert!(refusal("[[filter]]\nname = \"ni-\"\nflags = []\n").contains("label"));
    assert!(refusal("[[class]]\nname = \"noun\"\n").contains("patterns"));
    assert!(refusal("hidden = \"+Sem/*\"").contains("hidden"));
    assert!(UnitGrammar::from_toml(&[0xff, 0xfe]).is_err());
}

// [spec:hfst:syn:prediction-units.format+2/test]
#[test]
fn a_tags_entry_must_be_one_character() {
    assert!(refusal("tags = [\"##\"]").contains("\"##\""));
    assert!(refusal("tags = [\"\"]").contains("one character"));
    grammar("tags = [\"#\", \"ŋ\"]");
}

// [spec:hfst:syn:prediction-units.format+2/test]
#[test]
fn a_tags_entry_the_generator_lacks_is_refused() {
    let message = grammar("tags = [\"#\"]")
        .validate(&["a", "+N"])
        .expect_err("# is missing")
        .to_string();
    assert!(message.contains("tags lists \"#\""), "{message}");
    grammar("tags = [\"#\"]")
        .validate(&["a", "#", "+N"])
        .expect("# is there");
}

// [spec:hfst:def:prediction-units.grammar+2/test]
#[test]
fn classify_makes_listed_characters_tags() {
    let g = grammar("tags = [\"#\", \"ŋ\"]");
    assert_eq!(g.classify("#"), SymbolClass::Tag);
    assert_eq!(g.classify("ŋ"), SymbolClass::Tag);
    assert_eq!(g.classify("a"), SymbolClass::Letter);
    assert_eq!(g.classify("+N"), SymbolClass::Tag);
    assert_eq!(g.classify("@U.person.NI@"), SymbolClass::Flag);
    assert_eq!(g.classify("@_EPSILON_SYMBOL_@"), SymbolClass::Reserved);
    assert_eq!(grammar("").classify("#"), SymbolClass::Letter);
}

// [spec:hfst:syn:prediction-units.format+2/test]
// [spec:hfst:sem:prediction-units.hidden/test]
#[test]
fn listed_characters_are_tags_to_every_key() {
    let text = "tags = [\"#\"]\nhidden = [\"#\"]\n[[class]]\nname = \"join\"\npatterns = [\"#\"]\n";
    let g = grammar(text);
    g.validate(&["#", "+N"]).expect("# is a tag");
    let join = unit(UnitKind::Boundary, &["+N", "#"]);
    assert_eq!(g.key(&join).as_str(), "+N");
    assert_eq!(g.class_of(&join), Some("join"));
    let plain = grammar("hidden = [\"#\"]");
    assert!(plain.validate(&["#", "+N"]).is_err());
    assert_eq!(plain.key(&join).as_str(), "+N#");
}

// [spec:hfst:syn:prediction-units.format+2/test]
#[test]
fn a_filter_flag_must_be_a_flag_diacritic() {
    let text = "[[filter]]\nname = \"ni-\"\nlabel = \"ni-\"\nflags = [\"+1Sg\"]\n";
    let message = refusal(text);
    assert!(message.contains("+1Sg"), "{message}");
    assert!(message.contains("ni-"), "{message}");
}

const ALPHABET: &[&str] = &[
    "a",
    "g",
    "@_EPSILON_SYMBOL_@",
    "@U.person.NI@",
    "@U.person.KI@",
    "@D.order.cnj@",
    "+Der/NomAg",
    "+Cmp",
    "PV/e+",
    "+NIQ",
    "+KIP",
    "+Qst",
    "+Foc/ge",
    "+Sem/Hum",
    "+N",
    "+V",
];

// [spec:hfst:syn:prediction-units.format+2/test]
#[test]
fn a_grammar_its_alphabet_fits_validates() {
    grammar(EVERY_KEY)
        .validate(ALPHABET)
        .expect("every pattern matches a tag and every flag is there");
}

// [spec:hfst:syn:prediction-units.format+2/test]
#[test]
fn a_pattern_matching_no_tag_is_refused() {
    let without_cmp: Vec<&str> = ALPHABET.iter().copied().filter(|s| *s != "+Cmp").collect();
    let message = grammar(EVERY_KEY)
        .validate(&without_cmp)
        .expect_err("+Cmp is missing")
        .to_string();
    assert!(message.contains("boundary pattern \"+Cmp\""), "{message}");

    let without_kip: Vec<&str> = ALPHABET.iter().copied().filter(|s| *s != "+KIP").collect();
    let message = grammar(EVERY_KEY)
        .validate(&without_kip)
        .expect_err("+KIP is missing")
        .to_string();
    assert!(message.contains("derivation pattern \"+K*\""), "{message}");

    let without_verbs: Vec<&str> = ALPHABET.iter().copied().filter(|s| *s != "+V").collect();
    let message = grammar(EVERY_KEY)
        .validate(&without_verbs)
        .expect_err("+V is missing")
        .to_string();
    assert!(
        message.contains("class \"verb\" pattern \"+V\""),
        "{message}"
    );
}

// [spec:hfst:syn:prediction-units.format+2/test]
#[test]
fn letters_and_flags_do_not_satisfy_a_pattern() {
    let letter = grammar("hidden = [\"a\"]");
    assert!(letter.validate(&["a", "+N"]).is_err());
    let flag = grammar("enclitic = [\"@U.person.NI@\"]");
    assert!(flag.validate(&["@U.person.NI@", "+N"]).is_err());
}

// [spec:hfst:syn:prediction-units.format+2/test]
#[test]
fn a_filter_flag_the_generator_lacks_is_refused() {
    let without_flag: Vec<&str> = ALPHABET
        .iter()
        .copied()
        .filter(|s| *s != "@D.order.cnj@")
        .collect();
    let message = grammar(EVERY_KEY)
        .validate(&without_flag)
        .expect_err("@D.order.cnj@ is missing")
        .to_string();
    assert!(message.contains("@D.order.cnj@"), "{message}");
    assert!(message.contains("ki-"), "{message}");
}

// [spec:hfst:syn:prediction-units.format+2/test]
#[test]
fn a_star_matches_any_run_of_characters() {
    let cases: &[(&str, &str, bool)] = &[
        ("+Der/*", "+Der/vn", true),
        ("+Der/*", "+Der/", true),
        ("+Der/*", "+Der", false),
        ("+Der/*", "x+Der/vn", false),
        ("+N", "+N", true),
        ("+N", "+Nom", false),
        ("*/vn", "+Der/vn", true),
        ("*/vn", "+Der/vnx", false),
        ("+*+*", "+a+b", true),
        ("a*a", "a", false),
        ("a*a", "aa", true),
        ("+Der/viđ*", "+Der/viđá", true),
        ("+Q?", "+Q?", true),
        ("+Q?", "+Qx", false),
        ("*", "", true),
        ("", "+N", false),
    ];
    for &(pattern, tag, expected) in cases {
        let pattern = Pattern::from(pattern.to_owned());
        assert_eq!(pattern.matches(tag), expected, "{pattern:?} on {tag:?}");
    }
}

// [spec:hfst:def:prediction-units.grammar+2/test]
#[test]
fn the_hash_is_the_sha256_of_the_bytes() {
    let empty_sha256 = [
        0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9,
        0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52,
        0xb8, 0x55,
    ];
    assert_eq!(grammar("").hash(), empty_sha256);
    let spaced = "boundary = [ \"+Der/*\" ]";
    assert_ne!(
        grammar(spaced).hash(),
        grammar("boundary = [\"+Der/*\"]").hash()
    );
}

// [spec:hfst:sem:prediction-units.hidden/test]
#[test]
fn hidden_tags_stay_but_leave_the_key() {
    let g = grammar(EVERY_KEY);
    let tagged = unit(UnitKind::Ending, &["+N", "+Sem/Hum", "+Sg", "+Nom"]);
    let plain = unit(UnitKind::Ending, &["+N", "+Sg", "+Nom"]);
    assert_eq!(g.key(&tagged).as_str(), "+N+Sg+Nom");
    assert_eq!(g.key(&tagged), g.key(&plain));
    assert_eq!(tagged.symbols.len(), 4);
    assert_eq!(g.label(&tagged), Label::Text("nominative".to_owned()));
}

// [spec:hfst:sem:prediction-units.hidden/test]
#[test]
fn only_tags_can_be_hidden() {
    let g = grammar("hidden = [\"*\"]");
    let stem = unit(UnitKind::Stem, &["o", "q", "a", "r"]);
    assert_eq!(g.key(&stem).as_str(), "oqar");
    let ending = unit(UnitKind::Ending, &["+V", "+Ind"]);
    assert_eq!(g.key(&ending).as_str(), "");
}

// [spec:hfst:sem:prediction-units.labels/test]
#[test]
fn a_stem_shows_its_entry_or_its_letters() {
    let g = grammar(EVERY_KEY);
    let fish = unit(UnitKind::Stem, &["g", "u", "o", "l", "l", "i"]);
    assert_eq!(g.label(&fish), Label::Text("guolli (fish)".to_owned()));
    let catch = unit(UnitKind::Stem, &["b", "i", "v", "d", "u"]);
    assert_eq!(g.label(&catch), Label::Text("bivdu".to_owned()));
}

// [spec:hfst:sem:prediction-units.labels/test]
#[test]
fn an_ending_without_an_entry_shows_its_word() {
    let g = grammar(EVERY_KEY);
    let plural = unit(UnitKind::Ending, &["+N", "+Pl", "+Nom"]);
    assert_eq!(g.label(&plural), Label::CompleteWord);
}

// [spec:hfst:sem:prediction-units.labels/test]
#[test]
fn other_units_show_their_entry_or_their_key() {
    let g = grammar(EVERY_KEY);
    let agent = unit(UnitKind::Boundary, &["+V", "+TV", "+Der/NomAg"]);
    assert_eq!(g.label(&agent), Label::Text("-eaddji".to_owned()));
    let join = unit(UnitKind::Boundary, &["+N", "+Sem/Hum", "+Cmp"]);
    assert_eq!(g.label(&join), Label::Text("+N+Cmp".to_owned()));
    let preverb = unit(UnitKind::Prefix, &["PV/e+"]);
    assert_eq!(g.label(&preverb), Label::Text("PV/e+".to_owned()));
    let question = unit(UnitKind::Enclitic, &["+Qst"]);
    assert_eq!(g.label(&question), Label::Text("+Qst".to_owned()));
}

// [spec:hfst:sem:prediction-units.filter/test]
// [spec:hfst:sem:prediction-units.labels/test]
#[test]
fn a_filter_unit_holds_its_flags_and_label() {
    let g = grammar(EVERY_KEY);
    let names: Vec<&str> = g.filters().iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["ni-", "ki-"]);
    let ki = g.filters()[1].unit();
    assert_eq!(ki.kind, UnitKind::Filter);
    assert_eq!(ki.symbols, ["@U.person.KI@", "@D.order.cnj@"]);
    assert_eq!(g.label(&ki), Label::Text("ki-".to_owned()));
    assert_eq!(g.class_of(&ki), None);
    let ni = g.filters()[0].unit();
    assert_ne!(g.key(&ni), g.key(&ki));
}

// [spec:hfst:sem:prediction-units.classes/test]
#[test]
fn a_unit_takes_the_first_matching_class() {
    let g = grammar(EVERY_KEY);
    let agent = unit(UnitKind::Boundary, &["+V", "+TV", "+Der/NomAg"]);
    assert_eq!(g.class_of(&agent), Some("noun"));
    let verb = unit(UnitKind::Ending, &["+V", "+Ind", "+Prs", "+Sg3"]);
    assert_eq!(g.class_of(&verb), Some("verb"));
    let stem = unit(UnitKind::Stem, &["g", "u", "o", "l", "l", "i"]);
    assert_eq!(g.class_of(&stem), None);
    let question = unit(UnitKind::Enclitic, &["+Qst"]);
    assert_eq!(g.class_of(&question), None);
}

// [spec:hfst:sem:prediction-units.classes/test]
#[test]
fn a_word_takes_its_latest_class() {
    let g = grammar(EVERY_KEY);
    let word = [
        unit(UnitKind::Stem, &["b", "a", "r", "g", "a", "t"]),
        unit(UnitKind::Boundary, &["+V", "+TV", "+Der/NomAg"]),
        unit(UnitKind::Ending, &["+V", "+Ind"]),
        unit(UnitKind::Enclitic, &["+Qst"]),
    ];
    assert_eq!(g.word_class(&word[..1]), None);
    assert_eq!(g.word_class(&word[..2]), Some("noun"));
    assert_eq!(g.word_class(&word), Some("verb"));
}
