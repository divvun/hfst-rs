# tools/src/hfst-regexp2fst.cc

> [spec:hfst:def:hfst-regexp2fst.main-fn]
> int

> [spec:hfst:sem:hfst-regexp2fst.main-fn]
> Entry point for the hfst-regexp2fst tool. On Windows it sets stdout to
> binary mode. It registers the program name via hfst_set_program_name with
> version "0.2" and wiki name "Regexp2Fst", then calls parse_options(argc,
> argv). If parse_options returns anything other than EXIT_CONTINUE it returns
> that value immediately. When debug is set the xredebug toggle would be
> enabled (commented out). It reads the current value of
> hfst::get_encode_weights into 'enc'; if the --encode-weights flag was given
> it calls hfst::set_encode_weights(true). It then closes the output buffer
> (fclose(outfile)) when outfile is not stdout, since stream handling is used
> instead, and prints a verbose "Reading from <inputfilename>, writing to
> <outfilename>" message. It constructs the HfstOutputStream: with
> (outfilename, output_format) when outfile is not stdout, otherwise with just
> (output_format); the hfst-header format flag defaults to true. It calls
> process_stream on that stream. After processing, if --encode-weights was
> given it restores hfst::set_encode_weights to the saved 'enc'. Finally it
> frees inputfilename and outfilename and returns EXIT_SUCCESS.

> [spec:hfst:def:hfst-regexp2fst.parse-options-fn]
> int

> [spec:hfst:sem:hfst-regexp2fst.parse-options-fn]
> Parses command-line options. It first calls extend_options_getenv to splice
> in options from the environment. It loops calling getopt_long with the
> combined long-option table (common long options, unary long options, then
> the tool-specific options: --disjunct/'j', --epsilon/'e' (required arg),
> --line/'l', --semicolon/'S', --format/'f' (required arg),
> --do-not-harmonize/'H', --harmonize-flags/'F', --encode-weights/'E',
> --xerox-composition/'x' (required arg), --xfst/'X' (required arg),
> --do-not-minimize/'M') and the short-option string
> HFST_GETOPT_COMMON_SHORT HFST_GETOPT_UNARY_SHORT "je:lSf:HFEx:X:M". The loop
> ends when getopt_long returns -1. Each option is dispatched through the
> common-case handler (which may print usage and return), then the unary-case
> handler, then the tool's own cases: 'e' duplicates optarg into epsilonname;
> 'j' sets disjunct_expressions=true; 'S' sets line_separated=false; 'l' sets
> line_separated=true; 'f' sets output_format via hfst_parse_format_name(optarg);
> 'H' sets harmonize=false; 'F' sets harmonize_flags=true; 'E' sets
> encode_weights=true; 'M' sets minimize_result=false; 'x' reads optarg and on
> "yes"/"true"/"ON" calls hfst::set_xerox_composition(true), on
> "no"/"false"/"OFF" calls hfst::set_xerox_composition(false), otherwise errors
> with EXIT_FAILURE and returns EXIT_FAILURE; 'X' reads optarg and on
> "flag-is-epsilon" calls hfst::set_flag_is_epsilon_in_composition(true),
> otherwise errors and returns EXIT_FAILURE. Any unrecognized option falls
> through to the error case. After the loop it runs the common and unary
> parameter checks. If output_format is still UNSPECIFIED_TYPE it prints a
> verbose notice and defaults output_format to TROPICAL_OPENFST_TYPE. It
> returns EXIT_CONTINUE.

> [spec:hfst:def:hfst-regexp2fst.process-stream-fn]
> int

> [spec:hfst:sem:hfst-regexp2fst.process-stream-fn]
> Reads regular expressions and writes compiled transducers to outstream. It
> creates an XreCompiler over output_format and configures it: set_verbosity to
> the verbose flag, set_error_stream to std::cerr, set_harmonization to
> harmonize, set_flag_harmonization to harmonize_flags. It calls
> hfst::set_minimization(minimize_result) and creates an empty disjunction
> transducer of output_format. The delimiter is '\n' when line_separated else
> ';'.
>
> When NOT line_separated (semicolon mode): it reads the whole input file into
> memory with hfst_file_to_mem(inputfilename) and repeatedly calls
> comp.compile_first(filebuf_, chars_read), incrementing transducer_n and
> emitting a verbose "Compiling expression #N" each iteration. A thrown
> HfstException is reported via hfst_error (EXIT_FAILURE) naming the input file
> and expression number. If the result is NULL: when the compiler
> contained_only_comments, expression #1 being empty errors out, otherwise the
> loop breaks; if not comments-only it errors with EXIT_FAILURE. The buffer
> cursor is advanced by chars_read. For a non-NULL compiled transducer: if
> disjunct_expressions it disjuncts it into 'disjunction' with the harmonize
> flag, otherwise it sets the transducer's name to "?" with op "xre" and writes
> it to outstream; the compiled transducer is then deleted. The loop stops when
> the buffer cursor reaches the NUL terminator.
>
> When line_separated (line mode): it tracks whether the input contained only
> whitespace/comments. It loops reading delimiter-separated chunks with
> hfst_getdelim; on -1 (EOF) it errors if nothing was ever compiled, then
> breaks. The first line is saved (strdup) once. It skips leading '\n', '\r'
> and ' ' to find the expression start, increments line_count, and on an empty
> expression prints a verbose "Skipping whitespace expression #N" and
> continues. Otherwise it increments transducer_n, prints "Compiling expression
> N", and calls comp.compile(exp); a thrown HfstException is reported via
> hfst_error_at_line. A NULL result that is not comments-only errors via
> hfst_error_at_line, otherwise it continues. On success it clears the
> whitespace-only flag and, as in semicolon mode, either disjuncts into
> 'disjunction' or names the transducer "?" with op "xre" and writes it to
> outstream, then deletes the compiled transducer.
>
> After the loop, if disjunct_expressions, it sets the disjunction's name to
> "?" with op "xre" (both delimiter branches do the same) and writes the
> disjunction to outstream. It frees the line and first_line buffers and
> returns EXIT_SUCCESS.
