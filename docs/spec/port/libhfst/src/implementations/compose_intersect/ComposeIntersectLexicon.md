# libhfst/src/implementations/compose_intersect/ComposeIntersectLexicon.cc, libhfst/src/implementations/compose_intersect/ComposeIntersectLexicon.h

> [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon]
> class ComposeIntersectLexicon : public ComposeIntersectFst {
>   StatePairMap state_pair_map;
>   PairVector pair_vector;
>   StateQueue agenda;
>   HfstBasicTransducer result;
>   StateSet lexicon_non_epsilon_states;
>   HfstBasicTransducer &compute_composition_result (ComposeIntersectRule *);
> }

> [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.add-transition-fn]
> void ComposeIntersectLexicon::add_transition

> [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.add-transition-fn]
> Adds one transition to the member `result` (an `HfstBasicTransducer`).
> Parameters: `origin` (source state), `input` and `output` (symbol numbers as
> `size_t`), `weight` (float), `target` (destination state). Resolves `input`
> and `output` from symbol numbers back to symbol strings via
> `HfstTropicalTransducerTransitionData::get_symbol(size_t_to_uint(...))`, then
> calls `result.add_transition(origin, HfstBasicTransition(target, inputStr,
> outputStr, weight))`. No return value.

> [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.can-have-lexicon-epsilons-fn]
> bool ComposeIntersectLexicon::can_have_lexicon_epsilons(HfstState s)

> [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.can-have-lexicon-epsilons-fn]
> Returns `true` iff state `s` is present in the member set
> `lexicon_non_epsilon_states`, i.e. `lexicon_non_epsilon_states.count(s) > 0`.
> Pure predicate; no mutation.

> [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.clear-all-info-fn]
> void ComposeIntersectLexicon::clear_all_info(void)

> [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.clear-all-info-fn]
> Resets all per-composition working state. Clears `state_pair_map` and
> `pair_vector`; drains `agenda` by repeatedly popping until empty; and
> reassigns `result` to a fresh default-constructed `HfstBasicTransducer`.
> Note it does NOT clear `lexicon_non_epsilon_states`. No return value.

> [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.compose-fn]
> void ComposeIntersectLexicon::compose

> [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.compose-fn]
> Computes the cross product of a lexicon transition set against a rule
> transition set, emitting composed transitions from `origin`. Reads (but does
> not use) the state pair via `get_pair(origin)` (assigned to a discarded
> local). Iterates over each lexicon transition `it` in `lex_transitions`, and
> for each, iterates over every rule transition `jt` in `rule_transitions`.
> For each (it, jt) pair, calls `add_transition` with: `origin`; input label
> `it->ilabel` (the lexicon side input); output label `jt->olabel` (the rule
> side output); weight `it->weight + jt->weight`; and target state obtained from
> `get_state(StatePair(it->target, jt->target))` (default
> `allow_lexicon_epsilons`, which is `true`). No return value.

> [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.compose-intersect-lexicon-fn]
> ComposeIntersectLexicon::ComposeIntersectLexicon

> [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.compose-intersect-lexicon-fn]
> Constructors. The primary one takes a `const HfstBasicTransducer &t` and
> delegates to the base-class constructor `ComposeIntersectFst(t, false)` (the
> `false` argument indicating this is the lexicon, not a rule); its own body is
> empty. A second default constructor delegates to `ComposeIntersectFst()` with
> an empty body. No additional initialization.

> [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.compose-with-rules-fn]
> HfstBasicTransducer ComposeIntersectLexicon::compose_with_rules

> [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.compose-with-rules-fn]
> Top-level entry point that composes this lexicon with the given `rules`
> (`ComposeIntersectRule *`) and returns the resulting `HfstBasicTransducer`
> (by value). Steps: (1) call `clear_all_info()` to reset working state; (2)
> build the start state pair `start_pair = StatePair(START,
> ComposeIntersectRule::START)`; (3) call `map_state_and_add_to_agenda(start_pair,
> true)` to seed the agenda (return value discarded — it is guaranteed to be
> state 0); (4) return `compute_composition_result(rules)`, which drains the
> agenda and finalizes weights.

> [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.compute-state-fn]
> void ComposeIntersectLexicon::compute_state

