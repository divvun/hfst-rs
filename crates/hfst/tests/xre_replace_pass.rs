//! The replace-pass compile of a parallel optional replace rule `P` against
//! the classic compile of `?* P ?*`.
//!
//! Both are looked up on every string up to a small length over each rule
//! set's alphabet plus `z`, a symbol no rule mentions, and must give the same
//! outputs at the same least weights.

use std::collections::{BTreeMap, BTreeSet};

use hfst::hfst_data_types::{HfstTwoLevelPaths, Symbol};
use hfst::hfst_symbol_defs::{StringVector, internal_epsilon, internal_identity};
use hfst::hfst_transducer::HfstTransducer;
use hfst::xre::XreCompiler;
use hfst_openfst::StdVectorFst;

type Fst = HfstTransducer<StdVectorFst>;

fn classic(rule: &str) -> Fst {
    XreCompiler::<StdVectorFst>::new()
        .compile(&format!("?* {rule} ?*"))
        .unwrap_or_else(|| panic!("classic compile of {rule:?} failed"))
}

fn pass(rule: &str) -> Option<Fst> {
    let mut compiler = XreCompiler::<StdVectorFst>::new();
    compiler.set_replace_pass(true);
    compiler.compile(rule)
}

/// Every output of `t` for `word`, at its least weight.
fn relation(t: &Fst, word: &[&str]) -> BTreeMap<String, f32> {
    let basic = t.to_basic().expect("to_basic");
    let path: StringVector = word.iter().map(Symbol::new).collect();
    let mut paths: HfstTwoLevelPaths = BTreeSet::new();
    basic.lookup(&path, &mut paths, Some(0), None, -1, false);
    let mut outputs = BTreeMap::new();
    for p in paths {
        let output: String = p
            .second
            .iter()
            .map(|(i, o)| match o.as_str() {
                o if o == internal_epsilon => "",
                o if o == internal_identity => i.as_str(),
                o => o,
            })
            .collect();
        let least = outputs.entry(output).or_insert(f32::INFINITY);
        *least = least.min(p.first);
    }
    outputs
}

/// All strings of length 1 to `max` over `symbols`.
fn words<'a>(symbols: &[&'a str], max: usize) -> Vec<Vec<&'a str>> {
    let mut all: Vec<Vec<&str>> = vec![vec![]];
    let mut out = Vec::new();
    for _ in 0..max {
        all = all
            .iter()
            .flat_map(|w| {
                symbols.iter().map(move |s| {
                    let mut w = w.clone();
                    w.push(s);
                    w
                })
            })
            .collect();
        out.extend(all.iter().cloned());
    }
    out
}

fn assert_same_relation(rule: &str, symbols: &[&str], max: usize) {
    let a = classic(rule);
    let b = pass(rule).unwrap_or_else(|| panic!("pass compile of {rule:?} failed"));
    for word in words(symbols, max) {
        let (x, y) = (relation(&a, &word), relation(&b, &word));
        let same = x.len() == y.len()
            && x.iter()
                .all(|(k, w)| y.get(k).is_some_and(|v| (w - v).abs() < 1e-4));
        assert!(same, "{rule}\non {word:?}\nclassic {x:?}\npass    {y:?}");
    }
}

// [spec:hfst:req:xre-replace-pass.relation/test]
#[test]
fn groups_epsilon_outputs_and_last_rewrites() {
    assert_same_relation(
        "[ {ab} (->) x::1 , b (->) 0::2 ,, {ba} (->) {yy}::3 || _ .#. ,, a (->) b::0.5 ]",
        &["a", "b", "x", "z"],
        5,
    );
}

// [spec:hfst:req:xre-replace-pass.relation/test]
#[test]
fn overlapping_and_adjacent_left_hand_sides() {
    assert_same_relation(
        "[ {ab} (->) x::1 , {bc} (->) y::2 , b (->) z::0.5 , {abc} (->) w::3 , c (->) 0::0.25 ]",
        &["a", "b", "c", "z"],
        5,
    );
}

