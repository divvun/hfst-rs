# tools/src/hfst-multiply.cc

> [spec:hfst:def:hfst-multiply.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-multiply.main-fn]
> Entry point of the hfst-multiply (HfstDuplicate) tool. On Windows it first
> sets stdin and stdout to binary mode. It registers the program name via
> hfst_set_program_name(argv[0], "0.1", "HfstDuplicate"), then calls
> parse_options(argc, argv). If parse_options returns anything other than
> EXIT_CONTINUE, main returns that value immediately. Otherwise it flushes the
> command-line buffers it no longer needs: if inputfile is not stdin it fcloses
> inputfile, and if outfile is not stdout it fcloses outfile. It then emits a
> verbose message "Reading from <inputfilename>, writing to <outfilename>".
> It opens an HfstInputStream: from inputfilename when inputfile is not stdin,
> otherwise the default (stdin) stream; if the constructor throws HfstException
> it reports the error "<inputfilename> is not a valid transducer file" with
> status EXIT_FAILURE and returns EXIT_FAILURE. It opens an HfstOutputStream
> using the input stream's transducer type: to outfilename when outfile is not
> stdout, otherwise to stdout. It calls process_stream(instream, outstream),
> frees inputfilename and outfilename, and returns process_stream's value.

> [spec:hfst:def:hfst-multiply.process-stream-fn]
> int

> [spec:hfst:sem:hfst-multiply.process-stream-fn]
> Reads transducers from instream and writes each one to outstream dupe_count
> times. It keeps a counter transducer_n starting at 0. While instream is good,
> it increments transducer_n, reads one HfstTransducer from instream, and
> determines its name: the transducer's own name, or, if that is empty, the
> inputfilename. It emits a verbose message
> "Duplicate <name> times <dupe_count>...<transducer_n>", then writes the
> transducer to outstream dupe_count times (each iteration of i in
> [0, dupe_count) does outstream << trans). After the loop it closes instream
> and outstream and returns EXIT_SUCCESS. (The source also declares an unused
> queue<HfstTransducer> last_n that has no effect.)
