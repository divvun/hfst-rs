# back-ends/sfst/interface.h

> [spec:hfst:def:interface.sfst.contexts]
> typedef struct contexts_t

> [spec:hfst:def:interface.sfst.contexts-t]
> struct contexts_t {
>   Transducer *left, *right;
>   struct contexts_t *next;
> }

> [spec:hfst:def:interface.sfst.error-fn]
> void error( const char *message )

> [spec:hfst:sem:interface.sfst.error-fn]
> Free function declared in interface.h. NOTE: the body is not present in
> this checkout — the SFST `interface.C` implementation translation unit
> is not vendored here; only the declaration is available. Contract from
> the signature: a fatal error reporter that takes a single `const char*`
> `message` and returns `void`. It is the unconditional error path
> (paired with `error2`), used by the compiler/parser to report a fatal
> condition by emitting `message` as a diagnostic and stopping further
> processing. The precise output stream and termination mechanism cannot
> be reproduced from the header alone and must be confirmed against
> upstream SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.error2-fn]
> void error2( const char *message, char *input )

> [spec:hfst:sem:interface.sfst.error2-fn]
> Free function declared in interface.h. NOTE: the body is not vendored
> in this checkout (SFST `interface.C` is absent); only the declaration
> is available. Contract from the signature: a fatal error reporter that
> takes a `const char*` `message` plus a `char*` `input` (the offending
> input fragment) and returns `void`. Like `error`, it reports a fatal
> condition, additionally including the `input` text in the diagnostic so
> the user sees which token/string triggered the failure, then stops
> processing. Exact stream and termination behaviour must be confirmed
> against upstream SFST `interface.C`.

> [spec:hfst:def:interface.sfst.interface]
> class Interface {
>   struct ltstr { // [spec:hfst:def:interface.sfst.interface.ltstr.operator-fn] // [spec:hfst:sem:interface.sfst.interface.ltstr.operator-fn] bool operator()(co...;
>   VarMap VM;
>   SVarMap SVM;
>   RVarSet RS;
>   RVarSet RSS;
>   bool Verbose;
>   bool Alphabet_Defined;
>   bool LexiconComments;
>   Alphabet TheAlphabet;
> }

> [spec:hfst:def:interface.sfst.interface.add-alphabet-fn]
> void add_alphabet( Transducer* )

> [spec:hfst:sem:interface.sfst.interface.add-alphabet-fn]
> Member function declared in interface.h; its body lives in the SFST
> `interface.C` translation unit, which is NOT vendored in this checkout
> (only `interface.h` is present), so the implementation cannot be read
> here. Contract from the signature/name: takes a `Transducer*` and
> returns `void`. It merges the symbol/label alphabet of the given
> transducer into the Interface's `TheAlphabet` member, so that symbols
> used by that transducer become known to subsequent compilation steps.
> Exact iteration order and label-vs-symbol handling must be confirmed
> against upstream SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.add-context-fn]
> Contexts *add_context( Contexts *nc, Contexts *c )

> [spec:hfst:sem:interface.sfst.interface.add-context-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes two `Contexts*` (`nc`, a newly
> made single context node, and `c`, the existing list) and returns a
> `Contexts*`. It prepends/links `nc` onto the linked list `c` (a cons-
> style operation on the `contexts_t` singly-linked list via the `next`
> field) and returns the resulting head. Exact link order (whether `nc`
> becomes the new head or is appended) must be confirmed against upstream
> SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.add-pi-transitions-fn]
> void add_pi_transitions( Transducer *t, Node *node, Alphabet &alph )

> [spec:hfst:sem:interface.sfst.interface.add-pi-transitions-fn]
> Private member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes a `Transducer *t`, a `Node *node`
> (a state in `t`), and a reference `Alphabet &alph`; returns `void`. It
> adds, from `node`, the "pi" (sigma/identity) self-loop transitions —
> one transition per label in `alph` — building the transition set that
> lets the node accept any symbol of the alphabet (used to construct the
> pi-machine). Exact label set used (all labels vs. identity pairs) and
> whether it recurses must be confirmed against upstream SFST
> `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.add-range-fn]
> Ranges *add_range( Range*, Ranges* )

> [spec:hfst:sem:interface.sfst.interface.add-range-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes a `Range*` and a `Ranges*` and
> returns a `Ranges*`. It allocates a new `ranges_t` node whose `range`
> field is the given `Range*`, links it onto the given `Ranges*` list via
> the `next` field, and returns the new head — a cons operation building
> the list of character ranges (e.g. for a multi-character mapping).
> Exact link position must be confirmed against upstream SFST
> `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.add-value-fn]
> Range *add_value( Character, Range*)

> [spec:hfst:sem:interface.sfst.interface.add-value-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes a `Character` and a `Range*` and
> returns a `Range*`. It allocates a new `range_t` node whose `character`
> field is the given Character, links it onto the given `Range*` list via
> `next`, and returns the new head — i.e. prepends one character value to
> a character range list. Exact link position and any
> duplicate-handling must be confirmed against upstream SFST
> `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.add-values-fn]
> Range *add_values( unsigned int, unsigned int, Range*)

> [spec:hfst:sem:interface.sfst.interface.add-values-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes two `unsigned int` code points
> (the inclusive bounds of a character range, low and high) plus an
> existing `Range*` list, and returns a `Range*`. It walks every code
> point from the first bound to the second inclusive, maps each through
> `character_code` to obtain a `Character`, prepends each onto the
> running `Range*` list via `add_value`, and returns the extended list —
> i.e. expands a range expression like `a-z` into individual character
> values. Exact iteration direction and resulting order must be confirmed
> against upstream SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.add-var-values-fn]
> Range *add_var_values( char *name, Range*)

> [spec:hfst:sem:interface.sfst.interface.add-var-values-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes a `char *name` (the name of a
> previously defined SVar / range variable) and an existing `Range*`, and
> returns a `Range*`. It looks up the range value bound to `name` (via the
> SVar map / `svar_value`), copies that range's character values, and
> prepends/appends them onto the supplied `Range*` list, returning the
> combined list — i.e. splices the contents of a named set variable into a
> range expression. Whether it consumes/frees `name`, and the exact merge
> order, must be confirmed against upstream SFST `interface.C` before
> re-implementation.

> [spec:hfst:def:interface.sfst.interface.allow-lexicon-comments-fn]
> void allow_lexicon_comments()

> [spec:hfst:sem:interface.sfst.interface.allow-lexicon-comments-fn]
> Inline method (defined in the header). Sets the member field
> `LexiconComments` to `true`. Takes no arguments and returns nothing.
> This enables comment handling when reading lexicon word lists later.

> [spec:hfst:def:interface.sfst.interface.anti-cp-fn]
> Transducer *anti_cp( Range *lower_range, Range *upper_range )

> [spec:hfst:sem:interface.sfst.interface.anti-cp-fn]
> Private member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes `Range *lower_range` and
> `Range *upper_range` and returns a `Transducer*`. Like `cp` it builds a
> cross-product transducer over the two character ranges, but constructs
> the COMPLEMENT/"anti" version: the set of label pairs (lower:upper)
> whose lower symbol is in `lower_range` but whose upper symbol is NOT the
> matching member of `upper_range` (the negated correspondence used in
> two-level rule compilation). Exact pairing semantics and which symbol
> sets are complemented must be confirmed against upstream SFST
> `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.append-values-fn]
> Range *append_values( Range *r2, Range *r )

