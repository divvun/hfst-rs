---
id [dec:hfst:swift-package]
epitome "divvun.hfst is a SwiftPM package from pahkat's registry: a UniFFI binding over crates/hfst-embed with two products, HFST for what @divvun/hfst's main entry offers and HFSTCompile for what @divvun/hfst/compile and divvun-hfst offer beyond it, over one Rust static library that holds both and that the linker dead-strips, so an app that only looks words up carries no compiler and links beside other Rust static libraries."
state @decided
category @existence
scope {
    elements ([arch:hfst:swift-package])
    rules (
        [spec:hfst:def:swift.package]
        [spec:hfst:req:swift.version]
        [spec:hfst:req:swift.registry]
        [spec:hfst:req:swift.platforms]
        [spec:hfst:req:swift.runtime-product]
        [spec:hfst:req:swift.linking]
        [spec:hfst:req:swift.names]
        [spec:hfst:req:swift.errors]
        [spec:hfst:req:swift.threads]
        [spec:hfst:req:swift.logging]
        [spec:hfst:req:swift.load]
        [spec:hfst:req:swift.lookup]
        [spec:hfst:req:swift.tokenise]
        [spec:hfst:req:swift.dhfst]
        [spec:hfst:req:swift.compile-product]
        [spec:hfst:req:swift.transducer]
        [spec:hfst:req:swift.compile]
        [spec:hfst:req:swift.compile-files]
        [spec:hfst:req:swift.compile-errors]
        [spec:hfst:req:swift.operations]
        [spec:hfst:req:swift.writers]
        [spec:hfst:req:swift.compile-stack]
        [spec:hfst:req:swift.build]
        [spec:hfst:req:swift.parity]
        [spec:hfst:req:swift.compile-parity]
    )
}
author "brendan@necessary.nu"
decided_at "2026-11-15T00:00:00Z"
edges {
    requires ([dec:hfst:python-package] [dec:hfst:compile-surface] [dec:hfst:package-versions])
}
alternatives (
    {
        option "Prelink each slice's static library into one object whose only global symbols are UniFFI's C functions, with `ld -r -exported_symbols_list`, so no Rust symbol can meet another library's."
        rejected_because "Measured: it links beside a second Rust library even under `-all_load`, but `ld -r` drops MH_SUBSECTIONS_VIA_SYMBOLS from the object it writes, so the linker can no longer drop what an app does not call. A release iOS binary that only looks words up grew from 0.61 MB to 29.4 MB, the compilers included. rustc's own `lto = true` does not hide the symbols either: a static library built with it still exports its Rust symbols."
    }
    {
        option "Ship dynamic frameworks."
        rejected_because "A dynamic library keeps all the code its exports reach, so keeping the compilers out of an app that only looks words up would take two Rust frameworks, each with its own copy of the library and the standard library, which an app with both products loads twice. Frameworks also add launch time and an embed and signing step in every app and extension. The ordinary static library already links beside other Rust static libraries."
    }
    {
        option "A static library of its own for each product."
        rejected_because "The two would be separate Rust builds of the library with different features, so an app with both products would carry the runtime twice, each copy with its own logging state. One library holding both UniFFI crates, as Mozilla's application-services does, builds the library once and lets dead-stripping decide what an app carries."
    }
    {
        option "Publish through git, as most Swift packages are, with the XCFramework at a release URL."
        rejected_because "pahkat's Swift registry serves a version's source archive with the XCFramework inside it, signed, and SwiftPM resolves `divvun.hfst` by identifier like any registry package. A git package would need the binary hosted and checksummed apart from it, and a tag in the repository for every package version."
    }
    {
        option "Offer the readers as free functions, `readTransducers` and the others, as the other packages do."
        rejected_because "Measured with Swift 6.4: `HFSTCompile` re-exports `HFST` and both need a `readTransducers`, returning different `Transducer` types. Swift resolves a type name to the re-exporting module's own declaration, but two such functions are ambiguous. Initializers and static methods on the types read as Swift's own readers do, `Data(contentsOf:)` among them, and follow the shadowing types."
    }
    {
        option "Let `HFSTCompile` stand apart from `HFST`, so a caller imports both."
        rejected_because "Measured: a caller importing both modules finds `Transducer` and `Tokeniser` ambiguous. Re-exported, `HFSTCompile`'s take their place, as `divvun-hfst`'s one module has one `Transducer` of any type."
    }
    {
        option "Make UniFFI's generated API the public one."
        rejected_because "Its names are the glue's, such as `readOptimisedTransducer(bytes:)`; its errors are a generated enum, its options records with no defaults, and its converters public. A thin Swift layer gives initializers, `throws(HFSTError)`, `String.Index` ranges, `URL`s and option structs with the tools' defaults, and imports the generated modules internally."
    }
    {
        option "Install one global `tracing` subscriber that sends every event to Swift."
        rejected_because "Two Rust static libraries built with the same `tracing-core` share its symbols, so they would share one global dispatcher: the second library could not set its own, and its events would be logged as hfst's. A dispatcher set for the length of each call, and carried into the compile thread as hfst-python's is, keeps each library's events its own."
    }
    {
        option "Run compiles on iOS on the 512 MiB thread they run on natively."
        rejected_because "An iOS process has an address space of a few GiB, and every operation running at once reserves a stack of its own, so tasks compiling at once could exhaust it; and a keyboard extension is ended once the memory it has touched passes its limit, about 60 MB, which a compile near the native limits would pass. The simulator runs as a Mac process and cannot show either. 16 MiB with limits measured for it keeps the worst case at 8 MiB touched, and each limit is still about 25 times what Giella's grammars need."
    }
    {
        option "Async methods, so a compile does not block the caller's task."
        rejected_because "The other packages' calls are synchronous, and so is the library. A caller who wants a compile off its task runs it in one of its own; every call is safe from any thread."
    }
)
consequences {
    accepted (
        "crates/hfst-swift-runtime and crates/hfst-swift-compile are the UniFFI glue of the two products, and crates/hfst-swift is the static library that holds both, `libhfst_swift.a`. The runtime crate carries the package's version."
        "uniffi-bindgen-swift writes one clang module, HFSTFFI, for both crates' headers, and the Swift of each crate, which the targets HFSTGlue and HFSTCompileGlue hold."
        "An app must not force-load the package's static library beside another Rust static library: `-all_load` loads both copies of the standard library and the link fails."
        "hfst::depth has a third set of limits, for iOS, beside the native and wasm32 ones, and hfst-embed's compile thread is 16 MiB there."
        "Each XCFramework slice is a whole Rust static library, about 70 MB before an app's linker strips it."
    )
    deferred (
        "Building the archive in CI on a macOS agent and publishing it to pahkat on a `swift/v<version>` tag."
        "tvOS, watchOS, visionOS, Mac Catalyst and the x86_64 iOS simulator."
        "Running the tests on an iOS device, where the address space and memory limits are the device's."
        "Reading a file by memory map without copying it into the library."
    )
}
codifies (
    [spec:hfst:req:swift.runtime-product]
    [spec:hfst:req:swift.linking]
    [spec:hfst:req:swift.logging]
    [spec:hfst:req:swift.compile-stack]
)
establishes ([arch:hfst:swift-package])
---

