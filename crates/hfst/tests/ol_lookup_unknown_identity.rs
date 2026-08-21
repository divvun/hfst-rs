// Optimized-lookup enumeration over an input symbol outside the alphabet.
//
// The engine is a depth-first walk of a packed transition table, so anything
// that stops the walk part-way answers from whatever the table order happened
// to reach first. The failure that motivated these tests: a Giella speller
// error model held both as a nondeterministic union (symbolic unknown/identity
// arcs) and as its determinized build (concrete arcs) returned different
// analysis sets for one probe word, and the determinized file reported a
// minimum weight three times the true one, because a whole-lookup node-visit
// ceiling truncated each walk at a different place.
//
// `haystack` builds that failure in miniature: one relation, two layouts, an
// exhaustive walk needed to reach the cheap reading. The other tests pin the
// unknown/identity contract the probe word exercises.
//
// The tropical transition-data symbol coding used by `HfstBasicTransducer`
// lives in process-global statics; cargo runs each #[test] as a parallel thread
// in one process, so construction is serialized here as it is in the sibling
// OL test files.

use hfst::convert_transducer_format::ConversionFunctions;
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_data_types::HfstOneLevelPaths;
use hfst::transducer::{Transducer, WeightedTables};

const EPSILON: &str = "@_EPSILON_SYMBOL_@";
const IDENTITY: &str = "@_IDENTITY_SYMBOL_@";
const UNKNOWN: &str = "@_UNKNOWN_SYMBOL_@";

/// Outside every fixture's alphabet, so it can only be consumed by an identity
/// or unknown arc.
const OUT_OF_ALPHABET: &str = "ä";

static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn arc(net: &mut HfstBasicTransducer, from: u32, to: u32, i: &str, o: &str, w: f32) {
    let coder = net.coder_mut();
    let tr = HfstBasicTransition::new_symbols(to, i.into(), o.into(), w, coder);
    net.add_transition(from, &tr, true);
}

fn to_ol(net: &HfstBasicTransducer) -> Transducer<WeightedTables> {
    ConversionFunctions::hfst_basic_transducer_to_hfst_ol(net, true, "", None)
        .expect("fixture is well within the OL format limits")
}

/// Analyses as `(output string, weight)`, with the engine given no result cap
/// and no time cutoff — the caller-supplied limits are the only legitimate way
/// to curtail a walk, and this asks for the whole thing.
fn analyses(net: &HfstBasicTransducer, input: &str) -> Vec<(String, f32)> {
    let paths: HfstOneLevelPaths = to_ol(net).lookup_fd_cstr(input, -1, 0.0);
    let mut out: Vec<(String, f32)> = paths
        .into_iter()
        .map(|p| (p.second.concat(), p.first))
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    out
}

/// The cheapest weight reported for `output`, or `None` when the walk never
/// reached it.
fn best(found: &[(String, f32)], output: &str) -> Option<f32> {
    found
        .iter()
        .filter(|(o, _)| o == output)
        .map(|(_, w)| *w)
        .min_by(f32::total_cmp)
}

// ---------------------------------------------------------------------------
// The unknown/identity contract.
// ---------------------------------------------------------------------------

/// An identity arc consumes a symbol the machine never declared and echoes it.
// [spec:hfst:req:ol-lookup-enumeration.out-of-alphabet-input/test]
#[test]
fn identity_arc_echoes_an_out_of_alphabet_symbol() {
    let _g = serialized();
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    arc(&mut net, 0, 1, "a", "b", 0.0);
    arc(&mut net, 0, 1, IDENTITY, IDENTITY, 0.25);
    net.set_final_weight(1, &0.0);

    let found = analyses(&net, OUT_OF_ALPHABET);
    assert_eq!(
        found,
        vec![(OUT_OF_ALPHABET.to_string(), 0.25)],
        "an out-of-alphabet symbol must ride the identity arc and come back \
         echoed, carrying the arc's weight"
    );
}

