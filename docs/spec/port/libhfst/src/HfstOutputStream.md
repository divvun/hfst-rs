# libhfst/src/HfstOutputStream.cc, libhfst/src/HfstOutputStream.h

> [spec:hfst:def:hfst-output-stream.hfst.hfst-output-stream]
> class HfstOutputStream {
>   union StreamImplementation { #if HAVE_OPENFST #if HAVE_OPENFST_LOG || HAVE_LEAN_OPENFST_LOG hfst::implementations::LogWeightOutputStream * log_ofst; #endif h...;
>   ImplementationType type;
>   bool hfst_format;
>   StreamImplementation implementation;
>   bool is_open;
>   HFSTDLL HfstOutputStream &flush();
>   HFSTDLL HfstOutputStream &operator<< (HfstTransducer &transducer);
>   HFSTDLL HfstOutputStream& redirect (HfstTransducer &transducer);
> }

> [spec:hfst:def:hfst-output-stream.hfst.hfst-output-stream.append-fn]
> void HfstOutputStream::append(std::vector<char> &str, const std::string &s)

> [spec:hfst:sem:hfst-output-stream.hfst.hfst-output-stream.append-fn]
> Appends the string `s` to the byte vector `str`, then appends a
> terminating NUL byte. Iterates over each character of `s` (indices 0
> to s.length()-1) pushing each char onto `str`, then pushes a single
> '\0'. Mutates `str` in place; no return value, no I/O, no exceptions.

> [spec:hfst:def:hfst-output-stream.hfst.hfst-output-stream.append-hfst-header-data-fn]
> void HfstOutputStream::append_hfst_header_data(std::vector<char> &header)

> [spec:hfst:sem:hfst-output-stream.hfst.hfst-output-stream.append-hfst-header-data-fn]
> Appends the standard HFST header attribute/value pairs to the byte
> vector `header` using the `append` helper (which NUL-terminates each
> string). In order it appends: "version", "3.3", "type". Then it
> selects a type string from the stream's `type` field: SFST_TYPE ->
> "SFST", TROPICAL_OPENFST_TYPE -> "TROPICAL_OPENFST", LOG_OPENFST_TYPE
> -> "LOG_OPENFST", FOMA_TYPE -> "FOMA", XFSM_TYPE -> "XFSM",
> MY_TRANSDUCER_LIBRARY_TYPE -> "MY_TRANSDUCER_LIBRARY", HFST_OL_TYPE ->
> "HFST_OL", HFST_OLW_TYPE -> "HFST_OLW"; any other type triggers
> assert(false). Finally appends that type string to `header`. Mutates
> `header` in place; no return value.

> [spec:hfst:def:hfst-output-stream.hfst.hfst-output-stream.append-implementation-specific-header-data-fn]
> void

> [spec:hfst:sem:hfst-output-stream.hfst.hfst-output-stream.append-implementation-specific-header-data-fn]
> Appends implementation-specific header data for the given transducer.
> This is the non-SFST build variant (compiled when neither HAVE_SFST nor
> HAVE_LEAN_SFST is defined): both parameters (`header` and `transducer`)
> are unnamed and the body is empty, so it does nothing. In the SFST
> build variant the body switches on `type`: for SFST_TYPE it calls
> `implementation.sfst->append_implementation_specific_header_data(header,
> transducer.implementation.sfst)`, and for all other types does nothing.
> No return value.

> [spec:hfst:def:hfst-output-stream.hfst.hfst-output-stream.close-fn]
> void HfstOutputStream::close(void)

> [spec:hfst:sem:hfst-output-stream.hfst.hfst-output-stream.close-fn]
> Closes the underlying backend output stream and marks this stream
> closed. Switches on `type` and calls `close()` on the corresponding
> member of the `implementation` union: SFST_TYPE ->
> implementation.sfst; TROPICAL_OPENFST_TYPE ->
> implementation.tropical_ofst; LOG_OPENFST_TYPE ->
> implementation.log_ofst; FOMA_TYPE -> implementation.foma; XFSM_TYPE ->
> implementation.xfsm; MY_TRANSDUCER_LIBRARY_TYPE ->
> implementation.my_transducer_library; HFST_OL_TYPE and HFST_OLW_TYPE ->
> implementation.hfst_ol. Any other type triggers assert(false). After
> the switch sets `is_open = false`. No return value.

> [spec:hfst:def:hfst-output-stream.hfst.hfst-output-stream.hfst-output-stream-fn]
> HfstOutputStream::HfstOutputStream

> [spec:hfst:sem:hfst-output-stream.hfst.hfst-output-stream.hfst-output-stream-fn]
> Constructor taking `filename`, `type` (ImplementationType), and
> `hfst_format_` (bool). Initializes members `type=type`,
> `hfst_format=hfst_format_`, `is_open=false`. First checks
> `HfstTransducer::is_lean_implementation_type_available(type)`; if not
> available, throws ImplementationTypeNotAvailableException (with message
> string, __FILE__, __LINE__, type). Then switches on `type` and
> allocates the matching backend output stream into the `implementation`
> union:
> - SFST_TYPE: new SfstOutputStream(filename).
> - TROPICAL_OPENFST_TYPE: if `filename` is the empty string, new
>   TropicalWeightOutputStream(hfst_format); otherwise new
>   TropicalWeightOutputStream(filename, hfst_format).
> - LOG_OPENFST_TYPE: new LogWeightOutputStream(filename).
> - FOMA_TYPE: new FomaOutputStream(filename).
> - XFSM_TYPE: sets `hfst_format = false` (XFSM writes no HFST header,
>   since its reader requires a filename), then new
>   XfsmOutputStream(filename).
> - MY_TRANSDUCER_LIBRARY_TYPE: new
>   MyTransducerLibraryOutputStream(filename, hfst_format).
> - HFST_OL_TYPE: new HfstOlOutputStream(filename, false).
> - HFST_OLW_TYPE: new HfstOlOutputStream(filename.c_str(), true).
> - any other type: throws SpecifiedTypeRequiredException.
> Each backend case is guarded by its corresponding HAVE_* compile-time
> macro. Finally sets `is_open = true`.

> [spec:hfst:def:hfst-output-stream.hfst.hfst-output-stream.stream-implementation]
> union StreamImplementation {
>   hfst::implementations::HfstOlOutputStream * hfst_ol;
> }

> [spec:hfst:def:hfst-output-stream.hfst.hfst-output-stream.write-fn]
> void HfstOutputStream::write(const char &c)

> [spec:hfst:sem:hfst-output-stream.hfst.hfst-output-stream.write-fn]
> Writes a single character `c` to the underlying backend stream.
> Switches on `type` and forwards to `write(c)` on the matching member of
> the `implementation` union: SFST_TYPE -> implementation.sfst;
> TROPICAL_OPENFST_TYPE -> implementation.tropical_ofst; LOG_OPENFST_TYPE
> -> implementation.log_ofst; FOMA_TYPE -> implementation.foma;
> MY_TRANSDUCER_LIBRARY_TYPE -> implementation.my_transducer_library;
> HFST_OL_TYPE and HFST_OLW_TYPE -> implementation.hfst_ol. For XFSM_TYPE
> it throws the C-string "operation XfsmOutputStream::write(const char &c)
> not supported" (write of a single char is unsupported for XFSM). Any
> other type triggers assert(false). No return value. (Each backend case
> is guarded by its corresponding HAVE_* macro.)

> [spec:hfst:def:hfst-output-stream.main-fn]
> int main(int argc, char * argv[])

> [spec:hfst:sem:hfst-output-stream.main-fn]
> Unit-test entry point compiled only when MAIN_TEST is defined. Prints
> "Unit tests for <__FILE__>:" followed by a newline to std::cout, then
> prints "ok" and a newline, and returns 0. Performs no actual testing.

