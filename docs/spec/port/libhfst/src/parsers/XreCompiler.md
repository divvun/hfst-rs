# libhfst/src/parsers/XreCompiler.cc, libhfst/src/parsers/XreCompiler.h

> [spec:hfst:def:xre-compiler.hfst.xre.defined-multichar-symbols-fn]
> std::set<std::string> * defined_multichar_symbols_(NULL)

> [spec:hfst:sem:xre-compiler.hfst.xre.defined-multichar-symbols-fn]
> File-scope (within namespace `hfst::xre`) mutable global pointer
> `defined_multichar_symbols_`, of type `std::set<std::string>*`,
> initialized to NULL. It holds the optional set of user-declared
> multichar symbols; NULL means no such set has been created yet. It is
> lazily allocated by `add_defined_multichar_symbol` and freed/reset to
> NULL by `remove_defined_multichar_symbols`.

> [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler]
> class XreCompiler {
>   XreCompiler& setOutputToConsole(bool value);
>   std::map<std::string,hfst::HfstTransducer*> definitions_;
>   std::map<std::string, std::string> function_definitions_;
>   std::map<std::string, unsigned int > function_arguments_;
>   std::map<std::string, std::set<std::string> > list_definitions_;
>   hfst::ImplementationType format_;
>   bool verbose_;
> }

> [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.add-defined-multichar-symbol-fn]
> void

> [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.add-defined-multichar-symbol-fn]
> `add_defined_multichar_symbol(const std::string & symbol)` returns void.
> If the global pointer `defined_multichar_symbols_` is NULL, allocate a
> new empty `std::set<std::string>` and assign it to that pointer. Then
> insert `symbol` into the set (duplicates ignored by set semantics).

> [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.compile-first-fn]
> HfstTransducer*

> [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.compile-first-fn]
> `compile_first(const std::string& xre, unsigned int & chars_read)`
> returns `HfstTransducer*`. Compiles only the first regex in `xre` and
> reports how many characters were consumed. Steps: save the current
> value of the global counter `cr` into a local `cr_before`, then set the
> global `cr` to 0. In a try block, call
> `hfst::xre::compile_first(xre, definitions_, function_definitions_,
> function_arguments_, list_definitions_, format_, chars_read)` (passing
> the member maps and `format_`; `chars_read` is filled by the callee with
> the number of characters consumed). Restore `cr = cr_before` and return
> the resulting transducer pointer. If a `const char *` exception (`msg`)
> is thrown: if `msg` equals "Allocation of memory failed in Mem::add_buffer!"
> throw `HfstException` with message "Allocation of memory failed in SFST
> backend."; otherwise throw `HfstException` with message `msg`.

> [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.compile-fn]
> HfstTransducer*

> [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.compile-fn]
> `compile(const std::string& xre)` returns `HfstTransducer*`. Save the
> current value of the global counter `cr` into a local `cr_before`, then
> set the global `cr` to 0. In a try block, call
> `hfst::xre::compile(xre, definitions_, function_definitions_,
> function_arguments_, list_definitions_, format_)` (passing the member
> maps and `format_`), restore `cr = cr_before`, and return the resulting
> transducer pointer (which may be NULL on non-fatal parse error). If a
> `const char *` exception (`msg`) is thrown: if `msg` equals "Allocation
> of memory failed in Mem::add_buffer!" throw `HfstException` with message
> "Allocation of memory failed in SFST backend."; otherwise throw
> `HfstException` with message `msg`.

> [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.contained-only-comments-fn]
> bool

> [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.contained-only-comments-fn]
> `contained_only_comments()` returns bool: returns the value of the
> global flag `contains_only_comments` (set by the parser to indicate the
> last compiled input held nothing but comments).

> [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.define-fn]
> bool

> [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.define-fn]
> `define(const std::string& name, const std::string& xre)` returns bool.
> Call `compile(xre)` to get a `HfstTransducer* compiled`. If `compiled`
> is NULL (parse failed): if the member `verbose_` is true, obtain an
> error stream via `get_stream(get_error_stream())`, write
> "error: could not parse '<xre>', leaving '<name>' undefined" followed by
> a newline to it, call `flush` on that stream, then return false. If
> compilation succeeded, call `this->undefine(name)` (deleting and erasing
> any prior definition of `name`), store `definitions_[name] = compiled`
> (taking ownership of the pointer), and return true.

