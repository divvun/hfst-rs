# tools/src/hfst-multiply.cc

> [spec:hfst:def:hfst-multiply.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-multiply.main-fn]
> Entry point of the hfst-multiply (HfstDuplicate) tool. On Windows it first
> sets stdin and stdout to binary mode. It registers the program name via
> hfst_set_program_name(argv[0], "0.1", "HfstDuplicate"), then calls
> parse_options(argc, argv). If parse_options returns anything other than
> EXIT_CONTINUE, main returns that value immediately. Otherwise it flushes the
> command-line buffers it no longer needs: if inputfile is not stdin it fcloses
> inputfile, and if outfile is not stdout it fcloses outfile. It then emits a
> verbose message "Reading from <inputfilename>, writing to <outfilename>".
> It opens an HfstInputStream: from inputfilename when inputfile is not stdin,
> otherwise the default (stdin) stream; if the constructor throws HfstException
> it reports the error "<inputfilename> is not a valid transducer file" with
> status EXIT_FAILURE and returns EXIT_FAILURE. It opens an HfstOutputStream
> using the input stream's transducer type: to outfilename when outfile is not
> stdout, otherwise to stdout. It calls process_stream(instream, outstream),
> frees inputfilename and outfilename, and returns process_stream's value.

> [spec:hfst:def:hfst-multiply.parse-options-fn]
> int

> [spec:hfst:sem:hfst-multiply.parse-options-fn]
> Parses the command line for hfst-multiply. It first calls
> extend_options_getenv(&argc, &argv) to splice in any options from the
> environment. It then loops calling getopt_long with the long-option table
> formed by the common long options, the unary long options, and one
> tool-specific entry {"n-times", required_argument, 0, 'n'}, and the short
> option string HFST_GETOPT_COMMON_SHORT + HFST_GETOPT_UNARY_SHORT + "n:".
> The loop terminates when getopt_long returns -1. Each returned option code is
> dispatched through the common getopt cases, then the unary getopt cases, then
> the tool's own case: 'n' sets dupe_count = hfst_strtoul(optarg, 10) (base-10
> unsigned parse of the argument); unrecognised codes fall through to the error
> case. After the loop it runs the common parameter checks and the unary
> parameter checks, then returns EXIT_CONTINUE.

> [spec:hfst:def:hfst-multiply.print-usage-fn]
> void

> [spec:hfst:sem:hfst-multiply.print-usage-fn]
> Prints the tool's help text to message_out. It writes the usage line
> "Usage: <program_name> [OPTIONS...] [INFILE]" followed by the description
> "Use first transducer of an archive repeatedly" and a blank line. It then
> prints the common program options and the common unary program options,
> followed by an "Archive options:" section listing
> "  -n, --n-last=NUMBER   Duplicate each transducer NUMBER times". It prints a
> blank line, the common unary program parameter instructions, the note
> "NUMBER must be a positive integer as parsed by strtoul base 10", another
> blank line, the report-bugs block, a blank line, and finally the more-info
> block.

> [spec:hfst:def:hfst-multiply.process-stream-fn]
> int

> [spec:hfst:sem:hfst-multiply.process-stream-fn]
> Reads transducers from instream and writes each one to outstream dupe_count
> times. It keeps a counter transducer_n starting at 0. While instream is good,
> it increments transducer_n, reads one HfstTransducer from instream, and
> determines its name: the transducer's own name, or, if that is empty, the
> inputfilename. It emits a verbose message
> "Duplicate <name> times <dupe_count>...<transducer_n>", then writes the
> transducer to outstream dupe_count times (each iteration of i in
> [0, dupe_count) does outstream << trans). After the loop it closes instream
> and outstream and returns EXIT_SUCCESS. (The source also declares an unused
> queue<HfstTransducer> last_n that has no effect.)
