# tools/src/hfst-grep.cc

> [spec:hfst:def:hfst-grep.extend-matcher-with-options-fn]
> void

> [spec:hfst:sem:hfst-grep.extend-matcher-with-options-fn]
> Wrap the already-built global matcher transducer with the option-driven
> decorations, in order, each step replacing the global matcher with a new
> transducer built at the active format:
> 1. Match-boundary markers. If the global colour mode is COLOUR_ALWAYS, print
>    "Adding color codes to match boundaries..." (verbose) and build two
>    epsilon-to-string transducers: one mapping epsilon to the literal "[31m"
>    (ANSI red, written without the ESC prefix), one mapping epsilon to "[00m"
>    (reset). Otherwise print "Adding brackets to match boundaries..." and build
>    epsilon-to-"{{{" and epsilon-to-"}}}". In both cases set
>    matcher := colorStart . matcher . colorEnd (concatenation), so every match
>    is bracketed by the start marker before and the end marker after.
> 2. Word delimiting. If match_word is set, print "Delimiting matcher to word
>    boundaries (currently space)..." and build a left and a right single-symbol
>    transducer over " " (space), then set matcher := space . matcher . space.
> 3. Repetition/rest. If match_full_line is NOT set, print "Extending matcher
>    for repetitions and rest...". Build leftAny and rightAny as identity-symbol
>    transducers ("@_IDENTITY_SYMBOL_@"), repeat_star each so they accept any run
>    of identity symbols, set matcher := leftAny . matcher . rightAny so a match
>    may occur anywhere in a line surrounded by arbitrary context, then
>    repeat_plus the whole thing so one or more matches per line are accepted.
> 4. Print "Minimising extended matcher..." and minimize the matcher. If verbose
>    is set, print "Resulting FSM:" and dump the matcher in AT&T text format to
>    stderr.

> [spec:hfst:def:hfst-grep.main-fn]
> int

> [spec:hfst:sem:hfst-grep.main-fn]
> Entry point. Set the locale (hfst_setlocale). Register the program name as
> argv[0] with version "0.1" and wiki name "HfstGrep". Call parse_options; if it
> returns anything other than EXIT_CONTINUE, return that value immediately.
> Print (verbose) "Writing to <outfilename>". Build the matcher from the regexp
> string via read_matcher(regexp), then decorate it via
> extend_matcher_with_options. (The optimise_matcher call is compiled out unless
> HFST_OPTIMISED_LOOKUP_CAN_IDENTITY_SYMBOL is defined.) For each input file
> index i in 0..infile_n: set inputfilename to infilenames[i], reset the line
> counter linen to 0, and call match_lines(infiles[i], infilenames[i]). Finally
> free the output filename buffer and return the parse_options return value.

> [spec:hfst:def:hfst-grep.match-lines-fn]
> bool

> [spec:hfst:sem:hfst-grep.match-lines-fn]
> Read a single input file line by line and report matches against the global
> matcher; return whether the file is considered to have matched. Print
> "matching against <infilename>..." (verbose). Track matched (any line matched)
> and matches_n (count of matched lines). The active code path (when
> HFST_OPTIMISED_LOOKUP_CAN_IDENTITY is NOT defined) creates one HfstTokenizer
> and then, for each line read by getline:
> - increment the line counter linen;
> - truncate the line at the first newline by overwriting it with NUL;
> - print "matching <line>..." (verbose);
> - if the line is now empty, skip it (continue);
> - tokenize the line into an identity transducer lineTrans (upper = lower =
>   line, via the tokenizer, at the active format);
> - print "composing..." (verbose), then compute
>   results := (lineTrans .o. matcher) projected to the output side;
> - build an empty transducer at the active format and compare results to it:
>   - if results equals empty (no matches): print "no matches"; if invert_matches
>     is set, print the unmatched line via print_match_transducer(lineTrans);
>   - else (matches): print "matches"; if invert_matches is NOT set, print the
>     matched output via print_match_transducer(results); mark matched = true and
>     increment matches_n.
> After each line, if flush_newlines is set, flush the output file. If max_count
> is positive and matches_n has reached max_count, stop reading. Return matched
> normally, or its negation when invert_matches is set. (The
> HFST_OPTIMISED_LOOKUP_CAN_IDENTITY branch, which would tokenize via
> string_to_utf8 and lookup against optimised_matcher, is compiled out and never
> executed.)

