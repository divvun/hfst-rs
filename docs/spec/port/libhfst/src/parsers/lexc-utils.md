# libhfst/src/parsers/lexc-utils.cc

> [spec:hfst:def:lexc-utils.hfst.lexc.count-newlines-fn]
> static size_t

> [spec:hfst:sem:lexc-utils.hfst.lexc.count-newlines-fn]
> File-local static helper. Takes a NUL-terminated C string `linestring`.
> Initializes a counter to 0, walks every byte from the start until the
> terminating `'\0'`, incrementing the counter for each byte equal to `'\n'`.
> Returns the total number of `'\n'` bytes as a `size_t`. No mutation of input,
> no allocation, no side effects.

> [spec:hfst:def:lexc-utils.hfst.lexc.error-at-current-token-fn]
> void

> [spec:hfst:sem:lexc-utils.hfst.lexc.error-at-current-token-fn]
> Signature `error_at_current_token(int, int, const char *format)`; the two int
> parameters are unnamed and unused. Builds an error message about the current
> flex token and writes it to the lexc compiler's error stream.
> Steps: (1) `leader = strdup_token_positions()` (heap "filename:line.col"
> location string). (2) `token = strdup_token_part()` (heap "[near: `...']"
> string). (3) Obtain the output `std::ostream *err` via
> `hfst::lexc::lexc_->get_stream(hfst::lexc::lexc_->get_error_stream())`.
> (4) If `should_colourise()` is true, write the ANSI bold escape
> `"\033[01m"` to `err`. (5) Write `leader` followed by `": "`. (6) If
> colourising, write the ANSI red escape `"\033[31m"`. (7) Write `format`
> followed by `": "`. (8) If colourising, write the ANSI reset escape
> `"\033[0m"`. (9) Write `token` followed by `std::endl`. (10) Call
> `lexc_->flush(err)`. (11) Free `leader`. Note: `token` is NOT freed (the
> heap buffer from `strdup_token_part` is leaked). Returns void. Reads global
> compiler state via `lexc_`; reads the globals consulted by
> `strdup_token_positions`/`strdup_token_part`.

> [spec:hfst:def:lexc-utils.hfst.lexc.find-med-alingment-fn]
> pair<vector<string>, vector<string> >

> [spec:hfst:sem:lexc-utils.hfst.lexc.find-med-alingment-fn]
> Computes a minimum-edit-distance (MED) alignment between two symbol sequences
> `s1` and `s2` (each a `vector<string>`) and returns the aligned pair of
> sequences with epsilon symbols inserted where symbols are deleted/inserted.
> Costs: substitution = 100, deletion = 1, insertion = 1 (high substitution
> cost biases toward del+ins, i.e. true substitutions only when symbols match).
> Steps:
> (1) Let len1 = s1.size(), len2 = s2.size(). Allocate two (len1+1)×(len2+1)
> matrices `d` (unsigned int costs) and `dir` (unsigned int direction codes).
> (2) Init d[0][0]=0, dir[0][0]=0. For i in 1..=len1: d[i][0]=deletion*i,
> dir[i][0]=DELETE. For i in 1..=len2: d[0][i]=insertion*i, dir[0][i]=INSERT.
> (3) Fill row-major for i in 1..=len1, j in 1..=len2:
>     sub = d[i-1][j-1] + (s1[i-1]==s2[j-1] ? 0 : 100);
>     ins = d[i][j-1] + 1; del = d[i-1][j] + 1.
>     If sub<=ins && sub<=del: d[i][j]=sub, dir=SUBSTITUTE.
>     Else if del<=sub && del<=ins: d[i][j]=del, dir=DELETE.
>     Else: d[i][j]=ins, dir=INSERT.
>     (DELETE is checked before INSERT on ties so the first string gets zeroes
>     earlier.)
> (4) Backtrace: start x=s1.size(), y=s2.size(); loop while x>0 || y>0, reading
>     dir[x][y]. SUBSTITUTE: push s1[x-1] to medcwordin, s2[y-1] to medcwordout,
>     x--,y--. INSERT: push EPSILON_ to medcwordin, s2[y-1] to medcwordout, y--.
>     Otherwise (DELETE): push s1[x-1] to medcwordin, EPSILON_ to medcwordout,
>     x--. (EPSILON_ is the internal epsilon symbol constant.)
> (5) std::reverse both medcwordin and medcwordout.
> (6) Return pair(medcwordin, medcwordout). No I/O, no exceptions; uses
>     `hfst::size_t_to_int` for the loop bounds.

