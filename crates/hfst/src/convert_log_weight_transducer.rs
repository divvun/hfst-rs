//! Port of 'libhfst/src/implementations/ConvertLogWeightTransducer.cc' — the
//! 'HfstBasicTransducer' <-> OpenFst log-weight 'LogFst' ('LogVectorFst')
//! conversions.
//!
//! This is the log-semiring sibling of ['crate::convert_tropical_weight_transducer'],
//! but ported from the (older, string-table-based) log '.cc': the converters
//! walk the OpenFst 'SymbolTable's with 'Find' (= rustfst 'get_symbol'), recode
//! the initial/zero state numbers by hand ('zero_print'/'origin'/'target'), and
//! build the reverse transducer through the 'state_map' +
//! 'hfst_state_to_state_id' helper rather than the harmonization-vector path.
//!
//! 'using namespace fst;' is mapped onto the 'hfst-openfst' adapter:
//! 'LogVectorFst', 'LogTransition' (= 'fst::LogArc'), 'LogWeight',
//! 'SymbolTable', and the rustfst trait methods brought in via
//! 'hfst_openfst::prelude::*'. The C++ 'StateIterator'/'ArcIterator' traversal
//! becomes 'states_iter()' + 'get_trs(s)'; 't->Start() == kNoStateId' becomes
//! 't.start().is_none()'; and 't->Final(s) != Zero()' becomes 't.is_final(s)'.
//!
//! Ownership: the C++ 'new's and returns raw pointers; here both directions
//! return owned values ('HfstBasicTransducer' / 'LogVectorFst').
//!
//! BUG-PRESERVATION: 'hfst_basic_transducer_to_log_ofst' declares
//! 'unsigned int source_state = 0;' and uses it inside the state loop but never
//! increments it, so every state's transitions are keyed off state 0. This is a
//! genuine HFST bug; it is replicated verbatim below (the increment is absent on
//! purpose).

#![allow(non_snake_case)]

use std::collections::BTreeMap;
use std::sync::Arc;

use hfst_openfst::prelude::*;
use hfst_openfst::{LogTransition, LogVectorFst, LogWeight, SymbolTable};

use crate::convert_transducer_format::{ConversionFunctions, StateId};
use crate::hfst_basic_transducer::{HfstBasicTransducer, HfstState};
use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_symbol_defs::{internal_epsilon, internal_identity, internal_unknown};

impl ConversionFunctions {
    /* --- Conversion between log OpenFst and HfstBasicTransducer --- */

