//! Integration tests for the native foma backend's queries: lookup, path
//! extraction, ambiguity, state and arc counts, weights, input symbols and the
//! alphabet, each checked against an openfst-family backend. Like
//! foma_backend.rs, this file is gated on the `foma` feature and reaches the
//! backend through hfst's public surface only.
#![cfg(feature = "foma")]

mod foma_backend_common;

use std::collections::BTreeSet;

use foma_backend_common::{
    EPSILON, IDENTITY, UNKNOWN, arc_count, basic_acceptor, basic_arcs, basic_pair,
    basic_sigma_star, foma_of, serialized, state_count, sym, tropical_of,
};
use hfst::backend::{AlgebraBackend, Backend, LookupBackend};
use hfst::backend_foma::FomaTransducer;
use hfst::backend_thfst::ThfstTransducer;
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_data_types::HfstTwoLevelPath;
use hfst::hfst_extract_strings::{ExtractStringsCb, RetVal};
use hfst::hfst_symbol_defs::StringSet;
use hfst::transducer::{Transducer, WeightedTables};
use hfst::xfst_compiler::XfstCompiler;
use hfst_openfst::StdVectorFst;

// ---------------------------------------------------------------------------
// Test 4: lookup parity vs the optimized-lookup (openfst-family) backend.
// ---------------------------------------------------------------------------

/// The set of output words `lookup(input)` yields, as concatenated strings.
fn lookup_outputs(paths: &hfst::hfst_data_types::HfstOneLevelPaths) -> BTreeSet<String> {
    paths
        .iter()
        .map(|p| p.second.iter().map(|s| s.as_str()).collect::<String>())
        .collect()
}

// [spec:hfst:sem:foma-backend.lookup-impl/test]
#[test]
fn lookup_parity_vs_optimized_lookup() {
    let _g = serialized();

    // cat:dog and the nondeterministic a:b | a:c.
    let cat_dog = basic_pair("cat", "dog");
    let mut a_bc = basic_pair("a", "b");
    let tr = HfstBasicTransition::new_symbols(2, sym("a"), sym("c"), 0.0, a_bc.coder_mut());
    a_bc.add_transition(0, &tr, true);
    a_bc.set_final_weight(2, &0.0); // 0 -a:b-> 1(final), 0 -a:c-> 2(final)

    for (net, input, expected) in [(&cat_dog, "cat", vec!["dog"]), (&a_bc, "a", vec!["b", "c"])] {
        let mut foma = foma_of(net);
        // The openfst-family lookup path is the optimized-lookup backend, which
        // is how hfst looks up an openfst transducer (Backend::from_basic builds
        // the weighted-shaped OL tables).
        let ol: Transducer<WeightedTables> =
            <Transducer<WeightedTables> as Backend>::from_basic(net).expect("OL from_basic");

        let foma_out = lookup_outputs(&foma.lookup_fd_str(input, -1, 0.0));
        let ol_out = lookup_outputs(&ol.lookup_fd_str(input, -1, 0.0));
        let want: BTreeSet<String> = expected.iter().map(|s| s.to_string()).collect();

        assert_eq!(foma_out, want, "foma lookup({input})");
        assert_eq!(ol_out, want, "OL lookup({input})");
        assert_eq!(foma_out, ol_out, "foma/OL lookup parity for {input}");
    }

    // Unknown input yields the empty set in both backends.
    let mut foma = foma_of(&cat_dog);
    let ol: Transducer<WeightedTables> =
        <Transducer<WeightedTables> as Backend>::from_basic(&cat_dog).expect("OL from_basic");
    assert!(
        foma.lookup_fd_str("zzz", -1, 0.0).is_empty(),
        "foma lookup of unknown input is empty"
    );
    assert!(
        ol.lookup_fd_str("zzz", -1, 0.0).is_empty(),
        "OL lookup of unknown input is empty"
    );
}

// ---------------------------------------------------------------------------
// Test 5: path extraction parity vs the tropical openfst backend.
// ---------------------------------------------------------------------------

