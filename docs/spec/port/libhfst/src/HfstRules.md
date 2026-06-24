# libhfst/src/HfstRules.cc

> [spec:hfst:def:hfst-rules.hfst.rules.coercion-fn]
> HfstTransducer coercion(HfstTransducerPairVector &contexts,

> [spec:hfst:sem:hfst-rules.hfst.rules.coercion-fn]
> Thin wrapper. Returns `restriction(contexts, mapping, alphabet, twol_left, 0)`,
> i.e. calls the general `restriction` helper with twol type `twol_left` and
> direction `0`. Takes `contexts` (HfstTransducerPairVector), `mapping`
> (HfstTransducer&) and `alphabet` (StringPairSet&), all by reference.

> [spec:hfst:def:hfst-rules.hfst.rules.deep-coercion-fn]
> HfstTransducer deep_coercion(HfstTransducerPairVector &contexts,

> [spec:hfst:sem:hfst-rules.hfst.rules.deep-coercion-fn]
> Thin wrapper. Returns `restriction(contexts, mapping, alphabet, twol_left, -1)`,
> i.e. calls the general `restriction` helper with twol type `twol_left` and
> direction `-1` (deep/output side). Parameters `contexts`, `mapping`,
> `alphabet` are passed by reference unchanged.

> [spec:hfst:def:hfst-rules.hfst.rules.deep-restriction-and-coercion-fn]
> HfstTransducer deep_restriction_and_coercion

> [spec:hfst:sem:hfst-rules.hfst.rules.deep-restriction-and-coercion-fn]
> Thin wrapper. Returns `restriction(contexts, mapping, alphabet, twol_both, -1)`,
> i.e. calls the general `restriction` helper with twol type `twol_both` and
> direction `-1` (deep/output side). Parameters `contexts`, `mapping`,
> `alphabet` are passed by reference unchanged.

> [spec:hfst:def:hfst-rules.hfst.rules.deep-restriction-fn]
> HfstTransducer deep_restriction(HfstTransducerPairVector &contexts,

> [spec:hfst:sem:hfst-rules.hfst.rules.deep-restriction-fn]
> Thin wrapper. Returns `restriction(contexts, mapping, alphabet, twol_right, -1)`,
> i.e. calls the general `restriction` helper with twol type `twol_right` and
> direction `-1` (deep/output side). Parameters `contexts`, `mapping`,
> `alphabet` are passed by reference unchanged.

> [spec:hfst:def:hfst-rules.hfst.rules.left-replace-down-fn]
> HfstTransducer left_replace_down ( HfstTransducerPair &context,

> [spec:hfst:sem:hfst-rules.hfst.rules.left-replace-down-fn]
> Left-arrow ("<-") replace-down, SFST's version. If `optional` is true, returns
> `replace_down(context, mapping, 1, alphabet).invert()`; otherwise returns
> `replace_down(context, mapping, 0, alphabet).invert()`. That is: build the
> corresponding right-arrow replace-down rule with the same optional flag, then
> invert the resulting transducer (swap input/output sides) and return it.

> [spec:hfst:def:hfst-rules.hfst.rules.left-replace-down-karttunen-fn]
> HfstTransducer left_replace_down_karttunen( HfstTransducerPair &context,

> [spec:hfst:sem:hfst-rules.hfst.rules.left-replace-down-karttunen-fn]
> Left-arrow ("<-") replace-down, XFST's (Karttunen's) version. If `optional` is
> true, returns `replace_down_karttunen(context, mapping, 1, alphabet).invert()`;
> otherwise returns `replace_down_karttunen(context, mapping, 0, alphabet).invert()`.
> That is: build the corresponding right-arrow Karttunen replace-down rule with the
> same optional flag, then invert the resulting transducer and return it.

> [spec:hfst:def:hfst-rules.hfst.rules.left-replace-left-fn]
> HfstTransducer left_replace_left ( HfstTransducerPair &context,

> [spec:hfst:sem:hfst-rules.hfst.rules.left-replace-left-fn]
> Left-arrow ("<-") replace-left. If `optional` is true, returns
> `replace_left(context, mapping, 1, alphabet).invert()`; otherwise returns
> `replace_left(context, mapping, 0, alphabet).invert()`. That is: build the
> corresponding right-arrow replace-left rule with the same optional flag, then
> invert the resulting transducer (swap input/output) and return it.

> [spec:hfst:def:hfst-rules.hfst.rules.left-replace-right-fn]
> HfstTransducer left_replace_right ( HfstTransducerPair &context,

> [spec:hfst:sem:hfst-rules.hfst.rules.left-replace-right-fn]
> Left-arrow ("<-") replace-right. If `optional` is true, returns
> `replace_right(context, mapping, 1, alphabet).invert()`; otherwise returns
> `replace_right(context, mapping, 0, alphabet).invert()`. That is: build the
> corresponding right-arrow replace-right rule with the same optional flag, then
> invert the resulting transducer (swap input/output) and return it.

> [spec:hfst:def:hfst-rules.hfst.rules.left-replace-up-fn]
> HfstTransducer left_replace_up(HfstTransducer &mapping,

> [spec:hfst:sem:hfst-rules.hfst.rules.left-replace-up-fn]
> Left-arrow ("<-") replace-up without context. If `optional` is true, returns
> `replace_up(mapping, 1, alphabet).invert()`; otherwise returns
> `replace_up(mapping, 0, alphabet).invert()`. That is: build the corresponding
> context-free right-arrow replace-up rule (the `replace`-based overload, no
> context pair) with the same optional flag, then invert the resulting transducer
> (swap input/output) and return it.

> [spec:hfst:def:hfst-rules.hfst.rules.replace-context-fn]
> HfstTransducer replace_context(HfstTransducer &t,

> [spec:hfst:sem:hfst-rules.hfst.rules.replace-context-fn]
> Builds a context-restriction transducer over `alphabet` for context `t` with two
> boundary marker symbols `m1` and `m2`. Steps:
> 1. `t_copy` = copy of `t` with `StringPair(m1,m1)` inserted freely, then
>    `StringPair(m2,m2)` inserted freely (so the markers may appear anywhere
>    inside `t`).
> 2. `pi_star` = universal language `.*` over `alphabet` (StringPairSet ctor with
>    harmonize flag true), of `t.get_type()`.
> 3. `arg1` = `pi_star` concatenated with `t_copy` (i.e. `.* (m1>>(m2>>t))`).
> 4. `m1_tr` = single-symbol transducer for `m1`. `arg2` = `pi_star` minus
>    (`pi_star` concatenated with `m1_tr`), i.e. `!(.* m1)`.
> 5. `ct` = `arg1.compose(arg2)`.
> 6. `mt` = `m2` repeated (Kleene star), then concatenated with `m1_tr`, then with
>    `pi_star`, i.e. `m2* m1 .*`.
> 7. `ct_neg_mt` = `ct` concatenated with (`pi_star` minus `mt`), i.e. `ct !mt`.
> 8. `neg_ct_mt` = (`pi_star` minus `ct`) concatenated with `mt`, i.e. `!ct mt`.
> 9. `disj` = `neg_ct_mt` disjuncted with `ct_neg_mt`.
> 10. `retval` = `pi_star` minus `disj` (negation), then `retval.optimize()`.
> Returns `retval`. Reads/uses `alphabet` (not mutated here); throws nothing
> beyond what the called operations throw.

> [spec:hfst:def:hfst-rules.hfst.rules.replace-down-fn]
> HfstTransducer replace_down(HfstTransducerPair &context,

> [spec:hfst:sem:hfst-rules.hfst.rules.replace-down-fn]
> Thin wrapper. Returns
> `replace_in_context(context, REPL_DOWN, mapping, optional, alphabet)`. Passes
> the context pair, mapping, optional flag and alphabet straight through with
> replace type `REPL_DOWN`.

> [spec:hfst:def:hfst-rules.hfst.rules.replace-down-karttunen-fn]
> HfstTransducer replace_down_karttunen(HfstTransducerPair &context,

> [spec:hfst:sem:hfst-rules.hfst.rules.replace-down-karttunen-fn]
> Thin wrapper. Returns
> `replace_in_context(context, REPL_DOWN_KARTTUNEN, mapping, optional, alphabet)`.
> Passes the context pair, mapping, optional flag and alphabet straight through
> with replace type `REPL_DOWN_KARTTUNEN`.

> [spec:hfst:def:hfst-rules.hfst.rules.replace-fn]
> HfstTransducer replace( HfstTransducer &t,

> [spec:hfst:sem:hfst-rules.hfst.rules.replace-fn]
> Context-free unconditional replace. Steps:
> 1. `type` = `t.get_type()`.
> 2. `t_proj` = copy of `t`; if `repl_type == REPL_UP` call `t_proj.input_project()`,
>    else if `repl_type == REPL_DOWN` call `t_proj.output_project()`, else throw
>    `HfstFatalException` with message "impossible replace type".
> 3. `pi_star` = universal `.*` over `alphabet` (StringPairSet ctor, harmonize
>    flag true) of `type`.
> 4. `tc` = `pi_star` concatenated with `t_proj` concatenated with `pi_star`
>    (i.e. `.* t_proj .*`).
> 5. `tc_neg` = `pi_star` minus `tc` (i.e. `!(.* t_proj .*)`).
> 6. `retval` = `tc_neg`; concatenate `t`; apply `repeat_star()`; concatenate
>    `tc_neg` again (i.e. `(tc_neg t)* tc_neg`).
> 7. If `optional`, disjunct `pi_star` into `retval`.
> Returns `retval`.

> [spec:hfst:def:hfst-rules.hfst.rules.replace-in-context-fn]
> HfstTransducer replace_in_context(HfstTransducerPair &context,

> [spec:hfst:sem:hfst-rules.hfst.rules.replace-in-context-fn]
> Core conditional-replacement builder using left/right boundary markers. Steps:
> 1. Type check: if `context.first`, `context.second` and `t` do not all have the
>    same type, throw `TransducerTypeMismatchException("rules::replace_in_context")`.
>    `type` = `t.get_type()`.
> 2. Verify both context transducers are automata: copy each context, call
>    `input_project()` on the copy, and `compare()` it to the original; if either
>    differs, throw `ContextTransducersAreNotAutomataException`.
> 3. Define marker strings `leftm="@_LEFT_MARKER_@"`, `rightm="@_RIGHT_MARKER_@"`,
>    and `epsilon=internal_epsilon`.
> 4. `ibt` (insert-boundary) = universal transducer over `alphabet` plus pairs
>    `(epsilon,leftm)` and `(epsilon,rightm)`: `(. | <>:<L> | <>:<R>)*`.
> 5. `rbt` (remove-boundary) = universal over `alphabet` plus `(leftm,epsilon)` and
>    `(rightm,epsilon)`: `(. | <L>:<> | <R>:<>)*`.
> 6. Insert `(leftm,leftm)` and `(rightm,rightm)` into `alphabet` (MUTATES caller's
>    `alphabet`), then build `pi_star` over the augmented `alphabet`.
> 7. `cbt` (constrain-boundary) = `pi_star` minus `(pi_star <L>:<L> <R>:<R> pi_star)`,
>    i.e. `!(.* <L><R> .*)`, then `optimize()`.
> 8. `lct` (left context) = `replace_context(context.first, leftm, rightm, alphabet)`,
>    then `optimize()`.
> 9. `right_rev` = copy of `context.second`, `reverse()`d and `optimize()`d.
>    `rct` (right context) = `replace_context(right_rev, rightm, leftm, alphabet)`,
>    then `reverse()` and `optimize()`.
> 10. `rt` (unconditional replace transducer): if `repl_type` is one of `REPL_UP`,
>    `REPL_RIGHT`, `REPL_LEFT`, `REPL_DOWN_KARTTUNEN`, set
>    `rt = replace_transducer(t, leftm, rightm, REPL_UP, alphabet)`; else
>    `rt = replace_transducer(t, leftm, rightm, REPL_DOWN, alphabet)`; then
>    `rt.optimize()`.
> 11. Compose the result chain: `result` = `ibt`; compose `cbt`; `optimize()`.
>    If `repl_type` is `REPL_UP` or `REPL_RIGHT`, compose `rct`. If `REPL_UP` or
>    `REPL_LEFT`, compose `lct`. `optimize()`. Compose `rt`. If `repl_type` is
>    `REPL_DOWN`, `REPL_RIGHT` or `REPL_DOWN_KARTTUNEN`, compose `lct`. If
>    `REPL_DOWN`, `REPL_LEFT` or `REPL_DOWN_KARTTUNEN`, compose `rct`. `optimize()`.
>    Compose `rbt`.
> 12. Erase `(leftm,leftm)` and `(rightm,rightm)` from `alphabet` (restores the
>    caller's `alphabet` to its original contents).
> 13. If `optional`, build a fresh `pi_star_` over the now-restored `alphabet` and
>    disjunct it into `result`.
> 14. `result.optimize()` and return `result`.

> [spec:hfst:def:hfst-rules.hfst.rules.replace-left-fn]
> HfstTransducer replace_left(HfstTransducerPair &context,

> [spec:hfst:sem:hfst-rules.hfst.rules.replace-left-fn]
> Thin wrapper. Returns
> `replace_in_context(context, REPL_LEFT, mapping, optional, alphabet)`. Passes
> the context pair, mapping, optional flag and alphabet straight through with
> replace type `REPL_LEFT`.

> [spec:hfst:def:hfst-rules.hfst.rules.replace-right-fn]
> HfstTransducer replace_right(HfstTransducerPair &context,

> [spec:hfst:sem:hfst-rules.hfst.rules.replace-right-fn]
> Thin wrapper. Returns
> `replace_in_context(context, REPL_RIGHT, mapping, optional, alphabet)`. Passes
> the context pair, mapping, optional flag and alphabet straight through with
> replace type `REPL_RIGHT`.

> [spec:hfst:def:hfst-rules.hfst.rules.replace-transducer-fn]
> HfstTransducer replace_transducer(HfstTransducer &t,

> [spec:hfst:sem:hfst-rules.hfst.rules.replace-transducer-fn]
> Builds a replace transducer for mapping `t` bracketed by left marker `lm` and
> right marker `rm`. Steps:
> 1. `t.optimize()` (MUTATES the passed `t`). `type` = `t.get_type()`.
> 2. `tc` = copy of `t` with `StringPair(rm,rm)` inserted freely, then
>    `StringPair(lm,lm)` inserted freely.
> 3. `tm` = single-symbol transducer for `lm`; `rmtr` = single-symbol transducer
>    for `rm`. Concatenate `tc` onto `tm`, then concatenate `rmtr`, giving
>    `tm = lm tc rm` (i.e. `L (L>>(R>>t)) R`). `tm.optimize()`.
> 4. `retval` = `replace(tm, repl_type, false, alphabet)` (non-optional).
> 5. `retval.optimize()` and return `retval`.

> [spec:hfst:def:hfst-rules.hfst.rules.replace-up-fn]
> HfstTransducer replace_up(HfstTransducerPair &context,

> [spec:hfst:sem:hfst-rules.hfst.rules.replace-up-fn]
> Thin wrapper. Returns
> `replace_in_context(context, REPL_UP, mapping, optional, alphabet)`. Passes the
> context pair, mapping, optional flag and alphabet straight through with replace
> type `REPL_UP`.

> [spec:hfst:def:hfst-rules.hfst.rules.restriction-and-coercion-fn]
> HfstTransducer restriction_and_coercion(HfstTransducerPairVector &contexts,

> [spec:hfst:sem:hfst-rules.hfst.rules.restriction-and-coercion-fn]
> Thin wrapper. Returns `restriction(contexts, mapping, alphabet, twol_both, 0)`,
> i.e. calls the general `restriction` helper with twol type `twol_both` and
> direction `0`. Parameters `contexts`, `mapping`, `alphabet` are passed by
> reference unchanged.

> [spec:hfst:def:hfst-rules.hfst.rules.restriction-fn]
> HfstTransducer restriction(HfstTransducerPairVector &contexts,

> [spec:hfst:sem:hfst-rules.hfst.rules.restriction-fn]
> General two-level restriction/coercion builder. Parameters: `contexts`
> (HfstTransducerPairVector), `mapping` (HfstTransducer&), `alphabet`
> (StringPairSet&), `twol_type` (twol_right / twol_left / twol_both), `direction`
> (int: 0, 1, or -1). Steps:
> 1. Determine `type`: iterate `contexts`; the first pair's `first.get_type()`
>    fixes `type`. For every pair, if any `first` or `second` type differs from
>    `type`, throw `TransducerTypeMismatchException("rules::restriction")`. If
>    `contexts` is empty (type never defined), throw
>    `EmptySetOfContextsException("rules::restriction")`.
> 2. `marker = "@_MARKER_@"`; `mt` = single-symbol transducer for `marker`;
>    `pi_star` = universal `.*` over `alphabet` of `type`.
> 3. Center transducer `l1` = `internal_epsilon` then concatenate `pi_star`, `mt`,
>    `mapping`, `mt`, `pi_star` (i.e. `.* <M> mapping <M> .*`).
> 4. Build `tmp` by direction: if `direction==0`, `tmp = pi_star`. If
>    `direction==1`, `tmp = mapping.input_project().compose(pi_star)` (NOTE:
>    `mapping.input_project()` mutates `mapping`). Else (e.g. -1), `tmp = pi_star`
>    then `tmp.compose(mapping.output_project())` (mutates `mapping`).
> 5. Context transducer `l2`: for each context pair, build `ct` = `internal_epsilon`
>    then concatenate `pi_star`, `it->first`, `mt`, `tmp`, `mt`, `it->second`,
>    `pi_star`; disjunct each `ct` into `l2`.
> 6. Produce result by `twol_type`:
>    - `twol_right`: `retval` = `pi_star` (over alphabet) minus
>      `((l1 - l2).substitute(marker, internal_epsilon))`; return it.
>    - `twol_left`: `retval` = `pi_star` minus
>      `((l2 - l1).substitute(marker, internal_epsilon))`; return it.
>    - `twol_both`: `retval1` = `pi_star` minus `((l1 - l2).substitute(...))`;
>      `retval2` = `pi_star` minus `((l2 - l1).substitute(...))`; return
>      `retval1.intersect(retval2)`.
>    - otherwise: `assert(false)` and return an empty `HfstTransducer(type)`.

> [spec:hfst:def:hfst-rules.hfst.rules.surface-coercion-fn]
> HfstTransducer surface_coercion(HfstTransducerPairVector &contexts,

> [spec:hfst:sem:hfst-rules.hfst.rules.surface-coercion-fn]
> Thin wrapper. Returns `restriction(contexts, mapping, alphabet, twol_left, 1)`,
> i.e. calls the general `restriction` helper with twol type `twol_left` and
> direction `1` (surface/input side). Parameters passed by reference unchanged.

> [spec:hfst:def:hfst-rules.hfst.rules.surface-restriction-and-coercion-fn]
> HfstTransducer surface_restriction_and_coercion

> [spec:hfst:sem:hfst-rules.hfst.rules.surface-restriction-and-coercion-fn]
> Thin wrapper. Returns `restriction(contexts, mapping, alphabet, twol_both, 1)`,
> i.e. calls the general `restriction` helper with twol type `twol_both` and
> direction `1` (surface/input side). Parameters passed by reference unchanged.

> [spec:hfst:def:hfst-rules.hfst.rules.surface-restriction-fn]
> HfstTransducer surface_restriction(HfstTransducerPairVector &contexts,

> [spec:hfst:sem:hfst-rules.hfst.rules.surface-restriction-fn]
> Thin wrapper. Returns `restriction(contexts, mapping, alphabet, twol_right, 1)`,
> i.e. calls the general `restriction` helper with twol type `twol_right` and
> direction `1` (surface/input side). Parameters passed by reference unchanged.

> [spec:hfst:def:hfst-rules.hfst.rules.two-level-if-and-only-if-fn]
> HfstTransducer two_level_if_and_only_if(HfstTransducerPair &context,

> [spec:hfst:sem:hfst-rules.hfst.rules.two-level-if-and-only-if-fn]
> Computes `if_rule = two_level_if(context, mappings, alphabet)` and
> `only_if_rule = two_level_only_if(context, mappings, alphabet)`, then returns
> `if_rule.intersect(only_if_rule)` (their intersection). Parameters `context`,
> `mappings`, `alphabet` are forwarded by reference to both helpers.

> [spec:hfst:def:hfst-rules.hfst.rules.two-level-if-fn]
> HfstTransducer two_level_if(HfstTransducerPair &context,

> [spec:hfst:sem:hfst-rules.hfst.rules.two-level-if-fn]
> Builds the two-level "if" (=>) rule, equivalent to `![ .* l [a:. & !a:b] r .* ]`.
> Steps:
> 1. If `context.first.get_type() != context.second.get_type()`, throw
>    `TransducerTypeMismatchException("rules::two_level_if")`. `type` =
>    `context.first.get_type()`.
> 2. Build `input_to_any`: for each mapping pair in `mappings`, scan `alphabet`,
>    and for every alphabet pair whose `first` equals the mapping's `first`, insert
>    that alphabet pair into `input_to_any` (i.e. the set `a:.` of all output
>    realizations of each mapped input symbol).
> 3. `center` = transducer from `input_to_any` (`a:.`).
> 4. `neg_mappings` = universal `.*` over `alphabet` (harmonize flag true) minus the
>    transducer built from `mappings` (i.e. `.* - a:b`).
> 5. `center.intersect(neg_mappings)` so `center == a:. & !a:b`.
> 6. `left_context` = universal `.*` over `alphabet` concatenated with
>    `context.first` (`.* l`).
> 7. `right_context` = `context.second` concatenated with a universal `.*`
>    (`r .*`).
> 8. `inside` = `left_context` concatenated with `center` concatenated with
>    `right_context`.
> 9. `retval` = `universal` (the `.*` reused from step 7) minus `inside`. Return it.
> Note: two `assert(context.second.get_type() != ERROR_TYPE)` checks are present
> (duplicated).

> [spec:hfst:def:hfst-rules.hfst.rules.two-level-only-if-fn]
> HfstTransducer two_level_only_if(HfstTransducerPair &context,

> [spec:hfst:sem:hfst-rules.hfst.rules.two-level-only-if-fn]
> Builds the two-level "only if" (<=) rule, equivalent to
> `!(!(.* l) a:b .* | .* a:b !(r .*))`. Steps:
> 1. If `context.first.get_type() != context.second.get_type()`, throw
>    `TransducerTypeMismatchException("rules::two_level_only_if")`. `type` =
>    `context.first.get_type()`.
> 2. `center` = transducer from `mappings` (`a:b`).
> 3. `left` = universal `.*` over `alphabet` concatenated with `context.first`
>    (`.* l`); `left_neg` = universal `.*` minus `left` (`!(.* l)`).
> 4. `universal` = universal `.*` over `alphabet`. `right` = `context.second`
>    concatenated with `universal` (`r .*`); `right_neg` = universal `.*` minus
>    `right` (`!(r .*)`).
> 5. `rule` = `left_neg` concatenated with `center` concatenated with `universal`
>    (`!(.* l) a:b .*`). `rule_right` = `universal` concatenated with `center`
>    concatenated with `right_neg` (`.* a:b !(r .*)`). Disjunct `rule_right` into
>    `rule`.
> 6. `rule_neg` = universal `.*` over `alphabet` minus `rule`. Return `rule_neg`.
> Note: two duplicated `assert(context.second.get_type() != ERROR_TYPE)` checks
> are present.

> [spec:hfst:def:hfst-rules.left-arrow-test1-fn]
> void left_arrow_test1( ImplementationType type )

> [spec:hfst:sem:hfst-rules.left-arrow-test1-fn]
> MAIN_TEST-only unit test for left-arrow ("<-") rules with a non-trivial context.
> Builds a tokenizer `TOK` with multichar symbol `@_EPSILON_SYMBOL_@`; mapping
> `ca:d`; context pair (left `ca`, right `c`); alphabet `{a:a, c:c, d:d}`; inputs
> `input1="cacacac"` and `input2="cac"`. Constructs expected result transducers
> `result_left1..4` (and `_optional`/`_WithoutContext` variants) as literal
> input:output string-pair transducers, where optional variants disjunct the
> identity input and the `\/` (down) result is the disjunction-then-minimize of the
> `\\` (left) and `//` (right) results. Builds left-arrow rule transducers via
> `left_replace_up`, `left_replace_down_karttunen`, `left_replace_left`,
> `left_replace_right` (with and without context, optional and non-optional). For
> each rule, composes the rule transducer with the input on the LEFT
> (`rule.compose(input).minimize()`) and `assert`s the composed result `compare()`s
> equal to the corresponding expected result. Returns void; effect is the
> assertions (aborts on failure).

> [spec:hfst:def:hfst-rules.left-arrow-test2-fn]
> void left_arrow_test2( ImplementationType type )

> [spec:hfst:sem:hfst-rules.left-arrow-test2-fn]
> MAIN_TEST-only unit test for left-arrow rules where BOTH contexts are epsilon
> transducers (`@_EPSILON_SYMBOL_@`). Tokenizer with `@_EPSILON_SYMBOL_@`; mapping
> `a:d`; context pair (epsilon, epsilon); alphabet `{a:a, c:c, d:d}`; input
> `input1="caadaaa"`. Expected `result1 = "cdddddd":"caadaaa"` and
> `result1Optional` = `result1` disjuncted with `input1` (minimized). Builds
> `left_replace_up/down_karttunen/left/right` rules (optional and non-optional) for
> the epsilon context. For each, composes the rule with input on the left
> (`rule.compose(input1).minimize()`) and `assert`s equality to `result1` (or
> `result1Optional` for optional variants). Returns void.

> [spec:hfst:def:hfst-rules.left-arrow-test3-fn]
> void left_arrow_test3( ImplementationType type )

> [spec:hfst:sem:hfst-rules.left-arrow-test3-fn]
> MAIN_TEST-only unit test for left-arrow rules where the LEFT context is an
> epsilon transducer and the right context is `d`. Tokenizer with
> `@_EPSILON_SYMBOL_@`; mapping `a:d`; context pair (epsilon, `d`); alphabet
> `{a:a, c:c, d:d}`; input `input1="caadaaa"`. Expected `result1 =
> "caddaaa":"caadaaa"`, `result2 = "cdddaaa":"caadaaa"`, each with `*Optional`
> variants disjuncting `input1`. Builds the four left-arrow rules (optional and
> non-optional). For each, composes rule with input on the left and `assert`s the
> result: up `//` give `result1`/`result1Optional`; left `\\` and down `\/` give
> `result2`/`result2Optional`. Returns void.

> [spec:hfst:def:hfst-rules.left-arrow-test4-fn]
> void left_arrow_test4( ImplementationType type )

> [spec:hfst:sem:hfst-rules.left-arrow-test4-fn]
> MAIN_TEST-only unit test for left-arrow rules where the RIGHT context is an
> epsilon transducer and the left context is `d`. Tokenizer with
> `@_EPSILON_SYMBOL_@`; mapping `a:d`; context pair (`d`, epsilon); alphabet
> `{a:a, c:c, d:d}`; input `input1="caadaaa"`. Expected `result1 =
> "caaddaa":"caadaaa"`, `result2 = "caadddd":"caadaaa"`, each with `*Optional`
> variants disjuncting `input1`. Builds the four left-arrow rules (optional and
> non-optional). For each, composes rule with input on the left and `assert`s:
> up `\\` give `result1`/`result1Optional`; right `//` and down `\/` give
> `result2`/`result2Optional`. Returns void.

> [spec:hfst:def:hfst-rules.main-fn]
> int main(int argc, char * argv[])

> [spec:hfst:sem:hfst-rules.main-fn]
> MAIN_TEST-only test driver. Prints `"Unit tests for <__FILE__>:"`. Defines the
> array of implementation types `{SFST_TYPE, TROPICAL_OPENFST_TYPE, FOMA_TYPE}`
> with count 3. Loops over the three types; for each, if
> `HfstTransducer::is_implementation_type_available(type)` is false it `continue`s
> (skips). Otherwise it runs the eight test functions in order:
> `right_arrow_test1`, `left_arrow_test1`, `right_arrow_test2`, `left_arrow_test2`,
> `right_arrow_test3`, `left_arrow_test3`, `right_arrow_test4`, `left_arrow_test4`,
> each passed the current type. After the loop, prints `"ok"` and returns 0.

> [spec:hfst:def:hfst-rules.right-arrow-test1-fn]
> void right_arrow_test1( ImplementationType type )

> [spec:hfst:sem:hfst-rules.right-arrow-test1-fn]
> MAIN_TEST-only unit test for right-arrow ("->") rules with a non-trivial context.
> Tokenizer with multichar `@_EPSILON_SYMBOL_@`; mapping `ca:d`; context pair
> (left `ca`, right `c`); alphabet `{a:a, c:c, d:d}`; inputs `input1="cacacac"`,
> `input2="cac"`. Builds right-arrow rule transducers via `replace_up`,
> `replace_down_karttunen`, `replace_left`, `replace_right` (with context, plus
> `replace_up(mapping,...)` without context), optional and non-optional. Constructs
> expected `result_right1..4` literal string-pair transducers, with optional
> variants disjuncting the identity input and the `\/` (down) result being the
> minimized disjunction of the `\\` and `//` results. For each rule, composes the
> input on the LEFT (`input.compose(rule).minimize()`) and `assert`s the result
> `compare()`s equal to the corresponding expected transducer: up `||`, up without
> context, left `\\`, right `//`, down `\/`, and their optional `(->)` variants.
> Returns void.

> [spec:hfst:def:hfst-rules.right-arrow-test2-fn]
> void right_arrow_test2( ImplementationType type )

> [spec:hfst:sem:hfst-rules.right-arrow-test2-fn]
> MAIN_TEST-only unit test for right-arrow rules where BOTH contexts are epsilon
> transducers. Tokenizer with `@_EPSILON_SYMBOL_@`; context pair (epsilon,
> epsilon); mapping `a:d`; alphabet `{a:a, c:c, d:d}`; input `input1="caadaaa"`.
> Expected `result1 = "caadaaa":"cdddddd"` and `result1Optional` = `result1`
> disjuncted with `input1` (minimized). Builds `replace_up`,
> `replace_down_karttunen`, `replace_left`, `replace_right` (optional and
> non-optional) for the epsilon context. For each, composes input on the left
> (`input1.compose(rule).minimize()`) and `assert`s equality to `result1` (or
> `result1Optional` for optional variants), across `||`, `\\`, `//`, `\/`.
> Returns void.

> [spec:hfst:def:hfst-rules.right-arrow-test3-fn]
> void right_arrow_test3( ImplementationType type )

> [spec:hfst:sem:hfst-rules.right-arrow-test3-fn]
> MAIN_TEST-only unit test for right-arrow rules where the LEFT context is an
> epsilon transducer and the right context is `d`. Tokenizer with
> `@_EPSILON_SYMBOL_@`; context pair (epsilon, `d`); mapping `a:d`; alphabet
> `{a:a, c:c, d:d}`; input `input1="caadaaa"`. Expected `result1 =
> "caadaaa":"caddaaa"`, `result2 = "caadaaa":"cdddaaa"`, each with `*Optional`
> variants disjuncting `input1`. Builds the four context replace rules (optional
> and non-optional). For each, composes input on the left and `assert`s: up `||`
> and right `//` give `result1`/`result1Optional`; left `\\` and down `\/` give
> `result2`/`result2Optional`. Returns void.

> [spec:hfst:def:hfst-rules.right-arrow-test4-fn]
> void right_arrow_test4( ImplementationType type )

> [spec:hfst:sem:hfst-rules.right-arrow-test4-fn]
> MAIN_TEST-only unit test for right-arrow rules where the RIGHT context is an
> epsilon transducer and the left context is `d`. Tokenizer with
> `@_EPSILON_SYMBOL_@`; context pair (`d`, epsilon); mapping `a:d`; alphabet
> `{a:a, c:c, d:d}`; input `input1="caadaaa"`. Expected `result1 =
> "caadaaa":"caaddaa"`, `result2 = "caadaaa":"caadddd"`, each with `*Optional`
> variants disjuncting `input1`. Builds the four context replace rules (optional
> and non-optional). For each, composes input on the left and `assert`s: up `||`
> and left `\\` give `result1`/`result1Optional`; right `//` and down `\/` give
> `result2`/`result2Optional`. Returns void.

