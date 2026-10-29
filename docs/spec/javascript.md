# JavaScript and WebAssembly

Divvun HFST runs in JavaScript as WebAssembly, published to npm as
`@divvun/hfst`, a read-only package that replaces `hfst-js`. These rules fix
what the package contains, what the hfst library must do so it can run without
a host operating system, and how the API behaves.

## Package

> [spec:hfst:def:javascript.packages+1]
> Divvun HFST ships to npm as one package, `@divvun/hfst`, built from this
> repository. It loads optimized-lookup transducers, pmatch containers and
> DHFST files, and runs lookup, tokenisation and DHFST acceptor lookup. It does
> not compile, write or transform transducers. The package is one ES module
> entry point with one `.wasm` file beside it, and loads unchanged in browsers,
> Web Workers, Deno, Node 20 or later, and Bun. It declares LGPL-3.0-or-later
> and names Divvun HFST, copyright UiT.

> [spec:hfst:req:javascript.init+1]
> Importing the package MUST NOT fetch or compile its `.wasm` file. The caller
> starts the package with one async init call that accepts a URL, a `Response`,
> or the module's bytes. Calling init again MUST return the same instance. A
> call to anything else before init has resolved MUST throw an `HfstError` with
> code `not-initialised`.

> [spec:hfst:req:javascript.read-only-module]
> The package's WebAssembly module MUST NOT link the compilers (regex, lexc,
> twolc, pmatch, xfst), the foma backend, the tropical OpenFst backend or the
> transducer algebra. A build check MUST fail when code from any of them is
> reachable from the module's exports.

## Library groundwork

> [spec:hfst:req:javascript.portable-build+1]
> The hfst library and the binding crate MUST build for
> `wasm32-unknown-unknown` from their manifests alone. The build MUST NOT need
> `RUSTFLAGS`, a cargo config file, or a `getrandom` backend setting. A
> dependency whose default features need OS entropy, threads or a filesystem is
> configured, or fixed in that dependency, so that it does not.

> [spec:hfst:req:javascript.no-host-traps]
> No operation the JavaScript API exposes may trap because it reached a host
> facility that `wasm32-unknown-unknown` lacks. Lookup and tokenise time limits
> MUST measure time with a clock that works there, and MUST still stop the
> search when the limit passes. Random seeds MUST come from a source that works
> there. Code that splits work across threads MUST run it on the calling thread
> when no threads are available, with the same result.

The next three rules make the library embeddable without a terminal or a
filesystem. The package does not compile, but the library keeps them for
every other embedder.

> [spec:hfst:req:javascript.diagnostics-as-data]
> Every compile entry point in the hfst library (regex, lexc, twolc, pmatch,
> xfst) MUST hand its diagnostics to the caller as values. Each diagnostic
> carries its severity, message, source name, the byte span of its primary
> label, any further labels with their spans, its notes and help, and the
> rendered report text. The library MUST NOT write diagnostics to stderr
> itself. The command-line tools print the rendered text, so their output does
> not change.

> [spec:hfst:req:javascript.named-file-resolver]
> Every compile operation that reads a file by name MUST read it through a
> resolver the caller supplies. This covers the xfst commands that load or
> source files, pmatch includes and binary references, and xre file
> references. The default resolver reads the filesystem, so command-line
> behaviour does not change. A name the resolver cannot supply MUST fail the
> operation with an error that names the file.

> [spec:hfst:req:javascript.xfst-output-sink]
> The xfst compiler MUST write command output (printed networks, words, apply
> results, help text, prompts) to a writer the caller supplies, and its
> diagnostics through the diagnostics channel. Standard output is the
> command-line tool's choice of writer, not the library's.

## API conventions

> [spec:hfst:req:javascript.typed-surface+1]
> The package MUST ship TypeScript declarations for its whole public API. No
> exported signature may use `any`. Results MUST be plain objects and arrays
> with declared shapes, never opaque handles that need another call to read.
> Names follow JavaScript conventions (camelCase functions and methods,
> PascalCase classes), not the Rust names.

> [spec:hfst:req:javascript.errors+1]
> Every error the library returns MUST reach JavaScript as a thrown
> `HfstError`, a subclass of `Error` carrying a stable string `code` and the
> library's message. A WebAssembly trap MUST be rethrown as an `HfstError`
> with code `poisoned`, and every later call into that module instance MUST
> throw the same code without running, because the module's memory can no
> longer be trusted.

