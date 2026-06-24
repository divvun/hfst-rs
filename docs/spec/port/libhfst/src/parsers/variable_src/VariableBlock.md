# libhfst/src/parsers/variable_src/VariableBlock.h

> [spec:hfst:def:variable-block.freely-variable-block]
> typedef VariableBlock<ConstContainerIterator<VariableValues> >

> [spec:hfst:def:variable-block.matched-variable-block]
> typedef

> [spec:hfst:def:variable-block.mixed-variable-block]
> typedef VariableBlock<MixedConstContainerIterator<VariableValues> >

> [spec:hfst:def:variable-block.variable-block]
> class VariableBlock

> [spec:hfst:def:variable-block.variable-block.variable-block-fn]
> VariableBlock(const VariableValuesVector &v)

> [spec:hfst:sem:variable-block.variable-block.variable-block-fn]
> Constructor taking a `const VariableValuesVector &v`. First invokes the
> base `VariableContainer<VariableValues,IT>()` default constructor (empty
> container). Then iterates over `v` from `v.begin()` to `v.end()` with a
> const_iterator `it`. For each element: if `it->empty()` is true (the
> contained `VariableValues` is empty), throws `EmptyContainer()`. Otherwise
> calls `VariableContainer<VariableValues,IT>::add_object(*it)`, appending a
> copy of that element into the container. After the loop the constructed
> block holds every non-empty element of `v` in order; if any element was
> empty, construction aborts via the thrown exception (no value returned).

