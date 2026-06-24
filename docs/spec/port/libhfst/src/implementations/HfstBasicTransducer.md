# libhfst/src/implementations/HfstBasicTransducer.cc, libhfst/src/implementations/HfstBasicTransducer.h

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-states]
> typedef std::vector<hfst::implementations::HfstBasicTransitions>

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer]
> class HfstBasicTransducer {
>   HfstBasicStates state_vector;
>   static const HfstState INITIAL_STATE = 0;
>   FinalWeightMap final_weight_map;
>   HfstAlphabet alphabet;
>   std::string name;
>   const HfstBasicStates &states_and_transitions() const;
>   HfstBasicStates &states_and_transitions();
>   HFSTDLL HfstBasicTransducer &operator=(const HfstBasicTransducer &graph);
>   HFSTDLL HfstBasicTransducer &assign(const HfstBasicTransducer &graph);
>   HFSTDLL HfstAlphabet;
>   HFSTDLL const HfstAlphabet &get_alphabet() const;
>   HFSTDLL StringPairSet;
>   HFSTDLL HfstTropicalTransducerTransitionData::WeightType get_final_weight(HfstState s) const;
>   HFSTDLL HfstBasicTransducer &sort_arcs(void);
>   HFSTDLL const_iterator;
>   HFSTDLL const_iterator;
>   HFSTDLL const HfstBasicTransitions &operator[](HfstState s) const;
>   HFSTDLL const HfstBasicTransitions &transitions(HfstState s) const;
>   HFSTDLL HfstBasicTransitions &transitions(HfstState s);
>   HFSTDLL static std::string prologize_symbol(const std::string &symbol);
>   HFSTDLL static std::string deprologize_symbol(const std::string &symbol);
>   HFSTDLL static std::vector<unsigned int> get_positions_of_unescaped_char(const std::string &str, char c, char esc);
>   HFSTDLL static std::string strip_newlines(std::string &str);
>   HFSTDLL static std::string get_stripped_line(std::istream &is, FILE *file, unsigned int &linecount);
>   HFSTDLL static HfstBasicTransducer;
>   HFSTDLL static HfstBasicTransducer;
>   HFSTDLL static HfstBasicTransducer;
>   HFSTDLL static HfstBasicTransducer;
>   HFSTDLL static HfstBasicTransducer;
>   HFSTDLL static HfstBasicTransducer;
>   HFSTDLL HfstBasicTransducer &substitute(const HfstSymbol &old_symbol, const HfstSymbol &new_symbol, bool input_side = true, bool output_side = true);
>   HFSTDLL HfstBasicTransducer & substitute_symbols(const HfstSymbolSubstitutions &substitutions);
>   HfstBasicTransducer & substitute(const HfstSymbolSubstitutions &substitutions);
>   HFSTDLL HfstBasicTransducer & substitute_symbol_pairs(const HfstSymbolPairSubstitutions &substitutions);
>   HFSTDLL HfstBasicTransducer & substitute(const HfstSymbolPairSubstitutions &substitutions);
>   HFSTDLL HfstBasicTransducer &substitute(const HfstSymbolPair &sp, const HfstSymbolPairSet &sps);
>   HFSTDLL HfstBasicTransducer &substitute(const HfstSymbolPair &old_pair, const HfstSymbolPair &new_pair);
>   HFSTDLL HfstBasicTransducer & substitute(bool (*func)(const HfstSymbolPair &sp, HfstSymbolPairSet &sps));
>   struct substitution_data { HfstState origin_state; HfstState target_state; HfstTropicalTransducerTransitionData::WeightType weight; HfstBasicTransducer *subs...;
>   HFSTDLL HfstBasicTransducer &substitute(const HfstSymbolPair &sp, const HfstBasicTransducer &graph);
>   HFSTDLL std::string weight2marker(float weight);
>   HFSTDLL HfstBasicTransducer &substitute_weights_with_markers();
>   HFSTDLL HfstBasicTransducer &substitute(SubstMap &substitution_map, bool harmonize);
>   HFSTDLL HfstBasicTransducer &substitute_markers_with_weights();
>   HFSTDLL HfstBasicTransducer & substitute_symbol(const std::string &old_symbol, const std::string &new_symbol, bool input_side = true, bool output_side = true);
>   HFSTDLL HfstBasicTransducer & substitute_symbol_pair(const StringPair &old_symbol_pair, const StringPair &new_symbol_pair);
>   HFSTDLL HfstBasicTransducer &substitute_symbol_pair_with_set( const StringPair &old_symbol_pair, const hfst::StringPairSet &new_symbol_pair_set);
>   HFSTDLL HfstBasicTransducer & substitute_symbol_pair_with_transducer(const StringPair &symbol_pair, HfstBasicTransducer &transducer);
>   HFSTDLL HfstBasicTransducer & insert_freely(const HfstSymbolPair &symbol_pair, HfstTropicalTransducerTransitionData::WeightType weight);
>   HFSTDLL HfstBasicTransducer & insert_freely(const HfstSymbolPairSet &symbol_pairs, HfstTropicalTransducerTransitionData::WeightType weight);
>   HFSTDLL HfstBasicTransducer & insert_freely(const HfstBasicTransducer &graph);
>   HFSTDLL HfstBasicTransducer &harmonize(HfstBasicTransducer &another);
>   HFSTDLL HfstBasicTransducer & disjunct(const StringPairVector &spv, HfstTropicalTransducerTransitionData::WeightType weight);
>   HFSTDLL HfstBasicTransducer &complete();
>   struct TopologicalSort { std::vector<int> distance_of_state; std::vector<std::set<HfstState> > states_at_distance; /* Initialize the TopologicalSort by reser...;
>   enum SortDistance { MaximumDistance, MinimumDistance };
>   HFSTDLL std::vector<std::set<HfstState> > topsort(SortDistance dist) const;
>   HFSTDLL std::vector<unsigned int> path_sizes();
>   HFSTDLL HfstReplacementsMap;
>   HFSTDLL static HfstBasicTransducer;
>   HFSTDLL static HfstBasicTransducer;
> }

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-att-line-fn]
> bool

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-att-line-fn]
> Parse one AT&T-format text line (a null-terminated `char *line`) and mutate this transducer accordingly. Parameters: `line`, `epsilon_symbol` (the textual representation of epsilon in this input), and `warn_negs`. Allocate five 100-char buffers and run `sscanf(line, "%s%s%s%s%s", a1..a5)`, capturing the count `n` of whitespace-separated fields parsed. Compute a `float weight = 0`: if `n == 2` set `weight = double_to_float(atof(a2))`; if `n == 5` set `weight = double_to_float(atof(a5))`. If `weight < 0` and `warn_negs`, print `"Negative weight %f found :-(\n"` to stderr. Then: if `n == 1` or `n == 2` (a final-state line), call `set_final_weight(atoi(a1), weight)`. Else if `n == 4` or `n == 5` (a transition line), take `input_symbol = a3` and `output_symbol = a4`; in each, replace all occurrences of `@_SPACE_@`->" ", `@0@`->`@_EPSILON_SYMBOL_@`, `@_TAB_@`->tab, `@_COLON_@`->":"; then if the symbol string equals `epsilon_symbol`, set it to `@_EPSILON_SYMBOL_@`; construct `HfstBasicTransition(atoi(a2), input_symbol, output_symbol, weight)` and call `add_transition(atoi(a1), tr)`. Otherwise (any other field count) return false (line not parseable). Return true on success.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-state-fn]
> HfstState

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-state-fn]
> Ensure state number `s` exists in this graph and return `s`. While the size of `state_vector` is `<= s`, push back a fresh empty `HfstBasicTransitions` vector. This creates `s` and every lower-numbered state that did not yet exist; an already-existing state is left untouched. Returns `s`.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-substitution-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-substitution-fn]
> Splice a copy of a substituting graph into this graph using epsilon transitions, as described by `sub` (a `substitution_data` holding `origin_state`, `target_state`, `weight`, and `substituting_graph`). Steps: (1) Call `add_state()` to allocate a brand-new state `s`; this `s` also serves as the `offset` added to all of the substituting graph's state numbers. (2) Add an epsilon:epsilon transition from `sub.origin_state` to `s` with weight `sub.weight`. (3) Copy the substituting graph (`sub.substituting_graph`): iterate its states with index `source_state` starting at 0, and for each transition add to state `source_state + offset` a transition to `tr.target_state + offset` with the same input symbol, output symbol, and weight. (4) For each final state `it` (state number `it.first`, final weight `it.second`) in the substituting graph's `final_weight_map`, add an epsilon:epsilon transition from `it.first + offset` to `sub.target_state` with weight `it.second`. Mutates this graph; returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-symbol-to-alphabet-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-symbol-to-alphabet-fn]
> Insert `symbol` into the graph's `alphabet` set. No-op if already present (it is a set insert). Returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-symbols-to-alphabet-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-symbols-to-alphabet-fn]
> The annotated overload takes an `HfstSymbolPairSet`. For each symbol pair in the set, insert both `pair.first` and `pair.second` into the graph's `alphabet` set. (There is also a sibling overload taking an `HfstSymbolSet` that inserts each plain symbol; the annotated one is the pair-set version.) Returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-to-results-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-to-results-fn]
> Conditionally record a completed lookup path into the result set. Parameters: `results` (an `HfstTwoLevelPaths` set, mutated), `path_so_far` (an `HfstTwoLevelPath` = pair of cumulative weight `.first` and a vector of symbol pairs `.second`, mutated then restored), `final_weight` (the final weight of the reached final state), and `max_weight` (a `float*`, possibly NULL). Add `final_weight` to `path_so_far.first`. Then: if `max_weight == NULL`, insert `path_so_far` into `results`; else if `path_so_far.first` is not greater than `*max_weight`, insert it; otherwise (weight limit exceeded) do nothing. Finally subtract `final_weight` back off `path_so_far.first` to restore it. Returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-transition-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.add-transition-fn]
> Add `transition` as an outgoing transition of state `s`. Parameters: `s`, `transition` (an `HfstBasicTransition`), and `add_symbols_to_alphabet` (default true). Read the transition's data. Call `add_state(s)` and `add_state(transition.get_target_state())` to ensure both endpoint states exist. If `add_symbols_to_alphabet` is true, insert the transition's input symbol and output symbol into the graph's `alphabet`. Push the transition onto `state_vector[s]`. Returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.begin-fn]
> HfstBasicTransducer::iterator

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.begin-fn]
> Return a mutable iterator to the beginning of the graph's states, i.e. `state_vector.begin()`. (There is also a const overload returning a const_iterator the same way.)

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.check-alphabet-fn]
> bool

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.check-alphabet-fn]
> Verify that every symbol used in any transition is present in the graph's `alphabet`. Iterate over all states and, for each, over all transitions; for each transition's data, if either its input symbol or its output symbol is not found in `alphabet`, return false immediately. If the full traversal finds no missing symbol, return true.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.check-regexp-state-for-cycle-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.check-regexp-state-for-cycle-fn]
> Guard against cycles during compile-replace regexp path traversal. If state `s` is already present in the `states_visited` set, throw the C-string `"error: loop detected inside compile-replace regular expression"`. Otherwise do nothing. Does not modify state.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.check-regexp-transition-end-fn]
> bool

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.check-regexp-transition-end-fn]
> Validate a transition `tr` encountered while collecting a compile-replace regexp path and report whether it is the closing bracket. `input_side` selects which side of the transition to inspect. Let `istr`/`ostr` be the input/output symbols. Choose the relevant side string by `input_side`. If that side is epsilon, it is fine (do nothing). Otherwise, if that side is a special symbol (`is_special_symbol`), throw `"error: special symbol detected in compile-replace regular expression"`. Then: if the relevant side equals `"^["`, throw `"error: ^[ detected inside compile-replace regular expression"`. If the relevant side equals `"^]"`, return true (this is the closing bracket). Otherwise return false. Weights and flag diacritics are not handled.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.const-iterator]
> typedef HfstBasicStates::const_iterator const_iterator

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.deprologize-symbol-fn]
> std::string

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.deprologize-symbol-fn]
> Convert a symbol as it appears in prolog text back to the internal HFST symbol representation (inverse of prologize). If `symbol == "%0"` return `"0"`; if `symbol == "%?"` return `"?"`; if `symbol == "0"` return `"@_EPSILON_SYMBOL_@"`; if `symbol == "?"` return `"@_UNKNOWN_SYMBOL_@"`. Otherwise copy `symbol` and unescape it: replace all `\"` (backslash-doublequote) with `"`, then replace all `\\` (double backslash) with `\`. Return the result. (Caveat noted in source: bare `?` is always treated as unknown.)

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.disjunct-fn]
> HfstState

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.disjunct-fn]
> Follow or create the path of symbol pairs `spv` starting from cursor `it` (a const_iterator into `spv`, passed by reference and advanced) and starting state `s`, returning the final state reached. Set `current_state = s`. While `it != spv.end()`: copy the transitions of `current_state` (`state_vector[current_state]`); scan them for one whose input symbol equals `it->first` and whose output symbol equals `it->second` (weights ignored); if found, set `next_state` to that transition's target and mark transition_found. If not found, call `add_state()` to make a new `next_state` and `add_transition(current_state, HfstBasicTransition(next_state, it->first, it->second, 0))`. Then advance `it` and set `current_state = next_state`. When the cursor reaches the end, return `current_state` (the final state of this path).

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.end-fn]
> HfstBasicTransducer::iterator

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.end-fn]
> Return a mutable iterator one past the last state of the graph, i.e. `state_vector.end()`. (There is also a const overload returning a const_iterator the same way.)

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.extract-weight-fn]
> bool

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.extract-weight-fn]
> Try to split a trailing `, <float>` weight off a quoted-symbol prolog string `symbol`, mutating `symbol` and writing the parsed weight into `weight`. Find `last_double_quote = symbol.find_last_of('"')` and `last_space = symbol.find_last_of(' ')`. If no double quote is found at all (`npos`), return false. Then decide by cases: if there is no space (`npos`), there is no weight (leave `symbol` unchanged, succeed). If `last_double_quote > last_space` (the last space is inside a symbol, not a separator), there is no weight (succeed). Else if `last_double_quote + 2 == last_space` (accounting for the comma between the closing quote and the space) AND `last_space < symbol.size() - 1`, parse the substring after `last_space` via an `istringstream >> weight`; if the float parse fails, return false; on success `symbol.resize(last_space - 1)` to drop the comma, space, and weight. In any other case return false (malformed symbol/weight). Return true otherwise.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.final-weight-map]
> typedef std::map<HfstState,

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-matches-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-matches-fn]
> Recursive worker for `intersect`. Parameters: `graph1`/`state1`, `graph2`/`state2`, the output `intersection`/`state`, `state_map` (state-pair -> intersection state), and `agenda` (set of intersection states already handled). Precondition: both graphs are arc-sorted and deterministic. Insert `state` into `agenda` so it is not revisited. Get `tr1 = graph1.state_vector[state1]` and `tr2 = graph2.state_vector[state2]`. If either is empty, return (no matches possible). Maintain `start_search_from = 0` into `tr2`. For each `transition1` in `tr1` (in order), scan `tr2` from index `start_search_from`: for each `transition2`, compare the two transitions' data ignoring weight (`less_than_ignore_weight`). If `transition2 < transition1`, keep scanning. If `transition1 < transition2`, no match exists for this `transition1`; set `start_search_from = j` and break to the next `transition1`. Otherwise they are equal (a match): call `handle_match(graph1, transition1, graph2, transition2, intersection, state, state_map)` to obtain the intersection target state; if that target is not already in `agenda`, recurse with `find_matches(graph1, transition1.target, graph2, transition2.target, intersection, target, state_map, agenda)`; set `start_search_from = j + 1` and break. After all `tr1` are processed, return. Mutates `intersection`, `state_map`, and `agenda`.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-matches-for-merge-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-matches-for-merge-fn]
> Recursive worker for `merge`. Parameters: `graph`/`graph_state`, `merger`/`merger_state`, output `result`/`result_state`, `state_map`, `agenda`, `list_symbols` (map from a list-symbol name to its set of member symbols), and `markers_added` (set, mutated). Preconditions: both graphs arc-sorted and deterministic. Insert `result_state` into `agenda`. Get `graph_transitions = graph.state_vector[graph_state]` and `merger_transitions = merger.state_vector[merger_state]`; if `graph_transitions` is empty, return. For each `graph_transition` in `graph_transitions`: if its data is a list symbol (`is_list_symbol`, which also throws if input != output symbol), look up its member set `symbol_list`; iterate `merger_transitions`, for each requiring input == output (else throw `"find_matches_for_merge: input and output symbols must be the same"`); if the merger transition's symbol is in `symbol_list`, mark `list_match_found`, call `handle_list_match(graph, graph_transition, merger, merger_transition, result, result_state, state_map, markers_added)` to get a target, and if that target is not in `agenda` recurse with the graph and merger transition targets. If a list match was found, `continue` to the next `graph_transition`. Otherwise (not a list symbol, or no list match found), call `handle_non_list_match(graph, graph_transition, merger, merger_state, result, result_state, state_map)` — note the merger side stays at the same `merger_state` — to get a target, and if not in `agenda`, recurse with the graph transition's target but again the unchanged `merger_state`. After all transitions, return. Mutates `result`, `state_map`, `agenda`, `markers_added`.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-regexp-paths-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-regexp-paths-fn]
> Recursive DFS that, having entered a `^[`...region, collects all sub-paths of the form `[x:y]* "^]"` starting at state `s`. Parameters: `s`, `states_visited` (set, mutated), `path` (current vector of `(input,output)` string pairs, mutated then restored), `full_paths` (`HfstReplacements` output), `input_side`. First call `check_regexp_state_for_cycle(s, states_visited)` (throws on cycle), then insert `s` into `states_visited`. Iterate the transitions of `s`. For each `transition`: call `check_regexp_transition_end(transition, input_side)` (which throws on invalid/special/`^[` symbols). If it returns true (a closing `^]`): call `check_regexp_state_for_cycle(transition.target, states_visited)` to ensure the closing bracket does not lead to a visited state; push the transition's `(input,output)` pair onto `path`; append `HfstReplacement(transition.target, path)` to `full_paths`; pop the pair back off `path` (we do not descend further). Otherwise (an interior `x:y`): push the transition's `(input,output)` pair onto `path`, recurse into `find_regexp_paths(transition.target, states_visited, path, full_paths, input_side)`, then pop the pair. After all transitions, erase `s` from `states_visited` (backtrack). Returns nothing. (A separate non-annotated overload taking only `s`, `full_paths`, `input_side` seeds this by finding `^[`:`^[` transitions out of `s` and launching this recursion.)

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-replacements-fn]
> HfstReplacementsMap

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-replacements-fn]
> Find all `"^[" [x:y]* "^]"` subpaths in the whole graph and return them as an `HfstReplacementsMap`. Iterate over every state with index `state` starting at 0; for each, create an empty `HfstReplacements full_paths`, call the seeding overload `find_regexp_paths(state, full_paths, input_side)`, and if `full_paths` is non-empty, store `replacements[state] = full_paths`. The resulting map maps a start state to a vector of `(end_state, vector_of_(isymbol,osymbol))` entries (the entries omit the closing `^]` transition). Weights are ignored. `input_side` selects which side of transitions is matched. Returns the map.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-target-state-fn]
> HfstState

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.find-target-state-fn]
> Map a pair of source-graph states to a single state in the product graph, creating it on demand. Parameters: `target1`, `target2`, `state_map` (map from `StatePair` to product state, mutated), `intersection` (the product graph, mutated), and `was_new_state` (out-param bool). Build `state_pair = (target1, target2)`. If `state_map` already contains it, set `was_new_state = false` and return the mapped state. Otherwise call `intersection.add_state()` to allocate a new state, store `state_map[state_pair] = newstate`, set `was_new_state = true`, and return the new state.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.flag-purge-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.flag-purge-fn]
> Replace flag-diacritic arcs with epsilon arcs and drop the flag(s) from the alphabet. Parameter `flag`: a specific flag feature, or the empty string to purge all flags. (1) Iterate every state and every transition by index `i`. For each transition `tr_it`, if `purge_symbol(tr_it.input_symbol, flag)` or `purge_symbol(tr_it.output_symbol, flag)` is true (i.e. the symbol is a diacritic and matches `flag` or `flag` is empty), overwrite that transition slot with a new transition to the same target state but with input and output both `@_EPSILON_SYMBOL_@` and the original weight. (2) Build a set `extra_symbols` of every alphabet symbol for which `purge_symbol(symbol, flag)` holds, and remove them via `remove_symbols_from_alphabet`. Mutates this graph; returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-final-weight-fn]
> HfstTropicalTransducerTransitionData::WeightType

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-final-weight-fn]
> Return the final weight of state `s`. If `s > get_max_state()` (out of range), throw `StateIndexOutOfBoundsException`. Otherwise, if `s` is present in `final_weight_map`, return its mapped weight. If `s` is in range but not final, throw `StateIsNotFinalException`.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-flags-fn]
> StringSet

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-flags-fn]
> Return the set of flag-diacritic symbols present in the alphabet. Iterate over every symbol in `alphabet`; for each, if `FdOperation::is_diacritic(symbol)` is true, insert it into a `StringSet flags`. Return `flags`.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-input-symbols-fn]
> StringSet

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-input-symbols-fn]
> Return the set of input symbols that actually occur in transitions. Iterate over all states and all of their transitions; for each transition's data, insert its input symbol into a `StringSet retval`. Return `retval`. (This reflects transitions, not the declared alphabet.)

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-max-state-fn]
> HfstState

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-max-state-fn]
> Return the highest state number in use, computed as `state_vector.size() - 1` cast to `HfstState`. (Assumes the graph has at least one state; for an empty `state_vector` this underflows.)

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-output-symbols-fn]
> StringSet

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-output-symbols-fn]
> Return the set of output symbols that actually occur in transitions. Iterate over all states and all of their transitions; for each transition's data, insert its output symbol into a `StringSet retval`. Return `retval`. (This reflects transitions, not the declared alphabet.)

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-positions-of-unescaped-char-fn]
> std::vector<unsigned int>

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-positions-of-unescaped-char-fn]
> Return a vector of the 0-based indices in `str` where character `c` occurs without being escaped by `esc`. Iterate `i` from 0 to `str.length()-1`; whenever `str[i] == c`: if `i == 0` record `i`; else if the preceding char `str[i-1] == esc` skip it (escaped); else record `i`. Return the collected positions in order.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-prolog-arc-symbols-fn]
> bool

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-prolog-arc-symbols-fn]
> Parse a prolog arc string `str` (of form `"foo"` or `"foo":"bar"`) into `isymbol` and `osymbol` (out-params), returning whether parsing succeeded. Compute `quote_positions = get_positions_of_unescaped_char(str, '"', '\\')`. Validate by count: if there are exactly 2 quotes, they must be at index 0 and `str.length()-1` (else return false). If exactly 4 quotes, the first must be at 0 and the last at `str.length()-1`, the gap between the 2nd and 3rd quotes (`quote_positions[2]-quote_positions[1]`) must equal 2, and the char at `quote_positions[1]+1` must be `':'` (any failure -> false). Any other count -> false. Then extract: for the 2-quote case, take the substring between the quotes, `deprologize_symbol` it into `isymbol`; if `isymbol == "@_UNKNOWN_SYMBOL_@"` rewrite it to `"@_IDENTITY_SYMBOL_@"` (a single unknown means identity); set `osymbol = isymbol`. For the 4-quote case, take the inner substring of each quoted pair, deprologize the first into `isymbol` and the second into `osymbol`. Return true.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-stripped-line-fn]
> std::string

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-stripped-line-fn]
> Read one line (up to 255 chars) from either an istream `is` or a C `FILE *file`, strip newlines, bump `linecount`, and return it. Use a `char line[255]` buffer. If `file == NULL`, read via `is.getline(line, 255)`; if that read reaches eof (the condition `!is.getline(...).eof()` evaluating false), throw `EndOfStreamException`. Otherwise (`file != NULL`), read via `fgets(line, 255, file)`; if it returns NULL, throw `EndOfStreamException`. On a successful read, increment `linecount` by one, wrap `line` in a `std::string`, and return `strip_newlines(linestr)`. (Note: per the source the istream branch's eof test is structured such that a non-eof getline triggers the throw — replicate the exact condition `!is.getline(line,255).eof()` -> throw.)

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-symbol-number-fn]
> unsigned int

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-symbol-number-fn]
> Return the numeric id for `symbol` by delegating to `HfstTropicalTransducerTransitionData::get_number(symbol)`.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-transition-pairs-fn]
> StringPairSet

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.get-transition-pairs-fn]
> Return the set of `(input,output)` symbol pairs that occur on transitions. Create an empty `StringPairSet retval`. Iterate over all states and all of their transitions; for each transition's data, insert `StringPair(input_symbol, output_symbol)` into `retval`. Return `retval`. (Reflects actual transitions, not the declared alphabet; weights ignored.)

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.handle-list-match-fn]
> HfstState

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.handle-list-match-fn]
> Helper for `find_matches_for_merge` that splices in a list-symbol match. Parameters: `graph`/`graph_transition`, `merger`/`merger_transition`, output `result`, `result_state`, `state_map`, and `markers_added` (set, mutated). Let `graph_target = graph_transition.target` and `merger_target = merger_transition.target`. Call `find_target_state(graph_target, merger_target, state_map, result, was_new_state)` to obtain (and possibly create) the result state `retval`. Compute `transition_weight = graph_transition.weight + merger_transition.weight`. Allocate a brand-new intermediate state `extra_state = result.add_state()`. Add from `result_state` a marker transition to `extra_state` whose input is `"@" + graph_transition.input_symbol + "@"`, output is `"@" + graph_transition.output_symbol + "@"`, weight 0; insert that input marker string into `markers_added`. Then add from `extra_state` a transition to `retval` carrying the merger transition's input symbol, output symbol, and `transition_weight`. If `was_new_state` and both `graph.is_final_state(graph_target)` and `merger.is_final_state(merger_target)`, set `retval` final with weight `graph.get_final_weight(graph_target) + merger.get_final_weight(merger_target)`. Return `retval`.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.handle-match-fn]
> HfstState

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.handle-match-fn]
> Helper for `find_matches` (intersection). Parameters: `graph1`/`tr1`, `graph2`/`tr2` (the matching transitions), output `intersection`, current `state`, and `state_map`. Let `target1 = tr1.target` and `target2 = tr2.target`. Call `find_target_state(target1, target2, state_map, intersection, was_new_state)` to obtain (and possibly create) the product target state `retval`. Compute `transition_weight = tr1.weight + tr2.weight`. Add to `intersection` from `state` a transition to `retval` carrying `tr1`'s input symbol, `tr1`'s output symbol, and `transition_weight`. If `was_new_state` and both `graph1.is_final_state(target1)` and `graph2.is_final_state(target2)`, set `retval` final with weight `graph1.get_final_weight(target1) + graph2.get_final_weight(target2)`. Return `retval`.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.handle-non-list-match-fn]
> HfstState

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.handle-non-list-match-fn]
> Helper for `find_matches_for_merge` that copies a non-list-symbol transition into the merged result. Parameters: `graph`/`graph_transition`, `merger`, `merger_target` (the merger state to pair with — unchanged from the caller), output `result`, `result_state`, `state_map`. Let `graph_target = graph_transition.target`. Call `find_target_state(graph_target, merger_target, state_map, result, was_new_state)` to obtain (and possibly create) `retval`. Add to `result` from `result_state` a transition to `retval` carrying the graph transition's input symbol, output symbol, and weight (the merger contributes no weight here). If `was_new_state` and both `graph.is_final_state(graph_target)` and `merger.is_final_state(merger_target)`, set `retval` final with weight `graph.get_final_weight(graph_target) + merger.get_final_weight(merger_target)`. Return `retval`.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.has-negative-epsilon-cycles-fn]
> bool

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.has-negative-epsilon-cycles-fn]
> The annotated overload is the recursive DFS worker `has_negative_epsilon_cycles(HfstState state, float total_weight, std::map<HfstState,float> &state_weights)`. Look up `state` in `state_weights`. If present (a cycle back to a state on the current path): if `total_weight - state_weights[state] < 0` return true (negative-weight epsilon cycle found), else return false. Otherwise record `state_weights[state] = total_weight`. Then iterate the transitions of `state`; for each whose input and output symbols are both epsilon, recurse with `has_negative_epsilon_cycles(transition.target, total_weight + transition.weight, state_weights)` and return true if it returns true. After processing all transitions, erase `state` from `state_weights` (backtrack) and return false. (The public no-arg overload first scans for any epsilon:epsilon transition with weight < 0; if none, returns false; otherwise runs this worker from every state 0..get_max_state() with an empty `state_weights` and `total_weight=0`, returning true if any call does.)

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-alphabet]
> typedef std::set<HfstSymbol> HfstAlphabet

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-basic-transducer-fn]
> HfstBasicTransducer::HfstBasicTransducer(

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-basic-transducer-fn]
> Constructor that builds an `HfstBasicTransducer` equivalent to an `HfstTransducer transducer`. Call `ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(transducer)`, which returns a heap-allocated `HfstBasicTransducer *fsm`. Copy `fsm->state_vector`, `fsm->final_weight_map`, and `fsm->alphabet` into this object's members, then `delete fsm`.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-number]
> typedef unsigned int HfstNumber

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-number-pair]
> typedef std::pair<HfstNumber, HfstNumber> HfstNumberPair

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-number-pair-substitutions]
> typedef std::map<HfstNumberPair, HfstNumberPair>

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-number-vector]
> typedef std::vector<HfstNumber> HfstNumberVector

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-symbol]
> typedef HfstTropicalTransducerTransitionData::SymbolType HfstSymbol

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-symbol-pair]
> typedef std::pair<HfstSymbol, HfstSymbol> HfstSymbolPair

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-symbol-pair-set]
> typedef std::set<HfstSymbolPair> HfstSymbolPairSet

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-symbol-pair-vector]
> typedef std::vector<HfstSymbolPair> HfstSymbolPairVector

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.hfst-symbol-set]
> typedef std::set<HfstSymbol> HfstSymbolSet

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.initialize-alphabet-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.initialize-alphabet-fn]
> Static helper. Insert the three special symbols into the given `HfstAlphabet &alpha`: the epsilon symbol (`HfstTropicalTransducerTransitionData::get_epsilon()`), the unknown symbol (`get_unknown()`), and the identity symbol (`get_identity()`). Returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.initialize-state-vector-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.initialize-state-vector-fn]
> Optimization hint. Call `state_vector.reserve(number_of_states)` to reserve capacity for that many states. Does not create states or change the logical size; returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.initialize-transition-vector-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.initialize-transition-vector-fn]
> Optimization hint for a single state. Call `add_state(state_number)` to ensure the state exists, then `state_vector[state_number].reserve(number_of_transitions)` to reserve capacity for that many outgoing transitions. Returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.insert-transducer-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.insert-transducer-fn]
> Splice a copy of `graph` between this graph's states `state1` and `state2` using epsilon transitions. (1) Call `add_state()` to allocate a fresh state, whose number is `offset` (also the renumbering offset for graph's states). (2) Copy graph's transitions: iterate its states with index `source_state` starting at 0, and for each transition add to `source_state + offset` a transition to `tr.target + offset` with the same input symbol, output symbol, and weight. (3) For each final state in `graph.final_weight_map` (state number `it.first`, weight `it.second`), add an epsilon:epsilon transition from `it.first + offset` to `state2` with weight `it.second`. (4) Add an epsilon:epsilon transition from `state1` to `offset` with weight 0. Mutates this graph; returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.intersect-fn]
> HfstBasicTransducer

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.intersect-fn]
> Static function returning the intersection of `graph1` and `graph2`. Create an empty result `retval`, an empty `StateMap state_map`, and an empty `agenda` set. Sort the arcs of both inputs (`graph1.sort_arcs()`, `graph2.sort_arcs()`). Seed `state_map[StatePair(0,0)] = 0` (initial state of the product is 0). If both `graph1` and `graph2` have state 0 final, set `retval`'s state 0 final with weight `min(graph1.get_final_weight(0), graph2.get_final_weight(0))`. Then call `find_matches(graph1, 0, graph2, 0, retval, 0, state_map, agenda)` to build the rest. Return `retval`. (Note the initial-state final weight uses min, whereas matched intermediate states use the sum of final weights via `handle_match`.)

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-final-state-fn]
> bool

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-final-state-fn]
> Return true iff state `s` is final, i.e. `final_weight_map.find(s) != final_weight_map.end()` (s is a key of the final-weight map).

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-infinitely-ambiguous-fn]
> bool

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-infinitely-ambiguous-fn]
> The annotated overload is the recursive worker `is_infinitely_ambiguous(HfstState state, std::set<HfstState> &epsilon_path_states, std::vector<unsigned int> &states_handled)`. It detects a cycle reachable purely by input-epsilon (or flag-diacritic) transitions. If `states_handled[state] != 0`, this state was fully explored without finding a cycle: return false. Iterate the transitions of `state`; for each whose input symbol is epsilon OR is a flag diacritic (`FdOperation::is_diacritic`): insert `state` into `epsilon_path_states`; if the transition's target is already in `epsilon_path_states`, return true (epsilon cycle found); otherwise recurse with `is_infinitely_ambiguous(target, epsilon_path_states, states_handled)` and return true if it returns true; then erase `state` from `epsilon_path_states` (backtrack). After all transitions, set `states_handled[state] = 1` (mark handled) and return false. (The public no-arg overload allocates `states_handled` of size `get_max_state()+1` initialized to 0 and an empty `epsilon_path_states`, then runs the worker from every state 0..max_state, returning true if any returns true.)

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-list-symbol-fn]
> bool

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-list-symbol-fn]
> Determine whether a transition's symbol is a registered list symbol. Parameters: `transition_data` and `list_symbols` (a map from list-symbol name to its member set). Read `isymbol` and `osymbol` from `transition_data`. If `isymbol != osymbol`, throw the C-string `"is_list_symbol: input and output symbols must be the same"`. Otherwise return whether `isymbol` is a key in `list_symbols` (`list_symbols.find(isymbol) != list_symbols.end()`).

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-lookup-infinitely-ambiguous-fn]
> bool

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-lookup-infinitely-ambiguous-fn]
> The annotated overload is the recursive worker `is_lookup_infinitely_ambiguous(const HfstOneLevelPath &s, unsigned int &index, HfstState state, std::set<HfstState> &epsilon_path_states, StringVector &fds, bool obey_flags)`. It detects whether looking up the input sequence `s.second` can loop forever via input-epsilon/flag transitions. Set `only_epsilons = (s.second.size() == index)` (true when the input has been fully consumed). Iterate the transitions of `state`. For each transition: compute `possible_flag = is_possible_flag(transition.input, fds, obey_flags)` (which pushes the flag onto `fds` and validates it). CASE 1 — if the input symbol is epsilon OR `possible_flag`: insert `state` into `epsilon_path_states`; if the target is already in `epsilon_path_states`, return true; else recurse with the same `index`, target state, the shared `epsilon_path_states` and `fds`, returning true on a true result; then erase `state` from `epsilon_path_states`, and if `possible_flag` pop the flag back off `fds`. CASE 2 — else if `!only_epsilons` (an input-consuming transition is allowed): set `continu` true if the transition's input equals `s.second.at(index)`, or if the input is `@_UNKNOWN_SYMBOL_@`/`@_IDENTITY_SYMBOL_@` and `s.second.at(index)` is not in `alphabet`. If `continu`: increment `index`, recurse with a fresh empty `epsilon_path_states` for the target state (returning true on true), then decrement `index` back. After all transitions, return false. (The public overloads taking `HfstOneLevelPath` or `StringVector` seed `index=0`, `epsilon_path_states={0}`, empty `fds`, and start from `INITIAL_STATE`.)

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-possible-flag-fn]
> bool

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-possible-flag-fn]
> Decide whether `symbol` is a flag diacritic that may be traversed given the flags collected so far in `fds`, and provisionally record it. If `FdOperation::is_diacritic(symbol)` is false, return false. Otherwise construct a `FlagDiacriticTable FdT`, push `symbol` onto `fds`, and: if `!obey_flags` OR `FdT.is_valid_string(fds)`, return true (leaving `symbol` pushed onto `fds`); else pop `symbol` back off `fds` and return false. (Side effect: on a true return `fds` is left with `symbol` appended; the caller is responsible for popping it later.)

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-possible-transition-fn]
> bool

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-possible-transition-fn]
> Decide whether `transition` can be taken at position `lookup_index` of `lookup_path`, setting `input_symbol_consumed` accordingly. Parameters: `transition`, `lookup_path`, `lookup_index`, `alphabet`, out-param `input_symbol_consumed`, and optional `fds_so_far` (defaults NULL). Let `isymbol = transition.input_symbol`. (1) If not at end of path (`lookup_index != lookup_path.size()`): if `isymbol` equals `lookup_path.at(lookup_index)`, OR `isymbol` is identity/unknown and `lookup_path.at(lookup_index)` is not in `alphabet`, then set `input_symbol_consumed = true` and return true. (2) Regardless of position: if `isymbol` is epsilon, set `input_symbol_consumed = false` and return true. (3) If `isymbol` is a flag diacritic: if `fds_so_far == NULL`, set `input_symbol_consumed = false` and return true; else build a `FlagDiacriticTable FdT`, temporarily push `isymbol` onto `*fds_so_far`, test `FdT.is_valid_string(*fds_so_far)`, pop it back off, and if valid set `input_symbol_consumed = false` and return true. (4) Otherwise return false (no match).

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-special-symbol-fn]
> bool

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.is-special-symbol-fn]
> Static. Return true iff `symbol` starts with the two characters `@_`. If `symbol.size() < 2` return false; if `symbol[0] == '@' && symbol[1] == '_'` return true; otherwise return false.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.iterator]
> typedef HfstBasicStates::iterator iterator

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.longest-path-size-fn]
> int

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.longest-path-size-fn]
> Return the length of the longest string accepted by this graph, or -1 if none. Compute `states_sorted = topsort(MaximumDistance)`, a vector indexed by distance whose entries are sets of states at that maximum distance from the start. If `states_sorted` is non-empty, iterate `distance` from `states_sorted.size()-1` down to 0; for each, iterate the states in `states_sorted.at(distance)`, and if any is a final state, return `distance` immediately. If no final state is found at any distance, return -1.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.lookup-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.lookup-fn]
> Recursive DFS that collects all two-level paths matching `lookup_path` into `results`. Parameters: `lookup_path`, `results` (mutated), `state`, `lookup_index`, `path_so_far` (mutated then restored), `alphabet`, `Eh` (an `HfstEpsilonHandler` passed BY VALUE for epsilon-cycle limiting), `max_epsilon_cycles`, optional `max_weight` (float*, NULL = no limit), `max_number` (-1 = unlimited), optional `flag_diacritic_path`. Early returns: if `!Eh.can_continue(state)` return; if `max_weight != NULL && path_so_far.first > *max_weight` return; if `max_number >= 0 && (size_t)max_number <= results.size()` return. If `lookup_index == lookup_path.size()` (input exhausted) and `state` is final, call `add_to_results(results, path_so_far, get_final_weight(state), max_weight)`. Then, in all cases, iterate the transitions of `state`. For each transition, set `input_symbol_consumed=false` and test `is_possible_transition(transition, lookup_path, lookup_index, alphabet, input_symbol_consumed, flag_diacritic_path)`. If possible: compute the emitted pair `(istr,ostr)` — if the input symbol is identity, both `istr` and `ostr` are `lookup_path.at(lookup_index)`; else `istr` is `lookup_path.at(lookup_index)` when the input is unknown, otherwise the transition's input symbol, and `ostr` is the transition's output symbol. Call `push_back_to_two_level_path(path_so_far, (istr,ostr), transition.weight, flag_diacritic_path)`. If `input_symbol_consumed`, increment `lookup_index` and use a fresh heap `HfstEpsilonHandler(max_epsilon_cycles)`; otherwise push `state` onto a copy of `Eh` and use that. Recurse `lookup(lookup_path, results, transition.target, lookup_index, path_so_far, alphabet, *Ehp, max_epsilon_cycles, max_weight, max_number, flag_diacritic_path)`. After returning, restore state: if input was consumed, decrement `lookup_index` and delete the heap handler; then `pop_back_from_two_level_path` to undo the pushed pair and weight. Returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.marker2weight-fn]
> bool

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.marker2weight-fn]
> Try to parse a weight encoded as a marker symbol `@<float>@`, writing the parsed value into `weight`. If `str.size() < 3` return false. If the first char is not `'@'` or the last char is not `'@'` return false. Take `weight_string = str.substr(1, str.size()-2)` (the text between the at-signs), parse it via a `stringstream >> weight`; if the parse fails (`sstream.fail()`) return false. Otherwise return true with `weight` set.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.merge-fn]
> HfstBasicTransducer

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.merge-fn]
> Static function that merges `graph` with `merger`, expanding list symbols. Parameters: `graph`, `merger`, `list_symbols` (map name -> member set), and `markers_added` (set, mutated). Create empty `result`, `state_map`, `agenda`. Sort the arcs of both inputs. Seed `state_map[StatePair(0,0)] = 0`. If both `graph` and `merger` have state 0 final, set `result`'s state 0 final with weight `graph.get_final_weight(0) + merger.get_final_weight(0)` (note: sum, not min). Then call `find_matches_for_merge(graph, 0, merger, 0, result, 0, state_map, agenda, list_symbols, markers_added)` inside a try block; if it throws a `const char *msg`, rethrow as `TransducersAreNotAutomataException` carrying `std::string(msg)`. Return `result`.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.parse-prolog-arc-line-fn]
> bool

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.parse-prolog-arc-line-fn]
> Static. Parse a prolog arc line of form `arc(NAME, SOURCE, TARGET, SYMBOL).` (SYMBOL may carry a trailing `, weight`) and add the transition to `graph`. Use four 100-char buffers and `n = sscanf(line, "arc(%[^,], %[^,], %[^,], %[^\t\n]", namestr, sourcestr, targetstr, symbolstr)`. Build `symbol` from `symbolstr`. Call `strip_ending_parenthesis_and_comma(symbol)` to drop the trailing `).`; if it fails return false. If `n != 4` return false. If `namestr != graph.name` return false. Compute `source = atoi(sourcestr)`, `target = atoi(targetstr)`. Initialize `weight = 0` and call `extract_weight(symbol, weight)` to peel any trailing weight off `symbol`; if it fails return false. Call `get_prolog_arc_symbols(symbol, isymbol, osymbol)` to split into input/output symbols; if it fails return false. Then `graph.add_transition(source, HfstBasicTransition(target, isymbol, osymbol, weight))` and return true.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.parse-prolog-final-line-fn]
> bool

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.parse-prolog-final-line-fn]
> Static. Parse a prolog final-state line of form `final(NAME, number).` or `final(NAME, number, weight).` and mark the state final in `graph`. First count commas in `line` (scan with `find(',', pos+1)`). If exactly 1 comma: `sscanf(line, "final(%[^,], %[^)]).", namestr, finalstr)` must return 2, else false; `weight` stays 0. If exactly 2 commas: `sscanf(line, "final(%[^,], %[^,], %[^)]).", namestr, finalstr, weightstr)` must return 3, else false; then parse `weight` from `weightstr` via `istringstream >> weight`, returning false if the float parse fails. Any other comma count returns false. If `namestr != graph.name` return false. Otherwise `graph.set_final_weight(atoi(finalstr), weight)` and return true.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.parse-prolog-network-line-fn]
> bool

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.parse-prolog-network-line-fn]
> Static. Parse a prolog network header line of form `network(NAME).` and store the name into `graph`. Use a 100-char buffer and `n = sscanf(line, "network(%s", namearr)`; if `n != 1` return false. Build `namestr` from `namearr`, then call `strip_ending_parenthesis_and_comma(namestr)` to strip the trailing `).`; if that fails return false. Set `graph.name = namestr` and return true.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.parse-prolog-symbol-line-fn]
> bool

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.parse-prolog-symbol-line-fn]
> Static. Parse a prolog symbol-declaration line of form `symbol(NAME, "foo").` and add the symbol to `graph`'s alphabet. Use two 100-char buffers and `n = sscanf(line, "symbol(%[^,], %s", namearr, symbolarr)`; if `n != 2` return false. Build `namestr` and `symbolstr`. If `namestr != graph.name` return false. Call `strip_ending_parenthesis_and_comma(symbolstr)`; if it fails return false. Call `strip_quotes_from_both_sides(symbolstr)`; if it fails return false. Then `graph.add_symbol_to_alphabet(deprologize_symbol(symbolstr))` and return true.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.path-sizes-fn]
> std::vector<unsigned int>

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.path-sizes-fn]
> Return the lengths of strings accepted by this graph, in descending order; an empty vector if none accepted. Compute `states_sorted = topsort(MinimumDistance)` (vector indexed by minimum distance, each entry a set of states at that distance). Create empty `result`. If `states_sorted` is non-empty, iterate `distance` from `states_sorted.size()-1` down to 0; for each distance, scan the states in `states_sorted.at(distance)` and as soon as one is final, push `distance` onto `result` and break to the next distance. Return `result`.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.pop-back-from-two-level-path-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.pop-back-from-two-level-path-fn]
> Undo the last `push_back_to_two_level_path`. Parameters: `path` (an `HfstTwoLevelPath`, mutated), `weight`, and optional `fds_so_far` (defaults NULL). If `fds_so_far != NULL`: read the last pair `sp = path.second.back()`, and if `sp.first` is a flag diacritic (`FdOperation::is_diacritic`), pop the last entry off `*fds_so_far`. Then `path.second.pop_back()` (remove the last symbol pair) and `path.first -= weight` (subtract the weight back off the cumulative weight). Returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-alphabet-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-alphabet-fn]
> Print the graph's `alphabet` to `std::cerr`. Iterate the alphabet set in order; before each element that is not the first (i.e. `it != *alphabet.begin()`), write `", "`, then write the element. After all elements, write `std::endl`. Returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-prolog-arc-symbols-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-prolog-arc-symbols-fn]
> Print the input (and, when needed, output) symbol of a transition's `data` in prolog quoted form. The annotated overload writes to a `FILE *file` (a sibling overload writes to a `std::ostream`). Compute `symbol = prologize_symbol(data.get_input_symbol())` and print it as `"symbol"` (double-quoted). Then, if the input symbol differs from the output symbol OR the input symbol equals `"@_UNKNOWN_SYMBOL_@"`, compute `symbol = prologize_symbol(data.get_output_symbol())` and print `:"symbol"` (a colon followed by the quoted output). Returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-xfst-arc-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-xfst-arc-fn]
> Print a transition's `data` in xfst arc syntax. The annotated overload writes to a `FILE *file` (a sibling overload writes to a `std::ostream`). If the input symbol differs from the output symbol, print `"<"`. Take `s = data.get_input_symbol()`, call `xfstize_symbol(s)` (in-place transform), and print `s`. Then, if the input symbol differs from the output symbol OR the output symbol equals `"@_UNKNOWN_SYMBOL_@"`, set `s = data.get_output_symbol()`, `xfstize_symbol(s)`, and print `":" + s`. Finally, if input differs from output, print `">"`. Returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-xfst-state-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.print-xfst-state-fn]
> Print a state label in xfst form. The annotated overload writes to a `FILE *file` (a sibling writes to a `std::ostream`). If `state == INITIAL_STATE` (0), print `"S"`. If `is_final_state(state)`, print `"f"`. Then print `"s"` followed by the state number (i.e. `"s%i"`). Returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.prologize-symbol-fn]
> std::string

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.prologize-symbol-fn]
> Convert an internal HFST symbol into its prolog-text representation. If `symbol == "0"` return `"%0"`; if `symbol == "?"` return `"%?"`; if `symbol == "@_EPSILON_SYMBOL_@"` return `"0"`; if `symbol == "@_UNKNOWN_SYMBOL_@"` return `"?"`; if `symbol == "@_IDENTITY_SYMBOL_@"` return `"?"` (both unknown and identity prologize to `?`). Otherwise copy `symbol` and escape it: replace all `\` (backslash) with `\\` (double backslash) FIRST, then replace all `"` (doublequote) with `\"` (backslash-doublequote). Return the result.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.prune-alphabet-after-substitution-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.prune-alphabet-after-substitution-fn]
> Remove from the alphabet those symbols in the given set `symbols` (a `std::set<unsigned int>` of symbol numbers) that no longer occur in any transition. If `symbols` is empty, return immediately. Allocate a `std::vector<bool> symbols_found` sized `HfstTropicalTransducerTransitionData::get_max_number() + 1`, all false. Iterate every state and every transition; for each transition's data, set `symbols_found[data.get_input_number()] = true` and `symbols_found[data.get_output_number()] = true`. Then for each `symbol` number in `symbols`, if `symbols_found[symbol]` is false, erase `HfstTropicalTransducerTransitionData::get_symbol(symbol)` from the `alphabet`. Mutates `alphabet`; returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.prune-alphabet-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.prune-alphabet-fn]
> Remove from the alphabet every symbol that does not occur in any transition. Parameter `force` (default true). Compute `symbols_found = symbols_used()` (the set of symbols on transitions). Set `unknowns_or_identities_used` true iff `symbols_found` contains `"@_UNKNOWN_SYMBOL_@"` or `"@_IDENTITY_SYMBOL_@"`. If `!force` and `unknowns_or_identities_used`, return without pruning (cannot safely prune). Insert the three special symbols `"@_EPSILON_SYMBOL_@"`, `"@_UNKNOWN_SYMBOL_@"`, `"@_IDENTITY_SYMBOL_@"` into `symbols_found` so they are always kept. Build `symbols_not_found` = every alphabet symbol not in `symbols_found`. Erase each of those from `alphabet`. Mutates `alphabet`; returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.purge-symbol-fn]
> bool

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.purge-symbol-fn]
> Decide whether `symbol` must be purged after flag `flag` has been eliminated. If `symbol` is not a flag diacritic (`!FdOperation::is_diacritic(symbol)`), return false. Otherwise: if `flag` is the empty string (all flags eliminated), return true; else if `FdOperation::get_feature(symbol) == flag` (this diacritic belongs to the eliminated feature), return true; otherwise return false.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.push-back-to-two-level-path-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.push-back-to-two-level-path-fn]
> Extend a lookup path by one symbol pair. Parameters: `path` (an `HfstTwoLevelPath` = pair of cumulative weight `.first` and a vector of `StringPair` `.second`, mutated), `sp` (the `StringPair` to append), `weight` (added to the cumulative weight), and `fds_so_far` (a `StringVector*`, default NULL). Push `sp` onto `path.second`; add `weight` to `path.first`. If `fds_so_far != NULL` and `sp.first` (the input symbol) is a flag diacritic (`FdOperation::is_diacritic`), push `sp.first` onto `*fds_so_far`. Returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.read-in-att-format-fn]
> HfstBasicTransducer

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.read-in-att-format-fn]
> Read one AT&T-format transducer from istream `is` or `FILE *file` (file used if non-NULL, else `is`) and return it. Parameters also include `epsilon_symbol`, `linecount` (mutated by reference), and `warn_negs`. First, if at end of input (`is.eof()` when `file==NULL`, else `feof(file)`), throw `EndOfStreamException`. Create an empty `retval` and a `char line[255]`. Loop: read a line via `is.getline(line,255)` (break out of the loop when that read reports eof, i.e. condition `!is.getline(...).eof()` is false) or via `fgets(line,255,file)` (break when it returns NULL). On a read, increment `linecount`. If the line is empty (`line[0]=='\0'`, or `"\n"`, or windows `"\r\n"`): consume one more char to ensure EOF is reached (`is.get()` or `fgetc(file)`) and break (an empty line denotes an empty transducer, accepted only as the sole transducer in the stream). If the line begins with `'-'` (the `"--"` separator), return `retval` immediately. Otherwise call `retval.add_att_line(line, epsilon_symbol, warn_negs)`; if it returns false, throw `NotValidAttFormatException` with the line as message. After the loop ends, return `retval`.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.read-in-prolog-format-fn]
> HfstBasicTransducer

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.read-in-prolog-format-fn]
> Read one prolog-format transducer from istream `is` or `FILE *file` (file used if non-NULL, else `is`) and return it; `linecount` is mutated by reference. Create empty `retval`. First loop: repeatedly call `get_stripped_line(is, file, linecount)` (if it throws `EndOfStreamException`, rethrow as `NotValidPrologFormatException`); skip lines that are non-empty and start with `'#'` (comments); stop at the first non-comment line. Call `parse_prolog_network_line(linestr, retval)`; if it returns false, throw `NotValidPrologFormatException` with message "first line not valid prolog: " + line. Second loop: repeatedly get a stripped line; if it is empty (the prolog separator) return `retval`; if `get_stripped_line` throws `EndOfStreamException`, return `retval`. For each such line, try `parse_prolog_arc_line` then `parse_prolog_final_line` then `parse_prolog_symbol_line` (short-circuit OR); if none succeeds, throw `NotValidPrologFormatException` with "line not valid prolog: " + line. (The trailing unreachable `HFST_THROW(NotValidPrologFormatException)` should never execute.)

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-final-weight-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-final-weight-fn]
> Make state `s` non-final by erasing `s` from `final_weight_map` (`final_weight_map.erase(s)`). No-op if `s` was not final. Returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-symbol-from-alphabet-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-symbol-from-alphabet-fn]
> Remove `symbol` from the graph's `alphabet` (`alphabet.erase(symbol)`). No-op if not present. (Caller's responsibility: removing a symbol still used in transitions can cause unexpected results.) Returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-symbols-from-alphabet-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-symbols-from-alphabet-fn]
> Take an `HfstSymbolSet symbols`; for each `symbol` in it, erase it from the graph's `alphabet`. Returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-transition-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-transition-fn]
> Remove every transition of state `s` that matches `transition`. Parameters: `s`, `transition`, `remove_symbols_from_alphabet` (default false). If `state_vector.size() <= s` (state does not exist), return. Let `transitions = state_vector[s]`. Scan all its transitions; a transition matches iff its input symbol, output symbol, AND target state all equal those of `transition` (weight ignored). Collect matching iterators onto a stack, then pop and erase them (reverse order so iterators stay valid). If `remove_symbols_from_alphabet` is true: recompute `alpha = symbols_used()`, and if `transition`'s input symbol is no longer in `alpha`, call `remove_symbol_from_alphabet` on it; likewise for the output symbol. Mutates `state_vector[s]` and possibly `alphabet`; returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-transitions-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.remove-transitions-fn]
> Remove in place every transition equal to symbol pair `sp` (compared by symbol number, weight ignored), and prune the now-unused symbols from the alphabet. Compute `in_match = get_number(sp.first)` and `out_match = get_number(sp.second)`. Set flags `in_match_used = out_match_used = false`. Iterate every state; for each, iterate its transitions by index `i`: read `in_tr = get_input_number()`, `out_tr = get_output_number()`. If `in_tr == in_match && out_tr == out_match`, erase that transition (`it.erase(it.begin()+i)`) — note `i` is not decremented after erase, so the immediately following transition is skipped from re-examination. Otherwise, if `in_tr == in_match || out_tr == in_match` set `in_match_used = true`; if `in_tr == out_match || out_tr == out_match` set `out_match_used = true`. After all states: if `!in_match_used`, erase `sp.first` from `alphabet`; if `!out_match_used`, erase `sp.second` from `alphabet`. Returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.set-final-weight-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.set-final-weight-fn]
> Make state `s` final with weight `weight`. Call `add_state(s)` to ensure `s` (and any lower-numbered missing states) exist, then set `final_weight_map[s] = weight`. Returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.sort-distance]
> enum SortDistance {
>   MaximumDistance;
>   MinimumDistance;
> }

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.state-map]
> typedef std::map<StatePair, HfstState> StateMap

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.state-pair]
> typedef std::pair<HfstState, HfstState> StatePair

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.states-fn]
> std::vector<HfstState>

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.states-fn]
> Return a `std::vector<HfstState>` listing all state numbers `0..get_max_state()` in order. Allocate `retval` of size `get_max_state() + 1` (filled with 0), then set `retval[i] = i` for each `i` in `[0, get_max_state()]`. Return `retval`.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.strip-ending-parenthesis-and-comma-fn]
> bool

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.strip-ending-parenthesis-and-comma-fn]
> If `str` ends in `")."`, strip that trailing `")."` and return true; else return false. Precisely: if `str.size() < 3` return false. If the second-to-last char is not `')'` or the last char is not `'.'` return false. Otherwise `str.erase(str.length() - 2)` (drops the final two chars `)` and `.`) and return true. Mutates `str` only on success.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.strip-newlines-fn]
> std::string

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.strip-newlines-fn]
> Erase any trailing `\n` and `\r` characters from `str` and return `str`. Iterate `i` from `str.length()-1` downward; while `str[i]` is `'\n'` or `'\r'`, erase that char (`str.erase(i, 1)`); break at the first non-newline char. Mutates and returns `str`.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.strip-quotes-from-both-sides-fn]
> bool

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.strip-quotes-from-both-sides-fn]
> If `str` is of form `"...."` (double-quote on both ends, with content between), strip the surrounding quotes and return true; else return false. Precisely: if `str.size() < 3` return false. If `str[0] != '"'` or the last char `!= '"'` return false. Otherwise erase the first char and erase the (now-)last char, then return true. Mutates `str` only on success.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.subst-map]
> typedef std::map<HfstSymbol, HfstBasicTransducer> SubstMap

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.substitute-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.substitute-fn]
> In-place substitute every transition equal to `old_sp` with the set of transitions `new_sps` (this is `substitute_(const HfstSymbolPair&, const HfstSymbolPairSet&)`). If `new_sps` is empty, delegate to `remove_transitions(old_sp)` and return. Compute `old_input_number`/`old_output_number` from `old_sp`. Set `substitution_performed = false`. For each state: maintain a local `new_transitions` list. For each transition (by index `i`) whose input number and output number both equal the old pair: set `substitution_performed = true`; rewrite that transition slot in place to the FIRST element of `new_sps` (same target state, same weight, input/output numbers from `new_sps.begin()->first`/`->second`); then iterate `IT` from `new_sps.begin()` to end, building for each a transition with the same target and weight and `IT`'s symbol numbers and pushing it onto `new_transitions` (note: because the loop starts at `begin()`, the first substituting pair is appended again here in addition to overwriting the slot, so it ends up duplicated). After scanning a state's transitions, append all `new_transitions` to that state. After all states, if `substitution_performed`, call `add_symbols_to_alphabet(new_sps)`. Finally build a number set `syms = {old_input_number, old_output_number}` and call `prune_alphabet_after_substitution(syms)`. Mutates this graph; returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.substitution-data]
> struct substitution_data {
>   HfstState origin_state;
>   HfstState target_state;
>   HfstTropicalTransducerTransitionData::WeightType weight;
>   HfstBasicTransducer *substituting_graph;
> }

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.substitution-data.substitution-data-fn]
> substitution_data(

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.substitution-data.substitution-data-fn]
> Constructor for the `substitution_data` struct. Parameters `origin`, `target`, `weight`, `substituting` (an `HfstBasicTransducer*`). Store them into the members: `origin_state = origin`, `target_state = target`, `this->weight = weight`, `substituting_graph = substituting`.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.swap-state-numbers-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.swap-state-numbers-fn]
> Exchange state numbers `s1` and `s2` throughout the graph. (1) Swap the two states' transition lists: `tmp = state_vector[s1]; state_vector[s1] = state_vector[s2]; state_vector[s2] = tmp`. (2) Iterate every state and every transition; for each whose target is `s1`, rewrite the transition (same input/output/weight) to target `s2`, and for each whose target is `s2`, rewrite it to target `s1` (only replace the slot when the target actually changes). (3) Update `final_weight_map`: look up `s1` and `s2`. If both are final, swap their weights. If only `s1` is final, erase `s1` and set `final_weight_map[s2]` to its weight. If only `s2` is final, erase `s2` and set `final_weight_map[s1]` to its weight. (Note these three `if` blocks are independent, so the both-final case also runs the next two using the already-swapped map entries.) Mutates this graph; returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.symbols-used-fn]
> HfstBasicTransducer::HfstAlphabet

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.symbols-used-fn]
> Return the set (`HfstAlphabet`) of symbols actually used on transitions. Create empty `retval`. Iterate every state and every transition; for each transition's data, insert both its input symbol and its output symbol into `retval`. Return `retval`. (Reflects transitions, not the declared alphabet; weights ignored.)

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.topological-sort]
> struct TopologicalSort {
>   std::vector<int> distance_of_state;
>   std::vector<std::set<HfstState> > states_at_distance;
> }

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.topological-sort.set-biggest-state-number-fn]
> HFSTDLL void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.topological-sort.set-biggest-state-number-fn]
> Initialize the `TopologicalSort`'s `distance_of_state` vector to size `biggest_state_number + 1`, every element set to `-1` (meaning "no distance assigned yet"). Returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.topological-sort.set-state-at-distance-fn]
> HFSTDLL void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.topological-sort.set-state-at-distance-fn]
> Record that `state` sits at distance `distance` in the `TopologicalSort`. Parameters: `state`, `distance`, `overwrite`. If `state > distance_of_state.size() - 1` (out of range) print an error to `std::cerr` (but continue). Grow `states_at_distance` by pushing empty sets until its size is at least `distance + 1`. Read `previous_distance = distance_of_state[state]`; if it is not `-1` and not equal to `distance` and `overwrite` is true, erase `state` from `states_at_distance[previous_distance]`. Then insert `state` into `states_at_distance[distance]` and set `distance_of_state[state] = distance`. Mutates the sort's members; returns nothing. (With `overwrite` true this keeps the maximum-distance placement when revisited; with it false the original distance entry is left in place.)

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.topsort-fn]
> std::vector<std::set<HfstState> >

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.topsort-fn]
> Compute a topological sort of states by distance from the start state and return a `std::vector<std::set<HfstState>>` where index `d` holds the set of states at distance `d`. Parameter `dist` is `MaximumDistance` or `MinimumDistance`. If `state_vector` is empty, return an empty vector. Create a `TopologicalSort TopSort`; let `biggest_state_number = state_vector.size() - 1`; call `TopSort.set_biggest_state_number(biggest_state_number)`. Place the start state 0 at `current_distance = 0` via `TopSort.set_state_at_distance(0, 0, dist == MaximumDistance)` (the `overwrite` flag is true only for MaximumDistance). Then loop: set `new_states_found = false`; collect into `new_states` the targets of all transitions out of every state currently at `current_distance` (setting `new_states_found` true if any transition exists); for each such target call `set_state_at_distance(target, current_distance + 1, dist == MaximumDistance)`; increment `current_distance`; repeat while `new_states_found`. Return `TopSort.states_at_distance`. (For MaximumDistance, revisiting a state moves it to the larger distance via overwrite; for MinimumDistance the first/smallest assignment is kept.)

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.weight2marker-fn]
> std::string

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.weight2marker-fn]
> Encode a float `weight` as a marker symbol string. Format `weight` via `std::ostringstream` (default float formatting), then return `"@" + <that string> + "@"`.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-att-format-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-att-format-fn]
> Serialize the whole graph in AT&T text format into a caller-provided `char *ptr` buffer (the annotated overload; weights printed iff `write_weights`, default true). Track `cwt` (total chars written so far) and write via `sprintf(ptr + cwt, ...)`, advancing `cwt` by each call's return value. Iterate states with index `source_state` starting at 0. For each transition: take input and output symbols and in each replace " "->`@_SPACE_@`, `@_EPSILON_SYMBOL_@`->`@0@`, tab->`@_TAB_@`; write `"%i\t%i\t%s\t%s"` (source_state, target, isymbol, osymbol); if `write_weights`, append `"\t%f"` with the transition weight; then write a newline `"\n"`. After a state's transitions, if `is_final_state(source_state)`: write `"%i"` (source_state); if `write_weights`, append `"\t%f"` with `get_final_weight(source_state)`; then write `"\n"`. Increment `source_state`. Returns nothing. (No bounds checking on `ptr`.)

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-att-format-number-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-att-format-number-fn]
> Write the graph in AT&T format to `FILE *file` using symbol NUMBERS instead of names (weights iff `write_weights`, default true). Iterate states with index `source_state` from 0. For each transition: `fprintf` `"%i\t%i\t%i\t%i"` (source_state, target state, input number, output number); if `write_weights` append `"\t%f"` with the transition weight; then `"\n"`. Note: the final-state line is emitted INSIDE the per-transition loop, guarded by `is_final_state(source_state)` — so for a final state with multiple transitions its final line is printed once per transition, and a final state with no transitions emits no final line (a quirk of the source vs the name-based overload). The final line is `"%i"` (source_state), plus `"\t%f"` of `get_final_weight(source_state)` when `write_weights`, plus `"\n"`. Increment `source_state`. Returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-prolog-format-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-prolog-format-fn]
> Write the graph in prolog format to `FILE *file` (the annotated overload; a sibling writes to `std::ostream`). Parameters: `file`, network `name`, `write_weights` (default true). Let `identifier = name.c_str()`. If `name` contains a comma, throw `HfstException` with message "no commas allowed in the name of prolog networks". Print `"network(<name>).\n"`. Then print symbol declarations for alphabet symbols not used in arcs: compute `symbols_used_ = symbols_used()`, call `initialize_alphabet(symbols_used_)` (adds the special symbols so they are treated as used/excluded), and for each `it` in `alphabet` not in `symbols_used_`, print `"symbol(<name>, \"<prologize_symbol(it)>\").\n"`. Then print arcs: iterate states with index `source_state` from 0; for each transition print `"arc(<name>, <source_state>, <target>, "`, then call `print_prolog_arc_symbols(file, data)` for the symbol(s); if `write_weights`, print `", "` then `write_weight(file, weight)`; then print `").\n"`. Finally print final states: for each `(state, weight)` in `final_weight_map` print `"final(<name>, <state>"`, and if `write_weights` print `", "` then `write_weight`, then `").\n"`. Returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-xfst-format-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-in-xfst-format-fn]
> Write the graph in xfst (human-readable state listing) format to `FILE *file` (the annotated overload). Parameters: `file`, `write_weights` (default true, but UNUSED — cast to void). Iterate over states with an index `source_state` starting at 0 (one iteration per state in `state_vector`). For each state: call `print_xfst_state(file, source_state)` then print `":\t"`. If the state has no transitions (`it->begin() == it->end()`), print `"(no arcs)"`. Otherwise iterate its transitions: for every transition after the first, print `", "` first; then call `print_xfst_arc(file, data)` for that transition's data, print `" -> "`, and call `print_xfst_state(file, target_state)`. After the transitions (or the "(no arcs)" text), print `".\n"`, then increment `source_state`. Weights are never written. Returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-weight-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.write-weight-fn]
> Write a single weight to `FILE *file` via `fprintf(file, "%f", weight)`, i.e. the float formatted with C's default `%f` (six decimal places). Returns nothing. (A sibling overload writes to a `std::ostream` as `os << weight`.)

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.xfstize-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.xfstize-fn]
> Escape an xfst symbol string in place. Build `escaped_symbol` by scanning each char `pos` of `symbol`: if `pos == '%'` append the three chars `"%"` (i.e. a double-quote, percent, double-quote); if `pos == '"'` append `%"` (percent then double-quote); if `pos == '?'` append `"?"` (double-quote, question mark, double-quote); otherwise append the char unchanged. Assign `escaped_symbol` back into `symbol`. Returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.xfstize-symbol-fn]
> void

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.hfst-basic-transducer.xfstize-symbol-fn]
> Convert an internal HFST symbol to its xfst printed form, in place. First call `xfstize(symbol)` to escape `%`, `"`, and `?`. Then `replace_all` in this order: `@_EPSILON_SYMBOL_@`->"0", `@_UNKNOWN_SYMBOL_@`->"?", `@_IDENTITY_SYMBOL_@`->"?", and tab (`\t`)->`@_TAB_@`. Returns nothing.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-basic-transitions]
> typedef std::vector<hfst::implementations::HfstBasicTransition>

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-replacement]
> typedef std::pair<HfstState,

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-replacements]
> typedef std::vector<HfstReplacement> HfstReplacements

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-replacements-map]
> typedef std::map<HfstState, HfstReplacements> HfstReplacementsMap

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.hfst-state]
> typedef unsigned int HfstState

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.marker-pair-fn]
> HfstSymbolPair marker_pair(marker, marker)

> [spec:hfst:sem:hfst-basic-transducer.hfst.implementations.marker-pair-fn]
> Inside `insert_freely(const HfstBasicTransducer &graph)`: construct an `HfstSymbolPair marker_pair(marker, marker)` whose input and output sides are both the fresh marker symbol `marker` (the lexicographically greater of two distinct markers obtained from `get_marker(alphabet)`). This pair is then used to (1) `insert_freely(marker_pair, 0)` the marker:marker arc freely into this graph with weight 0, and (2) `substitute(marker_pair, graph)` to replace that marker arc with the contents of `graph`, after which `marker` is erased from the alphabet. The annotated line itself just builds the pair from `marker`.

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.state-map]
> typedef std::map<StatePair, HfstState> StateMap

> [spec:hfst:def:hfst-basic-transducer.hfst.implementations.state-pair]
> typedef std::pair<HfstState, HfstState> StatePair

> [spec:hfst:def:hfst-basic-transducer.main-fn]
> int

> [spec:hfst:sem:hfst-basic-transducer.main-fn]
> The `MAIN_TEST` unit-test entry point (only compiled when `MAIN_TEST` is defined). Print `"Unit tests for <__FILE__>:"` to `std::cout` followed by a newline, then immediately `return EXIT_SUCCESS`. All code after that return is unreachable/dead: it would have built a small `HfstBasicTransducer g1` (states 0..1, a/b/c/d arcs, state 1 final weight 0.5), built two substitution transducers `subst1`/`subst2`, applied `g1.substitute(subst_map, false)`, removed a `d:d` transition, written `g1` in AT&T format to stdout, and printed "ok". In practice the function does nothing but print the banner line and return success.

