# tools/src/hfst-dump-alphabets.cc

> [spec:hfst:def:hfst-dump-alphabets.alphadumpformat]
> enum alphadumpformat {
>   TSV;
>   VISLCG3_LIST;
>   VISLCG3_TAGS;
> }

> [spec:hfst:def:hfst-dump-alphabets.is-multichar-fn]
> bool

> [spec:hfst:sem:hfst-dump-alphabets.is-multichar-fn]
> Predicate over a symbol string s deciding whether it is a "multichar"
> (multi-character / special) symbol worth printing when only_multichars is set.
> Returns true only when both: (a) the byte length of s is strictly greater
> than 2, and (b) s begins with one of the prefix characters '+', ' ' (space),
> or '@' (tested with rfind(prefix, 0) == 0, i.e. starts-with). If the length is
> > 2 but the first character is none of those, return false. If the length is
> <= 2, return false. (So single-byte and two-byte symbols are never multichar.)

> [spec:hfst:def:hfst-dump-alphabets.main-fn]
> int

> [spec:hfst:sem:hfst-dump-alphabets.main-fn]
> Program entry point. On Windows, put stdin into binary mode. Set the program
> name/version/wikiname via hfst_set_program_name(argv[0], "0.1",
> "HfstSummarize"). Call parse_options(argc, argv); if it returns anything other
> than EXIT_CONTINUE, return that value immediately. Otherwise close the input
> buffer (fclose(inputfile)) unless it is stdin, then emit the verbose message
> "Reading from <inputfilename>, writing to <outfilename>". Open the input
> transducer stream: an HfstInputStream over inputfilename if a named input file
> was given, else an HfstInputStream over stdin; if construction throws
> HfstException, call error(EXIT_FAILURE, 0, "%s is not a valid transducer file",
> inputfilename) and return EXIT_FAILURE. Run process_stream on that stream
> (storing its return value). Finally close outfile (fclose(outfile)) unless it
> is stdout, free inputfilename and outfilename, and return EXIT_SUCCESS.

> [spec:hfst:def:hfst-dump-alphabets.parse-options-fn]
> int

> [spec:hfst:sem:hfst-dump-alphabets.parse-options-fn]
> Parse command-line options into the tool's global state. First call
> extend_options_getenv(&argc, &argv) to splice in any environment-provided
> options. Then loop calling getopt_long with the long-option table built from
> HFST_GETOPT_COMMON_LONG, HFST_GETOPT_UNARY_LONG, then the tool-specific long
> options { "format", required_argument, 'f' }, { "include-seen", no_argument,
> '1' }, { "include-metadata", no_argument, '2' } and the terminating
> zero entry, and short-option string HFST_GETOPT_COMMON_SHORT +
> HFST_GETOPT_UNARY_SHORT + "f:12". Exit the loop when getopt_long returns -1.
> Dispatch each returned option code c through, in order: the common cases, the
> unary cases, then the tool's own cases, then the error case. Tool cases:
>   - 'f': compare optarg against "tsv" (set output_format=TSV,
>     only_multichars=false, verbose "printing one symbol per line"),
>     "vislcg3-list" (output_format=VISLCG3_LIST, only_multichars=true, verbose
>     "printing LIST x = x ; for VISL CG 3..."), or "vislcg3-tags"
>     (output_format=VISLCG3_TAGS, only_multichars=true, verbose "printing
>     STRICT-TAGS += for VISL CG 3..."); any other value prints
>     "Error: unrecognised format <optarg>" to stderr and exits EXIT_FAILURE.
>   - '1': set print_seen = false.
>   - '2': set print_meta = false.
> After the loop, run the common and unary parameter checks
> (check-params-common, check-params-unary) and return EXIT_CONTINUE.

> [spec:hfst:def:hfst-dump-alphabets.process-stream-fn]
> int

> [spec:hfst:sem:hfst-dump-alphabets.process-stream-fn]
> Read every transducer from instream and dump its alphabet(s) to outfile.
> Maintain a 1-based counter transducer_n; for each transducer print the verbose
> message "Alphadumping...\n" for the first one and "Alphadumping... <n>\n"
> thereafter. For each transducer: read it (HfstTransducer trans{instream}) and
> build its interchange copy (HfstBasicTransducer mutt{trans}). Determine the
> header (metadata) alphabet: call trans.get_alphabet() and set
> transducerKnowsAlphabet = true on success; if it throws
> FunctionNotImplementedException, leave transducerKnowsAlphabet = false and the
> header alphabet empty. Determine the seen alphabet foundAlphabet by iterating
> every state of mutt and every transition of each state, inserting both its
> input and output symbol into foundAlphabet.
> Then emit output according to output_format:
>   - For VISLCG3_TAGS print the two "## ..." comment lines and a "STRICT-TAGS +="
>     line as a header. For VISLCG3_LIST print only the two "## ..." comment lines.
>   - If print_meta: if transducerKnowsAlphabet, iterate the header alphabet (a
>     sorted StringSet) and, skipping symbols for which only_multichars is set but
>     is_multichar is false, print each symbol s as "s\n" (TSV), "\ts\n"
>     (VISLCG3_TAGS), or "LIST s = s ;\n" (VISLCG3_LIST). If the transducer does
>     not know its alphabet, print "Error: cannot dump non-existent header
>     alphabet" to stderr and exit EXIT_FAILURE.
>   - If print_seen: iterate foundAlphabet with the same only_multichars filter
>     and the same per-format printing as above.
> After all transducers are processed, if output_format is VISLCG3_TAGS print a
> trailing "\t;\n" line to close the STRICT-TAGS block. Return EXIT_SUCCESS.
