# tools/src/hfst-pmatch2fst.cc

> [spec:hfst:def:hfst-pmatch2fst.get-current-dir-name-fn]
> char *

> [spec:hfst:sem:hfst-pmatch2fst.get-current-dir-name-fn]
> Fallback implementation of POSIX 'get_current_dir_name', compiled only when
> the platform lacks it. Returns a freshly allocated string holding the absolute
> pathname of the current working directory. It repeatedly calls 'getcwd' into a
> growing heap buffer (starting at 1024 bytes, doubling on ERANGE up to an 8x
> sanity cap); on success it returns the buffer. If 'getcwd' fails with EACCES it
> throws a runtime error ("Unable to access working directory"); on any other
> error it returns an empty string. The Rust port delegates to the standard
> library's current-directory query and returns the empty string on any failure,
> matching the C++ fallback path.

> [spec:hfst:def:hfst-pmatch2fst.main-fn]
> int

> [spec:hfst:sem:hfst-pmatch2fst.main-fn]
> Program entry point. On Windows, sets stdout to binary mode. Sets the program
> name/version/wikiname via 'hfst_set_program_name' (version "0.1", wikiname
> "Pmatch2Fst"). Calls 'parse_options'; if it returns anything other than
> EXIT_CONTINUE, returns that value. Otherwise, if the output file is not stdout,
> closes the output FILE buffer (the streams take over from here). Emits a
> verbose message naming the input and output files. Constructs an
> 'HfstOutputStream' of type HFST_OLW_TYPE — to a named file when an output file
> was given, else to standard output. Calls 'process_stream' on it, frees the
> input/output filename strings, and returns EXIT_SUCCESS.

> [spec:hfst:def:hfst-pmatch2fst.parse-options-fn]
> int

> [spec:hfst:sem:hfst-pmatch2fst.parse-options-fn]
> Parses the command line. First calls 'extend_options_getenv' to splice in any
> options from the environment. Then loops calling 'getopt_long' with the common
> and unary option tables plus three tool-specific options: '-e'/'--epsilon'
> (required argument), '--flatten' (no argument, internal val '1'), and
> '--cosine-distances' (no argument, internal val '2'); the short-option string
> is the common + unary short options followed by "e:". For each returned option
> it dispatches through the common cases, then the unary cases, then its own:
> '-e' copies the argument into 'epsilonname' (via 'hfst_strdup'), '--flatten'
> sets the 'flatten' flag, '--cosine-distances' sets the
> 'include_cosine_distances' flag; an unrecognised option falls to the error
> case. After the loop it runs the common and unary parameter checks and returns
> EXIT_CONTINUE.

> [spec:hfst:def:hfst-pmatch2fst.print-usage-fn]
> void

> [spec:hfst:sem:hfst-pmatch2fst.print-usage-fn]
> Prints the help text to 'message_out': a usage line "Usage: PROGRAM
> [OPTIONS...] [INFILE]" with the heading "Compile regular expressions into
> transducer(s) (Experimental version)"; the common and common-unary program
> options; then the tool's "String and format options" block documenting
> '-e/--epsilon=EPS' (map EPS as zero), '--flatten' (compile in all RTNs), and
> '--cosine-distances' (include cosine distance info when compiling Like()
> operations); then a note that missing/'-' OUTFILE/INFILE use the standard
> streams, that the default 0 is used when EPS is undefined, and that weights are
> not implemented; then an Examples section showing a Define-TOP pmatch script
> piped into the program; and finally the bug-report and more-info footers.

> [spec:hfst:def:hfst-pmatch2fst.process-stream-fn]
> int

> [spec:hfst:sem:hfst-pmatch2fst.process-stream-fn]
> Compiles the pmatch rule set read from the input file and writes the resulting
> transducers to 'outstream'. Steps:
> 1. Construct a 'PmatchCompiler' for the compilation format and propagate the
>    'verbose', 'flatten', and 'include_cosine_distances' flags into it.
> 2. Determine an include directory: when reading from a real file (not stdin)
>    with a non-empty filename, resolve the filename to an absolute path (it is
>    used directly if it begins with '/', else it is prefixed with the current
>    working directory) and strip the trailing path component after the last '/';
>    if there is no '/', the include directory is left empty. Set this on the
>    compiler via 'set_include_path'.
> 3. Read the entire input file byte-by-byte into a string. If it holds more than
>    one byte, call 'compile' to obtain a map from definition name to compiled
>    'HfstTransducer'. (The C++ catches HfstException, printing its name and
>    returning EXIT_FAILURE.)
> 4. Build a harmonizer: a transducer whose alphabet is the union of every
>    compiled transducer's alphabet (iterating definitions in key order,
>    inserting each not-yet-seen symbol). If no symbols were seen, print
>    "PROGRAM: Empty ruleset, nothing to write" to stderr and return
>    EXIT_FAILURE. Convert the harmonizer to HFST_OLW_TYPE.
> 5. If a definition named "TOP" exists, output it first: capture its properties,
>    convert it through HfstBasicTransducer to hfst-ol (weighted, no special
>    options) harmonized against the harmonizer, convert back to an
>    HfstTransducer, name it "TOP", re-apply its captured properties, and write
>    it to the output stream; then free it and erase it from the map. Then for
>    each remaining definition (in key order): convert it through
>    HfstBasicTransducer to hfst-ol harmonized against the harmonizer — using the
>    "empty_alphabet" option for ordinary RTNs, or no option for definitions
>    whose name contains "UNCOMPOSE" — convert back, name it after its key, and
>    write it out. Verbose mode prints per-step timing.
> 6. If there was no "TOP" definition, print "PROGRAM: Empty ruleset, nothing to
>    write" to stderr and return EXIT_FAILURE.
> 7. Close the output stream and return EXIT_SUCCESS.
