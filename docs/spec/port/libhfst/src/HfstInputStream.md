# libhfst/src/HfstInputStream.cc, libhfst/src/HfstInputStream.h

> [spec:hfst:def:hfst-input-stream.hfst-input-stream.close-fn]
> void HfstInputStream::close(void)

> [spec:hfst:sem:hfst-input-stream.hfst-input-stream.close-fn]
> Closes the underlying backend input stream. Switches on `this->type` and
> calls `close()` on the corresponding member of the `implementation` union:
> `sfst`, `tropical_ofst`, `log_ofst`, `foma`, `xfsm`,
> `my_transducer_library` (each only when its backend is compiled in), or
> `hfst_ol` for both `HFST_OL_TYPE` and `HFST_OLW_TYPE`. For any other type
> it asserts false. Returns nothing.

> [spec:hfst:def:hfst-input-stream.hfst-input-stream.get-fst-type-old-fn]
> ImplementationType HfstInputStream::get_fst_type_old(int &bytes_read)

> [spec:hfst:sem:hfst-input-stream.hfst-input-stream.get-fst-type-old-fn]
> Reads a NUL-terminated type string from the stream via `stream_getstring()`
> into `fst_type`. If the stream is then at EOF (`stream_eof()`), throws
> `EndOfStreamException`. Otherwise compares `fst_type` against the known
> pre-release type names and, on a match, sets the out-param `bytes_read`
> and returns the matching `ImplementationType`:
> "SFST_TYPE" -> bytes_read=10, SFST_TYPE;
> "FOMA_TYPE" -> bytes_read=10, FOMA_TYPE;
> "TROPICAL_OPENFST_TYPE" -> bytes_read=19, TROPICAL_OPENFST_TYPE;
> "LOG_OPENFST_TYPE" -> bytes_read=14, LOG_OPENFST_TYPE;
> "HFST_OL_TYPE" -> bytes_read=13, HFST_OL_TYPE;
> "HFST_OLW_TYPE" -> bytes_read=14, HFST_OLW_TYPE.
> If none match, returns ERROR_TYPE (leaving bytes_read unchanged).

> [spec:hfst:def:hfst-input-stream.hfst-input-stream.get-header-data-fn]
> StringPairVector HfstInputStream::get_header_data(int header_size)

