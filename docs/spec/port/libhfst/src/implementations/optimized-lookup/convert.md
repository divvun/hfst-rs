# libhfst/src/implementations/optimized-lookup/convert.cc, libhfst/src/implementations/optimized-lookup/convert.h

> [spec:hfst:def:convert.hfst-ol.add-transitions-with-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.add-transitions-with-fn]
> Appends one weighted transition (TransitionW) to `transition_table` for every
> TransitionPlaceholder in `transitions`, all sharing the given input `symbol`.
> For each placeholder `it`, compute its target table index: look up the target
> state via `state_placeholders[it->target]`; if that state is_simple(), set
> `target = state_placeholders[it->target].first_transition + TRANSITION_TARGET_TABLE_START - 1`
> (point directly at the transition-table entry); otherwise set
> `target = state_placeholders[it->target].start_index` (point at its index entry).
> Then append `TransitionW(symbol, it->output, target, it->weight)`.
> `flag_symbols` is a parameter but is not used in the body. No return value.

> [spec:hfst:def:convert.hfst-ol.arc-iterator]
> typedef fst::ArcIterator<TransduceR> ArcIterator

> [spec:hfst:def:convert.hfst-ol.check-finality-fn]
> bool

> [spec:hfst:sem:convert.hfst-ol.check-finality-fn]
> Returns true iff state `s` of OpenFST transducer `tr` is final. Computes
> `tr->Final(s)` and returns whether it differs from `fst::TropicalWeight::Zero()`
> (the semiring zero, which marks non-final states). Compiled only when
> HAVE_OPENFST is defined.

> [spec:hfst:def:convert.hfst-ol.compare-states-by-input-size-fn]
> bool

> [spec:hfst:sem:convert.hfst-ol.compare-states-by-input-size-fn]
> Comparator over StatePlaceholder ordering by number of distinct input symbols
> descending: returns `lhs.inputs > rhs.inputs`.

> [spec:hfst:def:convert.hfst-ol.compare-states-by-state-number-fn]
> bool

> [spec:hfst:sem:convert.hfst-ol.compare-states-by-state-number-fn]
> Comparator over StatePlaceholder ordering by state number ascending: returns
> `lhs.state_number < rhs.state_number`.

> [spec:hfst:def:convert.hfst-ol.compare-transition-labels]
> struct compare_transition_labels

