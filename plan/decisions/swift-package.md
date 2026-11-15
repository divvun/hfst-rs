---
id [dec:hfst:swift-package]
epitome "divvun.hfst is a SwiftPM package from pahkat's registry: a UniFFI binding over crates/hfst-embed with two products, HFST for what @divvun/hfst's main entry offers and HFSTCompile for what @divvun/hfst/compile and divvun-hfst offer beyond it, over one Rust static library that holds both, built with fat LTO and prelinked so its only global symbols are its C interface: it links beside any other Rust static library, and the linker still dead-strips it, so an app that only looks words up carries no compiler."
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
        option "An ordinary static library, as cargo builds it without LTO."
        rejected_because "Measured with a C program on the iOS simulator: it links beside another plain Rust static library, since the linker loads no second copy of a member, but beside one built with fat LTO, as divvunspell's release profile builds it, the link fails on two duplicate symbols, `_rust_eh_personality` and `std::panicking::EMPTY_PANIC`, in either order; Xcode's `-dead_strip` hides that, but an app may turn it off. Force-loading fails with 2200 duplicates. Two such libraries also share every crate they build alike, `tracing-core`'s global dispatcher among them."
    }
    {
        option "Prelink the ordinary static library, without LTO, so only its C interface is global."
        rejected_because "Measured: the archive holds C and assembly members without MH_SUBSECTIONS_VIA_SYMBOLS, so the prelinked object loses it and the linker can no longer drop what an app does not call. A release iOS binary that only looks words up grew from 0.61 MB to 29.4 MB, the compilers included. With fat LTO the archive is one Rust object and compiler-rt's helpers; the outline atomics helpers among those lack the flag too, but each is one function, which is all the flag promises, so the build gives it to them."
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
        rejected_because "Measured: a caller importing both modules finds `Transducer` ambiguous. Re-exported, `HFSTCompile`'s takes its place, as `divvun-hfst`'s one module has one `Transducer` of any type. Every other type is `HFST`'s: the compile glue uses the runtime glue's error, lookup results and tokeniser, so `HFSTCompile` extends `HFST`'s `Tokeniser` with `pmatch` rather than declaring a second one."
    }
    {
        option "Make UniFFI's generated API the public one."
        rejected_because "Its names are the glue's, such as `readOptimisedTransducer(bytes:)`; its errors are a generated enum, its options records with no defaults, and its converters public. A thin Swift layer gives initializers, `throws(HFSTError)`, `String.Index` ranges, `URL`s and option structs with the tools' defaults, and imports the generated modules internally."
    }
    {
        option "Set the library's dispatcher for the length of each call, as hfst-python does, rather than as the global default."
        rejected_because "A handler that calls back into the library would panic: `tracing` refuses to set a scoped default while it dispatches an event on the same thread. The prelink keeps the library's `tracing` its own, so its global default is one no other library can set or share."
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
        "The static library is built with the swift-release profile, fat LTO in one codegen unit, and swift/build.sh prelinks each slice into one object and checks that it keeps MH_SUBSECTIONS_VIA_SYMBOLS and has no global symbol but the C interface. Measured beside a probe library built with LTO and one built without, in either order, with and without `-all_load`, it links; LTO also shrank the runtime's archive from 55 MB to 7.3 MB a slice."
        "uniffi-bindgen-swift reads the library before the prelink, which makes UniFFI's metadata symbols local. It writes the compile glue as if both crates' Swift were one module, so swift/build.sh adds `import HFSTGlue` to it."
        "hfst::depth has a third set of limits, for iOS, beside the native and wasm32 ones, and hfst-embed's compile thread is 16 MiB there."
        "The library's tracing dispatcher is the global default, set when the Swift layer first sets its sink."
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

The main risk was linking. Every Rust static library carries the standard
library, and one built with fat LTO, as divvunspell's is, defines some of
its symbols where a plain one does too, so a plain archive links beside
another plain one but not beside an LTO'd one. Built with fat LTO and
prelinked into one object whose only global symbols are its C interface,
the library meets nothing of another's: measured beside an LTO'd and a
plain library, in either order and force-loaded, it links, and since the
object keeps MH_SUBSECTIONS_VIA_SYMBOLS the linker still drops the
compilers from an app that does not call them. The cg3 package, built
alongside this one, takes the same approach, so an app can link both.

The surface keeps `divvun-hfst`'s decisions: transducers change in place,
`copy()` gives an independent one, compilers return one transducer or the
union under their `disjunct` option, `guessify()` returns the generator, and
a transducer is locked while an operation changes it. It takes Swift's
conventions where they differ: initializers for readers, structs for
results and options, `throws(HFSTError)` with the npm codes,
`Range<String.Index>` for spans, `URL`s for files and the unified log.
