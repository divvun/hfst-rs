# tools/src/hfst-prune-alphabet.cc

> [spec:hfst:def:hfst-prune-alphabet.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-prune-alphabet.main-fn]
> Program entry point. On Windows sets stdin/stdout to binary mode. Calls
> hfst_set_program_name(argv[0], "0.1", "HfstPruneAlphabet"). Calls
> parse_options(argc, argv); if it returns anything other than EXIT_CONTINUE,
> returns that value immediately. Closes the input FILE buffer with fclose
> unless it is stdin, and the output FILE buffer unless it is stdout (the tool
> works with streams from here on). Emits a verbose message "Reading from
> <inputfilename>, writing to <outfilename>". Constructs an HfstInputStream:
> from inputfilename when input is a named file, otherwise from stdin; in C the
> construction is wrapped in try/catch that reports "<inputfilename> is not a
> valid transducer file" and returns EXIT_FAILURE on HfstException. Constructs
> an HfstOutputStream: with (outfilename, instream type) for a named output
> file, otherwise with the instream type. Then if is_input_stream_in_ol_format
> reports the stream is in optimized-lookup format (for tool
> "hfst-prune-alphabet"), returns EXIT_FAILURE. Otherwise calls
> process_stream(instream, outstream), frees inputfilename and outfilename, and
> returns its result.

> [spec:hfst:def:hfst-prune-alphabet.process-stream-fn]
> int

> [spec:hfst:sem:hfst-prune-alphabet.process-stream-fn]
> Processes every transducer in instream. Maintains a 1-based counter
> transducer_n. While instream.is_good(): increments the counter, reads the next
> HfstTransducer from instream, and obtains its name via
> hfst_get_name(trans, inputfilename). For the first transducer emits the verbose
> message "Pruning <name>...", and for any subsequent transducer emits
> "Pruning <name>... <transducer_n>". Prunes the transducer's alphabet by calling
> trans.prune_alphabet(force_pruning): when force_pruning is false (the default
> "safe" mode) the library only prunes if no unknown or identity symbols are used.
> Sets the result's name with hfst_set_name(trans, trans, "prune-alphabet") (the
> transducer-source overload, which wraps the existing name as
> "prune-alphabet(<name>)", or "prune-alphabet(UNNAMED)" if unnamed). Writes the
> transducer to outstream and frees the name string. After the loop closes both
> instream and outstream and returns EXIT_SUCCESS.
