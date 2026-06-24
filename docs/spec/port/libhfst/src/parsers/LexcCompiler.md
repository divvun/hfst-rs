# libhfst/src/parsers/LexcCompiler.cc, libhfst/src/parsers/LexcCompiler.h

> [spec:hfst:def:lexc-compiler.hfst.lexc.encoded-name-fn]
> string encodedName(lexiconName)

> [spec:hfst:sem:lexc-compiler.hfst.lexc.encoded-name-fn]
> This annotation marks the local-variable construction of `encodedName`
> inside `setCurrentLexiconName`, not a standalone function. It runs in the
> branch where the current lexicon name is NOT in `noFlags_`. A `std::string
> encodedName` is initialized as a copy of `lexiconName`. Then
> `flagJoinerEncode(encodedName, false)` is called (which mutates `encodedName`
> in place into its P-side flag-joiner-encoded form) and the resulting string
> is registered with `tokenizer_.add_multichar_symbol(encodedName)`. Then
> `flagJoinerEncode(encodedName, true)` is called again on the same variable
> (transforming the already-encoded string into its R-side form) and that
> result is also registered via `tokenizer_.add_multichar_symbol(encodedName)`.
> Net effect: both the P-side and R-side flag-joiner encodings of the lexicon
> name are added to the tokenizer as multichar symbols.

> [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler]
> class LexcCompiler {
>   LexcCompiler &parse(FILE *infile);
>   LexcCompiler &parse(const char *filename);
>   LexcCompiler &setVerbosity(unsigned int verbose);
>   LexcCompiler &setTreatWarningsAsErrors(bool value);
>   LexcCompiler &setAllowMultipleSublexiconDefinitions(bool value);
>   LexcCompiler &setAlignStrings(bool value);
>   LexcCompiler &setWithFlags(bool value);
>   LexcCompiler &setMinimizeFlags(bool value);
>   LexcCompiler &setRenameFlags(bool value);
>   LexcCompiler &addAlphabet(const std::string &alphabet);
>   LexcCompiler &addNoFlag(const std::string &lexname);
>   LexcCompiler &setCurrentLexiconName(const std::string &lexicon_name);
>   LexcCompiler &addStringEntry(const std::string &entry, const std::string &continuation, const double weight);
>   LexcCompiler &addStringPairEntry(const std::string &upper, const std::string &lower, const std::string &continuation, const double weight);
>   LexcCompiler &addXreEntry(const std::string &xre, const std::string &continuation, const double weight);
>   LexcCompiler &addXreDefinition(const std::string &name, const std::string &xre);
>   LexcCompiler &setInitialLexiconName(const std::string &lexicon_name);
>   const std::map<std::string, hfst::HfstTransducer> &getStringTries() const;
>   const std::map<std::string, hfst::HfstTransducer> &getRegexpUnions() const;
>   const LexcCompiler &printConnectedness(bool &warnings_printed);
>   bool parseErrors_;
>   LexcCompiler &unicodeCheck_(const string &data);
>   bool quiet_;
>   bool verbose_;
>   bool align_strings_;
>   bool with_flags_;
>   bool minimize_flags_;
>   bool rename_flags_;
>   bool split_characters_;
>   bool treat_warnings_as_errors_;
>   bool warn_everything_;
>   bool warn_missing_lexicons_;
>   bool warn_unused_lexicons_;
>   bool warn_repeated_lexicons_;
>   bool warn_missing_alphabets_;
>   bool warn_one_sided_flags_;
>   bool warn_unnecessary_escapes_;
>   std::ostream *error_;
>   hfst::ImplementationType format_;
>   hfst::HfstTokenizer tokenizer_;
>   hfst::xre::XreCompiler xre_;
>   std::string initialLexiconName_;
>   HfstBasicTransducer stringsTrie_;
>   std::map<std::string, hfst::HfstTransducer *> regexps_;
>   std::set<std::string> lexiconNames_;
>   std::set<std::string> noFlags_;
>   std::set<std::string> continuations_;
>   std::set<std::string> alphabets_;
>   std::string currentLexiconName_;
>   size_t totalEntries_;
>   size_t currentEntries_;
> }

