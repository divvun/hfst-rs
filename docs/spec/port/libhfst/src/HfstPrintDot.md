# libhfst/src/HfstPrintDot.cc

> [spec:hfst:def:hfst-print-dot.hfst.c99-snprintf-fn]
> __inline int c99_snprintf(char *outBuf, size_t size, const char *format, ...)

> [spec:hfst:sem:hfst-print-dot.hfst.c99-snprintf-fn]
> Windows-only (MSVC before VC++ 2015) replacement for the C99 `snprintf`,
> compiled only when `_MSC_VER` is defined and `< 1900`; a `#define snprintf
> c99_snprintf` redirects all `snprintf` calls in this file to it. It declares
> an `int count` and a `va_list ap`, starts the variadic argument list with
> `va_start(ap, format)`, delegates to `c99_vsnprintf(outBuf, size, format, ap)`
> storing its return in `count`, ends the list with `va_end(ap)`, and returns
> `count` (the number of characters that would have been written, per C99
> semantics). Parameters: `outBuf` output buffer, `size` its capacity, `format`
> the printf format string, followed by the format arguments.

> [spec:hfst:def:hfst-print-dot.hfst.c99-vsnprintf-fn]
> __inline int c99_vsnprintf(char *outBuf, size_t size, const char *format, va_list ap)

> [spec:hfst:sem:hfst-print-dot.hfst.c99-vsnprintf-fn]
> Windows-only (MSVC before VC++ 2015) replacement for the C99 `vsnprintf`,
> compiled only when `_MSC_VER` is defined and `< 1900`; a `#define vsnprintf
> c99_vsnprintf` redirects all `vsnprintf` calls in this file to it. Initializes
> `int count = -1`. If `size != 0`, calls `_vsnprintf_s(outBuf, size, _TRUNCATE,
> format, ap)` and stores its result in `count` (this writes the formatted output
> into `outBuf`, truncating to fit `size`). If `count == -1` (truncation occurred,
> or `size` was 0), recomputes `count = _vscprintf(format, ap)` to obtain the
> number of characters the full formatted string would require. Returns `count`.
> Parameters: `outBuf` output buffer, `size` its capacity, `format` printf format
> string, `ap` the already-started `va_list` of arguments.

> [spec:hfst:def:hfst-print-dot.hfst.print-dot-fn]
> void

