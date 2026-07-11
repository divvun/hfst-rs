// Regression tests locking four upstream hfst/hfst behaviors that the
// investigation (WBS upstream-bugs.t1) found ALREADY CORRECT in this port.
// Each was reported as a bug against upstream C++ HFST; this port either never
// exhibited it (the rustfst algebra back-end provides the fix as a library
// invariant) or fixed it in a prior commit. These tests pin the good behavior
// so a future refactor cannot silently regress it.
//
//   * hfst#143 — flag-diacritic harmonisation generated spurious flags/arcs
//     ("flag bloat"). Investigation verdict: the rustfst AutoFilter dedupes
//     arcs during composition, so no bloat occurs here. Library-provided —
//     could silently regress if the compose path changed.
//   * hfst#383 — flag-is-epsilon composition semantics. Verdict: correct in
//     BOTH directions via EngineConfig.flag_is_epsilon_in_composition.
//   * hfst#467 — hfst-eliminate-flags -F produced corrupt transducers.
//     Verdict: fixed by cab4fb06 (eliminate_flag composes the flag filter with
//     xerox_composition so foreign flags survive). Round-trips cleanly.
//   * hfst#426 — hfst-regexp2fst compiled weights wrongly (double-counted).
//     Verdict: cosmetic k:k/j:j arcs aside, PATH weights are correct — a k:j
//     mapping weighs 1.2, and two k's give 1.2 per path, never 2.4.
//
// These drive only public library entry points (XreCompiler, HfstTransducer,
// HfstBasicTransducer) with inline fixtures — no CLI, no on-disk files.

use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_data_types::{HfstTwoLevelPaths, StringVector, Symbol};
use hfst::hfst_transducer::{EngineConfig, HfstTransducer};
use hfst::xre::XreCompiler;
use hfst_openfst::StdVectorFst;
use std::collections::BTreeSet;
use std::io::BufReader;

// The tropical transition-data symbol coding lives in process-global statics
// behind Mutexes; cargo runs every #[test] as a parallel thread in ONE process
// where each C++ test was its own process. Serializing through this lock
// restores the one-at-a-time-per-process model. into_inner() recovers from a
// poisoned lock so one failing test does not cascade. (Same pattern as
// test_flag_complement.rs / test_thfst.rs.)
static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

// --- shared helpers -------------------------------------------------------

// Read an inline AT&T string into a basic transducer (epsilon marker matches
// the symbols used in the fixtures below).
fn read_att(att: &str) -> HfstBasicTransducer {
    let mut cursor = BufReader::new(att.as_bytes());
    let mut linecount = 0u32;
    HfstBasicTransducer::read_in_att_format(
        &mut cursor,
        "@_EPSILON_SYMBOL_@",
        &mut linecount,
        false,
    )
    .expect("AT&T fixture parses")
}

// Compile one XRE expression to a tropical transducer via the XreCompiler (the
// same entry point test_flag_complement.rs / test_pmatch.rs use).
fn compile(expr: &str) -> HfstTransducer<StdVectorFst> {
    let mut c = XreCompiler::<StdVectorFst>::new();
    c.compile(expr)
        .unwrap_or_else(|| panic!("XRE compilation of {expr:?} failed"))
}

// A single-symbol acceptor.
fn symbol(sym: &str) -> HfstTransducer<StdVectorFst> {
    HfstTransducer::new_from_symbol(sym).expect("single-symbol transducer")
}

// Concatenate the given symbols into one string acceptor (>=1 symbol).
fn string_net(syms: &[&str]) -> HfstTransducer<StdVectorFst> {
    let (first, rest) = syms.split_first().expect("string_net needs >=1 symbol");
    let mut t = symbol(first);
    for s in rest {
        t.concatenate(&symbol(s), true).expect("concatenate");
    }
    t
}

