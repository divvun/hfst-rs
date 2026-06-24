# libhfst/src/parsers/rule_src/ConflictResolvingLeftArrowRule.cc, libhfst/src/parsers/rule_src/ConflictResolvingLeftArrowRule.h

> [spec:hfst:def:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule]
> class ConflictResolvingLeftArrowRule : public LeftArrowRule {
>   std::string input_symbol;
> }

> [spec:hfst:def:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule-fn]
> ConflictResolvingLeftArrowRule::ConflictResolvingLeftArrowRule

> [spec:hfst:sem:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule-fn]
> Constructor. Parameters: `name` (rule name string), `center` (a
> `SymbolPair`, i.e. a pair of strings `(first, second)` = input:output
> symbols), and `contexts` (an `OtherSymbolTransducerVector`).
> Delegates to the base-class `LeftArrowRule` constructor, passing
> `name`, the transducer returned by `Rule::get_center(center.first,
> center.second)` (the single-symbol-pair center FST built from the
> input and output symbols), and `contexts`. After the base
> initialization, sets the member `input_symbol` to `center.first` (the
> input side of the center pair). The body is empty.

> [spec:hfst:def:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.conflicts-this-fn]
> bool ConflictResolvingLeftArrowRule::conflicts_this

> [spec:hfst:sem:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.conflicts-this-fn]
> Returns `bool`. Parameters: `another` (a const reference to another
> `ConflictResolvingLeftArrowRule`) and `v` (a `StringVector &`, an
> output parameter). Computes `wbize(another.context)` — the
> word-boundary-bracketed form of `another`'s context (see the `wbize`
> rule) — then calls `context.is_empty_intersection(...)` on this rule's
> own `context` member, passing the wbized context and `v`. That call
> reports whether the intersection of the two context languages is empty,
> and on a non-empty intersection writes a witnessing conflicting string
> into `v`. Returns the logical negation of that result, i.e. `true`
> when the intersection is NON-empty (a conflict exists). Does not mutate
> `context`; mutates `v` only via the `is_empty_intersection` call.

> [spec:hfst:def:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.resolvable-conflict-fn]
> bool ConflictResolvingLeftArrowRule::resolvable_conflict

> [spec:hfst:sem:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.resolvable-conflict-fn]
> Returns `bool`. Parameter: `another` (const reference to another
> `ConflictResolvingLeftArrowRule`). Computes `wbize(another.context)`
> and returns `context.is_subset(...)` of it — i.e. `true` when this
> rule's own `context` language is a subset of (a sub-language of) the
> word-boundary-bracketed form of `another`'s context. Does not mutate
> any state.

> [spec:hfst:def:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.resolve-conflict-fn]
> void ConflictResolvingLeftArrowRule::resolve_conflict

> [spec:hfst:sem:conflict-resolving-left-arrow-rule.conflict-resolving-left-arrow-rule.resolve-conflict-fn]
> Returns `void`. Parameter: `another` (const reference to another
> `ConflictResolvingLeftArrowRule`). Mutates this rule's own `context`
> member in place by applying `HfstTransducer::subtract` with
> `another.context` as the argument — i.e. `context := context -
> another.context`, removing `another`'s context language from this
> rule's context. Note: subtracts `another.context` directly (NOT its
> wbized form). No return value.

> [spec:hfst:def:conflict-resolving-left-arrow-rule.get-wb-fst-fn]
> OtherSymbolTransducer get_wb_fst(void)

