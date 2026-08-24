// Termination of the general (non-optimized-lookup) lookup engine.
//
// The engine is a depth-first walk of an `HfstBasicTransducer`, and the arcs
// that consume no input — epsilons and flag diacritics — are the ones that can
// return it to where it already stands. The failure that motivated these tests:
// `hfst lookup` on a Giella replace rule (8 states, four flag diacritics, cyclic
// away from the initial state) spun at full CPU with no output on a one-character
// input, because the only bound on non-consuming arcs was a count of cycle
// re-entries. A count does not bound a *tree*: with the default cap of five, the
// walk still enumerated 902,360 readings of `A` at a cap of three and roughly
// sixteen times that per further turn, all of them duplicates of the same two
// analyses.
//
// `flag_filter` reproduces that machine's flag skeleton: its thirty-two flag
// arcs over eight states, all final, with one `A:A` and one `A:a` reading
// underneath. The other tests pin the shape of the trap that now bounds it.
//
// The tropical transition-data symbol coding used by `HfstBasicTransducer` lives
// in process-global statics; cargo runs each #[test] as a parallel thread in one
// process, so construction is serialized here as it is in the sibling lookup
// test files.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use hfst::convert_transducer_format::ConversionFunctions;
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_data_types::{HfstOneLevelPaths, HfstTwoLevelPaths, StringVector};

const EPSILON: &str = "@_EPSILON_SYMBOL_@";

// The four flag diacritics of the machine that hung, renamed but not reshaped:
// two positive-set operations that always succeed, and two unifications that
// succeed against the value they themselves set. Every one of them is takeable
// on every turn of a loop, which is what makes the loops multiply.
const CAP: &str = "@U.Cap.Opt@";
const HYPH: &str = "@U.CmpHyph.TRUE@";
const ROOT: &str = "@P.LEXNAME.Root@";
const PROPER: &str = "@P.LEXNAME.ProperNoun@";

/// The flag skeleton of `downcase-derived_proper-strings.lookup.hfst`: every
/// flag arc it carries, including the self-loops at 4, 5 and 7 and the four-way
/// fan into 6 and 7.
const FLAG_ARCS: &[(u32, u32, &str)] = &[
    (0, 1, CAP),
    (0, 2, ROOT),
    (0, 5, HYPH),
    (0, 5, PROPER),
    (1, 3, PROPER),
    (1, 4, HYPH),
    (1, 6, CAP),
    (1, 6, ROOT),
    (2, 1, CAP),
    (2, 5, HYPH),
    (2, 5, ROOT),
    (2, 5, PROPER),
    (3, 4, HYPH),
    (3, 6, CAP),
    (3, 6, ROOT),
    (3, 6, PROPER),
    (4, 4, HYPH),
    (4, 6, CAP),
    (4, 6, ROOT),
    (4, 6, PROPER),
    (5, 5, HYPH),
    (5, 5, ROOT),
    (5, 5, PROPER),
    (5, 7, CAP),
    (6, 7, CAP),
    (6, 7, HYPH),
    (6, 7, ROOT),
    (6, 7, PROPER),
    (7, 7, CAP),
    (7, 7, HYPH),
    (7, 7, ROOT),
    (7, 7, PROPER),
];

/// Every arc of that machine that consumes an `A`: the downcasing relation the
/// rule expresses, laid over the same eight states. Its other sixty-seven
/// alphabet symbols per state pair cannot match this input and are left out —
/// what remains is the whole of the machine the probe walks. Each of these puts
/// a fresh flag closure underneath it, which is what lifts the closure's cost to
/// the whole lookup's cost.
const CONSUMING_ARCS: &[(u32, u32, &str)] = &[
    (0, 5, "A"),
    (1, 6, "a"),
    (2, 5, "A"),
    (3, 6, "a"),
    (4, 6, "a"),
    (5, 5, "A"),
    (6, 7, "a"),
    (7, 7, "A"),
];

static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn arc(net: &mut HfstBasicTransducer, from: u32, to: u32, i: &str, o: &str, w: f32) {
    net.add_state(from);
    net.add_state(to);
    let coder = net.coder_mut();
    let tr = HfstBasicTransition::new_symbols(to, i.into(), o.into(), w, coder);
    net.add_transition(from, &tr, true);
}

/// The machine that hung: eight states, all final, joined only by flag arcs,
/// with a downcasing reading of `A` underneath them.
fn flag_filter() -> HfstBasicTransducer {
    let mut net = HfstBasicTransducer::new();
    for (from, to, flag) in FLAG_ARCS {
        arc(&mut net, *from, *to, flag, flag, 0.0);
    }
    for (from, to, output) in CONSUMING_ARCS {
        arc(&mut net, *from, *to, "A", output, 0.0);
    }
    for state in 0..=7 {
        net.set_final_weight(state, &0.0);
    }
    net
}

