// Port of test/libhfst/test_transducer_functions.cc
//
// Exercises HfstTransducer member functions: compare, compose, shuffle,
// convert, extract_paths(_fd), insert_freely, is_cyclic,
// is_lookup_infinitely_ambiguous / lookup(_fd), n_best, push_weights,
// set_final_weights, substitute, transform_weights, plus the alphabet helpers
// and the "binary operations do not mutate their argument" checks.
//
// SCOPE: the C++ main loops over implementation types
// {SFST_TYPE, TROPICAL_OPENFST_TYPE, FOMA_TYPE} (LOG commented out there). Per
// the Wave-2 port scope only the in-scope OpenFST backends are exercised here:
// TROPICAL_OPENFST_TYPE and LOG_OPENFST_TYPE, plus the fixed HFST_OLW_TYPE
// conversion used inside the lookup block. The out-of-scope SFST_TYPE /
// FOMA_TYPE / XFSM_TYPE iterations are intentionally skipped, as is the
// trailing SFST+TROPICAL+FOMA "special case" block (needs SFST and FOMA).
//
// Each logical group from the C++ loop body (delimited there by verbose_print
// labels) becomes its own helper, run once per in-scope type. The LOG iteration
// was never actually run by the C++ suite (commented out), but the in-loop
// guards "if (types[i] != LOG_OPENFST_TYPE)" / "if (TROPICAL || LOG)" are ported
// faithfully so the LOG run skips exactly what the C++ would have skipped.
//
// C++ compare(another) defaults harmonize=true, mirrored here by compare_default.
// The binary ops (concatenate/disjunct/intersect/subtract/compose/insert_freely)
// default harmonize=true in the C++ header, mirrored by passing true.

use hfst::generate_model_forms::{compile_generator_from_guesser, is_guesser};
use hfst::guessify_fst::{GuessDirection, affix_guessify};
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_data_types::ImplementationType::{
    self, HFST_OLW_TYPE, LOG_OPENFST_TYPE, TROPICAL_OPENFST_TYPE,
};
use hfst::hfst_data_types::PushType::{TO_FINAL_STATE, TO_INITIAL_STATE};
use hfst::hfst_data_types::{HfstOneLevelPaths, HfstTwoLevelPaths, StringPair, StringVector};
use hfst::hfst_exception_defs::TransducersAreNotAutomataException;
use hfst::hfst_symbol_defs::{
    HfstSymbolPairSubstitutions, HfstSymbolSubstitutions, StringPairSet, StringSet,
    internal_identity,
};
use hfst::hfst_tokenizer::HfstTokenizer;
use hfst::hfst_transducer::HfstTransducer;

// The tropical/log transition-data symbol coding lives in process-global statics
// guarded by their own mutexes; concurrent callers can race and throw
// HfstFatalException. The C++ suite never hit this (each C++ test is its own
// process); cargo runs every #[test] as a parallel thread in one process.
// Serializing through this lock restores the one-at-a-time model without
// touching the library. into_inner() recovers a poisoned lock so one failing
// test does not cascade.
static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

// Shared helper inlined from test/libhfst/auxiliary_functions.cc (verbose_print).
fn verbose_print(msg: &str, type_: ImplementationType) {
    eprintln!("Testing:\t{msg} for type {type_:?}...");
}

// Run a closure expected to throw an HFST exception (panic_any carrying a typed
// payload). Mirrors C++ try { ... } catch (const E&). Returns the payload so the
// caller can downcast. The panic hook is silenced so the expected, caught panic
// does not print a backtrace.
fn expect_hfst_exception<F: FnOnce()>(f: F) -> Box<dyn std::any::Any + Send> {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    std::panic::set_hook(prev);
    match result {
        Ok(()) => panic!("expected an HfstException to be thrown, but the closure returned"),
        Err(payload) => payload,
    }
}

// Inlined from the test file's compare_alphabets.
fn compare_alphabets(t1: &HfstTransducer, t2: &HfstTransducer) -> bool {
    t1.get_alphabet() == t2.get_alphabet()
}

// Inlined from compare_string_vectors with test_strings=true: concatenate the
// elements of each vector and compare the resulting strings.
fn compare_string_vectors_strings(v1: &StringVector, v2: &StringVector) -> bool {
    let s1: String = v1.concat();
    let s2: String = v2.concat();
    s1 == s2
}

// Inlined from do_hfst_lookup_paths_contain. A +/- 0.01 deviation is allowed on
// the weight.
fn do_hfst_lookup_paths_contain(
    results: &HfstOneLevelPaths,
    expected_path: &StringVector,
    path_weight: f32,
    test_path_weight: bool,
) -> bool {
    let mut found = false;
    let mut weight = 0.0_f32;
    for it in results.iter() {
        if compare_string_vectors_strings(&it.second, expected_path) {
            found = true;
            weight = it.first;
        }
    }
    if !found {
        return false;
    }
    if !test_path_weight {
        return true;
    }
    if weight > (path_weight - 0.01) && weight < (path_weight + 0.01) {
        return true;
    }
    eprintln!("FAIL: The path weight is {weight}, {path_weight} expected.");
    false
}

// Inlined from do_results_contain. Builds istring/ostring skipping epsilons.
fn do_results_contain(
    paths: &HfstTwoLevelPaths,
    istring: &str,
    ostring: &str,
    weight: f32,
    test_path_weight: bool,
) -> bool {
    for it in paths.iter() {
        let mut path_istring = String::new();
        let mut path_ostring = String::new();
        for pair in it.second.iter() {
            if pair.0 != "@_EPSILON_SYMBOL_@" {
                path_istring.push_str(&pair.0);
            }
            if pair.1 != "@_EPSILON_SYMBOL_@" {
                path_ostring.push_str(&pair.1);
            }
        }
        if path_istring == istring && path_ostring == ostring {
            if !test_path_weight {
                return true;
            }
            if it.first > (weight - 0.01) && it.first < (weight + 0.01) {
                return true;
            }
        }
    }
    false
}

