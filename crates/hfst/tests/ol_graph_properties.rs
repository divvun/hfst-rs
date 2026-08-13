// The optimized-lookup family's graph predicates, and the header of the files
// it writes.
//
// `Transducer::is_cyclic` / `is_infinitely_ambiguous` used to probe
// `HeaderFlag::Cyclic` / `HeaderFlag::Has_input_epsilon_cycles`. Nothing in
// this workspace — or in the C++ tree it was ported from — ever calls
// `TransducerHeader::set_flag`, and every in-memory constructor hardcoded both
// flags false, so both queries answered "no" for every transducer ever built
// here. Path extraction is guarded on those answers, so `hfst fst2strings` on
// an `.hfstol` of `[a b]*` enumerated its infinite language until the disk
// filled, where the same net as tropical refused with "Transducer is cyclic".
//
// Every case below is therefore asked of a graph whose truthful answer is
// known by construction, and asked in pairs that no constant can satisfy:
// `a*` is cyclic and NOT infinitely ambiguous, `(0:a)*` is both.
//
// The tropical transition-data symbol coding used by `HfstBasicTransducer`
// lives in process-global statics guarded by their own mutexes; cargo runs each
// #[test] as a parallel thread in one process, so construction is serialized
// through a shared lock, matching the house style in format_limits.rs.

use hfst::backend::Backend;
use hfst::backend_thfst::ThfstTransducer;
use hfst::convert_transducer_format::ConversionFunctions;
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::transducer::{HeaderFlag, IStream, Transducer, WeightedTables};

const EPSILON: &str = "@_EPSILON_SYMBOL_@";
const FLAG: &str = "@U.FEAT.VAL@";

static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn arc(net: &mut HfstBasicTransducer, from: u32, to: u32, i: &str, o: &str, w: f32) {
    let coder = net.coder_mut();
    let tr = HfstBasicTransition::new_symbols(to, i.into(), o.into(), w, coder);
    net.add_transition(from, &tr, true);
}

/// `a*` — cyclic, but every turn of the loop consumes an input symbol, so one
/// input string still has finitely many analyses.
fn star() -> HfstBasicTransducer {
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    arc(&mut net, 0, 0, "a", "a", 0.0);
    net.set_final_weight(0, &0.0);
    net
}

/// `(0:a)*` — an input-epsilon cycle: the empty input has unboundedly many
/// analyses.
fn epsilon_star(weight: f32) -> HfstBasicTransducer {
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    arc(&mut net, 0, 0, EPSILON, "a", weight);
    net.set_final_weight(0, &0.0);
    net
}

/// `a b : x y` — no cycle of any kind.
fn straight_path() -> HfstBasicTransducer {
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    arc(&mut net, 0, 1, "a", "x", 0.0);
    arc(&mut net, 1, 2, "b", "y", 0.0);
    net.set_final_weight(2, &0.0);
    net
}

/// `a (0:b)*` — the epsilon cycle sits BEHIND an input arc, so a walk that only
/// followed epsilon arcs out of the start state would never reach it.
fn epsilon_cycle_behind_input() -> HfstBasicTransducer {
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    arc(&mut net, 0, 1, "a", "a", 0.0);
    arc(&mut net, 1, 1, EPSILON, "b", 0.0);
    net.set_final_weight(1, &0.0);
    net
}

/// A diamond: two disjoint paths reconverge on one state. Re-reaching a state
/// is not a cycle, and a visited-set walk without on-stack marking says it is.
fn diamond() -> HfstBasicTransducer {
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    arc(&mut net, 0, 1, "a", "a", 0.0);
    arc(&mut net, 0, 2, "b", "b", 0.0);
    arc(&mut net, 1, 3, "c", "c", 0.0);
    arc(&mut net, 2, 3, "c", "c", 0.0);
    net.set_final_weight(3, &0.0);
    net
}

/// A flag-diacritic self-loop. Flags consume nothing off the input tape, so the
/// lookup engine's `find_loop` and `HfstBasicTransducer::is_infinitely_ambiguous`
/// both count them as epsilon.
fn flag_cycle() -> HfstBasicTransducer {
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    arc(&mut net, 0, 0, FLAG, FLAG, 0.0);
    net.set_final_weight(0, &0.0);
    net
}

fn to_ol(net: &HfstBasicTransducer) -> Transducer<WeightedTables> {
    ConversionFunctions::hfst_basic_transducer_to_hfst_ol(net, true, "", None)
        .expect("fixture is well within the OL format limits")
}

