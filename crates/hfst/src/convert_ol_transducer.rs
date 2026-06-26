//! Port of 'libhfst/src/implementations/ConvertOlTransducer.cc' — the
//! 'HfstBasicTransducer' <-> 'hfst_ol::Transducer' conversions.
//!
//! The two self-contained entry points are ported here as methods on
//! ['ConversionFunctions']. The facade-dependent pair
//! 'hfst_ol_to_hfst_transducer' / 'hfst_transducer_to_hfst_ol' (which wrap/unwrap
//! a 'HfstTransducer') and the 'MAIN_TEST' main are deferred to the facade
//! layer. The 'harmonizer' parameter — in the C++ a 'HfstTransducer*' whose
//! optimized-lookup backend is unpacked via 'harmonizer->implementation.hfst_ol'
//! — is taken here as the already-unpacked 'Option<&Transducer>'; the unpacking
//! step belongs to the deferred facade.

use std::collections::{BTreeMap, BTreeSet};

use crate::convert::{
    IndexPlaceholders, StatePlaceholder, TransitionPlaceholder,
    write_transitions_from_state_placeholders,
};
use crate::convert_transducer_format::ConversionFunctions;
use crate::hfst_basic_transducer::HfstBasicTransducer;
use crate::hfst_basic_transition::HfstBasicTransition;
use crate::hfst_data_types::{double_to_float, size_t_to_uint, size_t_to_ushort};
use crate::hfst_flag_diacritics::FdOperation;
use crate::hfst_symbol_defs::{internal_epsilon, is_epsilon};
use crate::pmatch::PmatchAlphabet;
use crate::transducer::{
    HeaderFlag, NO_SYMBOL_NUMBER, SymbolNumber, SymbolTable, Transducer, TransducerAlphabet,
    TransducerHeader, TransducerTable, TransitionTableIndex, TransitionW, TransitionWIndex, Weight,
    indexes_transition_index_table,
};

use crate::convert::HfstOlToBasicStateMap;

/* An auxiliary function. */
// [spec:hfst:def:convert-ol-transducer.hfst.implementations.hfst-ol-to-hfst-basic-add-state-fn]
// [spec:hfst:sem:convert-ol-transducer.hfst.implementations.hfst-ol-to-hfst-basic-add-state-fn]
pub fn hfst_ol_to_hfst_basic_add_state(
    t: &Transducer,
    basic: &mut HfstBasicTransducer,
    state_map: &mut HfstOlToBasicStateMap,
    weighted: bool,
    index: TransitionTableIndex,
    state_number: u32,
) -> u32 {
    let new_state = state_number;
    state_map.insert(index, new_state);

    if indexes_transition_index_table(index) {
        let transition_index = t.get_index(index);

        if transition_index.final_() {
            basic.add_state(new_state);
            // dynamic_cast to TransitionWIndex is the trait's virtual final_weight()
            let w = if weighted {
                double_to_float(transition_index.final_weight() as f64)
            } else {
                0.0f32
            };
            basic.set_final_weight(new_state, &w);
        }
    } else {
        // indexes transition table
        let transition = t.get_transition(index);

        if transition.final_() {
            basic.add_state(new_state);
            let w = if weighted {
                double_to_float(transition.get_weight() as f64)
            } else {
                0.0f32
            };
            basic.set_final_weight(new_state, &w);
        }
    }
    new_state
}

// [spec:hfst:def:convert-ol-transducer.hfst.implementations.string-set]
pub type StringSet = BTreeSet<String>;

