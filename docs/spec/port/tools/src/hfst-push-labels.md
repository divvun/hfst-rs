# tools/src/hfst-push-labels.md

> [spec:hfst:def:hfst-push-labels.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-push-labels.main-fn]
> Program entry point. Sets the program name to argv[0] with version "0.1"
> and wiki name "HfstPush" via hfst_set_program_name. Calls parse_options;
> if its return value is not EXIT_CONTINUE, returns that value immediately.
> Closes the input buffer if it is not stdin and the output buffer if it is
> not stdout (the tool works on streams, not the raw FILE* buffers). Emits a
> verbose message "Reading from <inputfilename>, writing to <outfilename>".
> Constructs an HfstInputStream from inputfilename when input is a named file,
> otherwise from stdin; in C this is wrapped in a try/catch that reports
> "<inputfilename> is not a valid transducer file" and returns EXIT_FAILURE on
> HfstException. Constructs an HfstOutputStream to outfilename (or stdout) with
> the input stream's transducer type. If the input stream is in optimized-lookup
> format (is_input_stream_in_ol_format), returns EXIT_FAILURE. Otherwise calls
> process_stream on the two streams, frees inputfilename and outfilename, and
> returns its result.

> [spec:hfst:def:hfst-push-labels.parse-options-fn]
> int

> [spec:hfst:sem:hfst-push-labels.parse-options-fn]
> Parses command-line options. First calls extend_options_getenv to splice in
> options from the environment. Loops over getopt_long using the common long
> options, the unary long options, and one tool-specific long option
> {"push", required_argument, 0, 'p'}; the short option string is the common
> short options followed by the unary short options followed by "p:". Each
> returned option is dispatched first through the common cases, then the unary
> cases, then the tool's own 'p' case, then the error case. For 'p': the option
> argument is matched case-insensitively against its first character — if it
> begins like "start", "initial" or "begin" then push_initial is set true; if it
> begins like "end" or "final" then push_initial is set false; otherwise it calls
> error(EXIT_FAILURE, 0, "unknown push direction <optarg> ...") and returns
> EXIT_FAILURE. After the loop, runs the common and unary parameter checks and
> returns EXIT_CONTINUE.

> [spec:hfst:def:hfst-push-labels.process-stream-fn]
> int

> [spec:hfst:sem:hfst-push-labels.process-stream-fn]
> Processes every transducer in the input stream, writing the result to the
> output stream. Maintains a 1-based transducer counter. While the input stream
> is good, reads the next HfstTransducer and obtains its name via hfst_get_name
> with inputfilename. Emits a verbose message: for the first transducer
> "Pushing towards start <name>..." or "Pushing towards end <name>..." depending
> on push_initial; for later transducers the same with the transducer number
> appended. If push_initial is set, pushes labels toward the initial state
> (push_labels(TO_INITIAL_STATE)), sets the result's name to "push-labels-i" and
> formula to "Id"; otherwise pushes labels toward the final state
> (push_labels(TO_FINAL_STATE)), sets the name to "push-labels-f" and formula to
> "Id". Writes the transducer to the output stream. After the loop, closes both
> streams and returns EXIT_SUCCESS.
