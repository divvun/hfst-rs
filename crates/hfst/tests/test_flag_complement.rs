// Regression coverage for hfst/hfst#349: the XRE complement operators `~`
// (REGEXP8/9/10 UnaryOp::Complement) and `\` (UnaryOp::TermComplement) must
// treat flag diacritics as ORDINARY symbols, exactly like the already-fixed
// `negate` xfst command (HfstTransducer::negate, upstream commit cdab3f74).
//
// Before the fix, `~"@U.Cap.Obl@"` swallowed the flag (its universe was the
// bare identity `[?:?]*`, and subtract's harmonization erased the flag from A),
// so `~[~flag]` collapsed to the empty language instead of round-tripping. The
// fix routes both arms through HfstTransducer::identity_with_flags_of, which
// inserts A's flags into the identity universe as plain single-symbol arcs.
//
// This is a DELIBERATE DIVERGENCE from upstream C++ XRE (which never fixed the
// `~`/`\` arms) — see docs/spec/.../XreCompiler.md complement-compilation-fn /
// term-complement-compilation-fn. The oracle is the Xerox transcript in the
// issue: `~flag` is the 3-state/6-arc net, `~[~flag]` is the original flag net,
// and `\flag` accepts any single symbol other than the flag.
//
// Tests drive the XreCompiler (the same entry point test_pmatch.rs uses) and
// compare against HfstTransducer::negate() and hand-built expectations.

use hfst::backend::AlgebraBackend;
use hfst::hfst_transducer::HfstTransducer;
use hfst::xre::XreCompiler;
use hfst_openfst::StdVectorFst;

// The tropical transition-data symbol coding lives in process-global statics
// behind Mutexes; cargo runs every #[test] as a parallel thread in ONE process
// where each C++ test was its own process. Serializing through this lock
// restores the one-at-a-time-per-process model. into_inner() recovers from a
// poisoned lock so one failing test does not cascade. (Same pattern as
// test_thfst.rs / test_flag_diacritics.rs.)
static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

const FLAG: &str = "@U.Cap.Obl@";

// Compile one XRE expression to a tropical transducer via the XreCompiler.
fn compile(expr: &str) -> HfstTransducer<StdVectorFst> {
    let mut c = XreCompiler::<StdVectorFst>::new();
    c.compile(expr)
        .unwrap_or_else(|| panic!("XRE compilation of {expr:?} failed"))
}

// A single-symbol transducer accepting exactly `sym`.
fn symbol<B: AlgebraBackend>(sym: &str) -> HfstTransducer<B> {
    HfstTransducer::new_from_symbol(sym).expect("single-symbol transducer")
}

// `regex flag`: the 2-state net accepting exactly the flag string, treated as
// one ordinary symbol.
fn flag_net<B: AlgebraBackend>() -> HfstTransducer<B> {
    symbol(FLAG)
}

// negate()-of-flag: the cross-oracle. This is what the working `negate` xfst
// command produces (the 3-state/6-arc net in the issue transcript).
fn negate_of_flag<B: AlgebraBackend>() -> HfstTransducer<B> {
    let mut t = flag_net::<B>();
    t.negate().expect("negate of an automaton");
    t.clone()
}

// The `negate` command already treats flags as ordinary symbols; double
// negation must be the identity. Locks the already-working library command so
// the shared helper refactor cannot regress it.
#[test]
fn negate_double_identity() {
    let _guard = serialized();

    let original = flag_net::<StdVectorFst>();

    let mut once = original.clone();
    once.negate().expect("first negate");
    let mut twice = once.clone();
    twice.negate().expect("second negate");

    assert!(
        twice.compare(&original, true).expect("compare"),
        "negate(negate(flag)) must equal flag"
    );
    // And a single negate must NOT be the identity (guards against a no-op fix).
    assert!(
        !once.compare(&original, true).expect("compare"),
        "negate(flag) must differ from flag"
    );
}

// The core cross-oracle: XRE `~flag` must compile to exactly the transducer the
// `negate` command produces (flags-as-ordinary complement).
#[test]
fn tilde_matches_negate() {
    let _guard = serialized();

    let tilde = compile(&format!("~\"{FLAG}\""));
    let reference = negate_of_flag::<StdVectorFst>();

    assert!(
        tilde.compare(&reference, true).expect("compare"),
        "~flag must equal negate(flag)"
    );
}

