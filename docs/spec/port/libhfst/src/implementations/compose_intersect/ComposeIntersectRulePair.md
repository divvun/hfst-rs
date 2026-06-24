# libhfst/src/implementations/compose_intersect/ComposeIntersectRulePair.cc, libhfst/src/implementations/compose_intersect/ComposeIntersectRulePair.h

> [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair]
> class ComposeIntersectRulePair : public ComposeIntersectRule {
>   static const HfstState START;
>   virtual const TransitionSet &get_transitions(HfstState,size_t);
>   StatePairVector state_pair_vector;
>   PairStateMap pair_state_map;
>   StateTransitionVector state_transition_vector;
>   ComposeIntersectRule * fst1;
>   ComposeIntersectRule * fst2;
> }

> [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.add-transition-fn]
> void ComposeIntersectRulePair::add_transition

> [spec:hfst:sem:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.add-transition-fn]
> Constructs a `Transition` from the four scalar arguments `target`
> (HfstState), `input_symbol`, `output_symbol` (both size_t), and
> `weight` (float), in that order, and inserts it into the
> `transitions` TransitionSet passed by reference. Returns nothing.
> Mutates only `transitions`; no other state, I/O, or exceptions.

> [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.compose-intersect-rule-pair-fn]
> ComposeIntersectRulePair::ComposeIntersectRulePair

> [spec:hfst:sem:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.compose-intersect-rule-pair-fn]
> Constructor taking two `ComposeIntersectRule *` pointers, `fst1`
> and `fst2`, stored into the member fields `fst1` and `fst2`
> (taking ownership; the destructor `delete`s both). Then it
> initializes member state to represent the single start state, which
> is the pair (fst1's start, fst2's start):
> - Sets this object's inherited `symbol_set` to `fst1->get_symbols()`.
> - Inserts into `pair_state_map` the entry mapping the StatePair
>   (ComposeIntersectRule::START, ComposeIntersectRule::START) to the
>   value `START` (which equals 0).
> - Pushes that same StatePair onto `state_pair_vector` (so index 0
>   corresponds to the start pair).
> - Pushes a fresh empty `SymbolTransitionMap` onto
>   `state_transition_vector` (so index 0 has no transitions computed
>   yet).
> No I/O or exceptions.

> [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.compute-transition-set-fn]
> void ComposeIntersectRulePair::compute_transition_set

> [spec:hfst:sem:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.compute-transition-set-fn]
> Computes and caches the composed/intersected transitions out of
> composite `state` on input `symbol`. Steps:
> - Looks up `state_pair = state_pair_vector[state]`, the pair
>   (s1, s2) of underlying states in fst1 and fst2.
> - Fetches `fst1_transitions = fst1->get_transitions(state_pair.first,
>   symbol)` and `fst2_transitions = fst2->get_transitions(
>   state_pair.second, symbol)` (both const TransitionSet references),
>   and obtains begin iterators `it` and `jt` over them.
> - Forces creation of the cache slot `state_transition_vector[state]
>   [symbol]` (the `(void)` indexing default-constructs an empty entry
>   if absent, so this symbol is now considered "computed").
> - Builds a local empty TransitionSet `transitions`, then performs a
>   sorted merge-join over the two transition sets (TransitionSet is
>   ordered, so both are iterated in ascending order, primarily by
>   output label `olabel`): while both iterators are non-end, compare
>   `it->olabel` and `jt->olabel`. If equal, the two transitions match
>   on output: take `output = it->olabel`, compute
>   `target = get_state(StatePair(it->target, jt->target))` (allocating
>   a new composite state id if that pair is new), `weight =
>   it->weight + jt->weight`, call `add_transition(transitions, target,
>   symbol, output, weight)` (so the new transition has input label =
>   the queried `symbol` and output label = the shared `olabel`), then
>   advance both iterators. If `it->olabel < jt->olabel`, advance only
>   `it`; otherwise advance only `jt`.
> - After the loop, assigns the accumulated `transitions` into
>   `state_transition_vector[state][symbol]`, overwriting the slot.
> Mutates `state_transition_vector`, and via `get_state` may also grow
> `pair_state_map`, `state_pair_vector`, and `state_transition_vector`.
> Returns nothing.

> [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.get-final-weight-fn]
> float ComposeIntersectRulePair::get_final_weight(HfstState s) const

