# tools/src/hfst-file-to-mem.cc

> [spec:hfst:def:hfst-file-to-mem.hfst-file-to-mem-fn]
> char * hfst_file_to_mem(const char *filename)

> [spec:hfst:sem:hfst-file-to-mem.hfst-file-to-mem-fn]
> Reads the entire contents of the named file into a freshly allocated,
> NUL-terminated C string and returns a pointer to it (caller owns / frees it).
> Steps:
> 1. If 'filename' equals the literal string "<stdin>", delegate to
>    hfst_stdin_to_mem() and return its result.
> 2. Open 'filename' with fopen in binary read mode ("rb"). On failure (NULL),
>    call error(EXIT_FAILURE, 0, "Error opening file '<filename>'\n") and return
>    NULL.
> 3. Seek to end (fseek SEEK_END), record the byte length via ftell as numbytes,
>    then seek back to the start (fseek SEEK_SET).
> 4. malloc a buffer of (numbytes + 1) chars. On failure, call
>    error(EXIT_FAILURE, 0, "Error allocating memory to read file
>    '<filename>'\n") and return NULL.
> 5. fread numbytes chars into the buffer. If the number actually read is not
>    equal to numbytes, call error(EXIT_FAILURE, 0, "Error reading file
>    '<filename>' to memory\n") and return NULL.
> 6. fclose the file, write a '\0' terminator at buffer[numbytes], and return the
>    buffer.

> [spec:hfst:def:hfst-file-to-mem.hfst-stdin-to-mem-fn]
> char * hfst_stdin_to_mem()

> [spec:hfst:sem:hfst-file-to-mem.hfst-stdin-to-mem-fn]
> Reads standard input byte-by-byte into a freshly allocated, NUL-terminated C
> string and returns a pointer to it. Steps:
> 1. Set maxbytes = 1000000 and numbytes = 0.
> 2. malloc a buffer of maxbytes chars (noted as slow in the source). On failure
>    (NULL), call error(EXIT_FAILURE, 0, "Error allocating memory to read file
>    '<stdin>'\n") and return NULL.
> 3. Loop forever:
>    a. Store fgetc(stdin) (cast to char) into buffer[numbytes].
>    b. If feof(stdin) is now true, overwrite buffer[numbytes] with '\0' and
>       break out of the loop.
>    c. Otherwise increment numbytes; if numbytes >= maxbytes, call
>       error(EXIT_FAILURE, 0, "Error reading file '<stdin>' to memory, not
>       enough memory\n") and return NULL.
> 4. Return the buffer.

