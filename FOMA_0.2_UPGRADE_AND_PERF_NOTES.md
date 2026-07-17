# foma 0.2.0 upgrade + performance handoff

**Date:** 2026-07-17
**Author:** foma-side session (CSR line-table migration + perf series), handed to the hfst agent.
**Status of the foma bump:** DONE and verified — see §2. This doc is mostly §3–§4: what to *investigate and test* on the hfst side.

---

## 1. What landed in foma 0.2.0 (published to crates.io)

A multi-commit performance + memory pass on the `foma` crate, culminating in a
compressed in-memory line-table representation. All of it is **byte-identical**
at the FST level — the serialized `.foma`/`.att` bytes and the accepted
languages are unchanged; only the in-memory representation and speed changed.

Highlights (each is a separate commit in the `foma` repo):

- **CSR line table.** `Fsm.states` changed from a flat `Vec<FsmState>` (16 bytes
  per arc-row, with the state number / final / start flags redundantly repeated
  on every arc) to a compressed per-state form: one `StateBlock`
  `{state_no:i32, arc_len:u32, final:i8, start:i8}` (12 bytes) per state, plus a
  flat `CsrArc {in:i16, out:i16, target:i32}` (8 bytes) array. Consumers that
  still want flat rows call `net.states.rows()` (a materialized guard) or
  `net.states.rows_mut()` (recompresses on drop); hot paths read
  `net.states.blocks()` / `iter_blocks()` natively.
  - **Memory:** heap is `12*states + 8*arcs` vs the old `16*(arcs+states+1)`.
    Ratio ≈ `(12 + 8k)/(16(k+1))` where `k = arcs/state`. Dictionary/DAWG shapes
    (k≈6–8) → ~54% of the old size (~46% cut). Sparse chains (k≈1) → ~62% (now a
    win; used to be *worse* than flat). It is **arc-density dependent** — do not
    quote a flat "40%".
- **DAWG union.** A union of literal strings (`a|ab|abc|…`) compiles straight to
  a trie instead of folding N unions and determinizing the pile (~40× on a
  1000-word dictionary).
- **Determinize:** open-addressed subset-hash (no per-node Box chains).
- **Union:** built natively in the compressed form (no materialize/splice/recompress).
- **apply / fsm_read / apply_med:** each keeps ONE flat snapshot of the line
  table in its handle, built at init, so per-arc accessors don't re-materialize.
  (This one bit hard — see §3.1.)

**Breaking API change:** `Fsm.states` is now `LineTable`, not `Vec<FsmState>`;
`is_loop_free` (and the other `is_*` flags) are the `Tern` enum. That's why this
is 0.2.0 and not 0.1.2 (a 0.x breaking change bumps the minor, so downstreams
opt in — `^0.1.1` does NOT auto-pull 0.2.0).

---

## 2. What I already did on the hfst side (DONE, verified)

- `crates/hfst/Cargo.toml`: `foma = { version = "0.2", optional = true }`.
- `crates/hfst/src/backend_foma.rs`:
  - 3× `for line in &self.net.states` → `for line in self.net.states.rows().iter()`
    (`to_basic`, `initial_input_symbols`, `is_automaton`).
  - `sorted.is_loop_free == 0` → `== foma::types::Tern::No`.
- **Verified:** builds against local foma 0.2.0 (path patch) AND the published
  registry crate; **226 `-p hfst` tests pass**.
- Committed as `c812820` (`deps: bump foma to 0.2, adapt foma backend to the
  LineTable API`) with `NPLAN_ALLOW_BARE_COMMIT=1` (dep-bump, no plan node).

Nothing else in hfst uses foma internals that changed — every other `foma::`
call is a function whose signature was untouched.

**Note:** the foma→`HfstBasicTransducer` conversion in `to_basic` currently
materializes the whole flat table via `.rows()`. That's fine (one-time per
conversion), but foma 0.2.0 also exposes a native `net.states.iter_blocks()`
that yields `(&StateBlock, &[CsrArc])` with no materialization — a cheap win if
`to_basic` ever shows up in a profile.

---

## 3. Techniques from the foma work that MAY transfer to hfst-native code

**Discipline first:** on the foma side every one of these was justified by a
*measurement*, and one "obvious" conversion caused a **100× regression** before
it was fixed (§3.1). Do NOT port these on faith — profile the specific hfst hot
path first, then apply, then re-measure. The items below are ranked by expected
value, with concrete pointers into the hfst tree.

### 3.1 Per-call index/materialization footgun — AUDIT THIS FIRST

The single highest-value lesson. On foma, the apply path went **32µs/word →
after a naive conversion → and back to ~380ns/word** once fixed. The bug:
accessor functions that rebuild an index (or materialize a whole table) *per
call*, invoked inside a per-arc hot loop → accidentally O(n²).

Where to look in hfst:
- **`crates/hfst/src/transducer.rs`** — the OL (`Transducer<T>`) lookup
  traversal (`get_analyses`, the tape/traversal machinery). Any helper that
  scans a table or rebuilds state on each transition step is the smell.
- **`crates/hfst/src/hfst_basic_transducer.rs`** — read passes over
  `state_vector`. Any accessor that recomputes something derivable-once.
- Pattern to grep for: a function taking `&self` that allocates or iterates a
  whole table, called from inside another loop.

Fix pattern (from foma): build the index/snapshot ONCE at the start of the
operation (store it on the handle / in a local above the loop), index that.

### 3.2 `HfstBasicTransducer` is `Vec<Vec<transition>>` — heavier than it looks

