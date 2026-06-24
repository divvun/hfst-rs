# libhfst/src/implementations/compose_intersect/ComposeIntersectRule.cc, libhfst/src/implementations/compose_intersect/ComposeIntersectRule.h

> [spec:hfst:def:compose-intersect-rule.hfst.implementations.compose-intersect-rule]
> class ComposeIntersectRule : public ComposeIntersectFst {
>   StringSet symbols;
> }

> [spec:hfst:def:compose-intersect-rule.hfst.implementations.compose-intersect-rule.compose-intersect-rule-fn]
> ComposeIntersectRule::ComposeIntersectRule(const HfstBasicTransducer &t)

> [spec:hfst:sem:compose-intersect-rule.hfst.implementations.compose-intersect-rule.compose-intersect-rule-fn]
> Constructs a ComposeIntersectRule from a single HfstBasicTransducer `t`.
> Delegates to the base-class ComposeIntersectFst constructor invoked as
> `ComposeIntersectFst(t, true)` (the second argument, true, indicates this
> Fst is a rule). After base construction, sets the member `symbols` to the
> transducer's alphabet by calling `t.get_alphabet()`, which returns the
> StringSet of symbols declared on `t`. No I/O, no exceptions thrown here.
> There is also a separate default constructor `ComposeIntersectRule(void)`
> which simply delegates to the default `ComposeIntersectFst()` constructor
> and leaves `symbols` empty.

> [spec:hfst:def:compose-intersect-rule.hfst.implementations.compose-intersect-rule.known-symbol-fn]
> bool ComposeIntersectRule::known_symbol(size_t symbol)

> [spec:hfst:sem:compose-intersect-rule.hfst.implementations.compose-intersect-rule.known-symbol-fn]
> Returns whether the given numeric `symbol` (a size_t symbol number) is part
> of this rule's alphabet. Steps: convert `symbol` from size_t to unsigned int
> via `hfst::size_t_to_uint(symbol)`; map that number to its string name using
> the static `HfstTropicalTransducerTransitionData::get_symbol(...)`; then look
> that string up in the `symbols` StringSet with `symbols.count(...)`. Returns
> true if the count is greater than 0 (i.e. the symbol string is present in
> `symbols`), false otherwise. No mutation of state, no I/O.

> [spec:hfst:def:compose-intersect-rule.main-fn]
> int main(int argc, char * argv[])

> [spec:hfst:sem:compose-intersect-rule.main-fn]
> Compiled only when the MAIN_TEST macro is defined. A trivial unit-test stub:
> prints `Unit tests for <__FILE__>:` followed by a newline to std::cout, then
> prints `ok` and a newline, and returns 0. Ignores `argc`/`argv`; performs no
> actual assertions or tests.

