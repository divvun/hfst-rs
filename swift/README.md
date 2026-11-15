# divvun.hfst

Divvun HFST for Swift, on iOS and macOS. It looks words up in HFST
transducers, tokenises text with pmatch tokenisers (`.pmhfst`) and checks
words against DHFST spellers. It also compiles, changes, inspects and writes
transducers as the `hfst` command-line tools do, and gives the results the
tools give. It is the [hfst-rs](https://github.com/divvun/hfst-rs) library,
written in Rust, in one static library that both of the package's products
link. Its surface is that of the npm package `@divvun/hfst` and the Python
package `divvun-hfst`, in Swift's idioms.

| Product | What it does |
|---|---|
| `HFST` | Reads optimized-lookup transducers and looks words up in them; tokenises text; reads DHFST spellers. |
| `HFSTCompile` | Compiles regular expressions, lexc, twolc, xfst, pmatch, strings, AT&T and Prolog text; reads every transducer type; changes, inspects and writes transducers. It re-exports `HFST`. |

An app that links only `HFST` carries no compiler: the linker drops it. In
release, stripped, an iOS framework using all of `HFST` is 1.3 MB, and one
using `HFSTCompile` too is 9.1 MB.

## Installing

The package is published to Divvun's SwiftPM registry, under the scope
`divvun`. Route the scope to it once:

```sh
swift package-registry set --scope divvun https://pahkat-3.giellalt.org/swift/
```

Then depend on it by its identifier:

```swift
.package(id: "divvun.hfst", from: "0.1.0"),
```

```swift
.target(name: "Keyboard", dependencies: [.product(name: "HFST", package: "divvun.hfst")]),
```

It supports iOS 13 and macOS 11 and later, and needs Xcode 16 or later. The
XCFramework holds macOS (arm64 and x86_64), iOS devices (arm64) and the iOS
simulator (arm64, iOS 14 and later). It has no x86_64 simulator slice, so a
build for every simulator, such as `-destination 'generic/platform=iOS
Simulator'`, needs `ARCHS=arm64`.

## Lookup

```swift
import HFST

let analyser = try Transducer(contentsOf: analyserURL)
for result in try analyser.lookup("teach") {
    print(result.output, result.weight)
}
// teach+Noun+Masc+Com+Sg 0.0
// ...
```

`Transducer(data:)` and `Transducer(contentsOf:)` read one transducer;
`Transducer.all(in:)` and `Transducer.all(contentsOf:)` read every transducer
in a stream. `HFST` reads the optimized-lookup types, weighted and unweighted,
with or without an HFST header. `lookup(_:limit:timeLimit:)` returns every
result the hfst library finds, lightest first, then by output. `output` is
the output symbols joined; `symbols` lists them, without flag diacritics and
epsilons. `limit` stops the search after that many paths, and `timeLimit`, in
seconds, returns the results found so far once it runs out.

`OptimisedTransducer` reads the same files for divvunspell's lookup engine.
Its `lookup` returns what divvunspell returns, and is usually several times
faster.

## Tokenising

```swift
let tokeniser = try Tokeniser(contentsOf: tokeniserURL)
for token in try tokeniser.tokenise("The cat sat.") {
    print(token.surface, token.readings.map(\.analysis))
}
print(try tokeniser.format("The cat sat.", as: .cg))
```

`tokenise(_:options:)` returns the tokens, each with its place in the text as
a `Range<String.Index>` and as UTF-8 offsets. `format(_:as:options:)` returns
what `hfst tokenise` prints in each of its formats, byte for byte.

## Spellers

```swift
if case let .acceptor(speller) = try Dhfst(contentsOf: acceptorURL) {
    speller.lookup("cat")    // the word's weight, or nil
}
```

`Dhfst` reads a DHFST acceptor or error model and describes it.

## Compiling and changing transducers

```swift
import HFSTCompile

let words = try regex("{cat} | {dog}").value
let plural = try regex("[..] -> s || _ .#.").value
try words.compose(plural)
try words.minimize()
let bytes = try words.data()
```

The compilers return the transducer and the warnings the compile gave. The
lexc, xfst and pmatch compilers read the files a source names from the file
system, from a dictionary, or from a closure that takes a name and returns
the file's bytes:

```swift
let lexicon = try compileLexc([source], files: .memory(["affixes.lexc": affixes])).value
```

Under `import HFSTCompile`, `Transducer` holds any type the hfst library
reads. Its operations change it in place, as `sort()` does, and return
nothing; `copy()` gives one of its own to change. Each takes its options
named after the tool's long options: `hfst compose --xerox-composition` is
`compose(other, xeroxComposition: true)`. The queries, such as
`extractPaths`, `summarize` and `lookupReport`, return what the tools print
as data. The writers return the bytes or text the tools write, and
`writeThfst(to:)` and `packBhfst` write divvunspell's formats.

On iOS, a compile runs on a thread with a 16 MiB stack, and the compilers
refuse a source nested deeper than they can compile in it.

## Errors

Every error the library raises is an `HFSTError`. Its `code` says why, such
as `invalidFormat` for bytes that are not a transducer, or `compileFailed`
for a source that does not compile, and its `message` is the library's. A
compile that fails carries what went wrong in `diagnostics`, each with its
severity, message, source name, labels, notes, help and the text the tools
print. Each place in the source is a `Range<String.Index>` in the string you
gave, and its UTF-8 offsets.

```swift
do {
    _ = try regex("[a")
} catch let error where error.code == .compileFailed {
    for diagnostic in error.diagnostics {
        print(diagnostic.message, diagnostic.utf8Range ?? 0..<0)
    }
}
```

## Logging

The library's log events go to the unified logging system, under the
subsystem `no.divvun.hfst` and a category for each part of the library. Their
messages are private. Set `HFSTLogging.level` to choose the least level
logged (`.warning` by default, `nil` for none), and give a handler to receive
every event as well:

```swift
HFSTLogging.level = .info
HFSTLogging.setHandler { event in print(event.level, event.target, event.message) }
```

## Threads

Every type is `Sendable`. Tasks may share a transducer: an operation holds
it for its length, and a call on a transducer another task is changing waits
until that change is done.

## Linking beside other Rust libraries

The package's static library keeps every symbol but its C interface to
itself, so an app can link it beside other Rust libraries, such as
`divvun.cg3`'s.
