# libhfst/src/parsers/variable_src/VariableDefs.h

> [spec:hfst:def:variable-defs.empty-container]
> class EmptyContainer

> [spec:hfst:def:variable-defs.index-set]
> typedef HandySet<size_t> IndexSet

> [spec:hfst:def:variable-defs.matcher]
> enum Matcher {
>   FREELY;
>   MATCHED;
>   MIXED;
> }

> [spec:hfst:def:variable-defs.unequal-set-size]
> class UnequalSetSize

> [spec:hfst:def:variable-defs.variable-value-map]
> typedef HandyMap<std::string,std::string> VariableValueMap

