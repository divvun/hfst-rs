# tools/src/hfst-lookup.cc

> [spec:hfst:def:hfst-lookup.basic-fn]
> HfstBasicTransducer basic(trans)

> [spec:hfst:sem:hfst-lookup.basic-fn]
> Construct an HfstBasicTransducer copy of the just-read transducer 'trans'
> (only done for SFST/TROPICAL_OPENFST/LOG_OPENFST/FOMA types). Iterate every
> state and every transition out of it; for each transition take its input
> symbol and (a) insert it into this transducer's 'symbols_seen' set, (b) if it
> equals the internal unknown or internal identity symbol, set the
> 'id_or_unk_seen' flag, and (c) if it is longer than one character, append it
> to the shared 'mc_symbols' multicharacter-symbol list and emit a verbose
> "multicharacter symbol: <sym>" note. Then push 'basic' onto the mutable
> cascade, push 'symbols_seen' onto cascade_symbols_seen, and push
> 'id_or_unk_seen' onto cascade_unknown_or_identity_seen.

> [spec:hfst:def:hfst-lookup.escape-special-characters-fn]
> static std::string

> [spec:hfst:sem:hfst-lookup.escape-special-characters-fn]
> Return a copy of the input string in which every ':' , '\\' and ' '
> character is preceded by an inserted backslash; all other characters are
> copied unchanged. Used to protect tokenizer-significant characters before
> handing a line to the strings-to-fst tokenizer.

> [spec:hfst:def:hfst-lookup.get-lookup-string-fn]
> static std::string

> [spec:hfst:sem:hfst-lookup.get-lookup-string-fn]
> Concatenate, in order, the print-format rendering (get_print_format) of each
> symbol in the given string vector and return the resulting single string.

> [spec:hfst:def:hfst-lookup.get-print-format-fn]
> static std::string

> [spec:hfst:sem:hfst-lookup.get-print-format-fn]
> Render a single symbol for printing. If the symbol is epsilon, return the
> configured epsilon_format string. Otherwise, if quote_special is set, return
> the symbol with every backslash, colon and space backslash-escaped (replace
> '\\' with '\\\\', then ':' with '\\:', then ' ' with '\\ ', applied in that
> order). Otherwise return the symbol unchanged.

> [spec:hfst:def:hfst-lookup.is-possible-to-get-result-fn]
> bool

> [spec:hfst:sem:hfst-lookup.is-possible-to-get-result-fn]
> Quick pre-filter deciding whether a lookup path could possibly match the
> current (non-optimized) transducer. If that transducer was seen to contain an
> unknown or identity symbol, return true unconditionally. Otherwise return
> true only if every symbol of the path's input side is a member of the
> transducer's seen-symbols set; if any symbol is absent, return false.

> [spec:hfst:def:hfst-lookup.line-to-lookup-path-fn]
> HfstOneLevelPath *

> [spec:hfst:sem:hfst-lookup.line-to-lookup-path-fn]
> Turn one input line into a lookup path (weight 0). Increment the global
> 'inputs' counter and clear '*outside_sigma'. Dispatch on input_format:
> - SPACE_SEPARATED_TOKEN_INPUT: escape special characters, tokenize the line
>   as string pairs with spaces honoured, and take the input (first) member of
>   each pair as the path symbols.
> - UTF8_TOKEN_INPUT: if the transducer is optimized-lookup, push the whole
>   line as a single symbol; otherwise escape special characters, tokenize as
>   string pairs without spaces, and take each pair's input member.
> - APERTIUM_INPUT: split the line into a "real" string and a markup string.
>   Text inside '[' ... ']' (with '\\]' meaning a literal ']') goes to markup
>   along with the brackets; a backslash outside brackets escapes the next
>   character into the real string; everything else goes to the real string.
>   The real string is then byte-split into UTF-8 symbols (string_to_utf8), the
>   caller's line pointer is replaced by the real string and '*markup' is set.
> Returns the constructed path.

