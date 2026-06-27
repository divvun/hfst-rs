# tools/src/hfst-optimized-lookup.cc, tools/src/hfst-optimized-lookup.h

> [spec:hfst:def:hfst-optimized-lookup.arc-number]
> typedef unsigned int ArcNumber

> [spec:hfst:def:hfst-optimized-lookup.colour-tristate]
> enum colour_tristate {
>   COLOUR_NEVER;
>   COLOUR_ALWAYS;
>   COLOUR_AUTO;
> }

> [spec:hfst:def:hfst-optimized-lookup.display-map]
> typedef std::map<std::string, Weight> DisplayMap

> [spec:hfst:def:hfst-optimized-lookup.display-multi-map]
> typedef std::multimap<Weight, std::string> DisplayMultiMap

> [spec:hfst:def:hfst-optimized-lookup.display-set]
> typedef std::set<std::string> DisplaySet

> [spec:hfst:def:hfst-optimized-lookup.display-vector]
> typedef std::vector<std::string> DisplayVector

> [spec:hfst:def:hfst-optimized-lookup.encoder]
> class Encoder {
>   SymbolNumber number_of_input_symbols;
>   LetterTrie letters;
>   SymbolNumberVector ascii_symbols;
> }

> [spec:hfst:def:hfst-optimized-lookup.encoder.encoder-fn]
> Encoder(KeyTable * kt, SymbolNumber input_symbol_count)

> [spec:hfst:sem:hfst-optimized-lookup.encoder.encoder-fn]
> Construct an Encoder for input_symbol_count input symbols: initialise an empty
> LetterTrie and an ascii_symbols vector of UCHAR_MAX entries all NO_SYMBOL_NUMBER,
> then call read_input_symbols(kt) to populate both from the key table.

> [spec:hfst:def:hfst-optimized-lookup.encoder.find-key-fn]
> SymbolNumber

> [spec:hfst:sem:hfst-optimized-lookup.encoder.find-key-fn]
> Tokenise the next symbol at the cursor **p. If ascii_symbols[**p] is
> NO_SYMBOL_NUMBER, delegate to the letter trie's find_key (multi-byte / UTF-8
> path). Otherwise the first byte is a known single-byte ASCII symbol: advance the
> cursor by one and return that symbol number.

> [spec:hfst:def:hfst-optimized-lookup.encoder.read-input-symbols-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.encoder.read-input-symbols-fn]
> For each input symbol k in [0, number_of_input_symbols): let p be its key
> string. If p is exactly one ASCII byte (length 1, value <=127) and the trie has
> no longer symbol starting with that byte, record k in ascii_symbols[*p] as a
> fast single-byte path. If p is longer than one byte but starts with an ASCII
> byte that currently has an ascii_symbols entry, clear that entry
> (NO_SYMBOL_NUMBER) so the longer symbol is not shadowed. In all cases add p to
> the letter trie via add_string(p, k).

> [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-operation]
> class FlagDiacriticOperation {
>   FlagDiacriticOperator operation;
>   SymbolNumber feature;
>   ValueNumber value;
> }

> [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-operation.feature-fn]
> SymbolNumber Feature(void)

> [spec:hfst:sem:hfst-optimized-lookup.flag-diacritic-operation.feature-fn]
> Return the stored feature symbol number.

> [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-operation.flag-diacritic-operation-fn]
> FlagDiacriticOperation(FlagDiacriticOperator op, SymbolNumber feat, ValueNumber val)

> [spec:hfst:sem:hfst-optimized-lookup.flag-diacritic-operation.flag-diacritic-operation-fn]
> Construct a FlagDiacriticOperation from an operator op, a feature symbol
> number feat, and a value number val, storing each in the corresponding field.
> The dummy/default form instead stores operation=P, feature=NO_SYMBOL_NUMBER
> (USHRT_MAX) and value=0, which marks a non-flag entry (see is-flag).

> [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-operation.is-flag-fn]
> bool isFlag(void)

> [spec:hfst:sem:hfst-optimized-lookup.flag-diacritic-operation.is-flag-fn]
> Return true iff the stored feature is not NO_SYMBOL_NUMBER. A dummy operation
> (constructed with feature=NO_SYMBOL_NUMBER) is therefore not a flag; a real
> flag-diacritic operation always has a real feature number.

> [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-operation.operation-fn]
> FlagDiacriticOperator Operation(void)

> [spec:hfst:sem:hfst-optimized-lookup.flag-diacritic-operation.operation-fn]
> Return the stored FlagDiacriticOperator (one of P, N, R, D, C, U).

> [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-operation.print-fn]
> void print(void)

> [spec:hfst:sem:hfst-optimized-lookup.flag-diacritic-operation.print-fn]
> Debug-only (OL_FULL_DEBUG): print the operation, feature and value separated
> by tabs and followed by a newline to standard output.

> [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-operation.value-fn]
> ValueNumber Value(void)

> [spec:hfst:sem:hfst-optimized-lookup.flag-diacritic-operation.value-fn]
> Return the stored value number.

> [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-operator]
> enum FlagDiacriticOperator {
>   P;
>   N;
>   R;
>   D;
>   C;
>   U;
> }

> [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-state]
> typedef std::vector<ValueNumber> FlagDiacriticState

> [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-state-stack]
> typedef std::vector<FlagDiacriticState> FlagDiacriticStateStack

> [spec:hfst:def:hfst-optimized-lookup.header-flag]
> enum HeaderFlag {
>   Weighted;
>   Deterministic;
>   Input_deterministic;
>   Minimized;
>   Cyclic;
>   Has_epsilon_epsilon_transitions;
>   Has_input_epsilon_transitions;
>   Has_input_epsilon_cycles;
>   Has_unweighted_input_epsilon_cycles;
> }

> [spec:hfst:def:hfst-optimized-lookup.header-parsing-exception]
> class HeaderParsingException: public std::exception

> [spec:hfst:def:hfst-optimized-lookup.header-parsing-exception.what-fn]
> virtual const char* what() const throw()

> [spec:hfst:sem:hfst-optimized-lookup.header-parsing-exception.what-fn]
> Return the fixed C-string "Parsing error while reading header". This is the
> exception's diagnostic message, used when an optimized-lookup transducer header
> cannot be parsed.

> [spec:hfst:def:hfst-optimized-lookup.index-table-reader]
> class IndexTableReader {
>   TransitionTableIndex number_of_table_entries;
>   char * TableIndices;
>   TransitionIndexVector indices;
>   size_t table_size;
> }

> [spec:hfst:def:hfst-optimized-lookup.index-table-reader-w]
> class IndexTableReaderW {
>   TransitionTableIndex number_of_table_entries;
>   char * TableIndices;
>   TransitionWIndexVector indices;
>   size_t table_size;
> }

