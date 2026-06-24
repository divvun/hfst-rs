# libhfst/src/parsers/variable_src/RuleVariables.cc, libhfst/src/parsers/variable_src/RuleVariables.h

> [spec:hfst:def:rule-variables.rule-variables]
> class RuleVariables {
>   FreelyVariableBlockContainer freely_blocks;
>   MatchedVariableBlockContainer matched_blocks;
>   MixedVariableBlockContainer mixed_blocks;
>   VariableValuesVector current_variable_block;
> }

> [spec:hfst:def:rule-variables.rule-variables.add-value-fn]
> void RuleVariables::add_value(const std::string &value)

> [spec:hfst:sem:rule-variables.rule-variables.add-value-fn]
> Appends `value` as a new value to the most recently started variable in
> the current block. If `current_variable_block` is empty (no variable has
> been started via `set_variable`), throw `EmptyContainer`. Otherwise take
> the last `VariableValues` element of `current_variable_block` and
> `push_back(value)` onto it. No return value.

> [spec:hfst:def:rule-variables.rule-variables.add-values-fn]
> void RuleVariables::add_values(const std::vector<std::string> &values)

> [spec:hfst:sem:rule-variables.rule-variables.add-values-fn]
> Appends each string in `values` (in order) to the most recently started
> variable. If `current_variable_block` is empty, throw `EmptyContainer`
> first (before adding anything). Otherwise iterate over `values` from
> begin to end and call `add_value` on each element (so each is pushed onto
> the last `VariableValues` of `current_variable_block`). No return value.
> Note: when `values` is empty and the container is empty, the exception is
> still thrown because the emptiness check happens before the loop.

> [spec:hfst:def:rule-variables.rule-variables.begin-fn]
> RuleVariables::const_iterator RuleVariables::begin(void) const

> [spec:hfst:sem:rule-variables.rule-variables.begin-fn]
> Returns a `const_iterator` (a `RuleVariablesConstIterator`) positioned at
> the beginning of this `RuleVariables`. Implemented by delegating to the
> static factory `RuleVariablesConstIterator::begin(*this)`, passing this
> object by const reference. Const method; reads no mutable state.

> [spec:hfst:def:rule-variables.rule-variables.clear-fn]
> void RuleVariables::clear(void)

> [spec:hfst:sem:rule-variables.rule-variables.clear-fn]
> Resets the three block containers by calling `clear()` on each of
> `freely_blocks`, `matched_blocks`, and `mixed_blocks` in that order. Does
> NOT touch `current_variable_block`. No return value.

> [spec:hfst:def:rule-variables.rule-variables.const-iterator]
> typedef RuleVariablesConstIterator const_iterator

> [spec:hfst:def:rule-variables.rule-variables.empty-fn]
> bool RuleVariables::empty(void) const

> [spec:hfst:sem:rule-variables.rule-variables.empty-fn]
> Returns true iff all three block containers are empty. Computes
> emptiness of each container by comparing its `begin()` to its `end()`
> iterator, and returns the logical AND of the three comparisons: i.e.
> `freely_blocks.begin() == freely_blocks.end()` AND the same for
> `matched_blocks` AND for `mixed_blocks`. Const method; reads no mutable
> state and does not consider `current_variable_block`.

> [spec:hfst:def:rule-variables.rule-variables.end-fn]
> RuleVariables::const_iterator RuleVariables::end(void) const

> [spec:hfst:sem:rule-variables.rule-variables.end-fn]
> Returns a `const_iterator` (a `RuleVariablesConstIterator`) positioned at
> the end (past-the-last) of this `RuleVariables`. Implemented by
> delegating to the static factory `RuleVariablesConstIterator::end(*this)`,
> passing this object by const reference. Const method; reads no mutable
> state.

> [spec:hfst:def:rule-variables.rule-variables.set-matcher-fn]
> void RuleVariables::set_matcher(Matcher matcher)

> [spec:hfst:sem:rule-variables.rule-variables.set-matcher-fn]
> Finalizes the current variable block, assigning it to one of the three
> matcher-keyed containers, then clears the current block. Switch on the
> `Matcher` enum argument: `FREELY` -> `freely_blocks.add_object(current_variable_block)`;
> `MATCHED` -> `matched_blocks.add_object(current_variable_block)`;
> `MIXED` -> `mixed_blocks.add_object(current_variable_block)`. (Each
> `add_object` copies/stores the current block into that container.) After
> the switch, unconditionally call `current_variable_block.clear()` so the
> next block starts empty. No return value.

> [spec:hfst:def:rule-variables.rule-variables.set-variable-fn]
> void RuleVariables::set_variable(const std::string &var)

> [spec:hfst:sem:rule-variables.rule-variables.set-variable-fn]
> Starts a new variable in the current block. Constructs a fresh
> `VariableValues` `vv`, calls `vv.set_variable(var)` to record the
> variable name `var`, then `push_back`es `vv` onto
> `current_variable_block`. The newly pushed element becomes the current
> target for subsequent `add_value`/`add_values` calls. No return value.