> [spec:hfst:def:hfst-lookup.lookup-cascading-fn]
> HfstOneLevelPaths *

> [spec:hfst:sem:hfst-lookup.lookup-cascading-fn]
> Look a path up through a multi-transducer cascade and return the union of
> results. Iterate the cascade by index i (for the basic-transducer variant,
> first set transducer_number = i). For each transducer compute 'result':
> - If the cascade mode is composition and i != 0, feed the previous
>   accumulated results as inputs: for every previous result run lookup_simple
>   on transducer i (requesting pair printing only on the final transducer,
>   passing the original input as the string to print and suppressing the
>   trailing newline), and for each sub-result insert a path whose weight is the
>   sub-result weight plus the feeding path's weight; then zero the accumulator.
>   When this is the last transducer and print-pairs is on, print either the
>   "<input>\t<input>+?\tinf" failure line (no results) or a single newline.
> - Otherwise run lookup_simple on transducer i directly (the basic variant
>   requests pair printing unless the mode is composition).
> Emit a verbose level note, then insert every path of 'result' into the
> accumulated results. If the mode is priority-union and the accumulator is now
> non-empty, log and stop early. Return the accumulated results.

> [spec:hfst:def:hfst-lookup.lookup-fd-and-print-fn]
> void

> [spec:hfst:sem:hfst-lookup.lookup-fd-and-print-fn]
> Perform one lookup, optionally print its result pairs, and append its
> one-level results. Two input modes:
> - Given a basic transducer 'tr': if is_possible_to_get_result passes for the
>   current transducer_number, call tr.lookup(input, results_spv, limit, no
>   weight limit, max_number = -1, obey_flags) where 'limit' is the epsilon
>   cycle cap.
> - Given an optimized transducer 'TR' instead: concatenate the path's input
>   symbols into one string and call TR.lookup_pairs(string, *limit,
>   time_cutoff), taking the returned two-level paths.
> If print_pairs_at_this_point and the global print_pairs are both set, print:
> when there are no result pairs and print_fail is set, print
> "<input>\t<input>+?\tinf\n\n" (input via get_lookup_string) and flush; when
> there are results, for each result whose weight is within beam of the lowest
> (first) weight, print the lookup string (input_to_print if supplied, else the
> path), a tab, the colon-joined input:output symbol pairs (skipping flag
> diacritics unless show_flags, separating with spaces when print_space), a tab
> and the path weight plus the input weight; after the loop print a newline
> unless no_newline; always flush. Finally convert every two-level result into a
> one-level path by taking the output side of each pair and insert it into
> 'results' (no flag filtering).

> [spec:hfst:def:hfst-lookup.lookup-input-format]
> enum lookup_input_format {
>   UTF8_TOKEN_INPUT;
>   SPACE_SEPARATED_TOKEN_INPUT;
>   APERTIUM_INPUT;
> }

> [spec:hfst:def:hfst-lookup.lookup-output-format]
> enum lookup_output_format {
>   XEROX_OUTPUT;
>   CG_OUTPUT;
>   APERTIUM_OUTPUT;
> }

> [spec:hfst:def:hfst-lookup.lookup-printf-fn]
> int

> [spec:hfst:sem:hfst-lookup.lookup-printf-fn]
> printf-like result formatter. Render the result side into 'lookupform' and
> the input side into 'inputform' by concatenating each symbol's display form:
> epsilon becomes epsilon_format, flag diacritics are emitted only when
> show_flags, other symbols verbatim, with space_format inserted between
> symbols when print_space. The weight 'w' is the result weight, or +infinity
> when there is no result. Compute substitution sources: %i = inputform,
> %l = lookupform, %b = the lookupform prefix up to the first of '+', ' ', '<'
> or '[' (whole string if none), %a = the remainder from that split point,
> %m = markup (or empty). Then walk the format string copying literal text and
> expanding escapes: %i %l %b %a %m to their sources, %n to a newline, %w to the
> weight printed with %f (never "inf" on non-MSVC), and any other %x to a
> literal "%x". Print the result to 'ofile' (passed through get_print_format
> when quote_special is set) and return the number of characters written.

