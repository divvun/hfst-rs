//! Port of 'libhfst/src/implementations/ConvertTropicalWeightTransducer.cc' —
//! the 'HfstBasicTransducer' <-> OpenFst tropical-weight 'StdVectorFst'
//! conversions.
//!
//! As in ['crate::convert_ol_transducer'], the two self-contained entry points
//! are ported here as methods on ['ConversionFunctions'], and the two C++
//! file-'static' helpers ('handle_symbol_tables' / 'copy_alphabet') become
//! module-private free functions.
//!
//! 'using namespace fst;' is mapped onto the 'hfst-openfst' adapter:
//! 'StdVectorFst', 'StdTransition' (= 'fst::StdArc'), 'SymbolTable', and the
//! rustfst trait methods brought in via 'hfst_openfst::prelude::*'. The C++
//! 'StateIterator'/'ArcIterator' traversal becomes 'states_iter()' +
//! 'get_trs(s)'; 't->Final(s) != Zero()' becomes 't.is_final(s)'; and
//! 't->Start() == kNoStateId' becomes 't.start().is_none()'.
//!
//! Ownership: the C++ 'new's and returns raw pointers; here both directions
//! return owned values ('HfstBasicTransducer' / 'StdVectorFst').

#![allow(non_snake_case)]

use std::sync::Arc;

use hfst_openfst::prelude::*;
use hfst_openfst::{StdTransition, StdVectorFst, SymbolTable};

use crate::convert_transducer_format::ConversionFunctions;
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_symbol_defs::{internal_epsilon, internal_identity, internal_unknown};

/* Handle symbol tables when converting 't' to 'net'. 'has_hfst_header'
defines whether `t` is an HFST transducer. */
// [spec:hfst:def:convert-tropical-weight-transducer.hfst.implementations.handle-symbol-tables-fn]
// [spec:hfst:sem:convert-tropical-weight-transducer.hfst.implementations.handle-symbol-tables-fn]
fn handle_symbol_tables(
    t: &StdVectorFst,
    net: &mut HfstBasicTransducer,
    has_hfst_header: bool,
) -> crate::error::Result<()> {
    let inputsym = t.input_symbols();
    let outputsym = t.output_symbols();

    /* An HFST tropical transducer always has an input symbol table. */
    if has_hfst_header && inputsym.is_none() {
        crate::bail!(MissingOpenFstInputSymbolTable);
    }

    // An empty transducer
    if t.start().is_none() {
        /* An empty OpenFst transducer does not necessarily have to have
        an input or output symbol table. */
        if let Some(inputsym) = inputsym {
            for (label, sym) in inputsym.iter() {
                assert!(!sym.is_empty());

                if label != 0 {
                    // epsilon is not inserted
                    net.add_symbol_to_alphabet(&crate::hfst_data_types::Symbol::new(sym));
                }
            }
        }
        /* If the transducer is an OpenFst transducer, it might have an output
        symbol table. If the transducer is an HFST tropical transducer, it
        can have an output symbol table, but it is equivalent to the
        input symbol table. */
        if !has_hfst_header && let Some(outputsym) = outputsym {
            for (label, sym) in outputsym.iter() {
                assert!(!sym.is_empty());
                if label != 0 {
                    // epsilon is not inserted
                    net.add_symbol_to_alphabet(&crate::hfst_data_types::Symbol::new(sym));
                }
            }
        }
        return Ok(());
    }

    /* A non-empty OpenFst transducer must have at least an input symbol table.
    If the output symbol table is missing, we assume that it would be
    equivalent to the input symbol table. */
    if inputsym.is_none() {
        crate::bail!(MissingOpenFstInputSymbolTable);
    }
    Ok(())
}

/* Copy alphabet of 't' to 'net'. */
// [spec:hfst:def:convert-tropical-weight-transducer.hfst.implementations.copy-alphabet-fn]
// [spec:hfst:sem:convert-tropical-weight-transducer.hfst.implementations.copy-alphabet-fn]
fn copy_alphabet(t: &StdVectorFst, net: &mut HfstBasicTransducer) {
    let inputsym = t.input_symbols();
    let outputsym = t.output_symbols();

    if let Some(inputsym) = inputsym {
        for (label, sym) in inputsym.iter() {
            assert!(!sym.is_empty());
            if label != 0 {
                // epsilon is not inserted
                net.add_symbol_to_alphabet(&crate::hfst_data_types::Symbol::new(sym));
            }
        }
    }
    if let Some(outputsym) = outputsym {
        for (label, sym) in outputsym.iter() {
            assert!(!sym.is_empty());
            if label != 0 {
                // epsilon is not inserted
                net.add_symbol_to_alphabet(&crate::hfst_data_types::Symbol::new(sym));
            }
        }
    }
}

