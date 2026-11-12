use super::fixtures_test::*;
use super::memo::Shelf;
use super::*;
use crate::transducer::WeightedTables;

/// What a walk offers after `word`: each candidate's key and weight, and
/// the forms of the word with it picked.
fn offered(
    walker: &Walker<'_, WeightedTables>,
    word: &Word,
    next: &Next,
) -> Vec<(String, Weight, Vec<String>)> {
    next.candidates
        .iter()
        .map(|candidate| {
            let picked = word.pick(candidate);
            (
                candidate.key.as_str().to_owned(),
                candidate.weight,
                surfaces(walker, &picked),
            )
        })
        .collect()
}

#[test]
fn a_second_walk_reuses_what_the_first_learnt() {
    let generator = lexc(NOUNS);
    let grammar = grammar(NOUNS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    let word = word_after(&walker, "girji", &[]);
    let first = walker.next(&word);
    let learnt = walker.shelf.computed();
    assert!(learnt > 0);
    let state = word.seeds.iter().next().unwrap().state();
    assert!(walker.shelf.knows_reach(state));
    let second = walker.next(&word);
    assert_eq!(walker.shelf.computed(), learnt);
    assert_eq!(
        offered(&walker, &word, &first),
        offered(&walker, &word, &second)
    );
}

#[test]
fn offers_do_not_depend_on_what_was_seen() {
    let generator = lexc(NOUNS);
    let grammar = grammar(NOUNS_GRAMMAR);
    let fresh = walker(&generator, &grammar);
    let seasoned = walker(&generator, &grammar);
    for stem in ["borra", "guolli", "gáhkku"] {
        seasoned.next(&word_after(&seasoned, stem, &[]));
    }
    seasoned.next(&word_after(&seasoned, "girji", &["+N+Cmp#"]));
    for units in [&[][..], &["+N+Sg+Nom"], &["+N+Cmp#"]] {
        let word = word_after(&fresh, "girji", units);
        assert_eq!(
            offered(&fresh, &word, &fresh.next(&word)),
            offered(&seasoned, &word, &seasoned.next(&word)),
            "after girji {units:?}"
        );
        assert_eq!(fresh.forms(&word), seasoned.forms(&word));
    }
    let start = fresh.start();
    let typed = |walker: &Walker<'_, WeightedTables>| {
        offered(walker, &start, &walker.entry(&start, "g", None).unwrap())
    };
    assert_eq!(typed(&fresh), typed(&seasoned));
}

// [spec:hfst:req:prediction.bounded/test]
#[test]
fn a_cut_walk_keeps_no_partial_frontier() {
    let generator = lexc(CYCLES);
    let grammar = grammar("");
    let limits = WalkLimits {
        max_unit_symbols: 8,
        max_configurations: 200,
    };
    let walker = Walker::new(&generator, &grammar, limits).unwrap();
    let word = word_after(&walker, "bb", &[]);
    let first = walker.next(&word);
    let kept = walker.shelf.frontiers();
    let learnt = walker.shelf.computed();
    let second = walker.next(&word);
    assert!(first.truncated && second.truncated);
    assert_eq!(walker.shelf.frontiers(), kept);
    assert!(walker.shelf.computed() > learnt);
    assert_eq!(keys(&first), keys(&second));
}

#[test]
fn only_the_unit_picked_is_spelled_out() {
    let generator = lexc(NOUNS);
    let grammar = grammar(NOUNS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    let word = word_after(&walker, "girji", &[]);
    let next = walker.next(&word);
    assert!(next.candidates.len() > 1);
    assert!(next.candidates.iter().all(|c| !c.seeds.is_spelled()));
    let picked = word.pick(candidate(&next, "+N+Sg+Nom"));
    walker.next(&picked);
    assert!(picked.seeds.is_spelled());
    let unpicked = next
        .candidates
        .iter()
        .filter(|c| c.key.as_str() != "+N+Sg+Nom");
    assert!(unpicked.into_iter().all(|c| !c.seeds.is_spelled()));
}

#[test]
fn a_walk_finding_the_memo_taken_starts_afresh() {
    let shelf = Shelf::default();
    let generator = lexc(NOUNS);
    let grammar = grammar(NOUNS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    let mut outer = Pass::kept(
        &walker.machine,
        (&walker.roles, &walker.marks),
        1000,
        &shelf,
    );
    let start = outer.start();
    outer.finals(&[start]);
    assert!(outer.knows_reach(start.here.state));
    let inner = Pass::kept(
        &walker.machine,
        (&walker.roles, &walker.marks),
        1000,
        &shelf,
    );
    assert!(!inner.knows_reach(start.here.state));
    drop(inner);
    drop(outer);
    assert!(shelf.knows_reach(start.here.state));
}
