# tools/src/hfst-name.cc

> [spec:hfst:def:hfst-name.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-name.main-fn]
> Entry point of the hfst-name tool. Initialises the tool variable
> transducer_name to a fresh copy of the empty string. Sets the program
> name via hfst_set_program_name(argv[0], "0.1", "HfstName"). Calls
> parse_options(argc, argv); if it returns anything other than
> EXIT_CONTINUE, returns that value immediately.
> Then enforces option usage: if neither --print-name nor --name was
> given, prints "Error: hfst-name: use either option --print-name  or
> --name\n" to stderr and exits with status 1. If both --print-name and
> --name were given, prints "Warning: option --print-name overrides
> option --name\n" to stderr (and continues).
> Closes the input buffer (inputfile) if it is not stdin, and the output
> buffer (outfile) if it is not stdout. Emits the verbose message
> "Reading from <inputfilename>, writing to <outfilename>\n". Opens an
> HfstInputStream on inputfilename when input is a named file, else on
> stdin; the original code wraps this in a try/catch that reports
> "<inputfilename> is not a valid transducer file" and returns
> EXIT_FAILURE on HfstException (the Rust constructor panics instead, so
> the catch arm is not reproduced). Opens an HfstOutputStream on
> outfilename when output is a named file else on stdout, using the input
> stream's transducer type. Calls process_stream(instream, outstream),
> frees inputfilename and outfilename, and returns its result.

> [spec:hfst:def:hfst-name.parse-options-fn]
> int

> [spec:hfst:sem:hfst-name.parse-options-fn]
> Parses command-line options. First calls extend_options_getenv(&argc,
> &argv) to splice in any options from the environment. Loops calling
> getopt_long over the combined long-option table (common long options,
> unary long options, then the tool-specific options {"name",
> required_argument, 'n'}, {"print-name", no_argument, 'p'},
> {"truncate_length", required_argument, 't'}, terminated by a zero
> entry) and the short-option string
> HFST_GETOPT_COMMON_SHORT HFST_GETOPT_UNARY_SHORT "n:pt:". Exits the loop
> when getopt_long returns -1.
> Each option code is dispatched first through the common-case handler
> (its print_usage closure being this tool's print_usage), then the
> unary-case handler; a handled case that should continue the loop does
> so, and one that should terminate returns its code. The tool's own
> cases follow: 'n' sets transducer_name to hfst_strdup(optarg) and sets
> name_option_given = true; 'p' sets print_name = true; 't' sets
> truncate_length = hfst_strtoul(optarg, 10). Any unrecognised code falls
> through to the error handler.
> After the loop, runs the common and unary parameter checks
> (check-params-common, check-params-unary) and returns EXIT_CONTINUE.

> [spec:hfst:def:hfst-name.print-usage-fn]
> void

> [spec:hfst:sem:hfst-name.print-usage-fn]
> Prints the tool's help text to message_out. Writes the usage line
> "Usage: <program_name> [OPTIONS...] [INFILE]\n" followed by "Name a
> transducer\n\n". Then prints the name-options block:
> "Name options:\n  -n, --name=NAME      Name the transducer NAME\n
>  -p, --print-name     Only print the current name\n  -t,
> --truncate_length=LEN   Truncate name length to LEN\n". Then calls, in
> order, print_common_program_options(message_out),
> print_common_unary_program_options(message_out), a newline,
> print_common_unary_program_parameter_instructions(message_out), a
> newline, print_report_bugs(), a newline, and print_more_info().

> [spec:hfst:def:hfst-name.process-stream-fn]
> int

> [spec:hfst:sem:hfst-name.process-stream-fn]
> Reads transducers one at a time from instream and either renames them
> (writing to outstream) or prints their names. Maintains a 1-based
> counter transducer_n. While instream.is_good(): increments the counter;
> if this is not the first transducer and print_name is set, writes
> "---\n" to stderr as a separator. Emits a verbose message: for the
> first transducer "Naming <inputfilename>...\n", otherwise "Naming
> <inputfilename>...<transducer_n>\n". Constructs an HfstTransducer from
> instream.
> If print_name is NOT set: when truncate_length > 0, sets the
> transducer's name to hfst_strndup(transducer_name, truncate_length)
> (the name truncated to at most truncate_length bytes), otherwise sets
> it to transducer_name; then writes the transducer to outstream. If
> print_name IS set: writes the transducer's current name quoted to
> stderr as "\"<name>\"\n" and does not write the transducer out.
> After the loop, closes instream and outstream and returns EXIT_SUCCESS.
