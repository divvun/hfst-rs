---
id [dec:hfst:idiomatize-staging]
epitome "Idiomatize the HFST Rust port in named, test-gated stages: Stage 1 = memory-safety of the hfst library; Stage 2 = remove all C-isms / drop libc; each stage minimal and behavior-preserving."
state @decided
decided_at "2026-06-28T11:53:36Z"
category @executive
scope {
    elements ([arch:hfst:backend-dispatch] [arch:hfst:symbol-coding])
    rules ()
}
author "brendan@necessary.nu"
alternatives (
    {
        option "Big-bang idiomization: safety + Result-ification + naming + trait redesign in one pass."
        rejected_because "Diverges from the test-validated 1:1 port all at once, so any regression is un-localizable against the 110-test behavioral contract."
    }
    {
        option "Per-crate full rewrite to idiomatic Rust."
        rejected_because "Discards the [spec:hfst:...] 1:1 traceability annotations and the test parity that make the port auditable."
    }
)
consequences {
    accepted ("Every WBS node keeps the 110-test suite green and the workspace building; spec annotations are preserved." "Stage 1 drives hfst unsafe + static mut to ~0 (minus documented, flagged islands)." "Stage 2 removes the libc crate from every Cargo.toml.")
    deferred ("Stage 3+: de-singletonization of the safe global coding tables (thread a context), Result-ification / error model, renames, iterator-ification, trait redesign.")
}
establishes ([arch:hfst:backend-dispatch] [arch:hfst:symbol-coding])
---

## Rationale

The literal 1:1 C++->Rust port (Waves 2-3) is functionally complete and validated by
110 passing tests. It is deliberately non-idiomatic: a `union`, raw pointers, manual
`new`/`delete`, and `static mut` were all sanctioned to mirror the C++ exactly. Wave 4
makes it idiomatic, but doing everything at once would untether the result from the only
oracle we have (the ported tests + `sem` rules), making regressions impossible to bisect.

So idiomization proceeds in **named, minimal, behavior-preserving stages**, each gated by
the unchanged 110-test suite:

- **Stage 1 — memory-safety (hfst library).** The two worst properties are unsafety and
  global mutability. Convert the `TransducerImplementation` union to an `enum`, raw owning
  pointers to `Box`/owned values, and every `static mut` to a safe form. No renames, no
  error-model changes, no other structural change — except where removing unsafety
  *requires* it (the union->enum is the one sanctioned structural edit). Already-safe
  global singletons (the symbol-coding registry, [arch:hfst:symbol-coding]) are left as
  safe globals; de-globalizing them is structural and deferred.

- **Stage 2 — de-C-ism / drop libc.** Replace `FILE*` I/O, C string buffers, char-level
  `getc`/`fputc`, `printf`, `clock`, and the extern stream statics with Rust-native
  `std`/`core::ffi` equivalents across all crates until the `libc` crate dependency can be
  removed entirely. `CString`/`CStr` (std::ffi, not the libc crate) remain at the
  `hfst-c` FFI boundary.

The deferred costs (Stage 3+) buy a smaller, auditable blast radius per stage: each stage
is independently shippable, leaves the behavioral contract intact, and surfaces the next
stage's targets without forcing them early.