`HfstBasicStates = Vec<HfstBasicTransitions>` (`hfst_basic_transducer.rs`): one
`Vec` header (24 bytes) **per state**, each a separate heap allocation. For a
large lexicon held in this form, that per-state overhead + allocation
fragmentation is *more* bloated than even foma's old flat table.

- **CSR is NOT the answer here directly** — this is the *mutable construction*
  format, and CSR is hostile to incremental mutation. The right moves are:
  (a) convert to the OL format sooner for anything read-heavy, or
  (b) apply the §3.1 snapshot pattern for read-heavy passes, or
  (c) if a large `HfstBasicTransducer` genuinely lives in memory long-term, a
      flatten-on-freeze (arcs into one `Vec` + per-state offsets) — measure first.
- **The OL format (`Transducer<T>`) is already the CSR idea** (a flat, compact
  runtime table). Don't try to "CSR-ify" it — it already is one.

### 3.3 DAWG-for-union-of-strings — partly already present (`is_trie`)

hfst already threads an `is_trie` flag through construction
(`crates/hfst/src/hfst_transducer.rs:221` and propagation). So some trie
optimization exists. The question is whether **lexc / `disjunct`** exploits it
as aggressively as routing a literal-string union *straight to a trie* rather
than N-way disjunct + determinize.

- Look at `crates/hfst/src/lexc.rs` (`compile`) and the `disjunct` paths
  (`backend.rs`, `tropical_weight_transducer.rs`, `twolc.rs`).
- Test: build a big word-list lexicon and compare wall-clock/peak-RSS against a
  trie-first path (if one doesn't exist, that's the opportunity).

### 3.4 Open-addressing + FxHash for hot hash tables

Anywhere hfst does its OWN subset-construction dedup, symbol interning, or
compose-state hashing (its OpenFST-side `determinize`/`compose` in
`tropical_weight_transducer.rs`, the `SymbolCoder` interning in
`hfst_basic_transducer.rs`). foma got a measurable determinize win from
open-addressed probing (no per-collision Box) — the same applies to any
chained-hashmap on a hot construction path. Low risk, needs a profile to
confirm it's actually hot.

---

## 4. Concrete things to TEST / MEASURE

1. **Memory footprint on a real lexicon via the foma backend.** The stated
   motivation for the foma backend (see the internal notes) was large-FST
   blowup. Load a big morphological analyzer / the sma-tokeniser archive through
   the foma backend and compare peak RSS on foma 0.1.1 vs 0.2.0. Expect
   ~40–46% less *arc* memory for dictionary-shaped FSTs. (Note: this is distinct
   from the OpenFST *state-count* blowup foma already fixes via unweighted
   determinize — CSR is about bytes-per-arc, not number of states.)

2. **Apply / lookup throughput** through the foma backend, before/after. Should
   be flat-to-slightly-better (foma restored apply to baseline after the flip).
   If it *regressed*, that's a §3.1 per-call-materialization bug leaking through
   the backend seam — file it.

3. **Regression guard:** the 226 `-p hfst` tests already pass against 0.2.0.
   Add a parity test if there isn't one: same input FST → identical results on
   foma 0.1.1 vs 0.2.0 (they should be byte-identical FSTs).

4. **`build_bench` / `apply_bench` on the foma side** are examples in the foma
   repo (`crates/foma/examples/`) if you want to see the foma-level numbers.
   NB: `apply_bench`'s `csr_model` print under-counts per-state cost (models 8B,
   real StateBlock is 12B) — trust the formula in §1, not that print.

---

## 5. Gotchas encountered (so you don't rediscover them)

- **Publishing foma:** `cargo publish` needs network; if your shell is
  sandboxed it'll 403. Run with sandbox off or via `!`. Also: `cargo publish`
  refuses on a dirty tree — the untracked `examples/*bench.rs` had to be
  committed first.
- **Semver:** breaking 0.x changes → bump the MINOR (0.1→0.2), so `^0.1.1`
  pins don't silently pull a source-incompatible crate.
- **nplan MCP is CWD-pinned to foma**, so it can't commit in hfst's plan
  context. hfst's pre-commit hook blocks bare `git commit` but sanctions
  `NPLAN_ALLOW_BARE_COMMIT=1` for dep bumps / doc-only changes (that's how
  `c812820` and this file land).
- **Concurrent-edit hazard on hfst main** is live (commit `e7216d4`, the OL
  little-endian work, landed mid-session). Stage files explicitly; never
  `git add -A` here.

---

## 6. Migration technique reference (if you do a CSR-style change on hfst)

The foma flip used a **seam-then-flip** approach that kept the tree green
throughout, worth copying if you compress `HfstBasicTransducer` or similar:

1. **Prove the round-trip in isolation first.** Write `from_rows`/`to_rows` (or
   your compress/decompress pair) and assert `decompress(compress(x)) == x`
   byte-for-byte — then wire that assertion into a hot, always-called path (foma
   used `fsm_count`) so the *entire existing test corpus* exercises it before you
   flip the storage. This caught every edge case (marker rows, trailing garbage
   past the terminator, an invariant about per-state constant flags) with zero
   guesswork.
2. **Introduce a seam** (a newtype that Derefs to the old representation) so
   consumers keep compiling, THEN flip the internals and let the compiler
   enumerate every straggler.
3. **Convert per-scope, never per-access** — hoist one guard/snapshot per
   function, never call the materializer inside a loop (that's the §3.1 bug).
