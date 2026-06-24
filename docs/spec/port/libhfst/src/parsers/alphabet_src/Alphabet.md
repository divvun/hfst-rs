# libhfst/src/parsers/alphabet_src/Alphabet.cc, libhfst/src/parsers/alphabet_src/Alphabet.h

> [spec:hfst:def:alphabet.alphabet]
> class Alphabet {
>   HandySet<SymbolPair> alphabet_set;
>   HandySet<std::string> input_symbols;
>   HandySet<std::string> output_symbols;
>   HandySet<std::string> diacritics;
>   HandyMap<SymbolPair,OtherSymbolTransducer> alphabet;
>   HandyMap<std::string,SymbolRange> sets;
>   const OtherSymbolTransducer &compute(const SymbolPair &pair);
>   const OtherSymbolTransducer &get_transducer(const SymbolPair &pair);
> }

> [spec:hfst:def:alphabet.alphabet.alphabet-done-fn]
> void

> [spec:hfst:sem:alphabet.alphabet.alphabet-done-fn]
> Signature `void Alphabet::alphabet_done(void)`. Takes no arguments
> and returns nothing. Calls the static method
> `OtherSymbolTransducer::set_symbol_pairs(alphabet_set)`, passing this
> Alphabet's current `alphabet_set` (the set of defined SymbolPairs).
> This registers the accumulated alphabet of symbol pairs globally on
> OtherSymbolTransducer so that subsequently constructed transducers
> know the alphabet. No other state is read or mutated.

> [spec:hfst:def:alphabet.alphabet.define-alphabet-pair-fn]
> void

> [spec:hfst:sem:alphabet.alphabet.define-alphabet-pair-fn]
> Signature `void Alphabet::define_alphabet_pair(const SymbolPair &pair)`.
> Registers one input:output symbol pair as part of the alphabet.
> Performs three insertions: inserts `pair` into `alphabet_set`;
> inserts `pair.first` (the input symbol) into `input_symbols`; and
> inserts `pair.second` (the output symbol) into `output_symbols`.
> All three are sets, so duplicate insertions are no-ops. Returns
> nothing.

> [spec:hfst:def:alphabet.alphabet.define-diacritics-fn]
> void

> [spec:hfst:sem:alphabet.alphabet.define-diacritics-fn]
> Signature `void Alphabet::define_diacritics(const SymbolRange &diacs)`.
> Marks a collection of symbols as diacritics and removes them from the
> ordinary alphabet bookkeeping. First inserts every element of the
> input range `diacs` (via begin()/end() iterators) into the
> `diacritics` set. Then iterates over every symbol currently in
> `diacritics` and, for each such symbol `s`: erases the pair
> `SymbolPair(s, s)` from `alphabet_set`; erases the pair
> `SymbolPair(s, TWOLC_EPSILON)` from `alphabet_set`; erases `s` from
> `input_symbols`; and erases `s` from `output_symbols`. Erasing
> absent elements is a harmless no-op. Returns nothing.

> [spec:hfst:def:alphabet.alphabet.define-set-fn]
> void

> [spec:hfst:sem:alphabet.alphabet.define-set-fn]
> Signature
> `void Alphabet::define_set(const std::string &name, const SymbolRange &elements)`.
> Assigns `sets[name] = elements`, i.e. stores (or overwrites) the
> mapping from the set name to its SymbolRange of member symbols in the
> `sets` map. Returns nothing.

> [spec:hfst:def:alphabet.alphabet.define-singleton-set-fn]
> void

> [spec:hfst:sem:alphabet.alphabet.define-singleton-set-fn]
> Signature `void Alphabet::define_singleton_set(const std::string &name)`.
> Defines a set named `name` whose sole member is the symbol `name`
> itself. Assigns `sets[name] = SymbolRange(1, name)`, i.e. a
> SymbolRange constructed as a sequence of length 1 containing the
> single string `name` (the standard sequence "count, value"
> constructor). Returns nothing.

> [spec:hfst:def:alphabet.alphabet.get-symbol-pair-vector-fn]
> SymbolPairVector *

