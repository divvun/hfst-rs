# libhfst/src/implementations/ConvertTropicalWeightTransducer.cc

> [spec:hfst:def:convert-tropical-weight-transducer.hfst.implementations.conversion-functions.hfst-basic-transducer-to-tropical-ofst-fn]
> fst::StdVectorFst *

> [spec:hfst:sem:convert-tropical-weight-transducer.hfst.implementations.conversion-functions.hfst-basic-transducer-to-tropical-ofst-fn]
> `ConversionFunctions::hfst_basic_transducer_to_tropical_ofst(const HfstBasicTransducer *net)` builds and returns a newly heap-allocated `fst::StdVectorFst *` equivalent to `net`.
> Steps:
> 1. Allocate a new `fst::StdVectorFst`. Add a state (its id is always 0), and set it as the start state.
> 2. Build `state_vector` (vector of `StateId`) mapping HfstBasicTransducer state numbers to OpenFst state ids: push the start state (id 0) for index 0, then for `i` from 1 up to and including `net->get_max_state()`, add a new OpenFst state and push its id. Thus index `i` of `state_vector` is the OpenFst id for net state `i`.
> 3. Create an empty `fst::SymbolTable st("")`. Add the three special symbols with fixed numbers: `internal_epsilon` -> 0, `internal_unknown` -> 1, `internal_identity` -> 2.
> 4. Copy the alphabet: iterate over `net->alphabet` (each entry a non-empty string, asserted non-empty); for each symbol add it to `st` with number `net->get_symbol_number(symbol)`.
> 5. Iterate over all states via `net->begin()..net->end()`, tracking `source_state` starting at 0 and incremented once per state. For each transition in the current state, read `in = tr_it->get_input_number()`, `out = tr_it->get_output_number()`, and add an OpenFst arc from `state_vector[source_state]` with `fst::StdArc(in, out, tr_it->get_weight(), state_vector[tr_it->get_target_state()])`.
> 6. Iterate over `net->final_weight_map` (map from state number to weight); for each entry call `t->SetFinal(state_vector[entry.first], entry.second)`.
> 7. Set `t`'s input symbol table to `&st` (copied in by OpenFst), and return `t`.
> Note: only the input symbol table is set; no output symbol table is set. The caller owns the returned pointer.

> [spec:hfst:def:convert-tropical-weight-transducer.hfst.implementations.conversion-functions.tropical-ofst-to-hfst-basic-transducer-fn]
> HfstBasicTransducer *

> [spec:hfst:sem:convert-tropical-weight-transducer.hfst.implementations.conversion-functions.tropical-ofst-to-hfst-basic-transducer-fn]
> `ConversionFunctions::tropical_ofst_to_hfst_basic_transducer(fst::StdVectorFst *t, bool has_hfst_header)` builds and returns a newly heap-allocated `HfstBasicTransducer *` equivalent to the OpenFst tropical-weight transducer `t`.
> Steps:
> 1. Allocate a new `HfstBasicTransducer` named `net`.
> 2. Call `handle_symbol_tables(t, net, has_hfst_header)` (validates/copies symbol-table presence and, for empty transducers, copies alphabet symbols; may throw `MissingOpenFstInputSymbolTableException`).
> 3. Build `symbol_vector` (a `StringVector`) via `TropicalWeightTransducer::get_symbol_vector(t)`: this maps OpenFst label numbers (indices) to symbol strings.
> 4. Build `harmonization_vector` (`std::vector<unsigned int>`) via `HfstTropicalTransducerTransitionData::get_harmonization_vector(symbol_vector)`: this maps each OpenFst label number to the corresponding HFST internal symbol number.
> 5. Record `initial_state = t->Start()`. State numbering is remapped so the initial state becomes number 0 and (if state 0 is not initial) the original state 0 takes the initial state's number — i.e. the numbers of the initial state and of state 0 are swapped.
> 6. Iterate over all states with a `fst::StateIterator`. For each state `s`: compute `origin = s`, then if `origin == initial_state` set `origin = 0`, else if `origin == 0` set `origin = initial_state`. Read `number_of_arcs = t->NumArcs(s)` and call `net->initialize_transition_vector(s, number_of_arcs)` (note: keyed by the original `s`, not by `origin`).
> 7. For each arc of state `s` (via `fst::ArcIterator`): compute `target = arc.nextstate` then apply the same swap (== initial_state -> 0; == 0 -> initial_state). If `arc.ilabel >= symbol_vector.size()` or `arc.olabel >= symbol_vector.size()`, throw `HfstFatalException` with a message naming the offending input/output number. Otherwise call `net->add_transition(origin, HfstBasicTransition(target, harmonization_vector[arc.ilabel], harmonization_vector[arc.olabel], arc.weight.Value(), false), false)` — the transition is added by symbol numbers, and the trailing `false` means do not insert symbols into the alphabet.
> 8. After processing arcs of `s`: if `t->Final(s) != fst::TropicalWeight::Zero()`, call `net->set_final_weight(origin, t->Final(s).Value())` to mark the (remapped) state final with that weight.
> 9. After all states, call `copy_alphabet(t, net)` to copy the input/output symbol tables' symbols into `net`'s alphabet.
> 10. Assert `net != NULL` and return `net`. The caller owns the returned pointer.

