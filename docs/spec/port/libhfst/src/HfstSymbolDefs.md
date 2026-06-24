# libhfst/src/HfstSymbolDefs.cc, libhfst/src/HfstSymbolDefs.h

> [spec:hfst:def:hfst-symbol-defs.hfst.hfst-symbol-pair-substitutions]
> typedef std::map<StringPair, StringPair> HfstSymbolPairSubstitutions

> [spec:hfst:def:hfst-symbol-defs.hfst.hfst-symbol-substitutions]
> typedef std::map<String, String> HfstSymbolSubstitutions

> [spec:hfst:def:hfst-symbol-defs.hfst.hfst-two-level-path]
> typedef std::pair<float, StringPairVector> HfstTwoLevelPath

> [spec:hfst:def:hfst-symbol-defs.hfst.hfst-two-level-paths]
> typedef std::set<HfstTwoLevelPath> HfstTwoLevelPaths

> [spec:hfst:def:hfst-symbol-defs.hfst.is-default-fn]
> bool is_default(std::string const & str)

> [spec:hfst:sem:hfst-symbol-defs.hfst.is-default-fn]
> Returns true iff `str` equals the constant `internal_default`, whose
> value is the literal string `"@_DEFAULT_SYMBOL_@"`. No mutation, no side
> effects; a pure string equality test.

> [spec:hfst:def:hfst-symbol-defs.hfst.is-epsilon-fn]
> bool is_epsilon(std::string const & str)

> [spec:hfst:sem:hfst-symbol-defs.hfst.is-epsilon-fn]
> Returns true iff `str` equals the constant `internal_epsilon`, whose
> value is the literal string `"@_EPSILON_SYMBOL_@"`. No mutation, no side
> effects; a pure string equality test.

> [spec:hfst:def:hfst-symbol-defs.hfst.is-identity-fn]
> bool is_identity(std::string const & str)

> [spec:hfst:sem:hfst-symbol-defs.hfst.is-identity-fn]
> Returns true iff `str` equals the constant `internal_identity`, whose
> value is the literal string `"@_IDENTITY_SYMBOL_@"`. No mutation, no side
> effects; a pure string equality test.

> [spec:hfst:def:hfst-symbol-defs.hfst.is-unknown-fn]
> bool is_unknown(std::string const & str)

> [spec:hfst:sem:hfst-symbol-defs.hfst.is-unknown-fn]
> Returns true iff `str` equals the constant `internal_unknown`, whose
> value is the literal string `"@_UNKNOWN_SYMBOL_@"`. No mutation, no side
> effects; a pure string equality test.

> [spec:hfst:def:hfst-symbol-defs.hfst.number-number-map]
> typedef std::map<unsigned int,unsigned int> NumberNumberMap

> [spec:hfst:def:hfst-symbol-defs.hfst.number-pair]
> typedef std::pair<unsigned int, unsigned int> NumberPair

> [spec:hfst:def:hfst-symbol-defs.hfst.number-pair-set]
> typedef std::set<NumberPair> NumberPairSet

> [spec:hfst:def:hfst-symbol-defs.hfst.number-pair-vector]
> typedef std::vector<NumberPair> NumberPairVector

> [spec:hfst:def:hfst-symbol-defs.hfst.string]
> typedef std::string String

> [spec:hfst:def:hfst-symbol-defs.hfst.string-number-map]
> typedef std::map<String,unsigned int> StringNumberMap

> [spec:hfst:def:hfst-symbol-defs.hfst.string-pair]
> typedef std::pair<String, String> StringPair

> [spec:hfst:def:hfst-symbol-defs.hfst.string-pair-set]
> typedef std::set<StringPair> StringPairSet

> [spec:hfst:def:hfst-symbol-defs.hfst.string-pair-vector]
> typedef std::vector<StringPair> StringPairVector

> [spec:hfst:def:hfst-symbol-defs.hfst.string-set]
> typedef std::set<String> StringSet

> [spec:hfst:def:hfst-symbol-defs.hfst.string-vector]
> typedef std::vector<String> StringVector

