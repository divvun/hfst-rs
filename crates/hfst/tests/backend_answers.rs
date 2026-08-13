//! Every `Backend` method must ANSWER, not return a plausible default.
//!
//! The defect this file exists to catch: a method whose real logic was never
//! ported returns an empty / default / success value that the caller consumes
//! as fact, so the failure surfaces as silently wrong output instead of an
//! error. `unimplemented!` / `todo!` / `bail!` are NOT this class — they fail
//! visibly, which is the point. Every core-library instance found so far was a
//! backend trait method: `number_of_states` / `number_of_arcs` / `has_weights`
//! inherited a 0/false trait default, and `substitute_string_transducer`
//! returned `self.clone()` under a comment claiming callers "route through the
//! generic HfstBasicTransducer path" — a safety net that did not exist.
//!
//! The technique here is that a default cannot VARY. Each query is asked over
//! at least two fixtures chosen so the truthful answers differ, and each
//! transformation is asked over a fixture where the truthful result differs
//! from its input. No constant, no `self.clone()`, and no empty collection can
//! satisfy a pair of assertions like that, whatever the backend.
//!
//! Adding a backend? The compiler routes you here twice: `Backend`'s counts and
//! `has_weights` are undefaulted, so an incomplete impl does not build, and
//! `classify` below is a wildcard-free match over `ImplementationType`, so a
//! new tag does not build until it is placed. What the compiler cannot check,
//! and what review has to:
//!
//!   * A method that CANNOT be answered on this backend states so in its own
//!     body, with the reason. It never inherits silence from the trait.
//!   * A comment claiming some other path handles the real case names that
//!     path, and the claim is verified at the call site. Every such comment
//!     found so far was false.
//!   * Two trait methods answered by one shared helper are two claims that the
//!     queries are identical. Check the other backend before aliasing them.
//!   * A no-op that is CORRECT for this backend (foma has no weights to push)
//!     says why in the body, and the battery below asserts only what holds.

use std::collections::BTreeSet;

use hfst::backend::{AlgebraBackend, Backend};
use hfst::backend_thfst::ThfstTransducer;
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_data_types::{HfstTwoLevelPath, ImplementationType, Symbol};
use hfst::hfst_extract_strings::{ExtractStringsCb, RetVal};
use hfst::transducer::{Transducer, WeightedTables};
use hfst_openfst::StdVectorFst;

#[cfg(feature = "foma")]
use hfst::backend_foma::FomaTransducer;

const EPSILON: &str = "@_EPSILON_SYMBOL_@";
const FLAG: &str = "@U.FEAT.VAL@";

/// The tropical/OL symbol coding lives in process-global statics; cargo runs
/// every `#[test]` as a parallel thread in ONE process, so tests touching the
/// OpenFst family serialize through this lock (mirrors test_streams.rs).
static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn sym(s: &str) -> Symbol {
    Symbol::from(s)
}

// ---------------------------------------------------------------------------
// Registry: every ImplementationType declares how it is covered.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Covered {
    /// A `Backend` impl the battery below runs end to end.
    Battery,
    /// A `Backend` impl whose method bodies the battery reaches through another
    /// instantiation of the same generic code.
    SharedBodies,
    /// No `Backend` impl exists for this tag.
    NoImpl,
}

/// Wildcard-free on purpose: a new `ImplementationType` variant fails to
/// compile here until its author says which arm it belongs in. Reusing an
/// existing tag for a new backend is not an escape — the tag it reuses is
/// already claimed by a battery entry point below.
fn classify(t: ImplementationType) -> Covered {
    match t {
        ImplementationType::TROPICAL_OPENFST_TYPE => Covered::Battery,
        ImplementationType::HFST_OLW_TYPE => Covered::Battery,
        ImplementationType::THFST_TYPE => Covered::Battery,
        #[cfg(feature = "foma")]
        ImplementationType::FOMA_TYPE => Covered::Battery,
        #[cfg(not(feature = "foma"))]
        ImplementationType::FOMA_TYPE => Covered::NoImpl,
        // `Transducer<UnweightedTables>` shares every body in this battery's
        // reach with `Transducer<WeightedTables>` (`ol_walk` / `ol_counts` /
        // `ol_has_weights` are generic over the table type). It is not run
        // directly because it has no in-memory constructor: `from_basic`
        // deliberately errors, since conversions always build weighted-shaped
        // tables (interim invariant of [dec:hfst:monomorphic-backends]), so an
        // unweighted-tables value only arises from a disk load. Give this arm a
        // battery entry point the moment that invariant is lifted.
        ImplementationType::HFST_OL_TYPE => Covered::SharedBodies,
        // No backend: SFST and XFSM were never ported, HFST2 is a legacy
        // stream tag, and the last two are facade sentinels.
        ImplementationType::SFST_TYPE => Covered::NoImpl,
        ImplementationType::XFSM_TYPE => Covered::NoImpl,
        ImplementationType::HFST2_TYPE => Covered::NoImpl,
        ImplementationType::UNSPECIFIED_TYPE => Covered::NoImpl,
        ImplementationType::ERROR_TYPE => Covered::NoImpl,
    }
}

