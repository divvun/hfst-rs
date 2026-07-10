# back-ends/thfst — THFST backend + BHFST packing (Rust, target-only, novel)

Target-side backend that makes `ImplementationType::THFST_TYPE` a real,
usable transducer implementation. THFST (Tromsø-Helsinki Finite-State
Transducer) is divvunspell's mmap-optimized on-disk refinement of the
HFST optimized-lookup tables; BHFST is its box-container speller
archive (the `.box` successor to zip/zhfst). Neither has any C++ HFST
ancestor — these rules are authored greenfield and their contract is
**divvunspell compatibility**: the consumer of record is
`github.com/divvun/divvunspell` (its `src/transducer/thfst/` reader and
`src/archive/boxf.rs` archive loader), and the reference producer this
backend must match is `thfst-tools` (divvunspell
`crates/thfst-tools`).

Unlike foma, the backend is NOT feature-gated: it is always on. Its
runtime is the in-tree weighted optimized-lookup engine; the only new
`hfst`-crate dependency is serde/serde_json for the alphabet file.

## Motivation

Two purposes. (1) Proof of seam: the first post-parity backend that
C++ HFST never had, demonstrating the monomorphic backend taxonomy
([dec:hfst:monomorphic-backends]) is extensible by third parties.
(2) Pipeline need: today Giella spellers go hfst → zhfst (zip) →
`thfst-tools` → bhfst. With `hfst-fst2fst -f thfst` and `hfst-bhfst`,
rust-hfst emits divvunspell's artifacts directly and the zip and
conversion steps disappear.

## The backend type

> [spec:hfst:def:thfst-backend.thfst-transducer]
> A newtype wrapper `ThfstTransducer(Transducer<WeightedTables>)` — the
> backend's transducer handle, the in-memory weighted optimized-lookup
> engine under a distinct stream identity. Implements `Backend`
> (const `TYPE = THFST_TYPE`) and `LookupBackend` (by delegation to the
> inner engine), but NOT `AlgebraBackend`: THFST is a lookup-tier
> citizen exactly like HFST_OL/OLW. `stream_type()` returns
> `THFST_TYPE` unconditionally (the format is weighted-only; a
> logically-unweighted source serializes with 0.0 weights).
> `Backend::write` (the byte-stream arm) always errors: THFST has no
> byte-stream serialization (see `.directory-format`); serialization
> goes through `write_to_dir`.

## The directory format

> [spec:hfst:def:thfst-backend.directory-format]
> A THFST transducer on disk is a DIRECTORY `X.thfst/` containing
> exactly three files: `alphabet`, `index`, `transition`.

> [spec:hfst:sem:thfst-backend.directory-format]
> There is no HFST3 wrapper header anywhere — none of the three files
> begins with "HFST"; the directory itself is the container. One
> directory holds exactly one transducer. Consequently THFST has no
> byte-stream encoding at all: it can never appear on stdin/stdout,
> never inside a multi-transducer HFST stream, and is never a
> candidate in byte-sniffing format detection ('guess_fst_type' /
> 'process_header_data' deliberately have no THFST arm).

> [spec:hfst:def:thfst-backend.index-record]
> `index`: a flat array of 8-byte little-endian records
> { u16 input_symbol, u16 padding = 0, u32 weight_or_target }.

> [spec:hfst:sem:thfst-backend.index-record]
> input_symbol 0xFFFF means "no symbol"; weight_or_target 0xFFFFFFFF
> means "no target". For final-state entries (input_symbol == 0xFFFF
> with a non-none second field) the u32 holds the f32 final weight's
> raw bits (bit reinterpretation, not conversion). The record is the
> HFST-OLW on-disk index record (6 bytes: u16 input + u32
> target-or-weight) with two zero padding bytes inserted after the
> input symbol so the 4-byte field is 4-byte aligned for mmap access.
> Writing from OLW tables is therefore a raw bit copy: emit the stored
> `first_transition_index` u32 verbatim (it already carries weight
> bits for final entries and 0xFFFFFFFF for empty slots) — never
> decode-and-reencode. File length must be a multiple of 8.

> [spec:hfst:def:thfst-backend.transition-record]
> `transition`: a flat array of 12-byte little-endian records
> { u16 input_symbol, u16 output_symbol, u32 target, f32 weight }.

> [spec:hfst:sem:thfst-backend.transition-record]
> Identical field layout to the HFST-OLW on-disk transition record;
> 0xFFFF / 0xFFFFFFFF are the no-symbol / no-target markers, and the
> transition-table address space starts at TRANSITION_TARGET_TABLE_START
> = 2^31 exactly as in optimized-lookup. rust-hfst's OLW writer emits
> native-endian; the THFST writer must serialize each field with
> explicit little-endian byte order. File length must be a multiple
> of 12.