> [spec:hfst:def:lexc-utils.hfst.lexc.replace-zero-fn]
> string

> [spec:hfst:sem:lexc-utils.hfst.lexc.replace-zero-fn]
> Takes a `string s` by value. Copies it into local `str`. Searches for the
> FIRST occurrence of the literal substring `"@ZERO@"` via `str.find`. If found
> (position != npos), replaces that single 6-character occurrence with `"0"`.
> Only the first occurrence is replaced. Returns the (possibly modified) string
> by value. No side effects.

> [spec:hfst:def:lexc-utils.hfst.lexc.set-infile-name-fn]
> void

> [spec:hfst:sem:lexc-utils.hfst.lexc.set-infile-name-fn]
> Sets the file-local static global `hlexcfilename`. Frees the current
> `hlexcfilename` pointer (free(NULL) is a no-op if it was 0), then assigns it a
> freshly heap-allocated duplicate of the input C string `s` via `strdup(s)`.
> Returns void. Side effect: mutates the static `hlexcfilename`.

> [spec:hfst:def:lexc-utils.hfst.lexc.should-colourise-fn]
> static bool

> [spec:hfst:sem:lexc-utils.hfst.lexc.should-colourise-fn]
> File-local static, no parameters. Returns true if file descriptor 1 (stdout)
> is a terminal, i.e. `isatty(1)` is nonzero; otherwise returns false. No side
> effects.

> [spec:hfst:def:lexc-utils.hfst.lexc.strdup-nonconst-part-fn]
> char *

> [spec:hfst:sem:lexc-utils.hfst.lexc.strdup-nonconst-part-fn]
> Extracts the "variable" middle of `token` after stripping a known `prefix`
> and `suffix`, returning a heap-allocated NUL-terminated copy.
> Params: `token`, `prefix` (may be NULL), `suffix` (may be NULL), `strip` bool.
> Steps:
> (1) token_len = strlen(token). Allocate `token_part` of size token_len+1.
> (2) prefix_len = prefix ? strlen(prefix) : 0; suffix_len = suffix ?
>     strlen(suffix) : 0. varpart_len = token_len - prefix_len - suffix_len.
>     assert(varpart_len <= token_len).
> (3) Assertions verifying the prefix/suffix actually match: if prefix==NULL,
>     assert(strncmp(token,"",0)==0); else assert token starts with prefix.
>     Similarly assert token+prefix_len+varpart_len matches suffix (or "" when
>     NULL). (These are debug asserts.)
> (4) memcpy varpart_len bytes from token+prefix_len into token_part, then set
>     token_part[varpart_len]='\0'.
> (5) If `strip`: call strstrip(token_part) into a new buffer `tmp`, free the
>     old token_part, replace it with tmp.
> (6) Return token_part (heap-allocated; caller owns/frees).

> [spec:hfst:def:lexc-utils.hfst.lexc.strdup-token-part-fn]
> char *

