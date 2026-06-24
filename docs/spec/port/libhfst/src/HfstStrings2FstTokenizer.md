# libhfst/src/HfstStrings2FstTokenizer.cc, libhfst/src/HfstStrings2FstTokenizer.h

> [spec:hfst:def:hfst-strings2-fst-tokenizer.hfst.empty-multichar-symbol]
> class EmptyMulticharSymbol

> [spec:hfst:def:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer]
> class HfstStrings2FstTokenizer {
>   hfst::HfstTokenizer tokenizer;
>   std::string eps;
> }

> [spec:hfst:def:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.add-multichar-symbol-fn]
> void

> [spec:hfst:sem:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.add-multichar-symbol-fn]
> Registers `multichar_symbol` (a `const std::string&`) with the embedded
> `tokenizer` member by calling `tokenizer.add_multichar_symbol(multichar_symbol)`.
> Returns void; the only effect is mutating the tokenizer's set of recognized
> multichar symbols. No validation or branching.

> [spec:hfst:def:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.add-multichar-symbol-head-fn]
> void

> [spec:hfst:sem:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.add-multichar-symbol-head-fn]
> Given `multichar_symbol` (a `const std::string&`): if it is empty, throw
> `EmptyMulticharSymbol`. Otherwise tokenize it into one-level symbols via
> `tokenizer.tokenize_one_level(multichar_symbol, false)`, take the FIRST
> resulting token as the "head" (`*begin()`), and register the multichar symbol
> formed by prepending a backslash to that head, i.e.
> `tokenizer.add_multichar_symbol(std::string("\\") + head)`. Returns void;
> mutates the tokenizer's multichar-symbol set. The purpose is so that an escaped
> form `\X` of the symbol's leading character is recognized as a single token.

> [spec:hfst:def:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.check-cols-fn]
> void

> [spec:hfst:sem:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.check-cols-fn]
> Validates that `symbol` (a `const std::string&`) contains no unescaped colon
> (':', `COL_CHAR`). If `symbol` is empty, do nothing and return. Otherwise:
> if the first character `symbol[0]` is ':', throw `UnescapedColsFound`. Then
> scan for every subsequent ':' starting from index 1 (using
> `find(':', pos+1)` repeatedly). For each found position `pos`: if the
> preceding character `symbol[pos-1]` is NOT a backslash ('\\',
> `BACKSLASH_CHAR`), throw `UnescapedColsFound` (the colon is unescaped); also,
> if `pos > 1` and the character two before it `symbol[pos-2]` IS a backslash,
> throw `UnescapedColsFound` (the backslash itself was escaped, so the colon is
> effectively unescaped). Returns void if no violation found. The only side
> effect is potentially throwing `UnescapedColsFound`.

> [spec:hfst:def:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.get-col-pos-fn]
> int

> [spec:hfst:sem:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.get-col-pos-fn]
> Returns (as `int`) the byte index of the first unquoted colon (':',
> `COL_CHAR`) in `str` (a `const std::string&`), or -1 if there is none. If
> `str` is empty, return -1. If `str[0]` is ':', return 0. Otherwise iterate
> `i` from 1 up to `str.size()-1`; return the first `i` (cast to int) such that
> `str[i]` is ':' AND the preceding character `str[i-1]` is NOT a backslash
> ('\\', `BACKSLASH_CHAR`). If no such index exists, return -1. Pure function;
> no side effects. (Note: this helper is not referenced elsewhere in this file.)