> [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.are-warnings-treated-as-errors-fn]
> bool

> [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.are-warnings-treated-as-errors-fn]
> Plain getter. Returns the boolean member `treat_warnings_as_errors_`. No
> side effects.

> [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.compile-lexical-fn]
> HfstTransducer *

> [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.compile-lexical-fn]
> Builds and returns a newly allocated `HfstTransducer*` representing the full
> compiled lexc morphology, or returns null (0) on error.
> Steps:
> 1. Obtain the error stream via `get_stream(error_)` into `err`.
> 2. If `parseErrors_` is set, print "compilation aborted due to previous
>    errors" + newline to `err` and return 0.
> 3. Set local `warnings_generated=false`, call `printConnectedness(warnings_generated)`.
>    If `warnings_generated` and `treat_warnings_as_errors_`, print (optionally
>    colourised) an error message about missing or unused LEXICONs and -Werror,
>    flush, and return 0.
> 4. Construct `HfstTransducer lexicons` from the `stringsTrie_` basic
>    transducer with `format_`; call `optimize()`; then `repeat_star().optimize()`
>    to overgenerate.
> 5. Build `smallSubstitutions` mapping "@0@"->"@_EPSILON_SYMBOL_@",
>    "@@ANOTHER_EPSILON@@"->"@_EPSILON_SYMBOL_@", "@ZERO@"->"0"; apply via
>    `lexicons.substitute(...)`; then `lexicons.prune_alphabet()`.
> 6. Create empty `HfstBasicTransducer joinersTrie_` and empty
>    `HfstSymbolSubstitutions allJoinersToEpsilon`.
> 7. If NOT `with_flags_`: encode the initial lexicon name with `joinerEncode`
>    into a start transducer, encode "#" via `joinerEncode` into an end
>    transducer, and set `lexicons = start.concatenate(lexicons).concatenate(end).optimize()`.
>    Then for each name in `lexiconNames_`: optionally print "Morphotaxing... <name>"
>    when verbose; compute `joinerEncode(name)`, tokenize `joinerEnc+joinerEnc`
>    (no spaces) and `joinersTrie_.disjunct(thatVector, 0)`; insert
>    `joinerEnc -> @_EPSILON_SYMBOL_@` into `allJoinersToEpsilon`. Finally
>    joinerEncode the initial name and "#" and insert both as
>    `... -> @_EPSILON_SYMBOL_@` into `allJoinersToEpsilon`.
> 8. Else (`with_flags_`): build startP via `flagJoinerEncode(rootP,false)`,
>    startR via `flagJoinerEncode(rootR,true)`; flag-encode "#" both P (false)
>    and R (true) into endStringP/endStringR, add both as multichar symbols,
>    build endP/endR transducers; set `lexicons =
>    startP.concatenate(lexicons).concatenate(endR).optimize()`. Then for each
>    name in `lexiconNames_`: optionally print when verbose; compute
>    `flagJoinerEncode(name,false)` and `flagJoinerEncode(name,true)`, tokenize
>    their concatenation and disjunct into `joinersTrie_` with weight 0.
> 9. Build `HfstBasicTransducer fsm` from `lexicons`. Iterate all states and
>    transitions, collecting each transition's output symbol into `rightSymbols`
>    UNLESS it starts with any of the prefixes "@@ANOTHER_EPSILON@@",
>    "$_LEXC_JOINER.", "@_", "$P.LEXNAME.", or "$R.LEXNAME.".
> 10. For each symbol in `rightSymbols`: add it as a multichar symbol, tokenize
>     it, and disjunct into `joinersTrie_` with weight 0.
> 11. Build `HfstTransducer joinersAll` from `joinersTrie_` with `format_`;
>     call `repeat_star()` then `optimize()`. When `debug`, dump lexicons and
>     joinersAll. Then `lexicons.compose(joinersAll).optimize()`.
> 12. Build `HfstSymbolSubstitutions allSubstitutions`. If `with_flags_`:
>     optionally print "Changing flags..." when verbose; prune_alphabet; iterate
>     the lexicons alphabet, and for each symbol `s` that starts with '$', ends
>     with '$', and has size>2, copy it, replace every '$' with '@', and insert
>     `s -> mapped` into `fakeFlagsToRealFlags`; merge those into
>     `allSubstitutions`. Else: merge `allJoinersToEpsilon` into `allSubstitutions`.
> 13. `lexicons.substitute(allSubstitutions).optimize()` then `prune_alphabet()`.
> 14. (Insert regular expressions.) Build `fakeRegexprToReal`: for each entry in
>     `regexps_` whose key starts with '$', copy the key, replace all '$' with
>     '@', insert `key -> mapped`. Apply `lexicons.substitute(fakeRegexprToReal).optimize()`
>     then `prune_alphabet()`.
> 15. Build `std::map<String,HfstBasicTransducer> regMarkToTr`: for each entry
>     in `regexps_`, compute the mapped key (replace '$' with '@' if key starts
>     with '$'), and map it to a `HfstBasicTransducer` copy of `*it->second`.
> 16. Build `HfstBasicTransducer lexicons_basic` from `lexicons`; call
>     `lexicons_basic.substitute(regMarkToTr, true)` (transducer substitution);
>     then `prune_alphabet()`.
> 17. Allocate `HfstTransducer *rv = new HfstTransducer(lexicons_basic, format_)`.
> 18. If `with_flags_`: collect from `rv`'s alphabet every symbol whose first 10
>     chars are "@P.LEXNAME" or "@R.LEXNAME" into `flagD`. Build a regexp string
>     `[ "FLAG1" | "FLAG2" ... ]` (each flag quoted, joined with "| "), copy it
>     as `context_regexp`, then append ` -> 0 || <context> _ ` to form the
>     flag-removal rule. Compile it with a fresh `XreCompiler xre_comp(format_)`
>     into `flag_filter`, optimize it; create `inverted_flag_filter` = copy of
>     flag_filter, `invert().optimize()`. Compose
>     `inverted_flag_filter .o. rv .o. flag_filter` (using compose(...,true),
>     optimize at the end) into `filtered_lexicons`; delete inverted_flag_filter;
>     `rv->assign(filtered_lexicons)`.
> 19. `rv->optimize()`. If not quiet, print newline to `err`. Return `rv`.