> [spec:hfst:def:hfst-optimized-lookup.index-table-reader-w.at-fn]
> TransitionWIndex * at(TransitionTableIndex i)

> [spec:hfst:sem:hfst-optimized-lookup.index-table-reader-w.at-fn]
> Return the TransitionWIndex at position i.

> [spec:hfst:def:hfst-optimized-lookup.index-table-reader-w.get-finality-fn]
> bool get_finality(TransitionTableIndex i)

> [spec:hfst:sem:hfst-optimized-lookup.index-table-reader-w.get-finality-fn]
> Return whether weighted index i is final.

> [spec:hfst:def:hfst-optimized-lookup.index-table-reader-w.get-index-vector-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.index-table-reader-w.get-index-vector-fn]
> Decode the raw weighted index table into TransitionWIndex objects: per entry i,
> input symbol at i*SIZE and target index at + sizeof(SymbolNumber).

> [spec:hfst:def:hfst-optimized-lookup.index-table-reader-w.index-table-reader-w-fn]
> IndexTableReaderW(FILE * f,

> [spec:hfst:sem:hfst-optimized-lookup.index-table-reader-w.index-table-reader-w-fn]
> Read index_count weighted index entries from file f into a byte buffer of
> index_count*TransitionWIndex::SIZE bytes, then decode via get_index_vector.

> [spec:hfst:def:hfst-optimized-lookup.index-table-reader.at-fn]
> TransitionIndex * at(TransitionTableIndex i)

> [spec:hfst:sem:hfst-optimized-lookup.index-table-reader.at-fn]
> Return the TransitionIndex at position i.

> [spec:hfst:def:hfst-optimized-lookup.index-table-reader.get-finality-fn]
> bool get_finality(TransitionTableIndex i)

> [spec:hfst:sem:hfst-optimized-lookup.index-table-reader.get-finality-fn]
> Return whether the index entry at i is final (delegates to its final()).

> [spec:hfst:def:hfst-optimized-lookup.index-table-reader.get-index-vector-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.index-table-reader.get-index-vector-fn]
> Decode the raw index table into a vector of TransitionIndex objects. For each
> entry i, the input symbol is the SymbolNumber at byte offset i*SIZE and the
> target index is the TransitionTableIndex at offset i*SIZE + sizeof(SymbolNumber).

> [spec:hfst:def:hfst-optimized-lookup.index-table-reader.index-table-reader-fn]
> IndexTableReader(FILE * f,

> [spec:hfst:sem:hfst-optimized-lookup.index-table-reader.index-table-reader-fn]
> Read index_count index-table entries from file f. Allocate a byte buffer of
> index_count*TransitionIndex::SIZE bytes, fread the whole table into it, then call
> get_index_vector to decode the bytes into TransitionIndex objects.

> [spec:hfst:def:hfst-optimized-lookup.key-table]
> typedef std::map<SymbolNumber,const char*> KeyTable

> [spec:hfst:def:hfst-optimized-lookup.letter-trie]
> class LetterTrie {
>   LetterTrieVector letters;
>   SymbolNumberVector symbols;
> }

> [spec:hfst:def:hfst-optimized-lookup.letter-trie-vector]
> typedef std::vector<LetterTrie*> LetterTrieVector

> [spec:hfst:def:hfst-optimized-lookup.letter-trie.add-string-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.letter-trie.add-string-fn]
> Insert the NUL-terminated byte string p with associated symbol_key. If p is a
> single byte (i.e. *(p+1)==0), record symbol_key in symbols[*p] and return.
> Otherwise ensure a child trie exists at letters[*p] (creating one if absent) and
> recurse with p+1, so each successive byte descends one trie level.

> [spec:hfst:def:hfst-optimized-lookup.letter-trie.find-key-fn]
> SymbolNumber

> [spec:hfst:sem:hfst-optimized-lookup.letter-trie.find-key-fn]
> Greedily match the longest symbol at the cursor **p. Take the current byte
> *old_p and advance the cursor one byte. If there is no child trie for that byte,
> return symbols[old byte] (the symbol terminating here, possibly NO_SYMBOL_NUMBER).
> Otherwise recurse into the child; if the recursion fails (NO_SYMBOL_NUMBER),
> back the cursor up by one and fall back to symbols[old byte]; otherwise return
> the deeper match. Net effect: the cursor is advanced past exactly the bytes of
> the matched symbol.

> [spec:hfst:def:hfst-optimized-lookup.letter-trie.has-key-starting-with-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.letter-trie.has-key-starting-with-fn]
> Return true iff this node has a non-NULL child trie at index c, i.e. some
> longer symbol begins with byte c.

> [spec:hfst:def:hfst-optimized-lookup.letter-trie.letter-trie-fn]
> LetterTrie(void)

> [spec:hfst:sem:hfst-optimized-lookup.letter-trie.letter-trie-fn]
> Construct an empty LetterTrie node: a vector of UCHAR_MAX child pointers all set
> to NULL, and a parallel vector of UCHAR_MAX symbol numbers all set to
> NO_SYMBOL_NUMBER.

> [spec:hfst:def:hfst-optimized-lookup.main-fn]
> int

> [spec:hfst:sem:hfst-optimized-lookup.main-fn]
> Program entry point. Run getopt_long over the option set
> (hVvqsewb:t:uxfn:p:: plus long forms): -h prints usage and exits success; -V
> prints version and exits success; -v sets verbose; -q/-s clear verbose and set
> displayWeights; -e sets echoInputs; -w sets displayWeights; -u sets
> displayUnique; -b parses a non-negative beam (else fail); -t parses a
> non-negative time-cutoff (else fail); -n parses a positive maxAnalyses (else
> fail); -x sets xerox output; -f sets beFast; -p[=STREAM] sets pipe input/output
> (both/input/output, default both; unknown STREAM fails); any other option prints
> 'Invalid option', the short help, and fails. After options, exactly one
> positional argument is required: more than one is an error, none is an error;
> otherwise fopen it in binary mode (failure prints a message and returns 1) and
> call setup on the file.

> [spec:hfst:def:hfst-optimized-lookup.operation-vector]
> typedef std::vector<FlagDiacriticOperation> OperationVector

> [spec:hfst:def:hfst-optimized-lookup.output-type]
> enum OutputType {
>   HFST;
>   xerox;
> }

> [spec:hfst:def:hfst-optimized-lookup.print-short-help-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.print-short-help-fn]
> Call print_usage and return true.

> [spec:hfst:def:hfst-optimized-lookup.print-usage-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.print-usage-fn]
> Print the multi-line usage/help text (program name, the left-to-right lookup
> note, every option with its description, the constraints on N/B/S, the
> pipe-mode STREAM explanation, and the bug-report address) to standard output.
> Return true.

> [spec:hfst:def:hfst-optimized-lookup.print-version-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.print-version-fn]
> Print a blank line, the PACKAGE_STRING version banner, and the University of
> Helsinki copyright line to standard output. Return true.

> [spec:hfst:def:hfst-optimized-lookup.run-transducer-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.run-transducer-fn]
> Drive interactive lookup with a generic transducer T. Allocate an input_string
> buffer of NO_SYMBOL_NUMBER. Loop reading one line at a time from standard input
> (up to MAX_IO_STRING, newline stripped); stop at EOF. If echoInputsFlag, echo
> the line. Tokenise the line left to right via T.find_next_key, storing symbol
> numbers into input_string; if any byte fails to tokenise (NO_SYMBOL_NUMBER), mark
> the line failed. On failure, in xerox mode print word\tword\t+? and blank lines
> and continue to the next line. Otherwise terminate input_string with
> NO_SYMBOL_NUMBER, (re)start the per-input clock when time_cutoff is set, call
> T.analyze(input_string), then T.printAnalyses(line).

> [spec:hfst:def:hfst-optimized-lookup.setup-fn]
> int

> [spec:hfst:sem:hfst-optimized-lookup.setup-fn]
> Read and dispatch an optimized-lookup transducer from file f. Parse the
> TransducerHeader then the TransducerAlphabet(symbol_count); on a
> HeaderParsingException print an 'Invalid transducer header' / 'must be in
> optimized lookup format' message and return EXIT_FAILURE. Warn to stderr if the
> header reports input epsilon cycles (the engine does not handle them and may
> segfault). Then choose the transducer class by three booleans and run it via
> runTransducer: state size 0 means no flag diacritics, otherwise flag-diacritic
> variants; weighted vs unweighted; unique vs all analyses. The eight combinations
> map to Transducer / TransducerUniq / TransducerW / TransducerWUniq / TransducerFd
> / TransducerFdUniq / TransducerWFd / TransducerWFdUniq. Return 0 on success.

> [spec:hfst:def:hfst-optimized-lookup.state-id-number]
> typedef unsigned int StateIdNumber

> [spec:hfst:def:hfst-optimized-lookup.symbol-number]
> typedef unsigned short SymbolNumber

> [spec:hfst:def:hfst-optimized-lookup.symbol-number-vector]
> typedef std::vector<SymbolNumber> SymbolNumberVector

> [spec:hfst:def:hfst-optimized-lookup.transducer]
> class Transducer {
>   TransducerHeader header;
>   TransducerAlphabet alphabet;
>   KeyTable * keys;
>   IndexTableReader index_reader;
>   TransitionTableReader transition_reader;
>   Encoder encoder;
>   DisplayVector display_vector;
>   SymbolNumber * output_string;
>   static const TransitionTableIndex START_INDEX = 0;
>   std::vector<const char*> symbol_table;
>   TransitionIndexVector &indices;
>   TransitionVector &transitions;
> }

> [spec:hfst:def:hfst-optimized-lookup.transducer-alphabet]
> class TransducerAlphabet {
>   SymbolNumber number_of_symbols;
>   KeyTable * kt;
>   OperationVector operations;
>   char * line;
>   std::map<std::string, SymbolNumber> feature_bucket;
>   std::map<std::string, ValueNumber> value_bucket;
>   ValueNumber val_num;
>   SymbolNumber feat_num;
> }

> [spec:hfst:def:hfst-optimized-lookup.transducer-alphabet.get-key-table-fn]
> KeyTable * get_key_table(void)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-alphabet.get-key-table-fn]
> Return a pointer to the key table (the SymbolNumber -> symbol-string map).

> [spec:hfst:def:hfst-optimized-lookup.transducer-alphabet.get-next-symbol-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-alphabet.get-next-symbol-fn]
> Read the next symbol string for key k from file f: read bytes until a NUL
> terminator (EOF mid-symbol is a fatal corrupt-file error -> print to stderr and
> exit(1)). If the assembled string is at least 5 bytes long and looks like a
> flag diacritic (first byte '@', last byte '@', third byte '.'), parse it: the
> second byte selects the operator (P/N/R/D/C/U), the characters after index 3 up
> to a '.' or '@' are the feature name, and (if a '.' was seen) the characters
> after it up to '@' are the value name. Intern the feature name into
> feature_bucket (assigning the next feat_num if new) and the value name into
> value_bucket (assigning the next val_num if new), then push a
> FlagDiacriticOperation(op, feature_bucket[feat], value_bucket[val]) onto
> operations and store "" as the key string for k (flags are not printed).
> Otherwise push a dummy FlagDiacriticOperation onto operations and store the
> literal symbol string as the key string for k.

> [spec:hfst:def:hfst-optimized-lookup.transducer-alphabet.get-operation-vector-fn]
> OperationVector get_operation_vector(void)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-alphabet.get-operation-vector-fn]
> Return (a copy of) the operations vector: one FlagDiacriticOperation per
> symbol, dummy for non-flag symbols and a real operation for flag diacritics.