impl ConversionFunctions {
    /* ----------------------------------------------------------------------

    Create an HfstBasicTransducer equivalent to an OpenFst tropical weight
    transducer `t`.

    ---------------------------------------------------------------------- */

    // [spec:hfst:def:convert-tropical-weight-transducer.hfst.implementations.conversion-functions.tropical-ofst-to-hfst-basic-transducer-fn]
    // [spec:hfst:sem:convert-tropical-weight-transducer.hfst.implementations.conversion-functions.tropical-ofst-to-hfst-basic-transducer-fn]
    // [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.tropical-ofst-to-hfst-basic-transducer-fn]
    // [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.tropical-ofst-to-hfst-basic-transducer-fn]
    pub fn tropical_ofst_to_hfst_basic_transducer(
        t: &StdVectorFst,
        has_hfst_header: bool,
    ) -> crate::error::Result<HfstBasicTransducer> {
        let mut net = HfstBasicTransducer::new();

        handle_symbol_tables(t, &mut net, has_hfst_header)?;

        let symbol_vector =
            crate::tropical_weight_transducer::TropicalWeightTransducer::get_symbol_vector(t);

        // Intern the OpenFst symbol vector into 'net's own coder, so the arc
        // numbers below are in 'net's coding (the per-graph-coder replacement for
        // the former process-global harmonization vector).
        let harmonization_vector = net.coder_mut().get_harmonization_vector(&symbol_vector);

        /* This takes care that initial state is always number zero
        and state number zero (if it is not initial) is some other number
        (basically as the number of the initial state in that case, i.e.
        the numbers of initial state and state number zero are swapped) */
        // 'StateId initial_state = t->Start();' — OpenFst's 'kNoStateId' sentinel
        // ('-1') cast to the unsigned 'StateId' becomes 'u32::MAX'; for an empty
        // transducer the state loop below never runs, so the value is unused.
        let initial_state: u32 = t.start().unwrap_or(u32::MAX);

        /* Go through all states */
        for s in t.states_iter() {
            let mut origin: u32 = s;
            if origin == initial_state {
                origin = 0;
            } else if origin == 0 {
                origin = initial_state;
            }

            let number_of_arcs: u32 = t.num_trs(s).expect("s is a valid state of this fst") as u32;
            net.initialize_transition_vector(s, number_of_arcs);

            /* Go through all transitions in a state */
            for arc in t
                .get_trs(s)
                .expect("s is a valid state of this fst")
                .trs()
                .iter()
            {
                let mut target: u32 = arc.nextstate;
                if target == initial_state {
                    target = 0;
                } else if target == 0 {
                    target = initial_state;
                }

                if arc.ilabel as usize >= symbol_vector.len() {
                    let oss = format!(
                        "FATAL ERROR: input number {} not in symbol_vector\n",
                        arc.ilabel
                    );
                    crate::bail!(Fatal, oss);
                    // exit(1);
                }
                if arc.olabel as usize >= symbol_vector.len() {
                    let oss = format!(
                        "FATAL ERROR: output number {} not in symbol_vector\n",
                        arc.olabel
                    );
                    crate::bail!(Fatal, oss);
                    // exit(1);
                }

                net.add_transition(
                    origin,
                    &HfstBasicTransition::new_numbers(
                        target,
                        harmonization_vector[arc.ilabel as usize],
                        harmonization_vector[arc.olabel as usize],
                        *arc.weight.value(),
                        false,
                    ), // dummy parameter needed because numbers are used
                    false,
                ); // do not insert symbols to alphabet
            }

            if t.is_final(s).expect("s is a valid state of this fst") {
                // Set the state as final
                let fw = *t
                    .final_weight(s)
                    .expect("s is a valid state of this fst")
                    .expect("state is final so weight is present")
                    .value();
                net.set_final_weight(origin, &fw);
            }
        }

        // Copy the alphabet
        copy_alphabet(t, &mut net);

        Ok(net)
    }

    /* ----------------------------------------------------------------------

    Create an hfst_ol::Transducer equivalent to an OpenFst tropical weight
    transducer `t`, numbering symbols from an already-built harmonizer
    alphabet.

    ---------------------------------------------------------------------- */

