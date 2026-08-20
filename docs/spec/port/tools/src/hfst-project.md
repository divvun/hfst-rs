# tools/src/hfst-project.cc

> [spec:hfst:def:hfst-project.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-project.main-fn]
> Entry point of the hfst-project tool. On Windows, sets stdin (fd 0) and
> stdout (fd 1) to binary mode. Calls hfst_set_program_name(argv[0], "0.1",
> "HfstProject"). Calls parse_options(argc, argv); if its return value is not
> EXIT_CONTINUE, returns that value immediately. Otherwise closes the buffered
> FILE handles that are no longer needed because streams take over: if inputfile
> is not stdin it is fclosed, and if outfile is not stdout it is fclosed. Emits a
> verbose message "Reading from <inputfilename>, writing to <outfilename>".
> Constructs an HfstInputStream: if inputfile is not stdin, from inputfilename,
> else the default (stdin) constructor; if the constructor throws HfstException,
> calls error(EXIT_FAILURE, 0, "<inputfilename> is not a valid transducer file")
> and returns EXIT_FAILURE. Constructs an HfstOutputStream of the input stream's
> type: if outfile is not stdout, to outfilename, else to stdout. If
> is_input_stream_in_ol_format(instream, "hfst-project") is true, returns
> EXIT_FAILURE (optimized-lookup transducers cannot be projected). Otherwise sets
> retval = process_stream(instream, outstream), frees inputfilename and
> outfilename, and returns retval.

> [spec:hfst:def:hfst-project.parse-options-fn]
> int

> [spec:hfst:sem:hfst-project.parse-options-fn]
> Parses command-line options for hfst-project. First calls
> extend_options_getenv(&argc, &argv) to splice in options from the environment.
> Loops calling getopt_long with the long-option table consisting of the common
> long options, the unary long options, plus one tool-specific option
> {"project", required_argument, 0, 'p'} (and the terminating zero entry), and
> the short-option string HFST_GETOPT_COMMON_SHORT + HFST_GETOPT_UNARY_SHORT +
> "p:". Exits the loop when getopt_long returns -1. Each returned option is
> dispatched through the common getopt cases, then the unary getopt cases, then
> the tool's own case 'p', then the terminal error case. Case 'p': inspect the
> first character of optarg case-insensitively (strncasecmp with length 1). If it
> matches the first character of "upper", "input", "first", or "analysis",
> project_input is set to true. If it matches the first character of "lower",
> "output", "second", or "generation", project_input is set to false. Otherwise
> call error(EXIT_FAILURE, 0, "unknown project direction <optarg>\nshould be one
> of upper, input, analysis, first, lower, output, second or generation\n") and
> return EXIT_FAILURE. After the loop runs the common and unary parameter checks
> and returns EXIT_CONTINUE.

> [spec:hfst:def:hfst-project.process-stream-fn]
> int

> [spec:hfst:sem:hfst-project.process-stream-fn]
> Reads every transducer from instream and writes its projection to outstream.
> Maintains a 1-based counter transducer_n. While instream.is_good(): increment
> the counter, read one HfstTransducer trans from instream, and compute its name
> via hfst_get_name(trans, inputfilename). Emit a verbose progress message: for
> the first transducer, "Projecting first <name>...\n" when project_input is
> true, else "Projecting second <name>...\n"; for subsequent transducers the same
> text with " <transducer_n>" appended before the newline. Then if project_input
> is true, call trans.input_project(), set the transducer name via
> hfst_set_name(trans, trans, "project-1st") and the formula via
> hfst_set_formula(trans, trans, "¹"); otherwise call trans.output_project()
> and set the name via hfst_set_name(trans, trans, "project-2nd") and the formula
> via hfst_set_formula(trans, trans, "²"). Write trans to outstream
> (operator<<) and free the inputname buffer. After the loop, close instream and
> outstream and return EXIT_SUCCESS.