> [spec:hfst:sem:hfst-input-stream.hfst-input-stream.get-header-data-fn]
> Reads the HFST header body as a sequence of key/value string pairs until
> exactly `header_size` bytes have been consumed. Starts with an empty
> `StringPairVector retval` and `bytes_read=0`. Loops: reads two
> NUL-terminated strings `str1` and `str2` via `stream_getstring()`; adds
> `str1.length() + str2.length() + 2` (the 2 accounting for the two NUL
> terminators) to `bytes_read`. If `bytes_read > header_size`, throws
> `NotTransducerStreamException` ("more bytes read than the header
> contains"). If the stream is at EOF, throws `NotTransducerStreamException`
> ("stream ended before the header could be read"). Pushes the pair
> (str1, str2) onto `retval`. Breaks the loop when `bytes_read == header_size`.
> Returns `retval`.

> [spec:hfst:def:hfst-input-stream.hfst-input-stream.get-header-size-fn]
> int HfstInputStream::get_header_size(int &bytes_read)

> [spec:hfst:sem:hfst-input-stream.hfst-input-stream.get-header-size-fn]
> Reads the 2-byte little-endian header size followed by a NUL terminator.
> Initialises `unsigned short header_size = 0` and reads it via the
> `stream_get(unsigned short&)` overload (two bytes, low byte first). Then
> reads one more byte `c`; if `c != 0` throws `NotTransducerStreamException`
> ("header size could not be read"). Sets out-param `bytes_read = 3`.
> Returns `header_size` (as int).

> [spec:hfst:def:hfst-input-stream.hfst-input-stream.get-type-fn]
> ImplementationType HfstInputStream::get_type(void) const

> [spec:hfst:sem:hfst-input-stream.hfst-input-stream.get-type-fn]
> Const getter. Returns the `type` member (the stream's `ImplementationType`).
> No side effects.

> [spec:hfst:def:hfst-input-stream.hfst-input-stream.hfst-input-stream-fn]
> HfstInputStream::HfstInputStream(const std::string &filename)

> [spec:hfst:sem:hfst-input-stream.hfst-input-stream.hfst-input-stream-fn]
> Constructor that opens a stream from file `filename` (or stdin if filename
> is empty). Member-initialises `bytes_to_skip=0`, `filename=filename`,
> `has_hfst_header=false`, `hfst_version_2_weighted_transducer=false`.
> If `filename` is non-empty: opens a `std::ifstream` on it; if the open
> fails (`ifs.fail()`) throws `NotTransducerStreamException` ("file could not
> be opened"); sets `input_stream` to point at that ifstream. If empty: sets
> `input_stream = &std::cin`. In both cases, if `stream_eof()` throws
> `EndOfStreamException`, then sets `type = stream_fst_type()` (which sniffs
> the header/magic and may set `has_hfst_header`/`bytes_to_skip`).
> If `HfstTransducer::is_lean_implementation_type_available(type)` is false,
> throws `ImplementationTypeNotAvailableException`. Then switches on `type`
> and allocates the matching backend input-stream object into the
> `implementation` union, passing `filename`: SFST_TYPE ->
> `SfstInputStream(filename)`; TROPICAL_OPENFST_TYPE ->
> `TropicalWeightInputStream(filename)` (or no-arg ctor when filename is
> empty); LOG_OPENFST_TYPE -> `LogWeightInputStream(filename)`; FOMA_TYPE ->
> `FomaInputStream(filename)`; XFSM_TYPE -> `XfsmInputStream(filename)`;
> MY_TRANSDUCER_LIBRARY_TYPE -> its input stream; HFST_OL_TYPE ->
> `HfstOlInputStream(filename, false)`; HFST_OLW_TYPE ->
> `HfstOlInputStream(filename, true)`. Each backend case is compiled only
> when its backend is available. For any unrecognised type, throws
> `NotTransducerStreamException` ("transducer type not recognised").

> [spec:hfst:def:hfst-input-stream.hfst-input-stream.is-bad-fn]
> bool HfstInputStream::is_bad(void)

> [spec:hfst:sem:hfst-input-stream.hfst-input-stream.is-bad-fn]
> Returns whether the backend stream's badbit is set. Switches on `type` and
> returns `is_bad()` of the corresponding `implementation` union member
> (`sfst`, `tropical_ofst`, `log_ofst`, `foma`, `xfsm`,
> `my_transducer_library`, or `hfst_ol` for both HFST_OL_TYPE and
> HFST_OLW_TYPE). For any other type, asserts false and returns false.

> [spec:hfst:def:hfst-input-stream.hfst-input-stream.is-eof-fn]
> bool HfstInputStream::is_eof(void)

> [spec:hfst:sem:hfst-input-stream.hfst-input-stream.is-eof-fn]
> Returns whether the backend stream is at end-of-file. Switches on `type`
> and returns `is_eof()` of the corresponding `implementation` union member
> (`sfst`, `tropical_ofst`, `log_ofst`, `foma`, `xfsm`,
> `my_transducer_library`, or `hfst_ol` for both HFST_OL_TYPE and
> HFST_OLW_TYPE). For any other type, asserts false and returns false.

> [spec:hfst:def:hfst-input-stream.hfst-input-stream.is-good-fn]
> bool HfstInputStream::is_good(void)

> [spec:hfst:sem:hfst-input-stream.hfst-input-stream.is-good-fn]
> Returns whether the backend stream is in a good state for input. Switches
> on `type` and returns `is_good()` of the corresponding `implementation`
> union member (`sfst`, `tropical_ofst`, `log_ofst`, `foma`, `xfsm`,
> `my_transducer_library`, or `hfst_ol` for both HFST_OL_TYPE and
> HFST_OLW_TYPE). For any other type, asserts false and returns false.

> [spec:hfst:def:hfst-input-stream.hfst-input-stream.is-hfst-header-included-fn]
> bool HfstInputStream::is_hfst_header_included(void) const

> [spec:hfst:sem:hfst-input-stream.hfst-input-stream.is-hfst-header-included-fn]
> Const getter. Returns the `has_hfst_header` member (whether an HFST header
> was found/read for this stream). No side effects.

> [spec:hfst:def:hfst-input-stream.hfst-input-stream.read-hfst-header-fn]
> bool HfstInputStream::read_hfst_header(int &bytes_read)

> [spec:hfst:sem:hfst-input-stream.hfst-input-stream.read-hfst-header-fn]
> Tries to read an HFST header from the stream. Peeks the first byte with
> `stream_peek()`; if it is not 'H', sets `bytes_read=0` and returns false
> (no bytes consumed). Otherwise attempts the modern (version 3.0) header:
> calls `read_library_header(header_bytes)` to match the literal "HFST"; if
> it matches, reads the header size via `get_header_size(size_bytes)` (may
> throw), reads the header pairs via `get_header_data(header_size)`, and
> processes them via `process_header_data(header_info, false)` (may throw a
> TransducerHeaderException, also sets `type`/`name`/`props`). Sets
> `bytes_read = header_bytes + size_bytes + header_size` and returns true.
> If the modern library header did not match, resets `header_bytes=0` and
> tries the pre-release header: `read_library_header_old(header_bytes)`
> matching literal "HFST3"; if it matches, sets `type =
> get_fst_type_old(type_bytes)`, and if that returns ERROR_TYPE throws
> `NotTransducerStreamException`; otherwise sets `bytes_read = header_bytes +
> type_bytes` and returns true. If neither header matches, returns false.

> [spec:hfst:def:hfst-input-stream.hfst-input-stream.read-library-header-fn]
> bool HfstInputStream::read_library_header(int &bytes_read)

> [spec:hfst:sem:hfst-input-stream.hfst-input-stream.read-library-header-fn]
> Tries to consume the NUL-terminated library id string "HFST" (the C
> literal `id = "HFST"`, iterated for indices 0..4 inclusive, i.e. the four
> letters plus the trailing '\0'). For each `i` in 0..4, reads one byte `c`
> via `stream_get()`; if `c != id[i]`, the match fails: pushes `c` back with
> `stream_unget(c)`, and if `i > 0` pushes back the previously matched bytes
> `id[i-1] .. id[0]` in reverse order to fully restore the stream, sets
> `bytes_read=0`, and returns false. If all 5 bytes match, sets
> `bytes_read=5` and returns true.

> [spec:hfst:def:hfst-input-stream.hfst-input-stream.read-library-header-old-fn]
> bool HfstInputStream::read_library_header_old(int &bytes_read)

> [spec:hfst:sem:hfst-input-stream.hfst-input-stream.read-library-header-old-fn]
> Tries to consume the NUL-terminated pre-release library id string "HFST3"
> (the C literal `id = "HFST3"`, iterated for indices 0..5 inclusive, i.e.
> the five characters plus the trailing '\0'). For each `i` in 0..5, reads
> one byte `c` via `stream_get()`; if `c != id[i]`, the match fails: pushes
> `c` back with `stream_unget(c)`, and if `i > 0` pushes back the previously
> matched bytes `id[i-1] .. id[0]` in reverse order to restore the stream,
> sets `bytes_read=0`, and returns false. If all 6 bytes match, sets
> `bytes_read=6` and returns true.

> [spec:hfst:def:hfst-input-stream.hfst-input-stream.stream-fst-type-fn]
> ImplementationType HfstInputStream::stream_fst_type()

> [spec:hfst:sem:hfst-input-stream.hfst-input-stream.stream-fst-type-fn]
> Determines the implementation type of the first transducer in the stream.
> Initialises `bytes_read=0`. First calls `read_hfst_header(bytes_read)`: if
> it succeeds, sets `has_hfst_header=true`, `bytes_to_skip=bytes_read`, and
> returns the already-set `type`. Otherwise calls
> `guess_fst_type(bytes_read)` to sniff a native/legacy format, sets
> `bytes_to_skip=bytes_read`, and maps the returned TransducerType to an
> ImplementationType: HFST_VERSION_2_WEIGHTED -> sets
> `hfst_version_2_weighted_transducer=true` and returns
> TROPICAL_OPENFST_TYPE; HFST_VERSION_2_UNWEIGHTED_WITHOUT_ALPHABET -> prints
> an error to stderr about a version-2 transducer with no alphabet and
> returns ERROR_TYPE; HFST_VERSION_2_UNWEIGHTED -> SFST_TYPE; OPENFST_TROPICAL_
> -> TROPICAL_OPENFST_TYPE; OPENFST_LOG_ -> LOG_OPENFST_TYPE; SFST_ ->
> SFST_TYPE; FOMA_ -> FOMA_TYPE; XFSM_ -> XFSM_TYPE; MY_TRANSDUCER_LIBRARY_
> -> MY_TRANSDUCER_LIBRARY_TYPE (when compiled in); ERROR_TYPE_ or anything
> else -> ERROR_TYPE.

> [spec:hfst:def:hfst-input-stream.hfst.debug-error-fn]
> void debug_error(const char *msg)

> [spec:hfst:sem:hfst-input-stream.hfst.debug-error-fn]
> Free function in namespace `hfst`. When compiled with
> `PRINT_DEBUG_MESSAGES`, writes `msg` followed by a newline to stderr via
> `fprintf(stderr, "%s\n", msg)`. Otherwise it is a no-op (the parameter is
> cast to void to suppress the unused-parameter warning). Returns nothing.

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream]
> class HfstInputStream {
>   union StreamImplementation { #if HAVE_SFST || HAVE_LEAN_SFST hfst::implementations::SfstInputStream * sfst; #endif #if HAVE_OPENFST hfst::implementations::Tr...;
>   StreamImplementation implementation;
>   ImplementationType type;
>   std::string name;
>   std::map<std::string,std::string> props;
>   unsigned int bytes_to_skip;
>   std::string filename;
>   bool has_hfst_header;
>   bool hfst_version_2_weighted_transducer;
>   std::istream * input_stream;
>   char &stream_get(char &c);
>   short &stream_get(short &i);
>   unsigned short &stream_get(unsigned short &i);
>   enum TransducerType { /* See the above variable. */ HFST_VERSION_2_WEIGHTED, /* An SFST transducer with no alphabet, not supported. */ HFST_VERSION_2_UNWEIGH...;
>   HFSTDLL ImplementationType;
> }

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.close-fn]
> HFSTDLL void close(void)

> [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.close-fn]
> Member declaration of `close()`; behavior is identical to
> `[spec:hfst:sem:hfst-input-stream.hfst-input-stream.close-fn]`. Closes the
> underlying backend input stream by switching on `type` and calling
> `close()` on the matching `implementation` union member (`sfst`,
> `tropical_ofst`, `log_ofst`, `foma`, `xfsm`, `my_transducer_library`, or
> `hfst_ol` for HFST_OL_TYPE/HFST_OLW_TYPE); asserts false for any other
> type. Per the header doc, if the stream points to standard input nothing
> meaningful is done. Returns nothing.

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.get-fst-type-old-fn]
> ImplementationType get_fst_type_old(int &bytes_read)

> [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.get-fst-type-old-fn]
> Member declaration of `get_fst_type_old(int &bytes_read)`; behavior is
> identical to
> `[spec:hfst:sem:hfst-input-stream.hfst-input-stream.get-fst-type-old-fn]`.
> Reads a NUL-terminated type string via `stream_getstring()`; if at EOF
> afterwards, throws `EndOfStreamException`. On matching a known pre-release
> type name, sets `bytes_read` and returns the type ("SFST_TYPE"->10/SFST_TYPE,
> "FOMA_TYPE"->10/FOMA_TYPE, "TROPICAL_OPENFST_TYPE"->19/TROPICAL_OPENFST_TYPE,
> "LOG_OPENFST_TYPE"->14/LOG_OPENFST_TYPE, "HFST_OL_TYPE"->13/HFST_OL_TYPE,
> "HFST_OLW_TYPE"->14/HFST_OLW_TYPE); otherwise returns ERROR_TYPE.

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.get-header-data-fn]
> StringPairVector get_header_data(int header_size)

> [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.get-header-data-fn]
> Member declaration of `get_header_data(int header_size)`; behavior is
> identical to
> `[spec:hfst:sem:hfst-input-stream.hfst-input-stream.get-header-data-fn]`.
> Reads key/value string pairs until exactly `header_size` bytes are
> consumed: repeatedly reads two NUL-terminated strings, accumulates
> `len(str1)+len(str2)+2` into a running byte count, throws
> `NotTransducerStreamException` if the count exceeds `header_size` or if the
> stream hits EOF mid-header, appends each pair, and stops when the count
> equals `header_size`. Returns the `StringPairVector` of pairs.

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.get-header-size-fn]
> int get_header_size(int &bytes_read)