/// The next variant in declaration order. Also wildcard-free, and the only
/// place the enum's membership is written down: a new variant has to be spliced
/// into this chain to compile, which is what puts it in front of `classify`.
fn succ(t: ImplementationType) -> Option<ImplementationType> {
    match t {
        ImplementationType::SFST_TYPE => Some(ImplementationType::TROPICAL_OPENFST_TYPE),
        ImplementationType::TROPICAL_OPENFST_TYPE => Some(ImplementationType::FOMA_TYPE),
        ImplementationType::FOMA_TYPE => Some(ImplementationType::XFSM_TYPE),
        ImplementationType::XFSM_TYPE => Some(ImplementationType::HFST_OL_TYPE),
        ImplementationType::HFST_OL_TYPE => Some(ImplementationType::HFST_OLW_TYPE),
        ImplementationType::HFST_OLW_TYPE => Some(ImplementationType::THFST_TYPE),
        ImplementationType::THFST_TYPE => Some(ImplementationType::HFST2_TYPE),
        ImplementationType::HFST2_TYPE => Some(ImplementationType::UNSPECIFIED_TYPE),
        ImplementationType::UNSPECIFIED_TYPE => Some(ImplementationType::ERROR_TYPE),
        ImplementationType::ERROR_TYPE => None,
    }
}

fn all_types() -> Vec<ImplementationType> {
    let mut out = vec![ImplementationType::SFST_TYPE];
    while let Some(next) = succ(*out.last().expect("seeded with one element")) {
        out.push(next);
    }
    out
}

// ---------------------------------------------------------------------------
// Fixtures. Each pair is chosen so the two truthful answers differ.
// ---------------------------------------------------------------------------

/// `inp:outp`, one arc per aligned column.
fn basic_pair(inp: &str, outp: &str) -> HfstBasicTransducer {
    let ic: Vec<String> = inp.chars().map(|c| c.to_string()).collect();
    let oc: Vec<String> = outp.chars().map(|c| c.to_string()).collect();
    assert_eq!(ic.len(), oc.len(), "basic_pair needs aligned columns");
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    for i in 0..ic.len() {
        let tr = HfstBasicTransition::new_symbols(
            (i + 1) as u32,
            sym(&ic[i]),
            sym(&oc[i]),
            0.0,
            net.coder_mut(),
        );
        net.add_transition(i as u32, &tr, true);
    }
    net.set_final_weight(ic.len() as u32, &0.0);
    net
}

fn basic_acceptor(word: &str) -> HfstBasicTransducer {
    basic_pair(word, word)
}

/// A one-state acceptor with a self-loop per symbol — cyclic, but every arc
/// consumes an input symbol, so it is only finitely ambiguous.
fn basic_sigma_star(symbols: &[&str]) -> HfstBasicTransducer {
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    for s in symbols {
        let tr = HfstBasicTransition::new_symbols(0, sym(s), sym(s), 0.0, net.coder_mut());
        net.add_transition(0, &tr, true);
    }
    net.set_final_weight(0, &0.0);
    net
}

/// `(0:a)*` — an input-epsilon cycle, so infinitely ambiguous.
fn basic_epsilon_loop() -> HfstBasicTransducer {
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    let tr = HfstBasicTransition::new_symbols(0, sym(EPSILON), sym("a"), 0.0, net.coder_mut());
    net.add_transition(0, &tr, true);
    net.set_final_weight(0, &0.0);
    net
}

/// A path over explicit symbol vectors, so it can carry multichar symbols
/// (epsilon, flag diacritics) that a per-char split cannot express.
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

