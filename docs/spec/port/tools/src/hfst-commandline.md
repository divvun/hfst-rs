# tools/src/hfst-commandline.cc, tools/src/hfst-commandline.h

> [spec:hfst:def:hfst-commandline.colour-tristate]
> enum colour_tristate {
>   COLOUR_NEVER;
>   COLOUR_ALWAYS;
>   COLOUR_AUTO;
> }

> [spec:hfst:def:hfst-commandline.conversion-type-fn]
> int

> [spec:hfst:sem:hfst-commandline.conversion-type-fn]
> Decide the common format two transducer types should be converted into.
> Given type1 and type2: if they are equal, return 0 (no conversion needed).
> Otherwise, if is_safe_conversion(type2, type1) is true (converting a type2
> transducer into type1 loses nothing), return 1 (convert into type1). Else if
> is_safe_conversion(type1, type2) is true, return 2 (convert into type2). Else
> return -1 (convert into type1, but loss of information is possible).

> [spec:hfst:def:hfst-commandline.convert-transducers-fn]
> void

> [spec:hfst:sem:hfst-commandline.convert-transducers-fn]
> Convert two transducers into a common format in place, if needed. Read
> type1 = first.get_type() and type2 = second.get_type(), then ct =
> conversion_type(type1, type2). If ct == 0, return without changes. If ct == 1,
> print a warning (status 0, errnum 0) "transducers have different types,
> converting to format <hfst_strformat(type1)>\n" and convert second into type1.
> If ct == 2, warn similarly with hfst_strformat(type2) and convert first into
> type2. If ct == -1, warn "... converting to format <hfst_strformat(type1)>,
> loss of information is possible\n" and convert second into type1. Any other
> value throws HfstFatalException with message "convert_transducers:
> conversion_type returned an invalid integer". Conversions use the transducer
> convert method with empty options.

> [spec:hfst:def:hfst-commandline.debug-printf-fn]
> void

> [spec:hfst:sem:hfst-commandline.debug-printf-fn]
> Conditional diagnostic printer. If the global debug flag is false, do nothing.
> If true, write to stderr the literal "\nDEBUG: ", then the (already formatted)
> message, then a trailing "\n". In C this took a printf format plus varargs; in
> the port the caller formats the message and passes the finished string.

> [spec:hfst:def:hfst-commandline.debug-save-transducer-fn]
> void

> [spec:hfst:sem:hfst-commandline.debug-save-transducer-fn]
> If the global debug flag is false, do nothing. Otherwise take a working copy
> of transducer t, set its name to "DEBUG " followed by name, open an
> HfstOutputStream to the file named name using t.get_type() as the format
> (hfst header format), emit a debug_printf "*** DEBUG (<program_name>): saving
> current transducer to <name>\n", write the transducer to that stream, and
> close the stream.

> [spec:hfst:def:hfst-commandline.error-at-line-fn]
> void

> [spec:hfst:sem:hfst-commandline.error-at-line-fn]
> GNU-style file-position error printer (vim/emacs error highlighting depends on
> this format). To stderr: print "<filename>.<linenum>: ", then the message; if
> errnum != 0 append strerror(errnum); then a trailing "\n". If status != 0,
> exit the process with that status.

> [spec:hfst:def:hfst-commandline.error-fn]
> void

> [spec:hfst:sem:hfst-commandline.error-fn]
> GNU-style error printer. To stderr: print "<program_name>: ", then the
> message; if errnum != 0 append strerror(errnum); then a trailing "\n". If
> status != 0, exit the process with that status.

> [spec:hfst:def:hfst-commandline.extend-options-getenv-fn]
> void

> [spec:hfst:sem:hfst-commandline.extend-options-getenv-fn]
> Append options taken from the HFST_OPTIONS environment variable to argv. Read
> getenv("HFST_OPTIONS"); if unset, return. Count the number of space characters
> in it. Because the real argv cannot be realloc'd, allocate (via hfst_malloc) a
> fresh argv array of size argc + spaces + 1 pointers, memcpy the existing argc
> pointers into it, and point the caller's argv at the new array (the old argv
> is intentionally not freed). Then strtok the env string on spaces: for each
> token, hfst_strdup it into the next free slot at argv[argc] and increment argc.
> Note strtok mutates the env string in place, exactly as in C.

