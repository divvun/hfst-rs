# tools/src/hfst-prune-alphabet.cc

> [spec:hfst:def:hfst-prune-alphabet.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-prune-alphabet.main-fn]
> Program entry point. On Windows sets stdin/stdout to binary mode. Calls
> hfst_set_program_name(argv[0], "0.1", "HfstPruneAlphabet"). Calls
> parse_options(argc, argv); if it returns anything other than EXIT_CONTINUE,
> returns that value immediately. Closes the input FILE buffer with fclose
> unless it is stdin, and the output FILE buffer unless it is stdout (the tool
> works with streams from here on). Emits a verbose message "Reading from
> <inputfilename>, writing to <outfilename>". Constructs an HfstInputStream:
> from inputfilename when input is a named file, otherwise from stdin; in C the
> construction is wrapped in try/catch that reports "<inputfilename> is not a
> valid transducer file" and returns EXIT_FAILURE on HfstException. Constructs
> an HfstOutputStream: with (outfilename, instream type) for a named output
> file, otherwise with the instream type. Then if is_input_stream_in_ol_format
> reports the stream is in optimized-lookup format (for tool
> "hfst-prune-alphabet"), returns EXIT_FAILURE. Otherwise calls
> process_stream(instream, outstream), frees inputfilename and outfilename, and
> returns its result.

> [spec:hfst:def:hfst-prune-alphabet.parse-options-fn]
> int

> [spec:hfst:sem:hfst-prune-alphabet.parse-options-fn]
> Parses command-line options. First calls extend_options_getenv(&argc, &argv)
> to splice in options from the environment. Loops over getopt_long with the
> long-option table built from HFST_GETOPT_COMMON_LONG, HFST_GETOPT_UNARY_LONG,
> the two tool-specific entries {"force", no_argument, 0, 'f'} and
> {"safe", no_argument, 0, 'S'}, and a NULL terminator; the short-option string
> is HFST_GETOPT_COMMON_SHORT + HFST_GETOPT_UNARY_SHORT + "fS". Breaks the loop
> when getopt_long returns -1. Each returned option character is dispatched
> through, in order, the common cases (which include the --help arm that prints
> usage), the unary cases, then the tool-specific cases: 'f' sets
> force_pruning = true, 'S' sets force_pruning = false; an unrecognized option
> falls through to the error case. After the loop, runs the common parameter
> checks and the unary parameter checks (which resolve inputfilename/inputfile
> and outfilename/outfile from any positional arguments), then returns
> EXIT_CONTINUE.

> [spec:hfst:def:hfst-prune-alphabet.print-usage-fn]
> void

> [spec:hfst:sem:hfst-prune-alphabet.print-usage-fn]
> Prints help text to message_out. Writes the usage line
> "Usage: <program_name> [OPTIONS...] [INFILE]" followed by the description
> "Prune the alphabet of a transducer" and a blank line. Then prints the common
> program options, the common unary program options, and the alphabet pruning
> options block:
>   -f, --force            force pruning
>   -S, --safe             prune only if no unknown or identity symbols
>                          are used in the transducer (default)
> followed by a newline, the common unary program parameter instructions, a
> newline, the report-bugs text, a newline, and the more-info text.

> [spec:hfst:def:hfst-prune-alphabet.process-stream-fn]
> int

> [spec:hfst:sem:hfst-prune-alphabet.process-stream-fn]
> Processes every transducer in instream. Maintains a 1-based counter
> transducer_n. While instream.is_good(): increments the counter, reads the next
> HfstTransducer from instream, and obtains its name via
> hfst_get_name(trans, inputfilename). For the first transducer emits the verbose
> message "Pruning <name>...", and for any subsequent transducer emits
> "Pruning <name>... <transducer_n>". Prunes the transducer's alphabet by calling
> trans.prune_alphabet(force_pruning): when force_pruning is false (the default
> "safe" mode) the library only prunes if no unknown or identity symbols are used.
> Sets the result's name with hfst_set_name(trans, trans, "prune-alphabet") (the
> transducer-source overload, which wraps the existing name as
> "prune-alphabet(<name>)", or "prune-alphabet(UNNAMED)" if unnamed). Writes the
> transducer to outstream and frees the name string. After the loop closes both
> instream and outstream and returns EXIT_SUCCESS.
