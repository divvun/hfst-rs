# libhfst/src/implementations/HfstTransitionGraph.cc, libhfst/src/implementations/HfstTransitionGraph.h

> [spec:hfst:def:hfst-transition-graph.add-att-line-fn]
> bool add_att_line(char * line, const std::string & epsilon_symbol)

> [spec:hfst:sem:hfst-transition-graph.add-att-line-fn]
> Parses a single AT&T-format `line` and applies it to this graph; `epsilon_symbol`
> is the textual representation that should be mapped to internal epsilon.
> Returns whether the line was successfully parsed.
> Steps: sscanf up to five whitespace-separated fields a1..a5 into char[100]
> buffers; n is the number of fields parsed. Set weight=0; if n==2 (final
> state with weight) set weight=atof(a2); if n==5 (transition with weight) set
> weight=atof(a5).
> - If n==1 or n==2: final-state line. Call set_final_weight(atoi(a1), weight).
> - Else if n==4 or n==5: transition line. input_symbol=a3, output_symbol=a4.
>   In each of input and output, replace_all of the escapes: "@_SPACE_@"->" ",
>   "@0@"->"@_EPSILON_SYMBOL_@", "@_TAB_@"->"\t", "@_COLON_@"->":". Then if the
>   resulting symbol equals `epsilon_symbol`, replace it with
>   "@_EPSILON_SYMBOL_@". Build an HfstTransition<C>(target=atoi(a2),
>   input_symbol, output_symbol, weight) and add_transition(atoi(a1), tr).
> - Else (any other field count): return false.
> Return true. Mutates the graph via set_final_weight / add_transition.

> [spec:hfst:def:hfst-transition-graph.add-substitution-fn]
> void add_substitution(const substitution_data &sub)

> [spec:hfst:sem:hfst-transition-graph.add-substitution-fn]
> Splices a copy of a substituting graph into this graph, attached by epsilon
> transitions, as described by `sub` (a substitution_data with origin_state,
> target_state, weight, and substituting_graph pointer).
> Steps: call add_state() to create one fresh state `s` (the entry point of
> the spliced copy). Add an epsilon:epsilon transition with weight sub.weight
> from sub.origin_state to `s`. Set offset = s (used to renumber states of the
> copied graph so they do not collide with this graph's states).
> Then copy the substituting graph: iterate its states (source_state counter
> starting at 0) and for each transition build a new HfstTransition with
> target = original_target + offset, same input/output symbols and weight, and
> add_transition(source_state + offset, that transition).
> Finally, for each final state in the substituting graph's final_weight_map,
> add an epsilon:epsilon transition with weight = that state's final weight,
> from (final_state + offset) to sub.target_state. No return value; mutates
> this graph by adding states and transitions.

> [spec:hfst:def:hfst-transition-graph.add-to-results-fn]
> HFSTDLL static void add_to_results

> [spec:hfst:sem:hfst-transition-graph.add-to-results-fn]
> Static. Conditionally records `path_so_far` (an HfstTwoLevelPath, a (weight,
> symbol-pair-vector) pair) into the `results` set after adding a final
> weight. Steps: add `final_weight` to path_so_far.first (the running weight).
> If `max_weight` is NULL (no limit), insert path_so_far into results. Else if
> path_so_far.first is NOT greater than *max_weight, insert it; otherwise (the
> limit is exceeded) do nothing. Finally subtract `final_weight` back off
> path_so_far.first to restore the caller's running weight. Mutates `results`
> and temporarily mutates path_so_far.

> [spec:hfst:def:hfst-transition-graph.check-regexp-state-for-cycle-fn]
> HFSTDLL void check_regexp_state_for_cycle(HfstState s, const std::set<HfstState> & states_visited)

> [spec:hfst:sem:hfst-transition-graph.check-regexp-state-for-cycle-fn]
> If state `s` is already in `states_visited`, throws the C-string literal
> "error: loop detected inside compile-replace regular expression". Otherwise
> does nothing. Read-only on the graph.

> [spec:hfst:def:hfst-transition-graph.check-regexp-transition-end-fn]
> HFSTDLL bool check_regexp_transition_end(const HfstBasicTransition & tr, bool input_side)

> [spec:hfst:sem:hfst-transition-graph.check-regexp-transition-end-fn]
> Validates a transition encountered while scanning a compile-replace regexp
> path and returns whether it is the closing bracket "^]". `input_side`
> selects which side of the transition is inspected (input symbol if true,
> output symbol if false). Let istr/ostr be the transition's input/output
> symbols, and let "selected" be istr when input_side else ostr.
> Steps: if the selected symbol is epsilon, it is allowed (no-op). Else if the
> selected symbol is a special symbol (is_special_symbol true), throw "error:
> special symbol detected in compile-replace regular expression". Then: if the
> selected symbol equals "^[", throw "error: ^[ detected inside compile-replace
> regular expression". If the selected symbol equals "^]", return true.
> Otherwise return false. Throws are of C-string literals. Weights and flag
> diacritics are not handled.

