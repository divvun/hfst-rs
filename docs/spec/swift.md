# Swift

Divvun HFST runs in Swift as the SwiftPM package `divvun.hfst`, published to
pahkat's Swift registry. It is a UniFFI binding over `crates/hfst-embed`, with
a thin layer of Swift on top where UniFFI's own API is not Swift's. These
rules fix what the package contains and how its API behaves.

The package has two products. `HFST` is what `@divvun/hfst`'s main entry
point offers: it reads optimized-lookup transducers, pmatch tokenisers and
DHFST files, and looks words up, tokenises text and checks words against DHFST
acceptors. `HFSTCompile` is what `@divvun/hfst/compile` and `divvun-hfst`
offer beyond it: the compilers, transducers of every type with every
operation, query and writer, the reports and BHFST packing. Both give the
results the other packages give for the same input. The API follows
`divvun-hfst`'s decisions under Swift's names, and takes Swift's conventions
where they differ: initializers for readers, value types for results, typed
errors, `String.Index` ranges, `URL`s and the unified log.

## Package

> [spec:hfst:def:swift.package]
> `divvun.hfst` is one SwiftPM package with two library products. `HFST`
> offers `@divvun/hfst`'s main entry point. `HFSTCompile` re-exports `HFST`
> and offers what `@divvun/hfst/compile` and `divvun-hfst` offer beyond it.
> Both products sit over one binary target, the XCFramework `HFSTFFI`, which
> holds one static library for each platform: the UniFFI glue of both
> products and the hfst library under it, in one Rust build. Two Swift
> targets, `HFSTGlue` and `HFSTCompileGlue`, hold the Swift UniFFI generates,
> and each product imports its own internally, so none of the generated API
> is public. The package declares LGPL-3.0-or-later and names Divvun HFST,
> copyright UiT.

> [spec:hfst:req:swift.version]
> The package MUST be published at a version of its own, which
> `crates/hfst-swift-runtime` carries, from a `swift/v<version>` tag naming
> that version, and `HFST.version` MUST return it. Its first version is
> 0.1.0. The library and the tools keep the workspace's version, apart from
> it.

> [spec:hfst:req:swift.registry]
> The package MUST be published to pahkat's Swift registry under the scope
> `divvun` as `divvun.hfst`, never through git. Each version is the zip
> `swift package archive-source` makes of a directory holding
> `Package.swift`, the Swift sources of the four Swift targets and the
> XCFramework, which `Package.swift` names with `.binaryTarget(name:path:)`,
> and nothing else the package does not build. Beside it is a
> `package-metadata.json` with the fields pahkat's `swift.package` accepts:
> `author`, `description`, `licenseURL`, `readmeURL` and `repositoryURLs`.

> [spec:hfst:req:swift.platforms]
> The XCFramework MUST hold three libraries: macOS for arm64 and x86_64 in
> one, iOS devices for arm64, and the iOS simulator for arm64. The package
> MUST declare iOS 13 and macOS 11 as its minimum, and each library MUST be
> built for that minimum, except where the Rust target's own minimum is
> higher: the arm64 iOS simulator's is iOS 14, and its library is built for
> that.

> [spec:hfst:req:swift.runtime-product]
> An app that links `HFST` and not `HFSTCompile` MUST NOT carry the
> compilers, the foma backend, the tropical OpenFst backend or the
> transducer algebra. The linker's dead-stripping keeps them out, since no
> code `HFST` reaches calls them. A check MUST build an iOS binary that
> links only `HFST` and one that links both, and report each one's size.

> [spec:hfst:req:swift.linking+1]
> An app MUST be able to link `HFST` and `HFSTCompile` beside other Rust
> static libraries built by the same toolchain, such as `divvun.cg3`'s and
> divvunspell's, whether or not they were built with LTO and whether or not
> the app force-loads them. Each Rust static library carries the standard
> library and the crates it shares with others, and two of them define some
> of the same symbols. So the static library MUST be one object, built with
> fat LTO and prelinked, whose only global symbols are the UniFFI C
> interface, `_uniffi_hfst_swift_*` and `_ffi_hfst_swift_*`. It MUST keep
> MH_SUBSECTIONS_VIA_SYMBOLS, so the app's linker can still drop each
> function the app does not reach. The package MUST name its static library
> `libhfst_swift.a`, its clang module `HFSTFFI` and its Swift modules
> `HFST`, `HFSTCompile`, `HFSTGlue` and `HFSTCompileGlue`, and keep its
> headers and module map in the XCFramework's `Headers/HFSTFFI/`, since
> Xcode copies every static XCFramework's library and headers into one
> directory. A test MUST link both products with a second Rust static
> library, built with LTO, and run on the iOS simulator.