> [spec:hfst:def:convert.hfst-ol.compare-transition-labels.operator-fn]
> bool operator() ( const transition_label &l1,

> [spec:hfst:sem:convert.hfst-ol.compare-transition-labels.operator-fn]
> Strict-weak-ordering comparator on transition_label pairs (l1, l2), ordering
> lexicographically by (input_symbol, output_symbol). If the input symbols are
> equal, returns `l1.output_symbol < l2.output_symbol`; otherwise returns
> `l1.input_symbol < l2.input_symbol`.

> [spec:hfst:def:convert.hfst-ol.convert-fst-state]
> class ConvertFstState {
>   ConvertTransitionSet transitions;
>   ConvertTransitionIndexSet transition_indices;
>   TransitionTableIndex first_transition_index;
>   TransitionTableIndex table_index;
>   bool final;
>   Weight weight;
>   StateIdNumber id;
> }

> [spec:hfst:def:convert.hfst-ol.convert-fst-state-vector]
> typedef std::vector<ConvertFstState*> ConvertFstStateVector

> [spec:hfst:def:convert.hfst-ol.convert-fst-state.append-transitions-fn]
> TransitionTableIndex

> [spec:hfst:sem:convert.hfst-ol.convert-fst-state.append-transitions-fn]
> Template (over transition type T) method appending this state's transitions to
> `transition_table` starting at running position `place`. First pads: while
> `place < get_first_transition_index()`, append a `T(final, weight)` filler entry
> and increment `place` (this fills the gap before the state's reserved slot,
> encoding finality/weight in the filler). Then for each transition in
> `transitions` (in set order), append `(*it)->to_transition<T>()` and increment
> `place`. Returns the updated `place`. Does not mutate the state.

> [spec:hfst:def:convert.hfst-ol.convert-fst-state.convert-fst-state-fn]
> ConvertFstState::ConvertFstState(StateId n, TransduceR *tr)

> [spec:hfst:sem:convert.hfst-ol.convert-fst-state.convert-fst-state-fn]
> Constructor building a ConvertFstState for OpenFST state `n` of transducer `tr`.
> Initializes `table_index = NO_TABLE_INDEX`, `final = check_finality(tr, n)`,
> `weight = INFINITE_WEIGHT`, and `id` to the state's number via
> `ConvertTransducer::constructing_transducer->get_id_number_map().get_node_id(n)`.
> Body: calls `set_transitions(n, tr)` then `set_transition_indices()`. If the
> state is final: when the constructing transducer is_weighted(), set
> `weight = tr->Final(n).Value()`; otherwise build a TransitionTableIndex value 1
> and store its bit pattern into `weight` via reinterpret_cast (encoding the
> finality flag as a raw integer reinterpreted as a Weight). Reads the static
> `ConvertTransducer::constructing_transducer`. (The matching destructor frees
> every ConvertTransition in `transitions` and every ConvertTransitionIndex in
> `transition_indices`.)

> [spec:hfst:def:convert.hfst-ol.convert-fst-state.display-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.convert-fst-state.display-fn]
> Debug printer to std::cout. Prints `id` followed by " at index " and
> `table_index`; if final, prints " (final, " weight ")"; then ":" and newline.
> Prints " Transition indices:" and a newline, then calls `display()` on each
> ConvertTransitionIndex in `transition_indices`. Prints " Transitions:" and a
> newline, then calls `display()` on each ConvertTransition in `transitions`.
> Read-only; side effect is console output.

> [spec:hfst:def:convert.hfst-ol.convert-fst-state.get-first-transition-index-fn]
> TransitionTableIndex get_first_transition_index() const

> [spec:hfst:sem:convert.hfst-ol.convert-fst-state.get-first-transition-index-fn]
> Getter returning the member `first_transition_index`.

> [spec:hfst:def:convert.hfst-ol.convert-fst-state.get-id-fn]
> StateIdNumber get_id(void) const

> [spec:hfst:sem:convert.hfst-ol.convert-fst-state.get-id-fn]
> Getter returning the member `id` (the state's StateIdNumber).

> [spec:hfst:def:convert.hfst-ol.convert-fst-state.get-input-symbols-fn]
> SymbolNumberSet *

> [spec:hfst:sem:convert.hfst-ol.convert-fst-state.get-input-symbols-fn]
> Allocates a new SymbolNumberSet on the heap, iterates over this state's
> `transition_indices`, inserting each index's `get_input_symbol()` into the set,
> and returns the pointer. Caller owns the allocation. The set thus contains the
> distinct input symbols for which this state has index entries.

> [spec:hfst:def:convert.hfst-ol.convert-fst-state.get-table-index-fn]
> TransitionTableIndex get_table_index(void) const

> [spec:hfst:sem:convert.hfst-ol.convert-fst-state.get-table-index-fn]
> Getter returning the member `table_index`.

> [spec:hfst:def:convert.hfst-ol.convert-fst-state.insert-transition-indices-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.convert-fst-state.insert-transition-indices-fn]
> Template (over T) method writing this state's index-table entries into
> `index_table`. Early return if the state is neither big (`!is_big_state()`) nor
> the start state (`!is_start_state()`) — only those states get index entries.
> Otherwise set local `i = table_index`. If the state is final, overwrite
> `index_table[i]` with `T(index_table[i].get_input_symbol(), weight-as-index)`,
> where the weight is reinterpret_cast from the Weight member to a
> TransitionTableIndex (encoding finality at the state's base slot). Increment `i`.
> Then for each ConvertTransitionIndex `ind` in `transition_indices`, set
> `index_table[i + ind->get_input_symbol()] = ind->to_transition_index<T>()`
> (offsetting by the input symbol number). No return value.

> [spec:hfst:def:convert.hfst-ol.convert-fst-state.is-big-state-fn]
> bool is_big_state(void) const

> [spec:hfst:sem:convert.hfst-ol.convert-fst-state.is-big-state-fn]
> Returns true iff `transition_indices.size() > BIG_STATE_LIMIT`, i.e. the state
> has more distinct indexed input symbols than the threshold for "big" states.

> [spec:hfst:def:convert.hfst-ol.convert-fst-state.is-final-fn]
> bool is_final(void) const

> [spec:hfst:sem:convert.hfst-ol.convert-fst-state.is-final-fn]
> Returns the boolean member `final`.

> [spec:hfst:def:convert.hfst-ol.convert-fst-state.is-start-state-fn]
> bool is_start_state(void) const

> [spec:hfst:sem:convert.hfst-ol.convert-fst-state.is-start-state-fn]
> Returns true iff `id == 0` (the start state always has id number 0).

> [spec:hfst:def:convert.hfst-ol.convert-fst-state.number-of-input-symbols-fn]
> SymbolNumber number_of_input_symbols(void) const

> [spec:hfst:sem:convert.hfst-ol.convert-fst-state.number-of-input-symbols-fn]
> Returns `transition_indices.size()` converted to unsigned via
> `hfst::size_t_to_uint` — the count of distinct indexed input symbols.

> [spec:hfst:def:convert.hfst-ol.convert-fst-state.number-of-transitions-fn]
> SymbolNumber number_of_transitions(void) const

> [spec:hfst:sem:convert.hfst-ol.convert-fst-state.number-of-transitions-fn]
> Returns `transitions.size()` converted to unsigned via `hfst::size_t_to_uint`
> — the number of outgoing transitions of this state.

> [spec:hfst:def:convert.hfst-ol.convert-fst-state.set-table-index-fn]
> void set_table_index(TransitionTableIndex i)

> [spec:hfst:sem:convert.hfst-ol.convert-fst-state.set-table-index-fn]
> Setter assigning the argument `i` to the member `table_index`.

> [spec:hfst:def:convert.hfst-ol.convert-fst-state.set-transition-indices-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.convert-fst-state.set-transition-indices-fn]
> Builds the `transition_indices` set from the already-populated, sorted
> `transitions` set. Tracks `previous_symbol = NO_SYMBOL_NUMBER`, a running
> `position = 0`, and a flag `zero_transitions = false`. Iterates transitions in
> set order; for each transition `t` with `input_symbol = t->get_input_symbol()`:
> if `input_symbol != previous_symbol` (a new run of input symbols begins): query
> `ConvertTransducer::constructing_transducer->get_alphabet().is_flag_diacritic(input_symbol)`;
> if it is a flag diacritic, then only when `!zero_transitions` insert a new
> `ConvertTransitionIndex(0, t)` (flag diacritics index under symbol 0), set
> `previous_symbol = input_symbol`, and set `zero_transitions = true`; if it is
> not a flag diacritic, insert `ConvertTransitionIndex(input_symbol, t)` and set
> `previous_symbol = input_symbol`. Regardless of branch, if `input_symbol == 0`
> set `zero_transitions = true`. Increment `position` each iteration (value
> otherwise unused). Reads the static constructing_transducer's alphabet; mutates
> `transition_indices`. No return.

> [spec:hfst:def:convert.hfst-ol.convert-fst-state.set-transition-table-indices-fn]
> TransitionTableIndex

> [spec:hfst:sem:convert.hfst-ol.convert-fst-state.set-transition-table-indices-fn]
> Lays out this state's transitions in the transition table starting at `place`.
> Sets `first_transition_index = place`. Iterates `transitions` in set order,
> calling `t->set_table_index(place)` on each and incrementing `place` after each;
> then increments `place` once more (a one-entry gap separating states). Then for
> each ConvertTransitionIndex `i` in `transition_indices`, calls
> `i->set_first_transition_index(i->get_first_transition()->get_table_index())`,
> propagating each index's stored transition's now-assigned table location.
> Returns the updated `place` (next free position).

> [spec:hfst:def:convert.hfst-ol.convert-fst-state.set-transition-target-indices-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.convert-fst-state.set-transition-target-indices-fn]
> Iterates over this state's `transitions` and calls `set_target_state_index()`
> on each, converting every transition's target state id into its table index.
> No return value.

> [spec:hfst:def:convert.hfst-ol.convert-fst-state.set-transitions-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.convert-fst-state.set-transitions-fn]
> Populates this state's `transitions` set from OpenFST state `n` of transducer
> `tr`. Iterates the arcs leaving `n` via an ArcIterator; for each arc value `a`,
> heap-allocates a `new ConvertTransition(a)` and inserts it into `transitions`
> (which keeps them sorted by the set's comparator). No return value; transitions
> are owned by the state and freed in its destructor.

> [spec:hfst:def:convert.hfst-ol.convert-id-number-map]
> class ConvertIdNumberMap {
>   IdNumbersToStateIds id_to_node;
>   StateIdsToIdNumbers node_to_id;
>   StateIdNumber node_counter;
> }

> [spec:hfst:def:convert.hfst-ol.convert-id-number-map.add-node-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.convert-id-number-map.add-node-fn]
> Recursively assigns id numbers to states in DFS order. If state `n` is not yet
> in `node_to_id`: assign `node_to_id[n] = node_counter` and
> `id_to_node[node_counter] = n`, then increment `node_counter`. Then iterate the
> arcs leaving `n` (ArcIterator over `tr`), and for each arc `a` recursively call
> `add_node(a.nextstate, tr)`. If `n` is already mapped, do nothing (this both
> terminates recursion and prevents revisiting cycles). Mutates `node_to_id`,
> `id_to_node`, `node_counter`. No return value.

> [spec:hfst:def:convert.hfst-ol.convert-id-number-map.convert-id-number-map-fn]
> ConvertIdNumberMap(TransduceR * t)

> [spec:hfst:sem:convert.hfst-ol.convert-id-number-map.convert-id-number-map-fn]
> Constructor taking transducer `t`. Initializes `node_counter = 0` then calls
> `set_node_maps(t)`, which performs the DFS that fills `node_to_id`/`id_to_node`
> with id numbers starting from the start state as id 0.

> [spec:hfst:def:convert.hfst-ol.convert-id-number-map.get-id-node-fn]
> StateId

> [spec:hfst:sem:convert.hfst-ol.convert-id-number-map.get-id-node-fn]
> Reverse lookup: given an id number `n`, finds it in `id_to_node`. If absent,
> returns `NO_STATE_ID`; otherwise returns the mapped StateId (`i->second`).

> [spec:hfst:def:convert.hfst-ol.convert-id-number-map.get-node-id-fn]
> StateIdNumber

> [spec:hfst:sem:convert.hfst-ol.convert-id-number-map.get-node-id-fn]
> Forward lookup: given a StateId `n`, finds it in `node_to_id`. If absent,
> returns `NO_ID_NUMBER`; otherwise returns the mapped id number (`i->second`).

> [spec:hfst:def:convert.hfst-ol.convert-id-number-map.get-number-of-nodes-fn]
> StateIdNumber get_number_of_nodes(void) const

> [spec:hfst:sem:convert.hfst-ol.convert-id-number-map.get-number-of-nodes-fn]
> Returns `node_counter`, the count of nodes mapped (equal to the next id number
> that would be assigned).

> [spec:hfst:def:convert.hfst-ol.convert-id-number-map.id-numbers-to-state-ids]
> typedef std::map<StateIdNumber,StateId> IdNumbersToStateIds

> [spec:hfst:def:convert.hfst-ol.convert-id-number-map.set-node-maps-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.convert-id-number-map.set-node-maps-fn]
> Gets the transducer's start state via `t->Start()` and calls `add_node(n, t)`
> on it, triggering the recursive DFS that assigns id numbers to all reachable
> states (start state becoming id 0).

> [spec:hfst:def:convert.hfst-ol.convert-id-number-map.state-ids-to-id-numbers]
> typedef std::map<StateId,StateIdNumber> StateIdsToIdNumbers

> [spec:hfst:def:convert.hfst-ol.convert-transducer]
> class ConvertTransducer {
>   TransduceR * fst;
>   ConvertIdNumberMap * id_number_map;
>   ConvertTransitionTableIndices * fst_indices;
>   PlaceHolderVector::size_type index_table_size;
>   TransducerHeader header;
>   ConvertTransducerAlphabet alphabet;
>   ConvertFstStateVector states;
>   static ConvertTransducer* constructing_transducer;
> }

> [spec:hfst:def:convert.hfst-ol.convert-transducer-alphabet]
> class ConvertTransducerAlphabet {
>   SymbolTable symbol_table;
>   TransduceR* transducer;
>   fst::SymbolTable * ofst_symbol_table;
>   std::map<int64, SymbolNumber> input_symbols_map;
>   std::map<int64, SymbolNumber> output_symbols_map;
> }

> [spec:hfst:def:convert.hfst-ol.convert-transducer-alphabet.convert-transducer-alphabet-fn]
> ConvertTransducerAlphabet::ConvertTransducerAlphabet(TransduceR *t)

> [spec:hfst:sem:convert.hfst-ol.convert-transducer-alphabet.convert-transducer-alphabet-fn]
> Constructor taking transducer `t`; initializes member `transducer = t`. Builds
> `ofst_symbol_table` as a Copy() of `t->InputSymbols()`; if `t->OutputSymbols()`
> is non-NULL, merges them in via `ofst_symbol_table->AddTable(*t->OutputSymbols())`.
> Declares local `OfstSymbolCountMap symbol_count_map` and `SymbolSet all_symbol_set`,
> then calls `get_symbol_info(symbol_count_map, all_symbol_set)` (gathering symbol
> usage counts and the full symbol set), `populate_symbol_table(symbol_count_map, all_symbol_set)`
> (building the reordered `symbol_table`), and `set_maps()` (building the
> input/output symbol maps). Finally `delete ofst_symbol_table` to free the
> temporary copy.

> [spec:hfst:def:convert.hfst-ol.convert-transducer-alphabet.display-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.convert-transducer-alphabet.display-fn]
> Const debug printer to std::cout. Prints "Final reordered symbol table:" then,
> for each index i over `symbol_table`, prints `i << ": " << symbol_table[i]`.
> Prints "Initial input symbols (old/new: string):" then iterates the transducer's
> InputSymbols table, printing `Label() << "/" << lookup_ofst_input_symbol(Label())
> << ": " << Symbol()` per entry. Prints "Initial output symbols: (old/new: string)";
> if `transducer->OutputSymbols() != NULL`, iterates over the InputSymbols table
> (note: input table, not output) printing `Label() << "/" <<
> lookup_ofst_output_symbol(Label()) << ": " << Symbol()`; otherwise prints "[NULL]".
> Read-only; side effect is console output.

> [spec:hfst:def:convert.hfst-ol.convert-transducer-alphabet.get-symbol-info-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.convert-transducer-alphabet.get-symbol-info-fn]
> Gathers per-symbol usage counts and the full symbol set by traversal. Sets
> `symbol_count_map[0] = 1` (forcing the epsilon symbol, label 0, to count 1).
> Declares a local empty `StateIdSet visited_nodes`, then calls
> `inspect_node(transducer->Start(), visited_nodes, symbol_count_map, all_symbol_set)`
> which recursively visits all reachable states filling both output parameters.
> No return value; mutates the `symbol_count_map` and `all_symbol_set` arguments.

> [spec:hfst:def:convert.hfst-ol.convert-transducer-alphabet.inspect-node-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.convert-transducer-alphabet.inspect-node-fn]
> Recursive DFS over OpenFST state `n` collecting symbol usage. Early return if
> `n` is already in `visited_nodes`; otherwise insert `n` into `visited_nodes`.
> Declares a local `std::set<std::string> input_symbols`. Iterates the arcs leaving
> `n` via ArcIterator over `transducer`; for each arc `arc`: resolve the input
> symbol string via `transducer->InputSymbols()->Find(arc.ilabel)`; if it is NOT a
> flag diacritic (`!FdOperation::is_diacritic(...)`), insert that string into the
> local `input_symbols`. Insert the input symbol string into `all_symbol_set`
> unconditionally. For the output symbol: if `transducer->OutputSymbols() != NULL`
> insert `OutputSymbols()->Find(arc.olabel)` into `all_symbol_set`, else insert
> `InputSymbols()->Find(arc.olabel)`. Then recurse into `inspect_node(arc.nextstate,
> visited_nodes, symbol_count_map, all_symbol_set)`. After the arc loop, for each
> distinct non-diacritic input string `s` in the local `input_symbols` set,
> increment `symbol_count_map[ofst_symbol_table->Find(s)]` (so each symbol gets at
> most one count increment per state where it appears as input). Mutates
> `visited_nodes`, `symbol_count_map`, `all_symbol_set`. No return value.

> [spec:hfst:def:convert.hfst-ol.convert-transducer-alphabet.is-flag-diacritic-fn]
> bool

> [spec:hfst:sem:convert.hfst-ol.convert-transducer-alphabet.is-flag-diacritic-fn]
> Returns `FdOperation::is_diacritic(symbol_table[symbol])` — looks up the symbol
> string for the given SymbolNumber `symbol` in `symbol_table` and reports whether
> that string is a flag diacritic. Const.

> [spec:hfst:def:convert.hfst-ol.convert-transducer-alphabet.lookup-ofst-input-symbol-fn]
> SymbolNumber

> [spec:hfst:sem:convert.hfst-ol.convert-transducer-alphabet.lookup-ofst-input-symbol-fn]
> Const lookup of the new SymbolNumber for an original OpenFST input label `s`.
> Finds `s` in `input_symbols_map`; if absent returns `NO_SYMBOL_NUMBER`, otherwise
> returns the mapped value (`i->second`).

> [spec:hfst:def:convert.hfst-ol.convert-transducer-alphabet.lookup-ofst-output-symbol-fn]
> SymbolNumber

> [spec:hfst:sem:convert.hfst-ol.convert-transducer-alphabet.lookup-ofst-output-symbol-fn]
> Const lookup of the new SymbolNumber for an original OpenFST output label `s`.
> Finds `s` in `output_symbols_map`; if absent returns `NO_SYMBOL_NUMBER`,
> otherwise returns the mapped value (`i->second`).

> [spec:hfst:def:convert.hfst-ol.convert-transducer-alphabet.populate-symbol-table-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.convert-transducer-alphabet.populate-symbol-table-fn]
> Builds the reordered `symbol_table` (vector of strings) so that frequently-used
> input symbols come first. Builds a local `std::multimap<unsigned int, int64>
> count_keys` (count -> ofst label) from `input_symbol_counts`: for each entry, if
> the ofst symbol string (`ofst_symbol_table->Find(label)`) is NOT a flag diacritic
> insert the pair `(count, label)`, otherwise insert `(0, label)` (flag diacritics
> get count 0 so they sort last). Push the epsilon symbol first:
> `symbol_table.push_back(ofst_symbol_table->Find((int64)0))`. Then iterate
> `count_keys` in REVERSE order (highest count first); for each entry whose label
> `it->second != 0`, push `ofst_symbol_table->Find(it->second)` onto `symbol_table`.
> Finally iterate the whole `ofst_symbol_table`; for each entry whose Label() is
> absent from `input_symbol_counts` AND whose Symbol() string is present in
> `all_symbol_set`, push that Symbol() onto `symbol_table` (appending output-only /
> otherwise-unused but actually-occurring symbols at the end). Mutates the
> `symbol_table` member. No return value.

> [spec:hfst:def:convert.hfst-ol.convert-transducer-alphabet.set-maps-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.convert-transducer-alphabet.set-maps-fn]
> Builds `input_symbols_map` and `output_symbols_map`, each mapping an original
> OpenFST label to its new index in the reordered `symbol_table`. For each entry of
> `transducer->InputSymbols()`, scans `symbol_table` linearly for the first index i
> whose string equals the entry's Symbol(); on match sets
> `input_symbols_map[entry.Label()] = i` and breaks. If
> `transducer->OutputSymbols() != NULL`, does the same scan over OutputSymbols to
> fill `output_symbols_map`; otherwise sets `output_symbols_map = input_symbols_map`
> (a copy). Mutates the two map members. No return value.

> [spec:hfst:def:convert.hfst-ol.convert-transducer-alphabet.to-alphabet-fn]
> TransducerAlphabet

> [spec:hfst:sem:convert.hfst-ol.convert-transducer-alphabet.to-alphabet-fn]
> Const. Returns `TransducerAlphabet(symbol_table)` — a TransducerAlphabet
> constructed from this object's reordered `symbol_table` (the SymbolTable vector
> of strings). Returned by value.

> [spec:hfst:def:convert.hfst-ol.convert-transducer-header]
> class ConvertTransducerHeader

> [spec:hfst:def:convert.hfst-ol.convert-transducer-header.compute-header-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.convert-transducer-header.compute-header-fn]
> Static. Fills the output `header` (TransducerHeader) by initializing its fields
> then traversing the transducer `t`. Initial values: `number_of_input_symbols = 0`;
> `number_of_symbols = symbol_count`; `size_of_transition_index_table =
> number_of_index_table_entries`; `size_of_transition_target_table =
> number_of_target_table_entries`; `number_of_states = 0`; `number_of_transitions =
> 0`; `weighted = weighted`; `deterministic = true`; `input_deterministic = true`;
> `minimized = false`; `cyclic = false`; `has_epsilon_epsilon_transitions = false`;
> `has_input_epsilon_transitions = false`; `has_input_epsilon_cycles = false`;
> `has_unweighted_input_epsilon_cycles = false`. Declares local `StateIdSet nodes,
> nodes_in_path` and `OfstSymbolSet input_symbols`, inserts 0 (epsilon) into
> `input_symbols`, then calls `full_traversal(header, t, t->Start(), nodes,
> nodes_in_path, input_symbols)` which mutates many header flags and fills `nodes`
> and `input_symbols`. After traversal sets `header.number_of_input_symbols =
> input_symbols.size()` and `header.number_of_states = nodes.size()`. If
> `!header.weighted`, sets `header.has_unweighted_input_epsilon_cycles =
> header.has_input_epsilon_cycles`. No return value.

> [spec:hfst:def:convert.hfst-ol.convert-transducer-header.find-input-epsilon-cycles-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.convert-transducer-header.find-input-epsilon-cycles-fn]
> Static recursive search for input-epsilon cycles back to `start`, beginning at
> state `n`. Iterates the arcs leaving `n` (ArcIterator over `tr`); for each arc
> `a`: skip (continue) if `a.ilabel != 0` OR the input symbol string is a flag
> diacritic (`FdOperation::is_diacritic(tr->InputSymbols()->Find(a.ilabel))`); also,
> for a true input-epsilon arc, skip if `a.weight != StdArc::Weight::Zero()` (i.e.
> the arc carries weight). For a remaining (zero-weight input-epsilon) arc with
> target `target`: if `target == start`, set `h.has_input_epsilon_cycles = true`
> and, when `unweighted_only`, also set `h.has_unweighted_input_epsilon_cycles =
> true`, then return. If `target` is already present in `epsilon_targets`, insert it
> again and recurse `find_input_epsilon_cycles(target, start, epsilon_targets,
> unweighted_only, tr, h)`. After processing each arc, if either
> `h.has_input_epsilon_cycles` or `h.has_unweighted_input_epsilon_cycles` is set,
> return early. Mutates `h` and `epsilon_targets`. No return value. (Note: due to
> the `find(target) != end` guard before recursing, only targets already in the set
> are explored further — matching the source as written.)

> [spec:hfst:def:convert.hfst-ol.convert-transducer-header.full-traversal-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.convert-transducer-header.full-traversal-fn]
> Static recursive DFS over state `n` of `tr` computing header properties into `h`.
> Early return if `n` is in `visited_nodes`; else insert `n` into both
> `visited_nodes` and `nodes_in_path`. If `h.weighted` and not already
> `h.has_unweighted_input_epsilon_cycles`, call `find_input_epsilon_cycles(n, n,
> {}, true, tr, h)` with a fresh local StateIdSet. If not already
> `h.has_input_epsilon_cycles`, call `find_input_epsilon_cycles(n, n, {}, false,
> tr, h)`. Declares locals `OfstSymbolSet node_input_symbols` and `LabelSet
> transition_labels`. Iterates arcs leaving `n`; per arc `a`, build a
> transition_label `l` with `input_symbol=a.ilabel`, `output_symbol=a.olabel`, take
> `target=a.nextstate`, and: increment `h.number_of_transitions`; if the input
> symbol string is not a diacritic, insert `a.ilabel` into `all_input_symbols`. If
> `l.input_symbol == 0`: set `h.has_input_epsilon_transitions = true`, and if also
> `l.output_symbol == 0` set `h.has_epsilon_epsilon_transitions = true`. If
> `l.input_symbol` is already in `node_input_symbols`, set `h.input_deterministic =
> false`, else insert it. If the full label `l` is already in `transition_labels`,
> set `h.deterministic = false`, else insert it. If `target` is in `nodes_in_path`
> (a back edge), set `h.cyclic = true`. Recurse `full_traversal(h, tr, target,
> visited_nodes, nodes_in_path, all_input_symbols)`. After the arc loop, erase `n`
> from `nodes_in_path` (backtrack). Mutates `h`, `visited_nodes`, `nodes_in_path`,
> `all_input_symbols`. No return value.

> [spec:hfst:def:convert.hfst-ol.convert-transducer.add-input-symbols-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.convert-transducer.add-input-symbols-fn]
> Recursive DFS over state `n` of `fst` collecting all distinct input labels.
> Iterates the arcs leaving `n` (ArcIterator over `fst`); per arc `a`: insert
> `a.ilabel` into `input_symbols`; if `a.nextstate` is not yet in `visited_nodes`,
> insert it and recurse `add_input_symbols(a.nextstate, input_symbols,
> visited_nodes)`. Mutates `input_symbols` and `visited_nodes`. No return value.

> [spec:hfst:def:convert.hfst-ol.convert-transducer.convert-transducer-fn]
> ConvertTransducer::ConvertTransducer(TransduceR *tr, bool weighted)

> [spec:hfst:sem:convert.hfst-ol.convert-transducer.convert-transducer-fn]
> Constructor converting an OpenFST transducer `tr` into the optimized-lookup
> representation. Member initializer list: `fst = tr`; `id_number_map = new
> ConvertIdNumberMap(tr)`; `fst_indices = new
> ConvertTransitionTableIndices(number_of_input_symbols())`; `header(weighted)`;
> `alphabet(tr)`. Body: set the static `constructing_transducer = this` (so
> sub-objects can reach this instance). Reassign `id_number_map = new
> ConvertIdNumberMap(tr)` (leaks the initializer-list-allocated map). Call
> `read_nodes()` (builds the `states` vector), `set_transition_table_indices()`
> (lays out the transition table), then `set_index_table_indices()` (lays out the
> index table). Set `index_table_size = fst_indices->size()` then `delete
> fst_indices`. Call `ConvertTransducerHeader::compute_header(header, tr,
> alphabet.get_symbol_table().size(), index_table_size, count_transitions(),
> weighted)` to populate header properties. Finally `delete id_number_map`, set
> `id_number_map = NULL`, and reset `constructing_transducer = NULL`. (The several
> display() calls are commented out.) Allocates and frees heap objects; reads/writes
> the static `constructing_transducer`.

> [spec:hfst:def:convert.hfst-ol.convert-transducer.count-transitions-fn]
> TransitionTableIndex

> [spec:hfst:sem:convert.hfst-ol.convert-transducer.count-transitions-fn]
> Const. Returns the total number of transition-table entries. Iterates `states`;
> for each state adds 1 (the per-state separator entry) plus
> `state->number_of_transitions()` to a running `transition_count`, then returns it.

> [spec:hfst:def:convert.hfst-ol.convert-transducer.display-states-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.convert-transducer.display-states-fn]
> Const debug printer. Prints "Transducer states:" to std::cout then calls
> `display()` on each ConvertFstState in `states` in order. Side effect is console
> output.

> [spec:hfst:def:convert.hfst-ol.convert-transducer.display-tables-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.convert-transducer.display-tables-fn]
> Const debug printer to std::cout. Prints "Transducer tables:" and a "----------"
> separator. If `is_weighted()`: print " Transition index table:" then build
> `make_index_table<TransitionWIndex>(index_table_size)` and call `.display(false)`
> on it; print " Transition table:" then build `make_transition_table<TransitionW>()`
> and call `.display(true)`. Otherwise (unweighted): same but with `TransitionIndex`
> for the index table and `Transition` for the transition table. Print a closing
> "----------". Side effect is console output (and temporary table construction).

> [spec:hfst:def:convert.hfst-ol.convert-transducer.is-weighted-fn]
> bool is_weighted() const

> [spec:hfst:sem:convert.hfst-ol.convert-transducer.is-weighted-fn]
> Const. Returns `header.probe_flag(Weighted)` — whether the Weighted flag is set
> in the transducer header.

> [spec:hfst:def:convert.hfst-ol.convert-transducer.make-index-table-fn]
> TransducerTable<T>

> [spec:hfst:sem:convert.hfst-ol.convert-transducer.make-index-table-fn]
> Const template (over entry type T). Allocates a `TransducerTable<T> index_table`
> of `index_table_size` entries each initialized to default `T()`. Iterates all
> `states` and calls `state->insert_transition_indices(index_table)` on each (which
> writes that state's index entries into the table). Returns the populated
> `index_table` by value. Does not mutate this object.

> [spec:hfst:def:convert.hfst-ol.convert-transducer.make-transition-table-fn]
> TransducerTable<T>

> [spec:hfst:sem:convert.hfst-ol.convert-transducer.make-transition-table-fn]
> Const template (over entry type T). Builds the transition target table. Starts an
> empty `TransducerTable<T> transition_table` and a running position `place =
> TRANSITION_TARGET_TABLE_START`. Iterates all `states`, calling `place =
> (*it)->append_transitions(transition_table, place)` to append each state's padding
> and transition entries and advance `place`. After all states, appends one final
> sentinel entry `T(false, INFINITE_WEIGHT)`. Returns the table by value. Does not
> mutate this object.

> [spec:hfst:def:convert.hfst-ol.convert-transducer.number-of-input-symbols-fn]
> SymbolNumber

> [spec:hfst:sem:convert.hfst-ol.convert-transducer.number-of-input-symbols-fn]
> Returns the count of distinct input labels reachable from the start. Declares a
> local `SymbolNumberSet input_symbol_set`, inserts 0 (epsilon), declares a local
> `StateIdSet visited_nodes`, calls `add_input_symbols(fst->Start(),
> input_symbol_set, visited_nodes)` to collect every input label by DFS, then
> returns `input_symbol_set.size()`.

> [spec:hfst:def:convert.hfst-ol.convert-transducer.read-nodes-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.convert-transducer.read-nodes-fn]
> Builds the `states` vector. For each id number `id` from 0 up to (exclusive)
> `id_number_map->get_number_of_nodes()`, resolve the OpenFST StateId via
> `id_number_map->get_id_node(id)`, heap-allocate `new ConvertFstState(n, fst)`,
> and `push_back` it onto `states` (so `states[id]` corresponds to id number `id`).
> Mutates `states`; states are owned by this transducer and freed in its
> destructor. No return value.

> [spec:hfst:def:convert.hfst-ol.convert-transducer.set-index-table-indices-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.convert-transducer.set-index-table-indices-fn]
> Assigns each state its index-table location (`table_index`). Builds a local
> `StateSet state_set` (ordered by fst_state_compare) from all states EXCEPT the
> first (`states.begin() + 1` onward). Takes the start state `*states.begin()`,
> gets its index slot via `fst_indices->add_state(start_state)`, and calls
> `start_state->set_table_index(...)` with it. Then iterates `state_set` in REVERSE
> order; for each state: if `state->is_big_state()`, allocate its index slot via
> `fst_indices->add_state(state)`; otherwise compute `state_index =
> state->get_first_transition_index() - 1`, and if that is less than
> `TRANSITION_TARGET_TABLE_START`, print "FIXME!" to std::cerr and `throw;`
> (rethrow with no active exception → terminate). Call
> `state->set_table_index(state_index)`. Finally, iterate all `states` and call
> `set_transition_target_indices()` on each, so transitions learn their targets'
> now-assigned table locations. Mutates state objects, transitions, and
> `fst_indices`. No return value.

> [spec:hfst:def:convert.hfst-ol.convert-transducer.set-transition-table-indices-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.convert-transducer.set-transition-table-indices-fn]
> Lays out all states' transitions in the transition target table. Starts a running
> position `place = TRANSITION_TARGET_TABLE_START`. Iterates `states` in order,
> calling `place = state->set_transition_table_indices(place)` on each (each call
> assigns that state's transitions their table positions and returns the next free
> position). Mutates state and transition objects. No return value.

> [spec:hfst:def:convert.hfst-ol.convert-transducer.to-transducer-fn]
> Transducer *

> [spec:hfst:sem:convert.hfst-ol.convert-transducer.to-transducer-fn]
> Const. Heap-allocates and returns a new optimized-lookup `Transducer*`. If
> `is_weighted()`, constructs `new Transducer(header, alphabet.to_alphabet(),
> make_index_table<TransitionWIndex>(index_table_size),
> make_transition_table<TransitionW>())`. Otherwise constructs the unweighted
> variant with `TransitionIndex` and `Transition` table entry types. Caller owns
> the returned pointer.

> [spec:hfst:def:convert.hfst-ol.convert-transition]
> class ConvertTransition {
>   SymbolNumber input_symbol;
>   SymbolNumber output_symbol;
>   union { StateIdNumber target_state_id; TransitionTableIndex target_state_index; };
>   Weight weight;
>   TransitionTableIndex table_index;
> }

> [spec:hfst:def:convert.hfst-ol.convert-transition-compare]
> struct ConvertTransitionCompare

> [spec:hfst:def:convert.hfst-ol.convert-transition-compare.operator-fn]
> bool operator() (const ConvertTransition * t1,

> [spec:hfst:sem:convert.hfst-ol.convert-transition-compare.operator-fn]
> Const functor comparing two ConvertTransition pointers `t1`, `t2`. Returns
> `t1->operator<(*t2)` — delegates to ConvertTransition's own `<` ordering. Used as
> the comparator for ConvertTransitionSet.

> [spec:hfst:def:convert.hfst-ol.convert-transition-index]
> class ConvertTransitionIndex {
>   SymbolNumber input_symbol;
>   union { ConvertTransition* first_transition; TransitionTableIndex first_transition_index; };
> }

> [spec:hfst:def:convert.hfst-ol.convert-transition-index-compare]
> struct ConvertTransitionIndexCompare

> [spec:hfst:def:convert.hfst-ol.convert-transition-index-compare.operator-fn]
> bool operator() (const ConvertTransitionIndex * i1,

> [spec:hfst:sem:convert.hfst-ol.convert-transition-index-compare.operator-fn]
> Const functor comparing two ConvertTransitionIndex pointers `i1`, `i2`. Returns
> `i1->operator<(*i2)` — delegates to ConvertTransitionIndex's own `<` ordering (by
> input_symbol). Used as the comparator for ConvertTransitionIndexSet.

> [spec:hfst:def:convert.hfst-ol.convert-transition-index-set]
> typedef std::set<ConvertTransitionIndex*,ConvertTransitionIndexCompare>

> [spec:hfst:def:convert.hfst-ol.convert-transition-index.convert-transition-index-fn]
> ConvertTransitionIndex(SymbolNumber input, ConvertTransition* transition)

> [spec:hfst:sem:convert.hfst-ol.convert-transition-index.convert-transition-index-fn]
> Constructor. Initializes member `input_symbol = input` and the union member
> `first_transition = transition` (storing the pointer to the first ConvertTransition
> for this index entry). No body.

> [spec:hfst:def:convert.hfst-ol.convert-transition-index.display-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.convert-transition-index.display-fn]
> Const debug printer to std::cout. Prints "  input_symbol: " followed by
> `input_symbol`, then " to transitions starting at " followed by
> `first_transition_index`, then a newline. Side effect is console output. (Reads
> the `first_transition_index` union member, valid after that index has been set.)

> [spec:hfst:def:convert.hfst-ol.convert-transition-index.get-first-transition-fn]
> ConvertTransition* get_first_transition() const

> [spec:hfst:sem:convert.hfst-ol.convert-transition-index.get-first-transition-fn]
> Const getter returning the union member `first_transition` (the ConvertTransition*
> this index points at). Valid before the union is repurposed to hold
> `first_transition_index`.

> [spec:hfst:def:convert.hfst-ol.convert-transition-index.get-input-symbol-fn]
> SymbolNumber get_input_symbol(void) const

> [spec:hfst:sem:convert.hfst-ol.convert-transition-index.get-input-symbol-fn]
> Const getter returning the member `input_symbol` (the SymbolNumber this index
> entry is keyed on).

> [spec:hfst:def:convert.hfst-ol.convert-transition-index.operator-fn]
> bool

> [spec:hfst:sem:convert.hfst-ol.convert-transition-index.operator-fn]
> Const `<` ordering on ConvertTransitionIndex. Returns `input_symbol <
> another_index.input_symbol` — orders index entries solely by their input symbol
> number.

> [spec:hfst:def:convert.hfst-ol.convert-transition-index.set-first-transition-index-fn]
> void set_first_transition_index(TransitionTableIndex i)

> [spec:hfst:sem:convert.hfst-ol.convert-transition-index.set-first-transition-index-fn]
> Setter assigning argument `i` to the union member `first_transition_index`
> (repurposing the union previously holding `first_transition` to now hold the
> resolved table position).

> [spec:hfst:def:convert.hfst-ol.convert-transition-index.to-transition-index-fn]
> T

> [spec:hfst:sem:convert.hfst-ol.convert-transition-index.to-transition-index-fn]
> Const template (over entry type T). Constructs and returns `T(input_symbol,
> first_transition_index)` — a transition-index-table entry built from this
> object's input symbol and its resolved first-transition table position.

> [spec:hfst:def:convert.hfst-ol.convert-transition-set]
> typedef std::set<ConvertTransition*,ConvertTransitionCompare>

> [spec:hfst:def:convert.hfst-ol.convert-transition-table-indices]
> class ConvertTransitionTableIndices {
>   PlaceHolderVector indices;
>   PlaceHolderVector::size_type lower_bound;
>   unsigned int lower_bound_test_count;
>   SymbolNumber number_of_input_symbols;
> }

> [spec:hfst:def:convert.hfst-ol.convert-transition-table-indices.add-state-fn]
> PlaceHolderVector::size_type

> [spec:hfst:sem:convert.hfst-ol.convert-transition-table-indices.add-state-fn]
> Finds and reserves a free index-table position for `state`, returning that
> position. First, if `lower_bound_test_count >= 1`: reset `lower_bound_test_count
> = 0`; if `indices.size() > 2000` and `lower_bound < indices.size() - 2000`, jump
> `lower_bound = indices.size() - 1000` (skip far ahead to avoid re-scanning a long
> full prefix); then `++lower_bound`. Read `final_state = state->is_final()` and
> `state_input_symbols = state->get_input_symbols()` (heap-allocated set, owned
> here). Increment `lower_bound_test_count`. Then scan `index` from `lower_bound`
> up to `indices.size()`: if `index + number_of_input_symbols + 1 >=
> indices.size()`, call `get_more_space()` to grow the vector. If
> `state_fits(state_input_symbols, final_state, index)`, call
> `insert_state(...)` to mark the slots occupied, `delete state_input_symbols`, and
> return `index`. If the loop completes without fitting, return `UINT_MAX`. Note:
> on the success path the set is freed; on the (unreachable in practice) fall-through
> it is leaked. Mutates `lower_bound`, `lower_bound_test_count`, `indices`.

> [spec:hfst:def:convert.hfst-ol.convert-transition-table-indices.convert-transition-table-indices-fn]
> ConvertTransitionTableIndices(SymbolNumber input_symbol_count)

> [spec:hfst:sem:convert.hfst-ol.convert-transition-table-indices.convert-transition-table-indices-fn]
> Constructor taking `input_symbol_count`. Initializer list sets `lower_bound = 0`,
> `lower_bound_test_count = 0`, `number_of_input_symbols = input_symbol_count`. Body
> calls `get_more_space()` once, which appends `number_of_input_symbols + 1` EMPTY
> placeholder entries to `indices` (the initial index-table capacity).

> [spec:hfst:def:convert.hfst-ol.convert-transition-table-indices.get-more-space-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.convert-transition-table-indices.get-more-space-fn]
> Appends `number_of_input_symbols + 1` EMPTY place_holder entries to the `indices`
> vector (a fixed-size chunk = one finality slot plus one slot per input symbol),
> enlarging the index table. Mutates `indices`. No return value.

> [spec:hfst:def:convert.hfst-ol.convert-transition-table-indices.insert-state-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.convert-transition-table-indices.insert-state-fn]
> Marks the index-table slots occupied for a state placed at `index`. For the base
> (finality) slot at `index`: if `final_state` OR `indices.at(index) == OCCUPIED`,
> set it to OCCUPIED_START; otherwise set it to EMPTY_START. Then with
> `input_symbol_start = index + 1`, for each `input_symbol` in `input_symbols`: the
> target slot is `indices.at(input_symbol_start + input_symbol)` — if it is EMPTY,
> set it to OCCUPIED; otherwise (already occupied by another state) set it to
> OCCUPIED_START. Mutates `indices`. No return value.

> [spec:hfst:def:convert.hfst-ol.convert-transition-table-indices.last-full-index-fn]
> PlaceHolderVector::size_type

> [spec:hfst:sem:convert.hfst-ol.convert-transition-table-indices.last-full-index-fn]
> Const. Returns the highest index `i` whose `indices.at(i) != EMPTY`. Scans `i`
> downward from `indices.size() - 1` to (but not including) 0; returns the first
> such non-EMPTY index found. If none are found (loop reaches 0), returns 0. Note
> the slot at index 0 itself is never tested.

> [spec:hfst:def:convert.hfst-ol.convert-transition-table-indices.size-fn]
> PlaceHolderVector::size_type size(void) const

> [spec:hfst:sem:convert.hfst-ol.convert-transition-table-indices.size-fn]
> Const getter returning `indices.size()` — the current number of index-table
> entries.

> [spec:hfst:def:convert.hfst-ol.convert-transition-table-indices.state-fits-fn]
> bool

> [spec:hfst:sem:convert.hfst-ol.convert-transition-table-indices.state-fits-fn]
> Tests whether a state with the given `input_symbols`/`final_state` can be placed
> at base position `index` without colliding. Returns false if `indices.at(index)`
> is EMPTY_START or OCCUPIED_START (another state already starts there). Returns
> false if `final_state` and `indices.at(index) == OCCUPIED` (a final state needs
> its base slot for the finality marker, which can't share with an occupied
> transition slot). Otherwise, with `input_symbol_start = index + 1`, for each
> `input_symbol` in `input_symbols`: if `indices.at(input_symbol_start +
> input_symbol)` is OCCUPIED or OCCUPIED_START, return false. If no conflict found,
> return true. Read-only.

> [spec:hfst:def:convert.hfst-ol.convert-transition.convert-transition-fn]
> ConvertTransition::ConvertTransition(const StdArc &a)

> [spec:hfst:sem:convert.hfst-ol.convert-transition.convert-transition-fn]
> Constructor from an OpenFST `StdArc a` (empty body; all work in the initializer
> list). Sets `input_symbol =
> constructing_transducer->get_alphabet().lookup_ofst_input_symbol(a.ilabel)` (the
> new SymbolNumber for the arc's input label), `output_symbol =
> ...lookup_ofst_output_symbol(a.olabel)`, the union member `target_state_id =
> constructing_transducer->get_id_number_map().get_node_id(a.nextstate)` (the arc's
> destination state's id number), `weight = a.weight.Value()`, and `table_index =
> NO_TABLE_INDEX`. Reads the static `ConvertTransducer::constructing_transducer`.

> [spec:hfst:def:convert.hfst-ol.convert-transition.display-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.convert-transition.display-fn]
> Const debug printer to std::cout. Prints "  " then `input_symbol << ":" <<
> output_symbol`, then " at " << `table_index`, then " ->" << `target_state_index`
> (the union's resolved index member), then " (" << `weight` << ")" and a newline.
> Side effect is console output.

> [spec:hfst:def:convert.hfst-ol.convert-transition.get-input-symbol-fn]
> SymbolNumber get_input_symbol(void) const

> [spec:hfst:sem:convert.hfst-ol.convert-transition.get-input-symbol-fn]
> Const getter returning the member `input_symbol` (this transition's input
> SymbolNumber).

> [spec:hfst:def:convert.hfst-ol.convert-transition.get-table-index-fn]
> TransitionTableIndex get_table_index(void) const

> [spec:hfst:sem:convert.hfst-ol.convert-transition.get-table-index-fn]
> Const getter returning the member `table_index` (this transition's assigned
> position in the transition target table).

> [spec:hfst:def:convert.hfst-ol.convert-transition.numerical-cmp-fn]
> bool

> [spec:hfst:sem:convert.hfst-ol.convert-transition.numerical-cmp-fn]
> Const strict-weak-ordering tie-break comparing this transition to
> `another_transition` purely by numeric fields, lexicographically by
> (input_symbol, output_symbol, target_state_id). If input symbols are equal: if
> output symbols are equal, return `target_state_id <
> another_transition.target_state_id`; else return `output_symbol <
> another.output_symbol`. Otherwise return `input_symbol < another.input_symbol`.

> [spec:hfst:def:convert.hfst-ol.convert-transition.operator-fn]
> bool

> [spec:hfst:sem:convert.hfst-ol.convert-transition.operator-fn]
> Const `<` ordering on ConvertTransition, sorting epsilon/flag-diacritic
> transitions before ordinary ones. Define a symbol as "special" if its
> `input_symbol == 0` or
> `constructing_transducer->get_alphabet().is_flag_diacritic(input_symbol)` is true.
> If THIS transition is special: if `another_transition` is also special, return
> `numerical_cmp(another_transition)`; otherwise return true (specials sort before
> non-specials). If THIS transition is NOT special: if `another_transition` is also
> not special, return `numerical_cmp(another_transition)`; otherwise return false.
> Reads the static `constructing_transducer`'s alphabet.

> [spec:hfst:def:convert.hfst-ol.convert-transition.set-table-index-fn]
> void set_table_index(TransitionTableIndex i)

> [spec:hfst:sem:convert.hfst-ol.convert-transition.set-table-index-fn]
> Setter assigning argument `i` to the member `table_index`.

> [spec:hfst:def:convert.hfst-ol.convert-transition.set-target-state-index-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.convert-transition.set-target-state-index-fn]
> Resolves this transition's target into a table index. Looks up the target state
> via `constructing_transducer->get_state(target_state_id)` and sets the union
> member `target_state_index = state.get_table_index()` (overwriting the previously
> stored `target_state_id`). Reads the static `constructing_transducer`. No return.

> [spec:hfst:def:convert.hfst-ol.convert-transition.to-transition-fn]
> T

> [spec:hfst:sem:convert.hfst-ol.convert-transition.to-transition-fn]
> Const template (over entry type T). Constructs and returns `T(input_symbol,
> output_symbol, target_state_index, weight)` — a transition-table entry of type T
> built from this transition's fields (using the resolved `target_state_index`).

> [spec:hfst:def:convert.hfst-ol.fst-state-compare]
> struct fst_state_compare

> [spec:hfst:def:convert.hfst-ol.fst-state-compare.operator-fn]
> bool

> [spec:hfst:sem:convert.hfst-ol.fst-state-compare.operator-fn]
> Const functor comparing two ConvertFstState pointers `s1`, `s2`. If
> `s1->transition_indices.size() < s2->transition_indices.size()`, return true.
> Otherwise return `s1->id < s2->id`. (Friend access to the private members.) Note
> this is not a strict-weak-ordering when sizes differ asymmetrically, but matches
> the source: states are ordered primarily by fewer transition indices, then by id.

> [spec:hfst:def:convert.hfst-ol.hfst-ol-to-basic-state-map]
> typedef std::map<hfst_ol::TransitionTableIndex,unsigned int>

> [spec:hfst:def:convert.hfst-ol.index-placeholders]
> struct IndexPlaceholders {
>   std::vector<unsigned int> indices;
>   std::vector<std::pair<unsigned int, SymbolNumber> > targets;
> }

> [spec:hfst:def:convert.hfst-ol.index-placeholders.assign-fn]
> void assign(unsigned int const position, unsigned int target, SymbolNumber sym)

> [spec:hfst:sem:convert.hfst-ol.index-placeholders.assign-fn]
> Records that index slot `position` points at a transition target. First grows the
> `indices` vector with NO_TABLE_INDEX entries while `position >= indices.size()`
> (so `position` becomes addressable). Sets `indices[position] = targets.size()`
> (an index into the `targets` vector) and pushes the pair `(target, sym)` onto
> `targets`. Mutates both `indices` and `targets`. No return value.

> [spec:hfst:def:convert.hfst-ol.index-placeholders.fits-fn]
> bool fits(StatePlaceholder const & state,

> [spec:hfst:sem:convert.hfst-ol.index-placeholders.fits-fn]
> Const. Tests whether `state` can be placed with its base index at `position`.
> Returns false immediately if `used(position)` (the base slot is taken). Then
> iterates `state.transition_placeholders` (each inner vector is a group sharing one
> input symbol); for each group, take `index_offset = it->at(0).input` (the input
> symbol of the group's first transition); if `flag_symbols.count(index_offset) !=
> 0`, set `index_offset = 0` (flags index to 0). If `used(index_offset + position +
> 1)` return false (the symbol's target slot is occupied). If no conflict, return
> true. Read-only.

> [spec:hfst:def:convert.hfst-ol.index-placeholders.get-target-fn]
> std::pair<unsigned int, SymbolNumber> get_target(unsigned int index)

> [spec:hfst:sem:convert.hfst-ol.index-placeholders.get-target-fn]
> Returns the `(target, sym)` pair recorded for index slot `index`: looks up
> `indices[index]` (a position into `targets`) and returns `targets[indices[index]]`.
> Assumes the slot was previously assigned (no bounds/validity check).

> [spec:hfst:def:convert.hfst-ol.index-placeholders.unsuitable-fn]
> bool unsuitable(unsigned int const index,

> [spec:hfst:sem:convert.hfst-ol.index-placeholders.unsuitable-fn]
> Const. Heuristic rejecting a base position `index` for a state with `symbols`
> input symbols, governed by `packing_aggression` (a float fill fraction). Returns
> true if `used(index)` (base slot taken). Otherwise counts how many of the symbol
> slots are already used: loop `i` from 0 to `symbols`, add `used(index + i + 1)`
> (0/1) to `filled`; as soon as `filled >= packing_aggression * symbols`, return
> true (slot region "too full"). If the loop completes below that threshold, return
> false. Read-only.

> [spec:hfst:def:convert.hfst-ol.index-placeholders.used-fn]
> bool used(unsigned int const position) const

> [spec:hfst:sem:convert.hfst-ol.index-placeholders.used-fn]
> Const. Returns true iff index slot `position` is occupied: `position <
> indices.size() && indices[position] != NO_TABLE_INDEX`. Out-of-range positions
> count as unused.

> [spec:hfst:def:convert.hfst-ol.label-set]
> typedef std::set<transition_label,compare_transition_labels> LabelSet

> [spec:hfst:def:convert.hfst-ol.ofst-symbol-count-map]
> typedef std::map<int64,unsigned int> OfstSymbolCountMap

> [spec:hfst:def:convert.hfst-ol.ofst-symbol-set]
> typedef std::set<int64> OfstSymbolSet

> [spec:hfst:def:convert.hfst-ol.place-holder]
> enum place_holder {
>   EMPTY;
>   EMPTY_START;
>   OCCUPIED_START;
>   OCCUPIED;
> }

> [spec:hfst:def:convert.hfst-ol.place-holder-vector]
> typedef std::vector<place_holder> PlaceHolderVector

> [spec:hfst:def:convert.hfst-ol.state-id]
> typedef /*fst::StdArc::StateId*/ int StateId

> [spec:hfst:def:convert.hfst-ol.state-id-set]
> typedef std::set<StateId> StateIdSet

> [spec:hfst:def:convert.hfst-ol.state-placeholder]
> struct StatePlaceholder {
>   enum indexing_type {empty, simple_zero_index, simple_nonzero_index, nonsimple};
>   unsigned int state_number;
>   unsigned int start_index;
>   unsigned int first_transition;
>   std::vector<unsigned int> symbol_to_transition_placeholder_v;
>   std::vector<std::vector<TransitionPlaceholder> > transition_placeholders;
>   indexing_type type;
>   SymbolNumber inputs;
>   bool final;
>   float final_weight;
> }

> [spec:hfst:def:convert.hfst-ol.state-placeholder.add-input-fn]
> void add_input(SymbolNumber input, std::set<SymbolNumber> const & flag_symbols)

> [spec:hfst:sem:convert.hfst-ol.state-placeholder.add-input-fn]
> Registers that this state has an outgoing transition on input symbol `input`,
> creating its transition group and updating the indexing `type`. Early return if
> `input_present(input)` (already registered). Grows
> `symbol_to_transition_placeholder_v` with UINT_MAX entries while its size <=
> `input`, then sets `symbol_to_transition_placeholder_v[input] =
> transition_placeholders.size()` and pushes a new empty
> `std::vector<TransitionPlaceholder>` onto `transition_placeholders` (the group for
> this symbol). Increments `inputs`. Then, unless `type == nonsimple`, adjusts
> `type`: treat `input` as zero-indexing if `input == 0` or `flag_symbols.count(input)
> == 1`. If `type == empty`: becomes `simple_zero_index` if zero-indexing else
> `simple_nonzero_index`. If `type == simple_zero_index`: becomes `nonsimple` if
> `input` is NOT zero-indexing (input != 0 and not a flag). If `type ==
> simple_nonzero_index`: becomes `nonsimple` if `inputs > 1` OR `input == 0` OR
> `flag_symbols.count(input) == 1`. Mutates the vectors, `inputs`, and `type`.

> [spec:hfst:def:convert.hfst-ol.state-placeholder.add-transition-fn]
> void add_transition(TransitionPlaceholder & trans)

> [spec:hfst:sem:convert.hfst-ol.state-placeholder.add-transition-fn]
> Appends transition `trans` to this state's transition group for its input symbol:
> `transition_placeholders[symbol_to_transition_placeholder_v[trans.input]].push_back(trans)`.
> Assumes `add_input(trans.input, ...)` was already called so the group exists.
> Mutates `transition_placeholders`. No return value.

> [spec:hfst:def:convert.hfst-ol.state-placeholder.get-largest-index-fn]
> SymbolNumber get_largest_index(void)

> [spec:hfst:sem:convert.hfst-ol.state-placeholder.get-largest-index-fn]
> Returns the input symbol of the transition group registered for the highest-
> numbered input symbol: `transition_placeholders[symbol_to_transition_placeholder_v.back()][0].input`.
> Takes the last entry of `symbol_to_transition_placeholder_v` (the group index for
> the largest input symbol so far), indexes into `transition_placeholders`, and
> returns the `.input` of that group's first TransitionPlaceholder. Assumes the
> vectors are non-empty.

> [spec:hfst:def:convert.hfst-ol.state-placeholder.indexing-type]
> enum indexing_type {
>   empty;
>   simple_zero_index;
>   simple_nonzero_index;
>   nonsimple;
> }

> [spec:hfst:def:convert.hfst-ol.state-placeholder.input-present-fn]
> bool input_present(SymbolNumber input) const

> [spec:hfst:sem:convert.hfst-ol.state-placeholder.input-present-fn]
> Const. Returns true iff this state has any transition placeholder bucket for the
> given `input` symbol: returns `input < symbol_to_transition_placeholder_v.size()
> && symbol_to_transition_placeholder_v[input] != UINT_MAX`. (UINT_MAX in the
> symbol-to-bucket vector means "no bucket allocated for this symbol".)

> [spec:hfst:def:convert.hfst-ol.state-placeholder.is-simple-fn]
> bool is_simple(void) const

> [spec:hfst:sem:convert.hfst-ol.state-placeholder.is-simple-fn]
> Const. Returns `type != nonsimple` — true iff this placeholder's indexing_type is
> one of empty, simple_zero_index, or simple_nonzero_index (i.e. not the nonsimple
> case requiring a full index entry).

> [spec:hfst:def:convert.hfst-ol.state-placeholder.number-of-transitions-fn]
> unsigned int number_of_transitions(void) const

> [spec:hfst:sem:convert.hfst-ol.state-placeholder.number-of-transitions-fn]
> Const. Returns the total transition count across all per-symbol buckets. Starts
> `count = 0`, iterates every inner vector in `transition_placeholders`, adding each
> bucket's `size()` (converted to unsigned via `hfst::size_t_to_uint`) to `count`,
> then returns `count`.

> [spec:hfst:def:convert.hfst-ol.state-placeholder.state-placeholder-fn]
> StatePlaceholder (unsigned int state, bool finality, unsigned int first,

> [spec:hfst:sem:convert.hfst-ol.state-placeholder.state-placeholder-fn]
> Four-argument constructor (state, finality, first, final_weight). All work is in
> the initializer list: `state_number = state`; `start_index = UINT_MAX` (not yet
> placed); `first_transition = first`; `type = (state == 0 ? nonsimple : empty)`
> (the start state, number 0, is always nonsimple, all others start empty);
> `inputs = 0`; `final = finality`; `final_weight = final_weight`. The vectors
> `symbol_to_transition_placeholder_v` and `transition_placeholders` default to
> empty. Empty body. (A separate default constructor sets state_number,
> start_index, first_transition all to UINT_MAX, type=empty, inputs=0, final=false,
> final_weight=0.0.)

> [spec:hfst:def:convert.hfst-ol.state-placeholder.symbol-offset-fn]
> unsigned int symbol_offset(

> [spec:hfst:sem:convert.hfst-ol.state-placeholder.symbol-offset-fn]
> Computes how many transition entries precede those of `symbol` within this
> state's transition block (the offset of `symbol`'s transitions from the state's
> first transition), given the set `flag_symbols`. If `symbol == 0`, returns 0
> immediately (epsilon is always first). Initialize `offset = 0`. If
> `input_present(0)` (the state has epsilon transitions), add the size of the
> epsilon bucket (`get_transition_placeholders(0).size()`) to `offset`. Then iterate
> `flag_symbols` in set order: for each flag `*flag_it` that is present
> (`input_present`), if `symbol == *flag_it` return 0 immediately (flags index to 0
> even when there's no epsilon); otherwise add that flag's bucket size to `offset`.
> Then iterate `i` from 1 up to `symbol_to_transition_placeholder_v.size()`: skip if
> `i` not present, skip (continue) if `i` is in `flag_symbols` (already counted); if
> `symbol == i` return `offset`; otherwise add bucket `i`'s size to `offset`. If the
> loop completes without matching `symbol` (symbol not present in this state),
> throw HfstFatalException with the message about failing to calculate symbol_offset
> for a symbol not present in the state.

> [spec:hfst:def:convert.hfst-ol.state-set]
> typedef std::set<ConvertFstState *, fst_state_compare> StateSet

> [spec:hfst:def:convert.hfst-ol.std-arc]
> typedef fst::StdArc StdArc

> [spec:hfst:def:convert.hfst-ol.symbol-set]
> typedef std::set<std::string> SymbolSet

> [spec:hfst:def:convert.hfst-ol.transduce-r]
> typedef fst::StdVectorFst TransduceR

> [spec:hfst:def:convert.hfst-ol.transition-label]
> struct transition_label {
>   int64 input_symbol;
>   int64 output_symbol;
> }

> [spec:hfst:def:convert.hfst-ol.transition-placeholder]
> struct TransitionPlaceholder {
>   unsigned int target;
>   SymbolNumber input;
>   SymbolNumber output;
>   float weight;
> }

> [spec:hfst:def:convert.hfst-ol.transition-placeholder.transition-placeholder-fn]
> TransitionPlaceholder(unsigned int t, SymbolNumber i, SymbolNumber o, float w)

> [spec:hfst:sem:convert.hfst-ol.transition-placeholder.transition-placeholder-fn]
> Constructor (t, i, o, w). Initializer list only, empty body: `target = t` (target
> state number), `input = i`, `output = o`, `weight = w`.

> [spec:hfst:def:convert.hfst-ol.write-transitions-from-state-placeholders-fn]
> void

> [spec:hfst:sem:convert.hfst-ol.write-transitions-from-state-placeholders-fn]
> Writes the full weighted transition target table by appending entries to
> `transition_table`, iterating over `state_placeholders` in order. For each state
> `it`: if `it->state_number != 0`, first append a finality-marker entry
> `TransitionW(it->final, it->final_weight)` (the start state's finality is instead
> encoded in the index table, so it is skipped). Then emit that state's transitions
> in a fixed symbol order so epsilon and flags come first: (1) if
> `it->input_present(0)`, call `add_transitions_with(0,
> it->get_transition_placeholders(0), transition_table, state_placeholders,
> flag_symbols)`; (2) for each flag symbol `*flag_it` in `flag_symbols` (set order)
> that is present, call `add_transitions_with(*flag_it, ...)`; (3) for each `i` from
> 1 up to `it->symbol_to_transition_placeholder_v.size()`, skip if not present or if
> `i` is a flag symbol (already emitted), else call `add_transitions_with(i, ...)`.
> After all states, append one final padding entry `TransitionW(false,
> INFINITE_WEIGHT)`. Mutates `transition_table`. No return value.

> [spec:hfst:def:convert.main-fn]
> int

> [spec:hfst:sem:convert.main-fn]
> Unit-test entry point compiled only under MAIN_TEST. Prints "Unit tests for
> <__FILE__>:" then "ok" to std::cout, and returns 0. No actual test logic.