/// A transducer over explicit symbol vectors, so paths can carry multichar
/// symbols (epsilons, flag diacritics) that `basic_pair`'s per-char split
/// cannot express.
fn basic_symbols(inp: &[&str], outp: &[&str]) -> HfstBasicTransducer {
    assert_eq!(inp.len(), outp.len(), "basic_symbols needs aligned columns");
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    for i in 0..inp.len() {
        let tr = HfstBasicTransition::new_symbols(
            (i + 1) as u32,
            sym(inp[i]),
            sym(outp[i]),
            0.0,
            net.coder_mut(),
        );
        net.add_transition(i as u32, &tr, true);
    }
    net.set_final_weight(inp.len() as u32, &0.0);
    net
}

/// Every callback invocation of a path extraction, as `(is_final, columns)`.
/// This is the whole observable contract: the per-symbol column vector, not
/// just the concatenated words.
type Trace = Vec<(bool, Vec<(String, String)>)>;

struct TraceCb {
    trace: Trace,
    cap: usize,
}

impl ExtractStringsCb for TraceCb {
    fn operator_call(&mut self, path: &mut HfstTwoLevelPath, is_final: bool) -> RetVal {
        self.trace.push((
            is_final,
            path.second
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect(),
        ));
        RetVal::new(self.trace.len() < self.cap, true)
    }
}

/// The extraction trace, sorted so backends that visit arcs in a different
/// order still compare equal (multiplicity preserved).
fn trace_of<B: Backend>(b: &B, cycles: i32, filter_fd: Option<bool>) -> Trace {
    let mut cb = TraceCb {
        trace: Vec::new(),
        cap: 4096,
    };
    match filter_fd {
        None => b.extract_paths_cb(&mut cb, cycles),
        Some(f) => b.extract_paths_fd_cb(&mut cb, cycles, f),
    }
    cb.trace.sort();
    cb.trace
}

/// foma used to hand the callback ONE `StringPair` holding the whole input word
/// and the whole output word, so `HfstTwoLevelPath::second.len()` was always 1.
/// That made `print longest-string-size` report 1 for every net and broke
/// `fst2strings --xfst=print-pairs` / `--xfst=print-space`, which read the
/// per-symbol columns.
// [spec:hfst:sem:foma-backend.lookup-impl/test]
#[test]
fn extract_paths_columns_match_tropical() {
    let _g = serialized();

    let cases: [(&str, HfstBasicTransducer, usize); 3] = [
        (
            "acceptor abc",
            basic_symbols(&["a", "b", "c"], &["a", "b", "c"]),
            3,
        ),
        (
            "relation a:b c:d e",
            basic_symbols(&["a", "c", "e"], &["b", "d", "e"]),
            3,
        ),
        (
            "epsilon output",
            basic_symbols(&["x", "y"], &["x", EPSILON]),
            2,
        ),
    ];

    for (name, net, columns) in cases {
        let foma = trace_of(&foma_of(&net), -1, None);
        let tropical = trace_of(&tropical_of(&net), -1, None);
        assert_eq!(foma, tropical, "extract_paths trace parity for {name}");

        // The defect's signature: a single whole-word pair per path.
        let longest = foma
            .iter()
            .filter(|(is_final, _)| *is_final)
            .map(|(_, cols)| cols.len())
            .max()
            .expect("every case has a final path");
        assert_eq!(longest, columns, "{name}: one column per symbol");
    }
}

/// A final start state must still report the empty path, as the openfst
/// backends do (`regex [a|0]` reaches this).
// [spec:hfst:sem:foma-backend.lookup-impl/test]
#[test]
fn extract_paths_reports_the_empty_path_like_tropical() {
    let _g = serialized();
    let mut net = basic_symbols(&["a"], &["a"]);
    net.set_final_weight(0, &0.0);

    assert_eq!(
        trace_of(&foma_of(&net), -1, None),
        trace_of(&tropical_of(&net), -1, None),
        "empty-path reporting parity"
    );
}

/// `cycles` bounds the traversal per state, rather than being approximated by
/// a cap on the number of paths produced.
// [spec:hfst:sem:foma-backend.lookup-impl/test]
#[test]
fn extract_paths_cycle_bound_matches_tropical() {
    let _g = serialized();
    let net = basic_sigma_star(&["a", "b"]);

    for cycles in [0, 1, 2] {
        assert_eq!(
            trace_of(&foma_of(&net), cycles, None),
            trace_of(&tropical_of(&net), cycles, None),
            "cycles={cycles} traversal parity on a cyclic net"
        );
    }
}

