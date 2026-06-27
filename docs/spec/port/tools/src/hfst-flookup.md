# tools/src/hfst-flookup.cc

> [spec:hfst:def:hfst-flookup.basic-fn]
> HfstBasicTransducer basic(trans)

> [spec:hfst:sem:hfst-flookup.basic-fn]
> Construct a mutable HfstBasicTransducer equivalent to the (already inverted, if
> applicable) HfstTransducer 'trans'. This is the slow-lookup representation:
> the subsequent code iterates its states and transitions to collect the set of
> seen input symbols (and multicharacter symbols), and the transducer itself is
> pushed onto the 'cascade_mut' vector for use by the basic-transducer lookup
> path. Only built for non-optimized-lookup backends (SFST, tropical/log OpenFST,
> foma).

> [spec:hfst:def:hfst-flookup.escape-special-characters-fn]
> static std::string

> [spec:hfst:sem:hfst-flookup.escape-special-characters-fn]
> Return a copy of the input string in which every ':', '\' and ' ' character is
> prefixed with a single backslash '\'. All other characters are copied through
> unchanged. Used to protect the lookup-string tokenizer from interpreting those
> three characters as pair separators / escapes / token boundaries.

> [spec:hfst:def:hfst-flookup.get-print-format-fn]
> static std::string

> [spec:hfst:sem:hfst-flookup.get-print-format-fn]
> Convert one symbol to its printable form. If the symbol is the internal epsilon
> symbol, return the configured epsilon_format string. Otherwise, if quote_special
> is set, return the symbol with backslash escaping applied in this exact order:
> first replace every '\' with '\\', then every ':' with '\:', then every ' '
> with '\ '. If quote_special is not set, return the symbol unchanged.

> [spec:hfst:def:hfst-flookup.is-possible-to-get-result-fn]
> bool

> [spec:hfst:sem:hfst-flookup.is-possible-to-get-result-fn]
> Quick filter deciding whether a lookup path could possibly match a transducer.
> If the transducer was seen to contain an unknown or identity symbol
> (unknown_or_identity_seen is true), always return true. Otherwise return true
> only if every symbol of the path's output side (s.second) is present in the
> precomputed set symbols_seen; if any symbol is absent, return false (the lookup
> can be skipped because it cannot succeed).

> [spec:hfst:def:hfst-flookup.is-valid-flag-diacritic-path-fn]
> bool

> [spec:hfst:sem:hfst-flookup.is-valid-flag-diacritic-path-fn]
> Construct a fresh FlagDiacriticTable and ask whether the given vector of arc
> symbols is a valid flag-diacritic string (i.e. all flag operations along the
> path are consistent). If it is not valid and verbose mode is on, emit a
> "blocked by flags: " message followed by each arc symbol separated by a space.
> Return the validity boolean.

> [spec:hfst:def:hfst-flookup.line-to-lookup-path-fn]
> HfstOneLevelPath *

> [spec:hfst:sem:hfst-flookup.line-to-lookup-path-fn]
> Turn one input line into an HfstOneLevelPath (weight 0, symbol vector). Set
> *outside_sigma to false and increment the global inputs counter. Then branch on
> input_format:
> - SPACE_SEPARATED_TOKEN_INPUT: escape the line with escape_special_characters,
>   tokenize it as a string pair with spaces=true, and push each pair's input
>   (first) member onto the path.
> - UTF8_TOKEN_INPUT: if optimized_lookup, push the whole line as a single symbol;
>   otherwise escape and tokenize with spaces=false and push each pair's input
>   member.
> - APERTIUM_INPUT: walk the line splitting bracketed superblank markup ('[' ... ']')
>   into the *markup out-string (handling '\]' escapes inside brackets) from the
>   real surface text; outside brackets, a backslash escapes the next character.
>   Then split the collected real text into UTF-8 characters (string_to_utf8),
>   replace *s with the real text, set *markup, and store the characters as the
>   path.
> Return the path.

> [spec:hfst:def:hfst-flookup.lookup-cascading-fn]
> HfstOneLevelPaths *

> [spec:hfst:sem:hfst-flookup.lookup-cascading-fn]
> Look a single input path up against every transducer of a multi-transducer
> cascade and union the results. For each transducer index i in the cascade: set
> the global transducer_number to i (so lookup_simple can index the per-cascade
> symbol sets), call lookup_simple on transducer i, emit a verbose progress note,
> and insert every resulting one-level path into the accumulated result set.
> Return the union of all transducers' results.

> [spec:hfst:def:hfst-flookup.lookup-fd-and-print-fn]
> void

