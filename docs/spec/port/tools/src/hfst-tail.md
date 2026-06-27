# tools/src/hfst-tail.cc

> [spec:hfst:def:hfst-tail.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-tail.main-fn]
> Program entry point for hfst-tail.
> Steps:
> 1. (On Windows) set stdin (fd 0) and stdout (fd 1) to binary mode.
> 2. Call hfst_set_program_name(argv[0], "0.2", "HfstTail") to register the
>    tool name, version and wiki name.
> 3. Call parse_options(argc, argv); if its return value is not EXIT_CONTINUE,
>    return that value immediately (e.g. after --help/--version or an error).
> 4. Close the buffered FILE handles: if inputfile is not stdin, fclose it; if
>    outfile is not stdout, fclose it (the tool uses HFST streams, not the
>    FILE buffers).
> 5. verbose_printf "Reading from <inputfilename>, writing to <outfilename>".
> 6. Construct the HfstInputStream: HfstInputStream(inputfilename) when reading
>    from a named file, otherwise the default HfstInputStream() (stdin). If the
>    constructor throws HfstException, call error(EXIT_FAILURE, 0,
>    "<inputfilename> is not a valid transducer file") and return EXIT_FAILURE.
> 7. Construct the HfstOutputStream from the input stream's type:
>    HfstOutputStream(outfilename, type) when writing to a named file,
>    otherwise HfstOutputStream(type) (stdout).
> 8. Call process_stream(instream, outstream), store its result in retval.
> 9. free(inputfilename) and free(outfilename), then return retval.

> [spec:hfst:def:hfst-tail.parse-options-fn]
> int

> [spec:hfst:sem:hfst-tail.parse-options-fn]
> Parse the command-line options for hfst-tail.
> Steps:
> 1. Call extend_options_getenv(&argc, &argv) to splice in any options from the
>    environment.
> 2. Loop reading options with getopt_long. The long-option table is the common
>    long options, followed by the unary long options, followed by the
>    tool-specific option {"n-last", required_argument, 0, 'n'}, terminated by a
>    zero entry. The short-option string is HFST_GETOPT_COMMON_SHORT followed by
>    HFST_GETOPT_UNARY_SHORT followed by "n:".
> 3. When getopt_long returns -1, stop the loop.
> 4. Dispatch each option code through the case groups in order: the common
>    cases (getopt-cases-common: help, version, verbose, quiet, silent, output
>    file, input file, etc.), then the unary cases (getopt-cases-unary), then
>    the tool-specific case 'n', then the error case (getopt-cases-error) for
>    any unrecognised option. Some common/unary cases return a value (which
>    parse_options returns); others just continue the loop.
> 5. Case 'n' (--n-last=[+]K): if the option argument begins with '+', set
>    tail_count = -hfst_strtol(optarg, 10) (i.e. negate the parsed value, used
>    as the "start from the Kth" / skip mode); otherwise set
>    tail_count = hfst_strtol(optarg, 10) (the "last K" mode). hfst_strtol
>    parses with base 10.
> 6. After the loop run the common parameter checks (check-params-common) and
>    the unary parameter checks (check-params-unary).
> 7. Return EXIT_CONTINUE to signal main to proceed.

> [spec:hfst:def:hfst-tail.print-usage-fn]
> void

> [spec:hfst:sem:hfst-tail.print-usage-fn]
> Print the --help usage text to message_out.
> Steps:
> 1. Print "Usage: <program_name> [OPTIONS...] [INFILE]" followed by
>    "Get last transducers from an archive" and a blank line.
> 2. Call print_common_program_options(message_out).
> 3. Call print_common_unary_program_options(message_out).
> 4. Print the "Archive options:" block describing
>    "  -n, --n-last=[+]K   Print the last K transducers;" and
>    "                      use +K to print transducers starting from the Kth".
> 5. Print a blank line.
> 6. Call print_common_unary_program_parameter_instructions(message_out).
> 7. Print "K must be an integer, as parsed by strtoul base 10, and not 0." and
>    "if K is omitted, it defaults to +1 (all except the first)".
> 8. Print a blank line, then print_report_bugs(), then a blank line, then
>    print_more_info().

> [spec:hfst:def:hfst-tail.process-stream-fn]
> int

> [spec:hfst:sem:hfst-tail.process-stream-fn]
> Forward the trailing transducers of the input archive to the output stream.
> Maintains a FIFO queue last_n of transducers and a counter transducer_n=0.
>
> If tail_count > 0 ("last K" mode):
> 1. verbose_printf "Counting last <tail_count> transducers...".
> 2. While instream.is_good(): increment transducer_n, read a HfstTransducer
>    from instream, push it onto the back of last_n; if last_n now holds more
>    than tail_count transducers, pop (discard) the front. This keeps only the
>    last tail_count transducers.
> 3. After reading, recompute the running index: if tail_count < transducer_n,
>    set transducer_n -= (tail_count + 1); otherwise set transducer_n = 0.
> 4. While last_n is not empty: increment transducer_n, verbose_printf
>    "Forwarding <inputfilename>...<transducer_n>", write the front transducer
>    to outstream (outstream << front) and pop it.
>
> Else if tail_count < 0 ("skip / start from Kth" mode, threshold = -tail_count):
> 1. verbose_printf "Skipping <-tail_count> transducers...".
> 2. While instream.is_good(): increment transducer_n, read a HfstTransducer
>    from instream; if transducer_n >= -tail_count, verbose_printf
>    "Forwarding <inputfilename>...<transducer_n>" and write the transducer to
>    outstream. Transducers before the threshold index are read and discarded.
>
> (tail_count == 0 forwards nothing; both branches are skipped.)
>
> Finally: outstream.flush(), instream.close(), outstream.close(), and return
> EXIT_SUCCESS.
