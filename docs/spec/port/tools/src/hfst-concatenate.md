# tools/src/hfst-concatenate.cc

> [spec:hfst:def:hfst-concatenate.concatenate-streams-fn]
> int

> [spec:hfst:sem:hfst-concatenate.concatenate-streams-fn]
> Pairwise-concatenate the transducers of 'firststream' onto those of
> 'secondstream', writing each result to the output stream.
>
> 1. Set 'continueReading' to true only if both input streams currently have a
>    transducer available (both 'is_good()').
> 2. Read each stream's implementation type ('type1', 'type2'). Determine the
>    'output_type':
>    - If 'type1 == type2', 'output_type = type1'.
>    - Otherwise, if transducer conversion is allowed (ALLOW_TRANSDUCER_CONVERSION),
>      call 'conversion_type(type1, type2)' and build a warning string
>      "Transducer type mismatch in <firstfilename> and <secondfilename>; ":
>      ct==1 -> append "using former type as output", output_type=type1;
>      ct==2 -> append "using latter type as output", output_type=type2;
>      ct==-1 -> append "using former type as output, loss of information is
>      possible", output_type=type1; any other value -> throw (panic) an
>      "invalid integer" error. Emit the assembled warning via 'warning(0,0,..)'.
>    - Otherwise (conversion not allowed and types differ): call 'error(EXIT_FAILURE,..)'
>      reporting the incompatible formats and "(--do-not-convert was requested)"
>      (this exits).
> 3. Open the output stream: a named-file stream on 'outfilename' if an output
>    file was named, otherwise a stdout stream; both with 'output_type' and
>    'hfst_format=true'.
> 4. Loop while 'continueReading':
>    a. Read one transducer 'first' from 'firststream'; increment
>       'transducer_n_first'.
>    b. If 'secondstream.is_good()', read one transducer 'second' from it and
>       increment 'transducer_n_second' (otherwise 'second' is reused from the
>       previous iteration — the single-transducer fan-out case).
>    c. Compute 'firstname'/'secondname' via 'hfst_get_name' (falling back to the
>       filenames). If 'second' is absent, throw (should not happen).
>    d. Verbose-print "Concatenating <firstname> and <secondname>..." for the
>       first transducer, or with a trailing " <transducer_n_first>" thereafter.
>    e. If both transducers contain flag diacritics: when HARMONIZE_FLAGS is
>       false, emit (unless SILENT) a warning advising "-F to harmonize them";
>       when true, call 'first.harmonize_flag_diacritics(second, false)'.
>    f. Attempt 'first.concatenate(second, HARMONIZE)'. If it throws a
>       'TransducerTypeMismatchException': when conversion is allowed, call
>       'convert_transducers(first, second)' then retry the concatenate;
>       otherwise call 'error(EXIT_FAILURE,..)' reporting incompatible formats
>       (exits). Any other thrown exception propagates.
>    g. Set the result transducer's name via 'hfst_set_name(first,first,second,
>       "concatenate")' and its formula via 'hfst_set_formula(first,first,second,
>       "⋅")' (the dot operator), then write 'first' to the output stream.
>    h. Recompute 'continueReading = firststream.is_good() && (secondstream.is_good()
>       || transducer_n_second == 1)'. Free 'first'. Free 'second' unless we are
>       continuing and the second stream is still good (i.e. keep the single
>       second transducer alive for fan-out).
> 5. After the loop: if 'firststream' is still good, error "second input contains
>    fewer transducers than first input" (only valid if the second had exactly
>    one). If 'secondstream' is still good, error "first input contains fewer
>    transducers than second input".
> 6. Close both input streams, flush and close the output stream, return
>    EXIT_SUCCESS.

> [spec:hfst:def:hfst-concatenate.main-fn]
> int

> [spec:hfst:sem:hfst-concatenate.main-fn]
> Program entry point for the binary tool 'hfst-concatenate'.
>
> 1. (On Windows, set stdin/stdout to binary mode.)
> 2. Call 'hfst_set_program_name(argv[0], "0.1", "HfstConcatenate")'.
> 3. Call 'parse_options(argc, argv)'; if it does not return EXIT_CONTINUE,
>    return that value immediately.
> 4. Close the buffered FILE handles that were opened for option parsing: close
>    'firstfile' unless it is stdin, 'secondfile' unless stdin, 'outfile' unless
>    stdout (we use streams from here on).
> 5. Verbose-print "Reading from <firstfilename> and <secondfilename>, writing to
>    <outfilename>".
> 6. Construct 'firststream' (named-file stream on 'firstfilename' if a first
>    file was named, otherwise a stdin stream) and likewise 'secondstream'. The
>    C++ wraps each constructor in a try/catch that errors "<file> is not a valid
>    transducer file"; the Rust stream constructor panics on a bad file instead.
> 7. If either stream is in optimized-lookup (ol) format
>    ('is_input_stream_in_ol_format(..,"hfst-concatenate")'), return EXIT_FAILURE.
> 8. Return 'concatenate_streams(firststream, secondstream)' (freeing the
>    filename strings in the C++ version).

> [spec:hfst:def:hfst-concatenate.parse-options-fn]
> int

> [spec:hfst:sem:hfst-concatenate.parse-options-fn]
> Parse command-line options for this binary tool.
>
> 1. Call 'extend_options_getenv(&argc, &argv)' to splice in any options from the
>    environment.
> 2. Loop calling 'getopt_long' with the long-option table
>    (HFST_GETOPT_COMMON_LONG ++ HFST_GETOPT_BINARY_LONG ++
>    {"harmonize-flags",no_argument,0,'F'} ++ {"do-not-harmonize",no_argument,0,'H'}
>    ++ terminator) and the short-option string
>    (HFST_GETOPT_COMMON_SHORT ++ HFST_GETOPT_BINARY_SHORT ++ "FH"). Break when
>    getopt returns -1.
> 3. For each returned option character, dispatch in order: first the binary
>    cases ('1'/'2'/'o'/'C' — first/second/output files and --do-not-convert),
>    then the common cases (help/version/verbose/etc., which may print usage and
>    return), then the tool-specific cases: 'F' sets HARMONIZE_FLAGS=true, 'H'
>    sets HARMONIZE=false; otherwise fall through to the error case ('?'/':'/
>    default), which prints a short help and exits.
> 4. After the loop, run the binary parameter checks ('check_binary_params') and
>    the common parameter checks ('check_common_params').
> 5. Return EXIT_CONTINUE.

> [spec:hfst:def:hfst-concatenate.print-usage-fn]
> void

> [spec:hfst:sem:hfst-concatenate.print-usage-fn]
> Print the tool's usage/help text to the message-output stream:
>
> 1. "Usage: <program_name> [OPTIONS...] [INFILE1 [INFILE2]]" followed by
>    "Concatenate two transducers" and a blank line.
> 2. The common program options ('print_common_program_options').
> 3. The common binary program options ('print_common_binary_program_options').
> 4. A "Harmonization:" section listing
>    "-H, --do-not-harmonize Do not harmonize symbols." and
>    "-F, --harmonize-flags  Harmonize flag diacritics.", then a blank line.
> 5. The common binary parameter instructions
>    ('print_common_binary_program_parameter_instructions'), then a blank line.
> 6. An "Examples:" section showing
>    "<program_name> -o catdog.hfst cat.hfst dog.hfst" and a description that it
>    concatenates cat.hfst with dog.hfst into catdog.hfst, then a blank line.
> 7. The bug-report footer ('print_report_bugs'), a blank line, and the
>    more-info footer ('print_more_info').
