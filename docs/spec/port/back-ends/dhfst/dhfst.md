# back-ends/dhfst — DHFST error-model and acceptor writers (Rust, target-only, novel)

DHFST is divvunspell's family of speller transducer formats. A DHFST error
model (type 1) stores at most one default arc per kind in a state and may
borrow the rest of a state's row from a fallback state, so an error model
shrinks by one to two orders of magnitude. A DHFST acceptor (type 2) keeps
optimized lookup's one-probe lookup in a table of narrow checks and records,
three to four times smaller than THFST. hfst writes both so that a Giella
build can produce them without divvunspell's `dhfst-tools`.

There is no C++ HFST ancestor. The contract is divvunspell compatibility:
the consumers of record are divvunspell's `src/transducer/dhfst/` readers,
and the reference producers are `dhfst-tools write` and `dhfst-tools
acceptor`. hfst writes plain error models (type 1) and acceptors (type 2),
both version 1. It never reads DHFST as a transducer, so DHFST is an output
mode of `hfst-fst2fst`, not an implementation type.

## The format

> [spec:hfst:def:dhfst.header+2]
> A DHFST file starts with the five bytes `DHFST`, a `u8` type, a `u8`
> format version and a reserved zero byte. Type 1 is an error model and type
> 2 an acceptor; every other type is reserved. The version is 1 for both
> types. An error model's 24-byte header then holds `u32` flags (bit 0
> tropical `f32` weights, bit 1 fallback rows used, bit 2 default records
> used, bit 3 a `RULE` section), the `u32` section count, the `u32` longest
> fallback chain, and a reserved zero `u32`. An acceptor's 24-byte header
> holds `u32` flags (bit 0 tropical `f32` weights), the `u32` section count
> and eight reserved zero bytes. Every field is little-endian.

> [spec:hfst:sem:dhfst.header+2]
> A file is told to be DHFST by its first five bytes. A reader refuses a
> type it does not know, naming the number, a version it does not read, and
> a reserved byte that is not zero. Each reader refuses a file of the other
> type, saying what the file holds instead. The error-model reader then
> refuses any flag it does not know, a file without the tropical flag, and a
> file with a `RULE` section; the acceptor reader refuses any flag but the
> tropical one, a file without it, and reserved header bytes that are not
> zero. The error-model writer writes type 1 and the acceptor writer type 2,
> both version 1.

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

## Acceptors

> [spec:hfst:def:dhfst.acceptor-layout]
> An acceptor has the error model's section table, and the sections `SYMS`
> (as in an error model), `CHCK`, `SLOT`, `LIST`, `FINL`, `FREE`, `WGHT`,
> the optional `DIST` and the ancillary `meta`. Every array is followed by
> at least 8 zero bytes inside its section. A record is an unsigned integer
> of `record_bytes` bytes, its low `target_bits` bits its first field and
> the next `index_bits` its second; `I = 2^index_bits - 1`. `CHCK` is `u32
> n_slots; u32 n_ids; u16 label_max; u8 check_bytes; u8 0[5]`, then the low
> byte of every slot's check and, when `check_bytes` is 2, the high bytes.
> `SLOT` is `u32 n_slots; u8 record_bytes; u8 target_bits; u8 index_bits;
> u8 0` and a record per slot. `LIST` is `u32 n; u8 record_bytes; u8 0[3]`
> and `n` records. `FINL` is `u32 n; u32 0`, then per 64 state numbers `u64
> final; u32 before` (the final bits, and the bits set before them), then
> the `n` final weights in state number order. `FREE` is `u32 n; u32 0; {
> u16 symbol; u16 0; f32 weight }[n]`. `WGHT` is `u32 n; u32 0; f32
> weights[n]`, and `DIST` `u32 n_ids; u32 0; f32 distances[n_ids]`. A state
> is a number `q` below `n_ids`, the start state 0. Slot `q + s`, `s`
> regular (neither epsilon nor a flag diacritic), holds `q`'s arcs on `s`
> when its check is `s`: one arc (target, index into `WGHT`) when the second
> field is below `I`, else the first field is a position `k` in `LIST`,
> `LIST[k]` the count `n >= 2` and `LIST[k + 1 ..= k + n]` the arcs. Slot `q`
> holds `q`'s free arcs when its check is 0 and its record is not zero: their
> position in `LIST` and their number, each listed arc a target and an index
> into `FREE`. Every arc's output is its input.

