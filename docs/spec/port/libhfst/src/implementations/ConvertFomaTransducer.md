# libhfst/src/implementations/ConvertFomaTransducer.cc

> [spec:hfst:def:convert-foma-transducer.hfst.implementations.conversion-functions.foma-to-hfst-basic-transducer-fn]
> HfstBasicTransducer * ConversionFunctions

> [spec:hfst:sem:convert-foma-transducer.hfst.implementations.conversion-functions.foma-to-hfst-basic-transducer-fn]
> Builds and returns a newly-allocated `HfstBasicTransducer *` equivalent to foma transducer `t`.
> Steps:
> 1. Call `FomaTransducer::get_symbol_vector(t)` to obtain a `StringVector symbol_vector`, then
>    call `HfstTropicalTransducerTransitionData::get_harmonization_vector(symbol_vector)` to obtain
>    `std::vector<unsigned int> harmonization_vector` (maps foma symbol numbers to internal symbol numbers).
> 2. Allocate `net = new HfstBasicTransducer()`. Set `fsm = t->states`, `start_state_id = -1`,
>    `start_state_found = false`.
> 3. Iterate `i` from 0 while `(fsm+i)->state_no != -1` (the array is terminated by a state_no of -1).
>    For each `fsm+i`:
>    a. If `(fsm+i)->target != -1`: compute `number_of_transitions = get_number_of_transitions(fsm+i)`
>       and call `net->initialize_transition_vector((fsm+i)->state_no, number_of_transitions)`.
>    b. If `(fsm+i)->start_state == 1`: call `handle_start_state(fsm+i, start_state_id, start_state_found)`.
>    c. If `(fsm+i)->target != -1`: add a transition to `net` from `(fsm+i)->state_no` to target
>       `(fsm+i)->target` with input symbol `harmonization_vector.at((fsm+i)->in)`, output symbol
>       `harmonization_vector.at((fsm+i)->out)`, weight 0; via `net->add_transition(state_no,
>       HfstBasicTransition(target, in_num, out_num, 0, false), false)` (last bool false = do not add
>       symbols to alphabet here).
>    d. If `(fsm+i)->final_state == 1`: call `net->set_final_weight((fsm+i)->state_no, 0)`.
> 4. After the loop, if `! start_state_found`: call `copy_alphabet(t, net)` (the foma-to-net overload)
>    and return `net` immediately (treating it as an empty transducer; no exception is thrown).
> 5. Otherwise, if `start_state_id != 0`: call `net->swap_state_numbers(start_state_id, 0)` so the
>    start state becomes state 0.
> 6. Call `copy_alphabet(t, net)` to copy the alphabet, then return `net`.
> Note: `.at()` on the harmonization vector throws `std::out_of_range` if a symbol number is out of bounds.

> [spec:hfst:def:convert-foma-transducer.hfst.implementations.conversion-functions.hfst-basic-transducer-to-foma-fn]
> fsm * ConversionFunctions

> [spec:hfst:sem:convert-foma-transducer.hfst.implementations.conversion-functions.hfst-basic-transducer-to-foma-fn]
> Builds and returns a newly-constructed foma `fsm *` equivalent to `hfst_fsm`.
> Steps:
> 1. Initialize a foma construct handle: `h = fsm_construct_init(const_cast<char*>(""))` (empty name).
> 2. Iterate over states via `hfst_fsm->begin()..end()`, tracking a counter `source_state` starting at 0
>    that gives the source state number for the current state position. For each state, iterate its
>    transitions (`it->begin()..it->end()`); for each transition get the input symbol string and output
>    symbol string from `tr_it->get_transition_data()`, and call
>    `fsm_construct_add_arc(h, (int)source_state, (int)tr_it->get_target_state(), input, output)`.
>    After processing a state's transitions, increment `source_state`.
> 3. Iterate over `hfst_fsm->final_weight_map` (a FinalWeightMap); for each entry call
>    `fsm_construct_set_final(h, (int)it->first)` to mark that state final (weight is ignored).
> 4. Call `copy_alphabet(hfst_fsm, h)` (the net-to-handle overload) to copy the alphabet.
> 5. Call `fsm_construct_set_initial(h, 0)` to set state 0 as the initial state.
> 6. Finalize: `net = fsm_construct_done(h)`, then `fsm_count(net)`, then `net = fsm_topsort(net)`.
> 7. Return `net`.

> [spec:hfst:def:convert-foma-transducer.hfst.implementations.copy-alphabet-fn]
> static void copy_alphabet(const HfstBasicTransducer * hfst_fsm,

> [spec:hfst:sem:convert-foma-transducer.hfst.implementations.copy-alphabet-fn]
> Copies the alphabet of HFST basic transducer `hfst_fsm` into foma construct handle `h`.
> Obtains `alpha = hfst_fsm->get_alphabet()` (an `HfstAlphabet`, a set of symbol strings) and iterates
> over it. For each symbol string `it`, takes its C string `symbol = it->c_str()`; if
> `fsm_construct_check_symbol(h, symbol) == -1` (symbol not already present in the handle), calls
> `fsm_construct_add_symbol(h, symbol)` to add it. Symbols already present are skipped. Returns nothing
> (mutates `h`).

> [spec:hfst:def:convert-foma-transducer.hfst.implementations.get-number-of-transitions-fn]
> static unsigned int get_number_of_transitions(struct fsm_state * fsm)

> [spec:hfst:sem:convert-foma-transducer.hfst.implementations.get-number-of-transitions-fn]
> Counts the number of transitions leaving the foma state pointed to by `fsm`.
> Initializes `number_of_transitions = 0`. Iterates `j` from 0, incrementing the counter once per
> iteration, while both conditions hold: `(fsm+j)->target != -1` (not the array terminator / no-transition
> marker) AND `(fsm+j)->state_no == fsm->state_no` (still the same source state). Returns the count as an
> `unsigned int`. Relies on the foma state array storing all transitions of one state contiguously.

> [spec:hfst:def:convert-foma-transducer.hfst.implementations.handle-start-state-fn]
> static void handle_start_state

> [spec:hfst:sem:convert-foma-transducer.hfst.implementations.handle-start-state-fn]
> Handles encountering a start state `fsm_` in a foma transducer, using mutable references
> `start_state_id` (the recorded start state number, -1 if undefined) and `start_state_found` (whether a
> start state has been seen).
> - If `! start_state_found`: this is the first start state seen — set `start_state_id = fsm_->state_no`
>   and set `start_state_found = true`.
> - Else if `fsm_->state_no == start_state_id`: the same start state is encountered again — do nothing.
> - Else (a different start state number than already recorded): throw `HfstFatalException` with message
>   "Foma transducer has more than one start state" (via `HFST_THROW_MESSAGE`).
> Returns nothing.

> [spec:hfst:def:convert-foma-transducer.main-fn]
> int main(int argc, char * argv[])

> [spec:hfst:sem:convert-foma-transducer.main-fn]
> The unit-test entry point compiled only when `MAIN_TEST` is defined. Prints
> `Unit tests for <__FILE__>:` followed by `ok` to standard output (each on its own line), then returns 0.
> Performs no actual testing.