> [spec:hfst:sem:interface.sfst.interface.append-values-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes two `Range*` lists, `r2` and `r`,
> and returns a `Range*`. It concatenates the two character-range lists —
> walking to the end of one and linking the other on via the `next`
> field (or copying r2's values onto r) — and returns the combined head,
> i.e. the union/append of two character ranges. Exact order of the
> result and whether the inputs are reused or copied must be confirmed
> against upstream SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.catenate-fn]
> Transducer *catenate( Transducer *a1, Transducer *a2 )

> [spec:hfst:sem:interface.sfst.interface.catenate-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes two `Transducer*` `a1` and `a2` and
> returns a `Transducer*` that is their concatenation (a1 followed by a2),
> accepting any string that is a string of a1 concatenated with a string
> of a2. Per the "these functions delete their argument automata" note in
> the header, it consumes/frees `a1` and `a2` and returns a newly
> allocated result. Exact handling of the shared alphabet and whether the
> result is minimised must be confirmed against upstream SFST
> `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.center-transducer-fn]
> Transducer *center_transducer( Transducer *t, Transducer *pi,

> [spec:hfst:sem:interface.sfst.interface.center-transducer-fn]
> Private member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes the rule "center" transducer `t`,
> the pi-machine `pi` (accepting any symbol of the alphabet), and a marker
> transducer `mt`, and returns a `Transducer*`. It builds the center part
> of a replace/context rule by combining `t` with the surrounding
> pi-machine and the marker symbols `mt` so the matched center can be
> located within arbitrary surrounding material. Exact composition order
> and which operands are deleted must be confirmed against upstream SFST
> `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.character-code-fn]
> Character character_code( unsigned int uc )

> [spec:hfst:sem:interface.sfst.interface.character-code-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes an `unsigned int uc` (a Unicode
> code point) and returns a `Character`. It interns the single-character
> symbol for that code point into `TheAlphabet` (allocating a new symbol
> id if not already present) and returns the resulting `Character` code,
> so a literal character used in the grammar gets a stable alphabet code.
> Exact interaction with utf8 mode and the alphabet's symbol table must be
> confirmed against upstream SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.complement-range-fn]
> Range *complement_range( Range* )

> [spec:hfst:sem:interface.sfst.interface.complement-range-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes a `Range*` and returns a `Range*`.
> It computes the complement of the given character set with respect to
> `TheAlphabet`: it builds a new `Range` list containing every single
> character symbol known to the alphabet that is NOT present in the input
> range (the `[^...]` negated character class), and returns it. Whether
> the input range is freed and which alphabet symbols are excluded (e.g.
> epsilon/markers) must be confirmed against upstream SFST `interface.C`
> before re-implementation.

> [spec:hfst:def:interface.sfst.interface.composition-fn]
> Transducer *composition( Transducer *a1, Transducer *a2 )

> [spec:hfst:sem:interface.sfst.interface.composition-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes two `Transducer*` `a1` and `a2` and
> returns a `Transducer*` that is their relational composition (a1 then
> a2): the upper/output side of a1 is matched against the lower/input side
> of a2, yielding a transducer mapping a1's lower side to a2's upper side.
> Per the header note it consumes/frees both argument automata and returns
> a freshly allocated result. Exact direction convention (which level is
> matched) and epsilon handling must be confirmed against upstream SFST
> `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.conjunction-fn]
> Transducer *conjunction( Transducer *a1, Transducer *a2 )

> [spec:hfst:sem:interface.sfst.interface.conjunction-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes two `Transducer*` `a1` and `a2` and
> returns a `Transducer*` that is their conjunction/intersection — the
> transducer accepting exactly the string (pairs) accepted by both `a1`
> and `a2`. Per the header note it consumes/frees both argument automata
> and returns a newly allocated result. Exact alphabet harmonisation and
> whether the result is minimised must be confirmed against upstream SFST
> `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.constrain-boundary-transducer-fn]
> Transducer *constrain_boundary_transducer( Character leftm, Character rm )

> [spec:hfst:sem:interface.sfst.interface.constrain-boundary-transducer-fn]
> Private member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes two boundary marker characters
> `leftm` and `rm` and returns a `Transducer*`. It builds an auxiliary
> transducer used in context/replace-rule compilation that constrains how
> the inserted left marker (`leftm`) and right marker (`rm`) may appear
> relative to each other (e.g. forbidding nesting or out-of-order
> markers), so that only valid markings survive the subsequent
> intersection. Exact accepted language and which marker pairs are
> permitted must be confirmed against upstream SFST `interface.C` before
> re-implementation.

> [spec:hfst:def:interface.sfst.interface.context-transducer-fn]
> Transducer *context_transducer( Transducer *t, Transducer *pi,

> [spec:hfst:sem:interface.sfst.interface.context-transducer-fn]
> Private member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes the center transducer `t`, the
> pi-machine `pi`, a marker transducer `mt`, and a context list `c`
> (`Contexts*`, each holding a left/right context pair), and returns a
> `Transducer*`. It builds the transducer that enforces every
> left/right context in `c` around the center, by combining `t`, the
> markers `mt`, and the pi-machine `pi` and intersecting/iterating over
> the context-list entries. Exact per-context construction and operand
> deletion must be confirmed against upstream SFST `interface.C` before
> re-implementation.

> [spec:hfst:def:interface.sfst.interface.copy-values-fn]
> Range *copy_values( const Range *r )

> [spec:hfst:sem:interface.sfst.interface.copy-values-fn]
> Private member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes a `const Range *r` and returns a
> `Range*`. It performs a deep copy of the character-range linked list:
> walking `r` via `next`, allocating a fresh `range_t` node for each
> element with the same `character`, and returning the head of the new
> independent list (a NULL input yields NULL). The original list is left
> unmodified. Exact node order preservation must be confirmed against
> upstream SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.cp-fn]
> Transducer *cp( Range *lower_range, Range *upper_range )

> [spec:hfst:sem:interface.sfst.interface.cp-fn]
> Private member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes `Range *lower_range` and
> `Range *upper_range` and returns a `Transducer*`. It builds the
> cross-product transducer over the two character ranges: a one-transition
> machine accepting the set of label pairs (lower:upper) formed from the
> Cartesian product (or paired correspondence) of `lower_range` and
> `upper_range`, used as the center of a two-level correspondence. Exact
> pairing rule (full cross-product vs. positional pairing) and epsilon/
> any-symbol handling must be confirmed against upstream SFST
> `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.def-alphabet-fn]
> void def_alphabet( Transducer *a )

> [spec:hfst:sem:interface.sfst.interface.def-alphabet-fn]
> Member function declared in interface.h (one of the functions that
> "delete their argument automata"); body resides in the non-vendored
> SFST `interface.C`, so it cannot be read in this checkout. Contract from
> signature/name: takes a `Transducer *a` and returns `void`. It defines
> the grammar's alphabet from `a`: it merges/sets the symbol-pair alphabet
> of `a` into the Interface's `TheAlphabet`, sets `Alphabet_Defined` to
> true, and frees `a`. After this, subsequent operations treat that
> alphabet as the declared sigma. Exact handling of identity/unknown
> symbols and whether previously defined alphabet entries are cleared must
> be confirmed against upstream SFST `interface.C` before
> re-implementation.

> [spec:hfst:def:interface.sfst.interface.def-rvar-fn]
> bool def_rvar( char *name, Transducer *a )

> [spec:hfst:sem:interface.sfst.interface.def-rvar-fn]
> Member function declared in interface.h (one of the functions that
> "delete their argument automata"); body resides in the non-vendored
> SFST `interface.C`, so it cannot be read in this checkout. Contract from
> signature/name: takes a `char *name` and a `Transducer *a` and returns
> a `bool`. It defines an "agreement"/R-variable named `name` bound to
> transducer `a`: it records `name` in the RVar set/map (`RS`/the var map)
> with value `a`, returning a bool that indicates whether the name was
> newly defined (true) or was already in use (false), used by the parser
> to detect redefinition. Whether `a` is stored directly or copied/freed,
> and the exact true/false convention, must be confirmed against upstream
> SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.def-svar-fn]
> bool def_svar( char *name, Range *r )

> [spec:hfst:sem:interface.sfst.interface.def-svar-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes a `char *name` and a `Range *r` and
> returns a `bool`. It defines a set/range variable named `name` bound to
> the character range `r` by inserting the (name -> r) entry into the
> `SVM` (`SVarMap`), returning a bool indicating whether the name was
> newly defined (true) versus already present (false). Whether `r` is
> stored directly or copied, and the exact return convention, must be
> confirmed against upstream SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.def-var-fn]
> bool def_var( char *name, Transducer *a )

> [spec:hfst:sem:interface.sfst.interface.def-var-fn]
> Member function declared in interface.h (one of the functions that
> "delete their argument automata"); body resides in the non-vendored
> SFST `interface.C`, so it cannot be read in this checkout. Contract from
> signature/name: takes a `char *name` and a `Transducer *a` and returns
> a `bool`. It defines a transducer variable named `name` bound to `a` by
> inserting the (name -> a) entry into `VM` (`VarMap`), returning a bool
> indicating whether the name was newly defined (true) versus already
> present (false). Whether `a` is stored directly or copied/freed, and the
> exact return convention, must be confirmed against upstream SFST
> `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.disjunction-fn]
> Transducer *disjunction( Transducer *a1, Transducer *a2 )

> [spec:hfst:sem:interface.sfst.interface.disjunction-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes two `Transducer*` `a1` and `a2` and
> returns a `Transducer*` that is their disjunction/union — accepting any
> string (pair) accepted by `a1` or by `a2`. Per the header note it
> consumes/frees both argument automata and returns a freshly allocated
> result. Exact alphabet harmonisation and whether the result is
> minimised must be confirmed against upstream SFST `interface.C` before
> re-implementation.

> [spec:hfst:def:interface.sfst.interface.empty-transducer-fn]
> Transducer *empty_transducer()

> [spec:hfst:sem:interface.sfst.interface.empty-transducer-fn]
> Private member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes no arguments and returns a
> `Transducer*`. It allocates and returns a new transducer recognising the
> empty language (no accepting paths / non-final root state) — the
> identity element used as a base case in compilation. Whether its
> alphabet is seeded from `TheAlphabet` must be confirmed against upstream
> SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.explode-fn]
> Transducer *explode( Transducer *a )

> [spec:hfst:sem:interface.sfst.interface.explode-fn]
> Member function declared in interface.h (one of the functions that
> "delete their argument automata"); body resides in the non-vendored
> SFST `interface.C`, so it cannot be read in this checkout. Contract from
> signature/name: takes a `Transducer *a` and returns a `Transducer*`. It
> "explodes" `a` by replacing each surface symbol/range or multi-symbol
> label with the individual single-character transitions it stands for,
> expanding shorthand label sets into their explicit per-character
> transitions, and frees the input `a`. Exact treatment of label ranges
> and identity/unknown symbols must be confirmed against upstream SFST
> `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.extended-left-transducer-fn]
> Transducer *extended_left_transducer( Transducer *t,

> [spec:hfst:sem:interface.sfst.interface.extended-left-transducer-fn]
> Private member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes a `Transducer *t` and two boundary
> marker characters `m1` and `m2`, and returns a `Transducer*`. It builds
> an extended left-context transducer used in replace/context-rule
> compilation by wrapping `t` with the left/right marker symbols `m1` and
> `m2` (inserting the markers around the matched material) so that the
> left context can be recognised within arbitrary surrounding text. Exact
> marker placement and pi-machine combination must be confirmed against
> upstream SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.freely-insert-fn]
> Transducer *freely_insert( Transducer *a, Character lc, Character uc )

> [spec:hfst:sem:interface.sfst.interface.freely-insert-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes a `Transducer *a` and two
> characters `lc` (lower) and `uc` (upper) forming a symbol pair, and
> returns a `Transducer*`. It modifies/rebuilds `a` so that the label
> pair `lc:uc` may be freely inserted anywhere — i.e. it adds a self-loop
> transition on that pair at every state, so the symbol can occur an
> arbitrary number of times at any position (free insertion of a marker).
> Whether it mutates `a` in place or returns a new automaton (and deletes
> `a`) must be confirmed against upstream SFST `interface.C` before
> re-implementation.

> [spec:hfst:def:interface.sfst.interface.in-range-fn]
> bool in_range( unsigned int c, Range *r )

> [spec:hfst:sem:interface.sfst.interface.in-range-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes an `unsigned int c` (a character
> code) and a `Range *r` (a linked list of `Character` values) and
> returns a `bool`. It walks the `r` list via `next`, returning true as
> soon as a node's `character` equals `c`, and false if the end of the
> list is reached without a match (an empty/NULL range yields false) — a
> simple membership test of a character in a range. Exact comparison
> (raw code vs. mapped Character) must be confirmed against upstream SFST
> `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.interface-fn]
> Interface( bool utf8=false, bool verbose=false )

