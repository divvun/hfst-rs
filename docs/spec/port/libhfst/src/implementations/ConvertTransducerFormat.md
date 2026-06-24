# libhfst/src/implementations/ConvertTransducerFormat.cc, libhfst/src/implementations/ConvertTransducerFormat.h

> [spec:hfst:def:convert-transducer-format.fst.log-fst]
> typedef fst::VectorFst<LogArc> LogFst

> [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions]
> class ConversionFunctions {
>   static StringVector number_to_string_vector;
>   static String2NumberMap string_to_number_map;
> }

> [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.foma-to-hfst-basic-transducer-fn]
> static HfstBasicTransducer * foma_to_hfst_basic_transducer(fsm * t)

> [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.foma-to-hfst-basic-transducer-fn]
> Converts a foma `fsm * t` to a newly allocated `HfstBasicTransducer *`.
> Steps:
> 1. Build a `symbol_vector` via `FomaTransducer::get_symbol_vector(t)` and a
>    `harmonization_vector` via `HfstTropicalTransducerTransitionData::get_harmonization_vector(symbol_vector)`,
>    which recodes foma symbol numbers into HFST internal symbol numbers.
> 2. Allocate a new empty `net`. Iterate over the flat array `t->states` indexed
>    by `i`, stopping when `(fsm+i)->state_no == -1`. Each array entry represents
>    one transition row (or a stateless final/start marker) for `state_no`.
> 3. For each row whose `target != -1`, call `net->initialize_transition_vector(state_no, get_number_of_transitions(fsm+i))`
>    to presize the row.
> 4. If the row's `start_state == 1`, call `handle_start_state(fsm+i, start_state_id, start_state_found)`
>    (records the foma start-state id into `start_state_id` and sets `start_state_found`).
> 5. If `target != -1`, add a transition from `state_no` to `target` with input
>    `harmonization_vector.at((fsm+i)->in)` and output `harmonization_vector.at((fsm+i)->out)`,
>    weight 0 (numbers used directly, do not insert symbols to alphabet).
> 6. If the row's `final_state == 1`, set `state_no` final with weight 0.
> 7. After the loop: if no start state was found, copy the alphabet via
>    `copy_alphabet(t, net)` and return `net` (treated as empty rather than throwing).
> 8. If `start_state_id != 0`, call `net->swap_state_numbers(start_state_id, 0)`
>    so the start state becomes state 0.
> 9. Copy the alphabet with `copy_alphabet(t, net)` and return `net`.

> [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.get-harmonization-vector-fn]
> ConversionFunctions::NumberVector

> [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.get-harmonization-vector-fn]
> `ConversionFunctions::get_harmonization_vector(const StringVector &coding_vector)`
> returns a `NumberVector` (`std::vector<unsigned int>`) of the same length as
> `coding_vector`. Reserves capacity equal to `coding_vector.size()`, then for each
> string element: if it is non-empty, pushes `get_number(element)` (its global
> symbol number, allocating one if new); if it is the empty string (a gap in the
> indexing), pushes 0. Returns the vector. No exceptions, mutates only the global
> string/number maps via `get_number`.

> [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.get-number-fn]
> unsigned int ConversionFunctions::get_number(const std::string &str)

> [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.get-number-fn]
> `ConversionFunctions::get_number(const std::string &str)` returns the unsigned
> int symbol number for `str`, interning it if new. Looks `str` up in the static
> `string_to_number_map`. If found, returns the mapped value. If not found:
> appends `str` to the static `number_to_string_vector`, computes
> `new_index = number_to_string_vector.size() - 1` (converted via `hfst::size_t_to_uint`),
> stores `string_to_number_map[str] = new_index`, and returns `new_index`.
> Mutates both static members on a miss.

> [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.get-string-fn]
> std::string ConversionFunctions::get_string(unsigned int number)

> [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.get-string-fn]
> `ConversionFunctions::get_string(unsigned int number)` returns the symbol string
> for a number. If `number >= number_to_string_vector.size()` (number not found),
> returns the empty string `""`. Otherwise returns `number_to_string_vector[number]`.
> Read-only; no mutation.

> [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.hfst-basic-transducer-to-foma-fn]
> static fsm * hfst_basic_transducer_to_foma