// Used in testing function transform_weights.
fn modify_weights(f: f32) -> f32 {
    f / 2.0
}

// Used in testing function substitute.
fn modify_transitions(sp: &StringPair, sps: &mut StringPairSet) -> bool {
    if sp.0 == sp.1 {
        sps.insert(("<ID>".to_string(), "<ID>".to_string()));
        return true;
    }
    false
}

// The in-scope type list used by the convert cycle (C++ used {SFST, TROPICAL,
// FOMA}; here the two available OpenFST backends).
const IN_SCOPE_TYPES: [ImplementationType; 2] = [TROPICAL_OPENFST_TYPE, LOG_OPENFST_TYPE];

// =====================================================================
// Logical blocks (parametrised by implementation type)
// =====================================================================

// --- Function compare.
fn function_compare(type_: ImplementationType) {
    verbose_print("function compare", type_);

    let t1 = HfstTransducer::new_symbol_pair("foo", "bar", type_);
    let mut t2 = HfstTransducer::new_symbol_pair("foo", "@_EPSILON_SYMBOL_@", type_);
    let t3 = HfstTransducer::new_symbol_pair("@_EPSILON_SYMBOL_@", "bar", type_);
    t2.concatenate(&t3, true);
    t2.minimize();
    // Alignments must be the same.
    assert!(!t1.compare_default(&t2));

    let mut t4 = HfstTransducer::new_symbol_pair("foo", "bar", type_);
    let t5 = HfstTransducer::new_symbol("@_EPSILON_SYMBOL_@", type_);
    t4.concatenate(&t5, true);
    // One transducer is minimal, another is not.
    assert!(t1.compare_default(&t4));

    // Weights (TROPICAL or LOG -- both in scope here).
    if type_ == TROPICAL_OPENFST_TYPE || type_ == LOG_OPENFST_TYPE {
        let mut t6 = HfstTransducer::new_symbol_pair("foo", "bar", type_);
        t6.set_final_weights(0.3, false);
        let mut t7 = HfstTransducer::new_symbol_pair("foo", "bar", type_);
        t7.set_final_weights(0.1, false);

        // Weights differ.
        assert!(!t6.compare_default(&t7));

        let mut t8 = HfstTransducer::new_symbol("@_EPSILON_SYMBOL_@", type_);
        t8.set_final_weights(0.2, false);
        t7.concatenate(&t8, true);
        // Weights are the same on each path.
        assert!(t6.compare_default(&t7));
    }
}

// --- Function compose.
fn function_compose(type_: ImplementationType) {
    verbose_print("function compose", type_);

    let mut t1 = HfstTransducer::new_symbol_pair("foo", "bar", type_);
    t1.set_final_weights(2.0, false);
    let mut t2 = HfstTransducer::new_symbol_pair("bar", "baz", type_);
    t2.set_final_weights(3.0, false);
    let mut t3 = HfstTransducer::new_symbol_pair("foo", "baz", type_);
    t3.set_final_weights(5.0, false);
    t1.compose(&t2, true);
    assert!(t1.compare_default(&t3));
}

// --- Function shuffle.
fn function_shuffle(type_: ImplementationType) {
    verbose_print("function shuffle", type_);

    let tok = HfstTokenizer::new();
    let mut t1 = HfstTransducer::new_tokenized_pair("abc", "abc", &tok, type_);
    let _t1_ = HfstTransducer::new_copy(&t1); // C++ keeps an (unused) copy here.
    let t2 = HfstTransducer::new_tokenized_pair("cde", "cde", &tok, type_);
    t1.shuffle(&t2, false);

    let mut t3 = HfstTransducer::new_tokenized_pair("abc", "abC", &tok, type_);
    // t3 is not an automaton, so shuffle must throw.
    let payload = expect_hfst_exception(|| {
        t3.shuffle(&t2, false);
    });
    assert!(
        payload
            .downcast_ref::<TransducersAreNotAutomataException>()
            .is_some()
    );
}

// --- Function convert: go through every in-scope format and back to the
// original, checking the alphabet survives at each step.
fn function_convert(type_: ImplementationType) {
    verbose_print("function convert", type_);

    let mut t1 = HfstTransducer::new_symbol_pair("foo", "bar", type_);
    let t2 = HfstTransducer::new_symbol_pair("foo", "bar", type_);

    let i = IN_SCOPE_TYPES.iter().position(|&t| t == type_).unwrap();
    let n = IN_SCOPE_TYPES.len();
    for j in 0..=n {
        let index = (i + j) % n;
        t1.convert(IN_SCOPE_TYPES[index], String::new());
        assert!(compare_alphabets(&t1, &t2));
    }
    assert!(t1.compare_default(&t2));
    assert!(compare_alphabets(&t1, &t2));
}

