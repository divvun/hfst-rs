# tools/src/hfst-pair-test.cc

> [spec:hfst:def:hfst-pair-test.backslash-escape-fn]
> std::string backslash_escape(std::string perc_escaped)

> [spec:hfst:sem:hfst-pair-test.backslash-escape-fn]
> Converts a string whose special characters are percent-escaped into one
> whose special characters are backslash-escaped. Operating in place on a
> mutable copy of the argument, it performs five ordered substring
> replacements (each replacing every occurrence): replace "%%" with the
> sentinel "PAIR_TEST_PERC_PERC"; replace "%:" with the sentinel
> "PAIR_TEST_PERC_COL"; replace remaining "%" with the empty string (i.e.
> drop bare escape characters); replace the sentinel "PAIR_TEST_PERC_COL"
> with "\\:" (a backslash-escaped colon); replace the sentinel
> "PAIR_TEST_PERC_PERC" with "%" (a literal percent). Returns the
> transformed string. The two sentinels protect the escaped "%%" and "%:"
> sequences from the bare-"%" removal step.

> [spec:hfst:def:hfst-pair-test.basic-transducer-vector]
> typedef std::vector<HfstBasicTransducer> BasicTransducerVector

> [spec:hfst:def:hfst-pair-test.demangle-fn]
> std::string demangle(std::string name)

> [spec:hfst:sem:hfst-pair-test.demangle-fn]
> Recovers a human-readable twolc rule name from its mangled internal form.
> Working on a mutable copy of the argument: while the substring
> "__HFST_TWOLC_RULE_NAME=" occurs, delete that occurrence (replace it with
> the empty string); then, while the substring "__HFST_TWOLC_SPACE" occurs,
> replace that occurrence with a single space " ". Returns the resulting
> string.

> [spec:hfst:def:hfst-pair-test.get-symbols-fn]
> void get_symbols(HfstBasicTransducer &t,SymbolSet &known_symbols)

> [spec:hfst:sem:hfst-pair-test.get-symbols-fn]
> Collects every symbol used on any transition of basic transducer t into
> the set known_symbols. Iterates over every state's transition list and, for
> each transition, inserts both its input symbol and its output symbol into
> known_symbols (a std::set, so duplicates are ignored). known_symbols is
> mutated in place; nothing is returned.

