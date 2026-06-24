# libhfst/src/HfstTokenizer.cc, libhfst/src/HfstTokenizer.h

> [spec:hfst:def:hfst-tokenizer.hfst.hfst-one-level-path]
> typedef std::pair<float, StringVector> HfstOneLevelPath

> [spec:hfst:def:hfst-tokenizer.hfst.hfst-tokenizer]
> class HfstTokenizer {
>   MultiCharSymbolTrie multi_char_symbols;
>   StringSet skip_symbol_set;
>   HFSTDLL StringPairVector;
>   HFSTDLL StringVector;
>   HFSTDLL static StringPairVector;
>   HFSTDLL StringPairVector;
>   HFSTDLL StringPairVector;
>   HFSTDLL StringPairVector;
>   HFSTDLL static unsigned;
> }

> [spec:hfst:def:hfst-tokenizer.hfst.hfst-tokenizer.add-multichar-symbol-fn]
> void

> [spec:hfst:sem:hfst-tokenizer.hfst.hfst-tokenizer.add-multichar-symbol-fn]
> Registers `symbol` as a multi-character symbol. If `symbol` is empty,
> return immediately doing nothing. Otherwise, add the symbol's C-string
> (NUL-terminated bytes) to the tokenizer's `multi_char_symbols` trie via
> `MultiCharSymbolTrie::add`. No return value.

> [spec:hfst:def:hfst-tokenizer.hfst.hfst-tokenizer.add-skip-symbol-fn]
> void

> [spec:hfst:sem:hfst-tokenizer.hfst.hfst-tokenizer.add-skip-symbol-fn]
> Registers `symbol` as a skip symbol. If `symbol` is empty, return
> immediately doing nothing. Otherwise, first add the symbol's C-string to
> the `multi_char_symbols` trie via `MultiCharSymbolTrie::add` (so it is
> recognized as a unit during tokenization), then insert the symbol's
> C-string into `skip_symbol_set`. No return value.

> [spec:hfst:def:hfst-tokenizer.hfst.hfst-tokenizer.check-utf8-correctness-and-calculate-length-fn]
> unsigned int

> [spec:hfst:sem:hfst-tokenizer.hfst.hfst-tokenizer.check-utf8-correctness-and-calculate-length-fn]
> Validates that `input_string` is well-formed UTF-8 and returns its length
> in UTF-16 code units. Sets a status code to U_ZERO_ERROR and length to 0,
> then calls ICU `u_strFromUTF8` with a NULL destination and capacity 0 (so
> it only measures), passing the input C-string with source length -1
> (NUL-terminated); this writes the required UTF-16 length into `length`.
> Because the destination is too small, ICU sets U_BUFFER_OVERFLOW_ERROR;
> this specific status is treated as success and reset to U_ZERO_ERROR. If
> after that the status still indicates failure (U_FAILURE), print a debug
> line to stderr ("DEBUG: <status>: <u_errorName>") and throw
> IncorrectUtf8CodingException with message `u_errorName(status)`. Otherwise
> return `length` as an unsigned int. Static member function.

> [spec:hfst:def:hfst-tokenizer.hfst.hfst-tokenizer.check-utf8-correctness-fn]
> void

> [spec:hfst:sem:hfst-tokenizer.hfst.hfst-tokenizer.check-utf8-correctness-fn]
> Validates that `input_string` is well-formed UTF-8, throwing
> IncorrectUtf8CodingException if not. Implemented by calling
> `check_utf8_correctness_and_calculate_length(input_string)` and discarding
> the returned length (cast to void). No return value. Static member
> function.

> [spec:hfst:def:hfst-tokenizer.hfst.hfst-tokenizer.get-next-symbol-size-fn]
> int

> [spec:hfst:sem:hfst-tokenizer.hfst.hfst-tokenizer.get-next-symbol-size-fn]
> Returns the byte length of the next token at the start of C-string
> `symbol`, given `split_characters`. Steps:
> 1. If `*symbol` is 0 (empty string), return 0.
> 2. Call `multi_char_symbols.find(symbol)`. If it returns non-NULL
>    (`multi_char_symbol_end`), the string begins with a registered
>    multi-character symbol; return the byte distance
>    `multi_char_symbol_end - symbol` as an int.
> 3. Else if `split_characters` is false, take the next combining grapheme
>    cluster using ICU: convert `symbol` (UTF-8, source len -1) to UTF-16
>    (`u_strFromUTF8` into a heap buffer of `strlen(symbol)+1` UChars,
>    getting `length`); open a UBRK_CHARACTER break iterator for locale "C"
>    (`ubrk_open`); set its text to the UTF-16 data of `length` units;
>    `begin = ubrk_first(graphemes)`, `end = ubrk_next(graphemes)`. If
>    `begin == end`, return 0; if `end == UBRK_DONE`, return 0. Convert the
>    UTF-16 slice `[begin, end)` back to UTF-8 (`u_strToUTF8` into a heap
>    buffer of `(end-begin)*4+1` bytes) and return `strlen(grapheme)` (its
>    byte count). On any ICU error at each step, print a diagnostic to
>    stderr but continue. (Allocated buffers are not freed — leak, preserve
>    behavior or document.)
> 4. Else (`split_characters` true) take only the next raw UTF-8 byte
>    sequence by inspecting the lead byte `*symbol`: if bit 0x80 is clear
>    return 1; else if bit 0x20 is clear return 2; else if bit 0x10 is clear
>    return 3; else return 4. Const member function.