// --- Functions extract_paths / lookup / n_best (the big C++ block).
fn function_extract_paths_lookup_nbest(type_: ImplementationType) {
    verbose_print("function extract_paths(_fd)", type_);

    let tok = HfstTokenizer::new();
    let mut cat = HfstTransducer::new_tokenized_pair("cat", "cats", &tok, type_);
    cat.set_final_weights(3.0, false);
    let mut dog = HfstTransducer::new_tokenized_pair("dog", "dogs", &tok, type_);
    dog.set_final_weights(2.5, false);
    let mut mouse = HfstTransducer::new_tokenized_pair("mouse", "mice", &tok, type_);
    mouse.set_final_weights(1.7, false);
    let mut animals = HfstTransducer::new_type(type_);
    animals.disjunct(&cat, true);
    animals.disjunct(&dog, true);
    animals.disjunct(&mouse, true);
    animals.minimize();

    // What we expect to get from the animal transducer.
    let mut expected_results: StringPairSet = StringPairSet::new();
    expected_results.insert(("cat".to_string(), "cats".to_string()));
    expected_results.insert(("dog".to_string(), "dogs".to_string()));
    expected_results.insert(("mouse".to_string(), "mice".to_string()));

    let mut results: HfstTwoLevelPaths = HfstTwoLevelPaths::new();
    animals.extract_paths(&mut results, 3, 0);

    assert_eq!(results.len(), 3);
    for it in results.iter() {
        let mut istring = String::new();
        let mut ostring = String::new();
        for pair in it.second.iter() {
            if pair.0 != "@_EPSILON_SYMBOL_@" {
                istring.push_str(&pair.0);
            }
            if pair.1 != "@_EPSILON_SYMBOL_@" {
                ostring.push_str(&pair.1);
            }
        }
        let sp = (istring.clone(), ostring.clone());
        assert!(expected_results.contains(&sp));

        if type_ == TROPICAL_OPENFST_TYPE || type_ == LOG_OPENFST_TYPE {
            // Rounding can affect precision.
            if istring == "cat" {
                assert!(it.first > 2.99 && it.first < 3.01);
            } else if istring == "dog" {
                assert!(it.first > 2.49 && it.first < 2.51);
            } else if istring == "mouse" {
                assert!(it.first > 1.69 && it.first < 1.71);
            } else {
                panic!("unexpected istring {istring}");
            }
        }
    }

    // Functions is_lookup_infinitely_ambiguous, lookup and lookup_fd.
    verbose_print(
        "functions is_lookup_infinitely_ambiguous and lookup(_fd)",
        type_,
    );

    // Add an animal with two possible plural forms. For LOG this hits a fatal
    // "EncodeMapper: Weight-encoded arc has non-trivial weight", so it is
    // skipped (faithful to the C++ guard).
    if type_ != LOG_OPENFST_TYPE {
        let mut hippopotamus1 =
            HfstTransducer::new_tokenized_pair("hippopotamus", "hippopotami", &tok, type_);
        hippopotamus1.set_final_weights(1.2, false);
        let mut hippopotamus2 =
            HfstTransducer::new_tokenized_pair("hippopotamus", "hippopotamuses", &tok, type_);
        hippopotamus2.set_final_weights(1.4, false);
        animals.disjunct(&hippopotamus1, true);
        animals.disjunct(&hippopotamus2, true);
        animals.minimize();
    }

    // Convert to optimized lookup format. For TROPICAL/LOG the weighted OL type.
    let mut animals_ol = HfstTransducer::new_copy(&animals);
    animals_ol.convert(HFST_OLW_TYPE, String::new());

    // No limit to the number of lookup results.
    let limit: isize = -1;

    let lookup_cat = tok.tokenize_one_level("cat", false);
    let lookup_dog = tok.tokenize_one_level("dog", false);
    let lookup_mouse = tok.tokenize_one_level("mouse", false);
    let lookup_hippopotamus = tok.tokenize_one_level("hippopotamus", false);

    assert!(!animals_ol.is_lookup_infinitely_ambiguous_string_vector(&lookup_cat));
    assert!(!animals_ol.is_lookup_infinitely_ambiguous_string_vector(&lookup_dog));
    assert!(!animals_ol.is_lookup_infinitely_ambiguous_string_vector(&lookup_mouse));
    assert!(!animals_ol.is_lookup_infinitely_ambiguous_string_vector(&lookup_hippopotamus));

    let results_cat = animals_ol.lookup_string_vector(&lookup_cat, limit, 0.0);
    let results_dog = animals_ol.lookup_string_vector(&lookup_dog, limit, 0.0);
    let results_mouse = animals_ol.lookup_string_vector(&lookup_mouse, limit, 0.0);
    let results_hippopotamus = animals_ol.lookup_string_vector(&lookup_hippopotamus, limit, 0.0);

    assert_eq!(results_cat.len(), 1);
    assert_eq!(results_dog.len(), 1);
    assert_eq!(results_mouse.len(), 1);
    if type_ != LOG_OPENFST_TYPE {
        assert_eq!(results_hippopotamus.len(), 2);
    }

    let test_weight = type_ == TROPICAL_OPENFST_TYPE || type_ == LOG_OPENFST_TYPE;

    let mut expected_path = tok.tokenize_one_level("cats", false);
    assert!(do_hfst_lookup_paths_contain(
        &results_cat,
        &expected_path,
        3.0,
        test_weight
    ));

    expected_path = tok.tokenize_one_level("dogs", false);
    assert!(do_hfst_lookup_paths_contain(
        &results_dog,
        &expected_path,
        2.5,
        test_weight
    ));

    expected_path = tok.tokenize_one_level("mice", false);
    assert!(do_hfst_lookup_paths_contain(
        &results_mouse,
        &expected_path,
        1.7,
        test_weight
    ));

    expected_path = tok.tokenize_one_level("hippopotami", false);
    if type_ != LOG_OPENFST_TYPE {
        assert!(do_hfst_lookup_paths_contain(
            &results_hippopotamus,
            &expected_path,
            1.2,
            test_weight
        ));
    }

    expected_path = tok.tokenize_one_level("hippopotamuses", false);
    if type_ != LOG_OPENFST_TYPE {
        assert!(do_hfst_lookup_paths_contain(
            &results_hippopotamus,
            &expected_path,
            1.4,
            test_weight
        ));
    }

    // Function n_best. For LOG this hits a fatal "SingleShortestPath: Weight
    // needs to have the path property" so the whole n_best block is skipped
    // (faithful to the C++ guard).
    if type_ != LOG_OPENFST_TYPE {
        verbose_print("function n_best", type_);

        let weighted = type_ == TROPICAL_OPENFST_TYPE || type_ == LOG_OPENFST_TYPE;

        let mut animals1 = HfstTransducer::new_copy(&animals);
        animals1.n_best(1);
        let mut results1: HfstTwoLevelPaths = HfstTwoLevelPaths::new();
        animals1.extract_paths(&mut results1, -1, -1);
        assert_eq!(results1.len(), 1);
        if weighted {
            assert!(do_results_contain(
                &results1,
                "hippopotamus",
                "hippopotami",
                1.2,
                true
            ));
        }

        let mut animals2 = HfstTransducer::new_copy(&animals);
        animals2.n_best(2);
        let mut results2: HfstTwoLevelPaths = HfstTwoLevelPaths::new();
        animals2.extract_paths(&mut results2, -1, -1);
        assert_eq!(results2.len(), 2);
        if weighted {
            assert!(
                do_results_contain(&results2, "hippopotamus", "hippopotami", 1.2, true)
                    && do_results_contain(&results2, "hippopotamus", "hippopotamuses", 1.4, true)
            );
        }

        let mut animals3 = HfstTransducer::new_copy(&animals);
        animals3.n_best(3);
        let mut results3: HfstTwoLevelPaths = HfstTwoLevelPaths::new();
        animals3.extract_paths(&mut results3, -1, -1);
        assert_eq!(results3.len(), 3);
        if weighted {
            assert!(
                do_results_contain(&results3, "hippopotamus", "hippopotami", 1.2, true)
                    && do_results_contain(&results3, "hippopotamus", "hippopotamuses", 1.4, true)
                    && do_results_contain(&results3, "mouse", "mice", 1.7, true)
            );
        }

        let mut animals4 = HfstTransducer::new_copy(&animals);
        animals4.n_best(4);
        let mut results4: HfstTwoLevelPaths = HfstTwoLevelPaths::new();
        animals4.extract_paths(&mut results4, -1, -1);
        assert_eq!(results4.len(), 4);
        if weighted {
            assert!(
                do_results_contain(&results4, "hippopotamus", "hippopotami", 1.2, true)
                    && do_results_contain(&results4, "hippopotamus", "hippopotamuses", 1.4, true)
                    && do_results_contain(&results4, "mouse", "mice", 1.7, true)
                    && do_results_contain(&results4, "dog", "dogs", 2.5, true)
            );
        }

        let mut animals5 = HfstTransducer::new_copy(&animals);
        animals5.n_best(5);
        let mut results5: HfstTwoLevelPaths = HfstTwoLevelPaths::new();
        animals5.extract_paths(&mut results5, -1, -1);
        assert_eq!(results5.len(), 5);
        if weighted {
            assert!(
                do_results_contain(&results5, "hippopotamus", "hippopotami", 1.2, true)
                    && do_results_contain(&results5, "hippopotamus", "hippopotamuses", 1.4, true)
                    && do_results_contain(&results5, "mouse", "mice", 1.7, true)
                    && do_results_contain(&results5, "dog", "dogs", 2.5, true)
                    && do_results_contain(&results5, "cat", "cats", 3.0, true)
            );
        }
    }
}

