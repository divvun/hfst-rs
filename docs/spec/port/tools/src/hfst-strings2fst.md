# tools/src/hfst-strings2fst.cc

> [spec:hfst:def:hfst-strings2fst.divide-by-sum-of-weights-fn]
> float

> [spec:hfst:sem:hfst-strings2fst.divide-by-sum-of-weights-fn]
> Weight-transform callback used to normalise path weights. Reads the
> tool-global accumulator 'sum_of_weights'. If that accumulator is exactly 0,
> returns 0 (avoids division by zero). Otherwise returns the argument 'weight'
> divided by 'sum_of_weights'. Pure aside from reading the global; intended to
> be passed to HfstTransducer::transform_weights when option --norm is set.

> [spec:hfst:def:hfst-strings2fst.main-fn]
> int

> [spec:hfst:sem:hfst-strings2fst.main-fn]
> Program entry point. Sets the program name/version/wikiname via
> hfst_set_program_name(argv[0], "0.1", "Strings2Fst"). Calls parse_options;
> if it returns anything other than EXIT_CONTINUE, returns that value
> immediately. If a multichar-symbol filename was given (-m), verbose-logs it,
> opens the file, and errors out (EXIT_FAILURE) if it cannot be read; then
> reads it line by line (lines up to 1000 chars) and, for every non-empty line,
> verbose-logs "Defining multichar symbol ..." and appends the line to the
> global 'multichar_symbols' vector. Closes the output buffer 'outfile' unless
> it is stdout (the tool uses HfstOutputStream instead). Verbose-logs the
> input/output filenames. Constructs an HfstOutputStream: from the output
> filename + output_format when outfile is not stdout, otherwise from
> output_format alone (stdout). Calls process_stream on it. Frees
> 'inputfilename' and 'outfilename'. Returns EXIT_SUCCESS.

> [spec:hfst:def:hfst-strings2fst.parse-options-fn]
> int

> [spec:hfst:sem:hfst-strings2fst.parse-options-fn]
> Parses command-line options into the tool globals. First calls
> extend_options_getenv to splice in options from the environment. Loops over
> getopt_long with the long-option table = common long options + unary long
> options + the tool's own: --disjunct-strings(j), --epsilon=(e),
> --norm('2'), --log('3'), --log10('4'), --pairstrings(p), --has-spaces(S),
> --multichar-symbols=(m), --format=(f), --Wstuff=(W); and the short-option
> string = common short + unary short + "je:234pSm:f:W:". Dispatches each
> returned option code first through the common getopt cases, then the unary
> cases, then the tool-specific cases:
>  - 'e': epsilonname = strdup(optarg);
>  - '2': normalize_weights = true;
>  - '3': logarithmic_weights_e = true;
>  - '4': logarithmic_weights_10 = true;
>  - 'j': disjunct_strings = true;
>  - 'S': has_spaces = true;
>  - 'p': pairstrings = true;
>  - 'm': multichar_symbol_filename = strdup(optarg);
>  - 'f': output_format = hfst_parse_format_name(optarg);
>  - 'W': optarg "error" sets warnings_are_errors=true; "no-error" sets it
>    false; "negative-weights" sets warn_negative_weights=true;
>    "no-negative-weights" sets it false; anything else is a fatal error
>    ("unrecognised warning option -W<optarg>") returning EXIT_FAILURE;
> any unrecognised code falls through to the error case. After the loop runs
> the common and unary parameter checks. If output_format is still
> UNSPECIFIED_TYPE, verbose-logs that it defaults to openfst tropical and sets
> output_format = TROPICAL_OPENFST_TYPE. If epsilonname is still null, sets it
> to strdup("@0@"). Returns EXIT_CONTINUE.

> [spec:hfst:def:hfst-strings2fst.process-stream-fn]
> int

> [spec:hfst:sem:hfst-strings2fst.process-stream-fn]
> Reads the input line by line from 'inputfile' and compiles each into
> transducer(s) written to 'outstream'. Builds an HfstStrings2FstTokenizer from
> the global multichar_symbols and the epsilon name. Keeps an empty
> HfstBasicTransducer 'disjunction' for the --disjunct-strings accumulation and
> a line counter. For each line read by hfst_getline:
>  - increments transducer_n and line_n and verbose-logs "Parsing line N...".
>  - Splits off an optional weight: finds the first tab. If there is no tab, the
>    string end is the first of '\0','\n','\r' and the line is unweighted. If
>    there is a tab, every '\n'/'\r' from the tab onward is overwritten with
>    '\0', the substring after the tab is parsed with hfst_strtoweight, the line
>    is marked weighted, and—if the weight is negative and warn_negative_weights
>    is on—either errors out at that line (if warnings_are_errors) or warns at
>    that line, with the "Found negative weight ..." message; the string end is
>    set to the tab position. The string end byte is overwritten with '\0'.
>  - Tokenizes the (now NUL-terminated) string into a StringPairVector: if
>    pairstrings, via tokenize_pair_string(line, has_spaces); else via
>    tokenize_string_pair(line, has_spaces). Catches UnescapedColsFound (fatal
>    error at line: unescaped ':' message, with the pairstring variant of the
>    message when -p is set) and an invalid-UTF-8 exception (fatal error at
>    line: "Input string ... is not valid utf-8.").
>  - Computes path_weight: if weighted, adds the weight to sum_of_weights, uses
>    it as path_weight, and verbose-logs "Using final weight ...".
>  - If NOT disjunct_strings: makes a fresh HfstBasicTransducer; if
>    logarithmic_weights_e/_10, replaces path_weight with the negative natural /
>    base-10 logarithm of the weight; disjuncts the single path
>    (disjunct(spv, path_weight)); converts to an HfstTransducer in
>    output_format; sets its name via hfst_set_name(res, "", "string"); and
>    writes it to outstream.
>  - If disjunct_strings: disjuncts the path into the shared 'disjunction'
>    accumulator with the raw path_weight (logarithm deferred).
> After the loop, frees the line buffer. If disjunct_strings: converts
> 'disjunction' to an HfstTransducer in output_format; if normalize_weights,
> verbose-logs and applies transform_weights(divide_by_sum_of_weights); if
> logarithmic_weights_e/_10, verbose-logs and applies the matching negative-
> logarithm transform; sets its name via hfst_set_name(res, "?", "strings");
> and writes it to outstream. Returns EXIT_SUCCESS.

> [spec:hfst:def:hfst-strings2fst.take-negative-logarithm-10-fn]
> float

> [spec:hfst:sem:hfst-strings2fst.take-negative-logarithm-10-fn]
> Weight-transform callback. If the argument 'weight' is exactly 0, returns 0
> (the comment notes this should be INFINITY but that does not work in
> transitions). Otherwise clears errno, computes -log10(weight), and if errno
> became non-zero errors out (EXIT_FAILURE, "unable to take negative
> logarithm"); returns the result. Used for the --log10 option.

> [spec:hfst:def:hfst-strings2fst.take-negative-logarithm-e-fn]
> float

> [spec:hfst:sem:hfst-strings2fst.take-negative-logarithm-e-fn]
> Weight-transform callback. If the argument 'weight' is exactly 0, returns 0
> (the comment notes this should be INFINITY but that does not work in
> transitions). Otherwise clears errno, computes -log(weight) (natural log),
> and if errno became non-zero errors out (EXIT_FAILURE, "unable to take
> negative logarithm"); returns the result. Used for the --log option.
