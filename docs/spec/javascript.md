# JavaScript and WebAssembly

Divvun HFST runs in JavaScript as WebAssembly, published to npm as two
packages. These rules fix what each package contains, what the hfst library
must do so it can run without a host operating system, and how the JavaScript
API behaves.

## Packages

> [spec:hfst:def:javascript.packages]
> Divvun HFST ships to npm as two packages built from this repository.
> `@divvun/hfst-runtime` loads optimized-lookup transducers and pmatch
> containers, and runs lookup and tokenisation. `@divvun/hfst` carries
> everything the runtime carries, plus the compilers, the xfst session, the
> transducer algebra, and every format the hfst library reads and writes. Each
> package is one ES module entry point with one `.wasm` file beside it, and
> loads unchanged in browsers, Deno, Node 20 or later, and Bun. Both declare
> LGPL-3.0-or-later and name Divvun HFST, copyright UiT.

> [spec:hfst:req:javascript.init]
> Importing a package MUST NOT fetch or compile its `.wasm` file. The caller
> starts the package with one async init call that accepts a URL, a `Response`,
> or the module's bytes. Calling init again MUST return the same instance. A
> call to anything else before init has resolved MUST throw an `HfstError` with
> code `not-initialised`.

## Library groundwork

> [spec:hfst:req:javascript.portable-build]
> The hfst library and both binding crates MUST build for
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

> [spec:hfst:req:javascript.typed-surface]
> Each package MUST ship TypeScript declarations for its whole public API. No
> exported signature may use `any`. Results MUST be plain objects and arrays
> with declared shapes, never opaque handles that need another call to read.
> Names follow JavaScript conventions (camelCase functions and methods,
> PascalCase classes), not the Rust names.

> [spec:hfst:req:javascript.errors]
> Every error the library returns MUST reach JavaScript as a thrown
> `HfstError`, a subclass of `Error` carrying a stable string `code` and the
> library's message. A failed compile MUST throw an `HfstCompileError`, a
> subclass of `HfstError` that also carries the diagnostics. A WebAssembly trap
> MUST be rethrown as an `HfstError` with code `poisoned`, and every later call
> into that module instance MUST throw the same code without running, because
> the module's memory can no longer be trusted.

> [spec:hfst:req:javascript.handle-lifetime]
> An object that owns WebAssembly memory (a transducer, tokeniser, compiler or
> xfst session) MUST release it when its `free()` or `[Symbol.dispose]()`
> method is called, so `using` declarations work. An object the caller drops
> without freeing SHOULD be released through a `FinalizationRegistry`. Any call
> on a freed object MUST throw an `HfstError` with code `freed`. Freeing twice
> MUST do nothing.

> [spec:hfst:req:javascript.logging]
> The library's `tracing` events MUST reach a logger the caller can replace.
> The default logger writes warnings and errors to the console and drops lower
> levels. The caller can set the level and the function that receives each
> event's level, target and message.

## Runtime

> [spec:hfst:req:javascript.load-bytes+1]
> Transducers load from bytes, never from paths. The runtime package MUST read
> weighted and unweighted optimized-lookup transducers and pmatch containers,
> with or without an HFST header. The full package MUST also read and write
> tropical OpenFst and foma transducers and AT&T text, and convert between all
> of these. A stream holding several transducers MUST load as an array in
> stream order. Bytes in a format the package does not carry MUST throw an
> `HfstError` with code `unsupported-format` that names the format and, in the
> runtime package, says that `@divvun/hfst` reads it. THFST, a directory, and
> DHFST, a speller format, are not transducer streams and are outside this API.

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

## Runtime and full package

> [spec:hfst:req:javascript.runtime-excludes-compilers]
> The runtime package's WebAssembly module MUST NOT link the compilers (regex,
> lexc, twolc, pmatch, xfst), the foma backend, or the transducer algebra. A
> build check MUST fail when code from any of them is reachable from the
> runtime module's exports.

> [spec:hfst:req:javascript.shared-surface]
> Every export of `@divvun/hfst-runtime` MUST exist in `@divvun/hfst` with the
> same name, type and behaviour, so code written against the runtime runs
> unchanged on the full package. The runtime's test suite MUST run against both
> packages.

## Compiling

> [spec:hfst:req:javascript.compile]
> The full package MUST compile regular expressions, lexc, twolc and pmatch
> sources given as strings, with the options the library's compilers take.
> Files a source names MUST come from a map of names to bytes or strings that
> the caller passes in. A successful compile MUST return its transducers
> together with any warnings as diagnostics. A failed compile MUST throw an
> `HfstCompileError` holding every diagnostic the compiler produced, and MUST
> NOT return an empty or partial transducer as though it had succeeded.

> [spec:hfst:req:javascript.xfst-session]
> An xfst session MUST run single commands and whole scripts, and return for
> each run the text the commands printed and their diagnostics. Its stack and
> defined networks MUST be readable as transducer objects and writable from
> them. Files the commands read MUST come from the session's file map, and
> files they write MUST be readable from it afterwards. A failing command MUST
> stop a script the way the command-line tool's quit-on-fail setting does, and
> the error MUST say which command failed.

> [spec:hfst:req:javascript.algebra]
> Transducer operations in the full package (composition, intersection,
> union, concatenation, subtraction, repetition, inversion, projection,
> determinisation, minimisation, weight pushing, and the other operations
> `HfstTransducer` offers) MUST return a new transducer and MUST NOT change
> their receiver or arguments. Combining transducers of different
> implementation types MUST either convert them as the library does or throw
> an `HfstError` with code `type-mismatch`. It MUST NOT silently produce an
> empty result.

## Parity

> [spec:hfst:req:javascript.native-parity]
> For the same input bytes, source text and options, the JavaScript API MUST
> give the same results as the native hfst library: the same lookup results,
> the same tokenisation output, and compiled transducers whose written bytes
> are identical. A test suite MUST check this on shared fixtures under Deno,
> Node and Bun, and in at least one browser engine.

## Limits

A wasm32 module addresses at most 4 GiB. The largest Giella grammars need more
than that to compile natively (the lang-sma pmatch tokeniser peaks near
4.7 GB), so those builds stay on the command line. Lookup and tokenisation
with prebuilt artifacts fit comfortably. Every call is synchronous, so a long
compile belongs in a Web Worker or worker thread where it cannot block a page.
