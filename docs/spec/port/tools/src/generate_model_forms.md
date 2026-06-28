# tools/src/generate_model_forms.cc, tools/src/generate_model_forms.h

> [spec:hfst:def:generate-model-forms.contains-analysis-symbols-fn]
> bool

> [spec:hfst:sem:generate-model-forms.contains-analysis-symbols-fn]
> Given a tokenized word form (a sequence of symbol strings), return true if
> any single symbol looks like an analysis tag, i.e. its byte length is
> greater than 1 and its first byte is '[' and its last byte is ']'.
> Otherwise return false. Comparison is done on raw bytes, mirroring C++
> 'std::string' indexing.

> [spec:hfst:def:generate-model-forms.generate-word-forms-fn]
> StringVector

> [spec:hfst:sem:generate-model-forms.generate-word-forms-fn]
> Look up 'analysis' in 'form_generator' (flag diacritics handled as in plain
> lookup), obtaining the paths in weight-then-string order. Walk the paths
> keeping a running count 'num' starting at 1 and a 'best_weight' initialised
> to -1. For each path: stop once 'num' exceeds 'max_generated_forms'; on the
> first path record its weight as 'best_weight'; stop as soon as the path's
> weight minus 'best_weight' is greater than or equal to 'generate_threshold';
> skip (without incrementing 'num') any path whose output contains an analysis
> symbol (see contains-analysis-symbols-fn); otherwise insert the path output
> reversed (it is produced reversed) into a set of accepted forms and
> increment 'num'. The set dedups and orders forms. Then concatenate the
> accepted forms into one flat symbol vector, inserting a single ", " symbol
> between consecutive forms. If no forms were accepted, return the single
> symbol "<no word forms>". The looked-up path collection is freed before
> returning.

> [spec:hfst:def:generate-model-forms.get-alphabet-string-tokenizer-fn]
> HfstTokenizer

> [spec:hfst:sem:generate-model-forms.get-alphabet-string-tokenizer-fn]
> Build a tokenizer that knows the multi-character symbols of 'fst'. Make a
> deep copy of 'fst', convert the copy to the tropical OpenFST type (a
> workaround because optimized-lookup transducers do not expose get_alphabet),
> read the copy's alphabet symbol set, and register every alphabet symbol as a
> multi-character symbol on a fresh tokenizer. The original 'fst' is left
> unchanged. Return the tokenizer.

> [spec:hfst:def:generate-model-forms.get-analysis-prefix-fn]
> StringVector

> [spec:hfst:sem:generate-model-forms.get-analysis-prefix-fn]
> Given a reversed analysis (symbol vector), return the shortest prefix of the
> non-reversed analysis that ends at the first category symbol. Implemented by
> iterating 'reversed_analysis' from its last element toward its first,
> appending each visited symbol to an accumulator; as soon as a category
> symbol (one starting with the category-symbol prefix "[GUESS_CATEGORY=") is
> seen, return the accumulator reversed. If no category symbol is found
> (should not happen for valid analyses), return the accumulator as-is.

> [spec:hfst:def:generate-model-forms.get-guesses-fn]
> StringVectorVector

> [spec:hfst:sem:generate-model-forms.get-guesses-fn]
> Tokenize 'word_form' into a one-level symbol vector using 'tokenizer'
> (without splitting multi-character symbols) and reverse it. Look the reversed
> vector up in 'guesser' with flag-diacritic-aware lookup. Iterate the
> resulting paths in order, collecting each path's output symbol vector, until
> 'number_of_guesses' guesses have been collected (running count starts at 1
> and the loop stops once it exceeds 'number_of_guesses'). Free the path
> collection and return the collected guesses.

> [spec:hfst:def:generate-model-forms.get-model-forms-fn]
> StringVectorVector

> [spec:hfst:sem:generate-model-forms.get-model-forms-fn]
> Compute the reversed analysis prefix of 'reversed_analysis' (see
> get-analysis-prefix-fn). For each model form in 'model_forms', form a model
> analysis by appending that prefix to the model form, then generate its word
> forms via generate-word-forms-fn (passing 'form_generator',
> 'max_generated_forms' and 'generate_threshold'). Return the list of generated
> word-form vectors, one per model form, in order.