> [spec:hfst:sem:alphabet.alphabet.get-symbol-pair-vector-fn]
> Signature
> `SymbolPairVector *Alphabet::get_symbol_pair_vector(const SymbolPair &pair)`.
> Returns a newly heap-allocated SymbolPairVector listing the concrete
> input:output transition pairs that the given abstract `pair` expands
> to. Steps: (1) Obtain `result_fst` by calling the private helper
> `get_transducer(pair)`, which returns `alphabet[pair]` if that key
> already exists in the `alphabet` map, otherwise calls `compute(pair)`
> to build and cache it. (2) Allocate a new empty `SymbolPairVector`
> with `new`. (3) Call `result_fst.get_initial_transition_pairs(*result)`
> to fill the vector with the pairs labelling the transducer's initial
> transitions. (4) Return the pointer. Ownership of the allocation
> passes to the caller (no delete here).

> [spec:hfst:def:alphabet.alphabet.is-empty-pair-fn]
> bool

> [spec:hfst:sem:alphabet.alphabet.is-empty-pair-fn]
> Signature `bool Alphabet::is_empty_pair(const SymbolPair &pair)`.
> First asserts `is_pair(pair.first, pair.second)` (a debug
> precondition that the pair is a valid alphabet pair; active only when
> assertions are enabled). Then returns `alphabet[pair].is_empty()`,
> i.e. looks up the transducer stored for `pair` in the `alphabet` map
> and returns whether that OtherSymbolTransducer is empty. Note that
> indexing `alphabet[pair]` will default-insert an entry if `pair` is
> not already a key (HandyMap `operator[]` semantics), so this can
> mutate the map.

> [spec:hfst:def:alphabet.alphabet.is-pair-fn]
> bool

> [spec:hfst:sem:alphabet.alphabet.is-pair-fn]
> Signature
> `bool Alphabet::is_pair(const std::string &input, const std::string &output)`.
> Decides whether the given input/output symbol strings form a valid
> alphabet pair, using a sequence of guarded early returns evaluated in
> this order (the literal `"__HFST_TWOLC_?"` is the unknown-symbol
> marker): (1) if both `input` and `output` equal `"__HFST_TWOLC_?"`,
> return true. (2) if `input` is in the `diacritics` set and
> `input == output`, return true. (3) if `input` is in `diacritics`
> and `output == "__HFST_TWOLC_?"`, return true. (4) if
> `input == "__HFST_TWOLC_?"`, return whether `output_symbols` contains
> `output`. (5) if `output == "__HFST_TWOLC_?"`, return whether
> `input_symbols` contains `input`. (6) otherwise return whether
> `alphabet_set` contains `SymbolPair(input, output)`. Read-only; no
> mutation.

> [spec:hfst:def:alphabet.alphabet.is-set-pair-fn]
> bool

> [spec:hfst:sem:alphabet.alphabet.is-set-pair-fn]
> Signature `bool Alphabet::is_set_pair(const SymbolPair &pair) const`.
> Const method, read-only. Returns true iff either side of the pair is
> a set-name marker symbol: specifically returns true if
> `pair.first.find("__HFST_TWOLC_SET_NAME=") != npos` OR
> `pair.second.find("__HFST_TWOLC_SET_NAME=") != npos`, i.e. if the
> substring `"__HFST_TWOLC_SET_NAME="` occurs anywhere within either
> the input or the output symbol string. Otherwise false.

> [spec:hfst:def:alphabet.main-fn]
> int

> [spec:hfst:sem:alphabet.main-fn]
> Signature `int main(void)`. Compiled only under the `TEST_ALPHABET`
> macro; a standalone smoke-test driver. Steps: (1) Determine which
> backends are available via compile-time macros, setting booleans
> `have_openfst` (HAVE_OPENFST), `have_sfst` (HAVE_SFST), `have_foma`
> (HAVE_FOMA). (2) Call
> `OtherSymbolTransducer::set_transducer_type(...)` choosing the type by
> priority: TROPICAL_OPENFST_TYPE if openfst, else SFST_TYPE if sfst,
> else FOMA_TYPE if foma, else ERROR_TYPE. (3) Construct an `Alphabet`.
> (4) Build three SymbolRanges from file-scope C-string arrays:
> `sr1` = {"a","b"} (name "X"), `sr2` = {"a","b","c"} (name "Y"),
> `sr3` = {"a"} (name "Z"). (5) Call `define_alphabet_pair` for pairs
> ("a","b"), ("b","c"), ("b","b"), then `alphabet_done()`. (6) Call
> `define_set` for ("X",sr1), ("Y",sr2), ("Z",sr3). Falls off the end
> with no explicit return (returns 0). Exercises construction paths;
> no assertions on results.

