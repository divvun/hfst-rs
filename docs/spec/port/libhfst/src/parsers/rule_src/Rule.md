# libhfst/src/parsers/rule_src/Rule.cc, libhfst/src/parsers/rule_src/Rule.h

> [spec:hfst:def:rule.main-fn]
> int

> [spec:hfst:sem:rule.main-fn]
> Test harness entry point compiled only when the `TEST_RULE` macro is
> defined. `main(void)` has an empty body (a comment `/* TEST */`) and
> falls off the end, implicitly returning 0. No behavior; exists solely
> as a standalone-test stub. Omit entirely in the Rust port unless an
> equivalent test build flag is needed.

> [spec:hfst:def:rule.rule]
> class Rule {
>   bool is_empty;
>   std::string name;
>   OtherSymbolTransducer center;
>   OtherSymbolTransducer context;
>   OtherSymbolTransducer rule_transducer;
> }

> [spec:hfst:def:rule.rule.add-missing-symbols-freely-fn]
> void

> [spec:hfst:sem:rule.rule.add-missing-symbols-freely-fn]
> Ensures every diacritic symbol in `diacritics` (a `SymbolRange`) can
> occur freely in `rule_transducer`. Steps: build the current alphabet
> by constructing an `HfstBasicTransducer` from
> `rule_transducer.get_transducer()` and calling `get_alphabet()`,
> yielding a `std::set<std::string> symbol_set`. Then iterate over each
> symbol `*it` in `diacritics`; if it is NOT already present in
> `symbol_set`, (1) call `rule_transducer.add_symbol_to_alphabet(*it)`
> to register the symbol, and (2) call `rule_transducer.apply` with
> `&HfstTransducer::insert_freely`, passing `SymbolPair(*it, *it)` and
> the boolean `true`, which inserts that identity symbol pair freely at
> every position in the transducer. Symbols already in the alphabet are
> skipped. Mutates `rule_transducer` in place; returns void.

> [spec:hfst:def:rule.rule.add-name-fn]
> void

> [spec:hfst:sem:rule.rule.add-name-fn]
> Annotates `rule_transducer` with this rule's name by calling
> `rule_transducer.add_info_symbol(name)`, passing the member `name`
> string. Mutates `rule_transducer` in place; returns void. No branches
> or other side effects.

> [spec:hfst:def:rule.rule.compile-fn]
> OtherSymbolTransducer

> [spec:hfst:sem:rule.rule.compile-fn]
> Stub: returns a default-constructed `OtherSymbolTransducer()`. Reads
> no state, mutates nothing, has no side effects. The real compilation
> happens in the `Rule(name, RuleVector)` constructor; this method is a
> placeholder returning an empty transducer.

> [spec:hfst:def:rule.rule.empty-fn]
> bool

> [spec:hfst:sem:rule.rule.empty-fn]
> Const getter: returns the boolean member `is_empty`. No side effects.

> [spec:hfst:def:rule.rule.get-center-fn]
> OtherSymbolTransducer

> [spec:hfst:sem:rule.rule.get-center-fn]
> Static helper building a "center" transducer from a vector of symbol
> pairs `v` (`SymbolPairVector`). Steps: (1) build `unknown` = an
> `OtherSymbolTransducer(TWOLC_UNKNOWN)` then apply
> `&HfstTransducer::repeat_star` to it (Kleene-star of any symbol). (2)
> build `diamond` = `OtherSymbolTransducer(TWOLC_DIAMOND)`. (3) build
> `center_pair_transducer` (default-constructed) and for each pair `it`
> in `v`, construct `OtherSymbolTransducer pair(it->first, it->second)`
> and disjunct it into `center_pair_transducer` via
> `apply(&HfstTransducer::disjunct, pair)`; this yields the union of all
> the given input:output pairs. (4) copy `unknown` into `center`, then
> concatenate, in order: `diamond`, `center_pair_transducer`,
> `diamond`, `unknown` (i.e. result = unknown* · diamond ·
> (union of pairs) · diamond · unknown*). Returns `center` by value.
> Note: there are two other overloads of `get_center` (taking
> `input`/`output` strings, and taking a restricted-center transducer)
> that are not annotated by this rule; only the `SymbolPairVector`
> overload is described here.

> [spec:hfst:def:rule.rule.get-name-fn]
> std::string

> [spec:hfst:sem:rule.rule.get-name-fn]
> Getter: returns a copy of the member `name` string. No side effects.

> [spec:hfst:def:rule.rule.get-print-name-fn]
> std::string

