# tools/src/hfst-conjunct.cc

> [spec:hfst:def:hfst-conjunct.conjunct-streams-fn]
> int

> [spec:hfst:sem:hfst-conjunct.conjunct-streams-fn]
> Reads transducers pairwise from a first and a second input stream and writes
> their intersection (conjunction) to the output stream.
>
> Set 'continueReading' to (firststream.is_good() && secondstream.is_good()):
> there must be at least one transducer in both streams. Take 'type1' from the
> first stream and 'type2' from the second; let 'output_type' start as
> UNSPECIFIED_TYPE. If 'type1 != type2': when 'allow_transducer_conversion' is
> set, compute 'ct = conversion_type(type1, type2)' and build a warning string
> "Transducer type mismatch in <firstfilename> and <secondfilename>; "; if
> ct==1 append "using former type as output" and set output_type=type1; if
> ct==2 append "using latter type as output" and set output_type=type2; if
> ct==-1 append "using former type as output, loss of information is possible"
> and set output_type=type1; any other value throws an internal error; then
> emit the assembled warning via warning(0,0,...). When conversion is NOT
> allowed, call error(EXIT_FAILURE,0,...) reporting that the two formats are not
> compatible for conjunction (--do-not-convert was requested). If the types are
> equal, set output_type=type1.
>
> Open the output stream: HfstOutputStream(outfilename, output_type) when the
> output is a named file, else HfstOutputStream(output_type) (both with the
> default hfst_format=true).
>
> Keep two transducer slots ('first', 'second') and two counters
> ('transducer_n_first', 'transducer_n_second'), both initially 0. While
> 'continueReading': read a transducer into 'first' and increment
> transducer_n_first; if the second stream is_good(), read a transducer into
> 'second' and increment transducer_n_second. Obtain 'firstname' via
> hfst_get_name(first, firstfilename); if 'second' is null throw an internal
> error; obtain 'secondname' via hfst_get_name(second, secondfilename). Print a
> verbose message "Intersecting <firstname> and <secondname>...\n" when
> transducer_n_first==1, otherwise "Intersecting <firstname> and <secondname>...
> <transducer_n_first>\n".
>
> If either operand has flag diacritics: when 'harmonize_flags' is false and not
> 'silent', warn that at least one argument contains flag diacritics and to use
> -F; when 'harmonize_flags' is true, call
> first.harmonize_flag_diacritics(second) (insert_renamed_flags defaults true).
>
> Attempt first.intersect(second, harmonize). On a
> TransducerTypeMismatchException: if 'allow_transducer_conversion' is set,
> convert_transducers(first, second) and retry first.intersect(second,
> harmonize); otherwise error(EXIT_FAILURE,0,...) reporting that the formats are
> not compatible for conjunction (--do-not-convert was requested), citing
> transducer_n_first and the two stream types.
>
> Set the result's name with hfst_set_name(first, first, second, "intersect")
> and its formula with hfst_set_formula(first, first, second, "∩"), then write
> 'first' to the output stream.
>
> Recompute 'continueReading' = firststream.is_good() && (secondstream.is_good()
> || transducer_n_second == 1). Delete 'first'. Delete 'second' only when
> (continueReading && secondstream.is_good()) or when not continueReading — i.e.
> retain the single second-stream transducer for reuse against further first
> transducers. Free 'firstname' and 'secondname'.
>
> After the loop: if firststream is still good, error(EXIT_FAILURE) that the
> second input has fewer transducers than the first (only allowed when the
> second has exactly one); if secondstream is still good, error(EXIT_FAILURE)
> that the first input has fewer transducers than the second. Close both input
> streams, flush and close the output stream, and return EXIT_SUCCESS.

> [spec:hfst:def:hfst-conjunct.main-fn]
> int

> [spec:hfst:sem:hfst-conjunct.main-fn]
> Program entry point. On Windows set stdin/stdout to binary mode. Call
> hfst_set_program_name(argv[0], "0.1", "HfstConjunct"). Call
> parse_options(argc, argv); if its return value is not EXIT_CONTINUE, return
> that value immediately.
>
> Close the stdio buffers because streams are used instead: if firstfile is not
> stdin fclose it; if secondfile is not stdin fclose it; if outfile is not
> stdout fclose it. Emit the verbose message "Reading from <firstfilename> and
> <secondfilename>, writing to <outfilename>\n".
>
> Construct the first HfstInputStream from firstfilename when firstfile is not
> stdin, else the default (stdin) stream; on HfstException error(EXIT_FAILURE)
> that firstfilename is not a valid transducer file. Construct the second
> HfstInputStream likewise from secondfilename, with the analogous error for
> secondfilename.
>
> If is_input_stream_in_ol_format() is true for either stream (program name
> "hfst-conjunct"), return EXIT_FAILURE. Otherwise call conjunct_streams(first,
> second); free firstfilename, secondfilename and outfilename; return its value.
