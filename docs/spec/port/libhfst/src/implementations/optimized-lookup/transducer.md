# libhfst/src/implementations/optimized-lookup/transducer.cc, libhfst/src/implementations/optimized-lookup/transducer.h

> [spec:hfst:def:transducer.hfst-ol.alphabet-translation-exception]
> class AlphabetTranslationException: public std::runtime_error

> [spec:hfst:def:transducer.hfst-ol.alphabet-translation-exception.alphabet-translation-exception-fn]
> AlphabetTranslationException(const std::string what)

> [spec:hfst:sem:transducer.hfst-ol.alphabet-translation-exception.alphabet-translation-exception-fn]
> Constructor for AlphabetTranslationException (derives from
> std::runtime_error). Takes a std::string `what` by value and forwards it to
> the std::runtime_error base-class constructor, so `what()` returns this
> message. By convention the message holds the first untranslatable symbol's
> string. The body is empty; no other state is set.

> [spec:hfst:def:transducer.hfst-ol.analysis-queue]
> typedef std::priority_queue<StringWeightPair,

> [spec:hfst:def:transducer.hfst-ol.correction-queue]
> typedef std::priority_queue<StringWeightPair,

> [spec:hfst:def:transducer.hfst-ol.double-tape]
> struct DoubleTape: public std::vector<SymbolPair>

> [spec:hfst:def:transducer.hfst-ol.double-tape.extract-slice-fn]
> DoubleTape extract_slice(unsigned int start, unsigned int stop)

> [spec:hfst:sem:transducer.hfst-ol.double-tape.extract-slice-fn]
> Returns a new DoubleTape containing a copy of the SymbolPair elements of this
> tape in the half-open index range [start, stop). Constructs an empty
> DoubleTape `retval`, then while start < stop, push_back a copy of this->at(start)
> (bounds-checked access) and increment start. Returns retval by value. If start
> >= stop the result is empty. Out-of-range indices propagate the
> std::out_of_range from at().

