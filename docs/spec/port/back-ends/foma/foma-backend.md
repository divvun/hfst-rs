# back-ends/foma — native foma backend (Rust, target-only)

Target-side backend that makes `ImplementationType::FOMA_TYPE` a real,
usable transducer implementation, backed by the standalone Rust port of
foma (the `foma` crate, a path dependency). The upstream C++
`FomaTransducer.*` / `ConvertFomaTransducer.*` are **excluded** from
hfst's source-impl scope (foma was ported separately), so these rules
are authored greenfield: they describe the Rust backend's contract, not
a 1:1 C++ port.

The whole backend is gated behind the `foma` Cargo feature; with the
feature off, nothing here compiles and the facade behaves exactly as
before (FOMA_TYPE remains unavailable).

## Motivation

`plan/main.styx` (the weighted-determinize blowup notes) records that
foma is the intended fix for tropical subset-construction blowup: foma
determinizes/minimizes **unweighted** automata with boolean subset
construction, which merges freely where tropical determinize cannot.
Exposing foma's algebra natively lets the pmatch/tokeniser pipeline run
those ops in foma instead of round-tripping through openfst.

## Types

> [spec:hfst:def:foma-backend.foma-transducer]
> A newtype wrapper `FomaTransducer(foma::types::Fsm)` — the backend's
> transducer handle. The inner `Fsm` is foma's sentinel-terminated line
> table (`Vec<FsmState>`) plus its `Sigma` alphabet (number↔symbol,
> with reserved numbers EPSILON=0, UNKNOWN=1, IDENTITY=2). Implements
> `Backend` (const `TYPE = FOMA_TYPE`), `AlgebraBackend`, and
> `LookupBackend`.

## Conversion (the `foma-backend.convert` node)

> [spec:hfst:def:foma-backend.to-basic-fn]
> fn to_basic(&self) -> Result<HfstBasicTransducer>

> [spec:hfst:sem:foma-backend.to-basic-fn]
> Build an empty `HfstBasicTransducer`. Walk the inner `Fsm.states`
> line table in order, stopping at the sentinel row (`state_no == -1`).
> For each row with `state_no == s`:
> - If the row's `start_state`/`final_state` flags mark it (foma stores
>   these on the first row of each state), record finality: when
>   `final_state == 1`, `set_final_weight(s, 0.0)` (foma is unweighted →
>   weight 0.0). foma's start state is always state 0, matching HFST's
>   convention, so no start remapping is needed.
> - If the row encodes an arc (`in != -1`, `target != -1`), add a
>   transition `s -> target` with input symbol `sym(in)` and output
>   symbol `sym(out)`, weight 0.0, where `sym(n)` maps a foma sigma
>   number to an HFST symbol string: `0 -> "@_EPSILON_SYMBOL_@"`,
>   `1 -> "@_UNKNOWN_SYMBOL_@"`, `2 -> "@_IDENTITY_SYMBOL_@"`, otherwise
>   `sigma_string(n)` (the interned symbol). A state with no arcs
>   (`in == -1`) contributes only its state/finality, via `add_state`.
> Add every non-reserved sigma symbol to the basic transducer's
> alphabet (`add_symbol_to_alphabet`). Set the result's `name` from
> `Fsm.name`. Reserved numbers 0/1/2 are represented by their HFST
> special-symbol strings, never added as ordinary alphabet members.

> [spec:hfst:def:foma-backend.from-basic-fn]
> fn from_basic(net: &HfstBasicTransducer) -> Result<Self>

> [spec:hfst:sem:foma-backend.from-basic-fn]
> Construct a foma `Fsm` from the interchange graph using foma's
> `dynarray` construction API. Assign each HFST symbol a foma sigma
> number: the special strings map to reserved numbers
> (`"@_EPSILON_SYMBOL_@" -> 0`, `"@_UNKNOWN_SYMBOL_@" -> 1`,
> `"@_IDENTITY_SYMBOL_@" -> 2`), every other distinct symbol gets a
> fresh number ≥ 3 (interned in sigma insertion order). Walk the basic
> transducer's states in order; for each transition add an arc
> `origin -> target` with the mapped `(in, out)` sigma numbers; mark a
> state final when `is_final_state` holds (HFST weight is discarded —
> foma is unweighted). HFST state 0 is the foma start state. Finalize
> via the construction handle. The construction API interns only the
> symbols it is handed, so the sigma it produces is exactly the set seen
> on an arc; every remaining member of the interchange net's alphabet is
> then declared in that sigma (non-reserved, not already present) and the
> sigma re-sorted. This is not bookkeeping: foma's `?`/`@` match exactly
> the symbols the sigma does NOT list, so an alphabet member on no arc
> has to be a sigma entry for the net to mean what HFST says it means.
> Return `FomaTransducer(fsm)`. The round-trip `to_basic ∘ from_basic`
> preserves the recognized relation and the (non-reserved) alphabet,
> including alphabet members that no arc uses — which is what makes the
> `Backend` defaults that mutate through the interchange form
> (`prune_alphabet`, the flag encode/decode, substitution) alphabet-safe
> on this backend.