> [spec:hfst:def:hfst-optimized-lookup.transducer-alphabet.get-state-size-fn]
> SymbolNumber get_state_size(void)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-alphabet.get-state-size-fn]
> Return the number of distinct flag-diacritic features seen, i.e. the size of
> feature_bucket. Zero means the transducer has no flag diacritics.

> [spec:hfst:def:hfst-optimized-lookup.transducer-alphabet.transducer-alphabet-fn]
> TransducerAlphabet(FILE * f,SymbolNumber symbol_number)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-alphabet.transducer-alphabet-fn]
> Construct the alphabet for symbol_number symbols read from file f. Initialise
> feat_num=0, val_num=1, and seed value_bucket with the empty string mapped to 0
> (the neutral value). For each k in [0, number_of_symbols) call
> get_next_symbol(f, k) to populate the key table and operations vector. Finally
> override the key string for symbol 0 with "" (the first symbol is assumed to be
> epsilon, which must not be printed) and release the scratch line buffer.

> [spec:hfst:def:hfst-optimized-lookup.transducer-fd]
> class TransducerFd: public Transducer {
>   FlagDiacriticStateStack statestack;
>   OperationVector operations;
> }

> [spec:hfst:def:hfst-optimized-lookup.transducer-fd-uniq]
> class TransducerFdUniq: public TransducerFd {
>   DisplaySet display_vector;
> }

> [spec:hfst:def:hfst-optimized-lookup.transducer-fd-uniq.note-analysis-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-fd-uniq.note-analysis-fn]
> Concatenate the output-buffer symbols into a string and insert it into the
> DisplaySet, deduplicating.

> [spec:hfst:def:hfst-optimized-lookup.transducer-fd-uniq.print-analyses-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-fd-uniq.print-analyses-fn]
> Same emission logic as TransducerUniq::printAnalyses, over this class's
> deduplicating DisplaySet.

