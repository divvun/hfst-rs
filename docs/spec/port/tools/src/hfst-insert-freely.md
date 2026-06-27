# tools/src/hfst-insert-freely.cc

> [spec:hfst:def:hfst-insert-freely.label-to-stringpair-fn]
> static

> [spec:hfst:sem:hfst-insert-freely.label-to-stringpair-fn]
> Parse a label string into an input:output symbol pair, returning the pair
> or nothing when the label is a single (unpaired) symbol. Scan the label
> left to right looking for a colon ':' that genuinely delimits the two
> sides. Skip colons that are not delimiters: a colon at the very start of
> the label, a colon that is the last character, and a colon escaped by a
> single preceding backslash (an escaped colon '\\:'); but a colon preceded
> by two backslashes ('\\\\:') is treated as a real delimiter. If no valid
> delimiting colon is found (or it is at index 0 or at/after the end), return
> nothing. Otherwise the first side is the substring before the colon and the
> second side is the substring after it. If either side equals the literal
> "@0@", replace that side with the internal epsilon symbol
> "@_EPSILON_SYMBOL_@". Return the resulting (first, second) string pair.

> [spec:hfst:def:hfst-insert-freely.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-insert-freely.main-fn]
> Entry point. Set the program name to argv[0] with version "0.1" and wiki
> name "HfstPush". Call parse_options; if it returns anything other than
> EXIT_CONTINUE, return that value. Close the input and output FILE buffers
> when they are real files (not stdin/stdout), since stream handling is used
> from here on. Emit a verbose message naming the input and output files.
> Construct the HfstInputStream from the input filename when reading from a
> file, otherwise from standard input (the C++ catches HfstException and
> reports "<file> is not a valid transducer file"). Construct the
> HfstOutputStream targeting the output filename or standard output, using
> the input stream's transducer type. If the input stream is in optimized
> lookup (ol) format, report via is_input_stream_in_ol_format and return
> EXIT_FAILURE. Otherwise call process_stream on the two streams and return
> its result.

> [spec:hfst:def:hfst-insert-freely.parse-options-fn]
> int

> [spec:hfst:sem:hfst-insert-freely.parse-options-fn]
> Parse the command line. First call extend_options_getenv to splice in any
> options from the environment. Loop over getopt_long with the common and
> unary long-option tables plus two tool-specific long options:
> "symbol-pair" (taking a required argument, short 'a') and "harmonise"
> (declared with a required argument, short 'H'); the short option string is
> the common and unary shorts followed by "a:H" (so 'a' takes an argument and
> 'H' does not). Dispatch each returned option through the common-case
> handler (passing print_usage), then the unary-case handler. For 'a':
> duplicate the option argument as the label; if it equals "@0@" replace it
> with the internal epsilon symbol; parse the label into the symbol pair via
> label_to_stringpair; if the (possibly substituted) label is empty, call
> error with EXIT_FAILURE explaining the source-label argument is empty and
> suggesting "@0@" or the internal epsilon symbol. For 'H': set the harmonise
> flag to true. Any unrecognized option falls through to the error case
> handler. After the loop, run the common and unary parameter checks and
> return EXIT_CONTINUE.

> [spec:hfst:def:hfst-insert-freely.print-usage-fn]
> void

> [spec:hfst:sem:hfst-insert-freely.print-usage-fn]
> Print the help text to the message output stream: a usage line
> "Usage: <program_name> [OPTIONS...] [INFILE]" followed by the description
> "Freely insert a symbol (pair)". Then print the common program options, the
> common unary program options, and the tool-specific Option block listing
> "-a, --symbol-pair=SYM   symbol pair SYM" and "-H, --harmonise   harmonise".
> Print the common unary parameter instructions and a note that SYM must be
> either a single alphabetic symbol or two symbols separated by a colon.
> Finally print the report-bugs and more-info footers.

> [spec:hfst:def:hfst-insert-freely.process-stream-fn]
> int

> [spec:hfst:sem:hfst-insert-freely.process-stream-fn]
> Read transducers from the input stream while it is good. Counting each
> transducer, fetch its name (via hfst_get_name against the input filename).
> Only for the first transducer: freely insert the parsed symbol pair into
> the transducer (insert_freely with the symbol pair and the harmonise flag —
> if harmonise is true, identity and unknown symbols are expanded by the
> symbols in the pair, otherwise not), then set the transducer's name to
> "insert-freely(<name>)" and its formula to "Id" via the unary metadata
> helpers. Write each transducer (modified first one and the rest unchanged)
> to the output stream. Close both streams and return EXIT_SUCCESS.
