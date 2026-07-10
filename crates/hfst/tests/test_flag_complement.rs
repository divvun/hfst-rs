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

// ---------------------------------------------------------------------------
// flag-complement.audit follow-up: the containment `$` family and the
// `~$[flag]` filter use-case from the issue thread.
//
// Containment itself does NOT subtract, so the erasure only bites when the
// containment result is later complemented (`~$[flag]`). The audit's key
// empirical question is whether that composite now works end-to-end. The
// containment result's own alphabet already carries the flag (via
// `contains`'s harmonizing `concatenate(t, true)`), and the outer `~` (fixed
// in flag-complement.fix) finds it through `identity_with_flags_of`. The
// containment `?*` wings match a flag anywhere in the string, so a flag
// before/after the target still counts as "containing" it. These tests lock
// that composite; the audit found the `$` family itself needs no change.
// ---------------------------------------------------------------------------

// Concatenate a list of single-symbol nets into one string acceptor (at least
// one symbol required).
fn string_net<B: AlgebraBackend>(syms: &[&str]) -> HfstTransducer<B> {
    let (first, rest) = syms.split_first().expect("string_net needs >=1 symbol");
    let mut t = symbol::<B>(first);
    for s in rest {
        t.concatenate(&symbol::<B>(s), true).expect("concatenate");
    }
    t
}

// True iff `t` accepts (intersects non-empty with) the exact string `other`.
fn accepts<B: AlgebraBackend>(t: &HfstTransducer<B>, other: &HfstTransducer<B>) -> bool {
    let empty = HfstTransducer::<B>::new();
    let mut hit = t.clone();
    hit.intersect(other, true).expect("intersect");
    !hit.compare(&empty, true).expect("compare")
}

// `$[flag]` accepts any string CONTAINING the flag (the flag alone, or a flag
// between other symbols), and rejects flag-free strings. Proves the `?*` wings
// match a flag symbol mid-string.
#[test]
fn containment_of_flag_matches_flag_anywhere() {
    let _guard = serialized();

    let cont = compile(&format!("$[\"{FLAG}\"]"));

    // Flag stays in the containment alphabet.
    assert!(
        cont.get_alphabet().expect("alphabet").contains(FLAG),
        "$[flag] alphabet must contain the flag"
    );

    // Contains the flag: flag alone, and a flag wrapped by other symbols.
    assert!(
        accepts(&cont, &flag_net::<StdVectorFst>()),
        "$[flag] must accept the bare flag"
    );
    assert!(
        accepts(&cont, &string_net::<StdVectorFst>(&["a", FLAG, "b"])),
        "$[flag] must accept a string with the flag in the middle"
    );

    // Does not contain the flag: flag-free strings.
    assert!(
        !accepts(&cont, &symbol::<StdVectorFst>("a")),
        "$[flag] must reject the flag-free string 'a'"
    );
    assert!(
        !accepts(&cont, &string_net::<StdVectorFst>(&["a", "b"])),
        "$[flag] must reject the flag-free string 'ab'"
    );
}

// The issue thread's pressing use-case: the filter `~$[flag]` (accept any
// string that does NOT contain the flag), composed against inputs with and
// without the flag. Before flag-complement.fix the outer `~` swallowed the
// flag and this filter degenerated. Now it is the exact complement of
// `$[flag]`.
#[test]
fn negated_containment_filter_end_to_end() {
    let _guard = serialized();

    let filter = compile(&format!("~$[\"{FLAG}\"]"));

    // The flag survives in the filter's alphabet as an ordinary symbol.
    assert!(
        filter.get_alphabet().expect("alphabet").contains(FLAG),
        "~$[flag] alphabet must keep the flag"
    );

    // Accepts flag-free strings.
    assert!(
        accepts(&filter, &symbol::<StdVectorFst>("a")),
        "~$[flag] must accept the flag-free string 'a'"
    );
    assert!(
        accepts(&filter, &string_net::<StdVectorFst>(&["a", "b"])),
        "~$[flag] must accept the flag-free string 'ab'"
    );

    // Rejects strings containing the flag anywhere.
    assert!(
        !accepts(&filter, &flag_net::<StdVectorFst>()),
        "~$[flag] must reject the bare flag"
    );
    assert!(
        !accepts(&filter, &string_net::<StdVectorFst>(&["a", FLAG, "b"])),
        "~$[flag] must reject a string with the flag in the middle"
    );

    // Filter compose: {a, flag, ab, a-flag-b} & ~$[flag] keeps exactly the
    // flag-free inputs.
    let mut inputs = symbol::<StdVectorFst>("a");
    inputs
        .disjunct(&flag_net::<StdVectorFst>(), true)
        .expect("disjunct");
    inputs
        .disjunct(&string_net::<StdVectorFst>(&["a", "b"]), true)
        .expect("disjunct");
    inputs
        .disjunct(&string_net::<StdVectorFst>(&["a", FLAG, "b"]), true)
        .expect("disjunct");

    let expected = {
        let mut e = symbol::<StdVectorFst>("a");
        e.disjunct(&string_net::<StdVectorFst>(&["a", "b"]), true)
            .expect("disjunct");
        e
    };

    let mut filtered = inputs.clone();
    filtered.intersect(&filter, true).expect("intersect");
    assert!(
        filtered.compare(&expected, true).expect("compare"),
        "{{a, flag, ab, a-flag-b}} & ~$[flag] must equal {{a, ab}}"
    );
}

