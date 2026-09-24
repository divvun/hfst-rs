//! Label substitution and free insertion.

use std::sync::Arc;

use super::*;

impl TropicalWeightTransducer {
    // ---- substitute ----------------------------------------------------------

    /// 'substitute(StdVectorFst*, unsigned int, unsigned int)' — relabels label
    /// 'old_number' to 'new_number' on both the input and output side (C++ uses
    /// 'RelabelFst<StdArc>(*t, v, v)'; modelled here as a direct rebuild since
    /// rustfst's 'relabel_pairs' module is private).
    pub fn substitute_number(t: &StdVectorFst, old_number: u32, new_number: u32) -> StdVectorFst {
        let mut result = t.clone();
        let states: Vec<StateId> = result.states_iter().collect();
        for s in states {
            let trs = result.pop_trs(s).expect("s is a valid state of this fst");
            let mut nt: Vec<StdTransition> = Vec::with_capacity(trs.len());
            for mut a in trs {
                if a.ilabel == old_number {
                    a.ilabel = new_number;
                }
                if a.olabel == old_number {
                    a.olabel = new_number;
                }
                nt.push(a);
            }
            for a in nt {
                result
                    .add_tr(s, a)
                    .expect("transition re-added to a state of this fst");
            }
        }
        result
    }

    /// 'substitute(StdVectorFst*, NumberPair old, NumberPair new)'. The C++
    /// encodes label pairs ('kEncodeLabels'), substitutes the single encoded
    /// label, then decodes; the net effect (replace arcs whose '(ilabel,olabel)'
    /// equals 'old' with 'new') is reproduced here directly.
    pub fn substitute_number_pair(
        t: &StdVectorFst,
        old_number_pair: NumberPair,
        new_number_pair: NumberPair,
    ) -> StdVectorFst {
        let mut result = t.clone();
        let states: Vec<StateId> = result.states_iter().collect();
        for s in states {
            let trs = result.pop_trs(s).expect("s is a valid state of this fst");
            let mut nt: Vec<StdTransition> = Vec::with_capacity(trs.len());
            for mut a in trs {
                if a.ilabel == old_number_pair.0 && a.olabel == old_number_pair.1 {
                    a.ilabel = new_number_pair.0;
                    a.olabel = new_number_pair.1;
                }
                nt.push(a);
            }
            for a in nt {
                result
                    .add_tr(s, a)
                    .expect("transition re-added to a state of this fst");
            }
        }
        result
    }

    /// 'substitute(StdVectorFst*, std::string old_symbol, std::string new_symbol)'.
    pub fn substitute_symbol(
        t: &StdVectorFst,
        old_symbol: String,
        new_symbol: String,
    ) -> StdVectorFst {
        // assert(t->InputSymbols() != NULL);
        let mut st = (**t
            .input_symbols()
            .expect("transducer has an input symbol table"))
        .clone();
        let old_l = st.add_symbol(old_symbol.as_str());
        let new_l = st.add_symbol(new_symbol.as_str());
        let mut retval = Self::substitute_number(t, old_l, new_l);
        retval.set_input_symbols(Arc::new(st));
        retval
    }

    /// 'substitute(StdVectorFst*, StringPair old, StringPair new)'.
    pub fn substitute_string_pair(
        t: &StdVectorFst,
        old_symbol_pair: StringPair,
        new_symbol_pair: StringPair,
    ) -> StdVectorFst {
        // assert(t->InputSymbols() != NULL);
        let mut st = (**t
            .input_symbols()
            .expect("transducer has an input symbol table"))
        .clone();
        let old_pair: NumberPair = (
            st.add_symbol(old_symbol_pair.0.as_str()),
            st.add_symbol(old_symbol_pair.1.as_str()),
        );
        let new_pair: NumberPair = (
            st.add_symbol(new_symbol_pair.0.as_str()),
            st.add_symbol(new_symbol_pair.1.as_str()),
        );
        let mut retval = Self::substitute_number_pair(t, old_pair, new_pair);
        retval.set_input_symbols(Arc::new(st));
        retval
    }

