# tools/src/hfst-minimize.cc

> [spec:hfst:def:hfst-minimize.main-fn]
> int

> [spec:hfst:sem:hfst-minimize.main-fn]
> Entry point of the hfst-minimize tool. On Windows it first sets stdin and
> stdout to binary mode. It calls hfst_set_program_name(argv[0], "0.1",
> "HfstMinimize"), then parse_options(argc, argv); if that returns anything
> other than EXIT_CONTINUE it returns that value immediately. It then closes the
> buffered FILE handles it no longer needs: if inputfile is not stdin it fcloses
> inputfile, and if outfile is not stdout it fcloses outfile (the tool uses HFST
> streams from here on). It saves the current global encode-weights flag via
> hfst::get_encode_weights() into 'enc'; if the tool's -E flag (encode_weights)
> was given it sets hfst::set_encode_weights(true). It emits the verbose message
> "Reading from <inputfilename>, writing to <outfilename>". It then constructs
> the input stream: if inputfile is not stdin, an HfstInputStream over
> inputfilename, otherwise an HfstInputStream over stdin. This construction is
> guarded by try/catch: an ImplementationTypeNotAvailableException causes
> error(EXIT_FAILURE, 0, "file %s is in %s format which is not available", ...)
> and returns EXIT_FAILURE; any other HfstException causes
> error(EXIT_FAILURE, 0, "%s is not a valid transducer file", inputfilename) and
> returns EXIT_FAILURE. It then constructs the output stream from the input
> stream's type: an HfstOutputStream over outfilename if outfile is not stdout,
> otherwise an HfstOutputStream to stdout. If is_input_stream_in_ol_format(
> instream, "hfst-minimize") is true it returns EXIT_FAILURE (optimized-lookup
> transducers cannot be minimized). Otherwise it calls process_stream(instream,
> outstream) and keeps its return value. If the -E flag was given it restores
> the global encode-weights flag to the saved 'enc'. Finally it frees
> inputfilename and outfilename and returns the saved return value.

> [spec:hfst:def:hfst-minimize.parse-options-fn]
> int

> [spec:hfst:sem:hfst-minimize.parse-options-fn]
> Parses the command-line options. It first calls extend_options_getenv(&argc,
> &argv) to splice in options from the environment. It then loops calling
> getopt_long with the long-option table built from HFST_GETOPT_COMMON_LONG,
> HFST_GETOPT_UNARY_LONG, the tool-specific entry { "encode-weights",
> no_argument, 0, 'E' }, and the terminating zero entry; the short-option string
> is HFST_GETOPT_COMMON_SHORT HFST_GETOPT_UNARY_SHORT "E". The loop ends when
> getopt_long returns -1. Each returned option code is dispatched through the
> standard included case groups in order: the common cases (getopt-cases-common,
> e.g. help/version/verbose/quiet/output/input handling, which may print usage
> and return), the error case (getopt-cases-error), and the unary cases
> (getopt-cases-unary). The tool adds one case: 'E' sets the module-static
> encode_weights flag to true. After the loop it runs the common parameter
> checks (check-params-common) and the unary parameter checks
> (check-params-unary), then returns EXIT_CONTINUE.

> [spec:hfst:def:hfst-minimize.print-usage-fn]
> void

> [spec:hfst:sem:hfst-minimize.print-usage-fn]
> Prints the tool's help text to message_out. It writes the usage line
> "Usage: <program_name> [OPTIONS...] [INFILE]" followed by "Minimize a
> transducer" and a blank line. It then calls print_common_program_options and
> print_common_unary_program_options. It prints "Command-specific options:" and
> the tool-specific option description:
> "  -E, --encode-weights         Encode weights when minimizing" followed by a
> second line indented to "(default is false)." (31 leading spaces) and a blank
> line. It then calls print_common_unary_program_parameter_instructions, prints
> a newline, calls print_report_bugs, prints a newline, and calls
> print_more_info.

> [spec:hfst:def:hfst-minimize.process-stream-fn]
> int

> [spec:hfst:sem:hfst-minimize.process-stream-fn]
> Reads every transducer from instream, minimizes it, and writes it to
> outstream. It keeps a 1-based counter transducer_n. While instream.is_good(),
> it increments the counter and constructs an HfstTransducer from instream. It
> obtains the input transducer's name via hfst_get_name(trans, inputfilename).
> For the first transducer it emits the verbose message "Minimizing <name>...";
> for subsequent transducers it emits "Minimizing <name>...<transducer_n>". It
> calls trans.minimize() (under PROFILE builds this is bracketed by forcing
> hfst::set_minimize_even_if_already_minimal(true), timing the call with clock(),
> restoring the prior flag, and printing the elapsed seconds to stderr; the
> non-PROFILE path simply minimizes). It then sets the result transducer's name
> with hfst_set_name(trans, trans, "minimize") and its formula with
> hfst_set_formula(trans, trans, "M"), writes trans to outstream, and frees the
> inputname string. After the loop it flushes outstream, closes instream, closes
> outstream, and returns EXIT_SUCCESS.