## API conventions

> [spec:hfst:req:swift.names]
> Public names MUST follow the Swift API Design Guidelines: types in
> UpperCamelCase, everything else in lowerCamelCase, argument labels that
> read as phrases, and the American spelling of Swift's own words with the
> package's spelling of its own, `OptimisedTransducer`, `Tokeniser` and
> `tokenise`, which the other packages use. Readers MUST be initializers,
> `init(data:)` for the bytes of a file and `init(contentsOf:)` for a file
> URL, and `Transducer.all(in:)` and `Transducer.all(contentsOf:)` for every
> transducer in a stream. Results MUST be structs or enums of plain values
> that are `Sendable` and `Hashable`, none of them a handle that needs
> another call to read. Options MUST be structs whose properties have the
> default the tool gives each option, or parameters with that default.

> [spec:hfst:req:swift.errors]
> Every failure the library reports MUST be thrown as `HFSTError`, carrying
> the library's message and, as `code`, an `HFSTError.Code` whose raw value
> is the code `@divvun/hfst` gives the same failure: `type-mismatch`,
> `unsupported-format`, `invalid-format`, `io`, `not-automaton`, `cyclic`,
> `invalid-argument`, `internal`, `failed`, `transducer-count` or
> `compile-failed`. A function that fails only with `HFSTError` MUST declare
> `throws(HFSTError)`. An argument value the call cannot use, such as a
> negative limit, MUST throw `invalid-argument` before the library runs. A
> file a `contentsOf:` reader cannot read MUST throw what
> `Data(contentsOf:)` throws for it. A failed call MUST NOT end the process,
> and an object MUST stay usable after a call on it fails.

> [spec:hfst:req:swift.threads]
> Every public class MUST be a `final class` that is `Sendable`. A
> `Transducer` of `HFST`, an `OptimisedTransducer`, a `Tokeniser` and a
> `DhfstAcceptor` never change, so tasks MAY call one at the same time: each
> call is a run of its own over the shared tables. A `Transducer` of
> `HFSTCompile` holds a lock: an operation takes it to change the
> transducer, and every other call waits until no operation holds it. An
> operation that reads a second transducer locks both, in an order fixed by
> their addresses, so two calls on the same two transducers cannot wait for
> each other. An operation whose operand is the transducer it changes, as
> in `t.compose(t)`, MUST read it as it was before the change.

> [spec:hfst:req:swift.logging+1]
> The library's `tracing` events MUST reach the unified log, through
> `os_log` under the subsystem `no.divvun.hfst` with the event's target as
> the category, at the type of the event's level: error for an error, a
> warning as default, info, and debug for debug and trace. When a handler
> is set with `HFSTLogging.setHandler(_:)`, each event MUST also reach it
> as an `HFSTLogEvent` with its level, target and message.
> `HFSTLogging.level` sets the least level that is logged, `.warning` by
> default, and an event below it MUST NOT be formatted. The names carry the
> package's prefix, since swift-log is a module called `Logging`. The
> library's dispatcher MUST be its global default, which hfst-embed carries
> into the thread a compile runs on. The prelink keeps this `tracing` the
> library's own, so no other Rust library in the process can set or share
> that default, and a handler may call into the library, which a
> dispatcher set for each call would refuse.

## Loading and lookup

> [spec:hfst:req:swift.load]
> `Transducer.all(in:)` and `Transducer(data:)` of `HFST`,
> `OptimisedTransducer(data:)`, `Tokeniser(data:)` and `Dhfst(data:)` MUST
> read their `Data` as `@divvun/hfst`'s `readTransducers`,
> `readTransducer`, `readOptimisedTransducer`, `readTokeniser` and
> `readDhfst` read their bytes: the same formats, the same objects in the
> same order, and the same refusals with the same codes. The
> `contentsOf:` form of each MUST read the file at a file URL as
> `Data(contentsOf:)` reads it, and then read its bytes as the `data:`
> form does.