/// An unknown-output arc writes the symbol just consumed, not the placeholder.
/// This is the whole of the optimized-lookup engine's narrower reading of
/// unknown outputs, and the reason its results never carry the token as text.
// [spec:hfst:req:ol-lookup-enumeration.meta-arc-output/test]
// [spec:hfst:sem:ol-lookup-enumeration.meta-arc-restriction/test]
#[test]
fn meta_output_is_instantiated_from_the_input_tape() {
    let _g = serialized();
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    arc(&mut net, 0, 1, UNKNOWN, UNKNOWN, 0.0);
    net.set_final_weight(1, &0.0);

    let found = analyses(&net, OUT_OF_ALPHABET);
    assert_eq!(
        found,
        vec![(OUT_OF_ALPHABET.to_string(), 0.0)],
        "an unknown:unknown arc must write the consumed input symbol"
    );
    assert!(
        found
            .iter()
            .all(|(o, _)| !o.contains(UNKNOWN) && !o.contains(IDENTITY)),
        "a lookup result must never carry a meta symbol as literal output text: \
         {found:?}"
    );
}

/// Identity and unknown are tried independently: a machine offering both
/// readings of one position must return both, not whichever comes first.
// [spec:hfst:req:ol-lookup-enumeration.out-of-alphabet-input/test]
#[test]
fn identity_and_unknown_arcs_are_both_attempted() {
    let _g = serialized();
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    arc(&mut net, 0, 1, IDENTITY, IDENTITY, 1.0);
    arc(&mut net, 0, 2, UNKNOWN, UNKNOWN, 2.0);
    net.set_final_weight(1, &0.0);
    net.set_final_weight(2, &0.0);

    let found = analyses(&net, OUT_OF_ALPHABET);
    assert_eq!(
        found,
        vec![
            (OUT_OF_ALPHABET.to_string(), 1.0),
            (OUT_OF_ALPHABET.to_string(), 2.0),
        ],
        "both the identity and the unknown reading must be enumerated"
    );
}

// ---------------------------------------------------------------------------
// Exhaustiveness: one relation, two layouts.
// ---------------------------------------------------------------------------

/// Arcs per haystack state, and the depth of the haystack chain. 12^6 dead-end
/// paths is upwards of three million node visits before the walk can reach the
/// arcs that follow — comfortably past any ceiling an implementation might be
/// tempted to impose, while costing no allocation, since not one haystack path
/// reaches a final state.
const FANOUT: u32 = 12;
const DEPTH: u32 = 6;

/// The probe: `DEPTH` in-alphabet symbols followed by one the machine never
/// declared, so the last step of a surviving path must be an identity arc.
fn probe() -> String {
    format!("{}{OUT_OF_ALPHABET}", "a".repeat(DEPTH as usize))
}

/// A chain reading `probe()` from state `base`: `DEPTH` steps spelling `output`
/// followed by an identity arc carrying `weight`, which is what consumes the
/// out-of-alphabet symbol.
fn reading(net: &mut HfstBasicTransducer, base: u32, output: &str, weight: f32) {
    for step in 0..DEPTH {
        arc(net, base + step, base + step + 1, "a", output, 0.0);
    }
    arc(
        net,
        base + DEPTH,
        base + DEPTH + 1,
        IDENTITY,
        IDENTITY,
        weight,
    );
    net.set_final_weight(base + DEPTH + 1, &0.0);
}

