# libhfst/src/parsers/SfstUtf8.cc

> [spec:hfst:def:sfst-utf8.sfst-utf8.int2utf8-fn]
> char *int2utf8( unsigned int sym )

> [spec:hfst:sem:sfst-utf8.sfst-utf8.int2utf8-fn]
> Encodes a single Unicode code point `sym` (unsigned int) into a NUL-terminated
> UTF-8 byte sequence and returns a pointer to it. Writes into a function-local
> `static unsigned char ch[5]` buffer (so the result is shared/overwritten across
> calls and not thread-safe). Branches on the magnitude of `sym`:
> - `sym < 128` (1 byte, 7 bits): `ch[0] = sym`; `ch[1] = 0`.
> - `sym < 2048` (2 bytes, 5+6 bits): `ch[0] = (sym >> 6) | 0xC0`;
>   `ch[1] = (sym & 0x3F) | 0x80`; `ch[2] = 0`. (set2MSbits=192=0xC0,
>   get6LSbits=63=0x3F, set1MSbits=128=0x80.)
> - `sym < 65536` (3 bytes, 4+6+6 bits): `ch[0] = (sym >> 12) | 0xE0`;
>   `ch[1] = ((sym >> 6) & 0x3F) | 0x80`; `ch[2] = (sym & 0x3F) | 0x80`;
>   `ch[3] = 0`. (set3MSbits=224=0xE0.)
> - `sym < 2097152` (4 bytes, 3+6+6+6 bits): `ch[0] = (sym >> 18) | 0xF0`;
>   `ch[1] = ((sym >> 12) & 0x3F) | 0x80`; `ch[2] = ((sym >> 6) & 0x3F) | 0x80`;
>   `ch[3] = (sym & 0x3F) | 0x80`; `ch[4] = 0`. (set4MSbits=240=0xF0.)
> - Otherwise (`sym >= 2097152`): returns `NULL`.
> On success returns `(char*)ch`. No allocation, no exceptions; the trailing 0
> byte terminates the string. Each continuation byte uses the low 6 bits of the
> remaining value masked with 0x3F and ORed with the 0x80 marker.

> [spec:hfst:def:sfst-utf8.sfst-utf8.utf8toint-fn]
> unsigned int utf8toint( char **s )

> [spec:hfst:sem:sfst-utf8.sfst-utf8.utf8toint-fn]
> Decodes one UTF-8 character starting at `**s` and returns its code point as an
> unsigned int, advancing the caller's pointer `*s` past the consumed bytes.
> Takes `char **s` (pointer to a char pointer). Reads the first byte
> `c = (unsigned char)**s` and `result = 0`, then classifies the lead byte by
> comparing against the MS-bit masks (set4MSbits=240=0xF0, set3MSbits=224=0xE0,
> set2MSbits=192=0xC0, set1MSbits=128=0x80):
> - `c >= 0xF0` (1111xxxx): 4-byte lead, `bytes_to_come = 3`,
>   `result = (result << 3) | (c & get3LSbits/*7*/)`.
> - else `c >= 0xE0` (1110xxxx): 3-byte lead, `bytes_to_come = 2`,
>   `result = (result << 4) | (c & get4LSbits/*15*/)`.
> - else `c >= 0xC0` (1100xxxx): 2-byte lead, `bytes_to_come = 1`,
>   `result = (result << 5) | (c & get5LSbits/*31*/)`.
> - else `c < 0x80` (0xxxxxxx): 1-byte, `bytes_to_come = 0`, `result = c`.
> - else (i.e. `c` in 0x80..0xBF, a stray continuation byte): returns `0` (error).
> Then loops `bytes_to_come` times: each iteration decrements the counter,
> advances `(*s)++`, reads the next byte `c`. If `c` is a valid continuation byte
> (`c >= 0x80 && c < 0xC0`, i.e. 10xxxxxx), accumulates
> `result = (result << 6) | (c & get6LSbits/*63*/)`; otherwise immediately
> returns `0` (error) without further advancing. After the loop, advances
> `(*s)++` once more (past the last byte consumed) and returns `result`.
> Note: a successfully decoded NUL or a code point that genuinely equals 0 is
> indistinguishable from the error return; no exceptions, no allocation.

