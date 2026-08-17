//! Shared graph queries for the optimized-lookup backend implementations.

use crate::transducer::{
    Transducer, TransducerTablesInterface, TransitionTableIndex, TransitionTableIndexSet, Weight,
};

/// Walk the reachable states of an optimized-lookup table pair from the start
/// offset, handing each state index and its outgoing transition indices to
/// `visit`; stops early when `visit` answers false.
///
/// The OL encoding keeps no state list to read anything off: a state is just an
/// offset into the index or the transition table, and the two tables share one
/// address space. This is the walk `hfst_ol_to_hfst_basic_transducer` numbers
/// states with, minus the interchange transducer it would otherwise
/// materialize — so anything derived here matches the graph `to_basic` builds.
pub(super) fn ol_walk<T, F>(t: &Transducer<T>, mut visit: F)
where
    T: TransducerTablesInterface,
    F: FnMut(TransitionTableIndex, &TransitionTableIndexSet) -> bool,
{
    const START: TransitionTableIndex = 0;
    let mut seen = std::collections::BTreeSet::from([START]);
    let mut agenda = vec![START];
    while let Some(state) = agenda.pop() {
        let transitions = t.get_transitions_from_state(state);
        if !visit(state, &transitions) {
            return;
        }
        for tr in transitions.iter() {
            let target = t.get_transition_target(*tr);
            if seen.insert(target) {
                agenda.push(target);
            }
        }
    }
}

/// The reachable (state, arc) counts of an optimized-lookup table pair.
pub(super) fn ol_counts<T: TransducerTablesInterface>(t: &Transducer<T>) -> (u32, u32) {
    let mut states = 0u32;
    let mut arcs = 0u32;
    ol_walk(t, |_, transitions| {
        states += 1;
        arcs += transitions.len() as u32;
        true
    });
    (states, arcs)
}

/// The final weight of an optimized-lookup state index, 0.0 when non-final —
/// the two index-space arms of `hfst_ol_to_hfst_basic_add_state`.
pub(super) fn ol_final_weight<T: TransducerTablesInterface>(
    t: &Transducer<T>,
    state: TransitionTableIndex,
) -> Weight {
    if crate::transducer::indexes_transition_index_table(state) {
        if t.get_index_finality(state) {
            t.get_index_final_weight(state)
        } else {
            0.0
        }
    } else if t.get_transition_finality(state) {
        t.get_transition_weight(state)
    } else {
        0.0
    }
}

/// Whether any weight of an optimized-lookup table pair is non-zero.
///
/// Deliberately not `is_weighted()`, which reports the header's Weighted flag —
/// whether the tables are weighted-SHAPED. That is the right question for
/// `stream_type`'s OLW/OL tag and the wrong one here: conversions build
/// weighted-shaped tables even for logically unweighted material, so the flag
/// says true for nets whose every weight is 0.0, where the equivalent tropical
/// net says false. The flag is still the cheap negative: an unweighted-shaped
/// table reads every weight as 0.0, and `to_basic` zeroes them regardless.
pub(super) fn ol_has_weights<T: TransducerTablesInterface>(t: &Transducer<T>) -> bool {
    if !t.is_weighted() {
        return false;
    }
    let mut found = false;
    ol_walk(t, |state, transitions| {
        if ol_final_weight(t, state) != 0.0
            || transitions
                .iter()
                .any(|tr| t.get_transition_weight(*tr) != 0.0)
        {
            found = true;
        }
        !found
    });
    found
}
