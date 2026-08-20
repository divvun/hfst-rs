# tools/src/hfst-txt2fst.cc

> [spec:hfst:def:hfst-txt2fst.main-fn]
> int

> [spec:hfst:sem:hfst-txt2fst.main-fn]
> Program entry point. On Windows it first sets stdout to binary mode. It calls
> hfst_set_program_name(argv[0], "0.1", "HfstTxt2Fst"), then parse_options(argc,
> argv). If parse_options returns a value other than EXIT_CONTINUE, main returns
> that value. Otherwise it closes the output buffer (fclose(outfile)) when outfile
> is not stdout, since streams are used from here on, and emits a verbose message
> "Reading from <inputfilename>, writing to <outfilename>". It then switches on the
> resolved output_format and emits a verbose message naming the chosen output
> handler: SFST -> "Using SFST as output handler"; TROPICAL_OPENFST -> "Using
> OpenFst's tropical weights as output"; LOG_OPENFST -> "Using OpenFst's log weight
> output"; FOMA -> "Using foma as output handler"; XFSM -> "Using xfsm as output
> handler"; HFST_OL -> "Using optimized lookup output"; HFST_OLW -> "Using optimized
> lookup weighted output"; any other -> hfst_error(EXIT_FAILURE, 0, "Unknown format
> cannot be used as output") and return EXIT_FAILURE.
> When output_format is XFSM_TYPE it enforces three restrictions, each via
> hfst_error(EXIT_FAILURE, 0, ...) + return EXIT_FAILURE: writing to standard output
> (outfilename == "<stdout>") is rejected; writing att format (i.e. not prolog) is
> rejected, advising '--prolog'; and reading prolog from standard input
> (inputfilename == "<stdin>") is rejected.
> It then constructs the HfstOutputStream: HfstOutputStream(outfilename,
> output_format) when outfile is not stdout, else HfstOutputStream(output_format),
> and calls process_stream on it. Finally, if inputfile is not stdin it closes it
> (fclose), frees inputfilename and outfilename, and returns EXIT_SUCCESS.

> [spec:hfst:def:hfst-txt2fst.parse-options-fn]
> int

> [spec:hfst:sem:hfst-txt2fst.parse-options-fn]
> Parses the command line. First calls extend_options_getenv(&argc, &argv) to splice
> in any options from the environment. Then loops over getopt_long using the common +
> unary short option strings plus "e:nf:pjC", and the long-option table built from
> HFST_GETOPT_COMMON_LONG, HFST_GETOPT_UNARY_LONG and the tool options:
> {epsilon,required,'e'}, {number,no_arg,'n'}, {format,required,'f'},
> {prolog,no_arg,'p'}, {disjunct,no_arg,'j'}, {check-negative-epsilon-cycles,no_arg,
> 'C'}, {Wstuff,required,'W'}. The switch first runs the common cases
> (getopt-cases-common.h) then the unary cases (getopt-cases-unary.h), then the
> tool-specific cases: 'e' sets epsilonname = hfst_strdup(optarg); 'j' sets
> disjunct_multiple_transducers = true; 'n' sets use_numbers = true (unused); 'p' sets
> read_prolog_format = true; 'f' sets output_format = hfst_parse_format_name(optarg);
> 'C' sets check_negative_epsilon_cycles = true; 'W' sets warnings_are_errors (for
> "error"/"no-error") or warn_negative_weights (for "negative-weights"/
> "no-negative-weights"), and for any other -W value emits hfst_error(EXIT_FAILURE, 0,
> "Unrecognised warning switch -W<value>") and returns EXIT_FAILURE. Unknown options
> fall to the error case (getopt-cases-error.h).
> After the loop it runs check-params-common.h and check-params-unary.h. If epsilonname
> is still NULL it defaults to hfst_strdup("@0@") with a verbose message about the
> default epsilon representation. If output_format is UNSPECIFIED_TYPE it defaults to
> TROPICAL_OPENFST_TYPE with a verbose message about the default output format. If
> output_format is XFSM_TYPE and read_prolog_format and check_negative_epsilon_cycles
> are all set, it emits hfst_error(EXIT_FAILURE, 0, ...) explaining that checking
> negative epsilon cycles is not supported when reading prolog and outputting xfsm, and
> returns EXIT_FAILURE. Otherwise returns EXIT_CONTINUE.

> [spec:hfst:def:hfst-txt2fst.process-stream-fn]
> int

> [spec:hfst:sem:hfst-txt2fst.process-stream-fn]
> Reads transducers from inputfile and writes them to outstream. Keeps a transducer
> counter (transducer_n) and a line counter (linecount, used for prolog/att error
> reporting). Loops while !feof(inputfile). At the top of each iteration it increments
> transducer_n and emits a verbose message: "Reading transducer table...\n" for the
> first one, or "Reading transducer table <n>...\n" thereafter.
> Three mutually exclusive modes:
> (1) read_prolog_format is true. If output_format is XFSM_TYPE it calls
>     HfstTransducer::prolog_file_to_xfsm_transducer(inputfilename), writes the result
>     to outstream (outstream << *t), deletes t, flushes outstream and breaks the loop;
>     on HfstException it emits hfst_error(EXIT_FAILURE, 0, ...) about failing to
>     convert the prolog text file to xfsm and returns EXIT_FAILURE. Otherwise (non
>     xfsm) it reads one graph via HfstBasicTransducer::read_in_prolog_format(inputfile,
>     linecount). If check_negative_epsilon_cycles is set it emits a verbose check
>     message and, if fsm.has_negative_epsilon_cycles() is true and not silent, issues
>     hfst_warning(0, 0, "Transducer has epsilon cycles with a negative weight.\n"),
>     else a verbose "no negative cycles" message. It then builds HfstTransducer(fsm,
>     output_format), sets its name via hfst_set_name(t, inputfilename, "text") and
>     formula via hfst_set_formula(t, inputfilename, "T"), and writes it to outstream.
>     On NotValidPrologFormatException it emits hfst_error(EXIT_FAILURE, 0, "Error in
>     processing transducer text file (prolog) on line <linecount>") and returns
>     EXIT_FAILURE.
> (2) disjunct_multiple_transducers is true. It reads every transducer in the att file
>     into a vector via repeated HfstTransducer(inputfile, output_format, epsilonname,
>     warn_negative_weights) while !feof(inputfile); on NotValidAttFormatException it
>     prints "Error reading transducer: not valid AT&T format.". It then creates an
>     empty HfstTransducer(output_format) and disjuncts each collected transducer into
>     it, then writes the joined transducer to outstream.
> (3) otherwise (single att transducer). It reads HfstTransducer(inputfile,
>     output_format, epsilonname, linecount, warn_negative_weights), sets its name
>     ("text") and formula ("T") as above. If check_negative_epsilon_cycles is set it
>     emits the verbose check message, builds an HfstBasicTransducer from t, and if it
>     has negative epsilon cycles and not silent issues the same hfst_warning, else the
>     verbose "no negative cycles" message. It writes t to outstream. On
>     NotValidAttFormatException it emits hfst_error(EXIT_FAILURE, 0, "Error in
>     processing transducer text file (att) on line <linecount>") and returns
>     EXIT_FAILURE.
> After the loop it closes outstream and returns EXIT_SUCCESS.
