---
id [dec:hfst:adaptive-determinize]
epitome "Give weighted determinization a state budget with an automatic exact fallback: when label-only weighted determinization exceeds a generous per-input budget (it may never terminate on non-twins cyclic FSTs — hfst/hfst#435), transparently retry with weight encoding, which always terminates. Any input that converges unbounded stays byte-identical."
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
        "The budget heuristic max(1024, 256 * input_states) is tunable: 256 is a generous fan-out constant chosen so every input that terminates under today's unbounded weighted determinization stays under budget (byte-identical), and the 1024 floor covers tiny inputs. If a real grammar is found that terminates unbounded yet trips the budget, raise the constant — the fallback keeps such a case correct (just less minimal) in the meantime, so it is a tuning issue, not a correctness regression."
        "The fallback output is the weight-encoded determinization, which can be LESS MINIMAL than the true weighted-minimal FST. It is always correct (same weighted language) and always terminates. Callers that need true minimality on such inputs cannot get it (the true-minimal result is what does not terminate)."
        "The mechanism lives in the rustfst fork as an optional DeterminizeConfig.max_states (default None = today's unbounded behavior, upstreamable) plus a LazyFst::compute_bounded; hfst-openfst exposes DeterminizeBounded returning Result, and TropicalWeightTransducer::minimize/determinize own the budget + retry policy. encode_weights == true is unchanged (no budget)."
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

The port must not hang by default. The design is **budget then fall
back**:

1. Run label-only weighted determinization (today's behavior) under a
   generous produced-state budget.
2. If it converges within budget, keep its output verbatim — this is the
   **byte-identity invariant**: every input that works today produces the
   exact same machine, because the budget only ever trips where the
   algorithm would otherwise loop forever.
3. If it exceeds the budget, transparently re-run with weight encoding,
   which terminates. Log it with `tracing::info!`.

The budget is a produced-state count threaded through the rustfst fork as
`DeterminizeConfig.max_states` (default `None`, so upstream behavior is
untouched and the field is upstreamable) into a `LazyFst::compute_bounded`
that aborts the on-demand BFS once the count is exceeded. `hfst-openfst`
surfaces this as `DeterminizeBounded` (returns `Result` instead of the
infallible OpenFST-shaped `Determinize`), and the budget + retry policy
lives in `TropicalWeightTransducer::minimize`/`determinize`, the only two
sites that weighted-determinize a possibly-cyclic FST.

`-E` is still restored as a first-class preference: the C++ process global
that every tropical `minimize` read became `EngineConfig.encode_weights`
during de-globalization, but the `LexcCompiler` never forwarded it. The
`LexcCompiler::set_encode_weights` forwarding to the embedded
`XreCompiler` closes that gap. With the adaptive budget in place, `-E`
becomes a way to *force* the encoded (always-terminating) path up front
rather than the only way to avoid the hang.
