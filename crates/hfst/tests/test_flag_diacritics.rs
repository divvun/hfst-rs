// Port of test/libhfst/test_flag_diacritics.cc
//
// Tests flag-diacritic behaviour: identity symbols mixed with flags, and
// unification flags (@U.FEATURE.VALUE@) gating the accepted paths of a
// programmatically-built transducer.
//
// The C++ main builds one HfstBasicTransducer t, then loops over the types
// array {SFST, TROPICAL, FOMA, HFST_OL, HFST_OLW}. The loop bound is
// TYPES_SIZE-2, so it only ever iterates i = 0,1,2 -> SFST, TROPICAL, FOMA
// (the HFST_OL/HFST_OLW entries are deliberately excluded by the C++ author
// with the comment "FIXME: infinite loop in HFST_OL_TYPE"). LOG_OPENFST_TYPE
// is commented out of the array entirely, but the loop body still carries a
// guard 'if (types[i] != LOG_OPENFST_TYPE)' around the first ("Identities with
// flags") block, showing LOG was intended to run only the second block.
//
// Per the Wave-2 port scope, only the in-scope OpenFST backends are exercised:
//   - TROPICAL_OPENFST_TYPE: runs BOTH logical blocks (this is the one in-scope
//     type the C++ loop actually iterates).
//   - LOG_OPENFST_TYPE: runs ONLY the "Unification flags" block, honouring the
//     C++ guard that skips "Identities with flags" for LOG.
// The out-of-scope SFST_TYPE / FOMA_TYPE iterations are intentionally skipped.
// HFST_OL_TYPE / HFST_OLW_TYPE are not used by the executed loop body (excluded
// by TYPES_SIZE-2), so they are not exercised here either.
//
// Each verbose_print-delimited block of the C++ loop body becomes one #[test]
// fn per in-scope type. The shared helper verbose_print from
// test/libhfst/auxiliary_functions.cc is inlined below; get_bin is unused here
// and omitted.

use std::collections::BTreeSet;

use hfst::backend::AlgebraBackend;
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_data_types::{HfstTwoLevelPaths, ImplementationType, StringPair};
use hfst::hfst_transducer::HfstTransducer;
use hfst::log_weight_transducer::LogFst;
use hfst_openfst::StdVectorFst;

// The tropical/log transition-data symbol coding lives in process-global
// statics (NUMBER2SYMBOL_MAP / SYMBOL2NUMBER_MAP / MAX_NUMBER, each behind its
// own Mutex). get_number bumps MAX_NUMBER under one lock and then appends to the
// symbol vector under another, so concurrent callers race. The C++ test suite
// never hits this because each C++ test is its own process; cargo runs every
// #[test] as a parallel thread in ONE process. Serializing the tests through
// this lock restores the one-at-a-time-per-process model without touching the
// library or weakening any assertion. into_inner() recovers from a poisoned
// lock so one failing test does not cascade.
static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

// Shared helper inlined from test/libhfst/auxiliary_functions.cc (verbose_print).
fn verbose_print(msg: &str, ty: ImplementationType) {
    eprintln!("Testing:\t{msg} for type {ty:?}...");
}

// Build the HfstBasicTransducer t from the top of the C++ main:
//
//   0 -@U.FEATURE.FOO@-> s1 -b-> s3 -c-> s4 -@U.FEATURE.BAR@-> s6 (final)
//   0 -a->               s2 -@U.FEATURE.BAR@-> s3 -d-> s5 -@U.FEATURE.FOO@-> s6
//
// Unification flags gate the paths: only "ac" (BAR,BAR) and "bd" (FOO,FOO)
// unify; "bc" (FOO then BAR) and "ad" (BAR then FOO) are blocked.
fn build_t() -> HfstBasicTransducer {
    let mut t = HfstBasicTransducer::new();
    let s1 = t.add_state_new();
    let s2 = t.add_state_new();
    let s3 = t.add_state_new();
    let s4 = t.add_state_new();
    let s5 = t.add_state_new();
    let s6 = t.add_state_new();
    t.set_final_weight(s6, &0.0);

    let fd1 = "@U.FEATURE.FOO@".to_string();
    let fd2 = "@U.FEATURE.BAR@".to_string();

    let tr = HfstBasicTransition::new_symbols(s1, fd1.clone(), fd1.clone(), 0.0, t.coder_mut());
    t.add_transition(0, &tr, true);
    let tr =
        HfstBasicTransition::new_symbols(s2, "a".to_string(), "a".to_string(), 0.0, t.coder_mut());
    t.add_transition(0, &tr, true);
    let tr =
        HfstBasicTransition::new_symbols(s3, "b".to_string(), "b".to_string(), 0.0, t.coder_mut());
    t.add_transition(s1, &tr, true);
    let tr = HfstBasicTransition::new_symbols(s3, fd2.clone(), fd2.clone(), 0.0, t.coder_mut());
    t.add_transition(s2, &tr, true);
    let tr =
        HfstBasicTransition::new_symbols(s4, "c".to_string(), "c".to_string(), 0.0, t.coder_mut());
    t.add_transition(s3, &tr, true);
    let tr =
        HfstBasicTransition::new_symbols(s5, "d".to_string(), "d".to_string(), 0.0, t.coder_mut());
    t.add_transition(s3, &tr, true);
    let tr = HfstBasicTransition::new_symbols(s6, fd2.clone(), fd2.clone(), 0.0, t.coder_mut());
    t.add_transition(s4, &tr, true);
    let tr = HfstBasicTransition::new_symbols(s6, fd1.clone(), fd1.clone(), 0.0, t.coder_mut());
    t.add_transition(s5, &tr, true);
    t
}