> [spec:hfst:sem:rule.rule.get-print-name-fn]
> Static function producing a human-readable display form of an internal
> twol-c symbol string `s`. Copies `s` into `ss`, then performs four
> sequential replacement passes, each a `while` loop that repeats until
> the searched substring is absent (`find(...) == npos`); each iteration
> replaces the FIRST occurrence found:
> (1) replace every `"__HFST_TWOLC_SPACE"` with a single space `" "`;
> (2) replace every `"__HFST_TWOLC_RULE_NAME="` with a single space `" "`;
> (3) replace every `"__HFST_TWOLC_SET_NAME="` with the empty string `""`;
> (4) replace every remaining `"__HFST_TWOLC_"` prefix with the empty
> string `""`.
> Each `replace` uses the literal length of the searched token. Order
> matters: the longer SPACE/RULE_NAME=/SET_NAME= tokens are handled
> before the generic `__HFST_TWOLC_` prefix strip. Returns the
> transformed string `ss`. Pure (no member state touched).

> [spec:hfst:def:rule.rule.get-universal-language-with-diamonds-fn]
> OtherSymbolTransducer

> [spec:hfst:sem:rule.rule.get-universal-language-with-diamonds-fn]
> Static helper building the universal language interleaved with two
> diamond markers. Steps: (1) `universal` =
> `OtherSymbolTransducer(TWOLC_UNKNOWN)` then apply
> `&HfstTransducer::repeat_star` (Kleene-star of any symbol). (2)
> `diamond` = `OtherSymbolTransducer(TWOLC_DIAMOND)`. (3) copy
> `universal` into `universal_with_diamonds`, then concatenate in order:
> `diamond`, `universal`, `diamond`, `universal` (result = universal* ·
> diamond · universal* · diamond · universal*). Returns
> `universal_with_diamonds` by value. No member state read or mutated.

> [spec:hfst:def:rule.rule.rule-fn]
> Rule::Rule(const std::string &name, const RuleVector &v)

> [spec:hfst:sem:rule.rule.rule-fn]
> Constructor `Rule(const std::string &name, const RuleVector &v)` that
> composes a single rule transducer as the intersection of the
> transducers of its constituent sub-rules `v`. Member initializer list:
> `is_empty` = true, `name` = `unescape_name(name)`, and
> `rule_transducer` constructed from `TWOLC_UNKNOWN`. Body: (1) apply
> `&HfstTransducer::repeat_star` to `rule_transducer`, making it the
> Kleene-star of any symbol (the universal language, the identity
> element for intersection). (2) Iterate over each `Rule*` `*it` in `v`;
> for each sub-rule that is NOT empty (`!(*it)->empty()`), intersect its
> `rule_transducer` into this one via
> `rule_transducer.apply(&HfstTransducer::intersect,
> (*it)->rule_transducer)` and set `is_empty = false`. Empty sub-rules
> are skipped and contribute nothing. After the loop, `is_empty` is true
> only if every sub-rule was empty (in which case `rule_transducer`
> stays the universal language). Note `unescape_name` is an external
> helper applied to the name. No return value (constructor).

> [spec:hfst:def:rule.rule.rule-vector]
> typedef std::vector<Rule*> RuleVector

> [spec:hfst:def:rule.rule.store-fn]
> void

> [spec:hfst:sem:rule.rule.store-fn]
> Serializes the compiled rule into an `HfstOutputStream &out`. Early
> return: if `is_empty` is true, do nothing and return immediately.
> Otherwise: (1) call `add_name()` to stamp this rule's name onto
> `rule_transducer`. (2) `rule_transducer.remove_diacritics_from_output()`.
> (3) A sequence of `apply(&HfstTransducer::substitute, ...)` rewrites
> that translate internal twol-c symbols to their external forms, each
> mutating `rule_transducer` in place:
>   - substitute symbol `TWOLC_EPSILON` -> `HFST_EPSILON`, with the two
>     trailing booleans `true, true` (substitute on input and output);
>   - substitute symbol `"__HFST_TWOLC_.#.""` -> `"@#@"`, `true, true`;
>   - substitute symbol `"__HFST_TWOLC_SPACE"` -> `" "`, `true, true`;
>   - substitute the symbol PAIR `("@#@","@#@")` -> `("@#@",HFST_EPSILON)`
>     (the pair-form overload, no trailing booleans);
>   - substitute symbol `TWOLC_IDENTITY` -> `HFST_IDENTITY`, `true, true`.
> (4) Bind `HfstTransducer &t = rule_transducer.transducer` and write it
> to the stream with `out << t`. Mutates `rule_transducer` and performs
> stream I/O; returns void.