> [spec:hfst:def:hfst-optimized-lookup.transducer-fd-uniq.transducer-fd-uniq-fn]
> TransducerFdUniq(FILE * f, TransducerHeader h, TransducerAlphabet a)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-fd-uniq.transducer-fd-uniq-fn]
> Construct a TransducerFdUniq: build the base TransducerFd(f,h,a) and an empty
> deduplicating DisplaySet.

> [spec:hfst:def:hfst-optimized-lookup.transducer-fd.push-state-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.transducer-fd.push-state-fn]
> Attempt to apply flag-diacritic operation op to the top of the state stack,
> pushing a new state on success and returning whether it succeeded. P (positive
> set): push a copy and set feature to value. N (negative set): push a copy and
> set feature to -value. R (require): with value 0, succeed (pushing a copy) iff
> the feature is currently nonzero; with a nonzero value, succeed iff the feature
> equals value. D (disallow): with value 0, succeed iff the feature is currently
> zero; with a nonzero value, fail iff the feature equals value else succeed.
> C (clear): push a copy and set feature to 0. U (unify): succeed iff the feature
> is unset (0), already equals value, or is negatively set to something other than
> value, in which case push a copy and set feature to value. On success a state is
> pushed; on failure none is and false is returned.

> [spec:hfst:def:hfst-optimized-lookup.transducer-fd.transducer-fd-fn]
> TransducerFd(FILE * f, TransducerHeader h, TransducerAlphabet a)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-fd.transducer-fd-fn]
> Construct a TransducerFd: build the base Transducer(f,h,a), initialise the flag
> diacritic state stack with a single zero-filled state of width
> alphabet.get_state_size(), and copy the alphabet's operation vector.

> [spec:hfst:def:hfst-optimized-lookup.transducer-fd.try-epsilon-transitions-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-fd.try-epsilon-transitions-fn]
> Like the base try_epsilon_transitions but also honouring flag diacritics. Loop
> from i: if the transition input is 0 (epsilon) emit its output and recurse, then
> advance i. If the transition input is a real flag-diacritic symbol, evaluate it
> via PushState; if allowed, emit the output, recurse, then pop the pushed state;
> advance i regardless. On reaching any non-epsilon non-flag transition, return.

> [spec:hfst:def:hfst-optimized-lookup.transducer-header]
> class TransducerHeader {
>   SymbolNumber number_of_symbols;
>   SymbolNumber number_of_input_symbols;
>   TransitionTableIndex size_of_transition_index_table;
>   TransitionTableIndex size_of_transition_target_table;
>   StateIdNumber number_of_states;
>   TransitionNumber number_of_transitions;
>   bool weighted;
>   bool deterministic;
>   bool input_deterministic;
>   bool minimized;
>   bool cyclic;
>   bool has_epsilon_epsilon_transitions;
>   bool has_input_epsilon_transitions;
>   bool has_input_epsilon_cycles;
>   bool has_unweighted_input_epsilon_cycles;
> }

> [spec:hfst:def:hfst-optimized-lookup.transducer-header.index-table-size-fn]
> TransitionTableIndex index_table_size(void)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-header.index-table-size-fn]
> Return size_of_transition_index_table (number of entries in the index table).

> [spec:hfst:def:hfst-optimized-lookup.transducer-header.input-symbol-count-fn]
> SymbolNumber input_symbol_count(void)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-header.input-symbol-count-fn]
> Return number_of_input_symbols (the count of input-side symbols).

> [spec:hfst:def:hfst-optimized-lookup.transducer-header.probe-flag-fn]
> bool probe_flag(HeaderFlag flag)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-header.probe-flag-fn]
> Given a HeaderFlag enumerator, return the corresponding stored boolean field
> (Weighted->weighted, Deterministic->deterministic, etc.). Return false for any
> unmatched flag.

> [spec:hfst:def:hfst-optimized-lookup.transducer-header.read-property-fn]
> void read_property(bool &property, FILE * f)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-header.read-property-fn]
> Read one unsigned int (4 bytes, native order) from the file f. Set property to
> false if the value read is 0, otherwise true. The return-after-set means the
> trailing error print/exit is dead code, kept verbatim from the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-header.skip-hfst3-header-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-header.skip-hfst3-header-fn]
> Attempt to consume an optional HFST3 file header. Read bytes one at a time and
> match them against the literal "HFST" followed by a NUL. If the full 5-byte
> sequence "HFST\0" is matched, read an unsigned short remaining_header_len, then
> read one byte that must be NUL (else throw HeaderParsingException), then read
> remaining_header_len bytes into a buffer whose last byte must be NUL (else
> throw). Search that buffer for the substring "type"; if found at position t, the
> substrings "HFST_OL" or "HFST_OLW" must occur at offset t+5 (else throw). If the
> leading sequence did not fully match, push every consumed byte back onto the
> stream with ungetc in reverse order (the non-matching byte first, then each
> matched byte) so the stream is left untouched for the caller.

> [spec:hfst:def:hfst-optimized-lookup.transducer-header.symbol-count-fn]
> SymbolNumber symbol_count(void)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-header.symbol-count-fn]
> Return number_of_symbols (the total symbol count).

> [spec:hfst:def:hfst-optimized-lookup.transducer-header.target-table-size-fn]
> TransitionTableIndex target_table_size(void)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-header.target-table-size-fn]
> Return size_of_transition_target_table (number of entries in the transition
> target table).

> [spec:hfst:def:hfst-optimized-lookup.transducer-header.transducer-header-fn]
> TransducerHeader(FILE * f)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-header.transducer-header-fn]
> Construct a TransducerHeader from file f: first call skip_hfst3_header(f) to
> consume any HFST3 wrapper header, then read in order: number_of_input_symbols
> and number_of_symbols (each a SymbolNumber / unsigned short),
> size_of_transition_index_table and size_of_transition_target_table (each a
> TransitionTableIndex / unsigned int), number_of_states (StateIdNumber) and
> number_of_transitions (TransitionNumber). Then read nine boolean properties via
> read_property in this exact order: weighted, deterministic, input_deterministic,
> minimized, cyclic, has_epsilon_epsilon_transitions, has_input_epsilon_transitions,
> has_input_epsilon_cycles, has_unweighted_input_epsilon_cycles.

> [spec:hfst:def:hfst-optimized-lookup.transducer-uniq]
> class TransducerUniq: public Transducer {
>   DisplaySet display_vector;
> }

> [spec:hfst:def:hfst-optimized-lookup.transducer-uniq.note-analysis-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-uniq.note-analysis-fn]
> Concatenate the output-buffer symbols (up to NO_SYMBOL_NUMBER) into a string and
> insert it into the DisplaySet, automatically deduplicating.

