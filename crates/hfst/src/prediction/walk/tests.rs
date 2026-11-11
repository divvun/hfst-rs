use super::fixtures_test::*;
use super::*;

fn kinds(next: &Next) -> Vec<(UnitKind, &str)> {
    next.candidates
        .iter()
        .map(|candidate| (candidate.unit.kind, candidate.key.as_str()))
        .collect()
}

fn outputs(walker: &Walker<'_, crate::transducer::WeightedTables>, word: &Word) -> Vec<String> {
    let mut written: Vec<String> = walker
        .configurations(word)
        .iter()
        .map(|config| walker.machine.render(config.output()))
        .collect();
    written.sort();
    written.dedup();
    written
}

// [spec:hfst:def:prediction.unit/test]
// [spec:hfst:sem:prediction-units.hidden/test]
#[test]
fn units_differing_in_hidden_tags_are_one_candidate() {
    let generator = lexc(NOUNS);
    let grammar = grammar(NOUNS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    let next = walker.next(&word_after(&walker, "girji", &[]));
    let nominative = candidate(&next, "+N+Sg+Nom");
    assert_eq!(nominative.unit.kind, UnitKind::Ending);
    assert_eq!(
        next.candidates
            .iter()
            .filter(|candidate| candidate.key.as_str() == "+N+Sg+Nom")
            .count(),
        1
    );
    let picked = word_after(&walker, "girji", &["+N+Sg+Nom"]);
    assert_eq!(surfaces(&walker, &picked), ["girji", "girjii"]);
}

// [spec:hfst:def:prediction.state/test]
#[test]
fn the_empty_word_is_the_start_closure() {
    let generator = lexc(PERSONS);
    let grammar = grammar(PERSONS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    let start = walker.start();
    assert!(start.units().is_empty());
    let configurations = walker.configurations(&start);
    assert!(configurations.iter().any(|config| config.state() == 0));
    assert_eq!(outputs(&walker, &start), ["", "k", "ki", "n", "ni"]);
    let flag_values: std::collections::BTreeSet<&[i16]> = configurations
        .iter()
        .map(Configuration::flag_values)
        .collect();
    assert!(flag_values.len() >= 4, "{flag_values:?}");
    assert!(configurations.iter().all(|config| config.weight() == 0.0));
}

// [spec:hfst:def:prediction.state/test]
#[test]
fn configurations_keep_what_each_path_wrote_and_weighed() {
    let generator = lexc(NOUNS);
    let grammar = grammar(NOUNS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    let word = word_after(&walker, "girji", &["+N+Sg+Nom"]);
    let finals: Vec<(String, Weight)> = walker
        .configurations(&word)
        .iter()
        .filter(|config| config.is_final())
        .map(|config| (walker.machine.render(config.output()), config.weight()))
        .collect();
    assert!(finals.contains(&("girji".to_owned(), 0.0)), "{finals:?}");
    assert!(finals.contains(&("girjii".to_owned(), 2.0)), "{finals:?}");
}

// [spec:hfst:req:prediction.generator-decides/test]
// [spec:hfst:sem:prediction-units.ending+1/test]
// [spec:hfst:sem:prediction-units.boundary+3/test]
#[test]
fn the_units_offered_are_those_the_generator_accepts() {
    let generator = lexc(NOUNS);
    let grammar = grammar(NOUNS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    let after_noun = walker.next(&word_after(&walker, "girji", &[]));
    assert_eq!(
        kinds(&after_noun),
        [
            (UnitKind::Boundary, "+N+Cmp#"),
            (UnitKind::Ending, "+N+Pl+Nom"),
            (UnitKind::Ending, "+N+Sg+Gen"),
            (UnitKind::Ending, "+N+Sg+Nom"),
            (UnitKind::Ending, "+N+Sg+Nom+PxSg1"),
        ]
    );
    assert!(!after_noun.truncated);
    let after_verb = walker.next(&word_after(&walker, "borra", &[]));
    assert_eq!(
        kinds(&after_verb),
        [
            (UnitKind::Boundary, "+V+Der/NomAg"),
            (UnitKind::Ending, "+V+Inf"),
        ]
    );
}

// [spec:hfst:req:prediction.generator-decides/test]
#[test]
fn picking_continues_every_path_with_the_key() {
    let generator = lexc(NOUNS);
    let grammar = grammar(NOUNS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    let word = word_after(&walker, "girji", &["+N+Sg+Nom"]);
    assert!(word.is_complete());
    assert_eq!(
        walker.forms(&word),
        [
            Form {
                surface: "girji".to_owned(),
                weight: 0.0
            },
            Form {
                surface: "girjii".to_owned(),
                weight: 2.0
            },
        ]
    );
    let derived = word_after(&walker, "borra", &["+V+Der/NomAg", "+N+Sg+Nom"]);
    assert_eq!(surfaces(&walker, &derived), ["borrii", "borriii"]);
}

// [spec:hfst:req:prediction.flags/test]
#[test]
fn flags_decide_which_ending_a_path_reaches() {
    let generator = lexc(PERSONS);
    let grammar = grammar(PERSONS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    let next = walker.next(&word_after(&walker, "nipaa", &[]));
    assert_eq!(
        keys(&next),
        ["+V+AI+Ind+1Sg", "+V+AI+Ind+2Sg", "+V+AI+Ind+3Sg"]
    );
    let first = word_after(&walker, "nipaa", &["+V+AI+Ind+1Sg"]);
    assert_eq!(surfaces(&walker, &first), ["ninipaan"]);
    let second = word_after(&walker, "nipaa", &["+V+AI+Ind+2Sg"]);
    assert_eq!(surfaces(&walker, &second), ["kinipaan"]);
    let third = word_after(&walker, "nipaa", &["+V+AI+Ind+3Sg"]);
    assert_eq!(surfaces(&walker, &third), ["nipaaw"]);
}

// [spec:hfst:sem:prediction-units.filter/test]
// [spec:hfst:req:prediction.flags/test]
#[test]
fn a_filter_sets_the_flag_endings_agree_with() {
    let generator = lexc(PERSONS);
    let grammar = grammar(PERSONS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    let start = walker.start();
    let first = walker.next(&start);
    assert_eq!(
        kinds(&first),
        [
            (UnitKind::Filter, "@U.person.KI@"),
            (UnitKind::Filter, "@U.person.NI@"),
            (UnitKind::Prefix, "PV/kii+"),
        ]
    );
    assert!(first.stem_next);
    let filtered = start.pick(candidate(&first, "@U.person.NI@"));
    let entered = walker.entry(&filtered, "nipaa", None).unwrap();
    let stem = filtered.pick(candidate(&entered, "nipaa"));
    let next = walker.next(&stem);
    assert_eq!(keys(&next), ["+V+AI+Ind+1Sg"]);
    let word = stem.pick(candidate(&next, "+V+AI+Ind+1Sg"));
    assert_eq!(surfaces(&walker, &word), ["ninipaan"]);
}

// [spec:hfst:sem:prediction-units.filter/test]
#[test]
fn a_filter_no_configuration_survives_is_not_offered() {
    let generator = lexc(PERSONS);
    let grammar = grammar(PERSONS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    let start = walker.start();
    let ni = start.pick(candidate(&walker.next(&start), "@U.person.NI@"));
    assert_eq!(
        kinds(&walker.next(&ni)),
        [
            (UnitKind::Filter, "@U.person.NI@"),
            (UnitKind::Prefix, "PV/kii+"),
        ]
    );
}

// [spec:hfst:sem:prediction-units.prefix/test]
#[test]
fn a_prefix_comes_before_the_stem() {
    let generator = lexc(PERSONS);
    let grammar = grammar(PERSONS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    let start = walker.start();
    let first = walker.next(&start);
    let prefixed = start.pick(candidate(&first, "PV/kii+"));
    let entered = walker.entry(&prefixed, "ni", None).unwrap();
    let stem = prefixed.pick(candidate(&entered, "nipaa"));
    let next = walker.next(&stem);
    let word = stem.pick(candidate(&next, "+V+AI+Ind+1Sg"));
    assert_eq!(surfaces(&walker, &word), ["nikii-nipaan"]);
    assert_eq!(
        word.units()
            .iter()
            .map(|unit| unit.kind)
            .collect::<Vec<_>>(),
        [UnitKind::Prefix, UnitKind::Stem, UnitKind::Ending]
    );
}

// [spec:hfst:sem:prediction-units.enclitic/test]
// [spec:hfst:sem:prediction.complete/test]
#[test]
fn an_enclitic_follows_and_keeps_a_word_complete() {
    let generator = lexc(NOUNS);
    let grammar = grammar(NOUNS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    let stem = word_after(&walker, "girji", &[]);
    assert!(!stem.is_complete());
    assert!(walker.next(&stem).forms.is_empty());
    let complete = word_after(&walker, "girji", &["+N+Sg+Nom"]);
    let next = walker.next(&complete);
    assert_eq!(kinds(&next), [(UnitKind::Enclitic, "+Qst")]);
    assert_eq!(next.forms.len(), 2);
    let asked = complete.pick(candidate(&next, "+Qst"));
    assert!(asked.is_complete());
    assert_eq!(surfaces(&walker, &asked), ["girjigo", "girjiigo"]);
    assert!(walker.next(&asked).candidates.is_empty());
}

// [spec:hfst:sem:prediction.complete/test]
#[test]
fn forms_weigh_the_lightest_path_with_final_weight() {
    let generator = final_weights();
    let grammar = grammar("");
    let walker = walker(&generator, &grammar);
    let word = word_after(&walker, "a", &[]);
    assert!(word.is_complete());
    assert_eq!(
        walker.next(&word).forms,
        [
            Form {
                surface: "y".to_owned(),
                weight: 0.75
            },
            Form {
                surface: "x".to_owned(),
                weight: 1.0
            },
        ]
    );
}

// [spec:hfst:req:prediction.bounded/test]
#[test]
fn a_repeating_tag_stops_at_the_unit_bound() {
    let generator = lexc(CYCLES);
    let grammar = grammar("");
    let limits = WalkLimits {
        max_unit_symbols: 4,
        max_configurations: 10_000,
    };
    let walker = Walker::new(&generator, &grammar, limits).unwrap();
    let next = walker.next(&word_after(&walker, "aa", &[]));
    assert!(next.truncated);
    assert_eq!(keys(&next), ["+N+Sg", "+Rep+N+Sg", "+Rep+Rep+N+Sg"]);
}

// [spec:hfst:req:prediction.bounded/test]
#[test]
fn endless_output_stops_at_the_configuration_bound() {
    let generator = lexc(CYCLES);
    let grammar = grammar("");
    let limits = WalkLimits {
        max_unit_symbols: 8,
        max_configurations: 200,
    };
    let walker = Walker::new(&generator, &grammar, limits).unwrap();
    let word = word_after(&walker, "bb", &[]);
    let next = walker.next(&word);
    assert!(next.truncated);
    assert!(
        next.candidates
            .iter()
            .all(|candidate| candidate.key.as_str() == "+N+Sg")
    );
}

// [spec:hfst:req:prediction.bounded/test]
#[test]
fn a_walk_within_its_bounds_is_not_truncated() {
    let generator = lexc(NOUNS);
    let grammar = grammar(NOUNS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    let word = word_after(&walker, "girji", &[]);
    assert!(!word.truncated());
    assert!(!walker.next(&word).truncated);
}

// [spec:hfst:sem:prediction-units.boundary+3/test]
// [spec:hfst:sem:prediction-units.stem/test]
#[test]
fn a_stem_can_follow_a_compound_join() {
    let generator = lexc(NOUNS);
    let grammar = grammar(NOUNS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    let joined = word_after(&walker, "girji", &["+N+Cmp#"]);
    assert!(walker.stem_may_start(&joined));
    let next = walker.next(&joined);
    assert!(next.stem_next);
    assert!(next.candidates.is_empty());
    let entered = walker.entry(&joined, "guo", None).unwrap();
    assert_eq!(keys(&entered), ["guolli"]);
    let second = joined.pick(candidate(&entered, "guolli"));
    let ending = walker.next(&second);
    let word = second.pick(candidate(&ending, "+N+Sg+Nom"));
    assert_eq!(surfaces(&walker, &word), ["girjguoli", "girjguolii"]);
}

struct Prefer(&'static str);

impl CandidateOrder for Prefer {
    fn order(&self, _context: &[UnitKey], candidates: &[(UnitKey, Weight)]) -> Vec<usize> {
        let mut order: Vec<usize> = (0..candidates.len()).collect();
        order.sort_by_key(|&at| candidates[at].0.as_str() != self.0);
        order
    }
}

// [spec:hfst:sem:prediction.preview/test]
#[test]
fn the_preview_completes_with_the_first_ranked_ending() {
    let generator = lexc(NOUNS);
    let grammar = grammar(NOUNS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    let stem = word_after(&walker, "girji", &[]);
    assert_eq!(
        walker.preview(&stem, &[], &GeneratorWeightOrder).as_deref(),
        Some("girji")
    );
    assert_eq!(
        walker.preview(&stem, &[], &Prefer("+N+Sg+Gen")).as_deref(),
        Some("girja")
    );
    let complete = word_after(&walker, "girji", &["+N+Pl+Nom"]);
    assert_eq!(
        walker
            .preview(&complete, &[], &GeneratorWeightOrder)
            .as_deref(),
        Some("girjat")
    );
    assert_eq!(
        walker.preview(&walker.start(), &[], &GeneratorWeightOrder),
        None
    );
    let joined = word_after(&walker, "girji", &["+N+Cmp#"]);
    assert_eq!(walker.preview(&joined, &[], &GeneratorWeightOrder), None);
}

#[test]
fn generator_weight_order_is_lightest_then_by_key() {
    let candidates = [
        (UnitKey::from("+b"), 1.0),
        (UnitKey::from("+c"), 0.5),
        (UnitKey::from("+a"), 1.0),
    ];
    assert_eq!(GeneratorWeightOrder.order(&[], &candidates), [1, 2, 0]);
}

// [spec:hfst:req:prediction.views/test]
#[test]
fn the_walk_reads_the_generator_it_was_lent() {
    let generator = lexc(NOUNS);
    let grammar = grammar(NOUNS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    assert!(std::ptr::eq(walker.generator(), &generator));
    let unweighted = unweighted_lexc(NOUNS);
    let unweighted_walker = Walker::new(&unweighted, &grammar, WalkLimits::default()).unwrap();
    let start = unweighted_walker.start();
    let entered = unweighted_walker.entry(&start, "girji", None).unwrap();
    let stem = start.pick(candidate(&entered, "girji"));
    assert_eq!(
        keys(&unweighted_walker.next(&stem)),
        keys(&walker.next(&word_after(&walker, "girji", &[])))
    );
}

#[test]
fn a_grammar_naming_missing_tags_is_refused() {
    let generator = lexc(NOUNS);
    let grammar = grammar("boundary = [\"+Der/Missing\"]");
    let refused = Walker::new(&generator, &grammar, WalkLimits::default())
        .err()
        .expect("the grammar is refused");
    assert!(refused.to_string().contains("+Der/Missing"), "{refused}");
}

// [spec:hfst:sem:prediction-units.boundary+3/test]
// [spec:hfst:sem:prediction-units.ending+1/test]
// [spec:hfst:req:prediction.generator-decides/test]
#[test]
fn derivations_are_offered_only_with_der_tags() {
    let generator = lexc(DERIVATIONS);
    let grammar = grammar(DERIVATIONS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    let next = walker.next(&word_after(&walker, "oqar", &[]));
    assert_eq!(
        kinds(&next),
        [
            (UnitKind::Boundary, "+KIP+Der/nv"),
            (UnitKind::Boundary, "+LI+Der/vv"),
            (UnitKind::Boundary, "+NIQ+Der/vn"),
            (UnitKind::Ending, "+V+Ind+3Sg"),
        ]
    );
    assert!(!next.truncated);
    let derived = word_after(&walker, "oqar", &["+NIQ+Der/vn"]);
    let after = walker.next(&derived);
    assert!(
        after
            .candidates
            .iter()
            .any(|c| c.key.as_str() == "+N+Abs+Sg")
    );
    assert!(
        after
            .candidates
            .iter()
            .any(|c| c.key.as_str() == "+KIP+Der/nv")
    );
    assert!(!after.truncated);
}

// [spec:hfst:sem:prediction-units.boundary+3/test]
// [spec:hfst:req:prediction.bounded/test]
#[test]
fn without_derivation_tags_chains_never_end() {
    let generator = lexc(DERIVATIONS);
    let grammar = grammar(NO_DERIVATIONS_GRAMMAR);
    let limits = WalkLimits {
        max_unit_symbols: 6,
        max_configurations: 50_000,
    };
    let walker = Walker::new(&generator, &grammar, limits).unwrap();
    let next = walker.next(&word_after(&walker, "oqar", &[]));
    assert!(next.truncated);
    let keys = keys(&next);
    assert!(keys.contains(&"+NIQ+N+Abs+Sg"), "{keys:?}");
    assert!(keys.contains(&"+NIQ+KIP+Der/nv"), "{keys:?}");
}

// [spec:hfst:sem:prediction-units.enclitic/test]
#[test]
fn an_enclitic_derivation_tag_follows_complete_words() {
    let generator = lexc(DERIVATIONS);
    let grammar = grammar(DERIVATIONS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    let complete = word_after(&walker, "oqar", &["+V+Ind+3Sg"]);
    let next = walker.next(&complete);
    assert_eq!(kinds(&next), [(UnitKind::Enclitic, "+LI")]);
    let asked = complete.pick(candidate(&next, "+LI"));
    assert_eq!(surfaces(&walker, &asked), ["oqarpoqlu"]);
}

// [spec:hfst:sem:prediction-units.boundary+3/test]
#[test]
fn tags_may_come_before_the_one_derivation_tag() {
    let generator = lexc(DERIVATIONS);
    let grammar = grammar(DERIVATIONS_GRAMMAR);
    let walker = walker(&generator, &grammar);
    let next = walker.next(&word_after(&walker, "Akoq", &[]));
    assert_eq!(
        kinds(&next),
        [
            (UnitKind::Ending, "+Prop+Abs"),
            (UnitKind::Boundary, "+Prop+U+Der/nv"),
        ]
    );
    assert!(!next.truncated);
    let derived = word_after(&walker, "Akoq", &["+Prop+U+Der/nv", "+V+Ind+3Sg"]);
    assert_eq!(surfaces(&walker, &derived), ["Akoqupoq"]);
}
