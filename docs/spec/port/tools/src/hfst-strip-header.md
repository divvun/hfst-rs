# tools/src/hfst-strip-header.cc

> [spec:hfst:def:hfst-strip-header.main-fn]
> int main(int argc, char* argv[])

> [spec:hfst:sem:hfst-strip-header.main-fn]
> Entry point of the hfst-strip-header tool. Sets the program name via
> hfst_set_program_name(argv[0], "0.1", "HfstStripHeader"). Calls
> parse_options(argc, argv); if its return value is not EXIT_CONTINUE, returns
> that value immediately (this covers --help, --version and error exits). Then
> emits the verbose message "Reading from <inputfilename>, writing to
> <outfilename>\n". Calls process_stream(inputfile, outfile) — the raw input and
> output FILE* handles that parse_options' check-params step resolved (defaulting
> to stdin/stdout). Frees inputfilename and outfilename and returns the value
> process_stream produced.

> [spec:hfst:def:hfst-strip-header.parse-options-fn]
> int

> [spec:hfst:sem:hfst-strip-header.parse-options-fn]
> Parses command-line options into the global tool state. Loops calling
> getopt_long with the concatenation of the common and unary short-option strings
> and a long-option table built from the common long options followed by the
> unary long options followed by a terminating zero entry. The tool defines no
> options of its own. Each returned option code is dispatched first through the
> common case group, then the unary case group, then the error case group (an
> unrecognised option prints usage and exits with failure); a case that requests
> "break" continues the loop, one that requests "return" returns its code. When
> getopt_long returns -1 the loop ends and the common and unary check-params
> steps run (resolving input/output file handles and names, defaulting to
> stdin/stdout). Returns EXIT_CONTINUE to signal main to proceed.

> [spec:hfst:def:hfst-strip-header.print-usage-fn]
> void

> [spec:hfst:sem:hfst-strip-header.print-usage-fn]
> Prints the tool's help text to message_out: a usage line
> "Usage: <program_name> [OPTIONS...] [INFILE]" followed by the description
> "Remove any HFST3 headers" and a blank line. Then prints the common program
> options, the common unary program options, a blank line, the common unary
> program parameter instructions, a blank line, the report-bugs notice, a blank
> line and the more-info notice.

> [spec:hfst:def:hfst-strip-header.process-stream-fn]
> int

> [spec:hfst:sem:hfst-strip-header.process-stream-fn]
> Copies bytes from input FILE* f_in to output FILE* f_out, removing embedded
> HFST3 headers. Matches against the literal "HFST3" together with its trailing
> NUL terminator (six bytes, indices 0..5). Keeps header_loc, the count of header
> bytes matched so far (initially 0). Reads one character c at a time with getc:
> on EOF returns EXIT_SUCCESS. For every character read it emits the verbose
> message "Stripping...\n". If c equals header[header_loc]: when header_loc == 5
> the full header including its NUL terminator has matched, so it consumes
> characters until the next NUL byte or EOF and resets header_loc to 0; otherwise
> it increments header_loc to look for the next header character. If c does not
> match: when header_loc > 0, the partially matched bytes were not a header, so
> it flushes header[0..header_loc] to f_out, resets header_loc to 0, and pushes c
> back onto f_in with ungetc (since c may begin a new header); when header_loc is
> 0 it simply writes c to f_out. The loop never terminates except by the EOF
> return.