> [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.get-header-size-fn]
> Member declaration of `get_header_size(int &bytes_read)`; behavior is
> identical to
> `[spec:hfst:sem:hfst-input-stream.hfst-input-stream.get-header-size-fn]`.
> Reads a 2-byte little-endian `unsigned short` header size (via the
> `stream_get(unsigned short&)` overload), then one terminator byte that must
> be 0 (else throws `NotTransducerStreamException`), sets `bytes_read=3`, and
> returns the header size.

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.get-type-fn]
> get_type(void) const

> [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.get-type-fn]
> Member declaration of `get_type() const`; behavior is identical to
> `[spec:hfst:sem:hfst-input-stream.hfst-input-stream.get-type-fn]`. Returns
> the `type` member (the stream's ImplementationType), with no side effects.
> Per the header doc, all transducers in a stream are expected to share this
> type.

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.guess-fst-type-fn]
> HfstInputStream::TransducerType HfstInputStream::guess_fst_type

> [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.guess-fst-type-fn]
> Sniffs a native/legacy transducer format from leading bytes and returns a
> TransducerType. Sets out-param `bytes_read=0`, peeks one byte `c` via
> `stream_peek()`, and switches on it:
> - 0xd6 (OpenFst magic): reads 26 bytes into a buffer (throwing
>   EndOfStreamException if EOF is hit during the read), then ungets all 26
>   in reverse to restore the stream. If buffer[18]=='s' returns
>   OPENFST_TROPICAL_; if 'l' returns OPENFST_LOG_; otherwise throws
>   NotTransducerStreamException.
> - '#': returns FOMA_.
> - 0x1f (possible gzip magic 1F 8B 08): reads three bytes c0,c1,c2 (throwing
>   EndOfStreamException on EOF), ungets them in reverse; if they equal
>   0x1f,0x8b,0x08 throws FileIsInGZFormatException, else throws
>   NotTransducerStreamException.
> - 'a': returns SFST_.
> - 'm' (only when MY_TRANSDUCER_LIBRARY is compiled in): returns
>   MY_TRANSDUCER_LIBRARY_.
> - 'P' (HFST version 2 header): sets `has_hfst_header=false`, consumes 4
>   bytes, sets `bytes_read=4`, reads a 5th byte c5; if c5=='A' returns
>   HFST_VERSION_2_UNWEIGHTED_WITHOUT_ALPHABET; if c5=='a' ungets c5 and
>   returns HFST_VERSION_2_UNWEIGHTED; otherwise throws
>   NotTransducerStreamException.
> - 'A': sets `has_hfst_header=true`, consumes 1 byte, sets `bytes_read=1`,
>   peeks next byte c2; if c2=='a' returns
>   HFST_VERSION_2_UNWEIGHTED_WITHOUT_ALPHABET; if c2==0xd6 returns
>   HFST_VERSION_2_WEIGHTED; otherwise returns ERROR_TYPE_.
> - 0x00: returns XFSM_.
> - default: returns ERROR_TYPE_.

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.hfst-input-stream-fn]
> HFSTDLL HfstInputStream(void)

