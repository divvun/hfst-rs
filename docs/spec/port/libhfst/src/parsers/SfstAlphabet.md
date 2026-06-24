# libhfst/src/parsers/SfstAlphabet.cc, libhfst/src/parsers/SfstAlphabet.h

> [spec:hfst:def:sfst-alphabet.hfst.implementations.sfst-alphabet]
> class SfstAlphabet {
>   struct eqstr { // [spec:hfst:def:sfst-alphabet.hfst.implementations.sfst-alphabet.eqstr.operator-fn] // [spec:hfst:sem:sfst-alphabet.hfst.implementations.sfs...;
>   SymbolMap sm;
>   CharMap cm;
>   NumberPairSet pairs;
> }

> [spec:hfst:def:sfst-alphabet.hfst.implementations.sfst-alphabet.add-fn]
> void SfstAlphabet::add( const char *symbol, unsigned int c )

> [spec:hfst:sem:sfst-alphabet.hfst.implementations.sfst-alphabet.add-fn]
> Registers a symbol/code mapping in both directions. Duplicates the input
> C-string `symbol` via `sfst_basic::fst_strdup` (heap allocation, owned copy
> `s`). Stores `cm[c] = s` (code -> char* in CharMap) and `sm[s] = c` (char*
> -> code in SymbolMap), both keyed/valued by the SAME owned pointer `s`. No
> check for prior existence: if `c` was already mapped, its CharMap entry is
> overwritten (the previous owned pointer is leaked here, since only the
> destructor frees current cm values). Returns nothing.

> [spec:hfst:def:sfst-alphabet.hfst.implementations.sfst-alphabet.add-symbol-fn]
> void SfstAlphabet::add_symbol( const char *symbol, unsigned int c )

> [spec:hfst:sem:sfst-alphabet.hfst.implementations.sfst-alphabet.add-symbol-fn]
> Inserts a symbol with an explicitly chosen code `c`, validating consistency.
> First calls `symbol2code(symbol)` -> `sc`. If `sc != EOF` (symbol already
> known): if `(unsigned int)sc == c`, return immediately (already correctly
> defined); otherwise throw an error. The error message is built into a static
> `char[100]` via `sprintf` ("Error: reinserting symbol '%s' in alphabet with
> incompatible character value %u %u", symbol, sc, c) when `strlen(symbol) <
> 60`, else a fixed literal string "reinserting symbol in alphabet with
> incompatible character value"; the thrown value is a `const char*`. Next,
> check whether the code is in use via `code2symbol(c)` -> `s`. If `s == NULL`
> (code free), call `add(symbol, c)`. Otherwise if `strcmp(s, symbol) != 0`
> (code already bound to a different symbol), throw an error: into static
> `char[100]` ("Error: defining symbol %s as character %d (previously defined
> as %s)", symbol, c, s) when `strlen(symbol) < 70`, else the literal "Error:
> defining a (very long) symbol with previously used character". If `s` equals
> `symbol` it does nothing. Returns void.

> [spec:hfst:def:sfst-alphabet.hfst.implementations.sfst-alphabet.begin-fn]
> SfstAlphabet::const_iterator SfstAlphabet::begin() const

> [spec:hfst:sem:sfst-alphabet.hfst.implementations.sfst-alphabet.begin-fn]
> Returns `pairs.begin()`, a const_iterator over the NumberPairSet of
> code-pair transitions. Trivial accessor; no state mutated.

> [spec:hfst:def:sfst-alphabet.hfst.implementations.sfst-alphabet.char-map]
> typedef unordered_map<unsigned int, char*> CharMap

> [spec:hfst:def:sfst-alphabet.hfst.implementations.sfst-alphabet.clear-pairs-fn]
> void SfstAlphabet::clear_pairs()

> [spec:hfst:sem:sfst-alphabet.hfst.implementations.sfst-alphabet.clear-pairs-fn]
> Calls `pairs.clear()`, removing all entries from the NumberPairSet. The
> symbol/code maps (sm, cm) are left untouched. Returns void.

> [spec:hfst:def:sfst-alphabet.hfst.implementations.sfst-alphabet.code2symbol-fn]
> const char *SfstAlphabet::code2symbol( unsigned int c ) const

> [spec:hfst:sem:sfst-alphabet.hfst.implementations.sfst-alphabet.code2symbol-fn]
> Looks up code `c` in the CharMap `cm` via `cm.find(c)`. If not found
> (iterator equals `cm.end()`), returns NULL. Otherwise returns the stored
> `char*` symbol pointer (still owned by the alphabet; caller must not free).
> Const method, no mutation.

> [spec:hfst:def:sfst-alphabet.hfst.implementations.sfst-alphabet.complement-fn]
> void SfstAlphabet::complement( std::vector<unsigned int> &sym )

> [spec:hfst:sem:sfst-alphabet.hfst.implementations.sfst-alphabet.complement-fn]
> Replaces `sym` (in/out vector of codes) with the set-complement of `sym`
> within the alphabet's known non-special codes. Builds a local `result`
> vector. Iterates every entry in CharMap `cm`; for each code `c = it->first`,
> skips the special symbols 0, 1, 2 (epsilon, unknown, identity). For other
> codes, does a linear scan over `sym` looking for `c`; if `c` is NOT present
> in `sym` (the scan reaches `sym.size()` without a match), pushes `c` onto
> `result`. Finally `sym.swap(result)` so `sym` now holds every non-special
> alphabet code that was absent from the original `sym`. Order follows the
> CharMap iteration order. Returns void.

> [spec:hfst:def:sfst-alphabet.hfst.implementations.sfst-alphabet.const-iterator]
> typedef NumberPairSet::const_iterator const_iterator

> [spec:hfst:def:sfst-alphabet.hfst.implementations.sfst-alphabet.end-fn]
> SfstAlphabet::const_iterator SfstAlphabet::end() const

> [spec:hfst:sem:sfst-alphabet.hfst.implementations.sfst-alphabet.end-fn]
> Returns `pairs.end()`, the past-the-end const_iterator of the NumberPairSet.
> Trivial accessor; no state mutated.

> [spec:hfst:def:sfst-alphabet.hfst.implementations.sfst-alphabet.eqstr]
> struct eqstr

> [spec:hfst:def:sfst-alphabet.hfst.implementations.sfst-alphabet.eqstr.operator-fn]
> bool operator()(const char* s1, const char* s2) const

> [spec:hfst:sem:sfst-alphabet.hfst.implementations.sfst-alphabet.eqstr.operator-fn]
> Equality functor for C-strings used as the key-equality predicate of the
> SymbolMap hash map. Returns `strcmp(s1, s2) == 0`, i.e. true iff the two
> NUL-terminated strings are byte-for-byte equal. Pure, no side effects.

> [spec:hfst:def:sfst-alphabet.hfst.implementations.sfst-alphabet.get-char-map-fn]
> SfstAlphabet::CharMap SfstAlphabet::get_char_map()

> [spec:hfst:sem:sfst-alphabet.hfst.implementations.sfst-alphabet.get-char-map-fn]
> Returns a COPY of the CharMap `cm` (by value). The copy shares the same
> `char*` pointers as the original (shallow copy of the map; pointer values
> duplicated, not the underlying strings). No mutation of `this`.

> [spec:hfst:def:sfst-alphabet.hfst.implementations.sfst-alphabet.insert-fn]
> void SfstAlphabet::insert(NumberPair sp)

> [spec:hfst:sem:sfst-alphabet.hfst.implementations.sfst-alphabet.insert-fn]
> Inserts the code-pair `sp` (a NumberPair, i.e. pair of unsigned ints) into
> the NumberPairSet `pairs` via `pairs.insert(sp)`. Since `pairs` is a set,
> duplicate pairs are ignored. The comment "/* check special symbols */" marks
> intended validation that is not actually performed. Returns void.

> [spec:hfst:def:sfst-alphabet.hfst.implementations.sfst-alphabet.next-code-fn]
> int SfstAlphabet::next_code( char* &string, bool extended, bool insert )

> [spec:hfst:sem:sfst-alphabet.hfst.implementations.sfst-alphabet.next-code-fn]
> Parses and consumes the next character code from the cursor `string` (passed
> by reference; advanced in place). Parameters: `extended` enables backslash
> quoting, `insert` controls multi-char-symbol insertion (defaults to true at
> the declaration; passed through to `next_mcsym`). Steps: 1) If `*string ==
> 0` (end of string), return EOF. 2) Try `next_mcsym(string, insert)`; if it
> returns a value `!= EOF`, return that code (cursor already advanced past the
> `<...>` symbol). 3) If `extended` and the current char is a backslash `\`,
> advance `string` by one to strip the quote. 4) Decode one UTF-8 character:
> `sfst_utf8::utf8toint(&string)` advances the cursor over the UTF-8 sequence
> and yields a codepoint `c`; convert it back to a canonical UTF-8 byte string
> with `sfst_utf8::int2utf8(c)`, register it via `add_symbol(...)`, and return
> that symbol's assigned code. (Non-UTF-8 single-byte branch is commented
> out.) Mutates the alphabet by allocating new symbols as needed.

> [spec:hfst:def:sfst-alphabet.hfst.implementations.sfst-alphabet.next-label-fn]
> std::pair<unsigned int, unsigned int> SfstAlphabet::next_label(char * &string, bool extended)

> [spec:hfst:sem:sfst-alphabet.hfst.implementations.sfst-alphabet.next-label-fn]
> Parses the next transition label (input:output code pair) from cursor
> `string` (by reference, advanced in place). `extended` enables the
> `input:output` two-sided syntax and backslash quoting. Steps: 1) Read first
> code `c = next_code(string, extended)`. If `c == EOF`, return the pair
> (0,0) signalling end of string. 2) Let `lc = (unsigned)c`. If NOT `extended`
> OR the current char is not ':', treat as a single (identity) character: if
> `lc == 0` (epsilon), recurse `next_label(string, extended)` to skip it;
> otherwise return the pair (lc, lc). 3) Otherwise (extended and `*string ==
> ':'`): advance past the ':' and read the second code `c = next_code(string)`
> (note: called with default arguments, so `extended` defaults — second side
> parsed without the extended/insert flags being explicitly forwarded). If
> this `c == EOF`, throw a C-string error formatted via `sprintf` into a
> static `char[1000]` ("Error: incomplete symbol in input file: %s", string).
> 4) Form `retval = (lc, (unsigned)c)`; if both members are 0 (epsilon:epsilon),
> recurse to skip it; otherwise return `retval`. Recursion can mutate the
> alphabet (via next_code) and throws on malformed input.

> [spec:hfst:def:sfst-alphabet.hfst.implementations.sfst-alphabet.next-mcsym-fn]
> int SfstAlphabet::next_mcsym( char* &string, bool insert )

> [spec:hfst:sem:sfst-alphabet.hfst.implementations.sfst-alphabet.next-mcsym-fn]
> Recognizes a multi-character symbol delimited by angle brackets `<...>` at
> the start of cursor `string` (by reference). `insert`: if true, an unknown
> symbol is added to the alphabet; if false, only an already-known symbol is
> accepted. Steps: let `start = string`. If `*start != '<'`, fall through and
> return EOF. Otherwise scan forward from `start+1` for the first `>`. On
> finding it: set `end` to one past the `>`, save the byte at that position
> (`lastc`), temporarily NUL-terminate the substring there so `start` is now
> the C-string "<...>". Then: if the substring equals "<>" exactly, code `c =
> 0` (epsilon). Else if `insert`, `c = add_symbol(start)` (assigns/reuses a
> code); else `c = symbol2code(start)`. Restore the overwritten byte (`*end =
> lastc`). If `c != EOF`, advance `string = end` (past the closing bracket)
> and return `(unsigned int)c`. If `c == EOF` (not a known symbol with insert
> false), break out of the scan. If no `>` is found, or on break, return EOF
> without advancing `string`. Note `add_symbol(const char*)` here is the
> single-argument auto-assigning overload, which may throw "too many symbols".

> [spec:hfst:def:sfst-alphabet.hfst.implementations.sfst-alphabet.number-pair]
> typedef std::pair<unsigned int,unsigned int> NumberPair

> [spec:hfst:def:sfst-alphabet.hfst.implementations.sfst-alphabet.number-pair-set]
> typedef std::set<NumberPair> NumberPairSet

> [spec:hfst:def:sfst-alphabet.hfst.implementations.sfst-alphabet.print-fn]
> void SfstAlphabet::print()

> [spec:hfst:sem:sfst-alphabet.hfst.implementations.sfst-alphabet.print-fn]
> Debug-prints the CharMap to stdout. Writes the literal line "alphabet..\n",
> then for every entry of `cm` (in CharMap iteration order) prints "%i\t%s\n"
> with the code and its symbol string, then writes "..alphabet\n". Uses
> `printf`. No state mutated; returns void.

> [spec:hfst:def:sfst-alphabet.hfst.implementations.sfst-alphabet.print-pairs-fn]
> void SfstAlphabet::print_pairs(FILE *file)

> [spec:hfst:sem:sfst-alphabet.hfst.implementations.sfst-alphabet.print-pairs-fn]
> Writes every transition pair in `pairs` to the given `FILE *file`. For each
> NumberPair in `pairs` (set iteration order), prints "%s:%s\n" where the two
> fields are `code2symbol(it->first)` and `code2symbol(it->second)` (the symbol
> strings looked up from the codes; these may be NULL if a code is absent from
> `cm`). Uses `fprintf`. No state mutated; returns void.

> [spec:hfst:def:sfst-alphabet.hfst.implementations.sfst-alphabet.sfst-alphabet-fn]
> SfstAlphabet::~SfstAlphabet()

> [spec:hfst:sem:sfst-alphabet.hfst.implementations.sfst-alphabet.sfst-alphabet-fn]
> Destructor. Frees all heap-allocated symbol strings owned via the CharMap.
> Steps: allocate a temporary `char**` array `s` of size `cm.size()`. Clear
> `pairs` and `sm` (these do not own the strings). Iterate `cm`, copying each
> value pointer into `s[n++]`, then clear `cm`. Then for each of the `n`
> collected pointers, call `free(...)` (matching the `fst_strdup`/malloc
> allocation used in `add`). Finally `delete[] s`. Each string is freed exactly
> once even though it was referenced by both maps, because sm/cm shared the
> same pointer and only the cm copies are freed.

> [spec:hfst:def:sfst-alphabet.hfst.implementations.sfst-alphabet.size-fn]
> size_t SfstAlphabet::size() const

> [spec:hfst:sem:sfst-alphabet.hfst.implementations.sfst-alphabet.size-fn]
> Returns `pairs.size()`, the number of code-pair transitions in the
> NumberPairSet. Trivial const accessor; no mutation.

> [spec:hfst:def:sfst-alphabet.hfst.implementations.sfst-alphabet.symbol-map]
> typedef unordered_map<const char*, unsigned int> SymbolMap

> [spec:hfst:def:sfst-alphabet.hfst.implementations.sfst-alphabet.symbol2code-fn]
> int SfstAlphabet::symbol2code( const char * s ) const

> [spec:hfst:sem:sfst-alphabet.hfst.implementations.sfst-alphabet.symbol2code-fn]
> Looks up symbol string `s` in the SymbolMap `sm` via `sm.find(s)` (using the
> `eqstr` strcmp equality and the map's hash on the C-string contents). If
> found, returns the associated code `p->second` (as int). If not found,
> returns `EOF` (-1). Const method, no mutation.

> [spec:hfst:def:sfst-alphabet.main-fn]
> int

> [spec:hfst:sem:sfst-alphabet.main-fn]
> Standalone unit-test entry point, compiled only when `DEBUG_MAIN` is defined.
> Prints progress banners to `std::cout`. Exercises: the default constructor
> (`SfstAlphabet defaultAlpha`), the copy constructor (`SfstAlphabet
> copyAlpha(defaultAlpha)`), and the destructor via `delete new SfstAlphabet()`
> and `delete new SfstAlphabet(defaultAlpha)`. Prints "rest skipped..." and
> "ok", then returns `EXIT_SUCCESS`. Performs no assertions; it merely confirms
> these operations run without crashing.