> [spec:hfst:def:hfst-commandline.get-compatible-fst-format-fn]
> int

> [spec:hfst:sem:hfst-commandline.get-compatible-fst-format-fn]
> Deprecated since HFST3 (all formats are compatible). The body asserts false
> and returns -1; it is never meant to be called.

> [spec:hfst:def:hfst-commandline.getdelim-fn]
> ssize_t

> [spec:hfst:sem:hfst-commandline.getdelim-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-commandline.getline-fn]
> ssize_t

> [spec:hfst:sem:hfst-commandline.getline-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-commandline.hfst-calloc-fn]
> void *hfst_calloc(size_t nmemb, size_t size)

> [spec:hfst:sem:hfst-commandline.hfst-calloc-fn]
> Zeroing allocation that exits cleanly on failure. Call calloc(nmemb, size); if
> the result is null and size > 0, print an hfst_error (EXIT_FAILURE, errno,
> "calloc failed") which exits. Return the pointer. (Declared in the header but
> never defined in the C sources; the body mirrors hfst_malloc/hfst_realloc.)

> [spec:hfst:def:hfst-commandline.hfst-close-fn]
> int

> [spec:hfst:sem:hfst-commandline.hfst-close-fn]
> Wrap close(fd): if it returns -1, print hfst_error (EXIT_FAILURE, errno,
> "close failed") which exits. Return close's result.

> [spec:hfst:def:hfst-commandline.hfst-error-at-line-fn]
> void

> [spec:hfst:sem:hfst-commandline.hfst-error-at-line-fn]
> Colourised file-position error printer. To stderr, in order: maybe-colour
> BOLD, "<filename>.<linenum>: ", maybe-colour RED, "Error: ", maybe-colour
> RESET, then the message. If errnum != 0: maybe-colour MAGENTA,
> strerror(errnum), maybe-colour RESET. Unlike error_at_line there is NO
> trailing newline. If status != 0, exit with that status.

> [spec:hfst:def:hfst-commandline.hfst-error-fn]
> void

> [spec:hfst:sem:hfst-commandline.hfst-error-fn]
> Colourised error printer. To stderr, in order: maybe-colour BOLD,
> "<program_name>: ", maybe-colour RED, "Error: ", maybe-colour RESET, then the
> message. If errnum != 0: maybe-colour MAGENTA, strerror(errnum), maybe-colour
> RESET. Then a trailing "\n". If status != 0, exit with that status. This is
> the canonical fatal-error path used by all the hfst_* wrappers below.

> [spec:hfst:def:hfst-commandline.hfst-fopen-fn]
> FILE *

> [spec:hfst:sem:hfst-commandline.hfst-fopen-fn]
> Open a file, treating "-" specially. If filename == "-": with mode "r" return
> stdin, with mode "w" return stdout. Otherwise call fopen(filename, mode); if
> non-null, return it. On failure print hfst_error (EXIT_FAILURE, errno,
> "Could not open '<filename>'. ") which exits, then return null.

> [spec:hfst:def:hfst-commandline.hfst-fread-fn]
> size_t

> [spec:hfst:sem:hfst-commandline.hfst-fread-fn]
> Wrap fread(ptr, size, nmemb, stream). If the number of elements read is less
> than nmemb AND ferror(stream) is set, print hfst_error (EXIT_FAILURE, errno,
> "fread failed") which exits. Return the count actually read (a short read at
> clean EOF is not an error).

> [spec:hfst:def:hfst-commandline.hfst-fseek-fn]
> void

> [spec:hfst:sem:hfst-commandline.hfst-fseek-fn]
> Wrap fseek(stream, offset, whence): if it returns nonzero, print hfst_error
> (EXIT_FAILURE, errno, "fseek failed") which exits.

> [spec:hfst:def:hfst-commandline.hfst-ftell-fn]
> unsigned long

> [spec:hfst:sem:hfst-commandline.hfst-ftell-fn]
> Wrap ftell(stream). If the result is not -1, return it as an unsigned long. If
> it is -1, print hfst_error (EXIT_FAILURE, errno, "ftell failed") which exits,
> then return -1 cast to unsigned (the all-ones value).

> [spec:hfst:def:hfst-commandline.hfst-fwrite-fn]
> size_t

