//! Regression + behaviour locks for two successor fixes/findings:
//!
//!   * hfst/hfst#335 — the pmatch optimized-lookup runtime emitted DUPLICATE
//!     Location results. One accepting configuration can be reached through
//!     several structurally distinct paths (e.g. a union branch carrying an
//!     extra EndTag); `locatefy` projects away every non-printable, non-endtag
//!     symbol, so those paths collapse to byte-identical Locations and the same
//!     match was reported more than once. Upstream shipped an opt-in `-u`
//!     unique flag; as a successor we dedupe exact duplicates by default at the
//!     source (`PmatchContainer::process`). These tests reproduce the duplicate
//!     and lock the dedup, while proving genuinely-distinct matches survive.
//!
//!   * hfst/hfst#357 — hfst-compose-intersect memory/time blowup on large rule
//!     sets. The lazy agenda-driven product (`ComposeIntersectLexicon`) is a
//!     faithful transcription of the C++: it materialises one state per
//!     reachable (lexicon-state, rule-product-state) pair, so the intermediate
//!     size is O(|lex| * product|rules|). That is INHERENT to the algorithm
//!     (the port adds no extra states over C++, and eagerly pre-intersecting
//!     the rules would defeat the very purpose of the lazy construction). These
//!     tests lock the *behaviour*: correctness of the composed result and the
//!     characteristic raw-vs-minimized state gap, all inside a bounded rule set
//!     that runs well under the 10s/test cap.

use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_transducer::HfstTransducer;
use hfst::pmatch::PmatchContainer;
use hfst::pmatch_compiler::PmatchCompiler;
use hfst::transducer::{Transducer, WeightedTables};
use hfst_openfst::StdVectorFst;

type T = HfstTransducer<StdVectorFst>;

// ---------------------------------------------------------------------------
// hfst/hfst#335 — duplicate Location dedup
// ---------------------------------------------------------------------------

/// Build a runtime pmatch container from a grammar source (compile TOP, convert
/// to the weighted optimized-lookup backend the runtime pins).
fn container_for(src: &str) -> Result<PmatchContainer, hfst::error::Error> {
    let mut compiler = PmatchCompiler::<StdVectorFst>::new();
    let defs = compiler.compile(src)?;
    let top = defs.get("TOP").expect("no TOP in pmatch result");
    let top_owned = HfstTransducer::<Transducer<WeightedTables>>::new_from_basic(&top.to_basic()?)?;
    PmatchContainer::new_from_hfst_transducers(vec![top_owned])
}

/// Count how many located matches (across every location vector, ignoring the
/// synthetic non-matching filler) equal (start, length, output, tag, weight).
fn count_exact(c: &mut PmatchContainer, input: &str, want: (u32, u32, &str, &str)) -> usize {
    let locs = c.locate(input, 0.0, hfst::transducer::INFINITE_WEIGHT);
    let mut n = 0usize;
    for lv in locs.iter() {
        for l in lv.iter() {
            if l.output == "@_NONMATCHING_@" {
                continue;
            }
            if l.start == want.0 && l.length == want.1 && l.output == want.2 && l.tag == want.3 {
                n += 1;
            }
        }
    }
    n
}

// The union's second branch emits an EXTRA EndTag(w) (same tag name). The two
// branches are therefore structurally distinct (the minimizer keeps both), but
// `locatefy` keeps only the last start-tag and drops the internal marker arcs,
// so both project to the identical Location `0|3|C|<w>`. Before the fix this was
// reported twice; after the fix exactly once.
#[test]
fn issue335_extra_endtag_branch_is_deduped() -> Result<(), hfst::error::Error> {
    let src = "Define TOP [{cat}:{C} EndTag(w) | [{cat}:{C} EndTag(w)] EndTag(w)] ;\n";
    let mut c = container_for(src)?;
    assert_eq!(
        count_exact(&mut c, "cat", (0, 3, "C", "<w>")),
        1,
        "exact-duplicate located match must appear exactly once"
    );
    Ok(())
}

// A second shape of the same defect: one branch wraps the match in a nested
// EndTag(w) EndTag(w). Same projected Location, must be deduped to one.
#[test]
fn issue335_nested_same_tag_is_deduped() -> Result<(), hfst::error::Error> {
    let src = "Define TOP [[{cat}:{C} EndTag(w)] | [{cat}:{C} EndTag(w) EndTag(w)]] ;\n";
    let mut c = container_for(src)?;
    assert_eq!(
        count_exact(&mut c, "cat", (0, 3, "C", "<w>")),
        1,
        "nested-same-tag duplicate must collapse to one located match"
    );
    Ok(())
}