## Backend trait obligations (`foma-backend.seam` + downstream)

> [spec:hfst:def:foma-backend.backend-impl]
> `impl Backend for FomaTransducer`: `empty` = an empty foma net
> (`fsm_empty_set`); `copy` = `fsm_copy`; `get_alphabet` = the sigma's
> non-reserved symbols as a `StringSet`; `is_cyclic` = negation of
> foma's acyclicity (via `fsm_topsort`'s loop-free flag);
> `is_infinitely_ambiguous` derived
> from cyclicity on the input projection. `write` (foma-backend.io)
> serializes the native `.foma` binary format. `extract_paths_cb` /
> `extract_paths_fd_cb` (foma-backend.lookup) enumerate paths via foma
> apply.
>
> The three alphabet edits are in-place sigma work rather than the
> trait's interchange round trip. `insert_to_alphabet` and
> `add_symbols_to_alphabet` declare each non-reserved symbol not already
> in the sigma (`sigma_add`), then re-sort — foma's constructions read
> the sigma as sorted by symbol string, and `fsm_merge_sigma` mismaps
> arcs silently if it is not. `remove_from_alphabet` is alphabet-only
> and never touches the graph: a foma arc addresses its symbol by sigma
> NUMBER, so an entry some arc still carries is retained (dropping it
> would leave the arc addressing a hole that the following renumber
> fills with a neighbouring symbol, relabelling the arc), while every
> entry for that symbol which no arc uses is dropped and the sigma
> re-sorted.
>
> PORT DIVERGENCE. Upstream C++ HFST answers the set form and the
> removal through the same lossy interchange round trip this backend
> inherited, so on a foma net an alphabet insert never landed and a
> removal succeeded only because the round trip discarded arc-less
> symbols regardless — two paths to the alphabet that disagreed about
> what it was. Fixed here per [dec:hfst:independent-fork]: the port is a
> successor, and a silent `Ok(())` that changes nothing is the failure
> mode this backend's whole test battery exists to catch. The upstream
> SymbolTable backends will additionally unname a live arc label on
> request; foma will not, because in foma that is not a rename but a
> corruption.

## Algebra (`foma-backend.algebra` node)

> [spec:hfst:def:foma-backend.algebra-impl]
> `impl AlgebraBackend for FomaTransducer` maps each op to its foma
> construction, all unweighted (weight arguments and weight-transform
> ops are no-ops or trivial): `compose`→`fsm_compose`,
> `disjunct`→`fsm_union`, `intersect`→`fsm_intersect`,
> `subtract`→`fsm_minus`, `concatenate`→`fsm_concat`,
> `determinize`→`fsm_determinize`, `minimize`→`fsm_minimize`,
> `remove_epsilons`→`fsm_epsilon_remove`, `repeat_star`→`fsm_kleene_star`,
> `repeat_plus`→`fsm_kleene_plus`, `optionalize`→`fsm_optionality`,
> `invert`→`fsm_invert`, `reverse`→`fsm_reverse`,
> `extract_input_language`→`fsm_upper`,
> `extract_output_language`→`fsm_lower`, and the `define_transducer_*`
> constructors from foma's symbol/pair builders. `are_equivalent` uses
> `fsm_equivalent`. Inputs are consumed/copied per foma's ownership
> conventions. This is the node that lets hfst run unweighted
> determinize/minimize in foma to avoid tropical blowup.

## Lookup (`foma-backend.lookup` node)

> [spec:hfst:def:foma-backend.lookup-impl]
> `impl LookupBackend for FomaTransducer` drives foma's `apply` runtime:
> `lookup_fd_str` tokenizes the input against the sigma and enumerates
> outputs (apply-up/down) with flag-diacritic obeying, returning
> `HfstOneLevelPaths` with weight 0.0; infinite-ambiguity queries use
> foma's cyclicity on the relevant projection.

## Stream I/O (`foma-backend.io` node)

> [spec:hfst:def:foma-backend.stream-io]
> The `HfstInputStream` FOMA_TYPE arm reads a native `.foma` net
> (foma's `io::fsm_read_binary_file`, gzip-aware) and wraps it as
> `AnyTransducer::Foma`. `Backend::write` emits the native `.foma`
> binary format (`io::fsm_write_binary_file`) when not in HFST wrapper
> format, or the HFST-wrapped payload otherwise.
