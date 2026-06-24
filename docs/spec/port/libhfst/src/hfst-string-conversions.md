# libhfst/src/hfst-string-conversions.cc

> [spec:hfst:def:hfst-string-conversions.hfst.get-line-from-console-fn]
> bool get_line_from_console(std::string & str, size_t buffer_size, bool keep_newline /* = false*/)

> [spec:hfst:sem:hfst-string-conversions.hfst.get-line-from-console-fn]
> Windows-only. Reads one line of UTF-8 text from the console into `str`. Steps:
> Set a local `DEBUG=false` flag (gates only `std::cerr` debug output, which is disabled). Call `SetConsoleCP(65001)` to set the console input code page to UTF-8. Obtain the standard input handle via `GetStdHandle(STD_INPUT_HANDLE)`. Allocate a `WCHAR` buffer of `buffer_size` wide chars and a `DWORD numRead = 0`.
> Call `ReadConsoleW(stdIn, buffer, buffer_size/4 (converted to uint via hfst::size_t_to_uint), &numRead, NULL)`. If `ReadConsoleW` returns false, return `false`.
> On success: compute the required UTF-8 byte count with `WideCharToMultiByte(CP_UTF8, 0, buffer, (int)numRead, NULL, 0, NULL, NULL)` -> `size_needed`. Allocate a `CHAR` buffer `strbuf` of `size_needed` bytes and convert again into it with `WideCharToMultiByte`. Free the wide `buffer` (via `delete`, not `delete[]`). Write a NUL terminator at `strbuf[size_needed]` (note: this writes one byte past the allocated `size_needed`-length buffer). Assign `str = std::string(strbuf)`.
> If the first byte `str[0]` is char 26 (Ctrl+Z) or char 4 (Ctrl+D), return `false` (EOF). If `str.size() == 0`, return `true`. If `str.size() > 1` and the second-to-last char is `'\r'`, erase that one carriage-return char. If the last char is not `'\n'`, return `true`. If `keep_newline` is true, return `true`. Otherwise erase the trailing `'\n'` and return `true`.
> Side effects: allocations of `buffer` and `strbuf` (strbuf is leaked, never freed), console I/O, mutation of out-param `str`, and setting the console code page. Returns `bool` indicating whether a line was read (false on read failure or Ctrl+Z/Ctrl+D).

> [spec:hfst:def:hfst-string-conversions.hfst.hfst-fprintf-console-fn]
> int hfst_fprintf_console(FILE * stream, const char * format, ...)

> [spec:hfst:sem:hfst-string-conversions.hfst.hfst-fprintf-console-fn]
> Windows-only variadic printf that writes UTF-8 output correctly to the Windows console. Begin a `va_list args` with `va_start(args, format)`.
> If `stream` is `stdout` or `stderr`: format into a fixed 1024-byte stack `buffer` with `vsprintf(buffer, format, args)` -> `r`, then `va_end(args)`. If `r < 0`, return `r`. Choose the std handle: `GetStdHandle(STD_OUTPUT_HANDLE)`, or `STD_ERROR_HANDLE` if `stream == stderr`. Build `std::string pstr(buffer)`. Compute the wide-char count via `MultiByteToWideChar(CP_UTF8, 0, pstr.c_str(), -1, NULL, 0)` -> `wchars_num` (includes the NUL terminator). Allocate `wchar_t* wstr` of `wchars_num` and convert into it with `MultiByteToWideChar`. Write to the console with `WriteConsoleW(stdHandle, wstr, wchars_num-1, &numWritten, NULL)` (writing `wchars_num-1` chars to exclude the NUL) -> `retval`. Free `wstr` with `delete[]` and return `retval`.
> Otherwise (any other stream): call `vfprintf(stream, format, args)` -> `retval`, then `va_end(args)`, and return `retval`.
> Side effects: console or file output, one heap allocation (freed). Returns the `WriteConsoleW`/`vfprintf` return value, or the negative `vsprintf` result on formatting failure.

> [spec:hfst:def:hfst-string-conversions.hfst.set-console-cp-to-utf8-fn]
> void set_console_cp_to_utf8()

> [spec:hfst:sem:hfst-string-conversions.hfst.set-console-cp-to-utf8-fn]
> Windows-only. Calls `SetConsoleCP(65001)` to set the console input code page to UTF-8 (code page 65001). No parameters, no return value, return type `void`. The `SetConsoleCP` return value is ignored.

