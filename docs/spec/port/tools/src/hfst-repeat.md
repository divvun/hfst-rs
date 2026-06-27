# tools/src/hfst-repeat.cc

> [spec:hfst:def:hfst-repeat.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-repeat.main-fn]
> Entry point of the hfst-repeat tool. On WINDOWS it sets stdin/stdout to
> binary mode (no-op on other platforms). It calls hfst_set_program_name with
> argv[0], version "0.1", and wiki name "HfstRepeat", then calls parse_options
> on argc/argv. If parse_options returns anything other than EXIT_CONTINUE,
> main returns that value immediately. Otherwise it closes the open buffered
> FILE handles: if inputfile is not stdin it fcloses inputfile, and if outfile
> is not stdout it fcloses outfile (streams are used from here on). It then
> verbose-prints "Reading from <inputfilename>, writing to <outfilename>",
> followed by a verbose line describing the repetition mode: if neither
> from_infinity nor to_infinity, "Repeating from <at_least> to <at_most>
> times"; if both, "Repeating star infinitely"; if only to_infinity,
> "Repeating from <at_least> to infinite times"; if from_infinity and not
> to_infinity, it errors out (EXIT_FAILURE) with "Repeating at least infinite
> butno more than <at_most> times?". It then constructs an HfstInputStream:
> from inputfilename when inputfile is not stdin, otherwise from stdin; if the
> constructor throws HfstException it errors with "<inputfilename> is not a
> valid transducer file" and returns EXIT_FAILURE. It builds an
> HfstOutputStream from outfilename + the input stream's type when outfile is
> not stdout, otherwise from just the input stream's type. If
> is_input_stream_in_ol_format(instream, "hfst-repeat") is true it returns
> EXIT_FAILURE. Otherwise it calls process_stream(instream, outstream), frees
> inputfilename and outfilename, and returns process_stream's result.

> [spec:hfst:def:hfst-repeat.parse-options-fn]
> int parse_options(int argc, char** argv)

> [spec:hfst:sem:hfst-repeat.parse-options-fn]
> Parses the command-line options for hfst-repeat. It first calls
> extend_options_getenv(&argc, &argv) to splice in options from the
> environment. It then loops calling getopt_long over a long-option table
> consisting of the common long options, the unary long options, plus
> {"from", required_argument, 'f'} and {"to", required_argument, 't'}, with the
> short-option string HFST_GETOPT_COMMON_SHORT + HFST_GETOPT_UNARY_SHORT +
> "f:t:". The loop ends when getopt_long returns -1. Each returned option is
> dispatched through the common getopt cases, then the unary getopt cases, then
> the tool-specific cases: case 'f' sets at_least = hfst_strtonumber(optarg,
> &from_infinity); case 't' sets at_most = hfst_strtonumber(optarg,
> &to_infinity); any unrecognised option falls through to the common error case
> (which returns EXIT_FAILURE). hfst_strtonumber parses the argument as a number
> (via strtod) and sets the referenced infinity flag to true when the value is
> an infinity. After the loop it runs the common and unary parameter checks. It
> then validates: if at_least > at_most it errors (EXIT_FAILURE) with "Cannot
> repeat from <at_least> to <at_most> times"; if from_infinity and not
> to_infinity it errors (EXIT_FAILURE) with "Cannot repeat from infinity to
> <at_most> times". On success it returns EXIT_CONTINUE.

> [spec:hfst:def:hfst-repeat.print-usage-fn]
> void print_usage()

> [spec:hfst:sem:hfst-repeat.print-usage-fn]
> Prints the tool's help text to message_out. It writes the usage line
> "Usage: <program_name> [OPTIONS...] [INFILE]" followed by "Repeat
> transducer" and a blank line. It then prints the common program options and
> the common unary program options, then the repetition options block:
> "Repetition options:" with "  -f, --from=FNUM   repeat at least FNUM times"
> and "  -t, --to=TNUM     repeat at most TNUM times". After a blank line it
> prints the common unary program parameter instructions, then the note that
> FNUM and TNUM must be positive integers or infinities as parsed by strtod(3),
> that FNUM defaults to 0 and TNUM defaults to Inf when omitted, and that FNUM
> must be less than TNUM. Finally, after a blank line, it prints the report-bugs
> footer, a blank line, and the more-info footer.

> [spec:hfst:def:hfst-repeat.process-stream-fn]
> int process_stream(HfstInputStream& instream, HfstOutputStream& outstream)

> [spec:hfst:sem:hfst-repeat.process-stream-fn]
> Reads transducers from instream one at a time and writes their repetitions to
> outstream. It keeps a 1-based counter transducer_n. While instream.is_good(),
> it increments the counter, reads one HfstTransducer from instream, and gets
> its name via hfst_get_name(trans, inputfilename). For the first transducer it
> emits a verbose line describing the mode: neither infinity -> "Repeating
> [<at_least>..<at_most>] <name>..."; both infinities -> "Repeating star
> <name>..."; only to_infinity -> "Repeating [<at_least>..*] <name>...". For
> subsequent transducers it emits the same messages with the counter appended
> (" <transducer_n>"). It then applies the repetition: if neither infinity, it
> calls trans.repeat_n_to_k(at_least, at_most), sets the transducer name to
> hfst_set_name(trans, trans, "repeat-<at_least>-to-<at_most>") and the formula
> to hfst_set_formula(trans, trans, "_<at_least>^<at_most>"); if both
> infinities, it calls trans.repeat_star() and sets name "repeat-star" and
> formula "⋆"; if only to_infinity, it calls trans.repeat_n_plus(at_least)
> and sets name "repeat-<at_least>-plus" and formula "_<at_least>^∞"; the
> from_infinity-and-not-to_infinity branch errors (EXIT_FAILURE) — though
> parse_options has already rejected that combination. It writes the resulting
> transducer to outstream and frees the inputname. After the loop it closes
> instream and outstream and returns EXIT_SUCCESS.
