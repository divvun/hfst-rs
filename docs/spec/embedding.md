# The library behind the packages

`crates/hfst-embed` is what `divvun-hfst` and `@divvun/hfst` bind, so the two
agree by construction. Its read-only surface reads compiled transducers and
runs them. With its `compile` feature it does everything else the `hfst` tools
do to a transducer: compile it, transform it, inspect it and write it. A
binding that leaves the feature off links none of that.

Each tool is a process around a library function. These rules keep it that
way, so that a package calling the function gets what the tool writes.

## Which tools

> [spec:hfst:def:embed.tools]
> The `hfst` tools fall into five groups.
>
> - Compilers: `regexp2fst`, `lexc`, `twolc`, `xfst`, `pmatch2fst`,
>   `strings2fst` and `txt2fst`.
> - Operations, which make a transducer from one or two: `affix-guessify`,
>   `compose`, `compose-intersect`, `concatenate`, `conjunct`, `determinize`,
>   `disjunct`, `edit-metadata`, `eliminate-flags`, `expand-equivalences`,
>   `guessify`, `insert-freely`, `invert`, `kill-paths`, `minimize`, `name`,
>   `preprocess-for-optimized-lookup-format`, `priority-disjunct`, `project`,
>   `prune-alphabet`, `push-labels`, `push-weights`, `realign`,
>   `remove-epsilons`, `repeat`, `reverse`, `reweight`, `shuffle`,
>   `substitute` and `subtract`.
> - Queries, which read a transducer and report: `check-alpha`, `compare`,
>   `dump-alphabets`, `format`, `fst2strings`, `grep`, `guess`, `lookup`,
>   `flookup`, `optimized-lookup`, `pair-test`, `pmatch`, `summarize` and
>   `tokenize`.
> - Writers: `fst2fst`, `fst2txt` and `bhfst`.
> - Tools that only move a stream or talk to a terminal: `binary-tool`, `head`,
>   `info`, `multiply`, `split`, `strip-header`, `tail`, `traverse`, and
>   `xfst`'s interactive shell. A program does these with its own lists and
>   bytes, so the packages leave them out.
>
> The aliases (`union`, `minus`, `intersect` and the others) name the same
> tools.

## The library holds each tool

> [spec:hfst:req:embed.tools-in-library]
> Whatever a tool does that decides what it writes MUST be done by a function
> of the hfst library, which the tool calls. That covers compiling strings as
> `strings2fst` does, the multiple sources `lexc` reads and the expressions
> `regexp2fst` splits, converting a transducer to the type of the stream it is
> written to, reading a format name, packing a BHFST, and each query's report.
> A tool keeps only what a process has: its arguments, its files, its standard
> streams and its exit status.

> [spec:hfst:req:embed.no-panic]
> No compile, operation, query, conversion or write the library offers may
> panic, whatever its input. A failure MUST come back as an error.

## The compile feature

> [spec:hfst:req:embed.transducer]
> With the `compile` feature, hfst-embed MUST offer one transducer type that
> holds a transducer of any type the library reads: tropical OpenFst, foma,
> and optimized lookup, weighted and unweighted. It MUST read a stream of them
> from bytes, in any of those types, and write one to bytes in any of them,
> converting as `hfst fst2fst` converts. An operation MUST change the
> transducer it is called on, as the library's operations do, without copying
> it first. The type MUST be `Send` and `Sync`, so a binding can run an
> operation with its host's lock released.

> [spec:hfst:req:embed.compile]
> With the `compile` feature, hfst-embed MUST offer each compiler as a
> function from source text and typed options to what the tool writes:
> regular expressions as `regexp2fst` compiles them, lexc as `lexc`, the rule
> transducers of a twolc grammar as `twolc`, an xfst script as `xfst` runs it
> (its output, its stack, its defined networks and the files it wrote), a
> pmatch archive as `pmatch2fst`, strings as `strings2fst`, and AT&T and
> Prolog text as `txt2fst`. The options MUST cover every option of the tool
> that changes its result, the implementation type `--format` chooses among
> them. For the same input and options, the result written to bytes MUST be
> byte for byte what the tool writes.

> [spec:hfst:req:embed.compile-files]
> Every file a compile reads, and every file an xfst script writes, MUST go
> through a file resolver the caller supplies: the filesystem by default, or
> named files held in memory. An xfst script MUST write nothing to the
> process's standard output, whatever its commands.

> [spec:hfst:req:embed.compile-diagnostics]
> A compile MUST hand back every diagnostic it made, as data: with its result
> when it succeeds, and with its error when it fails. A compile that fails
> MUST NOT also return a transducer.

> [spec:hfst:req:embed.operations]
> With the `compile` feature, the transducer type MUST offer each operation
> and each query `embed.tools` lists, with typed options covering every option
> of the tool that changes its result. For the same transducers and options,
> an operation MUST give, written to bytes, what the tool writes, and a query
> MUST give what the tool reports, as data rather than as the tool's text.
> Where a tool converts two transducers of different types before it combines
> them, the operation MUST convert them the same way.

> [spec:hfst:req:embed.writers]
> With the `compile` feature, hfst-embed MUST write a transducer as each
> writer `embed.tools` lists writes it: in each type and DHFST model
> `fst2fst` writes, as the AT&T, Prolog, dot and PC-KIMMO text `fst2txt`
> writes, and packed with an error model and metadata as `bhfst` packs a
> speller, each byte for byte. Every writer MUST return bytes, apart from
> THFST, which is a directory and is written through the file resolver.