> [spec:hfst:def:hfst-optimized-lookup.transducer-uniq.print-analyses-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-uniq.print-analyses-fn]
> Like Transducer::printAnalyses but over the deduplicating DisplaySet: in xerox
> mode with an empty set print prepend\tprepend\t+? and two newlines; otherwise
> print up to maxAnalyses set entries (each preceded by prepend\t in xerox mode
> and followed by a newline), clear the set, and print a trailing newline.

> [spec:hfst:def:hfst-optimized-lookup.transducer-uniq.transducer-uniq-fn]
> TransducerUniq(FILE * f, TransducerHeader h, TransducerAlphabet a)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-uniq.transducer-uniq-fn]
> Construct a TransducerUniq: build the base Transducer(f,h,a) and an empty
> DisplaySet display_vector that deduplicates analyses.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w]
> class TransducerW {
>   TransducerHeader header;
>   TransducerAlphabet alphabet;
>   KeyTable * keys;
>   IndexTableReaderW index_reader;
>   TransitionTableReaderW transition_reader;
>   Encoder encoder;
>   DisplayMultiMap display_map;
>   std::vector<SymbolNumber> output_string;
>   static const TransitionTableIndex START_INDEX = 0;
>   std::vector<const char*> symbol_table;
>   TransitionWIndexVector &indices;
>   TransitionWVector &transitions;
>   Weight current_weight;
> }

> [spec:hfst:def:hfst-optimized-lookup.transducer-w-fd]
> class TransducerWFd: public TransducerW {
>   FlagDiacriticStateStack statestack;
>   OperationVector operations;
> }

> [spec:hfst:def:hfst-optimized-lookup.transducer-w-fd-uniq]
> class TransducerWFdUniq: public TransducerWFd {
>   DisplayMap display_map;
> }

> [spec:hfst:def:hfst-optimized-lookup.transducer-w-fd-uniq.note-analysis-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w-fd-uniq.note-analysis-fn]
> Concatenate output symbols into a string; insert/keep the lower-weight
> (string, weight) pair when no entry exists or the stored weight is greater than
> current_weight.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w-fd-uniq.print-analyses-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w-fd-uniq.print-analyses-fn]
> Same emission logic as TransducerWUniq::printAnalyses over this class's
> (string->best weight) map: beam-filter, weight-sort, print up to maxAnalyses
> entries with optional weights, then clear and print a trailing newline.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w-fd-uniq.transducer-w-fd-uniq-fn]
> TransducerWFdUniq(FILE * f, TransducerHeader h, TransducerAlphabet a)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w-fd-uniq.transducer-w-fd-uniq-fn]
> Construct a TransducerWFdUniq: build the base TransducerWFd(f,h,a) and an empty
> DisplayMap keeping the best weight per distinct output.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w-fd.push-state-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w-fd.push-state-fn]
> Identical flag-diacritic state-stack semantics to TransducerFd::PushState
> (P/N/R/D/C/U), operating on this class's weighted state stack: push a derived
> state and return true when the operation is allowed, return false otherwise.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w-fd.transducer-w-fd-fn]
> TransducerWFd(FILE * f, TransducerHeader h, TransducerAlphabet a)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w-fd.transducer-w-fd-fn]
> Construct a TransducerWFd: build the base TransducerW(f,h,a), seed the flag
> diacritic state stack with one zero state of width get_state_size(), and copy
> the alphabet's operation vector.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w-fd.try-epsilon-transitions-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w-fd.try-epsilon-transitions-fn]
> Weighted epsilon walk honouring flag diacritics. Return if i is past the
> transitions vector or the output pointer passes the buffer end. Loop: for an
> epsilon transition (input 0) emit output, add weight, recurse, subtract weight,
> advance. For a real flag-diacritic transition, evaluate PushState; if allowed,
> emit output, add weight, recurse, subtract weight, pop the state; advance.
> Otherwise return.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w-uniq]
> class TransducerWUniq: public TransducerW {
>   DisplayMap display_map;
> }