/// `a:b` carrying `arc_w` on its arc and `final_w` on its final state.
fn basic_weighted(arc_w: f32, final_w: f32) -> HfstBasicTransducer {
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    let tr = HfstBasicTransition::new_symbols(1, sym("a"), sym("b"), arc_w, net.coder_mut());
    net.add_transition(0, &tr, true);
    net.set_final_weight(1, &final_w);
    net
}

// ---------------------------------------------------------------------------
// Independent witnesses, all read off the interchange form.
// ---------------------------------------------------------------------------

fn witness_counts(net: &HfstBasicTransducer) -> (u32, u32) {
    let states = net.get_max_state() + 1;
    let arcs: usize = net
        .states_and_transitions()
        .iter()
        .map(|trs| trs.len())
        .sum();
    (states, arcs as u32)
}

/// Whether any weight in the interchange form is non-zero — the question
/// `has_weights` answers, computed without asking the backend.
fn witness_has_weight(net: &HfstBasicTransducer) -> bool {
    for (s, transitions) in net.states_and_transitions().iter().enumerate() {
        let s = s as u32;
        if net.is_final_state(s) && net.get_final_weight(s).unwrap_or(0.0) != 0.0 {
            return true;
        }
        if transitions.iter().any(|tr| tr.get_weight() != 0.0) {
            return true;
        }
    }
    false
}

struct PairCollector {
    pairs: BTreeSet<(String, String)>,
    cap: usize,
}

impl ExtractStringsCb for PairCollector {
    fn operator_call(&mut self, path: &mut HfstTwoLevelPath, is_final: bool) -> RetVal {
        if is_final {
            let mut i = String::new();
            let mut o = String::new();
            for (a, b) in path.second.iter() {
                if a.as_str() != EPSILON {
                    i.push_str(a.as_str());
                }
                if b.as_str() != EPSILON {
                    o.push_str(b.as_str());
                }
            }
            self.pairs.insert((i, o));
        }
        RetVal::new(self.pairs.len() < self.cap, true)
    }
}

/// The accepted `(input, output)` relation, concatenated per path so backends
/// that hand back whole words and backends that hand back per-symbol columns
/// compare equal.
fn accepted<B: Backend>(b: &B) -> BTreeSet<(String, String)> {
    accepted_within(b, -1)
}

/// The same, bounded to `cycles` traversals per state — the only safe form for
/// a result that may be cyclic (`repeat_star` and friends).
fn accepted_within<B: Backend>(b: &B, cycles: i32) -> BTreeSet<(String, String)> {
    let mut cb = PairCollector {
        pairs: BTreeSet::new(),
        cap: 4096,
    };
    b.extract_paths_cb(&mut cb, cycles);
    cb.pairs
}

/// The same relation, but through the flag-diacritic arm at a given filter
/// setting — the two settings must not agree on a net carrying flags.
fn accepted_fd<B: Backend>(b: &B, filter_fd: bool) -> BTreeSet<(String, String)> {
    let mut cb = PairCollector {
        pairs: BTreeSet::new(),
        cap: 4096,
    };
    b.extract_paths_fd_cb(&mut cb, -1, filter_fd);
    cb.pairs
}

fn pairs(items: &[(&str, &str)]) -> BTreeSet<(String, String)> {
    items
        .iter()
        .map(|(a, b)| ((*a).to_string(), (*b).to_string()))
        .collect()
}

fn alphabet_of<B: Backend>(b: &B) -> BTreeSet<String> {
    b.get_alphabet().iter().map(|s| s.to_string()).collect()
}

fn build<B: Backend>(net: &HfstBasicTransducer, what: &str) -> B {
    B::from_basic(net).unwrap_or_else(|e| panic!("from_basic({what}): {e}"))
}

// ---------------------------------------------------------------------------
// The battery. Generic over the backend, so a new one inherits every assertion.
// ---------------------------------------------------------------------------

