# libhfst/src/parsers/TwolcCompiler.cc, libhfst/src/parsers/TwolcCompiler.h

> [spec:hfst:def:twolc-compiler.hfst.twolc.twolc-compiler]
> class TwolcCompiler

> [spec:hfst:def:twolc-compiler.hfst.twolc.twolc-compiler.compile-fn]
> int

> [spec:hfst:sem:twolc-compiler.hfst.twolc.twolc-compiler.compile-fn]
> `TwolcCompiler::compile(inputfile, outputfile, silent, verbose, resolve_left_conflicts, resolve_right_conflicts, type)` runs the full three-pass twolc compilation pipeline and returns an `int` exit code.
> Pass 1 (preprocessing):
> - Calls `hfst::twolcpre1::reset_lexer()` then `hfst::twolcpre1::reset_parser()` to clear any state from a previous run.
> - Opens `inputfile` as an `std::ifstream istr` and calls `hfst::twolcpre1::set_input(istr, inputfile)`.
> - Creates an `std::ostringstream oss1` and calls `hfst::twolcpre1::set_output(oss1)`.
> - In a try block, calls `hfst::twolcpre1::parse()`; if its return value is non-zero, returns that value. Catches `const HfstException &e`: prints `e.what()` to `std::cerr` followed by newline and returns -1.
> Pass 2 (preprocessing):
> - Calls `hfst::twolcpre2::reset_lexer()` then `hfst::twolcpre2::reset_parser()`.
> - Constructs `std::istringstream iss1(oss1.str())` (the output of pass 1) and calls `hfst::twolcpre2::set_input(iss1, inputfile)`.
> - In a try block, calls `hfst::twolcpre2::parse()`; if non-zero, returns it. Same HfstException catch as pass 1 (print and return -1).
> - Calls `hfst::twolcpre2::complete_alphabet()`.
> - Builds `std::ostringstream oss2` by streaming `hfst::twolcpre2::get_total_alphabet_symbol_queue()`, then a literal `" "`, then `hfst::twolcpre2::get_non_alphabet_symbol_queue()`.
> Pass 3 (compilation):
> - Calls `hfst::twolcpre3::reset_parser()`.
> - In a try block: constructs `std::istringstream iss2(oss2.str())` and calls `hfst::twolcpre3::set_input(iss2, inputfile)`.
> - Calls `OtherSymbolTransducer::set_transducer_type(type)`, `hfst::twolcpre3::set_silent(silent)`, `hfst::twolcpre3::set_verbose(verbose)`.
> - Constructs a local `TwolCGrammar twolc_grammar(silent, verbose, resolve_left_conflicts, resolve_right_conflicts)` and calls `hfst::twolcpre3::set_grammar(&twolc_grammar)`.
> - Calls `hfst::twolcpre3::parse()` into `exit_code`; if non-zero, returns it.
> - Constructs an `HfstOutputStream out(outputfile, type)` and calls `hfst::twolcpre3::get_grammar()->compile_and_store(out)`, then returns `exit_code` (0).
> - Catch `const HfstException e`: prints `"This is an hfst interface bug:"`, newline, `e()`, newline to `std::cerr`; returns -1. Catch `const char *s`: prints `"This is an a bug probably from sfst:"`, newline, `s`, newline to `std::cerr`; returns -1.

> [spec:hfst:def:twolc-compiler.hfst.twolcpre1.parse-fn]
> int parse()

> [spec:hfst:sem:twolc-compiler.hfst.twolcpre1.parse-fn]
> `hfst::twolcpre1::parse()` simply calls and returns the result of the bison-generated parser entry point `htwolcpre1parse()` (the pass-1 parser). The return value is the parser's exit code (0 on success, non-zero on parse error).

> [spec:hfst:def:twolc-compiler.hfst.twolcpre1.reset-lexer-fn]
> void reset_lexer()

> [spec:hfst:sem:twolc-compiler.hfst.twolcpre1.reset-lexer-fn]
> `hfst::twolcpre1::reset_lexer()` resets the pass-1 lexer's module-level boolean state to its initial values: sets `regexp_start = false`, `htwolcpre1_rules_start = false`, and `where_seen = false`. No return value.

> [spec:hfst:def:twolc-compiler.hfst.twolcpre1.reset-parser-fn]
> void reset_parser()

