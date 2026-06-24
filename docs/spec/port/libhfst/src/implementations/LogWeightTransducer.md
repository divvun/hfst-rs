# libhfst/src/implementations/LogWeightTransducer.cc, libhfst/src/implementations/LogWeightTransducer.h

> [spec:hfst:def:log-weight-transducer.hfst.implementations.extract-paths-fn]
> static bool

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.extract-paths-fn]
> File-local static recursive DFS that enumerates paths of LogFst `t`
> starting at state `s`, invoking `callback` for each path. Parameters:
> `all_visitations` and `path_visitations` are maps state->count passed BY
> VALUE (each recursion gets its own copy); `weight_sum` is accumulated path
> weight; `cycles` is a cycle bound; `fd_state_stack` is an optional vector of
> FdState<int64> tracking flag-diacritic state; `filter_fd` controls whether
> flag symbols are suppressed from output strings; `spv` is the
> StringPairVector accumulating the current path's (input,output) symbol pairs.
> Steps: (1) If `cycles >= 0` and `path_visitations[s] > cycles`, return true
> (cycle bound reached). (2) Increment `all_visitations[s]` and
> `path_visitations[s]`. (3) If `spv` is non-empty, compute `final` = whether
> `t->Final(s) != LogWeight::Zero()`, build an HfstTwoLevelPath with weight
> `weight_sum + (final ? t->Final(s).Value() : 0)` and the current `spv`, call
> `callback(path, final)`; if the returned RetVal has `continueSearch` false or
> `continuePath` false, decrement `path_visitations[s]` and return
> `ret.continueSearch`. (4) Collect the arcs out of `s` into a vector sorted
> ascending by `all_visitations[arc.nextstate]` (insertion sort: find first
> position whose target has a greater visitation count, shift right, insert).
> (5) For each arc in that order while `res` is still true: if `fd_state_stack`
> is set and `arc.ilabel` is a flag operation, push a copy of the top FdState
> and apply the operation; if `apply_operation` fails, pop and `continue`
> (skip the arc), else mark `added_fd_state`. Build `istring`/`ostring`: each
> is set to `t->InputSymbols()->Find(label)` unless `filter_fd` is true and
> that label is a flag operation (in which case it stays empty). Push
> `StringPair(istring, ostring)` onto `spv`, recurse into `arc.nextstate` with
> `weight_sum + arc.weight.Value()`, storing the result in `res`, then pop the
> pair off `spv`. If `added_fd_state`, pop the FdState. (6) After the loop,
> decrement `path_visitations[s]` and return `res`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.initialize-symbol-tables-fn]
> void initialize_symbol_tables(LogFst *t)

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.initialize-symbol-tables-fn]
> Free function declared at file scope as `void initialize_symbol_tables(LogFst
> *t);` — a forward declaration only, with no body at this site. The actual
> definition is the member function
> `LogWeightTransducer::initialize_symbol_tables`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.label-pair]
> typedef std::pair<int, int> LabelPair