> [spec:hfst:def:hfst-optimized-lookup.transducer-w-uniq.note-analysis-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w-uniq.note-analysis-fn]
> Concatenate output symbols into a string; if there is no existing entry or the
> stored weight exceeds current_weight, insert/keep the lower (string, weight)
> pair so each distinct output retains its best weight.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w-uniq.print-analyses-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w-uniq.print-analyses-fn]
> Emit weighted unique analyses. In xerox mode with an empty map print
> prepend\tprepend\t+? then a blank line. Otherwise, build a weight-sorted
> multimap from the (string->weight) map, tracking the lowest weight and applying
> the beam filter, then print up to maxAnalyses entries: prepend\t (xerox), the
> string, optional weight, newline; clear the map and print a trailing newline.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w-uniq.transducer-w-uniq-fn]
> TransducerWUniq(FILE * f, TransducerHeader h, TransducerAlphabet a)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w-uniq.transducer-w-uniq-fn]
> Construct a TransducerWUniq: build the base TransducerW(f,h,a) and an empty
> DisplayMap (string -> best weight) used to keep only the lowest-weight analysis
> per output string.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.analyze-fn]
> void analyze(SymbolNumber * input_string)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.analyze-fn]
> Begin a weighted depth-first analysis: call get_analyses with the output buffer
> as both working output and original-output base, starting at START_INDEX.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.final-index-fn]
> bool final_index(TransitionTableIndex i)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.final-index-fn]
> Return whether weighted index i is final.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.final-transition-fn]
> bool final_transition(TransitionTableIndex i)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.final-transition-fn]
> Return whether weighted transition i is final.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.find-index-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.find-index-fn]
> Return early if i is past the indices vector. If the weighted index at i+input
> has input equal to input, follow it into the transition table via
> find_transitions at that entry's target minus TRANSITION_TARGET_TABLE_START.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.find-next-key-fn]
> SymbolNumber find_next_key(char ** p)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.find-next-key-fn]
> Tokenise the next input symbol at cursor **p via the encoder's find_key.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.find-transitions-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.find-transitions-fn]
> Return early if i is past the transitions vector or the output pointer is past
> the output buffer (endless-loop protection). Scan transitions from i while the
> input is not NO_SYMBOL_NUMBER: for each transition whose input equals the query,
> add its weight, emit its output, recurse via get_analyses at its target, then
> subtract the weight; stop at the first mismatching input.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.get-analyses-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.get-analyses-fn]
> Core weighted recursive lookup. Apply the same time-cutoff guard as the
> unweighted version, plus endless-loop protection that returns when the output
> pointer passes the output buffer's end. If i is at or above
> TRANSITION_TARGET_TABLE_START, subtract the offset and work on transitions: try
> epsilon transitions at i+1; if input is exhausted, terminate the output and, if
> transition i exists and is final, add its final weight, note the analysis, and
> subtract the weight back. Otherwise consume one input symbol and call
> find_transitions at i+1. If i is below the offset, work on indices analogously,
> using the index's final weight when noting a final analysis.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.get-final-index-weight-fn]
> Weight get_final_index_weight(TransitionTableIndex i)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.get-final-index-weight-fn]
> Return the final weight stored in weighted index i (its index field reinterpreted
> as a float).

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.get-final-transition-weight-fn]
> Weight get_final_transition_weight(TransitionTableIndex i)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.get-final-transition-weight-fn]
> Return the weight of weighted transition i.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.get-key-table-fn]
> KeyTable * get_key_table(void)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.get-key-table-fn]
> Return the key table pointer.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.note-analysis-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.note-analysis-fn]
> Record one completed weighted analysis: concatenate output-buffer symbols (until
> the output buffer end or NO_SYMBOL_NUMBER) into a string and insert the pair
> (current_weight, string) into the DisplayMultiMap (ordered by weight, duplicates
> allowed).

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.print-analyses-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.print-analyses-fn]
> Emit weighted analyses for input 'prepend'. In xerox mode with an empty
> display_map print prepend\tprepend\t+? then a blank line. Otherwise iterate the
> weight-ordered multimap: track the lowest weight from the first entry; for up to
> maxAnalyses entries that satisfy the beam constraint (beam<0 means unbounded,
> else weight<=lowest+beam) print prepend\t (xerox), the analysis string, and the
> weight when displayWeightsFlag is set, each followed by a newline. Clear the map
> and print a trailing newline.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.set-symbol-table-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.set-symbol-table-fn]
> Populate symbol_table from the key table in key order, so symbol_table[n] is
> the printable string for symbol n.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.transducer-w-fn]
> TransducerW(FILE * f, TransducerHeader h, TransducerAlphabet a)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.transducer-w-fn]
> Construct a TransducerW from file f, header h, alphabet a. Take the key table
> from the alphabet, build the weighted index reader then the weighted transition
> reader (reading index_table_size then target_table_size entries from f), build
> an Encoder for input_symbol_count input symbols, resize output_string to 1000
> NO_SYMBOL_NUMBER entries, bind indices/transitions to the readers' vectors, set
> current_weight to 0, and call set_symbol_table.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.try-epsilon-indices-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.try-epsilon-indices-fn]
> If the weighted index at i has input 0 (epsilon), follow it into the transition
> table via try_epsilon_transitions at its target minus
> TRANSITION_TARGET_TABLE_START.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.try-epsilon-transitions-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.try-epsilon-transitions-fn]
> Weighted epsilon-transition walk. Return immediately if i is past the end of
> the transitions vector. While the transition at i is non-null and has input 0
> (epsilon): emit its output, add its weight to current_weight, recurse into
> get_analyses at its target, subtract the weight back off, and advance i. After
> the loop write NO_SYMBOL_NUMBER at the output position.

> [spec:hfst:def:hfst-optimized-lookup.transducer.analyze-fn]
> void analyze(SymbolNumber * input_string)

> [spec:hfst:sem:hfst-optimized-lookup.transducer.analyze-fn]
> Begin a depth-first analysis of input_string by calling get_analyses with the
> output buffer as both the working output pointer and the original-output base,
> starting at START_INDEX (0).

> [spec:hfst:def:hfst-optimized-lookup.transducer.final-index-fn]
> bool final_index(TransitionTableIndex i)

> [spec:hfst:sem:hfst-optimized-lookup.transducer.final-index-fn]
> Return whether index i is final.

> [spec:hfst:def:hfst-optimized-lookup.transducer.final-transition-fn]
> bool final_transition(TransitionTableIndex i)

> [spec:hfst:sem:hfst-optimized-lookup.transducer.final-transition-fn]
> Return whether transition i is final.

> [spec:hfst:def:hfst-optimized-lookup.transducer.find-index-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer.find-index-fn]
> If the index entry at i+input has input symbol equal to input, follow it into
> the transition table via find_transitions at that entry's target minus
> TRANSITION_TARGET_TABLE_START.

> [spec:hfst:def:hfst-optimized-lookup.transducer.find-next-key-fn]
> SymbolNumber find_next_key(char ** p)

> [spec:hfst:sem:hfst-optimized-lookup.transducer.find-next-key-fn]
> Tokenise the next input symbol at cursor **p by delegating to the encoder's
> find_key.

> [spec:hfst:def:hfst-optimized-lookup.transducer.find-transitions-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer.find-transitions-fn]
> Scan transitions from i while the input symbol is not NO_SYMBOL_NUMBER. For each
> transition whose input equals the queried input, emit its output symbol and
> recurse via get_analyses at its target with output advanced by one. Stop as soon
> as a transition's input differs from the queried symbol (the table is sorted).

> [spec:hfst:def:hfst-optimized-lookup.transducer.get-analyses-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer.get-analyses-fn]
> Core recursive lookup. If time_cutoff is set, count this call and bail out
> (setting limit_reached) once the per-input time budget is exceeded. If i is at
> or above TRANSITION_TARGET_TABLE_START, subtract that offset and operate on the
> transition table: try epsilon transitions at i+1; if the input string is
> exhausted (*input_symbol == NO_SYMBOL_NUMBER) terminate the output and, if
> transition i is final, note the analysis; otherwise consume one input symbol and
> call find_transitions at i+1. If i is below the offset, operate on the index
> table analogously: try epsilon indices at i+1; if input is exhausted, note the
> analysis when index i is final; otherwise consume one input symbol and call
> find_index at i+1. Finally write NO_SYMBOL_NUMBER at the current output position.

> [spec:hfst:def:hfst-optimized-lookup.transducer.get-key-table-fn]
> KeyTable * get_key_table(void)

> [spec:hfst:sem:hfst-optimized-lookup.transducer.get-key-table-fn]
> Return the key table pointer.

> [spec:hfst:def:hfst-optimized-lookup.transducer.note-analysis-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer.note-analysis-fn]
> Record one completed analysis from the output buffer (read symbols until
> NO_SYMBOL_NUMBER). In beFast mode, print each symbol's string directly to stdout
> followed by a newline. Otherwise concatenate the symbol strings into one string
> and push it onto display_vector.

> [spec:hfst:def:hfst-optimized-lookup.transducer.print-analyses-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer.print-analyses-fn]
> Emit the collected analyses for one input word 'prepend'. In beFast mode do
> nothing (output already streamed). In xerox mode with no analyses, print
> prepend\tprepend\t+? then two newlines. Otherwise, for up to maxAnalyses
> entries of display_vector, print prepend and a tab (xerox), then the analysis
> and a newline; clear display_vector and print a trailing newline.