> [spec:hfst:sem:hfst-commandline.hfst-fwrite-fn]
> Wrap fwrite(ptr, size, nmemb, stream). If fewer than nmemb elements were
> written OR ferror(stream) is set, print hfst_error (EXIT_FAILURE, errno,
> "fwrite failed") which exits. Return the count actually written.

> [spec:hfst:def:hfst-commandline.hfst-getdelim-fn]
> ssize_t

> [spec:hfst:sem:hfst-commandline.hfst-getdelim-fn]
> Safe wrapper around getdelim(lineptr, n, delim, stream). After the call, if it
> returned a negative value AND errno is nonzero, print hfst_error (EXIT_FAILURE,
> errno, "getdelim failed") which exits. Return getdelim's result (a negative
> value with errno == 0 means clean end of input, which is not an error).

> [spec:hfst:def:hfst-commandline.hfst-getline-fn]
> ssize_t

> [spec:hfst:sem:hfst-commandline.hfst-getline-fn]
> Safe wrapper around getline(lineptr, n, stream). After the call, if it
> returned a negative value AND errno is nonzero, print hfst_error (EXIT_FAILURE,
> errno, "getline failed") which exits. Return getline's result (a negative
> value with errno == 0 means clean end of input, which is not an error).

> [spec:hfst:def:hfst-commandline.hfst-malloc-fn]
> void *

> [spec:hfst:sem:hfst-commandline.hfst-malloc-fn]
> Allocation that exits cleanly on failure. Call malloc(size); if the result is
> null AND size > 0, print hfst_error (EXIT_FAILURE, errno, "malloc failed")
> which exits. Return the pointer (a null result for size == 0 is allowed).

> [spec:hfst:def:hfst-commandline.hfst-mkstemp-fn]
> int

> [spec:hfst:sem:hfst-commandline.hfst-mkstemp-fn]
> Wrap mkstemp(templ) (which mutates the template in place and returns a file
> descriptor). If it returns -1, print hfst_error (EXIT_FAILURE, errno,
> "mkstemp failed") which exits. Return the descriptor. (The Windows arm of the
> C source is omitted in the port.)

> [spec:hfst:def:hfst-commandline.hfst-open-fn]
> int

> [spec:hfst:sem:hfst-commandline.hfst-open-fn]
> Wrap open(pathname, flags): if it returns -1, print hfst_error (EXIT_FAILURE,
> errno, "open failed") which exits. Return the file descriptor.

> [spec:hfst:def:hfst-commandline.hfst-parse-format-name-fn]
> hfst::ImplementationType

> [spec:hfst:sem:hfst-commandline.hfst-parse-format-name-fn]
> Map a case-insensitive format-name string to an ImplementationType. Matches
> (all compared case-insensitively): "sfst" -> SFST_TYPE; "openfst-tropical" or
> "ofst-tropical" -> TROPICAL_OPENFST_TYPE; "openfst-log" or "ofst-log" ->
> LOG_OPENFST_TYPE; "openfst" or "ofst" -> TROPICAL_OPENFST_TYPE plus a warning
> "Ambiguous format name <s>, guessing openfst-tropical"; "foma" -> FOMA_TYPE;
> "xfsm" -> XFSM_TYPE; "optimized-lookup-unweighted" or "olu" -> HFST_OL_TYPE;
> "optimized-lookup-weighted" or "olw" -> HFST_OLW_TYPE; "optimized-lookup" or
> "ol" -> HFST_OLW_TYPE plus a warning "Ambiguous format name <s>, guessing
> optimized-lookup-weighted". Anything else: print hfst_error (EXIT_FAILURE, 0,
> "Could not parse format name from string <s>") which exits, and (unreachable)
> return UNSPECIFIED_TYPE.

> [spec:hfst:def:hfst-commandline.hfst-read-fn]
> ssize_t

> [spec:hfst:sem:hfst-commandline.hfst-read-fn]
> Wrap read(fd, buf, count). First, if count exceeds the maximum signed size,
> print hfst_error (EXIT_FAILURE, 0, "cannot read <count> bytes in one read(2)")
> which exits. Then call read; if it returns -1, print hfst_error (EXIT_FAILURE,
> errno, "read failed") which exits. Return the number of bytes read.

> [spec:hfst:def:hfst-commandline.hfst-readline-fn]
> char *

