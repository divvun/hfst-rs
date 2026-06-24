# libhfst/src/parsers/SfstBasic.cc

> [spec:hfst:def:sfst-basic.sfst-basic.fst-strdup-fn]
> char* fst_strdup(const char* pString)

> [spec:hfst:sem:sfst-basic.sfst-basic.fst-strdup-fn]
> Duplicates a NUL-terminated C string. Allocates `strlen(pString) + 1`
> bytes with `malloc`. If the allocation returns NULL, prints
> `"\nError: out of memory (malloc failed)\naborted.\n"` to stderr and
> throws `HfstException` (via `HFST_THROW`). Otherwise copies the source
> string (including its terminating NUL) into the new buffer with `strcpy`
> and returns the pointer to the newly allocated copy. Caller owns and must
> free the returned buffer.

> [spec:hfst:def:sfst-basic.sfst-basic.read-num-fn]
> size_t read_num( void *p, size_t n, FILE *file )

> [spec:hfst:sem:sfst-basic.sfst-basic.read-num-fn]
> Reads `n` bytes from `file` into the memory at `p`, optionally swapping
> byte order. Casts `p` to `char*` (`pp`). Calls `fread(pp, 1, n, file)`
> and stores the number of bytes actually read in `result`. If the global
> `Switch_Bytes` flag is true, reverses the byte order of the `n`-byte
> region in place: lets `e = n/2` (integer division) and for `i` from 0 up
> to `e-1`, swaps `pp[i]` with `pp[--n]` (pre-decrementing the local copy of
> `n` each iteration), effectively swapping the first half of the buffer
> with the mirror-image second half. Returns `result` (the byte count from
> fread, not the possibly-mutated `n`). Note the byte-swap runs regardless
> of how many bytes were actually read.

> [spec:hfst:def:sfst-basic.sfst-basic.read-string-fn]
> int read_string( char *buffer, int size, FILE *file )

> [spec:hfst:sem:sfst-basic.sfst-basic.read-string-fn]
> Reads a NUL-terminated string of up to `size` bytes from `file` into
> `buffer`. Loops `i` from 0 to `size-1`, reading one character at a time
> with `fgetc(file)` into `int c`. If `c == EOF` or `c == 0` (the string
> terminator), it writes a 0 byte at `buffer[i]` to terminate, and returns
> `(c == 0)` — i.e. 1 (true) when terminated by a read NUL, 0 (false) when
> terminated by EOF. Otherwise stores `(char)c` at `buffer[i]` and
> continues. If the loop completes all `size` iterations without hitting
> EOF or NUL, forces `buffer[size-1] = 0` (overwriting the last stored byte
> to guarantee NUL-termination) and returns 0. The return value thus
> distinguishes a properly NUL-terminated read (1) from a truncated/EOF
> read (0).

