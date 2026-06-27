# tools/src/hfst-tokenize.cc

> [spec:hfst:def:hfst-tokenize.first-transducer-is-called-top-fn]
> bool

> [spec:hfst:sem:hfst-tokenize.first-transducer-is-called-top-fn]
> Predicate over an HfstTransducer: returns true iff the transducer's name
> (its stored "name" property, as returned by get_name) is exactly the string
> "TOP". Defined in the source as a helper but not called from main (main
> instead inspects the parsed header attributes directly); kept for fidelity.

> [spec:hfst:def:hfst-tokenize.main-fn]
> int

> [spec:hfst:sem:hfst-tokenize.main-fn]
> Entry point. Steps, in order:
> 1. Call hfst_set_program_name(argv[0], "0.1", "HfstTokenize") then
>    hfst_setlocale().
> 2. retval = parse_options(argc, argv); if retval != EXIT_CONTINUE, return
>    retval immediately (this covers --help/--version/error exits as well as
>    the option-validation failures inside parse_options).
> 3. verbose_printf("Reading from %s, writing to %s\n", tokenizer_filename,
>    outfilename).
> 4. Open tokenizer_filename as a binary input file. If it cannot be opened
>    (not good), print "Could not open file <name>" to stderr and return
>    EXIT_FAILURE.
> 5. Decide whether the archive was produced by a pmatch ruleset without
>    loading the whole transducer: read only the header attributes via
>    PmatchContainer::parse_hfst3_header(instream), then rewind the stream to
>    the beginning (seekg(0)/clear). If parse_hfst3_header throws
>    TransducerHeaderException, print "<name> doesn't look like a HFST archive.
>    Exiting." plus the exception text to stderr and return 1.
> 6. If the header has no "name" attribute, or its "name" is not "TOP":
>    verbose_printf("No TOP automaton found, using naive tokeniser?\n"); open
>    the file as an hfst::HfstInputStream, read one HfstTransducer (the
>    dictionary), close the binary stream, build the container with
>    make_naive_tokenizer(dictionary), delete the dictionary.
>    Otherwise (name == "TOP"): verbose_printf("TOP automaton seen, treating as
>    pmatch script...\n"); construct the container directly from the input
>    stream (PmatchContainer(instream)).
> 7. In both branches: container.set_verbose(verbose);
>    container.set_single_codepoint_tokenization(!settings.tokenize_multichar);
>    return process_input(container, std::cout).
> 8. The whole body is wrapped in try/catch on HfstException: on such an
>    exception print "Exception thrown:\n<what>" to stderr and return 1. (In
>    the Rust port the constructors panic rather than throw, so the catch arms
>    are not reproduced.)

> [spec:hfst:def:hfst-tokenize.make-naive-tokenizer-fn]
> hfst_ol::PmatchContainer

> [spec:hfst:sem:hfst-tokenize.make-naive-tokenizer-fn]
> Build a naive (no-ruleset) tokenizer PmatchContainer that wraps an arbitrary
> dictionary transducer, using default_format (TROPICAL_OPENFST_TYPE) for all
> constructions. Steps:
> 1. word_boundary = make_latin1_whitespace_acceptor; punctuation =
>    make_latin1_punct_acceptor; word_boundary->disjunct(punctuation).
> 2. others = make_exc_list(word_boundary) (a one-symbol acceptor for the
>    "everything except these" list); others->repeat_plus(); set every final
>    weight of others to float max, so the default token is less likely than
>    any dictionary token.
> 3. word_boundary_list = make_list(word_boundary); disjunct it with a
>    transducer accepting the literal "@BOUNDARY@" (pmatch's special input
>    boundary marker). Delete word_boundary and punctuation.
> 4. left_context = epsilon:LC_ENTRY_SYMBOL; right_context =
>    epsilon:RC_ENTRY_SYMBOL. Concatenate word_boundary_list onto each; delete
>    word_boundary_list. Then concatenate epsilon:LC_EXIT_SYMBOL onto
>    left_context and epsilon:RC_EXIT_SYMBOL onto right_context; delete those
>    exit transducers.
> 5. dict_name = dictionary->get_name(); if empty set it to
>    "unknown_pmatch_tokenized_dict" and store it back on the dictionary.
> 6. dict_ins_arc = acceptor for get_Ins_transition(dict_name) (i.e.
>    "@I.<dict_name>@"). others->disjunct(dict_ins_arc): the center of the
>    tokenizer is "any non-dictionary run OR an Ins of the dictionary".
> 7. Combine with context: left_context->concatenate(others);
>    left_context->concatenate(right_context); delete others and right_context.
> 8. tokenizer = add_pmatch_delimiters(left_context) (wraps with the pmatch
>    entry/exit delimiter markers, required because there are context
>    conditions); tokenizer->set_name("TOP"); tokenizer->minimize().
> 9. dictionary->convert(HFST_OLW_TYPE) (to optimized-lookup weighted if it
>    wasn't already). Compute tokenizer_syms minus dict_syms (set difference of
>    the two alphabets) and insert each of those symbols into the dictionary's
>    alphabet, so the dictionary harmonizes with the tokenizer.
> 10. Convert tokenizer to a HfstBasicTransducer, then to a hfst_ol::Transducer
>    (weighted = true, no special options, harmonized with the dictionary).
>    Construct the result PmatchContainer from that tokenizer_ol; convert the
>    dictionary to a hfst_ol::Transducer backend and add_rtn it under dict_name.
>    Return the container. (Note: the C++ harmonizer argument to
>    hfst_basic_transducer_to_hfst_ol is the HfstTransducer dictionary, which it
>    converts internally; the Rust port shifts that conversion to the caller, so
>    the dictionary's ol backend is produced once and used both as the
>    harmonizer and for add_rtn.)

> [spec:hfst:def:hfst-tokenize.maybe-erase-newline-fn]
> inline void

> [spec:hfst:sem:hfst-tokenize.maybe-erase-newline-fn]
> Given a mutable input_text string: if keep_newlines is false and the string
> is non-empty and its last character is '\n', erase exactly that final
> newline character. Otherwise leave it unchanged.

> [spec:hfst:def:hfst-tokenize.parse-options-fn]
> int

> [spec:hfst:sem:hfst-tokenize.parse-options-fn]
> Parse argv. First call extend_options_getenv(&argc, &argv). Then loop over
> getopt_long with the common long options plus the tool-specific table:
> --newline/-n, --keep-newline/-k, --print-all/-a, --print-weights/-w,
> --no-weights/-W, --tokenize-multichar/-m, --beam/-b (arg), --time-cutoff/-t
> (arg), --weight-classes/-l (arg), --unique/-u, --segment/-z,
> --space-separated/-d, --xerox/-x, --cg/-c, --superblanks/-S, --giella-cg/-g,
> --gtd/-g, --conllu/-C, --finnpos/-f, --visl/-L; short string is the common
> short options followed by "nkawWmub:t:l:zixcSgCfL". For each returned option
> code:
>  - common cases handled by the shared getopt-cases-common include (help,
>    version, verbose, quiet, etc.) — these may return a code or break.
>  - 'k': keep_newlines = true and blankline_separated = false.
>  - 'n': blankline_separated = false.
>  - 'a': settings.print_all = true.
>  - 'w': settings.print_weights = true.
>  - 'W': settings.print_weights = false.
>  - 'm': settings.tokenize_multichar = true.
>  - 't': settings.time_cutoff = atof(optarg); if < 0.0 print
>    "Invalid argument for --time-cutoff" to stderr and return EXIT_FAILURE.
>  - 'u': settings.dedupe = true.
>  - 'b': settings.beam = atof(optarg); if < 0 print "Invalid argument for
>    --beam" and return EXIT_FAILURE.
>  - 'l': settings.max_weight_classes = atoi(optarg); if < 1 print "Invalid or
>    no argument --weight-classes count" and return EXIT_FAILURE.
>  - 'z': output_format = tokenize. 'i': output_format = space_separated.
>    'x': output_format = xerox. 'c': output_format = cg.
>    'C': output_format = conllu. 'f': output_format = finnpos.
>  - 'S': superblanks = true.
>  - 'g': output_format = giellacg; print_weights = true; print_all = true;
>    dedupe = true; hack_uncompose = true; verbose = false; and if
>    max_weight_classes is still int max, set it to 2.
>  - 'L': output_format = visl; print_weights = false; print_all = true;
>    dedupe = true; verbose = false.
>  - the shared getopt-cases-error include handles the default/unknown arm.
> After handling each option, if the global verbose flag is set then also set
> settings.verbose = true.
> When getopt finishes: if optind+1 < argc print "More than one input file
> given" to stderr and return EXIT_FAILURE; else if optind+1 == argc set
> tokenizer_filename = argv[optind] and return EXIT_CONTINUE; else print
> "No input file given" and return EXIT_FAILURE.

> [spec:hfst:def:hfst-tokenize.print-usage-fn]
> void

> [spec:hfst:sem:hfst-tokenize.print-usage-fn]
> Print the help text to message_out: a usage line
> "Usage: <program_name> [--segment | --xerox | --cg | --giella-cg]
> [OPTIONS...] RULESET" followed by "perform matching/lookup on text streams"
> and a blank line; then print_common_program_options(message_out); then the
> block documenting every tool-specific option (-n/--newline, -a/--print-all,
> -w/--print-weight, -W/--no-weights, -m/--tokenize-multichar, -b/--beam,
> -tS/--time-cutoff, -lN/--weight-classes, -u/--unique, -z/--segment,
> -i/--space-separated, -x/--xerox, -c/--cg, -S/--superblanks, -g/--giella-cg,
> -C/--conllu, -f/--finnpos, -L/--visl) with their descriptions; then
> "Use standard streams for input and output (for now)." and a blank line; then
> print_report_bugs(); a newline; print_more_info(); a newline.

> [spec:hfst:def:hfst-tokenize.process-input-0delim-fn]
> int

> [spec:hfst:sem:hfst-tokenize.process-input-0delim-fn]
> NUL-delimited input processor, templated on a compile-time bool
> do_superblank (the Rust port passes it as a runtime parameter). Reads the
> input file in chunks delimited by '\0' via hfst_getdelim. For each chunk,
> iterate over its bytes maintaining a running buffer 'cur', a bool 'in_blank'
> (persists across chunks), and a per-byte 'escaped' flag (reset to false at
> the start of each chunk; beginning of line is necessarily unescaped). For
> byte line[i]:
>  - if escaped: append line[i] to cur, clear escaped, continue.
>  - else if do_superblank && !in_blank && line[i]=='[': flush cur via
>    process_input_0delim_print, then append '[' to cur and set in_blank.
>  - else if do_superblank && in_blank && line[i]==']': append ']' to cur; if
>    the next byte exists and is '[', consume it too (advance i, append it) to
>    join consecutive superblanks; otherwise clear in_blank, emit cur via
>    print_nonmatching_sequence, and reset cur.
>  - else if !in_blank && line[i]=='\n': append '\n' to cur; if verbose print
>    "processing: <cur>\n" to stdout; then flush cur via
>    process_input_0delim_print.
>  - else if line[i]=='\0': if verbose print "processing: <cur>\0"; flush cur
>    via process_input_0delim_print; write "<STREAMCMD:FLUSH>\n" to outstream
>    and flush it (CG format uses this instead of a literal NUL); if the
>    outstream is in a bad state print "hfst-tokenize: Could not flush file" to
>    stderr.
>  - else: append line[i] to cur.
> After processing each byte, set escaped = (line[i]=='\\'). After each chunk
> free the line buffer and break out of the read loop if feof. After the loop:
> if in_blank, emit the remaining cur via print_nonmatching_sequence; otherwise
> flush it via process_input_0delim_print. Return EXIT_SUCCESS.

> [spec:hfst:def:hfst-tokenize.process-input-0delim-print-fn]
> inline void

> [spec:hfst:sem:hfst-tokenize.process-input-0delim-print-fn]
> Helper used by process_input_0delim: take the current buffered text (cur's
> string contents); if it is non-empty, run match_and_print(container,
> outstream, input_text, settings). In all cases then clear cur (reset it to
> the empty string).

> [spec:hfst:def:hfst-tokenize.process-input-fn]
> int

> [spec:hfst:sem:hfst-tokenize.process-input-fn]
> Top-level input dispatcher. (For cg/giellacg/visl the C++ sets the output
> stream to std::fixed with precision 10; the library print functions format
> weights themselves so there is no separate flag to mirror.) Dispatch:
>  - If output_format == giellacg OR superblanks: if superblanks,
>    verbose_printf("Processign giellacg with superblanks\n") and return
>    process_input_0delim<true>; else verbose_printf("Processign giellacg
>    without superblanks\n") and return process_input_0delim<false>.
>  - Else if output_format == visl: verbose_printf("Processign VISL CG 3\n")
>    and return process_input_visl.
>  - Else, plain line-based processing. If blankline_separated:
>    verbose_printf("Processing blankline separated input\n"); read lines with
>    hfst_getline; if a line is just "\n" (blank line), maybe_erase_newline the
>    accumulated input_text, match_and_print it, then clear it; otherwise append
>    the line to input_text. After EOF, if input_text is non-empty,
>    maybe_erase_newline and match_and_print it.
>  - Else (newline or non-separated): verbose_printf("Processing non-separated
>    input\n"); read each line, set input_text = line, maybe_erase_newline,
>    match_and_print.
> Return EXIT_SUCCESS.

> [spec:hfst:def:hfst-tokenize.process-input-visl-fn]
> int

> [spec:hfst:sem:hfst-tokenize.process-input-visl-fn]
> VISL CG-3 line processor. Read newline-delimited lines via hfst_getline into
> a string, trim() it. For a non-empty trimmed line: if it both starts with
> '<' and ends with '>' it is treated as markup and emitted via
> print_nonmatching_sequence; otherwise it is matched via match_and_print. For
> an empty trimmed line, write a single '\n' to outstream. After each line
> flush the outstream, reset the buffer's first byte to 0, and break if feof.
> After the loop a final (possibly partial) line is handled the same way
> (clamping a negative getline length to 0 first), then a final flush, then the
> buffer is freed. Return EXIT_SUCCESS.

> [spec:hfst:def:hfst-tokenize.trim-fn]
> inline void

> [spec:hfst:sem:hfst-tokenize.trim-fn]
> In-place trim of a string: repeatedly pop the last character while the string
> is non-empty and that character is whitespace (per isspace) or a NUL byte;
> then repeatedly erase the first character while the string is non-empty and
> that character is whitespace or NUL. Removes leading and trailing whitespace
> and NUL bytes.