> [spec:hfst:sem:hfst-commandline.hfst-readline-fn]
> Read one line of interactive input. Thin wrapper that simply calls readline
> (the non-library fallback, since the real readline library path is compiled
> out) with the given prompt and returns its result.

> [spec:hfst:def:hfst-commandline.hfst-realloc-fn]
> void *

> [spec:hfst:sem:hfst-commandline.hfst-realloc-fn]
> Reallocation that exits cleanly on failure. Call realloc(ptr, size); if the
> result is null AND size > 0, print hfst_error (EXIT_FAILURE, errno,
> "realloc failed") which exits. Return the pointer.

> [spec:hfst:def:hfst-commandline.hfst-remove-fn]
> int

> [spec:hfst:sem:hfst-commandline.hfst-remove-fn]
> Wrap remove(filename): if it returns -1, print hfst_error (EXIT_FAILURE,
> errno, "remove <filename> failed") which exits. Return remove's result.

> [spec:hfst:def:hfst-commandline.hfst-set-program-name-fn]
> void

> [spec:hfst:sem:hfst-commandline.hfst-set-program-name-fn]
> Initialise the tool's identity globals. Call set_program_name(argv0) to derive
> and store program_name, then store hfst_tool_version = hfst_strdup(version) and
> hfst_tool_wikiname = hfst_strdup(wikiname). Must be called at the start of main
> because every error/usage message reads these.

> [spec:hfst:def:hfst-commandline.hfst-setlocale-fn]
> char *

> [spec:hfst:sem:hfst-commandline.hfst-setlocale-fn]
> Set the program locale from the environment. Call setlocale(LC_ALL, ""); if it
> returns null, print hfst_error (EXIT_FAILURE, errno, "Unable to set locale for
> character settings") which exits. Return setlocale's result. (When the host
> lacks setlocale the C returned null; that arm is not relevant to the port.)

> [spec:hfst:def:hfst-commandline.hfst-strdup-fn]
> char *

> [spec:hfst:sem:hfst-commandline.hfst-strdup-fn]
> Duplicate a C string, exiting cleanly on failure. Call strdup(s); if it returns
> null, print hfst_error (EXIT_FAILURE, errno, "strdup failed") which exits.
> Return the duplicate.

> [spec:hfst:def:hfst-commandline.hfst-strformat-fn]
> const char *

> [spec:hfst:sem:hfst-commandline.hfst-strformat-fn]
> Return a human-readable description string for an ImplementationType:
> SFST_TYPE -> "SFST (1.4 compatible)"; TROPICAL_OPENFST_TYPE -> "OpenFST, std
> arc, tropical semiring"; LOG_OPENFST_TYPE -> "OpenFST, std arc, log semiring";
> FOMA_TYPE -> "foma"; XFSM_TYPE -> "xfsm"; HFST_OL_TYPE -> "Hfst's lookup
> optimized, unweighted"; HFST_OLW_TYPE -> "Hfst's lookup optimized, weighted";
> HFST2_TYPE -> "Hfst 2 legacy (deprecated)"; ERROR_TYPE, UNSPECIFIED_TYPE and
> any other value -> "ERROR (not a HFST supported transducer)".

> [spec:hfst:def:hfst-commandline.hfst-strndup-fn]
> char *

> [spec:hfst:sem:hfst-commandline.hfst-strndup-fn]
> Duplicate at most n bytes of a C string, exiting cleanly on failure. Call
> strndup(s, n); if it returns null, print hfst_error (EXIT_FAILURE, errno,
> "strndup failed") which exits. Return the duplicate.

> [spec:hfst:def:hfst-commandline.hfst-strtol-fn]
> long

> [spec:hfst:sem:hfst-commandline.hfst-strtol-fn]
> Parse a signed long from string s in the given base. Reset errno, call
> strtol(s, &endptr, base). If endptr points at the terminating NUL (the whole
> string was consumed), return the value. Otherwise print hfst_error
> (EXIT_FAILURE, errno, "<s> is not a valid signed number string") which exits,
> then return the partial value.

> [spec:hfst:def:hfst-commandline.hfst-strtonumber-fn]
> int