> [spec:hfst:sem:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.get-final-weight-fn]
> Returns the final weight of composite state `s` (const). If
> `has_state(s)` is false, throws `StateNotDefined` (via HFST_THROW).
> Otherwise looks up `state_pair = state_pair_vector[s]` and returns
> `fst1->get_final_weight(state_pair.first) +
> fst2->get_final_weight(state_pair.second)` (the sum of the two
> underlying states' final weights). No mutation or I/O.

> [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.get-state-fn]
> HfstState ComposeIntersectRulePair::get_state(const StatePair &p)

> [spec:hfst:sem:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.get-state-fn]
> Returns the composite HfstState id for the state pair `p`,
> allocating a new id if `p` has not been seen. If `has_pair(p)` is
> false: assigns `pair_state_map[p] = size_t_to_uint(
> state_pair_vector.size())` (the next index), pushes `p` onto
> `state_pair_vector`, pushes a fresh empty `SymbolTransitionMap` onto
> `state_transition_vector`, and returns `size_t_to_uint(
> state_pair_vector.size() - 1)` (the index just assigned). If `p` is
> already known, returns the existing `pair_state_map[p]` without
> mutation. (`size_t_to_uint` narrows size_t to the HfstState/uint
> type.)

> [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.has-pair-fn]
> bool ComposeIntersectRulePair::has_pair

> [spec:hfst:sem:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.has-pair-fn]
> Const predicate returning whether the state pair `p` already has an
> assigned composite state: returns true iff
> `pair_state_map.find(p) != pair_state_map.end()`. No mutation.

> [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.has-state-fn]
> bool ComposeIntersectRulePair::has_state(HfstState s) const

> [spec:hfst:sem:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.has-state-fn]
> Const predicate returning whether `s` is a valid composite state id:
> returns `s < state_pair_vector.size()`. No mutation.

> [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.pair-state-map]
> typedef std::map<StatePair,HfstState> PairStateMap

> [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.state-pair]
> typedef std::pair<HfstState,HfstState> StatePair

> [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.state-pair-vector]
> typedef std::vector<StatePair> StatePairVector

> [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.state-transition-vector]
> typedef std::vector<SymbolTransitionMap> StateTransitionVector

> [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.symbol-transition-map]
> typedef std::map<size_t,TransitionSet> SymbolTransitionMap

> [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.transition-set]
> typedef ComposeIntersectRule::TransitionSet TransitionSet

> [spec:hfst:def:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.transitions-computed-fn]
> bool ComposeIntersectRulePair::transitions_computed

> [spec:hfst:sem:compose-intersect-rule-pair.hfst.implementations.compose-intersect-rule-pair.transitions-computed-fn]
> Returns whether the transitions out of `state` on `symbol` have
> already been computed/cached: returns true iff
> `state_transition_vector.at(state).find(symbol) !=
> state_transition_vector.at(state).end()` — i.e. the
> SymbolTransitionMap for `state` contains key `symbol`. Uses `.at`,
> so an out-of-range `state` throws `std::out_of_range`. No mutation.

> [spec:hfst:def:compose-intersect-rule-pair.main-fn]
> int main(int argc, char * argv[])

> [spec:hfst:sem:compose-intersect-rule-pair.main-fn]
> Unit-test entry point compiled only when MAIN_TEST is defined.
> Steps:
> - Prints `"Unit tests for " __FILE__ ":"` to stdout.
> - Creates an HfstTokenizer, then three TROPICAL_OPENFST_TYPE
>   transducers from the strings "a", "aa", "aaa" using that
>   tokenizer, setting their final weights to 1, 0.5, and 0.25
>   respectively.
> - Applies `repeat_star().minimize()` to each of the three.
> - Constructs a `ComposeIntersectRulePair` named
>   `compose_intersect_rule_pair` from `new ComposeIntersectRule(aaa)`
>   and a nested `new ComposeIntersectRulePair(new
>   ComposeIntersectRule(a), new ComposeIntersectRule(aa))`.
> - A commented-out block (disabled) would have tested that
>   get_transitions on an out-of-range state throws StateNotDefined.
> - Builds an empty std::string, wraps it in a std::stringstream,
>   writes "Print:" and then streams `compose_intersect_rule_pair`
>   into the stringstream (exercising the test-only `print` method,
>   which walks every state and symbol and emits transition and
>   final-weight lines).
> - Prints "ok" to stdout and returns 0.

