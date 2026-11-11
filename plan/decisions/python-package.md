---
id [dec:hfst:python-package]
epitome "divvun-hfst is a pyo3 extension module, imported as divvun_hfst, over crates/hfst-embed, the surface @divvun/hfst binds too: the same readers from bytes and also from paths, both lookups, tokenising and DHFST, with frozen objects that threads share and the GIL released while the library works."
state @decided
category @existence
scope {
    elements ()
    rules (
        [spec:hfst:req:python.module]
        [spec:hfst:req:python.typed]
        [spec:hfst:req:python.load]
        [spec:hfst:req:python.lookup]
        [spec:hfst:req:python.tokenise]
        [spec:hfst:req:python.dhfst]
        [spec:hfst:req:python.errors]
        [spec:hfst:req:python.threads]
        [spec:hfst:req:python.logging]
        [spec:hfst:req:python.parity]
    )
}
author "brendan@necessary.nu"
decided_at "2026-11-10T00:00:00Z"
alternatives (
    {
        option "Run the npm package's WebAssembly module from Python through a runtime such as wasmtime."
        rejected_because "Every install would carry a second runtime, lookups would run slower than native code, and the package would keep the limits of a target with no files and no threads: no loading by path, and one call at a time."
    }
    {
        option "Publish to PyPI as `hfst`."
        rejected_because "The HFST project publishes its own Python package, the SWIG binding of the C++ library, under that name. Installing both would put two modules called `hfst` on one path."
    }
    {
        option "Follow the HFST project's Python API, `hfst.HfstTransducer` and its relatives."
        rejected_because "It is the whole C++ library through SWIG in C++ names, with transducers changed in place. The package offers the same compilers, operations and writers under Python's names, on transducers that never change, as `[dec:hfst:compile-surface]` sets out."
    }
    {
        option "Copy the reading and running code of crates/hfst-wasm into the Python crate."
        rejected_because "Stream sniffing, token selection, the tokenise settings and DHFST reading would exist twice and drift, and the two packages would agree only as far as their tests reach. Moving that code into crates/hfst-embed, which both bind, makes them agree by construction."
    }
    {
        option "Raise one exception class carrying a code, as `HfstError` is in JavaScript."
        rejected_because "Python code tells failures apart with `except` clauses on classes. The package keeps the npm codes on `HfstError.code`, so the npm package's documentation of them applies, and adds `FormatError` for bytes a reader does not load, the failure callers handle. The wrapper's codes (`not-initialised`, `init-failed`, `poisoned`, `freed`) have no counterpart in a native module that needs no start and whose objects Python's reference counting frees."
    }
    {
        option "Take the tokeniser's options as keyword arguments to `tokenise` and `format`."
        rejected_because "`format` would carry eleven options and `tokenise` seven, six of them shared, and every call would check again options that seldom change. A frozen dataclass is made and checked once, reused for every call, typed for a checker, and varied with `dataclasses.replace`."
    }
    {
        option "Count time limits in milliseconds and token offsets in UTF-16 units, as the npm package does."
        rejected_because "Python counts time in seconds, as `hfst lookup` and `hfst tokenise` do, and indexes a `str` by code point. The npm package's units are JavaScript's."
    }
    {
        option "Keep one run state in each loaded object behind a lock."
        rejected_because "Threads sharing a tokeniser or a transducer would wait for each other. A lookup and a tokenise each make their run state over the shared tables, which costs empty buffers, and the optimised lookup takes divvun-fst's tables from a pool, so they keep their capacity without being shared."
    }
    {
        option "Send each `tracing` event to Python's `logging` as it happens."
        rejected_because "The library runs without the GIL, so every event would have to take it back, and a handler would run in the middle of a lookup. Events are kept for the call and handed over when it returns, below a level read from the root loggers when the call starts."
    }
)
consequences {
    accepted (
        "crates/hfst-embed holds what both packages read and run, with no wasm and no Python in it. crates/hfst-wasm and crates/hfst-python convert their host's values to its types and back, check their callers' arguments in their host's terms, and give it the names of their readers for its errors. It reports token offsets in bytes, and each binding counts them in its own units."
        "crates/hfst-python is a cdylib that no Rust test binary can link, so python/tests tests it through Python. maturin builds it from python/pyproject.toml as `divvun_hfst._divvun_hfst`, under a `divvun_hfst` package that re-exports it."
        "One abi3 wheel per platform covers CPython 3.11 and later. Free-threaded CPython cannot load it."
        "The library's events reach `logging` under two roots, `hfst` and `divvun_fst`, as their targets name them."
    )
    deferred (
        "Building the wheels in CI and publishing them to PyPI on a `python/v<version>` tag."
        "Testing the Windows wheels on Windows."
        "An abi3t wheel for free-threaded CPython."
        "Reading from any buffer, such as a `memoryview` or an `mmap`, rather than `bytes` and `bytearray`."
    )
}
codifies (
    [spec:hfst:req:python.load]
    [spec:hfst:req:python.threads]
    [spec:hfst:req:python.parity]
)
establishes ()
---

## Rationale

`@divvun/hfst` already fixes a read-only surface over the library and checks
it against the native library on shared fixtures. A Python package that binds
the same surface inherits that check, and a native module is the small step
from there: the library builds for every host Python runs on, and Python's
packaging handles one compiled extension per platform well.

The npm package's binding crate held the code that reads streams, selects
tokens and sets up the tokeniser, beside its wasm glue. That code is moved
into crates/hfst-embed, so the two bindings are glue over one implementation
and differ only where their hosts do.

A native host has what a browser lacks. It reads files, so each reader has a
`load_` form that reads the file in the library. It runs threads, so the
binding releases the GIL for every load and every call, and a loaded object
holds only shared tables: each call makes its own run state, as the library's
run-state rules allow.

The surface keeps the npm package's shape and takes Python's conventions where
they differ: snake_case names, option objects as frozen dataclasses,
keyword-only limits, seconds and code points, exception classes and the
standard `logging` module.
