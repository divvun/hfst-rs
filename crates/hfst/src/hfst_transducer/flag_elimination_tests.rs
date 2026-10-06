//! eliminate_flags / eliminate_flag: which paths survive, with what weights,
//! and that the cost follows the reachable flag states rather than every
//! combination of every feature's values.

use std::collections::BTreeMap;
use std::io::BufReader;

use super::*;
use crate::hfst_data_types::HfstTwoLevelPaths;
use hfst_openfst::StdVectorFst;

const EPSILON: &str = "@_EPSILON_SYMBOL_@";

fn from_att<B: AlgebraBackend>(att: &str) -> HfstTransducer<B> {
    let mut cursor = BufReader::new(att.as_bytes());
    let mut linecount = 0u32;
    let basic = HfstBasicTransducer::read_in_att_format(&mut cursor, "@0@", &mut linecount, false)
        .expect("AT&T fixture parses");
    HfstTransducer::new_from_basic(&basic).expect("the fixture converts to the backend")
}

// Every (input, output) string pair of an acyclic transducer with its best
// weight. Epsilons are dropped; any flag left on a path stays visible.
fn relation<B: AlgebraBackend>(t: &HfstTransducer<B>) -> BTreeMap<(String, String), f32> {
    let mut paths = HfstTwoLevelPaths::new();
    t.extract_paths(&mut paths, -1, -1)
        .expect("the result is acyclic");
    let mut best: BTreeMap<(String, String), f32> = BTreeMap::new();
    for path in &paths {
        let side = |first: bool| -> String {
            path.second
                .iter()
                .map(|(i, o)| if first { i.as_str() } else { o.as_str() })
                .filter(|s| *s != EPSILON)
                .collect()
        };
        let weight = best
            .entry((side(true), side(false)))
            .or_insert(f32::INFINITY);
        *weight = weight.min(path.first);
    }
    best
}

fn expected(pairs: &[(&str, &str, f32)]) -> BTreeMap<(String, String), f32> {
    pairs
        .iter()
        .map(|(i, o, w)| ((i.to_string(), o.to_string()), *w))
        .collect()
}

// One branch per flag sequence: 0 -> flags -> a letter pair -> state 1, final
// at weight 0.5. Branch k carries weight k * 0.25 on its letter arc, so a
// surviving branch is told apart by both its letter and its weight.
fn branches(cases: &[(&[&str], &str, &str)]) -> String {
    let mut att = String::new();
    let mut next = 2;
    for (k, (flags, input, output)) in cases.iter().enumerate() {
        let mut from = 0;
        for flag in flags.iter() {
            att.push_str(&format!("{from}\t{next}\t{flag}\t{flag}\t0\n"));
            from = next;
            next += 1;
        }
        let weight = (k + 1) as f32 * 0.25;
        att.push_str(&format!("{from}\t1\t{input}\t{output}\t{weight}\n"));
    }
    att.push_str("1\t0.5\n");
    att
}

// Each operator against the flags that make it fail or succeed. Branch
// letters: a.. in input, A.. in output; the comment gives the verdict.
const OPERATOR_CASES: &[(&[&str], &str, &str)] = &[
    (&["@P.F.a@", "@R.F.a@"], "a", "A"),            // kept
    (&["@P.F.a@", "@R.F.b@"], "b", "B"),            // R after another value
    (&["@R.F@"], "c", "C"),                         // R with nothing set
    (&["@P.F.b@", "@R.F@"], "d", "D"),              // kept
    (&["@P.F.a@", "@C.F@", "@R.F@"], "e", "E"),     // R after a clear
    (&["@P.F.a@", "@D.F.a@"], "f", "F"),            // D of the set value
    (&["@P.F.b@", "@D.F.a@"], "g", "G"),            // kept
    (&["@D.F@"], "h", "H"),                         // kept: nothing set
    (&["@P.F.a@", "@D.F@"], "i", "I"),              // D after a set
    (&["@P.F.a@", "@C.F@", "@D.F@"], "j", "J"),     // kept
    (&["@U.F.a@", "@U.F.a@"], "k", "K"),            // kept
    (&["@U.F.a@", "@U.F.b@"], "l", "L"),            // U of another value
    (&["@N.F.a@", "@U.F.a@"], "m", "M"),            // U of the negated value
    (&["@N.F.a@", "@U.F.b@"], "n", "N"),            // kept
    (&["@N.F.a@", "@D.F.a@"], "o", "O"),            // kept
    (&["@P.F.a@", "@U.F.b@"], "p", "P"),            // U after another value
    (&["@P.G.a@", "@R.F.a@"], "q", "Q"),            // G does not set F
    (&["@U.F.b@", "@R.F.b@"], "r", "R"),            // kept: U sets
    (&["@U.F.a@", "@P.F.b@", "@R.F.b@"], "s", "S"), // kept: P overrides
    (&["@N.F.a@", "@R.F@"], "t", "T"),              // kept: N sets
    (&["@C.F@", "@U.F.b@"], "u", "U"),              // kept
    (&["@P.G.b@", "@D.G.b@"], "v", "V"),            // D of the set value
];

