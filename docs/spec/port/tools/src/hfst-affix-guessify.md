# tools/src/hfst-affix-guessify.cc

> [spec:hfst:def:hfst-affix-guessify.guess-direction]
> enum guess_direction {
>   GUESS_PREFIX;
>   GUESS_SUFFIX;
> }

> [spec:hfst:def:hfst-affix-guessify.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-affix-guessify.main-fn]
> Entry point of the 'hfst-affix-guessify' tool.
> 1. Register the program name as 'HfstAffixGuessify', version '0.1', with
>    'hfst_set_program_name(argv[0], "0.1", "HfstAffixGuessify")'.
> 2. Call 'parse_options(argc, argv)'. If it returns a value other than
>    EXIT_CONTINUE, return that value immediately.
> 3. If the input file is a real file (not stdin), 'fclose' it now; the actual
>    reading is done through an 'HfstInputStream'. Emit the verbose message
>    "Reading from <inputfilename>, writing to <outfilename>".
> 4. Construct the input stream: 'HfstInputStream(inputfilename)' when reading a
>    named file, otherwise 'HfstInputStream()' for stdin. In C++ a failed
>    construction is caught as an 'HfstException' and reported with
>    "<inputfilename> is not a valid transducer file" followed by returning
>    EXIT_FAILURE.
> 5. Construct the output stream from the input stream's type:
>    'HfstOutputStream(outfilename, type)' for a named output file, otherwise
>    'HfstOutputStream(type)' for stdout.
> 6. If 'is_input_stream_in_ol_format(instream, "hfst-affix-guessify")' is true
>    (the input is in optimized-lookup format, which this tool cannot process),
>    return EXIT_FAILURE.
> 7. Call 'process_stream(instream, outstream)' and keep its return value.
> 8. If the output went to a real file, 'fclose' it; free 'inputfilename' and
>    'outfilename'. Return EXIT_SUCCESS.

> [spec:hfst:def:hfst-affix-guessify.parse-options-fn]
> int

> [spec:hfst:sem:hfst-affix-guessify.parse-options-fn]
> Parse the command line into the global tool state.
> 1. Call 'extend_options_getenv(&argc, &argv)' first so that options can also
>    come from the environment.
> 2. Loop over 'getopt_long' with the long-option table built from the common
>    options, the unary options, plus two tool-specific options:
>    '{"weight", required_argument, 0, 'w'}' and
>    '{"direction", required_argument, 0, 'D'}' (terminated by a zero entry); and
>    the short-option string HFST_GETOPT_COMMON_SHORT + HFST_GETOPT_UNARY_SHORT +
>    "w:D:". Break out of the loop when 'getopt_long' returns -1.
> 3. Dispatch each returned option code through, in order: the common cases, the
>    unary cases, then the tool's own cases, then the error case:
>    - 'w': set the global 'weight' to 'hfst_strtoweight(optarg)'.
>    - 'D': if 'optarg' begins with "prefix", set 'direction' to GUESS_PREFIX; if
>      it begins with "suffix", set 'direction' to GUESS_SUFFIX; otherwise call
>      'error(EXIT_FAILURE, 0, ...)' with the message
>      "Unable to parse guessing direction from <optarg>; please use one of
>      'prefix' or 'suffix'".
> 4. After the loop, run the common parameter checks and the unary parameter
>    checks, then return EXIT_CONTINUE.

> [spec:hfst:def:hfst-affix-guessify.process-stream-fn]
> int

> [spec:hfst:sem:hfst-affix-guessify.process-stream-fn]
> Read every transducer from 'instream' and write its affix guesser to
> 'outstream'. Maintain a 1-based counter 'transducer_n'.
> While 'instream.is_good()':
> 1. Read the next transducer 'trans'. Determine 'inputname' as 'trans.get_name()'
>    if non-empty, otherwise the global 'inputfilename'. Emit
>    "Guessifying <inputname>..." for the first transducer, or
>    "Guessifying <inputname>... <transducer_n>" for subsequent ones.
> 2. Take 'alpha = trans.get_alphabet()' (the transducer's alphabet, a string
>    set). Branch on the global 'direction':
>
>    GUESS_SUFFIX:
>    a. Verbose "Creating guesser prefix...". Make a basic transducer 'mutt' that
>       is a copy of 'trans', and a fresh empty basic transducer 'repl'.
>    b. Add state 0 to 'repl' and call it 'guess_state'. Add a self-loop on
>       'guess_state' that maps internal-identity to internal-identity with the
>       global 'weight'. For every symbol 'x' in 'alpha', add a self-loop on
>       'guess_state' mapping 'x' to 'x' with 'weight'. (These arcs are the part
>       that "guesses" arbitrary unknown prefix material.)
>    c. Verbose "Rebuilding suffix...". For each state 's' of 'mutt' from 0 up to
>       and including 'mutt.get_max_state()':
>       - Add state 's + 1' to 'repl' (call it 'd'); the +1 shift makes room for
>         the new 'guess_state' at index 0.
>       - If 's' is final in 'mutt', set 'd' final in 'repl' with the same final
>         weight.
>       - Add an internal-identity self-loop from 'guess_state' to 'd' with
>         'weight', and for every symbol 'x' in 'alpha' an 'x:x' arc from
>         'guess_state' to 'd' with 'weight'. (These let the guesser jump from the
>         guessing prefix into any point of the original suffix automaton.)
>       - Copy every original transition out of 's' into 'd', retargeted to
>         'target + 1' and keeping the same input/output symbols and weight.
>    d. Verbose "converting and saving...". Convert 'repl' to an 'HfstTransducer'
>       of the tool's 'format' and write it to 'outstream'.
>
>    GUESS_PREFIX:
>    a. Verbose "Creating guesser suffix...". Make a basic transducer 'repl' that
>       is a copy of 'trans'.
>    b. Add a new state 'guess_state' (appended after the existing states) and
>       make it final with weight 0. Add an internal-identity self-loop on
>       'guess_state' with 'weight'.
>    c. Verbose "Linking prefix...". For each state 's' from 0 up to and including
>       'repl.get_max_state()' (computed before adding the linking arcs), add an
>       internal-identity arc from 's' to 'guess_state' with 'weight'. (Every
>       state can transition into the guessing suffix.)
>    d. Verbose "Converting and saving...". Convert 'repl' to an 'HfstTransducer'
>       of the tool's 'format' and write it to 'outstream'.
> Return EXIT_SUCCESS when the stream is exhausted.
