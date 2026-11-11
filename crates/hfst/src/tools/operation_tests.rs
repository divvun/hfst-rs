//! What the operation tools decide: conversions between types, the names
//! they give their results, kill-paths' symbols and reweight's rules.

use super::conversion::{ConversionNote, conversion_needed, convert_to_common_type};
use super::kill_paths::{kill_paths, kill_symbols};
use super::metadata::{hfst_set_formula, hfst_set_name, hfst_set_name_maybe_truncate};
use super::reweight::{Reweighting, TsvError, name_reweighted, weight_function};
use super::test_support::{is_language, paths_of, read_all, regex, stream_bytes};
use crate::hfst_data_types::ImplementationType::{
    FOMA_TYPE, HFST_OL_TYPE, HFST_OLW_TYPE, TROPICAL_OPENFST_TYPE,
};
use crate::hfst_transducer::AnyTransducer;

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn conversions_go_to_the_type_that_loses_nothing() {
    assert_eq!(conversion_needed(FOMA_TYPE, FOMA_TYPE), None);
    let note = conversion_needed(TROPICAL_OPENFST_TYPE, FOMA_TYPE).expect("types differ");
    assert_eq!(note, ConversionNote::SecondToFirst(TROPICAL_OPENFST_TYPE));
    assert_eq!(note.target(), TROPICAL_OPENFST_TYPE);
    assert_eq!(note.output_choice(), "using former type as output");
    assert_eq!(
        note.to_string(),
        "transducers have different types, converting to format OpenFST, std arc, tropical semiring\n"
    );
    let note = conversion_needed(FOMA_TYPE, TROPICAL_OPENFST_TYPE).expect("types differ");
    assert_eq!(note.target(), TROPICAL_OPENFST_TYPE);
}

// [spec:hfst:req:embed.tools-in-library/test]
#[cfg(feature = "foma")]
#[test]
fn a_pair_converts_to_one_type() {
    let tropical = AnyTransducer::Tropical(regex("a b"));
    let foma = AnyTransducer::Tropical(regex("c"))
        .convert(FOMA_TYPE, "")
        .expect("foma");
    let (first, second) = convert_to_common_type(foma, tropical).expect("converted");
    assert_eq!(first.get_type(), TROPICAL_OPENFST_TYPE);
    assert_eq!(second.get_type(), TROPICAL_OPENFST_TYPE);
    assert!(is_language(&first, "c"));
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn compiled_results_take_the_stream_type() {
    for ty in [HFST_OLW_TYPE, HFST_OL_TYPE, TROPICAL_OPENFST_TYPE] {
        let bytes = stream_bytes(ty, |out| {
            out.write_converted(&regex("a b")).expect("written");
        });
        let read = read_all(&bytes);
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].get_type(), ty);
    }
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn names_are_cut_on_a_character_boundary() {
    let mut t = regex("a");
    hfst_set_name(&mut t, "x", "op");
    assert_eq!(t.get_name(), "op(x)");
    let long = "é".repeat(600);
    hfst_set_name_maybe_truncate(&mut t, &long);
    let name = t.get_name();
    assert!(
        name.starts_with("truncated(é") && name.ends_with("...)"),
        "{name}"
    );
    assert_eq!(name.len(), "truncated(...)".len() + 1000);
    hfst_set_formula(&mut t, "", "L");
    assert_eq!(t.get_property("formulaic-definition"), "L U8");
    hfst_set_formula(&mut t, "root.lexc", "L");
    assert_eq!(t.get_property("formulaic-definition"), "L r");
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn kill_paths_removes_each_listed_symbol() {
    assert_eq!(kill_symbols("# note\n\nb\nc"), ["b", "c"]);
    let mut t = regex("[a | b | c] d");
    t.set_name("in");
    let mut heard = Vec::new();
    let killed = kill_paths(t, ["b", "c"], &mut |s| heard.push(s.to_string())).expect("killed");
    assert_eq!(heard, ["b", "c"]);
    assert!(is_language(&AnyTransducer::Tropical(killed.clone()), "a d"));
    // kill_paths builds a new transducer, so the name is gone, as in the tool.
    assert_eq!(killed.get_name(), "pathkill(UNNAMED)");
    assert_eq!(killed.get_property("formulaic-definition"), "PK .");
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn reweight_rules_select_arcs_and_final_states() {
    let rule = Reweighting {
        multiplier: 2.0,
        addition: 1.0,
        ..Reweighting::default()
    };
    assert_eq!(rule.reweight(3.0, Some("a"), Some("a")), 7.0);
    assert_eq!(rule.reweight(3.0, None, None), 7.0);
    let arcs_only = Reweighting {
        arcs_only: true,
        ..rule.clone()
    };
    assert_eq!(arcs_only.reweight(3.0, None, None), 3.0);
    let on_b = Reweighting {
        symbol: Some("b".into()),
        ..rule.clone()
    };
    assert_eq!(on_b.reweight(3.0, Some("a"), Some("a")), 3.0);
    let bounded = Reweighting {
        upper_bound: 2.0,
        ..rule
    };
    assert_eq!(bounded.reweight(3.0, Some("a"), Some("a")), 3.0);
    assert_eq!(weight_function("floor").map(|f| f(2.7)), Some(2.0));
    assert!(weight_function("nope").is_none());
}

// [spec:hfst:req:embed.tools-in-library/test]
#[test]
fn reweight_tsv_rules_carry_over() {
    let mut rule = Reweighting::default();
    assert!(rule.next_tsv_rule("# comment\n").expect("read").is_none());
    let read = rule.next_tsv_rule("a\t+2\n").expect("read").cloned();
    assert_eq!(
        read.map(|r| (r.symbol, r.addition)),
        Some((Some("a".into()), 2.0))
    );
    let read = rule.next_tsv_rule("b\t3\n").expect("read").cloned();
    assert_eq!(
        read.map(|r| (r.symbol, r.addition, r.multiplier)),
        Some((Some("b".into()), 2.0, 3.0))
    );
    assert!(matches!(rule.next_tsv_rule("b 3\n"), Err(TsvError::NoTab)));
    let bad = rule.next_tsv_rule("b\tx\n").err().map(|e| e.to_string());
    assert_eq!(bad, Some("x not a weight".into()));

    let mut t = regex("a::1 | b::2");
    let only_a = Reweighting {
        symbol: Some("a".into()),
        addition: 10.0,
        ..Reweighting::default()
    };
    only_a.apply(&mut t).expect("reweighted");
    // A symbol selects arcs only: every final weight is reweighted too.
    assert_eq!(paths_of(&t), ["a:a 21", "b:b 12"]);
    name_reweighted(&mut t);
    assert_eq!(t.get_name(), "reweight(UNNAMED)");
}
