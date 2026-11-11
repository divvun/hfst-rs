//! The kal, sme and crk unit grammars in tests/fixtures/prediction, applied
//! to analyses their generators accept. The tests build small alphabets
//! from the tags these analyses use: the real generators are not in the
//! repository.

use hfst::prediction::units::{AnalysisTokenizer, Label, UnitGrammar, UnitKind};

const KAL: &[u8] = include_bytes!("fixtures/prediction/kal.toml");
const SME: &[u8] = include_bytes!("fixtures/prediction/sme.toml");
const CRK: &[u8] = include_bytes!("fixtures/prediction/crk.toml");

const KAL_TAGS: &[&str] = &[
    "+NIQ",
    "+KIP",
    "+LI",
    "+LU",
    "+TAQ",
    "+SSAQ",
    "+UTE",
    "+VIK",
    "+Der/vn",
    "+Der/nv",
    "+Der/vv",
    "+Der/nn",
    "+Gram/IV",
    "+Gram/Dem",
    "+N",
    "+V",
    "+Pron",
    "+Ind",
    "+Abs",
    "+Sg",
    "+3Sg",
    "+1SgPoss",
    "TA+",
];

const SME_TAGS: &[&str] = &[
    "+Cmp/SplitR",
    "+N",
    "+V",
    "+A",
    "+TV",
    "+IV",
    "+Pron",
    "+Indef",
    "+Der/NomAg",
    "+Der/Comp",
    "+Sem/Ani-fish",
    "+Cmp/SgNom",
    "+Cmp",
    "+Sg",
    "+Pl",
    "+Nom",
    "+Acc",
    "+Ind",
    "+Prs",
    "+Sg3",
    "+Qst",
    "+Foc/Pos-ge",
];

const CRK_TAGS: &[&str] = &[
    "PV/ki+",
    "PV/nitawi+",
    "RdplW+",
    "IC+",
    "+V",
    "+N",
    "+AI",
    "+TI",
    "+A",
    "+Ind",
    "+Cnj",
    "+Prs",
    "+1Sg",
    "+2Sg",
    "+Sg",
    "+Der/Dim",
];

fn cut(grammar: &[u8], tags: &[&str], analysis: &str) -> Vec<(UnitKind, String)> {
    let grammar = UnitGrammar::from_toml(grammar).expect("fixture reads");
    let mut alphabet: Vec<String> = tags.iter().map(|t| (*t).to_owned()).collect();
    alphabet.extend(analysis.chars().map(String::from));
    let symbols = AnalysisTokenizer::new(&alphabet)
        .tokenize(analysis)
        .expect("analysis tokenizes");
    grammar
        .segment(&symbols)
        .expect("analysis segments")
        .iter()
        .map(|unit| (unit.kind, grammar.key(unit).as_str().to_owned()))
        .collect()
}

fn units(expected: &[(UnitKind, &str)]) -> Vec<(UnitKind, String)> {
    expected
        .iter()
        .map(|&(kind, key)| (kind, key.to_owned()))
        .collect()
}

// [spec:hfst:syn:prediction-units.format+1/test]
// [spec:hfst:def:prediction-units.grammar+1/test]
#[test]
fn the_fixture_grammars_read_and_hash_apart() {
    let hashes: Vec<[u8; 32]> = [KAL, SME, CRK]
        .iter()
        .map(|bytes| UnitGrammar::from_toml(bytes).expect("fixture reads").hash())
        .collect();
    assert_ne!(hashes[0], hashes[1]);
    assert_ne!(hashes[1], hashes[2]);
    assert_ne!(hashes[0], hashes[2]);
}

// [spec:hfst:sem:prediction-units.boundary+1/test]
// [spec:hfst:sem:prediction-units.ending/test]
#[test]
fn kal_cuts_a_derivation_before_its_ending() {
    assert_eq!(
        cut(KAL, KAL_TAGS, "oqar+NIQ+Der/vn+N+Abs+Sg+1SgPoss"),
        units(&[
            (UnitKind::Stem, "oqar"),
            (UnitKind::Boundary, "+NIQ+Der/vn"),
            (UnitKind::Ending, "+N+Abs+Sg+1SgPoss"),
        ])
    );
}

