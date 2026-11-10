# Python

Divvun HFST runs in Python as a native extension module, published to PyPI as
`divvun-hfst`. It is a pyo3 binding over the read-only surface `@divvun/hfst`
binds, so a lookup is the library's own code on the host and not
WebAssembly. These rules fix what the package contains and how its API
behaves.

The surface follows `@divvun/hfst`: the same readers, the same lookups, the
same tokeniser and the same DHFST models, giving the same results. It differs
where Python has a convention of its own, for names, option objects,
exceptions and logging, and where a native host can do more: it loads files
by path, and it releases the GIL so threads can share a loaded object. Like
the npm package, it does not compile, write or transform transducers.

## Package

> [spec:hfst:req:python.module]
> The package MUST be published to PyPI as `divvun-hfst`, at a version of its
> own that `crates/hfst-python` carries, from a `python/v<version>` tag naming
> that version, and MUST be imported as `divvun_hfst`, since the name `hfst`
> belongs to the HFST project's own Python package. `divvun_hfst.__version__`
> MUST be that version. One wheel per platform MUST serve CPython 3.11 and
> every later release, through the stable ABI. The package MUST declare
> LGPL-3.0-or-later and carry the text of the LGPL and of the GPL it builds on.

> [spec:hfst:req:python.typed]
> The package MUST ship type stubs for its whole public API and the `py.typed`
> marker, and no public signature may use `Any`. Results MUST be instances of
> declared classes with typed attributes, and none of them is a handle that
> needs another call to read. Names follow Python conventions: snake_case
> functions, methods, attributes and options, and PascalCase classes.

## Loading

> [spec:hfst:req:python.load]
> `read_transducers`, `read_transducer`, `read_optimised_transducer`,
> `read_tokeniser` and `read_dhfst` MUST take the bytes of a file as `bytes`
> or `bytearray` and read them as `@divvun/hfst`'s reader of the same name
> reads them: the same formats, the same objects in the same order, and the
> same refusals with the same codes. For each there MUST be a `load_` function
> that takes a path, as a `str` or an `os.PathLike`, reads the file in the
> library rather than in Python, and loads its bytes as the `read_` function
> does.

## Lookup

> [spec:hfst:req:python.lookup]
> `Transducer.lookup(input, *, limit, time_limit)` MUST return what
> `@divvun/hfst`'s `Transducer.lookup` returns for the same transducer, input
> and limits, as a list of `LookupResult`, each with its `output`, its
> `symbols` as a tuple and its `weight`, in the same order. The time limit is
> in seconds, as Python counts time; 0 sets none, and a limit that runs out
> returns the results found so far. `OptimisedTransducer.lookup(input, *,
> limit)` MUST return what `@divvun/hfst`'s optimised lookup returns, in the
> same shape. Each limit is keyword-only and may be left out for none.

## Tokenising

> [spec:hfst:req:python.tokenise]
> `Tokeniser.tokenise(text, options)` MUST return the tokens `@divvun/hfst`'s
> `tokenise` returns for the same tokeniser, text and options, as a list of
> `Token`, each with its `surface`, `analysed` and `readings`. Its `start` and
> `end` MUST count code points, as `str` indices do, so `text[start:end]` is
> the surface wherever `@divvun/hfst`'s `text.slice(start, end)` is.
> `Tokeniser.format(text, format, options)` MUST return what `@divvun/hfst`'s
> `format` returns for the same format name and options, byte for byte what
> `hfst tokenise` prints. The options MUST be `TokeniseOptions` and
> `FormatOptions`, frozen dataclasses with a keyword-only field for each of
> the npm package's options, named in snake_case and with time limits in
> seconds. Each MUST check its fields when it is made.

## DHFST

> [spec:hfst:req:python.dhfst]
> `read_dhfst` MUST return a `DhfstAcceptor` or a `DhfstErrorModel`, as the
> file's type says. An acceptor's `lookup(word)` MUST return what
> `@divvun/hfst`'s acceptor lookup returns, the least weight as a `float` or
> `None`. Each model MUST expose the metadata the npm package exposes, and an
> acceptor its `info` as an `AcceptorInfo`.

## Errors

> [spec:hfst:req:python.errors]
> Every failure the library reports MUST be raised as `divvun_hfst.HfstError`,
> a subclass of `Exception`, carrying the library's message and, as `code`,
> the code `@divvun/hfst` gives the same failure. Bytes a reader does not load,
> whatever the reason, MUST raise `divvun_hfst.FormatError`, a subclass of
> `HfstError`. An argument of the wrong type or value MUST raise the
> `TypeError`, `ValueError` or `OverflowError` Python raises for it, and a file
> that cannot be read MUST raise the `OSError` `open()` raises for it. A failed
> call MUST NOT end the process, and an object MUST stay usable after a call
> on it fails.

## Threads

> [spec:hfst:req:python.threads]
> Loading, both lookups, tokenising and acceptor lookup MUST release the GIL
> while the library works. A loaded object is immutable, so threads MAY call
> one at the same time: each call is a run of its own over the shared tables,
> and gives what it gives when no other call runs.

## Logging

> [spec:hfst:req:python.logging]
> The library's `tracing` events, from a load or a call, MUST reach Python's
> `logging` module when the call returns, in the order they happened. Each
> goes to the logger named after its target with `::` read as `.`: under
> `hfst` for the hfst library, such as `hfst.pmatch_core`, and under
> `divvun_fst` for divvunspell's engine behind the optimised lookup. It goes
> at the level named after its own, and a `trace` event at level 5. An event
> below the effective level of its root logger, `hfst` or `divvun_fst`, when
> the call starts MUST NOT be kept.

## Parity

> [spec:hfst:req:python.parity]
> For the same bytes, input and options, the package MUST give the results
> the native library gives: the same lookups, the same optimised lookups, the
> same tokens, the same tokenise output, the same acceptor weights and the
> same DHFST metadata. A test suite MUST check this through the built package
> on the fixtures and native expectations the npm package's parity suite
> uses.
