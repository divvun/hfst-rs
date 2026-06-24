# libhfst/src/parsers/xre_utils.cc

> [spec:hfst:def:xre-utils.hfst.xre.add-percents-fn]
> char *

> [spec:hfst:sem:xre-utils.hfst.xre.add-percents-fn]
> Escapes special characters in a C string by prefixing them with `%`.
> Allocates a new buffer of size `strlen(s) * 2 + 1` bytes. Iterates over
> each character of `s` until the terminating NUL. If the current character
> is one of the set `@ - <space> | ! : ; 0 \ & ? $ + * / _ ( ) { } [ ]`,
> first writes a `%` to the output, then writes the character itself.
> Otherwise just writes the character. Terminates the output with `'\0'`
> and returns the newly allocated buffer (caller owns it). Note `/` appears
> twice in the test, which is redundant but harmless.

> [spec:hfst:def:xre-utils.hfst.xre.check-multichar-symbol-fn]
> void

> [spec:hfst:sem:xre-utils.hfst.xre.check-multichar-symbol-fn]
> Warns if a multichar symbol was used without being declared. Reads the
> module-global pointer `defined_multichar_symbols_`. If that pointer is
> NULL, returns immediately (no checking active). Otherwise looks up
> `std::string(symbol)` in the set it points to; if not found, obtains the
> error stream via `xreerrstr()`, writes `warning: multichar symbol
> '<symbol>' used but not defined` followed by a newline, and flushes it
> with `xreflush`. Returns void.

> [spec:hfst:def:xre-utils.hfst.xre.compile-first-fn]
> HfstTransducer *

> [spec:hfst:sem:xre-utils.hfst.xre.compile-first-fn]
> Like `compile`, but parses only the first regex from the input and
> reports how many characters were consumed. Sets module globals: `data =
> strdup(xre.c_str())` (keeping that pointer as local `startptr_`), `len =
> strlen(data)`, `definitions = defs`, `function_definitions = func_defs`,
> `function_arguments = func_args`, `symbol_lists = lists`, `format =
> impl`, and `contains_only_comments = false`. Initializes a `yyscan_t`
> scanner via `xrelex_init`, creates a flex buffer with
> `xre_scan_string(startptr_, scanner)`. Saves the old value of
> `hfst::xre::allow_extra_text_at_end`, sets it to `true`, sets
> `hfst::xre::cr = 0` and `hfst::xre::lr = 1`. Runs `xreparse(scanner)`,
> stores its return value, then writes `chars_read = hfst::xre::cr` (number
> of characters read) and restores `allow_extra_text_at_end` to the saved
> value. Cleans up: `xre_delete_buffer(bs, scanner)`, `xrelex_destroy`,
> `free(startptr_)`, sets `data = 0` and `len = 0`. If the parse returned 0
> and `contains_only_comments` is false, deep-copies `*last_compiled` into a
> new heap `HfstTransducer`, deletes `last_compiled`, and returns the copy;
> otherwise returns NULL.

> [spec:hfst:def:xre-utils.hfst.xre.compile-fn]
> HfstTransducer *

> [spec:hfst:sem:xre-utils.hfst.xre.compile-fn]
> Compiles an XRE regular-expression string into a transducer. Sets module
> globals: `data = strdup(xre.c_str())` (kept as local `startptr_`), `len =
> strlen(data)`, `definitions = defs`, `function_definitions = func_defs`,
> `function_arguments = func_args`, `symbol_lists = lists`, `format =
> impl`, and `contains_only_comments = false`. Initializes a `yyscan_t`
> scanner with `xrelex_init`, creates a flex buffer via
> `xre_scan_string(startptr_, scanner)`, and runs `xreparse(scanner)`,
> capturing its return value. Then cleans up: `xre_delete_buffer(bs,
> scanner)`, `xrelex_destroy(scanner)`, `free(startptr_)`, and sets `data =
> 0`, `len = 0`. If the parse returned 0 and `contains_only_comments` is
> false, deep-copies `*last_compiled` into a new heap `HfstTransducer`,
> deletes `last_compiled`, and returns the new pointer (caller owns it).
> Otherwise returns NULL. Unlike `compile_first`, does not touch
> `allow_extra_text_at_end` or the cr/lr counters.

> [spec:hfst:def:xre-utils.hfst.xre.contains-fn]
> HfstTransducer *

> [spec:hfst:sem:xre-utils.hfst.xre.contains-fn]
> Builds a transducer accepting any string that contains `t` as a substring,
> i.e. `$[t]` = `?* t ?*`. Constructs `any` as a transducer over the
> identity symbol `hfst::internal_identity` in `hfst::xre::format`, then
> `repeat_star().minimize()` to get `?*`. Allocates a new heap transducer
> `retval` as a copy of `any`, then concatenates `*t` and concatenates
> `any` again (so `?* t ?*`). Calls `retval->optimize()` and returns the
> heap pointer (caller owns it).

