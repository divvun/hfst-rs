# Flag elimination

`eliminate_flags` and `eliminate_flag` turn flag diacritics into structure.
They are the library side of `hfst eliminate-flags`, of its `-F` option and of
the xfst `eliminate flag` command. The result carries no flags of the
eliminated features and keeps exactly the paths whose flags were consistent.

Upstream builds one filter transducer for all constrained flags by
intersecting a small filter per flag, then composes that filter on both sides
of the input. The intersection holds every combination of every feature's
states, so its size is the product of the per-feature sizes. An
error-correction pair relation built by composing an analyser with a generator
carries each lexicon feature twice, once per operand, and the filter for one
such relation (84 flags in 29 features, 9,428 states, acyclic) did not finish
in upstream 3.16.2 or in the earlier port: both ran for minutes past 20 GB.
lang-sme's equivalent relation (101 flags, 259,666 states, cyclic) took the
earlier port 363 s and 21.6 GB, and upstream did not finish in 500 s. The
paths that survive are few: printing every consistent path of the first
relation takes under a second.

## The relation

> [spec:hfst:req:flag-elimination.relation]
> Eliminating the flags of one feature, or of every feature, must give the
> weighted relation of the paths that the flag filter accepts on the input tape
> and on the output tape, each tape checked on its own. Every arc that carries
> a flag of an eliminated feature on either side becomes an epsilon arc of the
> same weight, and those flags leave the alphabet. Arc and final weights are
> those of the original paths. The result is then optimized.
>
> The filter is the conjunction of one constraint for each U, R or D flag `f`
> of an eliminated feature that some flag makes fail or succeed, as
> [spec:hfst:sem:hfst-transducer.hfst.flag-build-fn] classifies each pair.
> Reading `f` is refused when the last flag read before it on the same tape
> that makes `f` fail or succeed was a fail flag. An R flag is also refused
> when no such flag was read before it. A U, R or D flag that no flag makes
> fail or succeed is not constrained. Every other symbol, including epsilon,
> the identity and unknown symbols, and the flags of features that are not
> being eliminated, leaves the filter state unchanged.

## The cost

> [spec:hfst:req:flag-elimination.reachable-product]
> Elimination must not build a filter over all combinations of flag values.
> It must apply the constraints while walking the transducer from its start
> state, so that its cost follows the triples (state, input-tape filter state,
> output-tape filter state) reachable from the start. A filter state records,
> for each constraint, whether its flag may be read now, and is built only when
> the walk reaches it. Flags of different features that never occur on the
> same path must not multiply the cost.
