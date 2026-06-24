# libhfst/src/implementations/HfstOlTransducer.cc, libhfst/src/implementations/HfstOlTransducer.h

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.extract-paths-fn]
> static bool extract_paths

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.extract-paths-fn]
> Free (file-static) recursive helper that does a depth-first traversal of the
> hfst_ol::Transducer `t` starting at transition-table index `s`, emitting paths
> via `callback`. Parameters: `all_visitations` (a per-state visit counter
> passed BY VALUE — each recursion gets its own copy), `path_visitations`
> (per-state counter on the current path, also passed BY VALUE), `weight_sum`
> (accumulated weight so far), `callback` (an ExtractStringsCb&), `cycles`
> (max allowed repeated visits of a state on one path, or negative for
> unlimited), `fd_state_stack` (pointer to a vector of FdState<SymbolNumber>;
> NULL when no flag-diacritic handling), `filter_fd` (whether flag-diacritic
> symbols are filtered out of the emitted strings), and `spv` (a
> StringPairVector& holding the current path's input/output symbol pairs,
> mutated and restored as the recursion proceeds). Steps:
> 1. If `cycles >= 0` and `path_visitations[s] > cycles`, return `true`
>    immediately (cycle limit reached for this state on this path).
> 2. Increment `all_visitations[s]` and `path_visitations[s]`.
> 3. If `spv` is non-empty (i.e. not at the very start): determine finality of
>    state `s`. If `indexes_transition_index_table(s)`, look at `t->get_index(s)`:
>    if `.final()`, mark final and, when the header's Weighted flag is set, set
>    final_weight from `TransitionWIndex::final_weight()` (else 0). Otherwise
>    look at `t->get_transition(s)`: if `.final()`, mark final and, when Weighted,
>    set final_weight from `TransitionW::get_weight()` (else 0). Build an
>    HfstTwoLevelPath from `weight_sum + final_weight` and `spv`, call
>    `callback(path, final)`. If the returned RetVal has `continueSearch` false
>    or `continuePath` false, decrement `path_visitations[s]` and return
>    `ret.continueSearch`.
> 4. Get the set of outgoing transitions `t->get_transitions_from_state(s)` and
>    build `sorted_transitions` ordered ascending by `all_visitations` of each
>    transition's target (insertion sort: least-visited targets first).
> 5. Iterate `sorted_transitions` while `res == true`. For each transition get
>    its input and output symbols. If `fd_state_stack` is non-NULL and the input
>    symbol is a flag-diacritic operation in the table: push a copy of the top
>    FdState, call `apply_operation(input)`; if it succeeds mark
>    `added_fd_state`, else pop the copy and `continue` (skip this transition).
> 6. Build `istring`/`ostring`: assert that `fd_state_stack != NULL || !filter_fd`.
>    If not filtering, or the input symbol is not a flag operation, set istring to
>    the alphabet symbol-table entry for `input` (else empty). Same for ostring
>    with `output`. Push `StringPair(istring, ostring)` onto `spv`.
> 7. Recurse into `extract_paths` on `transition.get_target()`, passing copies of
>    `all_visitations`/`path_visitations`, `weight_sum` plus (when Weighted) the
>    transition's `TransitionW::get_weight()` else 0, and the same callback,
>    cycles, fd_state_stack, filter_fd, spv. Store the result in `res`.
> 8. Pop the just-pushed entry off `spv`. If `added_fd_state`, pop the
>    fd_state_stack.
> 9. After the loop, decrement `path_visitations[s]` and return `res`.

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream]
> class HfstOlInputStream {
>   std::string filename;
>   ifstream i_stream;
>   istream &input_stream;
>   bool weighted;
> }

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.close-fn]
> void HfstOlInputStream::close(void)

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.close-fn]
> Closes the input stream. If `filename` is not the empty string (i.e. this
> stream was opened from a named file rather than stdin), calls `i_stream.close()`.
> If `filename` is empty, does nothing.

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.hfst-ol-input-stream-fn]
> HfstOlInputStream::HfstOlInputStream

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.hfst-ol-input-stream-fn]
> Constructor that opens an HfstOlInputStream from a named file. Parameters:
> `filename` (the path) and `weighted` (whether the transducer format is weighted).
> Initializes the `filename` member to a copy of `filename`, opens the member
> `i_stream` ifstream on `filename.c_str()` with mode `std::ios::in | std::ios::binary`,
> binds the `input_stream` reference to `i_stream`, and stores `weighted`.
> (Note: there are two other constructors not covered by this rule — a stdin
> constructor binding input_stream to std::cin, and one binding to an arbitrary
> std::istream&.)

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.ignore-fn]
> void HfstOlInputStream::ignore(unsigned int n)

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.ignore-fn]
> Discards `n` bytes from the underlying `input_stream` by calling
> `input_stream.ignore(n)`. No return value.

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.is-bad-fn]
> bool HfstOlInputStream::is_bad(void) const

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.is-bad-fn]
> Returns whether the stream is in a bad (unrecoverable error) state. If
> `filename` is empty (stdin case), returns `std::cin.bad()`. Otherwise returns
> `input_stream.bad()`.

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.is-eof-fn]
> bool HfstOlInputStream::is_eof(void) const

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.is-eof-fn]
> Returns true if the stream is at end-of-file, determined by peeking the next
> character (`input_stream.peek()`) and comparing it to `EOF`.

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.is-fst-fn]
> int HfstOlInputStream::is_fst(istream &s)

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.is-fst-fn]
> Static method that peeks at an istream `s` to classify its leading bytes as an
> HFST-OL transducer header without consuming them. Steps:
> 1. If `!s.good()`, return 0.
> 2. Read 24 bytes into a local `buffer`; record the actual count via
>    `s.gcount()` as `num_read`.
> 3. Interpret `*((int*)(buffer+20))` as an unsigned int `weighted` (the int at
>    byte offset 20).
> 4. Compute the result `res`: if `num_read != 24`, res = 0; else if
>    `weighted == 0`, res = 1; else if `weighted == 1`, res = 2; else res = 0.
> 5. If `num_read > 0`, put the read bytes back into `s` in reverse order
>    (`s.putback(buffer[i])` for i from `num_read-1` down to 0), restoring the
>    stream position.
> 6. If `num_read != 24`, call `s.clear()` to reset error flags.
> 7. Return `res` (0 = not an HFST-OL fst, 1 = unweighted, 2 = weighted).

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.is-good-fn]
> bool HfstOlInputStream::is_good(void) const

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.is-good-fn]
> Returns whether the stream is in a usable state. First, if `is_eof()` is true,
> returns false. Otherwise, if `filename` is empty (stdin case) returns
> `std::cin.good()`, else returns `input_stream.good()`.

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.is-open-fn]
> bool HfstOlInputStream::is_open(void) const

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.is-open-fn]
> Returns whether the stream is open. If `filename` is not the empty string
> (file-backed stream), returns `i_stream.is_open()`. If `filename` is empty
> (stdin), returns `true`.

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.open-fn]
> void HfstOlInputStream::open(void)

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.open-fn]
> No-op. The function body is empty (the stream is already opened in the
> constructor); does nothing.

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.operator-fn]
> bool HfstOlInputStream::operator() (void) const

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.operator-fn]
> Function-call operator returning the stream's usability: simply returns
> `is_good()`.

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.read-transducer-fn]
> hfst_ol::Transducer * HfstOlInputStream::read_transducer(bool has_header)

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.read-transducer-fn]
> Reads one hfst_ol::Transducer from the input stream. Parameter `has_header`
> indicates whether an HFST header precedes the transducer data. Steps:
> 1. If `is_eof()` is true, throw `StreamIsClosedException` (via HFST_THROW).
> 2. In a try block: if `has_header`, call `skip_hfst_header()` to consume the
>    header bytes. Then construct `new hfst_ol::Transducer(input_stream)`, reading
>    the transducer directly from the stream, and return that pointer.
> 3. The catch block catches `const HfstException e` and rethrows `e`.

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.skip-hfst-header-fn]
> void HfstOlInputStream::skip_hfst_header(void)

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.skip-hfst-header-fn]
> Skips the HFST header preceding the transducer. First calls
> `input_stream.ignore(6)` to discard 6 bytes (the leading HFST cookie/version
> bytes), then calls `skip_identifier_version_3_0()` to discard the type
> identifier string.

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.skip-identifier-version-3-0-fn]
> void HfstOlInputStream::skip_identifier_version_3_0(void)

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.skip-identifier-version-3-0-fn]
> Skips the type identifier string in the header by ignoring a fixed number of
> bytes from `input_stream`: 14 bytes if `weighted` is true (for "HFST_OLW_TYPE"),
> otherwise 13 bytes (for "HFST_OL_TYPE"). Calls
> `input_stream.ignore(weighted ? 14 : 13)`.

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.stream-get-fn]
> char HfstOlInputStream::stream_get()

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.stream-get-fn]
> Reads and returns a single byte from `input_stream` by calling
> `input_stream.get()` and casting the result to `char`.

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.stream-get-short-fn]
> short HfstOlInputStream::stream_get_short()

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.stream-get-short-fn]
> Reads a `short` from `input_stream` in raw binary form: declares a local
> `short i`, calls `input_stream.read((char*)&i, sizeof(i))` to read sizeof(short)
> bytes directly into it (native byte order), and returns `i`.

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.stream-unget-fn]
> void HfstOlInputStream::stream_unget(char c)

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-input-stream.stream-unget-fn]
> Pushes the character `c` back onto `input_stream` by calling
> `input_stream.putback(c)`, so the next read returns it again.

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-output-stream]
> class HfstOlOutputStream {
>   std::string filename;
>   ofstream o_stream;
>   ostream &output_stream;
>   bool weighted;
> }

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-output-stream.close-fn]
> void HfstOlOutputStream::close(void)

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-output-stream.close-fn]
> Closes the output stream. If `filename` is not the empty string (file-backed
> stream rather than stdout), calls `o_stream.close()`. If `filename` is empty,
> does nothing.

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-output-stream.hfst-ol-output-stream-fn]
> HfstOlOutputStream::HfstOlOutputStream(const std::string &str, bool weighted)

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-output-stream.hfst-ol-output-stream-fn]
> Constructor that opens an HfstOlOutputStream onto a named file. Parameters:
> `str` (the path) and `weighted`. Initializes `filename` to a copy of `str`,
> opens the member `o_stream` ofstream on `str.c_str()` with mode
> `std::ios::out | std::ios::binary`, binds the `output_stream` reference to
> `o_stream`, and stores `weighted`. After construction, if `!output_stream`
> (the stream is in a failed state), prints
> `"HfstOlOutputStream: ERROR: failbit set (3).\n"` to stderr.

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-output-stream.open-fn]
> void HfstOlOutputStream::open(void)

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-output-stream.open-fn]
> No-op. The function body is empty (the stream is already opened in the
> constructor); does nothing.

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-output-stream.write-fn]
> void HfstOlOutputStream::write(const char &c)

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-output-stream.write-fn]
> Writes a single character `c` to `output_stream` by calling
> `output_stream.put(char(c))`.

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-output-stream.write-transducer-fn]
> void HfstOlOutputStream::write_transducer(hfst_ol::Transducer * transducer)

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-output-stream.write-transducer-fn]
> Writes the given `transducer` to the output stream. First, if `!output_stream`
> (failed state), prints `"HfstOlOutputStream: ERROR: failbit set (1).\n"` to
> stderr. Then calls `transducer->write(output_stream)` to serialize the
> transducer onto the stream.

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-transducer]
> class HfstOlTransducer

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-transducer.create-empty-transducer-fn]
> hfst_ol::Transducer * HfstOlTransducer::create_empty_transducer(bool weighted)

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-transducer.create-empty-transducer-fn]
> Allocates and returns a new empty `hfst_ol::Transducer`, constructed with the
> `weighted` flag: `new hfst_ol::Transducer(weighted)`.

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-transducer.extract-paths-fn]
> void HfstOlTransducer::extract_paths

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-transducer.extract-paths-fn]
> Public entry point that sets up state and invokes the recursive free
> `extract_paths` helper. Parameters: `t` (the transducer), `callback`
> (ExtractStringsCb&), `cycles` (max repeated state visits per path), `fd`
> (a FdTable<SymbolNumber>* for flag diacritics, or NULL), `filter_fd`. Steps:
> 1. Create empty maps `all_visitations` and `path_visitations`
>    (TransitionTableIndex -> unsigned short).
> 2. If `fd == NULL`, set `fd_state_stack` to NULL; otherwise allocate a new
>    `std::vector<FdState<SymbolNumber>>` initialized with one element
>    `FdState<SymbolNumber>(*fd)`.
> 3. Create an empty `StringPairVector spv`.
> 4. Call the free helper `hfst::implementations::extract_paths(t, 0,
>    all_visitations, path_visitations, 0.0f, callback, cycles, fd_state_stack,
>    filter_fd, spv)` — starting at index 0, with weight_sum 0.0.
> Returns void. (Note: the allocated fd_state_stack is not freed here.)

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-transducer.get-alphabet-fn]
> StringSet HfstOlTransducer::get_alphabet(hfst_ol::Transducer * t)

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-transducer.get-alphabet-fn]
> Returns the set of all symbols in transducer `t`. Gets the symbol table via
> `t->get_alphabet().get_symbol_table()` (a SymbolTable, i.e. an ordered
> collection of symbol strings indexed by SymbolNumber), and constructs a
> `StringSet` from its begin/end iterators (deduplicating into a set).

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-transducer.get-flag-diacritics-fn]
> const FdTable<hfst_ol::SymbolNumber>* HfstOlTransducer

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-transducer.get-flag-diacritics-fn]
> Returns a pointer to the flag-diacritic table of transducer `t`: the address
> of `t->get_alphabet().get_fd_table()` (a const FdTable<SymbolNumber>&). The
> returned pointer borrows the table owned by the transducer's alphabet.

> [spec:hfst:def:hfst-ol-transducer.hfst.implementations.hfst-ol-transducer.is-cyclic-fn]
> bool HfstOlTransducer::is_cyclic(hfst_ol::Transducer* t)

> [spec:hfst:sem:hfst-ol-transducer.hfst.implementations.hfst-ol-transducer.is-cyclic-fn]
> Returns whether transducer `t` is cyclic by reading its header's Cyclic flag:
> `t->get_header().probe_flag(hfst_ol::Cyclic)`.

> [spec:hfst:def:hfst-ol-transducer.main-fn]
> int main(int argc, char * argv[])

> [spec:hfst:sem:hfst-ol-transducer.main-fn]
> Unit-test stub compiled only when MAIN_TEST is defined. Prints
> `"Unit tests for " __FILE__ ":"` followed by a newline and `"ok"` and another
> newline to std::cout, then returns EXIT_SUCCESS. Performs no actual testing.

