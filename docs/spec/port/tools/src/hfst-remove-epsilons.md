# tools/src/hfst-remove-epsilons.cc

> [spec:hfst:def:hfst-remove-epsilons.main-fn]
> int

> [spec:hfst:sem:hfst-remove-epsilons.main-fn]
> Program entry point for the hfst-remove-epsilons tool.
> On Windows, sets stdin (fd 0) and stdout (fd 1) to binary mode.
> Calls hfst_set_program_name with argv[0], version "0.1" and wiki name
> "HfstRemoveEpsilons". Calls parse_options(argc, argv); if its return value is
> not EXIT_CONTINUE, returns that value as the process exit code.
> Then flushes/closes the option-buffer files: if inputfile is not stdin,
> fclose(inputfile); if outfile is not stdout, fclose(outfile) (the tool reads
> through streams, not these buffers).
> Emits the verbose message "Reading from <inputfilename>, writing to
> <outfilename>".
> Constructs the HfstInputStream: if inputfile is not stdin, open it from
> inputfilename, else open stdin; the construction is guarded by a try/catch on
> HfstException, and on failure reports the error "<inputfilename> is not a valid
> transducer file" via hfst_error(EXIT_FAILURE, 0, ...) and returns EXIT_FAILURE.
> Constructs the HfstOutputStream from the input stream's transducer type: if
> outfile is not stdout, open it from outfilename, else open stdout.
> If is_input_stream_in_ol_format(instream, "hfst-remove-epsilons") is true
> (optimized-lookup format cannot be processed), returns EXIT_FAILURE.
> Otherwise calls process_stream(instream, outstream), frees inputfilename and
> outfilename, and returns its result.

> [spec:hfst:def:hfst-remove-epsilons.process-stream-fn]
> int

> [spec:hfst:sem:hfst-remove-epsilons.process-stream-fn]
> Reads every transducer from instream, removes its epsilon transitions, and
> writes the result to outstream.
> If not silent, routes the library warning stream to standard error
> (hfst::set_warning_stream(&std::cerr)).
> Maintains a 1-based counter transducer_n. While instream.is_good(): increment
> the counter, read one HfstTransducer from the stream, and obtain its name via
> hfst_get_name(trans, inputfilename) — if the transducer has no name (length
> <= 0) fall back to a copy of inputfilename. For the first transducer emit the
> verbose message "Removing epsilons <inputname>...\n"; for subsequent ones emit
> "Removing epsilons <inputname>...<transducer_n>\n".
> Calls trans.remove_epsilons() to eliminate epsilon transitions, then sets the
> result transducer's name via hfst_set_name(trans, trans, "remove-epsilons") and
> its formula via hfst_set_formula(trans, trans, "Id"), and writes it to outstream
> (outstream << trans), freeing the inputname copy.
> After the loop, closes instream and outstream and returns EXIT_SUCCESS.