// [spec:hfst:req:xre-replace-pass.relation/test]
#[test]
fn sides_that_are_not_strings() {
    assert_same_relation(
        "[ [a|b] (->) c::1 , a+ (->) d::2 , {ab} (->) [x|y]::1 ,, [b c]* b (->) {yx}::0.5 ]",
        &["a", "b", "c", "z"],
        5,
    );
}

// [spec:hfst:req:xre-replace-pass.relation/test]
#[test]
fn weights_on_either_side_and_least_weight_wins() {
    assert_same_relation(
        "[ a::1 (->) b::2 , [{ab}]::0.5 (->) c , b (->) [a::0.25 | c::0.75] ,, {cc} (->) a || _ .#. ]",
        &["a", "b", "c", "z"],
        5,
    );
    assert_same_relation(
        "[ a (->) b::1 , a (->) b::2 , a (->) c::1 ,, a (->) b::0.5 || _ .#. ,, {aa} (->) {bb}::1 ]",
        &["a", "b", "c", "z"],
        5,
    );
}

// [spec:hfst:req:xre-replace-pass.relation/test]
#[test]
fn any_symbol_in_a_rule() {
    assert_same_relation(
        "[ ? (->) 0::5 , [? - a] (->) b::1 , a (->) {ab}::2 ]",
        &["a", "b", "z"],
        5,
    );
}

// [spec:hfst:req:xre-replace-pass.boundary-contexts/test]
#[test]
fn first_only_and_either_end_rewrites() {
    assert_same_relation(
        "[ a (->) b::1 || .#. _ ,, b (->) c::2 ,, {ab} (->) x::3 || .#. _ .#. ,, c (->) a::1 || .#. _ , _ .#. ,, {ba} (->) y::4 || _ .#. ]",
        &["a", "b", "c", "z"],
        5,
    );
}

// [spec:hfst:req:xre-replace-pass.boundary-contexts/test]
#[test]
fn boundary_ends_the_pass_not_the_word() {
    let t = pass("[ {beal} (->) {bealde}::1 || _ .#. ]").expect("pass compile");
    let word: Vec<String> = "bealgo".chars().map(String::from).collect();
    let word: Vec<&str> = word.iter().map(String::as_str).collect();
    let out = relation(&t, &word);
    assert_eq!(out.get("bealdego"), Some(&1.0), "{out:?}");
    assert_eq!(out.get("bealgo"), Some(&0.0), "{out:?}");
    assert_eq!(out.len(), 2, "{out:?}");
}

// [spec:hfst:req:xre-replace-pass.shape/test]
#[test]
fn pass_is_loops_and_minimised_bodies() {
    let rule = "[ {ab} (->) x::1 , b (->) 0::2 ,, {ba} (->) {yy}::3 || _ .#. ]";
    let t = pass(rule).expect("pass compile");
    // Start and end states with copying loops, the anywhere body (a:x b:0,
    // b:0 sharing a state: 3 states) and the last-rewrite body (3 states).
    assert_eq!(t.number_of_states(), 8);
    let basic = t.to_basic().expect("to_basic");
    let coder = basic.coder();
    let epsilons = basic
        .states_and_transitions()
        .iter()
        .flatten()
        .filter(|tr| tr.get_input_symbol(coder).as_str() == internal_epsilon)
        .filter(|tr| tr.get_output_symbol(coder).as_str() == internal_epsilon)
        .count();
    // Into and out of each of the two bodies: the loops are not determinised
    // against the rules.
    assert_eq!(epsilons, 4);
}

// [spec:hfst:req:xre-replace-pass.refusals/test]
#[test]
fn everything_else_is_refused() {
    for rule in [
        "[ a -> b ]",
        "[ a (->) b , c @-> d ]",
        "[ a (->) b || c _ ]",
        "[ a (->) b || _ c , _ .#. ]",
        "[ a (->) b // _ .#. ]",
        "[ a (->) b ... c ]",
        "[ 0 (->) a ]",
        "[ a* (->) b ]",
        "[ a .#. (->) b ]",
        "[ [a - a] (->) b ]",
        "[ a:b (->) c ]",
        "[ \"@U.x.y@\" a (->) b ]",
        "a b",
        "[ a (->) b ] c",
        "[ a (->) b ]::1",
    ] {
        assert!(pass(rule).is_none(), "{rule:?} was not refused");
    }
}
