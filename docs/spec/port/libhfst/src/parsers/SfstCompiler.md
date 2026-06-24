# libhfst/src/parsers/SfstCompiler.cc, libhfst/src/parsers/SfstCompiler.h

> [spec:hfst:def:sfst-compiler.hfst.character]
> typedef unsigned int Character

> [spec:hfst:def:sfst-compiler.hfst.contexts]
> typedef struct contexts_t

> [spec:hfst:def:sfst-compiler.hfst.contexts-t]
> struct contexts_t {
>   HfstTransducer *left, *right;
>   struct contexts_t *next;
> }

> [spec:hfst:def:sfst-compiler.hfst.number-pair]
> typedef std::pair<unsigned int, unsigned int> NumberPair

> [spec:hfst:def:sfst-compiler.hfst.number-pair-set]
> typedef std::set<NumberPair> NumberPairSet

> [spec:hfst:def:sfst-compiler.hfst.number-pair-vector]
> typedef std::vector<NumberPair> NumberPairVector

> [spec:hfst:def:sfst-compiler.hfst.range]
> typedef struct range_t

> [spec:hfst:def:sfst-compiler.hfst.range-t]
> struct range_t {
>   Character character;
>   struct range_t *next;
> }

> [spec:hfst:def:sfst-compiler.hfst.ranges]
> typedef struct ranges_t

> [spec:hfst:def:sfst-compiler.hfst.ranges-t]
> struct ranges_t {
>   Range *range;
>   struct ranges_t *next;
> }

