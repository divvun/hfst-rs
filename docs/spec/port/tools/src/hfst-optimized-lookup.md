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
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.encoder.find-key-fn]
> SymbolNumber

> [spec:hfst:sem:hfst-optimized-lookup.encoder.find-key-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.encoder.read-input-symbols-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.encoder.read-input-symbols-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-operation]
> class FlagDiacriticOperation {
>   FlagDiacriticOperator operation;
>   SymbolNumber feature;
>   ValueNumber value;
> }

> [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-operation.feature-fn]
> SymbolNumber Feature(void)

> [spec:hfst:sem:hfst-optimized-lookup.flag-diacritic-operation.feature-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-operation.flag-diacritic-operation-fn]
> FlagDiacriticOperation(FlagDiacriticOperator op, SymbolNumber feat, ValueNumber val)

> [spec:hfst:sem:hfst-optimized-lookup.flag-diacritic-operation.flag-diacritic-operation-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-operation.is-flag-fn]
> bool isFlag(void)

> [spec:hfst:sem:hfst-optimized-lookup.flag-diacritic-operation.is-flag-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-operation.operation-fn]
> FlagDiacriticOperator Operation(void)

> [spec:hfst:sem:hfst-optimized-lookup.flag-diacritic-operation.operation-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-operation.print-fn]
> void print(void)

> [spec:hfst:sem:hfst-optimized-lookup.flag-diacritic-operation.print-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.flag-diacritic-operation.value-fn]
> ValueNumber Value(void)

> [spec:hfst:sem:hfst-optimized-lookup.flag-diacritic-operation.value-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

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
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

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
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.index-table-reader-w.get-finality-fn]
> bool get_finality(TransitionTableIndex i)

> [spec:hfst:sem:hfst-optimized-lookup.index-table-reader-w.get-finality-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.index-table-reader-w.get-index-vector-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.index-table-reader-w.get-index-vector-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.index-table-reader-w.index-table-reader-w-fn]
> IndexTableReaderW(FILE * f,

> [spec:hfst:sem:hfst-optimized-lookup.index-table-reader-w.index-table-reader-w-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.index-table-reader.at-fn]
> TransitionIndex * at(TransitionTableIndex i)

> [spec:hfst:sem:hfst-optimized-lookup.index-table-reader.at-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.index-table-reader.get-finality-fn]
> bool get_finality(TransitionTableIndex i)

> [spec:hfst:sem:hfst-optimized-lookup.index-table-reader.get-finality-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.index-table-reader.get-index-vector-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.index-table-reader.get-index-vector-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.index-table-reader.index-table-reader-fn]
> IndexTableReader(FILE * f,

> [spec:hfst:sem:hfst-optimized-lookup.index-table-reader.index-table-reader-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

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
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.letter-trie.find-key-fn]
> SymbolNumber

> [spec:hfst:sem:hfst-optimized-lookup.letter-trie.find-key-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.letter-trie.has-key-starting-with-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.letter-trie.has-key-starting-with-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.letter-trie.letter-trie-fn]
> LetterTrie(void)

> [spec:hfst:sem:hfst-optimized-lookup.letter-trie.letter-trie-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.main-fn]
> int

> [spec:hfst:sem:hfst-optimized-lookup.main-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

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
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.print-usage-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.print-usage-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.print-version-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.print-version-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.run-transducer-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.run-transducer-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.setup-fn]
> int

> [spec:hfst:sem:hfst-optimized-lookup.setup-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

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
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-alphabet.get-next-symbol-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-alphabet.get-next-symbol-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-alphabet.get-operation-vector-fn]
> OperationVector get_operation_vector(void)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-alphabet.get-operation-vector-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-alphabet.get-state-size-fn]
> SymbolNumber get_state_size(void)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-alphabet.get-state-size-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-alphabet.transducer-alphabet-fn]
> TransducerAlphabet(FILE * f,SymbolNumber symbol_number)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-alphabet.transducer-alphabet-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

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
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-fd-uniq.print-analyses-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-fd-uniq.print-analyses-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-fd-uniq.transducer-fd-uniq-fn]
> TransducerFdUniq(FILE * f, TransducerHeader h, TransducerAlphabet a)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-fd-uniq.transducer-fd-uniq-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-fd.push-state-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.transducer-fd.push-state-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-fd.transducer-fd-fn]
> TransducerFd(FILE * f, TransducerHeader h, TransducerAlphabet a)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-fd.transducer-fd-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-fd.try-epsilon-transitions-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-fd.try-epsilon-transitions-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

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
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-header.input-symbol-count-fn]
> SymbolNumber input_symbol_count(void)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-header.input-symbol-count-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-header.probe-flag-fn]
> bool probe_flag(HeaderFlag flag)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-header.probe-flag-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-header.read-property-fn]
> void read_property(bool &property, FILE * f)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-header.read-property-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-header.skip-hfst3-header-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-header.skip-hfst3-header-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-header.symbol-count-fn]
> SymbolNumber symbol_count(void)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-header.symbol-count-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-header.target-table-size-fn]
> TransitionTableIndex target_table_size(void)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-header.target-table-size-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-header.transducer-header-fn]
> TransducerHeader(FILE * f)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-header.transducer-header-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-uniq]
> class TransducerUniq: public Transducer {
>   DisplaySet display_vector;
> }

