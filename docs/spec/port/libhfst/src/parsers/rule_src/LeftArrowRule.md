# libhfst/src/parsers/rule_src/LeftArrowRule.cc, libhfst/src/parsers/rule_src/LeftArrowRule.h

> [spec:hfst:def:left-arrow-rule.left-arrow-rule]
> class LeftArrowRule : public Rule

> [spec:hfst:def:left-arrow-rule.left-arrow-rule.compile-fn]
> OtherSymbolTransducer LeftArrowRule::compile(void)

> [spec:hfst:sem:left-arrow-rule.left-arrow-rule.compile-fn]
> Compiles this `<=`-type twol rule into a single `OtherSymbolTransducer` and
> returns it. Steps:
> 1. Compute `abstract_center = center.get_inverse_of_upper_projection()` (the
>    inverse of the upper-side projection of the member `center` transducer).
> 2. Modify the member `context` transducer in place via successive
>    `OtherSymbolTransducer::apply(...)` calls (each applies the named
>    `HfstTransducer` member-function operation and returns `*this`):
>    a. `apply(&HfstTransducer::intersect, abstract_center)` — intersect
>       `context` with `abstract_center`.
>    b. `apply(&HfstTransducer::subtract, center)` — subtract `center` from the
>       result.
>    c. `apply(&HfstTransducer::substitute, TWOLC_DIAMOND, HFST_EPSILON, true, true)`
>       — substitute the diamond symbol `TWOLC_DIAMOND` with the epsilon symbol
>       `HFST_EPSILON` (both the input-side and output-side boolean flags set
>       true, replacing the symbol on both tape sides).
> 3. Assign `rule_transducer = OtherSymbolTransducer(TWOLC_UNKNOWN)` (a fresh
>    transducer over the unknown symbol).
> 4. Return `rule_transducer.apply(&HfstTransducer::repeat_star).apply(&HfstTransducer::subtract, context)`:
>    take the Kleene-star of `rule_transducer` (so it becomes `?*`) and then
>    subtract the now-transformed `context` from it. The returned value is the
>    result of this final `apply` chain (a reference to the mutated
>    `rule_transducer`, returned by value as an `OtherSymbolTransducer`).

> [spec:hfst:def:left-arrow-rule.left-arrow-rule.left-arrow-rule-fn]
> LeftArrowRule::LeftArrowRule

> [spec:hfst:sem:left-arrow-rule.left-arrow-rule.left-arrow-rule-fn]
> Constructor. Takes `name` (a `std::string`), `center` (an
> `OtherSymbolTransducer`), and `contexts` (an `OtherSymbolTransducerVector`),
> all by const reference. It does nothing of its own (empty body); it simply
> forwards all three arguments to the base-class `Rule` constructor as
> `Rule(name, center, contexts)`, which initializes the inherited `name`,
> `center`, and `context` members.

> [spec:hfst:def:left-arrow-rule.main-fn]
> int main(void)

> [spec:hfst:sem:left-arrow-rule.main-fn]
> Standalone test driver, compiled only when the `TEST_LEFT_ARROW_RULE` macro
> is defined (and which also pulls in `<cassert>`). Steps:
> 1. Detect available backends via preprocessor macros: set `have_openfst`,
>    `have_sfst`, `have_foma` to true iff `HAVE_OPENFST` / `HAVE_SFST` /
>    `HAVE_FOMA` respectively are defined, otherwise false.
> 2. Choose `transducer_type` by preference order: `TROPICAL_OPENFST_TYPE` if
>    `have_openfst`, else `SFST_TYPE` if `have_sfst`, else `FOMA_TYPE` if
>    `have_foma`, else `ERROR_TYPE`.
> 3. Call `OtherSymbolTransducer::set_transducer_type(transducer_type)` to set
>    the global transducer implementation type.
> 4. Build a `HandySet<SymbolPair>` named `symbols` containing the pairs
>    `("a","b")`, `(TWOLC_EPSILON,"c")`, and `(TWOLC_EPSILON,"d")`, then call
>    `OtherSymbolTransducer::set_symbol_pairs(symbols)` to register the global
>    symbol-pair alphabet.
> 5. Construct `unknown` as `OtherSymbolTransducer("__HFST_TWOLC_?","__HFST_TWOLC_?")`.
>    Copy it into `unknown_optional` and apply `&HfstTransducer::optionalize` to
>    `unknown_optional`. Then apply `&HfstTransducer::repeat_star` to `unknown`
>    (so `unknown` becomes `?*`).
> 6. Construct `diamond` as `OtherSymbolTransducer("__HFST_TWOLC_DIAMOND")` and
>    `zero_c_pair` as `OtherSymbolTransducer(TWOLC_EPSILON,"c")`.
> 7. Build `center` by copying `unknown`, then concatenating in order:
>    `diamond`, `zero_c_pair`, `diamond`, `unknown` — yielding `?* <D> 0:c <D> ?*`.
> 8. Construct `a_b_pair` as `OtherSymbolTransducer("a","b")`.
> 9. Build `context` by copying `unknown`, then concatenating in order:
>    `a_b_pair`, `diamond`, `unknown_optional`, `diamond`, `a_b_pair`, `unknown`
>    — yielding `?* a:b <D> ?* <D> a:b ?*`.
> 10. Build `contexts` as an `OtherSymbolTransducerVector` of size 1 holding
>     `context`.
> 11. Construct `rule` as `LeftArrowRule("__HFST_TWOLC_RULE_NAME=\"Test rule\"", center, contexts)`.
> 12. Call `rule.compile()` and store the result in `compiled_rule` (the line
>     printing it via `std::cout` is commented out, so there is no output).
> 13. Falls off the end of `main` returning 0 implicitly. The driver performs no
>     assertions despite including `<cassert>`; it merely exercises the compile
>     path.

