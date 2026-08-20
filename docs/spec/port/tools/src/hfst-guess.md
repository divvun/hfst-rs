# tools/src/hfst-guess.cc

> [spec:hfst:def:hfst-guess.get-float-fn]
> float get_float(const std::string &str)

> [spec:hfst:sem:hfst-guess.get-float-fn]
> Parse a floating-point number out of the given string using the same
> semantics as C++ istringstream extraction into a float: skip leading
> whitespace, then consume the longest leading run of characters that forms a
> valid float. If no float can be extracted (the stream's failbit would be
> set), return -1. Otherwise return the parsed float value. Used to validate
> the --generate-threshold argument; callers treat a negative result as the
> "invalid input" sentinel.

> [spec:hfst:def:hfst-guess.get-size-t-fn]
> size_t get_size_t(const std::string &str)

> [spec:hfst:sem:hfst-guess.get-size-t-fn]
> Parse a non-negative integer out of the given string using the same
> semantics as C++ istringstream extraction into a size_t: skip leading
> whitespace, then consume the leading run of decimal digits. If extraction
> fails (no digits, failbit set), signal failure by throwing the C string
> "fail" (in the Rust port: return Err). Otherwise return the parsed size_t.
> Used to validate the --max-number-of-guesses and --max-number-of-forms
> arguments; a thrown/Err result is reported as invalid input.

> [spec:hfst:def:hfst-guess.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-guess.main-fn]
> Program entry point. On Windows set stdin/stdout to binary mode. Set the
> program name/version/wikiname to ("hfst-guess", "0.3", "HfstGuess"), then
> call parse_options; if it returns anything other than EXIT_CONTINUE, return
> that value. Close the input FILE buffer if it is not stdin (streams are used
> from here on). Emit a verbose "Reading from <inputfilename>, writing to
> <outfilename>" message.
>
> Open an HfstInputStream on inputfilename (or stdin if no input file); on a
> read/construct failure the C reports "<inputfilename> is not a valid
> transducer file" and returns EXIT_FAILURE. Open the output as an ofstream on
> outfilename (or std::cout if stdout); on failure report "<outfilename>
> cannot be opened for writing." and return EXIT_FAILURE.
>
> Read the first transducer from the input stream as the guesser; on failure
> report "Error when reading guesser from file <inputfilename>" and return
> EXIT_FAILURE. Verify the guesser is actually a guesser by checking that its
> properties contain exactly one entry named "reverse input"; otherwise report
> "The transducer in <inputfilename> is not a guesser." and return
> EXIT_FAILURE.
>
> If model-form generation is requested (-f given): if the input stream has no
> further transducer, emit verbose "No generator found in <inputfilename>.
> Compiling generator from guesser." and build the generator from a copy of the
> guesser by converting it to TROPICAL_OPENFST_TYPE, inverting it, then
> converting to HFST_OLW_TYPE; otherwise read the next transducer from the
> stream as the generator.
>
> Build a tokenizer that knows the guesser's multi-character alphabet symbols
> via get_alphabet_string_tokenizer(guesser). If generating model forms, emit
> verbose "Reading inflectional information for model forms from
> <model_form_filename>." and read the model forms from that file with the
> tokenizer; on an InvalidModelLine error print "Invalid model form line in
> model form file:" then the offending line to stderr and return EXIT_FAILURE.
>
> Then loop over each line of standard input (std::getline, until EOF). For each
> input line compute its guesses via get_guesses(line, guesser,
> max_number_of_guesses, tokenizer). If generating model forms: assert the
> generator is non-null (throwing "Error: generator has a NULL value." if it is
> — should never happen), compute paradigms via get_paradigms(line, guesses,
> generator, model_forms, max_number_of_forms, generate_threshold), and write
> each paradigm StringVector to the output stream followed by a newline. Else:
> for each guess StringVector reverse its element order in place, then write
> "<line>\t<guess>" followed by a newline, where a StringVector prints as the
> concatenation of its symbols with no separator. After each input line, write a
> blank line to the output.
>
> Finally, if the output is a file, delete/flush it; free inputfilename and
> outfilename; delete the guesser and generator; return EXIT_SUCCESS.

> [spec:hfst:def:hfst-guess.parse-options-fn]
> int parse_options(int argc, char** argv)

> [spec:hfst:sem:hfst-guess.parse-options-fn]
> Standard HFST option-parsing loop for a unary tool. First call
> extend_options_getenv to splice in options from the environment. Then loop
> calling getopt_long with the common long options, the unary long options, and
> the tool-specific long options ("generate-threshold" -> 'g',
> "model-form-filename" -> 'f', "max-number-of-guesses" -> 'n',
> "max-number-of-forms" -> 'm', each taking a required argument) terminated by a
> zero entry; the short-option string is the common short options, then the
> unary short options, then "f:m:n:g:". Exit the loop when getopt_long returns
> -1.
>
> Dispatch each returned option character through the standard case groups in
> order: the common cases (which handle --help/--version/--verbose/etc and may
> return a status), the unary cases, then the tool-specific cases:
> - 'f': set generate_model_forms = true and model_form_filename = optarg.
> - 'g': set generate_threshold = get_float(optarg); if it is negative, call
>   error(EXIT_FAILURE, 0, "Invalid generate threshold <optarg>. Give a
>   positive float.").
> - 'n': set max_number_of_guesses = get_size_t(optarg); if get_size_t throws,
>   call error(EXIT_FAILURE, 0, "Invalid maximal number of guesses <optarg>.
>   Give a positive int.").
> - 'm': set max_number_of_forms = get_size_t(optarg); if get_size_t throws,
>   call error(EXIT_FAILURE, 0, "Invalid maximal number of generated forms
>   <optarg>. Give a positive int.").
> Any unrecognised option falls through to the standard error case.
> After the loop, run the common and unary parameter checks
> (check-params-common, check-params-unary) and return EXIT_CONTINUE.
