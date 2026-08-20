# tools/src/hfst-summarize.cc

> [spec:hfst:def:hfst-summarize.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-summarize.main-fn]
> Entry point of the hfst-summarize tool.
> 1. (On Windows only) set stdin to binary mode.
> 2. Call hfst_set_program_name(argv[0], "0.1", "HfstSummarize").
> 3. Call parse_options(argc, argv); if it returns anything other than
>    EXIT_CONTINUE, return that value immediately.
> 4. If inputfile is not stdin, fclose it (the streams take over from here).
> 5. verbose_printf "Reading from <inputfilename>, writing to <outfilename>".
> 6. Construct an HfstInputStream: from inputfilename when inputfile is not
>    stdin, otherwise the default (stdin) stream. In C++ this is wrapped in a
>    try/catch: on HfstException it calls error(EXIT_FAILURE, 0, "<name> is not
>    a valid transducer file") and returns EXIT_FAILURE. (The Rust constructor
>    panics rather than throwing on a bad file, so that catch arm is not
>    reproduced.)
> 7. Call process_stream(instream) and keep its return value.
> 8. If outfile is not stdout, fclose it. free(inputfilename) and
>    free(outfilename).
> 9. Return EXIT_SUCCESS (note: the process_stream return value is computed but
>    main always returns EXIT_SUCCESS).

> [spec:hfst:def:hfst-summarize.parse-options-fn]
> int