// [spec:hfst:def:convert-ol-transducer.hfst.implementations.get-states-and-symbols-fn]
// [spec:hfst:sem:convert-ol-transducer.hfst.implementations.get-states-and-symbols-fn]
#[allow(clippy::too_many_arguments)]
pub fn get_states_and_symbols(
    t: &HfstBasicTransducer,
    state_placeholders: &mut Vec<StatePlaceholder>,
    symbol_table: &mut SymbolTable,
    seen_input_symbols: &mut SymbolNumber,
    flag_symbols: &mut BTreeSet<SymbolNumber>,
    harmonizer: Option<&Transducer>,
) {
    // Symbols must be in the following order in an optimized-lookup transducer:
    // 1) epsilon  2) other input symbols  3) symbols not used as input symbols.
    // Flag diacritics are indexed as if they were symbol #0 (epsilon) but
    // otherwise have a proper unique number; here they appear at the end of the
    // alphabet so they can be ignored for indexing.

    let mut input_symbols: StringSet = StringSet::new();
    let mut flag_diacritics: StringSet = StringSet::new();
    let mut other_symbols: StringSet = StringSet::new();

    let mut first_transition: u32 = 0;
    let mut state_number: usize = 0;
    while state_number < t.state_vector.len() {
        let mut final_w: Weight = 0.0;
        if t.is_final_state(state_number as u32) {
            final_w = t.get_final_weight(state_number as u32);
        }
        state_placeholders.push(StatePlaceholder::new(
            state_number as u32,
            t.is_final_state(state_number as u32),
            first_transition,
            final_w,
        ));
        first_transition += 1; // there's a padding entry between states
        for tr_it in t.transitions(state_number as u32).iter() {
            first_transition += 1;
            // If we don't already have a symbol table, collect symbols
            if harmonizer.is_none() {
                if FdOperation::is_diacritic(&tr_it.get_input_symbol())
                    || PmatchAlphabet::is_insertion(&tr_it.get_input_symbol())
                {
                    flag_diacritics.insert(tr_it.get_input_symbol());
                } else {
                    input_symbols.insert(tr_it.get_input_symbol());
                }
                other_symbols.insert(tr_it.get_output_symbol());
            }
        }
        state_number += 1;
    }

    // Finally add symbols from the source alphabet that don't appear in any
    // transitions to "other symbols".
    let source_alphabet = t.get_alphabet().clone();
    for it in source_alphabet.iter() {
        if !input_symbols.contains(it) && !flag_diacritics.contains(it) {
            other_symbols.insert(it.clone());
        }
    }

    let mut string_symbol_map: BTreeMap<String, SymbolNumber> = BTreeMap::new();

    // Collect symbols if we need to
    if harmonizer.is_none() {
        // 1) epsilon
        string_symbol_map.insert(
            internal_epsilon.to_string(),
            size_t_to_ushort(symbol_table.len()),
        );
        symbol_table.push(internal_epsilon.to_string());

        // 2) input symbols
        for it in input_symbols.iter() {
            if !is_epsilon(it) {
                string_symbol_map.insert(it.clone(), size_t_to_ushort(symbol_table.len()));
                symbol_table.push(it.clone());
                *seen_input_symbols += 1;
            }
        }

        // 3) Flag diacritics
        for it in flag_diacritics.iter() {
            if !is_epsilon(it) {
                string_symbol_map.insert(it.clone(), size_t_to_ushort(symbol_table.len()));
                flag_symbols.insert(symbol_table.len() as u16);
                symbol_table.push(it.clone());
                // don't increment seen_input_symbols - we use it for indexing
            }
        }

        // 4) non-input symbols
        for it in other_symbols.iter() {
            if !is_epsilon(it) && !input_symbols.contains(it) && !flag_diacritics.contains(it) {
                string_symbol_map.insert(it.clone(), size_t_to_ushort(symbol_table.len()));
                symbol_table.push(it.clone());
            }
        }
    } else {
        let harmonizer = harmonizer.unwrap();
        *symbol_table = harmonizer.get_symbol_table().clone();
        string_symbol_map = harmonizer.get_alphabet().build_string_symbol_map();
        *seen_input_symbols = harmonizer.get_header().input_symbol_count();
        for i in 0..symbol_table.len() {
            if harmonizer.get_alphabet().is_flag_diacritic(i as u16)
                || PmatchAlphabet::is_insertion(&symbol_table[i])
            {
                flag_symbols.insert(i as u16);
            }
        }
    }

    // Do a second pass over the transitions, figuring out everything about the
    // states except starting indices
    let mut state_number: usize = 0;
    while state_number < t.state_vector.len() {
        // collect into a temp so the immutable 't' borrow doesn't overlap the
        // mutable 'state_placeholders[state_number]' borrow
        let trs: Vec<HfstBasicTransition> = t.transitions(state_number as u32).clone();
        for tr_it in trs.iter() {
            let in_sym = string_symbol_map
                .get(&tr_it.get_input_symbol())
                .copied()
                .unwrap_or(0);
            let out_sym = string_symbol_map
                .get(&tr_it.get_output_symbol())
                .copied()
                .unwrap_or(0);
            // add input in case we're seeing it the first time
            state_placeholders[state_number].add_input(in_sym, flag_symbols);
            let target = tr_it.get_target_state();
            let trans = TransitionPlaceholder::new(target, in_sym, out_sym, tr_it.get_weight());
            state_placeholders[state_number].add_transition(trans);
        }
        state_number += 1;
    }
}