// Lookup `input` through the transducer's basic form, returning weighted
// two-level paths. `obey_flags` runs the flag-diacritic constraints.
fn lookup(t: &HfstTransducer<StdVectorFst>, input: &[&str], obey_flags: bool) -> HfstTwoLevelPaths {
    let basic = t.to_basic().expect("to_basic");
    let path: StringVector = input.iter().map(|s| Symbol::new(s)).collect();
    let mut results: HfstTwoLevelPaths = BTreeSet::new();
    // Some(0) forbids epsilon cycles (the fixtures are acyclic); -1 = all paths.
    basic.lookup(&path, &mut results, Some(0), None, -1, obey_flags);
    results
}

// The output side of a two-level path, dropping every `@...@` special symbol
// (epsilons AND flag diacritics) so only printable material remains.
fn printable_output(path: &hfst::hfst_data_types::HfstTwoLevelPath) -> String {
    path.second
        .iter()
        .map(|(_, o)| o.as_str())
        .filter(|s| !s.starts_with('@'))
        .collect()
}

// The input side of a two-level path, dropping every `@...@` special symbol.
fn printable_input(path: &hfst::hfst_data_types::HfstTwoLevelPath) -> String {
    path.second
        .iter()
        .map(|(i, _)| i.as_str())
        .filter(|s| !s.starts_with('@'))
        .collect()
}

// The set of printable input strings across all extracted paths.
fn printable_input_set(t: &HfstTransducer<StdVectorFst>) -> BTreeSet<String> {
    let mut paths: HfstTwoLevelPaths = BTreeSet::new();
    t.extract_paths(&mut paths, -1, -1).expect("extract_paths");
    paths.iter().map(printable_input).collect()
}

// ==========================================================================
// hfst#143 — flag-diacritic harmonisation generated spurious flags/arcs.
//
// Upstream `hfst-compose analyser - -F` on divvun's Erzya blew up to 262144
// identical `talo+N+Sg+Nom` paths, each flag arc duplicated 8-9 times. This
// port never bloats: the rustfst AutoFilter dedupes arcs during composition,
// so the -F flag-harmonised compose yields exactly ONE path with no duplicate
// arcs. This is a LIBRARY-PROVIDED invariant (the back-end's filter), so it
// could regress silently if the compose path is rewritten — hence pinned here.
//
// Fixture: an analyser mapping surface `talo` to `talo` with two output-side
// flags (a minimal stand-in for `+Sg`/`+Nom` epsilon-output flag arcs), and a
// surface acceptor `talo`. We mirror exactly what `hfst-compose -F` does:
// harmonize_flag_diacritics(second, true) then compose (see hfst-cli
// tools/compose.rs ComposeOp).
// ==========================================================================
#[test]
fn hfst_143_flag_harmonise_no_bloat() {
    let _guard = serialized();

    // t:t a:a l:l o:o then two output-epsilon flag arcs (as in the issue's
    // fst2txt dump: `@U.DECL-NX.SG@ -> @0@`, `@U.DECL-CX.NOM@ -> @0@`).
    let analyser: HfstTransducer<StdVectorFst> = HfstTransducer::from_basic(&read_att(concat!(
        "0\t1\tt\tt\t0.000000\n",
        "1\t2\ta\ta\t0.000000\n",
        "2\t3\tl\tl\t0.000000\n",
        "3\t4\to\to\t0.000000\n",
        "4\t5\t@U.DECL-NX.SG@\t@_EPSILON_SYMBOL_@\t0.000000\n",
        "5\t6\t@U.DECL-CX.NOM@\t@_EPSILON_SYMBOL_@\t0.000000\n",
        "6\t0.000000\n",
    )));
    assert!(
        analyser.has_flag_diacritics(),
        "fixture analyser must carry flag diacritics"
    );

    // Surface acceptor 'talo'.
    let surface = string_net(&["t", "a", "l", "o"]);

    // Mirror `hfst-compose -F`: harmonize flags into both, then compose.
    let mut first = analyser.clone();
    let mut second = surface.clone();
    first
        .harmonize_flag_diacritics(&mut second, true)
        .expect("flag harmonisation (-F)");
    first
        .compose_with_config(&second, true, &EngineConfig::default())
        .expect("compose");

    // Exactly ONE path in the result (upstream bloated to 262144).
    let mut paths: HfstTwoLevelPaths = BTreeSet::new();
    first
        .extract_paths(&mut paths, -1, -1)
        .expect("extract_paths");
    assert_eq!(
        paths.len(),
        1,
        "flag-harmonised compose must yield exactly one path, got {}",
        paths.len()
    );

    // The single path maps the surface `talo` to `talo` (flags/epsilons erased).
    let only = paths.iter().next().expect("one path");
    assert_eq!(printable_input(only), "talo", "input side must be 'talo'");
    assert_eq!(printable_output(only), "talo", "output side must be 'talo'");

    // No duplicate arcs: every (src, in, out, tgt, weight) tuple is unique.
    // Upstream duplicated each flag arc 8-9 times; the AutoFilter must not.
    let basic = first.to_basic().expect("to_basic");
    let coder = basic.coder();
    let mut arcs: Vec<(u32, String, String, u32, u32)> = Vec::new();
    for (src, transitions) in basic.states_and_transitions().iter().enumerate() {
        for tr in transitions.iter() {
            let data = tr.get_transition_data();
            arcs.push((
                src as u32,
                data.get_input_symbol(coder).to_string(),
                data.get_output_symbol(coder).to_string(),
                tr.get_target_state(),
                // Weights are all 0.0 here; bit-quantise so the tuple is Ord.
                tr.get_weight().to_bits(),
            ));
        }
    }
    let unique: BTreeSet<_> = arcs.iter().cloned().collect();
    assert_eq!(
        arcs.len(),
        unique.len(),
        "no duplicate arcs allowed (flag bloat): {} total vs {} unique",
        arcs.len(),
        unique.len()
    );
}