> [spec:hfst:def:xre-utils.hfst.xre.contains-once-fn]
> HfstTransducer *

> [spec:hfst:sem:xre-utils.hfst.xre.contains-once-fn]
> Builds a transducer accepting strings that contain `c` exactly once,
> computing `[$c - $[ [[?+ c ?*] & [c ?*]] | [[c ?+] & c] ]]`. Builds
> `any_star` = `?*` (identity over `hfst::xre::format`, `repeat_star()`,
> `minimize()`) and `any_plus` = `?+` (identity, `repeat_plus()`,
> `minimize()`). Allocates `t1` on heap as `?+`, concatenates `*c`,
> optimizes, concatenates `any_star`, optimizes (so `?+ c ?*`). Builds `t2`
> = copy of `*c`, concatenate `any_star`, optimize (`c ?*`). Intersects
> `t1` with `t2` (so `[?+ c ?*] & [c ?*]`). Builds `t3` = copy of `*c`,
> concatenate `any_plus`, optimize, intersect with `*c`, optimize (so `[c
> ?+] & c`). Disjuncts `t3` into `t1` and optimizes (the union). Computes
> `cont_t1 = contains(t1)` (=$[t1]) and deletes `t1`, and `cont_c =
> contains(c)` (=$[c]). Subtracts `*cont_t1` from `cont_c`, optimizes
> `cont_c`, deletes `cont_t1`, and returns `cont_c` (caller owns it).

> [spec:hfst:def:xre-utils.hfst.xre.contains-once-optional-fn]
> HfstTransducer *

> [spec:hfst:sem:xre-utils.hfst.xre.contains-once-optional-fn]
> Builds a transducer accepting strings that contain `t` zero or one times,
> i.e. `~$[t] | contains_once(t)`. Computes `cont_t = contains(t)`
> (=$[t]). Builds `neg_t` = `?*` (identity over `hfst::xre::format`,
> `repeat_star()`, `optimize()`), subtracts `*cont_t` from it and optimizes
> (so `~$[t]`, the strings not containing t). Deletes `cont_t`. Computes
> `retval = contains_once(t)` (strings containing t exactly once), disjuncts
> `neg_t` into it, optimizes, and returns `retval` (caller owns it).

> [spec:hfst:def:xre-utils.hfst.xre.contains-twolc-fn]
> HfstTransducer *

> [spec:hfst:sem:xre-utils.hfst.xre.contains-twolc-fn]
> Constructs a two-level rule transducer related to containing `t`, marked
> with the symbol `M`. Builds `marker` = `[0:M ?]*`: a transducer mapping
> epsilon (`@_EPSILON_SYMBOL_@`) to `M`, concatenated with identity
> (`@_IDENTITY_SYMBOL_@`), `repeat_star().minimize()`, all in `t->get_type()`.
> Builds `right_context` = copy of `*t`, `insert_freely(StringPair("M",
> "@_EPSILON_SYMBOL_@")).optimize()` then `insert_freely(StringPair("M",
> "M")).optimize()`. Builds `left_context` = epsilon transducer. Forms the
> `HfstTransducerPair context(left_context, right_context)`. Builds
> `mappings` containing the single pair `M:M`. Builds `alphabet` containing
> `M:M`, `M:@_EPSILON_SYMBOL_@`, and identity:identity; then, from a
> `HfstBasicTransducer` view of `*t`, gets all transition pairs and for each
> inserts the pair itself plus the two identity pairs (first:first,
> second:second). Computes `rule =
> hfst::rules::two_level_if_and_only_if(context, mappings, alphabet)`.
> Returns a new heap `HfstTransducer(rule)` immediately. (The trailing code
> that composes `marker` with `rule` is dead code, never executed because of
> the earlier return.)

> [spec:hfst:def:xre-utils.hfst.xre.contains-with-weight-fn]
> HfstTransducer *

> [spec:hfst:sem:xre-utils.hfst.xre.contains-with-weight-fn]
> Builds a transducer that adds `weight` once for each occurrence of `t`,
> computing `[ 0::weight @-> 0 || _ [t] ] - [?* - $[t]]`. Builds
> `weighted_epsilon` = epsilon transducer (`hfst::internal_epsilon`,
> `hfst::xre::format`) with `set_final_weights(weight)`, and plain `epsilon`.
> Forms mapping pair (weighted_epsilon, epsilon) into a
> `HfstTransducerPairVector`, and context pair (epsilon, *t) into a context
> vector. Constructs a `hfst::xeroxRules::Rule rule(mappingPairVector,
> contextPairVector, hfst::xeroxRules::REPL_UP)` and computes `weighted_rule
> = replace(rule, false)`. Then builds `noT` = `?*` (identity over
> `t->get_type()`, `repeat_star().minimize()`), computes `oneOrMoreT =
> contains(t)` (=$[t]), subtracts `*oneOrMoreT` from `noT`, optimizes
> `noT`, deletes `oneOrMoreT` (so `noT` = strings not containing t).
> Subtracts `noT` from `weighted_rule`, optimizes, and returns a new heap
> `HfstTransducer(weighted_rule)` (caller owns it).