/// `extract_paths_fd_cb` used to ignore `filter_fd` entirely, so flag
/// diacritics never appeared in an extracted path on a foma transducer —
/// `fst2strings --xfst=print-flags` printed nothing where the openfst backends
/// printed the flags.
// [spec:hfst:sem:foma-backend.lookup-impl/test]
#[test]
fn extract_paths_fd_honours_filter_fd_like_tropical() {
    let _g = serialized();
    let net = basic_symbols(&["@U.N.SG@", "a", "b"], &["@U.N.SG@", "a", "b"]);

    let shown = trace_of(&foma_of(&net), -1, Some(false));
    let hidden = trace_of(&foma_of(&net), -1, Some(true));

    assert_eq!(
        shown,
        trace_of(&tropical_of(&net), -1, Some(false)),
        "filter_fd=false (print flags) parity"
    );
    assert_eq!(
        hidden,
        trace_of(&tropical_of(&net), -1, Some(true)),
        "filter_fd=true (filter flags) parity"
    );
    assert_ne!(shown, hidden, "filter_fd made no difference");

    let flagged = |t: &Trace| {
        t.iter()
            .any(|(_, cols)| cols.iter().any(|(a, _)| a == "@U.N.SG@"))
    };
    assert!(flagged(&shown), "filter_fd=false must show the flag");
    assert!(!flagged(&hidden), "filter_fd=true must hide the flag");
}

// ---------------------------------------------------------------------------
// Test 6: infinite ambiguity is about input-epsilon cycles, not cyclicity.
// ---------------------------------------------------------------------------

/// foma used to answer whole-net `is_cyclic()`, which reports every cyclic net
/// as infinitely ambiguous — `a*` is cyclic but reads one input symbol per arc,
/// so it is finitely ambiguous. Only a cycle that consumes no input (an input
/// epsilon or a flag diacritic) is.
// [spec:hfst:sem:foma-backend.lookup-impl/test]
#[test]
fn is_infinitely_ambiguous_needs_an_input_epsilon_cycle() {
    let _g = serialized();

    // a* — cyclic, finitely ambiguous.
    let consuming = basic_sigma_star(&["a"]);
    // (0:a)* — an input-epsilon cycle, infinitely ambiguous.
    let mut epsilon_loop = HfstBasicTransducer::new();
    epsilon_loop.add_state(0);
    let tr =
        HfstBasicTransition::new_symbols(0, sym(EPSILON), sym("a"), 0.0, epsilon_loop.coder_mut());
    epsilon_loop.add_transition(0, &tr, true);
    epsilon_loop.set_final_weight(0, &0.0);

    for (name, net, want) in [("a*", consuming, false), ("(0:a)*", epsilon_loop, true)] {
        let foma = foma_of(&net);
        assert!(foma.is_cyclic(), "{name} is cyclic either way");
        assert_eq!(
            foma.is_infinitely_ambiguous().expect("foma ambiguity"),
            want,
            "foma is_infinitely_ambiguous({name})"
        );
        assert_eq!(
            foma.is_infinitely_ambiguous().expect("foma ambiguity"),
            tropical_of(&net)
                .is_infinitely_ambiguous()
                .expect("tropical ambiguity"),
            "is_infinitely_ambiguous parity for {name}"
        );
    }
}

/// The lookup-time question is about the input, not the whole net: the answer
/// used to be whole-net cyclicity, so every input got the same answer.
// [spec:hfst:sem:foma-backend.lookup-impl/test]
#[test]
fn is_lookup_infinitely_ambiguous_depends_on_the_input() {
    let _g = serialized();

    // 0 -a:a-> 1 (final, with a 0:x self-loop); 0 -b:b-> 2 (final).
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    for (target, i, o) in [(1u32, "a", "a"), (2, "b", "b")] {
        let tr = HfstBasicTransition::new_symbols(target, sym(i), sym(o), 0.0, net.coder_mut());
        net.add_transition(0, &tr, true);
    }
    let loop_tr = HfstBasicTransition::new_symbols(1, sym(EPSILON), sym("x"), 0.0, net.coder_mut());
    net.add_transition(1, &loop_tr, true);
    net.set_final_weight(1, &0.0);
    net.set_final_weight(2, &0.0);

    let mut foma = foma_of(&net);
    let ol: Transducer<WeightedTables> =
        <Transducer<WeightedTables> as Backend>::from_basic(&net).expect("OL from_basic");

    for (input, want) in [("a", true), ("b", false)] {
        let sv = vec![sym(input)];
        assert_eq!(
            foma.is_lookup_infinitely_ambiguous_strvec(&sv),
            want,
            "foma is_lookup_infinitely_ambiguous({input})"
        );
        assert_eq!(
            foma.is_lookup_infinitely_ambiguous_strvec(&sv),
            ol.is_lookup_infinitely_ambiguous_strvec(&sv),
            "foma/OL lookup-ambiguity parity for {input}"
        );
        assert_eq!(
            foma.is_lookup_infinitely_ambiguous_str(input),
            want,
            "foma is_lookup_infinitely_ambiguous_str({input})"
        );
    }
}