> [spec:hfst:def:hfst-lookup.lookup-simple-fn]
> HfstOneLevelPaths *

> [spec:hfst:sem:hfst-lookup.lookup-simple-fn]
> Look one path up in a single transducer and return its result set.
> Optimized-lookup variant: when time_cutoff is 0 and the transducer is
> infinitely ambiguous for the input, choose maxnum = max_number if it was set
> (else the default 5), warn that results are limited (mentioning --max-number
> when the limit is the default), then either lookup_fd_and_print (print-pairs
> mode) or lookup_fd(input, maxnum, time_cutoff); mark infinity. Otherwise look
> up with the raw max_number. Basic-transducer variant: pre-filter with
> is_possible_to_get_result; when possible, time_cutoff is 0 and the path is
> infinitely ambiguous, warn (limited to infinite_cutoff cycles) and call
> lookup_fd_and_print with the epsilon-cycle limit, marking infinity; otherwise
> call lookup_fd_and_print with no limit. Both variants emit a verbose "Got no
> results" note when the result set is empty, and forward the
> print_pairs_at_this_point / print_fail / input_to_print / no_newline flags.

> [spec:hfst:def:hfst-lookup.main-fn]
> int

> [spec:hfst:sem:hfst-lookup.main-fn]
> Program entry. Set the locale and program name/version ("0.6"/"HfstLookup"),
> parse options and return early if parsing did not yield EXIT_CONTINUE. Close
> the input FILE buffer when it is not stdin (streams are used instead), emit
> verbose notes about the source/target names and all configured output format
> strings. Open an HfstInputStream from the named input file or stdin (the C++
> reports "<file> is not a valid transducer file" and fails if construction
> throws), run process_stream writing to outfile, close outfile when it is not
> stdout, free the file-name buffers, and return EXIT_SUCCESS.

> [spec:hfst:def:hfst-lookup.parse-options-fn]
> int

> [spec:hfst:sem:hfst-lookup.parse-options-fn]
> Parse command-line options. First apply getenv-supplied extra options, then
> loop over getopt_long with the common + unary tables plus the tool options:
> -I/--input-strings (open the lookup-strings file, mark lookup_given),
> -O/--output-format {xerox,cg,apertium} (apertium also forces apertium input;
> unknown values error out), -F/--input-format {text,spaced,apertium} (unknown
> values error out), -e/-E/--epsilon-format, -b/--beam (non-negative float),
> -t/--time-cutoff (non-negative float), -x/--statistics, -X/--xfst toggling one
> of {print-pairs,print-space (also sets space_format to " "),show-flags,
> quote-special,obey-flags (clears obey_flags)}, -c/--cycles (epsilon cycle
> count), -n/--max-number, -p/--pipe-mode[={both,input,output}], -P/--progress,
> -C/--cascade {union,priority-union,composition}; unrecognized options go to
> the common/unary/error handlers. After the loop, populate the begin/lookup/end
> format strings for the regular, empty, unknown and infinite cases from the
> predefined templates of the chosen output format. If no lookup-strings file
> was given, use stdin (named "<stdin>"). Run the common and unary parameter
> checks and return EXIT_CONTINUE.

> [spec:hfst:def:hfst-lookup.perform-lookups-fn]
> HfstOneLevelPaths *

> [spec:hfst:sem:hfst-lookup.perform-lookups-fn]
> Dispatch a single input path to the right lookup strategy. If the input was
> flagged unknown, return an empty result set. Otherwise, if the cascade holds
> exactly one transducer, call lookup_simple on it (requesting pair printing and
> failure printing); if it holds several, call lookup_cascading. Two overloads
> exist, one taking a vector of optimized transducers and one taking a vector of
> basic transducers, selected by whether the input is all optimized-lookup.

