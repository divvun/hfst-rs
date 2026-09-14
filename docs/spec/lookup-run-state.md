# Lookup run state

The ported optimized-lookup `Transducer` and the pmatch runtime keep their
per-run scratch — tapes, flag state, traversal bookkeeping, accumulated
results, limits, clocks — on the same struct as the loaded machine, because
the C++ did. Every lookup therefore takes the whole object exclusively, and an
embedder that wants concurrent lookups over one loaded machine is forced to
either wrap it in a lock (serializing every request behind the largest asset
in the bundle) or load a private copy per thread (multiplying resident memory
by the thread count). The speller already demonstrates the right shape in this
tree: an immutable core borrowed by a per-run object that owns all mutation.

These rules split machine from run everywhere the lookup and pmatch engines
traverse. They change where state lives, not what any traversal computes.

## The loaded machine is immutable

> [spec:hfst:req:lookup-run-state.immutable-core]
> After loading completes, the machine — header, alphabet, symbol encoder, and
> tables — MUST NOT be mutated by any lookup, match, or tokenisation call. The
> loaded core MUST be shareable across threads, and sharing it MUST NOT require
> a lock around traversal.

> [spec:hfst:req:lookup-run-state.caller-owned-scratch]
> All per-run scratch MUST live in a run-state object the caller owns, created
> against a machine and reusable across calls. Traversal entry points take the
> machine shared and the run state exclusively. The analyses returned MUST be a
> function of the machine, the input, and the caller-set limits alone; two run
> states over one machine MUST NOT observe each other's existence through
> results, ordering, or weights.

## Out-of-alphabet admission lives in the run

> [spec:hfst:req:lookup-run-state.out-of-alphabet-overlay]
> The admission of out-of-alphabet input symbols required by
> [spec:hfst:req:ol-lookup-enumeration.out-of-alphabet-input] MUST be recorded
> in the run state, as an overlay numbered past the machine's original symbol
> count, leaving the machine's own alphabet and encoder untouched. The analyses
> returned MUST be identical to those of an implementation that admits the
> symbol into the machine's alphabet. An overlay MAY persist across successive
> lookups through the same run state; it MUST NOT be visible to any other run
> state. Strict rejection of out-of-alphabet input is not a permitted
> alternative: identity and unknown arcs exist precisely to pass such symbols
> through, and tokenisers depend on that on ordinary text.

## Pmatch shares its machine the same way

> [spec:hfst:req:lookup-run-state.pmatch-shared-core]
> The pmatch runtime MUST hold its transition and index tables, and every other
> structure fixed at container load, behind atomically counted shared
> references, with all per-run state — local-variable stacks, flag state,
> tapes, positions, line counts — outside the shared structures in caller-owned
> run state. Concurrent runs over one loaded container MUST each produce output
> identical to the same inputs run serially against a privately loaded copy.
