# tools/src/hfst-lexc-compiler.cc

> [spec:hfst:def:hfst-lexc-compiler.lexc-streams-fn]
> int

> [spec:hfst:sem:hfst-lexc-compiler.lexc-streams-fn]
> Drives the actual lexc compilation given a configured LexcCompiler 'lexc' and
> an opened HfstOutputStream 'outstream'. For each of the 'lexccount' collected
> input files (index 'i' from 0): emit the verbose line "Parsing lexc file
> <name>\n"; if the file handle equals stdin, read the whole of standard input
> into a string and feed it to the compiler's incremental string parser
> (mirroring the C++ 'lexc.parse(stdin)'), otherwise read the named file's
> contents into a string and feed that (mirroring 'lexc.parse(filename)'). After
> all files are parsed, emit "Compiling... " and call 'compile_lexical()' to
> assemble the single result transducer (a raw owning pointer, null on failure).
> If the result is null, report a fatal error via 'error(EXIT_FAILURE, 0, ...)':
> when exactly one file was given the message names "The file <name[0]> did not
> compile cleanly." otherwise "The files <name[0]>... did not compile cleanly.";
> both append "(if there are no error messages above, try -v or -d to get more
> info)"; then return EXIT_FAILURE. On success: take ownership of the result, set
> its name with operation "lexc" against the first filename and its formula with
> operation "L" against the first filename, emit "\nWriting... ", write the
> transducer to 'outstream' (operator<< / redirect), emit "done\n", drop the
> transducer (C++ 'delete res'), and close 'outstream'. Finally, if the
> '--encode-weights' flag was set, restore the previously saved encode-weights
> setting 'enc' via 'set_encode_weights(enc)'. Return EXIT_SUCCESS.

> [spec:hfst:def:hfst-lexc-compiler.main-fn]
> int

> [spec:hfst:sem:hfst-lexc-compiler.main-fn]
> Program entry point. Set the program name to argv[0] with version "0.1" and
> wiki name "HfstLexc". Call 'parse_options'; if it returns anything other than
> EXIT_CONTINUE, return that value. Close the buffered FILE handles that the
> tool only needs as streams: for every collected lexc file that is not stdin
> (and not null), 'fclose' it; if the output file is not stdout (and not null),
> 'fclose' it. Save the library's current encode-weights setting into 'enc'
> (get_encode_weights); if the '--encode-weights' flag is set, turn encode
> weights on globally (set_encode_weights(true)). Emit the verbose progress:
> "Reading from " followed by "<name>, " for each input file, then "writing to
> <outfilename>\n". Open the output stream: if the output is a named file use
> 'HfstOutputStream(outfilename, format)', else the stdout 'HfstOutputStream(
> format)' (hfst-format wrappers enabled). Apply the global Xerox-composition
> setting (set_xerox_composition(xerox_composition)). Construct the LexcCompiler
> with 'format', 'with_flags', and 'align_strings'; then 'setMinimizeFlags(
> minimize_flags)' and 'setRenameFlags(rename_flags)'. Set verbosity: 0 when
> silent, else 2 when verbose, else 1. If warnings are to be treated as errors,
> 'setTreatWarningsAsErrors(true)'. Register every warning toggle by name
> ('-Wone-sided-flags', '-Wunused-lexicons', '-Wrepeated-lexicons',
> '-Wmissing-lexicons', '-Wmissing-alphabets', '-Wunnecessary-escapes') with its
> collected boolean. When not silent and verbose, print a "Warning settings: "
> line listing each enabled warning (and " -Werror (fail on all warnings)" when
> errors-as-warnings is on). If split-characters was requested, print the
> "Warningn: Disabling unicode character tokenisation" notice to stderr and call
> 'setSplitCharacters(true)'. Run 'lexc_streams(lexc, outstream)' and return its
> result (the C++ also frees the filename buffers here; the Rust owners drop
> automatically).

> [spec:hfst:def:hfst-lexc-compiler.parse-options-fn]
> int

> [spec:hfst:sem:hfst-lexc-compiler.parse-options-fn]
> Parse the command line. First call 'extend_options_getenv' to splice in any
> options from the environment. Then loop over 'getopt_long' with the common
> long options plus this tool's own long options: 'encode-weights'(E),
> 'format'(f, arg), 'output'(o, arg), 'alignStrings'(A), 'withFlags'(F),
> 'minimizeFlags'(M), 'renameFlags'(R), 'xerox-composition'(x, arg),
> 'xfst'(X, arg), 'Werror'(Q), 'Wstuff'(W, arg), 'split-characters'(9); and the
> short-option string HFST_GETOPT_COMMON_SHORT followed by "Ef:o:AFMRx:X:QW:9".
> The common getopt cases (including '-o'/'--output', help, version, verbosity)
> are handled first. Tool cases: 'A' sets align_strings; 'E' sets encode_weights;
> 'f' sets format from 'hfst_parse_format_name(optarg)'; 'F' sets with_flags;
> 'M' sets minimize_flags; 'R' sets rename_flags. For 'x' (xerox-composition):
> "yes"/"true"/"ON" set xerox_composition true, "no"/"false"/"OFF" set it false,
> anything else prints 'Error: unknown option to --xerox-composition: '<arg>''
> to stderr and returns EXIT_FAILURE. For 'X' (xfst): "flag-is-epsilon" calls
> 'set_flag_is_epsilon_in_composition(true)', anything else prints 'Error:
> unknown option to --xfst: '<arg>'' and returns EXIT_FAILURE. 'Q' (deprecated
> --Werror) turns on treat_warnings_as_errors plus the one-sided-flags,
> missing/unused/repeated-lexicons warnings, forces unnecessary-escapes and
> missing-alphabets off, and prints a deprecation notice. 'W' dispatches on the
> argument: "error" sets treat-as-errors; "all" enables one-sided-flags,
> everything, missing/unused/repeated-lexicons, missing-alphabets and
> unnecessary-escapes; "one-sided-flags"/"no-one-sided-flags",
> "unused-lexicons"/"no-unused-lexicons", "repeated-lexicons"/
> "no-repeated-lexicons", "missing-lexicons"/"no-missing-lexicons",
> "missing-alphabets"/"no-missing-alphabets", "unnecessary-escapes"/
> "no-unnecessary-escapes" each set/clear the matching flag; an unknown argument
> prints "Unknown warning option <arg>" and returns EXIT_FAILURE. '9' sets
> split_characters. The terminal error arm handles unrecognised options. After
> the loop run the common parameter checks. If 'format' is still
> UNSPECIFIED_TYPE, warn (unless silent) "Defaulting to OpenFst tropical type"
> and set format to TROPICAL_OPENFST_TYPE. If positional arguments remain
> (argc - optind > 0), collect each remaining argv entry as a lexc filename and
> open it for reading, incrementing lexccount, and mark input as not-stdin.
> Otherwise create a single entry named "<stdin>" bound to stdin, mark input as
> stdin, and set lexccount to 1. Return EXIT_CONTINUE.