> [spec:hfst:def:transducer.hfst-ol.double-tape.write-fn]
> void write(unsigned int pos, std::pair<SymbolNumberVector::iterator,

> [spec:hfst:sem:transducer.hfst-ol.double-tape.write-fn]
> Writes a run of symbols into the DoubleTape (a vector<SymbolPair>) starting at
> index `pos`, taking the source symbols from the iterator range
> start_and_end.first .. start_and_end.second. Computes size = second - first.
> Grows the tape by push_back-ing default SymbolPair() entries while
> (pos + size >= size()), so that all target positions exist. Then for i in
> [0, size), sets this[pos + i] = SymbolPair(in, in) where in = *(first + i),
> i.e. both the input and output fields are set to the same source symbol.
> This overload treats the written run as identity pairs.

> [spec:hfst:def:transducer.hfst-ol.encoder]
> class Encoder {
>   SymbolNumber number_of_input_symbols;
>   OlLetterTrie letters;
>   SymbolNumberVector ascii_symbols;
> }

> [spec:hfst:def:transducer.hfst-ol.encoder.encoder-fn]
> Encoder(const SymbolTable & st, SymbolNumber input_symbol_count)

> [spec:hfst:sem:transducer.hfst-ol.encoder.encoder-fn]
> Constructs an Encoder from a SymbolTable `st` and the input-symbol count
> `input_symbol_count`. Initializes number_of_input_symbols to
> input_symbol_count, initializes the OlLetterTrie `letters` to empty
> (default-constructed), and initializes ascii_symbols to a vector of 128
> entries all set to NO_SYMBOL_NUMBER. Then calls read_input_symbols(st), which
> registers each of the first number_of_input_symbols symbol strings from st
> into the trie and ascii lookup table.

> [spec:hfst:def:transducer.hfst-ol.encoder.find-key-fn]
> SymbolNumber Encoder::find_key(char ** p)

> [spec:hfst:sem:transducer.hfst-ol.encoder.find-key-fn]
> Tokenizes one symbol from the input, advancing the caller's char pointer.
> `p` is a pointer to a char* cursor into the input string. If the current byte
> **p is not ascii-tokenizable (should_ascii_tokenize is false) OR
> ascii_symbols[**p] == NO_SYMBOL_NUMBER (no fast ascii mapping), it delegates
> to letters.find_key(p) (the trie walk, which itself advances the cursor) and
> returns its result. Otherwise it has a single-byte ascii match: reads
> s = ascii_symbols[**p], advances the cursor by one byte (++(*p)), and returns
> s. May return NO_SYMBOL_NUMBER when the trie path yields no symbol.

> [spec:hfst:def:transducer.hfst-ol.encoder.read-input-symbol-fn]
> void Encoder::read_input_symbol(const char * s, const int s_num)

> [spec:hfst:sem:transducer.hfst-ol.encoder.read-input-symbol-fn]
> Registers one input symbol string `s` with symbol number `s_num` into the
> encoder's lookup structures. First, if s is exactly one byte long
> (strlen(s)==1), that byte is ascii-tokenizable, and the trie has no key
> starting with that byte (letters.has_key_starting_with(*s) is false), record
> the fast path: ascii_symbols[(unsigned char)*s] = s_num. Second, if s is
> longer than one byte, its first byte is ascii-tokenizable, and
> ascii_symbols[first byte] is currently set (!= NO_SYMBOL_NUMBER), clear that
> shadowing ascii entry by setting it to NO_SYMBOL_NUMBER. Finally, always add
> the full string to the trie via letters.add_string(s, s_num).

> [spec:hfst:def:transducer.hfst-ol.encoder.read-input-symbols-fn]
> void Encoder::read_input_symbols(const SymbolTable & kt)

> [spec:hfst:sem:transducer.hfst-ol.encoder.read-input-symbols-fn]
> Iterates k from 0 up to (exclusive) number_of_input_symbols, and for each k
> calls read_input_symbol(kt[k].c_str(), k), registering each of the first
> number_of_input_symbols entries of the SymbolTable `kt` into the encoder's
> trie and ascii lookup. No return value.

> [spec:hfst:def:transducer.hfst-ol.flag-diacritic-state]
> typedef std::vector<short> FlagDiacriticState

> [spec:hfst:def:transducer.hfst-ol.header-flag]
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

> [spec:hfst:def:transducer.hfst-ol.hyphenation-queue]
> typedef std::priority_queue<StringWeightPair,

> [spec:hfst:def:transducer.hfst-ol.indexes-transition-index-table-fn]
> inline bool indexes_transition_index_table(const TransitionTableIndex i)

> [spec:hfst:sem:transducer.hfst-ol.indexes-transition-index-table-fn]
> Returns true iff the given TransitionTableIndex `i` addresses the transition
> index table, i.e. i < TRANSITION_TARGET_TABLE_START. Inline, pure.

> [spec:hfst:def:transducer.hfst-ol.indexes-transition-table-fn]
> inline bool indexes_transition_table(const TransitionTableIndex i)

> [spec:hfst:sem:transducer.hfst-ol.indexes-transition-table-fn]
> Returns true iff the given TransitionTableIndex `i` addresses the transition
> (target) table, i.e. i >= TRANSITION_TARGET_TABLE_START. Inline, pure.

> [spec:hfst:def:transducer.hfst-ol.input-string]
> class InputString {
>   SymbolNumberVector s;
> }

> [spec:hfst:def:transducer.hfst-ol.input-string.initialize-fn]
> bool initialize(const Encoder & encoder, char * input, SymbolNumber other)

> [spec:hfst:sem:transducer.hfst-ol.input-string.initialize-fn]
> Tokenizes the C-string `input` into this InputString's symbol vector `s` using
> the encoder, returning true on success and false on failure. First clears s.
> Sets up a cursor (inpointer) over input. Loop while the current byte is not
> '\0': record oldpointer = current cursor; call encoder.find_key(inpointer),
> which returns symbol k and advances the cursor past the consumed bytes.
> If k == NO_SYMBOL_NUMBER (no alphabet tokenization): compute n =
> nByte_utf8(first byte at oldpointer). If n == 0 (not a valid utf-8 lead byte),
> return false. Else, if `other` == NO_SYMBOL_NUMBER (no OTHER symbol available),
> return false; otherwise advance the cursor by n bytes (oldpointer += n,
> *inpointer = oldpointer), push_back `other` onto s, and continue the loop.
> If k is a real symbol, push_back k onto s. After the loop (string exhausted),
> return true. The empty string yields an empty vector; there is no end marker.

> [spec:hfst:def:transducer.hfst-ol.input-string.input-string-fn]
> InputString()

> [spec:hfst:sem:transducer.hfst-ol.input-string.input-string-fn]
> Default constructor for InputString. Initializes the private member `s` to an
> empty SymbolNumberVector. Empty body.

> [spec:hfst:def:transducer.hfst-ol.input-string.len-fn]
> unsigned int len(void)

> [spec:hfst:sem:transducer.hfst-ol.input-string.len-fn]
> Returns the number of tokenized symbols, i.e. s.size() cast to unsigned int.

> [spec:hfst:def:transducer.hfst-ol.input-string.operator-fn]
> SymbolNumber operator[](unsigned int i)

> [spec:hfst:sem:transducer.hfst-ol.input-string.operator-fn]
> Indexing operator: returns the symbol s[i], the i-th tokenized symbol. No
> bounds checking (uses vector::operator[]).

> [spec:hfst:def:transducer.hfst-ol.n-byte-utf8-fn]
> int nByte_utf8(unsigned char c)

> [spec:hfst:sem:transducer.hfst-ol.n-byte-utf8-fn]
> Determines how many bytes the utf-8 character that begins with lead byte `c`
> occupies, used to peel off a character for representation as OTHER. Returns:
> 1 if c <= 127 (ascii); 4 if the top four bits are 1111 (c & 0xF0 == 0xF0);
> 3 if the top three bits are 1110 (c & 0xE0 == 0xE0); 2 if the top two bits are
> 11 (c & 0xC0 == 0xC0); otherwise 0 (an invalid/continuation byte that cannot
> start a character). The checks are evaluated in that order (4, then 3, then 2).

> [spec:hfst:def:transducer.hfst-ol.ol-letter-trie]
> class OlLetterTrie {
>   OlLetterTrieVector letters;
>   SymbolNumberVector symbols;
> }

> [spec:hfst:def:transducer.hfst-ol.ol-letter-trie-vector]
> typedef std::vector<OlLetterTrie*> OlLetterTrieVector

> [spec:hfst:def:transducer.hfst-ol.ol-letter-trie.add-string-fn]
> void OlLetterTrie::add_string(const char * p, SymbolNumber symbol_key)

> [spec:hfst:sem:transducer.hfst-ol.ol-letter-trie.add-string-fn]
> Inserts the NUL-terminated string `p` into the trie, mapping it to
> symbol_key. `letters` and `symbols` are 256-entry vectors indexed by the
> unsigned-char byte value. If p has exactly one remaining byte (the byte after
> *p is 0), record symbols[(unsigned char)*p] = symbol_key and return. Otherwise
> descend: if letters[(unsigned char)*p] is NULL, allocate a new OlLetterTrie
> there (heap, never freed except by the destructor). Then recurse:
> letters[(unsigned char)*p]->add_string(p+1, symbol_key). Recursion consumes
> one byte per level; the symbol is stored at the leaf indexed by the final byte.

> [spec:hfst:def:transducer.hfst-ol.ol-letter-trie.find-key-fn]
> SymbolNumber OlLetterTrie::find_key(char ** p)

> [spec:hfst:sem:transducer.hfst-ol.ol-letter-trie.find-key-fn]
> Greedily matches the longest string from the trie starting at the cursor `*p`,
> advancing the cursor past the consumed bytes, and returns the matched symbol
> (or NO_SYMBOL_NUMBER if none). Saves old_p = *p, then advances the cursor by
> one byte (++(*p)). If letters[(unsigned char)*old_p] is NULL (no deeper trie
> for this byte), returns symbols[(unsigned char)*old_p] (the symbol for the
> single byte, possibly NO_SYMBOL_NUMBER) with the cursor advanced by one. Else
> recurse: s = letters[byte]->find_key(p). If the recursion returned
> NO_SYMBOL_NUMBER (no longer match), back up the cursor by one (--(*p)) and
> return symbols[byte] (the shorter match at this node). Otherwise return s (the
> longer match). Net effect: longest-prefix match with backtracking by one node.

> [spec:hfst:def:transducer.hfst-ol.ol-letter-trie.has-key-starting-with-fn]
> bool OlLetterTrie::has_key_starting_with(const char c) const

> [spec:hfst:sem:transducer.hfst-ol.ol-letter-trie.has-key-starting-with-fn]
> Returns true iff the trie has a child node for byte `c`, i.e.
> letters[(unsigned char)c] != NULL. Pure, const.

> [spec:hfst:def:transducer.hfst-ol.ol-letter-trie.ol-letter-trie-fn]
> ~OlLetterTrie()

> [spec:hfst:sem:transducer.hfst-ol.ol-letter-trie.ol-letter-trie-fn]
> Destructor for OlLetterTrie. Iterates i from 0 to letters.size()-1 and
> `delete letters[i]` (deleting each child OlLetterTrie pointer; deleting a NULL
> pointer is a no-op), then sets letters[i] = 0. Child destructors recurse,
> freeing the whole subtree. The `symbols` vector is freed automatically.

> [spec:hfst:def:transducer.hfst-ol.operation-map]
> typedef std::map<SymbolNumber, hfst::FdOperation> OperationMap

> [spec:hfst:def:transducer.hfst-ol.s-transition]
> class STransition {
>   TransitionTableIndex index;
>   SymbolNumber symbol;
>   Weight weight;
> }

> [spec:hfst:def:transducer.hfst-ol.s-transition.s-transition-fn]
> STransition(TransitionTableIndex i,

> [spec:hfst:sem:transducer.hfst-ol.s-transition.s-transition-fn]
> Three-argument constructor for STransition. Initializes member index = i,
> symbol = s, weight = w from the parameters. Empty body. (A separate two-arg
> overload defaults weight to 0.0.)

> [spec:hfst:def:transducer.hfst-ol.should-ascii-tokenize-fn]
> bool should_ascii_tokenize(unsigned char c)

> [spec:hfst:sem:transducer.hfst-ol.should-ascii-tokenize-fn]
> Returns true iff byte `c` is in the ascii range, i.e. c <= 127. Used to decide
> whether a byte is eligible for the fast single-byte ascii tokenization path.

> [spec:hfst:def:transducer.hfst-ol.speller]
> class Speller {
>   Transducer * mutator;
>   Transducer * lexicon;
>   InputString input;
>   TreeNodeQueue queue;
>   SymbolNumberVector alphabet_translator;
>   std::vector<std::string> symbol_table;
> }

> [spec:hfst:def:transducer.hfst-ol.speller.build-alphabet-translator-fn]
> void build_alphabet_translator(void)

> [spec:hfst:sem:transducer.hfst-ol.speller.build-alphabet-translator-fn]
> Builds `alphabet_translator`, a vector mapping each mutator symbol number to
> the corresponding lexicon symbol number. Gets `from` = mutator's alphabet,
> `to` = lexicon's alphabet, from_keys = from.get_symbol_table(), and to_symbols
> = to.build_string_symbol_map() (lexicon string -> symbol number). Pushes 0 as
> the first element (epsilon maps to epsilon). Then for each i from 1 to
> from_keys.size()-1: if mutator symbol i is a flag diacritic
> (from.is_flag_diacritic(i)) OR i == from.get_unknown_symbol() (the OTHER
> symbol), push NO_SYMBOL_NUMBER (no translation) and continue. Otherwise, if the
> mutator's string from_keys[i] is not present exactly once in to_symbols and the
> string is non-empty, throw AlphabetTranslationException(from_keys[i]). Finally
> push to_symbols[from_keys[i]] — the lexicon symbol number for that string.
> (When the string is empty and absent, it falls through to the push, looking up
> an empty key.)

> [spec:hfst:def:transducer.hfst-ol.speller.check-fn]
> bool check(char * line)

> [spec:hfst:sem:transducer.hfst-ol.speller.check-fn]
> Tests whether `line` is accepted by the lexicon transducer; returns true/false.
> Calls init_input(line, lexicon->get_encoder(), NO_SYMBOL_NUMBER) (no OTHER
> symbol, so untokenizable input fails); if that returns false, return false.
> Builds a start TreeNode from lexicon->get_fd_table() and sets queue to that
> single node (queue.assign(1, start_node)). Then loops while queue is non-empty:
> if the front node has consumed all input (input_state == input.len()) AND the
> lexicon is in a final state (lexicon->final_index(front.lexicon_state)), return
> true immediately. Otherwise expand the front node by calling lexicon_epsilons()
> then lexicon_consume() (both push successors onto the queue), then pop the
> front. If the queue empties without reaching an accepting state, return false.

> [spec:hfst:def:transducer.hfst-ol.speller.consume-input-fn]
> void consume_input(void)

> [spec:hfst:sem:transducer.hfst-ol.speller.consume-input-fn]
> Expands the front queue node by consuming one input symbol simultaneously in
> the mutator and lexicon. Let input_state = front.input_state. Returns
> immediately (no-op) if input_state >= input.len() (no input left) or the
> mutator has no transitions on input[input_state] from front.mutator_state+1.
> Otherwise next_m = mutator->next(front.mutator_state, input[input_state]);
> iterate mutator_i_s = mutator->take_non_epsilons(next_m, input[input_state])
> while its symbol != NO_SYMBOL_NUMBER:
> - If mutator_i_s.symbol == 0 (mutator output is epsilon): push front.update(0,
>   input_state+1, mutator_i_s.index, front.lexicon_state, mutator_i_s.weight)
>   onto the queue (advance input and mutator, lexicon unchanged).
> - Else translate via alphabet_translator[mutator_i_s.symbol]; if the lexicon
>   has no transitions on that translated symbol from front.lexicon_state+1,
>   advance to the next mutator arc (++next_m; re-take) and continue. Otherwise
>   next_l = lexicon->next(front.lexicon_state, translated); inner loop over
>   lexicon_i_s = lexicon->take_non_epsilons(next_l, translated) while its symbol
>   != NO_SYMBOL_NUMBER: push front.update(lexicon_i_s.symbol, input_state+1,
>   mutator_i_s.index, lexicon_i_s.index, lexicon_i_s.weight + mutator_i_s.weight),
>   then advance next_l and re-take.
> After handling each mutator arc, advance ++next_m and re-take the next mutator
> non-epsilon. Mutates the queue by pushing successor TreeNodes; no return value.

> [spec:hfst:def:transducer.hfst-ol.speller.correct-fn]
> CorrectionQueue correct(char * line)

> [spec:hfst:sem:transducer.hfst-ol.speller.correct-fn]
> Produces a CorrectionQueue (priority queue) of corrections of `line` from the
> mutator+lexicon composition. Calls init_input(line, mutator->get_encoder(),
> mutator->get_unknown_symbol()) (OTHER symbol from mutator); if it fails, return
> an empty CorrectionQueue. Maintains a std::map<std::string, Weight> corrections.
> Builds a start TreeNode from lexicon->get_fd_table() and sets queue to that
> single node (queue.assign(1, start_node)). Loop while queue non-empty: call
> lexicon_epsilons() then mutator_epsilons() (expanding via epsilon/flag arcs).
> If the front node has consumed all input (input_state == input.len()): if both
> mutator and lexicon are in final states
> (mutator->final_index(front.mutator_state) and
> lexicon->final_index(front.lexicon_state)), compute string = stringify(
> front.string) and weight = front.weight + lexicon->final_weight(
> front.lexicon_state) + mutator->final_weight(front.mutator_state); if this
> string is new in corrections or has a lower weight than the stored one, store
> corrections[string] = weight. Else (input remaining) call consume_input(). Then
> pop the front. After the loop, build a CorrectionQueue by pushing a
> StringWeightPair(string, weight) for each entry in `corrections` (map iteration
> order, i.e. sorted by string), and return it (priority ordering applied by the
> queue's comparator).

> [spec:hfst:def:transducer.hfst-ol.speller.init-input-fn]
> bool init_input(char * str, const Encoder & encoder, SymbolNumber other)

> [spec:hfst:sem:transducer.hfst-ol.speller.init-input-fn]
> Thin wrapper: returns input.initialize(encoder, str, other), tokenizing the
> C-string `str` into this Speller's `input` member using `encoder` and the
> OTHER-symbol fallback `other`. Returns the bool success flag from initialize.

> [spec:hfst:def:transducer.hfst-ol.speller.lexicon-consume-fn]
> void lexicon_consume(void)

> [spec:hfst:sem:transducer.hfst-ol.speller.lexicon-consume-fn]
> Expands the front queue node by consuming one input symbol directly in the
> lexicon only (used by check()). Let input_state = front.input_state. Returns
> immediately if input_state >= input.len() or the lexicon has no transitions on
> input[input_state] from front.lexicon_state+1. Otherwise next =
> lexicon->next(front.lexicon_state, input[input_state]); iterate i_s =
> lexicon->take_non_epsilons(next, input[input_state]) while its symbol !=
> NO_SYMBOL_NUMBER: push front.update(i_s.symbol, input_state+1,
> front.mutator_state, i_s.index, i_s.weight) onto the queue (input advances by
> one, mutator_state unchanged, lexicon_state becomes i_s.index), then advance
> ++next and re-take. Mutates the queue; no return value.

> [spec:hfst:def:transducer.hfst-ol.speller.lexicon-epsilons-fn]
> void lexicon_epsilons(void)

> [spec:hfst:sem:transducer.hfst-ol.speller.lexicon-epsilons-fn]
> Expands the front queue node by following the lexicon's epsilon and flag-
> diacritic arcs (input unchanged). Returns immediately if the lexicon has no
> epsilons-or-flags at front.lexicon_state+1
> (!lexicon->has_epsilons_or_flags(...)). Otherwise next =
> lexicon->next(front.lexicon_state, 0); iterate i_s =
> lexicon->take_epsilons_and_flags(next) while its symbol != NO_SYMBOL_NUMBER:
> - If the lexicon transition at `next` has input symbol 0 (a true epsilon arc):
>   push front.update_lexicon(i_s.symbol, i_s.index, i_s.weight) (advance only the
>   lexicon, recording output symbol).
> - Else it is a flag-diacritic arc: take a copy `front` of queue.front(); call
>   front.flag_state.apply_operation(transition's input symbol); if that succeeds
>   (returns true), push front.update_lexicon(i_s.symbol, i_s.index, i_s.weight)
>   with the updated flag state; if it fails, skip.
> After each arc, advance ++next and re-take. Mutates the queue; no return value.

> [spec:hfst:def:transducer.hfst-ol.speller.mutator-epsilons-fn]
> void mutator_epsilons(void)

> [spec:hfst:sem:transducer.hfst-ol.speller.mutator-epsilons-fn]
> Expands the front queue node by following the mutator's epsilon-input arcs
> (no input consumed), pairing them with the lexicon. Returns immediately if the
> mutator has no transitions on 0 from front.mutator_state+1. Otherwise next_m =
> mutator->next(front.mutator_state, 0); iterate mutator_i_s =
> mutator->take_epsilons(next_m) while its symbol != NO_SYMBOL_NUMBER:
> - If mutator_i_s.symbol == 0 (mutator output also epsilon): push
>   front.update_mutator(mutator_i_s.symbol, mutator_i_s.index,
>   mutator_i_s.weight) (advance only the mutator).
> - Else translate via alphabet_translator[mutator_i_s.symbol]; if the lexicon
>   has no transitions on that translated symbol from front.lexicon_state+1,
>   advance to the next mutator arc and continue. Otherwise next_l =
>   lexicon->next(front.lexicon_state, translated); inner loop over lexicon_i_s =
>   lexicon->take_non_epsilons(next_l, translated) while its symbol !=
>   NO_SYMBOL_NUMBER: push front.update(lexicon_i_s.symbol, mutator_i_s.index,
>   lexicon_i_s.index, lexicon_i_s.weight + mutator_i_s.weight) (input_state
>   unchanged via this 4-arg update), advance next_l and re-take.
> After each mutator arc, ++next_m and re-take. Mutates the queue; no return.

> [spec:hfst:def:transducer.hfst-ol.speller.speller-fn]
> Speller(Transducer * mutator_ptr, Transducer * lexicon_ptr)

> [spec:hfst:sem:transducer.hfst-ol.speller.speller-fn]
> Constructs a Speller from two Transducer pointers. Stores mutator =
> mutator_ptr and lexicon = lexicon_ptr (non-owning). Initializes `input` to a
> default InputString, `queue` to an empty TreeNodeQueue, `alphabet_translator`
> to an empty SymbolNumberVector, and `symbol_table` to
> lexicon->get_symbol_table() (the lexicon's symbol strings, used by stringify).
> Then calls build_alphabet_translator() to populate alphabet_translator, which
> may throw AlphabetTranslationException if a mutator symbol cannot be mapped to
> the lexicon alphabet.

> [spec:hfst:def:transducer.hfst-ol.speller.stringify-fn]
> std::string stringify(SymbolNumberVector symbol_vector)

> [spec:hfst:sem:transducer.hfst-ol.speller.stringify-fn]
> Converts a SymbolNumberVector to its concatenated string form. Starts with an
> empty std::string s; for each symbol number in `symbol_vector` (in order),
> append symbol_table[symbol] (the lexicon's string for that symbol). Returns the
> resulting string. `symbol_vector` is taken by value.

> [spec:hfst:def:transducer.hfst-ol.state-id-number]
> typedef unsigned int StateIdNumber

> [spec:hfst:def:transducer.hfst-ol.string-pair]
> typedef std::pair<std::string, std::string> StringPair

> [spec:hfst:def:transducer.hfst-ol.string-symbol-map]
> typedef std::map<std::string, SymbolNumber> StringSymbolMap

> [spec:hfst:def:transducer.hfst-ol.string-weight-comparison]
> class StringWeightComparison {
>   bool reverse;
> }

> [spec:hfst:def:transducer.hfst-ol.string-weight-comparison.operator-fn]
> bool operator() (StringWeightPair lhs, StringWeightPair rhs)

> [spec:hfst:sem:transducer.hfst-ol.string-weight-comparison.operator-fn]
> Comparison functor for ordering StringWeightPair entries in a priority queue.
> Compares the weight components (the `.second` fields). If `reverse` is true,
> returns (lhs.second < rhs.second); otherwise returns (lhs.second > rhs.second).
> Both arguments are taken by value. The default (non-reversed) form returns true
> when lhs has the greater weight, which causes std::priority_queue to treat the
> lower-weight pair as higher priority; reverse=true inverts this ordering.

> [spec:hfst:def:transducer.hfst-ol.string-weight-comparison.string-weight-comparison-fn]
> StringWeightComparison(bool reverse_result=false)

> [spec:hfst:sem:transducer.hfst-ol.string-weight-comparison.string-weight-comparison-fn]
> Constructor for StringWeightComparison. Initializes the member `reverse` from
> the parameter `reverse_result`, which defaults to false. Empty body.

> [spec:hfst:def:transducer.hfst-ol.string-weight-pair]
> typedef std::pair<std::string, Weight> StringWeightPair

> [spec:hfst:def:transducer.hfst-ol.symbol-number]
> typedef unsigned short SymbolNumber

> [spec:hfst:def:transducer.hfst-ol.symbol-number-set]
> typedef std::set<SymbolNumber> SymbolNumberSet

> [spec:hfst:def:transducer.hfst-ol.symbol-number-vector]
> typedef std::vector<SymbolNumber> SymbolNumberVector

> [spec:hfst:def:transducer.hfst-ol.symbol-pair]
> struct SymbolPair {
>   SymbolNumber input;
>   SymbolNumber output;
> }

> [spec:hfst:def:transducer.hfst-ol.symbol-pair.symbol-pair-fn]
> SymbolPair(void): input(0), output(0)

> [spec:hfst:sem:transducer.hfst-ol.symbol-pair.symbol-pair-fn]
> Default constructor for SymbolPair. Initializes both members input = 0 and
> output = 0 (epsilon). Empty body. (A separate two-arg overload sets
> input = i and output = o.)

> [spec:hfst:def:transducer.hfst-ol.symbol-table]
> typedef std::vector<std::string> SymbolTable

> [spec:hfst:def:transducer.hfst-ol.tape]
> class Tape: public SymbolNumberVector

> [spec:hfst:def:transducer.hfst-ol.tape.write-fn]
> void write(unsigned int i, SymbolNumber s)

> [spec:hfst:sem:transducer.hfst-ol.tape.write-fn]
> Writes symbol `s` at index `i` of the Tape (a vector<SymbolNumber>), growing
> the tape if needed. If the tape already has size > i, set this[i] = s. Otherwise
> push_back NO_SYMBOL_NUMBER until size() > i (i.e. while size() <= i), then set
> this[i] = s. Net effect: index i always exists afterwards and holds s; any
> positions created by growth before index i hold NO_SYMBOL_NUMBER.

> [spec:hfst:def:transducer.hfst-ol.transducer]
> class Transducer {
>   TransducerHeader* header;
>   TransducerAlphabet* alphabet;
>   TransducerTablesInterface* tables;
>   Weight current_weight;
>   HfstTwoLevelPaths * lookup_paths;
>   Encoder * encoder;
>   Tape input_tape;
>   DoubleTape output_tape;
>   hfst::FdState<SymbolNumber> flag_state;
>   bool found_transition;
>   TraversalStates traversal_states;
>   ssize_t max_lookups;
>   unsigned int recursion_depth_left;
>   double max_time;
>   clock_t start_clock;
> }

> [spec:hfst:def:transducer.hfst-ol.transducer-alphabet]
> class TransducerAlphabet {
>   SymbolTable symbol_table;
>   hfst::FdTable<SymbolNumber> fd_table;
>   SymbolNumber unknown_symbol;
>   SymbolNumber default_symbol;
>   SymbolNumber identity_symbol;
>   SymbolNumber orig_symbol_count;
>   enum UnicodeClassCacheValue { upperalpha, loweralpha, whitespace, no_value, other };
>   std::vector<UnicodeClassCacheValue> unicode_cache;
> }

> [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.add-symbol-fn]
> void TransducerAlphabet::add_symbol(char * symbol)

> [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.add-symbol-fn]
> Appends a new symbol string to the alphabet's symbol_table via push_back(symbol),
> assigning it the next symbol number (its index). The `char *` overload converts
> the C-string to std::string on insertion. (A parallel const std::string& overload
> behaves identically.) No return value; no other state updated.

> [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.build-string-symbol-map-fn]
> StringSymbolMap TransducerAlphabet::build_string_symbol_map(void) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.build-string-symbol-map-fn]
> Builds and returns a StringSymbolMap (std::map<std::string, SymbolNumber>)
> mapping each symbol string to its symbol number. Iterates i from 0 to
> symbol_table.size()-1 and assigns ss_map[symbol_table[i]] = i. If the same
> string occurs multiple times, the last index wins (later assignment overwrites).
> Returns the map by value. Const.

> [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.cache-unicode-class-fn]
> void TransducerAlphabet::cache_unicode_class(SymbolNumber symbol)

> [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.cache-unicode-class-fn]
> Computes and caches the Unicode character class of the first code point of the
> symbol string for `symbol`. First grows unicode_cache by push_back-ing `no_value`
> until unicode_cache.size() > symbol (while size() <= symbol). If
> unicode_cache[symbol] is already not no_value, return immediately (already
> cached). Otherwise decode symbol_table[symbol] from UTF-8 into an ICU
> UnicodeString `us`. If us.countChar32() > 0, inspect the first code point
> us.char32At(0): if u_islower set cache to loweralpha; else if u_isupper set
> upperalpha; else if u_isUWhiteSpace set whitespace; else set other. (If the
> string has no code points, the cache entry is left as no_value.) Mutates
> unicode_cache; no return value.

> [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.display-fn]
> void TransducerAlphabet::display() const

> [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.display-fn]
> Prints the alphabet to std::cout for debugging. Writes the line "Transducer
> alphabet:" then, for each i from 0 to symbol_table.size()-1, a line
> " Symbol " << i << ": " << symbol_table[i]. Each output ends with std::endl.
> Const; side effect is stdout output only.

> [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.fake-read-alphabet-fn]
> void TransducerAlphabet::fake_read_alphabet(std::istream& is,

> [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.fake-read-alphabet-fn]
> Skips over `symbol_count` NUL-terminated symbol strings in the input stream `is`
> without storing them. Loops i from 0 to symbol_count-1, each iteration calling
> std::getline(is, str, '\0') into a throwaway local std::string. Static; no return
> value. Purpose: advance the stream past an alphabet section that is not needed.

> [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.get-default-symbol-fn]
> SymbolNumber get_default_symbol(void) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.get-default-symbol-fn]
> Getter: returns the member `default_symbol` (the symbol number of the @_UNKNOWN_
> default/@_DEFAULT symbol, or NO_SYMBOL_NUMBER if none). Const, pure.

> [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.get-identity-symbol-fn]
> SymbolNumber get_identity_symbol(void) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.get-identity-symbol-fn]
> Getter: returns the member `identity_symbol` (the symbol number of the
> @_IDENTITY_SYMBOL_@, or NO_SYMBOL_NUMBER if none). Const, pure.

> [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.get-operation-fn]
> const hfst::FdOperation * get_operation(SymbolNumber symbol) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.get-operation-fn]
> Returns the flag-diacritic operation associated with `symbol` by delegating to
> fd_table.get_operation(symbol), returning a const hfst::FdOperation* (NULL if the
> symbol is not a flag diacritic). Const, pure forwarding accessor.

> [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.get-orig-symbol-count-fn]
> SymbolNumber get_orig_symbol_count(void) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.get-orig-symbol-count-fn]
> Getter: returns the member `orig_symbol_count`, the number of symbols in the
> symbol_table as captured at construction time. Const, pure.

> [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.get-unknown-symbol-fn]
> SymbolNumber get_unknown_symbol(void) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.get-unknown-symbol-fn]
> Getter: returns the member `unknown_symbol` (the symbol number of the OTHER /
> @_UNKNOWN_SYMBOL_@, or NO_SYMBOL_NUMBER if none). Const, pure.

> [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.has-flag-diacritics-fn]
> bool has_flag_diacritics() const

> [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.has-flag-diacritics-fn]
> Returns true iff the alphabet contains any flag-diacritic features, i.e.
> fd_table.num_features() > 0. Const, pure forwarding accessor.

> [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.is-flag-diacritic-fn]
> bool is_flag_diacritic(SymbolNumber symbol) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.is-flag-diacritic-fn]
> Returns true iff `symbol` is a flag diacritic, by delegating to
> fd_table.is_diacritic(symbol). Const, pure forwarding accessor.

