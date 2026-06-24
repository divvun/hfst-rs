# libhfst/src/implementations/XfsmTransducer.cc, libhfst/src/implementations/XfsmTransducer.h

> [spec:hfst:def:xfsm-transducer.hfst.implementations.hfst-symbol-to-xfsm-symbol-fn]
> static id_type hfst_symbol_to_xfsm_symbol(const std::string & symbol)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.hfst-symbol-to-xfsm-symbol-fn]
> Forward declaration only (no body at this site): declares a file-local
> (`static`) free function `hfst_symbol_to_xfsm_symbol(const std::string & symbol)
> -> id_type`. It exists so that the declaration is visible before use; the
> actual conversion logic is provided by the member function
> `XfsmTransducer::hfst_symbol_to_xfsm_symbol` (see that rule). In a Rust port
> this declaration carries no behaviour of its own and need not be reproduced.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.label-id-to-symbol-pair-fn]
> static void label_id_to_symbol_pair(id_type label_id, std::string & isymbol, std::string & osymbol)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.label-id-to-symbol-pair-fn]
> Forward declaration only (no body at this site): declares a file-local
> (`static`) free function `label_id_to_symbol_pair(id_type label_id,
> std::string & isymbol, std::string & osymbol) -> void`. The behaviour is
> implemented by the member function `XfsmTransducer::label_id_to_symbol_pair`
> (see that rule). This declaration carries no behaviour of its own and need
> not be reproduced in a Rust port.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.symbol-pair-to-label-id-fn]
> static id_type symbol_pair_to_label_id(const std::string & isymbol, const std::string & osymbol)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.symbol-pair-to-label-id-fn]
> Forward declaration only (no body at this site): declares a file-local
> (`static`) free function `symbol_pair_to_label_id(const std::string & isymbol,
> const std::string & osymbol) -> id_type`. The behaviour is implemented by the
> member function `XfsmTransducer::symbol_pair_to_label_id` (see that rule).
> This declaration carries no behaviour of its own and need not be reproduced
> in a Rust port.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-input-stream]
> class XfsmInputStream {
>   std::string filename;
>   NVptr net_list;
>   int list_size;
>   int list_pos;
> }

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-input-stream.close-fn]
> void XfsmInputStream::close(void)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-input-stream.close-fn]
> Closes the input stream and releases its loaded transducers. Calls
> `free_nv_and_nets(net_list)` to free both the net-vector container and every
> net it holds, then sets `list_size = -1` and `list_pos = -1`. Returns void.
> No exceptions, no other side effects. (Note: it does not null out `net_list`
> after freeing.)

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-input-stream.is-bad-fn]
> bool XfsmInputStream::is_bad(void)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-input-stream.is-bad-fn]
> Returns whether the stream is bad for reading. Implemented by simply
> returning the result of `is_eof()` (i.e. true exactly when `list_pos >=
> list_size`). No side effects.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-input-stream.is-eof-fn]
> bool XfsmInputStream::is_eof(void)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-input-stream.is-eof-fn]
> Returns whether all transducers in the loaded net list have been consumed.
> Returns the boolean `list_pos >= list_size`. No side effects. (After
> `close()` sets both fields to -1, this also returns true since -1 >= -1.)

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-input-stream.is-fst-fn]
> bool is_fst(void)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-input-stream.is-fst-fn]
> Declared in the header (`bool is_fst(void)`) but has no definition anywhere in
> the codebase; it is never linked-call-resolved. Per the header documentation,
> it is intended to report whether the next item returned by `read_transducer()`
> is a valid XFSM transducer, which is basically always true except when
> `is_eof()` is true. A Rust port that needs this method should return
> `!is_eof()`; otherwise it can be omitted since the C++ never provides a body.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-input-stream.is-good-fn]
> bool XfsmInputStream::is_good(void)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-input-stream.is-good-fn]
> Returns whether the stream is good for reading. Returns the logical negation
> of `is_bad()` (i.e. `not is_bad()`), which is therefore `!is_eof()`. No side
> effects.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-input-stream.read-transducer-fn]
> NETptr XfsmInputStream::read_transducer()

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-input-stream.read-transducer-fn]
> Returns the next transducer from the loaded net list, advancing the cursor.
> Steps: (1) if `is_eof()` (no transducers remaining), return NULL immediately.
> (2) Otherwise fetch the net at the current position via `nv_get(net_list,
> list_pos)`. (3) If that returns NULL, throw `StreamNotReadableException`.
> (4) Increment `list_pos`. (5) Return a deep copy of the fetched net produced
> by `XfsmTransducer::copy(retval)` (i.e. `copy_net`), so the caller owns an
> independent transducer and the stream's stored net is left intact.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-input-stream.xfsm-input-stream-fn]
> XfsmInputStream::XfsmInputStream(const std::string &filename_)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-input-stream.xfsm-input-stream-fn]
> Constructs an XfsmInputStream that reads transducers from file `filename_`.
> Member init: `filename = filename_`, `net_list = NULL`, `list_size = -1`,
> `list_pos = -1`. Then: (1) if `filename` is empty, throw
> `FunctionNotImplementedException` (message about `("")` not supported).
> (2) Otherwise open the file for reading via `hfst::hfst_fopen(filename,"r")`;
> if the FILE* is NULL, throw `StreamNotReadableException`; otherwise `fclose`
> it immediately (this open is only an existence/readability check).
> (3) `strdup` the filename into `fn`, get the default cfsm context via
> `get_default_cfsm_context()`, and load all nets with `load_nets(fn,
> fst_cntxt)` into `net_list`; then `free(fn)`. (4) If `net_list` is NULL, throw
> `StreamNotReadableException`. (5) Set `list_size = NV_len(net_list)`; if
> `list_size <= 0`, throw `HfstFatalException`. (6) Set `list_pos = 0`.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-output-stream]
> class XfsmOutputStream {
>   std::string filename;
>   NVptr net_list;
> }

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-output-stream.close-fn]
> void XfsmOutputStream::close(void)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-output-stream.close-fn]
> No-op. The body is empty (only a comment): because output uses filenames and
> the file is opened/closed elsewhere, there is nothing to close here. Returns
> void with no side effects.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-output-stream.flush-fn]
> void XfsmOutputStream::flush()

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-output-stream.flush-fn]
> Writes all accumulated transducers to the output file and frees the buffer.
> Steps: (1) if `net_list != NULL`: obtain the default cfsm context via
> `get_default_cfsm_context()`, `strdup` `filename` into `fn`, call
> `save_nets(net_list, fn, cptr)`; if its return is non-zero (error) throw
> `HfstFatalException` ("an error happened when writing an xfsm transducer");
> then `free(fn)`. (2) Unconditionally call `free_nv_and_nets(net_list)` to free
> the net vector and its nets, then set `net_list = NULL`. If `net_list` was
> already NULL, only the free/reset (step 2) runs and nothing is written.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-output-stream.write-transducer-fn]
> void XfsmOutputStream::write_transducer(NETptr transducer)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-output-stream.write-transducer-fn]
> Buffers a transducer for delayed writing (actual file write happens in
> `flush()`). Steps: (1) if `net_list` is NULL, create an empty net vector via
> `make_nv(0)` and store it in `net_list`. (2) Append a deep copy of the given
> transducer — `XfsmTransducer::copy(transducer)` (i.e. `copy_net`) — to the
> net vector with `nv_add(copy, net_list)`. The original transducer is not
> modified or taken ownership of. Returns void.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-output-stream.xfsm-output-stream-fn]
> XfsmOutputStream::XfsmOutputStream(const std::string &str)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-output-stream.xfsm-output-stream-fn]
> Constructs an XfsmOutputStream targeting file `str`. Member init: `filename =
> str`, `net_list = NULL`. Then: (1) if `filename` is non-empty, open it for
> binary writing via `hfst::hfst_fopen(filename, "wb")`; if the FILE* is NULL,
> throw `StreamNotReadableException`; otherwise `fclose` it immediately (this
> just truncates/creates the file and verifies writability, since the XFSM API
> only writes by filename later in `flush`). (2) If `filename` is empty, throw
> a C-string `"XfsmOutputStream::XfsmOutputStream(\"\") not supported"`.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-symbol-to-hfst-symbol-fn]
> static std::string xfsm_symbol_to_hfst_symbol(id_type id)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-symbol-to-hfst-symbol-fn]
> Forward declaration only (no body at this site): declares a file-local
> (`static`) free function `xfsm_symbol_to_hfst_symbol(id_type id) ->
> std::string`. The behaviour is implemented by the member function
> `XfsmTransducer::xfsm_symbol_to_hfst_symbol` (see that rule). This declaration
> carries no behaviour of its own and need not be reproduced in a Rust port.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer]
> class XfsmTransducer {
>   static bool minimize_even_if_already_minimal_;
> }

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.add-symbol-to-alphabet-fn]
> void XfsmTransducer::add_symbol_to_alphabet(NETptr t, const std::string & symbol)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.add-symbol-to-alphabet-fn]
> Adds a single HFST symbol to the transducer's alphabet (sigma). Steps:
> (1) get the alphabet pointer `ap = net_sigma(t)`. (2) If `symbol` is the
> epsilon, unknown, or identity special symbol (`hfst::is_epsilon` /
> `is_unknown` / `is_identity`), return immediately without adding anything.
> (3) Otherwise convert the symbol to its XFSM id via
> `XfsmTransducer::hfst_symbol_to_xfsm_symbol(symbol)` and add it to the
> alphabet with `alph_add_to(ap, id, DONT_KEEP)`. Returns void; mutates `t`'s
> sigma.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.add-symbols-to-alphabet-fn]
> void XfsmTransducer::add_symbols_to_alphabet(NETptr t, const StringSet & symbols)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.add-symbols-to-alphabet-fn]
> Adds a set of HFST symbols to the transducer's alphabet (sigma). Steps:
> (1) get `ap = net_sigma(t)`. (2) Iterate over every symbol in `symbols`; for
> each, if it is epsilon, unknown, or identity (`hfst::is_epsilon` /
> `is_unknown` / `is_identity`), skip it; otherwise convert via
> `XfsmTransducer::hfst_symbol_to_xfsm_symbol` and add it with `alph_add_to(ap,
> id, DONT_KEEP)`. Returns void; mutates `t`'s sigma.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.are-equivalent-fn]
> bool XfsmTransducer::are_equivalent(NETptr t1, NETptr t2)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.are-equivalent-fn]
> Returns whether two transducers are equivalent. Delegates to the XFSM library
> `test_equivalent(t1, t2)` and returns true iff that call returns 1. No
> mutation of the arguments; pure boolean result.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.compose-fn]
> NETptr XfsmTransducer::compose(NETptr t1, const NETptr t2)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.compose-fn]
> Composes transducer `t1` with `t2` and returns the resulting net. Calls
> `compose_net(t1, t2, DONT_KEEP, KEEP)`: the `DONT_KEEP` for the first operand
> means `t1` may be consumed/freed by the operation, and `KEEP` for the second
> means `t2` is preserved (the `const` is cast away). Returns the composed net.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.concatenate-fn]
> NETptr XfsmTransducer::concatenate(NETptr t1, const NETptr t2)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.concatenate-fn]
> Concatenates transducer `t1` with `t2` and returns the result. Calls
> `concat_net(t1, t2, DONT_KEEP, KEEP)`: `t1` may be consumed/freed
> (`DONT_KEEP`), `t2` is preserved (`KEEP`, const cast away). Returns the
> concatenated net.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.copy-fn]
> NETptr XfsmTransducer::copy(NETptr t)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.copy-fn]
> Returns a deep copy of net `t` by delegating to the XFSM library function
> `copy_net(t)`. The argument is not modified; the caller owns the returned
> independent net.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.create-empty-transducer-fn]
> NETptr XfsmTransducer::create_empty_transducer(void)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.create-empty-transducer-fn]
> Returns a new empty net (a transducer recognising the empty language) by
> calling `null_net()`. No arguments, no side effects beyond the allocation.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.create-epsilon-transducer-fn]
> NETptr XfsmTransducer::create_epsilon_transducer(void)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.create-epsilon-transducer-fn]
> Returns a new net accepting only the empty string (epsilon). Steps: create an
> empty net via `null_net()`, then make it optional with `optional_net(result,
> DONT_KEEP)` and return that. Since the empty net accepts nothing, optionalising
> it yields a net whose only accepted string is epsilon. `DONT_KEEP` means the
> intermediate `result` may be consumed by `optional_net`.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.create-xfsm-identity-to-identity-transducer-fn]
> NETptr XfsmTransducer::create_xfsm_identity_to_identity_transducer()

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.create-xfsm-identity-to-identity-transducer-fn]
> Builds a two-state net with a single arc representing the identity-to-identity
> relation. Steps: (1) `result = null_net()`. (2) add a final state via
> `add_state_to_net(result, 1)` (the `1` marks it final), stored as `final`.
> (3) use the atomic label id `ti = OTHER` (the atomic OTHER label encodes the
> identity pair). (4) add an arc from `result->start.state` to `final` with
> label `ti` via `add_arc_to_state(result, start, ti, final, NULL, 0)`; if it
> returns NULL throw the C-string `"add_arc_to_state failed"`. (5) return
> `result`. Contrast with the unknown-to-unknown builder which uses
> `id_pair_to_id(OTHER, OTHER)` instead of the atomic OTHER.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.create-xfsm-unknown-to-unknown-transducer-fn]
> NETptr XfsmTransducer::create_xfsm_unknown_to_unknown_transducer()

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.create-xfsm-unknown-to-unknown-transducer-fn]
> Builds a two-state net with a single arc representing the unknown-to-unknown
> relation. Steps: (1) `result = null_net()`. (2) add a final state via
> `add_state_to_net(result, 1)`, stored as `final`. (3) build the non-atomic
> label id `ti = id_pair_to_id(OTHER, OTHER)` (a paired OTHER:OTHER label
> encodes the unknown pair). (4) add an arc from `result->start.state` to
> `final` with label `ti` via `add_arc_to_state(result, start, ti, final, NULL,
> 0)`; if it returns NULL throw `"add_arc_to_state failed"`. (5) return
> `result`.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.define-transducer-fn]
> NETptr XfsmTransducer::define_transducer(const std::vector<StringPairSet> &spsv)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.define-transducer-fn]
> Builds a transducer from a sequence of symbol-pair sets by constructing an
> XFSM regex string and compiling it. The vector `spsv` is a list of "positions";
> each position is a StringPairSet that becomes one bracketed disjunction in the
> regex. Steps: start with empty `regex`. For each StringPairSet `it1` in order:
> append `"["`; then for each pair `it2` in that set, if it is not the first pair
> append `" | "`, and append `"\"" + it2->first + "\":\"" + it2->second + "\""`
> (an input:output label literal); after the inner loop append `"] "`. After all
> positions, call `read_regex(regex.c_str())` and return its result. The overall
> regex is thus the concatenation of bracketed alternations, so the resulting
> net accepts, at each position, any one of that position's input:output pairs.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.disjunct-fn]
> NETptr XfsmTransducer::disjunct(NETptr t1, const NETptr t2)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.disjunct-fn]
> Returns the union (disjunction) of `t1` and `t2`. Calls `union_net(t1, t2,
> DONT_KEEP, KEEP)`: `t1` may be consumed/freed (`DONT_KEEP`), `t2` is preserved
> (`KEEP`, const cast away). Returns the union net.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.eliminate-flag-xfsm-fn]
> NETptr XfsmTransducer::eliminate_flag_xfsm(NETptr t, const std::string & flag)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.eliminate-flag-xfsm-fn]
> Eliminates a single named flag diacritic from net `t`. Steps: `strdup` the
> `flag` string into a mutable C string `f`; call `eliminate_flag(t, f,
> DONT_KEEP)` (restricting elimination to that one flag), capturing the result;
> `free(f)`; return the result net. `DONT_KEEP` means `t` may be consumed.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.eliminate-flags-xfsm-fn]
> NETptr XfsmTransducer::eliminate_flags_xfsm(NETptr t)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.eliminate-flags-xfsm-fn]
> Eliminates all flag diacritics from net `t`. Calls `eliminate_flag(t, NULL,
> DONT_KEEP)` — passing NULL as the flag name means eliminate every flag rather
> than a specific one — and returns the resulting net. `DONT_KEEP` means `t` may
> be consumed.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.extract-input-language-fn]
> NETptr XfsmTransducer::extract_input_language(NETptr t)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.extract-input-language-fn]
> Returns the input (upper) projection of net `t` as a new net. Calls
> `upper_side_net(t, DONT_KEEP)`; `DONT_KEEP` means `t` may be consumed. The
> result is an acceptor over `t`'s input side.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.extract-output-language-fn]
> NETptr XfsmTransducer::extract_output_language(NETptr t)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.extract-output-language-fn]
> Returns the output (lower) projection of net `t` as a new net. Calls
> `lower_side_net(t, DONT_KEEP)`; `DONT_KEEP` means `t` may be consumed. The
> result is an acceptor over `t`'s output side.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.get-alphabet-fn]
> StringSet XfsmTransducer::get_alphabet(const NETptr t)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.get-alphabet-fn]
> Returns the set of HFST symbols in net `t`'s alphabet (sigma). Steps:
> (1) create an empty `StringSet retval`. (2) get the alphabet pointer
> `alpha_ptr = net_sigma(const_cast<NETptr>(t))`. (3) start an alphabet iterator
> `start_alph_iterator(NULL, alpha_ptr)`. (4) loop: fetch each id with
> `next_alph_id(it)`; while the id is not the sentinel `ID_NO_SYMBOL`, convert it
> to an HFST symbol via `XfsmTransducer::xfsm_symbol_to_hfst_symbol(label_id)`,
> insert it into `retval`, and fetch the next id. (5) return `retval`. Each
> alphabet id is a single-side symbol, so this yields the one-sided symbol set.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.hfst-symbol-to-xfsm-symbol-fn]
> id_type XfsmTransducer::hfst_symbol_to_xfsm_symbol(const std::string & symbol)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.hfst-symbol-to-xfsm-symbol-fn]
> Converts a one-side HFST symbol string to its XFSM id. Branches: (1) if
> `symbol == hfst::internal_epsilon`, return `EPSILON`. (2) else if `symbol ==
> hfst::internal_unknown`, return `OTHER`. (3) else if `symbol ==
> hfst::internal_identity`, throw the C-string `"hfst_symbol_to_xfsm_symbol does
> not accept the identity symbol as its argument"`. (4) otherwise return
> `single_to_id(symbol.c_str())` (the XFSM library's symbol-interning function).
> No mutation of arguments.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.initialize-xfsm-fn]
> void XfsmTransducer::initialize_xfsm()

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.initialize-xfsm-fn]
> Initializes the XFSM/cfsm library context once at startup. Steps: (1) call
> `initialize_cfsm()` to get the context `cntxt`. (2) set its character encoding
> to UTF-8 via `set_char_encoding(cntxt, CHAR_ENC_UTF_8)` (return value
> discarded). (3) set the global `IY_VERBOSE = 0` to suppress library messages.
> Returns void.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.insert-freely-fn]
> NETptr XfsmTransducer::insert_freely(NETptr t, NETptr ins)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.insert-freely-fn]
> Freely inserts net `ins` into net `t` and returns the result. Calls
> `ignore_net(t, const_cast<NETptr>(ins), DONT_KEEP, KEEP)`: `t` may be
> consumed/freed (`DONT_KEEP`), `ins` is preserved (`KEEP`). Returns the net that
> accepts `t`'s strings with `ins` freely interspersed.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.intersect-fn]
> NETptr XfsmTransducer::intersect(NETptr t1, const NETptr t2)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.intersect-fn]
> Returns the intersection of `t1` and `t2`. Calls `intersect_net(t1,
> const_cast<NETptr>(t2), DONT_KEEP, KEEP)`: `t1` may be consumed/freed
> (`DONT_KEEP`), `t2` is preserved (`KEEP`, const cast away). Returns the
> intersected net.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.invert-fn]
> NETptr XfsmTransducer::invert(NETptr t)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.invert-fn]
> Returns the inverse of net `t` (swaps input and output sides of every label).
> Calls `invert_net(t, DONT_KEEP)`; `DONT_KEEP` means `t` may be consumed.
> Returns the inverted net.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.is-cyclic-fn]
> bool XfsmTransducer::is_cyclic(NETptr t)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.is-cyclic-fn]
> Returns whether net `t` is cyclic (has unbounded path length). Calls the XFSM
> library functions `test_upper_bounded(t)` and `test_lower_bounded(t)`. If
> either returns a value other than 1 (i.e. the upper side OR the lower side is
> not bounded), returns true (cyclic). Otherwise (both sides bounded) returns
> false. No mutation of `t`.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.label-id-to-symbol-pair-fn]
> void XfsmTransducer::label_id_to_symbol_pair(id_type label_id, std::string & isymbol, std::string & osymbol)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.label-id-to-symbol-pair-fn]
> Converts an XFSM label id (a transition symbol pair) into the HFST input and
> output symbol strings, writing them into the out-parameters `isymbol` and
> `osymbol`. Branch: (1) if `label_id == OTHER` (the atomic OTHER label), set
> both `isymbol` and `osymbol` to `hfst::internal_identity` (the
> identity:identity pair). (2) Otherwise, split the label into its two sides via
> `upper_id(label_id)` and `lower_id(label_id)`, then set `isymbol =
> xfsm_symbol_to_hfst_symbol(upperid)` and `osymbol =
> xfsm_symbol_to_hfst_symbol(lowerid)`. (Note: a non-atomic OTHER:OTHER pair
> therefore maps to internal_unknown:internal_unknown via that conversion.)
> Returns void; mutates the two string references.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.minimize-fn]
> NETptr XfsmTransducer::minimize(NETptr t)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.minimize-fn]
> Minimizes net `t` in place and returns it. Steps: (1) if the static flag
> `minimize_even_if_already_minimal_` is true, clear the net's "already
> minimized" mark by setting `NET_minimized(t) = 0`, forcing a re-minimization.
> (2) Call `minimize_net(t)`; if it returns 1 (error), throw `HfstFatalException`
> with message `"XfsmTransducer::minimize"`. (3) Return `t` (same pointer,
> mutated in place).

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.number-of-arcs-fn]
> unsigned int XfsmTransducer::number_of_arcs(NETptr t)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.number-of-arcs-fn]
> Returns the number of arcs (transitions) in net `t`, as `(unsigned int)
> NET_num_arcs(t)`. No mutation, no side effects.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.number-of-states-fn]
> unsigned int XfsmTransducer::number_of_states(NETptr t)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.number-of-states-fn]
> Returns the number of states in net `t`, as `(unsigned int) NET_num_states(t)`.
> No mutation, no side effects.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.optionalize-fn]
> NETptr XfsmTransducer::optionalize(NETptr t)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.optionalize-fn]
> Makes net `t` optional (adds the empty string to its language) and returns the
> result. Calls `optional_net(t, DONT_KEEP)`; `DONT_KEEP` means `t` may be
> consumed. Returns the optionalised net.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.prolog-file-to-xfsm-transducer-fn]
> NETptr XfsmTransducer::prolog_file_to_xfsm_transducer(const char * filename)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.prolog-file-to-xfsm-transducer-fn]
> Reads a single transducer from a Prolog-format file and returns it. Steps:
> (1) `strdup` `filename` into a mutable C string `f`. (2) Call `read_prolog(f)`,
> storing the result. (3) If the result is NULL, throw `HfstFatalException` with
> message `"XfsmTransducer::prolog_file_to_xfsm_transducer"`. (4) `free(f)`.
> (5) Return the net. Note: on the throw path `f` is leaked since `free` is not
> reached.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.remove-symbols-from-alphabet-fn]
> void XfsmTransducer::remove_symbols_from_alphabet(NETptr t, const StringSet & symbols)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.remove-symbols-from-alphabet-fn]
> Removes a set of HFST symbols from the transducer's alphabet (sigma). Steps:
> (1) get `ap = net_sigma(t)`. (2) Iterate over every symbol in `symbols`; for
> each, if it is epsilon, unknown, or identity (`hfst::is_epsilon` /
> `is_unknown` / `is_identity`), skip it; otherwise convert via
> `XfsmTransducer::hfst_symbol_to_xfsm_symbol` and remove it with
> `alph_remove_from(ap, id, DONT_KEEP)`. Returns void; mutates `t`'s sigma.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.repeat-le-n-fn]
> NETptr XfsmTransducer::repeat_le_n(NETptr t, unsigned int n)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.repeat-le-n-fn]
> Returns net `t` repeated between 0 and `n` times inclusive (i.e. at most `n`
> repetitions). Calls `repeat_net(t, 0, n, DONT_KEEP)` (min 0, max `n`);
> `DONT_KEEP` means `t` may be consumed. Returns the resulting net.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.repeat-n-fn]
> NETptr XfsmTransducer::repeat_n(NETptr t, unsigned int n)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.repeat-n-fn]
> Returns net `t` repeated exactly `n` times. Calls `repeat_net(t, n, n,
> DONT_KEEP)` (min `n`, max `n`); `DONT_KEEP` means `t` may be consumed. Returns
> the resulting net.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.repeat-n-plus-fn]
> NETptr XfsmTransducer::repeat_n_plus(NETptr t, unsigned int n)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.repeat-n-plus-fn]
> Returns net `t` repeated `n` or more times (at least `n`, unbounded above).
> Calls `repeat_net(t, n, -1, DONT_KEEP)` (min `n`, max -1 meaning unbounded);
> `DONT_KEEP` means `t` may be consumed. Returns the resulting net.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.repeat-n-to-k-fn]
> NETptr XfsmTransducer::repeat_n_to_k(NETptr t, unsigned int n, unsigned int k)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.repeat-n-to-k-fn]
> Returns net `t` repeated between `n` and `k` times inclusive. Calls
> `repeat_net(t, n, k, DONT_KEEP)` (min `n`, max `k`); `DONT_KEEP` means `t` may
> be consumed. Returns the resulting net.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.repeat-plus-fn]
> NETptr XfsmTransducer::repeat_plus(NETptr t)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.repeat-plus-fn]
> Returns net `t` repeated one or more times (Kleene plus). Calls `repeat_net(t,
> 1, -1, DONT_KEEP)` (min 1, max -1 meaning unbounded); `DONT_KEEP` means `t` may
> be consumed. Returns the resulting net.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.repeat-star-fn]
> NETptr XfsmTransducer::repeat_star(NETptr t)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.repeat-star-fn]
> Returns net `t` repeated zero or more times (Kleene star). Calls `repeat_net(t,
> 0, -1, DONT_KEEP)` (min 0, max -1 meaning unbounded); `DONT_KEEP` means `t` may
> be consumed. Returns the resulting net.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.reverse-fn]
> NETptr XfsmTransducer::reverse(NETptr t)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.reverse-fn]
> Returns the reversal of net `t` (accepts the reversed strings). Calls
> `reverse_net(t, DONT_KEEP)`; `DONT_KEEP` means `t` may be consumed. Returns the
> reversed net.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.set-compose-flag-as-special-fn]
> void XfsmTransducer::set_compose_flag_as_special(bool value)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.set-compose-flag-as-special-fn]
> Sets the XFSM library's "compose flag as special" setting on the default cfsm
> context. Steps: (1) obtain the context `fst_cntxt = get_default_cfsm_context()`.
> (2) set `fst_cntxt->interface->general.compose_flag_as_special` to 1 if `value`
> is true, else 0. Returns void; mutates global library context state.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.set-minimize-even-if-already-minimal-fn]
> void XfsmTransducer::set_minimize_even_if_already_minimal(bool value)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.set-minimize-even-if-already-minimal-fn]
> Sets the static class flag `minimize_even_if_already_minimal_` to `value`.
> This flag controls whether `minimize` forces re-minimization of nets already
> marked minimal. Returns void; mutates class-level static state.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.subtract-fn]
> NETptr XfsmTransducer::subtract(NETptr t1, const NETptr t2)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.subtract-fn]
> Returns the difference `t1` minus `t2` (strings in `t1` not in `t2`). Calls
> `minus_net(t1, const_cast<NETptr>(t2), DONT_KEEP, KEEP)`: `t1` may be
> consumed/freed (`DONT_KEEP`), `t2` is preserved (`KEEP`, const cast away).
> Returns the difference net.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.symbol-pair-to-label-id-fn]
> id_type XfsmTransducer::symbol_pair_to_label_id(const std::string & isymbol, const std::string & osymbol)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.symbol-pair-to-label-id-fn]
> Converts an HFST input:output symbol pair into the corresponding XFSM label id.
> Branch: (1) if `isymbol == hfst::internal_identity`: require `osymbol` to also
> be `internal_identity`, otherwise throw the C-string `"identity symbol cannot
> be on one side only"`; if both are identity, return the atomic `OTHER` label.
> (2) Otherwise convert each side independently via
> `hfst_symbol_to_xfsm_symbol(isymbol)` -> `input_id` and
> `hfst_symbol_to_xfsm_symbol(osymbol)` -> `output_id`, then return
> `id_pair_to_id(input_id, output_id)` (the paired label). No mutation of
> arguments.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.write-in-att-format-fn]
> void XfsmTransducer::write_in_att_format(NETptr t, const char * filename)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.write-in-att-format-fn]
> Not implemented: unconditionally throws `HfstFatalException` with message
> `"XfsmTransducer::write_in_att_format"`. The parameters `t` and `filename` are
> ignored; the function never writes anything. A Rust port should reproduce this
> as an always-failing operation.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.write-in-prolog-format-fn]
> void XfsmTransducer::write_in_prolog_format(NETptr t, const char * filename)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.write-in-prolog-format-fn]
> Writes net `t` to a Prolog-format file named `filename`. Steps: (1) `strdup`
> `filename` into a mutable C string `f`. (2) Call `write_prolog(t, f)`; if it
> returns a non-zero value (error), throw `HfstFatalException` with message
> `"XfsmTransducer::write_in_prolog_format"`. (3) `free(f)`. Returns void. Note:
> on the throw path `f` is leaked since `free` is not reached.

