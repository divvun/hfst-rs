//! Backend implementations and shared graph queries for optimized lookup.

use crate::backend::Backend;
use crate::convert_transducer_format::ConversionFunctions;
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_data_types::ImplementationType;
use crate::hfst_extract_strings::ExtractStringsCb;
use crate::hfst_ol_transducer::HfstOlTransducer;
use crate::hfst_symbol_defs::StringSet;
use crate::transducer::{
    Transducer, TransducerTablesInterface, TransitionTableIndex, TransitionTableIndexSet,
    UnweightedTables, Weight, WeightedTables,
};

impl Backend for Transducer<WeightedTables> {
    const TYPE: ImplementationType = ImplementationType::HFST_OLW_TYPE;

    fn stream_type(&self) -> ImplementationType {
        if self.is_weighted() {
            ImplementationType::HFST_OLW_TYPE
        } else {
            ImplementationType::HFST_OL_TYPE
        }
    }

    fn write(&self, os: &mut dyn std::io::Write, _hfst_format: bool) -> crate::error::Result<()> {
        Transducer::write(self, os);
        Ok(())
    }

    fn empty() -> Self {
        Transducer::new_empty()
    }

    fn copy(&self) -> crate::error::Result<Self> {
        Transducer::copy(self)
    }

    fn to_basic(&self) -> crate::error::Result<HfstBasicTransducer> {
        Ok(ConversionFunctions::hfst_ol_to_hfst_basic_transducer(self))
    }

    fn from_basic(net: &HfstBasicTransducer) -> crate::error::Result<Self> {
        ConversionFunctions::hfst_basic_transducer_to_hfst_ol(net, true, "", None)
    }

    fn get_alphabet(&self) -> StringSet {
        HfstOlTransducer::get_alphabet(self)
    }

    fn is_cyclic(&self) -> bool {
        HfstOlTransducer::is_cyclic(self)
    }

    fn number_of_states(&self) -> u32 {
        ol_counts(self).0
    }

    fn number_of_arcs(&self) -> u32 {
        ol_counts(self).1
    }

    fn has_weights(&self) -> bool {
        ol_has_weights(self)
    }

    fn insert_to_alphabet(&mut self, symbol: &str) -> crate::error::Result<()> {
        self.include_symbol_in_alphabet(symbol);
        Ok(())
    }

    fn is_infinitely_ambiguous(&self) -> crate::error::Result<bool> {
        Ok(Transducer::is_infinitely_ambiguous(self))
    }

    fn extract_paths_cb(&self, callback: &mut dyn ExtractStringsCb, cycles: i32) {
        HfstOlTransducer::extract_paths(self, callback, cycles, None, false);
    }

    fn extract_paths_fd_cb(
        &self,
        callback: &mut dyn ExtractStringsCb,
        cycles: i32,
        filter_fd: bool,
    ) {
        let flag_diacritics = HfstOlTransducer::get_flag_diacritics(self);
        HfstOlTransducer::extract_paths(self, callback, cycles, Some(flag_diacritics), filter_fd);
    }
}

impl Backend for Transducer<UnweightedTables> {
    const TYPE: ImplementationType = ImplementationType::HFST_OL_TYPE;

    fn write(&self, os: &mut dyn std::io::Write, _hfst_format: bool) -> crate::error::Result<()> {
        Transducer::write(self, os);
        Ok(())
    }

    fn empty() -> Self {
        Transducer::new_empty()
    }

    fn copy(&self) -> crate::error::Result<Self> {
        Transducer::copy(self)
    }

    fn to_basic(&self) -> crate::error::Result<HfstBasicTransducer> {
        Ok(ConversionFunctions::hfst_ol_to_hfst_basic_transducer(self))
    }

    fn from_basic(_net: &HfstBasicTransducer) -> crate::error::Result<Self> {
        crate::bail!(
            Fatal,
            "from_basic: HFST_OL conversions produce weighted-shaped tables; \
             build Transducer<WeightedTables> instead"
        )
    }

    fn get_alphabet(&self) -> StringSet {
        HfstOlTransducer::get_alphabet(self)
    }

    fn is_cyclic(&self) -> bool {
        HfstOlTransducer::is_cyclic(self)
    }

    fn number_of_states(&self) -> u32 {
        ol_counts(self).0
    }

    fn number_of_arcs(&self) -> u32 {
        ol_counts(self).1
    }

    fn has_weights(&self) -> bool {
        ol_has_weights(self)
    }

    fn insert_to_alphabet(&mut self, symbol: &str) -> crate::error::Result<()> {
        self.include_symbol_in_alphabet(symbol);
        Ok(())
    }

    fn is_infinitely_ambiguous(&self) -> crate::error::Result<bool> {
        Ok(Transducer::is_infinitely_ambiguous(self))
    }

    fn extract_paths_cb(&self, callback: &mut dyn ExtractStringsCb, cycles: i32) {
        HfstOlTransducer::extract_paths(self, callback, cycles, None, false);
    }

    fn extract_paths_fd_cb(
        &self,
        callback: &mut dyn ExtractStringsCb,
        cycles: i32,
        filter_fd: bool,
    ) {
        let flag_diacritics = HfstOlTransducer::get_flag_diacritics(self);
        HfstOlTransducer::extract_paths(self, callback, cycles, Some(flag_diacritics), filter_fd);
    }
}

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