> [spec:hfst:def:hfst-grep.optimise-matcher-fn]
> void

> [spec:hfst:sem:hfst-grep.optimise-matcher-fn]
> Print "Optimising..." (verbose) and build the global optimised_matcher as a
> copy of the global matcher converted to the HFST optimized-lookup format
> (HFST_OL_TYPE). Only reachable when HFST_OPTIMISED_LOOKUP_CAN_IDENTITY_SYMBOL
> is defined; otherwise compiled out and never called.

> [spec:hfst:def:hfst-grep.parse-options-fn]
> int

> [spec:hfst:sem:hfst-grep.parse-options-fn]
> Parse the command line into the tool's globals using getopt_long. First extend
> argv from the environment (extend_options_getenv). The long-option table is the
> common long options, then the unary long options, then the tool-specific
> options below, terminated by a zero entry; the matching short string is the
> common + unary short strings followed by
> "EFGPXe:f:IwxzqmbnOad:D:rLlcZA:B:C:uU9:". Loop reading options:
> - common option cases are handled by the shared common handler (which may print
>   usage and return, or continue);
> - '9' (--format): set format from hfst_parse_format_name(optarg);
> - 'E' (--extended-regexp): error out "POSIX ERE syntax not yet supported", then
>   set dialect_posix_ere;
> - 'F' (--fixed-strings): set dialect_fixed_strings;
> - 'G' (--basic-regexp): error out "POSIX BRE syntax not yet supported", then set
>   dialect_posix_bre;
> - 'P' (--perl-regexp): error out "Perl syntax not yet supported", then set
>   dialect_perl;
> - 'X' (--xerox-regexp): set dialect_xerox;
> - 'e' (--regexp): strdup optarg into regexp;
> - 'f' (--file): open optarg for reading into expfile;
> - 'I' (--ignore-case): error out "Ignore case not supported";
> - 'w' (--word-regexp): set match_word;
> - 'x' (--line-regexp): set match_full_line;
> - 'z' (--null-data): set linesep to 0;
> - INVERT_OPT (--invert-match, value 19): set invert_matches;
> - 'm' (--max-count): set max_count from hfst_strtoul(optarg, base 10) and set
>   count_matches;
> - 'b' (--byte-offset): set print_offset;
> - 'n' (--line-number): set print_linenumbers;
> - LINEBUFFER_OPT (--line-buffered, 20): set flush_newlines;
> - 'H' (--with-filename): set print_filenames;
> - 'O' (--only-matching): set print_only_matches;
> - BINARYFILES_OPT (--binary-files, 22): error "No binary handling implemented";
> - 'a' (--text): warn "All files are always handled as text";
> - 'D' (--devices) and 'r' (--recursive): error "No directory handling
>   implemented";
> - INCLUDE/EXCLUDE/INCLUDEFROM/EXCLUDEFROM (23..26): error "No directory/globbing
>   implemented";
> - 'L' (--files-without-match): set print_only_unmatching_filenames;
> - 'l' (--files-with-match): set print_only_matching_filenames;
> - 'c' (--count): set count_matches and print_only_count;
> - 'Z' (--null): set print_filename_null;
> - 'A' (--before-context): set before_context from hfst_strtoul(optarg, 10);
> - 'B' (--after-context): set after_context from hfst_strtoul(optarg, 10);
> - 'C' (--context): set both before_context and after_context from
>   hfst_strtoul(optarg, 10);
> - 'u'/'U' (--binary/--unix-byte-offset): error "MSDOS binary format not
>   supported; use fromdos or dos2unix";
> - any other option: the shared error case.
> After the loop: if no dialect flag was set, warn "Dialect not defined,
> defaulting to Xerox for now!" and set dialect_xerox. If format is unspecified,
> default it to TROPICAL_OPENFST_TYPE. If neither regexp nor expfile was given:
> if there are no remaining free arguments, print usage and short help and return
> EXIT_FAILURE; otherwise strdup the next free argument (argv[optind]) into regexp
> and advance optind. Then build the input file arrays: if no free arguments
> remain, allocate one entry, name it "<stdin>" and use stdin; otherwise allocate
> one entry per remaining free argument, strdup each name and open each for
> reading (hfst_fopen). Apply the common post-parse parameter check and return
> EXIT_CONTINUE.

> [spec:hfst:def:hfst-grep.print-match-line-fn]
> void

