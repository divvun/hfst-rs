// Port of test/libhfst/test_hfst_basic_transducer.cc
//
// Tests the standalone HfstBasicTransducer graph type: construction, copying,
// the get_final_weight / read_in_att_format exceptions, alphabet pruning,
// substitution, the EmptyStringException on an empty-symbol transition,
// building graphs with unknown/identity symbols, and iterating through states
// and transitions.
//
// HfstBasicTransducer is implementation-type independent, so unlike the other
// suites this C++ test does NOT loop over the {SFST, FOMA, TROPICAL, LOG}
// backends. The single backend-specific block (the unknown/identity disjunct)
// is gated on is_implementation_type_available(SFST_TYPE); SFST is out of scope
// for the Wave-2 port, so that disjunct is intentionally skipped and only the
// in-scope HfstBasicTransducer construction it performs is ported.
//
// Each verbose_print-delimited block of the C++ main becomes one #[test] fn.
// The shared helper from test/libhfst/auxiliary_functions.cc that this suite
// uses (verbose_print) is inlined below; get_bin is unused here and omitted.

use std::collections::BTreeSet;

use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_symbol_defs::StringPairSet;

// The tropical transition-data symbol coding lives in process-global statics
// (NUMBER2SYMBOL_MAP / SYMBOL2NUMBER_MAP / MAX_NUMBER, each behind its own
// Mutex). get_number bumps MAX_NUMBER under one lock and then appends to the
// symbol vector under another, so concurrent callers race and get_symbol can
// read a MAX_NUMBER ahead of the vector length and throw HfstFatalException. The
// C++ test suite never hits this because each C++ test is its own process; cargo
// runs every #[test] as a parallel thread in ONE process. Serializing the tests
// through this lock restores the one-at-a-time-per-process model without
// touching the library or weakening any assertion. It also makes the global
// panic-hook swapping in expect_hfst_exception safe (only one test at a time).
// into_inner() recovers from a poisoned lock so one failing test does not
// cascade.
static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

// Shared helper inlined from test/libhfst/auxiliary_functions.cc.
fn verbose_print(msg: &str) {
    eprintln!("Testing:\t{msg} (type undefined)...");
}

// Run a closure that is expected to throw an HFST exception (a panic_any
// carrying a typed exception payload). The C++ does this with try { ... }
// catch (const E&). Returns the panic payload so the caller can downcast to the
// specific exception type the C++ catch named. The panic hook is silenced so the
// expected, caught panic does not print a backtrace.
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

// Build the [a:b c:d] transducer with final weight 1.0 used by several blocks
// of the C++ main, returning it together with the two added state numbers.
fn build_abcd() -> (HfstBasicTransducer, u32, u32) {
    let mut t = HfstBasicTransducer::new();
    let s1 = t.add_state_new();
    let tr =
        HfstBasicTransition::new_symbols(s1, "a".to_string(), "b".to_string(), 1.2, t.coder_mut());
    t.add_transition(0, &tr, true);
    let s2 = t.add_state_new();
    let tr =
        HfstBasicTransition::new_symbols(s2, "c".to_string(), "d".to_string(), 0.8, t.coder_mut());
    t.add_transition(s1, &tr, true);
    t.set_final_weight(s2, &1.0);
    (t, s1, s2)
}

// --- "HfstBasicTransducer construction"
#[test]
fn construction() {
    let _g = serialized();
    verbose_print("HfstBasicTransducer construction");

    let mut t = HfstBasicTransducer::new();
    assert!(!t.is_final_state(0));

    let s1 = t.add_state_new();
    assert_eq!(s1, 1);
    let tr =
        HfstBasicTransition::new_symbols(s1, "a".to_string(), "b".to_string(), 1.2, t.coder_mut());
    t.add_transition(0, &tr, true);
    assert!(!t.is_final_state(s1));

    let s2 = t.add_state_new();
    // The C++ writes 'assert(s2 = 2)' (assignment, a typo for '=='); s2 already
    // equals 2 from add_state(), so the intended check is s2 == 2.
    assert_eq!(s2, 2);
    let tr =
        HfstBasicTransition::new_symbols(s2, "c".to_string(), "d".to_string(), 0.8, t.coder_mut());
    t.add_transition(s1, &tr, true);
    assert!(!t.is_final_state(s2));

    t.set_final_weight(s2, &1.0);
    assert!(t.is_final_state(s2) && t.get_final_weight(s2) == 1.0);

    // Take a copy (C++ 'HfstBasicTransducer tc(t)').
    let tc = t.clone();
    assert!(tc.is_final_state(s2) && tc.get_final_weight(s2) == 1.0);
}

