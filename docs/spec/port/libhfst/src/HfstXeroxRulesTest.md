# libhfst/src/HfstXeroxRulesTest.cc

> [spec:hfst:def:hfst-xerox-rules-test.after-test1-fn]
> void after_test1( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.after-test1-fn]
> Test helper for the "before" Xerox rule `a < b` over implementation `type`. Build a default `HfstTokenizer TOK`. Build single-char transducers `left="a"` and `right="b"`, inputs `input1="ba"`, `input2="bca"`, `input3="ab"`, `input4="acb"`, and an empty transducer `empty(type)`. Build the rule transducer `afterTr = before(left, right)`. For each input, copy it into `tmp2`, do `tmp2.compose(afterTr).minimize()`, and assert: input1 maps to input1, input2 maps to input2, input3 maps to empty, input4 maps to empty. (Note: despite the "after" name it calls `before`, and the inputs are the reversed cases of `before_test1`.) No return value; asserts on failure.

> [spec:hfst:def:hfst-xerox-rules-test.before-test1-fn]
> void before_test1( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.before-test1-fn]
> Test helper for the "before" Xerox rule `a < b` over implementation `type`. Build a default `HfstTokenizer TOK`. Build single-char transducers `left="a"` and `right="b"`, inputs `input1="ab"`, `input2="acb"`, `input3="ba"`, `input4="bca"`, and an empty transducer `empty(type)`. Build the rule transducer `beforeTr = before(left, right)`. For each input, copy it into `tmp2`, do `tmp2.compose(beforeTr).minimize()`, and assert: input1 maps to input1, input2 maps to input2, input3 maps to empty, input4 maps to empty (i.e. strings where an `a` appears before a `b` are accepted, otherwise the language is empty). No return value; asserts on failure.

