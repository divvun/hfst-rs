# tools/src/hfst-split.cc

> [spec:hfst:def:hfst-split.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-split.main-fn]
> Entry point of the hfst-split tool. On Windows it first puts stdin (fd 0)
> into binary mode. It calls hfst_set_program_name(argv[0], "0.1",
> "HfstSplit"), then parse_options(argc, argv); if the latter returns anything
> other than EXIT_CONTINUE it returns that value immediately. Otherwise it
> closes the option-buffer file handles it no longer needs: if inputfile is not
> stdin it fcloses inputfile, and if outfile is not stdout it fcloses outfile.
> It emits the verbose message "Reading from <inputfilename>, writing to
> <prefix>...<extension>\n". It then constructs an HfstInputStream: if
> inputfile is not stdin it opens HfstInputStream(inputfilename), else the
> default HfstInputStream() reading stdin; if construction throws HfstException
> it calls error(EXIT_FAILURE, 0, "<inputfilename> is not a valid transducer
> file") and returns EXIT_FAILURE. It calls process_stream(instream), frees
> inputfilename, and returns process_stream's result.

> [spec:hfst:def:hfst-split.parse-options-fn]
> int

> [spec:hfst:sem:hfst-split.parse-options-fn]
> Parses the command line. It first calls extend_options_getenv(&argc, &argv)
> to splice in any options from the environment. It initialises the two
> tool-specific globals: extension := hfst_strdup(".hfst") and prefix :=
> hfst_strdup(""). It then loops calling getopt_long with the short option
> string HFST_GETOPT_COMMON_SHORT followed by "i:p:e:" and a long-option table
> consisting of the common long options plus three tool-specific ones:
> {"input", required_argument, 'i'}, {"prefix", required_argument, 'p'},
> {"extension", required_argument, 'e'}. The loop ends when getopt_long returns
> -1. Each returned option character is dispatched: the common cases are
> handled by inc/getopt-cases-common.h (help, version, verbosity, output file,
> etc.); 'i' sets inputfilename := hfst_strdup(optarg) and inputfile :=
> hfst_fopen(inputfilename, "r"), and if hfst_fopen returned stdin (optarg was
> "-") it frees inputfilename and resets it to "<stdin>", then sets inputNamed
> := true; 'p' frees the previous prefix and sets prefix := hfst_strdup(optarg);
> 'e' frees the previous extension and sets extension := hfst_strdup(optarg);
> any unrecognised character falls through to inc/getopt-cases-error.h, which
> returns EXIT_FAILURE. After the loop it runs the common parameter checks
> (inc/check-params-common.h) and the unary parameter checks
> (inc/check-params-unary.h), then returns EXIT_CONTINUE.

> [spec:hfst:def:hfst-split.print-usage-fn]
> void

> [spec:hfst:sem:hfst-split.print-usage-fn]
> Prints the tool's help text to message_out. It writes the usage line
> "Usage: <program_name> [OPTIONS...] [INFILE]" followed by the one-line
> description "Extract transducers from archive with systematic file names" and
> a blank line. It then prints the common program options
> (print_common_program_options), followed by an "Input/Output options:" block
> documenting -i/--input=INFILE, -p/--prefix=PRE and -e/--extension=EXT. After
> a blank line it prints the explanatory paragraph: if INFILE is omitted or -,
> stdin is used; if PRE is omitted, no prefix is used; if EXT is omitted, .hfst
> is used; the extracted files are named "PRE" + N + "EXT" where N is the
> number of the transducer in the archive; plus a worked example showing
> "cat transducer_a transducer_b | hfst-split -p \"rule\" -e \".tr\"" producing
> "rule1.tr" and "rule2.tr". Finally it prints the report-bugs notice
> (print_report_bugs) and the more-info notice (print_more_info), separated by
> blank lines.

> [spec:hfst:def:hfst-split.process-stream-fn]
> int

> [spec:hfst:sem:hfst-split.process-stream-fn]
> Splits the input archive into one file per transducer. It keeps a counter
> transducer_n starting at 0. While instream.is_good() it: increments
> transducer_n; builds outfilename as the concatenation prefix + transducer_n
> (rendered as a decimal integer) + extension; emits the verbose message
> "Writing <transducer_n> of <inputfilename> to <outfilename>...\n"; constructs
> a new HfstOutputStream(outfilename, instream.get_type()) (hfst format);
> reads one HfstTransducer from instream; writes that transducer to the output
> stream (operator<<); flushes and closes the output stream; and frees
> outfilename. After the loop it closes instream and returns EXIT_SUCCESS.