> [spec:hfst:def:hfst-optimized-lookup.transducer-uniq.note-analysis-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-uniq.note-analysis-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-uniq.print-analyses-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-uniq.print-analyses-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-uniq.transducer-uniq-fn]
> TransducerUniq(FILE * f, TransducerHeader h, TransducerAlphabet a)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-uniq.transducer-uniq-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

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
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w-fd-uniq.print-analyses-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w-fd-uniq.print-analyses-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w-fd-uniq.transducer-w-fd-uniq-fn]
> TransducerWFdUniq(FILE * f, TransducerHeader h, TransducerAlphabet a)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w-fd-uniq.transducer-w-fd-uniq-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w-fd.push-state-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w-fd.push-state-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w-fd.transducer-w-fd-fn]
> TransducerWFd(FILE * f, TransducerHeader h, TransducerAlphabet a)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w-fd.transducer-w-fd-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w-fd.try-epsilon-transitions-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w-fd.try-epsilon-transitions-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w-uniq]
> class TransducerWUniq: public TransducerW {
>   DisplayMap display_map;
> }

> [spec:hfst:def:hfst-optimized-lookup.transducer-w-uniq.note-analysis-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w-uniq.note-analysis-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w-uniq.print-analyses-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w-uniq.print-analyses-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w-uniq.transducer-w-uniq-fn]
> TransducerWUniq(FILE * f, TransducerHeader h, TransducerAlphabet a)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w-uniq.transducer-w-uniq-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.analyze-fn]
> void analyze(SymbolNumber * input_string)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.analyze-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.final-index-fn]
> bool final_index(TransitionTableIndex i)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.final-index-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.final-transition-fn]
> bool final_transition(TransitionTableIndex i)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.final-transition-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.find-index-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.find-index-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.find-next-key-fn]
> SymbolNumber find_next_key(char ** p)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.find-next-key-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.find-transitions-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.find-transitions-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.get-analyses-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.get-analyses-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.get-final-index-weight-fn]
> Weight get_final_index_weight(TransitionTableIndex i)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.get-final-index-weight-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.get-final-transition-weight-fn]
> Weight get_final_transition_weight(TransitionTableIndex i)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.get-final-transition-weight-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.get-key-table-fn]
> KeyTable * get_key_table(void)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.get-key-table-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.note-analysis-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.note-analysis-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.print-analyses-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.print-analyses-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.set-symbol-table-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.set-symbol-table-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.transducer-w-fn]
> TransducerW(FILE * f, TransducerHeader h, TransducerAlphabet a)

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.transducer-w-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.try-epsilon-indices-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.try-epsilon-indices-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer-w.try-epsilon-transitions-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer-w.try-epsilon-transitions-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer.analyze-fn]
> void analyze(SymbolNumber * input_string)

> [spec:hfst:sem:hfst-optimized-lookup.transducer.analyze-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer.final-index-fn]
> bool final_index(TransitionTableIndex i)

> [spec:hfst:sem:hfst-optimized-lookup.transducer.final-index-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer.final-transition-fn]
> bool final_transition(TransitionTableIndex i)

> [spec:hfst:sem:hfst-optimized-lookup.transducer.final-transition-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer.find-index-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer.find-index-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer.find-next-key-fn]
> SymbolNumber find_next_key(char ** p)

> [spec:hfst:sem:hfst-optimized-lookup.transducer.find-next-key-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer.find-transitions-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer.find-transitions-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer.get-analyses-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer.get-analyses-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer.get-key-table-fn]
> KeyTable * get_key_table(void)