/// Look `input` up through the general engine, obeying flags, with no result
/// cap and no weight cap — the caller-supplied limits are the only legitimate
/// way to curtail a walk, and this asks for the whole thing. `cycles` is the
/// caller's input-epsilon-cycle cap (`None` = the standing default).
fn lookup(net: &HfstBasicTransducer, input: &[&str], cycles: Option<usize>) -> HfstTwoLevelPaths {
    let path: StringVector = input.iter().map(|s| (*s).into()).collect();
    let mut results: HfstTwoLevelPaths = BTreeSet::new();
    net.lookup(&path, &mut results, cycles, None, -1, true);
    results
}

/// The analyses as a caller sees them: output side, flag and epsilon symbols
/// dropped, paired with the path weight, deduplicated. Two readings that differ
/// only in how many turns of a flag loop they took are one analysis here — the
/// engine printed 902,360 lines of these for a two-analysis answer.
fn analyses(results: &HfstTwoLevelPaths) -> BTreeSet<(String, String)> {
    results
        .iter()
        .map(|p| {
            let output: String = p
                .second
                .iter()
                .map(|(_, o)| o.as_str())
                .filter(|s| !s.starts_with('@'))
                .collect();
            (output, format!("{:.1}", p.first))
        })
        .collect()
}

/// The same lookup through the optimized-lookup engine, which bounds its own
/// walk the same way and is this engine's conformance target.
fn ol_analyses(net: &HfstBasicTransducer, input: &str) -> BTreeSet<(String, String)> {
    let mut ol = ConversionFunctions::hfst_basic_transducer_to_hfst_ol(net, true, "", None)
        .expect("fixture is well within the OL format limits");
    let paths: HfstOneLevelPaths = ol.lookup_fd_cstr(input, -1, 0.0);
    paths
        .into_iter()
        .map(|p| {
            let output: String = p
                .second
                .iter()
                .map(|s| s.as_str())
                .filter(|s| !s.starts_with('@'))
                .collect();
            (output, format!("{:.1}", p.first))
        })
        .collect()
}

/// A lookup that has to finish, with the room to say so when it does not: the
/// pre-fix engine ran for tens of minutes here, so any bound generous enough to
/// be quiet on a loaded machine still catches it.
fn timed<T>(what: &str, f: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let out = f();
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_secs(5),
        "{what} took {elapsed:?} — the walk is being bounded by something other \
         than the shape of its cycles"
    );
    out
}

// ---------------------------------------------------------------------------
// The trap.
// ---------------------------------------------------------------------------

/// The reported hang, in miniature. Four always-satisfiable flag diacritics over
/// eight states offer a non-consuming arc out of every situation the walk stands
/// in, so a cap on cycle re-entries leaves a tree of readings behind it that
/// grows about sixteenfold per permitted turn. The walk has to refuse a cycle
/// that consumed nothing instead of counting how many it has taken.
// [spec:hfst:req:general-lookup-termination.non-progressing-cycle/test]
#[test]
fn a_flag_gated_cycle_terminates_on_its_shape() {
    let _g = serialized();
    let net = flag_filter();

    let found = timed("the flag-cycle lookup", || lookup(&net, &["A"], Some(5)));

    assert_eq!(
        analyses(&found),
        BTreeSet::from([
            ("A".to_string(), "0.0".to_string()),
            ("a".to_string(), "0.0".to_string()),
        ]),
        "the downcase relation's two readings of A did not both survive the trap"
    );
}

/// The trap must not be reachable past the caller's cycle cap: termination is a
/// property of the machine's shape, so asking for a hundred thousand turns of a
/// cycle must cost no more than asking for five. Before the trap this argument
/// ran the other way — the cap was the only thing that ended the walk, and every
/// increment of it multiplied the work.
// [spec:hfst:req:general-lookup-termination.no-cycle-count-termination/test]
#[test]
fn the_cycle_cap_does_not_bound_the_walk() {
    let _g = serialized();
    let net = flag_filter();

    let five = timed("a five-cycle lookup", || lookup(&net, &["A"], Some(5)));
    let many = timed("a hundred-thousand-cycle lookup", || {
        lookup(&net, &["A"], Some(100_000))
    });
    let default = timed("an uncapped lookup", || lookup(&net, &["A"], None));

    assert_eq!(
        analyses(&five),
        analyses(&many),
        "the answer moved when the cycle cap did, so the cap is still curtailing \
         the walk rather than bounding enumeration"
    );
    assert_eq!(analyses(&default), analyses(&many));
}