// --- "HfstBasicTransducer exceptions"
#[test]
fn exceptions() {
    let _g = serialized();
    verbose_print("HfstBasicTransducer exceptions");

    let (t, _s1, s2) = build_abcd();

    // Asking the weight of a non-final state. The C++ loop varies s over 0..5
    // (skipping s2) but always calls get_final_weight(0); state 0 is not final,
    // so every call throws StateIsNotFinalException.
    for s in 0u32..5 {
        if s != s2 {
            let payload = expect_hfst_exception(|| {
                let _w = t.get_final_weight(0);
            });
            assert!(
                payload
                    .downcast_ref::<hfst::error::Error>()
                    .filter(|__e| matches!(__e.kind, hfst::error::ErrorKind::StateIsNotFinal))
                    .is_some(),
                "expected StateIsNotFinalException"
            );
        }
    }

    // Reading a file in non-valid AT&T format. The third line "1\t2\tb" has only
    // three fields, so add_att_line cannot parse it and read_in_att_format
    // throws NotValidAttFormatException.
    let path = std::env::temp_dir().join("test_hfst_basic_transducer.att");
    std::fs::write(&path, "0\n0\t1\ta\tb\n1\t2\tb\n2\n").unwrap();

    let bytes = std::fs::read(&path).unwrap();

    let payload = expect_hfst_exception(|| {
        let mut reader = std::io::Cursor::new(bytes.clone());
        let mut linecount: u32 = 0;
        let _foo =
            HfstBasicTransducer::read_in_att_format_file(&mut reader, "@0@", &mut linecount, false);
    });
    assert!(
        payload
            .downcast_ref::<hfst::error::Error>()
            .filter(|__e| matches!(__e.kind, hfst::error::ErrorKind::NotValidAttFormat))
            .is_some(),
        "expected NotValidAttFormatException"
    );

    let _ = std::fs::remove_file(&path);
}

// --- "HfstBasicTransducer: symbol handling"
#[test]
fn symbol_handling() {
    let _g = serialized();
    verbose_print("HfstBasicTransducer: symbol handling");

    let (mut t, _s1, _s2) = build_abcd();

    t.add_symbol_to_alphabet(&"foo".to_string());
    // C++ prune_alphabet() defaults to force=true.
    t.prune_alphabet(true);

    let alphabet = t.get_alphabet();
    // {epsilon, unknown, identity} special symbols + {a, b, c, d}; "foo" is
    // pruned because it never occurs in a transition.
    assert_eq!(alphabet.len(), 7);
    assert!(alphabet.contains("a"));
    assert!(alphabet.contains("b"));
    assert!(alphabet.contains("c"));
    assert!(alphabet.contains("d"));
    assert!(!alphabet.contains("foo"));
}

// --- "HfstBasicTransducer: substitute"
// The C++ block has no assertions: it only checks that substituting the pair
// a:b with the set {A:B, C:D} does not throw.
#[test]
fn substitute() {
    let _g = serialized();
    verbose_print("HfstBasicTransducer: substitute");

    let mut tr = HfstBasicTransducer::new();
    tr.add_state_new();
    let arc =
        HfstBasicTransition::new_symbols(1, "a".to_string(), "b".to_string(), 0.0, tr.coder_mut());
    tr.add_transition(0, &arc, true);
    let arc =
        HfstBasicTransition::new_symbols(1, "a".to_string(), "b".to_string(), 0.0, tr.coder_mut());
    tr.add_transition(0, &arc, true);
    tr.set_final_weight(1, &0.0);

    let mut sps: StringPairSet = BTreeSet::new();
    sps.insert(("A".to_string(), "B".to_string()));
    sps.insert(("C".to_string(), "D".to_string()));
    tr.substitute_pair_with_set(&("a".to_string(), "b".to_string()), &sps);
}

// --- "HfstBasicTransducer: EmptyStringException"
#[test]
fn empty_string_exception() {
    let _g = serialized();
    verbose_print("HfstBasicTransducer: EmptyStringException");

    // Constructing a transition with empty input/output symbols throws
    // EmptyStringException (from the transition-data constructor), before the
    // add_transition call can run.
    let payload = expect_hfst_exception(|| {
        let mut empty_symbol = HfstBasicTransducer::new();
        let tr = HfstBasicTransition::new_symbols(
            0,
            "".to_string(),
            "".to_string(),
            0.0,
            empty_symbol.coder_mut(),
        );
        empty_symbol.add_transition(0, &tr, true);
    });
    assert!(
        payload
            .downcast_ref::<hfst::error::Error>()
            .filter(|__e| matches!(__e.kind, hfst::error::ErrorKind::EmptyString))
            .is_some(),
        "expected EmptyStringException"
    );
}

