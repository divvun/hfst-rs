# tools/src/hfst-binary-tool.cc

> [spec:hfst:def:hfst-binary-tool.binaryoperate-streams-fn]
> int

> [spec:hfst:sem:hfst-binary-tool.binaryoperate-streams-fn]
> Drives the binary operation over a first input stream, a second input
> stream, and an output stream (signature `(firststream, secondstream,
> outstream)`). Opens all three streams (a no-op when the streams are already
> opened by their constructors). Computes `bothInputs = firststream.is_good()
> && secondstream.is_good()`. If `firststream.get_type() !=
> secondstream.get_type()`, emits `warning(0, 0, "Tranducer type mismatch in
> <firstfilename> and <secondfilename>; using former type as output\n")`.
> Initialises a transducer counter to 0, then loops while `bothInputs` holds:
> increment the counter; if the counter is 1 emit verbose `"Doing things with
> <firstfilename> and <secondfilename>...\n"`, otherwise verbose `"Doing things
> with <firstfilename> and <secondfilename>... <n>\n"` with the counter. Read
> one transducer `first` from `firststream` and one transducer `second` from
> `secondstream`; compute `first.concatenate(second)` and write the result to
> `outstream`; then recompute `bothInputs = firststream.is_good() &&
> secondstream.is_good()`. After the loop, if `firststream` is still good emit
> `warning(0, 0, "Warning: <firstfilename> contains more transducers than
> <secondfilename>; residue skipped\n")`; else if `secondstream` is still good
> emit `warning(0, 0, "Warning: <firstfilename> contains fewer transducers than
> <secondfilename>; residue skipped\n")`. Closes all three streams and returns
> `EXIT_SUCCESS`.

> [spec:hfst:def:hfst-binary-tool.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-binary-tool.main-fn]
> Program entry point. Calls `hfst_set_program_name(argv[0], "0.1",
> "HfstGenericBinaryTool")`, then `retval = parse_options(argc, argv)`; if
> `retval != EXIT_CONTINUE` returns `retval`. Closes the stdio buffers that the
> tool replaces with HFST streams: `fclose(firstfile)` unless it is stdin,
> `fclose(secondfile)` unless it is stdin, `fclose(outfile)` unless it is
> stdout. Emits verbose `"Reading from <firstfilename> and <secondfilename>,
> writing to <outfilename>\n"`. Constructs the first input stream from
> `firstfilename` (or stdin), guarded by a try/catch that on `HfstException`
> calls `error(EXIT_FAILURE, 0, "<firstfilename> is not a valid transducer
> file")`; constructs the second input stream from `secondfilename` (or stdin)
> with the analogous guard for `secondfilename`. Constructs the output stream
> from `outfilename` (or stdout) using the first stream's type as the
> implementation type. Invokes `binaryoperate_streams(firststream,
> secondstream, outstream)` (the C source's `concatenate_streams` call names
> the same routine), stores its return in `retval`, frees `firstfilename`,
> `secondfilename`, and `outfilename`, and returns `retval`.
