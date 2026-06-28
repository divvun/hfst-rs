---
id [dec:hfst:hfst-c-out-of-scope]
epitome "hfst-c (the C FFI crate) is out of scope: incomplete, unused, and headed for a redesigned API, so it is excluded from the workspace build and must not constrain the hfst library's public signatures."
state @decided
category @executive
scope {
    elements ([arch:hfst:backend-dispatch])
    rules ()
}
author "brendan@necessary.nu"
alternatives (
    {
        option "Keep hfst-c in the build and preserve the C ABI shape of library signatures (e.g. the lookup-result *mut Hfst{One,Two}LevelPaths returns) so the FFI keeps compiling."
        rejected_because "The C API is incomplete and unused, and is slated for a different API; preserving its ABI was forcing self-imposed *mut boundaries on the library (the lookup-result API deferral) for no real consumer."
    }
    {
        option "Delete crates/hfst-c entirely."
        rejected_because "The source is worth keeping as a starting point for the eventual redesigned FFI; excluding it from the build achieves the same de-coupling without discarding the work."
    }
)
consequences {
    accepted ("hfst-c is removed from `members` via `exclude` in the root Cargo.toml; the workspace no longer compiles it, so it cannot gate library changes." "Library public signatures are free to become idiomatic (owned returns) without an FFI re-wrap; specifically this unblocks converting the lookup-result API (lookup_string_vector / lookup_string / lookup_pairs / lookup_fd_* / lookdown_*) from *mut Hfst{One,Two}LevelPaths to owned values inside idiom1.core." "The library's public surface loses the hfst-c compile-time check; that is acceptable given hfst-c is unused.")
    deferred ("A future C FFI is a dedicated effort that will wrap whatever the idiomatic library API settles on, not the current 1:1-port shapes.")
}
edges {
    requires ([dec:hfst:idiomatize-staging])
}
codifies ()
establishes ()
---

## Rationale

The lookup-result API was the only `idiom1.core` residual whose deferral reason
was real ABI coupling rather than an inherent structural problem: `hfst-c`
returned those `*mut Hfst{One,Two}LevelPaths` straight across the C boundary, so
converting them to owned values would have meant an FFI re-wrap or a broken
`hfst-c`. With `hfst-c` declared out of scope — it is incomplete, has no users,
and will get a different API — that coupling disappears and the conversion
becomes a clean in-tree change (callers are the hfst-lookup/flookup tools, the
xfst compiler, generate_model_forms, and a covered test). Excluding rather than
deleting keeps the FFI source available for the eventual redesign. The other
deferred raw-pointer items (the OL const-cast island, convert.rs's
self-referential CONSTRUCTING_TRANSDUCER / OL-layout graph, the print FILE*/libc
C-isms) were never FFI-driven and stand on their own grounds.