// ==========================================================================
// hfst#383 — flag-is-epsilon composition semantics.
//
// Composing a flag-bearing acceptor against a flag-free sigma-star should
// DROP the flag path by default (flags are not part of the flag-free
// alphabet, so they cannot match) but KEEP it when flags are treated as
// epsilons. Both directions are correct here via
// EngineConfig.flag_is_epsilon_in_composition, threaded through
// compose_with_config. Pinned in both directions.
//
// Fixture: acceptor {'bar' with a flag after 'b' (b @U.X.Y@ a r), 'foo'} and
// a sigma-star built from the letters only (b,a,r,f,o) — deliberately NO `?`,
// so it does not silently swallow the flag as an unknown.
// ==========================================================================
#[test]
fn hfst_383_flag_is_epsilon_both_directions() {
    let _guard = serialized();

    // Flag-bearing path: b @U.X.Y@ a r  (printable input 'bar').
    let flag_path = string_net(&["b", "@U.X.Y@", "a", "r"]);
    // Flag-free path: foo.
    let foo = string_net(&["f", "o", "o"]);
    let mut acceptor = flag_path.clone();
    acceptor.disjunct(&foo, true).expect("disjunct");

    // Flag-free sigma-star over the plain letters only (no `?`).
    let letters = ["b", "a", "r", "f", "o"];
    let mut sigma = symbol(letters[0]);
    for l in &letters[1..] {
        sigma.disjunct(&symbol(l), true).expect("disjunct");
    }
    sigma.repeat_star().expect("repeat_star");
    assert!(!sigma.has_flag_diacritics(), "sigma-star must be flag-free");

    // Default (flag_is_epsilon_in_composition = false): the flag path is
    // DROPPED (the flag has no counterpart in sigma), only 'foo' survives.
    let mut result_default = acceptor.clone();
    result_default
        .compose_with_config(&sigma, true, &EngineConfig::default())
        .expect("compose default");
    let surviving_default = printable_input_set(&result_default);
    assert_eq!(
        surviving_default,
        BTreeSet::from(["foo".to_string()]),
        "default compose must keep only the flag-free 'foo', got {surviving_default:?}"
    );

    // flag_is_epsilon_in_composition = true: the flag is treated as epsilon,
    // so BOTH 'bar' and 'foo' survive.
    let mut result_epsilon = acceptor.clone();
    let cfg = EngineConfig {
        flag_is_epsilon_in_composition: true,
        ..EngineConfig::default()
    };
    result_epsilon
        .compose_with_config(&sigma, true, &cfg)
        .expect("compose flag-is-epsilon");
    let surviving_epsilon = printable_input_set(&result_epsilon);
    assert_eq!(
        surviving_epsilon,
        BTreeSet::from(["bar".to_string(), "foo".to_string()]),
        "flag-is-epsilon compose must keep both 'bar' and 'foo', got {surviving_epsilon:?}"
    );
}

