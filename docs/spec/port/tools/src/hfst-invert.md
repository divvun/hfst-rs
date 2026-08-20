# tools/src/hfst-invert.cc

> [spec:hfst:def:hfst-invert.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-invert.main-fn]
> Entry point. Set the program name/version/wikiname via hfst_set_program_name
> (argv[0], "0.1", "HfstInvert"). Call parse_options(argc, argv); if it does not
> return EXIT_CONTINUE, return that value. Close the inputfile/outfile FILE
> buffers if they are not the standard streams (the tool re-opens via HfstStreams).
> verbose_printf the "Reading from X, writing to Y" line. Construct the
> HfstInputStream from inputfilename if a named input was given, else from stdin
> (the C wraps this in try/catch on HfstException, printing "%s is not a valid
> transducer file" and returning EXIT_FAILURE on failure). If
> is_input_stream_in_ol_format(instream, "hfst-invert") is true, return
> EXIT_FAILURE. Construct the HfstOutputStream to outfilename (named) or stdout,
> using the input stream's type. Return process_stream(instream, outstream).

> [spec:hfst:def:hfst-invert.process-stream-fn]
> int

> [spec:hfst:sem:hfst-invert.process-stream-fn]
> Read transducers from instream until it is no longer good. For each: construct
> an HfstTransducer from the stream, take its name via hfst_get_name(trans,
> inputfilename); verbose_printf "Inverting NAME..." (appending the 1-based
> transducer count on the second and later transducers); invert() it in place; set
> its name to "invert(NAME)" and its formula property to the inverse sign via the
> unary name/formula helpers (the C passes the transducer as both dest and src);
> and write it to outstream. After the loop close both streams and return
> EXIT_SUCCESS.
