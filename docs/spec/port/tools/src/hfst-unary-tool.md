# tools/src/hfst-unary-tool.cc

> [spec:hfst:def:hfst-unary-tool.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-unary-tool.main-fn]
> Entry point of the unary-operation example tool. Steps:
> 1. Call hfst_set_program_name(argv[0], "0.1", "HfstUnaryTool") to register the
>    program name, version and wiki name for diagnostics and --version.
> 2. Call parse_options(argc, argv); if it returns anything other than
>    EXIT_CONTINUE, return that value immediately (an early-exit option such as
>    --help/--version was handled, or an error occurred).
> 3. Close the stdio buffers the option parser may have opened: if inputfile is
>    not stdin, fclose it; if outfile is not stdout, fclose it. (Reading/writing
>    is done through HfstInputStream/HfstOutputStream by filename instead.)
> 4. Emit a verbose message "Reading from <inputfilename>, writing to
>    <outfilename>".
> 5. Construct the input stream: if inputfile was not stdin, open
>    HfstInputStream(inputfilename), otherwise HfstInputStream() reading stdin.
>    In C++ this is wrapped in try/catch on HfstException; on failure it calls
>    error(EXIT_FAILURE, 0, "%s is not a valid transducer file", inputfilename)
>    and returns EXIT_FAILURE.
> 6. Construct the output stream of the same transducer type as the input: if
>    outfile was not stdout, HfstOutputStream(outfilename, type), otherwise
>    HfstOutputStream(type).
> 7. Call process_stream(instream, outstream) and return its result, freeing the
>    inputfilename/outfilename buffers first.

> [spec:hfst:def:hfst-unary-tool.process-stream-fn]
> int

> [spec:hfst:sem:hfst-unary-tool.process-stream-fn]
> Drive the per-transducer processing loop for the unary example operation.
> First open the input and output streams (instream.open()/outstream.open()).
> Initialise transducer_n to 0. While the input stream is_good(): increment
> transducer_n; emit a verbose message — on the first transducer
> "Doing things <inputfilename>...", on subsequent transducers
> "Doing things <inputfilename>...<transducer_n>". Read the next transducer with
> HfstTransducer(instream), apply the unary operation trans.doStuff() (the
> placeholder unary operation of this example tool) and write the result to the
> output stream via outstream << ... . When the input is exhausted, close the
> input stream then the output stream and return EXIT_SUCCESS.
