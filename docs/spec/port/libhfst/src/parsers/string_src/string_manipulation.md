# libhfst/src/parsers/string_src/string_manipulation.cc, libhfst/src/parsers/string_src/string_manipulation.h

> [spec:hfst:def:string-manipulation.faulty-string-input]
> struct FaultyStringInput {
>   std::string function;
>   std::string input;
> }

> [spec:hfst:def:string-manipulation.faulty-string-input.faulty-string-input-fn]
> FaultyStringInput::FaultyStringInput(const std::string &function,

> [spec:hfst:sem:string-manipulation.faulty-string-input.faulty-string-input-fn]
> Constructor for `FaultyStringInput`. Copies the two arguments into the
> struct's members: the `function` parameter into the `function` member and
> the `input` parameter into the `input` member. No validation, no side
> effects.

> [spec:hfst:def:string-manipulation.main-fn]
> int main(void)

> [spec:hfst:sem:string-manipulation.main-fn]
> Test driver, compiled only when the `STRING_MANIPULATION_TEST` macro is
> defined. It exercises the module's functions via a series of `assert`
> checks (each aborts if false), returning no meaningful value. The checks are:
> - `new_string(0)` is empty; `new_string(1)` has size 1 with byte 0 (also
>   reads index 1, an out-of-bounds peek expected to be 0).
> - `remove_sign` removes every occurrence of a char: "" stays empty; "a"/'a'
>   -> ""; "a"/'b' -> "a"; "fooa"/'a' -> "foo"; "afoo"/'a' -> "foo";
>   "fooafoo"/'a' -> "foofoo"; "fooabar"/'a' -> "foobr".
> - `unquote` on `""`,`"a"`,`"ab"` yields "","a","ab"; and on "", `"`, "a"
>   each throws `FaultyStringInput` (caught and ignored).
> - `unescape` of "","foo","%foo","foo%","%fo%o" -> "","foo","foo","foo","foo";
>   "%%foo" -> "%foo"; "%%foo%%foo%%" -> "%foo%foo%"; "foo\nbar" throws.
> - `strcmp_unescaped`: ("","")==0; ("a","b")<0; ("b","a")>0; ("%a","a")==0;
>   ("a","%a")==0; ("foo%foo","foofoo")==0; ("foo%foo","foo%foo")==0;
>   ("%","")==0; ("foo\nbar","foobar") throws.
> - `str2int`: "0"->0; "-1"->-1; "-1 w"->-1; "-1 3"->-1; "20"->20; "" and "a"
>   each throw.
> - `remove_white_space`: "" empty; "foo"->"foo"; "foo "->"foo"; " foo"->"foo";
>   "foo bar"->"foobar"; "foo bar\tbaz\rfoo"->"foobarbazfoo"; "foo\nbar" throws.
> - `unescape_and_remove_white_space("foo\nbar")` throws.

> [spec:hfst:def:string-manipulation.new-string-fn]
> std::string new_string(size_t lgth)

> [spec:hfst:sem:string-manipulation.new-string-fn]
> Returns a `std::string` of length `lgth` in which every byte is `0` (NUL).
> Constructed via `std::string(lgth, 0)`. No side effects.

> [spec:hfst:def:string-manipulation.print-kill-symbol-fn]
> void print_kill_symbol(void)

> [spec:hfst:sem:string-manipulation.print-kill-symbol-fn]
> Writes to `std::cout`, in order: an end-of-line, the literal string
> `__HFST_TWOLC_DIE`, and another end-of-line (each `std::endl` also flushes
> the stream). Takes no arguments and returns nothing. Side effect: stdout
> output.

> [spec:hfst:def:string-manipulation.relaxed-str-cmp]
> struct relaxed_str_cmp

> [spec:hfst:def:string-manipulation.relaxed-str-cmp.operator-fn]
> bool operator() (const std::string &str1,

> [spec:hfst:sem:string-manipulation.relaxed-str-cmp.operator-fn]
> Const function-call operator providing a strict-weak-ordering comparator.
> Returns `true` iff `strcmp_unescaped(str1, str2) < 0`, i.e. iff `str1`
> sorts before `str2` after both have been passed through `unescape`. Because
> it delegates to `strcmp_unescaped`/`unescape`, it throws `FaultyStringInput`
> if either argument contains a newline.

> [spec:hfst:def:string-manipulation.remove-sign-fn]
> std::string remove_sign(std::string str,char sign)

> [spec:hfst:sem:string-manipulation.remove-sign-fn]
> Returns `str` with every occurrence of the single character `sign` removed.
> Implemented as `replace_substr(str, std::string(1, sign), "")`, i.e. it
> repeatedly replaces the one-character substring with the empty string until
> none remain. (A sibling overload takes `sign` as a `std::string` and calls
> `replace_substr(str, sign, "")` instead.)

> [spec:hfst:def:string-manipulation.remove-white-space-fn]
> std::string remove_white_space(std::string str)

> [spec:hfst:sem:string-manipulation.remove-white-space-fn]
> Removes unescaped whitespace from `str`, preserving escaped whitespace by
> replacing it with placeholder tokens. First, if `str` contains a `\n`, throw
> `FaultyStringInput("remove_white_space", str)`. Otherwise apply four
> sequential passes; each pass uses the same idiom:
> `replace_substr(remove_sign(replace_substr(str, ESCAPED, "\n"), UNESCAPED), "\n", TOKEN)`
> which (a) substitutes the escaped form to a temporary `\n` sentinel, (b)
> removes all remaining unescaped occurrences of the bare whitespace char, then
> (c) turns each sentinel `\n` back into the placeholder token. The four passes,
> in order, handle:
> - space: escaped `"% "`, bare `' '`, token `"__HFST_TWOLC_SPACE"`.
> - tab: escaped `"%\t"`, bare `'\t'`, token `"__HFST_TWOLC_TAB"`.
> - carriage return: escaped `"%\r"`, bare `'\r'`, token `"__HFST_TWOLC_CR"`.
> - literal newline placeholder: escaped `"%__HFST_TWOLC_\n"`, bare string
>   `"__HFST_TWOLC_\n"`, token `"__HFST_TWOLC_\n"`.
> Returns the resulting string.

> [spec:hfst:def:string-manipulation.replace-substr-fn]
> std::string replace_substr(std::string str,const std::string &substr,

> [spec:hfst:sem:string-manipulation.replace-substr-fn]
> Replaces every occurrence of `substr` in `str` with `replacement`, operating
> on a local copy of `str` (passed by value). Loops: find the first occurrence
> of `substr` (via `str.find(substr)`); while one exists (index !=
> `std::string::npos`), replace `substr.size()` characters at that index with
> `replacement`, then search again from the beginning. Continues until `substr`
> no longer occurs, then returns the modified string. Note: replacement
> restarts the search at position 0 each iteration, so if `replacement` itself
> contains `substr` this loops forever (the header documents it as recursive
> substitution until no occurrence remains).

> [spec:hfst:def:string-manipulation.str-cmp]
> struct str_cmp

> [spec:hfst:def:string-manipulation.str-cmp.operator-fn]
> bool operator() (const std::string &str1,

> [spec:hfst:sem:string-manipulation.str-cmp.operator-fn]
> Const function-call operator providing a strict-weak-ordering comparator.
> Returns `true` iff `str1 < str2`, using the standard lexicographic
> (byte-wise) ordering of `std::string`. No side effects, never throws.

> [spec:hfst:def:string-manipulation.str2int-fn]
> int str2int(const std::string &str)

> [spec:hfst:sem:string-manipulation.str2int-fn]
> Parses a leading integer out of `str`. Wraps `str` in a
> `std::istringstream` and extracts one `int` via `in >> number` (this skips
> leading whitespace, reads an optional sign and digits, and stops at the
> first non-numeric character, ignoring any trailing content). If the
> extraction fails (`in.fail()`, e.g. empty string or no leading digits),
> throw `FaultyStringInput("str2int", str)`. Otherwise return the parsed
> integer.

> [spec:hfst:def:string-manipulation.strcmp-unescaped-fn]
> int strcmp_unescaped(const std::string &str1,

> [spec:hfst:sem:string-manipulation.strcmp-unescaped-fn]
> Compares two strings after unescaping. Computes `unescape(str1)` and
> `unescape(str2)` (each may throw `FaultyStringInput` if its argument
> contains a newline), then returns `str1_copy.compare(str2_copy)` — the
> standard `std::string::compare` result: negative if the unescaped `str1`
> sorts before the unescaped `str2`, 0 if equal, positive otherwise.

> [spec:hfst:def:string-manipulation.string-copy-fn]
> char * string_copy(const char * str)

> [spec:hfst:sem:string-manipulation.string-copy-fn]
> Allocates a new NUL-terminated C string that is a copy of `str`. Allocates
> `strlen(str) + 1` bytes via `malloc` (no allocation-failure check), copies
> `str` including its terminating NUL into the buffer with `strcpy`, and
> returns the new buffer pointer (the return value of `strcpy`). Caller owns
> the buffer and must release it with `free`. Behavior is undefined if `str`
> is null.

> [spec:hfst:def:string-manipulation.string-vector]
> class StringVector : public std::vector<std::string> {
>   StringVector &add_values(const StringVector &another);
> }

> [spec:hfst:def:string-manipulation.string-vector.string-vector-fn]
> StringVector::StringVector(const std::string &s)

> [spec:hfst:sem:string-manipulation.string-vector.string-vector-fn]
> Constructs a `StringVector` (a `std::vector<std::string>`) by splitting `s`
> on single space characters. Maintains `start_pos = 0`; repeatedly finds the
> next `' '` at or after `start_pos`: for each space found, pushes the
> substring `[start_pos, space_pos)` and advances `start_pos` to `space_pos +
> 1`. After no more spaces are found, pushes the final substring from
> `start_pos` to end. Consequently the vector always has (number of spaces +
> 1) elements; consecutive spaces and leading/trailing spaces produce empty-
> string tokens. For an empty input `s`, the result is a single empty string.

> [spec:hfst:def:string-manipulation.unescape-and-remove-white-space-fn]
> std::string unescape_and_remove_white_space(std::string str)

> [spec:hfst:sem:string-manipulation.unescape-and-remove-white-space-fn]
> Returns `unescape(remove_white_space(str))`: first applies
> `remove_white_space` to `str` (which converts escaped whitespace to
> placeholder tokens and strips unescaped whitespace), then applies
> `unescape` to that result. Either inner call throws `FaultyStringInput` if
> `str` contains a newline.

> [spec:hfst:def:string-manipulation.unescape-fn]
> std::string unescape(std::string str)

> [spec:hfst:sem:string-manipulation.unescape-fn]
> Removes `%` escaping from `str`. First, if `str` contains a `\n`, throw
> `FaultyStringInput("unescape", str)`. Otherwise perform three nested
> substitutions, innermost first: (1) replace every `"%%"` with a sentinel
> `"\n"`; (2) `remove_sign(..., '%')` removes all remaining `%` characters
> (the unescaped ones); (3) replace every sentinel `"\n"` back with a single
> `"%"`. Net effect: each `%%` becomes a literal `%`, and each lone `%` is
> deleted (equivalent to perl `s/%(%?)/$1/g`). Returns the result.

> [spec:hfst:def:string-manipulation.unescape-name-fn]
> std::string unescape_name(const std::string &name)

> [spec:hfst:sem:string-manipulation.unescape-name-fn]
> Removes hfst-twolc rule-name escaping from `name`. Applies two
> substitutions: first removes every occurrence of the prefix marker
> `"__HFST_TWOLC_RULE_NAME="` (replacing it with the empty string), then
> replaces every `"__HFST_TWOLC_SPACE"` token with a single space `" "`.
> Returns the resulting string. No exceptions.

> [spec:hfst:def:string-manipulation.unquote-fn]
> std::string unquote(const std::string &str)

> [spec:hfst:sem:string-manipulation.unquote-fn]
> Strips one layer of surrounding double quotes from `str`. If `str` has fewer
> than 2 characters, or its first character is not `"`, or its last character
> is not `"`, throw `FaultyStringInput("unquote", str)`. Otherwise return the
> substring spanning from index 1 through the second-to-last character, i.e.
> `str.substr(1, str.size()-2)` (the content between the outer quotes). Only
> the outermost pair of quotes is removed.