> [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.hfst-input-stream-fn]
> Default constructor `HfstInputStream(void)`: opens a transducer stream on
> standard input. Member-initialises `bytes_to_skip=0`, `filename=""`,
> `has_hfst_header=false`, `hfst_version_2_weighted_transducer=false`. Sets
> `input_stream = &std::cin`. If `stream_eof()` throws `EndOfStreamException`.
> Sets `type = stream_fst_type()` (sniffs header/magic, may set
> `has_hfst_header`/`bytes_to_skip`). If
> `HfstTransducer::is_lean_implementation_type_available(type)` is false,
> throws `ImplementationTypeNotAvailableException`. Then switches on `type`
> and allocates the matching backend input-stream object (with default/no-arg
> constructors) into the `implementation` union: SFST_TYPE ->
> `SfstInputStream`; TROPICAL_OPENFST_TYPE -> `TropicalWeightInputStream`;
> LOG_OPENFST_TYPE -> `LogWeightInputStream`; FOMA_TYPE -> `FomaInputStream`;
> XFSM_TYPE -> `XfsmInputStream` (which itself errors); MY_TRANSDUCER_LIBRARY_TYPE
> -> its input stream; HFST_OL_TYPE -> `HfstOlInputStream(false)`;
> HFST_OLW_TYPE -> `HfstOlInputStream(true)`. Each backend case is compiled
> only when available. For any unrecognised type, throws
> `NotTransducerStreamException` ("transducer type not recognised").

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.ignore-fn]
> void HfstInputStream::ignore(unsigned int n)