> [spec:hfst:sem:interface.sfst.interface.interface-fn]
> Inline constructor (defined in the header). Signature
> `Interface(bool utf8=false, bool verbose=false)`. It initializes the
> member `Verbose` from the `verbose` argument, sets `Alphabet_Defined`
> to `false` and `LexiconComments` to `false` via the member-initializer
> list, and in the body sets `TheAlphabet.utf8 = utf8` (recording whether
> the alphabet operates in UTF-8 mode). The maps/sets `VM`, `SVM`, `RS`,
> `RSS` and `TheAlphabet` are otherwise default-constructed (empty). No
> other side effects.

> [spec:hfst:def:interface.sfst.interface.left-context-fn]
> Transducer *left_context( Transducer *t, Character m1, Character m2 )

> [spec:hfst:sem:interface.sfst.interface.left-context-fn]
> Private member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes a `Transducer *t` (the left context
> pattern) and two boundary marker characters `m1` and `m2`, and returns
> a `Transducer*`. It compiles `t` into a left-context constraint
> transducer for context/replace-rule construction, marking with `m1`/`m2`
> the positions where the left context is required to have matched, so the
> rule can be intersected against arbitrary input. Exact marker semantics
> and pi-machine combination must be confirmed against upstream SFST
> `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.lower-level-fn]
> Transducer *lower_level( Transducer *a )

> [spec:hfst:sem:interface.sfst.interface.lower-level-fn]
> Member function declared in interface.h (one of the functions that
> "delete their argument automata"); body resides in the non-vendored
> SFST `interface.C`, so it cannot be read in this checkout. Contract from
> signature/name: takes a `Transducer *a` and returns a `Transducer*`. It
> projects `a` onto its lower (input) level: it produces an automaton in
> which every label pair x:y is replaced by y:y (the lower symbol on both
> sides), so the result accepts exactly the lower-side language of `a`,
> and frees the input `a`. Exact identity/epsilon handling must be
> confirmed against upstream SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.ltstr]
> struct ltstr

> [spec:hfst:def:interface.sfst.interface.ltstr.operator-fn]
> bool operator()(const char* s1, const char* s2) const

> [spec:hfst:sem:interface.sfst.interface.ltstr.operator-fn]
> Inline comparator (defined in the header) for the `ltstr` functor used
> as the ordering predicate of the `char*`-keyed `set`/`map` types
> (`RVarSet`, `VarMap`, `SVarMap`). Signature
> `bool operator()(const char* s1, const char* s2) const`. It returns
> `strcmp(s1, s2) < 0`, i.e. true iff `s1` sorts strictly before `s2` in C
> lexicographic (byte) order — giving a strict weak ordering over the
> C-string keys so the containers compare by string contents rather than
> by pointer identity.

> [spec:hfst:def:interface.sfst.interface.make-context-fn]
> Contexts *make_context( Transducer *l, Transducer *r )

> [spec:hfst:sem:interface.sfst.interface.make-context-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes a left-context transducer `l` and a
> right-context transducer `r` and returns a `Contexts*`. It allocates one
> new `contexts_t` node, sets its `left` field to `l` and `right` field to
> `r`, sets `next` to NULL, and returns it — wrapping a single left/right
> context pair into a one-element context list. Whether NULL `l`/`r` are
> normalised (e.g. to an empty/universal context) must be confirmed
> against upstream SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.make-mapping-fn]
> Transducer *make_mapping( Ranges*, Ranges* )

> [spec:hfst:sem:interface.sfst.interface.make-mapping-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes two `Ranges*` lists (the lower-side
> sequence of ranges and the upper-side sequence of ranges) and returns a
> `Transducer*`. It builds a transducer accepting the sequence of label
> pairs formed by aligning the two `Ranges` lists position by position:
> the i-th lower range mapped against the i-th upper range, producing a
> linear (single-path-per-symbol) machine for a multi-character mapping
> like `abc:xyz`. Exact handling of length mismatches between the two
> lists and of epsilon padding must be confirmed against upstream SFST
> `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.make-optional-fn]
> Transducer *make_optional( Transducer *t, Repl_Type type )