// ---------------------------------------------------------------------------
// DOCUMENTED DEFERRALS (flag-complement.audit). These tests lock the CURRENT
// behavior of sites the audit deliberately left unchanged, so the deferral is
// checkable and any future change is a conscious decision, not a silent drift.
// ---------------------------------------------------------------------------

// DEFERRAL 1 — pmatch TermComplement (`\`) EXCLUDES flag diacritics.
//
// pmatch's PmatchUnaryOp::TermComplement iterates `get_non_special_alphabet`,
// which drops every `@...@` symbol (flags included, via PmatchAlphabet::
// is_printable). Unlike XRE, the pmatch RUNTIME (pmatch.rs) executes flag
// diacritics as FdOperation constraints rather than matching them as ordinary
// input, so treating a flag as an ordinary sigma member under complement is
// not the pmatch semantics. The upstream C++ pmatch never fixed this, the
// pmatch_utils.md spec is a 1:1 port, and there is no evidence the giellacg
// tokenizer places flags under pmatch complement. Deferred with rationale.
//
// This test locks the observed behavior: `\flag` in pmatch does NOT keep the
// flag in its alphabet (it was never subtracted, so never harmonized in).
#[test]
fn deferral_pmatch_term_complement_excludes_flag() {
    use hfst::pmatch_compiler::PmatchCompiler;
    let _guard = serialized();

    let mut c = PmatchCompiler::<StdVectorFst>::new();
    let defs = c
        .compile(&format!("Define TOP \\\"{FLAG}\" ;\n"))
        .expect("pmatch compile");
    let top = defs.get("TOP").expect("no TOP");
    let sigma = top.get_alphabet().expect("alphabet");

    // Documented deferral: the flag is NOT in sigma (excluded as a special
    // symbol). If this ever flips, the pmatch deferral must be revisited.
    assert!(
        !sigma.contains(FLAG),
        "DEFERRAL: pmatch \\flag currently EXCLUDES the flag from sigma (got {sigma:?})"
    );
}

// DEFERRAL 2 — hfst_xerox_rules restriction/before/after build `[?* - X]`
// contexts WITHOUT flag-ordinary universes.
//
// The replace rules call `Rule::encode_flags()` first (flags become ordinary
// `$...$` symbols before any subtract, so they are already flag-safe). But the
// restriction (`=>`), `before`, and `after` operators build their universe
// straight from `identity_pair()` and subtract a `[?* X ?* Y ?*]` context
// without encoding flags. Flags inside a restriction context are a genuinely
// unusual Xerox construction, these are heavily-spec'd 1:1 ports with their own
// test suite (test_xerox_rules.rs), and upstream C++ behaves identically.
// Deferred with rationale.
//
// This test documents that a plain (flag-free) restriction still compiles and
// behaves, so the deferral does not hide a regression in the common path.
#[test]
fn deferral_xerox_restriction_flag_free_baseline() {
    use hfst::hfst_xerox_rules::restriction;
    let _guard = serialized();

    // a => b _ c : `a` is allowed only between `b` and `c`.
    let center = symbol::<StdVectorFst>("a");
    let left = symbol::<StdVectorFst>("b");
    let right = symbol::<StdVectorFst>("c");
    let context = vec![(left, right)];
    let r = restriction(&center, &context).expect("restriction compiles");

    // "bac" satisfies the restriction; "a" alone does not.
    assert!(
        accepts(&r, &string_net::<StdVectorFst>(&["b", "a", "c"])),
        "restriction must accept 'bac'"
    );
    assert!(
        !accepts(&r, &symbol::<StdVectorFst>("a")),
        "restriction must reject a bare 'a' (no b_c context)"
    );
}