> [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.define-function-fn]
> bool

> [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.define-function-fn]
> `define_function(const std::string& name, unsigned int arguments,
> const std::string& xre)` returns bool. Store `function_arguments_[name]
> = arguments` (the arity) and `function_definitions_[name] = xre` (the
> raw regex body, not compiled here), overwriting any existing entries.
> Always returns true.

> [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.define-list-fn]
> void

> [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.define-list-fn]
> `define_list(const std::string& name, const std::set<std::string>&
> symbol_list)` returns void: stores `list_definitions_[name] =
> symbol_list`, overwriting any existing list under that name.

> [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.flush-fn]
> void XreCompiler::flush(std::ostream * oss)

> [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.flush-fn]
> `flush(std::ostream * oss)` returns void. On non-Windows builds it does
> nothing (entire body is guarded by `#ifdef WINDOWS`). On Windows: if the
> global `output_to_console_` is true and `oss` is the redirect buffer
> `&hfst::xre::winoss_`, then if the global `redirected_stream_` points at
> `std::cerr` write the buffer's accumulated string to the console via
> `hfst_fprintf_console(stderr, ...)`, else if it points at `std::cout`
> write via `hfst_fprintf_console(stdout, ...)`, else do nothing; then
> reset `redirected_stream_` to NULL and clear `winoss_` (set its string
> to empty).

> [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.get-error-stream-fn]
> std::ostream * XreCompiler::get_error_stream()

> [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.get-error-stream-fn]
> `get_error_stream()` returns `std::ostream*`: returns the global error
> stream pointer `hfst::xre::error_` (defaults to `&std::cerr`).

> [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.get-output-to-console-fn]
> bool

> [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.get-output-to-console-fn]
> `getOutputToConsole()` returns bool. On Windows builds it returns the
> member `output_to_console_`; on all other platforms it returns false.
> (Note: the Windows branch is gated on a misspelled `WINDOWs` macro, so
> in practice it returns false everywhere unless that macro is defined.)

> [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.get-positions-of-symbol-in-xre-fn]
> bool XreCompiler::get_positions_of_symbol_in_xre

> [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.get-positions-of-symbol-in-xre-fn]
> `get_positions_of_symbol_in_xre(const std::string & symbol, const
> std::string & xre, std::set<unsigned int> & positions_)` returns bool.
> Steps: set the global `position_symbol = strdup(symbol.c_str())` (a
> heap-allocated C copy of `symbol`); clear the global set `positions`;
> save the global `cr` into `cr_before` and set `cr = 0`. Call
> `hfst::xre::compile(xre, definitions_, function_definitions_,
> function_arguments_, list_definitions_, format_)` to compile, during
> which the parser records into the global `positions` set the character
> positions where `position_symbol` occurred. `free(position_symbol)` and
> set it to NULL. If the compiled transducer pointer is NULL, return false
> (positions not reported). Otherwise delete the compiled transducer, copy
> the global `positions` into the output parameter `positions_`, restore
> `cr = cr_before`, and return true.

> [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.get-stream-fn]
> std::ostream * XreCompiler::get_stream(std::ostream * oss)

> [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.get-stream-fn]
> `get_stream(std::ostream * oss)` returns `std::ostream*`. On non-Windows
> builds it simply returns `oss` unchanged. On Windows: if the global
> `output_to_console_` is true and `oss` is either `&std::cerr` or
> `&std::cout`, record `oss` in the global `redirected_stream_` and return
> a pointer to the redirect string buffer `&hfst::xre::winoss_` (so output
> is captured for later console flushing); otherwise return `oss`.

> [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.get-verbosity-fn]
> bool XreCompiler::get_verbosity()

> [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.get-verbosity-fn]
> `get_verbosity()` returns bool: returns the member field `verbose_`.

> [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.is-definition-fn]
> bool

> [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.is-definition-fn]
> `is_definition(const std::string & name)` returns bool: returns true if
> `name` is a key in the `definitions_` map, false otherwise.

> [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.is-function-definition-fn]
> bool