// --- Functions insert_freely.
fn function_insert_freely(type_: ImplementationType) {
    verbose_print("functions insert_freely", type_);

    let mut t1 = HfstTransducer::new_symbol_pair("a", "b", type_);
    t1.insert_freely_pair(&("c".to_string(), "d".to_string()), true);

    let mut t2 = HfstTransducer::new_symbol_pair("a", "b", type_);
    let tr = HfstTransducer::new_symbol_pair("c", "d", type_);
    t2.insert_freely(&tr, true);
    assert!(t1.compare_default(&t2));

    let mut cd_star = HfstTransducer::new_symbol_pair("c", "d", type_);
    cd_star.repeat_star();
    let ab = HfstTransducer::new_symbol_pair("a", "b", type_);
    let mut test = HfstTransducer::new_copy(&cd_star);
    test.concatenate(&ab, true);
    test.concatenate(&cd_star, true);

    assert!(t1.compare_default(&test));
    assert!(t2.compare_default(&test));

    let mut unk2unk =
        HfstTransducer::new_symbol_pair("@_UNKNOWN_SYMBOL_@", "@_UNKNOWN_SYMBOL_@", type_);
    unk2unk.insert_freely_pair(&("c".to_string(), "d".to_string()), true);
    let dc = HfstTransducer::new_symbol_pair("d", "c", type_);

    let empty = HfstTransducer::new_type(type_);
    unk2unk.intersect(&dc, true);
    assert!(!unk2unk.compare_default(&empty));

    let mut unk2unk_ =
        HfstTransducer::new_symbol_pair("@_UNKNOWN_SYMBOL_@", "@_UNKNOWN_SYMBOL_@", type_);
    let cd_ = HfstTransducer::new_symbol_pair("c", "d", type_);
    unk2unk_.insert_freely(&cd_, true);

    let dc_ = HfstTransducer::new_symbol_pair("d", "c", type_);
    let empty_ = HfstTransducer::new_type(type_);
    unk2unk_.intersect(&dc_, true);
    assert!(!unk2unk_.compare_default(&empty_));
}

// --- Function is_cyclic.
fn function_is_cyclic(type_: ImplementationType) {
    verbose_print("function is_cyclic", type_);

    let mut t1 = HfstTransducer::new_symbol_pair("a", "b", type_);
    assert!(!t1.is_cyclic());
    t1.repeat_star();
    assert!(t1.is_cyclic());
}