> [spec:hfst:def:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.hfst-strings2-fst-tokenizer-fn]
> HfstStrings2FstTokenizer::HfstStrings2FstTokenizer(

> [spec:hfst:sem:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.hfst-strings2-fst-tokenizer-fn]
> Constructor taking `multichar_symbols` (a `StringVector&`) and `eps` (a
> `const std::string&`). Stores `eps` into the member `eps`. Then, in order:
> (1) if `eps` is non-empty, register it as a multichar symbol via
> `add_multichar_symbol(eps)`. (2) Register the escaped-special tokens directly
> on the tokenizer member: `"\\:"` (`BACKSLASH COL`), `"\\ "`
> (`BACKSLASH SPACE`), and `"\\\\"` (`BACKSLASH BACKSLASH`). (3) Register the
> escape multichar symbols via `add_multichar_symbol`: `COL_ESCAPE`
> ("@_COLON_@"), `TAB_ESCAPE` ("@_TAB_@"), `SPACE_ESCAPE` ("@_SPACE_@").
> (4) If `eps` is non-empty (`eps.size() > 0`), additionally call
> `tokenizer.add_multichar_symbol(eps)` and `add_multichar_symbol_head(eps)`.
> (5) Call `add_multichar_symbol_head(SPACE_ESCAPE)`. (6) For each symbol in
> `multichar_symbols` (in iteration order), call `add_multichar_symbol_head(sym)`
> then `add_multichar_symbol(sym)`. Side effects: mutates the embedded
> tokenizer's symbol sets; may throw `EmptyMulticharSymbol` (from
> `add_multichar_symbol_head` if any symbol head is empty).

> [spec:hfst:def:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.is-pair-input-symbol-fn]
> bool

> [spec:hfst:sem:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.is-pair-input-symbol-fn]
> Given a const-iterator `it` and end-iterator `end` over a `StringVector`,
> returns `true` iff `it` denotes the input member of a `X : Y` triple. Logic:
> if `it == end`, return false. Advance `it` (to the would-be ':' position); if
> now `it == end`, return false. If `*it != ":"` (`COL`), return false. Advance
> `it` again (to the would-be output symbol); if now `it == end`, return false.
> Otherwise return true. In other words, true exactly when there exist a next
> element equal to ":" followed by a further element after it. Does not modify
> any state (the iterator is taken by value); no side effects.

> [spec:hfst:def:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.make-pair-vector-fn]
> StringPairVector

> [spec:hfst:sem:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.make-pair-vector-fn]
> The two-argument overload `make_pair_vector(const StringVector &input,
> const StringVector &output)`. Builds and returns a `StringPairVector` by
> zipping `input` and `output` element-by-element, padding the shorter side with
> epsilons. Walk both with iterators while neither is exhausted: for each pair,
> compute `input_symbol = unescape(*input_it)` and
> `output_symbol = unescape(*output_it)`; push a `StringPair` whose first is
> `EPSILON_SYMBOL` ("@_EPSILON_SYMBOL_@") if `input_symbol` is empty or equals
> the member `eps`, else `input_symbol`, and whose second is likewise
> `EPSILON_SYMBOL` if `output_symbol` is empty or equals `eps`, else
> `output_symbol`; advance both iterators. After the loop, exactly one side may
> have remaining elements: if `input` was exhausted, append for each remaining
> output element a pair `(EPSILON_SYMBOL, e)` where `e` is `EPSILON_SYMBOL` when
> the raw element is empty or equals `eps` else `unescape(element)`; otherwise
> (input has leftovers) append for each remaining input element a pair
> `(e, EPSILON_SYMBOL)` with `e` computed the same way. Return the vector.
> Note: in the main loop, the empty/eps check is applied to the ALREADY
> unescaped symbols, whereas in the padding loops it is applied to the raw
> elements before calling `unescape`. May propagate exceptions from `unescape`
> (e.g. `UnescapedColsFound`).

> [spec:hfst:def:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.split-at-spaces-fn]
> StringVector