> [spec:hfst:def:hfst-transition-graph.disjunct-fn]
> HfstState disjunct(const StringPairVector &spv,

> [spec:hfst:sem:hfst-transition-graph.disjunct-fn]
> Protected, recursive. Walks/extends the trie of this graph along the path
> `spv` starting from the transition pointed to by iterator `it`, at current
> state `s`. Returns the final state reached when the path is exhausted.
> Steps: base case: if `it == spv.end()` the whole path has been inserted;
> return `s`.
> Otherwise copy state_vector[s]'s transitions and search them for a
> transition whose input symbol equals it->first and output symbol equals
> it->second. If found, record its target as next_state. If not found, create
> a new state via add_state(), and add_transition(s, ...) for a transition
> with target=new state, symbols it->first/it->second, weight 0; next_state is
> that new state. Then advance `it` (it++) and tail-recurse:
> disjunct(spv, it, next_state). Note transitions are searched in a copy, but
> the new transition is added to the real state_vector[s]; weights are always
> 0 on these arcs (the public wrapper sets the path weight on the final
> state).

> [spec:hfst:def:hfst-transition-graph.extract-weight-fn]
> HFSTDLL static bool extract_weight(std::string & symbol, float & weight)

> [spec:hfst:sem:hfst-transition-graph.extract-weight-fn]
> Static. Given a prolog arc symbol string `symbol` that may have a trailing
> `, <weight>` after the symbol part, optionally extracts that weight into
> out-param `weight` and trims it off `symbol`. Returns whether the string is
> well-formed.
> Steps: find last_double_quote = position of the last `"` and last_space =
> position of the last space.
> - If no `"` exists at all (npos), return false.
> - If there is no space (npos): no weight present, leave symbol unchanged.
> - Else if last_double_quote > last_space: the last space is inside a symbol,
>   so no weight; leave unchanged.
> - Else if last_double_quote + 2 == last_space AND last_space < size-1 (the
>   +2 accounts for the comma between the closing quote and the space): parse
>   the substring after last_space as a float into `weight` via an
>   istringstream; if the read fails return false; otherwise resize `symbol`
>   to length last_space-1 to drop the comma, space and weight.
> - Otherwise (any other layout): return false.
> Return true. Note `weight` is only written in the explicit-weight branch.

> [spec:hfst:def:hfst-transition-graph.find-matches-fn]
> HFSTDLL static void find_matches

> [spec:hfst:sem:hfst-transition-graph.find-matches-fn]
> Static, recursive core of intersect. Computes the product (intersection) of
> `graph1` at `state1` and `graph2` at `state2`, building it into
> `intersection` at `state`. `state_map` maps state-pairs to product states;
> `agenda` records product states already handled. Precondition: both graphs
> are arc-sorted and deterministic.
> Steps: insert `state` into agenda. Get the transition vectors tr1 of
> graph1.state_vector[state1] and tr2 of graph2.state_vector[state2]. If either
> is empty, return (no matches possible). Use a merge-style scan exploiting the
> sortedness: keep start_search_from=0. For each transition1 in tr1 (index i),
> scan tr2 from index start_search_from. For each transition2 (index j),
> compare their transition data ignoring weight (less_than_ignore_weight):
> - if transition2 < transition1: not yet a match, continue scanning tr2;
> - if transition1 < transition2: no match for transition1 exists, set
>   start_search_from=j and break to the next tr1;
> - else (equal): a match. Call handle_match(graph1, transition1, graph2,
>   transition2, intersection, state, state_map) to get the product target.
>   If that target is not in agenda, recurse find_matches(graph1,
>   transition1.target, graph2, transition2.target, intersection, target,
>   state_map, agenda). Set start_search_from=j+1 and break to the next tr1.
> When all tr1 transitions are processed, return. No return value.

> [spec:hfst:def:hfst-transition-graph.find-matches-for-merge-fn]
> HFSTDLL static void find_matches_for_merge

> [spec:hfst:sem:hfst-transition-graph.find-matches-for-merge-fn]
> Static, recursive core of merge. Walks `graph` at `graph_state` against
> `merger` at `merger_state`, building the merged transducer into `result` at
> `result_state`. `state_map`, `agenda`, `list_symbols` (a map from a list
> symbol name to its set of member symbols), and `markers_added` are threaded
> through. Preconditions: both graphs arc-sorted and deterministic.
> Steps: insert `result_state` into agenda. Get graph_transitions =
> graph.state_vector[graph_state] and merger_transitions =
> merger.state_vector[merger_state]. If graph_transitions is empty, return.
> For each graph_transition (index i):
> - If is_list_symbol(graph_transition_data, list_symbols) is true: look up the
>   member symbol set for this list symbol. Scan every merger_transition; each
>   must have equal input and output symbols (else throw "find_matches_for_merge:
>   input and output symbols must be the same"). If the merger transition's
>   symbol is in the member set, mark list_match_found=true, call
>   handle_list_match(graph, graph_transition, merger, merger_transition,
>   result, result_state, state_map, markers_added) to get a target, and if
>   that target is not in agenda recurse using graph_transition.target and
>   merger_transition.target as the next states. After scanning, if any list
>   match was found, `continue` to the next graph_transition.
> - Otherwise (not a list symbol, or no list match found): call
>   handle_non_list_match(graph, graph_transition, merger, merger_state,
>   result, result_state, state_map) to get a target (note merger stays at
>   merger_state, it does not advance), and if target is not in agenda recurse
>   with graph_transition.target and the same merger_state.
> When all graph transitions are processed, return. No return value.

> [spec:hfst:def:hfst-transition-graph.find-regexp-paths-fn]
> HFSTDLL void find_regexp_paths

> [spec:hfst:sem:hfst-transition-graph.find-regexp-paths-fn]
> Recursive DFS (the 5-argument overload). Starting at state `s`, finds all
> sub-paths of the form [x:y]* "^]" (where x,y are not "^]"/"^[") and stores
> them into `full_paths`; `states_visited` tracks the current DFS stack to
> reject cycles, `path` is the symbol-pair vector built so far, `input_side`
> selects which symbol side is inspected.
> Steps: call check_regexp_state_for_cycle(s, states_visited) (throws if `s`
> already visited), then insert `s` into states_visited. Iterate `s`'s
> transitions. For each transition call check_regexp_transition_end(*it,
> input_side) (which throws on disallowed symbols):
> - If it returns true (closing "^]"): call check_regexp_state_for_cycle on the
>   transition's target (throws if it leads to an already-visited state), then
>   push the transition's (input,output) pair onto `path`, push
>   HfstReplacement(target_state, path) onto `full_paths`, and pop that pair
>   back off `path` (we do not descend further past the closing bracket).
> - Else: push the (input,output) pair onto `path`, recurse into
>   find_regexp_paths(target_state, states_visited, path, full_paths,
>   input_side), then pop the pair back off `path`.
> After all transitions, erase `s` from states_visited (backtrack). No return
> value; weights ignored.

> [spec:hfst:def:hfst-transition-graph.find-target-state-fn]
> static HfstState find_target_state

> [spec:hfst:sem:hfst-transition-graph.find-target-state-fn]
> Static. Maps a pair of source states (target1, target2) to a single state in
> the product graph `intersection`, creating it if necessary. Build
> StatePair(target1, target2) and look it up in `state_map`. If found, set
> out-param `was_new_state=false` and return the mapped state. Otherwise call
> intersection.add_state() to create a fresh state, record state_map[pair] =
> that state, set `was_new_state=true`, and return it.

> [spec:hfst:def:hfst-transition-graph.flag-purge-fn]
> HFSTDLL void flag_purge(const std::string & flag)

> [spec:hfst:sem:hfst-transition-graph.flag-purge-fn]
> Replaces flag-diacritic arcs with epsilon arcs and removes the flag(s) from
> the alphabet. `flag` is the feature name to purge; if empty, all flags are
> purged (per purge_symbol's semantics).
> Steps: (1) iterate every state and, by index, every transition; if
> purge_symbol(input_symbol, flag) OR purge_symbol(output_symbol, flag) is
> true, overwrite that transition in place with a new transition to the same
> target, weight preserved, but input and output both "@_EPSILON_SYMBOL_@".
> (2) iterate the alphabet collecting into a set every symbol for which
> purge_symbol(symbol, flag) is true, then call
> remove_symbols_from_alphabet on that set. No return value; mutates
> transitions and alphabet.

> [spec:hfst:def:hfst-transition-graph.get-flags-fn]
> HFSTDLL StringSet get_flags() const

> [spec:hfst:sem:hfst-transition-graph.get-flags-fn]
> Returns a StringSet of all flag-diacritic symbols in the alphabet. Iterates
> over `alphabet`; for each symbol, if FdOperation::is_diacritic(symbol) is
> true, inserts it into the result set. Returns that set. Read-only.

> [spec:hfst:def:hfst-transition-graph.get-prolog-arc-symbols-fn]
> HFSTDLL static bool get_prolog_arc_symbols

> [spec:hfst:sem:hfst-transition-graph.get-prolog-arc-symbols-fn]
> Static. Parses a prolog arc symbol string `str` of the form `"foo"` (single
> symbol) or `"foo":"bar"` (input:output pair), writing the decoded input and
> output symbols into out-params `isymbol`/`osymbol`. Returns whether parsing
> succeeded.
> Steps: compute the positions of all non-escaped `"` characters via
> get_positions_of_unescaped_char(str, '"', '\\').
> - If there are exactly 2 quotes: require the first quote at index 0 and the
>   second at the last index of the string (else return false for extra
>   characters outside quotes).
> - If there are exactly 4 quotes: require the first at index 0 and the last
>   at str.length()-1; require the gap between the 2nd and 3rd quote to be
>   exactly 2; require the single character between them to be `:` (else
>   return false in each failing case).
> - Any other quote count: return false.
> Then extract symbols: for the 2-quote case take the substring between the
> two quotes, run it through deprologize_symbol; if the result equals
> "@_UNKNOWN_SYMBOL_@" map it to "@_IDENTITY_SYMBOL_@"; set both isymbol and
> osymbol to that value. For the 4-quote case take the substring inside the
> first quote pair and the substring inside the second quote pair, run each
> through deprologize_symbol, assigning to isymbol and osymbol respectively.
> Return true.

> [spec:hfst:def:hfst-transition-graph.handle-list-match-fn]
> HFSTDLL static HfstState handle_list_match(const HfstTransitionGraph & graph, const HfstTransition <C> & graph_transition,

> [spec:hfst:sem:hfst-transition-graph.handle-list-match-fn]
> Static helper used by find_matches_for_merge for a list-symbol match. Given
> the matched graph_transition and merger_transition at result state
> `result_state`, copies the match into `result` (inserting a marker arc) and
> returns the product target state. Steps: graph_target=graph_transition.target,
> merger_target=merger_transition.target. Call find_target_state(graph_target,
> merger_target, state_map, result, was_new_state) -> retval. Compute
> transition_weight = graph_transition.weight + merger_transition.weight. Create
> a fresh intermediate state `extra_state` via result.add_state(). Add a
> transition from result_state to extra_state with weight 0 whose input symbol
> is "@" + graph_transition.input_symbol + "@" and output symbol is "@" +
> graph_transition.output_symbol + "@" (a marker), and insert that input-side
> marker string into `markers_added`. Then add a transition from extra_state to
> retval using merger_transition's input and output symbols and the summed
> transition_weight. If a new state was created and both graph and merger are
> final at graph_target/merger_target, set retval's final weight to the sum of
> their final weights. Return retval.

> [spec:hfst:def:hfst-transition-graph.handle-match-fn]
> HFSTDLL static HfstState handle_match(const HfstTransitionGraph & graph1, const HfstTransition <C> & tr1,

> [spec:hfst:sem:hfst-transition-graph.handle-match-fn]
> Static helper used by find_matches (intersection). Given matching
> transitions tr1 (from graph1) and tr2 (from graph2) at intersection state
> `state`, copies the matched transition into `intersection` and returns its
> target state. Steps: take target1=tr1.target, target2=tr2.target. Call
> find_target_state(target1, target2, state_map, intersection, was_new_state)
> to obtain the product target `retval`. Compute transition_weight =
> tr1.weight + tr2.weight. Add to `intersection` a transition from `state` to
> `retval` with tr1's input and output symbols and that summed weight. If a new
> state was created AND both graph1.is_final_state(target1) and
> graph2.is_final_state(target2) hold, set retval's final weight in
> intersection to the sum of the two final weights. Return retval.

> [spec:hfst:def:hfst-transition-graph.handle-non-list-match-fn]
> HFSTDLL static HfstState handle_non_list_match(const HfstTransitionGraph & graph, const HfstTransition <C> & graph_transition,

> [spec:hfst:sem:hfst-transition-graph.handle-non-list-match-fn]
> Static helper used by find_matches_for_merge for a non-list (plain copy)
> match. Given a graph_transition and a merger target state `merger_target`,
> copies the graph transition into `result` at `result_state` and returns the
> product target. Steps: graph_target=graph_transition.target. Call
> find_target_state(graph_target, merger_target, state_map, result,
> was_new_state) -> retval. Add a transition from result_state to retval using
> graph_transition's input symbol, output symbol and weight (unchanged). If a
> new state was created and both graph.is_final_state(graph_target) and
> merger.is_final_state(merger_target) hold, set retval's final weight to the
> sum of the two final weights. Return retval.

> [spec:hfst:def:hfst-transition-graph.has-negative-epsilon-cycles-fn]
> bool has_negative_epsilon_cycles

> [spec:hfst:sem:hfst-transition-graph.has-negative-epsilon-cycles-fn]
> Recursive DFS (this overload takes `state`, accumulated `total_weight`, and
> a `state_weights` map of states currently on the recursion stack to the
> accumulated weight when first entered) that detects negative-weight cycles
> consisting only of epsilon:epsilon transitions.
> Steps: look up `state` in state_weights. If present, a cycle is detected:
> return true if (total_weight - stored_weight) < 0 (negative-weight cycle),
> else return false. Otherwise record state_weights[state] = total_weight.
> Then iterate this state's transitions; for each transition whose input AND
> output symbols are both epsilon, recurse into has_negative_epsilon_cycles(
> target_state, total_weight + transition weight, state_weights); if that
> returns true, return true. After exploring, erase `state` from
> state_weights (backtrack) and return false. The parameterless overload (just
> below) drives this over all states.

> [spec:hfst:def:hfst-transition-graph.hfst-basic-transducer]
> typedef HfstTransitionGraph <HfstTropicalTransducerTransitionData>

> [spec:hfst:def:hfst-transition-graph.hfst-replacements-map-find-replacements-fn]
> HFSTDLL HfstReplacementsMap find_replacements(bool input_side)

> [spec:hfst:sem:hfst-transition-graph.hfst-replacements-map-find-replacements-fn]
> Finds, for every state, all "^[" [x:y]* "^]" sub-paths and returns them as
> an HfstReplacementsMap (state -> list of (end_state, transition-pair-vector)).
> `input_side` selects which symbol side is matched. Steps: create empty
> `replacements` map and a state counter starting at 0. Iterate over all states
> (begin()..end()); for each, create an empty `full_paths`, call the
> find_regexp_paths(state, full_paths, input_side) overload (which seeds the
> search from each "^[" transition out of this state); if full_paths is
> non-empty, store replacements[state] = full_paths. Increment the counter each
> iteration. Return the map. Weights ignored.

> [spec:hfst:def:hfst-transition-graph.hfst-transition-graph-fn]
> HFSTDLL static HfstTransitionGraph merge

> [spec:hfst:sem:hfst-transition-graph.hfst-transition-graph-fn]
> Static `merge`. Merges `graph` with `merger` using the list-symbol
> substitution scheme, returning a new result graph; `list_symbols` maps list
> symbol names to member sets and `markers_added` collects the marker strings
> emitted. Steps: create empty `result`, empty `state_map`, empty `agenda`.
> Call graph.sort_arcs() and merger.sort_arcs(). Seed state_map[StatePair(0,0)]
> = 0 (initial states map to result state 0). If both graph and merger are
> final at state 0, set result's final weight at 0 to the sum of their initial
> final weights. Then call find_matches_for_merge(graph, 0, merger, 0, result,
> 0, state_map, agenda, list_symbols, markers_added) inside a try block; if it
> throws a `const char *` message, rethrow it as a
> TransducersAreNotAutomataException carrying that message. Return `result`.

> [spec:hfst:def:hfst-transition-graph.hfst-transition-graph-read-in-att-format-fn]
> HFSTDLL static HfstTransitionGraph read_in_att_format

> [spec:hfst:sem:hfst-transition-graph.hfst-transition-graph-read-in-att-format-fn]
> Static. Reads one AT&T-format transducer and returns it as a new graph.
> Reads from FILE `file` if it is non-NULL, otherwise from istream `is`.
> `epsilon_symbol` is forwarded to add_att_line; `linecount` is incremented
> per line read.
> Steps: first check end-of-stream (is.eof() when file==NULL, else feof(file));
> if already at end throw EndOfStreamException. Create empty `retval`. Loop
> reading up to 254 chars per line into a char[255] buffer (is.getline when
> file==NULL, else fgets); break out of the loop when the read indicates EOF
> (getline().eof() true / fgets returns NULL). Increment linecount each
> iteration.
> Edge cases inside the loop:
> - If the line is empty (line[0]=='\0', or "\n", or windows "\r\n"): this
>   signifies an empty transducer; consume one more character to reach EOF
>   (is.get() or fgetc) and break.
> - If the line starts with '-' (the "--" transducer separator): return retval
>   immediately.
> Otherwise call retval.add_att_line(line, epsilon_symbol); if it returns
> false, throw NotValidAttFormatException with the offending line as message.
> After the loop, return retval. The istream-only and FILE-only sibling
> overloads forward here with a NULL/std::cin dummy.

> [spec:hfst:def:hfst-transition-graph.hfst-transition-graph-read-in-prolog-format-fn]
> HFSTDLL static HfstTransitionGraph read_in_prolog_format

> [spec:hfst:sem:hfst-transition-graph.hfst-transition-graph-read-in-prolog-format-fn]
> Static. Reads one prolog-format transducer from input (either istream `is`
> or FILE `file`; whichever read source get_stripped_line uses) and returns it
> as a new HfstTransitionGraph; `linecount` is passed through to
> get_stripped_line and incremented as lines are read.
> Steps: create empty `retval`. Loop reading stripped lines via
> get_stripped_line(is, file, linecount); if it throws EndOfStreamException,
> throw NotValidPrologFormatException. Skip lines whose first character is `#`
> (comments); stop at the first non-comment line. Pass that line to
> parse_prolog_network_line(line, retval); if it returns false, throw
> NotValidPrologFormatException with a message "first line not valid prolog: "
> + line.
> Then loop reading further stripped lines: if a read returns an empty line
> (the prolog separator) return retval; if the read throws
> EndOfStreamException return retval. Otherwise try to parse the line as an
> arc, a final-state line, or a symbol line by calling parse_prolog_arc_line,
> parse_prolog_final_line, and parse_prolog_symbol_line in that order (short
> circuit); if none succeed, throw NotValidPrologFormatException with message
> "line not valid prolog: " + line. The trailing throw after the loop is
> unreachable. The two sibling overloads (istream-only, FILE-only) forward to
> this with a NULL/std::cin dummy for the unused source.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-basic-states]
> typedef std::vector<std::vector<hfst::implementations::HfstBasicTransition> > HfstBasicStates

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-replacement]
> typedef std::pair<HfstState, std::vector<std::pair<std::string, std::string> > > HfstReplacement

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-replacements]
> typedef std::vector<HfstReplacement> HfstReplacements

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-replacements-map]
> typedef std::map<HfstState, HfstReplacements > HfstReplacementsMap

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-state]
> typedef unsigned int HfstState

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph]
> class HfstTransitionGraph {
>   HfstStates state_vector;
>   static const HfstState INITIAL_STATE = 0;
>   FinalWeightMap final_weight_map;
>   HfstTransitionGraphAlphabet alphabet;
>   std::string name;
>   HFSTDLL HfstTransitionGraphAlphabet;
>   HFSTDLL StringPairSet;
>   HFSTDLL typename;
>   HFSTDLL const_iterator;
>   HFSTDLL const_iterator;
>   HFSTDLL static std::vector<unsigned int> get_positions_of_unescaped_char (const std::string & str, char c, char esc) { std::vector<unsigned int> retval;
>   i < str.length(); i++) { if (str[i] == c) { if (i == 0) retval.push_back(i);
> }

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.add-state-fn]
> HFSTDLL HfstState add_state(HfstState s)

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.add-state-fn]
> Ensures that state number `s` exists. While the size of `state_vector` is
> less than or equal to `s`, pushes a fresh empty HfstTransitions vector onto
> `state_vector`. This creates every state from the current top up to and
> including `s` (with empty transition lists) if they did not already exist;
> if `s` already exists nothing is added. Returns `s`.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.add-symbol-to-alphabet-fn]
> HFSTDLL void add_symbol_to_alphabet(const HfstSymbol &symbol)

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.add-symbol-to-alphabet-fn]
> Inserts the given symbol into the alphabet set. Since `alphabet` is a set,
> a duplicate insert is a no-op. No return value.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.add-symbols-to-alphabet-fn]
> HFSTDLL void add_symbols_to_alphabet(const HfstSymbolPairSet &symbols)

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.add-symbols-to-alphabet-fn]
> Overload taking an HfstSymbolPairSet. Iterates over each symbol pair in the
> set and inserts both the first (input) and second (output) symbol of each
> pair into the alphabet set. (The sibling overload taking an HfstSymbolSet
> inserts each symbol once.) No return value.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.add-transition-fn]
> HFSTDLL void add_transition(HfstState s, const HfstTransition<C> & transition,

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.add-transition-fn]
> Adds transition `transition` to state `s`. Reads the transition's data.
> Calls add_state(s) and add_state(transition.get_target_state()) to ensure
> both the source and target states exist (creating intermediate states as
> needed). If `add_symbols_to_alphabet` (default true), inserts the
> transition data's input symbol and output symbol into `alphabet`. Then
> appends `transition` to the transitions vector of state `s`
> (state_vector[s]). No return value.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.begin-fn]
> HFSTDLL iterator begin()

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.begin-fn]
> Returns an iterator to the first element of `state_vector` (i.e. the
> transitions of the initial/lowest-numbered state). A const overload returns
> a const_iterator. Read-only.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.check-alphabet-fn]
> bool check_alphabet()

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.check-alphabet-fn]
> Verifies that every symbol appearing in any transition is present in the
> alphabet set. Iterates over all states (begin()..end()); for each state
> iterates over its transitions; for each transition reads its transition
> data and looks up both the input symbol and the output symbol in `alphabet`.
> If either lookup fails (symbol not found in the alphabet), returns false
> immediately. If all symbols are found, returns true. Read-only; mutates
> nothing.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.const-iterator]
> typedef typename HfstStates::const_iterator const_iterator

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.end-fn]
> HFSTDLL iterator end()

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.end-fn]
> Returns an iterator to one past the last element of `state_vector` (the
> last state + 1). A const overload returns a const_iterator. Read-only.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.final-weight-map]
> typedef std::map<HfstState,typename C::WeightType> FinalWeightMap

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.for-fn]
> for (size_t i=0

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.for-fn]
> The body of get_positions_of_unescaped_char(str, c, esc): the loop that
> computes the positions of every occurrence of character `c` in `str` that is
> NOT escaped by `esc`. Iterate i from 0 to str.length()-1. For each i where
> str[i]==c: if i==0 push i onto the result vector; else if the preceding
> character str[i-1]==esc skip it (escaped); else push i. Returns the vector of
> unescaped positions (this annotated block is the loop driving that
> computation).

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.get-final-weight-fn]
> C::WeightType get_final_weight(HfstState s) const

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.get-final-weight-fn]
> Returns the final weight of state `s`. If `s > get_max_state()` (state does
> not exist), throw StateIndexOutOfBoundsException. Otherwise look up `s` in
> final_weight_map; if found, return its mapped weight. If not found (state is
> not final), throw StateIsNotFinalException. Read-only.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.get-max-state-fn]
> HFSTDLL HfstState get_max_state() const

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.get-max-state-fn]
> Returns the biggest state number in use: `state_vector.size() - 1`. Read-only.
> (Since the graph always has at least one state, size is at least 1.)

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.get-symbol-number-fn]
> unsigned int get_symbol_number

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.get-symbol-number-fn]
> Returns the numeric id of `symbol` by delegating to the transition-data
> class's `C::get_number(symbol)`. Read-only.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.get-transition-pairs-fn]
> get_transition_pairs() const

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.get-transition-pairs-fn]
> Returns a StringPairSet of all (input_symbol, output_symbol) pairs that
> appear in any transition. Iterate over all states (begin()..end()); for each
> state iterate its transitions; for each transition read its transition data
> and insert StringPair(input_symbol, output_symbol) into the result set
> (duplicates collapse since it is a set). Return the set. Read-only.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-number]
> typedef unsigned int HfstNumber

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-number-pair]
> typedef std::pair<HfstNumber, HfstNumber> HfstNumberPair

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-number-pair-substitutions]
> typedef std::map<HfstNumberPair, HfstNumberPair> HfstNumberPairSubstitutions

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-number-vector]
> typedef std::vector<HfstNumber> HfstNumberVector

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-states]
> typedef std::vector<HfstTransitions> HfstStates

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-symbol]
> typedef typename C::SymbolType HfstSymbol

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-symbol-pair]
> typedef std::pair<HfstSymbol, HfstSymbol>

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-symbol-pair-set]
> typedef std::set<HfstSymbolPair> HfstSymbolPairSet

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-symbol-pair-vector]
> typedef std::vector<HfstSymbolPair> HfstSymbolPairVector

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-symbol-set]
> typedef std::set<HfstSymbol> HfstSymbolSet

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-transition-graph-alphabet]
> typedef std::set<HfstSymbol> HfstTransitionGraphAlphabet

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-transition-graph-fn]
> HFSTDLL HfstTransitionGraph(const hfst::HfstTransducer &transducer)

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-transition-graph-fn]
> Constructor that builds an HfstTransitionGraph equivalent to an
> hfst::HfstTransducer. Calls
> ConversionFunctions::hfst_transducer_to_hfst_basic_transducer(transducer),
> which returns a heap-allocated HfstTransitionGraph<HfstTropicalTransducer-
> TransitionData> pointer `fsm`. Copies fsm->state_vector, fsm->final_weight_map
> and fsm->alphabet into this object's corresponding members, then `delete fsm`.
> The `name` member is left default-constructed.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.hfst-transitions]
> typedef std::vector<HfstTransition<C> > HfstTransitions

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.if-fn]
> else if (str[i-1] == esc)

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.if-fn]
> The escaped-character branch inside get_positions_of_unescaped_char's loop:
> when str[i]==c and i!=0, this `else if (str[i-1] == esc)` tests whether the
> immediately preceding character is the escape char `esc`. If so, the body is
> empty (`;`) — the position is skipped (the occurrence is escaped, so it is not
> recorded). If not, control falls to the `else` which records the position.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.initialize-alphabet-fn]
> void initialize_alphabet(HfstTransitionGraphAlphabet &alpha)

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.initialize-alphabet-fn]
> Inserts the three always-present special symbols into the given alphabet set
> `alpha`: C::get_epsilon(), C::get_unknown(), and C::get_identity(). No return
> value; mutates `alpha`.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.initialize-state-vector-fn]
> void initialize_state_vector

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.initialize-state-vector-fn]
> Optimization helper. Calls state_vector.reserve(number_of_states) to reserve
> capacity for that many states without changing the number of existing states.
> No return value.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.initialize-transition-vector-fn]
> void initialize_transition_vector

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.initialize-transition-vector-fn]
> Optimization helper. Calls add_state(state_number) to ensure the state exists
> (creating it and any lower-numbered states if needed), then calls
> state_vector[state_number].reserve(number_of_transitions) to reserve capacity
> for that many transitions on that state. No return value.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.is-final-state-fn]
> HFSTDLL bool is_final_state(HfstState s) const

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.is-final-state-fn]
> Returns whether state `s` is final: true iff `s` is a key in
> final_weight_map (final_weight_map.find(s) != end()). Does not validate that
> `s` exists as a state. Read-only.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.iterator]
> typedef typename HfstStates::iterator iterator

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.parse-prolog-network-line-fn]
> HFSTDLL static bool parse_prolog_network_line(const std::string & line, HfstTransitionGraph & graph)

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.parse-prolog-network-line-fn]
> Static. Parses a prolog `network(NAME).` header line and sets `graph.name`.
> Returns whether parsing succeeded. Steps: sscanf(line, "network(%s", namearr)
> into a char[100] buffer; if it does not read exactly 1 field, return false.
> Build a std::string namestr from namearr, then call
> strip_ending_parenthesis_and_comma(namestr) to remove the trailing ")." (it
> actually strips the last two characters after checking they are ")" and ".");
> if that returns false, return false. Otherwise set graph.name = namestr and
> return true. Mutates graph.name on success.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.print-alphabet-fn]
> HFSTDLL void print_alphabet() const

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.print-alphabet-fn]
> Prints the alphabet to std::cerr. Iterate over `alphabet` in set order; for
> each symbol after the first, print ", " before it; print each symbol. After
> all symbols, print std::endl. No return value; side effect is stderr output.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.print-prolog-arc-symbols-fn]
> HFSTDLL static void print_prolog_arc_symbols(FILE * file, C data)

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.print-prolog-arc-symbols-fn]
> Static. Prints a prolog-format arc symbol (FILE* overload) for transition
> data `data`. Steps: symbol = prologize_symbol(data.get_input_symbol());
> fprintf(file, "\"%s\"", symbol) printing the input symbol in quotes. Then, if
> the input symbol differs from the output symbol OR the input symbol equals
> "@_UNKNOWN_SYMBOL_@", also print the output: symbol =
> prologize_symbol(data.get_output_symbol()); fprintf(file, ":\"%s\"", symbol).
> No return value. (A sibling ostream overload has the same structure.)

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.print-xfst-arc-fn]
> void print_xfst_arc(FILE * file, C data)

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.print-xfst-arc-fn]
> Prints an xfst-format arc (FILE* overload) for transition data `data`. Let
> in=data.get_input_symbol(), out=data.get_output_symbol(). Steps: if in != out
> print "<". Set s=in, call xfstize_symbol(s) (which replaces spaces, epsilon,
> unknown/identity, and tabs with their xfst escapes), fprintf "%s" of s. If
> (in != out) OR (out == "@_UNKNOWN_SYMBOL_@"), set s=out, xfstize_symbol(s),
> fprintf ":%s". Finally if in != out print ">". No return value. (A sibling
> ostream overload has the same structure but writes via `os`.)

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.print-xfst-state-fn]
> void print_xfst_state(std::ostream & os, HfstState state)

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.print-xfst-state-fn]
> Prints an xfst-format state label (ostream overload) for state `state` to
> `os`. Steps: if state == INITIAL_STATE (0), print "S". If is_final_state(state),
> print "f". Then print "s" followed by the state number. So state 0 final would
> print "Sfs0". No return value. (A sibling FILE* overload behaves identically
> using fprintf.)

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.prune-alphabet-after-substitution-fn]
> HFSTDLL void prune_alphabet_after_substitution(const std::set<unsigned int> &symbols)

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.prune-alphabet-after-substitution-fn]
> Removes from the alphabet those symbol numbers in `symbols` that no longer
> occur in any transition. Steps: if symbols is empty, return immediately.
> Build a bool vector symbols_found sized C::get_max_number()+1, all false. Walk
> every state and every transition; for each, mark
> symbols_found.at(data.get_input_number()) and symbols_found.at(
> data.get_output_number()) true. Then for each symbol-number in `symbols`, if
> symbols_found.at(that number) is false, erase C::get_symbol(number) from the
> alphabet. No return value; mutates alphabet.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.prune-alphabet-fn]
> HFSTDLL void prune_alphabet(bool force=true)

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.prune-alphabet-fn]
> Removes from the alphabet every symbol that does not occur in any transition,
> always keeping epsilon/unknown/identity. `force` (default true) controls
> behaviour when unknowns/identities are present. Steps: symbols_found =
> symbols_used() (the set of symbols actually on transitions). Compute
> unknowns_or_identities_used = whether symbols_found contains
> "@_UNKNOWN_SYMBOL_@" or "@_IDENTITY_SYMBOL_@". If !force AND
> unknowns_or_identities_used, return without changing anything. Otherwise
> insert "@_EPSILON_SYMBOL_@", "@_UNKNOWN_SYMBOL_@", "@_IDENTITY_SYMBOL_@" into
> symbols_found. Build symbols_not_found = every symbol in `alphabet` that is
> not in symbols_found, then erase each of those from `alphabet`. No return
> value; mutates alphabet.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.push-back-fn]
> else

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.push-back-fn]
> The unescaped-occurrence branch inside get_positions_of_unescaped_char's loop:
> the final `else` reached when str[i]==c, i!=0, and the preceding character is
> NOT the escape char. It records this position by retval.push_back(i) (the
> occurrence of `c` is genuine/unescaped).

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.remove-symbol-from-alphabet-fn]
> HFSTDLL void remove_symbol_from_alphabet(const HfstSymbol &symbol)

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.remove-symbol-from-alphabet-fn]
> Erases `symbol` from the alphabet set (alphabet.erase(symbol)). If the symbol
> is not present this is a no-op. No return value.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.remove-symbols-from-alphabet-fn]
> HFSTDLL void remove_symbols_from_alphabet(const HfstSymbolSet &symbols)

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.remove-symbols-from-alphabet-fn]
> Iterates over each symbol in the given HfstSymbolSet `symbols` and erases it
> from the alphabet set (alphabet.erase). No return value.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.remove-transition-fn]
> HFSTDLL void remove_transition(HfstState s, const HfstTransition<C> & transition,

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.remove-transition-fn]
> Removes from state `s` every transition matching `transition` (matching by
> input symbol, output symbol AND target state; weight ignored).
> `remove_symbols_from_alphabet` (default false) controls alphabet cleanup.
> Steps: if state `s` does not exist (state_vector.size() <= s), return. Let
> transitions = state_vector[s]. Scan its transitions and push every iterator
> matching all three fields onto a stack `elements_to_remove`. Then pop the
> stack (reverse order, so iterators stay valid) and erase each from
> state_vector[s]. If remove_symbols_from_alphabet is true, recompute alpha =
> symbols_used(); if the removed transition's input symbol is no longer in
> alpha, remove_symbol_from_alphabet(it); same for the output symbol. No return
> value; mutates transitions and possibly the alphabet.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.retval-fn]
> std::string retval(symbol)

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.retval-fn]
> The general-case tail of prologize_symbol (the path taken when `symbol` is not
> one of the special symbols "0", "?", epsilon, unknown, identity). Copy the
> symbol into a string `retval`, then replace_all backslash "\\" with double
> backslash "\\\\", and replace_all double-quote "\"" with backslash-quote
> "\\\"" (i.e. escape backslashes and double quotes). Return retval.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.set-final-weight-fn]
> HFSTDLL void set_final_weight(HfstState s,

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.set-final-weight-fn]
> Sets the final weight of state `s` to `weight`. Calls add_state(s) to ensure
> `s` (and any lower-numbered states) exist, then sets final_weight_map[s] =
> weight (creating or overwriting the entry). No return value; mutates
> final_weight_map and possibly state_vector.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.states-and-transitions-fn]
> HfstBasicStates states_and_transitions() const

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.states-and-transitions-fn]
> Returns a copy of `state_vector` (type HfstBasicStates), i.e. the full vector
> of states with their transition lists. Read-only.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.states-fn]
> std::vector<HfstState> states() const

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.states-fn]
> Returns a vector listing every state number 0..get_max_state(). Allocate a
> vector retval of size get_max_state()+1, then set retval[i]=i for each i in
> 0..get_max_state(). Effectively returns [0, 1, ..., max_state]. Read-only.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.strip-ending-parenthesis-and-comma-fn]
> HFSTDLL static bool strip_ending_parenthesis_and_comma(std::string & str)

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.strip-ending-parenthesis-and-comma-fn]
> Static. If `str` ends with ")." removes that final ")." and returns true;
> otherwise returns false. Steps: if str.size() < 3, return false. If the
> second-to-last char != ')' OR the last char != '.', return false. Otherwise
> str.erase(str.length()-2) (drops the last two characters) and return true.
> Mutates `str` only on success.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.strip-quotes-from-both-sides-fn]
> HFSTDLL static bool strip_quotes_from_both_sides(std::string & str)

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.strip-quotes-from-both-sides-fn]
> Static. If `str` is of the form `"..."` (starts and ends with a double quote),
> removes both quotes in place and returns true; otherwise returns false. Steps:
> if str.size() < 3, return false. If str[0] != '"' OR the last char != '"',
> return false. Otherwise str.erase(0,1) then str.erase(str.length()-1, 1) and
> return true. Mutates `str` only on success.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.swap-state-numbers-fn]
> void swap_state_numbers(HfstState s1, HfstState s2)

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.swap-state-numbers-fn]
> Renumbers state s1 to s2 and vice versa throughout the graph. Steps: swap the
> two transition lists state_vector[s1] and state_vector[s2] (via a copy). Then
> walk every state and, by index, every transition; for each transition compute
> new_target from its current target: if target==s1 set new_target=s2; if
> target==s2 set new_target=s1 (note these are independent if-checks, but s1!=s2
> so at most one applies); if new_target differs from the old target, replace
> that transition in place with a new HfstTransition having new_target and the
> same input/output symbols and weight. Finally swap the final weights: look up
> s1 and s2 in final_weight_map. If both are final, swap their weights. The code
> then additionally: if s1 was final, erase s1's entry and set
> final_weight_map[s2] to s1's old weight; if s2 was final, erase s2's entry and
> set final_weight_map[s1] to s2's old weight (these use the pre-swap iterators'
> captured weights, achieving the swap/move for the cases where only one was
> final). No return value; mutates state_vector and final_weight_map.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.symbols-used-fn]
> symbols_used()

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.symbols-used-fn]
> Returns the set (HfstTransitionGraphAlphabet) of all symbols that actually
> appear in transitions. Iterate over all states and all their transitions; for
> each transition read its data and insert both the input symbol and the output
> symbol into the result set. Return the set. Does not add the special
> epsilon/unknown/identity symbols unless they occur on arcs. Read-only.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.write-in-prolog-format-fn]
> HFSTDLL void write_in_prolog_format(FILE * file, const std::string & name,

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.write-in-prolog-format-fn]
> FILE* overload. Writes the graph in prolog format to `file` under network
> name `name`; `write_weights` (default true) controls weight printing. Steps:
> if `name` contains a comma, throw HfstException with message "no commas allowed
> in the name of prolog networks". Print `network(NAME).\n` (identifier =
> name.c_str()). Then print orphan symbols: compute symbols_used_ = symbols_used()
> and call initialize_alphabet(symbols_used_) to add the special symbols (so
> they are excluded); for each symbol in `alphabet` not present in symbols_used_,
> print `symbol(NAME, "<prologized symbol>").\n`. Then print arcs: iterate all
> states with a source_state counter from 0; for each transition print
> `arc(NAME, <source>, <target>, ` then print_prolog_arc_symbols(file, data),
> and if write_weights print ", " followed by write_weight(file, weight); close
> with ").\n". Then print final states: for each (state, weight) in
> final_weight_map print `final(NAME, <state>` then, if write_weights, ", " and
> write_weight; close with ").\n". No return value; writes to `file`. (A sibling
> ostream overload has identical structure.)

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.write-in-xfst-format-fn]
> HFSTDLL void write_in_xfst_format(std::ostream &os, bool write_weights=true)

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.write-in-xfst-format-fn]
> ostream overload. Writes the graph in xfst text format to `os`.
> `write_weights` is accepted but ignored (cast to void). Iterates over all
> states with a `source_state` counter starting at 0. For each state: call
> print_xfst_state(os, source_state), then print ":\t". If the state has no
> transitions (begin()==end()), print "(no arcs)". Otherwise for each
> transition, print ", " before all but the first, then call
> print_xfst_arc(os, data) for the transition data, print " -> ", and call
> print_xfst_state(os, target_state). After the arcs, print "." followed by
> std::endl, and increment source_state. No return value. (There is a sibling
> FILE* overload with the same structure.)

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.write-weight-fn]
> static void write_weight(FILE * file, float weight)

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.write-weight-fn]
> Static (FILE* overload). Writes `weight` to `file` via fprintf(file, "%f",
> weight) (the printf "%f" default formats with 6 decimal places). No special
> handling of zero. No return value. (A sibling ostream overload writes
> `os << weight`.)

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.xfstize-fn]
> static void xfstize(std::string & symbol)

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.xfstize-fn]
> Static. Escapes the special xfst metacharacters in `symbol` in place. Build a
> new string by scanning each character of `symbol`: a '%' becomes the 3-char
> sequence `"%"` (double-quote, percent, double-quote); a '"' becomes `%"`
> (percent, double-quote); a '?' becomes `"?"` (double-quote, question,
> double-quote); any other character is appended unchanged. Assign the built
> string back to `symbol`. No return value; mutates `symbol`.