> [spec:hfst:req:swift.lookup]
> `Transducer.lookup(_:limit:timeLimit:)` of `HFST` MUST return what
> `@divvun/hfst`'s `Transducer.lookup` returns for the same transducer,
> input and limits, as `[LookupResult]`, each with its `output`, its
> `symbols` and its `weight`, in the same order. The time limit is a
> `TimeInterval` in seconds; 0 sets none, and a limit that runs out returns
> the results found so far. `OptimisedTransducer.lookup(_:limit:)` MUST
> return what `@divvun/hfst`'s optimised lookup returns, in the same shape.

> [spec:hfst:req:swift.tokenise]
> `Tokeniser.tokenise(_:options:)` MUST return the tokens `@divvun/hfst`'s
> `tokenise` returns for the same tokeniser, text and options, as
> `[Token]`, each with its `surface`, `analysed` and `readings`. Its `range`
> MUST be a `Range<String.Index>` into the text it was given, so
> `text[token.range]` is the surface wherever `@divvun/hfst`'s
> `text.slice(start, end)` is, and its `utf8Range` the same span in UTF-8
> bytes. `Tokeniser.format(_:as:options:)` MUST return what
> `@divvun/hfst`'s `format` returns for the same format and options, byte
> for byte what `hfst tokenise` prints. The options MUST be the structs
> `TokeniseOptions` and `FormatOptions`, with a property for each of the
> npm package's options and time limits in seconds, and the format the enum
> `TokeniseFormat`.

> [spec:hfst:req:swift.dhfst]
> `Dhfst(data:)` MUST return `.acceptor` with a `DhfstAcceptor` or
> `.errorModel` with a `DhfstErrorModel`, as the file's type says. An
> acceptor's `lookup(_:)` MUST return what `@divvun/hfst`'s acceptor lookup
> returns, the least weight as a `Float`, or `nil`. Each model MUST expose
> the metadata the npm package exposes, and an acceptor its `info` as an
> `AcceptorInfo`. An error model keeps none of its file's bytes, so it is a
> struct.

## Compiling, transforming and writing

> [spec:hfst:req:swift.compile-product+1]
> `HFSTCompile` MUST re-export `HFST`, so one `import HFSTCompile` gives
> both. Its `Transducer`, which holds any type the library reads, MUST take
> the place of `HFST`'s under that import, as Swift resolves a name to the
> declaration of the module that re-exports the other; `HFST`'s is still
> there as `HFST.Transducer`, and a transducer crosses between the two as
> the `Data` one writes and the other reads. Every other type is `HFST`'s:
> `HFSTCompile` adds `pmatch` to `HFST`'s `Tokeniser`, and its glue returns
> `HFST`'s errors, diagnostics, lookup results and tokenisers.

> [spec:hfst:req:swift.transducer]
> `HFSTCompile.Transducer` MUST hold a transducer of any type through
> hfst-embed's transducer type and expose its `type`, `name` and
> `properties`. `Transducer.all(in:)` and `Transducer(data:)` MUST read every
> type, as `divvun-hfst`'s `read_transducers` and `read_transducer` do, and
> `Transducer.all(contentsOf:)` MUST also read a THFST directory, as
> `load_transducers` does. `lookup` MUST return what `divvun-hfst`'s
> `Transducer.lookup` returns. `copy()` MUST return a transducer of its own
> with the same contents.

> [spec:hfst:req:swift.compile]
> `HFSTCompile` MUST offer each compiler of `embed.compile` as a function:
> `regex`, `compileLexc`, `compileTwolc`, `compileXfst`, `compilePmatch`,
> `compileStrings`, `parseAtt` and `parseProlog`. Each takes its source as a
> `String`, or for `compileLexc` an array of them, its options as a struct
> with a property for each option, named after the tool's long option in
> lowerCamelCase, and the name of its source. `regex`, `compileStrings`,
> `parseAtt` and `parseProlog` MUST return one transducer, or with their
> `disjunct` option the union of every transducer the source makes; a source
> that makes several without it MUST throw `transducer-count`.
> `compileLexc` returns one transducer, `compileTwolc` the rule
> transducers, `compilePmatch` the archive's `Data`, which
> `Tokeniser(data:)` reads, and `compileXfst` an `XfstResult` holding the
> script's output, its stack, its defined networks and the files it wrote.
> Each MUST return the warnings the compile gave with its result: in a
> `Compiled`, or in the `XfstResult`.