> [spec:hfst:sem:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.split-at-spaces-fn]
> Splits `str` (a `const std::string&`) into a `StringVector` of symbols,
> breaking on unescaped spaces while keeping ":" as its own delimiter token.
> First tokenize via `tokenizer.tokenize_one_level(str, false)` into `sv`
> (escaped sequences like "\\ " become single tokens, so a quoted space is not a
> bare " "). Maintain an accumulator `symbol` (initially empty) and a result
> vector `res`. Iterate `it` over `sv`:
> - If `*it == " "` (`SPACE`) and `symbol` is non-empty: push `symbol` to `res`,
>   then skip any run of consecutive following SPACE tokens (advance `it` while
>   `it+1 != end && *(it+1) == " "`), reset `symbol` to empty; if `it == end`
>   break (this end check follows the increments).
> - Else if `*it == " "` (symbol empty): skip the run of following consecutive
>   SPACE tokens the same way (collapse leading/duplicate spaces).
> - Else if `*it == ":"` (`COL`) and `symbol` is non-empty: push `symbol`, then
>   push `":"` as a separate token, reset `symbol` to empty.
> - Else if `*it == ":"` (symbol empty): push `":"`.
> - Else: append `*it` to `symbol`.
> After the loop, if `symbol` is non-empty, push it to `res`. Return `res`.
> Side effects: none beyond allocation; may propagate exceptions from the
> tokenizer.

> [spec:hfst:def:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.tokenize-pair-string-fn]
> StringPairVector

> [spec:hfst:sem:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.tokenize-pair-string-fn]
> Public method. Returns a `StringPairVector` representation of the pair string
> `str`. Parameter `spaces` (bool) selects tokenization: if true, set
> `tokenized_str = split_at_spaces(str)`. If false, set
> `tokenized_str = tokenizer.tokenize_one_level(str, false)`, then REMOVE every
> token equal to a bare backslash "\\" (`BACKSLASH`) using
> `std::remove`/`erase` (these standalone backslashes act only as escape
> markers between adjacent symbols and must be dropped). Finally return
> `make_pair_vector(tokenized_str)` (the single-argument overload, which
> interprets `X : Y` triples as input:output pairs and other tokens as identity
> pairs). May propagate exceptions from the called helpers.

> [spec:hfst:def:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.tokenize-string-pair-fn]
> StringPairVector

> [spec:hfst:sem:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.tokenize-string-pair-fn]
> Public method. Returns a `StringPairVector` representation of a string pair
> `str`, where a single ":" splits the whole string into an input string and an
> output string (NOT per-symbol pairs). Parameter `spaces` (bool): if true,
> `tokenized_str = split_at_spaces(str)`; otherwise
> `tokenized_str = tokenizer.tokenize_one_level(str, false)` (note: unlike
> `tokenize_pair_string`, bare backslash tokens are NOT removed here). Then find
> the first token equal to ":" (`COL`) via `std::find`. If none is found, call
> the two-argument `make_pair_vector(tokenized_str, tokenized_str)` (input and
> output are identical) and return it. If a ":" token is found at iterator `it`,
> call `make_pair_vector(StringVector(begin, it), StringVector(it+1, end))`,
> i.e. everything before the colon is the input side and everything after it is
> the output side, and return the result. May propagate exceptions from the
> called helpers.

> [spec:hfst:def:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.unescape-fn]
> std::string

> [spec:hfst:sem:hfst-strings2-fst-tokenizer.hfst.hfst-strings2-fst-tokenizer.unescape-fn]
> Takes `symbol` by value (a `std::string`), removes backslash escaping, and
> returns the unescaped string. Steps:
> (1) Call `check_cols(symbol)` first; this may throw `UnescapedColsFound`.
> (2) Special case: if `symbol` equals exactly "\\\\" (`BACKSLASH BACKSLASH`),
>     return "\\" (`BACKSLASH`).
> (3) Replace every occurrence of "\\\\" (escaped backslash) with the sentinel
>     `BACKSLASH_ESC` ("@_BACKSLASH_@"), repeatedly via `find`/`replace(pos,2,...)`
>     from the start each time, so quoted backslashes are protected from the next
>     step.
> (4) Replace every remaining single backslash "\\" with the empty string
>     (removing escape markers), again repeatedly from the start.
> (5) Replace every `BACKSLASH_ESC` sentinel with the empty string (length
>     `strlen(BACKSLASH_ESC)`). [Note: in the C++ the replacement is `EMPTY`,
>     i.e. the previously protected `\\` ends up removed entirely.]
> (6) Replace every `SPACE_ESCAPE` ("@_SPACE_@") with a single space " ".
> (7) Replace every `TAB_ESCAPE` ("@_TAB_@") with the three-character string
>     "   " (three spaces, exactly as written in the source).
> (8) Replace every `COL_ESCAPE` ("@_COLON_@") with ":".
> Return the resulting string. Each replacement loop restarts its `find` from
> position 0. Only side effect is the possible `UnescapedColsFound` throw in
> step 1.