> [spec:hfst:sem:hfst-print-dot.hfst.print-dot-fn]
> Writes a Graphviz DOT representation of transducer `t` to output `out`. There
> are two overloads with identical structure: one writing to a C `FILE* out`
> (via `fprintf`), and one writing to a `std::ostream& out` (via `<<`); the
> ostream version first sets `out.precision(2)`. Steps:
> 1. Header: if `t.get_name()` is non-empty, emit `digraph "<name>" {`; else
>    emit `digraph H {`. Then emit three lines: `charset = UTF8;`,
>    `rankdir = LR;`, and `node [shape=circle,style=filled,fillcolor=yellow]`.
> 2. Construct a mutable basic transducer `mutt {t}` (an `HfstBasicTransducer`
>    copy of `t`). Set state counter `s = 0`.
> 3. First pass — emit all nodes before any arcs. Iterate over `mutt`'s states
>    (range-for); for each, using the current counter `s`: if `mutt.is_final_state(s)`
>    and `mutt.get_final_weight(s) > 0`, emit `q<s> [shape=doublecircle,label="q<s>/\n<w>"]`
>    with weight formatted to two decimals; if final with weight not > 0, emit
>    `q<s> [shape=doublecircle,label="q<s>"]`; otherwise (non-final) emit
>    `q<s> [label="q<s>"]`. Increment `s` after each state. (Exact spacing/newlines
>    differ slightly between the FILE* and ostream overloads but the content is
>    equivalent.)
> 4. Reset `s = 0`. Second pass — emit arcs. Iterate states again; for each state
>    keep a `std::map<HfstState,std::string> target_labels` mapping each target
>    state to an accumulated edge label. For each arc of the state:
>    - Read `old_label = target_labels[target]` (default empty for first arc to a
>      target — note `operator[]` inserts an empty entry).
>    - Read `first = arc.get_input_symbol()`, `second = arc.get_output_symbol()`.
>    - Symbol remapping: if `first` equals `internal_epsilon` -> "00";
>      `internal_identity` -> "??"; `internal_unknown` -> "?1". Independently, if
>      `second` equals `internal_epsilon` -> "00"; `internal_identity` -> "??";
>      `internal_unknown` -> "?2".
>    - Allocate a fixed `DOT_MAX_LABEL_SIZE` (= 64) byte buffer `l` with `malloc`.
>    - Format into `l` via `snprintf`, choosing among eight cases on two booleans
>      (`first == second`, i.e. identity pair) and (`arc.get_weight() > 0`),
>      crossed with whether `old_label` is empty. Identity pair with weight and
>      non-empty old: `"%s, %s/%.2f"` (old_label, first, weight); identity+weight
>      empty old: `"%s/%.2f"` (first, weight); identity no-weight non-empty old:
>      `"%s, %s"` (old_label, first); identity no-weight empty old: `"%s"` (first).
>      Non-identity pair with weight non-empty old: `"%s, %s:%s/%.2f"` (old_label,
>      first, second, weight); +weight empty old: `"%s:%s/%.2f"` (first, second,
>      weight); no-weight non-empty old: `"%s, %s:%s"` (old_label, first, second);
>      no-weight empty old: `"%s:%s"` (first, second). If any `snprintf` returns
>      `< 0`, throw `HfstException` with message "sprinting dot arc label".
>    - Call `trim_to_valid_utf8(l)` to drop any partial trailing UTF-8 byte caused
>      by the 64-byte truncation. Construct `std::string sl(l)`, then
>      `replace_all(sl, "\"", "\\\"")` to escape double quotes. Store
>      `target_labels[target] = sl`. `free(l)`.
>    After processing all arcs of the state, iterate `target_labels` in key order
>    (map ordering) and for each `(target, label)` emit
>    `q<s> -> q<target> [label="<label> "];` (note the trailing space inside the
>    label string and the literal `;`). Increment `s`.
> 5. Emit closing `}`. Returns void. Side effects: writes to `out`, allocates and
>    frees per-arc buffers, may throw `HfstException`.

> [spec:hfst:def:hfst-print-dot.hfst.trim-to-valid-utf8-fn]
> void

> [spec:hfst:sem:hfst-print-dot.hfst.trim-to-valid-utf8-fn]
> Truncates a NUL-terminated C string `inp` in place so it does not end in the
> middle of a multi-byte UTF-8 sequence, by chopping off a trailing incomplete
> leading byte. Computes `len = strlen(inp)`. Loops `i` from 1 to 3 inclusive,
> continuing only while `len - i > 0` (note: `len` is `size_t`, so the bytes
> examined are `inp[len-1]`, `inp[len-2]`, `inp[len-3]`, i.e. the last three
> bytes). For each `i` it tests the byte `inp[len-i]`: if `i < 2` (only `i==1`)
> and `(inp[len-i] & 0xc0) == 0xc0` (a 2-byte-sequence leading byte `110xxxxx`
> appearing as the final byte); else if `i < 3` and `(inp[len-i] & 0xe0) ==
> 0xe0` (a 3-byte leading byte `1110xxxx`); else if `i < 4` and `(inp[len-i] &
> 0xf0) == 0xf0` (a 4-byte leading byte `11110xxx`). On the first matching test
> it sets that byte to `'\0'` (truncating the string there) and returns
> immediately. If no test matches across all iterations, the string is left
> unchanged. Returns void; mutates `inp`. Note the masks are checked from
> coarsest bits, so the order means: a trailing 2-byte lead byte is trimmed at
> the last position; a 3-byte lead byte is trimmed if it is the last or
> second-to-last byte; a 4-byte lead byte if it is among the last three.