> [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.is-function-definition-fn]
> `is_function_definition(const std::string & name)` returns bool: returns
> true if `name` is a key in the `function_definitions_` map, false
> otherwise.

> [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.remove-defined-multichar-symbols-fn]
> void

> [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.remove-defined-multichar-symbols-fn]
> `remove_defined_multichar_symbols()` returns void: if the global pointer
> `defined_multichar_symbols_` is not NULL, delete the pointed-to set and
> set the pointer back to NULL. No-op if already NULL.

> [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.set-error-stream-fn]
> void XreCompiler::set_error_stream(std::ostream * os)

> [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.set-error-stream-fn]
> `set_error_stream(std::ostream * os)` returns void: assigns the global
> error stream pointer `hfst::xre::error_ = os`.

> [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.set-expand-definitions-fn]
> void XreCompiler::set_expand_definitions(bool expand)

> [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.set-expand-definitions-fn]
> `set_expand_definitions(bool expand)` returns void: assigns the global
> flag `expand_definitions = expand`.

> [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.set-flag-harmonization-fn]
> void XreCompiler::set_flag_harmonization(bool harmonize_flags)

> [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.set-flag-harmonization-fn]
> `set_flag_harmonization(bool harmonize_flags)` returns void: assigns the
> global flag `harmonize_flags_ = harmonize_flags`.

> [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.set-harmonization-fn]
> void XreCompiler::set_harmonization(bool harmonize)

> [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.set-harmonization-fn]
> `set_harmonization(bool harmonize)` returns void: assigns the global
> flag `harmonize_ = harmonize`.

> [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.set-verbosity-fn]
> void XreCompiler::set_verbosity(bool verbose)

> [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.set-verbosity-fn]
> `set_verbosity(bool verbose)` returns void: assigns the member field
> `this->verbose_ = verbose` and also assigns the global `hfst::xre::verbose_
> = verbose`.

> [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.undefine-fn]
> void

> [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.undefine-fn]
> `undefine(const std::string& name)` returns void: look up `name` in the
> `definitions_` map; if found, delete the owned `HfstTransducer*` value
> and erase the entry from the map. No-op if not present.

> [spec:hfst:def:xre-compiler.hfst.xre.xre-compiler.xre-compiler-fn]
> XreCompiler::XreCompiler()

> [spec:hfst:sem:xre-compiler.hfst.xre.xre-compiler.xre-compiler-fn]
> Default constructor `XreCompiler()`. Default-constructs the four member
> maps (`definitions_`, `function_definitions_`, `function_arguments_`,
> `list_definitions_`) as empty, sets `format_` to
> `hfst::TROPICAL_OPENFST_TYPE`, sets `verbose_` to false, and on Windows
> builds also sets `output_to_console_` to false. Body is empty.

> [spec:hfst:def:xre-compiler.hfst.xre.xre-constructor-arguments]
> struct XreConstructorArguments {
>   std::map<std::string,hfst::HfstTransducer*> definitions;
>   std::map<std::string, std::string> function_definitions;
>   std::map<std::string, unsigned int > function_arguments;
>   std::map<std::string, std::set<std::string> > list_definitions;
>   hfst::ImplementationType format;
> }

> [spec:hfst:def:xre-compiler.hfst.xre.xre-constructor-arguments.xre-constructor-arguments-fn]
> XreConstructorArguments

> [spec:hfst:sem:xre-compiler.hfst.xre.xre-constructor-arguments.xre-constructor-arguments-fn]
> Constructor for `XreConstructorArguments` taking five by-value
> parameters: `definitions_` (map of name to `HfstTransducer*`),
> `function_definitions_` (map name to regex string), `function_arguments_`
> (map name to arity), `list_definitions_` (map name to set of strings),
> and `format_` (an `ImplementationType`). It copies each parameter into
> the correspondingly-named struct member (`definitions`,
> `function_definitions`, `function_arguments`, `list_definitions`,
> `format`).

> [spec:hfst:def:xre-compiler.main-fn]
> int

