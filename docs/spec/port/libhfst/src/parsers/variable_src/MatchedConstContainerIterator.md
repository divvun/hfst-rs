# libhfst/src/parsers/variable_src/MatchedConstContainerIterator.h

> [spec:hfst:def:matched-const-container-iterator.matched-const-container-iterator]
> class MatchedConstContainerIterator

> [spec:hfst:def:matched-const-container-iterator.matched-const-container-iterator.matched-const-container-iterator-fn]
> MatchedConstContainerIterator(const ConstContainerIterator<T> &another)

> [spec:hfst:sem:matched-const-container-iterator.matched-const-container-iterator.matched-const-container-iterator-fn]
> Constructor that builds a MatchedConstContainerIterator from a base-class
> ConstContainerIterator<T> instance `another`.
> Steps:
> 1. Copy-assign `another` into this instance via the base class assignment
>    operator `ConstContainerIterator<T>::operator=(another)`, copying all
>    inherited member state (the begin/end/iterator vectors, etc.).
> 2. Compute `set_sizes`: if `begin_iterator_vector` is empty (size == 0),
>    `set_sizes` is 0; otherwise it is `end_iterator_vector[0] -
>    begin_iterator_vector[0]` (the size of the first variable's value set).
> 3. Loop over every index `i` from 0 up to `begin_iterator_vector.size()`.
>    For each `i`, if `end_iterator_vector[i] - begin_iterator_vector[i]`
>    is not equal to `set_sizes`, throw an `UnequalSetSize` exception.
> 4. If all sets have equal size, construction completes normally. The
>    effect is to enforce that all variables in a MATCHED block range over
>    value sets of identical size; otherwise it throws.

> [spec:hfst:def:matched-const-container-iterator.matched-const-container-iterator.operator-fn]
> int operator++(void)

> [spec:hfst:sem:matched-const-container-iterator.matched-const-container-iterator.operator-fn]
> Prefix increment operator. Advances the iterator one step over a MATCHED
> block, where all variables move in lockstep.
> Steps:
> 1. Loop over every index `i` from 0 up to `iterator_vector.size()`.
> 2. For each `i`, pre-increment the element at that position:
>    `++iterator_vector.at(i)` (using `.at()`, which is bounds-checked).
>    This advances every variable's current position by one simultaneously.
> 3. Return the int `1`.
> No bounds/end checking against the end_iterator_vector is performed here;
> the caller is responsible for stopping iteration.