> [spec:hfst:req:swift.compile-files]
> A compiler that reads files MUST take them as a `CompileFiles`:
> `.fileSystem`, the default, which reads and writes the filesystem;
> `.memory`, a dictionary from names to `Data` or to `String`; or
> `.resolver`, a closure that takes a name and returns the file's `Data`,
> or `nil` for a file it does not have. The closure MUST be `@Sendable`,
> since the library calls it from the thread the compile runs on. Without
> the filesystem, the files an xfst script writes MUST be kept in memory,
> where its later commands read them, and MUST come back in its result. A
> name the files do not supply MUST fail the compile and name the file.

> [spec:hfst:req:swift.compile-errors]
> A compile that fails MUST throw an `HFSTError` with code `compile-failed`,
> whose `diagnostics` hold every diagnostic it made as `Diagnostic` values:
> severity, message, source name, the range of its primary label, its other
> labels, its notes and help, and its rendered text without colour. A range
> MUST be a `Range<String.Index>` into the source string the caller gave
> whenever the diagnostic is about that source, and `nil` for a file the
> compile read; each label MUST also carry its span in UTF-8 bytes. A
> compile that succeeds MUST return its warnings, and each error it made
> without failing, in the same shape with its result; what it says at the
> info level goes to the log.

> [spec:hfst:req:swift.operations]
> `HFSTCompile.Transducer` MUST offer each operation and query of
> `embed.operations` as a method named after the tool in lowerCamelCase,
> such as `compose`, `determinize`, `pushWeights` and `extractPaths` for
> `fst2strings`, with the tool's options as parameters with its defaults.
> An operation MUST change the transducer it is called on and return
> nothing, as Swift's `sort()` does, except `guessify`, which returns the
> model form generator the tool writes. A query MUST return a value of a
> declared struct. `HFSTCompile` MUST also offer the reports that read
> several transducers or none as functions: `lookupReport`,
> `flookupReport`, `optimizedLookupReport`, `pairTest` and `grep`.

> [spec:hfst:req:swift.writers]
> `Transducer.data(type:useBackendFormat:quick:)` MUST write the transducer
> in `type`, or in its own type, as `embed.writers` says. `attText`,
> `prologText`, `dotText` and `pckimmoText` MUST return the text `fst2txt`
> writes; `dhfstAcceptorData` and `dhfstErrorModelData` the DHFST bytes
> `fst2fst` writes; `writeThfst(to:quick:)` MUST write the THFST directory
> at a file URL, as `divvun-hfst`'s `write_thfst` does; and `packBhfst`
> MUST return the bytes `bhfst` writes.

## The compile stack on iOS

> [spec:hfst:req:swift.compile-stack]
> On iOS, every compile and operation MUST run on a thread of 16 MiB, and
> `hfst::depth` MUST hold the compilers to limits that keep each construct
> at its limit within half of it: 125 levels of brackets, 3000 levels of
> evaluation, a tree 30,000 deep and a path of 15,000 arcs to minimize.
> Natively hfst-embed reserves 512 MiB for each, which a 64-bit Mac gives
> freely; an iOS process has an address space of a few GiB, every
> operation running at once reserves a stack of its own, and a keyboard
> extension is ended once the memory it has touched passes its limit. The
> per-level costs on the iOS simulator, which are those of iOS devices'
> arm64, MUST be measured as the native limits were, and recorded beside
> the limits.

## Building

> [spec:hfst:req:swift.build]
> One script MUST build the package from the repository: the static
> library for each slice, the Swift UniFFI generates and the module map,
> the XCFramework with `xcodebuild -create-xcframework`, and the release
> directory, archived with `swift package archive-source` beside its
> `package-metadata.json`. A build for the host's slice alone MUST serve
> local development and the tests.

## Parity

> [spec:hfst:req:swift.parity]
> For the same bytes, input and options, the package MUST give the results
> the native library gives: the same lookups, optimised lookups, tokens,
> tokenise output, acceptor weights and DHFST metadata. A Swift test suite
> MUST check this through the built package, with `swift test` on macOS and
> with `xcodebuild test` on the iOS simulator, on the fixtures and native
> expectations the npm package's parity suite uses.

> [spec:hfst:req:swift.compile-parity]
> For every input the repository's tests give a compiler, operation, query
> or writer `embed.tools` lists, the package MUST give what the tool gives,
> byte for byte where the tool writes bytes or text. The Swift test suite
> MUST check this on both platforms, on the cases `divvun-hfst` and
> `@divvun/hfst/compile` check.

## Limits

An `HFST` lookup or tokenise runs on the caller's thread. A Swift task's
thread and a dispatch queue's have 512 KiB of stack, which the parity cases
fit within; a very long input to a deep tokeniser is safer on a thread of
its own.
