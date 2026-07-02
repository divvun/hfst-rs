// Port of test/libhfst/test_examples.cc
//
// A grab-bag "examples" suite. The C++ main is structured as:
//   1. A loop over the implementation types {SFST, TROPICAL, FOMA} (LOG is
//      commented out in the C++ array) that, for each available type:
//        a. converts three hand-built HfstBasicTransducers (tr1 = [ ?:foo ],
//           tr2 = [ [ ?:? ] [ bar:bar ] ], disj = the expected disjunction) to
//           the implementation type, computes Tr1.disjunct(Tr2).minimize(), and
//           asserts it compares equal to Disj (the unknown/identity harmonising
//           oracle).
//        b. opens a deliberately malformed AT&T file (the second line "1 c d"
//           is missing the output field) through the FILE*-based AT&T reader and
//           asserts NotValidAttFormatException is thrown.
//   2. A FOMA-only block (replace_up on a tiny lexicon + extract_paths). FOMA is
//      out of scope and unavailable, so it is skipped (and omitted here).
//   3. An SFST-only compose_intersect block. SFST is out of scope -- skipped.
//   4. An SFST-only substitute(&function) block (mapping a:a to a:<back_wovel>).
//      SFST is out of scope -- skipped. (The free function "function" the C++
//      passes is only ever used by this SFST block.)
//
// Per the Wave-2 port scope only the in-scope OpenFST backends are exercised in
// block 1: TROPICAL_OPENFST_TYPE and LOG_OPENFST_TYPE (the latter following the
// sibling ported suites' convention of also running LOG, even though the C++
// array commented it out). The out-of-scope SFST_TYPE / FOMA_TYPE / XFSM_TYPE
// iterations are intentionally skipped -- is_implementation_type_available
// returns false for them in this build.
//
// Each in-scope type yields one #[test] for block 1a (the disjunct/minimize
// oracle) and one #[test] for block 1b (the NotValidAttFormatException check).
//
// Shared helper from test/libhfst/auxiliary_functions.cc: verbose_print is
// inlined as a plain message printer (get_bin is unused by this suite).

use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_data_types::ImplementationType::{self, LOG_OPENFST_TYPE, TROPICAL_OPENFST_TYPE};
use hfst::hfst_transducer::HfstTransducer;

// The tropical/log transition-data symbol coding lives in process-global
// statics behind Mutexes. cargo runs every #[test] as a parallel thread in ONE
// process, but each C++ test was its own process. Serializing the tests through
// this lock restores the one-at-a-time-per-process model without touching the
// library or weakening any assertion. It also makes the global panic-hook
// swapping in expect_hfst_exception safe (only one test at a time). into_inner()
// recovers from a poisoned lock so one failing test does not cascade.
static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

// Shared helper inlined from test/libhfst/auxiliary_functions.cc (verbose_print).
fn verbose_print(msg: &str, ty: ImplementationType) {
    eprintln!("Testing:\t{msg} for type {ty:?}...");
}

// Run a closure that is expected to throw an HFST exception (a panic_any
// carrying a typed exception payload). The C++ does this with try { ... }
// catch (NotValidAttFormatException e). Returns the panic payload so the caller
// can downcast to the specific exception type the C++ catch named. The panic
// hook is silenced so the expected, caught panic does not print a backtrace.
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

// Build tr1 = [ @_UNKNOWN_SYMBOL_@:foo ] as in the C++ main.
fn build_tr1() -> HfstBasicTransducer {
    let mut tr1 = HfstBasicTransducer::new();
    tr1.add_state(1);
    tr1.set_final_weight(1, &0.0);
    let tr = HfstBasicTransition::new_symbols(
        1,
        "@_UNKNOWN_SYMBOL_@".to_string(),
        "foo".to_string(),
        0.0,
        tr1.coder_mut(),
    );
    tr1.add_transition(0, &tr, true);
    tr1
}

// Build tr2 = [ [ @_IDENTITY_SYMBOL_@:@_IDENTITY_SYMBOL_@ ] [ bar:bar ] ].
fn build_tr2() -> HfstBasicTransducer {
    let mut tr2 = HfstBasicTransducer::new();
    tr2.add_state(1);
    tr2.add_state(2);
    tr2.set_final_weight(2, &0.0);
    let tr = HfstBasicTransition::new_symbols(
        1,
        "@_IDENTITY_SYMBOL_@".to_string(),
        "@_IDENTITY_SYMBOL_@".to_string(),
        0.0,
        tr2.coder_mut(),
    );
    tr2.add_transition(0, &tr, true);
    let tr = HfstBasicTransition::new_symbols(
        2,
        "bar".to_string(),
        "bar".to_string(),
        0.0,
        tr2.coder_mut(),
    );
    tr2.add_transition(1, &tr, true);
    tr2
}

