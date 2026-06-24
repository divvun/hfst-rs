# libhfst/src/implementations/ConvertLogWeightTransducer.cc

> [spec:hfst:def:convert-log-weight-transducer.hfst.implementations.conversion-functions.hfst-basic-transducer-to-log-ofst-fn]
> LogFst *

> [spec:hfst:sem:convert-log-weight-transducer.hfst.implementations.conversion-functions.hfst-basic-transducer-to-log-ofst-fn]
> Builds a new `LogFst` from a const `HfstBasicTransducer *net` and returns it (caller owns).
> Allocate a new `LogFst t`. Add one state via `t->AddState()`, set it as the start state, and
> initialize `state_map` (an `std::map<HfstState,StateId>`) so that HFST state 0 maps to that
> start state. Create a local `fst::SymbolTable st("")` and pre-add the three special symbols:
> `internal_epsilon` -> 0, `internal_unknown` -> 1, `internal_identity` -> 2.
> Iterate over the states of `net` (via `net->begin()..net->end()`), tracking the source HFST
> state number in `source_state` starting at 0 and incremented... NOTE: in this code `source_state`
> is declared as 0 but is NEVER incremented inside the loop, so every transition is added from
> source HFST state 0 (a latent bug — replicate it exactly: always pass `source_state` == 0).
> For each transition in the current state, call `t->AddArc(hfst_state_to_state_id(source_state,
> state_map, t), fst::LogArc(ilabel, olabel, weight, target))` where ilabel = `st.AddSymbol(input
> symbol string)`, olabel = `st.AddSymbol(output symbol string)`, weight = transition's weight, and
> target = `hfst_state_to_state_id(transition target state, state_map, t)`. `AddSymbol` returns the
> existing id if the symbol is already present, otherwise assigns a new one.
> Then iterate `net->final_weight_map`; for each (HfstState, weight) pair call
> `t->SetFinal(hfst_state_to_state_id(state, state_map, t), weight)`.
> Then iterate `net->alphabet` and call `st.AddSymbol` for each symbol to register symbols that
> never appeared in transitions. Finally `t->SetInputSymbols(&st)` (a copy is stored) and return `t`.

> [spec:hfst:def:convert-log-weight-transducer.hfst.implementations.conversion-functions.hfst-state-to-state-id-fn]
> StateId

> [spec:hfst:sem:convert-log-weight-transducer.hfst.implementations.conversion-functions.hfst-state-to-state-id-fn]
> Maps an HFST state number to an OpenFst `StateId`, creating the OpenFst state lazily.
> Parameters: `HfstState s`, a mutable `std::map<HfstState,StateId> &state_map`, and `LogFst *t`.
> Look up `s` in `state_map`. If not found, allocate a new state in the transducer via
> `t->AddState()`, store the resulting `StateId` in `state_map[s]`, and return it. If found,
> return the already-mapped `StateId` (`it->second`) without adding a state.

> [spec:hfst:def:convert-log-weight-transducer.hfst.implementations.conversion-functions.log-ofst-to-hfst-basic-transducer-fn]
> HfstBasicTransducer *

> [spec:hfst:sem:convert-log-weight-transducer.hfst.implementations.conversion-functions.log-ofst-to-hfst-basic-transducer-fn]
> Builds a new `HfstBasicTransducer *` equivalent to OpenFst log transducer `t`, given a flag
> `has_hfst_header`. Returns the new net (caller owns).
> Read `inputsym = t->InputSymbols()` and `outputsym = t->OutputSymbols()`. If `has_hfst_header`
> is true and `inputsym == NULL`, throw `MissingOpenFstInputSymbolTableException`.
> Allocate `net = new HfstBasicTransducer()`.
> Empty-transducer case: if `t->Start() == fst::kNoStateId`: if `inputsym != NULL`, insert every
> input symbol whose label != 0 (skip epsilon) into `net->alphabet`. Additionally, only if
> `!has_hfst_header` and `outputsym != NULL`, insert every output symbol with label != 0 into the
> alphabet (for HFST transducers the output table is considered equivalent to the input one and is
> skipped here). Return `net`.
> Non-empty case: if `inputsym == NULL` throw `MissingOpenFstInputSymbolTableException`; if
> `outputsym == NULL` set `outputsym = inputsym`.
> State renumbering: HFST requires the initial state to be numbered 0. Compute `initial_state =
> t->Start()`. If `initial_state != 0`, set `zero_print = initial_state` (else `zero_print = 0`).
> Define a mapping for any OpenFst state id `x`: printed id = `zero_print` if `x == 0`, else `0` if
> `x == initial_state`, else `x` (this swaps the numbers of state 0 and the initial state).
> First pass: iterate all states; act only on the state equal to `initial_state`. Compute its
> printed `origin` via the mapping above. For each arc out of it (`fst::ArcIterator`), compute the
> printed `target` of `arc.nextstate` via the same mapping. Resolve `istring = inputsym->Find(arc
> .ilabel)` and `ostring = outputsym->Find(arc.olabel)`, but if `arc.ilabel == 0` set istring to
> `internal_epsilon` and if `arc.olabel == 0` set ostring to `internal_epsilon`. Call
> `net->add_transition(origin, HfstBasicTransition(target, istring, ostring, arc.weight.Value()))`.
> If `t->Final(initial_state) != LogWeight::Zero()`, call `net->set_final_weight(origin,
> t->Final(s).Value())`. Then `break` (process only the initial state in this pass).
> Second pass: iterate all states, acting only on states where `s != initial_state`, performing the
> exact same origin/target printing, symbol resolution, `add_transition`, and final-weight handling
> as the first pass.
> Finally, copy any symbols that exist only in the symbol tables but not in transitions: iterate
> `inputsym` and `outputsym`, inserting each symbol with label != 0 into `net->alphabet`. Return `net`.

> [spec:hfst:def:convert-log-weight-transducer.main-fn]
> int

> [spec:hfst:sem:convert-log-weight-transducer.main-fn]
> Test-build entry point (compiled only when `MAIN_TEST` is defined). Prints `"Unit tests for "
> __FILE__ ":"` followed by a newline to stdout, then prints `"ok"` and a newline, and returns 0.
> Performs no actual testing.

