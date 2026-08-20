# tools/src/hfst-realign.cc

> [spec:hfst:def:hfst-realign.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-realign.main-fn]
> Entry point of the hfst-realign tool. On Windows it sets stdin/stdout to
> binary mode (not applicable on other platforms). It calls
> hfst_set_program_name(argv[0], "0.1", "HfstRealign"), then parse_options(argc,
> argv). If parse_options returns anything other than EXIT_CONTINUE, main
> returns that value immediately. Otherwise it flushes the line-buffer files:
> if inputfile is not stdin it fcloses inputfile, and if outfile is not stdout
> it fcloses outfile. It emits the verbose message
> "Reading from <inputfilename>, writing to <outfilename>\n". It then opens an
> HfstInputStream: from inputfilename when inputfile is not stdin, otherwise the
> default (stdin) stream; in C a failed open throws HfstException and is caught,
> reporting "<inputfilename> is not a valid transducer file" via error(EXIT_FAILURE,0,..)
> and returning EXIT_FAILURE. It opens an HfstOutputStream using the input
> stream's transducer type: to outfilename when outfile is not stdout, otherwise
> to stdout. If is_input_stream_in_ol_format(instream, "hfst-realign") is true it
> returns EXIT_FAILURE. Otherwise it calls process_stream(instream, outstream),
> frees inputfilename and outfilename, and returns that result.

> [spec:hfst:def:hfst-realign.parse-options-fn]
> int

> [spec:hfst:sem:hfst-realign.parse-options-fn]
> Parses the command-line options. It first calls extend_options_getenv(&argc,
> &argv) to splice in options from the environment. It then loops calling
> getopt_long with the concatenation of the common long options, the unary long
> options, and one tool-specific long option {"boundary", required_argument, 0,
> 'b'} (terminated by a {0,0,0,0} sentinel), and the short-option string
> HFST_GETOPT_COMMON_SHORT + HFST_GETOPT_UNARY_SHORT + "b:". The loop terminates
> when getopt_long returns -1. Each returned option character is dispatched
> through the common getopt cases, then the unary getopt cases, then the tool's
> own arm, then the error case. The tool's own arm matches case 'p' and simply
> resets boundary_symbol to its default '>' (note: although the long option
> registers 'b', the switch labels the tool arm 'p'). After the loop it runs the
> common parameter checks and the unary parameter checks, then returns
> EXIT_CONTINUE.

> [spec:hfst:def:hfst-realign.process-stream-fn]
> int

> [spec:hfst:sem:hfst-realign.process-stream-fn]
> Reads every transducer from instream and writes the realigned transducer to
> outstream. It keeps a 1-based counter transducer_n initialised to 0. While
> instream.is_good(): increment transducer_n; read one HfstTransducer from
> instream; get its name via hfst_get_name(trans, inputfilename). Emit a verbose
> message depending on the counter and boundary_symbol: when boundary_symbol is
> non-zero "Pushing towards start <name>...", else "Pushing towards end
> <name>..."; for transducer_n other than 1 the message additionally appends
> "... <transducer_n>". Realign the transducer by: trans.invert();
> trans.push_labels(TO_INITIAL_STATE); trans.invert();
> trans.push_labels(TO_INITIAL_STATE). Set its name with hfst_set_name(trans,
> trans, "realign") and its formula with hfst_set_formula(trans, trans, "Id").
> Write it with outstream << trans, and free the inputname string. After the
> loop, close instream and outstream and return EXIT_SUCCESS.
