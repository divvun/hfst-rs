# tools/src/hfst-edit-metadata.cc

> [spec:hfst:def:hfst-edit-metadata.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-edit-metadata.main-fn]
> Entry point of the hfst-edit-metadata tool. On WINDOWS sets stdin to binary
> mode. Calls hfst_set_program_name(argv[0], "0.1", "HfstEditMetadata") to
> register the program name, version and wiki name. Calls parse_options(argc,
> argv); if its return value is not EXIT_CONTINUE, returns that value
> immediately (the option handling already produced the exit code, e.g. for
> --help/--version/errors). On WINDOWS, when neither print_all_properties nor
> print_property is set (i.e. transducers will be written out, not printed),
> sets stdout to binary mode. Then closes the raw FILE buffers because streams
> are used instead: if inputfile is not stdin, fclose(inputfile); if outfile is
> not stdout, fclose(outfile). Emits a verbose message "Reading from X, writing
> to Y" with inputfilename and outfilename. Constructs an HfstInputStream: if
> inputfile is not stdin, from inputfilename, else from stdin; the construction
> is wrapped in try/catch on HfstException, and on exception it reports
> error(EXIT_FAILURE, 0, "%s is not a valid transducer file", inputfilename)
> and returns EXIT_FAILURE. Constructs an HfstOutputStream: if outfile is not
> stdout, with (outfilename, instream->get_type()), else with
> (instream->get_type()). Calls process_stream(instream, outstream), frees
> inputfilename and outfilename, and returns the process_stream result.

> [spec:hfst:def:hfst-edit-metadata.parse-options-fn]
> int

> [spec:hfst:sem:hfst-edit-metadata.parse-options-fn]
> Parses command-line options. First calls extend_options_getenv(&argc, &argv)
> to splice in options from the environment. Loops over getopt_long with the
> long option table consisting of the common long options, the unary long
> options, and the tool-specific options {"add", required_argument, 'a'},
> {"print-name", optional_argument, 'p'}, {"truncate_length",
> required_argument, 't'}, terminated by a zero entry; the short option string
> is HFST_GETOPT_COMMON_SHORT + HFST_GETOPT_UNARY_SHORT + "a:p::t:". The loop
> ends when getopt_long returns -1. Each returned option code is dispatched
> through the common cases, the unary cases, the error case (which handles
> unknown options), and then the tool-specific cases:
> - 'a': search optarg for an '=' sign; if none is present, report
>   error(EXIT_FAILURE, 0, "Equals sign `=' missing from %s", optarg). Otherwise
>   split optarg at the first '=' into property (before) and value (after),
>   store properties[property] = value, set properties_given = true and
>   print_all_properties = false.
> - 'p': if optarg is non-null, set print_property = strdup(optarg); otherwise
>   set print_all_properties = true.
> - 't': set truncate_length = hfst_strtoul(optarg, 10).
> After the loop, runs the common and unary parameter checks
> (check-params-common, check-params-unary) and returns EXIT_CONTINUE.

> [spec:hfst:def:hfst-edit-metadata.print-usage-fn]
> void

> [spec:hfst:sem:hfst-edit-metadata.print-usage-fn]
> Prints the tool usage text to message_out. Prints "Usage: PROGRAM
> [OPTIONS...] [INFILE]" followed by "Name a transducer" and a blank line. Then
> prints the "Name options:" section listing -a/--add=ANAME=VALUE (add or
> replace property ANAME with VALUE), -p/--print[=NAME] (print the current
> PNAME), and -t/--truncate_length=LEN (truncate added properties' lengths to
> LEN). Then prints the common program options, the common unary program
> options, a blank line, the common unary program parameter instructions, the
> line "If PNAME is omitted, all values are printed", a blank line, the
> report-bugs text, a blank line, and the more-info text.

> [spec:hfst:def:hfst-edit-metadata.process-stream-fn]
> int

> [spec:hfst:sem:hfst-edit-metadata.process-stream-fn]
> Processes every transducer in instream. Maintains a 1-based counter
> transducer_n. While instream.is_good(): increments transducer_n; if
> transducer_n > 1 and (print_all_properties or print_property != NULL), writes
> "--- \n" to stderr as a separator between printed records. Emits a verbose
> message "Metadata X...\n" for the first transducer or "Metadata X...N\n" for
> subsequent ones (X = inputfilename, N = transducer_n). Reads the next
> transducer from instream.
> If neither print_all_properties is set nor print_property is given (i.e. we
> are in edit mode): iterate over the requested properties map in key order; for
> each property: if the key is "type", emit warning that changing 'type'
> metadata will not change the transducer's type in the file and may cause
> breakage; if the key is "version", emit warning that changing 'version'
> changes header parsing semantics; if the key is "character-encoding" and the
> value is neither "utf-8" nor "UTF-8", report error(EXIT_FAILURE, ...) because
> the encoding is unsupported. Then set the property on the transducer: if
> truncate_length > 0, set it to the value truncated to truncate_length
> (hfst_strndup of the value), otherwise set it to the value as-is. After
> applying all properties, write the transducer to outstream (outstream <<
> trans).
> Otherwise (print mode): obtain the transducer's properties map. If
> print_all_properties, write each "key: value\n" pair to outfile in key order.
> Else, look up print_property in the properties map and write its value
> followed by a newline to outfile.
> After the loop, close instream and outstream and return EXIT_SUCCESS.
