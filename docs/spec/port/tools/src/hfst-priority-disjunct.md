# tools/src/hfst-priority-disjunct.cc

> [spec:hfst:def:hfst-priority-disjunct.main-fn]
> int

> [spec:hfst:sem:hfst-priority-disjunct.main-fn]
> Program entry point. Sets the program name to argv[0] with version "0.1"
> and wiki name "HfstPriorityDisjunct". Calls parse_options(argc, argv); if
> it returns anything other than EXIT_CONTINUE, returns that value
> immediately. Otherwise closes the stdio buffers that were opened for
> named files: if firstfile is not stdin, fclose(firstfile); if secondfile
> is not stdin, fclose(secondfile); if outfile is not stdout,
> fclose(outfile). Emits a verbose message "Reading from <firstfilename>
> and <secondfilename>, writing to <outfilename>". Constructs the first
> HfstInputStream from firstfilename (or stdin) and the second from
> secondfilename (or stdin); a failed construction (HfstException) is an
> EXIT_FAILURE error "<file> is not a valid transducer file". If either
> stream is in optimized-lookup format (is_input_stream_in_ol_format),
> returns EXIT_FAILURE. Otherwise returns the result of
> priority_disjunct_streams(firststream, secondstream), freeing the
> firstfilename, secondfilename and outfilename buffers.

> [spec:hfst:def:hfst-priority-disjunct.priority-disjunct-streams-fn]
> int

> [spec:hfst:sem:hfst-priority-disjunct.priority-disjunct-streams-fn]
> Computes the priority union (priority disjunction) of paired transducers
> from the two input streams and writes each result to the output stream.
> Begins by requiring at least one transducer in both streams
> (continueReading = firststream.is_good() && secondstream.is_good()).
> Determines the output type: if the two stream types differ and transducer
> conversion is allowed, calls conversion_type(type1, type2) and selects the
> former type for return value 1 or -1 (the latter warning that information
> loss is possible) or the latter type for 2 (an invalid value is a fatal
> internal error), issuing a warning describing the mismatch; if conversion
> is not allowed, errors out (EXIT_FAILURE) that the formats are not
> compatible for priority disjunction. If the types match, the output type
> is type1. Opens the HfstOutputStream on outfilename (or stdout) with the
> chosen type. Then loops while continueReading: reads one transducer from
> the first stream (incrementing transducer_n_first); if the second stream
> is good, reads one from the second stream (incrementing
> transducer_n_second). Retrieves both transducer names. For the first pair
> emits "Disjuncting <firstname> and <secondname>...", and for later pairs
> the same with the trailing count. Computes first->priority_union(second);
> if this raises a TransducerTypeMismatchException and conversion is allowed,
> converts the two transducers and retries the priority_union, otherwise
> errors out (EXIT_FAILURE) that the formats are not compatible. Sets the
> result transducer's name via hfst_set_name(.., "union") and its formula to
> the union symbol "∪", then writes it to the output stream. Recomputes
> continueReading as firststream.is_good() && (secondstream.is_good() ||
> transducer_n_second == 1), so a single transducer in the second stream is
> reused against every transducer in the first stream. Deletes the first
> transducer each iteration; deletes the second transducer unless reading
> continues against the single reused second transducer. Flushes the output
> stream. After the loop, if the first stream still has transducers it errors
> out that the second input has fewer transducers (only valid when the second
> holds exactly one); if the second stream still has transducers it errors
> out that the first input has fewer. Finally closes both input streams and
> the output stream and returns EXIT_SUCCESS.