> [spec:hfst:def:hfst-pair-test.get-target-fn]
> HfstState get_target(const std::string &isymbol,

> [spec:hfst:sem:hfst-pair-test.get-target-fn]
> Given an input symbol, an output symbol, a source state s, a basic
> transducer t and the set of known symbols, returns the target state reached
> by the symbol pair, or (unsigned)-1 if no transition matches. Initialises an
> identity_target to -1. Scans the transitions out of state s: if a transition
> has input symbol equal to isymbol AND output symbol equal to osymbol, return
> its target state immediately; otherwise, if a transition is the identity
> arc ("@_IDENTITY_SYMBOL_@":"@_IDENTITY_SYMBOL_@"), record its target in
> identity_target. After the scan, if isymbol equals osymbol and isymbol is
> not in known_symbols, return identity_target (the identity arc handles
> unknown identity pairs); otherwise return -1.

> [spec:hfst:def:hfst-pair-test.get-transducer-fn]
> HfstTransducer get_transducer(const StringPairVector &tokenized_pair_string)

> [spec:hfst:sem:hfst-pair-test.get-transducer-fn]
> Builds a single linear (chain) transducer accepting exactly the given
> tokenized pair string. Starting from an empty HfstBasicTransducer with
> current state s = 0 (the initial state), for each symbol pair (first,second)
> in order: add a new state target, add a transition from s to target with
> input first, output second and weight 0.0, then advance s to target. After
> all pairs, set the final weight of the last state s to 0.0. Convert the
> basic transducer to a HfstTransducer of TROPICAL_OPENFST_TYPE and return it.

> [spec:hfst:def:hfst-pair-test.is-empty-or-comment-fn]
> bool is_empty_or_comment(const char * line)

> [spec:hfst:sem:hfst-pair-test.is-empty-or-comment-fn]
> Reports whether a line should be skipped. Advances past leading spaces and
> tabs; if the first non-whitespace character is the NUL terminator (empty or
> all-whitespace line) or '!' (comment marker), returns true; otherwise
> returns false.

> [spec:hfst:def:hfst-pair-test.is-final-state-fn]
> bool is_final_state(HfstState s,const HfstBasicTransducer &t)

> [spec:hfst:sem:hfst-pair-test.is-final-state-fn]
> Returns whether state s is a final (accepting) state of basic transducer t,
> by delegating to t.is_final_state(s).

> [spec:hfst:def:hfst-pair-test.is-negative-test-line-fn]
> bool is_negative_test_line(const std::string &line)

> [spec:hfst:sem:hfst-pair-test.is-negative-test-line-fn]
> Returns true iff, after stripping leading/trailing whitespace, the line
> begins with the negative-test marker "!!$" (the first sizeof("!!$")-1 bytes
> equal "!!$").

> [spec:hfst:def:hfst-pair-test.is-positive-test-line-fn]
> bool is_positive_test_line(const std::string &line)

> [spec:hfst:sem:hfst-pair-test.is-positive-test-line-fn]
> Returns true iff, after stripping leading/trailing whitespace, the line
> begins with the positive-test marker "!!€" (the first sizeof("!!€")-1 bytes
> equal "!!€"; note "€" is multi-byte UTF-8).

> [spec:hfst:def:hfst-pair-test.main-fn]
> int main( int argc, char **argv )

> [spec:hfst:sem:hfst-pair-test.main-fn]
> Program entry point. Sets the program name to "HfstPairTest" version "0.6"
> and parses options; if parse_options returns anything other than
> EXIT_CONTINUE, returns that value. Closes the buffered input file (unless it
> is stdin), then logs "Reading from <in>, writing to <out>". Opens the rule
> transducer input stream from inputfilename (or stdin if input is not a named
> file); on a HfstException it errors out with EXIT_FAILURE reporting the file
> is not a valid transducer file. Calls process_stream on that stream and the
> output FILE, capturing exit_code. Unless silent, prints "Test passed.\n"
> when exit_code is 0, else "Test failed.\n". Closes the output file (unless
> stdout), frees inputfilename and outfilename, and returns exit_code.

> [spec:hfst:def:hfst-pair-test.parse-options-fn]
> int

> [spec:hfst:sem:hfst-pair-test.parse-options-fn]
> Parses command-line options. First extends argv from the environment. Loops
> over getopt_long with the common and unary long-option tables plus three
> tool-specific options: -I/--input-strings (required argument, the pair-test
> strings file), -N/--negative-test (no argument), -X/--xerox-mode (no
> argument); the short-option string is the common+unary shorts followed by
> "I:NX". For each option: common cases and unary cases are handled by the
> shared fragments; -I duplicates optarg into pair_test_file_name, opens it for
> reading into pair_test_file and sets pair_test_given true; -N sets
> positive_test false; -X sets xerox_mode true; unknown options fall through to
> the error case. After the loop, if no pair-test file was given, default
> pair_test_file to stdin and pair_test_file_name to "<stdin>". Runs the common
> and unary parameter checks. If inputfilename equals "<stdin>", error out with
> EXIT_FAILURE: the rule transducer file must be supplied with -i. Returns
> EXIT_CONTINUE.

> [spec:hfst:def:hfst-pair-test.print-failure-info-fn]
> void print_failure_info(const StringPairVector &tokenized_pair_string,

> [spec:hfst:sem:hfst-pair-test.print-failure-info-fn]
> Prints, for a rejected positive pair string, the prefix that a given rule
> could still recognise. Builds a chain transducer str_transducer from the
> tokenized pair string (via get_transducer), and a transducer tt from the
> rule's basic transducer t (TROPICAL_OPENFST_TYPE). Replaces str_transducer
> by (input-projection of str_transducer) composed with tt, then minimized.
> Finally calls print_recognized_prefix with the (now reduced) str_transducer,
> the rule name, the output file and known_symbols to report how far the rule
> can follow the string before running out of transitions.

> [spec:hfst:def:hfst-pair-test.print-recognized-prefix-fn]
> void print_recognized_prefix(const StringPairVector &tokenized_pair_string,

> [spec:hfst:sem:hfst-pair-test.print-recognized-prefix-fn]
> Prints the recognised prefix of a pair string with respect to a rule's
> (already composed/projected) str_transducer, marking where recognition
> breaks. Does nothing when silent. Prints "Rule <name> fails:\n". Starting at
> state 0, walks the pair string symbol by symbol following str_transducer via
> get_target; for each pair consumed before recognition fails, prints the
> unescaped symbol (single token if input==output, otherwise "in:out") followed
> by a space. When get_target returns -1 (no transition), stops the consumed
> loop. Prints "HERE ---> " to mark the failure position, then prints all the
> remaining (unconsumed) pairs in the same unescaped form. Ends with two
> newlines. Symbols are passed through unescape (epsilon -> "0", "@#@" -> "#").

> [spec:hfst:def:hfst-pair-test.print-usage-fn]
> void

> [spec:hfst:sem:hfst-pair-test.print-usage-fn]
> Prints the help/usage text to message_out: a usage line "Usage: <prog>
> [OPTIONS...] [INFILE]" and a one-line description, the common program
> options, the Input/Output options (-i/--input, -o/--output,
> -N/--negative-test, -X/--xerox-mode), the Pair test options
> (-I/--input-strings=SFILE), and several explanatory paragraphs describing:
> default STDIN/STDOUT behaviour; how the rule file is tested with pair
> strings; the pair-string file format and "!" comment lines; the three test
> modes (positive, negative, Xerox); exit codes (0 success, 1 failure) and the
> "Test passed"/"Test failed" messages; what is printed on failure in positive
> vs negative mode; the Xerox-mode two-line "!!€"/"!!$" test-case format with
> examples; and silent-mode (-s) behaviour. Finishes with the report-bugs and
> more-info footers.

> [spec:hfst:def:hfst-pair-test.process-stream-fn]
> int

> [spec:hfst:sem:hfst-pair-test.process-stream-fn]
> The core driver. Reads every transducer in the input rule stream into a
> grammar vector of basic transducers, recording each rule's demangled name;
> tracks the rule transducer implementation type; logs reading progress; then
> closes the input stream. If the grammar is non-empty, gathers the known
> symbols from the first rule transducer (get_symbols) and logs each.
>
> If not in Xerox mode: builds a HfstStrings2FstTokenizer with no multichar
> symbols and epsilon represented as "0". Reads pair_test_file line by line;
> truncates each line at the first newline; skips empty/comment lines; logs the
> pair test; tokenizes the line into a StringPairVector with spaces=true, and
> wraps it with a leading and trailing ("@#@", internal_epsilon) boundary pair;
> runs test() over the whole grammar in positive_test mode, accumulating the
> first non-zero exit code into exit_code. If tokenization throws
> UnescapedColsFound, errors out advising the use of "0" for epsilon pairs.
>
> If in Xerox mode: builds the tokenizer from the known symbols (epsilon "0").
> Reads the twolc source line by line (truncated at newline), collecting
> positive test cases (lines marked "!!€") and negative test cases (lines
> marked "!!$"); for each, strips the marker and surrounding whitespace and logs
> it. Each collected list must have an even number of entries (input then output
> per case), else errors out. For each consecutive (input,output) pair of
> positive cases: backslash-escapes both sides, joins them as "input:output",
> tokenizes with spaces=false, wraps with the "@#@" boundary pairs, and runs
> test() with positive=true; UnescapedColsFound errors out advising %-escaping.
> The negative cases are processed identically but with positive=false. exit_code
> accumulates the first non-zero result. Returns exit_code.

> [spec:hfst:def:hfst-pair-test.string-vector]
> typedef std::vector<std::string> StringVector

> [spec:hfst:def:hfst-pair-test.strip-space-fn]
> std::string strip_space(const std::string &line)

> [spec:hfst:sem:hfst-pair-test.strip-space-fn]
> Returns the line with leading and trailing spaces and tabs removed. Finds the
> first position not in " \t"; if there is none (the line is empty or all
> whitespace) returns the empty string. Otherwise finds the last position not in
> " \t" and returns the substring spanning the first through last non-whitespace
> characters inclusive.

> [spec:hfst:def:hfst-pair-test.symbol-set]
> typedef std::set<std::string> SymbolSet

> [spec:hfst:def:hfst-pair-test.test-fn]
> int test(const StringPairVector &tokenized_pair_string,

> [spec:hfst:sem:hfst-pair-test.test-fn]
> Tests one tokenized pair string against the whole grammar and reports the
> result. There are two overloads.
>
> The single-rule overload (tokenized_pair_string, t, positive, outfile,
> known_symbols): starting at state 0, follows the pair string through basic
> transducer t via get_target; if a transition is missing (-1), returns 1 for a
> positive test (rejected) or 0 for a negative test (correctly rejected). If the
> whole string is consumed, returns 0 if the final state is accepting and the
> test is positive; 1 if positive but not accepting; for a negative test returns
> 0 when not accepting and 1 when accepting.
>
> The grammar overload (tokenized_pair_string, pair_string, grammar, names,
> positive, outfile, known_symbols): initialises positive_exit_code=0 and
> negative_exit_code=1. For each rule (with its name from names) it runs the
> single-rule test. In positive mode, a per-rule result of 1 triggers
> print_failure_info for that rule, and positive_exit_code latches the first
> non-zero result. In negative mode, negative_exit_code latches the first 0
> result (clearing the initial 1). In positive mode: unless silent, prints
> "FAIL: <pair_string> REJECTED" when the code is 1; if verbose, prints
> "<pair_string> PASSED" when 0; returns positive_exit_code. In negative mode:
> unless silent, prints "FAIL: <pair_string> PASSED" when the code is 1; if
> verbose, prints "<pair_string> REJECTED" when 0; returns negative_exit_code.

> [spec:hfst:def:hfst-pair-test.unescape-fn]
> std::string unescape(std::string symbol)

> [spec:hfst:sem:hfst-pair-test.unescape-fn]
> Maps an internal symbol to its display form: the epsilon symbol becomes "0",
> the word-boundary symbol "@#@" becomes "#", and any other symbol is returned
> unchanged.
