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

> [spec:hfst:def:hfst-compare.parse-options-fn]
> int

> [spec:hfst:sem:hfst-compare.parse-options-fn]
> Parses command-line options for the binary comparison tool, setting global tool
> state, and returns either an exit code or EXIT_CONTINUE (to proceed).
>
> First call 'extend_options_getenv(&argc, &argv)'. Then loop calling
> 'getopt_long' with the long-option table built from HFST_GETOPT_COMMON_LONG,
> HFST_GETOPT_BINARY_LONG, plus the tool's own options
> {"do-not-harmonize", no_argument, 0, 'H'} and {"eliminate-flags", no_argument,
> 0, 'e'} (terminated by the zero option), and the short-option string
> HFST_GETOPT_COMMON_SHORT HFST_GETOPT_BINARY_SHORT "He".
>
> When 'getopt_long' returns -1, break out of the loop. Otherwise dispatch the
> returned option character through the case groups in order: the common cases,
> then the binary cases, then the tool's own cases — 'H' sets 'harmonize = false',
> 'e' sets 'eliminate_flags = true' (each then continues the loop) — then the
> terminal error case for any unrecognised option.
>
> After the loop, run the common parameter checks then the binary parameter checks
> (which resolve the positional first/second input filenames and open the files),
> and return EXIT_CONTINUE.

> [spec:hfst:def:hfst-compare.print-usage-fn]
> void

> [spec:hfst:sem:hfst-compare.print-usage-fn]
> Prints the tool's help text to 'message_out'. In order:
> - "Usage: <program_name> [OPTIONS...] [INFILE1 [INFILE2]]" then "Compare two
>   transducers" then a blank line.
> - 'print_common_program_options(message_out)'.
> - 'print_common_binary_program_options(message_out)'.
> - A "Harmonization:" section listing "-H, --do-not-harmonize Do not harmonize
>   symbols." and "-e, --eliminate-flags  Eliminate flag diacritics.", then a
>   blank line.
> - 'print_common_binary_program_parameter_instructions(message_out)', then a
>   blank line.
> - An "Examples:" block showing "$ <program_name> cat.hfst dog.hfst" →
>   "cat.hfst[1] != dog.hfst[1]" and "$ <program_name> cat.hfst cat.hfst" →
>   "cat.hfst[1] == cat.hfst[1]", then a blank line.
> - 'print_report_bugs()', a blank line, then 'print_more_info()'.
