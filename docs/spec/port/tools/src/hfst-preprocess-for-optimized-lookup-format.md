# tools/src/hfst-preprocess-for-optimized-lookup-format.cc

> [spec:hfst:def:hfst-preprocess-for-optimized-lookup-format.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-preprocess-for-optimized-lookup-format.main-fn]
> Program entry point. Sets the program name to argv[0] with version "0.1" and
> wiki name "HfstPreprocessForOptimizedLookupFormat". Calls parse_options(argc,
> argv); if the return value is not EXIT_CONTINUE, returns it immediately. Then,
> since the tool uses streams rather than the buffered FILE handles, it closes
> the input FILE if it is not stdin and the output FILE if it is not stdout.
> Emits a verbose message "Reading from <inputfilename>, writing to
> <outfilename>". Opens an HfstInputStream: from inputfilename when the input is
> a named file, otherwise from standard input (the C++ wraps this in a try/catch
> that, on HfstException, reports "<inputfilename> is not a valid transducer
> file" and returns EXIT_FAILURE). Opens an HfstOutputStream on outfilename (or
> standard out) using the input stream's transducer type. Runs
> process_stream(instream, outstream), frees the input and output filenames, and
> returns its result.

> [spec:hfst:def:hfst-preprocess-for-optimized-lookup-format.parse-options-fn]
> int

> [spec:hfst:sem:hfst-preprocess-for-optimized-lookup-format.parse-options-fn]
> Standard unary-tool option parser. First calls extend_options_getenv to splice
> in any options from the environment. Loops over getopt_long with the long
> option table built from the common long options followed by the unary long
> options (and a terminating zero entry) and the short option string formed by
> concatenating HFST_GETOPT_COMMON_SHORT and HFST_GETOPT_UNARY_SHORT. There are
> no tool-specific options. Each returned option code is dispatched, in order,
> through the common option cases, then the unary option cases, then the error
> case (which reports an unknown option and returns EXIT_FAILURE). The loop ends
> when getopt_long returns -1. After the loop, runs the common parameter checks
> and the unary parameter checks, then returns EXIT_CONTINUE.

> [spec:hfst:def:hfst-preprocess-for-optimized-lookup-format.print-usage-fn]
> void

> [spec:hfst:sem:hfst-preprocess-for-optimized-lookup-format.print-usage-fn]
> Prints the help text to message_out. Writes the usage line "Usage:
> <program_name> [OPTIONS...] [INFILE]" followed by the description "Remove
> epsilons from a transducer" and a blank line. Then prints the common program
> options, the common unary program options, a blank line, the common unary
> program parameter instructions, a blank line, the bug-report footer, a blank
> line, and the "more info" footer.

> [spec:hfst:def:hfst-preprocess-for-optimized-lookup-format.process-stream-fn]
> int

> [spec:hfst:sem:hfst-preprocess-for-optimized-lookup-format.process-stream-fn]
> Reads every transducer from instream and writes a rebuilt, epsilon-free copy of
> each to outstream. Maintains a 1-based counter transducer_n. While instream is
> good: increments transducer_n, reads one HfstTransducer from instream, and
> obtains its display name via hfst_get_name(trans, inputfilename). Emits a
> verbose "Removing epsilons <name>..." message (suffixed with the transducer
> number for the second and later transducers), then calls trans.remove_epsilons().
> Emits a verbose "Rebuilding and fixing <name>..." message for the first
> transducer, or "Rebuilding and fisting <name>...<n>" for later ones.
>
> Builds an HfstBasicTransducer 'original' from trans (the
> HfstBasicTransducer(const HfstTransducer&) conversion) and an empty
> HfstBasicTransducer 'replication'. Keeps a state counter state_count starting
> at 1 and a map 'rebuilt' from original state numbers to replication state
> numbers, seeded with rebuilt[0] = 0. Iterating the states of 'original' with a
> running index source_state starting at 0: if source_state is not yet in
> 'rebuilt', adds state state_count to 'replication', copies the final weight
> when source_state is final in 'original', records rebuilt[source_state] =
> state_count, and increments state_count. For each outgoing transition (arc) of
> the state: if the arc's target state is not yet in 'rebuilt', adds state
> state_count to 'replication', copies its final weight when the target is final
> in 'original', records the mapping, and increments state_count; then adds to
> 'replication', at source state rebuilt[source_state], a new transition to
> rebuilt[target] carrying the arc's input symbol, output symbol, and weight
> (adding symbols to the alphabet). Increments source_state after each state.
>
> Replaces trans with an HfstTransducer built from 'replication' using the type
> of the previous trans. Sets the transducer name to "fu" and the formula to
> "FU" (the dest and src being the same transducer, taken from a copy on the
> Rust side). Writes trans.remove_epsilons() to outstream, then frees the name
> string. After the loop closes instream and outstream and returns EXIT_SUCCESS.
