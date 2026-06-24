# libhfst/src/implementations/ConvertOlTransducer.cc

> [spec:hfst:def:convert-ol-transducer.hfst.implementations.conversion-functions.hfst-basic-transducer-to-hfst-ol-fn]
> hfst_ol::Transducer * ConversionFunctions

> [spec:hfst:sem:convert-ol-transducer.hfst.implementations.conversion-functions.hfst-basic-transducer-to-hfst-ol-fn]
> Builds an `hfst_ol::Transducer*` equivalent to const HfstBasicTransducer `t`. Parameters: `weighted` (whether the result is weighted), `options` (a string; if it contains the substring "empty_alphabet" the alphabet is emptied at the end), and `harmonizer` (an optional HfstTransducer whose symbol table is reused).
> Defines constants: `packing_aggression = 0.85f`, `floor_jump_threshold = 4`, and `TA_OFFSET = 2147483648u` (the offset at which the transition array is indexed).
> If `harmonizer != NULL`, set `harmonizer_ol = harmonizer->implementation.hfst_ol`; otherwise `harmonizer_ol = NULL`.
> Declares `state_placeholders` (vector<StatePlaceholder>), empty `symbol_table`, `seen_input_symbols = 1` (epsilon always counted), and empty set `flag_symbols`. Calls `get_states_and_symbols(t, state_placeholders, symbol_table, seen_input_symbols, flag_symbols, harmonizer_ol)` to populate these.
> Allocates a new `hfst_ol::IndexPlaceholders* used_indices`. Then assigns starting indices to each non-simple state to build the transition index table (TIA): iterate over `state_placeholders` in order; skip states where `it->is_simple()`. For the rest, start at `i = first_available_index` and increment `i` while `!used_indices->fits(*it, flag_symbols, i)`. Set `it->start_index = i` and `previous_successful_index = i`. Place a finality marker via `used_indices->assign(i, it->state_number, NO_SYMBOL_NUMBER)`. For each transition group `tr_it` in `it->transition_placeholders`, compute `index_offset = tr_it->at(0).input`; if `index_offset` is in `flag_symbols`, set it to 0; then `used_indices->assign(i + index_offset + 1, it->state_number, index_offset)`.
> After assigning a state, advance `first_available_index` while `used_indices->unsuitable(first_available_index, seen_input_symbols, packing_aggression)`. Anti-stall logic: if `first_available_index == previous_first_index`, and `floor_stuck_counter > floor_jump_threshold`, jump `first_available_index = previous_successful_index + 1`, reset `floor_stuck_counter = 0`, set `previous_first_index = first_available_index`; otherwise increment `floor_stuck_counter`. If `first_available_index` changed, set `previous_first_index = first_available_index` and reset `floor_stuck_counter = 0`.
> Build the index table `windex_table` (TransducerTable<TransitionWIndex>): compute `greatest_index = indices.size()-1` (0 if indices is empty). For each `i` in `0..=greatest_index`: if `!used_indices->used(i)`, append a default `TransitionWIndex()` (blank); else if `get_target(i).second == NO_SYMBOL_NUMBER` (finality marker), append `TransitionWIndex::create_final(state_placeholders[target.first].final_weight)` when that state is final, else a default `TransitionWIndex()`; else (actual entry) with `idx = get_target(i).first` and `sym = get_target(i).second`, append `TransitionWIndex(sym, state_placeholders[idx].first_transition + state_placeholders[idx].symbol_offset(sym, flag_symbols) + TA_OFFSET)`.
> Delete `used_indices`. Append `seen_input_symbols` default `TransitionWIndex()` padding entries to `windex_table`.
> Build `wtransition_table` (TransducerTable<TransitionW>) by calling `hfst_ol::write_transitions_from_state_placeholders(wtransition_table, state_placeholders, flag_symbols)`.
> If `empty_alphabet`, clear `symbol_table` and set `seen_input_symbols = 0`.
> Construct `TransducerAlphabet alphabet(symbol_table)` and `TransducerHeader header(seen_input_symbols, symbol_table.size(), windex_table.size(), wtransition_table.size(), weighted)`. Return a new `hfst_ol::Transducer(header, alphabet, windex_table, wtransition_table)`.

> [spec:hfst:def:convert-ol-transducer.hfst.implementations.conversion-functions.hfst-ol-to-hfst-basic-transducer-fn]
> HfstBasicTransducer * ConversionFunctions

