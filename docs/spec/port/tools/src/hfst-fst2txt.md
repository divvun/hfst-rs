# tools/src/hfst-fst2txt.cc

> [spec:hfst:def:hfst-fst2txt.fst-text-format]
> enum fst_text_format {
>   ATT_TEXT;
>   DOT_TEXT;
>   PCKIMMO_TEXT;
>   PROLOG_TEXT;
> }

The text output format selector. ATT_TEXT is the AT&T / OpenFst compatible
tab-separated value format (the default). DOT_TEXT is the Graphviz / dotty
format. PCKIMMO_TEXT is the PCKIMMO format. PROLOG_TEXT is the prolog format.
A module-level variable holds the selected format, defaulting to ATT_TEXT.

> [spec:hfst:def:hfst-fst2txt.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-fst2txt.main-fn]
> 1. Set the program name to 'HfstFst2Txt', version '0.3'.
> 2. Call parse_options(argc, argv); if it returns anything other than
>    EXIT_CONTINUE, return that value immediately.
> 3. If the input file is not standard input (an explicit input file was
>    opened), close that buffered input FILE handle (the tool uses streams).
> 4. Emit a verbose message 'Reading from <inputfilename>, writing to
>    <outfilename>'.
> 5. Construct an HfstInputStream: from the named input file if one was opened,
>    otherwise from standard input. (In C this is wrapped in a try/catch that on
>    HfstException reports '<inputfilename> is not a valid transducer file' and
>    returns EXIT_FAILURE; the Rust constructor panics instead, so that arm is
>    not reproduced.)
> 6. If the stream's type is XFSM_TYPE, enforce the xfsm restrictions, each
>    raising error(EXIT_FAILURE, 0, ...) and returning EXIT_FAILURE:
>    - format 'dot' is not supported ("use 'prolog'");
>    - format 'pckimmo' is not supported ("use 'prolog'");
>    - format 'att' is not supported ("use 'prolog'");
>    - option '--use-numbers' is not supported;
>    - reading from standard input (inputfilename == "<stdin>") is not supported;
>    - writing to standard output (outfilename == "<stdout>") is not supported.
> 7. Call process_stream with the input stream and the global output FILE handle,
>    and return its result.
> 8. (C frees inputfilename and outfilename here; the foundation owns these
>    allocations in the port.)

> [spec:hfst:def:hfst-fst2txt.parse-options-fn]
> int

> [spec:hfst:sem:hfst-fst2txt.parse-options-fn]
> 1. Call extend_options_getenv to splice any options from the environment into
>    argc/argv.
> 2. Loop calling getopt_long with the common long options, the unary long
>    options, and the tool-specific long options:
>    --print-weights ('w', no argument), --do-not-print-weights ('D', no
>    argument), --use-numbers ('n', no argument), --format ('f', required
>    argument); short option string is the common + unary short strings followed
>    by "wDnf:". Break out of the loop when getopt_long returns -1.
> 3. Dispatch each returned option code through the common-case group (its usage
>    closure prints this tool's usage), then the unary-case group, then the
>    tool-specific cases, then the terminal error case. Each group either returns
>    a code, continues the loop, or falls through to the next group.
> 4. Tool-specific cases:
>    - 'w': set print_weights = true.
>    - 'D': set do_not_print_weights = true.
>    - 'n': set use_numbers = true.
>    - 'f': parse optarg into the format variable: "att"/"AT&T"/"openfst"/
>      "OpenFst" -> ATT_TEXT; "dot"/"graphviz"/"GraphViz" -> DOT_TEXT;
>      "pckimmo" -> PCKIMMO_TEXT; "prolog"/"Prolog" -> PROLOG_TEXT; anything else
>      raises error(EXIT_FAILURE, 0, "Cannot parse <optarg> as text format; Use
>      one of att, pckimmo, dot, prolog").
> 5. After the loop, run the common parameter checks and the unary parameter
>    checks, then return EXIT_CONTINUE.

> [spec:hfst:def:hfst-fst2txt.process-stream-fn]
> int

> [spec:hfst:sem:hfst-fst2txt.process-stream-fn]
> Read every transducer from the input stream and write its text representation
> to the output FILE handle outf. Maintain a 1-based counter transducer_n.
> While the stream is good:
> 1. Increment transducer_n and read the next transducer. (In C reading is
>    wrapped in try/catch on TransducerTypeMismatchException, reporting "input
>    transducers do not have the same type"; the Rust constructor panics instead.)
> 2. Determine its display name: the transducer's name, or the input filename if
>    the name is empty.
> 3. If transducer_n == 1, emit a verbose 'Converting <name>...'. Otherwise:
>    if the stream type is XFSM_TYPE, raise error(EXIT_FAILURE, 0, "Writing more
>    than one transducer in text format to file not supported for xfsm
>    transducers, ...") and return EXIT_FAILURE; else emit a verbose
>    'Converting <name>...<transducer_n>'.
> 4. If transducer_n > 1, write a "--\n" separator line to outf.
> 5. Decide whether to print weights (printw): if print_weights, true; else if
>    do_not_print_weights, false; else if the type is SFST/FOMA/XFSM, false; else
>    if the type is TROPICAL_OPENFST/LOG_OPENFST, true; else (should not happen)
>    true.
> 6. Emit the transducer according to the selected format:
>    - ATT_TEXT: if use_numbers, write in AT&T number format to outf with printw;
>      otherwise write in AT&T format to outf with printw.
>    - DOT_TEXT: write the comment line "// This graph generated with
>      hfst-fst2txt\n" to outf, then print the transducer in dot format to outf.
>    - PCKIMMO_TEXT: print the transducer in pckimmo format to outf.
>    - PROLOG_TEXT: if the type is XFSM_TYPE, write the xfsm transducer in prolog
>      format to outfilename (no name or weights). Otherwise compute
>      alt_namestr = "NO_NAME_<transducer_n>"; if the transducer has no name and
>      not silent, report 'Transducer has no name, giving it a name
>      '<alt_namestr>'...' to stderr, else if not silent report 'Renaming
>      transducer into '<alt_namestr>'...' to stderr; in both branches use
>      alt_namestr as the name, then write in prolog format to outf with printw.
>      (In C this is wrapped in a try/catch on HfstException reporting "Error
>      encountered when writing in prolog format: <name>"; the Rust impl panics.)
> 7. (C deletes the transducer; the Rust value is dropped at the end of the loop
>    iteration.)
> After the loop, close the input stream; if outf is not standard output, close
> it; return EXIT_SUCCESS.