> [spec:hfst:def:generate-model-forms.get-paradigms-fn]
> StringVectorVector

> [spec:hfst:sem:generate-model-forms.get-paradigms-fn]
> For each analysis guess in 'guesses', build one paradigm row. First compute
> the guess's model forms via get-model-forms-fn (using 'generator',
> 'model_forms', 'number_of_generated_forms' and 'generate_threshold'). Then
> assemble a flat symbol vector starting with the original 'word_form' symbol,
> a "\t" tab symbol, then the analysis guess reversed (back to display order).
> For every generated model-form vector, append a "\t" tab symbol followed by
> that vector's symbols. Collect one such paradigm vector per guess, in order,
> and return them.

> [spec:hfst:def:generate-model-forms.invalid-model-file]
> struct InvalidModelFile

> [spec:hfst:def:generate-model-forms.invalid-model-line]
> struct InvalidModelLine {
>   std::string line;
> }

> [spec:hfst:def:generate-model-forms.invalid-model-line.invalid-model-line-fn]
> InvalidModelLine::InvalidModelLine(const std::string &line) : line(line)

> [spec:hfst:sem:generate-model-forms.invalid-model-line.invalid-model-line-fn]
> Construct an InvalidModelLine error value, storing the offending model-file
> line verbatim in its 'line' field.

> [spec:hfst:def:generate-model-forms.join-fn]
> StringVector

> [spec:hfst:sem:generate-model-forms.join-fn]
> Return a new symbol vector containing all symbols of 'sv1' in order followed
> by all symbols of 'sv2' in order. 'sv1' is taken by value and extended;
> 'sv2' is left unchanged.

> [spec:hfst:def:generate-model-forms.main-fn]
> int

> [spec:hfst:sem:generate-model-forms.main-fn]
> Test/standalone entry point (compiled only under MAIN_TEST). Open the FST
> input file named by argv[1] and read a guesser transducer followed by a
> generator transducer. Build a tokenizer from the generator's alphabet
> (get-alphabet-string-tokenizer-fn). Read the model analyses from the file
> named by argv[2] (read-model-forms-fn). Then read input word forms from
> standard input line by line until EOF: for each line, compute up to
> MAX_ANALYSES guesses (get-guesses-fn), expand them into paradigms
> (get-paradigms-fn) and print each resulting paradigm row to standard output.
> Not ported to Rust (out of the helper's scope).

> [spec:hfst:def:generate-model-forms.read-model-form-fn]
> StringVector

> [spec:hfst:sem:generate-model-forms.read-model-form-fn]
> Read a single line from the input stream. If the line does not begin with
> MODEL_FORM_PREFIX (the empty string, so this never fails in practice), raise
> an InvalidModelLine carrying the line. Otherwise strip the prefix, tokenize
> the remaining model-form text into a one-level symbol vector (without
> splitting multi-character symbols) and reverse it. Return the reversed
> tokenized model form.

> [spec:hfst:def:generate-model-forms.read-model-forms-fn]
> StringVectorVector

> [spec:hfst:sem:generate-model-forms.read-model-forms-fn]
> Open the model-form file named 'model_form_filename' (a failed open behaves
> like an empty stream). If the stream is already at end of file, raise
> InvalidModelFile. Otherwise repeatedly read one model form per line
> (read-model-form-fn) until end of file, collecting them in order, and return
> the collection.

> [spec:hfst:def:generate-model-forms.split-fn]
> StringPair

> [spec:hfst:sem:generate-model-forms.split-fn]
> Find the first occurrence of 'separator' in 'line'. Return a pair whose first
> element is the substring of 'line' before that position and whose second
> element is the substring of 'line' starting one byte after that position
> (i.e. skipping a single-byte separator). Mirrors the C++ 'std::string::find'
> plus 'substr' behaviour. This helper is retained from the source but is not
> used elsewhere in the module.

> [spec:hfst:def:generate-model-forms.string-vector-set]
> typedef std::set<StringVector> StringVectorSet

> [spec:hfst:def:generate-model-forms.string-vector-vector]
> typedef std::vector<StringVector> StringVectorVector
