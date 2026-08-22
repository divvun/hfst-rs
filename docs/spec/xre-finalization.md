# XRE root finalization

The XRE grammar optimizes at more than one point, and a port that walks an AST
instead of reducing a grammar has to reproduce the resulting *count*, not merely
the fact that optimization happened somewhere.

`REGEXP2` is the nonterminal every complete expression reduces through, and
every one of its productions in `xre_parse.yy` ends its action with
`optimize()`.  A few of those productions are themselves operators —
composition, cross product, lenient composition, and the two merges — and for
those the operator's own `optimize()` *is* the reduction's.  Everything else
reaches `REGEXP2` through the pass-through chain and is optimized there.  A
bracketed group is one of the everything-else cases: `[ ... ]` is a `REGEXP11`
production with its own `optimize()`, which does not discharge the `REGEXP2`
one that follows it.

The count is observable, because optimization is not idempotent under
`encode_weights`.  In that mode the weight is folded into the label before the
subset construction, so `Minimize` receives a weighted acceptor: it pushes
weights toward the initial state and refines its partition against *that*
intermediate weight distribution rather than the machine's own.  A second
application therefore finds merges the first could not see.  Upstream C++ HFST
3.17.1 behaves the same way — the two implementations agree state for state at
every count — which is precisely why the count is the thing that has to match.

## The root is optimized

> [spec:hfst:req:xre-finalization.root-optimize]
> A compiled XRE root MUST be optimized at the compiler boundary, on top of
> whatever its own evaluation already did, because that boundary stands in for
> the `REGEXP2` reduction every complete expression passes through.  The
> boundary step MAY be skipped only for a root whose own final action already
> performed it — the operators that ARE `REGEXP2` productions: composition,
> cross product, lenient composition, and both merges.  A bracketed group MUST
> NOT be treated as one of them: its optimization belongs to the bracket, not
> to the reduction, and the boundary step still applies.  Skipping it returns a
> machine minimized against an intermediate weight distribution instead of its
> own, which under `encode_weights` means surplus states and surplus
> non-minimal-weight paths — measured on a Giella speller error model as 8
> surplus states, 124,223 surplus transitions, and 2,109 duplicate paths for a
> single probe word, against an output whose state, transition and final-state
> counts otherwise match the oracle's exactly.
