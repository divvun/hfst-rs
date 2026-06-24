# libhfst/src/implementations/TropicalWeightTransducer.cc, libhfst/src/implementations/TropicalWeightTransducer.h

> [spec:hfst:def:tropical-weight-transducer.hfst.get-encode-weights-fn]
> bool get_encode_weights()

> [spec:hfst:sem:tropical-weight-transducer.hfst.get-encode-weights-fn]
> Free function in namespace `hfst`. Here it is only a forward declaration;
> the actual definition lives in HfstTransducer.cc. It returns the value of
> the module-level boolean `encode_weights` flag (the companion setter
> `set_encode_weights(bool)` assigns it). No arguments, no side effects.
> Callers use it to decide whether OpenFst encoding should include weights
> (`kEncodeLabels|kEncodeWeights`) or labels only (`kEncodeLabels`).

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.extract-paths-fn]
> static bool

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.extract-paths-fn]
> File-static recursive depth-first path enumerator. Params: transducer `t`,
> current state `s`, `all_visitations` and `path_visitations` maps (passed BY
> VALUE so each recursion gets an independent copy of the path-visitation
> counts), accumulated `weight_sum`, a `callback` (hfst::ExtractStringsCb),
> `cycles` limit, optional `fd_state_stack` (flag-diacritic states), `filter_fd`
> bool, and `spv` (StringPairVector, the path symbols accumulated so far,
> passed by reference).
> Step 1: if `cycles >= 0` and `path_visitations[s] > cycles`, return true
> (prune this branch but keep searching). Otherwise increment
> `all_visitations[s]` and `path_visitations[s]`.
> Step 2: if `spv` is non-empty, compute `final = (t->Final(s) != Zero)`, build
> an HfstTwoLevelPath with weight `weight_sum + (final ? Final(s).Value() : 0)`
> and symbol list `spv`, and invoke `callback(path, final)`. If the returned
> RetVal has `!continueSearch || !continuePath`, decrement `path_visitations[s]`
> and return `ret.continueSearch`.
> Step 3: collect this state's out-arcs into a vector sorted ascending by the
> `all_visitations` count of each arc's target state (insertion sort: find first
> index whose target's visitation count exceeds the new arc's, shift right,
> insert).
> Step 4: iterate the sorted arcs while a running `res` stays true. For each
> arc: if `fd_state_stack` is set and the arc's ilabel is a flag-diacritic
> operation, push a copy of the top fd-state, apply the operation; if it fails,
> pop and `continue` (skip this arc). Build `istring`/`ostring`: each is the
> symbol-table name for the arc's ilabel/olabel, unless `filter_fd` is true and
> that label is a flag-diacritic operation (in which case the empty string is
> used). Push `StringPair(istring,ostring)` onto `spv`, recurse into
> `arc.nextstate` with `weight_sum + arc.weight.Value()`, pop `spv` back, and
> pop the pushed fd-state if one was added.
> Step 5: decrement `path_visitations[s]` and return `res`.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.initialize-symbol-tables-fn]
> void initialize_symbol_tables(StdVectorFst *t)

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.initialize-symbol-tables-fn]
> Namespace-scope forward declaration `void initialize_symbol_tables(StdVectorFst
> *t)` in `hfst::implementations`. It has no free-function definition; all calls
> resolve to the static member `TropicalWeightTransducer::initialize_symbol_tables`.
> That implementation builds a fresh symbol table via `create_symbol_table("")`
> (which registers internal_epsilon=0, internal_unknown=1, internal_identity=2)
> and assigns it to the transducer as its input symbol table
> (`t->SetInputSymbols(&st)`), giving `t` the standard HFST special-symbol table.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.is-minimal-and-empty-fn]
> static bool

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.is-minimal-and-empty-fn]
> File-static helper. Reads `t->Start()`; if the start state is < 0 (no start
> state), returns true. Otherwise it opens an arc iterator over the start state
> and, inside the loop body, immediately returns false on the first arc seen
> (i.e., if the start state has any out-arc, the transducer is not minimal-empty).
> If the loop body never executes (start state has zero out-arcs), returns true.
> Net effect: returns true iff the transducer has no start state, or its start
> state has no outgoing transitions.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.label-pair]
> typedef std::pair<int, int> LabelPair

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.label-pair-vector]
> typedef std::vector<LabelPair> LabelPairVector

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.openfst-tropical-set-hopcroft-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.openfst-tropical-set-hopcroft-fn]
> Setter that assigns its `bool value` argument to the module-level flag
> `openfst_tropical_use_hopcroft` (initialised to false). No return, no other
> side effects. The flag later selects the Hopcroft minimization algorithm.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.print-att-number-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.print-att-number-fn]
> Debug printer that writes transducer `t` to `FILE *ofile` in a numeric AT&T-like
> format. First prints `initial state: <Start()>\n`. Then iterates all states
> (StateIterator). For each state `s`: if `t->Final(s) != TropicalWeight::Zero()`,
> prints `"%i\t%f\n"` with the state id and the final weight value. Then iterates
> the state's arcs (ArcIterator) and for each arc prints
> `"%i\t%i\t%i\t%i\t%f\n"` = source state, arc.nextstate, arc.ilabel, arc.olabel,
> arc.weight.Value(). No return value; pure I/O to `ofile`.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.random-path-fn]
> static HfstTwoLevelPath

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.random-path-fn]
> File-static `random_path_(StdVectorFst *t)` returning a single HfstTwoLevelPath
> (pair of weight and StringPair vector). Uses `rand()`.
> Step 1: if `is_minimal_and_empty(t)` throw the C-string "transducer is empty".
> Step 2: init `path.first = 0`, `current_state = t->Start()`. Compute
> `is_epsilon_path_accepted = (t->Final(Start()) != Zero)`. Init `last_index = 0`
> (the length of the longest accepting prefix found). Allocate two int vectors
> `visited` and `broken`, one slot per state (NumStates()), all zero.
> Step 3: loop forever. Mark `visited[current_state] = 1`. Collect all out-arcs
> of `current_state` into `t_transitions`.
> Step 4: if `t_transitions` is empty OR `broken[current_state]`, truncate
> `path.second` down to `last_index` entries (pop_back while size > last_index);
> if `!is_epsilon_path_accepted && path.second` is empty, throw "cannot extract
> random path"; otherwise return `path`.
> Step 5: inner loop over a random ordering: pick a random `index` in
> `t_transitions`, take that arc, erase it. Let `t_target = arc.nextstate`. Push
> `StringPair(InputSymbols()->Find(ilabel), InputSymbols()->Find(olabel))` onto
> `path.second` and add `arc.weight.Value()` to `path.first`. If `t_target` is
> final: with probability 1/4 (`rand()%4 == 0`) add `Final(t_target).Value()` to
> `path.first`, then (if `!is_epsilon_path_accepted && empty`) throw, else return
> `path`; otherwise set `last_index` to current `path.second.size()`. Then, to
> bias toward shorter paths: if `broken[t_target]==0` and `visited[t_target]==1`,
> with prob 1/4 set `broken[t_target]=1`; and again if `visited[t_target]==1`
> with prob 1/4 set `broken[t_target]=1`. Set `current_state = t_target` and
> `break` out of the inner loop (only one transition is followed per step).
> The code after the outer loop (unreachable) also throws "cannot extract random
> path" when no symbols and epsilon path not accepted, else returns `path`.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.state-id]
> typedef unsigned int StateId

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.state-map]
> typedef std::map<int, StateId> StateMap

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.std-arc-less-than]
> struct StdArcLessThan

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.std-arc-less-than.operator-fn]
> bool operator() (const StdArc &arc1,const StdArc &arc2) const

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.std-arc-less-than.operator-fn]
> Comparator method `bool operator()(const StdArc &arc1, const StdArc &arc2)
> const` on struct `StdArcLessThan`. It is only DECLARED in the header; no
> definition exists anywhere in the codebase, and the struct is never used.
> A faithful port may leave it as a declared-but-unimplemented strict-weak
> ordering over StdArc, or omit it.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.std-arc-vector]
> typedef std::vector<StdArc> StdArcVector

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream]
> class TropicalWeightInputStream {
>   std::string filename;
>   std::ifstream i_stream;
>   std::istream &input_stream;
> }

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.close-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.close-fn]
> If `filename != string()` (i.e. a non-empty filename, meaning this stream wraps
> a file rather than stdin), closes the underlying `i_stream` (`i_stream.close()`).
> For a stdin-backed stream (empty filename) it does nothing.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.ignore-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.ignore-fn]
> Discards the next `n` bytes from the input stream by calling
> `input_stream.ignore(n)`. No return value.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.is-bad-fn]
> bool

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.is-bad-fn]
> Const. If `filename` is empty (stdin-backed), returns `std::cin.bad()`;
> otherwise returns `input_stream.bad()`. Reports whether the stream's badbit is
> set.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.is-eof-fn]
> bool

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.is-eof-fn]
> Const. Returns `input_stream.peek() == EOF`, i.e. true iff peeking the next
> character yields end-of-file. Peek does not consume input.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.is-fst-fn]
> bool

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.is-fst-fn]
> The annotation sits on the `static bool is_fst(FILE *f)` overload, which checks
> whether the stream begins with the OpenFst magic byte `0xd6`. For the `FILE *`
> overload: if `f == NULL` return false; otherwise read one char via `getc(f)`,
> push it back with `ungetc`, and return `c == 0xd6`. The companion
> `is_fst(std::istream &s)` overload returns `s.good() && (s.peek() == 0xd6)`, and
> the no-arg `is_fst()` const member delegates to `is_fst(input_stream)`. None
> consume input permanently (peek/ungetc restore it).

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.is-good-fn]
> bool

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.is-good-fn]
> Const. First, if `is_eof()` is true, returns false immediately. Otherwise, if
> `filename` is empty (stdin), returns `std::cin.good()`; else returns
> `input_stream.good()`. Reports stream usability (not at EOF and goodbit set).

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.operator-fn]
> bool

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.operator-fn]
> `bool operator()(void) const`. Returns `is_good()`, i.e. the stream-state test:
> true when the stream is not at EOF and its goodbit is set. Provides the
> stream's boolean "is this still usable" conversion.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.read-transducer-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.read-transducer-fn]
> Reads one OpenFst transducer from the stream and returns a heap-allocated
> `StdVectorFst *`.
> Step 1: if `is_eof()`, throw `StreamIsClosedException` (via HFST_THROW).
> Step 2: declare a local `FstHeader header`. In a try block: if `filename` is
> empty, call `header.Read(input_stream, "STDIN")` then
> `t = static_cast<StdVectorFst*>(StdVectorFst::Read(input_stream,
> FstReadOptions("STDIN", &header)))`; otherwise use `filename` in both calls
> instead of "STDIN". If the resulting `t == NULL`, throw
> `TransducerHasWrongTypeException`.
> Step 3: any caught HfstException is rethrown unchanged. On success the function
> returns the pointer `t` (ownership passes to caller). The two trailing
> try/catch blocks are no-ops that just return/rethrow.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.skip-hfst-header-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.skip-hfst-header-fn]
> Skips the HFST header preamble: calls `input_stream.ignore(6)` (skip 6 bytes),
> then `skip_identifier_version_3_0()` which skips a further 19 bytes (the
> "TROPICAL_OFST_TYPE" identifier). No return value.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.skip-identifier-version-3-0-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.skip-identifier-version-3-0-fn]
> Skips the 19-byte identifier string "TROPICAL_OFST_TYPE" by calling
> `input_stream.ignore(19)`. No return value.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.stream-get-fn]
> char

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.stream-get-fn]
> Reads and consumes one byte: returns `(char)input_stream.get()`.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.stream-get-short-fn]
> short

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.stream-get-short-fn]
> Reads a raw `short` (`sizeof(short)` bytes, native byte order) directly from the
> stream: declares a local `short i`, calls `input_stream.read((char*)&i,
> sizeof(i))`, and returns `i`.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.stream-unget-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.stream-unget-fn]
> Pushes character `c` back onto the input stream via `input_stream.putback(c)`,
> so it will be the next byte read. No return value.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.tropical-weight-input-stream-fn]
> TropicalWeightInputStream::TropicalWeightInputStream(

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-input-stream.tropical-weight-input-stream-fn]
> Filename constructor `TropicalWeightInputStream(const std::string &filename_)`.
> Member-initialises `filename` to a copy of `filename_`, opens the member
> `i_stream` on `filename.c_str()` in `std::ios::in | std::ios::binary` mode, and
> binds the `input_stream` reference to that `i_stream`. Empty body. (There are
> also a default constructor binding `input_stream` to `std::cin` with empty
> filename, and an `istream&` constructor binding `input_stream` to the supplied
> stream.)

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-output-stream]
> class TropicalWeightOutputStream {
>   std::string filename;
>   std::ofstream o_stream;
>   std::ostream &output_stream;
>   bool hfst_format;
> }

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-output-stream.close-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-output-stream.close-fn]
> If `filename != string()` (non-empty filename, i.e. file-backed rather than
> stdout), closes the underlying `o_stream` (`o_stream.close()`). For a
> stdout-backed stream (empty filename) it does nothing.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-output-stream.tropical-weight-output-stream-fn]
> TropicalWeightOutputStream::TropicalWeightOutputStream(bool hfst_format)

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-output-stream.tropical-weight-output-stream-fn]
> Constructor `TropicalWeightOutputStream(bool hfst_format)` for stdout output.
> Member-initialises `filename` to empty `std::string()`, binds the
> `output_stream` reference to `std::cout`, and stores `hfst_format`. In the body,
> if `!output_stream` (stream in failed state), prints to stderr:
> "TropicalWeightOutputStream: ERROR: failbit set (3).\n". (A sibling constructor
> taking `(const std::string &str, bool hfst_format)` instead opens the member
> `o_stream` on `str` in out|binary mode and binds `output_stream` to it.)

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-output-stream.write-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-output-stream.write-fn]
> Writes a single character to the output stream: `output_stream.put(char(c))`.
> No return value.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-output-stream.write-transducer-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-output-stream.write-transducer-fn]
> Writes the OpenFst transducer `transducer` to the output stream.
> Step 1: if `!output_stream` (failed state), print to stderr
> "TropicalWeightOutputStream: ERROR: failbit set (1).\n".
> Step 2: if `transducer->InputSymbols() == NULL`, print to stderr
> "### Missing Input Symbol Table when writing! ###\n".
> Step 3: declare `fst::SymbolTable *output_st = NULL`. If `hfst_format` is false
> (raw OpenFst backend format, which includes both input and output symbol
> tables), allocate `output_st` as a copy of the input symbol table and set it as
> the transducer's output symbol table (`SetOutputSymbols`).
> Step 4: call `transducer->Write(output_stream, FstWriteOptions())`.
> Step 5: if `output_st != NULL`, delete it.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer]
> class TropicalWeightTransducer {
>   static std::ostream * warning_stream;
> }

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-and-map-state-fn]
> StateId

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-and-map-state-fn]
> Static. Params: transducer `t`, `int state_number` (an external/AT&T state id),
> and `StateMap &state_map` (map from external int -> internal StateId). Looks up
> `state_number` in `state_map`. If not found, it creates a new state with
> `t->AddState()`, inserts the pair `(state_number, new_state_id)` into
> `state_map`, and returns the new StateId. If already present, returns the
> existing mapped StateId without adding a state. Ensures each external state
> number maps to exactly one internal state.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-state-fn]
> StateId

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-state-fn]
> Static. Adds a new state to `t` via `t->AddState()` getting StateId `s`. If
> `s == 0` (this is the very first state added), it sets that state as the start
> state (`t->SetStart(s)`). Returns `s`.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-sub-trie-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-sub-trie-fn]
> Static, recursive. Params: mutable target `fst::StdVectorFst &t1` with current
> state `t1_state`, source `const fst::StdVectorFst *t2` with current state
> `t2_state`. Copies the subtree of `t2` rooted at `t2_state` into `t1` under
> `t1_state` as a trie.
> Step 1: if `t2->Final(t2_state) != TropicalWeight::Zero()`, set t1's final
> weight at `t1_state` to `Plus(t1.Final(t1_state), t2->Final(t2_state))`
> (tropical-semiring min of the existing and source final weights).
> Step 2: for each arc out of `t2_state` in `t2`: create a brand-new state in
> `t1` (`t1.AddState()`), add an arc from `t1_state` to that new state with the
> same ilabel, olabel, and weight (`t1.AddArc(t1_state, StdArc(...))`), then
> recurse `add_sub_trie(t1, new_state, t2, arc.nextstate)`. Each source arc thus
> always produces a fresh branch (no state sharing).

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-to-weights-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-to-weights-fn]
> Static. Adds float `w` to every weight in transducer `t` in place. Iterates all
> states (StateIterator). For each state `s`: iterates its arcs with a
> MutableArcIterator; for each arc, builds a new StdArc copying ilabel, olabel,
> nextstate and setting `weight = arc.weight.Value() + w`, then writes it back via
> `aiter.SetValue(new_arc)`. After the arcs, if `t->Final(s) != TropicalWeight::
> Zero()` (state is final), sets the final weight to `old_final_weight + w`. No
> return value; mutates `t` directly.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-transition-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.add-transition-fn]
> Static. Adds an arc to `t` from `source` to `target` labelled by string symbols.
> Params: transducer `t`, `StateId source`, `std::string &isymbol`,
> `std::string &osymbol`, float `w`, `StateId target`.
> Step 1: copy the transducer's input symbol table (`st = t->InputSymbols()->Copy()`).
> Step 2: obtain (adding if necessary) the symbol numbers `ilabel = st->AddSymbol(isymbol)`
> and `olabel = st->AddSymbol(osymbol)` in that copied table.
> Step 3: add the arc `StdArc(ilabel, olabel, w, target)` from `source` to `t`.
> Step 4: install the updated table as `t`'s input symbol table
> (`t->SetInputSymbols(st)`), then delete the local copy `st`. No return value.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.are-equivalent-fn]
> bool

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.are-equivalent-fn]
> Static. Tests whether two transducers `a_` and `b_` accept the same weighted
> relation. Returns bool.
> Step 1: make working copies `a = copy(a_)`, `b = copy(b_)` (originals untouched).
> Step 2: run `CHECK_EPSILON_CYCLES` on both (raises if epsilon cycles present),
> then remove epsilons from both (`RmEpsilon<StdArc>`).
> Step 3: build an `EncodeMapper<StdArc>` in ENCODE mode with flags
> `kEncodeLabels|kEncodeWeights` if `hfst::get_encode_weights()` else `kEncodeLabels`,
> and `Encode` both `a` and `b` with it (turns the arc relation into an acceptor).
> Step 4: determinize both into fresh `deta`, `detb` (`Determinize<StdArc>`), then
> delete `a` and `b`.
> Step 5: compute `retval = Equivalent(*deta, *detb)` (OpenFst equivalence on the
> two deterministic encoded acceptors), delete `deta` and `detb`, and return `retval`.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.compose-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.compose-fn]
> Static. Composes transducers `t1` and `t2` (the output side of `t1` matched
> against the input side of `t2`). Returns a new heap `StdVectorFst *`.
> Step 1: make a re-tabled copy `t2_ = expand_arcs(t2, foo, false)` with an empty
> `StringSet foo` and `unknown_symbols_in_use=false` — this exists only so its
> symbol-table check sum matches `t1`'s (OpenFst Compose requires matching tables).
> Step 2: set `t1`'s output symbol table to its own input symbol table
> (`t1->SetOutputSymbols(t1->InputSymbols())`), and set `t2_`'s input symbol table
> to `t1`'s output symbol table.
> Step 3: arc-sort `t1` by output label (`StdOLabelCompare`) and `t2_` by input
> label (`StdILabelCompare`).
> Step 4: allocate `result`, call `Compose(*t1, *t2_, result)`, then delete `t2_`.
> Step 5: reset `t1`'s output symbols to NULL, set `result`'s input symbols to
> `t1->InputSymbols()`, and return `result`.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.compose-intersect-fn]
> static StdVectorFst * compose_intersect(StdVectorFst * t,

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.compose-intersect-fn]
> Static method declared in the header as
> `static StdVectorFst * compose_intersect(StdVectorFst *t, Grammar *grammar)`,
> but the declaration is guarded by `#ifdef FOO` and is never compiled. No
> definition exists anywhere. A faithful port may omit it; there is no behaviour
> to reproduce.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.concatenate-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.concatenate-fn]
> Static. Returns a new heap `StdVectorFst *` equal to `t1` followed by `t2`.
> Copy-constructs `result = new StdVectorFst(*t1)`, sets its input symbol table to
> `t1->InputSymbols()`, calls OpenFst `Concat(result, *t2)` (which appends `t2`
> onto `result` in place), and returns `result`. `t1` and `t2` are not modified.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.copy-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.copy-fn]
> Static. Returns a deep copy of `t` as a freshly heap-allocated transducer:
> `return new StdVectorFst(*t);` (OpenFst copy constructor, which also copies the
> symbol tables). Caller owns the result.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.create-empty-transducer-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.create-empty-transducer-fn]
> Static, no args. Allocates a new `StdVectorFst`, calls
> `initialize_symbol_tables(t)` (installs the standard epsilon/unknown/identity
> symbol table), adds one state `s = t->AddState()`, sets it as the start state
> (`t->SetStart(s)`), and returns `t`. The single state is non-final, so the
> transducer recognizes the empty language.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.create-epsilon-transducer-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.create-epsilon-transducer-fn]
> Static, no args. Like create_empty_transducer but the single state is final:
> allocates `StdVectorFst`, calls `initialize_symbol_tables(t)`, adds one state
> `s`, sets it as start (`SetStart(s)`) AND final with weight 0 (`SetFinal(s, 0)`),
> and returns `t`. The result recognizes exactly the empty string (epsilon).

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.create-mapping-fn]
> NumberNumberMap

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.create-mapping-fn]
> Static. Builds the number-to-number remapping needed so that `t1`'s symbol
> numbering matches `t2`'s. Precondition: `t2`'s symbol table contains every
> symbol of `t1`'s table. Returns a `NumberNumberMap km`.
> Iterates every entry of `t1->InputSymbols()`. For each entry with label `L` and
> symbol string `S`, sets `km[(unsigned int)L] = (unsigned int)t2->InputSymbols()
> ->Find(S)` (the number `t2` uses for that same symbol). Asserts `L >= 0` and that
> `t2`'s Find did not return < 0. Returns `km`. Does not mutate either transducer.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.create-symbol-table-fn]
> fst::SymbolTable

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.create-symbol-table-fn]
> Static. Builds and returns (by value) a `fst::SymbolTable` named `name`,
> pre-populated with the three HFST special symbols at fixed numbers:
> `internal_epsilon`->0, `internal_unknown`->1, `internal_identity`->2 (each added
> via `st.AddSymbol(symbol, number)`). Returns the table.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.define-transducer-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.define-transducer-fn]
> The annotation sits on the overload `define_transducer(const StringPairSet &sps,
> bool cyclic)`. It builds a single-arc-bundle transducer recognizing the union of
> the symbol pairs in `sps` (one transition for each pair).
> Step 1: allocate `StdVectorFst t`, create the standard symbol table via
> `create_symbol_table("")`. Add the start state `s1` and `SetStart(s1)`. Init the
> final state `s2 = s1`.
> Step 2: if `sps` is non-empty: when `!cyclic`, add a separate final state
> `s2 = t->AddState()` (so the arcs go start->final); when `cyclic`, leave
> `s2 = s1` (arcs loop back on the start state). For each pair `(first, second)`
> (asserting neither string is empty) add an arc from `s1` to `s2` with
> ilabel `st.AddSymbol(first)`, olabel `st.AddSymbol(second)`, weight 0.
> Step 3: `SetFinal(s2, 0)`, set `t`'s input symbols to `st`, return `t`.
> (There are sibling overloads taking unsigned numbers, a StringPairVector, a
> std::vector<StringPairSet>, NumberPair containers, etc., each building the
> analogous single-path or branching transducer.)

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.delete-transducer-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.delete-transducer-fn]
> Static. Frees the transducer: `delete t;`. No return value. In a Rust port this
> is just dropping the owned value.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.determinize-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.determinize-fn]
> Static. Determinizes `t`, returning a new heap `StdVectorFst *`.
> Step 1: `CHECK_EPSILON_CYCLES(t, "determinize")`, then `RmEpsilon<StdArc>(t)`
> (mutates `t` in place).
> Step 2: compute `w = get_smallest_weight(t)`; if `w < 0`, shift all weights up by
> `-w` via `add_to_weights(t, -w)` so weights are non-negative (OpenFst
> determinization requires this).
> Step 3: build an `EncodeMapper<StdArc>` in ENCODE mode with flags
> `kEncodeLabels|kEncodeWeights` if `hfst::get_encode_weights()` else `kEncodeLabels`;
> `Encode(t, &encode_mapper)`.
> Step 4: allocate `det`, `Determinize<StdArc>(*t, det)`, then `Decode(det,
> encode_mapper)` to restore labels/weights.
> Step 5: if `w < 0`, undo the earlier shift on the result via
> `add_to_weights(det, w)`. Return `det`.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.disjunct-as-tries-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.disjunct-as-tries-fn]
> Static, recursive. Merges the trie rooted at `t2_state` in `t2` into the trie
> rooted at `t1_state` in mutable `t1`, sharing common prefixes.
> Step 1: if `t2->Final(t2_state) != TropicalWeight::Zero()`, set t1's final weight
> at `t1_state` to `Plus(t1.Final(t1_state), t2->Final(t2_state))` (tropical min).
> Step 2: for each arc out of `t2_state`: look up an existing matching arc in `t1`
> at `t1_state` with the same ilabel/olabel via `has_arc(t1, t1_state, ilabel,
> olabel)` (returns arc index or -1). If none (-1): create a new state in `t1`,
> add an arc `(ilabel, olabel, weight, new_state)` from `t1_state`, and copy the
> whole source subtree with `add_sub_trie(t1, new_state, t2, arc.nextstate)`
> (no further merging — fresh branch). If a matching arc exists: seek a
> MutableArcIterator to that arc index, read its `nextstate`, and recurse
> `disjunct_as_tries(t1, that_nextstate, t2, arc.nextstate)` to continue merging.
> No return value; mutates `t1`.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.disjunct-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.disjunct-fn]
> The annotation sits on the overload `disjunct(StdVectorFst *t, const
> StringPairVector &spv)`, which adds the single path `spv` into `t` in place
> (sharing the existing prefix), and returns `t`.
> Step 1: copy `t`'s input symbol table into `st`. Set `s = t->Start()`.
> Step 2: for each pair `(first, second)` in `spv`: obtain its numbers
> `inumber = st->AddSymbol(first)`, `onumber = st->AddSymbol(second)`. Scan the
> out-arcs of the current state `s`; if one has matching ilabel==inumber and
> olabel==onumber, set `transition_found=true`, advance `s = a.nextstate`, and stop
> scanning. If no matching arc was found, create a new state, add an arc
> `(inumber, onumber, 0, new_state)` from `s`, and set `s = new_state`.
> Step 3: after the loop, mark the final reached state final (`t->SetFinal(s, 0)`),
> install `st` as `t`'s input symbols, and return `t`.
> (There is also a plain `disjunct(t1, t2)` overload that copies `t1` and applies
> OpenFst `Union(result, *t2)`.)

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.expand-arcs-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.expand-arcs-fn]
> Static. Returns a new `StdVectorFst *` copy of `t` where unknown/identity
> meta-arcs are expanded against the symbol set `unknown`. Params: `t`,
> `StringSet &unknown` (the symbols to expand to), `bool unknown_symbols_in_use`.
> Step 1: allocate `result`; add the same number of states as `t` (one AddState per
> state of `t`), so state ids correspond 1:1.
> Step 2: for each state `s` of `t` (result state `result_s = s`): if it is the
> start state, set result's start; if `t->Final(s) != Zero`, copy the final weight.
> Step 3: for each arc of `s` (with `result_nextstate = arc.nextstate`): if
> `unknown_symbols_in_use`, using `is = t->InputSymbols()`, expand based on the
> arc's labels (numbers: epsilon=0, unknown=1, identity=2). The cases:
>   - ilabel==1 && olabel==1 ("?:?" cross product): for every non-flag symbol `x`
>     in `unknown` (number `inum`), and every non-flag symbol `y` (number `onum`),
>     add `(inum, onum, weight, nextstate)` when `inum != onum`; plus add
>     `(inum, 1, ...)` and `(1, inum, ...)`.
>   - ilabel==2 || olabel==2 ("?:?" identity): for every non-flag symbol `x`
>     (number `n`) add `(n, n, weight, nextstate)`.
>   - ilabel==1 ("?:x"): for every non-flag symbol `x` (number `n`) add
>     `(n, arc.olabel, weight, nextstate)`.
>   - olabel==1 ("x:?"): for every non-flag symbol `x` (number `n`) add
>     `(arc.ilabel, n, weight, nextstate)`.
> Step 4: in ALL cases (including when not expanding) the original arc
> `(arc.ilabel, arc.olabel, arc.weight, result_nextstate)` is also added to result.
> `FdOperation::is_diacritic` filters out flag-diacritic symbols from expansion.
> Step 5: return `result`. (The result's input symbol table is set by the caller.)

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-input-language-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-input-language-fn]
> Static. Returns the input projection of `t` as a new heap `StdVectorFst *`.
> Step 1: `proj = new StdVectorFst(ProjectFst<StdArc>(*t, ProjectType::INPUT))`
> (copies each arc's input label onto both sides).
> Step 2: `retval = substitute(proj, 1, 2)` replaces unknown (1) labels with
> identity (2) so the acceptor treats unknowns as identities. Delete `proj`.
> Step 3: set `retval`'s input symbols to `t->InputSymbols()` and return `retval`.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-output-language-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-output-language-fn]
> Static. Identical to extract_input_language but projects on the OUTPUT side:
> `proj = new StdVectorFst(ProjectFst<StdArc>(*t, ProjectType::OUTPUT))`, then
> `retval = substitute(proj, 1, 2)` (unknown->identity), delete `proj`, set
> `retval`'s input symbols to `t->InputSymbols()`, return `retval`.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-paths-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-paths-fn]
> Static member. Drives path extraction from `t`, delivering each path to
> `callback` (hfst::ExtractStringsCb). Params: `t`, `callback`, int `cycles`,
> `FdTable<int64> *fd`, bool `filter_fd`.
> Step 1: if `t->Start() == -1` (no start state) return immediately.
> Step 2: create empty `all_visitations` and `path_visitations` maps. If `fd` is
> non-NULL, allocate a `fd_state_stack` = a vector containing one `FdState<int64>`
> constructed from `*fd`; otherwise NULL.
> Step 3: with an empty `StringPairVector spv`, call the file-static
> `hfst::implementations::extract_paths(t, t->Start(), all_visitations,
> path_visitations, 0.0f, callback, cycles, fd_state_stack, filter_fd, spv)` to do
> the recursive DFS enumeration.
> Step 4: emit the epsilon path if accepted: if start state exists and
> `t->Final(t->Start()) != Zero`, build an `HfstTwoLevelPath` with weight
> `Final(Start()).Value()` and an empty symbol vector, and call
> `callback(epsilon_path, true)`.
> Step 5: if `fd_state_stack` was allocated, delete it. No return value.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-random-paths-fd-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-random-paths-fd-fn]
> Static. Extracts up to `max_num` random paths from `t`, keeping only those whose
> flag-diacritic sequence is valid, and stores them in `results`
> (HfstTwoLevelPaths). Params: `t`, `results` (out), int `max_num`, bool `filter_fd`.
> Step 1: build a `FlagDiacriticTable fdt`, inserting every symbol of `t`'s
> alphabet (`get_alphabet(t)`) via `fdt.insert_symbol`.
> Step 2: into a local `fd_results`, call `extract_random_paths(t, fd_results,
> 5*max_num)` (over-samples by 5x since some will be filtered out).
> Step 3: iterate `fd_results` while `max_num > 0`. For each path, convert to a
> string vector (`hfst::symbols::to_string_vector(path)`). If `fdt.is_valid_string`
> accepts it: when `filter_fd`, strip flag symbols via `hfst::symbols::remove_flags`;
> insert the (possibly stripped) path into `results` and decrement `max_num`.
> No return value.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-random-paths-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.extract-random-paths-fn]
> Static. Fills `results` (HfstTwoLevelPaths, a set) with up to `max_num` distinct
> random paths from `t`. Params: `t`, `results` (out), int `max_num`.
> Step 1: seed the RNG with `srand((unsigned int)(time(0)))`.
> Step 2: loop while `max_num > 0`. Decrement `max_num`, then try
> `path = random_path(t, 5)` (which attempts up to 5 random walks). If it throws a
> C-string: if the message is exactly "cannot extract random path", `continue`
> (treat this as one used trial and keep going); for any other message, `return`
> (extraction impossible).
> Step 3: if `path` is already in `results`, try up to `i = max_num` more times to
> obtain a different path via `random_path(t, 5)` (ignoring exceptions and
> decrementing `i` each attempt) until a new one is found or `i` reaches 0.
> Step 4: insert `path` (whether new or a duplicate) into `results`. Continue the
> outer loop. No return value.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-alphabet-fn]
> StringSet

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-alphabet-fn]
> Static. Returns a `StringSet` containing every symbol string in `t`'s input
> symbol table. Asserts `t->InputSymbols() != NULL`, then iterates the input
> symbol table from `begin()` to `end()`, inserting each entry's `Symbol()` string
> into the set. Returns the set (includes the special symbols epsilon, ?, @).

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-biggest-symbol-number-fn]
> unsigned int

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-biggest-symbol-number-fn]
> Static. Returns the largest symbol number (label) used in `t`'s input symbol
> table. Initializes `biggest_number = 0`, iterates the input symbol table, and
> for each entry whose `Label()` exceeds the running maximum updates it. Returns
> the maximum (0 if the table is empty or all labels are 0).

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-final-weight-fn]
> float

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-final-weight-fn]
> Static. Returns the final weight of state `s` in `t` as a float:
> `return t->Final(s).Value();`. For a non-final state this is the tropical Zero
> value (positive infinity).

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-first-input-symbols-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-first-input-symbols-fn]
> Static, recursive helper `get_first_input_symbols(t, s, visited_states,
> symbols)`. Collects into `symbols` (StringSet) the input symbols reachable on the
> first non-epsilon/non-flag transition from state `s`, descending through
> epsilon/flag/identity arcs. Marks `s` in `visited_states`. For each out-arc of
> `s`: resolve `sym = t->InputSymbols()->Find(arc.ilabel)` (asserting non-empty); if
> `sym` is not a flag diacritic AND `arc.ilabel != 0`, insert `sym` into `symbols`;
> THEN, unconditionally, if `arc.nextstate` has not been visited, recurse into it.
> (Note: unlike get_initial_input_symbols, the recursion is not gated on the symbol
> being epsilon/flag — it always recurses into unvisited targets.) The public
> overload `get_first_input_symbols(t)` returns an empty set if `t->NumStates()==0`,
> else seeds `visited_states` empty and recurses from `t->Start()`, returning the
> accumulated set.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-flag-diacritics-fn]
> FdTable<int64> *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-flag-diacritics-fn]
> Static. Returns a newly heap-allocated `FdTable<int64> *` listing the flag
> diacritics in `t`'s input symbol table. Allocates an empty `FdTable<int64>`,
> takes `symbols = t->InputSymbols()`, iterates it, and for each entry whose
> `Symbol()` is a flag diacritic (`FdOperation::is_diacritic`), registers it with
> `table->define_diacritic(it->Label(), it->Symbol())`. Returns `table` (caller
> owns it).

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-initial-input-symbols-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-initial-input-symbols-fn]
> Static, recursive helper `get_initial_input_symbols(t, s, visited_states,
> symbols)`. Collects into `symbols` (StringSet) the input symbols that can begin a
> path from state `s`, descending through epsilon/flag arcs. Marks `s` in
> `visited_states`. For each out-arc of `s`: resolve `sym = t->InputSymbols()
> ->Find(arc.ilabel)` (asserting non-empty). If `sym` is NOT a flag diacritic AND
> `arc.ilabel != 0` (real symbol), insert `sym` into `symbols` and DO NOT recurse
> past it. Otherwise (epsilon or flag): if `arc.nextstate` is unvisited, recurse
> into it to look further. The public overload `get_initial_input_symbols(t)`
> returns an empty set if the start state id `s` satisfies `s + 1 == 0` (i.e.
> kNoStateId, empty transducer); otherwise seeds an empty `visited_states` and
> recurses from `t->Start()`, returning the accumulated set.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-initial-state-fn]
> StateId

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-initial-state-fn]
> Static. Returns the start state id of `t`: `return t->Start();` (StateId; -1 /
> kNoStateId if `t` has no start state).

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-profile-seconds-fn]
> float

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-profile-seconds-fn]
> Static, no args. Returns the module-level float `tropical_seconds` (initialized
> to 0 and never updated by live code — the profiling accumulation is commented
> out). Effectively always returns 0.0.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-smallest-weight-fn]
> float

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-smallest-weight-fn]
> Static. Returns the smallest weight value found anywhere in `t` (across all arc
> weights and all final-state weights). Initializes `retval` to
> `std::numeric_limits<float>::infinity()`. Iterates every state `s`: for each
> out-arc, take `w = arc.weight.Value()` and update `retval = min(retval, w)`; then,
> if `t->Final(s) != TropicalWeight::Zero()` (state is final), take `w =
> t->Final(s).Value()` and update the minimum. Returns `retval` — which stays
> +infinity for an empty transducer (no arcs and no final states).

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-symbol-number-fn]
> unsigned int

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-symbol-number-fn]
> Static. Returns the symbol number for `symbol` in `t`'s input symbol table.
> Asserts `t->InputSymbols() != NULL`, computes `i = t->InputSymbols()->Find(symbol)`
> (an int64). If `i < 0` (symbol not in the table), throws `SymbolNotFoundException`
> via HFST_THROW. Otherwise returns `(unsigned int)i`.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-symbol-vector-fn]
> StringVector

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-symbol-vector-fn]
> Static. Returns a `StringVector` indexed by symbol number: `symbol_vector[n]`
> is the symbol string with number `n` (or "" for unused numbers).
> Step 1: `biggest = get_biggest_symbol_number(t)`; allocate a StringVector of
> size `biggest + 1`, all entries initialised to "".
> Step 2: get `alphabet = get_alphabet(t)` (all symbol strings). For each symbol
> `s` in the alphabet, compute `n = get_symbol_number(t, s)` and assign
> `symbol_vector.at(n) = s`.
> Step 3: return the vector. Numbers present in the table but absent from the
> alphabet iteration remain "".

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-warning-stream-fn]
> std::ostream *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.get-warning-stream-fn]
> Static getter, no args. Returns the static member pointer
> `TropicalWeightTransducer::warning_stream` (a `std::ostream *`). No side effects.
> The companion `set_warning_stream(std::ostream *os)` assigns that member.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.harmonize-fn]
> std::pair<StdVectorFst *, StdVectorFst *>

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.harmonize-fn]
> Static. Harmonizes the symbol tables (and optionally the unknown-symbol arcs) of
> `t1` and `t2`. Params: `t1`, `t2`, `bool unknown_symbols_in_use`. Returns a
> `std::pair<StdVectorFst*, StdVectorFst*>` of the harmonized transducers. (A local
> `DEBUG=false` gates some stderr printing.)
> Step 1: compute `t1_symbols = get_alphabet(t1)` and `t2_symbols =
> get_alphabet(t2)`, then call `hfst::symbols::collect_unknown_sets(t1_symbols,
> unknown_t1, t2_symbols, unknown_t2)` to fill `unknown_t1` (symbols in t2 but not
> t1) and `unknown_t2` (symbols in t1 but not t2).
> Step 2: copy t2's input symbol table into `st2`. For each symbol in `unknown_t2`,
> `st2->AddSymbol(...)`; if the returned number is `< 3` (collided with a special
> symbol) print an error to cerr and assert(false). Install `st2` as t2's input
> symbols.
> Step 3: `km = create_mapping(t1, t2)` (t1's numbers -> t2's numbers). Set t1's
> input symbols to `st2` (now both share the same table), delete the local `st2`,
> and `recode_symbol_numbers(t1, km)` so t1's arcs use the shared numbering.
> Step 4: if `!unknown_symbols_in_use`, `harmonized_t1 = t1` and `harmonized_t2 =
> t2` unchanged. Otherwise set `harmonized_t1 = expand_arcs(t1, unknown_t1,
> unknown_symbols_in_use)` and restore its input symbols to `t1->InputSymbols()`;
> likewise `harmonized_t2 = expand_arcs(t2, unknown_t2, ...)` with t2's input
> symbols. Return the pair `(harmonized_t1, harmonized_t2)`.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.has-arc-fn]
> int

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.has-arc-fn]
> Static. Searches `t` (passed by reference) for an out-arc from `sourcestate`
> whose ilabel == `ilabel` and olabel == `olabel`. Iterates the arcs of
> `sourcestate` in order; on the first matching arc it returns that arc's position
> index (`aiter.Position()`, 0-based). If none matches, returns -1. Used to find an
> existing transition to merge into.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.has-weights-fn]
> bool

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.has-weights-fn]
> Static. Returns true iff `t` (const) contains any non-zero weight. Iterates every
> state `s`: for each out-arc, if `arc.weight.Value() != 0` return true immediately;
> then if `t->Final(s) != TropicalWeight::Zero()` (state is final) and
> `t->Final(s).Value() != 0`, return true. If no non-zero weight is ever found,
> returns false.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.initialize-symbol-tables-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.initialize-symbol-tables-fn]
> Static member. Builds the standard HFST special-symbol table via
> `create_symbol_table("")` (registering internal_epsilon=0, internal_unknown=1,
> internal_identity=2) into a local `SymbolTable st`, then installs it as `t`'s
> input symbol table with `t->SetInputSymbols(&st)` (which copies the table; the
> local `st` is then destroyed). No return value.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.insert-freely-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.insert-freely-fn]
> Static. Adds a self-loop arc for `symbol_pair` on EVERY state of `t`, in place,
> and returns `t`. Params: `t`, `const StringPair &symbol_pair`.
> Step 1: copy `t`'s input symbol table into `st` (assert non-NULL).
> Step 2: iterate all states (StateIterator over StdFst). For each `state_id`, add
> an arc from `state_id` back to itself (`StdArc(st->AddSymbol(symbol_pair.first),
> st->AddSymbol(symbol_pair.second), 0, state_id)`) — weight 0, self-loop. AddSymbol
> registers the pair's symbols in `st` if not already present.
> Step 3: install `st` as `t`'s input symbols, delete the local `st`, return `t`.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.insert-to-alphabet-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.insert-to-alphabet-fn]
> Static. Adds `symbol` to `t`'s input symbol table (without adding any arc).
> Asserts `t->InputSymbols() != NULL`, copies the table into `st`, calls
> `st->AddSymbol(symbol)` (assigns it the next free number if not present),
> reinstalls `st` as `t`'s input symbols, then deletes the local copy `st`. No
> return value.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.intersect-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.intersect-fn]
> Static. Intersects `t1` and `t2` (mutating both in place as side effects),
> returning a new heap `StdVectorFst *`.
> Step 1: `CHECK_EPSILON_CYCLES` on both, then `RmEpsilon` both.
> Step 2: arc-sort `t1` by output label (`OLabelCompare`) and `t2` by input label
> (`ILabelCompare`).
> Step 3: build an `EncodeMapper<StdArc>(0x0001, ENCODE)` — labels-only encoding,
> weights deliberately NOT encoded (the comment notes encoding weights would make
> e.g. `[a:b::1] & [a:b::2]` wrongly empty). `Encode` both `t1` and `t2`, then
> arc-sort both again (OLabel for t1, ILabel for t2).
> Step 4: `IntersectFst<StdArc> intersect(*t1, *t2)`; materialize it into a heap
> `foo`, build `DecodeFst<StdArc> decode(*foo, encoder)` to undo the encoding,
> delete `foo`, and materialize `result = new StdVectorFst(decode)`.
> Step 5: reset `t1` and `t2` output symbols to NULL, return `result`.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.invert-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.invert-fn]
> Static. Returns a new heap transducer that is `t` with input and output sides
> swapped. Deep-copies `t` via `copy(t)`, applies OpenFst `Invert` in place on the
> copy (swaps ilabel/olabel of every arc), restores the copy's input symbols to
> `t->InputSymbols()`, and returns the copy. `t` itself is unmodified.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.is-automaton-fn]
> bool

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.is-automaton-fn]
> Static. Returns true iff `t` is an acceptor (automaton). Iterates every state and
> every out-arc: if any arc has `ilabel != olabel` return false; also if any arc has
> `ilabel == 1` (the unknown symbol "?", i.e. a "?:?" arc) return false. If no such
> arc is found, returns true.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.is-cyclic-fn]
> bool

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.is-cyclic-fn]
> Static. Returns true iff `t` contains a cycle. Computes
> `t->Properties(kCyclic, true)` (the `true` forces OpenFst to actually test the
> property) and returns the result masked with `kCyclic` (non-zero iff cyclic).

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.is-final-fn]
> float

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.is-final-fn]
> Static. Despite the `float` return type, returns the boolean predicate
> `t->Final(s) != TropicalWeight::Zero()` (implicitly converted to float, i.e. 1.0
> if state `s` is final, 0.0 otherwise).

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.minimize-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.minimize-fn]
> Static. Minimizes `t`, returning a new heap `StdVectorFst *`.
> Step 1: `CHECK_EPSILON_CYCLES(t, "minimize")`. Then remove epsilons: in the
> default build just `RmEpsilon<StdArc>(t)`; under
> `USE_FOMA_EPSILON_REMOVAL && HAVE_FOMA`, if `!has_weights(t)` it instead converts
> t to a foma fsm, removes epsilons via FomaTransducer, converts back, and deletes
> the old `t` (replacing the pointer) — otherwise still `RmEpsilon<StdArc>(t)`.
> Step 2: `w = get_smallest_weight(t)`; if `w < 0`, `add_to_weights(t, -w)` to make
> all weights non-negative.
> Step 3: build `EncodeMapper<StdArc>` in ENCODE mode with flags
> `kEncodeLabels|kEncodeWeights` if `hfst::get_encode_weights()` else `kEncodeLabels`;
> `Encode(t, &encode_mapper)`.
> Step 4: allocate `det`; `Determinize<StdArc>(*t, det)`, then `Minimize<StdArc>(det)`,
> then `Decode(det, encode_mapper)`.
> Step 5: if `w < 0`, `add_to_weights(det, w)` to undo the shift. Return `det`.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.n-best-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.n-best-fn]
> Static. Returns a new heap transducer containing the `n` shortest (lowest-weight)
> paths of `t`. Params: `t`, `unsigned int n`.
> Step 1: `CHECK_EPSILON_CYCLES(t, "n_best")`. Allocate `n_best_fst`. Make a working
> copy `scaled = t->Copy()` and `RmEpsilon(scaled)`.
> Step 2: `w = get_smallest_weight(scaled)`; if `w < 0`, `add_to_weights(scaled, -w)`
> to make weights non-negative (ShortestPath requires this).
> Step 3: in a try block, `fst::ShortestPath(*scaled, n_best_fst, (size_t)n)`. If a
> `std::bad_alloc` is caught, delete both `n_best_fst` and `scaled` and throw
> `HfstFatalException` with message "TropicalWeightTransducer::nbest runs out of
> memory".
> Step 4: `RmEpsilon(n_best_fst)`. If `w < 0`, `add_to_weights(n_best_fst, w)` to
> undo the shift. Delete `scaled`, return `n_best_fst`.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.number-of-arcs-fn]
> unsigned int

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.number-of-arcs-fn]
> Static. Returns the total number of arcs in `t` (const). Initializes `retval = 0`,
> iterates every state and, for each, every out-arc, incrementing `retval` per arc.
> Returns `retval`.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.number-of-states-fn]
> unsigned int

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.number-of-states-fn]
> Static. Returns the number of states in `t` (const). Initializes `retval = 0`,
> iterates every state with a StateIterator incrementing `retval` once per state,
> and returns `retval`.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.optionalize-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.optionalize-fn]
> Static. Returns a new heap transducer recognizing `t` OR the empty string.
> Creates an epsilon transducer `eps = create_epsilon_transducer()`, sets its input
> and output symbol tables to `t->InputSymbols()` / `t->OutputSymbols()`, then
> applies OpenFst `Union(eps, *t)` (appending t's language into eps in place), and
> returns `eps`. `t` is unmodified.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.print-alphabet-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.print-alphabet-fn]
> Static debug printer. Iterates `t`'s (const) input symbol table from begin to
> end, printing each entry's symbol string to stderr as `'<symbol>', ` (quoted,
> comma-space separated). After the loop prints a trailing newline. No return value.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.prune-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.prune-fn]
> Static. Returns a new heap transducer that is `t` pruned: allocates `retval` and
> calls `fst::Prune(*t, retval, TropicalWeight::One())` (threshold = One, i.e.
> tropical 0), removing states/arcs not on a path within that weight threshold of
> the shortest distance. Returns `retval`. `t` is unmodified.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.push-labels-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.push-labels-fn]
> Static. Returns a new heap transducer that is `t` with labels pushed.
> Asserts `t->InputSymbols() != NULL`, then `CHECK_EPSILON_CYCLES(t, "push_labels")`.
> Allocates `retval`. If `to_initial_state` is true, calls
> `Push<StdArc, REWEIGHT_TO_INITIAL>(*t, retval, kPushLabels)`; otherwise
> `Push<StdArc, REWEIGHT_TO_FINAL>(*t, retval, kPushLabels)`. Sets `retval`'s input
> symbols to `t->InputSymbols()` and returns it.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.push-weights-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.push-weights-fn]
> Static. Identical to push_labels but pushes WEIGHTS. Asserts
> `t->InputSymbols() != NULL`, `CHECK_EPSILON_CYCLES(t, "push_weights")`, allocates
> `retval`, then `Push<StdArc, REWEIGHT_TO_INITIAL>(*t, retval, kPushWeights)` if
> `to_initial_state` else `Push<StdArc, REWEIGHT_TO_FINAL>(*t, retval, kPushWeights)`.
> Sets `retval`'s input symbols to `t->InputSymbols()` and returns it.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.read-in-att-format-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.read-in-att-format-fn]
> Static. Reads one transducer in AT&T text format from `FILE *ifile` and returns a
> new heap `StdVectorFst *`.
> Step 1: allocate `t`, build `st = create_symbol_table("")`. Create a `StateMap
> state_map`. Add and map external state 0 (`add_and_map_state(t, 0, state_map)`)
> and set it as the start state.
> Step 2: loop reading lines with `fgets(line, 255, ifile)`. If the first char is
> '-' (transducer separator), return `t` immediately. Otherwise parse the line with
> `sscanf(line, "%s\t%s\t%s\t%s\t%s", a1..a5)` into up to 5 whitespace-separated
> fields; let `n` be the count parsed. Compute `weight`: 0 by default, `atof(a2)` if
> `n==2`, `atof(a5)` if `n==5`.
> Step 3: if `n==1` or `n==2` it is a FINAL-state line: `final_number = atoi(a1)`,
> map it to a state, `t->SetFinal(state, weight)`.
> If `n==4` or `n==5` it is a TRANSITION line: `origin = atoi(a1)`, `target =
> atoi(a2)`, map both to states; `input_number = st.AddSymbol(a3)`, `output_number =
> st.AddSymbol(a4)`; add arc `StdArc(input_number, output_number, weight, target)`
> from origin.
> Otherwise (any other `n`, e.g. 0 or 3) throw `NotValidAttFormatException` with the
> line text as message.
> Step 4: after EOF, set `t`'s input symbols to `st` and return `t`. (`add_and_map_state`
> creates+maps a fresh state on first sight of each external number.)

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.recode-symbol-numbers-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.recode-symbol-numbers-fn]
> Static. Rewrites every arc label number in `t` according to the NumberNumberMap
> `km`. Iterates all states; for each, iterates arcs with a MutableArcIterator. For
> each arc, builds a new StdArc with `ilabel = km[arc.ilabel]`, `olabel =
> km[arc.olabel]` (asserting both remapped values are >= 0), and the same
> `weight`/`nextstate`, then writes it back via `aiter.SetValue(new_arc)`. Mutates
> `t` in place; no return value. (Note: `km[...]` is operator[] on a std::map, so a
> label number absent from `km` would map to 0.)

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.remove-epsilons-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.remove-epsilons-fn]
> Static. Returns a new heap transducer that is `t` with epsilon transitions
> removed. First `CHECK_EPSILON_CYCLES(t, "remove_epsilons")` (raises if epsilon
> cycles present), then returns `new StdVectorFst(RmEpsilonFst<StdArc>(*t))` (lazy
> epsilon-removal delayed FST materialized into a concrete StdVectorFst). `t` is
> unmodified.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.remove-from-alphabet-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.remove-from-alphabet-fn]
> Static. Removes `symbol` from `t`'s input symbol table (arcs are not touched).
> Asserts `t->InputSymbols() != NULL`. Builds a fresh local `SymbolTable st` named
> with the original table's `Name()`. Iterates every entry of the current input
> table and re-adds it (`st.AddSymbol(it->Symbol(), it->Label())`, preserving its
> number) EXCEPT the entry whose `Symbol()` equals `symbol`, which is skipped.
> Finally installs `st` as `t`'s input symbols (`SetInputSymbols(&st)` copies it;
> the local `st` is then destroyed). No return value.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.remove-symbol-table-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.remove-symbol-table-fn]
> Static. Detaches the input symbol table from `t` by calling
> `t->SetInputSymbols(NULL)`. No return value.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.repeat-le-n-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.repeat-le-n-fn]
> Static. Returns a new heap transducer recognizing 0 to `n` concatenated copies of
> `t` (i.e. `t` repeated at most `n` times, including the empty string).
> If `n == 0`, returns `create_epsilon_transducer()`.
> Otherwise: build `repetition = create_epsilon_transducer()` and set its input
> symbols to `t->InputSymbols()`. Loop `n` times: each iteration build `optional_t =
> optionalize(t)` (= `t` or epsilon), set its input symbols to `t->InputSymbols()`,
> `Concat(repetition, *optional_t)`, then delete `optional_t`. Return `repetition`.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.repeat-n-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.repeat-n-fn]
> Static. Returns a new heap transducer recognizing exactly `n` concatenated copies
> of `t`. If `n == 0`, returns `create_epsilon_transducer()` (the empty string).
> Otherwise: build `repetition = create_epsilon_transducer()`, set its input symbols
> to `t->InputSymbols()`, then loop `n` times calling `Concat(repetition, *t)` to
> append `t` each time. Return `repetition`.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.repeat-plus-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.repeat-plus-fn]
> Static. Returns the Kleene-plus closure of `t` (one or more repetitions) as a
> new heap `StdVectorFst *`: `return new StdVectorFst(ClosureFst<StdArc>(*t,
> CLOSURE_PLUS));`. Does not modify `t`.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.repeat-star-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.repeat-star-fn]
> Static. Returns the Kleene-star closure of `t` (zero or more repetitions,
> including the empty string) as a new heap `StdVectorFst *`: `return new
> StdVectorFst(ClosureFst<StdArc>(*t, CLOSURE_STAR));`. Does not modify `t`.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.represent-empty-transducer-as-having-one-state-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.represent-empty-transducer-as-having-one-state-fn]
> Static. Intends to normalise a degenerate empty transducer into the canonical
> one-state representation. If `t->Start() == fst::kNoStateId` OR
> `t->NumStates() == 0`, it does `delete t;` then `t = create_empty_transducer();`.
> No return value. NOTE: the reassignment is to the local pointer parameter `t`
> only (passed by value), so the caller's pointer is NOT updated — this is a bug;
> the function deletes the transducer and leaks the replacement, and the caller is
> left with a dangling pointer. A faithful port should reproduce this signature's
> intent but realistically would return/replace by reference; the literal C++
> behaviour mutates nothing observable by the caller except freeing `t`.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.reverse-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.reverse-fn]
> Static. Returns the reversal of `t` as a new heap `StdVectorFst *`. Allocates
> `reversed = new StdVectorFst`, calls OpenFst `Reverse<StdArc,StdArc>(*t,
> reversed)` (which reverses the relation, adding a new super-initial state per
> OpenFst semantics), sets `reversed`'s input symbols to `t->InputSymbols()`, and
> returns it. Does not modify `t`. (Comment notes it makes valgrind unhappy.)

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-final-weight-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-final-weight-fn]
> Static. Sets the final weight of state `s` in `t` to float `w`:
> `t->SetFinal(s, w);`. No return value. (Setting a non-Zero weight makes the
> state final.)

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-final-weights-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-final-weights-fn]
> Static. Mutates `t` in place, setting the final weight of every FINAL state to
> `weight` (or adding to it). Params: `t`, float `weight`, bool `increment`.
> Iterates all states (StateIterator). For each state `s` with `t->Final(s) !=
> TropicalWeight::Zero()` (i.e. already final): if `increment` is true, read
> `old_weight = t->Final(s).Value()` and `t->SetFinal(s, weight + old_weight)`;
> otherwise `t->SetFinal(s, weight)`. Non-final states are left untouched.
> Returns `t` (the same pointer).

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-symbol-table-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-symbol-table-fn]
> Static. Installs a new input symbol table on `t` from an explicit list of
> (number, string) mappings. Params: `t`, `std::vector<std::pair<unsigned short,
> std::string>> symbol_mappings`.
> Step 1: build a base table `st = create_symbol_table("")` (pre-populated with
> internal_epsilon=0, internal_unknown=1, internal_identity=2).
> Step 2: for each mapping `i`, call `st.AddSymbol(symbol_mappings[i].second,
> symbol_mappings[i].first)` (string at the given number).
> Step 3: `t->SetInputSymbols(&st)` (copies the table into `t`). No return value.
> Note `st` is a stack local, so SetInputSymbols must copy it.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-warning-stream-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-warning-stream-fn]
> Static setter. Assigns its `std::ostream *os` argument to the static class
> member `warning_stream` (`warning_stream = os;`). No return value. Callers use
> this to redirect where the class emits warnings.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-weight-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.set-weight-fn]
> Static. Returns a copy of `t` in which every final state's final weight is set
> to `f` (does not modify `t`). Params: `t`, float `f`.
> Step 1: `t_copy = new StdVectorFst(*t)` (deep copy).
> Step 2: iterate the states of `t` (StateIterator over `*t`); for each state
> whose value `iter.Value()` is final in `t_copy` (`t_copy->Final(...) !=
> TropicalWeight::Zero()`), call `t_copy->SetFinal(iter.Value(), f)`.
> Step 3: return `t_copy`. Arc weights are unchanged; only final weights are reset.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.state-map]
> typedef std::map<int, StateId> StateMap

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.substitute-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.substitute-fn]
> The annotation sits on the overload `substitute(StdVectorFst *t, const
> StringPair old_symbol_pair, StdVectorFst *transducer)`, which replaces every arc
> labelled by `old_symbol_pair` in `t` with an embedded copy of `transducer`,
> spliced in via epsilon transitions. Mutates and returns `t`.
> Step 1: assert `t->InputSymbols() != NULL`; copy it into `st`. Record `states =
> t->NumStates()`.
> Step 2: for each state index `i` in `[0, states)`, iterate its arcs with a
> MutableArcIterator. For an arc whose `ilabel == st->AddSymbol(old_symbol_pair
> .first)` and `olabel == st->AddSymbol(old_symbol_pair.second)`:
>   - remember `destination_state = arc.nextstate`; allocate `start_state =
>     t->AddState()`; rewrite the arc in place to ilabel=0, olabel=0 (epsilon),
>     nextstate=start_state, keeping its weight (`it.SetValue`).
>   - add `transducer->NumStates() - 1` further new states to `t` (the loop adds
>     states for indices 1..N-1; combined with start_state this reserves a block
>     of `transducer`'s states starting at `start_state`).
>   - for each state `tr_state_id` of `transducer`: if it is final, add an epsilon
>     arc `(0, 0, transducer->Final(tr_state_id), destination_state)` from state
>     `tr_state_id + start_state` (the final weight is carried onto this epsilon
>     arc back to the original destination). Then for each arc of `tr_state_id`,
>     add `(tr_arc.ilabel, tr_arc.olabel, tr_arc.weight, tr_arc.nextstate +
>     start_state)` from `tr_state_id + start_state` (copying the sub-transducer
>     with all state ids offset by `start_state`).
> Step 3: `t->SetInputSymbols(st)`, delete `st`, return `t`.
> Note the iteration counts `t->NumStates()` once before the loop, so the newly
> added embedded states are not themselves rescanned for substitution.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.subtract-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.subtract-fn]
> Static. Returns a new heap `StdVectorFst *` = the relational difference `t1 -
> t2` (paths of `t1` not in `t2`). A local `DEBUG=false` gates some `printf`s.
> Step 1: if either operand has a NULL output symbol table, set its output symbols
> to its own input symbols.
> Step 2: `CHECK_EPSILON_CYCLES` on both, then `RmEpsilon` both in place. Arc-sort
> `t1` by output label, `t2` by input label.
> Step 3: make a copy `t2_ = copy(t2)` and zero out all of its weights: iterate
> every state, rewrite each arc with `weight = 0` (keeping labels/nextstate via
> MutableArcIterator SetValue), and for every final state set its final weight to
> 0. (Difference requires an unweighted second operand.)
> Step 4: build `EncodeMapper<StdArc> encoder(kEncodeLabels, ENCODE)`; `Encode`
> both `t1` and `t2_` with it. Re-sort `t1` by output label and `t2_` by input.
> Step 5: determinize `t2_` into a fresh `det2` (`Determinize<StdArc>`), delete
> `t2_`.
> Step 6: allocate `difference`, call `Difference(*t1, *det2, difference)`, delete
> `det2`. Build `DecodeFst<StdArc> subtract(*difference, encoder)` (decodes labels
> back), delete `difference`.
> Step 7: reset `t1`'s and `t2`'s output symbols to NULL. Return `new
> StdVectorFst(subtract)`. Note `t1` and `t2` are mutated (epsilons removed,
> sorted, encoded) as a side effect.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.transform-weights-fn]
> StdVectorFst *

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.transform-weights-fn]
> Static. Applies the function pointer `float (*func)(float)` to every weight in
> `t` in place, and returns `t`. Iterates all states (StateIterator). For each
> state `s`: if `t->Final(s) != TropicalWeight::Zero()` (final), set its final
> weight to `func(t->Final(s).Value())`. Then iterate the state's arcs with a
> MutableArcIterator; for each arc build a new StdArc copying ilabel, olabel,
> nextstate and setting `weight = func(arc.weight.Value())`, and write it back
> (`aiter.SetValue`). Returns the same pointer `t`.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.write-in-att-format-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.write-in-att-format-fn]
> The annotation sits on the `std::ostream &os` overload (a `FILE *ofile`
> overload also exists with identical logic). Writes `t` in AT&T text format with
> symbol NAMES, ensuring the initial state always prints as 0.
> Step 1: `sym = t->InputSymbols()` (assert non-NULL). Compute a state-id swap:
> `initial_state = t->Start()`; `zero_print = 0`, but if `initial_state != 0` set
> `zero_print = initial_state`. The printed id of a state `s` is: 0 if
> `s == initial_state`; else `zero_print` if `s == 0`; else `s` (so the initial
> state and state 0 swap printed numbers). The same mapping is applied to each
> arc's `nextstate` to compute the printed target.
> Step 2: first pass — find the initial state in a StateIterator and emit ONLY
> its block (so it appears first), then `break`. For each of its arcs print
> `origin\ttarget\t<isym>\t<osym>\t<weight>\n` where isym/osym are
> `sym->Find(ilabel/olabel)`. If the state is final, print `origin\t<final
> weight>\n`.
> Step 3: second pass — iterate all states again and emit the same per-arc and
> per-final lines for every state with `s != initial_state` (the non-initial
> states, in iteration order).
> Pure I/O; no return value; `t` is not modified.