// Guard against over-dedup: two branches whose only difference is the TAG are
// GENUINELY different matches (`0|3|C|<w>` vs `0|3|C|`) and must both survive.
#[test]
fn issue335_distinct_tag_matches_are_preserved() -> Result<(), hfst::error::Error> {
    let src = "Define TOP [{cat}:{C} EndTag(w) | {cat}:{C}] ;\n";
    let mut c = container_for(src)?;
    let tagged = count_exact(&mut c, "cat", (0, 3, "C", "<w>"));
    let untagged = count_exact(&mut c, "cat", (0, 3, "C", ""));
    assert_eq!(tagged, 1, "the tagged match must be kept");
    assert_eq!(untagged, 1, "the differently-tagged match must be kept");
    Ok(())
}

// Two distinct OUTPUTS at the same span are different matches and both survive.
#[test]
fn issue335_distinct_output_matches_are_preserved() -> Result<(), hfst::error::Error> {
    let src = "Define TOP [{cat}:{A} | {cat}:{B}] EndTag(w) ;\n";
    let mut c = container_for(src)?;
    assert_eq!(count_exact(&mut c, "cat", (0, 3, "A", "<w>")), 1);
    assert_eq!(count_exact(&mut c, "cat", (0, 3, "B", "<w>")), 1);
    Ok(())
}

// Direct unit test of the dedup semantics via the runtime: a plain single match
// is unaffected (dedup is a no-op when there are no duplicates).
#[test]
fn issue335_single_match_unaffected() -> Result<(), hfst::error::Error> {
    let src = "Define TOP {cat}:{C} EndTag(w) ;\n";
    let mut c = container_for(src)?;
    assert_eq!(count_exact(&mut c, "cat", (0, 3, "C", "<w>")), 1);
    Ok(())
}

// ---------------------------------------------------------------------------
// hfst/hfst#357 — compose-intersect behaviour / inherent-blowup lock
// ---------------------------------------------------------------------------

const EPS: &str = "@_EPSILON_SYMBOL_@";
const SYMS: [&str; 2] = ["a", "b"];

// Lexicon accepting (a|b)^len — a branching chain of len+1 states.
fn lexicon(len: usize) -> T {
    let mut b = HfstBasicTransducer::new();
    for i in 0..len {
        let s = i as u32;
        for sym in SYMS {
            let tr =
                HfstBasicTransition::new_symbols(s + 1, sym.into(), sym.into(), 0.0, b.coder_mut());
            b.add_transition(s, &tr, true);
        }
    }
    b.set_final_weight(len as u32, &0.0);
    HfstTransducer::from_basic(&b)
}

// Rule "count of 'a' mod k": a k-state cycle that advances on 'a', self-loops
// on 'b', accepting only when #a ≡ 0 (mod k).
fn rule_mod(k: usize) -> T {
    let mut b = HfstBasicTransducer::new();
    for i in 0..k {
        let s = i as u32;
        let adv = HfstBasicTransition::new_symbols(
            ((i + 1) % k) as u32,
            "a".into(),
            "a".into(),
            0.0,
            b.coder_mut(),
        );
        b.add_transition(s, &adv, true);
        let loopb = HfstBasicTransition::new_symbols(s, "b".into(), "b".into(), 0.0, b.coder_mut());
        b.add_transition(s, &loopb, true);
    }
    b.set_final_weight(0, &0.0);
    b.add_symbol_to_alphabet(&hfst::hfst_data_types::Symbol::new(EPS));
    HfstTransducer::from_basic(&b)
}

fn n_a(s: &str) -> usize {
    s.chars().filter(|&c| c == 'a').count()
}

/// Membership test: convert the composed tropical net to the optimized-lookup
/// backend (which exposes `lookup_string`) and check the input is accepted.
fn accepts(t: &T, s: &str) -> Result<bool, hfst::error::Error> {
    let mut ol = HfstTransducer::<Transducer<WeightedTables>>::new_from_basic(&t.to_basic()?)?;
    Ok(!ol.lookup_string(s, -1, 0.0)?.is_empty())
}