> [spec:hfst:sem:hfst-flookup.lookup-fd-and-print-fn]
> Perform a slow lookup of path 's' against basic transducer 't' (skipping the
> actual lookup if is_possible_to_get_result says it cannot match, leaving the
> two-level result set empty) using infinite_cutoff epsilon cycles, producing
> two-level (input:output) paths.
> If print_pairs is set, print immediately to outfile: when there are no results
> print just the lookup string then a newline; otherwise, for each result within
> the beam of the lowest weight, print the lookup string, a tab, the
> input:output pairs (space-separated if print_space), a tab, and the weight,
> each result on its own line, ending the block with a blank line; flush outfile.
> Then convert each two-level path into a one-level path keeping only the output
> side and insert into 'results'. Finally filter 'results': keep a path only if
> it is a valid flag-diacritic path (or obey_flags is off), and within each kept
> path drop flag-diacritic symbols unless show_flags is set. Replace 'results'
> with the filtered set.

> [spec:hfst:def:hfst-flookup.lookup-input-format]
> enum lookup_input_format {
>   UTF8_TOKEN_INPUT;
>   SPACE_SEPARATED_TOKEN_INPUT;
>   APERTIUM_INPUT;
> }

> [spec:hfst:def:hfst-flookup.lookup-output-format]
> enum lookup_output_format {
>   XEROX_OUTPUT;
>   CG_OUTPUT;
>   APERTIUM_OUTPUT;
> }

> [spec:hfst:def:hfst-flookup.lookup-printf-fn]
> int

> [spec:hfst:sem:hfst-flookup.lookup-printf-fn]
> Render one format-string line for a single lookup event and print it to ofile.
> First build the printable lookupform (from result->second, the analysis side)
> and inputform (from input->second): for each symbol, insert space_format between
> symbols when print_space is set, render epsilon as epsilon_format, drop flag
> diacritics unless show_flags, and otherwise emit the symbol verbatim. The weight
> w is result->first, or +infinity if result is NULL.
> Then expand the format string: '%i' -> inputform, '%l' -> full lookupform,
> '%b' -> the lookupform up to the first of '+',' ','<','[' (the "base"), '%a' ->
> the lookupform from that split point on (the "analysis"), '%m' -> markup (or
> empty), '%n' -> a newline, '%w' -> the weight printed with %f (on non-MSC builds
> "inf" is never used; %f is always used). An unrecognized '%X' is emitted as a
> literal '%' followed by X. Non-'%' characters are copied through.
> Finally print the expanded result to ofile: directly if quote_special is off, or
> via get_print_format if quote_special is on. Return the number of characters
> printed.

> [spec:hfst:def:hfst-flookup.lookup-simple-fn]
> HfstOneLevelPaths *