> [spec:hfst:sem:interface.sfst.interface.make-optional-fn]
> Private member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes a `Transducer *t` and a `Repl_Type
> type`, and returns a `Transducer*`. Used in replace-rule compilation, it
> makes the replacement encoded by `t` optional for the given replacement
> direction `type` (one of `repl_left`, `repl_right`, `repl_up`,
> `my_repl_down`, `repl_down`): it unions `t` with the identity/unchanged
> alternative so that the rule may either apply or leave the matched
> material untouched, with the direction `type` selecting how the optional
> alternative is constructed. Exact construction per `Repl_Type` and which
> operands are deleted must be confirmed against upstream SFST
> `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.make-rule-fn]
> Transducer *make_rule( Transducer *lc, Range *r1, Twol_Type type,

> [spec:hfst:sem:interface.sfst.interface.make-rule-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes `Transducer *lc` (left context),
> `Range *r1` (lower character range), `Twol_Type type` (one of
> `twol_left`, `twol_right`, `twol_both`), `Range *r2` (upper character
> range), and `Transducer *rc` (right context); returns a `Transducer*`.
> It compiles a single two-level (Koskenniemi) rule of the form
> `lc r1:r2 rc` for the given arrow `type`: it dispatches on `type` to
> build the left-restriction (`twol_left`), right-coercion (`twol_right`),
> or both (`twol_both`, conjoining the two) constraint transducer that
> permits/requires the correspondence `r1:r2` exactly in the context
> `lc ___ rc`. It delegates to `twol_left_rule`/`twol_right_rule` and
> combines via conjunction as needed. Exact dispatch, conjunction order
> and operand deletion must be confirmed against upstream SFST
> `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.make-transducer-fn]
> Transducer *make_transducer( Range *r1, Range *r2 )