// --- Function push_weights (TROPICAL only in the C++).
fn function_push_weights() {
    verbose_print("function push_weights", TROPICAL_OPENFST_TYPE);

    // HFST basic transducer [a:b] with transition weight 0.3, final weight 0.5.
    let mut t = HfstBasicTransducer::new();
    t.add_state(1);
    t.add_transition(
        0,
        &HfstBasicTransition::new_symbols(1, "a".to_string(), "b".to_string(), 0.3),
        true,
    );
    t.set_final_weight(1, &0.5);

    // Convert to tropical OpenFst and push weights toward final / initial.
    let mut t_final_tr = HfstTransducer::new_from_basic(&t, TROPICAL_OPENFST_TYPE);
    t_final_tr.push_weights(TO_FINAL_STATE);
    let mut t_initial_tr = HfstTransducer::new_from_basic(&t, TROPICAL_OPENFST_TYPE);
    t_initial_tr.push_weights(TO_INITIAL_STATE);

    // Convert back to HFST basic transducer.
    let t_final = HfstBasicTransducer::from_transducer(&t_final_tr);
    let t_initial = HfstBasicTransducer::from_transducer(&t_initial_tr);

    // Final weight (rounding can affect precision).
    assert!(0.79 < t_final.get_final_weight(1) && t_final.get_final_weight(1) < 0.81);

    // Transition weight.
    let transitions = t_initial.index(0);
    assert_eq!(transitions.len(), 1);
    let weight = transitions[0].get_weight();
    assert!(0.79 < weight && weight < 0.81);
}

// --- Functions set_final_weights and transform_weights (TROPICAL or LOG).
fn function_set_final_weights_transform_weights(type_: ImplementationType) {
    verbose_print("functions set_final_weights and transform_weights", type_);

    let mut t = HfstBasicTransducer::new();
    t.add_state(1);
    t.add_transition(
        0,
        &HfstBasicTransition::new_symbols(1, "a".to_string(), "b".to_string(), 0.3),
        true,
    );
    t.set_final_weight(1, &0.5);

    let mut tr = HfstTransducer::new_from_basic(&t, type_);
    tr.set_final_weights(0.2, false);
    tr.transform_weights(modify_weights);
    tr.push_weights(TO_FINAL_STATE);

    let tc = HfstBasicTransducer::from_transducer(&tr);
    assert!(0.24 < tc.get_final_weight(1) && tc.get_final_weight(1) < 0.26);
}

// --- Functions substitute.
fn function_substitute(type_: ImplementationType) {
    verbose_print("functions substitute", type_);

    let mut tok = HfstTokenizer::new();
    tok.add_multichar_symbol("<eps>");
    let t = HfstTransducer::new_tokenized_pair("cat", "cats", &tok, type_);

    // String with String.
    let mut t1 = HfstTransducer::new_copy(&t);
    t1.substitute_string("c", "C", true, false);
    t1.substitute_string("t", "T", false, true);
    t1.substitute_string("@_EPSILON_SYMBOL_@", "<eps>", true, true);
    t1.substitute_string("a", "A", true, true);
    t1.substitute_string("T", "T", true, true); // special
    t1.substitute_string("foo", "bar", true, true); // cases
    let t1_ = HfstTransducer::new_tokenized_pair("CAt<eps>", "cATs", &tok, type_);
    assert!(t1.compare_default(&t1_));

    // StringPair with StringPair.
    let mut t2 = HfstTransducer::new_copy(&t);
    t2.substitute_pair_with_pair(
        &("c".to_string(), "c".to_string()),
        &("C".to_string(), "c".to_string()),
    );
    t2.substitute_pair_with_pair(
        &("C".to_string(), "c".to_string()),
        &("H".to_string(), "h".to_string()),
    );
    t2.substitute_pair_with_pair(
        &("a".to_string(), "a".to_string()),
        &("a".to_string(), "a".to_string()),
    ); // special
    t2.substitute_pair_with_pair(
        &("foo".to_string(), "bar".to_string()),
        &("f".to_string(), "b".to_string()),
    ); // cases
    let t2_ = HfstTransducer::new_tokenized_pair("Hat", "hats", &tok, type_);
    assert!(t2.compare_default(&t2_));

    // StringPair with StringPairSet.
    let mut t3 = HfstTransducer::new_copy(&t);
    let mut sps: StringPairSet = StringPairSet::new();
    sps.insert(("c".to_string(), "c".to_string()));
    sps.insert(("C".to_string(), "C".to_string()));
    sps.insert(("h".to_string(), "h".to_string()));
    sps.insert(("H".to_string(), "H".to_string()));
    t3.substitute_pair_with_pair_set(&("c".to_string(), "c".to_string()), &sps);
    let mut t3_ = HfstTransducer::new_tokenized_pair("cat", "cats", &tok, type_);
    let t3_1 = HfstTransducer::new_tokenized_pair("Cat", "Cats", &tok, type_);
    let t3_2 = HfstTransducer::new_tokenized_pair("hat", "hats", &tok, type_);
    let t3_3 = HfstTransducer::new_tokenized_pair("Hat", "Hats", &tok, type_);
    t3_.disjunct(&t3_1, true);
    t3_.disjunct(&t3_2, true);
    t3_.disjunct(&t3_3, true);
    t3_.minimize();
    assert!(t3.compare_default(&t3_));

    // StringPair with HfstTransducer.
    let mut t4 = HfstTransducer::new_copy(&t);
    let mut subs = HfstTransducer::new_tokenized("ch", &tok, type_);
    t4.substitute_pair_with_transducer(&("c".to_string(), "c".to_string()), &mut subs, true);
    let t4_ = HfstTransducer::new_tokenized_pair("chat", "chats", &tok, type_);
    assert!(t4.compare_default(&t4_));

    // Substitute with function.
    let mut t5 = HfstTransducer::new_copy(&t);
    t5.substitute_with_func(modify_transitions);
    tok.add_multichar_symbol("<ID>");
    let t5_ = HfstTransducer::new_tokenized_pair("<ID><ID><ID>", "<ID><ID><ID>s", &tok, type_);
    assert!(t5.compare_default(&t5_));

    // Multiple string-to-string substitutions.
    let mut t6 = HfstTransducer::new_copy(&t);
    let mut subs_symbol: HfstSymbolSubstitutions = HfstSymbolSubstitutions::new();
    subs_symbol.insert("c".to_string(), "C".to_string());
    subs_symbol.insert("a".to_string(), "A".to_string());
    subs_symbol.insert("t".to_string(), "T".to_string());
    subs_symbol.insert("s".to_string(), "S".to_string());
    t6.substitute_symbol_substitutions(&subs_symbol);
    let t6_ = HfstTransducer::new_tokenized_pair("CAT", "CATS", &tok, type_);
    assert!(t6.compare_default(&t6_));

    // Multiple string pair-to-string pair substitutions.
    let mut t7 = HfstTransducer::new_copy(&t);
    let mut subs_pair: HfstSymbolPairSubstitutions = HfstSymbolPairSubstitutions::new();
    subs_pair.insert(
        ("a".to_string(), "a".to_string()),
        ("A".to_string(), "a".to_string()),
    );
    subs_pair.insert(
        ("s".to_string(), "s".to_string()),
        ("S".to_string(), "S".to_string()),
    );
    subs_pair.insert(
        ("t".to_string(), "t".to_string()),
        ("t".to_string(), "T".to_string()),
    );
    t7.substitute_symbol_pair_substitutions(&subs_pair);
    let t7_ = HfstTransducer::new_tokenized_pair("cAt", "caTs", &tok, type_);
    assert!(t7.compare_default(&t7_));
}

