# tools/src/hfst-fst2strings.cc

> [spec:hfst:def:hfst-fst2strings.callback]
> class Callback : public hfst::ExtractStringsCb {
>   int count;
>   int max_num;
>   std::ostream *out_;
> }

The path-printing callback. It implements the `ExtractStringsCb` interface so
that `extract_paths`/`extract_paths_fd` can call it back with each path found.
It carries a running `count` of strings printed, a `max_num` ceiling (0 or
negative meaning unbounded), and the destination output stream `out_`.

> [spec:hfst:def:hfst-fst2strings.callback.callback-fn]
> Callback(int max, std::ostream *out) : count(0), max_num(max), out_(out)

> [spec:hfst:sem:hfst-fst2strings.callback.callback-fn]
> Construct a Callback. Initialise `count` to 0, store `max` into `max_num`
> and store the output-stream pointer `out` into `out_`.

> [spec:hfst:def:hfst-fst2strings.callback.operator-fn]
> RetVal

> [spec:hfst:sem:hfst-fst2strings.callback.operator-fn]
> Called for every path candidate `(path, final)`. First concatenate every
> transition's input symbol into `istring` and every output symbol into
> `ostring`, and take `weight = path.first`.
>
> Then apply the path filters, each returning a RetVal that tells the search
> whether to keep going and whether to keep extending this path:
> - if `max_input_length > 0` and `istring` is longer than it, return
>   RetVal(true, false) (keep searching, abandon this path).
> - if `max_output_length > 0` and `ostring` is longer than it, return
>   RetVal(true, false).
> - if `input_prefix` is non-empty: if `istring` is shorter than the prefix
>   return RetVal(true, true) (keep searching, keep extending — the prefix may
>   still be reached); if `istring`'s leading bytes do not equal the prefix
>   return RetVal(true, false).
> - same two checks for `output_prefix` against `ostring`.
> - if `input_exclude` is non-empty and occurs anywhere in `istring`, return
>   RetVal(true, false).
> - if `output_exclude` is non-empty and occurs anywhere in `ostring`, return
>   RetVal(true, false).
> - if `max_weight >= 0` and `weight > max_weight + beam`, return
>   RetVal(true, false).
>
> If the path survived the filters and `final` is true, print it:
> - In pairstring mode (`print_in_pairstring_format`): iterate the transitions.
>   For each, unless flag filtering is on and the input symbol is a flag
>   diacritic, optionally emit a space (when `print_spaces` and not the first
>   pair) then the formatted input symbol; whenever input != output, and unless
>   flag filtering hides the output symbol, emit ":" followed by the formatted
>   output symbol. After the loop, if `display_weights` emit a tab and the
>   weight, then a newline.
> - Otherwise (default mode): iterate the transitions printing the formatted
>   input symbols (subject to the same flag filter and `print_spaces` spacing),
>   tracking whether the path is an automaton (input always equals output). If
>   `print_spaces` emit a trailing space. If it is not an automaton, emit ":"
>   and then iterate again printing the formatted output symbols (flag filtered,
>   with `print_spaces` spacing). If `display_weights` emit a tab and the
>   weight. Emit a newline (flushing, as std::endl does).
>
> When a final path was printed, increment `count`.
> Finally return RetVal((max_num < 1) || (count < max_num), true): keep
> searching until `max_num` strings have been printed, always allowing the
> current path to be extended.

> [spec:hfst:def:hfst-fst2strings.get-print-format-fn]
> static std::string

> [spec:hfst:sem:hfst-fst2strings.get-print-format-fn]
> Format a single symbol `s` for printing. If `s` is the epsilon symbol, return
> the user-configured `epsilon_format` string (default empty). Otherwise, if
> `quote_special` is off, return `s` unchanged. If `quote_special` is on,
> escape the characters that have a special meaning by replacing every space
> with "@_SPACE_@", every colon with "@_COLON_@", and every tab with
> "@_TAB_@" (applied in that nested order).

> [spec:hfst:def:hfst-fst2strings.main-fn]
> int

> [spec:hfst:sem:hfst-fst2strings.main-fn]
> Program entry point. Set the program name/version/wiki name to
> ("0.1", "HfstFst2Strings"); initialise `epsilon_format` to the empty string;
> call parse_options. If `max_strings > 0` and `max_random_strings > 0` and not
> silent, warn that --max_strings is ignored because --random is used and set
> `max_strings = -1`. If parse_options did not return EXIT_CONTINUE, return its
> value. Close the input buffer if it is not stdin (we read via streams).
> Verbose-print "Reading from <in>, writing to <out>". Open an HfstInputStream
> on the input filename (or stdin); on an HfstException print
> "<file> is not a valid transducer file" and return EXIT_FAILURE. Then run
> process_stream writing to the output file (an ofstream on the output filename
> when not stdout, else std::cout). Free the input/output filenames and the
> epsilon format, and return process_stream's result.

> [spec:hfst:def:hfst-fst2strings.parse-options-fn]
> int