> [spec:hfst:sem:hfst-commandline.hfst-strtonumber-fn]
> Parse an int from string s, with infinity handling. If the infinite out-param
> is provided, first set it to false. Call strtod(s, &endptr). If the whole
> string was consumed (endptr at NUL): if the value is infinite and infinite is
> provided, set infinite = true and return signbit(value) (1 for negative
> infinity, 0 for positive); else if value > INT_MAX return INT_MAX; else if
> value < INT_MIN return INT_MIN; else return floor(value) as int. If the string
> was not fully consumed, print hfst_error (EXIT_FAILURE, errno, "<s> not a
> number") which exits, then return the value cast to int.

> [spec:hfst:def:hfst-commandline.hfst-strtoul-fn]
> unsigned long

> [spec:hfst:sem:hfst-commandline.hfst-strtoul-fn]
> Parse an unsigned long from string s in the given base. Reset errno, call
> strtoul(s, &endptr, base). If the whole string was consumed (endptr at NUL),
> return the value. Otherwise print hfst_error (EXIT_FAILURE, errno, "<s> is not
> a valid unsigned number string") which exits, then return the partial value.

> [spec:hfst:def:hfst-commandline.hfst-strtoweight-fn]
> double

> [spec:hfst:sem:hfst-commandline.hfst-strtoweight-fn]
> Parse a weight (floating point) from string s. Reset errno, call
> strtod(s, &endptr). If the whole string was consumed (endptr at NUL), return
> the value. Otherwise print hfst_error (EXIT_FAILURE, errno, "<s> not a weight")
> which exits, then return the value. (The C returned a double; HFST weights are
> 32-bit, so the port returns f32.)

> [spec:hfst:def:hfst-commandline.hfst-tmpfile-fn]
> FILE *

> [spec:hfst:sem:hfst-commandline.hfst-tmpfile-fn]
> Wrap tmpfile(): if it returns null, print hfst_error (EXIT_FAILURE, errno,
> "tmpfile failed") which exits. Return the FILE pointer.

> [spec:hfst:def:hfst-commandline.hfst-warning-at-line-fn]
> void

> [spec:hfst:sem:hfst-commandline.hfst-warning-at-line-fn]
> Colourised file-position warning printer. To stderr, in order: maybe-colour
> BOLD, "<filename>.<linenum>: ", maybe-colour YELLOW, "Warning: ", maybe-colour
> RESET, then the message. If errnum != 0: maybe-colour MAGENTA, strerror(errnum),
> maybe-colour RESET. No trailing newline. If status != 0, exit with that status.

> [spec:hfst:def:hfst-commandline.hfst-warning-fn]
> void

> [spec:hfst:sem:hfst-commandline.hfst-warning-fn]
> Colourised warning printer. To stderr, in order: maybe-colour BOLD,
> "<program_name>: ", maybe-colour YELLOW, "Warning: ", maybe-colour RESET, then
> the message. If errnum != 0: maybe-colour MAGENTA, strerror(errnum),
> maybe-colour RESET. Then a trailing "\n". If status != 0, exit with that
> status.

> [spec:hfst:def:hfst-commandline.hfst-write-fn]
> ssize_t

> [spec:hfst:sem:hfst-commandline.hfst-write-fn]
> Wrap write(fd, buf, count): if it returns -1, print hfst_error (EXIT_FAILURE,
> errno, "write failed") which exits. Return the number of bytes written.

> [spec:hfst:def:hfst-commandline.is-input-stream-in-ol-format-fn]
> bool

> [spec:hfst:sem:hfst-commandline.is-input-stream-in-ol-format-fn]
> Detect whether an input stream's next transducer is in optimized-lookup form.
> If is.get_type() is HFST_OL_TYPE or HFST_OLW_TYPE, print to stderr "Error:
> <program> cannot process transducers that are in optimized lookup format.\n"
> and return true. Otherwise return false.

> [spec:hfst:def:hfst-commandline.isatty-fn]
> int

> [spec:hfst:sem:hfst-commandline.isatty-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-commandline.maybe-print-colour-fn]
> void

> [spec:hfst:sem:hfst-commandline.maybe-print-colour-fn]
> Conditionally emit an ANSI colour escape. If should_colourise() is true, write
> the colour string to file f; otherwise do nothing.

> [spec:hfst:def:hfst-commandline.parse-options-fn]
> int parse_options(int argc, char **argv)

> [spec:hfst:sem:hfst-commandline.parse-options-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-commandline.print-more-info-fn]
> void

> [spec:hfst:sem:hfst-commandline.print-more-info-fn]
> Print pointers to documentation, to message_out. Two lines: first
> "<program_name> home page: \n<<WIKI_URL>/<hfst_tool_wikiname>>\n", then
> "General help using HFST software: \n<<WIKI_URL>>\n".

> [spec:hfst:def:hfst-commandline.print-report-bugs-fn]
> void

> [spec:hfst:sem:hfst-commandline.print-report-bugs-fn]
> Print the bug-reporting message to message_out: "Report bugs to
> <<PACKAGE_BUGREPORT>> or directly to our bug tracker at:\n<https://github.com/
> hfst/hfst/issues>\n". PACKAGE_BUGREPORT expands to the empty string when no
> config header is present.

> [spec:hfst:def:hfst-commandline.print-short-help-fn]
> void

> [spec:hfst:sem:hfst-commandline.print-short-help-fn]
> Print a one-line pointer to the full help, to message_out:
> "Try ``<program_name> --help'' for more information.\n".

> [spec:hfst:def:hfst-commandline.print-usage-fn]
> void print_usage()

> [spec:hfst:sem:hfst-commandline.print-usage-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-commandline.print-version-fn]
> void

> [spec:hfst:sem:hfst-commandline.print-version-fn]
> Print the GNU-standard version banner to message_out: a first line
> "<program_name> <hfst_tool_version> (<PACKAGE_STRING>)\n" followed by the
> fixed copyright/licence block ("Copyright (C) 2017 University of Helsinki,",
> "License GPLv3: GNU GPL version 3 <http://gnu.org/licenses/gpl.html>", "This
> is free software: you are free to change and redistribute it.", "There is NO
> WARRANTY, to the extent permitted by law."), each terminated by a newline.

> [spec:hfst:def:hfst-commandline.readline-fn]
> char *

> [spec:hfst:sem:hfst-commandline.readline-fn]
> Non-library fallback line reader (the real readline-library path is compiled
> out). Write the prompt to message_out. Then read one line from stdin via
> hfst_getline into a freshly allocated buffer; if hfst_getline returns -1,
> return null. Otherwise return the allocated line buffer (caller owns it).

> [spec:hfst:def:hfst-commandline.set-program-name-fn]
> void

> [spec:hfst:sem:hfst-commandline.set-program-name-fn]
> Derive and store program_name from argv0 (gnulib logic). Find the basename
> (text after the last '/', or the whole string if no '/'). If the basename is
> preceded by exactly "/.libs/" (i.e. the 7 chars before it equal "/.libs/"),
> use the basename as the effective name, and if that basename further starts
> with "lt-", drop those 3 leading chars (libtool wrapper stripping). Finally, if
> the resulting name equals "hfst-calculate", store program_name =
> hfst_strdup("hfst-sfstpl2fst"); otherwise store program_name =
> hfst_strdup(name).

> [spec:hfst:def:hfst-commandline.should-colourise-fn]
> bool

> [spec:hfst:sem:hfst-commandline.should-colourise-fn]
> Decide whether to emit colour, from the global colour tristate. If
> COLOUR_AUTO, return true iff isatty(1) is true (stdout is a terminal). If
> COLOUR_ALWAYS, return true. If COLOUR_NEVER, return false. Any other value
> asserts false and returns false.

> [spec:hfst:def:hfst-commandline.ssize-t]
> typedef SSIZE_T ssize_t

> [spec:hfst:def:hfst-commandline.strndup-fn]
> char *

> [spec:hfst:sem:hfst-commandline.strndup-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-commandline.verbose-printf-fn]
> void

> [spec:hfst:sem:hfst-commandline.verbose-printf-fn]
> Conditional progress printer. If the global verbose flag is false, do nothing.
> If true, write the (already formatted) message verbatim to message_out. In C
> this took a printf format plus varargs; in the port the caller formats the
> message and passes the finished string.

> [spec:hfst:def:hfst-commandline.warning-fn]
> void

> [spec:hfst:sem:hfst-commandline.warning-fn]
> GNU-style warning printer. To stderr: print "<program_name>: warning: ", then
> the message; if errnum != 0 append strerror(errnum); then a trailing "\n". If
> status != 0, exit the process with that status.