> [spec:hfst:def:hfst-transition-graph.hfst.implementations.hfst-transition-graph.xfstize-symbol-fn]
> static void xfstize_symbol(std::string & symbol)

> [spec:hfst:sem:hfst-transition-graph.hfst.implementations.hfst-transition-graph.xfstize-symbol-fn]
> Static. Converts an internal symbol to its xfst text representation in place.
> First call xfstize(symbol) (escapes %, ", ?). Then replace_all in order:
> "@_EPSILON_SYMBOL_@" -> "0", "@_UNKNOWN_SYMBOL_@" -> "?",
> "@_IDENTITY_SYMBOL_@" -> "?", and "\t" -> "@_TAB_@". No return value; mutates
> `symbol`.

> [spec:hfst:def:hfst-transition-graph.insert-transducer-fn]
> HFSTDLL void insert_transducer(HfstState state1, HfstState state2, const HfstTransitionGraph & graph)

> [spec:hfst:sem:hfst-transition-graph.insert-transducer-fn]
> Splices a copy of `graph` between this graph's states `state1` and `state2`
> using epsilon transitions. Steps: offset = add_state() (the renumbering base /
> entry point of the copy). Copy graph's transitions: iterate its states with a
> source_state counter from 0; for each transition build a new HfstTransition
> with target = original target + offset, same input/output symbols and weight,
> and add_transition(source_state + offset, ...). Then add the exit epsilons: for
> each (final_state, final_weight) in graph.final_weight_map, add_transition(
> final_state + offset, epsilon:epsilon transition to `state2` with weight =
> final_weight). Finally add the entry epsilon: add_transition(state1,
> epsilon:epsilon transition to `offset` with weight 0). The copied graph's
> initial and final states thus become plain non-final states inside this graph.
> No return value; mutates this graph. (Alphabet is not copied/harmonized.)

> [spec:hfst:def:hfst-transition-graph.is-infinitely-ambiguous-fn]
> HFSTDLL bool is_infinitely_ambiguous

> [spec:hfst:sem:hfst-transition-graph.is-infinitely-ambiguous-fn]
> Recursive DFS (the 3-arg overload) detecting an epsilon/flag-diacritic cycle
> reachable from `state`. `epsilon_path_states` is the set of states on the
> current epsilon-only DFS stack; `states_handled` is a per-state vector marking
> states already fully explored. Steps: if states_handled[state] != 0 (already
> handled), return false. Iterate `state`'s transitions; for each transition
> whose input symbol is_epsilon OR FdOperation::is_diacritic (flag diacritics
> treated as epsilons): insert `state` into epsilon_path_states; if the
> transition's target is already in epsilon_path_states, return true (cycle
> found); else recurse is_infinitely_ambiguous(target, epsilon_path_states,
> states_handled) and return true if it returns true; then erase `state` from
> epsilon_path_states (backtrack). After all transitions, set
> states_handled[state]=1 and return false. The parameterless overload (just
> below) creates an empty epsilon_path_states and a states_handled vector of
> size max_state+1 all zero, and calls this for every state, returning true if
> any does.