// ==========================================================================
// hfst#467 — hfst-eliminate-flags -F produced corrupt transducers.
//
// The issue's own AT&T example (below) maps `xX -> Yy` guarded by P/U/R/C
// flags across three features D, F, G. Upstream `hfst-eliminate-flags -F <F>`
// corrupted the net for any single feature. Fixed by cab4fb06: eliminate_flag
// composes the symbol-level flag filter with xerox_composition so foreign
// flags survive harmonisation. Verdict: correct — every single-feature
// elimination round-trips to a structurally sane net that still yields Yy, and
// sequential F∘G∘D equals the full eliminate_flags.
// ==========================================================================
#[test]
fn hfst_467_eliminate_flag_no_corruption() {
    let _guard = serialized();

    // The exact AT&T from the issue (gh issue 467 comment): xX:Yy guarded by
    // P.D/U.F/U.G/R.D/C.F/C.G flags.
    let att = concat!(
        "0\t1\t@P.D.B@\t@P.D.B@\t0.000000\n",
        "1\t2\t@U.F.B@\t@U.F.B@\t0.000000\n",
        "2\t3\t@U.G.B@\t@U.G.B@\t0.000000\n",
        "3\t4\tx\tY\t0.000000\n",
        "4\t5\t@R.D.B@\t@R.D.B@\t0.000000\n",
        "5\t6\t@P.D.C@\t@P.D.C@\t0.000000\n",
        "6\t7\t@U.F.B@\t@U.F.B@\t0.000000\n",
        "7\t8\t@U.G.B@\t@U.G.B@\t0.000000\n",
        "8\t9\tX\ty\t0.000000\n",
        "9\t10\t@R.D.C@\t@R.D.C@\t0.000000\n",
        "10\t11\t@C.F@\t@C.F@\t0.000000\n",
        "11\t12\t@C.G@\t@C.G@\t0.000000\n",
        "12\t0.000000\n",
    );
    let base: HfstTransducer<StdVectorFst> = HfstTransducer::from_basic(&read_att(att));

    // Sanity: obeying the flags, `xX` maps to `Yy`.
    let base_paths = lookup(&base, &["x", "X"], true);
    assert_eq!(
        base_paths.len(),
        1,
        "base xX must have exactly one flag path"
    );
    assert_eq!(
        printable_output(base_paths.iter().next().expect("path")),
        "Yy",
        "base xX must map to Yy under flag constraints"
    );

    // Eliminate each single feature: the result must be structurally valid
    // (round-trip basic->transducer, sane state/arc counts, no panic) and
    // still map `xX -> Yy` (now flag-free, so obey_flags is irrelevant).
    for feature in ["D", "F", "G"] {
        let mut eliminated = base.clone();
        eliminated
            .eliminate_flag(feature)
            .unwrap_or_else(|e| panic!("eliminate_flag({feature}) failed: {e}"));

        // Structurally valid: round-trips to basic and back without panic, and
        // has a sane (non-degenerate, non-explosive) shape.
        let round_trip: HfstTransducer<StdVectorFst> =
            HfstTransducer::from_basic(&eliminated.to_basic().expect("to_basic"));
        assert!(
            round_trip
                .compare_default(&eliminated)
                .expect("compare_default"),
            "eliminate_flag({feature}) result must survive a basic round-trip"
        );
        let states = eliminated.number_of_states();
        let arcs = eliminated.number_of_arcs();
        assert!(
            (1..=32).contains(&states) && (1..=32).contains(&arcs),
            "eliminate_flag({feature}) shape must be sane, got {states} states / {arcs} arcs"
        );

        // The mapping is preserved.
        let paths = lookup(&eliminated, &["x", "X"], false);
        assert_eq!(
            paths.len(),
            1,
            "eliminate_flag({feature}): xX must still have exactly one path"
        );
        assert_eq!(
            printable_output(paths.iter().next().expect("path")),
            "Yy",
            "eliminate_flag({feature}): xX must still map to Yy"
        );
    }

    // Sequential single-feature elimination F -> G -> D equals the one-shot
    // eliminate_flags (the composite path must not corrupt either).
    let mut sequential = base.clone();
    sequential.eliminate_flag("F").expect("eliminate F");
    sequential.eliminate_flag("G").expect("eliminate G");
    sequential.eliminate_flag("D").expect("eliminate D");

    let mut full = base.clone();
    full.eliminate_flags().expect("eliminate_flags");

    assert!(
        sequential.compare_default(&full).expect("compare_default"),
        "sequential F∘G∘D elimination must equal the full eliminate_flags"
    );
}