impl ConversionFunctions {
    /* Create an HfstBasicTransducer equivalent to hfst_ol::Transducer 't'. */
    // [spec:hfst:def:convert-ol-transducer.hfst.implementations.conversion-functions.hfst-ol-to-hfst-basic-transducer-fn]
    // [spec:hfst:sem:convert-ol-transducer.hfst.implementations.conversion-functions.hfst-ol-to-hfst-basic-transducer-fn]
    pub fn hfst_ol_to_hfst_basic_transducer(t: &Transducer) -> HfstBasicTransducer {
        let mut basic = HfstBasicTransducer::new();
        let weighted = t.get_header().probe_flag(HeaderFlag::Weighted);
        let symbols: SymbolTable = t.get_alphabet().get_symbol_table().clone();
        for it in symbols.iter() {
            basic.add_symbol_to_alphabet(it);
        }

        let mut agenda: Vec<TransitionTableIndex> = Vec::new();
        let mut state_map: HfstOlToBasicStateMap = BTreeMap::new();
        let mut state_number: u32 = 0;

        hfst_ol_to_hfst_basic_add_state(t, &mut basic, &mut state_map, weighted, 0, state_number);
        agenda.push(0);
        while let Some(current_index) = agenda.pop() {
            let current_state = state_map[&current_index];

            let transitions = t.get_transitions_from_state(current_index);
            for it in transitions.iter() {
                let transition = t.get_transition(*it);
                let target = transition.get_target();
                let in_sym = transition.get_input_symbol();
                let out_sym = transition.get_output_symbol();
                let weight = if weighted {
                    transition.get_weight()
                } else {
                    0.0
                };

                if !state_map.contains_key(&target) {
                    state_number += 1;
                    hfst_ol_to_hfst_basic_add_state(
                        t,
                        &mut basic,
                        &mut state_map,
                        weighted,
                        target,
                        state_number,
                    );
                    agenda.push(target);
                }
                basic.add_transition(
                    current_state,
                    &HfstBasicTransition::new_symbols(
                        state_map[&target],
                        symbols[in_sym as usize].clone(),
                        symbols[out_sym as usize].clone(),
                        weight,
                    ),
                    true,
                );
            }
        }

        basic
    }

