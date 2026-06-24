# libhfst/src/parsers/xfst_help_message.cc

> [spec:hfst:def:xfst-help-message.hfst.xfst.append-help-message-fn]
> void append_help_message(const std::string & namelist, const std::string & arguments,

> [spec:hfst:sem:xfst-help-message.hfst.xfst.append-help-message-fn]
> Append a formatted help line for one command to `message`. Parameters: `namelist`
> (comma-separated names), `arguments`, `description`, the in/out `message` string,
> and `all_names` (defaults to true).
> Steps:
> 1. Define a constant `NAME_AND_ARGUMENTS_FIELD_WIDTH = 30`.
> 2. Split `namelist` into a name vector via `namelist_to_name_vector`.
> 3. Append to `message`: the first name, a single space, then `arguments`.
> 4. Compute `name_and_arguments_length = first_name.length() + 1 + arguments.length()`.
> 5. If that length is greater than 30, append exactly one space; otherwise append
>    `(30 - name_and_arguments_length)` spaces (padding to column width 30).
> 6. Append `description` followed by a newline `"\n"`.
> 7. If `all_names` is true AND there is more than one name: append `"("`, then for
>    every name in the vector starting from the second one, separate them with `", "`
>    and append each name (the first name in the vector is skipped for output via the
>    `continue` on the begin iterator; the separator is added for every name that is
>    not the second name). Finally append `")\n"`.
> Returns nothing (void); only mutates `message`.

> [spec:hfst:def:xfst-help-message.hfst.xfst.get-help-message-fn]
> bool get_help_message(const std::string & text, std::string & message, int help_mode,

> [spec:hfst:sem:xfst-help-message.hfst.xfst.get-help-message-fn]
> Build help message(s) for command(s) named `text` and append them to `message`.
> Parameters: `text`, in/out `message`, `help_mode` (0=ONE_COMMAND, 1=ALL_COMMANDS,
> 2=APROPOS), and `skip_ambiguous_cases` (defaults to false). Returns whether any
> help text was appended.
> Steps:
> 1. If `help_mode == HELP_MODE_APROPOS (2)` and `text` is empty, recurse:
>    `return get_help_message("apropos", message, HELP_MODE_ONE_COMMAND)` (i.e. return
>    the help for the apropos command itself).
> 2. Save a copy `message_at_start = message` to detect whether anything is appended.
> 3. Execute a long fixed sequence of command entries built from three macros:
>    - `COMMAND(names, args, description)`: calls
>      `handle_case(names, args, description, text, message, help_mode, true)`; if it
>      returns true, this function returns true immediately.
>    - `CONT_COMMAND(names, args, description)`: calls
>      `handle_case(names, args, description, text, message, help_mode, false)` and
>      ignores the result (never early-returns).
>    - `AMBIGUOUS_COMMAND(name, namelist)`: if `!skip_ambiguous_cases` and
>      `handle_ambiguous_case(name, namelist, text, message, help_mode)` returns true,
>      this function returns true immediately.
>    The full ordered list of entries (names, arguments, descriptions) is the literal
>    sequence in the C++ source from "ambiguous upper, ambiguous" through "variable
>    att-epsilon", including the AMBIGUOUS_COMMAND entries for apply, help, load, print,
>    write, show, sort, substitute, test, variable. Lines commented out in the source
>    (e.g. `//COMMAND(...)`) are NOT emitted. On `_WIN32` the "print directory" COMMAND
>    is omitted (guarded by `#ifndef _WIN32`).
> 4. After the whole list, return `(message != message_at_start)` — true if at least
>    one help message was appended, false otherwise.

> [spec:hfst:def:xfst-help-message.hfst.xfst.handle-ambiguous-case-fn]
> bool handle_ambiguous_case(const std::string & name, const std::string & namelist,

> [spec:hfst:sem:xfst-help-message.hfst.xfst.handle-ambiguous-case-fn]
> Handle a command name that is ambiguous (maps to several sub-commands). Parameters:
> `name` (the ambiguous command name), `namelist` (comma-separated list of the matching
> sub-command names), `text` (the query), in/out `message`, and `help_mode`. Returns
> whether the search should stop (true) or continue (false).
> Steps:
> 1. If `help_mode` is HELP_MODE_ALL_COMMANDS (1) or HELP_MODE_APROPOS (2), return false
>    (do nothing; all commands are being iterated anyway).
> 2. Otherwise (HELP_MODE_ONE_COMMAND, 0): if `name != text`, return false.
> 3. If `name == text`: split `namelist` into a name vector via `namelist_to_name_vector`.
>    For each name in the vector: if it is not the first one, append the separator
>    `"##\n"` to `message`; then call `get_help_message(name, message, help_mode, true)`
>    (the boolean result is discarded; the `true` argument sets `skip_ambiguous_cases`
>    to avoid infinite recursion).
> 4. Return true (the ambiguous command was handled, stop searching).

> [spec:hfst:def:xfst-help-message.hfst.xfst.handle-case-fn]
> bool handle_case(const std::string & names, const std::string & arguments,

> [spec:hfst:sem:xfst-help-message.hfst.xfst.handle-case-fn]
> Decide, per `help_mode`, whether one command entry matches `text` and, if so, append
> its help message. Parameters: `names` (comma-separated), `arguments`, `description`,
> `text` (the query), in/out `message`, `help_mode`, and `all_names` (defaults to true).
> Returns whether the caller should treat the command as found and stop searching.
> Steps:
> 1. If `help_mode == HELP_MODE_ALL_COMMANDS (1)`: always call
>    `append_help_message(names, arguments, description, message, all_names)` and return
>    false (continue through all commands).
> 2. Else if `help_mode == HELP_MODE_APROPOS (2)`: if `word_found_in_text(text, names)`
>    OR `word_found_in_text(text, description)` is true, call
>    `append_help_message(...)`. Then return false (continue searching).
> 3. Else (HELP_MODE_ONE_COMMAND, 0): if `text_matches_some_name(text, names)` is true,
>    call `append_help_message(...)` and return true (command found, stop). Otherwise
>    return false (continue searching).

> [spec:hfst:def:xfst-help-message.hfst.xfst.is-punctuation-char-fn]
> bool is_punctuation_char(char c)

> [spec:hfst:sem:xfst-help-message.hfst.xfst.is-punctuation-char-fn]
> Return whether the byte `c` is one of the punctuation/whitespace characters in the
> literal set `" \n\t.,;:?!-/'\"<>()|"` (space, newline, tab, period, comma, semicolon,
> colon, question mark, exclamation, hyphen, slash, single quote, double quote,
> less-than, greater-than, open paren, close paren, pipe). Iterates over that set and
> returns true on a match, false if none match.

> [spec:hfst:def:xfst-help-message.hfst.xfst.namelist-to-name-vector-fn]
> StringVector namelist_to_name_vector(const std::string & namelist)

> [spec:hfst:sem:xfst-help-message.hfst.xfst.namelist-to-name-vector-fn]
> Split a comma-separated `namelist` into a vector of names. Returns the vector.
> Steps:
> 1. Start with an empty result vector `names` and `pos = 0` (start index of the current
>    name).
> 2. Scan `namelist` index by index `i` from 0 to length-1:
>    - When a comma `','` is found at index `i`: push the substring `namelist[pos..i)`
>      (length `i-pos`) onto `names`. Then advance `i` past the comma (`i++`) and keep
>      incrementing `i` while `namelist[i] == ' '` (skip spaces following the comma).
>      Set `pos = i` (next name starts here), then decrement `i` by one because the for
>      loop will increment it again.
> 3. After the loop, push the final substring from `pos` to the end of the string.
> 4. Return `names`. Note: each comma yields a split, leading spaces after a comma are
>    trimmed, but spaces are not trimmed from the start of the first name or generally
>    otherwise; the last name is always added even if empty.

> [spec:hfst:def:xfst-help-message.hfst.xfst.string-vector]
> typedef std::vector<std::string> StringVector

> [spec:hfst:def:xfst-help-message.hfst.xfst.text-matches-some-name-fn]
> bool text_matches_some_name(const std::string & text, const std::string & namelist)

> [spec:hfst:sem:xfst-help-message.hfst.xfst.text-matches-some-name-fn]
> Return whether `text` exactly equals one of the names in the comma-separated
> `namelist`. Splits `namelist` via `namelist_to_name_vector`, then iterates the
> resulting names; returns true on the first name that is exactly equal (`==`) to
> `text`, false if no name matches. Comparison is exact string equality (case-sensitive).

> [spec:hfst:def:xfst-help-message.hfst.xfst.to-upper-case-fn]
> std::string to_upper_case(const std::string & str)

> [spec:hfst:sem:xfst-help-message.hfst.xfst.to-upper-case-fn]
> Return an ASCII upper-cased copy of `str`. Builds a new string by iterating each byte:
> if the byte value is in the inclusive range 97..122 (ASCII lowercase 'a'..'z'),
> append `byte - 32` (the uppercase equivalent); otherwise append the byte unchanged.
> Only ASCII lowercase letters are converted; all other bytes (including non-ASCII /
> multibyte UTF-8) are passed through verbatim.

> [spec:hfst:def:xfst-help-message.hfst.xfst.word-found-in-text-fn]
> bool word_found_in_text(const std::string & str_, const std::string & text_)

> [spec:hfst:sem:xfst-help-message.hfst.xfst.word-found-in-text-fn]
> Return whether the word `str_` occurs in `text_` as a whole word (case-insensitive,
> delimited by punctuation/whitespace or string boundaries).
> Steps:
> 1. Upper-case both arguments via `to_upper_case` into `str` and `text`.
> 2. Find the first occurrence position `pos` of `str` within `text` (substring search).
>    If not found (`npos`), return false.
> 3. Check the character immediately before the match: the left boundary is valid if
>    `pos == 0` (match at start) OR `text[pos-1]` is a punctuation char
>    (`is_punctuation_char`).
> 4. If the left boundary is valid, check the right boundary: it is valid if the match
>    ends at the end of text (`pos + str.length() == text.length()`) OR
>    `text[pos + str.length()]` is a punctuation char.
> 5. If both boundaries are valid, return true; otherwise return false. Note: only the
>    first occurrence is tested — if the first match fails the boundary check, the
>    function returns false without searching for later occurrences.