// [spec:hfst:sem:prediction-units.boundary+1/test]
// [spec:hfst:sem:prediction-units.hidden/test]
#[test]
fn kal_chains_derivations_and_hides_transitivity() {
    assert_eq!(
        cut(
            KAL,
            KAL_TAGS,
            "neri+Gram/IV+TAQ+Der/vn+SSAQ+Der/nn+UTE+Der/nn+LI+Der/nv+VIK+Der/vn+N+Abs+Sg"
        ),
        units(&[
            (UnitKind::Stem, "neri"),
            (UnitKind::Boundary, "+TAQ+Der/vn"),
            (UnitKind::Boundary, "+SSAQ+Der/nn"),
            (UnitKind::Boundary, "+UTE+Der/nn"),
            (UnitKind::Boundary, "+LI+Der/nv"),
            (UnitKind::Boundary, "+VIK+Der/vn"),
            (UnitKind::Ending, "+N+Abs+Sg"),
        ])
    );
}

// [spec:hfst:sem:prediction-units.enclitic/test]
// [spec:hfst:sem:prediction-units.boundary+1/test]
#[test]
fn kal_li_is_derivation_or_enclitic_by_place() {
    assert_eq!(
        cut(
            KAL,
            KAL_TAGS,
            "aak+KIP+Der/nv+Gram/IV+LI+Der/vv+Gram/IV+V+Ind+3Sg"
        ),
        units(&[
            (UnitKind::Stem, "aak"),
            (UnitKind::Boundary, "+KIP+Der/nv"),
            (UnitKind::Boundary, "+LI+Der/vv"),
            (UnitKind::Ending, "+V+Ind+3Sg"),
        ])
    );
    assert_eq!(
        cut(KAL, KAL_TAGS, "oqar+Gram/IV+Gram/IV+V+Ind+3Sg+LI"),
        units(&[
            (UnitKind::Stem, "oqar"),
            (UnitKind::Ending, "+V+Ind+3Sg"),
            (UnitKind::Enclitic, "+LI"),
        ])
    );
}

// [spec:hfst:sem:prediction-units.prefix/test]
#[test]
fn kal_reads_a_demonstrative_prefix() {
    assert_eq!(
        cut(KAL, KAL_TAGS, "TA+una+Gram/Dem+Pron+Abs+Sg"),
        units(&[
            (UnitKind::Prefix, "TA+"),
            (UnitKind::Stem, "una"),
            (UnitKind::Ending, "+Pron+Abs+Sg"),
        ])
    );
}

// [spec:hfst:sem:prediction-units.classes/test]
#[test]
fn kal_classes_derivations_by_what_they_make() {
    let grammar = UnitGrammar::from_toml(KAL).expect("fixture reads");
    let tokenizer = AnalysisTokenizer::new(&[KAL_TAGS, &["a", "k"]].concat());
    let symbols = tokenizer
        .tokenize("aak+KIP+Der/nv+Gram/IV+NIQ+Der/vn+N+Abs+Sg")
        .expect("analysis tokenizes");
    let units = grammar.segment(&symbols).expect("analysis segments");
    let classes: Vec<Option<&str>> = units.iter().map(|u| grammar.class_of(u)).collect();
    assert_eq!(classes, [None, Some("verb"), Some("noun"), Some("noun")]);
    assert_eq!(grammar.word_class(&units[..2]), Some("verb"));
}

// [spec:hfst:sem:prediction-units.boundary+1/test]
// [spec:hfst:sem:prediction-units.classes/test]
#[test]
fn sme_cuts_and_classes_an_agent_noun() {
    assert_eq!(
        cut(SME, SME_TAGS, "bargat+V+TV+Der/NomAg+N+Sg+Nom"),
        units(&[
            (UnitKind::Stem, "bargat"),
            (UnitKind::Boundary, "+V+TV+Der/NomAg"),
            (UnitKind::Ending, "+N+Sg+Nom"),
        ])
    );
    let grammar = UnitGrammar::from_toml(SME).expect("fixture reads");
    let tokenizer = AnalysisTokenizer::new(&[SME_TAGS, &["b", "a", "r", "g", "t"]].concat());
    let symbols = tokenizer
        .tokenize("bargat+V+TV+Der/NomAg+N+Sg+Nom")
        .expect("analysis tokenizes");
    let units = grammar.segment(&symbols).expect("analysis segments");
    assert_eq!(grammar.class_of(&units[1]), Some("noun"));
    assert_eq!(grammar.label(&units[2]), Label::CompleteWord);
}

// [spec:hfst:sem:prediction-units.boundary+1/test]
// [spec:hfst:sem:prediction-units.stem/test]
// [spec:hfst:sem:prediction-units.hidden/test]
#[test]
fn sme_cuts_a_compound_at_its_join() {
    assert_eq!(
        cut(
            SME,
            SME_TAGS,
            "guolli+N+Sem/Ani-fish+Cmp/SgNom+Cmp#bivdu+N+Sg+Nom"
        ),
        units(&[
            (UnitKind::Stem, "guolli"),
            (UnitKind::Boundary, "+N+Cmp/SgNom+Cmp#"),
            (UnitKind::Stem, "bivdu"),
            (UnitKind::Ending, "+N+Sg+Nom"),
        ])
    );
}