> [spec:hfst:def:hfst-optimized-lookup.transducer.set-symbol-table-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer.set-symbol-table-fn]
> Populate symbol_table by iterating the key table in key order and appending
> each key's symbol string, so symbol_table[n] is the printable string for symbol
> number n.

> [spec:hfst:def:hfst-optimized-lookup.transducer.transducer-fn]
> Transducer(FILE * f, TransducerHeader h, TransducerAlphabet a)

> [spec:hfst:sem:hfst-optimized-lookup.transducer.transducer-fn]
> Construct a Transducer from file f, header h and alphabet a. Take the key table
> from the alphabet, build the index table reader (reading header.index_table_size
> entries from f) then the transition table reader (header.target_table_size
> entries from f) in that order, and build an Encoder over the key table for
> header.input_symbol_count input symbols. Allocate the output_string buffer of
> 1000 SymbolNumber slots, fill it with NO_SYMBOL_NUMBER, bind indices/transitions
> to the readers' vectors, and call set_symbol_table.

> [spec:hfst:def:hfst-optimized-lookup.transducer.try-epsilon-indices-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer.try-epsilon-indices-fn]
> If the index entry at i has input symbol 0 (epsilon), follow it into the
> transition table by calling try_epsilon_transitions at its target minus
> TRANSITION_TARGET_TABLE_START.

> [spec:hfst:def:hfst-optimized-lookup.transducer.try-epsilon-transitions-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer.try-epsilon-transitions-fn]
> Starting at transition i, while the transition's input symbol is 0 (epsilon),
> write its output symbol to *output_symbol and recurse into get_analyses at the
> transition's target with output_symbol advanced by one; then advance i. Stops at
> the first non-epsilon transition.

> [spec:hfst:def:hfst-optimized-lookup.transition]
> class Transition {
>   SymbolNumber input_symbol;
>   SymbolNumber output_symbol;
>   TransitionTableIndex target_index;
>   static const size_t SIZE = 2 * sizeof(SymbolNumber) + sizeof(TransitionTableIndex);
> }

> [spec:hfst:def:hfst-optimized-lookup.transition-index]
> class TransitionIndex {
>   SymbolNumber input_symbol;
>   TransitionTableIndex first_transition_index;
>   static const size_t SIZE = sizeof(SymbolNumber) + sizeof(TransitionTableIndex);
> }

> [spec:hfst:def:hfst-optimized-lookup.transition-index-vector]
> typedef std::vector<TransitionIndex*> TransitionIndexVector

> [spec:hfst:def:hfst-optimized-lookup.transition-index.final-fn]
> bool final(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-index.final-fn]
> Return true iff first_transition_index equals 1, the sentinel marking a final
> index entry.

> [spec:hfst:def:hfst-optimized-lookup.transition-index.get-input-fn]
> SymbolNumber get_input(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-index.get-input-fn]
> Return the input symbol of this index entry.

> [spec:hfst:def:hfst-optimized-lookup.transition-index.matches-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.transition-index.matches-fn]
> Return false if input_symbol is NO_SYMBOL_NUMBER (no match possible). Return
> true if the queried s is NO_SYMBOL_NUMBER (wildcard). Otherwise return whether
> input_symbol equals s.

> [spec:hfst:def:hfst-optimized-lookup.transition-index.target-fn]
> TransitionTableIndex target(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-index.target-fn]
> Return first_transition_index, the target index this entry points to.

> [spec:hfst:def:hfst-optimized-lookup.transition-index.transition-index-fn]
> TransitionIndex(SymbolNumber input,

> [spec:hfst:sem:hfst-optimized-lookup.transition-index.transition-index-fn]
> Construct a TransitionIndex storing the input symbol and the first-transition
> target index.

> [spec:hfst:def:hfst-optimized-lookup.transition-number]
> typedef unsigned int TransitionNumber

> [spec:hfst:def:hfst-optimized-lookup.transition-table-index]
> typedef unsigned int TransitionTableIndex

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader]
> class TransitionTableReader {
>   TransitionTableIndex number_of_table_entries;
>   char * TableTransitions;
>   TransitionVector transitions;
>   size_t table_size;
>   size_t transition_size;
>   TransitionTableIndex position;
> }

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w]
> class TransitionTableReaderW {
>   TransitionTableIndex number_of_table_entries;
>   char * TableTransitions;
>   TransitionWVector transitions;
>   size_t table_size;
>   TransitionTableIndex position;
> }

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.at-fn]
> TransitionW * at(TransitionTableIndex i)

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.at-fn]
> Return the TransitionW at i - TRANSITION_TARGET_TABLE_START.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.get-finality-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.get-finality-fn]
> Return whether weighted transition i is final, indexing at
> i - TRANSITION_TARGET_TABLE_START when i is at or above that offset.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.get-input-fn]
> SymbolNumber get_input(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.get-input-fn]
> Return the input symbol of the transition at the current position.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.get-output-fn]
> SymbolNumber get_output(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.get-output-fn]
> Return the output symbol of the transition at the current position.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.get-target-fn]
> TransitionTableIndex get_target(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.get-target-fn]
> Return the target index of the transition at the current position.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.get-transition-vector-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.get-transition-vector-fn]
> Decode each weighted transition (input, output, target, weight) at the
> appropriate byte offsets (input at i*SIZE, output at +sizeof(SymbolNumber),
> target at +2*sizeof(SymbolNumber), weight at +2*sizeof(SymbolNumber)+
> sizeof(TransitionTableIndex)), then append two default (sentinel) TransitionW
> entries as end padding.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.matches-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.matches-fn]
> Return whether the weighted transition at the current position matches s.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.next-fn]
> void Next(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.next-fn]
> Advance the cursor position by one.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.set-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.set-fn]
> Set the cursor: subtract TRANSITION_TARGET_TABLE_START from pos when pos is at
> or above it, otherwise use pos directly.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.transition-table-reader-w-fn]
> TransitionTableReaderW(FILE * f,

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.transition-table-reader-w-fn]
> Read transition_count weighted transitions from file f into a byte buffer of
> transition_count*TransitionW::SIZE bytes, then decode via get_transition_vector;
> position starts at 0.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.at-fn]
> Transition * at(TransitionTableIndex i)

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.at-fn]
> Return the Transition at i, indexing the vector at i - TRANSITION_TARGET_TABLE_START.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.get-finality-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.get-finality-fn]
> Return whether transition i is final, indexing at i - TRANSITION_TARGET_TABLE_START
> when i is at or above that offset, otherwise at i directly.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.get-input-fn]
> SymbolNumber get_input(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.get-input-fn]
> Return the input symbol of the transition at the current position.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.get-output-fn]
> SymbolNumber get_output(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.get-output-fn]
> Return the output symbol of the transition at the current position.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.get-target-fn]
> TransitionTableIndex get_target(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.get-target-fn]
> Return the target index of the transition at the current position.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.get-transition-vector-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.get-transition-vector-fn]
> Decode the raw transition table into a vector of Transition objects. For each
> entry i: input symbol at offset i*SIZE, output symbol at + sizeof(SymbolNumber),
> target index at + 2*sizeof(SymbolNumber).

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.matches-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.matches-fn]
> Return whether the transition at the current position matches s.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.next-fn]
> void Next(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.next-fn]
> Advance the cursor position by one.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.set-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.set-fn]
> Set the cursor position from pos: if pos is at or above
> TRANSITION_TARGET_TABLE_START, subtract that offset first; otherwise use pos as
> is.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.transition-table-reader-fn]
> TransitionTableReader(FILE * f,

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.transition-table-reader-fn]
> Read transition_count transition entries from file f. Allocate a byte buffer of
> transition_count*Transition::SIZE bytes, fread the table into it, then call
> get_transition_vector to decode it; position starts at 0.

