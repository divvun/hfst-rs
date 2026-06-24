# libhfst/src/implementations/compose_intersect/ComposeIntersectUtilities.cc, libhfst/src/implementations/compose_intersect/ComposeIntersectUtilities.h

> [spec:hfst:def:compose-intersect-utilities.cmp-int]
> struct CmpInt

> [spec:hfst:def:compose-intersect-utilities.cmp-int.operator-fn]
> bool operator() (int i, int j) const

> [spec:hfst:sem:compose-intersect-utilities.cmp-int.operator-fn]
> A const function-call operator on `CmpInt` taking two `int` arguments `i` and `j`.
> Returns `true` iff `i < j` (strict less-than ordering of integers); otherwise `false`.
> Pure, no state, no side effects. Serves as the comparator type `C` for the integer
> instantiation `IntSpaceSavingSet`.

> [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set]
> class SpaceSavingSet {
>   static C comparator;
>   static struct ReverseCompare { // [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.reverse-compar...;
>   XVector container_;
> }

> [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.add-value-fn]
> void add_value(const X &x,iterator least_upper_bound)

> [spec:hfst:sem:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.add-value-fn]
> Protected helper. Inserts a copy of `x` into `container_` immediately before the
> position given by the `iterator least_upper_bound` (i.e. `container_.insert(least_upper_bound, x)`),
> shifting all later elements one position to the right. The caller passes the position
> returned by `get_least_upper_bound(x)`, so the element lands at the spot that keeps the
> vector sorted in ascending `comparator` order. Mutates `container_`; may reallocate and
> invalidate existing iterators. Returns nothing.

> [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.begin-fn]
> const_iterator begin(void) const

> [spec:hfst:sem:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.begin-fn]
> Const accessor. Returns `container_.begin()`, a `const_iterator` to the first element
> of the underlying sorted vector (or equal to `end()` if empty). No mutation, no side
> effects. (A non-const overload returns a mutable `iterator`.)

> [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.clear-fn]
> void clear(void)

> [spec:hfst:sem:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.clear-fn]
> Calls `container_.clear()`, removing all elements so the set becomes empty (`size()` becomes 0).
> Mutates `container_`; invalidates any outstanding iterators. Returns nothing.

> [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.const-iterator]
> typedef typename XVector::const_iterator const_iterator

> [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.end-fn]
> const_iterator end(void) const

> [spec:hfst:sem:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.end-fn]
> Const accessor. Returns `container_.end()`, a `const_iterator` one past the last element
> of the underlying vector (the past-the-end sentinel). No mutation, no side effects.
> (A non-const overload returns a mutable `iterator`.)

> [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.find-fn]
> const_iterator find(const X &x) const

> [spec:hfst:sem:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.find-fn]
> Const lookup of `x`. Computes `least_upper_bound = get_least_upper_bound(x)` (the first
> position whose element is not strictly less than `x`). If that equals `end()`, returns `end()`.
> Otherwise dereferences it to `new_x = *least_upper_bound`; if `new_x != x`, returns `end()`
> (the candidate is strictly greater, so `x` is absent). Otherwise the candidate equals `x`,
> and that iterator is returned. No mutation. Equality/inequality use `X`'s `operator==`/`operator!=`.

> [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.get-least-upper-bound-fn]
> const_iterator get_least_upper_bound(const X &x) const

> [spec:hfst:sem:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.get-least-upper-bound-fn]
> Protected helper performing a linear scan. Starts `it` at `container_.begin()` and advances
> one element at a time while `comparator(*it, x)` is true (i.e. while the current element is
> strictly less than `x`); breaks at the first element for which `comparator(*it, x)` is false,
> i.e. the first element that is `>= x` under the comparator. Returns that iterator (which is
> `end()` if every element is strictly less than `x`, or if the container is empty). Assumes
> `container_` is kept sorted in ascending `comparator` order, so this is the lower-bound
> position. No mutation. Both a `const_iterator` and a non-const `iterator` overload exist
> with identical logic.

> [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.has-element-fn]
> bool has_element(const X &x) const

> [spec:hfst:sem:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.has-element-fn]
> Const membership test. Returns `find(x) != end()`, i.e. `true` iff `x` is present in the set.
> No mutation, no side effects.

> [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.insert-fn]
> void insert(const X &x)

> [spec:hfst:sem:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.insert-fn]
> Inserts `x`, keeping the container sorted and free of duplicates. Computes
> `least_upper_bound = get_least_upper_bound(x)` (the non-const overload, the first position
> with element `>= x`). Reads `new_x = *least_upper_bound`. If `least_upper_bound == end()`
> OR `!(x == new_x)` (i.e. the lower-bound position is the end, or its element is not equal
> to `x`), calls `add_value(x, least_upper_bound)` to insert `x` at that position; otherwise
> `x` already exists and nothing is inserted. Returns nothing. Note: when
> `least_upper_bound == end()` the dereference `*least_upper_bound` is evaluated before the
> guard; a faithful Rust port must read the candidate only when the position is valid and treat
> the end case as "not equal, insert at end".

> [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.iterator]
> typedef typename XVector::iterator iterator

> [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.reverse-compare]
> struct ReverseCompare

> [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.reverse-compare.operator-fn]
> bool operator() (const X &x1,const X &x2) const

> [spec:hfst:sem:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.reverse-compare.operator-fn]
> Const function-call operator on the static `ReverseCompare` struct, taking two `X` values
> `x1` and `x2`. Returns `comparator()(x1, x2)` — it default-constructs a temporary `C` from
> the static `comparator` member by calling it with `()` and then invokes that temporary's
> own call operator on `(x1, x2)`. (Note: the despite-the-name "reverse" comparator simply
> forwards to a freshly-default-constructed comparator, yielding the same ordering as `C`.)
> No mutation, no side effects.

> [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.size-fn]
> size_t size(void) const

> [spec:hfst:sem:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.size-fn]
> Const accessor returning `container_.size()`, the number of elements currently stored in
> the set. No mutation, no side effects.

> [spec:hfst:def:compose-intersect-utilities.hfst.implementations.compose-intersect-utilities.space-saving-set.x-vector]
> typedef std::vector<X> XVector

> [spec:hfst:def:compose-intersect-utilities.int-space-saving-set]
> typedef

> [spec:hfst:def:compose-intersect-utilities.main-fn]
> int main(int argc, char * argv[])

> [spec:hfst:sem:compose-intersect-utilities.main-fn]
> Unit-test entry point compiled only when `MAIN_TEST` is defined. Prints
> `"Unit tests for <file>:"` to stdout, then exercises `IntSpaceSavingSet`
> (`SpaceSavingSet<int,CmpInt>`) via a series of `assert`s:
> - A fresh `sset` has `size()==0` and `!has_element(0)`.
> - After `insert(0)`: `size()==1`, `has_element(0)`.
> - After `insert(1)`: `size()==2`, both 0 and 1 present.
> - Inserting duplicates (`1,1,0`) and new values (`2,4,3`) leaves all of 0..4 present
>   (duplicates do not grow the set; out-of-order inserts still end up findable, i.e. sorted).
> - `clear()` empties it (`size()==0`, `!has_element(0)`).
> - Inserts 0 then 1, iterates from `begin()`: first element `*jt==0`, next `*jt==1`, then `jt==end()`,
>   confirming ascending order.
> - `clear()` again, inserts in reverse (1 then 0): iteration still yields 0 then 1 then end,
>   confirming the container stays sorted regardless of insertion order.
> - A stress loop runs 100000 times, each iteration heap-allocating a new `IntSpaceSavingSet`,
>   inserting 0 and 1, and deleting it (checks no leaks/crashes).
> - Finally builds a fresh set, `clear()`, `insert(2)`, asserts `!has_element(1)`.
> Prints `"ok"` and returns 0. Side effects: stdout writes, heap allocation, and `assert`
> aborts the process on any failed invariant.

