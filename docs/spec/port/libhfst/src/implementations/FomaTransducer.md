# libhfst/src/implementations/FomaTransducer.cc, libhfst/src/implementations/FomaTransducer.h

> [spec:hfst:def:foma-transducer.hfst.implementations.explode-line-fn]
> static inline int explode_line (char *buf, int *values)

> [spec:hfst:sem:foma-transducer.hfst.implementations.explode-line-fn]
> Parses a space-separated line of integers from `buf` into the `values` array, returning the count parsed.
> Initialize `i = j = items = 0`. Loop forever: starting at `i = j`, advance `j` while `buf[j]` is neither a space nor `'\0'`.
> If `buf[j] == '\0'`: store `atoi(buf+i)` into `values[items]`, increment `items`, then break out of the loop.
> Otherwise `buf[j]` is a space: overwrite it with `'\0'`, store `atoi(buf+i)` into `values[items]`, increment `items`, increment `j` past the now-null, and set `i = j` to start the next token.
> Returns `items`, the number of integers written. Note: it mutates `buf` in place (each space becomes a null terminator). Caller must ensure `values` is large enough.

> [spec:hfst:def:foma-transducer.hfst.implementations.extract-paths-fn]
> static bool extract_paths

> [spec:hfst:sem:foma-transducer.hfst.implementations.extract-paths-fn]
> Recursive depth-first path enumeration over foma net `t` starting at `state`, invoking `callback` for each path. Parameters: `all_visitations` and `path_visitations` are maps state->visit-count passed BY VALUE (each recursive call gets its own copies, so per-call mutations don't propagate back to the caller, but are shared down the recursion via copy); `cycles` is a cycle bound; `fd_state_stack` is an optional flag-diacritic state stack; `filter_fd` controls whether flag symbols are emitted; `spv` is the StringPairVector accumulating the current path (passed by reference).
> Step 1: if `cycles >= 0` and `path_visitations[state] > cycles`, return true (cycle limit reached for this state on the current path).
> Step 2: increment `all_visitations[state]` and `path_visitations[state]`.
> Step 3: if `spv` is non-empty, determine finality by scanning `t->states` array (entries terminated by `state_no == -1`) for an entry whose `state_no == state` and `final_state == 1`. Build an `HfstTwoLevelPath` with weight 0.0 and the current `spv`, call `callback(path, final)`. If the returned `ret.continueSearch` is false or `ret.continuePath` is false, decrement `path_visitations[state]` and return `ret.continueSearch`.
> Step 4: collect outgoing arcs: scan `t->states`; for each entry with `state_no == state` and `target != -1`, insert it into `sorted_arcs` keeping the vector sorted ascending by `all_visitations[arc->target]` (insertion sort: find position `j`, shift, insert). This visits least-visited targets first.
> Step 5: iterate `sorted_arcs` while `res == true`. For each arc: if `fd_state_stack` is set and the arc's input symbol (`arc->in`) has a flag operation in the table, push a copy of the top fd-state and apply the operation; if apply succeeds set `added_fd_state = true`, else pop and `continue` (skip this arc). Look up the input symbol string `c_in` by scanning `t->sigma` for `sig->number == arc->in`, and the output string `c_out` for `arc->out`. If `!filter_fd` (or fd present but arc->in has no flag op), assert `c_in != NULL` and set `istring = strdup(c_in)`; similarly for `ostring`/`c_out`. Push `StringPair(istring, ostring)` onto `spv`. Recurse into `extract_paths(t, arc->target, ...)`, storing result into `res`. Pop the StringPair off `spv`. If `added_fd_state`, pop the fd-state stack.
> Step 6: decrement `path_visitations[state]` and return `res`. Returns false to abort the whole search early, true otherwise.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-input-stream]
> class FomaInputStream {
>   std::string filename;
>   FILE * input_file;
> }

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-input-stream.close-fn]
> void FomaInputStream::close(void)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-input-stream.close-fn]
> Closes the input stream. If `input_file` is NULL, return immediately. Otherwise, if the first character of `filename` is not `'\0'` (i.e. a real file was opened, not stdin), call `fclose(input_file)` and set `input_file = NULL`. Streams reading from stdin (empty filename) are left open.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-input-stream.foma-input-stream-fn]
> FomaInputStream::FomaInputStream(const std::string &filename_)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-input-stream.foma-input-stream-fn]
> Constructor that opens an input stream from `filename_`. Stores `filename = filename_`. If `filename` is empty, set `input_file = stdin`. Otherwise open it via `hfst::hfst_fopen(filename.c_str(), "r")`; if the result is NULL, throw `StreamNotReadableException`.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-input-stream.ignore-fn]
> void FomaInputStream::ignore(unsigned int n)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-input-stream.ignore-fn]
> Discards the next `n` bytes from the stream by calling `fgetc(input_file)` `n` times in a loop (return values ignored). No bounds/EOF checking.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-input-stream.is-bad-fn]
> bool FomaInputStream::is_bad(void)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-input-stream.is-bad-fn]
> Returns the result of `is_eof()`; the stream is considered "bad" exactly when it is at end of file.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-input-stream.is-eof-fn]
> bool FomaInputStream::is_eof(void)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-input-stream.is-eof-fn]
> Non-destructively tests for end of file. Read one character with `getc(input_file)`, capture `retval = (feof(input_file) != 0)`, then push the character back with `ungetc(c, input_file)`. Return `retval`. The peeked character is restored so the stream position is unchanged.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-input-stream.is-fst-fn]
> bool FomaInputStream::is_fst(FILE * f)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-input-stream.is-fst-fn]
> Tests whether FILE `f` begins with a foma transducer. If `f` is NULL, return false. Otherwise peek the next byte with `getc(f)` then push it back with `ungetc(c, f)` (stream position unchanged). Return true iff `c == 31` (the 0x1F gzip/foma magic byte) or `c == '#'`.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-input-stream.is-good-fn]
> bool FomaInputStream::is_good(void)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-input-stream.is-good-fn]
> Returns the logical negation of `is_bad()`; the stream is good when not at EOF.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-input-stream.read-transducer-fn]
> fsm * FomaInputStream::read_transducer()

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-input-stream.read-transducer-fn]
> Reads one foma transducer from the stream. If `is_eof()` is true, return NULL. Otherwise call `FomaTransducer::read_net(input_file)`; if it returns NULL, throw `NotTransducerStreamException`. Otherwise return the resulting `fsm *`.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-input-stream.skip-hfst-header-fn]
> void FomaInputStream::skip_hfst_header(void)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-input-stream.skip-hfst-header-fn]
> Reads and discards the 6-byte HFST header. `fread` 6 bytes (one item of size 6) into `hfst_header`; convert the returned item count with `hfst::size_t_to_int`. If the count is not 1, throw `NotTransducerStreamException`. Then call `skip_identifier_version_3_0()` inside a try block; any caught `HfstException` is re-thrown.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-input-stream.skip-identifier-version-3-0-fn]
> void FomaInputStream::skip_identifier_version_3_0(void)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-input-stream.skip-identifier-version-3-0-fn]
> Reads and verifies the 10-byte type identifier. Declare `char foma_identifier[10]`. `fread` 10 bytes (one item of size 10) into it; convert the returned item count via `hfst::size_t_to_int`. If the count is not 1, throw `NotTransducerStreamException`. Then compare with `strcmp(foma_identifier, "FOMA_TYPE")`; if not equal (nonzero), throw `NotTransducerStreamException`. (Note: the buffer is exactly 10 bytes and "FOMA_TYPE" plus its terminating NUL is exactly 10 bytes.)

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-input-stream.stream-get-fn]
> char FomaInputStream::stream_get()

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-input-stream.stream-get-fn]
> Reads and returns the next byte from `input_file` as a `char`, via `(char) fgetc(input_file)`. Advances the stream by one byte.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-input-stream.stream-get-short-fn]
> short FomaInputStream::stream_get_short()

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-input-stream.stream-get-short-fn]
> Reads a `short` (in host byte order / raw memory layout) from the stream. Declare `short i = 0`, then `fread(&i, sizeof(short), 1, input_file)`; assert that exactly 1 item was read. Return `i`. Advances the stream by `sizeof(short)` bytes.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-input-stream.stream-unget-fn]
> void FomaInputStream::stream_unget(char c)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-input-stream.stream-unget-fn]
> Pushes character `c` back onto the input stream via `ungetc((int)c, input_file)`, so the next read returns it. Return value of `ungetc` is ignored.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-output-stream]
> class FomaOutputStream {
>   std::string filename;
>   FILE *ofile;
> }

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-output-stream.close-fn]
> void FomaOutputStream::close(void)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-output-stream.close-fn]
> Closes the output stream. Only if `filename` is non-empty (a real file, not stdout) does it call `fclose(ofile)`. When writing to stdout (empty filename) it does nothing.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-output-stream.foma-output-stream-fn]
> FomaOutputStream::FomaOutputStream(const std::string &str)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-output-stream.foma-output-stream-fn]
> Constructor opening an output stream to `str`. Stores `filename = str`. If `filename` is non-empty, open it via `hfst::hfst_fopen(filename.c_str(), "wb")` (binary write); if the result is NULL, throw `StreamNotReadableException`. If `filename` is empty, set `ofile = stdout`.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-output-stream.write-fn]
> void FomaOutputStream::write(const char &c)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-output-stream.write-fn]
> Writes a single character `c` to the output stream via `fputc(c, ofile)`. Return value ignored.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-output-stream.write-transducer-fn]
> void FomaOutputStream::write_transducer(fsm * transducer)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-output-stream.write-transducer-fn]
> Writes `transducer` to the output file by calling `FomaTransducer::write_net(transducer, ofile)`. If the return value is not 1 (success), throw `HfstFatalException` with the message "an error happened when writing a foma transducer".

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer]
> class FomaTransducer

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.are-equivalent-fn]
> bool FomaTransducer::are_equivalent

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.are-equivalent-fn]
> Tests whether nets `t1` and `t2` accept the same language by symmetric difference. Build `test = fsm_union(fsm_minus(copy(t1), copy(t2)), fsm_minus(copy(t2), copy(t1)))` (each operand is a fresh `fsm_copy` so the originals are not consumed). Compute `eq = fsm_isempty(test)`, then `fsm_destroy(test)` to free it. Return `eq == 1` (true iff the symmetric difference is empty, i.e. the two are equivalent).

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.compose-fn]
> fsm * FomaTransducer::compose

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.compose-fn]
> Returns the composition of `t1` and `t2` as `fsm_compose(fsm_copy(t1), fsm_copy(t2))`. Both arguments are copied first (foma consumes its operands), so the originals are left intact.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.concatenate-fn]
> fsm * FomaTransducer::concatenate

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.concatenate-fn]
> Returns the concatenation of `t1` and `t2` as `fsm_concat(fsm_copy(t1), fsm_copy(t2))`. Both arguments are copied first so the originals are left intact.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.copy-fn]
> fsm * FomaTransducer::copy(fsm * t)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.copy-fn]
> Returns a deep copy of net `t` via `fsm_copy(t)`. The original `t` is unmodified.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.create-empty-transducer-fn]
> fsm * FomaTransducer::create_empty_transducer(void)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.create-empty-transducer-fn]
> Returns a new net recognizing the empty language (empty set, accepting nothing) via `fsm_empty_set()`.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.create-epsilon-transducer-fn]
> fsm * FomaTransducer::create_epsilon_transducer(void)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.create-epsilon-transducer-fn]
> Returns a new net recognizing only the empty string (epsilon) via `fsm_empty_string()`.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.define-transducer-fn]
> fsm * FomaTransducer::define_transducer(const StringPairVector &spv)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.define-transducer-fn]
> Builds a linear (single-path) net from the StringPairVector `spv`, where each pair is one transition's (input, output) symbols.
> If `spv` is empty, return `fsm_empty_string()` immediately.
> Otherwise initialize `state_number = 0`. Create a construction handle `h = fsm_construct_init(empty)` where `empty` is a strdup'd empty string (then free it). For each pair in `spv` in order: `strdup` the first (input) and second (output) symbol strings, call `fsm_construct_add_arc(h, state_number, state_number+1, in, out)`, free both strings, then increment `state_number`. This produces a chain of states 0,1,2,...,N.
> Set initial state via `fsm_construct_set_initial(h, 0)` and final state via `fsm_construct_set_final(h, state_number)` (the last state). Finish with `net = fsm_construct_done(h)` and `fsm_count(net)`.
> Add the three foma special symbols to the net's sigma: `sigma_add_special(0, net->sigma)`, `sigma_add_special(1, net->sigma)`, `sigma_add_special(2, net->sigma)`. Return `net`.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.delete-foma-fn]
> void FomaTransducer::delete_foma(fsm * net)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.delete-foma-fn]
> Frees the net `net` by calling `fsm_destroy(net)`.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.determinize-fn]
> fsm * FomaTransducer::determinize(fsm * t)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.determinize-fn]
> Returns a determinized copy of `t`: `fsm_determinize(fsm_copy(t))`. Because foma's `fsm_determinize` mutates and returns its argument, a copy is taken first so the original `t` is preserved.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.disjunct-fn]
> fsm * FomaTransducer::disjunct

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.disjunct-fn]
> Returns the union (disjunction) of `t1` and `t2` as `fsm_union(fsm_copy(t1), fsm_copy(t2))`. Both arguments are copied first so the originals are left intact.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.eliminate-flag-fn]
> fsm * FomaTransducer::eliminate_flag(fsm * t, const std::string & flag)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.eliminate-flag-fn]
> Eliminates a single named flag diacritic from net `t`. `strdup` the `flag` string into `flag_`, call `retval = flag_eliminate(t, flag_)`, free `flag_`, and return `retval`. (Unlike copy-taking operations, `t` is passed directly to `flag_eliminate`.)

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.eliminate-flags-fn]
> fsm * FomaTransducer::eliminate_flags(fsm * t)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.eliminate-flags-fn]
> Eliminates all flag diacritics from net `t` by calling `flag_eliminate(t, NULL)` and returning its result. Passing NULL as the flag name tells foma to eliminate every flag, not a single named one. `t` is passed directly (no copy taken).

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.extract-input-language-fn]
> fsm * FomaTransducer::extract_input_language(fsm * t)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.extract-input-language-fn]
> Returns the input (upper) projection of `t` as `fsm_upper(fsm_copy(t))`. A copy is taken first so the original `t` is preserved. (Source note: foma does not handle the epsilon transducer properly here.)

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.extract-output-language-fn]
> fsm * FomaTransducer::extract_output_language(fsm * t)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.extract-output-language-fn]
> Returns the output (lower) projection of `t` as `fsm_lower(fsm_copy(t))`. A copy is taken first so the original `t` is preserved. (Source note: foma does not handle the epsilon transducer properly here.)

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.extract-paths-fn]
> void FomaTransducer::extract_paths

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.extract-paths-fn]
> Public driver that enumerates all paths of net `t`, invoking `callback` for each. Parameters: `cycles` (cycle bound passed through), `fd` (an optional `FdTable<int>*` of flag diacritics), `filter_fd` (whether to suppress flag symbols).
> Initialize two empty maps `all_visitations` and `path_visitations` (state -> visit count). Build `fd_state_stack`: if `fd == NULL`, set it to NULL; otherwise allocate a new `std::vector<FdState<int>>` initialized with one element `FdState<int>(*fd)`.
> Create an empty `std::set<int> initial_states`, an empty `StringPairVector spv`, and `res = true`.
> Scan `t->states` (entries terminated by `state_no == -1`) while `res == true`. For each entry whose `start_state == 1` and whose `state_no` is not already in `initial_states`: insert that `state_no` into `initial_states`, then call the static recursive `extract_paths(t, state_no, all_visitations, path_visitations, callback, cycles, fd_state_stack, filter_fd, spv)` (maps passed by value, spv by reference) and store the result in `res`.
> After the loop, add an epsilon path if any initial state is also final: for each state in `initial_states`, scan `t->states` for an entry with that `state_no` and `final_state == 1`; if found, build an empty `StringPairVector`, wrap it in an `HfstTwoLevelPath` with weight 0.0, and call `callback(epsilon_path, true)`.
> Returns void. Note: `fd_state_stack` is allocated with `new` and is not freed here (leaked).

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.extract-random-paths-fn]
> void FomaTransducer::extract_random_paths

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.extract-random-paths-fn]
> Not implemented. Ignores all three parameters (`t`, `results`, `max_num`) and unconditionally throws `FunctionNotImplementedException` (via `HFST_THROW`).

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.foma-transducer-fn]
> FomaTransducer::FomaTransducer()

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.foma-transducer-fn]
> Default constructor. Sets a local `_Bool val = 1` and calls `fsm_set_option(FSMO_SKIP_WORD_BOUNDARY_MARKER, &val)` to enable foma's option to skip the word boundary marker. No other state.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.get-alphabet-fn]
> StringSet FomaTransducer::get_alphabet(fsm *t)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.get-alphabet-fn]
> Builds and returns the `StringSet` alphabet of net `t`. Create an empty `StringSet alpha`. Walk the sigma linked list (`p = t->sigma`, follow `p->next` while `p != NULL`); if `p->symbol == NULL`, break; otherwise insert `std::string(p->symbol)` into `alpha`.
> After the loop, always insert the three internal special symbols `internal_epsilon`, `internal_unknown`, and `internal_identity` (since foma may not list them in sigma even though it tracks them). Return `alpha`.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.get-biggest-symbol-number-fn]
> unsigned int FomaTransducer::get_biggest_symbol_number(fsm * t)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.get-biggest-symbol-number-fn]
> Returns the largest symbol number used by net `t`. Initialize `biggest_number = 0`. Walk the sigma linked list (`p = t->sigma`, follow `p->next`); if `p->symbol == NULL`, break; otherwise if `biggest_number < (unsigned int)p->number`, set `biggest_number = p->number`.
> After the loop, since epsilon (0), unknown (1) and identity (2) are always considered present, if `biggest_number < 2` return 2; otherwise return `biggest_number`.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.get-flag-diacritics-fn]
> FdTable<int>* FomaTransducer::get_flag_diacritics(fsm * t)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.get-flag-diacritics-fn]
> Builds and returns a newly allocated `FdTable<int>*` of the flag diacritics in net `t`. Allocate `table = new FdTable<int>()`. Walk the sigma linked list (`p = t->sigma`, follow `p->next`); if `p->symbol == NULL`, break; otherwise if `FdOperation::is_diacritic(p->symbol)` is true, call `table->define_diacritic(p->number, p->symbol)`. Return `table` (caller owns it).

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.get-profile-seconds-fn]
> float FomaTransducer::get_profile_seconds()

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.get-profile-seconds-fn]
> Returns the global `foma_seconds` value (a profiling timer). No arguments, no side effects.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.get-symbol-map-fn]
> std::map<std::string, unsigned int> FomaTransducer::get_symbol_map

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.get-symbol-map-fn]
> Builds and returns a `std::map<std::string, unsigned int>` mapping each alphabet symbol to its symbol number. Call `get_alphabet(t)` to obtain the `StringSet alphabet`. Create an empty map. For each symbol `*it` in `alphabet`, set `symbol_map[*it] = get_symbol_number(t, it->c_str())`. Return the map.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.get-symbol-number-fn]
> unsigned int FomaTransducer::get_symbol_number

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.get-symbol-number-fn]
> Returns the symbol number for `symbol` in net `t`. Special-case first: if `symbol == internal_epsilon` return 0; if `symbol == internal_unknown` return 1; if `symbol == internal_identity` return 2.
> Otherwise let `c = symbol.c_str()` and walk the sigma linked list (`p = t->sigma`, follow `p->next`); if `p->symbol == NULL`, break; if `strcmp(p->symbol, c) == 0`, return `(unsigned int)p->number`.
> If no match is found, throw `SymbolNotFoundException` (via `HFST_THROW`).

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.get-symbol-vector-fn]
> StringVector FomaTransducer::get_symbol_vector

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.get-symbol-vector-fn]
> Returns a `StringVector` indexed by symbol number, where each slot holds the corresponding symbol string (and unused slots remain empty). Compute `biggest_symbol_number = get_biggest_symbol_number(t)`. Create `symbol_vector`, reserve `biggest_symbol_number+1`, and resize it to `biggest_symbol_number+1` filled with empty strings "".
> Get `alphabet = get_alphabet(t)`. For each symbol `*it` in `alphabet`, compute `symbol_number = get_symbol_number(t, *it)` and set `symbol_vector.at(symbol_number) = *it`. Return `symbol_vector`.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.harmonize-fn]
> void FomaTransducer::harmonize(fsm *net1, fsm *net2)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.harmonize-fn]
> Harmonizes the alphabets of `net1` and `net2` by calling `fsm_merge_sigma(net1, net2)`. Returns void; mutates both nets' sigmas in place.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.insert-freely-fn]
> fsm * FomaTransducer::insert_freely(fsm * t, const StringPair &symbol_pair)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.insert-freely-fn]
> Freely inserts the symbol pair `symbol_pair` (first=input symbol, second=output symbol) into net `t`. Steps:
> Let `epsilon = internal_epsilon.c_str()` and `identity = internal_identity.c_str()`; `strdup` a marker string `"@_EPSILON_SYMBOL_MARKER_@"` into `epsilon_marker`.
> `eps_marked = fsm_substitute_symbol(t, epsilon, epsilon_marker)` — replace existing epsilons in `t` with the marker so genuine epsilons are protected.
> Build `ins = fsm_kleene_star(fsm_union(fsm_symbol(identity), fsm_cross_product(fsm_symbol(epsilon), fsm_symbol(symbol_pair.second))))` — the free-insertion machine that maps identity to identity and epsilon to the output symbol, repeated any number of times.
> Compose and restore: `comp = fsm_substitute_symbol(fsm_compose(eps_marked, ins), epsilon, symbol_pair.first)` — substitute the inserted epsilons (on the input side) with `symbol_pair.first`.
> Return `fsm_substitute_symbol(comp, epsilon_marker, epsilon)` — restore the protected markers back to real epsilons.
> Note: the trailing `free(epsilon_marker)` is dead code (after the return) so the strdup'd marker leaks; and the marker is left in the sigma. (HfstBasicTransducer conversion is now used instead of this routine.)

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.insert-to-alphabet-fn]
> void FomaTransducer::insert_to_alphabet(fsm * t, const std::string &symbol)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.insert-to-alphabet-fn]
> Adds `symbol` to the sigma (alphabet) of net `t` by calling `sigma_add(strdup(symbol.c_str()), t->sigma)`. Returns void; mutates `t->sigma` in place. (The strdup'd string is handed to foma.)

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.intersect-fn]
> fsm * FomaTransducer::intersect

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.intersect-fn]
> Returns the intersection of `t1` and `t2` as `fsm_intersect(fsm_copy(t1), fsm_copy(t2))`. Both arguments are copied first so the originals are left intact.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.invert-fn]
> fsm * FomaTransducer::invert(fsm * t)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.invert-fn]
> Returns the inversion (swap of input and output sides) of `t` as `fsm_invert(fsm_copy(t))`. A copy is taken first so the original `t` is preserved.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.is-cyclic-fn]
> bool FomaTransducer::is_cyclic(fsm * t)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.is-cyclic-fn]
> Tests whether net `t` is cyclic. Calls `fsm_topsort(t)` (which sets `t->is_loop_free` as a side effect, mutating `t`), then returns `!(t->is_loop_free)` — true iff the net is NOT loop-free, i.e. cyclic.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.minimize-fn]
> fsm * FomaTransducer::minimize(fsm * t)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.minimize-fn]
> Returns a minimized copy of `t`: `fsm_minimize(fsm_copy(t))`. Because foma's `fsm_minimize` mutates and returns its argument, a copy is taken first so the original `t` is preserved.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.number-of-arcs-fn]
> unsigned int FomaTransducer::number_of_arcs(fsm * t)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.number-of-arcs-fn]
> Counts the arcs in net `t`. Initialize `retval = 0`. Iterate the state-line array `t->states` (index `i` from 0 while `(t->states)[i].state_no != -1`); for each line whose `in != -1` (a real arc, not a state-only/final marker line), increment `retval`. Return `retval`.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.number-of-states-fn]
> unsigned int FomaTransducer::number_of_states(fsm * t)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.number-of-states-fn]
> Counts the distinct states in net `t`. Initialize `retval = 0` and `laststate = -1`. Iterate the state-line array `t->states` (index `i` from 0 while `(t->states)[i].state_no != -1`): if the current line's `state_no != laststate`, increment `retval`; then set `laststate = state_no`. This relies on lines for the same state being contiguous, so each new `state_no` is counted once. Return `retval`.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.optionalize-fn]
> fsm * FomaTransducer::optionalize(fsm * t)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.optionalize-fn]
> Returns the optionalized form of `t` (the language plus the empty string) as `fsm_optionality(fsm_copy(t))`. A copy is taken first so the original `t` is preserved.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.print-test-fn]
> void FomaTransducer::print_test(fsm * t)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.print-test-fn]
> Debug helper. Prints net `t` in AT&T format to stdout by calling `net_print_att(t, stdout)`. Returns void.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.read-net-fn]
> fsm * FomaTransducer::read_net(FILE *infile)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.read-net-fn]
> Reads one foma net from `infile` in the textual `##foma-net 1.0##` format and returns the constructed `fsm *`, or NULL on EOF or format error. Uses a `char buf[4096]` line buffer and `io_gets` to read each line (newline stripped).
> Step 1: read first line; if `io_gets` returns 0 (immediate EOF), return NULL.
> Step 2: create an empty net `net = fsm_create("")` (strdup an empty name, free it after). If the first line != "##foma-net 1.0##", print "File format error foma!" and return NULL.
> Step 3: read next line; if != "##props##", print "File format error props!" and return NULL. Read the following line and `sscanf` it (with `LONG_LONG_SPECIFIER` for pathcount) into the net's fields, in order: `arity, arccount, statecount, linecount, finalcount, pathcount, is_deterministic, is_pruned, is_minimized, is_epsilon_free, is_loop_free, is_completed`, and a trailing name token into `buf` (the name is intentionally NOT copied to `net->name`; the empty name is kept). Read the next line.
> Step 4 (sigma): if the line != "##sigma##", print "File format error sigma!" and return NULL. Loop: read a line; if `buf[0] == '#'` break. Otherwise split on the first space (find " " via `strstr`, set it to '\0', advance past it to get `new_symbol`), parse the leading integer into `new_symbol_number` via `sscanf("%i")`, and call `sigma_add_number(net->sigma, new_symbol, new_symbol_number)`.
> Step 5 (states): the line that ended the sigma loop must be "##states##" else print "File format error!" and return NULL. Allocate `net->states = malloc(net->linecount * sizeof(struct fsm_state))`. Set `fsm_ = net->states`, `laststate = -1`, `last_final = 0`. Loop with index `i` from 0: read a line; if `buf[0] == '#'` break. Parse the line with `explode_line(buf, &lineint[0])` giving `items` integers, then fill `(fsm_+i)` by `items`:
> - 2 items: `state_no=laststate`, `in=out=lineint[0]`, `target=lineint[1]`, `final_state=last_final`.
> - 3 items: `state_no=laststate`, `in=lineint[0]`, `out=lineint[1]`, `target=lineint[2]`, `final_state=last_final`.
> - 4 items: `state_no=lineint[0]`, `in=out=lineint[1]`, `target=lineint[2]`, `final_state=lineint[3]`; update `laststate=lineint[0]`, `last_final=lineint[3]`.
> - 5 items: `state_no=lineint[0]`, `in=lineint[1]`, `out=lineint[2]`, `target=lineint[3]`, `final_state=lineint[4]`; update `laststate=lineint[0]`, `last_final=lineint[4]`.
> - default: print "File format error" and return NULL.
> Then set `start_state` from `laststate`: if `laststate > 0` -> 0; if `laststate == -1` -> -1; else (laststate == 0) -> 1.
> Step 6 (optional cmatrix): if the ending line == "##cmatrix##", call `cmatrix_init(net)`, set `cm = net->medlookup->confusion_matrix`, then loop reading lines until one starts with '#', parsing each as an int via `sscanf("%i")` and storing it through `cm` (post-incrementing the pointer).
> Step 7: the final ending line must be "##end##" else print "File format error!" and return NULL. Return `net`.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.remove-epsilons-fn]
> fsm * FomaTransducer::remove_epsilons(fsm * t)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.remove-epsilons-fn]
> Returns an epsilon-removed copy of `t`: `fsm_epsilon_remove(fsm_copy(t))`. Because foma's `fsm_epsilon_remove` mutates and returns its argument, a copy is taken first so the original `t` is preserved.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.remove-from-alphabet-fn]
> void FomaTransducer::remove_from_alphabet

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.remove-from-alphabet-fn]
> Removes `symbol` from the sigma (alphabet) of net `t` by calling `sigma_remove(strdup(symbol.c_str()), t->sigma)`. Returns void; mutates `t->sigma` in place.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.repeat-le-n-fn]
> fsm * FomaTransducer::repeat_le_n(fsm * t, unsigned int n)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.repeat-le-n-fn]
> Returns the net accepting between 0 and `n` concatenated copies of `t`'s language, via `fsm_concat_m_n(fsm_copy(t), 0, n)`. A copy is taken first so the original `t` is preserved.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.repeat-n-fn]
> fsm * FomaTransducer::repeat_n(fsm * t, unsigned int n)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.repeat-n-fn]
> Returns the net accepting exactly `n` concatenated copies of `t`'s language, via `fsm_concat_n(fsm_copy(t), n)`. A copy is taken first so the original `t` is preserved.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.repeat-plus-fn]
> fsm * FomaTransducer::repeat_plus(fsm * t)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.repeat-plus-fn]
> Returns the Kleene-plus closure (one or more repetitions) of `t` via `fsm_kleene_plus(fsm_copy(t))`. A copy is taken first so the original `t` is preserved.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.repeat-star-fn]
> fsm * FomaTransducer::repeat_star(fsm * t)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.repeat-star-fn]
> Returns the Kleene-star closure (zero or more repetitions) of `t` via `fsm_kleene_star(fsm_copy(t))`. A copy is taken first so the original `t` is preserved.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.reverse-fn]
> fsm * FomaTransducer::reverse(fsm * t)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.reverse-fn]
> Returns the reversal of `t` (accepting the reversed strings) via `fsm_reverse(fsm_copy(t))`. A copy is taken first so the original `t` is preserved.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.substitute-fn]
> fsm * FomaTransducer::substitute(fsm * t,String old_symbol,String new_symbol)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.substitute-fn]
> Substitutes every occurrence of `old_symbol` with `new_symbol` in net `t` by calling `fsm_substitute_symbol(t, strdup(old_symbol.c_str()), strdup(new_symbol.c_str()))` and returning its result. `t` is passed directly (no copy taken); the two symbol strings are strdup'd and handed to foma.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.subtract-fn]
> fsm * FomaTransducer::subtract

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.subtract-fn]
> Returns the difference (subtraction) of `t1` minus `t2` as `fsm_minus(fsm_copy(t1), fsm_copy(t2))`. Both arguments are copied first so the originals are left intact.