> [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.is-like-epsilon-fn]
> bool TransducerAlphabet::is_like_epsilon(SymbolNumber symbol) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.is-like-epsilon-fn]
> Returns true iff `symbol` behaves like epsilon (consumes no input). First, if
> fd_table.is_diacritic(symbol), return true. Then if symbol >= symbol_table.size(),
> return false (out of range). Otherwise let s = symbol_table[symbol]; return true
> iff s is an "Insert" symbol of the form @I.something@, detected as: s.size() >= 5
> AND s[0]=='@' AND s[1]=='I' AND s[2]=='.' AND s[s.size()-1]=='@'. Otherwise return
> false. Const.

> [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.is-meta-arc-fn]
> bool TransducerAlphabet::is_meta_arc(SymbolNumber symbol) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.is-meta-arc-fn]
> Returns true iff `symbol` is one of the special meta symbols. If symbol ==
> NO_SYMBOL_NUMBER, return false. Otherwise return true iff symbol equals any of
> unknown_symbol, default_symbol, or identity_symbol. Const.

> [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.is-unicode-alpha-fn]
> bool TransducerAlphabet::is_unicode_alpha(SymbolNumber symbol)

> [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.is-unicode-alpha-fn]
> Returns true iff the first code point of `symbol`'s string is alphabetic (lower
> or upper case). First calls cache_unicode_class(symbol) to ensure the class is
> computed and cached, then returns (unicode_cache[symbol] == loweralpha ||
> unicode_cache[symbol] == upperalpha). Non-const (may grow/populate the cache).

> [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.is-unicode-loweralpha-fn]
> bool TransducerAlphabet::is_unicode_loweralpha(SymbolNumber symbol)

> [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.is-unicode-loweralpha-fn]
> Returns true iff the first code point of `symbol`'s string is lowercase. Calls
> cache_unicode_class(symbol), then returns unicode_cache[symbol] == loweralpha.
> Non-const (may populate the cache).

> [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.is-unicode-upperalpha-fn]
> bool TransducerAlphabet::is_unicode_upperalpha(SymbolNumber symbol)

> [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.is-unicode-upperalpha-fn]
> Returns true iff the first code point of `symbol`'s string is uppercase. Calls
> cache_unicode_class(symbol), then returns unicode_cache[symbol] == upperalpha.
> Non-const (may populate the cache).

> [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.is-unicode-whitespace-fn]
> bool TransducerAlphabet::is_unicode_whitespace(SymbolNumber symbol)

> [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.is-unicode-whitespace-fn]
> Returns true iff the first code point of `symbol`'s string is Unicode whitespace.
> Calls cache_unicode_class(symbol), then returns unicode_cache[symbol] ==
> whitespace. Non-const (may populate the cache).

> [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.string-from-symbol-fn]
> const std::string string_from_symbol(const SymbolNumber symbol) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.string-from-symbol-fn]
> Returns the string for `symbol`. If symbol == 0 (epsilon), returns the empty
> string ""; otherwise returns symbol_table[symbol] (no bounds checking). Const.

> [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.symbol-from-string-fn]
> SymbolNumber TransducerAlphabet::symbol_from_string(

> [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.symbol-from-string-fn]
> Linear-search reverse lookup: returns the symbol number whose string equals
> `symbol_string`. Iterates i from 0 to symbol_table.size()-1 and returns the first
> i with symbol_table[i] == symbol_string. If no match is found, returns
> NO_SYMBOL_NUMBER. Const. (Returns the lowest matching index if duplicates exist.)

> [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.transducer-alphabet-fn]
> TransducerAlphabet::TransducerAlphabet(std::istream& is,

> [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.transducer-alphabet-fn]
> Constructs a TransducerAlphabet by reading `symbol_count` NUL-terminated symbol
> strings from input stream `is`. Initializes unknown_symbol, identity_symbol, and
> default_symbol all to NO_SYMBOL_NUMBER. For i from 0 to symbol_count-1: read one
> string via std::getline(is, str, '\0'). Then classify: if
> hfst::FdOperation::is_diacritic(str), call fd_table.define_diacritic(i, str) and,
> unless preserve_diacritic_strings is true, replace str with "" (so the stored
> string for the diacritic is blanked). Else if hfst::is_unknown(str) set
> unknown_symbol = i; else if hfst::is_default(str) set default_symbol = i; else if
> hfst::is_identity(str) set identity_symbol = i. After classification, if the
> stream is in a failed state (!is), HFST_THROW(TransducerHasWrongTypeException).
> Then push the (possibly blanked) str onto symbol_table (converting via c_str()).
> After the loop, set orig_symbol_count = symbol_table.size(). The third parameter
> `preserve_diacritic_strings` controls whether diacritic strings are kept.

> [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.unicode-class-cache-value]
> enum UnicodeClassCacheValue {
>   upperalpha;
>   loweralpha;
>   whitespace;
>   no_value;
>   other;
> }

> [spec:hfst:def:transducer.hfst-ol.transducer-alphabet.write-fn]
> void write(std::ostream& os) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-alphabet.write-fn]
> Serializes the alphabet to output stream `os`. Iterates over symbol_table in
> order; for each symbol string, writes the string to os (os << *i) followed by a
> NUL byte (os.put('\0')). Const; produces a sequence of NUL-terminated symbol
> strings, the inverse of the istream constructor's reading loop.

> [spec:hfst:def:transducer.hfst-ol.transducer-header]
> class TransducerHeader {
>   SymbolNumber number_of_input_symbols;
>   SymbolNumber number_of_symbols;
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

> [spec:hfst:def:transducer.hfst-ol.transducer-header.display-fn]
> void display() const

> [spec:hfst:sem:transducer.hfst-ol.transducer-header.display-fn]
> Prints all header fields to std::cout for debugging. Writes the line "Transducer
> properties:" then one " <name>: <value>" line (each ended with std::endl) for
> each member in order: number_of_symbols, number_of_input_symbols,
> size_of_transition_index_table, size_of_transition_target_table, number_of_states,
> number_of_transitions, weighted, deterministic, input_deterministic, minimized,
> cyclic, and the remaining boolean epsilon-transition flags. Const; side effect is
> stdout output only.

> [spec:hfst:def:transducer.hfst-ol.transducer-header.header-error-fn]
> static void header_error()

> [spec:hfst:sem:transducer.hfst-ol.transducer-header.header-error-fn]
> Static helper that unconditionally throws a TransducerHasWrongTypeException (via
> HFST_THROW). Never returns. Used to signal a malformed/incompatible header.

> [spec:hfst:def:transducer.hfst-ol.transducer-header.increment-symbol-count-fn]
> void increment_symbol_count(void)

> [spec:hfst:sem:transducer.hfst-ol.transducer-header.increment-symbol-count-fn]
> Increments both number_of_symbols and number_of_input_symbols by one
> (++number_of_symbols; ++number_of_input_symbols). No return value.

> [spec:hfst:def:transducer.hfst-ol.transducer-header.index-table-size-fn]
> TransitionTableIndex index_table_size(void) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-header.index-table-size-fn]
> Getter: returns the member `size_of_transition_index_table` (the number of
> entries in the transition index table). Const, pure.

> [spec:hfst:def:transducer.hfst-ol.transducer-header.input-symbol-count-fn]
> SymbolNumber input_symbol_count(void) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-header.input-symbol-count-fn]
> Getter: returns the member `number_of_input_symbols` (the count of input-side
> symbols in the alphabet). Const, pure.

> [spec:hfst:def:transducer.hfst-ol.transducer-header.probe-flag-fn]
> bool probe_flag(HeaderFlag flag) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-header.probe-flag-fn]
> Returns the current boolean value of the header property selected by the
> HeaderFlag `flag`. A switch on `flag` returns the corresponding member:
> Weighted->weighted, Deterministic->deterministic,
> Input_deterministic->input_deterministic, Minimized->minimized,
> Cyclic->cyclic, Has_epsilon_epsilon_transitions->
> has_epsilon_epsilon_transitions, Has_input_epsilon_transitions->
> has_input_epsilon_transitions, Has_input_epsilon_cycles->
> has_input_epsilon_cycles, Has_unweighted_input_epsilon_cycles->
> has_unweighted_input_epsilon_cycles. If `flag` matches none, returns false.
> Const, pure.

> [spec:hfst:def:transducer.hfst-ol.transducer-header.read-bool-property-fn]
> static bool read_bool_property(std::istream& is)