// ---------------------------------------------------------------------------
// State/arc counts. `Backend::number_of_states` / `number_of_arcs` used to be
// defaulted to 0, and only the tropical backend overrode them — so `hfst xfst
// -f foma` printed "0 states, 0 arcs" for every net it built, a stub value the
// caller printed as fact. The C++ FomaTransducer::number_of_states/_arcs do
// exist (FomaTransducer.cc), so this was a port gap, not a foma limitation.
// ---------------------------------------------------------------------------

/// Both counts, from the backend itself and from its interchange form.
fn counts<B: Backend>(b: &B) -> ((u32, u32), (usize, usize)) {
    (
        (b.number_of_states(), b.number_of_arcs()),
        (state_count(b), arc_count(b)),
    )
}

fn count_cases() -> Vec<(&'static str, HfstBasicTransducer)> {
    vec![
        ("cat", basic_acceptor("cat")),
        ("a:b", basic_pair("a", "b")),
        ("abc:xyz", basic_pair("abc", "xyz")),
        ("{a,b,c}*", basic_sigma_star(&["a", "b", "c"])),
    ]
}

#[test]
fn foma_matches_tropical_state_and_arc_counts() {
    let _g = serialized();

    for (name, net) in count_cases() {
        let foma = foma_of(&net);
        let tropical = tropical_of(&net);
        assert_eq!(
            (foma.number_of_states(), foma.number_of_arcs()),
            (tropical.number_of_states(), tropical.number_of_arcs()),
            "foma/tropical count parity for {name}"
        );
        assert!(
            foma.number_of_states() > 0 && foma.number_of_arcs() > 0,
            "{name} is a non-trivial net, so neither foma count may be 0"
        );
    }
}

#[test]
fn foma_counts_agree_with_its_own_graph() {
    let _g = serialized();

    for (name, net) in count_cases() {
        let (reported, witness) = counts(&foma_of(&net));
        assert_eq!(
            reported,
            (witness.0 as u32, witness.1 as u32),
            "foma counts disagree with its interchange graph for {name}"
        );
    }
}

#[test]
fn optimized_lookup_and_thfst_report_real_counts() {
    let _g = serialized();

    for (name, net) in count_cases() {
        let ol: Transducer<WeightedTables> =
            <Transducer<WeightedTables> as Backend>::from_basic(&net).expect("OL from_basic");
        let (reported, witness) = counts(&ol);
        assert_eq!(
            reported,
            (witness.0 as u32, witness.1 as u32),
            "OL counts disagree with its interchange graph for {name}"
        );
        assert!(
            reported.0 > 0 && reported.1 > 0,
            "{name} is a non-trivial net, so neither OL count may be 0"
        );

        // THFST is the same engine under a different stream identity, so it must
        // report the same counts rather than fall back to a stub.
        let thfst = ThfstTransducer::from(ol);
        assert_eq!(
            (thfst.number_of_states(), thfst.number_of_arcs()),
            reported,
            "THFST count parity with its inner OL engine for {name}"
        );
    }
}

