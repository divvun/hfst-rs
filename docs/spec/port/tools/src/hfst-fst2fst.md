# tools/src/hfst-fst2fst.cc

> [spec:hfst:def:hfst-fst2fst.main-fn]
> int

> [spec:hfst:sem:hfst-fst2fst.main-fn]
> Program entry point. On Windows it first puts stdin (fd 0) and stdout
> (fd 1) into binary mode; on other platforms this is a no-op.
> Calls hfst_set_program_name(argv[0], "0.1", "HfstFst2Fst"), then
> parse_options(argc, argv). If parse_options returns anything other than
> EXIT_CONTINUE, returns that value immediately.
> Otherwise closes the buffered input/output files we opened (fclose
> inputfile unless it is stdin, fclose outfile unless it is stdout) because
> the tool works with streams from here on.
> Emits a verbose message "Reading from <inputfilename>, writing to
> <outfilename>". Then, if hfst_format is true and the output type is not
> XFSM_TYPE, emits "Writing <strformat(output_type)> format transducers with
> HFST3 headers"; otherwise emits "Writing <strformat(output_type)> format
> transducers without HFST specific headers".
> If the output type is XFSM_TYPE and the output filename equals the literal
> "<stdout>", reports a fatal error ("Writing to standard output not
> supported for xfsm transducers, use 'hfst-fst2fst [--output|-o] OUTFILE'
> instead") and returns EXIT_FAILURE — xfsm transducers cannot be written to
> stdout.
> Constructs the HfstInputStream: HfstInputStream(inputfilename) when reading
> from a real file, else HfstInputStream() for stdin. In C this is wrapped in
> try/catch handling FileIsInGZFormatException (the file is a gzipped native
> foma file and must be gunzipped first), ImplementationTypeNotAvailableException
> (the file is in an unavailable format), and HfstException (not a valid
> transducer file; additionally, when reading from stdin under HAVE_XFSM, notes
> that xfsm transducers cannot be read from stdin) — each reporting a fatal
> error and returning EXIT_FAILURE. The Rust port's constructor panics instead
> of throwing, so those catch arms are not reproduced.
> Constructs the HfstOutputStream: HfstOutputStream(outfilename, output_type,
> hfst_format) for a real output file, else HfstOutputStream(output_type,
> hfst_format) for stdout.
> Calls process_stream(instream, outstream), frees inputfilename and
> outfilename, and returns its result.

> [spec:hfst:def:hfst-fst2fst.parse-options-fn]
> int

> [spec:hfst:sem:hfst-fst2fst.parse-options-fn]
> Parses the command-line options. First calls extend_options_getenv(&argc,
> &argv) to splice in options from the environment. Then loops calling
> getopt_long over the option table, which is the common long options,
> followed by the unary long options, followed by these tool-specific long
> options: --use-backend-format ('b', no arg), --format ('f', required arg),
> --sfst ('S'), --foma ('F'), --xfsm ('x'), --openfst-tropical ('t'),
> --openfst-log ('l'), --optimized-lookup-unweighted ('O'),
> --optimized-lookup-weighted ('w'), --quick ('Q'); all but --format take no
> argument. The short-option string is the common short options, the unary
> short options, then "SFtlOwQf:bx".
> The loop exits when getopt_long returns -1. For each returned option code
> it is dispatched in order through the common cases, then the unary cases,
> then the tool-specific cases below, then the terminal error case:
>   - 'f': set_output_type(hfst_parse_format_name(optarg)). Under !HAVE_XFSM,
>     if the parsed type is XFSM_TYPE, reports the fatal error "xfsm back-end
>     is not available".
>   - 'b': set hfst_format to false.
>   - 'S': set_output_type(SFST_TYPE).
>   - 'F': set_output_type(FOMA_TYPE).
>   - 'x': under HAVE_XFSM, set_output_type(XFSM_TYPE); otherwise reports the
>     fatal error "xfsm back-end is not available". (This port is built
>     without HAVE_XFSM, so the error arm applies.)
>   - 't': set_output_type(TROPICAL_OPENFST_TYPE).
>   - 'l': set_output_type(LOG_OPENFST_TYPE).
>   - 'O': set_output_type(HFST_OL_TYPE).
>   - 'w': set_output_type(HFST_OLW_TYPE).
>   - 'Q': set the options string to "quick".
> After the loop, if output_type is still UNSPECIFIED_TYPE, reports the fatal
> error "You must specify an output type (one of -S, -F, -t, -x, -l, -O, or
> -w)". Then runs the common parameter checks and the unary parameter checks
> and returns EXIT_CONTINUE.

> [spec:hfst:def:hfst-fst2fst.process-stream-fn]
> int

> [spec:hfst:sem:hfst-fst2fst.process-stream-fn]
> Reads every transducer from instream, converts each to the requested output
> type, and writes it to outstream. First, if the input stream's type is
> FOMA_TYPE and it does not include an HFST header (a native foma transducer)
> and the tool is not silent, emits a warning that inversion may be needed for
> hfst-lookup to work as expected (and that hfst-flookup behaves like foma's
> flookup).
> Then, with a transducer counter starting at 0, while the input stream is
> good: increment the counter, construct a transducer from the stream, fetch
> its name via hfst_get_name(orig, inputfilename). If this is the first
> transducer emit a verbose "Converting <name>...", otherwise "Converting
> <name>...<n>" where n is the counter. Convert the transducer in place to
> output_type using the options string (in C wrapped in try/catch on
> HfstException; the Rust conversion panics instead, so no catch arm).
> Set the transducer's name metadata to "convert" via hfst_set_name(orig,
> orig, "convert") and its formula metadata to "Id" via hfst_set_formula(orig,
> orig, "Id") — both resolving to the transducer-source overloads — then write
> the transducer to the output stream and free the fetched name. In C both
> metadata calls pass orig as both source and destination; Rust cannot alias a
> mutable and an immutable borrow of the same value, so the read side is taken
> from a clone of orig before the in-place metadata update.
> After the loop, flushes the output stream (needed for xfsm transducers whose
> writing is delayed), closes the input stream, closes the output stream, and
> returns EXIT_SUCCESS.

> [spec:hfst:def:hfst-fst2fst.set-output-type-fn]
> void

> [spec:hfst:sem:hfst-fst2fst.set-output-type-fn]
> Sets the global output_type to the given implementation type. If output_type
> has already been set to something other than UNSPECIFIED_TYPE, reports a
> fatal error ("Output type defined several times.") before assigning. This
> enforces that at most one output-type option is supplied on the command line.
