# libhfst/src/parsers/variable_src/RuleSymbolVector.cc, libhfst/src/parsers/variable_src/RuleSymbolVector.h

> [spec:hfst:def:rule-symbol-vector.rule-center]
> typedef std::pair<std::string, std::string> RuleCenter

> [spec:hfst:def:rule-symbol-vector.rule-symbol-vector]
> class RuleSymbolVector : public std::vector<std::string> {
>   const VariableValueMap &vvm;
>   RuleSymbolVector &push_back(const std::string &s);
>   RuleSymbolVector &push_back(const std::vector<std::string> &v);
> }

> [spec:hfst:def:rule-symbol-vector.rule-symbol-vector.replace-variables-fn]
> std::string RuleSymbolVector::replace_variables(const RuleCenter &center)

> [spec:hfst:sem:rule-symbol-vector.rule-symbol-vector.replace-variables-fn]
> Builds and returns a single space-joined string from the symbol vector (this
> object is a `std::vector<std::string>`), expanding rule-name and rule-center
> placeholders and substituting variable values.
> Initialize `result` to an empty string. Iterate over every element of the
> vector in order; let `symbol` be a mutable copy of the current element.
> If `symbol` contains the substring `"__HFST_TWOLC_RULE_NAME"` (anywhere,
> via find != npos):
>   - If `vvm` is non-empty, insert the literal string
>     `"__HFST_TWOLC_SPACE" "SUBCASE:"` (i.e. `"__HFST_TWOLC_SPACESUBCASE:"`)
>     into `symbol` at position `symbol.size()-1` (just before the last
>     character).
>   - Then for each (key, value) pair in `vvm` in iteration order, insert the
>     string `"__HFST_TWOLC_SPACE" + key + "=" + value` into `symbol` at
>     position `symbol.size()-1` (recomputed each time, so each insertion goes
>     just before the current last character, accumulating before that final
>     char).
> Else if `symbol` equals exactly `"__HFST_TWOLC_RULE_CENTER"`, replace
> `symbol` with `center.first + " __HFST_TWOLC_: " + center.second` (using the
> two halves of the `center` parameter).
> After this transformation, append to `result`: if `vvm.has_key(symbol)` then
> `vvm.get_value(symbol)`, otherwise `symbol`; followed in either case by a
> single space `" "`.
> Return `result`. Does not mutate the vector or `vvm`; no I/O, no exceptions
> beyond those from the underlying map/string operations.

> [spec:hfst:def:rule-symbol-vector.rule-symbol-vector.rule-symbol-vector-fn]
> RuleSymbolVector::RuleSymbolVector(const VariableValueMap &vvm)

> [spec:hfst:sem:rule-symbol-vector.rule-symbol-vector.rule-symbol-vector-fn]
> Constructor. Takes a `const VariableValueMap &vvm` and stores it in the
> object's `vvm` reference member (initializer list `vvm(vvm)`). The body is
> empty. Performs no other initialization; the vector base starts empty. No
> side effects.

