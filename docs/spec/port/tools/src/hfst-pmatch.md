# tools/src/hfst-pmatch.cc

> [spec:hfst:def:hfst-pmatch.libreadline-getline-fn]
> void

> [spec:hfst:sem:hfst-pmatch.libreadline-getline-fn]
> Compiled only under HAVE_READLINE (not defined in this build). Read one line of
> interactive input into *buffer using the readline library: if *buffer is
> already allocated, free it and null it; then set *buffer to readline("") (an
> empty prompt); and if the returned line is non-null and non-empty, append it to
> the readline history with add_history. In the non-readline build this whole
> helper is absent and the equivalent line reading is done by hfst_getline in
> process-input-fn.

> [spec:hfst:def:hfst-pmatch.main-fn]
> int

> [spec:hfst:sem:hfst-pmatch.main-fn]
> Entry point. Set the program name/version/wikiname via hfst_set_program_name
> (argv[0], "0.1", "HfstPmatch"), then hfst_setlocale(). Call parse_options(argc,
> argv); if it does not return EXIT_CONTINUE, return that value. (Under
> HAVE_READLINE, rebind TAB to plain insert to disable completion; compiled out
> here.) Open inputfilename as a binary input stream; if the stream is not good,
> print "Could not open file <name>" to stderr and return EXIT_FAILURE. Inside a
> try block, construct a hfst_ol::PmatchContainer from that stream, then apply the
> option state to it: set_verbose(verbose); if extract_patterns/locate_mode/
> count_patterns/delete_patterns/mark_patterns were given (not not_defined) call
> the matching setter with (value == on); if max_context >= 0 call set_max_context
> and if max_recursion >= 0 call set_max_recursion; always call set_profile
> (profile). Return process_input(container, std::cout). If constructing or using
> the container throws HfstException, the catch arm prints a hint that the archive
> in inputfilename does not look right (suggesting hfst-pmatch2fst / weighted
> optimized-lookup format) to stderr and returns 1.

> [spec:hfst:def:hfst-pmatch.match-and-print-fn]
> void

> [spec:hfst:sem:hfst-pmatch.match-and-print-fn]
> Match one block of input text against the container and write the result to
> outstream. First, if input_text ends in a trailing newline, erase that final
> newline. If the container is NOT in locate mode: write
> container.match(input_text, time_cutoff, weight_cutoff) to outstream followed by
> a newline, and one extra blank newline when blankline_separated is set. If the
> container IS in locate mode: call container.locate(input_text, time_cutoff,
> weight_cutoff) to get a LocationVectorVector; for each location vector, look at
> its first element [0], and unless its output equals "@_NONMATCHING_@" record
> that something was printed and write "start|length|output|tag" (and, when the
> print_weights flag is truthy in the C boolean sense — note the enum bug where
> 'on' is value 0 and therefore false — append "|weight"), each followed by a
> newline. After the loop, if anything was printed, write one trailing blank
> newline.

> [spec:hfst:def:hfst-pmatch.parse-options-fn]
> int

> [spec:hfst:sem:hfst-pmatch.parse-options-fn]
> Parse the command line. First extend_options_getenv(&argc, &argv). Then loop
> getopt_long over the option table built from the common + unary long options
> plus the pmatch-specific long options (newline n, extract-patterns x, locate l,
> print-weights w, count-patterns c, delete-patterns z, no-mark-patterns m,
> max-context b:, max-recursion r:, weight-cutoff W:, time-cutoff t:, profile p)
> with a null terminator, and the short string HFST_GETOPT_COMMON_SHORT +
> HFST_GETOPT_UNARY_SHORT + "nxlwcdmb:r:W:t:p"; break when getopt_long returns -1.
> Dispatch each option char through the common case group, then the unary case
> group, then the tool's own cases: 'n' clears blankline_separated; 'x' sets
> extract_patterns=on; 'l' sets locate_mode=on; 'w' sets print_weights=on; 'c'
> sets count_patterns=on; 'z' sets delete_patterns=on; 'm' sets mark_patterns=off;
> 'b' sets max_context=atoi(optarg), failing with EXIT_FAILURE and an
> "Invalid argument for --max-context" message if negative; 'r' sets
> max_recursion=atoi(optarg), failing similarly for --max-recursion; 'W' sets
> weight_cutoff=atof(optarg), failing for --weight-cutoff if negative and then —
> bug-for-bug, the C 'case W' has no break — falling through into the 't' logic
> that sets time_cutoff=atof(optarg) (failing for --time-cutoff if negative); 't'
> sets time_cutoff=atof(optarg) with the same negative check; 'p' sets
> profile=true; anything else goes to the error case. After the loop, resolve the
> single positional input filename from optind: if more than one remains print
> "More than one input file given" and return EXIT_FAILURE; if exactly one
> remains, error out the same way if inputfilename was already set, else strdup it
> into inputfilename and hfst_fopen it for reading (rewriting the name to "<stdin>"
> if the opened file is stdin) and return EXIT_CONTINUE; if none remains, return
> EXIT_FAILURE with "No input file given" when inputfilename is null, otherwise
> EXIT_CONTINUE.

> [spec:hfst:def:hfst-pmatch.process-input-fn]
> int

> [spec:hfst:sem:hfst-pmatch.process-input-fn]
> Drive the matcher over stdin line by line. Maintain an accumulating input_text
> string. Loop reading lines from stdin (in this build via hfst_getline; the
> interactive readline path is compiled out), breaking when a read fails / returns
> nothing. For each line: if blankline_separated is off (newline-separated mode),
> set input_text to the line and immediately match_and_print it; otherwise, in
> blank-line-separated mode, a line whose first char is NUL or '\n' triggers a
> match_and_print of the accumulated input_text then clears it, while any other
> line is appended to input_text (in the interactive case a trailing '\n' is also
> appended). Free and null the line buffer each iteration. After the loop, if
> blankline_separated and input_text is non-empty, match_and_print the remainder.
> If count_patterns == on, write a blank line, container.get_pattern_count_info(),
> and a newline to outstream. If profile, write a blank line,
> container.get_profiling_info(), and a newline. Return EXIT_SUCCESS.

> [spec:hfst:def:hfst-pmatch.var-val]
> enum var_val {
>   on;
>   off;
>   not_defined;
> }

> [spec:hfst:sem:hfst-pmatch.var-val]
> A three-state tri-bool used for the on/off pmatch toggles whose default
> ("not set on the command line") must be distinguished from an explicit on or
> off. The enumerators are declared in the order on, off, not_defined, giving them
> the C values 0, 1, 2 respectively. This ordering is load-bearing: the
> match_and_print weight test is written as a plain C truthiness check on the
> print_weights value, so the 'on' enumerator (value 0) reads as false there. All
> the toggle variables default to not_defined; main only forwards a setter to the
> container when the value is not not_defined, passing (value == on) as the bool.