// ==========================================================================
// hfst#426 — hfst-regexp2fst compiled weights wrongly.
//
// Cosmetic identity/self arcs aside (redundant k:k, j:j — a determinisation
// artefact the reporter also flagged), the DECISIVE contract is that PATH
// weights never double-count. For `?* k:j::1.2 ?*` the single k:j mapping
// weighs 1.2, and a string with a k plus a spectator k still weighs 1.2 per
// path — NOT 2.4. Verdict: path weights are correct here. Pinned via lookup.
// ==========================================================================
#[test]
fn hfst_426_xre_weight_totals() {
    let _guard = serialized();

    let t = compile("?* k:j::1.2 ?*");

    // k -> j, single path, weight 1.2.
    let k = lookup(&t, &["k"], false);
    assert_eq!(k.len(), 1, "k must have exactly one path");
    let k_path = k.iter().next().expect("k path");
    assert_eq!(printable_output(k_path), "j", "k must map to j");
    assert!(
        (k_path.first - 1.2).abs() < 1e-4,
        "k->j weight must be 1.2, got {}",
        k_path.first
    );

    // jk -> jj, single path, weight 1.2 (the k:j fires once; the leading j is a
    // spectator identity). The decisive double-count check: must NOT be 2.4.
    let jk = lookup(&t, &["j", "k"], false);
    assert_eq!(jk.len(), 1, "jk must have exactly one path");
    let jk_path = jk.iter().next().expect("jk path");
    assert_eq!(printable_output(jk_path), "jj", "jk must map to jj");
    assert!(
        (jk_path.first - 1.2).abs() < 1e-4,
        "jk->jj weight must be 1.2 (NOT 2.4 — double-count check), got {}",
        jk_path.first
    );

    // kk -> {jk, kj}, two paths (the k:j fires on either k), each weight 1.2.
    let kk = lookup(&t, &["k", "k"], false);
    assert_eq!(kk.len(), 2, "kk must have exactly two paths");
    let outputs: BTreeSet<String> = kk.iter().map(printable_output).collect();
    assert_eq!(
        outputs,
        BTreeSet::from(["jk".to_string(), "kj".to_string()]),
        "kk must map to {{jk, kj}}, got {outputs:?}"
    );
    for path in &kk {
        assert!(
            (path.first - 1.2).abs() < 1e-4,
            "each kk path weight must be 1.2, got {}",
            path.first
        );
    }

    // Bare `k:j::1.2`: a single mapping with weight 1.2.
    let bare = compile("k:j::1.2");
    let bare_paths = lookup(&bare, &["k"], false);
    assert_eq!(bare_paths.len(), 1, "bare k:j must have one path");
    let bare_path = bare_paths.iter().next().expect("bare path");
    assert_eq!(printable_output(bare_path), "j", "bare k must map to j");
    assert!(
        (bare_path.first - 1.2).abs() < 1e-4,
        "bare k:j weight must be 1.2, got {}",
        bare_path.first
    );
}