> [spec:hfst:def:foma-transducer.hfst.implementations.foma-transducer.write-net-fn]
> int FomaTransducer::write_net(fsm * net, FILE * outfile)

> [spec:hfst:sem:foma-transducer.hfst.implementations.foma-transducer.write-net-fn]
> Writes net `net` to `outfile` in the textual `##foma-net 1.0##` format, returning 1 on success.
> First call `fsm_count(net)` (otherwise linecount may be wrong).
> Header: print the literal line "##foma-net 1.0##\n".
> Props: print "##props##\n", then one line via `fprintf` (with `LONG_LONG_SPECIFIER` for pathcount) of the fields in order: `arity, arccount, statecount, linecount, finalcount, pathcount, is_deterministic, is_pruned, is_minimized, is_epsilon_free, is_loop_free, is_completed, name`, terminated by newline.
> Sigma: print "##sigma##\n", then walk the sigma linked list (`sigma = net->sigma`, follow `->next`) while `sigma != NULL && sigma->number != -1`, printing "%i %s\n" of `sigma->number` and `sigma->symbol`.
> States: print "##states##\n". Set `laststate = -1`. Iterate state lines (`fsm_ = net->states`; while `fsm_->state_no != -1`; `fsm_++`). For each line: if `fsm_->state_no != laststate` (start of a new state) print a full line — "%i %i %i %i %i" of `state_no, in, out, target, final_state` when `in != out`, else "%i %i %i %i" of `state_no, in, target, final_state`; if same state as previous, print a continuation line — "%i %i %i" of `in, out, target` when `in != out`, else "%i %i" of `in, target`. Then set `laststate = fsm_->state_no`. After the loop print the sentinel line "-1 -1 -1 -1 -1\n".
> Confusion matrix: if `net->medlookup != NULL && net->medlookup->confusion_matrix != NULL`, print "##cmatrix##\n", set `cm = net->medlookup->confusion_matrix`, compute `maxsigma = sigma_max(net->sigma)+1`, print "maxsigma is: %i\n" of maxsigma, then for `i` in `0..maxsigma*maxsigma` print "%i\n" of `cm[i]`.
> End: print "##end##\n". Call `fflush(outfile)`; if it returns nonzero, throw `HfstFatalException` with message "an error happened when writing a foma transducer". Otherwise return 1.

