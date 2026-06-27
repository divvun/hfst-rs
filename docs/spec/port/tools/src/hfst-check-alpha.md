# tools/src/hfst-check-alpha.cc

> [spec:hfst:def:hfst-check-alpha.fprint-stringset-fn]
> void

> [spec:hfst:sem:hfst-check-alpha.fprint-stringset-fn]
> Print a StringSet to the given output FILE* as a single comma-and-space
> separated line. Iterate the set in its natural (sorted) order; before every
> element except the first, write ", " (comma then space); then write the
> element's string verbatim. Emit no leading text, no trailing separator, and no
> trailing newline (the caller adds any surrounding text and newline). An empty
> set prints nothing.

> [spec:hfst:def:hfst-check-alpha.main-fn]
> int

> [spec:hfst:sem:hfst-check-alpha.main-fn]
> Program entry point. Set the program name/version/wikiname via
> hfst_set_program_name(argv[0], "0.1", "HfstALphaFix"). Call parse_options;
> if it returns anything other than EXIT_CONTINUE, return that value immediately.
> Otherwise close the two raw input file buffers (only when they are not stdin,
> i.e. when an actual file was opened), because processing uses HFST streams
> instead. Emit the verbose message "Reading from <firstfilename> and
> <secondfilename>, writing to <outfilename>". Open an HfstInputStream for each
> of the first and second inputs: a filename-based stream when a real file was
> given, else a stdin-based stream. If constructing either stream throws an
> HfstException, report via error(EXIT_FAILURE, 0, "<name> is not a valid
> transducer file") and return EXIT_FAILURE. Run process_stream(first, second);
> then close the output file unless it is stdout, free the three filename
> buffers, and return EXIT_SUCCESS (note: main returns EXIT_SUCCESS regardless of
> process_stream's mismatch result, which it discards).

> [spec:hfst:def:hfst-check-alpha.parse-options-fn]
> int

> [spec:hfst:sem:hfst-check-alpha.parse-options-fn]
> Parse command-line options for a binary (two-input) tool. First call
> extend_options_getenv(&argc, &argv) to splice in environment-supplied options.
> Then loop calling getopt_long over the option table built from the common long
> options followed by the binary long options (then a terminating zero entry),
> with the short-option string HFST_GETOPT_COMMON_SHORT HFST_GETOPT_BINARY_SHORT.
> Stop the loop when getopt_long returns -1. For each returned option code,
> dispatch through the binary cases first, then the common cases (whose --help
> uses this tool's print_usage), then the error case; the case groups may return
> a status code from parse_options or continue the loop. There are no
> tool-specific options. After the loop, run the binary parameter check
> (resolving firstfile/secondfile from the leftover positional arguments) and
> then the common parameter check, and return EXIT_CONTINUE.

> [spec:hfst:def:hfst-check-alpha.print-usage-fn]
> void

> [spec:hfst:sem:hfst-check-alpha.print-usage-fn]
> Print the tool's help text to message_out. Write the usage line
> "Usage: <program_name> [OPTIONS...] [INFILEs]" followed by the description
> "Compare the compatibility of alphabets between INFILEs" and a blank line.
> Then print the common program options, the common binary program options, the
> tool-specific section header "Check alpha options:" (which lists no actual
> options) and a blank line, the common binary parameter instructions, a blank
> line, the report-bugs notice, a blank line, and finally the more-info notice.

> [spec:hfst:def:hfst-check-alpha.process-stream-fn]
> int

> [spec:hfst:sem:hfst-check-alpha.process-stream-fn]
> Read transducers in lockstep from two input streams and compare their
> alphabets, returning EXIT_SUCCESS if every comparison is symmetric (each side a
> superset of the other) or EXIT_FAILURE if any difference is found. Continue
> looping while both streams are good. On each iteration increment the transducer
> counter and emit the verbose message "Checking alphas...\n" for the first
> transducer or "Checking alphas... <n>\n" thereafter.
>
> For the FIRST stream: read one HfstTransducer, build an HfstBasicTransducer copy
> of it, and try to obtain its declared alphabet via get_alphabet() (record
> transducerKnowsAlphabet = true on success; on FunctionNotImplementedException set
> it false and leave the declared alphabet empty). Then build firstFoundAlphabet by
> iterating every state and every transition of the basic transducer, inserting both
> the input symbol and the output symbol of each transition. Do the identical
> sequence for the SECOND stream (note: transducerKnowsAlphabet is reset to false
> before the second get_alphabet attempt, so its final value reflects the second
> transducer only), producing secondTransducerAlphabet and secondFoundAlphabet.
>
> Compare the FOUND alphabets and print to outfile. Print "Actual alphabet
> differences:\n". Compute firstFoundAlphabet minus secondFoundAlphabet: if
> non-empty, set mismatch = EXIT_FAILURE and print "In first <first.name> but not
> in second <second.name>:" then the difference set; else print "First <first.name>
> alpha is superset of second <second.name>."; then a newline. Compute
> secondFoundAlphabet minus firstFoundAlphabet: if non-empty, set mismatch and print
> "In second <second.name> but not in first <second.name>:" then the difference set
> (note: both names printed are second.name, mirroring the source); else print
> "Second <second.name> alpha is superset of second <second.name>."; then a newline.
> If verbose, print "<first.name> alphabet:" + firstFoundAlphabet + newline, then
> "<second.name> alphabet:" + secondFoundAlphabet + newline.
>
> If transducerKnowsAlphabet (the second transducer declared an alphabet): print
> "sigma set difference:\n", then compute firstTransducerAlphabet minus
> secondTransducerAlphabet and secondTransducerAlphabet minus firstTransducerAlphabet.
> For the first difference: if non-empty set mismatch and print "First <first.name>
> has but second <second.name> does not: " + the set; else print "First <first.name>
> alpha is superset of second <second.name>."; newline. For the second difference: if
> non-empty set mismatch and print "Second <second.name> has but first <first.name>
> does not: " + the set; else print "Second <second.name> alpha is superset of first
> <first.name>."; newline. If verbose, print "First (<first.name>):" +
> firstTransducerAlphabet + newline and "Second (<second.name>):" +
> secondTransducerAlphabet + newline. Otherwise (no declared alphabet) print "No
> internal alphabets to compare in this format\n".
>
> Re-evaluate continueReading from both streams' is_good and loop. After the loop,
> print "\nRead <transducer_n> transducers in total.\n" and return mismatch.