> [spec:hfst:sem:interface.sfst.interface.make-transducer-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes two `Range*` lists `r1` (lower side)
> and `r2` (upper side) and returns a `Transducer*`. It builds a single
> one-transition transducer accepting the symbol-pair set formed by pairing
> the lower-side character range `r1` against the upper-side character
> range `r2` (the elementary `r1:r2` correspondence used as a grammar
> building block); a NULL range typically denotes the any-symbol/identity
> set. This is the public counterpart of the private `cp`. Exact pairing
> rule (cross-product vs. positional) and NULL-range handling must be
> confirmed against upstream SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.marker-transducer-fn]
> Transducer *marker_transducer( Transducer *t, Contexts *c,

> [spec:hfst:sem:interface.sfst.interface.marker-transducer-fn]
> Private member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes a `Transducer *t`, a `Contexts *c`
> (left/right context list), and a `Character &marker` (by reference, so it
> can allocate/return the marker symbol it uses), and returns a
> `Transducer*`. It constructs a marker-insertion transducer for context/
> replace-rule compilation: it picks (or allocates into `marker`) a fresh
> boundary marker symbol and builds the machine that inserts that marker at
> the context boundaries described by `c` around `t`, so later
> intersections can recognise where contexts begin/end. Exact marker
> allocation and per-context construction must be confirmed against
> upstream SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.minimise-fn]
> Transducer *minimise( Transducer *a )

> [spec:hfst:sem:interface.sfst.interface.minimise-fn]
> Member function declared in interface.h (one of the functions that
> "delete their argument automata"); body resides in the non-vendored SFST
> `interface.C`, so it cannot be read in this checkout. Contract from
> signature/name: takes a `Transducer *a` and returns a `Transducer*`. It
> determinises and minimises `a`, returning a new transducer recognising
> the same relation with the minimal number of states, and frees the input
> `a`. Exact minimisation algorithm and whether the alphabet is copied
> across must be confirmed against upstream SFST `interface.C` before
> re-implementation.

> [spec:hfst:def:interface.sfst.interface.negation-fn]
> Transducer *negation( Transducer *a )

> [spec:hfst:sem:interface.sfst.interface.negation-fn]
> Member function declared in interface.h (one of the functions that
> "delete their argument automata"); body resides in the non-vendored SFST
> `interface.C`, so it cannot be read in this checkout. Contract from
> signature/name: takes a `Transducer *a` and returns a `Transducer*`. It
> computes the complement of `a` with respect to the declared alphabet
> sigma: the transducer accepting exactly the strings (label sequences over
> the alphabet) NOT accepted by `a` (typically by determinising,
> completing against the pi/sigma language, and swapping final/non-final
> states), and frees the input `a`. Exact alphabet/universe used for the
> complement must be confirmed against upstream SFST `interface.C` before
> re-implementation.

> [spec:hfst:def:interface.sfst.interface.new-transducer-fn]
> Transducer *new_transducer( Range*, Range* )

> [spec:hfst:sem:interface.sfst.interface.new-transducer-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes two `Range*` lists (lower side and
> upper side) and returns a `Transducer*`. It creates a fresh elementary
> transducer for the symbol-pair correspondence between the two ranges —
> effectively the public front for building a single `r1:r2` transition
> (delegating to `cp`/`make_transducer`) and registering any new symbols
> into `TheAlphabet`. Exact relationship to `make_transducer`, pairing rule
> and NULL-range (any-symbol) handling must be confirmed against upstream
> SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.one-label-transducer-fn]
> Transducer *one_label_transducer( Label l )

