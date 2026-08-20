# tools/src/hfst-determinize.cc

> [spec:hfst:def:hfst-determinize.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-determinize.main-fn]
> Entry point of the hfst-determinize tool. On Windows, sets stdin and
> stdout to binary mode. Calls hfst_set_program_name with argv[0], version
> "0.1", and wiki name "HfstDeterminize". Calls parse_options(argc, argv);
> if its return value is not EXIT_CONTINUE, returns that value. Otherwise,
> closes the buffered FILE handles: if inputfile is not stdin, fclose it; if
> outfile is not stdout, fclose it. Saves the current value of
> hfst::get_encode_weights() into enc; if the tool's encode_weights flag is
> set, calls hfst::set_encode_weights(true). Emits a verbose message
> "Reading from <inputfilename>, writing to <outfilename>". Opens the input
> stream: if inputfile is not stdin, construct HfstInputStream(inputfilename),
> else HfstInputStream(); if the constructor throws HfstException, report the
> error "<inputfilename> is not a valid transducer file" with EXIT_FAILURE
> and return EXIT_FAILURE. Opens the output stream from the input stream's
> type: if outfile is not stdout, HfstOutputStream(outfilename, type), else
> HfstOutputStream(type). If is_input_stream_in_ol_format(*instream,
> "hfst-determinize") is true, return EXIT_FAILURE. Calls
> process_stream(*instream, *outstream) and saves the result in retval. If
> the tool's encode_weights flag was set, restores hfst::set_encode_weights(enc).
> Frees inputfilename and outfilename, and returns retval.

> [spec:hfst:def:hfst-determinize.process-stream-fn]
> int

> [spec:hfst:sem:hfst-determinize.process-stream-fn]
> Reads transducers from instream, determinizes each, and writes them to
> outstream. Maintains a counter transducer_n starting at 0. While
> instream.is_good(): increments transducer_n; constructs a HfstTransducer
> from instream; obtains its name via hfst_get_name(trans, inputfilename).
> If this is the first transducer (transducer_n == 1) emits a verbose message
> "Determinizing <inputname>...", otherwise "Determinizing <inputname>...<n>"
> with the transducer index. Calls trans.determinize(), then sets the
> transducer's name to "determinize" via hfst_set_name(trans, trans,
> "determinize") and its formula to the U+2336 (APL functional symbol
> i-beam, "⌶") character via hfst_set_formula(trans, trans, "⌶"). Writes the
> transducer to outstream (outstream << trans) and frees inputname. After the
> loop, closes instream and outstream and returns EXIT_SUCCESS.