/// One relation over `probe()`, in two layouts.
///
/// Both denote `(probe, "QQQQQQä") @ 9.0` and `(probe, "zzzzzzä") @ 0.5`
/// alongside a mass of dead ends. The layouts differ only in where the cheap
/// reading sits in table order. Entered on an epsilon arc it is walked before
/// any input symbol is consumed, so it is reached immediately; entered on a
/// plain `a` arc inserted after the dead ends' `a` arcs, the walk reaches it
/// only after exhausting them, since arcs sharing an input symbol stay in
/// insertion order through the packing. Both are ordinary ways to hold this
/// relation, and a lookup answer must not turn on which one a caller has.
fn haystack(cheap_first: bool) -> HfstBasicTransducer {
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);

    // The expensive reading, entered on an epsilon arc in both layouts.
    let decoy_base = 1;
    reading(&mut net, decoy_base, "Q", 9.0);
    arc(&mut net, 0, decoy_base, EPSILON, EPSILON, 0.0);

    let cheap_base = decoy_base + DEPTH + 2;
    reading(&mut net, cheap_base, "z", 0.5);
    if cheap_first {
        arc(&mut net, 0, cheap_base, EPSILON, EPSILON, 0.0);
    }

    // Dead ends: `FANOUT` ways to spell each of `DEPTH` steps, arriving at a
    // state with no arc for the final out-of-alphabet symbol. The first step
    // leaves state 0, so these arcs share the input symbol the cheap reading's
    // entry arc uses in the `cheap_first == false` layout.
    let dead_end_base = cheap_base + DEPTH + 2;
    for k in 0..FANOUT {
        arc(&mut net, 0, dead_end_base, "a", &format!("A{k:02}"), 0.0);
    }
    for level in 0..DEPTH - 1 {
        for k in 0..FANOUT {
            arc(
                &mut net,
                dead_end_base + level,
                dead_end_base + level + 1,
                "a",
                &format!("A{k:02}"),
                0.0,
            );
        }
    }

    if !cheap_first {
        // Entering one step in: the entry arc itself spells the chain's first
        // step, so the relation is unchanged.
        arc(&mut net, 0, cheap_base + 1, "a", "z", 0.0);
    }
    net
}

/// The layout that walks the cheap reading last must still find it, and must
/// report the same analyses as the layout that walks it first. A walk curtailed
/// on internal accounting returns only the expensive reading here — a minimum
/// weight eighteen times the true one, with nothing to say it is not the whole
/// answer.
// [spec:hfst:req:ol-lookup-enumeration.no-internal-work-cap/test]
// [spec:hfst:req:ol-lookup-enumeration.representation-independence/test]
#[test]
fn exhaustive_walk_reaches_the_cheap_reading() {
    let _g = serialized();
    let probe = probe();
    let cheap_output = format!("{}{OUT_OF_ALPHABET}", "z".repeat(DEPTH as usize));
    let decoy_output = format!("{}{OUT_OF_ALPHABET}", "Q".repeat(DEPTH as usize));

    let first = analyses(&haystack(true), &probe);
    let last = analyses(&haystack(false), &probe);

    for (label, found) in [("cheap-first", &first), ("cheap-last", &last)] {
        assert_eq!(
            best(found, &decoy_output),
            Some(9.0),
            "{label}: the expensive reading is missing: {found:?}"
        );
        assert_eq!(
            best(found, &cheap_output),
            Some(0.5),
            "{label}: the cheap reading was not reached — the walk stopped short \
             of it: {found:?}"
        );
    }

    assert_eq!(
        first, last,
        "the same relation laid out two ways answered differently"
    );
}

/// With no work cap left to stop it, an input-epsilon cycle has to be stopped by
/// its shape: `(0:a)*` offers unboundedly many analyses of the empty input, and
/// the walk must refuse the second turn of a loop that consumed nothing rather
/// than ride it to the recursion-depth cap, piling up thousands of readings
/// whose weights climb with the turn count.
// [spec:hfst:req:ol-lookup-enumeration.no-internal-work-cap/test]
#[test]
fn an_input_epsilon_cycle_terminates_on_its_shape() {
    let _g = serialized();
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    arc(&mut net, 0, 0, EPSILON, "a", 1.0);
    net.set_final_weight(0, &0.0);

    let started = std::time::Instant::now();
    let found = analyses(&net, "");
    let elapsed = started.elapsed();

    assert!(
        found.len() <= 20,
        "an epsilon cycle produced a runaway result set: {found:?}"
    );
    let heaviest = found.iter().map(|(_, w)| *w).fold(0.0f32, f32::max);
    assert!(
        heaviest < 100.0,
        "the epsilon loop's weight climbed to {heaviest}, so the walk kept \
         taking turns of a loop that consumed nothing"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(5),
        "an epsilon-cyclic lookup took {elapsed:?}"
    );
}