> [spec:hfst:def:hfst-transition-graph.is-list-symbol-fn]
> HFSTDLL static bool is_list_symbol(const C & transition_data, const std::map<std::string, std::set<std::string> > & list_symbols)

> [spec:hfst:sem:hfst-transition-graph.is-list-symbol-fn]
> Static. Returns whether the symbol on `transition_data` is a known list
> symbol. Steps: isymbol = input symbol, osymbol = output symbol. If isymbol !=
> osymbol, throw the C-string literal "is_list_symbol: input and output symbols
> must be the same". Otherwise return whether isymbol is a key in the
> `list_symbols` map (find != end). Read-only.

> [spec:hfst:def:hfst-transition-graph.is-lookup-infinitely-ambiguous-fn]
> HFSTDLL bool is_lookup_infinitely_ambiguous

> [spec:hfst:sem:hfst-transition-graph.is-lookup-infinitely-ambiguous-fn]
> Recursive DFS (the 6-arg overload) detecting whether looking up the
> one-level path `s` against this graph is infinitely ambiguous (an epsilon/flag
> loop reachable while at lookup position `index` in `state`).
> `epsilon_path_states` is the current epsilon-only DFS stack, `fds` the flag
> stack, `obey_flags` whether flags are enforced. Steps: set only_epsilons =
> (index == s.second.size()) i.e. we are at the end of the input. Iterate
> `state`'s transitions. For each: compute possible_flag = is_possible_flag(
> input symbol, fds, obey_flags). CASE 1: if input symbol is_epsilon OR
> possible_flag (epsilon-like, consumes no input): insert `state` into
> epsilon_path_states; if the target is already in epsilon_path_states return
> true; else recurse with the same index and the target state, returning true if
> it does; then erase `state` from epsilon_path_states, and if possible_flag pop
> `fds`. CASE 2 (else, only if !only_epsilons): determine whether this
> transition can consume s.second.at(index): continu=true if input symbol equals
> that symbol, OR if input symbol is "@_UNKNOWN_SYMBOL_@"/"@_IDENTITY_SYMBOL_@"
> and that symbol is not in `alphabet`. If continu: index++, recurse with a
> fresh empty epsilon_path_states (epsilon stack resets when input is consumed),
> the target state; return true if it does; then index--. After all transitions,
> return false. The two public overloads seed epsilon_path_states={0}, index=0,
> empty fds, and call this from INITIAL_STATE; the StringVector overload first
> wraps s into an HfstOneLevelPath with weight 0.

