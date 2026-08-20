# tools/src/hfst-disjunct.cc

> [spec:hfst:def:hfst-disjunct.disjunct-streams-fn]
> int

> [spec:hfst:sem:hfst-disjunct.disjunct-streams-fn]
> Disjuncts (unions) transducers pairwise from two input streams, writing each
> result to the output stream. Steps:
> 1. Set continueReading = firststream.is_good() && secondstream.is_good().
> 2. Read each stream's implementation type. If the two types differ:
>    - If transducer conversion is allowed (the default), call
>      conversion_type(type1, type2). ct==1 selects type1 ("using former type as
>      output"), ct==2 selects type2 ("using latter type as output"), ct==-1
>      selects type1 with "loss of information is possible"; any other value is a
>      should-not-happen error. Emit a warning describing the mismatch and the
>      chosen output type.
>    - Otherwise call error(EXIT_FAILURE) reporting that the two formats are not
>      compatible for disjunction (--do-not-convert was requested).
>    If the types are equal, output_type = type1.
> 3. Open an HfstOutputStream of output_type: named (outfilename) when outfile is
>    not stdout, otherwise the stdout stream; both with the default hfst_format
>    (true).
> 4. Loop while continueReading, keeping counts transducer_n_first and
>    transducer_n_second:
>    - Read one transducer 'first' from firststream; increment
>      transducer_n_first.
>    - If secondstream is_good(), read one 'second' from it; increment
>      transducer_n_second.
>    - Get firstname = hfst_get_name(first, firstfilename). If 'second' is absent
>      this is a should-not-happen error ("Error: second stream has a NULL
>      value."). Get secondname = hfst_get_name(second, secondfilename).
>    - Verbose-print "Disjuncting <firstname> and <secondname>...\n" on the first
>      iteration, else with a trailing " <transducer_n_first>".
>    - Try first->disjunct(second, harmonize). On a
>      TransducerTypeMismatchException: if conversion is allowed, call
>      convert_transducers(first, second) and retry disjunct(second, harmonize);
>      otherwise error(EXIT_FAILURE) reporting incompatible formats.
>    - Set the result's metadata: hfst_set_name(first, first, second, "union")
>      and hfst_set_formula(first, first, second, "\u{222a}").
>    - Write first to the output stream.
>    - Recompute continueReading = firststream.is_good() &&
>      (secondstream.is_good() || transducer_n_second == 1).
>    - Delete 'first'. Delete 'second' unless continuing to read the first stream
>      while there is exactly one transducer in the second stream (i.e. delete it
>      when (continueReading && secondstream.is_good()) or !continueReading).
>    - Flush the output stream.
> 5. After the loop: if firststream is still good, error(EXIT_FAILURE) because
>    the second input has fewer transducers than the first (only valid when the
>    second holds exactly one). If secondstream is still good, error(EXIT_FAILURE)
>    because the first input has fewer transducers than the second.
> 6. Close both input streams and the output stream; return EXIT_SUCCESS.

> [spec:hfst:def:hfst-disjunct.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-disjunct.main-fn]
> Program entry point. Steps:
> 1. (On Windows, set stdin/stdout to binary mode.)
> 2. hfst_set_program_name(argv[0], "0.1", "HfstDisjunct").
> 3. retval = parse_options(argc, argv); if retval != EXIT_CONTINUE, return it.
> 4. Close the buffered FILE handles since streams are used: fclose firstfile
>    unless it is stdin, fclose secondfile unless stdin, fclose outfile unless
>    stdout.
> 5. Verbose-print "Reading from <firstfilename> and <secondfilename>, writing to
>    <outfilename>\n".
> 6. Construct firststream: an HfstInputStream over firstfilename when firstfile
>    is not stdin, else the default (stdin) stream; on HfstException,
>    error(EXIT_FAILURE) "<firstfilename> is not a valid transducer file".
>    Construct secondstream the same way for secondfilename/secondfile.
> 7. If either stream is in optimized-lookup format
>    (is_input_stream_in_ol_format(.., "hfst-disjunct")), return EXIT_FAILURE.
> 8. retval = disjunct_streams(firststream, secondstream). Free firstfilename,
>    secondfilename, outfilename. Return retval.

> [spec:hfst:def:hfst-disjunct.parse-options-fn]
> int

> [spec:hfst:sem:hfst-disjunct.parse-options-fn]
> Parses command-line options. Steps:
> 1. extend_options_getenv(&argc, &argv) to fold in environment-provided options.
> 2. Loop over getopt_long using the concatenation of the common long options,
>    the binary long options, the tool option {"do-not-harmonize", no_argument,
>    0, 'H'}, and a NULL terminator; the short-option string is
>    HFST_GETOPT_COMMON_SHORT HFST_GETOPT_BINARY_SHORT "H". Break when getopt_long
>    returns -1.
> 3. Dispatch each returned option through the case groups in order: common cases,
>    then binary cases, then the tool case 'H' (which sets harmonize=false), then
>    the terminal error case. The common/binary cases may continue the loop or
>    return an exit code (e.g. for --help/--version).
> 4. After the loop, run the common parameter checks and the binary parameter
>    checks (which resolve firstfile/secondfile/outfile and their names).
> 5. Return EXIT_CONTINUE.
>
> Note: the usage text advertises -F/--harmonize-flags, but this option is not
> wired into getopt here; the harmonize_flags static stays false.