> [spec:hfst:sem:xre-compiler.main-fn]
> `main(int, char**)` is the unit-test entry point compiled only when
> `UNIT_TEST` is defined. Steps: print a banner naming the file, then test
> constructors by printing progress and constructing an `XreCompiler` per
> available backend (SFST, OpenFst, Foma, each gated by its `HAVE_*`
> macro) plus a default one. Build several `HfstBasicTransducer` fixtures
> by hand: `basicCat` accepting "cat", `basicFight` accepting
> "f i:o 0:u g h t" (with substitutions and an epsilon-to-u arc),
> `basicCatOrDog` accepting "cat" or "dog", and `basicAaOrBc` accepting
> "aa" or "bc". Then for each available backend, compile regex strings
> ("c a t", "f i:o 0:u g h t", "c a t | dog", "a a | b c") via the
> backend's `compile`, asserting each result is non-null and `compare`s
> equal to the corresponding hand-built transducer converted to that
> backend type, deleting each compiled transducer afterward. Finally, for
> each available backend, call `define("vowels", "a | e | i | o | u | y")`.
> Print "ok." and return `EXIT_SUCCESS`.

## Complement compilation (REGEXP8/9/10 unary operators)

The `~` (complement) and `\` (term complement) unary regex operators are
compiled by the AST walker (`eval_unary`). Upstream C++ XRE built their
universe from the bare identity `[?:?]`, which means subtract's harmonization
erases any flag diacritics contained in the operand — flags are swallowed. The
port DIVERGES here to match the Xerox semantics of the issue and the already-
fixed `negate` command (`HfstTransducer::negate`, upstream commit cdab3f74):
flag diacritics are treated as ORDINARY symbols in both operators. Both share
the universe constructor `HfstTransducer::identity_with_flags_of(A)` =
`[? | flag1 | ... | flagN]`, which inserts A's flags into the identity universe
as plain single-symbol arcs so subtract cannot erase them.

> [spec:hfst:def:xre-compiler.hfst.xre.complement-compilation-fn]
> ~A (REGEXP8/9/10 complement)

> [spec:hfst:sem:xre-compiler.hfst.xre.complement-compilation-fn]
> Compiling `~A` (complement). A MUST be an automaton; otherwise raise the
> error "Complement operator ~ is defined only for automata". The result is
> `[? | flags(A)]* - A`: build the flag-ordinary single-symbol universe
> `HfstTransducer::identity_with_flags_of(A)` (identity `?` disjoined with each
> flag diacritic of A as an ordinary single-symbol arc), star it, optimize with
> the compiler's optimization config, subtract A (harmonize=true), then
> `prune_alphabet(false)`. Because the starred universe always uses the identity
> symbol in its transitions, `prune_alphabet(false)` is a no-op on the alphabet
> (it refuses to prune while identity/unknown symbols are in use), so A's flags
> survive in sigma as ordinary symbols. This DIVERGES from upstream C++ XRE
> (which compiled `[?:?]* - A`, swallowing flags): the divergence is deliberate
> (hfst/hfst#349) and makes `~A` agree with `HfstTransducer::negate()` — the
> issue's Xerox transcript (the 3-state/6-arc complement of a single flag) and
> the double-complement identity `~[~A] == A` both hold.

> [spec:hfst:def:xre-compiler.hfst.xre.term-complement-compilation-fn]
> \A (REGEXP8/9/10 term complement)

> [spec:hfst:sem:xre-compiler.hfst.xre.term-complement-compilation-fn]
> Compiling `\A` (term complement). The result is `[? | flags(A)] - A`: build
> the same flag-ordinary single-symbol universe
> `HfstTransducer::identity_with_flags_of(A)` used by `~A`, but WITHOUT starring
> it (a term complement matches exactly one symbol), subtract A (harmonize=true),
> then `prune_alphabet(false)` (a no-op on the alphabet because the universe uses
> the identity symbol). So `\A` accepts any single symbol other than A, with A's
> flag diacritics kept in sigma as ordinary symbols. This DIVERGES from upstream
> C++ XRE (which compiled `[?] - A`, swallowing flags): the divergence is
> deliberate (hfst/hfst#349) and matches the Xerox transcript — `\flag` does not
> accept the flag, does accept any other single symbol, and `[\flag | flag]`
> recovers the full flag-ordinary single-symbol universe `[? | flag]`.

