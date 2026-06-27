# tools/src/hfst-guessify.cc

> [spec:hfst:def:hfst-guessify.get-float-fn]
> float get_float(const std::string &str)

> [spec:hfst:sem:hfst-guessify.get-float-fn]
> Parses a single float out of the given string. Feeds the string to a
> formatted stream extraction (istringstream >> float): leading whitespace is
> skipped, then as many characters as form a valid float are consumed.
> If the extraction fails (no float could be read), returns -1. Otherwise
> returns the parsed float. Trailing characters after the number are ignored.

> [spec:hfst:def:hfst-guessify.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-guessify.main-fn]
> Program entry point. On Windows, sets stdin/stdout to binary mode. Calls
> hfst_set_program_name(argv[0], "0.3", "HfstGuessify"), then parse_options.
> If parse_options returns anything other than EXIT_CONTINUE, returns that
> value immediately. Otherwise closes the input buffer (fclose(inputfile) when
> it is not stdin) because streams are used from here on, and logs a verbose
> message "Reading from <inputfilename>, writing to <outfilename>". Constructs
> an HfstInputStream from inputfilename (or the default stdin stream when input
> is stdin); on HfstException reports error(EXIT_FAILURE, 0, "%s is not a valid
> transducer file", inputfilename) and returns EXIT_FAILURE. Constructs an
> HfstOutputStream of type HFST_OLW_TYPE on outfilename (or the default stdout
> stream when output is stdout); on HfstException reports error(EXIT_FAILURE,
> 0, "%s cannot be opened for writing.", outfilename) and returns EXIT_FAILURE.
> Calls process_stream(*instream, *outstream), frees inputfilename and
> outfilename, and returns process_stream's value.

> [spec:hfst:def:hfst-guessify.parse-options-fn]
> int

> [spec:hfst:sem:hfst-guessify.parse-options-fn]
> Parses command-line options for hfst-guessify. First calls
> extend_options_getenv(&argc, &argv) so options can also come from the
> environment. Loops calling getopt_long with the common long options, the
> unary long options, and two tool-specific long options:
> {"default-penalty", required_argument, 'p'} and
> {"do-not-compile-generator", no_argument, 'G'}; the short option string is
> the common short options, the unary short options, then "p:G". On each parsed
> option the common getopt cases are handled first, then the unary getopt
> cases, then the tool-specific cases:
>   - 'G': set compile_generator = false.
>   - 'p': set default_penalty = get_float(optarg); if it is < 0, call
>     error(EXIT_FAILURE, 0, "Invalid default penalty %s. Give a positive
>     float.", optarg).
> then the error case for any unrecognized option. The loop ends when
> getopt_long returns -1. After the loop, runs the common and unary
> parameter-checking blocks (check-params-common, check-params-unary) and
> returns EXIT_CONTINUE.

> [spec:hfst:def:hfst-guessify.print-usage-fn]
> void

> [spec:hfst:sem:hfst-guessify.print-usage-fn]
> Prints the help text to message_out. Emits the usage line
> "Usage: <program_name> [OPTIONS...] [INFILE]" followed by the one-line
> description "Compile a morphological analyzer into a guesser and generator.".
> Then prints the common program options and the common unary program options.
> Then prints the "Guesser options:" block describing -p/--default-penalty
> (penalty for skipping one symbol of input, 1.0 by default) and
> -G/--do-not-compile-generator (do not compile a model form generator). Then
> prints an explanation that all analyses in the morphological analyzer should
> have the form "w o r d f o r m POS <CATEGORY_SYMBOL_PREFIX>CLASS] X Y Z ..."
> where CATEGORY_SYMBOL_PREFIX is "[GUESS_CATEGORY=", describing POS as the
> part-of-speech tag, the category marker, and X/Y/Z as inflectional markers,
> noting CLASS may be any string not containing "]". Then prints a note about
> the -d option reducing file size by roughly half at a possible load-time
> cost, and a note that missing or "-" OUTFILE/INFILE use standard streams.
> Finally prints the report-bugs message and the more-info message.

> [spec:hfst:def:hfst-guessify.process-stream-fn]
> int

> [spec:hfst:sem:hfst-guessify.process-stream-fn]
> Processes every transducer in the input stream. Maintains a counter
> transducer_n. While instream.is_good(): increments the counter, reads one
> HfstTransducer (the morphological analyzer) from the stream, logs the verbose
> message "Compiling guesser from the transducer <analyzer name>.", then builds
> a guesser via guessify_analyzer(analyzer, default_penalty). If
> compile_generator is true, logs "Compiling generator and storing guesser and
> generator."; otherwise logs "Storing guesser.". Then calls
> store_guesser(guesser, out, compile_generator) to write the guesser (and,
> when compile_generator is true, a generator) to the output stream. After the
> loop closes the input stream and returns EXIT_SUCCESS.
