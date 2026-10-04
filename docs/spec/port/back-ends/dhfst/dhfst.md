# back-ends/dhfst — DHFST error-model writer (Rust, target-only, novel)

DHFST is divvunspell's compact error-model format. A state stores at most
one default arc per kind and may borrow the rest of its row from a fallback
state, so an error model shrinks by one to two orders of magnitude. hfst
writes it so that a Giella build can produce DHFST error models without
divvunspell's `dhfst-tools`.

There is no C++ HFST ancestor. The contract is divvunspell compatibility:
the consumer of record is divvunspell's `src/transducer/dhfst/` reader, and
the reference producer is `dhfst-tools write`. hfst writes plain version 1
files only. It never reads DHFST as a transducer, so DHFST is an output mode
of `hfst-fst2fst`, not an implementation type.

## The format

> [spec:hfst:def:dhfst.header]
> A DHFST file starts with the five bytes `DHFST` and a `u8` format version,
> which is 1. The 24-byte header then holds two reserved zero bytes, `u32`
> flags (bit 0 tropical `f32` weights, bit 1 fallback rows used, bit 2
> default records used, bit 3 a `RULE` section), the `u32` section count,
> the `u32` longest fallback chain, and a reserved zero `u32`. Every field is
> little-endian.

> [spec:hfst:sem:dhfst.header]
> A file is told to be DHFST by its first five bytes. A reader refuses any
> other version, any flag it does not know, a file without the tropical
> flag, and a file with a `RULE` section.

> [spec:hfst:def:dhfst.layout]
> After the header comes a table of 24-byte section records (tag, zero `u32`
> flags, `u64` offset, `u64` length), then the sections, each at a multiple
> of 8 and zero-padded to one. `SYMS` is `u32 n; u32 offsets[n + 1]; u8
> names[]`, symbol 0 being `@_EPSILON_SYMBOL_@`. `CLAS` is `u32 words; u32 n;
> u64 bits[n * words]`. `CPAI` is `u32 n; u32 0; { u16 input_class; u16
> output_class }[n]`. `STAT` is `u32 n; u32 start; { u32 first_entry; u32
> n_entries; u32 fallback; f32 final_weight }[n]`. `ENTR` is `u32 n; u32 0;
> { u16 input; u16 output; u32 target; f32 weight }[n]`. `meta` is UTF-8
> JSON. A tag starting with an upper-case letter is critical, and a reader
> refuses a critical tag it does not know.

> [spec:hfst:sem:dhfst.layout]
> The writer emits the sections in the order `SYMS`, `CLAS` (only if a class
> is used), `CPAI` (only if a class pair is used), `STAT`, `ENTR`, `meta`,
> and stores the source's start state as state 0. A state's run is its
> explicit entries sorted by `(input, output, target, weight)`, a blocker
> having target `0xFFFFFFFF` and weight 0, followed by its default records
> sorted by kind. Classes and class pairs are numbered in the order the
> states first use them. The fallback and defaults flags are set exactly
> when some state uses them.

> [spec:hfst:def:dhfst.regular-symbol]
> A symbol is regular unless it is symbol 0 or its name is longer than one
> character and starts and ends with `@`. The kinds of a pair `x:y` are
> identity (`x = y`, both regular), substitution (`x ≠ y`, both regular),
> deletion (`x` regular, `y` epsilon) and insertion (`x` epsilon, `y`
> regular). Only these pairs can be answered by a default record.

## Reading

> [spec:hfst:def:dhfst.reader]
> `DhfstReader::parse(bytes)` validates a whole file the way divvunspell
> does before it loads one, and answers the arcs of a state on an input.

> [spec:hfst:sem:dhfst.reader]
> Parsing checks every section extent, symbol, class, class pair, state run,
> entry, target and fallback, and that no fallback chain is cyclic or longer
> than the header promises. A file that fails any check is refused. The arcs
> of a state for a pair are its explicit entries for the pair if it has any
> (a blocker means none); otherwise its default record of the pair's kind,
> if the record's class holds the pair (for substitution, the input class
> holds `x` and the output class holds `y`, `y ≠ x`); otherwise whatever
> its fallback state answers, the same way. Finality is never inherited.
> State 0 and the stored start state trade numbers.

## Writing