> [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.flush-fn]
> void

> [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.flush-fn]
> Windows console-output flush helper. On non-WINDOWS builds it does nothing
> (the `oss` parameter is simply ignored). On WINDOWS, if `output_to_console_`
> is true and the passed `oss` equals the internal `winoss_` stream: depending
> on the saved `redirected_stream_`, write the accumulated `winoss_.str()` to
> the real console via `hfst_fprintf_console(stderr,...)` (if redirected stream
> was std::cerr) or `hfst_fprintf_console(stdout,...)` (if std::cout); otherwise
> do nothing. Then reset `redirected_stream_` to NULL and clear `winoss_` by
> setting its string buffer to "". Returns void.

> [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.get-error-stream-fn]
> std::ostream *

> [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.get-error-stream-fn]
> Plain getter. Returns the `std::ostream *` member `error_`. No side effects.

> [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.get-output-to-console-fn]
> bool

> [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.get-output-to-console-fn]
> Getter for the Windows console-output flag. On WINDOWS builds it returns the
> member `output_to_console_`; on all other builds it always returns false. No
> side effects.

> [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.get-stream-fn]
> std::ostream *

> [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.get-stream-fn]
> Returns the stream to actually write to, given a requested `std::ostream *oss`.
> On non-WINDOWS builds it simply returns `oss` unchanged. On WINDOWS, if
> `output_to_console_` is true AND `oss` is either `&std::cerr` or `&std::cout`,
> it saves `oss` into `redirected_stream_` and returns `&winoss_` (the internal
> buffering stream) instead; otherwise it returns `oss` unchanged. Used together
> with `flush()` to redirect cerr/cout through the Windows console writer.