> [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.ignore-fn]
> Skips/discards `n` bytes from the underlying backend stream. Switches on
> `type` and calls `ignore(n)` on the corresponding `implementation` union
> member (`sfst`, `tropical_ofst`, `log_ofst`, `foma`,
> `my_transducer_library`, or `hfst_ol` for HFST_OL_TYPE/HFST_OLW_TYPE).
> For any other type, asserts false. Returns nothing.

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.is-bad-fn]
> HFSTDLL bool is_bad(void)

> [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.is-bad-fn]
> Member declaration of `is_bad()`; behavior is identical to
> `[spec:hfst:sem:hfst-input-stream.hfst-input-stream.is-bad-fn]`. Returns
> whether the backend stream's badbit is set, by switching on `type` and
> returning `is_bad()` of the matching `implementation` union member;
> asserts false and returns false for any other type.

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.is-eof-fn]
> HFSTDLL bool is_eof(void)

> [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.is-eof-fn]
> Member declaration of `is_eof()`; behavior is identical to
> `[spec:hfst:sem:hfst-input-stream.hfst-input-stream.is-eof-fn]`. Returns
> whether the backend stream is at end-of-file, by switching on `type` and
> returning `is_eof()` of the matching `implementation` union member;
> asserts false and returns false for any other type.

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.is-good-fn]
> HFSTDLL bool is_good(void)

