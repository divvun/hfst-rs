//! Helpers shared by the foma_backend*.rs integration tests: the lock that
//! serializes the OpenFst-family tests, the special symbol names, and the
//! HfstBasicTransducer builders both backends are built from.

use hfst::backend::Backend;
use hfst::backend_foma::FomaTransducer;
use hfst::hfst_basic_transducer::HfstBasicTransducer;
use hfst::hfst_basic_transition::HfstBasicTransition;
use hfst::hfst_data_types::Symbol;
use hfst_openfst::StdVectorFst;

/// The tropical/OL symbol coding lives in process-global statics behind
/// their own mutexes; cargo runs every `#[test]` as a parallel thread in ONE
/// process, so tests touching the OpenFst family serialize through this lock to
/// restore the one-at-a-time-per-process model (mirrors test_streams.rs).
static SYMBOL_TABLE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub(super) fn serialized() -> std::sync::MutexGuard<'static, ()> {
    SYMBOL_TABLE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

pub(super) const EPSILON: &str = "@_EPSILON_SYMBOL_@";
pub(super) const UNKNOWN: &str = "@_UNKNOWN_SYMBOL_@";
pub(super) const IDENTITY: &str = "@_IDENTITY_SYMBOL_@";

pub(super) fn sym(s: &str) -> Symbol {
    Symbol::from(s)
}

// ---------------------------------------------------------------------------
// HfstBasicTransducer builders (the common parity source).
// ---------------------------------------------------------------------------

/// A transducer mapping the char sequence `inp` to `outp` (per-column aligned;
/// equal char counts required). `inp == outp` yields an acceptor.
pub(super) fn basic_pair(inp: &str, outp: &str) -> HfstBasicTransducer {
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

pub(super) fn basic_acceptor(word: &str) -> HfstBasicTransducer {
    basic_pair(word, word)
}

/// `{a,b,c}*` as a one-state acceptor with a self-loop per symbol.
pub(super) fn basic_sigma_star(symbols: &[&str]) -> HfstBasicTransducer {
    let mut net = HfstBasicTransducer::new();
    net.add_state(0);
    for s in symbols {
        let tr = HfstBasicTransition::new_symbols(0, sym(s), sym(s), 0.0, net.coder_mut());
        net.add_transition(0, &tr, true);
    }
    net.set_final_weight(0, &0.0);
    net
}

/// Build a net from an explicit arc list `(from, isym, osym, to)` and final
/// states, for shapes that branch and self-loop, which `basic_pair` cannot
/// express.
pub(super) fn basic_arcs(arcs: &[(u32, &str, &str, u32)], finals: &[u32]) -> HfstBasicTransducer {
    let mut n = HfstBasicTransducer::new();
    n.add_state(0);
    for (from, i, o, to) in arcs {
        let tr = HfstBasicTransition::new_symbols(*to, sym(i), sym(o), 0.0, n.coder_mut());
        n.add_transition(*from, &tr, true);
    }
    for f in finals {
        n.set_final_weight(*f, &0.0);
    }
    n
}

pub(super) fn foma_of(net: &HfstBasicTransducer) -> FomaTransducer {
    FomaTransducer::from_basic(net).expect("foma from_basic")
}

pub(super) fn tropical_of(net: &HfstBasicTransducer) -> StdVectorFst {
    <StdVectorFst as Backend>::from_basic(net).expect("tropical from_basic")
}

/// State count of a backend transducer, read off its interchange form — an
/// independent witness against the backend's own `number_of_states`.
pub(super) fn state_count<B: Backend>(b: &B) -> usize {
    let basic = b.to_basic().expect("to_basic");
    (basic.get_max_state() + 1) as usize
}

/// Arc count of a backend transducer, read off its interchange form — the
/// counterpart witness against `number_of_arcs`.
pub(super) fn arc_count<B: Backend>(b: &B) -> usize {
    let basic = b.to_basic().expect("to_basic");
    basic
        .states_and_transitions()
        .iter()
        .map(|trs| trs.len())
        .sum()
}