> [spec:hfst:sem:interface.sfst.interface.one-label-transducer-fn]
> Private member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes a single `Label l` (a lower:upper
> symbol pair) and returns a `Transducer*`. It allocates a minimal
> two-state transducer with one transition from the (non-final) root state
> on label `l` to a single final state, i.e. a machine accepting exactly
> the one-symbol-pair string `l`. Whether the label's symbols are
> registered into the alphabet must be confirmed against upstream SFST
> `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.optional-fn]
> Transducer *optional( Transducer *a )

> [spec:hfst:sem:interface.sfst.interface.optional-fn]
> Member function declared in interface.h (one of the functions that
> "delete their argument automata"); body resides in the non-vendored SFST
> `interface.C`, so it cannot be read in this checkout. Contract from
> signature/name: takes a `Transducer *a` and returns a `Transducer*`. It
> makes `a` optional — the `(a)?` / `a | epsilon` construction — returning
> a transducer that accepts either a string of `a` or the empty string
> (achieved by marking the root state final or unioning with an
> epsilon-accepting machine), and frees the input `a`. Exact construction
> (root made final vs. union) must be confirmed against upstream SFST
> `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.pi-machine-fn]
> Transducer *pi_machine( Alphabet &alph )

> [spec:hfst:sem:interface.sfst.interface.pi-machine-fn]
> Private member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes a reference `Alphabet &alph` and
> returns a `Transducer*`. It builds the "pi" machine: a single-state
> transducer whose root is final and which has a self-loop transition for
> every symbol(pair) of `alph` (delegating to `add_pi_transitions`), so it
> accepts any sequence of symbols over the alphabet (the universal/sigma-
> star language used as surrounding material in replace/context rules).
> Exact label set looped (all labels vs. identity pairs) must be confirmed
> against upstream SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.r-var-set]
> typedef set<char*, ltstr> RVarSet

> [spec:hfst:def:interface.sfst.interface.read-transducer-fn]
> Transducer *read_transducer( char *filename )

> [spec:hfst:sem:interface.sfst.interface.read-transducer-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes a `char *filename` and returns a
> `Transducer*`. It opens the named file, reads a previously stored SFST
> transducer from it (via the `Transducer` file-reading constructor),
> merges that transducer's alphabet into `TheAlphabet`, and returns the
> loaded transducer. On failure to open or parse the file it reports a
> fatal error (via `error`) and aborts. Exact error handling, stream mode
> (binary), and alphabet-merge behaviour must be confirmed against upstream
> SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.read-words-fn]
> Transducer *read_words( char *filename )

> [spec:hfst:sem:interface.sfst.interface.read-words-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes a `char *filename` and returns a
> `Transducer*`. It opens the named lexicon/word-list file, reads it line
> by line, tokenises each line into symbols against `TheAlphabet` (honouring
> the `LexiconComments` flag to skip comment lines when enabled), builds a
> transducer accepting the disjunction of all the listed words, and returns
> it. On failure to open the file it reports a fatal error (via `error`).
> Exact tokenisation, comment syntax, and how the per-word automata are
> combined must be confirmed against upstream SFST `interface.C` before
> re-implementation.

> [spec:hfst:def:interface.sfst.interface.repetition-fn]
> Transducer *repetition( Transducer *a )

> [spec:hfst:sem:interface.sfst.interface.repetition-fn]
> Member function declared in interface.h (one of the functions that
> "delete their argument automata"); body resides in the non-vendored SFST
> `interface.C`, so it cannot be read in this checkout. Contract from
> signature/name: takes a `Transducer *a` and returns a `Transducer*`. It
> computes the Kleene star `a*` — the transducer accepting zero or more
> concatenated repetitions of `a` (including the empty string) — and frees
> the input `a`. Exact construction (back-epsilon from finals to start plus
> making start final) must be confirmed against upstream SFST `interface.C`
> before re-implementation.

> [spec:hfst:def:interface.sfst.interface.repetition2-fn]
> Transducer *repetition2( Transducer *a )

> [spec:hfst:sem:interface.sfst.interface.repetition2-fn]
> Member function declared in interface.h (one of the functions that
> "delete their argument automata"); body resides in the non-vendored SFST
> `interface.C`, so it cannot be read in this checkout. Contract from
> signature/name: takes a `Transducer *a` and returns a `Transducer*`. It
> computes the Kleene plus `a+` — the transducer accepting one or more
> concatenated repetitions of `a` (excluding the empty string, unless `a`
> already accepts it) — and frees the input `a`. Exact construction (back-
> epsilon from finals to start, start NOT made final) must be confirmed
> against upstream SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.replace-fn]
> Transducer *replace( Transducer *a, Repl_Type type, bool optional )