/// The observable defect: the net-size line `hfst xfst -f foma` prints after
/// every command reads these counts straight off the backend.
#[test]
fn xfst_net_size_under_foma_is_nonzero() {
    let _g = serialized();

    let script = "regex [a:b | c:d | e:f];\n";

    let mut foma_c = XfstCompiler::<FomaTransducer>::new();
    foma_c.parse(script);
    let foma_top = *foma_c.get_stack().last().expect("foma stack non-empty");
    let foma_size = (
        foma_c.net(foma_top).number_of_states(),
        foma_c.net(foma_top).number_of_arcs(),
    );

    let mut trop_c = XfstCompiler::<StdVectorFst>::new();
    trop_c.parse(script);
    let trop_top = *trop_c.get_stack().last().expect("tropical stack non-empty");
    let trop_size = (
        trop_c.net(trop_top).number_of_states(),
        trop_c.net(trop_top).number_of_arcs(),
    );

    assert_eq!(foma_size, (2, 3), "three alternations over two states");
    assert_eq!(foma_size, trop_size, "xfst net size is backend-independent");
}

// ---------------------------------------------------------------------------
// has_weights. Same silent-stub shape as the counts: the trait defaulted it to
// false and only tropical overrode it, so every OL/OLW and THFST transducer
// answered false regardless of what it carried.
// ---------------------------------------------------------------------------

/// `a:b` carrying `arc_w` on its single arc and `final_w` on its final state.
fn basic_weighted(arc_w: f32, final_w: f32) -> HfstBasicTransducer {
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    let tr = HfstBasicTransition::new_symbols(1, sym("a"), sym("b"), arc_w, net.coder_mut());
    net.add_transition(0, &tr, true);
    net.set_final_weight(1, &final_w);
    net
}

/// The three cases that separate "carries a weight" from "is weighted-shaped".
fn weight_cases() -> Vec<(&'static str, HfstBasicTransducer, bool)> {
    vec![
        ("all-zero", basic_weighted(0.0, 0.0), false),
        ("weighted arc", basic_weighted(0.5, 0.0), true),
        ("weighted final", basic_weighted(0.0, 0.5), true),
    ]
}

#[test]
fn has_weights_reports_carried_weights_not_table_shape() {
    let _g = serialized();

    for (name, net, want) in weight_cases() {
        let ol: Transducer<WeightedTables> =
            <Transducer<WeightedTables> as Backend>::from_basic(&net).expect("OL from_basic");

        // Conversions always build weighted-SHAPED tables, so the header flag
        // `stream_type` reads is true even for the all-zero net. `has_weights`
        // deliberately answers the other question, and tropical is the reference
        // for what that question means.
        assert!(
            ol.is_weighted(),
            "{name}: conversions produce weighted-shaped tables"
        );
        assert_eq!(ol.has_weights(), want, "OL has_weights({name})");
        assert_eq!(
            ol.has_weights(),
            tropical_of(&net).has_weights(),
            "OL/tropical has_weights parity for {name}"
        );
    }
}

#[test]
fn foma_and_thfst_report_weights_honestly() {
    let _g = serialized();

    for (name, net, want) in weight_cases() {
        assert!(
            !foma_of(&net).has_weights(),
            "{name}: foma nets have no weight field to carry a weight in"
        );

        let ol: Transducer<WeightedTables> =
            <Transducer<WeightedTables> as Backend>::from_basic(&net).expect("OL from_basic");
        let thfst = ThfstTransducer::from(ol);
        assert_eq!(thfst.has_weights(), want, "THFST has_weights({name})");
    }
}

// ---------------------------------------------------------------------------
// get_initial_input_symbols vs get_first_input_symbols.
//
// The has_weights shape again, one layer subtler: foma answered BOTH from one
// helper that read the start state's out-arcs, so the wrong answer was never
// empty and still varied plausibly with the net — nothing an assertion about
// shape alone can catch. The two are DIFFERENT walks in the contract tropical
// sets, so the test has to be that they DISAGREE where the contract says they
// must, and that each separately agrees with tropical on the same net.
// ---------------------------------------------------------------------------

fn syms(items: &[&str]) -> StringSet {
    items.iter().copied().map(sym).collect()
}

