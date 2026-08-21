# Determinization envelope

Determinization and minimization are representation choices, not semantic ones:
both denote exactly the relation their input denoted.  HFST reaches for them
freely — every bracketed XRE subexpression optimizes, every `hfst-minimize`
invocation determinizes first — on the assumption that a deterministic machine
is the cheaper way to hold the same relation.

That assumption fails badly on some inputs.  Determinizing the union of a large
sparse machine with a small dense one gives every surviving state the dense
operand's out-degree, so the minimal deterministic form of a perfectly ordinary
union can be orders of magnitude larger than the nondeterministic one that
denotes it.  The measured case is a Giella speller error model: a 356,823-state
/ 21,460,248-transition union whose minimal deterministic form runs to roughly
1.5e9 transitions — a 30.7 GB artifact built in 26 minutes against 39.8 GB of
resident memory, where the union itself is built in ten seconds.  Upstream C++
HFST 3.17.1 does the same thing, worse.

The rules below state the contract that makes this survivable.  They constrain
the *representation* the operations may spend resources on; they do not give
any operation licence to change the relation it returns.

## Relation preservation

> [spec:hfst:req:determinize-envelope.relation-preserved]
> Determinization and minimization MUST return a machine denoting the same
> weighted relation as their input, on every path through the implementation
> including every resource-exhaustion path.  When the envelope stops a
> construction, the operation MUST return a machine that still denotes that
> relation — the input itself, or the result of an equivalence-preserving retry
> such as the reverse orientation — and MUST NOT return a partial, truncated, or
> otherwise smaller-language machine.  A result that is correct but weaker than
> promised (undeterminized, or determinized but not minimal) MUST be reported on
> the diagnostic channel, so that a caller who cares about the representation
> can see that it did not get one.  Consumers of these operations depend on the
> relation, not on determinism; exhausting memory to deliver determinism is
> never the better trade.

## Every strategy is bounded

> [spec:hfst:req:determinize-envelope.bounded-strategies]
> Every determinization strategy MUST run under the resource envelope, including
> the last one tried.  The weight-encoding strategy — folding the weight into
> the label so that the subset construction runs on an unweighted machine — is
> the final fallback, and precisely because nothing follows it, it MUST NOT be
> left unbounded: an unbounded last resort makes the envelope decorative, since
> every overrun of a bounded strategy funnels into it.  Weight encoding
> separates paths that label-only determinization would have merged, so it can
> only produce more states than the label-only strategy, never fewer; a
> transition-budget overrun on the label-only strategy therefore MUST NOT be
> retried under weight encoding, which cannot improve on a verdict about the
> size of the result.

## Bounding transitions, not just states

> [spec:hfst:req:determinize-envelope.transition-axis]
> The envelope MUST bound the transitions written to the determinized machine,
> as an axis independent of the state count and of the weighted-subset element
> count.  Neither of the other two axes can see this one: a state count bounds
> how many states exist and a subset count bounds the search data inside them,
> but one state may carry an unbounded out-degree, so a machine can stay far
> inside both bounds while writing orders of magnitude more transitions than its
> input held.  Memory follows the transitions.
>
> The transition bound MUST be a floor rather than a fixed ceiling: an input
> that already holds more transitions than the floor MUST be allowed an output
> of comparable size, because a determinization whose result stays within a small
> multiple of its input has not blown up, whatever its absolute size.  Any run
> that stays inside every bound MUST produce byte-identical output to the same
> run with the bounds removed, so that adding or widening an axis can never
> perturb a compilation that was already succeeding.
