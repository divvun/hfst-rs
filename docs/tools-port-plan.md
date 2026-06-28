# HFST command-line tools — port plan (hfst-cli)

The `tools/src` command-line tools were brought into the port on 2026-06-28.
Scope + fidelity decisions (operator): **core subset** (the flat `hfst-<verb>`
tools + their shared infra; the `hfst-proc`/`hfst-tagger`/`hfst-twolc`/xfst-shell
subsystems and `sfst-main`/`lexc-readline-ui`/`test.cc` are excluded), and
**faithful getopt 1:1** (port `hfst-getopt`/`hfst-program-options` and each tool's
getopt loop literally; readline-gated interactive paths are `#if`'d out like the
SFST backend was).

## State

- Manifest now 3295 symbols (2593 library + **702 tool symbols**). Scope is set in
  `.config/nspec/config.styx` (`source-impl` include adds `tools/src/**`, with the
  subsystem excludes). Re-extract with `nplan_port_extract` after any glob change.
- Wave-1 markup scaffold applied to all tool source (`nplan_port_markup`): every
  tool symbol has its `def` annotation in `tools/src/*.cc` + a seeded `def` rule;
  `sem` bodies are `TODO(sem)` under `docs/spec/port/tools/src/*.md` (71 files).
- Target crate: **`crates/hfst-cli`** (one crate, shared infra in `src/`, one
  `[[bin]]` per tool added as ported). Builds today.
- Done so far: **`hfst-getopt`** (the `getopt_long` fallback) — `src/hfst_getopt.rs`,
  Wave-1 sem + Wave-2 Rust, both ids credited. This is the reference for the flow.

## Per-symbol flow (proven on hfst-getopt)

1. Read the C++ in `tools/src/<file>` (its `[spec:hfst:def/sem:<id>]` annotations
   are already inserted).
2. Author the `sem` body in `docs/spec/port/tools/src/<file>.md` (replace each
   `TODO(sem)`), precise enough to re-implement from the rule alone.
3. Translate 1:1 into `crates/hfst-cli/src/<module>.rs` (or `src/bin/<tool>.rs`),
   carrying the same `// [spec:hfst:def:<id>]` + `// [spec:hfst:sem:<id>]` lines.
   `unsafe`/raw pointers/`static mut` (via `addr_of_mut!`, edition-2024) expected.
4. `cargo build -p hfst-cli`; commit per file/tool via the usual `nplan_commit`.

## Dependency order (do the foundation before the tools)

Tools share a foundation via C `#include "inc/..."` fragments + library helpers.
Port these first, into `hfst-cli/src/`:

1. `hfst-getopt` — DONE.
2. `hfst-tool-metadata` (`hfst_get_name`/`hfst_set_name`/`hfst_set_formula`…),
   `hfst-file-to-mem`.
3. `hfst-program-options` (the `print_common_*_program_options` help text).
4. `hfst-commandline` (~54 symbols: `program_name`/`message_out`/`verbose_printf`/
   `error`/`hfst_set_program_name`/`extend_options_getenv`/
   `is_input_stream_in_ol_format`/stream open helpers/locale).
5. The `inc/` fragments — NOT in the manifest (they are switch-body/`globals`/
   `check-params` `.h` fragments `#include`d into each tool). Port them once as
   shared Rust: a `globals` module mirroring `inc/globals-{common,unary,binary}.h`
   (the process-global mutable tool state), and shared functions/macros for the
   `getopt-cases-*` and `check-params-*` fragments. This is the make-or-break
   design step; once it exists every tool main is thin.

Then the tool mains (`src/bin/<tool>.rs`), simplest first: `hfst-invert`,
`hfst-reverse`, `hfst-project`, the other unary ops, then the binary ops
(`hfst-compose`/`-conjunct`/`-disjunct`/`-concatenate`/`-subtract`), then the
I/O tools (`hfst-fst2strings`/`-txt2fst`/`-strings2fst`/`-fst2txt`), then the
compiler front-ends (`hfst-lexc-compiler`/`-pmatch2fst`/`-regexp2fst`), then the
lookup tools (`hfst-lookup`/`-flookup`/`-optimized-lookup`). `hfst-invert.cc` is
the canonical unary template; read it first.

## STATUS 2026-06-28: 56 tools built (fan-out workflow complete)

The two-workflow approach landed: foundation workflow, then a 59-agent fan-out +
serial integrate. **56 of 59 tool bins build clean and run `--help`** (commits
`2647b49a`, `47118059`, `7971b707`). Wave-2 coverage 2299 -> **2867/3295 (87%)**.
`cargo build -p hfst-cli` is clean.

**4 deferred (deleted from src/bin; need an un-ported library item):**
- `hfst-guess` — port `tools/src/generate_model_forms.{cc,h}` into the `hfst`
  crate (StringVectorVector, read_model_forms, get_guesses, get_paradigms, …).
