---
id [dec:hfst:thfst-backend]
epitome "Add THFST (divvunspell's mmap format) as the first post-parity NOVEL backend — a full lookup-tier citizen with directory-format stream arms — plus an hfst-bhfst box packer, so rust-hfst emits divvunspell artifacts directly and the zip+thfst-tools pipeline steps disappear."
state @decided
category @existence
scope {
    elements ([arch:hfst:backend-dispatch])
    rules ([spec:hfst:def:thfst-backend.thfst-transducer] [spec:hfst:def:thfst-backend.directory-format] [spec:hfst:def:thfst-backend.alphabet-json] [spec:hfst:def:thfst-backend.stream-io] [spec:hfst:def:thfst-backend.bhfst-layout] [spec:hfst:def:thfst-backend.bhfst-tool])
}
author "brendan@necessary.nu"
alternatives (
    {
        option "Full native SFST port as the parity-completing backend."
        rejected_because "Weeks-to-months of work whose seam-proof value foma already delivers (an unweighted AlgebraBackend engine exists); SFST is a legacy sibling, not novel, and has no ecosystem demand."
    }
    {
        option "MyTransducerLibrary skeleton (upstream's add-a-backend template)."
        rejected_because "A stub proves the seam only on paper; it ships nothing anyone runs."
    }
    {
        option "BurntSushi fst crate backend."
        rejected_because "Model mismatch: byte-keyed acceptors/maps, no epsilons, no arbitrary symbol pairs, u64-only outputs — semantically lossy for HFST transducers."
    }
    {
        option "Write-only THFST (fst2fst emit + packer, no reading)."
        rejected_because "A serializer is not a backend; the feature-completeness claim needs a full citizen (read + write + lookup through the tools). Operator chose full citizen 2026-07-10."
    }
    {
        option "Feature-gate THFST like foma."
        rejected_because "foma is gated because it is a path dep into a sibling checkout; THFST's only core dep is serde_json (plain crates.io). Gating would double the cfg surface on every exhaustive ImplementationType/AnyTransducer match, and the deny(wildcard_enum_match_arm) forcing function works best with the variant unconditionally present. Heavy deps (box-format git, serde-xml-rs) are confined to hfst-cli."
    }
    {
        option "Depend on the divvunspell crate for the format types."
        rejected_because "Pulls zip/unic/lifeguard and a whole speller runtime for what is ~200 lines of (de)serialization; the format contract is captured as spec rules instead."
    }
    {
        option "meta.json-only packer input (no index.xml)."
        rejected_because "Not a drop-in replacement: Giella lang builds produce index.xml today. Accepting it (converted exactly as thfst-tools does, serde-xml-rs =0.6 semantics) lets the pipeline switch by swapping the pack command only. Operator chose index.xml + meta.json 2026-07-10."
    }
)
consequences {
    accepted (
        "The compatibility contract is divvunspell's parser, not rust-hfst's internals: the alphabet JSON re-derives flag feature/value numbering from symbol strings in divvunspell's 0-based first-encounter scheme, ignoring the engine FdTable's C++-derived scheme (empty value preseeded at 0, real values from 2) in both directions."
        "THFST stores less than an OLW header: input_symbol_count and the nine property flags are lost on write and synthesized on read (total count / all-false). Lookup strings and weights are unaffected; is_infinitely_ambiguous may under-report and hfst-summarize differs on a thfst-loaded transducer. Same loss exists in divvunspell itself."
        "A directory format enters a byte-stream world: the stream layer gains a preloaded-input path and a directory sink plus a Backend::write_to_dir hook (default error), and THFST is impossible on stdin/stdout and in multi-transducer streams by construction."
        "box-format is a git-only dependency pinned to divvunspell's Cargo.lock rev (bbqsrc/box 0.4.0, sync::BoxWriter); the pin must track divvunspell if it moves."
        "serde + serde_json become unconditional dependencies of the hfst crate."
    )
    deferred (
        "Chunked THFST (divvunspell's mmap-limit fallback reader variant) is not produced — thfst-tools never produces it either; divvunspell's chunked archive loader reads plain dirs fine."
        "Reading .bhfst archives back into hfst tools (only hfst-bhfst --info inspects metadata); divvunspell remains the consumer of record."
    )
}
edges {
    requires ([dec:hfst:monomorphic-backends])
}
codifies ([spec:hfst:def:thfst-backend.thfst-transducer] [spec:hfst:def:thfst-backend.directory-format] [spec:hfst:def:thfst-backend.index-record] [spec:hfst:def:thfst-backend.transition-record] [spec:hfst:def:thfst-backend.alphabet-json] [spec:hfst:def:thfst-backend.write-dir-fn] [spec:hfst:def:thfst-backend.read-dir-fn] [spec:hfst:def:thfst-backend.olw-moves] [spec:hfst:def:thfst-backend.stream-io] [spec:hfst:def:thfst-backend.bhfst-layout] [spec:hfst:def:thfst-backend.meta-json] [spec:hfst:def:thfst-backend.bhfst-tool])
establishes ()
---

## Rationale

The port reached C++ parity; the remaining claim to prove is that the
monomorphic backend seam ([dec:hfst:monomorphic-backends]) is not merely
a faithful re-encoding of the C++ union but an extensible taxonomy. A
backend C++ HFST never had is the only convincing witness. THFST wins
over every alternative on three axes at once:

1. **Novel by construction** — no C++ ancestor exists, so it cannot be
   dismissed as porting another union arm. It exercises the exact
   extension path a third party would use: new ImplementationType tag,
   new AnyTransducer variant, new stream arms, new CLI format name,
   end-to-end through the tools.

2. **Cheap for what it proves** — THFST is byte-for-byte the OLW tables
   with 2 pad bytes per index record plus a JSON alphabet; the in-tree
   weighted optimized-lookup engine IS the runtime. The backend is a
   (de)serialization layer over tables the port already owns, not a new
   engine (contrast the rejected SFST port).

3. **Actually needed** — divvunspell consumes .bhfst/.thfst in
   production; Giella pipelines currently need zip + thfst-tools to
   produce them. `hfst-fst2fst -f thfst` + `hfst-bhfst` collapse that.

The pivotal engineering decision inside the scope is the numbering
contract of the alphabet JSON: rust-hfst's FdTable reproduces C++
HFST's flag numbering (empty value 0, first real value 2) while
divvunspell assigns 0-based first-encounter numbers. Translating
numbers between schemes would be fragile in both directions; deriving
the JSON from the symbol STRINGS by replicating divvunspell's parser —
and, on read, ignoring the JSON numbers and letting the engine
re-derive its own — makes each side internally consistent and the
strings the single source of truth. The oracle asymmetry follows:
index/transition byte-compare against thfst-tools, alphabet
semantic-compare (hash-map key order is nondeterministic in the
reference producer itself).

The directory-vs-stream tension is resolved at the filename boundary,
where a path is data: directory in, preloaded transducer out; directory
sink on output with hfst_format forced off (the XFSM native-only
precedent). Keeping THFST out of the byte-sniffing tables makes the
illegal states (stdin THFST, THFST inside a multi-transducer stream)
unrepresentable rather than checked.