> [spec:hfst:sem:interface.sfst.interface.replace-fn]
> Member function declared in interface.h (one of the functions that
> "delete their argument automata"); body resides in the non-vendored SFST
> `interface.C`, so it cannot be read in this checkout. Contract from
> signature/name: takes a `Transducer *a` (the center/mapping), a
> `Repl_Type type` (replace direction: `repl_left`, `repl_right`,
> `repl_up`, `my_repl_down`, `repl_down`), and a `bool optional`; returns a
> a `Transducer*`. It compiles a context-free replace rule (no explicit
> context) for `a` in direction `type`, building the marker-based machine
> that performs the replacement throughout arbitrary surrounding text; when
> `optional` is true the replacement may be skipped (built via
> `make_optional`). It uses the private helpers (`pi_machine`,
> `marker_transducer`, `replace_transducer`, etc.) and frees `a`. Exact
> per-`type` construction and operand deletion must be confirmed against
> upstream SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.replace-in-context-fn]
> Transducer *replace_in_context( Transducer *a, Repl_Type type,

> [spec:hfst:sem:interface.sfst.interface.replace-in-context-fn]
> Member function declared in interface.h (one of the functions that
> "delete their argument automata"); body resides in the non-vendored SFST
> `interface.C`, so it cannot be read in this checkout. Contract from
> signature/name: takes a `Transducer *a` (center/mapping), a `Repl_Type
> type`, a `Contexts *c` (left/right context list), and a `bool optional`;
> returns a `Transducer*`. It compiles a contextual replace rule: it
> performs the replacement encoded by `a` in direction `type` but only
> where the surrounding text matches a context in `c`, building the
> marker/center/context transducers (via `marker_transducer`,
> `center_transducer`, `context_transducer`, `constrain_boundary_transducer`)
> and intersecting them; `optional` allows the rule not to fire. It frees
> `a` (and consumes `c`). Exact per-`type`/per-context construction and
> deletion must be confirmed against upstream SFST `interface.C` before
> re-implementation.

> [spec:hfst:def:interface.sfst.interface.replace-transducer-fn]
> Transducer *replace_transducer( Transducer *ct, Character lm,

> [spec:hfst:sem:interface.sfst.interface.replace-transducer-fn]
> Private member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes a `Transducer *ct` (a center/marked
> transducer), two boundary marker characters `lm` (left) and `rm`
> (right), and a `Repl_Type type`; returns a `Transducer*`. It builds the
> core replacement transducer for a replace rule by wrapping `ct` with the
> left/right markers `lm`/`rm` according to the replacement direction
> `type`, producing the machine that maps the marked center to its
> replacement within marker boundaries. Exact marker placement per
> `Repl_Type` and operand deletion must be confirmed against upstream SFST
> `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.restriction-fn]
> Transducer *restriction( Transducer *a, Twol_Type type, Contexts *c, int )

> [spec:hfst:sem:interface.sfst.interface.restriction-fn]
> Member function declared in interface.h (one of the functions that
> "delete their argument automata"); body resides in the non-vendored SFST
> `interface.C`, so it cannot be read in this checkout. Contract from
> signature/name: takes a `Transducer *a` (the center pattern), a
> `Twol_Type type`, a `Contexts *c` (left/right context list), and an
> unnamed `int` (a direction/level flag). Returns a `Transducer*`. It
> compiles the two-level restriction operator `a => c` (restrict the
> occurrences of `a` to exactly the contexts listed in `c`), dispatching on
> `type` for the arrow direction and using the private context/marker/
> restriction helpers; the `int` parameter selects which level the
> restriction applies to. It frees `a` and consumes `c`. Exact per-`type`
> construction, meaning of the `int`, and operand deletion must be
> confirmed against upstream SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.restriction-transducer-fn]
> Transducer *restriction_transducer( Transducer *l1, Transducer *l2,

> [spec:hfst:sem:interface.sfst.interface.restriction-transducer-fn]
> Private member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes two `Transducer*` `l1` and `l2` and a
> `Character marker`, and returns a `Transducer*`. It builds the auxiliary
> transducer enforcing a two-level restriction: using the inserted boundary
> `marker`, it constrains positions where `l1` (the marked center context)
> occurs so that the surrounding language `l2` (the permitted context) must
> hold, producing the constraint machine intersected during restriction
> compilation. Exact accepted language and operand deletion must be
> confirmed against upstream SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.result-fn]
> Transducer *result( Transducer*, bool )

> [spec:hfst:sem:interface.sfst.interface.result-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes a `Transducer*` and a `bool` flag,
> and returns a `Transducer*`. It finalises a compiled grammar result for
> output: it attaches/sets the alphabet (`TheAlphabet`) on the transducer
> and, depending on the `bool` flag (e.g. "minimise"/"switch levels"),
> optionally minimises or otherwise post-processes it before it is written
> out, returning the prepared transducer. Exact meaning of the bool and
> which post-processing it triggers must be confirmed against upstream SFST
> `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.result-transducer-fn]
> Transducer *result_transducer( Transducer *l1, Transducer *l2,

> [spec:hfst:sem:interface.sfst.interface.result-transducer-fn]
> Private member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes two `Transducer*` `l1` and `l2`, a
> `Twol_Type type`, and a `Character marker`, and returns a `Transducer*`.
> It assembles the final two-level rule transducer from its left-restriction
> part `l1` and right-coercion part `l2`, combining them according to the
> arrow `type` (`twol_left`/`twol_right`/`twol_both`) and removing the
> auxiliary `marker` symbol, yielding the clean rule transducer over the
> declared alphabet. Exact combination per `type`, marker removal and
> operand deletion must be confirmed against upstream SFST `interface.C`
> before re-implementation.

> [spec:hfst:def:interface.sfst.interface.rsvar-value-fn]
> Range *rsvar_value( char *name )

> [spec:hfst:sem:interface.sfst.interface.rsvar-value-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes a `char *name` and returns a
> `Range*`. It is the range-valued agreement-variable accessor: it records
> `name` in the `RSS` set (the set of referenced range-agreement variable
> names, marking it as used in the current rule) and returns the range
> value associated with it (an empty/placeholder `Range` that will later be
> instantiated per agreement value during rule expansion). Exact value
> returned and the role of `RSS` vs. `SVM` must be confirmed against
> upstream SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.rvar-value-fn]
> Transducer *rvar_value( char *name )

> [spec:hfst:sem:interface.sfst.interface.rvar-value-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes a `char *name` and returns a
> `Transducer*`. It is the transducer-valued agreement-variable accessor:
> it records `name` in the `RS` set (marking the R-variable as referenced
> in the current rule, so the rule will be expanded once per agreement
> binding) and returns the transducer currently bound to `name` (looked up
> in `VM`, copied if needed). If `name` is undefined it reports a fatal
> error (via `error2` with `name`). Exact lookup source, copy/free
> behaviour and undefined-name handling must be confirmed against upstream
> SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.s-var-map]
> typedef map<char*, Range*, ltstr> SVarMap

> [spec:hfst:def:interface.sfst.interface.subtraction-fn]
> Transducer *subtraction( Transducer *a1, Transducer *a2 )

> [spec:hfst:sem:interface.sfst.interface.subtraction-fn]
> Member function declared in interface.h (one of the functions that
> "delete their argument automata"); body resides in the non-vendored SFST
> `interface.C`, so it cannot be read in this checkout. Contract from
> signature/name: takes two `Transducer*` `a1` and `a2` and returns a
> `Transducer*` that is their difference `a1 - a2` — accepting exactly the
> strings (pairs) accepted by `a1` but NOT by `a2`. Per the header note it
> consumes/frees both argument automata and returns a freshly allocated
> result. Exact alphabet harmonisation and whether the result is minimised
> must be confirmed against upstream SFST `interface.C` before
> re-implementation.

> [spec:hfst:def:interface.sfst.interface.svar-value-fn]
> Range *svar_value( char *name )

> [spec:hfst:sem:interface.sfst.interface.svar-value-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes a `char *name` and returns a
> `Range*`. It looks up the set/range variable `name` in the `SVM`
> (`SVarMap`) and returns the `Range*` bound to it (typically a copy of the
> stored range). If `name` is not defined it reports a fatal error (via
> `error2` with `name`) and aborts. Exact copy-vs-shared return and
> undefined-name handling must be confirmed against upstream SFST
> `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.switch-levels-fn]
> Transducer *switch_levels( Transducer *a )

> [spec:hfst:sem:interface.sfst.interface.switch-levels-fn]
> Member function declared in interface.h (one of the functions that
> "delete their argument automata"); body resides in the non-vendored SFST
> `interface.C`, so it cannot be read in this checkout. Contract from
> signature/name: takes a `Transducer *a` and returns a `Transducer*`. It
> swaps the two levels of every label in `a`: each transition label pair
> x:y becomes y:x, producing the inverse relation of `a` (lower side and
> upper side exchanged), and frees the input `a`. Exact handling of
> identity/unknown labels must be confirmed against upstream SFST
> `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.symbol-code-fn]
> Character symbol_code( char *s )

> [spec:hfst:sem:interface.sfst.interface.symbol-code-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes a `char *s` (a multi-character/named
> symbol spelling, e.g. a bracketed `<symbol>`) and returns a `Character`.
> It interns the named symbol `s` into `TheAlphabet`, allocating a new
> symbol id if it is not already present, and returns its `Character` code,
> so a named/multi-character symbol used in the grammar gets a stable
> alphabet code. Contrast with `character_code`, which handles single
> Unicode code points. Exact interaction with the alphabet's symbol table
> and whether `s` is consumed must be confirmed against upstream SFST
> `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.twol-left-rule-fn]
> Transducer *twol_left_rule( Transducer *lc, Range *lower_range,

> [spec:hfst:sem:interface.sfst.interface.twol-left-rule-fn]
> Private member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes `Transducer *lc` (left context),
> `Range *lower_range`, `Range *upper_range`, and `Transducer *rc` (right
> context); returns a `Transducer*`. It builds the left-arrow (`<=`)
> two-level constraint for the correspondence `lower_range:upper_range` in
> the context `lc ___ rc`: it requires that whenever the lower symbol
> appears in that context, it must correspond to a member of
> `upper_range` (coercion of the surface realisation). It uses `cp`/
> `anti_cp`, `pi_machine` and `restriction_transducer`. Exact constraint
> construction and operand deletion must be confirmed against upstream SFST
> `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.twol-right-rule-fn]
> Transducer *twol_right_rule( Transducer *lc, Range *lower_range,

> [spec:hfst:sem:interface.sfst.interface.twol-right-rule-fn]
> Private member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes `Transducer *lc` (left context),
> `Range *lower_range`, `Range *upper_range`, and `Transducer *rc` (right
> context); returns a `Transducer*`. It builds the right-arrow (`=>`)
> two-level constraint for the correspondence `lower_range:upper_range` in
> the context `lc ___ rc`: it restricts the correspondence so that it may
> occur ONLY in that context (the surface pair is permitted nowhere else).
> It uses `cp`/`anti_cp`, `pi_machine` and `restriction_transducer`. Exact
> constraint construction and operand deletion must be confirmed against
> upstream SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.upper-level-fn]
> Transducer *upper_level( Transducer *a )

> [spec:hfst:sem:interface.sfst.interface.upper-level-fn]
> Member function declared in interface.h (one of the functions that
> "delete their argument automata"); body resides in the non-vendored SFST
> `interface.C`, so it cannot be read in this checkout. Contract from
> signature/name: takes a `Transducer *a` and returns a `Transducer*`. It
> projects `a` onto its upper (output) level: every label pair x:y is
> replaced by x:x (the upper symbol on both sides), so the result accepts
> exactly the upper-side language of `a`, and frees the input `a`. This is
> the mirror of `lower_level`. Exact identity/epsilon handling must be
> confirmed against upstream SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.var-map]
> typedef map<char*, Transducer*, ltstr> VarMap

> [spec:hfst:def:interface.sfst.interface.var-value-fn]
> Transducer *var_value( char *name )

> [spec:hfst:sem:interface.sfst.interface.var-value-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes a `char *name` and returns a
> `Transducer*`. It looks up the transducer variable `name` in `VM`
> (`VarMap`) and returns the bound transducer (typically a fresh copy so
> the stored definition is not consumed by the caller). If `name` is not
> defined it reports a fatal error (via `error2` with `name`) and aborts.
> Exact copy-vs-shared return and undefined-name handling must be confirmed
> against upstream SFST `interface.C` before re-implementation.

> [spec:hfst:def:interface.sfst.interface.write-to-file-fn]
> void write_to_file( Transducer*, char *filename)

> [spec:hfst:sem:interface.sfst.interface.write-to-file-fn]
> Member function declared in interface.h; body resides in the
> non-vendored SFST `interface.C`, so it cannot be read in this checkout.
> Contract from signature/name: takes a `Transducer*` and a `char
> *filename`, and returns `void`. It opens `filename` for (binary) writing
> and serialises the given transducer to it using the SFST `Transducer`
> store/write routine, so the compiled automaton can later be reloaded via
> `read_transducer`. On failure to open the file it reports a fatal error
> (via `error`). Exact file format/mode and whether the alphabet is written
> must be confirmed against upstream SFST `interface.C` before
> re-implementation.

> [spec:hfst:def:interface.sfst.range]
> typedef struct range_t

> [spec:hfst:def:interface.sfst.range-t]
> struct range_t {
>   Character character;
>   struct range_t *next;
> }

> [spec:hfst:def:interface.sfst.ranges]
> typedef struct ranges_t

> [spec:hfst:def:interface.sfst.ranges-t]
> struct ranges_t {
>   Range *range;
>   struct ranges_t *next;
> }

> [spec:hfst:def:interface.sfst.repl-type]
> typedef enum

> [spec:hfst:def:interface.sfst.twol-type]
> typedef enum

