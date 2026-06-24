# libhfst/src/parsers/xfst-utils.cc, libhfst/src/parsers/xfst-utils.h

> [spec:hfst:def:xfst-utils.hfst.xfst.getline-fn]
> ssize_t

> [spec:hfst:sem:xfst-utils.hfst.xfst.getline-fn]
> Fallback `getline` implementation, compiled only when the platform's
> `HAVE_GETLINE` is not defined. Signature `ssize_t getline(char** s, size_t* n, FILE* f)`.
> Allocates a buffer of `*n` bytes via `calloc(sizeof(char), *n)` (zero-initialized)
> and stores its pointer into `*s` (overwriting whatever was there; the caller's prior
> buffer is leaked/ignored). Calls `fgets(*s, (int)*n, f)` reading at most `*n - 1`
> characters into the buffer. If `fgets` returns null (EOF or error before any
> characters), returns -1. Otherwise returns `*n` (the buffer size, not the actual
> number of characters read — note this differs from POSIX getline semantics).
> Does not grow the buffer or update `*n`.

> [spec:hfst:def:xfst-utils.hfst.xfst.nametoken-to-number-fn]
> int nametoken_to_number(const char * token)

> [spec:hfst:sem:xfst-utils.hfst.xfst.nametoken-to-number-fn]
> `int nametoken_to_number(const char* token)`. Constructs a `std::string` from
> `token`, wraps it in a `std::stringstream`, and extracts an `unsigned int` from
> the stream via `str >> x`. If the extraction fails (stream in a failed state —
> i.e. the leading characters are not a valid unsigned integer), returns -1.
> Otherwise returns the parsed value cast to `int`. Note: parsing stops at the
> first non-numeric character and trailing junk after a valid number does not
> cause failure (only the value before it is read).

> [spec:hfst:def:xfst-utils.hfst.xfst.strdup-nonconst-part-fn]
> char*

> [spec:hfst:sem:xfst-utils.hfst.xfst.strdup-nonconst-part-fn]
> `char* strdup_nonconst_part(const char* token, const char* prefix, const char* suffix, bool strip)`.
> Extracts and heap-duplicates the "variable" middle portion of `token` that lies
> between a known `prefix` and `suffix`. Steps: let `token_len = strlen(token)`.
> Allocate `token_part` of `token_len + 1` bytes. Compute `prefix_len` = `strlen(prefix)`
> if `prefix` is non-null else 0, and `suffix_len` = `strlen(suffix)` if `suffix`
> non-null else 0. Compute `varpart_len = strlen(token) - prefix_len - suffix_len`.
> Asserts (debug builds): `varpart_len <= token_len`; `prefix != NULL`; `token`
> starts with `prefix` (`strncmp(token, prefix, prefix_len) == 0`); `suffix != NULL`;
> and the region of length `suffix_len` at offset `prefix_len + varpart_len` equals
> `suffix`. Copies `varpart_len` bytes starting at `token + prefix_len` into
> `token_part` and null-terminates at `[varpart_len]`. If `strip` is true, runs
> `strstrip` on `token_part`, frees the original `token_part`, and replaces it with
> the stripped result. Returns the (caller-owned) `token_part`.

> [spec:hfst:def:xfst-utils.hfst.xfst.strdup-token-part-fn]
> char*

> [spec:hfst:sem:xfst-utils.hfst.xfst.strdup-token-part-fn]
> `char* strdup_token_part()`. Builds a heap-allocated diagnostic string describing
> the text near the current flex token, read from the global `hxfsttext` (current
> matched lexer text). Allocates `error_token` of `strlen(hxfsttext)*1 + 100` bytes.
> Searches `hxfsttext` for the first newline via `strchr`. Three cases:
> (1) If a newline is found, allocates `beforelbr` of `strlen(hxfsttext)+1` bytes,
> copies the bytes before the newline into it (length `maybelbr - hxfsttext`),
> null-terminates, formats `error_token` as `[near: `<beforelbr>\n']` via sprintf,
> then frees `beforelbr`.
> (2) Else if `strlen(hxfsttext) < 80`, formats `[near: `<hxfsttext>']`.
> (3) Else formats `[near: `%30s...' (truncated)]` with `hxfsttext` (the `%30s`
> right-pads/min-width 30 but does not truncate, so the full text is still printed
> followed by the literal `...' (truncated)]`).
> Returns the caller-owned `error_token` buffer.

> [spec:hfst:def:xfst-utils.hfst.xfst.strstrip-fn]
> char*

> [spec:hfst:sem:xfst-utils.hfst.xfst.strstrip-fn]
> `char* strstrip(const char* s)`. Returns a heap-allocated copy of `s` with
> leading and trailing whitespace removed. Allocates `rv` of `strlen(s)+1` bytes.
> Special case: if `s` is the empty string, writes `'\0'` to `rv[0]` and returns it.
> Otherwise: advance `s` past all leading whitespace (`isspace`). Copy the remaining
> bytes of `s` into `rv` (pointer `p`), then null-terminate at `p`. Step `p` back
> one position (to the last copied char) and, while that char is whitespace, set it
> to `'\0'` and step `p` back further — trimming trailing whitespace in place.
> Returns the caller-owned `rv`. Note: if the input is non-empty but all-whitespace,
> the trailing-trim loop walks `p` backward and may read/write before the start of
> the buffer (undefined behavior in the original); a Rust port should guard against
> that by stopping at the buffer start.

> [spec:hfst:def:xfst-utils.ssize-t]
> typedef SSIZE_T ssize_t