> [spec:hfst:sem:dhfst.acceptor-layout]
> The writer emits `SYMS`, `CHCK`, `SLOT`, `LIST`, `FINL`, `FREE`, `WGHT`,
> `DIST` (only when some distance is not 0) and `meta`, each at a multiple
> of 8, and the file ends where `meta` does. Checks are one byte when
> `label_max` is at most 255, else two. `WGHT` holds the distinct regular arc
> weights and `FREE` the distinct free `(symbol, weight)` pairs, most
> frequent first, ties by value. The first field is as wide as the greater
> of `n_ids - 1` and `n_list - 1` needs, the second as wide as the greatest
> of the weight count, the free pair count less one, and the most free arcs
> of one state, each at least one bit; records take the bytes the two need.
> `LIST` is filled in state number order, a state's free arcs first and then
> its listed arcs by symbol, arcs in the source's order.

> [spec:hfst:sem:dhfst.acceptor-placement]
> States are numbered first fit in depth-first pre-order from the start
> state, which takes 0: each state with slots takes the least number that no
> other state has and whose slots (`q` when it has free arcs, `q + s` for
> each symbol it has arcs on) are all empty; then each state without slots,
> in the same order, takes the least number left. `n_ids` is one more than
> the greatest number, and `n_slots` the greater of the end of the last
> used slot and `n_ids + label_max`.

> [spec:hfst:def:dhfst.acceptor-reader]
> `AcceptorReader::parse(bytes)` validates a whole acceptor the way
> divvunspell does before it loads one, and answers the lookups the
> suggestion search makes.

> [spec:hfst:sem:dhfst.acceptor-reader]
> Parsing checks every section extent, and that every check is 0 or a
> regular label whose state is a state number, every record is free of
> stray bits with its targets below `n_ids` and its indices in range, every
> list is in range with a count of at least 1 (free arcs) or 2, every `FINL`
> count agrees, no weight or distance is NaN or `-inf`, final weights are
> finite, and `DIST` has one distance per state number. A file that fails
> any check is refused. A state has arcs on a symbol when that symbol's slot
> checks as it; a free symbol never has. Its free arcs are those its own
> slot names. A state is final when its `FINL` bit is set. Its distance is
> its `DIST` entry, 0 without `DIST`.

> [spec:hfst:def:dhfst.acceptor-source]
> `AcceptorSource::new(t)` opens a weighted optimized-lookup transducer as
> divvunspell's suggestion search reads a lexicon from the same tables
> written as a file, and computes its distances.

> [spec:hfst:sem:dhfst.acceptor-source]
> A state's free arcs are the epsilon and flag diacritic arcs its epsilon
> cursor finds; its arcs on a regular symbol are those the symbol's cursor
> finds once `has_transitions` has said yes; its finality and final weight
> are what the reader reports. The writer reads every state reachable from
> the start, depth first, successors in stored order (free arcs, then
> regular arcs by symbol). It refuses an arc whose output is not its input,
> a weight that is NaN or `-inf`, a final weight that is not finite, and a
> symbol table divvunspell would refuse to load.

> [spec:hfst:def:dhfst.acceptor-distance]
> `Distances::compute` gives every state of a weighted optimized-lookup
> transducer its lookahead distance: the least weight of any path from it to
> a final state, final weight included, `+inf` when there is none.