fn assert_queries_vary<B: Backend>(tag: &str) {
    // -- counts: a stubbed 0 reads at the call site as an honestly empty net.
    for (name, net) in [
        ("abc:xyz", basic_pair("abc", "xyz")),
        ("{a,b,c}*", basic_sigma_star(&["a", "b", "c"])),
    ] {
        let b: B = build(&net, name);
        let own = b.to_basic().expect("to_basic");
        assert_eq!(
            (b.number_of_states(), b.number_of_arcs()),
            witness_counts(&own),
            "{tag}: counts disagree with the graph the backend itself hands back ({name})"
        );
        assert!(
            b.number_of_states() > 0 && b.number_of_arcs() > 0,
            "{tag}: {name} is a non-trivial net, so neither count may be 0"
        );
    }

    // -- has_weights is a question about content, not table shape. Every
    // backend must agree with its own graph, and a backend that can carry a
    // weight must give BOTH answers across the pair.
    let mut weight_answers = BTreeSet::new();
    for (name, net) in [
        ("all-zero", basic_weighted(0.0, 0.0)),
        ("weighted arc", basic_weighted(0.5, 0.0)),
        ("weighted final", basic_weighted(0.0, 0.5)),
    ] {
        let b: B = build(&net, name);
        let own = b.to_basic().expect("to_basic");
        assert_eq!(
            b.has_weights(),
            witness_has_weight(&own),
            "{tag}: has_weights disagrees with the weights in its own graph ({name})"
        );
        weight_answers.insert(witness_has_weight(&own));
    }
    if weight_answers.len() > 1 {
        // The backend preserves weights, so a constant answer is a stub.
        let carried: Vec<bool> = [
            basic_weighted(0.0, 0.0),
            basic_weighted(0.5, 0.0),
            basic_weighted(0.0, 0.5),
        ]
        .iter()
        .map(|net| build::<B>(net, "weighted").has_weights())
        .collect();
        assert_eq!(
            carried,
            vec![false, true, true],
            "{tag}: has_weights must vary with the weights the net carries"
        );
    }

    // -- get_alphabet must reflect the net, not a fixed set.
    let ab: B = build(&basic_acceptor("ab"), "ab");
    let xy: B = build(&basic_acceptor("xy"), "xy");
    for (name, b, want) in [("ab", &ab, ["a", "b"]), ("xy", &xy, ["x", "y"])] {
        let alphabet = alphabet_of(b);
        for symbol in want {
            assert!(
                alphabet.contains(symbol),
                "{tag}: alphabet of {name} is missing {symbol}"
            );
        }
    }
    assert_ne!(
        alphabet_of(&ab),
        alphabet_of(&xy),
        "{tag}: get_alphabet must vary with the net"
    );

    // -- stream_type is the tag written to disk; a wrong one is a corrupt file.
    assert_eq!(
        ab.stream_type(),
        B::TYPE,
        "{tag}: stream_type must name this backend"
    );
    assert_eq!(
        classify(B::TYPE),
        Covered::Battery,
        "{tag}: this backend's tag must be registered as covered"
    );
}