> [spec:hfst:def:hfst-strings2-fst-tokenizer.hfst.unescaped-cols-found]
> class UnescapedColsFound

> [spec:hfst:def:hfst-strings2-fst-tokenizer.main-fn]
> int

> [spec:hfst:sem:hfst-strings2-fst-tokenizer.main-fn]
> Test/demo `main` compiled only under `#ifdef TEST_FST_2_STRINGS_TOKENIZER`.
> Builds a `StringVector multichar_symbols` containing "##", "+NOM", and
> ":NOM:SG". Constructs an `hfst::HfstStrings2FstTokenizer tokenizer` with those
> multichar symbols and eps = "@_EPS_@". Then exercises the tokenizer by calling
> the free helpers `test_ps` and `test_sp` on a sequence of hard-coded input
> strings (some with `spaces=false`, some with `spaces=true`), printing the
> tokenization of each to stdout. Specifically two `test_ps` calls (pair-string
> tokenization, the second using spaces) followed by five `test_sp` calls
> (string-pair tokenization, the last two using spaces). Returns int (falls off
> the end without an explicit return). Pure demonstration; no return value is
> used.

> [spec:hfst:def:hfst-strings2-fst-tokenizer.string-pair]
> typedef std::pair<std::string,std::string> StringPair

> [spec:hfst:def:hfst-strings2-fst-tokenizer.string-pair-vector]
> typedef std::vector<StringPair> StringPairVector

> [spec:hfst:def:hfst-strings2-fst-tokenizer.string-vector]
> typedef std::vector<std::string> StringVector

> [spec:hfst:def:hfst-strings2-fst-tokenizer.test-ps-fn]
> void

> [spec:hfst:sem:hfst-strings2-fst-tokenizer.test-ps-fn]
> Test helper compiled only under `#ifdef TEST_FST_2_STRINGS_TOKENIZER`.
> Takes `input` (const string&), `tokenizer` (HfstStrings2FstTokenizer&), and
> `spaces` (bool). Prints "Tokenizing: " followed by `input` and a newline, then
> "Tokenized:" and a newline. Calls `tokenizer.tokenize_pair_string(input,
> spaces)` to get a `StringPairVector spv`. Iterates over `spv`: for each pair,
> if `first != second`, print "first : second" on its own line; otherwise print
> just `first` on its own line. Finally print an empty line. Returns void; only
> side effect is stdout output.

> [spec:hfst:def:hfst-strings2-fst-tokenizer.test-sp-fn]
> void

> [spec:hfst:sem:hfst-strings2-fst-tokenizer.test-sp-fn]
> Test helper compiled only under `#ifdef TEST_FST_2_STRINGS_TOKENIZER`.
> Identical structure to `test_ps` but calls `tokenizer.tokenize_string_pair`
> instead of `tokenize_pair_string`. Takes `input` (const string&), `tokenizer`
> (HfstStrings2FstTokenizer&), and `spaces` (bool). Prints "Tokenizing: " +
> `input` + newline, then "Tokenized:" + newline. Calls
> `tokenizer.tokenize_string_pair(input, spaces)` to get a `StringPairVector
> spv`, iterates it printing "first : second" when the members differ else just
> `first`, one per line, then prints a trailing empty line. Returns void; only
> side effect is stdout output.

