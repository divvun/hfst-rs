# tools/src/hfst-compare.cc

> [spec:hfst:def:hfst-compare.compare-streams-fn]
> int

> [spec:hfst:sem:hfst-compare.compare-streams-fn]
> Compares the transducers read pairwise from two input streams ('firststream',
> 'secondstream') and writes an equality log to 'outfile', returning an exit
> code. Tool option state read here: 'harmonize' (default true) and
> 'eliminate_flags' (default false).
>
> Initialise 'continueReading = firststream.is_good() && secondstream.is_good()',
> counters 'transducer_n_first = 0', 'transducer_n_second = 0', and
> 'mismatches = 0'.
>
> While 'continueReading':
> 1. Read one transducer 'first' from 'firststream'; increment 'transducer_n_first'.
> 2. If 'secondstream.is_good()', read one transducer 'second' from it and
>    increment 'transducer_n_second' (otherwise 'second' keeps its previous value,
>    so a single transducer in the second stream is reused against each first one).
> 3. Take 'firstname = first.get_name()' and 'secondname = second.get_name()'
>    (if 'second' is somehow absent this is an error: "Error: second stream has a
>    NULL value."). If 'firstname' is empty, replace it with 'firstfilename'; if
>    'secondname' is empty, replace it with 'secondfilename'.
> 4. Emit a verbose message: for the first pair "Comparing <firstname> and
>    <secondname>...", otherwise "Comparing <firstname> and <secondname>...
>    <transducer_n_first>".
> 5. In a guarded region (catching a TransducerTypeMismatchException):
>    - If 'eliminate_flags', verbose-print "Eliminating flags..." and call
>      'eliminate_flags()' on both 'first' and 'second'.
>    - Compute 'first.compare(second, harmonize)'. If equal, and not 'silent',
>      print to 'outfile': for the first pair "<firstname> == <secondname>",
>      otherwise "<firstname>[<transducer_n_first>] == <secondname>[<transducer_n_second>]".
>      If not equal, print the same with "!=" instead of "==" and increment
>      'mismatches'.
>    - On a TransducerTypeMismatchException, call 'error(2, 0, ...)' with the
>      message "Cannot compare `<firstname>' and `<secondname>' [<transducer_n_first>]
>      the formats <fmt1> and <fmt2> are not compatible for comparison", where the
>      formats are 'hfst_strformat' of each stream's type (this exits the process).
> 6. Recompute 'continueReading = firststream.is_good() && (secondstream.is_good()
>    || transducer_n_second == 1)' — i.e. keep going while the first stream has
>    more and either the second stream has more or it held exactly one transducer.
> 7. Free 'first'. Free 'second' only when NOT (continueReading and second stream
>    still good), i.e. drop the reused single second transducer once done.
>
> After the loop: if 'firststream.is_good()', call 'error(EXIT_FAILURE, 0, ...)'
> "second input '<secondfilename>' contains fewer transducers than first input
> '<firstfilename>'; this is only possible if the second input contains exactly
> one transducer". Else if 'secondstream.is_good()', call 'error(EXIT_FAILURE, 0,
> ...)' "first input '<firstfilename>' contains fewer transducers than second
> input '<secondfilename>'".
>
> Close both streams and 'fclose(outfile)'. If 'mismatches == 0', verbose-print
> "All <transducer_n_first> transducers matched" and return EXIT_SUCCESS;
> otherwise verbose-print "<mismatches>/<transducer_n_first> were not equal" and
> return EXIT_FAILURE.

> [spec:hfst:def:hfst-compare.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-compare.main-fn]
> Program entry point for the binary comparison tool.
>
> 1. Call 'hfst_set_program_name(argv[0], "0.1", "HfstCompare")'.
> 2. Call 'parse_options(argc, argv)'; if its return value is not EXIT_CONTINUE,
>    return that value.
> 3. Close the buffered input files: if 'firstfile != stdin', 'fclose(firstfile)';
>    if 'secondfile != stdin', 'fclose(secondfile)' (the tool reads via streams,
>    not these buffers).
> 4. Verbose-print "Reading from <firstfilename> and <secondfilename>, writing log
>    to <outfilename>".
> 5. Construct two HfstInputStreams: 'firststream' from 'firstfilename' if
>    'firstfile != stdin' else from stdin; likewise 'secondstream'. In C each
>    construction is wrapped in a try/catch on HfstException that calls
>    'error(EXIT_FAILURE, 0, "<name> is not a valid transducer file")'.
> 6. If either 'is_input_stream_in_ol_format(firststream, "hfst-compare")' or the
>    same for 'secondstream' is true, return EXIT_FAILURE.
> 7. Set 'retval = compare_streams(firststream, secondstream)'.
> 8. If 'outfile != stdout', 'fclose(outfile)'. Free 'firstfilename',
>    'secondfilename', 'outfilename'. Return 'retval'.