// --- "HfstBasicTransducer: unknown and indentity symbols"
// In the xerox formalism used here, "?" means the unknown symbol and "?:?" the
// identity pair. The C++ builds tr1 = [ ?:foo ] and tr2 = [ [ ?:? ] [ bar:bar ] ]
// and then, ONLY if SFST is available, converts both to HfstTransducers and
// disjuncts them. SFST_TYPE is out of scope for the Wave-2 port, so that facade
// disjunct block is intentionally skipped; the in-scope part is the construction
// of the two HfstBasicTransducers exercised here.
#[test]
fn unknown_and_identity_symbols() {
    let _g = serialized();
    verbose_print("HfstBasicTransducer: unknown and indentity symbols");

    // tr1 is [ ?:foo ]
    let mut tr1 = HfstBasicTransducer::new();
    tr1.add_state(1);
    tr1.set_final_weight(1, &0.0);
    let arc = HfstBasicTransition::new_symbols(
        1,
        "@_UNKNOWN_SYMBOL_@".to_string(),
        "foo".to_string(),
        0.0,
        tr1.coder_mut(),
    );
    tr1.add_transition(0, &arc, true);

    // tr2 is [ [ ?:? ] [ bar:bar ] ]
    let mut tr2 = HfstBasicTransducer::new();
    tr2.add_state(1);
    tr2.add_state(2);
    tr2.set_final_weight(2, &0.0);
    let arc = HfstBasicTransition::new_symbols(
        1,
        "@_IDENTITY_SYMBOL_@".to_string(),
        "@_IDENTITY_SYMBOL_@".to_string(),
        0.0,
        tr2.coder_mut(),
    );
    tr2.add_transition(0, &arc, true);
    let arc = HfstBasicTransition::new_symbols(
        2,
        "bar".to_string(),
        "bar".to_string(),
        0.0,
        tr2.coder_mut(),
    );
    tr2.add_transition(1, &arc, true);

    // Sanity: the constructed graphs have the expected final states.
    assert!(tr1.is_final_state(1));
    assert!(tr2.is_final_state(2));

    // SFST facade disjunct block intentionally skipped (out of scope).
}

// --- renumber_states (librarify regression, not a C++ test-suite block)
// renumber_states() is the pure-renumber primitive lifted out of
// hfst-preprocess-for-optimized-lookup-format: it returns a copy whose states
// are renumbered in discovery order (state 0 stays 0; every other state takes
// the next free id the first time it is reached, as source or as arc target),
// copying every transition verbatim with remapped targets. Build a graph whose
// reachable states are NOT in sequential order so the renumber genuinely
// reorders, and check the compacted result preserves the language "ab".
#[test]
fn renumber_states_compacts_in_discovery_order() {
    let _g = serialized();
    verbose_print("HfstBasicTransducer: renumber_states");

    // 0 -(a:a)-> 2 -(b:b)-> 1(final); state 1 is reached only after state 2.
    let mut t = HfstBasicTransducer::new();
    t.add_state(2);
    let tr =
        HfstBasicTransition::new_symbols(2, "a".to_string(), "a".to_string(), 0.0, t.coder_mut());
    t.add_transition(0, &tr, true);
    let tr =
        HfstBasicTransition::new_symbols(1, "b".to_string(), "b".to_string(), 0.0, t.coder_mut());
    t.add_transition(2, &tr, true);
    t.set_final_weight(1, &0.5);

    let r = t.renumber_states();

    // Discovery order: 0->0, then target 2 becomes 1, then source 1 becomes 2.
    assert_eq!(r.get_max_state(), 2);

    // state 0: single a:a arc to the renumbered "2" (now id 1).
    let s0: Vec<_> = r.iter().next().unwrap().iter().collect();
    assert_eq!(s0.len(), 1);
    assert_eq!(s0[0].get_target_state(), 1);
    assert_eq!(s0[0].get_input_symbol(r.coder()), "a");

    // new id 1 (old state 2): single b:b arc to old state 1, now id 2.
    let s1: Vec<_> = r.iter().nth(1).unwrap().iter().collect();
    assert_eq!(s1.len(), 1);
    assert_eq!(s1[0].get_target_state(), 2);
    assert_eq!(s1[0].get_input_symbol(r.coder()), "b");
    assert!(!r.is_final_state(1));

    // new id 2 (old final state 1): no arcs, final, weight carried over.
    assert!(r.iter().nth(2).unwrap().is_empty());
    assert!(r.is_final_state(2));
    assert_eq!(r.get_final_weight(2), 0.5);
}

