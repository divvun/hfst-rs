# libhfst/src/parsers/rule_src/OtherSymbolTransducer.cc, libhfst/src/parsers/rule_src/OtherSymbolTransducer.h

> [spec:hfst:def:other-symbol-transducer.another-copy-fn]
> OtherSymbolTransducer another_copy(another)

> [spec:hfst:sem:other-symbol-transducer.another-copy-fn]
> This is the local-variable construction `OtherSymbolTransducer another_copy(another);`
> inside the one-arg `apply` member (the `HfstTransducerOneArgMember` overload). It
> copy-constructs a working copy of the `another` argument so the operand can be
> mutated (diacritic harmonization) without altering the caller's object. After
> construction, if the static `diacritics` set is non-empty, the code calls
> `harmonize_diacritics(another_copy)` on `*this` and then `another_copy.harmonize_diacritics(*this)`,
> making the two transducers' diacritic alphabets mutually compatible before the
> binary operation `p` is applied. The copy is just a value-semantics clone; the
> copy constructor itself copies `is_broken` and `transducer` and (with its body
> commented out) does not add a diamond transition.

> [spec:hfst:def:other-symbol-transducer.basic-fn]
> HfstBasicTransducer basic(transducer)

> [spec:hfst:sem:other-symbol-transducer.basic-fn]
> This is the local construction `HfstBasicTransducer basic(transducer);` at the top
> of `harmonize_diacritics`. It builds a mutable basic (explicit state/transition)
> representation of the receiver's compiled `transducer` so its alphabet and
> transitions can be inspected and edited. The function then: gets `basic`'s alphabet
> and the argument `t`'s alphabet (via `HfstBasicTransducer basic_t(t.transducer)`);
> computes `missing_diacritics` = the set of static `diacritics` that occur in `t`'s
> alphabet but NOT in `basic`'s alphabet; if `missing_diacritics` is empty it returns
> `*this` unchanged. Otherwise it walks every state `s` (counter starting at 0) and
> every transition; for the first transition out of a state whose input symbol equals
> `TWOLC_IDENTITY`, it adds, from `s` to that transition's target, a self-symbol
> transition `(d,d,0.0)` for each missing diacritic `d`, then `break`s out of that
> state's transition loop. After processing all states it rebuilds `transducer` as
> `HfstTransducer(basic, transducer_type)` and returns `*this`.

> [spec:hfst:def:other-symbol-transducer.empty-symbol-pair-set]
> class EmptySymbolPairSet