> [spec:hfst:def:log-weight-transducer.hfst.implementations.label-pair-vector]
> typedef std::vector<LabelPair> LabelPairVector

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-arc-less-than]
> struct LogArcLessThan

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-arc-less-than.operator-fn]
> bool operator() (const LogArc &arc1,const LogArc &arc2) const

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-arc-less-than.operator-fn]
> Comparison functor `bool operator()(const LogArc &arc1, const LogArc &arc2)
> const` for ordering LogArc values. Declared only (member of struct
> LogArcLessThan); no definition exists in this translation unit
> (LogWeightTransducer.cc/.h). The body must be supplied elsewhere or is unused;
> nothing here specifies the ordering semantics.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-arc-vector]
> typedef std::vector<LogArc> LogArcVector

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-fst]
> typedef VectorFst<LogArc> LogFst

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream]
> class LogWeightInputStream {
>   std::string filename;
>   std::ifstream i_stream;
>   std::istream &input_stream;
> }

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.close-fn]
> void

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.close-fn]
> If `filename` is non-empty (i.e. this stream reads from a file rather than
> stdin), close the underlying `i_stream` (`i_stream.close()`). If `filename`
> is empty, do nothing.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.ignore-fn]
> void

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.ignore-fn]
> `ignore(unsigned int n)`: discards `n` characters from `input_stream` by
> calling `input_stream.ignore(n)`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.is-bad-fn]
> bool

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.is-bad-fn]
> `is_bad() const`: returns the bad-bit state of the underlying stream. If
> `filename` is empty (stdin mode), returns `std::cin.bad()`; otherwise returns
> `input_stream.bad()`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.is-eof-fn]
> bool

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.is-eof-fn]
> `is_eof() const`: returns true iff `input_stream.peek() == EOF`, i.e. peeking
> the next character yields end-of-file.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.is-fst-fn]
> bool

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.is-fst-fn]
> Static `is_fst(FILE *f)`: returns false if `f == NULL`. Otherwise reads one
> byte with `getc(f)`, immediately pushes it back with `ungetc(c, f)`, and
> returns whether that byte equals `0xd6` (the OpenFst magic-number first byte).
> A companion overload `is_fst(std::istream &s)` returns `s.good() && (s.peek()
> == 0xd6)`, and the no-arg member `is_fst()` delegates to the istream overload
> on `input_stream`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.is-good-fn]
> bool

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.is-good-fn]
> `is_good() const`: returns false immediately if `is_eof()` is true.
> Otherwise, if `filename` is empty (stdin mode) returns `std::cin.good()`,
> else returns `input_stream.good()`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.log-weight-input-stream-fn]
> LogWeightInputStream::LogWeightInputStream(void)

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.log-weight-input-stream-fn]
> Default constructor `LogWeightInputStream()`: leaves `filename` empty,
> default-constructs the member `i_stream`, and binds the `input_stream`
> reference to `std::cin` — i.e. configures the stream to read from standard
> input. Body is empty. (Two sibling constructors exist: one taking a
> `const std::string &filename_` which stores it, opens `i_stream` on that path,
> and binds `input_stream` to `i_stream`; and one taking a `std::istream &is`
> which binds `input_stream` directly to `is`.)

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.operator-fn]
> bool

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.operator-fn]
> `operator()() const`: returns `is_good()` — lets the stream be tested for
> readiness like a boolean.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.read-transducer-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.read-transducer-fn]
> `read_transducer()`: reads one binary OpenFst log-semiring transducer from
> the stream and returns a `LogFst *`. Steps: (1) If `is_eof()`, throw
> `StreamIsClosedException` (via HFST_THROW). (2) Construct an `FstHeader
> header`. In a try block: if `filename` is empty, call
> `header.Read(input_stream, "STDIN")` then `t = static_cast<LogFst *>(
> LogFst::Read(input_stream, FstReadOptions("STDIN", &header)))`; otherwise use
> `filename` in place of "STDIN" for both calls. If the resulting `t` is NULL,
> throw `TransducerHasWrongTypeException`. Any caught `HfstException` is
> rethrown. (3) Return `t`. Side effect: consumes the transducer's bytes from
> the stream and heap-allocates the returned LogFst (caller owns it).

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.skip-hfst-header-fn]
> void

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.skip-hfst-header-fn]
> `skip_hfst_header()`: discards 6 bytes via `input_stream.ignore(6)`, then
> calls `skip_identifier_version_3_0()` (which discards a further 14 bytes). The
> commented-out original code shows it formerly branched on a header byte, but
> the live code unconditionally skips the version-3.0 identifier.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.skip-identifier-version-3-0-fn]
> void

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.skip-identifier-version-3-0-fn]
> `skip_identifier_version_3_0()`: skips the fixed 14-byte identifier string
> "LOG_OFST_TYPE" by calling `input_stream.ignore(14)`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.stream-get-fn]
> char

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.stream-get-fn]
> `stream_get()`: reads and consumes one byte from `input_stream` via
> `input_stream.get()`, casts the returned int to `char`, and returns it.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.stream-get-short-fn]
> short

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.stream-get-short-fn]
> `stream_get_short()`: reads `sizeof(short)` raw bytes from `input_stream`
> directly into a local `short i` via `input_stream.read((char*)&i,
> sizeof(i))` (native byte order), and returns `i`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-input-stream.stream-unget-fn]
> void

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-input-stream.stream-unget-fn]
> `stream_unget(char c)`: pushes character `c` back onto `input_stream` via
> `input_stream.putback(c)` so it will be read again next.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-output-stream]
> class LogWeightOutputStream {
>   std::string filename;
>   std::ofstream o_stream;
>   std::ostream &output_stream;
> }

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-output-stream.close-fn]
> void

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-output-stream.close-fn]
> If `filename` is non-empty (file-backed output), close the underlying
> `o_stream` (`o_stream.close()`). If `filename` is empty (stdout mode), do
> nothing.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-output-stream.log-weight-output-stream-fn]
> LogWeightOutputStream::LogWeightOutputStream(void)

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-output-stream.log-weight-output-stream-fn]
> Default constructor `LogWeightOutputStream()`: sets `filename` to an empty
> string and binds the `output_stream` reference to `std::cout`. If
> `output_stream` is in a failed state (`!output_stream`), prints
> "LogWeightOutputStream: ERROR: failbit set (3)." to stderr. (A sibling
> constructor taking `const std::string &str` stores it as `filename`, opens
> `o_stream` on that path with `std::ios::out`, and binds `output_stream` to
> `o_stream`.)

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-output-stream.write-fn]
> void

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-output-stream.write-fn]
> `write(const char &c)`: writes the single character `c` to `output_stream`
> via `output_stream.put(char(c))`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-output-stream.write-transducer-fn]
> void

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-output-stream.write-transducer-fn]
> `write_transducer(LogFst *transducer)`: writes the transducer in binary
> OpenFst format including both input and output symbol tables. Steps: (1) If
> `output_stream` is in a failed state, print "LogWeightOutputStream: ERROR:
> failbit set (1)." to stderr. (2) Copy-construct a local `fst::SymbolTable
> output_st` from `*(transducer->InputSymbols())`, and set it as the
> transducer's output symbols via `transducer->SetOutputSymbols(&output_st)`
> (so the output table mirrors the input table). (3) Call
> `transducer->Write(output_stream, FstWriteOptions())`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer]
> class LogWeightTransducer

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.add-and-map-state-fn]
> StateId

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.add-and-map-state-fn]
> `add_and_map_state(LogFst *t, int state_number, StateMap &state_map)`:
> returns the LogFst StateId corresponding to the external integer
> `state_number`, creating it on first use. Look up `state_number` in
> `state_map`; if absent, call `t->AddState()` to allocate a new StateId, insert
> the pair `(state_number, new_id)` into `state_map`, and return the new id.
> If already present, return the mapped StateId. Used when reading AT&T format
> to map source-file state numbers to allocated states.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.add-state-fn]
> StateId

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.add-state-fn]
> `add_state(LogFst *t)`: allocates a new state via `t->AddState()`. If the
> returned StateId is 0 (the first state added), also makes it the start state
> via `t->SetStart(s)`. Returns the new StateId.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.add-sub-trie-fn]
> void

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.add-sub-trie-fn]
> `add_sub_trie(LogFst &t1, StateId t1_state, const LogFst *t2, StateId
> t2_state)`: recursively copies the entire sub-trie of `t2` rooted at
> `t2_state` into `t1` rooted at `t1_state`, creating a fresh state in `t1` for
> every `t2` arc (no sharing/merging). Steps: (1) If `t2->Final(t2_state) !=
> LogWeight::Zero()`, set `t1`'s final weight at `t1_state` to
> `Plus(t1.Final(t1_state), t2->Final(t2_state))`. (2) For each arc out of
> `t2_state` in `t2`: allocate `new_state = t1.AddState()`, add to `t1` an arc
> from `t1_state` with the same ilabel, olabel, and weight targeting
> `new_state`, then recurse `add_sub_trie(t1, new_state, t2, arc.nextstate)`.
> Assumes `t2`'s sub-structure is a trie (tree); does not check for existing
> matching arcs.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.add-transition-fn]
> void

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.add-transition-fn]
> `add_transition(LogFst *t, StateId source, std::string &isymbol, std::string
> &osymbol, float w, StateId target)`: adds an arc carrying the named symbols.
> Steps: (1) Copy the transducer's input symbol table: `SymbolTable *st =
> t->InputSymbols()->Copy()`. (2) `ilabel = st->AddSymbol(isymbol)` and
> `olabel = st->AddSymbol(osymbol)` (adds the symbols if not already present,
> returning their numeric labels). (3) Add the arc `LogArc(ilabel, olabel, w,
> target)` from `source` via `t->AddArc`. (4) Install the augmented table back
> via `t->SetInputSymbols(st)`, then `delete st`. No return value.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.are-equivalent-fn]
> bool

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.are-equivalent-fn]
> `are_equivalent(LogFst *a, LogFst *b)`: tests language/relation equivalence.
> Steps: (1) `mina = minimize(a)`, `minb = minimize(b)` (each returns a new
> minimized LogFst). (2) Build a shared `EncodeMapper<LogArc> encode_mapper(
> 0x0001, ENCODE)` (encode labels only). (3) Form `EncodeFst<LogArc>` wrappers
> `enca(*mina, &encode_mapper)` and `encb(*minb, &encode_mapper)`, materialize
> them into `LogFst A(enca)` and `LogFst B(encb)`. (4) Return `Equivalent(A,
> B)` (OpenFst's equivalence check, valid because both are encoded with the same
> mapper). Note: `mina`/`minb` are heap-allocated and not freed here.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.compose-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.compose-fn]
> `compose(LogFst *t1, LogFst *t2)`: returns a new LogFst equal to the
> composition t1 ∘ t2. Steps: (1) Make `t2_ = expand_arcs(t2, foo, false)` (a
> copy of t2 with expanded arcs, using an empty StringSet `foo`) so its symbol
> table checksum matches t1's, avoiding OpenFst checksum mismatch errors.
> (2) Set `t2_`'s input symbols to `t1->InputSymbols()` and set `t1`'s output
> symbols to `t1->InputSymbols()`. (3) Arc-sort `t1` by output label
> (`OLabelCompare<LogArc>`) and `t2_` by input label
> (`ILabelCompare<LogArc>`). (4) Allocate `result = new LogFst()` and call
> `Compose(*t1, *t2_, result)`. (5) `delete t2_`. (6) Set `result`'s input
> symbols to `t1->InputSymbols()` and return `result`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.concatenate-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.concatenate-fn]
> `concatenate(LogFst *t1, LogFst *t2)`: returns a new LogFst equal to t1
> followed by t2. Steps: (1) `result = new LogFst(*t1)` (deep copy of t1).
> (2) `Concat(result, *t2)` (OpenFst in-place concatenation appending t2).
> (3) Set `result`'s input symbols to `t1->InputSymbols()` and return
> `result`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.copy-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.copy-fn]
> `copy(LogFst *t)`: returns a heap-allocated deep copy of `t` via `new
> LogFst(*t)`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.create-empty-transducer-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.create-empty-transducer-fn]
> `create_empty_transducer()`: allocates `t = new LogFst`, calls
> `initialize_symbol_tables(t)` to install the default symbol table
> (epsilon/unknown/identity at 0/1/2), adds one state `s = t->AddState()`, and
> sets it as the start state via `t->SetStart(s)`. The state is non-final, so
> the transducer accepts nothing (empty language). Returns `t`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.create-epsilon-transducer-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.create-epsilon-transducer-fn]
> `create_epsilon_transducer()`: allocates `t = new LogFst`, calls
> `initialize_symbol_tables(t)` to install the default symbol table
> (epsilon/unknown/identity at 0/1/2), adds one state `s = t->AddState()`,
> sets it as the start state via `t->SetStart(s)`, and makes it final with
> weight 0 via `t->SetFinal(s, 0)`. The result accepts the single empty
> (epsilon) path. Returns `t`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.create-mapping-fn]
> NumberNumberMap

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.create-mapping-fn]
> `create_mapping(LogFst *t1, LogFst *t2)`: computes the number-to-number
> remapping needed to make `t1`'s symbol encoding agree with `t2`'s. Precondition:
> `t2`'s symbol table must contain every symbol in `t1`'s. Build an empty
> `NumberNumberMap km`. For each entry in `t1`'s input symbol table, set
> `km[(unsigned)it->Label()] = (unsigned)t2->InputSymbols()->Find(it->Symbol())`,
> i.e. map t1's numeric label for a symbol to t2's numeric label for the same
> symbol string. Return `km`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.create-symbol-table-fn]
> fst::SymbolTable

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.create-symbol-table-fn]
> `create_symbol_table(std::string name)`: constructs a local
> `fst::SymbolTable st(name)`, then adds the three reserved internal symbols at
> fixed numeric labels: `st.AddSymbol(internal_epsilon, 0)`,
> `st.AddSymbol(internal_unknown, 1)`, `st.AddSymbol(internal_identity, 2)`.
> Returns the table by value.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.define-transducer-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.define-transducer-fn]
> `define_transducer(const StringPairSet &sps, bool cyclic)`: builds a one- or
> two-state transducer recognizing each symbol pair in `sps`. Steps: (1)
> Allocate `t = new LogFst` and `st = create_symbol_table("")`. (2) Add start
> state `s1 = t->AddState()`, `t->SetStart(s1)`, and set `s2 = s1`. (3) If `sps`
> is non-empty: if `!cyclic`, allocate a distinct final state `s2 =
> t->AddState()` (otherwise `s2` stays equal to `s1`, making the arcs loop back
> on the start state). For each pair in `sps`, add an arc from `s1` to `s2` with
> ilabel `st.AddSymbol(it->first)`, olabel `st.AddSymbol(it->second)`, weight 0.
> (4) Set `s2` final with weight 0 via `t->SetFinal(s2, 0)`. (5) Install `st`
> via `t->SetInputSymbols(&st)` and return `t`. (This is one of several
> overloads; sibling overloads accept StringPairVector, vector<StringPairSet>,
> NumberPairVector, NumberPairSet, and vector<NumberPairSet>, building chains or
> sets analogously, with the number variants not installing a symbol table.)

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.delete-transducer-fn]
> void

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.delete-transducer-fn]
> `delete_transducer(LogFst *t)`: frees the transducer via `delete t`. No
> return value.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.determinize-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.determinize-fn]
> `determinize(LogFst *t)`: returns a new determinized LogFst. Steps: (1)
> Remove epsilons in place with `RmEpsilon<LogArc>(t)`. (2) Build
> `EncodeMapper<LogArc> encode_mapper(kEncodeLabels | kEncodeWeights, ENCODE)`
> and encode `t` in place via `Encode(t, &encode_mapper)` (encodes both labels
> and weights into the labels so OpenFst's Determinize, which needs functional
> arcs, applies). (3) Allocate `det = new LogFst()` and run `Determinize<LogArc>(
> *t, det)`. (4) Decode `det` with `Decode(det, encode_mapper)` to restore the
> original labels/weights. (5) Return `det`. Note: `t` is mutated (epsilons
> removed, encoded).

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.disjunct-as-tries-fn]
> void

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.disjunct-as-tries-fn]
> `disjunct_as_tries(LogFst &t1, StateId t1_state, const LogFst *t2, StateId
> t2_state)`: recursively merges the trie of `t2` rooted at `t2_state` into the
> trie of `t1` rooted at `t1_state`, sharing prefixes where arcs match. Steps:
> (1) If `t2->Final(t2_state) != LogWeight::Zero()`, set `t1`'s final weight at
> `t1_state` to `Plus(t1.Final(t1_state), t2->Final(t2_state))`. (2) For each arc
> out of `t2_state`: compute `arc_index = has_arc(t1, t1_state, arc.ilabel,
> arc.olabel)`. If `arc_index == -1` (no matching arc in t1), allocate
> `new_state = t1.AddState()`, add an arc from `t1_state` carrying the same
> ilabel/olabel/weight to `new_state`, then call `add_sub_trie(t1, new_state,
> t2, arc.nextstate)` to copy the rest of t2's subtree wholesale. Otherwise (a
> matching arc exists), seek a `MutableArcIterator<LogFst>` on `t1`/`t1_state`
> to `arc_index`, read its target `nextstate`, and recurse
> `disjunct_as_tries(t1, that_nextstate, t2, arc.nextstate)` to merge further
> down. Assumes both are tries.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.disjunct-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.disjunct-fn]
> `disjunct(LogFst *t, const StringPairVector &spv)`: adds the single path
> described by `spv` into `t` in place, sharing the existing prefix where arcs
> already match, and returns `t`. Steps: (1) Copy the input symbol table `st =
> t->InputSymbols()->Copy()` (asserts non-null). (2) Start at `s = t->Start()`.
> (3) For each `(first, second)` pair in `spv`: get `inumber =
> st->AddSymbol(first)` and `onumber = st->AddSymbol(second)` (adding symbols as
> needed). Scan the arcs out of `s`; if one has matching ilabel `inumber` and
> olabel `onumber`, follow it (`s = a.nextstate`) and mark found. If no matching
> arc, allocate `new_state = t->AddState()`, add arc `LogArc(inumber, onumber,
> 0, new_state)` from `s`, and set `s = new_state`. (4) After consuming all
> pairs, make the final reached state final with weight 0 (`t->SetFinal(s,
> 0)`). (5) Install the augmented table `t->SetInputSymbols(st)` and return `t`.
> (A sibling overload takes a NumberPairVector and behaves identically but
> without symbol-table manipulation.)

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.expand-arcs-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.expand-arcs-fn]
> `expand_arcs(LogFst *t, StringSet &unknown, bool unknown_symbols_in_use)`:
> returns a new LogFst that copies `t` while expanding unknown/identity meta-arcs
> against the symbol set `unknown`. Steps: (1) Allocate `result = new LogFst()`,
> and pre-add one state per state of `t` (state ids are preserved 1:1). (2) For
> each state `s` of `t` (result state `result_s = s`): if `t->Start() == s`, set
> result's start; if `t->Final(s) != LogWeight::Zero()`, copy the final weight.
> (3) For each arc out of `s` (target preserved as `arc.nextstate`): if
> `unknown_symbols_in_use` is true, expand based on the labels using `is =
> t->InputSymbols()` to look up each symbol string in `unknown` to its number:
> (a) ilabel==1 && olabel==1 (cross-product "?:?"): for every unknown symbol `x`
> (inumber) and every unknown symbol `y` (onumber), if `inumber != onumber` add
> arc `inumber:onumber`; also add `inumber:1` and `1:inumber`. (b) ilabel==2 ||
> olabel==2 (identity): for every unknown symbol add `number:number`. (c)
> ilabel==1 ("?:x"): for every unknown symbol add `number:arc.olabel`. (d)
> olabel==1 ("x:?"): for every unknown symbol add `arc.ilabel:number`. All added
> arcs keep `arc.weight` and target `result_nextstate`. (4) In every case
> (whether or not expanded), also copy the original arc `LogArc(arc.ilabel,
> arc.olabel, arc.weight, result_nextstate)`. (5) Return `result` (its input
> symbol table is left unset — the commented-out SetInputSymbols line is not
> executed).

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.extract-input-language-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.extract-input-language-fn]
> `extract_input_language(LogFst *t)`: returns a new LogFst that is the input
> projection of `t`, i.e. `new LogFst(ProjectFst<LogArc>(*t, ProjectType::INPUT))`
> (each arc's olabel becomes equal to its ilabel). Copies `t`'s input symbol
> table onto the result via `SetInputSymbols(t->InputSymbols())`. Returns the
> result.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.extract-output-language-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.extract-output-language-fn]
> `extract_output_language(LogFst *t)`: returns a new LogFst that is the output
> projection of `t`, i.e. `new LogFst(ProjectFst<LogArc>(*t,
> ProjectType::OUTPUT))` (each arc's ilabel becomes equal to its olabel). Copies
> `t`'s input symbol table onto the result via
> `SetInputSymbols(t->InputSymbols())`. Returns the result.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.extract-paths-fn]
> void

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.extract-paths-fn]
> `extract_paths(LogFst *t, ExtractStringsCb &callback, int cycles,
> FdTable<int64> *fd, bool filter_fd)`: enumerates paths of `t`, invoking
> `callback` per path, by delegating to the file-local recursive DFS
> `hfst::implementations::extract_paths`. Steps: (1) If `t->Start() == -1` (no
> start state), return immediately. (2) Create empty maps `all_visitations` and
> `path_visitations` (StateId -> unsigned short). (3) Build `fd_state_stack`:
> NULL if `fd == NULL`, otherwise a `new std::vector<FdState<int64>>` initialized
> with one element `FdState<int64>(*fd)`. (4) Create an empty StringPairVector
> `spv`. (5) Call the recursive `extract_paths(t, t->Start(), all_visitations,
> path_visitations, 0.0f, callback, cycles, fd_state_stack, filter_fd, spv)`. No
> return value; results are delivered through `callback`. (Note: `fd_state_stack`
> is heap-allocated here and not explicitly freed in this function.)

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.extract-random-paths-fn]
> void

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.extract-random-paths-fn]
> `extract_random_paths(const LogFst *t, HfstTwoLevelPaths &results, int
> max_num)`: not implemented. Ignores all three parameters (cast to void) and
> unconditionally throws `FunctionNotImplementedException` via HFST_THROW.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.get-alphabet-fn]
> StringSet

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.get-alphabet-fn]
> `get_alphabet(LogFst *t)`: returns a `StringSet` of all symbol strings in
> `t`'s input symbol table. Asserts `t->InputSymbols() != NULL`. Iterates the
> input symbol table and inserts each `it->Symbol()` (as `std::string`) into a
> fresh `StringSet s`, then returns `s`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.get-final-weight-fn]
> float

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.get-final-weight-fn]
> `get_final_weight(LogFst *t, StateId s)`: returns the final weight of state
> `s` as a float via `t->Final(s).Value()`. For a non-final state this is the
> LogWeight::Zero() value (positive infinity).

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.get-flag-diacritics-fn]
> FdTable<int64> *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.get-flag-diacritics-fn]
> `get_flag_diacritics(LogFst *t)`: heap-allocates a `FdTable<int64> *table =
> new FdTable<int64>()`, iterates `t`'s input symbol table, and for every entry
> whose symbol string satisfies `FdOperation::is_diacritic(it->Symbol())`, calls
> `table->define_diacritic(it->Label(), it->Symbol())` to register that flag
> diacritic by its numeric label and name. Returns `table` (caller owns it).

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.get-initial-state-fn]
> StateId

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.get-initial-state-fn]
> `get_initial_state(LogFst *t)`: returns `t->Start()`, the StateId of the
> transducer's start state (or kNoStateId / -1 if none is set).

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.get-profile-seconds-fn]
> float

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.get-profile-seconds-fn]
> `get_profile_seconds()`: returns the file-scope accumulator
> `log_seconds_in_harmonize` (a float, initialized to 0), which `harmonize`
> increments with the wall-clock time it spends. Pure getter, no side effects.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.get-symbol-number-fn]
> unsigned int

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.get-symbol-number-fn]
> `get_symbol_number(LogFst *t, const std::string &symbol)`: asserts
> `t->InputSymbols() != NULL`, looks up `symbol` via `t->InputSymbols()->Find(
> symbol)` into an int64 `i`. If `i < 0` (not found), throws
> `SymbolNotFoundException`. Otherwise returns `(unsigned int)i`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.harmonize-fn]
> std::pair<LogFst *, LogFst *>

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.harmonize-fn]
> `harmonize(LogFst *t1, LogFst *t2, bool unknown_symbols_in_use)`: aligns the
> symbol encodings of `t1` and `t2` and (optionally) expands unknown/identity
> meta-arcs, returning a `pair<LogFst*, LogFst*>` of the harmonized transducers.
> Steps: (1) Record `startclock = clock()`. (2) Compute alphabets `t1_symbols =
> get_alphabet(t1)`, `t2_symbols = get_alphabet(t2)`, and call
> `hfst::symbols::collect_unknown_sets(t1_symbols, unknown_t1, t2_symbols,
> unknown_t2)` to fill `unknown_t1` (symbols in t2 but not t1) and `unknown_t2`
> (vice versa). (3) Copy t2's input table `st2`, add every symbol of
> `unknown_t2` to it, and install it on t2. (4) Compute `km = create_mapping(t1,
> t2)` (t1-label -> t2-label map). (5) Replace t1's symbol table with `st2`
> (so t1 and t2 share the table), `delete st2`, then recode t1's arc labels via
> `recode_symbol_numbers(t1, km)`. (6) If `!unknown_symbols_in_use`, set
> `harmonized_t1 = t1` and `harmonized_t2 = t2` unchanged. Otherwise set
> `harmonized_t1 = expand_arcs(t1, unknown_t1, true)` then copy t1's input table
> onto it, and likewise `harmonized_t2 = expand_arcs(t2, unknown_t2, true)` with
> t2's table. (7) Add elapsed `(clock() - startclock)/CLOCKS_PER_SEC` seconds to
> the global `log_seconds_in_harmonize`. (8) Return `(harmonized_t1,
> harmonized_t2)`. Mutates t1 and t2 (their symbol tables and, for t1, arc
> labels).

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.has-arc-fn]
> int

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.has-arc-fn]
> `has_arc(LogFst &t, StateId sourcestate, Label ilabel, Label olabel)`: scans
> the arcs out of `sourcestate` with an `ArcIterator<LogFst>`; for the first arc
> whose ilabel == `ilabel` and olabel == `olabel`, returns its position index
> (`aiter.Position()`). If no arc matches, returns -1.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.initialize-symbol-tables-fn]
> void

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.initialize-symbol-tables-fn]
> `initialize_symbol_tables(LogFst *t)`: builds a fresh symbol table via
> `create_symbol_table("")` (which adds `internal_epsilon`->0,
> `internal_unknown`->1, `internal_identity`->2) and installs it as the
> transducer's input symbols via `t->SetInputSymbols(&st)` (SetInputSymbols
> copies the table, so the local `st` going out of scope is fine).

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.insert-freely-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.insert-freely-fn]
> `insert_freely(LogFst *t, const StringPair &symbol_pair)`: adds a
> self-looping arc carrying `symbol_pair` to every state of `t` in place, then
> returns `t`. Steps: (1) Copy the input table `st = t->InputSymbols()->Copy()`
> (asserts non-null). (2) For each state `state_id`, add an arc from `state_id`
> back to itself: `LogArc(st->AddSymbol(symbol_pair.first),
> st->AddSymbol(symbol_pair.second), 0, state_id)` (symbols added to `st` as
> needed). (3) Install `st` via `t->SetInputSymbols(st)`, `delete st`, return
> `t`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.insert-to-alphabet-fn]
> void

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.insert-to-alphabet-fn]
> `insert_to_alphabet(LogFst *t, const std::string &symbol)`: asserts
> `t->InputSymbols() != NULL`, copies the input symbol table (`st =
> t->InputSymbols()->Copy()`), adds `symbol` to it via `st->AddSymbol(symbol)`,
> installs the augmented table with `t->SetInputSymbols(st)`, then `delete st`.
> No return value. Does not touch any arcs.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.intersect-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.intersect-fn]
> `intersect(LogFst *t1, LogFst *t2)`: returns a new LogFst recognizing the
> intersection of the (determinized) languages. Steps: (1) For each of t1, t2,
> if its output symbols are NULL, set them to its input symbols. (2) Arc-sort t1
> by output label (`OLabelCompare<LogArc>`) and t2 by input label
> (`ILabelCompare<LogArc>`). (3) Remove epsilons in place: `RmEpsilon(t1)`,
> `RmEpsilon(t2)`. (4) Build a shared `EncodeMapper<LogArc> encoder(0x0001,
> ENCODE)` (encode labels only); wrap each as `EncodeFst` then `DeterminizeFst`
> (`det1`, `det2`). (5) Form `IntersectFst<LogArc> intersect(det1, det2)`,
> materialize into `foo = new LogFst(intersect)`, decode it via `DecodeFst<LogArc>
> decode(*foo, encoder)`, `delete foo`, and materialize `result = new
> LogFst(decode)`. (6) Set result's input symbols to `t1->InputSymbols()` and
> return it. Mutates t1 and t2 (sorted, epsilon-removed, output symbols set).

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.invert-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.invert-fn]
> `invert(LogFst *t)`: returns a new LogFst with input and output labels
> swapped. Deep-copies `t` via `copy(t)` into `inverse`, calls
> `Invert(inverse)` (OpenFst in-place swap of each arc's ilabel/olabel), copies
> `t`'s input symbol table onto `inverse` via
> `SetInputSymbols(t->InputSymbols())`, and returns `inverse`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.is-automaton-fn]
> bool

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.is-automaton-fn]
> `is_automaton(LogFst *t)`: returns true iff `t` is an acceptor (automaton).
> Iterates all states and all their arcs: if any arc has `ilabel != olabel`,
> return false; if any arc has `ilabel == 1` (the unknown "?:?" symbol), return
> false. If no such arc is found, return true.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.is-cyclic-fn]
> bool

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.is-cyclic-fn]
> `is_cyclic(LogFst *t)`: returns whether `t` contains a cycle by querying
> OpenFst properties: `t->Properties(kCyclic, true) & kCyclic` (the `true`
> argument forces the property to be computed if not already known). Non-zero
> (true) if cyclic.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.is-final-fn]
> float

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.is-final-fn]
> `is_final(LogFst *t, StateId s)`: returns (as a float, effectively boolean
> 1.0/0.0) the result of `t->Final(s) != LogWeight::Zero()`, i.e. true when
> state `s` has a non-Zero final weight (is a final state).

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.minimize-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.minimize-fn]
> `minimize(LogFst *t)`: returns a new determinized and minimized LogFst.
> Steps: (1) `RmEpsilon<LogArc>(t)` (in place). (2) Build
> `EncodeMapper<LogArc> encode_mapper(kEncodeLabels, ENCODE)` (labels only — the
> `|kEncodeWeights` is commented out, so weights are NOT encoded) and `Encode(t,
> &encode_mapper)`. (3) Allocate `det = new LogFst()`, run
> `Determinize<LogArc>(*t, det)`, then `Minimize<LogArc>(det)`. (4) `Decode(det,
> encode_mapper)` to restore labels. (5) Return `det`. Mutates `t` (epsilons
> removed, encoded).

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.n-best-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.n-best-fn]
> `n_best(LogFst *, unsigned int)`: not implemented. Both parameters are unnamed
> and ignored; the original ShortestPath implementation is commented out because
> in OpenFst 1.8 the log semiring lacks the algebra required for shortest paths.
> Unconditionally throws `FunctionNotImplementedException` via HFST_THROW.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.number-of-states-fn]
> unsigned int

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.number-of-states-fn]
> `number_of_states(const LogFst *t)`: counts the states of `t` by iterating a
> `StateIterator<LogFst>` over `*t` and incrementing a counter (`retval`,
> starting at 0) once per state. Returns the count as `unsigned int`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.optionalize-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.optionalize-fn]
> `optionalize(LogFst *t)`: returns a new LogFst recognizing `t` or the empty
> string. Allocates `eps = create_epsilon_transducer()`, unions `t` into it via
> `Union(eps, *t)` (so `eps` now accepts epsilon plus everything `t` accepts),
> copies `t`'s input symbol table onto `eps` via
> `SetInputSymbols(t->InputSymbols())`, and returns `eps`. Does not mutate `t`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.push-labels-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.push-labels-fn]
> `push_labels(LogFst *t, bool to_initial_state)`: returns a new LogFst that is
> `t` with labels pushed. Asserts `t->InputSymbols() != NULL`. Allocates `retval
> = new LogFst()`. If `to_initial_state` is true, calls `fst::Push<LogArc,
> REWEIGHT_TO_INITIAL>(*t, retval, fst::kPushLabels)`; otherwise
> `fst::Push<LogArc, REWEIGHT_TO_FINAL>(*t, retval, fst::kPushLabels)`. Copies
> `t`'s input symbol table onto `retval` via `SetInputSymbols`, and returns
> `retval`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.push-weights-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.push-weights-fn]
> `push_weights(LogFst *t, bool to_initial_state)`: returns a new LogFst that is
> `t` with weights pushed. Asserts `t->InputSymbols() != NULL`. Allocates `retval
> = new LogFst()`. If `to_initial_state` is true, calls `fst::Push<LogArc,
> REWEIGHT_TO_INITIAL>(*t, retval, fst::kPushWeights)`; otherwise
> `fst::Push<LogArc, REWEIGHT_TO_FINAL>(*t, retval, fst::kPushWeights)`. Copies
> `t`'s input symbol table onto `retval` via `SetInputSymbols`, and returns
> `retval`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.read-in-att-format-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.read-in-att-format-fn]
> `read_in_att_format(FILE *ifile)`: parses one transducer in AT&T text format
> and returns a new LogFst. Steps: (1) Allocate `t = new LogFst`, build `st =
> create_symbol_table("")` (epsilon/unknown/identity at 0/1/2), create a
> `StateMap state_map`. (2) Add the initial state mapped from external number 0
> via `add_and_map_state(t, 0, state_map)` and set it as start. (3) Read lines
> with `fgets(line, 255, ifile)` until EOF. For each line: if it begins with '-'
> (transducer separator), return `t` immediately. Otherwise parse up to 5
> whitespace/tab-separated tokens with `sscanf(line, "%s\t%s\t%s\t%s\t%s", ...)`
> giving count `n`. Set `weight = 0`; if `n == 2`, `weight = atof(a2)`; if `n ==
> 5`, `weight = atof(a5)`. (4) If `n == 1` or `n == 2` (final-state line):
> `final_number = atoi(a1)`, map it via `add_and_map_state`, and
> `t->SetFinal(final_state, weight)`. (5) Else if `n == 4` or `n == 5`
> (transition line): map `atoi(a1)` -> origin and `atoi(a2)` -> target via
> `add_and_map_state`; get `input_number = st.AddSymbol(a3)` and `output_number =
> st.AddSymbol(a4)`; add `LogArc(input_number, output_number, weight,
> target_state)` from origin. (6) Else throw `NotValidAttFormatException` with the
> line as message (HFST_THROW_MESSAGE). (7) After EOF, install `st` via
> `t->SetInputSymbols(&st)` and return `t`. The initial state must be numbered 0
> in the input.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.recode-symbol-numbers-fn]
> void

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.recode-symbol-numbers-fn]
> `recode_symbol_numbers(LogFst *t, NumberNumberMap &km)`: rewrites every arc's
> labels in place according to the number-to-number map `km`. Iterates all
> states with a `StateIterator<LogFst>`; for each state, iterates its arcs with a
> `MutableArcIterator<LogFst>`. For each arc it builds a new LogArc with `ilabel
> = km[arc.ilabel]`, `olabel = km[arc.olabel]`, and unchanged weight and
> nextstate, then writes it back via `aiter.SetValue(new_arc)`. No return value.
> Note: `km[label]` uses operator[], so labels absent from the map are mapped to
> 0.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.remove-epsilons-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.remove-epsilons-fn]
> `remove_epsilons(LogFst *t)`: returns a new epsilon-free LogFst by
> materializing OpenFst's lazy `RmEpsilonFst<LogArc>(*t)` into `new
> LogFst(...)`. Does not mutate `t`; the returned transducer is heap-allocated.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.remove-from-alphabet-fn]
> void

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.remove-from-alphabet-fn]
> `remove_from_alphabet(LogFst *t, const std::string &symbol)`: rebuilds the
> input symbol table without `symbol`, preserving the remaining symbols' numeric
> labels. Asserts `t->InputSymbols() != NULL`. Constructs a local
> `fst::SymbolTable st(t->InputSymbols()->Name())`. Iterates the existing input
> symbol table; for every entry whose `Symbol()` is not equal to `symbol`, adds
> it to `st` at its original label via `st.AddSymbol(it->Symbol(),
> it->Label())`. Installs the rebuilt table via `t->SetInputSymbols(&st)`. No
> return value. Does not modify arcs.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.remove-symbol-table-fn]
> void

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.remove-symbol-table-fn]
> `remove_symbol_table(LogFst *t)`: clears the transducer's input symbol table
> by calling `t->SetInputSymbols(NULL)`. No return value.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.repeat-le-n-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.repeat-le-n-fn]
> `repeat_le_n(LogFst *t, unsigned int n)`: returns a new LogFst recognizing
> between 0 and `n` (inclusive) concatenated copies of `t`. If `n == 0`, returns
> `create_epsilon_transducer()`. Otherwise allocates `repetition =
> create_epsilon_transducer()` and loops `n` times: build `optional_t =
> optionalize(t)` (i.e. `t` made optional / union with epsilon), `Concat(
> repetition, *optional_t)`, then `delete optional_t`. After the loop copies
> `t`'s input symbol table onto `repetition` via `SetInputSymbols` and returns
> it.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.repeat-n-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.repeat-n-fn]
> `repeat_n(LogFst *t, unsigned int n)`: returns a new LogFst recognizing exactly
> `n` concatenated copies of `t`. If `n == 0`, returns
> `create_epsilon_transducer()` (accepts only the empty string). Otherwise
> allocates `repetition = create_epsilon_transducer()` and concatenates `t` onto
> it `n` times via `Concat(repetition, *t)` in a loop. Then copies `t`'s input
> symbol table onto `repetition` via `SetInputSymbols(t->InputSymbols())` and
> returns it.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.repeat-plus-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.repeat-plus-fn]
> `repeat_plus(LogFst *t)`: returns a new LogFst recognizing the Kleene plus
> (one-or-more) closure of `t` by materializing `ClosureFst<LogArc>(*t,
> CLOSURE_PLUS)` into `new LogFst(...)`. Does not mutate `t`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.repeat-star-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.repeat-star-fn]
> `repeat_star(LogFst *t)`: returns a new LogFst recognizing the Kleene star of
> `t` by materializing `ClosureFst<LogArc>(*t, CLOSURE_STAR)` into `new
> LogFst(...)`. Does not mutate `t`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.represent-empty-transducer-as-having-one-state-fn]
> void

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.represent-empty-transducer-as-having-one-state-fn]
> `represent_empty_transducer_as_having_one_state(LogFst *t)`: if `t` has no
> start state (`t->Start() == fst::kNoStateId`) or zero states (`t->NumStates()
> == 0`), `delete t` and set the local pointer `t` to
> `create_empty_transducer()`. No return value. NOTE: `t` is passed by value, so
> reassigning the local does NOT propagate the new transducer to the caller; only
> the side effect of deleting the original is observable (the function is
> effectively a no-op for non-empty transducers and frees-and-leaks for empty
> ones).

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.reverse-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.reverse-fn]
> `reverse(LogFst *t)`: returns a new LogFst that is the reversal of `t`.
> Allocates `reversed = new LogFst`, calls `Reverse<LogArc, LogArc>(*t,
> reversed)` (OpenFst reversal into `reversed`), copies `t`'s input symbol table
> onto `reversed` via `SetInputSymbols(t->InputSymbols())`, and returns it. Does
> not mutate `t`.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.set-final-weight-fn]
> void

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.set-final-weight-fn]
> `set_final_weight(LogFst *t, StateId s, float w)`: sets state `s`'s final
> weight to `w` via `t->SetFinal(s, w)`. No return value.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.set-final-weights-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.set-final-weights-fn]
> `set_final_weights(LogFst *t, float weight)`: iterates all states of `t`; for
> each state whose final weight is non-Zero (`t->Final(s) != LogWeight::Zero()`,
> i.e. a final state), resets its final weight to `weight` via `t->SetFinal(s,
> weight)`. Non-final states are left untouched. Returns `t` (mutated in place).

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.set-weight-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.set-weight-fn]
> `set_weight(LogFst *t, float f)`: returns a new LogFst that is a copy of `t`
> with every final state's final weight set to `f`. Deep-copies `t` into `t_copy
> = new LogFst(*t)`, iterates all states of `t` (with a `StateIterator`); for
> each state that is final in `t_copy` (`t_copy->Final(iter.Value()) !=
> LogWeight::Zero()`), sets its final weight to `f` via `t_copy->SetFinal(...,
> f)`. Returns `t_copy`. Does not mutate `t` itself.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.state-map]
> typedef std::map<int, StateId> StateMap

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.substitute-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.substitute-fn]
> `substitute(LogFst *t, const StringPair old_symbol_pair, LogFst *transducer)`:
> replaces every arc in `t` labeled `old_symbol_pair` with an embedded copy of
> `transducer`, in place, returning `t`. Asserts `t->InputSymbols() != NULL` and
> copies the input table into `st`. Captures `states = t->NumStates()` and loops
> `i` from 0 to states-1, iterating each state's arcs with a
> `MutableArcIterator`. For an arc whose ilabel equals
> `st->AddSymbol(old_symbol_pair.first)` and olabel equals
> `st->AddSymbol(old_symbol_pair.second)`: record `destination_state =
> arc.nextstate`, allocate `start_state = t->AddState()`, rewrite the matched arc
> to an epsilon arc (ilabel=0, olabel=0, weight unchanged) pointing to
> `start_state` via `SetValue`. Then add `transducer->NumStates() - 1` more
> states to `t` (a loop `j` from 1, so `transducer`'s state `k` corresponds to
> `t` state `k + start_state`). Iterate all states of `transducer`: if a state is
> final, add to `t` an epsilon arc `LogArc(0, 0, transducer->Final(state),
> destination_state)` from `state + start_state` (copying the final weight onto
> the epsilon transition back to the original destination); and for each arc of
> `transducer`, copy it into `t` as `LogArc(ilabel, olabel, weight, nextstate +
> start_state)` from `state + start_state`. After processing all states, install
> `st` via `t->SetInputSymbols(st)`, `delete st`, return `t`. (A sibling
> NumberPair overload behaves identically but matches numeric labels directly and
> omits symbol-table manipulation.)

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.subtract-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.subtract-fn]
> `subtract(LogFst *t1, LogFst *t2)`: returns a new LogFst recognizing the
> difference (language of t1 minus language of t2). Steps: (1) For each of t1,
> t2, if its output symbols are NULL set them to its input symbols. (2) Arc-sort
> t1 by output label (`OLabelCompare<LogArc>`) and t2 by input label
> (`ILabelCompare<LogArc>`). (3) Remove epsilons in place via `RmEpsilon(t1)` and
> `RmEpsilon(t2)`. (4) Build a shared `EncodeMapper<LogArc> encoder(0x0003,
> ENCODE)` (encodes both labels and weights — t2 must be unweighted), wrap each
> as `EncodeFst` (`enc1`, `enc2`) then `DeterminizeFst` (`det1`, `det2`).
> (5) Allocate `difference = new LogFst()`, run `Difference(det1, det2,
> difference)`, build `DecodeFst<LogArc> subtract(*difference, encoder)`, `delete
> difference`. (6) Materialize `result = new LogFst(subtract)`, set its input
> symbols to `t1->InputSymbols()`, and return it. Mutates t1 and t2 (sorted,
> epsilon-removed, output symbols set). Contains a debug flag (false) that gates
> diagnostic printfs.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.test-minimize-fn]
> void

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.test-minimize-fn]
> `test_minimize(void)`: a self-contained debug/test routine (no parameters, no
> return). Builds a tiny LogFst: `t = new LogFst`, adds `initial` state set as
> start, adds `state` set final with weight 0.5, adds arc `LogArc(1,1,0.5,state)`
> from `initial` and arc `LogArc(2,2,0.5,initial)` from `state`. Prints it to
> stderr via `print_att_number(t, stderr)`. Then builds `RmEpsilonFst<LogArc>
> t_rm_eps(*t)`, an `EncodeMapper<LogArc> encode_mapper(0x0001, ENCODE)` (labels
> only), `EncodeFst t_rm_eps_enc(t_rm_eps, &encode_mapper)`, `DeterminizeFst
> t_DET(t_rm_eps_enc)`, a `DecodeFst dec(t_DET, encode_mapper)` (unused), and
> materializes `LogFst t_det_std(t_DET)`. Calls `Minimize<LogArc>(&t_det_std)`,
> then prints the minimized result to stderr via `print_att_number`. The final
> `Decode` step is commented out. Leaks `t` (never deleted).

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.transform-weights-fn]
> LogFst *

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.transform-weights-fn]
> `transform_weights(LogFst *t, float (*func)(float f))`: applies the function
> `func` to every weight in `t` in place. Iterates all states; for each state
> `s`, if it is final (`t->Final(s) != LogWeight::Zero()`), replaces its final
> weight with `func(t->Final(s).Value())` via `t->SetFinal`. Then iterates the
> state's arcs with a `MutableArcIterator`; for each arc builds a new LogArc with
> the same ilabel/olabel/nextstate but weight `func(arc.weight.Value())`, and
> writes it back with `SetValue`. Returns `t` (mutated in place).

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.write-in-att-format-fn]
> void

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.write-in-att-format-fn]
> `write_in_att_format(LogFst *t, std::ostream &os)`: writes `t` in AT&T text
> format to `os`, printing the initial state first as number 0. Gets `sym =
> t->InputSymbols()` (asserts non-null). Computes the state-number swap: set
> `initial_state = t->Start()`; if `initial_state != 0`, set `zero_print =
> initial_state`, else `zero_print = 0`. A helper renumbering rule maps any state
> id `x` to: `zero_print` if `x == 0`, `0` if `x == initial_state`, else `x`.
> Pass 1: iterate states, and for the one equal to `initial_state` (then
> `break`), compute its printed `origin` via the rule and for each outgoing arc
> compute printed `target` via the rule, writing one line `origin \t target \t
> sym->Find(ilabel) \t sym->Find(olabel) \t weight.Value() \n`; afterward if that
> state is final write `origin \t finalweight.Value() \n`. Pass 2: iterate all
> states except `initial_state`, doing the same arc and final-weight printing.
> No return value.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.log-weight-transducer.write-in-att-format-number-fn]
> void

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.log-weight-transducer.write-in-att-format-number-fn]
> `write_in_att_format_number(LogFst *t, std::ostream &os)`: identical in
> structure to `write_in_att_format` (same initial-state/zero swap renumbering,
> same two passes — first the initial state then `break`, then all other states),
> except it prints arc labels as raw numbers rather than symbol strings: each arc
> line is `origin \t target \t \\ilabel \t \\olabel \t weight.Value() \n` (the
> backslash-prefixed numeric labels, not looked up in any symbol table). Final
> states are written as `origin \t finalweight.Value() \n`. No symbol table is
> consulted; no return value.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.openfst-log-set-hopcroft-fn]
> void

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.openfst-log-set-hopcroft-fn]
> `openfst_log_set_hopcroft(bool value)`: assigns `value` to the file-scope
> global boolean `openfst_log_use_hopcroft` (which defaults to false). Pure
> setter; no return value. This flag selects whether Hopcroft's minimization
> algorithm is used.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.print-att-number-fn]
> void

> [spec:hfst:sem:log-weight-transducer.hfst.implementations.print-att-number-fn]
> File-scope free function `print_att_number(LogFst *t, FILE *ofile)`: prints a
> debug AT&T-style dump using raw state ids and numeric labels (no renumbering,
> no symbol-table lookup). First writes `initial state: <Start>\n` via fprintf.
> Then iterates all states with a `StateIterator`; for each state `s`: if it is
> final (`t->Final(s) != LogWeight::Zero()`), write `s \t finalweight.Value()\n`;
> then for each outgoing arc write `s \t arc.nextstate \t arc.ilabel \t
> arc.olabel \t arc.weight.Value()\n`. No return value.

> [spec:hfst:def:log-weight-transducer.hfst.implementations.state-id]
> typedef unsigned int StateId

> [spec:hfst:def:log-weight-transducer.int64]
> typedef __int64 int64

> [spec:hfst:def:log-weight-transducer.main-fn]
> int

> [spec:hfst:sem:log-weight-transducer.main-fn]
> `main(int argc, char *argv[])`: unit-test entry point compiled only when
> MAIN_TEST is defined. If `HAVE_OPENFST_LOG` is set: prints "Unit tests for
> <file>:", constructs a `LogWeightTransducer ofst`, calls
> `ofst.create_empty_transducer()` and deletes the result, then
> `ofst.create_epsilon_transducer()` and deletes it, prints a newline and "ok",
> and returns `EXIT_SUCCESS`. Otherwise prints a "Skipping unit tests ...
> LogWeightTransducer has not been enabled" message and returns 77 (the
> automake "test skipped" exit code).

