// The machine/run split of the optimized-lookup engine.
//
// The loaded transducer used to carry the tapes, flag state, traversal
// bookkeeping and accumulated results the walk writes, so every lookup took the
// whole object exclusively and an out-of-alphabet input rewrote the alphabet it
// was being looked up in. These tests pin what the split has to preserve: a run
// sees only its own scratch, an admitted symbol never reaches the machine, and
// the machine can be traversed from several threads at once.
//
// The tropical transition-data symbol coding used by `HfstBasicTransducer`
// lives in process-global statics; cargo runs each #[test] as a parallel thread
// in one process, so construction is serialized here as it is in the sibling
// OL test files.

use hfst::convert_transducer_format::ConversionFunctions;
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::transducer::{Transducer, UnweightedTables, WeightedTables};

const IDENTITY: &str = "@_IDENTITY_SYMBOL_@";

/// Two symbols outside the fixture's alphabet, so each can only be consumed by
/// the identity arc — and only after a run has admitted it.
const FIRST_UNKNOWN: &str = "ä";
const SECOND_UNKNOWN: &str = "ö";

static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn arc(net: &mut HfstBasicTransducer, from: u32, to: u32, i: &str, o: &str, w: f32) {
    let coder = net.coder_mut();
    let tr = HfstBasicTransition::new_symbols(to, i.into(), o.into(), w, coder);
    net.add_transition(from, &tr, true);
}

/// Accepts one or two symbols, each either the declared `a` or anything at all
/// through an identity arc, so an out-of-alphabet symbol is analysable at every
/// position and carries a weight that says which arc took it.
fn fixture() -> HfstBasicTransducer {
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    arc(&mut net, 0, 1, "a", "a", 0.0);
    arc(&mut net, 0, 1, IDENTITY, IDENTITY, 0.5);
    arc(&mut net, 1, 2, "a", "a", 0.0);
    arc(&mut net, 1, 2, IDENTITY, IDENTITY, 0.25);
    net.set_final_weight(1, &0.0);
    net.set_final_weight(2, &0.0);
    net
}

fn to_ol(net: &HfstBasicTransducer) -> Transducer<WeightedTables> {
    ConversionFunctions::hfst_basic_transducer_to_hfst_ol(net, true, "", None)
        .expect("fixture is well within the OL format limits")
}

/// Analyses as `(output string, weight)`, ordered so two runs are comparable.
fn analyses(paths: hfst::hfst_data_types::HfstOneLevelPaths) -> Vec<(String, f32)> {
    let mut out: Vec<(String, f32)> = paths
        .into_iter()
        .map(|p| (p.second.concat(), p.first))
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    out
}

/// Words that each admit a different out-of-alphabet symbol, so a run state
/// that leaked would be handing another run a symbol it never saw.
fn first_words() -> Vec<String> {
    vec![
        "a".to_string(),
        FIRST_UNKNOWN.to_string(),
        format!("a{FIRST_UNKNOWN}"),
        format!("{FIRST_UNKNOWN}{SECOND_UNKNOWN}"),
    ]
}

fn second_words() -> Vec<String> {
    vec![
        SECOND_UNKNOWN.to_string(),
        format!("{SECOND_UNKNOWN}a"),
        "aa".to_string(),
        format!("{SECOND_UNKNOWN}{FIRST_UNKNOWN}"),
    ]
}

/// One machine serving two interleaved run states must answer exactly as two
/// privately loaded machines would.
// [spec:hfst:req:lookup-run-state.caller-owned-scratch/test]
#[test]
fn two_states_over_one_machine_stay_isolated() {
    let _g = serialized();
    let net = fixture();
    let shared = to_ol(&net);
    let alone_first = to_ol(&net);
    let alone_second = to_ol(&net);

    let mut state_first = shared.lookup_state();
    let mut state_second = shared.lookup_state();
    let mut private_first = alone_first.lookup_state();
    let mut private_second = alone_second.lookup_state();

    for (first, second) in first_words().iter().zip(second_words().iter()) {
        let shared_first = analyses(state_first.lookup_fd(first, -1, 0.0));
        let shared_second = analyses(state_second.lookup_fd(second, -1, 0.0));
        assert_eq!(
            shared_first,
            analyses(private_first.lookup_fd(first, -1, 0.0)),
            "sharing a machine changed the analyses of {first:?}"
        );
        assert_eq!(
            shared_second,
            analyses(private_second.lookup_fd(second, -1, 0.0)),
            "sharing a machine changed the analyses of {second:?}"
        );
    }
}