> [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.is-good-fn]
> Member declaration of `is_good()`; behavior is identical to
> `[spec:hfst:sem:hfst-input-stream.hfst-input-stream.is-good-fn]`. Returns
> whether the backend stream is in a good state for input, by switching on
> `type` and returning `is_good()` of the matching `implementation` union
> member; asserts false and returns false for any other type.

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.is-hfst-header-included-fn]
> HFSTDLL bool is_hfst_header_included(void) const

> [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.is-hfst-header-included-fn]
> Member declaration of `is_hfst_header_included() const`; behavior is
> identical to
> `[spec:hfst:sem:hfst-input-stream.hfst-input-stream.is-hfst-header-included-fn]`.
> Returns the `has_hfst_header` member with no side effects.

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.process-header-data-fn]
> void HfstInputStream::process_header_data

> [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.process-header-data-fn]
> Validates and applies a parsed HFST header (`StringPairVector
> &header_data`; the second bool parameter is unused). If `header_data` has
> fewer than 2 pairs, throws `TransducerHeaderException` ("too few
> attributes"). Requires pair[0] to be key "version" with value "3.0" or
> "3.3" (else throws TransducerHeaderException "version not recognised").
> Requires pair[1] key to be "type" (else throws "type not given"). Maps
> pair[1]'s value to the `type` member: "SFST"->SFST_TYPE; "FOMA"->FOMA_TYPE;
> "TROPICAL_OPENFST"/"TROPICAL_OFST"->TROPICAL_OPENFST_TYPE;
> "LOG_OPENFST"/"LOG_OFST"->LOG_OPENFST_TYPE;
> "MY_TRANSDUCER_LIBRARY"->MY_TRANSDUCER_LIBRARY_TYPE (when compiled in);
> "HFST_OL"->HFST_OL_TYPE; "HFST_OLW"->HFST_OLW_TYPE; otherwise throws
> TransducerHeaderException "type not recognised". If there are exactly 2
> pairs, returns. Otherwise, if pair[2]'s key is "name", sets the `name`
> member to its value. Finally copies every pair in `header_data` into the
> `props` map (`props[key] = value`). Returns nothing.

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.read-hfst-header-fn]
> bool read_hfst_header(int &bytes_read)

> [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.read-hfst-header-fn]
> Member declaration of `read_hfst_header(int &bytes_read)`; behavior is
> identical to
> `[spec:hfst:sem:hfst-input-stream.hfst-input-stream.read-hfst-header-fn]`.
> Peeks for 'H' (returns false with bytes_read=0 if absent). Tries the modern
> "HFST" library header then size + header pairs + `process_header_data`,
> returning true with `bytes_read = header_bytes + size_bytes + header_size`.
> Otherwise tries the old "HFST3" header then `get_fst_type_old` (setting
> `type`, throwing NotTransducerStreamException on ERROR_TYPE), returning true
> with `bytes_read = header_bytes + type_bytes`. Returns false if neither
> matches.

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.read-library-header-fn]
> bool read_library_header(int &bytes_read)

> [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.read-library-header-fn]
> Member declaration of `read_library_header(int &bytes_read)`; behavior is
> identical to
> `[spec:hfst:sem:hfst-input-stream.hfst-input-stream.read-library-header-fn]`.
> Tries to consume the NUL-terminated id "HFST" (5 bytes including the
> trailing '\0'). On the first mismatch, ungets the read byte plus all
> previously matched bytes (in reverse) to restore the stream, sets
> bytes_read=0, returns false. On full match sets bytes_read=5, returns true.

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.read-library-header-old-fn]
> bool read_library_header_old(int &bytes_read)

