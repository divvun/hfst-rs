# libhfst/src/c/libhfst_c.cpp, libhfst/src/c/libhfst_c.h

> [spec:hfst:def:libhfst-c.hfst-empty-transducer-fn]
> void *

> [spec:hfst:sem:libhfst-c.hfst-empty-transducer-fn]
> Takes no parameters. Heap-allocates a new default-constructed
> `hfst::HfstTransducer` (an empty automaton) and returns it as an opaque
> `void *`. Ownership transfers to the caller; nothing is freed here. No
> error handling.

> [spec:hfst:def:libhfst-c.hfst-empty-transducer-t]
> typedef void *hfst_empty_transducer_t()

> [spec:hfst:def:libhfst-c.hfst-input-stream-close-fn]
> void

> [spec:hfst:sem:libhfst-c.hfst-input-stream-close-fn]
> Takes the opaque `void *his`, casts it to `hfst::HfstInputStream *`, and
> calls its `close()` method. Does not delete the object or check for null.
> Returns nothing.

> [spec:hfst:def:libhfst-c.hfst-input-stream-close-t-void]
> typedef void* hfst_input_stream_close_t(void*)

> [spec:hfst:def:libhfst-c.hfst-input-stream-fn]
> void *

> [spec:hfst:sem:libhfst-c.hfst-input-stream-fn]
> Takes a C string `filename`. Initializes a local pointer to null, then in a
> try block heap-allocates a new `hfst::HfstInputStream(filename)`, opening the
> named file. Wraps the construction in a catch-all (`catch (...)`) that
> swallows every C++ exception and does nothing, so no exception can ever
> propagate to the C/Rust caller. Returns the pointer as an opaque `void *`;
> on failure the pointer remains null and null is returned. Caller owns the
> returned object.

> [spec:hfst:def:libhfst-c.hfst-input-stream-free-fn]
> void

> [spec:hfst:sem:libhfst-c.hfst-input-stream-free-fn]
> Takes the opaque `void *input_stream`. Asserts it is non-null, then casts it
> to `hfst::HfstInputStream *` and `delete`s it, freeing the object. Sets the
> local copy of the pointer to null (which has no effect on the caller's
> pointer). Returns nothing.

> [spec:hfst:def:libhfst-c.hfst-input-stream-is-bad-fn]
> bool

> [spec:hfst:sem:libhfst-c.hfst-input-stream-is-bad-fn]
> Takes the opaque `void *his`, casts it to `hfst::HfstInputStream *`, and
> returns the result of calling its `is_bad()` method (the stream's bad/error
> flag) as a bool. No null check.

> [spec:hfst:def:libhfst-c.hfst-input-stream-is-bad-t-void]
> typedef bool hfst_input_stream_is_bad_t(void*)

> [spec:hfst:def:libhfst-c.hfst-input-stream-is-eof-fn]
> bool

> [spec:hfst:sem:libhfst-c.hfst-input-stream-is-eof-fn]
> Takes the opaque `void *his`, casts it to `hfst::HfstInputStream *`, and
> returns the result of calling its `is_eof()` method (the stream's
> end-of-file flag) as a bool. No null check.

> [spec:hfst:def:libhfst-c.hfst-input-stream-is-eof-t-void]
> typedef bool hfst_input_stream_is_eof_t(void*)

> [spec:hfst:def:libhfst-c.hfst-input-stream-t-const-char-filename]
> typedef void *hfst_input_stream_t(const char *filename)

> [spec:hfst:def:libhfst-c.hfst-lookup-begin-fn]
> EXTERN void *hfst_lookup_begin(const void *)

> [spec:hfst:sem:libhfst-c.hfst-lookup-begin-fn]
> Declared in the header as `EXTERN void *hfst_lookup_begin(const void *)` but
> has NO definition anywhere in the codebase (no body in libhfst_c.cpp or
> elsewhere). It is an unused forward declaration with no behavior to port.

> [spec:hfst:def:libhfst-c.hfst-lookup-begin-t-const-void]
> typedef void *hfst_lookup_begin_t(const void *)

> [spec:hfst:def:libhfst-c.hfst-lookup-fn]
> void *

> [spec:hfst:sem:libhfst-c.hfst-lookup-fn]
> Takes the opaque `void *self` and a C string `s`. Casts `self` to
> `hfst::HfstTransducer *`, then calls its `lookup(s)` method, which returns a
> heap-allocated `hfst::HfstOneLevelPaths *` (the set of weighted output paths
> for input `s`). Returns that pointer as an opaque `void *`. Caller owns the
> returned paths object. No null check or error handling.

> [spec:hfst:def:libhfst-c.hfst-lookup-iterator-done-fn]
> bool

> [spec:hfst:sem:libhfst-c.hfst-lookup-iterator-done-fn]
> Takes a `ResultIterator *it`. Casts `it->begin` and `it->end` to
> `hfst::HfstOneLevelPaths::iterator *` and returns whether the two iterators
> compare equal (`*begin == *end`), i.e. true when the current position has
> reached the end and iteration is done. Reads only; mutates nothing.

> [spec:hfst:def:libhfst-c.hfst-lookup-iterator-done-t-struct-result-iterator-it]
> typedef bool hfst_lookup_iterator_done_t(struct ResultIterator *it)

> [spec:hfst:def:libhfst-c.hfst-lookup-iterator-fn]
> struct ResultIterator *

> [spec:hfst:sem:libhfst-c.hfst-lookup-iterator-fn]
> Takes the opaque `void *holps` and casts it to `hfst::HfstOneLevelPaths *`
> (`v`). `malloc`s a `ResultIterator` struct. Sets `s->begin` to a newly
> heap-allocated (`new`) copy of `v->begin()` and `s->end` to a newly
> heap-allocated copy of `v->end()`, each stored as a `void *` pointing to an
> `hfst::HfstOneLevelPaths::iterator`. Returns the `ResultIterator *`. The
> caller owns all three allocations (the struct via `malloc`, the two iterators
> via `new`) and must release them with the iterator-free function.