// [spec:hfst:sem:prediction-units.ending/test]
#[test]
fn sme_ends_a_split_compound_with_no_join() {
    assert_eq!(
        cut(SME, SME_TAGS, "álbmot+N+Cmp/SgNom+Cmp/SplitR+Cmp"),
        units(&[
            (UnitKind::Stem, "álbmot"),
            (UnitKind::Ending, "+N+Cmp/SgNom+Cmp/SplitR+Cmp"),
        ])
    );
}

// [spec:hfst:sem:prediction-units.enclitic/test]
#[test]
fn sme_reads_question_and_focus_clitics() {
    assert_eq!(
        cut(SME, SME_TAGS, "boahtit+V+IV+Ind+Prs+Sg3+Qst"),
        units(&[
            (UnitKind::Stem, "boahtit"),
            (UnitKind::Ending, "+V+IV+Ind+Prs+Sg3"),
            (UnitKind::Enclitic, "+Qst"),
        ])
    );
    assert_eq!(
        cut(SME, SME_TAGS, "muhtin+Pron+Indef+Pl+Acc+Foc/Pos-ge"),
        units(&[
            (UnitKind::Stem, "muhtin"),
            (UnitKind::Ending, "+Pron+Indef+Pl+Acc"),
            (UnitKind::Enclitic, "+Foc/Pos-ge"),
        ])
    );
}

// [spec:hfst:sem:prediction-units.prefix/test]
// [spec:hfst:sem:prediction-units.classes/test]
#[test]
fn crk_reads_each_preverb_as_a_prefix() {
    assert_eq!(
        cut(
            CRK,
            CRK_TAGS,
            "PV/ki+PV/nitawi+kiskinwahamâkosiw+V+AI+Ind+1Sg"
        ),
        units(&[
            (UnitKind::Prefix, "PV/ki+"),
            (UnitKind::Prefix, "PV/nitawi+"),
            (UnitKind::Stem, "kiskinwahamâkosiw"),
            (UnitKind::Ending, "+V+AI+Ind+1Sg"),
        ])
    );
    assert_eq!(
        cut(CRK, CRK_TAGS, "IC+kiskêyihtam+V+TI+Cnj+Prs+2Sg"),
        units(&[
            (UnitKind::Prefix, "IC+"),
            (UnitKind::Stem, "kiskêyihtam"),
            (UnitKind::Ending, "+V+TI+Cnj+Prs+2Sg"),
        ])
    );
    assert_eq!(
        cut(CRK, CRK_TAGS, "RdplW+ayâw+V+AI+Ind+1Sg"),
        units(&[
            (UnitKind::Prefix, "RdplW+"),
            (UnitKind::Stem, "ayâw"),
            (UnitKind::Ending, "+V+AI+Ind+1Sg"),
        ])
    );
}

// [spec:hfst:sem:prediction-units.boundary+1/test]
#[test]
fn crk_cuts_a_diminutive() {
    assert_eq!(
        cut(CRK, CRK_TAGS, "nâpêw+N+A+Der/Dim+N+A+Sg"),
        units(&[
            (UnitKind::Stem, "nâpêw"),
            (UnitKind::Boundary, "+N+A+Der/Dim"),
            (UnitKind::Ending, "+N+A+Sg"),
        ])
    );
}

// [spec:hfst:sem:prediction-units.filter/test]
// [spec:hfst:syn:prediction-units.format+1/test]
#[test]
fn crk_filters_set_the_person_flags() {
    let grammar = UnitGrammar::from_toml(CRK).expect("fixture reads");
    let filters = grammar.filters();
    assert_eq!(filters.len(), 2);
    assert_eq!(filters[0].flags, ["@U.person.NI@"]);
    assert_eq!(filters[1].flags, ["@U.person.KI@"]);
    assert_eq!(
        grammar.label(&filters[0].unit()),
        Label::Text("ni-".to_owned())
    );

    let mut alphabet: Vec<&str> = CRK_TAGS.to_vec();
    alphabet.extend(["+II", "+TA", "RdplS+", "@U.person.NI@", "@U.person.KI@"]);
    grammar
        .validate(&alphabet)
        .expect("every tag and flag is there");
    alphabet.retain(|symbol| *symbol != "@U.person.KI@");
    let message = grammar
        .validate(&alphabet)
        .expect_err("the ki- flag is missing")
        .to_string();
    assert!(message.contains("@U.person.KI@"), "{message}");
}