> [spec:hfst:def:xfsm-transducer.hfst.implementations.xfsm-transducer.xfsm-symbol-to-hfst-symbol-fn]
> std::string XfsmTransducer::xfsm_symbol_to_hfst_symbol(id_type id)

> [spec:hfst:sem:xfsm-transducer.hfst.implementations.xfsm-transducer.xfsm-symbol-to-hfst-symbol-fn]
> Converts a one-side XFSM symbol id to its HFST symbol string. Branches: (1) if
> `id == EPSILON`, return `hfst::internal_epsilon`. (2) else if `id == OTHER`,
> return `hfst::internal_unknown`. (3) otherwise look up the label via
> `id_to_label(id)`, take its name `lptr->content.name` (a FAT_STR, a
> null-terminated character buffer), and build a `std::string` by appending each
> character until the `'\0'` terminator; return that string. No mutation of
> arguments.

> [spec:hfst:def:xfsm-transducer.main-fn]
> int main(int argc, char * argv[])

> [spec:hfst:sem:xfsm-transducer.main-fn]
> Unit-test entry point, compiled only when `MAIN_TEST` is defined. Prints
> `"Unit tests for " __FILE__ ":"` to stdout, then a newline followed by `"ok"`
> and another newline, and returns `EXIT_SUCCESS`. It performs no actual tests
> (no assertions); it is a placeholder that always succeeds. Ignores `argc`/
> `argv`.