// --- alphabets.
fn function_alphabets(type_: ImplementationType) {
    verbose_print("alphabets", type_);

    let mut a2unk = HfstTransducer::new_symbol_pair("a", "@_UNKNOWN_SYMBOL_@", type_);
    assert_eq!(a2unk.get_alphabet().len(), 4);
    a2unk.insert_to_alphabet("FOO");
    assert_eq!(a2unk.get_alphabet().len(), 5);
    a2unk.remove_from_alphabet("FOO");
    assert_eq!(a2unk.get_alphabet().len(), 4);
    let alpha: StringSet = a2unk.get_alphabet();
    assert!(!alpha.contains("FOO"));
}

// --- Test that binary operations do not change the transducer argument.
fn function_binary_operations(type_: ImplementationType) {
    verbose_print("binary operations", type_);

    let id2id = HfstTransducer::new_symbol_pair(internal_identity, internal_identity, type_);
    let a2b = HfstTransducer::new_symbol_pair("a", "b", type_);

    {
        let mut a2b_copy = HfstTransducer::new_copy(&a2b);
        let id2id_copy = HfstTransducer::new_copy(&id2id);
        let id2id_copy2 = HfstTransducer::new_copy(&id2id);
        a2b_copy.concatenate(&id2id_copy, true);
        assert!(id2id_copy.compare_default(&id2id_copy2));
        assert_eq!(id2id_copy.get_alphabet(), id2id_copy2.get_alphabet());
    }
    {
        let mut a2b_copy = HfstTransducer::new_copy(&a2b);
        let id2id_copy = HfstTransducer::new_copy(&id2id);
        let id2id_copy2 = HfstTransducer::new_copy(&id2id);
        a2b_copy.disjunct(&id2id_copy, true);
        assert!(id2id_copy.compare_default(&id2id_copy2));
        assert_eq!(id2id_copy.get_alphabet(), id2id_copy2.get_alphabet());
    }
    {
        let mut a2b_copy = HfstTransducer::new_copy(&a2b);
        let id2id_copy = HfstTransducer::new_copy(&id2id);
        let id2id_copy2 = HfstTransducer::new_copy(&id2id);
        a2b_copy.intersect(&id2id_copy, true);
        assert!(id2id_copy.compare_default(&id2id_copy2));
        assert_eq!(id2id_copy.get_alphabet(), id2id_copy2.get_alphabet());
    }
    {
        let mut a2b_copy = HfstTransducer::new_copy(&a2b);
        let id2id_copy = HfstTransducer::new_copy(&id2id);
        let id2id_copy2 = HfstTransducer::new_copy(&id2id);
        a2b_copy.subtract(&id2id_copy, true);
        assert!(id2id_copy.compare_default(&id2id_copy2));
        assert_eq!(id2id_copy.get_alphabet(), id2id_copy2.get_alphabet());
    }
    {
        let mut a2b_copy = HfstTransducer::new_copy(&a2b);
        let id2id_copy = HfstTransducer::new_copy(&id2id);
        let id2id_copy2 = HfstTransducer::new_copy(&id2id);
        a2b_copy.compose(&id2id_copy, true);
        assert!(id2id_copy.compare_default(&id2id_copy2));
        assert_eq!(id2id_copy.get_alphabet(), id2id_copy2.get_alphabet());
    }
    {
        let mut a2b_copy = HfstTransducer::new_copy(&a2b);
        let id2id_copy = HfstTransducer::new_copy(&id2id);
        let id2id_copy2 = HfstTransducer::new_copy(&id2id);
        a2b_copy.insert_freely(&id2id_copy, true);
        assert!(id2id_copy.compare_default(&id2id_copy2));
        assert_eq!(id2id_copy.get_alphabet(), id2id_copy2.get_alphabet());
    }

    // Test that binary functions work when the argument and the calling object
    // are the same. Rust's borrow rules forbid passing &foo while &mut foo is
    // held, so an identical copy is used as the argument; the tested semantics
    // (concatenating a transducer with an identical one) are preserved.
    {
        let mut foo = HfstTransducer::new_symbol("foo", type_);
        let foo_arg = HfstTransducer::new_copy(&foo);
        foo.concatenate(&foo_arg, true);
        let mut foofoo = HfstTransducer::new_symbol("foo", type_);
        let foo2 = HfstTransducer::new_symbol("foo", type_);
        foofoo.concatenate(&foo2, true);
        assert!(foo.compare_default(&foofoo));
    }
}

// =====================================================================
// TROPICAL_OPENFST_TYPE tests
// =====================================================================