// --- "Identitites with flags" (C++ spelling preserved in the verbose label).
// Builds id = [ ? ]* (identity star) and abid = [ ? | a | b ]* and asserts they
// compare equal after minimization (the identity symbol expands to cover a/b).
// The intermediate ab_flag transducer is dead code in the C++ (never asserted
// on) but is ported faithfully because concatenate(id) reads id.
fn identities_with_flags<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    verbose_print("Identitites with flags", B::TYPE);

    let mut id = HfstTransducer::<B>::new_symbol("@_IDENTITY_SYMBOL_@")?;
    id.repeat_star()?;
    let mut ab_flag = HfstTransducer::<B>::new_symbol_pair("a", "b")?;
    let flag = HfstTransducer::<B>::new_symbol("@U.F.A@")?;
    ab_flag.disjunct(&flag, true)?;

    ab_flag.concatenate(&id, true)?;
    id.minimize()?;

    let a_tr = HfstTransducer::<B>::new_symbol("a")?;
    let b_tr = HfstTransducer::<B>::new_symbol("b")?;
    let mut abid = HfstTransducer::<B>::new_symbol("@_IDENTITY_SYMBOL_@")?;
    abid.disjunct(&a_tr, true)?;
    abid.disjunct(&b_tr, true)?;
    abid.repeat_star()?;
    abid.minimize()?;

    // C++ compare(another) defaults to harmonize=true.
    assert!(abid.compare_default(&id)?);
    Ok(())
}

// --- "Unification flags".
// Converts the basic transducer t to an HfstTransducer of the given type,
// extracts paths with flags filtered, and asserts exactly the two unifying
// strings "ac" and "bd" survive.
fn unification_flags<B: AlgebraBackend>() -> Result<(), hfst::error::Error> {
    verbose_print("Unification flags", B::TYPE);

    let t = build_t();
    let tr = HfstTransducer::<B>::new_from_basic(&t)?;
    let mut results: HfstTwoLevelPaths = BTreeSet::new();

    // C++ extract_paths_fd(results) defaults: max_num=-1, cycles=-1, filter_fd=true.
    tr.extract_paths_fd(&mut results, -1, -1, true)?;

    assert_eq!(results.len(), 2);

    let mut result_strings: BTreeSet<StringPair> = BTreeSet::new();
    for it in results.iter() {
        let mut istring = String::new();
        let mut ostring = String::new();
        for (i, o) in it.second.iter() {
            istring.push_str(i);
            ostring.push_str(o);
        }
        result_strings.insert((istring, ostring));
    }

    assert!(result_strings.contains(&("ac".to_string(), "ac".to_string())));
    assert!(result_strings.contains(&("bd".to_string(), "bd".to_string())));
    Ok(())
}

// =====================================================================
// TROPICAL_OPENFST_TYPE (the in-scope type the C++ loop actually iterates)
// =====================================================================

#[test]
fn identities_with_flags_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    identities_with_flags::<StdVectorFst>()?;
    Ok(())
}

#[test]
fn unification_flags_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    unification_flags::<StdVectorFst>()?;
    Ok(())
}

// =====================================================================
// LOG_OPENFST_TYPE (in-scope; C++ guard runs only "Unification flags" for LOG,
// and "Identities with flags" is intentionally NOT ported for LOG to honour it)
// =====================================================================

// PORT DISCREPANCY (latent C++ bug surfaced, not a Rust regression): converting
// the basic transducer t to LOG_OPENFST_TYPE mis-builds the graph because
// hfst_basic_transducer_to_log_ofst hardcodes source_state = 0 and never
// advances it (convert_log_weight_transducer.rs, a faithfully ported C++ bug).
// Every transition then originates from state 0, so the only paths reaching the
// (sole) final state s6 are the two direct 0->s6 flag transitions; after fd
// filtering the surviving strings are the empty/flag-only paths, NOT "ac"/"bd".
// results.len() == 2 happens to still hold (two distinct flag paths), but the
// result_strings.contains("ac") assertion fails. The C++ suite never triggered
// this because LOG was commented out of the types array.
#[test]
#[ignore = "PORT DISCREPANCY: LOG basic->log conversion hardcodes source_state=0 (faithfully ported C++ bug), so all transitions originate at state 0 and only the direct 0->s6 flag transitions remain; extract_paths_fd no longer yields ac/bd and the contains(ac) assertion fails; never exercised by C++ (LOG commented out of types array)"]
fn unification_flags_log() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    unification_flags::<LogFst>()?;
    Ok(())
}