    /* Create an HfstBasicTransducer equivalent to an OpenFst log weight
    transducer `t`. */
    // [spec:hfst:def:convert-log-weight-transducer.hfst.implementations.conversion-functions.log-ofst-to-hfst-basic-transducer-fn]
    // [spec:hfst:sem:convert-log-weight-transducer.hfst.implementations.conversion-functions.log-ofst-to-hfst-basic-transducer-fn]
    // [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.log-ofst-to-hfst-basic-transducer-fn]
    // [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.log-ofst-to-hfst-basic-transducer-fn]
    pub fn log_ofst_to_hfst_basic_transducer(
        t: &LogVectorFst,
        has_hfst_header: bool,
    ) -> crate::error::Result<HfstBasicTransducer> {
        let inputsym = t.input_symbols();
        let mut outputsym = t.output_symbols();

        /* An HFST log transducer always has an input symbol table. */
        if has_hfst_header && inputsym.is_none() {
            crate::bail!(MissingOpenFstInputSymbolTable);
        }

        let mut net = HfstBasicTransducer::new();

        // An empty transducer
        if t.start().is_none() {
            /* An empty OpenFst transducer does not necessarily have to have
            an input or output symbol table. */
            if let Some(inputsym) = inputsym {
                for (label, sym) in inputsym.iter() {
                    if label != 0 {
                        // epsilon is not inserted
                        net.add_symbol_to_alphabet(&sym.to_string());
                    }
                }
            }
            /* If the transducer is an OpenFst transducer, it might have an output
            symbol table. If the transducer is an HFST log transducer, it
            can have an output symbol table, but it is equivalent to the
            input symbol table. */
            if !has_hfst_header {
                if let Some(outputsym) = outputsym {
                    for (label, sym) in outputsym.iter() {
                        if label != 0 {
                            // epsilon is not inserted
                            net.add_symbol_to_alphabet(&sym.to_string());
                        }
                    }
                }
            }
            return Ok(net);
        }

        /* A non-empty OpenFst transducer must have at least an input symbol table.
        If the output symbol table is missing, we assume that it would be
        equivalent to the input symbol table. */
        let inputsym = match inputsym {
            None => {
                crate::bail!(MissingOpenFstInputSymbolTable);
            }
            Some(inputsym) => inputsym,
        };
        if outputsym.is_none() {
            outputsym = Some(inputsym);
        }
        let outputsym = outputsym.unwrap();

        /* This takes care that initial state is always number zero
        and state number zero (if it is not initial) is some other number
        (basically as the number of the initial state in that case, i.e.
        the numbers of initial state and state number zero are swapped) */
        let mut zero_print: StateId = 0;
        let initial_state: StateId = t.start().unwrap();
        if initial_state != 0 {
            zero_print = initial_state;
        }

        /* Go through all states */
        for s in t.states_iter() {
            if s == initial_state {
                // how origin state is printed, see the first comment
                let origin: i32 = if s == 0 {
                    zero_print as i32
                } else if s == initial_state {
                    0
                } else {
                    s as i32
                };

                /* Go through all transitions in a state */
                for arc in t.get_trs(s).unwrap().trs().iter() {
                    // how target state is printed, see the first comment
                    let target: i32 = if arc.nextstate == 0 {
                        zero_print as i32
                    } else if arc.nextstate == initial_state {
                        0
                    } else {
                        arc.nextstate as i32
                    };

                    // Copy the transition
                    let mut istring: String =
                        inputsym.get_symbol(arc.ilabel).unwrap_or("").to_string();
                    let mut ostring: String =
                        outputsym.get_symbol(arc.olabel).unwrap_or("").to_string();
                    if arc.ilabel == 0 {
                        istring = internal_epsilon.to_string();
                    }
                    if arc.olabel == 0 {
                        ostring = internal_epsilon.to_string();
                    }
                    let new_tr = HfstBasicTransition::new_symbols(
                        target as HfstState,
                        istring,
                        ostring,
                        *arc.weight.value(),
                        net.coder_mut(),
                    );
                    net.add_transition(origin as HfstState, &new_tr, true);
                }
                if t.is_final(s).unwrap() {
                    // Set the state as final
                    let fw = *t.final_weight(s).unwrap().unwrap().value();
                    net.set_final_weight(origin as HfstState, &fw);
                }
                break;
            }
        }

        for s in t.states_iter() {
            if s != initial_state {
                // how origin state is printed, see the first comment
                let origin: i32 = if s == 0 {
                    zero_print as i32
                } else if s == initial_state {
                    0
                } else {
                    s as i32
                };
                for arc in t.get_trs(s).unwrap().trs().iter() {
                    // how target state is printed, see the first comment
                    let target: i32 = if arc.nextstate == 0 {
                        zero_print as i32
                    } else if arc.nextstate == initial_state {
                        0
                    } else {
                        arc.nextstate as i32
                    };

                    let mut istring: String =
                        inputsym.get_symbol(arc.ilabel).unwrap_or("").to_string();
                    let mut ostring: String =
                        outputsym.get_symbol(arc.olabel).unwrap_or("").to_string();
                    if arc.ilabel == 0 {
                        istring = internal_epsilon.to_string();
                    }
                    if arc.olabel == 0 {
                        ostring = internal_epsilon.to_string();
                    }
                    let new_tr = HfstBasicTransition::new_symbols(
                        target as HfstState,
                        istring,
                        ostring,
                        *arc.weight.value(),
                        net.coder_mut(),
                    );
                    net.add_transition(origin as HfstState, &new_tr, true);
                }
                if t.is_final(s).unwrap() {
                    let fw = *t.final_weight(s).unwrap().unwrap().value();
                    net.set_final_weight(origin as HfstState, &fw);
                }
            }
        }

        /* Make sure that also the symbols that occur only in the alphabet
        but not in transitions are copied. */
        for (label, sym) in inputsym.iter() {
            if label != 0 {
                // epsilon is not inserted
                net.add_symbol_to_alphabet(&sym.to_string());
            }
        }
        for (label, sym) in outputsym.iter() {
            if label != 0 {
                // epsilon is not inserted
                net.add_symbol_to_alphabet(&sym.to_string());
            }
        }

        Ok(net)
    }