> [spec:hfst:def:hfst-transition-graph.is-possible-flag-fn]
> bool is_possible_flag(std::string symbol, StringVector & fds, bool obey_flags)

> [spec:hfst:sem:hfst-transition-graph.is-possible-flag-fn]
> Tests whether `symbol` is a flag diacritic that may currently be applied,
> given the flag-diacritic stack `fds` and `obey_flags`. Steps: if
> FdOperation::is_diacritic(symbol) is false, return false. Otherwise create a
> FlagDiacriticTable FdT and push `symbol` onto `fds`. If !obey_flags OR
> FdT.is_valid_string(fds) is true, return true (the flag is left pushed on
> `fds`). Else pop `symbol` back off `fds` and return false. Mutates `fds` (push
> remains only when returning true).

> [spec:hfst:def:hfst-transition-graph.is-possible-transition-fn]
> HFSTDLL static bool is_possible_transition

> [spec:hfst:sem:hfst-transition-graph.is-possible-transition-fn]
> Static. Decides whether `transition` can be taken during a lookup at position
> `lookup_index` in `lookup_path`, given `alphabet`; sets out-param
> `input_symbol_consumed` accordingly. Steps: isymbol = transition input symbol.
> If not at the end of lookup_path (lookup_index != lookup_path.size()): if
> isymbol equals lookup_path.at(lookup_index), OR isymbol is identity/unknown and
> lookup_path.at(lookup_index) is not in `alphabet`, then set
> input_symbol_consumed=true and return true. Regardless of position: if isymbol
> is_epsilon, set input_symbol_consumed=false and return true. If
> FdOperation::is_diacritic(isymbol): if fds_so_far is NULL, set
> input_symbol_consumed=false and return true; else push isymbol onto *fds_so_far,
> compute valid=FlagDiacriticTable::is_valid_string(*fds_so_far), pop it back,
> and if valid set input_symbol_consumed=false and return true. If none of these,
> return false (input_symbol_consumed left unchanged on the false paths).