    /// 'substitute(StdVectorFst*, StringPair old, StringPairSet new)'.
    pub fn substitute_string_pair_set(
        t: &StdVectorFst,
        old_symbol_pair: StringPair,
        new_symbol_pair_set: StringPairSet,
    ) -> StdVectorFst {
        let mut tc = t.clone();
        let mut st = (**tc
            .input_symbols()
            .expect("transducer has an input symbol table"))
        .clone();
        // assert(st != NULL);
        let states: Vec<StateId> = tc.states_iter().collect();
        for s in states {
            let trs = tc.pop_trs(s).expect("s is a valid state of this fst");
            let mut nt: Vec<StdTransition> = Vec::new();
            for arc in trs {
                let isym = st.get_symbol(arc.ilabel).unwrap_or("").to_string();
                let osym = st.get_symbol(arc.olabel).unwrap_or("").to_string();
                if isym == old_symbol_pair.0 && osym == old_symbol_pair.1 {
                    // C++ replaces this arc with one arc per pair in the set;
                    // an empty set leaves the original arc untouched (the C++
                    // 'SetValue' is never reached).
                    if new_symbol_pair_set.is_empty() {
                        nt.push(arc);
                    } else {
                        for it in new_symbol_pair_set.iter() {
                            let il = st.add_symbol(it.0.as_str());
                            let ol = st.add_symbol(it.1.as_str());
                            nt.push(StdTransition::new(il, ol, arc.weight, arc.nextstate));
                        }
                    }
                } else {
                    nt.push(arc);
                }
            }
            for a in nt {
                tc.add_tr(s, a)
                    .expect("transition re-added to a state of this fst");
            }
        }
        tc.set_input_symbols(Arc::new(st));
        tc
    }