> [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.read-library-header-old-fn]
> Member declaration of `read_library_header_old(int &bytes_read)`; behavior is
> identical to
> `[spec:hfst:sem:hfst-input-stream.hfst-input-stream.read-library-header-old-fn]`.
> Uses the C literal `id = "HFST3"` and loops `i` from 0 to 5 inclusive (6
> iterations, the five characters plus the trailing '\0'). On each iteration
> reads one byte `c` via `stream_get()`; if `c != id[i]`, ungets `c` via
> `stream_unget(c)`, and when `i > 0` ungets the previously matched bytes
> `id[i-1]..id[0]` in reverse to restore the stream, sets `bytes_read=0`, and
> returns false. If all 6 bytes match, sets `bytes_read=6` and returns true.

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.read-transducer-fn]
> void HfstInputStream::read_transducer(HfstTransducer &t)

> [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.read-transducer-fn]
> Reads the next transducer from the stream into `t`. Preamble (skipped
> entirely when `type == XFSM_TYPE`): if `input_stream != NULL` this is the
> first transducer, so set `input_stream = NULL`, throw `EndOfStreamException`
> if `stream_eof()`, and if `filename` is non-empty call `ignore(bytes_to_skip)`
> to skip the already-read header bytes. Otherwise (not the first) throw
> `EndOfStreamException` if `stream_eof()`, then compare `get_type()` against
> `stream_fst_type()`; if they differ throw `TransducerTypeMismatchException`
> ("HfstInputStream contains HfstTransducers whose type is not the same").
> Then switch on `type` and read the backend transducer into the matching
> `t.implementation` union member by calling the backend stream's
> `read_transducer()`. For SFST_TYPE, TROPICAL_OPENFST_TYPE and LOG_OPENFST_TYPE,
> when `has_hfst_header` is false the just-read transducer is round-tripped
> through an `HfstBasicTransducer` (via the matching ConversionFunctions
> `*_to_hfst_basic_transducer` then `hfst_basic_transducer_to_*`, deleting the
> intermediate net) to normalise epsilon/unknown/identity symbol coding.
> For TROPICAL_OPENFST_TYPE only, if `hfst_version_2_weighted_transducer` is
> true, parse the trailing SFST-style alphabet: read one UTF8 byte, then read a
> 2-byte little-endian count `n` (low byte first); for each of `n` entries read
> a 2-byte little-endian symbol number and a NUL-terminated symbol string,
> classifying numbers 0/1/2 into `special_cases` and all others into
> `symbol_mappings` while incrementing a `max_number` counter; then for each
> special case other than number 0, substitute that symbol number with a fresh
> `++max_number` in the transducer and append (max_number, string) to
> `symbol_mappings`; set the transducer's symbol table to `symbol_mappings`;
> finally read a 2-byte little-endian count and skip 4 times that many bytes
> (the character pairs) via repeated `stream_get()`. For LOG_OPENFST_TYPE, if
> `hfst_version_2_weighted_transducer` is true throw `HfstFatalException`
> ("not transducer stream"). For FOMA_TYPE, XFSM_TYPE and
> MY_TRANSDUCER_LIBRARY_TYPE just call the backend `read_transducer()`. For
> HFST_OL_TYPE/HFST_OLW_TYPE call `hfst_ol->read_transducer(false)` and, if
> `t.get_type() != type`, call `t.convert(type)` to add/remove weights. For
> ERROR_TYPE or any unrecognised type, call `debug_error("#1")` and throw
> `NotTransducerStreamException`. Finally, unless `type == XFSM_TYPE`, set the
> transducer name via `t.set_name(name)` and copy every entry of the `props`
> map onto the transducer via `t.set_property(key, value)`. Returns nothing.

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.set-implementation-specific-header-data-fn]
> bool HfstInputStream::set_implementation_specific_header_data

> [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.set-implementation-specific-header-data-fn]
> Stub that applies backend-specific header data. Both parameters
> (`StringPairVector& data`, `unsigned int index`) are unused. When SFST is
> compiled in, it switches on `type` but the SFST_TYPE branch's actual call is
> commented out, so every branch falls through. Always returns false (no state
> is read or mutated).

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.stream-eof-fn]
> bool HfstInputStream::stream_eof()

> [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.stream-eof-fn]
> Returns whether the stream is at end-of-file. If `input_stream != 0` (the raw
> std::istream is still in use, before the first transducer is read), returns
> `input_stream->eof()`. Otherwise (a backend stream has taken over) delegates
> to `is_eof()`, which switches on `type` and queries the matching backend
> `implementation` union member.

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.stream-fst-type-fn]
> ImplementationType stream_fst_type()

