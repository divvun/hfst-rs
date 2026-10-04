# XRE replace pass

A Giella speller's error model includes "strings" rule files. Each holds one
parallel optional replace rule `P`, often with thousands of `L (->) R::w`
mappings, and the build compiles it as `?* P ?*`. The classic compile of that
expression is determinised, and it grows with every overlap between
left-hand sides: the lang-sme orthographic error rules alone come to 380,677
arcs. The same relation fits in a few thousand arcs when it is left
nondeterministic. A speller determinises lazily while it searches, so it does
not need the expanded form.

The replace-pass mode builds that small form. It is a different
representation of the same relation, so it has to agree with the classic
compile on every input, and it has to refuse what it cannot represent rather
than compile something else.

## Relation

> [spec:hfst:req:xre-replace-pass.relation]
> In replace-pass mode an expression that is one parallel optional replace
> rule `P` MUST compile to the weighted relation of `?* P ?*` as the classic
> compile builds it, for every input string. A pass chooses any set of
> non-overlapping occurrences of the rules' left-hand sides, adjacent ones
> included and the empty set meaning identity. It rewrites each chosen
> occurrence to its right-hand side and copies every other symbol. No rule
> applies to another rule's output. The weight is the sum of the chosen
> mappings' weights, and several ways of producing the same output keep the
> least. The mode MUST NOT repeat the pass; repetition belongs to the caller.

> [spec:hfst:req:xre-replace-pass.boundary-contexts]
> Inside `?* P ?*` the boundary `.#.` of a context is the edge of `P`'s own
> domain, not of the word, and the pass MUST keep that meaning. A rule with
> the context `_ .#.` may make only the last rewrite of the pass, and the rest
> of the word is copied after it: with `{beal} (->) {bealde}::1 || _ .#.`,
> `bealgo` gives `bealdego` at weight 1. A rule with `.#. _` may make only the
> first rewrite, and one with `.#. _ .#.` only the sole rewrite. A rule with
> several contexts may rewrite wherever any of them allows.

## Shape

> [spec:hfst:req:xre-replace-pass.shape]
> The pass MUST NOT be determinised. It has a start state, a state for
> rewrites after the first when some rule depends on being first, and an end
> state when some rule must be last. Each of these is final and copies any
> one symbol in a loop. Each transition between them is the union of the
> cross-products `L .x. R` of the mappings that may rewrite there, minimised
> on its own and entered and left by epsilon. A rule's weight stays on its
> body; the loops weigh nothing.

## Refusals

> [spec:hfst:req:xre-replace-pass.refusals]
> Anything the pass cannot represent exactly MUST be an error at its source
> position, naming the rule (counting `,,`-separated rules from 1) and the
> mapping within it (counting `,`-separated mappings from 1). There is no
> fallback to the classic compile and nothing is dropped. The errors are: a
> root that is not one replace rule, grouping brackets aside; any arrow other
> than `(->)`, including the obligatory `->`; a markup mapping; a context
> mark other than `||`; a context other than `_ .#.`, `.#. _` or
> `.#. _ .#.`; a left-hand side that matches the empty string; a mapping that
> matches nothing; the boundary `.#.` inside a mapping; a flag diacritic in a
> mapping; and a mapping that does not compile. Every problem in an
> expression is reported before it fails. In a semicolon-separated file the
> position is the file's own line and column.

## Command line

> [spec:hfst:req:xre-replace-pass.cli]
> `hfst regexp2fst --replace-pass` compiles each expression of its input as
> one pass, in line-separated and in semicolon-separated (`-S`) mode alike,
> and writes one pass per expression. Any expression that is refused makes
> the tool exit non-zero.