> [spec:hfst:sem:lexc-utils.hfst.lexc.strdup-token-part-fn]
> No params. Builds a heap-allocated "[near: ...]" string from the current flex
> token text in global `hlexctext`.
> Steps:
> (1) Allocate `error_token` of size strlen(hlexctext)+100.
> (2) maybelbr = strchr(hlexctext, '\n').
> (3) If a newline was found: allocate `beforelbr` of size strlen(hlexctext)+1,
>     memcpy the bytes before the newline (maybelbr - hlexctext bytes) into it,
>     NUL-terminate at that length, sprintf into error_token the format
>     "[near: `%s\\n']" with beforelbr, then free beforelbr.
> (4) Else if strlen(hlexctext) < 80: sprintf "[near: `%s']" with hlexctext.
> (5) Else: sprintf "[near: `%30s...' (truncated)]" with hlexctext (the %30s
>     pads to a min width of 30 but does not truncate the value; printf prints
>     the whole string).
> (6) Return error_token (heap-allocated; caller owns/frees). Reads global
>     `hlexctext`.

> [spec:hfst:def:lexc-utils.hfst.lexc.strdup-token-positions-fn]
> char *

> [spec:hfst:sem:lexc-utils.hfst.lexc.strdup-token-positions-fn]
> No params. Formats the current source location from global flex location
> `hlexclloc` and global `hlexcfilename` into a heap "filename:line.col[...]"
> string (GNU error-message format for editor integration).
> Steps:
> (1) Allocate `filenames_lines_cols` of size strlen(hlexcfilename)+100.
> (2) If first_line == last_line AND first_column == last_column-1 (a single
>     position): sprintf "%s:%d.%d" with hlexcfilename, first_line,
>     first_column.
> (3) Else if first_line == last_line (same line, column range): sprintf
>     "%s:%d.%d-%d" with hlexcfilename, first_line, first_column, last_column.
> (4) Else (spans lines): sprintf "%s:%d.%d-%d.%d" with hlexcfilename,
>     first_line, first_column, last_line, last_column.
> (5) Return the buffer (heap-allocated; caller owns/frees). Reads globals
>     `hlexclloc` and `hlexcfilename`.

> [spec:hfst:def:lexc-utils.hfst.lexc.strip-percents-fn]
> char *