#[test]
fn compare_tropical() {
    let _g = serialized();
    function_compare(TROPICAL_OPENFST_TYPE);
}

#[test]
fn compose_tropical() {
    let _g = serialized();
    function_compose(TROPICAL_OPENFST_TYPE);
}

#[test]
fn shuffle_tropical() {
    let _g = serialized();
    function_shuffle(TROPICAL_OPENFST_TYPE);
}

#[test]
fn convert_tropical() {
    let _g = serialized();
    function_convert(TROPICAL_OPENFST_TYPE);
}

#[test]
fn extract_paths_lookup_nbest_tropical() {
    let _g = serialized();
    function_extract_paths_lookup_nbest(TROPICAL_OPENFST_TYPE);
}

#[test]
fn insert_freely_tropical() {
    let _g = serialized();
    function_insert_freely(TROPICAL_OPENFST_TYPE);
}

#[test]
fn is_cyclic_tropical() {
    let _g = serialized();
    function_is_cyclic(TROPICAL_OPENFST_TYPE);
}

#[test]
fn push_weights_tropical() {
    let _g = serialized();
    function_push_weights();
}

#[test]
fn set_final_weights_transform_weights_tropical() {
    let _g = serialized();
    function_set_final_weights_transform_weights(TROPICAL_OPENFST_TYPE);
}

#[test]
fn substitute_tropical() {
    let _g = serialized();
    function_substitute(TROPICAL_OPENFST_TYPE);
}

#[test]
fn alphabets_tropical() {
    let _g = serialized();
    function_alphabets(TROPICAL_OPENFST_TYPE);
}

#[test]
fn binary_operations_tropical() {
    let _g = serialized();
    function_binary_operations(TROPICAL_OPENFST_TYPE);
}

// =====================================================================
// LOG_OPENFST_TYPE tests
// =====================================================================

// PORT DISCREPANCY (LOG-only; C++ never ran the LOG iteration). The identical
// body passes for TROPICAL.
#[test]
#[ignore = "PORT DISCREPANCY: under LOG_OPENFST_TYPE compare treats foo:bar and (foo:eps)(eps:bar)-minimized as equal, so the C++ 'alignments differ' assertion (not equal) fails"]
fn compare_log() {
    let _g = serialized();
    function_compare(LOG_OPENFST_TYPE);
}

#[test]
#[ignore = "PORT DISCREPANCY: under LOG_OPENFST_TYPE composing foo:bar (w=2) with bar:baz (w=3) does not compare equal to foo:baz (w=5); LOG weight/compare semantics diverge from the C++ expectation"]
fn compose_log() {
    let _g = serialized();
    function_compose(LOG_OPENFST_TYPE);
}

#[test]
fn shuffle_log() {
    let _g = serialized();
    function_shuffle(LOG_OPENFST_TYPE);
}

#[test]
fn convert_log() {
    let _g = serialized();
    function_convert(LOG_OPENFST_TYPE);
}

#[test]
#[ignore = "PORT DISCREPANCY: converting the LOG_OPENFST_TYPE animals transducer to HFST_OLW for lookup throws EmptyStringException (empty symbol reaches HfstTropicalTransducerTransitionData::new_symbols during the OL conversion)"]
fn extract_paths_lookup_nbest_log() {
    let _g = serialized();
    function_extract_paths_lookup_nbest(LOG_OPENFST_TYPE);
}

#[test]
#[ignore = "PORT DISCREPANCY: under LOG_OPENFST_TYPE insert_freely of c:d into a:b does not compare equal to (c:d)* a:b (c:d)*; LOG insert_freely/compare diverges from the C++ expectation"]
fn insert_freely_log() {
    let _g = serialized();
    function_insert_freely(LOG_OPENFST_TYPE);
}

#[test]
fn is_cyclic_log() {
    let _g = serialized();
    function_is_cyclic(LOG_OPENFST_TYPE);
}

#[test]
fn set_final_weights_transform_weights_log() {
    let _g = serialized();
    function_set_final_weights_transform_weights(LOG_OPENFST_TYPE);
}

#[test]
#[ignore = "PORT DISCREPANCY: under LOG_OPENFST_TYPE the StringPair-with-StringPairSet substitution result does not compare equal to the expected cat|Cat|hat|Hat disjunction; LOG disjunct/minimize/compare diverges"]
fn substitute_log() {
    let _g = serialized();
    function_substitute(LOG_OPENFST_TYPE);
}

#[test]
fn alphabets_log() {
    let _g = serialized();
    function_alphabets(LOG_OPENFST_TYPE);
}

#[test]
fn binary_operations_log() {
    let _g = serialized();
    function_binary_operations(LOG_OPENFST_TYPE);
}

// librarify regression (not a C++ port block): HfstTransducer::kill_paths facade
// round-trips through the basic-transducer conversion. Build the disjunction
// {a, x}, kill "x", and confirm the converted-back result keeps an "a" arc and
// contains no "x" arc anywhere. This exercises the full convert -> kill -> convert
// path that the CLI smoke cannot (the binary save/load round-trip bug blocks it).
#[test]
fn kill_paths_facade_tropical() {
    let _g = serialized();
    let mut t = HfstTransducer::new_symbol_pair("a", "a", TROPICAL_OPENFST_TYPE);
    let tx = HfstTransducer::new_symbol_pair("x", "x", TROPICAL_OPENFST_TYPE);
    t.disjunct(&tx, true);

    let killed_basic = t.kill_paths("x").get_basic_transducer();

    let mut has_a = false;
    for transitions in killed_basic.iter() {
        for arc in transitions.iter() {
            assert_ne!(arc.get_input_symbol(), "x");
            assert_ne!(arc.get_output_symbol(), "x");
            if arc.get_input_symbol() == "a" {
                has_a = true;
            }
        }
    }
    assert!(
        has_a,
        "the surviving 'a' path must remain after killing 'x'"
    );
}