// Double complement round-trips: `~[~flag]` must recover the original flag net.
// Before the fix this collapsed to the empty language.
#[test]
fn tilde_double_identity() {
    let _guard = serialized();

    let double = compile(&format!("~[~\"{FLAG}\"]"));
    let original = flag_net::<StdVectorFst>();

    assert!(
        double.compare(&original, true).expect("compare"),
        "~[~flag] must equal flag"
    );
}

// The complement's alphabet must retain the flag as an ordinary symbol (sigma
// parity with the `negate` command: `?, @U.Cap.Obl@`). Before the fix the flag
// was swallowed and the net degenerated to `?*`.
#[test]
fn tilde_sigma_keeps_flag() {
    let _guard = serialized();

    let tilde = compile(&format!("~\"{FLAG}\""));
    let sigma = tilde.get_alphabet().expect("alphabet");

    assert!(
        sigma.contains(FLAG),
        "~flag alphabet must contain the flag as an ordinary symbol, got {sigma:?}"
    );
}

// Term complement `\flag`: any SINGLE symbol other than the flag, with the flag
// kept in sigma as an ordinary symbol.
//   * flag in alphabet;
//   * does NOT accept the flag string;
//   * DOES accept some other single symbol;
//   * `[\flag | flag]` recovers the full single-symbol universe (identity plus
//     the flag as an ordinary arc), i.e. equals `[? | flag]`.
#[test]
fn term_complement_flag() {
    let _guard = serialized();

    let term = compile(&format!("\\\"{FLAG}\""));

    // Flag stays in the alphabet.
    let sigma = term.get_alphabet().expect("alphabet");
    assert!(
        sigma.contains(FLAG),
        "\\flag alphabet must contain the flag, got {sigma:?}"
    );

    let empty = HfstTransducer::<StdVectorFst>::new();

    // Does NOT accept the flag: intersecting with the flag net is the empty
    // language.
    let mut flag_hit = term.clone();
    flag_hit
        .intersect(&flag_net::<StdVectorFst>(), true)
        .expect("intersect");
    assert!(
        flag_hit.compare(&empty, true).expect("compare"),
        "\\flag must not accept the flag string"
    );

    // DOES accept some other single symbol: intersecting with `a` is non-empty.
    let mut other_hit = term.clone();
    other_hit
        .intersect(&symbol::<StdVectorFst>("a"), true)
        .expect("intersect");
    assert!(
        !other_hit.compare(&empty, true).expect("compare"),
        "\\flag must accept the single symbol a"
    );

    // `[\flag | flag]` == `[? | flag]`: the flag-ordinary single-symbol universe.
    let mut union = term.clone();
    union
        .disjunct(&flag_net::<StdVectorFst>(), true)
        .expect("disjunct");
    let universe =
        HfstTransducer::<StdVectorFst>::identity_with_flags_of(&flag_net::<StdVectorFst>())
            .expect("identity_with_flags_of");
    assert!(
        union.compare(&universe, true).expect("compare"),
        "[\\flag | flag] must equal the [? | flag] single-symbol universe"
    );
}

// Plain-symbol regression: for a non-flag symbol the operators keep their naive
// semantics (`~a` == `[?* - a]`, `\a` == `[? - a]`), built here with the bare
// identity universe (no flags to insert) so the fix is confirmed to touch ONLY
// the flag path.
#[test]
fn plain_symbol_unchanged() {
    let _guard = serialized();

    // ~a == [?:?]* - a.
    let tilde_a = compile("~a");
    let mut expected_tilde = HfstTransducer::<StdVectorFst>::identity_pair();
    expected_tilde.repeat_star().expect("repeat_star");
    expected_tilde.minimize().expect("minimize");
    expected_tilde
        .subtract(&symbol::<StdVectorFst>("a"), true)
        .expect("subtract");
    assert!(
        tilde_a.compare(&expected_tilde, true).expect("compare"),
        "~a must equal [?* - a]"
    );

    // \a == [?] - a.
    let term_a = compile("\\a");
    let mut expected_term = HfstTransducer::<StdVectorFst>::new_from_symbol("@_IDENTITY_SYMBOL_@")
        .expect("identity symbol");
    expected_term
        .subtract(&symbol::<StdVectorFst>("a"), true)
        .expect("subtract");
    assert!(
        term_a.compare(&expected_term, true).expect("compare"),
        "\\a must equal [? - a]"
    );
}
