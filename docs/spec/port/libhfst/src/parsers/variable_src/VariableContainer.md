# libhfst/src/parsers/variable_src/VariableContainer.h

> [spec:hfst:def:variable-container.variable-container]
> class VariableContainer

> [spec:hfst:def:variable-container.variable-container.begin-fn]
> const_iterator begin(void) const

> [spec:hfst:sem:variable-container.variable-container.begin-fn]
> Returns a `const_iterator` (the template parameter type `IT`) positioned at
> the beginning of the container's underlying variable vector. It is implemented
> by calling the static factory `const_iterator::begin(...)`, passing the
> inherited member `VariableContainerBase<T>::T_vector` (the vector of variable
> values of type `T` held by the base class). The container is not modified
> (const method); the returned iterator delegates begin-positioning logic to
> `IT::begin`.

> [spec:hfst:def:variable-container.variable-container.const-iterator]
> typedef IT const_iterator

> [spec:hfst:def:variable-container.variable-container.end-fn]
> const_iterator end(void) const

> [spec:hfst:sem:variable-container.variable-container.end-fn]
> Returns a `const_iterator` (the template parameter type `IT`) positioned one
> past the last element of the container's underlying variable vector. It is
> implemented by calling the static factory `const_iterator::end(...)`, passing
> the inherited member `VariableContainerBase<T>::T_vector` (the vector of
> variable values of type `T` held by the base class). The container is not
> modified (const method); the returned iterator delegates end-positioning logic
> to `IT::end`.

