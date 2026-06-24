# libhfst/src/implementations/ConvertSfstTransducer.cc

> [spec:hfst:def:convert-sfst-transducer.hfst.implementations.conversion-functions.hfst-basic-transducer-to-sfst-fn]
> SFST::Transducer * ConversionFunctions

> [spec:hfst:sem:convert-sfst-transducer.hfst.implementations.conversion-functions.hfst-basic-transducer-to-sfst-fn]
> Converts a `const HfstBasicTransducer * net` into a newly allocated `SFST::Transducer *` and returns it. Steps:
> 1. Allocate a new `SFST::Transducer t`. Seed its alphabet with the two HFST special symbols: add `internal_unknown` with number 1 and `internal_identity` with number 2 via `t->alphabet.add_symbol(name, number)`.
> 2. Copy the alphabet: iterate over `net->alphabet`; for each symbol that is not epsilon, not unknown, and not identity (checked via `is_epsilon`/`is_unknown`/`is_identity`), add it to `t->alphabet` with `net->get_symbol_number(symbol)` as its SFST number.
> 3. Build the recoding from HFST symbol numbers to SFST numbers: get `symbol_map` (string -> unsigned int) via `SfstTransducer::get_symbol_map(t)`; erase the entry for SFST's internal epsilon key `"<>"` and instead map `internal_epsilon` to 0. Then compute `harm = HfstTropicalTransducerTransitionData::get_reverse_harmonization_vector(symbol_map)`, a vector indexed by HFST symbol number giving the SFST symbol number.
> 4. Build a `state_vector` mapping HFST state numbers to `SFST::Node*`: push `t->root_node()` for state 0, then for `i` from 1 up to and including `net->get_max_state()`, push a fresh `t->new_node()`.
> 5. Iterate over all states of `net` (index `source_state` starting at 0, incremented after each state). For each transition in the state, build an `SFST::Label(harm.at(input_number), harm.at(output_number))` from the transition's input and output numbers, and add it as an arc via `state_vector[source_state]->add_arc(label, state_vector[target_state], t)` where `target_state` is the transition's target state.
> 6. Mark final states: iterate over `net->final_weight_map`; for each final state number, if it is `>= state_vector.size()` (noted as should-not-happen) push a new node, then call `set_final(1)` on `state_vector[state_number]`. Weights are discarded (SFST is unweighted).
> 7. Return `t`. Caller owns the allocation.

> [spec:hfst:def:convert-sfst-transducer.hfst.implementations.conversion-functions.sfst-to-hfst-basic-transducer-fn]
> HfstBasicTransducer * ConversionFunctions::sfst_to_hfst_basic_transducer

> [spec:hfst:sem:convert-sfst-transducer.hfst.implementations.conversion-functions.sfst-to-hfst-basic-transducer-fn]
> Converts an `SFST::Transducer * t` into a newly allocated `HfstBasicTransducer *` and returns it. Steps:
> 1. Allocate a new `HfstBasicTransducer net`.
> 2. Build the symbol recoding: get `symbol_vector = SfstTransducer::get_symbol_vector(t)` (a `StringVector` indexed by SFST symbol number). Overwrite element 0 (SFST's internal `"<>"` epsilon) with HFST's `"@_EPSILON_SYMBOL_@"`. Compute `harmonization_vector = HfstTropicalTransducerTransitionData::get_harmonization_vector(symbol_vector)`, a vector indexed by SFST symbol number giving the corresponding HFST symbol number.
> 3. Determine node count: call `t->nodeindexing(&indexing)` to assign indices to all nodes and fill `indexing`; set `number_of_nodes = indexing.size()` and pre-size the state vector via `net->initialize_state_vector(number_of_nodes)`.
> 4. Visited-mark handling: if `t->root_node()->check_visited(VMARK)` is true, increment the global `VMARK` so this traversal uses a fresh mark value.
> 5. Recursively copy transitions: call the helper `sfst_to_hfst_basic_transducer(t->root_node(), net, harmonization_vector)`. That helper, for each not-yet-visited node (keyed on `VMARK`): counts the node's arcs and calls `net->initialize_transition_vector(node->index, number_of_arcs)`; then for each arc adds an `HfstBasicTransition(target_node->index, harmonization_vector.at(arc.label.lower_char()), harmonization_vector.at(arc.label.upper_char()), weight 0, false)` to `net` at `node->index`; if the node is final, calls `net->set_final_weight(node->index, 0)`; then recurses into every arc's target node.
> 6. Copy the alphabet: iterate `t->alphabet.get_char_map()` (number -> name); for each entry whose number is not 0 (i.e. excluding the epsilon `"<>"`), insert the symbol name into `net->alphabet`.
> 7. Return `net`. Caller owns the allocation. (The `DEBUG_CONVERSION` blocks capturing and asserting alphabet equality before/after are compiled out unless that macro is defined.)

> [spec:hfst:def:convert-sfst-transducer.main-fn]
> int main(int argc, char * argv[])

> [spec:hfst:sem:convert-sfst-transducer.main-fn]
> Test stub compiled only when `MAIN_TEST` is defined. Prints `"Unit tests for " __FILE__ ":"` followed by a newline to `std::cout`, then prints `"ok"` and a newline, then returns 0. Performs no actual testing.

