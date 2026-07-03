---
id [dec:hfst:monomorphic-backends]
epitome "Monomorphize the entire backend dispatch in one pass: HfstTransducer<B: Backend> with capability traits; runtime type exists only at the stream/format boundary (divvunspell precedent)."
state @decided
category @executive
scope {
    elements ([arch:hfst:backend-dispatch])
}
author "brendan@necessary.nu"
alternatives (
    {
        option "Closed-variant enum dispatch (match per accessor/method)."
        rejected_because "Not monomorphization: still a branch per hot-path access, and the capability mismatch (algebra ops on lookup-only backends) stays a runtime error. Operator rejected 2026-07-03."
    }
    {
        option "Incremental genericization (OL tables first, facade later, tools later)."
        rejected_because "Generic refactors ripple through every signature they touch; doing it twice means re-breaking every consumer twice. Operator: 'making things generic is painful and should be done all in one pass'."
    }
)
consequences {
    accepted ("The working tree is red for the whole pass; the oracle (153 tests + byte-identical lang-sma artifacts) is only checked at the end and at designated midpoints.")
    accepted ("The ~103 FunctionNotImplemented/TransducerHasWrongType runtime bail sites for capability mismatches become compile-time impossibilities and are deleted, a deliberate behavior change from C++ (errors move from runtime to compile time).")
    accepted ("CLI tools that read arbitrary .hfst files dispatch once at the stream boundary; each tool body is a generic fn instantiated per backend.")
    deferred ("SmolStr symbol interning is a separate pass (task #27).")
    deferred ("hfst-c FFI re-binding to the generic facade (hfst-c is out of scope per [dec:hfst:hfst-c-out-of-scope]).")
}
codifies ()
establishes ([arch:hfst:backend-dispatch])
---

## Rationale

Rust monomorphizes what C++ devirtualizes. The literal port carried C++'s
union+type-tag facade (`HfstTransducer { ty, implementation }`), which pays a
runtime dispatch on ~102 sites per facade call and re-checks capability
(`HFST_THROW(FunctionNotImplementedException)`) on ~103 more. divvunspell —
the hand-optimized hfst-ospell descendant — demonstrates the target shape:
generics + capability bounds all the way down, with exactly one runtime sum
at the point where file bytes (whose type is data, not code) enter the
program.

## The design

### Backends

The backend data types themselves implement the marker + common surface:

- `StdVectorFst`  (tropical weights)      -> algebra backend
- `LogFst`        (log weights)           -> algebra backend
- `ol::Transducer<WeightedTables>`   (HFST_OLW_TYPE) -> lookup backend
- `ol::Transducer<UnweightedTables>` (HFST_OL_TYPE)  -> lookup backend

`ol::Transducer<T: TransducerTablesInterface>` is itself generic over the
table pair (landed in this pass, transducer.rs): the innermost lookup loop is
monomorphic per weightedness.

### Traits (new module `crates/hfst/src/backend.rs`)

```rust
pub trait Backend: Sized {
    const TYPE: ImplementationType;       // serialization tag
    fn write(&self, ...);                 // stream write
    // common metadata/alphabet surface used by every facade path
}

pub trait AlgebraBackend: Backend {
    // the mutable FST algebra: one method per former apply/apply_bool
    // closure pair — compose, determinize(encode_weights), minimize(..),
    // union, concatenate, n_best, push_weights, substitute, harmonize, ...
    // Bodies move verbatim from TropicalWeightTransducer/LogWeightTransducer
    // wrappers; no logic change.
}

pub trait LookupBackend: Backend {
    // lookup_fd*, is_lookup_infinitely_ambiguous, ...
}
```

### Facade

```rust
pub struct HfstTransducer<B: Backend> {
    name: String,
    props: BTreeMap<String, String>,
    anonymous: bool,
    is_trie: bool,
    fst: B,                                // was: ty + TransducerImplementation
}
impl<B: Backend>         HfstTransducer<B> { /* metadata, write, alphabet */ }
impl<B: AlgebraBackend>  HfstTransducer<B> { /* the ~150 algebra ops */ }
impl<B: LookupBackend>   HfstTransducer<B> { /* lookup surface */ }
```

`ImplementationType` and `TransducerImplementation` (the union port) are
deleted from the facade; `ImplementationType` survives only as the stream
header tag and CLI `--format` value.

### Conversions (typed, replacing `convert(ty)`)

- `HfstTransducer<StdVectorFst> <-> HfstTransducer<LogFst>` (via basic)
- `HfstTransducer<B: AlgebraBackend> -> HfstTransducer<ol::Transducer<WeightedTables>>`
  (hfst_basic_transducer_to_hfst_ol; C++ always builds W-shaped tables in
  memory even for HFST_OL_TYPE output — preserved)
- `HfstTransducer<ol::Transducer<T>> -> HfstTransducer<B: AlgebraBackend>`
  (hfst_ol_to_hfst_basic_transducer + basic->backend)

### The one runtime sum (stream/format boundary)

```rust
pub enum AnyTransducer {                   // produced ONLY by readers
    Tropical(HfstTransducer<StdVectorFst>),
    Log(HfstTransducer<LogFst>),
    OlW(HfstTransducer<ol::Transducer<WeightedTables>>),
    OlU(HfstTransducer<ol::Transducer<UnweightedTables>>),
}
```

`HfstInputStream::read -> AnyTransducer`; tool drivers match once and enter
generic fns. Compilers (Xre/Lexc/Twolc/Xfst/Pmatch) become generic over
`B: AlgebraBackend` — their `format` field moves into the type; each CLI
main matches `--format` once: `Tropical => run::<StdVectorFst>() | Log =>
run::<LogFst>()`. Binary tools (compose etc.) require both operands the same
B (match the pair; cross-type follows the C++ convert-then-operate semantic,
now as an explicit typed conversion).

The pmatch/ospell runtimes pin concrete: `PmatchContainer` and the speller
hold `ol::Transducer<WeightedTables>` (pmatch archives are always weighted).

### Pass order (single pass, tree red until the end)

1. transducer.rs: `Transducer<T>` core (DONE in this pass), satellites
   (convert_ol, hfst_ol_transducer, ospell generic, pmatch pinned).
2. backend.rs traits; impls for the four backends (bodies moved from the
   weight-transducer wrappers / apply closures).
3. hfst_transducer.rs facade rewrite; delete ty/TransducerImplementation;
   AnyTransducer + HfstInputStream/HfstOutputStream.
4. Compilers generic: xre, lexc, twolc, xfst_compiler, pmatch_compiler,
   rules, xerox_rules, compose_intersect (kills dyn RuleT /
   ComposeIntersectRuleObject as part of the same reshape).
5. hfst-cli: per-tool driver dispatch (format flag / stream type), 59 tools.
6. Tests: mostly tropical — instantiate concretely.

### Oracle

`cargo build --workspace --all-targets` clean; `cargo nextest run -p hfst -p
hfst-cli` 153 passed; lang-sma speller rebuild byte-compares against the
pre-pass artifacts (acceptor/errmodel/zhfst sizes + divvunspell suggest
smoke); `hfst summarize`/`tokenise` smoke on the sma .hfstol/.pmhfst.
