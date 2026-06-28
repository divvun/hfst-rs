---
id [dec:hfst:librarify-cli]
epitome "Lift bespoke transducer logic out of the CLI tools into the hfst library so every tool is a thin driver (parse opts -> open std::fs I/O -> call lib -> format); sequence by I/O coupling."
state @decided
category @executive
scope {
    elements ([arch:hfst:backend-dispatch])
    rules ()
}
author "brendan@necessary.nu"
alternatives (
    {
        option "De-C-ify the CLI tools in place as originally scoped by idiom2.cli (FILE* -> std::io across every bin, leaving the logic where it is)."
        rejected_because "The 59-tool audit found ~1800 lines of hfst-optimized-lookup duplicate the OL runtime the library already owns, 9 tools share a verbatim n-ary binary-fold loop, and 7 share the symbols_used scan. De-C-ifying that logic in place is wasted work — it gets deleted or collapsed. Lifting first makes the residual de-C-ism trivial."
    }
    {
        option "Lift the logic AND split the hfst crate into layered sub-crates (lookup/pmatch/core) at the same time."
        rejected_because "Bigger structural move that pulls the Stage-3 `types` decomposition forward and entangles two concerns. Keep the lifts inside the single hfst crate; the crate split stays the later `types` wave."
    }
)
consequences {
    accepted ("30 tools are confirmed thin (I/O only); 29 carry transducer-semantic logic that moves into crates/hfst/src as reusable APIs (hfst_lookup_format, renumber_states, summarize, pmatch_tokenize handlers, label_to_stringpair, etc.). Every tool ends as parse -> std::fs -> lib -> format. The 121-test contract holds per node.")
    deferred ("Crate decomposition stays the Stage-3 `types` wave. The stream-family lifts (lookup, pmatch/tokenize) are joint with the FILE*->std::fs conversion and land in Tier 3; the CLI std::fs I/O foundation subsumes idiom2.cli.")
}
edges {
    requires ([dec:hfst:idiomatize-staging])
}
codifies ()
establishes ()
---

## Rationale

The Wave-4 Stage-2 plan ([dec:hfst:idiomatize-staging]) scoped a CLI de-C-ism node
(idiom2.cli) that would convert every tool's FILE* I/O to std::io in place. A 59-tool
audit (one assessor per tool + synthesis) found that doing so would de-C-ify a great deal
of logic that should not live in a CLI at all: hfst-optimized-lookup re-implements the
entire optimized-lookup runtime the library already exposes in `crate::transducer`; nine
binary tools share a byte-for-byte n-ary stream-fold loop; the lookup and flookup tools
share a verbatim %-format engine, an Apertium parser, and a cascade engine; and the
symbols_used scan, a state-renumber rebuild, and an escaped-colon label parser are each
copied across multiple tools.

The decisive finding: for stream/format tools the logic-lift and the FILE*->std::fs
de-C-ism are the *same* pass (the bespoke logic is interleaved with hfst_getline/FILE*).
So the work is sequenced by I/O coupling, not by tool:

- **Tier 1 — pure-logic lib lifts, zero I/O coupling, parallelizable**: reuse the OL
  runtime; lift symbols_used, renumber_states, label_to_stringpair, summarize/properties,
  conditional transform_weights, kill_paths, realign, is_weighted,
  substitute_by_composition, compose_intersect_fast, the pair recognizer, affix_guessify,
  generator-from-guesser, expand_equivalences, strip_hfst3_headers.
- **Tier 2 — CLI foundation**: the std::fs I/O foundation (kill the *mut FILE globals;
  subsumes idiom2.cli) plus the shared binary-fold driver (collapses 9 tools).
- **Tier 3 — stream families lifted jointly with their I/O**: the lookup family
  (hfst_lookup_format + cascade + extract renderer) and the pmatch/tokenize family
  (pmatch_tokenize handlers + pmatch2fst archive writer + locate formatter).

Lifts stay in the single hfst crate; the layered crate split is the later `types` wave.
After all three tiers every CLI is a thin driver and idiom2.cli's residual de-C-ism is
absorbed into Tier 2/3.
