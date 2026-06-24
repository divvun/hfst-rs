# libhfst/src/implementations/SfstTransducer.cc, libhfst/src/implementations/SfstTransducer.h

> [spec:hfst:def:sfst-transducer.does-sfst-alphabet-contain-fn]
> bool does_sfst_alphabet_contain(SFST::Transducer *t, const char *str)

> [spec:hfst:sem:sfst-transducer.does-sfst-alphabet-contain-fn]
> Free function. Obtains the alphabet's CharMap from `t->alphabet.get_char_map()`
> (a mapping from character code to symbol string), then iterates over every
> entry. For each entry it compares the entry's symbol string (`it->second`)
> against `str` with `strcmp`; if any entry's string equals `str` exactly it
> returns true immediately. If no entry matches after the full iteration, returns
> false. Pure read; no mutation, no I/O.

> [spec:hfst:def:sfst-transducer.hfst.implementations.extract-paths-fn]
> static bool extract_paths

> [spec:hfst:sem:sfst-transducer.hfst.implementations.extract-paths-fn]
> Free recursive DFS helper (file-static) that enumerates paths and reports each
> via `callback`. Parameters: transducer `t`, current `node`, `all_visitations`
> and `path_visitations` (HfstNode2Int maps node->int counters), `callback`, a
> cycle limit `cycles`, a flag-diacritic state stack `fd_state_stack` (may be
> NULL), a `filter_fd` bool, and `spv` (a StringPairVector accumulating the path
> so far). Steps:
> 1. If `cycles >= 0` and `path_visitations[node] > cycles`, return true (cycle
>    bound reached, prune).
> 2. Increment `all_visitations[node]` and `path_visitations[node]`.
> 3. If `spv` is non-empty: build an HfstTwoLevelPath(0, spv), determine
>    `final = node->is_final()`, call `callback(path, final)`. If the returned
>    RetVal has `!continueSearch` or `!continuePath`, decrement
>    `path_visitations[node]` and return `ret.continueSearch`.
> 4. Build a vector of the node's outgoing arcs sorted ascending by the target
>    node's `all_visitations` count (insertion sort: each arc inserted before the
>    first existing arc whose target's all_visitations exceeds it).
> 5. Iterate the sorted arcs while a running `res` is true. For each arc with
>    label `l`: if `fd_state_stack` is non-NULL and the back state's table has a
>    flag operation for `l.lower_char()`, push a copy of the back state and apply
>    the operation; if apply succeeds mark `added_fd_state`, else pop and
>    `continue` (skip this transition).
> 6. Compute `lc=l.lower_char()`, `uc=l.upper_char()`. If `!filter_fd` OR the
>    back state's table has no operation for `lc`, set istring to
>    `t->alphabet.write_char(lc)` (else ""); same for ostring/`uc`. If a resulting
>    string equals "<>", replace it with `internal_epsilon`. Push
>    StringPair(istring, ostring) onto `spv`.
> 7. Recurse into the arc's target node with the same maps/callback/etc., storing
>    result into `res`. Pop the StringPair from `spv`. If `added_fd_state`, pop
>    the fd state.
> 8. After the loop, decrement `path_visitations[node]` and return `res`.