> [spec:hfst:sem:conflict-resolving-left-arrow-rule.get-wb-fst-fn]
> Free function (file-local). Takes no parameters; returns an
> `OtherSymbolTransducer`. Builds and returns the transducer for the
> language `WB ( (?:? - WB) | <D> )* WB`, i.e. a word-boundary-bracketed
> any-string. Steps:
> 1. Construct `wb` as the identity pair transducer for the
>    word-boundary symbol `"__HFST_TWOLC_.#."` (both input and output =
>    that symbol).
> 2. Construct `no_wb` as the identity transducer for `TWOLC_UNKNOWN`
>    (the unknown-symbol pair `?:?`).
> 3. Construct `diamond` as the identity transducer for `TWOLC_DIAMOND`.
> 4. Mutate `no_wb`: apply `HfstTransducer::subtract` with `wb` (so
>    `no_wb := ?:? - WB`), then apply `HfstTransducer::disjunct` with
>    `diamond` (`no_wb := (?:? - WB) | <D>`), then apply
>    `HfstTransducer::repeat_star` (`no_wb := ((?:? - WB) | <D>)*`).
> 5. Construct `result` as a copy of `wb`, then apply
>    `HfstTransducer::concatenate` with `no_wb`, then apply
>    `HfstTransducer::concatenate` with `wb` again, producing
>    `WB · no_wb · WB`.
> 6. Return `result`.

> [spec:hfst:def:conflict-resolving-left-arrow-rule.main-fn]
> int main(void)

> [spec:hfst:sem:conflict-resolving-left-arrow-rule.main-fn]
> Test driver, compiled only when `TEST_CONFLICT_RESOLVING_LEFT_ARROW_RULE`
> is defined. Takes no parameters; returns `int` (falls off the end, so
> effectively returns 0). Steps:
> 1. Build a `HandySet<SymbolPair>` containing the pairs `("a","b")`,
>    `("a","c")`, `("d","e")` and register it via
>    `OtherSymbolTransducer::set_symbol_pairs`.
> 2. Set the transducer type to `hfst::TROPICAL_OPENFST_TYPE` via
>    `OtherSymbolTransducer::set_transducer_type`.
> 3. Build helper transducers: `unknown` = `TWOLC_UNKNOWN` then
>    `repeat_star` (`?*`); `diamond` = `TWOLC_DIAMOND` (`<D>`); `a_sth` =
>    pair `("a", TWOLC_UNKNOWN)` (`a:?`).
> 4. Build `context1` starting from a copy of `unknown` and concatenating
>    in sequence `a_sth`, `diamond`, `unknown`, `diamond`, `unknown` to
>    form `?* a:? <D> ?* <D> ?*`. Wrap it in an
>    `OtherSymbolTransducerVector v1` of size 1, and construct `rule1`
>    named `"__TWOLC_RULE_NAME=\"test rule I\""` with center
>    `SymbolPair("a","b")` and contexts `v1`.
> 5. Build `a_b` = pair `("a","b")`, then `context2` starting from a copy
>    of `unknown` concatenating `a_b`, `diamond`, `unknown`, `diamond`,
>    `unknown`. Wrap in `OtherSymbolTransducerVector v2` of size 1 and
>    construct `rule2` named `"__TWOLC_RULE_NAME=\"test rule II\""` with
>    center `SymbolPair("a","b")` and contexts `v2`.
> 6. Declare an empty `StringVector v`. The actual assertions/conflict
>    calls (`conflicts_this`, `resolvable_conflict`, `resolve_conflict`,
>    and printing of compiled transducers) are all commented out, so the
>    function performs no checks and produces no output.

> [spec:hfst:def:conflict-resolving-left-arrow-rule.wbize-fn]
> OtherSymbolTransducer wbize(const OtherSymbolTransducer &t)

> [spec:hfst:sem:conflict-resolving-left-arrow-rule.wbize-fn]
> Free function (file-local). Parameter: `t` (const reference to an
> `OtherSymbolTransducer`); returns an `OtherSymbolTransducer`. Makes a
> local copy `t_copy` of `t`, obtains the word-boundary FST by calling
> `get_wb_fst()`, mutates `t_copy` by applying
> `HfstTransducer::intersect` with that FST (so `t_copy := t ∩
> wb_fst`), and returns `t_copy`. Effect: restricts `t` to the strings
> that are properly word-boundary bracketed. Does not mutate the input
> `t`.

