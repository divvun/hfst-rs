//! Regression + behaviour locks for two successor fixes/findings:
//!
//!   * hfst/hfst#335 — one accepting configuration can be reached through
//!     several structurally distinct paths (e.g. a union branch carrying an
//!     extra EndTag); `locatefy` projects away every non-printable, non-endtag
//!     symbol, so those paths collapse to byte-identical Locations and the same
//!     match is reported more than once.
//!
//!     The port briefly deduped these at the source. That was WRONG for the
//!     tokeniser: `hfst-tokenize -c`/`-g` feed Constraint Grammar, and the
//!     multiplicity of a reading inside a cohort is part of that contract — the
//!     dedup silently dropped one of the two `akte Num Sg Acc` readings that the
//!     Giella sma tokeniser emits for "aktem", across 668 of 39 200 cohorts in a
//!     2 000-line corpus sample. Locations are now passed through unfiltered,
//!     matching C++ exactly. Upstream offers an opt-in `-u` flag for callers who
//!     want uniqueness; it is not applied by default.
//!
//!     The expected counts below were taken from the C++ oracle
//!     (`hfst-pmatch2fst` + `hfst-tokenize -c` on these very grammars).
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
// hfst/hfst#335 — duplicate Locations are REPORTED, as C++ does
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
// so both project to the identical Location `0|3|C|<w>`. C++ reports it twice
// (verified: `hfst-tokenize -c` prints "\tC" twice for input "cat"), and so
// must we — a CG cohort's reading multiplicity is contractual.
#[test]
fn issue335_extra_endtag_branch_reports_both_paths() -> Result<(), hfst::error::Error> {
    let src = "Define TOP [{cat}:{C} EndTag(w) | [{cat}:{C} EndTag(w)] EndTag(w)] ;\n";
    let mut c = container_for(src)?;
    assert_eq!(
        count_exact(&mut c, "cat", (0, 3, "C", "<w>")),
        2,
        "both structurally distinct paths must be reported, as C++ does"
    );
    Ok(())
}

// A second shape of the same thing: one branch wraps the match in a nested
// EndTag(w) EndTag(w). Same projected Location, still two paths, still two
// reported matches (C++ oracle: "\tC" twice).
#[test]
fn issue335_nested_same_tag_reports_both_paths() -> Result<(), hfst::error::Error> {
    let src = "Define TOP [[{cat}:{C} EndTag(w)] | [{cat}:{C} EndTag(w) EndTag(w)]] ;\n";
    let mut c = container_for(src)?;
    assert_eq!(
        count_exact(&mut c, "cat", (0, 3, "C", "<w>")),
        2,
        "the nested-same-tag path is a second path and is reported separately"
    );
    Ok(())
}

// Two branches whose only difference is the TAG are GENUINELY different matches
// (`0|3|C|<w>` vs `0|3|C|`) and each is reported once.
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

// A grammar with a single path reports a single match — the multiplicity comes
// from the number of accepting paths, never from the reporting layer.
#[test]
fn issue335_single_match_reported_once() -> Result<(), hfst::error::Error> {
    let src = "Define TOP {cat}:{C} EndTag(w) ;\n";
    let mut c = container_for(src)?;
    assert_eq!(count_exact(&mut c, "cat", (0, 3, "C", "<w>")), 1);
    Ok(())
}

// ---------------------------------------------------------------------------
// Normalization aliases must never shadow a REAL symbol
// ---------------------------------------------------------------------------

// The encoder additionally indexes every symbol under its NFC and NFD spellings
// (hfst/hfst#439), so a grapheme cluster matches whichever normal form the input
// happens to use. That aliasing is only safe while the alias spelling is not
// itself a symbol of the alphabet.
//
// U+0387 GREEK ANO TELEIA has a SINGLETON canonical decomposition to U+00B7
// MIDDLE DOT, so NFC(U+0387) == NFD(U+0387) == U+00B7. An alphabet holding both
// characters — the Giella sma tokeniser does — therefore had U+0387's alias
// overwrite the genuine U+00B7 trie entry whenever U+0387 came later in the
// symbol table. Input U+00B7 then encoded to the U+0387 symbol: its analyses
// disappeared and even the echoed surface form came back as U+0387.
//
// Aliases are now registered in a second pass, into free spellings only, so a
// real symbol always wins and the result no longer depends on table order.
#[test]
fn normalization_alias_does_not_shadow_a_real_symbol() -> Result<(), hfst::error::Error> {
    // Both orders, because the defect was order-dependent.
    for src in [
        "Define TOP [ {\u{0387}}:{ANO} | {\u{00B7}}:{MID} ] EndTag(w) ;\n",
        "Define TOP [ {\u{00B7}}:{MID} | {\u{0387}}:{ANO} ] EndTag(w) ;\n",
    ] {
        let mut c = container_for(src)?;
        assert_eq!(
            count_exact(&mut c, "\u{00B7}", (0, 1, "MID", "<w>")),
            1,
            "U+00B7 must match its own arc, not U+0387's alias (grammar: {src})"
        );
        assert_eq!(
            count_exact(&mut c, "\u{00B7}", (0, 1, "ANO", "<w>")),
            0,
            "U+00B7 must NOT reach the U+0387 arc (grammar: {src})"
        );
        assert_eq!(
            count_exact(&mut c, "\u{0387}", (0, 1, "ANO", "<w>")),
            1,
            "U+0387 must still match its own arc (grammar: {src})"
        );
    }
    Ok(())
}

// The alias mechanism itself must still work: when the alias spelling is NOT a
// symbol of the alphabet there is nothing to shadow, so decomposed input still
// reaches a precomposed arc. (hfst/hfst#439, kept alive by the two-pass change.)
#[test]
fn normalization_alias_still_matches_when_unshadowed() -> Result<(), hfst::error::Error> {
    // Precomposed Cyrillic U+045D only; the decomposed spelling U+0438 U+0300 is
    // not a symbol here, so it stays available as an alias.
    let src = "Define TOP {\u{045D}}:{IGRAVE} EndTag(w) ;\n";
    let mut c = container_for(src)?;
    assert_eq!(
        count_exact(&mut c, "\u{045D}", (0, 1, "IGRAVE", "<w>")),
        1,
        "precomposed input matches the precomposed arc"
    );
    assert_eq!(
        count_exact(&mut c, "\u{0438}\u{0300}", (0, 1, "IGRAVE", "<w>")),
        1,
        "decomposed input must still match the precomposed arc via the NFD alias"
    );
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

    assert_eq!(
        lex.number_of_arcs(),
        0,
        "no rules yields the empty language"
    );
    // And it behaves like one end to end: converting it to the optimized-lookup
    // form and looking a string up used to walk off the index table (see
    // tests/malformed_ol_input.rs), which is why this was once asserted only
    // structurally.
    assert!(
        !accepts(&lex, "aaaa")?,
        "the empty language accepts nothing"
    );
    assert!(!accepts(&lex, "")?, "not even the empty string");
    Ok(())
}