> [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.compute-state-fn]
> Expands a single result state `state` by composing the outgoing lexicon
> transitions of its lexicon component against the `rules`. Parameters: `state`,
> `rules` (`ComposeIntersectRule *`), and `allow_lexicon_epsilons` (bool).
> Steps: (1) recover the state pair `p = get_pair(state)`, where `p.first` is
> the lexicon state and `p.second` the rule state. (2) Iterate over each entry
> `it` in `transition_map_vector[p.first]` (a `SymbolTransitionMap` mapping a
> symbol number `it->first` to its `TransitionSet` `it->second`):
> - If `it->first` is the epsilon symbol number (number of
>   `"@_EPSILON_SYMBOL_@"`): only if `allow_lexicon_epsilons` is true, call
>   `lexicon_skip_symbol_compose(it->second, p.second, state)`; otherwise skip.
> - Else if `it->first` is a flag diacritic (`is_flag_diacritic(it->first)`)
>   AND `rules->known_symbol(it->first)` is false: call
>   `lexicon_skip_symbol_compose(it->second, p.second, state)`.
> - Otherwise: call `compose(it->second, rules->get_transitions(p.second,
>   it->first), state)`, composing this lexicon symbol's transitions against the
>   rule transitions for that same symbol from the rule state.
> (3) Finally, handle rule epsilons: call `rule_skip_symbol_compose(
> rules->get_transitions(p.second, <epsilon symbol number>), p.first, state)`.
> No return value.

> [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.get-pair-fn]
> ComposeIntersectLexicon::StatePair ComposeIntersectLexicon::get_pair

> [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.get-pair-fn]
> Returns the `StatePair` that result state `s` maps to, by indexing
> `pair_vector[s]`. If `s >= pair_vector.size()`, throws `StateNotDefined`
> (via `HFST_THROW(StateNotDefined)`). No mutation.