fn operator_survivors() -> BTreeMap<(String, String), f32> {
    expected(&[
        ("a", "A", 0.75),
        ("d", "D", 1.5),
        ("g", "G", 2.25),
        ("h", "H", 2.5),
        ("j", "J", 3.0),
        ("k", "K", 3.25),
        ("n", "N", 4.0),
        ("o", "O", 4.25),
        ("r", "R", 5.0),
        ("s", "S", 5.25),
        ("t", "T", 5.5),
        ("u", "U", 5.75),
    ])
}

fn check_every_operator<B: AlgebraBackend>(weighted: bool) {
    let mut t: HfstTransducer<B> = from_att(&branches(OPERATOR_CASES));
    t.eliminate_flags().expect("eliminate_flags");
    let got = relation(&t);
    let want = operator_survivors();
    if weighted {
        assert_eq!(got, want);
    } else {
        assert!(got.keys().eq(want.keys()), "got {got:?}");
    }
    assert!(
        !t.has_flag_diacritics(),
        "no flag may survive in the alphabet"
    );
}

// [spec:hfst:req:flag-elimination.relation/test]
#[test]
fn every_operator_keeps_the_consistent_paths() {
    check_every_operator::<StdVectorFst>(true);
}

// [spec:hfst:req:flag-elimination.relation/test]
// Foma is unweighted, so only the string pairs are compared.
#[cfg(feature = "foma")]
#[test]
fn foma_keeps_the_same_consistent_paths() {
    check_every_operator::<crate::backend_foma::FomaTransducer>(false);
}

// The input tape and the output tape are filtered separately, and an arc
// with an eliminated flag on either side becomes an epsilon arc.
// [spec:hfst:req:flag-elimination.relation/test]
#[test]
fn each_tape_is_checked_on_its_own() {
    let att = concat!(
        "0\t2\t@P.F.a@\t@P.F.b@\t0\n",
        "2\t3\t@R.F.a@\t@R.F.b@\t0\n",
        "3\t1\tx\tX\t0.25\n",
        "0\t4\t@P.F.a@\t@P.F.a@\t0\n",
        "4\t5\t@R.F.a@\t@R.F.b@\t0\n",
        "5\t1\ty\tY\t0.5\n",
        "0\t6\t@P.F.a@\t@0@\t0\n",
        "6\t7\t@0@\t@R.F.a@\t0\n",
        "7\t1\tz\tZ\t0.75\n",
        "0\t8\t@P.F.a@\t@0@\t0\n",
        "8\t9\t@R.F.a@\t@0@\t0\n",
        "9\t1\tw\tW\t1\n",
        "0\t10\t@P.F.a@\tv\t0\n",
        "10\t1\tu\tU\t1.25\n",
        "1\t0\n",
    );
    let mut t: HfstTransducer<StdVectorFst> = from_att(att);
    t.eliminate_flags().expect("eliminate_flags");
    assert_eq!(
        relation(&t),
        expected(&[("x", "X", 0.25), ("w", "W", 1.0), ("u", "U", 1.25)])
    );
}