> [spec:hfst:sem:convert-ol-transducer.hfst.implementations.conversion-functions.hfst-ol-to-hfst-basic-transducer-fn]
> Builds and returns a new `HfstBasicTransducer*` equivalent to the optimized-lookup transducer `t`.
> Allocates `basic = new HfstBasicTransducer()`. Reads `weighted = t->get_header().probe_flag(hfst_ol::Weighted)`. Gets `symbols = t->get_alphabet().get_symbol_table()` and adds each symbol in it to `basic`'s alphabet via `basic->add_symbol_to_alphabet`.
> Declares `agenda` (vector<TransitionTableIndex>), `state_map` (HfstOlToBasicStateMap), and `state_number = 0`. Calls `hfst_ol_to_hfst_basic_add_state(t, basic, state_map, weighted, 0, state_number)` to register the start state (index 0 -> state 0, with finality), then pushes 0 onto `agenda`.
> Worklist loop while `agenda` is non-empty: pop `current_index` from the back of `agenda`; `current_state = state_map[current_index]`. Get `transitions = t->get_transitions_from_state(current_index)`. For each transition index `it` in that set: fetch `transition = t->get_transition(*it)`. If `transition.get_target()` is not yet in `state_map`, increment `state_number`, call `hfst_ol_to_hfst_basic_add_state(t, basic, state_map, weighted, transition.get_target(), state_number)`, and push the target onto `agenda`. Then add a transition to `basic` from `current_state` to a `HfstBasicTransition(state_map[target], symbols[transition.get_input_symbol()], symbols[transition.get_output_symbol()], weighted ? TransitionW(transition).get_weight() : 0)`.
> Returns `basic`.