// --- kill_paths (librarify regression, not a C++ test-suite block)
// kill_paths(sym) returns a copy with every transition whose input or output is
// `sym` dropped, surviving states renumbered. Build a two-path transducer and
// kill one branch's symbol; the surviving branch stays, the killed arc is gone.
#[test]
fn kill_paths_drops_matching_arcs() {
    let _g = serialized();
    verbose_print("HfstBasicTransducer: kill_paths");

    // 0 -(a:a)-> 1(final), 0 -(x:x)-> 2(final)
    let mut t = HfstBasicTransducer::new();
    t.add_state(2);
    let tr =
        HfstBasicTransition::new_symbols(1, "a".to_string(), "a".to_string(), 0.0, t.coder_mut());
    t.add_transition(0, &tr, true);
    let tr =
        HfstBasicTransition::new_symbols(2, "x".to_string(), "x".to_string(), 0.0, t.coder_mut());
    t.add_transition(0, &tr, true);
    t.set_final_weight(1, &0.0);
    t.set_final_weight(2, &0.0);

    let killed = t.kill_paths("x");

    // State 0 keeps only the a:a arc; the x:x arc is gone.
    let s0: Vec<_> = killed.iter().next().unwrap().iter().collect();
    assert_eq!(s0.len(), 1);
    assert_eq!(s0[0].get_input_symbol(killed.coder()), "a");

    // No surviving transition anywhere mentions the killed symbol.
    for transitions in killed.iter() {
        for arc in transitions.iter() {
            assert_ne!(arc.get_input_symbol(killed.coder()), "x");
            assert_ne!(arc.get_output_symbol(killed.coder()), "x");
        }
    }
}

// --- input_symbols_used (librarify regression, not a C++ test-suite block)
// input_symbols_used() collects only the input side of each transition (the
// input-only sibling of symbols_used), used by hfst-compose-intersect's alphabet
// diagnostics.
#[test]
fn input_symbols_used_collects_input_side_only() {
    let _g = serialized();
    verbose_print("HfstBasicTransducer: input_symbols_used");

    let mut t = HfstBasicTransducer::new();
    let tr =
        HfstBasicTransition::new_symbols(1, "a".to_string(), "x".to_string(), 0.0, t.coder_mut());
    t.add_transition(0, &tr, true);
    let tr =
        HfstBasicTransition::new_symbols(2, "b".to_string(), "y".to_string(), 0.0, t.coder_mut());
    t.add_transition(1, &tr, true);
    t.set_final_weight(2, &0.0);

    let inputs = t.input_symbols_used();
    assert!(inputs.contains("a"));
    assert!(inputs.contains("b"));
    // Output-side symbols must not appear.
    assert!(!inputs.contains("x"));
    assert!(!inputs.contains("y"));
}

// --- pair_target_state (librarify regression, not a C++ test-suite block)
// The pair-path recogniser step lifted from hfst-pair-test: follow the exact
// (in,out) transition, else fall back to an @_IDENTITY_SYMBOL_@ identity arc when
// the queried pair is an unknown identity.
#[test]
fn pair_target_state_with_identity_fallback() {
    let _g = serialized();
    verbose_print("HfstBasicTransducer: pair_target_state");

    let mut t = HfstBasicTransducer::new();
    let tr =
        HfstBasicTransition::new_symbols(1, "a".to_string(), "b".to_string(), 0.0, t.coder_mut());
    t.add_transition(0, &tr, true);
    let tr = HfstBasicTransition::new_symbols(
        2,
        "@_IDENTITY_SYMBOL_@".to_string(),
        "@_IDENTITY_SYMBOL_@".to_string(),
        0.0,
        t.coder_mut(),
    );
    t.add_transition(0, &tr, true);
    t.set_final_weight(1, &0.0);
    t.set_final_weight(2, &0.0);

    let empty: BTreeSet<String> = BTreeSet::new();
    let known: BTreeSet<String> = ["z".to_string()].into_iter().collect();

    // Exact pair match wins.
    assert_eq!(t.pair_target_state(0, "a", "b", &empty), Some(1));
    // An unknown identity falls back to the identity transition.
    assert_eq!(t.pair_target_state(0, "z", "z", &empty), Some(2));
    // A *known* identity does not take the fallback.
    assert_eq!(t.pair_target_state(0, "z", "z", &known), None);
    // A non-identity pair with no exact match never falls back.
    assert_eq!(t.pair_target_state(0, "x", "y", &empty), None);
}