> [spec:hfst:def:hfst-transition-graph.is-special-symbol-fn]
> HFSTDLL bool is_special_symbol(const std::string & symbol)

> [spec:hfst:sem:hfst-transition-graph.is-special-symbol-fn]
> Returns whether `symbol` is a special symbol, i.e. one beginning with "@_".
> Steps: if symbol.size() < 2, return false. If symbol[0]=='@' AND symbol[1]=='_',
> return true; otherwise return false. Read-only.

> [spec:hfst:def:hfst-transition-graph.longest-path-size-fn]
> HFSTDLL int longest_path_size()

> [spec:hfst:sem:hfst-transition-graph.longest-path-size-fn]
> Returns the length of the longest string accepted, or -1 if none. Steps: call
> topsort(MaximumDistance) to get states_sorted (vector index = max distance ->
> set of states). Iterate distance from states_sorted.size()-1 down to 0; for
> each distance, iterate the set of states at that distance; if any is a final
> state (is_final_state), return that distance immediately. If the descent
> completes with no final state found, return -1.

> [spec:hfst:def:hfst-transition-graph.lookup-fn]
> HFSTDLL void lookup

> [spec:hfst:sem:hfst-transition-graph.lookup-fn]
> Recursive DFS lookup core (the long overload). Walks the graph from `state`
> consuming `lookup_path` from position `lookup_index`, accumulating into
> `path_so_far`, collecting completed paths into `results`. `Eh` is an
> HfstEpsilonHandler bounding input-epsilon cycles, `max_epsilon_cycles`/
> `max_weight`/`flag_diacritic_path` optional limits/state. Steps:
> - If !Eh.can_continue(state), return (epsilon-cycle limit reached).
> - If max_weight!=NULL and path_so_far.first > *max_weight, return.
> - If lookup_index == lookup_path.size() (input exhausted) and state is final,
>   call add_to_results(results, path_so_far, get_final_weight(state),
>   max_weight).
> Then iterate `state`'s transitions. For each, call is_possible_transition(*it,
> lookup_path, lookup_index, alphabet, input_symbol_consumed,
> flag_diacritic_path); if true: build istr/ostr — if input is identity, istr =
> lookup_path.at(lookup_index) and ostr = istr; else istr = lookup symbol if
> input is unknown else the transition input symbol, and ostr = transition output
> symbol. push_back_to_two_level_path(path_so_far, (istr,ostr), weight,
> flag_diacritic_path). If input_symbol_consumed: increment lookup_index and use
> a freshly-allocated HfstEpsilonHandler(max_epsilon_cycles); else push `state`
> onto Eh and reuse it. Recurse lookup on the transition's target with the chosen
> handler. Afterwards, if input_symbol_consumed: decrement lookup_index and
> delete the allocated handler. Finally pop_back_from_two_level_path(path_so_far,
> weight, flag_diacritic_path) to backtrack. No return value; mutates results and
> the threaded path/handler state.

> [spec:hfst:def:hfst-transition-graph.main-fn]
> int main(int argc, char * argv[])

> [spec:hfst:sem:hfst-transition-graph.main-fn]
> The unit-test `main` compiled only when MAIN_TEST is defined. Steps: print
> "Unit tests for <file>:" to std::cout, then immediately `return EXIT_SUCCESS`.
> All code after that early return is dead/unreachable (it constructs a small
> HfstBasicTransducer g1 with transitions a/b/c/d to state 1, a final weight,
> builds substitution transducers subst1/subst2 keyed "a"/"b", substitutes,
> removes the d transition, writes att format to stdout, and returns again) but
> is never executed because of the preceding return. Effectively this function
> only prints the header line and returns success.

> [spec:hfst:def:hfst-transition-graph.marker-pair-fn]
> HfstSymbolPair marker_pair(marker, marker)

> [spec:hfst:sem:hfst-transition-graph.marker-pair-fn]
> The body of insert_freely(const HfstTransitionGraph & graph) (the overload
> that inserts `graph` freely at every state). Preceding this annotated line it
> picks a fresh marker symbol not in the alphabet: marker_this =
> C::get_marker(alphabet), marker_graph = C::get_marker(alphabet), marker =
> max(marker_this, marker_graph). The annotated statement constructs
> HfstSymbolPair marker_pair(marker, marker) (a symbol pair with the marker on
> both sides). Then: insert_freely(marker_pair, 0) inserts a marker:marker
> self-loop with weight 0 at every state; substitute(marker_pair, graph)
> replaces each marker:marker arc with a copy of `graph`; alphabet.erase(marker)
> removes the temporary marker. Returns *this.

> [spec:hfst:def:hfst-transition-graph.marker2weight-fn]
> HFSTDLL bool marker2weight(const std::string & str, float & weight)

