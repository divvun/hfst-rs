# libhfst/src/implementations/compose_intersect/ComposeIntersectFst.cc, libhfst/src/implementations/compose_intersect/ComposeIntersectFst.h

> [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst]
> class ComposeIntersectFst {
>   struct Transition { size_t ilabel; size_t olabel; float weight; HfstState target; Transition(const HfstBasicTransition &); Transition(HfstState,size_t,size_t...;
>   struct CompareTransitions { bool operator() (const Transition &transition1, const Transition &transition2) const; };
>   static const HfstState START;
>   virtual const TransitionSet & get_transitions(HfstState,size_t);
>   const SymbolSet &get_symbols(void) const;
>   HfstBasicTransducer t;
>   SymbolSet symbol_set;
>   TransitionMapVector transition_map_vector;
>   FloatVector finality_vector;
>   TransitionVector identity_transition_vector;
> }

> [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.compare-transitions]
> struct CompareTransitions

> [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.compare-transitions.operator-fn]
> bool ComposeIntersectFst::CompareTransitions::operator()

> [spec:hfst:sem:compose-intersect-fst.hfst.implementations.compose-intersect-fst.compare-transitions.operator-fn]
> Strict weak ordering comparator over two `Transition` values `tr1`, `tr2` (used as the comparator for the `TransitionSet`/`SpaceSavingSet`). Returns true when `tr1` should sort before `tr2`. Compares fields lexicographically in this order: `ilabel`, then `olabel`, then `weight`, then `target`. Concretely: if `tr1.ilabel != tr2.ilabel` return `tr1.ilabel < tr2.ilabel`; else if `tr1.olabel != tr2.olabel` return `tr1.olabel < tr2.olabel`; else if `tr1.weight != tr2.weight` return `tr1.weight < tr2.weight`; else return `tr1.target < tr2.target`. Pure, const, no side effects.

> [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.compose-intersect-fst-fn]
> ComposeIntersectFst::ComposeIntersectFst

> [spec:hfst:sem:compose-intersect-fst.hfst.implementations.compose-intersect-fst.compose-intersect-fst-fn]
> Constructor taking a `const HfstBasicTransducer &t` and a bool `input_keys`. Copy-initializes member `this->t` from `t`, then calls `this->t.sort_arcs()` to sort the arcs of the stored copy.
> Builds `symbol_set`: gets `this->t.get_alphabet()` (a `std::set<std::string>`) and for each alphabet symbol string inserts `HfstTropicalTransducerTransitionData::get_number(symbol)` into `symbol_set`.
> Then iterates over the states of `this->t` via its iterator, tracking a counter `source_state` starting at 0. For each state:
>   - push a fresh empty `SymbolTransitionMap` onto `transition_map_vector`.
>   - if `this->t.is_final_state(source_state)` push `this->t.get_final_weight(source_state)` onto `finality_vector`; otherwise push `std::numeric_limits<float>::infinity()`.
>   - increment `source_state`.
>   - take a reference `symbol_transition_map` to the just-pushed back map, and set a local flag `identity_found = false`.
>   - iterate over the state's outgoing transitions (`HfstBasicTransition`): if a transition's input symbol equals the literal `"@_IDENTITY_SYMBOL_@"`, set `identity_found = true` and push that transition (converted to a `Transition`) onto `identity_transition_vector`. Otherwise, compute the key symbol number as `get_number(input symbol)` when `input_keys` is true else `get_number(output symbol)`, and insert the transition (converted to a `Transition`) into `symbol_transition_map[key]` (the `TransitionSet` for that key).
>   - after the transition loop, if `identity_found` is still false, push onto `identity_transition_vector` a `Transition` constructed with target 0, ilabel and olabel both equal to `get_number("@_EPSILON_SYMBOL_@")`, and weight 0 (a placeholder non-identity entry).
> Net effect: `transition_map_vector`, `finality_vector`, and `identity_transition_vector` all end up with one entry per state, indexed by state number.

> [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.float-vector]
> typedef std::vector<float> FloatVector

> [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.get-final-weight-fn]
> float ComposeIntersectFst::get_final_weight(HfstState s) const

> [spec:hfst:sem:compose-intersect-fst.hfst.implementations.compose-intersect-fst.get-final-weight-fn]
> Const accessor returning the final weight of state `s`. If `s >= transition_map_vector.size()` throw `StateNotDefined`. Otherwise return `finality_vector.at(s)` (which holds the state's final weight, or `+infinity` if the state is non-final). No mutation.

> [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.get-identity-transition-fn]
> ComposeIntersectFst::Transition

> [spec:hfst:sem:compose-intersect-fst.hfst.implementations.compose-intersect-fst.get-identity-transition-fn]
> Returns the `Transition` stored for state `s` in `identity_transition_vector`. If `s >= transition_map_vector.size()` throw `StateNotDefined`. Otherwise return `identity_transition_vector.at(s)` by value. Note the entry may be a real identity transition or the epsilon placeholder inserted by the constructor when the state had no identity transition. No mutation.

> [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.get-symbol-number-fn]
> size_t ComposeIntersectFst::get_symbol_number(const std::string &symbol)

> [spec:hfst:sem:compose-intersect-fst.hfst.implementations.compose-intersect-fst.get-symbol-number-fn]
> Returns `HfstTropicalTransducerTransitionData::get_number(symbol)` for the given symbol string — i.e. looks up (and as a side effect of that static call may assign) the global numeric id for the symbol. One-line delegation; no member state read or written.

> [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.has-identity-transition-fn]
> bool ComposeIntersectFst::has_identity_transition(HfstState s)

> [spec:hfst:sem:compose-intersect-fst.hfst.implementations.compose-intersect-fst.has-identity-transition-fn]
> Returns whether state `s` has a real identity transition. If `s >= transition_map_vector.size()` throw `StateNotDefined`. Otherwise return true iff `identity_transition_vector.at(s).ilabel == HfstTropicalTransducerTransitionData::get_number("@_IDENTITY_SYMBOL_@")` — i.e. the stored entry's input-label number equals the identity-symbol number (false when the entry is the epsilon placeholder). No mutation of member state.

> [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.is-known-symbol-fn]
> bool ComposeIntersectFst::is_known_symbol(size_t symbol) const

> [spec:hfst:sem:compose-intersect-fst.hfst.implementations.compose-intersect-fst.is-known-symbol-fn]
> Const predicate returning true iff `symbol` (a numeric symbol id) is present in `symbol_set` — i.e. `symbol_set.find(symbol) != symbol_set.end()`. No mutation.

> [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.symbol-set]
> typedef std::set<size_t> SymbolSet

> [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.symbol-transition-map]
> typedef std::map<size_t,TransitionSet> SymbolTransitionMap

> [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.transition]
> struct Transition {
>   size_t ilabel;
>   size_t olabel;
>   float weight;
>   HfstState target;
> }

> [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.transition-map-vector]
> typedef std::vector<SymbolTransitionMap> TransitionMapVector

> [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.transition-set]
> typedef compose_intersect_utilities::SpaceSavingSet

> [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.transition-vector]
> typedef std::vector<Transition> TransitionVector

> [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.transition.operator-fn]
> bool ComposeIntersectFst::Transition::operator==

> [spec:hfst:sem:compose-intersect-fst.hfst.implementations.compose-intersect-fst.transition.operator-fn]
> Equality operator on two `Transition` values. Returns true iff all four fields are equal: `ilabel == another.ilabel && olabel == another.olabel && weight == another.weight && target == another.target`. Const, pure.

> [spec:hfst:def:compose-intersect-fst.hfst.implementations.compose-intersect-fst.transition.transition-fn]
> ComposeIntersectFst::Transition::Transition(const HfstBasicTransition &t)

> [spec:hfst:sem:compose-intersect-fst.hfst.implementations.compose-intersect-fst.transition.transition-fn]
> Converting constructor from an `HfstBasicTransition &t`. Initializes the four `Transition` fields: `ilabel = HfstTropicalTransducerTransitionData::get_number(t.transition_data.get_input_symbol())`, `olabel = HfstTropicalTransducerTransitionData::get_number(t.transition_data.get_output_symbol())`, `weight = t.get_weight()`, `target = t.get_target_state()`. After initialization, asserts that `t.get_input_symbol() != ""` and `t.get_output_symbol() != ""` (debug-only checks). The symbol strings are converted to numeric ids via the static `get_number`.

> [spec:hfst:def:compose-intersect-fst.main-fn]
> int main(int argc, char * argv[])

> [spec:hfst:sem:compose-intersect-fst.main-fn]
> Compiled only under `MAIN_TEST`. Unit-test entry point. Prints `"Unit tests for <__FILE__>:"` to stdout. Constructs an `HfstTokenizer tokenizer` and registers the multichar symbol `"@_IDENTITY_SYMBOL_@"`. Builds two `HfstTransducer`s of type `TROPICAL_OPENFST_TYPE` from the strings `"abc@_IDENTITY_SYMBOL_@"` (t) and `"bcd@_IDENTITY_SYMBOL_@"` (s) using that tokenizer. Disjuncts `s` into `t` (`t.disjunct(s)`), applies `t.repeat_star()`, then `t.minimize()`. The actual `ComposeIntersectFst` construction and printing are commented out (no suitable constructor). Prints `"ok"` to stdout and returns 0.