## Rationale

The npm and Python packages already fix a surface over hfst-embed and check
it against the library on shared fixtures. A Swift package that binds the
same crate inherits those checks, and UniFFI writes the glue between Rust
and Swift that wasm-bindgen and pyo3 write for the others.

An app ships what it links, so the split between a runtime and a compiler
matters as it does for the npm package's download. A static library lets
the app's linker decide: everything `HFST` reaches stays, and the rest, the
compilers above all, is dropped. One Rust build holds both products, so the
library is built once and the two products cannot disagree about it.

The main risk was linking. Measured with a second Rust static library
sharing the standard library, uniffi and tracing with this one, an ordinary
static library links and runs beside it on macOS and the iOS simulator,
with or without LTO in the other library and with `-ObjC`. Only
force-loading fails, and it fails for any two Rust static libraries. The
alternative that survives force-loading costs the dead-stripping that keeps
the compilers out of an app.

The surface keeps `divvun-hfst`'s decisions: transducers change in place,
`copy()` gives an independent one, compilers return one transducer or the
union under their `disjunct` option, `guessify()` returns the generator, and
a transducer is locked while an operation changes it. It takes Swift's
conventions where they differ: initializers for readers, structs for
results and options, `throws(HFSTError)` with the npm codes,
`Range<String.Index>` for spans, `URL`s for files and the unified log.
