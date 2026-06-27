# tools/src/hfst-kill-paths.cc

> [spec:hfst:def:hfst-kill-paths.main-fn]
> int

> [spec:hfst:sem:hfst-kill-paths.main-fn]
> Entry point. On Windows, sets stdin/stdout to binary mode (not modelled in
> the port). Calls hfst_set_program_name(argv[0], "0.1", "HfstKillPaths"),
> then parse_options(argc, argv); if its return value is not EXIT_CONTINUE,
> returns that value immediately. Closes the input buffer file (fclose) if the
> input is not stdin and the output buffer file if the output is not stdout —
> the tool reads via streams, not the raw buffers. Emits the verbose messages
> "Reading from INPUTFILENAME, writing to OUTFILENAME" and "Killing paths"; if
> a single --symbol was set, also "only if arc has symbol SYM". Opens an
> HfstInputStream: from inputfilename when the input is a named file, otherwise
> from stdin (the C wraps the constructor in try/catch and, on HfstException,
> calls error(EXIT_FAILURE, 0, "<file> is not a valid transducer file") and
> returns EXIT_FAILURE; the Rust constructor panics instead). Creates an
> HfstOutputStream of the input stream's type — to outfilename when the output
> is a named file, otherwise to stdout. Then, if the input stream is in
> optimized-lookup format (is_input_stream_in_ol_format(stream,
> "hfst-kill-paths")), returns EXIT_FAILURE. Otherwise calls
> process_stream(instream, outstream), frees inputfilename and outfilename, and
> returns its result.

> [spec:hfst:def:hfst-kill-paths.original-fn]
> HfstBasicTransducer original(trans)

> [spec:hfst:sem:hfst-kill-paths.original-fn]
> do_killing(trans): rebuild the transducer dropping every arc that touches the
> current symbol, and replace trans in place with the result. Build an
> HfstBasicTransducer 'original' from trans, and an empty 'replication'.
> Maintain state_count = 1, a map 'rebuilt' from original state numbers to
> replication state numbers, seeded with rebuilt[0] = 0 (both graphs start with
> state 0). If state 0 is final in original, copy its final weight onto state 0
> of replication. Iterate the states of original in ascending state order,
> tracking source_state (starting at 0, incremented after each state). For each
> source_state: if it is not yet in rebuilt, add a new state state_count to
> replication, copy the final weight if source_state is final, record
> rebuilt[source_state] = state_count, and increment state_count. Then for each
> outgoing arc of that state: if the arc's input symbol equals the current
> symbol OR its output symbol equals the current symbol, skip it entirely (the
> arc, and hence every path through it, is removed). Otherwise ensure the arc's
> target state is mapped — if the target is not yet in rebuilt, add state
> state_count, copy its final weight if final, record the mapping, increment
> state_count — and add to replication, from rebuilt[source_state], a new
> transition to rebuilt[target] carrying the same input symbol, output symbol,
> and weight. After processing all states, set trans = HfstTransducer(replication,
> trans.get_type()) (its symbols added to the alphabet).

> [spec:hfst:def:hfst-kill-paths.parse-options-fn]
> int

> [spec:hfst:sem:hfst-kill-paths.parse-options-fn]
> Parses command-line options. First calls extend_options_getenv(&argc, &argv)
> to splice in any options from the environment. Loops over getopt_long with the
> long-option table = common long options + unary long options + the
> tool-specific { "symbol", required_argument, 'S' } and { "tsv",
> required_argument, 'T' } (plus the terminating zero entry), and the short
> string HFST_GETOPT_COMMON_SHORT + HFST_GETOPT_UNARY_SHORT + "a:b:F:l:u:I:O:S:eT:A"
> (the trailing literal is carried verbatim from the source). For each returned
> option: the common cases are handled first, then the unary cases, then the
> tool's own: 'S' copies optarg into 'symbol' (hfst_strdup) and 'T' copies optarg
> into 'tsv_file_name'; unrecognised options fall through to the common error
> case. After the loop, if neither 'symbol' nor 'tsv_file_name' was set, calls
> error(EXIT_FAILURE, 0, "Either --symbol or --tsv-file is required") and returns
> EXIT_FAILURE. Otherwise runs the common parameter checks and the unary
> parameter checks, then — if a tsv_file_name was given — opens it for reading
> via hfst_fopen(tsv_file_name, "r") into 'tsv_file'. Returns EXIT_CONTINUE.

> [spec:hfst:def:hfst-kill-paths.print-usage-fn]
> void

> [spec:hfst:sem:hfst-kill-paths.print-usage-fn]
> Prints the help text to message_out. Emits the usage line "Usage: PROGRAM
> [OPTIONS...] [INFILE]" followed by "Kill all paths with specific symbols" and a
> blank line. Then prints the common program options, the common unary program
> options, and the tool-specific "Reweighting options:" block documenting
> -S/--symbol=SYM ("remove arcs with input or output symbol SYM or both") and
> -T/--tsv-file=TFILE ("read kill rules from TFILE"). Prints a blank line, the
> common unary program parameter instructions, the note that TFILE should contain
> tab-separated lines (comment lines starting with # and empty lines are ignored),
> another blank line, the report-bugs blurb, a blank line, and the more-info blurb.

> [spec:hfst:def:hfst-kill-paths.process-stream-fn]
> int

> [spec:hfst:sem:hfst-kill-paths.process-stream-fn]
> Processes every transducer in the input stream. Keeps a 1-based counter
> transducer_n. While the input stream is good: read the next HfstTransducer
> 'trans', take its display name via hfst_get_name(trans, inputfilename), and emit
> the verbose message "Path killing NAME...\n" for the first transducer or "Path
> killing NAME...N\n" thereafter. Then, if no tsv_file was opened, call
> do_killing(trans) once (using the single --symbol) and set the transducer's name
> to "pathkill(...)" via hfst_set_name(trans, trans, "pathkill") and its formula
> to "PK" via hfst_set_formula(trans, trans, "PK"). Otherwise (tsv_file given):
> rewind tsv_file, free the previous symbol, and emit "Reading reweights from
> TSVNAME". Read the file line by line with getline; for each line, increment a
> line counter, skip lines that begin with '\n' (empty) or '#' (comment), find the
> end of the symbol token by scanning to the first '\0' or '\n', set symbol to that
> leading substring (hfst_strndup), emit "Killing patsh with symbol SYM", and call
> do_killing(trans) — so each tsv symbol is killed in turn. After the file is
> exhausted, free the line buffer and set name "pathkill" and formula "PK" as
> above. In both branches, write trans.remove_epsilons() to the output stream and
> free the input name. After the loop, close the input and output streams and
> return EXIT_SUCCESS.