- `hfst-guessify` — port `tools/src/guessify_fst.{cc,h}` (`guessify_analyzer`,
  `store_guesser`, `CATEGORY_SYMBOL_PREFIX`).
- `hfst-lexc-compiler` — needs an incremental `LexcCompiler::parse(&str)` entry
  point in the lib (it currently only exposes one-shot `compile`).
- `hfst-unary-tool` — the example/template tool; calls a placeholder
  `HfstTransducer::do_stuff()` that does not (and need not) exist. Leave deferred.

**Built-but-runtime-gapped (compile + `--help` fine, panic on one path):**
- `hfst-tokenize`, `hfst-pmatch` — the pmatch-script / "TOP" path calls
  `PmatchContainer::{new_from_stream,parse_hfst3_header}`, still `unimplemented!`
  in `crates/hfst/src/pmatch.rs` (Wave-3 lib work). Their naive/non-TOP paths are
  fully wired.

**Integrate-stage mechanical fixes worth knowing (faithful, no logic change):**
13 bins needed `#![allow(static_mut_refs)]` (edition-2024) for their C file-scope
statics; several bins needed the macOS `link_name="__std*p"` stream fix; a few had
import-path/`libc::getc`->`fgetc`/`clock` portability fixes.

## Proven so far

- `hfst-getopt` (committed) and the foundation (`globals`, `hfst_commandline`,
  `hfst_program_options`, `hfst_tool_metadata`, `hfst_file_to_mem`, `inc`) build
  clean. First tool **`hfst-invert`** is ported (`src/bin/hfst-invert.rs`): builds,
  links, `--help` prints faithful usage (exit 0), and it functionally inverts +
  writes a binary transducer.
- macOS link fix: the std-stream externs (`stdin`/`stdout`/`stderr`) needed
  `#[cfg_attr(target_os = "macos", link_name = "__std*p")]` — the lib only surfaced
  this when the first **binary** linked. Applied across globals/getopt/commandline/
  file-to-mem/inc.

## FINISHED follow-ups (no longer deferred)

- **Multi-byte header bug — FIXED** (`a5ab4f3a`): `stream_getstring` re-encoded
  bytes >= 0x80 as UTF-8, corrupting multi-byte header properties and overrunning
  the header length. Now decodes raw bytes once; `hfst-invert` round-trips
  `a:b`->`b:a`.
- **3 previously-deferred tools — DONE** (`038564aa`): ported the missing lib
  pieces `generate_model_forms`, `guessify_fst`, and incremental
  `LexcCompiler::parse`, and re-added `hfst-guess` / `hfst-guessify` /
  `hfst-lexc-compiler` (build + `--help`).
- **pmatch binary-archive reader — DONE** (`533341cd`): `IStream` gained
  `get()`/`putback()`; `parse_hfst3_header`, `PmatchAlphabet::new_from_stream`,
  and `PmatchContainer::new_from_stream` ported (were `unimplemented!`). Verified:
  reads a `TOP` archive and `match_("a")` returns `"b"`. Unblocks
  `hfst-tokenize`/`hfst-pmatch` TOP path.

## Still UNFINISHED Wave-2 (not deferred — real translation gaps)

24 in-scope `unimplemented!` remain in the LIBRARY (not the tools; tools build and
their main paths work). The bulk is the per-backend **input-stream
format-detection layer**, now partly unblocked by the new `IStream`
`get`/`putback`:
- tropical/log/ol `*InputStream`: `is_fst` / `is_fst(istream)` / `stream_unget` /
  `new` / `new_istream` (~17 across `tropical_weight_transducer.rs`,
  `log_weight_transducer.rs`, `hfst_ol_transducer.rs`, `hfst_input_stream.rs`).
  These detect a stream's transducer format and read OL/OpenFST backends through
  `HfstInputStream`. `HfstInputStream::new_istream` is the one structural snag (an
  owned-reader port adopting a borrowed `IStream`).
- `pmatch.rs`: `process_symbol_list` (needs `&mut PmatchContainer`) + 2 others.
- `twolc.rs` (1), `compose_intersect_lexicon.rs` (1).
The genuinely out-of-scope `unimplemented!`s (SFST/foma/xfsm backends) are not
counted here.

## Guardrails

- `cargo nextest run` for tests (process isolation); avoid backticks in NEW
  comments (cheap hygiene — the credit-zeroing bug is fixed but be tidy).
- A tool whose library dependency is not ported (e.g. guess/guessify, pair-test)
  will fail to build — defer it and `log` the deferral; do not stub-fake it.
- Each `hfst-cli` source file is one editor/agent owner at a time (shared crate;
  concurrent edits corrupt the build).