> [spec:hfst:def:sfst-transducer.hfst.implementations.hfst-node2-int]
> class HfstNode2Int {
>   struct hashf { // [spec:hfst:def:sfst-transducer.hfst.implementations.hfst-node2-int.hashf.operator-fn] // [spec:hfst:sem:sfst-transducer.hfst.implementation...;
>   struct equalf { // [spec:hfst:def:sfst-transducer.hfst.implementations.hfst-node2-int.equalf.operator-fn] // [spec:hfst:sem:sfst-transducer.hfst.implementati...;
>   NL number;
> }

> [spec:hfst:def:sfst-transducer.hfst.implementations.hfst-node2-int.equalf]
> struct equalf

> [spec:hfst:def:sfst-transducer.hfst.implementations.hfst-node2-int.equalf.operator-fn]
> int operator()(const SFST::Node *n1, const SFST::Node *n2) const

> [spec:hfst:sem:sfst-transducer.hfst.implementations.hfst-node2-int.equalf.operator-fn]
> Equality functor for the node hash map. Returns the int result of the pointer
> comparison `(n1 == n2)`: 1 (true) when the two node pointers are identical, 0
> otherwise.

> [spec:hfst:def:sfst-transducer.hfst.implementations.hfst-node2-int.hashf]
> struct hashf

> [spec:hfst:def:sfst-transducer.hfst.implementations.hfst-node2-int.hashf.operator-fn]
> size_t operator()(const SFST::Node *node) const

> [spec:hfst:sem:sfst-transducer.hfst.implementations.hfst-node2-int.hashf.operator-fn]
> Hash functor for the node hash map. Returns the node pointer reinterpreted as a
> `size_t` (`(size_t)node`), i.e. the raw address value is the hash.

> [spec:hfst:def:sfst-transducer.hfst.implementations.hfst-node2-int.nl]
> typedef SFST::hash_map<SFST::Node*, int, hashf, equalf> NL

> [spec:hfst:def:sfst-transducer.hfst.implementations.is-minimal-and-empty-fn]
> static bool is_minimal_and_empty(Transducer *t)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.is-minimal-and-empty-fn]
> File-static helper. Iterates over the arcs of `t->root_node()`. If there is at
> least one arc, the loop body executes on the first arc and returns false. If
> the root node has no outgoing arcs, the loop never runs and it returns true.
> So: returns true iff the root node has no outgoing arcs (an empty/no-language
> minimal transducer).

> [spec:hfst:def:sfst-transducer.hfst.implementations.random-path-fn]
> static HfstTwoLevelPath random_path(Transducer *t)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.random-path-fn]
> File-static helper that walks one random path through `t`. Initializes an empty
> HfstTwoLevelPath `path`, sets `current_t_node = t->root_node()`, and
> `last_index = 0` (index up to which the path is a valid accepted prefix).
> Calls `t->nodeindexing(&indexing)` to assign indices to nodes and obtain the
> node count `number_of_nodes` (first of the returned pair). Allocates `visited`
> and `broken` int vectors; the init loop pushes two 0 entries per node into
> `visited` (so it has size 2*number_of_nodes), `broken` is left empty/reserved.
> Then loops:
> 1. Mark `visited[current_t_node->index] = 1`.
> 2. Collect all outgoing arcs of the current node into `t_transitions`.
> 3. If `t_transitions` is empty OR `broken[current_t_node->index]` is set, trim
>    `path.second` back to `last_index` (pop trailing pairs) and return `path`.
> 4. Inner loop: pick a uniformly random transition index via `rand() %
>    t_transitions.size()`, take that Arc, erase it from the vector. Get its
>    target node. Build istring/ostring from `t->alphabet.code2symbol` of the
>    label's lower/upper char; if either equals "<>" replace with
>    `internal_epsilon`. Push StringPair(istring, ostring) onto `path.second`.
> 5. If the target node is final: with probability 1/4 (`rand()%4==0`) return
>    `path` immediately; otherwise set `last_index = path.second.size()`.
> 6. To bias toward shorter paths on cycles: if `broken[target->index]==0` and
>    `visited[target->index]==1`, then with probability 1/4 set
>    `broken[target->index]=1`; separately, if `visited[target->index]==1`, with
>    probability 1/4 set `broken[target->index]=1`.
> 7. Set `current_t_node = target` and `break` out of the inner loop (only one
>    transition per outer iteration is followed).
> Returns the accumulated `path`. Uses C `rand()`. Note `broken` is indexed
> without prior sizing in the init loop (relies on reserve/index behavior of the
> source).

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-input-stream]
> class SfstInputStream {
>   std::string filename;
>   FILE * input_file;
>   bool is_minimal;
> }

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-input-stream.add-symbol-fn]
> void SfstInputStream::add_symbol(StringNumberMap &string_number_map,

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-input-stream.add-symbol-fn]
> Resolves character code `c` to its symbol string via `alphabet.code2symbol(c)`.
> If that string is not already a key in `string_number_map`, inserts the mapping
> `string_symbol -> c`. Otherwise, if the existing mapping for that string differs
> from `c`, throws `HfstFatalException` with message "SfstInputStream: symbol
> redefined". If the existing mapping already equals `c`, does nothing.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-input-stream.close-fn]
> void SfstInputStream::close(void)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-input-stream.close-fn]
> If `input_file` is NULL, returns immediately. Otherwise, if `filename` is
> non-empty (its first char is not the NUL byte — i.e. not stdin), calls
> `fclose(input_file)` and sets `input_file = NULL`. When reading from stdin
> (empty filename) the file is left open and not closed.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-input-stream.ignore-fn]
> void SfstInputStream::ignore(unsigned int n)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-input-stream.ignore-fn]
> Discards `n` bytes from the input stream by calling `fgetc(input_file)` exactly
> `n` times in a loop, ignoring the returned values. No return value.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-input-stream.is-bad-fn]
> bool SfstInputStream::is_bad(void)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-input-stream.is-bad-fn]
> Returns the result of `is_eof()`; the stream is considered "bad" exactly when
> it is at end of file.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-input-stream.is-eof-fn]
> bool SfstInputStream::is_eof(void)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-input-stream.is-eof-fn]
> Peeks at the stream without consuming: reads one char with `getc(input_file)`,
> records whether EOF is now set via `feof(input_file) != 0`, then pushes the
> char back with `ungetc(c, input_file)`. Returns the recorded boolean (true if
> end of file was reached).

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-input-stream.is-fst-fn]
> bool SfstInputStream::is_fst(FILE * f)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-input-stream.is-fst-fn]
> Static, takes a `FILE * f`. If `f` is NULL returns false. Otherwise peeks the
> first byte: `c = getc(f)`, immediately `ungetc(c, f)`, and returns whether
> `c == (int)'a'` — i.e. true iff the next byte to read is the ASCII letter 'a'
> (the SFST binary magic byte). Does not consume the byte.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-input-stream.is-good-fn]
> bool SfstInputStream::is_good(void)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-input-stream.is-good-fn]
> Returns the negation of `is_bad()`, i.e. true when the stream is not at end of
> file.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-input-stream.read-transducer-fn]
> Transducer * SfstInputStream::read_transducer()

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-input-stream.read-transducer-fn]
> Reads one SFST transducer from `input_file`. If `is_eof()` returns true, throws
> `StreamIsClosedException`. Initializes a local `Transducer * t = NULL`, then in
> a try block: asserts `stream_get() == 'a'` (debug check of the magic byte) and
> ungets 'a' via `stream_unget('a')`; constructs a new `Transducer(input_file,
> true)` (note: bound to a shadowing local `t`); if `is_minimal` is false, sets
> the new transducer's `minimised = false` and `deterministic = false`; returns
> the new transducer. If a `const char *` exception is thrown during reading,
> catches it, `delete t` (the outer NULL pointer), prints `caught message: "..."`
> to stderr, and throws `TransducerHasWrongTypeException`. Falls through to return
> NULL only if neither path is taken.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-input-stream.set-implementation-specific-header-data-fn]
> bool SfstInputStream::set_implementation_specific_header_data

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-input-stream.set-implementation-specific-header-data-fn]
> Validates and consumes the single SFST-specific header field "minimal".
> Parameters: `header_data` (vector of string pairs) and `index`. Returns false
> unless `index == header_data.size()-1` (must be the last entry). Returns false
> unless the entry's first (key) equals "minimal". Then inspects the second
> (value): if "true" sets `is_minimal = true`; if "false" sets
> `is_minimal = false`; any other value returns false. On success returns true.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-input-stream.sfst-input-stream-fn]
> SfstInputStream::SfstInputStream(const std::string &filename_)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-input-stream.sfst-input-stream-fn]
> Constructor taking a filename. Initializes `filename` to a copy of `filename_`
> and `is_minimal` to false. If `filename` is empty, sets `input_file = stdin`.
> Otherwise opens the file for reading via `hfst::hfst_fopen(filename, "r")`; if
> the result is NULL, throws `StreamNotReadableException`.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-input-stream.stream-get-fn]
> char SfstInputStream::stream_get()

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-input-stream.stream-get-fn]
> Reads and consumes one byte from `input_file` via `fgetc` and returns it cast
> to `char`.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-input-stream.stream-get-short-fn]
> short SfstInputStream::stream_get_short()

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-input-stream.stream-get-short-fn]
> Reads one `short` from `input_file` via `fread(&i, sizeof(short), 1,
> input_file)`, asserting the read returned exactly 1 element, and returns the
> raw short (native byte order, no conversion).

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-input-stream.stream-unget-fn]
> void SfstInputStream::stream_unget(char c)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-input-stream.stream-unget-fn]
> Pushes the byte `c` back onto `input_file` via `ungetc((int)c, input_file)`, so
> it will be the next byte read.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-output-stream]
> class SfstOutputStream {
>   std::string filename;
>   FILE *ofile;
> }

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-output-stream.append-implementation-specific-header-data-fn]
> void SfstOutputStream::append_implementation_specific_header_data

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-output-stream.append-implementation-specific-header-data-fn]
> Appends the SFST-specific header field into `header` (a vector<char>). Pushes
> the bytes of the literal string "minimal" one by one, then a NUL terminator
> `'\0'`. Determines a value string: "true" if `t->minimised && t->deterministic`
> are both set, otherwise "false". Pushes the bytes of that value string one by
> one, then another NUL terminator. No return value.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-output-stream.close-fn]
> void SfstOutputStream::close(void)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-output-stream.close-fn]
> If `filename` is non-empty (a real file, not stdout), calls `fclose(ofile)`.
> When writing to stdout (empty filename) does nothing.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-output-stream.sfst-output-stream-fn]
> SfstOutputStream::SfstOutputStream(const std::string &str)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-output-stream.sfst-output-stream-fn]
> Constructor taking a string. Initializes `filename` to a copy of `str`. If
> `filename` is non-empty, opens it for binary writing via
> `hfst::hfst_fopen(filename, "wb")`; if the result is NULL throws
> `StreamNotReadableException`. If `filename` is empty, sets `ofile = stdout`.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-output-stream.write-fn]
> void SfstOutputStream::write(const char &c)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-output-stream.write-fn]
> Writes the single byte `c` to `ofile` via `fputc(c, ofile)`. No return value.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-output-stream.write-transducer-fn]
> void SfstOutputStream::write_transducer(Transducer * transducer)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-output-stream.write-transducer-fn]
> Serializes `transducer` to `ofile` by calling `transducer->store(ofile)`, then
> flushes with `fflush(ofile)`. If `fflush` returns non-zero, throws
> `HfstFatalException` with message "An error happened when writing an
> SfstTransducer.".

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-set-hopcroft-fn]
> void sfst_set_hopcroft(bool value)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-set-hopcroft-fn]
> Free function. Sets the static class flag
> `SFST::Transducer::hopcroft_minimisation = value`, controlling whether SFST
> minimisation uses the Hopcroft algorithm.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer]
> class SfstTransducer

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.are-equivalent-fn]
> bool SfstTransducer::are_equivalent(Transducer * t1, Transducer * t2)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.are-equivalent-fn]
> Returns the result of `(*t1 == *t2)`, delegating to SFST's Transducer equality
> operator, which tests whether the two transducers recognize the same relation.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.compose-fn]
> Transducer * SfstTransducer::compose

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.compose-fn]
> Composes `t1` with `t2` by invoking SFST's composition operator
> `t1->operator||(*t2)` and returns a pointer to the resulting transducer (the
> address of the reference returned by the operator).

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.concatenate-fn]
> Transducer * SfstTransducer::concatenate

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.concatenate-fn]
> Concatenates `t1` with `t2` by invoking SFST's concatenation operator
> `t1->operator+(*t2)` and returns a pointer to the resulting transducer (the
> address of the reference returned by the operator).

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.copy-fn]
> Transducer * SfstTransducer::copy(Transducer * t)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.copy-fn]
> Returns a pointer to a deep copy of `t`, obtained by `&t->copy()` (taking the
> address of the reference returned by SFST's `Transducer::copy`).

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.create-empty-transducer-fn]
> Transducer * SfstTransducer::create_empty_transducer(void)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.create-empty-transducer-fn]
> Allocates a new empty `Transducer`, calls `initialize_alphabet(retval)` to set
> up its special-symbol alphabet, and returns the pointer. The result accepts the
> empty language (root node is not final and has no arcs).

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.create-epsilon-transducer-fn]
> Transducer * SfstTransducer::create_epsilon_transducer(void)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.create-epsilon-transducer-fn]
> Allocates a new empty `Transducer`, calls `initialize_alphabet(t)` to set up the
> special-symbol alphabet, marks the root node as final via
> `t->root_node()->set_final(1)`, and returns the pointer. The result accepts only
> the empty string (epsilon).

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.define-transducer-fn]
> Transducer * SfstTransducer::define_transducer

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.define-transducer-fn]
> The annotated overload takes a `const std::vector<StringPairSet> &spsv`,
> building a transducer where each set in `spsv` becomes one "column" of parallel
> arcs between consecutive nodes (a sausage/chain of alternative symbol pairs).
> Steps: allocate a new `Transducer * t`, call `initialize_alphabet(t)`, set
> `n = t->root_node()`. For each StringPairSet `*it` in `spsv` (in order): create
> a fresh node `temp = t->new_node()`; for each StringPair `it2` in that set,
> compute `inumber`/`onumber` from `it2->first`/`it2->second` — 0 if the string
> is epsilon (`is_epsilon`) or literally "<>", otherwise
> `t->alphabet.add_symbol(str.c_str())` — and add arc
> `Label(inumber,onumber)` from `n` to `temp`. After processing the set, advance
> `n = temp`. After all sets, mark the final node `n->set_final(1)` and return `t`.
> (There are sibling overloads for single number, number pair, single symbol,
> symbol pair, StringPairVector, and StringPairSet+cyclic flag, but this rule
> covers the vector-of-StringPairSet form.)

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.delete-transducer-fn]
> void SfstTransducer::delete_transducer(Transducer * t)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.delete-transducer-fn]
> Frees the transducer: `delete t`. Releases the heap-allocated SFST Transducer.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.determinize-fn]
> Transducer * SfstTransducer::determinize(Transducer * t)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.determinize-fn]
> Returns a pointer to the determinised transducer: `&t->determinise()` (address
> of the reference returned by SFST's `Transducer::determinise`).

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.disjunct-fn]
> Transducer * SfstTransducer::disjunct

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.disjunct-fn]
> The annotated overload takes `(Transducer * t, const StringPairVector &spv)`
> and disjuncts a single string-pair path into `t` IN PLACE (mutating and
> returning the same `t`). (A separate two-transducer overload `disjunct(t1,t2)`
> just returns `&t1->operator|(*t2)`; this rule covers the path form.) Steps: set
> `node = t->root_node()`. For each StringPair `it` in `spv`: compute
> `inumber`/`onumber` (0 if epsilon or "<>", else `t->alphabet.add_symbol`); build
> `Label l(inumber, onumber)` and `t->alphabet.insert(l)`. Get `arcs =
> node->arcs()` and look up `arcs->target_node(l)`: if an arc with that exact
> label already exists, follow it (set `node` to that target); otherwise create a
> new node, add arc `l` from the current node to it via `arcs->add_arc(l, node,
> t)`, and set `node` to the new node. After the loop mark `node->set_final(1)`
> and return `t`. Effectively adds the single path described by `spv` to the
> transducer's language, reusing existing prefix arcs (trie-style insertion).

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.expand-arcs-fn]
> Transducer * SfstTransducer::expand_arcs(Transducer * t, StringSet &unknown)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.expand-arcs-fn]
> Makes a deep copy of `t` via `t->copy()` (binding to a reference `tc`), then
> calls `SfstTransducer::expand(&tc, unknown)` to expand the copy's unknown/
> identity arcs against the `unknown` StringSet, and returns `&tc` (address of the
> copy). The input `t` is left unmodified; the returned transducer is the expanded
> copy.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.expand-fn]
> void SfstTransducer::expand(Transducer *t, hfst::StringSet &new_symbols)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.expand-fn]
> Expands all transitions of `t` IN PLACE against `new_symbols`. Creates an empty
> `std::set<Node*> visited_nodes` and calls `expand2(t, t->root_node(),
> new_symbols, visited_nodes)`, which recursively visits every reachable node once
> and applies `expand_node` to each arc. No return value.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.expand-node-fn]
> void SfstTransducer::expand_node

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.expand-node-fn]
> Adds extra arcs from `origin` to `target` in `t` that expand a special
> unknown/identity label `l` against the symbol set `s`. Special codes: 0 =
> epsilon "<>", 1 = unknown "?", 2 = identity. For each symbol in `s`, flag
> diacritics (`FdOperation::is_diacritic`) are skipped; symbol codes are obtained
> via `t->alphabet.symbol2code` (a -1 result triggers a stderr error message and
> `assert(false)`). Cases by label:
> 1. `lower==1 && upper==1` (non-identity "?:?"): for every ordered pair
>    (in-symbol, out-symbol) drawn from `s` with `inumber != onumber`, add arc
>    `Label(inumber,onumber)`; additionally for each in-symbol add `Label(inumber,
>    1)` (x:?) and `Label(1, inumber)` (?:x).
> 2. `lower==2 || upper==2` (identity): for each symbol add identity arc
>    `Label(number, number)`.
> 3. `lower==1` ("?:x"): for each symbol add `Label(number, l.upper_char())`.
> 4. `upper==1` ("x:?"): for each symbol add `Label(l.lower_char(), number)`.
> The original arc `l` is NOT removed — the new arcs are added alongside it
> (comment: "keep the original transition in all cases"). No return value.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.expand2-fn]
> void SfstTransducer::expand2

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.expand2-fn]
> Recursive depth-first traversal that expands every arc of `t`. Parameters: `t`,
> current `node`, the `new_symbols` StringSet, and `visited_nodes` (set of already
> processed nodes). If `node` is already in `visited_nodes`, returns immediately.
> Otherwise inserts `node` into `visited_nodes`, then iterates over its outgoing
> arcs; for each arc it first recurses into `arc->target_node()` (post-order
> descent), then calls `expand_node(t, node, l, arc->target_node(),
> new_symbols)` with the arc's label `l` to add the expansion arcs. No return
> value. Note: new arcs added by `expand_node` are not themselves recursed into
> because the loop iterates the arc list captured at entry / and targets are
> guarded by `visited_nodes`.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.extract-input-language-fn]
> Transducer * SfstTransducer::extract_input_language(Transducer * t)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.extract-input-language-fn]
> Projects `t` onto its input (lower) side. Steps: `retval = &t->lower_level()`
> (SFST lower projection). Because the projection's alphabet would only contain
> symbols actually appearing on the input side, restore the full alphabet: iterate
> the original `t->alphabet.get_char_map()` and for each entry call
> `retval->alphabet.add_symbol(it->second, it->first)` (symbol string, code).
> Then replace unknowns with identities: `tmp = retval; retval =
> substitute(retval, internal_unknown, internal_identity); delete tmp;`. Return the
> substituted `retval`.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.extract-output-language-fn]
> Transducer * SfstTransducer::extract_output_language(Transducer * t)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.extract-output-language-fn]
> Projects `t` onto its output (upper) side. First calls `t->complete_alphabet()`
> (unlike the input-language variant). Then `retval = &t->upper_level()` (SFST
> upper projection). Restores the full alphabet by iterating
> `t->alphabet.get_char_map()` and calling `retval->alphabet.add_symbol(it->second,
> it->first)` for each entry. Then replaces unknowns with identities: `tmp =
> retval; retval = substitute(retval, internal_unknown, internal_identity); delete
> tmp;`. Returns the substituted `retval`.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.extract-path-transducers-fn]
> std::vector<Transducer*> SfstTransducer::extract_path_transducers

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.extract-path-transducers-fn]
> Returns a vector of single-path transducers, one per path of `t`. Steps:
> declare `vector<Transducer*> paths` and call `t->enumerate_paths(paths)` (SFST
> fills it with path transducers whose alphabets lack the special symbols). Create
> a reference transducer `foo = define_transducer(internal_epsilon)`. For each
> path `i`: copy the original alphabet into it via
> `paths[i]->alphabet.copy(t->alphabet)`, then harmonize it against `foo` with
> `harmonize(paths[i], foo, false)` and replace `paths[i]` with `harm.first`. After
> the loop `delete foo` and return `paths`. Effect: each enumerated path transducer
> ends up with the harmonized full alphabet of `t`.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.extract-paths-fn]
> void SfstTransducer::extract_paths

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.extract-paths-fn]
> Public path-enumeration driver. Parameters: `t`, `callback`
> (hfst::ExtractStringsCb&), cycle limit `cycles`, optional flag-diacritic table
> `fd`, and `filter_fd`. Steps: if `t->root_node()` is null, return. Create two
> empty HfstNode2Int maps `all_visitations` and `path_visitations`. Build the
> flag-diacritic state stack: if `fd == NULL` set `fd_state_stack = NULL`,
> otherwise allocate `new std::vector<FdState<Character>>(1,
> FdState<Character>(*fd))` (a one-element stack holding the initial fd state).
> Create empty `StringPairVector spv` and call the file-static recursive
> `hfst::implementations::extract_paths(t, t->root_node(), all_visitations,
> path_visitations, callback, cycles, fd_state_stack, filter_fd, spv)` to walk all
> paths. Afterwards, if the root node exists and is final, emit the empty
> (epsilon) path: build `HfstTwoLevelPath epsilon_path(0, empty_spv)` and call
> `callback(epsilon_path, true)`. Finally, if `fd_state_stack` was allocated,
> `delete` it. No return value.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.extract-random-paths-fn]
> void SfstTransducer::extract_random_paths

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.extract-random-paths-fn]
> Fills `results` (HfstTwoLevelPaths set) with up to `max_num` random paths of
> `t`. Steps: if `is_minimal_and_empty(t)` (root has no arcs), return immediately.
> Record `is_epsilon_path_accepted = t->root_node()->is_final()`. Seed the C RNG
> with `srand((unsigned)time(0))`. Loop while `max_num > 0`: generate a path via
> `random_path(t)`; if that path is already in `results`, retry `random_path(t)`
> up to 5 times to get a distinct one. If the empty path is NOT accepted yet
> `path.second.size() == 0` (a spurious epsilon path), print "wrong epsilon path
> returned, retrying...\n" to stderr and `continue` WITHOUT decrementing
> `max_num`. Otherwise insert `path` into `results` and decrement `max_num`. No
> return value. (Duplicates that survive the 5 retries may still be re-inserted
> into the set, which simply has no effect, but `max_num` is still decremented.)

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.get-alphabet-fn]
> StringSet SfstTransducer::get_alphabet(Transducer * t)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.get-alphabet-fn]
> Returns the StringSet of symbol strings in `t`'s alphabet. Obtains
> `t->alphabet.get_char_map()` and iterates every entry; for each, if the symbol
> string `it->second` equals "<>" it inserts `internal_epsilon` into the set,
> otherwise it inserts `std::string(it->second)`. Returns the resulting set. Pure
> read.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.get-biggest-symbol-number-fn]
> unsigned int SfstTransducer::get_biggest_symbol_number(Transducer * t)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.get-biggest-symbol-number-fn]
> Returns the largest symbol code in `t`'s alphabet. Initializes `biggest_number =
> 0`, iterates `t->alphabet.get_char_map()`, and for each entry whose code
> (`it->first`) exceeds the current `biggest_number` updates it. Returns
> `biggest_number` (0 if the alphabet is empty).

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.get-flag-diacritics-fn]
> FdTable<SFST::Character>* SfstTransducer::get_flag_diacritics(Transducer * t)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.get-flag-diacritics-fn]
> Builds and returns a newly allocated `FdTable<SFST::Character>*` containing the
> flag diacritics present in `t`'s alphabet. Allocates `new
> FdTable<SFST::Character>()`, iterates `t->alphabet.get_char_map()`, and for each
> entry whose symbol string is a flag diacritic
> (`FdOperation::is_diacritic(it->second)`) calls
> `table->define_diacritic(it->first, it->second)` (code, symbol). Returns the
> table (caller owns it).

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.get-profile-seconds-fn]
> float SfstTransducer::get_profile_seconds()

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.get-profile-seconds-fn]
> Returns the file-scope float counter `sfst_seconds_in_harmonize` (which is
> initialized to 0 and accumulates time spent in harmonize). Pure getter.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.get-symbol-map-fn]
> std::map<std::string, unsigned int> SfstTransducer::get_symbol_map

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.get-symbol-map-fn]
> Returns a `std::map<std::string, unsigned int>` mapping each alphabet symbol to
> its code. Calls `get_alphabet(t)` to obtain the symbol StringSet, then for each
> symbol `*it` sets `symbol_map[*it] = get_symbol_number(t, it->c_str())`. Returns
> the map.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.get-symbol-number-fn]
> unsigned int SfstTransducer::get_symbol_number

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.get-symbol-number-fn]
> Returns the alphabet code for `symbol`. If `symbol == "@_EPSILON_SYMBOL_@"`,
> returns 0. Otherwise looks up `i = t->alphabet.symbol2code(symbol.c_str())`; if
> the lookup returns `EOF` (symbol not found), throws `SymbolNotFoundException`.
> Otherwise returns `(unsigned int)i`.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.get-symbol-pairs-fn]
> StringPairSet SfstTransducer::get_symbol_pairs(Transducer *t)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.get-symbol-pairs-fn]
> Returns the StringPairSet of label pairs in `t`'s alphabet. Declares empty
> `StringPairSet s`, then (mutating `t`) calls `t->alphabet.clear_char_pairs()`
> followed by `t->complete_alphabet()` to regenerate the label pair set. Iterates
> the alphabet's label set (`begin()..end()`); for each label, resolves
> `isymbol = code2symbol(it->lower_char())` and `osymbol =
> code2symbol(it->upper_char())`. If either is NULL, throws `HfstFatalException`
> ("input number not found" or "output number not found" respectively). Builds
> `istring`/`ostring` from those (a no-op "<>"→"<>" remap is present) and inserts
> `StringPair(istring, ostring)` into `s`. Returns `s`.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.get-symbol-vector-fn]
> StringVector SfstTransducer::get_symbol_vector

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.get-symbol-vector-fn]
> Returns a `StringVector` indexed by symbol code. Computes `biggest =
> get_biggest_symbol_number(t)`, reserves and resizes a `symbol_vector` to
> `biggest+1` entries all initialized to "". Then obtains `get_alphabet(t)` and for
> each symbol `*it` computes `symbol_number = get_symbol_number(t, it->c_str())`
> and assigns `symbol_vector.at(symbol_number) = *it`. Returns the vector; indices
> with no symbol remain empty strings.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.harmonize-fn]
> std::pair<Transducer*, Transducer*> SfstTransducer::harmonize

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.harmonize-fn]
> Harmonizes two transducers so their alphabets and unknown/identity arcs match.
> Parameters `(t1, t2, unknown_symbols_in_use)`. Whole body is wrapped in a
> try/catch that converts any thrown `const char *` into `HfstFatalException`.
> Steps:
> 1. Declare `StringSet unknown_t1, unknown_t2`. If `unknown_symbols_in_use`, get
>    `t1_symbols = get_alphabet(t1)` and `t2_symbols = get_alphabet(t2)` and call
>    `hfst::symbols::collect_unknown_sets(t1_symbols, unknown_t1, t2_symbols,
>    unknown_t2)` to fill the two unknown sets (symbols in one but not the other).
> 2. Merge alphabets: `new_t1 = &t1->copy(false, &t2->alphabet)`; then
>    `new_t1->alphabet.insert_symbols(t2->alphabet)`; then iterate
>    `t1->alphabet.get_char_map()` and `new_t1->alphabet.add_symbol(it->second)`
>    for each; then `t2->alphabet.insert_symbols(new_t1->alphabet)`. `delete t1`
>    and set `t1 = new_t1`.
> 3. Compute the harmonized pair. If `unknown_symbols_in_use`: `harmonized_t1 =
>    expand_arcs(t1, unknown_t1); delete t1;` and `harmonized_t2 = expand_arcs(t2,
>    unknown_t2); delete t2;`. Otherwise just deep-copy: `harmonized_t1 =
>    &t1->copy(); harmonized_t2 = &t2->copy();`.
> 4. Return `std::pair<Transducer*,Transducer*>(harmonized_t1, harmonized_t2)`.
> Note the function takes ownership and deletes the inputs along the way.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.initialize-alphabet-fn]
> void SfstTransducer::initialize_alphabet(Transducer *t)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.initialize-alphabet-fn]
> Resets `t`'s alphabet to the standard HFST/SFST special-symbol layout. Steps:
> `t->alphabet.clear()`; set `t->alphabet.utf8 = true`; add the three reserved
> symbols with fixed codes — `add_symbol("<>", 0)` (epsilon),
> `add_symbol(internal_unknown.c_str(), 1)` (unknown "?"), and
> `add_symbol(internal_identity.c_str(), 2)` (identity). No return value.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.insert-freely-fn]
> Transducer * SfstTransducer::insert_freely

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.insert-freely-fn]
> Freely inserts the symbol pair `symbol_pair` at every state of `t`. Copies
> `isymbol = symbol_pair.first`, `osymbol = symbol_pair.second`; if either is
> epsilon (`is_epsilon`) replaces it with the literal "<>". Then returns
> `&t->freely_insert(Label(t->alphabet.add_symbol(isymbol.c_str()),
> t->alphabet.add_symbol(osymbol.c_str())))` — i.e. resolves both symbols to codes
> (adding them to the alphabet if needed), builds the Label, and delegates to
> SFST's `freely_insert`, returning the address of the resulting transducer.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.insert-to-alphabet-fn]
> void SfstTransducer::insert_to_alphabet

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.insert-to-alphabet-fn]
> Adds `symbol` to `t`'s alphabet via `t->alphabet.add_symbol(symbol.c_str())`
> (assigning it a fresh code if not already present). No return value.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.intersect-fn]
> Transducer * SfstTransducer::intersect

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.intersect-fn]
> Intersects `t1` with `t2` by invoking SFST's intersection operator
> `t1->operator&(*t2)` and returns the address of the resulting transducer
> reference.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.invert-fn]
> Transducer * SfstTransducer::invert(Transducer * t)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.invert-fn]
> Inverts `t` (swaps input and output sides) by returning `&t->switch_levels()`
> (address of the reference returned by SFST's `switch_levels`).

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.is-automaton-fn]
> bool SfstTransducer::is_automaton(Transducer * t)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.is-automaton-fn]
> Returns `t->is_automaton()`, delegating to SFST: true iff every arc of `t` has
> equal lower and upper characters (i.e. the transducer is an acceptor/automaton).

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.is-cyclic-fn]
> bool SfstTransducer::is_cyclic(Transducer * t)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.is-cyclic-fn]
> Returns `t->is_cyclic()`, delegating to SFST: true iff the transducer's graph
> contains a cycle.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.minimize-fn]
> Transducer * SfstTransducer::minimize(Transducer * t)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.minimize-fn]
> Minimizes `t`: `retval = &t->minimise(false)` (SFST minimisation, the `false`
> argument suppressing verbose output). SFST's minimise may drop alphabet symbols,
> so it restores them with `retval->alphabet.copy(t->alphabet)` and returns
> `retval`.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.number-of-arcs-fn]
> unsigned int SfstTransducer::number_of_arcs(Transducer* t)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.number-of-arcs-fn]
> Returns the number of arcs (transitions) in `t`. Declares a local
> `std::vector<SFST::Node*> indexing`, calls `t->nodeindexing(&indexing)` which
> returns a `std::pair<size_t, size_t>` of (node count, transition count), and
> returns the `.second` member (the transition/arc count). Pure read aside from
> the indexing side-effect.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.number-of-states-fn]
> unsigned int SfstTransducer::number_of_states(Transducer* t)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.number-of-states-fn]
> Returns the number of states (nodes) in `t`. Declares a local
> `std::vector<SFST::Node*> indexing`, calls `t->nodeindexing(&indexing)` which
> returns a `std::pair<size_t, size_t>` of (node count, transition count), and
> returns the `.first` member (the node count). Pure read aside from the indexing
> side-effect.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.optionalize-fn]
> Transducer * SfstTransducer::optionalize(Transducer * t)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.optionalize-fn]
> Makes `t` optional (adds the empty string to its language). Steps: create
> `eps = create_epsilon_transducer()` (accepts only epsilon), compute
> `opt = &(*t | *eps)` (SFST disjunction/union operator `|`), `delete eps`, and
> return `opt`. Effect: returns a new transducer accepting the language of `t`
> plus the empty string.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.print-alphabet-fn]
> void SfstTransducer::print_alphabet(Transducer *t)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.print-alphabet-fn]
> Debug-prints `t`'s alphabet to stderr. Prints the header line "alphabet..\n",
> obtains `cm = t->alphabet.get_char_map()`, then for each entry prints
> `"%i\t%s\n"` with the code (`it->first`) and symbol string (`it->second`).
> Finally prints "..alphabet\n". No return value; stderr I/O only.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.print-test-fn]
> void SfstTransducer::print_test(Transducer *t)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.print-test-fn]
> Debug helper that streams the transducer to stderr: `std::cerr << *t;` using
> SFST's `Transducer` stream-insertion operator. No return value; stderr I/O only.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.remove-epsilons-fn]
> Transducer * SfstTransducer::remove_epsilons(Transducer * t)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.remove-epsilons-fn]
> Removes epsilon transitions from `t` by returning `&t->remove_epsilons()`
> (address of the reference returned by SFST's `Transducer::remove_epsilons`).

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.remove-from-alphabet-fn]
> void SfstTransducer::remove_from_alphabet

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.remove-from-alphabet-fn]
> Removes `symbol` from `t`'s alphabet (both the symbol table and any label pairs
> referencing it), IN PLACE. Steps: bind `alpha = t->alphabet`,
> `symbol_to_remove = symbol.c_str()`. Build three temporary vectors: `sym`
> (char*), `code` (Character), `label` (Label).
> 1. Iterate `alpha.get_char_map()`; for each entry (code `c`, symbol `s`) whose
>    string `s` does NOT equal `symbol_to_remove` (strcmp != 0), push
>    `fst_strdup(s)` onto `sym` and `c` onto `code` (preserving all other
>    symbols).
> 2. Iterate the alphabet's label set (`alpha.begin()..alpha.end()`); for each
>    label `l`, if neither `code2symbol(l.upper_char())` nor
>    `code2symbol(l.lower_char())` equals `symbol_to_remove`, push `l` onto
>    `label` (keep labels not touching the removed symbol).
> 3. `alpha.clear()`.
> 4. For each saved symbol `i`, `alpha.add_symbol(sym[i], code[i])` then
>    `free(sym[i])` (re-add with original code, freeing the strdup'd copy).
> 5. For each saved label, `alpha.insert(label[i])`.
> No return value.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.repeat-le-n-fn]
> Transducer * SfstTransducer::repeat_le_n(Transducer * t, unsigned int n)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.repeat-le-n-fn]
> Builds a transducer accepting between 0 and `n` (inclusive) concatenations of
> `t`'s language. Steps: `result = create_empty_transducer()` (empty language).
> Loop `i` from 0 to `n` inclusive (`i < n+1`): compute `power = repeat_n(t, i)`
> (i-fold concatenation; for i=0 this is the epsilon transducer), then
> `temp = &(*power | *result)` (SFST union `|`), `delete power`, `delete result`,
> `result = temp`. After the loop return `result`. Effect: union of t^0, t^1, ...,
> t^n.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.repeat-n-fn]
> Transducer * SfstTransducer::repeat_n(Transducer * t, unsigned int n)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.repeat-n-fn]
> Builds a transducer accepting exactly `n` concatenations of `t`'s language
> (t^n). Steps: `power = create_epsilon_transducer()` (the n=0 base case, accepts
> only epsilon). Loop `i` from 0 to `n-1` (`i < n`): compute
> `temp = &(*power + *t)` (SFST concatenation `+`), `delete power`, `power = temp`.
> After the loop return `power`. For n=0 returns the epsilon transducer
> unchanged.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.repeat-plus-fn]
> Transducer * SfstTransducer::repeat_plus(Transducer * t)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.repeat-plus-fn]
> Builds the Kleene-plus (one or more concatenations) of `t`. Steps:
> `star = repeat_star(t)` (Kleene star of `t`), then `t = &(*t + *star)` (SFST
> concatenation `+` of `t` with its star, i.e. t followed by t*), `delete star`,
> and return the resulting `t` pointer. Note the parameter pointer `t` is reused
> to hold the concatenation result.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.repeat-star-fn]
> Transducer * SfstTransducer::repeat_star(Transducer * t)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.repeat-star-fn]
> Builds the Kleene-star (zero or more concatenations) of `t` by returning
> `&t->kleene_star()` (address of the reference returned by SFST's
> `Transducer::kleene_star`).

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.reverse-fn]
> Transducer * SfstTransducer::reverse(Transducer * t)

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.reverse-fn]
> Reverses `t` (reverses every accepted string/relation) by returning
> `&t->reverse()` (address of the reference returned by SFST's
> `Transducer::reverse`).

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.substitute-fn]
> Transducer * SfstTransducer::substitute

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.substitute-fn]
> The annotated overload is
> `substitute(Transducer *t, const StringPair &symbol_pair, Transducer *tr)`,
> which replaces every arc in `t` labelled `symbol_pair` with the transducer `tr`
> (splice substitution). Steps: copy `isymbol = symbol_pair.first`,
> `osymbol = symbol_pair.second`; if `is_epsilon(isymbol)` set isymbol = "<>";
> if `is_epsilon(osymbol)` set osymbol = "<>". Resolve both to codes via
> `t->alphabet.add_symbol(...)` and build `Label(icode, ocode)`. Call
> `retval = &t->splice(label, tr)` (SFST splice). Restore the full alphabet onto
> the result via `retval->alphabet.copy(t->alphabet)` and return `retval`.

> [spec:hfst:def:sfst-transducer.hfst.implementations.sfst-transducer.subtract-fn]
> Transducer * SfstTransducer::subtract

> [spec:hfst:sem:sfst-transducer.hfst.implementations.sfst-transducer.subtract-fn]
> Computes the language difference `t1 - t2`. Whole body in a try block. Steps:
> record `t1_alphabet_size = t1->alphabet.size()`. SFST's subtraction internally
> computes a negation that fails on an empty alphabet, so if
> `t1_alphabet_size == 0`, insert a dummy label pair `t1->alphabet.insert(Label(1,
> 1))`. Compute `retval = &t1->operator/(*t2)` (SFST subtraction `/`). If the
> alphabet was originally empty, undo the dummy: `t1->alphabet.clear_char_pairs()`
> then `t1->complete_alphabet()`. Return `retval`. The catch clause handles a
> thrown `const char *msg`: prints `"ERROR: %s\n"` to stderr and throws
> `HfstFatalException` with that message.

> [spec:hfst:def:sfst-transducer.hfst.implementations.transducer]
> typedef SFST::Transducer Transducer

> [spec:hfst:def:sfst-transducer.main-fn]
> int main(int argc, char * argv[])

> [spec:hfst:sem:sfst-transducer.main-fn]
> Unit-test entry point compiled only under MAIN_TEST. When `HAVE_SFST` is
> defined: prints "Unit tests for <file>:" to stdout, then exercises alphabet
> pruning. Builds `t = SfstTransducer::define_transducer("a", "b")`. Then for each
> of these operations it asserts the result's alphabet still contains the expected
> symbols (via the free function `does_sfst_alphabet_contain`):
> 1. `t_input = extract_input_language(t)` — asserts contains "a" and "b".
> 2. `t_output = extract_output_language(t)` — asserts contains "a" and "b".
> 3. `t_min = minimize(t_input)` — asserts contains "a" and "b".
> 4. `t_eps_free = remove_epsilons(t_output)` — asserts contains "a" and "b".
> 5. `t_subst = substitute(t, "a", "c")` — asserts contains "a", "b" and "c".
> Then prints newline + "ok" + newline and returns `EXIT_SUCCESS`. When
> `HAVE_SFST` is not defined: prints a skip message ("Skipping unit tests for
> <file>, SfstTransducer has not been enabled") and returns 77 (the automake
> "skipped test" exit code).

> [spec:hfst:def:sfst-transducer.sfst.character]
> typedef short unsigned int Character