/// Serialize and read back, so the answers can be asked of a transducer that
/// came off "disk" rather than out of a conversion.
fn round_trip(t: &Transducer<WeightedTables>) -> Transducer<WeightedTables> {
    let mut bytes: Vec<u8> = Vec::new();
    t.write(&mut bytes);
    let mut cursor = std::io::Cursor::new(bytes);
    let mut is = IStream::new(&mut cursor);
    Transducer::<WeightedTables>::new_istream(&mut is).expect("round-tripped OL bytes are valid")
}

// ---------------------------------------------------------------------------
// The two queries, over graphs whose answers are known by construction.
// ---------------------------------------------------------------------------

#[test]
fn star_is_cyclic_but_finitely_ambiguous() {
    let _g = serialized();
    let t = to_ol(&star());
    assert!(t.is_cyclic(), "a* loops");
    assert!(
        !t.is_infinitely_ambiguous(),
        "a* consumes an input symbol per turn"
    );
}

#[test]
fn epsilon_star_is_cyclic_and_infinitely_ambiguous() {
    let _g = serialized();
    let t = to_ol(&epsilon_star(0.0));
    assert!(t.is_cyclic());
    assert!(t.is_infinitely_ambiguous(), "(0:a)* loops on empty input");
}

#[test]
fn straight_path_is_neither_cyclic_nor_ambiguous() {
    let _g = serialized();
    let t = to_ol(&straight_path());
    assert!(!t.is_cyclic());
    assert!(!t.is_infinitely_ambiguous());
}

#[test]
fn a_diamond_is_not_a_cycle() {
    let _g = serialized();
    let t = to_ol(&diamond());
    assert!(!t.is_cyclic(), "reconverging paths are not a back edge");
}

#[test]
fn epsilon_cycle_behind_an_input_arc_is_found() {
    let _g = serialized();
    let t = to_ol(&epsilon_cycle_behind_input());
    assert!(t.is_cyclic());
    assert!(
        t.is_infinitely_ambiguous(),
        "the epsilon cycle is reachable only across an input arc"
    );
}

#[test]
fn a_flag_cycle_counts_as_input_epsilon() {
    let _g = serialized();
    let t = to_ol(&flag_cycle());
    assert!(t.is_cyclic());
    assert!(
        t.is_infinitely_ambiguous(),
        "a flag diacritic advances no input tape position"
    );
}

/// The interchange form is the reference reading of both questions; the OL walk
/// must not disagree with the graph it converts to.
#[test]
fn ol_answers_agree_with_the_basic_transducer() {
    let _g = serialized();
    for (name, net) in [
        ("a*", star()),
        ("(0:a)*", epsilon_star(0.0)),
        ("ab:xy", straight_path()),
        ("a(0:b)*", epsilon_cycle_behind_input()),
        ("diamond", diamond()),
    ] {
        let t = to_ol(&net);
        assert_eq!(
            t.is_infinitely_ambiguous(),
            net.is_infinitely_ambiguous(),
            "{name}: OL and basic disagree on infinite ambiguity"
        );
    }
}

// ---------------------------------------------------------------------------
// The written file describes itself.
// ---------------------------------------------------------------------------

#[test]
fn written_then_reread_reports_the_same_answers() {
    let _g = serialized();
    for (name, net) in [
        ("a*", star()),
        ("(0:a)*", epsilon_star(0.0)),
        ("ab:xy", straight_path()),
        ("a(0:b)*", epsilon_cycle_behind_input()),
    ] {
        let memory = to_ol(&net);
        let disk = round_trip(&memory);
        assert_eq!(
            disk.is_cyclic(),
            memory.is_cyclic(),
            "{name}: cyclicity changed across a write/read round trip"
        );
        assert_eq!(
            disk.is_infinitely_ambiguous(),
            memory.is_infinitely_ambiguous(),
            "{name}: ambiguity changed across a write/read round trip"
        );
    }
}

/// The header we emit is what a third-party reader (the C++ `hfst-fst2strings`
/// among them) trusts, and it used to say "acyclic, no epsilon cycles" for
/// every file this port wrote.
#[test]
fn the_written_header_records_the_real_properties() {
    let _g = serialized();
    for (name, net, cyclic, eps_cycles) in [
        ("a*", star(), true, false),
        ("(0:a)*", epsilon_star(0.0), true, true),
        ("ab:xy", straight_path(), false, false),
    ] {
        let header = round_trip(&to_ol(&net));
        let header = header.get_header();
        assert_eq!(
            header.probe_flag(HeaderFlag::Cyclic),
            cyclic,
            "{name}: Cyclic flag on disk"
        );
        assert_eq!(
            header.probe_flag(HeaderFlag::Has_input_epsilon_cycles),
            eps_cycles,
            "{name}: Has_input_epsilon_cycles flag on disk"
        );
    }
}