> [spec:hfst:def:dhfst.source-model]
> `SourceModel::from_olw` turns a weighted optimized-lookup transducer into
> plain states and arcs, state 0 being the start.

> [spec:hfst:sem:dhfst.source-model]
> The model is what divvunspell's suggestion search reads from the same
> tables written as a file: for each state, in the order states are first
> reached, the arcs on each input symbol in symbol order, exactly as its
> optimized-lookup cursor finds them, and the final weight its reader
> reports. Each state's arcs are then sorted and exact duplicates dropped. A
> model with a flag diacritic arc is refused (the search never follows one),
> and so is a symbol table divvunspell would refuse to load: one whose
> flag-shaped name has an operator other than `P N R D C U`.

> [spec:hfst:def:dhfst.write]
> `dhfst_writer::write(model, options)` encodes a source model as DHFST.
> The options are the longest fallback chain allowed (default 4, or no
> bound), the worker thread count, and the source name for `meta`.

> [spec:hfst:sem:dhfst.write+1]
> For the same optimized-lookup input, depth bound and source name, the
> output is the bytes `dhfst-tools write` writes in every section but
> `meta`, and the two `meta` sections differ only in the `writer` value. The
> output does not depend on the thread count.

> [spec:hfst:sem:dhfst.encoding]
> The encoding is divvunspell's writer's. Per state and kind, the default is
> the largest group of pairs whose only arc is the same `(target, weight)`,
> ties going to the smaller `(target, weight)` bits, and only for groups of
> two or more. Fallback candidates are the states that share a MinHash value
> over `(pair, value)` cells under one of 16 hash functions (the next six
> in each bucket), plus the state's eight most frequent targets. States
> choose a fallback greedily, those that save most first, passing over a
> choice that would close a cycle or exceed the depth bound. A state then
> keeps each default only where it costs fewer entries than going without.

> [spec:hfst:sem:dhfst.self-check]
> Nothing is written unless two checks pass. Every `(state, pair)` resolved
> through the encoding must equal the source. Then the serialised bytes are
> parsed by the reader, and every `(state, input)` it answers must equal the
> source's arcs exactly, weights and final weights compared bit for bit.

> [spec:hfst:def:dhfst.meta]
> The `meta` section is one line of JSON with the keys `writer`, `source`,
> `max-fallback-depth-bound` (a number, or `null` for no bound),
> `source-states`, `source-arcs` and `source-duplicate-arcs`, in that order.

> [spec:hfst:sem:dhfst.meta+1]
> `writer` names the program that wrote the file: `Divvun HFST v` and the
> hfst crate version, the number `--version` prints, with no date or commit,
> so the bytes are stable within a version. `source` is what the caller
> names the source; `hfst-fst2fst` gives the input's file name without its
> directory, as `dhfst-tools` does, and the empty string for standard input.
> Only `"`, `\` and control characters are escaped.

## Tools

> [spec:hfst:def:dhfst.fst2fst]
> `hfst-fst2fst -f dhfst [--max-fallback-depth N | --unbounded-fallback]`
> writes the input transducer as a DHFST error model.

> [spec:hfst:sem:dhfst.fst2fst+1]
> The transducer is first converted exactly as `-f olw` converts it (`-Q`
> included), so `hfst-fst2fst -f dhfst -i X` writes what `dhfst-tools write`
> writes from the output of `hfst-fst2fst -f olw -i X`, but for the `meta`
> `writer` value (see `.write`). The input must hold one transducer. The
> output, a file or standard output, is created only once the self-check
> has passed. `-v` prints the writer's report. The depth options are refused
> without `-f dhfst` and together, and `-b` is refused with it.

> [spec:hfst:def:dhfst.bhfst-member]
> In a BHFST archive a DHFST error model is the single member
> `errmodel.default.dhfst`, in place of the `errmodel.default.thfst`
> directory.

> [spec:hfst:sem:dhfst.bhfst-member]
> `hfst-bhfst -e FILE`, and the error model of `-z`, is a DHFST error model
> when FILE starts with `DHFST`. It is validated with the reader and stored,
> unchanged and uncompressed, as `errmodel.default.dhfst`, between the
> acceptor directory and `meta.json`. Metadata converted from index.xml
> gets the error-model id `errmodel.default.dhfst`; a `-m` meta.json stays
> verbatim. A DHFST file given as the acceptor is refused.