> [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.stream-fst-type-fn]
> Member declaration of `stream_fst_type()`; behavior is identical to
> `[spec:hfst:sem:hfst-input-stream.hfst-input-stream.stream-fst-type-fn]`.
> Initialises `bytes_read=0`. First tries `read_hfst_header(bytes_read)`: on
> success sets `has_hfst_header=true`, `bytes_to_skip=bytes_read`, and returns
> the already-set `type`. Otherwise calls `guess_fst_type(bytes_read)`, sets
> `bytes_to_skip=bytes_read`, and maps the resulting TransducerType to an
> ImplementationType: HFST_VERSION_2_WEIGHTED -> sets
> `hfst_version_2_weighted_transducer=true` and returns TROPICAL_OPENFST_TYPE;
> HFST_VERSION_2_UNWEIGHTED_WITHOUT_ALPHABET -> prints a stderr error about a
> version-2 transducer with no alphabet and returns ERROR_TYPE;
> HFST_VERSION_2_UNWEIGHTED -> SFST_TYPE; OPENFST_TROPICAL_ ->
> TROPICAL_OPENFST_TYPE; OPENFST_LOG_ -> LOG_OPENFST_TYPE; SFST_ -> SFST_TYPE;
> FOMA_ -> FOMA_TYPE; XFSM_ -> XFSM_TYPE; MY_TRANSDUCER_LIBRARY_ ->
> MY_TRANSDUCER_LIBRARY_TYPE (when compiled in); ERROR_TYPE_ or anything else
> -> ERROR_TYPE.

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.stream-get-fn]
> char HfstInputStream::stream_get()

> [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.stream-get-fn]
> Reads and consumes one byte from the stream, returned as a `char`. If
> `input_stream != NULL` (raw istream still in use, before the first
> transducer), returns `(char) input_stream->get()`. Otherwise switches on
> `type` and returns `stream_get()` of the matching backend `implementation`
> union member (`sfst`, `tropical_ofst`, `log_ofst`, `foma`,
> `my_transducer_library`, or `hfst_ol` for HFST_OL_TYPE/HFST_OLW_TYPE). For
> any other type, asserts false; if execution reaches the end (compiler
> fallthrough) throws `HfstFatalException` ("stream_get() failed").

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.stream-getstring-fn]
> std::string HfstInputStream::stream_getstring()

> [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.stream-getstring-fn]
> Reads a NUL-terminated string from the stream and returns it (without the
> terminator). Starts with an empty `std::string retval`. Loops indefinitely:
> reads one byte `c` via `stream_get()`; if `stream_eof()` is then true, throws
> `EndOfStreamException`; if `c == '\0'` breaks the loop; otherwise appends `c`
> to `retval`. Returns `retval`.

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.stream-implementation]
> union StreamImplementation {
>   hfst::implementations::HfstOlInputStream * hfst_ol;
> }

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.stream-peek-fn]
> char HfstInputStream::stream_peek()

> [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.stream-peek-fn]
> Returns the next byte without consuming it. Reads one byte `c` via
> `stream_get()`, immediately pushes it back via `stream_unget(c)`, and returns
> `c`. Net effect leaves the stream position unchanged.

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.stream-unget-fn]
> void HfstInputStream::stream_unget(char c)

> [spec:hfst:sem:hfst-input-stream.hfst.hfst-input-stream.stream-unget-fn]
> Pushes the byte `c` back onto the stream so the next read returns it. If
> `input_stream != NULL` (raw istream still in use), calls
> `input_stream->putback(c)` and returns. Otherwise switches on `type` and
> calls `stream_unget(c)` on the matching backend `implementation` union member
> (`sfst`, `tropical_ofst`, `log_ofst`, `foma`, `my_transducer_library`, or
> `hfst_ol` for HFST_OL_TYPE/HFST_OLW_TYPE). For any other type, asserts false.
> Returns nothing.

> [spec:hfst:def:hfst-input-stream.hfst.hfst-input-stream.transducer-type]
> enum TransducerType {
>   HFST_VERSION_2_WEIGHTED;
>   HFST_VERSION_2_UNWEIGHTED_WITHOUT_ALPHABET;
>   HFST_VERSION_2_UNWEIGHTED;
>   OPENFST_TROPICAL_;
>   OPENFST_LOG_;
>   SFST_;
>   FOMA_;
>   XFSM_;
>   ERROR_TYPE_;
> }

> [spec:hfst:def:hfst-input-stream.main-fn]
> int main(int argc, char * argv[])

> [spec:hfst:sem:hfst-input-stream.main-fn]
> Unit-test entry point, compiled only when `MAIN_TEST` is defined. Prints
> "Unit tests for " followed by the source file name (`__FILE__`) and a newline
> to stdout, then prints "ok" and a newline to stdout, and returns 0. Ignores
> `argc`/`argv`.

