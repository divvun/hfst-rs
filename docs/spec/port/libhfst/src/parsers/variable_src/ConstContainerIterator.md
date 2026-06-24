# libhfst/src/parsers/variable_src/ConstContainerIterator.h

> [spec:hfst:def:const-container-iterator.const-container-iterator]
> class ConstContainerIterator {
>   std::vector<typename T::const_iterator> iterator_vector;
>   std::vector<typename T::const_iterator> begin_iterator_vector;
>   std::vector<typename T::const_iterator> end_iterator_vector;
> }

> [spec:hfst:def:const-container-iterator.const-container-iterator.begin-fn]
> static ConstContainerIterator begin(const TVector &v)

> [spec:hfst:sem:const-container-iterator.const-container-iterator.begin-fn]
> Static factory producing a "begin" iterator over the vector of containers `v`
> (type `TVector` = `std::vector<T>`). Constructs an empty `ConstContainerIterator i`.
> Then iterates over each element `it` of `v` from `v.begin()` to `v.end()` in
> order; for each container `*it` it pushes onto `i`: into `iterator_vector` the
> container's `begin()` iterator, into `begin_iterator_vector` that same
> `begin()` iterator, and into `end_iterator_vector` the container's `end()`
> iterator. After processing all elements, returns `i` by value. The resulting
> iterator's current position equals all begin positions, i.e. the first element
> of the cartesian product over the contained sub-containers.

> [spec:hfst:def:const-container-iterator.const-container-iterator.const-container-iterator-fn]
> ConstContainerIterator(const ConstContainerIterator &another)

> [spec:hfst:sem:const-container-iterator.const-container-iterator.const-container-iterator-fn]
> Copy constructor. Initializes this iterator's three member vectors as copies of
> the corresponding members of `another`: `iterator_vector` from
> `another.iterator_vector`, `begin_iterator_vector` from
> `another.begin_iterator_vector`, and `end_iterator_vector` from
> `another.end_iterator_vector`. Has an empty body; all work is done in the member
> initializer list. No other side effects.

> [spec:hfst:def:const-container-iterator.const-container-iterator.end-fn]
> static ConstContainerIterator end(const TVector &v)

> [spec:hfst:sem:const-container-iterator.const-container-iterator.end-fn]
> Static factory producing an "end" (past-the-end) iterator over the vector of
> containers `v`. Constructs an empty `ConstContainerIterator i`. Iterates over
> each element `it` of `v` from `v.begin()` to `v.end()` in order; for each
> container `*it` it pushes onto `i`: into `iterator_vector` the container's
> `end()` iterator, into `begin_iterator_vector` the container's `begin()`
> iterator, and into `end_iterator_vector` the container's `end()` iterator.
> Returns `i` by value. This differs from `begin` only in that `iterator_vector`
> holds the `end()` positions instead of the `begin()` positions, marking the
> terminal state used for end-of-iteration comparison.

> [spec:hfst:def:const-container-iterator.const-container-iterator.operator-fn]
> virtual int operator++(void)

> [spec:hfst:sem:const-container-iterator.const-container-iterator.operator-fn]
> Prefix increment, advancing the iterator to the next element of the cartesian
> product (an odometer-style / mixed-radix increment). Sets a local flag
> `found_a_non_final_iterator = false`. Loops over positions `i` from 0 up to
> `iterator_vector.size()` (the lowest index is the fastest-changing digit). For
> each `i`: if `iterator_vector[i] + 1 == end_iterator_vector[i]` (i.e. advancing
> would reach this sub-container's end, meaning it is currently at its last
> element), it resets `iterator_vector[i] = begin_iterator_vector[i]` (carry: wrap
> this digit back to its begin) and continues to the next position. Otherwise it
> increments `iterator_vector[i]` in place, sets `found_a_non_final_iterator =
> true`, and breaks out of the loop. After the loop, if no non-final iterator was
> found (every position wrapped, i.e. overflow), it sets `iterator_vector =
> end_iterator_vector`, making this iterator compare equal to the end iterator.
> Always returns the int `1`. Note: each sub-iterator must support `+ 1` and
> equality with the end iterator; positions whose sub-container is empty
> (begin == end) are not specially handled here.

> [spec:hfst:def:const-container-iterator.const-container-iterator.set-values-fn]
> void set_values(VariableValueMap * vvm) const

> [spec:hfst:sem:const-container-iterator.const-container-iterator.set-values-fn]
> Const method that applies the current iterator state to the supplied
> `VariableValueMap * vvm`. Iterates over each element `it` of `iterator_vector`
> from `begin()` to `end()` in order; each such element is itself a
> `T::const_iterator` pointing at the currently selected value of one
> sub-container. For each, it calls `it->set_values(vvm)`, delegating to the
> pointed-to value object to write its variable/value assignment(s) into `vvm`.
> Returns nothing; the only side effect is the mutation of `*vvm` performed by the
> delegated `set_values` calls.

> [spec:hfst:def:const-container-iterator.const-container-iterator.t-iterator-vector]
> typedef std::vector<typename T::const_iterator> TIteratorVector

> [spec:hfst:def:const-container-iterator.const-container-iterator.t-vector]
> typedef std::vector<T> TVector