/// A run that admits an out-of-alphabet symbol must echo it back as the old
/// alphabet-growing implementation did, and leave the machine as it found it —
/// the alphabet it was looked up in is the alphabet the next caller gets.
// [spec:hfst:req:lookup-run-state.out-of-alphabet-overlay/test]
#[test]
fn admitting_a_symbol_leaves_the_machine_unchanged() {
    let _g = serialized();
    let machine = to_ol(&fixture());
    let symbols_before = machine.get_symbol_table().len();

    let mut state = machine.lookup_state();
    for word in first_words().iter().chain(second_words().iter()) {
        let found = analyses(state.lookup_fd(word, -1, 0.0));
        assert!(
            found.iter().any(|(o, _)| o == word),
            "{word:?} was not echoed back by the identity arcs: {found:?}"
        );
    }

    assert_eq!(
        machine.get_symbol_table().len(),
        symbols_before,
        "a lookup grew the machine's alphabet"
    );
    assert!(
        !machine.can_tokenize(FIRST_UNKNOWN),
        "a lookup taught the machine's encoder a symbol it never declared"
    );
}

/// An overlay outlives the lookup that built it, so a reused state meets its
/// own admitted symbols again — within one input and across inputs. It must
/// recognize them rather than admit them a second time under a fresh number,
/// and it must answer as a state seeing the word for the first time does.
// [spec:hfst:req:lookup-run-state.out-of-alphabet-overlay/test]
#[test]
fn a_reused_state_admits_a_symbol_once() {
    let _g = serialized();
    let machine = to_ol(&fixture());
    let doubled = format!("{FIRST_UNKNOWN}{FIRST_UNKNOWN}");

    let mut reused = machine.lookup_state();
    for word in first_words() {
        reused.lookup_fd(&word, -1, 0.0);
    }
    let second_time = analyses(reused.lookup_fd(&doubled, -1, 0.0));

    assert_eq!(
        second_time,
        analyses(machine.lookup_state().lookup_fd(&doubled, -1, 0.0)),
        "a state that had already admitted the symbol answered differently"
    );
    assert_eq!(
        second_time,
        analyses(reused.lookup_fd(&doubled, -1, 0.0)),
        "looking the same word up twice through one state changed the answer"
    );
}

fn assert_send_sync<T: Send + Sync>() {}

/// The loaded machine has to cross a thread boundary for any of this to be
/// worth doing.
// [spec:hfst:req:lookup-run-state.immutable-core/test]
#[test]
fn the_loaded_machine_is_send_and_sync() {
    assert_send_sync::<Transducer<WeightedTables>>();
    assert_send_sync::<Transducer<UnweightedTables>>();
}

/// Several runs over one shared machine, with no lock around the traversal,
/// answer as the same inputs do run one after another.
// [spec:hfst:req:lookup-run-state.immutable-core/test]
#[test]
fn concurrent_lookups_share_one_loaded_machine() {
    let _g = serialized();
    let machine = to_ol(&fixture());

    let expected: Vec<Vec<(String, f32)>> = first_words()
        .iter()
        .chain(second_words().iter())
        .map(|w| analyses(machine.lookup_state().lookup_fd(w, -1, 0.0)))
        .collect();

    let words: Vec<String> = first_words().into_iter().chain(second_words()).collect();
    let machine = &machine;
    let found: Vec<Vec<(String, f32)>> = std::thread::scope(|scope| {
        let handles: Vec<_> = words
            .iter()
            .map(|word| {
                scope.spawn(move || analyses(machine.lookup_state().lookup_fd(word, -1, 0.0)))
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("lookup thread did not panic"))
            .collect()
    });

    assert_eq!(found, expected, "a concurrent lookup answered differently");
}