> [spec:hfst:sem:hfst-summarize.parse-options-fn]
> Parse the command line for hfst-summarize.
> 1. Call extend_options_getenv(&argc, &argv) to splice in any options from the
>    environment.
> 2. Loop calling getopt_long with the long option table = the common long
>    options, then the unary long options, then one tool-specific entry
>    {"print-symbol-pair-statistics", optional_argument, 0, 'S'}, then the
>    terminating zero entry; and the short option string = HFST_GETOPT_COMMON_SHORT
>    + HFST_GETOPT_UNARY_SHORT + "S::". Break out of the loop when getopt_long
>    returns -1.
> 3. Dispatch each returned option character through, in order: the common
>    getopt cases, the unary getopt cases, then the tool-specific case:
>    - 'S': set print_symbol_pair_statistics = true. If optarg is non-null:
>      if optarg[0] == '=', advance optarg past it; set
>      symbol_pair_threshold = hfst_strtoul(optarg, 10). If the threshold is < 0,
>      error(EXIT_FAILURE, 0, "<n> is not a valid argument for option
>      --print-symbol-pair-statistics"). If the threshold == 0,
>      error(EXIT_FAILURE, 0, "0 is not a valid argument for option
>      --print-symbol-pair-statistics").
>    - any other unrecognised character falls through to the error case.
> 4. After the loop, run the common parameter checks and the unary parameter
>    checks. Return EXIT_CONTINUE.

> [spec:hfst:def:hfst-summarize.process-stream-fn]
> int

> [spec:hfst:sem:hfst-summarize.process-stream-fn]
> Read every transducer from instream and write a properties report for each to
> outfile.
> Initialise transducer_n = 0, then while instream.is_good():
> 1. Increment transducer_n. verbose_printf "Summarizing..." for the first
>    transducer, otherwise "Summarizing... <transducer_n>".
> 2. Read the next transducer (trans) from instream and build a mutable
>    HfstBasicTransducer copy (mutt) from it.
> 3. Initialise per-transducer counters/flags: states, final_states, arcs,
>    io_epsilons, input_epsilons, output_epsilons = 0; densest_arcs = 0;
>    sparsest_arcs = 1<<31; uniq_input_arcs = uniq_output_arcs = 0;
>    most_ambiguous_input = ("", 0); most_ambiguous_output = ("", 0);
>    initial_state = 0; acceptor = input_deterministic = output_deterministic =
>    true; cyclic = cyclic_at_initial_state = false.
> 4. Try trans.get_alphabet() into transducer_alphabet, setting
>    transducer_knows_alphabet = true on success; on
>    FunctionNotImplementedException set transducer_knows_alphabet = false.
> 5. From trans.get_type() set is_mutable and weighted: SFST -> mutable,
>    unweighted; TROPICAL_OPENFST -> mutable, weighted; LOG_OPENFST -> mutable,
>    weighted; FOMA -> mutable, unweighted; HFST_OL -> not mutable, unweighted;
>    HFST_OLW -> not mutable, weighted; default -> not mutable.
> 6. Iterate the states of mutt in order (source_state starting at 0). For each
>    state s: increment states; if mutt.is_final_state(s) increment final_states;
>    set arcs_here = 0 and empty per-state input_ambiguity / output_ambiguity
>    maps. For each transition tr of the state:
>    - increment arcs and arcs_here; insert its input and output symbols into
>      found_alphabet.
>    - if print_symbol_pair_statistics, increment
>      symbol_pairs[(input_symbol, output_symbol)].
>    - if input_symbol != output_symbol, set acceptor = false.
>    - epsilon classification using hfst::is_epsilon: if both input and output
>      are epsilon, increment io_epsilons, input_epsilons, output_epsilons and
>      clear input_deterministic and output_deterministic; else if input is
>      epsilon, increment input_epsilons and clear input_deterministic; else if
>      output is epsilon, increment output_epsilons and clear
>      output_deterministic.
>    - increment input_ambiguity[input_symbol]; if it exceeds 1 clear
>      input_deterministic. Increment output_ambiguity[output_symbol]; if it
>      exceeds 1 clear output_deterministic.
>    - if this is the first state and the transition targets state 0, set cyclic
>      and cyclic_at_initial_state. If source_state equals the target state, set
>      cyclic.
>    After the transitions of a state: update densest_arcs (max arcs_here) and
>    sparsest_arcs (min arcs_here). For each entry of input_ambiguity, update
>    most_ambiguous_input if its count exceeds the stored count and increment
>    uniq_input_arcs; likewise for output_ambiguity / most_ambiguous_output /
>    uniq_output_arcs. Then advance source_state.
> 7. Compute averages: average_arcs_per_state = arcs/states;
>    average_input_epsilons = input_epsilons/states;
>    average_input_ambiguity = arcs/uniq_input_arcs;
>    average_output_ambiguity = arcs/uniq_output_arcs;
>    expected_arcs_per_symbol = average_arcs_per_state/found_alphabet.size().
> 8. Print the report to outfile: if transducer_n > 1 a "-- \nTransducer #<n>:"
>    separator; then 'name: "<trans.get_name()>"'; then the fst-type/arc-type
>    pair in OpenFST fstinfo style chosen from get_type() (SFST/SFST,
>    OpenFST/tropical, OpenFST/log, foma/foma, "HFST optimized lookup"/unweighted,
>    "HFST optimized lookup"/weighted, or ???/???); then the block of input/output
>    symbol table = yes, # of states, # of arcs, initial state, # of final
>    states, # of input/output epsilons, # of input epsilons, # of output
>    epsilons, and the four "???" accessible/coaccessible/connected/strongly conn
>    component lines; then the properties block (expanded ???, mutable,
>    acceptor, input deterministic, output deterministic, input/output label
>    sorted ???, weighted, cyclic, cyclic at initial state, topologically sorted
>    ???, accessible/coaccessible/string/minimised ???), with the boolean flags
>    rendered as "yes"/"no".
> 9. If verbose, additionally print: arcs in sparsest/densest state, average arcs
>    per state, average input epsilons per state, most ambiguous input and its
>    count, most ambiguous output and its count, average input ambiguity, average
>    output ambiguity, expected arcs per symbol, and whether mutt is infinitely
>    ambiguous. Then print the "sigma set:" line followed by the transducer
>    alphabet comma-separated (or "<Unknown in used transducer format>" if the
>    alphabet is unknown); "arc symbols actually seen in transducer:" followed by
>    found_alphabet comma-separated; "sigma symbols missing from transducer:"
>    followed by the set difference transducer_alphabet minus found_alphabet
>    comma-separated (or "<Unknown in used transducer format>"). If the type is
>    TROPICAL_OPENFST, also print "first input symbols:" followed by
>    trans.get_first_input_symbols() comma-separated.
> 10. If print_symbol_pair_statistics, print "symbol pairs that occur at most <n>
>     times:" when symbol_pair_threshold > -1 else "symbol pairs:", then for each
>     (input,output) pair in symbol_pairs whose count is <= symbol_pair_threshold,
>     print "<input>:<output>\t<count>", then a blank line.
> After the loop, print "\nRead <transducer_n> transducers in total." and return
> EXIT_SUCCESS.