> [spec:hfst:sem:twolc-compiler.hfst.twolcpre1.reset-parser-fn]
> `hfst::twolcpre1::reset_parser()` resets all module-level state of the pass-1 parser to fresh/initial values:
> - `output = NULL` (clears the output stream pointer).
> - `htwolcpre1_line_number = 1`.
> - `htwolcpre1_input_reader.reset()` (resets the InputReader).
> - `variable_value_map = VariableValueMap()`, `rule_variables = RuleVariables()` (default-constructed replacements). (Note: `rule_symbol_vector.reset()` is commented out, so the rule symbol vector is NOT reset.)
> - `htwolcpre1_symbol_queue = HandyDeque<std::string>()` (empty).
> - `sets = HandySet<std::string>()`, `definitions = HandySet<std::string>()` (empty).
> - `set_symbols = HandyMap<std::string,std::vector<std::string>>()` (empty).
> - `set_name = std::string()` (empty), `latest_set.clear()`.
> - `htwolcpre1_inside_parenthesis = false`.
> - `variable_vector.clear()`.
> - `matcher_queue = HandyDeque<Matcher>()` (empty).
> No return value.

> [spec:hfst:def:twolc-compiler.hfst.twolcpre1.set-input-fn]
> void set_input(std::istream &istr, const std::string &filename)

> [spec:hfst:sem:twolc-compiler.hfst.twolcpre1.set-input-fn]
> `hfst::twolcpre1::set_input(istr, filename)` forwards to `htwolcpre1_input_reader.set_input(istr, filename)`, installing the given input stream and source filename into the pass-1 module-level `InputReader`. No return value.

> [spec:hfst:def:twolc-compiler.hfst.twolcpre1.set-output-fn]
> void set_output(std::ostream &ostr)

> [spec:hfst:sem:twolc-compiler.hfst.twolcpre1.set-output-fn]
> `hfst::twolcpre1::set_output(ostr)` stores the address of the given output stream into the module-level `std::ostream * output` pointer (`output = &ostr`). The pass-1 parser writes its preprocessed result to this stream. No return value.

> [spec:hfst:def:twolc-compiler.hfst.twolcpre2.complete-alphabet-fn]
> void complete_alphabet(void)