> [spec:hfst:def:thfst-backend.alphabet-json]
> `alphabet`: a JSON object with fields (in this order) key_table:
> [string], initial_symbol_count: u16, flag_state_size: u16,
> length: usize, string_to_symbol: {string: u16}, operations:
> {u16: {operation, feature: u16, value: i16}}, identity_symbol:
> u16|null, unknown_symbol: u16|null — the serde shape of
> divvunspell's `TransducerAlphabet`.

> [spec:hfst:sem:thfst-backend.alphabet-json]
> The writer derives every field from the transducer's symbol STRINGS
> by replicating divvunspell's `TransducerAlphabetParser` algorithm
> verbatim (divvunspell src/transducer/hfst/alphabet.rs), never by
> translating rust-hfst's internal `FdTable` numbers — the two schemes
> disagree (`FdTable` preseeds the empty value at 0 and starts real
> values at 2, C++ semantics; divvunspell numbers values 0,1,2,... in
> first-encounter order). Scan symbols 0..N (N = the header symbol
> count) in order:
> - A key with len > 1 that starts and ends with '@' and has byte '.'
>   at index 2 and len >= 5 is a FLAG DIACRITIC: the operator is the
>   single char at index 1 and must be one of P N R D C U (serialized
>   in JSON as the enum variant names PositiveSet, NegativeSet,
>   Require, Disallow, Clear, Unification; any other operator char is
>   an error, mirroring divvunspell's hard error). Its feature is the
>   second '.'-chunk and value the third (or "" when absent —
>   divvunspell accepts valueless P/N/U like "@P.FOO@", which
>   rust-hfst's own `is_diacritic` would reject; the writer follows
>   divvunspell). Feature and value numbers are assigned 0-based in
>   first-encounter order. The literal flag string is pushed to
>   key_table; an entry {operation, feature, value} is added to
>   operations keyed by the symbol number; flags get NO
>   string_to_symbol entry.
> - "@_EPSILON_SYMBOL_@" pushes "" to key_table and inserts "" into
>   the value bucket (consuming the next value number — for the
>   canonical symbol-0 epsilon this is value 0).
> - "@_IDENTITY_SYMBOL_@" / "@_UNKNOWN_SYMBOL_@" set identity_symbol /
>   unknown_symbol to the symbol number and are pushed literally; no
>   string_to_symbol entry.
> - Any other "@...@" special pushes "" with a warning; no
>   string_to_symbol entry.
> - Every plain symbol is pushed literally and gets a string_to_symbol
>   entry mapping string → symbol number.
> flag_state_size = the count of distinct flag FEATURES;
> initial_symbol_count = N; length = Σ over the ORIGINAL symbol
> strings of (byte length + 1) — the byte size the alphabet section
> would occupy in the OL binary format including NUL terminators.
> Serialization is pretty-printed JSON; map iteration order is
> reader-irrelevant (divvunspell deserializes into hash maps), so the
> writer uses ordered maps for deterministic output while the
> compatibility oracle for this file is semantic equality, not byte
> equality.

## Serialization functions

> [spec:hfst:def:thfst-backend.write-dir-fn]
> fn write_dir(&self, dir: &Path) -> Result<()>

> [spec:hfst:sem:thfst-backend.write-dir-fn]
> Create `dir` (and parents) if absent, then write the three member
> files: `transition` by iterating the inner OLW transition table
> 0..target_table_size emitting each record per
> `.transition-record`; `index` by iterating the index table
> 0..index_table_size emitting { input, 0u16, raw stored u32 } per
> `.index-record`; `alphabet` per `.alphabet-json`. Against the
> reference producer (`thfst-tools hfst-to-thfst` on the same OLW
> input) `index` and `transition` are byte-identical and `alphabet`
> is semantically equal.

> [spec:hfst:def:thfst-backend.read-dir-fn]
> fn read_dir(dir: &Path) -> Result<ThfstTransducer>

> [spec:hfst:sem:thfst-backend.read-dir-fn]
> Require `dir` to be a directory containing all three member files
> (else NotTransducerStream). Parse `alphabet` as JSON; rebuild the
> engine alphabet from key_table with index 0's "" restored to
> "@_EPSILON_SYMBOL_@", re-deriving flag operations and
> identity/unknown from the symbol strings via the engine's own
> symbol-table constructor — the JSON operations/feature/value
> NUMBERS are ignored (the engine renumbers internally; semantics are
> preserved because both schemes are derived from the same strings).
> Read `index`/`transition` with length-multiple guards (8/12) into
> the OLW table types. Synthesize the OL header that THFST does not
> store: both symbol counts = key_table length, table sizes = exact
> record counts from the file lengths, weighted = true, and all nine
> property flags false. Two documented divergences from a native OLW
> round-trip follow: (1) input_symbol_count is approximated by the
> total symbol count (THFST does not record it; divvunspell has the
> same loss), which can only ENLARGE the tokenizer's input symbol
> set; (2) property flags default false, so
> `is_infinitely_ambiguous` may under-report and `hfst-summarize`
> output differs from the OLW original. Lookup results (strings and
> weights) are unaffected.

## Conversions

> [spec:hfst:def:thfst-backend.olw-moves]
> Conversions between THFST and HFST_OLW are O(1) table MOVES, not
> round-trips through the basic transducer: `from_any` /
> `into_thfst()` / `into_olw()` transfer the inner
> `Transducer<WeightedTables>` and retag, preserving the facade
> metadata (name, properties, anonymous, is_trie). Conversions to or
> from every other backend go through the basic transducer, exactly
> like OLW's. `from_basic` builds weighted OL tables (the OLW
> conversion path with weighted = true).