> [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.get-verbosity-fn]
> unsigned int

> [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.get-verbosity-fn]
> Derives a verbosity level from the two boolean flags `quiet_` and `verbose_`.
> Returns 0 when `quiet_ && !verbose_`; returns 1 when `!quiet_ && !verbose_`;
> returns 2 when `!quiet_ && verbose_`. If none of these match (i.e. `quiet_`
> and `verbose_` are both true), it throws the C-string literal
> "LexcCompiler::getVerbosity() failed".

> [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.has-split-characters-fn]
> bool

> [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.has-split-characters-fn]
> Plain getter. Returns the boolean member `split_characters_`. No side effects.

> [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.is-quiet-fn]
> bool

> [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.is-quiet-fn]
> Plain getter. Returns the boolean member `quiet_`. No side effects.

> [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.is-strict-alphabets-fn]
> bool

> [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.is-strict-alphabets-fn]
> Getter aliasing the missing-alphabets warning flag. Returns the boolean
> member `warn_missing_alphabets_`. No side effects.

> [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.is-warning-fn]
> bool

> [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.is-warning-fn]
> Queries whether a named warning category is enabled. Takes a C-string
> `warning` and compares it (with `strcmp`) against the known warning names,
> returning the corresponding boolean member: "-Wone-sided-flags" ->
> `warn_one_sided_flags_`; "-Wmissing-lexicons" -> `warn_missing_lexicons_`;
> "-Wunused-lexicons" -> `warn_unused_lexicons_`; "-Wrepeated-lexicons" ->
> `warn_repeated_lexicons_`; "-Wmissing-alphabets" -> `warn_missing_alphabets_`;
> "-Wunnecessary-escapes" -> `warn_unnecessary_escapes_`. For any other name it
> prints "unknown warning <warning>\n" to stderr and falls through to return
> false.