    /// 'substitute(StdVectorFst*, const StringPair old, StdVectorFst *transducer)'.
    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.substitute-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.substitute-fn]
    pub fn substitute_string_transducer(
        t: &StdVectorFst,
        old_symbol_pair: StringPair,
        transducer: &StdVectorFst,
    ) -> StdVectorFst {
        // assert(t->InputSymbols() != NULL);
        let mut result = t.clone();
        let mut st = (**result
            .input_symbols()
            .expect("transducer has an input symbol table"))
        .clone();
        let old_il = st.add_symbol(old_symbol_pair.0.as_str());
        let old_ol = st.add_symbol(old_symbol_pair.1.as_str());

        let states = result.num_states() as u32;
        for i in 0..states {
            let trs = result.pop_trs(i).expect("i is a valid state of this fst");
            let mut kept: Vec<StdTransition> = Vec::with_capacity(trs.len());
            for mut arc in trs {
                // find arcs that must be replaced
                if arc.ilabel == old_il && arc.olabel == old_ol {
                    let destination_state = arc.nextstate;
                    let start_state = result.add_state();

                    // change the label of the arc to epsilon and point the arc
                    // to a new state (weight remains the same)
                    arc.ilabel = 0;
                    arc.olabel = 0;
                    arc.nextstate = start_state;
                    kept.push(arc);

                    // add rest of the states to transducer t
                    let states_to_add = transducer.num_states();
                    for _ in 1..states_to_add {
                        result.add_state();
                    }

                    // go through all states and arcs in replace transducer tr
                    for tr_state_id in transducer.states_iter() {
                        // final states in tr correspond in t to a non-final
                        // state which has an epsilon transition to original
                        // destination state of arc that is being replaced
                        if transducer
                            .is_final(tr_state_id)
                            .expect("tr_state_id is a valid state of transducer")
                        {
                            let fw = transducer
                                .final_weight(tr_state_id)
                                .expect("tr_state_id is a valid state of transducer")
                                .expect("tr_state_id confirmed final via is_final");
                            result
                                .add_tr(
                                    tr_state_id + start_state,
                                    StdTransition::new(0, 0, fw, destination_state),
                                )
                                .expect("transition added to a state created above in result");
                        }

                        for tr_arc in transducer
                            .get_trs(tr_state_id)
                            .expect("tr_state_id is a valid state of transducer")
                            .trs()
                        {
                            result
                                .add_tr(
                                    tr_state_id + start_state,
                                    StdTransition::new(
                                        tr_arc.ilabel,
                                        tr_arc.olabel,
                                        tr_arc.weight,
                                        tr_arc.nextstate + start_state,
                                    ),
                                )
                                .expect("transition added to a state created above in result");
                        }
                    }
                } else {
                    kept.push(arc);
                }
            }
            for a in kept {
                result
                    .add_tr(i, a)
                    .expect("transition re-added to a state of this fst");
            }
        }

        result.set_input_symbols(Arc::new(st));
        result
    }

    /// 'substitute(StdVectorFst*, const NumberPair old, StdVectorFst *transducer)'.
    pub fn substitute_number_transducer(
        t: &StdVectorFst,
        old_number_pair: NumberPair,
        transducer: &StdVectorFst,
    ) -> StdVectorFst {
        let mut result = t.clone();

        let states = result.num_states() as u32;
        for i in 0..states {
            let trs = result.pop_trs(i).expect("i is a valid state of this fst");
            let mut kept: Vec<StdTransition> = Vec::with_capacity(trs.len());
            for mut arc in trs {
                // find arcs that must be replaced
                if arc.ilabel == old_number_pair.0 && arc.olabel == old_number_pair.1 {
                    let destination_state = arc.nextstate;
                    let start_state = result.add_state();

                    arc.ilabel = 0;
                    arc.olabel = 0;
                    arc.nextstate = start_state;
                    kept.push(arc);

                    let states_to_add = transducer.num_states();
                    for _ in 1..states_to_add {
                        result.add_state();
                    }

                    for tr_state_id in transducer.states_iter() {
                        if transducer
                            .is_final(tr_state_id)
                            .expect("tr_state_id is a valid state of transducer")
                        {
                            let fw = transducer
                                .final_weight(tr_state_id)
                                .expect("tr_state_id is a valid state of transducer")
                                .expect("tr_state_id confirmed final via is_final");
                            result
                                .add_tr(
                                    tr_state_id + start_state,
                                    StdTransition::new(0, 0, fw, destination_state),
                                )
                                .expect("transition added to a state created above in result");
                        }

                        for tr_arc in transducer
                            .get_trs(tr_state_id)
                            .expect("tr_state_id is a valid state of transducer")
                            .trs()
                        {
                            result
                                .add_tr(
                                    tr_state_id + start_state,
                                    StdTransition::new(
                                        tr_arc.ilabel,
                                        tr_arc.olabel,
                                        tr_arc.weight,
                                        tr_arc.nextstate + start_state,
                                    ),
                                )
                                .expect("transition added to a state created above in result");
                        }
                    }
                } else {
                    kept.push(arc);
                }
            }
            for a in kept {
                result
                    .add_tr(i, a)
                    .expect("transition re-added to a state of this fst");
            }
        }

        result
    }

    // ---- insert_freely -------------------------------------------------------

    /// 'insert_freely(StdVectorFst*, const StringPair &symbol_pair)'.
    // [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.insert-freely-fn]
    // [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.insert-freely-fn]
    pub fn insert_freely_string(t: &StdVectorFst, symbol_pair: &StringPair) -> StdVectorFst {
        let mut result = t.clone();
        let mut st = (**result
            .input_symbols()
            .expect("transducer has an input symbol table"))
        .clone();
        // assert(st != NULL);
        let states: Vec<StateId> = result.states_iter().collect();
        for state_id in states {
            let il = st.add_symbol(symbol_pair.0.as_str());
            let ol = st.add_symbol(symbol_pair.1.as_str());
            result
                .add_tr(
                    state_id,
                    StdTransition::new(il, ol, TropicalWeight::new(0.0), state_id),
                )
                .expect("self-loop added to a valid state of this fst");
        }
        result.set_input_symbols(Arc::new(st));
        result
    }
}