> [spec:hfst:sem:hfst-grep.print-match-line-fn]
> Print one matched line from a one-level path (the optimised-lookup output
> representation). If print_only_matching_filenames or
> print_only_unmatching_filenames is set, print nothing and return. Otherwise:
> if print_filenames is set, write the input filename, followed by a single NUL
> byte when print_filename_null is set or ": " otherwise; if print_linenumbers is
> set, write "<linen>: "; then write each symbol of the path's symbol vector in
> order, and finish with a newline. Only reachable in the optimised-lookup build,
> which is compiled out; never executed in the active build.

> [spec:hfst:def:hfst-grep.print-match-transducer-fn]
> void

> [spec:hfst:sem:hfst-grep.print-match-transducer-fn]
> Print one matched line from a transducer by extracting a single path. Extract
> up to one two-level path from the transducer. If print_only_matching_filenames
> or print_only_unmatching_filenames is set, print nothing and return. Otherwise:
> if print_filenames is set, write the input filename, followed by a single NUL
> byte when print_filename_null is set or ": " otherwise; if print_linenumbers is
> set, write "<linen>: "; then iterate the first extracted path's symbol pairs and
> write the input (first) side of each pair whose input symbol is not epsilon;
> finish with a newline.

> [spec:hfst:def:hfst-grep.print-usage-fn]
> void

> [spec:hfst:sem:hfst-grep.print-usage-fn]
> Print the help text to message_out: a usage line "Usage: <program> [OPTIONS...]
> PATTERN [FILE...]" with a one-paragraph description noting PATTERN defaults to a
> Xerox regular expression and an example invocation; then the common program
> options followed by the tool's "-9, --format=TYPE" line; then the grouped option
> sections "Regexp selection and interpretation", "Miscellaneous options",
> "Output control", and "Context control", each listing the tool's flags with
> descriptions (matching GNU grep's layout, including options the tool does not
> actually implement); then the bug-report address and the pointer to external
> documentation.

> [spec:hfst:def:hfst-grep.read-matcher-fn]
> int

> [spec:hfst:sem:hfst-grep.read-matcher-fn]
> Two overloads build the global matcher transducer.
>
> read_matcher(HfstInputStream&): build the matcher by reading transducers from a
> binary stream. Initialise the global matcher as an empty transducer of the
> stream's type. For each transducer in the stream: increment the count; read it;
> take its name (or the input filename if it has none); print "Reading matcher
> <name>..." for the first, "Reading matcher <name>...<n>" for later ones, and
> "and disjuncting..." for the second and beyond; then disjunct the matcher with
> the transducer's input projection. After the loop print "minimising
> matchers...", minimize the matcher, close the stream, and return EXIT_SUCCESS.
>
> read_matcher(const char* expression): build the matcher by compiling a string
> expression. Initialise the global matcher as an empty transducer at the active
> format. If dialect_xerox: create an XreCompiler at the format, print "parsing
> <expression> as Xerox style regular expression...", compile the expression, and
> disjunct the matcher with the compiled transducer's input projection. Else if
> dialect_fixed_strings: print "parsing <expression> as fixed string of UTF-8
> symbols...", tokenize the expression into an identity transducer (upper = lower
> = expression) at the format, and disjunct that into the matcher. Otherwise error
> out "dialect unsupported". Then print "minimizing...", minimize the matcher, and
> if verbose print "Resulting FSM:" and dump the matcher in AT&T text format to
> stderr. This overload is the one main uses.

> [spec:hfst:def:hfst-grep.string-to-utf8-fn]
> vector<string> *

> [spec:hfst:sem:hfst-grep.string-to-utf8-fn]
> Split a C string into a vector of UTF-8 symbol strings, one per code point.
> Walk the bytes from the start; for each leading byte determine the encoded
> length from its high bits: 1 byte for 0xxxxxxx (<= 127), 4 bytes for 11110xxx,
> 3 bytes for 1110xxxx, 2 bytes for 110xxxxx; any other leading byte is invalid
> UTF-8 and triggers an error_at_line on the current input file and line with the
> message "<rest-of-string> not valid UTF-8". Duplicate that many bytes into a
> fresh string, append it to the result vector, advance the pointer by the symbol
> length, and free the temporary. Return the vector. Used only by the
> optimised-lookup match path, which is compiled out; never executed in the
> active build.
