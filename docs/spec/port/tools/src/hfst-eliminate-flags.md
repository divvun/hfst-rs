# tools/src/hfst-eliminate-flags.cc

> [spec:hfst:def:hfst-eliminate-flags.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-eliminate-flags.main-fn]
> Program entry point for the 'hfst-eliminate-flags' tool. On Windows it sets
> stdin (fd 0) and stdout (fd 1) to binary mode. Calls hfst_set_program_name
> with argv[0], version "0.1" and wiki name "HfstEliminateFlags". Calls
> parse_options(argc, argv); if the returned value is not EXIT_CONTINUE it
> returns that value immediately. Then, since the tool uses streams rather than
> the FILE buffers, it closes the input FILE if it is not stdin and the output
> FILE if it is not stdout. Emits a verbose message "Reading from <inputfilename>,
> writing to <outfilename>". Opens an HfstInputStream: if the input file is not
> stdin it constructs one from inputfilename, otherwise the default (stdin)
> constructor; in C this is wrapped in try/catch and on an HfstException it calls
> error(EXIT_FAILURE, 0, "<inputfilename> is not a valid transducer file") and
> returns EXIT_FAILURE. It then opens an HfstOutputStream parameterised by the
> input stream's transducer type: to outfilename when output is not stdout,
> otherwise to stdout. Calls is_input_stream_in_ol_format(instream,
> "hfst-eliminate-flags"); if true (optimized-lookup input, which cannot be
> processed) it returns EXIT_FAILURE. Otherwise it calls
> process_stream(instream, outstream), frees inputfilename and outfilename, and
> returns that result.

> [spec:hfst:def:hfst-eliminate-flags.parse-options-fn]
> int

> [spec:hfst:sem:hfst-eliminate-flags.parse-options-fn]
> Parses the command-line options. First calls extend_options_getenv(&argc,
> &argv) to splice in any options from the environment. Then loops calling
> getopt_long over a long-option table consisting of the common long options,
> the unary long options, a tool-specific {"flag", required_argument, 0, 'F'}
> entry, and a terminating zero entry; the short-option string is the common
> short options concatenated with the unary short options and "F:". The loop
> ends when getopt_long returns -1. For each returned option character it
> dispatches in order: the common cases (which include -h/--help printing usage,
> -V/--version, the verbosity flags, and may return an exit code or break to
> continue), then the unary cases (-i/--input, -o/--output), then the
> tool-specific case 'F' which sets the global 'flag' to a duplicated copy of
> optarg, and finally the error case for any unrecognized option. After the loop
> it runs the common parameter checks and the unary parameter checks, then
> returns EXIT_CONTINUE.

> [spec:hfst:def:hfst-eliminate-flags.print-usage-fn]
> void

> [spec:hfst:sem:hfst-eliminate-flags.print-usage-fn]
> Prints the tool's help text to message_out. Emits the usage line
> "Usage: <program_name> [OPTIONS...] [INFILE]" followed by the description
> "Eliminate flags from a transducer" and a blank line. Then prints the common
> program options and the common unary program options. Then prints a
> "Command-specific options:" header followed by the single tool option line
> "  -F, --flag=FLAG        Only eliminate flag FLAG" (with a trailing blank
> line), then another blank line, then the common unary program parameter
> instructions, a blank line, the "report bugs" footer, a blank line, and the
> "more info" footer.

> [spec:hfst:def:hfst-eliminate-flags.process-stream-fn]
> int

> [spec:hfst:sem:hfst-eliminate-flags.process-stream-fn]
> Processes every transducer in the input stream, eliminating flag diacritics,
> and writes the results to the output stream. If not in silent mode it routes
> the library warning stream to standard error. It computes a description string
> 'flags': if the global 'flag' is unset the string is "flags", otherwise it is
> "flag " concatenated with the flag value. Then, while the input stream is good,
> it reads the next transducer and increments a 1-based counter transducer_n. It
> obtains the transducer's name via hfst_get_name(trans, inputfilename); if the
> name is empty it falls back to a copy of inputfilename. It emits a verbose
> message: for the first transducer "Eliminating <flags> <inputname>...", and for
> subsequent ones "Eliminating <flags> <inputname>...<transducer_n>". If the
> global 'flag' is unset it calls trans.eliminate_flags(); otherwise it calls
> trans.eliminate_flag(flag) wrapped so that if it raises an HfstException (the
> named flag feature does not occur in the transducer) it calls
> error(EXIT_FAILURE, 0, "flag feature <flag> does not occur in the
> transducer\nonly the flag feature must be given, no value or operator") and
> returns EXIT_FAILURE. It then sets the result transducer's name to
> "eliminate-flags" and its formula to "Id" (both derived from the transducer
> itself), and writes the transducer to the output stream. After the loop it
> closes the input and output streams and returns EXIT_SUCCESS.