> [spec:hfst:def:hfst-tokenizer.hfst.hfst-tokenizer.hfst-tokenizer-fn]
> HfstTokenizer::HfstTokenizer()

> [spec:hfst:sem:hfst-tokenizer.hfst.hfst-tokenizer.hfst-tokenizer-fn]
> Default constructor. Has an empty body; performs no explicit
> initialization. The member `multi_char_symbols` (a MultiCharSymbolTrie)
> and `skip_symbol_set` (a StringSet) are default-constructed empty.

> [spec:hfst:def:hfst-tokenizer.hfst.hfst-tokenizer.is-skip-symbol-fn]
> bool

> [spec:hfst:sem:hfst-tokenizer.hfst.hfst-tokenizer.is-skip-symbol-fn]
> Returns true if `s` should be skipped during tokenization: true when `s`
> is the empty string, OR when `s` is present in `skip_symbol_set` (i.e.
> `skip_symbol_set.find(s) != skip_symbol_set.end()`). Otherwise false.
> Const member function; does not mutate state.

> [spec:hfst:def:hfst-tokenizer.hfst.hfst-tokenizer.tokenize-and-align-flag-diacritics-fn]
> StringPairVector

> [spec:hfst:sem:hfst-tokenizer.hfst.hfst-tokenizer.tokenize-and-align-flag-diacritics-fn]
> Tokenizes `input_string` and `output_string` and aligns them into a
> StringPairVector, keeping flag diacritics aligned with themselves. Steps:
> 1. `check_utf8_correctness` on both input and output strings.
> 2. Tokenize each side via the single-string `tokenize(str, split_characters)`
>    into `input_spv` and `output_spv` (vectors of (sym,sym) pairs); only the
>    `.first` of each pair is used here. assert both are non-empty.
> 3. Iterate two cursors `it` (input) and `jt` (output) until BOTH reach end.
>    At each step build `sp` (the pair to emit) and optionally `sp_cont`:
>    - If input exhausted (`it==end`): if `jt->first` is a flag diacritic
>      (`FdOperation::is_diacritic`), emit it on both sides as
>      `(jt->first, jt->first)`; else pad input side with epsilon
>      `(internal_epsilon, jt->first)`. Advance `jt`.
>    - Else if output exhausted (`jt==end`): symmetric — diacritic copied to
>      both sides `(it->first, it->first)`, else `(it->first, internal_epsilon)`.
>      Advance `it`.
>    - Else (both available): if neither side is a diacritic, OR the two
>      tokens are equal (`*it == *jt`), emit `(it->first, jt->first)`.
>      Otherwise (a misaligned-flags case) call `warn_about_pair` with the
>      wrong pair `(it->first, jt->first)`, then emit `sp = (it->first,
>      it->first)` and set `sp_cont = (jt->first, jt->first)` to be emitted
>      next. Advance both `it` and `jt`.
> 4. After computing `sp`, push it. If `sp_cont` has both members non-empty
>    (i.e. it was set), push `sp_cont` as well.
> 5. Return the accumulated StringPairVector. Const member; `warn_about_pair`
>    is a function pointer callback receiving a std::pair<string,string>.

> [spec:hfst:def:hfst-tokenizer.hfst.hfst-tokenizer.tokenize-fn]
> StringPairVector

> [spec:hfst:sem:hfst-tokenizer.hfst.hfst-tokenizer.tokenize-fn]
> The 4-argument overload: tokenizes `input_string` and `output_string` and
> zips them into a StringPairVector, invoking the `warn_about_pair` callback
> on every produced pair. Steps:
> 1. `check_utf8_correctness` on both input and output strings.
> 2. Tokenize each side independently via the single-string
>    `tokenize(str, split_characters)` into `input_spv` and `output_spv`
>    (each a vector of (sym,sym) pairs); only `.first` is used.
> 3. If `input_spv.size() < output_spv.size()`: walk `it` over input_spv and
>    `jt` over output_spv in lockstep — for each input token emit pair
>    `(it->first, jt->first)`, advancing `jt`; after input is exhausted,
>    emit the remaining output tokens as `(internal_epsilon, jt->first)`.
> 4. Else (input length >= output length): walk `jt` over output_spv and
>    `it` over input_spv in lockstep — for each output token emit
>    `(it->first, jt->first)`, advancing `it`; after output is exhausted,
>    emit remaining input tokens as `(it->first, internal_epsilon)`.
> 5. In every emit branch, build the StringPair `sp`, call
>    `warn_about_pair(sp)`, then push `sp` onto the result.
> 6. Return the StringPairVector. Const member function.

