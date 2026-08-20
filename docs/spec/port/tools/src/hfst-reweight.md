# tools/src/hfst-reweight.cc

> [spec:hfst:def:hfst-reweight.func-fn]
> float (*func)(float) = id

> [spec:hfst:sem:hfst-reweight.func-fn]
> The currently selected per-weight transformation function, a pointer to a
> single-argument float->float function. It is initialised to 'id' (the
> identity function). Option '-F'/'--function=FNAME' rebinds it to the
> matching <cmath> float function: cos, sin, tan, acos, asin, atan, cosh,
> sinh, tanh, exp, log (natural logarithm), log10, sqrt, floor or ceil. Any
> other FNAME is an error. The reweight formula evaluates this function on the
> weight as MULTIPLIER * func(w) + ADDITION.

> [spec:hfst:def:hfst-reweight.id-fn]
> static float

> [spec:hfst:sem:hfst-reweight.id-fn]
> The identity function on weights: takes a float w and returns w unchanged.
> Used as the default value of the 'func' function pointer so that, absent a
> '-F' option, reweighting leaves func(w) == w.

> [spec:hfst:def:hfst-reweight.main-fn]
> int

> [spec:hfst:sem:hfst-reweight.main-fn]
> Program entry point. Sets the program name/version/wikiname to
> ("hfst-reweight", "0.1", "HfstReweight"), then calls parse_options; if that
> returns anything other than EXIT_CONTINUE, returns that value. Closes the
> buffered input/output FILE handles when they are not stdin/stdout (streams
> are used instead). Emits verbose diagnostics: the read/write filenames; the
> active reweighting formula "Modifying weights LOWER_BOUND < w < UPPER_BOUND
> as MULTIPLIER * FUNCNAME(w) + ADDITION"; and, conditionally, "only if arc
> has symbol SYM", "only if input symbol is ISYM", "only if output symbol is
> OSYM", "only on final weights, no arcs" (when ends-only) and "only on arc
> weights, no end states" (when arcs-only). Opens an HfstInputStream from the
> input filename (or stdin); on failure to construct it the tool errors out
> with "<file> is not a valid transducer file". Opens an HfstOutputStream of
> the input stream's type to the output filename (or stdout). If the input
> stream is in optimized-lookup format, returns EXIT_FAILURE. Otherwise calls
> process_stream on the two streams, frees the input/output filename buffers,
> and returns its result.

> [spec:hfst:def:hfst-reweight.original-fn]
> HfstBasicTransducer original(trans)

> [spec:hfst:sem:hfst-reweight.original-fn]
> The core single-transducer reweighting step (do_reweight). Converts the
> facade transducer 'trans' into a mutable basic transducer 'original' and
> builds a fresh basic transducer 'replication' by copying the structure while
> applying reweight() to every weight. A state-number remapping 'rebuilt' maps
> original state numbers to replication state numbers; it is seeded with
> rebuilt[0] = 0 (the initial state). If original's state 0 is final, its
> final weight is reweighted (with null input/output symbols) and set on
> replication's state 0. Then it iterates states in order with an external
> 0-based counter 'source_state' and a running 'state_count' starting at 1:
> for each state, if it has no rebuilt entry yet, a new replication state is
> added, its final weight (if final) is reweighted and set, the mapping is
> recorded and state_count is incremented; the same lazy add/remap is done for
> each transition's target state. Each transition is copied to replication as
> a new transition from rebuilt[source_state] to rebuilt[target] carrying the
> same input/output symbols and a reweighted weight (reweight(w, isym, osym)).
> Finally 'trans' is replaced by a facade transducer built from 'replication'
> using trans's implementation type.

> [spec:hfst:def:hfst-reweight.parse-options-fn]
> int

> [spec:hfst:sem:hfst-reweight.parse-options-fn]
> Parses the command line. First extend_options_getenv augments argv from the
> environment. Loops over getopt_long with the common + unary long options
> plus the tool-specific options: -a/--addition=AVAL, -b/--multiplier=BVAL,
> -F/--function=FNAME, -l/--lower-bound=LVAL, -u/--upper-bound=UVAL,
> -I/--input-symbol=ISYM, -O/--output-symbol=OSYM, -S/--symbol=SYM,
> -e/--end-states-only, -A/--arcs-only, -T/--tsv=TFILE (short option string
> appends "a:b:F:l:u:I:O:S:eT:A"). Common cases then unary cases are handled
> first; the tool cases set: ADDITION = strtoweight(arg); MULTIPLIER =
> strtoweight(arg); FUNCNAME = strdup(arg) and FUNC rebound to the matching
> cmath function (erroring on an unknown name); LOWER_BOUND / UPPER_BOUND =
> strtoweight(arg); INPUT_SYMBOL / OUTPUT_SYMBOL / SYMBOL = strdup(arg);
> ENDS_ONLY / ARCS_ONLY = true; TSV_FILE_NAME = strdup(arg). Unknown options
> fall to the error case. After the loop, using both --arcs-only and
> --end-states-only is an error. Then check_common_params and
> check_unary_params run. If FUNCNAME is still unset it defaults to "id". If
> UPPER_BOUND < LOWER_BOUND a warning is issued that the reweight will never
> apply. If a TSV filename was given, the TSV file is opened for reading.
> Returns EXIT_CONTINUE on success.

> [spec:hfst:def:hfst-reweight.process-stream-fn]
> int

> [spec:hfst:sem:hfst-reweight.process-stream-fn]
> Reweights every transducer in the input stream and writes the results. While
> the input stream is good, reads the next transducer (counting them). If its
> type is FOMA, warns that weighting is unsupported and weights will be
> discarded. Fetches its name and emits a verbose "Reweighting <name>..."
> message (suffixing the 1-based index for the second and later transducers).
> If no TSV file is open, applies do_reweight once, sets the result's name to
> "reweight" and formula to "W". Otherwise rewinds the TSV file, frees and
> clears SYMBOL, resets ADDITION to 0 and MULTIPLIER to 1, and for each TSV
> line (read with getline): skips empty lines and lines starting with '#';
> requires at least one tab per line (else an error at that line number);
> takes SYM as the text before the first tab and the weight spec as the text
> from after the tab up to end-of-line; a weight spec starting with '+' sets
> ADDITION from the rest, otherwise sets MULTIPLIER from the whole spec; emits
> a verbose "Modifying weights ..." message; and applies do_reweight for that
> rule. After processing all TSV lines, sets the result's name to "reweight"
> and formula to "W". In both branches the transducer with epsilons removed is
> written to the output stream. Closes both streams and returns EXIT_SUCCESS.

> [spec:hfst:def:hfst-reweight.reweight-fn]
> static float reweight(float w, const char * i, const char * o)

> [spec:hfst:sem:hfst-reweight.reweight-fn]
> The per-weight transform applied to a single weight w. i and o are the arc's
> input/output symbols, or both null when w is a final weight. Returns w
> unchanged (no reweighting) whenever a guard fails: w lies outside the
> [lower_bound, upper_bound] window; or w is a final weight (i and o both null)
> and arcs_only is set; or w is an arc weight (i and o both non-null) and
> ends_only is set; or a single 'symbol' filter is set and neither i nor o
> equals it; or both input_symbol and output_symbol are set and i differs from
> input_symbol and o differs from output_symbol; or only input_symbol is set and
> i differs from it; or only output_symbol is set and o differs from it.
> Otherwise returns multiplier * (*func)(w) + addition — the selected per-weight
> function scaled by MULTIPLIER and offset by ADDITION.