> [spec:hfst:sem:twolc-compiler.hfst.twolcpre2.complete-alphabet-fn]
> `hfst::twolcpre2::complete_alphabet()` collects every symbol pair occurring anywhere in the grammar (Alphabet section and elsewhere) and appends them, as a full Alphabet section, to `total_alphabet_symbol_queue`. Steps:
> - Creates a local `HandySet<SymbolPair> symbol_pair_set`.
> - Calls helper `insert_alphabet_pairs(htwolcpre2_alphabet_symbol_queue, symbol_pair_set)` then `insert_alphabet_pairs(htwolcpre2_non_alphabet_symbol_queue, symbol_pair_set)`. The helper scans each queue and, for every position where the current element is a valid symbol (one of `__HFST_TWOLC_0`, `__HFST_TWOLC_.#.`, `__HFST_TWOLC_#`, `__HFST_TWOLC_SPACE`, `__HFST_TWOLC_TAB`, or any string not containing `__HFST_TWOLC_`), the next element equals `__HFST_TWOLC_:`, and the element after that is likewise a valid symbol, inserts a `SymbolPair(input,output)` into the set; here `__HFST_TWOLC_#` is mapped to literal `#` for both input and output. The helper finally always inserts `SymbolPair("__HFST_TWOLC_.#.","__HFST_TWOLC_.#.")`.
> - Pushes `"__HFST_TWOLC_Alphabet"` onto `total_alphabet_symbol_queue`.
> - For each `SymbolPair` in `symbol_pair_set` (iterated in the set's order), pushes three elements onto `total_alphabet_symbol_queue`: `it->first`, `"__HFST_TWOLC_:"`, `it->second`.
> No return value; it mutates the module-level `total_alphabet_symbol_queue`.

> [spec:hfst:def:twolc-compiler.hfst.twolcpre2.parse-fn]
> int parse()

> [spec:hfst:sem:twolc-compiler.hfst.twolcpre2.parse-fn]
> `hfst::twolcpre2::parse()` calls and returns the result of the bison-generated parser entry point `htwolcpre2parse()` (the pass-2 parser). The return value is the parser's exit code (0 on success, non-zero on error).

> [spec:hfst:def:twolc-compiler.hfst.twolcpre2.reset-lexer-fn]
> void reset_lexer()

> [spec:hfst:sem:twolc-compiler.hfst.twolcpre2.reset-lexer-fn]
> `hfst::twolcpre2::reset_lexer()` resets the pass-2 lexer's module-level state by setting `alphabet_ended = false`. No return value.

> [spec:hfst:def:twolc-compiler.hfst.twolcpre2.reset-parser-fn]
> void reset_parser()

> [spec:hfst:sem:twolc-compiler.hfst.twolcpre2.reset-parser-fn]
> `hfst::twolcpre2::reset_parser()` resets the pass-2 parser's module-level state:
> - `htwolcpre2_line_number = 1`.
> - `htwolcpre2_input_reader.reset()`.
> - `htwolcpre2_non_alphabet_symbol_queue = HandyDeque<std::string>()` (empty).
> - `htwolcpre2_alphabet_symbol_queue = HandyDeque<std::string>()` (empty).
> - `total_alphabet_symbol_queue = HandyDeque<std::string>()` (empty).
> No return value.

> [spec:hfst:def:twolc-compiler.hfst.twolcpre2.set-input-fn]
> void set_input(std::istream &istr, const std::string &filename)

> [spec:hfst:sem:twolc-compiler.hfst.twolcpre2.set-input-fn]
> `hfst::twolcpre2::set_input(istr, filename)` forwards to `htwolcpre2_input_reader.set_input(istr, filename)`, installing the given input stream and source filename into the pass-2 module-level `InputReader`. No return value.

> [spec:hfst:def:twolc-compiler.hfst.twolcpre3.get-grammar-fn]
> TwolCGrammar *get_grammar()

> [spec:hfst:sem:twolc-compiler.hfst.twolcpre3.get-grammar-fn]
> `hfst::twolcpre3::get_grammar()` returns the module-level `TwolCGrammar * grammar` pointer (the grammar previously installed via `set_grammar`). No mutation.

> [spec:hfst:def:twolc-compiler.hfst.twolcpre3.parse-fn]
> int parse()

> [spec:hfst:sem:twolc-compiler.hfst.twolcpre3.parse-fn]
> `hfst::twolcpre3::parse()` calls and returns the result of the bison-generated parser entry point `htwolcpre3parse()` (the pass-3 compilation parser). The return value is the parser's exit code (0 on success, non-zero on error).

> [spec:hfst:def:twolc-compiler.hfst.twolcpre3.reset-parser-fn]
> void reset_parser()

> [spec:hfst:sem:twolc-compiler.hfst.twolcpre3.reset-parser-fn]
> `hfst::twolcpre3::reset_parser()` resets the pass-3 parser's module-level state:
> - `htwolcpre3_line_number = 1`.
> - `htwolcpre3_input_reader.reset()`.
> - `alphabet = Alphabet()` (default-constructed; note `Alphabet` is `#define`d to `TwolCAlphabet` when `HAVE_XFSM` is set).
> - `definition_map = HandyMap<std::string,OtherSymbolTransducer>()` (empty).
> No return value. (It does not reset the `grammar` pointer.)

> [spec:hfst:def:twolc-compiler.hfst.twolcpre3.set-grammar-fn]
> void set_grammar(TwolCGrammar *grammar)

> [spec:hfst:sem:twolc-compiler.hfst.twolcpre3.set-grammar-fn]
> `hfst::twolcpre3::set_grammar(grammar_)` stores the given `TwolCGrammar *` argument into the module-level `grammar` pointer (`grammar = grammar_`). The pass-3 parser uses this grammar to compile rules. No return value; the pointer is not owned (no allocation/free).

> [spec:hfst:def:twolc-compiler.hfst.twolcpre3.set-input-fn]
> void set_input(std::istream &istr, const std::string &filename)

> [spec:hfst:sem:twolc-compiler.hfst.twolcpre3.set-input-fn]
> `hfst::twolcpre3::set_input(istr, filename)` forwards only the stream to `htwolcpre3_input_reader.set_input(istr)` (the single-argument overload); the `filename` parameter is accepted but ignored. No return value.

> [spec:hfst:def:twolc-compiler.hfst.twolcpre3.set-silent-fn]
> void set_silent(bool val)

> [spec:hfst:sem:twolc-compiler.hfst.twolcpre3.set-silent-fn]
> `hfst::twolcpre3::set_silent(val)` stores the boolean argument into the module-level `bool silent_` (`silent_ = val`), controlling whether the pass-3 parser suppresses messages. No return value.

> [spec:hfst:def:twolc-compiler.hfst.twolcpre3.set-verbose-fn]
> void set_verbose(bool val)

> [spec:hfst:sem:twolc-compiler.hfst.twolcpre3.set-verbose-fn]
> `hfst::twolcpre3::set_verbose(val)` stores the boolean argument into the module-level `bool verbose_` (`verbose_ = val`), controlling whether the pass-3 parser emits verbose output. No return value.