fn assert_conversions_preserve<B: Backend>(tag: &str) {
    let want = pairs(&[("abc", "xyz")]);
    let b: B = build(&basic_pair("abc", "xyz"), "abc:xyz");

    // -- extract_paths_cb: an empty callback stream is the silent shape here.
    assert_eq!(
        accepted(&b),
        want,
        "{tag}: extract_paths_cb must yield the net's relation"
    );

    // -- copy: `Self::empty()` would satisfy the signature and nothing else.
    let copied = b.copy().expect("copy");
    assert_eq!(
        accepted(&copied),
        want,
        "{tag}: copy must copy the relation"
    );
    assert_eq!(
        (copied.number_of_states(), copied.number_of_arcs()),
        (b.number_of_states(), b.number_of_arcs()),
        "{tag}: copy must copy the graph"
    );

    // -- the interchange round trip.
    let basic = b.to_basic().expect("to_basic");
    let back: B = build(&basic, "round trip");
    assert_eq!(
        accepted(&back),
        want,
        "{tag}: to_basic/from_basic must preserve the relation"
    );

    // -- to_hfst_ol: the conversion the pmatch archive writer runs per member.
    let ol = b.to_hfst_ol(true, "", None).expect("to_hfst_ol");
    assert_eq!(
        accepted(&ol),
        want,
        "{tag}: to_hfst_ol must carry the relation into the OL tables"
    );

    // -- write: either refuse loudly or actually emit bytes. `Ok(())` over an
    // untouched sink is the silent shape.
    let mut buf: Vec<u8> = Vec::new();
    if b.write(&mut buf, true).is_ok() {
        assert!(
            !buf.is_empty(),
            "{tag}: write returned Ok without writing anything"
        );
    }

    // -- write_to_dir: same contract against a directory container.
    let dir = std::env::temp_dir().join(format!(
        "hfst-backend-answers-{}-{}",
        std::process::id(),
        tag
    ));
    let _ = std::fs::remove_dir_all(&dir);
    if b.write_to_dir(&dir).is_ok() {
        let members = std::fs::read_dir(&dir)
            .map(|entries| entries.count())
            .unwrap_or(0);
        assert!(
            members > 0,
            "{tag}: write_to_dir returned Ok without writing anything"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

fn assert_mutations_take_effect<B: Backend>(tag: &str) {
    // -- insert_to_alphabet: `Ok(())` that inserts nothing is the silent shape.
    let mut b: B = build(&basic_acceptor("ab"), "ab");
    let before = alphabet_of(&b);
    b.insert_to_alphabet("zebra").expect("insert_to_alphabet");
    let after = alphabet_of(&b);
    assert!(
        after.contains("zebra"),
        "{tag}: insert_to_alphabet reported success without inserting"
    );
    assert_ne!(before, after, "{tag}: insert_to_alphabet changed nothing");

    // -- the flag encode/decode pair. Encode must rewrite the flag to its
    // escaped form and decode must put it back, so neither can be a no-op.
    let flag_net = basic_symbols(&["a", FLAG, "b"], &["a", FLAG, "b"]);
    let mut b: B = build(&flag_net, "flag net");
    let plain = alphabet_of(&b);
    assert!(
        plain.contains(FLAG),
        "{tag}: the fixture's flag must reach the backend alphabet"
    );
    b.encode_flag_diacritics();
    let encoded = alphabet_of(&b);
    assert!(
        !encoded.contains(FLAG) && encoded.contains("%U.FEAT.VAL%"),
        "{tag}: encode_flag_diacritics left the flag unescaped"
    );
    b.decode_flag_diacritics();
    assert!(
        alphabet_of(&b).contains(FLAG),
        "{tag}: decode_flag_diacritics did not restore the flag"
    );

    // -- extract_paths_fd_cb: ignoring `filter_fd` is how the flag columns
    // silently vanished from fst2strings under foma.
    let b: B = build(&flag_net, "flag net");
    assert_ne!(
        accepted_fd(&b, true),
        accepted_fd(&b, false),
        "{tag}: filter_fd must change what a flag-carrying net extracts"
    );
}

fn run_battery<B: Backend>(tag: &str) {
    assert_queries_vary::<B>(tag);
    assert_conversions_preserve::<B>(tag);
    assert_mutations_take_effect::<B>(tag);
}

// ---------------------------------------------------------------------------
// Set-at-a-time alphabet edits. Held out of the shared battery because foma
// cannot honour them — see the expected-red pin below.
// ---------------------------------------------------------------------------

/// `add_symbols_to_alphabet` and `remove_from_alphabet` are inverses, so
/// neither can be an `Ok(())` no-op that the other's assertion happens to
/// cover.
fn assert_alphabet_set_edits<B: Backend>(tag: &str) {
    let mut b: B = build(&basic_acceptor("ab"), "ab");
    let mut symbols = hfst::hfst_symbol_defs::StringSet::new();
    symbols.insert(sym("quokka"));
    symbols.insert(sym("wombat"));
    b.add_symbols_to_alphabet(&symbols)
        .expect("add_symbols_to_alphabet");
    let after = alphabet_of(&b);
    for symbol in ["quokka", "wombat"] {
        assert!(
            after.contains(symbol),
            "{tag}: add_symbols_to_alphabet reported success without adding {symbol}"
        );
    }

    b.remove_from_alphabet("quokka")
        .expect("remove_from_alphabet");
    let after = alphabet_of(&b);
    assert!(
        !after.contains("quokka") && after.contains("wombat"),
        "{tag}: remove_from_alphabet removed the wrong thing, or nothing"
    );
}

#[test]
fn tropical_honours_alphabet_set_edits() {
    let _g = serialized();
    assert_alphabet_set_edits::<StdVectorFst>("tropical");
}

#[test]
fn optimized_lookup_honours_alphabet_set_edits() {
    let _g = serialized();
    assert_alphabet_set_edits::<Transducer<WeightedTables>>("olw");
    assert_alphabet_set_edits::<ThfstTransducer>("thfst");
}

/// EXPECTED RED WHEN FIXED. foma overrides `insert_to_alphabet` (a direct
/// `sigma_add`, asserted in the shared battery) but inherits the round-trip
/// default for the set-at-a-time edits, and the round trip cannot carry them:
/// `FomaTransducer::from_basic` builds the sigma purely by interning each arc's
/// symbols and never reads the interchange net's alphabet, so every symbol on
/// no arc is dropped on the way back in. `add_symbols_to_alphabet` therefore
/// returns `Ok(())` having added nothing — the silent-success shape, and
/// inconsistent with the single-symbol path on the very same backend.
///
/// `remove_from_alphabet` is the same defect with the opposite sign: it appears
/// to work, because the round trip drops arc-less symbols whether or not they
/// were named, and it cannot touch a symbol that is still on an arc.
///
/// The consumer to check when fixing this is `hfst_xerox_rules`, which strips
/// its temporary markers with `remove_from_alphabet_symbol` / `_set` on every
/// replace-rule compile.
#[cfg(feature = "foma")]
#[test]
fn foma_alphabet_set_edits_are_silent_no_ops() {
    let _g = serialized();

    let mut b: FomaTransducer = build(&basic_acceptor("ab"), "ab");
    let mut symbols = hfst::hfst_symbol_defs::StringSet::new();
    symbols.insert(sym("quokka"));
    b.add_symbols_to_alphabet(&symbols)
        .expect("add_symbols_to_alphabet");
    assert!(
        !alphabet_of(&b).contains("quokka"),
        "foma add_symbols_to_alphabet now adds — wire foma into \
         assert_alphabet_set_edits and delete this pin"
    );

    b.remove_from_alphabet("a").expect("remove_from_alphabet");
    assert!(
        alphabet_of(&b).contains("a"),
        "foma remove_from_alphabet now reaches a symbol that is still on an \
         arc — wire foma into assert_alphabet_set_edits and delete this pin"
    );
}

// ---------------------------------------------------------------------------
// Battery entry points, one per registered backend.
// ---------------------------------------------------------------------------

#[test]
fn tropical_backend_answers_every_query() {
    let _g = serialized();
    run_battery::<StdVectorFst>("tropical");
}

#[test]
fn optimized_lookup_backend_answers_every_query() {
    let _g = serialized();
    run_battery::<Transducer<WeightedTables>>("olw");
}

#[test]
fn thfst_backend_answers_every_query() {
    let _g = serialized();
    run_battery::<ThfstTransducer>("thfst");
}

#[cfg(feature = "foma")]
#[test]
fn foma_backend_answers_every_query() {
    let _g = serialized();
    run_battery::<FomaTransducer>("foma");
}

// ---------------------------------------------------------------------------
// Graph properties. Every backend answers these from its own graph.
// ---------------------------------------------------------------------------

/// `is_cyclic` and `is_infinitely_ambiguous` are different questions, and
/// neither may be constant. `a*` separates them: cyclic, but every arc consumes
/// an input symbol, so it is only finitely ambiguous.
fn assert_graph_properties<B: Backend>(tag: &str) {
    for (name, net, cyclic, ambiguous) in [
        ("abc:xyz", basic_pair("abc", "xyz"), false, false),
        ("a*", basic_sigma_star(&["a"]), true, false),
        ("(0:a)*", basic_epsilon_loop(), true, true),
    ] {
        let b: B = build(&net, name);
        assert_eq!(b.is_cyclic(), cyclic, "{tag}: is_cyclic({name})");
        assert_eq!(
            b.is_infinitely_ambiguous().expect("ambiguity query"),
            ambiguous,
            "{tag}: is_infinitely_ambiguous({name})"
        );
    }
}

#[test]
fn tropical_reports_real_graph_properties() {
    let _g = serialized();
    assert_graph_properties::<StdVectorFst>("tropical");
}

#[cfg(feature = "foma")]
#[test]
fn foma_reports_real_graph_properties() {
    let _g = serialized();
    assert_graph_properties::<FomaTransducer>("foma");
}

/// This pin used to assert the OL family got both answers WRONG: it probed
/// `HeaderFlag::Cyclic` / `HeaderFlag::Has_input_epsilon_cycles`, which nothing
/// in either tree ever sets — `TransducerHeader::set_flag` has no callers, and
/// the in-memory constructors hardcode the flags false — so every OL/OLW/THFST
/// transducer answered false to both, whatever it contained, behind an override
/// of the trait default that WOULD have computed the answer. The queries now
/// walk the tables, so the pin is the same battery every other backend runs.
/// Cyclicity needs on-stack marking rather than a visited set, and infinite
/// ambiguity needs input-epsilon arcs specifically; both distinctions are
/// pinned in detail in `ol_graph_properties.rs`.
#[test]
fn optimized_lookup_reports_real_graph_properties() {
    let _g = serialized();
    assert_graph_properties::<Transducer<WeightedTables>>("olw");
    assert_graph_properties::<ThfstTransducer>("thfst");
}

/// The registry is only worth its compile error if the arms it claims are real:
/// every `Battery` tag must be one a battery entry point above actually ran.
#[test]
fn every_backend_tag_has_a_battery() {
    let _g = serialized();

    let mut ran: BTreeSet<ImplementationType> = BTreeSet::new();
    ran.insert(<StdVectorFst as Backend>::TYPE);
    ran.insert(<Transducer<WeightedTables> as Backend>::TYPE);
    ran.insert(<ThfstTransducer as Backend>::TYPE);
    #[cfg(feature = "foma")]
    ran.insert(<FomaTransducer as Backend>::TYPE);

    let claimed: BTreeSet<ImplementationType> = all_types()
        .into_iter()
        .filter(|t| classify(*t) == Covered::Battery)
        .collect();

    assert_eq!(
        claimed, ran,
        "a tag classified Battery has no entry point (or vice versa)"
    );
}

// ---------------------------------------------------------------------------
// AlgebraBackend: a transformation must transform.
// ---------------------------------------------------------------------------

/// Ops whose result is guaranteed to differ from the input on these fixtures,
/// for any correct backend. `self.clone()` fails every one of them.
fn assert_algebra_transforms<B: AlgebraBackend>(tag: &str) {
    let ab: B = build(&basic_pair("ab", "xy"), "ab:xy");

    assert_eq!(
        accepted(&ab.invert()),
        pairs(&[("xy", "ab")]),
        "{tag}: invert must swap the two sides"
    );
    assert_eq!(
        accepted(&ab.extract_input_language()),
        pairs(&[("ab", "ab")]),
        "{tag}: extract_input_language must project onto the input side"
    );
    assert_eq!(
        accepted(&ab.extract_output_language()),
        pairs(&[("xy", "xy")]),
        "{tag}: extract_output_language must project onto the output side"
    );
    assert_eq!(
        accepted(&ab.reverse()),
        pairs(&[("ba", "yx")]),
        "{tag}: reverse must reverse the path"
    );

    // The empty string is in none of these fixtures and in all of these
    // results, so a clone of the input cannot pass. Star and plus are cyclic,
    // so every extraction here is cycle-bounded.
    for (name, got) in [
        ("repeat_star", ab.repeat_star()),
        ("repeat_le_n", ab.repeat_le_n(2)),
        ("optionalize", ab.optionalize()),
    ] {
        assert!(
            accepted_within(&got, 1).contains(&(String::new(), String::new())),
            "{tag}: {name} must admit the empty path"
        );
    }
    for (name, got) in [
        ("repeat_plus", ab.repeat_plus()),
        ("repeat_n", ab.repeat_n(2)),
    ] {
        assert!(
            accepted_within(&got, 1).contains(&("abab".to_string(), "xyxy".to_string())),
            "{tag}: {name} must admit the doubled path"
        );
    }

    // Binary ops with identical operands still have a guaranteed-different
    // result, and identical operands keep the two symbol tables in step (the
    // raw backend ops do not harmonize).
    assert_eq!(
        accepted(&ab.concatenate(&ab)),
        pairs(&[("abab", "xyxy")]),
        "{tag}: concatenate must join the two paths"
    );
    assert!(
        accepted(&ab.subtract(&ab)).is_empty(),
        "{tag}: a net minus itself is empty"
    );

    // Predicates over pairs chosen so a constant answer fails one of them. The
    // unequal operand is DERIVED from `ab` rather than built beside it: the raw
    // backend ops do not harmonize, so two separately built nets would compare
    // over two unrelated symbol tables.
    assert!(
        ab.are_equivalent(&ab.copy().expect("copy"), false),
        "{tag}: a net is equivalent to its own copy"
    );
    assert!(
        !ab.are_equivalent(&ab.concatenate(&ab), false),
        "{tag}: a net is not equivalent to itself concatenated"
    );
    let acceptor: B = build(&basic_acceptor("ab"), "ab");
    assert!(
        acceptor.is_automaton(),
        "{tag}: an acceptor is an automaton"
    );
    assert!(!ab.is_automaton(), "{tag}: a:b is not an automaton");

    // Symbol queries: the empty set is the silent answer here, and it must also
    // track the net rather than being fixed.
    for (name, b) in [("ab:xy", &ab), ("acceptor", &acceptor)] {
        assert!(
            !b.get_initial_input_symbols().is_empty(),
            "{tag}: get_initial_input_symbols({name}) is empty"
        );
        assert!(
            !b.get_first_input_symbols().is_empty(),
            "{tag}: get_first_input_symbols({name}) is empty"
        );
    }
    let xy: B = build(&basic_acceptor("xy"), "xy");
    assert_ne!(
        acceptor.get_initial_input_symbols(),
        xy.get_initial_input_symbols(),
        "{tag}: get_initial_input_symbols must vary with the net"
    );
    assert_ne!(
        acceptor.get_first_input_symbols(),
        xy.get_first_input_symbols(),
        "{tag}: get_first_input_symbols must vary with the net"
    );

    // Constructors: `Self::empty()` satisfies the signature and nothing else.
    assert_eq!(
        accepted(&B::define_transducer_symbol("q")),
        pairs(&[("q", "q")]),
        "{tag}: define_transducer_symbol must build the symbol"
    );
    assert_eq!(
        accepted(&B::define_transducer_symbol_pair("q", "r")),
        pairs(&[("q", "r")]),
        "{tag}: define_transducer_symbol_pair must build the pair"
    );
    assert_eq!(
        accepted(&B::define_transducer_spv(&vec![
            (sym("q"), sym("r")),
            (sym("s"), sym("t")),
        ])),
        pairs(&[("qs", "rt")]),
        "{tag}: define_transducer_spv must concatenate the pairs"
    );

    // disjunct_spv mutates in place; the added path must appear.
    let mut grown: B = build(&basic_pair("ab", "xy"), "ab:xy");
    grown.disjunct_spv(&vec![(sym("a"), sym("a")), (sym("b"), sym("b"))]);
    let after = accepted(&grown);
    assert!(
        after.contains(&("ab".to_string(), "ab".to_string()))
            && after.contains(&("ab".to_string(), "xy".to_string())),
        "{tag}: disjunct_spv must add the path without dropping the old one"
    );

    // extract_random_paths fills a caller-owned collection; leaving it untouched
    // is the silent shape.
    let mut sampled = hfst::hfst_data_types::HfstTwoLevelPaths::new();
    ab.extract_random_paths(&mut sampled, 1);
    assert!(
        !sampled.is_empty(),
        "{tag}: extract_random_paths returned no path for a one-path net"
    );

    // substitute_string_transducer is the method that returned `self.clone()`
    // under a comment claiming callers routed around it. Asserted structurally:
    // splicing a relation in place of an arc has to add states. The resulting
    // LABELS are not asserted because the raw backend ops do not harmonize —
    // the replacement carries its own symbol table, and reconciling the two is
    // the facade's job, not this method's.
    let sub: B = build(&basic_pair("q", "r"), "q:r");
    let substituted = ab.substitute_string_transducer((sym("a"), sym("x")), &sub);
    assert!(
        substituted.number_of_states() > ab.number_of_states(),
        "{tag}: substitute_string_transducer returned the net unchanged"
    );

    // The fast path may decline (tropical returns None by design), but if it
    // answers, the answer has to be the substitution.
    if let Some(fast) = ab.substitute_symbol_fast("a", "q") {
        assert_eq!(
            accepted(&fast),
            pairs(&[("qb", "xy")]),
            "{tag}: substitute_symbol_fast answered without substituting"
        );
    }
}

/// The weight-carrying half of the algebra, asserted only where weights exist:
/// foma's line table has no weight field, so its copies are honest no-ops.
fn assert_weight_ops_transform<B: AlgebraBackend>(tag: &str) {
    let net: B = build(&basic_weighted(0.0, 0.0), "all-zero");
    assert!(
        !net.has_weights(),
        "{tag}: the fixture starts with no weight"
    );
    assert!(
        net.set_final_weights(0.5, false).has_weights(),
        "{tag}: set_final_weights must put a weight on the net"
    );

    let weighted: B = build(&basic_weighted(0.5, 0.0), "weighted arc");
    assert!(
        !weighted.transform_weights(|_| 0.0).has_weights(),
        "{tag}: transform_weights must reach the weights"
    );
}

#[test]
fn tropical_algebra_actually_transforms() {
    let _g = serialized();
    assert_algebra_transforms::<StdVectorFst>("tropical");
    assert_weight_ops_transform::<StdVectorFst>("tropical");
}

#[cfg(feature = "foma")]
#[test]
fn foma_algebra_actually_transforms() {
    let _g = serialized();
    assert_algebra_transforms::<FomaTransducer>("foma");
}
