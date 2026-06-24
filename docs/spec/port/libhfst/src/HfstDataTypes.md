# libhfst/src/HfstDataTypes.cc, libhfst/src/HfstDataTypes.h

> [spec:hfst:def:hfst-data-types.hfst.double-to-float-fn]
> float double_to_float(double value)

> [spec:hfst:sem:hfst-data-types.hfst.double-to-float-fn]
> Converts a `double` to a `float`. If `value` is greater than `FLT_MAX`
> (the maximum finite `float`), throws `std::overflow_error` with the
> message "data is larger than FLT_MAX". Otherwise returns the value cast
> to `float` (a narrowing `static_cast`). Note it does not guard against
> values below `-FLT_MAX`.

> [spec:hfst:def:hfst-data-types.hfst.hfst-fopen-fn]
> FILE * hfst_fopen(const char * filename, const char * mode)

> [spec:hfst:sem:hfst-data-types.hfst.hfst-fopen-fn]
> Opens a file by name in the given mode, returning a `FILE *`. On MSVC
> (`_MSC_VER` defined), uses the secure `fopen_s(&f, filename, mode)`:
> if it returns a non-zero error code, returns `NULL`; otherwise returns
> the opened `FILE *`. On all other platforms, simply returns
> `fopen(filename, mode)` (which is `NULL` on failure). A thin portability
> wrapper around the platform's file-open call.

> [spec:hfst:def:hfst-data-types.hfst.hfst-one-level-path]
> typedef std::pair<float, StringVector> HfstOneLevelPath

> [spec:hfst:def:hfst-data-types.hfst.hfst-one-level-paths]
> typedef std::set<HfstOneLevelPath> HfstOneLevelPaths

> [spec:hfst:def:hfst-data-types.hfst.hfst-transducer-pair]
> typedef std::pair <HfstTransducer,HfstTransducer> HfstTransducerPair

> [spec:hfst:def:hfst-data-types.hfst.hfst-transducer-pair-vector]
> typedef std::vector <HfstTransducerPair> HfstTransducerPairVector

> [spec:hfst:def:hfst-data-types.hfst.hfst-transducer-vector]
> typedef std::vector<HfstTransducer> HfstTransducerVector

> [spec:hfst:def:hfst-data-types.hfst.hfst-two-level-path]
> typedef std::pair<float, StringPairVector> HfstTwoLevelPath

> [spec:hfst:def:hfst-data-types.hfst.hfst-two-level-paths]
> typedef std::set<HfstTwoLevelPath> HfstTwoLevelPaths

> [spec:hfst:def:hfst-data-types.hfst.implementation-type]
> enum ImplementationType {
>   SFST_TYPE;
>   TROPICAL_OPENFST_TYPE;
>   LOG_OPENFST_TYPE;
>   FOMA_TYPE;
>   XFSM_TYPE;
>   HFST_OL_TYPE;
>   HFST_OLW_TYPE;
>   HFST2_TYPE;
>   UNSPECIFIED_TYPE;
>   ERROR_TYPE;
> }

> [spec:hfst:def:hfst-data-types.hfst.implementation-type-to-format-fn]
> const char * implementation_type_to_format(ImplementationType type)

> [spec:hfst:sem:hfst-data-types.hfst.implementation-type-to-format-fn]
> Maps an `ImplementationType` enum value to a static format-name C string,
> via a switch: `SFST_TYPE` -> "sfst"; `TROPICAL_OPENFST_TYPE` ->
> "openfst-tropical"; `LOG_OPENFST_TYPE` -> "openfst-log"; `FOMA_TYPE` ->
> "foma"; `XFSM_TYPE` -> "xfsm"; `HFST_OL_TYPE` ->
> "hfst-optimized-lookup-unweighted"; `HFST_OLW_TYPE` ->
> "hfst-optimized-lookup-weighted"; `HFST2_TYPE` -> "hfst2";
> `UNSPECIFIED_TYPE` -> "unspecified-type"; `ERROR_TYPE` -> "error-type".
> Any other/unrecognized value returns
> "(implementation-type-not-recognized)". No side effects; returns a
> pointer to a string literal.

> [spec:hfst:def:hfst-data-types.hfst.implementation-type-to-string-fn]
> const char * implementation_type_to_string(ImplementationType type)

> [spec:hfst:sem:hfst-data-types.hfst.implementation-type-to-string-fn]
> Maps an `ImplementationType` enum value to the static C string of its
> own enumerator name, via a switch: `SFST_TYPE` -> "SFST_TYPE";
> `TROPICAL_OPENFST_TYPE` -> "TROPICAL_OPENFST_TYPE"; `LOG_OPENFST_TYPE`
> -> "LOG_OPENFST_TYPE"; `FOMA_TYPE` -> "FOMA_TYPE"; `XFSM_TYPE` ->
> "XFSM_TYPE"; `HFST_OL_TYPE` -> "HFST_OL_TYPE"; `HFST_OLW_TYPE` ->
> "HFST_OLW_TYPE"; `HFST2_TYPE` -> "HFST2_TYPE"; `UNSPECIFIED_TYPE` ->
> "UNSPECIFIED_TYPE"; `ERROR_TYPE` -> "ERROR_TYPE". Any other/unrecognized
> value returns "(implementation type not recognized)". No side effects;
> returns a pointer to a string literal.

> [spec:hfst:def:hfst-data-types.hfst.implementations.hfst-state]
> typedef unsigned int HfstState

> [spec:hfst:def:hfst-data-types.hfst.push-type]
> enum PushType {
>   TO_INITIAL_STATE;
>   TO_FINAL_STATE;
> }

> [spec:hfst:def:hfst-data-types.hfst.size-t-to-int-fn]
> int size_t_to_int(size_t value)

> [spec:hfst:sem:hfst-data-types.hfst.size-t-to-int-fn]
> Converts a `size_t` to an `int`. If `value` is greater than `INT_MAX`,
> throws `std::overflow_error` with the message "data is larger than
> INT_MAX". Otherwise returns the value cast to `int` via `static_cast`.

> [spec:hfst:def:hfst-data-types.hfst.size-t-to-uint-fn]
> unsigned int size_t_to_uint(size_t value)

> [spec:hfst:sem:hfst-data-types.hfst.size-t-to-uint-fn]
> Converts a `size_t` to an `unsigned int`. If `value` is greater than
> `UINT_MAX`, throws `std::overflow_error` with the message "data is
> larger than UINT_MAX". Otherwise returns the value cast to
> `unsigned int` via `static_cast`.

> [spec:hfst:def:hfst-data-types.hfst.size-t-to-ushort-fn]
> unsigned short size_t_to_ushort(size_t value)

> [spec:hfst:sem:hfst-data-types.hfst.size-t-to-ushort-fn]
> Converts a `size_t` to an `unsigned short`. If `value` is greater than
> `USHRT_MAX`, throws `std::overflow_error` with the message "data is
> larger than USHRT_MAX". Otherwise returns the value cast to
> `unsigned short` via `static_cast`.

> [spec:hfst:def:hfst-data-types.hfst.string-pair]
> typedef std::pair<std::string, std::string> StringPair

> [spec:hfst:def:hfst-data-types.hfst.string-pair-set]
> typedef std::set<std::pair<std::string, std::string> > StringPairSet

> [spec:hfst:def:hfst-data-types.hfst.string-pair-vector]
> typedef std::vector<std::pair<std::string,std::string> > StringPairVector

> [spec:hfst:def:hfst-data-types.hfst.string-vector]
> typedef std::vector<std::string> StringVector