> [spec:hfst:def:hfst-xerox-rules-test.restriction-test1-fn]
> void restriction_test1( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.restriction-test1-fn]
> Test helper for restriction rule `a => b _ c` over implementation `type`. Build `HfstTokenizer TOK` and register multichar symbol `@_EPSILON_SYMBOL_@`. Center `= "a"`. Build one context pair `(left="b", right="c")` and push it into a `HfstTransducerPairVector`. Inputs: `input1="bac"`, `input2="abc"`, `input3="abac"`, `input4="bcab"`; `result1="bac"`; `empty(type)`. Build `restrictionTr = restriction(center, ContextVector)`. For each input copy into `tmp2`, do `tmp2.compose(restrictionTr).minimize()`, assert: input1 maps to result1 (`bac`), input2/input3/input4 map to empty. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.restriction-test1a-fn]
> void restriction_test1a( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.restriction-test1a-fn]
> Test helper for restriction rule `a => b k _ c` over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`. Center `= "a"`. One context pair `(left="bk", right="c")`. Inputs: `input1="bkac"`, `input2="abkc"`, `input3="abkac"`, `input4="bkcabk"`; `result1="bkac"`; `empty(type)`. Build `restrictionTr = restriction(center, ContextVector)`. For each input compose with `restrictionTr` and minimize, assert: input1 maps to result1 (`bkac`), input2/input3/input4 map to empty. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.restriction-test1b-fn]
> void restriction_test1b( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.restriction-test1b-fn]
> Test helper for restriction rule `a => bb _ bb` over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`. Center `= "a"`. One context pair `(left="bb", right="bb")`. Inputs: `input1="bbabb"`, `input2="abb"`, `input3="abbabb"`, `input4="bbbbab"`; `result1="bbabb"`; `empty(type)`. Build `restrictionTr = restriction(center, ContextVector)`. For each input compose with `restrictionTr` and minimize, assert: input1 maps to result1 (`bbabb`), input2/input3/input4 map to empty. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.restriction-test2-fn]
> void restriction_test2( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.restriction-test2-fn]
> Test helper for restriction rule `a k => b _ c` over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`. Center `= "ak"`. One context pair `(left="b", right="c")`. Inputs: `input1="bakc"`, `input2="akbc"`, `input3="akbakc"`, `input4="bcak"`; `result1="bakc"`; `empty(type)`. Build `restrictionTr = restriction(center, ContextVector)`. For each input compose with `restrictionTr` and minimize, assert: input1 maps to result1 (`bakc`), input2/input3/input4 map to empty. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.restriction-test3-fn]
> void restriction_test3( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.restriction-test3-fn]
> Test helper for restriction rule `b => b _ c` over implementation `type` (center symbol equals the left context symbol). Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`. Center `= "b"`. One context pair `(left="b", right="c")`. Inputs: `input1="c"`, `input2="bc"`, `input3="bbc"`, `input4="cb"`; `result1="c"`; `empty(type)`. Build `restrictionTr = restriction(center, ContextVector)`. For each input compose with `restrictionTr` and minimize, assert: input1 maps to result1 (`c`), input2/input3/input4 map to empty. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.restriction-test3a-fn]
> void restriction_test3a( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.restriction-test3a-fn]
> Test helper for restriction rule `a => a _` (empty right context) over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`. Center `= "a"`; build `epsilon = "@_EPSILON_SYMBOL_@"`. One context pair `(left="a", right=epsilon)`. Inputs: `input1="c"`, `input2="aa"`, `input3="a"`, `input4="aca"`; `result1="c"`; `empty(type)`. Build `restrictionTr = restriction(center, ContextVector)`. For each input compose with `restrictionTr` and minimize, assert: input1 maps to result1 (`c`), input2/input3/input4 map to empty. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.restriction-test3b-fn]
> void restriction_test3b( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.restriction-test3b-fn]
> Test helper for restriction rule `a b => a b _` (empty right context) over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`. Center `= "ab"`; build `epsilon = "@_EPSILON_SYMBOL_@"`. One context pair `(left="ab", right=epsilon)`. Inputs: `input1="ba"`, `input2="ab"`, `input3="abab"`, `input4="abc"`; `result1="ba"`; `empty(type)`. Build `restrictionTr = restriction(center, ContextVector)`. For each input compose with `restrictionTr` and minimize, assert: input1 maps to result1 (`ba`), input2/input3/input4 map to empty. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.restriction-test3c-fn]
> void restriction_test3c( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.restriction-test3c-fn]
> Test helper for restriction rule `a b => _ a b` (empty left context) over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`. Center `= "ab"`; build `epsilon = "@_EPSILON_SYMBOL_@"`. One context pair `(left=epsilon, right="ab")`. Inputs: `input1="ba"`, `input2="ab"`, `input3="abab"`, `input4="abc"`; `result1="ba"`; `empty(type)`. Build `restrictionTr = restriction(center, ContextVector)`. For each input compose with `restrictionTr` and minimize, assert: input1 maps to result1 (`ba`), input2/input3/input4 map to empty. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.restriction-test4-fn]
> void restriction_test4( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.restriction-test4-fn]
> Test helper for restriction rule with two contexts `a => b _ c , j _ k` over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`. Center `= "a"`. Two context pairs `Context1=(b, c)` and `Context2=(j, k)`, pushed in that order. Inputs: `input1="bac"`, `input2="jak"`, `input3="bacjak"`, `input4="bajc"`; results `result1="bac"`, `result2="jak"`, `result3="bacjak"`; `empty(type)`. Build `restrictionTr = restriction(center, ContextVector)`. For each input compose with `restrictionTr` and minimize, assert: input1 maps to result1, input2 maps to result2, input3 maps to result3, input4 maps to empty. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.restriction-test5-fn]
> void restriction_test5( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.restriction-test5-fn]
> Test helper for restriction rule with two one-sided contexts `a => b _ , _ c` over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`. Center `= "a"`; build `epsilon = "@_EPSILON_SYMBOL_@"`. Two context pairs `Context1=(b, epsilon)` and `Context2=(epsilon, c)`, pushed in that order. Inputs: `input1="bac"`, `input2="ba"`, `input3="ac"`, `input4="abac"`; results `result1="bac"`, `result2="ba"`, `result3="ac"`; `empty(type)`. Build `restrictionTr = restriction(center, ContextVector)`. For each input compose and minimize, assert: input1 maps to result1, input2 maps to result2, input3 maps to result3, input4 maps to empty. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.restriction-test5a-fn]
> void restriction_test5a( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.restriction-test5a-fn]
> Test helper for restriction rule with two one-sided contexts `a => a _ , _ a` over implementation `type` (center equals both context symbols). Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`. Center `= "a"`; build `epsilon = "@_EPSILON_SYMBOL_@"`. Two context pairs `Context1=(a, epsilon)` and `Context2=(epsilon, a)`, pushed in that order. Inputs: `input1="aa"`, `input2="aaa"`, `input3="ba"`, `input4="cac"`; results `result1="aa"`, `result2="aaa"`; `empty(type)`. Build `restrictionTr = restriction(center, ContextVector)`. For each input compose and minimize, assert: input1 maps to result1, input2 maps to result2, input3/input4 map to empty. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.restriction-test6-fn]
> void restriction_test6( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.restriction-test6-fn]
> Test helper for restriction rule with two one-sided contexts `a b => a b _ , _ a b` over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`. Center `= "ab"`; build `epsilon = "@_EPSILON_SYMBOL_@"`. Two context pairs `Context1=("ab", epsilon)` and `Context2=(epsilon, "ab")`, pushed in that order. Inputs: `input1="abab"`, `input2="ab"`, `input3="aba"`, `input4="ababab"`; `result1="abab"`; `empty(type)`. Build `restrictionTr = restriction(center, ContextVector)`. For each input compose and minimize, assert: input1 maps to result1 (`abab`), input2/input3 map to empty, and input4 maps to itself (`ababab`). Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.restriction-test7-fn]
> void restriction_test7( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.restriction-test7-fn]
> Test helper for restriction rule `[ x ?* y ] | [ z ?* v ] => b _ c` over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`. Build `identity` as `HfstTransducer::identity_pair(type)` then `repeat_star().minimize()` (matches any sequence). Build center: `zSthV = z . identity . v` (minimized); `center = x . identity . y` (minimized) then `center.disjunct(zSthV).minimize()`, i.e. any string of form x...y or z...v. One context pair `(b, c)`. Inputs: `input1="bxbzycvc"`, `input2="xy"`, `input3="zv"`, `input4="bxyzvc"`; `result1="bxbzycvc"`; `empty(type)`. Build `restrictionTr = restriction(center, ContextVector)`. For each input compose and minimize, assert: input1 maps to result1, input2/input3/input4 map to empty. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.restriction-test8-fn]
> void restriction_test8( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.restriction-test8-fn]
> Test helper for restriction rule `[ x y | x x y y ] => a _ b , x _ y` over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`. Build `tmp = "xxyy"`, `center = "xy"`, then `center.disjunct(tmp).minimize()` (so center matches `xy` or `xxyy`). Two context pairs `Context1=(a, b)` and `Context2=(x, y)`, pushed in that order. Inputs: `input1="axxyyb"`, `input2="xxyy"`, `input3="xy"`, `input4="xxxyyy"`; `result1="axxyyb"`; `empty(type)`. Build `restrictionTr = restriction(center, ContextVector)`. For each input compose and minimize, assert: input1 maps to result1, input2/input3 map to empty, and input4 maps to itself (`xxxyyy`). Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test1-fn]
> void test1( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test1-fn]
> Test helper for replace rule `ab -> x || ab _ a` over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`. Mapping pair `leftMapping="ab"` to `rightMapping="x"`. One context pair `("ab", "a")`. `input1="abababa"`. Build `result1` as the disjunction (minimized along the way) of: identity `abababa`, `r1tmp` (abababa:abx⟨eps⟩aba), `r2tmp` (abababa:ababx⟨eps⟩a), and `r3tmp` (abababa:abx⟨eps⟩x⟨eps⟩a) — i.e. all optional combinations of replacing the two valid `ab` occurrences. Build `Rule(mappingPairVector, ContextVector, REPL_UP)`. First do the optional unconditional replace `replaceTr = replace(rule, true)`, compose input1 with it, minimize, and assert it equals `result1`. Then do non-optional/leftmost `replaceTr = replace(rule, false)`, compose input1, minimize, and assert it equals `r3tmp` (both occurrences replaced). Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test10a-fn]
> void test10a( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test10a-fn]
> Test helper for empty-language replacement `a -> ~[?*]` over implementation `type`. Build default `HfstTokenizer TOK`. Mapping pair `("a", HfstTransducer(type))` (the right side is the empty-language transducer, so `a` maps to nothing). Build `Rule(mappingPairVector)` with no contexts. Build expected `result1` as the identity-pair star: `identityPair = HfstTransducer::identity_pair(type)`, `result1.repeat_star().minimize()`, then `result1.insert_to_alphabet("a")`. Build `replaceTr = replace(rule, false)` and assert `replaceTr.compare(result1)` (the replace transducer itself equals `?*` with `a` in the alphabet, deleting all `a`s). Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test10b-fn]
> void test10b( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test10b-fn]
> Test helper for empty-language replacement `~[?*] -> a` over implementation `type`. Build default `HfstTokenizer TOK`. Mapping pair `(HfstTransducer(type), "a")` (left side is the empty-language transducer, so nothing is matched). Build `Rule(mappingPairVector)` with no contexts. Build expected `result1` as the identity-pair star: `identityPair = HfstTransducer::identity_pair(type)`, `result1.repeat_star().minimize()` (no `insert_to_alphabet` here). Build `replaceTr = replace(rule, false)` and assert `replaceTr.compare(result1)` (replacing on the empty language is a no-op, yielding plain `?*`). Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test1b-fn]
> void test1b( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test1b-fn]
> Test helper for replace rule `a -> x` (epsilon contexts) over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`. Mapping pair `("a", "x")`. One context pair `(epsilon, epsilon)`. `input1="aaana"`. Build expected optional `result1` from an `HfstBasicTransducer bt` with states 0..5: each `a` position (states 0->1, 1->2, 2->3, 4->5) has two transitions, `a:a` and `a:x`; state 3->4 has `n:n`; state 5 is final (weight 0); `result1 = HfstTransducer(bt, type)`. Build `result2 = "aaana":"xxxnx"` (all `a`s replaced). Build `Rule(mappingPairVector, ContextVector, REPL_UP)`. Steps: (1) `replace(rule, true)` (optional), compose input1, minimize, assert equals `result1`; (2) `replace(rule, false)` (non-optional), compose input1, minimize, assert equals `result2`; (3) `replace_leftmost_longest_match(rule)`, compose input1, minimize, assert equals `result2`; (4) `replace_leftmost_shortest_match(rule)`, compose input1, minimize, assert equals `result2`. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test1c-fn]
> void test1c( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test1c-fn]
> Test helper for replace rule `? -> x` (identity to x) over implementation `type`. Build `HfstTokenizer TOK` with multichar symbols `@_EPSILON_SYMBOL_@`, `@_UNKNOWN_SYMBOL_@`, `@_IDENTITY_SYMBOL_@`. Mapping pair `leftMapping="@_IDENTITY_SYMBOL_@"` to `rightMapping="x"`. Build `Rule(mappingPairVector)` with no contexts. `input1="s"`; expected `result1="s":"x"`. Build `replaceTr = replace(rule, false)`, compose input1 with it, minimize, assert equals `result1` (any single symbol is replaced by `x`). Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test1d-fn]
> void test1d( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test1d-fn]
> Test helper for replace rule `a -> b || .#. _ c` (boundary-anchored context) over implementation `type`. Build `HfstTokenizer TOK` with multichar `.#.`. Mapping pair `("a", "b")`. One context pair `(".#.", "c")`. Inputs: `input1=".#.ac"`, `input2="ac"`; expected `result1=".#.ac":".#.ac"` (the `a` after `.#.` is NOT replaced because the literal boundary symbol is in the string, not a true start anchor), `result2="ac":"bc"`. Build `Rule(mappingPairVector, ContextVector, REPL_UP)` and `replaceTr = replace(rule, false)`. Compose input1, minimize, assert equals `result1`; compose input2, minimize, assert equals `result2`. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test2a-fn]
> void test2a( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test2a-fn]
> Test helper for replace rule `a+ @-> x || a _ a` (and `//`, `\\`, `\/` variants) over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`, `@_UNKNOWN_SYMBOL_@`, `@_IDENTITY_SYMBOL_@`, and markers `@_LM_@`/`@_RM_@`. Mapping: `leftMapping="a"` then `repeat_plus().minimize()` (a+), to `rightMapping="x"`. One context pair `("a", "a")`. Inputs `input1="aaaa"`, `input2="aaaaabaaaa"`, `input3="aaaaabaaaacaaaa"`. Build many expected transducers (r1tmp..r4tmp and result1..result11) as disjunctions of specific aaaa:replacement mappings, plus result4..result7 for the longer inputs. Build four rules over the same mapping/context with directions REPL_UP, REPL_LEFT, REPL_RIGHT, REPL_DOWN. Then: (1) optional replace `replace(rule, true)` for each direction, compose input1, minimize, assert Up==result8, Left==result1, Right==result1, Down==result1; (2) non-optional `replace(rule, false)` for each direction, compose input1, minimize, assert Up==result2, Left==result10, Right==result9, Down==result11; (3) `replace_leftmost_longest_match(ruleUp)`, compose input1/input2/input3, assert ==result3/result4/result6; (4) `replace_leftmost_shortest_match(ruleUp)`, compose input1/input2/input3, assert ==r4tmp/result5/result7. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test2b-fn]
> void test2b( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test2b-fn]
> Test helper for replace rule `a+ b+ | b+ a+ @-> x` over implementation `type`, exercising all four longest/shortest x left/right match modes. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@` and markers `@_LM_@`/`@_RM_@`. Build `aPlus="a".repeat_plus().minimize()`, `bPlus="b".repeat_plus().minimize()`; `mtmp1 = aPlus.concatenate(bPlus)`, `mtmp2 = bPlus.concatenate(aPlus)`; `leftMapping = mtmp1.disjunct(mtmp2).minimize()` to `rightMapping="x"`. `input1="aabbaa"`. Expected `result1..result4` are specific aabbaa:replacement mappings. Build `ruleUp(mappingPairVector)` (no contexts). Then: `replace_leftmost_longest_match(ruleUp)` -> result1; `replace_rightmost_longest_match` -> result2; `replace_leftmost_shortest_match` -> result3; `replace_rightmost_shortest_match` -> result4 (each composed with input1, minimized, asserted). Then an in-context variant `a+ b+ | b+ a+ @-> x \/ _ x`: `input2="aabbaax"`, `result5`, one context pair `(epsilon, "x")`, `ruleDown(mappingPairVector, ContextVector, REPL_DOWN)`; `replace_leftmost_longest_match(ruleDown)`, compose input2, minimize, assert ==result5. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test2c-fn]
> void test2c( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test2c-fn]
> Test helper for replace rule `a+ @-> x || c _` over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`, `@_UNKNOWN_SYMBOL_@`, `@_IDENTITY_SYMBOL_@`, markers `@_LM_@`/`@_RM_@`. Mapping: `leftMapping="a".repeat_plus().minimize()` (a+) to `rightMapping="x"`. One context pair `("c", epsilon)`. `input1="caav"`; expected `result1="caav":"cx⟨eps⟩v"`. Build `ruleUp(mappingPairVector, ContextVector, REPL_UP)`, `replaceTr = replace_leftmost_longest_match(ruleUp)`, compose input1, minimize, assert equals result1. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test3a-fn]
> void test3a( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test3a-fn]
> Test helper for replace rule `a -> b || x _ x` over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@` and markers `@_LM_@`/`@_RM_@`. Mapping pair `("a", "b")`. One context pair `("x", "x")`. `input1="xaxax"`. Build `result1` as the minimized disjunction of identity `xaxax`, `xbxax`, `xaxbx`, and `xbxbx` (all optional combinations of replacing the two `a`s that sit between `x`s). Build `ruleUp(mappingPairVector, ContextVector, REPL_UP)`, optional `replaceTr = replace(ruleUp, true)`, compose input1, minimize, assert equals result1. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test3b-fn]
> void test3b( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test3b-fn]
> Test helper for replace rule `a+ -> b || x _ y , y _ z` (two contexts) over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@` and markers `@_LM_@`/`@_RM_@`. Mapping: `leftMapping="a".repeat_plus().minimize()` (a+) to `rightMapping="b"`. Two context pairs `Context=("x","y")` and `Context2=("y","z")`, pushed in that order. `input1="axayaz"`. Build `result1` as the minimized disjunction of identity `axayaz`, `axbybz`, `axbyaz`, and `axaybz`. Build `ruleUp(mappingPairVector, ContextVector, REPL_UP)`, optional `replaceTr = replace(ruleUp, true)`, compose input1, minimize, assert equals result1. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test3c-fn]
> void test3c( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test3c-fn]
> Test helper for replace rule `a+ -> x || x x _ y y , y _ x` (two contexts) over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@` and markers `@_LM_@`/`@_RM_@`. Mapping: `leftMapping="a".repeat_plus().minimize()` (a+) to `rightMapping="x"`. Two context pairs `Context=("xx","yy")` and `Context2=("y","x")`, pushed in that order. `input1="axxayyax"`. Build `result1` as the minimized disjunction of identity `axxayyax`, `axxayyxx`, `axxxyyax`, and `axxxyyxx`. Build `ruleUp(mappingPairVector, ContextVector, REPL_UP)`, optional `replaceTr = replace(ruleUp, true)`, compose input1, minimize, assert equals result1. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test3d-fn]
> void test3d( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test3d-fn]
> Test helper for replace rule `a -> b` (epsilon/epsilon context, i.e. everywhere) over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@` and markers `@_LM_@`/`@_RM_@`. Mapping pair `("a", "b")`. One context pair `(epsilon, epsilon)`. `input1="xaxax"`. Build `result1` as the minimized disjunction of identity `xaxax`, `xbxax`, `xaxbx`, and `xbxbx`. Build `ruleUp(mappingPairVector, ContextVector, REPL_UP)`, optional `replaceTr = replace(ruleUp, true)`, compose input1, minimize, assert equals result1. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test4a-fn]
> void test4a( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test4a-fn]
> Test helper for replace rule `b -> a || _ a` (and `\\`, `//`, `\/` variants) over implementation `type`, input `bbba`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@` and markers `@_LM_@`/`@_RM_@`. Mapping pair `("b", "a")`. One context pair `(epsilon, "a")`. `input1="bbba"`. Expected: `result1="bbba":"bbaa"`, `result2="bbba":"aaaa"`, `r1Tmp="bbba":"baaa"`; `result3 = input1 disjunct result1` (minimized); `result4 = result3 disjunct result2 disjunct r1Tmp` (minimized). Build four rules over the same mapping/context: REPL_UP, REPL_LEFT, REPL_RIGHT, REPL_DOWN. Then: (1) optional `replace(rule, true)` per direction, compose input1, minimize, assert Up==result3, Left==result4, Right==result3, Down==result4; (2) non-optional `replace(rule, false)` per direction, compose input1, minimize, assert Up==result1, Left==result2, Right==result1, Down==result2. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test4b-fn]
> void test4b( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test4b-fn]
> Test helper for replace rule `b -> a || a _` (left context, and Left/Right/Down variants) over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@` and markers `@_LM_@`/`@_RM_@`. Mapping pair `("b", "a")`. One context pair `("a", epsilon)`. `input1="abbb"`. Expected: `result1="abbb":"aabb"`, `result2="abbb":"aaaa"`, `r1Tmp="abbb":"aaab"`; `result3 = input1 disjunct result1` (minimized); `result4 = result3 disjunct r1Tmp` (minimized) `disjunct result2` (minimized). Build four rules over the same mapping/context: REPL_UP, REPL_LEFT, REPL_RIGHT, REPL_DOWN. Then: (1) optional `replace(rule, true)` per direction, compose input1, minimize, assert Up==result3, Left==result3, Right==result4, Down==result4; (2) non-optional `replace(rule, false)` per direction, compose input1, minimize, assert Up==result1, Left==result1, Right==result2, Down==result2. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test4c-fn]
> void test4c( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test4c-fn]
> Test helper for replace rule `ab -> x || ab _ a` (and Left/Right/Down variants) over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`. Mapping pair `("ab", "x")`. One context pair `("ab", "a")`. `input1="abababa"`. Expected building blocks: `result1="abababa":"abababa"` (identity), `r2tmp="abababa":"ababx⟨eps⟩a"`, `r3tmp="abababa":"abx⟨eps⟩aba"`, `r4tmp="abababa":"abx⟨eps⟩x⟨eps⟩a"`. Derived: `result2 = result1 disjunct r2tmp disjunct r3tmp` (minimized); `result3 = result2 disjunct r4tmp` (minimized); `result4 = r2tmp disjunct r3tmp` (minimized). Build four rules over the same mapping/context: REPL_UP, REPL_LEFT, REPL_RIGHT, REPL_DOWN. Then: (1) optional `replace(rule, true)` per direction, compose input1, minimize, assert Up==result3, Left==result2, Right==result2, Down==result2; (2) non-optional `replace(rule, false)` per direction, compose input1, minimize, assert Up==r4tmp, Left==r2tmp, Right==r3tmp, Down==result4. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test6a-fn]
> void test6a( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test6a-fn]
> Test helper for epenthesis replace rule `0 -> p || m _ k` over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@` and markers `@_LM_@`/`@_RM_@`. Mapping pair `leftMapping="@_EPSILON_SYMBOL_@"` to `rightMapping="p"`. One context pair `("m", "k")`. `input1="mk"`. Expected `result1="m@_EPSILON_SYMBOL_@k":"mpk"` (insert `p` between m and k); `result2 = "mk":"mk" disjunct result1` (minimized). Build `ruleUp(mappingPairVector, ContextVector, REPL_UP)`. Step 1: non-optional `replaceTr = replace(ruleUp, false)`, compose input1, minimize, assert equals result1. Step 2: optional `replaceTr = replace(ruleUp, true)`, compose input1, minimize, assert equals result2. (Epenthesis is handled by the basic replace path.) Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test6b-fn]
> void test6b( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test6b-fn]
> Test helper for epenthesis replace rule `a* -> p` (everywhere, epsilon contexts) over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`, markers `@_LM_@`/`@_RM_@`, and `.#.`. Mapping: `leftMapping="a"` then `repeat_star().minimize()` (a*) to `rightMapping="p"`. One context pair `(epsilon, epsilon)`. `input1="mak"`. Expected `result1` maps `"@_EPSILON_SYMBOL_@m@_EPSILON_SYMBOL_@a@_EPSILON_SYMBOL_@k@_EPSILON_SYMBOL_@"` to `"pmpppkp"` (a `p` epenthesized at every epsilon position, and the `a` itself rewritten). Build `ruleUp(mappingPairVector, ContextVector, REPL_UP)`, non-optional `replaceTr = replace(ruleUp, false)`, compose input1, minimize, assert equals result1. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test6c-fn]
> void test6c( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test6c-fn]
> Test helper for epenthesis replace rule `0 -> b || _ a a` over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@` and markers `@_LM_@`/`@_RM_@`. Mapping pair `leftMapping="@_EPSILON_SYMBOL_@"` to `rightMapping="b"`. One context pair `(epsilon, "aa")`. `input1="aa"`. Expected `result1="@_EPSILON_SYMBOL_@aa":"baa"` (insert `b` before the `aa`). Build `ruleUp(mappingPairVector, ContextVector, REPL_UP)`, non-optional `replaceTr = replace(ruleUp, false)`, compose input1, minimize, assert equals result1. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test7a-fn]
> void test7a( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test7a-fn]
> Test helper for a parallel/sequential rule vector `a -> b , b -> c` over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`. Build two context-free rules: rule1 from mapping pair `("a", "b")`, rule2 from mapping pair `("b", "c")`. Push rule1 then rule2 into a `std::vector<Rule> ruleVector`. `input1="aab"`; expected `result1="aab":"bbc"` (each `a` becomes `b`, the original `b` becomes `c`; mappings apply to the original input simultaneously, not chained). Build `replaceTr = replace(ruleVector, false)` (non-optional), compose input1, minimize, assert equals result1. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test7b-fn]
> void test7b( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test7b-fn]
> Test helper for a rule vector combining epenthesis with replacement `[. .] -> b , a -> c` over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`. Build two context-free rules: rule1 from mapping pair `("@_EPSILON_SYMBOL_@", "b")` (epenthesis of `b`), rule2 from mapping pair `("a", "c")`. Push rule1 then rule2 into `ruleVector`. `input1="a"`; expected `result1="@_EPSILON_SYMBOL_@a@_EPSILON_SYMBOL_@":"bcb"` (a `b` epenthesized before and after, the `a` replaced by `c`). Build `replaceTr = replace(ruleVector, false)` (non-optional), compose input1, minimize, assert equals result1. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test7c-fn]
> void test7c( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test7c-fn]
> Test helper for a rule vector `a+ @-> x , b+ @-> y` plus an in-context variant, over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`. Build mapping1 `leftMapping1="a".repeat_plus().minimize()` to `"x"`, mapping2 `leftMapping2="b".repeat_plus().minimize()` to `"y"`; rule1/rule2 are context-free over these; push into `ruleVector`. `input1="aaabbb"`; `result1="aaabbb":"x⟨eps⟩⟨eps⟩y⟨eps⟩⟨eps⟩"` (longest match: whole run to one symbol), `result1b="aaabbb":"xxxyyy"` (shortest match). Step A: `replace_leftmost_longest_match(ruleVector)`, compose input1, minimize, assert ==result1; `replace_leftmost_shortest_match(ruleVector)`, compose input1, minimize, assert ==result1b. Step B (with contexts `a -> x \/ m _ ,, b -> y || x _`): context pairs `Context1=("m", epsilon)`, `Context2=("x", epsilon)`; inputs `input2="mab"`, `input3="maabb"`; results `result2="mab":"mxb"`, `result3="mab":"mxy"`, `result4 = "maabb":"mx⟨eps⟩bb" disjunct "maabb":"mxabb"` (minimized), `result5 = "maabb":"mx⟨eps⟩yb" disjunct "maabb":"mx⟨eps⟩y⟨eps⟩" disjunct "maabb":"mxabb"` (minimized). Build REPL_UP rules `rule2aUp(mappingPairVector1, ContextVector1)` and `rule2bUp(mappingPairVector2, ContextVector2)` into `ruleVector2`; `replace(ruleVector2, false)`, compose input2 -> assert result2, compose input3 -> assert result4. Build REPL_DOWN rules likewise into `ruleVector3`; `replace(ruleVector3, false)`, compose input2 -> assert result3, compose input3 -> assert result5. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test7d-fn]
> void test7d( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test7d-fn]
> Test helper for an epenthesis rule vector with contexts `[. 0 .] -> a \/ _ b a , a b _ ,, [. 0 .] -> b \/ a _ a` over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`. mapping1 = `("@_EPSILON_SYMBOL_@", "a")`, mapping2 = `("@_EPSILON_SYMBOL_@", "b")`. rule1 uses mappingPairVector1 with two context pairs `Context1a=(epsilon, "ba")` and `Context1b=("ab", epsilon)`, direction REPL_DOWN. rule2 uses mappingPairVector2 with one context pair `Context2=("a", "a")`, direction REPL_DOWN. Push rule1 then rule2 into `ruleVector`. `input1="@_EPSILON_SYMBOL_@"` (the empty string). Build `replaceTr = replace(ruleVector, false)`, compose input1, minimize, assert equals input1 (the empty input yields the empty string — no epenthesis fires). Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test7e-fn]
> void test7e( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test7e-fn]
> Test helper for a rule vector `? -> x , a -> b` (identity-to-x plus a-to-b) over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`, `@_IDENTITY_SYMBOL_@`. rule1 = context-free mapping `("@_IDENTITY_SYMBOL_@", "x")`, rule2 = context-free mapping `("a", "b")`. Push rule1 then rule2 into `ruleVector`. `input1="ak"`; build expected `result1 = "ak":"bx" disjunct "ak":"xx"` (minimized), i.e. the `a` may be matched by either the `a->b` rule or the identity `?->x` rule, and `k` is matched by `?->x`. Build `replaceTr = replace(ruleVector, false)`, compose input1, minimize, assert equals result1. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test7f-fn]
> void test7f( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test7f-fn]
> Test helper for a rule vector `a -> b , b -> a` (simultaneous swap) over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`. rule1 = context-free mapping `("a", "b")`, rule2 = context-free mapping `("b", "a")`. Push rule1 then rule2 into `ruleVector`. `input1="aabbaa"`; expected `result1="aabbaa":"bbaabb"` (each `a` becomes `b` and each `b` becomes `a`, applied to the original input simultaneously). Build `replaceTr = replace(ruleVector, false)`, compose input1, minimize, assert equals result1. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test7g-fn]
> void test7g( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test7g-fn]
> Test helper for a rule vector `a -> b b , a -> b` (two alternative outputs for `a`) over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`. rule1 = context-free mapping `("a", "bb")`, rule2 = context-free mapping `("a", "b")`. Push rule1 then rule2 into `ruleVector`. `input1="a"`; build expected `result1 = "a":"b" disjunct "a@_EPSILON_SYMBOL_@":"bb"` (minimized), i.e. `a` may map either to single `b` or to `bb`. Build `replaceTr = replace(ruleVector, false)`, compose input1, minimize, assert equals result1. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test7h-fn]
> void test7h( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test7h-fn]
> Test helper for epenthesis rule `[..] @-> a` (leftmost-longest single epenthesis) over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`, `@_IDENTITY_SYMBOL_@`. One context-free rule from mapping pair `("@_EPSILON_SYMBOL_@", "a")`. Build `replaceTr = replace_leftmost_longest_match(rule)`. Build expected `result1` directly from an `HfstBasicTransducer bt`: transition state 0->1 on `@_EPSILON_SYMBOL_@:a` (weight 0); transitions state 1->0 on `@_IDENTITY_SYMBOL_@:@_IDENTITY_SYMBOL_@` and on `a:a` (both weight 0); state 1 final (weight 0). `result1 = HfstTransducer(bt, type)`. Assert `replaceTr.compare(result1)`. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test8-fn]
> void test8( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test8-fn]
> Test helper for replace rule `[? - a] -> 0` (every symbol except `a` maps to the empty language) over implementation `type`. Build default `HfstTokenizer TOK`. Build acceptors `a="a"`, `b="b"`, and `identityPair = HfstTransducer::identity_pair(type)`. Build `leftMapping = identityPair` then `leftMapping.subtract(a)` (the identity relation over every symbol except `a`). Mapping pair `(leftMapping, HfstTransducer(type))`, the right side being the empty-language transducer. Build `Rule(mappingPairVector)` with no contexts. `input1` is the acceptor for `"maa"`; `result1` is the acceptor for `"mba"` (built via `HfstTransducer("mba", TOK, type)`). Build non-optional `replaceTr = replace(rule, false)`, copy `input1` into `tmp`, do `tmp.compose(replaceTr).minimize()`, and assert `tmp.compare(result1)`. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test9a-fn]
> void test9a( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test9a-fn]
> Test helper for left-replace rule `d0 <- ca || ca _ c` over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`. Mapping pair `(HfstTransducer("d@_EPSILON_SYMBOL_@", TOK, type), HfstTransducer("ca", TOK, type))` — left side `d0` (d then epsilon), right side `ca`. One context pair `Context=("ca", "c")`. Build `Rule(mappingPairVector, ContextVector, REPL_UP)`. `input1` is the acceptor `"cacacac"`; `result1 = HfstTransducer("cad@_EPSILON_SYMBOL_@d@_EPSILON_SYMBOL_@c", "cacacac", TOK, type)` (a two-tape transducer mapping `cad0d0c` to `cacacac`). Build `replaceTr = replace_left(rule, false)`, copy `replaceTr` into `tmp2`, do `tmp2.compose(input1).minimize()` (note: replaceTr composed WITH input1, since this is a left-replace whose lower side is the surface form), and assert `tmp2.compare(result1)`. Asserts on failure; no return.

> [spec:hfst:def:hfst-xerox-rules-test.test9b-fn]
> void test9b( ImplementationType type )

> [spec:hfst:sem:hfst-xerox-rules-test.test9b-fn]
> Test helper for a left-replace rule vector `b <- a ,, a <- b` (simultaneous swap, left-replace) over implementation `type`. Build `HfstTokenizer TOK` with multichar `@_EPSILON_SYMBOL_@`. rule1 from mapping pair `("b", "a")` (context-free), rule2 from mapping pair `("a", "b")` (context-free). Push rule1 then rule2 into a `std::vector<Rule> ruleVector`. `input1` is the acceptor `"abba"`; `result1 = HfstTransducer("baab", "abba", TOK, type)` (maps `baab` to `abba`). Build `replaceTr = replace_left(ruleVector, false)`, copy `replaceTr` into `tmp2`, do `tmp2.compose(input1).minimize()`, and assert `tmp2.compare(result1)`. Asserts on failure; no return.