> [spec:hfst:sem:hfst-transition-graph.marker2weight-fn]
> Inverse of weight2marker. Parses a marker string `str` of the form
> "@<float>@" into out-param `weight`. Returns whether parsing succeeded. Steps:
> if str.size() < 3, return false. If str[0] != '@' OR the last char != '@',
> return false. Take the substring between the two '@' (str.substr(1,
> str.size()-2)), parse it as a float via a stringstream into `weight`; if the
> read fails, return false. Otherwise return true. `weight` is written only on
> success.

> [spec:hfst:def:hfst-transition-graph.parse-prolog-arc-line-fn]
> HFSTDLL static bool parse_prolog_arc_line(const std::string & line, HfstTransitionGraph & graph)

> [spec:hfst:sem:hfst-transition-graph.parse-prolog-arc-line-fn]
> Static. Parses a prolog `arc(NAME, SOURCE, TARGET, SYMBOL).` line and adds the
> corresponding transition to `graph`. Returns whether parsing succeeded.
> Steps: sscanf(line, "arc(%[^,], %[^,], %[^,], %[^\t\n]", namestr, sourcestr,
> targetstr, symbolstr) into four char[100] buffers; n is the number of fields.
> Build symbol = symbolstr. Call strip_ending_parenthesis_and_comma(symbol) to
> drop the trailing ")."; if it returns false, return false. If n != 4, return
> false. If namestr != graph.name, return false. Compute source=atoi(sourcestr),
> target=atoi(targetstr). Set weight=0 and call extract_weight(symbol, weight)
> (parses and trims any trailing weight); if it returns false, return false.
> Then isymbol="", osymbol=""; call get_prolog_arc_symbols(symbol, isymbol,
> osymbol); if it returns false, return false. Finally
> graph.add_transition(source, HfstTransition<C>(target, isymbol, osymbol,
> weight)) and return true. Mutates graph on success.

> [spec:hfst:def:hfst-transition-graph.parse-prolog-final-line-fn]
> HFSTDLL static bool parse_prolog_final_line(const std::string & line, HfstTransitionGraph & graph)

> [spec:hfst:sem:hfst-transition-graph.parse-prolog-final-line-fn]
> Static. Parses a prolog `final(NAME, number).` or `final(NAME, number,
> weight).` line and marks the state final in `graph`. Returns whether parsing
> succeeded. Steps: count the commas in `line` (scan for ',' repeatedly).
> - If exactly 1 comma: sscanf(line, "final(%[^,], %[^)]).", namestr, finalstr);
>   if it does not read 2 fields, return false. weight stays 0.
> - If exactly 2 commas: sscanf(line, "final(%[^,], %[^,], %[^)]).", namestr,
>   finalstr, weightstr); if it does not read 3 fields, return false. Parse
>   weightstr as a float via an istringstream into `weight`; if the read fails,
>   return false.
> - Otherwise (0 or >2 commas): return false.
> Then if namestr != graph.name, return false. Otherwise call
> graph.set_final_weight(atoi(finalstr), weight) and return true.

> [spec:hfst:def:hfst-transition-graph.parse-prolog-symbol-line-fn]
> HFSTDLL static bool parse_prolog_symbol_line(const std::string & line, HfstTransitionGraph & graph)

> [spec:hfst:sem:hfst-transition-graph.parse-prolog-symbol-line-fn]
> Static. Parses a prolog `symbol(NAME, "foo").` line and inserts the symbol
> into `graph`'s alphabet. Returns whether parsing succeeded. Steps:
> sscanf(line, "symbol(%[^,], %s", namearr, symbolarr) into two char[100]
> buffers; if it does not read exactly 2 fields, return false. Build
> namestr=namearr, symbolstr=symbolarr. If namestr != graph.name, return false.
> Call strip_ending_parenthesis_and_comma(symbolstr) to drop the trailing ")."
> — if false, return false. Call strip_quotes_from_both_sides(symbolstr) to drop
> the surrounding double quotes — if false, return false. Then
> graph.add_symbol_to_alphabet(deprologize_symbol(symbolstr)) and return true.

> [spec:hfst:def:hfst-transition-graph.pop-back-from-two-level-path-fn]
> HFSTDLL static void pop_back_from_two_level_path

> [spec:hfst:sem:hfst-transition-graph.pop-back-from-two-level-path-fn]
> Static. Inverse of push_back_to_two_level_path: removes the last symbol pair
> from a running two-level path. Steps: if `fds_so_far` is not NULL, read the
> last pair sp = path.second.back(); if FdOperation::is_diacritic(sp.first),
> pop_back from *fds_so_far. Then pop_back from path.second. Subtract `weight`
> from path.first. No return value; mutates `path` and optionally `fds_so_far`.

> [spec:hfst:def:hfst-transition-graph.purge-symbol-fn]
> HFSTDLL bool purge_symbol(const std::string & symbol, const std::string & flag)

> [spec:hfst:sem:hfst-transition-graph.purge-symbol-fn]
> Returns whether `symbol` must be purged given that flag feature `flag` is
> being eliminated. Steps: if FdOperation::is_diacritic(symbol) is false (not a
> flag diacritic), return false. Else if `flag` is empty, return true (purge all
> flags). Else if FdOperation::get_feature(symbol) == flag, return true (this
> diacritic belongs to the purged feature). Otherwise return false. Read-only.

> [spec:hfst:def:hfst-transition-graph.push-back-to-two-level-path-fn]
> HFSTDLL static void push_back_to_two_level_path

> [spec:hfst:sem:hfst-transition-graph.push-back-to-two-level-path-fn]
> Static. Appends one symbol pair to a running two-level path. Steps: push `sp`
> onto path.second (the symbol-pair vector). Add `weight` to path.first (the
> running weight). If `fds_so_far` is not NULL and FdOperation::is_diacritic(
> sp.first) is true, push sp.first onto *fds_so_far. No return value; mutates
> `path` and optionally `fds_so_far`.

> [spec:hfst:def:hfst-transition-graph.remove-transitions-fn]
> HFSTDLL void remove_transitions(const HfstSymbolPair &sp)

> [spec:hfst:sem:hfst-transition-graph.remove-transitions-fn]
> Removes every transition whose (input,output) symbol pair equals `sp`, and
> prunes the two symbols from the alphabet if they no longer occur anywhere.
> Steps: in_match=C::get_number(sp.first), out_match=C::get_number(sp.second).
> Track in_match_used=false, out_match_used=false. Iterate all states; for each,
> iterate its transitions by index i: read in_tr=input number, out_tr=output
> number. If in_tr==in_match AND out_tr==out_match, erase that transition
> (it->erase(begin()+i)) — note i is NOT decremented, so the next transition is
> skipped from inspection. Otherwise: if in_tr==in_match OR out_tr==in_match set
> in_match_used=true; if in_tr==out_match OR out_tr==out_match set
> out_match_used=true. After all states: if !in_match_used erase sp.first from
> alphabet; if !out_match_used erase sp.second from alphabet. No return value.

> [spec:hfst:def:hfst-transition-graph.sort-distance]
> enum SortDistance {
>   MaximumDistance;
>   MinimumDistance;
> }

> [spec:hfst:def:hfst-transition-graph.state-map]
> typedef std::map<StatePair, HfstState> StateMap

> [spec:hfst:def:hfst-transition-graph.state-pair]
> typedef std::pair<HfstState, HfstState> StatePair

> [spec:hfst:def:hfst-transition-graph.std.string-get-stripped-line-fn]
> HFSTDLL static std::string get_stripped_line

> [spec:hfst:sem:hfst-transition-graph.std.string-get-stripped-line-fn]
> Static. Reads one line (up to 254 chars into a char[255] buffer) from istream
> `is` when `file`==NULL, else from FILE `file`, strips its trailing newlines,
> increments `linecount`, and returns it. Steps: if file==NULL, call
> is.getline(line,255); if the result reports eof (no line read), throw
> EndOfStreamException. Else (file!=NULL) call fgets(line,255,file); if it
> returns NULL, throw EndOfStreamException. Then linecount++. Build a string
> from `line`, pass it through strip_newlines, and return the result.

> [spec:hfst:def:hfst-transition-graph.std.string-strip-newlines-fn]
> HFSTDLL static std::string strip_newlines(std::string & str)

> [spec:hfst:sem:hfst-transition-graph.std.string-strip-newlines-fn]
> Static. Removes trailing newline characters from `str` in place. Iterate i
> from str.length()-1 downward; while str[i] is '\n' or '\r', erase that
> character; on the first character that is neither, break. Returns the
> (mutated) `str`. Only trailing '\n'/'\r' are removed; interior ones are kept.

> [spec:hfst:def:hfst-transition-graph.std.string-weight2marker-fn]
> HFSTDLL std::string weight2marker(float weight)

> [spec:hfst:sem:hfst-transition-graph.std.string-weight2marker-fn]
> Encodes a float `weight` as a marker symbol string. Stream `weight` into an
> ostringstream (default float formatting), then return "@" + that text + "@".
> E.g. weight 1.5 yields "@1.5@". Read-only / pure.

> [spec:hfst:def:hfst-transition-graph.std.vector-std.set-hfst-state-topsort-fn]
> HFSTDLL std::vector<std::set<HfstState> > topsort(SortDistance dist) const

> [spec:hfst:sem:hfst-transition-graph.std.vector-std.set-hfst-state-topsort-fn]
> Computes a topological sort by distance from the start state and returns a
> vector indexed by distance: result[d] = set of states whose distance from
> state 0 is d. `dist` selects MaximumDistance or MinimumDistance semantics
> (passed to set_state_at_distance as the boolean `dist == MaximumDistance`).
> Steps: current_distance=0. Create a TopologicalSort `TopSort`; call
> set_biggest_state_number(state_vector.size()-1) and set_state_at_distance(0,
> 0, dist==MaximumDistance) (place the start state at distance 0). Then a
> do-while loop: each iteration set new_states_found=false and an empty set
> new_states; get the set of states currently at current_distance
> (TopSort.get_states_at_distance); for each such state walk all its transitions,
> setting new_states_found=true and inserting each transition's target into
> new_states. Then for each state in new_states call set_state_at_distance(state,
> current_distance+1, dist==MaximumDistance). Increment current_distance. Repeat
> while new_states_found. Finally return TopSort.states_at_distance. (The
> distance update rule, max vs min, is implemented inside set_state_at_distance.)
> Read-only on the graph; mutates only the local TopSort.

