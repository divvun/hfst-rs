# Virtual flag-diacritic algebra

HFST historically harmonizes flag diacritics for binary algebra operations by
adding an identity self-loop for every missing flag at every state.  That graph
rewrite is the semantic reference for this specification, but it is not a
required representation: on real Giella transducers it can create hundreds of
millions of redundant transitions before the requested operation begins.

## Materialized reference

For two operands `L` and `R`, the materialized reference is the result of the
existing `harmonize_flag_diacritics(..., true)` pass followed by the requested
operation:

1. Flag membership and set differences are computed from the complete
   alphabets, including symbols that do not occur on transitions.
2. If both original alphabets contain flags, every flag feature in `L` is
   renamed with suffix `_1` and every flag feature in `R` with suffix `_2`.
3. Each post-rename flag missing from an operand is added there as an identity
   self-loop at every state.  Such a loop has the same source and target,
   identical input and output labels, and semiring-one weight.
4. In the two-sided case, paths in the left prepared operand are restricted so
   that `_1` events precede `_2` events between ordinary left-output symbols.
   Epsilon events do not reset this ordering state.
5. Normal symbol harmonization, special-symbol handling, operation cleanup,
   and result metadata rules then apply unchanged.

> [spec:hfst:req:virtual-flag-algebra.materialized-reference]
> Every virtual flag operation must denote the same weighted relation as its
> materialized reference.  Preparation must use complete alphabet differences,
> preserve alphabet-only flags, apply the same two-sided feature renaming and
> ordering restriction, and expose each missing flag only as a same-state,
> identity-labelled, semiring-one logical transition.  A logical transition
> may match only a real transition from the opposite operand; two virtual
> transitions must never match each other.  Flag labels must remain excluded
> from unknown and identity wildcard expansion.  True epsilon transitions,
> parallel paths, final weights, and the backend's established symbol-table and
> result-metadata behavior must remain unchanged.

## Shared virtual representation

> [spec:hfst:req:virtual-flag-algebra.backend-core]
> HFST must represent a prepared flag overlay independently of composition and
> pass it through its algebra-backend boundary without inserting physical
> `states * missing_flags` transitions.  OpenFst tropical/RustFST and Foma must
> consume that representation on demand, preserve dense operation state and
> error behavior, and release operation-owned resources after success or
> failure.  Preparing an overlay may rename flag symbols and extend alphabets,
> but must not change either operand's state or transition count.  Existing
> eager harmonization remains available for compatibility and for backends or
> operations that have not opted into the virtual contract.

## Compose frontends

> [spec:hfst:req:virtual-flag-algebra.frontend-compose]
> When flag harmonization is enabled, XRE/`hfst-regexp2fst` composition and XFST
> stack composition must route through the same virtual flag preparation and
> backend composition path as `hfst-compose -F`.  Their existing enablement,
> warning, optimization, error, and stack-order behavior must not change, and
> their results must match the materialized reference for one-sided and
> two-sided flag alphabets.

## Intersection

> [spec:hfst:req:virtual-flag-algebra.intersection]
> `hfst-conjunct -F` and the corresponding HFST intersection API must consume a
> virtual flag overlay in both OpenFst tropical/RustFST and Foma backends.  The
> result must match eager flag harmonization followed by intersection for
> one-sided and two-sided inputs, without constructing the harmonized operand
> graphs.  Weighted paths, duplicate transitions, true epsilons, alphabet-only
> flags, unknown/identity symbols, and the two-sided ordering restriction must
> retain their materialized-reference semantics.

## Special composition modes

> [spec:hfst:req:virtual-flag-algebra.special-compose]
> Virtual flag composition must support both established special modes.  With
> `flag-is-epsilon`, relevant left-output and right-input flag events participate
> in composition as epsilons and the result retains the established one-sided
> flag restoration and validation behavior.  With Xerox composition, flags are
> matched using the established encoded spelling and decoded in the result.
> Each mode must match its eager reference on OpenFst tropical/RustFST and Foma,
> including error cases, without materializing missing-flag self-loops.

## Subtraction

> [spec:hfst:req:virtual-flag-algebra.subtraction]
> When both operands contain flags, `hfst-subtract -F` and the corresponding
> HFST subtraction API must consume a virtual overlay in OpenFst
> tropical/RustFST and Foma.  The result must match two-sided eager flag
> harmonization followed by subtraction, including left-path ordering, weights,
> special symbols, and the existing warning and enablement behavior, without
> constructing the harmonized operand graphs.

## Language-level release gates

> [spec:hfst:req:virtual-flag-algebra.language-gates]
> Release validation must exercise the virtual compose frontend, intersection,
> special-compose, and subtraction paths with reproducible artifacts from each
> of `lang-gle`, `lang-kal`, `lang-sma`, and `lang-sme`.  Every virtual result
> must be semantically equivalent to its eager reference and leave no operation
> scratch behind.  The gates must record input and output graph sizes, elapsed
> time, peak resident memory, configured memory allowance, and whether either
> backend spilled.  At least one fixture per operation family must demonstrate
> that preparation avoids a nonzero `states * missing_flags` expansion.
