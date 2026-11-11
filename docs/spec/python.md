# Python

Divvun HFST runs in Python as a native extension module, published to PyPI as
`divvun-hfst`. It is a pyo3 binding over `crates/hfst-embed` with its
`compile` feature, so a lookup or a compile is the library's own code on the
host and not WebAssembly. These rules fix what the package contains and how
its API behaves.

The surface follows `@divvun/hfst`: its main entry point's readers, lookups,
tokeniser and DHFST models, and its compile entry point's compilers,
operations and writers, giving the same results. It differs where Python has
a convention of its own, for names, option objects, exceptions, warnings and
logging, and where a native host can do more: it reads and writes files by
path, and it releases the GIL so threads can share a loaded object. A native
module pays nothing for code it does not run, so it is one module.

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

> [spec:hfst:req:python.load+1]
> `read_transducers`, `read_transducer`, `read_optimised_transducer`,
> `read_tokeniser` and `read_dhfst` MUST take the bytes of a file as `bytes`
> or `bytearray`. `read_transducers` and `read_transducer` MUST read them as
> `@divvun/hfst/compile` reads a stream: every type hfst-embed's transducer
> type holds, in stream order. The other three MUST read them as
> `@divvun/hfst`'s reader of the same name reads them: the same formats, the
> same objects in the same order, and the same refusals with the same codes.
> For each there MUST be a `load_` function that takes a path, as a `str` or
> an `os.PathLike`, reads the file in the library rather than in Python, and
> loads its bytes as the `read_` function does; `load_transducers` MUST also
> read a THFST directory.

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

## Compiling, transforming and writing

> [spec:hfst:req:python.transducer]
> `Transducer` MUST hold a transducer of any type through hfst-embed's
> transducer type and expose its `type`, `name` and `properties`. For a type
> `@divvun/hfst` does not read, `lookup` MUST return what `hfst lookup` finds
> for it, in the shape and order `python.lookup` gives. `copy()` MUST return
> a transducer of its own with the same contents.

> [spec:hfst:req:python.compile]
> The package MUST offer each compiler of `embed.compile` as a function:
> `regex`, `compile_lexc`, `compile_twolc`, `compile_xfst`, `compile_pmatch`,
> `compile_strings`, `parse_att` and `parse_prolog`. Each takes its source as a
> `str`, or for `compile_lexc` a sequence of them, and its options as a frozen
> dataclass with a keyword-only field for each option, named after the tool's
> long option in snake_case, which checks its fields when it is made. `regex`,
> `compile_lexc`, `compile_strings` and the two parsers return a `Transducer`;
> `compile_twolc` a list of the rule transducers; `compile_pmatch` the
> archive's `bytes`, which `read_tokeniser` reads; and `compile_xfst` an
> `XfstResult` holding the script's output, its stack, its defined networks
> and the files it wrote. A keyword-only `files`, a mapping from names to
> `str` or `bytes`, MUST make every file the compile reads come from that
> mapping and every file it writes land in its result; without it, the compile
> reads and writes the filesystem. Compiling MUST release the GIL while the
> library works.

> [spec:hfst:req:python.compile-errors]
> A compile that fails MUST raise `divvun_hfst.CompileError`, a subclass of
> `HfstError`, whose `diagnostics` hold every diagnostic it made as
> `Diagnostic` objects: severity, message, source name, the span of its
> primary label in code points, its other labels, its notes and help, and its
> rendered text without colour. Each warning of a compile that succeeds MUST
> be issued through `warnings.warn` as a `divvun_hfst.CompileWarning` carrying
> its `Diagnostic`.

> [spec:hfst:req:python.operations]
> `Transducer` MUST offer each operation and query of `embed.operations` as a
> method named after the tool in snake_case, such as `compose`,
> `determinize`, `push_weights` and `extract_paths` for `fst2strings`, with
> the tool's options as keyword-only arguments. An operation MUST change the
> transducer it is called on and return `None`, as Python's mutating methods
> do; a query MUST return an instance of a declared class. Each MUST release
> the GIL while the library works.

> [spec:hfst:req:python.writers]
> `Transducer.to_bytes(type)` MUST write the transducer, in `type` or in its
> own type, as `embed.writers` says. `to_att`, `to_prolog`, `to_dot` and
> `to_pckimmo` MUST return the text `fst2txt` writes as a `str`;
> `to_dhfst_acceptor` and `to_dhfst_error_model` the DHFST bytes `fst2fst`
> writes; `write_thfst(path)` MUST write the THFST directory; and
> `divvun_hfst.pack_bhfst(acceptor, error_model, metadata)` MUST return the
> bytes `bhfst` writes. Writing MUST release the GIL while the library works.

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

> [spec:hfst:req:python.threads+1]
> Loading, both lookups, tokenising and acceptor lookup MUST release the GIL
> while the library works. An `OptimisedTransducer`, a `Tokeniser` and a DHFST
> model never change, so threads MAY call one at the same time: each call is a
> run of its own over the shared tables, and gives what it gives when no other
> call runs. A `Transducer` changes only through its operations. A call on a
> `Transducer` that another call is changing MUST wait until that change is
> done, and an operation that reads and changes the same `Transducer`, such as
> `t.compose(t)`, MUST read it as it was before the change.

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

> [spec:hfst:req:python.compile-parity]
> For every input the repository's tests give a compiler, operation, query or
> writer `embed.tools` lists, the package MUST give what the tool gives, byte
> for byte where the tool writes bytes or text. A test suite MUST check this
> through the built package.