> [spec:hfst:sem:transducer.hfst-ol.transducer-header.read-bool-property-fn]
> Static. Reads one boolean property from input stream `is`, encoded as a raw
> unsigned int. Declares `unsigned int prop` and does is.read into it with
> sizeof(unsigned int) bytes (raw little-/native-endian binary read). If prop ==
> 0 return false; if prop == 1 return true; otherwise call header_error() (which
> throws TransducerHasWrongTypeException) and then return false (unreachable).
> Mutates the stream cursor by sizeof(unsigned int) bytes.

> [spec:hfst:def:transducer.hfst-ol.transducer-header.read-property-fn]
> static T read_property(std::istream& is)

> [spec:hfst:sem:transducer.hfst-ol.transducer-header.read-property-fn]
> Static template over T. Reads one value of type T from input stream `is` as a
> raw binary blob. Declares a local `T p`, does is.read(reinterpret_cast<char*>(
> &p), sizeof(T)) (raw native-byte-order read of sizeof(T) bytes into p), and
> returns p. Mutates the stream cursor by sizeof(T) bytes. No validation of the
> read result is performed here.

> [spec:hfst:def:transducer.hfst-ol.transducer-header.set-flag-fn]
> void set_flag(HeaderFlag flag, bool value)

> [spec:hfst:sem:transducer.hfst-ol.transducer-header.set-flag-fn]
> Sets the header property selected by `flag` to true. NOTE: the `value`
> parameter is ignored — it is cast to void and the matched member is always set
> to true regardless of `value`. A switch on `flag` sets the corresponding
> member to true and breaks: Weighted->weighted, Deterministic->deterministic,
> Input_deterministic->input_deterministic, Minimized->minimized,
> Cyclic->cyclic, Has_epsilon_epsilon_transitions, Has_input_epsilon_transitions,
> Has_input_epsilon_cycles. The Has_unweighted_input_epsilon_cycles case sets its
> member true but (lacking a break) falls through into the default case, which
> returns. Any unmatched flag hits default and returns with no change. No return
> value.

> [spec:hfst:def:transducer.hfst-ol.transducer-header.symbol-count-fn]
> SymbolNumber symbol_count(void) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-header.symbol-count-fn]
> Getter: returns the member `number_of_symbols` (total number of symbols in the
> alphabet, including epsilon). Const, pure.

> [spec:hfst:def:transducer.hfst-ol.transducer-header.target-table-size-fn]
> TransitionTableIndex target_table_size(void) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-header.target-table-size-fn]
> Getter: returns the member `size_of_transition_target_table` (the number of
> entries in the transition target/transition table). Const, pure.

> [spec:hfst:def:transducer.hfst-ol.transducer-header.transducer-header-fn]
> TransducerHeader(std::istream& is)

> [spec:hfst:sem:transducer.hfst-ol.transducer-header.transducer-header-fn]
> Constructs a TransducerHeader by reading all fields in binary order from input
> stream `is`, via the member initializer list (which evaluates in declaration
> order). Reads, in this exact sequence: number_of_input_symbols =
> read_property<SymbolNumber>(is); number_of_symbols =
> read_property<SymbolNumber>(is); size_of_transition_index_table =
> read_property<TransitionTableIndex>(is); size_of_transition_target_table =
> read_property<TransitionTableIndex>(is); number_of_states =
> read_property<StateIdNumber>(is); number_of_transitions =
> read_property<TransitionNumber>(is); then nine boolean flags each via
> read_bool_property(is) in order: weighted, deterministic, input_deterministic,
> minimized, cyclic, has_epsilon_epsilon_transitions,
> has_input_epsilon_transitions, has_input_epsilon_cycles,
> has_unweighted_input_epsilon_cycles. After reading, the body checks: if (!is)
> (the stream is in a failed/eof/bad state), HFST_THROW(
> TransducerHasWrongTypeException). Note read_bool_property itself may throw if a
> bool field is neither 0 nor 1. Mutates the stream cursor.

> [spec:hfst:def:transducer.hfst-ol.transducer-header.write-bool-property-fn]
> static void write_bool_property(bool value, std::ostream& os)

> [spec:hfst:sem:transducer.hfst-ol.transducer-header.write-bool-property-fn]
> Static. Writes one boolean `value` to output stream `os` as a raw unsigned int.
> Computes `unsigned int prop = (value ? 1 : 0)` and does os.write(
> reinterpret_cast<char*>(&prop), sizeof(prop)) (raw native-byte-order write of
> sizeof(unsigned int) bytes). No return value. The inverse of
> read_bool_property.

> [spec:hfst:def:transducer.hfst-ol.transducer-header.write-fn]
> void write(std::ostream& os) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-header.write-fn]
> Serializes the whole header to output stream `os` in binary, in the exact order
> matching the istream constructor's reads. Calls write_property for, in order:
> number_of_input_symbols, number_of_symbols, size_of_transition_index_table,
> size_of_transition_target_table, number_of_states, number_of_transitions. Then
> calls write_bool_property for, in order: weighted, deterministic,
> input_deterministic, minimized, cyclic, has_epsilon_epsilon_transitions,
> has_input_epsilon_transitions, has_input_epsilon_cycles,
> has_unweighted_input_epsilon_cycles. Const; side effect is the byte output. The
> inverse of TransducerHeader(std::istream&).

> [spec:hfst:def:transducer.hfst-ol.transducer-header.write-property-fn]
> static void write_property(T prop, std::ostream& os)

> [spec:hfst:sem:transducer.hfst-ol.transducer-header.write-property-fn]
> Static template over T. Writes the value `prop` to output stream `os` as a raw
> binary blob: os.write(reinterpret_cast<const char*>(&prop), sizeof(prop)) (raw
> native-byte-order write of sizeof(T) bytes). No return value. The inverse of
> read_property<T>.

> [spec:hfst:def:transducer.hfst-ol.transducer-table]
> class TransducerTable {
>   std::vector<T> table;
> }

> [spec:hfst:def:transducer.hfst-ol.transducer-table.append-fn]
> void append(const T& v)

> [spec:hfst:sem:transducer.hfst-ol.transducer-table.append-fn]
> Appends a copy of element `v` to the end of the underlying `table` vector via
> table.push_back(v), growing the table by one. No return value.

> [spec:hfst:def:transducer.hfst-ol.transducer-table.display-fn]
> void display(bool transition_table) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-table.display-fn]
> Prints the table to std::cout for debugging. Loops i from 0 to table.size()-1:
> writes i; if the `transition_table` flag is true, additionally writes "/" then
> i+TRANSITION_TARGET_TABLE_START (the absolute transition-table index); then
> writes ": " and calls table[i].display() to print that entry. Const; side
> effect is stdout output only. The `transition_table` parameter just toggles
> showing the offset index for transition-table entries.

> [spec:hfst:def:transducer.hfst-ol.transducer-table.get-vector-fn]
> std::vector<T> get_vector(void) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-table.get-vector-fn]
> Returns a copy of the underlying `table` vector, by constructing and returning
> std::vector<T>(table). Const. Callers receive an independent copy.

> [spec:hfst:def:transducer.hfst-ol.transducer-table.set-fn]
> void set(size_t index, const T& v)

> [spec:hfst:sem:transducer.hfst-ol.transducer-table.set-fn]
> Overwrites the element at position `index` with a copy of `v` via table[index]
> = v. No bounds checking (uses vector::operator[]); `index` must already be a
> valid position. No return value.

> [spec:hfst:def:transducer.hfst-ol.transducer-table.size-fn]
> unsigned int size() const

> [spec:hfst:sem:transducer.hfst-ol.transducer-table.size-fn]
> Returns the number of entries in the table, table.size(), narrowed to unsigned
> int via hfst::size_t_to_uint. Const, pure.

