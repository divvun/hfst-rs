# tools/src/hfst-expand-equivalences.cc

> [spec:hfst:def:hfst-expand-equivalences.add-extension-fn]
> static

> [spec:hfst:sem:hfst-expand-equivalences.add-extension-fn]
> Adds one equivalence-class extension to the running 'extensions'
> transducer t. Prints "extending FROM by TO\n" via verbose_printf.
> Constructs a one-symbol-pair transducer 'remap' mapping the single
> input symbol FROM to the single output symbol TO, using t's
> implementation type. Then disjuncts (unions) 'remap' into t in place
> (with harmonisation enabled), so t accumulates the alternation of the
> identity and every FROM:TO mapping seen so far.

> [spec:hfst:def:hfst-expand-equivalences.check-options-fn]
> static

> [spec:hfst:sem:hfst-expand-equivalences.check-options-fn]
> Validates the mutually exclusive extension-source options after option
> parsing. If either only_from_label or only_to_label was given: it is an
> error (EXIT_FAILURE) to also give tsv_file_name or acx_file_name; it is
> an error if only_from_label is unset (-t requires -f); it is an error if
> only_to_label is unset (-f requires -t). Otherwise, if neither
> tsv_file_name nor acx_file_name was given, error "Must give extension
> specification file with either -a or -t.". Otherwise, if both
> tsv_file_name and acx_file_name were given, error "Only one of
> parameters -a, -t, must be used.". Otherwise, if tsv_file_name is set,
> open it for reading into tsv_file via hfst_fopen. Otherwise, if
> acx_file_name is set, open it for reading into acx_file. Otherwise error
> "Logic error again!". Every error exits the process.

> [spec:hfst:def:hfst-expand-equivalences.fsa-level-t]
> enum fsa_level_t {
>   FSA_LEVEL_FIRST;
>   FSA_LEVEL_SECOND;
>   FSA_LEVEL_BOTH;
> }

> [spec:hfst:sem:hfst-expand-equivalences.fsa-level-t]
> Enumeration of which side(s) of the transducer the extensions are
> applied to: FSA_LEVEL_FIRST (input/upper side), FSA_LEVEL_SECOND
> (output/lower side), FSA_LEVEL_BOTH (both sides). The module-level
> 'level' variable defaults to FSA_LEVEL_FIRST.

> [spec:hfst:def:hfst-expand-equivalences.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-expand-equivalences.main-fn]
> Program entry point. Sets the program name to "HfstExpandEquivalences"
> (version "0.1"). Calls parse_options; if it returns anything other than
> EXIT_CONTINUE, returns that value. Calls check_options to validate and
> open the extension-source file. Closes the inputfile buffer if it is not
> stdin and the outfile buffer if it is not stdout (streams are used from
> here on). Logs "Reading from INFILE, writing to OUTFILE". Opens an
> HfstInputStream on inputfilename (or stdin); on a thrown HfstException
> the C++ errors "X is not a valid transducer file" and returns
> EXIT_FAILURE. Opens an HfstOutputStream on outfilename (or stdout) using
> the input stream's transducer type. If the input stream is in
> optimized-lookup format (is_input_stream_in_ol_format), returns
> EXIT_FAILURE. Otherwise calls process_stream, then returns EXIT_SUCCESS.

> [spec:hfst:def:hfst-expand-equivalences.parse-options-fn]
> int

> [spec:hfst:sem:hfst-expand-equivalences.parse-options-fn]
> Parses the command line with getopt_long over the common long options,
> the unary long options, plus this tool's options: --from/-f (ISYM,
> required arg), --to/-t (OSYM, required arg), --acx/-a (ACXFILE, required
> arg), --tsv/-T (TSVFILE, required arg), --level/-l (LEVEL, required
> arg). The short option string is the common short options, the unary
> short options, then "f:t:a:T:l:". The common getopt cases are handled
> first. Tool cases: -f copies optarg (hfst_strdup) into only_from_label;
> -t into only_to_label; -a into acx_file_name; -T into tsv_file_name; -l
> sets level: "first"/"upper"/"input"/"1" -> FSA_LEVEL_FIRST,
> "second"/"lower"/"output"/"2" -> FSA_LEVEL_SECOND, "both" ->
> FSA_LEVEL_BOTH, anything else errors (EXIT_FAILURE) with a message
> listing the valid level names. Unrecognised options fall to the error
> case. After the loop, runs the common and unary parameter checks and
> returns EXIT_CONTINUE.

> [spec:hfst:def:hfst-expand-equivalences.process-stream-fn]
> static

> [spec:hfst:sem:hfst-expand-equivalences.process-stream-fn]
> Reads every transducer from instream in turn. For each one: read the
> transducer 'trans'; build an 'extensions' transducer initialised to the
> identity-to-identity single pair (internal_identity:internal_identity)
> in trans's type. Then populate extensions from the chosen source:
>   - If only_from_label is set (single command-line extension): logs the
>     pair and calls add_extension(extensions, only_from_label,
>     only_to_label).
>   - Else if tsv_file is open: read it line by line. Skip blank lines
>     (line starting with newline). For each line, locate the first tab;
>     if there is no tab, a line starting with '#' is a comment and is
>     skipped, otherwise it is an error ("At least one tab required per
>     line"). The text before the first tab is the FROM field; an empty
>     FROM field is an error advising the use of @0@ or internal_epsilon.
>     After the first tab, each subsequent tab-delimited field up to (but
>     not including) the last is a TO value: for each, an empty field is an
>     error, otherwise call add_extension(extensions, from, to). Finally
>     the trailing field (from after the last tab up to end-of-line or
>     NUL) is also a TO value, again erroring if empty, otherwise
>     add_extension(extensions, from, to). All errors use error_at_line
>     with the tsv filename and 1-based line number and exit.
>   - Else if acx_file is open: logs "Reading ACX from FILE..."; the actual
>     libxml-based parsing of the analysis-chars/char/equiv-char tree is
>     compiled only when HAVE_LIBXML_TREE_H is defined, calling
>     add_extension for each char value paired with each equiv-char value.
>     Without libxml the body is empty.
>   - Else error "DANGER TERROR HORROR !!!!!!".
> Then normalise extensions: minimize, repeat_star (Kleene star),
> minimize. Apply to trans according to level: FSA_LEVEL_BOTH composes
> trans with extensions (second level), then sets trans to the inverse of
> extensions composed with trans (first level); FSA_LEVEL_FIRST sets trans
> to inverse-extensions composed with trans; FSA_LEVEL_SECOND composes
> trans with extensions. Writes the resulting trans to outstream and frees
> the per-transducer extensions transducer.
