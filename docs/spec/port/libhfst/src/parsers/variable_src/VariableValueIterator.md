# libhfst/src/parsers/variable_src/VariableValueIterator.h

> [spec:hfst:def:variable-value-iterator.const-variable-value-iterator]
> typedef VariableValueIterator<std::vector<std::string>::const_iterator>

> [spec:hfst:def:variable-value-iterator.variable-value-iterator]
> class VariableValueIterator {
>   std::string variable;
>   IT it;
> }

> [spec:hfst:def:variable-value-iterator.variable-value-iterator.begin-fn]
> static VariableValueIterator begin(const std::string &variable,

> [spec:hfst:sem:variable-value-iterator.variable-value-iterator.begin-fn]
> Static factory (protected) returning a `VariableValueIterator<IT>`
> positioned at the first element of a value list. Takes `variable` (the
> variable name, by const reference) and `v` (a `const std::vector<std::string>&`
> of the variable's possible values). Constructs and returns
> `VariableValueIterator<IT>(variable, v.begin())`, i.e. a new iterator
> holding a copy of the variable name and an underlying iterator pointing at
> `v.begin()`. No mutation, no side effects.

> [spec:hfst:def:variable-value-iterator.variable-value-iterator.end-fn]
> static VariableValueIterator end(const std::string &variable,

> [spec:hfst:sem:variable-value-iterator.variable-value-iterator.end-fn]
> Static factory (protected) returning a `VariableValueIterator<IT>`
> positioned one past the last element of a value list. Takes `variable` (the
> variable name, by const reference) and `v` (a `const std::vector<std::string>&`).
> Constructs and returns `VariableValueIterator<IT>(variable, v.end())`, i.e. a
> new iterator holding a copy of the variable name and an underlying iterator
> pointing at `v.end()` (the past-the-end sentinel). No mutation, no side effects.

> [spec:hfst:def:variable-value-iterator.variable-value-iterator.operator-fn]
> VariableValueIterator<IT> operator+(size_t i) const

> [spec:hfst:sem:variable-value-iterator.variable-value-iterator.operator-fn]
> `operator+(size_t i) const`: returns a new iterator advanced `i` steps past
> `this`. Copy-constructs a local `VariableValueIterator<IT> vvit` from `*this`
> (copying both `variable` and `it`). Then loops `n` from `0` to `i-1`
> inclusive, calling `++vvit` each iteration (which advances `vvit.it` via the
> prefix increment operator, i.e. `++it`). Returns `vvit` by value. When `i ==
> 0` no increments occur and a copy of `this` is returned. `this` is not
> mutated. Advancing past the underlying container's end is not guarded.

> [spec:hfst:def:variable-value-iterator.variable-value-iterator.set-values-fn]
> void set_values(VariableValueMap * vvm) const

> [spec:hfst:sem:variable-value-iterator.variable-value-iterator.set-values-fn]
> `set_values(VariableValueMap * vvm) const`: assigns this iterator's current
> value into the supplied map under this iterator's variable name. Specifically
> evaluates `vvm->operator[](variable) = *it;` — it dereferences the underlying
> iterator `it` to obtain the current `std::string` value, indexes `*vvm` by
> the member `variable` (a `std::string` key), and assigns the dereferenced
> value to that map entry (inserting the key if absent). Mutates `*vvm`; does
> not mutate `this`. Returns void. `vvm` must be non-null and `it` must be
> dereferenceable (not at end); neither precondition is checked.

> [spec:hfst:def:variable-value-iterator.variable-value-iterator.variable-value-iterator-fn]
> VariableValueIterator(const std::string &variable,const IT &it)

> [spec:hfst:sem:variable-value-iterator.variable-value-iterator.variable-value-iterator-fn]
> Protected two-argument constructor `VariableValueIterator(const std::string
> &variable, const IT &it)`: initializes the member `variable` from the
> `variable` parameter (string copy) and the member `it` from the `it`
> parameter (copy of the underlying iterator), via the member-initializer list.
> The body is empty. No side effects beyond member initialization. Used by the
> static `begin`/`end` factories to build positioned iterators.

