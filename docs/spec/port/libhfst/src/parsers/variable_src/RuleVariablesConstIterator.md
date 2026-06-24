# libhfst/src/parsers/variable_src/RuleVariablesConstIterator.cc, libhfst/src/parsers/variable_src/RuleVariablesConstIterator.h

> [spec:hfst:def:rule-variables-const-iterator.rule-variables-const-iterator]
> class RuleVariablesConstIterator {
>   FreelyVariableBlockConstIterator f_it;
>   MatchedVariableBlockConstIterator ma_it;
>   MixedVariableBlockConstIterator mi_it;
>   FreelyVariableBlockConstIterator f_begin;
>   MatchedVariableBlockConstIterator ma_begin;
>   MixedVariableBlockConstIterator mi_begin;
>   FreelyVariableBlockConstIterator f_end;
>   MatchedVariableBlockConstIterator ma_end;
>   MixedVariableBlockConstIterator mi_end;
>   RuleVariablesConstIterator &operator= (const RuleVariablesConstIterator &another);
> }

> [spec:hfst:def:rule-variables-const-iterator.rule-variables-const-iterator.begin-fn]
> RuleVariablesConstIterator RuleVariablesConstIterator::begin

> [spec:hfst:sem:rule-variables-const-iterator.rule-variables-const-iterator.begin-fn]
> Static factory. Constructs a default RuleVariablesConstIterator `it`, then
> positions its three current-position iterators at the start of each block
> collection in `rv`: `it.f_it = rv.freely_blocks.begin()`,
> `it.ma_it = rv.matched_blocks.begin()`, `it.mi_it = rv.mixed_blocks.begin()`.
> Then calls `it.set_begin_and_end(rv)` to record the begin/end bounds of all
> three collections, and returns `it` by value. Reads `rv` only; mutates nothing
> outside the returned local iterator.

> [spec:hfst:def:rule-variables-const-iterator.rule-variables-const-iterator.end-fn]
> RuleVariablesConstIterator RuleVariablesConstIterator::end

> [spec:hfst:sem:rule-variables-const-iterator.rule-variables-const-iterator.end-fn]
> Static factory. Constructs a default RuleVariablesConstIterator `it`, then sets
> its three current-position iterators to the past-the-end position of each
> collection in `rv`: `it.f_it = rv.freely_blocks.end()`,
> `it.ma_it = rv.matched_blocks.end()`, `it.mi_it = rv.mixed_blocks.end()`.
> Then calls `it.set_begin_and_end(rv)` to record the begin/end bounds of all
> three collections, and returns `it` by value. Reads `rv` only; mutates nothing
> outside the returned local iterator.

> [spec:hfst:def:rule-variables-const-iterator.rule-variables-const-iterator.operator-fn]
> void RuleVariablesConstIterator::operator++(void)

> [spec:hfst:sem:rule-variables-const-iterator.rule-variables-const-iterator.operator-fn]
> Pre-increment. Advances the three nested current-position iterators as an
> odometer-style counter, with `f_it` (freely) the fastest digit, then `ma_it`
> (matched), then `mi_it` (mixed) the slowest. Logic:
> - If `f_it + 1 == f_end` (the freely iterator is at its last element):
>   - If `ma_it + 1 == ma_end` (matched also at its last element):
>     - If `mi_it + 1 == mi_end` (mixed also at its last element): this is the
>       last combination; set `f_it = f_end`, `ma_it = ma_end`, `mi_it = mi_end`
>       (the end/done state, equal to `end(rv)`'s positions) and `return`.
>     - Else: `++mi_it` (advance mixed by one).
>     - Then reset `ma_it = ma_begin`.
>   - Else: `++ma_it` (advance matched by one).
>   - Then reset `f_it = f_begin`.
> - Else: `++f_it` (advance freely by one).
> Mutates the iterator's f_it/ma_it/mi_it in place; reads the stored
> begin/end bounds. Returns void. Note it uses `it + 1 == end` comparisons rather
> than `it == end`, so each block collection is assumed to be non-empty.

> [spec:hfst:def:rule-variables-const-iterator.rule-variables-const-iterator.set-begin-and-end-fn]
> void RuleVariablesConstIterator::set_begin_and_end(const RuleVariables &rv)

> [spec:hfst:sem:rule-variables-const-iterator.rule-variables-const-iterator.set-begin-and-end-fn]
> Records the bounds of `rv`'s three block collections into this iterator's
> stored fields: `f_begin = rv.freely_blocks.begin()`,
> `ma_begin = rv.matched_blocks.begin()`, `mi_begin = rv.mixed_blocks.begin()`,
> `f_end = rv.freely_blocks.end()`, `ma_end = rv.matched_blocks.end()`,
> `mi_end = rv.mixed_blocks.end()`. Reads `rv`; mutates only `this`. Returns void.

> [spec:hfst:def:rule-variables-const-iterator.rule-variables-const-iterator.set-values-fn]
> void RuleVariablesConstIterator::set_values(VariableValueMap &vvm)

> [spec:hfst:sem:rule-variables-const-iterator.rule-variables-const-iterator.set-values-fn]
> Forwards the current values of all three nested iterators into the
> VariableValueMap `vvm`. Calls `f_it.set_values(&vvm)`, then
> `ma_it.set_values(&vvm)`, then `mi_it.set_values(&vvm)`, each passing the
> address of `vvm` so each block iterator writes its current variable bindings
> into the shared map. Mutates `vvm` via those delegated calls; returns void.