> [spec:hfst:def:xre-utils.hfst.xre.count-lines-fn]
> void

> [spec:hfst:sem:xre-utils.hfst.xre.count-lines-fn]
> Scans the NUL-terminated string `s` and updates the module-global
> counters `hfst::xre::lr` (line count) and `hfst::xre::cr` (character
> count). Iterates a pointer `c` over each character until `'\0'`. If `*c`
> is `'\n'`, increments `lr`. Else if `*c` is `'\r'`, advances `c` one and
> checks: if the next char is `'\n'` increments `cr` (counting the CR of a
> CRLF pair) otherwise steps `c` back one; either way increments `lr`. After
> these checks, unconditionally increments `cr` and advances `c` by one. Net
> effect: `lr` counts line breaks (treating `\n`, `\r`, and `\r\n` as line
> ends) and `cr` counts characters processed (with a `\r\n` pair counted as
> two via the special `cr` bump). Returns void.

> [spec:hfst:def:xre-utils.hfst.xre.define-function-args-fn]
> bool

> [spec:hfst:sem:xre-utils.hfst.xre.define-function-args-fn]
> Registers each actual argument transducer as a temporary named definition
> for a function call. First calls `is_valid_function_call(name, args)`; if
> that returns false, returns false immediately. Otherwise iterates over
> `*args` with a 1-based counter `arg_number`. For each argument, builds the
> definition key `"@" + name + arg_number + "@"` (the number formatted via
> `ostringstream`), and stores `definitions[key] = new
> HfstTransducer(*it)` (a heap copy added to the module-global
> `definitions` map). Returns true. Side effect: inserts heap allocations
> into `definitions` that must later be freed (see
> `undefine_function_args`).

> [spec:hfst:def:xre-utils.hfst.xre.escape-enclosing-angle-brackets-fn]
> char *

> [spec:hfst:sem:xre-utils.hfst.xre.escape-enclosing-angle-brackets-fn]
> If the C string `s` is wrapped in angle brackets (`<...>`), rewrites it as
> `@_<...>_@`. If `s[0]` is not `'<'`, returns `s` unchanged. Otherwise
> walks to the last character (index of the final non-NUL char); if that
> last char is not `'>'`, returns `s` unchanged. If both conditions hold,
> builds `retval = "@_" + s + "_@"`, frees the original `s`, and returns
> `strdup(retval.c_str())` (a freshly allocated copy the caller owns).

> [spec:hfst:def:xre-utils.hfst.xre.expand-definition-fn]
> HfstTransducer *

