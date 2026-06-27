# tools/src/hfst-format.cc

> [spec:hfst:def:hfst-format.main-fn]
> int main (int argc, char * argv[])

> [spec:hfst:sem:hfst-format.main-fn]
> Entry point of the 'hfst-format' tool. On Windows, sets stdin to binary
> mode. Calls hfst_set_program_name(argv[0], "0.1", "HfstFormat") to register
> the program name, version and wiki name. Forces the global 'verbose' flag to
> true so the single result line is always emitted. Calls parse_options(argc,
> argv), which (unless it has already exited for --list-formats, --test-format,
> an error, or a non-transducer stream) returns the ImplementationType of the
> input stream; this value is the program's effective transducer type. Then
> emits, via verbose_printf, the line
> "Transducers in <inputfilename> are of type <hfst_strformat(type)>\n",
> where <inputfilename> is the global input filename set during parse_options
> (e.g. "<stdin>" or the named file) and hfst_strformat maps the
> ImplementationType to its human-readable description. main performs no other
> work and does not run a process_stream loop.

> [spec:hfst:def:hfst-format.parse-options-fn]
> int

> [spec:hfst:sem:hfst-format.parse-options-fn]
> Parses the command line and, depending on the options, either lists formats,
> tests a format, or opens the input stream to determine its type. Maintains two
> tool-local flags: list_formats (bool, default false) and format_to_test
> (string, default null).
>
> Option table: the common long options, the unary long options, plus the
> tool-specific entries
>   --input1 (required arg, val '1'),
>   --input2 (required arg, val '2'),
>   --list-formats (no arg, val 'l'),
>   --test-format (required arg, val 't'),
> terminated by a null entry. The short option string is the common short
> options, then the unary short options, then "1:2:lt:".
>
> Loop calling getopt_long until it returns -1. For each option:
>   - the common getopt cases are handled first (e.g. --help prints usage and
>     exits, --version, --verbose, --silent, --output, etc.);
>   - then the unary getopt cases;
>   - then the tool's own cases:
>       '1': set inputfilename = strdup(optarg);
>       '2': set inputfilename = strdup(optarg);  (same effect as '1')
>       'l': set list_formats = true;
>       't': set format_to_test = strdup(optarg);
>       default: do nothing — this tool deliberately ignores other options
>                (it does NOT fall through to the common error handler).
>
> After the loop:
>   1. If format_to_test is non-null: exit(0) if format_to_test equals one of
>      the recognized format names AND the corresponding implementation type is
>      available; otherwise exit(1). The mapping is:
>        "sfst" -> SFST_TYPE,
>        "openfst-tropical" -> TROPICAL_OPENFST_TYPE,
>        "openfst-log" -> LOG_OPENFST_TYPE,
>        "foma" -> FOMA_TYPE,
>        "optimized-lookup-unweighted" -> HFST_OL_TYPE,
>        "optimized-lookup-weighted" -> HFST_OLW_TYPE.
>      Availability is checked via HfstTransducer::is_implementation_type_available.
>   2. Else if list_formats is set: print to stdout a header
>      " Backend                         Names recognized\n\n", then one line per
>      available backend (each guarded by is_implementation_type_available):
>        SFST_TYPE              -> " SFST                            sfst\n"
>        TROPICAL_OPENFST_TYPE  -> " OpenFst (tropical weights)      openfst-tropical, openfst, ofst, ofst-tropical\n"
>        LOG_OPENFST_TYPE       -> " OpenFst (logarithmic weights)   openfst-log, ofst-log\n"
>        FOMA_TYPE              -> " foma                            foma\n"
>        HFST_OL_TYPE           -> " Optimized lookup (weighted)     optimized-lookup-unweighted, olu\n"
>        HFST_OLW_TYPE          -> " Optimized lookup (unweighted)   optimized-lookup-weighted, olw, optimized-lookup, ol\n"
>      then exit(0).
>   3. Otherwise determine the input type. Wrapped in a try/catch on
>      HfstException: if inputfilename is still null, then with (argc - optind)
>      arguments remaining: if 0, set inputfilename = strdup("<stdin>"), open an
>      HfstInputStream on "" (stdin) and return is.get_type(); if 1, set
>      inputfilename = argv[optind]. Then open an HfstInputStream on inputfilename
>      and return is.get_type(). If opening throws HfstException (the stream is
>      not a transducer stream), print
>      "ERROR: The file/stream does not contain transducers.\n" to stderr and
>      exit(1).
>
> The returned int is the ImplementationType of the input stream, consumed by
> main. In the Rust port the return type is ImplementationType directly (rather
> than int round-tripped through a cast), since the Rust enum discriminants do
> not match the C++ enum's numeric layout; behaviour is otherwise identical.

> [spec:hfst:def:hfst-format.print-usage-fn]
> void

> [spec:hfst:sem:hfst-format.print-usage-fn]
> Prints the tool's help text to message_out. First the usage line
> "Usage: <program_name> [OPTIONS...] [INFILE]\ndetermine HFST transducer
> format\n\n". Then print_common_program_options(message_out) and
> print_common_unary_program_options(message_out). Then a tool-specific options
> block:
>   "Tool-specific options:\n"
>   "  -l, --list-formats     List available transducer formats\n"
>   "                         and print them to standard output\n"
> followed by
>   "  -t, --test-format FMT  Whether the format FMT is available,\n"
>   "                         exits with 0 if it is, else with 1\n".
> Then a blank line, print_common_unary_program_parameter_instructions(
> message_out), a blank line, print_report_bugs(), a blank line, and finally
> print_more_info().
