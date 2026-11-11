---
id [dec:hfst:compile-surface]
epitome "The packages do what the hfst tools do to a transducer: hfst-embed's compile feature compiles, transforms, inspects and writes transducers of every type through library functions the tools call, divvun-hfst binds all of it, and @divvun/hfst offers it as a second entry point, @divvun/hfst/compile, whose .wasm only a caller who compiles loads."
state @decided
category @existence
scope {
    elements ()
    rules (
        [spec:hfst:def:embed.tools]
        [spec:hfst:req:embed.tools-in-library]
        [spec:hfst:req:embed.no-panic]
        [spec:hfst:req:embed.transducer]
        [spec:hfst:req:embed.compile]
        [spec:hfst:req:embed.compile-files]
        [spec:hfst:req:embed.compile-diagnostics]
        [spec:hfst:req:embed.operations]
        [spec:hfst:req:embed.writers]
        [spec:hfst:req:python.load]
        [spec:hfst:req:python.threads]
        [spec:hfst:req:python.transducer]
        [spec:hfst:req:python.compile]
        [spec:hfst:req:python.compile-errors]
        [spec:hfst:req:python.operations]
        [spec:hfst:req:python.writers]
        [spec:hfst:req:python.compile-parity]
        [spec:hfst:def:javascript.packages]
        [spec:hfst:req:javascript.read-only-module]
        [spec:hfst:req:javascript.compile-module]
        [spec:hfst:req:javascript.compile-files]
        [spec:hfst:req:javascript.compile-errors]
        [spec:hfst:req:javascript.compile-parity]
    )
}
author "brendan@necessary.nu"
decided_at "2026-11-11T00:00:00Z"
edges {
    requires ([dec:hfst:python-package] [dec:hfst:librarify-cli] [dec:hfst:package-versions])
}
alternatives (
    {
        option "Compile and transform in the main npm entry point."
        rejected_because "The compilers, the algebra and both backends would be in the module every page that only looks words up loads, and `javascript.read-only-module` exists to keep them out. A second entry point with its own `.wasm` costs those pages nothing."
    }
    {
        option "Publish compiling as a separate npm package, such as `@divvun/hfst-compile`."
        rejected_because "Two packages carry two versions, which a user has to keep in step so that one's bytes load in the other. An entry point of the same package has its version by construction."
    }
    {
        option "Follow the HFST project's Python API, `hfst.HfstTransducer`, `hfst.regex` and `hfst.compile_lexc_file`, so scripts written for it run unchanged."
        rejected_because "Its names are the C++ library's. divvun-hfst is a clean break from that binding, not a replacement for it, and keeps one convention, Python's, across everything it offers."
    }
    {
        option "Transducers that never change, each operation returning a new one."
        rejected_because "The library's operations work in place, so every step would first copy the transducer, doubling peak memory on the language-sized networks the compile surface exists to build. A lock held for the length of an operation costs a thread that shares a transducer some waiting; a copy costs every caller memory on every step. A caller who wants the original keeps a `copy()`."
    }
    {
        option "Bind each tool's command line, taking its arguments as strings."
        rejected_because "An argument list has no types for a checker, and it carries what only a process has: file names, standard streams, `--help`. Typed options over bytes and text are what the packages already do for lookup and tokenising."
    }
    {
        option "Give the compile entry point WASI's files."
        rejected_because "A browser has no WASI. The library's file resolver already takes named files in memory, which serves a browser, and the filesystem, which serves Python and the tools."
    }
)
consequences {
    accepted (
        "What still decides a tool's output in hfst-cli moves into the library: strings2fst, the expressions regexp2fst splits and the sources lexc reads, conversion to a stream's type, format names, the BHFST packer and each query's report."
        "The remaining panics in the compilers become errors."
        "hfst-embed gains a `compile` feature. hfst-python enables it; hfst-wasm does not, and a new hfst-wasm-compile crate does, so the main module's build check still holds."
        "divvun-hfst's `Transducer` holds any type, so `read_transducers` reads tropical OpenFst and foma streams it refused before."
        "A Python `Transducer` holds a lock that an operation takes for its length, so a thread that calls a transducer another thread is changing waits for it."
    )
    deferred (
        "The tools that only move a stream or talk to a terminal, which `embed.tools` lists."
        "Writing THFST from the npm package, which has no directories."
    )
}
codifies (
    [spec:hfst:req:embed.tools-in-library]
    [spec:hfst:req:embed.transducer]
    [spec:hfst:req:javascript.read-only-module]
)
establishes ()
---

## Rationale

Each hfst tool already is a library call with a process around it, after the
CLI was made thin. The packages bind the library through hfst-embed, so giving
hfst-embed the compilers, operations and writers gives both packages the
tools, and checking the packages against the tools on the inputs the
repository's tests use shows they agree. A few tools still decide part of
their output themselves; that moves into the library first, so a package and
a tool cannot drift.

Python pays nothing for code it does not call, so its package has one module.
The npm package does pay, in bytes every page downloads, so compiling is a
second entry point with a `.wasm` of its own, and the main entry point stays
the read-only module its build check keeps it.

Operations change the transducer they are called on, as the library's do, so
building a network costs what the tools cost and no copy per step. Python's
mutating methods return `None` and JavaScript's return the object, and the
packages follow each. A Python transducer is locked for the length of an
operation, with the GIL released, so threads that share one wait rather than
see it half changed.
