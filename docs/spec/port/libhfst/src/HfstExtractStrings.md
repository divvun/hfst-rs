# libhfst/src/HfstExtractStrings.h

> [spec:hfst:def:hfst-extract-strings.hfst.extract-strings-cb]
> class ExtractStringsCb {
>   class RetVal { public: bool continueSearch; bool continuePath; // [spec:hfst:def:hfst-extract-strings.hfst.extract-strings-cb.ret-val.ret-val-fn] // [spec:hf...;
> }

> [spec:hfst:def:hfst-extract-strings.hfst.extract-strings-cb.operator-fn]
> virtual RetVal operator()(HfstTwoLevelPath& path, bool final) = 0

> [spec:hfst:sem:hfst-extract-strings.hfst.extract-strings-cb.operator-fn]
> Pure virtual call operator defining the callback contract; it has no body
> and must be overridden by concrete subclasses. The extraction routine
> (`extract_paths`) invokes it after every transition, passing the path
> accumulated so far as `path` (a mutable reference to an `HfstTwoLevelPath`,
> i.e. a `(float weight, StringPairVector)`) and a boolean `final` indicating
> whether that path currently ends at a final state. The implementation
> inspects the path and returns a `RetVal` whose two flags tell the caller
> whether to keep searching overall and whether to keep following this
> specific path.

> [spec:hfst:def:hfst-extract-strings.hfst.extract-strings-cb.ret-val]
> class RetVal {
>   bool continueSearch;
>   bool continuePath;
> }

> [spec:hfst:def:hfst-extract-strings.hfst.extract-strings-cb.ret-val.operator-fn]
> void operator=(const RetVal& o)

> [spec:hfst:sem:hfst-extract-strings.hfst.extract-strings-cb.ret-val.operator-fn]
> Copy-assignment operator for `RetVal`. Copies the two boolean members from
> the source object `o` into this object: sets `continueSearch = o.continueSearch`
> and `continuePath = o.continuePath`. No self-assignment guard is performed
> (none is needed for plain booleans). Returns void.

> [spec:hfst:def:hfst-extract-strings.hfst.extract-strings-cb.ret-val.ret-val-fn]
> RetVal(bool s, bool p): continueSearch(s), continuePath(p)

> [spec:hfst:sem:hfst-extract-strings.hfst.extract-strings-cb.ret-val.ret-val-fn]
> Constructor for `RetVal`. Takes two booleans `s` and `p` and initializes the
> members via the initializer list: `continueSearch = s` and `continuePath = p`.
> The body is empty.

> [spec:hfst:def:hfst-extract-strings.hfst.hfst-two-level-path]
> typedef std::pair<float, StringPairVector> HfstTwoLevelPath

> [spec:hfst:def:hfst-extract-strings.hfst.hfst-two-level-paths]
> typedef std::set<HfstTwoLevelPath> HfstTwoLevelPaths

> [spec:hfst:def:hfst-extract-strings.hfst.string-pair-vector]
> typedef std::vector<std::pair<std::string,std::string> > StringPairVector

> [spec:hfst:def:hfst-extract-strings.hfst.weighted-path]
> class WeightedPath {
>   std::string istring;
>   std::string ostring;
>   W weight;
>   StringPairVector spv;
>   bool is_spv_in_use;
> }

> [spec:hfst:def:hfst-extract-strings.hfst.weighted-path.operator-fn]
> bool operator< (const WeightedPath &another) const

> [spec:hfst:sem:hfst-extract-strings.hfst.weighted-path.operator-fn]
> Strict-weak-ordering `operator<` comparing this `WeightedPath` against `another`.
> Compares lexicographically in this priority order:
> 1. If `weight != another.weight`, return `weight < another.weight`.
> 2. Else if `istring != another.istring`, return `istring < another.istring`.
> 3. Else if `ostring != another.ostring`, return `ostring < another.ostring`.
> 4. Else (weight, istring, ostring all equal) compare the string-pair-vector
>    `spv`: if `is_spv_in_use` is false, return false (paths are equivalent).
>    Otherwise let `common_length` be the smaller of `spv.size()` and
>    `another.spv.size()`, and iterate `i` from 0 to `common_length`. For each
>    pair, if the `.first` components differ return `spv[i].first < another.spv[i].first`;
>    else if the `.second` components differ return `spv[i].second < another.spv[i].second`;
>    else continue to the next pair. If all common pairs are equal, return
>    `spv.size() < another.spv.size()` (the shorter spv is smaller).
> Note this class is compiled only when the `FOO` preprocessor macro is defined.

> [spec:hfst:def:hfst-extract-strings.hfst.weighted-path.to-string-fn]
> std::string to_string(void) const

> [spec:hfst:sem:hfst-extract-strings.hfst.weighted-path.to-string-fn]
> Builds a textual representation of the path. Creates an output stringstream and
> writes `istring`, then a literal `":"`, then `ostring`, then a literal tab
> character `"\t"`, then `weight`. Flushes the stream and returns the resulting
> string (format: `"<istring>:<ostring>\t<weight>"`). Const method; mutates no
> state. Compiled only when the `FOO` macro is defined.

> [spec:hfst:def:hfst-extract-strings.hfst.weighted-path.weighted-path-fn]
> WeightedPath(const std::string &is,const std::string &os,W w)

> [spec:hfst:sem:hfst-extract-strings.hfst.weighted-path.weighted-path-fn]
> Constructor taking input string `is`, output string `os`, and weight `w`.
> Assigns `weight = w`, `istring = is`, `ostring = os`, and sets `is_spv_in_use = false`.
> The `spv` member is left default-constructed (empty). Compiled only when the
> `FOO` macro is defined.

> [spec:hfst:def:hfst-extract-strings.hfst.weighted-paths]
> class WeightedPaths

> [spec:hfst:def:hfst-extract-strings.hfst.weighted-paths.add-fn]
> static void add(Vector &v,WeightedPath<W> &s)

> [spec:hfst:sem:hfst-extract-strings.hfst.weighted-paths.add-fn]
> Static method appending path `s` to the end of every path in vector `v`.
> Iterates over each element `it` of `v` and calls `it->add(s, false)`, where the
> `false` argument means "not in front", i.e. `s`'s istring/ostring are concatenated
> after each element's istring/ostring and `s`'s weight is added to each element's
> weight. Mutates every element of `v` in place; `s` is unchanged. Returns void.
> (There is a sibling overload `add(WeightedPath&, Vector&)` that instead prepends.)
> Compiled only when the `FOO` macro is defined.

> [spec:hfst:def:hfst-extract-strings.hfst.weighted-paths.cat-fn]
> static void cat(Vector &v, const Vector &another_v)

> [spec:hfst:sem:hfst-extract-strings.hfst.weighted-paths.cat-fn]
> Static method concatenating vector `another_v` onto the end of vector `v`.
> Calls `v.insert(v.end(), another_v.begin(), another_v.end())`, appending copies
> of all elements of `another_v` (in order) to `v`. `another_v` is unchanged.
> Returns void. Compiled only when the `FOO` macro is defined.

> [spec:hfst:def:hfst-extract-strings.hfst.weighted-paths.reverse-strings-fn]
> static void reverse_strings(Vector &v)

> [spec:hfst:sem:hfst-extract-strings.hfst.weighted-paths.reverse-strings-fn]
> Static method reversing the strings of every path in vector `v`. Iterates over
> each element `it` of `v` and calls `it->reverse()`, which reverses the
> characters of that path's `istring` and `ostring` in place (byte-wise, by
> swapping symmetric positions). Mutates every element of `v`. Returns void.
> Compiled only when the `FOO` macro is defined.

> [spec:hfst:def:hfst-extract-strings.hfst.weighted-paths.set]
> typedef std::set< WeightedPath<W> > Set

> [spec:hfst:def:hfst-extract-strings.hfst.weighted-paths.vector]
> typedef std::vector< WeightedPath<W> > Vector

