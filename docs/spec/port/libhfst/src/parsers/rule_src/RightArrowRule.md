# libhfst/src/parsers/rule_src/RightArrowRule.cc, libhfst/src/parsers/rule_src/RightArrowRule.h

> [spec:hfst:def:right-arrow-rule.main-fn]
> int main(void)

> [spec:hfst:sem:right-arrow-rule.main-fn]
> Standalone test driver, compiled only when `TEST_RIGHT_ARROW_RULE` is
> defined. Steps:
> 1. Determine availability of back-ends via preprocessor macros into
>    booleans `have_openfst` (from `HAVE_OPENFST`), `have_sfst` (from
>    `HAVE_SFST`), `have_foma` (from `HAVE_FOMA`).
> 2. Pick `transducer_type` as the first available of
>    `hfst::TROPICAL_OPENFST_TYPE`, `hfst::SFST_TYPE`, `hfst::FOMA_TYPE`,
>    falling back to `hfst::ERROR_TYPE` if none are available, and call
>    `OtherSymbolTransducer::set_transducer_type(transducer_type)`.
> 3. Build a `HandySet<SymbolPair>` `symbols` containing the pairs
>    `("a","b")`, `("a","d")`, `("b","c")`, and register it via
>    `OtherSymbolTransducer::set_symbol_pairs(symbols)`.
> 4. Construct `unknown` as `OtherSymbolTransducer("__HFST_TWOLC_?",
>    "__HFST_TWOLC_?")` then `apply(&HfstTransducer::repeat_star)` to get
>    `?*`. Construct `diamond` = `OtherSymbolTransducer("__HFST_TWOLC_DIAMOND")`
>    and `a_b_pair` = `OtherSymbolTransducer("a","b")`.
> 5. Build `center` as a copy of `unknown`, then concatenate in order
>    (each via `apply(&HfstTransducer::concatenate, ...)`): `diamond`,
>    `a_b_pair`, `diamond`, `unknown` — yielding `?* DIAMOND a:b DIAMOND ?*`.
> 6. Build `context` as a copy of `unknown`, then concatenate in order:
>    `b_c_pair` (= `OtherSymbolTransducer("b","c")`), `diamond`,
>    `unknown`, `diamond`, `unknown`.
> 7. Make `contexts` an `OtherSymbolTransducerVector` of size 1 holding
>    `context`.
> 8. Construct `RightArrowRule rule("__HFST_TWOLC_RULE_NAME=\"Test rule\"",
>    center, contexts)` and call `rule.compile()`, storing the result in
>    `compiled_rule` (which is otherwise unused; the printing of its
>    transducer is commented out).
> Returns implicitly (no asserts are actually exercised; `<cassert>` is
> included). Side effects are the static state set on
> `OtherSymbolTransducer`.

> [spec:hfst:def:right-arrow-rule.right-arrow-rule]
> class RightArrowRule : public Rule

> [spec:hfst:def:right-arrow-rule.right-arrow-rule.compile-fn]
> OtherSymbolTransducer RightArrowRule::compile(void)

> [spec:hfst:sem:right-arrow-rule.right-arrow-rule.compile-fn]
> Compiles this `=>`-type twol rule and returns the resulting
> `OtherSymbolTransducer`. `apply(member, args...)` invokes the named
> `HfstTransducer` member function on the wrapped transducer in place
> and returns `*this`, enabling chaining. Steps, all mutating member
> state:
> 1. Transform the `center` member in place by chaining two operations:
>    first `center.apply(&HfstTransducer::subtract, context)` —
>    subtract the `context` transducer (the union of contexts built in
>    the constructor) from `center`; then on the result
>    `apply(&HfstTransducer::substitute, TWOLC_DIAMOND, HFST_EPSILON,
>    true, true)` — substitute every occurrence of the diamond symbol
>    `TWOLC_DIAMOND` with epsilon `HFST_EPSILON`, passing `true, true`
>    for the input-side and output-side substitution flags. After this,
>    `center` holds the set of strings that are in the rule's center
>    but not licensed by any context, with diamonds removed.
> 2. Assign `rule_transducer = OtherSymbolTransducer(TWOLC_UNKNOWN)`,
>    a single-symbol transducer over the unknown symbol `TWOLC_UNKNOWN`.
> 3. Transform `rule_transducer` in place by chaining:
>    `apply(&HfstTransducer::repeat_star)` — Kleene-star it to form
>    `?*` (the universal language); then
>    `apply(&HfstTransducer::subtract, center)` — subtract the modified
>    `center` from it. The result is the universal language minus the
>    illicit (unlicensed) center strings.
> 4. Return `rule_transducer` (by value).
> The `center` and `rule_transducer` members are both mutated as side
> effects.

> [spec:hfst:def:right-arrow-rule.right-arrow-rule.right-arrow-rule-fn]
> RightArrowRule::RightArrowRule

> [spec:hfst:sem:right-arrow-rule.right-arrow-rule.right-arrow-rule-fn]
> Constructor for `RightArrowRule`. Takes `name` (a string), `center`
> (an `OtherSymbolTransducer`), and `contexts` (an
> `OtherSymbolTransducerVector`). The body is empty; all work is
> delegated to the base-class `Rule(name, center, contexts)`
> constructor in the initializer list. That base constructor sets
> `is_empty=false`, sets `name` to `unescape_name(name)`, copies
> `center` into the `center` member, then folds all contexts together
> into the single `context` member by applying
> `HfstTransducer::disjunct` of each element of `contexts` into
> `context` (an initially default-constructed `OtherSymbolTransducer`,
> so the result is the union of all contexts), and finally calls
> `this->center.harmonize_diacritics(context)` to harmonize the
> other-symbol/diacritic alphabets between `center` and `context`.
> No additional state is set and nothing is returned.