// Eliminating one feature filters on that feature only: flags of the others
// stay on their paths, consistent or not.
// [spec:hfst:req:flag-elimination.relation/test]
#[test]
fn eliminate_flag_leaves_the_other_features() {
    let att = concat!(
        "0\t2\t@P.F.a@\t@P.F.a@\t0\n",
        "2\t3\t@P.G.a@\t@P.G.a@\t0\n",
        "3\t4\t@R.F.a@\t@R.F.a@\t0\n",
        "4\t5\t@R.G.b@\t@R.G.b@\t0\n",
        "5\t1\ta\tA\t0.25\n",
        "0\t6\t@P.F.a@\t@P.F.a@\t0\n",
        "6\t7\t@R.F.b@\t@R.F.b@\t0\n",
        "7\t1\tb\tB\t0.5\n",
        "1\t0\n",
    );
    let base: HfstTransducer<StdVectorFst> = from_att(att);

    let mut f_only = base.clone();
    f_only.eliminate_flag("F").expect("eliminate_flag F");
    assert_eq!(
        relation(&f_only),
        expected(&[("@P.G.a@@R.G.b@a", "@P.G.a@@R.G.b@A", 0.25)])
    );

    let mut both = f_only.clone();
    both.eliminate_flag("G").expect("eliminate_flag G");
    let mut all = base.clone();
    all.eliminate_flags().expect("eliminate_flags");
    assert!(relation(&both).is_empty(), "G then refuses the last path");
    assert!(
        both.compare_default(&all).expect("compare"),
        "one feature at a time must equal all at once"
    );

    assert!(base.clone().eliminate_flag("H").is_err());
    assert!(base.clone().eliminate_flag("F.a").is_err());
}

// A cycle that re-reads its flags: (P.F.a a | P.F.b b) R.F.a c, repeated.
// Only the a branch passes, every time round.
// [spec:hfst:req:flag-elimination.relation/test]
#[test]
fn cycle_flags_are_checked_every_round() {
    let att = concat!(
        "0\t1\t@P.F.a@\t@P.F.a@\t0\n",
        "0\t2\t@P.F.b@\t@P.F.b@\t0\n",
        "1\t3\ta\ta\t0.25\n",
        "2\t3\tb\tb\t0.25\n",
        "3\t4\t@R.F.a@\t@R.F.a@\t0\n",
        "4\t0\tc\tc\t0.5\n",
        "0\t0\n",
    );
    let mut t: HfstTransducer<StdVectorFst> = from_att(att);
    t.eliminate_flags().expect("eliminate_flags");
    let mut want: HfstTransducer<StdVectorFst> =
        from_att("0\t1\ta\ta\t0.25\n1\t0\tc\tc\t0.5\n0\t0\n");
    want.minimize().expect("minimize");
    assert!(t.compare_default(&want).expect("compare"));
}

// Twelve features, each with four U values and an R check, on one path. A
// filter over every combination of their values has some 5^12 states; the
// walk meets one combination per state of the path.
// [spec:hfst:req:flag-elimination.reachable-product/test]
#[test]
fn independent_features_do_not_multiply() {
    let mut att = String::new();
    let mut state = 0;
    for feature in 0..12 {
        let value = feature % 4;
        // A branch that sets the wrong value and dies at the R check, and the
        // branch that survives; both rejoin before the check.
        att.push_str(&format!(
            "{state}\t{}\t@U.F{feature}.v{value}@\t@U.F{feature}.v{value}@\t0\n",
            state + 1
        ));
        let wrong = (value + 1) % 4;
        att.push_str(&format!(
            "{state}\t{}\t@U.F{feature}.v{wrong}@\t@U.F{feature}.v{wrong}@\t0\n",
            state + 1
        ));
        for other in 0..4 {
            if other != value && other != wrong {
                att.push_str(&format!(
                    "{}\t{}\t@U.F{feature}.v{other}@\t@U.F{feature}.v{other}@\t0\n",
                    state + 1,
                    state + 1
                ));
            }
        }
        att.push_str(&format!(
            "{}\t{}\t@R.F{feature}.v{value}@\t@R.F{feature}.v{value}@\t0\n",
            state + 1,
            state + 2
        ));
        att.push_str(&format!("{}\t{}\tx\ty\t0.25\n", state + 2, state + 3));
        state += 3;
    }
    att.push_str(&format!("{state}\t0\n"));

    let mut t: HfstTransducer<StdVectorFst> = from_att(&att);
    t.eliminate_flags().expect("eliminate_flags");
    assert_eq!(
        relation(&t),
        expected(&[("x".repeat(12).as_str(), "y".repeat(12).as_str(), 3.0)])
    );
}
