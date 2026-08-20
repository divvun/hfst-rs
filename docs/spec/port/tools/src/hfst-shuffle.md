# tools/src/hfst-shuffle.cc

> [spec:hfst:def:hfst-shuffle.main-fn]
> int

> [spec:hfst:sem:hfst-shuffle.main-fn]
> Program entry point for the binary tool hfst-shuffle.
> 1. (On Windows) set stdin and stdout to binary mode.
> 2. Call hfst_set_program_name(argv[0], "0.1", "HfstShuffle").
> 3. Call parse_options(argc, argv); if it returns anything other than
>    EXIT_CONTINUE, return that value immediately.
> 4. Close the buffered FILE* handles that the option parser opened: if
>    firstfile is not stdin, fclose it; if secondfile is not stdin, fclose it;
>    if outfile is not stdout, fclose it (the tool reads via HFST streams, not
>    these FILE buffers).
> 5. Emit a verbose message "Reading from <firstfilename> and <secondfilename>,
>    writing to <outfilename>".
> 6. Construct the first HfstInputStream from firstfilename (or from stdin when
>    firstfile is stdin); on an HfstException, error out with
>    "<firstfilename> is not a valid transducer file" (EXIT_FAILURE).
> 7. Construct the second HfstInputStream likewise from secondfilename/stdin,
>    erroring with the analogous message for the second file.
> 8. If either input stream is in optimized-lookup format
>    (is_input_stream_in_ol_format(..., "hfst-shuffle")), return EXIT_FAILURE.
> 9. Call shuffle_streams(firststream, secondstream) and capture its return
>    value.
> 10. Free firstfilename, secondfilename and outfilename, then return the
>    captured value.

> [spec:hfst:def:hfst-shuffle.shuffle-streams-fn]
> int

> [spec:hfst:sem:hfst-shuffle.shuffle-streams-fn]
> Read transducers pairwise from the two input streams, shuffle each pair, and
> write the result to the output stream.
> 1. Set continueReading = firststream.is_good() && secondstream.is_good()
>    (there must be at least one transducer in each input).
> 2. Determine output_type from the two stream types type1 and type2:
>    - If type1 == type2, output_type = type1.
>    - Otherwise, if transducer conversion is allowed
>      (allow_transducer_conversion), call conversion_type(type1, type2):
>      result 1 -> output_type = type1 ("using former type as output");
>      result 2 -> output_type = type2 ("using latter type as output");
>      result -1 -> output_type = type1 ("using former type as output, loss of
>      information is possible"); any other result -> throw (an internal error).
>      Emit the assembled "Transducer type mismatch in <firstfilename> and
>      <secondfilename>; ..." text as a warning.
>      If conversion is not allowed, error out (EXIT_FAILURE) reporting that the
>      formats are not compatible for shuffle and --do-not-convert was
>      requested.
> 3. Open the output stream: HfstOutputStream(outfilename, output_type) when
>    outfile is not stdout, otherwise HfstOutputStream(output_type).
> 4. Loop while continueReading, keeping running counts transducer_n_first and
>    transducer_n_second of transducers read from each stream:
>    a. Read one transducer from the first stream (increment
>       transducer_n_first). If the second stream is good, read one transducer
>       from it (increment transducer_n_second); otherwise reuse the previously
>       read second transducer.
>    b. Obtain firstname = hfst_get_name(first, firstfilename). If the second
>       transducer pointer is NULL, throw (should not happen). Obtain
>       secondname = hfst_get_name(second, secondfilename).
>    c. Emit verbose "Shuffling <firstname> and <secondname>..." (append the
>       1-based count when transducer_n_first != 1).
>    d. Attempt first->shuffle(second) (harmonize defaults to true):
>       - If it raises TransducersAreNotAutomataException, error out
>         (EXIT_FAILURE) "Could not shuffle <firstname> and <secondname>
>         [<n>]\nat least one of the input arguments is not an automaton".
>       - If it raises TransducerTypeMismatchException: when conversion is
>         allowed, convert_transducers(first, second) and retry the shuffle;
>         otherwise error out (EXIT_FAILURE) reporting the incompatible formats
>         and that --do-not-convert was requested.
>    e. Set the result transducer's name via hfst_set_name(first, first, second,
>       "shuffle") and its formula via hfst_set_formula(first, first, second,
>       "shuffle"), then write it to the output stream (outstream << first).
>    f. Recompute continueReading = firststream.is_good() &&
>       (secondstream.is_good() || transducer_n_second == 1). Release the first
>       transducer. Release the second transducer unless we will continue
>       reading the first stream while the second stream is exhausted and held
>       exactly one transducer (i.e. release it when (continueReading &&
>       secondstream.is_good()) || !continueReading).
>    g. Free firstname and secondname.
> 5. After the loop, if firststream is still good, error out (EXIT_FAILURE):
>    the second input contains fewer transducers than the first, which is only
>    valid when the second input has exactly one transducer.
> 6. If secondstream is still good, error out (EXIT_FAILURE): the first input
>    contains fewer transducers than the second.
> 7. Close both input streams and the output stream and return EXIT_SUCCESS.