> [spec:hfst:def:other-symbol-transducer.have-common-string-fn]
> bool have_common_string(HfstState state1,HfstState state2,

> [spec:hfst:sem:other-symbol-transducer.have-common-string-fn]
> Free function (DFS) testing whether `fst1` (at `state1`) and `fst2` (at `state2`)
> accept a common symbol-pair string, accumulating one such string into `v`.
> Steps: (1) if both `state1` and `state2` are final states, return true. (2) Build a
> map `fst1_transition_map` from each of `state1`'s outgoing transitions' input:output
> SymbolPair to its target state (later duplicates of a pair overwrite earlier ones).
> (3) For each outgoing transition of `state2`, form its input:output SymbolPair; if
> that pair is a key in `fst1_transition_map`, form the StatePair (fst1's target for
> that pair, fst2's transition target). If that StatePair is not already in
> `visited_pairs`: push `"input:output"` onto `v`, insert the StatePair into
> `visited_pairs`, and recurse on the two targets; if the recursion returns true,
> return true; otherwise pop the just-pushed element off `v` and continue. (4) If no
> branch succeeds, return false. Mutates `visited_pairs` (additions persist even on
> backtrack) and `v` (pushes/pops to hold the current path). Returns bool.

> [spec:hfst:def:other-symbol-transducer.hfst-basic-transition-set]
> typedef std::set<HfstBasicTransition> HfstBasicTransitionSet

> [spec:hfst:def:other-symbol-transducer.hfst-transducer-hfst-transducer-bool-arg-member-const-hfst-transducer-bool]
> typedef HfstTransducer &(HfstTransducer::*HfstTransducerBoolArgMember)

> [spec:hfst:def:other-symbol-transducer.hfst-transducer-hfst-transducer-one-arg-member-bool-const-hfst-transducer]
> typedef bool (HfstTransducer::*HfstTransducerOneArgMemberBool)

> [spec:hfst:def:other-symbol-transducer.hfst-transducer-hfst-transducer-one-arg-member-const-hfst-transducer]
> typedef HfstTransducer &(HfstTransducer::*HfstTransducerOneArgMember)

> [spec:hfst:def:other-symbol-transducer.hfst-transducer-hfst-transducer-one-num-arg-member-unsigned-int]
> typedef HfstTransducer &(HfstTransducer::*HfstTransducerOneNumArgMember)

> [spec:hfst:def:other-symbol-transducer.hfst-transducer-hfst-transducer-one-symbol-pair-arg-member-const-symbol-pair]
> typedef HfstTransducer &(HfstTransducer::*HfstTransducerOneSymbolPairArgMember)

> [spec:hfst:def:other-symbol-transducer.hfst-transducer-hfst-transducer-one-symbol-pair-bool-arg-member-const-symbol-pair-bool]
> typedef HfstTransducer &(HfstTransducer::*HfstTransducerOneSymbolPairBoolArgMember)

> [spec:hfst:def:other-symbol-transducer.hfst-transducer-hfst-transducer-subst-member-const-std-string-const-std-string-bool-bool]
> typedef HfstTransducer &(HfstTransducer::*HfstTransducerSubstMember)

> [spec:hfst:def:other-symbol-transducer.hfst-transducer-hfst-transducer-subst-pair-fst-member-const-symbol-pair-hfst-transducer-bool]
> typedef HfstTransducer &(HfstTransducer::*HfstTransducerSubstPairFstMember)

> [spec:hfst:def:other-symbol-transducer.hfst-transducer-hfst-transducer-subst-pair-member-const-symbol-pair-const-symbol-pair]
> typedef HfstTransducer &(HfstTransducer::*HfstTransducerSubstPairMember)

> [spec:hfst:def:other-symbol-transducer.hfst-transducer-hfst-transducer-two-num-arg-member-unsigned-int-unsigned-int]
> typedef HfstTransducer &(HfstTransducer::*HfstTransducerTwoNumArgMember)

> [spec:hfst:def:other-symbol-transducer.hfst-transducer-hfst-transducer-zero-arg-member-void]
> typedef HfstTransducer &(HfstTransducer::*HfstTransducerZeroArgMember) (void)

> [spec:hfst:def:other-symbol-transducer.main-fn]
> int main(void)

> [spec:hfst:sem:other-symbol-transducer.main-fn]
> Test-build entry point, compiled only when `TEST_OTHER_SYMBOL_TRANSDUCER` is
> defined. It selects an `ImplementationType`: TROPICAL_OPENFST_TYPE if HAVE_OPENFST,
> else SFST_TYPE if HAVE_SFST, else FOMA_TYPE if HAVE_FOMA, else ERROR_TYPE (each
> guarded by the corresponding compile-time macro). It calls
> `OtherSymbolTransducer::set_transducer_type(transducer_type)`. It builds a
> `HandySet<SymbolPair>` containing the pairs ("a","b"), ("c","d"), ("a","d") and
> passes it to `set_symbol_pairs`. It constructs six OtherSymbolTransducers:
> ost1 = (TWOLC_UNKNOWN,TWOLC_UNKNOWN), ost2 = ("c",TWOLC_UNKNOWN),
> ost3 = (TWOLC_UNKNOWN,"b"), ost4 = ("c","d"), ost5 = ("a","b"), ost6 = ("a","d").
> It then chains `ost1.apply(&HfstTransducer::concatenate, ost2)` followed by the same
> concatenate-apply with ost3, ost4, ost5, ost6 in sequence (each returning ost1 by
> reference). Finally it retrieves `ost1.get_transducer()` into `ost1_t` (the
> printout is commented out). Returns implicitly (no explicit return). Used purely as
> a smoke test of the symbol-pair/apply machinery.

> [spec:hfst:def:other-symbol-transducer.name-to-regex-map]
> typedef std::map<std::string,OtherSymbolTransducer> NameToRegexMap

> [spec:hfst:def:other-symbol-transducer.other-symbol-transducer]
> class OtherSymbolTransducer {
>   static HandySet<std::string> input_symbols;
>   static HandySet<std::string> output_symbols;
>   static HandySet<std::string> diacritics;
>   static HandySet<SymbolPair> symbol_pairs;
>   static ImplementationType transducer_type;
>   bool is_broken;
>   HfstTransducer transducer;
>   OtherSymbolTransducer &operator=(const OtherSymbolTransducer &another);
>   OtherSymbolTransducer &apply(HfstTransducerZeroArgMember p);
>   OtherSymbolTransducer &apply(const HfstTransducerOneArgMember, const OtherSymbolTransducer &another);
>   OtherSymbolTransducer &apply(const HfstTransducerBoolArgMember, const OtherSymbolTransducer &another);
>   OtherSymbolTransducer &apply(const HfstTransducerOneNumArgMember,unsigned int number);
>   OtherSymbolTransducer &apply(const HfstTransducerTwoNumArgMember,unsigned int num1, unsigned int num2);
>   OtherSymbolTransducer &apply (const HfstTransducerOneSymbolPairArgMember,const SymbolPair &pair);
>   OtherSymbolTransducer &apply (const HfstTransducerOneSymbolPairBoolArgMember,const SymbolPair &pair, bool b);
>   OtherSymbolTransducer &apply(const HfstTransducerSubstMember p,const std::string &str1, const std::string &str2, bool b1, bool b2);
>   OtherSymbolTransducer &apply(const HfstTransducerSubstPairMember p,const SymbolPair &p1, const SymbolPair &p2);
>   OtherSymbolTransducer &apply(const HfstTransducerSubstPairFstMember p,const SymbolPair &p1, const OtherSymbolTransducer &t, bool b);
>   OtherSymbolTransducer &add_info_symbol(const std::string &info_symbol);
>   OtherSymbolTransducer &harmonize_diacritics(OtherSymbolTransducer &t);
>   OtherSymbolTransducer &contained(void);
>   OtherSymbolTransducer &contained_once(void);
>   OtherSymbolTransducer &negated(void);
>   OtherSymbolTransducer &term_complemented(void);
> }

> [spec:hfst:def:other-symbol-transducer.other-symbol-transducer-vector]
> typedef std::vector<OtherSymbolTransducer> OtherSymbolTransducerVector

> [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.add-diamond-transition-fn]
> void OtherSymbolTransducer::add_diamond_transition(void)

> [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.add-diamond-transition-fn]
> One-liner: calls `add_symbol_to_alphabet(TWOLC_DIAMOND)`, i.e. it ensures the
> `TWOLC_DIAMOND` symbol is present in the receiver transducer's alphabet (without
> adding any actual transition). No return value.

> [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.add-symbol-to-alphabet-fn]
> void OtherSymbolTransducer::add_symbol_to_alphabet(const std::string &symbol)

> [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.add-symbol-to-alphabet-fn]
> Builds a mutable basic transducer copy of the receiver's `transducer`
> (`HfstBasicTransducer mutable_transducer(transducer)`), calls
> `mutable_transducer.add_symbol_to_alphabet(symbol)` to register `symbol` in its
> alphabet, then rebuilds and reassigns `transducer = HfstTransducer(mutable_transducer,
> transducer_type)`. Net effect: adds `symbol` to the transducer's alphabet. No return
> value.

> [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.add-transition-fn]
> void OtherSymbolTransducer::add_transition

> [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.add-transition-fn]
> Static helper. Adds a single transition to the given mutable basic transducer
> `center_t`: calls `center_t.add_transition(source_state, HfstBasicTransition(
> target_state, input, output, 0.0))`, i.e. an arc from `source_state` to
> `target_state` labeled `input:output` with weight 0.0. No return value.

> [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.apply-fn]
> bool OtherSymbolTransducer::apply

> [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.apply-fn]
> The `const` boolean-returning `apply` overload taking an
> `HfstTransducerOneArgMemberBool p` (a predicate member like `compare`) and another
> OtherSymbolTransducer. Validation first: if the static `symbol_pairs` set is empty,
> throw `EmptySymbolPairSet`; if `this->is_broken`, throw `UndefinedSymbolPairsFound`;
> if `another.is_broken`, throw `UndefinedSymbolPairsFound`. Then it copy-constructs
> `copy(*this)` and `another_copy(another)` (so neither operand's `transducer` is
> mutated — note this overload does NOT harmonize diacritics), and returns
> `(copy.transducer.*p)(another_copy.transducer)`, i.e. invokes the bool-returning
> member `p` on the copy's transducer with the other copy's transducer as argument.

> [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.check-pair-fn]
> void OtherSymbolTransducer::check_pair(const std::string &input_symbol,

> [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.check-pair-fn]
> Validates the pair (`input_symbol`,`output_symbol`) against the declared alphabet,
> setting the member `is_broken` accordingly. Cascading if/else, first match wins:
> (1) input == TWOLC_IDENTITY -> valid (is_broken=false). (2) input==HFST_UNKNOWN
> AND output==HFST_UNKNOWN (other:other) -> valid. (3) input==TWOLC_EPSILON AND
> output==TWOLC_EPSILON -> valid. (4) input==HFST_EPSILON AND output==HFST_EPSILON
> -> valid. (5) input==TWOLC_DIAMOND -> valid. (6) input==HFST_UNKNOWN (other:X):
> broken unless output==TWOLC_EPSILON or output is in `output_symbols`. (7)
> output==HFST_UNKNOWN (X:other): broken unless input==TWOLC_EPSILON or input is in
> `input_symbols`. (8) input==TWOLC_EPSILON (0:X): broken unless output is in
> `output_symbols`. (9) output==TWOLC_EPSILON (X:0): broken unless input is in
> `input_symbols`. (10) input is in `diacritics` -> valid. (11) otherwise: broken
> unless the SymbolPair(input,output) is in the static `symbol_pairs` set. Finally,
> if `is_broken` is true, prints `"Unknown pair: <input> <output>\n"` to std::cerr.
> No return value; only mutates `is_broken` and may write to stderr.

> [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.define-diacritics-fn]
> void OtherSymbolTransducer::define_diacritics

> [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.define-diacritics-fn]
> Static. Replaces the static `diacritics` set with the contents of the passed
> `diacritics` vector: clears `OtherSymbolTransducer::diacritics`, then inserts all
> elements of the vector. Then, for each diacritic `d` now in the static set, it
> removes `d`'s footprint from the other static alphabets: erases SymbolPair(d,d) and
> SymbolPair(d,TWOLC_EPSILON) from `symbol_pairs`, and erases `d` from `input_symbols`
> and from `output_symbols`. No return value. (Diacritics are thereby tracked
> separately and excluded from the ordinary symbol-pair/alphabet sets.)

> [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.empty-fn]
> bool OtherSymbolTransducer::empty(const HfstBasicTransducer &fsm)

> [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.empty-fn]
> Static. Tests whether the basic transducer `fsm` accepts the empty language by
> checking for reachable final states (it actually iterates over all states). It
> walks every state (a counter `state` incremented from 0 in lockstep with the
> state iterator); if any state is a final state it returns false immediately.
> If no state is final, returns true. Note: it does not consider reachability or
> accepting paths beyond final-state presence.

> [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.get-context-fn]
> OtherSymbolTransducer OtherSymbolTransducer::get_context

> [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.get-context-fn]
> Static. Builds the context transducer "UNIVERSAL* left DIAMOND UNIVERSAL* DIAMOND
> right UNIVERSAL*" wrapping `left` and `right` between diamonds. Steps: construct
> `universal = get_universal()` and apply `repeat_star` to it (so it becomes
> UNIVERSAL*). Copy `result(universal)`. Construct `diamond(TWOLC_DIAMOND)`. Apply
> `repeat_star` to `universal` a second time (idempotent). Then return the chained
> result of concatenating, in order, onto `result`: `left`, then `diamond`, then
> `universal`, then `diamond`, then `right`, then `universal` (each call is
> `apply(&HfstTransducer::concatenate, X)` returning `result` by reference, so the
> final returned value is `result` after all six concatenations). Returns an
> OtherSymbolTransducer by value.

> [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.get-initial-transition-pairs-fn]
> void OtherSymbolTransducer::get_initial_transition_pairs

> [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.get-initial-transition-pairs-fn]
> Const. Collects the input:output symbol pairs labeling the transitions leaving the
> start state. If `is_broken`, throws `UndefinedSymbolPairsFound`. Otherwise builds a
> basic transducer copy of `this->transducer`, takes its first state (the start
> state, `fst.begin()`), and for each outgoing transition appends a
> `SymbolPair(input_symbol, output_symbol)` to the caller-supplied `pair_container`
> (out parameter, appended via push_back). No return value.

> [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.get-inverse-of-upper-projection-fn]
> OtherSymbolTransducer OtherSymbolTransducer::get_inverse_of_upper_projection

> [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.get-inverse-of-upper-projection-fn]
> Builds a transducer whose lower (output) side is expanded so each transition's
> input is paired with every output symbol that could co-occur, producing (after
> minimization) the inverse of the upper projection. If `is_broken`, throws
> `UndefinedSymbolPairsFound`. It makes a basic copy `fst` of `this->transducer` and
> an empty `new_fst`. Iterating states (counter `state` from 0): adds the state to
> `new_fst`; if `fst` marks it final, copies its final weight into `new_fst`. For each
> outgoing transition with input `i`, output `o`, target `target`:
> - If `i == HFST_UNKNOWN`: add an UNKNOWN:UNKNOWN arc state->target, and for every
>   symbol `k` in static `output_symbols` that is present in `fst`'s alphabet
>   (`has_symbol(fst,k)`), add an UNKNOWN:k arc state->target.
> - Else: add the original `i:o` arc state->(transition's target). Then for every
>   SymbolPair in static `symbol_pairs` whose first==`i` and whose second is present
>   in `fst`'s alphabet, add an `i:second` arc state->target. Additionally: if
>   `i == TWOLC_EPSILON`, add arcs HFST_EPSILON:HFST_EPSILON and
>   TWOLC_EPSILON:HFST_UNKNOWN state->target; else if `i != TWOLC_DIAMOND`, add an
>   `i:HFST_UNKNOWN` arc state->target.
> After the loop it copies `*this` into `copy`, sets
> `copy.transducer = HfstTransducer(new_fst, transducer_type)`, and returns
> `copy.apply(&HfstTransducer::minimize)` (i.e. the minimized result).

> [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.get-transducer-fn]
> HfstTransducer OtherSymbolTransducer::get_transducer(void) const

> [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.get-transducer-fn]
> Const getter. Unless compiled with `TEST_OTHER_SYMBOL_TRANSDUCER` defined, it first
> checks `is_broken` and throws `UndefinedSymbolPairsFound` if true. Then returns a
> copy of the member `transducer` (an HfstTransducer, by value).

> [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.get-universal-fn]
> OtherSymbolTransducer OtherSymbolTransducer::get_universal(void)

> [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.get-universal-fn]
> Static. Builds a single-arc-from-start "universal" transducer accepting any one
> symbol pair from the declared alphabet (plus identity). Constructs a default
> `universal` and a basic transducer `fst` from its (empty) transducer. Adds one new
> state `target`, sets its final weight to 0.0. Adds from state 0 to `target` an
> IDENTITY:IDENTITY (`TWOLC_IDENTITY`:`TWOLC_IDENTITY`) transition weight 0.0. Then,
> for each SymbolPair in the static `symbol_pairs` set whose first is NOT TWOLC_DIAMOND
> (diamond pairs are skipped via `continue`), adds a transition from 0 to `target`
> labeled with that pair's first:second, weight 0.0. Sets
> `universal.transducer = HfstTransducer(fst, transducer_type)` and returns `universal`
> by value.

> [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.has-symbol-fn]
> bool OtherSymbolTransducer::has_symbol

> [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.has-symbol-fn]
> Static. Returns true iff symbol `sym` is present in basic transducer `t`'s
> alphabet: gets `t.get_alphabet()` and returns whether `find(sym)` is not end().

> [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.is-empty-fn]
> bool OtherSymbolTransducer::is_empty(void) const

> [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.is-empty-fn]
> Const. Returns `empty(HfstBasicTransducer(transducer))`, i.e. builds a basic copy of
> the member transducer and delegates to the static `empty` predicate (true iff it has
> no final state).

> [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.is-empty-intersection-fn]
> bool OtherSymbolTransducer::is_empty_intersection

> [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.is-empty-intersection-fn]
> Tests whether the receiver and `another` share no common accepted string. Builds
> basic copies `this_fst` (from `transducer`) and `another_fst` (from
> `another.transducer`). Initializes a `visited_pairs` set containing the StatePair
> (0,0). Returns `! have_common_string(0,0, this_fst, another_fst, visited_pairs, v)`,
> i.e. true if the two transducers have no common string. If a common string exists,
> it is left in the out parameter `v` by the recursive helper.

> [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.is-subset-fn]
> bool OtherSymbolTransducer::is_subset(const OtherSymbolTransducer &another)

> [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.is-subset-fn]
> Tests whether `*this` (its language) is a subset of `another`'s language. Copies
> `another` into `another_fst`, then applies `another_fst.apply(&HfstTransducer::subtract,
> *this)` so `another_fst` becomes `another - *this`. Builds a basic transducer
> `internal` from `another_fst.get_transducer()` and returns `empty(internal)`. (The
> subtraction direction means it actually tests whether `another \ this` is empty,
> i.e. whether `another` is a subset of `this`; the comment "Do this properly later"
> flags it as provisional.)

> [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.other-symbol-transducer-fn]
> OtherSymbolTransducer::OtherSymbolTransducer

> [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.other-symbol-transducer-fn]
> Two-argument constructor `(i_symbol, o_symbol)`. Initializes `is_broken=false` and
> `transducer(transducer_type)`. Copies the args into locals `input_symbol`,
> `output_symbol`, mapping `TWOLC_UNKNOWN` to `HFST_UNKNOWN` for each independently.
> Calls `check_pair(input_symbol, output_symbol)`; if `is_broken` is now true, returns
> immediately (leaving an empty transducer). Otherwise:
> - If both are HFST_UNKNOWN: set `transducer = get_universal().transducer`.
> - Else build a basic transducer `fst` from the (empty) member transducer, add one
>   new `target` state, set its final weight to 0.0, and:
>   - If input is HFST_UNKNOWN: for each symbol `s` in static `input_symbols` such that
>     SymbolPair(s, output_symbol) is in `symbol_pairs`, add a 0->target arc
>     `s:output_symbol` weight 0.0.
>   - Else if output is HFST_UNKNOWN: for each `s` in static `output_symbols` such that
>     SymbolPair(input_symbol, s) is in `symbol_pairs`, add a 0->target arc
>     `input_symbol:s` weight 0.0.
>   - Else add a single 0->target arc `input_symbol:output_symbol` weight 0.0.
>   Then set `transducer = HfstTransducer(fst, transducer_type)`.

> [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.remove-diacritics-from-output-fn]
> void OtherSymbolTransducer::remove_diacritics_from_output(void)

> [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.remove-diacritics-from-output-fn]
> For each diacritic `d` in the static `diacritics` set, calls
> `apply(&HfstTransducer::substitute, SymbolPair(d,d), SymbolPair(d,TWOLC_EPSILON))`,
> i.e. substitutes every `d:d` transition with `d:TWOLC_EPSILON`, removing the
> diacritic from the output side (replacing it with epsilon) while keeping it on the
> input side. No return value; mutates the member transducer (each `apply` also
> minimizes).

> [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.set-final-fn]
> void OtherSymbolTransducer::set_final

> [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.set-final-fn]
> Static helper. Marks `state` final in the mutable basic transducer `center_t` by
> calling `center_t.set_final_weight(state, 0.0)`. No return value.

> [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.set-symbol-pairs-fn]
> void OtherSymbolTransducer::set_symbol_pairs

> [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.set-symbol-pairs-fn]
> Static. Resets the global alphabet state from the provided `symbol_pairs` set.
> Clears the static `input_symbols`, `output_symbols`, and `symbol_pairs`. Inserts all
> of the argument's pairs into the static `symbol_pairs`. Then iterates the argument's
> pairs, inserting each pair's `first` into `input_symbols` and each `second` into
> `output_symbols`. Finally inserts the pair `SymbolPair(TWOLC_DIAMOND, TWOLC_DIAMOND)`
> into the static `symbol_pairs` (note: the diamond symbol is NOT added to
> input_symbols/output_symbols). No return value.

> [spec:hfst:def:other-symbol-transducer.other-symbol-transducer.set-transducer-type-fn]
> void OtherSymbolTransducer::set_transducer_type

> [spec:hfst:sem:other-symbol-transducer.other-symbol-transducer.set-transducer-type-fn]
> Static setter. Assigns the passed `transducer_type` to the static member
> `OtherSymbolTransducer::transducer_type`. No return value.

> [spec:hfst:def:other-symbol-transducer.state-pair]
> typedef std::pair<HfstState,HfstState> StatePair

> [spec:hfst:def:other-symbol-transducer.t-copy-fn]
> OtherSymbolTransducer t_copy(t)

> [spec:hfst:sem:other-symbol-transducer.t-copy-fn]
> This is the local construction `OtherSymbolTransducer t_copy(t);` inside the
> `HfstTransducerSubstPairFstMember` `apply` overload (substitute a symbol pair with a
> transducer). It copy-constructs a working clone of the operand `t` so its
> `transducer` member can be passed by reference to the substitution member function
> without mutating the caller's object. After the copy, the code computes
> `transducer = (transducer.*p)(p1, t_copy.transducer, b)` and then minimizes,
> returning `*this`. (Preceding validation in that overload throws `EmptySymbolPairSet`
> if `symbol_pairs` is empty, or `UndefinedSymbolPairsFound` if `is_broken`.)

> [spec:hfst:def:other-symbol-transducer.undefined-symbol-pairs-found]
> class UndefinedSymbolPairsFound

> [spec:hfst:def:other-symbol-transducer.universal-fn]
> OtherSymbolTransducer universal(TWOLC_UNKNOWN)

> [spec:hfst:sem:other-symbol-transducer.universal-fn]
> This is the local construction `OtherSymbolTransducer universal(TWOLC_UNKNOWN);`
> inside `contained()`. It invokes the single-symbol constructor with the unknown
> symbol, yielding (because that constructor maps TWOLC_UNKNOWN to HFST_UNKNOWN and
> then returns `get_universal().transducer`) a transducer accepting any one declared
> symbol pair. `contained()` then applies `repeat_star` to `universal` (making it
> UNIVERSAL*), copies it into `result`, and concatenates `*this` and `universal` onto
> `result` (so `result = UNIVERSAL* . this . UNIVERSAL*`), assigning `*this = result`
> and returning it. The same `OtherSymbolTransducer universal(TWOLC_UNKNOWN)` idiom is
> used in `contained_once`, `negated`, and `term_complemented`.