> [spec:hfst:sem:hfst-flookup.lookup-simple-fn]
> Look path 's' up against a single basic transducer 't'. First test, via
> is_possible_to_get_result over the current transducer's seen-symbol set, whether
> the path could match. If it could and the lookup is infinitely ambiguous, warn
> (unless silent and only when infinite_cutoff > 0) that results are limited to
> infinite_cutoff cycles, then call lookup_fd_and_print with the cutoff and set
> *infinity to true; otherwise call lookup_fd_and_print with no cutoff. If the
> result set is empty, emit a verbose "Got no results" note. Return the results.
> (The optimized-lookup overload instead checks time_cutoff and uses the
> transducer's own lookup_fd / is_lookup_infinitely_ambiguous.)

> [spec:hfst:def:hfst-flookup.main-fn]
> int

> [spec:hfst:sem:hfst-flookup.main-fn]
> Program entry point. Set the locale, set the program name/version/wikiname to
> ("HfstFlookup","0.6"). Parse options; if parse_options returns anything other
> than EXIT_CONTINUE, return that code. Close the (now-unneeded) input FILE buffer
> unless it is stdin. Emit verbose notes about the input/output file names and the
> resolved set of output format strings (regular/unanalysed/untokenised/infinite
> templates, the epsilon and space formats, and the show_flags flag). Open an
> HfstInputStream from the input file name (or stdin); on a bad transducer file
> the C++ caught the exception and reported "is not a valid transducer file"
> (the Rust port currently relies on the constructor and does not reproduce that
> catch arm). Run process_stream over the stream and outfile, close outfile unless
> it is stdout, free the file-name buffers, and return EXIT_SUCCESS.

> [spec:hfst:def:hfst-flookup.parse-options-fn]
> int

> [spec:hfst:sem:hfst-flookup.parse-options-fn]
> Parse the command line. After extend_options_getenv, loop over getopt_long with
> the common + unary option tables plus the tool-specific options
> ("I:O:F:xc:X:e:E:b:t:p::PRf"), dispatching the common and unary cases first and
> then the tool's own:
> -R/--invert sets invert. -I/--input-strings opens the named lookup strings file
> (sets lookup_given). -O/--output-format selects xerox|cg|apertium output
> (apertium also forces apertium input), erroring on unknown values. -F/--input-format
> selects text|spaced|apertium input, erroring on unknown values. -e/-E sets the
> epsilon_format. -b/--beam parses a non-negative beam float (error if negative).
> -t/--time-cutoff parses a non-negative time cutoff. -x sets print_statistics.
> -X/--xfst toggles one of print-pairs / print-space (also sets space_format to a
> single space) / show-flags / quote-special / obey-flags (clears obey_flags),
> erroring on unknown values; note this case falls through into -c and reparses
> optarg as the cycle count. -c/--cycles sets infinite_cutoff. -p/--pipe-mode
> sets pipe_input and/or pipe_output (both by default / from input|output|both),
> erroring on unknown values. -P sets the progress bar. -f sets force_ol.
> After the loop, select the begin/lookup/end format-string triples (regular,
> empty, unknown, infinite) for the chosen output format. If no lookup file was
> given, default it to stdin named "<stdin>". Run the common and unary parameter
> checks and return EXIT_CONTINUE.

> [spec:hfst:def:hfst-flookup.perform-lookups-fn]
> HfstOneLevelPaths *

> [spec:hfst:sem:hfst-flookup.perform-lookups-fn]
> Dispatch a single lookup over a cascade. If the input was flagged 'unknown'
> (outside the alphabet), return an empty result set. Otherwise, if the cascade
> holds exactly one transducer call lookup_simple on it, and if it holds more
> than one call lookup_cascading; return the result set. (Two overloads exist —
> one over a vector of HfstTransducer for the optimized-lookup path and one over a
> vector of HfstBasicTransducer for the slow path.)

> [spec:hfst:def:hfst-flookup.print-lookup-string-fn]
> static void

> [spec:hfst:sem:hfst-flookup.print-lookup-string-fn]
> Print every symbol of the given symbol vector to stderr, each rendered through
> get_print_format (so epsilon and special-character quoting are applied). No
> separators or trailing newline are added.

> [spec:hfst:def:hfst-flookup.print-lookups-fn]
> void

> [spec:hfst:sem:hfst-flookup.print-lookups-fn]
> Print the full set of results 'kvs' for one input 'kv' to ofile using the
> currently selected format templates. Choose the template triple by case:
> if outside_sigma (token outside the alphabet), use the unknown_* templates and
> increment no_analyses; else if kvs is empty, use the empty_* templates and
> increment no_analyses; else if inf (infinitely ambiguous), increment analysed,
> print infinite_begin_setf, then for each result within the beam of the lowest
> weight print infinite_lookupf (incrementing analyses), then infinite_end_setf;
> else (normal), increment analysed, print begin_setf, then each in-beam result
> via lookupf (incrementing analyses), then end_setf. The lowest weight is taken
> from the first result; beam < 0 means no beam filtering.

> [spec:hfst:def:hfst-flookup.print-prompt-fn]
> static void

> [spec:hfst:sem:hfst-flookup.print-prompt-fn]
> If not silent, not in pipe-input mode, and no lookup-strings file was given
> (i.e. input is being read interactively), print the prompt "> " to stderr.
> Otherwise do nothing.

> [spec:hfst:def:hfst-flookup.print-usage-fn]
> void

> [spec:hfst:sem:hfst-flookup.print-usage-fn]
> Print the --help text to message_out: a usage line with the program name,
> a one-paragraph description (lookup is done right to left like flookup/xfst),
> the common program options, the Input/Output options (-i/-o/-p), the Lookup
> options (-R,-I,-O,-e,-F,-x,-X,-c,-b,-t,-P,-f), the common unary parameter
> instructions, notes on OFORMAT/IFORMAT/VARIABLE values and defaults, the
> --pipe-mode STREAM explanation, the list of known bugs, the report-bugs line and
> the more-info line.

> [spec:hfst:def:hfst-flookup.replace-all-fn]
> static std::string

> [spec:hfst:sem:hfst-flookup.replace-all-fn]
> Return a copy of 'symbol' with every (non-overlapping) occurrence of the
> substring str1 replaced by str2. Scanning resumes after each inserted str2, so
> str2 is not re-scanned. If str1 is empty, return the input unchanged.

> [spec:hfst:def:hfst-flookup.string-to-utf8-fn]
> vector<string> *

> [spec:hfst:sem:hfst-flookup.string-to-utf8-fn]
> Split a raw byte string into a vector of one-character UTF-8 strings. Determine
> each character's byte length from the leading byte (1 for ASCII <=127, 2/3/4 for
> the multibyte lead-byte bit patterns), error out via hfst_error_at_line on an
> invalid lead byte, copy that many bytes as one symbol, and advance. Continue to
> the terminating NUL. Return the vector of single-character symbols.

> [spec:hfst:def:hfst-flookup.trans-fn]
> HfstTransducer trans(inputstream)

> [spec:hfst:sem:hfst-flookup.trans-fn]
> Read the next transducer from the input stream into 'trans'. Determine its
> implementation type. If it is an optimized-lookup type and we are not inverting
> and --force-ol was not given, error out (lookup is unsupported on optimized
> lookup transducers). Resolve its display name (the transducer's stored name, or
> the input file name if empty) and emit a verbose "Reading ..." note. Unless
> --invert was given, invert it (for optimized-lookup types this is done by
> converting to tropical OpenFST, inverting, and converting back). Then, for
> non-optimized backends, build a basic transducer, collect its seen / multichar
> symbols, and record the per-cascade symbol set and unknown-or-identity flag.
> Finally push the transducer onto the cascade.