> [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.lexc-compiler-fn]
> LexcCompiler::LexcCompiler(ImplementationType impl, bool withFlags,

> [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.lexc-compiler-fn]
> Constructor `LexcCompiler(ImplementationType impl, bool withFlags, bool
> alignStrings)`. Initializes members: `quiet_=false`, `verbose_=false`,
> `align_strings_=alignStrings`, `with_flags_=withFlags`, `minimize_flags_=false`,
> `rename_flags_=false`, `split_characters_=false`,
> `treat_warnings_as_errors_=false`, all `warn_*` flags initialized to false
> (warn_everything_, warn_missing_lexicons_, warn_unused_lexicons_,
> warn_repeated_lexicons_, warn_missing_alphabets_, warn_unnecessary_escapes_),
> `error_=&std::cerr`, `format_=impl`, `xre_` constructed with `impl`,
> `initialLexiconName_="Root"`, `totalEntries_=0`, `currentEntries_=0`,
> (on WINDOWS also `output_to_console_=false`, `winoss_` empty,
> `redirected_stream_=NULL`), and `parseErrors_=false`.
> Constructor body: adds the multichar symbols "@_EPSILON_SYMBOL_@", "@0@",
> "@ZERO@", "@@ANOTHER_EPSILON@@" to `tokenizer_`; inserts the string "#" into
> `lexiconNames_`; adds `joinerEncode("#")` as a multichar symbol to
> `tokenizer_`; then on `xre_` calls `set_expand_definitions(true)`,
> `set_error_stream(error_)`, and `set_verbosity(!quiet_)` (i.e. true since
> quiet_ is false).

> [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.reset-fn]
> void

> [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.reset-fn]
> Resets the compiler to a fresh state without changing config flags. Steps:
> assign a brand new default `hfst::HfstTokenizer()` to `tokenizer_`, then add
> multichar symbols "@_EPSILON_SYMBOL_@", "@0@", "@ZERO@", "@@ANOTHER_EPSILON@@".
> Set `initialLexiconName_="Root"`, `totalEntries_=0`, `currentEntries_=0`,
> `parseErrors_=false`. Clear the sets `lexiconNames_`, `noFlags_`,
> `continuations_`, `alphabets_`. Set `currentLexiconName_=""`. Insert "#" into
> `lexiconNames_`. Assign a new empty `HfstBasicTransducer()` to `stringsTrie_`.
> Finally, delete every `HfstTransducer*` value held in `regexps_` and then
> clear the `regexps_` map. Returns void.

> [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.set-error-stream-fn]
> void

> [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.set-error-stream-fn]
> Setter. Assigns the given `std::ostream *os` to the member `error_`, then
> propagates it by calling `xre_.set_error_stream(error_)`. Returns void.

> [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.set-output-to-console-fn]
> void

> [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.set-output-to-console-fn]
> Setter for the Windows console-output flag. On WINDOWS builds it sets
> `output_to_console_ = value`; on all other builds it ignores `value` and does
> nothing. Returns void.

> [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.set-split-characters-fn]
> void

> [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.set-split-characters-fn]
> Setter. Assigns the boolean parameter `splitness` to the member
> `split_characters_`. Returns void.

> [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.set-strict-alphabets-fn]
> void

> [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.set-strict-alphabets-fn]
> Setter aliasing the missing-alphabets warning flag. Assigns the boolean
> parameter `strictness` to the member `warn_missing_alphabets_`. Returns void.

> [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.set-warning-fn]
> void

> [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.set-warning-fn]
> Enables/disables a named warning category. Takes a C-string `warning` and a
> bool `value`, and assigns `value` to the corresponding boolean member based on
> `strcmp`: "-Wone-sided-flags" -> `warn_one_sided_flags_`; "-Wmissing-lexicons"
> -> `warn_missing_lexicons_`; "-Wunused-lexicons" -> `warn_unused_lexicons_`;
> "-Wrepeated-lexicons" -> `warn_repeated_lexicons_`; "-Wmissing-alphabets" ->
> `warn_missing_alphabets_`; "-Wunnecessary-escapes" -> `warn_unnecessary_escapes_`.
> For any other name it prints "unknown warning <warning>\n" to stderr and
> changes nothing. Returns void.

> [spec:hfst:def:lexc-compiler.hfst.lexc.lexc-compiler.warn-about-one-sided-flags-fn]
> void

> [spec:hfst:sem:lexc-compiler.hfst.lexc.lexc-compiler.warn-about-one-sided-flags-fn]
> Callback (static, operating on the global singleton `hfst::lexc::lexc_`) that
> warns when a symbol pair has a flag diacritic on only one side. Takes a
> `std::pair<std::string,std::string> symbol_pair` (first=input, second=output).
> Logic:
> 1. If `FdOperation::is_diacritic(symbol_pair.first)` is true: if first != second
>    (i.e. it's one-sided), format the message "one-sided flag diacritic
>    <first>:<second> [-Wone-sided-flags]" into a heap buffer. If both
>    `lexc_->warn_one_sided_flags_` and `lexc_->treat_warnings_as_errors_` are
>    set, call `error_at_current_token(0,0,errm)` and set `lexc_->parseErrors_=true`.
>    If `lexc_->warn_one_sided_flags_` is set, call `warning_at_current_token(0,0,errm)`.
>    Free the buffer. Then return (early) regardless of whether first==second.
> 2. Else if `FdOperation::is_diacritic(symbol_pair.second)` is true: same as
>    above (format the same message, the error/warning emission gated identically
>    on `warn_one_sided_flags_` and `treat_warnings_as_errors_`, free the buffer).
> Returns void. Note the buffer is allocated with malloc sized roughly
> first.length()+second.length()+128; messages are emitted via the lexer's
> current-token error/warning reporting functions.

> [spec:hfst:def:lexc-compiler.hlexclex-destroy-fn]
> extern int hlexclex_destroy()

> [spec:hfst:sem:lexc-compiler.hlexclex-destroy-fn]
> External declaration only (no body in this translation unit):
> `extern int hlexclex_destroy()`. It is the flex-generated lexer cleanup
> routine (`yylex_destroy` renamed via the `hlexc` prefix) that frees the
> scanner's internal buffers and state. The compiler calls it before each parse
> to reset lexer state. The definition lives in the generated flex scanner, not
> here.

> [spec:hfst:def:lexc-compiler.hlexcparse-fn]
> extern int hlexcparse()

> [spec:hfst:sem:lexc-compiler.hlexcparse-fn]
> External declaration only (no body in this translation unit):
> `extern int hlexcparse()`. It is the bison/yacc-generated parser entry point
> (`yyparse` renamed via the `hlexc` prefix). When called it parses the input
> currently set on the global lexer input `hlexcin`, driving the lexc grammar
> actions which feed entries into the global `LexcCompiler` singleton
> (`hfst::lexc::lexc_`). Returns the standard yacc parse status (0 on success).
> Error count accumulates in the related global `hlexcnerrs`. The definition
> lives in the generated parser, not here.

> [spec:hfst:def:lexc-compiler.main-fn]
> int

> [spec:hfst:sem:lexc-compiler.main-fn]
> Standalone unit-test driver, compiled only when `DEBUG_MAIN` is defined
> (i.e. in the `#else` branch of `#ifndef DEBUG_MAIN`). It exercises the
> LexcCompiler API for each available backend (`SFST_TYPE`,
> `TROPICAL_OPENFST_TYPE`, `FOMA_TYPE`), gated by the `HAVE_SFST`/`HAVE_OPENFST`/
> `HAVE_FOMA` (and `HAVE_OFST`) macros, printing progress to std::cout. Steps:
> 1. Print a header and construct compilers: a default `LexcCompiler`, and one
>    per available backend with `setAllowMultipleSublexiconDefinitions(true)`.
> 2. Call `setVerbosity(1)` then `setVerbosity(2)` on each backend compiler.
> 3. Ensure test input files exist: open "LexcCompiler_test.lexc" for reading;
>    if NULL, create it for writing (asserting success) and write a fixed lexc
>    source containing a Definitions block (def1..def6) and a "LEXICON Root"
>    with entries "cat # ;" and "dog Plural ;", then close it. Do the same for
>    "LexcCompiler_test2.lexc" with contents "LEXICON Plural\ns # ;\n".
> 4. Parse: for each backend, open "LexcCompiler_test.lexc" and call
>    `parse(FILE*)` then `fclose`, and call `parse("LexcCompiler_test2.lexc")`.
> 5. Add multichar alphabets "foo" and "bar" via `addAlphabet`.
> 6. Call `setCurrentLexiconName("Root")`.
> 7. Add entries: `addStringEntry("dog","#",0)`,
>    `addStringPairEntry("banana","apple","#",0)`,
>    `addXreEntry("f i:o 0:u g h t","#",0)`.
> 8. Add definition `addXreDefinition("Vowels","a | e | i | o | u | y")`.
> 9. Set `setInitialLexiconName("Root")`.
> 10. Compile each: `compiled = compileLexical()`, assert non-NULL, then
>     `delete compiled`.
> 11. Return `EXIT_SUCCESS`.

> [spec:hfst:def:lexc-compiler.should-colourise-fn]
> static bool

> [spec:hfst:sem:lexc-compiler.should-colourise-fn]
> Static helper deciding whether to emit ANSI colour escape codes. Calls
> `isatty(1)` (file descriptor 1 = stdout); returns true if stdout is a
> terminal, false otherwise. No side effects. (The trailing unreachable
> `return false;` is dead code after the if/else.)