> [spec:hfst:def:convert-tropical-weight-transducer.hfst.implementations.copy-alphabet-fn]
> static void

> [spec:hfst:sem:convert-tropical-weight-transducer.hfst.implementations.copy-alphabet-fn]
> `static void copy_alphabet(fst::StdVectorFst *t, HfstBasicTransducer *net)` copies the symbols of `t`'s symbol tables into `net`'s alphabet.
> Steps:
> 1. Get `inputsym = t->InputSymbols()` and `outputsym = t->OutputSymbols()`.
> 2. If `inputsym != NULL`, iterate over its entries; assert each symbol string is non-empty; for every entry whose label number is not 0 (epsilon, label 0, is skipped), call `net->add_symbol_to_alphabet(symbol)`.
> 3. If `outputsym != NULL`, do the same iteration and insertion over the output symbol table (again skipping label 0).
> No return value; the only effect is mutating `net`'s alphabet.

> [spec:hfst:def:convert-tropical-weight-transducer.hfst.implementations.handle-symbol-tables-fn]
> static void

> [spec:hfst:sem:convert-tropical-weight-transducer.hfst.implementations.handle-symbol-tables-fn]
> `static void handle_symbol_tables(fst::StdVectorFst *t, HfstBasicTransducer *net, bool has_hfst_header)` validates the presence of `t`'s symbol tables and, for empty transducers, copies symbols into `net`'s alphabet.
> Steps:
> 1. Get `inputsym = t->InputSymbols()` and `outputsym = t->OutputSymbols()`.
> 2. If `has_hfst_header` is true and `inputsym == NULL`, throw `MissingOpenFstInputSymbolTableException` (an HFST tropical transducer always has an input symbol table).
> 3. Empty-transducer case: if `t->Start() == fst::kNoStateId` (no start state):
> 3a. If `inputsym != NULL`, iterate its entries; assert each symbol non-empty; for every entry with label != 0 (epsilon skipped) call `net->add_symbol_to_alphabet(symbol)`.
> 3b. If `!has_hfst_header` and `outputsym != NULL`, iterate the output table the same way (skip label 0) and add each symbol to `net`'s alphabet. (For an HFST transducer the output table, if present, is equivalent to the input table, so it is not separately copied here.)
> 3c. Return.
> 4. Non-empty case: if `inputsym == NULL`, throw `MissingOpenFstInputSymbolTableException` (a non-empty OpenFst transducer must have at least an input symbol table; a missing output table is assumed equivalent to the input table). Otherwise return with no further action — the actual alphabet copying for non-empty transducers happens later via `copy_alphabet`.
> No return value.

> [spec:hfst:def:convert-tropical-weight-transducer.main-fn]
> int

> [spec:hfst:sem:convert-tropical-weight-transducer.main-fn]
> `int main(int argc, char *argv[])` is the unit-test entry point compiled only when `MAIN_TEST` is defined. It prints `"Unit tests for " __FILE__ ":"` followed by a newline, then prints `"ok"` followed by a newline, and returns 0. It performs no actual test logic.