> [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.hfst-basic-transducer-to-foma-fn]
> Converts `const HfstBasicTransducer * hfst_fsm` to a newly built foma `fsm *`.
> Steps:
> 1. Initialize a foma construct handle `h = fsm_construct_init("")`.
> 2. Iterate states by index `source_state` starting at 0 (one per
>    `hfst_fsm` row). For each transition in the row, read the input and output
>    *symbol strings* (via `get_input_symbol`/`get_output_symbol`) and call
>    `fsm_construct_add_arc(h, source_state, target_state, input, output)`.
> 3. For each entry in `hfst_fsm->final_weight_map`, call
>    `fsm_construct_set_final(h, state)` (weights are not carried; foma is unweighted).
> 4. Copy the alphabet with `copy_alphabet(hfst_fsm, h)`.
> 5. Call `fsm_construct_set_initial(h, 0)` (state 0 is the start state), finalize
>    with `net = fsm_construct_done(h)`, then `fsm_count(net)` and
>    `net = fsm_topsort(net)`.
> 6. Return `net`.

> [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.hfst-basic-transducer-to-hfst-ol-fn]
> static hfst_ol::Transducer * hfst_basic_transducer_to_hfst_ol

> [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.hfst-basic-transducer-to-hfst-ol-fn]
> Builds an optimized-lookup `hfst_ol::Transducer *` from `const HfstBasicTransducer * t`.
> Parameters: `weighted` (whether to produce a weighted transducer), `options`
> (string; if it contains `"empty_alphabet"` the symbol table is emptied at the
> end), and an optional `harmonizer` HfstTransducer (whose raw `hfst_ol`
> implementation is used as a fixed symbol table). Algorithm:
> 1. Constants: `packing_aggression = 0.85f`, `floor_jump_threshold = 4`,
>    `TA_OFFSET = 2147483648u` (the transition-array index offset).
> 2. Call `get_states_and_symbols(t, state_placeholders, symbol_table,
>    seen_input_symbols, flag_symbols, harmonizer_ol)` to collect per-state
>    placeholders, the symbol table (epsilon/input/other ordering), the count of
>    input symbols (starting at 1 for epsilon), and the set of flag-diacritic symbol numbers.
> 3. Assign transition-index-table (TIA) starting indices: allocate an
>    `IndexPlaceholders`. For each non-simple state placeholder in order, scan from
>    `first_available_index` upward to the first index `i` where
>    `used_indices->fits(state, flag_symbols, i)`; set `start_index = i`; mark a
>    finality marker at `i` and, for each transition group, mark `i + index_offset + 1`
>    where `index_offset` is the group's input symbol number (or 0 if it is a flag
>    symbol). Then advance `first_available_index` past indices that are `unsuitable`
>    under `packing_aggression`. The `previous_first_index`/`floor_stuck_counter`
>    logic jumps `first_available_index` to `previous_successful_index + 1` if it
>    stalls more than `floor_jump_threshold` iterations.
> 4. Build the weighted index table `windex_table`: for each index `i` up to the
>    greatest used index, append a blank `TransitionWIndex()` if unused; a final
>    marker (`create_final` with the state's final weight) if the target's symbol is
>    `NO_SYMBOL_NUMBER` and the state is final (else blank); otherwise a
>    `TransitionWIndex(sym, first_transition + symbol_offset(sym, flag_symbols) + TA_OFFSET)`.
>    Delete `used_indices`. Append `seen_input_symbols` blank padding entries.
> 5. Build the transition table via
>    `write_transitions_from_state_placeholders(wtransition_table, state_placeholders, flag_symbols)`.
> 6. If `empty_alphabet`, clear `symbol_table` and set `seen_input_symbols = 0`.
> 7. Construct a `TransducerAlphabet(symbol_table)` and a `TransducerHeader`
>    (input symbol count, total symbol count, index-table size, transition-table
>    size, weighted flag), and return `new hfst_ol::Transducer(header, alphabet,
>    windex_table, wtransition_table)`.

> [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.hfst-basic-transducer-to-log-ofst-fn]
> static fst::LogFst * hfst_basic_transducer_to_log_ofst

> [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.hfst-basic-transducer-to-log-ofst-fn]
> Builds an OpenFst log-weight `LogFst *` from `const HfstBasicTransducer * net`.
> Steps:
> 1. Allocate `t = new LogFst()`, add a start state and `SetStart` it; seed a
>    `state_map` mapping HFST state 0 to that start state.
> 2. Create a `SymbolTable` seeded with `internal_epsilon`=0, `internal_unknown`=1,
>    `internal_identity`=2.
> 3. Iterate states by index `source_state` (starting at 0). For each transition,
>    add a `LogArc` whose ilabel/olabel are `st.AddSymbol(input_symbol)` /
>    `st.AddSymbol(output_symbol)` (interning each symbol into the table), with the
>    transition weight, from `hfst_state_to_state_id(source_state, state_map, t)` to
>    `hfst_state_to_state_id(target_state, state_map, t)` (each call lazily creates
>    the OpenFst state if absent). Note `source_state` is incremented implicitly by
>    iteration order via `hfst_state_to_state_id` keyed on it.
> 4. For each entry in `net->final_weight_map`, call
>    `t->SetFinal(hfst_state_to_state_id(state, state_map, t), weight)`.
> 5. Add every alphabet symbol of `net` to the symbol table (`st.AddSymbol`) so
>    symbols not occurring in transitions are preserved.
> 6. `t->SetInputSymbols(&st)` and return `t`.

> [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.hfst-basic-transducer-to-sfst-fn]
> static SFST::Transducer * hfst_basic_transducer_to_sfst

> [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.hfst-basic-transducer-to-sfst-fn]
> Builds an `SFST::Transducer *` from `const HfstBasicTransducer * net`. Steps:
> 1. Allocate a new `SFST::Transducer`, and add the HFST special symbols
>    `internal_unknown` (number 1) and `internal_identity` (number 2) to its alphabet.
> 2. Copy `net`'s alphabet: for each symbol that is not epsilon, unknown, or
>    identity, add it to the SFST alphabet with number `net->get_symbol_number(symbol)`.
> 3. Build a recoding from HFST symbol numbers to SFST symbol numbers: get
>    `SfstTransducer::get_symbol_map(t)`, erase `"<>"`, set the entry for
>    `internal_epsilon` to 0, then build `harm` via
>    `HfstTropicalTransducerTransitionData::get_reverse_harmonization_vector(symbol_map)`.
> 4. Create SFST nodes: push the root node for state 0, then `t->new_node()` for
>    each state 1..`net->get_max_state()`, into `state_vector`.
> 5. Iterate states by index `source_state` (starting at 0); for each transition,
>    build an `SFST::Label(harm.at(input_number), harm.at(output_number))` and call
>    `state_vector[source_state]->add_arc(label, state_vector[target_state], t)`.
> 6. For each entry in `net->final_weight_map`, if its state index is beyond
>    `state_vector.size()` push a new node (should not happen), then mark
>    `state_vector[state]->set_final(1)` (weights dropped; SFST is unweighted).
> 7. Return `t`.

> [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.hfst-basic-transducer-to-tropical-ofst-fn]
> static fst::StdVectorFst * hfst_basic_transducer_to_tropical_ofst

> [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.hfst-basic-transducer-to-tropical-ofst-fn]
> Builds an OpenFst tropical `fst::StdVectorFst *` from `const HfstBasicTransducer * net`.
> Steps:
> 1. Allocate `t = new StdVectorFst()`, add the start state (always state 0) and
>    `SetStart` it.
> 2. Build `state_vector` mapping HFST state numbers to OpenFst StateIds: index 0
>    is the start state, then `t->AddState()` for each state 1..`net->get_max_state()`.
> 3. Build a `SymbolTable` seeded with `internal_epsilon`=0, `internal_unknown`=1,
>    `internal_identity`=2, then add each alphabet symbol of `net` with number
>    `net->get_symbol_number(symbol)`.
> 4. Iterate states by index `source_state` (starting at 0); for each transition,
>    add `fst::StdArc(input_number, output_number, weight, state_vector[target_state])`
>    from `state_vector[source_state]` (symbol *numbers* used directly as labels).
> 5. For each entry in `net->final_weight_map`, call
>    `t->SetFinal(state_vector[state], weight)`.
> 6. `t->SetInputSymbols(&st)` and return `t`.

> [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.hfst-basic-transducer-to-xfsm-fn]
> static NETptr hfst_basic_transducer_to_xfsm(const HfstBasicTransducer * t)

> [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.hfst-basic-transducer-to-xfsm-fn]
> Builds an xfsm `NETptr` from `const HfstBasicTransducer * hfst_fsm`. Steps:
> 1. `result = null_net()` (creates the net with its initial/start state already present).
> 2. Copy states into `state_vector` (HFST state index -> `STATEptr`): iterate
>    states by index `fsm_state` from 0; for `fsm_state == 0` push
>    `result->start.state`; for others call
>    `add_state_to_net(result, is_final_state(fsm_state) ? 1 : 0)` and push the new state.
> 3. Iterate states again by index `source_state`; for each transition compute the
>    label id `ti`: if the input symbol is `internal_identity` (and the output must
>    also be identity, else throw `"identity symbol cannot be on one side only"`),
>    use the atomic `OTHER` label; otherwise map both symbols via
>    `XfsmTransducer::hfst_symbol_to_xfsm_symbol` and combine with `id_pair_to_id`.
>    Then `add_arc_to_state(result, source_state_ptr, ti, target_state_ptr, NULL, 0)`,
>    throwing `"add_arc_to_state failed"` if it returns NULL.
> 4. If state 0 is final, set `result = optional_net(result, 0)` (makes the empty
>    string accepted).
> 5. Copy alphabet: get `net_sigma(result)`, and for each symbol in
>    `hfst_fsm->get_alphabet()` that is not epsilon/unknown/identity, call
>    `alph_add_to(ap, hfst_symbol_to_xfsm_symbol(symbol), DONT_KEEP)`.
> 6. Return `result`.

> [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.hfst-ol-to-hfst-basic-transducer-fn]
> static HfstBasicTransducer * hfst_ol_to_hfst_basic_transducer

> [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.hfst-ol-to-hfst-basic-transducer-fn]
> Builds an `HfstBasicTransducer *` from an optimized-lookup `hfst_ol::Transducer * t`.
> Steps:
> 1. Allocate `basic`. Determine `weighted` from `t->get_header().probe_flag(Weighted)`.
>    Copy every symbol from `t->get_alphabet().get_symbol_table()` into `basic`'s
>    alphabet via `add_symbol_to_alphabet`.
> 2. Do a graph traversal over the OL transition index table. Maintain an `agenda`
>    (stack) of `TransitionTableIndex` and a `state_map` (OL index -> HFST state number),
>    plus a `state_number` counter. Add the initial state (index 0) with
>    `hfst_ol_to_hfst_basic_add_state(t, basic, state_map, weighted, 0, state_number)`
>    and push 0 onto the agenda.
> 3. While the agenda is non-empty: pop `current_index`, look up `current_state` in
>    `state_map`. For each transition from that index
>    (`t->get_transitions_from_state(current_index)`): if its target is not yet in
>    `state_map`, increment `state_number`, add the new state via
>    `hfst_ol_to_hfst_basic_add_state(...)`, and push the target onto the agenda.
>    Then add a transition from `current_state` to `state_map[target]` with input
>    symbol `symbols[transition.get_input_symbol()]`, output symbol
>    `symbols[transition.get_output_symbol()]`, and weight equal to the
>    `TransitionW` weight if `weighted` else 0.
> 4. Return `basic`. (Final states/weights are set inside `hfst_ol_to_hfst_basic_add_state`.)

> [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.hfst-ol-to-hfst-transducer-fn]
> static HfstTransducer * hfst_ol_to_hfst_transducer(hfst_ol::Transducer * t)

> [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.hfst-ol-to-hfst-transducer-fn]
> `hfst_ol_to_hfst_transducer(hfst_ol::Transducer * t)` wraps an OL transducer in a
> new `HfstTransducer *`. Chooses `type = HFST_OLW_TYPE` if `t->is_weighted()` else
> `HFST_OL_TYPE`. Allocates `retval = new HfstTransducer(type)`, deletes the
> default `retval->implementation.hfst_ol`, and replaces it with a copy
> `new hfst_ol::Transducer(*t)`. Returns `retval`.

> [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.hfst-state-to-state-id-fn]
> static StateId hfst_state_to_state_id

> [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.hfst-state-to-state-id-fn]
> `hfst_state_to_state_id(HfstState s, std::map<HfstState, StateId> &state_map, LogFst *t)`
> returns the OpenFst `StateId` corresponding to HFST state `s`, lazily allocating.
> Looks `s` up in `state_map`; if absent, calls `t->AddState()`, stores
> `state_map[s] = newId`, and returns `newId`; otherwise returns the existing mapped
> id. Mutates `state_map` and `t` on a miss. (Defined alongside the log-weight
> conversions; an analogous one exists for tropical.)

> [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.hfst-transducer-to-hfst-basic-transducer-fn]
> HfstBasicTransducer * ConversionFunctions

> [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.hfst-transducer-to-hfst-basic-transducer-fn]
> `hfst_transducer_to_hfst_basic_transducer(const HfstTransducer &t)` dispatches on
> `t.type` to the matching backend converter, copies the transducer name onto the
> result, and returns a new `HfstBasicTransducer *`. Order of checks (each guarded
> by the corresponding HAVE_* build flag):
> - `SFST_TYPE` -> `sfst_to_hfst_basic_transducer(t.implementation.sfst)`
> - `TROPICAL_OPENFST_TYPE` -> `tropical_ofst_to_hfst_basic_transducer(t.implementation.tropical_ofst)`
> - `LOG_OPENFST_TYPE` -> `log_ofst_to_hfst_basic_transducer(t.implementation.log_ofst)`
> - `FOMA_TYPE` -> `foma_to_hfst_basic_transducer(t.implementation.foma)`
> - `XFSM_TYPE` -> `xfsm_to_hfst_basic_transducer(t.implementation.xfsm)`
> - `HFST_OL_TYPE` or `HFST_OLW_TYPE` -> `hfst_ol_to_hfst_basic_transducer(t.implementation.hfst_ol)`
> In every matched branch, set `retval->name = t.get_name()` and return `retval`.
> If `t.type` matches none of the available types, throw `FunctionNotImplementedException`.

> [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.hfst-transducer-to-hfst-ol-fn]
> static hfst_ol::Transducer * hfst_transducer_to_hfst_ol(HfstTransducer * t)

> [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.hfst-transducer-to-hfst-ol-fn]
> `hfst_transducer_to_hfst_ol(HfstTransducer * t)` returns the underlying
> `hfst_ol::Transducer *`. If `t->type` is neither `HFST_OL_TYPE` nor
> `HFST_OLW_TYPE`, first calls `t->convert(HFST_OLW_TYPE)` (mutating `t` in place to
> a weighted OL representation). Then returns `t->implementation.hfst_ol` (a
> borrowed pointer owned by `t`, not a copy).

> [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.log-ofst-to-hfst-basic-transducer-fn]
> static HfstBasicTransducer * log_ofst_to_hfst_basic_transducer

> [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.log-ofst-to-hfst-basic-transducer-fn]
> Builds an `HfstBasicTransducer *` from a log-weight `LogFst *t`; `has_hfst_header`
> indicates whether the source came from an HFST stream (which guarantees an input
> symbol table). Steps:
> 1. Read `inputsym`/`outputsym` symbol tables. If `has_hfst_header` and
>    `inputsym == NULL`, throw `MissingOpenFstInputSymbolTableException`. Allocate `net`.
> 2. Empty transducer case (`t->Start() == kNoStateId`): if `inputsym != NULL`,
>    insert each non-epsilon input symbol into `net->alphabet`; if `!has_hfst_header`
>    and `outputsym != NULL`, also insert non-epsilon output symbols; return `net`.
> 3. Non-empty case: require `inputsym != NULL` (else throw the same exception); if
>    `outputsym == NULL` use `inputsym` for it.
> 4. State-number swap: state 0 and the initial state are swapped in the output so
>    the initial state is always printed as 0. Compute `zero_print = initial_state`
>    if the initial state is not 0 (else 0). For both origin and target states, the
>    mapping is: state 0 -> `zero_print`, initial_state -> 0, otherwise the state id.
> 5. First pass processes only the initial state (iterating states, picking
>    `s == initial_state`, then `break`); second pass processes all states
>    `s != initial_state`. In each, for every arc add a transition using the remapped
>    origin/target and symbol *strings* `inputsym->Find(ilabel)` /
>    `outputsym->Find(olabel)`, except ilabel/olabel 0 map to `internal_epsilon`, with
>    weight `arc.weight.Value()`. If `t->Final(s) != LogWeight::Zero()`, set
>    `net->set_final_weight(remapped_origin, final.Value())`.
> 6. Finally insert every non-epsilon symbol from both `inputsym` and `outputsym`
>    into `net->alphabet` so alphabet-only symbols are preserved. Return `net`.

> [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.number-vector]
> typedef std::vector<unsigned int> NumberVector

> [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.sfst-to-hfst-basic-transducer-fn]
> static void sfst_to_hfst_basic_transducer

> [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.sfst-to-hfst-basic-transducer-fn]
> Builds an `HfstBasicTransducer *` from an `SFST::Transducer * t`. (A recursive
> helper `sfst_to_hfst_basic_transducer(SFST::Node*, net, harmonization_vector)`
> does the per-node copy; this entry function sets it up.) Steps:
> 1. Allocate `net`. Build `symbol_vector = SfstTransducer::get_symbol_vector(t)`,
>    overwrite index 0 with `"@_EPSILON_SYMBOL_@"` (SFST's internal "<>"), then build
>    `harmonization_vector = get_harmonization_vector(symbol_vector)`.
> 2. Get node indexing via `t->nodeindexing(&indexing)`; presize the state vector
>    with `net->initialize_state_vector(indexing.size())`.
> 3. If the root node is already marked with `VMARK` (`check_visited`), increment
>    the global `VMARK` so the upcoming traversal sees all nodes as unvisited.
> 4. Call the recursive helper on `t->root_node()`. The helper, for each
>    not-yet-visited node: counts its arcs, presizes the node's transition row,
>    adds a transition per arc to `arc->target_node()->index` with input
>    `harmonization_vector.at(arc->label().lower_char())` and output
>    `harmonization_vector.at(arc->label().upper_char())` (weight 0), sets the node
>    final (weight 0) if `node->is_final()`, then recurses into each target node.
> 5. Copy the alphabet: for each entry in the SFST char map, insert the symbol
>    string into `net->alphabet`, skipping the epsilon entry (char number 0).
> 6. Return `net`.

> [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.string2-number-map]
> typedef std::map<std::string, unsigned int> String2NumberMap

> [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.tropical-ofst-to-hfst-basic-transducer-fn]
> static HfstBasicTransducer * tropical_ofst_to_hfst_basic_transducer

> [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.tropical-ofst-to-hfst-basic-transducer-fn]
> Builds an `HfstBasicTransducer *` from a tropical `fst::StdVectorFst *t`;
> `has_hfst_header` indicates HFST provenance. Steps:
> 1. Allocate `net`, call `handle_symbol_tables(t, net, has_hfst_header)` to set up
>    symbol tables, then build `symbol_vector = get_symbol_vector(t)` and
>    `harmonization_vector = get_harmonization_vector(symbol_vector)`.
> 2. State-number swap so the initial state becomes 0: record
>    `initial_state = t->Start()`. For each state id, the remapped state is: if it
>    equals `initial_state` -> 0; else if it is 0 -> `initial_state`; else unchanged.
> 3. Iterate all states. For each state `s` with remapped `origin`, presize its row
>    via `net->initialize_transition_vector(s, t->NumArcs(s))`, then iterate its arcs.
>    For each arc, validate `arc.ilabel` and `arc.olabel` are within
>    `symbol_vector.size()` (throwing `HfstFatalException` with a message otherwise),
>    compute the remapped `target`, and add a transition from `origin` to `target`
>    with input `harmonization_vector[arc.ilabel]`, output
>    `harmonization_vector[arc.olabel]`, weight `arc.weight.Value()` (numbers used
>    directly, symbols not inserted to alphabet here).
> 4. If `t->Final(s) != TropicalWeight::Zero()`, set
>    `net->set_final_weight(origin, t->Final(s).Value())`.
> 5. Copy the alphabet via `copy_alphabet(t, net)` and return `net`.

> [spec:hfst:def:convert-transducer-format.hfst.implementations.conversion-functions.xfsm-to-hfst-basic-transducer-fn]
> static HfstBasicTransducer * xfsm_to_hfst_basic_transducer(NETptr t)

> [spec:hfst:sem:convert-transducer-format.hfst.implementations.conversion-functions.xfsm-to-hfst-basic-transducer-fn]
> Builds an `HfstBasicTransducer *` from an xfsm `NETptr t`. Steps:
> 1. Allocate `result`. Iterate xfsm states (`t->body.states` linked list);
>    `start_ptr = t->start.state`. First loop: for every state except the start
>    state, call `result->add_state()` (the initial state already exists in `result`).
> 2. Build the `xfsm_to_hfst_state` map `STATEptr -> HfstState`. xfsm states are
>    stored in a stack order, so HFST state numbers are assigned from the largest
>    downward: start with `result_state = result->get_max_state()`. The start state
>    maps to HFST state 0 (and is set final with weight 0 if `state_ptr->final != 0`);
>    each non-start state maps to the current `result_state` (set final weight 0 if
>    `final != 0`), then decrement `result_state`.
> 3. Iterate states again; for each arc, decode its `label` into an input/output
>    symbol pair via `XfsmTransducer::label_id_to_symbol_pair`, and add a transition
>    from `xfsm_to_hfst_state[state_ptr]` to `xfsm_to_hfst_state[arc->destination]`
>    with those symbols and weight 0.
> 4. Copy the alphabet via `copy_xfsm_alphabet_into_hfst_alphabet(t, result)` and
>    return `result`.

> [spec:hfst:def:convert-transducer-format.hfst.implementations.dummy3-fn]
> StringVectorInitializer

> [spec:hfst:sem:convert-transducer-format.hfst.implementations.dummy3-fn]
> `dummy3` is a file-scope static object of type `StringVectorInitializer`,
> constructed with `ConversionFunctions::number_to_string_vector` as its argument.
> Its sole purpose is the side effect of its constructor running at static
> initialization time: it seeds `number_to_string_vector` with the three reserved
> symbols (epsilon, unknown, identity) at indices 0,1,2. The object itself is never
> referenced afterwards.

> [spec:hfst:def:convert-transducer-format.hfst.implementations.dummy4-fn]
> String2NumberMapInitializer

> [spec:hfst:sem:convert-transducer-format.hfst.implementations.dummy4-fn]
> `dummy4` is a file-scope static object of type `String2NumberMapInitializer`,
> constructed with `ConversionFunctions::string_to_number_map` as its argument. Its
> purpose is the constructor side effect at static initialization: it seeds
> `string_to_number_map` with the three reserved symbols mapped to their numbers
> (epsilon->0, unknown->1, identity->2). The object is never referenced afterwards.

> [spec:hfst:def:convert-transducer-format.hfst.implementations.state-id]
> typedef /*fst::StdArc::StateId*/ unsigned int StateId

> [spec:hfst:def:convert-transducer-format.hfst.implementations.string-vector-initializer]
> class StringVectorInitializer

> [spec:hfst:def:convert-transducer-format.hfst.implementations.string-vector-initializer.string-vector-initializer-fn]
> StringVectorInitializer

> [spec:hfst:sem:convert-transducer-format.hfst.implementations.string-vector-initializer.string-vector-initializer-fn]
> The `StringVectorInitializer(StringVector &vector)` constructor pushes the three
> reserved HFST symbol strings, in order, onto the given vector:
> `"@_EPSILON_SYMBOL_@"` (index 0), `"@_UNKNOWN_SYMBOL_@"` (index 1),
> `"@_IDENTITY_SYMBOL_@"` (index 2). No return; mutates `vector` only.

> [spec:hfst:def:convert-transducer-format.hfst.implementations.string2-number-map-initializer]
> class String2NumberMapInitializer

> [spec:hfst:def:convert-transducer-format.hfst.implementations.string2-number-map-initializer.string2-number-map-initializer-fn]
> String2NumberMapInitializer

> [spec:hfst:sem:convert-transducer-format.hfst.implementations.string2-number-map-initializer.string2-number-map-initializer-fn]
> The `String2NumberMapInitializer(String2NumberMap &map)` constructor sets three
> entries in the given map: `map["@_EPSILON_SYMBOL_@"] = 0`,
> `map["@_UNKNOWN_SYMBOL_@"] = 1`, `map["@_IDENTITY_SYMBOL_@"] = 2`. No return;
> mutates `map` only.

> [spec:hfst:def:convert-transducer-format.main-fn]
> int main(void)

> [spec:hfst:sem:convert-transducer-format.main-fn]
> The `MAIN_TEST` unit-test `main(void)`: prints a banner `"Unit tests for <file>:"`.
> For each of the three implementation types {SFST_TYPE, FOMA_TYPE,
> TROPICAL_OPENFST_TYPE}: if that type is available
> (`HfstTransducer::is_implementation_type_available`), build a tokenizer `tok` and
> a transducer `fsm1("cat", "dog", tok, type)`, set its final weights to 4, convert
> it to an `HfstBasicTransducer *` via
> `ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(fsm1)`, then build
> `fsm1_converted_twice` from that basic transducer back into the same `type` and
> delete the intermediate. If `fsm1.compare(fsm1_converted_twice)` is false (the
> round trip changed the transducer), return 1. After all types, return 0.