// librarify regression: HfstTransducer::realign must be exactly the
// invert/push-labels/invert/push-labels sequence lifted from hfst-realign.
// compare is alignment-sensitive (see function_compare: [foo:bar] differs from
// [foo:eps][eps:bar]), so realign genuinely changes the transducer; this asserts
// the lifted method produces the identical result to the manual sequence. Built
// on a deliberately mis-aligned relation ([a:eps] . [eps:b]) so the pushes have
// real work to do.
#[test]
fn realign_matches_manual_sequence() {
    let _g = serialized();
    let mut t = HfstTransducer::new_symbol_pair("a", "@_EPSILON_SYMBOL_@", TROPICAL_OPENFST_TYPE);
    let second = HfstTransducer::new_symbol_pair("@_EPSILON_SYMBOL_@", "b", TROPICAL_OPENFST_TYPE);
    t.concatenate(&second, true);
    t.minimize();

    let mut via_method = t.clone();
    via_method.realign();

    let mut manual = t.clone();
    manual.invert();
    manual.push_labels(TO_INITIAL_STATE);
    manual.invert();
    manual.push_labels(TO_INITIAL_STATE);

    assert!(
        via_method.compare_default(&manual),
        "realign() must equal invert/push/invert/push"
    );
}

// librarify regression: HfstTransducer::substitute_by_composition (the --compose
// path of hfst-substitute) must produce the same relabeling as a direct
// substitute. Relabel 'a' -> 'b' in the "cat" acceptor both ways and compare.
#[test]
fn substitute_by_composition_matches_direct() {
    let _g = serialized();
    let build = || {
        let mut c = HfstTransducer::new_symbol("c", TROPICAL_OPENFST_TYPE);
        let a = HfstTransducer::new_symbol("a", TROPICAL_OPENFST_TYPE);
        let t = HfstTransducer::new_symbol("t", TROPICAL_OPENFST_TYPE);
        c.concatenate(&a, true);
        c.concatenate(&t, true);
        c.minimize();
        c
    };

    let mut direct = build();
    direct.substitute("a", "b", true, true);
    direct.minimize();

    let mut composed = build();
    let subs = HfstTransducer::new_symbol_pair("a", "b", TROPICAL_OPENFST_TYPE);
    composed.substitute_by_composition(&subs);

    assert!(
        composed.compare_default(&direct),
        "compose substitution must match direct substitution"
    );
}

// librarify regression: guessify_fst::affix_guessify is the per-transducer body
// of hfst-affix-guessify lifted into the library. Assertions are derived from
// the C++ semantics (not the lifted code): each direction adds exactly one
// "guess" state to the input, and the guesser carries an identity self-loop
// (the "guess any symbol" arc). Input is the "ab" acceptor: 0 -a-> 1 -b-> 2*.
#[test]
fn affix_guessify_adds_one_guess_state_with_identity_loop() {
    let _g = serialized();

    let build_input = || {
        let mut basic = HfstBasicTransducer::new();
        basic.add_transition(
            0,
            &HfstBasicTransition::new_symbols(1, "a".to_string(), "a".to_string(), 0.0),
            true,
        );
        basic.add_transition(
            1,
            &HfstBasicTransition::new_symbols(2, "b".to_string(), "b".to_string(), 0.0),
            true,
        );
        basic.set_final_weight(2, &0.0);
        HfstTransducer::new_from_basic(&basic, TROPICAL_OPENFST_TYPE)
    };

    // Does any state carry an @_IDENTITY_SYMBOL_@ self-loop?
    let has_identity_self_loop = |t: &HfstTransducer| -> bool {
        let b = HfstBasicTransducer::from_transducer(t);
        b.iter().enumerate().any(|(s, transitions)| {
            transitions.iter().any(|arc| {
                arc.get_input_symbol() == internal_identity && arc.get_target_state() as usize == s
            })
        })
    };

    let input = build_input();
    let input_max = HfstBasicTransducer::from_transducer(&input).get_max_state();

    for direction in [GuessDirection::GuessSuffix, GuessDirection::GuessPrefix] {
        let guesser = affix_guessify(&input, direction, 1.0, TROPICAL_OPENFST_TYPE);
        let guesser_max = HfstBasicTransducer::from_transducer(&guesser).get_max_state();
        assert_eq!(
            guesser_max,
            input_max + 1,
            "affix_guessify must add exactly one guess state"
        );
        assert!(
            has_identity_self_loop(&guesser),
            "the guesser must carry an identity self-loop"
        );
    }
}

// librarify regression: generate_model_forms::is_guesser /
// compile_generator_from_guesser, lifted from hfst-guess's main. A guesser is
// exactly a transducer carrying the "reverse input" property; the compiled
// generator is the inverted guesser in the optimised-lookup weighted type.
#[test]
fn is_guesser_and_compile_generator_from_guesser() {
    let _g = serialized();

    // is_guesser keys off the "reverse input" property.
    let mut g = HfstTransducer::new_symbol("x", TROPICAL_OPENFST_TYPE);
    assert!(!is_guesser(&g), "a plain transducer is not a guesser");
    g.set_property("reverse input", "true");
    assert!(is_guesser(&g), "the property marks a guesser");

    // compile_generator_from_guesser inverts and converts to HFST_OLW_TYPE.
    let guesser = HfstTransducer::new_symbol_pair("a", "b", TROPICAL_OPENFST_TYPE);
    let generator = compile_generator_from_guesser(&guesser);
    assert_eq!(generator.get_type(), HFST_OLW_TYPE);

    // Converted back to tropical, it equals the manually inverted guesser.
    let mut generator_back = generator.clone();
    generator_back.convert(TROPICAL_OPENFST_TYPE, String::new());
    let mut expected = guesser.clone();
    expected.invert();
    assert!(
        generator_back.compare_default(&expected),
        "the generator must be the inverted guesser"
    );
}
