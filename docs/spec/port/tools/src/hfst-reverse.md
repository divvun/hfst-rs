# tools/src/hfst-reverse.cc

> [spec:hfst:def:hfst-reverse.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-reverse.main-fn]
> Entry point of the hfst-reverse tool. On Windows it sets stdin and stdout to
> binary mode. It calls hfst_set_program_name(argv[0], "0.1", "HfstReverse") to
> register the program name, version, and wiki page. It then calls
> parse_options(argc, argv); if the return value is not EXIT_CONTINUE the tool
> returns that value immediately. Otherwise it closes the buffered FILE handles
> the option parser opened: if inputfile is not stdin it fcloses inputfile, and
> if outfile is not stdout it fcloses outfile (the tool works through HFST
> streams instead). It emits a verbose message "Reading from <inputfilename>,
> writing to <outfilename>". It then constructs the input HfstInputStream: from
> inputfilename when an input file was named, otherwise from stdin; if the
> constructor throws HfstException it reports the error "<inputfilename> is not a
> valid transducer file" and returns EXIT_FAILURE. The output HfstOutputStream
> is constructed from outfilename with the input stream's type when an output
> file was named, otherwise from stdout with that type. If
> is_input_stream_in_ol_format(input, "hfst-reverse") is true (the optimized
> lookup format cannot be processed) it returns EXIT_FAILURE. Otherwise it calls
> process_stream(input, output), frees inputfilename and outfilename, and
> returns the result of process_stream.

> [spec:hfst:def:hfst-reverse.parse-options-fn]
> int

> [spec:hfst:sem:hfst-reverse.parse-options-fn]
> Parses the command-line options for hfst-reverse. It first calls
> extend_options_getenv(&argc, &argv) to splice in any options from the
> environment. It then loops calling getopt_long with the combined common and
> unary short option string (HFST_GETOPT_COMMON_SHORT HFST_GETOPT_UNARY_SHORT)
> and a long-option table consisting of HFST_GETOPT_COMMON_LONG followed by
> HFST_GETOPT_UNARY_LONG terminated by a {0,0,0,0} sentinel. The tool defines no
> tool-specific options. Each returned option code is dispatched through the
> common getopt cases, then the unary getopt cases, then the error case (an
> unrecognized option). The loop ends when getopt_long returns -1. After the
> loop it runs the common parameter checks then the unary parameter checks
> (resolving input/output file handles and names), and returns EXIT_CONTINUE.

> [spec:hfst:def:hfst-reverse.print-usage-fn]
> void

> [spec:hfst:sem:hfst-reverse.print-usage-fn]
> Prints the usage text for hfst-reverse to message_out. It writes the header
> "Usage: <program_name> [OPTIONS...] [INFILE]" followed by the line "Reverse a
> transducer" and a blank line. It then prints the common program options and
> the common unary program options, a newline, the common unary program
> parameter instructions, a newline, the report-bugs banner, a newline, and the
> more-info banner.

> [spec:hfst:def:hfst-reverse.process-stream-fn]
> int

> [spec:hfst:sem:hfst-reverse.process-stream-fn]
> Reverses every transducer read from the input stream and writes each result to
> the output stream. It keeps a counter transducer_n starting at 0. While the
> input stream is_good() it increments transducer_n, reads one HfstTransducer
> from the stream, and gets its name via hfst_get_name(trans, inputfilename). On
> the first transducer it emits the verbose message "Reversing <inputname>...";
> for subsequent transducers it emits "Reversing <inputname>...<transducer_n>".
> It then reverses the transducer in place with trans.reverse(), sets the result
> name with hfst_set_name(trans, trans, "reverse") and the result formula with
> hfst_set_formula(trans, trans, "\u{21c6}") (the left-right-arrow glyph), and
> writes the transducer to the output stream. The name lookup buffer is freed
> each iteration. After the loop both streams are closed and the function
> returns EXIT_SUCCESS.
