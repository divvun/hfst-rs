# libhfst/src/parsers/variable_src/MixedConstContainerIterator.h

> [spec:hfst:def:mixed-const-container-iterator.mixed-const-container-iterator]
> class MixedConstContainerIterator

> [spec:hfst:def:mixed-const-container-iterator.mixed-const-container-iterator.didnt-end-fn]
> bool didnt_end(void)

> [spec:hfst:sem:mixed-const-container-iterator.mixed-const-container-iterator.didnt-end-fn]
> Returns whether the iterator has not yet reached the end. Iterate `i`
> from `0` over the indices of the inherited `iterator_vector`. For each
> `i`, compare element `iterator_vector.at(i)` against the corresponding
> element `end_iterator_vector.at(i)`; if any pair is not equal, return
> `true` immediately. If every position has reached its end iterator
> (all equal), return `false`. Reads `iterator_vector` and
> `end_iterator_vector` (both inherited from `ConstContainerIterator<T>`),
> which are assumed to have the same size. Mutates nothing.

> [spec:hfst:def:mixed-const-container-iterator.mixed-const-container-iterator.equal-indices-fn]
> bool equal_indices(void)

> [spec:hfst:sem:mixed-const-container-iterator.mixed-const-container-iterator.equal-indices-fn]
> Returns whether any two of the current per-position offsets collide.
> Create an empty local `IndexSet` named `index_set`. Iterate `i` from
> `0` over the indices of the inherited `iterator_vector`. For each `i`,
> compute `index` as the offset of the current iterator from its begin
> iterator: `iterator_vector.at(i) - begin_iterator_vector.at(i)` (pointer/
> iterator subtraction yielding a `size_t` distance). If `index_set`
> already contains `index` (`has_element(index)` true), return `true`
> immediately. Otherwise insert `index` into `index_set` and continue.
> If the loop completes with all offsets distinct, return `false`. Reads
> inherited `iterator_vector` and `begin_iterator_vector`; mutates only
> the local `index_set`.

> [spec:hfst:def:mixed-const-container-iterator.mixed-const-container-iterator.mixed-const-container-iterator-fn]
> MixedConstContainerIterator(const ConstContainerIterator<T> &another)

> [spec:hfst:sem:mixed-const-container-iterator.mixed-const-container-iterator.mixed-const-container-iterator-fn]
> Constructs a `MixedConstContainerIterator` from a base-class
> `ConstContainerIterator<T>` instance `another`. First copy-assign the
> full base state from `another` via `ConstContainerIterator<T>::operator=(another)`.
> Then advance past any invalid initial position: while `didnt_end()` is
> true AND `equal_indices()` is true, call `operator++()`. This skips
> combinations where two positions share the same per-position offset, so
> the constructed iterator points at the first valid "mixed" combination
> (or at the end). No return value (constructor).

> [spec:hfst:def:mixed-const-container-iterator.mixed-const-container-iterator.operator-fn]
> int operator++(void)

> [spec:hfst:sem:mixed-const-container-iterator.mixed-const-container-iterator.operator-fn]
> Pre-increment to the next valid "mixed" combination. Run a do/while
> loop: first call the base-class `ConstContainerIterator<T>::operator++()`
> to advance one step, then re-check the condition `didnt_end() &&
> equal_indices()`; repeat the base increment while that condition holds.
> This guarantees at least one base increment and then keeps advancing
> until either the end is reached (`didnt_end()` false) or the current
> per-position offsets are all distinct (`equal_indices()` false). Always
> returns the `int` value `1`.