    /* Get a state id for a state in transducer 't' that corresponds
    to HfstState s as defined in `state_map`.
    Used by function hfst_basic_transducer_to_log_ofst. */
    // [spec:hfst:def:convert-log-weight-transducer.hfst.implementations.conversion-functions.hfst-state-to-state-id-fn]
    // [spec:hfst:sem:convert-log-weight-transducer.hfst.implementations.conversion-functions.hfst-state-to-state-id-fn]
    // [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.hfst-state-to-state-id-fn]
    // [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.hfst-state-to-state-id-fn]
    pub fn hfst_state_to_state_id(
        s: HfstState,
        state_map: &mut BTreeMap<HfstState, StateId>,
        t: &mut LogVectorFst,
    ) -> StateId {
        match state_map.get(&s) {
            None => {
                // If not found, add a state
                let retval = t.add_state();
                state_map.insert(s, retval);
                retval
            }
            Some(second) => *second,
        }
    }

    /* Create an OpenFst transducer equivalent to HfstBasicTransducer 'net'. */
    // [spec:hfst:def:convert-log-weight-transducer.hfst.implementations.conversion-functions.hfst-basic-transducer-to-log-ofst-fn]
    // [spec:hfst:sem:convert-log-weight-transducer.hfst.implementations.conversion-functions.hfst-basic-transducer-to-log-ofst-fn]
    // [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.hfst-basic-transducer-to-log-ofst-fn]
    // [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.hfst-basic-transducer-to-log-ofst-fn]
    pub fn hfst_basic_transducer_to_log_ofst(net: &HfstBasicTransducer) -> LogVectorFst {
        let mut t = LogVectorFst::new();
        let start_state = t.add_state();
        t.set_start(start_state).unwrap();

        // The mapping between states in HfstBasicTransducer and StdVectorFst
        let mut state_map: BTreeMap<HfstState, StateId> = BTreeMap::new();
        state_map.insert(0, start_state);

        // 'fst::SymbolTable st("");' — an empty table (rustfst's 'new()' would
        // pre-seed epsilon, so 'empty()' is used and epsilon is added below).
        let mut st = SymbolTable::empty();
        st.add_symbol(internal_epsilon);
        st.add_symbol(internal_unknown);
        st.add_symbol(internal_identity);

        // Go through all states
        // BUG-PRESERVATION: declared but never incremented inside the loop, so
        // 'hfst_state_to_state_id(source_state, ...)' always resolves to state 0.
        // Replicated verbatim from the C++ ('unsigned int source_state = 0;').
        let source_state: u32 = 0;
        let coder = net.coder();
        for it in net.iter() {
            // Go through the set of transitions in each state
            for tr_it in it.iter() {
                // Copy the transition
                let ilabel = st.add_symbol(tr_it.get_input_symbol(coder));
                let olabel = st.add_symbol(tr_it.get_output_symbol(coder));
                let origin = Self::hfst_state_to_state_id(source_state, &mut state_map, &mut t);
                let nextstate =
                    Self::hfst_state_to_state_id(tr_it.get_target_state(), &mut state_map, &mut t);
                t.add_tr(
                    origin,
                    LogTransition::new(ilabel, olabel, tr_it.get_weight(), nextstate),
                )
                .unwrap();
            }
        }

        // Go through the final states
        // The C++ iterates 'net->final_weight_map' (a map ordered by state); the
        // private map is not exposed, so the equivalent ascending-state walk over
        // the final states is used.
        for state in 0..=net.get_max_state() {
            if net.is_final_state(state) {
                let s = Self::hfst_state_to_state_id(state, &mut state_map, &mut t);
                t.set_final(
                    s,
                    LogWeight::new(
                        net.get_final_weight(state)
                            .expect("state was confirmed final via is_final_state"),
                    ),
                )
                .expect("s is a state that exists in the fst");
            }
        }

        // Add also symbols that do not occur in transitions. Resolve each
        // alphabet symbol's label through a clone of 'net's coder (mirrors the
        // tropical convert); 'get_symbol_number' needs '&mut self', and the
        // clone interns alphabet-only symbols without disturbing 'net'.
        let mut coder = net.coder().clone();
        for it in net.get_alphabet().iter() {
            // explicit-label add so the FST labels match the basic-transducer
            // symbol numbers (see the tropical convert for the rationale).
            let symbol_number = coder.get_number(it);
            st.add_symbol_with_key(it.clone(), symbol_number);
        }

        t.set_input_symbols(Arc::new(st));
        t
    }
}
