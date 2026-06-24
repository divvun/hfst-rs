# libhfst/src/implementations/optimized-lookup/ospell.cc

> [spec:hfst:def:ospell.hfst-ol.input-string.initialize-fn]
> bool InputString::initialize(const Encoder & encoder,

> [spec:hfst:sem:ospell.hfst-ol.input-string.initialize-fn]
> Tokenizes the NUL-terminated C string `input` into the member symbol
> vector `s` using `encoder`, returning true on success and false on
> failure. Steps:
> - Clear `s`. Set a local `k = NO_SYMBOL_NUMBER`. Maintain a pointer
>   `inpointer` that points at the current position in `input`.
> - Loop while the current character `**inpointer != '\0'`:
>   - Save `oldpointer = *inpointer`.
>   - Call `encoder.find_key(inpointer)` (const-cast away constness),
>     which returns the longest matching symbol number and advances
>     `*inpointer` past the consumed bytes; assign to `k`.
>   - If `k == NO_SYMBOL_NUMBER` (no alphabet tokenization): compute
>     `n = nByte_utf8(*oldpointer)`. If `n == 0` (cannot parse a utf-8
>     character), return false. Otherwise, if `other == NO_SYMBOL_NUMBER`
>     (no "other" symbol available), return false. Else advance the
>     pointer by `n` bytes (set `*inpointer = oldpointer + n`), push
>     `other` onto `s`, and continue the loop.
>   - Else (`k` valid): push `k` onto `s`.
> - After the loop (empty string yields empty `s`, no end marker), return
>   true.

> [spec:hfst:def:ospell.hfst-ol.n-byte-utf8-fn]
> int nByte_utf8(unsigned char c)

> [spec:hfst:sem:ospell.hfst-ol.n-byte-utf8-fn]
> Given an unsigned char `c` (a utf-8 lead byte), returns how many bytes
> the utf-8 character occupies, used to peel off a character to represent
> as OTHER. Branches by inspecting the high bits of `c`:
> - If `c <= 127` (ASCII, high bit clear): return 1.
> - Else if `(c & 0xF0) == 0xF0` (top four bits 1111): return 4.
> - Else if `(c & 0xE0) == 0xE0` (top three bits 111): return 3.
> - Else if `(c & 0xC0) == 0xC0` (top two bits 11): return 2.
> - Otherwise (a continuation byte 10xxxxxx, not a valid lead): return 0.

> [spec:hfst:def:ospell.hfst-ol.speller.build-alphabet-translator-fn]
> void Speller::build_alphabet_translator(void)

> [spec:hfst:sem:ospell.hfst-ol.speller.build-alphabet-translator-fn]
> Builds the member vector `alphabet_translator`, which maps each
> mutator (error model) symbol number to the corresponding lexicon
> symbol number. Steps:
> - Get `from = mutator->get_alphabet()` and `to = lexicon->get_alphabet()`.
> - Get `from_keys = from.get_symbol_table()` (mutator's symbol number ->
>   string), and `to_symbols = to.build_string_symbol_map()` (lexicon's
>   string -> symbol number).
> - Push 0 as the zeroth element (epsilon always maps to 0).
> - For each mutator symbol number `i` from 1 to `from_keys.size() - 1`:
>   - If `from.is_flag_diacritic(i)` is true OR `i == from.get_unknown_symbol()`
>     (the OTHER symbol): push `NO_SYMBOL_NUMBER` (no translation) and
>     continue.
>   - Otherwise, if `to_symbols` does not contain the string `from_keys[i]`
>     exactly once (count != 1): let `name = from_keys[i]`; if `name` is
>     non-empty, throw `AlphabetTranslationException(from_keys[i])`. (If the
>     string is empty, fall through without throwing.)
>   - Push `to_symbols[from_keys[i]]`, the lexicon symbol number for the
>     mutator symbol's string.

> [spec:hfst:def:ospell.hfst-ol.speller.check-fn]
> bool Speller::check(char * line)

> [spec:hfst:sem:ospell.hfst-ol.speller.check-fn]
> Determines whether the lexicon transducer accepts the input `line`,
> returning true if it does, false otherwise. Steps:
> - Call `init_input(line, lexicon->get_encoder(), NO_SYMBOL_NUMBER)`
>   (no "other"/unknown symbol). If it returns false (tokenization
>   failed), return false.
> - Construct a start `TreeNode` from `lexicon->get_fd_table()` and set
>   the member `queue` to contain exactly that one node.
> - While `queue` is non-empty:
>   - Let `front = queue.front()`. If `front.input_state == input.len()`
>     (all input consumed) AND `lexicon->final_index(front.lexicon_state)`
>     (lexicon in a final state), return true.
>   - Otherwise call `lexicon_epsilons()` then `lexicon_consume()` (each
>     expands `front` and pushes successor nodes onto the back of `queue`).
>   - Pop the front node.
> - If the queue empties without a match, return false.

> [spec:hfst:def:ospell.hfst-ol.speller.consume-input-fn]
> void Speller::consume_input(void)

> [spec:hfst:sem:ospell.hfst-ol.speller.consume-input-fn]
> Expands the front `queue` node by taking non-epsilon mutator
> transitions on the current input symbol, jointly with matching lexicon
> transitions, pushing successor nodes onto the back of `queue`. Steps:
> - Let `input_state = queue.front().input_state`. If `input_state >=
>   input.len()` OR the mutator has no transitions for
>   `(front.mutator_state + 1, input[input_state])`, return (not enough
>   input or no suitable transitions).
> - Let `next_m = mutator->next(front.mutator_state, input[input_state])`
>   and `mutator_i_s = mutator->take_non_epsilons(next_m, input[input_state])`.
> - While `mutator_i_s.symbol != NO_SYMBOL_NUMBER`:
>   - If `mutator_i_s.symbol == 0` (mutator output epsilon): push
>     `front.update(0, input_state + 1, mutator_i_s.index,
>     front.lexicon_state, mutator_i_s.weight)` — advance input and
>     mutator state, lexicon state unchanged.
>   - Else (mutator emits a real symbol): if the lexicon has no
>     transitions for `(front.lexicon_state + 1,
>     alphabet_translator[mutator_i_s.symbol])`, increment `next_m`, refetch
>     `mutator_i_s = mutator->take_non_epsilons(next_m, input[input_state])`,
>     and continue. Otherwise let `next_l = lexicon->next(front.lexicon_state,
>     alphabet_translator[mutator_i_s.symbol])` and `lexicon_i_s =
>     lexicon->take_non_epsilons(next_l, alphabet_translator[...])`. While
>     `lexicon_i_s.symbol != NO_SYMBOL_NUMBER`: push `front.update(
>     lexicon_i_s.symbol, input_state + 1, mutator_i_s.index,
>     lexicon_i_s.index, lexicon_i_s.weight + mutator_i_s.weight)`, increment
>     `next_l`, and refetch `lexicon_i_s`.
>   - Increment `next_m` and refetch `mutator_i_s = mutator->take_non_epsilons(
>     next_m, input[input_state])`.
> - Note: `queue.front()` is re-read each iteration as the basis node;
>   pushes append to the back so the front is unchanged.

> [spec:hfst:def:ospell.hfst-ol.speller.correct-fn]
> CorrectionQueue Speller::correct(char * line)

> [spec:hfst:sem:ospell.hfst-ol.speller.correct-fn]
> Produces a `CorrectionQueue` of weighted spelling corrections for the
> input `line`, by composing the mutator (error model) with the lexicon.
> Steps:
> - Call `init_input(line, mutator->get_encoder(),
>   mutator->get_unknown_symbol())`. If it returns false (tokenization
>   failed), return an empty `CorrectionQueue()`.
> - Create a local `std::map<std::string, Weight> corrections`.
> - Construct a start `TreeNode` from `lexicon->get_fd_table()` and set
>   `queue` to contain exactly that node.
> - While `queue` is non-empty:
>   - Call `lexicon_epsilons()` then `mutator_epsilons()` (each pushes
>     successor nodes onto the back of `queue`).
>   - If `queue.front().input_state == input.len()` (all input consumed):
>     if BOTH `mutator->final_index(front.mutator_state)` and
>     `lexicon->final_index(front.lexicon_state)` are true, compute the
>     output `string = stringify(front.string)` and `weight = front.weight
>     + lexicon->final_weight(front.lexicon_state) +
>     mutator->final_weight(front.mutator_state)`. If `string` is not yet in
>     `corrections` OR the stored weight is greater than `weight`, set
>     `corrections[string] = weight` (keep best/lowest weight).
>   - Else (input remains), call `consume_input()`.
>   - Pop the front node.
> - Build a `CorrectionQueue correction_queue`; for each entry in
>   `corrections`, push `StringWeightPair(string, weight)`. Return it.

> [spec:hfst:def:ospell.hfst-ol.speller.init-input-fn]
> bool Speller::init_input(char * str,

> [spec:hfst:sem:ospell.hfst-ol.speller.init-input-fn]
> Thin delegate: calls and returns `input.initialize(encoder, str, other)`
> on the member `input` (an InputString), tokenizing `str` with the given
> `encoder` and `other` symbol. Returns the boolean success result.

> [spec:hfst:def:ospell.hfst-ol.speller.lexicon-consume-fn]
> void Speller::lexicon_consume(void)

> [spec:hfst:sem:ospell.hfst-ol.speller.lexicon-consume-fn]
> Expands the front `queue` node by taking lexicon transitions on the
> current input symbol (used by `check`, no mutator involved), pushing
> successors onto the back of `queue`. Steps:
> - Let `input_state = queue.front().input_state`. If `input_state >=
>   input.len()` OR the lexicon has no transitions for
>   `(front.lexicon_state + 1, input[input_state])`, return.
> - Let `next = lexicon->next(front.lexicon_state, input[input_state])`
>   and `i_s = lexicon->take_non_epsilons(next, input[input_state])`.
> - While `i_s.symbol != NO_SYMBOL_NUMBER`: push `front.update(i_s.symbol,
>   input_state + 1, front.mutator_state, i_s.index, i_s.weight)` (advance
>   input and lexicon state, mutator state unchanged); then increment
>   `next` and refetch `i_s = lexicon->take_non_epsilons(next,
>   input[input_state])`.

> [spec:hfst:def:ospell.hfst-ol.speller.lexicon-epsilons-fn]
> void Speller::lexicon_epsilons(void)

> [spec:hfst:sem:ospell.hfst-ol.speller.lexicon-epsilons-fn]
> Expands the front `queue` node by following lexicon epsilon and flag
> transitions, pushing successors onto the back of `queue`. Steps:
> - If `lexicon->has_epsilons_or_flags(front.lexicon_state + 1)` is false,
>   return.
> - Let `next = lexicon->next(front.lexicon_state, 0)` and `i_s =
>   lexicon->take_epsilons_and_flags(next)`.
> - While `i_s.symbol != NO_SYMBOL_NUMBER`:
>   - If the input symbol of `lexicon->get_transition(next)` is 0 (a true
>     epsilon, not a flag): push `front.update_lexicon(i_s.symbol,
>     i_s.index, i_s.weight)`.
>   - Else (the transition's input symbol is a flag diacritic): let
>     `front = queue.front()`; if `front.flag_state.apply_operation(
>     lexicon->get_transition(next).get_input_symbol())` succeeds (the flag
>     operation is permitted, mutating a copy of the flag state held by the
>     new node), push `front.update_lexicon(i_s.symbol, i_s.index,
>     i_s.weight)`. If it fails, push nothing.
>   - Increment `next` and refetch `i_s = lexicon->take_epsilons_and_flags(
>     next)`.

> [spec:hfst:def:ospell.hfst-ol.speller.mutator-epsilons-fn]
> void Speller::mutator_epsilons(void)

> [spec:hfst:sem:ospell.hfst-ol.speller.mutator-epsilons-fn]
> Expands the front `queue` node by following mutator epsilon (input-side)
> transitions, jointly advancing the lexicon for non-epsilon mutator
> outputs, pushing successors onto the back of `queue`. Steps:
> - If `mutator->has_transitions(front.mutator_state + 1, 0)` is false,
>   return.
> - Let `next_m = mutator->next(front.mutator_state, 0)` and `mutator_i_s =
>   mutator->take_epsilons(next_m)`.
> - While `mutator_i_s.symbol != NO_SYMBOL_NUMBER`:
>   - If `mutator_i_s.symbol == 0` (mutator output epsilon): push
>     `front.update_mutator(mutator_i_s.symbol, mutator_i_s.index,
>     mutator_i_s.weight)` (advance mutator state only).
>   - Else (mutator emits a real symbol): if the lexicon has no transitions
>     for `(front.lexicon_state + 1, alphabet_translator[mutator_i_s.symbol])`,
>     increment `next_m`, refetch `mutator_i_s = mutator->take_epsilons(next_m)`,
>     and continue. Otherwise let `next_l = lexicon->next(front.lexicon_state,
>     alphabet_translator[mutator_i_s.symbol])` and `lexicon_i_s =
>     lexicon->take_non_epsilons(next_l, alphabet_translator[...])`. While
>     `lexicon_i_s.symbol != NO_SYMBOL_NUMBER`: push `front.update(
>     lexicon_i_s.symbol, mutator_i_s.index, lexicon_i_s.index,
>     lexicon_i_s.weight + mutator_i_s.weight)` (the four-arg update that
>     leaves input_state unchanged, advancing both mutator and lexicon
>     states); increment `next_l` and refetch `lexicon_i_s`.
>   - Increment `next_m` and refetch `mutator_i_s = mutator->take_epsilons(
>     next_m)`.

> [spec:hfst:def:ospell.hfst-ol.speller.stringify-fn]
> std::string Speller::stringify(SymbolNumberVector symbol_vector)

> [spec:hfst:sem:ospell.hfst-ol.speller.stringify-fn]
> Converts a `SymbolNumberVector` into a single string. Starts with an
> empty `std::string s`; for each symbol number in `symbol_vector` in
> order, appends `symbol_table[symbol]` (the member symbol-number ->
> string table) to `s`. Returns `s`.

> [spec:hfst:def:ospell.hfst-ol.tree-node.update-fn]
> TreeNode TreeNode::update(SymbolNumber next_symbol,

> [spec:hfst:sem:ospell.hfst-ol.tree-node.update-fn]
> Returns a new `TreeNode` derived from `this`, appending one output
> symbol and advancing input, mutator, and lexicon states. The annotated
> overload takes `(next_symbol, next_input, next_mutator, next_lexicon,
> weight)`. Steps: copy `this->string` into a local vector `str`, push
> `next_symbol` onto it, then construct and return `TreeNode(str,
> next_input, next_mutator, next_lexicon, this->flag_state, this->weight +
> weight)` — flag state is carried unchanged and the new weight is the old
> weight plus `weight`.
> There is also a four-arg sibling overload `(next_symbol, next_mutator,
> next_lexicon, weight)` (same annotation group) that behaves identically
> except it keeps `this->input_state` unchanged instead of taking a new
> input state.

> [spec:hfst:def:ospell.hfst-ol.tree-node.update-lexicon-fn]
> TreeNode TreeNode::update_lexicon(SymbolNumber next_symbol,

> [spec:hfst:sem:ospell.hfst-ol.tree-node.update-lexicon-fn]
> Returns a new `TreeNode` derived from `this`, appending one output
> symbol and advancing only the lexicon state. Takes `(next_symbol,
> next_lexicon, weight)`. Steps: copy `this->string` into a local vector
> `str`, push `next_symbol` onto it, then construct and return `TreeNode(
> str, this->input_state, this->mutator_state, next_lexicon,
> this->flag_state, this->weight + weight)` — input state, mutator state,
> and flag state are carried unchanged; the new weight is the old weight
> plus `weight`.

> [spec:hfst:def:ospell.hfst-ol.tree-node.update-mutator-fn]
> TreeNode TreeNode::update_mutator(SymbolNumber next_symbol,

> [spec:hfst:sem:ospell.hfst-ol.tree-node.update-mutator-fn]
> Returns a new `TreeNode` derived from `this`, appending one output
> symbol and advancing only the mutator state. Takes `(next_symbol,
> next_mutator, weight)`. Steps: copy `this->string` into a local vector
> `str`, push `next_symbol` onto it, then construct and return `TreeNode(
> str, this->input_state, next_mutator, this->lexicon_state,
> this->flag_state, this->weight + weight)` — input state, lexicon state,
> and flag state are carried unchanged; the new weight is the old weight
> plus `weight`.