> [spec:hfst:def:hfst-lookup.print-lookup-string-fn]
> static void

> [spec:hfst:sem:hfst-lookup.print-lookup-string-fn]
> Print each symbol of the given string vector to the output file, rendering
> each through get_print_format (so epsilon/quote-special handling applies),
> with no separators.

> [spec:hfst:def:hfst-lookup.print-lookups-fn]
> void

> [spec:hfst:sem:hfst-lookup.print-lookups-fn]
> Print the full result set for one input, beam-limiting the printed analyses.
> Choose the format-string group: if outside_sigma (token unrecognised by the
> analyser) use the unknown_* group and increment no_analyses; else if the
> result set is empty use the empty_* group and increment no_analyses; else if
> the lookup was infinite use the infinite_* group and increment analysed; else
> use the regular group and increment analysed. Always emit the begin string,
> then for each result whose weight is within beam of the lowest (first) weight
> emit the per-lookup string (incrementing analyses) — for the unknown/empty
> cases just the single placeholder line — and finally emit the end string. All
> emission goes through lookup_printf with the given input path, markup and
> output file.

> [spec:hfst:def:hfst-lookup.print-prompt-fn]
> static void

> [spec:hfst:sem:hfst-lookup.print-prompt-fn]
> Write a "> " prompt to stderr, but only when not silent, not in piped-input
> mode, and no lookup-strings file was given (i.e. only when reading
> interactively).

> [spec:hfst:def:hfst-lookup.print-usage-fn]
> void

> [spec:hfst:sem:hfst-lookup.print-usage-fn]
> Print the GNU-style help text to message_out: the usage line, the note that
> hfst-lookup looks up left-to-right (unlike xfst/foma, pointing to
> hfst-flookup), the common program options, the input/output options
> (-i/-o/-p), the lookup options (-I/-O/-e/-F/-x/-X/-c/-n/-b/-t/-C/-P), the
> common unary parameter instructions, the explanations of OFORMAT/IFORMAT/xfst
> VARIABLEs/cycles/epsilon/beam/time-cutoff/multi-transducer behaviour, the
> CASCADE value list, the STREAM/pipe-mode explanation, the Todo and Known-bugs
> notes, and finally the bug-report and more-info footers.

> [spec:hfst:def:hfst-lookup.replace-all-fn]
> static std::string

> [spec:hfst:sem:hfst-lookup.replace-all-fn]
> Return a copy of 'symbol' in which every occurrence of substring 'str1' has
> been replaced by 'str2', scanning left to right and continuing past each
> inserted replacement (an empty 'str1' yields the string unchanged).

> [spec:hfst:def:hfst-lookup.string-to-utf8-fn]
> vector<string> *

> [spec:hfst:sem:hfst-lookup.string-to-utf8-fn]
> Split a byte string into a vector of one-character UTF-8 strings. Read the
> leading byte of each character to determine its length (1 for ASCII, 2/3/4
> from the high-bit prefix 110/1110/11110); a byte that fits none of these is a
> fatal "not valid UTF-8" error reporting the input file and current line.
> Append each multi-byte slice as its own string and advance by that length.

> [spec:hfst:def:hfst-lookup.trans-fn]
> HfstTransducer trans(inputstream)

> [spec:hfst:sem:hfst-lookup.trans-fn]
> Read the next transducer from the input stream. Note its implementation type;
> if it is not an optimized-lookup type, clear the only_optimized_lookup flag.
> Derive its display name (the transducer's stored name, or the input file name
> when empty) and emit a verbose "Reading <name>..." note (suffixed with the
> ordinal for transducers after the first). The transducer is appended to the
> optimized-lookup cascade, and (for SFST/OpenFST/Foma types) a basic-transducer
> copy is also built and appended to the slow-lookup cascade.
