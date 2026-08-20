# tools/src/hfst-subtract.cc

> [spec:hfst:def:hfst-subtract.main-fn]
> int

> [spec:hfst:sem:hfst-subtract.main-fn]
> Program entry point for hfst-subtract. On Windows it sets stdin/stdout to
> binary mode. It registers the program name via hfst_set_program_name(argv[0],
> "0.1", "HfstSubtract"), then calls parse_options(argc, argv); if that returns
> anything other than EXIT_CONTINUE the value is returned immediately. It then
> closes the raw buffered FILE handles it no longer needs: firstfile (unless
> stdin), secondfile (unless stdin) and outfile (unless stdout), since the work
> is done through HFST streams. It prints a verbose "Reading from FIRST and
> SECOND, writing to OUT" message. It opens an HfstInputStream for the first
> input (named file or stdin) and another for the second input, reporting
> "<file> is not a valid transducer file" on an HfstException for either. (A
> throw-away output stream is constructed from the first stream's type but is
> not used; the real output stream is created inside subtract_streams.) If
> either input stream is in optimized-lookup format
> (is_input_stream_in_ol_format), it returns EXIT_FAILURE. Otherwise it calls
> subtract_streams(first, second), frees firstfilename/secondfilename/outfilename
> and returns that result.

> [spec:hfst:def:hfst-subtract.subtract-streams-fn]
> int

> [spec:hfst:sem:hfst-subtract.subtract-streams-fn]
> Performs the streaming subtraction of two HfstInputStreams. It sets
> continueReading = firststream.is_good() && secondstream.is_good(). It reads
> the implementation types of both streams. If the types differ: when
> allow_transducer_conversion is set it calls conversion_type(type1, type2) and
> chooses the output type — 1 selects the former type ("using former type as
> output"), 2 selects the latter ("using latter type as output"), -1 selects
> the former with a possible loss-of-information warning, and any other value
> throws an internal error; the assembled warning is emitted via hfst_warning.
> When conversion is not allowed it errors out with a type-mismatch /
> not-compatible message and exits. If the types are equal, output_type is set
> to type1. It opens an HfstOutputStream on outfilename (or on stdout when
> outfile is stdout) with output_type.
>
> It then loops while continueReading: it reads one transducer from the first
> stream (incrementing transducer_n_first); if the second stream is good it
> reads one transducer from the second stream (incrementing
> transducer_n_second). It obtains firstname and secondname via hfst_get_name
> (using firstfilename/secondfilename as fallbacks); if no second transducer is
> available it throws. It prints a verbose message "Subtracting <secondname>
> from <firstname>..." (appending the running first-stream count for transducers
> after the first). If the second transducer has flag diacritics it warns
> "Warning: <secondfilename> contains flag diacritics. The result of
> subtraction may be incorrect." If both transducers have flag diacritics then,
> when harmonize_flags is false, it warns (unless silent) "The argumentes
> contain flag diacritics. Use -F to harmonize them."; when harmonize_flags is
> true it calls first.harmonize_flag_diacritics(second). It then computes
> first.subtract(second, harmonize); if a TransducerTypeMismatchException is
> thrown it either converts both transducers (convert_transducers) and retries
> the subtract when allow_transducer_conversion is set, or errors out with a
> "Could not subtract" not-compatible message. It sets the result's name via
> hfst_set_name(first, first, second, "subtract") and formula via
> hfst_set_formula(first, first, second, "\u{2212}") (MINUS SIGN), and writes
> first to the output stream.
>
> It then recomputes continueReading = firststream.is_good() &&
> (secondstream.is_good() || transducer_n_second == 1). It deletes the first
> transducer, and deletes the second transducer unless it is continuing to read
> the first stream while the second stream still has data and there is exactly
> one transducer in the second stream (i.e. it keeps the single second
> transducer to subtract from every remaining first transducer). After the loop,
> if the first stream still has data it errors that the second input contains
> fewer transducers than the first (only valid when the second has exactly one);
> if the second stream still has data it errors that the first input contains
> fewer transducers than the second. Finally it closes both input streams,
> flushes and closes the output stream, and returns EXIT_SUCCESS.