> [spec:hfst:sem:xre-utils.hfst.xre.expand-definition-fn]
> Substitutes a defined symbol inside transducer `tr` with its definition,
> mutating `tr` in place. If the module-global flag `expand_definitions` is
> true, iterates over the `definitions` map looking for an entry whose key
> equals `symbol` (via `strcmp`). On the first match: captures the
> definition's alphabet, calls `tr->substitute(StringPair(symbol, symbol),
> *(it->second), false)` (replacing the symbol pair with the definition
> transducer, without harmonizing); then if `symbol` is not present in the
> definition's alphabet, calls `tr->remove_from_alphabet(symbol)`; then
> breaks out of the loop. If `expand_definitions` is false or no match is
> found, `tr` is unchanged. Returns `tr` (the same pointer that was passed
> in). Note: this is the two-argument overload; a separate one-argument
> `expand_definition` exists that instead returns a new transducer.

> [spec:hfst:def:xre-utils.hfst.xre.get-function-xre-fn]
> const char *

> [spec:hfst:sem:xre-utils.hfst.xre.get-function-xre-fn]
> Looks up the XRE source string for a defined function. Searches the
> module-global `function_definitions` map for the key `name`. If not found,
> returns NULL. Otherwise returns `it->second.c_str()` — a pointer to the
> internal C string of the stored definition (valid only as long as the map
> entry lives).

> [spec:hfst:def:xre-utils.hfst.xre.get-n-to-k-fn]
> int *

> [spec:hfst:sem:xre-utils.hfst.xre.get-n-to-k-fn]
> Parses a repetition range of the form `^{n,k}` or `^n,k` from string `s`
> into a two-element int array. Allocates `rv` = `malloc(sizeof(int)*2)`.
> The first character of `s` is assumed to be the `^` operator. If `s[1]`
> is `'{'`, parses `rv[0] = strtol(s+2, &endptr, 10)` (the `n` after
> `^{`), then `rv[1] = strtol(endptr+1, &finalptr, 10)` (the `k` after the
> comma), and asserts the final char is `'}'`. Otherwise (no brace), parses
> `rv[0] = strtol(s+1, &endptr, 10)` and `rv[1] = strtol(endptr+1,
> &finalptr, 10)`, asserting `*finalptr == '\0'`. Returns the heap array
> `rv` (caller owns it), where `rv[0]` is the lower bound and `rv[1]` the
> upper bound.

> [spec:hfst:def:xre-utils.hfst.xre.get-quoted-fn]
> char *

> [spec:hfst:sem:xre-utils.hfst.xre.get-quoted-fn]
> Extracts the substring of `s` between its first and last double-quote
> characters. Computes `qstart = strchr(s, '"') + 1` (first char after the
> first `"`) and `qend = strrchr(s, '"')` (the last `"`). Duplicates from
> `qstart` to end with `strdup` into `qpart`, then writes a `'\0'` at offset
> `qend - qstart` within `qpart` to truncate it at the closing quote.
> Returns `qpart` (heap-allocated, caller owns it). Assumes `s` contains at
> least two `"` characters (no validation).

> [spec:hfst:def:xre-utils.hfst.xre.get-weight-fn]
> double

> [spec:hfst:sem:xre-utils.hfst.xre.get-weight-fn]
> Parses a floating-point weight from the start of string `s`. Initializes
> `rv = -3.1415` (overwritten). Advances a pointer `weightstart` past any
> leading skip characters — spaces, tabs, and `';'` — stopping at NUL or the
> first other char. Then `rv = strtod(weightstart, &endp)` and asserts the
> parse consumed at least one character (`endp != weightstart`). Returns
> `rv`.

> [spec:hfst:def:xre-utils.hfst.xre.getinput-fn]
> int

> [spec:hfst:sem:xre-utils.hfst.xre.getinput-fn]
> Flex input callback that copies up to `maxlen` bytes from the module
> globals `data`/`len` into `buf`. If `maxlen` exceeds the remaining length
> `len`, clamps `maxlen` to `len` (via `hfst::size_t_to_int(len)`). Copies
> `maxlen` bytes from `data` to `buf` with `memcpy`, advances `data` by
> `maxlen`, decrements `len` by `maxlen`, and returns `maxlen` (the number
> of bytes provided; 0 signals end of input).

> [spec:hfst:def:xre-utils.hfst.xre.has-non-identity-pairs-fn]
> bool

> [spec:hfst:sem:xre-utils.hfst.xre.has-non-identity-pairs-fn]
> Returns true if transducer `t` has any transition whose input symbol
> differs from its output symbol. Builds a `HfstBasicTransducer` view of
> `*t`, gets its set of transition symbol pairs via `get_transition_pairs()`,
> and iterates; if any pair has `first != second`, returns true immediately.
> If none do, returns false.

> [spec:hfst:def:xre-utils.hfst.xre.insert-angle-bracket-substitutions-fn]
> static void

> [spec:hfst:sem:xre-utils.hfst.xre.insert-angle-bracket-substitutions-fn]
> Static helper: if `str` has the form `@_<...>_@`, records a substitution
> mapping it back to the un-escaped `<...>` form. If `str.length() < 6`,
> returns immediately. If the first three chars are `"@_<"` and the last
> three chars are `">_@"`, computes `substituting_str = str.substr(2,
> str.length() - 4)` (drops the leading `@_` and trailing `_@`, leaving
> `<...>`) and inserts the pair `StringPair(str, substituting_str)` into the
> `substitutions` map (passed by reference, mutated). Returns void.

> [spec:hfst:def:xre-utils.hfst.xre.is-definition-fn]
> bool

> [spec:hfst:sem:xre-utils.hfst.xre.is-definition-fn]
> Returns true if `symbol` is a defined name. Constructs
> `std::string(symbol)` and returns false if it is not found as a key in the
> module-global `definitions` map, true otherwise.

> [spec:hfst:def:xre-utils.hfst.xre.is-valid-function-call-fn]
> bool

> [spec:hfst:sem:xre-utils.hfst.xre.is-valid-function-call-fn]
> Validates that a function `name` is defined and is called with the right
> number of arguments. Looks up `name` in the module-global
> `function_definitions` map and in `function_arguments` map. If either
> lookup fails (name not defined), gets the error stream via `xreerrstr()`,
> writes `No such function defined: '<name>'` with a newline, flushes via
> `xreflush`, and returns false. Otherwise reads `number_of_args =
> function_arguments[name]`; if it does not equal `args->size()`, writes to
> the error stream `Wrong number of arguments: function '<name>' expects
> <number_of_args>, <args->size()> given` with a newline, flushes, and
> returns false. If both checks pass, returns true.

> [spec:hfst:def:xre-utils.hfst.xre.merge-first-to-second-fn]
> HfstTransducer *

> [spec:hfst:sem:xre-utils.hfst.xre.merge-first-to-second-fn]
> Merges transducer `tr1` into `tr2` and returns `tr2`. Builds an
> `XreConstructorArguments args` from the current module globals
> (`hfst::xre::definitions`, `function_definitions`, `function_arguments`,
> `symbol_lists`, `format`) — these are needed because the merge operation
> internally creates an `XreCompiler` that would otherwise overwrite this
> state. Optimizes `tr1` via `tr1->optimize()`, then calls `tr2->merge(*tr1,
> args)` (mutating `tr2`). Returns `tr2` (the same pointer passed in).

> [spec:hfst:def:xre-utils.hfst.xre.parse-quoted-fn]
> char *

> [spec:hfst:sem:xre-utils.hfst.xre.parse-quoted-fn]
> Parses a double-quoted XRE string, resolving backslash escapes, and
> returns the decoded bytes plus the UTF-8 length. Gets the error stream via
> `xreerrstr()`. Extracts the inner text with `get_quoted(s)` (heap string
> `quoted`). Allocates output `rv` of size `strlen(quoted) + 1`. Iterates a
> read pointer `p` over `quoted`, writing to `r`. For each char: if it is a
> raw `'\n'` or `'\r'`, throws the C-string exception "Unescaped newline
> characters found inside quoted string." If it is not a backslash, copies
> it verbatim and advances both pointers. If it is a backslash, switches on
> the next char `*(p+1)`:
> - digits `'0'`-`'7'` (octal): prints an unimplemented-octal-escape message
>   to `err`, flushes, writes `'\0'` (does NOT advance `r`), and advances `p`
>   by 5.
> - `'a' -> '\a'`, `'b' -> '\b'`, `'f' -> '\f'`, `'n' -> '\n'`, `'r' ->
>   '\r'`, `'t' -> '\t'`, `'v' -> '\v'`: writes the control char, advances
>   `r` by 1 and `p` by 2.
> - `'u'`: prints "Unimplemented: parse unicode escapes in ..." to `err`,
>   flushes, writes `'\0'`, advances `r` by 1 and `p` by 6.
> - `'x'`: parses `strtol(p+2, &endp, base 10)` into `i`; if `0 < i <= 127`
>   writes `(char)i`, else prints an unimplemented `\x<i>` message, flushes,
>   and writes `'\0'`; advances `r` by 1, asserts `endp != p`, and sets `p =
>   endp`.
> - `'\0'` (backslash at end): prints "End of line after \\ escape" to
>   `err`, flushes, writes `'\0'`, advances `r` by 1 and `p` by 1.
> - default: copies the literal next char `*(p+1)`, advances `r` by 1 and
>   `p` by 2.
> After the loop, writes terminating `'\0'` to `*r`, frees `quoted`, sets the
> out-parameter `length =
> HfstTokenizer::check_utf8_correctness_and_calculate_length(string(rv))`,
> and returns `rv` (heap-allocated, caller owns it).

> [spec:hfst:def:xre-utils.hfst.xre.set-substitution-function-symbol-fn]
> void

> [spec:hfst:sem:xre-utils.hfst.xre.set-substitution-function-symbol-fn]
> Stores `symbol` into the module-global `substitution_function_symbol`
> string. Returns void.

> [spec:hfst:def:xre-utils.hfst.xre.strip-curly-fn]
> char *

> [spec:hfst:sem:xre-utils.hfst.xre.strip-curly-fn]
> Returns a new string with a leading `'{'` and/or trailing `'}'` removed
> from `s`. Allocates `stripped` via `calloc(strlen(s) + 1)`. Walks `s`
> with pointer `c` and output index `i`. When the current char is `'{'` at
> position 0, or is `'}'` immediately before the NUL (last char): if the
> next char is `'\0'` (this is the closing brace at the very end) breaks out
> of the loop; otherwise (the opening brace case) copies the next char
> `*(c+1)` into output, increments `i`, and advances `c` by 2. For all other
> characters, copies the char, increments `i`, advances `c` by 1. After the
> loop writes terminating `'\0'` at `stripped[i]` and returns `stripped`
> (heap-allocated, caller owns it).

> [spec:hfst:def:xre-utils.hfst.xre.strip-newline-fn]
> char *

> [spec:hfst:sem:xre-utils.hfst.xre.strip-newline-fn]
> Truncates/removes newline characters in place. Iterates each position of
> `s` until NUL; whenever the char is `'\n'` or `'\r'`, overwrites it with
> `'\0'`. (Because each newline becomes a NUL terminator, this effectively
> truncates `s` at the first newline.) Returns the same `s` pointer.

> [spec:hfst:def:xre-utils.hfst.xre.strip-percents-fn]
> char *

> [spec:hfst:sem:xre-utils.hfst.xre.strip-percents-fn]
> Returns a new string with `%` escape characters removed from `s`.
> Allocates `stripped` via `calloc(strlen(s) + 1)`. Walks `s` with pointer
> `c` and output index `i`. When the current char is `'%'`: if the next char
> is `'\0'` (a trailing lone `%`) breaks out of the loop; otherwise copies
> the next char `*(c+1)` to output, increments `i`, advances `c` by 2 (i.e.
> drops the `%` and keeps the escaped char). For any non-`%` char, copies it,
> increments `i`, advances `c` by 1. After the loop writes terminating
> `'\0'` at `stripped[i]` and returns `stripped` (heap-allocated, caller
> owns it).

> [spec:hfst:def:xre-utils.hfst.xre.substitution-function-fn]
> bool

> [spec:hfst:sem:xre-utils.hfst.xre.substitution-function-fn]
> Substitution callback used during transducer substitution. Given a symbol
> pair `p` and an output set `sps` (by reference): if either `p.first` or
> `p.second` equals the module-global `substitution_function_symbol`,
> inserts the identity pair
> `StringPair(substitution_function_symbol, substitution_function_symbol)`
> into `sps` and returns true. Otherwise leaves `sps` unchanged and returns
> false.

> [spec:hfst:def:xre-utils.hfst.xre.undefine-function-args-fn]
> void

> [spec:hfst:sem:xre-utils.hfst.xre.undefine-function-args-fn]
> Removes and frees the temporary argument definitions created by
> `define_function_args` for function `name`. Looks up `name` in the
> module-global `function_arguments` map; if absent, returns immediately.
> Otherwise for each `arg_number` from 1 to the stored argument count
> (inclusive), reconstructs the key `"@" + name + arg_number + "@"` (number
> via `ostringstream`), calls `delete definitions[key]` to free the heap
> transducer, and erases the entry from `definitions`. Returns void.

> [spec:hfst:def:xre-utils.hfst.xre.unescape-enclosing-angle-brackets-fn]
> HfstTransducer *

> [spec:hfst:sem:xre-utils.hfst.xre.unescape-enclosing-angle-brackets-fn]
> Reverses the `<...>` to `@_<...>_@` escaping applied earlier, mutating `t`
> in place. Builds an empty `HfstSymbolSubstitutions substitutions` map. Gets
> `t`'s alphabet via `t->get_alphabet()` and, for each symbol, calls the
> static helper `insert_angle_bracket_substitutions(symbol, substitutions)`
> (which adds a mapping `@_<...>_@ -> <...>` for any symbol of that shape).
> If `substitutions.size() == 0` (no such symbols), returns `t` unchanged.
> Otherwise applies `t->substitute(substitutions)`, then `t->optimize()`, and
> returns `t` (the same pointer passed in).

> [spec:hfst:def:xre-utils.hfst.xre.warn-about-hfst-special-symbol-fn]
> void

> [spec:hfst:sem:xre-utils.hfst.xre.warn-about-hfst-special-symbol-fn]
> Warns if `symbol` looks like an HFST special symbol `@_..._@`. Returns
> immediately if `symbol[0] != '@'` or `symbol[1] != '_'`. Computes
> `max_index` = the index of the last character (length of the string minus
> one) by advancing from index 2 to the NUL then decrementing once. Returns
> if `max_index < 3`, or if `symbol[max_index] != '@'`, or if
> `symbol[max_index - 1] != '_'`. Returns if `verbose_` is false. Then (under
> a redundant second `verbose_` check) obtains the error stream via
> `xreerrstr()`, writes `warning: '<symbol>' is not an ordinary symbol in
> hfst` followed by `std::endl`, and flushes with `xreflush`. Returns void.

> [spec:hfst:def:xre-utils.hfst.xre.warn-about-special-symbols-in-replace-fn]
> void

> [spec:hfst:sem:xre-utils.hfst.xre.warn-about-special-symbols-in-replace-fn]
> Warns about special symbols appearing in a replace-rule transducer `t`.
> Returns immediately if `verbose_` is false. Otherwise obtains the error
> stream via `xreerrstr()`, gets `t->get_alphabet()`, and iterates over it.
> For each symbol that `HfstTransducer::is_special_symbol` reports true AND is
> not `hfst::internal_epsilon`, not `hfst::internal_unknown`, and not
> `hfst::internal_identity`, writes `warning: using special symbol
> '<symbol>' in replace rule, use substitute instead` followed by `std::endl`
> to the error stream. After the loop, flushes via `xreflush`. Returns void.

> [spec:hfst:def:xre-utils.hfst.xre.warn-about-xfst-special-symbol-fn]
> void

> [spec:hfst:sem:xre-utils.hfst.xre.warn-about-xfst-special-symbol-fn]
> Warns that an xfst-style special symbol has no special meaning in HFST.
> Special-cases the literal `"all"`: if `strcmp("all", symbol) == 0`, then if
> `verbose_` is true calls `warn("warning: symbol 'all' has no special meaning
> in hfst\n")`, and returns. Otherwise, returns immediately if `symbol[0] !=
> '<'`. Computes `max_index` = index of the last character (start at 1,
> advance to the NUL, decrement once); returns if `max_index < 1`. Returns if
> `symbol[max_index] != '>'`. Returns if `verbose_` is false. Otherwise gets
> the error stream via `xreerrstr()`, writes `warning: '<symbol> ' is an
> ordinary symbol in hfst` (note the space after the symbol before the
> closing quote) followed by `std::endl`, and flushes with `xreflush`.
> Returns void.

> [spec:hfst:def:xre-utils.hfst.xre.warn-fn]
> void

> [spec:hfst:sem:xre-utils.hfst.xre.warn-fn]
> Emits a warning message `msg` to the error stream, gated on verbosity.
> Returns immediately if the module-global `verbose_` is false. Otherwise
> obtains the error stream via `xreerrstr()`, writes `msg` to it (`*err <<
> msg`), and flushes via `xreflush`. Returns void.

> [spec:hfst:def:xre-utils.hfst.xre.xfst-curly-label-to-transducer-fn]
> HfstTransducer *

> [spec:hfst:sem:xre-utils.hfst.xre.xfst-curly-label-to-transducer-fn]
> Builds a transducer from a curly-brace xfst label given as input/output
> C-strings, handling the unknown symbol specially. Three cases:
> (1) If `input` equals `hfst::internal_unknown`: tokenizes `output` into a
> `StringVector sv` via `HfstTokenizer::tokenize_one_level(output, false)`,
> takes `first_token = sv.at(0)`, and creates `retval = new
> HfstTransducer(internal_unknown, first_token, format)`. Then for every token
> in `sv` disjuncts in `HfstTransducer(token, first_token, format)` (with
> harmonize=false); then for every token from the SECOND onward concatenates
> `HfstTransducer(internal_epsilon, token, format)`.
> (2) Else if `output` equals `hfst::internal_unknown`: symmetric — tokenizes
> `input`, `first_token = sv.at(0)`, `retval = new HfstTransducer(first_token,
> internal_unknown, format)`, disjuncts `HfstTransducer(first_token, token,
> format)` for every token, then concatenates `HfstTransducer(token,
> internal_epsilon, format)` for every token from the second onward.
> (3) Else: builds a tokenizer `tok` with `internal_epsilon` added as a
> multichar symbol, and `retval = new HfstTransducer(input, output, tok,
> format)`.
> In all cases calls `retval->minimize()` and returns `retval` (caller owns
> it). Uses `hfst::xre::format` throughout.

> [spec:hfst:def:xre-utils.hfst.xre.xfst-label-to-transducer-fn]
> HfstTransducer *

> [spec:hfst:sem:xre-utils.hfst.xre.xfst-label-to-transducer-fn]
> Builds a transducer from an xfst input:output label, handling definitions
> and the unknown symbol. Computes four flags: `input_is_definition =
> is_definition(input)`, `output_is_definition = is_definition(output)`,
> `input_is_unknown = (input == internal_unknown)`, `output_is_unknown =
> (output == internal_unknown)`.
> If either side is a definition: computes `retval` and a temp `tmp` by
> cross-product. If `input_is_unknown`, `retval = new
> HfstTransducer(internal_identity, format)` and `tmp =
> expand_definition(output)`. Else if `output_is_unknown`, `tmp = new
> HfstTransducer(internal_identity, format)` and `retval =
> expand_definition(input)`. Else `retval = expand_definition(input)` and
> `tmp = expand_definition(output)`. Then `retval->cross_product(*tmp)`,
> `delete tmp`, and returns `retval`.
> Otherwise (no definitions): if both sides unknown, `retval = new
> HfstTransducer(internal_unknown, internal_unknown, format)` disjuncted with
> `HfstTransducer(internal_identity, internal_identity, format)` then
> minimized. Else if only input unknown, `retval = new
> HfstTransducer(internal_unknown, output, format)` disjuncted with
> `HfstTransducer(output, output, format)` then minimized. Else if only
> output unknown, `retval = new HfstTransducer(input, internal_unknown,
> format)` disjuncted with `HfstTransducer(input, input, format)` then
> minimized. Else `retval = new HfstTransducer(input, output, format)`.
> Returns `retval` (caller owns it). All transducers use `hfst::xre::format`.

> [spec:hfst:def:xre-utils.should-colourise-fn]
> static bool

> [spec:hfst:sem:xre-utils.should-colourise-fn]
> Static helper deciding whether terminal colour escape codes should be
> emitted. Calls `isatty(1)` (file descriptor 1, stdout): if it is a TTY
> returns true, otherwise returns false. (A trailing `return false` is
> unreachable.)

> [spec:hfst:def:xre-utils.xre-delete-buffer-fn]
> extern void xre_delete_buffer(YY_BUFFER_STATE, yyscan_t)

> [spec:hfst:sem:xre-utils.xre-delete-buffer-fn]
> External flex-generated function (declared here, defined in the generated
> lexer). Releases the scanner buffer state `YY_BUFFER_STATE` previously
> created by `xre_scan_string`, for the given re-entrant scanner `yyscan_t`.
> Frees the buffer's memory; no return value. Callers invoke it after parsing
> to clean up the buffer paired with the scanner.

> [spec:hfst:def:xre-utils.xre-scan-string-fn]
> extern YY_BUFFER_STATE xre_scan_string(const char *, yyscan_t)

> [spec:hfst:sem:xre-utils.xre-scan-string-fn]
> External flex-generated function (declared here, defined in the generated
> lexer). Sets up the re-entrant scanner `yyscan_t` to read its tokens from
> the given NUL-terminated C string. Allocates and returns a new
> `YY_BUFFER_STATE` representing that input buffer, which the caller must
> later release with `xre_delete_buffer`. After this call the scanner will
> tokenize the supplied string.

> [spec:hfst:def:xre-utils.xreerror-fn]
> int

> [spec:hfst:sem:xre-utils.xreerror-fn]
> Bison error callback (two-argument scanner overload). If the module-global
> `hfst::xre::verbose_` is false, does nothing and returns 0. Otherwise builds
> a diagnostic message into a heap buffer: allocates `malloc(strlen(msg) +
> strlen(hfst::xre::data) + strlen(xreget_text(scanner)) + 100)`. Optionally
> (when `should_colourise()`) prepends the red colour escape, then writes
> `*** xre parsing failed: <msg>\n`, optionally another red escape, then a
> location line: if `strlen(hfst::xre::data) < 60` writes `***    parsing
> <data> [near <scanner text>] on line <lr>\n` (followed by a NUL), else uses
> the same format with `data` right-justified to 60 columns and a trailing
> `...`; the line uses `xreget_text(scanner)` and the module-global line
> counter `hfst::xre::lr`. Optionally appends the reset escape. Then obtains
> the error stream via `xreerrstr()`, writes the buffer string to it, frees
> the buffer, and flushes via `xreflush`. Returns 0.

> [spec:hfst:def:xre-utils.xreerrstr-fn]
> std::ostream *

> [spec:hfst:sem:xre-utils.xreerrstr-fn]
> Returns the output stream to which XRE parser errors/warnings are written.
> Calls and returns `hfst::xre::XreCompiler::get_stream(hfst::xre::error_)`,
> where `error_` is the module-global configured error stream pointer (the
> `get_stream` helper resolves a default if it is null).

> [spec:hfst:def:xre-utils.xreflush-fn]
> void

> [spec:hfst:sem:xre-utils.xreflush-fn]
> Flushes the given output stream `os`. Delegates to
> `hfst::xre::XreCompiler::flush(os)`. Returns void.

> [spec:hfst:def:xre-utils.xreget-text-fn]
> extern char *xreget_text(yyscan_t)

> [spec:hfst:sem:xre-utils.xreget-text-fn]
> External flex-generated accessor (declared here, defined in the generated
> lexer). Returns a pointer to the text of the token most recently matched by
> the re-entrant scanner `yyscan_t` (the flex `yytext`). The returned pointer
> is owned by the scanner and valid only until the next token is matched.

> [spec:hfst:def:xre-utils.xrelex-destroy-fn]
> extern int xrelex_destroy(yyscan_t)

> [spec:hfst:sem:xre-utils.xrelex-destroy-fn]
> External flex-generated function (declared here, defined in the generated
> lexer). Tears down the re-entrant scanner `yyscan_t`, freeing all memory
> associated with it (the counterpart to `xrelex_init`). Returns 0 on
> success. After this call the scanner handle is invalid.

> [spec:hfst:def:xre-utils.xrelex-init-fn]
> extern int xrelex_init(yyscan_t *)

> [spec:hfst:sem:xre-utils.xrelex-init-fn]
> External flex-generated function (declared here, defined in the generated
> lexer). Allocates and initializes a new re-entrant scanner, storing the
> resulting `yyscan_t` handle through the out-parameter pointer. Returns 0 on
> success (non-zero on allocation failure). The handle must later be released
> with `xrelex_destroy`.

> [spec:hfst:def:xre-utils.xreparse-fn]
> extern int xreparse(yyscan_t)

> [spec:hfst:sem:xre-utils.xreparse-fn]
> External bison-generated parser entry point (declared here, defined in the
> generated parser). Drives the XRE grammar over tokens supplied by the
> re-entrant scanner `yyscan_t`, executing the grammar's semantic actions
> (which build the compiled transducer into `last_compiled` and set globals
> such as `contains_only_comments`). Returns 0 if the input parsed
> successfully, non-zero on a parse error (1 for syntax error, 2 for memory
> exhaustion). On error it invokes the `xreerror` callback.

> [spec:hfst:def:xre-utils.yy-buffer-state]
> typedef yy_buffer_state *YY_BUFFER_STATE

> [spec:hfst:def:xre-utils.yyscan-t]
> typedef void *yyscan_t