## Stream integration

> [spec:hfst:def:thfst-backend.stream-io]
> Directory-format arms of HfstInputStream / HfstOutputStream: the
> filename constructors special-case `.thfst` directories; the byte
> paths never see THFST.

> [spec:hfst:sem:thfst-backend.stream-io]
> INPUT: `HfstInputStream::new_filename` on a path that is a
> directory: if the three THFST members exist, load via
> `read_dir` into a preloaded slot with ty = THFST_TYPE (no byte
> probing, no HFST header); the first `read()` yields the preloaded
> transducer, the second yields EndOfStream, and `is_eof`/`is_good`
> reflect the preloaded slot. A directory missing the members is
> NotTransducerStream with a message naming the three files. The
> stdin constructor and `stream_fst_type` byte-sniffing can never
> produce THFST_TYPE (per `.directory-format`).
> OUTPUT: `HfstOutputStream::new` (stdout) with THFST_TYPE errors
> (StreamCannotBeWritten — a directory format cannot be streamed);
> `new_filename` binds a directory sink instead of a byte sink and
> forces hfst_format off (the XFSM native-only precedent). The first
> `write` serializes via the `Backend::write_to_dir` hook (default
> errors for every other backend; overridden by ThfstTransducer to
> `write_dir`); a second `write` errors — a .thfst directory holds
> exactly one transducer.

## BHFST packing

> [spec:hfst:def:thfst-backend.bhfst-layout]
> A BHFST speller archive `X.bhfst` is a box-format archive
> (github.com/bbqsrc/box) written with data alignment 8 whose entries
> are, in order: acceptor.default.thfst/{alphabet,index,transition},
> errmodel.default.thfst/{alphabet,index,transition}, and optionally
> a top-level meta.json — every entry stored UNCOMPRESSED
> (Compression::Stored).

> [spec:hfst:sem:thfst-backend.bhfst-layout]
> Stored compression and 8-byte alignment are hard requirements:
> divvunspell memory-maps the member files at their raw archive
> offsets, so compressed bytes or unaligned records would be read as
> table data. The two directory entry names are exactly
> "acceptor.default.thfst" and "errmodel.default.thfst" — divvunspell
> hard-codes them — regardless of the input file names; the packer
> re-homes differently-named inputs under the canonical names.
> meta.json is optional: a missing file yields metadata None, not an
> error.

> [spec:hfst:def:thfst-backend.meta-json]
> meta.json: the JSON serialization of divvunspell's SpellerMetadata —
> { info: { locale, title: [{lang: string|null, "$value": string}],
> description, producer }, acceptor: { type (default ""), id, title,
> description, continuation? }, errmodel: { id, title, description } }.

> [spec:hfst:sem:thfst-backend.meta-json]
> When converted from a zhfst index.xml, the XML is parsed with
> whitespace trimming, comment skipping, and character coalescing
> (serde-xml-rs 0.6 semantics, matching thfst-tools), title elements
> map to {lang: the optional xml:lang attribute, "$value": the text
> content}, the acceptor's `type` attribute defaults to "" when
> absent, and both the acceptor and errmodel `id` fields are
> rewritten replacing ".hfst" with ".thfst". A caller-supplied
> meta.json is embedded verbatim.

> [spec:hfst:def:thfst-backend.bhfst-tool]
> hfst-bhfst — a new CLI tool (no C++ ancestor) that packs and
> inspects BHFST archives:
> hfst-bhfst -a/--acceptor FILE -e/--errmodel FILE
> [-X/--index-xml FILE | -m/--meta FILE] -o/--output FILE.bhfst,
> or hfst-bhfst -I/--info FILE.bhfst.

> [spec:hfst:sem:thfst-backend.bhfst-tool]
> Pack mode: each of acceptor/errmodel may be a ready .thfst
> directory (used as-is, re-homed to the canonical entry name if
> named otherwise) or any transducer file readable by
> HfstInputStream, which is converted to THFST via the standard
> format-conversion path and serialized to a temporary directory
> first. Metadata comes from -X (index.xml, converted per
> `.meta-json`) or -m (meta.json verbatim); the two options are
> mutually exclusive and both optional. The archive is written per
> `.bhfst-layout`; the output path is required (no stdout). Info
> mode: open the archive, print meta.json (or a no-metadata notice).
> All failures exit nonzero through the common error path; the tool
> follows the house getopt/CommonOptions pattern.
