# libhfst/src/parsers/variable_src/VariableValues.cc, libhfst/src/parsers/variable_src/VariableValues.h

> [spec:hfst:def:variable-values.variable-values]
> class VariableValues : public std::vector<std::string> {
>   std::string variable;
> }

> [spec:hfst:def:variable-values.variable-values-vector]
> typedef std::vector<VariableValues> VariableValuesVector

> [spec:hfst:def:variable-values.variable-values.begin-fn]
> VariableValues::const_iterator VariableValues::begin(void) const

> [spec:hfst:sem:variable-values.variable-values.begin-fn]
> Const member function with no parameters. Constructs and returns a
> `const_iterator` (i.e. a `ConstVariableValueIterator`) built from two
> arguments: this object's `variable` string field, and the underlying
> `std::vector<std::string>::begin()` iterator (the begin iterator of the
> base-class vector of value strings). It pairs the variable name with the
> position of the first value. No state is read or mutated beyond reading
> `variable` and the base vector; no side effects.

> [spec:hfst:def:variable-values.variable-values.const-iterator]
> typedef ConstVariableValueIterator const_iterator

> [spec:hfst:def:variable-values.variable-values.end-fn]
> VariableValues::const_iterator VariableValues::end(void) const

> [spec:hfst:sem:variable-values.variable-values.end-fn]
> Const member function with no parameters. Constructs and returns a
> `const_iterator` (i.e. a `ConstVariableValueIterator`) built from this
> object's `variable` string field and the underlying
> `std::vector<std::string>::end()` iterator (the past-the-end iterator of the
> base-class vector of value strings). It pairs the variable name with the
> end position. No state is read or mutated beyond reading `variable` and the
> base vector; no side effects.

> [spec:hfst:def:variable-values.variable-values.set-variable-fn]
> void VariableValues::set_variable(const std::string &variable)

> [spec:hfst:sem:variable-values.variable-values.set-variable-fn]
> Setter that takes one parameter `variable`, a const reference to a
> `std::string`. It assigns (copies) that string into this object's
> `variable` member field, replacing any previous value. Returns nothing.
> No other state is touched and there are no side effects.