> [spec:hfst:def:hfst-transition-graph.std.vector-unsigned-int-path-sizes-fn]
> HFSTDLL std::vector<unsigned int> path_sizes()

> [spec:hfst:sem:hfst-transition-graph.std.vector-unsigned-int-path-sizes-fn]
> Returns the distinct accepted path lengths in descending order (empty if none
> accepted). Steps: result is an empty vector. Call topsort(MinimumDistance) to
> get states_sorted (index = min distance -> set of states). Iterate distance
> from states_sorted.size()-1 down to 0; for each distance, scan the states at
> that distance; on the first that is final (is_final_state), push that distance
> onto result and break to the next distance. Return result. So each distance
> level that contains at least one final state contributes its value once, in
> descending order.

> [spec:hfst:def:hfst-transition-graph.subst-map]
> typedef std::map<HfstSymbol, HfstTransitionGraph> SubstMap

> [spec:hfst:def:hfst-transition-graph.substitute-fn]
> void substitute_(const HfstSymbolPair &old_sp,

> [spec:hfst:sem:hfst-transition-graph.substitute-fn]
> Protected. In-place substitution: replaces every transition matching the
> symbol pair `old_sp` with the set of pairs `new_sps`. Steps: if new_sps is
> empty, tail-call remove_transitions(old_sp) and return. Compute
> old_input_number, old_output_number via C::get_number on old_sp's symbols.
> Track substitution_performed=false. Iterate all states; for each, maintain a
> local list new_transitions. Iterate the state's transitions by index i: if a
> transition's input number == old_input_number AND output number ==
> old_output_number, set substitution_performed=true. Take the first pair in
> new_sps and overwrite the current transition in place with one keeping the
> same target and weight but the first pair's input/output numbers (the `true`
> ctor flag indicates numbers are passed). Then loop over ALL pairs in new_sps
> (from begin to end) building an equivalent transition for each (same target
> and weight) and push them onto new_transitions — note this loop starts at
> begin(), so the first pair is effectively added a second time as well. After
> scanning the state, append all new_transitions to the state. After all states:
> if substitution_performed, add_symbols_to_alphabet(new_sps). Finally build a
> set syms = {old_input_number, old_output_number} and call
> prune_alphabet_after_substitution(syms) to drop now-unused old symbols.
> Returns void; mutates transitions and alphabet.

> [spec:hfst:def:hfst-transition-graph.substitution-data]
> struct substitution_data {
>   HfstState origin_state;
>   HfstState target_state;
>   typename C::WeightType weight;
>   HfstTransitionGraph * substituting_graph;
> }

> [spec:hfst:def:hfst-transition-graph.substitution-data.substitution-data-fn]
> substitution_data(HfstState origin,

> [spec:hfst:sem:hfst-transition-graph.substitution-data.substitution-data-fn]
> Constructor for the `substitution_data` struct. Stores its four arguments into
> the corresponding members: origin_state=origin, target_state=target,
> this->weight=weight, substituting_graph=substituting. No other behaviour.

> [spec:hfst:def:hfst-transition-graph.topological-sort]
> struct TopologicalSort {
>   std::vector<int> distance_of_state;
>   std::vector<std::set<HfstState> > states_at_distance;
> }

> [spec:hfst:def:hfst-transition-graph.topological-sort.set-biggest-state-number-fn]
> HFSTDLL void set_biggest_state_number(unsigned int biggest_state_number)

> [spec:hfst:sem:hfst-transition-graph.topological-sort.set-biggest-state-number-fn]
> Member of TopologicalSort. Initializes/resizes the `distance_of_state` member
> to a std::vector<int> of length biggest_state_number+1, every element set to
> -1 (meaning "no distance assigned yet"). No return value; replaces any prior
> contents of distance_of_state.

> [spec:hfst:def:hfst-transition-graph.topological-sort.set-state-at-distance-fn]
> HFSTDLL void set_state_at_distance(HfstState state, unsigned int distance,

> [spec:hfst:sem:hfst-transition-graph.topological-sort.set-state-at-distance-fn]
> Member of TopologicalSort. Records that `state` is at distance `distance`,
> using `overwrite` to decide max-vs-min behaviour. Steps: if `state` >
> distance_of_state.size()-1, print an out-of-range error to std::cerr (and
> continue). Grow states_at_distance with empty sets until its size > distance
> (i.e. index `distance` exists). Read previous_distance = distance_of_state.at(
> state); if previous_distance != -1 AND previous_distance != distance AND
> `overwrite` is true, erase `state` from states_at_distance.at(
> previous_distance) (moving it). Then insert `state` into states_at_distance.at(
> distance) and set distance_of_state.at(state) = distance. With overwrite true
> (MaximumDistance) a later, larger distance replaces an earlier one; with
> overwrite false (MinimumDistance) the first-seen (smallest) distance is kept in
> distance_of_state but the state is still added to the new distance's set. No
> return value; mutates the TopologicalSort's members.

> [spec:hfst:def:hfst-transition-graph.void-find-regexp-paths-fn]
> HFSTDLL void find_regexp_paths

> [spec:hfst:sem:hfst-transition-graph.void-find-regexp-paths-fn]
> The 3-argument seeding overload of find_regexp_paths (no states_visited/path
> params). For state `s` it locates each outgoing "^[" transition and launches
> the recursive 5-argument search from there, collecting results into
> `full_paths`. `input_side` selects which symbol side is inspected.
> Steps: get the transitions of state `s` via this->operator[](s). Iterate them;
> for each transition read istr=input_symbol, ostr=output_symbol. If (input_side
> && istr=="^[") OR (!input_side && ostr=="^["), this transition opens a regexp:
> create a fresh states_visited set seeded with {s}, create a fresh `path` vector
> seeded with the single pair (istr, ostr), then call the recursive overload
> find_regexp_paths(transition.target_state, states_visited, path, full_paths,
> input_side). Non-"^[" transitions are ignored at this level. No return value;
> weights ignored. Each call uses its own local states_visited/path so multiple
> "^[" branches out of `s` are independent.

> [spec:hfst:def:hfst-transition-graph.write-in-att-format-fn]
> HFSTDLL void write_in_att_format(char * ptr, bool write_weights=true)

> [spec:hfst:sem:hfst-transition-graph.write-in-att-format-fn]
> Writes the whole graph in AT&T text format into the caller-supplied char
> buffer `ptr` (no bounds checking; caller must size it). `write_weights`
> controls whether weights are emitted. Maintains cwt = total characters written
> so far (running offset into ptr) and writes each chunk with sprintf at
> ptr+cwt, adding the returned length to cwt.
> Steps: source_state=0; cwt=0. Iterate states begin()..end(). For each state,
> iterate its transitions; for each transition get its data, then derive isymbol
> and osymbol from the input/output symbols by replace_all of: " "->"@_SPACE_@",
> "@_EPSILON_SYMBOL_@"->"@0@", "\t"->"@_TAB_@". sprintf "%i\t%i\t%s\t%s" with
> source_state, target_state, isymbol, osymbol; if write_weights, sprintf
> "\t%f" with the transition weight; then sprintf "\n". After the transitions,
> if is_final_state(source_state): sprintf "%i" with source_state; if
> write_weights sprintf "\t%f" with get_final_weight(source_state); then sprintf
> "\n". Increment source_state. No return value; output goes only into `ptr`.

> [spec:hfst:def:hfst-transition-graph.write-in-att-format-number-fn]
> HFSTDLL void write_in_att_format_number(FILE *file, bool write_weights=true)

> [spec:hfst:sem:hfst-transition-graph.write-in-att-format-number-fn]
> Writes the graph in AT&T text format to FILE `file` using numeric symbol ids
> instead of symbol names. `write_weights` controls weight emission.
> Steps: source_state=0. Iterate states begin()..end(). For each state, iterate
> its transitions; for each transition get its data and fprintf "%i\t%i\t%i\t%i"
> with source_state, target_state, get_input_number(), get_output_number(); if
> write_weights fprintf "\t%f" with the weight; fprintf "\n". Then, STILL INSIDE
> the per-transition loop, if is_final_state(source_state): fprintf "%i" with
> source_state, optionally "\t%f" with get_final_weight, then "\n". (Note: unlike
> the symbol-name overload, the final-state line is emitted inside the transition
> loop, so for a final state with k outgoing transitions the final line is
> written k times, and a final state with zero transitions gets no final line —
> reproduce this behaviour exactly.) Increment source_state after each state. No
> return value; output goes to `file`.

> [spec:hfst:def:hfst-transition-graph.write-in-xfst-format-fn]
> HFSTDLL void write_in_xfst_format(FILE * file, bool write_weights=true)

> [spec:hfst:sem:hfst-transition-graph.write-in-xfst-format-fn]
> FILE* overload. Writes the graph in xfst text format to `file`.
> `write_weights` is accepted but ignored (cast to void). Iterates over all
> states with a `source_state` counter starting at 0. For each state: call
> print_xfst_state(file, source_state), then print ":\t". If the state has no
> transitions (begin()==end()), print "(no arcs)". Otherwise for each
> transition, print ", " before all but the first, then call
> print_xfst_arc(file, data) for the transition data, print " -> ", and call
> print_xfst_state(file, target_state). After the arcs, print ".\n" and
> increment source_state. No return value. (There is a sibling ostream
> overload with the same structure.)

