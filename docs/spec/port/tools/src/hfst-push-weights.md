# tools/src/hfst-push-weights.cc

> [spec:hfst:def:hfst-push-weights.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-push-weights.main-fn]
> Entry point of the hfst-push-weights tool. On Windows, sets stdin and stdout
> to binary mode. Calls hfst_set_program_name(argv[0], "0.1", "HfstPush"). Calls
> parse_options(argc, argv); if its return value is not EXIT_CONTINUE, returns
> that value immediately. Otherwise closes the FILE* buffers that the option
> parser opened: if inputfile is not stdin, fclose(inputfile); if outfile is not
> stdout, fclose(outfile). Emits a verbose message "Reading from <inputfilename>,
> writing to <outfilename>". Constructs the input HfstInputStream: from the named
> file inputfilename when inputfile is not stdin, otherwise from standard input;
> a failure constructing it (HfstException) is reported via error(EXIT_FAILURE, 0,
> "%s is not a valid transducer file", inputfilename) and returns EXIT_FAILURE.
> Constructs the output HfstOutputStream from the named file outfilename and the
> input stream's type when outfile is not stdout, otherwise from standard output
> and that type. If is_input_stream_in_ol_format(*instream, "hfst-push-weights")
> reports the input is an optimized-lookup format (which cannot be processed),
> returns EXIT_FAILURE. Otherwise runs process_stream(*instream, *outstream),
> frees inputfilename and outfilename, and returns its result.

> [spec:hfst:def:hfst-push-weights.parse-options-fn]
> int

> [spec:hfst:sem:hfst-push-weights.parse-options-fn]
> Parses the command-line options. First calls extend_options_getenv(&argc,
> &argv) to splice in options from the environment. Then loops calling
> getopt_long over the option tables: the common long options, the unary long
> options, plus one tool-specific option {"push", required_argument, 0, 'p'} and
> the NULL terminator; the short-option string is the common short options
> concatenated with the unary short options and "p:". The loop ends when
> getopt_long returns -1. Each returned option character is dispatched, in order,
> to the common case group, then the unary case group, then the tool's own 'p'
> case, then the error case group. For 'p': the argument optarg is matched
> case-insensitively on its first character (strncasecmp(optarg, X, 1) == 0)
> against "start", "initial" and "begin" — any match sets push_initial = true;
> otherwise against "end" and "final" — any match sets push_initial = false;
> otherwise it is an error, reported via error(EXIT_FAILURE, 0, "unknown push
> direction %s\nshould be one of start, initial, begin, end or final.\n", optarg)
> and the function returns EXIT_FAILURE. After the loop runs the common and unary
> parameter checks, then returns EXIT_CONTINUE to signal that processing should
> proceed.

> [spec:hfst:def:hfst-push-weights.process-stream-fn]
> int

> [spec:hfst:sem:hfst-push-weights.process-stream-fn]
> Processes every transducer in the input stream, pushing its weights, and writes
> the results to the output stream. Maintains a 1-based counter transducer_n.
> While instream.is_good(): increments transducer_n, reads one HfstTransducer
> from the stream, and obtains its display name via hfst_get_name(trans,
> inputfilename). Emits a verbose progress message: for the first transducer,
> "Pushing towards start <name>..." when push_initial is true else "Pushing
> towards end <name>...", and for subsequent transducers the same prefix followed
> by " <transducer_n>". When push_initial is true, applies trans.push_weights(
> hfst::TO_INITIAL_STATE), then sets the result's metadata name with
> hfst_set_name(trans, trans, "push-weights-i") and its formula with
> hfst_set_formula(trans, trans, "Id"); when push_initial is false, applies
> trans.push_weights(hfst::TO_FINAL_STATE) and sets name "push-weights-f" and
> formula "Id" the same way. Writes the transducer to the output stream
> (outstream << trans) and frees the inputname string. After the loop closes the
> input and output streams and returns EXIT_SUCCESS.
