# tools/src/hfst-determinize.cc

> [spec:hfst:def:hfst-determinize.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-determinize.main-fn]
> Entry point of the hfst-determinize tool. On Windows, sets stdin and
> stdout to binary mode. Calls hfst_set_program_name with argv[0], version
> "0.1", and wiki name "HfstDeterminize". Calls parse_options(argc, argv);
> if its return value is not EXIT_CONTINUE, returns that value. Otherwise,
> closes the buffered FILE handles: if inputfile is not stdin, fclose it; if
> outfile is not stdout, fclose it. Saves the current value of
> hfst::get_encode_weights() into enc; if the tool's encode_weights flag is
> set, calls hfst::set_encode_weights(true). Emits a verbose message
> "Reading from <inputfilename>, writing to <outfilename>". Opens the input
> stream: if inputfile is not stdin, construct HfstInputStream(inputfilename),
> else HfstInputStream(); if the constructor throws HfstException, report the
> error "<inputfilename> is not a valid transducer file" with EXIT_FAILURE
> and return EXIT_FAILURE. Opens the output stream from the input stream's
> type: if outfile is not stdout, HfstOutputStream(outfilename, type), else
> HfstOutputStream(type). If is_input_stream_in_ol_format(*instream,
> "hfst-determinize") is true, return EXIT_FAILURE. Calls
> process_stream(*instream, *outstream) and saves the result in retval. If
> the tool's encode_weights flag was set, restores hfst::set_encode_weights(enc).
> Frees inputfilename and outfilename, and returns retval.

> [spec:hfst:def:hfst-determinize.parse-options-fn]
> int

> [spec:hfst:sem:hfst-determinize.parse-options-fn]
> Parses the command-line options for hfst-determinize. First calls
> extend_options_getenv(&argc, &argv) to splice in options from the
> environment. Loops calling getopt_long with the long-option table built
> from HFST_GETOPT_COMMON_LONG, HFST_GETOPT_UNARY_LONG, the tool-specific
> entry {"encode-weights", no_argument, 0, 'E'}, and a terminating zero
> entry; the short-option string is HFST_GETOPT_COMMON_SHORT
> HFST_GETOPT_UNARY_SHORT "E". When getopt_long returns -1, the loop breaks.
> Each returned option code is dispatched through the common getopt cases,
> then the unary getopt cases, then the error case, and finally the
> tool-specific case 'E' which sets encode_weights to true and breaks. After
> the loop, runs the common parameter checks and the unary parameter checks
> (which resolve inputfilename/inputfile and outfilename/outfile from any
> remaining positional argument). Returns EXIT_CONTINUE.

> [spec:hfst:def:hfst-determinize.print-usage-fn]
> void

> [spec:hfst:sem:hfst-determinize.print-usage-fn]
> Prints the help/usage text to message_out. Prints a usage line
> "Usage: <program_name> [OPTIONS...] [INFILE]" followed by "Determinize a
> transducer" and a blank line. Then prints, in order: the common program
> options (print_common_program_options), the common unary program options
> (print_common_unary_program_options), a "Command-specific options:" header,
> the tool-specific option line "  -E, --encode-weights         Encode
> weights when determinizing" with a continuation line
> "                               (default is false)." followed by a blank
> line, then another blank line, the common unary program parameter
> instructions (print_common_unary_program_parameter_instructions), a blank
> line, the bug-reporting info (print_report_bugs), a blank line, and the
> more-info footer (print_more_info).

> [spec:hfst:def:hfst-determinize.process-stream-fn]
> int

> [spec:hfst:sem:hfst-determinize.process-stream-fn]
> Reads transducers from instream, determinizes each, and writes them to
> outstream. Maintains a counter transducer_n starting at 0. While
> instream.is_good(): increments transducer_n; constructs a HfstTransducer
> from instream; obtains its name via hfst_get_name(trans, inputfilename).
> If this is the first transducer (transducer_n == 1) emits a verbose message
> "Determinizing <inputname>...", otherwise "Determinizing <inputname>...<n>"
> with the transducer index. Calls trans.determinize(), then sets the
> transducer's name to "determinize" via hfst_set_name(trans, trans,
> "determinize") and its formula to the U+2336 (APL functional symbol
> i-beam, "⌶") character via hfst_set_formula(trans, trans, "⌶"). Writes the
> transducer to outstream (outstream << trans) and frees inputname. After the
> loop, closes instream and outstream and returns EXIT_SUCCESS.
