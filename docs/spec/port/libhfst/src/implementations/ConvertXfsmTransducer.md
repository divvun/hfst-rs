# libhfst/src/implementations/ConvertXfsmTransducer.cc

> [spec:hfst:def:convert-xfsm-transducer.hfst.implementations.conversion-functions.hfst-basic-transducer-to-xfsm-fn]
> NETptr ConversionFunctions

> [spec:hfst:sem:convert-xfsm-transducer.hfst.implementations.conversion-functions.hfst-basic-transducer-to-xfsm-fn]
> Build an xfsm NET equivalent to the given const HfstBasicTransducer pointer `hfst_fsm`, returning the resulting NETptr.
> Create `result = null_net()` (which already contains an initial/start state). Maintain `state_vector`, a vector mapping HfstBasicTransducer state numbers (= iteration index) to xfsm STATEptr.
> First pass — copy states: iterate states with `hfst_fsm->begin()`/`end()`, keeping a counter `fsm_state` starting at 0. For `fsm_state == 0`, push `result->start.state` onto `state_vector` (finality of the initial state is deferred). For each `fsm_state != 0`, call `add_state_to_net(result, hfst_fsm->is_final_state(fsm_state) ? 1 : 0)` and push the returned STATEptr. Increment `fsm_state` each iteration.
> Second pass — copy transitions: iterate states again with a `source_state` counter starting at 0; let `xfsm_source_state = state_vector.at(source_state)`. For each transition `tr_it` in the current state, read `isymbol = get_input_symbol()`, `osymbol = get_output_symbol()`, `target_state = get_target_state()`. Compute label id `ti`: if `isymbol == hfst::internal_identity`, then require `osymbol == hfst::internal_identity` (otherwise throw the C-string "identity symbol cannot be on one side only") and set `ti = OTHER` (atomic OTHER label); else set `input_id = XfsmTransducer::hfst_symbol_to_xfsm_symbol(isymbol)`, `output_id = ...(osymbol)`, and `ti = id_pair_to_id(input_id, output_id)`. (Note: an earlier `ti = XfsmTransducer::symbol_pair_to_label_id(isymbol, osymbol)` is computed but immediately overwritten by the branch.) Let `xfsm_target_state = state_vector.at(target_state)`. Call `add_arc_to_state(result, xfsm_source_state, ti, xfsm_target_state, NULL, 0)`; if it returns NULL, throw the C-string "add_arc_to_state failed". Increment `source_state` per state.
> After all transitions: if `hfst_fsm->is_final_state(0)` (initial state is final), reassign `result = optional_net(result, 0)` to make the result optional.
> Copy alphabet: get `ap = net_sigma(result)`; iterate over `hfst_fsm->get_alphabet()`, and for each symbol skip it if `hfst::is_epsilon`, `hfst::is_unknown`, or `hfst::is_identity` is true, otherwise call `alph_add_to(ap, XfsmTransducer::hfst_symbol_to_xfsm_symbol(it->c_str()), DONT_KEEP)`.
> Return `result`.

> [spec:hfst:def:convert-xfsm-transducer.hfst.implementations.conversion-functions.xfsm-to-hfst-basic-transducer-fn]
> HfstBasicTransducer * ConversionFunctions

> [spec:hfst:sem:convert-xfsm-transducer.hfst.implementations.conversion-functions.xfsm-to-hfst-basic-transducer-fn]
> Build a new heap-allocated `HfstBasicTransducer * result` equivalent to the xfsm NET `t`, and return it.
> Maintain `xfsm_to_hfst_state`, a `std::map<STATEptr, HfstState>`. Let `start_ptr = t->start.state`.
> First, create states in `result`: iterate the xfsm state list `state_ptr = t->body.states` following `state_ptr->next`; for every state that is NOT the start state, call `result->add_state()` (the result already has its own initial state, so the start state needs no new state). Discard the returned ids here.
> Second, assign the mapping and finality: set `result_state = result->get_max_state()`. xfsm states are stored as a stack, so numbering proceeds from the largest result state number downward. Iterate the xfsm states again: for the start state, map it to HfstState 0 and, if `state_ptr->final != 0`, call `result->set_final_weight(0, 0)`; for any other state, map it to the current `result_state`, and if `state_ptr->final != 0` call `result->set_final_weight(result_state, 0)`, then decrement `result_state`.
> Third, copy transitions: iterate the xfsm states again; for each, walk its arc list `arc_ptr = state_ptr->arc.set` following `arc_ptr->next`. For each arc, take `label_id = arc_ptr->label`, decode it into `isymbol`/`osymbol` via `XfsmTransducer::label_id_to_symbol_pair(label_id, isymbol, osymbol)`, read `target_state_ptr = arc_ptr->destination`, construct `HfstBasicTransition tr(xfsm_to_hfst_state[target_state_ptr], isymbol, osymbol, 0)`, and call `result->add_transition(xfsm_to_hfst_state[state_ptr], tr)`.
> Finally, call `copy_xfsm_alphabet_into_hfst_alphabet(t, result)` to copy the sigma into the result's alphabet, then return `result`.

> [spec:hfst:def:convert-xfsm-transducer.hfst.implementations.copy-xfsm-alphabet-into-hfst-alphabet-fn]
> static void copy_xfsm_alphabet_into_hfst_alphabet(NETptr t, HfstBasicTransducer * fsm)

> [spec:hfst:sem:convert-xfsm-transducer.hfst.implementations.copy-xfsm-alphabet-into-hfst-alphabet-fn]
> Insert every symbol of the xfsm transducer `t`'s alphabet (sigma) into the alphabet of HfstBasicTransducer `fsm`.
> Obtain `alpha_ptr = net_sigma(t)`, then `alpha_it_ptr = start_alph_iterator(NULL, alpha_ptr)`. Read the first id with `label_id = next_alph_id(alpha_it_ptr)`.
> Loop while `label_id != ID_NO_SYMBOL`: convert the id to a string via `symbol = XfsmTransducer::xfsm_symbol_to_hfst_symbol(label_id)`, call `fsm->add_symbol_to_alphabet(symbol)`, then advance with `label_id = next_alph_id(alpha_it_ptr)`.
> Returns void; the side effect is the added alphabet symbols on `fsm`.

> [spec:hfst:def:convert-xfsm-transducer.main-fn]
> int main(int argc, char * argv[])

> [spec:hfst:sem:convert-xfsm-transducer.main-fn]
> Compiled only when `MAIN_TEST` is defined. The unit-test entry point: print `"Unit tests for " __FILE__ ":"` followed by a newline to stdout, then print `"ok"` followed by a newline, and return 0. It performs no actual testing.

