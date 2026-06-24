# back-ends/sfst/utf8.cc

> [spec:hfst:def:utf8.sfst.int2utf8-fn]
> char *int2utf8( unsigned int sym )

> [spec:hfst:sem:utf8.sfst.int2utf8-fn]
> Encodes the Unicode code point `sym` (unsigned int) into a UTF-8 byte
> sequence and returns a pointer to it. The result is written into a
> function-`static` buffer `ch` of 5 `unsigned char` (NOT thread-safe;
> overwritten on each call), NUL-terminated, and returned cast to `char*`.
> Branch on the magnitude of `sym`:
> - `sym < 128`: 1 byte. `ch[0] = sym`; `ch[1] = 0` (terminator).
> - `sym < 2048`: 2 bytes (5+6 bits). `ch[0] = (sym >> 6) | 0xC0`;
>   `ch[1] = (sym & 0x3F) | 0x80`; `ch[2] = 0`.
> - `sym < 65536`: 3 bytes (4+6+6 bits). `ch[0] = (sym >> 12) | 0xE0`;
>   `ch[1] = ((sym >> 6) & 0x3F) | 0x80`; `ch[2] = (sym & 0x3F) | 0x80`;
>   `ch[3] = 0`.
> - `sym < 2097152`: 4 bytes (3+6+6+6 bits). `ch[0] = (sym >> 18) | 0xF0`;
>   `ch[1] = ((sym >> 12) & 0x3F) | 0x80`; `ch[2] = ((sym >> 6) & 0x3F) | 0x80`;
>   `ch[3] = (sym & 0x3F) | 0x80`; `ch[4] = 0`.
> - otherwise (`sym >= 2097152`): return `NULL` (no buffer written/returned).
> The constants used are the lead-byte masks 0x80/0xC0/0xE0/0xF0 (set N most
> significant bits) and the low-bit masks 0x07/0x0F/0x1F/0x3F. Each byte is
> truncated to `unsigned char`. Returns `(char*)ch` for all non-error cases.

> [spec:hfst:def:utf8.sfst.utf8toint-fn]
> unsigned int utf8toint( char **s )

> [spec:hfst:sem:utf8.sfst.utf8toint-fn]
> Decodes one UTF-8 character from the byte stream pointed to by `*s`,
> returning its Unicode code point as `unsigned int`, and ADVANCES `*s`
> (the caller's char pointer, passed by `char**`) past the bytes consumed.
> Read the lead byte `c = (unsigned char)**s` and determine sequence length:
> - `c >= 0xF0` (1111xxxx): 4-byte; set `bytes_to_come = 3`,
>   `result = (result << 3) | (c & 0x07)`.
> - `c >= 0xE0` (1110xxxx): 3-byte; `bytes_to_come = 2`,
>   `result = (result << 4) | (c & 0x0F)`.
> - `c >= 0xC0` (1100xxxx): 2-byte; `bytes_to_come = 1`,
>   `result = (result << 5) | (c & 0x1F)`.
> - `c < 0x80` (0xxxxxxx): 1-byte; `bytes_to_come = 0`, `result = c`.
> - otherwise (`0x80 <= c < 0xC0`, a stray continuation byte): return 0
>   (error) WITHOUT advancing `*s`.
> (`result` starts at 0, so the initial left-shifts are no-ops; the masks
> extract the data bits from the lead byte.) Then loop `bytes_to_come` times:
> each iteration decrement `bytes_to_come`, do `(*s)++`, read the next byte
> `c = (unsigned char)**s`; it must be a continuation byte
> (`0x80 <= c < 0xC0`, i.e. 10xxxxxx) — if so `result = (result << 6) | (c & 0x3F)`,
> otherwise return 0 (error). After the loop, do one final `(*s)++` (so `*s`
> points just past the last byte of the character) and return `result`.