// --- transform_weights (librarify regression, not a C++ test-suite block)
// The do_reweight rebuild lifted from hfst-reweight: f receives (weight, in, out)
// — (w, None, None) for a final weight, (w, Some, Some) for an arc — so it can
// reweight conditionally on the symbols.
#[test]
fn transform_weights_applies_per_arc_and_final_symbol_aware() {
    let _g = serialized();
    verbose_print("HfstBasicTransducer: transform_weights");

    let mut t = HfstBasicTransducer::new();
    let tr =
        HfstBasicTransition::new_symbols(1, "a".to_string(), "a".to_string(), 1.0, t.coder_mut());
    t.add_transition(0, &tr, true);
    let tr =
        HfstBasicTransition::new_symbols(2, "b".to_string(), "b".to_string(), 1.0, t.coder_mut());
    t.add_transition(1, &tr, true);
    t.set_final_weight(2, &0.5);

    // +10 only to arcs whose input symbol is "a"; finals (None) get +1.
    let r = t.transform_weights(|w, i, _o| match i {
        Some("a") => w + 10.0,
        None => w + 1.0,
        _ => w,
    });

    // state 0: a-arc 1.0 -> 11.0
    let s0: Vec<_> = r.iter().next().unwrap().iter().collect();
    assert_eq!(s0.len(), 1);
    assert!((s0[0].get_weight() - 11.0).abs() < 1e-6);
    // state 1: b-arc 1.0 unchanged
    let s1: Vec<_> = r.iter().nth(1).unwrap().iter().collect();
    assert!((s1[0].get_weight() - 1.0).abs() < 1e-6);
    // final weight 0.5 -> 1.5 (the None, None branch)
    assert!((r.get_final_weight(2) - 1.5).abs() < 1e-6);
}

// --- summarize (librarify regression, not a C++ test-suite block)
// The single-pass statistics lifted from hfst-summarize. Build {cat, cab} (a
// shared "ca" prefix, branching at state 2) and check the core figures.
#[test]
fn summarize_counts_states_arcs_and_alphabet() {
    let _g = serialized();
    verbose_print("HfstBasicTransducer: summarize");

    let mut t = HfstBasicTransducer::new();
    for (from, to, sym) in [(0, 1, "c"), (1, 2, "a"), (2, 3, "t"), (2, 4, "b")] {
        let tr = HfstBasicTransition::new_symbols(
            to,
            sym.to_string(),
            sym.to_string(),
            0.0,
            t.coder_mut(),
        );
        t.add_transition(from, &tr, true);
    }
    t.set_final_weight(3, &0.0);
    t.set_final_weight(4, &0.0);

    let s = t.summarize();
    assert_eq!(s.states, 5); // 0..4
    assert_eq!(s.arcs, 4);
    assert_eq!(s.final_states, 2);
    assert!(s.acceptor); // every arc is x:x
    assert!(!s.cyclic);
    assert_eq!(s.densest_arcs, 2); // state 2 branches to t and b
    let alpha: Vec<&str> = s.found_alphabet.iter().map(|x| x.as_str()).collect();
    assert_eq!(alpha, vec!["a", "b", "c", "t"]);
}

// --- "HfstBasicTransducer: iterating through"
// The C++ block has no assertions: it walks every state and its transitions,
// printing source/target/input/output/weight, and the final weight of final
// states, to stderr. Ported faithfully as a walk over the iterator API.
#[test]
fn iterating_through() {
    let _g = serialized();
    verbose_print("HfstBasicTransducer: iterating through");

    let (t, _s1, _s2) = build_abcd();

    let mut source_state: u32 = 0;
    for it in t.iter() {
        for tr_it in it.iter() {
            eprintln!(
                "{}\t{}\t{}\t{}\t{}",
                source_state,
                tr_it.get_target_state(),
                tr_it.get_input_symbol(t.coder()),
                tr_it.get_output_symbol(t.coder()),
                tr_it.get_weight()
            );
        }
        if t.is_final_state(source_state) {
            eprintln!("{}\t{}", source_state, t.get_final_weight(source_state));
        }
        source_state += 1;
    }
}
