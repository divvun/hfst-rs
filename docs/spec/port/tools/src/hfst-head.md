# tools/src/hfst-head.cc

> [spec:hfst:def:hfst-head.main-fn]
> int

> [spec:hfst:sem:hfst-head.main-fn]
> Program entry point. On Windows, set stdin (fd 0) and stdout (fd 1) to
> binary mode. Call hfst_set_program_name with argv[0], version "0.2", and
> wiki name "HfstHead". Call parse_options(argc, argv); if its return value is
> not EXIT_CONTINUE, return that value as the exit code. Otherwise close the
> buffered input/output: if inputfile is not stdin, fclose(inputfile); if
> outfile is not stdout, fclose(outfile). Emit a verbose message
> "Reading from <inputfilename>, writing to <outfilename>". Open the input
> stream: if inputfile was a named file, construct HfstInputStream(inputfilename),
> otherwise the default HfstInputStream() reading stdin; if construction throws
> HfstException, report a fatal error "<inputfilename> is not a valid transducer
> file" with EXIT_FAILURE and return EXIT_FAILURE. Open the output stream from
> the input stream's transducer type: if outfile was a named file, construct
> HfstOutputStream(outfilename, type), otherwise HfstOutputStream(type). Run
> process_stream on the two streams, free inputfilename and outfilename, and
> return process_stream's result.

> [spec:hfst:def:hfst-head.parse-options-fn]
> int

> [spec:hfst:sem:hfst-head.parse-options-fn]
> Parse command-line options. First call extend_options_getenv(&argc, &argv)
> to splice in environment-provided options. Loop calling getopt_long over the
> option table built from the common long options, the unary long options, and
> one tool-specific entry { "n-first", required_argument, 0, 'n' } terminated
> by a zero entry; the short-option string is the common short options followed
> by the unary short options followed by "n:". Break out of the loop when
> getopt_long returns -1. Dispatch each returned option character through the
> common getopt cases, then the unary getopt cases, then the tool case 'n':
> for 'n', set head_count = hfst_strtol(optarg, 10) (base-10 signed parse) and
> break; otherwise fall through to the error case. After the loop, run the
> common and unary parameter checks. Then, if head_count == 0, emit a warning
> (status 0, errnum 0) "Argument 0 for count is not sensible". Return
> EXIT_CONTINUE.

> [spec:hfst:def:hfst-head.print-usage-fn]
> void

> [spec:hfst:sem:hfst-head.print-usage-fn]
> Print help to message_out. Print the usage line
> "Usage: <program_name> [OPTIONS...] [INFILE]" followed by the description
> "Get first transducers from an archive" and a blank line. Print the common
> program options, then the common unary program options. Print the archive
> options block:
> "Archive options:
>   -n, --n-first=[-]K   print the first K transducers;
>                        with the leading `-', print all but last K transducers".
> Print a blank line, then the common unary program parameter instructions,
> then the note "K must be an integer, as parsed by strtoul base 10, and not 0.
> If K is omitted default is 1." Print a blank line, the report-bugs notice, a
> blank line, and the more-info notice.

> [spec:hfst:def:hfst-head.process-stream-fn]
> int

> [spec:hfst:sem:hfst-head.process-stream-fn]
> Forward the head of the transducer archive from instream to outstream,
> governed by the global head_count. Maintain a counter transducer_n starting
> at 0.
>
> If head_count > 0: while the input stream is good and transducer_n <
> head_count, increment transducer_n, read one HfstTransducer from instream,
> determine its name from get_name() falling back to inputfilename when empty,
> emit a verbose message "Forwarding <name>...<transducer_n>", and write the
> transducer to outstream.
>
> Else if head_count < 0: emit a verbose message "Counting all but last
> <head_count>". Read every transducer from instream into a deque (in order),
> incrementing transducer_n for each. If -head_count exceeds the number of
> transducers read, emit a warning "Stream in <inputfilename> has less than
> <-head_count> automata; Nothing will be written to output". Pop -head_count
> transducers off the back of the deque (skipping the pop when the deque is
> already empty). Then, while the deque is non-empty, take the front
> transducer, compute its name as above, emit "Forwarding <name>...<transducer_n>",
> write it to outstream, and pop it from the front.
>
> Finally flush outstream, close instream, close outstream, and return
> EXIT_SUCCESS.