    /// The direct tropical→OL conversion used by the pmatch archive writer:
    /// the composition of [`Self::tropical_ofst_to_hfst_basic_transducer`]
    /// and 'hfst_basic_transducer_to_hfst_ol' for the harmonizer case,
    /// without materializing the basic transducer — on a multi-million-arc
    /// pmatch TOP the basic intermediate costs ~150 B/arc and owned the
    /// process's peak RSS. State 0 and the start state are swapped and arcs
    /// walked in order, exactly as the basic-transducer route does, so the
    /// emitted tables are byte-identical to that route's.
    pub fn tropical_ofst_to_hfst_ol(
        t: &StdVectorFst,
        weighted: bool,
        options: &str,
        harmonizer: &crate::transducer::Transducer,
    ) -> crate::error::Result<crate::transducer::Transducer> {
        use crate::convert::{StatePlaceholder, TransitionPlaceholder};
        use crate::convert_ol_transducer::{harmonizer_numbering, pack_ol_tables};

        let (symbol_table, string_symbol_map, seen_input_symbols, flag_symbols) =
            harmonizer_numbering(harmonizer);

        // Resolve each tropical label to its OL symbol number once, up front —
        // the per-arc lookups below are then plain indexing. Symbols missing
        // from the harmonizer alphabet fall back to 0 (epsilon), matching the
        // string-map lookup in get_states_and_symbols.
        let symbol_vector =
            crate::tropical_weight_transducer::TropicalWeightTransducer::get_symbol_vector(t);
        let label_to_ol: Vec<crate::transducer::SymbolNumber> = symbol_vector
            .iter()
            .map(|sym| string_symbol_map.get(sym).copied().unwrap_or(0))
            .collect();

        let initial_state: u32 = t.start().unwrap_or(u32::MAX);
        let swap = |s: u32| {
            if s == initial_state {
                0
            } else if s == 0 {
                initial_state
            } else {
                s
            }
        };

        let state_count =
            crate::tropical_weight_transducer::TropicalWeightTransducer::number_of_states(t);
        let mut state_placeholders: Vec<StatePlaceholder> =
            Vec::with_capacity(state_count as usize);
        let mut first_transition: u32 = 0;
        for basic_state in 0..state_count {
            let s = swap(basic_state);
            let is_final = t.is_final(s).expect("s is a valid state of this fst");
            let final_w: f32 = if is_final {
                *t.final_weight(s)
                    .expect("s is a valid state of this fst")
                    .expect("state is final so weight is present")
                    .value()
            } else {
                0.0
            };
            let mut placeholder =
                StatePlaceholder::new(basic_state, is_final, first_transition, final_w);
            first_transition += 1; // there's a padding entry between states
            for arc in t
                .get_trs(s)
                .expect("s is a valid state of this fst")
                .trs()
                .iter()
            {
                first_transition += 1;
                if arc.ilabel as usize >= label_to_ol.len() {
                    let oss = format!(
                        "FATAL ERROR: input number {} not in symbol_vector\n",
                        arc.ilabel
                    );
                    crate::bail!(Fatal, oss);
                }
                if arc.olabel as usize >= label_to_ol.len() {
                    let oss = format!(
                        "FATAL ERROR: output number {} not in symbol_vector\n",
                        arc.olabel
                    );
                    crate::bail!(Fatal, oss);
                }
                let in_sym = label_to_ol[arc.ilabel as usize];
                let out_sym = label_to_ol[arc.olabel as usize];
                placeholder.add_input(in_sym, &flag_symbols);
                placeholder.add_transition(TransitionPlaceholder::new(
                    swap(arc.nextstate),
                    in_sym,
                    out_sym,
                    *arc.weight.value(),
                ));
            }
            state_placeholders.push(placeholder);
        }

        pack_ol_tables(
            state_placeholders,
            symbol_table,
            seen_input_symbols,
            flag_symbols,
            weighted,
            options.contains("empty_alphabet"),
        )
    }

    /* ------------------------------------------------------------------------

    Create an OpenFst transducer equivalent to HfstBasicTransducer `net`.

    ------------------------------------------------------------------------ */