> [spec:hfst:sem:lexc-utils.hfst.lexc.strip-percents-fn]
> Params: `s` (input C string), `do_zeros` bool. Processes lexc escape syntax,
> producing a heap-allocated NUL-terminated result `rv`, or NULL on a trailing
> stray escape. Returns the result.
> Setup: obtains an error `std::ostream *err` from
> `lexc_->get_stream(lexc_->get_error_stream())` (computed but only used
> indirectly via the warning/error helpers). Allocates `rv`: if do_zeros, size
> strlen(s)*strlen("@0@")+1; else size ((strlen(s)/2)+1)*strlen("@ZERO@")+1.
> `p` is the write cursor into rv; `c` reads s. Two state flags: `escaping`,
> `in_at`, both start false.
> Main loop over each input char `*c` until '\0':
> - If `in_at` (inside an `@...@` symbol): copy `*c` to output; if `*c=='@'`
>   clear in_at; advance both.
> - Else if `escaping` (previous char was '%'): if `*c != '0'`: when `*c` is NOT
>   one of the allowed escapables (`: < space ; % " @ ! > #`), build a warning
>   message "Unnecessary escape %<c> [-Wunnecessary-escapes]" (using "%%%c" for
>   `*c>0`, else "%%%s" with `c` for non-positive bytes); if that warning is
>   enabled AND warnings-as-errors, call error_at_current_token(0,0,msg) and set
>   lexc_->parseErrors_=true; else if enabled, call
>   warning_at_current_token(0,0,msg). Then write `*c` to output regardless.
>   If `*c == '0'`: instead write the literal expansion "@ZERO@" to output. In
>   all escaping cases clear escaping and advance c.
> - Else if `*c == '%'`: set escaping=true, advance c (do not output).
> - Else if `*c == '@'`: set in_at=true, copy '@' to output, advance both.
> - Else if `do_zeros && *c == '0'`: write literal "@0@" to output, advance c.
> - Else: copy `*c` to output, advance both.
> After the loop: write terminating '\0' to *p. If `escaping` is still set (the
> string ended on a bare '%'): call warning_at_current_token(0,0,"Stray escape
> char %%\n") and return NULL (rv is leaked). Otherwise return rv.
> Note: the unnecessary-escape errmsg buffer is allocated but never freed.
> Reads global compiler `lexc_`.

> [spec:hfst:def:lexc-utils.hfst.lexc.strstrip-fn]
> char *

> [spec:hfst:sem:lexc-utils.hfst.lexc.strstrip-fn]
> Param: `s` (input C string). Returns a heap-allocated copy of `s` with leading
> and trailing whitespace removed.
> Steps:
> (1) Allocate `rv` of size strlen(s)+1.
> (2) Special case: if `*s == '\0'` (empty input), set *rv='\0' and return rv.
> (3) Cursor `p` = rv. Advance `s` past leading whitespace
>     (while isspace(*s) ++s).
> (4) Copy remaining bytes from s to rv (while *s != '\0': *p++ = *s++), then
>     write '\0' at *p.
> (5) Trim trailing whitespace: decrement p to the last copied char, then while
>     isspace(*p): set *p='\0' and decrement p.
> (6) Return rv (heap-allocated; caller owns/frees). Note: isspace is called on
>     raw (possibly signed) char values as in the source.

> [spec:hfst:def:lexc-utils.hfst.lexc.token-reset-positions-fn]
> void

> [spec:hfst:sem:lexc-utils.hfst.lexc.token-reset-positions-fn]
> No params. Resets the flex location/line globals to start-of-file state.
> Sets hlexclloc.first_line = hlexclloc.last_line = 1; hlexclloc.first_column =
> hlexclloc.last_column = 1; global hlexclineno = 1. If global hlexcfilename is
> non-null, free it; then set hlexcfilename = 0. Returns void. Side effects:
> mutates globals hlexclloc, hlexclineno, hlexcfilename.

> [spec:hfst:def:lexc-utils.hfst.lexc.token-update-positions-fn]
> void

> [spec:hfst:sem:lexc-utils.hfst.lexc.token-update-positions-fn]
> Param: `token` (the just-matched flex token C string). Advances the flex
> location global `hlexclloc` to span this token.
> Steps:
> (1) token_length = strlen(token). newlines =
>     hfst::size_t_to_int(count_newlines(token)).
> (2) hlexclloc.first_line = hlexclloc.last_line (start where the previous
>     token ended). hlexclloc.last_line = first_line + newlines.
> (3) hlexclloc.first_column = hlexclloc.last_column + 1.
> (4) If newlines == 0: hlexclloc.last_column = first_column +
>     size_t_to_int(token_length) (columns count bytes, not characters).
> (5) Else: find token_last_line_start = strrchr(token, '\n') and token_end =
>     strrchr(token, '\0') (pointer to the terminating NUL); set
>     hlexclloc.last_column = (token_end - token_last_line_start) - 1.
> Returns void. Side effect: mutates global hlexclloc. Calls count_newlines.

> [spec:hfst:def:lexc-utils.hfst.lexc.warning-at-current-token-fn]
> void

> [spec:hfst:sem:lexc-utils.hfst.lexc.warning-at-current-token-fn]
> Signature `warning_at_current_token(int, int, const char *format)`; the two
> int parameters are unnamed and unused. Identical in structure to
> error_at_current_token but uses the yellow colour instead of red.
> Steps: (1) leader = strdup_token_positions(); token = strdup_token_part().
> (2) Get err stream via lexc_->get_stream(lexc_->get_error_stream()).
> (3) If should_colourise(), write bold escape "\033[01m". (4) Write leader then
> ": ". (5) If colourising, write yellow escape "\033[33m". (6) Write `format`
> then ": ". (7) If colourising, write reset "\033[0m". (8) Write token then
> std::endl. (9) lexc_->flush(err). (10) Free leader. Note: `token` is NOT
> freed (leaked). Returns void. Reads global compiler state `lexc_`.