> [spec:hfst:def:foma-transducer.hfst.implementations.io-gets-fn]
> static int io_gets(FILE *infile, char *target)

> [spec:hfst:sem:foma-transducer.hfst.implementations.io-gets-fn]
> Reads one line from `infile` into the caller-provided buffer `target`, returning the number of characters stored (excluding the terminating NUL). Read the first character with `getc`. Loop with index `i` from 0 while the current character `c` is neither `'\n'` nor `'\0'`: store `c` into `target[i]`, then read the next char with `getc`. After the loop write `target[i] = '\0'`.
> Windows handling: if the last stored character `target[i-1]` is `'\r'`, overwrite it with `'\0'`.
> If the terminating character `c` was `'\0'`, push it back with `ungetc(c, infile)`. Return `i` (the count before the NUL). Note: no bounds checking on `target`; a `'\n'` is consumed and not stored.

> [spec:hfst:def:foma-transducer.main-fn]
> int main(int argc, char * argv[])

> [spec:hfst:sem:foma-transducer.main-fn]
> Unit-test entry point, compiled only under the `MAIN_TEST` build. Behavior depends on the `HAVE_FOMA` compile flag.
> If `HAVE_FOMA` is defined: print `"Unit tests for " __FILE__ ":"` to `std::cout` (no newline). Using namespace `hfst::implementations`, build `epsilon = FomaTransducer::define_transducer("@_EPSILON_SYMBOL_@")` (the single-symbol overload), then `epsilon_i = FomaTransducer::extract_input_language(epsilon)`, then `epsilon_i_min = FomaTransducer::minimize(fsm_copy(epsilon_i))` (the result is cast to void / unused). Then build `a = FomaTransducer::define_transducer("a")` and `a2 = FomaTransducer::repeat_n(a, 2)` (also unused, cast to void). Print `std::endl` followed by `"ok"` and `std::endl`, then return `EXIT_SUCCESS`. (No assertions on the results; this exercises construction, projection, minimization, and repetition without crashing.)
> If `HAVE_FOMA` is not defined: print `"Skipping unit tests for " << __FILE__ << ", FomaTransducer has not been enabled"` with a trailing newline to `std::cout`, and return 77 (the automake "test skipped" exit code).