> [spec:hfst:def:sfst-compiler.hfst.repl-type]
> typedef enum

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler]
> class SfstCompiler {
>   struct ltstr { // [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.ltstr.operator-fn] // [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.ltstr.operator-fn] bo...;
>   struct eqstr { // [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.eqstr.operator-fn] // [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.eqstr.operator-fn] bo...;
>   VarMap VM;
>   SVarMap SVM;
>   RVarSet RS;
>   RVarSet RSS;
>   HfstTransducer * result_;
>   bool Verbose;
>   bool Alphabet_Defined;
>   SfstAlphabet TheAlphabet;
>   ImplementationType compiler_type;
>   std::string filename;
>   std::string foldername;
>   int switch_;
> }

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.add-context-fn]
> Contexts *SfstCompiler::add_context( Contexts *nc, Contexts *c )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.add-context-fn]
> Prepends context node `nc` onto the context list `c`. First checks
> type compatibility: if `nc->left->get_type() != c->left->get_type()`
> OR `nc->right->get_type() != c->right->get_type()`, prints
> "ERROR: in sfst-compiler.yy: context transducers do not have the same
> type.\n" to stderr and throws HfstException. Otherwise sets
> `nc->next = c` and returns `nc` (so `nc` becomes the new head of the
> singly-linked list). No allocation; mutates `nc` in place.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.add-range-fn]
> Ranges * SfstCompiler::add_range( Range *r, Ranges *l )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.add-range-fn]
> Allocates a new `Ranges` node, sets its `range` field to `r` and its
> `next` field to `l`, and returns the new node. This prepends `r` as a
> new head onto the `Ranges` singly-linked list `l`. Pure cons-cell
> construction; no mutation of existing nodes, no I/O.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.add-value-fn]
> Range * SfstCompiler::add_value( Character c, Range *r)

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.add-value-fn]
> Allocates a new `Range` node, sets its `character` field to `c` and
> its `next` field to `r`, and returns the new node. This prepends a
> single character `c` as the new head of the `Range` singly-linked
> list `r`. Pure cons-cell construction; no mutation, no I/O.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.add-values-fn]
> Range * SfstCompiler::add_values( unsigned int c1, unsigned int c2, Range *r)

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.add-values-fn]
> Prepends a contiguous range of character codes `c1..c2` (inclusive)
> onto the `Range` list `r`. Iterates `c` downward from `c2` to `c1`;
> for each value calls `character_code(c)` (which converts the unsigned
> int code via UTF-8 into an internal alphabet Character) and prepends
> it via `add_value(character_code(c), r)`, reassigning `r` each step.
> Because iteration is descending and each value is prepended, the
> resulting list is in ascending order `c1, c1+1, ..., c2, <original r>`.
> Returns the final list head. Note `c` is unsigned, so a call where
> `c1 > c2` would underflow/loop indefinitely; callers ensure `c1 <= c2`.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.add-var-values-fn]
> static Range *add_var_values( char *name, Range*)

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.add-var-values-fn]
> Declared only (`static Range *add_var_values( char *name, Range*)`) and
> never defined anywhere in this codebase (the only definitions are this
> declaration plus an identical one in back-ends/sfst/interface.h). It is
> a dead/leftover declaration with no body. Nothing to implement: the Rust
> port should omit it entirely unless a corresponding C++ definition is
> later located.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.append-values-fn]
> Range * SfstCompiler::append_values( Range *r2, Range *r )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.append-values-fn]
> Recursively concatenates the `Range` list `r2` in front of the list
> `r`, producing a fresh copy of `r2`'s nodes whose tail is `r`. Base
> case: if `r2 == NULL`, return `r`. Otherwise return
> `add_value(r2->character, append_values(r2->next, r))`, i.e. allocate
> a new node holding `r2->character` and recurse on `r2->next`. The
> original `r2` nodes are not freed or mutated; `r` is shared (not
> copied). Result preserves the order of `r2` followed by `r`.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.character-code-fn]
> Character SfstCompiler::character_code( unsigned int uc )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.character-code-fn]
> Converts a Unicode codepoint `uc` to an internal alphabet Character.
> Calls `sfst_utf8::int2utf8(uc)` to get the UTF-8 byte string for the
> codepoint, duplicates it with `sfst_basic::fst_strdup` (so a heap copy
> is owned), then passes that to `symbol_code(...)`, which interns the
> symbol in `TheAlphabet` (adding it if new) and frees the duplicated
> string. Returns the resulting Character code. UTF-8 is always used.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.complement-range-fn]
> Range * SfstCompiler::complement_range( Range *r )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.complement-range-fn]
> Computes the alphabet complement of the character set in `Range` list
> `r`. Walks `r` collecting each `character` into a `std::vector<Character>
> sym`, then frees the input list with `free_values(r)`. Calls
> `TheAlphabet.complement(sym)` which replaces `sym` in place with all
> alphabet symbols NOT in the original set. If the resulting `sym` is
> empty, calls `error("Empty character range!")` (which throws). Otherwise
> builds a new `Range` list by iterating `i` from 0 to `sym.size()-1`,
> prepending each `sym[i]` (allocating a new node whose `next` points at
> the previously built head). Because of prepending while iterating
> forward, the result is in reverse order of `sym`. Returns the new list.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.copy-values-fn]
> Range *SfstCompiler::copy_values( const Range *r )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.copy-values-fn]
> Returns a deep copy of the `Range` list `r`. Base case: if `r == NULL`
> return NULL. Otherwise return `add_value(r->character, copy_values(
> r->next))`, allocating a new node for the head character and recursing
> on the tail. The original list is neither mutated nor freed; the copy
> preserves order. Const input.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.def-alphabet-fn]
> void SfstCompiler::def_alphabet( HfstTransducer *tr )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.def-alphabet-fn]
> Defines `TheAlphabet` from a transducer `tr`. First explodes agreement
> variables (`tr = explode(tr)`) and minimizes (`tr->minimize()`). Resets
> the alphabet's pair set via `TheAlphabet.clear_pairs()`, then seeds it
> with the unknown symbol (`add(internal_unknown.c_str(), 1)`) and the
> identity symbol (`add(internal_identity.c_str(), 2)`). Then collects
> symbol pairs into the alphabet by one of two branches: if
> `tr->type == SFST_TYPE` (the `false ||` makes only the type test
> matter), iterates over `tr->get_symbol_pairs()` and for each pair maps
> each side to a code (the literal "<>" maps to 0 epsilon, otherwise
> `TheAlphabet.symbol2code(...)`) and inserts the `NumberPair(inumber,
> onumber)`. Otherwise builds an `HfstBasicTransducer t(*tr)` and iterates
> every state's transitions, inserting `NumberPair(symbol2code(input),
> symbol2code(output))` for each transition. In both branches finally sets
> `Alphabet_Defined = 1`. Mutates `tr` (replaced by exploded/minimized
> version; the new `tr` is not deleted here). No return value.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.def-rvar-fn]
> bool SfstCompiler::def_rvar( char *name, HfstTransducer *t )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.def-rvar-fn]
> Defines a (range/agreement) variable that must be acyclic. If
> `t->is_cyclic()` is true, calls `error2("cyclic transducer assigned to",
> name)` (which prints to stderr and throws HfstException). Otherwise
> delegates entirely to `def_var(name, t)` and returns its result (always
> false in the success path). Same name/transducer-map semantics as
> def_var.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.def-svar-fn]
> bool SfstCompiler::def_svar( char *name, Range *r )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.def-svar-fn]
> Defines a string/range-set variable `name` with value `r` (a `Range*`)
> in the `SVM` map (keyed by `std::string(name)`). If an entry already
> exists for `name`, erases it and `delete`s the old `Range*` value first
> (the old node, not recursively freed). Then stores
> `SVM[std::string(name)] = r`. Returns `r == NULL` (true if the assigned
> value is the empty range). Does not free `name`.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.def-var-fn]
> bool SfstCompiler::def_var( char *name, HfstTransducer *t )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.def-var-fn]
> Defines a transducer variable `name` with value `t` in the `VM` map
> (keyed by `std::string(name)`). If an entry already exists for `name`,
> erases it and `delete`s the old `HfstTransducer*` value first. Then
> explodes agreement variables in `t` (`t = explode(t)`) and minimizes it
> (`t->minimize()`), and stores `VM[std::string(name)] = t`. Returns
> `false` unconditionally. Does not free `name`. Takes ownership of the
> (exploded) transducer.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.eqstr]
> struct eqstr

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.eqstr.operator-fn]
> bool operator()(const char* s1, const char* s2) const

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.eqstr.operator-fn]
> C-string equality comparator functor. Returns `strcmp(s1, s2) == 0`,
> i.e. true iff the two NUL-terminated strings are byte-for-byte equal.
> Used as an equality predicate for hashed/keyed containers of `char*`.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.error-fn]
> void SfstCompiler::error( const char *message )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.error-fn]
> Prints `"\nError: " << message << "\naborted.\n"` to `std::cerr`, then
> throws `HfstException` (via HFST_THROW). Never returns.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.error2-fn]
> void SfstCompiler::error2( const char *message, char *input )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.error2-fn]
> Prints `"\nError: " << message << ": " << input << "\naborted.\n"` to
> `std::cerr` (appending the offending `input` string after the message),
> then throws `HfstException` (via HFST_THROW). Never returns.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.explode-fn]
> HfstTransducer * SfstCompiler::explode( HfstTransducer *t )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.explode-fn]
> Expands all pending agreement variables recorded in the sets `RS`
> (transducer agreement variables) and `RSS` (range agreement variables)
> into transducer `t`, returning the expanded transducer.
> Early exit: if both `RS` and `RSS` are empty, return `t` unchanged.
> Otherwise call `t->minimize()`.
> Phase 1 (RS): copy all names from `RS` into a vector `name`, then clear
> `RS`. For each `name[i]`: create an empty result transducer `nt` of
> `t`'s type; obtain the variable's transducer via
> `var_value(strdup(name[i]))` (var_value frees its argument and returns a
> fresh clone) into `vt`. Enumerate `vt`'s paths into a vector of
> transducers `transducer_paths`: if `t->type == SFST_TYPE`, use
> `vt->extract_path_transducers()`; otherwise call
> `vt->extract_paths(paths, -1, -1)` and, for each weighted path, build a
> new HfstTransducer from the path's symbol vector and set its final
> weight to the path weight. Delete `vt`. For each extracted path `j`:
> copy `t` into `ti`, substitute the symbol pair `(name[i], name[i])` in
> `ti` with that path transducer, delete the path transducer, and
> disjunct `ti` into `nt`. After all paths, `free(name[i])`, delete `t`,
> and set `t = nt`.
> Phase 2 (RSS): clear `name`, copy all names from `RSS` into it, clear
> `RSS`. For each `name[i]`: create empty `nt`; get the range via
> `svar_value(strdup(name[i]))` (frees its argument, returns a copy) into
> `r`. While `r != NULL`: copy `t` into `ti`, substitute the (single,
> identity) symbol string `name[i]` with `TheAlphabet.code2symbol(
> r->character)`, disjunct `ti` into `nt`; advance `r` to `r->next`,
> deleting each consumed node. After the loop, `free(name[i])`, delete
> `t`, set `t = nt`.
> Returns the fully expanded `t`. Side effects: empties `RS`/`RSS`,
> allocates/deletes many transducers, frees the duplicated name strings.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.free-values-fn]
> void SfstCompiler::free_values( Ranges *r )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.free-values-fn]
> Recursively frees a `Ranges` singly-linked list (the overload taking
> `Ranges *r`). If `r` is non-null: first frees the embedded `Range` list
> via the sibling `free_values(r->range)` overload, then recurses into
> `free_values(r->next)`, then `delete`s `r` itself. No-op if `r == NULL`.
> The companion `Range*` overload (defined just above, not separately
> annotated) similarly recurses `free_values(r->next)` then deletes the
> node.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.get-result-fn]
> HfstTransducer * SfstCompiler::get_result()

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.get-result-fn]
> Getter. Returns the stored `result_` pointer (the compiled result
> transducer, may be NULL). No side effects.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.in-range-fn]
> bool SfstCompiler::in_range( unsigned int c, Range *r )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.in-range-fn]
> Linear membership test: walks the `Range` list `r` and returns true as
> soon as some node's `character` equals `c`; returns false if the end of
> the list is reached without a match (including when `r == NULL`). No
> mutation, no I/O.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.insert-freely-fn]
> HfstTransducer * SfstCompiler::insert_freely(HfstTransducer *t, Character input, Character output)

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.insert-freely-fn]
> Freely inserts a single symbol pair into transducer `t`. Builds the
> `StringPair` from `(TheAlphabet.code2symbol(input),
> TheAlphabet.code2symbol(output))` and calls `t->insert_freely(pair)`
> (which adds self-loop transitions on that pair at every state).
> Mutates `t` in place and returns the same `t` pointer.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.ltstr]
> struct ltstr

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.ltstr.operator-fn]
> bool operator()(const char* s1, const char* s2) const

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.ltstr.operator-fn]
> C-string less-than comparator functor. Returns `strcmp(s1, s2) < 0`,
> i.e. true iff `s1` sorts before `s2` in lexicographic byte order. Used
> as the ordering predicate for the `std::set<char*, ltstr>` (RVarSet).

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.make-context-fn]
> Contexts *SfstCompiler::make_context( HfstTransducer *l, HfstTransducer *r )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.make-context-fn]
> Builds a single-element `Contexts` node from a left context transducer
> `l` and a right context transducer `r`, either of which may be NULL.
> If both `l` and `r` are non-null and their `get_type()` differ, prints
> "ERROR: in sfst-compiler.yy: context transducers do not have the same
> type.\n" to stderr and throws HfstException. Determines the
> ImplementationType `type` from `l` if non-null else from `r` (assumes
> at least one is non-null). For any NULL side, replaces it with a newly
> allocated epsilon transducer `new HfstTransducer(internal_epsilon,
> type)`. Allocates a new `Contexts`, sets `left = l`, `right = r`,
> `next = NULL`, and returns it.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.make-mapping-fn]
> HfstTransducer * SfstCompiler::make_mapping( Ranges *list1, Ranges *list2, ImplementationType type )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.make-mapping-fn]
> Builds a transducer mapping from two parallel lists of character ranges
> `list1` (upper/input side) and `list2` (lower/output side). Walks both
> `Ranges` lists in lockstep (`l1`, `l2`). For each aligned pair of
> `Ranges` nodes, forms a `StringPairSet sps` containing the cross product
> of every character in `l1->range` with every character in `l2->range`
> (each pair `(code2symbol(r1->character), code2symbol(r2->character))`),
> and appends `sps` to a vector `spsv`. Once one list is exhausted, the
> remaining nodes of the longer list are handled: leftover `list1` nodes
> map each character to epsilon (`code2symbol(0)` on the output side);
> leftover `list2` nodes map epsilon (`code2symbol(0)` on the input side)
> to each character. Each leftover node contributes one `StringPairSet`
> to `spsv`. Finally frees both input lists (`free_values(list1)`,
> `free_values(list2)`) and returns `new HfstTransducer(spsv, type)` (a
> concatenation, in order, of the per-position symbol-pair sets).

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.make-rule-fn]
> HfstTransducer * SfstCompiler::make_rule( HfstTransducer * lc, Range * lower_range, Twol_Type type,

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.make-rule-fn]
> Builds a two-level (twolc) rule transducer from a left context `lc`,
> a lower (input) `Range` `lower_range`, a `Twol_Type type`, an upper
> (output) `Range` `upper_range`, a right context `rc`, and the target
> `implementation_type`.
> If `RS` or `RSS` is non-empty, prints a warning "agreement operation
> inside of replacement rule!" to stderr. If `!Alphabet_Defined`, prints
> an error about two-level rules requiring an alphabet and throws
> HfstException. NULL `lc`/`rc` are each replaced by a new epsilon
> transducer `HfstTransducer(internal_epsilon, implementation_type)`.
> Forms `tr_pair = (lc, rc)`. Builds the alphabet symbol-pair set `sps`
> by iterating every `NumberPair` in `TheAlphabet` and inserting
> `(code2symbol(first), code2symbol(second))`.
> Computes the `mappings` StringPairSet (center of the rule): if either
> `lower_range` or `upper_range` is NULL (wildcard '.'), requires an
> alphabet (else error+throw) and inserts, for every alphabet pair where
> the input is in `lower_range` (or lower is NULL) AND the output is in
> `upper_range` (or upper is NULL), the pair `(code2symbol(first),
> code2symbol(second))`. Otherwise iterates the two ranges in lockstep
> (advancing each until both `next` are null, holding the last char of the
> shorter), inserting `(code2symbol(r1->character),
> code2symbol(r2->character))` each step.
> Then switches on `type`: `twol_left` returns
> `new HfstTransducer(rules::two_level_if(tr_pair, mappings, sps))`;
> `twol_right` returns `...two_level_only_if(...)`; `twol_both` returns
> `...two_level_if_and_only_if(...)`. Falls through to return NULL if
> `type` matches none (unreachable for valid enum values).

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.make-transducer-fn]
> HfstTransducer * SfstCompiler::make_transducer

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.make-transducer-fn]
> Builds a single-letter-transition transducer from an input `Range`
> `r1` and an output `Range` `r2`. Accumulates a `StringPairSet sps`.
> Wildcard branch: if `r1 == NULL` OR `r2 == NULL` (a '.' wildcard),
> requires `Alphabet_Defined` (else prints the wildcard/alphabet error to
> stderr and throws HfstException), then iterates every `NumberPair` in
> `TheAlphabet` and inserts `(code2symbol(it->first),
> code2symbol(it->second))` for each pair whose input is in `r1` (or
> `r1==NULL`) AND output is in `r2` (or `r2==NULL`).
> Non-wildcard branch: iterates `r1` and `r2` in lockstep in an infinite
> loop, each iteration inserting `(code2symbol(r1->character),
> code2symbol(r2->character))`; breaks when both `r1->next` and `r2->next`
> are null; otherwise advances each list that still has a `next` (the
> shorter list holds its last character while the longer continues).
> Returns `new HfstTransducer(sps, type)`. Does not free `r1`/`r2`.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.negation-fn]
> HfstTransducer * SfstCompiler::negation( HfstTransducer *t )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.negation-fn]
> Computes the complement of transducer `t` relative to the universal
> language over `TheAlphabet`. If `RS` or `RSS` is non-empty, calls
> `warn("agreement operation inside of negation")`. If `!Alphabet_Defined`,
> calls `error("Negation requires the definition of an alphabet")` (which
> throws). Builds a `StringPairSet sps` from every `NumberPair` in
> `TheAlphabet` (`(code2symbol(first), code2symbol(second))`). Constructs
> `pi_star = new HfstTransducer(sps, t->get_type())`, applies
> `repeat_star()` to make it the universal (Sigma*) language, then
> `subtract(*t)` to remove `t`'s language. Deletes `t` and returns
> `pi_star` (the negation). Caller-supplied `t` is consumed/freed.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.new-transducer-fn]
> HfstTransducer * SfstCompiler::new_transducer( Range *r1, Range *r2, ImplementationType type )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.new-transducer-fn]
> Constructs a transducer from input range `r1` and output range `r2` via
> `make_transducer(r1, r2, type)`, then frees the range lists and returns
> the transducer. Frees `r1` with `free_values(r1)` only if `r1 != r2`
> (to avoid a double free when the same list is passed for both sides),
> and always frees `r2` with `free_values(r2)`. Returns the new
> transducer `t`.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.parse-fn]
> void SfstCompiler::parse()

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.parse-fn]
> Runs the bison/yacc-generated SFST grammar parser by calling the
> free function `sfstparse()`. Reads from the global `sfstin` FILE* (set
> via set_input) and drives the grammar actions, which populate this
> compiler's state (e.g. `result_`) through the global `sfst_compiler`
> pointer. No return value; ignores `sfstparse()`'s int return.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.r-var-set]
> typedef std::set<char*, ltstr> RVarSet

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.read-transducer-fn]
> HfstTransducer * SfstCompiler::read_transducer(const char *folder, char *filename, ImplementationType type)

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.read-transducer-fn]
> Reads a single transducer from a file and converts it to `type`. Builds
> the path `filestr`: if `folder != NULL`, append `folder` then "/" (a
> Windows-path FIXME), then append `filename`. If `Verbose`, prints
> "\nreading transducer from <filestr>..." to stderr. Opens an
> `HfstInputStream is(filestr.c_str())`, reads one transducer via
> `new HfstTransducer(is)`, closes the stream, and `free(filename)`. If
> `Verbose`, prints "finished\n". Calls `t->convert(type)` to coerce the
> implementation type, and returns `t`. (Unlike read_words, the folder
> emptiness is not specially handled; an empty non-null folder yields a
> leading "/".)

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.read-words-fn]
> HfstTransducer * SfstCompiler::read_words(const char *folder, char *filename,

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.read-words-fn]
> Reads a word list file and builds a transducer disjuncting all the
> word entries. Builds the path `filestr`: if `folder != NULL` and
> `folder != ""`, append `folder` then "/" (Windows FIXME); then append
> `filename`. If `Verbose`, prints "\nreading words from <filestr>..." to
> stderr. Opens an `std::ifstream is(filestr)`. If the stream failed to
> open, formats "Error: Cannot open file \"<filestr>\"!" into a static
> char buffer and `throw`s that `char*` (a C++ throw of a C string, not
> HfstException). Then `free(filename)`.
> Chooses an accumulation strategy: if `type` is FOMA_TYPE,
> TROPICAL_OPENFST_TYPE, or LOG_OPENFST_TYPE, it accumulates into an
> `HfstBasicTransducer retval_fsm`; otherwise it allocates
> `retval_hfst = new HfstTransducer(type)` and accumulates into it.
> Reads lines into a 10000-byte buffer with `is.getline`. For Verbose
> progress, increments `n` per line and every 10000 lines prints
> "\r<n> words" to cerr (printing a leading "\n" the first time n hits
> 10000). For each line: strips trailing whitespace (space, tab, CR) by
> scanning from the end, but stops trimming at a whitespace char that is
> escaped (i.e. keep going only while the char is whitespace and not
> immediately preceded by a backslash); NUL-terminates after the last
> kept char. Then tokenizes the line into symbol pairs using
> `TheAlphabet.next_label(bufptr, true)` repeatedly, advancing `bufptr`,
> pushing each `(code2symbol(first), code2symbol(second))` into a
> `StringPairVector spv` until next_label returns (0,0). Disjuncts the
> path: for the non-OpenFST/Foma branch `retval_hfst->disjunct(spv)`;
> otherwise `retval_fsm.disjunct(spv, 0)`. After all lines: if Verbose
> and n>=10000 print "\n"; close the stream; if Verbose print
> "finished\n". Returns `retval_hfst` for the non-OpenFST/Foma branch,
> else `new HfstTransducer(retval_fsm, type)`.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.replace-fn]
> HfstTransducer * SfstCompiler::replace(HfstTransducer * mapping, Repl_Type repl_type, bool optional)

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.replace-fn]
> Builds a context-free (unconditional) replace-rule transducer from a
> `mapping` transducer, a `Repl_Type repl_type`, and an `optional` flag.
> First builds a `StringPairSet sps` by iterating every `NumberPair` in
> `TheAlphabet` and inserting `(code2symbol(first), code2symbol(second))`
> (the rule alphabet). Then switches on `repl_type`: `repl_up` returns
> `new HfstTransducer(rules::replace_up(*mapping, optional, sps))`;
> `repl_down` returns `...replace_down(*mapping, optional, sps)`. For any
> other repl_type (default), prints "ERROR: invalid replace type
> requested\n" to stderr and throws HfstException. (If control somehow
> reaches the end it prints "ERROR: in function SfstCompiler::replace\n"
> and throws HfstException, but the switch cases already return/throw.)
> Note: unlike replace_in_context, no context pair is used here.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.replace-in-context-fn]
> HfstTransducer * SfstCompiler::replace_in_context(HfstTransducer * mapping, Repl_Type repl_type, Contexts *contexts, bool optional)

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.replace-in-context-fn]
> Builds a context-conditioned replace-rule transducer from a `mapping`
> transducer, a `Repl_Type repl_type`, a single `Contexts *contexts`
> node, and an `optional` flag. Forms the context pair
> `tr_pair = (*(contexts->left), *(contexts->right))` (uses only the
> first/head node of the contexts list, not the whole list). Builds a
> `StringPairSet sps` (the rule alphabet) by iterating every `NumberPair`
> in `TheAlphabet` and inserting `(code2symbol(first),
> code2symbol(second))`. Then switches on `repl_type`: `repl_up` returns
> `new HfstTransducer(rules::replace_up(tr_pair, *mapping, optional,
> sps))`; `repl_down` -> `replace_down(...)`; `repl_left` ->
> `replace_left(...)`; `repl_right` -> `replace_right(...)`, each with the
> same `(tr_pair, *mapping, optional, sps)` arguments. Returns NULL if
> `repl_type` matches no case (unreachable for valid enum values).

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.restriction-fn]
> HfstTransducer * SfstCompiler::restriction( HfstTransducer * t, Twol_Type type, Contexts *c, int direction )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.restriction-fn]
> Builds a two-level restriction rule transducer from a center transducer
> `t`, a `Twol_Type type`, a context list `c`, and an int `direction`.
> First builds a `StringPairSet sps` (the rule alphabet) by iterating
> every `NumberPair` in `TheAlphabet` and inserting `(code2symbol(first),
> code2symbol(second))`. Then collects all contexts: walks the `Contexts`
> singly-linked list from `c`, and for each node pushes
> `HfstTransducerPair(*(p->left), *(p->right))` into an
> `HfstTransducerPairVector contexts`, advancing `p = p->next`. Returns
> `new HfstTransducer(hfst::rules::restriction(contexts, *t, sps,
> (hfst::rules::TwolType)type, direction))`. The `type` enum value is cast
> directly to `rules::TwolType`. Does not free `t` or the contexts list.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.result-fn]
> HfstTransducer * SfstCompiler::result( HfstTransducer *t, bool switch_flag)

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.result-fn]
> Finalizes the compiled result transducer `t`. First explodes pending
> agreement variables (`t = explode(t)`). Then tears down all defined
> transducer variables: iterates the `VM` map, `delete`s each value
> transducer (`it->second`) and sets it to NULL, then `VM.clear()`. (The
> commented-out code that would also free the key strings is not active.)
> If `switch_flag` is true, calls `t->invert()` (swaps input/output
> tapes). Then `t->minimize()`. Returns the resulting `t`. (The string-
> variable map SVM is not cleared here.)

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.rsvar-value-fn]
> Range *SfstCompiler::rsvar_value( char *name )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.rsvar-value-fn]
> Looks up / registers a range agreement variable `name` and returns a
> single-element `Range` referencing it. If `name` is not already in the
> `RSS` set, inserts a heap copy `sfst_basic::fst_strdup(name)` into RSS
> (so RSS owns its own copy). Then returns `add_value(symbol_code(name),
> NULL)`, i.e. a new single-node Range whose character is the interned
> code for `name`. Note `symbol_code` frees the passed-in `name`, so the
> RSS copy (not the original) persists. The actual range expansion is
> deferred to `explode` (phase 2).

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.rvar-value-fn]
> HfstTransducer * SfstCompiler::rvar_value( char *name, ImplementationType type )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.rvar-value-fn]
> Looks up / registers a transducer agreement variable `name` and returns
> a placeholder transducer of implementation type `type`. If `name` is not
> already in the `RS` set, inserts a heap copy `sfst_basic::fst_strdup(
> name)` into RS (RS owns its own copy). Then builds a single-node `Range`
> `r = add_value(symbol_code(name), NULL)` (symbol_code interns the name
> into TheAlphabet and frees the passed-in `name`), and returns
> `new_transducer(r, r, type)` — i.e. an identity transducer over the
> single agreement-marker symbol (same Range used for both input and
> output, so new_transducer avoids the double-free). The actual variable
> expansion is deferred to `explode` (phase 1).

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.s-var-map]
> typedef unordered_map<std::string,Range*> SVarMap

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.set-filename-fn]
> void SfstCompiler::set_filename(const std::string & name)

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.set-filename-fn]
> Setter. Assigns the member `filename = name` (copying the string). No
> other effect, no return value.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.set-foldername-fn]
> void SfstCompiler::set_foldername(const std::string & name)

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.set-foldername-fn]
> Setter. Assigns the member `foldername = name` (copying the string). No
> other effect, no return value.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.set-input-fn]
> void SfstCompiler::set_input(FILE * input)

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.set-input-fn]
> Sets the parser's input source by assigning the global FILE pointer
> `sfstin = input` (the extern global the bison/flex scanner reads from).
> No return value, no other side effects.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.set-result-fn]
> void SfstCompiler::set_result(HfstTransducer * tr)

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.set-result-fn]
> Setter. Assigns the member pointer `result_ = tr` (no copy, no deletion
> of any previous value). No return value.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.set-switch-fn]
> void SfstCompiler::set_switch(int value)

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.set-switch-fn]
> Setter. Assigns the member `switch_ = value`. No return value, no other
> side effects.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.sfst-compiler-fn]
> SfstCompiler::SfstCompiler( ImplementationType type, bool verbose /*=false*/ )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.sfst-compiler-fn]
> Constructor taking `ImplementationType type` and an optional `bool
> verbose` (default false). Initializes members: `result_ = NULL`,
> `Verbose = verbose`, `Alphabet_Defined = false`, `compiler_type = type`,
> `filename = ""`, `foldername = ""`, `switch_ = 0`. Finally registers
> this instance as the global `sfst_compiler = this` (so grammar actions
> can reach it). The maps VM/SVM/RS/RSS/TheAlphabet are left
> default-constructed (empty).

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.substitute-fn]
> HfstTransducer * SfstCompiler::substitute(HfstTransducer *t, Character old_char_in, Character old_char_out,

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.substitute-fn]
> Substitutes one symbol pair for another throughout transducer `t`. This
> is the 4-Character overload `(t, old_char_in, old_char_out, new_char_in,
> new_char_out)`. Calls `t->substitute(StringPair(code2symbol(old_char_in),
> code2symbol(old_char_out)), StringPair(code2symbol(new_char_in),
> code2symbol(new_char_out)))`, replacing every transition labeled with the
> old (input,output) pair by the new (input,output) pair. Mutates `t` in
> place and returns the same `t` pointer. (Sibling overloads — not this
> one — handle single-Character substitution and pair->transducer
> substitution.)

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.svar-value-fn]
> Range *SfstCompiler::svar_value( char *name )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.svar-value-fn]
> Returns the value (a deep-copied `Range*`) of a previously-defined
> string/range variable `name`. Looks up `std::string(name)` in the `SVM`
> map. If not found, calls `error2("undefined variable", name)` (prints to
> stderr and throws HfstException). Otherwise `free(name)` and returns
> `copy_values(it->second)` — a fresh deep copy of the stored Range list,
> leaving the stored value intact.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.symbol-code-fn]
> Character SfstCompiler::symbol_code( char *symbol )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.symbol-code-fn]
> Interns a symbol string `symbol` into `TheAlphabet`, returning its
> Character code. Special case: if `symbol` equals "<>" (the SFST source
> notation for epsilon, distinct from HFST's "@_EPSILON_SYMBOL_@"), returns
> 0 immediately WITHOUT freeing `symbol`. Otherwise looks up
> `TheAlphabet.symbol2code(symbol)`; if that returns EOF (not present),
> adds it via `TheAlphabet.add_symbol(symbol)` and uses that new code.
> Then `free(symbol)` (the input string is freed in the non-"<>" path) and
> returns the code cast to `Character`.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.utf8toint-fn]
> unsigned int SfstCompiler::utf8toint( char *s )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.utf8toint-fn]
> Thin wrapper. Decodes the first UTF-8 character of C-string `s` to its
> Unicode codepoint by delegating to `sfst_utf8::utf8toint(s)` and returns
> that. No mutation of compiler state.

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.var-map]
> typedef unordered_map<std::string,HfstTransducer*> VarMap

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.var-value-fn]
> HfstTransducer * SfstCompiler::var_value( char *name )

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.var-value-fn]
> Returns a fresh clone of the transducer bound to variable `name`. Looks
> up `std::string(name)` in the `VM` map. If not found, prints "undefined
> variable <name>\n" to stdout (printf) and throws HfstException.
> Otherwise `free(name)` and returns `new HfstTransducer(*(it->second))`
> (a copy; the stored transducer is left intact and still owned by VM).

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.warn-fn]
> void SfstCompiler::warn(const char *msg)

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.warn-fn]
> Prints a warning to stderr: writes `"\nWarning: " << msg << "!\n"` to
> `std::cerr`. Does NOT throw and returns normally (unlike `error`).

> [spec:hfst:def:sfst-compiler.hfst.sfst-compiler.write-to-file-fn]
> void SfstCompiler::write_to_file(HfstTransducer *t, const char* folder, char* filename)

> [spec:hfst:sem:sfst-compiler.hfst.sfst-compiler.write-to-file-fn]
> Writes transducer `t` to a binary HFST file. Builds the path `filestr`:
> if `folder != NULL`, append `folder` then "/" (Windows FIXME); then
> append `filename` (note: unlike read_words, no empty-folder check, so a
> non-null empty folder yields a leading "/"). Opens an
> `HfstOutputStream os(filestr, t->get_type())`, streams the transducer
> with `os << *t`, then `os.close()`. Returns void. Does not free
> `filename` or delete `t`.

> [spec:hfst:def:sfst-compiler.hfst.twol-type]
> typedef enum

> [spec:hfst:def:sfst-compiler.sfstparse-fn]
> int sfstparse( void )

> [spec:hfst:sem:sfst-compiler.sfstparse-fn]
> External entry point of the bison/yacc-generated SFST grammar parser,
> only declared here (`int sfstparse(void);`) — its body is the generated
> parser (from sfst-compiler.yy), not defined in this file. When invoked
> it runs the LALR parse loop, reading tokens from the flex scanner over
> the global `sfstin` FILE*, executing the grammar's semantic actions
> (which call into the global `sfst_compiler` SfstCompiler to build
> transducers, define variables, set the result, etc.). Returns 0 on
> success, non-zero on parse error (per standard yacc convention). The
> Rust port reimplements this as the generated/hand-written parser driver;
> there is no hand-written C++ body to translate here.