> [spec:hfst:sem:dhfst.acceptor-distance]
> The distances are divvunspell's (`heuristic.rs`) bit for bit: Dijkstra
> over the reverse graph of every table position, seeded with the final
> weights, the queue ordered by `f32::total_cmp`. Negative weights are
> floored at zero and their sum is taken off every finite distance at the
> end. Every distance is 0 when a weight is NaN, when the floored sum
> exceeds 0.01, or when fewer than one reachable position in 1000 has a
> distance above 0.001.

> [spec:hfst:def:dhfst.acceptor-write]
> `dhfst_acceptor_writer::write(source, options)` writes an acceptor as
> DHFST type 2. The options are the worker thread count and the source name
> for `meta`.

> [spec:hfst:sem:dhfst.acceptor-write]
> For the same optimized-lookup input and source name, the output is the
> bytes `dhfst-tools acceptor` writes in every section but `meta`, and the
> two `meta` sections differ only in the `writer` value. Each state's
> distance is stored bit for bit. The output does not depend on the thread
> count.

> [spec:hfst:sem:dhfst.acceptor-self-check]
> Nothing is written unless the bytes, parsed by the reader, answer as the
> source does for every state: its finality and final weight, its distance,
> whether it has free arcs and which, and for every symbol of the alphabet
> and three past it whether it has arcs on that symbol and which, weights
> compared bit for bit. A free symbol must have no arcs in the reader.

> [spec:hfst:def:dhfst.acceptor-meta]
> An acceptor's `meta` section is one line of JSON with the keys `arcs`,
> `distances`, `free_arcs`, `placement`, `source`, `states` and `writer`, in
> that order.

> [spec:hfst:sem:dhfst.acceptor-meta]
> `arcs` and `free_arcs` count the arcs written, `distances` says whether
> `DIST` is present, `placement` is `depth-first`, `states` counts the states
> written, and `source` and `writer` are as in an error model's `meta`.
> Strings are escaped as `serde_json` escapes them.

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

> [spec:hfst:def:dhfst.fst2fst+2]
> `hfst-fst2fst -f dhfst --dhfst-type errmodel [--max-fallback-depth N |
> --unbounded-fallback]` writes the input transducer as a DHFST error model,
> and `hfst-fst2fst -f dhfst --dhfst-type acceptor` as a DHFST acceptor.

> [spec:hfst:sem:dhfst.fst2fst+2]
> The transducer is first converted exactly as `-f olw` converts it (`-Q`
> included), so `hfst-fst2fst -f dhfst --dhfst-type errmodel -i X` writes
> what `dhfst-tools write` writes, and `--dhfst-type acceptor` what
> `dhfst-tools acceptor` writes, from the output of `hfst-fst2fst -f olw -i
> X`, but for the `meta` `writer` value. The type is never guessed: `-f
> dhfst` without `--dhfst-type` is refused, and so is `--dhfst-type` without
> `-f dhfst`. The input must hold one transducer. The output, a file or
> standard output, is created only once the self-check has passed. `-v`
> prints the writer's report. The depth options are refused without
> `--dhfst-type errmodel` and together, and `-b` is refused with `-f dhfst`.

> [spec:hfst:def:dhfst.bhfst-member+2]
> In a BHFST archive a DHFST error model is the single member
> `errmodel.default.dhfst`, in place of the `errmodel.default.thfst`
> directory, and a DHFST acceptor the single member
> `acceptor.default.dhfst`, in place of the `acceptor.default.thfst`
> directory. An archive may hold both.

> [spec:hfst:sem:dhfst.bhfst-member+2]
> `hfst-bhfst -e FILE` and `-a FILE`, and the error model and acceptor of
> `-z`, are DHFST when FILE starts with `DHFST`. The error model must be type
> 1 and the acceptor type 2; a file of another type is refused, naming its
> type and the type wanted. The file is validated with the reader of its
> type and stored, unchanged and uncompressed, as `errmodel.default.dhfst` or
> `acceptor.default.dhfst`, in the order acceptor, error model, `meta.json`.
> Metadata converted from index.xml gets the id `errmodel.default.dhfst` or
> `acceptor.default.dhfst` for a DHFST member; a `-m` meta.json stays
> verbatim.
