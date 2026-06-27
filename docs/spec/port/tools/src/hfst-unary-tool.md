# tools/src/hfst-unary-tool.cc

> [spec:hfst:def:hfst-unary-tool.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-unary-tool.main-fn]
> Entry point of the unary-operation example tool. Steps:
> 1. Call hfst_set_program_name(argv[0], "0.1", "HfstUnaryTool") to register the
>    program name, version and wiki name for diagnostics and --version.
> 2. Call parse_options(argc, argv); if it returns anything other than
>    EXIT_CONTINUE, return that value immediately (an early-exit option such as
>    --help/--version was handled, or an error occurred).
> 3. Close the stdio buffers the option parser may have opened: if inputfile is
>    not stdin, fclose it; if outfile is not stdout, fclose it. (Reading/writing
>    is done through HfstInputStream/HfstOutputStream by filename instead.)
> 4. Emit a verbose message "Reading from <inputfilename>, writing to
>    <outfilename>".
> 5. Construct the input stream: if inputfile was not stdin, open
>    HfstInputStream(inputfilename), otherwise HfstInputStream() reading stdin.
>    In C++ this is wrapped in try/catch on HfstException; on failure it calls
>    error(EXIT_FAILURE, 0, "%s is not a valid transducer file", inputfilename)
>    and returns EXIT_FAILURE.
> 6. Construct the output stream of the same transducer type as the input: if
>    outfile was not stdout, HfstOutputStream(outfilename, type), otherwise
>    HfstOutputStream(type).
> 7. Call process_stream(instream, outstream) and return its result, freeing the
>    inputfilename/outfilename buffers first.

> [spec:hfst:def:hfst-unary-tool.parse-options-fn]
> int

> [spec:hfst:sem:hfst-unary-tool.parse-options-fn]
> Parse the command-line options for a unary tool. Loop calling getopt_long with
> the long-option table built from HFST_GETOPT_COMMON_LONG followed by
> HFST_GETOPT_UNARY_LONG (terminated by a zero entry) and the short-option string
> HFST_GETOPT_COMMON_SHORT concatenated with HFST_GETOPT_UNARY_SHORT. This tool
> adds no options of its own. For each returned option character c, dispatch
> through the standard switch made of the included case groups, in order:
> getopt-cases-common, getopt-cases-unary, then getopt-cases-error (the default
> arm). The common and unary cases handle things like --help (print_usage then
> exit), --version, --verbose/--silent/--quiet, input/output file selection, etc.
> getopt_long returning -1 ends the loop. After the loop, run the
> check-params-common and check-params-unary validation blocks, then return
> EXIT_CONTINUE to signal main to proceed.

> [spec:hfst:def:hfst-unary-tool.print-usage-fn]
> void

> [spec:hfst:sem:hfst-unary-tool.print-usage-fn]
> Print the tool's help text to message_out following the GNU --help convention:
> 1. The usage line "Usage: <program_name> [OPTIONS...] [INFILE]" followed by the
>    one-line description "Do things to a transducer" and a blank line.
> 2. The common program options (print_common_program_options) then the common
>    unary program options (print_common_unary_program_options), followed by a
>    blank line.
> 3. The common unary parameter instructions
>    (print_common_unary_program_parameter_instructions), followed by a blank
>    line.
> 4. The bug-report address (print_report_bugs) and the pointer to further
>    documentation (print_more_info).

> [spec:hfst:def:hfst-unary-tool.process-stream-fn]
> int

> [spec:hfst:sem:hfst-unary-tool.process-stream-fn]
> Drive the per-transducer processing loop for the unary example operation.
> First open the input and output streams (instream.open()/outstream.open()).
> Initialise transducer_n to 0. While the input stream is_good(): increment
> transducer_n; emit a verbose message — on the first transducer
> "Doing things <inputfilename>...", on subsequent transducers
> "Doing things <inputfilename>...<transducer_n>". Read the next transducer with
> HfstTransducer(instream), apply the unary operation trans.doStuff() (the
> placeholder unary operation of this example tool) and write the result to the
> output stream via outstream << ... . When the input is exhausted, close the
> input stream then the output stream and return EXIT_SUCCESS.
