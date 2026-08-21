# Optimized-lookup enumeration

The optimized-lookup engine is a depth-first walk of a packed transition table.
Two machines can denote one weighted relation and lay it out completely
differently — a nondeterministic union keeps the alternatives as sibling arcs
and leans on symbolic `@_UNKNOWN_SYMBOL_@` / `@_IDENTITY_SYMBOL_@` arcs, while
the determinized-and-minimized form of the same relation spreads them across
concrete arcs and many more states.  A caller picks between those two for size
and build-time reasons and expects the same answers out of either.

The measured case is a Giella speller error model held both ways: a 13 MB union
and a 251 MB determinized build of one relation, probed with a word carrying a
symbol outside the model's alphabet.  Anything that curtails the walk part-way
answers from whichever paths the table order happened to reach first, so the two
files disagree with each other and the cheapest analysis — the whole point of a
weighted lookup — can be missed entirely while a costlier one is reported in its
place.

These rules constrain the traversal.  They do not license it to change the
relation it walks.

## The walk is exhaustive

> [spec:hfst:req:ol-lookup-enumeration.no-internal-work-cap]
> The traversal MUST NOT curtail itself on an implementation-internal budget
> over total work — node visits, arcs followed, elapsed cycles, or any other
> whole-lookup accounting the caller did not ask for.  Only limits the caller
> supplied MAY end a lookup early: a maximum result count, or a wall-clock
> cutoff.  An internal ceiling is not a safety net but a silent wrong answer:
> the traversal stops mid-walk with no diagnostic, and returns a proper subset
> of the analyses while presenting it as the whole.
>
> Termination on a cyclic machine MUST instead come from bounding the walk's
> shape rather than its size: an epsilon or flag-diacritic arc that returns to a
> transition-table index already on the current DFS path under the same flag
> values makes no progress and MUST NOT be re-entered.  That bound is exact —
> it removes only re-entries that consume no input and can therefore only repeat
> work — so it terminates every epsilon cycle while discarding no analysis.

> [spec:hfst:req:ol-lookup-enumeration.representation-independence]
> The analyses returned for an input MUST be a function of the weighted relation
> the machine denotes, not of the order the packed tables happen to present it
> in.  Arc order within a state, the factoring of states, and whether the
> machine is deterministic or minimized MUST NOT change the set of analyses
> returned, nor the weight reported for any of them.  In particular the minimum
> weight over the analyses of an input MUST be the true minimum over the
> relation, reachable no matter how deep in table order the path that carries it
> happens to sit.

## Symbols outside the alphabet

> [spec:hfst:req:ol-lookup-enumeration.out-of-alphabet-input]
> An input symbol absent from the machine's original alphabet is admitted to the
> alphabet for the duration of the lookup and numbered past the original symbol
> count.  On consuming such a symbol the traversal MUST attempt the identity arc
> and the unknown arc — both, when the machine defines both, since a machine can
> carry an identity reading and a substitution reading of the same position —
> and MUST NOT attempt any concrete alphabet arc, which by construction cannot
> match.  Only when no transition was found at all MAY a default arc be tried.

> [spec:hfst:req:ol-lookup-enumeration.meta-arc-output]
> When a traversed arc's output side is a meta symbol — unknown, identity, or
> default — the symbol written to the output tape MUST be the input symbol just
> consumed, not the meta symbol itself.  A lookup result therefore never carries
> `@_UNKNOWN_SYMBOL_@` or `@_IDENTITY_SYMBOL_@` as literal output text: the
> engine instantiates the placeholder from the tape or emits nothing at all.

> [spec:hfst:sem:ol-lookup-enumeration.meta-arc-restriction]
> Instantiating from the tape is narrower than the general lookup engine, which
> expands an unknown-output arc over the alphabet and leaves the placeholder
> standing when it cannot.  Against the error model above the two agree on
> 958,376 output strings with identical weights on every one of them; the
> general engine reports a further 10,790, and every one of those is a string
> containing a literal `@_UNKNOWN_SYMBOL_@` token that was never instantiated.
> The optimized-lookup engine loses no realized string and no minimum weight by
> this — the difference is entirely uninstantiated placeholders — and the
> narrower reading is the conformance target.