> [spec:hfst:def:hfst-tokenizer.hfst.hfst-tokenizer.tokenize-one-level-fn]
> StringVector

> [spec:hfst:sem:hfst-tokenizer.hfst.hfst-tokenizer.tokenize-one-level-fn]
> Tokenizes `input_string` into a flat StringVector (one entry per token).
> Steps:
> 1. `check_utf8_correctness(input_string)`.
> 2. Walk a char pointer `s` from the start of the string while `*s != 0`:
>    compute `symbol_size = get_next_symbol_size(s, split_characters)`,
>    extract `symbol` as the substring of `symbol_size` bytes starting at
>    `s`, advance `s` by `symbol_size`. If `is_skip_symbol(symbol)`, skip
>    (continue) without emitting; otherwise push `symbol` onto the vector.
> 3. Return the StringVector. Const member function.

> [spec:hfst:def:hfst-tokenizer.hfst.hfst-tokenizer.tokenize-space-separated-fn]
> StringPairVector

> [spec:hfst:sem:hfst-tokenizer.hfst.hfst-tokenizer.tokenize-space-separated-fn]
> Splits `str` on ASCII space (' ') characters into a StringPairVector,
> emitting each whitespace-delimited run as an identity pair (symbol,symbol).
> Steps:
> 1. `check_utf8_correctness(str)`.
> 2. Track `pos` (scan index) and `symbol_pos` (start of current symbol,
>    initialized to std::string::npos meaning "no symbol started").
> 3. While `pos < str.size()`:
>    - If `str[pos] == ' '` and a symbol is in progress (`symbol_pos !=
>      npos`): the current symbol ended at `pos`; extract substring
>      `str[symbol_pos .. pos)` (length `pos - symbol_pos`), push
>      `(symbol, symbol)`, and reset `symbol_pos` to npos.
>    - Else if `str[pos] != ' '` and no symbol in progress: record
>      `symbol_pos = pos` (a new symbol begins).
>    - Otherwise do nothing.
>    - Increment `pos`.
> 4. After the loop, if a symbol is still in progress (`symbol_pos != npos`),
>    extract the trailing substring from `symbol_pos` to end and push it as
>    `(symbol, symbol)`.
> 5. Return the StringPairVector. Static member function (does not depend on
>    multi-char or skip-symbol state).

> [spec:hfst:def:hfst-tokenizer.hfst.multi-char-symbol-trie]
> class MultiCharSymbolTrie {
>   MultiCharSymbolTrieVector symbol_rests;
>   SymbolEndVector is_leaf;
> }

> [spec:hfst:def:hfst-tokenizer.hfst.multi-char-symbol-trie-vector]
> typedef std::vector<MultiCharSymbolTrie *> MultiCharSymbolTrieVector

> [spec:hfst:def:hfst-tokenizer.hfst.multi-char-symbol-trie.add-fn]
> void

