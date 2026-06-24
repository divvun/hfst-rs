# libhfst/src/parsers/io_src/InputReader.cc, libhfst/src/parsers/io_src/InputReader.h

> [spec:hfst:def:input-reader.input-not-set]
> class InputNotSet

> [spec:hfst:def:input-reader.input-reader]
> class InputReader {
>   std::istream *input_stream;
>   std::string filename;
>   size_t &counter;
>   char buffer[HTWOLCBUFFERSIZE];
>   size_t buffer_size;
>   size_t buffer_index;
>   std::ostream *warning_stream;
>   std::ostream *error_stream;
> }

> [spec:hfst:def:input-reader.input-reader.error-fn]
> void

> [spec:hfst:sem:input-reader.input-reader.error-fn]
> Reports a fatal error message `err` to the error stream. Does nothing if
> `error_stream` is NULL. Otherwise writes to `*error_stream`: a newline; if
> `should_colourise()` returns true the bold escape `COLOUR_BOLD`; then
> `filename`, `":"`, `counter`, `": "`; if colourising, `COLOUR_RESET`
> followed by `COLOUR_RED`; then `err`, `":"`, and a newline; if colourising,
> `COLOUR_RESET`; then the current `buffer` contents and a newline; then the
> literal `"Aborted."` followed by two newlines. Does not throw or change the
> return-by-void; purely a side-effecting print.

> [spec:hfst:def:input-reader.input-reader.input-fn]
> char

> [spec:hfst:sem:input-reader.input-reader.input-fn]
> Returns the next character from the input, one at a time, advancing through
> the line buffer. If `input_stream` is NULL, throws `InputNotSet`. Otherwise,
> if the character at `buffer[buffer_index]` is the NUL terminator `0` (i.e.
> the current line has been fully consumed), reads the next line into `buffer`
> via `input_stream->getline(buffer, buffer_size)`; then if
> `input_stream->gcount() == 0` (nothing was read, i.e. end of input) returns
> `0`; otherwise sets `buffer_index = 0` and returns `'\n'` (the newline that
> separated the consumed line from the next). Otherwise returns
> `buffer[buffer_index]` and post-increments `buffer_index`. Net effect: emits
> each line's characters in order, emits a single `'\n'` at each line boundary,
> and emits `0` at end of input (and on every subsequent call, since the buffer
> stays empty).

> [spec:hfst:def:input-reader.input-reader.input-reader-fn]
> InputReader::InputReader(size_t &counter)

> [spec:hfst:sem:input-reader.input-reader.input-reader-fn]
> Constructor. Takes a reference `counter` to a `size_t` (a line counter owned
> by the caller) and stores it as the member reference `counter`. Initializes
> `input_stream` to NULL, `filename` to `"<unknown>"`, `buffer_size` to
> `HTWOLCBUFFERSIZE`, `buffer_index` to `0`, `warning_stream` to NULL, and
> `error_stream` to NULL. The `buffer` array is left uninitialized.

> [spec:hfst:def:input-reader.input-reader.reset-fn]
> void

> [spec:hfst:sem:input-reader.input-reader.reset-fn]
> Resets the reader to its initial state. Sets `input_stream` to NULL,
> `buffer_size` to `HTWOLCBUFFERSIZE`, `buffer_index` to `0`, `warning_stream`
> to NULL, and `error_stream` to NULL. Does NOT reset the `counter` reference
> (it aliases caller-owned state and must be reset separately) and does not
> change `filename` or `buffer` contents.

> [spec:hfst:def:input-reader.input-reader.set-error-stream-fn]
> void

> [spec:hfst:sem:input-reader.input-reader.set-error-stream-fn]
> Stores the address of the given `std::ostream` reference `ostr` into the
> member `error_stream`. No other effect.

> [spec:hfst:def:input-reader.input-reader.set-input-fn]
> void

> [spec:hfst:sem:input-reader.input-reader.set-input-fn]
> Two overloads. The single-argument form `set_input(file)` stores `&file` into
> `input_stream`, sets `filename` to `"<stdin>"`, then eagerly reads the first
> line into `buffer` via `input_stream->getline(buffer, buffer_size)`. The
> two-argument form `set_input(file, filename)` is identical except it stores
> the supplied `filename` into the member `filename` instead of `"<stdin>"`.
> Both prime the buffer so the very first `input()` call can return the first
> character.

> [spec:hfst:def:input-reader.input-reader.set-warning-stream-fn]
> void

> [spec:hfst:sem:input-reader.input-reader.set-warning-stream-fn]
> Stores the address of the given `std::ostream` reference `ostr` into the
> member `warning_stream`. No other effect.

> [spec:hfst:def:input-reader.input-reader.warn-fn]
> void

> [spec:hfst:sem:input-reader.input-reader.warn-fn]
> Reports a non-fatal warning message `warning` to the warning stream. Does
> nothing if `warning_stream` is NULL. Otherwise writes to `*warning_stream`: a
> newline; if `should_colourise()` returns true the bold escape `COLOUR_BOLD`;
> then `filename`, `":"`, `counter`, `": "`; if colourising, `COLOUR_RESET`
> followed by `COLOUR_YELLOW`; then `warning`, `":"`, and a newline; if
> colourising, `COLOUR_RESET`; then the current `buffer` contents and a
> newline. Unlike `error`, does not append `"Aborted."`. Purely
> side-effecting; returns void.

> [spec:hfst:def:input-reader.main-fn]
> int

> [spec:hfst:sem:input-reader.main-fn]
> Compiled only under `#ifdef INPUT_READER_TEST`. A standalone unit-test
> `main` exercising `InputReader`. (1) Builds `str1 =
> "Some text spanning one line only."`, wraps it in an `istringstream in1`,
> sets `counter = 1`, constructs `InputReader ir1(counter)`. Calls
> `ir1.input()` before any input is set and asserts it throws `InputNotSet`
> (the `assert(false)` after the call must not be reached; the `catch` does
> nothing). Then `ir1.set_input(in1)` and asserts each successive `input()`
> returns the characters of `str1` in order, finally asserting `input()`
> returns `0` at end. (2) Builds `str2 = "line\nline."`, wraps in `in2`,
> constructs `InputReader ir2(counter)`, `set_input(in2)`, and asserts
> `input()` yields `l`,`i`,`n`,`e`, then `'\n'` at the line boundary, then
> `l`,`i`,`n`,`e`,`.`, then `0`, and asserts a further `input()` again returns
> `0` (end-of-input is idempotent). Returns no explicit value.

> [spec:hfst:def:input-reader.should-colourise-fn]
> static bool

> [spec:hfst:sem:input-reader.should-colourise-fn]
> File-local helper. Returns true if standard output (file descriptor `1`) is
> a terminal, i.e. `isatty(1)` is non-zero; otherwise returns false. Used to
> decide whether warning/error output should include ANSI colour escapes. No
> arguments, no side effects.