    /* Create an hfst_ol::Transducer equivalent to HfstBasicTransducer 't'. */
    // [spec:hfst:def:convert-ol-transducer.hfst.implementations.conversion-functions.hfst-basic-transducer-to-hfst-ol-fn]
    // [spec:hfst:sem:convert-ol-transducer.hfst.implementations.conversion-functions.hfst-basic-transducer-to-hfst-ol-fn]
    #[allow(unused_assignments)] // C++ initialises previous_successful_index to 0
    pub fn hfst_basic_transducer_to_hfst_ol(
        t: &HfstBasicTransducer,
        weighted: bool,
        options: &str,
        harmonizer_ol: Option<&Transducer>,
    ) -> Transducer {
        let packing_aggression: f32 = 0.85;
        let floor_jump_threshold: i32 = 4; // a packing aggression parameter

        let empty_alphabet = options.contains("empty_alphabet");

        // The transition array is indexed starting from this constant
        const TA_OFFSET: u32 = 2147483648u32;

        let mut state_placeholders: Vec<StatePlaceholder> = Vec::new();
        let mut symbol_table: SymbolTable = SymbolTable::new();
        let mut seen_input_symbols: SymbolNumber = 1; // We always have epsilon
        let mut flag_symbols: BTreeSet<SymbolNumber> = BTreeSet::new();
        get_states_and_symbols(
            t,
            &mut state_placeholders,
            &mut symbol_table,
            &mut seen_input_symbols,
            &mut flag_symbols,
            harmonizer_ol,
        );

        let mut used_indices = IndexPlaceholders::new();

        // Assign starting indices (or determine a state is "simple"). The
        // starting state has index 0 and always gets a TIA entry.
        let mut first_available_index: u32 = 0;
        let mut previous_first_index: u32 = 0;
        let mut previous_successful_index: u32 = 0;
        let mut floor_stuck_counter: i32 = 0;
        for idx in 0..state_placeholders.len() {
            if state_placeholders[idx].is_simple() {
                continue;
            }
            let mut i = first_available_index;

            // While this index is not suitable for a starting index, keep going
            while !used_indices.fits(&state_placeholders[idx], &flag_symbols, i) {
                i += 1;
            }
            state_placeholders[idx].start_index = i;
            previous_successful_index = i;
            // Insert a finality marker and mark all the used indices
            let state_number = state_placeholders[idx].state_number;
            used_indices.assign(i, state_number, NO_SYMBOL_NUMBER);
            for tr_idx in 0..state_placeholders[idx].transition_placeholders.len() {
                let mut index_offset =
                    state_placeholders[idx].transition_placeholders[tr_idx][0].input;
                if flag_symbols.contains(&index_offset) {
                    index_offset = 0;
                }
                used_indices.assign(i + index_offset as u32 + 1, state_number, index_offset);
            }

            while used_indices.unsuitable(
                first_available_index,
                seen_input_symbols,
                packing_aggression,
            ) {
                first_available_index += 1;
            }
            if first_available_index == previous_first_index {
                if floor_stuck_counter > floor_jump_threshold {
                    first_available_index = previous_successful_index + 1;
                    floor_stuck_counter = 0;
                    previous_first_index = first_available_index;
                } else {
                    floor_stuck_counter += 1;
                }
            } else {
                previous_first_index = first_available_index;
                floor_stuck_counter = 0;
            }
        }

        // Now for each index entry we write its input symbol and target
        let mut windex_table: TransducerTable<TransitionWIndex> = TransducerTable::new();

        let mut greatest_index: u32 = 0;
        if !used_indices.indices.is_empty() {
            greatest_index = size_t_to_uint(used_indices.indices.len() - 1);
        }

        for i in 0..=greatest_index {
            if !used_indices.used(i) {
                // blank entries
                windex_table.append(TransitionWIndex::new());
            } else if used_indices.get_target(i).1 == NO_SYMBOL_NUMBER {
                // finality markers
                let first = used_indices.get_target(i).0 as usize;
                if state_placeholders[first].final_ {
                    windex_table.append(TransitionWIndex::create_final_weight(
                        state_placeholders[first].final_weight,
                    ));
                } else {
                    windex_table.append(TransitionWIndex::new());
                }
            } else {
                // actual entries
                let idx = used_indices.get_target(i).0 as usize;
                let sym = used_indices.get_target(i).1;
                let target = state_placeholders[idx].first_transition
                    + state_placeholders[idx].symbol_offset(sym, &flag_symbols)
                    + TA_OFFSET;
                windex_table.append(TransitionWIndex::new_values(sym, target));
            }
        }

        for _ in 0..seen_input_symbols {
            windex_table.append(TransitionWIndex::new()); // padding
        }

        // Now write the transition table
        let mut wtransition_table: TransducerTable<TransitionW> = TransducerTable::new();

        write_transitions_from_state_placeholders(
            &mut wtransition_table,
            &state_placeholders,
            &flag_symbols,
        );

        if empty_alphabet {
            symbol_table.clear();
            seen_input_symbols = 0;
        }

        let alphabet = TransducerAlphabet::new_symboltable(&symbol_table);
        let header = TransducerHeader::new_sizes(
            seen_input_symbols,
            size_t_to_ushort(symbol_table.len()),
            windex_table.size(),
            wtransition_table.size(),
            weighted,
        );
        Transducer::new_from_tables_weighted(&header, &alphabet, windex_table, wtransition_table)
    }
}
