# libhfst/src/implementations/optimized-lookup/find_epsilon_loops.cc

> [spec:hfst:def:find-epsilon-loops.hfst-ol.transducer.find-loop-epsilon-indices-fn]
> void Transducer::find_loop_epsilon_indices(unsigned int input_pos,

> [spec:hfst:sem:find-epsilon-loops.hfst-ol.transducer.find-loop-epsilon-indices-fn]
> Inspects index-table entry `i`. Reads `tables->get_index_input(i)`; if it
> equals 0 (the epsilon symbol), this index entry leads to epsilon transitions:
> it calls `find_loop_epsilon_transitions(input_pos, tables->get_index_target(i)
> - TRANSITION_TARGET_TABLE_START)` (converting the index target into a
> transition-table offset by subtracting `TRANSITION_TARGET_TABLE_START`), then
> sets the member `found_transition = true`. If the index input is not 0, it
> does nothing. Returns void.

> [spec:hfst:def:find-epsilon-loops.hfst-ol.transducer.find-loop-epsilon-transitions-fn]
> void Transducer::find_loop_epsilon_transitions(

> [spec:hfst:sem:find-epsilon-loops.hfst-ol.transducer.find-loop-epsilon-transitions-fn]
> Walks consecutive transition-table entries starting at index `i`, following
> epsilon and flag-diacritic transitions while detecting non-progressing loops.
> First snapshots the current flag-diacritic state into `flags =
> flag_state.get_values()`. Then loops indefinitely. Each iteration: reads
> `target = tables->get_transition_target(i)` and builds a `TraversalState
> epsilon_reachable(target, flags)`.
> - If `tables->get_transition_input(i) == 0` (epsilon): if
>   `traversal_states.count(epsilon_reachable) == 1` (this state already seen),
>   `throw true;` (a thrown bool signalling a loop was found). Otherwise insert
>   `epsilon_reachable` into the `traversal_states` set, recurse via
>   `find_loop(input_pos, target)`, then erase `epsilon_reachable` from the set,
>   set `found_transition = true`, and `++i`.
> - Else if `alphabet->is_flag_diacritic(tables->get_transition_input(i))`:
>   apply the flag operation via `flag_state.apply_operation(*(alphabet->
>   get_operation(input)))`. If it returns true (flag allowed): same loop check
>   (throw true if already in `traversal_states`), else insert
>   `epsilon_reachable`, recurse `find_loop(input_pos, target)`, erase it. In all
>   flag cases, restore the snapshot with `flag_state.assign_values(flags)` and
>   `++i`.
> - Else (neither epsilon nor flag): `return;` ending the loop.
> Returns void. Mutates `traversal_states`, `found_transition`, and transiently
> `flag_state`. May throw `true` (bool) to signal a detected loop.

> [spec:hfst:def:find-epsilon-loops.hfst-ol.transducer.find-loop-fn]
> void Transducer::find_loop(unsigned int input_pos,

> [spec:hfst:sem:find-epsilon-loops.hfst-ol.transducer.find-loop-fn]
> Core recursive traversal step at table position `i` with current `input_pos`.
> Sets `found_transition = false` at entry. Then branches on
> `indexes_transition_table(i)` (whether `i` is in the transition table region
> vs the index table region).
> - If it indexes the transition table: subtract `i -=
>   TRANSITION_TARGET_TABLE_START` so `i` is a transition-table offset. Call
>   `find_loop_epsilon_transitions(input_pos, i+1)`. If `input_tape[input_pos]
>   == NO_SYMBOL_NUMBER` (input exhausted), `return;`. Otherwise read `input =
>   input_tape[input_pos]`, then `++input_pos`. Call
>   `find_loop_transitions(input, input_pos, i+1)`. Then, if
>   `alphabet->get_default_symbol() != NO_SYMBOL_NUMBER && !found_transition`,
>   call `find_loop_transitions(alphabet->get_default_symbol(), input_pos,
>   i+1)`.
> - Else (it indexes the index table): call `find_loop_epsilon_indices(
>   input_pos, i+1)`. If `input_tape[input_pos] == NO_SYMBOL_NUMBER`, `return;`.
>   Otherwise read `input = input_tape[input_pos]`, `++input_pos`. Call
>   `find_loop_index(input, input_pos, i+1)`. Then, if a default symbol is
>   defined (`alphabet->get_default_symbol() != NO_SYMBOL_NUMBER`) and
>   `!found_transition`, call `find_loop_index(alphabet->get_default_symbol(),
>   input_pos, i+1)`.
> Returns void. Reads `input_tape`, mutates `found_transition` (and indirectly
> `traversal_states`/`flag_state` via callees). Note `input_pos` is passed by
> value, so the local `++input_pos` only affects the recursive calls below it,
> not the caller. May propagate a thrown `true` from callees.

> [spec:hfst:def:find-epsilon-loops.hfst-ol.transducer.find-loop-index-fn]
> void Transducer::find_loop_index(SymbolNumber input,

> [spec:hfst:sem:find-epsilon-loops.hfst-ol.transducer.find-loop-index-fn]
> Looks up symbol `input` in the index table at offset `i`. Reads
> `tables->get_index_input(i+input)`; if it equals `input` (this index slot
> matches the symbol), it calls `find_loop_transitions(input, input_pos,
> tables->get_index_target(i+input) - TRANSITION_TARGET_TABLE_START)`
> (converting the index target to a transition-table offset by subtracting
> `TRANSITION_TARGET_TABLE_START`), then sets `found_transition = true`. If the
> index slot does not match, does nothing. Returns void.

> [spec:hfst:def:find-epsilon-loops.hfst-ol.transducer.find-loop-transitions-fn]
> void Transducer::find_loop_transitions(SymbolNumber input,

> [spec:hfst:sem:find-epsilon-loops.hfst-ol.transducer.find-loop-transitions-fn]
> Walks consecutive transition-table entries starting at offset `i`, consuming
> matches on symbol `input`. Loops while `tables->get_transition_input(i) !=
> NO_SYMBOL_NUMBER`. In each iteration: if `tables->get_transition_input(i) ==
> input`, then since real input was consumed we cannot be in an epsilon/flag
> loop, so it clears `traversal_states.clear()`, recurses `find_loop(input_pos,
> tables->get_transition_target(i))`, and sets `found_transition = true`. If the
> transition input does not equal `input`, `return;` immediately (transitions
> are sorted, so no further match is possible). After a matching iteration,
> `++i` and continue. Returns void. Mutates `traversal_states` and
> `found_transition`. May propagate a thrown `true` from `find_loop`.

> [spec:hfst:def:find-epsilon-loops.hfst-ol.traversal-state.operator-fn]
> bool TraversalState::operator<(const TraversalState & rhs) const

> [spec:hfst:sem:find-epsilon-loops.hfst-ol.traversal-state.operator-fn]
> Strict-less-than ordering on `TraversalState`, used to make it usable as a
> key in an ordered set. First compares `this->index` to `rhs.index`: if
> `this->index < rhs.index` return true; if `this->index > rhs.index` return
> false. When indices are equal, compares the `flags` vectors element by element
> over `i` in `[0, this->flags.size())`: at the first position where
> `this->flags[i] < rhs.flags[i]` return true, or where `this->flags[i] >
> rhs.flags[i]` return false. If all compared positions are equal, return false
> (the two are equivalent, not less-than). Const, no side effects. Assumes both
> `flags` vectors have the same length.