> [spec:hfst:sem:hfst-fst2strings.parse-options-fn]
> Parse command-line options. First call extend_options_getenv. Then loop with
> getopt_long over the common long options, the unary long options, and the
> tool's own long options (beam, cycles, epsilon-format, in-exclude, in-prefix,
> max-in-length, max-out-length, max-strings, nbest, random, print-separator,
> out-exclude, out-prefix, print-weights, xfst) plus the short string
> "Swb:c:e:u:p:l:L:n:r:N:U:P:X:" appended to the common and unary short
> strings. Each iteration dispatch the returned option character: first through
> the common cases, then the unary cases, then the tool's own cases:
> - 'n' max-strings: `max_strings = strtoul(optarg, 10)`.
> - 'N' nbest: `nbest_strings = strtoul(optarg, 10)`.
> - 'r' random: `max_random_strings = strtoul(optarg, 10)`.
> - 'b' beam: `beam = atof(optarg)`; if negative print "Invalid argument for
>   --beam" to stderr and return EXIT_FAILURE.
> - 'c' cycles: `cycles = strtoul(optarg, 10)`.
> - 'w' print-weights: `display_weights = true`.
> - 'X' xfst: set the matching flag — "obey-flags" sets `eval_fd`, "print-flags"
>   clears `filter_fd`, "quote-special" sets `quote_special`, "print-pairs" sets
>   `print_in_pairstring_format`, "print-space" sets `print_spaces`; any other
>   value is an error (available options are obey-flags, print-flags).
> - 'l' max-in-length: `max_input_length = strtoul(optarg, 10)`.
> - 'L' max-out-length: `max_output_length = strtoul(optarg, 10)`.
> - 'p' in-prefix: `input_prefix = optarg`.
> - 'P' out-prefix: `output_prefix = optarg`.
> - 'u' in-exclude: `input_exclude = optarg`.
> - 'U' out-exclude: `output_exclude = optarg`.
> - 'S' print-separator: `print_separator_after_each_transducer = true`.
> - 'e' epsilon-format: `epsilon_format = strdup(optarg)`.
> - otherwise fall to the common error case.
> When getopt_long returns -1, break. Then run the common and unary parameter
> checks and return EXIT_CONTINUE.

> [spec:hfst:def:hfst-fst2strings.print-usage-fn]
> void

> [spec:hfst:sem:hfst-fst2strings.print-usage-fn]
> Print the help text to `message_out`. Emit the usage line "Usage: <prog>
> [OPTIONS...] [INFILE]" and the one-line description "Display the strings
> recognized by a transducer", then the common program options, then the
> Fst2strings options block (-n/-N/-r/-c/-w/-S/-e/-X), then the Path filters
> block (-b/-l/-L/-p/-P/-u/-U), then the common unary parameter instructions,
> then the notes about NSTR/NBEST/NCYC defaults and overrides plus the supported
> xfst variables, then the Examples block (using the program name twice), then
> the Known bugs note about optimized lookup format, then report-bugs and
> more-info footers.

> [spec:hfst:def:hfst-fst2strings.process-stream-fn]
> int

> [spec:hfst:sem:hfst-fst2strings.process-stream-fn]
> Read and print every transducer from `instream`, writing to `outstream`.
> Loop while the input stream is good, tracking `first_transducer`:
> - If not the first transducer and `print_separator_after_each_transducer`,
>   print "--" and a newline before the next one.
> - Read one HfstTransducer `t` from the stream.
> - If `print_in_pairstring_format` and the stream is HFST_OL_TYPE or
>   HFST_OLW_TYPE, print an error that pairstring format is unsupported on
>   optimized lookup transducers and exit(1).
> - If `input_prefix` is non-empty, verbose-print it.
> - If `beam >= 0`: verbose-print "Finding the weight of the best path...";
>   copy `t`, prune the copy to the single best path with n_best(1), extract its
>   paths, and if there is not exactly one path error out ("n_best(1) produced
>   more than one path"); set `max_weight` to that path's weight. (n_best not
>   being implemented, or running out of memory, is an error reported against
>   the optimized-lookup or generic case.)
> - If `nbest_strings > 0`: verbose-print "Pruning transducer to N best
>   path(s)..." and prune `t` with n_best(nbest_strings) (same not-implemented /
>   out-of-memory error handling). Otherwise, if no bound at all is set
>   (max_random_strings, max_strings, max_input_length, max_output_length all
>   non-positive and cycles < 0) and `t` is cyclic, error out telling the user
>   to use one of -n/-N/-r/-l/-L/-c.
> - Verbose-print a "Finding ..." message depending on whether max_strings or
>   max_random_strings is positive.
> - If `max_random_strings <= 0` (not random): build a Callback with
>   max_strings; if `eval_fd` call extract_paths_fd(cb, cycles, filter_fd) else
>   extract_paths(cb, cycles); verbose-print how many strings were printed.
> - Else (random): build a results set; if `eval_fd` call
>   extract_random_paths_fd(results, max_random_strings, filter_fd) else
>   extract_random_paths(results, max_random_strings) (a not-implemented case is
>   an error against optimized-lookup or generic); then build a Callback with
>   max_random_strings and feed each result path to it as final, and
>   verbose-print how many random strings were printed.
> After the loop, close the input stream and return EXIT_SUCCESS.

> [spec:hfst:def:hfst-fst2strings.replace-all-fn]
> static std::string

> [spec:hfst:sem:hfst-fst2strings.replace-all-fn]
> Return a copy of `symbol` with every occurrence of `str1` replaced by `str2`.
> Find the first occurrence of `str1`; while one exists, erase it and insert
> `str2` in its place, then continue searching for `str1` starting just past the
> inserted `str2` (so replacements are not re-scanned). Return the modified
> string.
