# tools/src/hfst-compose.cc

> [spec:hfst:def:hfst-compose.compose-streams-fn]
> int

> [spec:hfst:sem:hfst-compose.compose-streams-fn]
> Composes every transducer pair drawn from two input streams and writes each
> result to the output stream. Steps:
> 1. Set continueReading = firststream.is_good() && secondstream.is_good().
> 2. Read type1 = firststream.get_type(), type2 = secondstream.get_type(), and
>    output_type = UNSPECIFIED_TYPE. If type1 != type2: when transducer
>    conversion is allowed, compute ct = conversion_type(type1, type2) and build
>    a warning "Transducer type mismatch in <firstfilename> and <secondfilename>;
>    "; for ct == 1 append "using former type as output" and set output_type =
>    type1; for ct == 2 append "using latter type as output" and set output_type
>    = type2; for ct == -1 append "using former type as output, loss of
>    information is possible" and set output_type = type1; any other ct is an
>    internal error (throw). Emit the assembled warning via hfst_warning(0,0,..).
>    When conversion is NOT allowed, call hfst_error(EXIT_FAILURE, ...) reporting
>    the format names (it exits). If type1 == type2, output_type = type1.
> 3. Open the output stream: HfstOutputStream(outfilename, output_type) when an
>    output file was given, else HfstOutputStream(output_type).
> 4. Loop while continueReading, tracking transducer_n_first and
>    transducer_n_second counts:
>    a. If firststream.is_good(), read 'first' and increment transducer_n_first;
>       if secondstream.is_good(), read 'second' and increment
>       transducer_n_second.
>    b. firstname = hfst_get_name(first, firstfilename); if 'second' is null this
>       is an internal error (throw); secondname = hfst_get_name(second,
>       secondfilename).
>    c. If transducer_n_first == 1 print "Composing <firstname> and
>       <secondname>...\n", else "Composing <firstname> and <secondname>...
>       <transducer_n_first>\n" (verbose).
>    d. If either operand has flag diacritics: if NOT harmonize_flags, and not
>       silent, warn that an argument contains flag diacritics and -F may be used
>       to harmonize them. If harmonize_flags, call
>       first.harmonize_flag_diacritics(second); on TransducerTypeMismatch, if
>       conversion is allowed call convert_transducers(first, second) and retry,
>       else hfst_error(EXIT_FAILURE,...) about incompatible formats.
>    e. Call first.compose(second, harmonize); on TransducerTypeMismatch, if
>       conversion is allowed convert_transducers(first, second) and retry, else
>       hfst_error(EXIT_FAILURE,...) about incompatible formats.
>    f. hfst_set_name(first, first, second, "compose"); hfst_set_formula(first,
>       first, second, "∘"); write first to the output stream.
>    g. Recompute continueReading = (firststream.is_good() &&
>       secondstream.is_good()) || (firststream.is_good() && transducer_n_second
>       == 1) || (transducer_n_first == 1 && secondstream.is_good()); free the
>       operands no longer needed.
> 5. After the loop, if firststream.is_good() hfst_error that the second input
>    has fewer transducers than the first (only valid when the second has exactly
>    one); if secondstream.is_good() the symmetric error.
> 6. Close both input streams, flush and close the output stream, return
>    EXIT_SUCCESS.

> [spec:hfst:def:hfst-compose.main-fn]
> int

> [spec:hfst:sem:hfst-compose.main-fn]
> Program entry point. Sets the program name to "hfst-compose" via
> hfst_set_program_name(argv[0], "0.1", "HfstCompose"). Calls parse_options; if
> it returns anything other than EXIT_CONTINUE, returns that value. Closes the
> first, second and output FILE buffers when they are not the standard streams
> (the tool uses HFST streams instead). Prints "Reading from <firstfilename> and
> <secondfilename>, writing to <outfilename>\n" (verbose). Opens the first input
> stream from firstfilename (or stdin) and the second from secondfilename (or
> stdin) — in C++ a HfstException during construction triggers hfst_error
> "<file> is not a valid transducer file". Opens the output stream from
> outfilename using the first stream's type (or stdout). If either input stream
> is in optimized-lookup format (is_input_stream_in_ol_format), returns
> EXIT_FAILURE. Otherwise calls compose_streams(first, second), frees the
> filename strings, and returns its result.

> [spec:hfst:def:hfst-compose.parse-options-fn]
> int

> [spec:hfst:sem:hfst-compose.parse-options-fn]
> Parses the command line. First extends argv from the environment
> (extend_options_getenv). Then loops calling getopt_long with the combined short
> spec HFST_GETOPT_COMMON_SHORT + HFST_GETOPT_BINARY_SHORT + "FHx:X:" and the
> long-option table = common long options + binary long options + the tool's own:
> {"harmonize-flags", no_argument, 'F'}, {"do-not-harmonize", no_argument, 'H'},
> {"xerox-composition", required_argument, 'x'}, {"xfst", required_argument,
> 'X'}. Each returned option is dispatched through the binary input cases, then
> the common cases, then the tool cases, then the error case:
> - 'F': set harmonize_flags = true.
> - 'H': set harmonize = false.
> - 'x': read the argument; for "yes"/"true"/"ON" call set_xerox_composition(true),
>   for "no"/"false"/"OFF" call set_xerox_composition(false); otherwise print
>   "Error: unknown option to --xerox-composition: '<optarg>'\n" to stderr and
>   return EXIT_FAILURE.
> - 'X': read the argument; for "flag-is-epsilon" call
>   set_flag_is_epsilon_in_composition(true); otherwise print "Error: unknown
>   option to --xfst: '<optarg>'\n" to stderr and return EXIT_FAILURE.
> The loop ends when getopt_long returns -1. Then runs the binary and common
> parameter checks (check-params-binary, check-params-common) and returns
> EXIT_CONTINUE.

> [spec:hfst:def:hfst-compose.print-usage-fn]
> void

> [spec:hfst:sem:hfst-compose.print-usage-fn]
> Prints the help text to message_out: a "Usage: <program_name> [OPTIONS...]
> [INFILE1 [INFILE2]]" line with the description "Compose two transducers", then
> the common program options, the common binary program options, the composition
> options block (-x/--xerox-composition=VALUE, -X/--xfst=VARIABLE) and
> harmonization options block (-H/--do-not-harmonize, -F/--harmonize-flags),
> followed by the common binary parameter instructions, a note that the xfst
> variables are {flag-is-epsilon (default OFF)}, an explanation that VALUE may be
> [true|false], [yes|no] or [ON|OFF] with false being the default, an Examples
> section showing "<program_name> -o cat2dog.hfst cat2mouse.hfst mouse2dog.hfst
> composes two automata", then the report-bugs and more-info footers.
