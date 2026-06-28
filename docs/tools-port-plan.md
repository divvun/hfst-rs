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

## Input-stream / format-detection layer — DONE

The 24-stub `unimplemented!` cluster has been worked down to **0 in-scope
remainders + 2 non-gaps**. Cleared (`2bc7e52c`, `8f3252dc`, `6a0eac0`): `IStream`
gained `new_owned`/`clear`/`read_to_end`; tropical/log/ol `*InputStream`
`new`/`new_filename`/`is_fst`/`is_fst(istream)`/`stream_unget` (14); tropical/log
`read_transducer` via `read_to_end`+`load_prefix` (2); pmatch `process_symbol_list`
threaded with `&mut PmatchContainer` (1). All build; lib tests green.

### Final 4 — DONE
- `pmatch.rs` `uncompose` (`pmatch.cc:2756`): ported via `uncompose_left`/`_right`
  `lookup_fd` + midform reconstruction (the "needs HfstTransducer facade" label
  was wrong — it uses the optimized-lookup `Transducer::lookup_fd`, already ported
  as `lookup_fd_str`).
- `pmatch.rs` multi-archive harmonization (`PmatchContainer(std::vector<HfstTransducer>)`
  else-branch): ported the shared-alphabet harmonizer — build a TROPICAL
  harmonizer over every member's symbols, convert to OLW, re-route TOP and each
  RTN through `hfst_transducer_to_hfst_basic_transducer` +
  `hfst_basic_transducer_to_hfst_ol(..., harmonizer_ol)`. Bug-for-bug: members
  named "TOP" leave a NULL temporaries slot; no-TOP fallback re-adds member 0 as
  both TOP and an RTN.
- `hfst_input_stream.rs` `new_istream`: made `PushbackReader<'a>` /
  `HfstInputStream<'a>` lifetime-generic so a borrowed `IStream<'a>` can back the
  stream; `IStream::into_reader` adopts it (replays LIFO putback via a
  `Cursor.chain`).
- `twolc.rs` `mixed` where-clause: ported `MixedConstContainerIterator` as a
  precomputed combos list on `VarDim` — the block's value-position cross-product
  filtered to pairwise-distinct positions (the C++ `equal_indices` skip).

### Correctly NOT gaps (left intentionally)
- `compose_intersect_lexicon.rs` `identity_compose`: declared in a header but
  never defined in C++ (like `universal_fst`/`negation_fst`) — `unimplemented!`
  IS the faithful port (calling it would be a link error in C++).
- `hfst_input_stream.rs:479` HFST version-2 weighted transducer with an appended
  **SFST** alphabet: the SFST format is out of scope (dropped backend).

## Guardrails

- `cargo nextest run` for tests (process isolation); avoid backticks in NEW
  comments (cheap hygiene — the credit-zeroing bug is fixed but be tidy).
- A tool whose library dependency is not ported (e.g. guess/guessify, pair-test)
  will fail to build — defer it and `log` the deferral; do not stub-fake it.
- Each `hfst-cli` source file is one editor/agent owner at a time (shared crate;
  concurrent edits corrupt the build).
