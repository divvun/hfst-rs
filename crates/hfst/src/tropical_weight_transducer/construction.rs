//! Transducer factories, weight edits, and state and arc accessors.

use std::sync::Arc;

use hfst_openfst::rustfst::fst_properties::{FstProperties, compute_fst_properties};

use super::*;

// ---------------------------------------------------------------------------
// File-static globals from the .cc
// ---------------------------------------------------------------------------

// 'float tropical_seconds = 0;' — only ever non-zero under PROFILE_OPENFST,
// which is compiled out here.
// [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-profile-seconds-fn]
// [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-profile-seconds-fn]
// (the getter is an associated fn on TropicalWeightTransducer below)

// 'std::ostream * TropicalWeightTransducer::warning_stream = NULL;'

impl TropicalWeightTransducer {
    // ---- profiling / warning-stream globals ----

    pub fn get_profile_seconds() -> f32 {
        // 'tropical_seconds' is 0 unless PROFILE_OPENFST (compiled out).
        0.0
    }

    // ---- private symbol-table helpers ----

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.create-symbol-table-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.create-symbol-table-fn]
    pub(super) fn create_symbol_table(_name: String) -> SymbolTable {
        // rustfst 'SymbolTable' has no name; the 'name' arg is dropped. Start
        // from 'empty()' so the internal symbols land at exactly 0/1/2 (an
        // 'add_symbol' on a fresh 'new()' table would already hold <eps> at 0).
        let mut st = SymbolTable::empty();
        st.add_symbol(internal_epsilon); // 0
        st.add_symbol(internal_unknown); // 1
        st.add_symbol(internal_identity); // 2
        st
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.initialize-symbol-tables-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.initialize-symbol-tables-fn]
    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.initialize-symbol-tables-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.initialize-symbol-tables-fn]
    fn initialize_symbol_tables(t: &mut StdVectorFst) {
        let st = Self::create_symbol_table(String::new());
        t.set_input_symbols(Arc::new(st));
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.remove-symbol-table-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.remove-symbol-table-fn]
    fn remove_symbol_table(t: &mut StdVectorFst) {
        let _ = t.take_input_symbols();
    }

    // ---- factories ----

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.create-empty-transducer-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.create-empty-transducer-fn]
    pub fn create_empty_transducer() -> StdVectorFst {
        let mut t = StdVectorFst::new();
        Self::initialize_symbol_tables(&mut t);
        let s = t.add_state();
        t.set_start(s)
            .expect("start state just created by add_state");
        t
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.create-epsilon-transducer-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.create-epsilon-transducer-fn]
    pub fn create_epsilon_transducer() -> StdVectorFst {
        let mut t = StdVectorFst::new();
        Self::initialize_symbol_tables(&mut t);
        let s = t.add_state();
        t.set_start(s)
            .expect("start state just created by add_state");
        t.set_final(s, 0.0f32)
            .expect("state just created by add_state");
        t
    }

    // ---- string versions of define_transducer ----

    pub fn define_transducer_symbol(symbol: &str) -> StdVectorFst {
        assert!(!symbol.is_empty());
        let mut t = StdVectorFst::new();
        let mut st = Self::create_symbol_table(String::new());
        let s1 = t.add_state();
        let s2 = t.add_state();
        t.set_start(s1)
            .expect("start state just created by add_state");
        t.set_final(s2, 0.0f32)
            .expect("state just created by add_state");
        let il = st.add_symbol(symbol);
        let ol = st.add_symbol(symbol);
        t.add_tr(s1, StdTransition::new(il, ol, 0.0f32, s2))
            .expect("transition added between states created above");
        t.set_input_symbols(Arc::new(st));
        t
    }

    pub fn define_transducer_symbol_pair(isymbol: &str, osymbol: &str) -> StdVectorFst {
        assert!(!isymbol.is_empty());
        assert!(!osymbol.is_empty());
        let mut t = StdVectorFst::new();
        let mut st = Self::create_symbol_table(String::new());
        let s1 = t.add_state();
        let s2 = t.add_state();
        t.set_start(s1)
            .expect("start state just created by add_state");
        t.set_final(s2, 0.0f32)
            .expect("state just created by add_state");
        let il = st.add_symbol(isymbol);
        let ol = st.add_symbol(osymbol);
        t.add_tr(s1, StdTransition::new(il, ol, 0.0f32, s2))
            .expect("transition added between states created above");
        t.set_input_symbols(Arc::new(st));
        t
    }

    pub fn define_transducer_spv(spv: &StringPairVector) -> StdVectorFst {
        let mut t = StdVectorFst::new();
        let mut st = Self::create_symbol_table(String::new());
        let mut s1 = t.add_state();
        t.set_start(s1)
            .expect("start state just created by add_state");
        for it in spv {
            let s2 = t.add_state();
            assert!(!it.0.is_empty());
            assert!(!it.1.is_empty());
            let il = st.add_symbol(it.0.as_str());
            let ol = st.add_symbol(it.1.as_str());
            t.add_tr(s1, StdTransition::new(il, ol, 0.0f32, s2))
                .expect("transition added between states created above");
            s1 = s2;
        }
        t.set_final(s1, 0.0f32)
            .expect("state just created by add_state");
        t.set_input_symbols(Arc::new(st));
        t
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.define-transducer-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.define-transducer-fn]
    pub fn define_transducer_sps(sps: &StringPairSet, cyclic: bool) -> StdVectorFst {
        let mut t = StdVectorFst::new();
        let mut st = Self::create_symbol_table(String::new());
        let s1 = t.add_state(); // start state
        t.set_start(s1)
            .expect("start state just created by add_state");
        let mut s2 = s1; // final state
        if !sps.is_empty() {
            if !cyclic {
                s2 = t.add_state();
            }
            for it in sps {
                assert!(!it.0.is_empty());
                assert!(!it.1.is_empty());
                let il = st.add_symbol(it.0.as_str());
                let ol = st.add_symbol(it.1.as_str());
                t.add_tr(s1, StdTransition::new(il, ol, 0.0f32, s2))
                    .expect("transition added between states created above");
            }
        }
        t.set_final(s2, 0.0f32)
            .expect("state just created by add_state");
        t.set_input_symbols(Arc::new(st));
        t
    }

    pub fn define_transducer_spsv(spsv: &[StringPairSet]) -> StdVectorFst {
        let mut t = StdVectorFst::new();
        let mut st = Self::create_symbol_table(String::new());
        let mut s1 = t.add_state();
        t.set_start(s1)
            .expect("start state just created by add_state");
        for spset in spsv {
            let s2 = t.add_state();
            for it2 in spset {
                assert!(!it2.0.is_empty());
                assert!(!it2.1.is_empty());
                let il = st.add_symbol(it2.0.as_str());
                let ol = st.add_symbol(it2.1.as_str());
                t.add_tr(s1, StdTransition::new(il, ol, 0.0f32, s2))
                    .expect("transition added between states created above");
            }
            s1 = s2;
        }
        t.set_final(s1, 0.0f32)
            .expect("state just created by add_state");
        t.set_input_symbols(Arc::new(st));
        t
    }

    // ---- number versions of define_transducer (no symbol table, per C++) ----

    pub fn define_transducer_number(number: u32) -> StdVectorFst {
        let mut t = StdVectorFst::new();
        Self::initialize_symbol_tables(&mut t);
        let s1 = t.add_state();
        let s2 = t.add_state();
        t.set_start(s1)
            .expect("start state just created by add_state");
        t.set_final(s2, 0.0f32)
            .expect("state just created by add_state");
        t.add_tr(s1, StdTransition::new(number, number, 0.0f32, s2))
            .expect("transition added between states created above");
        t
    }

    pub fn define_transducer_number_pair(inumber: u32, onumber: u32) -> StdVectorFst {
        let mut t = StdVectorFst::new();
        Self::initialize_symbol_tables(&mut t);
        let s1 = t.add_state();
        let s2 = t.add_state();
        t.set_start(s1)
            .expect("start state just created by add_state");
        t.set_final(s2, 0.0f32)
            .expect("state just created by add_state");
        t.add_tr(s1, StdTransition::new(inumber, onumber, 0.0f32, s2))
            .expect("transition added between states created above");
        t
    }

    pub fn define_transducer_npv(npv: &NumberPairVector) -> StdVectorFst {
        let mut t = StdVectorFst::new();
        let mut s1 = t.add_state();
        t.set_start(s1)
            .expect("start state just created by add_state");
        for it in npv {
            let s2 = t.add_state();
            t.add_tr(s1, StdTransition::new(it.0, it.1, 0.0f32, s2))
                .expect("transition added between states created above");
            s1 = s2;
        }
        t.set_final(s1, 0.0f32)
            .expect("state just created by add_state");
        t
    }

    pub fn define_transducer_nps(nps: &NumberPairSet, cyclic: bool) -> StdVectorFst {
        let mut t = StdVectorFst::new();
        let s1 = t.add_state(); // start state
        t.set_start(s1)
            .expect("start state just created by add_state");
        let mut s2 = s1; // final state
        if !nps.is_empty() {
            if !cyclic {
                s2 = t.add_state();
            }
            for it in nps {
                t.add_tr(s1, StdTransition::new(it.0, it.1, 0.0f32, s2))
                    .expect("transition added between states created above");
            }
        }
        t.set_final(s2, 0.0f32)
            .expect("state just created by add_state");
        t
    }

    pub fn define_transducer_npsv(npsv: &[NumberPairSet]) -> StdVectorFst {
        let mut t = StdVectorFst::new();
        let mut s1 = t.add_state();
        t.set_start(s1)
            .expect("start state just created by add_state");
        for npset in npsv {
            let s2 = t.add_state();
            for it2 in npset {
                t.add_tr(s1, StdTransition::new(it2.0, it2.1, 0.0f32, s2))
                    .expect("transition added between states created above");
            }
            s1 = s2;
        }
        t.set_final(s1, 0.0f32)
            .expect("state just created by add_state");
        t
    }

    // ---- weight properties / setters ----

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-to-weights-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-to-weights-fn]
    pub fn add_to_weights(t: &mut StdVectorFst, w: f32) {
        let states: Vec<StateId> = t.states_iter().collect();
        for s in states {
            // (no in-place arc mutator in rustfst — pop & re-add, order preserved)
            let trs = t.pop_trs(s).expect("s is a valid state of this fst");
            for arc in trs {
                let nw = *arc.weight.value() + w;
                t.add_tr(
                    s,
                    StdTransition::new(arc.ilabel, arc.olabel, nw, arc.nextstate),
                )
                .expect("transition re-added to a state of this fst");
            }
            if t.is_final(s).expect("s is a valid state of this fst") {
                let old_weight = *t
                    .final_weight(s)
                    .expect("s is a valid state of this fst")
                    .expect("state confirmed final via is_final")
                    .value();
                t.set_final(s, old_weight + w)
                    .expect("s is a valid state of this fst");
            }
        }
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-smallest-weight-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-smallest-weight-fn]
    pub fn get_smallest_weight(t: &StdVectorFst) -> f32 {
        let mut retval = f32::INFINITY;
        for s in t.states_iter() {
            for arc in t.get_trs(s).expect("s is a valid state of this fst").trs() {
                let w = *arc.weight.value();
                if w < retval {
                    retval = w;
                }
            }
            if t.is_final(s).expect("s is a valid state of this fst") {
                let w = *t
                    .final_weight(s)
                    .expect("s is a valid state of this fst")
                    .expect("state confirmed final via is_final")
                    .value();
                if w < retval {
                    retval = w;
                }
            }
        }
        retval
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.has-weights-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.has-weights-fn]
    pub fn has_weights(t: &StdVectorFst) -> bool {
        for s in t.states_iter() {
            for arc in t.get_trs(s).expect("s is a valid state of this fst").trs() {
                if *arc.weight.value() != 0.0 {
                    return true;
                }
            }
            if t.is_final(s).expect("s is a valid state of this fst")
                && *t
                    .final_weight(s)
                    .expect("s is a valid state of this fst")
                    .expect("state confirmed final via is_final")
                    .value()
                    != 0.0
            {
                return true;
            }
        }
        false
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-final-weights-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-final-weights-fn]
    pub fn set_final_weights(t: &StdVectorFst, weight: f32, increment: bool) -> StdVectorFst {
        let mut t = t.clone();
        let states: Vec<StateId> = t.states_iter().collect();
        for s in states {
            if t.is_final(s).expect("s is a valid state of this fst") {
                if increment {
                    let old_weight = *t
                        .final_weight(s)
                        .expect("s is a valid state of this fst")
                        .expect("state confirmed final via is_final")
                        .value();
                    t.set_final(s, weight + old_weight)
                        .expect("s is a valid state of this fst");
                } else {
                    t.set_final(s, weight)
                        .expect("s is a valid state of this fst");
                }
            }
        }
        t
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.transform-weights-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.transform-weights-fn]
    pub fn transform_weights(t: &StdVectorFst, func: fn(f32) -> f32) -> StdVectorFst {
        let mut t = t.clone();
        let states: Vec<StateId> = t.states_iter().collect();
        for s in states {
            if t.is_final(s).expect("s is a valid state of this fst") {
                let v = *t
                    .final_weight(s)
                    .expect("s is a valid state of this fst")
                    .expect("state confirmed final via is_final")
                    .value();
                t.set_final(s, func(v))
                    .expect("s is a valid state of this fst");
            }
            let trs = t.pop_trs(s).expect("s is a valid state of this fst");
            for arc in trs {
                let nw = func(*arc.weight.value());
                t.add_tr(
                    s,
                    StdTransition::new(arc.ilabel, arc.olabel, nw, arc.nextstate),
                )
                .expect("transition re-added to a state of this fst");
            }
        }
        t
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-weight-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-weight-fn]
    pub fn set_weight(t: &StdVectorFst, f: f32) -> StdVectorFst {
        let mut t_copy = t.clone();
        let states: Vec<StateId> = t_copy.states_iter().collect();
        for s in states {
            if t_copy.is_final(s).expect("s is a valid state of this fst") {
                t_copy
                    .set_final(s, f)
                    .expect("s is a valid state of this fst");
            }
        }
        t_copy
    }

    // ---- basic accessors ----

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.is-automaton-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.is-automaton-fn]
    pub fn is_automaton(t: &StdVectorFst) -> bool {
        for s in t.states_iter() {
            for arc in t.get_trs(s).expect("s is a valid state of this fst").trs() {
                if arc.ilabel != arc.olabel {
                    return false;
                }
                if arc.ilabel == 1 {
                    // ?:?
                    return false;
                }
            }
        }
        true
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.number-of-states-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.number-of-states-fn]
    pub fn number_of_states(t: &StdVectorFst) -> u32 {
        let mut retval = 0u32;
        for _s in t.states_iter() {
            retval += 1;
        }
        retval
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.number-of-arcs-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.number-of-arcs-fn]
    pub fn number_of_arcs(t: &StdVectorFst) -> u32 {
        let mut retval = 0u32;
        for s in t.states_iter() {
            retval += t.num_trs(s).expect("s is a valid state of this fst") as u32;
        }
        retval
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.is-cyclic-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.is-cyclic-fn]
    pub fn is_cyclic(t: &StdVectorFst) -> bool {
        // C++: return t->Properties(kCyclic, true) & kCyclic;
        let mut known = FstProperties::empty();
        let props = compute_fst_properties(t, FstProperties::CYCLIC, &mut known, true)
            .expect("rustfst compute_fst_properties");
        props.contains(FstProperties::CYCLIC)
    }

    // ---- public low-level builders ----

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-state-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-state-fn]
    pub fn add_state(t: &mut StdVectorFst) -> StateId {
        let s = t.add_state();
        if s == 0 {
            t.set_start(s)
                .expect("start state just created by add_state");
        }
        s
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-final-weight-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-final-weight-fn]
    pub fn set_final_weight(t: &mut StdVectorFst, s: StateId, w: f32) {
        t.set_final(s, w)
            .expect("s is a valid state obtained from add_state");
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-transition-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-transition-fn]
    pub fn add_transition(
        t: &mut StdVectorFst,
        source: StateId,
        isymbol: &str,
        osymbol: &str,
        w: f32,
        target: StateId,
    ) {
        let mut st = t
            .input_symbols()
            .expect("transducer has an input symbol table")
            .as_ref()
            .clone();
        let ilabel = st.add_symbol(isymbol);
        let olabel = st.add_symbol(osymbol);
        t.add_tr(source, StdTransition::new(ilabel, olabel, w, target))
            .expect("source is a valid state of this fst");
        t.set_input_symbols(Arc::new(st));
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-final-weight-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-final-weight-fn]
    pub fn get_final_weight(t: &StdVectorFst, s: StateId) -> f32 {
        // C++ 't->Final(s).Value()' — Zero().Value() is +inf for a non-final state.
        t.final_weight(s)
            .expect("s is a valid state of this fst")
            .map(|w| *w.value())
            .unwrap_or(f32::INFINITY)
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.is-final-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.is-final-fn]
    pub fn is_final(t: &StdVectorFst, s: StateId) -> bool {
        // C++ declares 'float' but computes '(t->Final(s) != Zero())' — a bool.
        t.is_final(s).expect("state id comes from the same fst")
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-initial-state-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-initial-state-fn]
    pub fn get_initial_state(t: &StdVectorFst) -> StateId {
        t.start().unwrap_or(NO_STATE_ID)
    }

    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.represent-empty-transducer-as-having-one-state-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.represent-empty-transducer-as-having-one-state-fn]
    pub fn represent_empty_as_one_state(t: &mut StdVectorFst) {
        if t.start().is_none() || t.num_states() == 0 {
            // BUG PRESERVED: the C++ does 'delete t; t = create_empty_transducer();',
            // assigning a LOCAL pointer — the caller's transducer is unchanged.
            // We replicate the no-op (mutating *t here would change the caller).
        }
    }
}