> [spec:hfst:req:javascript.handle-lifetime+1]
> An object that owns WebAssembly memory (a transducer, a tokeniser or a DHFST
> model) MUST release it when its `free()` or `[Symbol.dispose]()` method is
> called, so `using` declarations work. An object the caller drops without
> freeing SHOULD be released through a `FinalizationRegistry`. Any call on a
> freed object MUST throw an `HfstError` with code `freed`. Freeing twice MUST
> do nothing.

> [spec:hfst:req:javascript.logging]
> The library's `tracing` events MUST reach a logger the caller can replace.
> The default logger writes warnings and errors to the console and drops lower
> levels. The caller can set the level and the function that receives each
> event's level, target and message.

## Loading and lookup

> [spec:hfst:req:javascript.load-bytes+2]
> Transducers load from bytes, never from paths. The package MUST read
> weighted and unweighted optimized-lookup transducers and pmatch containers,
> with or without an HFST header, and DHFST files of both types. A stream
> holding several transducers MUST load as an array in stream order. Bytes in
> any other format MUST throw an `HfstError` with code `unsupported-format`
> that names the format.

> [spec:hfst:req:javascript.lookup]
> `lookup(input, options)` on a transducer MUST return every result the
> library's flag-aware lookup returns for the same input and limits, as an
> array of `{ output, symbols, weight }` ordered by weight and then by output.
> `output` is the concatenated output symbols and `symbols` is the array of
> them. The options MUST accept a result limit and a time limit in
> milliseconds. An expired time limit MUST return the results found so far
> rather than throw.

> [spec:hfst:req:javascript.tokenise]
> A tokeniser loads from pmatch container bytes. `tokenise(text, options)` MUST
> return the tokens the library's tokeniser finds, in text order. Each token
> carries its surface form, its UTF-16 start and end offsets in the input
> string, whether it was analysed, and its readings with their weights. The
> options MUST cover every library tokeniser setting that changes which tokens
> or readings come back. The same tokeniser MUST also return the command-line
> output formats (CG, Giella CG, Xerox, plain) as one string, byte for byte
> what `hfst tokenise` prints for the same input and settings.

> [spec:hfst:req:javascript.dhfst+1]
> `readDhfst(bytes)` MUST load a DHFST file of either type, run the validation
> divvunspell runs on load, and say which type it holds. An acceptor's
> `lookup(word)` MUST answer as divvunspell's speller does: it splits `word`
> into extended grapheme clusters, maps each to the acceptor symbol with the
> same text or else to the acceptor's unknown symbol, and returns the least
> weight of a path that consumes them, final weight included, walking free
> arcs and obeying flag diacritics as divvunspell does, or `null` when no path
> does. For an acceptor written from an optimized-lookup transducer whose
> symbols are each one grapheme cluster, that weight MUST equal the least
> weight the transducer's own lookup returns for the same word. An error model
> MUST expose its metadata (version, state count, symbols and meta text). The
> package offers no suggestion search: that is divvunspell's.

## Parity

> [spec:hfst:req:javascript.native-parity+2]
> For the same input bytes and options, the package MUST give the same results
> as the native hfst library: the same lookup results and the same
> tokenisation output. A test suite MUST check this on shared fixtures under
> Deno, Node and Bun, and in at least one browser engine.

> [spec:hfst:req:javascript.hfst-js-parity+1]
> For every optimized-lookup transducer `hfst-js` loads, `lookup` MUST return
> the same analyses with the same weights as `hfst-js` returns for the same
> input, except where `hfst-js` disagrees with the reference C++ `hfst-lookup`;
> there `lookup` MUST equal the reference. Results compare as sets, since
> `hfst-js` reports analyses in search order and may repeat one, and with
> `hfst-js`'s weights rounded to 32-bit floats, since it sums them in 64 bits
> where HFST sums in 32. Every kind of disagreement MUST be listed in the
> package's notes for users moving from `hfst-js`. Load time and uncached
> lookup time MUST be measured against `hfst-js` on the same files and
> machine, and the figures published with the package.

## Limits

A wasm32 module addresses at most 4 GiB, far more than any shipped analyser,
tokeniser or speller model needs. Every call is synchronous, so tokenising a
large text belongs in a Web Worker or worker thread, where it cannot block a
page.
