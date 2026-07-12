# hfst (Rust port)

An idiomatic Rust re-implementation of [**HFST**](https://hfst.github.io/)
(Helsinki Finite-State Technology), the library and tool suite for building and
applying weighted and unweighted finite-state transducers. HFST is a
backend-neutral framework for computational morphology and other finite-state
NLP: it compiles regular expressions and `lexc` / `twolc` / `xfst` / `pmatch`
sources into transducers, applies them to text (morphological analysis and
generation, tokenisation, pattern matching), and converts between transducer
formats — including its own compact optimized-lookup runtime format. It is the
engine behind a large body of rule-based language technology, notably the
[GiellaLT](https://giellalt.github.io/) / [Divvun](https://divvun.no/)
infrastructure.

This repository contains:

- `crates/hfst/` — the library port: a literal 1:1 translation of `libhfst/src`
  (one Rust module per C++ file), reshaped into idiomatic, `unsafe`-free Rust.
- `crates/hfst-cli/` — the `hfst` command-line multiplexer: 63 tools (`lookup`,
  `regexp2fst`, `lexc`, `twolc`, `xfst`, `pmatch`, `tokenize`, `fst2fst`, …)
  dispatched by subcommand or by `hfst-<verb>` symlink.
- `crates/hfst-openfst/` — the adapter onto
  [`rustfst`](https://github.com/necessary-nu/rustfst) that provides the
  weighted (tropical) transducer backend.
- `docs/spec/port/` — the behavioral specification (per-symbol `def` / `sem`
  rules) that pins the port to the C++ behavior of
  [upstream HFST](https://github.com/hfst/hfst), which served as the porting
  reference.

## Backends

A HFST transducer is backend-neutral; the port implements the same facade over:

- **Tropical / weighted** — via `rustfst` (the `hfst-openfst` adapter). The
  default weighted backend; replaces the C++ OpenFST back-end.
- **foma / unweighted** — the native [`foma`](https://crates.io/crates/foma)
  crate (a sibling Rust port), on by default via the `foma` feature; the fast
  path for unweighted algebra and `.foma` I/O.
- **Optimized-lookup (OL)** — the compact, mmap-friendly runtime format for fast
  analysis / generation lookup.
- **THFST / BHFST** — the transducer and speller-archive formats used by
  [`divvunspell`](https://github.com/divvun/divvunspell); the port reads and
  writes them byte-identically (`hfst fst2fst -f thfst`, `hfst bhfst`).

The `lexc` / `twolc` / `xfst` / `pmatch` / regex (`xre`) front-ends are provided
by the sibling parser crates `nfst-lexc` / `nfst-twolc` / `nfst-xfst` /
`nfst-pmatch` / `nfst-xre` ([`necessary-nu/nfst`](https://github.com/necessary-nu/nfst));
the port walks their typed ASTs and calls the same construction routines the C++
grammar actions would.

## Scope

`libhfst/src` and its back-ends only, ported into a **single `hfst` crate** (the
C++ sources form cross-file cycles that no layered crate split can express). Out
of scope by design: the `libhfst` C API and its Python (SWIG) bindings; the SFST
and native-C++ OpenFST back-ends (replaced by `rustfst` and the native `foma`
crate); and the log-semiring back-end (removed — its non-idempotent
determinisation is intractable, leaving tropical as the one weighted backend).

## Building

There is no crates.io release; build from a checkout of
[`divvun/hfst-rs`](https://github.com/divvun/hfst-rs):

```sh
cargo build                          # the library + the `hfst` multiplexer (foma backend on)
cargo build --no-default-features    # without the native foma backend
cargo nextest run                    # the full test suite (unit + integration + CLI)
# or: cargo test
```

The weighted backend pulls `rustfst` (from
[`necessary-nu/rustfst`](https://github.com/necessary-nu/rustfst)); the `foma`
feature (default) pulls the `foma` crate. Both are ordinary cargo dependencies —
no C toolchain, no submodules.

## Tools

The `hfst` binary is a busybox-style multiplexer: invoke a tool as `hfst <verb>`
or via a `hfst-<verb>` symlink. The 63 tools mirror the upstream `hfst-*`
commands; the most-used:

| Tool | Purpose |
|------|---------|
| `lookup` / `optimized-lookup` | Apply a transducer to input lines — morphological analysis / generation (`input⇥output⇥weight`). |
| `regexp2fst` | Compile a regular expression into a transducer. |
| `lexc` / `twolc` / `xfst` | Compile a `lexc` lexicon / `twolc` two-level rules / run the `xfst` interpreter. |
| `pmatch` / `pmatch2fst` | Compile and run pattern-matching (`pmatch`) grammars over text. |
| `tokenize` | Tokenise text with a `pmatch` tokeniser (analysed-token output). |
| `fst2fst` | Convert between backend formats (tropical / foma / OL / THFST). |
| `fst2strings` / `fst2txt` | Enumerate a transducer's paths / dump AT&T text. |
| `compose` `invert` `minimize` `determinize` `disjunct` `conjunct` … | The transducer algebra. |

### Example

```sh
$ cargo build
$ echo '{cat}:{dog}' | ./target/debug/hfst regexp2fst -o cat2dog.hfst
$ echo cat | ./target/debug/hfst lookup cat2dog.hfst
cat	dog	0.000000
```

`{cat}:{dog}` compiles a transducer mapping the string `cat` to `dog`; `lookup`
runs input through it and prints `input⇥output⇥weight`.

## Module map

The port mirrors `libhfst/src` file-for-file:

- **Facade** — `hfst_transducer` (the backend-dispatching `HfstTransducer`),
  `hfst_basic_transducer` (the interchange graph), `hfst_input_stream` /
  `hfst_output_stream` (format-detecting binary I/O).
- **Backends** — `tropical_weight_transducer` (rustfst), `backend_foma`,
  `hfst_ol_transducer` (optimized-lookup), `thfst_io` / `backend_thfst`.
- **Compilers** — `xre`, `lexc`, `twolc`, `xfst_compiler`, `pmatch_compiler`
  (over the `nfst-*` ASTs).
- **Rules & ops** — `hfst_xerox_rules` (replace rules), `hfst_rules`,
  `compose_intersect_*`, `harmonize_*`.
- **Runtime** — `pmatch`, `pmatch_tokenize`, `ospell`, `hfst_lookup_format`,
  `hfst_extract_strings`.

See `cargo doc -p hfst --open` for the full surface.

## Specification

Every ported symbol carries `// [spec:hfst:def:…]` / `// [spec:hfst:sem:…]`
annotations tying the code to a rule under `docs/spec/port/`, and every rule is
verified by a test carrying the matching `…/test` facet. The library is
`#![forbid(unsafe_code)]` and lint-gated against bare `.unwrap()`
(`#![deny(clippy::unwrap_used)]`).

## License

**LGPL-3.0-or-later**, matching upstream HFST. See [`COPYING`](COPYING).

This is a port; all credit for the original design, algorithms, and C++
implementation goes to the **HFST team at the University of Helsinki**
(<https://hfst.github.io/>, <https://github.com/hfst/hfst>).