> [spec:hfst:sem:hfst-optimized-lookup.transducer.get-key-table-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer.note-analysis-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer.note-analysis-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer.print-analyses-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer.print-analyses-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer.set-symbol-table-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer.set-symbol-table-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer.transducer-fn]
> Transducer(FILE * f, TransducerHeader h, TransducerAlphabet a)

> [spec:hfst:sem:hfst-optimized-lookup.transducer.transducer-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer.try-epsilon-indices-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer.try-epsilon-indices-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transducer.try-epsilon-transitions-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transducer.try-epsilon-transitions-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

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
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-index.get-input-fn]
> SymbolNumber get_input(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-index.get-input-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-index.matches-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.transition-index.matches-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-index.target-fn]
> TransitionTableIndex target(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-index.target-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-index.transition-index-fn]
> TransitionIndex(SymbolNumber input,

> [spec:hfst:sem:hfst-optimized-lookup.transition-index.transition-index-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

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
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.get-finality-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.get-finality-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.get-input-fn]
> SymbolNumber get_input(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.get-input-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.get-output-fn]
> SymbolNumber get_output(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.get-output-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.get-target-fn]
> TransitionTableIndex get_target(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.get-target-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.get-transition-vector-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.get-transition-vector-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.matches-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.matches-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.next-fn]
> void Next(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.next-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.set-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.set-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader-w.transition-table-reader-w-fn]
> TransitionTableReaderW(FILE * f,

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader-w.transition-table-reader-w-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.at-fn]
> Transition * at(TransitionTableIndex i)

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.at-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.get-finality-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.get-finality-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.get-input-fn]
> SymbolNumber get_input(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.get-input-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.get-output-fn]
> SymbolNumber get_output(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.get-output-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.get-target-fn]
> TransitionTableIndex get_target(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.get-target-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.get-transition-vector-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.get-transition-vector-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.matches-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.matches-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.next-fn]
> void Next(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.next-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.set-fn]
> void

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.set-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-table-reader.transition-table-reader-fn]
> TransitionTableReader(FILE * f,

> [spec:hfst:sem:hfst-optimized-lookup.transition-table-reader.transition-table-reader-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

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
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-w-index.final-weight-fn]
> Weight final_weight(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-w-index.final-weight-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-w-index.get-input-fn]
> SymbolNumber get_input(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-w-index.get-input-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-w-index.matches-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.transition-w-index.matches-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-w-index.target-fn]
> TransitionTableIndex target(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-w-index.target-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-w-index.transition-w-index-fn]
> TransitionWIndex(SymbolNumber input,

> [spec:hfst:sem:hfst-optimized-lookup.transition-w-index.transition-w-index-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-w-vector]
> typedef std::vector<TransitionW*> TransitionWVector

> [spec:hfst:def:hfst-optimized-lookup.transition-w.final-fn]
> bool final(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-w.final-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-w.get-input-fn]
> SymbolNumber get_input(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-w.get-input-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-w.get-output-fn]
> SymbolNumber get_output(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-w.get-output-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-w.get-weight-fn]
> Weight get_weight(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-w.get-weight-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-w.matches-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.transition-w.matches-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-w.target-fn]
> TransitionTableIndex target(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition-w.target-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition-w.transition-w-fn]
> TransitionW(SymbolNumber input,

> [spec:hfst:sem:hfst-optimized-lookup.transition-w.transition-w-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition.final-fn]
> bool final(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition.final-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition.get-input-fn]
> SymbolNumber get_input(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition.get-input-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition.get-output-fn]
> SymbolNumber get_output(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition.get-output-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition.matches-fn]
> bool

> [spec:hfst:sem:hfst-optimized-lookup.transition.matches-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition.target-fn]
> TransitionTableIndex target(void)

> [spec:hfst:sem:hfst-optimized-lookup.transition.target-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.transition.transition-fn]
> Transition(SymbolNumber input,

> [spec:hfst:sem:hfst-optimized-lookup.transition.transition-fn]
> TODO(sem): what this does, step by step — precisely enough to
> re-implement from this rule alone, without reading the source.

> [spec:hfst:def:hfst-optimized-lookup.value-number]
> typedef short ValueNumber

> [spec:hfst:def:hfst-optimized-lookup.weight]
> typedef float Weight

