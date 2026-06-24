# libhfst/src/implementations/HfstTropicalTransducerTransitionData.cc, libhfst/src/implementations/HfstTropicalTransducerTransitionData.h

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.dummy1-fn]
> Number2SymbolVectorInitializer

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.dummy1-fn]
> This is the definition of a file-scope static object named `dummy1` of
> type `Number2SymbolVectorInitializer`, constructed with the static
> member `HfstTropicalTransducerTransitionData::number2symbol_map` passed
> by reference. Its sole purpose is the side effect of running the
> `Number2SymbolVectorInitializer` constructor at static-initialization
> time, which pre-populates `number2symbol_map` with the three reserved
> symbols (see that constructor's rule). The object itself is never used
> afterward. In Rust this corresponds to seeding the number2symbol vector
> with its initial three entries before any other use.

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.dummy2-fn]
> Symbol2NumberMapInitializer

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.dummy2-fn]
> This is the definition of a file-scope static object named `dummy2` of
> type `Symbol2NumberMapInitializer`, constructed with the static member
> `HfstTropicalTransducerTransitionData::symbol2number_map` passed by
> reference. Its only purpose is the side effect of running the
> `Symbol2NumberMapInitializer` constructor at static-initialization time,
> which seeds `symbol2number_map` with the three reserved symbol->number
> mappings (see that constructor's rule). The object is otherwise unused.
> In Rust this corresponds to seeding the symbol2number map with its
> initial three entries before any other use. Note: this static object is
> defined after `dummy1`, and immediately after it `max_number` is
> initialized to 2.

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data]
> class HfstTropicalTransducerTransitionData {
>   HFSTDLL static Number2SymbolVector;
>   number2symbol_map;
>   HFSTDLL static Symbol2NumberMap;
>   symbol2number_map;
>   HFSTDLL static unsigned;
>   int max_number;
>   HFSTDLL static unsigned;
>   static const std::string &get_symbol(unsigned int number);
>   unsigned int input_number;
>   unsigned int output_number;
>   WeightType weight;
>   HFSTDLL const SymbolType &get_input_symbol() const;
>   HFSTDLL const SymbolType &get_output_symbol() const;
> }

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-epsilon-fn]
> HfstTropicalTransducerTransitionData::SymbolType HfstTropicalTransducerTransitionData::get_epsilon()

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-epsilon-fn]
> Static function taking no arguments. Returns a `SymbolType` (a
> `std::string`) holding the constant string `"@_EPSILON_SYMBOL_@"`. No
> state read or mutated; no side effects.

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-harmonization-vector-fn]
> std::vector<unsigned int> HfstTropicalTransducerTransitionData::get_harmonization_vector

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-harmonization-vector-fn]
> Static function taking a const reference to a vector of `SymbolType`
> (`symbols`). Creates a result vector `harmv`, reserves
> `symbols.size()` capacity, and resizes it to `symbols.size()` elements
> all initialized to 0. Then iterates `i` from 0 to `symbols.size()-1`:
> if `symbols.at(i)` is not the empty string `""`, sets
> `harmv.at(i) = get_number(symbols.at(i))` (which looks up or assigns a
> new number for that symbol, mutating the static symbol/number maps as a
> side effect). Entries whose symbol is empty are left as 0. Returns
> `harmv`. The result maps each input position to the global number of
> the symbol at that position (0 for empty positions).

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-identity-fn]
> HfstTropicalTransducerTransitionData::SymbolType HfstTropicalTransducerTransitionData::get_identity()

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-identity-fn]
> Static function taking no arguments. Returns a `SymbolType` (a
> `std::string`) holding the constant string `"@_IDENTITY_SYMBOL_@"`. No
> state read or mutated; no side effects.

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-input-number-fn]
> unsigned int HfstTropicalTransducerTransitionData::get_input_number() const

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-input-number-fn]
> Const member function taking no arguments. Returns the instance field
> `input_number` (an `unsigned int`). No mutation; no side effects.

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-marker-fn]
> HfstTropicalTransducerTransitionData::SymbolType HfstTropicalTransducerTransitionData::get_marker(const SymbolTypeSet &sts)

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-marker-fn]
> Static function taking a const reference to a `SymbolTypeSet` argument
> `sts`. The argument is explicitly ignored (cast to void) and never
> used. Always returns a `SymbolType` holding the constant string
> `"@_MARKER_SYMBOL_@"`. No state read or mutated; no side effects.

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-max-number-fn]
> unsigned int HfstTropicalTransducerTransitionData::get_max_number()

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-max-number-fn]
> Static function taking no arguments. Returns the static class member
> `max_number` (an `unsigned int`, initialized to 2). No mutation; no
> side effects.

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-number-fn]
> unsigned int HfstTropicalTransducerTransitionData::get_number(const std::string &symbol)

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-number-fn]
> Static function taking a const reference to a `std::string` `symbol`,
> returning an `unsigned int`. It maps a symbol to its global number,
> assigning a new number if the symbol is unseen.
> Step 1 (empty-symbol guard): if `symbol` is empty, it looks the empty
> string up in the static `symbol2number_map`; if not found it prints
> `"ERROR: No number for the empty symbol\n"` (plus a newline) to stderr,
> otherwise it prints `"ERROR: The empty symbol corresdponds to number "`
> followed by the found number to stderr; then it calls `assert(false)`
> (aborting in debug builds). (Note the source's misspelling
> "corresdponds".)
> Step 2 (lookup): finds `symbol` in the static `symbol2number_map`. If
> found, returns the mapped number (`it->second`).
> Step 3 (assign new): if not found, increments the static `max_number`
> by 1, then sets `symbol2number_map[symbol] = max_number`, appends
> `symbol` to the back of the static `number2symbol_map` vector, and
> returns the new `max_number`. Mutates the two static maps and
> `max_number` as side effects.

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-output-number-fn]
> unsigned int HfstTropicalTransducerTransitionData::get_output_number() const

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-output-number-fn]
> Const member function taking no arguments. Returns the instance field
> `output_number` (an `unsigned int`). No mutation; no side effects.

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-reverse-harmonization-vector-fn]
> std::vector<unsigned int> HfstTropicalTransducerTransitionData::get_reverse_harmonization_vector

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-reverse-harmonization-vector-fn]
> Static function taking a const reference to a map from `SymbolType` to
> `unsigned int` (`symbols`). Creates a result vector `harmv`, reserves
> `max_number+1` capacity, and resizes it to `max_number+1` elements all
> initialized to 0 (where `max_number` is the static class member). Then
> iterates `i` from 0 to `harmv.size()-1`: calls `get_symbol(i)` to get
> the global symbol with number `i` (this may throw HfstFatalException if
> `i` is out of range of `number2symbol_map`), looks that symbol up in
> the passed-in `symbols` map; if found, sets `harmv.at(i)` to the mapped
> value (`it->second`), otherwise leaves it as 0. Returns `harmv`. The
> result maps each global symbol number `i` to the number that the
> caller's map assigns to the same symbol (0 when the symbol is absent
> from `symbols`).

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-unknown-fn]
> HfstTropicalTransducerTransitionData::SymbolType HfstTropicalTransducerTransitionData::get_unknown()

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-unknown-fn]
> Static function taking no arguments. Returns a `SymbolType` (a
> `std::string`) holding the constant string `"@_UNKNOWN_SYMBOL_@"`. No
> state read or mutated; no side effects.

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-weight-fn]
> HfstTropicalTransducerTransitionData::WeightType HfstTropicalTransducerTransitionData::get_weight() const

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.get-weight-fn]
> Const member function taking no arguments. Returns the instance field
> `weight` (a `WeightType`, i.e. `float`). No mutation; no side effects.

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.hfst-tropical-transducer-transition-data-fn]
> HfstTropicalTransducerTransitionData::HfstTropicalTransducerTransitionData(HfstTropicalTransducerTransitionData::SymbolType isymbol,

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.hfst-tropical-transducer-transition-data-fn]
> Constructor taking `SymbolType isymbol`, `SymbolType osymbol`, and
> `WeightType weight` (all by value). First, if either `isymbol` or
> `osymbol` is the empty string, throws an `EmptyStringException` with
> the message
> `"HfstTropicalTransducerTransitionData(SymbolType, SymbolType, WeightType)"`.
> Otherwise sets the instance field `input_number = get_number(isymbol)`
> and `output_number = get_number(osymbol)` (each of which looks up or
> assigns a global number for the symbol, mutating the static maps and
> `max_number` as a side effect), and sets `this->weight = weight`.

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.is-epsilon-fn]
> bool HfstTropicalTransducerTransitionData::is_epsilon(const SymbolType &symbol)

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.is-epsilon-fn]
> Static function taking a const reference to a `SymbolType` `symbol`.
> Returns true if `symbol` equals the string `"@_EPSILON_SYMBOL_@"`
> (exact string comparison, `compare(...) == 0`), false otherwise. No
> state mutated; no side effects.

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.is-identity-fn]
> bool HfstTropicalTransducerTransitionData::is_identity(const SymbolType &symbol)

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.is-identity-fn]
> Static function taking a const reference to a `SymbolType` `symbol`.
> Returns true if `symbol` equals the string `"@_IDENTITY_SYMBOL_@"`
> (exact string comparison, `compare(...) == 0`), false otherwise. No
> state mutated; no side effects.

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.is-unknown-fn]
> bool HfstTropicalTransducerTransitionData::is_unknown(const SymbolType &symbol)

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.is-unknown-fn]
> Static function taking a const reference to a `SymbolType` `symbol`.
> Returns true if `symbol` equals the string `"@_UNKNOWN_SYMBOL_@"`
> (exact string comparison, `compare(...) == 0`), false otherwise. No
> state mutated; no side effects.

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.is-valid-symbol-fn]
> bool HfstTropicalTransducerTransitionData::is_valid_symbol(const SymbolType &symbol)

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.is-valid-symbol-fn]
> Static function taking a const reference to a `SymbolType` `symbol`.
> Returns false if `symbol` is the empty string; otherwise returns true.
> No state mutated; no side effects.

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.less-than-ignore-weight-fn]
> bool HfstTropicalTransducerTransitionData::less_than_ignore_weight(const HfstTropicalTransducerTransitionData &another)

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.less-than-ignore-weight-fn]
> Const member function taking a const reference to another
> `HfstTropicalTransducerTransitionData` `another`. Returns a bool
> ordering by (input_number, output_number) only, ignoring weight:
> returns true if `input_number < another.input_number`; returns false if
> `input_number > another.input_number`; else if equal, returns true if
> `output_number < another.output_number`, false if
> `output_number > another.output_number`; else (both equal) returns
> false. No mutation; no side effects.

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.number2-symbol-vector]
> typedef std::vector<SymbolType>

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.operator-fn]
> bool HfstTropicalTransducerTransitionData::operator<(const HfstTropicalTransducerTransitionData &another)

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.operator-fn]
> Const `operator<` taking a const reference to another
> `HfstTropicalTransducerTransitionData` `another`. Returns a bool
> lexicographic ordering by (input_number, output_number, weight):
> returns true if `input_number < another.input_number`; returns false if
> `input_number > another.input_number`; else if equal, returns true if
> `output_number < another.output_number`, false if
> `output_number > another.output_number`; else (both equal) returns
> `weight < another.weight`. No mutation; no side effects.

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.print-transition-data-fn]
> void HfstTropicalTransducerTransitionData::print_transition_data()

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.print-transition-data-fn]
> Member function taking no arguments, returning void. Prints to stderr
> (via `fprintf(stderr, ...)`) the instance's data formatted as
> `"%i:%i %f\n"` with the arguments `input_number`, `output_number`, and
> `weight` in that order, i.e. input-number, a colon, output-number, a
> space, the weight as a float, then a newline. No state mutated.

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.set-input-symbol-fn]
> void HfstTropicalTransducerTransitionData::set_input_symbol(const HfstTropicalTransducerTransitionData::SymbolType & symbol)

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.set-input-symbol-fn]
> Member function taking a const reference to a `SymbolType` `symbol`,
> returning void. Sets the instance field `input_number` to
> `get_number(symbol)` (which looks up or assigns a global number for the
> symbol, mutating the static maps and `max_number` as a side effect).

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.set-output-symbol-fn]
> void HfstTropicalTransducerTransitionData::set_output_symbol(const HfstTropicalTransducerTransitionData::SymbolType & symbol)

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.set-output-symbol-fn]
> Member function taking a const reference to a `SymbolType` `symbol`,
> returning void. Sets the instance field `output_number` to
> `get_number(symbol)` (which looks up or assigns a global number for the
> symbol, mutating the static maps and `max_number` as a side effect).

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.set-weight-fn]
> void HfstTropicalTransducerTransitionData::set_weight(WeightType w)

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.set-weight-fn]
> Member function taking a `WeightType w` (i.e. `float`), returning void.
> Sets the instance field `weight` to `w`. No other side effects.

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.symbol-type]
> typedef std::string SymbolType

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.symbol-type-set]
> typedef std::set<SymbolType> SymbolTypeSet

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.symbol2-number-map]
> typedef std::map<SymbolType, unsigned int, string_comparison>

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.hfst-tropical-transducer-transition-data.weight-type]
> typedef float WeightType

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.number2-symbol-vector-initializer]
> class Number2SymbolVectorInitializer

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.number2-symbol-vector-initializer.number2-symbol-vector-initializer-fn]
> Number2SymbolVectorInitializer::Number2SymbolVectorInitializer

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.number2-symbol-vector-initializer.number2-symbol-vector-initializer-fn]
> Constructor of `Number2SymbolVectorInitializer` taking a mutable
> reference `vect` to a `Number2SymbolVector` (a vector of `SymbolType`).
> Appends three reserved symbols to the back of `vect` in this exact
> order: `"@_EPSILON_SYMBOL_@"` (index 0), `"@_UNKNOWN_SYMBOL_@"`
> (index 1), then `"@_IDENTITY_SYMBOL_@"` (index 2). This establishes the
> initial number->symbol mapping. No return value; the only effect is
> mutating `vect`.

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.string-comparison]
> struct string_comparison

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.string-comparison.operator-fn]
> bool operator() (const std::string &str1, const std::string &str2) const

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.string-comparison.operator-fn]
> Const call operator of the `string_comparison` comparator functor,
> taking two const string references `str1` and `str2`. Returns true if
> `str1.compare(str2) < 0`, i.e. if `str1` is lexicographically (byte-
> wise) less than `str2`. This is the strict-weak-ordering used to order
> keys in the `symbol2number_map`. No state mutated; no side effects.

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.symbol2-number-map-initializer]
> class Symbol2NumberMapInitializer

> [spec:hfst:def:hfst-tropical-transducer-transition-data.hfst.implementations.symbol2-number-map-initializer.symbol2-number-map-initializer-fn]
> Symbol2NumberMapInitializer::Symbol2NumberMapInitializer

> [spec:hfst:sem:hfst-tropical-transducer-transition-data.hfst.implementations.symbol2-number-map-initializer.symbol2-number-map-initializer-fn]
> Constructor of `Symbol2NumberMapInitializer` taking a mutable reference
> `map` to a `Symbol2NumberMap` (map from `SymbolType` to `unsigned int`,
> ordered by `string_comparison`). Inserts three reserved symbol->number
> entries: `"@_EPSILON_SYMBOL_@"` -> 0, `"@_UNKNOWN_SYMBOL_@"` -> 1, and
> `"@_IDENTITY_SYMBOL_@"` -> 2. This establishes the initial
> symbol->number mapping consistent with the number2symbol vector. No
> return value; the only effect is mutating `map`.