// [spec:hfst:sem:foma-backend.algebra-impl/test]
#[test]
fn initial_and_first_input_symbols_are_different_walks() {
    let _g = serialized();

    // `abc`: initial is the one symbol a path can start with; first is every
    // symbol in the net. Answering both from the start state's out-arcs gives
    // {a} twice — non-empty, correct for `initial`, and wrong for `first`.
    let abc = basic_acceptor("abc");
    let f = foma_of(&abc);
    let t = tropical_of(&abc);

    assert_eq!(f.get_initial_input_symbols(), syms(&["a"]));
    assert_eq!(f.get_first_input_symbols(), syms(&["a", "b", "c"]));
    assert_ne!(
        f.get_initial_input_symbols(),
        f.get_first_input_symbols(),
        "the two walks must disagree on a net longer than one symbol"
    );
    assert_eq!(
        f.get_initial_input_symbols(),
        t.get_initial_input_symbols(),
        "initial-symbol parity with tropical"
    );
    assert_eq!(
        f.get_first_input_symbols(),
        t.get_first_input_symbols(),
        "first-symbol parity with tropical"
    );
}

// [spec:hfst:sem:foma-backend.algebra-impl/test]
#[test]
fn initial_input_symbols_descend_through_epsilon_and_flags() {
    let _g = serialized();

    // Reading the start state's out-arcs literally answers `@_EPSILON_SYMBOL_@`
    // / `@U.F.A@` here — a symbol no path can begin with. `@_UNKNOWN_@` is the
    // control: it is a reserved sigma number too, but it is not epsilon and not
    // a flag, so both walks report it as the real symbol it is.
    let eps = basic_arcs(
        &[(0, EPSILON, EPSILON, 1), (1, "a", "a", 2), (2, "b", "b", 3)],
        &[3],
    );
    let flag = basic_arcs(
        &[
            (0, "@U.F.A@", "@U.F.A@", 1),
            (1, "a", "a", 2),
            (2, "b", "b", 3),
        ],
        &[3],
    );
    let unk = basic_arcs(&[(0, UNKNOWN, IDENTITY, 1), (1, "b", "b", 2)], &[2]);
    // Two branches off the start state, one of them behind an epsilon: both
    // first symbols are initial, and the walk must not stop at the first branch.
    let branch = basic_arcs(
        &[
            (0, "a", "a", 1),
            (1, "b", "b", 2),
            (0, EPSILON, EPSILON, 3),
            (3, "c", "c", 4),
        ],
        &[2, 4],
    );

    let cases: [(&str, &HfstBasicTransducer, &[&str], &[&str]); 4] = [
        ("epsilon prefix", &eps, &["a"], &["a", "b"]),
        ("flag prefix", &flag, &["a"], &["a", "b"]),
        ("unknown arc", &unk, &[UNKNOWN], &[UNKNOWN, "b"]),
        ("epsilon branch", &branch, &["a", "c"], &["a", "b", "c"]),
    ];

    for (name, net, initial, first) in cases {
        let f = foma_of(net);
        let t = tropical_of(net);
        assert_eq!(
            f.get_initial_input_symbols(),
            syms(initial),
            "{name}: foma initial"
        );
        assert_eq!(
            f.get_first_input_symbols(),
            syms(first),
            "{name}: foma first"
        );
        assert_eq!(
            f.get_initial_input_symbols(),
            t.get_initial_input_symbols(),
            "{name}: initial parity with tropical"
        );
        assert_eq!(
            f.get_first_input_symbols(),
            t.get_first_input_symbols(),
            "{name}: first parity with tropical"
        );
    }

    // An empty net has no start state to walk from; both answer the empty set
    // rather than panicking.
    let empty = HfstBasicTransducer::new();
    assert!(foma_of(&empty).get_initial_input_symbols().is_empty());
    assert!(foma_of(&empty).get_first_input_symbols().is_empty());
}

/// An alphabet always contains the three special symbols.
///
/// Foma tracks them as reserved sigma NUMBERS rather than sigma entries, so a
/// sigma walk alone under-reports the alphabet by exactly those three. Callers
/// that build one arc per alphabet member then construct a smaller relation on
/// foma than on any other backend — `affix_guessify` lost its epsilon and
/// unknown guess arcs that way.
// [spec:hfst:sem:foma-backend.backend-impl/test]
#[test]
fn foma_alphabet_carries_the_special_symbols() -> Result<(), hfst::error::Error> {
    let alpha = foma_of(&basic_acceptor("cat")).get_alphabet();
    for special in [EPSILON, UNKNOWN, IDENTITY] {
        assert!(
            alpha.contains(&sym(special)),
            "{special} missing from the foma alphabet: {alpha:?}"
        );
    }
    Ok(())
}