> [spec:hfst:def:transducer.hfst-ol.transducer-table.transducer-table-fn]
> TransducerTable(

> [spec:hfst:sem:transducer.hfst-ol.transducer-table.transducer-table-fn]
> Constructs a TransducerTable<T> by reading `index_count` entries from input
> stream `is`. Initializes `table` to empty. Allocates a raw byte buffer p =
> malloc(T::size * index_count), reads exactly T::size * index_count bytes from
> `is` into p (is.read), saves p_orig = p. Then while index_count != 0:
> push_back T(p) (construct a T from the buffer cursor), decrement index_count,
> and advance p by T::size bytes. After the loop, free(p_orig). Each T is built
> from its fixed on-disk size T::size; the whole block is read at once then
> parsed entry-by-entry. Side effects: heap malloc/free and stream consumption.

> [spec:hfst:def:transducer.hfst-ol.transducer-tables]
> class TransducerTables : public TransducerTablesInterface {
>   TransducerTable<T1> index_table;
>   TransducerTable<T2> transition_table;
> }

> [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface]
> class TransducerTablesInterface {
>   virtual const TransitionIndex& get_index( TransitionTableIndex i) const = 0;
>   virtual const Transition& get_transition( TransitionTableIndex i) const = 0;
> }

> [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.display-fn]
> virtual void display() const

> [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.display-fn]
> Virtual hook for debug-printing the tables. Unlike the other interface members
> this one is NOT pure-virtual: it has an empty default body ({}) that does
> nothing. Concrete subclasses (TransducerTables) override it to print the index
> and transition tables to stdout. No return value.

> [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.get-final-weight-fn]
> virtual Weight get_final_weight(

> [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.get-final-weight-fn]
> Pure-virtual interface method (= 0; no body here). Contract: return the final
> weight of the index-table entry at index `i`. The concrete TransducerTables
> implementation returns index_table[i].final_weight(). Const.

> [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.get-index-finality-fn]
> virtual bool get_index_finality(

> [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.get-index-finality-fn]
> Pure-virtual interface method (= 0; no body here). Contract: return whether the
> index-table entry at index `i` is final. The concrete TransducerTables
> implementation returns index_table[i].final(). Const.

> [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.get-index-input-fn]
> virtual SymbolNumber get_index_input(

> [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.get-index-input-fn]
> Pure-virtual interface method (= 0; no body here). Contract: return the input
> symbol of the index-table entry at index `i`. The concrete TransducerTables
> implementation returns index_table[i].get_input_symbol(). Const.

> [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.get-index-target-fn]
> virtual TransitionTableIndex get_index_target(

> [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.get-index-target-fn]
> Pure-virtual interface method (= 0; no body here). Contract: return the target
> TransitionTableIndex of the index-table entry at index `i`. The concrete
> TransducerTables implementation returns index_table[i].get_target(). Const.

> [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.get-transition-finality-fn]
> virtual bool get_transition_finality(

> [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.get-transition-finality-fn]
> Pure-virtual interface method (= 0; no body here). Contract: return whether the
> transition-table entry at index `i` is final. The concrete TransducerTables
> implementation returns transition_table[i].final(). Const.

> [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.get-transition-input-fn]
> virtual SymbolNumber get_transition_input(

> [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.get-transition-input-fn]
> Pure-virtual interface method (= 0; no body here). Contract: return the input
> symbol of the transition-table entry at index `i`. The concrete
> TransducerTables implementation returns transition_table[i].get_input_symbol().
> Const.

> [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.get-transition-output-fn]
> virtual SymbolNumber get_transition_output(

> [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.get-transition-output-fn]
> Pure-virtual interface method (= 0; no body here). Contract: return the output
> symbol of the transition-table entry at index `i`. The concrete
> TransducerTables implementation returns
> transition_table[i].get_output_symbol(). Const.

> [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.get-transition-target-fn]
> virtual TransitionTableIndex get_transition_target(

> [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.get-transition-target-fn]
> Pure-virtual interface method (= 0; no body here). Contract: return the target
> TransitionTableIndex of the transition-table entry at index `i`. The concrete
> TransducerTables implementation returns transition_table[i].get_target().
> Const.

> [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.get-weight-fn]
> virtual Weight get_weight(

> [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.get-weight-fn]
> Pure-virtual interface method (= 0; no body here). Contract: return the weight
> of the transition-table entry at index `i`. The concrete TransducerTables
> implementation returns transition_table[i].get_weight(). Const.

> [spec:hfst:def:transducer.hfst-ol.transducer-tables-interface.transducer-tables-interface-fn]
> virtual ~TransducerTablesInterface()

> [spec:hfst:sem:transducer.hfst-ol.transducer-tables-interface.transducer-tables-interface-fn]
> Virtual destructor for the abstract base TransducerTablesInterface, with an
> empty body ({}). Its purpose is to make deletion through a base-class pointer
> destroy the derived object correctly (polymorphic destruction). No state of its
> own to release.

> [spec:hfst:def:transducer.hfst-ol.transducer-tables.display-fn]
> void display() const

> [spec:hfst:sem:transducer.hfst-ol.transducer-tables.display-fn]
> Override of the virtual display() for the concrete TransducerTables. Prints the
> line "Transition index table:" to std::cout, then calls index_table.display(
> false) (the index table, without offset indices). Then prints "Transition
> table:" and calls transition_table.display(true) (the transition table, with
> the TRANSITION_TARGET_TABLE_START-offset index shown). Const; side effect is
> stdout output only.

> [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-final-weight-fn]
> Weight get_final_weight(TransitionTableIndex i) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-final-weight-fn]
> Concrete override. Returns the final weight of the index-table entry at index
> `i`: index_table[i].final_weight(). Const, pure forwarding accessor.

> [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-index-finality-fn]
> bool get_index_finality(TransitionTableIndex i) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-index-finality-fn]
> Concrete override. Returns whether the index-table entry at index `i` is final:
> index_table[i].final(). Const, pure forwarding accessor.

> [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-index-input-fn]
> SymbolNumber get_index_input(TransitionTableIndex i) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-index-input-fn]
> Concrete override. Returns the input symbol of the index-table entry at index
> `i`: index_table[i].get_input_symbol(). Const, pure forwarding accessor.

> [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-index-target-fn]
> TransitionTableIndex get_index_target(TransitionTableIndex i) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-index-target-fn]
> Concrete override. Returns the target TransitionTableIndex of the index-table
> entry at index `i`: index_table[i].get_target(). Const, pure forwarding
> accessor.

> [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-transition-finality-fn]
> bool get_transition_finality(TransitionTableIndex i) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-transition-finality-fn]
> Returns whether the transition at index `i` in the transition_table is a final
> entry, by returning transition_table[i].final(). Const, pure forwarding
> accessor. (transition_table is indexed by `i` directly; `i` is relative to the
> transition table, not the absolute combined index.)

> [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-transition-input-fn]
> SymbolNumber get_transition_input(TransitionTableIndex i) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-transition-input-fn]
> Returns the input symbol of the transition at index `i` in the transition_table,
> by returning transition_table[i].get_input_symbol(). Const, pure forwarding
> accessor.

> [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-transition-output-fn]
> SymbolNumber get_transition_output(TransitionTableIndex i) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-transition-output-fn]
> Returns the output symbol of the transition at index `i` in the
> transition_table, by returning transition_table[i].get_output_symbol(). Const,
> pure forwarding accessor.

> [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-transition-target-fn]
> TransitionTableIndex get_transition_target(TransitionTableIndex i) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-transition-target-fn]
> Returns the target index of the transition at index `i` in the transition_table,
> by returning transition_table[i].get_target(). Const, pure forwarding accessor.

> [spec:hfst:def:transducer.hfst-ol.transducer-tables.get-weight-fn]
> Weight get_weight(TransitionTableIndex i) const

> [spec:hfst:sem:transducer.hfst-ol.transducer-tables.get-weight-fn]
> Returns the weight of the transition at index `i` in the transition_table, by
> returning transition_table[i].get_weight(). Const, pure forwarding accessor.

> [spec:hfst:def:transducer.hfst-ol.transducer-tables.transducer-tables-fn]
> TransducerTables(std::istream& is, TransitionTableIndex index_table_size,

> [spec:hfst:sem:transducer.hfst-ol.transducer-tables.transducer-tables-fn]
> Constructs a TransducerTables<T1, T2> (a TransducerTablesInterface
> implementation holding an index_table of TransducerTable<T1> and a
> transition_table of TransducerTable<T2>) by reading both tables from the input
> stream `is`. Via the member initializer list it constructs index_table from
> (is, index_table_size) and transition_table from (is, transition_table_size),
> i.e. each TransducerTable reads its given number of entries sequentially from
> the same stream — the index table first, then the transition table. Empty body.
> (Two other overloads exist: a default one that builds index_table with a single
> T1::create_final() entry and an empty transition_table, and a copy one that
> takes pre-built tables.)

> [spec:hfst:def:transducer.hfst-ol.transducer.copy-fn]
> Transducer * Transducer::copy(Transducer * t, bool weighted)

> [spec:hfst:sem:transducer.hfst-ol.transducer.copy-fn]
> Creates and returns a new heap-allocated Transducer that copies the contents of
> `t`. If `weighted` is true, constructs `another` = new Transducer(t->get_header(),
> t->get_alphabet(), t->copy_windex_table(), t->copy_transitionw_table()) — the
> weighted-table constructor. Otherwise constructs new Transducer(t->get_header(),
> t->get_alphabet(), t->copy_index_table(), t->copy_transition_table()) — the
> unweighted-table constructor. Returns the new pointer (caller owns it). The
> copy_*_table calls (which throw TransducerHasWrongTypeException if t's weighted
> flag disagrees) build independent copies of the index and transition tables.

> [spec:hfst:def:transducer.hfst-ol.transducer.copy-index-table-fn]
> TransducerTable<TransitionIndex> Transducer::copy_index_table()

> [spec:hfst:sem:transducer.hfst-ol.transducer.copy-index-table-fn]
> Builds and returns a fresh TransducerTable<TransitionIndex> copying this
> transducer's index table (unweighted variant). First, if the header's Weighted
> flag IS set (header->probe_flag(Weighted) is true), HFST_THROW(
> TransducerHasWrongTypeException) — this method is only valid for unweighted
> transducers. Otherwise create empty `another`, loop i from 0 to
> header->index_table_size()-1, and append a copy of tables->get_index(i) (the
> TransitionIndex at i) to `another`. Returns `another` by value.

> [spec:hfst:def:transducer.hfst-ol.transducer.copy-transition-table-fn]
> TransducerTable<Transition> Transducer::copy_transition_table()

> [spec:hfst:sem:transducer.hfst-ol.transducer.copy-transition-table-fn]
> Builds and returns a fresh TransducerTable<Transition> copying this transducer's
> transition table (unweighted variant). First, if header->probe_flag(Weighted) is
> true, HFST_THROW(TransducerHasWrongTypeException). Otherwise create empty
> `another`, loop i from 0 to header->target_table_size()-1, and append a copy of
> tables->get_transition(i) (the Transition at i) to `another`. Returns `another`
> by value.

> [spec:hfst:def:transducer.hfst-ol.transducer.copy-transitionw-table-fn]
> TransducerTable<TransitionW> Transducer::copy_transitionw_table()

> [spec:hfst:sem:transducer.hfst-ol.transducer.copy-transitionw-table-fn]
> Builds and returns a fresh TransducerTable<TransitionW> copying this
> transducer's weighted transition table. First, if header->probe_flag(Weighted)
> is FALSE (i.e. the transducer is NOT weighted), HFST_THROW(
> TransducerHasWrongTypeException). Otherwise create empty `another`, loop i from
> 0 to header->target_table_size()-1, and append TransitionW(get_transition_input(i),
> get_transition_output(i), get_transition_target(i), get_weight(i)) — i.e. a new
> weighted transition built from this transducer's transition fields including its
> weight. Returns `another` by value.

> [spec:hfst:def:transducer.hfst-ol.transducer.copy-windex-table-fn]
> TransducerTable<TransitionWIndex> Transducer::copy_windex_table()

> [spec:hfst:sem:transducer.hfst-ol.transducer.copy-windex-table-fn]
> Builds and returns a fresh TransducerTable<TransitionWIndex> copying this
> transducer's weighted index table. First, if header->probe_flag(Weighted) is
> FALSE, HFST_THROW(TransducerHasWrongTypeException). Otherwise create empty
> `another`, loop i from 0 to header->index_table_size()-1, and append
> TransitionWIndex(get_index_input(i), get_index_target(i)) — a new weighted index
> entry built from this transducer's index input symbol and target at i. Returns
> `another` by value.

> [spec:hfst:def:transducer.hfst-ol.transducer.display-fn]
> void Transducer::display() const

> [spec:hfst:sem:transducer.hfst-ol.transducer.display-fn]
> Prints the entire transducer to std::cout for debugging. Writes a banner line
> "-----Displaying optimized-lookup transducer------" (std::endl), then calls
> header->display(), alphabet->display(), and tables->display(), then writes a
> trailing line of dashes "-------------------------------------------------"
> (std::endl). Const; side effect is stdout output only.

> [spec:hfst:def:transducer.hfst-ol.transducer.final-index-fn]
> bool final_index(TransitionTableIndex i) const

> [spec:hfst:sem:transducer.hfst-ol.transducer.final-index-fn]
> Returns whether the state addressed by index `i` is final. If `i` addresses the
> transition table (indexes_transition_table(i), i.e. i >=
> TRANSITION_TARGET_TABLE_START), return tables->get_transition_finality(i)
> (note: passed as-is, not offset-subtracted). Otherwise (`i` addresses the index
> table) return tables->get_index_finality(i). Const.

> [spec:hfst:def:transducer.hfst-ol.transducer.final-weight-fn]
> Weight Transducer::final_weight(const TransitionTableIndex i) const

> [spec:hfst:sem:transducer.hfst-ol.transducer.final-weight-fn]
> Returns the final weight of the state addressed by index `i`. If i >=
> TRANSITION_TARGET_TABLE_START (addresses the transition table), return
> get_transition(i - TRANSITION_TARGET_TABLE_START).get_weight() (the weight of
> the final transition entry). Otherwise (addresses the index table) return
> get_index(i).final_weight() (the index entry's stored final weight). Const.

> [spec:hfst:def:transducer.hfst-ol.transducer.find-index-fn]
> void Transducer::find_index(SymbolNumber input,

> [spec:hfst:sem:transducer.hfst-ol.transducer.find-index-fn]
> Looks up `input` in the index table at base `i` and, if present, descends into
> the corresponding transition run. If tables->get_index_input(i+input) == input
> (the index slot at offset i+input matches the input symbol), call
> find_transitions(input, input_pos, output_pos, tables->get_index_target(i+input)
> - TRANSITION_TARGET_TABLE_START) (translating the absolute index target into a
> transition-table-relative index) and set found_transition = true. If the slot
> does not match, do nothing. Mutates found_transition (and, via find_transitions,
> the output tape, current_weight and lookup_paths). No return value.

> [spec:hfst:def:transducer.hfst-ol.transducer.find-loop-epsilon-indices-fn]
> void find_loop_epsilon_indices(unsigned int input_pos,

> [spec:hfst:sem:transducer.hfst-ol.transducer.find-loop-epsilon-indices-fn]
> (Definition in find_epsilon_loops.cc.) Index-table analogue of
> find_loop_epsilon_transitions, used in epsilon-cycle detection. If
> tables->get_index_input(i) == 0 (the index slot at i is an epsilon index), call
> find_loop_epsilon_transitions(input_pos, tables->get_index_target(i) -
> TRANSITION_TARGET_TABLE_START) (descending into the transition run, with the
> absolute target translated to a transition-table-relative index) and set
> found_transition = true. Otherwise do nothing. May propagate a thrown bool true
> from the descent (loop detected). No return value.

> [spec:hfst:def:transducer.hfst-ol.transducer.find-loop-epsilon-transitions-fn]
> void find_loop_epsilon_transitions(unsigned int input_pos,

> [spec:hfst:sem:transducer.hfst-ol.transducer.find-loop-epsilon-transitions-fn]
> (Definition in find_epsilon_loops.cc.) Walks the epsilon and flag-diacritic arcs
> starting at transition index `i`, recursing via find_loop, and detects non-
> progressing epsilon loops by throwing bool true. Snapshots flags =
> flag_state.get_values(). Loops forever: compute target =
> tables->get_transition_target(i) and epsilon_reachable =
> TraversalState(target, flags). If the transition input at i is 0 (epsilon): if
> traversal_states already contains epsilon_reachable (count == 1), throw true (a
> loop). Otherwise insert epsilon_reachable into traversal_states, call
> find_loop(input_pos, target), erase epsilon_reachable, set found_transition =
> true, and ++i. Else if the input is a flag diacritic
> (alphabet->is_flag_diacritic): attempt flag_state.apply_operation(
> *alphabet->get_operation(input)); if it succeeds, do the same loop-detection /
> insert / find_loop(input_pos, target) / erase as the epsilon case; then in all
> flag cases restore flag_state via flag_state.assign_values(flags) and ++i. Else
> (input is neither epsilon nor flag), return. Mutates flag_state, traversal_states,
> found_transition; may throw bool true. No return value.

> [spec:hfst:def:transducer.hfst-ol.transducer.find-loop-fn]
> void find_loop(unsigned int input_pos,

> [spec:hfst:sem:transducer.hfst-ol.transducer.find-loop-fn]
> (Body defined in find_epsilon_loops.cc.) Recursive traversal that detects
> non-progressing epsilon/flag loops while consuming the input tape. Sets
> found_transition = false. If indexes_transition_table(i) is true: subtract
> TRANSITION_TARGET_TABLE_START from i, then call
> find_loop_epsilon_transitions(input_pos, i+1). If input_tape[input_pos] ==
> NO_SYMBOL_NUMBER (input ended), return. Otherwise input = input_tape[input_pos],
> ++input_pos, call find_loop_transitions(input, input_pos, i+1); then if a
> default symbol exists and !found_transition, call find_loop_transitions with
> the default symbol. Else (indexes the index table): call
> find_loop_epsilon_indices(input_pos, i+1); if input ended, return; else read
> input, ++input_pos, call find_loop_index(input, input_pos, i+1), and if a
> default symbol exists and !found_transition, call find_loop_index with the
> default symbol. Mutates found_transition, traversal_states, flag_state; may
> propagate `throw true` from the epsilon-transition helper (signalling a loop /
> infinite ambiguity). No return value.

> [spec:hfst:def:transducer.hfst-ol.transducer.find-loop-index-fn]
> void find_loop_index(SymbolNumber input,

> [spec:hfst:sem:transducer.hfst-ol.transducer.find-loop-index-fn]
> (Body defined in find_epsilon_loops.cc.) Index-table counterpart of
> find_loop_transitions. If tables->get_index_input(i+input) == input (the index
> entry at offset i+input matches the input symbol), call
> find_loop_transitions(input, input_pos, tables->get_index_target(i+input) -
> TRANSITION_TARGET_TABLE_START) to descend into the matching transition run, then
> set found_transition = true. Otherwise do nothing. No return value.

> [spec:hfst:def:transducer.hfst-ol.transducer.find-loop-transitions-fn]
> void find_loop_transitions(SymbolNumber input,

> [spec:hfst:sem:transducer.hfst-ol.transducer.find-loop-transitions-fn]
> (Body defined in find_epsilon_loops.cc.) Walks the run of transitions starting
> at index i looking for ones whose input matches `input`. Loops while
> tables->get_transition_input(i) != NO_SYMBOL_NUMBER: if that input == input,
> clear traversal_states (we're consuming a real input symbol, so we won't find an
> epsilon/flag loop), call find_loop(input_pos, tables->get_transition_target(i))
> to recurse from the target, set found_transition = true, and ++i to continue.
> Otherwise (first non-matching input) return immediately (transitions for a given
> input are contiguous). Mutates traversal_states, found_transition; may propagate
> `throw true`. No return value.

> [spec:hfst:def:transducer.hfst-ol.transducer.find-transitions-fn]
> void Transducer::find_transitions(SymbolNumber input,

> [spec:hfst:sem:transducer.hfst-ol.transducer.find-transitions-fn]
> Walks the contiguous run of transitions starting at index i, taking each one
> whose input equals `input` and recursing via get_analyses. Loops while
> tables->get_transition_input(i) != NO_SYMBOL_NUMBER: if the transition input ==
> input: save old_weight = current_weight; clear traversal_states (a real input
> symbol is being consumed, no epsilon/flag loop possible); read output =
> tables->get_transition_output(i); if alphabet->is_meta_arc(output) (default /
> identity / unknown), instead set output = input_tape[input_pos - 1] (echo the
> actual input symbol just consumed); write the pair (input, output) to
> output_tape at output_pos; add tables->get_weight(i) to current_weight; call
> get_analyses(input_pos, output_pos + 1, tables->get_transition_target(i));
> restore current_weight = old_weight; set found_transition = true. If the
> transition input does not match `input`, return immediately. After a matching
> transition, ++i to continue. Mutates current_weight, traversal_states,
> output_tape, found_transition. No return value.

> [spec:hfst:def:transducer.hfst-ol.transducer.get-analyses-fn]
> void Transducer::get_analyses(unsigned int input_pos,

> [spec:hfst:sem:transducer.hfst-ol.transducer.get-analyses-fn]
> The core recursive lookup traversal over (input_pos, output_pos, table index i).
> First sets found_transition = false. Early returns (no recursion) if:
> recursion_depth_left == 0; or max_lookups >= 0 and lookup_paths->size() >=
> max_lookups (enough results); or max_time > 0 and the elapsed wall clock since
> start_clock exceeds max_time. Otherwise --recursion_depth_left, then branches on
> whether i indexes the transition table or the index table.
> Transition-table branch (indexes_transition_table(i)): subtract
> TRANSITION_TARGET_TABLE_START from i. If input_tape[input_pos] == NO_SYMBOL_NUMBER
> (input exhausted) and we still have result room, write a (NO_SYMBOL_NUMBER,
> NO_SYMBOL_NUMBER) terminator pair at output_pos and, if
> tables->get_transition_finality(i), add tables->get_weight(i) to current_weight,
> call note_analysis(), and restore current_weight. Then always call
> try_epsilon_transitions(input_pos, output_pos, i+1). If input is exhausted,
> ++recursion_depth_left and return. Else read input = input_tape[input_pos],
> ++input_pos: if input < alphabet->get_orig_symbol_count() call
> find_transitions(input, ...); otherwise (an OTHER symbol) call find_transitions
> for the identity symbol (if defined) and for the unknown symbol (if defined). In
> all cases, if a default symbol is defined and !found_transition, call
> find_transitions for the default symbol.
> Index-table branch: symmetric, using get_index_finality / get_final_weight,
> try_epsilon_indices, and find_index instead of the transition-table accessors.
> After either branch finishes, write a (NO_SYMBOL_NUMBER, NO_SYMBOL_NUMBER) pair
> at output_pos and ++recursion_depth_left. Mutates output_tape, current_weight,
> recursion_depth_left, found_transition, lookup_paths (via note_analysis),
> traversal_states. No return value.

> [spec:hfst:def:transducer.hfst-ol.transducer.get-string-symbol-map-fn]
> StringSymbolMap get_string_symbol_map(void) const

> [spec:hfst:sem:transducer.hfst-ol.transducer.get-string-symbol-map-fn]
> Inline forwarding accessor: returns alphabet->build_string_symbol_map(), a
> StringSymbolMap from each symbol string to its symbol number. Const.

> [spec:hfst:def:transducer.hfst-ol.transducer.get-transitions-from-state-fn]
> TransitionTableIndexSet Transducer::get_transitions_from_state(

> [spec:hfst:sem:transducer.hfst-ol.transducer.get-transitions-from-state-fn]
> Returns a TransitionTableIndexSet of all transition-table indices reachable as
> outgoing arcs from the state denoted by `state_index`. Builds an empty set
> `transitions`. If indexes_transition_index_table(state_index) (the state lives in
> the index table): for each symbol from 0 to header->symbol_count()-1:
> - If alphabet->is_like_epsilon(symbol) (epsilon or a flag-like symbol): look at
>   get_index(state_index+1); if it does not match input 0, continue to next
>   symbol; otherwise walk transition_i = that index's target forward, inserting
>   any transition whose input matches `symbol`, and breaking out of the inner
>   while only when a transition input is neither 0 nor like-epsilon (so epsilons /
>   other flags between matches don't stop the scan); otherwise ++transition_i.
> - Else (ordinary symbol): consult test_transition_index =
>   get_index(state_index+1+symbol); if it matches `symbol`, walk transition_i from
>   its target, inserting each transition that matches
>   test_transition_index.get_input_symbol() and breaking at the first that does
>   not, ++transition_i each step.
> Else (state_index indexes the transition table directly): the entry at
> state_index must be a boundary with both input and output == NO_SYMBOL_NUMBER;
> if not, `throw;` (rethrow / abort). Then from transition_i = state_index+1, walk
> forward inserting every transition whose input != NO_SYMBOL_NUMBER and stop at
> the first input == NO_SYMBOL_NUMBER. Returns `transitions`. Const.

> [spec:hfst:def:transducer.hfst-ol.transducer.get-unknown-symbol-fn]
> SymbolNumber get_unknown_symbol(void) const

> [spec:hfst:sem:transducer.hfst-ol.transducer.get-unknown-symbol-fn]
> Inline forwarding getter: returns alphabet->get_unknown_symbol() (the OTHER /
> @_UNKNOWN_SYMBOL_@ symbol number, or NO_SYMBOL_NUMBER if none). Const.

> [spec:hfst:def:transducer.hfst-ol.transducer.has-epsilons-or-flags-fn]
> bool Transducer::has_epsilons_or_flags(const TransitionTableIndex i)

> [spec:hfst:sem:transducer.hfst-ol.transducer.has-epsilons-or-flags-fn]
> Returns true iff there are epsilon or flag-diacritic arcs at table position `i`.
> If i >= TRANSITION_TARGET_TABLE_START (transition table): let t =
> get_transition(i - TRANSITION_TARGET_TABLE_START); return true iff its input
> symbol == 0 (epsilon) OR is_flag(that input symbol). Otherwise (index table):
> return true iff get_index(i).get_input_symbol() == 0. Non-const (is_flag is a
> non-const member).

> [spec:hfst:def:transducer.hfst-ol.transducer.has-transitions-fn]
> bool Transducer::has_transitions(const TransitionTableIndex i,

> [spec:hfst:sem:transducer.hfst-ol.transducer.has-transitions-fn]
> Returns true iff there is at least one arc on `symbol` at table position `i`. If
> i >= TRANSITION_TARGET_TABLE_START (transition table): return
> get_transition(i - TRANSITION_TARGET_TABLE_START).get_input_symbol() == symbol.
> Otherwise (index table): return get_index(i+symbol).get_input_symbol() == symbol
> (the index slot is offset by the symbol number). Const.

> [spec:hfst:def:transducer.hfst-ol.transducer.include-symbol-in-alphabet-fn]
> void Transducer::include_symbol_in_alphabet(const std::string & sym)

> [spec:hfst:sem:transducer.hfst-ol.transducer.include-symbol-in-alphabet-fn]
> Ensures the string `sym` is present in the alphabet and encoder. First key =
> alphabet->symbol_from_string(sym); if it is already present (key !=
> NO_SYMBOL_NUMBER), return immediately. Otherwise assign key =
> alphabet->get_symbol_table().size() (the next index), call
> alphabet->add_symbol(sym) to append it, then build a heap C-string copy of sym
> (new char[sym.size()+1] + strcpy), call encoder->read_input_symbol(that cstr,
> key) to register it for tokenization, and delete[] the temporary. No return
> value. Mutates the alphabet and encoder.

> [spec:hfst:def:transducer.hfst-ol.transducer.initialize-input-fn]
> bool Transducer::initialize_input(const char * input)

> [spec:hfst:sem:transducer.hfst-ol.transducer.initialize-input-fn]
> Tokenizes the NUL-terminated C-string `input` onto this transducer's input_tape,
> dynamically extending the alphabet with previously-unseen utf-8 symbols.
> Sets up a char* cursor over input and index i = 0. Loop while the current byte
> is not '\0': save original_input_loc = cursor; k = encoder->find_key(&cursor)
> (which advances the cursor over a matched symbol). If k == NO_SYMBOL_NUMBER
> (untokenizable): reset cursor to original_input_loc; bytes_to_tokenize =
> nByte_utf8(first byte). If that is 0 (invalid lead byte), return false. Otherwise
> allocate new_symbol = new char[bytes_to_tokenize+1], memcpy that many bytes plus
> a NUL terminator, advance the cursor by bytes_to_tokenize, call
> alphabet->add_symbol(new_symbol), set k = (symbol_table.size() - 1) as the new
> symbol number, register it via encoder->read_input_symbol(new_symbol, k), and
> delete[] new_symbol. Then write k onto input_tape at index i via
> input_tape.write(i, k) and ++i. After the loop, write a NO_SYMBOL_NUMBER
> terminator at input_tape[i] and return true. Mutates input_tape, alphabet,
> encoder.

> [spec:hfst:def:transducer.hfst-ol.transducer.is-flag-fn]
> bool is_flag(const SymbolNumber symbol)

> [spec:hfst:sem:transducer.hfst-ol.transducer.is-flag-fn]
> Inline forwarding accessor: returns alphabet->is_flag_diacritic(symbol), i.e.
> true iff `symbol` is a flag diacritic. Non-const member.

> [spec:hfst:def:transducer.hfst-ol.transducer.is-infinitely-ambiguous-fn]
> bool is_infinitely_ambiguous(void) const

> [spec:hfst:sem:transducer.hfst-ol.transducer.is-infinitely-ambiguous-fn]
> Inline accessor: returns header->probe_flag(Has_input_epsilon_cycles), i.e.
> whether the transducer's header marks it as having input-epsilon cycles (and is
> therefore potentially infinitely ambiguous regardless of input). Const.

> [spec:hfst:def:transducer.hfst-ol.transducer.is-lookup-infinitely-ambiguous-fn]
> bool Transducer::is_lookup_infinitely_ambiguous(const std::string & s)

> [spec:hfst:sem:transducer.hfst-ol.transducer.is-lookup-infinitely-ambiguous-fn]
> Tests whether looking up the specific string `s` would loop forever. Calls
> initialize_input(s.c_str()); if that returns false (tokenization failed), return
> false. Clears traversal_states. Then calls find_loop(0, 0) inside a try block:
> find_loop throws `bool true` if it detects a non-progressing epsilon/flag loop.
> If a bool e is caught, reset current_weight = 0.0, restore flag_state =
> alphabet->get_fd_table(), and return e (true). If find_loop completes without
> throwing, return false. Mutates input_tape, traversal_states, current_weight,
> flag_state.

> [spec:hfst:def:transducer.hfst-ol.transducer.is-weighted-fn]
> bool is_weighted(void)

> [spec:hfst:sem:transducer.hfst-ol.transducer.is-weighted-fn]
> Inline accessor: returns header->probe_flag(Weighted), i.e. whether the
> transducer's header marks it as weighted. Non-const member.

> [spec:hfst:def:transducer.hfst-ol.transducer.load-tables-fn]
> void Transducer::load_tables(std::istream& is)

> [spec:hfst:sem:transducer.hfst-ol.transducer.load-tables-fn]
> Constructs the `tables` member by reading the index and transition tables from
> input stream `is`. If header->probe_flag(Weighted), allocate new
> TransducerTables<TransitionWIndex,TransitionW>(is, header->index_table_size(),
> header->target_table_size()); otherwise allocate new
> TransducerTables<TransitionIndex,Transition>(is, ...) with the same sizes. After
> reading, if the stream is in a failed state (!is),
> HFST_THROW(TransducerHasWrongTypeException). Mutates `tables` and the stream
> cursor. No return value.

> [spec:hfst:def:transducer.hfst-ol.transducer.lookup-fd-fn]
> HfstOneLevelPaths * Transducer::lookup_fd(const char * s, ssize_t limit,

> [spec:hfst:sem:transducer.hfst-ol.transducer.lookup-fd-fn]
> (This is the `const char * s` overload; the std::string and StringVector
> overloads concatenate/convert and delegate here.) Looks up surface string `s`,
> honoring flag diacritics, and returns a newly-allocated HfstOneLevelPaths* of
> output strings. Sets max_lookups = limit and max_time = 0.0; if time_cutoff >
> 0.0, set max_time = time_cutoff and start_clock = clock(). Allocates results =
> new HfstOneLevelPaths. If !initialize_input(s) (tokenization failed), returns the
> empty results immediately. Otherwise allocates lookup_paths = new
> HfstTwoLevelPaths, clears traversal_states, and calls get_analyses(0, 0, 0) to
> populate lookup_paths. Then for each two-level path in lookup_paths: build an
> HfstOneLevelPath whose first (weight) = path.first and whose second is the vector
> of the output (second) component of each StringPair in path.second, and insert it
> into results. Finally delete lookup_paths, set lookup_paths = NULL, and return
> results. Mutates max_lookups, max_time, start_clock, lookup_paths, input_tape,
> traversal_states.

> [spec:hfst:def:transducer.hfst-ol.transducer.lookup-fd-pairs-fn]
> HfstTwoLevelPaths * Transducer::lookup_fd_pairs(const char * s, ssize_t limit,

> [spec:hfst:sem:transducer.hfst-ol.transducer.lookup-fd-pairs-fn]
> (The `const char * s` overload; the std::string overload converts and delegates.)
> Like lookup_fd but returns the full input:output two-level paths. Sets
> max_lookups = limit and max_time = 0.0; if time_cutoff > 0.0, set max_time =
> time_cutoff and start_clock = clock(). Allocates results = new HfstTwoLevelPaths
> and points lookup_paths directly at results. If !initialize_input(s), sets
> lookup_paths = NULL and returns the empty results. Otherwise clears
> traversal_states, calls get_analyses(0, 0, 0) (which inserts directly into
> results via lookup_paths), then sets lookup_paths = NULL and returns results.
> Mutates max_lookups, max_time, start_clock, lookup_paths, input_tape,
> traversal_states. Caller owns the returned pointer.

> [spec:hfst:def:transducer.hfst-ol.transducer.next-e-fn]
> TransitionTableIndex next_e(const TransitionTableIndex i) const

> [spec:hfst:sem:transducer.hfst-ol.transducer.next-e-fn]
> Declaration only: `TransitionTableIndex next_e(const TransitionTableIndex i)
> const` is declared in transducer.h but has no definition anywhere in the
> codebase (and the only other occurrence is inside a commented-out class block).
> It is effectively dead/unused; the port may omit it. No body to describe.

> [spec:hfst:def:transducer.hfst-ol.transducer.next-fn]
> TransitionTableIndex Transducer::next(const TransitionTableIndex i,

> [spec:hfst:sem:transducer.hfst-ol.transducer.next-fn]
> Given table position `i` and an input `symbol`, returns the transition-table
> index at which the run of arcs on `symbol` begins (as a transition-table-relative
> index). If i >= TRANSITION_TARGET_TABLE_START (already in the transition table):
> return i - TRANSITION_TARGET_TABLE_START + 1 (skip past the current boundary
> entry). Otherwise (index table): return get_index(i+1+symbol).get_target() -
> TRANSITION_TARGET_TABLE_START. Const.

> [spec:hfst:def:transducer.hfst-ol.transducer.note-analysis-fn]
> void Transducer::note_analysis(void)

> [spec:hfst:sem:transducer.hfst-ol.transducer.note-analysis-fn]
> Records the current output_tape contents as a completed analysis in
> lookup_paths. Builds an HfstTwoLevelPath `result`. Iterates the output_tape from
> begin() while the entry's `output` field != NO_SYMBOL_NUMBER, pushing onto
> result.second a StringPair(alphabet->string_from_symbol(it->input),
> alphabet->string_from_symbol(it->output)) for each pair. Sets result.first =
> current_weight, then lookup_paths->insert(result). Reads output_tape,
> current_weight, alphabet; mutates lookup_paths. No return value. (Note the loop
> stops at the first NO_SYMBOL_NUMBER terminator written by get_analyses.)

> [spec:hfst:def:transducer.hfst-ol.transducer.take-epsilons-and-flags-fn]
> STransition Transducer::take_epsilons_and_flags(const TransitionTableIndex i)

> [spec:hfst:sem:transducer.hfst-ol.transducer.take-epsilons-and-flags-fn]
> Returns the STransition for the transition at index `i` if it is an epsilon or
> flag-diacritic arc, else a sentinel. If get_transition(i).get_input_symbol() != 0
> AND !is_flag(that input symbol) (neither epsilon nor flag), return STransition(0,
> NO_SYMBOL_NUMBER) (sentinel meaning "stop"). Otherwise return STransition(
> get_transition(i).get_target(), get_transition(i).get_output_symbol(),
> get_transition(i).get_weight()). Non-const (is_flag is non-const).

> [spec:hfst:def:transducer.hfst-ol.transducer.take-epsilons-fn]
> STransition Transducer::take_epsilons(const TransitionTableIndex i) const

> [spec:hfst:sem:transducer.hfst-ol.transducer.take-epsilons-fn]
> Returns the STransition for the transition at index `i` only if it is a true
> epsilon arc (input symbol 0). If get_transition(i).get_input_symbol() != 0,
> return STransition(0, NO_SYMBOL_NUMBER) (sentinel meaning "stop"). Otherwise
> return STransition(get_transition(i).get_target(),
> get_transition(i).get_output_symbol(), get_transition(i).get_weight()). Const.

> [spec:hfst:def:transducer.hfst-ol.transducer.take-non-epsilons-fn]
> STransition Transducer::take_non_epsilons(const TransitionTableIndex i,

> [spec:hfst:sem:transducer.hfst-ol.transducer.take-non-epsilons-fn]
> Returns the STransition for the transition at index `i` only if its input symbol
> equals `symbol`. If get_transition(i).get_input_symbol() != symbol, return
> STransition(0, NO_SYMBOL_NUMBER) (sentinel meaning "stop"). Otherwise return
> STransition(get_transition(i).get_target(),
> get_transition(i).get_output_symbol(), get_transition(i).get_weight()). Const.

> [spec:hfst:def:transducer.hfst-ol.transducer.transducer-fn]
> Transducer::Transducer(bool weighted)

> [spec:hfst:sem:transducer.hfst-ol.transducer.transducer-fn]
> Constructs an empty Transducer of the requested weightedness via the member
> initializer list: header = new TransducerHeader(weighted); alphabet = new
> TransducerAlphabet() (empty); current_weight = 0.0; lookup_paths = NULL; encoder
> = new Encoder(alphabet->get_symbol_table(), header->input_symbol_count());
> input_tape and output_tape default-constructed; flag_state =
> alphabet->get_fd_table(); found_transition = false; max_lookups = -1;
> recursion_depth_left = MAX_RECURSION_DEPTH. In the body, allocate `tables`: if
> weighted, new TransducerTables<TransitionWIndex,TransitionW>(); else new
> TransducerTables<TransitionIndex,Transition>(). Allocates header, alphabet,
> encoder, tables on the heap (owned, freed by the destructor).

> [spec:hfst:def:transducer.hfst-ol.transducer.try-epsilon-indices-fn]
> void Transducer::try_epsilon_indices(unsigned int input_pos,

> [spec:hfst:sem:transducer.hfst-ol.transducer.try-epsilon-indices-fn]
> Index-table entry point for epsilon traversal. If tables->get_index_input(i) ==
> 0 (the index entry has an epsilon arc), call try_epsilon_transitions(input_pos,
> output_pos, tables->get_index_target(i) - TRANSITION_TARGET_TABLE_START) to
> follow the epsilon/flag transitions at that target, then set found_transition =
> true. Otherwise do nothing. Mutates found_transition (and whatever
> try_epsilon_transitions mutates). No return value.

> [spec:hfst:def:transducer.hfst-ol.transducer.try-epsilon-transitions-fn]
> void Transducer::try_epsilon_transitions(unsigned int input_pos,

> [spec:hfst:sem:transducer.hfst-ol.transducer.try-epsilon-transitions-fn]
> Follows the contiguous run of epsilon and flag-diacritic transitions starting at
> index i, recursing via get_analyses for each. Infinite loop reading the
> transition's input, output, target, weight and saving old_weight =
> current_weight:
> - If input == 0 (epsilon): write the (input, output) pair to output_tape at
>   output_pos, add weight to current_weight, call get_analyses(input_pos,
>   output_pos + 1, target), set found_transition = true, restore current_weight =
>   old_weight, ++i, continue.
> - Else if alphabet->is_flag_diacritic(input): snapshot flags =
>   flag_state.get_values(); if flag_state.apply_operation(
>   *alphabet->get_operation(input)) succeeds (flag allowed): build TraversalState
>   flag_reachable(target, flags); if traversal_states already contains it (loop
>   guard), restore flag_state.assign_values(flags), ++i, continue; otherwise
>   insert flag_reachable, write (input, output) at output_pos, add weight, call
>   get_analyses(input_pos, output_pos + 1, target), set found_transition = true,
>   restore current_weight, erase flag_reachable. In all flag cases restore
>   flag_state.assign_values(flags) and ++i.
> - Else (neither epsilon nor flag): return (end of the epsilon run).
> Mutates output_tape, current_weight, flag_state, traversal_states,
> found_transition. No return value.

> [spec:hfst:def:transducer.hfst-ol.transducer.write-fn]
> void Transducer::write(std::ostream& os) const

> [spec:hfst:sem:transducer.hfst-ol.transducer.write-fn]
> Serializes the whole transducer to output stream `os` in binary. Calls
> header->write(os), then alphabet->write(os). Then for i from 0 to
> header->index_table_size()-1, writes each index entry via
> tables->get_index(i).write(os, header->probe_flag(Weighted)). Then for i from 0
> to header->target_table_size()-1, writes each transition via
> tables->get_transition(i).write(os, header->probe_flag(Weighted)). The Weighted
> flag tells each entry's write() whether to emit weight fields. Const; side
> effect is the byte output. No return value.

> PORT DIVERGENCE (upstream bug deliberately fixed): upstream writes the header
> it happens to be holding. No constructor ever computed the property flags and
> `set_flag` has no caller in either tree, so every file written from an
> in-memory transducer described itself with constructor defaults — cyclic and
> the epsilon flags false whatever the graph, deterministic / input_deterministic
> / minimized true whatever the graph. `hfst-fst2strings` guards path extraction
> on the cyclic flag, so an `.hfstol` of `[a b]*` enumerated its infinite
> language until the disk filled. This port derives the flags a walk can decide
> from the graph immediately before writing (one walk per file), and claims
> nothing for the three no single walk can establish. "Input epsilon" is read as
> the lookup engine reads it — an arc that advances no input tape position,
> which includes a flag diacritic — so the cycle and transition flags cannot
> contradict each other.

> [spec:hfst:def:transducer.hfst-ol.transition]
> class Transition {
>   SymbolNumber input_symbol;
>   SymbolNumber output_symbol;
>   TransitionTableIndex target_index;
>   static const size_t size = 2 * sizeof(SymbolNumber) + sizeof(TransitionTableIndex);
> }

> [spec:hfst:def:transducer.hfst-ol.transition-index]
> class TransitionIndex {
>   SymbolNumber input_symbol;
>   TransitionTableIndex first_transition_index;
>   static const size_t size = sizeof(SymbolNumber) + sizeof(TransitionTableIndex);
> }

> [spec:hfst:def:transducer.hfst-ol.transition-index.create-final-fn]
> static TransitionIndex create_final()

> [spec:hfst:sem:transducer.hfst-ol.transition-index.create-final-fn]
> Static factory: returns a TransitionIndex value representing a final (accepting)
> state index, constructed as TransitionIndex(NO_SYMBOL_NUMBER, 1) — i.e.
> input_symbol = NO_SYMBOL_NUMBER and first_transition_index = 1. Pure; no side
> effects.

> [spec:hfst:def:transducer.hfst-ol.transition-index.display-fn]
> void TransitionIndex::display() const

> [spec:hfst:sem:transducer.hfst-ol.transition-index.display-fn]
> Prints this TransitionIndex to std::cout for debugging. Writes a single line:
> "input_symbol: " << input_symbol << ", target: " << first_transition_index,
> followed by " (final)" if final() returns true (else nothing), then std::endl.
> Const; side effect is stdout output only.

> [spec:hfst:def:transducer.hfst-ol.transition-index.final-fn]
> bool TransitionIndex::final() const

> [spec:hfst:sem:transducer.hfst-ol.transition-index.final-fn]
> Virtual. Returns true iff this index entry marks a final (accepting) state, i.e.
> input_symbol == NO_SYMBOL_NUMBER AND first_transition_index != NO_TABLE_INDEX.
> Const, pure.

> [spec:hfst:def:transducer.hfst-ol.transition-index.final-weight-fn]
> virtual Weight final_weight(void) const

> [spec:hfst:sem:transducer.hfst-ol.transition-index.final-weight-fn]
> Virtual. Returns the final weight of this index entry. The base (unweighted)
> TransitionIndex implementation always returns 0.0. (The weighted subclass
> TransitionWIndex overrides this to reinterpret first_transition_index as a
> Weight.) Const.

> [spec:hfst:def:transducer.hfst-ol.transition-index.get-input-symbol-fn]
> SymbolNumber get_input_symbol(void) const

> [spec:hfst:sem:transducer.hfst-ol.transition-index.get-input-symbol-fn]
> Getter: returns the member `input_symbol` (the input symbol this index entry is
> keyed on, or NO_SYMBOL_NUMBER for a final/empty entry). Const, pure.

> [spec:hfst:def:transducer.hfst-ol.transition-index.get-target-fn]
> TransitionTableIndex get_target(void) const

> [spec:hfst:sem:transducer.hfst-ol.transition-index.get-target-fn]
> Getter: returns the member `first_transition_index` (the target — the index of
> the first transition in the transition table for this state, or NO_TABLE_INDEX).
> Const, pure.

> [spec:hfst:def:transducer.hfst-ol.transition-index.matches-fn]
> bool TransitionIndex::matches(SymbolNumber s) const

> [spec:hfst:sem:transducer.hfst-ol.transition-index.matches-fn]
> Returns true iff this index entry's input symbol matches `s`, i.e.
> input_symbol != NO_SYMBOL_NUMBER AND input_symbol == s. Const, pure. (A
> NO_SYMBOL_NUMBER input_symbol never matches anything.)

> [spec:hfst:def:transducer.hfst-ol.transition-index.transition-index-fn]
> TransitionIndex(std::istream& is)

> [spec:hfst:sem:transducer.hfst-ol.transition-index.transition-index-fn]
> Constructs a TransitionIndex by reading it from input stream `is` in binary.
> Member initializer list sets input_symbol = NO_SYMBOL_NUMBER and
> first_transition_index = 0. The body then does two raw reads in order:
> is.read(&input_symbol, sizeof(SymbolNumber)) then
> is.read(&first_transition_index, sizeof(TransitionTableIndex)) — both raw
> native-byte-order reads. Mutates the stream cursor by
> sizeof(SymbolNumber)+sizeof(TransitionTableIndex) bytes. No validation.

> [spec:hfst:def:transducer.hfst-ol.transition-index.write-fn]
> void write(std::ostream& os, bool weighted) const

> [spec:hfst:sem:transducer.hfst-ol.transition-index.write-fn]
> Serializes this TransitionIndex to output stream `os` in binary. First writes
> input_symbol via os.write(&input_symbol, sizeof(SymbolNumber)). Then writes the
> target field: if `weighted` is false AND this is a final entry (input_symbol ==
> NO_SYMBOL_NUMBER AND first_transition_index != NO_TABLE_INDEX), it writes the
> literal value 1 (a local unsigned int unweighted_final_index = 1) instead of
> first_transition_index, sizeof(first_transition_index) bytes — ensuring the
> correct unweighted final-index marker. Otherwise it writes first_transition_index
> itself, sizeof(first_transition_index) bytes. Const; side effect is byte output.

> [spec:hfst:def:transducer.hfst-ol.transition-number]
> typedef unsigned int TransitionNumber

> [spec:hfst:def:transducer.hfst-ol.transition-table-index]
> typedef unsigned int TransitionTableIndex

> [spec:hfst:def:transducer.hfst-ol.transition-table-index-set]
> typedef std::set<TransitionTableIndex> TransitionTableIndexSet

> [spec:hfst:def:transducer.hfst-ol.transition-w]
> class TransitionW : public Transition {
>   Weight transition_weight;
>   static const size_t size = 2 * sizeof(SymbolNumber) + sizeof(TransitionTableIndex) + sizeof(Weight);
> }

> [spec:hfst:def:transducer.hfst-ol.transition-w-index]
> class TransitionWIndex : public TransitionIndex

> [spec:hfst:def:transducer.hfst-ol.transition-w-index.create-final-fn]
> static TransitionWIndex create_final(Weight w)

> [spec:hfst:sem:transducer.hfst-ol.transition-w-index.create-final-fn]
> Static factory: builds a final TransitionWIndex that encodes the final weight `w`
> in its target field. Declares a union { TransitionTableIndex i; Weight w; }
> weight, sets weight.w = w, then returns
> TransitionWIndex(NO_SYMBOL_NUMBER, weight.i) — i.e. input_symbol =
> NO_SYMBOL_NUMBER and first_transition_index = the raw bit-reinterpretation of
> the float weight as a TransitionTableIndex. Pure; relies on
> sizeof(Weight)==sizeof(TransitionTableIndex) for the type-pun to round-trip.

> [spec:hfst:def:transducer.hfst-ol.transition-w-index.final-weight-fn]
> Weight TransitionWIndex::final_weight(void) const

> [spec:hfst:sem:transducer.hfst-ol.transition-w-index.final-weight-fn]
> Overrides TransitionIndex::final_weight. Returns the final weight stored in this
> weighted index entry by type-punning first_transition_index back into a Weight.
> Declares a union { TransitionTableIndex i; Weight w; } weight, sets weight.i =
> first_transition_index, and returns weight.w (the raw bits reinterpreted as a
> float). Const. Inverse of create_final(Weight).

> [spec:hfst:def:transducer.hfst-ol.transition-w-index.transition-w-index-fn]
> TransitionWIndex(SymbolNumber input,

> [spec:hfst:sem:transducer.hfst-ol.transition-w-index.transition-w-index-fn]
> Two-argument constructor for TransitionWIndex. Forwards both parameters to the
> base TransitionIndex(input, first_transition) constructor, setting input_symbol
> = input and first_transition_index = first_transition. Empty body; adds no
> additional state over the base class.

> [spec:hfst:def:transducer.hfst-ol.transition-w.display-fn]
> void TransitionW::display() const

> [spec:hfst:sem:transducer.hfst-ol.transition-w.display-fn]
> Prints this weighted Transition to std::cout for debugging. Writes a single line:
> "input_symbol: " << input_symbol << ", output_symbol: " << output_symbol <<
> ", target: " << target_index << ", weight: " << transition_weight, followed by
> " (final)" if final() returns true (else nothing), then std::endl. Const; side
> effect is stdout output only.

> [spec:hfst:def:transducer.hfst-ol.transition-w.get-weight-fn]
> Weight get_weight(void) const

> [spec:hfst:sem:transducer.hfst-ol.transition-w.get-weight-fn]
> Overrides Transition::get_weight. Returns the member `transition_weight` (the
> weight of this weighted transition). Const, pure.

> [spec:hfst:def:transducer.hfst-ol.transition-w.transition-w-fn]
> TransitionW(char * p)

> [spec:hfst:sem:transducer.hfst-ol.transition-w.transition-w-fn]
> Constructs a TransitionW by reading from a raw char buffer `p`. Forwards p to the
> base Transition(char* p) constructor, which reads input_symbol from offset 0,
> output_symbol from offset sizeof(SymbolNumber), and target_index from offset
> 2*sizeof(SymbolNumber). Then initializes transition_weight by reading a Weight
> from offset 2*sizeof(SymbolNumber)+sizeof(TransitionTableIndex) of p (i.e.
> *(Weight*)(p + that offset)). Empty body. Does not advance any cursor (raw
> pointer arithmetic into a fixed-layout record).

> [spec:hfst:def:transducer.hfst-ol.transition-w.write-fn]
> void write(std::ostream& os, bool weighted) const

> [spec:hfst:sem:transducer.hfst-ol.transition-w.write-fn]
> Serializes this weighted transition to output stream `os`. First calls
> Transition::write(os, false) to write input_symbol, output_symbol, and
> target_index (passing weighted=false so the base does not append any weight).
> Then, if `weighted` is true, writes the member transition_weight via
> os.write(&transition_weight, sizeof(transition_weight)) — a raw binary write.
> If `weighted` is false, the weight is omitted entirely. Const; side effect is
> byte output.

> [spec:hfst:def:transducer.hfst-ol.transition.display-fn]
> void Transition::display() const

> [spec:hfst:sem:transducer.hfst-ol.transition.display-fn]
> Prints this Transition to std::cout for debugging. Writes a single line:
> "input_symbol: " << input_symbol << ", output_symbol: " << output_symbol <<
> ", target: " << target_index, followed by " (final)" if final() returns true
> (else nothing), then std::endl. Const; side effect is stdout output only.

> [spec:hfst:def:transducer.hfst-ol.transition.final-fn]
> bool Transition::final() const

> [spec:hfst:sem:transducer.hfst-ol.transition.final-fn]
> Virtual. Returns true iff this transition marks a final (accepting) state, i.e.
> input_symbol == NO_SYMBOL_NUMBER AND output_symbol == NO_SYMBOL_NUMBER AND
> target_index == 1. Const, pure. (Note: the final marker for a Transition uses
> target_index == 1, distinct from TransitionIndex::final which tests
> first_transition_index != NO_TABLE_INDEX.)

> [spec:hfst:def:transducer.hfst-ol.transition.get-input-symbol-fn]
> SymbolNumber get_input_symbol(void) const

> [spec:hfst:sem:transducer.hfst-ol.transition.get-input-symbol-fn]
> Getter: returns the member `input_symbol` (the transition's input-side symbol,
> or NO_SYMBOL_NUMBER for a final/empty transition). Const, pure.

> [spec:hfst:def:transducer.hfst-ol.transition.get-output-symbol-fn]
> SymbolNumber get_output_symbol(void) const

> [spec:hfst:sem:transducer.hfst-ol.transition.get-output-symbol-fn]
> Getter: returns the member `output_symbol` (the transition's output-side symbol,
> or NO_SYMBOL_NUMBER for a final/empty transition). Const, pure.

> [spec:hfst:def:transducer.hfst-ol.transition.get-target-fn]
> TransitionTableIndex get_target(void) const

> [spec:hfst:sem:transducer.hfst-ol.transition.get-target-fn]
> Getter: returns the member `target_index` (the transition-table index of the
> target state this transition leads to). Const, pure.

> [spec:hfst:def:transducer.hfst-ol.transition.get-weight-fn]
> virtual Weight get_weight(void) const

> [spec:hfst:sem:transducer.hfst-ol.transition.get-weight-fn]
> Virtual. Returns the transition weight. The base (unweighted) Transition
> implementation always returns 0.0. (The weighted subclass TransitionW overrides
> this to return its transition_weight member.) Const.

> [spec:hfst:def:transducer.hfst-ol.transition.matches-fn]
> bool Transition::matches(SymbolNumber s) const

> [spec:hfst:sem:transducer.hfst-ol.transition.matches-fn]
> Returns true iff this transition's input symbol matches `s`, i.e.
> input_symbol != NO_SYMBOL_NUMBER AND input_symbol == s. Const, pure. (A
> NO_SYMBOL_NUMBER input_symbol never matches anything.)

> [spec:hfst:def:transducer.hfst-ol.transition.transition-fn]
> Transition(std::istream& is)

> [spec:hfst:sem:transducer.hfst-ol.transition.transition-fn]
> Constructs a Transition by reading it from input stream `is` in binary. Member
> initializer list sets input_symbol = NO_SYMBOL_NUMBER, output_symbol =
> NO_SYMBOL_NUMBER, target_index = 0. The body then does three raw reads in order:
> is.read(&input_symbol, sizeof(SymbolNumber)); is.read(&output_symbol,
> sizeof(SymbolNumber)); is.read(&target_index, sizeof(target_index)). Mutates the
> stream cursor by 2*sizeof(SymbolNumber)+sizeof(TransitionTableIndex) bytes. No
> validation; no weight is read (this is the unweighted Transition).

> [spec:hfst:def:transducer.hfst-ol.transition.write-fn]
> virtual void write(std::ostream& os, bool weighted) const

> [spec:hfst:sem:transducer.hfst-ol.transition.write-fn]
> Virtual. Serializes this Transition to output stream `os` in binary. Writes, in
> order: input_symbol via os.write(&input_symbol, sizeof(input_symbol));
> output_symbol via os.write(&output_symbol, sizeof(output_symbol)); target_index
> via os.write(&target_index, sizeof(target_index)). Then, if `weighted` is true,
> appends a weight of 0.0f using the formatted stream operator `os << 0.0f` (text
> formatting, NOT a raw binary write). Const; side effect is byte output.

> [spec:hfst:def:transducer.hfst-ol.traversal-state]
> struct TraversalState {
>   TransitionTableIndex index;
>   FlagDiacriticState flags;
> }

> [spec:hfst:def:transducer.hfst-ol.traversal-state.operator-fn]
> bool operator==(const TraversalState & rhs) const

> [spec:hfst:sem:transducer.hfst-ol.traversal-state.operator-fn]
> Equality comparison for TraversalState (used in epsilon-loop checking). Returns
> false immediately if this->index != rhs.index. Otherwise iterates i from 0 to
> this->flags.size()-1 and returns false on the first mismatch this->flags[i] !=
> rhs.flags[i]. If all compared, returns true. Const. NOTE: it iterates only over
> this->flags.size(); if rhs.flags is shorter it would read out of bounds, and if
> rhs.flags is longer its trailing entries are ignored (the code assumes equal-
> length flag vectors).

> [spec:hfst:def:transducer.hfst-ol.traversal-state.traversal-state-fn]
> TraversalState(TransitionTableIndex i, FlagDiacriticState f)

> [spec:hfst:sem:transducer.hfst-ol.traversal-state.traversal-state-fn]
> Constructor for TraversalState. Initializes member index = i and flags = f
> (a copy of the FlagDiacriticState vector) from the parameters. Empty body.

> [spec:hfst:def:transducer.hfst-ol.traversal-states]
> typedef std::set<TraversalState> TraversalStates

> [spec:hfst:def:transducer.hfst-ol.tree-node]
> class TreeNode {
>   SymbolNumberVector string;
>   unsigned int input_state;
>   TransitionTableIndex mutator_state;
>   TransitionTableIndex lexicon_state;
>   hfst::FdState<SymbolNumber> flag_state;
>   Weight weight;
> }

> [spec:hfst:def:transducer.hfst-ol.tree-node-queue]
> typedef std::deque<TreeNode> TreeNodeQueue

> [spec:hfst:def:transducer.hfst-ol.tree-node.increment-mutator-fn]
> void increment_mutator(void)

> [spec:hfst:sem:transducer.hfst-ol.tree-node.increment-mutator-fn]
> Declared member `void increment_mutator(void)` on TreeNode. NOTE: this method is
> only declared in the header and has no definition anywhere in the codebase (no
> implementation exists), so it is effectively dead/unused. A faithful port should
> omit it or provide an empty stub; there is no observable behavior to replicate.

> [spec:hfst:def:transducer.hfst-ol.tree-node.tree-node-fn]
> TreeNode(SymbolNumberVector prev_string,

> [spec:hfst:sem:transducer.hfst-ol.tree-node.tree-node-fn]
> Six-argument constructor for TreeNode. Initializes the members directly from the
> parameters via the member initializer list: string = prev_string (copy of the
> output-symbol vector built so far), input_state = i, mutator_state = mutator,
> lexicon_state = lexicon, flag_state = state (copy of the FdState), weight = w.
> Empty body. (A separate single-argument constructor builds a fresh start node
> with empty string, input_state/mutator_state/lexicon_state all 0, weight 0.0,
> and flag_state = the given start_state.)

> [spec:hfst:def:transducer.hfst-ol.tree-node.update-fn]
> TreeNode update(SymbolNumber next_symbol,

> [spec:hfst:sem:transducer.hfst-ol.tree-node.update-fn]
> Returns a new TreeNode advanced from this one, consuming input. (This is the
> five-argument overload: next_symbol, next_input, next_mutator, next_lexicon,
> weight.) Copies this->string into a local SymbolNumberVector str and
> push_back(next_symbol) onto it. Returns TreeNode(str, next_input, next_mutator,
> next_lexicon, this->flag_state, this->weight + weight) — i.e. the new node has
> the extended output string, input_state set to next_input, mutator_state set to
> next_mutator, lexicon_state set to next_lexicon, flag_state copied unchanged, and
> weight = this node's weight plus the passed-in increment. Does not mutate `this`.
> (A separate four-argument overload omits next_input and instead keeps
> this->input_state unchanged.)

> [spec:hfst:def:transducer.hfst-ol.tree-node.update-lexicon-fn]
> TreeNode update_lexicon(SymbolNumber next_symbol,

> [spec:hfst:sem:transducer.hfst-ol.tree-node.update-lexicon-fn]
> Returns a new TreeNode advanced only on the lexicon side, recording one output
> symbol. Copies this->string into a local SymbolNumberVector str and
> push_back(next_symbol) onto it. Returns TreeNode(str, this->input_state,
> this->mutator_state, next_lexicon, this->flag_state, this->weight + weight) —
> i.e. the new node keeps this node's input_state and mutator_state unchanged,
> sets lexicon_state to next_lexicon, copies flag_state unchanged, and sets
> weight = this node's weight plus the passed-in increment. Does not mutate `this`.

> [spec:hfst:def:transducer.hfst-ol.tree-node.update-mutator-fn]
> TreeNode update_mutator(SymbolNumber next_symbol,

> [spec:hfst:sem:transducer.hfst-ol.tree-node.update-mutator-fn]
> Returns a new TreeNode advanced only on the mutator side, recording one output
> symbol. Copies this->string into a local SymbolNumberVector str and
> push_back(next_symbol) onto it. Returns TreeNode(str, this->input_state,
> next_mutator, this->lexicon_state, this->flag_state, this->weight + weight) —
> i.e. the new node keeps this node's input_state and lexicon_state unchanged,
> sets mutator_state to next_mutator, copies flag_state unchanged, and sets
> weight = this node's weight plus the passed-in increment. Does not mutate `this`.

> [spec:hfst:def:transducer.hfst-ol.value-number]
> typedef short ValueNumber

> [spec:hfst:def:transducer.hfst-ol.weight]
> typedef float Weight

> [spec:hfst:def:transducer.hfst-ol.weighted-double-tape]
> struct WeightedDoubleTape: public DoubleTape {
>   Weight weight;
> }

> [spec:hfst:def:transducer.hfst-ol.weighted-double-tape.weighted-double-tape-fn]
> WeightedDoubleTape(DoubleTape dt, Weight w): DoubleTape(dt), weight(w)

> [spec:hfst:sem:transducer.hfst-ol.weighted-double-tape.weighted-double-tape-fn]
> Constructor for WeightedDoubleTape (a DoubleTape with an added Weight). Takes a
> DoubleTape `dt` (by value) and a Weight `w`. Copy-constructs the DoubleTape base
> subobject from `dt` and initializes the member `weight = w`. Empty body.

> [spec:hfst:def:transducer.main-fn]
> int main(int argc, char * argv[])

> [spec:hfst:sem:transducer.main-fn]
> The MAIN_TEST entry point (compiled only when MAIN_TEST is defined). A stub
> unit-test main: prints "Unit tests for <__FILE__>:" to std::cout (followed by
> std::endl), then prints "ok" (followed by std::endl), and returns 0. Performs
> no actual tests. Takes argc/argv but ignores both.

> [spec:hfst:def:transducer.ssize-t]
> typedef SSIZE_T ssize_t

