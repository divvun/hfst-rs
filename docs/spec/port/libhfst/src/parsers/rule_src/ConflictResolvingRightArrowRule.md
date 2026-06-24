# libhfst/src/parsers/rule_src/ConflictResolvingRightArrowRule.cc, libhfst/src/parsers/rule_src/ConflictResolvingRightArrowRule.h

> [spec:hfst:def:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule]
> class ConflictResolvingRightArrowRule : public RightArrowRule {
>   SymbolPair center_pair;
> }

> [spec:hfst:def:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule-fn]
> ConflictResolvingRightArrowRule::ConflictResolvingRightArrowRule

> [spec:hfst:sem:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule-fn]
> Constructor. Parameters: `name` (string), `center` (a `SymbolPair`), and
> `contexts` (an `OtherSymbolTransducerVector`).
> Delegates to the base-class `RightArrowRule` constructor, passing `name`,
> the result of `get_center(center.first, center.second)`, and `contexts`.
> (`get_center` is an inherited helper that builds the center transducer from
> the input symbol `center.first` and output symbol `center.second`.)
> After the base subobject is constructed, initialises the member
> `center_pair` to a copy of `center`.
> Body is empty; no other side effects.

> [spec:hfst:def:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule.conflicts-this-fn]
> bool ConflictResolvingRightArrowRule::conflicts_this

> [spec:hfst:sem:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule.conflicts-this-fn]
> Predicate. Takes `another`, a mutable reference to another
> `ConflictResolvingRightArrowRule`. Returns `true` iff this rule's
> `center_pair` equals `another`'s `center_pair` componentwise, i.e. both
> `center_pair.first == another.center_pair.first` and
> `center_pair.second == another.center_pair.second` hold; otherwise `false`.
> Reads only the two `center_pair` members; mutates no state and has no side
> effects.

> [spec:hfst:def:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule.resolve-conflict-fn]
> void ConflictResolvingRightArrowRule::resolve_conflict

> [spec:hfst:sem:conflict-resolving-right-arrow-rule.conflict-resolving-right-arrow-rule.resolve-conflict-fn]
> Mutates this rule to merge it with `another` (a mutable reference to another
> `ConflictResolvingRightArrowRule`). Operates on the inherited `context`
> member (an `OtherSymbolTransducer`).
> First applies `HfstTransducer::disjunct` to `context` with `another.context`
> as the argument (union of the two context languages), then chains an
> application of `HfstTransducer::minimize` to the result. `apply` mutates
> `context` in place and returns a reference to it, enabling the chaining.
> Then appends `" and " + another.name` to the inherited `name` member.
> No return value; side effects are the in-place mutation of `context` and
> `name`. Does not modify `center_pair` or `another`.

> [spec:hfst:def:conflict-resolving-right-arrow-rule.main-fn]
> int main(void)

> [spec:hfst:sem:conflict-resolving-right-arrow-rule.main-fn]
> Test entry point, compiled only when `TEST_CONFLICT_RESOLVING_RIGHT_ARROW_RULE`
> is defined. Exercises the class:
> 1. Builds a `HandySet<SymbolPair>` containing pairs ("a","b"), ("a","c"),
>    ("d","e") and registers it via `OtherSymbolTransducer::set_symbol_pairs`.
> 2. Sets the transducer type to `hfst::TROPICAL_OPENFST_TYPE` via
>    `OtherSymbolTransducer::set_transducer_type`.
> 3. Constructs helper `OtherSymbolTransducer`s: `unknown` (`TWOLC_UNKNOWN`)
>    then `repeat_star`-ed; `diamond` (`TWOLC_DIAMOND`); `a_sth` ("a",
>    `TWOLC_UNKNOWN`); `a_c` ("a","c").
> 4. Builds `context1` starting from `unknown` and concatenating, in order,
>    `a_sth`, `diamond`, `unknown`, `diamond`, `unknown`; wraps it in a
>    one-element `OtherSymbolTransducerVector v1`; constructs `rule1` with name
>    `__TWOLC_RULE_NAME="test rule"`, center `SymbolPair("a","b")`, contexts
>    `v1`.
> 5. Builds `context2` similarly but with `a_c` in place of `a_sth`; wraps it
>    in `v2`; constructs `rule2` with the same name and center, contexts `v2`.
> 6. Asserts `rule1.conflicts_this(rule2)` is true, then calls
>    `rule1.resolve_conflict(rule2)`. The final compile/print line is commented
>    out.
> Returns nothing meaningful (implicitly 0); aborts via `assert` on failure.

