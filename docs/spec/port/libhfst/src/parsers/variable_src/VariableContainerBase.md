# libhfst/src/parsers/variable_src/VariableContainerBase.h

> [spec:hfst:def:variable-container-base.variable-container-base]
> class VariableContainerBase {
>   std::vector<T> T_vector;
> }

> [spec:hfst:def:variable-container-base.variable-container-base.add-object-fn]
> void add_object(const T &t)

> [spec:hfst:sem:variable-container-base.variable-container-base.add-object-fn]
> Appends a copy of the object `t` to the end of the protected member
> `T_vector` (i.e. `T_vector.push_back(t)`). No return value, no other
> side effects.

> [spec:hfst:def:variable-container-base.variable-container-base.clear-fn]
> void clear(void)

> [spec:hfst:sem:variable-container-base.variable-container-base.clear-fn]
> Removes all elements from the protected member `T_vector` (i.e.
> `T_vector.clear()`), leaving it empty. No return value, no other side
> effects.

> [spec:hfst:def:variable-container-base.variable-container-base.variable-container-base-fn]
> VariableContainerBase(void)

> [spec:hfst:sem:variable-container-base.variable-container-base.variable-container-base-fn]
> Default constructor with an empty body. It default-initializes the
> protected member `T_vector` to an empty vector. No parameters, no side
> effects.