> [spec:hfst:def:convert-ol-transducer.hfst.implementations.conversion-functions.hfst-ol-to-hfst-transducer-fn]
> HfstTransducer * ConversionFunctions::hfst_ol_to_hfst_transducer(

> [spec:hfst:sem:convert-ol-transducer.hfst.implementations.conversion-functions.hfst-ol-to-hfst-transducer-fn]
> Wraps a raw `hfst_ol::Transducer* t` into a heap-allocated `HfstTransducer*`.
> Chooses `type = t->is_weighted() ? HFST_OLW_TYPE : HFST_OL_TYPE`. Allocates `retval = new HfstTransducer(type)`. Deletes the default-constructed `retval->implementation.hfst_ol`, then sets it to a new deep copy `new hfst_ol::Transducer(*t)`. Returns `retval`.

> [spec:hfst:def:convert-ol-transducer.hfst.implementations.conversion-functions.hfst-transducer-to-hfst-ol-fn]
> hfst_ol::Transducer * ConversionFunctions::hfst_transducer_to_hfst_ol(

> [spec:hfst:sem:convert-ol-transducer.hfst.implementations.conversion-functions.hfst-transducer-to-hfst-ol-fn]
> Returns the raw optimized-lookup backend of HfstTransducer `t`.
> If `t->type` is neither `HFST_OL_TYPE` nor `HFST_OLW_TYPE`, calls `t->convert(HFST_OLW_TYPE)` to convert `t` in place to weighted optimized lookup. Then returns `t->implementation.hfst_ol` (a borrowed pointer into `t`, not a copy).

> [spec:hfst:def:convert-ol-transducer.hfst.implementations.get-states-and-symbols-fn]
> void get_states_and_symbols(

> [spec:hfst:sem:convert-ol-transducer.hfst.implementations.get-states-and-symbols-fn]
> Scans HfstBasicTransducer `t` to populate, by reference: `state_placeholders` (per-state info), `symbol_table` (the OL alphabet in required order), `seen_input_symbols` (count of real input symbols), and `flag_symbols` (set of SymbolNumbers that are flag diacritics/insertions). `harmonizer` is an optional OL transducer whose symbol table is reused instead of collecting one.
> Required symbol ordering for an OL transducer: (1) epsilon, (2) other input symbols, (3) symbols not used as input. Flag diacritics are indexed as if they were symbol #0 (epsilon) but still get a unique number; here they are placed at the end of the alphabet so they can be ignored for indexing.
> Allocates three temporary `StringSet*`: `input_symbols`, `flag_diacritics`, `other_symbols`.
> First pass: iterate states of `t` with `state_number` starting at 0 and `first_transition` starting at 0. For each state, compute `final_w` = `t->get_final_weight(state_number)` if `t->is_final_state(state_number)` else 0.0; push `StatePlaceholder(state_number, is_final, first_transition, final_w)` onto `state_placeholders`; increment `first_transition` once (padding entry between states). For each transition of the state: increment `first_transition`; and if `harmonizer == NULL`, classify the input symbol — if `FdOperation::is_diacritic(input)` or `PmatchAlphabet::is_insertion(input)` insert it into `flag_diacritics`, else into `input_symbols`; and insert the output symbol into `other_symbols`. Increment `state_number`.
> Then add every symbol from `t->get_alphabet()` that is not in `input_symbols` and not in `flag_diacritics` into `other_symbols`.
> Build `string_symbol_map` (string -> SymbolNumber). If `harmonizer == NULL`, fill the symbol table in order: (1) map `internal_epsilon` to index 0 and push it; (2) for each `input_symbols` entry that is not epsilon, map it to the current `symbol_table.size()`, push it, and increment `seen_input_symbols`; (3) for each `flag_diacritics` entry that is not epsilon, map it, insert its index into `flag_symbols`, push it (do NOT increment `seen_input_symbols`); (4) for each `other_symbols` entry not epsilon and not already in input/flag sets, map it and push it.
> Else (harmonizer given): set `symbol_table = harmonizer->get_symbol_table()`, `string_symbol_map = harmonizer->get_alphabet().build_string_symbol_map()`, `seen_input_symbols = harmonizer->get_header().input_symbol_count()`, and for each index `i` in the table, if `harmonizer->get_alphabet().is_flag_diacritic(i)` or `PmatchAlphabet::is_insertion(symbol_table[i])` insert `i` into `flag_symbols`.
> Deletes the three temporary StringSets.
> Second pass: with `state_number` reset to 0, iterate states and their transitions; for each transition call `state_placeholders[state_number].add_input(string_symbol_map[input], flag_symbols)` and then `add_transition(TransitionPlaceholder(target_state, string_symbol_map[input], string_symbol_map[output], weight))`. Increment `state_number` per state. Returns nothing (void).

> [spec:hfst:def:convert-ol-transducer.hfst.implementations.hfst-ol-to-hfst-basic-add-state-fn]
> unsigned int hfst_ol_to_hfst_basic_add_state

> [spec:hfst:sem:convert-ol-transducer.hfst.implementations.hfst-ol-to-hfst-basic-add-state-fn]
> Auxiliary helper that registers one OL state (identified by `index`) as basic state number `state_number`, returning that number.
> Sets `new_state = state_number` and records `state_map[index] = new_state`.
> If `hfst_ol::indexes_transition_index_table(index)` is true, fetch `transition_index = t->get_index(index)`; if it is final, call `basic->add_state(new_state)` and set its final weight to (if `weighted`) `hfst::double_to_float(((const TransitionWIndex&)transition_index).final_weight())`, else 0.0f.
> Otherwise (index addresses the transition table), fetch `transition = t->get_transition(index)`; if it is final, call `basic->add_state(new_state)` and set its final weight to (if `weighted`) `hfst::double_to_float(((const TransitionW&)transition).get_weight())`, else 0.0f.
> Non-final states get no explicit `add_state`/`set_final_weight` here. Returns `new_state`.

> [spec:hfst:def:convert-ol-transducer.hfst.implementations.string-set]
> typedef std::set<std::string> StringSet

> [spec:hfst:def:convert-ol-transducer.main-fn]
> int main(int argc, char * argv[])

> [spec:hfst:sem:convert-ol-transducer.main-fn]
> Standalone unit-test entry point (compiled only when MAIN_TEST is defined). Prints "Unit tests for <file>:".
> If `HfstTransducer::is_implementation_type_available(TROPICAL_OPENFST_TYPE)` is false, prints "No tests run in absence of OpenFst library" and returns 0.
> Builds a small HfstBasicTransducer `basic`: adds states 1 and 2; transitions (0->1 "a":"a"), (1->2 "a":"a"), (1->2 "a":"b"), and (0->2 "a":internal_epsilon), all weight 0; sets state 2 final with weight 0. Copies it to `basic_w` and sets state 2 final weight to 1.0.
> Converts both to OL: `basic_ol = hfst_basic_transducer_to_hfst_ol(&basic, false)` and `basic_olw = hfst_basic_transducer_to_hfst_ol(&basic_w, true)`. Converts those back to basic with `hfst_ol_to_hfst_basic_transducer`, giving `basic_converted` and `basic_converted_w`.
> Builds reference transducers `cmp(basic, TROPICAL_OPENFST_TYPE)` and `cmp_w(basic_w, TROPICAL_OPENFST_TYPE)`. Asserts `cmp.compare(HfstTransducer(*basic_converted, TROPICAL_OPENFST_TYPE))` and the weighted analogue both hold (round-trip equality).
> Deletes the four allocated transducers, prints "ok", and returns 0.