/// An input-epsilon cycle that writes output offers unboundedly many readings of
/// one input. The walk takes the loop once — the second turn stands in the same
/// situation as the first — and stops, rather than riding it until a counter
/// runs out and reporting the count as the answer.
// [spec:hfst:req:general-lookup-termination.non-progressing-cycle/test]
#[test]
fn an_input_epsilon_cycle_terminates_on_its_shape() {
    let _g = serialized();
    let mut net = HfstBasicTransducer::new();
    arc(&mut net, 0, 1, "A", "A", 0.0);
    arc(&mut net, 1, 1, EPSILON, "x", 1.0);
    net.set_final_weight(1, &0.0);

    let found = timed("the epsilon-cycle lookup", || {
        lookup(&net, &["A"], Some(100_000))
    });
    let found = analyses(&found);

    assert!(
        found.len() <= 4,
        "an epsilon cycle produced a runaway result set: {found:?}"
    );
    assert!(
        found.contains(&("A".to_string(), "0.0".to_string())),
        "the reading that never took the loop is missing: {found:?}"
    );
    let heaviest = found
        .iter()
        .filter_map(|(_, w)| w.parse::<f32>().ok())
        .fold(0.0f32, f32::max);
    assert!(
        heaviest < 10.0,
        "the loop's weight climbed to {heaviest}, so the walk kept taking turns \
         of a cycle that consumed nothing"
    );
}

/// One relation, two engines: a machine walked as a transition graph and the
/// same machine walked as a packed optimized-lookup table have to answer the
/// same thing about a cyclic input. Before the trap they did not — the graph
/// reported one reading per permitted cycle and the table reported one turn —
/// and which of the two a caller got depended only on which file format they
/// had been handed.
// [spec:hfst:sem:general-lookup-termination.enumeration-divergence/test]
#[test]
fn both_engines_bound_a_cyclic_machine_alike() {
    let _g = serialized();
    let mut net = HfstBasicTransducer::new();
    arc(&mut net, 0, 1, "A", "A", 0.0);
    arc(&mut net, 1, 1, EPSILON, "x", 1.0);
    net.set_final_weight(1, &0.0);

    let general = analyses(&lookup(&net, &["A"], Some(5)));

    assert_eq!(
        general,
        BTreeSet::from([
            ("A".to_string(), "0.0".to_string()),
            ("Ax".to_string(), "1.0".to_string()),
        ]),
        "the loop was taken a number of times, rather than until it repeated a \
         situation"
    );
    assert_eq!(
        general,
        ol_analyses(&net, "A"),
        "the two engines disagreed about the same relation"
    );
}

/// Consuming an input symbol is progress, and progress makes every situation
/// reachable again: a machine whose only arc is a self-loop on `a` re-enters one
/// state per input symbol, and all three of them have to be walked. A trap that
/// outlived the arc that consumed input would answer nothing here.
// [spec:hfst:req:general-lookup-termination.progress-resets-the-trap/test]
#[test]
fn consuming_input_reopens_a_visited_state() {
    let _g = serialized();
    let mut net = HfstBasicTransducer::new();
    arc(&mut net, 0, 0, "a", "b", 1.0);
    net.set_final_weight(0, &0.0);

    let found = analyses(&lookup(&net, &["a", "a", "a"], Some(5)));

    assert_eq!(
        found,
        BTreeSet::from([("bbb".to_string(), "3.0".to_string())]),
        "a state re-entered after consuming input was treated as a cycle"
    );
}

/// A flag arc rewrites the registers, so the situation it lands in is not the
/// one it left even when the state repeats: `@U.Feat.One@` then `@U.Feat.Two@`
/// is a different walk from either alone, and unifying `Feat` twice is what
/// tells the two apart. The trap keys on the flag configuration for this reason;
/// keyed on the state alone it would cut the second flag off and lose the
/// reading that needs both.
// [spec:hfst:req:general-lookup-termination.non-progressing-cycle/test]
#[test]
fn changing_the_flag_registers_is_not_a_cycle() {
    let _g = serialized();
    let mut net = HfstBasicTransducer::new();
    // 0 -> 1 sets Feat=One; 1 -> 1 clears it; 1 -> 2 sets Feat=Two. Reaching 2
    // needs both flag arcs and the clear between them, all without consuming.
    arc(&mut net, 0, 1, "@U.Feat.One@", "@U.Feat.One@", 0.0);
    arc(&mut net, 1, 1, "@C.Feat@", "@C.Feat@", 0.0);
    arc(&mut net, 1, 2, "@U.Feat.Two@", "@U.Feat.Two@", 0.0);
    arc(&mut net, 2, 3, "a", "a", 0.0);
    net.set_final_weight(3, &0.0);

    let found = analyses(&lookup(&net, &["a"], Some(5)));

    assert_eq!(
        found,
        BTreeSet::from([("a".to_string(), "0.0".to_string())]),
        "the reading that needed a second flag arc through a repeated state was \
         trapped as a cycle"
    );
}