> [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.get-state-fn]
> HfstState ComposeIntersectLexicon::get_state(const StatePair &p,

> [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.get-state-fn]
> Returns the result state corresponding to state pair `p`, creating it if
> necessary. Parameters: `p` (a `StatePair`) and `allow_lexicon_epsilons`
> (bool, default `true`). If `p` is not yet a key of `state_pair_map`, returns
> `map_state_and_add_to_agenda(p, allow_lexicon_epsilons)` (which allocates a
> new state and enqueues it). Otherwise returns the existing mapping
> `state_pair_map[p]`.

> [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.identity-compose-fn]
> void identity_compose

> [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.identity-compose-fn]
> Declared only in the header (`void identity_compose(const TransitionSet &,
> const HfstBasicTransition &, HfstState)`) with no definition provided anywhere
> in the source tree. It is never defined or called. No behavior to port.

> [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.is-flag-diacritic-fn]
> bool ComposeIntersectLexicon::is_flag_diacritic(size_t symbol)

> [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.is-flag-diacritic-fn]
> Returns whether the given symbol number denotes a flag diacritic. Resolves
> `symbol` (a `size_t`) to its symbol string via
> `HfstTropicalTransducerTransitionData::get_symbol(size_t_to_uint(symbol))`,
> then returns `FdOperation::is_diacritic(symbolStr)`.

> [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.lexicon-skip-symbol-compose-fn]
> void ComposeIntersectLexicon::lexicon_skip_symbol_compose

> [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.lexicon-skip-symbol-compose-fn]
> Emits transitions for a lexicon symbol that the rule side skips (lexicon
> epsilon or rule-unknown flag diacritic): the rule state stays put while the
> lexicon advances. Parameters: `transitions` (the lexicon `TransitionSet`),
> `rule_state` (the fixed rule-side state), `origin` (result source state).
> Iterates over each transition `it` in `transitions`, calling `add_transition(
> origin, it->ilabel, it->olabel, it->weight, get_state(StatePair(it->target,
> rule_state)))`. The lexicon's own input and output labels are preserved, and
> the target pair advances the lexicon component to `it->target` while keeping
> the rule component at `rule_state`. `get_state` is called with its default
> `allow_lexicon_epsilons=true`. No return value.

> [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.map-state-and-add-to-agenda-fn]
> HfstState ComposeIntersectLexicon::map_state_and_add_to_agenda

> [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.map-state-and-add-to-agenda-fn]
> Allocates a fresh result state for the state pair `p`, records the mapping,
> and enqueues it for processing. Parameters: `p` (a `StatePair`) and
> `allow_lexicon_epsilons` (bool — present in the signature but not used in the
> body). Steps: (1) If `p.first == START && p.second ==
> ComposeIntersectRule::START` (the composite start pair), use state `0`;
> otherwise call `result.add_state()` to allocate a new state `s`. (2) Assert
> `s == state_pair_map.size()` (states are numbered densely in insertion order).
> (3) Set `state_pair_map[p] = s`; push `p` onto `pair_vector` (so
> `pair_vector[s] == p`); push `s` onto `agenda`; and insert `s` into
> `lexicon_non_epsilon_states`. (4) Return `s`. Note: every state created here
> is added to `lexicon_non_epsilon_states` regardless of the
> `allow_lexicon_epsilons` argument.

> [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.pair-vector]
> typedef std::vector<StatePair> PairVector

> [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.rule-skip-symbol-compose-fn]
> void ComposeIntersectLexicon::rule_skip_symbol_compose

> [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.rule-skip-symbol-compose-fn]
> Emits transitions for a rule epsilon: the lexicon state stays put while the
> rule side advances. Parameters: `transitions` (the rule `TransitionSet`),
> `lex_state` (the fixed lexicon-side state), `origin` (result source state).
> Iterates over each transition `it` in `transitions`, calling `add_transition(
> origin, it->ilabel, it->olabel, it->weight, get_state(StatePair(lex_state,
> it->target), false))`. The rule transition's input and output labels and
> weight are used directly, and the target pair keeps the lexicon component at
> `lex_state` while advancing the rule component to `it->target`. Crucially
> `get_state` is called with `allow_lexicon_epsilons = false` (note: this flag
> is currently ignored by `map_state_and_add_to_agenda`). No return value.

> [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.set-final-state-weights-fn]
> void ComposeIntersectLexicon::set_final_state_weights

> [spec:hfst:sem:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.set-final-state-weights-fn]
> Assigns final weights to result states that are final in both the lexicon and
> the rules. For each result state index `s` from `0` to `pair_vector.size()-1`:
> let `lexicon_weight = get_final_weight(pair_vector[s].first)` and
> `rules_weight = rules->get_final_weight(pair_vector[s].second)`. If BOTH
> weights are not `+infinity` (i.e. both components are final), call
> `result.set_final_weight(s, lexicon_weight + rules_weight)`. States where
> either component is non-final are left unset (non-final). No return value.

> [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.state-pair]
> typedef std::pair<HfstState,HfstState> StatePair

> [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.state-pair-map]
> typedef std::map<StatePair,HfstState> StatePairMap

> [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.state-queue]
> typedef std::queue<HfstState> StateQueue

> [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.state-set]
> typedef std::set<HfstState> StateSet

> [spec:hfst:def:compose-intersect-lexicon.hfst.implementations.compose-intersect-lexicon.symbol-transition-map]
> typedef ComposeIntersectFst::SymbolTransitionMap SymbolTransitionMap

> [spec:hfst:def:compose-intersect-lexicon.main-fn]
> int main(int argc, char * argv[])

> [spec:hfst:sem:compose-intersect-lexicon.main-fn]
> Unit-test driver compiled only when `MAIN_TEST` is defined. Prints a header
> line to stdout. Builds a tokenizer with multichar symbols `@D.SomeVar.1@`,
> `@R.SomeVar.1@`, `@_IDENTITY_SYMBOL_@`. Constructs a `lexicon`
> `HfstTransducer` (TROPICAL_OPENFST_TYPE) from input/output strings, wraps it
> in a `ComposeIntersectLexicon l`. Builds single-symbol transducers (x, x:a,
> y, z, D) and a `universal` transducer = `(@_IDENTITY_SYMBOL_@ | x | x:a | y |
> z)*` minimized, then re-imported through an `HfstBasicTransducer` after adding
> `"D"` to its alphabet. Constructs right and left context/center rule
> transducers via concatenation, subtraction, and substitution of `"D"` with
> `@_EPSILON_SYMBOL_@`, minimizing along the way, and wraps the resulting
> `right_rule` and `left_rule` in `ComposeIntersectRule` objects. Combines them
> into a `ComposeIntersectRulePair` (left, right), then a further
> `ComposeIntersectRulePair three_rules(some_rule, rules)` where `some_rule`
> wraps `universal`. Calls `l.compose_with_rules(&three_rules)` to get an
> `HfstBasicTransducer lex`, converts it to a TROPICAL_OPENFST_TYPE
> `HfstTransducer`, minimizes it, and prints it to stderr. Prints "ok" to
> stdout and returns 0. Allocates several `ComposeIntersectRule`/
> `ComposeIntersectRulePair` objects with `new` that are not freed.

