---
id [dec:hfst:adaptive-determinize]
epitome "Bound both the state count and weighted-subset population of determinization. Ordinary runs remain byte-identical; state divergence retries with weight encoding, while a subset-memory overrun during minimization tries the reverse orientation before preserving the exact relation without further minimization."
state @decided
category @existence
scope {
    elements ([arch:hfst:backend-dispatch])
    rules ([spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.minimize-fn] [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.determinize-fn])
}
author "brendan@necessary.nu"
alternatives (
    {
        option "Restore only upstream's -E contract (a user-set global that forces weight encoding), no budget."
        rejected_because "-E is a manual escape hatch: the user must know in advance that their grammar triggers the non-termination and pass the flag. hfst/hfst#435 is exactly a user hitting the loop WITHOUT -E and having hfst-lexc hang forever (6GB+, ^C). The port's job is to not hang by default. -E is still honoured (the LexcCompiler::set_encode_weights forwarding is restored), but it is a preference, not the safety net."
    }
    {
        option "Hard error when the budget is exceeded (no fallback)."
        rejected_because "Converts a hang into a failed compile — still no output for a grammar OpenFST-style tools would eventually process. Weight encoding gives an exact, always-terminating result; a slightly-less-minimal FST beats no FST."
    }
    {
        option "Always determinize with weight encoding (make EncodeWeightsAndLabels the default)."
        rejected_because "Breaks the byte-identity invariant: weight encoding produces a different (correct but less minimal) machine than label-only weighted determinization on the vast majority of inputs that DO terminate today. Byte-identical output on every currently-working input is the whole point of the budget-then-fallback shape — the encoded path runs only where the label-only path would otherwise loop forever."
    }
    {
        option "Detect non-twins (twins property) up front and pick the strategy statically."
        rejected_because "The twins property test is itself a nontrivial cyclic-FST analysis; a produced-state budget is a cheap, always-correct dominating check (any FST whose determinization blows up trips it regardless of the reason), and it needs no new algorithm."
    }
)
consequences {
    accepted (
        "The default label-only envelope is min(max(1024, 256 * input_states), 2,000,000) output states and 4,194,304 coexisting logical weighted-subset elements. The latter was selected as roughly 256 MiB at a conservative 64-byte transient estimate; loaded inputs, outputs, and allocator overhead are outside that estimate."
        "Within both limits, output is byte-identical to the unbounded algorithm. Crossing a limit is a correctness-preserving representation fallback, not an error: a state-count overrun retries with weight encoding; a subset-memory overrun in minimize first minimizes the reverse orientation and reverses back, then preserves the exact relation without further minimization only if both orientations exceed the limit. Such an output can be less minimal."
        "The mechanism lives in the rustfst fork as optional DeterminizeConfig.max_states and max_subset_elements fields (both default None). RustFST merges duplicate default-semiring destinations while scanning instead of materializing and sorting every raw path, and raises a typed subset-limit error before either persistent or transient logical elements exceed the configured count. hfst-openfst preserves that distinction for TropicalWeightTransducer's retry policy."
        "A tracing::info! fires on every fallback so builds can observe when a grammar hit the non-termination path."
    )
    deferred (
        "Propagating the budget/fallback to the other weighted backends (foma is unweighted; SFST is not ported) — only the tropical OpenFST backend determinizes weighted cyclic FSTs, so it is the only site that can loop."
        "Upstreaming DeterminizeConfig.max_states to garvys-org/rustfst — the field is kept default-None precisely so it is a clean upstream candidate, but the PR is out of scope here."
    )
}
edges {
    requires ([dec:hfst:monomorphic-backends])
}
codifies ()
establishes ()
---

## Rationale

hfst/hfst#435 is a plain non-termination bug: a lexc grammar that assigns
a weight to an iterated term (the reporter's `< Co* [a::1]+ ... 0::10 >`)
builds a cyclic weighted acceptor whose two epsilon branches feed
`a`-self-loops with different weights (1 vs 11 in the maintainer's minimal
`att`). That FST is not a *twins* automaton, so weighted subset
determinization splits states forever — the process eats 6GB+ and never
returns. Upstream's only remedy is the `-E` switch, which forces weight
encoding (`EncodeWeightsAndLabels`) before determinizing; encoded
determinization is exact and always terminates. But `-E` is opt-in: the
default still hangs.

The port must not hang or consume unbounded subset memory by default. The
design is **budget then fall back**:

1. Run label-only weighted determinization under both an output-state bound
   and a bound on the weighted-subset elements retained or accumulated during
   the current expansion.
2. If it converges within both bounds, keep its output verbatim. This is the
   byte-identity invariant for ordinary inputs.
3. A state-count overrun retains the hfst/hfst#435 behavior: retry with weight
   encoding.
4. A subset-memory overrun during minimization first tries the reverse
   orientation, which is cheap for large co-deterministic rule machines such
   as KAL's phonology. If both orientations exceed the bound, preserve the
   exact weighted relation and warn that no further minimization was done.

The state count remains threaded through `LazyFst::compute_bounded`. The
subset limit lives inside RustFST's determinize state table and transition
expansion, where it can account for the structure the generic lazy
materializer cannot see. `hfst-openfst` surfaces a typed subset exhaustion,
and the retry policy lives in `TropicalWeightTransducer::minimize` and
`determinize`.

`-E` is still restored as a first-class preference: the C++ process global
that every tropical `minimize` read became `EngineConfig.encode_weights`
during de-globalization, but the `LexcCompiler` never forwarded it. The
`LexcCompiler::set_encode_weights` forwarding to the embedded
`XreCompiler` closes that gap. With the adaptive budget in place, `-E`
becomes a way to *force* the encoded (always-terminating) path up front
rather than the only way to avoid the hang.
