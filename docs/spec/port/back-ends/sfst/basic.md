# back-ends/sfst/basic.cc

> [spec:hfst:def:basic.sfst.fst-strdup-fn]
> char* fst_strdup(const char* pString)

> [spec:hfst:sem:basic.sfst.fst-strdup-fn]
> Duplicates a NUL-terminated C string `pString` into a freshly
> `malloc`-allocated buffer of size `strlen(pString) + 1` bytes.
> If `malloc` returns NULL (out of memory), prints
> "\nError: out of memory (malloc failed)\naborted.\n" to stderr
> and calls `exit(1)` (terminating the process). Otherwise copies
> the source string (including its terminating NUL) into the new
> buffer with `strcpy` and returns the pointer to the copy. The
> caller owns the returned buffer and is responsible for freeing it.

> [spec:hfst:def:basic.sfst.read-num-fn]
> size_t read_num( void *p, size_t n, FILE *file )

> [spec:hfst:sem:basic.sfst.read-num-fn]
> Reads `n` bytes from `file` into the buffer pointed to by `p`,
> performing byte-swapping if needed for endianness correction.
> Casts `p` to a `char*` (`pp`) and calls `fread(pp, 1, n, file)`,
> saving the number of bytes actually read as `result`. If the
> global flag `Switch_Bytes` is true, reverses the byte order of
> the buffer in place: iterating `i` from 0 to `n/2 - 1`, it swaps
> `pp[i]` with `pp[n-1]` while decrementing `n` each step (i.e. a
> standard in-place reversal of the first `n` bytes using `tmp`).
> Returns `result` (the byte count from `fread`). Note the swap
> always reverses the full original `n` bytes regardless of how
> many were actually read.

> [spec:hfst:def:basic.sfst.read-string-fn]
> int read_string( char *buffer, int size, FILE *file )

> [spec:hfst:sem:basic.sfst.read-string-fn]
> Reads a NUL-terminated string from `file` into `buffer`, reading
> at most `size` bytes. Loops `i` from 0 up to `size-1`, reading one
> byte per iteration with `fgetc(file)` into `c`. If `c` is EOF or 0
> (the NUL terminator), it stores 0 at `buffer[i]` and returns
> immediately: `1` (true) if the byte was a NUL (`c==0`), or `0`
> (false) if it was EOF. Otherwise it stores `(char)c` at
> `buffer[i]` and continues. If the loop completes without
> encountering EOF or NUL (i.e. `size` bytes were consumed), it
> forces `buffer[size-1] = 0` to NUL-terminate (truncating) and
> returns `0`. Mutates `buffer`; advances the file position.