> [spec:hfst:def:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.write-in-att-format-number-fn]
> void

> [spec:hfst:sem:tropical-weight-transducer.hfst.implementations.tropical-weight-transducer.write-in-att-format-number-fn]
> Static. Like write_in_att_format but prints arc labels as NUMBERS (not symbol
> names). Writes `t` to `std::ostream &os`, with the same initial-state/state-0
> swapping so the initial state prints as 0 (compute `initial_state = t->Start()`,
> `zero_print = (initial_state != 0) ? initial_state : 0`; a state's printed id is
> 0 if it is the initial state, else `zero_print` if it is state 0, else its own
> id; the same mapping is applied to each arc's nextstate).
> First pass: emit only the initial state's block then `break`. Second pass: emit
> all states with `s != initial_state`. For each arc print
> `origin\ttarget\t\\<ilabel>\t\\<olabel>\t<weight>\n` (the labels are the raw
> integer arc labels, each prefixed with a backslash). For each final state print
> `origin\t<final weight>\n`. No symbol table is consulted. Pure I/O; no return;
> `t` unmodified.

> [spec:hfst:def:tropical-weight-transducer.int64]
> typedef __int64 int64

> [spec:hfst:def:tropical-weight-transducer.main-fn]
> int

> [spec:hfst:sem:tropical-weight-transducer.main-fn]
> Unit-test `main` compiled only under `#ifdef MAIN_TEST`. Prints "Unit tests for
> <__FILE__>:" to std::cout. Constructs a `TropicalWeightTransducer ofst`, calls
> `ofst.create_empty_transducer()` and `delete`s it, then
> `ofst.create_epsilon_transducer()` and `delete`s it. Prints a newline then "ok"
> and a newline. Returns `EXIT_SUCCESS`. Effectively a smoke test that the two
> factory methods run and the result is destructible.