    // [spec:hfst:def:convert-tropical-weight-transducer.hfst.implementations.conversion-functions.hfst-basic-transducer-to-tropical-ofst-fn]
    // [spec:hfst:sem:convert-tropical-weight-transducer.hfst.implementations.conversion-functions.hfst-basic-transducer-to-tropical-ofst-fn]
    // [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.hfst-basic-transducer-to-tropical-ofst-fn]
    // [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.hfst-basic-transducer-to-tropical-ofst-fn]
    pub fn hfst_basic_transducer_to_tropical_ofst(net: &HfstBasicTransducer) -> StdVectorFst {
        let (mut t, state_vector, st) = tropical_shell(net);
        for (source_state, transitions) in net.iter().enumerate() {
            copy_transitions(&mut t, &state_vector, source_state, transitions);
        }
        finish_tropical(net, &mut t, &state_vector, st);
        t
    }

    /// [`Self::hfst_basic_transducer_to_tropical_ofst`], consuming `net`.
    ///
    /// Each state's transitions are moved out and dropped as soon as they are
    /// copied, so the source shrinks while the result grows instead of both
    /// standing at full size. On a flag-harmonized operand — hundreds of
    /// millions of transitions — that is the difference between one copy of the
    /// graph in memory and two.
    pub fn basic_to_tropical_ofst_owned(mut net: HfstBasicTransducer) -> StdVectorFst {
        let (mut t, state_vector, st) = tropical_shell(&net);
        for source_state in 0..net.state_vector.len() {
            let transitions = std::mem::take(&mut net.state_vector[source_state]);
            copy_transitions(&mut t, &state_vector, source_state, &transitions);
        }
        finish_tropical(&net, &mut t, &state_vector, st);
        t
    }
}

/// The states, the state renumbering and the symbol table of the tropical
/// transducer equivalent to `net` — everything but its transitions and final
/// weights.
fn tropical_shell(net: &HfstBasicTransducer) -> (StdVectorFst, Vec<u32>, SymbolTable) {
    let mut t = StdVectorFst::new();
    let start_state = t.add_state(); // always zero
    t.set_start(start_state)
        .expect("start state just created by add_state");

    // How state numbers are recoded
    let mut state_vector: Vec<u32> = Vec::new();
    state_vector.push(start_state);
    for _i in 1..=net.get_max_state() {
        state_vector.push(t.add_state());
    }

    // 'fst::SymbolTable st("");' — an empty table (rustfst's 'new()' would
    // pre-seed epsilon, so 'empty()' is used and epsilon is added below).
    let mut st = SymbolTable::empty();
    st.add_symbol(internal_epsilon); // label 0
    st.add_symbol(internal_unknown); // label 1
    st.add_symbol(internal_identity); // label 2

    // Copy the alphabet. The arc labels written below are 'net's own coder
    // numbers (get_input_number/get_output_number); resolve each alphabet
    // symbol's label through a clone of that coder so they coincide (and the
    // tropical->basic round-trip recovers them). The clone interns the few
    // alphabet-only symbols absent from any arc without disturbing 'net'.
    let mut coder = net.coder().clone();
    for it in net.get_alphabet().iter() {
        assert!(!it.is_empty());
        // C++: 'st.AddSymbol(*it, net->get_symbol_number(*it));' — assign the
        // symbol's coder number as its explicit label.
        let symbol_number = coder.get_number(it);
        st.add_symbol_with_key(it.clone(), symbol_number);
    }

    (t, state_vector, st)
}

/// Copy one state's transitions across, reserving exactly the room they need.
fn copy_transitions(
    t: &mut StdVectorFst,
    state_vector: &[u32],
    source_state: usize,
    transitions: &[HfstBasicTransition],
) {
    let source = state_vector[source_state];
    t.reserve_trs(source, transitions.len())
        .expect("state created above from state_vector");
    for tr_it in transitions {
        t.add_tr(
            source,
            StdTransition::new(
                tr_it.get_input_number(),
                tr_it.get_output_number(),
                tr_it.get_weight(),
                state_vector[tr_it.get_target_state() as usize],
            ),
        )
        .expect("transition added to a state created above from state_vector");
    }
}

/// The final weights and the symbol table, once every transition is in place.
fn finish_tropical(
    net: &HfstBasicTransducer,
    t: &mut StdVectorFst,
    state_vector: &[u32],
    st: SymbolTable,
) {
    // The C++ iterates 'net->final_weight_map' (a map ordered by state); the
    // private map is not exposed, so the equivalent ascending-state walk over
    // the final states is used.
    for state in 0..=net.get_max_state() {
        if net.is_final_state(state) {
            t.set_final(
                state_vector[state as usize],
                net.get_final_weight(state)
                    .expect("state was confirmed final via is_final_state"),
            )
            .expect("state_vector maps to a state that exists in the fst");
        }
    }

    t.set_input_symbols(Arc::new(st));
}