> [spec:hfst:def:libhfst-c.hfst-lookup-iterator-free-fn]
> void

> [spec:hfst:sem:libhfst-c.hfst-lookup-iterator-free-fn]
> Takes a `ResultIterator *it`. `delete`s the `hfst::HfstOneLevelPaths::iterator`
> pointed to by `it->begin`, `delete`s the one pointed to by `it->end`, then
> `free`s the `ResultIterator` struct itself (matching the `new`/`new`/`malloc`
> allocations made by the iterator-creation function). Returns nothing.

> [spec:hfst:def:libhfst-c.hfst-lookup-iterator-free-t-result-iterator-it]
> typedef void hfst_lookup_iterator_free_t(ResultIterator *it)

> [spec:hfst:def:libhfst-c.hfst-lookup-iterator-next-fn]
> void

> [spec:hfst:sem:libhfst-c.hfst-lookup-iterator-next-fn]
> Takes a `ResultIterator *it`. Casts `it->begin` to
> `hfst::HfstOneLevelPaths::iterator *` and post-increments the underlying
> iterator (`(*begin)++`), advancing the current position by one element.
> Mutates the begin iterator in place; `it->end` is untouched. Returns nothing.

> [spec:hfst:def:libhfst-c.hfst-lookup-iterator-next-t-result-iterator]
> typedef void hfst_lookup_iterator_next_t(ResultIterator *)

> [spec:hfst:def:libhfst-c.hfst-lookup-iterator-t-void]
> typedef ResultIterator *hfst_lookup_iterator_t(void *)

> [spec:hfst:def:libhfst-c.hfst-lookup-iterator-value-fn]
> void

> [spec:hfst:sem:libhfst-c.hfst-lookup-iterator-value-fn]
> Takes a `ResultIterator *it`, an out-param `char **s`, and an out-param
> `float *weight`. Casts `it->begin` to `hfst::HfstOneLevelPaths::iterator *`
> and dereferences it twice to get the current `HfstOneLevelPath` pair
> (`pair`). Writes `pair.first` (the path weight) into `*weight`. Builds the
> output string by concatenating every element of `pair.second` (the
> `StringVector`) with no separator into a `std::ostringstream` (via
> `std::copy` into an `ostream_iterator<std::string>`), then takes
> `os.str()` into a local `std::string full`. `malloc`s `full.size() + 1`
> bytes, `strncpy`s `full.size()` bytes of `full` into it, sets the trailing
> byte to NUL, and writes the pointer into `*s`. The caller owns and must
> `free` the returned string. Mutates nothing in the iterator.

> [spec:hfst:def:libhfst-c.hfst-lookup-iterator-value-t-result-iterator-it-char-s-float-weight]
> typedef void hfst_lookup_iterator_value_t(ResultIterator *it, char **s, float *weight)

> [spec:hfst:def:libhfst-c.hfst-lookup-results-fn]
> size_t

> [spec:hfst:sem:libhfst-c.hfst-lookup-results-fn]
> Takes the opaque `void *holps`, an out-array `void **results`, and an
> out-array `float *weights` (both assumed pre-allocated by the caller with
> enough slots for every path). Casts `holps` to `hfst::HfstOneLevelPaths *`
> (`v`). Initializes `i = 0` and iterates each path `it` in `*v`. For each: takes
> `it.second` (the `StringVector`), `malloc`s a fixed 256-byte char buffer,
> sets its first byte to NUL, then `strcat`s every string element of the
> vector onto the buffer in order (no separator). Stores the buffer pointer in
> `results[i]` and `it.first` (the weight) in `weights[i]`, then increments
> `i`. After the loop returns `i`, the number of paths written. Note: the
> 256-byte buffer is a fixed size with no bounds checking (overflow if the
> concatenation exceeds 255 chars), and each `malloc`ed buffer becomes the
> caller's responsibility to free.

> [spec:hfst:def:libhfst-c.hfst-lookup-results-t-void-char-float]
> typedef size_t hfst_lookup_results_t(void*, char**, float*)

> [spec:hfst:def:libhfst-c.hfst-lookup-t-void-const-char]
> typedef void* hfst_lookup_t(void *, const char *)

> [spec:hfst:def:libhfst-c.hfst-transducer-from-stream-fn]
> void *

> [spec:hfst:sem:libhfst-c.hfst-transducer-from-stream-fn]
> Takes the opaque `void *his`, casts it to `hfst::HfstInputStream *` (`inp`),
> heap-allocates a new `hfst::HfstTransducer(*inp)` by reading the next
> transducer from the stream, and returns it as an opaque `void *`. Caller owns
> the returned transducer. No null check or error handling (any C++ exception
> from the constructor would propagate).

> [spec:hfst:def:libhfst-c.hfst-transducer-from-stream-t-void]
> typedef void* hfst_transducer_from_stream_t(void*)

> [spec:hfst:def:libhfst-c.hfst-value]
> typedef struct HfstValue

> [spec:hfst:def:libhfst-c.hfst.hfst-one-level-path]
> typedef std::pair<float, StringVector> HfstOneLevelPath

> [spec:hfst:def:libhfst-c.hfst.hfst-one-level-paths]
> typedef std::set<HfstOneLevelPath> HfstOneLevelPaths

> [spec:hfst:def:libhfst-c.hfst.string-vector]
> typedef std::vector<std::string> StringVector

> [spec:hfst:def:libhfst-c.result-iterator]
> typedef struct ResultIterator

