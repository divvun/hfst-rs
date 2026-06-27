# tools/src/hfst-compose-intersect.cc

> [spec:hfst:def:hfst-compose-intersect.check-all-symbols-fn]
> std::string

> [spec:hfst:sem:hfst-compose-intersect.check-all-symbols-fn]
> Given a lexicon transducer and a single rule transducer, decide whether the
> lexicon emits any output symbol that the rule cannot read on its input tape,
> and if so return one such symbol (otherwise return the empty string).
> Steps:
> 1. Convert 'rule' to an HfstBasicTransducer 'rule_b' (a non-destructive copy).
> 2. Collect into a set 'rule_input_symbols' the input symbol of every
>    transition of 'rule_b', iterating states 0..=get_max_state() and all of
>    each state's transitions.
> 3. If that set contains the internal identity symbol ('@_IDENTITY_SYMBOL_@'),
>    the rule reads anything, so return "" immediately.
> 4. Otherwise convert 'lexicon' to an HfstBasicTransducer 'lexicon_b' and
>    iterate its states and transitions in the same order. For the output symbol
>    of each transition, if it is not present in 'rule_input_symbols', return
>    that output symbol at once (first one found, in state/transition order).
> 5. If every lexicon output symbol is found in the rule input set, return "".

> [spec:hfst:def:hfst-compose-intersect.check-multi-char-symbols-fn]
> std::string

> [spec:hfst:sem:hfst-compose-intersect.check-multi-char-symbols-fn]
> Like check-all-symbols but restricted to flagging multi-character symbols,
> ignoring special '@...@' symbols. Steps:
> 1. Convert 'lexicon' to HfstBasicTransducer 'lexicon_b' and 'rule' to
>    'rule_b' (non-destructive copies). Create a fresh HfstTokenizer.
> 2. Build 'rule_input_symbols' = the set of input symbols over all transitions
>    of 'rule_b' (states 0..=get_max_state(), all transitions).
> 3. Iterate the transitions of 'lexicon_b' (states 0..=get_max_state()). For
>    each transition's output symbol that is NOT in 'rule_input_symbols':
>    a. If it is a special symbol (is-special-symbol returns true), skip it.
>    b. Otherwise tokenize it at one level (split_characters = false); if the
>       resulting token vector has more than one element, the symbol is a
>       multi-character symbol unknown to the rule, so return it immediately.
> 4. If none qualifies, return "".

> [spec:hfst:def:hfst-compose-intersect.compose-streams-fn]
> int

