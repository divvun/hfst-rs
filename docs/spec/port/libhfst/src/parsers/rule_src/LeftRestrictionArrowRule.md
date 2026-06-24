# libhfst/src/parsers/rule_src/LeftRestrictionArrowRule.cc, libhfst/src/parsers/rule_src/LeftRestrictionArrowRule.h

> [spec:hfst:def:left-restriction-arrow-rule.left-restriction-arrow-rule]
> class LeftRestrictionArrowRule : public Rule

> [spec:hfst:def:left-restriction-arrow-rule.left-restriction-arrow-rule.compile-fn]
> OtherSymbolTransducer LeftRestrictionArrowRule::compile(void)

> [spec:hfst:sem:left-restriction-arrow-rule.left-restriction-arrow-rule.compile-fn]
> Compiles the left-restriction (`/<=`) rule and returns the resulting
> `OtherSymbolTransducer`, also storing it in the inherited member
> `rule_transducer`.
> Steps, in order, mutating the inherited `center` member in place:
> 1. `center.apply(&HfstTransducer::intersect, context)` — intersect `center`
>    with the inherited `context` member (the disjunction of all the rule's
>    contexts built by the base `Rule` constructor).
> 2. On that same result, `apply(&HfstTransducer::substitute, TWOLC_DIAMOND,
>    HFST_EPSILON, true, true)` — substitute every occurrence of the diamond
>    boundary symbol `TWOLC_DIAMOND` with epsilon `HFST_EPSILON` (both the
>    input-side and output-side flags are true, i.e. replace on both sides).
>    After these two `apply` calls, `center` holds the set of strings that are
>    centers surrounded by an allowed context.
> 3. Assign `rule_transducer = OtherSymbolTransducer(TWOLC_UNKNOWN)` — a fresh
>    transducer accepting the single unknown symbol `TWOLC_UNKNOWN`.
> 4. `rule_transducer.apply(&HfstTransducer::repeat_star)` — Kleene-star it so it
>    accepts any string over the unknown alphabet (the universal language Sigma*).
> 5. On that same `rule_transducer`, `apply(&HfstTransducer::subtract, center)` —
>    subtract the (now context-restricted) `center` language, yielding
>    Sigma* minus center.
> 6. Return `rule_transducer`.
> Note: `apply` mutates the receiver and returns a reference to it so calls can
> be chained. No exceptions are thrown explicitly here; any come from the
> underlying HfstTransducer operations.

> [spec:hfst:def:left-restriction-arrow-rule.left-restriction-arrow-rule.left-restriction-arrow-rule-fn]
> LeftRestrictionArrowRule::LeftRestrictionArrowRule

> [spec:hfst:sem:left-restriction-arrow-rule.left-restriction-arrow-rule.left-restriction-arrow-rule-fn]
> Two overloaded constructors, both of which simply delegate to the base
> `Rule(name, center, contexts)` constructor and have empty bodies:
> 1. `LeftRestrictionArrowRule(const std::string &name,
>    const OtherSymbolTransducer &center,
>    const OtherSymbolTransducerVector &contexts)` — forwards `name`, `center`,
>    and `contexts` directly to `Rule(name, center, contexts)`.
> 2. `LeftRestrictionArrowRule(const std::string &name,
>    const SymbolPair &center,
>    const OtherSymbolTransducerVector &contexts)` — builds the center
>    transducer from the symbol pair by calling the static
>    `Rule::get_center(center.first, center.second)` (input symbol, output
>    symbol) and forwards the result, along with `name` and `contexts`, to
>    `Rule(name, Rule::get_center(...), contexts)`.
> The base `Rule` constructor unescapes `name`, stores `center`, sets
> `is_empty = false`, builds the `context` member by disjuncting all
> transducers in `contexts`, and harmonizes diacritics between center and
> context. Neither constructor body does any additional work.

> [spec:hfst:def:left-restriction-arrow-rule.main-fn]
> int main(void)

> [spec:hfst:sem:left-restriction-arrow-rule.main-fn]
> A test-only `main` compiled only when the `TEST_LEFT_RESTRICTION_ARROW_RULE`
> macro is defined (the surrounding `#ifdef` also pulls in `<cassert>`). It
> exercises compilation of the rule `a:b /<= b:c _ ;`. Steps:
> 1. Determine the implementation type at compile time via `HAVE_OPENFST`,
>    `HAVE_SFST`, `HAVE_FOMA` flags, picking, in priority order,
>    `TROPICAL_OPENFST_TYPE`, else `SFST_TYPE`, else `FOMA_TYPE`, else
>    `ERROR_TYPE`. Call `OtherSymbolTransducer::set_transducer_type(...)` with it.
> 2. Build a `HandySet<SymbolPair>` containing the pairs `("a","b")`,
>    `("a","d")`, and `("b","c")`, and register it via
>    `OtherSymbolTransducer::set_symbol_pairs(symbols)`.
> 3. Construct `unknown` as `OtherSymbolTransducer("__HFST_TWOLC_?","__HFST_TWOLC_?")`
>    then `apply(&HfstTransducer::repeat_star)` so it accepts any string.
> 4. Construct `diamond` = `OtherSymbolTransducer("__HFST_TWOLC_DIAMOND")` and
>    `a_b_pair` = `OtherSymbolTransducer("a","b")`.
> 5. Build the center: copy `unknown` into `center`, then chain-concatenate
>    `diamond`, `a_b_pair`, `diamond`, `unknown` (i.e. unknown DIAMOND a:b
>    DIAMOND unknown).
> 6. Construct `b_c_pair` = `OtherSymbolTransducer("b","c")`. Build the context:
>    copy `unknown` into `context`, then chain-concatenate `b_c_pair`,
>    `diamond`, `unknown`, `diamond`, `unknown`.
> 7. Make `contexts` a vector of one element (`context`).
> 8. Construct the rule `LeftRestrictionArrowRule
>    ("__HFST_TWOLC_RULE_NAME=\"Test rule\"", center, contexts)` and call
>    `rule.compile()`, storing into `compiled_rule`.
> 9. Build a small test string transducer `b_c` = `("b","c")`, concatenate
>    `a_d` = `OtherSymbolTransducer("a","b")` onto it, then intersect with
>    `compiled_rule`.
> The commented-out `std::cout` lines indicate the result transducers would be
> printed for manual inspection. The function returns nothing explicit (falls
> off the end, returning 0) and contains no `assert` calls despite including
> `<cassert>`; it is purely a smoke test of compilation.