// Build the expected disjunction (the C++ "disj").
fn build_disj() -> HfstBasicTransducer {
    let mut disj = HfstBasicTransducer::new();
    disj.add_state(1);
    disj.add_state(2);
    disj.set_final_weight(2, &0.0);

    let tr = HfstBasicTransition::new_symbols(
        1,
        "@_IDENTITY_SYMBOL_@".to_string(),
        "@_IDENTITY_SYMBOL_@".to_string(),
        0.0,
        disj.coder_mut(),
    );
    disj.add_transition(0, &tr, true);
    let tr = HfstBasicTransition::new_symbols(
        1,
        "foo".to_string(),
        "foo".to_string(),
        0.0,
        disj.coder_mut(),
    );
    disj.add_transition(0, &tr, true);
    let tr = HfstBasicTransition::new_symbols(
        2,
        "@_UNKNOWN_SYMBOL_@".to_string(),
        "foo".to_string(),
        0.0,
        disj.coder_mut(),
    );
    disj.add_transition(0, &tr, true);
    let tr = HfstBasicTransition::new_symbols(
        2,
        "bar".to_string(),
        "foo".to_string(),
        0.0,
        disj.coder_mut(),
    );
    disj.add_transition(0, &tr, true);
    let tr = HfstBasicTransition::new_symbols(
        2,
        "bar".to_string(),
        "bar".to_string(),
        0.0,
        disj.coder_mut(),
    );
    disj.add_transition(1, &tr, true);
    disj
}

// --- Block 1a: expanding unknowns (the disjunct/minimize oracle).
fn run_expanding_unknowns(ty: ImplementationType) -> Result<(), hfst::error::Error> {
    verbose_print("expanding unknowns", ty);

    let mut tr1 = HfstTransducer::new_from_basic(&build_tr1(), ty)?;
    let tr2 = HfstTransducer::new_from_basic(&build_tr2(), ty)?;
    let disj = HfstTransducer::new_from_basic(&build_disj(), ty)?;

    // Tr1.disjunct(Tr2).minimize(); C++ disjunct/compare default harmonize=true.
    tr1.disjunct(&tr2, true)?.minimize()?;
    // Tr1 is expanded to [ @_UNKNOWN_SYMBOL_@:foo | bar:foo ]
    // Tr2 is expanded to
    // [ [ @_IDENTITY_SYMBOL_@:@_IDENTITY_SYMBOL_@ | foo:foo ] [ bar:bar ] ]
    assert!(tr1.compare(&disj, true)?);
    Ok(())
}

// --- Block 1b: NotValidAttFormatException.
// The C++ writes "0 1 a b 0.4\n1 c d\n": the second line "1 c d" is missing the
// output field, so the AT&T reader cannot parse it and throws
// NotValidAttFormatException. The C++ uses the (FILE*, type, epsilon, linecount)
// constructor; read_in_att_format_filename is the facade equivalent that opens
// the file itself.
fn run_not_valid_att_format(ty: ImplementationType) {
    verbose_print("testing NotValidAttFormatException", ty);

    let path = std::env::temp_dir().join(format!("test_examples_{ty:?}.att"));
    std::fs::write(&path, "0 1 a b 0.4\n1 c d\n").unwrap();
    let path_str = path.to_str().unwrap().to_string();

    let r = HfstTransducer::read_in_att_format_filename(&path_str, ty, "@_EPSILON_SYMBOL_@", false);
    assert!(
        matches!(&r, Err(e) if matches!(e.kind, hfst::error::ErrorKind::NotValidAttFormat)),
        "expected NotValidAttFormatException"
    );

    let _ = std::fs::remove_file(&path);
}

// =====================================================================
// TROPICAL_OPENFST_TYPE
// =====================================================================

#[test]
fn expanding_unknowns_tropical() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    run_expanding_unknowns(TROPICAL_OPENFST_TYPE)?;
    Ok(())
}

#[test]
fn not_valid_att_format_tropical() {
    let _g = serialized();
    run_not_valid_att_format(TROPICAL_OPENFST_TYPE);
}

// =====================================================================
// LOG_OPENFST_TYPE
// =====================================================================

#[test]
fn expanding_unknowns_log() -> Result<(), hfst::error::Error> {
    let _g = serialized();
    run_expanding_unknowns(LOG_OPENFST_TYPE)?;
    Ok(())
}

#[test]
fn not_valid_att_format_log() {
    let _g = serialized();
    run_not_valid_att_format(LOG_OPENFST_TYPE);
}