> [spec:hfst:def:hfst-optimized-lookup.transition-vector]
> typedef std::vector<Transition*> TransitionVector

> [spec:hfst:def:hfst-optimized-lookup.transition-w]
> class TransitionW {
>   SymbolNumber input_symbol;
>   SymbolNumber output_symbol;
>   TransitionTableIndex target_index;
>   Weight transition_weight;
>   static const size_t SIZE = 2 * sizeof(SymbolNumber) + sizeof(TransitionTableIndex) + sizeof(Weight);
> }

> [spec:hfst:def:hfst-optimized-lookup.transition-w-index]
> class TransitionWIndex {
>   SymbolNumber input_symbol;
>   TransitionTableIndex first_transition_index;
>   static const size_t SIZE = sizeof(SymbolNumber) + sizeof(TransitionTableIndex);
> }

> [spec:hfst:def:hfst-optimized-lookup.transition-w-index-vector]
> typedef std::vector<TransitionWIndex*> TransitionWIndexVector

> [spec:hfst:def:hfst-optimized-lookup.transition-w-index.final-fn]
> bool final(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-w-index.final-fn]
> Return true iff input_symbol is NO_SYMBOL_NUMBER and first_transition_index is
> not NO_TABLE_INDEX; such an entry encodes a final weight in its index field.

> [spec:hfst:def:hfst-optimized-lookup.transition-w-index.final-weight-fn]
> Weight final_weight(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-w-index.final-weight-fn]
> Reinterpret the bits of first_transition_index (a 32-bit unsigned int) as a
> 32-bit float and return it as the final weight (a type-pun via union in C).

> [spec:hfst:def:hfst-optimized-lookup.transition-w-index.get-input-fn]
> SymbolNumber get_input(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-w-index.get-input-fn]
> Return the input symbol of this weighted index entry.

> [spec:hfst:def:hfst-optimized-lookup.transition-w-index.matches-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.transition-w-index.matches-fn]
> Return false if input_symbol is NO_SYMBOL_NUMBER; true if s is NO_SYMBOL_NUMBER;
> otherwise input_symbol == s.

> [spec:hfst:def:hfst-optimized-lookup.transition-w-index.target-fn]
> TransitionTableIndex target(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-w-index.target-fn]
> Return first_transition_index, the target index.

> [spec:hfst:def:hfst-optimized-lookup.transition-w-index.transition-w-index-fn]
> TransitionWIndex(SymbolNumber input,

> [spec:hfst:sem:hfst-optimized-lookup.transition-w-index.transition-w-index-fn]
> Construct a TransitionWIndex storing input symbol and first-transition index.

> [spec:hfst:def:hfst-optimized-lookup.transition-w-vector]
> typedef std::vector<TransitionW*> TransitionWVector

> [spec:hfst:def:hfst-optimized-lookup.transition-w.final-fn]
> bool final(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-w.final-fn]
> Return true iff input_symbol and output_symbol are both NO_SYMBOL_NUMBER and
> target_index equals 1, marking a final weighted transition.

> [spec:hfst:def:hfst-optimized-lookup.transition-w.get-input-fn]
> SymbolNumber get_input(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-w.get-input-fn]
> Return the input symbol.

> [spec:hfst:def:hfst-optimized-lookup.transition-w.get-output-fn]
> SymbolNumber get_output(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-w.get-output-fn]
> Return the output symbol.

> [spec:hfst:def:hfst-optimized-lookup.transition-w.get-weight-fn]
> Weight get_weight(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-w.get-weight-fn]
> Return the transition weight.

> [spec:hfst:def:hfst-optimized-lookup.transition-w.matches-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.transition-w.matches-fn]
> Return false if input_symbol is NO_SYMBOL_NUMBER; true if s is NO_SYMBOL_NUMBER;
> otherwise input_symbol == s.

> [spec:hfst:def:hfst-optimized-lookup.transition-w.target-fn]
> TransitionTableIndex target(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-w.target-fn]
> Return target_index.

> [spec:hfst:def:hfst-optimized-lookup.transition-w.transition-w-fn]
> TransitionW(SymbolNumber input,

> [spec:hfst:sem:hfst-optimized-lookup.transition-w.transition-w-fn]
> Construct a TransitionW storing input symbol, output symbol, target index and
> weight. The default form stores NO_SYMBOL_NUMBER input/output, NO_TABLE_INDEX
> target and INFINITE_WEIGHT, used as padding sentinels.

> [spec:hfst:def:hfst-optimized-lookup.transition.final-fn]
> bool final(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition.final-fn]
> Return true iff target_index equals 1, marking a final transition.

> [spec:hfst:def:hfst-optimized-lookup.transition.get-input-fn]
> SymbolNumber get_input(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition.get-input-fn]
> Return the input symbol of this transition.

> [spec:hfst:def:hfst-optimized-lookup.transition.get-output-fn]
> SymbolNumber get_output(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition.get-output-fn]
> Return the output symbol of this transition.

> [spec:hfst:def:hfst-optimized-lookup.transition.matches-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.transition.matches-fn]
> Return false if input_symbol is NO_SYMBOL_NUMBER; return true if s is
> NO_SYMBOL_NUMBER (wildcard); otherwise return input_symbol == s.

> [spec:hfst:def:hfst-optimized-lookup.transition.target-fn]
> TransitionTableIndex target(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition.target-fn]
> Return target_index, the index of the target state's first table entry.

> [spec:hfst:def:hfst-optimized-lookup.transition.transition-fn]
> Transition(SymbolNumber input,

> [spec:hfst:sem:hfst-optimized-lookup.transition.transition-fn]
> Construct a Transition storing input symbol, output symbol and target index.

> [spec:hfst:def:hfst-optimized-lookup.value-number]
> typedef short ValueNumber

> [spec:hfst:def:hfst-optimized-lookup.weight]
> typedef float Weight