> [spec:hfst:def:hfst-symbol-defs.hfst.symbols.collect-unknown-sets-fn]
> void collect_unknown_sets(StringSet &s1, StringSet &unknown1,

> [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.collect-unknown-sets-fn]
> Computes the symmetric difference between two symbol sets `s1` and `s2`,
> accumulating into two output sets passed by reference. First loop: for each
> symbol in `s1`, if it is not present in `s2`, insert it into `unknown2`.
> Second loop: for each symbol in `s2`, if it is not present in `s1`, insert it
> into `unknown1`. Thus `unknown2` ends up holding symbols that occur only in
> `s1` (unknown to side 2), and `unknown1` holds symbols that occur only in
> `s2` (unknown to side 1). Output sets are added to, not cleared first.
> Returns void; mutates `unknown1` and `unknown2` only.

> [spec:hfst:def:hfst-symbol-defs.hfst.symbols.get-longest-paths-fn]
> HfstTwoLevelPaths get_longest_paths(const HfstTwoLevelPaths & paths)

> [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.get-longest-paths-fn]
> Returns the subset of `paths` whose string-pair-vector (the `.second` of each
> `HfstTwoLevelPath`) has maximal length. First pass: iterate all paths and
> track `max_path_length` = the maximum of `it->second.size()` over all paths
> (starting from 0). Second pass: iterate all paths again and insert into the
> result set every path whose `it->second.size()` equals `max_path_length`. The
> result is an `HfstTwoLevelPaths` (a set); for an empty input the result is
> empty (max length stays 0, and no path matches since there are none). Does not
> mutate the input.

> [spec:hfst:def:hfst-symbol-defs.hfst.symbols.hfst.hfst-two-level-path-remove-flags-fn]
> HFSTDLL hfst::HfstTwoLevelPath remove_flags(const hfst::HfstTwoLevelPath & path)

> [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.hfst.hfst-two-level-path-remove-flags-fn]
> Removes flag-diacritic symbols from a single `HfstTwoLevelPath` (a pair of a
> float weight and a `StringPairVector`). Takes the path's `.second`
> string-pair-vector, applies `remove_flags` to it (filtering out any pair whose
> first or second component is a flag diacritic; see
> `string-pair-vector-remove-flags-fn`), and returns a new `HfstTwoLevelPath`
> built from the original `.first` weight and the filtered vector. The input is
> not mutated.

> [spec:hfst:def:hfst-symbol-defs.hfst.symbols.hfst.hfst-two-level-paths-get-longest-paths-fn]
> HFSTDLL hfst::HfstTwoLevelPaths get_longest_paths(const hfst::HfstTwoLevelPaths & paths)

> [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.hfst.hfst-two-level-paths-get-longest-paths-fn]
> Declaration of `get_longest_paths`; its body is the definition under
> `hfst-symbol-defs.hfst.symbols.get-longest-paths-fn`. Returns the subset of
> `paths` whose `.second` string-pair-vector is of maximal length: compute the
> maximum size over all paths, then collect every path matching that maximum.
> Empty input yields empty output.

> [spec:hfst:def:hfst-symbol-defs.hfst.symbols.hfst.hfst-two-level-paths-remove-flags-fn]
> HFSTDLL hfst::HfstTwoLevelPaths remove_flags(const hfst::HfstTwoLevelPaths & paths)

> [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.hfst.hfst-two-level-paths-remove-flags-fn]
> Removes flag-diacritic symbols from every path in a set of
> `HfstTwoLevelPath`. Iterates each path in `paths` and inserts into the result
> set a new `HfstTwoLevelPath` whose weight (`.first`) is unchanged and whose
> `.second` is `remove_flags(it->second)` — the string-pair-vector with all
> pairs containing a flag diacritic on either side filtered out. Returns the new
> `HfstTwoLevelPaths` set; input is not mutated. Note that because the result is
> a set, two distinct input paths that become identical after flag removal
> collapse into one entry.

> [spec:hfst:def:hfst-symbol-defs.hfst.symbols.longest-path-length-fn]
> int longest_path_length(const HfstTwoLevelPaths & paths, bool equally_long)

> [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.longest-path-length-fn]
> Returns the length (number of symbol pairs) of the longest path in `paths`,
> where each path's length is the size of its `.second` string-pair-vector.
> Control flow: if `paths` is empty, return -1. Otherwise, if `equally_long` is
> true, assume all paths have the same length and return the size of the first
> path's `.second` (cast to int) without scanning the rest. If `equally_long` is
> false, iterate every path, tracking the maximum `it->second.size()` (starting
> from 0), and return that maximum as an int. Does not mutate input; `equally_long`
> defaults to false.

> [spec:hfst:def:hfst-symbol-defs.hfst.symbols.remove-flags-fn]
> StringPairVector remove_flags(const StringPairVector &v)

> [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.remove-flags-fn]
> Returns a copy of the `StringPairVector` `v` with every symbol pair that
> contains a flag diacritic removed. Iterates each pair `it` in order; a pair is
> kept (pushed onto the result vector) only if neither `it->first` nor
> `it->second` is a flag diacritic, as determined by `FdOperation::is_diacritic`.
> Order of the kept pairs is preserved. Input is not mutated.

> [spec:hfst:def:hfst-symbol-defs.hfst.symbols.std.string-to-string-fn]
> HFSTDLL std::string to_string(const StringVector & sv, bool spaces=false)

> [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.std.string-to-string-fn]
> Concatenates the symbols of a `StringVector` `sv` into a single string. Builds
> a result string starting empty; iterates each element in order, and if
> `spaces` is true and this is not the first element, appends a single space `" "`
> before appending the element itself. With `spaces` false (the default), the
> symbols are concatenated with no separator. Returns the result string; input is
> not mutated.

> [spec:hfst:def:hfst-symbol-defs.hfst.symbols.string-pair-set-to-string-pair-set-fn]
> HFSTDLL StringPairSet to_string_pair_set(const StringSet & ss)

> [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.string-pair-set-to-string-pair-set-fn]
> Declaration of `to_string_pair_set`; its body is the definition under
> `hfst-symbol-defs.hfst.symbols.to-string-pair-set-fn`. Returns a
> `StringPairSet` containing, for each symbol `s` in the input `StringSet` `ss`,
> the identity pair `(s, s)`. Input is not mutated.

> [spec:hfst:def:hfst-symbol-defs.hfst.symbols.string-pair-vector-remove-flags-fn]
> HFSTDLL StringPairVector remove_flags(const StringPairVector &v)

> [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.string-pair-vector-remove-flags-fn]
> Declaration of the `StringPairVector` overload of `remove_flags`; its body is
> the definition under `hfst-symbol-defs.hfst.symbols.remove-flags-fn`. Returns a
> copy of `v` keeping only the pairs where neither the first nor the second
> component is a flag diacritic (per `FdOperation::is_diacritic`), preserving
> order. Input is not mutated.

> [spec:hfst:def:hfst-symbol-defs.hfst.symbols.string-vector-remove-flags-fn]
> HFSTDLL StringVector remove_flags(const StringVector &v)

> [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.string-vector-remove-flags-fn]
> Returns a copy of the `StringVector` `v` with every flag-diacritic symbol
> removed. Iterates each element `it` in order; keeps (pushes onto the result
> vector) an element only if it is not a flag diacritic, as determined by
> `FdOperation::is_diacritic(*it)`. Order of kept elements is preserved. Input is
> not mutated.

> [spec:hfst:def:hfst-symbol-defs.hfst.symbols.string-vector-to-string-vector-fn]
> HFSTDLL StringVector to_string_vector(const hfst::HfstTwoLevelPath & path)

> [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.string-vector-to-string-vector-fn]
> Extracts the input side of an `HfstTwoLevelPath` as a `StringVector`. Reads the
> path's `.second` string-pair-vector, then iterates it in order pushing each
> pair's first component (`it->first`) onto the result vector. Returns that
> vector of input-side symbols; the weight (`.first`) and output-side symbols are
> ignored. Input is not mutated.

> [spec:hfst:def:hfst-symbol-defs.hfst.symbols.to-string-fn]
> std::string to_string(const StringPairVector & spv, bool spaces)

> [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.to-string-fn]
> Renders a `StringPairVector` `spv` as a single string. Builds a result string
> starting empty; iterates each pair in order. If `spaces` is true and this is
> not the first pair, appends a single space `" "` before processing the pair.
> Then always appends the pair's first component; and if the first component
> differs from the second component, additionally appends a colon `":"` followed
> by the second component. Thus identity pairs render as just the symbol, and
> non-identity pairs render as `first:second`. Returns the result string; input
> is not mutated.

> [spec:hfst:def:hfst-symbol-defs.hfst.symbols.to-string-pair-set-fn]
> StringPairSet to_string_pair_set(const StringSet & ss)

> [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.to-string-pair-set-fn]
> Converts a `StringSet` `ss` into a `StringPairSet` of identity pairs. Iterates
> each symbol in `ss` and inserts the pair `(symbol, symbol)` into the result
> set. Returns the result set; input is not mutated.

> [spec:hfst:def:hfst-symbol-defs.hfst.symbols.to-string-vector-fn]
> StringVector to_string_vector(const StringPairVector & spv, bool input_side)

> [spec:hfst:sem:hfst-symbol-defs.hfst.symbols.to-string-vector-fn]
> Projects a `StringPairVector` `spv` onto one of its two sides as a
> `StringVector`. Iterates each pair in order; if `input_side` is true, pushes
> the pair's first component, otherwise pushes the pair's second component.
> Returns the resulting vector preserving order; input is not mutated.

> [spec:hfst:def:hfst-symbol-defs.main-fn]
> int main(int argc, char * argv[])

> [spec:hfst:sem:hfst-symbol-defs.main-fn]
> Unit-test entry point compiled only when `MAIN_TEST` is defined. Prints
> `"Unit tests for " __FILE__ ":"` followed by a newline to stdout, then prints
> `"ok"` followed by a newline, and returns 0. Performs no actual assertions;
> `argc`/`argv` are unused.