> [spec:hfst:sem:hfst-tokenizer.hfst.multi-char-symbol-trie.add-fn]
> Inserts the NUL-terminated string starting at `p` into this trie node.
> If `is_end_of_string(p)` is true (i.e. `*(p+1) == 0`, so `p` points at the
> last character before the terminating NUL), call `set_symbol_end(p)` to
> mark the byte `*p` as a leaf/symbol terminator at this node. Otherwise,
> ensure the child trie for byte `*p` exists via `init_symbol_rests(p)`,
> then recurse into that child with `add_symbol_rest(p)` (which calls the
> child's `add(p+1)`). No return value; recursion descends one byte per
> level.

> [spec:hfst:def:hfst-tokenizer.hfst.multi-char-symbol-trie.add-symbol-rest-fn]
> void

> [spec:hfst:sem:hfst-tokenizer.hfst.multi-char-symbol-trie.add-symbol-rest-fn]
> Continues insertion of a string into the child trie keyed by the first
> byte. Indexes `symbol_rests` by `(unsigned char)(*p)` to get the child
> MultiCharSymbolTrie pointer (assumed already non-NULL, normally ensured by
> a prior `init_symbol_rests` call) and invokes its `add(p + 1)`, i.e.
> recursively adds the remainder of the string starting one byte later. No
> return value.

> [spec:hfst:def:hfst-tokenizer.hfst.multi-char-symbol-trie.find-fn]
> const char *

> [spec:hfst:sem:hfst-tokenizer.hfst.multi-char-symbol-trie.find-fn]
> Finds the longest registered multi-character symbol that is a prefix of
> the NUL-terminated string `p`, returning a pointer just past that symbol's
> last byte (i.e. to the next character after the match) or NULL if no
> registered symbol is a prefix. Steps:
> 1. `symbol_rest_trie = get_symbol_rest_trie(p)` = the child trie for byte
>    `*p` (or NULL if no child).
> 2. If the child is NULL: if `is_symbol_end(p)` (byte `*p` is a leaf here),
>    return `p + 1`; otherwise return NULL.
> 3. Otherwise recurse: `symbol_end = symbol_rest_trie->find(p + 1)`. If that
>    recursion returns NULL but `is_symbol_end(p)` is true, return `p + 1`
>    (a shorter symbol ends at this byte). Return `symbol_end` (which is the
>    longer match if found, else NULL when this byte is not a symbol end).
> Const member function; gives precedence to the longest match because the
> recursive (longer) result is preferred over the local symbol-end. Note the
> trie indexes by single bytes, so a "character" here is one byte.

> [spec:hfst:def:hfst-tokenizer.hfst.multi-char-symbol-trie.get-symbol-rest-trie-fn]
> MultiCharSymbolTrie *

> [spec:hfst:sem:hfst-tokenizer.hfst.multi-char-symbol-trie.get-symbol-rest-trie-fn]
> Returns the child trie associated with the first byte of `p`: indexes
> `symbol_rests` at `(unsigned char)(*p)` and returns that pointer, which is
> NULL when no child exists for that byte. Const member function; no
> mutation.

> [spec:hfst:def:hfst-tokenizer.hfst.multi-char-symbol-trie.init-symbol-rests-fn]
> void

> [spec:hfst:sem:hfst-tokenizer.hfst.multi-char-symbol-trie.init-symbol-rests-fn]
> Ensures a child trie exists for the first byte of `p`: if
> `symbol_rests[(unsigned char)(*p)]` is NULL, allocate a new
> MultiCharSymbolTrie and store its pointer at that index. If a child
> already exists, do nothing. No return value. Mutates `symbol_rests`.

> [spec:hfst:def:hfst-tokenizer.hfst.multi-char-symbol-trie.is-end-of-string-fn]
> bool

> [spec:hfst:sem:hfst-tokenizer.hfst.multi-char-symbol-trie.is-end-of-string-fn]
> Returns true if `p` points at the last character of a NUL-terminated
> string, i.e. when the next byte `*(p + 1)` equals 0. Const member
> function; pure read.

> [spec:hfst:def:hfst-tokenizer.hfst.multi-char-symbol-trie.is-symbol-end-fn]
> bool

> [spec:hfst:sem:hfst-tokenizer.hfst.multi-char-symbol-trie.is-symbol-end-fn]
> Returns whether the byte `*p` is marked as a symbol terminator (leaf) at
> this trie node: returns `is_leaf[(unsigned char)(*p)]`. Const member
> function; pure read.

> [spec:hfst:def:hfst-tokenizer.hfst.multi-char-symbol-trie.multi-char-symbol-trie-fn]
> MultiCharSymbolTrie::~MultiCharSymbolTrie(void)

> [spec:hfst:sem:hfst-tokenizer.hfst.multi-char-symbol-trie.multi-char-symbol-trie-fn]
> Destructor. Iterates over every entry in `symbol_rests` and `delete`s the
> pointed-to child MultiCharSymbolTrie. Since each child's own destructor
> runs on delete, this recursively frees the entire subtree. NULL entries
> are deleted harmlessly (delete on NULL is a no-op). No return value.

> [spec:hfst:def:hfst-tokenizer.hfst.multi-char-symbol-trie.set-symbol-end-fn]
> void

> [spec:hfst:sem:hfst-tokenizer.hfst.multi-char-symbol-trie.set-symbol-end-fn]
> Marks the byte `*p` as a symbol terminator (leaf) at this trie node:
> sets `is_leaf[(unsigned char)(*p)] = true`. No return value. Mutates
> `is_leaf`.

> [spec:hfst:def:hfst-tokenizer.hfst.string-vector]
> typedef std::vector<std::string> StringVector

> [spec:hfst:def:hfst-tokenizer.hfst.symbol-end-vector]
> typedef std::vector<bool> SymbolEndVector

> [spec:hfst:def:hfst-tokenizer.main-fn]
> int

> [spec:hfst:sem:hfst-tokenizer.main-fn]
> Test-build entry point (compiled only when MAIN_TEST is defined). Prints
> "Unit tests for <__FILE__>:" then "ok" to std::cout, and returns 0. It
> performs no actual assertions; ignores argc/argv.