#[test]
fn the_written_header_records_epsilon_arcs() {
    let _g = serialized();
    let straight = round_trip(&to_ol(&straight_path()));
    assert!(
        !straight
            .get_header()
            .probe_flag(HeaderFlag::Has_input_epsilon_transitions)
    );

    let eps = round_trip(&to_ol(&epsilon_star(0.0)));
    assert!(
        eps.get_header()
            .probe_flag(HeaderFlag::Has_input_epsilon_transitions)
    );
    assert!(
        !eps.get_header()
            .probe_flag(HeaderFlag::Has_epsilon_epsilon_transitions),
        "(0:a) emits a symbol, so it is not epsilon on both tapes"
    );
}

/// A flag diacritic advances no input tape position, which is why
/// `is_infinitely_ambiguous` counts a flag cycle as an input-epsilon cycle. The
/// written header has to take the same reading: a file claiming an
/// input-epsilon CYCLE while denying it has any input-epsilon TRANSITION
/// describes a graph that cannot exist.
#[test]
fn the_header_reads_flags_as_the_engine_does() {
    let _g = serialized();
    let t = to_ol(&flag_cycle());
    assert!(t.is_infinitely_ambiguous(), "engine reading, for reference");

    let header = round_trip(&t);
    let header = header.get_header();
    assert!(header.probe_flag(HeaderFlag::Has_input_epsilon_cycles));
    assert!(
        header.probe_flag(HeaderFlag::Has_input_epsilon_transitions),
        "the cycle is made of arcs the header just denied having"
    );
    assert!(
        !header.probe_flag(HeaderFlag::Has_epsilon_epsilon_transitions),
        "the flag is written to the output tape, so it is not epsilon on both"
    );
}

/// A weight on the loop arc is what a cutoff prunes; a free loop is the one
/// nothing can stop.
#[test]
fn only_a_free_epsilon_loop_is_unweighted() {
    let _g = serialized();
    let free = round_trip(&to_ol(&epsilon_star(0.0)));
    assert!(
        free.get_header()
            .probe_flag(HeaderFlag::Has_unweighted_input_epsilon_cycles)
    );

    let costly = round_trip(&to_ol(&epsilon_star(1.5)));
    assert!(
        costly
            .get_header()
            .probe_flag(HeaderFlag::Has_input_epsilon_cycles)
    );
    assert!(
        !costly
            .get_header()
            .probe_flag(HeaderFlag::Has_unweighted_input_epsilon_cycles),
        "every arc of this cycle costs 1.5"
    );
}

/// `Deterministic` / `Input_deterministic` / `Minimized` are claims no single
/// walk can establish. The C++ stamped all three true on every conversion; we
/// claim nothing, which is the direction that cannot mislead a consumer into
/// skipping work.
#[test]
fn undecidable_properties_are_not_claimed() {
    let _g = serialized();
    // Deliberately non-minimal and input-nondeterministic: two arcs share the
    // input symbol `a`, and the two branches are equivalent.
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    arc(&mut net, 0, 1, "a", "x", 0.0);
    arc(&mut net, 0, 2, "a", "x", 0.0);
    net.set_final_weight(1, &0.0);
    net.set_final_weight(2, &0.0);

    let header = round_trip(&to_ol(&net));
    let header = header.get_header();
    assert!(!header.probe_flag(HeaderFlag::Deterministic));
    assert!(!header.probe_flag(HeaderFlag::Input_deterministic));
    assert!(!header.probe_flag(HeaderFlag::Minimized));
}

/// THFST stores no header at all, so its answers can only come from the graph.
#[test]
fn thfst_answers_from_the_graph() {
    let _g = serialized();
    for (name, net, cyclic, ambiguous) in [
        ("a*", star(), true, false),
        ("(0:a)*", epsilon_star(0.0), true, true),
        ("ab:xy", straight_path(), false, false),
    ] {
        let t = ThfstTransducer::from_basic(&net).expect("thfst conversion");
        assert_eq!(t.is_cyclic(), cyclic, "{name}: thfst is_cyclic");
        assert_eq!(
            t.is_infinitely_ambiguous().expect("ambiguity query"),
            ambiguous,
            "{name}: thfst is_infinitely_ambiguous"
        );
    }
}