// Correctness: composing the branching lexicon with several coprime modular
// rules yields exactly the strings whose #a is divisible by every modulus,
// i.e. #a ≡ 0 (mod lcm). Bounded (len=6, rules [2,3]) so it is instant.
#[test]
fn issue357_compose_intersect_is_correct() -> Result<(), hfst::error::Error> {
    let len = 6usize;
    let ks = [2usize, 3usize]; // lcm = 6
    let mut lex = lexicon(len);
    let rules: Vec<T> = ks.iter().map(|&k| rule_mod(k)).collect();
    lex.compose_intersect(&rules, false, true)?;

    // Every (a|b)^6 string with #a divisible by 6 (only "bbbbbb" and "aaaaaa")
    // is accepted; strings whose #a is not a multiple of 6 are rejected.
    assert!(accepts(&lex, "bbbbbb")?, "#a=0 is divisible by 6");
    assert!(accepts(&lex, "aaaaaa")?, "#a=6 is divisible by 6");
    assert!(!accepts(&lex, "aabbbb")?, "#a=2 not divisible by 6");
    assert!(!accepts(&lex, "aaabbb")?, "#a=3 not divisible by 6");
    assert!(!accepts(&lex, "aaaabb")?, "#a=4 not divisible by 6");

    // Spot-check the property directly for every length-6 string.
    for bits in 0..(1u32 << len) {
        let s: String = (0..len)
            .map(|i| if (bits >> i) & 1 == 1 { 'a' } else { 'b' })
            .collect();
        let expected = n_a(&s).is_multiple_of(6);
        assert_eq!(
            accepts(&lex, &s)?,
            expected,
            "string {s:?} membership must match #a % 6 == 0"
        );
    }
    Ok(())
}

// Behaviour lock for the inherent blowup: the lazy product materialises one
// state per reachable (lexicon, rule-product) pair, so the RAW result is far
// larger than its minimized equivalent. This documents where the O(|lex| *
// product|rules|) cost lives (raw states) versus the tiny final answer, and
// pins that the port terminates well within the test-time cap on a rule set
// whose UNRESTRICTED rule intersection (which an eager algorithm would build)
// would be product(k) = 210 states — but the lexicon-restricted lazy product is
// what actually gets materialised here.
#[test]
fn issue357_blowup_is_bounded_and_minimizes_away() -> Result<(), hfst::error::Error> {
    let ks = [2usize, 3usize, 5usize, 7usize]; // product = 210
    let len = ks.iter().product::<usize>(); // 210 — one full period, forces density
    let mut lex = lexicon(len);
    let rules: Vec<T> = ks.iter().map(|&k| rule_mod(k)).collect();
    lex.compose_intersect(&rules, false, true)?;

    let raw = lex.number_of_states();
    let mut min = lex.clone();
    min.minimize()?;
    let minimized = min.number_of_states();

    // The lazy product is genuinely large (thousands of states) ...
    assert!(
        raw > 5_000,
        "the lazy product should materialise the multiplicative blowup (raw={raw})"
    );
    // ... yet collapses to a tiny minimized net: the blowup is redundancy in the
    // intermediate product, inherent to the agenda-driven construction.
    assert!(
        minimized < raw / 10,
        "minimized ({minimized}) should be an order of magnitude below raw ({raw})"
    );

    // Correctness still holds at this scale: #a ≡ 0 (mod 210).
    assert!(accepts(&lex, &"b".repeat(len))?, "#a=0 divisible by 210");
    assert!(accepts(&lex, &"a".repeat(len))?, "#a=210 divisible by 210");
    assert!(
        !accepts(&lex, &("a".to_string() + &"b".repeat(len - 1)))?,
        "#a=1 not divisible"
    );
    Ok(())
}

// The intersection of an EMPTY rule set is the empty language. The guard for
// it assigned the empty transducer but did not return, so control fell through
// to `&v[0]` and panicked on the index: any caller that composed a lexicon
// against a rule vector which happened to be empty crashed instead of getting
// the documented empty result.
#[test]
fn compose_intersect_with_no_rules_is_empty() -> Result<(), hfst::error::Error> {
    let mut lex = lexicon(4);
    let no_rules: Vec<T> = Vec::new();
    lex.compose_intersect(&no_rules, false, true)?;

    // Asserted structurally rather than through accepts(): converting the
    // empty result to the optimized-lookup form and looking a string up in it
    // panics in TransitionTable::at, which is a separate unfixed defect
    // (defects/panic-on-malformed-input). Coupling this regression test to
    // that one would make it fail for the wrong reason.
    assert_eq!(
        lex.number_of_arcs(),
        0,
        "no rules yields the empty language"
    );
    Ok(())
}