> [spec:hfst:sem:hfst-compose-intersect.compose-streams-fn]
> Drive the whole compose-intersect operation over the two open input streams
> (first = lexicon, second = rules) and write the result to the output stream.
> Steps:
> 1. Read each stream's implementation type. If they differ: when transducer
>    conversion is allowed, call conversion_type(type1, type2); a result of 1
>    means use the former type, 2 the latter, -1 the former (with a
>    possible-information-loss note); any other value is an internal error
>    (panic). Emit the assembled message as a warning and set output_type. If
>    conversion is NOT allowed, error out (EXIT_FAILURE) reporting the mismatch.
>    If the types are equal, output_type = type1.
> 2. Open the output stream: by filename if outfile is not stdout, else the
>    stdout-backed stream, both with output_type and hfst_format = true.
> 3. If either input stream is in optimized-lookup format, return EXIT_FAILURE.
> 4. Read all rule transducers from the second stream into a vector: for each,
>    convert it to output_type, log "Reading and minimizing rule <name-or-n>...",
>    then minimize it. If the encode-weights flag is set, temporarily turn on
>    global weight encoding (saving and restoring the previous value) around the
>    minimize() call. Push each minimized rule onto the rules vector.
> 5. For each transducer in the first (lexicon) stream: convert to output_type;
>    obtain its name via hfst_get_name(.., firstfilename); log progress.
>    If there are rules, run check_all_symbols(lexicon, rules[0]); if it returns
>    a non-empty symbol, warn that such output symbols will be filtered out;
>    otherwise run check_multi_char_symbols(lexicon, rules[0]) and, if non-empty,
>    warn about unknown output multi-char symbols.
> 6. If the harmonize flag is set, harmonize every rule against the lexicon
>    (harmonize_rules).
> 7. Compute the result:
>    - If the fast flag is set: when invert is set, copy the lexicon, project to
>      its input side and minimize, compose_intersect(rules, invert=true), then
>      compose that with the original lexicon and keep it as the lexicon; when
>      not inverting, copy the lexicon, project to its output side and minimize,
>      compose_intersect(rules, invert=false), then compose the original lexicon
>      with that copy.
>    - Otherwise call lexicon.compose_intersect(rules, invert).
>    (compose_intersect's harmonize argument keeps its default of true.)
> 8. Set the result transducer's name to
>    "compose(<lexiconname>, intersect(<secondfilename>))" and its formula to
>    " \u{2218} \u{22c2}R" (via the HfstTransducer-source hfst_set_formula
>    overload). Log "Storing result in <outfilename>..." and write the result
>    to the output stream.
> 9. After both streams are exhausted, close first stream, second stream, and
>    output stream, and return EXIT_SUCCESS.

> [spec:hfst:def:hfst-compose-intersect.harmonize-rules-fn]
> void

> [spec:hfst:sem:hfst-compose-intersect.harmonize-rules-fn]
> For every rule transducer in the 'rules' vector, call harmonize against the
> 'lexicon' transducer (in place, in iteration order), so each rule's alphabet
> is reconciled with the lexicon's. Returns nothing.

> [spec:hfst:def:hfst-compose-intersect.is-special-symbol-fn]
> bool

> [spec:hfst:sem:hfst-compose-intersect.is-special-symbol-fn]
> Return true iff 'symbol' is a special bracketed symbol: it has more than two
> bytes, its first byte is '@', and its last byte is '@'. Otherwise false.

> [spec:hfst:def:hfst-compose-intersect.main-fn]
> int

> [spec:hfst:sem:hfst-compose-intersect.main-fn]
> Program entry point. Steps:
> 1. (On Windows, set stdin/stdout to binary mode.)
> 2. hfst_set_program_name(argv[0], "0.1", "HfstComposeIntersect").
> 3. Call parse_options; if it returns anything other than EXIT_CONTINUE,
>    return that value.
> 4. Close the buffered FILE handles that were opened for filenames (firstfile,
>    secondfile, outfile) when they are not the std streams, since the work is
>    done through HFST streams instead.
> 5. Log "Reading from <firstfilename> and <secondfilename>, writing to
>    <outfilename>".
> 6. Open an HfstInputStream for the first input (by filename if not stdin, else
>    the stdin-backed stream); on failure error out as "not a valid transducer
>    file". Do the same for the second input.
> 7. retval = compose_streams(firststream, secondstream); free the duplicated
>    filename strings; return retval.

> [spec:hfst:def:hfst-compose-intersect.parse-options-fn]
> int

> [spec:hfst:sem:hfst-compose-intersect.parse-options-fn]
> Parse command-line options for the binary tool. Steps:
> 1. extend_options_getenv(&argc, &argv) so environment-supplied options are
>    spliced in.
> 2. Loop over getopt_long using the long-option table built from the common
>    long options, the binary long options, and the tool's own
>    --invert (I), --encode-weights (e), --fast (f), --harmonize (a), with a
>    NULL terminator; and the short-option string
>    HFST_GETOPT_COMMON_SHORT + HFST_GETOPT_BINARY_SHORT + "FIeHfa". Stop when
>    getopt_long returns -1.
> 3. Dispatch each option code through the binary cases, then the common cases,
>    then the tool-specific cases ('I' sets invert, 'e' sets encode_weights,
>    'f' sets fast_ci, 'a' sets harmonize), and finally the shared error arm.
> 4. After the loop run the binary parameter check then the common parameter
>    check (resolve first/second/out filenames and streams). Return
>    EXIT_CONTINUE.

> [spec:hfst:def:hfst-compose-intersect.print-usage-fn]
> void

> [spec:hfst:sem:hfst-compose-intersect.print-usage-fn]
> Print the tool's --help text to message_out: a usage line
> "Usage: <program_name> [OPTIONS...] [INFILE1 [INFILE2]]" and a one-line
> description; the common program options; the common binary program options;
> the "Composition options:" block documenting -I/--invert, -f/--fast,
> -e/--encode-weights, and -a/--harmonize; the note about std streams and that
> INFILE1/INFILE2 must share a format, with INFILE1 (lexicon) holding exactly
> one transducer and INFILE2 (rules) possibly several; an Examples block; the
> bug-report footer; and the more-info footer.

> [spec:hfst:def:hfst-compose-intersect.string-set]
> typedef std::set<std::string> StringSet

